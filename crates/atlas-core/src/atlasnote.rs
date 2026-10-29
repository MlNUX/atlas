// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Read-only support for `.atlasnote` notebooks written by Atlas Tablet.
//!
//! ```text
//! Notiz.atlasnote (ZIP)
//!   notebook.json      pages, strokes, text boxes, placed images (format 2 or 3)
//!   pdf/<uuid>.pdf     PDF backgrounds
//!   images/<uuid>.png  image pages; placed pictures may also be .jpg
//! ```
//!
//! Pages are 1000 units wide; their height comes from the page. Points are
//! `[tool, x, y, time, unit, pressure, tilt, orientation]` with pressure
//! −1 when unknown. Colours are Android ARGB integers. A placed image is a
//! mark with `image` and three points: top left, top right, bottom left.
//!
//! The archive is extracted once into the cache (keyed by path, size and
//! modification time) and pages are rendered with cairo to PNG files there;
//! the original file is never written.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;

pub const PAGE_WIDTH: f64 = 1000.0;
const ARCHIVE_LIMIT: u64 = 500 * 1024 * 1024;
const JSON_LIMIT: u64 = 20 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize)]
pub struct Notebook {
    pub version: u32,
    pub pages: Vec<Page>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Page {
    pub id: String,
    pub paper: String,
    pub dark: bool,
    pub height: f64,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub pdf: Option<PdfBackground>,
    #[serde(default)]
    pub marks: Vec<Mark>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PdfBackground {
    pub file: String,
    pub page: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Style {
    pub tool: String,
    pub color: i64,
    pub width: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Mark {
    pub style: Style,
    pub points: Vec<[f64; 8]>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub rotation: f64,
    #[serde(default)]
    pub image: Option<String>,
}

fn valid_source(name: &str) -> bool {
    let check = |dir: &str, exts: &[&str]| {
        name.strip_prefix(dir).and_then(|rest| rest.rsplit_once('.')).is_some_and(|(stem, ext)| {
            !stem.is_empty() && stem.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') && exts.contains(&ext)
        })
    };
    check("pdf/", &["pdf"]) || check("images/", &["png", "jpg"])
}

impl Notebook {
    pub fn parse(json: &str) -> Result<Self> {
        let notebook: Notebook = serde_json::from_str(json).context("notebook.json ist kein Atlas-Notizformat")?;
        ensure!(notebook.version == 2 || notebook.version == 3, "Unbekannte Notizversion {}", notebook.version);
        ensure!(!notebook.pages.is_empty(), "Die Notiz hat keine Seiten");
        for page in &notebook.pages {
            ensure!(page.height.is_finite() && (100.0..=100_000.0).contains(&page.height), "Ungültige Seitenhöhe");
            for name in page.sources() {
                ensure!(valid_source(&name), "Ungültiger Quellname {name}");
            }
            for mark in &page.marks {
                ensure!(mark.image.is_none() || mark.points.len() == 3, "Bild ohne drei Eckpunkte");
                ensure!(mark.points.iter().flatten().all(|v| v.is_finite()), "Ungültige Koordinaten");
            }
        }
        Ok(notebook)
    }
}

impl Page {
    /// Files of this page as `pdf/<name>` or `images/<name>`.
    pub fn sources(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(pdf) = &self.pdf {
            out.push(format!("pdf/{}", pdf.file));
        }
        if let Some(image) = &self.image {
            out.push(format!("images/{image}"));
        }
        out.extend(self.marks.iter().filter_map(|m| m.image.as_ref().map(|i| format!("images/{i}"))));
        out
    }
}

/// An opened notebook with its sources extracted into `dir`.
pub struct NoteFile {
    pub notebook: Notebook,
    dir: PathBuf,
}

fn cache_root() -> PathBuf {
    dirs::cache_dir().unwrap_or_else(std::env::temp_dir).join("atlas").join("atlasnote")
}

/// Cache folder for this version of the file: changes with size and mtime.
fn cache_dir_for(path: &Path) -> Result<PathBuf> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::metadata(path).with_context(|| format!("{} kann nicht gelesen werden", path.display()))?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(path.as_os_str().as_encoded_bytes());
    hasher.update(&meta.len().to_le_bytes());
    hasher.update(&meta.mtime().to_le_bytes());
    hasher.update(&meta.mtime_nsec().to_le_bytes());
    Ok(cache_root().join(&hasher.finalize().to_hex()[..24]))
}

/// Unpacks only known entries, with the same limits as Atlas Tablet.
fn extract(archive: &Path, target: &Path) -> Result<()> {
    let mut zip = zip::ZipArchive::new(File::open(archive)?).context("Keine gültige Atlas-Notizdatei (ZIP)")?;
    ensure!(zip.len() <= 1000, "Zu viele Dateien im Notizarchiv");
    let mut total = 0u64;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let name = entry.name().to_owned();
        if entry.is_dir() {
            continue;
        }
        ensure!(name == "notebook.json" || valid_source(&name), "Ungültiger Dateipfad im Notizarchiv: {name}");
        let limit = if name == "notebook.json" { JSON_LIMIT } else { 150 * 1024 * 1024 };
        let out = target.join(&name);
        std::fs::create_dir_all(out.parent().context("Pfad ohne Ordner")?)?;
        let mut file = File::create(&out)?;
        let mut buffer = [0u8; 64 * 1024];
        let mut written = 0u64;
        loop {
            let n = entry.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            written += n as u64;
            total += n as u64;
            ensure!(written <= limit && total <= ARCHIVE_LIMIT, "Notizarchiv ist zu groß");
            file.write_all(&buffer[..n])?;
        }
    }
    Ok(())
}

impl NoteFile {
    pub fn open(path: &Path) -> Result<Self> {
        let dir = cache_dir_for(path)?;
        if !dir.join("notebook.json").is_file() {
            // Extract next to the final folder and rename, so a half-extracted folder is never used.
            let parent = dir.parent().context("Cache ohne Ordner")?;
            std::fs::create_dir_all(parent)?;
            let staging = tempfile_dir(parent)?;
            let result = extract(path, &staging);
            if let Err(err) = result {
                let _ = std::fs::remove_dir_all(&staging);
                return Err(err);
            }
            if std::fs::rename(&staging, &dir).is_err() {
                // Another viewer extracted the same version meanwhile.
                let _ = std::fs::remove_dir_all(&staging);
            }
        }
        let json = std::fs::read_to_string(dir.join("notebook.json")).context("notebook.json fehlt im Notizarchiv")?;
        let notebook = Notebook::parse(&json)?;
        for name in notebook.pages.iter().flat_map(Page::sources) {
            ensure!(dir.join(&name).is_file(), "Eine Quelle fehlt im Notizarchiv: {name}");
        }
        Ok(Self { notebook, dir })
    }

    /// PNG of page `index` at `width_px` pixels, rendered once and cached.
    pub fn render_png(&self, index: usize, width_px: u32) -> Result<PathBuf> {
        let width_px = width_px.clamp(100, 4000);
        let out = self.dir.join("render").join(format!("{index}-{width_px}.png"));
        if out.is_file() {
            return Ok(out);
        }
        let (w, h, rgba) = self.render(index, width_px)?;
        std::fs::create_dir_all(out.parent().context("Pfad ohne Ordner")?)?;
        let partial = out.with_extension("part");
        image::save_buffer_with_format(&partial, &rgba, w, h, image::ColorType::Rgba8, image::ImageFormat::Png)?;
        std::fs::rename(&partial, &out)?;
        Ok(out)
    }

    /// Page `index` as RGBA pixels, `width_px` wide.
    pub fn render(&self, index: usize, width_px: u32) -> Result<(u32, u32, Vec<u8>)> {
        let page = self.notebook.pages.get(index).context("Seite nicht vorhanden")?;
        let scale = f64::from(width_px) / PAGE_WIDTH;
        let height_px = (page.height * scale).ceil().max(1.0) as u32;
        let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, width_px as i32, height_px as i32)?;
        {
            let cr = cairo::Context::new(&surface)?;
            cr.scale(scale, scale);
            self.draw_paper(&cr, page)?;
            for mark in &page.marks {
                cr.save()?;
                if mark.image.is_some() {
                    self.draw_image(&cr, mark)?;
                } else if mark.text.is_some() {
                    draw_text(&cr, mark)?;
                } else {
                    draw_stroke(&cr, mark)?;
                }
                cr.restore()?;
            }
        }
        surface.flush();
        let stride = surface.stride() as usize;
        let data = surface.data().context("cairo surface data")?;
        // Premultiplied BGRA to straight RGBA.
        let mut rgba = Vec::with_capacity(width_px as usize * height_px as usize * 4);
        for row in data.chunks(stride).take(height_px as usize) {
            for px in row[..width_px as usize * 4].chunks(4) {
                let a = px[3];
                let un = |c: u8| {
                    if a == 0 { 0 } else { ((u16::from(c) * 255 + u16::from(a) / 2) / u16::from(a)).min(255) as u8 }
                };
                rgba.extend_from_slice(&[un(px[2]), un(px[1]), un(px[0]), a]);
            }
        }
        Ok((width_px, height_px, rgba))
    }

    fn draw_paper(&self, cr: &cairo::Context, page: &Page) -> Result<()> {
        let (r, g, b) = if page.dark { rgb(0x242528) } else { rgb(0xfffdf7) };
        cr.set_source_rgb(r, g, b);
        cr.rectangle(0.0, 0.0, PAGE_WIDTH, page.height);
        cr.fill()?;
        if let Some(pdf) = &page.pdf {
            cr.set_source_rgb(1.0, 1.0, 1.0);
            cr.rectangle(0.0, 0.0, PAGE_WIDTH, page.height);
            cr.fill()?;
            let uri = glib::filename_to_uri(self.dir.join("pdf").join(&pdf.file), None).context("ungültiger Pfad")?;
            let doc = poppler::Document::from_file(&uri, None).map_err(|e| anyhow::anyhow!("PDF-Hintergrund: {e}"))?;
            if (0..doc.n_pages()).contains(&pdf.page)
                && let Some(p) = doc.page(pdf.page)
            {
                let (w, h) = p.size();
                cr.save()?;
                cr.scale(PAGE_WIDTH / w, page.height / h);
                p.render(cr);
                cr.restore()?;
            }
            return Ok(());
        }
        if let Some(image) = &page.image {
            let surface = load_picture(&self.dir.join("images").join(image))?;
            cr.save()?;
            cr.scale(PAGE_WIDTH / f64::from(surface.width()), page.height / f64::from(surface.height()));
            cr.set_source_surface(&surface, 0.0, 0.0)?;
            cr.paint()?;
            cr.restore()?;
            return Ok(());
        }
        let (r, g, b) = if page.dark { rgb(0x424348) } else { rgb(0xd9dfeb) };
        cr.set_source_rgb(r, g, b);
        cr.set_line_width(0.65);
        match page.paper.as_str() {
            "LINED" => {
                let mut y = 48.0;
                while y < page.height {
                    cr.move_to(32.0, y);
                    cr.line_to(968.0, y);
                    y += 32.0;
                }
                cr.stroke()?;
            }
            "GRID" => {
                let start = paper_start(48.0, 0.0);
                let mut x = start;
                while x < PAGE_WIDTH {
                    cr.move_to(x, 0.0);
                    cr.line_to(x, page.height);
                    x += 48.0;
                }
                let mut y = start;
                while y < page.height {
                    cr.move_to(0.0, y);
                    cr.line_to(PAGE_WIDTH, y);
                    y += 48.0;
                }
                cr.stroke()?;
            }
            "DOTS" => {
                let start = paper_start(32.0, 16.0);
                let mut y = start;
                while y < page.height {
                    let mut x = start;
                    while x <= PAGE_WIDTH - start {
                        cr.new_sub_path();
                        cr.arc(x, y, 1.1, 0.0, std::f64::consts::TAU);
                        x += 32.0;
                    }
                    y += 32.0;
                }
                cr.fill()?;
            }
            _ => {}
        }
        Ok(())
    }

    fn draw_image(&self, cr: &cairo::Context, mark: &Mark) -> Result<()> {
        let name = mark.image.as_deref().context("Bild fehlt")?;
        let surface = load_picture(&self.dir.join("images").join(name))?;
        let (w, h) = (f64::from(surface.width()), f64::from(surface.height()));
        let [a, b, c] = [&mark.points[0], &mark.points[1], &mark.points[2]];
        // Maps the picture's pixels onto the three corners (move, scale and rotation in one).
        cr.transform(cairo::Matrix::new(
            (b[1] - a[1]) / w,
            (b[2] - a[2]) / w,
            (c[1] - a[1]) / h,
            (c[2] - a[2]) / h,
            a[1],
            a[2],
        ));
        cr.set_source_surface(&surface, 0.0, 0.0)?;
        cr.paint()?;
        Ok(())
    }
}

/// First line of a pattern with spacing `step`, centred with at least `margin` at both edges (as on the tablet).
pub fn paper_start(step: f64, margin: f64) -> f64 {
    (PAGE_WIDTH - ((PAGE_WIDTH - 2.0 * margin) / step).floor() * step) / 2.0
}

fn rgb(value: u32) -> (f64, f64, f64) {
    (f64::from((value >> 16) & 0xff) / 255.0, f64::from((value >> 8) & 0xff) / 255.0, f64::from(value & 0xff) / 255.0)
}

/// Colour and opacity of a stroke, following the tablet's brushes.
fn stroke_color(style: &Style) -> (f64, f64, f64, f64) {
    let argb = (style.color & 0xffff_ffff) as u32;
    let (r, g, b) = rgb(argb);
    let alpha = f64::from(argb >> 24) / 255.0;
    let alpha = match style.tool.as_str() {
        "HIGHLIGHTER" => f64::from(0x55) / 255.0,
        "PENCIL" => alpha * f64::from(0xb4) / 255.0,
        _ => alpha,
    };
    (r, g, b, alpha)
}

fn draw_stroke(cr: &cairo::Context, mark: &Mark) -> Result<()> {
    let (r, g, b, alpha) = stroke_color(&mark.style);
    let width = mark.style.width;
    let pressure = matches!(mark.style.tool.as_str(), "PEN" | "PENCIL");
    let translucent = alpha < 0.999;
    // Overlapping segments of a translucent stroke must not darken each other: draw opaque, then fade as a whole.
    if translucent {
        cr.push_group();
        cr.set_source_rgb(r, g, b);
    } else {
        cr.set_source_rgba(r, g, b, alpha);
    }
    cr.set_line_cap(cairo::LineCap::Round);
    cr.set_line_join(cairo::LineJoin::Round);
    let points = &mark.points;
    if points.len() == 1 {
        cr.arc(points[0][1], points[0][2], width / 2.0, 0.0, std::f64::consts::TAU);
        cr.fill()?;
    } else if pressure && points.iter().any(|p| p[5] >= 0.0) {
        for pair in points.windows(2) {
            let p = (pair[0][5].max(0.0) + pair[1][5].max(0.0)) / 2.0;
            cr.set_line_width(width * (0.4 + 0.8 * p));
            cr.move_to(pair[0][1], pair[0][2]);
            cr.line_to(pair[1][1], pair[1][2]);
            cr.stroke()?;
        }
    } else {
        cr.set_line_width(width);
        cr.move_to(points[0][1], points[0][2]);
        for p in &points[1..] {
            cr.line_to(p[1], p[2]);
        }
        cr.stroke()?;
    }
    if translucent {
        cr.pop_group_to_source()?;
        cr.paint_with_alpha(alpha)?;
    }
    Ok(())
}

fn draw_text(cr: &cairo::Context, mark: &Mark) -> Result<()> {
    let text = mark.text.as_deref().unwrap_or_default();
    let origin = mark.points.first().context("Textfeld ohne Position")?;
    let size = mark.style.width;
    let (r, g, b, alpha) = stroke_color(&mark.style);
    cr.translate(origin[1], origin[2]);
    cr.rotate(mark.rotation.to_radians());
    cr.set_source_rgba(r, g, b, alpha);
    cr.select_font_face("sans-serif", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    cr.set_font_size(size);
    // Same wrapping as the tablet: a fixed number of characters per line.
    let limit = ((900.0 / (size * 0.65)) as usize).max(1);
    let lines = text.lines().flat_map(|line| {
        let chars: Vec<char> = line.chars().collect();
        if chars.is_empty() { vec![String::new()] } else { chars.chunks(limit).map(|c| c.iter().collect()).collect() }
    });
    for (i, line) in lines.enumerate() {
        cr.move_to(0.0, size * (1.0 + i as f64 * 1.3));
        cr.show_text(&line)?;
    }
    Ok(())
}

/// PNG or JPEG as a cairo surface.
fn load_picture(path: &Path) -> Result<cairo::ImageSurface> {
    let picture =
        image::open(path).with_context(|| format!("Bild {} kann nicht geladen werden", path.display()))?.to_rgba8();
    let (w, h) = picture.dimensions();
    let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, w as i32, h as i32)?;
    let stride = surface.stride() as usize;
    {
        let mut data = surface.data().context("cairo surface data")?;
        for (y, row) in picture.rows().enumerate() {
            for (x, px) in row.enumerate() {
                let [r, g, b, a] = px.0;
                let pre = |c: u8| ((u16::from(c) * u16::from(a) + 127) / 255) as u8;
                let i = y * stride + x * 4;
                data[i..i + 4].copy_from_slice(&[pre(b), pre(g), pre(r), a]);
            }
        }
    }
    surface.mark_dirty();
    Ok(surface)
}

fn tempfile_dir(parent: &Path) -> Result<PathBuf> {
    for _ in 0..16 {
        let dir = parent.join(format!(".extract-{}", uuid::Uuid::new_v4().simple()));
        if std::fs::create_dir(&dir).is_ok() {
            return Ok(dir);
        }
    }
    bail!("Cache-Ordner kann nicht angelegt werden")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn archive(dir: &Path, json: &str, files: &[(&str, &[u8])]) -> PathBuf {
        let path = dir.join("Test.atlasnote");
        let mut zip = zip::ZipWriter::new(File::create(&path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("notebook.json", options).unwrap();
        zip.write_all(json.as_bytes()).unwrap();
        for (name, bytes) in files {
            zip.start_file(*name, options).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
        path
    }

    fn png(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        image::RgbaImage::from_pixel(w, h, image::Rgba(rgba)).write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn renders_paper_strokes_text_and_placed_images() {
        let dir = tempfile::tempdir().unwrap();
        // SAFETY: tests set the cache location before any thread reads it.
        unsafe { std::env::set_var("XDG_CACHE_HOME", dir.path().join("cache")) };
        let json = r#"{"version":3,"favorites":[],"pages":[{"id":"p","paper":"GRID","dark":false,"height":1414,"marks":[
            {"id":"s","style":{"tool":"PEN","color":-16776961,"width":6},"rotation":0,"points":[[1,100,100,0,0,0.8,0,0],[1,400,100,1,0,0.8,0,0]]},
            {"id":"h","style":{"tool":"HIGHLIGHTER","color":-10923,"width":22},"rotation":0,"points":[[1,100,300,0,0,-1,-1,-1],[1,600,300,1,0,-1,-1,-1]]},
            {"id":"t","style":{"tool":"TEXT","color":-16777216,"width":24},"text":"Hallo","rotation":0,"points":[[1,100,500,0,0,-1,-1,-1]]},
            {"id":"i","style":{"tool":"PEN","color":-16777216,"width":3},"image":"pic-1.png","rotation":0,
             "points":[[1,500,700,0,0,-1,-1,-1],[1,900,700,1,0,-1,-1,-1],[1,500,900,2,0,-1,-1,-1]]}]}]}"#;
        let path = archive(dir.path(), json, &[("images/pic-1.png", &png(4, 2, [255, 0, 0, 255]))]);
        let note = NoteFile::open(&path).unwrap();
        assert_eq!(note.notebook.pages.len(), 1);
        let (w, h, rgba) = note.render(0, 1000).unwrap();
        assert_eq!((w, h), (1000, 1414));
        let px = |x: u32, y: u32| {
            let i = ((y * w + x) * 4) as usize;
            [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
        };
        assert_eq!(px(250, 100), [0, 0, 255, 255], "blue pen stroke");
        assert_eq!(px(700, 800), [255, 0, 0, 255], "placed picture spans its corners");
        assert_eq!(px(950, 1000), [0xff, 0xfd, 0xf7, 255], "light paper outside the grid lines");
        let marker = px(350, 300);
        assert!(
            marker[0] > 200 && marker[2] < 240 && marker[2] > 150,
            "translucent highlighter over paper: {marker:?}"
        );
        assert!((480..560).any(|y| (100..200).any(|x| px(x, y)[0] < 100)), "text is drawn");
        let file = note.render_png(0, 800).unwrap();
        assert!(file.is_file());
        assert_eq!(image::image_dimensions(&file).unwrap(), (800, 1132));
    }

    #[test]
    fn rejects_paths_outside_known_folders_and_unknown_versions() {
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("XDG_CACHE_HOME", dir.path().join("cache")) };
        let ok = r#"{"version":2,"pages":[{"id":"p","paper":"BLANK","dark":true,"height":1414,"marks":[]}]}"#;
        let evil = archive(dir.path(), ok, &[("../evil.png", b"x")]);
        assert!(NoteFile::open(&evil).is_err());
        assert!(!dir.path().join("cache/evil.png").exists());
        assert!(Notebook::parse(&ok.replace("\"version\":2", "\"version\":9")).is_err());
        assert!(Notebook::parse(&ok.replace("\"marks\":[]", "\"image\":\"../x.png\",\"marks\":[]")).is_err());
        assert!(valid_source("images/abc-1.jpg") && !valid_source("images/a/b.png") && !valid_source("pdf/.pdf"));
        assert_eq!(paper_start(48.0, 0.0), 20.0);
    }
}
