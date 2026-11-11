//! A stand-in for a model service, for trying the assistant's tool loop
//! without an API key:
//!
//!     cargo run -p wisp_core --example mock_model -- 8787
//!
//! Point a profile at it (协议 Anthropic, 地址 http://127.0.0.1:8787, any
//! key and model). It speaks the Claude Messages streaming format and always
//! plays the same parts. Asked for search queries by the guide (a request
//! without tools), it proposes two for PubMed. In a chat, asked a question, it calls `list_papers` and
//! `library_status`; given their results, it answers and cites the first
//! paper the search returned.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

/// The request body, whichever way its length is announced.
fn body(stream: &mut TcpStream) -> String {
    let mut received = vec![];
    let mut buffer = [0u8; 8192];
    loop {
        let n = stream.read(&mut buffer).unwrap_or(0);
        received.extend_from_slice(&buffer[..n]);
        let text = String::from_utf8_lossy(&received).to_string();
        if let Some(end) = text.find("\r\n\r\n") {
            let head = text[..end].to_lowercase();
            let length: usize = head.lines().find_map(|l| l.strip_prefix("content-length:").and_then(|v| v.trim().parse().ok())).unwrap_or(0);
            let done = if head.contains("transfer-encoding: chunked") { text.ends_with("0\r\n\r\n") } else { received.len() >= end + 4 + length };
            if done { return text[end + 4..].to_string() }
        }
        if n == 0 { return text }
    }
}

/// A text as a JSON string.
fn quoted(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

fn event(value: &str) -> String {
    format!("data: {value}\n\n")
}

/// The label of the first paper in the tool results, as the app wrote it: `\"label\":\"P1\"`.
fn first_label(body: &str) -> String {
    let marker = "\\\"label\\\":\\\"";
    body.find(marker).map(|at| body[at + marker.len()..].chars().take_while(|c| c.is_ascii_alphanumeric()).collect()).unwrap_or_else(|| "P1".into())
}

fn main() {
    let port = std::env::args().nth(1).unwrap_or_else(|| "8787".into());
    let listener = TcpListener::bind(format!("127.0.0.1:{port}")).expect("bind");
    println!("mock model on http://127.0.0.1:{port}");
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        let request = body(&mut stream);
        // The last message decides the step: tool results get the answer, a question gets the calls.
        let answered = request.rfind("\"tool_result\"").is_some_and(|at| request.rfind("\"role\":\"user\"").is_some_and(|user| user < at));
        let mut events = String::new();
        if !request.contains("\"tools\"") {
            // A request without tools is the guide asking for search queries: two for PubMed, as JSON in the text.
            let reply = r#"[{"source": "pubmed", "query": "(\"single cell\"[Title/Abstract] OR \"single-cell\"[Title/Abstract]) AND root[Title/Abstract] AND Arabidopsis[Title/Abstract]", "why": "宽：单细胞加根加拟南芥，可能带进只是顺带提到根的文章"}, {"source": "pubmed", "query": "\"single cell\"[Title] AND root[Title] AND (atlas OR \"stem cell niche\")", "why": "聚焦：题目里同时有单细胞和根，并限定图谱或干细胞微环境"}]"#;
            events.push_str(&event(r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#));
            events.push_str(&event(&format!(r#"{{"type":"content_block_delta","index":0,"delta":{{"type":"text_delta","text":{}}}}}"#, quoted(reply))));
            events.push_str(&event(r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"}}"#));
        } else if answered {
            let label = first_label(&request);
            events.push_str(&event(r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#));
            for piece in ["（假模型）我查了文献库的概况和匹配的文献。", &format!("最相关的一篇是 [{label}]。"), "这段话只用来演示工具调用的显示和引用的跳转。"] {
                events.push_str(&event(&format!(r#"{{"type":"content_block_delta","index":0,"delta":{{"type":"text_delta","text":"{piece}"}}}}"#)));
            }
            events.push_str(&event(r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"}}"#));
        } else {
            events.push_str(&event(r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#));
            events.push_str(&event(r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"我先看看文献库里有什么。"}}"#));
            events.push_str(&event(r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_1","name":"library_status","input":{}}}"#));
            events.push_str(&event(r#"{"type":"content_block_start","index":2,"content_block":{"type":"tool_use","id":"toolu_2","name":"list_papers","input":{}}}"#));
            events.push_str(&event(r#"{"type":"content_block_delta","index":2,"delta":{"type":"input_json_delta","partial_json":"{\"limit\": 3, \"sort\": \"year-desc\"}"}}"#));
            events.push_str(&event(r#"{"type":"message_delta","delta":{"stop_reason":"tool_use"}}"#));
        }
        let _ = stream.write_all(format!("HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n{events}").as_bytes());
    }
}
