// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! The external editor, quick open (Ctrl+P) and the file list behind it.

use super::*;

impl qobject::AppController {
    pub fn editor_command(&self) -> QString {
        qs(Config::load().editor.command)
    }

    pub fn editor_in_terminal(&self) -> bool {
        Config::load().editor.terminal.unwrap_or(false)
    }

    pub fn set_editor_command(&self, command: &QString, terminal: bool) -> QString {
        let command = command.to_string();
        if let Err(err) = atlas_core::config::split_words(&command) {
            return qs(format!("{err:#}"));
        }
        let editor = EditorConfig { command, terminal: Some(terminal), ..Config::load().editor };
        match Config::save_editor(&editor) {
            Ok(()) => QString::default(),
            Err(err) => qs(format!("{err:#}")),
        }
    }

    pub fn open_in_editor(&self, path: &QString, line: i32) -> QString {
        let file = path_of(path);
        let result = match editor_launch(&Config::load().editor, &file, line.max(0) as u32, 0) {
            Ok(EditorLaunch::System) => serde_json::json!({ "mode": "system" }),
            Ok(EditorLaunch::Terminal(command)) => serde_json::json!({ "mode": "terminal", "command": command }),
            Ok(EditorLaunch::Detached(command)) => match spawn_detached(&command, file.parent()) {
                Ok(()) => serde_json::json!({ "mode": "detached", "command": command }),
                Err(err) => serde_json::json!({ "mode": "error", "command": command, "error": format!("{err:#}") }),
            },
            Err(err) => serde_json::json!({ "mode": "error", "error": format!("{err:#}") }),
        };
        qs(result.to_string())
    }

    pub fn quick_open(&self, query: &QString, limit: i32) -> QString {
        let (Some(root), Some(files)) = (self.root.as_ref(), self.files.as_ref()) else { return qs("[]") };
        let query = query.to_string();
        let (query, line) = finder::split_location(query.trim());
        let notes = self.notes.as_ref().and_then(|n| n.lock().ok());
        let mut results: Vec<(i32, serde_json::Value)> = finder::best(files, query, limit.max(1) as usize * 2)
            .into_iter()
            .map(|(entry, score)| {
                let path = finder::absolute(root, entry);
                let title = notes.as_ref().and_then(|n| n.note(&path)).and_then(|n| n.title.clone());
                let kind = if entry.is_dir {
                    "dir"
                } else if atlas_core::notes::is_markdown(&path) {
                    "note"
                } else {
                    "file"
                };
                let value = serde_json::json!({
                    "path": path.to_string_lossy(),
                    "relative": entry.relative,
                    "kind": kind,
                    "title": title,
                    "line": line,
                });
                (score, value)
            })
            .collect();
        // Notes are also found by their title.
        if let Some(notes) = notes.as_ref() {
            for path in notes.paths() {
                let Some(title) = notes.note(path).and_then(|n| n.title.clone()) else { continue };
                let Some(score) = finder::score(query, &title) else { continue };
                let path_str = path.to_string_lossy().into_owned();
                if results.iter().any(|(_, v)| v["path"] == path_str.as_str()) {
                    continue;
                }
                results.push((
                    score,
                    serde_json::json!({
                        "path": path_str,
                        "relative": path.strip_prefix(root).unwrap_or(path).to_string_lossy(),
                        "kind": "note",
                        "title": title,
                        "line": line,
                    }),
                ));
            }
        }
        results.sort_by_key(|r| std::cmp::Reverse(r.0));
        results.truncate(limit.max(1) as usize);
        qs(serde_json::Value::from(results.into_iter().map(|(_, v)| v).collect::<Vec<_>>()).to_string())
    }

    pub fn refresh_file_list(self: Pin<&mut Self>) {
        let Some(root) = self.root.clone() else { return };
        let generation = self.generation;
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let files = Arc::new(finder::list(&root));
            let _ = thread.queue(move |mut app| {
                if app.generation == generation {
                    // The graph shows all files (and folder nodes their
                    // current members): redraw on changes.
                    let changed = app.files.as_deref() != Some(&*files);
                    app.as_mut().rust_mut().files = Some(files);
                    if changed {
                        app.as_mut().bump_notes();
                    }
                }
            });
        });
    }

    pub fn fuzzy_score(&self, query: &QString, text: &QString) -> i32 {
        finder::score(&query.to_string(), &text.to_string()).unwrap_or(-1)
    }
}
