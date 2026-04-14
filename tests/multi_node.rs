//! Property-based tests for Phase 2: uncoordinated multi-node routing.
//!
//! Verifies that the simulation runs to completion under randomized workloads.

use proptest::prelude::*;
use proptest::property_test;

use kv_store::simulator::Simulator;
use kv_store::{ClientID, Key, Request, Value};

/// Generate a random key-value request.
fn generate_single_request() -> impl Strategy<Value = Request> {
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
            Just(Request::Put {
                key: k.clone(),
                value: v
            }),
            Just(Request::Get { key: k.clone() }),
            Just(Request::Delete { key: k }),
        ]
    })
}

fn generate_requests(max_requests: usize) -> impl Strategy<Value = Vec<Request>> {
    prop::collection::vec(generate_single_request(), 1..=max_requests)
}

fn generate_client_workloads(
    max_clients: usize,
    max_requests_per_client: usize,
) -> impl Strategy<Value = Vec<Vec<Request>>> {
    prop::collection::vec(generate_requests(max_requests_per_client), 1..=max_clients)
}

#[property_test]
fn all_requests_complete_single_client(
    seed: u64,
    #[strategy = 1..=10u64] max_delivery_delay: u64,
    #[strategy = generate_requests(30)] requests: Vec<Request>,
) -> Result<(), TestCaseError> {
    let num_requests = requests.len();
    let mut sim = Simulator::new(seed, 1..(max_delivery_delay + 1));
    sim.register_client(ClientID(0), requests);
    sim.schedule_tick_all(0);
    sim.run();

    prop_assert!(
        sim.all_clients_done(),
        "client should have completed all requests\nlog:\n{}",
        sim.format_log(),
    );

    let history = sim.request_history();
    prop_assert!(
        history.all_responded(),
        "all requests should have received responses"
    );
    prop_assert_eq!(history.entries().len(), num_requests);
    for entry in history.entries() {
        prop_assert!(entry.invoke_time <= entry.return_time);
    }

    Ok(())
}

#[property_test]
fn all_requests_complete_multiple_clients(
    seed: u64,
    #[strategy = 1..=10u64] max_delivery_delay: u64,
    #[strategy = generate_client_workloads(5, 20)] workloads: Vec<Vec<Request>>,
) -> Result<(), TestCaseError> {
    let total_requests: usize = workloads.iter().map(|w| w.len()).sum();
    let mut sim = Simulator::new(seed, 1..(max_delivery_delay + 1));
    for (i, requests) in workloads.into_iter().enumerate() {
        sim.register_client(ClientID(i as u8), requests);
    }
    sim.schedule_tick_all(0);
    sim.run();

    prop_assert!(
        sim.all_clients_done(),
        "all clients should have completed all requests\nlog:\n{}",
        sim.format_log(),
    );

    let history = sim.request_history();
    prop_assert!(
        history.all_responded(),
        "all requests should have received responses"
    );
    prop_assert_eq!(history.entries().len(), total_requests);
    for entry in history.entries() {
        prop_assert!(entry.invoke_time <= entry.return_time);
    }

    Ok(())
}
