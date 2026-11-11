//! `Workspace`, the window's root view: the home page of libraries, or one
//! open library. All state lives here; every click becomes a `Cmd` handled
//! by `run`, and the other modules render parts of it.

use crate::input::{InputEvent, TextInput};
use crate::pdf::PdfView;
use crate::settings::{SettingsPage, SettingsView, UpdateState};
use crate::shortcuts::{self, Dismiss};
use crate::theme;
use gpui::*;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use serde_json::Value;
use wisp_core::ai::{self, ChatEvent, Protocol, Scope, ToolCall};
use wisp_core::filter::{self, Filters, View};
use wisp_core::libraries::{self, LibraryInfo};
use wisp_core::lookup::{self, Identifier};
use wisp_core::model::*;
pub(crate) use wisp_core::settings::default_root;
use wisp_core::settings::{Settings, Task, settings_file};
use wisp_core::store::{Conversation, ConversationInfo, ImportBatch, SavedSearch, Store, StructureInfo, Suggestion, TrashEntry, translation_key};
use wisp_core::{export, fulltext, import, structure, time, tools};

/// What the middle of an open library shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Papers,
    /// 文献库提问: questions over the current list.
    Ask,
    Trash,
    /// 筛选: the papers waiting for a decision, with what the assistant suggests for each.
    Screen,
    /// The guide of a new library: where to start, and searches composed with the assistant.
    Guide,
    /// 检索与导入: saved searches for the literature databases and what they brought in.
    Sources,
    /// 写作: the manuscript, and beside it how the library's papers put the same part.
    Write,
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
    /// Which model a task uses (设置 › AI).
    TaskModel(Task),
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
    /// Show another model service's fields on the AI page.
    SelectProfile(usize),
    AddProfile,
    AskRemoveProfile(usize),
    RemoveProfile(usize),
    /// How hard the model on show thinks, a key of `ai::EFFORTS`.
    SetEffort(String),
    /// Bind a task to a profile id; an empty id follows the default.
    SetTaskModel(Task, String),
    SetScreenWorkers(usize),
    ResetTranslationPrompt,
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
    /// Show the selected paper's title and abstract translated, or stop showing it.
    ToggleTranslation,
    RetryTranslation,
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
    /// Stop the answer being given.
    StopChat(bool),
    /// Show the past conversations, or go back to the one in hand.
    ToggleHistory(bool),
    /// Take up a past conversation again.
    OpenChat(bool, String),
    AskRemoveChat(bool, String),
    RemoveChat(bool, String),
    /// Take the n-th of the slash commands on offer.
    PickSlash(bool, usize),
    /// Choose pictures to go with the next question.
    AttachImages(bool),
    RemoveAttachment(bool, usize),
    /// The person's answer to the assistant wanting to change the library.
    AnswerTools(bool, Consent),
    /// Unfold or fold the tool calls that start at this place in the conversation.
    ToggleTools(bool, usize),
    CopyText(String),
    /// Show every source of an answer (by its place in the conversation), or the first few again.
    ToggleSources(bool, usize),
    /// Open a paper; with a page, its PDF there; with a quoted sentence too, that sentence highlighted.
    GotoSource(String, Option<usize>, String),
    // Searches and imports
    NewSearch,
    EditSearch(String),
    AskRemoveSearch(String),
    RemoveSearch(String),
    /// Run a saved search without importing, to see what it finds.
    TrySearch(String),
    CloseTrial,
    RunSearch(String),
    RunAllSearches,
    AskUndoImport(String),
    UndoImport(String),
    PickSource(String),
    // The guide
    /// Show the guide; true starts at the choice a new library gets.
    OpenGuide(bool),
    GuideSearch,
    GuideToggleSource(String),
    GuideGenerate,
    GuideToggleIdea(usize),
    GuideImport,
    // Screening
    /// Ask the assistant about the pending papers it has not judged yet.
    ScreenStart,
    ScreenStop,
    /// The person's decision on a paper, made on the screening page.
    ScreenDecide(String, String),
    /// Show only the queue's papers with this suggestion (a key of `SCREENINGS`), those without one (`none`), or all (empty).
    ScreenFilter(String),
    /// Take the assistant's suggestion for every waiting paper it gave this verdict, after asking.
    AskScreenAccept(String),
    ScreenAccept(String),
    /// How often the open library runs its searches on its own, in hours; 0 turns it off.
    SetAutoSync(u32),
    ToggleSearch(String),
    // Writing
    /// Write another part of the manuscript (a key of `structure::SECTIONS`).
    WriteSection(String),
    /// Put a citation in at the caret.
    WriteCite(String),
    /// Have the model sort the full texts into parts (false), or read the pages of the files that gave no text (true).
    WriteRun(bool),
    WriteStop,
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

/// A search the assistant proposed in the guide, with what a trial run of it found.
pub struct GuideIdea {
    pub idea: ai::QueryIdea,
    /// Whether the person wants it saved and run.
    pub chosen: bool,
    pub trial: Option<Result<Value, String>>,
}

/// The guide page of a library: a choice of where to start, then searches
/// composed from a topic for the person to judge.
pub struct Guide {
    /// Still at the first choice (a new library), not yet composing searches.
    pub choosing: bool,
    pub keywords: Entity<TextInput>,
    /// The databases to compose searches for.
    pub kinds: Vec<String>,
    pub busy: bool,
    pub error: String,
    pub ideas: Vec<GuideIdea>,
    /// Bumped with every generation, so answers to an earlier one are dropped.
    round: u64,
}

impl Guide {
    fn new(topic: &str, cx: &mut App) -> Guide {
        let keywords = cx.new(|cx| TextInput::new("研究主题或关键词，中文英文都可以。例如：植物根尖 单细胞转录组 干细胞微环境", cx).multiline(2).with_text(topic));
        Guide { choosing: false, keywords, kinds: vec!["pubmed".into()], busy: false, error: String::new(), ideas: vec![], round: 0 }
    }
}

/// The screening page: what to screen against, and the assistant's run
/// over the papers still waiting for a decision.
pub struct Screen {
    pub criteria: Entity<TextInput>,
    /// Papers the assistant has yet to be asked about in this run, last first.
    todo: Vec<String>,
    /// The papers being read right now, in the order they were started.
    pub reading: Vec<String>,
    pub total: usize,
    pub done: usize,
    /// Papers of the run the assistant gave no usable answer for.
    pub failed: usize,
    /// Failures since the last answer that could be used.
    streak: usize,
    /// The model the run was started with.
    pub model: String,
    /// Which part of the queue is on show: a suggested verdict, `none` for the papers without a suggestion, empty for all.
    filter: String,
    pub error: String,
    /// Bumped when a run starts or stops, so answers to an earlier one are dropped.
    run: u64,
}

impl Screen {
    fn new(criteria: &str, cx: &mut App) -> Screen {
        let criteria = cx.new(|cx| TextInput::new("筛选标准：什么样的文献应当收进这个文献库。留空则按文献库的主题判断。", cx).multiline(2).with_text(criteria));
        Screen { criteria, todo: vec![], reading: vec![], total: 0, done: 0, failed: 0, streak: 0, model: String::new(), filter: String::new(), error: String::new(), run: 0 }
    }

    /// Whether the assistant is working through the papers right now.
    pub fn running(&self) -> bool {
        !self.reading.is_empty() || !self.todo.is_empty()
    }
}

/// 写作's groundwork under way: a model sorting full texts into the parts
/// of a paper, or reading the pages of files that gave no text.
#[derive(Default)]
pub struct Sorting {
    /// The run reads pages; otherwise it sorts parts.
    pub reading: bool,
    /// The files still to do, last first.
    todo: Vec<String>,
    /// The files being worked on.
    pub working: Vec<String>,
    pub total: usize,
    pub done: usize,
    pub failed: usize,
    streak: usize,
    pub model: String,
    pub error: String,
    /// Bumped when a run starts or stops, so what still arrives for an earlier one is dropped.
    run: u64,
}

impl Sorting {
    pub fn running(&self) -> bool {
        !self.working.is_empty() || !self.todo.is_empty()
    }
}

/// The pages of one file a model is asked to read: a paper is rarely longer, a book is not what this is for.
const READ_PAGES: usize = 40;

/// The writing page: the part of the manuscript being written, and the
/// sentences the library's papers have in the same part.
pub struct Writing {
    /// The part being written, a key of `structure::SECTIONS`.
    pub section: String,
    pub editor: Entity<TextInput>,
    /// Words to look for, instead of what is being written.
    pub find: Entity<TextInput>,
    /// What the candidates on show were looked for: the part and the words.
    asked: Option<(String, String)>,
    /// The English words a search was made with, when they are not the person's own.
    pub words: String,
    /// What `find_sentences` gave.
    pub candidates: Option<Result<Value, String>>,
    pub searching: bool,
    round: u64,
    /// English search words a model gave for something written in Chinese; `None` while it is asked.
    keywords: HashMap<String, Option<String>>,
    /// How each full text was sorted into parts, by file id.
    pub structures: BTreeMap<String, StructureInfo>,
    pub sorting: Sorting,
    pub scroll: ScrollHandle,
}

/// A run of the screening assistant gives up after this many papers in a
/// row without a usable answer: a wrong key or a service that is down
/// would fail every paper the same way.
const SCREEN_GIVE_UP: usize = 3;

/// Where the translation of one text stands.
pub enum Translated {
    Busy,
    Done(String),
    Failed(String),
}

/// The steps one question may take. Each is a request to the model; all but
/// the last end in tool calls. With the results before the last one the
/// model is told to answer from what it has (`ai::LAST_STEP_NOTE`).
const MAX_CHAT_STEPS: usize = 24;

/// The pictures one question may carry.
pub const MAX_ATTACHMENTS: usize = 8;

/// A picture in a conversation: as it is drawn and sent, and the id it is kept under.
#[derive(Clone)]
pub struct ChatImage {
    pub id: String,
    pub image: Arc<Image>,
}

/// The pictures as a request carries them.
fn wire_images(images: &[ChatImage]) -> Vec<ai::ImageData> {
    images.iter().map(|picture| ai::ImageData { media_type: picture.image.format.mime_type().to_string(), data: ai::base64(&picture.image.bytes) }).collect()
}

/// Where a picture to attach comes from: the clipboard, or a file still to be read.
pub enum Picture {
    Bytes(Vec<u8>),
    File(PathBuf),
}

/// What a conversation shows, in order.
pub enum ChatItem {
    /// A question, as typed, with the pictures that went with it.
    User { text: String, images: Vec<ChatImage> },
    /// Text of the model; one answer may come in several pieces with tool calls between them.
    Answer(String),
    /// A tool the model called, and what came back (summary, failed) once it has run.
    Tool { call: String, result: Option<(String, bool)> },
}

/// What the person says to the assistant wanting to change the library.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Consent {
    Once,
    /// This and every later change in the same conversation.
    Always,
    Deny,
}

/// A tool call made ready to run.
pub struct PreparedCall {
    id: String,
    name: String,
    args: Result<Value, String>,
    writes: bool,
    /// The person is asked before it runs (`ToolDef::confirm`).
    pub confirm: bool,
    /// The call as the window words it.
    pub line: String,
}

/// The calls of a turn that wait for the person's consent, and where their lines stand in the conversation.
pub struct PendingTools {
    pub calls: Vec<PreparedCall>,
    first: usize,
}

