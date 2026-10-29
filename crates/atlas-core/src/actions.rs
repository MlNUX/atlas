// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! User defined actions: declarative commands that run visibly in the Atlas
//! terminal.
//!
//! Actions are described in YAML – per project in `.atlas/actions.yaml`,
//! for all projects in `$XDG_CONFIG_HOME/atlas/actions.yaml` (format in
//! docs/actions.md):
//!
//! ```yaml
//! version: 1
//! actions:
//!   - name: Markdown als PDF exportieren
//!     applies_to: "*.md"
//!     command: [pandoc, "{file}", -o, "{stem}.pdf"]
//!   - name: Tests ausführen
//!     when: project
//!     command: cargo test
//!   - name: Bilder verkleinern
//!     applies_to: ["*.png", "*.jpg"]
//!     shell: true
//!     command: "for f in {files}; do convert \"$f\" -resize 50% \"$f\"; done"
//!     confirm: true
//! ```
//!
//! A command is a program with arguments and never goes through a shell
//! unless `shell: true` says so. Placeholders are replaced after the
//! command was split into arguments, so paths with spaces or quotes stay one
//! argument; in shell commands they are inserted shell-quoted.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};
use yaml_rust2::Yaml;

pub const FORMAT_VERSION: i64 = 1;
pub const PROJECT_FILE: &str = "actions.yaml";

/// Where an action can be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Selected files (matching `applies_to`).
    File,
    /// A selected folder.
    Dir,
    /// The project as a whole (no selection needed).
    Project,
}

impl Scope {
    fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "file" | "files" | "datei" => Some(Scope::File),
            "dir" | "folder" | "ordner" | "directory" => Some(Scope::Dir),
            "project" | "projekt" => Some(Scope::Project),
            _ => None,
        }
    }
}

/// Where the command runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// New tab in the terminal panel (default).
    Panel,
    /// Terminal tab in the editor area.
    Tab,
    /// No terminal: Atlas runs it in the background (crate::background),
    /// e.g. a server; one at a time per action.
    Background,
}

/// Where an action was defined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Project,
    User,
    /// Name of the extension.
    Extension(String),
}

impl Source {
    pub fn label(&self) -> String {
        match self {
            Source::Project => "Projekt".into(),
            Source::User => "Benutzer".into(),
            Source::Extension(name) => format!("Erweiterung {name}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Program and arguments (from a list, or a string split like a shell
    /// would – without running one).
    Argv(Vec<String>),
    /// Explicit shell command line (`shell: true`), run with `sh -c`.
    Shell(String),
}

#[derive(Debug, Clone)]
pub struct Action {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source: Source,
    /// Glob patterns; a pattern without `/` matches the file name, one with
    /// `/` the project-relative path. Empty: every file.
    pub applies_to: Vec<String>,
    pub scopes: Vec<Scope>,
    pub command: Command,
    /// Working directory (with placeholders); default: project root.
    pub cwd: Option<String>,
    pub confirm: bool,
    pub placement: Placement,
    globs: Option<globset::GlobSet>,
    /// Folder of the extension that defines the action (`{extension}`).
    pub base_dir: Option<PathBuf>,
    /// Key sequence that runs the action (normalized, see [`keybinding`]).
    pub keybinding: Option<String>,
    /// Offered in the context menu (otherwise only palette and key).
    pub in_menu: bool,
    /// Offered in the command palette.
    pub in_palette: bool,
    /// More placeholders (`{setting:…}` of an extension) and environment
    /// variables for the command.
    pub extra: Vec<(String, String)>,
    pub env: Vec<(String, String)>,
}

/// A problem in an action file (shown to the user, the rest still loads).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadError {
    pub file: PathBuf,
    pub message: String,
}

/// What the action is run on.
#[derive(Debug, Clone, Default)]
pub struct Context {
    pub project: Option<PathBuf>,
    /// Selected files or folders (absolute); empty for project actions.
    pub paths: Vec<PathBuf>,
    /// Line in the active editor (0 = unknown).
    pub line: u32,
}

impl Context {
    fn scope(&self) -> Scope {
        match self.paths.first() {
            None => Scope::Project,
            Some(p) if p.is_dir() => Scope::Dir,
            Some(_) => Scope::File,
        }
    }
}

/// A command ready to run in a terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prepared {
    /// What the terminal starts (the command wrapped so that its output
    /// stays visible after it ended).
    pub argv: Vec<String>,
    /// The command as it will run, for confirmations.
    pub display: String,
    pub cwd: PathBuf,
    /// Ask first: configured, or the command looks destructive.
    pub confirm: bool,
    pub destructive: bool,
    pub placement: Placement,
}

pub fn user_file(config_dir: &Path) -> PathBuf {
    config_dir.join(PROJECT_FILE)
}

pub fn project_file(atlas_dir: &Path) -> PathBuf {
    atlas_dir.join(PROJECT_FILE)
}

/// Template for a new action file.
pub const TEMPLATE: &str = r#"# Eigene Aktionen für Atlas (Format: docs/actions.md)
#
# Platzhalter: {file} {files} {name} {basename} {stem} {ext} {dir}
#              {relative} {project} {line}
# Befehle laufen ohne Shell; `shell: true` aktiviert sie ausdrücklich.
version: 1
actions:
  - name: Markdown als Word-Datei (pandoc)
    applies_to: "*.md"
    command: [pandoc, "{file}", -o, "{stem}.docx"]

