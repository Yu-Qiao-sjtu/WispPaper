//! Draws an answer of the assistant: the blocks `wisp_core::markdown` reads
//! from its Markdown, one under the other. Lists are set tight, tables as a
//! grid of equal columns, links open in the browser.

use crate::theme::{self, Theme};
use gpui::{prelude::FluentBuilder, *};
use wisp_core::markdown::{Align, Block, Marker, Rich, parse};

/// `text` as elements. `id` keeps the links of this text apart from those of another.
pub fn render(id: &str, text: &str) -> Div {
    let mut links = 0;
    div().flex().flex_col().gap(px(10.)).children(blocks(id, &parse(text), &mut links))
}

fn blocks(id: &str, blocks: &[Block], links: &mut usize) -> Vec<AnyElement> {
    let mut out: Vec<AnyElement> = vec![];
    let mut at = 0;
    while at < blocks.len() {
        // The items of one list stand closer together than paragraphs do.
        let items = blocks[at..].iter().take_while(|block| matches!(block, Block::Item { .. })).count();
        if items > 0 {
            out.push(div().flex().flex_col().gap(px(4.)).children(blocks[at..at + items].iter().map(|item| block(id, item, links))).into_any_element());
            at += items;
        } else {
            out.push(block(id, &blocks[at], links));
            at += 1;
        }
    }
    out
}

fn block(id: &str, block: &Block, links: &mut usize) -> AnyElement {
    match block {
        Block::Paragraph(text) => div().line_height(px(22.)).child(rich(id, text, links)).into_any_element(),
        Block::Heading(level, text) => {
            let (size, line) = match level {
                1 => (19., 28.),
                2 => (17., 26.),
                3 => (15., 24.),
                _ => (14., 22.),
            };
            div().pt(px(4.)).text_size(px(size)).line_height(px(line)).font_weight(FontWeight::SEMIBOLD).child(rich(id, text, links)).into_any_element()
        }
        Block::Item { marker, text, children } => {
            let mark = match marker {
                Marker::Bullet => "•".to_string(),
                Marker::Number(n) => format!("{n}."),
                Marker::Todo(true) => "☑".to_string(),
                Marker::Todo(false) => "☐".to_string(),
            };
            div()
                .flex()
                .items_start()
                .gap(px(6.))
                .line_height(px(22.))
                .child(div().flex_none().min_w(px(16.)).text_right().text_color(Theme::text_secondary()).child(mark))
                .child(div().flex().flex_col().flex_1().min_w_0().gap(px(4.)).child(rich(id, text, links)).children(blocks(id, children, links)))
                .into_any_element()
        }
        Block::Code { text, .. } => div().px(px(12.)).py(px(10.)).rounded(px(6.)).border_1().border_color(Theme::line()).bg(Theme::bg_sunken()).font_family(theme::MONO_FONT).text_size(px(12.5)).line_height(px(19.)).child(text.clone()).into_any_element(),
        Block::Quote(inner) => div().flex().flex_col().gap(px(8.)).pl(px(12.)).border_l_2().border_color(Theme::line_strong()).text_color(Theme::text_secondary()).children(blocks(id, inner, links)).into_any_element(),
        Block::Rule => div().h(px(1.)).bg(Theme::line()).into_any_element(),
        Block::Table { aligns, rows } => {
            let cells = rows.iter().enumerate().flat_map(|(r, row)| row.iter().enumerate().map(move |(c, cell)| (r, c, cell))).map(|(r, c, cell)| {
                div()
                    .min_w_0()
                    .px(px(10.))
                    .py(px(6.))
                    .text_size(px(13.))
                    .line_height(px(20.))
                    .border_color(Theme::line())
                    .when(c > 0, |d| d.border_l_1())
                    .when(r > 0, |d| d.border_t_1())
                    .when(r == 0, |d| d.bg(Theme::bg_sunken()).font_weight(FontWeight::MEDIUM))
                    .map(|d| match aligns.get(c) {
                        Some(Align::Center) => d.text_center(),
                        Some(Align::Right) => d.text_right(),
                        _ => d,
                    })
                    .child(rich(id, cell, links))
            });
            let cells: Vec<Div> = cells.collect();
            div().grid().grid_cols(aligns.len() as u16).rounded(px(8.)).border_1().border_color(Theme::line()).overflow_hidden().children(cells).into_any_element()
        }
    }
}

/// Text with its marks; with links in it, they open on a click.
fn rich(id: &str, rich: &Rich, links: &mut usize) -> AnyElement {
    let mut highlights = vec![];
    let (mut ranges, mut urls) = (vec![], vec![]);
    for (range, marks) in rich.spans() {
        let mut style = HighlightStyle::default();
        if marks.bold { style.font_weight = Some(FontWeight::SEMIBOLD) }
        if marks.italic { style.font_style = Some(FontStyle::Italic) }
        if marks.strike { style.strikethrough = Some(StrikethroughStyle { thickness: px(1.), color: None }) }
        if marks.code { style.background_color = Some(Theme::groove()) }
        if let Some(url) = &marks.link {
            style.color = Some(Theme::accent());
            style.underline = Some(UnderlineStyle { thickness: px(1.), color: None, wavy: false });
            ranges.push(range.clone());
            urls.push(url.clone());
        }
        if style != HighlightStyle::default() { highlights.push((range, style)) }
    }
    let text = StyledText::new(rich.text.clone()).with_highlights(highlights);
    if urls.is_empty() { return text.into_any_element() }
    *links += 1;
    InteractiveText::new(SharedString::from(format!("{id}-link-{links}")), text).on_click(ranges, move |at, _, cx| cx.open_url(&urls[at])).into_any_element()
}
