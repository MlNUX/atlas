// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Spellcheck for notes: German and English at once (a word is right if
//! one of the dictionaries knows it), with Hunspell dictionaries read by
//! `spellbook` and a personal word list (`dictionary.txt` in the config
//! folder). Only prose is checked: not code, links, URLs, math, tags or
//! the front matter.

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

/// Dictionaries looked for, by file name (`de_DE.aff`/`.dic` …).
pub const LANGUAGES: [&str; 2] = ["de_DE", "en_US"];

/// Folders that may hold Hunspell dictionaries: `ATLAS_HUNSPELL_DIRS` (at
/// run time, then as built into the Nix package), then the usual places.
pub fn dictionary_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    for list in [std::env::var("ATLAS_HUNSPELL_DIRS").ok(), option_env!("ATLAS_HUNSPELL_DIRS").map(str::to_owned)]
        .into_iter()
        .flatten()
    {
        dirs.extend(std::env::split_paths(&list));
    }
    if let Ok(data) = std::env::var("XDG_DATA_DIRS") {
        dirs.extend(std::env::split_paths(&data).map(|d| d.join("hunspell")));
    }
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(Path::new(&home).join(".nix-profile/share/hunspell"));
    }
    dirs.extend(
        [
            "/run/current-system/sw/share/hunspell",
            "/usr/share/hunspell",
            "/usr/share/myspell/dicts",
            "/usr/share/myspell",
        ]
        .map(PathBuf::from),
    );
    dirs
}

/// Reads a dictionary file; Latin-1 ones (`SET ISO8859-1`, older German
/// dictionaries) are converted to UTF-8.
fn read_dictionary(aff: &Path, dic: &Path) -> Result<(String, String)> {
    let aff_bytes = std::fs::read(aff).with_context(|| format!("cannot read {}", aff.display()))?;
    let dic_bytes = std::fs::read(dic).with_context(|| format!("cannot read {}", dic.display()))?;
    let latin1 = |b: &[u8]| -> String { b.iter().map(|&c| char::from(c)).collect() };
    let head = String::from_utf8_lossy(&aff_bytes[..aff_bytes.len().min(4096)]).to_ascii_uppercase();
    if head.contains("SET ISO8859-1") || head.contains("SET ISO-8859-1") || head.contains("SET ISO8859-15") {
        let aff = latin1(&aff_bytes)
            .lines()
            .map(|l| if l.trim_start().starts_with("SET ") { "SET UTF-8" } else { l })
            .collect::<Vec<_>>()
            .join("\n");
        Ok((aff, latin1(&dic_bytes)))
    } else {
        Ok((String::from_utf8_lossy(&aff_bytes).into_owned(), String::from_utf8_lossy(&dic_bytes).into_owned()))
    }
}

pub struct Checker {
    dictionaries: Vec<(String, spellbook::Dictionary)>,
    personal: HashSet<String>,
    personal_file: Option<PathBuf>,
    known: HashMap<String, bool>,
}

impl Checker {
    /// Loads the dictionaries of [`LANGUAGES`] found in `dirs` and the
    /// personal word list; an error if none was found.
    pub fn load(dirs: &[PathBuf], personal_file: Option<PathBuf>) -> Result<Checker> {
        let mut dictionaries = Vec::new();
        for lang in LANGUAGES {
            let Some(dir) = dirs
                .iter()
                .find(|d| d.join(format!("{lang}.aff")).is_file() && d.join(format!("{lang}.dic")).is_file())
            else {
                log::info!("spellcheck: no {lang} dictionary");
                continue;
            };
            let (aff, dic) = read_dictionary(&dir.join(format!("{lang}.aff")), &dir.join(format!("{lang}.dic")))?;
            match spellbook::Dictionary::new(&aff, &dic) {
                Ok(d) => dictionaries.push((lang.to_owned(), d)),
                Err(err) => log::warn!("spellcheck: {lang} in {}: {err}", dir.display()),
            }
        }
        if dictionaries.is_empty() {
            anyhow::bail!("Keine Wörterbücher gefunden (de_DE, en_US)");
        }
        let personal = personal_file
            .as_ref()
            .and_then(|f| std::fs::read_to_string(f).ok())
            .map(|t| t.lines().map(|l| l.trim().to_owned()).filter(|l| !l.is_empty()).collect())
            .unwrap_or_default();
        Ok(Checker { dictionaries, personal, personal_file, known: HashMap::new() })
    }

    /// Languages loaded (`de_DE`, `en_US`).
    pub fn languages(&self) -> Vec<String> {
        self.dictionaries.iter().map(|(l, _)| l.clone()).collect()
    }

