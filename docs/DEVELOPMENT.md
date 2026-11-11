# 开发

## 构建

需要 Rust（版本见 `rust-toolchain.toml`，rustup 会自动安装）。Windows 上需要 MSVC 构建工具。

```sh
cargo run --release                 # 运行
cargo test                          # 核心引擎、命令行和网络层的测试
cargo test -- --ignored lookup      # 访问 Crossref / Europe PMC 的真实查询测试
cargo clippy --all-targets          # 应当没有警告
```

代码是手工排版的，仓库里没有 rustfmt 配置，请不要运行 `cargo fmt`，照着周围的风格写。

生成一个带示例数据的目录来试用：

```sh
cargo run -p wisp_core --example seed -- /tmp/wisp-demo some.pdf
WISP_PAPER_HOME=/tmp/wisp-demo cargo run
```

手头没有 API Key 时，`cargo run -p wisp_core --example mock_model -- 8787` 是一个假的 Claude 接口；把示例目录 `settings.json` 里的一个模型服务指向 `http://127.0.0.1:8787` 即可。

## 调试用的环境变量

| 变量 | 作用 |
|---|---|
| `WISP_PAPER_HOME` | 把数据目录和 `settings.json` 一起指到一个文件夹，手动试用时不碰真正的文献库 |
| `WISP_PAPER_DEV` | 仅调试构建：让窗口启动后直接进入某个状态，如 `open:0,select:0,tab:pdf`、`settings:ai`、`theme:dark`（步骤见 `src/app.rs` 末尾的 `dev_start`） |
| `WISP_PAPER_UPDATE_URL` | 把检查更新指向别的地址（测试用） |

## 代码结构

仓库是一个 Cargo workspace，分四个 crate：core 不依赖其他三个，net 只依赖 core，应用和命令行依赖这两个。

### `crates/core`（`wisp_core`）

文献库引擎，不依赖 GPUI，可以单独编译和测试。

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
| `ai` | 提问的上下文组装、三种协议的请求体（含工具定义）、流式返回和工具调用的解析、斜杠命令 |
| `settings` | settings.json，以及它和数据目录的位置 |
| `tools` | agent 能对文献库做的事：命令行、MCP 和应用内助手共用的一张工具表 |
| `mcp` | 这些工具的 MCP（JSON-RPC）应答 |
| `update` | 最新发布的版本号，是否比当前版本新 |

### `crates/net`（`wisp_net`）

HTTP 请求（阻塞的 ureq）：元数据查询、文献检索、流式对话、检查更新。

### `crates/cli`（`wisp_cli`）

可执行文件 `wisp`。把命令行参数按工具的 schema 转成调用，`wisp mcp` 在标准输入输出上收发 MCP 消息。

### 根 crate（`wisp_paper`）

GPUI 应用。

| 模块 | 内容 |
|---|---|
| `app` | `Workspace`：全部状态；每次点击是一个 `Cmd`，由 `run` 处理 |
| `home` / `library` / `detail` / `chat` / `dialogs` | 各部分界面的绘制 |
| `settings` | 设置页：通用、外观、快捷键、AI、网络、关于；检查更新 |
| `shortcuts` | 快捷键的默认值、改键规则和显示 |
| `input` | 文本输入框（单行 / 多行，支持输入法） |
| `pdf` | PDF 预览：后台线程渲染，虚拟化列表显示；在页面的字形里定位引文并高亮 |
| `guide` | 建库向导：从哪里开始，助手拟的检索式和它们的试检索结果 |
| `searches` | 「检索与导入」页面：检索式、试检索结果、导入记录 |
| `screen` | 「筛选」页面：待判断的队列、助手的建议、确认按钮 |
| `theme` / `ui` | 浅色 / 深色两套颜色与通用控件；窗口标题栏 |

### `vendor/`

`gpui`、`taffy` 和 `xattr` 带着本地补丁放在仓库里，通过 `[patch.crates-io]` 接入。改过的地方有 `quiet.md:` 注释，并列在 `vendor/PATCHES.md`。