  - name: Prüfsumme anzeigen
    applies_to: "*"
    command: [sha256sum, "{files}"]

  - name: Tests ausführen
    when: project
    command: cargo test
"#;

// ------------------------------------------------------------------ parsing

fn strings(value: &Yaml) -> Option<Vec<String>> {
    match value {
        Yaml::String(s) => Some(vec![s.clone()]),
        Yaml::Array(items) => items.iter().map(scalar).collect(),
        Yaml::BadValue | Yaml::Null => None,
        other => scalar(other).map(|s| vec![s]),
    }
}

fn scalar(value: &Yaml) -> Option<String> {
    match value {
        Yaml::String(s) | Yaml::Real(s) => Some(s.clone()),
        Yaml::Integer(i) => Some(i.to_string()),
        Yaml::Boolean(b) => Some(b.to_string()),
        _ => None,
    }
}

fn slug(name: &str) -> String {
    let s: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if s.is_empty() { "aktion".into() } else { s }
}

fn build_globs(patterns: &[String]) -> Result<Option<globset::GlobSet>> {
    if patterns.is_empty() {
        return Ok(None);
    }
    let mut builder = globset::GlobSetBuilder::new();
    for p in patterns {
        let glob = globset::GlobBuilder::new(p)
            .literal_separator(true)
            .case_insensitive(true)
            .build()
            .with_context(|| format!("ungültiges Muster „{p}“"))?;
        builder.add(glob);
    }
    Ok(Some(builder.build()?))
}

/// Parses one action entry.
pub fn parse_action(entry: &Yaml, source: &Source) -> Result<Action> {
    let name = entry["name"].as_str().map(str::trim).filter(|n| !n.is_empty()).context("„name“ fehlt")?;
    let shell = entry["shell"].as_bool().unwrap_or(false);
    let command = match &entry["command"] {
        Yaml::String(s) if shell => Command::Shell(s.clone()),
        Yaml::String(s) => Command::Argv(crate::config::split_words(s).context("„command“ ist nicht lesbar")?),
        Yaml::Array(_) if shell => bail!("mit „shell: true“ muss „command“ ein Text sein"),
        list @ Yaml::Array(_) => Command::Argv(strings(list).context("„command“ darf nur Texte enthalten")?),
        _ => bail!("„command“ fehlt"),
    };
    if let Command::Argv(argv) = &command
        && argv.is_empty()
    {
        bail!("„command“ ist leer");
    }
    let applies_to = strings(&entry["applies_to"]).unwrap_or_default();
    let globs = build_globs(&applies_to)?;
    let scopes = match strings(&entry["when"]) {
        Some(list) => list
            .iter()
            .map(|s| Scope::parse(s).with_context(|| format!("unbekanntes „when: {s}“ (file, dir, project)")))
            .collect::<Result<Vec<_>>>()?,
        None if applies_to.is_empty() && !uses_file(&command) => vec![Scope::Project],
        None => vec![Scope::File],
    };
    let placement = match entry["terminal"].as_str().map(str::to_ascii_lowercase).as_deref() {
        None | Some("panel") => Placement::Panel,
        Some("tab") => Placement::Tab,
        Some("background" | "hintergrund" | "none") => Placement::Background,
        Some(other) => bail!("unbekanntes „terminal: {other}“ (panel, tab, background)"),
    };
    let confirm = entry["confirm"].as_bool().or_else(|| entry["confirmation"].as_bool()).unwrap_or(false);
    let keybinding = match scalar(&entry["keybinding"]).or_else(|| scalar(&entry["key"])) {
        Some(k) => Some(keybinding(&k)?),
        None => None,
    };
    let (in_menu, in_palette) = match strings(&entry["menu"]) {
        None => (true, true),
        Some(list) => {
            let mut places = (false, false);
            for place in &list {
                match place.trim().to_ascii_lowercase().as_str() {
                    "context" | "kontextmenü" => places.0 = true,
                    "palette" | "arbeitsleiste" => places.1 = true,
                    "none" | "keine" => {}
                    other => bail!("unbekanntes „menu: {other}“ (context, palette, none)"),
                }
            }
            places
        }
    };
    let id = entry["id"].as_str().map(str::to_owned).unwrap_or_else(|| slug(name));
    let id = match source {
        Source::Extension(ext) => format!("{ext}/{id}"),
        _ => id,
    };
    Ok(Action {
        id,
        name: name.to_owned(),
        description: entry["description"].as_str().unwrap_or_default().to_owned(),
        source: source.clone(),
        applies_to,
        scopes,
        command,
        cwd: entry["cwd"].as_str().map(str::to_owned),
        confirm,
        placement,
        globs,
        base_dir: None,
        keybinding,
        in_menu,
        in_palette,
        extra: Vec::new(),
        env: Vec::new(),
    })
}

/// Shortcuts of Atlas itself (docs/shortcuts.md) and of text fields; an
/// action cannot take them over.
const RESERVED_KEYS: &[&str] = &[
    "Ctrl+P",
    "Ctrl+Shift+P",
    "Ctrl+O",
    "Ctrl+Shift+F",
    "Ctrl+Shift+E",
    "Ctrl+B",
    "Ctrl+`",
    "Ctrl+Shift+`",
    "Ctrl+\\",
    "Ctrl+W",
    "Ctrl+Tab",
    "Ctrl+Shift+Tab",
    "Alt+Left",
    "Alt+Right",
    "Alt+Up",
    "Alt+Down",
    "Alt+Return",
    "Alt+Enter",
    "F2",
    "F3",
    "Shift+F3",
    "F5",
    "Shift+F5",
    "F11",
    "Ctrl+A",
    "Ctrl+C",
    "Ctrl+X",
    "Ctrl+V",
    "Ctrl+Z",
    "Ctrl+Y",
    "Ctrl+Shift+Z",
    "Ctrl+S",
    "Ctrl+F",
    "Ctrl+G",
    "Ctrl+L",
    "Ctrl+H",
    "Ctrl+Q",
    "Ctrl+D",
    "Ctrl+0",
    "Ctrl++",
    "Ctrl+-",
    "Ctrl+=",
    "Ctrl+Return",
    "Ctrl+Enter",
    "Ctrl+Shift+C",
    "Ctrl+Shift+V",
    "Ctrl+Home",
    "Ctrl+End",
    "Ctrl+Left",
    "Ctrl+Right",
    "Ctrl+Backspace",
    "Ctrl+Delete",
    "Shift+Insert",
];

/// Checks and normalizes a key sequence: modifiers (Ctrl/Strg, Alt,
/// Shift/Umschalt, Meta) in a fixed order, then one key. It needs Ctrl,
/// Alt or Meta (or a function key), so typing is never taken over, and
/// may not be one of Atlas's own shortcuts.
pub fn keybinding(text: &str) -> Result<String> {
    let parts: Vec<&str> = text.split('+').map(str::trim).collect();
    // „Ctrl++“ splits into an empty last part: the key is „+“.
    let (mods, key) = match parts.as_slice() {
        [mods @ .., "", ""] => (mods, "+"),
        [mods @ .., key] => (mods, *key),
        [] => bail!("leeres Tastenkürzel"),
    };
    let (mut ctrl, mut alt, mut shift, mut meta) = (false, false, false, false);
    for m in mods {
        match m.to_ascii_lowercase().as_str() {
            "ctrl" | "strg" | "control" => ctrl = true,
            "alt" => alt = true,
            "shift" | "umschalt" => shift = true,
            "meta" | "super" => meta = true,
            _ => bail!("unbekannte Taste „{m}“ in „{text}“"),
        }
    }
    let key = match key.to_ascii_lowercase().as_str() {
        "" => bail!("„{text}“: Taste fehlt"),
        "enter" => "Return".to_owned(),
        "esc" => "Escape".to_owned(),
        "entf" | "del" => "Delete".to_owned(),
        k if k.chars().count() == 1 => k.to_uppercase(),
        k if k.len() <= 3 && k.starts_with('f') && k[1..].parse::<u8>().is_ok_and(|n| (1..=24).contains(&n)) => {
            k.to_uppercase()
        }
        k if [
            "left",
            "right",
            "up",
            "down",
            "home",
            "end",
            "pageup",
            "pagedown",
            "insert",
            "delete",
            "return",
            "escape",
            "space",
            "tab",
            "backspace",
        ]
        .contains(&k) =>
        {
            let mut c = k.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        }
        _ => bail!("unbekannte Taste „{key}“ in „{text}“"),
    };
    let function_key = key.len() >= 2 && key.starts_with('F') && key[1..].parse::<u8>().is_ok();
    if !(ctrl || alt || meta || function_key) {
        bail!("„{text}“ braucht Strg, Alt oder Meta (oder eine F-Taste)");
    }
    let mut out = String::new();
    for (on, name) in [(ctrl, "Ctrl+"), (alt, "Alt+"), (shift, "Shift+"), (meta, "Meta+")] {
        if on {
            out.push_str(name);
        }
    }
    out.push_str(&key);
    if RESERVED_KEYS.iter().any(|r| r.eq_ignore_ascii_case(&out)) {
        bail!("„{out}“ ist ein Tastenkürzel von Atlas");
    }
    Ok(out)
}

fn uses_file(command: &Command) -> bool {
    let has = |s: &str| {
        ["{file}", "{files}", "{name}", "{stem}", "{basename}", "{ext}", "{relative}"].iter().any(|p| s.contains(p))
    };
    match command {
        Command::Argv(argv) => argv.iter().any(|a| has(a)),
        Command::Shell(s) => has(s),
    }
}

/// Parses an action list (`actions:` of an action file or an extension).
pub fn parse_list(list: &Yaml, source: &Source, file: &Path, out: &mut Vec<Action>, errors: &mut Vec<LoadError>) {
    let Some(items) = list.as_vec() else {
        if !list.is_badvalue() && !list.is_null() {
            errors.push(LoadError { file: file.to_path_buf(), message: "„actions“ muss eine Liste sein".into() });
        }
        return;
    };
    for (i, entry) in items.iter().enumerate() {
        match parse_action(entry, source) {
            Ok(action) => out.push(action),
            Err(err) => {
                errors.push(LoadError { file: file.to_path_buf(), message: format!("Aktion {}: {err:#}", i + 1) })
            }
        }
    }
}

/// Reads one action file (missing files are fine).
pub fn load_file(file: &Path, source: &Source, out: &mut Vec<Action>, errors: &mut Vec<LoadError>) {
    let text = match std::fs::read_to_string(file) {
        Ok(t) => t,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return,
        Err(err) => {
            errors.push(LoadError { file: file.to_path_buf(), message: err.to_string() });
            return;
        }
    };
    let docs = match yaml_rust2::YamlLoader::load_from_str(&text) {
        Ok(d) => d,
        Err(err) => {
            errors.push(LoadError { file: file.to_path_buf(), message: format!("kein gültiges YAML: {err}") });
            return;
        }
    };
    let Some(doc) = docs.first() else { return };
    let version = doc["version"].as_i64().unwrap_or(FORMAT_VERSION);
    if version > FORMAT_VERSION {
        errors.push(LoadError {
            file: file.to_path_buf(),
            message: format!("Format {version} ist neuer als diese Atlas-Version ({FORMAT_VERSION})"),
        });
        return;
    }
    parse_list(&doc["actions"], source, file, out, errors);
}

/// All actions: project actions first, then the user's (an ID defined in
/// the project replaces the user's action with the same ID).
pub fn load(atlas_dir: Option<&Path>, config_dir: &Path) -> (Vec<Action>, Vec<LoadError>) {
    let (mut actions, mut errors) = (Vec::new(), Vec::new());
    if let Some(dir) = atlas_dir {
        load_file(&project_file(dir), &Source::Project, &mut actions, &mut errors);
    }
    let mut user = Vec::new();
    load_file(&user_file(config_dir), &Source::User, &mut user, &mut errors);
    for action in user {
        if !actions.iter().any(|a| a.id == action.id) {
            actions.push(action);
        }
    }
    (actions, errors)
}

// ---------------------------------------------------------------- matching

impl Action {
    /// Whether the action can run on `ctx` (scope and file patterns).
    pub fn applies(&self, ctx: &Context) -> bool {
        let scope = ctx.scope();
        if !self.scopes.contains(&scope) {
            return false;
        }
        if scope != Scope::File {
            return true;
        }
        let Some(globs) = &self.globs else { return true };
        ctx.paths.iter().all(|p| {
            let name = p.file_name().map(Path::new).unwrap_or(p);
            let relative = ctx.project.as_ref().and_then(|r| p.strip_prefix(r).ok()).unwrap_or(p);
            globs.is_match(name) || globs.is_match(relative)
        })
    }
}

pub fn applicable<'a>(actions: &'a [Action], ctx: &Context) -> Vec<&'a Action> {
    actions.iter().filter(|a| a.applies(ctx)).collect()
}

