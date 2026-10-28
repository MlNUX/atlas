// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! A small, declarative extension system.
//!
//! An extension is a folder with an `extension.yaml` (API version 1 or 2,
//! see docs/extensions.md) – for the user in
//! `$XDG_CONFIG_HOME/atlas/extensions/` or for one project in
//! `.atlas/extensions/`. It can contribute
//!
//! - **commands** (actions in the format of `actions.yaml`, with a key
//!   sequence and a place in the menus),
//! - **processors** that show files Atlas cannot display itself (a command
//!   turns them into Markdown, text or a PDF),
//! - **exporters** that write a file in another format, and
//! - **settings** the user fills in (in Atlas), passed to the commands.
//!
//! Version 2 also declares **permissions** (programs it runs, files,
//! network); the programs are checked when the manifest is read, the rest
//! is shown before trusting the extension.
//!
//! Extensions cannot change the user interface. Commands follow the rules
//! of actions: program plus arguments, a shell only with `shell: true`,
//! placeholders filled in after splitting. Extensions that come with a
//! project run only after the user trusted them; any changed file of the
//! extension needs trust again.

use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};
use yaml_rust2::Yaml;

use crate::actions::{self, Action, Command, Context, LoadError, Source};

/// Newest extension API this Atlas understands (`atlas_api:`).
pub const API_VERSION: i64 = 2;
pub const MANIFEST: &str = "extension.yaml";
pub const DIR: &str = "extensions";
/// User settings of extensions, in the config folder.
pub const SETTINGS_FILE: &str = "extension-settings.toml";
/// Output of a processor or exporter that is kept (and shown on errors).
const MAX_OUTPUT: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Location {
    /// Installed by the user (trusted).
    User,
    /// Comes with the project (needs trust).
    Project,
}

/// What a processor produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputKind {
    Markdown,
    Text,
    Pdf,
}

impl OutputKind {
    pub fn as_str(self) -> &'static str {
        match self {
            OutputKind::Markdown => "markdown",
            OutputKind::Text => "text",
            OutputKind::Pdf => "pdf",
        }
    }
    fn file_extension(self) -> &'static str {
        match self {
            OutputKind::Markdown => "md",
            OutputKind::Text => "txt",
            OutputKind::Pdf => "pdf",
        }
    }
}

/// Which files an extension says it reads or writes (shown, not enforced).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileAccess {
    None,
    Project,
    Home,
    All,
}

impl FileAccess {
    pub fn as_str(self) -> &'static str {
        match self {
            FileAccess::None => "none",
            FileAccess::Project => "project",
            FileAccess::Home => "home",
            FileAccess::All => "all",
        }
    }
}

/// What an extension of API version 2 says it needs. Programs are checked
/// when the manifest is read: a command may only start a program from
/// `run`, a file of the extension, or one the user chose in a setting.
/// Files and network are shown before trusting the extension (not yet
/// enforced: scripts of the extension may do more).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Permissions {
    pub run: Vec<String>,
    pub files: FileAccess,
    /// Hosts it connects to; `["*"]` for any.
    pub network: Vec<String>,
}

/// Kind of a setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingKind {
    String,
    Path,
    Bool,
    Number,
    Choice,
    /// Kept encrypted like the Nextcloud password; only passed in the
    /// environment of commands that do not run in a terminal.
    Secret,
}

impl SettingKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SettingKind::String => "string",
            SettingKind::Path => "path",
            SettingKind::Bool => "bool",
            SettingKind::Number => "number",
            SettingKind::Choice => "choice",
            SettingKind::Secret => "secret",
        }
    }
}

/// A value the user sets for an extension (`settings:` in the manifest).
#[derive(Debug, Clone, PartialEq)]
pub struct Setting {
    pub key: String,
    pub title: String,
    pub description: String,
    pub kind: SettingKind,
    /// Default (never set for secrets).
    pub default: toml::Value,
    pub choices: Vec<String>,
}

impl Setting {
    /// `ATLAS_SETTING_<KEY>`.
    pub fn env_name(&self) -> String {
        format!("ATLAS_SETTING_{}", self.key.to_ascii_uppercase().replace('-', "_"))
    }

    /// Whether `value` fits this setting.
    pub fn accepts(&self, value: &toml::Value) -> bool {
        match (self.kind, value) {
            (SettingKind::String | SettingKind::Path, toml::Value::String(_)) => true,
            (SettingKind::Bool, toml::Value::Boolean(_)) => true,
            (SettingKind::Number, toml::Value::Integer(_) | toml::Value::Float(_)) => true,
            (SettingKind::Choice, toml::Value::String(s)) => self.choices.contains(s),
            _ => false,
        }
    }
}

/// A setting value as text (placeholders and environment).
fn setting_text(value: &toml::Value) -> String {
    match value {
        toml::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Shows files of a type Atlas has no viewer for.
#[derive(Debug, Clone)]
pub struct Processor {
    pub id: String,
    pub name: String,
    pub extension: String,
    pub applies_to: Vec<String>,
    pub command: Command,
    pub output: OutputKind,
    pub cwd: Option<String>,
    pub timeout: Duration,
    /// Used automatically when such a file is opened (otherwise offered).
    pub auto: bool,
    pub base_dir: PathBuf,
    pub settings: Applied,
    globs: globset::GlobSet,
}

/// Writes a file in another format.
#[derive(Debug, Clone)]
pub struct Exporter {
    pub id: String,
    pub name: String,
    pub extension: String,
    pub applies_to: Vec<String>,
    pub command: Command,
    /// File extension of the result (`pdf`).
    pub target: String,
    pub cwd: Option<String>,
    pub timeout: Duration,
    /// Open the result in Atlas afterwards.
    pub open: bool,
    pub base_dir: PathBuf,
    pub settings: Applied,
    globs: globset::GlobSet,
}

/// Setting values for one command: placeholders, environment, and the
/// secrets that are read just before it runs (environment name, store).
#[derive(Debug, Clone, Default)]
pub struct Applied {
    pub extra: Vec<(String, String)>,
    pub env: Vec<(String, String)>,
    pub secrets: Vec<(String, String)>,
}

impl Applied {
    fn placeholders(&self, output: &Path, base_dir: &Path) -> Vec<(String, String)> {
        let mut extra = vec![
            ("{output}".to_owned(), output.to_string_lossy().into_owned()),
            ("{extension}".to_owned(), base_dir.to_string_lossy().into_owned()),
        ];
        extra.extend(self.extra.iter().cloned());
        extra
    }

    /// The environment, with the secrets read now.
    fn environment(&self) -> Result<Vec<(String, String)>> {
        let mut env = self.env.clone();
        for (name, store) in &self.secrets {
            if let Some((secret, _)) = crate::secret::SecretStore::default_for(store).load()? {
                env.push((name.clone(), secret));
            }
        }
        Ok(env)
    }
}

#[derive(Debug, Clone)]
pub struct Extension {
    pub name: String,
    pub title: String,
    pub version: String,
    pub description: String,
    /// API version of the manifest (1 or 2; 0 if unreadable).
    pub api: i64,
    /// Declared permissions (API 2); `None`: an extension of version 1,
    /// which may run anything the user can.
    pub permissions: Option<Permissions>,
    pub settings: Vec<Setting>,
    pub dir: PathBuf,
    pub location: Location,
    /// What trust is bound to: for project extensions BLAKE3 over every
    /// file of the folder (see [`folder_hash`]), for user extensions over
    /// the manifest. Empty if it could not be computed.
    pub hash: String,
    pub actions: Vec<Action>,
    pub processors: Vec<Processor>,
    pub exporters: Vec<Exporter>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Active,
    Disabled,
    /// A project extension the user has not trusted (yet, or since it changed).
    Untrusted,
    /// The manifest could not be read at all.
    Broken,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Active => "active",
            Status::Disabled => "disabled",
            Status::Untrusted => "untrusted",
            Status::Broken => "broken",
        }
    }
}

pub const TEMPLATE: &str = r#"# Atlas-Erweiterung (Format: docs/extensions.md)
atlas_api: 2
name: beispiel
title: Beispiel-Erweiterung
version: 0.1.0
description: Zeigt, was eine Erweiterung beitragen kann.

# Was die Erweiterung braucht; Atlas zeigt es vor der Freigabe. Befehle
# dürfen nur Programme aus „run“ starten (und Dateien dieses Ordners).
permissions:
  run: [wc, od, pandoc]
  files: project            # none, project, home oder all
  network: false            # true oder Liste von Rechnern

