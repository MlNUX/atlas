// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Small text helpers shared by several modules.

/// Decodes `%xx` escapes (URLs, `file://` paths). Invalid or cut-off
/// escapes stay as they are; bytes that are no UTF-8 become `�`.
pub fn percent_decode(s: &str) -> String {
    percent_encoding::percent_decode_str(s).decode_utf8_lossy().into_owned()
}

/// An ID from a name: lowercase, every run of other characters than
/// letters and digits becomes one `-`, none at the ends. Empty if the name
/// has no letters or digits (callers choose their own fallback).
pub fn slug(name: &str) -> String {
    name.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_percent_escapes() {
        assert_eq!(percent_decode("/home/a%20b/%C3%A4.pdf"), "/home/a b/ä.pdf");
        assert_eq!(percent_decode("a%20b%zz%4"), "a b%zz%4");
        assert_eq!(percent_decode("/x%2"), "/x%2");
        assert_eq!(percent_decode("/x%aä"), "/x%aä");
        // A sign is no hex digit (u8::from_str_radix took "+1").
        assert_eq!(percent_decode("%+1"), "%+1");
    }

    #[test]
    fn slugs() {
        assert_eq!(slug("Mit Pandoc nach HTML!"), "mit-pandoc-nach-html");
        assert_eq!(slug("  Größe – Übung  "), "größe-übung");
        assert_eq!(slug("--"), "");
    }
}
