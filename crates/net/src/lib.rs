//! The network: metadata lookups, literature searches, the streamed chat
//! request and the update check. Shared by the app and `wisp`. Everything
//! here blocks, so the app runs it on the background executor.

use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::time::Duration;
use wisp_core::ai::{self, ChatEvent, HttpRequest, Protocol, TurnReader};
use wisp_core::lookup::{self, Identifier};
use wisp_core::model::Paper;
use wisp_core::update::{self, Release};

const USER_AGENT: &str = concat!("WispPaper/", env!("CARGO_PKG_VERSION"));

fn agent(overall: Option<Duration>) -> ureq::Agent {
    ureq::Agent::config_builder()
        // Error responses carry the reason in their body; read it rather than a bare status.
        .http_status_as_error(false)
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_recv_response(Some(Duration::from_secs(120)))
        .timeout_global(overall)
        .user_agent(USER_AGENT)
        .build()
        .into()
}

fn network_error(err: ureq::Error) -> String {
    format!("网络请求失败：{err}")
}

/// GET a JSON document; `None` for a 404.
fn get_json(url: &str) -> Result<Option<Value>, String> {
    let mut response = agent(Some(Duration::from_secs(40))).get(url).call().map_err(network_error)?;
    let status = response.status().as_u16();
    if status == 404 { return Ok(None) }
    if !(200..300).contains(&status) { return Err(format!("服务返回 HTTP {status}：{url}")) }
    response.body_mut().read_json::<Value>().map(Some).map_err(|e| format!("无法解析返回内容：{e}"))
}

/// GET a document as text: what `wisp_core::sources` asks the literature
/// databases for (JSON from most, XML from PubMed and arXiv).
pub fn get_text(url: &str) -> Result<String, String> {
    let host = url.split('/').nth(2).unwrap_or(url);
    // A service that says "too fast" is asked again, a little later each time.
    for wait in [Some(1500), Some(4000), None] {
        take_turn(host);
        let mut response = agent(Some(Duration::from_secs(60))).get(url).call().map_err(network_error)?;
        let status = response.status().as_u16();
        match (status, wait) {
            (200..300, _) => return response.body_mut().with_config().limit(64 << 20).read_to_string().map_err(|e| format!("无法读取 {host} 的返回内容：{e}")),
            (429, Some(millis)) => std::thread::sleep(Duration::from_millis(millis)),
            (429, None) => break,
            _ => return Err(format!("{host} 返回 HTTP {status}。")),
        }
    }
    Err(format!("{host} 限制了请求频率，请稍后再试。"))
}

/// NCBI allows three requests a second without a key, counted across
/// everything this process sends: searches running side by side take turns,
/// one every 400 ms. Other hosts are not held back.
fn take_turn(host: &str) {
    static NEXT: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);
    if !host.ends_with("ncbi.nlm.nih.gov") { return }
    let now = std::time::Instant::now();
    let at = {
        let mut next = NEXT.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let at = next.filter(|at| *at > now).unwrap_or(now);
        *next = Some(at + Duration::from_millis(400));
        at
    };
    std::thread::sleep(at.saturating_duration_since(now));
}

/// The paper behind a DOI or PMID: Crossref first for a DOI, completed by
/// Europe PMC (abstract, PMID); Europe PMC alone for a PMID.
pub fn lookup_paper(id: &Identifier) -> Result<Paper, String> {
    let mut notes = vec![];
    let mut paper: Option<Paper> = None;
    if let Identifier::Doi(doi) = id {
        match get_json(&lookup::crossref_url(doi)) {
            Ok(Some(body)) => paper = lookup::crossref_paper(&body),
            Ok(None) => {}
            Err(err) => notes.push(format!("Crossref：{err}")),
        }
    }
    match get_json(&lookup::europepmc_url(id)) {
        Ok(Some(body)) => {
            if let Some(extra) = lookup::europepmc_paper(&body) {
                paper = Some(match paper {
                    Some(found) => lookup::merge_lookup(found, extra),
                    None => extra,
                });
            }
        }
        Ok(None) => {}
        Err(err) => notes.push(format!("Europe PMC：{err}")),
    }
    paper.ok_or_else(|| if notes.is_empty() { "没有找到这个编号对应的文献，请检查后重试，或改用手动录入。".to_string() } else { notes.join("；") })
}

