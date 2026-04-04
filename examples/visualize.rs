//! Generate an interactive HTML trace visualization.
//!
//! Usage:
//!   cargo run --example visualize
//!   open target/visualize.html

#[path = "scenarios/mod.rs"]
mod scenarios;

use std::collections::HashMap;

use kv_store::simulator::{LogEntry, Simulator};
use kv_store::{ActorId, Message, MessagePayload, Operation, OperationResult};

/// Display name for an actor. Node and Server collapse to "Server" since the
/// node sits behind the server and clients never see it directly.
fn actor_name(a: &ActorId) -> &'static str {
    match a {
        ActorId::Server | ActorId::Node(_) => "Server",
        ActorId::Client(id) => match id.0 {
            0 => "Client 0",
            1 => "Client 1",
            2 => "Client 2",
            3 => "Client 3",
            4 => "Client 4",
            _ => "Client ?",
        },
    }
}

/// Extract the client-side ID from a message (one end is always a client).
fn client_id_of(msg: &Message) -> u8 {
    match (&msg.from, &msg.to) {
        (ActorId::Client(id), _) | (_, ActorId::Client(id)) => id.0,
        _ => 0,
    }
}

fn op_kind(op: &Operation) -> &'static str {
    match op {
        Operation::Put { .. } => "Put",
        Operation::Get { .. } => "Get",
        Operation::Delete { .. } => "Delete",
    }
}

/// Build a lookup from (client_id, operation_id) to operation kind.
/// Used to color-code response arrows by the operation they answer.
fn build_op_map(log: &[LogEntry]) -> HashMap<(u8, u64), &'static str> {
    let mut map = HashMap::new();
    for entry in log {
        let msg = match entry {
            LogEntry::Send { message, .. } => message,
            _ => continue,
        };
        if let MessagePayload::ClientRequest {
            operation_id,
            ref operation,
        } = msg.payload
        {
            map.insert((client_id_of(msg), operation_id), op_kind(operation));
        }
    }
    map
}

fn request_label(op: &Operation) -> String {
    match op {
        Operation::Put { key, value } => format!("Put {key}={value}"),
        Operation::Get { key } => format!("Get {key}"),
        Operation::Delete { key } => format!("Del {key}"),
    }
}

fn response_label(result: &OperationResult) -> String {
    match &result.0 {
        Some(v) => format!("\u{2192} {v}"),
        None => "\u{2192} None".into(),
    }
}

fn request_detail(op: &Operation) -> String {
    match op {
        Operation::Put { key, value } => format!("Put key={key} value={value}"),
        Operation::Get { key } => format!("Get key={key}"),
        Operation::Delete { key } => format!("Delete key={key}"),
    }
}

fn response_detail(result: &OperationResult) -> String {
    match &result.0 {
        Some(v) => format!("Result: {v}"),
        None => "Result: None".into(),
    }
}

/// Escape a string for embedding in a JSON string literal.
fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

/// Extract JSON fields common to both Send and Deliver entries.
fn payload_fields(
    msg: &Message,
    op_map: &HashMap<(u8, u64), &'static str>,
) -> (String, String, u64, u8, String) {
    let cid = client_id_of(msg);
    match &msg.payload {
        MessagePayload::ClientRequest {
            operation_id,
            operation,
        } => {
            let kind = op_kind(operation);
            (
                format!("{kind}Req"),
                request_label(operation),
                *operation_id,
                cid,
                request_detail(operation),
            )
        }
        MessagePayload::ClientResponse {
            operation_id,
            result,
        } => {
            let kind = op_map
                .get(&(cid, *operation_id))
                .copied()
                .unwrap_or("?");
            (
                format!("{kind}Resp"),
                response_label(result),
                *operation_id,
                cid,
                response_detail(result),
            )
        }
    }
}

fn entry_json(e: &LogEntry, op_map: &HashMap<(u8, u64), &'static str>) -> Option<String> {
    match e {
        LogEntry::TickAll { .. } => None,
        LogEntry::Send {
            at,
            deliver_at,
            message: msg,
        } => {
            let (msg_type, label, op_id, cid, detail) = payload_fields(msg, op_map);
            Some(format!(
                r#"{{"kind":"send","at":{at},"deliver_at":{deliver_at},"from":"{f}","to":"{t}","msgType":"{msg_type}","label":"{label}","opId":{op_id},"clientId":{cid},"detail":"{detail}"}}"#,
                f = actor_name(&msg.from),
                t = actor_name(&msg.to),
                label = json_escape(&label),
                detail = json_escape(&detail),
            ))
        }
        LogEntry::Deliver { at, msg } => {
            let (msg_type, label, op_id, cid, detail) = payload_fields(msg, op_map);
            Some(format!(
                r#"{{"kind":"deliver","at":{at},"from":"{f}","to":"{t}","msgType":"{msg_type}","label":"{label}","opId":{op_id},"clientId":{cid},"detail":"{detail}"}}"#,
                f = actor_name(&msg.from),
                t = actor_name(&msg.to),
                label = json_escape(&label),
                detail = json_escape(&detail),
            ))
        }
    }
}

fn scenario_json(name: &str, sim: &Simulator) -> String {
    let op_map = build_op_map(sim.log());

    let mut actors = Vec::new();
    for id in &sim.client_ids() {
        actors.push(format!("Client {}", id.0));
    }
    actors.push("Server".into());

    let actor_list = actors
        .iter()
        .map(|a| format!("\"{a}\""))
        .collect::<Vec<_>>()
        .join(",");
    let entries: Vec<String> = sim
        .log()
        .iter()
        .filter_map(|e| entry_json(e, &op_map))
        .collect();

    let history = sim.history();
    let total = history.entries().len();
    let client_summaries: Vec<String> = sim
        .client_ids()
        .iter()
        .map(|cid| {
            let count = history
                .entries()
                .iter()
                .filter(|e| e.client_id == *cid)
                .count();
            format!(r#"{{"id":"Client {}","ops":{count}}}"#, cid.0)
        })
        .collect();

    format!(
        r#"{{"name":"{name}","actors":[{actor_list}],"entries":[{e}],"result":{{"total":{total},"clients":[{c}]}}}}"#,
        e = entries.join(","),
        c = client_summaries.join(","),
    )
}

fn main() {
    let all = scenarios::all();
    let jsons: Vec<String> = all
        .iter()
        .map(|s| scenario_json(s.name, &s.sim))
        .collect();
    let json = format!("[{}]", jsons.join(",\n"));
    let html = HTML_TEMPLATE.replace("__DATA__", &json);

    std::fs::create_dir_all("target").ok();
    let path = std::path::Path::new("target/visualize.html");
    std::fs::write(path, &html).expect("Failed to write HTML");

    let abs = std::fs::canonicalize(path).unwrap();
    eprintln!("Written to {}", abs.display());

    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(&abs).spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(&abs).spawn();
    }
}

const HTML_TEMPLATE: &str = include_str!("visualize.html");
