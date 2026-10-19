// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! The places in the sidebar when `[places] file_manager` is on:
//!
//! - with KDE/Dolphin's `$XDG_DATA_HOME/user-places.xbel` (shared with the
//!   KDE file dialogs): exactly Dolphin's places, in its order, instead of
//!   Atlas' fixed ones – a place removed or hidden there is gone here too;
//! - otherwise Atlas' fixed places, followed by the GTK bookmarks
//!   (`$XDG_CONFIG_HOME/gtk-3.0/bookmarks`; Nautilus, Thunar, …).
//!
//! Only local folders that exist are taken, and `trash:/` as Atlas' trash;
//! hidden entries, entries limited to another application, network places
//! (`sftp://`, `smb://`, …) and other KIO places (`remote:/`, …) are left
//! out. Devices are not in these files (Dolphin asks UDisks2 for them).

use std::path::{Path, PathBuf};

use crate::listing::Place;

/// The files the places are read from (for watching).
pub fn sources() -> Vec<PathBuf> {
    sources_pair().into_iter().flatten().collect()
}

/// The sidebar's places and how many of them are Atlas' `fixed` ones (0
/// when Dolphin's replace them).
pub fn sidebar_places(fixed: Vec<Place>, file_manager: bool) -> (Vec<Place>, usize) {
    let count = fixed.len();
    if !file_manager {
        return (fixed, count);
    }
    let [xbel, gtk] = sources_pair().map(|f| f.and_then(|f| std::fs::read_to_string(f).ok()));
    choose(fixed, xbel.as_deref(), gtk.as_deref())
}

/// `sidebar_places` for the given file contents (`None`: no such file).
fn choose(fixed: Vec<Place>, xbel: Option<&str>, gtk: Option<&str>) -> (Vec<Place>, usize) {
    let count = fixed.len();
    if let Some(dolphin) = xbel.and_then(parse_xbel) {
        return (select(dolphin, &[]), 0);
    }
    let bookmarks = gtk.map(parse_gtk_bookmarks).unwrap_or_default();
    let extra = select(bookmarks, &fixed);
    let mut all = fixed;
    all.extend(extra);
    (all, count)
}

fn sources_pair() -> [Option<PathBuf>; 2] {
    [
        dirs::data_dir().map(|d| d.join("user-places.xbel")),
        dirs::config_dir().map(|d| d.join("gtk-3.0").join("bookmarks")),
    ]
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

/// Places of a KDE `user-places.xbel` (`None` if it is not valid XML).
pub fn parse_xbel(text: &str) -> Option<Vec<Place>> {
    // KDE writes `<!DOCTYPE xbel>`.
    let options = roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() };
    let Ok(doc) = roxmltree::Document::parse_with_options(text, options) else {
        log::warn!("user-places.xbel is not valid XML");
        return None;
    };
    let mut out = Vec::new();
    for bookmark in doc.descendants().filter(|n| n.is_element() && n.tag_name().name() == "bookmark") {
        let href = bookmark.attribute("href").unwrap_or("");
        let trash = href.starts_with("trash:");
        let path = if trash { crate::listing::trash_files_dir() } else { local_path(href) };
        let Some(path) = path else { continue };
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
        let mut title = child_text("title");
        // KDE keeps its own places with English titles and translates them
        // for display.
        if child_text("isSystemItem") == "true" {
            title = system_title(&title).map(str::to_owned).unwrap_or(title);
        }
        out.push(place(title, if trash { "user-trash" } else { icon }, path));
    }
    Some(out)
}

fn system_title(title: &str) -> Option<&'static str> {
    Some(match title {
        "Home" => "Persönlicher Ordner",
        "Desktop" => "Schreibtisch",
        "Documents" => "Dokumente",
        "Downloads" => "Downloads",
        "Pictures" => "Bilder",
        "Music" => "Musik",
        "Videos" => "Videos",
        "Trash" => "Papierkorb",
        "Root" => "Basisordner",
        _ => return None,
    })
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
        let places = parse_xbel(XBEL).unwrap();
        let summary: Vec<(String, String, String)> =
            places.iter().map(|p| (p.name.clone(), p.icon.clone(), p.path.to_string_lossy().into_owned())).collect();
        assert_eq!(
            summary,
            vec![
                // A KDE system place, translated.
                ("Persönlicher Ordner".into(), "home".into(), "/home/leo".into()),
                ("Arbeit".into(), "folder".into(), "/home/leo/Projects/Meine Arbeit".into()),
                // trash:/ is Atlas' trash.
                (
                    "Papierkorb".into(),
                    "trash".into(),
                    crate::listing::trash_files_dir().unwrap().to_string_lossy().into_owned()
                ),
                // No title: the folder's name.
                ("Musik".into(), "music".into(), "/home/leo/Musik".into()),
            ]
        );
        assert!(parse_xbel("<kaputt").is_none());
    }

    #[test]
    fn dolphin_replaces_the_fixed_places() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = |name: &str| {
            let d = tmp.path().join(name);
            std::fs::create_dir_all(&d).unwrap();
            normalize(&d)
        };
        let (home, desk, work) = (dir("home"), dir("Schreibtisch"), dir("arbeit"));
        let fixed = vec![
            Place { name: "Persönlicher Ordner".into(), icon: "home".into(), path: home.clone() },
            Place { name: "Schreibtisch".into(), icon: "desktop".into(), path: desk.clone() },
        ];
        // The desktop was removed in Dolphin, a work folder added before home.
        let xbel = format!(
            "<?xml version=\"1.0\"?>\n<!DOCTYPE xbel>\n<xbel>\
             <bookmark href=\"file://{}\"><title>Arbeit</title></bookmark>\
             <bookmark href=\"file://{}\"><title>Home</title><info><metadata><isSystemItem>true</isSystemItem></metadata></info></bookmark>\
             </xbel>",
            work.display(),
            home.display()
        );
        let gtk = format!("file://{} GTK", desk.display());
        let (places, count) = choose(fixed.clone(), Some(&xbel), Some(&gtk));
        let names: Vec<&str> = places.iter().map(|p| p.name.as_str()).collect();
        assert_eq!((names, count), (vec!["Arbeit", "Persönlicher Ordner"], 0));
        // Everything removed in Dolphin: nothing shown.
        assert!(choose(fixed.clone(), Some("<xbel/>"), None).0.is_empty());
        // No (valid) Dolphin file: fixed places plus GTK bookmarks.
        let (places, count) =
            choose(fixed.clone(), Some("<kaputt"), Some(&format!("file://{} Arbeit", work.display())));
        assert_eq!((places.len(), count), (3, 2));
        assert_eq!(choose(fixed.clone(), None, None), (fixed, 2));
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
