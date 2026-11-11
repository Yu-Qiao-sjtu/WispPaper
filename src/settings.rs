//! 设置: a view of its own, with a sidebar of pages (通用, 外观, 快捷键, AI,
//! 关于) and 返回 at the bottom. The library that was open stays open
//! underneath. Every change is saved at once; there is no 保存 button.
//!
//! The layout follows quiet-md's settings (MIT): grouped rows in rounded
//! cards, the title on the left and the control on the right.

use crate::app::{Cmd, Workspace, default_root};
use crate::input::{InputEvent, TextInput};
use crate::shortcuts::{self, GROUPS, SHORTCUTS};
use crate::theme::{self, Theme};
use crate::ui;
use gpui::{prelude::FluentBuilder, *};
use std::path::PathBuf;
use std::sync::{Arc, LazyLock};
use std::time::Duration;
use wisp_core::ai::{self, Profile, Protocol};
use wisp_core::libraries;
use wisp_core::update::{self, Release};

const SIDEBAR_W: f32 = 224.;
const COLUMN_W: f32 = 640.;
const INPUT_W: f32 = 320.;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SettingsPage {
    #[default]
    General,
    Appearance,
    Shortcuts,
    Ai,
    About,
}

impl SettingsPage {
    const ALL: [SettingsPage; 5] = [SettingsPage::General, SettingsPage::Appearance, SettingsPage::Shortcuts, SettingsPage::Ai, SettingsPage::About];

    fn title(self) -> &'static str {
        match self {
            SettingsPage::General => "通用",
            SettingsPage::Appearance => "外观",
            SettingsPage::Shortcuts => "快捷键",
            SettingsPage::Ai => "AI",
            SettingsPage::About => "关于",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            SettingsPage::General => "sliders",
            SettingsPage::Appearance => "palette",
            SettingsPage::Shortcuts => "keyboard",
            SettingsPage::Ai => "sparkle",
            SettingsPage::About => "info",
        }
    }
}

/// The open settings view.
pub struct SettingsView {
    pub page: SettingsPage,
    scroll: ScrollHandle,
    protocol: Protocol,
    base_url: Entity<TextInput>,
    model: Entity<TextInput>,
    api_key: Entity<TextInput>,
    /// The shortcut waiting for its new key. While it waits, every key
    /// binding is cleared, so the keys pressed reach `record_key`.
    recording: Option<String>,
    /// A shortcut and why the key pressed for it was refused.
    shortcut_error: Option<(String, String)>,
    _subscriptions: Vec<Subscription>,
}

/// Where the check for a newer release stands.
#[derive(Clone, Debug, PartialEq)]
pub enum UpdateState {
    Idle,
    Checking,
    Current,
    Available(Release),
    Failed(String),
}

// ---- controls

/// Rows in one rounded card split by hairlines, under an optional caption.
fn group(title: Option<&'static str>, rows: Vec<AnyElement>) -> Div {
    let count = rows.len();
    div()
        .flex()
        .flex_col()
        .w_full()
        .gap(px(8.))
        .children(title.map(|t| div().px(px(4.)).font_weight(FontWeight::SEMIBOLD).text_color(Theme::text_secondary()).child(t)))
        .child(div().flex().flex_col().w_full().rounded(px(12.)).border_1().border_color(Theme::line()).overflow_hidden().children(rows.into_iter().enumerate().map(move |(i, row)| div().flex().flex_col().w_full().when(i + 1 < count, |d| d.border_b_1().border_color(Theme::line())).child(row))))
}

/// One setting: its name and an optional explanation on the left, the control on the right.
fn row(title: impl Into<SharedString>, detail: Option<SharedString>, control: impl IntoElement) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(24.))
        .w_full()
        .min_h(px(48.))
        .px(px(16.))
        .py(px(10.))
        .child(div().flex().flex_col().flex_1().min_w_0().gap(px(2.)).child(div().child(title.into())).children(detail.map(|d| div().text_size(px(12.)).line_height(px(17.)).text_color(Theme::text_tertiary()).child(d))))
        .child(div().flex().flex_none().items_center().gap(px(8.)).child(control))
}

/// The small print under a group.
fn note(text: &'static str) -> Div {
    div().px(px(4.)).text_size(px(12.)).line_height(px(18.)).text_color(Theme::text_tertiary()).child(text)
}

