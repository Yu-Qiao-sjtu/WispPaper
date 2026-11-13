# 固定版本的源码入口

以下定位通过固定提交中的精确字符串查找生成，并校验每个范围。`release` 是 v0.6.0；`main` 是本次发布后快照；`fork` 是我们自己的版本。局部入口用于解释行为，全部变更见 [PR 索引](changes/README.md)。

## pdf-read

**PDF 文本提取链路**（release，`60703bd12514`）

`crates/core/src/fulltext.rs` [L59–L101](https://github.com/xuzhougeng/WispPaper/blob/60703bd1251456dd78eb13dad100de764fe3611d/crates/core/src/fulltext.rs#L59-L101)

先用 hayro 解释字形与布局，再回退 pdf-extract；加密、编码、解析、无文本层等失败分开报告。浏览器可选中文字不等于本解析器一定提取正确，也不是已接入 Doc2X。

## pdf-title

**首页标题猜测**（release，`60703bd12514`）

`crates/core/src/pdftext.rs` [L396–L423](https://github.com/xuzhougeng/WispPaper/blob/60703bd1251456dd78eb13dad100de764fe3611d/crates/core/src/pdftext.rs#L396-L423)

这是标题候选启发式，不能据此声称已从首页可靠提取全部作者和发表年份。

## metadata-api

**无标识符记录按题目查候选**（release，`60703bd12514`）

`crates/core/src/complete.rs` [L65–L84](https://github.com/xuzhougeng/WispPaper/blob/60703bd1251456dd78eb13dad100de764fe3611d/crates/core/src/complete.rs#L65-L84)

Crossref、Europe PMC、arXiv API 生成候选；不是浏览器桥接。用户核对身份和字段后写回。

## metadata-write

**元数据人工接受与并发核对**（release，`60703bd12514`）

`src/app.rs` [L3836–L3880](https://github.com/xuzhougeng/WispPaper/blob/60703bd1251456dd78eb13dad100de764fe3611d/src/app.rs#L3836-L3880)

定位实际应用候选的入口；原始记录变化、标识冲突等安全条件见完整 #44 diff。

## metadata-year

**Crossref 年份的遗留限制**（release，`60703bd12514`）

`crates/core/src/lookup.rs` [L61–L105](https://github.com/xuzhougeng/WispPaper/blob/60703bd1251456dd78eb13dad100de764fe3611d/crates/core/src/lookup.rs#L61-L105)

旧通用解析仍允许缺少发表日期时回退记录创建时间；该值不能保证是论文发表年份。

## pdf-zoom

**PDF 缩放**（release，`60703bd12514`）

`src/pdf.rs` [L530–L552](https://github.com/xuzhougeng/WispPaper/blob/60703bd1251456dd78eb13dad100de764fe3611d/src/pdf.rs#L530-L552)

放大、缩小、指定比例和适应宽度等已经存在；不等价于已支持触控板双指缩放。

## pdf-locate-release

**原文引用定位（发布版）**（release，`60703bd12514`）

`src/pdf.rs` [L478–L515](https://github.com/xuzhougeng/WispPaper/blob/60703bd1251456dd78eb13dad100de764fe3611d/src/pdf.rs#L478-L515)

已有页码与引句定位；因此后续需求应描述交互和定位准确性增强，而非从零新增。

## paper-notes

**单篇问答转笔记**（release，`60703bd12514`）

`crates/core/src/ai.rs` [L480–L501](https://github.com/xuzhougeng/WispPaper/blob/60703bd1251456dd78eb13dad100de764fe3611d/crates/core/src/ai.rs#L480-L501)

已可把回答、问题、来源整理成笔记；不等价于每篇论文拥有可开关、分层检索的独立知识库。

## custom-command

**自定义斜杠命令**（release，`60703bd12514`）

`crates/core/src/ai.rs` [L1109–L1122](https://github.com/xuzhougeng/WispPaper/blob/60703bd1251456dd78eb13dad100de764fe3611d/crates/core/src/ai.rs#L1109-L1122)

模板展开为提示文本，内置命令优先；不是可执行 Skill、脚本运行环境或 wisp-science Skill 无缝兼容层。

## intent-queries

**意图检索生成查询组**（release，`60703bd12514`）

`crates/core/src/retrieval.rs` [L31–L39](https://github.com/xuzhougeng/WispPaper/blob/60703bd1251456dd78eb13dad100de764fe3611d/crates/core/src/retrieval.rs#L31-L39)

模型将用户段落／想表达的意思转成文献语言的 3–5 组术语、论述功能和同义表达；不是只做逐字翻译。

## intent-search

**写作检索入口**（release，`60703bd12514`）

`src/write_find.rs` [L71–L88](https://github.com/xuzhougeng/WispPaper/blob/60703bd1251456dd78eb13dad100de764fe3611d/src/write_find.rs#L71-L88)

由选区／当前段落触发按需检索；进一步的查询生成、排名与候选上下文见该文件及 #47。

## ai-write

**AI 辅助写作**（release，`60703bd12514`）

`src/write_assist.rs` [L70–L111](https://github.com/xuzhougeng/WispPaper/blob/60703bd1251456dd78eb13dad100de764fe3611d/src/write_assist.rs#L70-L111)

模型已参与改写、翻译、润色和自定义操作，接收文献候选例句并流式给出卡片预览。

## ai-accept

**AI 结果采纳**（release，`60703bd12514`）

`src/write_assist.rs` [L165–L205](https://github.com/xuzhougeng/WispPaper/blob/60703bd1251456dd78eb13dad100de764fe3611d/src/write_assist.rs#L165-L205)

替换／插入是明确操作；需核对原文范围与快照，不应解读为后台自动覆盖整份稿件。

## citation

**引用转换与定位信息**（release，`60703bd12514`）

`crates/core/src/cite.rs` [L53–L87](https://github.com/xuzhougeng/WispPaper/blob/60703bd1251456dd78eb13dad100de764fe3611d/crates/core/src/cite.rs#L53-L87)

将原生文献标签转成稳定稿件引用键并保留页码；引用语法正确不能证明论据支持结论。

## citation-export

**参考文献与 BibTeX**（release，`60703bd12514`）

`crates/core/src/cite.rs` [L439–L461](https://github.com/xuzhougeng/WispPaper/blob/60703bd1251456dd78eb13dad100de764fe3611d/crates/core/src/cite.rs#L439-L461)

已有参考文献文本／Markdown、BibTeX 输出函数。尚不能据此认定已有完整 CSL 期刊样式系统或 Word 动态域。

## review-evidence

**从引用派生证据记录**（release，`60703bd12514`）

`crates/core/src/review.rs` [L189–L236](https://github.com/xuzhougeng/WispPaper/blob/60703bd1251456dd78eb13dad100de764fe3611d/crates/core/src/review.rs#L189-L236)

v0.6.0 使用 #61 后的实现：提取相邻论断与引用并保存核对记录；不是 #59 的早期手工证据存储方案。

## review-validity

**核对记录与来源变化**（release，`60703bd12514`）

`crates/core/src/review.rs` [L408–L445](https://github.com/xuzhougeng/WispPaper/blob/60703bd1251456dd78eb13dad100de764fe3611d/crates/core/src/review.rs#L408-L445)

来源指纹及可用性参与核对状态；引句出现与是否支持论断分别处理，仍需人工判断。

## figure-library

**图片库持久化**（release，`60703bd12514`）

`crates/core/src/figure.rs` [L170–L214](https://github.com/xuzhougeng/WispPaper/blob/60703bd1251456dd78eb13dad100de764fe3611d/crates/core/src/figure.rs#L170-L214)

PDF 截图与元数据持久保存。图片库已经落地；截图收藏不等价于机制图生成或可编辑 PPTX。

## figure-caption

**截图关联图注**（release，`60703bd12514`）

`crates/core/src/figure.rs` [L418–L442](https://github.com/xuzhougeng/WispPaper/blob/60703bd1251456dd78eb13dad100de764fe3611d/crates/core/src/figure.rs#L418-L442)

按页面字形布局寻找图注；视觉模型结果仍须用户接受。复杂多栏、跨页图注仍需验证。

## find-pdf

**查找全文工具**（release，`60703bd12514`）

`crates/core/src/tools.rs` [L762–L805](https://github.com/xuzhougeng/WispPaper/blob/60703bd1251456dd78eb13dad100de764fe3611d/crates/core/src/tools.rs#L762-L805)

已有标准文献来源下的查找全文入口。工具可用不代表已授权任意网页下载或保证获取全部文献。

## main-framework

**综述范式确认**（main，`facc7b249d04`）

`crates/core/src/review.rs` [L160–L167](https://github.com/xuzhougeng/WispPaper/blob/facc7b249d04b14064b77eb4e167c7efc7d5732a/crates/core/src/review.rs#L160-L167)

发布后 #71 增加已有综述框架的确认阶段，并保持旧记录兼容；v0.6.0 安装包不含这一提交。

## main-docx-read

**DOCX 解析**（main，`facc7b249d04`）

`crates/core/src/docx.rs` [L1140–L1171](https://github.com/xuzhougeng/WispPaper/blob/facc7b249d04b14064b77eb4e167c7efc7d5732a/crates/core/src/docx.rs#L1140-L1171)

读取原始 OOXML 与段落／批注信息，编辑模型并非通用所见即所得 Word 引擎。

## main-docx-write

**DOCX 保真写回与修订**（main，`facc7b249d04`）

`crates/core/src/docx.rs` [L1311–L1394](https://github.com/xuzhougeng/WispPaper/blob/facc7b249d04b14064b77eb4e167c7efc7d5732a/crates/core/src/docx.rs#L1311-L1394)

对齐段落，保留未改 XML，改动处生成修订／批注。不能承诺全部公式、表格、图片和版式都可在编辑器中编辑或百分之百往返兼容。

## main-docx-export

**稿件实际导出路径**（main，`facc7b249d04`）

`src/manuscripts.rs` [L432–L466](https://github.com/xuzhougeng/WispPaper/blob/facc7b249d04b14064b77eb4e167c7efc7d5732a/src/manuscripts.rs#L432-L466)

正文 Markdown／DOCX 导出与引用列表函数是不同路径；不要因存在 cite::reference_list 就声称所有导出已自动附期刊格式参考文献。

## main-annotations

**PDF 高亮／下划线／笔记**（main，`facc7b249d04`）

`crates/core/src/annotation.rs` [L81–L107](https://github.com/xuzhougeng/WispPaper/blob/facc7b249d04b14064b77eb4e167c7efc7d5732a/crates/core/src/annotation.rs#L81-L107)

按论文、附件、页码、矩形与引文保存标记，源 PDF 不改写。

## main-annotation-tool

**助手读取 PDF 标记**（main，`facc7b249d04`）

`crates/core/src/tools.rs` [L589–L625](https://github.com/xuzhougeng/WispPaper/blob/facc7b249d04b14064b77eb4e167c7efc7d5732a/crates/core/src/tools.rs#L589-L625)

只读工具读取标记；用户笔记需与论文原文区分，不能当作原文证据。

## main-page-highlight

**仅页码引用与异步定位修复**（main，`facc7b249d04`）

`src/pdf.rs` [L561–L603](https://github.com/xuzhougeng/WispPaper/blob/facc7b249d04b14064b77eb4e167c7efc7d5732a/src/pdf.rs#L561-L603)

#73 区分页码边框与带引句的句子高亮，并阻止旧搜索结果覆盖新导航。

## main-fields

**自定义字段提取建议**（main，`facc7b249d04`）

`crates/core/src/tools.rs` [L1264–L1295](https://github.com/xuzhougeng/WispPaper/blob/facc7b249d04b14064b77eb4e167c7efc7d5732a/crates/core/src/tools.rs#L1264-L1295)

助手提出字段候选，由用户接受后写入；不能据此声称模型直接改变筛选决定。

## main-append

**助手回答写入指定稿件段落**（main，`facc7b249d04`）

`crates/core/src/manuscript.rs` [L794–L811](https://github.com/xuzhougeng/WispPaper/blob/facc7b249d04b14064b77eb4e167c7efc7d5732a/crates/core/src/manuscript.rs#L794-L811)

有持久化、引用键与快照边界；AI 写作能力已超出早期仅人工输入的定位。

## main-apparatus

**候选语料过滤表格装置**（main，`facc7b249d04`）

`crates/core/src/structure.rs` [L611–L648](https://github.com/xuzhougeng/WispPaper/blob/facc7b249d04b14064b77eb4e167c7efc7d5732a/crates/core/src/structure.rs#L611-L648)

#76 在入库／检索中过滤表格数字行及注释，提高语料质量；不是全新的 PDF 表格结构解析器。

## fork-writing

**我们 fork 的 AI 写作基线**（fork，`c7f84ea5ea52`）

`docs/AI-WRITING.md` [L1–L8](https://github.com/Yu-Qiao-sjtu/WispPaper/blob/c7f84ea5ea52fbea148f592cf5df4fb18044d319/docs/AI-WRITING.md#L1-L8)

fork v0.5.0 与上游同名版本不是同一个版本；既有草稿生成、意图搜索和候选翻译需单独评估，不能按版本号自动判定已同步。