// --------------------------------------------------------------- preparing

/// Quotes for POSIX `sh`.
pub fn shell_quote(s: &str) -> String {
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c)) {
        s.to_owned()
    } else {
        format!("'{}'", s.replace('\'', r"'\''"))
    }
}

struct Values {
    single: Vec<(String, String)>,
    files: Vec<String>,
}

fn values(ctx: &Context, extra: &[(String, String)]) -> Values {
    let project = ctx.project.clone().unwrap_or_default();
    let first = ctx.paths.first().cloned();
    let s = |p: &Path| p.to_string_lossy().into_owned();
    let mut single = vec![("{project}".to_owned(), s(&project)), ("{line}".to_owned(), ctx.line.max(1).to_string())];
    if let Some(file) = &first {
        let dir = if file.is_dir() { file.clone() } else { file.parent().map(Path::to_path_buf).unwrap_or_default() };
        let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let basename = file.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let ext = file.extension().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let stem = if file.is_dir() { s(file) } else { s(&file.with_extension("")) };
        let relative = file.strip_prefix(&project).map(s).unwrap_or_else(|_| s(file));
        single.extend(
            [
                ("{file}", s(file)),
                ("{dir}", s(&dir)),
                ("{name}", name),
                ("{basename}", basename),
                ("{ext}", ext),
                ("{stem}", stem),
                ("{relative}", relative),
            ]
            .map(|(k, v)| (k.to_owned(), v)),
        );
    } else {
        single.push(("{dir}".to_owned(), s(&project)));
    }
    single.extend(extra.iter().cloned());
    Values { single, files: ctx.paths.iter().map(|p| s(p)).collect() }
}

