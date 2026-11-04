// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! The process-wide entry point: version, settings, places, known projects
//! and their sessions, background jobs, ILIAS. Opens [`Project`]s.

use std::sync::Arc;

use atlas_core::project::Project as CoreProject;

use crate::events::EventSink;
use crate::project::{Project, SessionSummary};
use crate::util::{path, string};
use crate::{AtlasError, Result, todo_task};

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

/// A project Atlas has opened before, with its active session.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct KnownProject {
    pub root: String,
    pub name: String,
    pub session: Option<SessionSummary>,
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
        Arc::new(crate::settings::Settings::new(self.sink.clone()))
    }

    /// Sidebar places (fixed ones first, then file manager bookmarks if on).
    pub fn places(&self) -> Vec<Place> {
        let file_manager = atlas_core::config::Config::load().places.file_manager;
        let (places, fixed) = atlas_core::places::sidebar_places(atlas_core::places::fixed(), file_manager);
        places
            .into_iter()
            .enumerate()
            .map(|(i, p)| Place { name: p.name, icon: p.icon, path: string(&p.path), fixed: i < fixed })
            .collect()
    }

    /// Projects opened before (existing folders only), most recent first.
    pub fn known_projects(&self) -> Vec<KnownProject> {
        atlas_core::session::AppState::load()
            .projects
            .into_iter()
            .filter(|root| root.is_dir())
            .map(|root| {
                let session = CoreProject::open_existing(&root).map(|p| {
                    let id = p.active_session();
                    crate::project::summary_of(&p, &id, true, p.session_name())
                });
                KnownProject {
                    name: root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                    root: string(&root),
                    session,
                }
            })
            .collect()
    }

    /// Renames the active session of another project (not open here).
    pub fn rename_project_session(&self, root: String, name: String) -> Result<()> {
        let root = path(&root);
        let project = CoreProject::open_existing(&root)
            .ok_or_else(|| AtlasError::NotFound { message: format!("{} hat keine Session.", root.display()) })?;
        Ok(project.rename(&name)?)
    }

    /// Removes a project from the list and clears its sessions (its files stay).
    pub fn forget_project(&self, root: String) -> Result<()> {
        let root = path(&root);
        if let Some(project) = CoreProject::open_existing(&root) {
            project.clear_sessions()?;
        }
        let mut state = atlas_core::session::AppState::load();
        state.forget(&root);
        state.save()?;
        self.sink.on_event(crate::events::Event::SessionsChanged { project_root: string(&root) });
        Ok(())
    }

    /// Opens the project at `root` (creating `.atlas/` if needed) for one
    /// window; `sink` receives that window's events. Opening the same root
    /// twice gives two independent handles.
    pub fn open_project(&self, root: String, sink: Arc<dyn EventSink>) -> Result<Arc<Project>> {
        Ok(Arc::new(Project::open(&path(&root), sink)?))
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
        let rest = url.strip_prefix("file://")?;
        // `file://localhost/…` and `file:///…`
        let rest = rest.strip_prefix("localhost").unwrap_or(rest);
        rest.starts_with('/').then(|| atlas_core::text::percent_decode(rest))
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
