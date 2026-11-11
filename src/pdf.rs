//! PDF preview. A worker thread owns the parsed file and rasterises pages
//! with hayro (pure Rust, no system PDF library); the view is a virtualised
//! list that asks for the pages on screen and keeps the latest few. The same
//! thread finds a quoted sentence among the glyphs of a page, so a citation
//! can be shown where it stands.

use crate::theme::Theme;
use crate::ui;
use gpui::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc;

/// Pixels per PDF point: 2× keeps text sharp on a high-density display at reading width.
const RENDER_SCALE: f32 = 2.0;
/// Rendered pages kept in memory (about 8 MB each for A4).
const KEEP_PAGES: usize = 10;
const PAGE_GAP: f32 = 12.;

/// The pages a quotation is looked for on before giving up.
const FIND_PAGES: usize = 300;
/// Page renders kept from a burst of requests; older ones are handed back.
const KEEP_REQUESTS: usize = 3;

enum Request {
    Page(usize),
    /// Look for a quoted sentence, starting at this 0-based page.
    Find(Option<usize>, String),
}

enum Reply {
    /// Page sizes in points, once, after the file parsed.
    Opened(Vec<(f32, f32)>),
    Failed(String),
    Page(usize, u32, u32, Vec<u8>),
    /// A page whose render was dropped for newer ones: ask again when it is in view.
    Skipped(usize),
    /// Where the quotation is: the page, and one box per line in page points.
    Found(usize, Vec<[f32; 4]>),
    NotFound,
}

/// Collects the glyphs of a page as it is interpreted: what each one says
/// and where it stands, in page points with the origin at the top left.
#[derive(Default)]
struct Glyphs {
    texts: Vec<String>,
    boxes: Vec<[f32; 4]>,
}

mod device {
    use super::Glyphs;
    use hayro::hayro_interpret::font::GlyphRun;
    use hayro::hayro_interpret::hayro_cmap::BfString;
    use hayro::hayro_interpret::{BlendMode, ClipPath, Device, DrawMode, DrawProps, Image, ImageDrawProps, SoftMask};
    use hayro::kurbo::{BezPath, Point};

    impl<'a> Device<'a> for Glyphs {
        fn draw_glyph_run(&mut self, run: &GlyphRun<'_, 'a>, props: DrawProps<'a>, _: &DrawMode) {
            for glyph in run.glyphs() {
                let text = match glyph.as_unicode() {
                    Some(BfString::Char(c)) => c.to_string(),
                    Some(BfString::String(text)) => text,
                    None => continue,
                };
                // Glyph space is a thousandth of the font size. The advance is not known
                // here: half an em from the origin, from below the baseline to the cap height.
                let place = props.transform * glyph.transform();
                let corners = [(0., -220.), (0., 780.), (500., -220.), (500., 780.)].map(|(x, y)| place * Point::new(x, y));
                let (xs, ys) = (corners.map(|p| p.x as f32), corners.map(|p| p.y as f32));
                self.texts.push(text);
                self.boxes.push([xs.iter().copied().fold(f32::MAX, f32::min), ys.iter().copied().fold(f32::MAX, f32::min), xs.iter().copied().fold(f32::MIN, f32::max), ys.iter().copied().fold(f32::MIN, f32::max)]);
            }
        }
        fn draw_path(&mut self, _: &BezPath, _: DrawProps<'a>, _: &DrawMode) {}
        fn push_clip_path(&mut self, _: &ClipPath) {}
        fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'a>>, _: BlendMode) {}
        fn draw_image(&mut self, _: Image<'a, '_>, _: ImageDrawProps<'a>) {}
        fn pop_clip(&mut self) {}
        fn pop_transparency_group(&mut self) {}
    }
}

/// The first pages of a PDF as PNG pictures, for a model to read a file
/// that gives no text of its own. One and a half pixels per point: enough
/// to read print by, and small enough to send.
pub fn page_pictures(path: &Path, limit: usize) -> Result<Vec<Vec<u8>>, String> {
    use hayro::hayro_interpret::InterpreterSettings;
    use hayro::hayro_syntax::Pdf;
    use hayro::vello_cpu::color::palette::css::WHITE;
    use hayro::{PixmapSettings, RenderCache, RenderSettings, render};
    let data = std::fs::read(path).map_err(|err| format!("无法读取文件：{err}"))?;
    let pdf = Pdf::new(data).map_err(|err| format!("无法解析这个 PDF（{err:?}）。"))?;
    let (cache, interpreter, settings) = (RenderCache::new(), InterpreterSettings::default(), RenderSettings::default());
    let mut out = vec![];
    for page in pdf.pages().iter().take(limit) {
        let (w, h) = page.render_dimensions();
        let scale = 1.5_f32.min(2000. / w.max(h).max(1.));
        let pixmap = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| render(page, &cache, &interpreter, &settings, &PixmapSettings { x_scale: scale, y_scale: scale, bg_color: WHITE }))).map_err(|_| "这个 PDF 的页面画不出来。".to_string())?;
        // On an opaque white background premultiplied colour is the colour itself.
        let picture = image::RgbaImage::from_raw(pixmap.width() as u32, pixmap.height() as u32, pixmap.data_as_u8_slice().to_vec()).ok_or("这个 PDF 的页面画不出来。")?;
        let mut png = vec![];
        image::DynamicImage::ImageRgba8(picture).to_rgb8().write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).map_err(|err| format!("页面图像写不出来：{err}"))?;
        out.push(png);
    }
    if out.is_empty() { return Err("这个 PDF 没有页面。".into()) }
    Ok(out)
}

