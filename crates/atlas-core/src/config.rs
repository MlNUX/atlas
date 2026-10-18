// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! User configuration: `$XDG_CONFIG_HOME/atlas/config.toml`.
//!
//! ```toml
//! [editor]
//! # {file}, {line}, {column} and {dir} are replaced; quoting like a shell.
//! command = "code --goto {file}:{line}:{column}"
//! # Run inside an Atlas terminal (for vim, helix, nano, …).
//! terminal = false
//! ```
//!
//! Without a configured command Atlas uses `$VISUAL` or `$EDITOR` (in a
//! terminal), and otherwise the system's default application.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorConfig {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub editor: EditorConfig,
}

pub fn config_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("ATLAS_CONFIG_DIR") {
        return PathBuf::from(dir);
    }
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("atlas")
}

pub fn config_file() -> PathBuf {
    config_dir().join("config.toml")
}

impl Config {
    /// Loads the configuration; a missing or invalid file gives defaults
    /// (invalid files are logged, never overwritten silently).
    pub fn load() -> Self {
        Self::load_from(&config_file())
    }

    pub fn load_from(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).unwrap_or_else(|err| {
                log::warn!("ignoring invalid {}: {err}", path.display());
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    /// Updates only the `[editor]` table and keeps everything else in the
    /// file (including keys Atlas does not know).
    pub fn save_editor(editor: &EditorConfig) -> Result<()> {
        Self::save_editor_to(&config_file(), editor)
    }

    pub fn save_editor_to(path: &Path, editor: &EditorConfig) -> Result<()> {
        let mut doc: toml::Table = match std::fs::read_to_string(path) {
            Ok(text) => text.parse().with_context(|| format!("{} is not valid TOML", path.display()))?,
            Err(_) => toml::Table::new(),
        };
        doc.insert("editor".into(), toml::Value::try_from(editor)?);
        crate::paths::write_atomic(path, toml::to_string_pretty(&doc)?.as_bytes())
    }
}

/// Splits a command line like a POSIX shell (quotes and backslashes; no
/// variables, globs or operators).
pub fn split_words(line: &str) -> Result<Vec<String>> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                in_word = true;
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(c) => word.push(c),
                        None => anyhow::bail!("unclosed ' in command"),
                    }
                }
            }
            '"' => {
                in_word = true;
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some(c @ ('"' | '\\' | '$' | '`')) => word.push(c),
                            Some(c) => {
                                word.push('\\');
                                word.push(c);
                            }
                            None => anyhow::bail!("unclosed \" in command"),
                        },
                        Some(c) => word.push(c),
                        None => anyhow::bail!("unclosed \" in command"),
                    }
                }
            }
            '\\' => {
                in_word = true;
                if let Some(c) = chars.next() {
                    word.push(c);
                }
            }
            c if c.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut word));
                    in_word = false;
                }
            }
            c => {
                in_word = true;
                word.push(c);
            }
        }
    }
    if in_word {
        words.push(word);
    }
    Ok(words)
}

/// Terminal editors that need a terminal to run in.
const TERMINAL_EDITORS: &[&str] =
    &["vi", "vim", "nvim", "nano", "hx", "helix", "micro", "kak", "emacsclient -t", "mg", "ne", "joe", "mcedit", "ed"];

/// How to open a file in the external editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorLaunch {
    /// Start this program detached.
    Detached(Vec<String>),
    /// Run this program in an Atlas terminal.
    Terminal(Vec<String>),
    /// Let the desktop open the file with its default application.
    System,
}

/// Builds the command for `file` at `line`/`column` (1-based, 0 = unknown).
pub fn editor_launch(config: &EditorConfig, file: &Path, line: u32, column: u32) -> Result<EditorLaunch> {
    let from_env = || std::env::var("VISUAL").ok().or_else(|| std::env::var("EDITOR").ok()).filter(|e| !e.is_empty());
    let (template, configured) = if !config.command.trim().is_empty() {
        (config.command.clone(), true)
    } else if let Some(editor) = from_env() {
        (editor, false)
    } else {
        return Ok(EditorLaunch::System);
    };
    let mut words = split_words(&template)?;
    let program = words.first().context("empty editor command")?.clone();
    let has_placeholder = words.iter().any(|w| w.contains("{file}"));
    let base = Path::new(&program).file_name().and_then(|n| n.to_str()).unwrap_or(&program).to_owned();
    // `$EDITOR` without placeholders: use the common `+LINE FILE` form of
    // terminal editors, or just the file.
    if !has_placeholder {
        if line > 0 && TERMINAL_EDITORS.iter().any(|e| e.split(' ').next() == Some(base.as_str())) {
            words.push(format!("+{line}"));
        }
        words.push("{file}".into());
    }
    let dir = file.parent().unwrap_or(Path::new("/"));
    let line = line.max(1);
    let column = column.max(1);
    let args: Vec<String> = words
        .into_iter()
        .map(|w| {
            w.replace("{file}", &file.to_string_lossy())
                .replace("{dir}", &dir.to_string_lossy())
                .replace("{line}", &line.to_string())
                .replace("{column}", &column.to_string())
        })
        .collect();
    let terminal = match config.terminal {
        Some(t) if configured => t,
        _ => TERMINAL_EDITORS.iter().any(|e| e.split(' ').next() == Some(base.as_str())),
    };
    Ok(if terminal { EditorLaunch::Terminal(args) } else { EditorLaunch::Detached(args) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_like_a_shell() {
        assert_eq!(split_words(r#"code --goto "{file}:{line}""#).unwrap(), ["code", "--goto", "{file}:{line}"]);
        assert_eq!(split_words(r"a 'b c' d\ e").unwrap(), ["a", "b c", "d e"]);
        assert_eq!(split_words(r#""a \"q\"""#).unwrap(), [r#"a "q""#]);
        assert!(split_words("'open").is_err());
    }

    #[test]
    fn builds_editor_commands() {
        let file = Path::new("/p/src/Mein Parser.rs");
        let cfg = EditorConfig { command: "code --goto {file}:{line}:{column}".into(), terminal: None };
        assert_eq!(
            editor_launch(&cfg, file, 12, 0).unwrap(),
            EditorLaunch::Detached(vec!["code".into(), "--goto".into(), "/p/src/Mein Parser.rs:12:1".into()])
        );
        let cfg = EditorConfig { command: "nvim".into(), terminal: None };
        assert_eq!(
            editor_launch(&cfg, file, 7, 0).unwrap(),
            EditorLaunch::Terminal(vec!["nvim".into(), "+7".into(), "/p/src/Mein Parser.rs".into()])
        );
        let cfg = EditorConfig { command: "my-term-editor {file}".into(), terminal: Some(true) };
        assert!(matches!(editor_launch(&cfg, file, 1, 1).unwrap(), EditorLaunch::Terminal(_)));
    }

    #[test]
    fn keeps_unknown_keys_when_saving() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.toml");
        std::fs::write(&file, "[other]\nkey = 1\n").unwrap();
        let editor = EditorConfig { command: "kate -l {line} {file}".into(), terminal: Some(false) };
        Config::save_editor_to(&file, &editor).unwrap();
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.contains("[other]"), "{text}");
        assert_eq!(Config::load_from(&file).editor, editor);
    }
}
