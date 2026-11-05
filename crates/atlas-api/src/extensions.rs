// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Extensions: list, trust, enable, settings (secrets in the keychain /
//! systemd-creds), bundled ones, views (web pages in a `WebView` talking to
//! the core through [`Extensions::call`]), processors and exporters.
//! Template: `app/extensions.rs`.
//!
//! Requests from a view's page and their results stay JSON: they are
//! defined by the extension, not by Atlas (`docs/extensions.md`).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use atlas_core::config::Config;
use atlas_core::extensions::{self as core, Extension, Location, Status};

use crate::events::Event;
use crate::inner::Shared;
use crate::util::background;
use crate::{AtlasError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ExtensionLocation {
    User,
    Project,
    /// Installed from the extensions that come with Atlas.
    Bundled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ExtensionStatus {
    Active,
    Disabled,
    /// A project extension not trusted in its current form.
    Untrusted,
    /// The manifest has errors.
    Broken,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum SettingKind {
    Text,
    Secret,
    Bool,
    Number,
    Choice,
    Path,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ExtensionSetting {
    pub key: String,
    pub title: String,
    pub description: String,
    pub kind: SettingKind,
    /// Allowed values of `Choice`.
    pub choices: Vec<String>,
    /// Current value as JSON (`"abc"`, `true`, `3`); secrets: `true` if one
    /// is stored, else `false`.
    pub value: String,
    /// Default as JSON (`null` for secrets).
    pub default_value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ExtensionInfo {
    pub name: String,
    pub title: String,
    pub version: String,
    pub description: String,
    pub dir: String,
    pub manifest: String,
    pub location: ExtensionLocation,
    pub status: ExtensionStatus,
    pub errors: Vec<String>,
    /// Names of its actions, processors, exporters, views (for the details).
    pub actions: Vec<String>,
    pub processors: Vec<String>,
    pub exporters: Vec<String>,
    pub views: Vec<String>,
    pub settings: Vec<ExtensionSetting>,
    /// Requested permissions, as shown in the trust dialog; empty for an
    /// extension of API 1 (may run anything the user can).
    pub permissions: Vec<String>,
    /// API version of the manifest.
    pub api: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct BundledExtension {
    pub name: String,
    pub title: String,
    pub description: String,
    pub installed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ViewInfo {
    /// `view:<extension>/<id>`
    pub id: String,
    pub name: String,
    pub extension: String,
    pub keybinding: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ViewPage {
    /// `file://` URL of the page.
    pub url: String,
    /// The extension folder (the page may only read below it).
    pub dir: String,
    pub name: String,
    pub extension: String,
    /// The page may load from the network.
    pub network: bool,
}

/// What a processor turns a file into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ProcessedKind {
    Markdown,
    Text,
    Pdf,
    /// A view of the extension shows the file (open [`Extensions::view`]).
    View,
}

/// A way to show a file Atlas has no viewer for.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ProcessorInfo {
    pub id: String,
    pub name: String,
    pub extension: String,
    pub output: ProcessedKind,
    /// Used at once when such a file opens.
    pub auto: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Processed {
    pub kind: ProcessedKind,
    /// Markdown or text; for `Pdf` the path of the produced PDF.
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ExporterInfo {
    pub id: String,
    pub name: String,
    pub extension: String,
    /// File extension of the result (`pdf`).
    pub target: String,
    /// Proposed result path.
    pub output: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Exported {
    pub message: String,
    /// Open the result in Atlas.
    pub open: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum CardEntryKind {
    /// Opens [`Extensions::view`].
    View,
    /// Runs the action (`Actions::prepare`).
    Command,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct CardEntry {
    pub id: String,
    pub name: String,
    pub kind: CardEntryKind,
    pub keybinding: String,
}

/// An active extension's card in the context bar: its views and palette
/// commands. The bar lists it as block `ext:<name>`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ExtensionCard {
    pub name: String,
    pub title: String,
    pub description: String,
    /// Shown as a card (the user's choice, else what the extension asks for).
    pub shown: bool,
    pub entries: Vec<CardEntry>,
}

#[cfg_attr(feature = "ffi", derive(uniffi::Object))]
pub struct Extensions {
    shared: Arc<Shared>,
}

impl Extensions {
    pub(crate) fn new(shared: Arc<Shared>) -> Self {
        Self { shared }
    }
}

#[cfg_attr(feature = "ffi", uniffi::export)]
impl Extensions {
    pub fn list(&self) -> Vec<ExtensionInfo> {
        let all = core::load_settings(&atlas_core::config::config_dir());
        extension_list(&self.shared)
            .iter()
            .map(|(e, status)| {
                let settings = e
                    .setting_values(&all)
                    .into_iter()
                    .map(|(s, value)| ExtensionSetting {
                        key: s.key.clone(),
                        title: s.title.clone(),
                        description: s.description.clone(),
                        kind: setting_kind(s.kind),
                        choices: s.choices.clone(),
                        value: serde_json::to_string(&value).unwrap_or_default(),
                        default_value: if s.kind == core::SettingKind::Secret {
                            "null".into()
                        } else {
                            serde_json::to_string(&s.default).unwrap_or_default()
                        },
                    })
                    .collect();
                info(e, *status, settings)
            })
            .collect()
    }

    /// Project extensions waiting for trust.
    pub fn untrusted(&self) -> Vec<String> {
        extension_list(&self.shared).into_iter().filter(|(_, s)| *s == Status::Untrusted).map(|(e, _)| e.name).collect()
    }

    /// Trusts the extension in its current form (project extensions: until
    /// a file of it changes).
    pub fn trust(&self, name: String) -> Result<()> {
        let ext = self.find(&name)?;
        let mut trust = core::Trust::load(&atlas_core::paths::state_root());
        trust.trust(&ext).map_err(|e| failed("Freigabe kann nicht gespeichert werden", e))?;
        self.changed();
        Ok(())
    }

    pub fn set_enabled(&self, name: String, enabled: bool) -> Result<()> {
        let mut config = Config::load().extensions;
        config.disabled.retain(|n| n != &name);
        if !enabled {
            config.disabled.push(name);
        }
        Config::save_extensions(&config).map_err(|e| failed("Einstellungen können nicht gespeichert werden", e))?;
        self.changed();
        Ok(())
    }

    /// `value` as JSON (`"abc"`, `true`, `3`); `None` resets it. Secrets go
    /// to the secret store.
    pub fn set_setting(&self, name: String, key: String, value: Option<String>) -> Result<()> {
        let ext = self.find(&name)?;
        let json = value.unwrap_or_else(|| "null".into());
        core::save_setting_json(&atlas_core::config::config_dir(), &ext, &key, &json)
            .map_err(|e| AtlasError::Invalid { message: format!("{e:#}") })?;
        self.changed();
        Ok(())
    }

    /// New user extension from the template; returns the manifest path.
    pub fn create(&self, title: String) -> Result<String> {
        let root = core::user_root(&atlas_core::config::config_dir());
        let (base, text) = core::starter(&title);
        let (mut name, mut n) = (base.clone(), 2);
        while root.join(&name).exists() {
            name = format!("{base}-{n}");
            n += 1;
        }
        let manifest = root.join(&name).join(core::MANIFEST);
        let text = text.replace(&format!("name: {base}\n"), &format!("name: {name}\n"));
        atlas_core::paths::write_atomic(&manifest, text.as_bytes())
            .map_err(|e| failed("Erweiterung kann nicht angelegt werden", e))?;
        self.changed();
        Ok(manifest.to_string_lossy().into_owned())
    }

    pub fn bundled(&self) -> Vec<BundledExtension> {
        let root = core::user_root(&atlas_core::config::config_dir());
        core::BUNDLED
            .iter()
            .map(|b| BundledExtension {
                name: b.name.into(),
                title: b.title.into(),
                description: b.description.into(),
                installed: root.join(b.name).join(core::MANIFEST).is_file(),
            })
            .collect()
    }

    /// Installs (or updates) a bundled extension; its settings stay.
    pub fn install_bundled(&self, name: String) -> Result<()> {
        if !core::BUNDLED.iter().any(|b| b.name == name) {
            return Err(AtlasError::NotFound { message: format!("Keine mitgelieferte Erweiterung „{name}“") });
        }
        let root = core::user_root(&atlas_core::config::config_dir());
        core::install_bundled(&root, &name).map_err(|e| failed("Installieren fehlgeschlagen", e))?;
        self.changed();
        Ok(())
    }

    /// The folder goes to the trash; settings, secrets and trust are forgotten.
    pub fn uninstall(&self, dir: String) -> Result<()> {
        let dir = PathBuf::from(dir);
        let (ext, _) = extension_list(&self.shared).into_iter().find(|(e, _)| e.dir == dir).ok_or_else(|| {
            AtlasError::NotFound { message: format!("Unter {} ist keine Erweiterung (mehr).", dir.display()) }
        })?;
        let result = (|| -> anyhow::Result<()> {
            atlas_core::fileops::trash(std::slice::from_ref(&ext.dir))?;
            let mut trust = core::Trust::load(&atlas_core::paths::state_root());
            core::forget(&ext, &atlas_core::config::config_dir(), &mut trust)?;
            // A project extension of the same name may still be there.
            if !extension_list(&self.shared).iter().any(|(e, _)| e.name == ext.name) {
                let mut config = Config::load().extensions;
                config.disabled.retain(|n| n != &ext.name);
                Config::save_extensions(&config)?;
            }
            Ok(())
        })();
        self.changed();
        result.map_err(|e| failed("Entfernen fehlgeschlagen", e))
    }

    /// Views of the active extensions.
    pub fn views(&self) -> Vec<ViewInfo> {
        active_extensions(&self.shared)
            .iter()
            .flat_map(|e| e.views.iter())
            .map(|v| ViewInfo {
                id: v.id.clone(),
                name: v.name.clone(),
                extension: v.extension.clone(),
                keybinding: v.keybinding.clone().unwrap_or_default(),
            })
            .collect()
    }

    pub fn view(&self, id: String) -> Result<ViewPage> {
        let (view, ext) = active_extensions(&self.shared)
            .into_iter()
            .find_map(|e| e.views.iter().find(|v| v.id == id).cloned().map(|v| (v, e)))
            .ok_or_else(|| AtlasError::NotFound {
                message: format!(
                    "Die Ansicht „{id}“ ist nicht (mehr) aktiv – Erweiterung ausgeschaltet, nicht freigegeben oder entfernt."
                ),
            })?;
        let page = ext.dir.join(&view.page);
        Ok(ViewPage {
            url: file_url(&page),
            dir: ext.dir.to_string_lossy().into_owned(),
            name: view.name,
            extension: ext.name.clone(),
            network: ext.permissions.as_ref().is_some_and(|p| !p.network.is_empty()),
        })
    }

    /// A request of a view's page: `params` and the result are JSON.
    pub async fn call(&self, extension: String, method: String, params: String) -> Result<String> {
        let ext = active_extensions(&self.shared).into_iter().find(|e| e.name == extension).ok_or_else(|| {
            AtlasError::Unavailable { message: format!("Die Erweiterung „{extension}“ ist nicht aktiv.") }
        })?;
        let params: serde_json::Value = serde_json::from_str(&params)
            .map_err(|e| AtlasError::Invalid { message: format!("Parameter sind kein JSON: {e}") })?;
        let root = self.shared.root.clone();
        background(move || {
            let settings = core::load_settings(&atlas_core::config::config_dir());
            core::call(&ext, Some(&root), &settings, &method, &params)
                .map(|v| v.to_string())
                .map_err(|e| AtlasError::Failed { message: format!("{e:#}") })
        })
        .await
    }

    /// Processors and views that can show `path` (absolute).
    pub fn processors_for(&self, path: String) -> Vec<ProcessorInfo> {
        let file = PathBuf::from(path);
        let root = Some(self.shared.root.as_path());
        let active = active_extensions(&self.shared);
        let processors =
            active.iter().flat_map(|e| e.processors.iter()).filter(|p| p.applies(&file, root)).map(|p| ProcessorInfo {
                id: p.id.clone(),
                name: p.name.clone(),
                extension: p.extension.clone(),
                output: match p.output {
                    core::OutputKind::Markdown => ProcessedKind::Markdown,
                    core::OutputKind::Text => ProcessedKind::Text,
                    core::OutputKind::Pdf => ProcessedKind::Pdf,
                },
                auto: p.auto,
            });
        let views =
            active.iter().flat_map(|e| e.views.iter()).filter(|v| v.applies(&file, root)).map(|v| ProcessorInfo {
                id: v.id.clone(),
                name: v.name.clone(),
                extension: v.extension.clone(),
                output: ProcessedKind::View,
                auto: v.auto,
            });
        processors.chain(views).collect()
    }

    /// Runs processor `id` on `path` (absolute).
    pub async fn process(&self, id: String, path: String) -> Result<Processed> {
        let processor =
            active_extensions(&self.shared).into_iter().flat_map(|e| e.processors).find(|p| p.id == id).ok_or_else(
                || AtlasError::Unavailable { message: format!("Die Verarbeitung „{id}“ ist nicht (mehr) aktiv.") },
            )?;
        let root = self.shared.root.clone();
        let cache = self.processed_cache();
        background(move || {
            let out = processor
                .process(Path::new(&path), Some(&root), &cache)
                .map_err(|e| AtlasError::Failed { message: format!("{e:#}") })?;
            Ok(match out {
                core::Processed::Markdown(text) => Processed { kind: ProcessedKind::Markdown, content: text },
                core::Processed::Text(text) => Processed { kind: ProcessedKind::Text, content: text },
                core::Processed::Pdf(pdf) => {
                    Processed { kind: ProcessedKind::Pdf, content: pdf.to_string_lossy().into_owned() }
                }
            })
        })
        .await
    }

    /// Exporters for `path` (one file, absolute).
    pub fn exporters_for(&self, path: String) -> Vec<ExporterInfo> {
        let file = PathBuf::from(path);
        let root = Some(self.shared.root.as_path());
        active_extensions(&self.shared)
            .iter()
            .flat_map(|e| e.exporters.iter())
            .filter(|x| x.applies(&file, root))
            .map(|x| ExporterInfo {
                id: x.id.clone(),
                name: x.name.clone(),
                extension: x.extension.clone(),
                target: x.target.clone(),
                output: x.default_output(&file).to_string_lossy().into_owned(),
            })
            .collect()
    }

    /// Writes `output` from `path` with exporter `id`.
    pub async fn export(&self, id: String, path: String, output: String) -> Result<Exported> {
        let exporter =
            active_extensions(&self.shared).into_iter().flat_map(|e| e.exporters).find(|x| x.id == id).ok_or_else(
                || AtlasError::Unavailable { message: format!("Der Exporter „{id}“ ist nicht (mehr) aktiv.") },
            )?;
        let root = self.shared.root.clone();
        background(move || {
            let out = PathBuf::from(&output);
            exporter
                .export(Path::new(&path), &out, Some(&root))
                .map_err(|e| AtlasError::Failed { message: format!("{e:#}") })?;
            Ok(Exported {
                message: format!(
                    "„{}“ exportiert: {}",
                    exporter.name,
                    out.file_name().unwrap_or_default().to_string_lossy()
                ),
                open: exporter.open,
            })
        })
        .await
    }

    /// Cards of the active extensions for the context bar (those with views
    /// or palette commands).
    pub fn context_cards(&self) -> Vec<ExtensionCard> {
        let config = Config::load().context;
        active_extensions(&self.shared)
            .iter()
            .filter(|e| offers_card(e))
            .map(|e| {
                let views = e.views.iter().map(|v| CardEntry {
                    id: v.id.clone(),
                    name: v.name.clone(),
                    kind: CardEntryKind::View,
                    keybinding: v.keybinding.clone().unwrap_or_default(),
                });
                let commands = e.actions.iter().filter(|a| a.in_palette).map(|a| CardEntry {
                    id: a.id.clone(),
                    name: a.name.clone(),
                    kind: CardEntryKind::Command,
                    keybinding: a.keybinding.clone().unwrap_or_default(),
                });
                ExtensionCard {
                    name: e.name.clone(),
                    title: e.title.clone(),
                    description: e.description.clone(),
                    shown: config.extensions.get(&e.name).copied().unwrap_or(e.context),
                    entries: views.chain(commands).collect(),
                }
            })
            .collect()
    }

    /// Shows or hides an extension's card in the context bar (all profiles).
    pub fn set_card_shown(&self, name: String, shown: bool) -> Result<()> {
        let mut config = Config::load().context;
        config.extensions.insert(name, shown);
        Config::save_context(&config).map_err(|e| failed("Einstellungen können nicht gespeichert werden", e))?;
        self.shared.emit(Event::SettingsChanged);
        Ok(())
    }

    /// Reads the extensions again (after changes on disk).
    pub fn reload(&self) {
        self.changed();
    }
}

impl Extensions {
    fn find(&self, name: &str) -> Result<Extension> {
        extension_list(&self.shared).into_iter().find(|(e, _)| e.name == name).map(|(e, _)| e).ok_or_else(|| {
            AtlasError::NotFound { message: format!("Die Erweiterung „{name}“ gibt es nicht (mehr).") }
        })
    }

    fn changed(&self) {
        self.shared.emit(Event::ExtensionsChanged);
    }

    fn processed_cache(&self) -> PathBuf {
        self.shared
            .project
            .as_ref()
            .map(|p| p.cache_dir())
            .unwrap_or_else(|| atlas_core::paths::state_root().join("cache"))
            .join("processed")
    }
}

fn failed(what: &str, err: anyhow::Error) -> AtlasError {
    AtlasError::Failed { message: format!("{what}: {err:#}") }
}

/// `file://` URL of an absolute path (percent-encoded per segment).
fn file_url(path: &Path) -> String {
    let escaped: Vec<String> = path
        .to_string_lossy()
        .split('/')
        .map(|segment| {
            segment
                .bytes()
                .map(|b| match b {
                    b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
                    _ => format!("%{b:02X}"),
                })
                .collect()
        })
        .collect();
    format!("file://{}", escaped.join("/"))
}

/// All extensions with their status and the user's settings. A key
/// sequence that an action or an earlier extension already uses is dropped
/// (with an error).
pub(crate) fn extension_list(shared: &Shared) -> Vec<(Extension, Status)> {
    let config_dir = atlas_core::config::config_dir();
    let atlas_dir = shared.project.as_ref().map(|p| p.atlas_dir());
    let project_root = atlas_dir.as_deref().map(core::project_root);
    let found = core::discover(&core::user_root(&config_dir), project_root.as_deref());
    let trust = core::Trust::load(&atlas_core::paths::state_root());
    let disabled = Config::load().extensions.disabled;
    let settings = core::load_settings(&config_dir);
    let (own, _) = atlas_core::actions::load(atlas_dir.as_deref(), &config_dir);
    let mut taken: Vec<(String, String)> =
        own.iter().filter_map(|a| a.keybinding.clone().map(|k| (k, a.source.label()))).collect();
    found
        .into_iter()
        .map(|mut e| {
            e.apply_settings(&settings);
            let status = core::status(&e, &trust, &disabled);
            let mut errors = Vec::new();
            let owner_name = e.name.clone();
            let mut claim = |name: &str, key: &mut Option<String>| {
                let Some(k) = key.clone() else { return };
                if let Some((_, owner)) = taken.iter().find(|(t, _)| *t == k) {
                    errors.push(format!("„{name}“: {k} ist schon belegt ({owner})"));
                    *key = None;
                } else if status == Status::Active {
                    taken.push((k, format!("Erweiterung {owner_name}")));
                }
            };
            for action in &mut e.actions {
                claim(&action.name.clone(), &mut action.keybinding);
            }
            for view in &mut e.views {
                claim(&view.name.clone(), &mut view.keybinding);
            }
            e.errors.extend(errors);
            (e, status)
        })
        .collect()
}

/// An extension that can show a card: it has views or palette commands.
fn offers_card(e: &Extension) -> bool {
    !e.views.is_empty() || e.actions.iter().any(|a| a.in_palette)
}

/// The extensions as the context bar sees them (`ext:<name>` blocks).
pub(crate) fn bar_extensions(
    shared: &Shared,
    config: &atlas_core::config::ContextConfig,
) -> Vec<atlas_core::context_profiles::Extension> {
    active_extensions(shared)
        .iter()
        .filter(|e| offers_card(e))
        .map(|e| atlas_core::context_profiles::Extension {
            name: e.name.clone(),
            shown: config.extensions.get(&e.name).copied().unwrap_or(e.context),
        })
        .collect()
}

pub(crate) fn active_extensions(shared: &Shared) -> Vec<Extension> {
    extension_list(shared).into_iter().filter(|(_, s)| *s == Status::Active).map(|(e, _)| e).collect()
}

fn setting_kind(kind: core::SettingKind) -> SettingKind {
    match kind {
        core::SettingKind::String => SettingKind::Text,
        core::SettingKind::Path => SettingKind::Path,
        core::SettingKind::Bool => SettingKind::Bool,
        core::SettingKind::Number => SettingKind::Number,
        core::SettingKind::Choice => SettingKind::Choice,
        core::SettingKind::Secret => SettingKind::Secret,
    }
}

fn info(e: &Extension, status: Status, settings: Vec<ExtensionSetting>) -> ExtensionInfo {
    let bundled = e.location == Location::User && core::BUNDLED.iter().any(|b| b.name == e.name);
    ExtensionInfo {
        name: e.name.clone(),
        title: e.title.clone(),
        version: e.version.clone(),
        description: e.description.clone(),
        dir: e.dir.to_string_lossy().into_owned(),
        manifest: e.dir.join(core::MANIFEST).to_string_lossy().into_owned(),
        location: match e.location {
            Location::Project => ExtensionLocation::Project,
            Location::User if bundled => ExtensionLocation::Bundled,
            Location::User => ExtensionLocation::User,
        },
        status: match status {
            Status::Active => ExtensionStatus::Active,
            Status::Disabled => ExtensionStatus::Disabled,
            Status::Untrusted => ExtensionStatus::Untrusted,
            Status::Broken => ExtensionStatus::Broken,
        },
        errors: e.errors.clone(),
        actions: e.actions.iter().map(|a| a.name.clone()).collect(),
        processors: e.processors.iter().map(|p| p.name.clone()).collect(),
        exporters: e.exporters.iter().map(|x| x.name.clone()).collect(),
        views: e.views.iter().map(|v| v.name.clone()).collect(),
        settings,
        permissions: e
            .permissions
            .as_ref()
            .map(|p| {
                let mut out = Vec::new();
                if !p.run.is_empty() {
                    out.push(format!("Programme: {}", p.run.join(", ")));
                }
                out.push(format!(
                    "Dateien: {}",
                    match p.files {
                        core::FileAccess::None => "keine",
                        core::FileAccess::Project => "das Projekt",
                        core::FileAccess::Home => "der persönliche Ordner",
                        core::FileAccess::All => "alle",
                    }
                ));
                if !p.network.is_empty() {
                    out.push(format!("Netzwerk: {}", p.network.join(", ")));
                }
                out
            })
            .unwrap_or_default(),
        api: e.api,
    }
}
