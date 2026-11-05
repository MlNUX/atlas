// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Extensions: list, trust, settings, views and their bridge, processors and exporters.

use super::*;

impl qobject::AppController {
    pub fn context_extensions(&self) -> QString {
        let config = Config::load().context;
        let list: Vec<_> = self
            .active_extensions()
            .iter()
            .filter(|e| !e.views.is_empty() || e.actions.iter().any(|a| a.in_palette))
            .map(|e| {
                let views = e.views.iter().map(
                    |v| serde_json::json!({ "id": v.id, "name": v.name, "kind": "view", "keybinding": v.keybinding }),
                );
                let commands = e.actions.iter().filter(|a| a.in_palette).map(|a| {
                    serde_json::json!({ "id": a.id, "name": a.name, "kind": "command", "keybinding": a.keybinding })
                });
                serde_json::json!({
                    "name": e.name,
                    "title": e.title,
                    "description": e.description,
                    "shown": config.extensions.get(&e.name).copied().unwrap_or(e.context),
                    "entries": views.chain(commands).collect::<Vec<_>>(),
                })
            })
            .collect();
        qs(serde_json::Value::from(list).to_string())
    }

    /// All extensions with their status and the user's settings. A key
    /// sequence that an action or an earlier extension already uses is
    /// dropped (with an error).
    fn extension_list(&self) -> Vec<(atlas_core::extensions::Extension, atlas_core::extensions::Status)> {
        use atlas_core::extensions;
        let config_dir = atlas_core::config::config_dir();
        let project_root = self.project.as_ref().map(|p| extensions::project_root(&p.atlas_dir()));
        let found = extensions::discover(&extensions::user_root(&config_dir), project_root.as_deref());
        let trust = extensions::Trust::load(&atlas_core::paths::state_root());
        let disabled = Config::load().extensions.disabled;
        let settings = extensions::load_settings(&config_dir);
        let atlas_dir = self.project.as_ref().map(|p| p.atlas_dir());
        let (own, _) = atlas_core::actions::load(atlas_dir.as_deref(), &config_dir);
        let mut taken: Vec<(String, String)> =
            own.iter().filter_map(|a| a.keybinding.clone().map(|k| (k, a.source.label()))).collect();
        found
            .into_iter()
            .map(|mut e| {
                e.apply_settings(&settings);
                let status = extensions::status(&e, &trust, &disabled);
                let mut errors = Vec::new();
                for action in &mut e.actions {
                    let Some(key) = action.keybinding.clone() else { continue };
                    if let Some((_, owner)) = taken.iter().find(|(k, _)| *k == key) {
                        errors.push(format!("„{}“: {key} ist schon belegt ({owner})", action.name));
                        action.keybinding = None;
                    } else if status == extensions::Status::Active {
                        taken.push((key, format!("Erweiterung {}", e.name)));
                    }
                }
                let mut view_errors = Vec::new();
                for view in &mut e.views {
                    let Some(key) = view.keybinding.clone() else { continue };
                    if let Some((_, owner)) = taken.iter().find(|(k, _)| *k == key) {
                        view_errors.push(format!("„{}“: {key} ist schon belegt ({owner})", view.name));
                        view.keybinding = None;
                    } else if status == extensions::Status::Active {
                        taken.push((key, format!("Erweiterung {}", e.name)));
                    }
                }
                e.errors.extend(errors);
                e.errors.extend(view_errors);
                (e, status)
            })
            .collect()
    }

    pub(super) fn active_extensions(&self) -> Vec<atlas_core::extensions::Extension> {
        self.extension_list()
            .into_iter()
            .filter(|(_, s)| *s == atlas_core::extensions::Status::Active)
            .map(|(e, _)| e)
            .collect()
    }

