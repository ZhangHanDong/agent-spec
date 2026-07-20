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
use std::io::{BufRead, BufReader, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex};

const PROTOCOL_VERSION: &str = "2024-11-05";
const SERVER_NAME: &str = "agent-spec";
const MCP_CONTEXT_CLIENT_WORKERS: usize = 2;
const MCP_CONTEXT_CLIENT_QUEUE: usize = 4;
const MCP_EVENT_QUEUE: usize = 64;
const MCP_CONTEXT_RETRY_AFTER_MS: u64 = 100;

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
            let text = if matches!(name, "atlas_explore" | "atlas_context") {
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
    if tools::atlas_context_enabled()
        && std::env::var("AGENT_SPEC_MCP_ATLAS_QUERY_MODE").is_ok_and(|value| value == "worker")
    {
        return serve_concurrent_with_io(
            BufReader::new(stdin),
            &mut out,
            ctx,
            Arc::new(DaemonAtlasContextWorker),
            ConcurrentServeConfig {
                context_enabled: true,
                workers: MCP_CONTEXT_CLIENT_WORKERS,
                queue_capacity: MCP_CONTEXT_CLIENT_QUEUE,
            },
        );
    }
    serve_serial_with_io(stdin.lock(), &mut out, ctx)
}

fn serve_serial_with_io(
    input: impl BufRead,
    out: &mut dyn Write,
    ctx: &McpContext,
) -> std::io::Result<()> {
    for line in input.lines() {
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

#[derive(Debug, Clone, Copy)]
struct ConcurrentServeConfig {
    context_enabled: bool,
    workers: usize,
    queue_capacity: usize,
}

struct McpWorkerResult {
    payload: Value,
    is_error: bool,
}

trait AtlasContextWorker: Send + Sync + 'static {
    fn execute(&self, request_id: &str, args: &Value, ctx: &McpContext) -> McpWorkerResult;
}

struct DaemonAtlasContextWorker;

impl AtlasContextWorker for DaemonAtlasContextWorker {
    fn execute(&self, request_id: &str, args: &Value, ctx: &McpContext) -> McpWorkerResult {
        let (query, options) = match tools::atlas_context_request(args) {
            Ok(request) => request,
            Err(error) => {
                return McpWorkerResult {
                    payload: json!({
                        "schema": "agent-spec/atlas-mcp-context-error-v1",
                        "outcome": "failed",
                        "diagnostic": error,
                    }),
                    is_error: true,
                };
            }
        };
        let graph = ctx.code.join(".agent-spec/graph");
        let reply =
            match crate::atlas_daemon::context(&ctx.code, &graph, request_id, &query, &options) {
                Ok(reply) => reply,
                Err(error) => {
                    crate::atlas_query_service::QueryServiceWireReply::unavailable_attempt(
                        request_id.to_string(),
                        format!("atlas-query-unavailable: {error}"),
                    )
                }
            };
        let is_error = reply.outcome != crate::atlas_query_service::QueryOutcome::Success;
        McpWorkerResult {
            payload: serde_json::to_value(reply).unwrap_or_else(|error| {
                json!({
                    "schema": "agent-spec/atlas-mcp-context-error-v1",
                    "outcome": "failed",
                    "diagnostic": error.to_string(),
                })
            }),
            is_error,
        }
    }
}

struct ContextJob {
    jsonrpc_id: Value,
    request_id: String,
    args: Value,
}

struct ContextCompletion {
    jsonrpc_id: Value,
    result: McpWorkerResult,
}

enum ConcurrentEvent {
    Input(String),
    InputError(String),
    InputClosed,
    Context(ContextCompletion),
}

fn serve_concurrent_with_io<R: BufRead + Send, W: Write>(
    input: R,
    out: &mut W,
    ctx: &McpContext,
    worker: Arc<dyn AtlasContextWorker>,
    config: ConcurrentServeConfig,
) -> std::io::Result<()> {
    if config.workers == 0 || config.queue_capacity == 0 {
        return Err(std::io::Error::other(
            "atlas MCP worker mode requires non-zero fixed workers and queue capacity",
        ));
    }
    let (event_sender, events) = sync_channel::<ConcurrentEvent>(MCP_EVENT_QUEUE);
    let (job_sender, jobs) = sync_channel::<ContextJob>(config.queue_capacity);
    let jobs = Arc::new(Mutex::new(jobs));
    let context = Arc::new(ctx.clone());

    std::thread::scope(|scope| -> std::io::Result<()> {
        let input_events = event_sender.clone();
        scope.spawn(move || {
            for line in input.lines() {
                let event = match line {
                    Ok(line) => ConcurrentEvent::Input(line),
                    Err(error) => {
                        let _ = input_events.send(ConcurrentEvent::InputError(error.to_string()));
                        return;
                    }
                };
                if input_events.send(event).is_err() {
                    return;
                }
            }
            let _ = input_events.send(ConcurrentEvent::InputClosed);
        });

        for position in 0..config.workers {
            let worker = Arc::clone(&worker);
            let jobs = Arc::clone(&jobs);
            let completions = event_sender.clone();
            let context = Arc::clone(&context);
            scope.spawn(move || {
                loop {
                    let job = {
                        let receiver = jobs
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        receiver.recv()
                    };
                    let Ok(job) = job else {
                        break;
                    };
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        worker.execute(&job.request_id, &job.args, &context)
                    }))
                    .unwrap_or_else(|_| McpWorkerResult {
                        payload: json!({
                            "schema": "agent-spec/atlas-mcp-context-error-v1",
                            "outcome": "failed",
                            "diagnostic": format!("atlas MCP context worker {position} panicked"),
                        }),
                        is_error: true,
                    });
                    if completions
                        .send(ConcurrentEvent::Context(ContextCompletion {
                            jsonrpc_id: job.jsonrpc_id,
                            result,
                        }))
                        .is_err()
                    {
                        break;
                    }
                }
            });
        }
        drop(event_sender);

        let mut input_closed = false;
        let mut outstanding = 0_usize;
        let mut jobs = Some(job_sender);
        while !input_closed || outstanding > 0 {
            let event = events.recv().map_err(|_| {
                std::io::Error::other("atlas MCP concurrent event channel disconnected")
            })?;
            match event {
                ConcurrentEvent::Input(line) => {
                    if line.trim().is_empty() {
                        continue;
                    }
                    let request = match serde_json::from_str::<Value>(&line) {
                        Ok(request) => request,
                        Err(_) => {
                            write_response(out, &err(Value::Null, -32700, "parse error"))?;
                            continue;
                        }
                    };
                    if config.context_enabled
                        && let Some((jsonrpc_id, args)) = atlas_context_call(&request)
                    {
                        let request_id = next_mcp_context_request_id();
                        let job = ContextJob {
                            jsonrpc_id,
                            request_id: request_id.clone(),
                            args,
                        };
                        let Some(job_sender) = jobs.as_ref() else {
                            let reply = crate::atlas_query_service::QueryServiceWireReply::unavailable_attempt(
                                request_id,
                                "atlas-query-unavailable: MCP input is closed".to_string(),
                            );
                            write_worker_response(
                                out,
                                job.jsonrpc_id,
                                McpWorkerResult {
                                    payload: serde_json::to_value(reply).unwrap_or_default(),
                                    is_error: true,
                                },
                            )?;
                            continue;
                        };
                        match job_sender.try_send(job) {
                            Ok(()) => outstanding = outstanding.saturating_add(1),
                            Err(TrySendError::Full(job)) => {
                                let reply =
                                    crate::atlas_query_service::QueryServiceWireReply::busy_attempt(
                                        request_id,
                                        "atlas-query-busy: MCP context client lane is full"
                                            .to_string(),
                                        MCP_CONTEXT_RETRY_AFTER_MS,
                                    );
                                write_worker_response(
                                    out,
                                    job.jsonrpc_id,
                                    McpWorkerResult {
                                        payload: serde_json::to_value(reply).unwrap_or_default(),
                                        is_error: true,
                                    },
                                )?;
                            }
                            Err(TrySendError::Disconnected(job)) => {
                                let reply = crate::atlas_query_service::QueryServiceWireReply::unavailable_attempt(
                                    request_id,
                                    "atlas-query-unavailable: MCP context client lane stopped"
                                        .to_string(),
                                );
                                write_worker_response(
                                    out,
                                    job.jsonrpc_id,
                                    McpWorkerResult {
                                        payload: serde_json::to_value(reply).unwrap_or_default(),
                                        is_error: true,
                                    },
                                )?;
                            }
                        }
                    } else if let Some(response) = handle_request(&request, ctx) {
                        write_response(out, &response)?;
                    }
                }
                ConcurrentEvent::InputError(error) => {
                    jobs.take();
                    return Err(std::io::Error::other(error));
                }
                ConcurrentEvent::InputClosed => {
                    input_closed = true;
                    jobs.take();
                }
                ConcurrentEvent::Context(completion) => {
                    outstanding = outstanding.saturating_sub(1);
                    write_worker_response(out, completion.jsonrpc_id, completion.result)?;
                }
            }
        }
        Ok(())
    })
}

