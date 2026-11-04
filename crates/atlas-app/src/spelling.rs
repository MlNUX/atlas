// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Spellchecking for QML (singleton `Spelling`): German and English
//! dictionaries of the system plus a personal one in the config folder.

use std::sync::{Mutex, OnceLock};

use atlas_core::spell::{Checker, dictionary_dirs};
use cxx_qt_lib::{QString, QStringList};

use crate::util::qs;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        type Spelling = super::SpellingRust;

        /// Misspelled words of the Markdown `text`: JSON `[[start, length]]`
        /// in characters (UTF-16); "null" if no dictionary was found.
        #[qinvokable]
        fn check(self: &Spelling, text: &QString) -> QString;

        /// Up to five suggestions for `word`.
        #[qinvokable]
        fn suggest(self: &Spelling, word: &QString) -> QStringList;

        /// Adds `word` to the personal dictionary (config folder).
        #[qinvokable]
        fn add(self: &Spelling, word: &QString);

        /// The dictionaries in use ("de_DE, en_US"; "" if none was found).
        #[qinvokable]
        fn languages(self: &Spelling) -> QString;
    }
}

#[derive(Default)]
pub struct SpellingRust;

/// Runs `f` with the spellchecker, loaded on first use (about 0.1 s);
/// `None` if no dictionary was found.
fn with_speller<T>(f: impl FnOnce(&mut Checker) -> T) -> Option<T> {
    static SPELLER: OnceLock<Mutex<Option<Checker>>> = OnceLock::new();
    let speller = SPELLER.get_or_init(|| {
        let personal = atlas_core::config::config_dir().join("dictionary.txt");
        Mutex::new(
            Checker::load(&dictionary_dirs(), Some(personal)).inspect_err(|e| log::info!("spellcheck: {e:#}")).ok(),
        )
    });
    let mut guard = speller.lock().unwrap_or_else(|e| e.into_inner());
    guard.as_mut().map(f)
}

impl qobject::Spelling {
    pub fn check(&self, text: &QString) -> QString {
        let text = text.to_string();
        with_speller(|checker| {
            let bad = checker.misspelled(&text);
            let list: Vec<[usize; 2]> =
                atlas_core::spell::utf16_ranges(&text, &bad).into_iter().map(|(s, l)| [s, l]).collect();
            serde_json::to_string(&list).unwrap_or_else(|_| "[]".into())
        })
        .map_or_else(|| qs("null"), qs)
    }

    pub fn suggest(&self, word: &QString) -> QStringList {
        let mut out = QStringList::default();
        for s in with_speller(|c| c.suggest(&word.to_string(), 5)).unwrap_or_default() {
            out.append(qs(s));
        }
        out
    }

    pub fn add(&self, word: &QString) {
        if let Some(Err(err)) = with_speller(|c| c.add(&word.to_string())) {
            log::warn!("spellcheck: {err:#}");
        }
    }

    pub fn languages(&self) -> QString {
        qs(with_speller(|c| c.languages().join(", ")).unwrap_or_default())
    }
}