# Befehle: wie Aktionen in actions.yaml; {extension} ist dieser Ordner.
commands:
  - name: Wörter zählen
    applies_to: "*.md"
    command: [wc, -w, "{file}"]
    keybinding: Ctrl+Alt+W    # optional
    menu: [context, palette]  # Standard: beides

# Einstellungen: Atlas zeigt sie in „Erweiterungen“; in Befehlen als
# {setting:schlüssel}, in der Umgebung als ATLAS_SETTING_SCHLÜSSEL.
settings:
  - key: format
    title: Exportformat
    type: choice              # string, path, bool, number, choice, secret
    choices: [html, docx]
    default: html

# Dateiverarbeitung: Dateien anzeigen, für die Atlas keine Ansicht hat.
# Ausgabe: markdown, text (Standardausgabe oder {output}) oder pdf ({output}).
processors:
  - name: Hexdump
    applies_to: ["*.bin"]
    output: text
    command: [od, -A, x, -t, x1z, "{file}"]

# Exporter: schreiben eine Datei in einem anderen Format nach {output}.
exporters:
  - name: Export (pandoc)
    applies_to: "*.md"
    target: html
    command: [pandoc, "{file}", --standalone, -o, "{output}"]
"#;

/// A new extension called `title`: its name (folder, ID prefix) and its
/// manifest – the template with the examples commented out, so a new
/// extension adds nothing until it is filled in.
pub fn starter(title: &str) -> (String, String) {
    let title = title.trim();
    let name = match slug(title) {
        s if s.is_empty() => "erweiterung".to_owned(),
        s => s,
    };
    let mut out = String::new();
    let mut examples = false;
    for line in TEMPLATE.lines() {
        let line = match line {
            "name: beispiel" => format!("name: {name}"),
            "title: Beispiel-Erweiterung" => format!("title: {}", yaml_text(title)),
            l if l.starts_with("description:") => "description: \"\"".to_owned(),
            "  run: [wc, od, pandoc]" => "  run: []".to_owned(),
            l => l.to_owned(),
        };
        examples |= line.starts_with("# Befehle");
        if examples && !line.is_empty() && !line.starts_with('#') {
            out.push_str("# ");
        }
        out.push_str(&line);
        out.push('\n');
    }
    (name, out)
}

