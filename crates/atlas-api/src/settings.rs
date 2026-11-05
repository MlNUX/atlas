// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Settings (`config.toml`, shared by all windows), the context bar
//! profiles (logic from P1.3, `atlas_core::context_profiles`) and spell
//! checking (`atlas-spell`). Template: `app/settings.rs`, `spelling.rs`,
//! `SettingsDialog.qml`, `ContextProfiles.qml`.

use std::sync::{Arc, Mutex, OnceLock};

use atlas_core::config::Config;
use atlas_core::context_profiles as cp;

use crate::{AtlasError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum Theme {
    /// Follow the system (the only sensible choice on macOS, E8).
    System,
    Light,
    Dark,
}

/// The general settings. Sections of their own (calendar account,
/// ILIAS, extensions, context bar, external editor) live with their services.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct AppSettings {
    pub theme: Theme,
    pub pdf_dark: bool,
    pub natural_sort: bool,
    pub spellcheck: bool,
    pub file_manager_places: bool,
    /// Shell for terminals, empty: login shell.
    pub shell: String,
}

/// Sort order of the folder view (one for all folders).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct FolderSort {
    pub key: crate::files::SortKey,
    pub ascending: bool,
}

#[cfg_attr(feature = "ffi", derive(uniffi::Object))]
pub struct Settings {
    sink: crate::events::Sink,
}

impl Settings {
    pub(crate) fn new(sink: crate::events::Sink) -> Self {
        Self { sink }
    }
}

fn theme_of(name: &str) -> Theme {
    match name {
        "light" => Theme::Light,
        "system" => Theme::System,
        _ => Theme::Dark,
    }
}

fn sort_name(key: crate::files::SortKey) -> &'static str {
    use crate::files::SortKey as K;
    match key {
        K::Name => "name",
        K::Size => "size",
        K::Modified => "modified",
        K::Kind => "kind",
    }
}

#[cfg_attr(feature = "ffi", uniffi::export)]
impl Settings {
    pub fn get(&self) -> AppSettings {
        let c = Config::load();
        AppSettings {
            theme: theme_of(c.appearance.theme()),
            pdf_dark: c.pdf.dark,
            natural_sort: c.files.natural_sort,
            spellcheck: c.editor.spellcheck.unwrap_or(true),
            file_manager_places: c.places.file_manager,
            shell: c.terminal.shell,
        }
    }

    /// Saves the sections that changed (keeping comments and unknown keys);
    /// other windows get `Event::SettingsChanged`.
    pub fn save(&self, settings: AppSettings) -> Result<()> {
        let c = Config::load();
        let old = self.get();
        if settings.theme != old.theme {
            let mut a = c.appearance.clone();
            a.theme = match settings.theme {
                Theme::System => "system",
                Theme::Light => "light",
                Theme::Dark => "dark",
            }
            .into();
            Config::save_appearance(&a)?;
        }
        if settings.pdf_dark != old.pdf_dark {
            let mut p = c.pdf.clone();
            p.dark = settings.pdf_dark;
            Config::save_pdf(&p)?;
        }
        if settings.natural_sort != old.natural_sort {
            let mut f = c.files.clone();
            f.natural_sort = settings.natural_sort;
            Config::save_files(&f)?;
            atlas_core::listing::set_natural_sort(settings.natural_sort);
        }
        if settings.spellcheck != old.spellcheck {
            let mut e = c.editor.clone();
            e.spellcheck = if settings.spellcheck { None } else { Some(false) };
            Config::save_editor(&e)?;
        }
        if settings.file_manager_places != old.file_manager_places {
            let mut p = c.places.clone();
            p.file_manager = settings.file_manager_places;
            Config::save_places(&p)?;
            self.sink.on_event(crate::events::Event::PlacesChanged);
        }
        if settings.shell != old.shell {
            Config::save_terminal(&atlas_core::config::TerminalConfig { shell: settings.shell })?;
        }
        self.sink.on_event(crate::events::Event::SettingsChanged);
        Ok(())
    }

    /// Path of `config.toml` (for „In Editor öffnen“).
    pub fn file(&self) -> String {
        crate::util::string(&atlas_core::config::config_file())
    }

    pub fn folder_sort(&self) -> FolderSort {
        let f = Config::load().files;
        let key = match atlas_core::listing::SortKey::from_name(&f.sort_key) {
            atlas_core::listing::SortKey::Name => crate::files::SortKey::Name,
            atlas_core::listing::SortKey::Size => crate::files::SortKey::Size,
            atlas_core::listing::SortKey::Modified => crate::files::SortKey::Modified,
            atlas_core::listing::SortKey::Kind => crate::files::SortKey::Kind,
        };
        FolderSort { key, ascending: f.sort_ascending }
    }

