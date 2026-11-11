//! 文献库提问: what the model is given, the request for either protocol, and
//! reading the streamed answer. No HTTP here; the app sends the request.
//!
//! The library is put in front of the model first: every paper's record
//! (most relevant first) and the full-text pages that match the question,
//! each under a label like `[P3]` or `[P3 p.5]` the answer cites. What did
//! not fit is counted and shown, never dropped silently. Beyond that the
//! model has the library's tools (`tools`) and may search and read for
//! itself; a turn that ends in tool calls is answered with their results
//! and the conversation goes on.

use crate::fulltext::{score, terms};
use crate::model::{FieldDef, PUBLICATION_STATUSES, Paper, PaperFile, READINGS, SCREENINGS, label};
use crate::tools::ToolDef;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    /// The Claude Messages API.
    #[default]
    Anthropic,
    /// Any service speaking OpenAI's chat completions.
    OpenAI,
}

/// One model configuration from the settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Profile {
    pub name: String,
    pub protocol: Protocol,
    pub base_url: String,
    /// Empty means the ANTHROPIC_API_KEY / OPENAI_API_KEY environment variable.
    pub api_key: String,
    pub model: String,
}

pub const ANTHROPIC_BASE: &str = "https://api.anthropic.com";
pub const DEFAULT_MODEL: &str = "claude-opus-5-5";

impl Default for Profile {
    fn default() -> Self {
        Profile { name: "Claude".into(), protocol: Protocol::Anthropic, base_url: ANTHROPIC_BASE.into(), api_key: String::new(), model: DEFAULT_MODEL.into() }
    }
}

impl Profile {
    /// The key to send: the stored one, else the protocol's environment variable.
    pub fn key(&self) -> String {
        if !self.api_key.trim().is_empty() { return self.api_key.trim().to_string() }
        std::env::var(match self.protocol {
            Protocol::Anthropic => "ANTHROPIC_API_KEY",
            Protocol::OpenAI => "OPENAI_API_KEY",
        })
        .unwrap_or_default()
    }
}

/// What a label in the answer points at.
#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    /// `P3`
    pub label: String,
    pub paper_id: String,
    pub title: String,
}

/// What a conversation is about.
#[derive(Clone, Debug, PartialEq)]
pub enum Scope {
    /// The papers handed in (the whole library, or the current filter result).
    Library,
    /// One paper, read in full.
    Paper(String),
}

/// Everything the model is given for one conversation. Built once, with the
/// first question, and reused for the follow-ups so the prompt prefix stays
/// byte-identical and cacheable.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Context {
    pub instructions: String,
    pub documents: String,
    pub sources: Vec<Source>,
    /// Papers given with their full record.
    pub full: usize,
    /// Papers given by title only, for lack of room.
    pub titles_only: usize,
    /// Papers left out entirely.
    pub omitted: usize,
    /// Full-text pages included.
    pub pages: usize,
    /// Full-text pages of the paper in scope that did not fit.
    pub pages_omitted: usize,
}

impl Context {
    /// Give the papers outside the documents a label too: the tools reach the
    /// whole library, and what they find must be citable. The new labels
    /// continue after the highest one in use.
    pub fn add_sources(&mut self, papers: &[Paper]) {
        let mut next = self.sources.iter().filter_map(|s| s.label[1..].parse::<usize>().ok()).max().unwrap_or(0);
        for paper in papers {
            if self.sources.iter().any(|s| s.paper_id == paper.id) { continue }
            next += 1;
            self.sources.push(Source { label: format!("P{next}"), paper_id: paper.id.clone(), title: paper.title.clone() });
        }
    }
}

/// Room for paper records, for full-text pages, and for the bare title list, in bytes.
const RECORD_BUDGET: usize = 400_000;
const PAGE_BUDGET: usize = 240_000;
const TITLE_BUDGET: usize = 120_000;
/// Room for one paper read in full.
const PAPER_BUDGET: usize = 1_200_000;
const MAX_LIBRARY_PAGES: usize = 40;

fn record(label_text: &str, p: &Paper, fields: &[FieldDef]) -> String {
    let mut out = format!("[{label_text}] {}\n", p.title);
    let mut line = |name: &str, value: &str| {
        if !value.trim().is_empty() { out.push_str(&format!("{name}：{}\n", value.trim())) }
    };
    line("作者", &p.authors);
    line("期刊", &p.journal);
    line("年份", &p.year.map(|y| y.to_string()).unwrap_or_default());
    line("DOI", &p.doi);
    line("PMID", &p.pmid);
    if p.publication_status != "unknown" { line("发表状态", label(&PUBLICATION_STATUSES, &p.publication_status)) }
    line("标签", &p.tags.join("、"));
    for field in fields {
        if let Some(values) = p.fields.get(&field.id) { line(&field.name, &values.join("、")) }
    }
    line("阅读状态", label(&READINGS, &p.reading));
    line("收录判断", label(&SCREENINGS, &p.screening));
    line("判断理由", &p.screening_reason);
    line("摘要", &p.abstract_text);
    line("我的笔记", &p.notes);
    out
}

