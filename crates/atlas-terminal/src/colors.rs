// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Terminal palette (VS Code "Dark Modern" terminal colors, "Light Modern"
//! in the light design) and resolution of cell colors to RGB.

use std::sync::atomic::{AtomicBool, Ordering};

use alacritty_terminal::term::color::Colors;
use alacritty_terminal::vte::ansi::{Color, NamedColor, Rgb};

const fn rgb(hex: u32) -> Rgb {
    Rgb { r: (hex >> 16) as u8, g: (hex >> 8) as u8, b: hex as u8 }
}

struct Palette {
    foreground: Rgb,
    background: Rgb,
    cursor: Rgb,
    selection: Rgb,
    ansi: [Rgb; 16],
}

const DARK: Palette = Palette {
    foreground: rgb(0xc5d0e0),
    background: rgb(0x0b111d),
    cursor: rgb(0xaeafad),
    selection: rgb(0x1d3a63),
    ansi: [
        rgb(0x000000),
        rgb(0xcd3131),
        rgb(0x0dbc79),
        rgb(0xe5e510),
        rgb(0x2472c8),
        rgb(0xbc3fbc),
        rgb(0x11a8cd),
        rgb(0xe5e5e5),
        rgb(0x666666),
        rgb(0xf14c4c),
        rgb(0x23d18b),
        rgb(0xf5f543),
        rgb(0x3b8eea),
        rgb(0xd670d6),
        rgb(0x29b8db),
        rgb(0xe5e5e5),
    ],
};

const LIGHT: Palette = Palette {
    foreground: rgb(0x1f2a3d),
    background: rgb(0xffffff),
    cursor: rgb(0x3b4a63),
    selection: rgb(0xc9dcfb),
    ansi: [
        rgb(0x000000),
        rgb(0xcd3131),
        rgb(0x107c10),
        rgb(0x949800),
        rgb(0x0451a5),
        rgb(0xbc05bc),
        rgb(0x0598bc),
        rgb(0x555555),
        rgb(0x666666),
        rgb(0xcd3131),
        rgb(0x14ce14),
        rgb(0xb5ba00),
        rgb(0x0451a5),
        rgb(0xbc05bc),
        rgb(0x0598bc),
        rgb(0xa5a5a5),
    ],
};

static LIGHT_DESIGN: AtomicBool = AtomicBool::new(false);

/// Switches all terminals to the light (or dark) palette; they pick it up
/// on their next repaint.
pub fn set_light(on: bool) {
    LIGHT_DESIGN.store(on, Ordering::Relaxed);
}

fn palette() -> &'static Palette {
    if LIGHT_DESIGN.load(Ordering::Relaxed) { &LIGHT } else { &DARK }
}

pub fn foreground() -> Rgb {
    palette().foreground
}

pub fn background() -> Rgb {
    palette().background
}

pub fn cursor() -> Rgb {
    palette().cursor
}

pub fn selection() -> Rgb {
    palette().selection
}

/// Default color of xterm's 256 color palette entry `index`.
pub fn indexed(index: u8) -> Rgb {
    match index {
        0..=15 => palette().ansi[index as usize],
        16..=231 => {
            let i = index - 16;
            let level = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            Rgb { r: level(i / 36), g: level((i / 6) % 6), b: level(i % 6) }
        }
        232..=255 => {
            let v = 8 + (index - 232) * 10;
            Rgb { r: v, g: v, b: v }
        }
    }
}

fn dim(c: Rgb) -> Rgb {
    Rgb { r: (c.r as u16 * 2 / 3) as u8, g: (c.g as u16 * 2 / 3) as u8, b: (c.b as u16 * 2 / 3) as u8 }
}

/// Default for a palette slot as used by OSC 4/10/11/12 color queries.
pub fn default_for_slot(index: usize) -> Rgb {
    match index {
        0..=255 => indexed(index as u8),
        i if i == NamedColor::Foreground as usize => foreground(),
        i if i == NamedColor::Background as usize => background(),
        i if i == NamedColor::Cursor as usize => cursor(),
        _ => foreground(),
    }
}

/// Resolves a cell color, honouring palette changes made by the
/// application (OSC 4 etc.) that are stored in `colors`.
pub fn resolve(color: Color, colors: &Colors, bold: bool) -> Rgb {
    match color {
        Color::Spec(rgb) => rgb,
        Color::Indexed(i) => {
            // Bold text in one of the 8 base colors is shown bright.
            let i = if bold && i < 8 { i + 8 } else { i };
            colors[i as usize].unwrap_or_else(|| indexed(i))
        }
        Color::Named(named) => {
            let named = if bold { named.to_bright() } else { named };
            if let Some(c) = colors[named] {
                return c;
            }
            let index = named as usize;
            match named {
                NamedColor::Foreground | NamedColor::BrightForeground => foreground(),
                NamedColor::Background => background(),
                NamedColor::Cursor => cursor(),
                NamedColor::DimForeground => dim(foreground()),
                _ if index < 16 => palette().ansi[index],
                // Dim variants follow the bright foreground block.
                _ => {
                    let base = index - NamedColor::DimBlack as usize;
                    dim(palette().ansi[base.min(7)])
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xterm_cube_and_ramp() {
        assert_eq!(indexed(16), Rgb { r: 0, g: 0, b: 0 });
        assert_eq!(indexed(196), Rgb { r: 255, g: 0, b: 0 });
        assert_eq!(indexed(231), Rgb { r: 255, g: 255, b: 255 });
        assert_eq!(indexed(232), Rgb { r: 8, g: 8, b: 8 });
        assert_eq!(indexed(255), Rgb { r: 238, g: 238, b: 238 });
    }
}
