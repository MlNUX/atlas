// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Settings shown in the UI (places, theme, PDF, sorting, context panel).

use super::*;

impl qobject::AppController {
    pub fn refresh_places(mut self: Pin<&mut Self>) {
        let (places, fixed) = atlas_core::places::sidebar_places(atlas_core::places::fixed(), self.file_manager_places);
        let (mut names, mut paths, mut icons) =
            (QStringList::default(), QStringList::default(), QStringList::default());
        for place in &places {
            names.append(qs(&place.name));
            paths.append(qs(place.path.to_string_lossy()));
            icons.append(qs(&place.icon));
        }
        if paths == self.place_paths
            && names == self.place_names
            && icons == self.place_icons
            && fixed as i32 == self.fixed_place_count
        {
            return;
        }
        let mut r = self.as_mut().rust_mut();
        r.place_names = names;
        r.place_paths = paths;
        r.place_icons = icons;
        r.fixed_place_count = fixed as i32;
        self.places_changed();
    }

    pub fn set_file_manager_places(mut self: Pin<&mut Self>, on: bool) {
        if self.file_manager_places == on {
            return;
        }
        let mut config = Config::load().places;
        config.file_manager = on;
        if let Err(err) = Config::save_places(&config) {
            log::warn!("cannot save [places]: {err:#}");
        }
        self.as_mut().rust_mut().file_manager_places = on;
        self.as_mut().places_changed();
        self.refresh_places();
    }

    pub fn set_pdf_dark(mut self: Pin<&mut Self>, on: bool) {
        if self.pdf_dark == on {
            return;
        }
        let mut config = Config::load().pdf;
        config.dark = on;
        if let Err(err) = Config::save_pdf(&config) {
            log::warn!("cannot save [pdf]: {err:#}");
        }
        self.as_mut().rust_mut().pdf_dark = on;
        self.as_mut().pdf_dark_changed();
    }

    pub fn set_theme(mut self: Pin<&mut Self>, theme: QString) {
        let mut config = Config::load().appearance;
        config.theme = theme.to_string();
        let theme = qs(config.theme());
        if self.theme == theme {
            return;
        }
        config.theme = theme.to_string();
        if let Err(err) = Config::save_appearance(&config) {
            log::warn!("cannot save [appearance]: {err:#}");
        }
        self.as_mut().rust_mut().theme = theme;
        self.as_mut().theme_changed();
    }

    pub fn set_terminal_light(&self, on: bool) {
        atlas_terminal::colors::set_light(on);
    }

    pub fn set_natural_sort(mut self: Pin<&mut Self>, on: bool) {
        if self.natural_sort == on {
            return;
        }
        let mut config = Config::load().files;
        config.natural_sort = on;
        if let Err(err) = Config::save_files(&config) {
            log::warn!("cannot save [files]: {err:#}");
        }
        atlas_core::listing::set_natural_sort(on);
        self.as_mut().rust_mut().natural_sort = on;
        // Views sort again when they see the change.
        self.as_mut().natural_sort_changed();
    }

    pub fn set_touchpad_fling(mut self: Pin<&mut Self>, on: bool) {
        if self.touchpad_fling == on {
            return;
        }
        let mut config = Config::load().files;
        config.touchpad_fling = on;
        if let Err(err) = Config::save_files(&config) {
            log::warn!("cannot save [files]: {err:#}");
        }
        self.as_mut().rust_mut().touchpad_fling = on;
        self.as_mut().touchpad_fling_changed();
    }

    pub fn set_spellcheck(mut self: Pin<&mut Self>, on: bool) {
        if self.spellcheck == on {
            return;
        }
        let mut config = Config::load().editor;
        config.spellcheck = if on { None } else { Some(false) };
        if let Err(err) = Config::save_editor(&config) {
            log::warn!("cannot save [editor]: {err:#}");
        }
        self.as_mut().rust_mut().spellcheck = on;
        self.as_mut().spellcheck_changed();
    }

    pub fn pdf_users(&self, path: &QString) -> i32 {
        atlas_core::pdf::PdfService::get().users_of(std::path::Path::new(&path.to_string())) as i32
    }

    pub fn state_dir(&self) -> QString {
        qs(atlas_core::paths::state_root().to_string_lossy())
    }

    pub fn config_dir(&self) -> QString {
        qs(atlas_core::config::config_dir().to_string_lossy())
    }