/// Assemble what the model reads. `papers` is the scope's list in display
/// order; `fulltext` holds the extracted pages by paper id.
pub fn build_context(library_name: &str, topic: &str, papers: &[&Paper], fields: &[FieldDef], fulltext: &BTreeMap<String, (PaperFile, Vec<String>)>, scope: &Scope, question: &str) -> Context {
    let mut ctx = Context::default();
    let words = terms(question);
    let labels: BTreeMap<&str, String> = papers.iter().enumerate().map(|(i, p)| (p.id.as_str(), format!("P{}", i + 1))).collect();
    for p in papers {
        ctx.sources.push(Source { label: labels[p.id.as_str()].clone(), paper_id: p.id.clone(), title: p.title.clone() });
    }
    let mut documents = String::new();

    match scope {
        Scope::Paper(id) => {
            if let Some(p) = papers.iter().find(|p| &p.id == id) {
                let tag = &labels[p.id.as_str()];
                documents.push_str("## 文献记录\n\n");
                documents.push_str(&record(tag, p, fields));
                ctx.full = 1;
                match fulltext.get(id) {
                    Some((_, pages)) => {
                        documents.push_str("\n## 全文（逐页）\n");
                        let mut used = 0;
                        for (i, page) in pages.iter().enumerate() {
                            if used + page.len() > PAPER_BUDGET {
                                ctx.pages_omitted = pages.len() - i;
                                break;
                            }
                            used += page.len();
                            documents.push_str(&format!("\n[{tag} p.{}]\n{}\n", i + 1, page.trim()));
                            ctx.pages += 1;
                        }
                    }
                    None => documents.push_str("\n（这篇文献没有可读取的全文，只有上面的记录。）\n"),
                }
            }
            ctx.sources.retain(|s| &s.paper_id == id);
        }
        Scope::Library => {
            // Most relevant first; without any match the list keeps its order.
            let mut order: Vec<(f32, usize)> = papers
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let hay = format!("{} {} {} {} {}", p.title, p.abstract_text, p.tags.join(" "), p.notes, p.fields.values().flatten().cloned().collect::<Vec<_>>().join(" ")).to_lowercase();
                    (score(&hay, &words), i)
                })
                .collect();
            order.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal).then(a.1.cmp(&b.1)));
            let mut full: Vec<usize> = vec![];
            let mut titles: Vec<usize> = vec![];
            let (mut used, mut used_titles) = (0, 0);
            for (_, i) in order {
                let text = record(&labels[papers[i].id.as_str()], papers[i], fields);
                if used + text.len() <= RECORD_BUDGET {
                    used += text.len();
                    full.push(i);
                } else if used_titles + papers[i].title.len() + 24 <= TITLE_BUDGET {
                    used_titles += papers[i].title.len() + 24;
                    titles.push(i);
                } else {
                    ctx.omitted += 1;
                }
            }
            ctx.full = full.len();
            ctx.titles_only = titles.len();
            // Listed in the library's own order, so labels read in sequence.
            full.sort();
            titles.sort();
            documents.push_str("## 文献记录\n\n");
            for i in full {
                documents.push_str(&record(&labels[papers[i].id.as_str()], papers[i], fields));
                documents.push('\n');
            }
            if !titles.is_empty() {
                documents.push_str("## 仅列出题目的文献（篇幅所限，未附摘要）\n\n");
                for i in titles {
                    let p = papers[i];
                    documents.push_str(&format!("[{}] {}{}\n", labels[p.id.as_str()], p.title, p.year.map(|y| format!("（{y}）")).unwrap_or_default()));
                }
                documents.push('\n');
            }
            // Full-text pages that match the question.
            let mut hits: Vec<(f32, &str, usize, &str)> = vec![];
            if !words.is_empty() {
                for p in papers {
                    let Some((_, pages)) = fulltext.get(&p.id) else { continue };
                    for (n, page) in pages.iter().enumerate() {
                        let s = score(&page.to_lowercase(), &words);
                        if s > 0. { hits.push((s, p.id.as_str(), n, page.as_str())) }
                    }
                }
            }
            hits.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
            let mut used = 0;
            let mut chosen: Vec<(&str, usize, &str)> = vec![];
            for (_, id, n, page) in hits.into_iter().take(MAX_LIBRARY_PAGES) {
                if used + page.len() > PAGE_BUDGET { continue }
                used += page.len();
                chosen.push((id, n, page));
            }
            chosen.sort_by(|a, b| labels[a.0].len().cmp(&labels[b.0].len()).then(labels[a.0].cmp(&labels[b.0])).then(a.1.cmp(&b.1)));
            if !chosen.is_empty() {
                documents.push_str("## 与问题相关的全文页面\n");
                for (id, n, page) in &chosen {
                    documents.push_str(&format!("\n[{} p.{}]\n{}\n", labels[id], n + 1, page.trim()));
                }
            }
            ctx.pages = chosen.len();
        }
    }

    let about = if topic.trim().is_empty() { String::new() } else { format!("这个文献库的主题：{}\n", topic.trim()) };
    let scope_line = match scope {
        Scope::Paper(_) => "用户正在阅读其中一篇文献，下面是它的记录和全文。".to_string(),
        Scope::Library => {
            let mut line = format!("下面是文献库中 {} 篇文献的记录", ctx.full);
            if ctx.titles_only > 0 { line.push_str(&format!("，另有 {} 篇只列出题目", ctx.titles_only)) }
            if ctx.omitted > 0 { line.push_str(&format!("，还有 {} 篇因篇幅未提供", ctx.omitted)) }
            if ctx.pages > 0 { line.push_str(&format!("，以及 {} 页与问题相关的全文", ctx.pages)) }
            line.push('。');
            line
        }
    };
    ctx.instructions = format!(
        "你是 Wisp Paper 里的文献助手，帮助用户理解和整理文献库「{library_name}」。\n{about}{scope_line}\n\n\
回答时遵循：\n\
- 依据提供的文献材料作答。材料里没有的内容，明确说明文献库中没有相关信息，不要凭印象补充文献细节；需要用到常识性背景时，说明它不出自文献库。\n\
- 引用文献时使用材料中的标签，例如 [P3]；引用全文某一页时写 [P3 p.5]。标签要紧跟在它支持的那句话后面。\n\
- 「我的笔记」是用户自己写的，可以引用，但要与论文本身的结论区分开。\n\
- 只列出题目的文献没有摘要，对它们的内容不要下结论。\n\
- 上面的材料不够时使用工具：search_fulltext 在已保存的全文里找短语或关键词，read_fulltext 按页读原文核对上下文，list_papers 和 get_paper 查文献记录。工具查的是整个文献库，不限于上面列出的文献。\n\
- search_online 可以在 PubMed、Europe PMC、Crossref、arXiv 里试检索，不写入文献库。保存或删除检索式、运行导入、撤销导入会改动文献库：只在用户明确要求时使用，并说明做了什么。
\n- 工具结果里每篇文献带有 label，就是引用它时用的标签；工具的 paperId 和 id 参数可以直接填标签（如 P3）。没有保存全文的文献只有记录和摘要，回答依据的只是摘要时要说明。\n\
- 用用户提问的语言回答，直接回答问题，不必复述材料。"
    );
    ctx.documents = documents;
    ctx
}