    pub fn check(&mut self, word: &str) -> bool {
        if let Some(&ok) = self.known.get(word) {
            return ok;
        }
        let ok = self.personal.contains(word)
            || self.personal.contains(&word.to_lowercase())
            || self.dictionaries.iter().any(|(_, d)| d.check(word));
        self.known.insert(word.to_owned(), ok);
        ok
    }

    /// Up to `limit` suggestions, from all dictionaries.
    pub fn suggest(&self, word: &str, limit: usize) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for (_, d) in &self.dictionaries {
            let mut found = Vec::new();
            d.suggest(word, &mut found);
            for s in found {
                if !out.contains(&s) {
                    out.push(s);
                }
            }
        }
        out.truncate(limit);
        out
    }

    /// Adds `word` to the personal word list (and its file).
    pub fn add(&mut self, word: &str) -> Result<()> {
        if self.personal.insert(word.to_owned())
            && let Some(file) = &self.personal_file
        {
            if let Some(dir) = file.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let mut words: Vec<&String> = self.personal.iter().collect();
            words.sort();
            let text: String = words.iter().map(|w| format!("{w}\n")).collect();
            std::fs::write(file, text).with_context(|| format!("cannot write {}", file.display()))?;
        }
        self.known.insert(word.to_owned(), true);
        Ok(())
    }

    /// Misspelled words of the Markdown `src` (byte ranges).
    pub fn misspelled(&mut self, src: &str) -> Vec<Range<usize>> {
        prose_words(src).into_iter().filter(|r| !self.check(&src[r.clone()])).collect()
    }
}

/// Words of the prose of a Markdown text (byte ranges): letters with
/// apostrophes inside; parts joined by hyphens count one by one. Left out:
/// code, links and their targets, math, HTML, front matter, `#tags`,
/// words with digits, acronyms (all capitals) and CamelCase.
pub fn prose_words(src: &str) -> Vec<Range<usize>> {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
        | Options::ENABLE_MATH
        | Options::ENABLE_WIKILINKS;
    let mut words = Vec::new();
    let mut skip = 0usize; // inside code blocks, metadata, links
    for (event, range) in Parser::new_ext(src, options).into_offset_iter() {
        match event {
            Event::Start(Tag::CodeBlock(_) | Tag::MetadataBlock(_) | Tag::Link { .. } | Tag::Image { .. }) => skip += 1,
            Event::End(TagEnd::CodeBlock | TagEnd::MetadataBlock(_) | TagEnd::Link | TagEnd::Image) => {
                skip = skip.saturating_sub(1)
            }
            Event::Text(_) if skip == 0 => words_in(src, range, &mut words),
            _ => {}
        }
    }
    words
}

fn words_in(src: &str, range: Range<usize>, out: &mut Vec<Range<usize>>) {
    let text = &src[range.clone()];
    // Whitespace separated chunks: a chunk that looks like a URL, path,
    // mail address or tag is left alone.
    let mut offset = 0;
    for chunk in text.split(|c: char| c.is_whitespace()) {
        let start = range.start + offset;
        offset += chunk.len() + 1;
        if chunk.contains("://")
            || chunk.starts_with("www.")
            || chunk.contains('@')
            || chunk.starts_with('#')
            || chunk.contains('/')
        {
            continue;
        }
        let mut word_start: Option<usize> = None;
        let mut chars = chunk.char_indices().peekable();
        while let Some((i, c)) = chars.next() {
            let next_is_letter = chars.peek().is_some_and(|(_, n)| n.is_alphabetic());
            let inside = c.is_alphabetic() || (word_start.is_some() && (c == '\'' || c == '’') && next_is_letter);
            if inside {
                word_start.get_or_insert(i);
            }
            if (!inside || chars.peek().is_none())
                && let Some(ws) = word_start.take()
            {
                let end = if inside { i + c.len_utf8() } else { i };
                // A digit right before or after: part of a code or number.
                let touches_digit = chunk[..ws].chars().next_back().is_some_and(|p| p.is_ascii_digit())
                    || chunk[end..].chars().next().is_some_and(|n| n.is_ascii_digit());
                if !touches_digit && worth_checking(&chunk[ws..end]) {
                    out.push(start + ws..start + end);
                }
            }
        }
    }
}

fn worth_checking(word: &str) -> bool {
    let letters = word.chars().filter(|c| c.is_alphabetic()).count();
    if letters < 2 {
        return false;
    }
    // Acronyms (HTTP, KIT) and CamelCase (QtQuick) are names, not words.
    let upper = word.chars().filter(|c| c.is_uppercase()).count();
    if upper == letters || word.chars().skip(1).any(char::is_uppercase) {
        return false;
    }
    true
}

