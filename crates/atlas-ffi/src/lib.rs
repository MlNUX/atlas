// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! UniFFI export of `atlas-api` (P2.11). The component lives in
//! `atlas-api` (feature `ffi`, Swift module `atlas`); this crate only links
//! it into one static library for the XCFramework and carries the bindgen.
//! Built by `apps/macos/scripts/build-core.sh`.

pub use atlas_api::*;
