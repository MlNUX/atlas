// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Where installed Python packages are, found by looking at folders – no
//! Python is started. In this order: the folder from the setting (an
//! environment, its Python, or a site-packages folder), environments in
//! the project (also in subfolders), the environments of registered
//! Jupyter kernels, those built by the Jupyter view, Conda and pyenv
//! environments, the user's site-packages, and the system Pythons (with
//! their standard library). Each package comes from the first folder that
//! has it.

use std::path::{Path, PathBuf};

/// Folders that hold packages, in the order they are searched (no
/// duplicates).
pub fn search_path(setting: &str, project: Option<&Path>) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    let mut sites: Vec<PathBuf> = Vec::new();
    let home = dirs::home_dir().unwrap_or_default();
    let setting = setting.trim();
    if !setting.is_empty() {
        // An environment, its Python, or a folder with packages in it.
        let p = expand_home(setting, &home);
        roots.push(env_root(&p));
        if p.is_dir() {
            sites.push(p);
        }
    }
    if let Some(project) = project {
        for name in [".venv", "venv", "env", ".env"] {
            roots.push(project.join(name));
        }
        find_venvs(project, 0, &mut roots);
    }
    for spec in kernel_specs(&home) {
        roots.extend(spec.0);
        sites.extend(spec.1);
    }
    roots.extend(children(&home.join(".local/share/atlas/envs")));
    for var in ["CONDA_PREFIX", "VIRTUAL_ENV"] {
        if let Some(v) = std::env::var_os(var) {
            roots.push(PathBuf::from(v));
        }
    }
    if let Ok(text) = std::fs::read_to_string(home.join(".conda/environments.txt")) {
        roots.extend(text.lines().map(str::trim).filter(|l| !l.is_empty()).map(PathBuf::from));
    }
    for base in [
        "miniforge3",
        "mambaforge",
        "miniconda3",
        "anaconda3",
        ".miniconda3",
        "micromamba",
        ".local/share/atlas/micromamba",
    ] {
        let base = home.join(base);
        roots.push(base.clone());
        roots.extend(children(&base.join("envs")));
    }
    for base in ["/opt/miniforge3", "/opt/miniconda3", "/opt/anaconda3", "/opt/conda"] {
        roots.push(PathBuf::from(base));
        roots.extend(children(&Path::new(base).join("envs")));
    }
    roots.extend(children(&home.join(".pyenv/versions")));
    let mut out: Vec<PathBuf> = Vec::new();
    let push = |p: PathBuf, out: &mut Vec<PathBuf>| {
        if p.is_dir() && !out.contains(&p) {
            out.push(p);
        }
    };
    for p in sites {
        push(p, &mut out);
    }
    for root in roots {
        for site in site_packages(&root) {
            push(site, &mut out);
        }
    }
    // The user's own packages (pip install --user).
    for lib in python_dirs(&home.join(".local/lib")) {
        push(lib.join("site-packages"), &mut out);
    }
    // System Pythons: those in PATH, then the usual places; with their
    // standard library (json, pathlib, re …).
    let mut system: Vec<PathBuf> = Vec::new();
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            for exe in ["python3", "python"] {
                let p = dir.join(exe);
                if p.is_file() {
                    system.push(env_root(&p.canonicalize().unwrap_or(p)));
                }
            }
        }
    }
    for base in ["/usr/local", "/usr", "/opt/homebrew", "/run/current-system/sw"] {
        system.push(PathBuf::from(base));
    }
    system.push(home.join(".nix-profile"));
    for root in system {
        for site in site_packages(&root) {
            push(site, &mut out);
        }
        for lib in python_dirs(&root.join("lib")) {
            push(lib.join("dist-packages"), &mut out);
            push(lib, &mut out);
        }
        push(root.join("lib/python3/dist-packages"), &mut out);
    }
    out
}

fn expand_home(path: &str, home: &Path) -> PathBuf {
    match path.strip_prefix("~/") {
        Some(rest) => home.join(rest),
        None => PathBuf::from(path),
    }
}