fn switch(id: &'static str, on: bool) -> Stateful<Div> {
    div().id(id).flex().flex_none().items_center().w(px(34.)).h(px(20.)).p(px(2.)).rounded_full().cursor_pointer().when(on, |d| d.bg(Theme::accent()).justify_end()).when(!on, |d| d.bg(Theme::groove()).justify_start()).child(div().size(px(16.)).rounded_full().bg(gpui::white()).shadow(theme::shadow_card()))
}

/// A choice among a few: a groove with the chosen one raised.
fn segmented(id: &'static str, choices: Vec<(&'static str, Cmd)>, chosen: usize, this: &Workspace, cx: &mut Context<Workspace>) -> Div {
    div().flex().flex_none().gap(px(2.)).h(px(28.)).p(px(2.)).rounded(px(9.)).bg(Theme::groove()).children(choices.into_iter().enumerate().map(|(i, (label, cmd))| {
        div()
            .id((id, i))
            .flex()
            .items_center()
            .justify_center()
            .h_full()
            .px(px(12.))
            .rounded(px(7.))
            .cursor_pointer()
            .when(i == chosen, |d| d.bg(Theme::segment()).shadow(theme::shadow_card()))
            .when(i != chosen, |d| d.text_color(Theme::text_secondary()).hover(|s| s.text_color(Theme::text())))
            .on_click(this.on(cmd, cx))
            .child(label)
    }))
}

fn button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Stateful<Div> {
    ui::secondary_button(id, label).h(px(28.))
}

/// A text button in the accent (恢复默认 …).
fn link_button(id: impl Into<ElementId>, label: &'static str) -> Stateful<Div> {
    div().id(id).flex().flex_none().items_center().h(px(26.)).px(px(8.)).rounded(px(7.)).cursor_pointer().text_size(px(12.)).text_color(Theme::accent()).hover(|s| s.bg(Theme::hover())).child(label)
}

/// A shortcut's key; `recording` shows it waiting for the new one.
fn keycap(id: impl Into<ElementId>, key: &str, recording: bool) -> Stateful<Div> {
    let text = if recording { "按下新的快捷键…".to_string() } else if key.is_empty() { "未设置".to_string() } else { shortcuts::pretty(key) };
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .min_w(px(148.))
        .h(px(28.))
        .px(px(12.))
        .rounded(px(7.))
        .border_1()
        .cursor_pointer()
        .text_size(px(12.))
        .when(recording, |d| d.border_color(Theme::accent()).text_color(Theme::accent()))
        .when(!recording, |d| d.border_color(gpui::transparent_black()).bg(Theme::hover()).hover(|s| s.bg(Theme::pressed())).text_color(if key.is_empty() { Theme::text_tertiary() } else { Theme::text() }))
        .child(text)
}

/// The app icon resampled once to 176px, its size at 88pt on a 2x screen:
/// GPUI samples images bilinearly without mipmaps, so shrinking the large
/// file on the GPU leaves jagged edges.
fn about_icon() -> ImageSource {
    const PATH: &str = "branding/app-icon.png";
    static ICON: LazyLock<Option<Arc<RenderImage>>> = LazyLock::new(|| {
        let bytes = crate::assets::Assets.load(PATH).ok()??;
        let mut small = image::imageops::resize(&image::load_from_memory(&bytes).ok()?.into_rgba8(), 176, 176, image::imageops::FilterType::Lanczos3);
        // GPUI wants BGRA.
        for pixel in small.pixels_mut() {
            pixel.0.swap(0, 2);
        }
        Some(Arc::new(RenderImage::new(vec![image::Frame::new(small)])))
    });
    match &*ICON {
        Some(icon) => icon.clone().into(),
        None => PATH.into(),
    }
}

impl Workspace {
    // ---- opening and closing

