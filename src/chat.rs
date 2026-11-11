//! 提问: the conversation with the library (the current list) or with the
//! selected paper. The answer cites labels like [P3 p.5]; the chips under
//! it open the paper, or its PDF at that page.

use crate::app::{Cmd, Workspace};
use crate::settings::SettingsPage;
use crate::theme::Theme;
use crate::ui;
use gpui::{prelude::FluentBuilder, *};
use wisp_core::ai;

impl Workspace {
    pub fn render_chat(&mut self, about_paper: bool, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let lib = self.lib.as_ref().unwrap();
        let chat = if about_paper { &lib.paper_chat } else { &lib.library_chat };
        let profile = self.settings.profile_or_default();
        let configured = !profile.key().is_empty();
        let model = profile.model.clone();

        // What the model has been given, or will be given with the first question.
        let scope = match &chat.ctx {
            Some(ctx) if about_paper => {
                let mut line = if ctx.pages > 0 { format!("已提供这篇文献的记录和 {} 页全文", ctx.pages) } else { "这篇文献没有可读取的全文，只提供了记录和摘要".to_string() };
                if ctx.pages_omitted > 0 { line.push_str(&format!("（另有 {} 页超出长度未提供）", ctx.pages_omitted)) }
                line
            }
            Some(ctx) => {
                let mut line = format!("已提供 {} 篇文献的记录", ctx.full);
                if ctx.titles_only > 0 { line.push_str(&format!("，{} 篇仅题目", ctx.titles_only)) }
                if ctx.omitted > 0 { line.push_str(&format!("，{} 篇未提供", ctx.omitted)) }
                if ctx.pages > 0 { line.push_str(&format!("，{} 页相关全文", ctx.pages)) }
                line
            }
            None if about_paper => {
                let id = lib.selected.clone().unwrap_or_default();
                match lib.pdf_file(&id).map(|f| lib.store.fulltext_pages(&f.id)) {
                    Some(Ok(Some(Ok(pages)))) => format!("将提供这篇文献的记录和 {} 页全文", pages.len()),
                    Some(Ok(Some(Err(error)))) => format!("全文无法读取（{error}），将只提供记录和摘要"),
                    Some(_) => "全文的文字还在提取，现在提问只会用到记录和摘要".to_string(),
                    None => "这篇文献没有 PDF，将只提供记录和摘要".to_string(),
                }
            }
            None => format!("范围：当前列表的 {} 篇文献（受左侧视图、搜索和筛选影响），以及其中已存 PDF 的相关页面", lib.shown.len()),
        };

        let mut items: Vec<AnyElement> = vec![];
        for (i, message) in chat.messages.iter().enumerate() {
            if message.from_user {
                items.push(div().flex().justify_end().child(div().max_w(relative(0.85)).px(px(12.)).py(px(8.)).rounded(px(12.)).bg(Theme::accent_soft()).child(message.content.clone())).into_any_element());
                continue;
            }
            let waiting = message.content.is_empty();
            // Each cited label, once, in the order the answer uses it.
            let chips: Vec<AnyElement> = chat
                .ctx
                .iter()
                .flat_map(|ctx| ai::citations(&message.content).into_iter().filter_map(move |(label, page)| ctx.sources.iter().find(|s| s.label == label).map(|s| (s.clone(), page))))
                .enumerate()
                .map(|(n, (source, page))| {
                    let text = match page {
                        Some(page) => format!("{} p.{page} · {}", source.label, source.title),
                        None => format!("{} · {}", source.label, source.title),
                    };
                    div()
                        .id(SharedString::from(format!("cite-{i}-{n}")))
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .max_w(px(300.))
                        .h(px(24.))
                        .px(px(8.))
                        .rounded(px(6.))
                        .border_1()
                        .border_color(Theme::line())
                        .text_size(px(12.))
                        .text_color(Theme::text_secondary())
                        .cursor_pointer()
                        .hover(|s| s.bg(Theme::hover()).border_color(Theme::accent()))
                        .on_click(self.on(Cmd::GotoSource(source.paper_id.clone(), page), cx))
                        .child(ui::icon("file-text", 12., Theme::accent()))
                        .child(div().min_w_0().truncate().child(text))
                        .into_any_element()
                })
                .collect();
            items.push(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(div().line_height(px(22.)).text_color(if waiting { Theme::text_tertiary() } else { Theme::text() }).child(if waiting { "正在阅读文献并思考…".to_string() } else { message.content.clone() }))
                    .when(!chips.is_empty(), |d| d.child(div().flex().flex_wrap().gap(px(6.)).children(chips)))
                    .into_any_element(),
            );
        }
        if !chat.note.is_empty() {
            items.push(div().text_size(px(12.)).text_color(Theme::text_tertiary()).child(chat.note.clone()).into_any_element());
        }
        if !chat.error.is_empty() {
            items.push(ui::error_line(&chat.error).into_any_element());
        }

        let empty = chat.messages.is_empty() && chat.error.is_empty();
        let body = if !configured {
            ui::empty_state("sparkle", "还没有配置模型", "提问需要一个模型服务：Claude API，或任何 OpenAI 兼容的服务。API Key 只保存在本机。").child(ui::primary_button(if about_paper { "chat-settings-paper" } else { "chat-settings" }, "打开设置").on_click(self.on(Cmd::OpenSettings(SettingsPage::Ai), cx))).into_any_element()
        } else if empty {
            let suggestions: &[&str] = if about_paper { &["这篇文献的研究问题、方法和主要结论是什么？", "作者的结论有哪些证据支持？有什么局限？", "用三句话向同行介绍这篇文章。"] } else { &["这些文献主要在研究哪些问题？按主题归类。", "哪些文献的结论相互支持或矛盾？", "我还没读的文献里，哪几篇最值得先读？为什么？"] };
            ui::empty_state("sparkle", if about_paper { "就这篇文献提问" } else { "向文献库提问" }, "回答依据文献库里的记录、笔记和已存 PDF 的全文，并标出出处。")
                .children(suggestions.iter().enumerate().map(|(i, text)| ui::secondary_button(SharedString::from(format!("suggest-{about_paper}-{i}")), *text).h(px(30.)).on_click(self.on(Cmd::Suggest(about_paper, text.to_string()), cx))))
                .into_any_element()
        } else {
            div().id(if about_paper { "chat-paper" } else { "chat-library" }).flex().flex_col().flex_1().min_h_0().overflow_y_scroll().track_scroll(&chat.scroll).px(px(16.)).py(px(14.)).gap(px(16.)).children(items).into_any_element()
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
                    .px(px(16.))
                    .py(px(8.))
                    .border_b_1()
                    .border_color(Theme::line())
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .when(!about_paper, |d| d.child(div().text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).child("向文献库提问")))
                            .child(div().text_size(px(12.)).line_height(px(17.)).text_color(Theme::text_tertiary()).child(scope)),
                    )
                    .when(!chat.messages.is_empty(), |d| d.child(ui::ghost_button(if about_paper { "chat-new-paper" } else { "chat-new" }, Some("plus"), Some("新对话".into())).on_click(self.on(Cmd::NewChat(about_paper), cx)))),
            )
            .child(body)
            .when(configured, |d| {
                d.child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_none()
                        .gap(px(4.))
                        .px(px(16.))
                        .pb(px(12.))
                        .pt(px(8.))
                        .child(
                            div()
                                .flex()
                                .items_end()
                                .gap(px(8.))
                                .child(ui::field(&chat.input, window, cx).flex_1().max_h(px(160.)).overflow_hidden())
                                .child(ui::primary_button(if about_paper { "chat-send-paper" } else { "chat-send" }, "").w(px(34.)).px(px(0.)).when(chat.busy, |d| d.opacity(0.5)).child(ui::icon("send", 16., Theme::on_accent())).on_click(self.on(Cmd::Ask(about_paper), cx))),
                        )
                        .child(div().text_size(px(11.)).text_color(Theme::text_placeholder()).child(format!("{model} · Enter 发送，Shift+Enter 换行 · 回答可能有误，请对照原文"))),
                )
            })
            .into_any_element()
    }
}