/// One conversation: with the library (the current list) or with one paper.
pub struct Chat {
    pub input: Entity<TextInput>,
    pub messages: Vec<ChatItem>,
    /// The conversation as the model's protocol carries it: questions, the
    /// model's turns as it sent them, tool results. Only ever appended to.
    wire: Vec<Value>,
    /// The protocol `wire` is written in.
    protocol: Option<Protocol>,
    /// Requests made for the question being answered.
    pub steps: usize,
    /// What the model was given, built with the first question.
    pub ctx: Option<ai::Context>,
    pub busy: bool,
    pub error: String,
    /// Why the last answer ended early, if it did; while one is awaited, what it is waiting for.
    pub note: String,
    pub scroll: ScrollHandle,
    /// The answers (by their place in `messages`) whose sources are all on show.
    pub sources_open: HashSet<usize>,
    /// Bumped by 新对话 and by stopping, so what still arrives for the old run is dropped.
    pub generation: u64,
    /// The conversation in the library's history this is; empty until a question was asked.
    pub id: String,
    created_at: String,
    /// What the labels of a conversation taken up again name, until its context is built anew.
    known: Vec<ai::Source>,
    /// The pictures already kept with the conversation.
    stored_images: HashSet<String>,
    /// Pictures that go with the next question, and how many more are still being read.
    pub attachments: Vec<ChatImage>,
    pub attaching: usize,
    /// Which of the slash commands on offer is picked.
    pub slash: usize,
    /// The list of commands was closed with Escape; typing opens it again.
    slash_closed: bool,
    /// The past conversations, while their list is on show.
    pub history: Option<Vec<ConversationInfo>>,
    /// The tool calls waiting for the person's consent.
    pub pending: Option<PendingTools>,
    /// The person allowed every change for the rest of this conversation.
    allow_changes: bool,
    /// The calls running now, which get an answer of their own if the person stops.
    running: Vec<String>,
    /// The groups of tool lines (by the place of their first) that are unfolded.
    pub tools_open: HashSet<usize>,
    /// When the question being answered was asked.
    pub started: Option<Instant>,
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
    Search { editing: Option<String>, kind: String, name: Entity<TextInput>, query: Entity<TextInput>, max: Entity<TextInput> },
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
    pub guide: Guide,
    pub screen: Screen,
    /// The assistant's suggestion for each paper that has one.
    pub suggestions: HashMap<String, Suggestion>,
    /// Translations asked for in this window, by `store::translation_key`.
    pub translations: HashMap<String, Translated>,
    /// Whether the detail panel shows the selected paper translated. It
    /// stays on from paper to paper, so each one opened is translated.
    pub show_translation: bool,
    pub searches: Vec<SavedSearch>,
    /// The latest import batches, newest first.
    pub batches: Vec<ImportBatch>,
    /// The trial run on show: which search, and what it found once it is back.
    pub trial: Option<(String, Option<Result<Value, String>>)>,
    /// Searches being run right now.
    pub running: HashSet<String>,
    /// How many dropped or picked files are still being read.
    pub importing: usize,
    pub writing: Writing,
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
            input.accepts_images = true;
            input
        });
        Chat {
            input,
            messages: vec![],
            wire: vec![],
            protocol: None,
            steps: 0,
            ctx: None,
            busy: false,
            error: String::new(),
            note: String::new(),
            scroll: ScrollHandle::new(),
            sources_open: Default::default(),
            generation: 0,
            id: String::new(),
            created_at: String::new(),
            known: vec![],
            stored_images: Default::default(),
            attachments: vec![],
            attaching: 0,
            slash: 0,
            slash_closed: false,
            history: None,
            pending: None,
            allow_changes: false,
            running: vec![],
            tools_open: Default::default(),
            started: None,
        }
    }

    /// What the labels in the answers name.
    pub fn sources(&self) -> &[ai::Source] {
        self.ctx.as_ref().map_or(&self.known, |ctx| &ctx.sources)
    }

    /// The slash commands to offer for what is in the box right now.
    pub fn slash_list(&self, cx: &App) -> Vec<&'static ai::Slash> {
        if self.slash_closed { vec![] } else { ai::slash_matches(self.input.read(cx).text()) }
    }

    /// Have the context built again with the next question. The labels
    /// stay what they were: the answers already given use them.
    fn forget_context(&mut self) {
        if let Some(ctx) = self.ctx.take() { self.known = ctx.sources }
    }

    /// An answer piece that never got any text leaves no empty bubble behind.
    fn drop_empty_answer(&mut self) {
        if matches!(self.messages.last(), Some(ChatItem::Answer(text)) if text.is_empty()) { self.messages.pop(); }
    }

    /// The question has been answered, or given up on.
    fn end_run(&mut self) {
        self.busy = false;
        self.started = None;
        self.pending = None;
        self.running.clear();
        self.drop_empty_answer();
        if self.note.is_empty() && self.error.is_empty() && !matches!(self.messages.last(), Some(ChatItem::Answer(_))) { self.note = "模型没有返回内容。".into() }
    }

    /// The conversation as it is kept in the library: what was said, and
    /// the papers its answers cite, so the labels still lead somewhere later.
    fn transcript(&self) -> ai::Transcript {
        let mut cited: HashSet<String> = HashSet::new();
        let items = self
            .messages
            .iter()
            .filter_map(|item| match item {
                ChatItem::User { text, images } => Some(ai::Said::User { text: text.clone(), images: images.iter().map(|picture| picture.id.clone()).collect() }),
                ChatItem::Answer(text) if !text.is_empty() => {
                    cited.extend(ai::citations(text).into_iter().map(|citation| citation.label));
                    Some(ai::Said::Answer { text: text.clone() })
                }
                ChatItem::Answer(_) => None,
                ChatItem::Tool { call, result } => {
                    let (result, failed) = result.clone().unwrap_or_default();
                    Some(ai::Said::Tool { call: call.clone(), result, failed })
                }
            })
            .collect();
        ai::Transcript { items, sources: self.sources().iter().filter(|source| cited.contains(&source.label)).cloned().collect() }
    }

    fn reset(&mut self, cx: &mut App) {
        self.messages.clear();
        self.wire.clear();
        self.protocol = None;
        self.steps = 0;
        self.ctx = None;
        self.busy = false;
        self.error.clear();
        self.note.clear();
        self.sources_open.clear();
        self.generation += 1;
        self.id.clear();
        self.created_at.clear();
        self.known.clear();
        self.stored_images.clear();
        self.attachments.clear();
        self.attaching = 0;
        self.slash = 0;
        self.slash_closed = false;
        self.history = None;
        self.pending = None;
        self.allow_changes = false;
        self.running.clear();
        self.tools_open.clear();
        self.started = None;
        self.input.update(cx, |input, cx| {
            input.choosing = false;
            input.set_text("", cx);
        });
    }
}

impl Library {
    pub fn chat(&self, about_paper: bool) -> &Chat {
        if about_paper { &self.paper_chat } else { &self.library_chat }
    }

    fn chat_mut(&mut self, about_paper: bool) -> &mut Chat {
        if about_paper { &mut self.paper_chat } else { &mut self.library_chat }
    }

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

    /// The papers waiting for a decision, as the screening page lists them:
    /// those with a suggestion first (to include, related, uncertain,
    /// irrelevant; the surer ones first), then the rest, newest first.
    pub fn screen_queue(&self) -> Vec<&Paper> {
        let rank = |p: &Paper| match self.suggestions.get(&p.id) {
            Some(s) => (SCREENINGS.iter().position(|(key, _)| *key == s.verdict).unwrap_or(9), -s.confidence.unwrap_or(0.)),
            None => (99, 0.),
        };
        let mut queue: Vec<&Paper> = self.papers.iter().filter(|p| p.screening == "pending").collect();
        // Stable: within a group the library's own order holds.
        queue.sort_by(|a, b| rank(a).partial_cmp(&rank(b)).unwrap_or(std::cmp::Ordering::Equal));
        queue
    }

    /// Which part of the queue a paper belongs to: its suggested verdict, or `none`.
    pub fn screen_group(&self, paper_id: &str) -> &str {
        self.suggestions.get(paper_id).map_or("none", |s| s.verdict.as_str())
    }

