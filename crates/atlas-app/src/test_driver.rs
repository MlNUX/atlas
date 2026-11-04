// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Driving the window from QML like a user (QML singleton `TestDriver`):
//! mouse, wheel and keys for the smoke test (`--self-test`), a picture of
//! the window for `--screenshot`. Nothing of the app itself uses it.

use cxx_qt_lib::QString;

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
        type TestDriver = super::TestDriverRust;

        /// Saves a PNG of `window` (a QQuickWindow).
        #[qinvokable]
        #[cxx_name = "saveWindowImage"]
        unsafe fn save_window_image(self: &TestDriver, window: *mut QObject, path: &QString) -> bool;

        /// Sends a wheel event at window coordinates.
        #[qinvokable]
        #[cxx_name = "sendWheel"]
        unsafe fn send_wheel(self: &TestDriver, window: *mut QObject, x: f64, y: f64, delta: i32, flags: i32);

        /// Sends a key press and release (Qt key code, modifier flags) to
        /// the window's focus item.
        #[qinvokable]
        #[cxx_name = "sendKey"]
        unsafe fn send_key(self: &TestDriver, window: *mut QObject, key: i32, modifiers: i32);

        /// Sends a mouse event (0 press, 1 move, 2 release; +10 with Ctrl)
        /// at window coordinates after `delay_ms`.
        #[qinvokable]
        #[cxx_name = "postMouse"]
        unsafe fn post_mouse(self: &TestDriver, window: *mut QObject, kind: i32, x: f64, y: f64, delay_ms: i32);
    }
}

#[derive(Default)]
pub struct TestDriverRust;

impl qobject::TestDriver {
    /// # Safety
    /// `window` must be null or point to a live QObject (QML passes the
    /// window object).
    pub unsafe fn save_window_image(&self, window: *mut cxx_qt::QObject, path: &QString) -> bool {
        unsafe { crate::qt_helpers::save_window_image(window, path) }
    }

    /// # Safety
    /// As for `save_window_image`.
    pub unsafe fn send_wheel(&self, window: *mut cxx_qt::QObject, x: f64, y: f64, delta: i32, flags: i32) {
        unsafe { crate::qt_helpers::send_wheel(window, x, y, delta, flags) }
    }

    /// # Safety
    /// As for `save_window_image`.
    pub unsafe fn send_key(&self, window: *mut cxx_qt::QObject, key: i32, modifiers: i32) {
        unsafe { crate::qt_helpers::send_key(window, key, modifiers) }
    }

    /// # Safety
    /// As for `save_window_image`.
    pub unsafe fn post_mouse(&self, window: *mut cxx_qt::QObject, kind: i32, x: f64, y: f64, delay_ms: i32) {
        unsafe { crate::qt_helpers::post_mouse(window, kind, x, y, delay_ms) }
    }
}
