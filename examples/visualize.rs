//! Generate an interactive HTML trace visualization.
//!
//! Usage:
//!   cargo run --example visualize
//!   open target/trace.html

#[path = "scenarios/mod.rs"]
mod scenarios;

use kv_store::visualization::trace;

fn main() {
    let all = scenarios::all();
    let scenarios: Vec<trace::Scenario<'_>> = all
        .iter()
        .map(|s| trace::Scenario {
            name: s.name,
            sim: &s.sim,
        })
        .collect();
    let html = trace::render(&scenarios);

    std::fs::create_dir_all("target").ok();
    let path = std::path::Path::new("target/trace.html");
    std::fs::write(path, &html).expect("Failed to write HTML");

    let abs = std::fs::canonicalize(path).unwrap();
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