/// `text` as a YAML scalar (quoted if needed).
fn yaml_text(text: &str) -> String {
    if text.chars().all(|c| c.is_alphanumeric() || c == ' ' || c == '-') && !text.is_empty() {
        text.to_owned()
    } else {
        format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

// ------------------------------------------------------------------ loading

fn globs(patterns: &[String]) -> Result<globset::GlobSet> {
    let mut builder = globset::GlobSetBuilder::new();
    for p in patterns {
        builder.add(
            globset::GlobBuilder::new(p)
                .literal_separator(true)
                .case_insensitive(true)
                .build()
                .with_context(|| format!("ungültiges Muster „{p}“"))?,
        );
    }
    Ok(builder.build()?)
}

fn strings(value: &Yaml) -> Vec<String> {
    match value {
        Yaml::String(s) => vec![s.clone()],
        Yaml::Array(items) => items.iter().filter_map(|i| i.as_str().map(str::to_owned)).collect(),
        _ => Vec::new(),
    }
}

fn command(entry: &Yaml) -> Result<Command> {
    let shell = entry["shell"].as_bool().unwrap_or(false);
    Ok(match &entry["command"] {
        Yaml::String(s) if shell => Command::Shell(s.clone()),
        Yaml::String(s) => Command::Argv(crate::config::split_words(s).context("„command“ ist nicht lesbar")?),
        Yaml::Array(items) if !shell => Command::Argv(
            items
                .iter()
                .map(|i| i.as_str().map(str::to_owned))
                .collect::<Option<Vec<_>>>()
                .context("„command“ darf nur Texte enthalten")?,
        ),
        Yaml::Array(_) => bail!("mit „shell: true“ muss „command“ ein Text sein"),
        _ => bail!("„command“ fehlt"),
    })
}

fn slug(name: &str) -> String {
    name.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-")
}

fn timeout(entry: &Yaml, default: u64) -> Duration {
    Duration::from_secs(entry["timeout"].as_i64().filter(|t| *t > 0).map(|t| t as u64).unwrap_or(default))
}

fn parse_processor(entry: &Yaml, ext: &str, dir: &Path) -> Result<Processor> {
    let name = entry["name"].as_str().filter(|n| !n.trim().is_empty()).context("„name“ fehlt")?.trim();
    let applies_to = strings(&entry["applies_to"]);
    if applies_to.is_empty() {
        bail!("„applies_to“ fehlt");
    }
    let output = match entry["output"].as_str().unwrap_or("text") {
        "markdown" | "md" => OutputKind::Markdown,
        "text" | "txt" => OutputKind::Text,
        "pdf" => OutputKind::Pdf,
        other => bail!("unbekannte Ausgabe „{other}“ (markdown, text, pdf)"),
    };
    let command = command(entry)?;
    if output == OutputKind::Pdf && !uses(&command, "{output}") {
        bail!("PDF-Ausgabe braucht {{output}} im Befehl");
    }
    Ok(Processor {
        id: format!("{ext}/{}", entry["id"].as_str().map(str::to_owned).unwrap_or_else(|| slug(name))),
        name: name.to_owned(),
        extension: ext.to_owned(),
        globs: globs(&applies_to)?,
        applies_to,
        command,
        output,
        cwd: entry["cwd"].as_str().map(str::to_owned),
        timeout: timeout(entry, 30),
        auto: entry["auto"].as_bool().unwrap_or(true),
        base_dir: dir.to_path_buf(),
        settings: Applied::default(),
    })
}

fn parse_exporter(entry: &Yaml, ext: &str, dir: &Path) -> Result<Exporter> {
    let name = entry["name"].as_str().filter(|n| !n.trim().is_empty()).context("„name“ fehlt")?.trim();
    let target = entry["target"].as_str().map(|t| t.trim_start_matches('.').to_owned()).filter(|t| !t.is_empty());
    let target = target.context("„target“ (Dateiendung des Ergebnisses) fehlt")?;
    let command = command(entry)?;
    if !uses(&command, "{output}") {
        bail!("der Befehl muss nach {{output}} schreiben");
    }
    let applies_to = strings(&entry["applies_to"]);
    Ok(Exporter {
        id: format!("{ext}/{}", entry["id"].as_str().map(str::to_owned).unwrap_or_else(|| slug(name))),
        name: name.to_owned(),
        extension: ext.to_owned(),
        globs: globs(&applies_to)?,
        applies_to,
        command,
        target,
        cwd: entry["cwd"].as_str().map(str::to_owned),
        timeout: timeout(entry, 300),
        open: entry["open"].as_bool().unwrap_or(true),
        base_dir: dir.to_path_buf(),
        settings: Applied::default(),
    })
}

fn uses(command: &Command, placeholder: &str) -> bool {
    match command {
        Command::Argv(argv) => argv.iter().any(|a| a.contains(placeholder)),
        Command::Shell(s) => s.contains(placeholder),
    }
}

fn parse_permissions(doc: &Yaml) -> Result<Permissions> {
    let run = strings(&doc["run"]);
    if let Some(bad) = run.iter().find(|p| p.trim().is_empty() || p.contains(char::is_whitespace)) {
        bail!("„run“: „{bad}“ ist kein Programmname");
    }
    let files = match doc["files"].as_str().map(str::trim) {
        None if doc["files"].as_bool() == Some(false) => FileAccess::None,
        None | Some("project" | "projekt") => FileAccess::Project,
        Some("none" | "keine") => FileAccess::None,
        Some("home") => FileAccess::Home,
        Some("all" | "alle") => FileAccess::All,
        Some(other) => bail!("unbekanntes „files: {other}“ (none, project, home, all)"),
    };
    let network = match &doc["network"] {
        Yaml::Boolean(true) => vec!["*".to_owned()],
        Yaml::Boolean(false) | Yaml::BadValue | Yaml::Null => Vec::new(),
        other => strings(other),
    };
    Ok(Permissions { run, files, network })
}

fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 64
        && key.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

fn parse_setting(entry: &Yaml) -> Result<Setting> {
    let key = entry["key"].as_str().map(str::trim).context("„key“ fehlt")?;
    if !valid_key(key) {
        bail!("ungültiger Schlüssel „{key}“ (Kleinbuchstaben, Ziffern, - und _)");
    }
    let kind = match entry["type"].as_str().unwrap_or("string") {
        "string" | "text" => SettingKind::String,
        "path" | "pfad" => SettingKind::Path,
        "bool" | "boolean" => SettingKind::Bool,
        "number" | "zahl" => SettingKind::Number,
        "choice" | "auswahl" => SettingKind::Choice,
        "secret" | "passwort" => SettingKind::Secret,
        other => bail!("„{key}“: unbekannter Typ „{other}“ (string, path, bool, number, choice, secret)"),
    };
    let choices = strings(&entry["choices"]);
    if kind == SettingKind::Choice && choices.is_empty() {
        bail!("„{key}“: Auswahl ohne „choices“");
    }
    let default = match (&entry["default"], kind) {
        (Yaml::BadValue | Yaml::Null, SettingKind::Bool) => toml::Value::Boolean(false),
        (Yaml::BadValue | Yaml::Null, SettingKind::Number) => toml::Value::Integer(0),
        (Yaml::BadValue | Yaml::Null, SettingKind::Choice) => toml::Value::String(choices[0].clone()),
        (Yaml::BadValue | Yaml::Null, _) => toml::Value::String(String::new()),
        (_, SettingKind::Secret) => bail!("„{key}“: ein Geheimnis hat keinen Standardwert"),
        (Yaml::Boolean(b), _) => toml::Value::Boolean(*b),
        (Yaml::Integer(i), _) => toml::Value::Integer(*i),
        (Yaml::Real(r), _) => toml::Value::Float(r.parse().unwrap_or_default()),
        (Yaml::String(t), _) => toml::Value::String(t.clone()),
        _ => bail!("„{key}“: unlesbarer Standardwert"),
    };
    let setting = Setting {
        key: key.to_owned(),
        title: entry["title"].as_str().unwrap_or(key).to_owned(),
        description: entry["description"].as_str().unwrap_or_default().to_owned(),
        kind,
        default,
        choices,
    };
    if kind != SettingKind::Secret && !setting.accepts(&setting.default) {
        bail!("„{key}“: der Standardwert passt nicht zum Typ {}", kind.as_str());
    }
    Ok(setting)
}

/// `{setting:…}` placeholders used in `text`.
fn setting_placeholders(text: &str) -> Vec<&str> {
    text.match_indices("{setting:")
        .filter_map(|(i, _)| text[i + 9..].find('}').map(|end| &text[i + 9..i + 9 + end]))
        .collect()
}

/// Checks a command of an API 2 extension against its settings and
/// permissions: placeholders name settings (secrets only reach the
/// environment), and the program is declared in `run`, lies in the
/// extension folder, or is chosen by the user in a setting.
fn check_command(command: &Command, settings: &[Setting], permissions: &Permissions) -> Result<()> {
    let texts: Vec<&str> = match command {
        Command::Argv(argv) => argv.iter().map(String::as_str).collect(),
        Command::Shell(line) => vec![line.as_str()],
    };
    for key in texts.iter().flat_map(|t| setting_placeholders(t)) {
        match settings.iter().find(|s| s.key == key) {
            None => bail!("unbekannte Einstellung {{setting:{key}}}"),
            Some(s) if s.kind == SettingKind::Secret => {
                bail!("das Geheimnis „{key}“ gibt es nur als Umgebungsvariable {}", s.env_name())
            }
            Some(_) => {}
        }
    }
    let program = match command {
        Command::Shell(_) => "sh",
        Command::Argv(argv) => argv.first().map(String::as_str).unwrap_or_default(),
    };
    let allowed = permissions.run.iter().any(|p| p == program)
        || (program.starts_with("{extension}/") && !program.contains(".."))
        // A program the user chooses; its default must be declared too.
        || (program.starts_with("{setting:")
            && program.ends_with('}')
            && settings.iter().any(|s| {
                program[9..program.len() - 1] == s.key
                    && matches!(s.kind, SettingKind::String | SettingKind::Path)
                    && s.default.as_str().is_some_and(|d| d.is_empty() || permissions.run.iter().any(|p| p == d))
            }));
    if !allowed {
        bail!("„{program}“ fehlt in „permissions: run“");
    }
    Ok(())
}

fn valid_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 64 && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Reads one extension folder. Problems are collected in `errors`; an
/// unreadable manifest gives an extension without contributions.
pub fn load(dir: &Path, location: Location) -> Extension {
    let manifest = dir.join(MANIFEST);
    let fallback = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut ext = Extension {
        name: fallback.clone(),
        title: fallback.clone(),
        version: String::new(),
        description: String::new(),
        api: 0,
        permissions: None,
        settings: Vec::new(),
        dir: dir.to_path_buf(),
        location,
        hash: String::new(),
        actions: Vec::new(),
        processors: Vec::new(),
        exporters: Vec::new(),
        errors: Vec::new(),
    };
    let bytes = match std::fs::read(&manifest) {
        Ok(b) => b,
        Err(err) => {
            ext.errors.push(format!("{MANIFEST}: {err}"));
            return ext;
        }
    };
    ext.hash = match location {
        Location::User => blake3::hash(&bytes).to_hex().to_string(),
        Location::Project => match folder_hash(dir) {
            Ok(hash) => hash,
            Err(err) => {
                ext.errors.push(format!("{err:#}"));
                String::new()
            }
        },
    };
    let docs = match yaml_rust2::YamlLoader::load_from_str(&String::from_utf8_lossy(&bytes)) {
        Ok(d) => d,
        Err(err) => {
            ext.errors.push(format!("kein gültiges YAML: {err}"));
            return ext;
        }
    };
    let Some(doc) = docs.first() else {
        ext.errors.push("leeres Manifest".into());
        return ext;
    };
    // `atlas_extension: 1` is the name version 1 used.
    match doc["atlas_api"].as_i64().or_else(|| doc["atlas_extension"].as_i64()) {
        Some(v) if v > API_VERSION => {
            ext.errors.push(format!(
                "braucht eine neuere Atlas-Version (Erweiterungs-API {v}, diese Version kann {API_VERSION})"
            ));
            return ext;
        }
        Some(v) if v >= 1 => ext.api = v,
        _ => {
            ext.errors.push(format!("„atlas_api: {API_VERSION}“ fehlt"));
            return ext;
        }
    }
    let name = doc["name"].as_str().map(str::trim).unwrap_or(&fallback).to_owned();
    if !valid_name(&name) {
        ext.errors.push(format!("ungültiger Name „{name}“ (Buchstaben, Ziffern, - und _)"));
        return ext;
    }
    ext.name = name.clone();
    ext.title = doc["title"].as_str().unwrap_or(&name).to_owned();
    ext.version = doc["version"]
        .as_str()
        .map(str::to_owned)
        .or_else(|| doc["version"].as_f64().map(|v| v.to_string()))
        .unwrap_or_default();
    ext.description = doc["description"].as_str().unwrap_or_default().to_owned();

    if ext.api >= 2 {
        match parse_permissions(&doc["permissions"]) {
            Ok(p) => ext.permissions = Some(p),
            Err(err) => {
                // Without readable permissions nothing may run.
                ext.errors.push(format!("permissions: {err:#}"));
                return ext;
            }
        }
        for (i, entry) in doc["settings"].as_vec().into_iter().flatten().enumerate() {
            match parse_setting(entry) {
                Ok(s) if ext.settings.iter().any(|o| o.key == s.key) => {
                    ext.errors.push(format!("Einstellung {}: „{}“ gibt es zweimal", i + 1, s.key))
                }
                Ok(s) => ext.settings.push(s),
                Err(err) => ext.errors.push(format!("Einstellung {}: {err:#}", i + 1)),
            }
        }
    }

    let mut errors = Vec::new();
    let source = Source::Extension(name.clone());
    // Version 2 calls them commands; `actions:` still works.
    for list in ["actions", "commands"] {
        actions::parse_list(&doc[list], &source, &manifest, &mut ext.actions, &mut errors);
    }
    ext.errors.extend(errors.into_iter().map(|e: LoadError| e.message));
    for action in &mut ext.actions {
        action.base_dir = Some(dir.to_path_buf());
    }
    for (i, entry) in doc["processors"].as_vec().into_iter().flatten().enumerate() {
        match parse_processor(entry, &name, dir) {
            Ok(p) => ext.processors.push(p),
            Err(err) => ext.errors.push(format!("Verarbeitung {}: {err:#}", i + 1)),
        }
    }
    for (i, entry) in doc["exporters"].as_vec().into_iter().flatten().enumerate() {
        match parse_exporter(entry, &name, dir) {
            Ok(e) => ext.exporters.push(e),
            Err(err) => ext.errors.push(format!("Exporter {}: {err:#}", i + 1)),
        }
    }
    if let Some(permissions) = &ext.permissions {
        let settings = &ext.settings;
        let mut errors = Vec::new();
        let mut check = |what: String, command: &Command| match check_command(command, settings, permissions) {
            Ok(()) => true,
            Err(err) => {
                errors.push(format!("{what}: {err:#}"));
                false
            }
        };
        ext.actions.retain(|a| check(format!("Befehl „{}“", a.name), &a.command));
        ext.processors.retain(|p| check(format!("Verarbeitung „{}“", p.name), &p.command));
        ext.exporters.retain(|x| check(format!("Exporter „{}“", x.name), &x.command));
        ext.errors.extend(errors);
    }
    ext
}

// ----------------------------------------------------------------- settings

pub fn settings_file(config_dir: &Path) -> PathBuf {
    config_dir.join(SETTINGS_FILE)
}

/// What the user set for all extensions: extension name → key → value.
pub fn load_settings(config_dir: &Path) -> toml::Table {
    let file = settings_file(config_dir);
    match std::fs::read_to_string(&file) {
        Ok(text) => text.parse().unwrap_or_else(|err| {
            log::warn!("{} is not valid TOML: {err}", file.display());
            toml::Table::new()
        }),
        Err(_) => toml::Table::new(),
    }
}

/// Where the secret `key` of extension `ext` is kept.
pub fn secret_store(ext: &str, key: &str) -> String {
    format!("extension-{ext}-{key}")
}

/// Sets (or with `None` resets) one value; secrets go to the secret store,
/// never into the file.
pub fn save_setting(config_dir: &Path, ext: &Extension, key: &str, value: Option<toml::Value>) -> Result<()> {
    let setting = ext.settings.iter().find(|s| s.key == key).with_context(|| format!("keine Einstellung „{key}“"))?;
    if setting.kind == SettingKind::Secret {
        let store = crate::secret::SecretStore::default_for(&secret_store(&ext.name, key));
        return match value {
            Some(toml::Value::String(secret)) if !secret.is_empty() => store.store(&secret).map(|_| ()),
            None | Some(toml::Value::String(_)) => store.forget(),
            Some(_) => bail!("ein Geheimnis ist ein Text"),
        };
    }
    if let Some(v) = &value
        && !setting.accepts(v)
    {
        bail!("„{v}“ passt nicht zu „{}“", setting.title);
    }
    let mut all = load_settings(config_dir);
    let table = all.entry(ext.name.clone()).or_insert_with(|| toml::Value::Table(toml::Table::new()));
    let toml::Value::Table(table) = table else { bail!("[{}] in {SETTINGS_FILE} ist keine Tabelle", ext.name) };
    match value {
        Some(v) => table.insert(key.to_owned(), v),
        None => table.remove(key),
    };
    if table.is_empty() {
        all.remove(&ext.name);
    }
    crate::paths::write_atomic(&settings_file(config_dir), toml::to_string_pretty(&all)?.as_bytes())
}

/// [`save_setting`] with the value as JSON (`null` resets it).
pub fn save_setting_json(config_dir: &Path, ext: &Extension, key: &str, json: &str) -> Result<()> {
    let value = match serde_json::from_str::<serde_json::Value>(json).context("kein gültiger Wert")? {
        serde_json::Value::Null => None,
        v => Some(toml::Value::try_from(v).context("kein gültiger Wert")?),
    };
    save_setting(config_dir, ext, key, value)
}

impl Extension {
    /// Current value of each setting (secrets: whether one is stored).
    pub fn setting_values(&self, all: &toml::Table) -> Vec<(&Setting, toml::Value)> {
        let mine = all.get(&self.name).and_then(toml::Value::as_table);
        self.settings
            .iter()
            .map(|s| {
                let value = if s.kind == SettingKind::Secret {
                    let store = crate::secret::SecretStore::default_for(&secret_store(&self.name, &s.key));
                    toml::Value::Boolean(store.kind().is_some())
                } else {
                    mine.and_then(|t| t.get(&s.key)).filter(|v| s.accepts(v)).cloned().unwrap_or(s.default.clone())
                };
                (s, value)
            })
            .collect()
    }

    /// Hands the setting values to every command of the extension.
    pub fn apply_settings(&mut self, all: &toml::Table) {
        let mut applied = Applied::default();
        for (setting, value) in self.setting_values(all) {
            if setting.kind == SettingKind::Secret {
                applied.secrets.push((setting.env_name(), secret_store(&self.name, &setting.key)));
            } else {
                let text = setting_text(&value);
                applied.extra.push((format!("{{setting:{}}}", setting.key), text.clone()));
                applied.env.push((setting.env_name(), text));
            }
        }
        for action in &mut self.actions {
            action.extra = applied.extra.clone();
            action.env = applied.env.clone();
        }
        for p in &mut self.processors {
            p.settings = applied.clone();
        }
        for x in &mut self.exporters {
            x.settings = applied.clone();
        }
    }
}

fn scan(root: &Path, location: Location, out: &mut Vec<Extension>) {
    let Ok(entries) = std::fs::read_dir(root) else { return };
    let mut dirs: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.join(MANIFEST).is_file()).collect();
    dirs.sort();
    for dir in dirs {
        out.push(load(&dir, location));
    }
}

/// All extensions: the project's first; a user extension with the same
/// name as a project extension is left out.
pub fn discover(user_root: &Path, project_root: Option<&Path>) -> Vec<Extension> {
    let mut out = Vec::new();
    if let Some(root) = project_root {
        scan(root, Location::Project, &mut out);
    }
    let mut user = Vec::new();
    scan(user_root, Location::User, &mut user);
    for ext in user {
        if !out.iter().any(|e| e.name == ext.name) {
            out.push(ext);
        }
    }
    out
}

/// Title the extension template of Atlas ≤ 1.0 left in every new extension
/// (folders `beispiel`, `beispiel-2`, …).
const LEGACY_TITLE: &str = "title: Beispiel-Erweiterung";

/// Gives extensions created from the old template a real name: the first
/// becomes „Core“ (`core/`), further ones „Core 2“ … – folder, `name:` and
/// `title:` change, the rest of the manifest stays. Returns the renamed
/// extensions as (old name, new name), e.g. to carry over switched-off ones.
pub fn migrate_legacy(user_root: &Path) -> Vec<(String, String)> {
    let Ok(entries) = std::fs::read_dir(user_root) else { return Vec::new() };
    let mut legacy: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|dir| {
            dir.file_name().is_some_and(|n| n.to_string_lossy().starts_with("beispiel"))
                && std::fs::read_to_string(dir.join(MANIFEST))
                    .is_ok_and(|t| t.lines().any(|l| l.trim_end() == LEGACY_TITLE))
        })
        .collect();
    // `beispiel` before `beispiel-2` before `beispiel-10`.
    legacy.sort_by_key(|d| {
        let name = d.file_name().unwrap_or_default().to_string_lossy().into_owned();
        (name.rsplit('-').next().and_then(|n| n.parse::<u32>().ok()).unwrap_or(1), name)
    });
    let mut renamed = Vec::new();
    let mut n = 1;
    for dir in legacy {
        let (name, title) = loop {
            let candidate = if n == 1 {
                ("core".to_owned(), "Core".to_owned())
            } else {
                (format!("core-{n}"), format!("Core {n}"))
            };
            n += 1;
            if !user_root.join(&candidate.0).exists() {
                break candidate;
            }
        };
        let Ok(text) = std::fs::read_to_string(dir.join(MANIFEST)) else { continue };
        let old = dir.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let text: String = text
            .lines()
            .map(|l| {
                if l.trim_end() == LEGACY_TITLE {
                    format!("title: {title}")
                } else if l.starts_with("name: ") {
                    format!("name: {name}")
                } else {
                    l.to_owned()
                }
            })
            .map(|l| l + "\n")
            .collect();
        let target = user_root.join(&name);
        if std::fs::rename(&dir, &target).is_err() {
            continue;
        }
        if let Err(err) = crate::paths::write_atomic(&target.join(MANIFEST), text.as_bytes()) {
            log::warn!("cannot rename extension {old}: {err:#}");
        }
        renamed.push((old, name));
    }
    renamed
}

