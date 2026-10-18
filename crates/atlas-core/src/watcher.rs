// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Basic file system watcher for the project tree.
//!
//! Only directories whose children are visible are watched (non-recursively),
//! which keeps the number of inotify watches proportional to what the user
//! sees instead of the size of the project. Events are coalesced per
//! directory and delivered after a short quiet period.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossbeam_channel::{Receiver, Sender, unbounded};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher as _};

const DEBOUNCE: Duration = Duration::from_millis(120);

/// Watches a set of directories and reports which of them changed.
pub struct DirWatcher {
    watcher: Mutex<RecommendedWatcher>,
    watched: Mutex<HashSet<PathBuf>>,
    _stop: Sender<()>,
}

impl DirWatcher {
    /// `on_change` is called from a background thread with the set of
    /// directories whose listing may have changed.
    pub fn new(on_change: impl Fn(Vec<PathBuf>) + Send + 'static) -> Result<Self> {
        let (raw_tx, raw_rx) = unbounded::<PathBuf>();
        let (stop_tx, stop_rx) = unbounded::<()>();
        let watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            let Ok(event) = res else { return };
            if matches!(event.kind, EventKind::Access(_)) {
                return;
            }
            for path in event.paths {
                // An event names the changed entry; its parent is the listing
                // that changed. Events on a watched directory itself (e.g.
                // deletion) are reported for both.
                if let Some(parent) = path.parent() {
                    let _ = raw_tx.send(parent.to_path_buf());
                }
                let _ = raw_tx.send(path);
            }
        })?;

        std::thread::Builder::new()
            .name("atlas-fs-debounce".into())
            .spawn(move || debounce_loop(raw_rx, stop_rx, on_change))?;

        Ok(Self { watcher: Mutex::new(watcher), watched: Mutex::new(HashSet::new()), _stop: stop_tx })
    }

    /// Makes the watched set equal to `dirs`.
    pub fn sync(&self, dirs: &[PathBuf]) {
        let wanted: HashSet<PathBuf> = dirs.iter().cloned().collect();
        let mut watched = self.watched.lock().unwrap();
        let mut watcher = self.watcher.lock().unwrap();
        for dir in watched.difference(&wanted) {
            let _ = watcher.unwatch(dir);
        }
        for dir in wanted.difference(&watched) {
            if let Err(err) = watcher.watch(dir, RecursiveMode::NonRecursive) {
                log::warn!("cannot watch {}: {err}", dir.display());
            }
        }
        *watched = wanted;
    }

    pub fn is_watching(&self, dir: &Path) -> bool {
        self.watched.lock().unwrap().contains(dir)
    }
}

fn debounce_loop(raw: Receiver<PathBuf>, stop: Receiver<()>, on_change: impl Fn(Vec<PathBuf>)) {
    // Flush once the newest event is older than DEBOUNCE, so a burst (e.g.
    // `git checkout`) results in a single refresh per directory. A steady
    // stream of events is still flushed every MAX_DELAY.
    const MAX_DELAY: Duration = Duration::from_millis(1000);
    let mut pending: HashSet<PathBuf> = HashSet::new();
    let mut first: Option<Instant> = None;
    let mut last = Instant::now();
    loop {
        let timeout = match first {
            Some(first) => (last + DEBOUNCE).min(first + MAX_DELAY).saturating_duration_since(Instant::now()),
            None => Duration::from_secs(3600),
        };
        crossbeam_channel::select! {
            recv(raw) -> msg => match msg {
                Ok(path) => {
                    pending.insert(path);
                    last = Instant::now();
                    first.get_or_insert(last);
                }
                Err(_) => return,
            },
            recv(stop) -> _ => return,
            default(timeout) => {}
        }
        if let Some(started) = first
            && (last.elapsed() >= DEBOUNCE || started.elapsed() >= MAX_DELAY)
        {
            on_change(pending.drain().collect());
            first = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_created_file_in_watched_dir() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let (tx, rx) = unbounded();
        let watcher = DirWatcher::new(move |dirs| {
            let _ = tx.send(dirs);
        })
        .unwrap();
        watcher.sync(std::slice::from_ref(&root));
        std::fs::write(root.join("new.txt"), "x").unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let dirs = rx.recv_timeout(deadline.saturating_duration_since(Instant::now())).unwrap();
            if dirs.contains(&root) {
                break;
            }
        }
    }
}
