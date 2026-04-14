//! Pre-built scenarios for the trace visualizer.
//!
//! Each scenario constructs a [`Simulator`], registers client workloads,
//! runs to completion, and returns the finished simulation for visualization.

use kv_store::simulator::Simulator;
use kv_store::{ClientID, Key, Operation, Value};

/// A named, already-executed simulation scenario.
pub struct Scenario {
    pub name: &'static str,
    pub sim: Simulator,
}

/// All demonstration scenarios, in display order.
pub fn all() -> Vec<Scenario> {
    vec![
        routed_stale_read(),
        single_client_misses_own_write(),
        two_clients_diverge(),
        five_clients_concurrent(),
        sequential_fixed_delay(),
    ]
}

fn key(s: &str) -> Key {
    Key(s.into())
}
fn val(s: &str) -> Value {
    Value(s.into())
}

fn single_client_misses_own_write() -> Scenario {
    let mut sim = Simulator::new(42, 1..3);
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
        name: "Single client - routed across nodes",
        sim,
    }
}

fn two_clients_diverge() -> Scenario {
    let mut sim = Simulator::new(42, 1..3);
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
        name: "Two clients - divergent node views",
        sim,
    }
}

/// Five clients each operating on a different key. Demonstrates
/// the viewer layout with many actor lanes.
fn five_clients_concurrent() -> Scenario {
    let mut sim = Simulator::new(7, 1..5);
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
        name: "Five clients - concurrent workload",
        sim,
    }
}

fn sequential_fixed_delay() -> Scenario {
    let mut sim = Simulator::new(1, 1..2);
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
        name: "Sequential - fixed 1-tick delay",
        sim,
    }
}

fn routed_stale_read() -> Scenario {
    let mut sim = Simulator::new(1, 1..4);
    sim.register_client(
        ClientID(0),
        vec![Operation::Put {
            key: key("x"),
            value: val("1"),
        }],
    );
    sim.register_client(
        ClientID(1),
        vec![
            Operation::Get { key: key("y") },
            Operation::Get { key: key("x") },
        ],
    );
    sim.schedule_tick_all(0);
    sim.run();
    Scenario {
        name: "Seeded stale read from routed nodes",
        sim,
    }
}
