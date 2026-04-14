//! Message-flow trace visualization.
//!
//! Renders the simulator's event log as an interactive HTML page showing
//! Send/Deliver arrows between actors on a timeline. Color-coded by
//! request type (Put/Get/Delete), with step-by-step playback controls
//! and hover tooltips.

use std::collections::HashMap;
use std::fmt::Write;

use crate::simulator::{EventEntry, Simulator};
use crate::{ActorId, Message, MessagePayload, Request, Response};

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

/// Display name for an actor.
fn actor_name(a: &ActorId) -> String {
    match a {
        ActorId::Node(id) => format!("Node {}", id.0),
        ActorId::Client(id) => format!("Client {}", id.0),
    }
}

fn boundary_client_id_of(msg: &Message) -> Option<u8> {
    match (&msg.from, &msg.to) {
        (ActorId::Client(id), _) | (_, ActorId::Client(id)) => Some(id.0),
        _ => None,
    }
}

fn request_kind(request: &Request) -> &'static str {
    match request {
        Request::Put { .. } => "Put",
        Request::Get { .. } => "Get",
        Request::Delete { .. } => "Delete",
    }
}

/// Build a lookup from a client request key to request kind.
fn build_request_map(log: &[EventEntry]) -> HashMap<(u8, u64), &'static str> {
    let mut map = HashMap::new();
    for entry in log {
        let msg = match entry {
            EventEntry::Send { message, .. } => message,
            _ => continue,
        };
        if let MessagePayload::ClientRequest {
            request_id,
            ref request,
        } = msg.payload
            && let Some(client_id) = boundary_client_id_of(msg)
        {
            map.insert((client_id, request_id), request_kind(request));
        }
    }
    map
}

fn request_label(request: &Request) -> String {
    match request {
        Request::Put { key, value } => format!("Put {key}={value}"),
        Request::Get { key } => format!("Get {key}"),
        Request::Delete { key } => format!("Del {key}"),
    }
}

fn response_label(response: &Response) -> String {
    match &response.0 {
        Some(v) => format!("\u{2192} {v}"),
        None => "\u{2192} None".into(),
    }
}

fn request_detail(request: &Request) -> String {
    match request {
        Request::Put { key, value } => format!("Put key={key} value={value}"),
        Request::Get { key } => format!("Get key={key}"),
        Request::Delete { key } => format!("Delete key={key}"),
    }
}

fn response_detail(response: &Response) -> String {
    match &response.0 {
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
    request_map: &HashMap<(u8, u64), &'static str>,
) -> (String, String, u64, u64, String) {
    let client_id = boundary_client_id_of(msg)
        .expect("all runtime messages should include a client as sender or recipient");
    match &msg.payload {
        MessagePayload::ClientRequest {
            request_id,
            request,
        } => {
            let kind = request_kind(request);
            (
                format!("{kind}Req"),
                request_label(request),
                *request_id,
                u64::from(client_id),
                request_detail(request),
            )
        }
        MessagePayload::ClientResponse {
            request_id,
            response,
        } => {
            let kind = request_map
                .get(&(client_id, *request_id))
                .copied()
                .unwrap_or("?");
            (
                format!("{kind}Resp"),
                response_label(response),
                *request_id,
                u64::from(client_id),
                response_detail(response),
            )
        }
    }
}

fn entry_json(e: &EventEntry, request_map: &HashMap<(u8, u64), &'static str>) -> Option<String> {
    match e {
        EventEntry::TickAll { .. } => None,
        EventEntry::Send {
            at,
            deliver_at,
            message: msg,
        } => {
            let (msg_type, label, request_id, cid, detail) = payload_fields(msg, request_map);
            Some(format!(
                r#"{{"kind":"send","at":{at},"deliver_at":{deliver_at},"from":"{f}","to":"{t}","msgType":"{msg_type}","label":"{label}","requestId":{request_id},"clientId":{cid},"detail":"{detail}"}}"#,
                f = actor_name(&msg.from),
                t = actor_name(&msg.to),
                label = json_escape(&label),
                detail = json_escape(&detail),
            ))
        }
        EventEntry::Deliver { at, msg } => {
            let (msg_type, label, request_id, cid, detail) = payload_fields(msg, request_map);
            Some(format!(
                r#"{{"kind":"deliver","at":{at},"from":"{f}","to":"{t}","msgType":"{msg_type}","label":"{label}","requestId":{request_id},"clientId":{cid},"detail":"{detail}"}}"#,
                f = actor_name(&msg.from),
                t = actor_name(&msg.to),
                label = json_escape(&label),
                detail = json_escape(&detail),
            ))
        }
    }
}

fn scenario_json(name: &str, sim: &Simulator) -> String {
    let request_map = build_request_map(sim.event_log().entries());

    let mut actors = Vec::new();
    for id in &sim.client_ids() {
        actors.push(format!("Client {}", id.0));
    }
    for id in &sim.node_ids() {
        actors.push(format!("Node {}", id.0));
    }

    let actor_list = actors
        .iter()
        .map(|a| format!("\"{a}\""))
        .collect::<Vec<_>>()
        .join(",");
    let entries: Vec<String> = sim
        .event_log()
        .entries()
        .iter()
        .filter_map(|e| entry_json(e, &request_map))
        .collect();

    let history = sim.request_history();
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
            format!(r#"{{"id":"Client {}","requests":{count}}}"#, cid.0)
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