    pub fn set_folder_sort(&self, sort: FolderSort) -> Result<()> {
        let mut f = Config::load().files;
        f.sort_key = sort_name(sort.key).into();
        f.sort_ascending = sort.ascending;
        Ok(Config::save_files(&f)?)
    }

    /// The spell checker (dictionaries found once per process).
    pub fn spelling(&self) -> Arc<Spelling> {
        Arc::new(Spelling {})
    }
}

// --- context bar ---

/// A building block of the context bar (display names belong to the UI).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ContextBlock {
    pub id: String,
    pub icon: String,
    /// Can be a tab of its own.
    pub tab: bool,
    /// Can be pinned to the bottom edge.
    pub pin: bool,
    /// `file`, `plan` or `ext`.
    pub group: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ContextPage {
    pub id: String,
    /// Empty when the UI supplies it (built-in tabs, single blocks).
    pub title: String,
    pub blocks: Vec<String>,
    /// A block filling the whole page (`links`, `graph`).
    pub full: bool,
}

/// How a profile lays out the bar for one file.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ContextLayout {
    pub profile: String,
    /// Tab ids after „Übersicht“, in order.
    pub tabs: Vec<String>,
    /// The pages of `tabs`, in the same order.
    pub pages: Vec<ContextPage>,
    pub overview: Vec<String>,
    pub pinned: Vec<String>,
    pub collapsed: Vec<String>,
    pub hidden: Vec<String>,
}

/// Areas a block can be moved between in the edit mode.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ContextArea {
    Tabs,
    Overview,
    Pinned,
    Hidden,
    /// A page (`page:…` or a built-in tab id).
    Page {
        id: String,
    },
}

impl ContextArea {
    /// Area name as `context_profiles` uses it (`tabs`, …, `page:<id>`).
    fn key(&self) -> String {
        match self {
            ContextArea::Tabs => "tabs".into(),
            ContextArea::Overview => "overview".into(),
            ContextArea::Pinned => "pinned".into(),
            ContextArea::Hidden => "hidden".into(),
            ContextArea::Page { id } => format!("page:{id}"),
        }
    }
}

/// Which profiles an edit changes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ContextTarget {
    /// Only this profile (the first change of a profile also saves
    /// „Allgemein“, as on Linux: `context_profiles::commit`).
    Profile { name: String },
    /// „Gilt für alle Dateitypen“: `default` and every profile of its own,
    /// so no profile hides the change.
    All,
}

/// Opaque copy of all profiles, for undo after hiding or removing.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ContextSnapshot {
    pub data: String,
}

/// Options of the bar besides the profiles.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ContextOptions {
    /// Empty blocks: `show`, `collapse` (only their header) or `hide`.
    pub empty: String,
    /// Tab labels: `auto`, `text` or `icons`.
    pub tab_labels: String,
    /// Profiles by kind of file; off: `default` everywhere.
    pub by_kind: bool,
    pub pomodoro_work: u32,
    pub pomodoro_break: u32,
    pub pomodoro_long_break: u32,
}

/// The context bar (P1.3, `atlas_core::context_profiles`), stored in
/// `[context]` of `config.toml`. Layouts include the cards of active
/// extensions (`ext:…`), which this object takes from the extension
/// settings. Moving by places follows the *visible* order.
#[cfg_attr(feature = "ffi", derive(uniffi::Object))]
pub struct Context {
    shared: std::sync::Arc<crate::inner::Shared>,
}

impl Context {
    pub(crate) fn new(shared: std::sync::Arc<crate::inner::Shared>) -> Self {
        Self { shared }
    }

    /// Extension cards as the bar sees them: the active extensions with
    /// views or palette commands (P2.7).
    fn extensions(&self, config: &atlas_core::config::ContextConfig) -> Vec<cp::Extension> {
        crate::extensions::bar_extensions(&self.shared, config)
    }

    /// Applies `change` to the lists of the target profiles and stores the result.
    fn edit(&self, target: &ContextTarget, change: impl Fn(&mut cp::Lists)) -> Result<()> {
        let mut config = Config::load().context;
        let names: Vec<String> = match target {
            ContextTarget::Profile { name } => vec![name.clone()],
            ContextTarget::All => std::iter::once("default".to_owned())
                .chain(cp::NAMES.iter().filter(|n| **n != "default" && cp::is_set(&config, n)).map(|n| (*n).to_owned()))
                .collect(),
        };
        for name in names {
            let mut lists = cp::lists(&config, &name);
            change(&mut lists);
            for (n, profile) in cp::commit(&config, &name, &lists) {
                config.profiles.insert(n, profile);
            }
        }
        Ok(Config::save_context(&config)?)
    }
}