/// The test plugin that comes with Atlas (`crates/atlas-core/test-plugin/`):
/// file name, content, executable.
const TEST_PLUGIN: &[(&str, &str, bool)] = &[
    (MANIFEST, include_str!("../test-plugin/extension.yaml"), false),
    ("gruss.sh", include_str!("../test-plugin/gruss.sh"), true),
    ("zaehlen.sh", include_str!("../test-plugin/zaehlen.sh"), true),
    ("anzeigen.sh", include_str!("../test-plugin/anzeigen.sh"), true),
    ("export.sh", include_str!("../test-plugin/export.sh"), true),
];
pub const TEST_PLUGIN_NAME: &str = "test-plugin";

/// Installs (or updates) the test plugin among the user's extensions;
/// returns its folder. Its settings are kept.
pub fn install_test_plugin(user_root: &Path) -> Result<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let dir = user_root.join(TEST_PLUGIN_NAME);
    for (name, content, executable) in TEST_PLUGIN {
        let path = dir.join(name);
        crate::paths::write_atomic(&path, content.as_bytes())?;
        let mode = if *executable { 0o755 } else { 0o644 };
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode))
            .with_context(|| format!("{} kann nicht geändert werden", path.display()))?;
    }
    Ok(dir)
}

pub fn user_root(config_dir: &Path) -> PathBuf {
    config_dir.join(DIR)
}

