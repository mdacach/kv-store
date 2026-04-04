//! Property-based tests for Phase 1: single-node KV store.
//!
//! Verifies that the simulation runs to completion and all client operations
//! are serviced under random delivery delays and workloads.

use proptest::prelude::*;
use proptest::property_test;

use kv_store::node::Operation;
use kv_store::simulator::Simulator;
use kv_store::{ClientID, Key, Node, NodeID, Server, Value};

/// Generate a random key-value operation.
///
/// Keys and values are drawn from small pools ("a"/"b"/"c" and "1"/"2"/"3")
/// so that operations frequently collide on the same key, exercising
/// read-after-write and overwrite behavior.
fn generate_single_operation() -> impl Strategy<Value = Operation> {
    let key = prop_oneof![
        Just(Key("a".into())),
        Just(Key("b".into())),
        Just(Key("c".into())),
    ];
    let value = prop_oneof![
        Just(Value("1".into())),
        Just(Value("2".into())),
        Just(Value("3".into())),
    ];

    (key, value).prop_flat_map(|(k, v)| {
        prop_oneof![
            Just(Operation::Put {
                key: k.clone(),
                value: v
            }),
            Just(Operation::Get { key: k.clone() }),
            Just(Operation::Delete { key: k }),
        ]
    })
}

/// Generate a variable-length sequence of random operations.
fn generate_operations(max_operations: usize) -> impl Strategy<Value = Vec<Operation>> {
    prop::collection::vec(generate_single_operation(), 1..=max_operations)
}

/// Generate workloads for 1..=max_clients clients, each with up to
/// max_ops_per_client operations.
fn generate_client_workloads(
    max_clients: usize,
    max_ops_per_client: usize,
) -> impl Strategy<Value = Vec<Vec<Operation>>> {
    prop::collection::vec(generate_operations(max_ops_per_client), 1..=max_clients)
}

#[property_test]
fn all_operations_complete_single_client(
    seed: u64,
    #[strategy = 0..=10u64] max_delivery_delay: u64,
    #[strategy = generate_operations(30)] operations: Vec<Operation>,
) -> Result<(), TestCaseError> {
    let server = Server::new(Node::new(NodeID(0)));
    let mut sim = Simulator::new(server, seed, 0..max_delivery_delay);
    sim.register_client(ClientID(0), operations);
    sim.schedule_tick_all(0);
    sim.run();

    prop_assert!(
        sim.all_clients_done(),
        "Client should have completed all ops.\nLog:\n{}",
        sim.format_log(),
    );
    Ok(())
}

#[property_test]
fn all_operations_complete_multiple_clients(
    seed: u64,
    #[strategy = 0..=10u64] max_delivery_delay: u64,
    #[strategy = generate_client_workloads(5, 20)] workloads: Vec<Vec<Operation>>,
) -> Result<(), TestCaseError> {
    let server = Server::new(Node::new(NodeID(0)));
    let mut sim = Simulator::new(server, seed, 0..max_delivery_delay);
    for (i, ops) in workloads.into_iter().enumerate() {
        sim.register_client(ClientID(i as u8), ops);
    }
    sim.schedule_tick_all(0);
    sim.run();

    prop_assert!(
        sim.all_clients_done(),
        "All clients should have completed all ops.\nLog:\n{}",
        sim.format_log(),
    );
    Ok(())
}

/// Fixed-seed trace for visual inspection.
/// Run with `cargo test example_trace -- --nocapture` to see the log.
#[test]
fn example_trace() {
    let server = Server::new(Node::new(NodeID(0)));
    let mut sim = Simulator::new(server, 42, 0..3);
    sim.register_client(
        ClientID(0),
        vec![
            Operation::Put {
                key: Key("x".into()),
                value: Value("1".into()),
            },
            Operation::Get {
                key: Key("x".into()),
            },
            Operation::Delete {
                key: Key("x".into()),
            },
        ],
    );
    sim.register_client(
        ClientID(1),
        vec![
            Operation::Put {
                key: Key("x".into()),
                value: Value("2".into()),
            },
            Operation::Get {
                key: Key("x".into()),
            },
        ],
    );
    sim.schedule_tick_all(0);
    sim.run();

    println!("{}", sim.format_log());
    assert!(sim.all_clients_done());
}