/// The labels an answer cites, in order of first appearance: (label, page).
pub fn citations(answer: &str) -> Vec<(String, Option<usize>)> {
    let mut found: Vec<(String, Option<usize>)> = vec![];
    let mut rest = answer;
    while let Some(open) = rest.find('[') {
        rest = &rest[open + 1..];
        let Some(close) = rest.find(']') else { break };
        // "[P3]", "[P3 p.5]", "[P3, p.5]", "[P1, P2]"
        for part in rest[..close].split([',', '，', ';', '、']) {
            let part = part.trim();
            let mut words = part.split_whitespace();
            let Some(first) = words.next() else { continue };
            let page = |word: &str| word.trim_start_matches("p.").trim_start_matches('p').parse::<usize>().ok();
            if first.starts_with('P') && first.len() > 1 && first[1..].bytes().all(|b| b.is_ascii_digit()) {
                let entry = (first.to_string(), words.next().and_then(page));
                if !found.contains(&entry) { found.push(entry) }
            } else if first.starts_with("p.") && words.next().is_none() && let (Some(page), Some(last)) = (page(first), found.last().cloned()) {
                // "[P3, p.5]": the page belongs to the label before the comma.
                if last.1.is_none() {
                    found.pop();
                    let entry = (last.0, Some(page));
                    if !found.contains(&entry) { found.push(entry) }
                }
            }
        }
        rest = &rest[close + 1..];
    }
    found
}

// ---- requests

#[derive(Clone, Debug, PartialEq)]
pub struct HttpRequest {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Value,
}

/// Models whose safety classifiers can decline a request, and for which the
/// API can then rerun it on another model inside the same call.
const FALLBACK_MODELS: [&str; 5] = ["claude-fable-5-2", "claude-fable-5-1", "claude-opus-5-5", "claude-opus-5", "claude-sonnet-5-5"];

/// A question as it goes on the wire; the same for both protocols.
pub fn user_message(text: &str) -> Value {
    json!({"role": "user", "content": text})
}

/// An answer already given, when a conversation is replayed to another protocol.
pub fn assistant_message(text: &str) -> Value {
    json!({"role": "assistant", "content": text})
}

/// The streaming request for one step of a conversation. `wire` is the
/// conversation so far in the profile's protocol: questions, the model's
/// turns exactly as it sent them, and tool results. It only ever grows, so
/// the prefix stays cacheable and the model's thinking blocks stay valid.
pub fn chat_request(profile: &Profile, ctx: &Context, wire: &[Value], tools: &[ToolDef]) -> Result<HttpRequest, String> {
    let key = profile.key();
    if key.is_empty() { return Err("还没有配置 API Key。请在「设置 › 模型」里填写，或设置对应的环境变量。".into()) }
    if profile.model.trim().is_empty() { return Err("请在「设置 › 模型」里填写模型名称。".into()) }
    let base = profile.base_url.trim().trim_end_matches('/');
    if !base.starts_with("https://") && !base.starts_with("http://") { return Err("模型服务地址应以 https:// 或 http:// 开头。".into()) }
    match profile.protocol {
        Protocol::Anthropic => {
            let mut headers = vec![("x-api-key".to_string(), key), ("anthropic-version".to_string(), "2026-10-21".to_string()), ("content-type".to_string(), "application/json".to_string())];
            let mut body = json!({
                "model": profile.model.trim(),
                // Streaming, so a long answer has room; thinking stays at the model's default (adaptive).
                "max_tokens": 64000,
                "stream": true,
                // The documents come last in the system prompt with a cache breakpoint: follow-up
                // questions in the same conversation read them from the cache.
                "system": [
                    {"type": "text", "text": ctx.instructions},
                    {"type": "text", "text": ctx.documents, "cache_control": {"type": "ephemeral"}},
                ],
                "messages": wire,
            });
            if !tools.is_empty() {
                body["tools"] = tools.iter().map(|t| json!({"name": t.name, "description": t.description, "input_schema": t.input_schema})).collect();
            }
            // Only on Anthropic's own API: the other platforms and proxies reject these parameters.
            if base == ANTHROPIC_BASE {
                // The steps of a tool loop resend the conversation; cache it up to the last block.
                body["cache_control"] = json!({"type": "ephemeral"});
                if FALLBACK_MODELS.contains(&profile.model.trim()) {
                    headers.push(("anthropic-beta".to_string(), "server-side-fallback-2026-07-01".to_string()));
                    body["fallbacks"] = json!("default");
                }
            }
            Ok(HttpRequest { url: format!("{base}/v1/messages"), headers, body })
        }
        Protocol::OpenAI => {
            let mut all = vec![json!({"role": "system", "content": format!("{}\n\n{}", ctx.instructions, ctx.documents)})];
            all.extend(wire.iter().cloned());
            let mut body = json!({"model": profile.model.trim(), "stream": true, "messages": all});
            if !tools.is_empty() {
                body["tools"] = tools.iter().map(|t| json!({"type": "function", "function": {"name": t.name, "description": t.description, "parameters": t.input_schema}})).collect();
            }
            Ok(HttpRequest { url: format!("{base}/chat/completions"), headers: vec![("authorization".to_string(), format!("Bearer {key}")), ("content-type".to_string(), "application/json".to_string())], body })
        }
    }
}

// ---- the streamed turn

/// A tool the model asked for.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    /// The arguments; `Err` when what the model sent is not a JSON object.
    pub args: Result<Value, String>,
}