/// Placeholders that exist (some only for extensions).
const KNOWN: &[&str] = &[
    "{file}",
    "{files}",
    "{name}",
    "{basename}",
    "{stem}",
    "{ext}",
    "{dir}",
    "{relative}",
    "{project}",
    "{line}",
    "{output}",
    "{extension}",
];

fn substitute(text: &str, values: &Values, quote: bool) -> Result<String> {
    // Unknown placeholders are mistakes (checked before values with braces
    // in file names are inserted).
    let mut rest = text;
    while let Some(start) = rest.find('{') {
        let Some(len) = rest[start..].find('}') else { break };
        let name = &rest[start..start + len + 1];
        if name.len() > 2 && name[1..name.len() - 1].chars().all(|c| c.is_ascii_lowercase()) {
            if !KNOWN.contains(&name) {
                bail!("unbekannter Platzhalter {name}");
            }
            if name != "{files}" && !values.single.iter().any(|(k, _)| k == name) {
                bail!("Platzhalter {name} ist hier nicht verfügbar");
            }
        } else if name.starts_with("{setting:") && !values.single.iter().any(|(k, _)| k == name) {
            bail!("unbekannte Einstellung {name}");
        }
        rest = &rest[start + 1..];
    }
    // One pass over the template, so inserted values are never expanded again.
    let files = if quote {
        values.files.iter().map(|f| shell_quote(f)).collect::<Vec<_>>().join(" ")
    } else {
        values.files.join(" ")
    };
    let mut result = String::with_capacity(text.len());
    let mut rest = text;
    'outer: while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("{files}") {
            result.push_str(&files);
            rest = after;
            continue;
        }
        for (key, value) in &values.single {
            if let Some(after) = rest.strip_prefix(key) {
                result.push_str(&if quote { shell_quote(value) } else { value.clone() });
                rest = after;
                continue 'outer;
            }
        }
        let c = rest.chars().next().unwrap_or_default();
        result.push(c);
        rest = &rest[c.len_utf8()..];
    }
    Ok(result)
}