    pub fn remember_folder_sort(mut self: Pin<&mut Self>, key: &QString, ascending: bool) {
        let mut config = Config::load().files;
        let key = key.to_string();
        if config.sort_key == key && config.sort_ascending == ascending {
            return;
        }
        config.sort_key = key;
        config.sort_ascending = ascending;
        if let Err(err) = Config::save_files(&config) {
            log::warn!("cannot save [files]: {err:#}");
        }
        self.as_mut().rust_mut().folder_sort = folder_sort_json(&config);
        self.as_mut().folder_sort_changed();
    }

    pub fn set_context_module(mut self: Pin<&mut Self>, key: &QString, value: i32) {
        let mut config = Config::load().context;
        let minutes = value.clamp(1, 180) as u32;
        match key.to_string().as_str() {
            "calendar" => config.calendar = value != 0,
            "tasks" => config.tasks = value != 0,
            "pomodoro" => config.pomodoro = value != 0,
            "pomodoro_work" => config.pomodoro_work = minutes,
            "pomodoro_break" => config.pomodoro_break = minutes,
            "pomodoro_long_break" => config.pomodoro_long_break = minutes,
            // `ext:<name>`: the card of an extension.
            other if other.starts_with("ext:") && other.len() > 4 => {
                config.extensions.insert(other[4..].to_owned(), value != 0);
            }
            other => {
                log::warn!("unknown context module {other}");
                return;
            }
        }
        if let Err(err) = Config::save_context(&config) {
            log::warn!("cannot save [context]: {err:#}");
        }
        self.as_mut().rust_mut().context_modules = context_json(&config);
        self.as_mut().context_modules_changed();
    }

    pub fn set_context_profile(mut self: Pin<&mut Self>, name: &QString, json: &QString) {
        let mut config = Config::load().context;
        let name = name.to_string();
        let json = json.to_string();
        if json.trim().is_empty() || json.trim() == "null" {
            config.profiles.remove(&name);
        } else {
            match serde_json::from_str::<atlas_core::config::ContextProfile>(&json) {
                Ok(profile) => {
                    config.profiles.insert(name, profile);
                }
                Err(err) => {
                    log::warn!("invalid context profile {name}: {err}");
                    return;
                }
            }
        }
        self.as_mut().store_context(config);
    }

    pub fn set_context_option(mut self: Pin<&mut Self>, key: &QString, value: &QString) {
        let mut config = Config::load().context;
        let value = value.to_string();
        match key.to_string().as_str() {
            "empty" if ["show", "collapse", "hide"].contains(&value.as_str()) => config.empty = value,
            "tab_labels" if ["auto", "text", "icons"].contains(&value.as_str()) => config.tab_labels = value,
            "by_kind" => config.by_kind = value == "true",
            other => {
                log::warn!("unknown context option {other}={value}");
                return;
            }
        }
        self.as_mut().store_context(config);
    }

    fn store_context(mut self: Pin<&mut Self>, config: atlas_core::config::ContextConfig) {
        if let Err(err) = Config::save_context(&config) {
            log::warn!("cannot save [context]: {err:#}");
        }
        self.as_mut().rust_mut().context_modules = context_json(&config);
        self.as_mut().context_modules_changed();
    }

    pub fn tags_of_note(&self, path: &QString) -> QStringList {
        let path = path_of(path);
        let mut out = QStringList::default();
        for tag in self.with_notes(|n| n.note(&path).map(|note| note.tags.clone())).flatten().unwrap_or_default() {
            out.append(qs(tag));
        }
        out
    }

    /// Dolphin or a GTK file manager changed its places: read them again.
    pub(super) fn watch_places(mut self: Pin<&mut Self>) {
        let sources = atlas_core::places::sources();
        let dirs: Vec<PathBuf> =
            sources.iter().filter_map(|f| f.parent().map(Path::to_path_buf)).filter(|d| d.is_dir()).collect();
        let thread = self.qt_thread();
        let watched = sources.clone();
        match atlas_core::watcher::DirWatcher::new(move |changed| {
            if changed.iter().any(|p| watched.contains(p)) {
                let _ = thread.queue(|this| {
                    if this.file_manager_places {
                        this.refresh_places();
                    }
                });
            }
        }) {
            Ok(watcher) => {
                watcher.sync(&dirs);
                self.as_mut().rust_mut().places_watcher = Some(watcher);
            }
            Err(err) => log::warn!("places watcher unavailable: {err:#}"),
        }
    }
}