/// One turn of the model, read to its end.
#[derive(Clone, Debug, PartialEq)]
pub struct Turn {
    /// The turn as it goes back on the wire; `Null` when the model sent nothing.
    pub message: Value,
    pub calls: Vec<ToolCall>,
    /// The protocol's stop reason; "end" when the stream gave none.
    pub stop: String,
}

impl Turn {
    /// Whether the calls may run: not when the turn was cut off or declined,
    /// since a call's arguments may then be half written.
    pub fn runnable(&self) -> bool {
        !matches!(self.stop.as_str(), "max_tokens" | "length" | "refusal" | "content_filter" | "model_context_window_exceeded")
    }
}

/// What the app hears from a request in flight.
#[derive(Clone, Debug, PartialEq)]
pub enum ChatEvent {
    /// More of the answer's text.
    Text(String),
    Done(Turn),
    Error(String),
}

fn push_text(value: &mut Value, more: &str) {
    match value {
        Value::String(text) => text.push_str(more),
        other => *other = json!(more),
    }
}

/// Reads the server-sent events of one turn. Both protocols put a complete
/// JSON object on every `data:` line.
pub struct TurnReader {
    protocol: Protocol,
    /// Anthropic: the content blocks as they arrive, by index, with the
    /// pieces of each tool call's JSON beside them.
    blocks: Vec<Value>,
    inputs: Vec<String>,
    /// OpenAI: the text, and (id, name, arguments) of each tool call.
    text: String,
    calls: Vec<(String, String, String)>,
    stop: String,
}

impl TurnReader {
    pub fn new(protocol: Protocol) -> TurnReader {
        TurnReader { protocol, blocks: vec![], inputs: vec![], text: String::new(), calls: vec![], stop: String::new() }
    }

    /// One line of the stream: the answer text it adds, if any. `Err` is an
    /// error the service reported inside the stream.
    pub fn line(&mut self, line: &str) -> Result<Option<String>, String> {
        let Some(data) = line.strip_prefix("data:").map(str::trim) else { return Ok(None) };
        if data.is_empty() || data == "[DONE]" { return Ok(None) }
        let Ok(value) = serde_json::from_str::<Value>(data) else { return Ok(None) };
        if let Some(message) = value["error"]["message"].as_str() { return Err(message.to_string()) }
        match self.protocol {
            Protocol::Anthropic => {
                let index = value["index"].as_u64().unwrap_or(0) as usize;
                match value["type"].as_str() {
                    Some("content_block_start") => {
                        while self.blocks.len() <= index {
                            self.blocks.push(Value::Null);
                            self.inputs.push(String::new());
                        }
                        self.blocks[index] = value["content_block"].clone();
                    }
                    Some("content_block_delta") if index < self.blocks.len() => {
                        let delta = &value["delta"];
                        match delta["type"].as_str() {
                            Some("text_delta") => {
                                let text = delta["text"].as_str().unwrap_or("");
                                push_text(&mut self.blocks[index]["text"], text);
                                if !text.is_empty() { return Ok(Some(text.to_string())) }
                            }
                            Some("input_json_delta") => self.inputs[index].push_str(delta["partial_json"].as_str().unwrap_or("")),
                            // Thinking goes back to the model unchanged with the next step, signature included.
                            Some("thinking_delta") => push_text(&mut self.blocks[index]["thinking"], delta["thinking"].as_str().unwrap_or("")),
                            Some("signature_delta") => self.blocks[index]["signature"] = delta["signature"].clone(),
                            _ => {}
                        }
                    }
                    Some("message_delta") => {
                        if let Some(reason) = value["delta"]["stop_reason"].as_str() { self.stop = reason.to_string() }
                    }
                    _ => {}
                }
            }
            Protocol::OpenAI => {
                let choice = &value["choices"][0];
                if let Some(reason) = choice["finish_reason"].as_str() { self.stop = reason.to_string() }
                for call in choice["delta"]["tool_calls"].as_array().into_iter().flatten() {
                    let index = call["index"].as_u64().unwrap_or(self.calls.len().saturating_sub(1) as u64) as usize;
                    while self.calls.len() <= index { self.calls.push(Default::default()) }
                    let (id, name, arguments) = &mut self.calls[index];
                    if let Some(value) = call["id"].as_str() && !value.is_empty() { *id = value.to_string() }
                    name.push_str(call["function"]["name"].as_str().unwrap_or(""));
                    arguments.push_str(call["function"]["arguments"].as_str().unwrap_or(""));
                }
                if let Some(text) = choice["delta"]["content"].as_str() && !text.is_empty() {
                    self.text.push_str(text);
                    return Ok(Some(text.to_string()));
                }
            }
        }
        Ok(None)
    }

