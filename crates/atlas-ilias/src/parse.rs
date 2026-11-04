// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! ILIAS pages: login state, the objects (courses, groups, folders, files) linked on them.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use url::Url;

/// An ILIAS object: course (`crs`), group (`grp`), session (`sess`),
/// folder (`fold`) or file (`file`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub kind: String,
    pub id: String,
    pub name: String,
    pub url: String,
    /// What ILIAS shows about it (type, size, version, date): a changed
    /// text means a new version.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub info: String,
    /// Files: the address that sends the file itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download: Option<String>,
    /// Containers: what is inside (None for files).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<Node>>,
    /// Containers that could not be read.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub error: String,
}

impl Node {
    pub(super) fn key(&self) -> String {
        format!("{}{}", self.kind, self.id)
    }
    pub(super) fn is_container(&self) -> bool {
        self.kind != "file"
    }
}

/// Logged in on page `at`: it links to the logout (ILIAS 7: `logout.php`,
/// 8 and later: `cmd=doLogout`, in any case); else it must not be a login
/// page (address `login.php` or Shibboleth, a password field, or a link to
/// log in – ILIAS shows one to visitors).
pub fn logged_in(html: &str, at: &Url) -> bool {
    login_check(&Html::parse_document(html), at).0
}

/// [`logged_in`] with the reason, for the log.
pub(super) fn login_check(doc: &Html, at: &Url) -> (bool, String) {
    let targets = Selector::parse("a[href], form[action]").unwrap();
    let addresses: Vec<String> = doc
        .select(&targets)
        .filter_map(|e| e.value().attr("href").or(e.value().attr("action")))
        .map(str::to_lowercase)
        .collect();
    if let Some(a) = addresses.iter().find(|a| a.contains("logout")) {
        return (true, format!("Abmelde-Link {a}"));
    }
    let login = |a: &str| a.contains("login.php") || a.contains("shib_login") || a.contains("cmd=showlogin");
    if login(&at.as_str().to_lowercase()) {
        return (false, "Adresse ist eine Anmeldeseite".into());
    }
    let password = Selector::parse(r#"input[type="password" i]"#).unwrap();
    if doc.select(&password).next().is_some() {
        return (false, "Seite hat ein Passwortfeld".into());
    }
    if let Some(a) = addresses.iter().find(|a| login(a)) {
        return (false, format!("Seite verlinkt die Anmeldung ({a})"));
    }
    (true, "keine Anmeldeseite (kein Abmelde-Link gefunden)".into())
}

pub(super) fn page_title(doc: &Html) -> String {
    let sel = Selector::parse("title").unwrap();
    doc.select(&sel).next().map(|t| clean(&t.text().collect::<String>())).unwrap_or_default()
}

// Patterns of ILIAS addresses, built once.
pub(super) fn regex(pattern: &str) -> Regex {
    Regex::new(pattern).expect("valid pattern")
}
static LINK_PATTERNS: LazyLock<[Regex; 3]> = LazyLock::new(|| {
    [
        regex(r"(?i)[?&]target=(crs|grp|sess|fold|file)_(\d+)"),
        regex(r"(?i)goto_[^/?#]*?_(crs|grp|sess|fold|file)_(\d+)"),
        // `/go/crs/12` and, from ILIAS 9 on, `goto.php/crs/12`.
        regex(r"(?i)/go(?:to\.php)?/(crs|grp|sess|fold|file)/(\d+)"),
    ]
});
static REF_ID: LazyLock<Regex> = LazyLock::new(|| regex(r"(?i)[?&]ref_id=(\d+)"));
static CMD_CLASS: LazyLock<Regex> =
    LazyLock::new(|| regex(r"(?i)[?&]cmdClass=ilObj(Course|Group|Session|Folder|File)GUI\b"));
static ICON_KIND: LazyLock<Regex> = LazyLock::new(|| regex(r"(?i)icon_(crs|grp|sess|fold|file)\b"));
static TARGET_FILE: LazyLock<Regex> = LazyLock::new(|| regex(r"(?i)(target=file_\d+)"));

/// Kind and ID of an ILIAS object link: from permalinks (`target=fold_12`,
/// `goto_…_fold_12.html`, `/go/fold/12`, `goto.php/fold/12`), from the
/// GUI class of `ref_id` links (`cmdClass=ilObjFolderGUI`) or, for
/// other links with a
/// `ref_id`, from the type icon of the list item.
pub(super) fn classify(href: &str, icon: Option<&str>) -> Option<(String, String)> {
    let decoded = atlas_core::text::percent_decode(href);
    for re in LINK_PATTERNS.iter() {
        if let Some(c) = re.captures(&decoded) {
            return Some((c[1].to_lowercase(), c[2].to_owned()));
        }
    }
    let ref_id = REF_ID.captures(&decoded)?;
    // `ilias.php?…&cmdClass=ilObjFileGUI&cmd=sendfile&ref_id=12`: the GUI
    // class names the type (KIT links files and folders like this).
    if let Some(class) = CMD_CLASS.captures(&decoded) {
        let kind = match class[1].to_lowercase().as_str() {
            "course" => "crs",
            "group" => "grp",
            "session" => "sess",
            "folder" => "fold",
            _ => "file",
        };
        return Some((kind.to_owned(), ref_id[1].to_owned()));
    }
    let kind = ICON_KIND.captures(icon?)?;
    Some((kind[1].to_lowercase(), ref_id[1].to_owned()))
}

/// Address that sends the file itself.
pub(super) fn download_url(href: &str, id: &str, page: &Url) -> String {
    let lower = href.to_lowercase();
    if lower.contains("_download")
        || lower.contains("/download")
        || lower.contains("cmd=sendfile")
        || lower.contains("cmd=download")
    {
        return href.to_owned();
    }
    if let Some(m) = TARGET_FILE.find(href) {
        return format!("{}_download{}", &href[..m.end()], &href[m.end()..]);
    }
    page.join(&format!("goto.php?target=file_{id}_download")).map(|u| u.to_string()).unwrap_or_default()
}

fn clean(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Classes and IDs of navigation, tabs and breadcrumbs: their links lead
/// back up or sideways, not into the container.
fn in_navigation(a: ElementRef) -> bool {
    a.ancestors().filter_map(ElementRef::wrap).any(|e| {
        let el = e.value();
        // (Not `#mainspacekeeper`: in ILIAS it holds the whole content.)
        matches!(el.name(), "nav" | "header")
            || el.classes().any(|c| {
                matches!(
                    c,
                    "breadcrumb"
                            | "il-breadcrumbs"
                            | "ilTabs"
                            | "ilSubTabs"
                            | "il-maincontrols"
                            | "il-mainbar"
                            | "il-metabar"
                            | "ilLocator"
                            // Side blocks (news, calendar …) link other courses.
                            | "ilBlock"
                )
            })
            || matches!(el.id(), Some("il_right_col" | "il_left_col"))
    })
}

/// The list item a link belongs to (title, icon and properties are there):
/// a row of the list view or a card of the tile view.
fn list_item(a: ElementRef) -> Option<ElementRef> {
    a.ancestors().filter_map(ElementRef::wrap).find(|e| {
        let el = e.value();
        matches!(el.name(), "li" | "tr")
            || el.classes().any(|c| {
                matches!(c, "il_ContainerListItem" | "il-std-item" | "il-item" | "ilObjListRow" | "il-card" | "card")
            })
    })
}

/// The type icon of a list item, as `icon_<kind>`: from the image file
/// (`icon_fold.svg`) or, in ILIAS 9, its classes (`icon fold medium`).
fn item_icon(item: ElementRef) -> Option<String> {
    let images = Selector::parse("img").unwrap();
    let kind = &*ICON_KIND;
    item.select(&images).find_map(|img| {
        let el = img.value();
        if let Some(src) = el.attr("src").filter(|s| kind.is_match(s)) {
            return Some(src.to_owned());
        }
        let classes: Vec<&str> = el.classes().collect();
        classes
            .contains(&"icon")
            .then(|| {
                classes
                    .iter()
                    .find(|c| matches!(**c, "crs" | "grp" | "sess" | "fold" | "file"))
                    .map(|k| format!("icon_{k}"))
            })
            .flatten()
    })
}

/// The ILIAS objects linked in the main content of a page, in order and
/// each once, with name and info.
pub fn objects(html: &str, page: &Url) -> Vec<Node> {
    let doc = Html::parse_document(html);
    let main = Selector::parse("#il_center_col, main, [role=main]").unwrap();
    let links = Selector::parse("a[href]").unwrap();
    let title_sel = Selector::parse(".il_ContainerItemTitle, .il-item-title, .card-title").unwrap();
    let props_sel = Selector::parse(".il_ItemProperties, .il-item-properties").unwrap();
    let root = doc.select(&main).next().unwrap_or_else(|| doc.root_element());
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for a in root.select(&links) {
        if in_navigation(a) {
            continue;
        }
        let Some(href) = a.value().attr("href").and_then(|h| page.join(h).ok()) else { continue };
        if href.host_str() != page.host_str() {
            continue;
        }
        let item = list_item(a);
        let icon = item.and_then(item_icon);
        let Some((kind, id)) = classify(href.as_str(), icon.as_deref()) else { continue };
        let title = item.and_then(|i| i.select(&title_sel).next()).map(|t| t.text().collect::<String>());
        let name = clean(&title.unwrap_or_else(|| a.text().collect()));
        let name = if name.is_empty() { clean(a.value().attr("title").unwrap_or_default()) } else { name };
        if name.is_empty() || !seen.insert(format!("{kind}{id}")) {
            continue;
        }
        let info = item
            .map(|i| {
                i.select(&props_sel)
                    .map(|p| clean(&p.text().collect::<Vec<_>>().join(" ")))
                    .collect::<Vec<_>>()
                    .join(" · ")
            })
            .unwrap_or_default();
        let href = href.to_string();
        out.push(Node {
            download: (kind == "file").then(|| download_url(&href, &id, page)),
            children: None,
            kind,
            id,
            name,
            url: href,
            info,
            error: String::new(),
        });
    }
    out
}

/// Why a page yields no objects (for the log): which part of the page was
/// read, how many links it has and why they were left out, with examples.
pub fn explain(html: &str, page: &Url) -> String {
    let doc = Html::parse_document(html);
    let main = Selector::parse("#il_center_col, main, [role=main]").unwrap();
    let links = Selector::parse("a[href]").unwrap();
    let (root, part) = match doc.select(&main).next() {
        Some(e) => {
            (e, format!("<{}{}>", e.value().name(), e.value().id().map(|i| format!(" id={i}")).unwrap_or_default()))
        }
        None => (doc.root_element(), "ganze Seite".to_owned()),
    };
    let (mut total, mut nav, mut foreign, mut unknown) = (0, 0, 0, Vec::new());
    for a in root.select(&links) {
        total += 1;
        if in_navigation(a) {
            nav += 1;
            continue;
        }
        let Some(href) = a.value().attr("href").and_then(|h| page.join(h).ok()) else { continue };
        if href.host_str() != page.host_str() {
            foreign += 1;
            continue;
        }
        let icon = list_item(a).and_then(item_icon);
        if classify(href.as_str(), icon.as_deref()).is_none() {
            let shown = format!("{}{}", href.path(), href.query().map(|q| format!("?{q}")).unwrap_or_default());
            unknown.push(shown.chars().take(110).collect::<String>());
        }
    }
    let all = doc.select(&links).count();
    let mut examples = unknown.clone();
    examples.sort();
    examples.dedup();
    examples.truncate(12);
    format!(
        "gelesen: {part} · {total} von {all} Links · {nav} in Navigation · {foreign} zu anderen Rechnern · {} nicht erkannt, z. B.: {}",
        unknown.len(),
        examples.join("  |  ")
    )
}

/// What was found on a page, by kind (for the log).
pub(super) fn summary(found: &[Node]) -> String {
    if found.is_empty() {
        return "nichts gefunden".to_owned();
    }
    let mut counts: Vec<(String, usize)> = Vec::new();
    for o in found {
        match counts.iter_mut().find(|(k, _)| *k == o.kind) {
            Some((_, n)) => *n += 1,
            None => counts.push((o.kind.clone(), 1)),
        }
    }
    counts.iter().map(|(k, n)| format!("{n}× {k}")).collect::<Vec<_>>().join(", ")
}