/// The newest published release, read off the redirect `url` answers with
/// (`update::latest_url()`); `None` when nothing has been published. Only
/// the redirect is read: no page or package is fetched.
pub fn latest_release(url: &str) -> Result<Option<Release>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder().http_status_as_error(false).max_redirects(0).timeout_global(Some(Duration::from_secs(20))).user_agent(USER_AGENT).build().into();
    let response = agent.get(url).call().map_err(network_error)?;
    let status = response.status().as_u16();
    match status {
        300..400 => Ok(response.headers().get("location").and_then(|v| v.to_str().ok()).and_then(update::release_from_location)),
        200..300 | 404 => Ok(None),
        _ => Err(format!("更新服务器返回 HTTP {status}")),
    }
}

/// Send a chat request and forward the model's turn: its text as it
/// arrives, then the whole turn with any tool calls. Returns when the stream
/// ends or the receiver is gone (the conversation was reset).
pub fn stream_chat(request: HttpRequest, protocol: Protocol, events: async_channel::Sender<ChatEvent>) {
    let send = |event| events.send_blocking(event).is_ok();
    let mut call = agent(None).post(&request.url);
    for (name, value) in &request.headers {
        call = call.header(name, value);
    }
    // Compact JSON: the documents can be hundreds of kilobytes, and indentation only adds to them.
    let mut response = match call.send(request.body.to_string()) {
        Ok(response) => response,
        Err(err) => {
            send(ChatEvent::Error(network_error(err)));
            return;
        }
    };
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        let body = response.body_mut().read_to_string().unwrap_or_default();
        send(ChatEvent::Error(ai::error_message(status, &body)));
        return;
    }
    let mut reader = TurnReader::new(protocol);
    for line in BufReader::new(response.body_mut().as_reader()).lines() {
        let line = match line {
            Ok(line) => line,
            Err(err) => {
                send(ChatEvent::Error(format!("连接中断：{err}")));
                return;
            }
        };
        match reader.line(&line) {
            Ok(Some(text)) => {
                if !send(ChatEvent::Text(text)) { return }
            }
            Ok(None) => {}
            Err(error) => {
                send(ChatEvent::Error(error));
                return;
            }
        }
    }
    send(ChatEvent::Done(reader.finish()));
}

