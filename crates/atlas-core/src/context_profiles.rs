// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Profiles of the context bar (docs/concept/kontextleiste.html): which
//! blocks there are and how a profile lays them out for a file.
//!
//! A profile has lists of block ids – `tabs` (after „Übersicht“),
//! `overview`, `pinned` (bottom edge, on every tab), `hidden`, `collapsed` –
//! and `pages`: each tab is a page of blocks like the overview, built in
//! (`links`, `graph`, `actions`), a single block shown as a tab, or a page
//! of one's own (`page:…`) with a title. They live in
//! `[context.profiles.<name>]` of config.toml. `default` („Allgemein“)
//! applies to everything without a profile of its own and fills what
//! another profile leaves out. Without any profile the built-in layout
//! applies, with the old switches (`calendar = false` …) as hidden blocks.
//!
//! Display texts (block names, page titles of built-in tabs) belong to the
//! user interface; here a page title is empty when the interface supplies
//! it (built-in tabs, single blocks).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::config::{ContextConfig, ContextPage, ContextProfile};

/// A block of the bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Block {
    pub id: &'static str,
    pub icon: &'static str,
    /// May be a tab of its own.
    pub tab: bool,
    /// May be pinned to the bottom edge.
    pub pin: bool,
    /// Group in the gallery: `file`, `plan` (`ext` for extensions).
    pub group: &'static str,
}

const fn block(id: &'static str, icon: &'static str, tab: bool, pin: bool, group: &'static str) -> Block {
    Block { id, icon, tab, pin, group }
}

/// All built-in blocks, in the order of the gallery.
pub const CATALOG: &[Block] = &[
    block("annotations", "highlighter", true, false, "file"),
    block("outline", "list", true, false, "file"),
    block("links", "link", true, false, "file"),
    block("tags", "bookmark", false, false, "file"),
    block("fileinfo", "info", false, false, "file"),
    block("graph", "graph", true, false, "file"),
    block("actions", "play", true, false, "file"),
    block("builtin-actions", "play", false, false, "file"),
    block("user-actions", "terminal", false, false, "file"),
    block("today", "calendar", false, true, "plan"),
    block("week", "calendar", false, false, "plan"),
    block("tasks", "check-square", false, true, "plan"),
    block("pomodoro", "focus", false, true, "plan"),
];

/// Groups of the gallery, in order.
pub const GROUPS: &[&str] = &["file", "plan", "ext"];
/// Profiles in the order of the settings. Others than `default` exist only
/// once set; until then `default` applies.
pub const NAMES: &[&str] = &["default", "pdf", "markdown", "code", "calendar"];
/// The lists of a profile (besides `pages`).
pub const KEYS: &[&str] = &["tabs", "overview", "pinned", "hidden", "collapsed"];
/// Blocks shown at full height when they are alone on a page.
pub const FULL_BLOCKS: &[&str] = &["links", "graph"];

pub fn catalog(id: &str) -> Option<&'static Block> {
    CATALOG.iter().find(|b| b.id == id)
}

/// Blocks of a built-in tab: one big block, or two for `actions`.
pub fn builtin_page(id: &str) -> Option<&'static [&'static str]> {
    match id {
        "links" => Some(&["links"]),
        "graph" => Some(&["graph"]),
        "actions" => Some(&["builtin-actions", "user-actions"]),
        _ => None,
    }
}

fn strings(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|s| (*s).to_owned()).collect()
}

/// All lists of a profile, as edited.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lists {
    #[serde(default)]
    pub tabs: Vec<String>,
    #[serde(default)]
    pub overview: Vec<String>,
    #[serde(default)]
    pub pinned: Vec<String>,
    #[serde(default)]
    pub hidden: Vec<String>,
    #[serde(default)]
    pub collapsed: Vec<String>,
    #[serde(default)]
    pub pages: BTreeMap<String, ContextPage>,
}

impl Lists {
    /// The layout without any profile (as the bar was before profiles).
    pub fn builtin() -> Self {
        Lists {
            tabs: strings(&["links", "graph", "actions"]),
            overview: strings(&["today", "tasks", "annotations", "graph", "actions"]),
            pinned: strings(&["pomodoro"]),
            ..Default::default()
        }
    }