    pub fn extensions(&self) -> QString {
        use atlas_core::extensions::Location;
        let settings = atlas_core::extensions::load_settings(&atlas_core::config::config_dir());
        let list: Vec<_> = self
            .extension_list()
            .iter()
            .map(|(e, status)| {
                let commands = |c: &atlas_core::actions::Command| match c {
                    atlas_core::actions::Command::Argv(argv) => argv.join(" "),
                    atlas_core::actions::Command::Shell(s) => format!("sh -c {s}"),
                };
                let values = e.setting_values(&settings);
                serde_json::json!({
                    "name": e.name,
                    "title": e.title,
                    "version": e.version,
                    "description": e.description,
                    "api": e.api,
                    "permissions": e.permissions.as_ref().map(|p| serde_json::json!({
                        "run": p.run, "files": p.files.as_str(), "network": p.network,
                    })),
                    "settings": values.iter().map(|(s, value)| serde_json::json!({
                        "key": s.key, "title": s.title, "description": s.description, "type": s.kind.as_str(),
                        "choices": s.choices, "value": value, "default": s.default,
                    })).collect::<Vec<_>>(),
                    "dir": e.dir.to_string_lossy(),
                    "manifest": e.dir.join(atlas_core::extensions::MANIFEST).to_string_lossy(),
                    "location": if e.location == Location::User { "user" } else { "project" },
                    "status": status.as_str(),
                    "errors": e.errors,
                    "actions": e.actions.iter().map(|a| serde_json::json!({
                        "name": a.name, "command": commands(&a.command), "keybinding": a.keybinding,
                    })).collect::<Vec<_>>(),
                    "processors": e.processors.iter().map(|p| serde_json::json!({
                        "name": p.name, "applies_to": p.applies_to, "output": p.output.as_str(), "command": commands(&p.command),
                    })).collect::<Vec<_>>(),
                    "exporters": e.exporters.iter().map(|x| serde_json::json!({
                        "name": x.name, "applies_to": x.applies_to, "target": x.target, "command": commands(&x.command),
                    })).collect::<Vec<_>>(),
                    "views": e.views.iter().map(|v| serde_json::json!({
                        "id": v.id, "name": v.name, "page": v.page, "applies_to": v.applies_to, "keybinding": v.keybinding,
                    })).collect::<Vec<_>>(),
                })
            })
            .collect();
        qs(serde_json::Value::from(list).to_string())
    }

    pub fn refresh_extensions(mut self: Pin<&mut Self>) {
        let mut names = QStringList::default();
        for (e, status) in self.extension_list() {
            if status == atlas_core::extensions::Status::Untrusted {
                names.append(qs(&e.name));
            }
        }
        self.as_mut().rust_mut().untrusted_extensions = names;
        self.untrusted_extensions_changed();
    }

    pub fn set_extension_setting(mut self: Pin<&mut Self>, name: &QString, key: &QString, value: &QString) -> QString {
        let (name, key) = (name.to_string(), key.to_string());
        let Some((ext, _)) = self.extension_list().into_iter().find(|(e, _)| e.name == name) else {
            return qs(format!("Die Erweiterung „{name}“ gibt es nicht (mehr)."));
        };
        let config_dir = atlas_core::config::config_dir();
        let result = match atlas_core::extensions::save_setting_json(&config_dir, &ext, &key, &value.to_string()) {
            Ok(()) => QString::default(),
            Err(err) => qs(format!("{err:#}")),
        };
        self.as_mut().refresh_extensions();
        result
    }

    pub fn bundled_extensions(&self) -> QString {
        let root = atlas_core::extensions::user_root(&atlas_core::config::config_dir());
        let list: Vec<_> = atlas_core::extensions::BUNDLED
            .iter()
            .map(|b| {
                serde_json::json!({
                    "name": b.name, "title": b.title, "description": b.description,
                    "installed": root.join(b.name).join(atlas_core::extensions::MANIFEST).is_file(),
                })
            })
            .collect();
        qs(serde_json::Value::from(list).to_string())
    }

    pub fn install_bundled(mut self: Pin<&mut Self>, name: &QString) -> QString {
        let root = atlas_core::extensions::user_root(&atlas_core::config::config_dir());
        let result = match atlas_core::extensions::install_bundled(&root, &name.to_string()) {
            Ok(_) => QString::default(),
            Err(err) => qs(format!("{err:#}")),
        };
        self.as_mut().refresh_extensions();
        result
    }

    pub fn uninstall_extension(mut self: Pin<&mut Self>, dir: &QString) -> QString {
        use atlas_core::extensions;
        let dir = path_of(dir);
        let result = (|| -> anyhow::Result<()> {
            let (ext, _) = self
                .extension_list()
                .into_iter()
                .find(|(e, _)| e.dir == dir)
                .ok_or_else(|| anyhow::anyhow!("Unter {} ist keine Erweiterung (mehr).", dir.display()))?;
            atlas_core::fileops::trash(std::slice::from_ref(&ext.dir))?;
            let mut trust = extensions::Trust::load(&atlas_core::paths::state_root());
            extensions::forget(&ext, &atlas_core::config::config_dir(), &mut trust)?;
            // A project extension of the same name may still be there.
            if !self.extension_list().iter().any(|(e, _)| e.name == ext.name) {
                let mut config = Config::load().extensions;
                config.disabled.retain(|n| n != &ext.name);
                Config::save_extensions(&config)?;
            }
            crate::qt_helpers::set_extension_dir(&qs(&ext.name), &QString::default());
            Ok(())
        })();
        self.as_mut().refresh_extensions();
        match result {
            Ok(()) => QString::default(),
            Err(err) => qs(format!("{err:#}")),
        }
    }