    /// The part of the queue on show, and its filter. A part that has
    /// emptied (the last of its papers was decided) gives way to the whole queue.
    pub fn screen_view(&self) -> (Vec<&Paper>, &str) {
        let queue = self.screen_queue();
        let filter = self.screen.filter.as_str();
        if filter.is_empty() || !queue.iter().any(|p| self.screen_group(&p.id) == filter) { return (queue, "") }
        (queue.into_iter().filter(|p| self.screen_group(&p.id) == filter).collect(), filter)
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
        self.searches = self.store.list_searches()?;
        self.suggestions = self.store.list_suggestions()?.into_iter().map(|s| (s.paper_id.clone(), s)).collect();
        self.batches = self.store.list_batches(30)?;
        self.writing.structures = self.store.structures()?;
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
        wisp_net::set_proxy(&settings.proxy);
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
        this.watch_searches(cx);
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
            MenuKind::TaskModel(task) => {
                let bound = self.settings.tasks.get(task).to_string();
                // Questions always have a model: the first one until another is chosen.
                let chosen = if task == Task::Chat { self.settings.profile_for(task).map(|p| p.id.clone()).unwrap_or_default() } else { bound };
                if task != Task::Chat { items.push(MenuItem { label: "与提问相同".into(), checked: chosen.is_empty(), note: None, cmd: Cmd::SetTaskModel(task, String::new()) }) }
                for profile in &self.settings.profiles {
                    items.push(MenuItem { label: profile.title().into(), checked: chosen == profile.id, note: Some(profile.model.clone().into()), cmd: Cmd::SetTaskModel(task, profile.id.clone()) });
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
            Cmd::SelectProfile(index) => self.select_profile(index, cx),
            Cmd::AddProfile => self.add_profile(cx),
            Cmd::AskRemoveProfile(index) => {
                let Some(profile) = self.settings.profiles.get(index) else { return };
                let serves: Vec<&str> = Task::ALL.into_iter().filter(|task| self.settings.tasks.get(*task) == profile.id).map(Task::label).collect();
                let after = if serves.is_empty() { String::new() } else { format!("它现在用于{}，删除后这些任务改用默认的模型。", serves.join("、")) };
                self.modal_error.clear();
                self.modal = Some(Modal::Confirm { title: format!("删除模型「{}」？", profile.title()), message: format!("它的地址、API Key 和设置会从 settings.json 里删除。{after}"), action: "删除".into(), cmd: Cmd::RemoveProfile(index) });
            }
            Cmd::RemoveProfile(index) => {
                self.run(Cmd::CloseModal, window, cx);
                self.remove_profile(index, cx);
            }
            Cmd::SetEffort(value) => self.set_effort(value, cx),
            Cmd::SetTaskModel(task, id) => {
                self.settings.tasks.set(task, &id);
                self.save_settings(cx);
            }
            Cmd::SetScreenWorkers(count) => {
                self.settings.screen_workers = count.clamp(1, wisp_core::settings::MAX_SCREEN_WORKERS);
                self.save_settings(cx);
            }
            Cmd::ResetTranslationPrompt => self.reset_translation_prompt(cx),

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
                    if page == Page::Write {
                        lib.writing.editor.read(cx).focus(window);
                        lib.writing.structures = lib.store.structures().unwrap_or_default();
                        lib.writing.asked = None;
                        self.writing_changed(cx);
                    }
                }
            }
            Cmd::WriteSection(section) => {
                if let Some(lib) = self.lib.as_mut().filter(|l| l.writing.section != section) {
                    let draft = lib.store.drafts().ok().and_then(|mut drafts| drafts.remove(&section)).unwrap_or_default();
                    lib.writing.section = section;
                    lib.writing.candidates = None;
                    lib.writing.editor.update(cx, |input, cx| input.set_text(&draft, cx));
                    lib.writing.editor.read(cx).focus(window);
                    self.writing_changed(cx);
                }
            }
            Cmd::WriteCite(cite) => {
                if let Some(lib) = self.lib.as_ref() {
                    lib.writing.editor.update(cx, |input, cx| {
                        let before = input.text()[..input.caret()].chars().last();
                        let space = if before.is_none_or(|c| c.is_whitespace()) { "" } else { " " };
                        input.insert(&format!("{space}({cite})"), cx);
                    });
                    lib.writing.editor.read(cx).focus(window);
                }
            }
            Cmd::WriteRun(reading) => self.write_run_start(reading, cx),
            Cmd::WriteStop => {
                if let Some(lib) = self.lib.as_mut() {
                    let sorting = &mut lib.writing.sorting;
                    sorting.run += 1;
                    sorting.todo.clear();
                    sorting.working.clear();
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
            Cmd::ToggleTranslation => {
                if let Some(lib) = self.lib.as_mut() { lib.show_translation = !lib.show_translation }
                self.translate_selected(true, cx);
            }
            Cmd::RetryTranslation => self.translate_selected(true, cx),

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
                // An answer still arriving is stopped first, so that what there is of it is kept.
                self.stop_chat(paper, cx);
                if let Some(lib) = self.lib.as_mut() {
                    let chat = if paper { &mut lib.paper_chat } else { &mut lib.library_chat };
                    chat.reset(cx);
                    chat.input.read(cx).focus(window);
                }
            }
            Cmd::StopChat(paper) => self.stop_chat(paper, cx),
            Cmd::ToggleHistory(paper) => {
                if let Some(lib) = self.lib.as_mut() {
                    let about = if paper { lib.selected.clone().unwrap_or_default() } else { String::new() };
                    let listed = if lib.chat(paper).history.is_some() { Ok(None) } else { lib.store.list_conversations(&about).map(Some) };
                    match listed {
                        Ok(list) => lib.chat_mut(paper).history = list,
                        Err(err) => self.fail(format!("{err:#}"), cx),
                    }
                }
            }
            Cmd::OpenChat(paper, id) => {
                self.stop_chat(paper, cx);
                self.open_chat(paper, &id, cx);
                if let Some(lib) = self.lib.as_ref() { lib.chat(paper).input.read(cx).focus(window) }
            }
            Cmd::AskRemoveChat(paper, id) => {
                let Some(title) = self.lib.as_ref().and_then(|l| l.chat(paper).history.as_ref()?.iter().find(|c| c.id == id).map(|c| c.title.clone())) else { return };
                self.modal_error.clear();
                self.modal = Some(Modal::Confirm { title: "删除这次对话？".into(), message: format!("「{title}」连同其中的图片会从这个文献库的历史对话里删除，不能恢复。文献库里的文献不受影响。"), action: "删除".into(), cmd: Cmd::RemoveChat(paper, id) });
            }
            Cmd::RemoveChat(paper, id) => {
                if let Some(lib) = self.lib.as_mut() {
                    let about = if paper { lib.selected.clone().unwrap_or_default() } else { String::new() };
                    let result = lib.store.remove_conversation(&id).and_then(|()| lib.store.list_conversations(&about));
                    let chat = lib.chat_mut(paper);
                    // The conversation in hand, once removed, would only be saved again with the next answer.
                    if chat.id == id {
                        let listed = chat.history.take();
                        chat.reset(cx);
                        chat.history = listed;
                    }
                    match result {
                        Ok(list) => chat.history = Some(list),
                        Err(err) => self.fail(format!("{err:#}"), cx),
                    }
                }
                self.run(Cmd::CloseModal, window, cx);
            }
            Cmd::PickSlash(paper, at) => {
                self.pick_slash(paper, at, cx);
                if let Some(lib) = self.lib.as_ref() { lib.chat(paper).input.read(cx).focus(window) }
            }
            Cmd::AttachImages(paper) => {
                let extensions: Vec<SharedString> = ["png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff"].into_iter().map(SharedString::from).collect();
                let picked = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: true, prompt: Some("添加图片".into()), extensions: Some(extensions) });
                cx.spawn(async move |this, cx| {
                    if let Ok(Ok(Some(paths))) = picked.await {
                        this.update(cx, |this, cx| this.attach_images(paper, paths.into_iter().map(Picture::File).collect(), cx)).ok();
                    }
                })
                .detach();
            }
            Cmd::RemoveAttachment(paper, at) => {
                if let Some(lib) = self.lib.as_mut() {
                    let chat = lib.chat_mut(paper);
                    if at < chat.attachments.len() { chat.attachments.remove(at); }
                }
            }
            Cmd::AnswerTools(paper, consent) => {
                let Some(pending) = self.lib.as_mut().and_then(|lib| {
                    let chat = lib.chat_mut(paper);
                    if consent == Consent::Always { chat.allow_changes = true }
                    chat.pending.take()
                }) else {
                    return;
                };
                self.execute_tools(paper, pending.calls, pending.first, consent != Consent::Deny, cx);
            }
            Cmd::ToggleTools(paper, at) => {
                if let Some(lib) = self.lib.as_mut() {
                    let chat = lib.chat_mut(paper);
                    if !chat.tools_open.remove(&at) { chat.tools_open.insert(at); }
                }
            }
            Cmd::CopyText(text) => {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
                self.say("已复制", cx);
            }
            Cmd::ToggleSources(paper, at) => {
                if let Some(lib) = self.lib.as_mut() {
                    let chat = if paper { &mut lib.paper_chat } else { &mut lib.library_chat };
                    if !chat.sources_open.remove(&at) { chat.sources_open.insert(at); }
                }
            }
            Cmd::GotoSource(id, page, quote) => {
                self.select(Some(id), window, cx);
                if let Some(page) = page && let Some(lib) = self.lib.as_mut() {
                    lib.tab = Tab::Pdf;
                    self.ensure_pdf(cx);
                    if let Some(pdf) = self.lib.as_ref().and_then(|l| l.pdf.clone()) {
                        pdf.update(cx, |pdf, cx| if quote.is_empty() { pdf.go_to(page, cx) } else { pdf.find(page, &quote, cx) });
                    }
                }
            }

            Cmd::NewSearch => self.search_dialog(None, window, cx),
            Cmd::EditSearch(id) => self.search_dialog(Some(id), window, cx),
            Cmd::AskRemoveSearch(id) => {
                let Some(search) = self.lib.as_ref().and_then(|l| l.searches.iter().find(|s| s.id == id)) else { return };
                self.modal_error.clear();
                self.modal = Some(Modal::Confirm { title: format!("删除检索式「{}」？", search.name), message: "它导入过的文献和运行记录都会保留，只是不再运行这个检索式。".into(), action: "删除".into(), cmd: Cmd::RemoveSearch(id) });
            }
            Cmd::RemoveSearch(id) => {
                self.change(cx, |lib| lib.store.remove_search(&id));
                self.run(Cmd::CloseModal, window, cx);
            }
            Cmd::TrySearch(id) => {
                let Some(lib) = self.lib.as_mut() else { return };
                let Some(search) = lib.searches.iter().find(|s| s.id == id) else { return };
                let args = serde_json::json!({"source": search.kind, "query": search.query, "limit": 20});
                lib.trial = Some((id.clone(), None));
                self.library_tool("search_online", args, cx, move |this, result, _| {
                    // Only when this trial is still the one on show.
                    if let Some(lib) = this.lib.as_mut() && let Some((shown, found)) = lib.trial.as_mut() && *shown == id { *found = Some(result) }
                });
            }
            Cmd::CloseTrial => {
                if let Some(lib) = self.lib.as_mut() { lib.trial = None }
            }
            Cmd::RunSearch(id) => {
                let Some(lib) = self.lib.as_mut() else { return };
                if !lib.running.insert(id.clone()) { return }
                self.library_tool("run_search", serde_json::json!({"id": id}), cx, move |this, result, cx| {
                    if let Some(lib) = this.lib.as_mut() { lib.running.remove(&id); }
                    // The run wrote through its own connection, failed runs included.
                    this.change(cx, |_| Ok(()));
                    match result {
                        Ok(run) => this.say(format!("{}：新增 {} 篇，跳过已有的 {} 篇", run["search"].as_str().unwrap_or("检索"), run["added"], run["skipped"]), cx),
                        Err(err) => this.fail(err, cx),
                    }
                });
            }
            Cmd::RunAllSearches => {
                let ids: Vec<String> = self.lib.as_ref().map(|l| l.searches.iter().filter(|s| s.enabled).map(|s| s.id.clone()).collect()).unwrap_or_default();
                for id in ids {
                    self.run(Cmd::RunSearch(id), window, cx);
                }
            }
            Cmd::AskUndoImport(id) => {
                let Some(batch) = self.lib.as_ref().and_then(|l| l.batches.iter().find(|b| b.id == id)) else { return };
                self.modal_error.clear();
                self.modal = Some(Modal::Confirm {
                    title: "撤销这次导入？".into(),
                    message: format!("{}\n\n这次导入新增的 {} 篇文献里，还没有处理过的会被删除；已经做过收录判断、读过、收藏、写过笔记、加过标签或附件的会保留。", batch.label, batch.added),
                    action: "撤销导入".into(),
                    cmd: Cmd::UndoImport(id),
                });
            }
            Cmd::UndoImport(id) => {
                let mut undone = None;
                if self.change(cx, |lib| lib.store.undo_batch(&id).map(|done| undone = Some(done)))
                    && let Some(done) = undone
                {
                    self.say(if done.kept.is_empty() { format!("已撤销：删除了 {} 篇", done.removed) } else { format!("已撤销：删除了 {} 篇，保留了处理过的 {} 篇", done.removed, done.kept.len()) }, cx);
                }
                self.run(Cmd::CloseModal, window, cx);
            }
            Cmd::PickSource(value) => {
                if let Some(Modal::Search { kind, .. }) = self.modal.as_mut() { *kind = value }
            }

