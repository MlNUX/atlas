// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Poppler vs. PDFium comparison for Phase 0.
//!
//! ```text
//! cargo run --release -p atlas-core --example pdf_compare -- [--pages N] [--width PX] [--png DIR] FILE.pdf...
//! ```
//!
//! For every file and engine it measures opening, rendering the first N
//! pages and extracting the text of page 1, and prints a Markdown table.
//! With `--png DIR` the first page of each engine is written as PNG for a
//! visual comparison.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use atlas_core::pdf::{Engine, PdfService, RenderedPage};

fn png(page: &RenderedPage, path: &Path) -> anyhow::Result<()> {
    // BGRA premultiplied → cairo ARGB32 surface → PNG.
    let stride = cairo::Format::ARgb32.stride_for_width(page.width)? as usize;
    let mut data = vec![0u8; stride * page.height as usize];
    for (row, src) in data.chunks_mut(stride).zip(page.bgra.chunks(page.width as usize * 4)) {
        row[..src.len()].copy_from_slice(src);
    }
    let surface = cairo::ImageSurface::create_for_data(
        data,
        cairo::Format::ARgb32,
        page.width as i32,
        page.height as i32,
        stride as i32,
    )?;
    let mut file = std::fs::File::create(path)?;
    surface.write_to_png(&mut file)?;
    Ok(())
}

fn ms(d: Duration) -> String {
    format!("{:.1}", d.as_secs_f64() * 1000.0)
}

fn main() -> anyhow::Result<()> {
    let mut pages = 10usize;
    let mut width = 1200u32;
    let mut png_dir: Option<PathBuf> = None;
    let mut files = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--pages" => pages = args.next().unwrap_or_default().parse()?,
            "--width" => width = args.next().unwrap_or_default().parse()?,
            "--png" => png_dir = args.next().map(PathBuf::from),
            _ => files.push(PathBuf::from(arg)),
        }
    }
    if files.is_empty() {
        anyhow::bail!("usage: pdf_compare [--pages N] [--width PX] [--png DIR] FILE.pdf...");
    }
    if let Some(dir) = &png_dir {
        std::fs::create_dir_all(dir)?;
    }

    println!(
        "| Datei | Engine | Seiten | Öffnen ms | Rendern Ø ms | Rendern max ms | Erste Seite ms | Text S.1 ms | Zeichen S.1 |"
    );
    println!("|---|---|---:|---:|---:|---:|---:|---:|---:|");
    for file in &files {
        let name = file.file_name().unwrap_or_default().to_string_lossy();
        for engine in Engine::ALL {
            let service = PdfService::get(engine);
            let started = Instant::now();
            let info = match service.open_blocking(file) {
                Ok(info) => info,
                Err(err) => {
                    println!("| {name} | {} | – | Fehler: {err:#} | | | | | |", engine.name());
                    continue;
                }
            };
            let open = started.elapsed();
            let n = pages.min(info.page_sizes.len());
            let mut times = Vec::new();
            for page in 0..n {
                let t = Instant::now();
                let rendered = service.render_blocking(file, page, width)?;
                times.push(t.elapsed());
                if page == 0
                    && let Some(dir) = &png_dir
                {
                    let stem = file.file_stem().unwrap_or_default().to_string_lossy();
                    png(&rendered, &dir.join(format!("{stem}-{}.png", engine.name())))?;
                }
            }
            let t = Instant::now();
            let text = service.text_blocking(file, 0)?;
            let text_time = t.elapsed();
            let avg = times.iter().sum::<Duration>() / times.len().max(1) as u32;
            let max = times.iter().max().copied().unwrap_or_default();
            let first = times.first().copied().unwrap_or_default();
            println!(
                "| {name} | {} | {} | {} | {} | {} | {} | {} | {} |",
                engine.name(),
                info.page_sizes.len(),
                ms(open),
                ms(avg),
                ms(max),
                ms(first),
                ms(text_time),
                text.chars().count()
            );
            service.close(file.clone());
        }
    }
    Ok(())
}
