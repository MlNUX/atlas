// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Small helpers shared by the QObjects.

use cxx_qt_lib::QString;

/// A `QString` from Rust text.
pub fn qs(s: impl AsRef<str>) -> QString {
    QString::from(s.as_ref())
}
