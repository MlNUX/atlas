// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! The places of the desktop's file managers, shown below Atlas' fixed
//! places when `[places] file_manager` is on:
//!
//! - KDE/Dolphin: `$XDG_DATA_HOME/user-places.xbel` (XBEL, shared with the
//!   KDE file dialogs);
//! - GTK (Nautilus, Thunar, …): `$XDG_CONFIG_HOME/gtk-3.0/bookmarks`.
//!
//! Only local folders that exist are taken; hidden entries, entries limited
//! to another application, network places (`sftp://`, `smb://`, …) and
//! special KIO places (`trash:/`, `remote:/`) are left out. Devices are not
//! in these files (Dolphin asks UDisks2 for them).

use std::path::{Path, PathBuf};

use crate::listing::Place;

/// The files the places are read from (for watching).
pub fn sources() -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Some(dir) = dirs::data_dir() {
        files.push(dir.join("user-places.xbel"));
    }
    if let Some(dir) = dirs::config_dir() {
        files.push(dir.join("gtk-3.0").join("bookmarks"));
    }
    files
}

/// The file managers' places that are not among `fixed`, in their order
/// (KDE first, then GTK), each folder once.
pub fn file_manager_places(fixed: &[Place]) -> Vec<Place> {
    let mut found = Vec::new();
    for source in sources() {
        let Ok(text) = std::fs::read_to_string(&source) else { continue };
        if source.extension().is_some_and(|e| e == "xbel") {
            found.extend(parse_xbel(&text));
        } else {
            found.extend(parse_gtk_bookmarks(&text));
        }
    }
    select(found, fixed)
}

/// Drops duplicates (also of `fixed`) and folders that do not exist.
fn select(found: Vec<Place>, fixed: &[Place]) -> Vec<Place> {
    let mut seen: Vec<PathBuf> = fixed.iter().map(|p| normalize(&p.path)).collect();
    let mut out = Vec::new();
    for place in found {
        let path = normalize(&place.path);
        if seen.contains(&path) || !path.is_dir() {
            continue;
        }
        seen.push(path.clone());
        out.push(Place { path, ..place });
    }
    out
}

fn normalize(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Places of a KDE `user-places.xbel`.
pub fn parse_xbel(text: &str) -> Vec<Place> {
    // KDE writes `<!DOCTYPE xbel>`.
    let options = roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() };
    let Ok(doc) = roxmltree::Document::parse_with_options(text, options) else {
        log::warn!("user-places.xbel is not valid XML");
        return Vec::new();
    };
    let mut out = Vec::new();
    for bookmark in doc.descendants().filter(|n| n.is_element() && n.tag_name().name() == "bookmark") {
        let Some(path) = bookmark.attribute("href").and_then(local_path) else { continue };
        let child_text = |name: &str| {
            bookmark
                .descendants()
                .find(|n| n.is_element() && n.tag_name().name() == name)
                .and_then(|n| n.text())
                .map(|t| t.trim().to_owned())
                .unwrap_or_default()
        };
        if child_text("IsHidden") == "true" || !child_text("OnlyInApp").is_empty() {
            continue;
        }
        let icon = bookmark
            .descendants()
            .find(|n| n.is_element() && n.tag_name().name() == "icon")
            .and_then(|n| n.attribute("name"))
            .unwrap_or("");
        let title = child_text("title");
        out.push(place(title, icon, path));
    }
    out
}

/// Places of a GTK `bookmarks` file: `URI [label]` per line.
pub fn parse_gtk_bookmarks(text: &str) -> Vec<Place> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            let (uri, label) = line.split_once(' ').unwrap_or((line, ""));
            local_path(uri).map(|path| place(label.trim().to_owned(), "", path))
        })
        .collect()
}

fn place(title: String, icon: &str, path: PathBuf) -> Place {
    let name = if title.is_empty() {
        path.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string_lossy().into_owned())
    } else {
        title
    };
    Place { name, icon: atlas_icon(icon).to_owned(), path }
}

