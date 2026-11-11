//! An open library: the sidebar (collect, views, tags), the paper table with
//! its search and filters, and the trash.

use crate::app::{Cmd, FilterKey, Library, MenuKind, Page, Tab, Workspace};
use crate::settings::SettingsPage;
use crate::theme::{self, Theme};
use crate::ui;
use gpui::{prelude::FluentBuilder, *};
use std::ops::Range;
use wisp_core::filter::{self, View};
use wisp_core::model::*;
use wisp_core::time;

const SIDEBAR_W: f32 = 224.;
pub const DETAIL_W: f32 = 448.;

/// A column of the paper table: fixed width, or the flexible title column.
struct Column {
    label: String,
    width: Option<f32>,
    sort: Option<&'static str>,
    /// What the cells show.
    kind: ColumnKind,
}

#[derive(Clone)]
enum ColumnKind {
    Star,
    Title,
    Journal,
    Year,
    Field(String),
    Fulltext,
    Reading,
    Screening,
}

impl Workspace {
    /// The detail panel's width for this window.
    pub fn detail_width(&self, window: &Window) -> f32 {
        let Some(lib) = &self.lib else { return 0. };
        if lib.selected.is_none() || matches!(lib.page, Page::Trash | Page::Sources | Page::Guide) { return 0. }
        let viewport = f32::from(window.viewport_size().width);
        if lib.wide { (viewport * 0.62).max(DETAIL_W).min(viewport - SIDEBAR_W - 80.) } else { DETAIL_W.min(viewport - SIDEBAR_W - 80.).max(280.) }
    }

    pub fn render_library(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let detail_width = self.detail_width(window);
        let main_width = f32::from(window.viewport_size().width) - SIDEBAR_W - detail_width;
        let page = self.lib.as_ref().map_or(Page::Papers, |l| l.page);
        let main = match page {
            Page::Papers => self.render_papers(main_width, window, cx),
            Page::Ask => self.render_chat(false, window, cx),
            Page::Trash => self.render_trash(cx),
            Page::Sources => self.render_searches(cx),
            Page::Guide => self.render_guide(window, cx),
        };
        div()
            .flex()
            .size_full()
            .child(self.render_sidebar(cx))
            .child(div().flex().flex_col().flex_1().min_w_0().h_full().bg(Theme::bg_body()).border_l_1().border_color(Theme::line()).child(ui::title_strip()).child(div().flex().flex_col().flex_1().min_h_0().child(main)))
            .when(detail_width > 0., |d| d.child(self.render_detail(detail_width, window, cx)))
            .into_any_element()
    }