fn layout_of(l: cp::Layout) -> ContextLayout {
    let pages = l
        .tabs
        .iter()
        .filter_map(|id| l.pages.get(id).map(|p| (id, p)))
        .map(|(id, p)| ContextPage { id: id.clone(), title: p.title.clone(), blocks: p.blocks.clone(), full: p.full })
        .collect();
    ContextLayout {
        profile: l.name,
        tabs: l.tabs,
        pages,
        overview: l.overview,
        pinned: l.pinned,
        collapsed: l.collapsed,
        hidden: l.hidden,
    }
}

fn check_profile(name: &str) -> Result<()> {
    if cp::NAMES.contains(&name) {
        Ok(())
    } else {
        Err(AtlasError::Invalid { message: format!("unbekanntes Profil „{name}“") })
    }
}

#[cfg_attr(feature = "ffi", uniffi::export)]
impl Context {
    pub fn catalog(&self) -> Vec<ContextBlock> {
        cp::CATALOG
            .iter()
            .map(|b| ContextBlock {
                id: b.id.into(),
                icon: b.icon.into(),
                tab: b.tab,
                pin: b.pin,
                group: b.group.into(),
            })
            .collect()
    }

    /// Profile names (`default`, `pdf`, `markdown`, `code`, `calendar`).
    pub fn profiles(&self) -> Vec<String> {
        cp::NAMES.iter().map(|n| (*n).to_owned()).collect()
    }

    /// The profile has settings of its own (else it shows „wie Allgemein“).
    pub fn is_set(&self, profile: String) -> bool {
        cp::is_set(&Config::load().context, &profile)
    }

    /// The profile that applies to a tab of this viewer kind (`pdf`, `markdown`, …).
    pub fn profile_for(&self, kind: String) -> String {
        cp::profile_for(&Config::load().context, cp::kind_of(&kind)).to_owned()
    }

    pub fn layout(&self, profile: String) -> ContextLayout {
        let config = Config::load().context;
        layout_of(cp::layout(&config, &profile, &self.extensions(&config)))
    }

    pub fn options(&self) -> ContextOptions {
        let c = Config::load().context;
        ContextOptions {
            empty: c.empty,
            tab_labels: c.tab_labels,
            by_kind: c.by_kind,
            pomodoro_work: c.pomodoro_work,
            pomodoro_break: c.pomodoro_break,
            pomodoro_long_break: c.pomodoro_long_break,
        }
    }

    pub fn set_options(&self, options: ContextOptions) -> Result<()> {
        let ok = |v: &str, allowed: &[&str]| allowed.contains(&v);
        if !ok(&options.empty, &["show", "collapse", "hide"]) || !ok(&options.tab_labels, &["auto", "text", "icons"]) {
            return Err(AtlasError::Invalid { message: "ungültige Option der Kontextleiste".into() });
        }
        let mut c = Config::load().context;
        c.empty = options.empty;
        c.tab_labels = options.tab_labels;
        c.by_kind = options.by_kind;
        c.pomodoro_work = options.pomodoro_work.clamp(1, 180);
        c.pomodoro_break = options.pomodoro_break.clamp(1, 180);
        c.pomodoro_long_break = options.pomodoro_long_break.clamp(1, 180);
        Ok(Config::save_context(&c)?)
    }

    /// Edit mode: moves `block` from one area to another (at the end).
    pub fn move_block(&self, target: ContextTarget, block: String, from: ContextArea, to: ContextArea) -> Result<()> {
        self.edit(&target, |l| cp::move_to(l, &block, &from.key(), &to.key()))
    }

    /// Moves `block` within its area by `delta` visible places.
    pub fn move_by(&self, target: ContextTarget, area: ContextArea, block: String, delta: i64) -> Result<()> {
        let config = Config::load().context;
        let extensions = self.extensions(&config);
        self.edit(&target, |l| {
            // What the bar shows there, in order (cards missing from the stored list included).
            let name = match &target {
                ContextTarget::Profile { name } => name.as_str(),
                ContextTarget::All => "default",
            };
            let shown = cp::layout(&config, name, &extensions);
            let visible: Option<Vec<String>> = match &area {
                ContextArea::Overview => Some(shown.overview.clone()),
                ContextArea::Pinned => Some(shown.pinned.clone()),
                ContextArea::Tabs => Some(shown.tabs.clone()),
                ContextArea::Page { id } => shown.pages.get(id).map(|p| p.blocks.clone()),
                ContextArea::Hidden => None,
            };
            cp::move_by(l, &area.key(), &block, delta, visible.as_deref());
        })
    }

    /// Hides or shows a block (it keeps its place).
    pub fn set_hidden(&self, target: ContextTarget, block: String, hidden: bool) -> Result<()> {
        self.edit(&target, |l| cp::toggle(&mut l.hidden, &block, hidden))
    }