/// Programs (and subcommands) that may delete or overwrite data.
const DESTRUCTIVE: &[&str] = &[
    "rm",
    "rmdir",
    "shred",
    "dd",
    "mkfs",
    "truncate",
    "mv",
    "unlink",
    "wipefs",
    "fdisk",
    "parted",
    "chmod",
    "chown",
    "find -delete",
    "git clean",
    "git reset",
    "git push --force",
    "git checkout --",
    "rsync --delete",
];

/// Heuristic: does the command look like it deletes or overwrites files?
pub fn looks_destructive(command: &Command) -> bool {
    let words: Vec<String> = match command {
        Command::Argv(argv) => argv.clone(),
        Command::Shell(s) => s
            .split(|c: char| c.is_whitespace() || matches!(c, ';' | '|' | '&' | '(' | ')' | '`'))
            .filter(|w| !w.is_empty())
            .map(str::to_owned)
            .collect(),
    };
    let base = |w: &str| Path::new(w).file_name().and_then(|n| n.to_str()).unwrap_or(w).to_owned();
    let names: Vec<String> = words.iter().map(|w| base(w)).collect();
    if matches!(command, Command::Shell(s) if s.contains('>') && !s.contains(">&") && !s.contains("2>")) {
        return true;
    }
    DESTRUCTIVE.iter().any(|pattern| {
        let parts: Vec<&str> = pattern.split(' ').collect();
        names.iter().enumerate().any(|(i, w)| {
            (w == parts[0] || (parts[0] == "mkfs" && w.starts_with("mkfs")))
                && parts[1..].iter().all(|p| names[i + 1..].iter().any(|n| n == p))
        })
    })
}

/// Shell wrapper: runs the command, then keeps its output visible with the
/// exit status until Enter is pressed. The command's own arguments are
/// passed through unchanged (`"$@"`).
const HOLD: &str = r#"name=$1; shift; "$@"; code=$?; if [ "$code" -eq 0 ]; then printf '\033]0;✓ %s\007\n\033[32m✓ %s beendet\033[0m' "$name" "$name"; else printf '\033]0;✗ %s\007\n\033[31m✗ %s beendet mit Code %s\033[0m' "$name" "$name" "$code"; fi; printf ' – Enter schließt\n'; read -r _"#;

