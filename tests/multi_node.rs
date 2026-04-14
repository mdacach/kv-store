//! Property-based tests for Phase 2: uncoordinated multi-node routing.
//!
//! Verifies that the simulation runs to completion under randomized workloads.

use proptest::prelude::*;
use proptest::property_test;

use kv_store::simulator::Simulator;
use kv_store::{ClientID, Key, Operation, Value};

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

fn generate_operations(max_operations: usize) -> impl Strategy<Value = Vec<Operation>> {
    prop::collection::vec(generate_single_operation(), 1..=max_operations)
}

fn generate_client_workloads(
    max_clients: usize,
    max_ops_per_client: usize,
) -> impl Strategy<Value = Vec<Vec<Operation>>> {
    prop::collection::vec(generate_operations(max_ops_per_client), 1..=max_clients)
}

#[property_test]
fn all_operations_complete_single_client(
    seed: u64,
    #[strategy = 1..=10u64] max_delivery_delay: u64,
    #[strategy = generate_operations(30)] operations: Vec<Operation>,
) -> Result<(), TestCaseError> {
    let num_ops = operations.len();
    let mut sim = Simulator::new(seed, 1..(max_delivery_delay + 1));
    sim.register_client(ClientID(0), operations);
    sim.schedule_tick_all(0);
    sim.run();

    prop_assert!(
        sim.all_clients_done(),
        "client should have completed all ops\nlog:\n{}",
        sim.format_log(),
    );

    let history = sim.history();
    prop_assert!(history.all_returned(), "all operations should have returned");
    prop_assert_eq!(history.entries().len(), num_ops);
    for entry in history.entries() {
        prop_assert!(entry.invoke_time <= entry.return_time);
    }

    Ok(())
}

#[property_test]
fn all_operations_complete_multiple_clients(
    seed: u64,
    #[strategy = 1..=10u64] max_delivery_delay: u64,
    #[strategy = generate_client_workloads(5, 20)] workloads: Vec<Vec<Operation>>,
) -> Result<(), TestCaseError> {
    let total_ops: usize = workloads.iter().map(|w| w.len()).sum();
    let mut sim = Simulator::new(seed, 1..(max_delivery_delay + 1));
    for (i, ops) in workloads.into_iter().enumerate() {
        sim.register_client(ClientID(i as u8), ops);
    }
    sim.schedule_tick_all(0);
    sim.run();

    prop_assert!(
        sim.all_clients_done(),
        "all clients should have completed all ops\nlog:\n{}",
        sim.format_log(),
    );

    let history = sim.history();
    prop_assert!(history.all_returned(), "all operations should have returned");
    prop_assert_eq!(history.entries().len(), total_ops);
    for entry in history.entries() {
        prop_assert!(entry.invoke_time <= entry.return_time);
    }

    Ok(())
}
