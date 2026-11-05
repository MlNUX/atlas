// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! `.atlasnote` notebooks from Atlas Tablet (lücke L7 in
//! `docs/porting/paritaet.md`, task P2.13). Without `pdf-poppler` the core
//! cannot draw pages (P1.1), so the macOS app draws them itself from the
//! notebook data this module hands out (strokes, text, placed images,
//! paper, PDF backgrounds). Template: `atlas_note_info`/`page`/`preview` in
//! the Qt bridge, core `atlasnote.rs`.
//!
//! Empty until P2.13 fills it (types and methods are LLM2's proposal there).
