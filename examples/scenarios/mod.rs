//! Pre-built scenarios for the trace visualizer.
//!
//! Each scenario constructs a [`Simulator`], registers client workloads,
//! runs to completion, and returns the finished simulation for visualization.

use kv_store::simulator::Simulator;
use kv_store::{ClientID, Key, Node, NodeID, Operation, Server, Value};

/// A named, already-executed simulation scenario.
pub struct Scenario {
    pub name: &'static str,
    pub sim: Simulator,
}

/// All demonstration scenarios, in display order.
pub fn all() -> Vec<Scenario> {
    vec![
        single_client_crud(),
        two_clients_racing(),
        five_clients_concurrent(),
        sequential_no_delay(),
    ]
}

fn key(s: &str) -> Key {
    Key(s.into())
}
fn val(s: &str) -> Value {
    Value(s.into())
}

/// Single client performing Put, Get, Delete on one key.
/// Moderate delivery delay (1..3) gives a spread-out timeline.
fn single_client_crud() -> Scenario {
    let server = Server::new(Node::new(NodeID(0)));
    let mut sim = Simulator::new(server, 42, 1..3);
    sim.register_client(
        ClientID(0),
        vec![
            Operation::Put {
                key: key("x"),
                value: val("1"),
            },
            Operation::Get { key: key("x") },
            Operation::Delete { key: key("x") },
        ],
    );
    sim.schedule_tick_all(0);
    sim.run();
    Scenario {
        name: "Single client \u{2014} Put, Get, Delete",
        sim,
    }
}

/// Two clients both writing and reading key "x", demonstrating how
/// the server serializes concurrent requests.
fn two_clients_racing() -> Scenario {
    let server = Server::new(Node::new(NodeID(0)));
    let mut sim = Simulator::new(server, 42, 1..3);
    sim.register_client(
        ClientID(0),
        vec![
            Operation::Put {
                key: key("x"),
                value: val("1"),
            },
            Operation::Get { key: key("x") },
            Operation::Delete { key: key("x") },
        ],
    );
    sim.register_client(
        ClientID(1),
        vec![
            Operation::Put {
                key: key("x"),
                value: val("2"),
            },
            Operation::Get { key: key("x") },
        ],
    );
    sim.schedule_tick_all(0);
    sim.run();
    Scenario {
        name: "Two clients \u{2014} racing writes on key x",
        sim,
    }
}

/// Five clients each operating on a different key. Demonstrates
/// the viewer layout with many actor lanes.
fn five_clients_concurrent() -> Scenario {
    let server = Server::new(Node::new(NodeID(0)));
    let mut sim = Simulator::new(server, 7, 1..5);
    let keys = ["a", "b", "c", "d", "e"];
    for (i, k) in keys.iter().enumerate() {
        sim.register_client(
            ClientID(i as u8),
            vec![
                Operation::Put {
                    key: key(k),
                    value: val(&format!("{}", i + 1)),
                },
                Operation::Get { key: key(k) },
            ],
        );
    }
    sim.schedule_tick_all(0);
    sim.run();
    Scenario {
        name: "Five clients \u{2014} concurrent workload",
        sim,
    }
}

/// Single client, five operations, fixed 1-tick delay.
/// Produces a clean staircase pattern showing stop-and-wait behavior.
fn sequential_no_delay() -> Scenario {
    let server = Server::new(Node::new(NodeID(0)));
    let mut sim = Simulator::new(server, 1, 1..2);
    sim.register_client(
        ClientID(0),
        vec![
            Operation::Put {
                key: key("a"),
                value: val("1"),
            },
            Operation::Put {
                key: key("a"),
                value: val("2"),
            },
            Operation::Get { key: key("a") },
            Operation::Delete { key: key("a") },
            Operation::Get { key: key("a") },
        ],
    );
    sim.schedule_tick_all(0);
    sim.run();
    Scenario {
        name: "Sequential \u{2014} fixed 1-tick delay",
        sim,
    }
}