    pub fn list(&self, key: &str) -> Option<&Vec<String>> {
        match key {
            "tabs" => Some(&self.tabs),
            "overview" => Some(&self.overview),
            "pinned" => Some(&self.pinned),
            "hidden" => Some(&self.hidden),
            "collapsed" => Some(&self.collapsed),
            _ => None,
        }
    }

    pub fn list_mut(&mut self, key: &str) -> Option<&mut Vec<String>> {
        match key {
            "tabs" => Some(&mut self.tabs),
            "overview" => Some(&mut self.overview),
            "pinned" => Some(&mut self.pinned),
            "hidden" => Some(&mut self.hidden),
            "collapsed" => Some(&mut self.collapsed),
            _ => None,
        }
    }

    /// Replaces every list (and the pages) that `p` sets.
    fn overlay(&mut self, p: &ContextProfile) {
        for (key, list) in [
            ("tabs", &p.tabs),
            ("overview", &p.overview),
            ("pinned", &p.pinned),
            ("hidden", &p.hidden),
            ("collapsed", &p.collapsed),
        ] {
            if let (Some(list), Some(target)) = (list, self.list_mut(key)) {
                *target = list.clone();
            }
        }
        if let Some(pages) = &p.pages {
            self.pages = pages.clone();
        }
    }
}

/// What a new profile starts with (the concept's suggestions).
pub fn suggestion(name: &str) -> Option<ContextProfile> {
    let p = |tabs: &[&str], overview: &[&str], collapsed: &[&str]| ContextProfile {
        tabs: Some(strings(tabs)),
        overview: Some(strings(overview)),
        collapsed: (!collapsed.is_empty()).then(|| strings(collapsed)),
        ..Default::default()
    };
    match name {
        "pdf" => Some(p(&["links", "actions"], &["annotations", "outline", "today", "graph", "actions"], &["graph"])),
        "markdown" => Some(p(&["graph", "actions"], &["links", "tags", "outline", "today"], &[])),
        "code" => Some(p(&["actions"], &["actions", "fileinfo"], &[])),
        "calendar" => Some(p(&[], &["week", "tasks"], &[])),
        _ => None,
    }
}

/// Profile for a tab of the editor area by its kind (`pdf`, `markdown` …).
pub fn kind_of(tab_kind: &str) -> &'static str {
    match tab_kind {
        "pdf" | "atlasnote" => "pdf",
        "markdown" => "markdown",
        "text" | "notebook" => "code",
        "calendar" => "calendar",
        _ => "default",
    }
}

/// An extension as the bar sees it (`ext:<name>` blocks).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Extension {
    pub name: String,
    /// Shown as a card unless placed somewhere.
    #[serde(default)]
    pub shown: bool,
}

/// `default` with all lists: as stored over the built-in layout, or the
/// built-in layout with the old switches as hidden blocks while there are
/// no profiles at all.
pub fn base(config: &ContextConfig) -> Lists {
    let mut out = Lists::builtin();
    if let Some(stored) = config.profiles.get("default") {
        out.overlay(stored);
        return out;
    }
    if config.profiles.is_empty() {
        for (on, id) in [(config.calendar, "today"), (config.tasks, "tasks"), (config.pomodoro, "pomodoro")] {
            if !on {
                out.hidden.push(id.to_owned());
            }
        }
    }
    out
}

/// Whether profile `name` is set (`default` always is).
pub fn is_set(config: &ContextConfig, name: &str) -> bool {
    name == "default" || config.profiles.contains_key(name)
}

/// The profile used for profile kind `kind` (see [`kind_of`]).
pub fn profile_for<'a>(config: &ContextConfig, kind: &'a str) -> &'a str {
    if config.by_kind && kind != "default" && is_set(config, kind) { kind } else { "default" }
}

/// All lists of profile `name`: as stored over `default`; a profile not set
/// yet starts from the suggestion.
pub fn lists(config: &ContextConfig, name: &str) -> Lists {
    let mut out = base(config);
    if name == "default" {
        return out;
    }
    if let Some(stored) = config.profiles.get(name) {
        out.overlay(stored);
    } else if let Some(s) = suggestion(name) {
        out.overlay(&s);
    }
    out
}