    pub fn extension_views(&self) -> QString {
        let list: Vec<_> = self
            .active_extensions()
            .iter()
            .flat_map(|e| e.views.iter())
            .map(|v| serde_json::json!({ "id": v.id, "name": v.name, "extension": v.extension, "keybinding": v.keybinding, "window": v.window }))
            .collect();
        qs(serde_json::Value::from(list).to_string())
    }

    pub fn extension_view(&self, id: &QString) -> QString {
        let id = id.to_string();
        let found = self
            .active_extensions()
            .into_iter()
            .find_map(|e| e.views.iter().find(|v| v.id == id).cloned().map(|v| (v, e)));
        let result = match found {
            None => {
                serde_json::json!({ "error": format!("Die Ansicht „{id}“ ist nicht (mehr) aktiv – Erweiterung ausgeschaltet, nicht freigegeben oder entfernt.") })
            }
            Some((view, ext)) => {
                crate::qt_helpers::set_extension_dir(&qs(&ext.name), &qs(ext.dir.to_string_lossy()));
                let page: String = view.page.split('/').map(url_escape).collect::<Vec<_>>().join("/");
                serde_json::json!({
                    "url": format!("atlas://app/ext/{}/{page}", ext.name),
                    "name": view.name,
                    "extension": ext.name,
                    "network": ext.permissions.as_ref().is_some_and(|p| !p.network.is_empty()),
                    "window": view.window,
                })
            }
        };
        qs(result.to_string())
    }

    pub fn extension_call(&self, name: &QString, method: &QString, params: &QString) -> QString {
        let name = name.to_string();
        let result = (|| -> anyhow::Result<serde_json::Value> {
            let ext = self
                .active_extensions()
                .into_iter()
                .find(|e| e.name == name)
                .ok_or_else(|| anyhow::anyhow!("Die Erweiterung „{name}“ ist nicht aktiv."))?;
            let params: serde_json::Value = serde_json::from_str(&params.to_string()).unwrap_or_default();
            let settings = atlas_core::extensions::load_settings(&atlas_core::config::config_dir());
            atlas_core::extensions::call(&ext, self.root.as_deref(), &settings, &method.to_string(), &params)
        })();
        qs(match result {
            Ok(value) => serde_json::json!({ "ok": true, "value": value }),
            Err(err) => serde_json::json!({ "ok": false, "error": format!("{err:#}") }),
        }
        .to_string())
    }

    pub fn keybindings(&self) -> QString {
        let (actions, _) = self.load_actions();
        let mut seen = std::collections::HashSet::new();
        let mut list: Vec<_> = actions
            .iter()
            .filter_map(|a| a.keybinding.as_ref().map(|k| (k, a)))
            .filter(|(k, _)| seen.insert(k.to_string()))
            .map(|(k, a)| serde_json::json!({ "sequence": k, "id": a.id, "name": a.name }))
            .collect();
        for ext in self.active_extensions() {
            for view in ext.views.iter().filter(|v| v.keybinding.as_ref().is_some_and(|k| seen.insert(k.clone()))) {
                list.push(serde_json::json!({ "sequence": view.keybinding, "id": view.id, "name": view.name }));
            }
        }
        qs(serde_json::Value::from(list).to_string())
    }

    pub fn trust_extension(mut self: Pin<&mut Self>, name: &QString) -> bool {
        let name = name.to_string();
        let mut trust = atlas_core::extensions::Trust::load(&atlas_core::paths::state_root());
        let Some((ext, _)) = self.extension_list().into_iter().find(|(e, _)| e.name == name) else { return false };
        let ok = match trust.trust(&ext) {
            Ok(()) => true,
            Err(err) => {
                log::warn!("cannot store trust: {err:#}");
                false
            }
        };
        self.as_mut().refresh_extensions();
        ok
    }

    pub fn set_extension_enabled(mut self: Pin<&mut Self>, name: &QString, enabled: bool) -> QString {
        let name = name.to_string();
        let mut config = Config::load().extensions;
        config.disabled.retain(|n| n != &name);
        if !enabled {
            config.disabled.push(name);
        }
        let result = match Config::save_extensions(&config) {
            Ok(()) => QString::default(),
            Err(err) => qs(format!("{err:#}")),
        };
        self.as_mut().refresh_extensions();
        result
    }