pub fn project_root(atlas_dir: &Path) -> PathBuf {
    atlas_dir.join(DIR)
}

// -------------------------------------------------------------------- trust

/// Largest extension folder that is checked (files, bytes).
const MAX_FILES: usize = 10_000;
const MAX_BYTES: u64 = 512 * 1024 * 1024;

/// Content hash of a file, remembered by its metadata: a change of the
/// content always changes the ctime, which (unlike the mtime) cannot be set
/// back by hand.
fn file_hash(path: &Path, meta: &std::fs::Metadata) -> Result<String> {
    use std::os::unix::fs::MetadataExt;
    type Seen = HashMap<PathBuf, ((u64, u64, i64, i64, i64, i64), String)>;
    static SEEN: std::sync::OnceLock<std::sync::Mutex<Seen>> = std::sync::OnceLock::new();
    let key = (
        meta.dev(),
        meta.ino(),
        meta.size() as i64,
        meta.mtime_nsec() + meta.mtime() * 1_000_000_000,
        meta.ctime(),
        meta.ctime_nsec(),
    );
    let seen = SEEN.get_or_init(Default::default);
    if let Some((k, hash)) = seen.lock().ok().and_then(|s| s.get(path).cloned())
        && k == key
    {
        return Ok(hash);
    }
    let mut hasher = blake3::Hasher::new();
    hasher.update_reader(std::fs::File::open(path)?)?;
    let hash = hasher.finalize().to_hex().to_string();
    if let Ok(mut s) = seen.lock() {
        s.insert(path.to_path_buf(), (key, hash.clone()));
    }
    Ok(hash)
}

/// BLAKE3 over the whole extension folder: every file's relative path,
/// executable bit and content, every symbolic link's target (a link to a
/// file also with that file's content, a link to a folder with everything
/// in it) – so a trusted extension whose script changed is untrusted again,
/// not only one whose manifest changed.
pub fn folder_hash(dir: &Path) -> Result<String> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"atlas-extension-folder-v2\0");
    let (mut files, mut bytes) = (0usize, 0u64);
    // Folders on the current path, so that a link loop ends.
    let mut stack = vec![(PathBuf::new(), Vec::<(u64, u64)>::new())];
    while let Some((rel, mut parents)) = stack.pop() {
        let meta = std::fs::metadata(dir.join(&rel))?;
        if parents.contains(&(meta.dev(), meta.ino())) {
            hasher.update(b"C");
            hasher.update(rel.as_os_str().as_encoded_bytes());
            hasher.update(b"\0");
            continue;
        }
        parents.push((meta.dev(), meta.ino()));
        let mut entries: Vec<_> = std::fs::read_dir(dir.join(&rel))
            .with_context(|| format!("{} nicht lesbar", dir.join(&rel).display()))?
            .collect::<std::io::Result<_>>()?;
        entries.sort_by_key(|e| e.file_name());
        // Depth first, in name order (sub folders after the files).
        let mut subdirs = Vec::new();
        for entry in entries {
            let rel = rel.join(entry.file_name());
            let path = entry.path();
            let link = std::fs::symlink_metadata(&path)?;
            let kind = if link.file_type().is_symlink() {
                hasher.update(b"L");
                hasher.update(rel.as_os_str().as_encoded_bytes());
                hasher.update(b"\0");
                hasher.update(std::fs::read_link(&path)?.as_os_str().as_encoded_bytes());
                hasher.update(b"\0");
                // What it points to: a file counts with its content, a
                // folder with everything in it.
                match std::fs::metadata(&path) {
                    Ok(m) if m.is_file() => Some(m),
                    Ok(m) if m.is_dir() => {
                        subdirs.push(rel.clone());
                        None
                    }
                    _ => None,
                }
            } else if link.is_dir() {
                subdirs.push(rel.clone());
                None
            } else if link.is_file() {
                Some(link)
            } else {
                None
            };
            if let Some(meta) = kind {
                files += 1;
                bytes += meta.len();
                if files > MAX_FILES || bytes > MAX_BYTES {
                    bail!("Erweiterung zu groß zum Prüfen (mehr als {MAX_FILES} Dateien oder {} MB)", MAX_BYTES >> 20);
                }
                hasher.update(b"F");
                hasher.update(rel.as_os_str().as_encoded_bytes());
                hasher.update(if meta.permissions().mode() & 0o111 != 0 { b"\0x\0" } else { b"\0-\0" });
                hasher.update(file_hash(&path, &meta)?.as_bytes());
            }
        }
        subdirs.reverse();
        stack.extend(subdirs.into_iter().map(|rel| (rel, parents.clone())));
    }
    Ok(format!("v2:{}", hasher.finalize().to_hex()))
}

/// Project extensions the user trusted: folder → manifest hash, in
/// `$XDG_STATE_HOME/atlas/trusted-extensions.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Trust {
    #[serde(default)]
    trusted: BTreeMap<String, String>,
    #[serde(skip)]
    file: PathBuf,
}

impl Trust {
    pub fn load(state_dir: &Path) -> Self {
        let file = state_dir.join("trusted-extensions.json");
        let mut trust: Trust =
            std::fs::read_to_string(&file).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        trust.file = file;
        trust
    }

    pub fn is_trusted(&self, ext: &Extension) -> bool {
        ext.location == Location::User || self.trusted.get(&ext.dir.to_string_lossy().into_owned()) == Some(&ext.hash)
    }

    /// Trusts the extension in its current form.
    pub fn trust(&mut self, ext: &Extension) -> Result<()> {
        self.trusted.insert(ext.dir.to_string_lossy().into_owned(), ext.hash.clone());
        crate::paths::write_atomic(&self.file, serde_json::to_string_pretty(self)?.as_bytes())
    }

    pub fn revoke(&mut self, ext: &Extension) -> Result<()> {
        self.trusted.remove(&ext.dir.to_string_lossy().into_owned());
        crate::paths::write_atomic(&self.file, serde_json::to_string_pretty(self)?.as_bytes())
    }
}

pub fn status(ext: &Extension, trust: &Trust, disabled: &[String]) -> Status {
    if ext.hash.is_empty()
        || (ext.actions.is_empty() && ext.processors.is_empty() && ext.exporters.is_empty() && !ext.errors.is_empty())
    {
        Status::Broken
    } else if disabled.contains(&ext.name) {
        Status::Disabled
    } else if !trust.is_trusted(ext) {
        Status::Untrusted
    } else {
        Status::Active
    }
}

// ------------------------------------------------------------------ running

/// Result of a finished command.
#[derive(Debug, Clone)]
pub struct Finished {
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: String,
}

