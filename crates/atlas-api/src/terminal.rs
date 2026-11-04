// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Terminal support. The macOS app draws terminals with SwiftTerm (P0.5,
//! own PTY), so the core only says what to start and tracks the shell's
//! folder; P2.9 (terminal grid in the core) is dropped.

use crate::actions::{EnvVar, Placement, Prepared};
use crate::util::string;

/// Atlas' terminal colours as `0xRRGGBB` (same as the Linux terminal).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct TerminalPalette {
    pub foreground: u32,
    pub background: u32,
    pub cursor: u32,
    pub selection: u32,
    /// ANSI colours 0–15.
    pub ansi: Vec<u32>,
}

#[cfg_attr(feature = "ffi", derive(uniffi::Object))]
pub struct Terminal {}

#[cfg_attr(feature = "ffi", uniffi::export)]
impl Terminal {
    #[cfg_attr(feature = "ffi", uniffi::constructor)]
    pub fn new() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self {})
    }

    /// The shell to start in `cwd`: the configured one (`[terminal] shell`),
    /// else `ATLAS_SHELL`, the login shell, `$SHELL`; with the same
    /// environment as the Linux terminal (`TERM`, `COLORTERM`,
    /// `TERM_PROGRAM`) plus `ATLAS_PROJECT`.
    pub fn shell(&self, cwd: String, project_root: Option<String>) -> Prepared {
        let configured = atlas_core::config::Config::load().terminal.shell;
        let program = if !configured.is_empty() && std::path::Path::new(&configured).exists() {
            configured
        } else {
            atlas_terminal::default_shell()
        };
        let var = |name: &str, value: &str| EnvVar { name: name.into(), value: value.into() };
        let mut env = vec![var("TERM", "xterm-256color"), var("COLORTERM", "truecolor"), var("TERM_PROGRAM", "atlas")];
        if let Some(root) = project_root {
            env.push(var("ATLAS_PROJECT", &root));
        }
        let name =
            std::path::Path::new(&program).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        Prepared {
            display: program.clone(),
            argv: vec![program, "-l".into()],
            name,
            cwd,
            env,
            confirm: false,
            destructive: false,
            placement: Placement::Panel,
        }
    }

    /// The dark or light palette (`dark` follows the system appearance).
    pub fn palette(&self, dark: bool) -> TerminalPalette {
        let p = atlas_terminal::colors::hex_palette(!dark);
        TerminalPalette {
            foreground: p.foreground,
            background: p.background,
            cursor: p.cursor,
            selection: p.selection,
            ansi: p.ansi.to_vec(),
        }
    }

    /// Current folder of process `pid` (the shell), for „Ordner folgen“.
    pub fn process_cwd(&self, pid: u32) -> Option<String> {
        atlas_terminal::process_cwd(pid).map(|p| string(&p))
    }
}
