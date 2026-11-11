//! PDF preview. A worker thread owns the parsed file and rasterises pages
//! with hayro (pure Rust, no system PDF library); the view is a virtualised
//! list that asks for the pages on screen and keeps the latest few.

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

enum Reply {
    /// Page sizes in points, once, after the file parsed.
    Opened(Vec<(f32, f32)>),
    Failed(String),
    Page(usize, u32, u32, Vec<u8>),
}

fn worker(path: PathBuf, requests: mpsc::Receiver<usize>, replies: async_channel::Sender<Reply>) {
    use hayro::hayro_interpret::InterpreterSettings;
    use hayro::hayro_syntax::Pdf;
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
    let (interpreter, settings) = (InterpreterSettings::default(), RenderSettings::default());
    while let Ok(first) = requests.recv() {
        // Scrolling queues pages faster than they render: take the latest request first.
        let mut wanted = vec![first];
        wanted.extend(requests.try_iter());
        wanted.dedup();
        let Some(index) = wanted.pop() else { continue };
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

pub struct PdfView {
    sizes: Vec<(f32, f32)>,
    images: HashMap<usize, Arc<RenderImage>>,
    /// Pages that failed to render.
    broken: HashSet<usize>,
    /// Most recently shown last.
    recent: VecDeque<usize>,
    pending: HashSet<usize>,
    list: ListState,
    requests: mpsc::Sender<usize>,
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
        }
    }

    pub fn page_count(&self) -> usize {
        self.sizes.len()
    }

    /// The page at the top of the view, 1-based.
    pub fn current_page(&self) -> usize {
        self.list.logical_scroll_top().item_ix + 1
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
        let width = (self.width - 32.).max(120.);
        let height = width * h / w.max(1.);
        let image = self.images.get(&index).cloned();
        if image.is_some() {
            self.touch(index);
        } else if !self.broken.contains(&index) && self.pending.insert(index) {
            let _ = self.requests.send(index);
        }
        let sheet = div().w(px(width)).h(px(height)).bg(gpui::white()).border_1().border_color(Theme::line()).overflow_hidden();
        let sheet = match image {
            Some(image) => sheet.child(img(image).w(px(width)).h(px(height))),
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
            .child(list(self.list.clone(), move |index, _, cx| entity.update(cx, |this, _| this.page(index))).size_full().pt(px(PAGE_GAP)))
            .into_any_element()
    }
}
