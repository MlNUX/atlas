// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Terminal support. The macOS app draws terminals with SwiftTerm (P0.5,
//! own PTY), so the core only says what to start and tracks the shell's
//! folder; P2.9 (terminal grid in the core) is dropped.

use crate::actions::Prepared;
use crate::todo_task;

#[cfg_attr(feature = "ffi", derive(uniffi::Object))]
pub struct Terminal {}

#[cfg_attr(feature = "ffi", uniffi::export)]
impl Terminal {
    #[cfg_attr(feature = "ffi", uniffi::constructor)]
    pub fn new() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self {})
    }

    /// The shell to start in `cwd`: the configured one or the login shell,
    /// with Atlas' environment (`TERM`, `COLORTERM`, `ATLAS_PROJECT`, …).
    pub fn shell(&self, cwd: String, project_root: Option<String>) -> Prepared {
        let _ = (cwd, project_root);
        todo_task!("P2.2")
    }

    /// Current folder of process `pid` (the shell), for „Ordner folgen“.
    pub fn process_cwd(&self, pid: u32) -> Option<String> {
        let _ = pid;
        todo_task!("P2.2")
    }
}