    pub fn open_settings(&mut self, page: SettingsPage, window: &mut Window, cx: &mut Context<Self>) {
        self.modal = None;
        self.modal_subscriptions.clear();
        match self.settings_view.as_mut() {
            Some(view) => {
                if view.page != page { view.scroll = ScrollHandle::new() }
                view.page = page;
                view.shortcut_error = None;
            }
            None => {
                let profile = self.settings.profile_or_default();
                let mut input = |placeholder: &'static str, text: &str| cx.new(|cx| TextInput::new(placeholder, cx).with_text(text));
                let base_url = input("https://…", &profile.base_url);
                let model = input("模型名称", &profile.model);
                let api_key = input("留空则读取环境变量", &profile.api_key);
                api_key.update(cx, |input, _| input.masked = true);
                let subscriptions = [&base_url, &model, &api_key]
                    .into_iter()
                    .map(|input| {
                        cx.subscribe(input, |this, _, event: &InputEvent, cx| {
                            if *event == InputEvent::Changed { this.save_profile(cx) }
                        })
                    })
                    .collect();
                self.settings_view = Some(SettingsView { page, scroll: ScrollHandle::new(), protocol: profile.protocol, base_url, model, api_key, recording: None, shortcut_error: None, _subscriptions: subscriptions });
            }
        }
        window.focus(&self.focus);
        cx.notify();
    }

