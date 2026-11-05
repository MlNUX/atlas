// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! User-defined actions (`.atlas/actions.yaml`, user actions), running
//! scripts, background jobs, key bindings. The UI runs prepared commands
//! in its terminal (SwiftTerm) or starts them in the background here.
//! Template: `app/actions.rs`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use atlas_core::actions as core;

use crate::inner::Shared;
use crate::{AtlasError, Result};

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
    /// Offered in the context menu (else only palette and key).
    pub in_menu: bool,
    pub in_palette: bool,
    /// Atlas notation (`Ctrl+Alt+R`), empty if none.
    pub keybinding: String,
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
    /// Extra environment (already part of `argv` for terminal commands).
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
pub struct Actions {
    shared: Arc<Shared>,
}

impl Actions {
    pub(crate) fn new(shared: Arc<Shared>) -> Self {
        Self { shared }
    }

    /// Project and user actions, then those of the active extensions.
    fn load(&self) -> (Vec<core::Action>, Vec<core::LoadError>) {
        let atlas_dir = self.shared.project.as_ref().map(|p| p.atlas_dir());
        let (mut actions, errors) = core::load(atlas_dir.as_deref(), &atlas_core::config::config_dir());
        for ext in crate::extensions::active_extensions(&self.shared) {
            actions.extend(ext.actions);
        }
        (actions, errors)
    }

    fn context(&self, paths: &[String], line: u32) -> core::Context {
        core::Context {
            project: Some(self.shared.root.clone()),
            paths: paths.iter().map(PathBuf::from).collect(),
            line,
        }
    }
}

#[cfg_attr(feature = "ffi", uniffi::export)]
impl Actions {
    /// Actions for `paths` (files, a folder, or none for project actions).
    pub fn actions_for(&self, paths: Vec<String>) -> Vec<ActionInfo> {
        let (actions, _) = self.load();
        let ctx = self.context(&paths, 0);
        let applicable = core::applicable(&actions, &ctx);
        // Two actions with the same name (e.g. from two extensions) say
        // where they come from.
        let twice = |name: &str| applicable.iter().filter(|a| a.name == name).count() > 1;
        applicable
            .iter()
            .map(|a| ActionInfo {
                id: a.id.clone(),
                name: if twice(&a.name) { format!("{} ({})", a.name, a.source.label()) } else { a.name.clone() },
                description: a.description.clone(),
                source: match a.source {
                    core::Source::Project => ActionSource::Project,
                    core::Source::User => ActionSource::User,
                    core::Source::Extension(_) => ActionSource::Extension,
                },
                extension: match &a.source {
                    core::Source::Extension(name) => name.clone(),
                    _ => String::new(),
                },
                in_menu: a.in_menu,
                in_palette: a.in_palette,
                keybinding: a.keybinding.clone().unwrap_or_default(),
            })
            .collect()
    }

    /// Problems in the action files.
    pub fn errors(&self) -> Vec<ActionError> {
        self.load()
            .1
            .into_iter()
            .map(|e| ActionError { file: e.file.to_string_lossy().into_owned(), message: e.message })
            .collect()
    }

    /// The command of action `id` for `paths` (absolute) and the editor's
    /// `line` (0 = unknown).
    pub fn prepare(&self, id: String, paths: Vec<String>, line: u32) -> Result<Prepared> {
        let (actions, _) = self.load();
        let ctx = self.context(&paths, line);
        let action = actions
            .iter()
            .find(|a| a.id == id)
            .ok_or_else(|| AtlasError::NotFound { message: format!("Die Aktion „{id}“ gibt es nicht mehr.") })?;
        if !action.applies(&ctx) {
            return Err(AtlasError::Invalid { message: format!("„{}“ passt nicht zur Auswahl.", action.name) });
        }
        let p = action
            .prepare(&ctx)
            .map_err(|e| AtlasError::Invalid { message: format!("„{}“: {e:#}", action.name) })?;
        Ok(prepared(action.name.clone(), p))
    }

    /// `path` is a script Atlas can run (`.sh`, or executable with `#!`).
    pub fn is_runnable(&self, path: String) -> bool {
        core::script_command(Path::new(&path)).is_some()
    }

    /// The command that runs the script `path` in a terminal.
    pub fn prepare_script(&self, path: String) -> Result<Prepared> {
        let (p, name) =
            core::prepare_script(Path::new(&path)).map_err(|e| AtlasError::Invalid { message: format!("{e:#}") })?;
        Ok(prepared(name, p))
    }

    /// Starts a prepared command as background job `key` (the action id);
    /// one at a time per key. Follow it with `Atlas::background_job`.
    pub fn start_background(&self, key: String, prepared: Prepared) -> Result<()> {
        let env: Vec<(String, String)> = prepared.env.into_iter().map(|v| (v.name, v.value)).collect();
        atlas_core::background::start(&key, &prepared.name, &prepared.argv, Path::new(&prepared.cwd), &env)
            .map_err(|e| AtlasError::Failed { message: format!("{e:#}") })?;
        self.shared.emit(crate::events::Event::BackgroundJobChanged { key });
        Ok(())
    }

    /// The project (`global` false) or user action file, created from a
    /// commented template if missing.
    pub fn actions_file(&self, global: bool) -> Result<String> {
        let file = if global {
            core::user_file(&atlas_core::config::config_dir())
        } else {
            let project = self.shared.project.as_ref().ok_or_else(|| AtlasError::Unavailable {
                message: self.shared.storage_error.clone().unwrap_or_else(|| "Kein .atlas-Ordner".into()),
            })?;
            core::project_file(&project.atlas_dir())
        };
        if !file.exists() {
            atlas_core::paths::write_atomic(&file, core::TEMPLATE.as_bytes())
                .map_err(|e| AtlasError::Failed { message: format!("{e:#}") })?;
        }
        Ok(file.to_string_lossy().into_owned())
    }

    /// Key sequences of actions and active extensions' views (the first
    /// one wins).
    pub fn keybindings(&self) -> Vec<KeyBinding> {
        let (actions, _) = self.load();
        let mut seen = std::collections::HashSet::new();
        let mut list: Vec<KeyBinding> = actions
            .iter()
            .filter_map(|a| a.keybinding.as_ref().map(|k| (k, a)))
            .filter(|(k, _)| seen.insert(k.to_string()))
            .map(|(k, a)| KeyBinding { sequence: k.clone(), id: a.id.clone(), name: a.name.clone() })
            .collect();
        for ext in crate::extensions::active_extensions(&self.shared) {
            for view in ext.views {
                if let Some(k) = view.keybinding.filter(|k| seen.insert(k.clone())) {
                    list.push(KeyBinding { sequence: k, id: view.id, name: view.name });
                }
            }
        }
        list
    }

    /// Re-reads the action files (they are read on every call; this only
    /// tells the windows).
    pub fn reload(&self) {
        self.shared.emit(crate::events::Event::ExtensionsChanged);
    }
}

fn prepared(name: String, p: core::Prepared) -> Prepared {
    Prepared {
        name,
        argv: p.argv,
        display: p.display,
        cwd: p.cwd.to_string_lossy().into_owned(),
        env: Vec::new(),
        confirm: p.confirm,
        destructive: p.destructive,
        placement: match p.placement {
            core::Placement::Panel => Placement::Panel,
            core::Placement::Tab => Placement::Tab,
            core::Placement::Background => Placement::Background,
        },
    }
}
