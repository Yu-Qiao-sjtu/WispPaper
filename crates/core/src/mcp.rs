//! The Model Context Protocol face of `tools`: what to answer to one
//! JSON-RPC message. The transport (lines on stdin and stdout) is the `wisp`
//! binary's; nothing here reads or writes a stream.

use crate::tools;
use serde_json::{Value, json};
use std::path::Path;

/// Protocol revisions this server speaks, newest first.
const VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

const INSTRUCTIONS: &str = "Wisp Paper 的本地文献库。先用 list_libraries 看有哪些文献库，再用 library 参数指定其中一个。\
查出处时先 search_fulltext，再用 read_fulltext 读命中的页面核对上下文；没有保存全文的文献只有书目信息和摘要。\
收录判断（screening）由用户本人决定；它的存储值不等于界面上的名称（excluded 显示为「相关」），名称见 library_status 的 labels。";

/// The reply to a message, or `None` when it asks for none (a notification,
/// or a response to something this server never sent).
pub fn handle(root: &Path, message: &Value) -> Option<Value> {
    let method = message["method"].as_str()?;
    let id = message.get("id").filter(|id| !id.is_null())?.clone();
    let params = &message["params"];
    let result = match method {
        "initialize" => {
            let asked = params["protocolVersion"].as_str().unwrap_or("");
            Ok(json!({
                "protocolVersion": VERSIONS.iter().find(|v| **v == asked).unwrap_or(&VERSIONS[0]),
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "wisp-paper", "title": "Wisp Paper", "version": env!("CARGO_PKG_VERSION")},
                "instructions": INSTRUCTIONS,
            }))
        }
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({"tools": tools::catalog().iter().map(|t| json!({"name": t.name, "title": t.title, "description": t.description, "inputSchema": t.input_schema})).collect::<Vec<_>>()})),
        "tools/call" => match params["name"].as_str() {
            None => Err((-32602, "tools/call needs the name of a tool".to_string())),
            Some(name) => {
                let empty = json!({});
                let args = if params["arguments"].is_null() { &empty } else { &params["arguments"] };
                // A failed tool is a result the model reads and recovers from, not a protocol error.
                Ok(match tools::call(root, name, args) {
                    Ok(value) => json!({"content": [{"type": "text", "text": value.to_string()}], "structuredContent": value, "isError": false}),
                    Err(error) => json!({"content": [{"type": "text", "text": error}], "isError": true}),
                })
            }
        },
        _ => Err((-32601, format!("method not found: {method}"))),
    };
    Some(match result {
        Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
        Err((code, text)) => json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": text}}),
    })
}

/// The reply to a line that is not JSON.
pub fn parse_error(detail: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": format!("parse error: {detail}")}})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session() {
        let (root, id) = tools::tests::sample();
        let ask = |message: Value| handle(&root, &message);
        let hello = ask(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}}})).unwrap();
        assert_eq!((hello["id"].as_i64(), hello["result"]["protocolVersion"].as_str()), (Some(1), Some("2025-03-26")));
        assert!(hello["result"]["capabilities"]["tools"].is_object());
        // An unknown revision gets the newest one this server speaks.
        assert_eq!(ask(json!({"id": "a", "method": "initialize", "params": {"protocolVersion": "1999-01-01"}})).unwrap()["result"]["protocolVersion"], VERSIONS[0]);
        assert_eq!(ask(json!({"jsonrpc": "2.0", "method": "notifications/initialized"})), None);
        assert_eq!(ask(json!({"jsonrpc": "2.0", "id": 9, "result": {}})), None);
        assert_eq!(ask(json!({"id": 2, "method": "ping"})).unwrap()["result"], json!({}));

        let listed = ask(json!({"id": 3, "method": "tools/list"})).unwrap();
        let names: Vec<&str> = listed["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"search_fulltext") && listed["result"]["tools"][0]["inputSchema"]["type"] == "object");

        let called = ask(json!({"id": 4, "method": "tools/call", "params": {"name": "get_paper", "arguments": {"id": id}}})).unwrap();
        assert_eq!((called["result"]["isError"].as_bool(), called["result"]["structuredContent"]["title"].as_str()), (Some(false), Some("Root stem cell niche")));
        assert!(called["result"]["content"][0]["text"].as_str().unwrap().contains("Root stem cell niche"));
        // No arguments at all is an empty object.
        assert_eq!(ask(json!({"id": 5, "method": "tools/call", "params": {"name": "list_libraries"}})).unwrap()["result"]["structuredContent"]["libraries"][0]["name"], "根");
        let failed = ask(json!({"id": 6, "method": "tools/call", "params": {"name": "get_paper", "arguments": {"id": "P-none"}}})).unwrap();
        assert_eq!(failed["result"]["isError"], true);
        assert!(failed["result"]["content"][0]["text"].as_str().unwrap().contains("文献不存在"));

        assert_eq!(ask(json!({"id": 7, "method": "tools/call", "params": {}})).unwrap()["error"]["code"], -32602);
        assert_eq!(ask(json!({"id": 8, "method": "resources/list"})).unwrap()["error"]["code"], -32601);
        assert_eq!(parse_error("x")["error"]["code"], -32700);
        std::fs::remove_dir_all(root).ok();
    }
}
