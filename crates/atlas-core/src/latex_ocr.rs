// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Formulas from PDFs as LaTeX, built in: the pix2tex model
//! ([LaTeX-OCR](https://github.com/lukas-blecher/LaTeX-OCR)) in the ONNX
//! export of [RapidLaTeXOCR](https://github.com/RapidAI/RapidLaTeXOCR), run
//! with ONNX Runtime – no Python, nothing leaves the machine.
//!
//! - The model files (about 100 MB) are downloaded once on first use into
//!   `$XDG_DATA_HOME/atlas/latex-ocr/` (`ATLAS_LATEX_OCR_MODELS` replaces
//!   the folder, for tests).
//! - ONNX Runtime is loaded at run time: `ORT_DYLIB_PATH`, else the library
//!   the package was built with (`ATLAS_ORT_DYLIB`), else
//!   `libonnxruntime.so` from the system.
//! - The steps follow RapidLaTeXOCR (`rapid_latex_ocr` 0.0.9): crop and pad
//!   the picture, let the resizer model pick the width the encoder was
//!   trained for, encode, then decode token by token (greedy; RapidLaTeXOCR
//!   samples at temperature 1e-5, which is the same) and tidy the LaTeX.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use anyhow::{Context, Result, bail};
use image::imageops::FilterType;
use image::{GrayImage, Luma};
use ort::session::Session;
use ort::session::builder::GraphOptimizationLevel;
use ort::value::Tensor;

/// Where the models are published.
const MODEL_URL: &str = "https://github.com/RapidAI/RapidLaTeXOCR/releases/download/v0.0.0/";
pub const MODEL_FILES: [&str; 4] = ["image_resizer.onnx", "encoder.onnx", "decoder.onnx", "tokenizer.json"];

const MAX_DIMS: (u32, u32) = (672, 192);
const MIN_DIMS: (u32, u32) = (32, 32);
const BOS: i64 = 1;
const EOS: i64 = 2;
const MAX_SEQ_LEN: usize = 512;
const MEAN: f32 = 0.7931;
const STD: f32 = 0.1738;

/// The folder with the model files.
pub fn model_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("ATLAS_LATEX_OCR_MODELS") {
        return PathBuf::from(dir);
    }
    dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("atlas/latex-ocr")
}

/// All model files are there.
pub fn models_present(dir: &Path) -> bool {
    MODEL_FILES.iter().all(|f| dir.join(f).is_file())
}

/// Downloads the missing model files; `progress(file, received, total)`.
pub fn download_models(dir: &Path, progress: &dyn Fn(&str, u64, Option<u64>)) -> Result<()> {
    use std::io::{Read, Write};
    std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    let agent = ureq::Agent::config_builder()
        .tls_config(ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier).build())
        .user_agent("Atlas (LaTeX-OCR)")
        .build()
        .new_agent();
    for file in MODEL_FILES {
        let target = dir.join(file);
        if target.is_file() {
            continue;
        }
        let url = format!("{MODEL_URL}{file}");
        let mut res = agent.get(&url).call().with_context(|| format!("Download von {url}"))?;
        let total = res.headers().get("content-length").and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok());
        let part = dir.join(format!(".{file}.part"));
        let mut out = std::fs::File::create(&part)?;
        let mut body = res.body_mut().with_config().limit(1 << 30).reader();
        let mut buf = vec![0u8; 256 * 1024];
        let mut received = 0u64;
        loop {
            let n = match body.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => {
                    let _ = std::fs::remove_file(&part);
                    return Err(e).with_context(|| format!("Download von {file} abgebrochen"));
                }
            };
            out.write_all(&buf[..n])?;
            received += n as u64;
            progress(file, received, total);
        }
        out.sync_all()?;
        drop(out);
        if total.is_some_and(|t| t != received) {
            let _ = std::fs::remove_file(&part);
            bail!("{file} unvollständig ({received} von {} Bytes)", total.unwrap_or(0));
        }
        std::fs::rename(&part, &target)?;
    }
    Ok(())
}