fn worker(path: PathBuf, requests: mpsc::Receiver<Request>, replies: async_channel::Sender<Reply>) {
    use hayro::hayro_interpret::{Context, InterpreterCache, InterpreterSettings, TransformExt, interpret_page};
    use hayro::hayro_syntax::Pdf;
    use hayro::kurbo::Rect;
    use hayro::vello_cpu::color::palette::css::WHITE;
    use hayro::{PixmapSettings, RenderCache, RenderSettings, render};
    let data = match std::fs::read(&path) {
        Ok(data) => data,
        Err(err) => {
            let _ = replies.send_blocking(Reply::Failed(format!("无法读取文件：{err}")));
            return;
        }
    };
    let pdf = match Pdf::new(data) {
        Ok(pdf) => pdf,
        Err(err) => {
            let _ = replies.send_blocking(Reply::Failed(format!("无法解析这个 PDF（{err:?}）。可以改用系统应用打开。")));
            return;
        }
    };
    let pages = pdf.pages();
    if replies.send_blocking(Reply::Opened(pages.iter().map(|p| p.render_dimensions()).collect())).is_err() { return }
    let cache = RenderCache::new();
    let text_cache = InterpreterCache::new();
    let (interpreter, settings) = (InterpreterSettings::default(), RenderSettings::default());
    while let Ok(first) = requests.recv() {
        let mut waiting = vec![first];
        waiting.extend(requests.try_iter());
        // Of several quotations asked for in a row, only the last still matters.
        let find = waiting.iter().rev().find_map(|request| match request {
            Request::Find(page, quote) => Some((*page, quote.clone())),
            Request::Page(_) => None,
        });
        // Scrolling queues pages faster than they render: the latest few are drawn,
        // newest first, and the view is told about the rest so it can ask again.
        let mut wanted: Vec<usize> = vec![];
        for request in waiting.iter().rev() {
            if let Request::Page(index) = request && !wanted.contains(index) { wanted.push(*index) }
        }
        for skipped in wanted.split_off(wanted.len().min(KEEP_REQUESTS)) {
            if replies.send_blocking(Reply::Skipped(skipped)).is_err() { return }
        }

        if let Some((start, quote)) = find {
            // The page named first, then its neighbours, then the rest in order.
            let mut order: Vec<usize> = start.into_iter().flat_map(|page| [Some(page), page.checked_sub(1), Some(page + 1)]).flatten().filter(|page| *page < pages.len()).collect();
            let rest: Vec<usize> = (0..pages.len().min(FIND_PAGES)).filter(|page| !order.contains(page)).collect();
            order.extend(rest);
            let mut reply = Reply::NotFound;
            for index in order {
                let page = &pages[index];
                let (w, h) = page.render_dimensions();
                let read = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let mut glyphs = Glyphs::default();
                    let mut context = Context::new(page.initial_transform(true).to_kurbo(), Rect::new(0., 0., w as f64, h as f64), &text_cache, page.xref(), interpreter.clone());
                    interpret_page(page, &mut context, &mut glyphs);
                    glyphs
                }));
                let Ok(glyphs) = read else { continue };
                if let Some(range) = wisp_core::fulltext::find_quote(&glyphs.texts, &quote) {
                    reply = Reply::Found(index, wisp_core::fulltext::line_boxes(&glyphs.boxes[range]));
                    break;
                }
            }
            if replies.send_blocking(reply).is_err() { return }
        }

        for index in wanted {
            let Some(page) = pages.get(index) else { continue };
            let (w, h) = page.render_dimensions();
            // Pixmaps are at most 65535 px a side; huge posters render smaller.
            let scale = RENDER_SCALE.min(8000. / w.max(h).max(1.));
            let rendered = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| render(page, &cache, &interpreter, &settings, &PixmapSettings { x_scale: scale, y_scale: scale, bg_color: WHITE })));
            let reply = match rendered {
                Ok(pixmap) => {
                    let (width, height) = (pixmap.width() as u32, pixmap.height() as u32);
                    // Opaque white background, so premultiplied equals straight; GPUI wants BGRA.
                    let mut bytes = pixmap.data_as_u8_slice().to_vec();
                    for pixel in bytes.as_chunks_mut::<4>().0 {
                        pixel.swap(0, 2);
                    }
                    Reply::Page(index, width, height, bytes)
                }
                Err(_) => Reply::Page(index, 0, 0, vec![]),
            };
            if replies.send_blocking(reply).is_err() { return }
        }
    }
}