    /// Back to where 设置 was opened from.
    pub fn close_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.stop_recording(cx);
        self.settings_view = None;
        window.focus(&self.focus);
        cx.notify();
    }

    // ---- 通用

    pub fn pick_data_dir(&mut self, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: false, prompt: Some("选择".into()), extensions: None });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = picked.await && let Some(dir) = paths.into_iter().next() {
                this.update(cx, |this, cx| this.set_data_dir(Some(dir), cx)).ok();
            }
        })
        .detach();
    }

    /// Keep the libraries in `dir`; `None` goes back to the default folder.
    /// Nothing is moved: the home page shows what the new folder holds.
    pub fn set_data_dir(&mut self, dir: Option<PathBuf>, cx: &mut Context<Self>) {
        let default = default_root();
        let root = dir.unwrap_or_else(|| default.clone());
        if root == self.root { return }
        if let Err(err) = std::fs::create_dir_all(&root) {
            return self.fail(format!("无法使用这个目录：{err}"), cx);
        }
        // The default folder is stored as "unset", so it follows the user's Documents folder.
        self.settings.data_dir = if root == default { String::new() } else { root.to_string_lossy().to_string() };
        self.root = root;
        // The open library belongs to the old folder.
        self.lib = None;
        theme::set_accent("blue");
        self.libraries = libraries::list(&self.root);
        self.save_settings(cx);
        self.say(format!("数据目录已改为 {}", self.root.display()), cx);
    }

    // ---- 更新

    /// Start the automatic check: shortly after launch, then twice a day, while 自动检查更新 is on.
    pub fn watch_updates(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(10)).await;
            loop {
                let alive = this.update(cx, |this, cx| {
                    if this.settings.auto_update { this.check_update(false, cx) }
                });
                if alive.is_err() { break }
                cx.background_executor().timer(Duration::from_secs(12 * 3600)).await;
            }
        })
        .detach();
    }

    /// Ask which release is the newest. `manual` (检查更新) also reports a
    /// failure; the automatic check stays silent except for one toast per
    /// new version.
    pub fn check_update(&mut self, manual: bool, cx: &mut Context<Self>) {
        if self.update == UpdateState::Checking { return }
        if manual {
            self.update = UpdateState::Checking;
            cx.notify();
        }
        // WISP_PAPER_UPDATE_URL points the check elsewhere (testing).
        let url = std::env::var("WISP_PAPER_UPDATE_URL").ok().filter(|u| !u.trim().is_empty()).unwrap_or_else(update::latest_url);
        cx.spawn(async move |this, cx| {
            let found = cx.background_executor().spawn(async move { wisp_net::latest_release(&url) }).await;
            this.update(cx, |this, cx| {
                match found {
                    Ok(Some(release)) if update::newer(&release.version, env!("CARGO_PKG_VERSION")) => {
                        if !manual && this.update_told.as_ref() != Some(&release.version) {
                            this.say(format!("Wisp Paper {} 已发布，可在「设置 › 关于」前往下载。", release.version), cx);
                        }
                        this.update_told = Some(release.version.clone());
                        this.update = UpdateState::Available(release);
                    }
                    Ok(_) => this.update = UpdateState::Current,
                    Err(err) if manual => this.update = UpdateState::Failed(err),
                    // A quiet retry at the next check; keep whatever was shown.
                    Err(_) => {}
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Where the check stands and the one next step.
    fn update_row(&self, cx: &mut Context<Self>) -> AnyElement {
        let current = format!("当前版本 {}", env!("CARGO_PKG_VERSION"));
        let check = |this: &Self, cx: &mut Context<Self>| Some(button("update-check", "检查更新").on_click(this.on(Cmd::CheckUpdate, cx)));
        let (title, detail, control): (String, Option<String>, Option<Stateful<Div>>) = match &self.update {
            UpdateState::Idle => ("软件更新".into(), Some(current), check(self, cx)),
            UpdateState::Checking => ("正在检查更新…".into(), Some(current), None),
            UpdateState::Current => ("已是最新版本".into(), Some(current), check(self, cx)),
            UpdateState::Available(release) => (format!("新版本 {} 可用", release.version), Some(format!("{current}。在发布页下载新版本后覆盖安装，文献库和设置不受影响。")), Some(button("update-open", "前往下载").on_click(self.on(Cmd::OpenUrl(release.url.clone()), cx)))),
            UpdateState::Failed(err) => ("检查更新失败".into(), Some(err.clone()), check(self, cx)),
        };
        row(title, detail.map(Into::into), div().children(control)).into_any_element()
    }

    // ---- 外观

    /// Use the light or the dark colours: the chosen one, else the system's.
    pub fn apply_appearance(&mut self, window: &mut Window) {
        let dark = match self.settings.appearance.as_str() {
            "light" => false,
            "dark" => true,
            _ => matches!(window.appearance(), WindowAppearance::Dark | WindowAppearance::VibrantDark),
        };
        theme::set_dark(dark);
        window.refresh();
    }

    // ---- 快捷键

    pub fn start_recording(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(view) = self.settings_view.as_mut() else { return };
        view.recording = Some(id);
        view.shortcut_error = None;
        // The keys go to the workspace, and none of them runs anything meanwhile.
        window.focus(&self.focus);
        cx.clear_key_bindings();
        cx.notify();
    }

    /// Stop waiting for a key and put the key bindings back.
    pub fn stop_recording(&mut self, cx: &mut Context<Self>) {
        if self.settings_view.as_mut().is_some_and(|view| view.recording.take().is_some()) {
            shortcuts::rebuild(&self.settings.shortcuts, cx);
            cx.notify();
        }
    }

    /// A key pressed while a shortcut waits for its new key.
    pub fn record_key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.settings_view.as_ref().and_then(|view| view.recording.clone()) else { return };
        cx.stop_propagation();
        let stroke = &event.keystroke;
        if event.is_held || matches!(stroke.key.as_str(), "" | "shift" | "alt" | "ctrl" | "control" | "cmd" | "win" | "super" | "platform" | "fn" | "function") { return }
        if stroke.key == "escape" && !stroke.modifiers.modified() { return self.stop_recording(cx) }
        let key = stroke.unparse();
        match shortcuts::refusal(&id, &key, &self.settings.shortcuts) {
            Some(why) => {
                if let Some(view) = self.settings_view.as_mut() { view.shortcut_error = Some((id, why)) }
            }
            None => self.set_shortcut(&id, Some(key), cx),
        }
        cx.notify();
    }

    /// Give the shortcut a key (an empty one removes it); `None` restores its default.
    pub fn set_shortcut(&mut self, id: &str, key: Option<String>, cx: &mut Context<Self>) {
        match key {
            Some(key) if !shortcuts::is_default(id, &key) => {
                self.settings.shortcuts.insert(id.to_string(), key);
            }
            _ => {
                self.settings.shortcuts.remove(id);
            }
        }
        // A restored default may now collide with a key given to another shortcut meanwhile.
        let key = shortcuts::key_of(id, &self.settings.shortcuts);
        let clash = if key.is_empty() { None } else { shortcuts::refusal(id, &key, &self.settings.shortcuts) };
        if let Some(view) = self.settings_view.as_mut() {
            view.recording = None;
            view.shortcut_error = clash.map(|why| (id.to_string(), why));
        }
        self.save_settings(cx);
        shortcuts::rebuild(&self.settings.shortcuts, cx);
        cx.notify();
    }

    pub fn reset_shortcuts(&mut self, cx: &mut Context<Self>) {
        self.settings.shortcuts.clear();
        if let Some(view) = self.settings_view.as_mut() {
            view.recording = None;
            view.shortcut_error = None;
        }
        self.save_settings(cx);
        shortcuts::rebuild(&self.settings.shortcuts, cx);
        cx.notify();
    }

    // ---- AI

    pub fn pick_protocol(&mut self, protocol: Protocol, cx: &mut Context<Self>) {
        let Some(view) = self.settings_view.as_mut() else { return };
        if view.protocol == protocol { return }
        view.protocol = protocol;
        // The other protocol's address and model would only fail: offer this one's defaults.
        let (url, model) = match protocol {
            Protocol::Anthropic => (ai::ANTHROPIC_BASE, ai::DEFAULT_MODEL),
            Protocol::OpenAI => ("https://api.openai.com/v1", ""),
        };
        view.base_url.update(cx, |input, cx| input.set_text(url, cx));
        view.model.update(cx, |input, cx| input.set_text(model, cx));
        self.save_profile(cx);
    }

    /// Keep what the AI page shows as the profile questions go to.
    fn save_profile(&mut self, cx: &mut Context<Self>) {
        let Some(view) = &self.settings_view else { return };
        let text = |input: &Entity<TextInput>| input.read(cx).text().trim().to_string();
        let profile = Profile { name: self.settings.profile().map_or("默认".into(), |p| p.name.clone()), protocol: view.protocol, base_url: text(&view.base_url), api_key: text(&view.api_key), model: text(&view.model) };
        if self.settings.profile() == Some(&profile) { return }
        self.settings.profiles = vec![profile];
        self.settings.active_profile = 0;
        self.save_settings(cx);
        cx.notify();
    }

    // ---- drawing

    pub fn render_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(view) = &self.settings_view else { return div().into_any_element() };
        let page = view.page;
        let nav = |id: &'static str, icon: &'static str, label: &'static str, active: bool, cmd: Cmd, this: &Self, cx: &mut Context<Self>| {
            div()
                .id(id)
                .flex()
                .items_center()
                .gap(px(9.))
                .h(px(32.))
                .px(px(8.))
                .rounded(px(7.))
                .cursor_pointer()
                .when(active, |d| d.bg(Theme::pressed()).font_weight(FontWeight::MEDIUM))
                .when(!active, |d| d.text_color(Theme::text_secondary()).hover(|s| s.bg(Theme::hover())))
                .on_click(this.on(cmd, cx))
                .child(ui::icon(icon, 16., if active { Theme::text() } else { Theme::text_secondary() }))
                .child(label)
        };
        let pages: Vec<AnyElement> = SettingsPage::ALL.into_iter().map(|p| nav(p.icon(), p.icon(), p.title(), p == page, Cmd::OpenSettings(p), self, cx).into_any_element()).collect();
        let sidebar = div()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(SIDEBAR_W))
            .h_full()
            .px(px(10.))
            .pb(px(10.))
            .pt(px(if ui::custom_titlebar() { 4. } else { 10. }))
            .gap(px(2.))
            .child(ui::title_strip())
            .child(div().px(px(8.)).pt(px(6.)).pb(px(8.)).font_weight(FontWeight::SEMIBOLD).text_color(Theme::text_secondary()).child("设置"))
            .children(pages)
            .child(div().flex_1())
            .child(nav("settings-back", "arrow-left", "返回", false, Cmd::CloseSettings, self, cx));

        let body = match page {
            SettingsPage::General => self.general_page(cx),
            SettingsPage::Appearance => self.appearance_page(cx),
            SettingsPage::Shortcuts => self.shortcuts_page(cx),
            SettingsPage::Ai => self.ai_page(window, cx),
            SettingsPage::About => self.about_page(cx),
        };
        let changed = page == SettingsPage::Shortcuts && !self.settings.shortcuts.is_empty();
        let heading = div()
            .flex()
            .items_center()
            .gap(px(16.))
            .child(div().flex_1().min_w_0().px(px(4.)).text_size(px(22.)).line_height(px(28.)).font_weight(FontWeight::SEMIBOLD).child(page.title()))
            .when(changed, |d| d.child(link_button("shortcuts-reset", "全部恢复默认").on_click(self.on(Cmd::ResetShortcuts, cx))));
        let content = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .h_full()
            .bg(Theme::bg_body())
            .border_l_1()
            .border_color(Theme::line())
            .child(ui::title_strip())
            .child(
                div()
                    .id("settings-content")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&view.scroll)
                    .child(div().flex().flex_col().items_center().w_full().px(px(32.)).pt(px(12.)).pb(px(64.)).child(div().flex().flex_col().w_full().max_w(px(COLUMN_W)).gap(px(24.)).child(heading).child(body))),
            );
        div().flex().size_full().child(sidebar).child(content).into_any_element()
    }

    fn general_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let custom = !self.settings.data_dir.trim().is_empty();
        let reveal = if cfg!(target_os = "macos") { "在访达中显示" } else if cfg!(target_os = "windows") { "在资源管理器中显示" } else { "在文件管理器中显示" };
        let folder = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .when(custom, |d| d.child(link_button("data-reset", "恢复默认").on_click(self.on(Cmd::ResetDataDir, cx))))
            .child(button("data-reveal", reveal).on_click(self.on(Cmd::RevealDataDir, cx)))
            .child(button("data-pick", "更改…").on_click(self.on(Cmd::PickDataDir, cx)));
        div()
            .flex()
            .flex_col()
            .w_full()
            .gap(px(24.))
            .child(
                group(Some("数据"), vec![row("数据目录", Some(self.root.display().to_string().into()), folder).into_any_element()])
                    .child(note("所有文献库都在这个文件夹下，每个文献库是其中的一个子文件夹。改到别的目录后，首页显示那个目录里的文献库；原目录里的文献库不会被移动。")),
            )
            .child(group(
                Some("更新"),
                vec![
                    row("自动检查更新", Some("只读取一份版本信息，不发送任何数据；新版本由你决定何时下载安装。".into()), switch("auto-update", self.settings.auto_update).on_click(self.on(Cmd::ToggleAutoUpdate, cx))).into_any_element(),
                    self.update_row(cx),
                ],
            ))
            .into_any_element()
    }

    fn appearance_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let mode = match self.settings.appearance.as_str() {
            "light" => 1,
            "dark" => 2,
            _ => 0,
        };
        let compact = usize::from(self.settings.density == "compact");
        let set = |value: &str| Cmd::SetAppearance(value.to_string());
        let density = |value: &str| Cmd::SetDensity(value.to_string());
        group(
            None,
            vec![
                row("主题", Some("跟随系统时，随系统的浅色 / 深色模式切换。".into()), segmented("appearance", vec![("跟随系统", set("")), ("浅色", set("light")), ("深色", set("dark"))], mode, self, cx)).into_any_element(),
                row("文献列表", Some("紧凑的行更矮，一屏能看到更多文献。".into()), segmented("density", vec![("标准", density("")), ("紧凑", density("compact"))], compact, self, cx)).into_any_element(),
            ],
        )
        .child(note("标识色属于文献库：在文献库的「编辑名称与主题」里选择，打开文献库后界面随之变色。"))
        .into_any_element()
    }

    fn shortcuts_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(view) = &self.settings_view else { return div().into_any_element() };
        let custom = &self.settings.shortcuts;
        let mut groups: Vec<Vec<AnyElement>> = GROUPS.iter().map(|_| vec![]).collect();
        for shortcut in &SHORTCUTS {
            let id = shortcut.id;
            let key = shortcuts::key_of(id, custom);
            let recording = view.recording.as_deref() == Some(id);
            let error = view.shortcut_error.as_ref().filter(|(e, _)| e == id).map(|(_, why)| why.clone());
            let line = div()
                .flex()
                .items_center()
                .gap(px(8.))
                .w_full()
                .min_h(px(28.))
                .child(div().flex_1().min_w_0().child(shortcut.label))
                .when(custom.contains_key(id) && !recording, |d| d.child(link_button(SharedString::from(format!("reset-{id}")), "恢复默认").on_click(self.on(Cmd::ResetShortcut(id.into()), cx))))
                .when(!key.is_empty() && !recording, |d| d.child(link_button(SharedString::from(format!("clear-{id}")), "移除").invisible().group_hover("shortcut-row", |s| s.visible()).on_click(self.on(Cmd::ClearShortcut(id.into()), cx))))
                .child(keycap(SharedString::from(format!("key-{id}")), &key, recording).on_click(self.on(Cmd::RecordShortcut(id.into()), cx)));
            groups[shortcut.group].push(
                div()
                    .group("shortcut-row")
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .w_full()
                    .px(px(16.))
                    .py(px(8.))
                    .child(line)
                    .when(recording, |d| d.child(div().text_size(px(12.)).text_color(Theme::text_tertiary()).child("按下新的组合键即可保存 · Esc 取消")))
                    .children(error.map(|why| div().text_size(px(12.)).line_height(px(17.)).text_color(Theme::danger()).child(why)))
                    .into_any_element(),
            );
        }
        // Esc is not for sale: every dialog and menu relies on it.
        groups[0].push(div().flex().items_center().w_full().px(px(16.)).py(px(8.)).min_h(px(44.)).child(div().flex_1().min_w_0().child("关闭对话框和菜单，返回上一层")).child(div().min_w(px(148.)).text_size(px(12.)).text_color(Theme::text_tertiary()).flex().justify_center().child("Esc")).into_any_element());
        div()
            .flex()
            .flex_col()
            .w_full()
            .gap(px(24.))
            .children(GROUPS.iter().zip(groups).map(|(title, rows)| group(Some(*title), rows)))
            .child(note("点一个快捷键，再按下新的组合键。文本框里的编辑按键（复制、粘贴、全选等）是固定的。"))
            .into_any_element()
    }

    fn ai_page(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(view) = &self.settings_view else { return div().into_any_element() };
        let profile = self.settings.profile_or_default();
        let (variable, privacy) = match view.protocol {
            Protocol::Anthropic => ("ANTHROPIC_API_KEY", "提问时，问题和相关文献的记录、笔记、全文页面会发送给这个服务。API Key 以明文保存在本机的 settings.json 里。"),
            Protocol::OpenAI => ("OPENAI_API_KEY", "适用于任何兼容 chat/completions 的服务，地址写到 /v1。提问时，问题和相关文献的记录、笔记、全文页面会发送给这个服务。API Key 以明文保存在本机的 settings.json 里。"),
        };
        let (state, colors) = if profile.key().is_empty() {
            ("还缺 API Key", theme::status_colors("uncertain"))
        } else if profile.model.trim().is_empty() {
            ("还缺模型名称", theme::status_colors("uncertain"))
        } else {
            ("可以提问", theme::status_colors("included"))
        };
        let field = |input: &Entity<TextInput>| div().w(px(INPUT_W)).child(ui::field(input, window, cx));
        let (base_url, model, api_key) = (field(&view.base_url), field(&view.model), field(&view.api_key));
        div()
            .flex()
            .flex_col()
            .w_full()
            .gap(px(24.))
            .child(group(None, vec![row("文献库提问", Some("向文献库或单篇文献提问时使用下面的模型服务。修改会自动保存。".into()), ui::chip(state, colors.0, colors.1)).into_any_element()]))
            .child(
                group(
                    Some("模型服务"),
                    vec![
                        row("接口", None, segmented("protocol", vec![("Claude API", Cmd::PickProtocol(Protocol::Anthropic)), ("OpenAI 兼容", Cmd::PickProtocol(Protocol::OpenAI))], usize::from(view.protocol == Protocol::OpenAI), self, cx)).into_any_element(),
                        row("服务地址", None, base_url).into_any_element(),
                        row("模型", None, model).into_any_element(),
                        row("API Key", Some(format!("留空则读取环境变量 {variable}。").into()), api_key).into_any_element(),
                    ],
                )
                .child(note(privacy)),
            )
            .into_any_element()
    }

    fn about_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let open = |id: &'static str, label: &'static str, url: String, this: &Self, cx: &mut Context<Self>| button(id, label).on_click(this.on(Cmd::OpenUrl(url), cx));
        div()
            .flex()
            .flex_col()
            .w_full()
            .gap(px(24.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(8.))
                    .pt(px(8.))
                    .child(img(about_icon()).size(px(88.)))
                    .child(div().pt(px(4.)).text_size(px(20.)).line_height(px(26.)).font_weight(FontWeight::SEMIBOLD).child("Wisp Paper"))
                    .child(div().text_color(Theme::text_secondary()).child(format!("版本 {}", env!("CARGO_PKG_VERSION"))))
                    .child(div().text_color(Theme::text_tertiary()).child("本地优先的文献库：收集文献、整理、预览 PDF、对文献库提问。")),
            )
            .child(group(None, vec![self.update_row(cx)]))
            .child(group(
                None,
                vec![
                    row("项目主页", Some("github.com/xuzhougeng/WispPaper".into()), open("about-project", "打开", update::PROJECT_URL.to_string(), self, cx)).into_any_element(),
                    row("开源许可", Some("Wisp Paper 以 MIT 协议发布；随附的第三方组件见 THIRD_PARTY.md。".into()), open("about-license", "查看", format!("{}/blob/main/THIRD_PARTY.md", update::PROJECT_URL), self, cx)).into_any_element(),
                ],
            ))
            .into_any_element()
    }
}
