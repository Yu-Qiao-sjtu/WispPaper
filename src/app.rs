//! `Workspace`, the window's root view: the home page of libraries, or one
//! open library. All state lives here; every click becomes a `Cmd` handled
//! by `run`, and the other modules render parts of it.

use crate::input::{InputEvent, TextInput};
use crate::pdf::PdfView;
use crate::settings::{SettingsPage, SettingsView, UpdateState};
use crate::shortcuts::{self, Dismiss};
use crate::theme;
use gpui::*;
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::time::Instant;
use wisp_core::ai::{self, ChatMessage, Protocol, Scope, StreamEvent};
use wisp_core::filter::{self, Filters, View};
use wisp_core::libraries::{self, LibraryInfo};
use wisp_core::lookup::{self, Identifier};
use wisp_core::model::*;
pub(crate) use wisp_core::settings::default_root;
use wisp_core::settings::{Settings, settings_file};
use wisp_core::store::{Store, TrashEntry};
use wisp_core::{export, fulltext, import, time};

/// What the middle of an open library shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Papers,
    /// 文献库提问: questions over the current list.
    Ask,
    Trash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Info,
    Pdf,
    Ask,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FilterKey {
    Tag,
    Year,
    Reading,
    Screening,
    Fulltext,
    Source,
    Preprint,
    Field(String),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ExportKind {
    Bibtex,
    Csv,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MenuKind {
    Add,
    Filter(FilterKey),
    Sort,
    Export,
    Reading(String),
    Screening(String),
    LibraryCard(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Cmd {
    // Home
    OpenLibrary(String),
    NewLibrary,
    EditLibrary(String),
    AskRemoveLibrary(String),
    RemoveLibrary(String),
    GoHome,
    RevealFolder,
    OpenUrl(String),
    // Settings
    OpenSettings(SettingsPage),
    CloseSettings,
    PickDataDir,
    ResetDataDir,
    RevealDataDir,
    ToggleAutoUpdate,
    CheckUpdate,
    SetAppearance(String),
    SetDensity(String),
    RecordShortcut(String),
    ClearShortcut(String),
    ResetShortcut(String),
    ResetShortcuts,
    PickProtocol(Protocol),
    // Collect
    AddById,
    AddManual,
    ImportReferences,
    ImportPdfs,
    // The list
    SetView(View),
    SetPage(Page),
    SetFilter(FilterKey, String),
    ClearFilters,
    SetSort(String),
    Select(String),
    CloseDetail,
    ToggleStar(String),
    Export(ExportKind),
    // One paper
    SetReading(String, String),
    SetScreening(String, String),
    EditPaper(String),
    TrashPaper(String),
    RestorePaper(String),
    AskDeletePaper(String),
    DeletePaper(String),
    OpenLink(String),
    AttachPdf(String),
    OpenFile(String),
    RemoveFile(String),
    SetTab(Tab),
    ToggleWide,
    // Custom fields
    ManageFields,
    EditField(String),
    DeleteField(String),
    MoveField(String, i32),
    ToggleFieldInTable(String),
    AddPreset,
    // Questions
    Ask(bool),
    /// Ask one of the suggested questions.
    Suggest(bool, String),
    NewChat(bool),
    GotoSource(String, Option<usize>),
    // Dialogs
    CloseModal,
    SubmitModal,
    PickColor(String),
    PickKind(FieldKind),
    PickOption(String, String),
    ToggleInTable,
}

pub struct MenuItem {
    pub label: SharedString,
    pub checked: bool,
    pub note: Option<SharedString>,
    pub cmd: Cmd,
}

pub struct Menu {
    pub at: Point<Pixels>,
    pub items: Vec<MenuItem>,
}

pub struct Toast {
    pub message: SharedString,
    pub error: bool,
    pub at: Instant,
}

/// One conversation: with the library (the current list) or with one paper.
pub struct Chat {
    pub input: Entity<TextInput>,
    pub messages: Vec<ChatMessage>,
    /// What the model was given, built with the first question.
    pub ctx: Option<ai::Context>,
    pub busy: bool,
    pub error: String,
    /// Why the last answer ended early, if it did.
    pub note: String,
    pub scroll: ScrollHandle,
    /// Bumped by 新对话, so a stream still arriving for the old one is dropped.
    pub generation: u64,
}

pub struct PaperForm {
    /// The paper being edited; `None` adds a new one.
    pub id: Option<String>,
    pub title: Entity<TextInput>,
    pub authors: Entity<TextInput>,
    pub year: Entity<TextInput>,
    pub journal: Entity<TextInput>,
    pub doi: Entity<TextInput>,
    pub pmid: Entity<TextInput>,
    pub url: Entity<TextInput>,
    pub tags: Entity<TextInput>,
    pub abstract_text: Entity<TextInput>,
    /// Text and Tags fields, by field id.
    pub fields: Vec<(String, Entity<TextInput>)>,
    /// The chosen option of each Select field.
    pub selects: BTreeMap<String, String>,
}

pub enum Modal {
    Library { editing: Option<String>, name: Entity<TextInput>, topic: Entity<TextInput>, color: String },
    AddById { input: Entity<TextInput>, busy: bool },
    Paper(Box<PaperForm>),
    Fields { editing: Option<String>, name: Entity<TextInput>, options: Entity<TextInput>, kind: FieldKind, in_table: bool },
    Confirm { title: String, message: String, action: String, cmd: Cmd },
}

pub struct Library {
    pub info: LibraryInfo,
    pub store: Store,
    pub papers: Vec<Paper>,
    pub fields: Vec<FieldDef>,
    pub files: Vec<PaperFile>,
    pub trash: Vec<TrashEntry>,
    pub filters: Filters,
    /// Indices into `papers` of the rows shown, in order.
    pub shown: Vec<usize>,
    pub search: Entity<TextInput>,
    pub page: Page,
    pub selected: Option<String>,
    pub tab: Tab,
    /// The detail panel takes most of the window (reading a PDF).
    pub wide: bool,
    pub notes: Entity<TextInput>,
    pub pdf: Option<Entity<PdfView>>,
    pub library_chat: Chat,
    pub paper_chat: Chat,
    pub table_scroll: UniformListScrollHandle,
    pub detail_scroll: ScrollHandle,
    /// Files whose text is being extracted.
    pub extracting: HashSet<String>,
    /// How many dropped or picked files are still being read.
    pub importing: usize,
    _subscriptions: Vec<Subscription>,
}

pub struct Workspace {
    pub focus: FocusHandle,
    pub settings: Settings,
    settings_path: PathBuf,
    pub root: PathBuf,
    pub libraries: Vec<LibraryInfo>,
    pub lib: Option<Library>,
    /// 设置, shown instead of the home page or the library while it is open.
    pub settings_view: Option<SettingsView>,
    pub update: UpdateState,
    /// The version the automatic check last announced, so it says it once.
    pub update_told: Option<String>,
    pub modal: Option<Modal>,
    pub modal_error: String,
    pub modal_subscriptions: Vec<Subscription>,
    pub menu: Option<Menu>,
    pub toast: Option<Toast>,
    pub home_scroll: ScrollHandle,
}

impl Chat {
    fn new(placeholder: &'static str, cx: &mut App) -> Chat {
        let input = cx.new(|cx| {
            let mut input = TextInput::new(placeholder, cx).multiline(1);
            input.submit_on_enter = true;
            input
        });
        Chat { input, messages: vec![], ctx: None, busy: false, error: String::new(), note: String::new(), scroll: ScrollHandle::new(), generation: 0 }
    }

    fn reset(&mut self, cx: &mut App) {
        self.messages.clear();
        self.ctx = None;
        self.busy = false;
        self.error.clear();
        self.note.clear();
        self.generation += 1;
        self.input.update(cx, |input, cx| input.set_text("", cx));
    }
}

impl Library {
    pub fn paper(&self, id: &str) -> Option<&Paper> {
        self.papers.iter().find(|p| p.id == id)
    }

    pub fn selected_paper(&self) -> Option<&Paper> {
        self.selected.as_deref().and_then(|id| self.paper(id))
    }

    pub fn paper_files(&self, id: &str) -> Vec<&PaperFile> {
        self.files.iter().filter(|f| f.paper_id == id).collect()
    }

    /// The stored PDF to preview and read: the first full-text PDF, else any PDF.
    pub fn pdf_file(&self, id: &str) -> Option<&PaperFile> {
        let files = self.paper_files(id);
        files.iter().find(|f| f.kind == "fulltext" && f.format == "pdf").or(files.iter().find(|f| f.format == "pdf")).copied()
    }

    pub fn refilter(&mut self) {
        self.shown = filter::filter_papers(&self.papers, &self.filters, time::now_millis());
    }

    /// Read everything again after a change.
    pub fn reload(&mut self) -> anyhow::Result<()> {
        self.papers = self.store.list_papers()?;
        self.fields = self.store.list_fields()?;
        self.files = self.store.list_files()?;
        self.trash = self.store.list_trash()?;
        // Filters on a field that no longer exists would hide everything.
        let known: HashSet<&str> = self.fields.iter().map(|f| f.id.as_str()).collect();
        self.filters.fields.retain(|id, _| known.contains(id.as_str()));
        if self.selected.as_ref().is_some_and(|id| self.paper(id).is_none()) {
            self.selected = None;
            self.pdf = None;
        }
        self.refilter();
        Ok(())
    }
}

impl Workspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let settings_path = settings_file();
        let mut toast = None;
        let settings = Settings::load(&settings_path).unwrap_or_else(|err| {
            toast = Some(Toast { message: format!("{err:#}；本次使用默认设置。").into(), error: true, at: Instant::now() });
            Settings::default()
        });
        let root = settings.data_root(&default_root());
        let _ = std::fs::create_dir_all(&root);
        let focus = cx.focus_handle();
        window.focus(&focus);
        theme::set_accent("blue");
        shortcuts::rebuild(&settings.shortcuts, cx);
        let libraries = libraries::list(&root);
        let mut this = Workspace { focus, settings, settings_path, root, libraries, lib: None, settings_view: None, update: UpdateState::Idle, update_told: None, modal: None, modal_error: String::new(), modal_subscriptions: vec![], menu: None, toast, home_scroll: ScrollHandle::new() };
        this.apply_appearance(window);
        cx.observe_window_appearance(window, |this, window, _| this.apply_appearance(window)).detach();
        this.watch_updates(cx);
        this
    }

    pub fn on(&self, cmd: Cmd, cx: &mut Context<Self>) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
        cx.listener(move |this, _, window, cx| this.run(cmd.clone(), window, cx))
    }

    pub fn on_menu(&self, kind: MenuKind, cx: &mut Context<Self>) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
        cx.listener(move |this, ev: &ClickEvent, _, cx| this.open_menu(kind.clone(), ev.position(), cx))
    }

    pub fn say(&mut self, message: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.show_toast(message.into(), false, cx);
    }

    pub fn fail(&mut self, message: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.show_toast(message.into(), true, cx);
    }

    fn show_toast(&mut self, message: SharedString, error: bool, cx: &mut Context<Self>) {
        let at = Instant::now();
        self.toast = Some(Toast { message, error, at });
        cx.notify();
        // Errors stay longer: they usually need reading twice.
        let seconds = if error { 7 } else { 3 };
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(std::time::Duration::from_secs(seconds)).await;
            this.update(cx, |this, cx| {
                if this.toast.as_ref().is_some_and(|t| t.at == at) {
                    this.toast = None;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Run a store operation on the open library, reload, and report a failure.
    fn change(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Library) -> anyhow::Result<()>) -> bool {
        let Some(lib) = self.lib.as_mut() else { return false };
        let result = f(lib).and_then(|_| lib.reload());
        cx.notify();
        match result {
            Ok(()) => true,
            Err(err) => {
                self.fail(format!("{err:#}"), cx);
                false
            }
        }
    }

    pub fn save_settings(&mut self, cx: &mut Context<Self>) {
        if let Err(err) = self.settings.save(&self.settings_path) {
            self.fail(format!("{err:#}"), cx);
        }
    }

    // ---- menus

    fn open_menu(&mut self, kind: MenuKind, at: Point<Pixels>, cx: &mut Context<Self>) {
        let item = |label: &str, cmd: Cmd| MenuItem { label: label.to_string().into(), checked: false, note: None, cmd };
        let mut items = vec![];
        match kind {
            MenuKind::Add => {
                items.push(item("通过 DOI / PMID 添加…", Cmd::AddById));
                items.push(item("导入 PDF 文件…", Cmd::ImportPdfs));
                items.push(item("导入 BibTeX / RIS…", Cmd::ImportReferences));
                items.push(item("手动录入…", Cmd::AddManual));
            }
            MenuKind::Export => {
                items.push(item("BibTeX（.bib）", Cmd::Export(ExportKind::Bibtex)));
                items.push(item("表格（.csv）", Cmd::Export(ExportKind::Csv)));
            }
            MenuKind::LibraryCard(id) => {
                items.push(item("打开", Cmd::OpenLibrary(id.clone())));
                items.push(item("编辑名称与主题…", Cmd::EditLibrary(id.clone())));
                items.push(item("移除文献库…", Cmd::AskRemoveLibrary(id)));
            }
            MenuKind::Sort => {
                let Some(lib) = &self.lib else { return };
                let current = if lib.filters.sort.is_empty() { "newest" } else { lib.filters.sort.as_str() };
                for (value, label) in [("newest", "最近添加"), ("year-desc", "发表年份 ↓"), ("year-asc", "发表年份 ↑"), ("title-asc", "题目 A→Z"), ("title-desc", "题目 Z→A"), ("journal-asc", "期刊 A→Z"), ("authors-asc", "作者 A→Z")] {
                    items.push(MenuItem { label: label.into(), checked: current == value, note: None, cmd: Cmd::SetSort(value.into()) });
                }
            }
            MenuKind::Reading(id) => {
                let Some(current) = self.lib.as_ref().and_then(|l| l.paper(&id)).map(|p| p.reading.clone()) else { return };
                for (value, label) in READINGS {
                    items.push(MenuItem { label: label.into(), checked: current == value, note: None, cmd: Cmd::SetReading(id.clone(), value.into()) });
                }
            }
            MenuKind::Screening(id) => {
                let Some(current) = self.lib.as_ref().and_then(|l| l.paper(&id)).map(|p| p.screening.clone()) else { return };
                let hints = [("pending", "还没有判断"), ("included", "符合主题，重点阅读"), ("excluded", "可能有用，保留备查"), ("uncertain", "暂时判断不了"), ("irrelevant", "与主题无关，默认隐藏")];
                for (value, label) in SCREENINGS {
                    let hint = hints.iter().find(|(v, _)| *v == value).map(|(_, h)| SharedString::from(*h));
                    items.push(MenuItem { label: label.into(), checked: current == value, note: hint, cmd: Cmd::SetScreening(id.clone(), value.into()) });
                }
            }
            MenuKind::Filter(key) => {
                let Some(lib) = &self.lib else { return };
                let f = &lib.filters;
                let fixed = |table: &[(&str, &str)]| table.iter().map(|(v, l)| (v.to_string(), l.to_string(), None)).collect::<Vec<_>>();
                let counted = |list: Vec<(String, usize)>| list.into_iter().take(40).map(|(v, n)| (v.clone(), v, Some(n))).collect::<Vec<_>>();
                let (current, choices): (String, Vec<(String, String, Option<usize>)>) = match &key {
                    FilterKey::Tag => (f.tag.clone(), counted(filter::tag_counts(&lib.papers))),
                    FilterKey::Year => (f.year.clone(), counted(filter::years(&lib.papers))),
                    FilterKey::Source => (f.source.clone(), counted(filter::source_counts(&lib.papers))),
                    FilterKey::Reading => (f.reading.clone(), fixed(&READINGS)),
                    FilterKey::Screening => (f.screening.clone(), fixed(&SCREENINGS)),
                    FilterKey::Fulltext => (f.fulltext.clone(), fixed(&FULLTEXTS)),
                    FilterKey::Preprint => (f.preprint.clone(), fixed(&[("yes", "只看预印本"), ("no", "不含预印本")])),
                    FilterKey::Field(id) => (f.fields.get(id).cloned().unwrap_or_default(), counted(filter::field_counts(&lib.papers, id))),
                };
                items.push(MenuItem { label: "全部".into(), checked: current.is_empty(), note: None, cmd: Cmd::SetFilter(key.clone(), String::new()) });
                for (value, label, count) in choices {
                    items.push(MenuItem { label: label.into(), checked: current == value, note: count.map(|n| n.to_string().into()), cmd: Cmd::SetFilter(key.clone(), value) });
                }
            }
        }
        self.menu = Some(Menu { at, items });
        cx.notify();
    }

    // ---- commands

    pub fn run(&mut self, cmd: Cmd, window: &mut Window, cx: &mut Context<Self>) {
        self.menu = None;
        // Anything else than a key ends the wait for a shortcut's new key.
        if !matches!(cmd, Cmd::RecordShortcut(_)) { self.stop_recording(cx) }
        cx.notify();
        match cmd {
            Cmd::OpenLibrary(id) => self.open_library(&id, window, cx),
            Cmd::GoHome => {
                self.lib = None;
                self.settings_view = None;
                self.modal = None;
                theme::set_accent("blue");
                self.libraries = libraries::list(&self.root);
                window.set_window_title("Wisp Paper");
                window.focus(&self.focus);
            }
            Cmd::NewLibrary => self.library_dialog(None, window, cx),
            Cmd::EditLibrary(id) => self.library_dialog(Some(id), window, cx),
            Cmd::AskRemoveLibrary(id) => {
                let Some(info) = self.libraries.iter().find(|l| l.id == id).or(self.lib.as_ref().map(|l| &l.info)) else { return };
                self.modal_error.clear();
                self.modal = Some(Modal::Confirm {
                    title: format!("移除文献库「{}」？", info.name),
                    message: format!("文献库会从首页消失。它的文件夹（{} 篇文献、PDF 和笔记）会移到数据目录下的 removed 文件夹，不会被删除，需要时可以手动移回。", info.paper_count),
                    action: "移除".into(),
                    cmd: Cmd::RemoveLibrary(id),
                });
            }
            Cmd::RemoveLibrary(id) => {
                self.modal = None;
                // The database must be closed before its folder can move.
                if self.lib.as_ref().is_some_and(|l| l.info.id == id) { self.lib = None }
                let Some(info) = self.libraries.iter().find(|l| l.id == id).cloned() else { return };
                match libraries::remove(&self.root, &info) {
                    Ok(target) => self.say(format!("已移到 {}", target.display()), cx),
                    Err(err) => self.fail(format!("{err:#}"), cx),
                }
                self.libraries = libraries::list(&self.root);
            }
            Cmd::RevealFolder => {
                let path = self.lib.as_ref().map_or(self.root.clone(), |l| l.info.dir.clone());
                cx.open_with_system(&path);
            }
            Cmd::OpenUrl(url) => cx.open_url(&url),

            Cmd::OpenSettings(page) => self.open_settings(page, window, cx),
            Cmd::CloseSettings => self.close_settings(window, cx),
            Cmd::PickDataDir => self.pick_data_dir(cx),
            Cmd::ResetDataDir => self.set_data_dir(None, cx),
            Cmd::RevealDataDir => cx.open_with_system(&self.root),
            Cmd::ToggleAutoUpdate => {
                self.settings.auto_update = !self.settings.auto_update;
                self.save_settings(cx);
                if self.settings.auto_update { self.check_update(false, cx) }
            }
            Cmd::CheckUpdate => self.check_update(true, cx),
            Cmd::SetAppearance(value) => {
                self.settings.appearance = value;
                self.save_settings(cx);
                self.apply_appearance(window);
            }
            Cmd::SetDensity(value) => {
                self.settings.density = value;
                self.save_settings(cx);
            }
            Cmd::RecordShortcut(id) => self.start_recording(id, window, cx),
            Cmd::ClearShortcut(id) => self.set_shortcut(&id, Some(String::new()), cx),
            Cmd::ResetShortcut(id) => self.set_shortcut(&id, None, cx),
            Cmd::ResetShortcuts => self.reset_shortcuts(cx),
            Cmd::PickProtocol(value) => self.pick_protocol(value, cx),

            Cmd::AddById => {
                let input = self.modal_input("10.1038/s41586-020-2649-2、PMID 或它们的链接", "", false, window, cx);
                input.read(cx).focus(window);
                self.modal_error.clear();
                self.modal = Some(Modal::AddById { input, busy: false });
            }
            Cmd::AddManual => self.paper_dialog(None, window, cx),
            Cmd::EditPaper(id) => self.paper_dialog(Some(id), window, cx),
            Cmd::ImportReferences => self.pick_files("导入", false, cx),
            Cmd::ImportPdfs => self.pick_files("导入", true, cx),

            Cmd::SetView(view) => {
                if let Some(lib) = self.lib.as_mut() {
                    lib.filters.view = view;
                    lib.page = Page::Papers;
                    lib.refilter();
                }
            }
            Cmd::SetPage(page) => {
                if let Some(lib) = self.lib.as_mut() {
                    lib.page = page;
                    if page == Page::Ask { lib.library_chat.input.read(cx).focus(window) }
                }
            }
            Cmd::SetFilter(key, value) => {
                if let Some(lib) = self.lib.as_mut() {
                    let f = &mut lib.filters;
                    match key {
                        FilterKey::Tag => f.tag = value,
                        FilterKey::Year => f.year = value,
                        FilterKey::Reading => f.reading = value,
                        FilterKey::Screening => f.screening = value,
                        FilterKey::Fulltext => f.fulltext = value,
                        FilterKey::Source => f.source = value,
                        FilterKey::Preprint => f.preprint = value,
                        FilterKey::Field(id) => {
                            if value.is_empty() { f.fields.remove(&id); } else { f.fields.insert(id, value); }
                        }
                    }
                    lib.page = Page::Papers;
                    lib.refilter();
                }
            }
            Cmd::ClearFilters => {
                if let Some(lib) = self.lib.as_mut() {
                    lib.filters.clear();
                    lib.search.update(cx, |input, cx| input.set_text("", cx));
                    lib.refilter();
                }
            }
            Cmd::SetSort(sort) => {
                if let Some(lib) = self.lib.as_mut() {
                    lib.filters.sort = sort;
                    lib.refilter();
                }
            }
            Cmd::Select(id) => self.select(Some(id), window, cx),
            Cmd::CloseDetail => self.select(None, window, cx),
            Cmd::ToggleStar(id) => {
                self.edit_paper(&id, cx, |p| p.starred = !p.starred);
            }
            Cmd::Export(kind) => self.export(kind, cx),

            Cmd::SetReading(id, value) => {
                self.edit_paper(&id, cx, |p| p.reading = value);
            }
            Cmd::SetScreening(id, value) => {
                self.edit_paper(&id, cx, |p| p.decide(&value, ""));
            }
            Cmd::TrashPaper(id) => {
                if self.change(cx, |lib| lib.store.trash_paper(&id)) { self.say("已移到回收站。", cx) }
            }
            Cmd::RestorePaper(id) => {
                if self.change(cx, |lib| lib.store.restore_paper(&id)) { self.say("已恢复。", cx) }
            }
            Cmd::AskDeletePaper(id) => {
                let title = self.lib.as_ref().and_then(|l| l.trash.iter().find(|t| t.paper.id == id)).map(|t| t.paper.title.clone()).unwrap_or_default();
                self.modal_error.clear();
                self.modal = Some(Modal::Confirm { title: "彻底删除这篇文献？".into(), message: format!("「{title}」的记录、笔记和词条会被删除，无法恢复。它的 PDF 会移到文献库文件夹下的 removed 文件夹。"), action: "彻底删除".into(), cmd: Cmd::DeletePaper(id) });
            }
            Cmd::DeletePaper(id) => {
                self.modal = None;
                self.change(cx, |lib| lib.store.delete_paper(&id));
            }
            Cmd::OpenLink(id) => {
                if let Some(link) = self.lib.as_ref().and_then(|l| l.paper(&id)).map(|p| p.link()).filter(|l| !l.is_empty()) {
                    cx.open_url(&link);
                }
            }
            Cmd::AttachPdf(id) => self.pick_attachment(id, cx),
            Cmd::OpenFile(id) => {
                if let Some(lib) = &self.lib && let Some(file) = lib.files.iter().find(|f| f.id == id) {
                    cx.open_with_system(&lib.store.file_path(file));
                }
            }
            Cmd::RemoveFile(id) => {
                if self.change(cx, |lib| lib.store.remove_file(&id)) {
                    if let Some(lib) = self.lib.as_mut() { lib.pdf = None }
                    self.say("附件已移除（文件移到了 removed 文件夹）。", cx);
                }
            }
            Cmd::SetTab(tab) => {
                if let Some(lib) = self.lib.as_mut() {
                    lib.tab = tab;
                    if tab == Tab::Ask { lib.paper_chat.input.read(cx).focus(window) }
                }
                self.ensure_pdf(cx);
            }
            Cmd::ToggleWide => {
                if let Some(lib) = self.lib.as_mut() { lib.wide = !lib.wide }
            }

            Cmd::ManageFields => self.fields_dialog(None, window, cx),
            Cmd::EditField(id) => self.fields_dialog(Some(id), window, cx),
            Cmd::DeleteField(id) => {
                self.change(cx, |lib| lib.store.delete_field(&id));
                self.fields_dialog(None, window, cx);
            }
            Cmd::MoveField(id, step) => {
                self.change(cx, |lib| lib.store.move_field(&id, step));
            }
            Cmd::ToggleFieldInTable(id) => {
                self.change(cx, |lib| {
                    let Some(mut field) = lib.fields.iter().find(|f| f.id == id).cloned() else { return Ok(()) };
                    field.in_table = !field.in_table;
                    lib.store.save_field(&mut field)
                });
            }
            Cmd::AddPreset => {
                self.change(cx, |lib| {
                    // BiblioCell's single-cell fields, as a starting point.
                    for (name, kind, options) in [("物种", FieldKind::Tags, vec![]), ("组织", FieldKind::Tags, vec![]), ("测序类型", FieldKind::Tags, vec!["scRNA-seq", "snRNA-seq", "scATAC-seq", "Multiome", "空间转录组"]), ("测序平台", FieldKind::Tags, vec!["10x Genomics Chromium", "BD Rhapsody", "Smart-seq2"])] {
                        if lib.fields.iter().any(|f| f.name == name) { continue }
                        lib.store.save_field(&mut FieldDef { name: name.into(), kind, options: options.into_iter().map(String::from).collect(), in_table: name == "物种", ..Default::default() })?;
                    }
                    Ok(())
                });
            }

            Cmd::Ask(paper) => self.ask(paper, cx),
            Cmd::Suggest(paper, question) => {
                if let Some(lib) = self.lib.as_ref() {
                    let chat = if paper { &lib.paper_chat } else { &lib.library_chat };
                    chat.input.update(cx, |input, cx| input.set_text(&question, cx));
                }
                self.ask(paper, cx);
            }
            Cmd::NewChat(paper) => {
                if let Some(lib) = self.lib.as_mut() {
                    let chat = if paper { &mut lib.paper_chat } else { &mut lib.library_chat };
                    chat.reset(cx);
                    chat.input.read(cx).focus(window);
                }
            }
            Cmd::GotoSource(id, page) => {
                self.select(Some(id), window, cx);
                if let Some(page) = page && let Some(lib) = self.lib.as_mut() {
                    lib.tab = Tab::Pdf;
                    self.ensure_pdf(cx);
                    if let Some(pdf) = self.lib.as_ref().and_then(|l| l.pdf.clone()) {
                        pdf.update(cx, |pdf, cx| pdf.go_to(page, cx));
                    }
                }
            }

            Cmd::CloseModal => {
                self.modal = None;
                self.modal_subscriptions.clear();
                window.focus(&self.focus);
            }
            Cmd::SubmitModal => self.submit_modal(window, cx),
            Cmd::PickColor(value) => {
                if let Some(Modal::Library { color, .. }) = self.modal.as_mut() { *color = value }
            }
            Cmd::PickKind(value) => {
                if let Some(Modal::Fields { kind, .. }) = self.modal.as_mut() { *kind = value }
            }
            Cmd::ToggleInTable => {
                if let Some(Modal::Fields { in_table, .. }) = self.modal.as_mut() { *in_table = !*in_table }
            }
            Cmd::PickOption(field, option) => {
                if let Some(Modal::Paper(form)) = self.modal.as_mut() {
                    if form.selects.get(&field) == Some(&option) { form.selects.remove(&field); } else { form.selects.insert(field, option); }
                }
            }
        }
    }

    pub fn dismiss(&mut self, _: &Dismiss, window: &mut Window, cx: &mut Context<Self>) {
        if self.menu.take().is_some() {
        } else if self.modal.is_some() {
            self.run(Cmd::CloseModal, window, cx);
        } else if self.settings_view.is_some() {
            self.close_settings(window, cx);
        } else if self.lib.as_ref().is_some_and(|l| l.selected.is_some()) {
            self.select(None, window, cx);
        }
        cx.notify();
    }

    /// A keyboard shortcut (shortcuts.rs) was pressed.
    pub fn shortcut(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        // A dialog keeps the keyboard to itself.
        if self.modal.is_some() { return }
        let in_settings = self.settings_view.is_some();
        let cmd = match id {
            "settings" if !in_settings => Cmd::OpenSettings(SettingsPage::General),
            "home" => Cmd::GoHome,
            "new-library" if !in_settings && self.lib.is_none() => Cmd::NewLibrary,
            // The rest act on the open library, when it is the one in view.
            _ if in_settings || self.lib.is_none() => return,
            "search" => {
                if let Some(lib) = self.lib.as_mut() {
                    lib.page = Page::Papers;
                    lib.search.read(cx).focus(window);
                    cx.notify();
                }
                return;
            }
            "add" => Cmd::AddById,
            "import-pdf" => Cmd::ImportPdfs,
            "ask" => Cmd::SetPage(Page::Ask),
            "detail" if self.lib.as_ref().is_some_and(|l| l.selected.is_some()) => Cmd::ToggleWide,
            _ => return,
        };
        self.run(cmd, window, cx);
    }

    // ---- libraries

    fn open_library(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(mut info) = self.libraries.iter().find(|l| l.id == id).cloned() else { return };
        let store = match Store::open(&info.dir) {
            Ok(store) => store,
            Err(err) => return self.fail(format!("{err:#}"), cx),
        };
        info.opened_at = time::now_iso();
        let _ = libraries::save(&info);
        theme::set_accent(&info.color);
        window.set_window_title(&format!("{} — Wisp Paper", info.name));
        let search = cx.new(|cx| TextInput::new("搜索题目、作者、DOI、标签、词条、笔记…", cx));
        let notes = cx.new(|cx| TextInput::new("读这篇文献时想到的…", cx).multiline(4));
        let subscriptions = vec![
            cx.subscribe(&search, |this, input, event: &InputEvent, cx| {
                if *event == InputEvent::Changed && let Some(lib) = this.lib.as_mut() {
                    lib.filters.search = input.read(cx).text().to_string();
                    lib.page = Page::Papers;
                    lib.refilter();
                    cx.notify();
                }
            }),
            cx.subscribe(&notes, |this, input, event: &InputEvent, cx| {
                if *event == InputEvent::Changed {
                    let text = input.read(cx).text().to_string();
                    if let Some(id) = this.lib.as_ref().and_then(|l| l.selected.clone()) {
                        this.edit_paper(&id, cx, |p| p.notes = text);
                    }
                }
            }),
        ];
        let library_chat = Chat::new("向文献库提问：例如“哪些文献研究了根尖干细胞？它们的结论一致吗？”", cx);
        let paper_chat = Chat::new("就这篇文献提问：例如“这篇文章的主要结论和证据是什么？”", cx);
        let mut subscriptions = subscriptions;
        for (chat, paper) in [(&library_chat, false), (&paper_chat, true)] {
            subscriptions.push(cx.subscribe(&chat.input, move |this, _, event: &InputEvent, cx| {
                if *event == InputEvent::Submit { this.ask(paper, cx) }
            }));
        }
        let mut lib = Library {
            info,
            store,
            papers: vec![],
            fields: vec![],
            files: vec![],
            trash: vec![],
            filters: Filters::default(),
            shown: vec![],
            search,
            page: Page::Papers,
            selected: None,
            tab: Tab::Info,
            wide: false,
            notes,
            pdf: None,
            library_chat,
            paper_chat,
            table_scroll: UniformListScrollHandle::new(),
            detail_scroll: ScrollHandle::new(),
            extracting: HashSet::new(),
            importing: 0,
            _subscriptions: subscriptions,
        };
        if let Err(err) = lib.reload() {
            return self.fail(format!("{err:#}"), cx);
        }
        self.lib = Some(lib);
        window.focus(&self.focus);
        self.extract_missing(cx);
    }

    // ---- one paper

    fn select(&mut self, id: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(lib) = self.lib.as_mut() else { return };
        if lib.selected == id { return }
        lib.selected = id;
        lib.pdf = None;
        lib.paper_chat.reset(cx);
        lib.detail_scroll = ScrollHandle::new();
        match lib.selected_paper() {
            Some(paper) => {
                let notes = paper.notes.clone();
                lib.notes.update(cx, |input, cx| input.set_text(&notes, cx));
            }
            None => {
                lib.wide = false;
                lib.tab = Tab::Info;
                window.focus(&self.focus);
            }
        }
        self.ensure_pdf(cx);
        cx.notify();
    }

    /// Open the PDF viewer when its tab is showing and the paper has a PDF.
    fn ensure_pdf(&mut self, cx: &mut Context<Self>) {
        let Some(lib) = self.lib.as_mut() else { return };
        if lib.tab != Tab::Pdf || lib.pdf.is_some() { return }
        let Some(path) = lib.selected.as_deref().and_then(|id| lib.pdf_file(id)).map(|f| lib.store.file_path(f)) else { return };
        lib.pdf = Some(cx.new(|cx| PdfView::new(&path, cx)));
    }

    /// Change one paper and save it.
    fn edit_paper(&mut self, id: &str, cx: &mut Context<Self>, edit: impl FnOnce(&mut Paper)) -> bool {
        self.change(cx, |lib| {
            let mut paper = lib.store.get_paper(id)?;
            edit(&mut paper);
            lib.store.save_paper(&mut paper)
        })
    }

    // ---- collecting

    fn pick_files(&mut self, prompt: &'static str, pdfs: bool, cx: &mut Context<Self>) {
        let extensions: Vec<SharedString> = if pdfs { vec!["pdf".into()] } else { vec!["bib".into(), "ris".into(), "txt".into(), "enw".into()] };
        let picked = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: true, prompt: Some(prompt.into()), extensions: Some(extensions) });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = picked.await {
                this.update(cx, |this, cx| this.import_paths(paths, cx)).ok();
            }
        })
        .detach();
    }

    fn pick_attachment(&mut self, paper_id: String, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: true, prompt: Some("添加".into()), extensions: None });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = picked.await {
                this.update(cx, |this, cx| {
                    let mut added = 0;
                    this.change(cx, |lib| {
                        let has_fulltext = lib.files.iter().any(|f| f.paper_id == paper_id && f.kind == "fulltext");
                        for (i, path) in paths.iter().enumerate() {
                            let is_pdf = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf"));
                            // The first PDF of a paper is its full text; the rest are supplements.
                            let kind = if is_pdf && !has_fulltext && i == 0 { "fulltext" } else { "supplement" };
                            lib.store.import_file(&paper_id, path, kind)?;
                            added += 1;
                        }
                        Ok(())
                    });
                    if added > 0 {
                        if let Some(lib) = this.lib.as_mut() { lib.pdf = None }
                        this.ensure_pdf(cx);
                        this.extract_missing(cx);
                        this.say(format!("已添加 {added} 个附件。"), cx);
                    }
                })
                .ok();
            }
        })
        .detach();
    }

    /// Files picked or dropped: PDFs become papers, reference files are imported.
    pub fn import_paths(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        if self.lib.is_none() { return self.fail("请先打开一个文献库，再拖入文件。", cx) }
        let (pdfs, others): (Vec<PathBuf>, Vec<PathBuf>) = paths.into_iter().partition(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")));
        let mut added = vec![];
        let mut skipped = 0;
        let mut errors = vec![];
        for path in others {
            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let parsed = std::fs::read(&path).map_err(|e| format!("无法读取 {name}：{e}")).and_then(|bytes| import::parse_references(&String::from_utf8_lossy(&bytes), &format!("导入 · {name}")).map_err(|e| format!("{name}：{e}")));
            match parsed {
                Ok(papers) => {
                    let mut result = None;
                    self.change(cx, |lib| {
                        result = Some(lib.store.import_papers(papers)?);
                        Ok(())
                    });
                    if let Some(result) = result {
                        skipped += result.skipped.len();
                        added.extend(result.added.into_iter().map(|p| p.id));
                    }
                }
                Err(err) => errors.push(err),
            }
        }
        if !errors.is_empty() {
            self.fail(errors.join("；"), cx);
        } else if !added.is_empty() || skipped > 0 {
            self.say(format!("导入 {} 篇{}。", added.len(), if skipped > 0 { format!("，跳过 {skipped} 篇已有的") } else { String::new() }), cx);
        }
        if pdfs.is_empty() { return }
        if let Some(lib) = self.lib.as_mut() { lib.importing += pdfs.len() }
        let library = self.lib.as_ref().map(|l| l.info.id.clone()).unwrap_or_default();
        cx.spawn(async move |this, cx| {
            for path in pdfs {
                // Read the text, look for a DOI on the first pages, and ask Crossref about it.
                let file = path.clone();
                let (pages, found) = cx
                    .background_executor()
                    .spawn(async move {
                        let pages = fulltext::file_pages(&file);
                        let doi = pages.as_ref().ok().and_then(|pages| lookup::find_doi(&pages.iter().take(2).cloned().collect::<Vec<_>>().join("\n")));
                        let found = doi.and_then(|doi| crate::net::lookup_paper(&Identifier::Doi(doi)).ok());
                        (pages, found)
                    })
                    .await;
                let done = this.update(cx, |this, cx| {
                    if this.lib.as_ref().is_none_or(|l| l.info.id != library) { return false }
                    this.add_pdf(&path, pages, found, cx);
                    true
                });
                if !matches!(done, Ok(true)) { break }
            }
        })
        .detach();
    }

    /// One read PDF: a new paper with the file attached, or the file attached
    /// to the paper the library already holds for its DOI.
    fn add_pdf(&mut self, path: &std::path::Path, pages: Result<Vec<String>, String>, found: Option<Paper>, cx: &mut Context<Self>) {
        let stem = path.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "未命名文献".into());
        let recognised = found.is_some();
        let mut paper = found.unwrap_or_else(|| Paper { title: stem.clone(), ..Default::default() });
        paper.source = "PDF 导入".into();
        let mut outcome = String::new();
        let mut selected = None;
        self.change(cx, |lib| {
            let target = match lib.store.existing_paper(&{
                let mut probe = paper.clone();
                probe.normalize();
                probe
            })? {
                Some(existing) => {
                    if lib.trash.iter().any(|t| t.paper.id == existing) {
                        outcome = format!("「{stem}」对应的文献在回收站中，未导入。");
                        return Ok(());
                    }
                    outcome = format!("「{stem}」已在文献库中，PDF 已附到原条目。");
                    existing
                }
                None => {
                    let added = lib.store.add_paper(paper)?;
                    outcome = if recognised { format!("已添加：{}", added.title) } else { format!("已添加「{stem}」。没有识别出 DOI，请补全题目和作者。") };
                    added.id
                }
            };
            let kind = if lib.store.paper_files(&target)?.iter().any(|f| f.kind == "fulltext") { "supplement" } else { "fulltext" };
            let file = lib.store.import_file(&target, path, kind)?;
            if lib.store.fulltext_pages(&file.id)?.is_none() { lib.store.save_fulltext_pages(&file.id, &pages)? }
            selected = Some(target);
            Ok(())
        });
        if let Some(lib) = self.lib.as_mut() {
            lib.importing = lib.importing.saturating_sub(1);
            if let Some(id) = selected && lib.importing == 0 && lib.selected.is_none() {
                lib.selected = Some(id);
                let notes = lib.selected_paper().map(|p| p.notes.clone()).unwrap_or_default();
                lib.notes.update(cx, |input, cx| input.set_text(&notes, cx));
            }
        }
        if !outcome.is_empty() { self.say(outcome, cx) }
    }

    /// Extract the text of stored full texts that have none yet, one at a time.
    pub fn extract_missing(&mut self, cx: &mut Context<Self>) {
        let Some(lib) = self.lib.as_mut() else { return };
        let mut todo = vec![];
        for file in &lib.files {
            if file.kind != "fulltext" || lib.extracting.contains(&file.id) { continue }
            if matches!(lib.store.fulltext_pages(&file.id), Ok(None)) {
                todo.push((file.id.clone(), lib.store.file_path(file)));
            }
        }
        if todo.is_empty() { return }
        lib.extracting.extend(todo.iter().map(|(id, _)| id.clone()));
        let library = lib.info.id.clone();
        cx.spawn(async move |this, cx| {
            for (id, path) in todo {
                let pages = cx.background_executor().spawn(async move { fulltext::file_pages(&path) }).await;
                let same = this.update(cx, |this, cx| {
                    let Some(lib) = this.lib.as_mut().filter(|l| l.info.id == library) else { return false };
                    lib.extracting.remove(&id);
                    let _ = lib.store.save_fulltext_pages(&id, &pages);
                    cx.notify();
                    true
                });
                if !matches!(same, Ok(true)) { break }
            }
        })
        .detach();
    }

    fn export(&mut self, kind: ExportKind, cx: &mut Context<Self>) {
        let Some(lib) = &self.lib else { return };
        let papers: Vec<&Paper> = lib.shown.iter().map(|&i| &lib.papers[i]).collect();
        if papers.is_empty() { return self.fail("当前列表是空的，没有可导出的文献。", cx) }
        let (text, extension) = match kind {
            ExportKind::Bibtex => (export::bibtex(&papers), "bib"),
            ExportKind::Csv => (export::papers_csv(&papers, &lib.fields), "csv"),
        };
        let count = papers.len();
        let name = format!("{}.{extension}", lib.info.name.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "-"));
        let directory = dirs::document_dir().or_else(dirs::home_dir).unwrap_or_else(|| self.root.clone());
        let picked = cx.prompt_for_new_path(&directory, Some(&name));
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(mut path))) = picked.await {
                if path.extension().is_none() { path.set_extension(extension); }
                let result = std::fs::write(&path, text);
                this.update(cx, |this, cx| match result {
                    Ok(()) => this.say(format!("已导出 {count} 篇到 {}", path.display()), cx),
                    Err(err) => this.fail(format!("无法写入 {}：{err}", path.display()), cx),
                })
                .ok();
            }
        })
        .detach();
    }

    // ---- questions

    fn ask(&mut self, about_paper: bool, cx: &mut Context<Self>) {
        let profile = self.settings.profile_or_default();
        let Some(lib) = self.lib.as_mut() else { return };
        let paper_id = lib.selected.clone();
        let chat = if about_paper { &mut lib.paper_chat } else { &mut lib.library_chat };
        let question = chat.input.read(cx).text().trim().to_string();
        if question.is_empty() || chat.busy { return }
        chat.error.clear();
        chat.note.clear();

        if chat.ctx.is_none() {
            let scope = match (about_paper, &paper_id) {
                (true, Some(id)) => Scope::Paper(id.clone()),
                (true, None) => return,
                _ => Scope::Library,
            };
            let fulltext = match lib.store.all_fulltext() {
                Ok(fulltext) => fulltext,
                Err(err) => {
                    chat.error = format!("{err:#}");
                    return cx.notify();
                }
            };
            // A question about one paper reads that paper; one about the library reads the current list.
            let papers: Vec<&Paper> = match &scope {
                Scope::Paper(_) => lib.papers.iter().collect(),
                Scope::Library => lib.shown.iter().map(|&i| &lib.papers[i]).collect(),
            };
            if papers.is_empty() {
                chat.error = "当前列表里没有文献。先添加文献，或清空筛选条件。".into();
                return cx.notify();
            }
            chat.ctx = Some(ai::build_context(&lib.info.name, &lib.info.topic, &papers, &lib.fields, &fulltext, &scope, &question));
        }
        chat.messages.push(ChatMessage { from_user: true, content: question });
        let request = match ai::chat_request(&profile, chat.ctx.as_ref().unwrap(), &chat.messages) {
            Ok(request) => request,
            Err(err) => {
                chat.messages.pop();
                if chat.messages.is_empty() { chat.ctx = None }
                chat.error = err;
                return cx.notify();
            }
        };
        chat.input.update(cx, |input, cx| input.set_text("", cx));
        chat.messages.push(ChatMessage { from_user: false, content: String::new() });
        chat.busy = true;
        chat.scroll.scroll_to_bottom();
        let generation = chat.generation;
        let library = lib.info.id.clone();
        cx.notify();

        let (sender, events) = async_channel::unbounded();
        let protocol = profile.protocol;
        cx.background_executor().spawn(async move { crate::net::stream_chat(request, protocol, sender) }).detach();
        cx.spawn(async move |this, cx| {
            while let Ok(event) = events.recv().await {
                let alive = this.update(cx, |this, cx| {
                    let Some(lib) = this.lib.as_mut().filter(|l| l.info.id == library) else { return false };
                    let chat = if about_paper { &mut lib.paper_chat } else { &mut lib.library_chat };
                    if chat.generation != generation { return false }
                    // Keep following the answer while the reader is at the bottom.
                    let at_bottom = chat.scroll.offset().y <= -chat.scroll.max_offset().height + px(40.);
                    let mut done = false;
                    match event {
                        StreamEvent::Text(text) => {
                            if let Some(last) = chat.messages.last_mut() { last.content.push_str(&text) }
                        }
                        StreamEvent::Stop(reason) => {
                            chat.note = ai::stop_note(&reason).unwrap_or_default().to_string();
                            done = true;
                        }
                        StreamEvent::Error(error) => {
                            chat.error = error;
                            done = true;
                        }
                    }
                    if done {
                        chat.busy = false;
                        // An answer that never started leaves no empty bubble behind.
                        if chat.messages.last().is_some_and(|m| !m.from_user && m.content.is_empty()) {
                            chat.messages.pop();
                            if chat.note.is_empty() && chat.error.is_empty() { chat.note = "模型没有返回内容。".into() }
                        }
                    }
                    if at_bottom { chat.scroll.scroll_to_bottom() }
                    cx.notify();
                    !done
                });
                if !matches!(alive, Ok(true)) { break }
            }
        })
        .detach();
    }

    // ---- dialogs: building and submitting (rendering is in dialogs.rs)

    pub fn modal_input(&mut self, placeholder: &'static str, text: &str, multiline: bool, window: &mut Window, cx: &mut Context<Self>) -> Entity<TextInput> {
        let input = cx.new(|cx| {
            let input = TextInput::new(placeholder, cx);
            if multiline { input.multiline(3).with_text(text) } else { input.with_text(text) }
        });
        if !multiline {
            let subscription = cx.subscribe_in(&input, window, |this, _, event: &InputEvent, window, cx| {
                if *event == InputEvent::Submit { this.run(Cmd::SubmitModal, window, cx) }
            });
            self.modal_subscriptions.push(subscription);
        }
        input
    }

    fn library_dialog(&mut self, editing: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        self.modal_subscriptions.clear();
        self.modal_error.clear();
        let current = editing.as_ref().and_then(|id| self.libraries.iter().find(|l| &l.id == id).or(self.lib.as_ref().map(|l| &l.info)).cloned());
        let (name, topic, color) = current.map_or((String::new(), String::new(), libraries::COLORS[self.libraries.len() % libraries::COLORS.len()].to_string()), |l| (l.name, l.topic, l.color));
        let name = self.modal_input("例如：植物根尖单细胞", &name, false, window, cx);
        let topic = self.modal_input("这个文献库收集什么？关注哪些问题？（会作为 AI 提问的背景）", &topic, true, window, cx);
        name.read(cx).focus(window);
        self.modal = Some(Modal::Library { editing, name, topic, color });
    }

    fn paper_dialog(&mut self, id: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        self.modal_subscriptions.clear();
        self.modal_error.clear();
        let Some(lib) = &self.lib else { return };
        let paper = id.as_ref().and_then(|id| lib.paper(id)).cloned().unwrap_or_default();
        let fields = lib.fields.clone();
        let title = self.modal_input("文献题目", &paper.title, true, window, cx);
        title.update(cx, |input, _| input.min_lines = 2);
        let abstract_text = self.modal_input("摘要", &paper.abstract_text, true, window, cx);
        abstract_text.update(cx, |input, _| input.min_lines = 4);
        let mut form = PaperForm {
            id,
            title,
            authors: self.modal_input("作者，用分号分隔：Zhang, Wei; Li, Na", &paper.authors, false, window, cx),
            year: self.modal_input("例如 2024", &paper.year.map(|y| y.to_string()).unwrap_or_default(), false, window, cx),
            journal: self.modal_input("期刊或会议", &paper.journal, false, window, cx),
            doi: self.modal_input("10.xxxx/…", &paper.doi, false, window, cx),
            pmid: self.modal_input("PubMed 编号", &paper.pmid, false, window, cx),
            url: self.modal_input("https://…", &paper.url, false, window, cx),
            tags: self.modal_input("标签，用逗号分隔", &paper.tags.join(", "), false, window, cx),
            abstract_text,
            fields: vec![],
            selects: BTreeMap::new(),
        };
        for field in fields {
            let values = paper.fields.get(&field.id).cloned().unwrap_or_default();
            match field.kind {
                FieldKind::Select => {
                    if let Some(value) = values.first() { form.selects.insert(field.id.clone(), value.clone()); }
                }
                FieldKind::Text => form.fields.push((field.id.clone(), self.modal_input("", &values.join(" "), false, window, cx))),
                FieldKind::Tags => form.fields.push((field.id.clone(), self.modal_input("多个值用逗号分隔", &values.join(", "), false, window, cx))),
            }
        }
        form.title.read(cx).focus(window);
        self.modal = Some(Modal::Paper(Box::new(form)));
    }

    fn fields_dialog(&mut self, editing: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        self.modal_subscriptions.clear();
        self.modal_error.clear();
        let field = editing.as_ref().and_then(|id| self.lib.as_ref()?.fields.iter().find(|f| &f.id == id).cloned()).unwrap_or(FieldDef { in_table: true, kind: FieldKind::Tags, ..Default::default() });
        let name = self.modal_input("词条名称，例如：物种、研究方法、所属课题", &field.name, false, window, cx);
        let options = self.modal_input("可选值，用逗号分隔（单选必填）", &field.options.join(", "), false, window, cx);
        self.modal = Some(Modal::Fields { editing, name, options, kind: field.kind, in_table: field.in_table });
    }

    fn submit_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = |input: &Entity<TextInput>, cx: &App| input.read(cx).text().trim().to_string();
        self.modal_error.clear();
        let Some(modal) = self.modal.as_mut() else { return };
        match modal {
            Modal::Confirm { cmd, .. } => {
                let cmd = cmd.clone();
                self.run(cmd, window, cx);
            }
            Modal::Library { editing, name, topic, color } => {
                let (name, topic, color) = (text(name, cx), text(topic, cx), color.clone());
                let result = match editing.clone() {
                    None => libraries::create(&self.root, &name, &topic, &color).map(|_| ()),
                    Some(id) => {
                        let Some(mut info) = self.libraries.iter().find(|l| l.id == id).cloned() else { return };
                        (info.name, info.topic, info.color) = (name, topic, color);
                        libraries::save(&info).map(|_| {
                            if let Some(lib) = self.lib.as_mut().filter(|l| l.info.id == info.id) {
                                theme::set_accent(&info.color);
                                window.set_window_title(&format!("{} — Wisp Paper", info.name));
                                // A changed topic changes what the assistant is told.
                                lib.library_chat.ctx = None;
                                lib.paper_chat.ctx = None;
                                lib.info = LibraryInfo { paper_count: lib.info.paper_count, opened_at: lib.info.opened_at.clone(), ..info };
                            }
                        })
                    }
                };
                match result {
                    Ok(()) => {
                        self.libraries = libraries::list(&self.root);
                        self.run(Cmd::CloseModal, window, cx);
                    }
                    Err(err) => self.modal_error = format!("{err:#}"),
                }
            }
            Modal::AddById { input, busy } => {
                if *busy { return }
                let Some(id) = lookup::parse_identifier(&text(input, cx)) else {
                    self.modal_error = "没有认出 DOI 或 PMID。DOI 形如 10.1038/s41586-020-2649-2，PMID 是一串数字。".into();
                    return;
                };
                *busy = true;
                let library = self.lib.as_ref().map(|l| l.info.id.clone()).unwrap_or_default();
                cx.spawn_in(window, async move |this, cx| {
                    let found = cx.background_executor().spawn(async move { crate::net::lookup_paper(&id) }).await;
                    this.update_in(cx, |this, window, cx| {
                        if !matches!(this.modal, Some(Modal::AddById { .. })) || this.lib.as_ref().is_none_or(|l| l.info.id != library) { return }
                        if let Some(Modal::AddById { busy, .. }) = this.modal.as_mut() { *busy = false }
                        let added = found.and_then(|paper| this.lib.as_mut().unwrap().store.add_paper(paper).map_err(|e| format!("{e:#}")));
                        match added {
                            Ok(paper) => {
                                this.change(cx, |_| Ok(()));
                                this.run(Cmd::CloseModal, window, cx);
                                this.select(Some(paper.id), window, cx);
                                this.say(format!("已添加：{}", paper.title), cx);
                            }
                            Err(err) => this.modal_error = err,
                        }
                        cx.notify();
                    })
                    .ok();
                })
                .detach();
            }
            Modal::Paper(form) => {
                let Some(lib) = self.lib.as_mut() else { return };
                let mut paper = form.id.as_ref().and_then(|id| lib.paper(id)).cloned().unwrap_or_default();
                let year = text(&form.year, cx);
                paper.year = match year.parse::<i32>() {
                    Ok(year) => Some(year),
                    Err(_) if year.is_empty() => None,
                    Err(_) => {
                        self.modal_error = "年份应为数字，例如 2024；不确定可以留空。".into();
                        return;
                    }
                };
                paper.title = text(&form.title, cx);
                paper.authors = text(&form.authors, cx);
                paper.journal = text(&form.journal, cx);
                paper.doi = text(&form.doi, cx);
                paper.pmid = text(&form.pmid, cx);
                paper.url = text(&form.url, cx);
                paper.tags = split_list(&text(&form.tags, cx));
                paper.abstract_text = text(&form.abstract_text, cx);
                paper.fields.clear();
                for (id, input) in &form.fields {
                    let value = text(input, cx);
                    let kind = lib.fields.iter().find(|f| &f.id == id).map(|f| f.kind);
                    let values = if kind == Some(FieldKind::Tags) { split_list(&value) } else { clean_strings([value]) };
                    if !values.is_empty() { paper.fields.insert(id.clone(), values); }
                }
                for (id, value) in &form.selects {
                    paper.fields.insert(id.clone(), vec![value.clone()]);
                }
                let editing = form.id.is_some();
                let result = if editing { lib.store.save_paper(&mut paper).map(|_| paper.id.clone()) } else { lib.store.add_paper(paper).map(|p| p.id) };
                match result {
                    Ok(id) => {
                        self.change(cx, |_| Ok(()));
                        self.run(Cmd::CloseModal, window, cx);
                        if !editing { self.select(Some(id), window, cx) }
                        self.say(if editing { "修改已保存。" } else { "已添加。" }, cx);
                    }
                    Err(err) => self.modal_error = format!("{err:#}"),
                }
            }
            Modal::Fields { editing, name, options, kind, in_table } => {
                let mut field = FieldDef { id: editing.clone().unwrap_or_default(), name: text(name, cx), kind: *kind, options: split_list(&text(options, cx)), in_table: *in_table };
                let Some(lib) = self.lib.as_mut() else { return };
                match lib.store.save_field(&mut field) {
                    Ok(()) => {
                        self.change(cx, |_| Ok(()));
                        self.fields_dialog(None, window, cx);
                    }
                    Err(err) => self.modal_error = format!("{err:#}"),
                }
            }
        }
        cx.notify();
    }
}

