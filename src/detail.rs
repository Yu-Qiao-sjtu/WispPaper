//! The detail panel of the selected paper: 信息 (record, statuses, custom
//! fields, notes, files), PDF (preview) and 提问 (questions about this paper).

use crate::app::{Cmd, MenuKind, Tab, Workspace};
use crate::theme::{self, Theme};
use crate::ui;
use gpui::{prelude::FluentBuilder, *};
use wisp_core::model::*;
use wisp_core::time;

impl Workspace {
    pub fn render_detail(&mut self, width: f32, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let lib = self.lib.as_ref().unwrap();
        let Some(p) = lib.selected_paper() else { return div().into_any_element() };
        let tab = lib.tab;
        let has_pdf = lib.pdf_file(&p.id).is_some();
        let tab_button = |id: &'static str, label: &'static str, value: Tab, this: &Self, cx: &mut Context<Self>| {
            let active = tab == value;
            div()
                .id(id)
                .flex()
                .items_center()
                .h(px(34.))
                .px(px(4.))
                .border_b_2()
                .cursor_pointer()
                .border_color(if active { Theme::accent() } else { gpui::transparent_black() })
                .text_color(if active { Theme::text() } else { Theme::text_tertiary() })
                .when(active, |d| d.font_weight(FontWeight::MEDIUM))
                .hover(|s| s.text_color(Theme::text()))
                .on_click(this.on(Cmd::SetTab(value), cx))
                .child(label)
        };
        let header = div()
            .flex()
            .flex_col()
            .flex_none()
            .px(px(16.))
            .pt(px(12.))
            .border_b_1()
            .border_color(Theme::line())
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(6.))
                    .child(div().flex_1().min_w_0().line_clamp(3).text_size(px(15.)).line_height(px(22.)).font_weight(FontWeight::SEMIBOLD).child(p.title.clone()))
                    .child(ui::ghost_button("detail-wide", Some(if lib.wide { "panel-left-open" } else { "expand" }), None).on_click(self.on(Cmd::ToggleWide, cx)))
                    .child(ui::ghost_button("detail-close", Some("x"), None).on_click(self.on(Cmd::CloseDetail, cx))),
            )
            .child(
                div()
                    .flex()
                    .gap(px(16.))
                    .pt(px(6.))
                    .child(tab_button("tab-info", "信息", Tab::Info, self, cx))
                    .child(tab_button("tab-pdf", if has_pdf { "PDF" } else { "PDF（无）" }, Tab::Pdf, self, cx))
                    .child(tab_button("tab-ask", "提问", Tab::Ask, self, cx)),
            );
        let body = match tab {
            Tab::Info => self.render_info(window, cx),
            Tab::Pdf => self.render_pdf_tab(width, cx),
            Tab::Ask => self.render_chat(true, window, cx),
        };
        div().flex().flex_col().flex_none().w(px(width)).h_full().bg(Theme::bg_body()).border_l_1().border_color(Theme::line()).child(ui::title_strip()).child(header).child(div().flex().flex_col().flex_1().min_h_0().child(body)).into_any_element()
    }

    fn render_info(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let lib = self.lib.as_ref().unwrap();
        let Some(p) = lib.selected_paper() else { return div().into_any_element() };
        let id = p.id.clone();
        let section = |title: &'static str| div().flex().flex_col().gap(px(6.)).child(ui::caption(title));
        // A status the reader changes with one click.
        let pick = |element_id: &'static str, value: &str, text: &str, kind: MenuKind, this: &Self, cx: &mut Context<Self>| {
            let (color, fill) = theme::status_colors(value);
            div()
                .id(element_id)
                .flex()
                .flex_none()
                .items_center()
                .gap(px(3.))
                .h(px(24.))
                .pl(px(9.))
                .pr(px(6.))
                .rounded(px(12.))
                .bg(fill)
                .text_color(color)
                .text_size(px(12.))
                .cursor_pointer()
                .hover(|s| s.opacity(0.85))
                .on_click(this.on_menu(kind, cx))
                .child(text.to_string())
                .child(ui::icon("chevron-down", 11., color))
        };
        let link = p.link();
        let mut source_line = vec![];
        if !p.journal.is_empty() { source_line.push(p.journal.clone()) }
        if let Some(year) = p.year { source_line.push(year.to_string()) }
        if p.publication_status != "unknown" { source_line.push(label(&PUBLICATION_STATUSES, &p.publication_status).to_string()) }
        let identifiers: Vec<AnyElement> = [("DOI", &p.doi), ("PMID", &p.pmid)]
            .into_iter()
            .filter(|(_, value)| !value.is_empty())
            .map(|(name, value)| div().flex().gap(px(6.)).text_size(px(12.)).child(div().flex_none().w(px(40.)).text_color(Theme::text_tertiary()).child(name)).child(div().min_w_0().text_color(Theme::text_secondary()).child(value.clone())).into_any_element())
            .collect();
        let field_rows: Vec<AnyElement> = lib
            .fields
            .iter()
            .map(|field| {
                let values = p.fields.get(&field.id).cloned().unwrap_or_default();
                div()
                    .flex()
                    .items_start()
                    .gap(px(8.))
                    .child(div().flex_none().w(px(84.)).pt(px(1.)).truncate().text_size(px(12.)).text_color(Theme::text_tertiary()).child(field.name.clone()))
                    .child(if values.is_empty() {
                        div().text_color(Theme::text_placeholder()).child("未填写")
                    } else {
                        div().flex().flex_wrap().flex_1().min_w_0().gap(px(4.)).children(values.into_iter().map(|v| ui::chip(v, Theme::text_secondary(), Theme::bg_sunken())))
                    })
                    .into_any_element()
            })
            .collect();
        let files = lib.paper_files(&id);
        let file_rows: Vec<AnyElement> = files
            .iter()
            .enumerate()
            .map(|(i, file)| {
                let note = match lib.store.fulltext_pages(&file.id) {
                    _ if lib.extracting.contains(&file.id) => "正在提取文字…".to_string(),
                    Ok(Some(Ok(pages))) => format!("{} 页文字可用于提问", pages.len()),
                    Ok(Some(Err(error))) => error,
                    _ => String::new(),
                };
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .p(px(8.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(Theme::line())
                    .child(ui::icon("file-text", 18., Theme::accent()))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .child(div().truncate().child(file.title.clone()))
                            .child(div().truncate().text_size(px(12.)).text_color(Theme::text_tertiary()).child(format!("{} · {}{}", if file.kind == "fulltext" { "全文" } else { "附件" }, file_size(file.size_bytes), if note.is_empty() { String::new() } else { format!(" · {note}") }))),
                    )
                    .child(ui::ghost_button(("file-open", i), Some("link"), None).on_click(self.on(Cmd::OpenFile(file.id.clone()), cx)))
                    .child(ui::ghost_button(("file-remove", i), Some("x"), None).on_click(self.on(Cmd::RemoveFile(file.id.clone()), cx)))
                    .into_any_element()
            })
            .collect();
        let has_fields = !lib.fields.is_empty();
        div()
            .id("detail-info")
            .flex()
            .flex_col()
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&lib.detail_scroll)
            .p(px(16.))
            .gap(px(18.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(div().text_color(Theme::text_secondary()).child(if p.authors.is_empty() { "作者待补".to_string() } else { p.authors.clone() }))
                    .when(!source_line.is_empty(), |d| d.child(div().text_size(px(12.)).text_color(Theme::text_tertiary()).child(source_line.join(" · "))))
                    .children(identifiers),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(6.))
                    .child(pick("pick-reading", &p.reading, label(&READINGS, &p.reading), MenuKind::Reading(id.clone()), self, cx))
                    .child(pick("pick-screening", &p.screening, label(&SCREENINGS, &p.screening), MenuKind::Screening(id.clone()), self, cx))
                    .child(ui::status_chip(&p.fulltext, label(&FULLTEXTS, &p.fulltext)))
                    .child(ui::ghost_button("detail-star", Some("star"), Some(if p.starred { "已收藏".into() } else { "收藏".into() })).when(p.starred, |d| d.text_color(Theme::star())).on_click(self.on(Cmd::ToggleStar(id.clone()), cx))),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(8.))
                    .child(ui::secondary_button("detail-edit", "编辑信息与词条").on_click(self.on(Cmd::EditPaper(id.clone()), cx)))
                    .when(!link.is_empty(), |d| d.child(ui::secondary_button("detail-link", "打开网页").on_click(self.on(Cmd::OpenLink(id.clone()), cx))))
                    .child(ui::ghost_button("detail-trash", Some("bin"), Some("移到回收站".into())).h(px(32.)).on_click(self.on(Cmd::TrashPaper(id.clone()), cx))),
            )
            .child(section("标签").child(if p.tags.is_empty() {
                div().text_color(Theme::text_placeholder()).child("没有标签")
            } else {
                div().flex().flex_wrap().gap(px(4.)).children(p.tags.iter().map(|t| ui::chip(format!("#{t}"), Theme::accent(), Theme::accent_soft())))
            }))
            .child(section("自定义词条").child(if has_fields {
                div().flex().flex_col().gap(px(8.)).children(field_rows)
            } else {
                div().text_color(Theme::text_placeholder()).child("这个文献库还没有自定义词条。在左下角「自定义词条」里添加，例如物种、研究方法、所属课题。")
            }))
            .child(section("摘要").child(if p.abstract_text.is_empty() { div().text_color(Theme::text_placeholder()).child("没有摘要") } else { div().text_color(Theme::text_secondary()).line_height(px(21.)).child(p.abstract_text.clone()) }))
            .child(section("我的笔记").child(ui::field(&lib.notes, window, cx)))
            .child(section("文件").child(div().flex().flex_col().gap(px(6.)).children(file_rows).child(ui::secondary_button("attach", if files.is_empty() { "添加 PDF 或附件" } else { "再添加一个文件" }).on_click(self.on(Cmd::AttachPdf(id.clone()), cx)))))
            .child(div().text_size(px(12.)).text_color(Theme::text_placeholder()).child(format!("来源：{} · 添加于 {}", p.source, time::date_text(&p.added_at))))
            .into_any_element()
    }

    fn render_pdf_tab(&mut self, width: f32, cx: &mut Context<Self>) -> AnyElement {
        let lib = self.lib.as_ref().unwrap();
        let Some(p) = lib.selected_paper() else { return div().into_any_element() };
        let Some(file) = lib.pdf_file(&p.id) else {
            return ui::empty_state("file-text", "这篇文献还没有 PDF", "添加后可以在这里预览，全文也会用于提问。").child(ui::primary_button("pdf-attach", "添加 PDF").on_click(self.on(Cmd::AttachPdf(p.id.clone()), cx))).size_full().into_any_element();
        };
        let Some(pdf) = lib.pdf.clone() else { return div().into_any_element() };
        pdf.update(cx, |pdf, _| pdf.set_width(width));
        let (page, count) = {
            let view = pdf.read(cx);
            (view.current_page(), view.page_count())
        };
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(8.))
                    .h(px(36.))
                    .px(px(12.))
                    .border_b_1()
                    .border_color(Theme::line())
                    .text_size(px(12.))
                    .text_color(Theme::text_tertiary())
                    .child(div().flex_1().min_w_0().truncate().child(file.title.clone()))
                    .when(count > 0, |d| d.child(div().flex_none().child(format!("{page} / {count} 页"))))
                    .child(ui::ghost_button("pdf-open", Some("link"), Some("用系统应用打开".into())).on_click(self.on(Cmd::OpenFile(file.id.clone()), cx))),
            )
            .child(div().flex_1().min_h_0().child(pdf))
            .into_any_element()
    }
}
