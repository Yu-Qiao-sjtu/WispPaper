# Wisp Paper

本地优先的文献库桌面应用：收集文献、整理、预览 PDF、对文献库提问。用 Rust 和 [GPUI](https://www.gpui.rs/) 写成，支持 Windows 和 macOS（Linux 可以编译，未测试）。

文献部分移植自 [BiblioCell](https://github.com/xuzhougeng/BiblioCell)，应用结构参考 [quiet-md](https://github.com/s1dashu/quiet-md)。

## 功能

- **多个独立的文献库**：启动后的首页是文献库列表。每个文献库有自己的名称、主题描述和标识色，是数据目录下的一个独立文件夹（数据库、PDF、自定义词条都在里面），可以单独备份或拷走。
- **收集**
  - 粘贴 DOI 或 PMID：从 Crossref 和 Europe PMC 查询题目、作者、期刊、年份、摘要。
  - 拖入或选择 PDF：提取文字，在前两页找 DOI 并查询元数据；找不到 DOI 时以文件名建条目。
  - 导入 BibTeX / RIS；手动录入。
  - 按 DOI、PMID、题目去重，重复导入不会产生重复条目。
- **整理**
  - 搜索（题目、作者、DOI、标签、词条、摘要、笔记）、筛选、排序。
  - 阅读状态、收录判断、收藏、标签、笔记。
  - **自定义词条**：每个文献库自己定义字段（文本 / 多值 / 单选），可填写、筛选、作为表格列。
  - 回收站：删除的文献可恢复，并继续阻止同一篇被重复导入。
  - 导出 BibTeX、CSV（当前列表）。
- **PDF 预览**：在详情面板里直接看 PDF，可展开成阅读宽度。渲染用纯 Rust 的 [hayro](https://github.com/LaurenzV/hayro)，不依赖系统 PDF 库。
- **建库向导**
  - 新建文献库后先问从哪里开始：从检索文献开始、导入已有的文献，或者先空着。
  - 从检索开始：写下主题或关键词，助手为所选的数据库各拟几条取向不同的检索式，并逐条试检索；你按命中的文献选出靠谱的，保存并导入。已有的文献库也可以从「检索与导入」页进入。
- **检索与导入**
  - 为 PubMed、Europe PMC、Crossref、arXiv 保存检索式；「试检索」显示数据库命中总数和最新的记录，并标出哪些已在库中。
  - 「运行并导入」把新文献加入文献库（都是「待判断」），已有的跳过；每次导入可以整批撤销，处理过的文献会保留。
  - 自动更新（默认关闭）：每 6 小时、每天或每周运行已启用的检索式。应用开着时每分钟检查一次；关着时错过的，下次打开时补上；失败的最多一小时后重试。不开窗口时可以让系统的计划任务定期执行 `wisp sync_searches`。
- **筛选**
  - 「筛选」页列出待判断的文献。助手逐篇阅读（有全文读全文，否则读摘要），给出建议、理由、一句逐字引自原文的依据和把握；引文不在原文里的不会被当作依据。
  - 建议只是建议：每一篇的收录判断由你确认，可以采纳，也可以另选。外部 agent 通过 MCP 或命令行也只能提建议。
- **提问**
  - 向文献库提问：范围是当前列表（受视图、搜索、筛选影响），模型读到每篇的记录、笔记，以及已存 PDF 中与问题相关的页面。
  - 就单篇文献提问：模型读到该文献的记录和全文。
  - 材料不够时，助手会自己调用工具在整个文献库里检索全文、按页翻阅、查询记录，每次调用都显示在对话里。
  - 回答里的出处可以点击：打开这篇文献的 PDF 并跳到所引的页；助手引了原句时，这句话会在页面上高亮（那一页没有就在全文里找，找不到会明说）。筛选建议里的原文依据同样可以点击。
  - `/search 词语` 在全文和记录里检索；`/cite 一段文字` 为这段文字逐句从库里找可以引用的文献。
  - 回答里的 `[P3 p.5]` 出处可点击，跳到对应文献或 PDF 页。
  - 支持 Claude API 和任何 OpenAI 兼容的服务。
- **设置**：单独的设置页（首页或文献库侧栏的「设置」，默认 `Ctrl` / `⌘` + `,`），改动即时保存。
  - 通用：数据目录、自动检查更新。
  - 外观：跟随系统 / 浅色 / 深色，文献列表的行高。
  - 快捷键：搜索、添加文献、导入 PDF、提问等都可以改键、移除或恢复默认。
  - AI：提问使用的模型服务。
  - 关于：版本、检查更新、项目主页。

## 数据在哪里

| 内容 | 位置 |
|---|---|
| 文献库 | `文档/WispPaper/<文献库 ID>/`，内含 `library.json`、`wisp-paper.sqlite`、`papers/` |
| 被移除的文献库、附件 | 同目录下的 `removed/`（只移动，不删除） |
| 设置 | 系统配置目录下的 `WispPaper/settings.json` |

数据目录可以在「设置 › 通用」里更改。环境变量 `WISP_PAPER_HOME` 可把数据和设置一起指到别处（测试用）。

**会离开本机的数据**：DOI / PMID 查询会把编号发给 Crossref 和 Europe PMC；试检索和运行检索式（包括自动更新）会把检索式发给所选的数据库（PubMed、Europe PMC、Crossref 或 arXiv）；向导拟检索式时会把文献库的名称、主题和你写的关键词发给你配置的模型服务；助手筛选时会把筛选标准和每篇文献的记录、摘要、全文发给它；检查更新会向 GitHub 请求本项目的 `releases/latest`，只读取它跳转到的版本号，不下载、不上传任何内容（可在「设置 › 通用」关闭，新版本需要自己到发布页下载安装）；提问会把问题和相关文献的记录、笔记、全文页面发给你配置的模型服务。API Key 以明文保存在 `settings.json`，留空则读取环境变量 `ANTHROPIC_API_KEY` / `OPENAI_API_KEY`。

## 命令行与 MCP

`wisp` 是不开窗口的入口，读写和应用同一个数据目录，应用开着时也能用。它提供的每个工具既是一个子命令，也是一个 MCP 工具，应用内的助手用的也是同一张工具表。

```sh
cargo build --release -p wisp_cli       # 生成 target/release/wisp

wisp help                               # 命令一览
wisp list_libraries
wisp list_papers -l 大语言模型 --query attention --limit 10
wisp get_paper <文献 ID>
wisp search_fulltext "10x Genomics Chromium"
wisp read_fulltext <文献 ID> --page 3
wisp search_fulltext --help             # 一个工具的参数
```

输出是 JSON；出错时信息写到 stderr，退出码为 1。只有一个文献库时可以省略 `-l`；`--data-dir 文件夹` 指定另一个数据目录。

工具：

| 工具 | 用途 |
|---|---|
| `list_libraries` | 所有文献库的 ID、名称、主题和文献数量 |
| `library_status` | 一个库的概况：各状态的数量、自定义词条、状态值的显示名称 |
| `list_papers` | 搜索、筛选、翻页；规则与界面上的列表相同 |
| `get_paper` | 一篇文献的完整记录、词条和附件 |
| `search_fulltext` | 在已保存的全文里找短语或关键词，返回页码和逐字片段 |
| `read_fulltext` | 按页读取全文，一次最多 8 页 |
| `find_citations` | 为一段文字逐句找出库里可能支持它的文献（按词语匹配的候选） |
| `search_online` | 在 PubMed、Europe PMC、Crossref 或 arXiv 试运行一个检索式，不写入；标出每条结果是否已在库中 |
| `list_searches` / `save_search` / `remove_search` | 查看、保存、删除检索式（后两个会改动文献库） |
| `run_search` | 运行一个保存的检索式并导入，已有的文献跳过，返回批次号（会改动文献库） |
| `list_imports` / `undo_import` | 导入记录；整批撤销一次导入，处理过的文献保留（撤销会改动文献库） |
| `sync_searches` | 运行到期的检索式（所有开启了自动更新的文献库，或指定的一个）；`--force` 不看间隔（会改动文献库） |
| `suggest_screening` | 为一篇文献提出收录建议（结论、理由、逐字引文、把握）；不改变收录判断，由用户在筛选页确认 |

全文检索第一次用到某个 PDF 时会提取它的文字并缓存到库里，之后即时返回。

把 `wisp mcp` 配置给 AI 客户端，它就能通过 MCP（标准输入输出）使用同一组工具；`wisp mcp --read-only` 不提供会改动文献库的工具：

```json
{
  "mcpServers": {
    "wisp-paper": { "command": "/path/to/wisp", "args": ["mcp"] }
  }
}
```

## 构建

需要 Rust（版本见 `rust-toolchain.toml`，rustup 会自动安装）。Windows 上需要 MSVC 构建工具。

```sh
cargo run --release                 # 运行
cargo test                          # 核心引擎、命令行和网络层的测试
cargo test -- --ignored lookup      # 访问 Crossref / Europe PMC 的真实查询测试
cargo clippy --all-targets          # 应当没有警告
```

生成一个带示例数据的目录来试用：

```sh
cargo run -p wisp_core --example seed -- /tmp/wisp-demo some.pdf
WISP_PAPER_HOME=/tmp/wisp-demo cargo run
```

## 代码结构

仓库是一个 Cargo workspace，分四个 crate：core 不依赖其他三个，net 只依赖 core，应用和命令行依赖这两个：

- **`crates/core`（`wisp_core`）**：文献库引擎，不依赖 GPUI，可以单独编译和测试。

  | 模块 | 内容 |
  |---|---|
  | `model` | 文献、自定义词条、附件；DOI / PMID 规范化 |
  | `libraries` | 数据目录下的各个文献库 |
  | `store` | 一个文献库：SQLite 数据库和 `papers/` 里的附件 |
  | `filter` | 搜索、筛选、排序、侧栏计数 |
  | `import` / `export` | BibTeX、RIS 读入；BibTeX、CSV 写出 |
  | `lookup` | Crossref / Europe PMC 返回值 → 文献 |
  | `sources` | 文献数据库检索：要请求的地址，以及从返回内容读出的记录 |
  | `fulltext` | PDF 逐页文字提取；引文与页面字形的比对 |
  | `ai` | 提问的上下文组装、两种协议的请求体（含工具定义）、流式返回和工具调用的解析、斜杠命令 |
  | `settings` | settings.json，以及它和数据目录的位置 |
  | `tools` | agent 能对文献库做的事：命令行、MCP 和应用内助手共用的一张工具表 |
  | `mcp` | 这些工具的 MCP（JSON-RPC）应答 |
  | `update` | 最新发布的版本号，是否比当前版本新 |

- **`crates/net`（`wisp_net`）**：HTTP 请求（阻塞的 ureq）：元数据查询、文献检索、流式对话、检查更新。

- **`crates/cli`（`wisp_cli`）**：可执行文件 `wisp`。把命令行参数按工具的 schema 转成调用，`wisp mcp` 在标准输入输出上收发 MCP 消息。

- **根 crate（`wisp_paper`）**：GPUI 应用。

  | 模块 | 内容 |
  |---|---|
  | `app` | `Workspace`：全部状态；每次点击是一个 `Cmd`，由 `run` 处理 |
  | `home` / `library` / `detail` / `chat` / `dialogs` | 各部分界面的绘制 |
  | `settings` | 设置页：通用、外观、快捷键、AI、关于；检查更新 |
  | `shortcuts` | 快捷键的默认值、改键规则和显示 |
  | `input` | 文本输入框（单行 / 多行，支持输入法） |
  | `pdf` | PDF 预览：后台线程渲染，虚拟化列表显示；在页面的字形里定位引文并高亮 |
  | `guide` | 建库向导：从哪里开始，助手拟的检索式和它们的试检索结果 |
  | `searches` | 「检索与导入」页面：检索式、试检索结果、导入记录 |
  | `screen` | 「筛选」页面：待判断的队列、助手的建议、确认按钮 |
  | `theme` / `ui` | 浅色 / 深色两套颜色与通用控件；窗口标题栏 |

调试构建下，`WISP_PAPER_DEV=open:0,select:0,tab:pdf` 可以让窗口启动后直接进入某个状态（如 `settings:ai`、`theme:dark`，见 `app.rs` 末尾）。`WISP_PAPER_UPDATE_URL` 可把检查更新指向别的地址（测试用）。

## 第一版的范围

BiblioCell 的数据集和网页版没有移植。提问的对话不保存，关闭文献库即清空。界面目前只有中文。

## 许可

MIT，见 [LICENSE](LICENSE)。随附的第三方代码和素材见 [THIRD_PARTY.md](THIRD_PARTY.md)。
