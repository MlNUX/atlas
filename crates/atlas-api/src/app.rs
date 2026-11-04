// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! The process-wide entry point: version, settings, places, known projects
//! and their sessions, background jobs, ILIAS. Opens [`Project`]s.

use std::sync::Arc;

use crate::events::EventSink;
use crate::project::{Project, SessionSummary};
use crate::{Result, todo_task};

/// A place in the sidebar (home, desktop, …, trash, file manager bookmarks).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Place {
    pub name: String,
    /// Symbolic icon name (`home`, `desktop`, `documents`, `trash`, …).
    pub icon: String,
    pub path: String,
    /// One of Atlas' fixed places (not a file manager bookmark).
    pub fixed: bool,
}

/// A project Atlas has opened before.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct KnownProject {
    pub root: String,
    pub name: String,
    pub exists: bool,
    /// Unix seconds of the last use.
    pub last_used: i64,
    pub sessions: Vec<SessionSummary>,
}

/// A command running in the background (`terminal: background` actions).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct BackgroundJob {
    pub key: String,
    pub name: String,
    pub cwd: String,
    pub running: bool,
    pub exit_code: Option<i32>,
    /// End of the output (at most 16 KiB).
    pub log: String,
}

#[cfg_attr(feature = "ffi", derive(uniffi::Object))]
pub struct Atlas {
    sink: Arc<dyn EventSink>,
}

#[cfg_attr(feature = "ffi", uniffi::export)]
impl Atlas {
    /// Once per process. `sink` receives the app-wide events.
    #[cfg_attr(feature = "ffi", uniffi::constructor)]
    pub fn new(sink: Arc<dyn EventSink>) -> Arc<Self> {
        Arc::new(Self { sink })
    }

    pub fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_owned()
    }

    /// Folder of `config.toml`, actions and extension settings.
    pub fn config_dir(&self) -> String {
        crate::util::string(&atlas_core::config::config_dir())
    }

    /// The settings (shared by all windows).
    pub fn settings(&self) -> Arc<crate::settings::Settings> {
        todo_task!("P2.8")
    }

    /// Sidebar places (fixed ones first, then file manager bookmarks if on).
    pub fn places(&self) -> Vec<Place> {
        todo_task!("P2.2")
    }

    /// Projects opened before, most recent first.
    pub fn known_projects(&self) -> Vec<KnownProject> {
        todo_task!("P2.2")
    }

    /// Removes a project from the list (its files and `.atlas/` stay).
    pub fn forget_project(&self, root: String) -> Result<()> {
        let _ = root;
        todo_task!("P2.2")
    }

    /// Opens the project at `root` (creating `.atlas/` if needed) for one
    /// window; `sink` receives that window's events. Opening the same root
    /// twice gives two independent handles.
    pub fn open_project(&self, root: String, sink: Arc<dyn EventSink>) -> Result<Arc<Project>> {
        let _ = (root, sink, &self.sink);
        todo_task!("P2.2")
    }

    /// Background jobs of all projects.
    pub fn background_jobs(&self) -> Vec<BackgroundJob> {
        todo_task!("P2.7")
    }

    pub fn background_job(&self, key: String) -> Option<BackgroundJob> {
        let _ = key;
        todo_task!("P2.7")
    }

    /// Stops a job (SIGTERM, then SIGKILL after 3 s); `false` if not running.
    pub fn stop_background_job(&self, key: String) -> bool {
        let _ = key;
        todo_task!("P2.7")
    }

    /// Stops every background job; call when the app quits (macOS has no
    /// parent-death signal, see `atlas_core::platform`).
    pub fn stop_all_background_jobs(&self) {
        atlas_core::background::stop_all()
    }

    /// The ILIAS connection (one account per user).
    pub fn ilias(&self) -> Arc<crate::ilias::Ilias> {
        todo_task!("P2.10")
    }

    /// Converts a `file://` URL (drag and drop, clipboard) to a path.
    pub fn url_to_path(&self, url: String) -> Option<String> {
        let _ = url;
        todo_task!("P2.2")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::Event;

    struct Quiet;
    impl EventSink for Quiet {
        fn on_event(&self, _: Event) {}
    }

    #[test]
    fn version_and_config_dir() {
        let atlas = Atlas::new(Arc::new(Quiet));
        assert_eq!(atlas.version(), env!("CARGO_PKG_VERSION"));
        assert!(!atlas.config_dir().is_empty());
    }

    #[test]
    fn async_helper_runs_on_a_thread() {
        let main = std::thread::current().id();
        let other = futures_executor_block_on(crate::util::background(move || std::thread::current().id() != main));
        assert!(other);
    }

    /// Minimal executor for the test (the UI's executor polls in real use).
    fn futures_executor_block_on<F: std::future::Future>(future: F) -> F::Output {
        use std::task::{Context, Poll, Wake, Waker};
        struct Unpark(std::thread::Thread);
        impl Wake for Unpark {
            fn wake(self: Arc<Self>) {
                self.0.unpark();
            }
        }
        let waker = Waker::from(Arc::new(Unpark(std::thread::current())));
        let mut cx = Context::from_waker(&waker);
        let mut future = std::pin::pin!(future);
        loop {
            match future.as_mut().poll(&mut cx) {
                Poll::Ready(v) => return v,
                Poll::Pending => std::thread::park(),
            }
        }
    }
}
