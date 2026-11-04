// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! C ABI around `atlas_terminal::Terminal` for the terminal spike (P0.5).
//! Frames go to Swift as JSON - good enough to judge rendering, not meant
//! for production (P2.9 would use UniFFI records).

use std::ffi::{CStr, CString, c_char, c_void};
use std::sync::Arc;

use atlas_terminal::{Frame, GridSize, Options, TermEvent, Terminal};
use serde_json::{Value, json};

/// Event kinds passed to the callback.
pub const ATS_REDRAW: u32 = 1;
pub const ATS_TITLE: u32 = 2;
pub const ATS_BELL: u32 = 3;
pub const ATS_EXITED: u32 = 4;

pub type EventFn = extern "C" fn(ctx: *mut c_void, kind: u32, code: i32);

struct Ctx(*mut c_void);
// SAFETY: the Swift side keeps the context alive until ats_free and its
// callback is thread-safe (it only hops to the main queue).
unsafe impl Send for Ctx {}
unsafe impl Sync for Ctx {}

fn cstr(p: *const c_char) -> Option<String> {
    // SAFETY: callers pass NUL-terminated strings or null.
    (!p.is_null()).then(|| unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned())
}

fn out(s: String) -> *mut c_char {
    CString::new(s.replace('\0', "")).unwrap_or_default().into_raw()
}

/// Starts `argv[0..argc]` (or the default shell if `argc == 0`) in a PTY.
/// Returns null on failure.
///
/// # Safety
/// `argv` must point to `argc` C strings; `cwd` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ats_spawn(
    argv: *const *const c_char,
    argc: usize,
    cwd: *const c_char,
    cols: u16,
    lines: u16,
    cell_width: u16,
    cell_height: u16,
    callback: EventFn,
    ctx: *mut c_void,
) -> *mut Terminal {
    let args: Vec<String> =
        (0..argc).filter_map(|i| cstr(unsafe { *argv.add(i) })).collect();
    let shell = args.split_first().map(|(p, rest)| (p.clone(), rest.to_vec()));
    let options = Options { working_directory: cstr(cwd).map(Into::into), shell, scrollback: 10_000 };
    let size = GridSize { cols, lines, cell_width, cell_height };
    let ctx = Arc::new(Ctx(ctx));
    let result = Terminal::spawn(options, size, move |event| {
        let (kind, code) = match event {
            TermEvent::Redraw => (ATS_REDRAW, 0),
            TermEvent::Title(_) => (ATS_TITLE, 0),
            TermEvent::Bell => (ATS_BELL, 0),
            TermEvent::Exited(code) => (ATS_EXITED, code.unwrap_or(-1)),
            TermEvent::ClipboardStore(_) => return,
        };
        callback(ctx.0, kind, code);
    });
    match result {
        Ok(t) => Box::into_raw(Box::new(t)),
        Err(_) => std::ptr::null_mut(),
    }
}

/// # Safety
/// `t` from ats_spawn; `bytes` points to `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ats_write(t: *mut Terminal, bytes: *const u8, len: usize) {
    let (Some(t), false) = (unsafe { t.as_ref() }, bytes.is_null()) else { return };
    t.write(unsafe { std::slice::from_raw_parts(bytes, len) }.to_vec());
}

/// # Safety
/// `t` from ats_spawn.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ats_resize(t: *mut Terminal, cols: u16, lines: u16, cell_width: u16, cell_height: u16) {
    if let Some(t) = unsafe { t.as_ref() } {
        t.resize(GridSize { cols, lines, cell_width, cell_height });
    }
}

/// # Safety
/// `t` from ats_spawn.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ats_scroll(t: *mut Terminal, lines: i32) {
    if let Some(t) = unsafe { t.as_ref() } {
        t.scroll(lines);
    }
}

/// Visible screen as JSON (see `frame_json`); free with ats_string_free.
///
/// # Safety
/// `t` from ats_spawn.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ats_frame_json(t: *mut Terminal, focused: bool) -> *mut c_char {
    match unsafe { t.as_ref() } {
        Some(t) => out(frame_json(&t.frame(focused)).to_string()),
        None => std::ptr::null_mut(),
    }
}

/// Screen text plus `history` lines of scrollback; free with ats_string_free.
///
/// # Safety
/// `t` from ats_spawn.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ats_text(t: *mut Terminal, history: usize) -> *mut c_char {
    match unsafe { t.as_ref() } {
        Some(t) => out(t.text(history)),
        None => std::ptr::null_mut(),
    }
}

/// # Safety
/// `s` from this library or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ats_string_free(s: *mut c_char) {
    if !s.is_null() {
        drop(unsafe { CString::from_raw(s) });
    }
}

/// Stops the shell and frees the terminal.
///
/// # Safety
/// `t` from ats_spawn, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ats_free(t: *mut Terminal) {
    if !t.is_null() {
        drop(unsafe { Box::from_raw(t) });
    }
}

pub fn frame_json(f: &Frame) -> Value {
    let rgb = |c: &alacritty_terminal::vte::ansi::Rgb| json!([c.r, c.g, c.b]);
    json!({
        "cols": f.cols,
        "background": rgb(&f.background),
        "cursorColor": rgb(&f.cursor_color),
        "displayOffset": f.display_offset,
        "historySize": f.history_size,
        "cursor": f.cursor.as_ref().map(|c| json!({
            "col": c.col, "line": c.line, "wide": c.wide, "shape": format!("{:?}", c.shape),
        })),
        "lines": f.lines.iter().map(|l| json!({
            "backgrounds": l.backgrounds.iter().map(|b| json!({"col": b.col, "cells": b.cells, "color": rgb(&b.color)})).collect::<Vec<_>>(),
            "runs": l.runs.iter().map(|r| json!({
                "col": r.col, "cells": r.cells, "text": r.text, "fg": rgb(&r.fg),
                "bold": r.bold, "italic": r.italic, "underline": r.underline, "strikeout": r.strikeout,
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    extern "C" fn ignore(_: *mut c_void, _: u32, _: i32) {}

    #[test]
    fn frame_has_coloured_and_wide_runs() {
        let args = [c"/bin/sh".as_ptr(), c"-c".as_ptr(), c"printf 'hello \\033[31mred\\033[0m \\346\\227\\245\\346\\234\\254x\\n'; sleep 3".as_ptr()];
        let t = unsafe { ats_spawn(args.as_ptr(), args.len(), std::ptr::null(), 40, 5, 8, 16, ignore, std::ptr::null_mut()) };
        assert!(!t.is_null());
        let start = Instant::now();
        let frame = loop {
            let s = unsafe { ats_frame_json(t, true) };
            let v: Value = serde_json::from_str(unsafe { CStr::from_ptr(s) }.to_str().unwrap()).unwrap();
            unsafe { ats_string_free(s) };
            if v.to_string().contains("\"x\"") || start.elapsed() > Duration::from_secs(5) {
                break v;
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        let runs = frame["lines"][0]["runs"].as_array().unwrap();
        let red = runs.iter().find(|r| r["text"] == "red").expect("red run");
        assert!(red["fg"][0].as_u64().unwrap() > 150 && red["fg"][1].as_u64().unwrap() < 120, "{red}");
        let wide = runs.iter().find(|r| r["text"] == "日").expect("wide run");
        assert_eq!(wide["cells"], 2);
        let x = runs.iter().find(|r| r["text"] == "x").expect("x");
        assert_eq!(x["col"].as_u64().unwrap(), wide["col"].as_u64().unwrap() + 4);
        unsafe { ats_free(t) };
    }
}
