// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Formula recognition (LaTeX OCR, ONNX models via `ort`). The UI renders
//! the selected area (PDFKit) and hands in the pixels. Template:
//! `formula_ocr.rs`, core `latex_ocr.rs`.

use crate::{Result, todo_task};

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct OcrStatus {
    /// Models are installed.
    pub models: bool,
    /// The ONNX runtime library was found (`ORT_DYLIB_PATH`, bundle).
    pub runtime: bool,
    /// Models are loaded (the first recognition is fast).
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

#[cfg_attr(feature = "ffi", uniffi::export)]
impl LatexOcr {
    #[cfg_attr(feature = "ffi", uniffi::constructor)]
    pub fn new() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self {})
    }

    pub fn status(&self) -> OcrStatus {
        todo_task!("P2.6")
    }

    /// Loads the models in the background so the first request is quick.
    pub fn warm_up(&self) {
        todo_task!("P2.6")
    }

    /// LaTeX of the formula in `image` (rendered at about 2× of 72 dpi, on white).
    pub async fn recognize(&self, image: GrayImage) -> Result<String> {
        let _ = image;
        todo_task!("P2.6")
    }
}
