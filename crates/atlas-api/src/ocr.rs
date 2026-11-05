// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Formula recognition (LaTeX OCR, ONNX models via `ort`). The UI renders
//! the selected area (PDFKit) and hands in the pixels. Template:
//! `formula_ocr.rs`, core `latex_ocr.rs`.

use std::path::Path;

use atlas_core::latex_ocr;

use crate::util::background;
use crate::{AtlasError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct OcrStatus {
    /// Models are installed (else the first recognition downloads them).
    pub models: bool,
    /// The ONNX runtime library was found (`ORT_DYLIB_PATH`, bundle);
    /// true when it is looked up by name and cannot be checked before use.
    pub runtime: bool,
    /// Models are loaded (the next recognition is fast).
    pub warm: bool,
    pub models_dir: String,
    pub message: String,
}

/// Greyscale image, one byte per pixel, rows without padding.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct GrayImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

#[cfg_attr(feature = "ffi", derive(uniffi::Object))]
pub struct LatexOcr {}

/// Where ONNX Runtime is loaded from (as `latex_ocr` does).
fn runtime_library() -> String {
    std::env::var("ORT_DYLIB_PATH")
        .ok()
        .filter(|p| !p.is_empty())
        .or_else(|| option_env!("ATLAS_ORT_DYLIB").map(str::to_owned))
        .unwrap_or_else(|| atlas_core::platform::ONNX_RUNTIME.into())
}

#[cfg_attr(feature = "ffi", uniffi::export)]
impl LatexOcr {
    #[cfg_attr(feature = "ffi", uniffi::constructor)]
    pub fn new() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self {})
    }

    pub fn status(&self) -> OcrStatus {
        let dir = latex_ocr::model_dir();
        let models = latex_ocr::models_present(&dir);
        let warm = latex_ocr::is_running();
        let library = runtime_library();
        // A bare name is found by the dynamic loader: only checkable by loading.
        let runtime = warm || !Path::new(&library).is_absolute() || Path::new(&library).exists();
        let message = if !runtime {
            format!("ONNX Runtime nicht gefunden ({library})")
        } else if !models {
            "Das Formelmodell wird beim ersten Erkennen geladen (einmalig, etwa 100 MB).".to_owned()
        } else if warm {
            "Formelerkennung bereit".to_owned()
        } else {
            "Formelerkennung bereit (das erste Erkennen lädt das Modell)".to_owned()
        };
        OcrStatus { models, runtime, warm, models_dir: dir.to_string_lossy().into_owned(), message }
    }

    /// Loads the models in the background so the first request is quick
    /// (only if they are installed: downloading waits for a request).
    pub fn warm_up(&self) {
        let dir = latex_ocr::model_dir();
        if latex_ocr::is_running() || !latex_ocr::models_present(&dir) {
            return;
        }
        std::thread::spawn(|| {
            // A blank image loads the engine; its result does not matter.
            let _ = latex_ocr::recognize_gray(8, 8, vec![255; 64]);
        });
    }

    /// LaTeX (tidied) of the formula in `image` (rendered at about 2× of
    /// 72 dpi, on white). Downloads the models the first time.
    pub async fn recognize(&self, image: GrayImage) -> Result<String> {
        if image.width == 0 || image.height == 0 || image.pixels.len() != (image.width * image.height) as usize {
            return Err(AtlasError::Invalid { message: "Bildgröße passt nicht zu den Pixeln".into() });
        }
        background(move || {
            let dir = latex_ocr::model_dir();
            if !latex_ocr::models_present(&dir) {
                latex_ocr::download_models(&dir, &|_, _, _| {})
                    .map_err(|e| AtlasError::Unavailable { message: format!("Formelmodell: {e:#}") })?;
            }
            let latex = latex_ocr::recognize_gray(image.width, image.height, image.pixels)
                .map_err(|e| AtlasError::Failed { message: format!("{e:#}") })?;
            Ok(atlas_core::pdf::formulas::tidy(&latex))
        })
        .await
    }
}
