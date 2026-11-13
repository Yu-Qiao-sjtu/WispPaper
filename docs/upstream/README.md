# WispPaper 上游演进与 fork 构想追踪

本索引承接已关闭的 [上游概念 issue #29](https://github.com/xuzhougeng/WispPaper/issues/29)，在 [Yu-Qiao-sjtu/WispPaper](https://github.com/Yu-Qiao-sjtu/WispPaper) 中持续整理需求、开发者取舍、实现、发布边界与剩余工作。它是 fork 的观察与规划记录，不代表上游接受了 #29 的全部构想，也不表示已经把上游应用代码合入本 fork。

上游定位是本地优先的文献库与研究写作应用：用 Rust／GPUI 提供收集、检索、阅读、筛选、问答与稿件界面，CLI／MCP 共享文献工具。fork 的演进以可追溯的 AI 写作和证据组织为中心；远端计算、实验与图形工作流通过可选边界逐步连接。

快照时间：**2026-11-13 16:56:49（北京时间）**。本次收录全部 **21 个 issue、57 个 PR**，包括已关闭的历史记录。后续状态变化应另建快照，不能悄悄改写本次依据。

## 1. 版本与讨论边界

| 范围 | 固定提交 | 如何理解 |
|---|---|---|
| 上游 [v0.6.0](https://github.com/xuzhougeng/WispPaper/releases/tag/v0.6.0)，北京时间 2026-11-13 10:27 发布 | `60703bd1251456dd78eb13dad100de764fe3611d` | 以剥离 annotated tag 后的提交判定安装包代码；不是 release 的可变 `targetCommitish: main` |
| 上游发布后的 main 快照 | `facc7b249d04b14064b77eb4e167c7efc7d5732a` | #65–#76 已合并，但不属于 v0.6.0 安装包 |
| 上游开放 PR | #77、#78，head 见各自索引 | 只是提案，未合入上述 main 或 v0.6.0 |
| 本 fork 已发布 v0.5.0 | `c7f84ea5ea52fbea148f592cf5df4fb18044d319` | 与上游同名 v0.5.0 不是同一版本；已有 AI 草稿、意图检索与候选句翻译 |

上游 #29 状态为 CLOSED / COMPLETED。开发者的[最后回复](https://github.com/xuzhougeng/WispPaper/issues/29#issuecomment-6813715185)是：“关于写作，我会继续按照我的设想开发，等我完成后再开相关的讨论。”因此这里承接历史和 fork 自己的规划，不重新打开 #29，不把关闭状态翻译成“宏观构想全部实现”。

查阅入口：

- [全部 PR、前后版本与源码行索引](changes/README.md)：55 个已合并，2 个未合并。
- [上游 #29 历史原文与回复](history-29.md)：保留完整构想和开发者取舍，标明历史属性。
- [31 个固定版本源码入口](sources.md)：解释关键能力；新增、删除的每行原文另存于各 PR 的逐行数据。
- [全部 issue 与开发者回复快照](snapshots/2026-11-13/issues.json)、[PR 说明快照](snapshots/2026-11-13/prs.json)、[PR 讨论／review 快照](snapshots/2026-11-13/pr-discussions.json)、[发布说明](snapshots/2026-11-13/release.json)。其中 #29 的 body 和 comments 原样保留为历史证据；本次查询的 57 个 PR 没有额外评论或 review 记录。
- [校验结果](validation.json)：行号、原文与 Git blob 一一核对；仅为源码审计结果，不是本机运行测试或 Word/PDF 兼容性认证。

## 2. 相对于旧构想，需要修正的判断

| 主题 | 截至快照的发现 | 源码与演进依据 | 仍然不能推断的能力 |
|---|---|---|---|
| AI 写论文 | v0.6.0 已有 AI 改写、翻译、润色、按用户命令处理、分阶段综述生成与人工采纳 | [#47](changes/pr-47.md)、[#56](changes/pr-56.md)、[AI 请求](sources.md#ai-write)、[结果采纳](sources.md#ai-accept) | 不是“只能自己写再配引用”；也不等于已经支持 GitHub-to-Paper 全流程 |
| 中文意图找英文语料 | 选区／当前段落按需生成文献语言的多组查询，再排名和显示上下文 | [#47](changes/pr-47.md)、[查询提示](sources.md#intent-queries)、[入口](sources.md#intent-search) | 与 fork 的显式翻译交互存在重叠，需比较触发、翻译和原文展示，不应整块重复移植 |
| PDF 解析 | hayro 布局字形提取优先，pdf-extract 回退；可识别不同提取失败，并提供模型读页的另一条路径 | [#31](changes/pr-31.md)、[#33](changes/pr-33.md)、[提取入口](sources.md#pdf-read) | 浏览器能选中文字并不证明 WispPaper 可正确提取；未发现 Doc2X 内置集成 |
| 阅读与问答 | 原文定位、缩放、选区入聊天、阅读模式、浮动会话和单篇问答布局已存在 | [#8](changes/pr-8.md)、[#33](changes/pr-33.md)、[#34](changes/pr-34.md)、[#57](changes/pr-57.md)、[#60](changes/pr-60.md) | #60 的实际布局是列表左、问答中、PDF 右，并非原建议的 PDF 必须居中；浮窗只从右下角调整大小 |
| 引文与证据 | 稳定引用键、页码、参考文献／BibTeX 输出、综述证据核对均有实现 | [引用转换](sources.md#citation)、[输出函数](sources.md#citation-export)、[证据派生](sources.md#review-evidence) | 引句存在不等于支持论断；不等于完整 CSL 样式管理或 Word 动态引用域 |
| 证据方案变化 | #59 曾加人工论断／证据表；#61 在发布前改成从引用派生、两张综述表，移除中间复杂迁移方案 | [#59 历史](changes/pr-59.md) → [#61 当前发布方案](changes/pr-61.md)、[来源指纹](sources.md#review-validity) | 不能把 #59 的中间表结构当作 v0.6.0 当前架构继续设计 |
| CiteBox 类图片收藏 | #24 当时 NOT_PLANNED，但 #63 后来已实现 PDF 截图收藏、图注、标签、备注和模型建议 | [#63](changes/pr-63.md)、[持久化](sources.md#figure-library)、[图注](sources.md#figure-caption) | 不能继续说上游没有图片库；图片收藏也不等于机制图生成和可编辑 PPTX |
| 无 DOI 元数据 | #44 使用 Crossref / Europe PMC / arXiv API 候选，由用户选择身份和字段；标题可来自首页启发式 | [#44](changes/pr-44.md)、[API](sources.md#metadata-api)、[人工接受](sources.md#metadata-write) | UI 入口针对 DOI、PMID 都没有的文献；不是完整首页作者／年份抽取，也未覆盖所有 DOI 查询失败情形 |
| 自定义命令与 Skill | #43 已有本地斜杠提示模板；#70 发布后重整模型、任务与命令设置 | [#43](changes/pr-43.md)、[#70](changes/pr-70.md)、[模板展开](sources.md#custom-command) | 没有发送前完整展开预览或独立禁用开关；提示模板不等于可执行 Skill 及 wisp-science 格式兼容 |
| DOCX 内置编辑 | 发布后 #72 支持 DOCX／Markdown 导入、文本编辑、Word 批注与可选修订导出，尽量保留原始 OOXML | [#72](changes/pr-72.md)、[读取](sources.md#main-docx-read)、[写回](sources.md#main-docx-write) | 表格／图片不在编辑器内显示编辑；移动段落可能失去行内格式；原 PR 未在 Word／WPS 或 Word 原生文件上验证，不承诺公式或复杂版式全面兼容 |
| 综述写作范式 | 发布后 #71 在需求与大纲之间增加“已有综述的框架”步骤，可跳过且兼容旧记录 | [#71](changes/pr-71.md)、[确认状态](sources.md#main-framework) | 与 GitHub-to-Paper 按领域搜索范式、实验补证不是同一功能 |
| 助手组织与写入 | 发布后 #67/#68 可提议自定义字段、整理文献、读稿件、把回答追加到稿件并保留快照 | [#67](changes/pr-67.md)、[#68](changes/pr-68.md)、[字段提议](sources.md#main-fields)、[段落追加](sources.md#main-append) | 不能自动替用户最终筛选；AI 引用建议仍需人工核对 |
| PDF 持久标记 | 发布后 #74 可保存下划线、高亮及笔记，并提供只读 agent 工具 | [#74](changes/pr-74.md)、[标记记录](sources.md#main-annotations)、[工具](sources.md#main-annotation-tool) | 源 PDF 未改写；用户笔记与原文证据必须区分，仍不是完整知识库检索策略 |
| 定位与语料质量新修复 | 发布后 #73 修正仅页码引用高亮与陈旧异步结果；#76 过滤表格数字行和注释 | [#73](changes/pr-73.md)、[#76](changes/pr-76.md)、[导航](sources.md#main-page-highlight)、[过滤](sources.md#main-apparatus) | 页码引用只标页框；有引句才定位句子。语料过滤不等于完整表格解析 |
| 正在讨论的编辑器改进 | #77 稿件侧栏大纲、边栏批注；#78 无标题样式 DOCX 的标题猜测、用户确认及章节拆合 | [#77](changes/pr-77.md)、[#78](changes/pr-78.md) | 均未合并，不能写成 v0.6.0 或 main 已交付能力 |

无 DOI 补全的取舍需要特别保留：[开发者明确拒绝浏览器桥接](https://github.com/xuzhougeng/WispPaper/issues/39#issuecomment-6813126542)，接受 API 候选。#39 后来修改过标题／正文，不能按我们最初提交时的名称推断最终范围。记录级来源已有体现，但逐字段来源历史仍是缺口；[Crossref 旧解析器](sources.md#metadata-year)在无发表日期时仍可能回退记录创建年份，需作为准确性风险继续跟踪。

参考文献输出也应按真实调用路径判断：`cite` 有列表和 BibTeX 函数，但[稿件实际导出](sources.md#main-docx-export)没有因此自动获得期刊 CSL 排版。可继续保留外部 citation-export Skill／Pandoc 方案，单列“缺失键、元数据、样式、DOCX 域”的交付边界。

## 3. 全部历史 issue 的约束与归属

以下为全部 21 个 issue。除 #24 为 CLOSED / NOT_PLANNED，其余均 CLOSED / COMPLETED。这里记录实际处理范围和残余问题，而非重新判定上游是否应关闭。

| 上游 issue | 原需求／问题 | 已实现与 PR | 在总构想中的约束／剩余范围 |
|---|---|---|---|
| [#17](https://github.com/xuzhougeng/WispPaper/issues/17) | 批量 PDF 导入进度 | [#33](changes/pr-33.md)、[#50](changes/pr-50.md) | 已有统一进度与详情；后续解析／下载长任务也应延续明确状态 |
| [#18](https://github.com/xuzhougeng/WispPaper/issues/18) | 面板宽度调整 | [#33](changes/pr-33.md)、[#36](changes/pr-36.md)、[#57](changes/pr-57.md)、[#60](changes/pr-60.md) | 调整能力随界面演进；不以某一旧截图推断当前布局 |
| [#19](https://github.com/xuzhougeng/WispPaper/issues/19) | 文献材料浏览 | [#34](changes/pr-34.md) | 附件／材料入口已实现；不等同于 PDF 页面缩略图 |
| [#20](https://github.com/xuzhougeng/WispPaper/issues/20) | PDF 选中 Add to chat | [#34](changes/pr-34.md)；发布后 [#74](changes/pr-74.md) 加持久标记 | 对话选区、永久标记与原文来源分别记录 |
| [#21](https://github.com/xuzhougeng/WispPaper/issues/21) | 历史会话可发现 | [#16](changes/pr-16.md)、[#33](changes/pr-33.md) | 已保存会话；恢复后上下文重建不等于保存所有工具读取结果 |
| [#22](https://github.com/xuzhougeng/WispPaper/issues/22) | 提前提示 PDF 提取失败 | [#31](changes/pr-31.md)、[#33](changes/pr-33.md) | 已区分失败原因；不能把解析失败一律称为扫描件 |
| [#23](https://github.com/xuzhougeng/WispPaper/issues/23) | UI／聊天文字复制与引用 | [#34](changes/pr-34.md)、[#49](changes/pr-49.md) | 扩展选择、复制与右键入口；注意语料／模型回答的来源差别 |
| [#24](https://github.com/xuzhougeng/WispPaper/issues/24) | CiteBox 功能融合 | 当时 NOT_PLANNED；后有 [#63](changes/pr-63.md) 图片库 | 保留原决定，更新后来的能力事实；不强求整库合并 |
| [#25](https://github.com/xuzhougeng/WispPaper/issues/25) | 对照 wisp-science 增强 PDF 解析 | [#31](changes/pr-31.md) | 本地 reader 改进已做；Doc2X／Python 服务不是既有内置能力 |
| [#26](https://github.com/xuzhougeng/WispPaper/issues/26) | 可选中文本 PDF 仍解析异常 | [#33](changes/pr-33.md)、[#34](changes/pr-34.md) | 诊断与替代阅读已有；乱码、多栏顺序仍需具体样本与回归集 |
| [#27](https://github.com/xuzhougeng/WispPaper/issues/27) | PDF 缩放 | [#33](changes/pr-33.md) | 比例／适宽等已有；不承诺 pinch 手势 |
| [#28](https://github.com/xuzhougeng/WispPaper/issues/28) | 原文核对与跳转 | [#8](changes/pr-8.md)、[#34](changes/pr-34.md)、[#59](changes/pr-59.md) → [#61](changes/pr-61.md)；发布后 [#73](changes/pr-73.md) | 原文定位早已存在；继续追踪引用邻接、证据判断及不中断单篇阅读的交互 |
| [#29](https://github.com/xuzhougeng/WispPaper/issues/29) | 研究与写作总构想 | 已关闭；转由本 fork 总索引承接 | 历史提案、开发者取舍与实际交付分开，不将整套构想视为已接受 |
| [#30](https://github.com/xuzhougeng/WispPaper/issues/30) | 问答沉淀笔记／知识库 | [#33](changes/pr-33.md)；发布后 [#74](changes/pr-74.md) | 回答可转笔记；每篇知识库的启停、检索优先级与笔记来源治理仍需设计 |
| [#32](https://github.com/xuzhougeng/WispPaper/issues/32) | 查找并取得全文 | [#34](changes/pr-34.md)、[#35](changes/pr-35.md) | 已有批量查全文／CLI 工具；不等同任意网页自动找、登录与下载 |
| [#39](https://github.com/xuzhougeng/WispPaper/issues/39) | 无 DOI 等记录补全 | [#44](changes/pr-44.md) | API 候选与人工确认；浏览器桥接明确不做；字段溯源和日期回退需追踪 |
| [#40](https://github.com/xuzhougeng/WispPaper/issues/40) | 用户自定义斜杠模板 | [#43](changes/pr-43.md)；发布后 [#70](changes/pr-70.md) | 本地提示模板，无脚本执行、完整发送前预览或单独停用开关 |
| [#45](https://github.com/xuzhougeng/WispPaper/issues/45) | Windows 标题栏拖动与激活 | [#46](changes/pr-46.md) | 共享标题栏 `occlude()` 命中修复；持续保留平台回归要求 |
| [#52](https://github.com/xuzhougeng/WispPaper/issues/52) | 沉浸阅读／浮动问答／翻译 | [#57](changes/pr-57.md) | 已有；浮窗调整范围有限，Windows 原 PR 未实际运行验证 |
| [#55](https://github.com/xuzhougeng/WispPaper/issues/55) | 可恢复的分阶段综述 | [#56](changes/pr-56.md)、[#61](changes/pr-61.md)；发布后 [#71](changes/pr-71.md) | 需求／大纲／正文持久化与确认；后加范式步骤不能算入 v0.6.0 |
| [#58](https://github.com/xuzhougeng/WispPaper/issues/58) | 人工论断／证据核对记录 | [#59](changes/pr-59.md) → [#61](changes/pr-61.md) | 以 #61 从引用派生的发布方案为准，来源变化影响核对有效性 |

## 4. 发布后的 PR 应如何纳入整体路线

| PR | 合并后能力 | fork 对齐时的检查重点 |
|---|---|---|
| [#65](changes/pr-65.md) | CLI／MCP 建库、导入 PDF／参考文献／Zotero | 使用统一工具契约；仓库根 SKILL.md 是外部操作说明，不是应用内 Skill 执行器 |
| [#66](changes/pr-66.md) | 中英 UI 与窄窗口写作布局 | 翻译、布局及 fork 新入口的兼容性 |
| [#67](changes/pr-67.md) | 文献库问答改名“助手”、自定义字段提取建议 | 字段候选需接受，持久化必须事务化 |
| [#68](changes/pr-68.md) | 文献组织、读取稿件、回答追加、引用助手 | 写操作确认、快照、稳定引用与证据边界 |
| [#69](changes/pr-69.md) | 后台运行、托盘、单实例 | Windows 平台验证，不能把维护者 Mac 检查等同全部平台验证 |
| [#70](changes/pr-70.md) | 服务商多模型与模型／任务／命令分区 | 旧设置迁移和任务绑定；原 PR 未测试真实服务商调用 |
| [#71](changes/pr-71.md) | 先查已有综述范式再做大纲 | 确认步骤、旧记录兼容与用户跳过 |
| [#72](changes/pr-72.md) | 文件导入、文本批注、Word 批注／修订写回 | 必须明确兼容范围；优先测真实 Word／WPS 文件、公式、浮动图片和多栏 |
| [#73](changes/pr-73.md) | 仅页码引用的页框、异步定位防过期 | 页框与句子框分开；新导航不能被旧结果覆盖 |
| [#74](changes/pr-74.md) | PDF 持久高亮／下划线／笔记、只读工具 | 坐标锚点、附件身份、删除语义、笔记与原文区分 |
| [#75](changes/pr-75.md) | 选区旁写作动作 | 选区保持、Escape／外点关闭和结果采纳 |
| [#76](changes/pr-76.md) | 写作界面简化、中日韩长度、过滤表格装置 | 历史句库与重新检索的一致性，不能误删正文 |
| [#77](changes/pr-77.md)（开放） | 大纲侧栏与边栏批注 | 等合并后再确认最终接口；当前只记录 head |
| [#78](changes/pr-78.md)（开放） | 无样式 Word 标题猜测、确认与章节拆合 | 猜测必须经用户确认；拆合应保留批注、快照和原有 XML 样式 |

## 5. 承接 #29 的 fork 总构想

总体方向仍是“搜 → 导入与解析 → 读与标记 → 沉淀证据／笔记 → AI 辅助写作 → 引用核对 → 图与稿件交付”。现有上游已经覆盖多段链路。fork 应优先填补可验证的缺口，每次只处理一个行为，不把总构想变成一次性大合并。

| 概念编号 | 原 #29 构想 | 当前约束与后续讨论 |
|---|---|---|
| C01 解析与身份 | 更可靠 PDF 阅读、无 DOI 元数据 | 建 PDF 回归集，记录来源文件／页码／提取器／失败类型；元数据按候选与字段确认，未知年份保留未知。上游明确不计划 OCR／Doc2X 云解析；若 fork 重启可选提供方讨论，应另列范围，说明数据发送、费用与本地回退 |
| C02 证据与知识 | 单篇／文献库问答、笔记、原文核对 | 复用已有会话、笔记、引用及标记；定义笔记何时参与检索、如何禁用，区分原文、用户笔记和模型推断；论断与引文就近绑定 |
| C03 AI 写作与约束 | 草稿到正文、写作范式、AI 味防御 | 复用已存在的模型辅助和综述阶段；后续 Skill 规定语言、论据与引文邻接、未验证结论标记、禁止编造来源。目标是可审阅写作，不承诺规避 AI 检测或全自动投稿 |
| C04 Skill 衔接 | 用户自定义、会话转 Skill、兼容 wisp-science | 斜杠模板作为当前事实；单独设计 Skill 格式版本、解析、权限、上下文、执行与失败机制，核对 wisp-science #1679／PR #1706，而非宣称现已无缝兼容 |
| C05 有界 DOCX 编辑 | 结构化编辑、保留原始 OOXML、DOCX 交付 | 将 #72 作为可对照实现；先定义支持编辑、仅保留、拒绝／警告的元素矩阵；对公式、表格、域、脚注、修订等做真实样本往返验证，不承诺完全兼容 |
| C06 引用交付 | 引用格式、BibTeX／CSL 与外部 Skill | 先检查引用键和元数据，再导出；CSL 排版与 Word 动态域分别定义。不能用引文存在替代证据支持判断 |
| C07 图片与机制图 | CiteBox 图收藏、自定义绘图模型、可编辑 PPTX | 收藏已落地；新增机制图／流程图应独立卡片确认，选模型与 Skill，保存可编辑字体／形状和来源，询问图注及插入位置；生成图不能伪装成论文原始证据 |
| C08 GitHub-to-Paper | 仓库定位 → 方向 → 文献范式 → 创新点 → 真实结果 → 稿件 | 绑定 commit、数据与实验产物；先盘点已有证据，再补实验；结果优先写作必须基于真实结果。MCP 是连接协议，不保证计算资源或任务已运行；长任务保存 ID、状态与产物 |
| C09 可选工具扩展 | wisp-science 的寻找、下载与远端工具 | 提取清楚的工具边界，作为可选能力；不要求合并整个仓库，不在没有能力／授权时声称完成下载或实验 |

wisp-science 写作概念参考：[issue #1679](https://github.com/xuzhougeng/wisp-science/issues/1679)、[PR #1706](https://github.com/xuzhougeng/wisp-science/pull/1706)。它们在这里是设计参考，未做该仓库的逐行兼容性审计。

原 #29 中其他仓库的参考关系也继续保留：

| 仓库 | 可借鉴边界 | 本次结论 |
|---|---|---|
| [CiteBox](https://github.com/xuzhougeng/citebox) | 精读交互、图注与图笔记 | 上游此前决定借鉴交互，不合并应用／代码、不增加本地服务；Paper 自己的 #63/#74 已覆盖部分场景 |
| [wisp-science](https://github.com/xuzhougeng/wisp-science) | 分页读取／视觉阅读工作流、证据工作区、修订与 Skill 思路 | Paper 已选择纯 Rust 读取链路，不搬 Python 运行时；科研计算／实验管理仍通过清楚的可选工具边界讨论 |
| [Typorax](https://github.com/xuzhougeng/typorax) | 编辑器与渲染器分层、文件交接 | 原 #29 将其作为轻量编辑骨架参考，不据此推定完整学术排版能力 |
| [ScientificFigureLibrary](https://github.com/xuzhougeng/ScientificFigureLibrary) | 图形模板、绘图代码与版本化资产 | 与“阅读论文中的图”分开；可通过 MCP 连接，不预设已经支持新提出的 PPTX 生成向导 |

这些是历史设计参考关系，本次逐行审计对象只有 WispPaper；其他仓库若进入实现，需另行固定提交并验证接口、许可证、依赖和数据契约。

建议 fork 的实际推进顺序：先做源码和行为差异核对，再选择 PDF／证据准确性、写作交互或有界 DOCX 中一个最小任务；接着补引用交付与 Skill 约束；图生成和 GitHub-to-Paper 保持独立讨论。每个新 issue 都引用本总索引的 C 编号，并写明是上游已有能力复用、缺口修复还是 fork 扩展。

## 6. 源码追踪与更新方法

最小追踪单位为：**需求／issue → 开发者决定 → PR → 发布边界 → 固定提交的新增／删除行 → 验证证据 → 剩余范围**。

合并 PR 使用第一父提交到合并提交的差异，避免将尚未合并的分支或后续 main 内容混入。开放 PR 使用固定 main 与 head 的 merge-base；更新 head 后必须重新生成。新增行链接新提交，删除行链接父提交；重命名保留前后路径。二进制和仅文件模式变更明确无文本行号。

每个 PR 的 Markdown 索引列出全部 hunk 的源码范围；对应 `pr-N.json.gz` 保存**每一条新增／删除行**的行号、原文、前后提交和路径，能构造单行永久链接。生成器逐行比较 Git blob，发现不符即失败。历史实现的行号用于说明当时改动；判断当前能力时使用 `sources.md` 中 release／main 的入口，避免引用已被替换的实现。

本次只做文档／追踪工具变更，未运行应用构建，也未把维护者自述的测试当作本机复测。各 PR 的原始 `Checked`／`Left out` 保存在快照中。上游应用的 UI、平台和真实文件兼容性仍需在后续实现 PR 中验证。

更新时先另存 `gh issue list`／`gh pr list`／`gh release view` 的公开 JSON（字段与本快照一致），记录 UTC 时间、剥离 tag 的 release SHA、main SHA、fork SHA 和开放 PR head。获取历史时避免覆盖 fork 同名标签：

```powershell
git fetch --no-tags upstream main refs/tags/v0.6.0:refs/upstream-tags/v0.6.0
git fetch --no-tags upstream refs/pull/77/head:refs/remotes/upstream/pr-77 refs/pull/78/head:refs/remotes/upstream/pr-78
python -X utf8 -m unittest discover -s docs/upstream/tools -p test_build_index.py
python -X utf8 docs/upstream/tools/build_index.py
python -X utf8 docs/upstream/tools/build_index.py --check
```

新快照用 `--snapshot snapshots/YYYY-MM-DD` 选择；同一天多次更新应在目录名中加时间。生成器只读本地 Git 对象与快照，不访问网络，不修改应用数据。缺少旧提交时先 fetch 上游完整历史。提交 JSON 快照、生成索引与人工能力判断时应一起复核，避免只更新 PR 状态而遗留过时结论。

逐行数据可用 Python 标准库 `gzip.open(path, 'rt', encoding='utf-8')` 读取；使用文件的 `hunks[].added`／`deleted`，按 `after + new_path`／`before + old_path` 与行号定位。`validation.json` 统计的是各 PR 历史累计文本变更，不是当前有效代码行数。

未来状态采用“讨论中／已接受待实现／开放 PR／main 已合并／已发布／已验证／部分实现／被替代／不计划”分别记录。维护者关闭 issue、PR 合并、安装包发布和本机验证是四种不同证据，不能互相代替。