    /// The turn, once the stream has ended.
    pub fn finish(self) -> Turn {
        let parse = |raw: &str| -> Result<Value, String> {
            if raw.trim().is_empty() { return Ok(json!({})) }
            match serde_json::from_str::<Value>(raw) {
                Ok(value) if value.is_object() => Ok(value),
                Ok(_) => Err("工具参数应为一个 JSON 对象。".into()),
                Err(err) => Err(format!("工具参数不是有效的 JSON：{err}")),
            }
        };
        let stop = if self.stop.is_empty() { "end".to_string() } else { self.stop };
        let mut calls = vec![];
        match self.protocol {
            Protocol::Anthropic => {
                // When another model took over mid-answer, what the first one thought and
                // called before the `fallback` block is not sent back, and its calls do not run.
                let boundary = self.blocks.iter().rposition(|b| b["type"] == "fallback").unwrap_or(0);
                let mut kept = vec![];
                for (index, mut block) in self.blocks.into_iter().enumerate() {
                    let kind = block["type"].as_str().unwrap_or("").to_string();
                    if index < boundary && matches!(kind.as_str(), "thinking" | "redacted_thinking" | "tool_use") { continue }
                    match kind.as_str() {
                        "tool_use" => {
                            let args = parse(&self.inputs[index]);
                            block["input"] = args.clone().unwrap_or_else(|_| json!({}));
                            calls.push(ToolCall { id: block["id"].as_str().unwrap_or("").to_string(), name: block["name"].as_str().unwrap_or("").to_string(), args });
                        }
                        // The API rejects an empty text block when it is sent back.
                        "text" if block["text"].as_str().unwrap_or("").is_empty() => continue,
                        "" => continue,
                        _ => {}
                    }
                    kept.push(block);
                }
                let message = if kept.is_empty() { Value::Null } else { json!({"role": "assistant", "content": kept}) };
                Turn { message, calls, stop }
            }
            Protocol::OpenAI => {
                let mut wire_calls = vec![];
                for (n, (id, name, arguments)) in self.calls.into_iter().enumerate() {
                    if name.is_empty() { continue }
                    // Some compatible servers leave the id out.
                    let id = if id.is_empty() { format!("call_{n}") } else { id };
                    let args = parse(&arguments);
                    wire_calls.push(json!({"id": id, "type": "function", "function": {"name": name, "arguments": if args.is_ok() && !arguments.trim().is_empty() { arguments } else { "{}".to_string() }}}));
                    calls.push(ToolCall { id, name, args });
                }
                let message = if self.text.is_empty() && wire_calls.is_empty() {
                    Value::Null
                } else {
                    let mut message = json!({"role": "assistant", "content": if self.text.is_empty() { Value::Null } else { json!(self.text) }});
                    if !wire_calls.is_empty() { message["tool_calls"] = json!(wire_calls) }
                    message
                };
                Turn { message, calls, stop }
            }
        }
    }
}

/// The messages that answer a turn's tool calls: (call id, output, failed).
/// Anthropic takes every result of a turn in one user message.
pub fn tool_results(protocol: Protocol, results: &[(String, String, bool)]) -> Vec<Value> {
    match protocol {
        Protocol::Anthropic => vec![json!({"role": "user", "content": results.iter().map(|(id, output, failed)| json!({"type": "tool_result", "tool_use_id": id, "content": output, "is_error": failed})).collect::<Vec<_>>()})],
        Protocol::OpenAI => results.iter().map(|(id, output, _)| json!({"role": "tool", "tool_call_id": id, "content": output})).collect(),
    }
}

// ---- labels in tool calls

/// The paper a label or an id names: `P3` (any case) or the id itself.
fn source_of<'a>(sources: &'a [Source], value: &str) -> Option<&'a Source> {
    sources.iter().find(|s| s.paper_id == value || s.label.eq_ignore_ascii_case(value.trim()))
}

/// Let the model name a paper by its label: `paperId` and `id` arguments
/// holding one are replaced by the paper's id.
pub fn resolve_labels(args: &mut Value, sources: &[Source]) {
    for key in ["paperId", "id"] {
        if let Some(found) = args[key].as_str().and_then(|value| source_of(sources, value)) { args[key] = json!(found.paper_id) }
    }
}

/// Put the label beside every paper id in a tool result, so the answer can cite it.
pub fn add_labels(value: &mut Value, sources: &[Source]) {
    match value {
        Value::Array(items) => items.iter_mut().for_each(|item| add_labels(item, sources)),
        Value::Object(map) => {
            let label = ["paperId", "id"].iter().find_map(|key| map.get(*key).and_then(Value::as_str).and_then(|id| sources.iter().find(|s| s.paper_id == id))).map(|s| s.label.clone());
            if let Some(label) = label { map.insert("label".into(), json!(label)); }
            map.values_mut().for_each(|item| add_labels(item, sources));
        }
        _ => {}
    }
}

// ---- slash commands

/// `/name`, what it does, and the request it stands for (`{}` is what follows the name).
pub const SLASH: [(&str, &str, &str); 2] = [
    (
        "search",
        "在全文和记录里检索",
        "在文献库里检索：{}\n\n先用 search_fulltext 查已保存的全文，再用 list_papers 查题目、摘要、标签和笔记。列出命中的文献；全文命中的给出页码和原句。有文献因为没有保存全文而只查了记录时，说明这一点。",
    ),
    (
        "cite",
        "为一段文字逐句配引用",
        "为下面这段文字逐句找出文献库中可以引用的文献：\n\n{}\n\n先调用 find_citations 取得每句的候选，再用 read_fulltext 或文献记录核对候选是否真的支持这句话。逐句给出结果：可以引用的文献（标签和页码）和它依据的原句；没有合适文献的句子直接说明没有，不要勉强配引用。",
    ),
];

/// What a typed message asks for: itself, or the request behind a slash command.
pub fn expand_slash(input: &str) -> Result<String, String> {
    let input = input.trim();
    let Some(rest) = input.strip_prefix('/') else { return Ok(input.to_string()) };
    let (name, argument) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
    let Some((_, about, template)) = SLASH.iter().find(|(command, _, _)| *command == name) else {
        return Err(format!("没有 /{name} 这个命令。可用：{}", SLASH.iter().map(|(command, about, _)| format!("/{command}（{about}）")).collect::<Vec<_>>().join("、")));
    };
    if argument.trim().is_empty() { return Err(format!("/{name} 后面写上内容：{about}。")) }
    Ok(template.replace("{}", argument.trim()))
}

/// What to tell the reader when a turn did not end normally.
pub fn stop_note(reason: &str) -> Option<&'static str> {
    match reason {
        "refusal" | "content_filter" => Some("模型拒绝回答这个问题。"),
        "max_tokens" | "length" => Some("回答达到长度上限，被截断了。"),
        "model_context_window_exceeded" => Some("材料超出了模型的上下文长度，回答可能不完整。请缩小提问范围。"),
        _ => None,
    }
}

