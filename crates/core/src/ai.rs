//! 文献库提问: what the model is given, the request for either protocol, and
//! reading the streamed answer. No HTTP here; the app sends the request.
//!
//! The library is put in front of the model rather than searched by it: every
//! paper's record (most relevant first) and the full-text pages that match
//! the question, each under a label like `[P3]` or `[P3 p.5]` the answer
//! cites. What did not fit is counted and shown, never dropped silently.

use crate::fulltext::{score, terms};
use crate::model::{FieldDef, PUBLICATION_STATUSES, Paper, PaperFile, READINGS, SCREENINGS, label};
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

#[derive(Clone, Debug, PartialEq)]
pub struct ChatMessage {
    pub from_user: bool,
    pub content: String,
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

/// The streaming request for one turn of the conversation.
pub fn chat_request(profile: &Profile, ctx: &Context, messages: &[ChatMessage]) -> Result<HttpRequest, String> {
    let key = profile.key();
    if key.is_empty() { return Err("还没有配置 API Key。请在「设置 › AI」里填写，或设置对应的环境变量。".into()) }
    if profile.model.trim().is_empty() { return Err("请在「设置 › AI」里填写模型名称。".into()) }
    let base = profile.base_url.trim().trim_end_matches('/');
    if !base.starts_with("https://") && !base.starts_with("http://") { return Err("模型服务地址应以 https:// 或 http:// 开头。".into()) }
    let turns: Vec<Value> = messages.iter().map(|m| json!({"role": if m.from_user { "user" } else { "assistant" }, "content": m.content})).collect();
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
                "messages": turns,
            });
            // Only on Anthropic's own API: the other platforms and proxies reject the parameter.
            if base == ANTHROPIC_BASE && FALLBACK_MODELS.contains(&profile.model.trim()) {
                headers.push(("anthropic-beta".to_string(), "server-side-fallback-2026-07-01".to_string()));
                body["fallbacks"] = json!("default");
            }
            Ok(HttpRequest { url: format!("{base}/v1/messages"), headers, body })
        }
        Protocol::OpenAI => {
            let mut all = vec![json!({"role": "system", "content": format!("{}\n\n{}", ctx.instructions, ctx.documents)})];
            all.extend(turns);
            Ok(HttpRequest {
                url: format!("{base}/chat/completions"),
                headers: vec![("authorization".to_string(), format!("Bearer {key}")), ("content-type".to_string(), "application/json".to_string())],
                body: json!({"model": profile.model.trim(), "stream": true, "messages": all}),
            })
        }
    }
}

// ---- the streamed answer

#[derive(Clone, Debug, PartialEq)]
pub enum StreamEvent {
    Text(String),
    /// The turn ended, with the protocol's stop reason.
    Stop(String),
    Error(String),
}

/// One line of the server-sent event stream. Both protocols put a complete
/// JSON object on every `data:` line.
pub fn stream_line(protocol: Protocol, line: &str) -> Vec<StreamEvent> {
    let Some(data) = line.strip_prefix("data:").map(str::trim) else { return vec![] };
    if data.is_empty() { return vec![] }
    if data == "[DONE]" { return vec![StreamEvent::Stop("end".into())] }
    let Ok(value) = serde_json::from_str::<Value>(data) else { return vec![] };
    let mut events = vec![];
    if let Some(message) = value["error"]["message"].as_str() {
        return vec![StreamEvent::Error(message.to_string())];
    }
    match protocol {
        Protocol::Anthropic => match value["type"].as_str() {
            // Thinking and fallback blocks carry no answer text and are skipped.
            Some("content_block_delta") if value["delta"]["type"] == "text_delta" => {
                if let Some(text) = value["delta"]["text"].as_str() && !text.is_empty() { events.push(StreamEvent::Text(text.to_string())) }
            }
            Some("message_delta") => {
                if let Some(reason) = value["delta"]["stop_reason"].as_str() { events.push(StreamEvent::Stop(reason.to_string())) }
            }
            _ => {}
        },
        Protocol::OpenAI => {
            let choice = &value["choices"][0];
            if let Some(text) = choice["delta"]["content"].as_str() && !text.is_empty() { events.push(StreamEvent::Text(text.to_string())) }
            if let Some(reason) = choice["finish_reason"].as_str() { events.push(StreamEvent::Stop(reason.to_string())) }
        }
    }
    events
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
        let messages = [ChatMessage { from_user: true, content: "hi".into() }];
        let claude = Profile { api_key: "k".into(), ..Default::default() };
        let r = chat_request(&claude, &ctx, &messages).unwrap();
        assert_eq!(r.url, "https://api.anthropic.com/v1/messages");
        assert_eq!(r.body["model"], "claude-opus-5-5");
        assert_eq!(r.body["fallbacks"], "default");
        assert_eq!(r.body["system"][1]["cache_control"]["type"], "ephemeral");
        assert!(r.body.get("thinking").is_none() && r.body.get("temperature").is_none());
        assert!(r.headers.iter().any(|(k, v)| k == "anthropic-beta" && v == "server-side-fallback-2026-07-01"));
        // A proxy does not get the beta parameter.
        let proxied = chat_request(&Profile { base_url: "https://proxy.example/".into(), ..claude.clone() }, &ctx, &messages).unwrap();
        assert!(proxied.body.get("fallbacks").is_none());
        assert_eq!(proxied.url, "https://proxy.example/v1/messages");

        let other = Profile { protocol: Protocol::OpenAI, base_url: "http://localhost:11434/v1".into(), api_key: "k".into(), model: "qwen".into(), ..Default::default() };
        let r = chat_request(&other, &ctx, &messages).unwrap();
        assert_eq!(r.url, "http://localhost:11434/v1/chat/completions");
        assert_eq!(r.body["messages"][0]["content"], "I\n\nD");
        assert_eq!(r.body["messages"][1]["role"], "user");
        assert!(chat_request(&Profile { base_url: "ftp://x".into(), api_key: "k".into(), ..Default::default() }, &ctx, &messages).is_err());
    }

    #[test]
    fn streams() {
        use StreamEvent::*;
        let a = |line| stream_line(Protocol::Anthropic, line);
        assert_eq!(a("event: content_block_delta"), []);
        assert_eq!(a(r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#), [Text("Hello".into())]);
        assert_eq!(a(r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"hm"}}"#), []);
        assert_eq!(a(r#"data: {"type":"message_delta","delta":{"stop_reason":"refusal"},"usage":{"output_tokens":3}}"#), [Stop("refusal".into())]);
        assert_eq!(a(r#"data: {"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#), [Error("Overloaded".into())]);
        let o = |line| stream_line(Protocol::OpenAI, line);
        assert_eq!(o(r#"data: {"choices":[{"delta":{"content":"Hi"},"finish_reason":null}]}"#), [Text("Hi".into())]);
        assert_eq!(o(r#"data: {"choices":[{"delta":{},"finish_reason":"stop"}]}"#), [Stop("stop".into())]);
        assert_eq!(o("data: [DONE]"), [Stop("end".into())]);
        assert!(stop_note("refusal").is_some() && stop_note("end_turn").is_none());
        assert_eq!(error_message(401, r#"{"error":{"message":"bad key"}}"#), "API Key 无效或没有权限（HTTP 401）：bad key");
    }
}