/// The environment of a Python program (`…/env/bin/python` → `…/env`) or
/// the folder itself.
fn env_root(p: &Path) -> PathBuf {
    if p.is_file() || !p.exists() {
        let bin = p.parent().unwrap_or(p);
        if bin.file_name().is_some_and(|n| n == "bin" || n == "Scripts") {
            return bin.parent().unwrap_or(bin).to_path_buf();
        }
        return bin.to_path_buf();
    }
    p.to_path_buf()
}

fn children(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> =
        std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    out.sort();
    out
}

/// `lib/python3.*` folders (newest first).
fn python_dirs(lib: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = children(lib)
        .into_iter()
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("python3")))
        .collect();
    out.sort_by_key(|p| {
        let n = p.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_owned();
        std::cmp::Reverse(n.trim_start_matches("python3.").parse::<u32>().unwrap_or(0))
    });
    out
}

/// The site-packages folders of an environment, then those of its base
/// Python if the venv includes them (`include-system-site-packages`).
fn site_packages(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for lib in ["lib", "lib64"] {
        for dir in python_dirs(&root.join(lib)) {
            out.push(dir.join("site-packages"));
        }
    }
    out.push(root.join("Lib/site-packages"));
    if let Ok(cfg) = std::fs::read_to_string(root.join("pyvenv.cfg")) {
        let value = |key: &str| {
            cfg.lines().find_map(|l| {
                let (k, v) = l.split_once('=')?;
                (k.trim() == key).then(|| v.trim().to_owned())
            })
        };
        if value("include-system-site-packages").is_some_and(|v| v == "true") {
            if let Some(home) = value("home") {
                let base = env_root(&Path::new(&home).join("python"));
                for lib in python_dirs(&base.join("lib")) {
                    out.push(lib.join("site-packages"));
                }
            }
        }
    }
    out
}

/// Environments in subfolders of the project (a folder with `pyvenv.cfg`),
/// at most a few levels deep.
fn find_venvs(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > 4 || out.len() > 64 {
        return;
    }
    for child in children(dir) {
        let name = child.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        if matches!(name, "node_modules" | ".git" | "target" | "__pycache__" | "site-packages" | ".atlas") {
            continue;
        }
        if child.join("pyvenv.cfg").is_file() || child.join("conda-meta").is_dir() {
            out.push(child);
            continue;
        }
        find_venvs(&child, depth + 1, out);
    }
}

/// Environments (and PYTHONPATH folders) of registered Jupyter kernels.
fn kernel_specs(home: &Path) -> Vec<(Vec<PathBuf>, Vec<PathBuf>)> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(d) = std::env::var_os("JUPYTER_DATA_DIR") {
        dirs.push(PathBuf::from(d));
    }
    if let Some(d) = std::env::var_os("XDG_DATA_HOME") {
        dirs.push(PathBuf::from(d).join("jupyter"));
    }
    dirs.push(home.join(".local/share/jupyter"));
    dirs.push(home.join("Library/Jupyter"));
    dirs.push(PathBuf::from("/usr/local/share/jupyter"));
    dirs.push(PathBuf::from("/usr/share/jupyter"));
    let mut out = Vec::new();
    for d in dirs {
        for kernel in children(&d.join("kernels")) {
            let Ok(text) = std::fs::read_to_string(kernel.join("kernel.json")) else { continue };
            let Ok(spec) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
            let mut roots = Vec::new();
            if let Some(python) = spec["argv"][0].as_str() {
                // The link to the environment's Python names the
                // environment (a venv's python links to the base one).
                let p = Path::new(python);
                roots.push(env_root(p));
                roots.push(env_root(&p.canonicalize().unwrap_or_else(|_| p.to_path_buf())));
            }
            let sites = spec["env"]["PYTHONPATH"]
                .as_str()
                .map(|p| std::env::split_paths(p).collect::<Vec<_>>())
                .unwrap_or_default();
            out.push((roots, sites));
        }
    }
    out
}

/// Where package `name` is: its folder (or `name.py`) in the first folder of
/// `path` that has it.
pub fn find_package(name: &str, path: &[PathBuf]) -> Option<(PathBuf, PathBuf)> {
    for site in path {
        let dir = site.join(name);
        if dir.join("__init__.py").is_file() || dir.join("__init__.pyi").is_file() {
            return Some((site.clone(), dir));
        }
        let file = site.join(format!("{name}.py"));
        if file.is_file() {
            return Some((site.clone(), file));
        }
    }
    None
}

