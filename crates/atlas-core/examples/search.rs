// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Runs a project search through `atlas_core::search` and prints timings.
//!
//! ```text
//! cargo run --release -p atlas-core --example search -- ROOT PATTERN
//! ```

use std::time::Instant;

use atlas_core::search::{self, Query};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let root = std::path::PathBuf::from(args.next().unwrap_or_else(|| ".".into()));
    let pattern = args.next().unwrap_or_else(|| "TODO".into());
    let started = Instant::now();
    let (tx, rx) = crossbeam_channel::unbounded();
    let _handle = search::start(
        &root,
        Query::new(pattern),
        move |batch| {
            for m in batch {
                println!(
                    "{:>8.1} ms  {}:{}  {}",
                    started.elapsed().as_secs_f64() * 1000.0,
                    m.relative,
                    m.line,
                    m.text.trim()
                );
            }
        },
        move |summary| {
            let _ = tx.send(summary);
        },
    )?;
    let summary = rx.recv()?;
    println!(
        "{} matches in {:.1} ms (truncated: {}, error: {:?})",
        summary.matches,
        summary.elapsed.as_secs_f64() * 1000.0,
        summary.truncated,
        summary.error
    );
    Ok(())
}