/// How a script file is run (built-in action „Ausführen“): shell scripts
/// with bash (no execute bit needed), other executable files with a
/// `#!` line directly. `None`: not a runnable script.
pub fn script_command(path: &Path) -> Option<Vec<String>> {
    use std::io::Read;
    use std::os::unix::fs::PermissionsExt;
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    let file = path.to_string_lossy().into_owned();
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if matches!(ext.as_str(), "sh" | "bash") {
        return Some(vec!["bash".into(), file]);
    }
    let mut head = [0u8; 2];
    let has_shebang = std::fs::File::open(path).and_then(|mut f| f.read_exact(&mut head)).is_ok() && &head == b"#!";
    (has_shebang && meta.permissions().mode() & 0o111 != 0).then(|| vec![file])
}

/// The script at `path` run visibly in the terminal (in its folder); its
/// output and exit status stay until Enter is pressed.
/// Also returns the script's file name.
pub fn prepare_script(path: &Path) -> Result<(Prepared, String)> {
    let Some(command) = script_command(path) else { bail!("{} ist kein ausführbares Skript", path.display()) };
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let cwd = path.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("/"));
    let display = command.iter().map(|a| shell_quote(a)).collect::<Vec<_>>().join(" ");
    let mut argv = vec!["sh".into(), "-c".into(), HOLD.into(), "atlas-run".into(), name.clone()];
    argv.extend(command);
    Ok((Prepared { argv, display, cwd, confirm: false, destructive: false, placement: Placement::Panel }, name))
}

/// A command with its placeholders filled in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Built {
    /// Program and arguments (`sh -c …` for shell commands).
    pub argv: Vec<String>,
    /// For display and confirmations.
    pub display: String,
    pub cwd: PathBuf,
    /// Added to the environment.
    pub env: Vec<(String, String)>,
}

/// Fills in `command` for `ctx`; `extra` adds placeholders such as
/// `{output}` or `{extension}`. `cwd` defaults to the project.
pub fn build(command: &Command, cwd: Option<&str>, ctx: &Context, extra: &[(String, String)]) -> Result<Built> {
    let values = values(ctx, extra);
    let (argv, display): (Vec<String>, String) = match command {
        Command::Argv(argv) => {
            let mut args = Vec::new();
            for word in argv {
                if word == "{files}" {
                    args.extend(values.files.iter().cloned());
                } else {
                    args.push(substitute(word, &values, false)?);
                }
            }
            if args.is_empty() || args[0].is_empty() {
                bail!("kein Programm angegeben");
            }
            let display = args.iter().map(|a| shell_quote(a)).collect::<Vec<_>>().join(" ");
            (args, display)
        }
        Command::Shell(line) => {
            let line = substitute(line, &values, true)?;
            (vec!["sh".into(), "-c".into(), line.clone()], line)
        }
    };
    let cwd = match cwd {
        Some(dir) => PathBuf::from(substitute(dir, &values, false)?),
        None => ctx
            .project
            .clone()
            .or_else(|| ctx.paths.first().and_then(|p| p.parent().map(Path::to_path_buf)))
            .unwrap_or_else(|| PathBuf::from("/")),
    };
    if !cwd.is_dir() {
        bail!("Arbeitsordner {} existiert nicht", cwd.display());
    }
    Ok(Built { argv, display, cwd, env: Vec::new() })
}

