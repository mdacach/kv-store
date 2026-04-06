//! Message-flow trace visualization.
//!
//! Renders the simulator's action log as an interactive HTML page showing
//! Send/Deliver arrows between actors on a timeline. Color-coded by
//! operation type (Put/Get/Delete), with step-by-step playback controls
//! and hover tooltips.

use std::collections::HashMap;
use std::fmt::Write;

use crate::simulator::{LogEntry, Simulator};
use crate::{ActorId, Message, MessagePayload, Operation, OperationResult};

const HTML_TEMPLATE: &str = include_str!("trace.html");

/// A named simulation scenario for trace rendering.
pub struct Scenario<'a> {
    pub name: &'a str,
    pub sim: &'a Simulator,
}

/// Render one or more scenarios as a self-contained interactive HTML page.
pub fn render(scenarios: &[Scenario<'_>]) -> String {
    let jsons: Vec<String> = scenarios
        .iter()
        .map(|s| scenario_json(s.name, s.sim))
        .collect();
    let json = format!("[{}]", jsons.join(",\n"));
    HTML_TEMPLATE.replace("__DATA__", &json)
}

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

    let mut out = String::new();
    write!(
        out,
        r#"{{"name":"{name}","actors":[{actor_list}],"entries":[{e}],"result":{{"total":{total},"clients":[{c}]}}}}"#,
        e = entries.join(","),
        c = client_summaries.join(","),
    )
    .unwrap();
    out
}