/// Runs a built command without terminal: no input, output captured, killed
/// after `timeout`.
pub fn run(built: &actions::Built, timeout: Duration) -> Result<Finished> {
    use std::process::{Command as Process, Stdio};
    let (program, args) = built.argv.split_first().context("leerer Befehl")?;
    let mut child = Process::new(program)
        .args(args)
        .envs(built.env.iter().map(|(k, v)| (k, v)))
        .current_dir(&built.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("„{program}“ kann nicht gestartet werden"))?;
    let reader = |pipe: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(pipe) = pipe {
                let _ = pipe.take(MAX_OUTPUT as u64).read_to_end(&mut buf);
            }
            buf
        })
    };
    let out = reader(child.stdout.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
    let err = reader(child.stderr.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            bail!("„{program}“ hat nach {} s nicht geantwortet und wurde beendet", timeout.as_secs());
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let stdout = out.join().unwrap_or_default();
    let stderr = String::from_utf8_lossy(&err.join().unwrap_or_default()).into_owned();
    Ok(Finished { code: status.code(), stdout, stderr })
}

fn failure(name: &str, finished: &Finished) -> anyhow::Error {
    let tail: String =
        finished.stderr.lines().rev().take(8).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
    match finished.code {
        Some(code) => anyhow::anyhow!("„{name}“ ist mit Code {code} beendet:\n{tail}"),
        None => anyhow::anyhow!("„{name}“ wurde abgebrochen:\n{tail}"),
    }
}

/// Converted content of a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Processed {
    Markdown(String),
    Text(String),
    Pdf(PathBuf),
}

fn matches(globs: &globset::GlobSet, file: &Path, project: Option<&Path>) -> bool {
    let name = file.file_name().map(Path::new).unwrap_or(file);
    let relative = project.and_then(|r| file.strip_prefix(r).ok()).unwrap_or(file);
    globs.is_match(name) || globs.is_match(relative)
}

impl Processor {
    pub fn applies(&self, file: &Path, project: Option<&Path>) -> bool {
        matches(&self.globs, file, project)
    }

    /// Converts `file`; results are cached in `cache_dir` by processor and
    /// file content, so reopening is instant and an edited file is redone.
    pub fn process(&self, file: &Path, project: Option<&Path>, cache_dir: &Path) -> Result<Processed> {
        let mut hasher = blake3::Hasher::new();
        hasher.update(self.id.as_bytes());
        hasher.update(format!("{:?}{:?}", self.command, self.settings.env).as_bytes());
        let source =
            std::fs::File::open(file).with_context(|| format!("{} kann nicht gelesen werden", file.display()))?;
        hasher.update_reader(source)?;
        let key = hasher.finalize().to_hex()[..32].to_owned();
        let cached = cache_dir.join(format!("{key}.{}", self.output.file_extension()));
        if !cached.exists() {
            std::fs::create_dir_all(cache_dir)
                .with_context(|| format!("{} kann nicht angelegt werden", cache_dir.display()))?;
            // Unique per run: the same file may be converted twice at once.
            static RUN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let run_id = RUN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let tmp =
                cache_dir.join(format!(".{key}.tmp-{}-{run_id}.{}", std::process::id(), self.output.file_extension()));
            let ctx = Context { project: project.map(Path::to_path_buf), paths: vec![file.to_path_buf()], line: 0 };
            let extra = self.settings.placeholders(&tmp, &self.base_dir);
            let mut built = actions::build(&self.command, self.cwd.as_deref(), &ctx, &extra)?;
            built.env = self.settings.environment()?;
            let finished = run(&built, self.timeout)?;
            if finished.code != Some(0) {
                let _ = std::fs::remove_file(&tmp);
                return Err(failure(&self.name, &finished));
            }
            if !tmp.exists() {
                if self.output == OutputKind::Pdf {
                    bail!("„{}“ hat keine PDF geschrieben", self.name);
                }
                std::fs::write(&tmp, &finished.stdout)
                    .with_context(|| format!("{} kann nicht geschrieben werden", tmp.display()))?;
            }
            if let Err(err) = std::fs::rename(&tmp, &cached) {
                let _ = std::fs::remove_file(&tmp);
                // Another run finished first: its result is just as good.
                if !cached.exists() {
                    return Err(
                        anyhow::Error::new(err).context(format!("{} kann nicht abgelegt werden", cached.display()))
                    );
                }
            }
        }
        Ok(match self.output {
            OutputKind::Pdf => Processed::Pdf(cached),
            OutputKind::Markdown => Processed::Markdown(String::from_utf8_lossy(&std::fs::read(&cached)?).into_owned()),
            OutputKind::Text => Processed::Text(String::from_utf8_lossy(&std::fs::read(&cached)?).into_owned()),
        })
    }
}

impl Exporter {
    pub fn applies(&self, file: &Path, project: Option<&Path>) -> bool {
        file.is_file() && (self.applies_to.is_empty() || matches(&self.globs, file, project))
    }

    /// Proposed result: next to the file with the target extension.
    pub fn default_output(&self, file: &Path) -> PathBuf {
        file.with_extension(&self.target)
    }

    /// Writes `output`. The result is produced in a temporary file next to
    /// it and renamed at the end, so a failed export leaves nothing half
    /// written and never touches an existing file.
    pub fn export(&self, file: &Path, output: &Path, project: Option<&Path>) -> Result<()> {
        if output == file {
            bail!("Das Ergebnis darf die Quelldatei nicht ersetzen.");
        }
        let dir = output.parent().context("Ziel ohne Ordner")?;
        let tmp = dir.join(format!(
            ".{}.atlas-export-{}.{}",
            output.file_stem().unwrap_or_default().to_string_lossy(),
            std::process::id(),
            self.target
        ));
        let ctx = Context { project: project.map(Path::to_path_buf), paths: vec![file.to_path_buf()], line: 0 };
        let extra = self.settings.placeholders(&tmp, &self.base_dir);
        let mut built = actions::build(&self.command, self.cwd.as_deref(), &ctx, &extra)?;
        built.env = self.settings.environment()?;
        let finished = run(&built, self.timeout);
        let finished = match finished {
            Ok(f) => f,
            Err(err) => {
                let _ = std::fs::remove_file(&tmp);
                return Err(err);
            }
        };
        if finished.code != Some(0) {
            let _ = std::fs::remove_file(&tmp);
            return Err(failure(&self.name, &finished));
        }
        if !tmp.exists() {
            bail!("„{}“ hat keine Datei geschrieben", self.name);
        }
        std::fs::rename(&tmp, output).with_context(|| format!("{} kann nicht geschrieben werden", output.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_ext(root: &Path, dir: &str, yaml: &str) -> PathBuf {
        let path = root.join(dir);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join(MANIFEST), yaml).unwrap();
        path
    }

    #[test]
    fn loads_manifests_with_contributions_and_errors() {
        let dir = tempfile::tempdir().unwrap();
        let user = dir.path().join("user");
        let project = dir.path().join("project");
        write_ext(&user, "beispiel", TEMPLATE);
        write_ext(&user, "kaputt", "name: kaputt\nactions: []\n");
        write_ext(
            &project,
            "proj",
            "atlas_extension: 1\nname: proj\nprocessors:\n- {name: Ohne Muster, command: [cat]}\n\
             - {name: Text, applies_to: \"*.log\", command: [cat, \"{file}\"]}\nexporters:\n\
             - {name: Ohne Ziel, command: [cp, \"{file}\", \"{output}\"]}\n- {name: Kopie, target: bak, command: [cp, \"{file}\"]}\n",
        );
        write_ext(
            &project,
            "beispiel",
            "atlas_extension: 1\nname: beispiel\nactions:\n- {name: Projekt, command: ls}\n",
        );
        let found = discover(&user, Some(&project));
        let names: Vec<_> = found.iter().map(|e| (e.name.as_str(), e.location)).collect();
        assert_eq!(names, [("beispiel", Location::Project), ("proj", Location::Project), ("kaputt", Location::User)]);
        let proj = &found[1];
        assert_eq!(proj.processors.len(), 1);
        assert_eq!(proj.errors.len(), 3, "{:?}", proj.errors);
        assert!(proj.errors.iter().any(|e| e.contains("applies_to")));
        assert!(proj.errors.iter().any(|e| e.contains("target")));
        assert!(proj.errors.iter().any(|e| e.contains("{output}")));
        assert!(found[2].errors[0].contains("atlas_api"));

        // A new extension: own name, examples only as comments.
        let (name, text) = starter("Projekt-Werkzeuge: PDF");
        assert_eq!(name, "projekt-werkzeuge-pdf");
        write_ext(&user, &name, &text);
        let fresh = load(&user.join(&name), Location::User);
        assert_eq!(fresh.title, "Projekt-Werkzeuge: PDF", "{text}");
        assert!(
            fresh.errors.is_empty() && fresh.actions.is_empty() && fresh.processors.is_empty(),
            "{:?}",
            fresh.errors
        );
        assert!(text.contains("#   - name: Wörter zählen"), "{text}");
        let template = load(&user.join("beispiel"), Location::User);
        assert!(template.errors.is_empty(), "{:?}", template.errors);
        assert_eq!((template.actions.len(), template.processors.len(), template.exporters.len()), (1, 1, 1));
        assert_eq!(template.actions[0].id, "beispiel/wörter-zählen");
        assert_eq!(template.actions[0].base_dir.as_deref(), Some(user.join("beispiel").as_path()));
        assert_eq!(template.processors[0].id, "beispiel/hexdump");
    }

    #[test]
    fn test_plugin_installs_and_runs() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("extensions");
        let ext_dir = install_test_plugin(&root).unwrap();
        let mut ext = load(&ext_dir, Location::User);
        assert!(ext.errors.is_empty(), "{:?}", ext.errors);
        assert_eq!((ext.api, ext.settings.len(), ext.actions.len()), (2, 5, 2));
        assert_eq!(ext.actions[0].keybinding.as_deref(), Some("Ctrl+Alt+T"));
        let config = dir.path().join("config");
        save_setting(&config, &ext, "gruss", Some(toml::Value::String("Servus".into()))).unwrap();
        save_setting(&config, &ext, "gross", Some(toml::Value::Boolean(true))).unwrap();
        save_setting(&config, &ext, "wiederholungen", Some(toml::Value::Integer(2))).unwrap();
        ext.apply_settings(&load_settings(&config));

        let file = dir.path().join("probe.testplugin");
        std::fs::write(&file, "Inhalt").unwrap();
        let Processed::Markdown(md) = ext.processors[0].process(&file, None, &dir.path().join("cache")).unwrap() else {
            panic!("markdown expected")
        };
        assert!(md.contains("| Begrüßung | Servus |") && md.contains("| Token | nicht gesetzt |"), "{md}");

        let note = dir.path().join("notiz.md");
        std::fs::write(&note, "eins zwei\n").unwrap();
        let out = ext.exporters[0].default_output(&note);
        assert_eq!(out, dir.path().join("notiz.gross.txt"));
        ext.exporters[0].export(&note, &out, None).unwrap();
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "EINS ZWEI\n");

        // The greeting script with the settings in its environment.
        let action = &ext.actions[0];
        let ctx = Context { project: Some(dir.path().to_path_buf()), paths: Vec::new(), line: 0 };
        let mut extra = vec![("{extension}".to_owned(), ext_dir.to_string_lossy().into_owned())];
        extra.extend(action.extra.iter().cloned());
        let mut built = actions::build(&action.command, None, &ctx, &extra).unwrap();
        built.env = action.env.clone();
        let finished = run(&built, Duration::from_secs(10)).unwrap();
        let text = String::from_utf8_lossy(&finished.stdout);
        assert_eq!(text.matches("SERVUS aus dem Test-Plugin!").count(), 2, "{text}");

        // Installing again updates the files and keeps the settings.
        install_test_plugin(&root).unwrap();
        assert!(load_settings(&config).contains_key(TEST_PLUGIN_NAME));
    }

