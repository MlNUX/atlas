// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! The index of the docs search: the documentation of installed Python
//! packages (pandas, numpy, matplotlib …), read from their source files –
//! no Python is started, nothing is executed, nothing is downloaded.
//!
//! For every package [`build`] finds the folder it is installed in
//! ([`envs`]), walks its public API following imports and stubs ([`api`])
//! and writes `<cache>/<package>.json` (the entries the view searches)
//! plus `<cache>/index.json` (what was built, from where). Docstrings in
//! the numpydoc format are split into parameters, returns and sections
//! ([`numpydoc`]).

pub mod api;
pub mod envs;
pub mod examples;
pub mod lexer;
pub mod numpydoc;
pub mod outline;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use serde_json::{Value, json};

/// Version of the files written (the view checks it).
pub const FORMAT: i64 = 1;

/// Package names in a setting ("numpy, pandas matplotlib").
pub fn package_list(text: &str) -> Vec<String> {
    let mut out: Vec<String> = text
        .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .map(str::trim)
        .filter(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.'))
        .map(str::to_owned)
        .collect();
    out.dedup();
    out
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or_default()
}

/// Builds the index of `packages` into `cache`; `setting` is the
/// environment from the settings (may be empty), `project` the open
/// project. Progress goes to `log`. Returns whether any package was found.
pub fn build(
    cache: &Path,
    packages: &[String],
    setting: &str,
    project: Option<&Path>,
    log: &mut dyn FnMut(String),
) -> Result<bool> {
    std::fs::create_dir_all(cache)?;
    let path = envs::search_path(setting, project);
    if !setting.trim().is_empty() && !path.first().is_some_and(|p| p.starts_with(envs_root_hint(setting))) {
        log(format!("Umgebung aus den Einstellungen ohne Pakete: {setting}"));
    }
    let summary_path = cache.join("index.json");
    let mut summary: Value = std::fs::read_to_string(&summary_path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .filter(|s: &Value| s["format"] == FORMAT)
        .unwrap_or_else(|| json!({ "format": FORMAT, "packages": {} }));
    let mut found_any = false;
    for name in packages {
        let start = Instant::now();
        let Some((site, dir)) = envs::find_package(name, &path) else {
            log(format!("{name}: nicht gefunden"));
            summary["packages"][name] = json!({ "error": "nicht installiert", "built": now() });
            continue;
        };
        log(format!("{name}: wird gelesen aus {} …", site.display()));
        let version = envs::version(name, &site, &dir);
        let mut pkg = api::Package::open(name, &dir);
        let entries = api::entries(&mut pkg);
        let data = json!({
            "format": FORMAT,
            "package": name,
            "version": version,
            "source": site.to_string_lossy(),
            "built": now(),
            "entries": entries,
        });
        let file = cache.join(format!("{name}.json"));
        let text = serde_json::to_string(&data)?;
        crate::paths::write_atomic(&file, text.as_bytes())?;
        log(format!(
            "{name} {version}: {} Einträge in {:.1} s ({} KB)",
            entries.len(),
            start.elapsed().as_secs_f32(),
            text.len() / 1024
        ));
        summary["packages"][name] = json!({
            "version": version,
            "count": entries.len(),
            "source": site.to_string_lossy(),
            "built": now(),
            "size": text.len(),
        });
        found_any = true;
    }
    summary["built"] = json!(now());
    summary["searched"] = json!(path.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>());
    crate::paths::write_atomic(&summary_path, serde_json::to_string_pretty(&summary)?.as_bytes())?;
    if !found_any {
        log(format!("Keins der Pakete ({}) gefunden. Gesucht in:", packages.join(", ")));
        for p in path.iter().take(40) {
            log(format!("  {}", p.display()));
        }
    }
    Ok(found_any)
}

/// The environment folder a setting names (for the message above).
fn envs_root_hint(setting: &str) -> PathBuf {
    let p = PathBuf::from(setting.trim());
    match p.parent() {
        Some(bin) if bin.file_name().is_some_and(|n| n == "bin") => bin.parent().unwrap_or(bin).to_path_buf(),
        _ => p,
    }
}

// ------------------------------------------------------------- the job

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Status {
    pub running: bool,
    /// 0: done, 1: nothing found, 2: failed; absent while running.
    pub code: Option<i32>,
    pub log: String,
}

static JOBS: Mutex<Option<HashMap<PathBuf, Status>>> = Mutex::new(None);

fn update(cache: &Path, f: impl FnOnce(&mut Status)) {
    let mut jobs = JOBS.lock().unwrap_or_else(|e| e.into_inner());
    f(jobs.get_or_insert_with(HashMap::new).entry(cache.to_path_buf()).or_default());
}

/// Starts building the index in the background; false if a build for
/// `cache` is already running.
pub fn start(cache: PathBuf, packages: Vec<String>, setting: String, project: Option<PathBuf>) -> bool {
    {
        let mut jobs = JOBS.lock().unwrap_or_else(|e| e.into_inner());
        let jobs = jobs.get_or_insert_with(HashMap::new);
        if jobs.get(&cache).is_some_and(|s| s.running) {
            return false;
        }
        jobs.insert(cache.clone(), Status { running: true, code: None, log: String::new() });
    }
    std::thread::spawn(move || {
        let mut log = |line: String| {
            log::info!("pydocs: {line}");
            update(&cache, |s| {
                s.log.push_str(&line);
                s.log.push('\n');
            });
        };
        let result = build(&cache, &packages, &setting, project.as_deref(), &mut log);
        let code = match result {
            Ok(true) => 0,
            Ok(false) => 1,
            Err(err) => {
                log(format!("Fehler: {err:#}"));
                2
            }
        };
        log("fertig".to_owned());
        update(&cache, |s| {
            s.running = false;
            s.code = Some(code);
        });
    });
    true
}

/// State of the build for `cache` (empty if none ran since Atlas started).
pub fn status(cache: &Path) -> Option<Status> {
    JOBS.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|j| j.get(cache).cloned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_names_from_the_setting() {
        assert_eq!(package_list("numpy, pandas  matplotlib;scipy"), ["numpy", "pandas", "matplotlib", "scipy"]);
        assert_eq!(package_list("pandas'; x$y, b"), ["b"]);
    }

    #[test]
    fn builds_the_index_in_the_cache_folder() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("projekt");
        let site = project.join(".venv/lib/python3.12/site-packages");
        std::fs::create_dir_all(site.join("kleinpaket")).unwrap();
        std::fs::write(project.join(".venv/pyvenv.cfg"), "home = /nirgends/bin\n").unwrap();
        std::fs::write(
            site.join("kleinpaket/__init__.py"),
            "\"\"\"Ein kleines Paket.\"\"\"\n__version__ = '0.3'\ndef summe(a, b=0):\n    \"\"\"Summe zweier Zahlen.\"\"\"\n    return a + b\n",
        )
        .unwrap();
        let cache = dir.path().join("cache");
        let mut lines = Vec::new();
        let ok =
            build(&cache, &["kleinpaket".into(), "fehlt_sicher".into()], "", Some(&project), &mut |l| lines.push(l))
                .unwrap();
        assert!(ok, "{lines:?}");
        let data: Value =
            serde_json::from_str(&std::fs::read_to_string(cache.join("kleinpaket.json")).unwrap()).unwrap();
        assert_eq!(data["version"], "0.3");
        let summe = data["entries"].as_array().unwrap().iter().find(|e| e["id"] == "kleinpaket.summe").unwrap();
        assert_eq!(summe["sig"], "(a, b=0)");
        assert_eq!(summe["sum"], "Summe zweier Zahlen.");
        let summary: Value = serde_json::from_str(&std::fs::read_to_string(cache.join("index.json")).unwrap()).unwrap();
        assert_eq!(summary["packages"]["kleinpaket"]["count"], data["entries"].as_array().unwrap().len());
        assert_eq!(summary["packages"]["fehlt_sicher"]["error"], "nicht installiert");
        assert!(lines.iter().any(|l| l.contains("fehlt_sicher: nicht gefunden")));
    }
}

/// Builds the index of real packages for a manual check:
/// `ATLAS_PYDOCS_SITE=<site-packages> ATLAS_PYDOCS_OUT=<dir> cargo test -p atlas-core pydocs_real -- --ignored --nocapture`
#[cfg(test)]
#[test]
#[ignore]
fn pydocs_real() {
    let site = std::env::var("ATLAS_PYDOCS_SITE").expect("ATLAS_PYDOCS_SITE");
    let out = PathBuf::from(std::env::var("ATLAS_PYDOCS_OUT").expect("ATLAS_PYDOCS_OUT"));
    let pkgs =
        package_list(&std::env::var("ATLAS_PYDOCS_PACKAGES").unwrap_or_else(|_| "numpy pandas matplotlib".into()));
    build(&out, &pkgs, &site, None, &mut |l| println!("{l}")).unwrap();
}
