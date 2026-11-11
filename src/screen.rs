//! 筛选: the papers waiting for a decision. The assistant reads each one and
//! suggests; the person decides, one paper at a time, with the paper's
//! record and PDF open beside the list.

use crate::app::{Cmd, Workspace};
use crate::settings::SettingsPage;
use crate::theme::Theme;
use crate::ui;
use gpui::{prelude::FluentBuilder, *};
use wisp_core::model::{Paper, SCREENINGS, label};
use wisp_core::store::Suggestion;

/// The rows drawn at most: a longer queue is worked off from the top.
const SHOWN: usize = 200;

impl Workspace {
    pub fn render_screen(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let lib = self.lib.as_ref().unwrap();
        let screen = &lib.screen;
        let queue = lib.screen_queue();
        let suggested = queue.iter().filter(|p| lib.suggestions.contains_key(&p.id)).count();
        let waiting = queue.len() - suggested;
        let configured = !self.settings.profile_or_default().key().is_empty();
        let status = if screen.running() {
            format!("助手正在判断：{} / {}", screen.done, screen.total)
        } else if queue.is_empty() {
            "没有待判断的文献。".to_string()
        } else {
            format!("待判断 {} 篇，其中 {suggested} 篇已有助手的建议。", queue.len())
        };
        let rows: Vec<AnyElement> = queue.iter().take(SHOWN).enumerate().map(|(i, paper)| self.render_screen_row(i, paper, lib.suggestions.get(&paper.id), lib.selected.as_deref() == Some(paper.id.as_str()), cx)).collect();

        div()
            .flex()
            .flex_col()
            .size_full()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_none()
                    .gap(px(10.))
                    .px(px(16.))
                    .pt(px(16.))
                    .pb(px(12.))
                    .child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).child("筛选"))
                    .child(div().text_size(px(12.)).text_color(Theme::text_tertiary()).child("助手逐篇阅读待判断的文献（有全文读全文，否则读摘要），给出建议、理由和原文依据。建议只是建议：每一篇的收录判断由你确认。"))
                    .child(ui::field(&screen.criteria, window, cx))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .child(div().flex_1().min_w_0().text_size(px(12.)).text_color(Theme::text_secondary()).child(status))
                            .when(!configured, |d| d.child(ui::ghost_button("screen-settings", Some("settings-gear"), Some("先配置模型".into())).on_click(self.on(Cmd::OpenSettings(SettingsPage::Ai), cx))))
                            .child(if screen.running() {
                                ui::secondary_button("screen-stop", "停止").on_click(self.on(Cmd::ScreenStop, cx))
                            } else {
                                ui::primary_button("screen-start", if waiting == 0 { "让助手判断".to_string() } else { format!("让助手判断 {waiting} 篇") }).when(waiting == 0, |d| d.opacity(0.5)).on_click(self.on(Cmd::ScreenStart, cx))
                            }),
                    )
                    .when(!screen.error.is_empty(), |d| d.child(ui::error_line(&screen.error))),
            )
            .child(if rows.is_empty() {
                ui::empty_state("check", "都判断过了", "新导入的文献会出现在这里。").into_any_element()
            } else {
                div()
                    .id("screen-queue")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .border_t_1()
                    .border_color(Theme::line())
                    .children(rows)
                    .when(queue.len() > SHOWN, |d| d.child(div().px(px(16.)).py(px(10.)).text_size(px(12.)).text_color(Theme::text_tertiary()).child(format!("还有 {} 篇，判断完上面的会继续出现。", queue.len() - SHOWN))))
                    .into_any_element()
            })
            .into_any_element()
    }

    /// One paper of the queue. The selected one opens up: what the
    /// assistant found, and the four decisions.
    fn render_screen_row(&self, index: usize, paper: &Paper, suggestion: Option<&Suggestion>, selected: bool, cx: &mut Context<Self>) -> AnyElement {
        let mut about: Vec<String> = vec![paper.short_authors(), paper.year.map(|y| y.to_string()).unwrap_or_default(), paper.journal.clone()];
        about.retain(|part| !part.is_empty());
        let chip = suggestion.map(|s| {
            let sure = s.confidence.map(|c| format!(" · 把握 {:.0}%", c * 100.)).unwrap_or_default();
            ui::status_chip(&s.verdict, &format!("建议：{}{sure}", label(&SCREENINGS, &s.verdict)))
        });
        let head = div()
            .flex()
            .items_center()
            .gap(px(10.))
            .child(div().flex().flex_col().flex_1().min_w_0().child(div().truncate().font_weight(if selected { FontWeight::SEMIBOLD } else { FontWeight::NORMAL }).child(paper.title.clone())).child(div().truncate().text_size(px(12.)).text_color(Theme::text_tertiary()).child(about.join(" · "))))
            .children(chip);
        let mut row = div()
            .id(("screen-row", index))
            .flex()
            .flex_col()
            .gap(px(8.))
            .px(px(16.))
            .py(px(10.))
            .border_b_1()
            .border_color(Theme::line())
            .when(selected, |d| d.bg(Theme::accent_soft()))
            .when(!selected, |d| d.cursor_pointer().hover(|s| s.bg(Theme::hover())).on_click(self.on(Cmd::Select(paper.id.clone()), cx)))
            .child(head);
        if !selected { return row.into_any_element() }

        if let Some(s) = suggestion {
            let read = match s.basis.as_str() {
                "fulltext" => "读了全文",
                "abstract" => "只读了摘要",
                _ => "没有可读的内容",
            };
            row = row.child(div().text_size(px(13.)).line_height(px(20.)).child(s.reason.clone()));
            if !s.excerpt.is_empty() {
                // The words the suggestion rests on; with a page, a click opens the PDF there.
                let quote = div().id(("screen-quote", index)).flex().gap(px(8.)).px(px(10.)).py(px(8.)).rounded(px(6.)).bg(Theme::bg_body()).border_l_2().border_color(Theme::accent()).text_size(px(12.)).line_height(px(18.)).text_color(Theme::text_secondary()).child(div().flex_1().min_w_0().child(s.excerpt.clone()));
                row = row.child(match s.page {
                    Some(page) => quote.cursor_pointer().child(div().flex_none().text_color(Theme::accent()).child(format!("第 {page} 页"))).on_click(self.on(Cmd::GotoSource(paper.id.clone(), Some(page), s.excerpt.clone()), cx)),
                    None => quote.child(div().flex_none().text_color(Theme::text_tertiary()).child("摘要")),
                });
            }
            row = row.child(div().text_size(px(11.)).text_color(Theme::text_tertiary()).child(format!("{read} · {}", s.author)));
        } else {
            row = row.child(div().text_size(px(12.)).text_color(Theme::text_tertiary()).child("助手还没有判断这一篇。"));
        }
        // The decision: every choice is there, the suggested one stands out.
        let buttons = SCREENINGS.iter().filter(|(key, _)| *key != "pending").enumerate().map(|(n, (key, name))| {
            let agree = suggestion.is_some_and(|s| s.verdict == *key);
            let button = if agree { ui::primary_button(("decide", n), format!("采纳：{name}")) } else { ui::secondary_button(("decide", n), *name) };
            button.on_click(self.on(Cmd::ScreenDecide(paper.id.clone(), key.to_string()), cx))
        });
        row.child(div().flex().items_center().gap(px(8.)).pt(px(2.)).children(buttons)).into_any_element()
    }
}