/// The message inside an error response body; the body itself when it is not JSON.
pub fn error_message(status: u16, body: &str) -> String {
    let detail = serde_json::from_str::<Value>(body).ok().and_then(|v| v["error"]["message"].as_str().or(v["message"].as_str()).map(String::from)).unwrap_or_else(|| body.chars().take(300).collect());
    let hint = match status {
        401 | 403 => "API Key 无效或没有权限",
        404 => "地址或模型名称不存在",
        429 => "请求过于频繁或额度不足",
        500..=599 => "模型服务暂时不可用",
        _ => "请求被拒绝",
    };
    format!("{hint}（HTTP {status}）：{}", detail.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paper(id: &str, title: &str, abstract_text: &str) -> Paper {
        let mut p = Paper { id: id.into(), title: title.into(), abstract_text: abstract_text.into(), ..Default::default() };
        p.normalize();
        p
    }

    fn fulltext(id: &str, pages: &[&str]) -> BTreeMap<String, (PaperFile, Vec<String>)> {
        [(id.to_string(), (PaperFile::default(), pages.iter().map(|p| p.to_string()).collect()))].into()
    }

    #[test]
    fn library_context() {
        let a = paper("a", "Root stem cell niche", "WOX5 maintains the quiescent centre.");
        let b = paper("b", "Leaf development", "Unrelated.");
        let fields = [FieldDef { id: "F1".into(), name: "物种".into(), ..Default::default() }];
        let mut a2 = a.clone();
        a2.fields.insert("F1".into(), vec!["Arabidopsis".into()]);
        let text = fulltext("a", &["Introduction about leaves.", "WOX5 expression in the root tip."]);
        let ctx = build_context("根", "根尖发育", &[&a2, &b], &fields, &text, &Scope::Library, "What does WOX5 do in the root?");
        assert_eq!((ctx.full, ctx.titles_only, ctx.omitted, ctx.pages), (2, 0, 0, 1));
        assert!(ctx.instructions.contains("「根」") && ctx.instructions.contains("根尖发育") && ctx.instructions.contains("2 篇"));
        assert!(ctx.documents.contains("[P1] Root stem cell niche\n"));
        assert!(ctx.documents.contains("物种：Arabidopsis"));
        assert!(ctx.documents.contains("[P1 p.2]\nWOX5 expression"));
        assert!(!ctx.documents.contains("Introduction about leaves"));
        assert_eq!(ctx.sources[1], Source { label: "P2".into(), paper_id: "b".into(), title: "Leaf development".into() });
    }

    #[test]
    fn paper_context_reads_everything() {
        let a = paper("a", "Root", "Abs");
        let b = paper("b", "Leaf", "Abs");
        let text = fulltext("b", &["one", "two"]);
        let ctx = build_context("L", "", &[&a, &b], &[], &text, &Scope::Paper("b".into()), "summarise");
        assert_eq!((ctx.full, ctx.pages, ctx.pages_omitted, ctx.sources.len()), (1, 2, 0, 1));
        assert!(ctx.documents.contains("[P2 p.1]\none") && ctx.documents.contains("[P2 p.2]\ntwo"));
        assert!(!ctx.documents.contains("Root"));
        let none = build_context("L", "", &[&a], &[], &BTreeMap::new(), &Scope::Paper("a".into()), "q");
        assert!(none.documents.contains("没有可读取的全文"));
    }

    #[test]
    fn budget_is_counted_not_hidden() {
        let big = "x".repeat(150_000);
        let papers: Vec<Paper> = (0..4).map(|i| paper(&format!("p{i}"), &format!("Paper {i}"), &big)).collect();
        let refs: Vec<&Paper> = papers.iter().collect();
        let ctx = build_context("L", "", &refs, &[], &BTreeMap::new(), &Scope::Library, "anything");
        assert_eq!((ctx.full, ctx.titles_only), (2, 2));
        assert!(ctx.documents.contains("仅列出题目"));
        assert!(ctx.instructions.contains("另有 2 篇只列出题目"));
    }

    #[test]
    fn cited_labels() {
        assert_eq!(
            citations("根尖 [P3] 与 [P1 p.5]；另见 [P2, p.7] 和 [P4、P3]，[1] 不是标签。"),
            [("P3".to_string(), None), ("P1".to_string(), Some(5)), ("P2".to_string(), Some(7)), ("P4".to_string(), None)]
        );
    }

    #[test]
    fn requests() {
        let ctx = Context { instructions: "I".into(), documents: "D".into(), ..Default::default() };
        let wire = [user_message("hi")];
        let tools = crate::tools::library_catalog();
        let claude = Profile { api_key: "k".into(), ..Default::default() };
        let r = chat_request(&claude, &ctx, &wire, &tools).unwrap();
        assert_eq!(r.url, "https://api.anthropic.com/v1/messages");
        assert_eq!(r.body["model"], "claude-opus-5-5");
        assert_eq!(r.body["fallbacks"], "default");
        assert_eq!(r.body["system"][1]["cache_control"]["type"], "ephemeral");
        assert_eq!(r.body["cache_control"]["type"], "ephemeral");
        assert_eq!(r.body["messages"], json!([{"role": "user", "content": "hi"}]));
        assert!(r.body.get("thinking").is_none() && r.body.get("temperature").is_none() && r.body.get("tool_choice").is_none());
        assert!(r.headers.iter().any(|(k, v)| k == "anthropic-beta" && v == "server-side-fallback-2026-07-01"));
        let tool = r.body["tools"].as_array().unwrap().iter().find(|t| t["name"] == "search_fulltext").unwrap();
        assert!(tool["input_schema"]["properties"]["query"].is_object() && tool["input_schema"]["properties"].get("library").is_none());
        // A proxy does not get the parameters only Anthropic's own API takes.
        let proxied = chat_request(&Profile { base_url: "https://proxy.example/".into(), ..claude.clone() }, &ctx, &wire, &tools).unwrap();
        assert!(proxied.body.get("fallbacks").is_none() && proxied.body.get("cache_control").is_none());
        assert_eq!(proxied.url, "https://proxy.example/v1/messages");
        assert!(chat_request(&claude, &ctx, &wire, &[]).unwrap().body.get("tools").is_none());

        let other = Profile { protocol: Protocol::OpenAI, base_url: "http://localhost:11434/v1".into(), api_key: "k".into(), model: "qwen".into(), ..Default::default() };
        let r = chat_request(&other, &ctx, &wire, &tools).unwrap();
        assert_eq!(r.url, "http://localhost:11434/v1/chat/completions");
        assert_eq!(r.body["messages"][0]["content"], "I\n\nD");
        assert_eq!(r.body["messages"][1]["role"], "user");
        assert_eq!((r.body["tools"][0]["type"].as_str(), r.body["tools"][0]["function"]["parameters"]["type"].as_str()), (Some("function"), Some("object")));
        assert!(chat_request(&Profile { base_url: "ftp://x".into(), api_key: "k".into(), ..Default::default() }, &ctx, &wire, &tools).is_err());
    }

    fn read(protocol: Protocol, lines: &[&str]) -> (String, Turn) {
        let mut reader = TurnReader::new(protocol);
        let mut text = String::new();
        for line in lines {
            if let Some(more) = reader.line(line).unwrap() { text.push_str(&more) }
        }
        (text, reader.finish())
    }

    #[test]
    fn anthropic_turn_with_tools() {
        let (text, turn) = read(
            Protocol::Anthropic,
            &[
                "event: content_block_start",
                r#"data: {"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":"","signature":""}}"#,
                r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"hm"}}"#,
                r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"sig=="}}"#,
                r#"data: {"type":"content_block_start","index":1,"content_block":{"type":"text","text":""}}"#,
                r#"data: {"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"先查"}}"#,
                r#"data: {"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"全文。"}}"#,
                r#"data: {"type":"content_block_start","index":2,"content_block":{"type":"tool_use","id":"toolu_1","name":"search_fulltext","input":{}}}"#,
                r#"data: {"type":"content_block_delta","index":2,"delta":{"type":"input_json_delta","partial_json":"{\"query\": \"chro"}}"#,
                r#"data: {"type":"content_block_delta","index":2,"delta":{"type":"input_json_delta","partial_json":"mium\"}"}}"#,
                r#"data: {"type":"content_block_start","index":3,"content_block":{"type":"tool_use","id":"toolu_2","name":"library_status","input":{}}}"#,
                r#"data: {"type":"content_block_stop","index":3}"#,
                r#"data: {"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":30}}"#,
            ],
        );
        assert_eq!(text, "先查全文。");
        assert_eq!(turn.stop, "tool_use");
        assert!(turn.runnable());
        assert_eq!(turn.calls, [ToolCall { id: "toolu_1".into(), name: "search_fulltext".into(), args: Ok(json!({"query": "chromium"})) }, ToolCall { id: "toolu_2".into(), name: "library_status".into(), args: Ok(json!({})) }]);
        // The turn goes back as it came: thinking with its signature, text, and both calls with their input.
        assert_eq!(
            turn.message,
            json!({"role": "assistant", "content": [
                {"type": "thinking", "thinking": "hm", "signature": "sig=="},
                {"type": "text", "text": "先查全文。"},
                {"type": "tool_use", "id": "toolu_1", "name": "search_fulltext", "input": {"query": "chromium"}},
                {"type": "tool_use", "id": "toolu_2", "name": "library_status", "input": {}},
            ]})
        );
        assert_eq!(
            tool_results(Protocol::Anthropic, &[("toolu_1".into(), "{}".into(), false), ("toolu_2".into(), "坏了".into(), true)]),
            [json!({"role": "user", "content": [{"type": "tool_result", "tool_use_id": "toolu_1", "content": "{}", "is_error": false}, {"type": "tool_result", "tool_use_id": "toolu_2", "content": "坏了", "is_error": true}]})]
        );
    }

    #[test]
    fn anthropic_turn_edges() {
        // A plain answer; an empty text block is not sent back.
        let (text, turn) = read(
            Protocol::Anthropic,
            &[
                r#"data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#,
                r#"data: {"type":"content_block_start","index":1,"content_block":{"type":"text","text":""}}"#,
                r#"data: {"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"Hello [P1]"}}"#,
                r#"data: {"type":"message_delta","delta":{"stop_reason":"end_turn"}}"#,
            ],
        );
        assert_eq!((text.as_str(), turn.calls.len(), turn.stop.as_str()), ("Hello [P1]", 0, "end_turn"));
        assert_eq!(turn.message["content"], json!([{"type": "text", "text": "Hello [P1]"}]));

        // Cut off inside a call: the half-written arguments are an error, and nothing may run.
        let (_, cut) = read(
            Protocol::Anthropic,
            &[
                r#"data: {"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"t","name":"get_paper","input":{}}}"#,
                r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"id\": \"P"}}"#,
                r#"data: {"type":"message_delta","delta":{"stop_reason":"max_tokens"}}"#,
            ],
        );
        assert!(!cut.runnable() && cut.calls[0].args.is_err());
        assert_eq!(cut.message["content"][0]["input"], json!({}));

        // Another model took over: thinking and calls before the fallback block are dropped, the block stays.
        let (_, fell) = read(
            Protocol::Anthropic,
            &[
                r#"data: {"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":"a","signature":"s"}}"#,
                r#"data: {"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"old","name":"get_paper","input":{}}}"#,
                r#"data: {"type":"content_block_start","index":2,"content_block":{"type":"fallback","model":"claude-opus-4-8"}}"#,
                r#"data: {"type":"content_block_start","index":3,"content_block":{"type":"tool_use","id":"new","name":"library_status","input":{}}}"#,
                r#"data: {"type":"message_delta","delta":{"stop_reason":"tool_use"}}"#,
            ],
        );
        assert_eq!(fell.calls.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), ["new"]);
        assert_eq!(fell.message["content"].as_array().unwrap().iter().map(|b| b["type"].as_str().unwrap()).collect::<Vec<_>>(), ["fallback", "tool_use"]);

        // Nothing at all, and an error inside the stream.
        let (_, nothing) = read(Protocol::Anthropic, &["event: ping", "data: {\"type\":\"ping\"}"]);
        assert_eq!((nothing.message.clone(), nothing.stop.as_str()), (Value::Null, "end"));
        assert_eq!(TurnReader::new(Protocol::Anthropic).line(r#"data: {"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#), Err("Overloaded".into()));
    }

    #[test]
    fn openai_turn_with_tools() {
        let (text, turn) = read(
            Protocol::OpenAI,
            &[
                r#"data: {"choices":[{"delta":{"role":"assistant","content":"查一下"},"finish_reason":null}]}"#,
                r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_a","type":"function","function":{"name":"read_fulltext","arguments":""}}]},"finish_reason":null}]}"#,
                r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"paperId\":"}}]},"finish_reason":null}]}"#,
                r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"P2\"}"}},{"index":1,"function":{"name":"library_status","arguments":"{}"}}]},"finish_reason":null}]}"#,
                r#"data: {"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#,
                "data: [DONE]",
            ],
        );
        assert_eq!((text.as_str(), turn.stop.as_str()), ("查一下", "tool_calls"));
        // The second call came without an id.
        assert_eq!(turn.calls, [ToolCall { id: "call_a".into(), name: "read_fulltext".into(), args: Ok(json!({"paperId": "P2"})) }, ToolCall { id: "call_1".into(), name: "library_status".into(), args: Ok(json!({})) }]);
        assert_eq!(turn.message["content"], "查一下");
        assert_eq!(turn.message["tool_calls"][0], json!({"id": "call_a", "type": "function", "function": {"name": "read_fulltext", "arguments": "{\"paperId\":\"P2\"}"}}));
        assert_eq!(tool_results(Protocol::OpenAI, &[("call_a".into(), "{}".into(), false), ("call_1".into(), "x".into(), true)]), [json!({"role": "tool", "tool_call_id": "call_a", "content": "{}"}), json!({"role": "tool", "tool_call_id": "call_1", "content": "x"})]);

        let (text, plain) = read(Protocol::OpenAI, &[r#"data: {"choices":[{"delta":{"content":"Hi"},"finish_reason":null}]}"#, r#"data: {"choices":[{"delta":{},"finish_reason":"stop"}]}"#, "data: [DONE]"]);
        assert_eq!((text.as_str(), plain.stop.as_str(), plain.message), ("Hi", "stop", json!({"role": "assistant", "content": "Hi"})));
        let (_, bad) = read(Protocol::OpenAI, &[r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c","function":{"name":"get_paper","arguments":"[1]"}}]},"finish_reason":"length"}]}"#]);
        assert!(bad.calls[0].args.is_err() && !bad.runnable());
        assert_eq!(bad.message["tool_calls"][0]["function"]["arguments"], "{}");
        assert!(stop_note("refusal").is_some() && stop_note("end_turn").is_none() && stop_note("tool_use").is_none());
        assert_eq!(error_message(401, r#"{"error":{"message":"bad key"}}"#), "API Key 无效或没有权限（HTTP 401）：bad key");
    }

    #[test]
    fn labels_for_tools() {
        let a = paper("a", "Root", "");
        let b = paper("b", "Leaf", "");
        let c = paper("c", "Stem", "");
        // The list in view is a and b; the library also holds c.
        let mut ctx = build_context("L", "", &[&a, &b], &[], &BTreeMap::new(), &Scope::Library, "q");
        ctx.add_sources(&[a.clone(), c.clone()]);
        assert_eq!(ctx.sources.iter().map(|s| (s.label.as_str(), s.paper_id.as_str())).collect::<Vec<_>>(), [("P1", "a"), ("P2", "b"), ("P3", "c")]);
        // One paper keeps the label it has in the whole list; the others follow the highest.
        let mut one = build_context("L", "", &[&a, &b], &[], &BTreeMap::new(), &Scope::Paper("b".into()), "q");
        one.add_sources(&[a.clone(), b.clone(), c.clone()]);
        assert_eq!(one.sources.iter().map(|s| s.label.as_str()).collect::<Vec<_>>(), ["P2", "P3", "P4"]);

        let mut args = json!({"paperId": "p3", "page": 2});
        resolve_labels(&mut args, &ctx.sources);
        assert_eq!(args, json!({"paperId": "c", "page": 2}));
        let mut unknown = json!({"id": "P9"});
        resolve_labels(&mut unknown, &ctx.sources);
        assert_eq!(unknown, json!({"id": "P9"}));
        let mut result = json!({"total": 1, "hits": [{"paperId": "c", "page": 1}], "papers": [{"id": "a"}, {"id": "zzz"}]});
        add_labels(&mut result, &ctx.sources);
        assert_eq!(result, json!({"total": 1, "hits": [{"paperId": "c", "page": 1, "label": "P3"}], "papers": [{"id": "a", "label": "P1"}, {"id": "zzz"}]}));
        assert!(ctx.instructions.contains("search_fulltext") && ctx.instructions.contains("label"));
    }

    #[test]
    fn slash_commands() {
        assert_eq!(expand_slash("  WOX5 做什么？ ").unwrap(), "WOX5 做什么？");
        let search = expand_slash("/search 10x Genomics").unwrap();
        assert!(search.starts_with("在文献库里检索：10x Genomics\n") && search.contains("search_fulltext"));
        let cite = expand_slash("/cite 根尖有干细胞。\n它们由 WOX5 维持。").unwrap();
        assert!(cite.contains("根尖有干细胞。\n它们由 WOX5 维持。") && cite.contains("find_citations"));
        assert!(expand_slash("/search").unwrap_err().contains("/search 后面写上内容"));
        assert!(expand_slash("/dance now").unwrap_err().contains("/cite"));
    }
}