/// Debug builds only: `WISP_PAPER_DEV=open:0,select:1,tab:pdf` puts the
/// window into a state at start, for screenshots while developing.
#[cfg(debug_assertions)]
impl Workspace {
    pub fn dev_start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Ok(steps) = std::env::var("WISP_PAPER_DEV") else { return };
        for step in steps.split(',') {
            let (name, arg) = step.split_once(':').unwrap_or((step, ""));
            let cmd = match name {
                "open" => self.libraries.get(arg.parse::<usize>().unwrap_or(0)).map(|l| Cmd::OpenLibrary(l.id.clone())),
                "select" => self.lib.as_ref().and_then(|l| l.shown.get(arg.parse::<usize>().unwrap_or(0)).map(|&i| Cmd::Select(l.papers[i].id.clone()))),
                "tab" => Some(Cmd::SetTab(match arg { "pdf" => Tab::Pdf, "ask" => Tab::Ask, _ => Tab::Info })),
                "page" => Some(Cmd::SetPage(match arg { "ask" => Page::Ask, "trash" => Page::Trash, _ => Page::Papers })),
                "wide" => Some(Cmd::ToggleWide),
                "menu" => {
                    let kind = match arg { "sort" => MenuKind::Sort, "year" => MenuKind::Filter(FilterKey::Year), _ => MenuKind::Add };
                    self.open_menu(kind, point(px(20.), px(120.)), cx);
                    return;
                }
                "search" => {
                    if let Some(lib) = self.lib.as_mut() {
                        lib.search.update(cx, |input, cx| input.set_text(arg, cx));
                        lib.filters.search = arg.to_string();
                        lib.refilter();
                    }
                    None
                }
                "settings" => Some(Cmd::OpenSettings(match arg {
                    "appearance" => SettingsPage::Appearance,
                    "shortcuts" => SettingsPage::Shortcuts,
                    "ai" => SettingsPage::Ai,
                    "about" => SettingsPage::About,
                    _ => SettingsPage::General,
                })),
                // Saved like the choice in 设置 › 外观: "light", "dark", or empty for the system's.
                "theme" => Some(Cmd::SetAppearance(arg.to_string())),
                "record" => Some(Cmd::RecordShortcut(arg.to_string())),
                "update" => Some(Cmd::CheckUpdate),
                "modal" => match arg {
                    "fields" => Some(Cmd::ManageFields),
                    "library" => Some(Cmd::NewLibrary),
                    "add" => Some(Cmd::AddById),
                    "paper" => self.lib.as_ref().and_then(|l| l.selected.clone()).map(Cmd::EditPaper),
                    _ => None,
                },
                _ => None,
            };
            if let Some(cmd) = cmd { self.run(cmd, window, cx) }
        }
    }
}