    fn render_sidebar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let lib = self.lib.as_ref().unwrap();
        let now = time::now_millis();
        let nav = |id: &'static str, icon: &'static str, label: &'static str, count: Option<usize>, active: bool, cmd: Cmd, this: &Self, cx: &mut Context<Self>| {
            div()
                .id(id)
                .flex()
                .items_center()
                .gap(px(8.))
                .h(px(30.))
                .px(px(8.))
                .rounded(px(6.))
                .cursor_pointer()
                .when(active, |d| d.bg(Theme::accent_soft()).text_color(Theme::accent()).font_weight(FontWeight::MEDIUM))
                .when(!active, |d| d.hover(|s| s.bg(Theme::hover())))
                .on_click(this.on(cmd, cx))
                .child(ui::icon(icon, 16., if active { Theme::accent() } else { Theme::text_secondary() }))
                .child(div().flex_1().min_w_0().truncate().child(label))
                .when_some(count, |d, n| d.child(div().text_size(px(12.)).text_color(Theme::text_tertiary()).child(n.to_string())))
        };
        let on_papers = lib.page == Page::Papers;
        let views = [
            ("view-all", "library", "全部文献", View::All),
            ("view-recent", "plus", "最近添加", View::Recent),
            ("view-unread", "eye", "未读", View::Unread),
            ("view-starred", "star", "收藏", View::Starred),
            ("view-pending", "list-todo", "待判断", View::Pending),
            ("view-nopdf", "file-text", "缺 PDF", View::NoPdf),
        ];
        let view_rows: Vec<AnyElement> = views
            .into_iter()
            .map(|(id, icon, label, view)| nav(id, icon, label, Some(filter::view_count(&lib.papers, view, now)), on_papers && lib.filters.view == view, Cmd::SetView(view), self, cx).into_any_element())
            .collect();
        let tags = filter::tag_counts(&lib.papers);
        let tag_rows: Vec<AnyElement> = tags
            .iter()
            .take(30)
            .enumerate()
            .map(|(i, (tag, count))| {
                let active = on_papers && &lib.filters.tag == tag;
                div()
                    .id(("tag", i))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .h(px(28.))
                    .px(px(8.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .when(active, |d| d.bg(Theme::accent_soft()).text_color(Theme::accent()))
                    .when(!active, |d| d.hover(|s| s.bg(Theme::hover())))
                    .on_click(self.on(Cmd::SetFilter(FilterKey::Tag, if active { String::new() } else { tag.clone() }), cx))
                    .child(div().size(px(6.)).flex_none().rounded_full().bg(if active { Theme::accent() } else { Theme::text_placeholder() }))
                    .child(div().flex_1().min_w_0().truncate().child(tag.clone()))
                    .child(div().text_size(px(12.)).text_color(Theme::text_tertiary()).child(count.to_string()))
                    .into_any_element()
            })
            .collect();
        let trash_count = lib.trash.len();
        div()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(SIDEBAR_W))
            .h_full()
            .px(px(10.))
            .pb(px(10.))
            .pt(px(if ui::custom_titlebar() { 4. } else { 10. }))
            .gap(px(10.))
            .child(ui::title_strip().pl(px(ui::traffic_lights_width())).child(ui::ghost_button("back-home", Some("arrow-left"), Some("所有文献库".into())).occlude().on_click(self.on(Cmd::GoHome, cx))))
            .child(
                div()
                    .id("library-head")
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .px(px(8.))
                    .py(px(6.))
                    .rounded(px(8.))
                    .cursor_pointer()
                    .hover(|s| s.bg(Theme::hover()))
                    .on_click(self.on(Cmd::EditLibrary(lib.info.id.clone()), cx))
                    .child(div().flex().items_center().gap(px(8.)).child(div().size(px(10.)).flex_none().rounded(px(3.)).bg(Theme::accent())).child(div().flex_1().min_w_0().truncate().text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).child(lib.info.name.clone())))
            )
            .child(ui::primary_button("add-paper", "添加文献").w_full().on_click(self.on_menu(MenuKind::Add, cx)))
            .child(div().flex().flex_col().gap(px(1.)).children(view_rows))
            .child(div().flex().flex_col().gap(px(1.)).child(nav("page-ask", "sparkle", "向文献库提问", None, lib.page == Page::Ask, Cmd::SetPage(Page::Ask), self, cx)).child(nav("page-sources", "search", "检索与导入", (!lib.searches.is_empty()).then_some(lib.searches.len()), matches!(lib.page, Page::Sources | Page::Guide), Cmd::SetPage(Page::Sources), self, cx)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .gap(px(2.))
                    .child(ui::caption("标签").px(px(8.)).py(px(4.)))
                    .child(div().id("tags").flex().flex_col().flex_1().min_h_0().overflow_y_scroll().gap(px(1.)).children(tag_rows).when(tags.is_empty(), |d| d.child(div().px(px(8.)).text_size(px(12.)).text_color(Theme::text_placeholder()).child("编辑文献时添加标签"))))
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(1.))
                    .pt(px(6.))
                    .border_t_1()
                    .border_color(Theme::line())
                    .child(nav("manage-fields", "list", "自定义词条", Some(lib.fields.len()), false, Cmd::ManageFields, self, cx))
                    .child(nav("page-trash", "bin", "回收站", (trash_count > 0).then_some(trash_count), lib.page == Page::Trash, Cmd::SetPage(Page::Trash), self, cx))
                    .child(nav("open-settings", "settings-gear", "设置", None, false, Cmd::OpenSettings(SettingsPage::General), self, cx)),
            )
            .into_any_element()
    }

    fn columns(lib: &Library, width: f32) -> Vec<Column> {
        let column = |label: &str, width: f32, sort: Option<&'static str>, kind: ColumnKind| Column { label: label.to_string(), width: Some(width), sort, kind };
        let mut columns = vec![column("", 34., None, ColumnKind::Star), Column { label: "文献".into(), width: None, sort: Some("title"), kind: ColumnKind::Title }];
        // The title keeps at least 300px; optional columns join while there is room, in this order.
        let mut room = width - 34. - 300. - 32.;
        let mut optional = vec![column("阅读", 76., None, ColumnKind::Reading), column("年份", 60., Some("year"), ColumnKind::Year), column("全文", 84., None, ColumnKind::Fulltext)];
        optional.extend(lib.fields.iter().filter(|f| f.in_table).map(|f| column(&f.name, 132., None, ColumnKind::Field(f.id.clone()))));
        optional.push(column("期刊", 150., Some("journal"), ColumnKind::Journal));
        optional.push(column("收录", 84., None, ColumnKind::Screening));
        let mut chosen = vec![];
        for c in optional {
            let w = c.width.unwrap_or(0.);
            if room >= w {
                room -= w;
                chosen.push(c);
            }
        }
        // Shown in reading order rather than the order they were admitted in.
        let rank = |c: &Column| match c.kind {
            ColumnKind::Journal => 0,
            ColumnKind::Year => 1,
            ColumnKind::Field(_) => 2,
            ColumnKind::Fulltext => 3,
            ColumnKind::Reading => 4,
            ColumnKind::Screening => 5,
            _ => 0,
        };
        chosen.sort_by_key(rank);
        columns.extend(chosen);
        columns
    }

    fn render_papers(&mut self, width: f32, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let lib = self.lib.as_ref().unwrap();
        let f = &lib.filters;
        // Filter chips: the label alone when unset, "label: value" when set.
        let chip = |id: ElementId, label: &str, value: String, key: FilterKey, this: &Self, cx: &mut Context<Self>| {
            let set = !value.is_empty();
            div()
                .id(id)
                .flex()
                .flex_none()
                .items_center()
                .gap(px(4.))
                .h(px(26.))
                .px(px(9.))
                .rounded(px(13.))
                .border_1()
                .cursor_pointer()
                .text_size(px(12.))
                .when(set, |d| d.bg(Theme::accent_soft()).border_color(Theme::accent_soft()).text_color(Theme::accent()))
                .when(!set, |d| d.border_color(Theme::line_strong()).text_color(Theme::text_secondary()).hover(|s| s.bg(Theme::hover())))
                .on_click(this.on_menu(MenuKind::Filter(key), cx))
                .child(if set { format!("{label}：{value}") } else { label.to_string() })
                .child(ui::icon("chevron-down", 12., if set { Theme::accent() } else { Theme::text_tertiary() }))
        };
        let mut chips: Vec<AnyElement> = vec![
            chip("f-year".into(), "年份", f.year.clone(), FilterKey::Year, self, cx).into_any_element(),
            chip("f-reading".into(), "阅读", label(&READINGS, &f.reading).to_string(), FilterKey::Reading, self, cx).into_any_element(),
            chip("f-screening".into(), "收录", label(&SCREENINGS, &f.screening).to_string(), FilterKey::Screening, self, cx).into_any_element(),
            chip("f-fulltext".into(), "全文", label(&FULLTEXTS, &f.fulltext).to_string(), FilterKey::Fulltext, self, cx).into_any_element(),
            chip("f-tag".into(), "标签", f.tag.clone(), FilterKey::Tag, self, cx).into_any_element(),
        ];
        for (i, field) in lib.fields.iter().enumerate() {
            if field.kind == FieldKind::Text { continue }
            let value = f.fields.get(&field.id).cloned().unwrap_or_default();
            chips.push(chip(("f-field", i).into(), &field.name, value, FilterKey::Field(field.id.clone()), self, cx).into_any_element());
        }
        chips.push(chip("f-source".into(), "来源", f.source.clone(), FilterKey::Source, self, cx).into_any_element());
        chips.push(chip("f-preprint".into(), "预印本", match f.preprint.as_str() { "yes" => "只看".into(), "no" => "不含".into(), _ => String::new() }, FilterKey::Preprint, self, cx).into_any_element());
        let filtered = f.active() > 0 || !f.search.trim().is_empty();
        if filtered {
            chips.push(ui::ghost_button("f-clear", Some("x"), Some("清空筛选".into())).h(px(26.)).on_click(self.on(Cmd::ClearFilters, cx)).into_any_element());
        }

        let columns = Self::columns(lib, width);
        let (sort_key, sort_desc) = filter::sort_parts(&f.sort);
        let header: Vec<AnyElement> = columns
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let active = c.sort == Some(sort_key);
                div()
                    .id(("th", i))
                    .flex()
                    .items_center()
                    .gap(px(3.))
                    .h_full()
                    .px(px(8.))
                    .map(|d| match c.width {
                        Some(w) => d.w(px(w)).flex_none(),
                        None => d.flex_1().min_w_0(),
                    })
                    .when_some(c.sort, |d, key| d.cursor_pointer().hover(|s| s.text_color(Theme::text())).on_click(self.on(Cmd::SetSort(filter::toggle_sort(&f.sort, key)), cx)))
                    .when(active, |d| d.text_color(Theme::text()))
                    .child(c.label.clone())
                    .when(active, |d| d.child(ui::icon("chevron-down", 12., Theme::text()).when(!sort_desc, |s| s.with_transformation(Transformation::rotate(percentage(0.5))))))
                    .into_any_element()
            })
            .collect();

        let total = filter::view_count(&lib.papers, View::All, 0);
        let shown = lib.shown.len();
        let body = if shown == 0 {
            if lib.papers.is_empty() {
                ui::empty_state("library", "这个文献库还是空的", "点击左侧「添加文献」：粘贴 DOI 或 PMID、导入 BibTeX / RIS，或者直接把 PDF 拖进窗口。").into_any_element()
            } else {
                ui::empty_state("search", "没有匹配的文献", "调整搜索词或筛选条件。").child(ui::secondary_button("empty-clear", "清空筛选").on_click(self.on(Cmd::ClearFilters, cx))).into_any_element()
            }
        } else {
            uniform_list("papers", shown, cx.processor(move |this, range: Range<usize>, window, cx| this.render_rows(range, width, window, cx))).track_scroll(lib.table_scroll.clone()).flex_1().into_any_element()
        };
        let status = if lib.importing > 0 {
            format!("正在读取 {} 个 PDF…", lib.importing)
        } else if !lib.extracting.is_empty() {
            format!("正在提取 {} 篇全文的文字，用于提问…", lib.extracting.len())
        } else {
            "拖入 PDF 即可添加 · Ctrl+F 搜索 · Esc 关闭详情".to_string()
        };
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .px(px(16.))
                    .pt(px(14.))
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .items_center()
                            .gap(px(8.))
                            .h(px(34.))
                            .px(px(10.))
                            .rounded(px(8.))
                            .bg(Theme::bg_sunken())
                            .border_1()
                            .border_color(if lib.search.focus_handle(cx).is_focused(window) { Theme::accent() } else { Theme::bg_sunken() })
                            .child(ui::icon("search", 15., Theme::text_tertiary()))
                            .child(lib.search.clone()),
                    )
                    .child(ui::secondary_button("sort", "排序").child(ui::icon("sort-down", 14., Theme::text_secondary())).on_click(self.on_menu(MenuKind::Sort, cx)))
                    .child(ui::secondary_button("export", "导出").child(ui::icon("export", 14., Theme::text_secondary())).on_click(self.on_menu(MenuKind::Export, cx))),
            )
            .child(div().id("filters").flex().flex_wrap().items_center().gap(px(6.)).px(px(16.)).py(px(10.)).children(chips))
            .child(div().flex().flex_none().h(px(32.)).px(px(8.)).border_t_1().border_b_1().border_color(Theme::line()).bg(Theme::bg_sunken()).text_size(px(12.)).text_color(Theme::text_tertiary()).children(header))
            .child(body)
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_between()
                    .h(px(30.))
                    .px(px(16.))
                    .border_t_1()
                    .border_color(Theme::line())
                    .text_size(px(12.))
                    .text_color(Theme::text_tertiary())
                    .child(if filtered || shown != total { format!("显示 {shown} / {total} 篇") } else { format!("共 {total} 篇") })
                    .child(status),
            )
            .into_any_element()
    }

    fn render_rows(&mut self, range: Range<usize>, width: f32, _: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(lib) = &self.lib else { return vec![] };
        let columns = Self::columns(lib, width);
        // 设置 › 外观: the compact table still fits the title and its line of details.
        let row_height = if self.settings.density == "compact" { 44. } else { 56. };
        let muted = |text: &str| div().text_color(Theme::text_placeholder()).child(text.to_string());
        range
            .filter_map(|row| {
                let p = lib.papers.get(*lib.shown.get(row)?)?;
                let selected = lib.selected.as_deref() == Some(p.id.as_str());
                let cells: Vec<AnyElement> = columns
                    .iter()
                    .map(|c| {
                        let cell = div().flex().items_center().h_full().px(px(8.)).map(|d| match c.width {
                            Some(w) => d.w(px(w)).flex_none(),
                            None => d.flex_1().min_w_0(),
                        });
                        match &c.kind {
                            ColumnKind::Star => cell
                                .px(px(4.))
                                .child(
                                    div()
                                        .id(("star", row))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .size(px(26.))
                                        .rounded(px(6.))
                                        .cursor_pointer()
                                        .hover(|s| s.bg(Theme::hover()))
                                        .on_click({
                                            let toggle = self.on(Cmd::ToggleStar(p.id.clone()), cx);
                                            move |ev, window, cx| {
                                                cx.stop_propagation();
                                                toggle(ev, window, cx);
                                            }
                                        })
                                        .child(ui::icon("star", 15., if p.starred { Theme::star() } else { Theme::text_placeholder() })),
                                )
                                .into_any_element(),
                            ColumnKind::Title => {
                                let mut meta = vec![];
                                let authors = p.short_authors();
                                if !authors.is_empty() { meta.push(authors) }
                                if let Some(year) = p.year { meta.push(year.to_string()) }
                                if !p.journal.is_empty() { meta.push(p.journal.clone()) }
                                cell.child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .min_w_0()
                                        .w_full()
                                        .child(div().truncate().font_weight(if p.reading == "unread" { FontWeight::MEDIUM } else { FontWeight::NORMAL }).text_color(if p.reading == "unread" { Theme::text() } else { Theme::text_secondary() }).child(p.title.clone()))
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(6.))
                                                .text_size(px(12.))
                                                .line_height(px(18.))
                                                .text_color(Theme::text_tertiary())
                                                .child(div().min_w_0().truncate().child(if meta.is_empty() { "作者、年份待补".to_string() } else { meta.join(" · ") }))
                                                .when(p.publication_status == "preprint", |d| d.child(div().flex_none().text_color(theme::status_colors("preprint").0).child("预印本")))
                                                .children(p.tags.iter().take(2).map(|t| div().flex_none().text_color(Theme::accent()).child(format!("#{t}")))),
                                        ),
                                )
                                .into_any_element()
                            }
                            ColumnKind::Journal => cell.child(if p.journal.is_empty() { muted("—") } else { div().truncate().text_color(Theme::text_secondary()).child(p.journal.clone()) }).into_any_element(),
                            ColumnKind::Year => cell.child(match p.year {
                                Some(year) => div().text_color(Theme::text_secondary()).child(year.to_string()),
                                None => muted("—"),
                            })
                            .into_any_element(),
                            ColumnKind::Field(id) => cell
                                .child(match p.fields.get(id) {
                                    Some(values) => div().truncate().text_color(Theme::text_secondary()).child(values.join("、")),
                                    None => muted("—"),
                                })
                                .into_any_element(),
                            ColumnKind::Fulltext => cell.child(ui::status_chip(&p.fulltext, label(&FULLTEXTS, &p.fulltext))).into_any_element(),
                            ColumnKind::Reading => cell.child(ui::status_chip(&p.reading, label(&READINGS, &p.reading))).into_any_element(),
                            ColumnKind::Screening => cell.child(ui::status_chip(&p.screening, label(&SCREENINGS, &p.screening))).into_any_element(),
                        }
                    })
                    .collect();
                Some(
                    div()
                        .id(("row", row))
                        .flex()
                        .items_center()
                        .h(px(row_height))
                        .w(px(width))
                        .overflow_hidden()
                        .px(px(8.))
                        .border_b_1()
                        .border_color(Theme::line())
                        .cursor_pointer()
                        .when(selected, |d| d.bg(Theme::accent_soft()))
                        .when(!selected, |d| d.hover(|s| s.bg(Theme::hover())))
                        .on_click(cx.listener({
                            let id = p.id.clone();
                            move |this, ev: &ClickEvent, window, cx| {
                                // A double click opens the PDF straight away.
                                if ev.click_count() >= 2 && let Some(lib) = this.lib.as_mut() && lib.pdf_file(&id).is_some() { lib.tab = Tab::Pdf }
                                this.run(Cmd::Select(id.clone()), window, cx);
                                this.run(Cmd::SetTab(this.lib.as_ref().map_or(Tab::Info, |l| l.tab)), window, cx);
                            }
                        }))
                        .children(cells)
                        .into_any_element(),
                )
            })
            .collect()
    }

    fn render_trash(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let lib = self.lib.as_ref().unwrap();
        let rows: Vec<AnyElement> = lib
            .trash
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let p = &entry.paper;
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .px(px(16.))
                    .py(px(10.))
                    .border_b_1()
                    .border_color(Theme::line())
                    .child(div().flex().flex_col().flex_1().min_w_0().child(div().truncate().child(p.title.clone())).child(div().truncate().text_size(px(12.)).text_color(Theme::text_tertiary()).child(format!("{} · 删除于 {}", if p.authors.is_empty() { "作者待补".to_string() } else { p.short_authors() }, time::date_text(&entry.deleted_at)))))
                    .child(ui::secondary_button(("restore", i), "恢复").on_click(self.on(Cmd::RestorePaper(p.id.clone()), cx)))
                    .child(ui::ghost_button(("delete", i), Some("bin"), Some("彻底删除".into())).text_color(Theme::danger()).on_click(self.on(Cmd::AskDeletePaper(p.id.clone()), cx)))
                    .into_any_element()
            })
            .collect();
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(div().px(px(16.)).pt(px(16.)).pb(px(10.)).child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).child("回收站")).child(div().text_size(px(12.)).text_color(Theme::text_tertiary()).child("这里的文献保留着笔记、词条和 PDF，可以恢复。它们仍会阻止同一篇文献被重复导入。")))
            .child(if rows.is_empty() { ui::empty_state("bin", "回收站是空的", "").into_any_element() } else { div().id("trash").flex().flex_col().flex_1().min_h_0().overflow_y_scroll().border_t_1().border_color(Theme::line()).children(rows).into_any_element() })
            .into_any_element()
    }
}
