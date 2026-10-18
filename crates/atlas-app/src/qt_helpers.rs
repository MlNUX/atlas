// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bindings for the hand written helpers in `cpp/helpers.{h,cpp}`.

#[cxx::bridge(namespace = "atlas")]
pub mod ffi {
    unsafe extern "C++" {
        include!("atlas-app/cpp/helpers.h");
        include!("cxx-qt-lib/qfont.h");
        include!("cxx-qt-lib/qstring.h");

        #[namespace = ""]
        type QFont = cxx_qt_lib::QFont;
        #[namespace = ""]
        type QString = cxx_qt_lib::QString;
        #[namespace = ""]
        type QObject = cxx_qt::QObject;

        #[cxx_name = "fontCellWidth"]
        fn font_cell_width(font: &QFont) -> f64;
        #[cxx_name = "fontLineHeight"]
        fn font_line_height(font: &QFont) -> f64;
        #[cxx_name = "fontAscent"]
        fn font_ascent(font: &QFont) -> f64;
        #[cxx_name = "clipboardSetText"]
        fn clipboard_set_text(text: &QString);
        #[cxx_name = "clipboardText"]
        fn clipboard_text() -> QString;
        #[cxx_name = "saveWindowImage"]
        unsafe fn save_window_image(window: *mut QObject, path: &QString) -> bool;
    }
}

pub use ffi::*;
