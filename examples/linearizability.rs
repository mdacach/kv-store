//! Generate a linearizability check visualization.
//!
//! Usage:
//!   cargo run --example linearizability
//!   open target/linearizability.html

use kv_store::visualization::linearizability;
use kv_store::{ClientID, HistoryEntry, Key, Request, Response, Value};

fn main() {
    let entries = vec![
        HistoryEntry {
            client_id: ClientID(0),
            request: Request::Put {
                key: Key("x".into()),
                value: Value("1".into()),
            },
            invoke_time: 0,
            return_time: 1,
            response: Response(None),
        },
        HistoryEntry {
            client_id: ClientID(1),
            request: Request::Get {
                key: Key("y".into()),
            },
            invoke_time: 0,
            return_time: 2,
            response: Response(None),
        },
        HistoryEntry {
            client_id: ClientID(1),
            request: Request::Get {
                key: Key("x".into()),
            },
            invoke_time: 2,
            return_time: 3,
            response: Response(None),
        },
    ];
    let result = kv_store::check_linearizable(&entries);
    let html = linearizability::visualize(&entries, &result);

    std::fs::create_dir_all("target").ok();
    let path = std::path::Path::new("target/linearizability.html");
    std::fs::write(path, &html).expect("Failed to write HTML");

    let abs = std::fs::canonicalize(path).unwrap();
    eprintln!("Hand-crafted stale-read violation\n{result}");
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
