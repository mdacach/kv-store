//! HTML visualization of linearizability check results.
//!
//! Produces a self-contained HTML page with swim lanes per client,
//! requests as colored rectangles, linearization point markers,
//! and hover tooltips showing reference state transitions.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use crate::analysis::history::HistoryEntry;
use crate::analysis::linearizability::{CheckResult, apply_to_reference};
use crate::kv::{Key, Value};

const HTML_TEMPLATE: &str = include_str!("linearizability.html");

/// Layout constants.
const LANE_HEIGHT: f64 = 50.0;
const LANE_GAP: f64 = 20.0;
const LANE_LABEL_WIDTH: f64 = 80.0;
const TOP_MARGIN: f64 = 40.0;
const RIGHT_MARGIN: f64 = 40.0;
const REQUEST_HEIGHT: f64 = 30.0;
const MIN_REQUEST_WIDTH: f64 = 8.0;
const TIME_SCALE: f64 = 60.0;
const LP_RADIUS: f64 = 5.0;
const LP_MARKER_OFFSET: f64 = 2.0;

/// Generate a self-contained HTML visualization.
pub fn visualize(entries: &[HistoryEntry], result: &CheckResult) -> String {
    let mut svg = String::with_capacity(4096 + entries.len() * 256);

    let clients = collect_clients(entries);
    let time_max = entries.iter().map(|e| e.return_time).max().unwrap_or(0) as f64;
    let svg_width = LANE_LABEL_WIDTH + time_max * TIME_SCALE + RIGHT_MARGIN;
    let svg_height = TOP_MARGIN + clients.len() as f64 * (LANE_HEIGHT + LANE_GAP);

    let (lp_order, lp_states) = compute_linearization_points(entries, result);

    // Lookup table: entry index -> position in the linearization (if any).
    let mut lp_position: Vec<Option<usize>> = vec![None; entries.len()];
    for (pos, &idx) in lp_order.iter().enumerate() {
        lp_position[idx] = Some(pos);
    }

    write!(
        svg,
        r#"<svg width="{svg_width}" height="{svg_height}" xmlns="http://www.w3.org/2000/svg">"#
    )
    .unwrap();

    write_time_axis(&mut svg, time_max, svg_height);

    for (lane_idx, &client_id) in clients.iter().enumerate() {
        let lane_y = lane_y(lane_idx);
        write_lane_label(&mut svg, client_id, lane_y);

        for (entry_idx, entry) in entries.iter().enumerate() {
            if entry.client_id != client_id {
                continue;
            }
            write_request(
                &mut svg,
                entry,
                entry_idx,
                lane_y,
                lp_position[entry_idx],
                &lp_states,
                result,
            );
        }
    }

    write_lp_lines(&mut svg, entries, &clients, &lp_order);
    svg.push_str("</svg>\n");

    HTML_TEMPLATE
        .replace("__SVG__", &svg)
        .replace("__LEGEND__", &legend_html(result))
        .replace("__SUMMARY__", &summary_html(result))
        .replace("__SCRIPT__", TOOLTIP_SCRIPT)
}

fn lane_y(lane_idx: usize) -> f64 {
    TOP_MARGIN + lane_idx as f64 * (LANE_HEIGHT + LANE_GAP)
}

fn collect_clients(entries: &[HistoryEntry]) -> Vec<crate::ClientID> {
    let mut clients: BTreeSet<crate::ClientID> = BTreeSet::new();
    for e in entries {
        clients.insert(e.client_id);
    }
    clients.into_iter().collect()
}

/// Returns (linearization order as indices, reference states after each step).
fn compute_linearization_points(
    entries: &[HistoryEntry],
    result: &CheckResult,
) -> (Vec<usize>, Vec<BTreeMap<Key, Value>>) {
    let indexed_entries = match result {
        CheckResult::Ok { linearization } => linearization.as_slice(),
        CheckResult::Violation {
            linearized_prefix, ..
        } => linearized_prefix.as_slice(),
    };

    let order: Vec<usize> = indexed_entries.iter().map(|ie| ie.index).collect();

    let mut states = Vec::with_capacity(order.len());
    let mut state = BTreeMap::new();
    for &idx in &order {
        apply_to_reference(&mut state, &entries[idx].request);
        states.push(state.clone());
    }

    (order, states)
}

