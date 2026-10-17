//! The lab as an MCP server over stdio (JSON-RPC 2.0, one message a line): `initialize`,
//! `tools/list`, `tools/call`, `ping`. Hand-rolled: the protocol is small and the crate has
//! serde_json already.

use std::io::{BufRead, Write};

use serde_json::{json, Value};

use super::Session;

const PROTOCOL: &str = "2025-06-18";

const INSTRUCTIONS: &str = "The planet generator's lab: open a world (open_world: the dev world, a seed, or a saved .world file), then ask about it at every scale: world_summary and tile (the world map), region_survey, places, region_cell and views_agree (the region at ~600 m a cell and its embarks), render_world and render_embark (PNG files you can open), and the history (history_events, history_event, history_why, history_entity, history_stats, history_export, history_branch; `tile` lists a tile's own events) and history_explorer, which writes the whole history as an HTML app (DF-style legends reader, timeline, causal graph, family trees, web of peoples, map through the years, checks, compare). Colonies (on a world with history): colony_found founds a camp as the game does and records it as it lives (colony_run lives it on); ask colony_status, colony_settler, colony_log (log, moments, decisions), colony_why (a settler's job and reason at a moment), colony_track (their walk over a span), colony_cell and colony_path (what a cell is, whether it can be reached), act as the patron (colony_act), colony_render, colony_checks, colony_branch (the same camp with other acts) and colony_explorer (an HTML debugger: days, settlers, activity chart, needs, map replay, bonds, works, checks, compare). Tools act on the last opened world (and colony) unless given `world` (`colony`). Renders go to out/lab/.";

/// Serve until stdin closes. Stdout is the protocol's: everything the library prints goes to
/// stderr outside a call and into the call's log inside one.
pub fn serve() -> std::io::Result<()> {
    // SAFETY: duplicating and redirecting this process's own descriptors.
    let proto_fd = unsafe { libc::dup(1) };
    unsafe { libc::dup2(2, 1); }
    let mut proto = unsafe { <std::fs::File as std::os::unix::io::FromRawFd>::from_raw_fd(proto_fd) };
    let mut session = Session::new();
    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() { continue; }
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => { send(&mut proto, &json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": format!("parse error: {e}") } }))?; continue; }
        };
        if let Some(reply) = handle(&mut session, &msg) { send(&mut proto, &reply)?; }
    }
    Ok(())
}

fn send(out: &mut std::fs::File, v: &Value) -> std::io::Result<()> {
    let mut s = serde_json::to_string(v).unwrap_or_default();
    s.push('\n');
    out.write_all(s.as_bytes())?;
    out.flush()
}

/// One message in, at most one reply out (notifications get none).
pub fn handle(session: &mut Session, msg: &Value) -> Option<Value> {
    let id = msg.get("id").cloned();
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    let params = msg.get("params").cloned().unwrap_or(Value::Null);
    let id = id?; // a notification (`notifications/initialized`, `notifications/cancelled`)
    let result = match method {
        "initialize" => {
            let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or(PROTOCOL).to_string();
            Ok(json!({
                "protocolVersion": asked,
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": { "name": "planet-lab", "version": env!("CARGO_PKG_VERSION") },
                "instructions": INSTRUCTIONS,
            }))
        }
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": super::tools().iter().map(|t| json!({
            "name": t.name, "description": t.description, "inputSchema": (t.schema)(),
        })).collect::<Vec<_>>() })),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
            Ok(match session.call(name, &args) {
                Ok(out) => {
                    let mut content = vec![json!({ "type": "text", "text": out.render() })];
                    if args.get("inline_images").and_then(Value::as_bool).unwrap_or(false) {
                        for f in out.files.iter().filter(|f| f.extension().map_or(false, |e| e == "png")).take(4) {
                            if let Ok(bytes) = std::fs::read(f) {
                                use base64::Engine;
                                content.push(json!({ "type": "image", "mimeType": "image/png", "data": base64::engine::general_purpose::STANDARD.encode(bytes) }));
                            }
                        }
                    }
                    json!({ "content": content, "isError": false })
                }
                Err(e) => json!({ "content": [{ "type": "text", "text": e }], "isError": true }),
            })
        }
        _ => Err(json!({ "code": -32601, "message": format!("method not found: {method}") })),
    };
    Some(match result {
        Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
        Err(e) => json!({ "jsonrpc": "2.0", "id": id, "error": e }),
    })
}