fn atlas_context_call(request: &Value) -> Option<(Value, Value)> {
    if request.get("method").and_then(Value::as_str) != Some("tools/call")
        || request.pointer("/params/name").and_then(Value::as_str) != Some("atlas_context")
    {
        return None;
    }
    Some((
        request.get("id").cloned()?,
        request
            .pointer("/params/arguments")
            .cloned()
            .unwrap_or_else(|| json!({})),
    ))
}

fn next_mcp_context_request_id() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    format!(
        "atlas-mcp-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

fn write_worker_response(
    out: &mut dyn Write,
    id: Value,
    result: McpWorkerResult,
) -> std::io::Result<()> {
    let text = serde_json::to_string(&result.payload).unwrap_or_default();
    write_response(
        out,
        &ok(
            id,
            json!({
                "content": [{ "type": "text", "text": text }],
                "isError": result.is_error,
            }),
        ),
    )
}

fn write_response(out: &mut dyn Write, response: &Value) -> std::io::Result<()> {
    writeln!(
        out,
        "{}",
        serde_json::to_string(response).unwrap_or_default()
    )?;
    out.flush()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Barrier};

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

    struct BarrierContextWorker {
        release: Arc<Barrier>,
        block_once: AtomicBool,
    }

    impl AtlasContextWorker for BarrierContextWorker {
        fn execute(&self, _request_id: &str, _args: &Value, _ctx: &McpContext) -> McpWorkerResult {
            if self.block_once.swap(false, Ordering::AcqRel) {
                self.release.wait();
            }
            McpWorkerResult {
                payload: json!({
                    "schema": "agent-spec/atlas-query-service-response-v1",
                    "outcome": "success"
                }),
                is_error: false,
            }
        }
    }

    struct PingReleaseWriter {
        bytes: Vec<u8>,
        release: Arc<Barrier>,
        release_id: u64,
        released: bool,
    }

    impl Write for PingReleaseWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.bytes.extend_from_slice(bytes);
            if !self.released
                && String::from_utf8_lossy(&self.bytes)
                    .contains(&format!("\"id\":{}", self.release_id))
            {
                self.released = true;
                self.release.wait();
            }
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_atlas_mcp_context_worker_mode_preserves_discovery_and_ping_liveness() {
        tools::with_atlas_context_tool(false, || {
            assert_eq!(tool_specs().as_array().unwrap().len(), 11);
            assert!(
                tool_specs()
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|tool| tool["name"] != "atlas_context")
            );
        });
        tools::with_atlas_context_tool(true, || {
            let tools = tool_specs();
            assert_eq!(tools.as_array().unwrap().len(), 12);
            assert!(
                tools
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|tool| tool["name"] == "atlas_context")
            );

            let input = [
                json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }),
                json!({
                    "jsonrpc": "2.0",
                    "id": 2,
                    "method": "tools/call",
                    "params": {
                        "name": "atlas_context",
                        "arguments": { "query": "MemStore", "profile": "symbol" }
                    }
                }),
                json!({ "jsonrpc": "2.0", "id": 3, "method": "ping" }),
            ]
            .into_iter()
            .map(|value| serde_json::to_string(&value).unwrap())
            .collect::<Vec<_>>()
            .join("\n")
                + "\n";
            let release = Arc::new(Barrier::new(2));
            let worker = Arc::new(BarrierContextWorker {
                release: Arc::clone(&release),
                block_once: AtomicBool::new(true),
            });
            let mut output = PingReleaseWriter {
                bytes: Vec::new(),
                release,
                release_id: 3,
                released: false,
            };
            serve_concurrent_with_io(
                Cursor::new(input.into_bytes()),
                &mut output,
                &ctx(),
                worker,
                ConcurrentServeConfig {
                    context_enabled: true,
                    workers: 1,
                    queue_capacity: 1,
                },
            )
            .unwrap();

            let responses = String::from_utf8(output.bytes)
                .unwrap()
                .lines()
                .map(|line| serde_json::from_str::<Value>(line).unwrap())
                .collect::<Vec<_>>();
            assert_eq!(
                responses
                    .iter()
                    .map(|response| response["id"].as_u64().unwrap())
                    .collect::<Vec<_>>(),
                vec![1, 3, 2]
            );
            assert_eq!(responses[2]["result"]["isError"], false);
        });
    }

    #[test]
    fn test_atlas_mcp_context_client_queue_rejects_with_typed_busy() {
        tools::with_atlas_context_tool(true, || {
            let input = [(1, "one"), (2, "two"), (3, "three")]
                .into_iter()
                .map(|(id, query)| {
                    serde_json::to_string(&json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "method": "tools/call",
                        "params": {
                            "name": "atlas_context",
                            "arguments": { "query": query }
                        }
                    }))
                    .unwrap()
                })
                .chain(std::iter::once(
                    serde_json::to_string(&json!({ "jsonrpc": "2.0", "id": 4, "method": "ping" }))
                        .unwrap(),
                ))
                .collect::<Vec<_>>()
                .join("\n")
                + "\n";
            let release = Arc::new(Barrier::new(2));
            let worker = Arc::new(BarrierContextWorker {
                release: Arc::clone(&release),
                block_once: AtomicBool::new(true),
            });
            let mut output = PingReleaseWriter {
                bytes: Vec::new(),
                release,
                release_id: 4,
                released: false,
            };
            serve_concurrent_with_io(
                Cursor::new(input.into_bytes()),
                &mut output,
                &ctx(),
                worker,
                ConcurrentServeConfig {
                    context_enabled: true,
                    workers: 1,
                    queue_capacity: 1,
                },
            )
            .unwrap();

            let responses = String::from_utf8(output.bytes)
                .unwrap()
                .lines()
                .map(|line| serde_json::from_str::<Value>(line).unwrap())
                .collect::<Vec<_>>();
            let busy = responses
                .iter()
                .find(|response| response["result"]["isError"] == true)
                .unwrap();
            let text = busy["result"]["content"][0]["text"].as_str().unwrap();
            let reply =
                serde_json::from_str::<crate::atlas_query_service::QueryServiceWireReply>(text)
                    .unwrap();
            assert_eq!(
                reply.outcome,
                crate::atlas_query_service::QueryOutcome::Busy
            );
            assert_eq!(reply.retry_after_ms, Some(MCP_CONTEXT_RETRY_AFTER_MS));
            reply.validate_shape().unwrap();
            let ping_position = responses
                .iter()
                .position(|response| response["id"] == 4)
                .unwrap();
            assert!(
                responses
                    .iter()
                    .skip(ping_position + 1)
                    .any(|response| response["result"]["isError"] == false),
                "at least one slow context completion must follow ping"
            );
        });
    }
}