fn time_to_x(t: u64) -> f64 {
    LANE_LABEL_WIDTH + t as f64 * TIME_SCALE
}

fn lane_center(lane_y: f64) -> f64 {
    lane_y + LANE_HEIGHT / 2.0
}

fn format_state(state: &BTreeMap<Key, Value>) -> String {
    if state.is_empty() {
        return "{}".to_string();
    }
    let pairs: Vec<String> = state.iter().map(|(k, v)| format!("{k}: \"{v}\"")).collect();
    format!("{{{}}}", pairs.join(", "))
}

fn write_time_axis(html: &mut String, time_max: f64, svg_height: f64) {
    let max_t = time_max as u64;
    for t in 0..=max_t {
        let x = time_to_x(t);
        write!(
            html,
            r#"<line x1="{x}" y1="{}" x2="{x}" y2="{svg_height}" class="time-line"/>"#,
            TOP_MARGIN - 5.0,
        )
        .unwrap();
        write!(
            html,
            r#"<text x="{x}" y="{}" text-anchor="middle" class="time-label">t={t}</text>"#,
            TOP_MARGIN - 10.0,
        )
        .unwrap();
    }
}

fn write_lane_label(html: &mut String, client_id: crate::ClientID, lane_y: f64) {
    let label_y = lane_center(lane_y) + 4.0;
    write!(
        html,
        r#"<text x="5" y="{label_y}" class="lane-label">{client_id}</text>"#,
    )
    .unwrap();
    let lx = LANE_LABEL_WIDTH;
    write!(
        html,
        r##"<rect x="{lx}" y="{lane_y}" width="100%" height="{LANE_HEIGHT}" fill="#f8fafc" rx="3"/>"##,
    )
    .unwrap();
}

fn write_request(
    html: &mut String,
    entry: &HistoryEntry,
    entry_idx: usize,
    lane_y: f64,
    lp_pos: Option<usize>,
    lp_states: &[BTreeMap<Key, Value>],
    result: &CheckResult,
) {
    let x1 = time_to_x(entry.invoke_time);
    let x2 = time_to_x(entry.return_time);
    let width = (x2 - x1).max(MIN_REQUEST_WIDTH);
    let y = lane_y + (LANE_HEIGHT - REQUEST_HEIGHT) / 2.0;
    let center_x = x1 + width / 2.0;
    let marker_y = y - LP_MARKER_OFFSET;

    let (fill, stroke) = if lp_pos.is_some() {
        ("#dbeafe", "#93c5fd")
    } else {
        ("#fee2e2", "#fca5a5")
    };

    let mut tooltip = format!(
        "{} {} -> {}\nt={}..{}",
        entry.client_id, entry.request, entry.response, entry.invoke_time, entry.return_time,
    );
    if let Some(pos) = lp_pos {
        let prev_state = if pos > 0 {
            format_state(&lp_states[pos - 1])
        } else {
            "{}".to_string()
        };
        let new_state = format_state(&lp_states[pos]);
        write!(
            tooltip,
            "\n\nLinearization point #{}\nState before: {prev_state}\nState after: {new_state}",
            pos + 1
        )
        .unwrap();
    } else if matches!(result, CheckResult::Violation { .. }) {
        tooltip.push_str("\n\n(not linearized)");
    }
    let tooltip_escaped = tooltip.replace('"', "&quot;");

    write!(
        html,
        r#"<rect x="{x1}" y="{y}" width="{width}" height="{REQUEST_HEIGHT}" rx="4" fill="{fill}" stroke="{stroke}" stroke-width="1.5" class="request-rect" data-tooltip="{tooltip_escaped}"/>"#,
    )
    .unwrap();

    let label_y = y + REQUEST_HEIGHT / 2.0 + 4.0;
    let request = &entry.request;
    write!(
        html,
        r#"<text x="{center_x}" y="{label_y}" text-anchor="middle" class="request-label">{request}</text>"#,
    )
    .unwrap();

    if lp_pos.is_some() {
        write!(
            html,
            r#"<circle cx="{center_x}" cy="{marker_y}" r="{LP_RADIUS}" class="lp-valid"/>"#,
        )
        .unwrap();
    }

    if let CheckResult::Violation {
        failed_candidates, ..
    } = result
        && failed_candidates.iter().any(|fc| fc.index == entry_idx)
    {
        write!(
            html,
            r#"<circle cx="{center_x}" cy="{marker_y}" r="{LP_RADIUS}" class="lp-invalid"/>"#,
        )
        .unwrap();
    }
}

