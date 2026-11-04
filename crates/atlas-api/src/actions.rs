// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! User-defined actions (`.atlas/actions.yaml`, user actions), running
//! scripts, background jobs, key bindings. The UI runs prepared commands
//! in its terminal (SwiftTerm) or starts them in the background here.
//! Template: `app/actions.rs`.

use crate::{Result, todo_task};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ActionSource {
    Project,
    User,
    Extension,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ActionInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source: ActionSource,
    /// Extension name for `Extension`, empty otherwise.
    pub extension: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ActionError {
    pub file: String,
    pub message: String,
}

/// Where a prepared command runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum Placement {
    /// The terminal panel.
    Panel,
    /// A terminal tab of its own.
    Tab,
    /// No terminal: [`Actions::start_background`].
    Background,
}

/// A command ready to run.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Prepared {
    pub name: String,
    pub argv: Vec<String>,
    /// The command as shown to the user (shell-quoted).
    pub display: String,
    pub cwd: String,
    pub env: Vec<EnvVar>,
    /// Ask before running.
    pub confirm: bool,
    /// Looks destructive (`rm`, `git reset --hard`, …): ask with a warning.
    pub destructive: bool,
    pub placement: Placement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct EnvVar {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct KeyBinding {
    /// Atlas notation (`Ctrl+Alt+R`); the macOS app maps it (E10).
    pub sequence: String,
    pub id: String,
    pub name: String,
}

#[cfg_attr(feature = "ffi", derive(uniffi::Object))]
pub struct Actions {}

#[cfg_attr(feature = "ffi", uniffi::export)]
impl Actions {
    /// Actions for `paths` (files, a folder, or none for project actions).
    pub fn actions_for(&self, paths: Vec<String>) -> Vec<ActionInfo> {
        let _ = paths;
        todo_task!("P2.7")
    }

    /// Problems in the action files.
    pub fn errors(&self) -> Vec<ActionError> {
        todo_task!("P2.7")
    }

    pub fn prepare(&self, id: String, paths: Vec<String>, line: u32) -> Result<Prepared> {
        let _ = (id, paths, line);
        todo_task!("P2.7")
    }

    /// `path` is a script Atlas can run (`.sh`, or executable with `#!`).
    pub fn is_runnable(&self, path: String) -> bool {
        let _ = path;
        todo_task!("P2.7")
    }

    pub fn prepare_script(&self, path: String) -> Result<Prepared> {
        let _ = path;
        todo_task!("P2.7")
    }

    /// Starts a prepared command as background job `key` (the action id).
    pub fn start_background(&self, key: String, prepared: Prepared) -> Result<()> {
        let _ = (key, prepared);
        todo_task!("P2.7")
    }

    /// The project (`global` false) or user action file, created from a
    /// commented template if missing.
    pub fn actions_file(&self, global: bool) -> Result<String> {
        let _ = global;
        todo_task!("P2.7")
    }

    /// Key sequences of actions and active extensions.
    pub fn keybindings(&self) -> Vec<KeyBinding> {
        todo_task!("P2.7")
    }

    /// Re-reads the action files.
    pub fn reload(&self) {
        todo_task!("P2.7")
    }
}
