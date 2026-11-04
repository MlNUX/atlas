// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Encoding of keyboard and mouse input into xterm escape sequences.
//!
//! Key codes are Qt's `Qt::Key` values so that the QML layer can pass
//! `event.key` straight through.

use alacritty_terminal::term::TermMode;

#[allow(non_upper_case_globals, dead_code)]
pub mod qt_key {
    pub const Space: i32 = 0x20;
    pub const Escape: i32 = 0x0100_0000;
    pub const Tab: i32 = 0x0100_0001;
    pub const Backtab: i32 = 0x0100_0002;
    pub const Backspace: i32 = 0x0100_0003;
    pub const Return: i32 = 0x0100_0004;
    pub const Enter: i32 = 0x0100_0005;
    pub const Insert: i32 = 0x0100_0006;
    pub const Delete: i32 = 0x0100_0007;
    pub const Home: i32 = 0x0100_0010;
    pub const End: i32 = 0x0100_0011;
    pub const Left: i32 = 0x0100_0012;
    pub const Up: i32 = 0x0100_0013;
    pub const Right: i32 = 0x0100_0014;
    pub const Down: i32 = 0x0100_0015;
    pub const PageUp: i32 = 0x0100_0016;
    pub const PageDown: i32 = 0x0100_0017;
    pub const Shift: i32 = 0x0100_0020;
    pub const Control: i32 = 0x0100_0021;
    pub const Meta: i32 = 0x0100_0022;
    pub const Alt: i32 = 0x0100_0023;
    pub const CapsLock: i32 = 0x0100_0024;
    pub const NumLock: i32 = 0x0100_0025;
    pub const AltGr: i32 = 0x0100_1103;
    pub const F1: i32 = 0x0100_0030;
    pub const F12: i32 = 0x0100_003b;
}

/// `Qt::KeyboardModifier` bits.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Mods {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

impl Mods {
    pub fn from_qt(bits: i32) -> Self {
        Self {
            shift: bits & 0x0200_0000 != 0,
            ctrl: bits & 0x0400_0000 != 0,
            alt: bits & 0x0800_0000 != 0,
            meta: bits & 0x1000_0000 != 0,
        }
    }

    /// xterm modifier parameter (1 = none).
    fn param(self) -> u8 {
        1 + self.shift as u8 + 2 * self.alt as u8 + 4 * self.ctrl as u8 + 8 * self.meta as u8
    }

    fn any(self) -> bool {
        self.shift || self.ctrl || self.alt || self.meta
    }
}

