// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Formulas in PDFs as LaTeX for QML (singleton `FormulaOcr`): the built-in
//! pix2tex model (atlas_core::latex_ocr), downloaded on first use.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use atlas_core::latex_ocr;
use cxx_qt::Threading;
use cxx_qt_lib::QString;

use crate::util::qs;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        type FormulaOcr = super::FormulaOcrRust;

        /// Recognizes the formula in `area` (JSON `[x0, y0, x1, y1]`,
        /// points) of `page` (0-based) of PDF `path` as LaTeX, in the
        /// background; then `recognized` with `request`.
        #[qinvokable]
        fn recognize(self: &FormulaOcr, request: i32, path: &QString, page: i32, area: &QString);

        /// What the recognition of `request` is doing (the first time:
        /// downloading the model).
        #[qsignal]
        fn progress(self: Pin<&mut FormulaOcr>, request: i32, text: QString);

        /// `latex` (tidied) or `error` for `request`.
        #[qsignal]
        fn recognized(self: Pin<&mut FormulaOcr>, request: i32, latex: QString, error: QString);

        /// Whether the model is loaded already (later recognitions are
        /// quick).
        #[qinvokable]
        fn warm(self: &FormulaOcr) -> bool;
    }

    impl cxx_qt::Threading for FormulaOcr {}
}

#[derive(Default)]
pub struct FormulaOcrRust;

impl qobject::FormulaOcr {
    pub fn recognize(&self, request: i32, path: &QString, page: i32, area: &QString) {
        let thread = self.qt_thread();
        let Ok([x0, y0, x1, y1]) = serde_json::from_str::<[f64; 4]>(&area.to_string()) else {
            log::warn!("FormulaOcr.recognize: bad area {area}");
            return;
        };
        atlas_core::pdf::PdfService::get().formula_clip(
            PathBuf::from(path.to_string()),
            page.max(0) as usize,
            [x0, y0, x1, y1],
            move |result| {
                // The recognition takes a while: not on the PDF thread.
                std::thread::spawn(move || {
                    let outcome = result.and_then(|(w, h, pixels)| {
                        ensure_models(&|text| {
                            let _ = thread.queue(move |o| o.progress(request, qs(text)));
                        })?;
                        latex_ocr::recognize_gray(w, h, pixels).map(|t| atlas_core::pdf::formulas::tidy(&t))
                    });
                    let (latex, error) = match outcome {
                        Ok(l) => (l, String::new()),
                        Err(e) => (String::new(), format!("{e:#}")),
                    };
                    let _ = thread.queue(move |o| o.recognized(request, qs(latex), qs(error)));
                });
            },
        );
    }

    pub fn warm(&self) -> bool {
        latex_ocr::is_running()
    }
}

/// Downloads the formula model the first time (see
/// [`atlas_core::latex_ocr`]), telling `progress` about it now and then.
pub(crate) fn ensure_models(progress: &dyn Fn(String)) -> anyhow::Result<()> {
    let dir = latex_ocr::model_dir();
    if latex_ocr::models_present(&dir) {
        return Ok(());
    }
    let last = std::sync::Mutex::new(Instant::now() - Duration::from_secs(1));
    latex_ocr::download_models(&dir, &|file, received, total| {
        let mut last = last.lock().unwrap_or_else(|e| e.into_inner());
        if last.elapsed() < Duration::from_millis(250) {
            return;
        }
        *last = Instant::now();
        let mb = |b: u64| b as f64 / 1_048_576.0;
        progress(match total {
            Some(t) => format!("Lade das Formelmodell (einmalig): {file} {:.0} von {:.0} MB", mb(received), mb(t)),
            None => format!("Lade das Formelmodell (einmalig): {file} {:.0} MB", mb(received)),
        });
    })
}