/// The page of tab `id`: own page, built-in tab or a block that may be a
/// tab. The title is empty when the interface supplies it.
pub fn page_of(l: &Lists, id: &str) -> Option<ContextPage> {
    if let Some(own) = l.pages.get(id) {
        return Some(own.clone());
    }
    if let Some(blocks) = builtin_page(id) {
        return Some(ContextPage { title: String::new(), blocks: strings(blocks) });
    }
    catalog(id).filter(|b| b.tab).map(|b| ContextPage { title: String::new(), blocks: vec![b.id.to_owned()] })
}

pub fn is_page(area: &str) -> bool {
    !KEYS.contains(&area)
}

/// The list of an area: `tabs`, `overview`, `pinned`, `hidden`,
/// `collapsed`, or a page (its blocks; a built-in page becomes stored on
/// first use).
pub fn area_list<'a>(l: &'a mut Lists, area: &str) -> &'a mut Vec<String> {
    if !is_page(area) {
        return l.list_mut(area).expect("key");
    }
    if !l.pages.contains_key(area) {
        let page = page_of(l, area).unwrap_or_else(|| ContextPage { title: area.to_owned(), blocks: Vec::new() });
        l.pages.insert(area.to_owned(), page);
    }
    &mut l.pages.get_mut(area).expect("inserted").blocks
}

/// Adds a new tab of one's own with id `page:<suffix>`; returns the id.
pub fn new_page(l: &mut Lists, title: &str, suffix: &str) -> String {
    let mut id = format!("page:{suffix}");
    let mut n = 1;
    while l.pages.contains_key(&id) || l.tabs.contains(&id) {
        n += 1;
        id = format!("page:{suffix}-{n}");
    }
    l.pages.insert(id.clone(), ContextPage { title: title.to_owned(), blocks: Vec::new() });
    l.tabs.push(id.clone());
    id
}

/// Suffix for [`new_page`]: the time in milliseconds, base 36 (as before).
pub fn page_suffix_now() -> String {
    let mut n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
    let digits = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut out = Vec::new();
    loop {
        out.push(digits[(n % 36) as usize]);
        n /= 36;
        if n == 0 {
            break;
        }
    }
    out.reverse();
    String::from_utf8(out).expect("ascii")
}

pub fn rename_page(l: &mut Lists, id: &str, title: &str) {
    area_list(l, id);
    if let Some(page) = l.pages.get_mut(id) {
        page.title = title.to_owned();
    }
}

/// Removes tab `id`; a page of one's own goes with it.
pub fn remove_tab(l: &mut Lists, id: &str) {
    l.tabs.retain(|t| t != id);
    if id.starts_with("page:") {
        l.pages.remove(id);
    }
}

pub fn toggle(list: &mut Vec<String>, id: &str, on: bool) {
    let present = list.iter().any(|x| x == id);
    if on && !present {
        list.push(id.to_owned());
    }
    if !on {
        list.retain(|x| x != id);
    }
}

/// Moves block `id` from area `from` to the end of area `to`; a block
/// moved into the overview or to the bottom is no longer hidden.
pub fn move_to(l: &mut Lists, id: &str, from: &str, to: &str) {
    area_list(l, from).retain(|x| x != id);
    let target = area_list(l, to);
    if !target.iter().any(|x| x == id) {
        target.push(id.to_owned());
    }
    if to == "overview" || to == "pinned" {
        toggle(&mut l.hidden, id, false);
    }
}

/// Removes `id` from tabs, overview and bottom (not from hidden / collapsed).
pub fn unplace(l: &mut Lists, id: &str) {
    for list in [&mut l.tabs, &mut l.overview, &mut l.pinned] {
        list.retain(|x| x != id);
    }
}

/// Moves `id` within its area by `delta`, skipping blocks not shown
/// (`visible`: the ids the bar shows there, in order). Returns whether
/// something moved.
pub fn move_by(l: &mut Lists, area: &str, id: &str, delta: i64, visible: Option<&[String]>) -> bool {
    let list = area_list(l, area);
    let shown: Vec<String> = visible.map_or_else(|| list.clone(), <[String]>::to_vec);
    let Some(at) = shown.iter().position(|x| x == id) else { return false };
    let to = at as i64 + delta;
    if to < 0 || to >= shown.len() as i64 {
        return false;
    }
    let to = to as usize;
    let other = shown[to].clone();
    let (a, b) = (list.iter().position(|x| x == id), list.iter().position(|x| *x == other));
    match (a, b) {
        (Some(a), Some(b)) => list.swap(a, b),
        _ => {
            // The visible order includes blocks the stored list lacks
            // (extension cards): store the visible order then.
            let mut order = shown;
            let moved = order.remove(at);
            order.insert(to, moved);
            let rest: Vec<String> = list.iter().filter(|x| !order.contains(x)).cloned().collect();
            *list = order.into_iter().chain(rest).collect();
        }
    }
    true
}

