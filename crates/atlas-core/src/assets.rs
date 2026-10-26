// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Pictures for notes: pasted, dropped or cut out of a PDF, they go into one
//! folder of the project (`assets/`; next to the note outside a project)
//! and the note links them relative to itself, so the link also works in
//! other Markdown tools.

use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};

/// Folder for pictures in the project.
pub const ASSETS_DIR: &str = "assets";

/// File types inserted as pictures (the others as links).
pub fn is_image(path: &Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "avif")
}

/// Where the pictures of `note` go.
pub fn assets_dir(root: Option<&Path>, note: &Path) -> PathBuf {
    match root {
        Some(root) if note.starts_with(root) => root.join(ASSETS_DIR),
        _ => note.parent().unwrap_or(Path::new("/")).join(ASSETS_DIR),
    }
}

/// `dir/name`, or `dir/stem-2.ext`, `-3` … if that exists.
pub fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let candidate = dir.join(name);
    if !candidate.exists() {
        return candidate;
    }
    let path = Path::new(name);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("bild");
    let ext = path.extension().and_then(|e| e.to_str()).map(|e| format!(".{e}")).unwrap_or_default();
    (2..).map(|n| dir.join(format!("{stem}-{n}{ext}"))).find(|p| !p.exists()).expect("a free name")
}

/// A name for a new picture of `note`: `<note>-<date-time>.png`.
pub fn new_image_name(note: &Path, ext: &str) -> String {
    let stem = note.file_stem().and_then(|s| s.to_str()).unwrap_or("bild");
    let stem: String =
        stem.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect();
    format!("{stem}-{}.{ext}", chrono::Local::now().format("%Y%m%d-%H%M%S"))
}

/// `target` relative to the folder of `note` (`../assets/x.png`), as a
/// Markdown link destination (spaces and brackets encoded).
pub fn link_from(note: &Path, target: &Path) -> String {
    let from: Vec<Component> = note.parent().unwrap_or(Path::new("/")).components().collect();
    let to: Vec<Component> = target.components().collect();
    let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut rel = PathBuf::new();
    for _ in common..from.len() {
        rel.push("..");
    }
    for c in &to[common..] {
        rel.push(c.as_os_str());
    }
    crate::markdown::percent_encode_path(&rel.to_string_lossy())
}

/// Markdown for a picture: `![name](link)`.
pub fn image_markdown(note: &Path, image: &Path) -> String {
    let alt = image.file_stem().and_then(|s| s.to_str()).unwrap_or("Bild");
    format!("![{alt}]({})", link_from(note, image))
}

/// Copies `source` into the assets folder of `note` and returns the
/// Markdown for it: a picture, or a link for other files. A file already
/// in the project is linked where it is.
pub fn import(root: Option<&Path>, note: &Path, source: &Path) -> Result<String> {
    let in_project = root.is_some_and(|r| source.starts_with(r));
    let target = if in_project {
        source.to_path_buf()
    } else {
        let dir = assets_dir(root, note);
        std::fs::create_dir_all(&dir).with_context(|| format!("cannot create {}", dir.display()))?;
        let name = source.file_name().and_then(|n| n.to_str()).context("no file name")?;
        let target = unique_path(&dir, name);
        std::fs::copy(source, &target).with_context(|| format!("cannot copy {}", source.display()))?;
        target
    };
    if is_image(&target) {
        Ok(image_markdown(note, &target))
    } else {
        let name = target.file_name().and_then(|n| n.to_str()).unwrap_or("Datei");
        Ok(format!("[{name}]({})", link_from(note, &target)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_relative_to_the_note() {
        assert_eq!(link_from(Path::new("/p/notes/a.md"), Path::new("/p/assets/x.png")), "../assets/x.png");
        assert_eq!(link_from(Path::new("/p/a.md"), Path::new("/p/assets/mein bild.png")), "assets/mein%20bild.png");
        assert_eq!(link_from(Path::new("/p/n/t/a.md"), Path::new("/p/n/t/b.png")), "b.png");
    }

    #[test]
    fn assets_folder_of_the_project_or_next_to_the_note() {
        assert_eq!(assets_dir(Some(Path::new("/p")), Path::new("/p/notes/a.md")), Path::new("/p/assets"));
        assert_eq!(assets_dir(Some(Path::new("/p")), Path::new("/elsewhere/a.md")), Path::new("/elsewhere/assets"));
        assert_eq!(assets_dir(None, Path::new("/x/a.md")), Path::new("/x/assets"));
    }

    #[test]
    fn imports_copies_with_free_names_and_links_project_files_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("p");
        std::fs::create_dir_all(root.join("notes")).unwrap();
        let note = root.join("notes/a.md");
        let outside = dir.path().join("foto.png");
        std::fs::write(&outside, b"png").unwrap();
        assert_eq!(import(Some(&root), &note, &outside).unwrap(), "![foto](../assets/foto.png)");
        assert_eq!(import(Some(&root), &note, &outside).unwrap(), "![foto-2](../assets/foto-2.png)");
        assert!(root.join("assets/foto-2.png").exists());
        let inside = root.join("notes/skript.pdf");
        std::fs::write(&inside, b"%PDF").unwrap();
        assert_eq!(import(Some(&root), &note, &inside).unwrap(), "[skript.pdf](skript.pdf)");
        let name = new_image_name(&note, "png");
        assert!(name.starts_with("a-") && name.ends_with(".png"), "{name}");
    }
}