impl Action {
    /// Builds the command for `ctx`.
    pub fn prepare(&self, ctx: &Context) -> Result<Prepared> {
        let mut extra: Vec<_> =
            self.base_dir.iter().map(|d| ("{extension}".to_owned(), d.to_string_lossy().into_owned())).collect();
        extra.extend(self.extra.iter().cloned());
        let built = build(&self.command, self.cwd.as_deref(), ctx, &extra)?;
        let destructive = looks_destructive(&self.command);
        // In a terminal the tab stays open after the command (HOLD); in the
        // background the command runs as is.
        let mut argv: Vec<String> = match self.placement {
            Placement::Background => Vec::new(),
            _ => vec!["sh".into(), "-c".into(), HOLD.into(), "atlas-action".into(), self.name.clone()],
        };
        // The terminal starts the command as is: its variables via env(1).
        if !self.env.is_empty() {
            argv.extend(["env".into(), "--".into()]);
            argv.extend(self.env.iter().map(|(k, v)| format!("{k}={v}")));
        }
        argv.extend(built.argv);
        Ok(Prepared {
            argv,
            display: built.display,
            cwd: built.cwd,
            confirm: self.confirm || destructive,
            destructive,
            placement: self.placement,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripts_run_with_bash_or_their_shebang() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let sh = tmp.path().join("bau.sh");
        std::fs::write(&sh, "echo hallo\n").unwrap();
        assert_eq!(script_command(&sh), Some(vec!["bash".into(), sh.to_string_lossy().into_owned()]));
        let py = tmp.path().join("werkzeug.py");
        std::fs::write(&py, "#!/usr/bin/env python3\nprint(1)\n").unwrap();
        // Not executable: not run (Python only with its execute bit).
        assert_eq!(script_command(&py), None);
        std::fs::set_permissions(&py, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(script_command(&py), Some(vec![py.to_string_lossy().into_owned()]));
        let txt = tmp.path().join("notiz.md");
        std::fs::write(&txt, "# x").unwrap();
        std::fs::set_permissions(&txt, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(script_command(&txt), None, "no #! line");
        assert_eq!(script_command(tmp.path()), None);

        let (p, name) = prepare_script(&sh).unwrap();
        assert_eq!(name, "bau.sh");
        assert_eq!(p.cwd, tmp.path());
        assert_eq!(&p.argv[..2], &["sh".to_string(), "-c".to_string()]);
        assert_eq!(p.argv[p.argv.len() - 2..], ["bash".to_string(), sh.to_string_lossy().into_owned()]);
        assert!(prepare_script(&txt).is_err());
    }

    #[test]
    fn keybindings_need_a_modifier_and_a_free_sequence() {
        assert_eq!(keybinding("strg+alt+j").unwrap(), "Ctrl+Alt+J");
        assert_eq!(keybinding("Umschalt+Strg+F7").unwrap(), "Ctrl+Shift+F7");
        assert_eq!(keybinding("F9").unwrap(), "F9");
        assert_eq!(
            keybinding("Alt+Enter").map_err(|e| e.to_string()).unwrap_err(),
            "„Alt+Return“ ist ein Tastenkürzel von Atlas"
        );
        assert!(keybinding("Ctrl+P").is_err(), "palette");
        assert!(keybinding("Shift+A").is_err(), "typing");
        assert!(keybinding("j").is_err());
        assert!(keybinding("Ctrl+Hyper+J").is_err());
        assert_eq!(keybinding("Ctrl+Alt++").unwrap(), "Ctrl+Alt++");
    }

    fn parse(yaml: &str) -> (Vec<Action>, Vec<LoadError>) {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("actions.yaml");
        std::fs::write(&file, yaml).unwrap();
        let (mut out, mut errors) = (Vec::new(), Vec::new());
        load_file(&file, &Source::Project, &mut out, &mut errors);
        (out, errors)
    }

    #[test]
    fn parses_actions_and_reports_errors() {
        let (actions, errors) = parse(
            "version: 1\nactions:\n\
             - name: PDF\n  applies_to: \"*.md\"\n  command: pandoc {file} -o \"{stem}.pdf\"\n\
             - name: Tests ausführen\n  command: [cargo, test]\n\
             - name: Shell\n  shell: true\n  command: echo {files} > liste.txt\n  confirm: true\n  terminal: tab\n\
             - name: Kaputt\n\
             - command: ls\n\
             - name: Falsch\n  when: irgendwo\n  command: ls\n",
        );
        assert_eq!(actions.len(), 3, "{errors:?}");
        assert_eq!(errors.len(), 3, "{errors:?}");
        assert!(errors[0].message.contains("Aktion 4") && errors[0].message.contains("command"));
        assert_eq!(actions[0].id, "pdf");
        assert_eq!(actions[0].scopes, [Scope::File]);
        assert_eq!(
            actions[0].command,
            Command::Argv(vec!["pandoc".into(), "{file}".into(), "-o".into(), "{stem}.pdf".into()])
        );
        assert_eq!(actions[1].scopes, [Scope::Project], "no file placeholder: a project action");
        assert_eq!(actions[1].id, "tests-ausführen");
        assert!(matches!(actions[2].command, Command::Shell(_)));
        assert_eq!(actions[2].placement, Placement::Tab);

        let (_, errors) = parse("version: 99\nactions: []\n");
        assert!(errors[0].message.contains("neuer"));
        let (_, errors) = parse("actions: [unclosed\n");
        assert!(errors[0].message.contains("YAML"));
    }

    #[test]
    fn matches_files_and_folders() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("docs/sub")).unwrap();
        for f in ["docs/a.md", "docs/sub/b.MD", "docs/c.pdf"] {
            std::fs::write(root.join(f), "").unwrap();
        }
        let (actions, errors) = parse(
            "actions:\n\
             - {name: Md, applies_to: \"*.md\", command: [echo, \"{file}\"]}\n\
             - {name: Docs-PDF, applies_to: \"docs/*.pdf\", command: [echo, \"{file}\"]}\n\
             - {name: Ordner, when: dir, command: [ls, \"{dir}\"]}\n\
             - {name: Überall, when: [file, dir, project], command: [ls]}\n",
        );
        assert!(errors.is_empty(), "{errors:?}");
        let ctx = |paths: &[&str]| Context {
            project: Some(root.to_path_buf()),
            paths: paths.iter().map(|p| root.join(p)).collect(),
            line: 0,
        };
        let names = |c: &Context| applicable(&actions, c).iter().map(|a| a.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(&ctx(&["docs/a.md"])), ["Md", "Überall"]);
        assert_eq!(names(&ctx(&["docs/sub/b.MD", "docs/a.md"])), ["Md", "Überall"], "case-insensitive, all files");
        assert_eq!(names(&ctx(&["docs/a.md", "docs/c.pdf"])), ["Überall"], "every selected file must match");
        assert_eq!(names(&ctx(&["docs/c.pdf"])), ["Docs-PDF", "Überall"]);
        assert_eq!(names(&ctx(&["docs"])), ["Ordner", "Überall"]);
        assert_eq!(names(&ctx(&[])), ["Überall"]);
    }

    #[test]
    fn prepares_commands_without_a_shell() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("my docs")).unwrap();
        let file = root.join("my docs/Bericht $(rm -rf x).md");
        std::fs::write(&file, "").unwrap();
        let ctx = Context { project: Some(root.to_path_buf()), paths: vec![file.clone()], line: 7 };

        let (actions, _) = parse(
            "actions:\n\
             - {name: PDF, applies_to: \"*.md\", command: \"pandoc {file} -o {stem}.pdf --metadata line={line}\"}\n\
             - {name: Liste, shell: true, applies_to: \"*\", command: \"ls -l {files} | wc -l\", cwd: \"{dir}\"}\n\
             - {name: Weg, applies_to: \"*\", command: [rm, \"{files}\"]}\n\
             - {name: Unbekannt, applies_to: \"*\", command: [echo, \"{datei}\"]}\n",
        );
        let p = actions[0].prepare(&ctx).unwrap();
        let inner = &p.argv[5..];
        assert_eq!(inner[0], "pandoc");
        assert_eq!(inner[1], file.to_string_lossy(), "one argument, nothing interpreted");
        assert_eq!(inner[3], root.join("my docs/Bericht $(rm -rf x).pdf").to_string_lossy());
        assert_eq!(inner[5], "line=7");
        assert_eq!(p.cwd, root, "default: project root");
        assert!(!p.confirm && !p.destructive);
        assert_eq!(&p.argv[..2], ["sh", "-c"], "wrapped to keep the output visible");
        assert!(p.display.contains("'") && p.display.starts_with("pandoc "), "{}", p.display);

        let p = actions[1].prepare(&ctx).unwrap();
        let line = p.argv.last().unwrap();
        assert!(line.contains("'") && line.ends_with("| wc -l"), "quoted for sh: {line}");
        assert!(!line.contains("rm -rf x).md "), "the file name stays one quoted word");
        assert_eq!(p.cwd, root.join("my docs"));

        let p = actions[2].prepare(&ctx).unwrap();
        assert!(p.confirm && p.destructive, "rm needs a confirmation");
        assert!(actions[3].prepare(&ctx).unwrap_err().to_string().contains("{datei}"));
    }

