// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

// Memory while browsing many PDFs: opens each PDF given, renders its first
// pages (like scrolling into it), then moves on. Prints the resident memory
// (RSS and PSS of this process) at the end.
//
//   cargo run -p atlas-core --example pdf_memory -- a.pdf b.pdf …

use std::path::PathBuf;
use std::time::Duration;

use atlas_core::pdf::{CancelToken, PdfService};

fn status(key: &str) -> String {
    let rollup = std::fs::read_to_string("/proc/self/smaps_rollup").unwrap_or_default();
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    rollup
        .lines()
        .chain(status.lines())
        .find(|l| l.starts_with(key))
        .map(|l| l.split_whitespace().nth(1).unwrap_or("0").to_owned())
        .unwrap_or_default()
}

fn main() {
    let files: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    let service = PdfService::get();
    for path in &files {
        let pages = service.open_blocking(path).expect("open").page_sizes.len();
        let (tx, rx) = crossbeam_channel::unbounded();
        let n = pages.min(3);
        for page in 0..n {
            let tx = tx.clone();
            service.render(path.clone(), page, 1200, CancelToken::default(), move |r| {
                let _ = tx.send(r.is_ok());
            });
        }
        for _ in 0..n {
            rx.recv_timeout(Duration::from_secs(30)).expect("render");
        }
        for page in 0..n {
            let _ = service.text_blocking(path, page);
        }
    }
    std::thread::sleep(Duration::from_millis(500));
    println!("pdfs={} rss_kib={} pss_kib={}", files.len(), status("VmRSS:"), status("Pss:"));
}