            Cmd::SetAutoSync(hours) => {
                let Some(lib) = self.lib.as_mut() else { return };
                lib.info.auto_sync_hours = hours;
                if let Err(err) = libraries::save(&lib.info) { self.fail(format!("{err:#}"), cx) }
                self.libraries = libraries::list(&self.root);
            }
            Cmd::ToggleSearch(id) => {
                self.change(cx, |lib| {
                    let Some(mut search) = lib.searches.iter().find(|s| s.id == id).cloned() else { return Ok(()) };
                    search.enabled = !search.enabled;
                    lib.store.save_search(&mut search)
                });
            }
            Cmd::ScreenStart => self.screen_start(cx),
            Cmd::ScreenStop => {
                if let Some(lib) = self.lib.as_mut() {
                    // Answers still on their way belong to a run that is over.
                    lib.screen.run += 1;
                    lib.screen.todo.clear();
                    lib.screen.reading.clear();
                }
            }
            Cmd::ScreenDecide(id, verdict) => {
                let Some(lib) = self.lib.as_ref() else { return };
                // The paper after this one in the queue on show comes up next; the one before when it was the last.
                let queue: Vec<&str> = lib.screen_view().0.iter().map(|p| p.id.as_str()).collect();
                let next = queue.iter().position(|p| *p == id).and_then(|at| queue.get(at + 1).or(at.checked_sub(1).and_then(|before| queue.get(before)))).map(|p| p.to_string());
                // Agreeing with the assistant takes its reason along; deciding otherwise leaves the reason to the person.
                let reason = lib.suggestions.get(&id).filter(|s| s.verdict == verdict).map(|s| s.reason.clone()).unwrap_or_default();
                if self.edit_paper(&id, cx, |p| p.decide(&verdict, &reason)) { self.select(next, window, cx) }
            }
            Cmd::ScreenFilter(filter) => {
                if let Some(lib) = self.lib.as_mut() { lib.screen.filter = filter }
            }
            Cmd::AskScreenAccept(verdict) => {
                let Some(lib) = self.lib.as_ref() else { return };
                let count = lib.screen_queue().iter().filter(|p| lib.screen_group(&p.id) == verdict).count();
                if count == 0 { return }
                let name = label(&SCREENINGS, &verdict);
                self.modal_error.clear();
                self.modal = Some(Modal::Confirm {
                    title: format!("把这 {count} 篇都判断为「{name}」？"),
                    message: format!("助手对这 {count} 篇待判断文献的建议都是「{name}」。采纳后，它们的收录判断改为「{name}」，判断理由用助手给出的理由。之后仍然可以在每篇文献的详情里改回来。"),
                    action: format!("采纳 {count} 篇"),
                    cmd: Cmd::ScreenAccept(verdict),
                });
            }
            Cmd::ScreenAccept(verdict) => {
                self.run(Cmd::CloseModal, window, cx);
                let Some(lib) = self.lib.as_ref() else { return };
                let agreed: Vec<(String, String)> = lib.screen_queue().iter().filter(|p| lib.screen_group(&p.id) == verdict).map(|p| (p.id.clone(), lib.suggestions[&p.id].reason.clone())).collect();
                let done = self.change(cx, |lib| {
                    for (id, reason) in &agreed {
                        let mut paper = lib.store.get_paper(id)?;
                        paper.decide(&verdict, reason);
                        lib.store.save_paper(&mut paper)?;
                    }
                    Ok(())
                });
                if done {
                    self.select(None, window, cx);
                    self.say(format!("已把 {} 篇判断为「{}」。", agreed.len(), label(&SCREENINGS, &verdict)), cx);
                }
            }