pub struct PdfView {
    sizes: Vec<(f32, f32)>,
    images: HashMap<usize, Arc<RenderImage>>,
    /// Pages that failed to render.
    broken: HashSet<usize>,
    /// Most recently shown last.
    recent: VecDeque<usize>,
    pending: HashSet<usize>,
    list: ListState,
    requests: mpsc::Sender<Request>,
    /// The quotation on show: its 0-based page, and one box per line in page points.
    highlight: Option<(usize, Vec<[f32; 4]>)>,
    /// A quotation was looked for and is not in this PDF.
    missing: bool,
    error: Option<String>,
    width: f32,
    /// A page to jump to once the file has opened.
    goto: Option<usize>,
}

impl PdfView {
    pub fn new(path: &Path, cx: &mut Context<Self>) -> Self {
        let (requests, request_rx) = mpsc::channel();
        let (reply_tx, replies) = async_channel::unbounded();
        let file = path.to_path_buf();
        std::thread::Builder::new().name("pdf".into()).spawn(move || worker(file, request_rx, reply_tx)).ok();
        cx.spawn(async move |this, cx| {
            while let Ok(reply) = replies.recv().await {
                if this.update(cx, |this, cx| this.receive(reply, cx)).is_err() { break }
            }
        })
        .detach();
        PdfView {
            sizes: vec![],
            images: HashMap::new(),
            broken: HashSet::new(),
            recent: VecDeque::new(),
            pending: HashSet::new(),
            list: ListState::new(0, ListAlignment::Top, px(600.)),
            requests,
            error: None,
            width: 0.,
            goto: None,
            highlight: None,
            missing: false,
        }
    }

    pub fn page_count(&self) -> usize {
        self.sizes.len()
    }

    /// The page at the top of the view, 1-based.
    pub fn current_page(&self) -> usize {
        self.list.logical_scroll_top().item_ix + 1
    }

    /// Go to a 1-based page and look for a sentence quoted from it; when it
    /// is found, there or on another page, it is highlighted and scrolled
    /// into view.
    pub fn find(&mut self, page: usize, quote: &str, cx: &mut Context<Self>) {
        self.highlight = None;
        self.missing = false;
        self.go_to(page, cx);
        let _ = self.requests.send(Request::Find(page.checked_sub(1), quote.to_string()));
    }

    /// How wide a page is drawn, and so how many pixels a page point takes.
    fn sheet_width(&self) -> f32 {
        (self.width - 32.).max(120.)
    }

    /// Scroll to a 1-based page.
    pub fn go_to(&mut self, page: usize, cx: &mut Context<Self>) {
        if self.sizes.is_empty() {
            self.goto = Some(page);
        } else {
            self.list.scroll_to(ListOffset { item_ix: page.clamp(1, self.sizes.len()) - 1, offset_in_item: px(0.) });
        }
        cx.notify();
    }

