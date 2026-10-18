// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Creates a test project for the Phase 0 acceptance checks.
//!
//! ```text
//! cargo run -p atlas-core --example fixture -- /tmp/atlas-fixture [ENTRIES]
//! ```
//!
//! Layout:
//! - `docs/paper.pdf`        – generated 40 page PDF (text, vector art, image)
//! - `notes/README.md`       – Markdown with table, code block, list
//! - `src/…`                 – a few source files
//! - `bulk/dir-NNN/…`        – ENTRIES files spread over 100 directories
//! - `flat/`                 – one directory with ENTRIES files
//! - `ignored/`              – listed in .gitignore, must not appear in search

use std::f64::consts::PI;
use std::fs;
use std::path::{Path, PathBuf};

const WORDS: &[&str] = &[
    "atlas",
    "workspace",
    "projekt",
    "ordner",
    "datei",
    "notiz",
    "terminal",
    "wayland",
    "rendering",
    "seite",
    "zitat",
    "suche",
    "baum",
    "struktur",
    "linux",
    "hyprland",
    "poppler",
    "pdfium",
    "rust",
    "qml",
    "markdown",
    "sitzung",
    "analyse",
    "kapitel",
    "abschnitt",
    "ergebnis",
    "messung",
    "methode",
    "quelle",
    "verweis",
];

fn words(seed: &mut u64, n: usize) -> String {
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        out.push(WORDS[(*seed >> 33) as usize % WORDS.len()]);
    }
    out.join(" ")
}

fn make_pdf(path: &Path, pages: usize) -> anyhow::Result<()> {
    let (w, h) = (595.0, 842.0);
    let surface = cairo::PdfSurface::new(w, h, path)?;
    let cr = cairo::Context::new(&surface)?;
    let mut seed = 42u64;

    // A raster image to exercise image decoding.
    let image = cairo::ImageSurface::create(cairo::Format::Rgb24, 256, 160)?;
    {
        let ic = cairo::Context::new(&image)?;
        for x in 0..256 {
            let t = x as f64 / 255.0;
            ic.set_source_rgb(t, 0.4 + 0.4 * (t * PI).sin(), 1.0 - t);
            ic.rectangle(x as f64, 0.0, 1.0, 160.0);
            ic.fill()?;
        }
    }

    for page in 0..pages {
        cr.set_source_rgb(1.0, 1.0, 1.0);
        cr.paint()?;
        cr.set_source_rgb(0.1, 0.1, 0.1);
        cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
        cr.set_font_size(20.0);
        cr.move_to(56.0, 72.0);
        cr.show_text(&format!("Kapitel {} – Atlas Testdokument", page + 1))?;

        cr.select_font_face("Serif", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
        cr.set_font_size(10.5);
        let mut y = 100.0;
        for _ in 0..22 {
            cr.move_to(56.0, y);
            cr.show_text(&words(&mut seed, 14))?;
            y += 14.0;
        }

        // Vector art: many curves and transparency.
        cr.save()?;
        cr.translate(w / 2.0, 520.0);
        for i in 0..120 {
            let a = i as f64 / 120.0 * 2.0 * PI;
            cr.set_source_rgba(0.5 + 0.5 * a.cos(), 0.3, 0.5 + 0.5 * a.sin(), 0.35);
            cr.move_to(0.0, 0.0);
            cr.curve_to(80.0 * a.cos(), 30.0, 160.0 * a.sin(), -60.0, 180.0 * (a * 2.0).cos(), 120.0 * a.sin());
            cr.set_line_width(1.2);
            cr.stroke()?;
        }
        cr.restore()?;

        cr.set_source_surface(&image, 56.0, 640.0)?;
        cr.paint()?;

        cr.set_source_rgb(0.1, 0.1, 0.1);
        cr.select_font_face("Monospace", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
        cr.set_font_size(9.0);
        cr.move_to(330.0, 660.0);
        cr.show_text("fn main() { println!(\"Hallo Atlas\"); }")?;
        cr.move_to(w - 90.0, h - 40.0);
        cr.show_text(&format!("Seite {}", page + 1))?;
        cr.show_page()?;
    }
    surface.finish();
    Ok(())
}

const README: &str = r#"# Atlas Testprojekt

Dieses Projekt wurde von `cargo run -p atlas-core --example fixture` erzeugt.

## Inhalt

| Ordner    | Zweck                              |
|-----------|------------------------------------|
| `docs/`   | generiertes PDF für den Engine-Vergleich |
| `notes/`  | Markdown-Notizen                   |
| `bulk/`   | viele Dateien in vielen Ordnern    |
| `flat/`   | ein Ordner mit sehr vielen Dateien |

## Beispiel

```rust
fn main() {
    println!("needle in a haystack");
}
```

- [x] Projektbaum lädt ohne Blockieren
- [ ] Externe Änderungen erscheinen im Baum

> Zitate und Links zu PDF-Seiten folgen in Phase 2.
"#;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let root = PathBuf::from(args.next().unwrap_or_else(|| "/tmp/atlas-fixture".into()));
    let entries: usize = args.next().map(|s| s.parse()).transpose()?.unwrap_or(10_000);

    fs::create_dir_all(&root)?;
    for dir in ["docs", "notes", "src/core", "ignored", ".git"] {
        fs::create_dir_all(root.join(dir))?;
    }
    fs::write(root.join(".gitignore"), "ignored/\n*.log\n")?;
    fs::write(root.join("ignored/secret.txt"), "needle (must not be found)\n")?;
    fs::write(root.join("build.log"), "needle (ignored by pattern)\n")?;
    fs::write(root.join("notes/README.md"), README)?;
    fs::write(
        root.join("notes/ideen.md"),
        "# Ideen\n\nTODO: needle für die Suche\n\nFIXME: Links auf `docs/paper.pdf?page=3`\n",
    )?;
    fs::write(root.join("src/main.rs"), "fn main() {\n    // needle\n    atlas::run();\n}\n")?;
    fs::write(root.join("src/core/tree.rs"), "pub struct Tree;\n// TODO: needle\n")?;
    make_pdf(&root.join("docs/paper.pdf"), 40)?;

    let mut seed = 7u64;
    let per_dir = entries.div_ceil(100);
    for d in 0..100 {
        let dir = root.join(format!("bulk/dir-{d:03}"));
        fs::create_dir_all(&dir)?;
        for f in 0..per_dir {
            fs::write(dir.join(format!("file-{f:04}.txt")), words(&mut seed, 8))?;
        }
    }
    let flat = root.join("flat");
    fs::create_dir_all(&flat)?;
    for f in 0..entries {
        fs::write(flat.join(format!("entry-{f:05}.md")), "")?;
    }
    println!("fixture created at {} ({} + {} files)", root.display(), per_dir * 100, entries);
    Ok(())
}
