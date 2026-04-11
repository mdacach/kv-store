//! Generate a linearizability check visualization.
//!
//! Usage:
//!   cargo run --example linearizability
//!   open target/linearizability.html

#[path = "scenarios/mod.rs"]
mod scenarios;

use kv_store::visualization::linearizability;

fn main() {
    let all = scenarios::all();
    let scenario = all
        .iter()
        .find(|s| s.name.starts_with("Two clients"))
        .expect("expected the 'Two clients' scenario");

    let entries = scenario.sim.history().entries();
    let result = kv_store::check_linearizable(entries);
    let html = linearizability::visualize(entries, &result);

    std::fs::create_dir_all("target").ok();
    let path = std::path::Path::new("target/linearizability.html");
    std::fs::write(path, &html).expect("Failed to write HTML");

    let abs = std::fs::canonicalize(path).unwrap();
    eprintln!("{}\n{result}", scenario.name);
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