/// A tab of the bar as shown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LayoutPage {
    /// Empty: the interface's name for the tab (built in or single block).
    pub title: String,
    pub blocks: Vec<String>,
    /// A single big block that fills the tab.
    pub full: bool,
}

/// What the bar shows for a profile – only known blocks; extension cards
/// that are shown but not placed come at the end of the overview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Layout {
    pub name: String,
    pub tabs: Vec<String>,
    pub pages: BTreeMap<String, LayoutPage>,
    pub overview: Vec<String>,
    pub pinned: Vec<String>,
    pub collapsed: Vec<String>,
    pub hidden: Vec<String>,
}

pub fn layout(config: &ContextConfig, name: &str, extensions: &[Extension]) -> Layout {
    let l = lists(config, name);
    let known = |id: &str| {
        catalog(id).is_some() || id.strip_prefix("ext:").is_some_and(|n| extensions.iter().any(|e| e.name == n))
    };
    // `hidden` is about the overview and the bottom: a tab goes only by
    // removing it from `tabs` (graph is a tab and in the overview).
    let tabs: Vec<String> = l.tabs.iter().filter(|id| page_of(&l, id).is_some()).cloned().collect();
    let mut pages = BTreeMap::new();
    for id in &tabs {
        let p = page_of(&l, id).expect("filtered");
        let full = p.blocks.len() == 1 && FULL_BLOCKS.contains(&p.blocks[0].as_str());
        pages.insert(
            id.clone(),
            LayoutPage { title: p.title, blocks: p.blocks.into_iter().filter(|b| known(b)).collect(), full },
        );
    }
    let pinned: Vec<String> = l
        .pinned
        .iter()
        .filter(|id| known(id) && catalog(id).is_some_and(|b| b.pin) && !l.hidden.contains(id) && !tabs.contains(id))
        .cloned()
        .collect();
    // A block may be a tab and in the overview (graph and actions are), but
    // not in the overview and at the bottom.
    let mut overview: Vec<String> = Vec::new();
    for id in &l.overview {
        if known(id) && !l.hidden.contains(id) && !pinned.contains(id) && !overview.contains(id) {
            overview.push(id.clone());
        }
    }
    let mentioned = |id: &str| {
        ["tabs", "overview", "pinned", "hidden"].iter().any(|k| l.list(k).is_some_and(|v| v.iter().any(|x| x == id)))
            || tabs.iter().any(|t| page_of(&l, t).is_some_and(|p| p.blocks.iter().any(|b| b == id)))
    };
    for e in extensions {
        let id = format!("ext:{}", e.name);
        if e.shown && !mentioned(&id) {
            overview.push(id);
        }
    }
    Layout { name: name.to_owned(), tabs, pages, overview, pinned, collapsed: l.collapsed, hidden: l.hidden }
}

/// What to store for profile `name` with the lists `full`: `default`
/// completely, another profile only with what differs from `default`.
pub fn save(config: &ContextConfig, name: &str, full: &Lists) -> ContextProfile {
    if name == "default" {
        return ContextProfile {
            tabs: Some(full.tabs.clone()),
            overview: Some(full.overview.clone()),
            pinned: Some(full.pinned.clone()),
            hidden: Some(full.hidden.clone()),
            collapsed: Some(full.collapsed.clone()),
            pages: (!full.pages.is_empty()).then(|| full.pages.clone()),
        };
    }
    let d = base(config);
    let differs = |a: &Vec<String>, b: &Vec<String>| (a != b).then(|| a.clone());
    ContextProfile {
        tabs: differs(&full.tabs, &d.tabs),
        overview: differs(&full.overview, &d.overview),
        pinned: differs(&full.pinned, &d.pinned),
        hidden: differs(&full.hidden, &d.hidden),
        collapsed: differs(&full.collapsed, &d.collapsed),
        pages: (full.pages != d.pages).then(|| full.pages.clone()),
    }
}