    #[test]
    fn destructive_heuristic() {
        let argv = |s: &str| Command::Argv(s.split(' ').map(str::to_owned).collect());
        assert!(looks_destructive(&argv("/usr/bin/rm -r x")));
        assert!(looks_destructive(&argv("git clean -fdx")));
        assert!(looks_destructive(&argv("git push --force origin")));
        assert!(!looks_destructive(&argv("git push origin")));
        assert!(!looks_destructive(&argv("cargo test")));
        assert!(looks_destructive(&Command::Shell("echo x > datei".into())));
        assert!(!looks_destructive(&Command::Shell("make 2>&1 | tee log".into())));
        assert!(looks_destructive(&Command::Shell("find . -name '*.o' -delete".into())));
    }

    #[test]
    fn wrapper_runs_the_command_and_reports_the_status() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = Context { project: Some(dir.path().to_path_buf()), paths: vec![], line: 0 };
        let (actions, _) =
            parse("actions:\n- {name: Echo, when: project, command: [sh, -c, \"echo hallo; exit 3\"]}\n");
        let p = actions[0].prepare(&ctx).unwrap();
        let out = std::process::Command::new(&p.argv[0])
            .args(&p.argv[1..])
            .current_dir(&p.cwd)
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(text.contains("hallo") && text.contains("Code 3"), "{text}");
    }

    #[test]
    fn project_actions_replace_user_actions() {
        let dir = tempfile::tempdir().unwrap();
        let atlas = dir.path().join(".atlas");
        let config = dir.path().join("config");
        std::fs::create_dir_all(&atlas).unwrap();
        std::fs::create_dir_all(&config).unwrap();
        std::fs::write(atlas.join(PROJECT_FILE), "actions:\n- {id: test, name: Projekttest, command: make test}\n")
            .unwrap();
        std::fs::write(
            config.join(PROJECT_FILE),
            "actions:\n- {id: test, name: Benutzertest, command: cargo test}\n- {name: Andere, command: ls}\n",
        )
        .unwrap();
        let (actions, errors) = load(Some(&atlas), &config);
        assert!(errors.is_empty());
        assert_eq!(actions.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["Projekttest", "Andere"]);
        assert_eq!(actions[0].source, Source::Project);
        // The template is valid.
        std::fs::write(config.join(PROJECT_FILE), TEMPLATE).unwrap();
        let (actions, errors) = load(None, &config);
        assert!(errors.is_empty() && actions.len() == 3, "{errors:?}");
    }
}