/// Bytes to send for a key press, or `None` if the key produces no input
/// (pure modifier keys, unknown keys without text).
pub fn encode_key(key: i32, mods: Mods, text: &str, mode: TermMode) -> Option<Vec<u8>> {
    use qt_key as k;
    let app_cursor = mode.contains(TermMode::APP_CURSOR);

    // Cursor and editing keys: CSI/SS3 sequences with optional modifiers.
    let cursor = |letter: u8| -> Vec<u8> {
        if mods.any() {
            format!("\x1b[1;{}{}", mods.param(), letter as char).into_bytes()
        } else if app_cursor {
            vec![0x1b, b'O', letter]
        } else {
            vec![0x1b, b'[', letter]
        }
    };
    let tilde = |code: u8| -> Vec<u8> {
        if mods.any() {
            format!("\x1b[{code};{}~", mods.param()).into_bytes()
        } else {
            format!("\x1b[{code}~").into_bytes()
        }
    };
    let alt_prefix = |bytes: &[u8]| -> Vec<u8> {
        let mut v = Vec::with_capacity(bytes.len() + 1);
        if mods.alt {
            v.push(0x1b);
        }
        v.extend_from_slice(bytes);
        v
    };

    let bytes = match key {
        k::Shift | k::Control | k::Meta | k::Alt | k::AltGr | k::CapsLock | k::NumLock => return None,
        k::Up => cursor(b'A'),
        k::Down => cursor(b'B'),
        k::Right => cursor(b'C'),
        k::Left => cursor(b'D'),
        k::Home => cursor(b'H'),
        k::End => cursor(b'F'),
        k::Insert => tilde(2),
        k::Delete => tilde(3),
        k::PageUp => tilde(5),
        k::PageDown => tilde(6),
        _ if (k::F1..=k::F1 + 3).contains(&key) => {
            let letter = b'P' + (key - k::F1) as u8;
            if mods.any() {
                format!("\x1b[1;{}{}", mods.param(), letter as char).into_bytes()
            } else {
                vec![0x1b, b'O', letter]
            }
        }
        _ if (k::F1 + 4..=k::F12).contains(&key) => {
            const CODES: [u8; 8] = [15, 17, 18, 19, 20, 21, 23, 24];
            tilde(CODES[(key - k::F1 - 4) as usize])
        }
        k::Escape => alt_prefix(b"\x1b"),
        k::Return | k::Enter => alt_prefix(b"\r"),
        k::Backtab => b"\x1b[Z".to_vec(),
        k::Tab if mods.shift => b"\x1b[Z".to_vec(),
        k::Tab => alt_prefix(b"\t"),
        k::Backspace if mods.ctrl => alt_prefix(b"\x08"),
        k::Backspace => alt_prefix(b"\x7f"),
        _ if mods.ctrl && !mods.meta => match control_byte(key, text) {
            Some(b) => alt_prefix(&[b]),
            None if !text.is_empty() => alt_prefix(text.as_bytes()),
            None => return None,
        },
        _ if !text.is_empty() => {
            // Qt reports Ctrl/Alt combinations with control characters in
            // `text`; plain printable text is passed through unchanged.
            alt_prefix(text.as_bytes())
        }
        _ => return None,
    };
    Some(bytes)
}

/// Control character for Ctrl+<key>.
fn control_byte(key: i32, text: &str) -> Option<u8> {
    match key {
        0x41..=0x5a => Some((key as u8) & 0x1f), // A–Z
        0x20 | 0x40 | 0x32 => Some(0),           // Space, @, 2
        0x5b | 0x33 => Some(0x1b),               // [, 3
        0x5c | 0x34 => Some(0x1c),               // \, 4
        0x5d | 0x35 => Some(0x1d),               // ], 5
        0x5e | 0x36 => Some(0x1e),               // ^, 6
        0x5f | 0x2f | 0x37 => Some(0x1f),        // _, /, 7
        0x38 => Some(0x7f),                      // 8
        _ => text.bytes().next().filter(|b| *b < 0x20),
    }
}

