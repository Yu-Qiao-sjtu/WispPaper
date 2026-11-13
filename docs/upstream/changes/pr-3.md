# PR #3：The chat as an agent loop over the library's tools

[上游 PR](https://github.com/xuzhougeng/WispPaper/pull/3) · **v0.6.0 已包含**

新版本／提案 head：`c047804db4da701afd2180380520d322b88d9261`

旧版本：`e945b7daa5707294dd200244561821f495eadfe5`

合并 PR 比较第一父提交与合并结果；未合并 PR 比较固定 main 与 head 的共同祖先。删除行链接旧版本，新增行链接新版本。

[完整逐行数据（gzip JSON）](pr-3.json.gz)：包含每一条新增／删除行的行号与原文。

范围链接覆盖该 hunk 的每一行；`—` 表示该侧没有行。二进制和仅模式变化另行标出。

## `CLAUDE.md`

Git 状态：`M`

| 删除／旧源码 | 新增／新源码 |
|---|---|
| — | [L23](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/CLAUDE.md#L23) |
| [L24](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/CLAUDE.md#L24) | [L25](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/CLAUDE.md#L25) |
| [L44](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/CLAUDE.md#L44) | [L45](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/CLAUDE.md#L45) |
| [L59](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/CLAUDE.md#L59) | [L60–L61](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/CLAUDE.md#L60-L61) |
| [L73](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/CLAUDE.md#L73) | [L75](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/CLAUDE.md#L75) |

## `README.md`

Git 状态：`M`

| 删除／旧源码 | 新增／新源码 |
|---|---|
| — | [L25–L26](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/README.md#L25-L26) |
| — | [L76](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/README.md#L76) |
| [L120](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/README.md#L120) | [L123](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/README.md#L123) |
| [L145](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/README.md#L145) | [L148](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/README.md#L148) |

## `crates/core/examples/mock_model.rs`

Git 状态：`A`

| 删除／旧源码 | 新增／新源码 |
|---|---|
| — | [L1–L70](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/examples/mock_model.rs#L1-L70) |

## `crates/core/src/ai.rs`

Git 状态：`M`

| 删除／旧源码 | 新增／新源码 |
|---|---|
| [L4–L7](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/core/src/ai.rs#L4-L7) | [L4–L10](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/ai.rs#L4-L10) |
| — | [L14](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/ai.rs#L14) |
| [L58–L63](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/core/src/ai.rs#L58-L63) | — |
| — | [L100–L113](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/ai.rs#L100-L113) |
| — | [L275–L276](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/ai.rs#L275-L276) |
| [L312–L313](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/core/src/ai.rs#L312-L313) | [L326–L340](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/ai.rs#L326-L340) |
| [L315–L316](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/core/src/ai.rs#L315-L316) | [L342–L343](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/ai.rs#L342-L343) |
| [L319](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/core/src/ai.rs#L319) | — |
| [L334](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/core/src/ai.rs#L334) | [L360](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/ai.rs#L360) |
| [L336–L339](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/core/src/ai.rs#L336-L339) | [L362–L372](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/ai.rs#L362-L372) |
| [L345–L350](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/core/src/ai.rs#L345-L350) | [L378–L383](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/ai.rs#L378-L383) |
| [L355](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/core/src/ai.rs#L355) | [L388–L415](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/ai.rs#L388-L415) |
| — | [L417](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/ai.rs#L417) |
| [L358](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/core/src/ai.rs#L358) | [L419–L420](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/ai.rs#L419-L420) |
| [L360–L361](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/core/src/ai.rs#L360-L361) | [L422](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/ai.rs#L422) |
| [L365](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/core/src/ai.rs#L365) | [L426–L433](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/ai.rs#L426-L433) |
| [L367–L374](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/core/src/ai.rs#L367-L374) | [L435–L449](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/ai.rs#L435-L449) |
| [L376–L380](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/core/src/ai.rs#L376-L380) | [L451–L489](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/ai.rs#L451-L489) |
| [L382–L383](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/core/src/ai.rs#L382-L383) | [L491–L505](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/ai.rs#L491-L505) |
| [L385–L390](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/core/src/ai.rs#L385-L390) | — |
| — | [L508–L604](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/ai.rs#L508-L604) |
| [L393](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/core/src/ai.rs#L393) | [L606–L633](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/ai.rs#L606-L633) |
| [L486](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/core/src/ai.rs#L486) | [L726–L727](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/ai.rs#L726-L727) |
| [L488](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/core/src/ai.rs#L488) | [L729](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/ai.rs#L729) |
| [L493](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/core/src/ai.rs#L493) | [L734–L736](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/ai.rs#L734-L736) |
| [L495–L497](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/core/src/ai.rs#L495-L497) | [L738–L742](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/ai.rs#L738-L742) |
| — | [L744](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/ai.rs#L744) |
| [L501](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/core/src/ai.rs#L501) | [L747](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/ai.rs#L747) |
| [L505](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/core/src/ai.rs#L505) | [L751–L801](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/ai.rs#L751-L801) |
| [L509–L521](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/core/src/ai.rs#L509-L521) | [L805–L876](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/ai.rs#L805-L876) |
| — | [L879–L915](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/ai.rs#L879-L915) |

## `crates/core/src/mcp.rs`

Git 状态：`M`

| 删除／旧源码 | 新增／新源码 |
|---|---|
| [L33](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/core/src/mcp.rs#L33) | [L33](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/mcp.rs#L33) |

## `crates/core/src/tools.rs`

Git 状态：`M`

| 删除／旧源码 | 新增／新源码 |
|---|---|
| — | [L20–L21](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/tools.rs#L20-L21) |
| [L27](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/core/src/tools.rs#L27) | [L29](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/tools.rs#L29) |
| [L29](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/core/src/tools.rs#L29) | [L31](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/tools.rs#L31) |
| — | [L43](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/tools.rs#L43) |
| [L44](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/core/src/tools.rs#L44) | [L47](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/tools.rs#L47) |
| — | [L50](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/tools.rs#L50) |
| [L61](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/core/src/tools.rs#L61) | [L65](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/tools.rs#L65) |
| — | [L68](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/tools.rs#L68) |
| — | [L79](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/tools.rs#L79) |
| — | [L88–L97](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/tools.rs#L88-L97) |
| [L85–L87](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/crates/core/src/tools.rs#L85-L87) | [L101–L112](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/tools.rs#L101-L112) |
| — | [L121–L177](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/tools.rs#L121-L177) |
| — | [L189](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/tools.rs#L189) |
| — | [L191](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/tools.rs#L191) |
| — | [L424–L495](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/tools.rs#L424-L495) |
| — | [L664–L699](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/crates/core/src/tools.rs#L664-L699) |

## `src/app.rs`

Git 状态：`M`

| 删除／旧源码 | 新增／新源码 |
|---|---|
| [L14](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/app.rs#L14) | [L14–L15](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/app.rs#L14-L15) |
| [L22](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/app.rs#L22) | [L23](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/app.rs#L23) |
| — | [L165–L178](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/app.rs#L165-L178) |
| [L167](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/app.rs#L167) | [L182–L189](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/app.rs#L182-L189) |
| [L261](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/app.rs#L261) | [L283–L295](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/app.rs#L283-L295) |
| — | [L300–L302](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/app.rs#L300-L302) |
| — | [L1123–L1130](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/app.rs#L1123-L1130) |
| — | [L1132–L1134](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/app.rs#L1132-L1134) |
| [L1109](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/app.rs#L1109) | [L1157–L1160](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/app.rs#L1157-L1160) |
| [L1111–L1114](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/app.rs#L1111-L1114) | [L1162–L1184](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/app.rs#L1162-L1184) |
| — | [L1187–L1202](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/app.rs#L1187-L1202) |
| [L1118](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/app.rs#L1118) | [L1204–L1205](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/app.rs#L1204-L1205) |
| [L1121–L1122](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/app.rs#L1121-L1122) | [L1208](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/app.rs#L1208) |
| — | [L1210](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/app.rs#L1210) |
| [L1127](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/app.rs#L1127) | — |
| [L1140–L1143](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/app.rs#L1140-L1143) | [L1226–L1230](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/app.rs#L1226-L1230) |
| [L1145–L1149](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/app.rs#L1145-L1149) | [L1232](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/app.rs#L1232) |
| [L1151](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/app.rs#L1151) | [L1234–L1235](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/app.rs#L1234-L1235) |
| [L1153–L1159](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/app.rs#L1153-L1159) | [L1237–L1257](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/app.rs#L1237-L1257) |
| [L1161](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/app.rs#L1161) | [L1259](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/app.rs#L1259) |
| — | [L1261](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/app.rs#L1261) |
| [L1164](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/app.rs#L1164) | [L1263](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/app.rs#L1263) |
| — | [L1269–L1332](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/app.rs#L1269-L1332) |
| — | [L1559–L1563](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/app.rs#L1559-L1563) |

## `src/chat.rs`

Git 状态：`M`

| 删除／旧源码 | 新增／新源码 |
|---|---|
| [L3](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/chat.rs#L3) | [L3–L4](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/chat.rs#L3-L4) |
| [L5](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/chat.rs#L5) | [L6](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/chat.rs#L6) |
| [L48–L52](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/chat.rs#L48-L52) | [L49–L76](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/chat.rs#L49-L76) |
| [L57](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/chat.rs#L57) | [L81](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/chat.rs#L81) |
| [L90](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/chat.rs#L90) | [L114](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/chat.rs#L114) |
| [L107](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/chat.rs#L107) | [L131](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/chat.rs#L131) |
| [L158](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/chat.rs#L158) | [L182](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/chat.rs#L182) |

## `src/net.rs`

Git 状态：`M`

| 删除／旧源码 | 新增／新源码 |
|---|---|
| [L7](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/net.rs#L7) | [L7](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/net.rs#L7) |
| [L80–L82](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/net.rs#L80-L82) | [L80–L83](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/net.rs#L80-L83) |
| [L92](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/net.rs#L92) | [L93](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/net.rs#L93) |
| [L99](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/net.rs#L99) | [L100](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/net.rs#L100) |
| [L102](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/net.rs#L102) | [L103](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/net.rs#L103) |
| [L107](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/net.rs#L107) | [L108](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/net.rs#L108) |
| [L111–L113](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/net.rs#L111-L113) | [L112–L120](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/net.rs#L112-L120) |
| [L116–L118](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/net.rs#L116-L118) | [L123](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/net.rs#L123) |
| [L125](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/net.rs#L125) | [L130](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/net.rs#L130) |
| [L156](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/net.rs#L156) | [L161–L162](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/net.rs#L161-L162) |
| [L159](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/net.rs#L159) | [L165](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/net.rs#L165) |
| — | [L175–L216](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/net.rs#L175-L216) |
| [L174](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/net.rs#L174) | [L222–L224](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/net.rs#L222-L224) |
| [L181](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/net.rs#L181) | [L231–L234](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/net.rs#L231-L234) |
| [L185](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/net.rs#L185) | [L238](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/net.rs#L238) |
| [L194](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/net.rs#L194) | [L247–L248](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/net.rs#L247-L248) |
| [L199](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/net.rs#L199) | [L253](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/net.rs#L253) |
| [L204](https://github.com/xuzhougeng/WispPaper/blob/e945b7daa5707294dd200244561821f495eadfe5/src/net.rs#L204) | [L258](https://github.com/xuzhougeng/WispPaper/blob/c047804db4da701afd2180380520d322b88d9261/src/net.rs#L258) |
