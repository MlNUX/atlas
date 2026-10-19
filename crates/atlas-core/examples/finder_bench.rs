// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Time of a quick open query over a large synthetic file list:
//! `cargo run --release -p atlas-core --example finder_bench [entries]`.

use std::time::Instant;

use atlas_core::finder::{Entry, best};

fn main() {
    let n: usize = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(100_000);
    let entries: Vec<Entry> = (0..n)
        .map(|i| Entry {
            relative: format!(
                "modul_{}/unter{}/datei_{i}_{}.rs",
                i % 97,
                i % 13,
                ["parser", "lexer", "view", "model"][i % 4]
            ),
            is_dir: false,
        })
        .collect();
    for query in ["p", "parser", "mod5 lex", "datei_99999", "xyz"] {
        let start = Instant::now();
        let hits = best(&entries, query, 60);
        println!("{n} Einträge, „{query}“: {} Treffer, {:.1} ms", hits.len(), start.elapsed().as_secs_f64() * 1000.0);
    }
}
