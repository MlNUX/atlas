// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Helpers shared by the services.

use std::path::{Path, PathBuf};

/// Paths cross the API as absolute UTF-8 strings.
pub(crate) fn path(s: &str) -> PathBuf {
    PathBuf::from(s)
}

pub(crate) fn string(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// Runs `work` on a new thread and awaits its result without a Rust async
/// runtime (pattern from spike P0.2: UniFFI polls the future from Swift).
#[allow(dead_code)]
pub(crate) async fn background<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = futures_channel::oneshot::channel();
    std::thread::spawn(move || {
        let _ = tx.send(work());
    });
    rx.await.expect("worker thread ended without a result")
}