/// Wraps pasted text in bracketed paste markers when the application asked
/// for it, and normalises newlines to carriage returns.
pub fn encode_paste(text: &str, mode: TermMode) -> Vec<u8> {
    let normalized = text.replace("\r\n", "\r").replace('\n', "\r");
    if mode.contains(TermMode::BRACKETED_PASTE) {
        // Never let pasted content end bracketed paste early.
        let cleaned = normalized.replace("\x1b[201~", "");
        format!("\x1b[200~{cleaned}\x1b[201~").into_bytes()
    } else {
        normalized.into_bytes()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    WheelUp,
    WheelDown,
    /// Motion without a pressed button.
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseAction {
    Press,
    Release,
    Motion,
}

/// Mouse report for applications that enabled mouse tracking. `col`/`line`
/// are 0 based viewport coordinates.
pub fn encode_mouse(
    button: MouseButton,
    action: MouseAction,
    col: usize,
    line: usize,
    mods: Mods,
    mode: TermMode,
) -> Option<Vec<u8>> {
    if !mode.intersects(TermMode::MOUSE_MODE) {
        return None;
    }
    if action == MouseAction::Motion {
        let dragging = button != MouseButton::None;
        if !(mode.contains(TermMode::MOUSE_MOTION) || dragging && mode.contains(TermMode::MOUSE_DRAG)) {
            return None;
        }
    }
    let mut code: u8 = match button {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
        MouseButton::None => 3,
        MouseButton::WheelUp => 64,
        MouseButton::WheelDown => 65,
    };
    if action == MouseAction::Motion {
        code += 32;
    }
    code += 4 * mods.shift as u8 + 8 * mods.alt as u8 + 16 * mods.ctrl as u8;

    if mode.contains(TermMode::SGR_MOUSE) {
        let suffix = if action == MouseAction::Release { 'm' } else { 'M' };
        return Some(format!("\x1b[<{code};{};{}{suffix}", col + 1, line + 1).into_bytes());
    }
    // Legacy X10 encoding: releases are reported as button 3, coordinates
    // are limited to 223.
    if action == MouseAction::Release {
        code = 3 + (code & !3);
    }
    if col > 222 || line > 222 {
        return None;
    }
    Some(vec![0x1b, b'[', b'M', 32 + code, 33 + col as u8, 33 + line as u8])
}

#[cfg(test)]
mod tests {
    use super::*;
    use qt_key as k;

    fn none() -> Mods {
        Mods::default()
    }

    #[test]
    fn arrows_follow_cursor_mode() {
        assert_eq!(encode_key(k::Up, none(), "", TermMode::NONE).unwrap(), b"\x1b[A");
        assert_eq!(encode_key(k::Up, none(), "", TermMode::APP_CURSOR).unwrap(), b"\x1bOA");
        let ctrl = Mods { ctrl: true, ..none() };
        assert_eq!(encode_key(k::Right, ctrl, "", TermMode::APP_CURSOR).unwrap(), b"\x1b[1;5C");
    }

    #[test]
    fn control_and_alt() {
        let ctrl = Mods { ctrl: true, ..none() };
        assert_eq!(encode_key(0x43, ctrl, "\x03", TermMode::NONE).unwrap(), b"\x03");
        let alt = Mods { alt: true, ..none() };
        assert_eq!(encode_key(0x42, alt, "b", TermMode::NONE).unwrap(), b"\x1bb");
        assert_eq!(encode_key(k::Backspace, none(), "", TermMode::NONE).unwrap(), b"\x7f");
        assert_eq!(encode_key(k::Shift, none(), "", TermMode::NONE), None);
    }

    #[test]
    fn function_keys() {
        assert_eq!(encode_key(k::F1, none(), "", TermMode::NONE).unwrap(), b"\x1bOP");
        assert_eq!(encode_key(k::F1 + 4, none(), "", TermMode::NONE).unwrap(), b"\x1b[15~");
        assert_eq!(encode_key(k::F12, none(), "", TermMode::NONE).unwrap(), b"\x1b[24~");
    }

    #[test]
    fn unicode_text_passes_through() {
        assert_eq!(encode_key(0, none(), "ä€😀", TermMode::NONE).unwrap(), "ä€😀".as_bytes());
    }

    #[test]
    fn bracketed_paste() {
        assert_eq!(encode_paste("a\nb", TermMode::NONE), b"a\rb");
        assert_eq!(encode_paste("x", TermMode::BRACKETED_PASTE), b"\x1b[200~x\x1b[201~");
    }

    #[test]
    fn sgr_mouse() {
        let mode = TermMode::MOUSE_REPORT_CLICK | TermMode::SGR_MOUSE;
        assert_eq!(encode_mouse(MouseButton::Left, MouseAction::Press, 4, 2, none(), mode).unwrap(), b"\x1b[<0;5;3M");
        assert_eq!(encode_mouse(MouseButton::Left, MouseAction::Release, 4, 2, none(), mode).unwrap(), b"\x1b[<0;5;3m");
        assert_eq!(encode_mouse(MouseButton::Left, MouseAction::Press, 0, 0, none(), TermMode::NONE), None);
    }
}