/// Profiles to store after profile `name` was changed to `changed`: the
/// first change of all also stores `default` (taking over the old
/// switches). In the order to store.
pub fn commit(config: &ContextConfig, name: &str, changed: &Lists) -> Vec<(String, ContextProfile)> {
    let mut config = config.clone();
    let mut out = Vec::new();
    if name != "default" && !config.profiles.contains_key("default") {
        let d = save(&config, "default", &base(&config));
        config.profiles.insert("default".into(), d.clone());
        out.push(("default".to_owned(), d));
    }
    out.push((name.to_owned(), save(&config, name, changed)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(ids: &[&str]) -> Vec<String> {
        strings(ids)
    }

    fn config_with(profiles: &[(&str, ContextProfile)]) -> ContextConfig {
        let mut c = ContextConfig::default();
        for (name, p) in profiles {
            c.profiles.insert((*name).to_owned(), p.clone());
        }
        c
    }

    #[test]
    fn builtin_layout_without_profiles() {
        let l = layout(&ContextConfig::default(), "default", &[]);
        assert_eq!(l.tabs, s(&["links", "graph", "actions"]));
        assert_eq!(l.overview, s(&["today", "tasks", "annotations", "graph", "actions"]));
        assert_eq!(l.pinned, s(&["pomodoro"]));
        assert!(l.pages["links"].full && l.pages["graph"].full && !l.pages["actions"].full);
        assert_eq!(l.pages["actions"].blocks, s(&["builtin-actions", "user-actions"]));
        assert_eq!(l.pages["links"].title, "", "built-in titles come from the interface");
    }

    #[test]
    fn old_switches_hide_blocks_only_without_profiles() {
        let mut c = ContextConfig { calendar: false, pomodoro: false, ..Default::default() };
        let l = layout(&c, "default", &[]);
        assert!(!l.overview.contains(&"today".into()));
        assert!(l.pinned.is_empty());
        c.profiles.insert("pdf".into(), ContextProfile::default());
        assert!(layout(&c, "default", &[]).overview.contains(&"today".into()));
    }

    #[test]
    fn profile_selection() {
        let c = config_with(&[("pdf", ContextProfile::default())]);
        assert_eq!(profile_for(&c, kind_of("pdf")), "pdf");
        assert_eq!(profile_for(&c, kind_of("atlasnote")), "pdf");
        assert_eq!(profile_for(&c, kind_of("markdown")), "default", "not set yet");
        let off = ContextConfig { by_kind: false, ..c };
        assert_eq!(profile_for(&off, "pdf"), "default");
        assert_eq!(kind_of("notebook"), "code");
    }

    #[test]
    fn unset_profile_starts_from_suggestion_over_default() {
        let c = config_with(&[("default", ContextProfile { pinned: Some(s(&["tasks"])), ..Default::default() })]);
        let l = lists(&c, "pdf");
        assert_eq!(l.tabs, s(&["links", "actions"]));
        assert_eq!(l.collapsed, s(&["graph"]));
        assert_eq!(l.pinned, s(&["tasks"]), "taken from default");
    }

    #[test]
    fn layout_rules() {
        let c = config_with(&[(
            "default",
            ContextProfile {
                tabs: Some(s(&["graph", "unknown", "tags", "outline"])),
                overview: Some(s(&["today", "today", "nope", "tasks", "ext:cards", "pomodoro"])),
                pinned: Some(s(&["pomodoro", "graph", "outline"])),
                hidden: Some(s(&["tasks"])),
                ..Default::default()
            },
        )]);
        let exts = [
            Extension { name: "cards".into(), shown: true },
            Extension { name: "free".into(), shown: true },
            Extension { name: "off".into(), shown: false },
        ];
        let l = layout(&c, "default", &exts);
        // `unknown` and `tags` (no tab) are not tabs; `outline` is.
        assert_eq!(l.tabs, s(&["graph", "outline"]));
        // Only pinnable blocks at the bottom, not shown in the overview again.
        assert_eq!(l.pinned, s(&["pomodoro"]));
        assert_eq!(l.overview, s(&["today", "ext:cards", "ext:free"]));
    }

    #[test]
    fn pages_of_ones_own() {
        let mut l = Lists::builtin();
        let id = new_page(&mut l, "Lesen", "abc");
        assert_eq!(id, "page:abc");
        assert_eq!(new_page(&mut l, "Noch einer", "abc"), "page:abc-2");
        move_to(&mut l, "annotations", "overview", &id);
        assert_eq!(l.pages[&id].blocks, s(&["annotations"]));
        assert!(!l.overview.contains(&"annotations".into()));
        rename_page(&mut l, &id, "Lektüre");
        assert_eq!(page_of(&l, &id).unwrap().title, "Lektüre");
        remove_tab(&mut l, &id);
        assert!(!l.pages.contains_key(&id) && !l.tabs.contains(&id));
    }

    #[test]
    fn builtin_page_is_stored_on_first_change() {
        let mut l = Lists::builtin();
        area_list(&mut l, "actions").retain(|b| b != "user-actions");
        assert_eq!(l.pages["actions"].blocks, s(&["builtin-actions"]));
        assert_eq!(l.pages["actions"].title, "");
        // A built-in tab keeps working after removal of its own page.
        remove_tab(&mut l, "graph");
        assert!(!l.tabs.contains(&"graph".into()));
    }

    #[test]
    fn move_to_overview_unhides() {
        let mut l = Lists::builtin();
        l.hidden.push("week".into());
        move_to(&mut l, "week", "hidden", "overview");
        assert!(!l.hidden.contains(&"week".into()));
        assert_eq!(l.overview.last().unwrap(), "week");
        unplace(&mut l, "graph");
        assert!(!l.tabs.contains(&"graph".into()) && !l.overview.contains(&"graph".into()));
    }

    #[test]
    fn move_by_swaps_and_respects_visible_order() {
        let mut l = Lists::builtin();
        assert!(move_by(&mut l, "tabs", "graph", -1, None));
        assert_eq!(l.tabs, s(&["graph", "links", "actions"]));
        assert!(!move_by(&mut l, "tabs", "graph", -1, None), "already first");
        // Visible order with an extension card the list lacks.
        let visible = s(&["today", "tasks", "annotations", "graph", "actions", "ext:cards"]);
        assert!(move_by(&mut l, "overview", "ext:cards", -2, Some(&visible)));
        assert_eq!(l.overview, s(&["today", "tasks", "annotations", "ext:cards", "graph", "actions"]));
        // Skips hidden blocks: tasks is not shown, so today moves past it.
        let visible = s(&["today", "annotations"]);
        let mut l = Lists::builtin();
        assert!(move_by(&mut l, "overview", "today", 1, Some(&visible)));
        assert_eq!(l.overview, s(&["annotations", "tasks", "today", "graph", "actions"]));
    }

    #[test]
    fn save_stores_only_differences_and_commit_adopts_default() {
        let c = ContextConfig { tasks: false, ..Default::default() };
        let mut l = lists(&c, "markdown");
        l.overview.push("week".into());
        let writes = commit(&c, "markdown", &l);
        assert_eq!(writes.len(), 2);
        let (name, d) = &writes[0];
        assert_eq!(name, "default");
        assert_eq!(d.hidden, Some(s(&["tasks"])), "old switch taken over");
        let (name, md) = &writes[1];
        assert_eq!(name, "markdown");
        assert_eq!(md.overview, Some(s(&["links", "tags", "outline", "today", "week"])));
        assert_eq!(md.tabs, Some(s(&["graph", "actions"])));
        assert_eq!(md.pinned, None, "same as default");
        assert_eq!(md.pages, None);
        // Once default is stored, only the profile itself is written.
        let mut c2 = c.clone();
        c2.profiles.insert("default".into(), d.clone());
        assert_eq!(commit(&c2, "markdown", &l).len(), 1);
    }

    #[test]
    fn stored_profile_round_trips_through_lists() {
        let mut c = ContextConfig::default();
        let mut l = lists(&c, "default");
        let id = new_page(&mut l, "Plan", "x");
        move_to(&mut l, "week", "overview", &id);
        for (name, p) in commit(&c, "default", &l) {
            c.profiles.insert(name, p);
        }
        assert_eq!(lists(&c, "default"), l);
        let lay = layout(&c, "default", &[]);
        assert_eq!(lay.pages[&id].title, "Plan");
        assert_eq!(lay.pages[&id].blocks, s(&["week"]));
    }
}