            Cmd::OpenGuide(choosing) => {
                if let Some(lib) = self.lib.as_mut() {
                    lib.page = Page::Guide;
                    lib.guide.choosing = choosing;
                    if !choosing { lib.guide.keywords.read(cx).focus(window) }
                }
            }
            Cmd::GuideSearch => {
                if let Some(lib) = self.lib.as_mut() {
                    lib.guide.choosing = false;
                    lib.guide.keywords.read(cx).focus(window);
                }
            }
            Cmd::GuideToggleSource(kind) => {
                if let Some(lib) = self.lib.as_mut() {
                    let kinds = &mut lib.guide.kinds;
                    // At least one database stays chosen.
                    if let Some(at) = kinds.iter().position(|k| *k == kind) { if kinds.len() > 1 { kinds.remove(at); } } else { kinds.push(kind) }
                }
            }
            Cmd::GuideGenerate => self.guide_generate(cx),
            Cmd::GuideToggleIdea(index) => {
                if let Some(idea) = self.lib.as_mut().and_then(|l| l.guide.ideas.get_mut(index)) { idea.chosen = !idea.chosen }
            }
            Cmd::GuideImport => {
                let Some(lib) = self.lib.as_mut() else { return };
                let mut saved = vec![];
                let mut failure = None;
                for entry in lib.guide.ideas.iter().filter(|entry| entry.chosen) {
                    // Named after what the search aims at, as far as a name may go.
                    let mut search = SavedSearch { kind: entry.idea.source.clone(), name: entry.idea.why.chars().take(40).collect(), query: entry.idea.query.clone(), max_results: wisp_core::sources::MAX_RESULTS, enabled: true, ..Default::default() };
                    match lib.store.save_search(&mut search) {
                        Ok(()) => saved.push(search.id),
                        Err(err) => failure = Some(format!("{err:#}")),
                    }
                }
                if saved.is_empty() && failure.is_none() { return }
                // What was saved is on the searches page from here on.
                lib.guide.ideas.retain(|entry| !entry.chosen);
                lib.page = Page::Sources;
                self.change(cx, |_| Ok(()));
                if let Some(err) = failure { self.fail(err, cx) }
                for id in saved {
                    self.run(Cmd::RunSearch(id), window, cx);
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
        let library_chat = Chat::new("向文献库提问，输入 / 选择命令，可以粘贴图片", cx);
        let paper_chat = Chat::new("就这篇文献提问，输入 / 选择命令", cx);
        let mut subscriptions = subscriptions;
        for (chat, paper) in [(&library_chat, false), (&paper_chat, true)] {
            subscriptions.push(cx.subscribe(&chat.input, move |this, _, event: &InputEvent, cx| this.chat_input(paper, *event, cx)));
        }
        let guide = Guide::new(&info.topic, cx);
        let screen = Screen::new(&info.criteria, cx);
        let section = "introduction".to_string();
        let draft = store.drafts().ok().and_then(|mut drafts| drafts.remove(&section)).unwrap_or_default();
        let editor = cx.new(|cx| TextInput::new("在这里写。右边会列出文献库里的文献在同一部分是怎么写的；中文也可以，先写下想表达的意思。", cx).multiline(14).with_text(&draft));
        let find = cx.new(|cx| TextInput::new("找句子：输入词或短语，例如 remains unclear", cx));
        subscriptions.push(cx.subscribe(&editor, |this, input, event: &InputEvent, cx| {
            if *event != InputEvent::Changed { return }
            let text = input.read(cx).text().to_string();
            let saved = this.lib.as_mut().map(|lib| lib.store.save_draft(&lib.writing.section.clone(), &text));
            if let Some(Err(err)) = saved { this.fail(format!("草稿没有保存：{err:#}"), cx) }
        }));
        // The caret moving to another paragraph changes what is looked for, as typing does.
        subscriptions.push(cx.observe(&editor, |this, _, cx| this.writing_changed(cx)));
        subscriptions.push(cx.observe(&find, |this, _, cx| this.writing_changed(cx)));
        let writing = Writing { section, editor, find, asked: None, words: String::new(), candidates: None, searching: false, round: 0, keywords: HashMap::new(), structures: BTreeMap::new(), sorting: Sorting::default(), scroll: ScrollHandle::new() };
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
            guide,
            screen,
            suggestions: HashMap::new(),
            translations: HashMap::new(),
            show_translation: false,
            searches: vec![],
            batches: vec![],
            trial: None,
            running: HashSet::new(),
            importing: 0,
            writing,
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
        let Some(lib) = self.lib.as_ref() else { return };
        if lib.selected == id { return }
        // The conversation with the paper being left is kept as far as it got.
        self.stop_chat(true, cx);
        let Some(lib) = self.lib.as_mut() else { return };
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
        self.translate_selected(false, cx);
        cx.notify();
    }

    /// Translate the selected paper's title and abstract while translation
    /// is on. What the library already holds is shown without asking; the
    /// rest goes to the model for translation, each text on its own, and is
    /// kept. `retry` asks again for what failed before.
    fn translate_selected(&mut self, retry: bool, cx: &mut Context<Self>) {
        let profile = self.settings.profile_for_or_default(Task::Translate);
        let prompt = self.settings.translation_prompt.clone();
        let Some(lib) = self.lib.as_mut().filter(|l| l.show_translation) else { return };
        let Some(paper) = lib.selected_paper() else { return };
        let library = lib.info.id.clone();
        for text in [paper.title.clone(), paper.abstract_text.clone()] {
            if text.trim().is_empty() { continue }
            let key = translation_key(&text, ai::translation_prompt(&prompt));
            match lib.translations.get(&key) {
                Some(Translated::Done(_) | Translated::Busy) => continue,
                Some(Translated::Failed(_)) if !retry => continue,
                _ => {}
            }
            if let Ok(Some(kept)) = lib.store.translation(&key) {
                lib.translations.insert(key, Translated::Done(kept));
                continue;
            }
            let request = match ai::translate_request(&profile, &prompt, &text) {
                Ok(request) => request,
                Err(err) => {
                    lib.translations.insert(key, Translated::Failed(err));
                    continue;
                }
            };
            lib.translations.insert(key.clone(), Translated::Busy);
            let (library, protocol, author) = (library.clone(), profile.protocol, format!("{} · {}", profile.title(), profile.model));
            let work = cx.background_executor().spawn(async move { wisp_net::complete(request, protocol) });
            cx.spawn(async move |this, cx| {
                let translated = work.await;
                this.update(cx, |this, cx| {
                    let Some(lib) = this.lib.as_mut().filter(|l| l.info.id == library) else { return };
                    let state = match translated {
                        Ok(text) => {
                            // Not kept is not lost: it is on show, and asked for again another day.
                            let _ = lib.store.save_translation(&key, &text, &author);
                            Translated::Done(text.trim().to_string())
                        }
                        Err(err) => Translated::Failed(err),
                    };
                    lib.translations.insert(key, state);
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
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
                        let found = doi.and_then(|doi| wisp_net::lookup_paper(&Identifier::Doi(doi)).ok());
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
            if lib.store.needs_structure(&file.id)? { lib.store.save_fulltext_pages(&file.id, &pages)? }
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

    /// Read the text of the stored full texts that have not been read and
    /// sorted into parts yet, one at a time: new files, and those an
    /// earlier build read without telling headings from paragraphs.
    pub fn extract_missing(&mut self, cx: &mut Context<Self>) {
        let Some(lib) = self.lib.as_mut() else { return };
        let _ = lib.store.restructure_outdated();
        let mut todo = vec![];
        for file in &lib.files {
            if file.kind != "fulltext" || lib.extracting.contains(&file.id) { continue }
            if matches!(lib.store.needs_structure(&file.id), Ok(true)) {
                todo.push((file.id.clone(), lib.store.file_path(file), lib.store.fulltext_pages(&file.id).ok().flatten()));
            }
        }
        if todo.is_empty() { return }
        lib.extracting.extend(todo.iter().map(|(id, ..)| id.clone()));
        let library = lib.info.id.clone();
        cx.spawn(async move |this, cx| {
            for (id, path, known) in todo {
                let pages = cx.background_executor().spawn(async move { fulltext::reread(&path, known) }).await;
                let same = this.update(cx, |this, cx| {
                    let Some(lib) = this.lib.as_mut().filter(|l| l.info.id == library) else { return false };
                    lib.extracting.remove(&id);
                    let _ = lib.store.save_fulltext_pages(&id, &pages);
                    lib.writing.structures = lib.store.structures().unwrap_or_default();
                    // The candidates on show were found without this paper.
                    if lib.extracting.is_empty() {
                        lib.writing.asked = None;
                        this.writing_changed(cx);
                    }
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

    /// A key or a paste in a conversation's box.
    fn chat_input(&mut self, about_paper: bool, event: InputEvent, cx: &mut Context<Self>) {
        let Some(lib) = self.lib.as_mut() else { return };
        let chat = lib.chat_mut(about_paper);
        let (offered, picked) = (chat.slash_list(cx).len(), chat.slash);
        match event {
            // Enter on a command in the list takes it; otherwise the message goes.
            InputEvent::Submit if offered > 0 => return self.pick_slash(about_paper, picked, cx),
            InputEvent::Submit => return self.ask(about_paper, cx),
            InputEvent::Tab if offered > 0 => return self.pick_slash(about_paper, picked, cx),
            InputEvent::Changed => {
                chat.slash_closed = false;
                chat.slash = 0;
            }
            InputEvent::Up if offered > 0 => chat.slash = (chat.slash + offered - 1) % offered,
            InputEvent::Down if offered > 0 => chat.slash = (chat.slash + 1) % offered,
            InputEvent::Cancel => chat.slash_closed = true,
            InputEvent::Images => {
                let pictures = chat.input.update(cx, |input, _| input.take_images()).into_iter().map(|image| Picture::Bytes(image.bytes)).collect();
                return self.attach_images(about_paper, pictures, cx);
            }
            _ => {}
        }
        // While the list is on show the arrow keys and Tab are its keys.
        let choosing = !chat.slash_list(cx).is_empty();
        chat.input.update(cx, |input, _| input.choosing = choosing);
        cx.notify();
    }

    /// Take one of the slash commands on offer: a command that stands
    /// alone runs, the others are written out for their subject to follow.
    fn pick_slash(&mut self, about_paper: bool, at: usize, cx: &mut Context<Self>) {
        let Some(lib) = self.lib.as_mut() else { return };
        let chat = lib.chat_mut(about_paper);
        let Some(command) = chat.slash_list(cx).get(at).copied() else { return };
        let alone = command.hint.is_empty();
        chat.slash = 0;
        chat.input.update(cx, |input, cx| {
            input.choosing = false;
            input.set_text(&if alone { format!("/{}", command.name) } else { format!("/{} ", command.name) }, cx);
        });
        if alone { self.ask(about_paper, cx) }
        cx.notify();
    }

    /// Read pictures in and put them with the next question. Each is made
    /// one a model takes (`picture::prepare`) off the main thread.
    fn attach_images(&mut self, about_paper: bool, mut pictures: Vec<Picture>, cx: &mut Context<Self>) {
        let Some(lib) = self.lib.as_mut() else { return };
        let (library, selected) = (lib.info.id.clone(), lib.selected.clone());
        let chat = lib.chat_mut(about_paper);
        let room = MAX_ATTACHMENTS.saturating_sub(chat.attachments.len() + chat.attaching);
        let too_many = pictures.len() > room;
        pictures.truncate(room);
        chat.attaching += pictures.len();
        if too_many { self.fail(format!("一次提问最多带 {MAX_ATTACHMENTS} 张图片。"), cx) }
        if pictures.is_empty() { return }
        let work = cx.background_executor().spawn(async move {
            let read = |picture: Picture| match picture {
                Picture::Bytes(bytes) => Ok(bytes),
                Picture::File(path) => std::fs::read(&path).map_err(|err| format!("无法读取 {}：{err}", path.display())),
            };
            pictures.into_iter().map(|picture| read(picture).and_then(crate::picture::prepare)).collect::<Vec<_>>()
        });
        cx.spawn(async move |this, cx| {
            let prepared = work.await;
            this.update(cx, |this, cx| {
                // Another library, or another paper's conversation by now: the pictures were not meant for it.
                let Some(lib) = this.lib.as_mut().filter(|l| l.info.id == library && (!about_paper || l.selected == selected)) else { return };
                let chat = lib.chat_mut(about_paper);
                chat.attaching = chat.attaching.saturating_sub(prepared.len());
                let mut failed = None;
                for picture in prepared {
                    match picture {
                        Ok(image) => chat.attachments.push(ChatImage { id: new_id("I"), image: Arc::new(image) }),
                        Err(err) => failed = Some(err),
                    }
                }
                if let Some(err) = failed { this.fail(err, cx) }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn ask(&mut self, about_paper: bool, cx: &mut Context<Self>) {
        let profile = self.settings.profile_for_or_default(Task::Chat);
        let Some(lib) = self.lib.as_mut() else { return };
        let paper_id = lib.selected.clone();
        let chat = if about_paper { &mut lib.paper_chat } else { &mut lib.library_chat };
        let question = chat.input.read(cx).text().trim().to_string();
        if (question.is_empty() && chat.attachments.is_empty()) || chat.busy { return }
        chat.error.clear();
        chat.note.clear();
        if chat.attaching > 0 {
            chat.error = "图片还在读取，稍等一下再发送。".into();
            return cx.notify();
        }
        // `/search …` and the like stand for a longer request; the bubble shows what was typed.
        let prompt = match ai::read_input(&question) {
            Ok(ai::Input::Ask(prompt)) => prompt,
            Ok(ai::Input::NewChat) => {
                chat.reset(cx);
                return cx.notify();
            }
            Err(err) => {
                chat.error = err;
                return cx.notify();
            }
        };
        chat.history = None;

        // A changed context or protocol starts the wire again from the text of what was said:
        // the model's own turns are only valid in the conversation that produced them.
        let replay = chat.ctx.is_none() || chat.protocol != Some(profile.protocol);
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
            // A conversation taken up from the history keeps the labels its answers already use.
            let mut ctx = ai::build_context(&lib.info.name, &lib.info.topic, &papers, &lib.fields, &fulltext, &scope, &prompt, &chat.known);
            // The tools reach every paper of the library, so every paper can be cited.
            ctx.add_sources(&lib.papers);
            chat.ctx = Some(ctx);
        }
        if replay {
            // Questions with their pictures and what a slash command stood for; the pieces of one answer as one turn.
            let mut wire: Vec<Value> = vec![];
            let mut answer = String::new();
            for item in &chat.messages {
                match item {
                    ChatItem::User { text, images } => {
                        if !answer.is_empty() { wire.push(ai::assistant_message(&std::mem::take(&mut answer))) }
                        wire.push(ai::user_message_with(profile.protocol, &ai::expand_slash(text), &wire_images(images)));
                    }
                    ChatItem::Answer(text) if !text.is_empty() => {
                        if !answer.is_empty() { answer.push_str("\n\n") }
                        answer.push_str(text);
                    }
                    _ => {}
                }
            }
            if !answer.is_empty() { wire.push(ai::assistant_message(&answer)) }
            chat.wire = wire;
            chat.protocol = Some(profile.protocol);
        }
        let images = std::mem::take(&mut chat.attachments);
        chat.wire.push(ai::user_message_with(profile.protocol, &prompt, &wire_images(&images)));
        chat.messages.push(ChatItem::User { text: question, images });
        chat.steps = 0;
        chat.started = Some(Instant::now());
        let sent = self.chat_step(about_paper, cx);
        if let Some(lib) = self.lib.as_mut() {
            let chat = if about_paper { &mut lib.paper_chat } else { &mut lib.library_chat };
            if sent {
                chat.input.update(cx, |input, cx| input.set_text("", cx));
            } else {
                // Nothing was sent: the question stays in the box, its pictures beside it, to be asked again.
                chat.wire.pop();
                if let Some(ChatItem::User { images, .. }) = chat.messages.pop() { chat.attachments = images }
                if chat.messages.is_empty() { chat.ctx = None }
            }
        }
        // Kept from the question on, so that closing the window mid-answer does not lose it.
        if sent { self.save_chat(about_paper, cx) }
        cx.notify();
    }

    /// Send the conversation to the model and follow its turn: the text as
    /// it arrives, then either the end of the answer or the tools it asked
    /// for. False when the request could not be made.
    fn chat_step(&mut self, about_paper: bool, cx: &mut Context<Self>) -> bool {
        let profile = self.settings.profile_for_or_default(Task::Chat);
        let Some(lib) = self.lib.as_mut() else { return false };
        let chat = if about_paper { &mut lib.paper_chat } else { &mut lib.library_chat };
        let Some(ctx) = chat.ctx.as_ref() else { return false };
        let request = match ai::chat_request(&profile, ctx, &chat.wire, &tools::library_catalog()) {
            Ok(request) => request,
            Err(err) => {
                chat.error = err;
                chat.end_run();
                return false;
            }
        };
        chat.messages.push(ChatItem::Answer(String::new()));
        chat.busy = true;
        chat.steps += 1;
        chat.scroll.scroll_to_bottom();
        let generation = chat.generation;
        let library = lib.info.id.clone();

        let (sender, events) = async_channel::unbounded();
        let protocol = profile.protocol;
        cx.background_executor().spawn(async move { wisp_net::stream_chat(request, protocol, sender) }).detach();
        cx.spawn(async move |this, cx| {
            while let Ok(event) = events.recv().await {
                let alive = this.update(cx, |this, cx| {
                    let Some(lib) = this.lib.as_mut().filter(|l| l.info.id == library) else { return false };
                    let chat = if about_paper { &mut lib.paper_chat } else { &mut lib.library_chat };
                    if chat.generation != generation { return false }
                    // Keep following the answer while the reader is at the bottom.
                    let at_bottom = chat.scroll.offset().y <= -chat.scroll.max_offset().height + px(40.);
                    let mut calls = None;
                    let more = match event {
                        ChatEvent::Text(text) => {
                            chat.note.clear();
                            if let Some(ChatItem::Answer(answer)) = chat.messages.last_mut() { answer.push_str(&text) }
                            true
                        }
                        ChatEvent::Retrying(attempt, why) => {
                            chat.note = format!("模型服务暂时没有回应（{why}），正在重试，第 {attempt} 次…");
                            true
                        }
                        ChatEvent::Error(error) => {
                            chat.note.clear();
                            chat.error = error;
                            chat.end_run();
                            false
                        }
                        ChatEvent::Done(turn) => {
                            chat.note.clear();
                            chat.wire.extend(turn.wire());
                            if turn.calls.is_empty() {
                                chat.note = ai::stop_note(&turn.stop).unwrap_or_default().to_string();
                                chat.end_run();
                            } else if !turn.runnable() || chat.steps >= MAX_CHAT_STEPS {
                                // The calls still get an answer: a conversation that ends on an unanswered call cannot continue.
                                let (why, note) = if turn.runnable() {
                                    ("已达到单次提问的步数上限，工具没有执行。", "已达到单次提问的步数上限。可以继续提问，让它接着做。")
                                } else {
                                    ("回答被截断，工具没有执行。", ai::stop_note(&turn.stop).unwrap_or("回答没有正常结束。"))
                                };
                                let results: Vec<(String, String, bool)> = turn.calls.iter().map(|call| (call.id.clone(), why.to_string(), true)).collect();
                                chat.wire.extend(ai::tool_results(protocol, &results));
                                chat.note = note.to_string();
                                chat.end_run();
                            } else {
                                chat.drop_empty_answer();
                                calls = Some(turn.calls);
                            }
                            false
                        }
                    };
                    if at_bottom { chat.scroll.scroll_to_bottom() }
                    let ended = !chat.busy;
                    if let Some(calls) = calls { this.run_tools(about_paper, calls, cx) }
                    if ended { this.save_chat(about_paper, cx) }
                    cx.notify();
                    more
                });
                if !matches!(alive, Ok(true)) { break }
            }
        })
        .detach();
        true
    }

    /// Make the tools a turn asked for ready, show them, and run them:
    /// at once, or after the person's consent when one of them would bring
    /// papers in, take them out, or drop a saved search.
    fn run_tools(&mut self, about_paper: bool, calls: Vec<ToolCall>, cx: &mut Context<Self>) {
        let Some(lib) = self.lib.as_mut() else { return };
        let chat = lib.chat_mut(about_paper);
        let first = chat.messages.len();
        let catalog = tools::catalog();
        let mut prepared = vec![];
        for call in calls {
            let tool = catalog.iter().find(|t| t.name == call.name);
            let line = tools::describe_call(&call.name, call.args.as_ref().unwrap_or(&Value::Null));
            chat.messages.push(ChatItem::Tool { call: line.clone(), result: None });
            // The model may name a paper by its label.
            let args = call.args.map(|mut args| {
                ai::resolve_labels(&mut args, chat.sources());
                args
            });
            // A call that cannot run as written has nothing to ask about: it fails by itself.
            let confirm = args.is_ok() && tool.is_some_and(|t| t.confirm);
            prepared.push(PreparedCall { id: call.id, name: call.name, args, writes: tool.is_some_and(|t| t.writes), confirm, line });
        }
        chat.scroll.scroll_to_bottom();
        if prepared.iter().any(|call| call.confirm) && !chat.allow_changes {
            chat.pending = Some(PendingTools { calls: prepared, first });
            return cx.notify();
        }
        self.execute_tools(about_paper, prepared, first, true, cx);
    }

    /// Run a turn's tools off the main thread, then hand the results to
    /// the model with the next step. Calls that only read run side by
    /// side; a turn that changes the library runs in the order it was
    /// given. Without the person's consent (`allowed`) the calls that asked
    /// for it are answered with a refusal and the others still run.
    fn execute_tools(&mut self, about_paper: bool, calls: Vec<PreparedCall>, first: usize, allowed: bool, cx: &mut Context<Self>) {
        let Some(lib) = self.lib.as_mut() else { return };
        let info = lib.info.clone();
        let chat = lib.chat_mut(about_paper);
        let Some(protocol) = chat.protocol else { return };
        let sources: Arc<Vec<ai::Source>> = Arc::new(chat.sources().to_vec());
        let generation = chat.generation;
        chat.running = calls.iter().map(|call| call.id.clone()).collect();
        let wrote = calls.iter().any(|call| call.writes && (allowed || !call.confirm));
        let library = info.id.clone();
        let executor = cx.background_executor().clone();
        let tasks: Vec<gpui::Task<Vec<(String, String, bool, String)>>> = if wrote || calls.len() == 1 {
            vec![executor.spawn(async move { calls.into_iter().map(|call| run_tool(&info, &sources, call, allowed)).collect() })]
        } else {
            calls
                .into_iter()
                .map(|call| {
                    let (info, sources) = (info.clone(), sources.clone());
                    executor.spawn(async move { vec![run_tool(&info, &sources, call, allowed)] })
                })
                .collect()
        };
        cx.spawn(async move |this, cx| {
            let mut done = vec![];
            for task in tasks {
                done.extend(task.await);
            }
            this.update(cx, |this, cx| {
                let Some(lib) = this.lib.as_mut().filter(|l| l.info.id == library) else { return };
                // A change was made whether or not its answer is still wanted: the window reads the library again.
                let reloaded = if wrote { lib.reload() } else { Ok(()) };
                let chat = lib.chat_mut(about_paper);
                if chat.generation == generation {
                    for (n, (_, _, failed, shown)) in done.iter().enumerate() {
                        if let Some(ChatItem::Tool { result, .. }) = chat.messages.get_mut(first + n) { *result = Some((shown.clone(), *failed)) }
                    }
                    let results: Vec<(String, String, bool)> = done.into_iter().map(|(id, output, failed, _)| (id, output, failed)).collect();
                    let mut results = ai::tool_results(protocol, &results);
                    // One step is left: the model is told to answer with what it has.
                    if chat.steps + 1 >= MAX_CHAT_STEPS { results = ai::with_note(protocol, results, ai::LAST_STEP_NOTE) }
                    chat.wire.extend(results);
                    chat.running.clear();
                    if !this.chat_step(about_paper, cx) { this.save_chat(about_paper, cx) }
                }
                if let Err(err) = reloaded { this.fail(format!("{err:#}"), cx) }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Stop the answer being given. What still arrives for it is dropped;
    /// the conversation stays one that can go on: the calls in flight are
    /// answered as stopped, and the text so far stays as the model's turn.
    fn stop_chat(&mut self, about_paper: bool, cx: &mut Context<Self>) {
        let Some(lib) = self.lib.as_mut() else { return };
        let chat = lib.chat_mut(about_paper);
        if !chat.busy { return }
        chat.generation += 1;
        let unanswered: Vec<String> = match chat.pending.take() {
            Some(pending) => pending.calls.into_iter().map(|call| call.id).collect(),
            None => std::mem::take(&mut chat.running),
        };
        if let Some(protocol) = chat.protocol {
            if !unanswered.is_empty() {
                let results: Vec<(String, String, bool)> = unanswered.into_iter().map(|id| (id, "用户中止了这次提问，这个工具的结果没有交回。".to_string(), true)).collect();
                chat.wire.extend(ai::tool_results(protocol, &results));
            } else if let Some(ChatItem::Answer(text)) = chat.messages.last() && !text.is_empty() {
                chat.wire.push(ai::assistant_message(text));
            }
        }
        for item in chat.messages.iter_mut() {
            if let ChatItem::Tool { result, .. } = item && result.is_none() { *result = Some(("已中止".into(), true)) }
        }
        chat.note = "已停止。".into();
        chat.end_run();
        self.save_chat(about_paper, cx);
        cx.notify();
    }

    /// Keep the conversation in the library's history, as it stands.
    fn save_chat(&mut self, about_paper: bool, cx: &mut Context<Self>) {
        let Some(lib) = self.lib.as_mut() else { return };
        let paper_id = match (about_paper, &lib.selected) {
            (true, Some(id)) => id.clone(),
            (true, None) => return,
            _ => String::new(),
        };
        let chat = if about_paper { &mut lib.paper_chat } else { &mut lib.library_chat };
        if !chat.messages.iter().any(|item| matches!(item, ChatItem::User { .. })) { return }
        let mut conversation = Conversation { id: chat.id.clone(), paper_id, transcript: chat.transcript(), created_at: chat.created_at.clone(), ..Default::default() };
        let mut kept = vec![];
        let result = lib.store.save_conversation(&mut conversation).and_then(|()| {
            for item in &chat.messages {
                let ChatItem::User { images, .. } = item else { continue };
                for picture in images.iter().filter(|picture| !chat.stored_images.contains(&picture.id)) {
                    lib.store.save_conversation_image(&conversation.id, &picture.id, picture.image.format.mime_type(), &picture.image.bytes)?;
                    kept.push(picture.id.clone());
                }
            }
            Ok(())
        });
        chat.stored_images.extend(kept);
        (chat.id, chat.created_at) = (conversation.id, conversation.created_at);
        if let Err(err) = result { self.fail(format!("这次对话没能保存：{err:#}"), cx) }
    }

    /// Take up a conversation from the history: what was said is shown
    /// again, and the next question continues it. The model is given the
    /// library as it is now, under the labels the old answers use.
    fn open_chat(&mut self, about_paper: bool, id: &str, cx: &mut Context<Self>) {
        let Some(lib) = self.lib.as_mut() else { return };
        let conversation = match lib.store.conversation(id) {
            Ok(Some(conversation)) => conversation,
            Ok(None) => return self.fail("这次对话已经不在了。", cx),
            Err(err) => return self.fail(format!("{err:#}"), cx),
        };
        let chat = if about_paper { &mut lib.paper_chat } else { &mut lib.library_chat };
        chat.reset(cx);
        for item in conversation.transcript.items {
            chat.messages.push(match item {
                ai::Said::User { text, images } => {
                    let mut pictures = vec![];
                    for image_id in images {
                        // A picture that cannot be read back is left out; the question stays.
                        let Ok(Some((media_type, data))) = lib.store.conversation_image(&conversation.id, &image_id) else { continue };
                        let Some(format) = crate::picture::format_of(&media_type) else { continue };
                        chat.stored_images.insert(image_id.clone());
                        pictures.push(ChatImage { id: image_id, image: Arc::new(Image::from_bytes(format, data)) });
                    }
                    ChatItem::User { text, images: pictures }
                }
                ai::Said::Answer { text } => ChatItem::Answer(text),
                ai::Said::Tool { call, result, failed } => ChatItem::Tool { call, result: Some((result, failed)) },
            });
        }
        chat.known = conversation.transcript.sources;
        (chat.id, chat.created_at) = (conversation.id, conversation.created_at);
        chat.scroll = ScrollHandle::new();
        chat.scroll.scroll_to_bottom();
        cx.notify();
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

    fn search_dialog(&mut self, editing: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        self.modal_subscriptions.clear();
        self.modal_error.clear();
        let search = editing.as_ref().and_then(|id| self.lib.as_ref()?.searches.iter().find(|s| &s.id == id).cloned()).unwrap_or(SavedSearch { kind: "pubmed".into(), max_results: 50, enabled: true, ..Default::default() });
        let name = self.modal_input("名称，例如：根尖单细胞（可以留空）", &search.name, false, window, cx);
        let query = self.modal_input("检索式", &search.query, true, window, cx);
        let max = self.modal_input("50", &search.max_results.to_string(), false, window, cx);
        self.modal = Some(Modal::Search { editing, kind: search.kind, name, query, max });
    }

    /// Ask the model for searches on the guide's topic, then try each one so
    /// the person can judge them by what they find.
    fn guide_generate(&mut self, cx: &mut Context<Self>) {
        let profile = self.settings.profile_for_or_default(Task::Chat);
        let Some(lib) = self.lib.as_mut() else { return };
        if lib.guide.busy { return }
        let keywords = lib.guide.keywords.read(cx).text().trim().to_string();
        lib.guide.error.clear();
        if keywords.is_empty() && lib.info.topic.trim().is_empty() {
            lib.guide.error = "先写下研究主题或几个关键词。".into();
            return cx.notify();
        }
        let (system, prompt) = ai::query_prompt(&lib.info.name, &lib.info.topic, &keywords, &lib.guide.kinds);
        let request = match ai::plain_request(&profile, &system, &prompt) {
            Ok(request) => request,
            Err(err) => {
                lib.guide.error = err;
                return cx.notify();
            }
        };
        lib.guide.busy = true;
        lib.guide.round += 1;
        let (round, library, protocol) = (lib.guide.round, lib.info.id.clone(), profile.protocol);
        cx.notify();
        let work = cx.background_executor().spawn(async move { wisp_net::complete(request, protocol).and_then(|reply| ai::parse_queries(&reply)) });
        cx.spawn(async move |this, cx| {
            let ideas = work.await;
            this.update(cx, |this, cx| {
                let Some(lib) = this.lib.as_mut().filter(|l| l.info.id == library && l.guide.round == round) else { return };
                lib.guide.busy = false;
                match ideas {
                    Err(err) => lib.guide.error = err,
                    Ok(ideas) => {
                        lib.guide.ideas = ideas.into_iter().map(|idea| GuideIdea { idea, chosen: false, trial: None }).collect();
                        let trials: Vec<(usize, Value)> = lib.guide.ideas.iter().enumerate().map(|(i, entry)| (i, serde_json::json!({"source": entry.idea.source, "query": entry.idea.query, "limit": 8}))).collect();
                        for (index, args) in trials {
                            this.library_tool("search_online", args, cx, move |this, result, _| {
                                // Only for the ideas this trial was started for.
                                if let Some(lib) = this.lib.as_mut().filter(|l| l.guide.round == round) && let Some(entry) = lib.guide.ideas.get_mut(index) { entry.trial = Some(result) }
                            });
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// While the app runs, look once a minute for saved searches whose turn
    /// has come, in every library that updates on its own, and run them.
    /// What was missed while the app was closed is due at the first look.
    fn watch_searches(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(std::time::Duration::from_secs(20)).await;
            loop {
                let Ok(root) = this.update(cx, |this, _| this.root.clone()) else { break };
                // One look at a time: the next waits for this one's runs to end.
                let outcome = cx.background_executor().spawn(async move { tools::call(&root, "sync_searches", &serde_json::json!({}), &wisp_net::get_text) }).await;
                let alive = this.update(cx, |this, cx| {
                    let Ok(outcome) = outcome else { return };
                    let runs = outcome["runs"].as_array().cloned().unwrap_or_default();
                    if runs.is_empty() { return }
                    this.libraries = libraries::list(&this.root);
                    // The open library shows what came in; the others show it when opened.
                    let Some(open) = this.lib.as_ref().map(|l| l.info.id.clone()) else { return cx.notify() };
                    let mine: Vec<&Value> = runs.iter().filter(|run| run["libraryId"] == open.as_str()).collect();
                    if mine.is_empty() { return }
                    this.change(cx, |_| Ok(()));
                    let added: u64 = mine.iter().filter_map(|run| run["added"].as_u64()).sum();
                    if added > 0 { this.say(format!("自动更新：新增 {added} 篇待判断的文献"), cx) }
                });
                if alive.is_err() { break }
                cx.background_executor().timer(std::time::Duration::from_secs(60)).await;
            }
        })
        .detach();
    }

    /// Start asking the assistant about the pending papers it has not judged
    /// yet, several at a time (设置 › AI says how many, and with which
    /// model). What it says is saved as a suggestion; the decision stays
    /// with the person.
    fn screen_start(&mut self, cx: &mut Context<Self>) {
        let profile = self.settings.profile_for_or_default(Task::Screen);
        let workers = self.settings.screen_workers;
        let Some(lib) = self.lib.as_mut() else { return };
        if lib.screen.running() { return }
        lib.screen.error.clear();
        // The key and address are checked once here, not once per paper.
        if let Err(err) = ai::plain_request(&profile, "s", "p") {
            lib.screen.error = err;
            return cx.notify();
        }
        // The criteria are the library's: they are kept with it.
        let criteria = lib.screen.criteria.read(cx).text().trim().to_string();
        if criteria != lib.info.criteria {
            lib.info.criteria = criteria;
            if let Err(err) = libraries::save(&lib.info) {
                lib.screen.error = format!("{err:#}");
                return cx.notify();
            }
        }
        let mut todo: Vec<String> = lib.screen_queue().iter().filter(|p| !lib.suggestions.contains_key(&p.id)).map(|p| p.id.clone()).collect();
        if todo.is_empty() {
            lib.screen.error = "待判断的文献都已经有建议了。".into();
            return cx.notify();
        }
        todo.reverse();
        let screen = &mut lib.screen;
        screen.run += 1;
        (screen.total, screen.done, screen.failed, screen.streak, screen.todo) = (todo.len(), 0, 0, 0, todo);
        screen.reading.clear();
        screen.model = format!("{} · {}", profile.title(), profile.model);
        let run = screen.run;
        for _ in 0..workers {
            self.screen_next(run, cx);
        }
        cx.notify();
    }

    /// Ask about the next paper of the run, and about the one after it once
    /// the answer is in. A paper without a usable answer is counted and
    /// left waiting; several of them in a row end the run.
    fn screen_next(&mut self, run: u64, cx: &mut Context<Self>) {
        let profile = self.settings.profile_for_or_default(Task::Screen);
        let Some(lib) = self.lib.as_mut().filter(|l| l.screen.run == run) else { return };
        let Some(id) = lib.screen.todo.pop() else { return };
        let Some(paper) = lib.paper(&id).cloned() else { return self.screen_next(run, cx) };
        let pages = lib.store.paper_pages(&id).ok().flatten();
        let (system, prompt, basis) = ai::screen_prompt(&lib.info.topic, &lib.info.criteria, &paper, pages.as_deref());
        let author = format!("{} · {}", profile.title(), profile.model);
        // Nothing to read: say so without asking, rather than have the model guess from a title.
        if basis == "none" {
            let mut suggestion = Suggestion { paper_id: id, verdict: "uncertain".into(), reason: "这篇文献没有摘要，也没有保存全文，无法判断。".into(), basis: basis.into(), author, ..Default::default() };
            let saved = lib.store.save_suggestion(&mut suggestion);
            lib.screen.done += 1;
            if saved.is_ok() { lib.suggestions.insert(suggestion.paper_id.clone(), suggestion); }
            return self.screen_next(run, cx);
        }
        let request = match ai::plain_request(&profile, &system, &prompt) {
            Ok(request) => request,
            Err(err) => {
                lib.screen.error = err;
                lib.screen.todo.clear();
                return;
            }
        };
        lib.screen.reading.push(id);
        let (library, protocol) = (lib.info.id.clone(), profile.protocol);
        let work = cx.background_executor().spawn(async move {
            let judged = wisp_net::complete(request, protocol).and_then(|reply| ai::parse_judgement(&reply, &paper, pages.as_deref()));
            (paper, judged)
        });
        cx.spawn(async move |this, cx| {
            let (paper, judged) = work.await;
            this.update(cx, |this, cx| {
                let Some(lib) = this.lib.as_mut().filter(|l| l.info.id == library && l.screen.run == run) else { return };
                lib.screen.reading.retain(|reading| *reading != paper.id);
                lib.screen.done += 1;
                let saved = judged.and_then(|j| {
                    let mut suggestion = Suggestion { paper_id: paper.id.clone(), verdict: j.verdict, reason: j.reason, excerpt: j.excerpt, page: j.page, basis: basis.into(), confidence: j.confidence, author, ..Default::default() };
                    lib.store.save_suggestion(&mut suggestion).map(|()| suggestion).map_err(|e| format!("{e:#}"))
                });
                match saved {
                    Ok(suggestion) => {
                        lib.screen.streak = 0;
                        lib.suggestions.insert(suggestion.paper_id.clone(), suggestion);
                        this.screen_next(run, cx);
                    }
                    Err(err) => {
                        lib.screen.failed += 1;
                        lib.screen.streak += 1;
                        if lib.screen.streak >= SCREEN_GIVE_UP {
                            lib.screen.error = format!("连续 {SCREEN_GIVE_UP} 篇没有得到可用的回答，已停止。最后一篇「{}」：{err}", paper.title);
                            lib.screen.todo.clear();
                        } else {
                            lib.screen.error = format!("「{}」没有判断成，留待下一轮：{err}", paper.title);
                            this.screen_next(run, cx);
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    // ---- writing

    /// Look for candidates again when the part, the words looked for or
    /// what is being written has changed: a moment after the last change,
    /// so typing does not ask for every letter.
    fn writing_changed(&mut self, cx: &mut Context<Self>) {
        let Some(lib) = self.lib.as_mut().filter(|l| l.page == Page::Write) else { return };
        let writing = &mut lib.writing;
        let find = writing.find.read(cx).text().trim().to_string();
        let query = if find.is_empty() {
            let editor = writing.editor.read(cx);
            structure::writing_at(editor.text(), editor.caret())
        } else {
            find
        };
        let wanted = (writing.section.clone(), query);
        if writing.asked.as_ref() == Some(&wanted) { return }
        writing.asked = Some(wanted);
        writing.round += 1;
        let (round, library) = (writing.round, lib.info.id.clone());
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(std::time::Duration::from_millis(220)).await;
            this.update(cx, |this, cx| {
                if this.lib.as_ref().is_some_and(|l| l.info.id == library && l.writing.round == round) { this.writing_search(cx) }
            })
            .ok();
        })
        .detach();
    }

    /// Find the candidates for what was last asked. Something written in
    /// Chinese is first put into English search words by the model for
    /// translation (once per text): the library's sentences are English.
    fn writing_search(&mut self, cx: &mut Context<Self>) {
        let profile = self.settings.profile_for_or_default(Task::Translate);
        let Some(lib) = self.lib.as_mut() else { return };
        let Some((section, query)) = lib.writing.asked.clone() else { return };
        let round = lib.writing.round;
        let mut words = query.clone();
        if structure::in_chinese(&query) {
            match lib.writing.keywords.get(&query) {
                Some(Some(known)) => words = known.clone(),
                Some(None) => return,
                None => {
                    // Without a model the words go as written, and find what little they can.
                    if let Ok(request) = ai::plain_request(&profile, structure::KEYWORDS_SYSTEM, &query) {
                        lib.writing.keywords.insert(query.clone(), None);
                        lib.writing.searching = true;
                        let (library, protocol) = (lib.info.id.clone(), profile.protocol);
                        let work = cx.background_executor().spawn(async move { wisp_net::complete(request, protocol) });
                        cx.spawn(async move |this, cx| {
                            let reply = work.await;
                            this.update(cx, |this, cx| {
                                let Some(lib) = this.lib.as_mut().filter(|l| l.info.id == library) else { return };
                                let found = reply.map(|words| words.split_whitespace().collect::<Vec<_>>().join(" ")).unwrap_or_else(|_| query.clone());
                                lib.writing.keywords.insert(query, Some(found));
                                if lib.writing.round == round { this.writing_search(cx) }
                            })
                            .ok();
                        })
                        .detach();
                        return cx.notify();
                    }
                }
            }
        }
        lib.writing.searching = true;
        lib.writing.words = if words == query { String::new() } else { words.clone() };
        self.library_tool("find_sentences", serde_json::json!({"section": section, "query": words, "limit": 30}), cx, move |this, result, _| {
            let Some(lib) = this.lib.as_mut().filter(|l| l.writing.round == round) else { return };
            lib.writing.searching = false;
            lib.writing.candidates = Some(result);
            lib.writing.scroll.set_offset(point(px(0.), px(0.)));
        });
    }

    /// Start the model on the full texts: sorting those the rules sorted
    /// into parts, or reading the pages of those that gave no text.
    fn write_run_start(&mut self, reading: bool, cx: &mut Context<Self>) {
        let profile = self.settings.profile_for_or_default(Task::Structure);
        let workers = self.settings.screen_workers;
        let Some(lib) = self.lib.as_mut() else { return };
        if lib.writing.sorting.running() { return }
        lib.writing.sorting.error.clear();
        if let Err(err) = ai::plain_request(&profile, "s", "p") {
            lib.writing.sorting.error = err;
            return cx.notify();
        }
        // Pages to look at: only a PDF has them. Parts to sort: the texts the rules had to guess at.
        let mut todo: Vec<String> = lib.writing.structures.iter().filter(|(id, info)| if reading { info.method == "none" && lib.files.iter().any(|f| f.id == **id && f.format == "pdf") } else { info.method == "rules" && info.open >= structure::OPEN_ENOUGH }).map(|(id, _)| id.clone()).collect();
        if todo.is_empty() { return }
        todo.reverse();
        let sorting = &mut lib.writing.sorting;
        sorting.run += 1;
        (sorting.reading, sorting.total, sorting.done, sorting.failed, sorting.streak, sorting.todo) = (reading, todo.len(), 0, 0, 0, todo);
        sorting.model = format!("{} · {}", profile.title(), profile.model);
        let run = sorting.run;
        // A file's pages are read one after another; two files at a time is as much as a service takes kindly.
        for _ in 0..if reading { workers.min(2) } else { workers } {
            self.write_run_next(run, cx);
        }
        cx.notify();
    }

    /// Work on the next file of the run, and on the one after it once this is done.
    fn write_run_next(&mut self, run: u64, cx: &mut Context<Self>) {
        enum Made {
            Sorted(Vec<structure::Sentence>),
            Read(Vec<String>),
        }
        let profile = self.settings.profile_for_or_default(Task::Structure);
        let Some(lib) = self.lib.as_mut().filter(|l| l.writing.sorting.run == run) else { return };
        let Some(file_id) = lib.writing.sorting.todo.pop() else { return };
        let Some(file) = lib.files.iter().find(|f| f.id == file_id).cloned() else { return self.write_run_next(run, cx) };
        let (title, summary) = lib.paper(&file.paper_id).map(|p| (p.title.clone(), p.abstract_text.clone())).unwrap_or_default();
        let author = format!("{} · {}", profile.title(), profile.model);
        let (library, protocol, reading) = (lib.info.id.clone(), profile.protocol, lib.writing.sorting.reading);
        let path = lib.store.file_path(&file);
        let pages = lib.store.fulltext_pages(&file.id).ok().flatten().and_then(Result::ok).unwrap_or_default();
        lib.writing.sorting.working.push(file_id.clone());
        let name = title.clone();
        let work = cx.background_executor().spawn(async move {
            if reading {
                let mut pages = vec![];
                for picture in crate::pdf::page_pictures(&path, READ_PAGES)? {
                    let request = ai::read_page_request(&profile, ai::ImageData { media_type: "image/png".into(), data: ai::base64(&picture) })?;
                    pages.push(ai::page_reading(&wisp_net::complete(request, protocol)?));
                }
                if pages.iter().all(|page| page.trim().is_empty()) { return Err("模型在页面上没有读到文字。".to_string()) }
                Ok(Made::Read(pages))
            } else {
                let blocks = structure::blocks(&pages);
                let request = ai::plain_request(&profile, structure::OUTLINE_SYSTEM, &structure::outline_prompt(&name, &blocks))?;
                let read = structure::parse_outline(&wisp_net::complete(request, protocol)?, &blocks, &summary)?;
                // What stands under a heading that names its part stays there; the model decides the rest.
                let (labels, sure) = structure::assign_sure(&blocks, &summary);
                Ok(Made::Sorted(structure::sentences(&blocks, &structure::merge(&labels, &sure, &read))))
            }
        });
        cx.spawn(async move |this, cx| {
            let made = work.await;
            this.update(cx, |this, cx| {
                let Some(lib) = this.lib.as_mut().filter(|l| l.info.id == library && l.writing.sorting.run == run) else { return };
                lib.writing.sorting.working.retain(|working| *working != file_id);
                lib.writing.sorting.done += 1;
                let saved = made.and_then(|made| {
                    match made {
                        Made::Sorted(sentences) => lib.store.save_model_structure(&file_id, &author, &sentences),
                        Made::Read(pages) => lib.store.save_read_pages(&file_id, pages, &author),
                    }
                    .map_err(|e| format!("{e:#}"))
                });
                let sorting = &mut lib.writing.sorting;
                match saved {
                    Ok(()) => {
                        sorting.streak = 0;
                        lib.writing.structures = lib.store.structures().unwrap_or_default();
                        lib.writing.asked = None;
                        this.write_run_next(run, cx);
                        this.writing_changed(cx);
                    }
                    Err(err) => {
                        sorting.failed += 1;
                        sorting.streak += 1;
                        if sorting.streak >= SCREEN_GIVE_UP {
                            sorting.error = format!("连续 {SCREEN_GIVE_UP} 篇没有成功，已停止。最后一篇「{title}」：{err}");
                            sorting.todo.clear();
                        } else {
                            sorting.error = format!("「{title}」没有整理成：{err}");
                            this.write_run_next(run, cx);
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Run a library tool off the main thread, on a connection of its own
    /// and with the network, and hand the result back if the same library
    /// is still open.
    fn library_tool(&mut self, name: &'static str, args: Value, cx: &mut Context<Self>, done: impl FnOnce(&mut Self, Result<Value, String>, &mut Context<Self>) + 'static) {
        let Some(lib) = self.lib.as_ref() else { return };
        let info = lib.info.clone();
        let library = info.id.clone();
        cx.notify();
        let work = cx.background_executor().spawn(async move {
            let mut store = Store::open(&info.dir).map_err(|e| format!("{e:#}"))?;
            tools::call_in(&mut store, &info, name, &args, &wisp_net::get_text)
        });
        cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                if this.lib.as_ref().is_none_or(|l| l.info.id != library) { return }
                done(this, result, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
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
                let mut created = None;
                let result = match editing.clone() {
                    None => libraries::create(&self.root, &name, &topic, &color).map(|info| created = Some(info.id)),
                    Some(id) => {
                        let Some(mut info) = self.libraries.iter().find(|l| l.id == id).cloned() else { return };
                        (info.name, info.topic, info.color) = (name, topic, color);
                        libraries::save(&info).map(|_| {
                            if let Some(lib) = self.lib.as_mut().filter(|l| l.info.id == info.id) {
                                theme::set_accent(&info.color);
                                window.set_window_title(&format!("{} — Wisp Paper", info.name));
                                // A changed topic changes what the assistant is told.
                                lib.library_chat.forget_context();
                                lib.paper_chat.forget_context();
                                lib.info = LibraryInfo { paper_count: lib.info.paper_count, opened_at: lib.info.opened_at.clone(), ..info };
                            }
                        })
                    }
                };
                match result {
                    Ok(()) => {
                        self.libraries = libraries::list(&self.root);
                        self.run(Cmd::CloseModal, window, cx);
                        // A new library opens on its guide: where to start filling it.
                        if let Some(id) = created {
                            self.run(Cmd::OpenLibrary(id), window, cx);
                            self.run(Cmd::OpenGuide(true), window, cx);
                        }
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
                    let found = cx.background_executor().spawn(async move { wisp_net::lookup_paper(&id) }).await;
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
                        // An edited title or abstract is a new text to translate.
                        self.translate_selected(false, cx);
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
            Modal::Search { editing, kind, name, query, max } => {
                let Ok(max_results) = text(max, cx).parse::<usize>() else {
                    self.modal_error = "每次取回的篇数应为数字，例如 50。".into();
                    return;
                };
                let Some(lib) = self.lib.as_mut() else { return };
                // An edited search keeps whether it is enabled.
                let enabled = editing.as_ref().and_then(|id| lib.searches.iter().find(|s| &s.id == id)).is_none_or(|s| s.enabled);
                let mut search = SavedSearch { id: editing.clone().unwrap_or_default(), kind: kind.clone(), name: text(name, cx), query: text(query, cx), max_results, enabled, ..Default::default() };
                match lib.store.save_search(&mut search) {
                    Ok(()) => {
                        // What the old query found says nothing about the new one.
                        lib.trial = None;
                        self.change(cx, |_| Ok(()));
                        self.run(Cmd::CloseModal, window, cx);
                    }
                    Err(err) => self.modal_error = format!("{err:#}"),
                }
            }
        }
        cx.notify();
    }
}

/// Run one tool call for the assistant on a connection of its own (the
/// window's stays free while a PDF's text is extracted): the call's id,
/// what the model reads, whether it failed, and the line for the window.
fn run_tool(info: &LibraryInfo, sources: &[ai::Source], call: PreparedCall, allowed: bool) -> (String, String, bool, String) {
    if call.confirm && !allowed {
        return (call.id, "用户没有同意这次操作，它没有执行。不要重试；需要时向用户说明它会做什么，由用户决定。".into(), true, "没有执行：未获同意".into());
    }
    let result = call.args.and_then(|args| {
        let mut store = Store::open(&info.dir).map_err(|e| format!("{e:#}"))?;
        let mut value = tools::call_in(&mut store, info, &call.name, &args, &wisp_net::get_text)?;
        ai::add_labels(&mut value, sources);
        Ok(value)
    });
    let shown = tools::describe_result(&result);
    match result {
        Ok(value) => (call.id, tools::output_text(&value), false, shown),
        Err(err) => (call.id, err, true, shown),
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
                "page" => Some(Cmd::SetPage(match arg { "ask" => Page::Ask, "trash" => Page::Trash, "sources" => Page::Sources, "screen" => Page::Screen, "write" => Page::Write, _ => Page::Papers })),
                // `write:methods` shows another part of the manuscript; `draft:<text>` leaves text in it, as if typed.
                "write" => Some(Cmd::WriteSection(arg.to_string())),
                "draft" => {
                    if let Some(lib) = self.lib.as_ref() { lib.writing.editor.update(cx, |input, cx| input.insert(arg, cx)) }
                    return;
                }
                "screen" => Some(Cmd::ScreenStart),
                // `sort` has the model sort the full texts into parts, `read` has it read the pages of those without text.
                "sort" => Some(Cmd::WriteRun(false)),
                "read" => Some(Cmd::WriteRun(true)),
                // `guide:new` is what a new library shows; `guide:search` goes straight to composing, `guide:go` also generates.
                "guide" => {
                    self.run(Cmd::OpenGuide(arg == "new"), window, cx);
                    (arg == "go").then_some(Cmd::GuideGenerate)
                }
                // `quote:3|a sentence` opens the selected paper's PDF at page 3 with that sentence highlighted.
                "quote" => self.lib.as_ref().and_then(|l| l.selected.clone()).zip(arg.split_once('|')).map(|(id, (page, quote))| Cmd::GotoSource(id, page.parse().ok(), quote.to_string())),
                // `try:0` runs the first saved search as a trial.
                "try" => self.lib.as_ref().and_then(|l| l.searches.get(arg.parse::<usize>().unwrap_or(0)).map(|s| Cmd::TrySearch(s.id.clone()))),
                "wide" => Some(Cmd::ToggleWide),
                "translate" => Some(Cmd::ToggleTranslation),
                // `profile:1` shows the second model service on the AI page.
                "profile" => Some(Cmd::SelectProfile(arg.parse().unwrap_or(0))),
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
                // Ask the library a question (no commas in it: they separate the steps).
                "ask" => {
                    if let Some(lib) = self.lib.as_ref() { lib.library_chat.input.update(cx, |input, cx| input.set_text(arg, cx)) }
                    Some(Cmd::Ask(false))
                }
                // `type:/se` leaves text in the library's question box, as if typed (the slash commands show).
                "type" => {
                    if let Some(lib) = self.lib.as_ref() {
                        lib.library_chat.input.update(cx, |input, cx| input.set_text(arg, cx));
                        lib.library_chat.input.read(cx).focus(window);
                    }
                    self.chat_input(false, InputEvent::Changed, cx);
                    None
                }
                // `image:/path/to.png` puts a picture with the next question to the library.
                "image" => {
                    self.attach_images(false, vec![Picture::File(arg.into())], cx);
                    None
                }
                "history" => Some(Cmd::ToggleHistory(false)),
                "settings" => Some(Cmd::OpenSettings(match arg {
                    "appearance" => SettingsPage::Appearance,
                    "shortcuts" => SettingsPage::Shortcuts,
                    "ai" => SettingsPage::Ai,
                    "network" => SettingsPage::Network,
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
                    "search" => Some(Cmd::NewSearch),
                    "paper" => self.lib.as_ref().and_then(|l| l.selected.clone()).map(Cmd::EditPaper),
                    _ => None,
                },
                _ => None,
            };
            if let Some(cmd) = cmd { self.run(cmd, window, cx) }
        }
    }
}