    /// Takes a block out of every area („ganz herausnehmen“).
    pub fn unplace(&self, target: ContextTarget, block: String) -> Result<()> {
        self.edit(&target, |l| cp::unplace(l, &block))
    }

    pub fn set_collapsed(&self, target: ContextTarget, block: String, collapsed: bool) -> Result<()> {
        self.edit(&target, |l| cp::toggle(&mut l.collapsed, &block, collapsed))
    }

    /// New page of one's own with `title` (as a new tab); returns its id.
    pub fn new_page(&self, target: ContextTarget, title: String) -> Result<String> {
        let suffix = cp::page_suffix_now();
        let id = Mutex::new(String::new());
        self.edit(&target, |l| {
            let new = cp::new_page(l, &title, &suffix);
            *id.lock().unwrap() = new;
        })?;
        Ok(id.into_inner().unwrap())
    }

    pub fn rename_page(&self, target: ContextTarget, id: String, title: String) -> Result<()> {
        self.edit(&target, |l| cp::rename_page(l, &id, &title))
    }

    /// Removes a tab; a page of one's own goes with it.
    pub fn remove_tab(&self, target: ContextTarget, id: String) -> Result<()> {
        self.edit(&target, |l| cp::remove_tab(l, &id))
    }

    /// Back to the suggestion for this profile (`default`: the built-in layout).
    pub fn reset(&self, profile: String) -> Result<()> {
        check_profile(&profile)?;
        let mut config = Config::load().context;
        config.profiles.remove(&profile);
        if let Some(s) = cp::suggestion(&profile)
            && profile != "default"
        {
            config.profiles.insert(profile, s);
        }
        Ok(Config::save_context(&config)?)
    }

    pub fn snapshot(&self) -> ContextSnapshot {
        let profiles = Config::load().context.profiles;
        ContextSnapshot { data: serde_json::to_string(&profiles).unwrap_or_default() }
    }

    pub fn restore(&self, snapshot: ContextSnapshot) -> Result<()> {
        let profiles = serde_json::from_str(&snapshot.data)
            .map_err(|e| AtlasError::Invalid { message: format!("ungültiger Schnappschuss: {e}") })?;
        let mut config = Config::load().context;
        config.profiles = profiles;
        Ok(Config::save_context(&config)?)
    }
}

// --- spelling ---

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Misspelling {
    /// UTF-16 positions in the checked text.
    pub start: u32,
    pub end: u32,
    pub word: String,
}

#[cfg_attr(feature = "ffi", derive(uniffi::Object))]
pub struct Spelling {}

fn with_speller<T>(f: impl FnOnce(&mut atlas_spell::Checker) -> T) -> Option<T> {
    static SPELLER: OnceLock<Mutex<Option<atlas_spell::Checker>>> = OnceLock::new();
    let speller = SPELLER.get_or_init(|| {
        let personal = atlas_core::config::config_dir().join("dictionary.txt");
        Mutex::new(
            atlas_spell::Checker::load(&atlas_spell::dictionary_dirs(), Some(personal))
                .inspect_err(|e| log::info!("spellcheck: {e:#}"))
                .ok(),
        )
    });
    speller.lock().unwrap_or_else(|e| e.into_inner()).as_mut().map(f)
}

#[cfg_attr(feature = "ffi", uniffi::export)]
impl Spelling {
    /// Dictionaries were found (else nothing is checked).
    pub fn available(&self) -> bool {
        with_speller(|_| ()).is_some()
    }

    /// Misspelled words of `text` (Markdown aware: code, links and math are skipped).
    pub fn check(&self, text: String) -> Vec<Misspelling> {
        with_speller(|c| {
            let bad = c.misspelled(&text);
            let words: Vec<String> = bad.iter().map(|r| text[r.clone()].to_owned()).collect();
            atlas_spell::utf16_ranges(&text, &bad)
                .into_iter()
                .zip(words)
                .map(|((start, len), word)| Misspelling { start: start as u32, end: (start + len) as u32, word })
                .collect()
        })
        .unwrap_or_default()
    }

    pub fn suggest(&self, word: String) -> Vec<String> {
        with_speller(|c| c.suggest(&word, 5)).unwrap_or_default()
    }

    /// Adds to the personal dictionary (`dictionary.txt` in the settings folder).
    pub fn add(&self, word: String) -> Result<()> {
        with_speller(|c| c.add(&word)).unwrap_or_else(|| Err(anyhow::anyhow!("keine Wörterbücher")))?;
        Ok(())
    }

    /// Languages of the loaded dictionaries (`de_DE`, `en_US`).
    pub fn languages(&self) -> Vec<String> {
        with_speller(|c| c.languages()).unwrap_or_default()
    }
}
