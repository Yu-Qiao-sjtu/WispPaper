# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

Wisp Paper is a local-first literature library desktop app (collect papers, organise, preview PDFs, ask an LLM about the library), written in Rust with GPUI. Targets Windows and macOS; Linux compiles but is untested. The UI is Chinese-only: user-facing strings and error messages are written in Chinese, code comments in English.

## Commands

```sh
cargo run --release                     # run the app
cargo test                              # all four crates (workspace default-members)
cargo run -p wisp_cli -- help           # the `wisp` command line; `-- mcp` is the MCP server
cargo test -p wisp_core                 # engine only; no GPUI build, much faster
cargo test -p wisp_core store::         # one module's tests
cargo test -p wisp_core <test_name>     # one test
cargo test -- --ignored lookup          # live Crossref / Europe PMC lookups (network)
cargo clippy --all-targets              # expected to be warning-free
```

- The toolchain is pinned in `rust-toolchain.toml` (clippy only). There is no rustfmt component and no `rustfmt.toml`: the code is hand-formatted with long lines and one-line `if x { return }` bodies. Do not run `cargo fmt`; match the surrounding style.
- Windows needs the MSVC build tools.
- CI (`.github/workflows/ci.yml`) runs clippy with `-D warnings` and the tests on Windows and macOS, and the three windowless crates on Linux. A release is a pushed tag `v<version>` that matches the version in `Cargo.toml` (`release.yml`; notes from `.github/release-notes/v<version>.md`). It is always signed, the way BiblioCell does it: `scripts/package-macos.sh --arch universal` builds, signs with the Developer ID certificate, notarizes and staples the app and the DMG; `scripts/package-windows.ps1 -Sign` signs both exes through SSL.com eSigner (`scripts/sign-windows-esigner.ps1`); Linux is an unsigned tar.gz. The ten secrets are listed at the top of `release.yml`; with one missing the run stops before building. A pull request touching `scripts/` or the workflow rehearses the packaging unsigned. Locally `CARGO="cargo +stable" scripts/package-macos.sh` gives an ad hoc signed `dist/Wisp Paper.app`.
- Demo data: `cargo run -p wisp_core --example seed -- /tmp/wisp-demo some.pdf`, then `WISP_PAPER_HOME=/tmp/wisp-demo cargo run`.
- No API key at hand: `cargo run -p wisp_core --example mock_model -- 8787` is a fake Claude endpoint: in a chat it calls two tools and then answers, asked for search queries by the guide it proposes two for PubMed, asked to screen a paper it decides by one word, and asked to translate (with the default instructions) it hands the text back marked as translated. A second argument (`-- 8787 1500`) delays every answer by that many milliseconds, to watch a screening run; each request is printed with its model and thinking setting. Point a profile in the demo folder's `settings.json` at `http://127.0.0.1:8787` and drive the window with `WISP_PAPER_DEV=open:0,page:ask,ask:<question>`.
- `WISP_PAPER_HOME` moves both the data folder and `settings.json` to one directory; use it for any manual run so the real library in `Documents/WispPaper` is not touched.
- Debug builds only: `WISP_PAPER_DEV=open:0,select:0,tab:pdf` scripts the window into a state at start (steps: `open`, `select`, `tab`, `page`, `wide`, `menu`, `search`, `ask`, `try`, `guide:new|search|go`, `screen` (start the assistant on the pending papers), `translate` (turn translation on for the selected paper), `profile:<n>` (open the n-th model on the AI settings page), `quote:<page>|<sentence>` (highlight a sentence in the selected paper's PDF), `modal`, `settings:<page>`, `theme:dark`, `record:<shortcut id>`, `update`; see `dev_start` at the end of `src/app.rs`). The window does not take focus when it is set. `theme:` is saved to `settings.json`, so combine it with `WISP_PAPER_HOME`.
- `WISP_PAPER_UPDATE_URL` points the release check at another address (it must answer with a redirect to this project's release page).
- In a `#[cfg(test)]` module of a file that has `use gpui::*`, do not write `use super::*`: it brings in gpui's `test` attribute, and `#[test]` then fails with "recursion limit reached". Import the names you need.

## Architecture

A Cargo workspace of four crates. Dependencies point one way only: `wisp_net` depends on core, the app and the command line depend on both.

### `crates/core` (`wisp_core`): the engine, no GPUI

Everything that can be decided without a window or a socket lives here, and it is where nearly all tests are. That includes the pure halves of networked features: `lookup` builds Crossref / Europe PMC URLs and turns their JSON into a `Paper`; `ai` builds the context, the request body for the three protocols (Anthropic Messages, OpenAI-compatible chat completions, OpenAI Responses) and parses stream lines into `StreamEvent`s. The actual HTTP (blocking `ureq`) is in `crates/net` (`wisp_net`), shared by the app and `wisp`; its tests serve canned responses from a local port. Code in core that needs the network takes the fetching function as an argument (`sources::Fetch`), so core tests pass a closure.

On-disk layout: the data root holds one folder per library, each with `library.json` (`libraries`), `wisp-paper.sqlite` and `papers/` (`store`). Attachment paths in the database are relative to the library folder so a folder can be copied elsewhere. Removing a library or attachment moves it to `removed/`; nothing is deleted.

`store.rs` notes:
- The schema is one `CREATE TABLE IF NOT EXISTS` batch guarded by `SCHEMA_VERSION` / `PRAGMA user_version`; a newer database is refused. Bump the version when an older build would misread the data.
- Deduplication is by DOI, PMID (`paper_identifiers`), then `title_key`. Trashed papers stay in `papers` (marked by a `trashed_papers` row) so they keep blocking re-import.
- `Store::in_memory` is what tests use.
- Status values are stored as English keys and shown through the label tables in `model.rs` (`SCREENINGS`, `READINGS`, …). The labels are not translations: the stored key `excluded` is displayed as 「相关」.

`tools.rs` is the one table of what an agent can do with a library; the command line, MCP and (next) the in-app assistant all end in `tools::call`, so a rule belongs there and not in a front end. `ai.rs` holds the model side of the loop: the request with tool definitions for each protocol, `TurnReader` (one streamed turn → text, tool calls, the message to send back), `tool_results`, and the slash commands (`SLASH`, prompt templates behind `/search` and `/cite`).
- A new tool is an entry in `catalog()` (name, Chinese description, JSON Schema) and an arm in `call_in`. Nothing else: `wisp <tool>` reads its flags off the schema (`cli_args`), and `mcp::handle` lists and calls whatever the catalog holds. The test `every_tool_is_callable` fails when an entry has no arm.
- Tools take `library` (id or name, optional when there is one library) and return JSON; an error is a Chinese sentence for the caller, person or model.
- A suggestion never writes `papers.screening_status`: only the window does, in `Cmd::ScreenDecide`, `Cmd::ScreenAccept` (a whole group of suggestions taken at once, after a confirmation) and the detail panel. An excerpt is kept only when `ai::locate_quote` finds it in the abstract or the full text.
- Models per task: `Settings::profiles` holds any number of model services, each with an `id` (given by `Settings::tidy` / `add_profile`, never reused while in use) and its own `effort` (`ai::EFFORTS`: empty sends nothing, `off`, or a level). `Settings::tasks` binds `Task::Chat`, `Screen` and `Translate` to a profile id; `Settings::profile_for` falls back to the chat profile, then the first. `ai::apply_effort` is the one place that knows how each protocol, model and service spells thinking; a new service's way of turning it off goes there.
- Translation: `ai::translate_request` (instructions from `Settings::translation_prompt`, else `ai::TRANSLATION_PROMPT`), kept in the library's `translations` table under `store::translation_key` (text and instructions), so a text is requested once. The window translates the selected paper's title and abstract while `Library::show_translation` is on.
- Proxies: `Settings::proxy` is global and applied with `wisp_net::set_proxy` (app start, 设置 › 网络, and `wisp`); `Profile::proxy` overrides it for one model service (`direct` = none) and travels in `HttpRequest::proxy`. Empty everywhere means ureq reads the environment. `settings::proxy_problem` is the address check.
- `ToolDef::writes` marks the tools that change a library (the `WRITING` list); `wisp mcp --read-only` leaves them out, and the chat reloads the library after one ran. The screening decision stays the person's: an agent may propose, never set `screening`.
- `sources.rs` searches PubMed, Europe PMC, Crossref and arXiv (PubMed and arXiv answer in XML, read by the small `elements` helper). A saved search is a `searches` row; each run is an `import_batches` row with its papers in `batch_papers`, which is what `undo_batch` removes (untouched papers only).

### `crates/cli` (`wisp_cli`): the `wisp` binary

A thin shell over `tools` and `mcp`: argument words in, JSON out; `wisp mcp` is newline-delimited JSON-RPC on stdin/stdout. It is a separate binary because the app is a Windows GUI-subsystem program with no console. It opens the same data folder as the app (`settings::default_root` / `settings_file`), possibly while the app is running.

### Root crate (`wisp_paper`): the GPUI app

- `src/app.rs` holds all state in one root view, `Workspace` (`lib: Option<Library>` is the open library, `None` is the home page). Every click is a `Cmd` enum value, wired with `self.on(Cmd::…, cx)` and handled in `Workspace::run`. Popup menus are built from a `MenuKind` in `open_menu`; dialogs are `Modal` variants, built and submitted in `app.rs`.
- `home.rs`, `library.rs`, `detail.rs`, `chat.rs`, `dialogs.rs` are `impl Workspace` blocks that only render. A new feature usually means: a `Cmd` variant, its arm in `run`, and the element that emits it.
- Store writes go through `Workspace::change`, which runs the closure, calls `Library::reload` (re-reads papers, fields, files, trash and refilters) and shows a toast on failure. `Library::shown` is indices into `papers`, recomputed by `refilter`.
- Background work (PDF text extraction, lookups, chat streaming) runs on `cx.background_executor()` and comes back through `this.update`. Each task captures the library id (chats also a `generation` counter) and stops when the user has since closed the library or started a new conversation; keep that check in new async code.
- A library question's scope is the currently shown list (`lib.shown`), so view, search and filters change what the model reads up front. The context is built once with the first question of a conversation. Conversations are not persisted.
- The chat is an agent loop (`ask` → `chat_step` → `run_tools` → `chat_step` …, at most `MAX_CHAT_STEPS` requests per question). `Chat::wire` is the conversation in the protocol's own messages and is append-only: the model's turns go back exactly as received (`ai::TurnReader` keeps thinking blocks and their signatures), and every tool call gets a result even when it did not run. Tools run on the background executor with their own `Store` connection. Papers are named to the model by label (`P3`): `ai::resolve_labels` on the way in, `ai::add_labels` on the way out.
- An answer is drawn as Markdown: `wisp_core::markdown::parse` (blocks and inline marks, rules ported from quiet.md; what does not close stays as written, since an answer is drawn while it streams) and `src/markdown.rs` (`render`). Citations like `[P3 p.5 "…"]` stay ordinary text; the chips under an answer come from `ai::citations`, the first `SOURCES_SHOWN` of them until `Chat::sources_open` holds the answer's index.
- `settings.rs` is the settings view (通用, 外观, 快捷键, AI, 网络, 关于). On the AI page the models are a list whose open row shows its fields (`ProfileFields`, made anew for the model on show). It replaces the page while `Workspace::settings_view` is `Some`; the open library stays underneath. There is no save button: each change writes `settings.json` at once.
- `shortcuts.rs` owns every key binding. `shortcuts::rebuild` clears and re-binds all of them (the text field's too) from the defaults plus `Settings::shortcuts`; call it after any change. While 设置 › 快捷键 waits for a key, all bindings are cleared and `Workspace::record_key` receives the key. A new shortcut needs an entry in `SHORTCUTS`, an action, its binding in `rebuild`, an `.on_action` in `home.rs` and an arm in `Workspace::shortcut`.
- Motion is `with_animation` through the helpers in `ui.rs` (`spinner`, `fade_in`). An animation's state lives under its element id, so give animated rows an id made from what they show (a paper id), not their position: the screening queue is sorted anew with every suggestion.
- A wrapping `TextInput` under ancestors whose widths are all percentages (the settings column) is measured at 240px; give its wrapper a pixel width there, as `ai_page` does for the translation instructions.
- Colours come only from `theme.rs`, which has a light and a dark set (`theme::dark()`); `Workspace::apply_appearance` picks one from the setting or the system. Do not hard-code colours elsewhere; text on an accent fill is `Theme::on_accent()`, a library colour is `Accent::strong()` / `soft()`.
- The release check (`check_update`) reads the redirect of GitHub's `releases/latest` (the JSON API is rate-limited per address). It only notifies and opens the release page: nothing is downloaded or installed.
- `input.rs` is the app's own text input (single / multi-line, IME via `EntityInputHandler`); `pdf.rs` renders pages with `hayro` on a worker thread into a virtualised list, keeping a bounded number of pages.
- `build.rs` embeds every file under `assets/` (of `branding/`, only `app-icon.png`) into the binary; icons are loaded by name through `ui::icon`. On Windows it also compiles the icon resource.

### `vendor/`

`gpui` 0.2.2, `taffy` 0.9.0 and `xattr` 0.2.3 are vendored with local patches and wired in through `[patch.crates-io]`. Patched spots carry a `quiet.md:` comment and are listed in `vendor/PATCHES.md`. When changing vendored code, add a row there and keep the comment; do not bump these versions without re-applying the patches. GPUI APIs here are 0.2.2 plus those patches (for example `grid_col_widths`, `ListState::estimated_scroll_metrics`), so check `vendor/gpui/src` rather than upstream docs.

## Scope

Ported from BiblioCell, without its datasets and web version. Being added step by step: the tool layer with CLI and MCP (done, read-only tools), the chat as an agent loop over the tools with `/search` and `/cite` (done), search sources with trial queries and batch import (done: 检索与导入 page, `src/searches.rs`), a guide page for a new library that starts from a search (done: `src/guide.rs`; `ai::query_prompt` / `parse_queries` ask the model for queries per database, each is tried with `search_online`, the person picks), AI screening suggestions that the person confirms (鲸吞法: import broadly by query, let the agent judge, the person decides; done: 筛选 page in `src/screen.rs`, `screening_suggestions` table, `ai::screen_prompt` / `parse_judgement`, tool `suggest_screening`), scheduled source sync (done: `LibraryInfo::auto_sync_hours`, tool `sync_searches` with `tools::due_searches`, `Workspace::watch_searches` looks once a minute), and jumping to the quoted sentence in the PDF (done: the assistant may cite `[P3 p.5 "a sentence"]`, parsed by `ai::citations`; `PdfView::find` has the worker interpret the page with a glyph-collecting hayro `Device`, and `fulltext::find_quote` / `line_boxes` turn the match into one box per line, drawn over the page image). The roadmap is complete; what each step left out is in its pull request (2 to 8). API keys are stored in plain text in `settings.json`; when empty, `ANTHROPIC_API_KEY` / `OPENAI_API_KEY` are read from the environment.