// ------------------------------------------------------------- the picture

/// Crops to the ink (dark on light or the other way round), makes it dark
/// on white and pads it with white to multiples of 32 (`PreProcess.pad`).
fn pad(img: &GrayImage) -> Result<GrayImage> {
    let (min, max) = img.pixels().fold((255u8, 0u8), |(lo, hi), p| (lo.min(p[0]), hi.max(p[0])));
    if max == min {
        bail!("Im Bereich ist nichts zu erkennen");
    }
    let scale = 255.0 / f32::from(max - min);
    let norm: Vec<f32> = img.pixels().map(|p| f32::from(p[0] - min) * scale).collect();
    let mean = norm.iter().sum::<f32>() / norm.len() as f32;
    // Text becomes dark on white; `ink` marks it.
    let (data, ink): (Vec<f32>, Vec<bool>) = if mean > 128.0 {
        (norm.clone(), norm.iter().map(|&v| v < 128.0).collect())
    } else {
        (norm.iter().map(|&v| 255.0 - v).collect(), norm.iter().map(|&v| v > 128.0).collect())
    };
    let (w, h) = img.dimensions();
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for y in 0..h {
        for x in 0..w {
            if ink[(y * w + x) as usize] {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    if x1 < x0 {
        bail!("Im Bereich ist nichts zu erkennen");
    }
    let (cw, ch) = (x1 - x0 + 1, y1 - y0 + 1);
    let round_up = |v: u32| v.div_ceil(32) * 32;
    let mut out = GrayImage::from_pixel(round_up(cw), round_up(ch), Luma([255]));
    for y in 0..ch {
        for x in 0..cw {
            let v = data[((y + y0) * w + x + x0) as usize];
            out.put_pixel(x, y, Luma([v.clamp(0.0, 255.0) as u8]));
        }
    }
    Ok(out)
}

/// Fits into the model's size: scaled down if too big, padded with white
/// if too small (`PreProcess.minmax_size`).
fn minmax_size(img: GrayImage) -> GrayImage {
    let (w, h) = img.dimensions();
    let ratio = (f64::from(w) / f64::from(MAX_DIMS.0)).max(f64::from(h) / f64::from(MAX_DIMS.1));
    let img = if ratio > 1.0 {
        let nw = ((f64::from(w) / ratio).floor() as u32).max(1);
        let nh = ((f64::from(h) / ratio).floor() as u32).max(1);
        image::imageops::resize(&img, nw, nh, FilterType::Triangle)
    } else {
        img
    };
    let (w, h) = img.dimensions();
    if w >= MIN_DIMS.0 && h >= MIN_DIMS.1 {
        return img;
    }
    let mut out = GrayImage::from_pixel(w.max(MIN_DIMS.0), h.max(MIN_DIMS.1), Luma([255]));
    image::imageops::overlay(&mut out, &img, 0, 0);
    out
}

/// `[1, 1, H, W]`, normalized like in training.
fn to_tensor(img: &GrayImage) -> Result<Tensor<f32>> {
    let (w, h) = img.dimensions();
    let data: Vec<f32> = img.pixels().map(|p| (f32::from(p[0]) / 255.0 - MEAN) / STD).collect();
    Ok(Tensor::from_array(([1usize, 1, h as usize, w as usize], data))?)
}

fn argmax(values: &[f32]) -> usize {
    values.iter().enumerate().fold((0, f32::NEG_INFINITY), |best, (i, &v)| if v > best.1 { (i, v) } else { best }).0
}

// ------------------------------------------------------------- the model

struct Engine {
    resizer: Session,
    encoder: Session,
    decoder: Session,
    /// Token id → text.
    vocab: Vec<String>,
}

static ORT: OnceLock<std::result::Result<ort::environment::Environment, String>> = OnceLock::new();

/// ONNX Runtime, loaded once.
fn environment() -> Result<&'static ort::environment::Environment> {
    let env = ORT.get_or_init(|| {
        let lib = std::env::var("ORT_DYLIB_PATH")
            .ok()
            .filter(|p| !p.is_empty())
            .or_else(|| option_env!("ATLAS_ORT_DYLIB").map(str::to_owned))
            .unwrap_or_else(|| "libonnxruntime.so".into());
        ort::init_from(&lib)
            .map_err(|e| format!("ONNX Runtime ({lib}) lässt sich nicht laden: {e}"))?
            .with_name("atlas")
            .build()
            .map_err(|e| format!("ONNX Runtime: {e}"))
    });
    env.as_ref().map_err(|e| anyhow::anyhow!("{e}"))
}

impl Engine {
    fn load(dir: &Path) -> Result<Engine> {
        let env = environment()?;
        let open = |file: &str| -> Result<Session> {
            let path = dir.join(file);
            Session::builder(env)
                .map_err(|e| anyhow::anyhow!("{e}"))?
                .with_optimization_level(GraphOptimizationLevel::Level3)
                .map_err(|e| anyhow::anyhow!("{e}"))?
                .with_intra_threads(std::thread::available_parallelism().map(|n| n.get().min(4)).unwrap_or(2))
                .map_err(|e| anyhow::anyhow!("{e}"))?
                .commit_from_file(&path)
                .map_err(|e| anyhow::anyhow!("{} lässt sich nicht laden: {e}", path.display()))
        };
        Ok(Engine {
            resizer: open("image_resizer.onnx")?,
            encoder: open("encoder.onnx")?,
            decoder: open("decoder.onnx")?,
            vocab: load_vocab(&dir.join("tokenizer.json"))?,
        })
    }

    /// The picture at the width the encoder knows: up to ten rounds of
    /// "resize, then ask the resizer model which width fits"
    /// (`loop_image_resizer`).
    fn fit(&mut self, img: &GrayImage) -> Result<GrayImage> {
        let input = minmax_size(pad(img)?);
        let (mut w, mut h) = input.dimensions();
        let mut r = 1.0f64;
        let mut fitted = None;
        for _ in 0..10 {
            h = ((f64::from(h) * r) as u32).max(1);
            let filter = if r > 1.0 { FilterType::Triangle } else { FilterType::Lanczos3 };
            let resized = image::imageops::resize(&input, w.max(1), h, filter);
            let padded = pad(&minmax_size(resized))?;
            let outputs = self.resizer.run(ort::inputs![to_tensor(&padded)?]).map_err(|e| anyhow::anyhow!("{e}"))?;
            let (_, logits) = outputs[0].try_extract_tensor::<f32>().map_err(|e| anyhow::anyhow!("{e}"))?;
            w = (argmax(logits) as u32 + 1) * 32;
            let done = w == padded.width();
            r = f64::from(w) / f64::from(padded.width());
            fitted = Some(padded);
            if done {
                break;
            }
        }
        fitted.context("kein Bild")
    }

    fn recognize(&mut self, img: &GrayImage) -> Result<String> {
        let fitted = self.fit(img)?;
        let outputs = self.encoder.run(ort::inputs![to_tensor(&fitted)?]).map_err(|e| anyhow::anyhow!("{e}"))?;
        let (shape, context) = outputs[0].try_extract_tensor::<f32>().map_err(|e| anyhow::anyhow!("{e}"))?;
        let context_shape: Vec<i64> = shape.to_vec();
        let context = context.to_vec();
        drop(outputs);
        let mut tokens = vec![BOS];
        for _ in 0..MAX_SEQ_LEN {
            let window = &tokens[tokens.len().saturating_sub(MAX_SEQ_LEN)..];
            let t = window.len();
            let x = Tensor::from_array(([1usize, t], window.to_vec())).map_err(|e| anyhow::anyhow!("{e}"))?;
            let mask = Tensor::from_array(([1usize, t], vec![true; t])).map_err(|e| anyhow::anyhow!("{e}"))?;
            let ctx =
                Tensor::from_array((context_shape.clone(), context.clone())).map_err(|e| anyhow::anyhow!("{e}"))?;
            let outputs = self.decoder.run(ort::inputs![x, mask, ctx]).map_err(|e| anyhow::anyhow!("{e}"))?;
            let (shape, logits) = outputs[0].try_extract_tensor::<f32>().map_err(|e| anyhow::anyhow!("{e}"))?;
            let vocab = *shape.last().context("logits")? as usize;
            let last = &logits[logits.len() - vocab..];
            let next = argmax(last) as i64;
            tokens.push(next);
            if next == EOS {
                break;
            }
        }
        Ok(post_process(&decode(&self.vocab, &tokens[1..])))
    }
}

/// The vocabulary of `tokenizer.json` (`model.vocab`: text → id).
fn load_vocab(path: &Path) -> Result<Vec<String>> {
    let text = std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    let json: serde_json::Value = serde_json::from_str(&text)?;
    let map = json["model"]["vocab"].as_object().context("tokenizer.json ohne model.vocab")?;
    let mut vocab = vec![String::new(); map.len()];
    for (token, id) in map {
        let id = id.as_u64().context("Token-ID")? as usize;
        if id >= vocab.len() {
            vocab.resize(id + 1, String::new());
        }
        vocab[id] = token.clone();
    }
    Ok(vocab)
}

/// Tokens to text like `TokenizerCls.token2str`: joined, `Ġ` is a space,
/// special tokens dropped.
fn decode(vocab: &[String], tokens: &[i64]) -> String {
    let text: String = tokens
        .iter()
        .filter_map(|&t| vocab.get(t as usize))
        .filter(|t| !matches!(t.as_str(), "[PAD]" | "[BOS]" | "[EOS]"))
        .map(|t| t.replace(' ', ""))
        .collect();
    text.replace('Ġ', " ").trim().to_owned()
}

/// Removes spaces that change nothing (`LaTeXOCR.post_process`): inside
/// `\operatorname{…}`, `\mathrm{…}`, `\text{…}`, `\mathbf{…}`, and between
/// two non-letters or a letter and a non-letter – not after `\` (an
/// explicit space) and never between two letters (`\alpha b`).
pub fn post_process(s: &str) -> String {
    let named = regex::Regex::new(r"(\\(operatorname|mathrm|text|mathbf)\s?\*? \{.*?\})").expect("regex");
    let s = named.replace_all(s, |c: &regex::Captures| c[0].replace(' ', "")).into_owned();
    let letter = |c: char| c.is_ascii_alphabetic();
    // Python's `[\W_^\d]`: neither a word character nor … i.e. not a letter
    // in the sense of `\w`, or `_`, `^`, a digit.
    let noletter = |c: char| !(c.is_alphanumeric() || c == '_') || c == '_' || c == '^' || c.is_ascii_digit();
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() && !out.is_empty() {
            let mut j = i;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            let prev = out.chars().last().unwrap_or(' ');
            let next = chars.get(j).copied();
            // `\ ` is a space on purpose.
            let escaped = prev == '\\';
            let drop = match next {
                Some(n) if !escaped => {
                    (noletter(prev) && noletter(n)) || (noletter(prev) && letter(n)) || (letter(prev) && noletter(n))
                }
                _ => false,
            };
            if drop {
                i = j;
                continue;
            }
            out.extend(&chars[i..j]);
            i = j;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out.trim().to_owned()
}

static ENGINE: Mutex<Option<Engine>> = Mutex::new(None);

/// Whether the model is loaded (later recognitions are quick).
pub fn is_running() -> bool {
    ENGINE.lock().map(|e| e.is_some()).unwrap_or(false)
}

/// LaTeX of the formula in the greyscale picture `img` (models from
/// `dir`, which must be complete).
pub fn recognize_in(dir: &Path, img: &GrayImage) -> Result<String> {
    let mut slot = ENGINE.lock().unwrap_or_else(|e| e.into_inner());
    if slot.is_none() {
        *slot = Some(Engine::load(dir)?);
    }
    slot.as_mut().context("kein Modell")?.recognize(img)
}

/// LaTeX of the formula in a greyscale picture (`w × h`, one byte per
/// pixel), with the models of [`model_dir`].
pub fn recognize_gray(w: u32, h: u32, pixels: Vec<u8>) -> Result<String> {
    let img = GrayImage::from_raw(w, h, pixels).context("Bildgröße passt nicht")?;
    recognize(&img)
}

/// LaTeX of the formula in `img`, with the models of [`model_dir`].
pub fn recognize(img: &GrayImage) -> Result<String> {
    let dir = model_dir();
    if !models_present(&dir) {
        bail!("Das Formelmodell fehlt in {}", dir.display());
    }
    recognize_in(&dir, img)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The test models (scripts/make-latex-ocr-fixtures.py): the decoder
    /// spells `\frac{a}{b}` whatever the picture.
    fn fixtures() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/latex-ocr")
    }

    /// A dark bar on white, like a formula.
    fn formula(w: u32, h: u32) -> GrayImage {
        let mut img = GrayImage::from_pixel(w, h, Luma([250]));
        for y in h / 3..2 * h / 3 {
            for x in w / 8..7 * w / 8 {
                img.put_pixel(x, y, Luma([20]));
            }
        }
        img
    }

    #[test]
    fn pictures_are_cropped_and_padded() {
        let img = pad(&formula(200, 60)).unwrap();
        // Ink 150 × 20 → padded to 160 × 32, dark on white.
        assert_eq!(img.dimensions(), (160, 32));
        assert!(img.get_pixel(0, 0)[0] < 30 && img.get_pixel(159, 31)[0] == 255);
        // Light on dark is turned round.
        let mut dark = formula(200, 60);
        image::imageops::invert(&mut dark);
        assert_eq!(pad(&dark).unwrap().get_pixel(0, 0)[0], img.get_pixel(0, 0)[0]);
        assert!(pad(&GrayImage::from_pixel(50, 50, Luma([255]))).is_err(), "nothing to see");
        // Too wide: scaled into 672 × 192; too small: padded to 32.
        assert_eq!(minmax_size(GrayImage::new(1344, 96)).dimensions(), (672, 48));
        assert_eq!(minmax_size(GrayImage::new(20, 10)).dimensions(), (32, 32));
    }

    #[test]
    fn latex_is_tidied() {
        assert_eq!(post_process("\\frac { a } { b } ^ { 2 }"), "\\frac{a}{b}^{2}");
        assert_eq!(post_process("\\alpha b + c"), "\\alpha b+c");
        assert_eq!(post_process("\\mathrm { d } x"), "\\mathrm{d}x");
        assert_eq!(post_process("a \\ b"), "a\\ b", "an explicit space stays");
        let vocab = ["[BOS]".to_owned(), "x".into(), "Ġ+".into(), "[EOS]".into()];
        assert_eq!(decode(&vocab, &[1, 2, 3]), "x +", "Ġ is a space, [EOS] goes");
    }

    #[test]
    fn recognizes_with_the_onnx_models() {
        if environment().is_err() {
            // ONNX Runtime comes with the package (Nix: ATLAS_ORT_DYLIB).
            eprintln!("skipped: {}", environment().err().map(|e| e.to_string()).unwrap_or_default());
            return;
        }
        let dir = fixtures();
        assert!(models_present(&dir));
        assert_eq!(recognize_in(&dir, &formula(300, 80)).unwrap(), "\\frac{a}{b}");
        assert!(is_running(), "the model stays loaded");
        assert_eq!(recognize_in(&dir, &formula(120, 50)).unwrap(), "\\frac{a}{b}");
    }
}
