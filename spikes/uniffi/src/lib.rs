// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Spike P0.2: the smallest API that exercises everything `atlas-api` will need
//! from UniFFI: plain functions, records, enums, errors, an object, a foreign
//! callback trait called from a background thread, and an `async` function that
//! does not depend on any Rust async runtime.

use std::path::Path;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

uniffi::setup_scaffolding!();

/// Version string, proves a plain call with a return value works.
#[uniffi::export]
pub fn core_version() -> String {
    format!("atlas-spike {}", env!("CARGO_PKG_VERSION"))
}

/// Plain arithmetic, proves integer arguments.
#[uniffi::export]
pub fn add(a: i64, b: i64) -> i64 {
    a + b
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ProjectInfo {
    pub name: String,
    pub path: String,
    pub entries: u32,
    pub has_atlas_dir: bool,
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum SpikeError {
    #[error("not a directory: {path}")]
    NotADirectory { path: String },
    #[error("io error: {message}")]
    Io { message: String },
}

/// Reads a folder like Atlas opens a project: returns a record or throws.
#[uniffi::export]
pub fn describe_project(path: String) -> Result<ProjectInfo, SpikeError> {
    let dir = Path::new(&path);
    if !dir.is_dir() {
        return Err(SpikeError::NotADirectory { path });
    }
    let entries = std::fs::read_dir(dir)
        .map_err(|e| SpikeError::Io { message: e.to_string() })?
        .count() as u32;
    Ok(ProjectInfo {
        name: dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        has_atlas_dir: dir.join(".atlas").is_dir(),
        path,
        entries,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum SpikeEvent {
    Tick { n: u32 },
    Done { total: u32 },
}

/// Implemented in Swift. Rust calls it from a background thread; the Swift
/// side is responsible for hopping to the main actor.
#[uniffi::export(with_foreign)]
pub trait EventSink: Send + Sync {
    fn on_event(&self, event: SpikeEvent);
}

/// Long-lived Rust object owned by Swift.
#[derive(uniffi::Object)]
pub struct Ticker {
    interval: Duration,
}

#[uniffi::export]
impl Ticker {
    #[uniffi::constructor]
    pub fn new(interval_ms: u64) -> Arc<Self> {
        Arc::new(Self {
            interval: Duration::from_millis(interval_ms),
        })
    }

    /// Emits `count` ticks and a final `Done` on a background thread.
    pub fn start(&self, sink: Arc<dyn EventSink>, count: u32) {
        let interval = self.interval;
        thread::spawn(move || {
            for n in 1..=count {
                thread::sleep(interval);
                sink.on_event(SpikeEvent::Tick { n });
            }
            sink.on_event(SpikeEvent::Done { total: count });
        });
    }
}

/// Async without a Rust runtime: the work runs on a thread and completes a
/// oneshot channel, whose future UniFFI polls from Swift's executor.
#[uniffi::export]
pub async fn delayed_sum(values: Vec<i64>, delay_ms: u64) -> i64 {
    let (tx, rx) = futures_channel::oneshot::channel();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(delay_ms));
        let _ = tx.send(values.iter().sum::<i64>());
    });
    rx.await.unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::sync::mpsc;

    #[test]
    fn plain_functions() {
        assert_eq!(add(2, 40), 42);
        assert!(core_version().starts_with("atlas-spike"));
    }

    #[test]
    fn describe_project_errors() {
        assert!(matches!(
            describe_project("/definitely/missing".into()),
            Err(SpikeError::NotADirectory { .. })
        ));
        let here = describe_project(env!("CARGO_MANIFEST_DIR").into()).unwrap();
        assert_eq!(here.name, "uniffi");
        assert!(here.entries > 0);
    }

    struct Collect(Mutex<mpsc::Sender<SpikeEvent>>);
    impl EventSink for Collect {
        fn on_event(&self, event: SpikeEvent) {
            self.0.lock().unwrap().send(event).unwrap();
        }
    }

    #[test]
    fn ticker_calls_back() {
        let (tx, rx) = mpsc::channel();
        Ticker::new(1).start(Arc::new(Collect(Mutex::new(tx))), 3);
        let got: Vec<_> = rx.iter().take(4).collect();
        assert_eq!(got.last(), Some(&SpikeEvent::Done { total: 3 }));
    }
}