/// Byte ranges of `src` as UTF-16 offsets and lengths (positions in Qt).
pub fn utf16_ranges(src: &str, ranges: &[Range<usize>]) -> Vec<(usize, usize)> {
    let mut out = Vec::with_capacity(ranges.len());
    let mut byte = 0;
    let mut utf16 = 0;
    for r in ranges {
        utf16 += src[byte..r.start].encode_utf16().count();
        let len = src[r.clone()].encode_utf16().count();
        out.push((utf16, len));
        utf16 += len;
        byte = r.end;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(src: &str) -> Vec<&str> {
        prose_words(src).into_iter().map(|r| &src[r]).collect()
    }

    #[test]
    fn checks_prose_only() {
        let src = "---\ntags: [Rechtschreibung]\n---\n\n# Überschrift\n\nDas ist ein Tesst mit `kode`, [Linktext](ziel.md), \
                   https://exampel.org, #tagg, HTTP, QtQuick, 3D und e-Mail.\n\n```\nfalsch geschribn\n```\n\n$x = \\alpha$ Ende.\n";
        assert_eq!(
            words(src),
            vec!["Überschrift", "Das", "ist", "ein", "Tesst", "mit", "und", "e", "Mail", "Ende"]
                .into_iter()
                .filter(|w| w.chars().count() > 1)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn apostrophes_and_utf16_positions() {
        let src = "Ärger isn't größer 😀 Wort";
        let found = prose_words(src);
        let w: Vec<&str> = found.iter().map(|r| &src[r.clone()]).collect();
        assert_eq!(w, vec!["Ärger", "isn't", "größer", "Wort"]);
        // The emoji is two UTF-16 units.
        assert_eq!(utf16_ranges(src, &found).last(), Some(&(22, 4)));
    }

    /// With the real dictionaries where they are installed (Nix dev shell).
    #[test]
    fn real_dictionaries_accept_normal_text() {
        let started = std::time::Instant::now();
        let Ok(mut checker) = Checker::load(&dictionary_dirs(), None) else { return };
        let loaded = started.elapsed();
        let src = "Die Übungsblätter für Analysis liegen im Ordner. Morgen schreiben wir die Klausur, \
                   danach gehen wir Kaffee trinken. The lecture notes are online and the homework is due on Friday. \
                   Das ist ein Tippfeler und this is a mistaek.";
        let bad: Vec<&str> = checker.misspelled(src).into_iter().map(|r| &src[r]).collect();
        assert_eq!(bad, vec!["Tippfeler", "mistaek"], "languages {:?}", checker.languages());
        assert!(checker.suggest("Tippfeler", 5).iter().any(|s| s == "Tippfehler"));
        assert!(loaded.as_secs() < 10, "loading took {loaded:?}");
        eprintln!("dictionaries {:?} loaded in {loaded:?}", checker.languages());
    }

    #[test]
    fn checks_with_a_small_dictionary_and_a_personal_list() {
        let dir = tempfile::tempdir().unwrap();
        // A tiny Latin-1 German dictionary, as older ones are.
        std::fs::write(dir.path().join("de_DE.aff"), b"SET ISO8859-1\nTRY esianrtolcdugmphbyfvkwz\n").unwrap();
        let dic: Vec<u8> = b"3\nTest\ngr\xf6\xdfer\nWort\n".to_vec();
        std::fs::write(dir.path().join("de_DE.dic"), dic).unwrap();
        let personal = dir.path().join("config/dictionary.txt");
        let mut checker = Checker::load(&[dir.path().to_path_buf()], Some(personal.clone())).unwrap();
        assert_eq!(checker.languages(), vec!["de_DE"]);
        assert!(checker.check("größer") && checker.check("Test") && !checker.check("Tesst"));
        let src = "Ein Test, Tesst und Wort.";
        let bad: Vec<&str> = checker.misspelled(src).into_iter().map(|r| &src[r]).collect();
        assert_eq!(bad, vec!["Ein", "Tesst", "und"]);
        assert!(checker.suggest("Tesst", 5).contains(&"Test".to_owned()));
        checker.add("Tesst").unwrap();
        assert!(checker.check("Tesst"));
        assert_eq!(std::fs::read_to_string(&personal).unwrap(), "Tesst\n");
        let mut again = Checker::load(&[dir.path().to_path_buf()], Some(personal)).unwrap();
        assert!(again.check("Tesst"));
    }
}
