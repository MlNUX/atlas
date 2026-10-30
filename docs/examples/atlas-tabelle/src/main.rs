// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Beispiel-Erweiterung für Atlas: ein einziges Programm für drei Beiträge
//! (siehe extension.yaml und docs/extensions-guide.md).
//!
//! - `statistik <datei>`        – Befehl: läuft sichtbar im Terminal von Atlas
//! - `markdown <datei>`         – Dateiverarbeitung: Markdown nach stdout
//! - `html <datei> <ausgabe>`   – Exporter: schreibt nach `{output}`
//!
//! Einstellungen kommen als Umgebungsvariablen (`ATLAS_SETTING_…`). Fehler
//! gehen nach stderr mit einem Exit-Code ungleich 0; Atlas zeigt dann die
//! letzten Zeilen von stderr.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["statistik", file] => statistik(file),
        ["markdown", file] => markdown(file),
        ["html", file, output] => html(file, output),
        _ => Err("Aufruf: atlas-tabelle statistik|markdown <datei> | html <datei> <ausgabe>".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("atlas-tabelle: {message}");
            ExitCode::FAILURE
        }
    }
}

/// Einstellung `trennzeichen`; „auto“ wählt das häufigste Zeichen der ersten Zeile.
fn separator(text: &str, file: &str) -> char {
    match std::env::var("ATLAS_SETTING_TRENNZEICHEN").as_deref() {
        Ok("komma") => ',',
        Ok("semikolon") => ';',
        Ok("tab") => '\t',
        _ if file.to_lowercase().ends_with(".tsv") => '\t',
        _ => {
            let first = text.lines().next().unwrap_or_default();
            [',', ';', '\t'].into_iter().max_by_key(|c| first.matches(*c).count()).unwrap_or(',')
        }
    }
}

/// Einstellung `max_zeilen` (Zahl; Atlas übergibt sie als Text).
fn max_rows() -> usize {
    std::env::var("ATLAS_SETTING_MAX_ZEILEN")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .map_or(500, |v| v.max(1.0) as usize)
}

fn read(file: &str) -> Result<(Vec<Vec<String>>, char), String> {
    let text = std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
    let sep = separator(&text, file);
    Ok((parse(&text, sep), sep))
}

/// CSV nach RFC 4180: Felder in Anführungszeichen dürfen Trennzeichen,
/// Zeilenumbrüche und verdoppelte Anführungszeichen enthalten.
fn parse(text: &str, sep: char) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let (mut row, mut field) = (Vec::new(), String::new());
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' if field.is_empty() && !quoted => quoted = true,
            '"' if quoted => quoted = false,
            c if c == sep && !quoted => row.push(std::mem::take(&mut field)),
            '\r' if !quoted => {}
            '\n' if !quoted => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            c => field.push(c),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

fn statistik(file: &str) -> Result<(), String> {
    let (rows, sep) = read(file)?;
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    let empty = rows.iter().flatten().filter(|f| f.trim().is_empty()).count();
    let name = match sep {
        '\t' => "Tab".to_owned(),
        c => format!("„{c}“"),
    };
    println!("{file}");
    println!("  Zeilen:        {} (davon 1 Kopfzeile)", rows.len());
    println!("  Spalten:       {columns}");
    println!("  Leere Felder:  {empty}");
    println!("  Trennzeichen:  {name}");
    if let Some(header) = rows.first() {
        println!("  Kopfzeile:     {}", header.join(" | "));
    }
    Ok(())
}

/// Text für eine Markdown-Tabellenzelle.
fn cell(text: &str) -> String {
    text.replace('\\', "\\\\").replace('|', "\\|").replace('\n', "<br>")
}

fn markdown(file: &str) -> Result<(), String> {
    let (rows, _) = read(file)?;
    let Some(header) = rows.first() else {
        println!("*Leere Tabelle.*");
        return Ok(());
    };
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    let line = |row: &[String]| {
        let cells: Vec<String> = (0..columns).map(|i| cell(row.get(i).map_or("", String::as_str))).collect();
        format!("| {} |", cells.join(" | "))
    };
    println!("{}", line(header));
    println!("|{}|", vec!["---"; columns].join("|"));
    let limit = max_rows();
    for row in rows.iter().skip(1).take(limit) {
        println!("{}", line(row));
    }
    if rows.len() - 1 > limit {
        println!("\n*… {} weitere Zeilen (Einstellung „Höchstens angezeigte Zeilen“).*", rows.len() - 1 - limit);
    }
    Ok(())
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn html(file: &str, output: &str) -> Result<(), String> {
    let (rows, _) = read(file)?;
    let title = std::path::Path::new(file).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let mut out = format!(
        "<!doctype html>\n<meta charset=\"utf-8\">\n<title>{}</title>\n<style>table{{border-collapse:collapse}}td,th{{border:1px solid #999;padding:4px 8px}}</style>\n<table>\n",
        escape(&title)
    );
    for (i, row) in rows.iter().enumerate() {
        let tag = if i == 0 { "th" } else { "td" };
        out.push_str("<tr>");
        for field in row {
            out.push_str(&format!("<{tag}>{}</{tag}>", escape(field)));
        }
        out.push_str("</tr>\n");
    }
    out.push_str("</table>\n");
    // Atlas gibt eine temporäre Datei vor und benennt sie erst bei Erfolg um.
    std::fs::write(output, out).map_err(|e| format!("{output}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_quotes_separators_and_line_breaks() {
        let rows = parse("a;\"b;c\";\"d\"\"e\"\n1;\"zwei\nZeilen\";3\r\n", ';');
        assert_eq!(rows, vec![vec!["a", "b;c", "d\"e"], vec!["1", "zwei\nZeilen", "3"]]);
    }

    #[test]
    fn detects_separator_and_escapes_cells() {
        // SAFETY: the test sets no other variables and runs alone in this process.
        unsafe { std::env::remove_var("ATLAS_SETTING_TRENNZEICHEN") };
        assert_eq!(separator("a;b;c\n", "x.csv"), ';');
        assert_eq!(separator("a,b\n", "x.tsv"), '\t');
        assert_eq!(cell("a|b\nc"), "a\\|b<br>c");
    }
}