/// The version of the distribution that installed `name` into `site`
/// (`*.dist-info/METADATA`), else `__version__ = "…"` in the package.
pub fn version(name: &str, site: &Path, package: &Path) -> String {
    let normalized = |s: &str| s.to_lowercase().replace(['-', '.'], "_");
    let mut fallback = None;
    for info in children(site) {
        let Some(dir) = info.file_name().and_then(|n| n.to_str()) else { continue };
        let Some(stem) = dir.strip_suffix(".dist-info").or_else(|| dir.strip_suffix(".egg-info")) else { continue };
        let (dist, ver) = stem.split_once('-').unwrap_or((stem, ""));
        let tops = std::fs::read_to_string(info.join("top_level.txt")).unwrap_or_default();
        let ours = normalized(dist) == normalized(name) || tops.lines().any(|l| l.trim() == name);
        if ours {
            let meta = std::fs::read_to_string(info.join("METADATA"))
                .or_else(|_| std::fs::read_to_string(info.join("PKG-INFO")));
            if let Some(v) =
                meta.ok().and_then(|m| m.lines().find_map(|l| l.strip_prefix("Version:").map(|v| v.trim().to_owned())))
            {
                return v;
            }
            return ver.to_owned();
        }
        if fallback.is_none() {
            // Distributions named differently (scikit-learn → sklearn)
            // without top_level.txt: their file list says it.
            let record = std::fs::read_to_string(info.join("RECORD")).unwrap_or_default();
            if record.lines().take(4000).any(|l| l.starts_with(&format!("{name}/"))) {
                fallback = Some(ver.to_owned());
            }
        }
    }
    if let Some(v) = fallback {
        return v;
    }
    // Packages without metadata (the standard library: json.__version__).
    for file in
        [package.join("__init__.py"), package.join("version.py"), package.join("_version.py"), package.to_path_buf()]
    {
        if let Ok(text) = std::fs::read_to_string(&file) {
            for line in text.lines().take(400) {
                let line = line.trim();
                for key in ["__version__", "version"] {
                    if let Some(rest) = line.strip_prefix(key) {
                        let rest = rest.trim_start();
                        if let Some(v) = rest.strip_prefix('=') {
                            let v = v.trim().trim_matches(|c| c == '"' || c == '\'');
                            if !v.is_empty() && v.chars().next().is_some_and(|c| c.is_ascii_digit()) && !v.contains(' ')
                            {
                                return v.to_owned();
                            }
                        }
                    }
                }
            }
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_packages_in_project_environments_with_versions() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("projekt");
        let site = project.join("kurs/.venv/lib/python3.12/site-packages");
        std::fs::create_dir_all(site.join("demo")).unwrap();
        std::fs::write(project.join("kurs/.venv/pyvenv.cfg"), "home = /nirgends/bin\n").unwrap();
        std::fs::write(site.join("demo/__init__.py"), "").unwrap();
        std::fs::create_dir_all(site.join("demo_dist-1.2.3.dist-info")).unwrap();
        std::fs::write(site.join("demo_dist-1.2.3.dist-info/METADATA"), "Name: demo-dist\nVersion: 1.2.3\n").unwrap();
        std::fs::write(site.join("demo_dist-1.2.3.dist-info/RECORD"), "demo/__init__.py,,\n").unwrap();
        let path = search_path("", Some(&project));
        assert_eq!(path[0], site);
        let (found_site, pkg) = find_package("demo", &path).unwrap();
        assert_eq!(pkg, site.join("demo"));
        assert_eq!(version("demo", &found_site, &pkg), "1.2.3");
        // The setting: an environment's Python comes first.
        let other = dir.path().join("andere/lib/python3.11/site-packages");
        std::fs::create_dir_all(&other).unwrap();
        let path = search_path(&dir.path().join("andere/bin/python").to_string_lossy(), Some(&project));
        assert_eq!(path[0], other);
    }

    #[test]
    fn version_from_the_package_itself() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("json")).unwrap();
        std::fs::write(dir.path().join("json/__init__.py"), "\"\"\"Doc\"\"\"\n__version__ = '2.0.9'\n").unwrap();
        assert_eq!(version("json", dir.path(), &dir.path().join("json")), "2.0.9");
    }
}
