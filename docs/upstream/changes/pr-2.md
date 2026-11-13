# PR #2：Tool layer with the wisp command line and an MCP server

[上游 PR](https://github.com/xuzhougeng/WispPaper/pull/2) · **v0.6.0 已包含**

新版本／提案 head：`e945b7daa5707294dd200244561821f495eadfe5`

旧版本：`a6bf567f8f8c6765ec83a330170f9566cbc10aed`

合并 PR 比较第一父提交与合并结果；未合并 PR 比较固定 main 与 head 的共同祖先。删除行链接旧版本，新增行链接新版本。

[完整逐行数据（gzip JSON）](pr-2.json.gz)：包含每一条新增／删除行的行号与原文。

范围链接覆盖该 hunk 的每一行；`—` 表示该侧没有行。二进制和仅模式变化另行标出。

## `CLAUDE.md`

Git 状态：`M`

| 删除／旧源码 | 新增／新源码 |
|---|---|
| [L11](https://github.com/xuzhougeng/WispPaper/blob/a6bf567f8f8c6765ec83a330170f9566cbc10aed/CLAUDE.md#L11) | [L11–L12](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/CLAUDE.md#L11-L12) |
| [L29](https://github.com/xuzhougeng/WispPaper/blob/a6bf567f8f8c6765ec83a330170f9566cbc10aed/CLAUDE.md#L29) | [L30](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/CLAUDE.md#L30) |
| — | [L44–L52](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/CLAUDE.md#L44-L52) |
| [L63](https://github.com/xuzhougeng/WispPaper/blob/a6bf567f8f8c6765ec83a330170f9566cbc10aed/CLAUDE.md#L63) | [L73](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/CLAUDE.md#L73) |

## `Cargo.lock`

Git 状态：`M`

| 删除／旧源码 | 新增／新源码 |
|---|---|
| — | [L7428–L7435](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/Cargo.lock#L7428-L7435) |
| — | [L7442](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/Cargo.lock#L7442) |

## `Cargo.toml`

Git 状态：`M`

| 删除／旧源码 | 新增／新源码 |
|---|---|
| [L9](https://github.com/xuzhougeng/WispPaper/blob/a6bf567f8f8c6765ec83a330170f9566cbc10aed/Cargo.toml#L9) | [L9](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/Cargo.toml#L9) |
| [L11–L12](https://github.com/xuzhougeng/WispPaper/blob/a6bf567f8f8c6765ec83a330170f9566cbc10aed/Cargo.toml#L11-L12) | [L11–L12](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/Cargo.toml#L11-L12) |

## `README.md`

Git 状态：`M`

| 删除／旧源码 | 新增／新源码 |
|---|---|
| — | [L46–L86](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/README.md#L46-L86) |
| [L52](https://github.com/xuzhougeng/WispPaper/blob/a6bf567f8f8c6765ec83a330170f9566cbc10aed/README.md#L52) | [L93](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/README.md#L93) |
| [L66](https://github.com/xuzhougeng/WispPaper/blob/a6bf567f8f8c6765ec83a330170f9566cbc10aed/README.md#L66) | [L107](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/README.md#L107) |
| [L80](https://github.com/xuzhougeng/WispPaper/blob/a6bf567f8f8c6765ec83a330170f9566cbc10aed/README.md#L80) | [L121–L123](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/README.md#L121-L123) |
| — | [L126–L127](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/README.md#L126-L127) |
| [L100](https://github.com/xuzhougeng/WispPaper/blob/a6bf567f8f8c6765ec83a330170f9566cbc10aed/README.md#L100) | [L145](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/README.md#L145) |

## `crates/cli/Cargo.toml`

Git 状态：`A`

| 删除／旧源码 | 新增／新源码 |
|---|---|
| — | [L1–L17](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/cli/Cargo.toml#L1-L17) |

## `crates/cli/src/main.rs`

Git 状态：`A`

| 删除／旧源码 | 新增／新源码 |
|---|---|
| — | [L1–L172](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/cli/src/main.rs#L1-L172) |

## `crates/core/Cargo.toml`

Git 状态：`M`

| 删除／旧源码 | 新增／新源码 |
|---|---|
| — | [L14](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/core/Cargo.toml#L14) |

## `crates/core/src/lib.rs`

Git 状态：`M`

| 删除／旧源码 | 新增／新源码 |
|---|---|
| [L13](https://github.com/xuzhougeng/WispPaper/blob/a6bf567f8f8c6765ec83a330170f9566cbc10aed/crates/core/src/lib.rs#L13) | [L13–L15](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/core/src/lib.rs#L13-L15) |
| — | [L25](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/core/src/lib.rs#L25) |
| — | [L30](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/core/src/lib.rs#L30) |

## `crates/core/src/mcp.rs`

Git 状态：`A`

| 删除／旧源码 | 新增／新源码 |
|---|---|
| — | [L1–L94](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/core/src/mcp.rs#L1-L94) |

## `crates/core/src/settings.rs`

Git 状态：`M`

| 删除／旧源码 | 新增／新源码 |
|---|---|
| — | [L34–L51](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/core/src/settings.rs#L34-L51) |

## `crates/core/src/tools.rs`

Git 状态：`A`

| 删除／旧源码 | 新增／新源码 |
|---|---|
| — | [L1–L559](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/core/src/tools.rs#L1-L559) |

## `src/app.rs`

Git 状态：`M`

| 删除／旧源码 | 新增／新源码 |
|---|---|
| [L19](https://github.com/xuzhougeng/WispPaper/blob/a6bf567f8f8c6765ec83a330170f9566cbc10aed/src/app.rs#L19) | [L19–L20](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/app.rs#L19-L20) |
| [L253–L264](https://github.com/xuzhougeng/WispPaper/blob/a6bf567f8f8c6765ec83a330170f9566cbc10aed/src/app.rs#L253-L264) | — |
