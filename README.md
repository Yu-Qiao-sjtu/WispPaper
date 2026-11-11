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
- **提问**
  - 向文献库提问：范围是当前列表（受视图、搜索、筛选影响），模型读到每篇的记录、笔记，以及已存 PDF 中与问题相关的页面。
  - 就单篇文献提问：模型读到该文献的记录和全文。
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

**会离开本机的数据**：DOI / PMID 查询会把编号发给 Crossref 和 Europe PMC；检查更新会向 GitHub 请求本项目的 `releases/latest`，只读取它跳转到的版本号，不下载、不上传任何内容（可在「设置 › 通用」关闭，新版本需要自己到发布页下载安装）；提问会把问题和相关文献的记录、笔记、全文页面发给你配置的模型服务。API Key 以明文保存在 `settings.json`，留空则读取环境变量 `ANTHROPIC_API_KEY` / `OPENAI_API_KEY`。

## 构建

需要 Rust（版本见 `rust-toolchain.toml`，rustup 会自动安装）。Windows 上需要 MSVC 构建工具。

```sh
cargo run --release                 # 运行
cargo test                          # 核心引擎和网络层的测试
cargo test -- --ignored lookup      # 访问 Crossref / Europe PMC 的真实查询测试
cargo clippy --all-targets          # 应当没有警告
```

生成一个带示例数据的目录来试用：

```sh
cargo run -p wisp_core --example seed -- /tmp/wisp-demo some.pdf
WISP_PAPER_HOME=/tmp/wisp-demo cargo run
```

## 代码结构

仓库是一个 Cargo workspace，分两个 crate，依赖只能从应用指向 core：

- **`crates/core`（`wisp_core`）**：文献库引擎，不依赖 GPUI，可以单独编译和测试。

  | 模块 | 内容 |
  |---|---|
  | `model` | 文献、自定义词条、附件；DOI / PMID 规范化 |
  | `libraries` | 数据目录下的各个文献库 |
  | `store` | 一个文献库：SQLite 数据库和 `papers/` 里的附件 |
  | `filter` | 搜索、筛选、排序、侧栏计数 |
  | `import` / `export` | BibTeX、RIS 读入；BibTeX、CSV 写出 |
  | `lookup` | Crossref / Europe PMC 返回值 → 文献 |
  | `fulltext` | PDF 逐页文字提取 |
  | `ai` | 提问的上下文组装、两种协议的请求体、流式返回的解析 |
  | `settings` | settings.json |
  | `update` | 最新发布的版本号，是否比当前版本新 |

- **根 crate（`wisp_paper`）**：GPUI 应用。

  | 模块 | 内容 |
  |---|---|
  | `app` | `Workspace`：全部状态；每次点击是一个 `Cmd`，由 `run` 处理 |
  | `home` / `library` / `detail` / `chat` / `dialogs` | 各部分界面的绘制 |
  | `settings` | 设置页：通用、外观、快捷键、AI、关于；检查更新 |
  | `shortcuts` | 快捷键的默认值、改键规则和显示 |
  | `input` | 文本输入框（单行 / 多行，支持输入法） |
  | `pdf` | PDF 预览：后台线程渲染，虚拟化列表显示 |
  | `net` | 元数据查询、流式对话和检查更新的 HTTP 请求 |
  | `theme` / `ui` | 浅色 / 深色两套颜色与通用控件；窗口标题栏 |

调试构建下，`WISP_PAPER_DEV=open:0,select:0,tab:pdf` 可以让窗口启动后直接进入某个状态（如 `settings:ai`、`theme:dark`，见 `app.rs` 末尾）。`WISP_PAPER_UPDATE_URL` 可把检查更新指向别的地址（测试用）。

## 第一版的范围

BiblioCell 的数据集、来源定时跟踪、MCP 服务、命令行和网页版没有移植。提问的对话不保存，关闭文献库即清空。界面目前只有中文。

## 许可

MIT，见 [LICENSE](LICENSE)。随附的第三方代码和素材见 [THIRD_PARTY.md](THIRD_PARTY.md)。
