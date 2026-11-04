// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Actions (`actions.yaml`, extension commands) and background jobs.

use super::*;

impl qobject::AppController {
    pub(super) fn load_actions(&self) -> (Vec<atlas_core::actions::Action>, Vec<atlas_core::actions::LoadError>) {
        let atlas_dir = self.project.as_ref().map(|p| p.atlas_dir());
        let (mut actions, errors) = atlas_core::actions::load(atlas_dir.as_deref(), &atlas_core::config::config_dir());
        // Actions of active extensions follow the user's own.
        for ext in self.active_extensions() {
            actions.extend(ext.actions);
        }
        (actions, errors)
    }

    fn action_context(&self, paths: &QStringList, line: i32) -> atlas_core::actions::Context {
        atlas_core::actions::Context {
            project: self.root.clone(),
            paths: paths.iter().map(path_of).filter(|p| !is_terminal_path(p)).collect(),
            line: line.max(0) as u32,
        }
    }

    pub fn actions_for(&self, paths: &QStringList) -> QString {
        let (actions, _) = self.load_actions();
        let ctx = self.action_context(paths, 0);
        let applicable = atlas_core::actions::applicable(&actions, &ctx);
        // Two actions with the same name (e.g. from two extensions) say
        // where they come from.
        let twice = |name: &str| applicable.iter().filter(|a| a.name == name).count() > 1;
        let list: Vec<_> = applicable
            .iter()
            .map(|a| {
                let name = if twice(&a.name) { format!("{} ({})", a.name, a.source.label()) } else { a.name.clone() };
                serde_json::json!({
                    "id": a.id,
                    "name": name,
                    "description": a.description,
                    "source": a.source.label(),
                    "menu": a.in_menu,
                    "palette": a.in_palette,
                    "keybinding": a.keybinding,
                })
            })
            .collect();
        qs(serde_json::Value::from(list).to_string())
    }

    pub fn action_errors(&self) -> QString {
        let (_, errors) = self.load_actions();
        let list: Vec<_> = errors
            .iter()
            .map(|e| serde_json::json!({ "file": e.file.to_string_lossy(), "message": e.message }))
            .collect();
        qs(serde_json::Value::from(list).to_string())
    }

    pub fn prepare_action(&self, id: &QString, paths: &QStringList, line: i32) -> QString {
        let (actions, _) = self.load_actions();
        let id = id.to_string();
        let ctx = self.action_context(paths, line);
        let result = match actions.iter().find(|a| a.id == id) {
            None => serde_json::json!({ "error": format!("Die Aktion „{id}“ gibt es nicht mehr.") }),
            Some(action) if !action.applies(&ctx) => {
                serde_json::json!({ "error": format!("„{}“ passt nicht zur Auswahl.", action.name) })
            }
            Some(action) => match action.prepare(&ctx) {
                Ok(p) => serde_json::json!({
                    "name": action.name,
                    "argv": p.argv,
                    "display": p.display,
                    "cwd": p.cwd.to_string_lossy(),
                    "confirm": p.confirm,
                    "destructive": p.destructive,
                    "placement": match p.placement {
                        atlas_core::actions::Placement::Panel => "panel",
                        atlas_core::actions::Placement::Tab => "tab",
                        atlas_core::actions::Placement::Background => "background",
                    },
                }),
                Err(err) => serde_json::json!({ "error": format!("„{}“: {err:#}", action.name) }),
            },
        };
        qs(result.to_string())
    }

    pub fn start_background(&self, key: &QString, name: &QString, argv: &QString, cwd: &QString) -> QString {
        let argv: Vec<String> = serde_json::from_str(&argv.to_string()).unwrap_or_default();
        match atlas_core::background::start(&key.to_string(), &name.to_string(), &argv, &path_of(cwd), &[]) {
            Ok(()) => QString::default(),
            Err(err) => qs(format!("{err:#}")),
        }
    }

    pub fn background_status(&self, key: &QString) -> QString {
        let status = atlas_core::background::status(&key.to_string());
        qs(status.map_or_else(|| "{}".to_owned(), |s| serde_json::to_string(&s).unwrap_or_default()))
    }

    pub fn background_jobs(&self) -> QString {
        qs(serde_json::to_string(&atlas_core::background::list()).unwrap_or_default())
    }

    pub fn stop_background(&self, key: &QString) -> bool {
        atlas_core::background::stop(&key.to_string())
    }

    pub fn is_runnable(&self, path: &QString) -> bool {
        atlas_core::actions::script_command(&path_of(path)).is_some()
    }

    pub fn prepare_script(&self, path: &QString) -> QString {
        let result = match atlas_core::actions::prepare_script(&path_of(path)) {
            Ok((p, name)) => serde_json::json!({
                "name": name,
                "argv": p.argv,
                "display": p.display,
                "cwd": p.cwd.to_string_lossy(),
            }),
            Err(err) => serde_json::json!({ "error": format!("{err:#}") }),
        };
        qs(result.to_string())
    }

    pub fn actions_file(&self, global: bool) -> QString {
        let file = if global {
            atlas_core::actions::user_file(&atlas_core::config::config_dir())
        } else {
            match self.project.as_ref() {
                Some(p) => atlas_core::actions::project_file(&p.atlas_dir()),
                None => return QString::default(),
            }
        };
        if !file.exists()
            && let Err(err) = atlas_core::paths::write_atomic(&file, atlas_core::actions::TEMPLATE.as_bytes())
        {
            log::warn!("cannot create {}: {err:#}", file.display());
            return QString::default();
        }
        qs(file.to_string_lossy())
    }
}
