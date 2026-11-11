//! The network: metadata lookups and the streamed chat request. Everything
//! here blocks, so it runs on the background executor.

use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::time::Duration;
use wisp_core::ai::{self, HttpRequest, Protocol, StreamEvent};
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

/// Send a chat request and forward the streamed answer. Returns when the
/// stream ends or the receiver is gone (the conversation was reset).
pub fn stream_chat(request: HttpRequest, protocol: Protocol, events: async_channel::Sender<StreamEvent>) {
    let send = |event| events.send_blocking(event).is_ok();
    let mut call = agent(None).post(&request.url);
    for (name, value) in &request.headers {
        call = call.header(name, value);
    }
    // Compact JSON: the documents can be hundreds of kilobytes, and indentation only adds to them.
    let mut response = match call.send(request.body.to_string()) {
        Ok(response) => response,
        Err(err) => {
            send(StreamEvent::Error(network_error(err)));
            return;
        }
    };
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        let body = response.body_mut().read_to_string().unwrap_or_default();
        send(StreamEvent::Error(ai::error_message(status, &body)));
        return;
    }
    let mut stopped = false;
    for line in BufReader::new(response.body_mut().as_reader()).lines() {
        let line = match line {
            Ok(line) => line,
            Err(err) => {
                send(StreamEvent::Error(format!("连接中断：{err}")));
                return;
            }
        };
        for event in ai::stream_line(protocol, &line) {
            stopped |= matches!(event, StreamEvent::Stop(_) | StreamEvent::Error(_));
            if !send(event) { return }
        }
    }
    if !stopped {
        send(StreamEvent::Stop("end".into()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use wisp_core::ai::{ChatMessage, Context, Profile, chat_request};

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

    fn ask(base: String, protocol: Protocol) -> (Vec<StreamEvent>, HttpRequest) {
        let profile = Profile { protocol, base_url: base, api_key: "test-key".into(), model: "m".into(), ..Default::default() };
        let ctx = Context { instructions: "I".into(), documents: "D".into(), ..Default::default() };
        let request = chat_request(&profile, &ctx, &[ChatMessage { from_user: true, content: "你好".into() }]).unwrap();
        let (sender, events) = async_channel::unbounded();
        stream_chat(request.clone(), protocol, sender);
        let mut seen = vec![];
        while let Ok(event) = events.try_recv() {
            seen.push(event);
        }
        (seen, request)
    }

    #[test]
    fn anthropic_stream() {
        let (base, server) = serve(
            "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n\
event: message_start\ndata: {\"type\":\"message_start\",\"message\":{}}\n\n\
event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"\"}}\n\n\
event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"text_delta\",\"text\":\"根尖 \"}}\n\n\
event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"text_delta\",\"text\":\"[P1]\"}}\n\n\
event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"}}\n\n\
event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
        );
        let (events, _) = ask(base, Protocol::Anthropic);
        assert_eq!(events, [StreamEvent::Text("根尖 ".into()), StreamEvent::Text("[P1]".into()), StreamEvent::Stop("end_turn".into())]);
        let received = server.join().unwrap();
        assert!(received.starts_with("POST /v1/messages "), "{received}");
        assert!(received.to_lowercase().contains("x-api-key: test-key"));
        assert!(received.contains("\"stream\":true") && received.contains("你好"), "{received}");
        // A local address is not Anthropic's API: no beta parameters.
        assert!(!received.contains("fallbacks"));
    }

    #[test]
    fn openai_stream_and_errors() {
        let (base, server) = serve("HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\ndata: {\"choices\":[{\"delta\":{\"content\":\"Hi\"},\"finish_reason\":null}]}\n\ndata: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n");
        let (events, _) = ask(format!("{base}/v1"), Protocol::OpenAI);
        assert_eq!(events[..2], [StreamEvent::Text("Hi".into()), StreamEvent::Stop("stop".into())]);
        assert!(server.join().unwrap().starts_with("POST /v1/chat/completions "));

        let (base, server) = serve("HTTP/1.1 401 Unauthorized\r\ncontent-type: application/json\r\ncontent-length: 41\r\nconnection: close\r\n\r\n{\"error\":{\"message\":\"invalid x-api-key\"}}");
        let (events, _) = ask(base, Protocol::Anthropic);
        assert_eq!(events, [StreamEvent::Error("API Key 无效或没有权限（HTTP 401）：invalid x-api-key".into())]);
        server.join().unwrap();

        // Nothing listening: a network error, not a hang.
        let (events, _) = ask("http://127.0.0.1:9".into(), Protocol::OpenAI);
        assert!(matches!(&events[..], [StreamEvent::Error(e)] if e.starts_with("网络请求失败")));
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
