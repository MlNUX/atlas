// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bindings for the hand written helpers in `cpp/helpers.{h,cpp}`.

#[cxx::bridge(namespace = "atlas")]
pub mod ffi {
    unsafe extern "C++" {
        include!("atlas-app/cpp/helpers.h");
        include!("cxx-qt-lib/qfont.h");
        include!("cxx-qt-lib/qstring.h");
        include!("cxx-qt-lib/qstringlist.h");

        #[namespace = ""]
        type QFont = cxx_qt_lib::QFont;
        #[namespace = ""]
        type QString = cxx_qt_lib::QString;
        #[namespace = ""]
        type QStringList = cxx_qt_lib::QStringList;
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
        #[cxx_name = "clipboardSetFiles"]
        fn clipboard_set_files(paths: &QStringList, cut: bool);
        #[cxx_name = "clipboardFiles"]
        fn clipboard_files() -> QStringList;
        #[cxx_name = "clipboardFilesCut"]
        fn clipboard_files_cut() -> bool;
        #[cxx_name = "initWebEngine"]
        fn init_web_engine();
        #[cxx_name = "installWebSchemeHandler"]
        fn install_web_scheme_handler();
        #[cxx_name = "saveWindowImage"]
        unsafe fn save_window_image(window: *mut QObject, path: &QString) -> bool;
        #[cxx_name = "sendWheel"]
        unsafe fn send_wheel(window: *mut QObject, x: f64, y: f64, delta: i32, flags: i32);
        #[cxx_name = "sendKey"]
        unsafe fn send_key(window: *mut QObject, key: i32, modifiers: i32);
        #[cxx_name = "watchTouchpadHold"]
        fn watch_touchpad_hold(on_hold: fn()) -> bool;
        #[cxx_name = "setOwner"]
        unsafe fn set_owner(object: *mut QObject, owner: *mut QObject);
        #[cxx_name = "imageFormats"]
        fn image_formats() -> QStringList;
        #[cxx_name = "watchWebCookies"]
        unsafe fn watch_web_cookies(profile: *mut QObject) -> bool;
        #[cxx_name = "webProfilePath"]
        fn web_profile_path() -> QString;
        #[cxx_name = "webCookieHeader"]
        fn web_cookie_header(url: &QString) -> QString;
        #[cxx_name = "webCookieSummary"]
        fn web_cookie_summary() -> QString;
        #[cxx_name = "webUserAgent"]
        fn web_user_agent() -> QString;
        #[cxx_name = "clearWebCookies"]
        fn clear_web_cookies();
        #[cxx_name = "postMouse"]
        unsafe fn post_mouse(window: *mut QObject, kind: i32, x: f64, y: f64, delay_ms: i32);
    }
}

pub use ffi::*;