fn write_lp_lines(
    html: &mut String,
    entries: &[HistoryEntry],
    clients: &[crate::ClientID],
    lp_order: &[usize],
) {
    if lp_order.len() < 2 {
        return;
    }

    let lane_of = |client_id: crate::ClientID| -> f64 {
        let lane_idx = clients.iter().position(|&c| c == client_id).unwrap();
        lane_y(lane_idx) + (LANE_HEIGHT - REQUEST_HEIGHT) / 2.0 - LP_MARKER_OFFSET
    };

    for window in lp_order.windows(2) {
        let ea = &entries[window[0]];
        let eb = &entries[window[1]];

        let xa = request_center_x(ea);
        let xb = request_center_x(eb);
        let ya = lane_of(ea.client_id);
        let yb = lane_of(eb.client_id);

        write!(
            html,
            r#"<line x1="{xa}" y1="{ya}" x2="{xb}" y2="{yb}" class="lp-line lp-valid" stroke-dasharray="4,3" opacity="0.4"/>"#,
        )
        .unwrap();
    }
}

fn request_center_x(entry: &HistoryEntry) -> f64 {
    let x1 = time_to_x(entry.invoke_time);
    let x2 = time_to_x(entry.return_time);
    let width = (x2 - x1).max(MIN_REQUEST_WIDTH);
    x1 + width / 2.0
}

fn legend_html(result: &CheckResult) -> String {
    let mut html = String::new();
    html.push_str(r#"<div class="legend">"#);
    html.push_str(
        r#"<span><span class="dot" style="background:#2563eb"></span> Linearization point</span>"#,
    );
    if result.is_violation() {
        html.push_str(
            r#"<span><span class="dot" style="background:#dc2626"></span> Failed candidate</span>"#,
        );
        html.push_str(r#"<span><span class="dot" style="background:#dbeafe;border:1px solid #93c5fd"></span> Linearized</span>"#);
        html.push_str(r#"<span><span class="dot" style="background:#fee2e2;border:1px solid #fca5a5"></span> Not linearized</span>"#);
    }
    html.push_str("</div>\n");
    html
}

fn summary_html(result: &CheckResult) -> String {
    let mut html = String::new();
    html.push_str(r#"<pre style="margin-top:12px;font-size:13px;color:#334155;">"#);
    match result {
        CheckResult::Ok { linearization } => {
            write!(
                html,
                "Result: Linearizable ({} requests)",
                linearization.len()
            )
            .unwrap();
        }
        CheckResult::Violation {
            linearized_prefix,
            state_at_failure,
            failed_candidates,
        } => {
            write!(
                html,
                "Result: Violation (linearized {} requests before failure)\nReference state at failure: {}\nFailed candidates: {}",
                linearized_prefix.len(),
                format_state(state_at_failure),
                failed_candidates.len(),
            )
            .unwrap();
        }
    }
    html.push_str("</pre>\n");
    html
}

const TOOLTIP_SCRIPT: &str = r##"const tooltip = document.getElementById('tooltip');
document.querySelectorAll('.request-rect').forEach(rect => {
  rect.addEventListener('mouseenter', e => {
    tooltip.textContent = rect.getAttribute('data-tooltip');
    tooltip.style.display = 'block';
  });
  rect.addEventListener('mousemove', e => {
    tooltip.style.left = (e.clientX + 12) + 'px';
    tooltip.style.top = (e.clientY + 12) + 'px';
  });
  rect.addEventListener('mouseleave', () => {
    tooltip.style.display = 'none';
  });
});
"##;
