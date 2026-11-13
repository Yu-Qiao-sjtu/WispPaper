# PR #5：A guide for a new library that starts from a search

[上游 PR](https://github.com/xuzhougeng/WispPaper/pull/5) · **v0.6.0 已包含**

新版本／提案 head：`05abcb7d26cc8999bd2ddb9a214d4358d5cf6fcc`

旧版本：`f4699d640c17fdba7a11cae0c99f54de70b0c9e9`

合并 PR 比较第一父提交与合并结果；未合并 PR 比较固定 main 与 head 的共同祖先。删除行链接旧版本，新增行链接新版本。

[完整逐行数据（gzip JSON）](pr-5.json.gz)：包含每一条新增／删除行的行号与原文。

范围链接覆盖该 hunk 的每一行；`—` 表示该侧没有行。二进制和仅模式变化另行标出。

## `CLAUDE.md`

Git 状态：`M`

| 删除／旧源码 | 新增／新源码 |
|---|---|
| [L23](https://github.com/xuzhougeng/WispPaper/blob/f4699d640c17fdba7a11cae0c99f54de70b0c9e9/CLAUDE.md#L23) | [L23](https://github.com/xuzhougeng/WispPaper/blob/05abcb7d26cc8999bd2ddb9a214d4358d5cf6fcc/CLAUDE.md#L23) |
| [L25](https://github.com/xuzhougeng/WispPaper/blob/f4699d640c17fdba7a11cae0c99f54de70b0c9e9/CLAUDE.md#L25) | [L25](https://github.com/xuzhougeng/WispPaper/blob/05abcb7d26cc8999bd2ddb9a214d4358d5cf6fcc/CLAUDE.md#L25) |
| [L76](https://github.com/xuzhougeng/WispPaper/blob/f4699d640c17fdba7a11cae0c99f54de70b0c9e9/CLAUDE.md#L76) | [L76](https://github.com/xuzhougeng/WispPaper/blob/05abcb7d26cc8999bd2ddb9a214d4358d5cf6fcc/CLAUDE.md#L76) |

## `README.md`

Git 状态：`M`

| 删除／旧源码 | 新增／新源码 |
|---|---|
| — | [L22–L24](https://github.com/xuzhougeng/WispPaper/blob/05abcb7d26cc8999bd2ddb9a214d4358d5cf6fcc/README.md#L22-L24) |
| [L49](https://github.com/xuzhougeng/WispPaper/blob/f4699d640c17fdba7a11cae0c99f54de70b0c9e9/README.md#L49) | [L52](https://github.com/xuzhougeng/WispPaper/blob/05abcb7d26cc8999bd2ddb9a214d4358d5cf6fcc/README.md#L52) |
| — | [L154](https://github.com/xuzhougeng/WispPaper/blob/05abcb7d26cc8999bd2ddb9a214d4358d5cf6fcc/README.md#L154) |
| [L158](https://github.com/xuzhougeng/WispPaper/blob/f4699d640c17fdba7a11cae0c99f54de70b0c9e9/README.md#L158) | [L162](https://github.com/xuzhougeng/WispPaper/blob/05abcb7d26cc8999bd2ddb9a214d4358d5cf6fcc/README.md#L162) |

## `crates/core/examples/mock_model.rs`

Git 状态：`M`

| 删除／旧源码 | 新增／新源码 |
|---|---|
| [L8](https://github.com/xuzhougeng/WispPaper/blob/f4699d640c17fdba7a11cae0c99f54de70b0c9e9/crates/core/examples/mock_model.rs#L8) | [L8–L9](https://github.com/xuzhougeng/WispPaper/blob/05abcb7d26cc8999bd2ddb9a214d4358d5cf6fcc/crates/core/examples/mock_model.rs#L8-L9) |
| — | [L34–L38](https://github.com/xuzhougeng/WispPaper/blob/05abcb7d26cc8999bd2ddb9a214d4358d5cf6fcc/crates/core/examples/mock_model.rs#L34-L38) |
| [L53](https://github.com/xuzhougeng/WispPaper/blob/f4699d640c17fdba7a11cae0c99f54de70b0c9e9/crates/core/examples/mock_model.rs#L53) | [L59–L65](https://github.com/xuzhougeng/WispPaper/blob/05abcb7d26cc8999bd2ddb9a214d4358d5cf6fcc/crates/core/examples/mock_model.rs#L59-L65) |

## `crates/core/src/ai.rs`

Git 状态：`M`

| 删除／旧源码 | 新增／新源码 |
|---|---|
| — | [L637–L705](https://github.com/xuzhougeng/WispPaper/blob/05abcb7d26cc8999bd2ddb9a214d4358d5cf6fcc/crates/core/src/ai.rs#L637-L705) |
| — | [L976–L1001](https://github.com/xuzhougeng/WispPaper/blob/05abcb7d26cc8999bd2ddb9a214d4358d5cf6fcc/crates/core/src/ai.rs#L976-L1001) |

## `crates/core/src/tools.rs`

Git 状态：`M`

| 删除／旧源码 | 新增／新源码 |
|---|---|
| [L246](https://github.com/xuzhougeng/WispPaper/blob/f4699d640c17fdba7a11cae0c99f54de70b0c9e9/crates/core/src/tools.rs#L246) | [L246–L247](https://github.com/xuzhougeng/WispPaper/blob/05abcb7d26cc8999bd2ddb9a214d4358d5cf6fcc/crates/core/src/tools.rs#L246-L247) |

## `crates/net/src/lib.rs`

Git 状态：`M`

| 删除／旧源码 | 新增／新源码 |
|---|---|
| [L43–L44](https://github.com/xuzhougeng/WispPaper/blob/f4699d640c17fdba7a11cae0c99f54de70b0c9e9/crates/net/src/lib.rs#L43-L44) | — |
| [L46–L49](https://github.com/xuzhougeng/WispPaper/blob/f4699d640c17fdba7a11cae0c99f54de70b0c9e9/crates/net/src/lib.rs#L46-L49) | [L44–L54](https://github.com/xuzhougeng/WispPaper/blob/05abcb7d26cc8999bd2ddb9a214d4358d5cf6fcc/crates/net/src/lib.rs#L44-L54) |
| — | [L56–L72](https://github.com/xuzhougeng/WispPaper/blob/05abcb7d26cc8999bd2ddb9a214d4358d5cf6fcc/crates/net/src/lib.rs#L56-L72) |
| — | [L162–L181](https://github.com/xuzhougeng/WispPaper/blob/05abcb7d26cc8999bd2ddb9a214d4358d5cf6fcc/crates/net/src/lib.rs#L162-L181) |

## `src/app.rs`

Git 状态：`M`

| 删除／旧源码 | 新增／新源码 |
|---|---|
| — | [L32–L33](https://github.com/xuzhougeng/WispPaper/blob/05abcb7d26cc8999bd2ddb9a214d4358d5cf6fcc/src/app.rs#L32-L33) |
| — | [L155–L162](https://github.com/xuzhougeng/WispPaper/blob/05abcb7d26cc8999bd2ddb9a214d4358d5cf6fcc/src/app.rs#L155-L162) |
| — | [L190–L219](https://github.com/xuzhougeng/WispPaper/blob/05abcb7d26cc8999bd2ddb9a214d4358d5cf6fcc/src/app.rs#L190-L219) |
| — | [L307](https://github.com/xuzhougeng/WispPaper/blob/05abcb7d26cc8999bd2ddb9a214d4358d5cf6fcc/src/app.rs#L307) |
| — | [L867–L913](https://github.com/xuzhougeng/WispPaper/blob/05abcb7d26cc8999bd2ddb9a214d4358d5cf6fcc/src/app.rs#L867-L913) |
| — | [L1017](https://github.com/xuzhougeng/WispPaper/blob/05abcb7d26cc8999bd2ddb9a214d4358d5cf6fcc/src/app.rs#L1017) |
| — | [L1039](https://github.com/xuzhougeng/WispPaper/blob/05abcb7d26cc8999bd2ddb9a214d4358d5cf6fcc/src/app.rs#L1039) |
| — | [L1607–L1656](https://github.com/xuzhougeng/WispPaper/blob/05abcb7d26cc8999bd2ddb9a214d4358d5cf6fcc/src/app.rs#L1607-L1656) |
| — | [L1692](https://github.com/xuzhougeng/WispPaper/blob/05abcb7d26cc8999bd2ddb9a214d4358d5cf6fcc/src/app.rs#L1692) |
| [L1553](https://github.com/xuzhougeng/WispPaper/blob/f4699d640c17fdba7a11cae0c99f54de70b0c9e9/src/app.rs#L1553) | [L1694](https://github.com/xuzhougeng/WispPaper/blob/05abcb7d26cc8999bd2ddb9a214d4358d5cf6fcc/src/app.rs#L1694) |
| — | [L1714–L1718](https://github.com/xuzhougeng/WispPaper/blob/05abcb7d26cc8999bd2ddb9a214d4358d5cf6fcc/src/app.rs#L1714-L1718) |
| — | [L1842–L1846](https://github.com/xuzhougeng/WispPaper/blob/05abcb7d26cc8999bd2ddb9a214d4358d5cf6fcc/src/app.rs#L1842-L1846) |

## `src/guide.rs`

Git 状态：`A`

| 删除／旧源码 | 新增／新源码 |
|---|---|
| — | [L1–L171](https://github.com/xuzhougeng/WispPaper/blob/05abcb7d26cc8999bd2ddb9a214d4358d5cf6fcc/src/guide.rs#L1-L171) |

## `src/library.rs`

Git 状态：`M`

| 删除／旧源码 | 新增／新源码 |
|---|---|
| [L42](https://github.com/xuzhougeng/WispPaper/blob/f4699d640c17fdba7a11cae0c99f54de70b0c9e9/src/library.rs#L42) | [L42](https://github.com/xuzhougeng/WispPaper/blob/05abcb7d26cc8999bd2ddb9a214d4358d5cf6fcc/src/library.rs#L42) |
| — | [L56](https://github.com/xuzhougeng/WispPaper/blob/05abcb7d26cc8999bd2ddb9a214d4358d5cf6fcc/src/library.rs#L56) |
| [L152](https://github.com/xuzhougeng/WispPaper/blob/f4699d640c17fdba7a11cae0c99f54de70b0c9e9/src/library.rs#L152) | [L153](https://github.com/xuzhougeng/WispPaper/blob/05abcb7d26cc8999bd2ddb9a214d4358d5cf6fcc/src/library.rs#L153) |

## `src/main.rs`

Git 状态：`M`

| 删除／旧源码 | 新增／新源码 |
|---|---|
| — | [L12](https://github.com/xuzhougeng/WispPaper/blob/05abcb7d26cc8999bd2ddb9a214d4358d5cf6fcc/src/main.rs#L12) |

## `src/searches.rs`

Git 状态：`M`

| 删除／旧源码 | 新增／新源码 |
|---|---|
| — | [L112](https://github.com/xuzhougeng/WispPaper/blob/05abcb7d26cc8999bd2ddb9a214d4358d5cf6fcc/src/searches.rs#L112) |