    #[test]
    fn api_2_declares_permissions_settings_and_keys() {
        let dir = tempfile::tempdir().unwrap();
        let ext_dir = write_ext(
            dir.path(),
            "nb",
            r#"atlas_api: 2
name: nb
permissions: {run: [cat, jupyter], files: home, network: [cloud.example.org]}
settings:
- {key: format, type: choice, choices: [html, md], default: md}
- {key: python, type: path, default: jupyter}
- {key: anders, type: path, default: /tmp/fremd}
- {key: token, type: secret}
- {key: zahl, type: number, default: nein}
commands:
- {name: Umwandeln, applies_to: "*.ipynb", command: [jupyter, nbconvert, --to, "{setting:format}", "{file}"], keybinding: strg+alt+j, menu: palette}
- {name: Python, command: ["{setting:python}", -V]}
- {name: Anders, command: ["{setting:anders}", -V]}
- {name: Fremd, command: [curl, x]}
- {name: Shell, shell: true, command: "cat {file}"}
- {name: Geheim, command: [cat, "{setting:token}"]}
- {name: Skript, command: ["{extension}/run.sh"]}
processors:
- {name: Zeigen, applies_to: "*.ipynb", output: text, command: [sh, -c, "echo $ATLAS_SETTING_FORMAT"]}
- {name: Katze, applies_to: "*.ipynb", output: text, command: [cat, "{file}"]}
"#,
        );
        let mut ext = load(&ext_dir, Location::User);
        assert_eq!(ext.api, 2);
        let permissions = ext.permissions.clone().unwrap();
        assert_eq!(permissions.files, FileAccess::Home);
        assert_eq!(permissions.network, ["cloud.example.org"]);
        let keys: Vec<_> = ext.settings.iter().map(|s| s.key.as_str()).collect();
        assert_eq!(keys, ["format", "python", "anders", "token"], "{:?}", ext.errors);
        let names: Vec<_> = ext.actions.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["Umwandeln", "Python", "Skript"], "{:?}", ext.errors);
        assert_eq!(ext.actions[0].keybinding.as_deref(), Some("Ctrl+Alt+J"));
        assert_eq!((ext.actions[0].in_menu, ext.actions[0].in_palette), (false, true));
        assert_eq!(ext.processors.len(), 1, "sh is not declared");
        for wanted in ["zahl", "„curl“ fehlt", "„sh“ fehlt", "ATLAS_SETTING_TOKEN", "„{setting:anders}“ fehlt"] {
            assert!(ext.errors.iter().any(|e| e.contains(wanted)), "{wanted}: {:?}", ext.errors);
        }

        // Settings: defaults, then the user's values (wrong types ignored).
        let config = dir.path().join("config");
        let ctx = Context { project: None, paths: vec![ext_dir.join("a.ipynb")], line: 0 };
        let argv = |ext: &Extension| ext.actions[0].prepare(&ctx).unwrap().argv;
        ext.apply_settings(&load_settings(&config));
        assert!(argv(&ext).ends_with(&["--to".into(), "md".into(), ext_dir.join("a.ipynb").to_string_lossy().into()]));
        assert!(argv(&ext).contains(&"ATLAS_SETTING_FORMAT=md".to_owned()));
        save_setting(&config, &ext, "format", Some(toml::Value::String("html".into()))).unwrap();
        assert!(save_setting(&config, &ext, "format", Some(toml::Value::String("pdf".into()))).is_err());
        assert!(save_setting(&config, &ext, "fehlt", None).is_err());
        ext.apply_settings(&load_settings(&config));
        assert!(argv(&ext).contains(&"html".to_owned()));
        save_setting(&config, &ext, "format", None).unwrap();
        assert!(!settings_file(&config).exists() || load_settings(&config).is_empty());

        // Processors get the values in their environment.
        let mut ext = load(&ext_dir, Location::User);
        ext.permissions.as_mut().unwrap().run.push("sh".into());
        let _ = &ext;
        let text = r#"atlas_api: 2
name: env
permissions: {run: [sh]}
settings: [{key: format, type: choice, choices: [html, md], default: md}]
processors:
- name: Zeigen
  applies_to: "*.txt"
  output: text
  command: [sh, -c, 'printf %s "$ATLAS_SETTING_FORMAT"']
"#;
        let env_dir = write_ext(dir.path(), "env", text);
        let mut ext = load(&env_dir, Location::User);
        assert!(ext.errors.is_empty(), "{:?}", ext.errors);
        save_setting(&config, &ext, "format", Some(toml::Value::String("html".into()))).unwrap();
        ext.apply_settings(&load_settings(&config));
        let file = dir.path().join("x.txt");
        std::fs::write(&file, "x").unwrap();
        let out = ext.processors[0].process(&file, None, &dir.path().join("cache")).unwrap();
        assert_eq!(out, Processed::Text("html".into()));

