//! Seeded regression tests for notable phase-2 behaviors.

use kv_store::simulator::Simulator;
use kv_store::{ClientID, Key, Request, Value};

#[test]
fn simulator_is_deterministic_for_fixed_seed() {
    let workload_0 = vec![
        Request::Put {
            key: Key("x".into()),
            value: Value("1".into()),
        },
        Request::Get {
            key: Key("x".into()),
        },
        Request::Delete {
            key: Key("x".into()),
        },
    ];
    let workload_1 = vec![
        Request::Put {
            key: Key("y".into()),
            value: Value("2".into()),
        },
        Request::Get {
            key: Key("x".into()),
        },
    ];

    let mut left = Simulator::new(42, 1..4);
    left.register_client(ClientID(0), workload_0.clone());
    left.register_client(ClientID(1), workload_1.clone());
    left.schedule_tick_all(0);
    left.run();

    let mut right = Simulator::new(42, 1..4);
    right.register_client(ClientID(0), workload_0);
    right.register_client(ClientID(1), workload_1);
    right.schedule_tick_all(0);
    right.run();

    assert_eq!(left.clock(), right.clock());
    assert_eq!(left.format_log(), right.format_log());
    assert_eq!(left.request_history().entries(), right.request_history().entries());
    assert_eq!(left.check_linearizable(), right.check_linearizable());
}

#[test]
fn client_can_miss_its_own_write_due_to_rerouting() {
    let mut sim = Simulator::with_node_count(3, 0, 1..4);
    sim.register_client(
        ClientID(0),
        vec![
            Request::Put {
                key: Key("x".into()),
                value: Value("1".into()),
            },
            Request::Get {
                key: Key("x".into()),
            },
        ],
    );
    sim.schedule_tick_all(0);
    sim.run();

    let entries = sim.request_history().entries();
    assert_eq!(entries.len(), 2, "{}", sim.format_log());
    assert_eq!(entries[0].response.0, None);
    assert_eq!(entries[1].response.0, None, "{}", sim.format_log());
    assert_ne!(
        sim.routed_node(ClientID(0), 0),
        sim.routed_node(ClientID(0), 1),
        "the stable stale-read scenario should route the two requests to different nodes",
    );
}

#[test]
fn seeded_uncoordinated_routing_can_violate_linearizability() {
    let mut sim = Simulator::new(1, 1..4);
    sim.register_client(
        ClientID(0),
        vec![Request::Put {
            key: Key("x".into()),
            value: Value("1".into()),
        }],
    );
    sim.register_client(
        ClientID(1),
        vec![
            Request::Get {
                key: Key("y".into()),
            },
            Request::Get {
                key: Key("x".into()),
            },
        ],
    );
    sim.schedule_tick_all(0);
    sim.run();

    let history = sim.request_history();
    assert!(history.all_responded());
    assert_eq!(history.entries().len(), 3);
    assert!(sim.all_clients_done(), "{}", sim.format_log());
    assert_ne!(
        sim.routed_node(ClientID(0), 0),
        sim.routed_node(ClientID(1), 1),
        "the violating seed should route the write and final read to different nodes",
    );
    assert!(
        sim.check_linearizable().is_violation(),
        "expected a stable non-linearizable routed execution\nlog:\n{}",
        sim.format_log(),
    );
}
