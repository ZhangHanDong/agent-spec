//! Read-only, deterministic MCP server (§10). Speaks JSON-RPC 2.0 over
//! newline-delimited stdio (the MCP stdio transport). No RAG/embeddings/model
//! calls — a thin read layer over SpecGateway, the satisfies index, and the
//! knowledge parser. The protocol routing (`handle_request`) is a pure function
//! so it is unit-testable without spawning a process.
#![allow(unused_imports)]

pub mod tools;

#[cfg(test)]
pub(crate) use tools::with_atlas_explore_tool;
pub use tools::{McpContext, dispatch, tool_specs};

use serde_json::{Value, json};
use std::io::{BufRead, Write};

const PROTOCOL_VERSION: &str = "2024-11-05";
const SERVER_NAME: &str = "agent-spec";

/// Route a single JSON-RPC request. Returns `Some(response)` for requests and
/// `None` for notifications (methods with no `id`, e.g. `notifications/*`).
pub fn handle_request(req: &Value, ctx: &McpContext) -> Option<Value> {
    let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");

    // Notifications carry no id and expect no response.
    let id = req.get("id").cloned()?;

    match method {
        "initialize" => Some(ok(
            id,
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": SERVER_NAME, "version": env!("CARGO_PKG_VERSION") },
            }),
        )),
        "ping" => Some(ok(id, json!({}))),
        "tools/list" => Some(ok(id, json!({ "tools": tool_specs() }))),
        "tools/call" => Some(handle_tools_call(id, req, ctx)),
        other => Some(err(id, -32601, &format!("method not found: {other}"))),
    }
}

fn handle_tools_call(id: Value, req: &Value, ctx: &McpContext) -> Value {
    let params = req.get("params").cloned().unwrap_or(Value::Null);
    let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    match dispatch(name, &args, ctx) {
        Ok(payload) => {
            let text = if name == "atlas_explore" {
                serde_json::to_string(&payload)
            } else {
                serde_json::to_string_pretty(&payload)
            }
            .unwrap_or_default();
            ok(
                id,
                json!({ "content": [ { "type": "text", "text": text } ], "isError": false }),
            )
        }
        Err(message) => ok(
            id,
            json!({ "content": [ { "type": "text", "text": message } ], "isError": true }),
        ),
    }
}

fn ok(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn err(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// Serve MCP over stdio until EOF. One JSON object per line in and out.
pub fn serve(ctx: &McpContext) -> std::io::Result<()> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let parsed: Result<Value, _> = serde_json::from_str(&line);
        match parsed {
            Ok(req) => {
                if let Some(resp) = handle_request(&req, ctx) {
                    writeln!(out, "{}", serde_json::to_string(&resp).unwrap_or_default())?;
                    out.flush()?;
                }
            }
            Err(_) => {
                let resp = err(Value::Null, -32700, "parse error");
                writeln!(out, "{}", serde_json::to_string(&resp).unwrap_or_default())?;
                out.flush()?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn ctx() -> McpContext {
        McpContext {
            knowledge: PathBuf::from("knowledge"),
            specs: PathBuf::from("specs"),
            code: PathBuf::from("."),
        }
    }

    #[test]
    fn test_initialize_returns_server_info() {
        let resp = handle_request(
            &json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize" }),
            &ctx(),
        )
        .unwrap();
        assert_eq!(resp["result"]["serverInfo"]["name"], "agent-spec");
        assert_eq!(resp["result"]["protocolVersion"], PROTOCOL_VERSION);
    }

    #[test]
    fn test_tools_list_has_six_tools() {
        let resp = handle_request(
            &json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
            &ctx(),
        )
        .unwrap();
        assert_eq!(resp["result"]["tools"].as_array().unwrap().len(), 11);
    }

    #[test]
    fn test_notification_has_no_response() {
        let resp = handle_request(
            &json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            &ctx(),
        );
        assert!(resp.is_none());
    }

    #[test]
    fn test_unknown_method_is_error() {
        let resp = handle_request(
            &json!({ "jsonrpc": "2.0", "id": 3, "method": "no/such" }),
            &ctx(),
        )
        .unwrap();
        assert_eq!(resp["error"]["code"], -32601);
    }

    #[test]
    fn test_tools_call_unknown_tool_is_iserror() {
        let resp = handle_request(
            &json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call",
                     "params": { "name": "bogus", "arguments": {} } }),
            &ctx(),
        )
        .unwrap();
        assert_eq!(resp["result"]["isError"], true);
    }
}