        // Newer API: not loaded.
        let future = write_ext(
            dir.path(),
            "neu",
            "atlas_api: 3
name: neu
",
        );
        assert!(load(&future, Location::User).errors[0].contains("neuere Atlas-Version"));
    }

    #[test]
    fn old_template_extensions_become_core() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let old = |n: &str| {
            format!(
                "atlas_extension: 1\nname: {n}\ntitle: Beispiel-Erweiterung\nactions:\n- {{name: Ls, command: ls}}\n"
            )
        };
        write_ext(root, "beispiel", &old("beispiel"));
        write_ext(root, "beispiel-2", &old("beispiel-2"));
        write_ext(root, "beispiel-3", "atlas_extension: 1\nname: beispiel-3\ntitle: Meine\n");
        let renamed = migrate_legacy(root);
        assert_eq!(renamed, [("beispiel".into(), "core".into()), ("beispiel-2".into(), "core-2".into())]);
        let core = load(&root.join("core"), Location::User);
        assert_eq!((core.name.as_str(), core.title.as_str(), core.actions.len()), ("core", "Core", 1));
        assert_eq!(load(&root.join("core-2"), Location::User).title, "Core 2");
        assert!(root.join("beispiel-3").is_dir(), "own titles stay");
        assert!(migrate_legacy(root).is_empty(), "runs once");
    }

    #[test]
    fn trust_covers_every_file_of_a_project_extension() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let ext_dir = write_ext(
            dir.path(),
            "skript",
            "atlas_extension: 1\nname: skript\nactions:\n- {name: A, command: [\"{extension}/run.sh\"]}\n",
        );
        std::fs::create_dir(ext_dir.join("lib")).unwrap();
        std::fs::write(ext_dir.join("run.sh"), "#!/bin/sh\necho harmlos\n").unwrap();
        std::fs::write(ext_dir.join("lib/helfer.py"), "print(1)\n").unwrap();
        let state = dir.path().join("state");
        let mut trust = Trust::load(&state);
        trust.trust(&load(&ext_dir, Location::Project)).unwrap();
        let active = || status(&load(&ext_dir, Location::Project), &Trust::load(&state), &[]) == Status::Active;
        assert!(active());
        // The script changes (same size even), the manifest does not.
        std::fs::write(ext_dir.join("run.sh"), "#!/bin/sh\necho bösartig\n").unwrap();
        assert!(!active(), "a changed script needs a new approval");
        trust.trust(&load(&ext_dir, Location::Project)).unwrap();
        assert!(active());
        // A file in a sub folder, a new file, the executable bit, a link.
        std::fs::write(ext_dir.join("lib/helfer.py"), "print(2)\n").unwrap();
        assert!(!active());
        trust.trust(&load(&ext_dir, Location::Project)).unwrap();
        std::fs::write(ext_dir.join("neu.sh"), "x").unwrap();
        assert!(!active());
        trust.trust(&load(&ext_dir, Location::Project)).unwrap();
        std::fs::set_permissions(ext_dir.join("neu.sh"), std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(!active());
        trust.trust(&load(&ext_dir, Location::Project)).unwrap();
        let outside = dir.path().join("draussen.sh");
        std::fs::write(&outside, "echo 1").unwrap();
        std::os::unix::fs::symlink(&outside, ext_dir.join("link.sh")).unwrap();
        assert!(!active());
        trust.trust(&load(&ext_dir, Location::Project)).unwrap();
        assert!(active());
        std::fs::write(&outside, "echo 2").unwrap();
        assert!(!active(), "the file a link points to counts too");
        // A link to a folder counts with everything in it.
        let werkzeuge = dir.path().join("werkzeuge");
        std::fs::create_dir(&werkzeuge).unwrap();
        std::fs::write(werkzeuge.join("run.sh"), "echo 1").unwrap();
        std::os::unix::fs::symlink(&werkzeuge, ext_dir.join("werkzeuge")).unwrap();
        trust.trust(&load(&ext_dir, Location::Project)).unwrap();
        assert!(active());
        std::fs::write(werkzeuge.join("run.sh"), "echo 2").unwrap();
        assert!(!active(), "a file behind a folder link counts too");
        // A link loop ends instead of hanging.
        std::os::unix::fs::symlink(&ext_dir, werkzeuge.join("zurueck")).unwrap();
        trust.trust(&load(&ext_dir, Location::Project)).unwrap();
        assert!(active());
    }

    #[test]
    fn project_extensions_need_trust_bound_to_the_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let ext_dir = write_ext(dir.path(), "x", "atlas_extension: 1\nname: x\nactions:\n- {name: A, command: ls}\n");
        let state = dir.path().join("state");
        let mut trust = Trust::load(&state);
        let ext = load(&ext_dir, Location::Project);
        assert_eq!(status(&ext, &trust, &[]), Status::Untrusted);
        trust.trust(&ext).unwrap();
        assert_eq!(status(&ext, &Trust::load(&state), &[]), Status::Active, "trust is stored");
        assert_eq!(status(&ext, &trust, &["x".into()]), Status::Disabled);
        // A changed manifest is not trusted any more.
        std::fs::write(
            ext_dir.join(MANIFEST),
            "atlas_extension: 1\nname: x\nactions:\n- {name: B, command: rm -rf /}\n",
        )
        .unwrap();
        let changed = load(&ext_dir, Location::Project);
        assert_eq!(status(&changed, &Trust::load(&state), &[]), Status::Untrusted);
        // User extensions are trusted.
        assert_eq!(status(&load(&ext_dir, Location::User), &trust, &[]), Status::Active);
    }

    #[test]
    fn processors_convert_and_cache() {
        let dir = tempfile::tempdir().unwrap();
        let ext_dir = write_ext(
            dir.path(),
            "conv",
            "atlas_extension: 1\nname: conv\nprocessors:\n\
             - {name: Groß, applies_to: \"*.log\", output: markdown, shell: true, command: \"echo '# Log'; tr a-z A-Z < {file}\"}\n\
             - {name: Fehler, applies_to: \"*.err\", command: [sh, -c, \"echo kaputt >&2; exit 2\"]}\n\
             - {name: Langsam, applies_to: \"*.slow\", timeout: 1, command: [sleep, \"5\"]}\n",
        );
        let ext = load(&ext_dir, Location::User);
        assert!(ext.errors.is_empty(), "{:?}", ext.errors);
        let file = dir.path().join("mein log.log");
        std::fs::write(&file, "hallo welt\n").unwrap();
        let cache = dir.path().join("cache");
        let p = &ext.processors[0];
        assert!(p.applies(&file, None));
        assert_eq!(p.process(&file, None, &cache).unwrap(), Processed::Markdown("# Log\nHALLO WELT\n".into()));
        assert_eq!(std::fs::read_dir(&cache).unwrap().count(), 1, "cached");
        std::fs::write(&file, "anders\n").unwrap();
        assert_eq!(p.process(&file, None, &cache).unwrap(), Processed::Markdown("# Log\nANDERS\n".into()));

        // Several views converting the same file at once all succeed.
        std::fs::write(&file, "parallel\n").unwrap();
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let (p, file, cache) = (p.clone(), file.clone(), cache.clone());
                std::thread::spawn(move || p.process(&file, None, &cache))
            })
            .collect();
        for h in handles {
            assert_eq!(h.join().unwrap().unwrap(), Processed::Markdown("# Log\nPARALLEL\n".into()));
        }

        let err = dir.path().join("a.err");
        std::fs::write(&err, "").unwrap();
        let message = ext.processors[1].process(&err, None, &cache).unwrap_err().to_string();
        assert!(message.contains("Code 2") && message.contains("kaputt"), "{message}");

        let slow = dir.path().join("a.slow");
        std::fs::write(&slow, "").unwrap();
        let started = Instant::now();
        assert!(ext.processors[2].process(&slow, None, &cache).is_err());
        assert!(started.elapsed() < Duration::from_secs(4), "killed after the timeout");
    }

    #[test]
    fn exporters_write_atomically_next_to_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let ext_dir = write_ext(
            dir.path(),
            "exp",
            "atlas_extension: 1\nname: exp\nexporters:\n\
             - {name: Kopie, applies_to: \"*.md\", target: txt, command: [cp, \"{file}\", \"{output}\"]}\n\
             - {name: Kaputt, target: txt, shell: true, command: \"echo halb > {output}; exit 1\"}\n",
        );
        let ext = load(&ext_dir, Location::User);
        assert!(ext.errors.is_empty(), "{:?}", ext.errors);
        let file = dir.path().join("notiz.md");
        std::fs::write(&file, "# Hallo").unwrap();
        let e = &ext.exporters[0];
        assert!(e.applies(&file, None));
        let out = e.default_output(&file);
        assert_eq!(out, dir.path().join("notiz.txt"));
        e.export(&file, &out, None).unwrap();
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "# Hallo");

        std::fs::write(&out, "vorher").unwrap();
        assert!(ext.exporters[1].export(&file, &out, None).is_err());
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "vorher", "a failed export changes nothing");
        let leftovers = std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains("atlas-export"))
            .count();
        assert_eq!(leftovers, 0);
        assert!(e.export(&file, &file, None).is_err(), "never over the source");
    }
}