/// Send a request and wait for the whole answer as text: for a one-off
/// question whose reply is read by code, not shown as it arrives.
pub fn complete(request: HttpRequest, protocol: Protocol) -> Result<String, String> {
    let (sender, events) = async_channel::unbounded();
    stream_chat(request, protocol, sender);
    let mut text = String::new();
    while let Ok(event) = events.try_recv() {
        match event {
            ChatEvent::Text(more) => text.push_str(&more),
            ChatEvent::Error(error) => return Err(error),
            // A reply that was cut off or declined is not one to parse.
            ChatEvent::Done(turn) => {
                if let Some(note) = ai::stop_note(&turn.stop) { return Err(note.to_string()) }
            }
        }
    }
    if text.trim().is_empty() { return Err("模型没有返回内容。".into()) }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use wisp_core::ai::{Context, Profile, chat_request, user_message};

    /// Serve one canned HTTP response on a local port; returns the base URL and the request it received.
    fn serve(response: &'static str) -> (String, std::thread::JoinHandle<String>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut received = vec![];
            let mut buffer = [0u8; 4096];
            // Read the headers and the body they announce.
            loop {
                let n = stream.read(&mut buffer).unwrap();
                received.extend_from_slice(&buffer[..n]);
                let text = String::from_utf8_lossy(&received).to_string();
                if let Some(end) = text.find("\r\n\r\n") {
                    let length: usize = text.lines().find_map(|l| l.to_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap())).unwrap_or(0);
                    // ureq sends a JSON body chunked: it ends with a zero-length chunk.
                    let chunked = text.to_lowercase().contains("transfer-encoding: chunked");
                    if (chunked && text.ends_with("0

")) || (!chunked && received.len() >= end + 4 + length) { break }
                }
                if n == 0 { break }
            }
            stream.write_all(response.as_bytes()).unwrap();
            String::from_utf8_lossy(&received).to_string()
        });
        (base, handle)
    }

    /// Send the conversation `wire` to the server at `base` and collect what comes back.
    fn step(base: String, protocol: Protocol, wire: &[Value]) -> (Vec<ChatEvent>, HttpRequest) {
        let profile = Profile { protocol, base_url: base, api_key: "test-key".into(), model: "m".into(), ..Default::default() };
        let ctx = Context { instructions: "I".into(), documents: "D".into(), ..Default::default() };
        let request = chat_request(&profile, &ctx, wire, &wisp_core::tools::library_catalog()).unwrap();
        let (sender, events) = async_channel::unbounded();
        stream_chat(request.clone(), protocol, sender);
        let mut seen = vec![];
        while let Ok(event) = events.try_recv() {
            seen.push(event);
        }
        (seen, request)
    }

    fn ask(base: String, protocol: Protocol) -> (Vec<ChatEvent>, HttpRequest) {
        step(base, protocol, &[user_message("你好")])
    }

    /// One question answered in two steps: the model calls a tool, gets the
    /// result, and answers. What matters is what the second request carries.
    #[test]
    fn tool_round_trip() {
        let (base, server) = serve(
            "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n\
data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\",\"signature\":\"\"}}\n\n\
data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"signature_delta\",\"signature\":\"c2ln\"}}\n\n\
data: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_1\",\"name\":\"search_fulltext\",\"input\":{}}}\n\n\
data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"query\\\": \\\"chromium\\\"}\"}}\n\n\
data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\"}}\n\n",
        );
        let mut wire = vec![user_message("哪篇文献提到 Chromium？")];
        let (events, _) = step(base, Protocol::Anthropic, &wire);
        server.join().unwrap();
        let [ChatEvent::Done(turn)] = &events[..] else { panic!("{events:?}") };
        assert_eq!((turn.calls[0].name.as_str(), turn.calls[0].args.clone()), ("search_fulltext", Ok(serde_json::json!({"query": "chromium"}))));
        assert!(turn.runnable());

        wire.push(turn.message.clone());
        wire.extend(ai::tool_results(Protocol::Anthropic, &[(turn.calls[0].id.clone(), "{\"total\":1}".into(), false)]));
        let (base, server) = serve(
            "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n\
data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n\
data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"[P1 p.2] 提到了。\"}}\n\n\
data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"}}\n\n",
        );
        let (events, request) = step(base, Protocol::Anthropic, &wire);
        server.join().unwrap();
        assert_eq!(events[0], ChatEvent::Text("[P1 p.2] 提到了。".into()));
        assert!(matches!(&events[1], ChatEvent::Done(turn) if turn.calls.is_empty() && turn.stop == "end_turn"));
        // The model's turn went back whole, followed by the result of its call.
        let sent = &request.body["messages"];
        assert_eq!(sent[1]["content"][0], serde_json::json!({"type": "thinking", "thinking": "", "signature": "c2ln"}));
        assert_eq!(sent[1]["content"][1]["input"]["query"], "chromium");
        assert_eq!(sent[2]["content"][0], serde_json::json!({"type": "tool_result", "tool_use_id": "toolu_1", "content": "{\"total\":1}", "is_error": false}));
    }

    #[test]
    fn anthropic_stream() {
        let (base, server) = serve(
            "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n\
event: message_start\ndata: {\"type\":\"message_start\",\"message\":{}}\n\n\
event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\",\"signature\":\"\"}}\n\n\
event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"signature_delta\",\"signature\":\"c2ln\"}}\n\n\
event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n\
event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"text_delta\",\"text\":\"根尖 \"}}\n\n\
event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"text_delta\",\"text\":\"[P1]\"}}\n\n\
event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"}}\n\n\
event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
        );
        let (events, _) = ask(base, Protocol::Anthropic);
        assert_eq!(events[..2], [ChatEvent::Text("根尖 ".into()), ChatEvent::Text("[P1]".into())]);
        let ChatEvent::Done(turn) = &events[2] else { panic!("{events:?}") };
        // The turn goes back to the model as it came, the signed thinking block included.
        assert_eq!((turn.stop.as_str(), turn.calls.len(), turn.message["content"][0]["signature"].as_str(), turn.message["content"][1]["text"].as_str()), ("end_turn", 0, Some("c2ln"), Some("根尖 [P1]")));
        let received = server.join().unwrap();
        assert!(received.starts_with("POST /v1/messages "), "{received}");
        assert!(received.to_lowercase().contains("x-api-key: test-key"));
        assert!(received.contains("\"stream\":true") && received.contains("你好") && received.contains("\"search_fulltext\""), "{received}");
        // A local address is not Anthropic's API: no beta parameters.
        assert!(!received.contains("fallbacks"));
    }

    #[test]
    fn responses_stream() {
        let (base, server) = serve(
            "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n\
event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"output_index\":0,\"delta\":\"根尖 [P1]\"}\n\n\
event: response.output_item.done\ndata: {\"type\":\"response.output_item.done\",\"output_index\":0,\"item\":{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"根尖 [P1]\"}]}}\n\n\
event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\n\n",
        );
        let (events, request) = ask(format!("{base}/v1"), Protocol::Responses);
        assert_eq!(events[0], ChatEvent::Text("根尖 [P1]".into()));
        assert!(matches!(&events[1], ChatEvent::Done(turn) if turn.stop == "stop" && turn.wire()[0]["id"] == "msg_1"));
        let received = server.join().unwrap();
        assert!(received.starts_with("POST /v1/responses "), "{received}");
        assert!(received.to_lowercase().contains("authorization: bearer test-key"));
        assert_eq!((request.body["store"].as_bool(), request.body["input"][0]["content"].as_str()), (Some(false), Some("你好")));
    }

    #[test]
    fn openai_stream_and_errors() {
        let (base, server) = serve("HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\ndata: {\"choices\":[{\"delta\":{\"content\":\"Hi\"},\"finish_reason\":null}]}\n\ndata: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n");
        let (events, _) = ask(format!("{base}/v1"), Protocol::OpenAI);
        assert_eq!(events[0], ChatEvent::Text("Hi".into()));
        assert!(matches!(&events[1], ChatEvent::Done(turn) if turn.stop == "stop" && turn.message["content"] == "Hi"));
        assert!(server.join().unwrap().starts_with("POST /v1/chat/completions "));

        let (base, server) = serve("HTTP/1.1 401 Unauthorized\r\ncontent-type: application/json\r\ncontent-length: 41\r\nconnection: close\r\n\r\n{\"error\":{\"message\":\"invalid x-api-key\"}}");
        let (events, _) = ask(base, Protocol::Anthropic);
        assert_eq!(events, [ChatEvent::Error("API Key 无效或没有权限（HTTP 401）：invalid x-api-key".into())]);
        server.join().unwrap();

        // Nothing listening: a network error, not a hang.
        let (events, _) = ask("http://127.0.0.1:9".into(), Protocol::OpenAI);
        assert!(matches!(&events[..], [ChatEvent::Error(e)] if e.starts_with("网络请求失败")));
    }

    #[test]
    fn latest_release_reads_the_redirect() {
        let (base, server) = serve("HTTP/1.1 302 Found\r\nlocation: https://github.com/xuzhougeng/WispPaper/releases/tag/v9.8.7\r\ncontent-length: 0\r\nconnection: close\r\n\r\n");
        let release = latest_release(&format!("{base}/releases/latest")).unwrap().unwrap();
        assert_eq!((release.version.as_str(), release.url.as_str()), ("9.8.7", "https://github.com/xuzhougeng/WispPaper/releases/tag/v9.8.7"));
        assert!(server.join().unwrap().starts_with("GET /releases/latest "));

        // No release published: GitHub redirects to the list. Elsewhere: never offered.
        for response in ["HTTP/1.1 302 Found\r\nlocation: https://github.com/xuzhougeng/WispPaper/releases\r\ncontent-length: 0\r\nconnection: close\r\n\r\n", "HTTP/1.1 302 Found\r\nlocation: https://example.com/releases/tag/v9.9.9\r\ncontent-length: 0\r\nconnection: close\r\n\r\n", "HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"] {
            let (base, server) = serve(response);
            assert_eq!(latest_release(&base), Ok(None));
            server.join().unwrap();
        }
        let (base, server) = serve("HTTP/1.1 503 Service Unavailable\r\ncontent-length: 0\r\nconnection: close\r\n\r\n");
        assert_eq!(latest_release(&base), Err("更新服务器返回 HTTP 503".into()));
        server.join().unwrap();
        assert!(latest_release("http://127.0.0.1:9").unwrap_err().starts_with("网络请求失败"));
    }

    /// Talks to Crossref and Europe PMC: `cargo test -- --ignored lookup`.
    #[test]
    #[ignore]
    fn lookup_real_identifiers() {
        let paper = lookup_paper(&Identifier::Doi("10.1038/s41586-020-2649-2".into())).unwrap();
        assert_eq!(paper.title, "Array programming with NumPy");
        assert_eq!((paper.year, paper.journal.as_str(), paper.pmid.as_str()), (Some(2020), "Nature", "32939066"));
        assert!(paper.authors.starts_with("Harris, Charles R.") && !paper.abstract_text.is_empty());
        let by_pmid = lookup_paper(&Identifier::Pmid("32939066".into())).unwrap();
        assert_eq!(by_pmid.doi, "10.1038/s41586-020-2649-2");
        assert!(lookup_paper(&Identifier::Doi("10.9999/definitely-not-a-real-doi-000".into())).is_err());
    }
}