    pub fn new_extension(&self, title: &QString) -> QString {
        let root = atlas_core::extensions::user_root(&atlas_core::config::config_dir());
        let (base, text) = atlas_core::extensions::starter(&title.to_string());
        let (mut name, mut n) = (base.clone(), 2);
        while root.join(&name).exists() {
            name = format!("{base}-{n}");
            n += 1;
        }
        let manifest = root.join(&name).join(atlas_core::extensions::MANIFEST);
        let text = text.replace(&format!("name: {base}\n"), &format!("name: {name}\n"));
        match atlas_core::paths::write_atomic(&manifest, text.as_bytes()) {
            Ok(()) => qs(manifest.to_string_lossy()),
            Err(err) => {
                log::warn!("cannot create {}: {err:#}", manifest.display());
                QString::default()
            }
        }
    }

    fn processed_cache(&self) -> PathBuf {
        self.project
            .as_ref()
            .map(|p| p.cache_dir())
            .unwrap_or_else(|| atlas_core::paths::state_root().join("cache"))
            .join("processed")
    }

    pub fn processors_for(&self, path: &QString) -> QString {
        let file = path_of(path);
        let list: Vec<_> = self
            .active_extensions()
            .iter()
            .flat_map(|e| e.processors.iter())
            .filter(|p| p.applies(&file, self.root.as_deref()))
            .map(|p| {
                serde_json::json!({ "id": p.id, "name": p.name, "extension": p.extension, "output": p.output.as_str(), "auto": p.auto })
            })
            .chain(
                self.active_extensions()
                    .iter()
                    .flat_map(|e| e.views.iter())
                    .filter(|v| v.applies(&file, self.root.as_deref()))
                    .map(|v| serde_json::json!({ "id": v.id, "name": v.name, "extension": v.extension, "output": "view", "auto": v.auto })),
            )
            .collect();
        qs(serde_json::Value::from(list).to_string())
    }

    pub fn process_file(&self, id: &QString, path: &QString) {
        use atlas_core::extensions::Processed;
        let id_str = id.to_string();
        let file = path_of(path);
        let processor = self.active_extensions().into_iter().flat_map(|e| e.processors).find(|p| p.id == id_str);
        let (root, cache) = (self.root.clone(), self.processed_cache());
        let (id, path) = (id.clone(), path.clone());
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let result = match processor {
                None => Err(anyhow::anyhow!("Die Verarbeitung „{id_str}“ ist nicht (mehr) aktiv.")),
                Some(p) => p.process(&file, root.as_deref(), &cache),
            };
            let (kind, content, error) = match result {
                Ok(Processed::Markdown(text)) => ("markdown", text, String::new()),
                Ok(Processed::Text(text)) => ("text", text, String::new()),
                Ok(Processed::Pdf(pdf)) => ("pdf", pdf.to_string_lossy().into_owned(), String::new()),
                Err(err) => ("", String::new(), format!("{err:#}")),
            };
            let _ = thread.queue(move |app| app.file_processed(path, id, qs(kind), qs(content), qs(error)));
        });
    }

    pub fn exporters_for(&self, paths: &QStringList) -> QString {
        let files: Vec<PathBuf> = paths.iter().map(path_of).collect();
        let [file] = files.as_slice() else { return qs("[]") };
        let list: Vec<_> = self
            .active_extensions()
            .iter()
            .flat_map(|e| e.exporters.iter())
            .filter(|x| x.applies(file, self.root.as_deref()))
            .map(|x| {
                serde_json::json!({
                    "id": x.id, "name": x.name, "extension": x.extension, "target": x.target,
                    "output": x.default_output(file).to_string_lossy(),
                })
            })
            .collect();
        qs(serde_json::Value::from(list).to_string())
    }

    pub fn export_file(&self, id: &QString, path: &QString, output: &QString) {
        let id = id.to_string();
        let (file, out) = (path_of(path), path_of(output));
        let exporter = self.active_extensions().into_iter().flat_map(|e| e.exporters).find(|x| x.id == id);
        let root = self.root.clone();
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let (ok, message, open) = match exporter {
                None => (false, format!("Der Exporter „{id}“ ist nicht (mehr) aktiv."), false),
                Some(x) => match x.export(&file, &out, root.as_deref()) {
                    Ok(()) => (
                        true,
                        format!("„{}“ exportiert: {}", x.name, out.file_name().unwrap_or_default().to_string_lossy()),
                        x.open,
                    ),
                    Err(err) => (false, format!("{err:#}"), false),
                },
            };
            let output = qs(out.to_string_lossy());
            let _ = thread.queue(move |app| app.file_exported(output, ok, qs(message), open));
        });
    }
}
