// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! A small, declarative extension system.
//!
//! An extension is a folder with an `extension.yaml` (format version 1, see
//! docs/extensions.md) – for the user in `$XDG_CONFIG_HOME/atlas/extensions/`
//! or for one project in `.atlas/extensions/`. It can contribute
//!
//! - **actions** (same format as `actions.yaml`),
//! - **processors** that show files Atlas cannot display itself (a command
//!   turns them into Markdown, text or a PDF), and
//! - **exporters** that write a file in another format.
//!
//! Extensions cannot change the user interface. Commands follow the rules
//! of actions: program plus arguments, a shell only with `shell: true`,
//! placeholders filled in after splitting. Extensions that come with a
//! project run only after the user trusted them; a changed manifest needs
//! trust again.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};
use yaml_rust2::Yaml;

use crate::actions::{self, Action, Command, Context, LoadError, Source};

pub const FORMAT_VERSION: i64 = 1;
pub const MANIFEST: &str = "extension.yaml";
pub const DIR: &str = "extensions";
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
    globs: globset::GlobSet,
}

#[derive(Debug, Clone)]
pub struct Extension {
    pub name: String,
    pub title: String,
    pub version: String,
    pub description: String,
    pub dir: PathBuf,
    pub location: Location,
    /// BLAKE3 of the manifest (trust is bound to it).
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
atlas_extension: 1
name: beispiel
title: Beispiel-Erweiterung
version: 0.1.0
description: Zeigt, was eine Erweiterung beitragen kann.

# Aktionen wie in actions.yaml; {extension} ist dieser Ordner.
actions:
  - name: Wörter zählen
    applies_to: "*.md"
    command: [wc, -w, "{file}"]

# Dateiverarbeitung: Dateien anzeigen, für die Atlas keine Ansicht hat.
# Ausgabe: markdown, text (Standardausgabe oder {output}) oder pdf ({output}).
processors:
  - name: Hexdump
    applies_to: ["*.bin"]
    output: text
    command: [od, -A, x, -t, x1z, "{file}"]

# Exporter: schreiben eine Datei in einem anderen Format nach {output}.
exporters:
  - name: HTML (pandoc)
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
            l => l.to_owned(),
        };
        examples |= line.starts_with("# Aktionen");
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
    })
}

fn uses(command: &Command, placeholder: &str) -> bool {
    match command {
        Command::Argv(argv) => argv.iter().any(|a| a.contains(placeholder)),
        Command::Shell(s) => s.contains(placeholder),
    }
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
    ext.hash = blake3::hash(&bytes).to_hex().to_string();
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
    match doc["atlas_extension"].as_i64() {
        Some(v) if v > FORMAT_VERSION => {
            ext.errors.push(format!("Format {v} ist neuer als diese Atlas-Version ({FORMAT_VERSION})"));
            return ext;
        }
        Some(_) => {}
        None => {
            ext.errors.push("„atlas_extension: 1“ fehlt".into());
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

    let mut errors = Vec::new();
    actions::parse_list(&doc["actions"], &Source::Extension(name.clone()), &manifest, &mut ext.actions, &mut errors);
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
    ext
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

pub fn user_root(config_dir: &Path) -> PathBuf {
    config_dir.join(DIR)
}

pub fn project_root(atlas_dir: &Path) -> PathBuf {
    atlas_dir.join(DIR)
}

// -------------------------------------------------------------------- trust

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
        hasher.update(format!("{:?}", self.command).as_bytes());
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
            let extra = [
                ("{output}", tmp.to_string_lossy().into_owned()),
                ("{extension}", self.base_dir.to_string_lossy().into_owned()),
            ];
            let built = actions::build(&self.command, self.cwd.as_deref(), &ctx, &extra)?;
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
        let extra = [
            ("{output}", tmp.to_string_lossy().into_owned()),
            ("{extension}", self.base_dir.to_string_lossy().into_owned()),
        ];
        let built = actions::build(&self.command, self.cwd.as_deref(), &ctx, &extra)?;
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
        assert!(found[2].errors[0].contains("atlas_extension"));

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