    fn receive(&mut self, reply: Reply, cx: &mut Context<Self>) {
        match reply {
            Reply::Opened(sizes) => {
                self.sizes = sizes;
                self.list.reset(self.sizes.len());
                if let Some(page) = self.goto.take() { self.go_to(page, cx) }
            }
            Reply::Failed(error) => self.error = Some(error),
            Reply::Skipped(index) => {
                self.pending.remove(&index);
            }
            Reply::Found(index, boxes) => {
                // Bring the first line of the quotation a little below the top of the view.
                if let (Some(first), Some((w, _))) = (boxes.first(), self.sizes.get(index)) {
                    let top = first[1] * self.sheet_width() / w.max(1.) - 80.;
                    self.list.scroll_to(ListOffset { item_ix: index, offset_in_item: px(top.max(0.)) });
                }
                self.highlight = Some((index, boxes));
            }
            Reply::NotFound => self.missing = true,
            Reply::Page(index, width, height, bytes) => {
                self.pending.remove(&index);
                match image::RgbaImage::from_raw(width, height, bytes).filter(|_| width > 0) {
                    Some(buffer) => {
                        self.images.insert(index, Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])));
                        self.touch(index);
                        while self.recent.len() > KEEP_PAGES {
                            if let Some(old) = self.recent.pop_front() && let Some(image) = self.images.remove(&old) {
                                cx.drop_image(image, None);
                            }
                        }
                    }
                    None => {
                        self.broken.insert(index);
                    }
                }
            }
        }
        cx.notify();
    }

    fn touch(&mut self, index: usize) {
        self.recent.retain(|i| *i != index);
        self.recent.push_back(index);
    }

    fn page(&mut self, index: usize) -> AnyElement {
        let (w, h) = self.sizes.get(index).copied().unwrap_or((595., 842.));
        let width = self.sheet_width();
        let height = width * h / w.max(1.);
        let image = self.images.get(&index).cloned();
        if image.is_some() {
            self.touch(index);
        } else if !self.broken.contains(&index) && self.pending.insert(index) {
            let _ = self.requests.send(Request::Page(index));
        }
        // The quotation on this page: a translucent marker over each of its lines.
        let scale = width / w.max(1.);
        let marks: Vec<Div> = self
            .highlight
            .iter()
            .filter(|(page, _)| *page == index)
            .flat_map(|(_, boxes)| boxes.iter())
            .map(|line| div().absolute().left(px(line[0] * scale - 1.)).top(px(line[1] * scale)).w(px((line[2] - line[0]) * scale + 2.)).h(px((line[3] - line[1]) * scale)).rounded(px(2.)).bg(hsla(50. / 360., 1., 0.5, 0.35)))
            .collect();
        let sheet = div().relative().w(px(width)).h(px(height)).bg(gpui::white()).border_1().border_color(Theme::line()).overflow_hidden();
        let sheet = match image {
            Some(image) => sheet.child(img(image).w(px(width)).h(px(height))).children(marks),
            None => sheet.flex().items_center().justify_center().text_size(px(12.)).text_color(Theme::text_placeholder()).child(if self.broken.contains(&index) { format!("第 {} 页无法渲染", index + 1) } else { format!("第 {} 页", index + 1) }),
        };
        div().w_full().flex().justify_center().pb(px(PAGE_GAP)).child(sheet).into_any_element()
    }

    /// The viewer, `width` logical pixels wide.
    pub fn set_width(&mut self, width: f32) {
        if (self.width - width).abs() > 0.5 {
            self.width = width;
            // Page heights follow the width: measure the list again, keeping the page in view.
            let top = self.list.logical_scroll_top();
            self.list.reset(self.sizes.len());
            self.list.scroll_to(ListOffset { item_ix: top.item_ix, offset_in_item: px(0.) });
        }
    }
}

impl Drop for PdfView {
    fn drop(&mut self) {
        // The textures of dropped images are released with the window's atlas entries.
        self.images.clear();
    }
}

impl Render for PdfView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(error) = &self.error {
            return ui::empty_state("image-broken", "无法预览", error).size_full().into_any_element();
        }
        if self.sizes.is_empty() {
            return ui::empty_state("file-text", "正在打开 PDF…", "").size_full().into_any_element();
        }
        let entity = cx.entity();
        div()
            .size_full()
            .bg(Theme::bg_sunken())
            .relative()
            .child(list(self.list.clone(), move |index, _, cx| entity.update(cx, |this, _| this.page(index))).size_full().pt(px(PAGE_GAP)))
            // Said plainly: the page is the cited one, the sentence was not found on it or anywhere else.
            .children(self.missing.then(|| div().absolute().top(px(8.)).left_0().w_full().flex().justify_center().child(div().px(px(10.)).py(px(5.)).rounded(px(6.)).bg(Theme::bg_body()).border_1().border_color(Theme::line()).text_size(px(12.)).text_color(Theme::text_secondary()).child("已跳到引用的页，但没有在 PDF 里找到这句原文。"))))
            .into_any_element()
    }
}