/// The path of a `file://` URI (percent-decoded); `None` for other schemes.
fn local_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    // `file:///path` or `file://localhost/path`.
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    if !rest.starts_with('/') {
        return None;
    }
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Some(b) = std::str::from_utf8(&bytes[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(b);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    use std::os::unix::ffi::OsStringExt;
    Some(PathBuf::from(std::ffi::OsString::from_vec(out)))
}

/// Atlas' icon for a freedesktop icon name.
fn atlas_icon(name: &str) -> &'static str {
    match name {
        "user-home" | "folder-home" => "home",
        "user-desktop" => "desktop",
        "folder-documents" => "documents",
        "folder-download" | "folder-downloads" => "downloads",
        "folder-pictures" | "folder-images" => "pictures",
        "folder-music" | "folder-sound" => "music",
        "folder-videos" | "folder-video" => "videos",
        "user-trash" => "trash",
        _ => "folder",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const XBEL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE xbel>
<xbel xmlns:bookmark="http://www.freedesktop.org/standards/desktop-bookmarks" xmlns:mime="http://www.freedesktop.org/standards/shared-mime-info" xmlns:kdepriv="http://www.kde.org/kdepriv">
 <info><metadata owner="http://www.kde.org"><kde_places_version>4</kde_places_version></metadata></info>
 <bookmark href="file:///home/leo">
  <title>Home</title>
  <info>
   <metadata owner="http://freedesktop.org"><bookmark:icon name="user-home"/></metadata>
   <metadata owner="http://www.kde.org"><ID>1/0</ID><isSystemItem>true</isSystemItem></metadata>
  </info>
 </bookmark>
 <bookmark href="file:///home/leo/Projects/Meine%20Arbeit">
  <title>Arbeit</title>
  <info><metadata owner="http://freedesktop.org"><bookmark:icon name="folder-blue"/></metadata></info>
 </bookmark>
 <bookmark href="file:///home/leo/Versteckt">
  <title>Versteckt</title>
  <info><metadata owner="http://www.kde.org"><IsHidden>true</IsHidden></metadata></info>
 </bookmark>
 <bookmark href="file:///home/leo/NurDolphin">
  <title>Nur Dolphin</title>
  <info><metadata owner="http://www.kde.org"><OnlyInApp>org.kde.dolphin</OnlyInApp></metadata></info>
 </bookmark>
 <bookmark href="trash:/"><title>Papierkorb</title></bookmark>
 <bookmark href="sftp://server/home"><title>Server</title></bookmark>
 <separator/>
 <bookmark href="file:///home/leo/Musik"><title></title>
  <info><metadata owner="http://freedesktop.org"><bookmark:icon name="folder-music"/></metadata></info>
 </bookmark>
</xbel>"#;

    #[test]
    fn xbel_local_visible_places() {
        let places = parse_xbel(XBEL);
        let summary: Vec<(String, String, String)> =
            places.iter().map(|p| (p.name.clone(), p.icon.clone(), p.path.to_string_lossy().into_owned())).collect();
        assert_eq!(
            summary,
            vec![
                ("Home".into(), "home".into(), "/home/leo".into()),
                ("Arbeit".into(), "folder".into(), "/home/leo/Projects/Meine Arbeit".into()),
                // No title: the folder's name.
                ("Musik".into(), "music".into(), "/home/leo/Musik".into()),
            ]
        );
        assert!(parse_xbel("<kaputt").is_empty());
    }

    #[test]
    fn gtk_bookmarks() {
        let places = parse_gtk_bookmarks("file:///tmp/a%C3%A4 Ablage\nsftp://host/x\nfile:///tmp/ohne\n\n");
        assert_eq!(places.len(), 2);
        assert_eq!((places[0].name.as_str(), places[0].path.as_path()), ("Ablage", Path::new("/tmp/aä")));
        assert_eq!(places[1].name, "ohne");
    }

    #[test]
    fn duplicates_and_missing_folders_dropped() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        let p = |path: &Path| Place { name: "x".into(), icon: "folder".into(), path: path.to_path_buf() };
        let fixed = vec![p(&a)];
        let found = vec![p(&a), p(&b), p(&b.join("../b")), p(&tmp.path().join("fehlt"))];
        let out = select(found, &fixed);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].path, normalize(&b));
    }
}
