// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Embedded terminal: `alacritty_terminal` for VT emulation and the PTY,
//! plus a Qt independent [`Frame`] that describes what to paint.
//!
//! The PTY is read on alacritty's own I/O thread which updates the shared
//! [`Term`]; the UI is notified through [`TermEvent::Redraw`] and takes a
//! [`Frame`] snapshot under a short lock when it paints.

pub mod colors;
pub mod input;

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, EventLoopSender, Msg};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{self, Term, TermMode};
use alacritty_terminal::tty;
use alacritty_terminal::vte::ansi::{CursorShape, Rgb};
use anyhow::{Context, Result};

pub use alacritty_terminal::term::TermMode as Mode;

/// Events for the UI. Delivered on the PTY thread.
#[derive(Debug, Clone)]
pub enum TermEvent {
    Redraw,
    Title(String),
    Bell,
    ClipboardStore(String),
    Exited(Option<i32>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridSize {
    pub cols: u16,
    pub lines: u16,
    pub cell_width: u16,
    pub cell_height: u16,
}

impl GridSize {
    fn window_size(self) -> WindowSize {
        WindowSize {
            num_lines: self.lines,
            num_cols: self.cols,
            cell_width: self.cell_width,
            cell_height: self.cell_height,
        }
    }
}

impl Dimensions for GridSize {
    fn total_lines(&self) -> usize {
        self.lines as usize
    }
    fn screen_lines(&self) -> usize {
        self.lines as usize
    }
    fn columns(&self) -> usize {
        self.cols as usize
    }
}

#[derive(Debug, Clone, Default)]
pub struct Options {
    pub working_directory: Option<PathBuf>,
    /// Program and arguments; `None` uses [`default_shell`].
    pub shell: Option<(String, Vec<String>)>,
    pub scrollback: usize,
}

type Callback = dyn Fn(TermEvent) + Send + Sync;

/// Bridges alacritty's events to the UI callback and answers terminal
/// queries (colors, text area size) directly on the PTY thread.
#[derive(Clone)]
struct Listener {
    callback: Arc<Callback>,
    sender: Arc<Mutex<Option<EventLoopSender>>>,
    size: Arc<Mutex<GridSize>>,
}

impl Listener {
    fn write(&self, text: String) {
        if let Some(sender) = self.sender.lock().unwrap().as_ref() {
            let _ = sender.send(Msg::Input(Cow::Owned(text.into_bytes())));
        }
    }
}

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        match event {
            Event::Wakeup | Event::MouseCursorDirty | Event::CursorBlinkingChange => (self.callback)(TermEvent::Redraw),
            Event::Title(title) => (self.callback)(TermEvent::Title(title)),
            Event::ResetTitle => (self.callback)(TermEvent::Title(String::new())),
            Event::Bell => (self.callback)(TermEvent::Bell),
            Event::ClipboardStore(_, text) => (self.callback)(TermEvent::ClipboardStore(text)),
            // Reading the clipboard from inside the terminal (OSC 52) is not
            // allowed: a remote program could exfiltrate its contents.
            Event::ClipboardLoad(..) => {}
            Event::PtyWrite(text) => self.write(text),
            Event::ColorRequest(index, format) => {
                self.write(format(colors::default_for_slot(index)));
            }
            Event::TextAreaSizeRequest(format) => {
                let size = *self.size.lock().unwrap();
                self.write(format(size.window_size()));
            }
            Event::ChildExit(status) => (self.callback)(TermEvent::Exited(status.code())),
            Event::Exit => (self.callback)(TermEvent::Exited(None)),
        }
    }
}

pub struct Terminal {
    term: Arc<FairMutex<Term<Listener>>>,
    sender: EventLoopSender,
    size: Arc<Mutex<GridSize>>,
    /// PID of the shell, for its current working directory.
    pid: u32,
}

impl Terminal {
    pub fn spawn(
        options: Options,
        size: GridSize,
        on_event: impl Fn(TermEvent) + Send + Sync + 'static,
    ) -> Result<Self> {
        let size_cell = Arc::new(Mutex::new(size));
        let listener = Listener { callback: Arc::new(on_event), sender: Arc::default(), size: size_cell.clone() };

        let mut env = HashMap::new();
        env.insert("TERM".to_owned(), "xterm-256color".to_owned());
        env.insert("COLORTERM".to_owned(), "truecolor".to_owned());
        env.insert("TERM_PROGRAM".to_owned(), "atlas".to_owned());
        let shell = options
            .shell
            .map(|(program, args)| tty::Shell::new(program, args))
            .unwrap_or_else(|| tty::Shell::new(default_shell(), Vec::new()));
        let pty_options =
            tty::Options { shell: Some(shell), working_directory: options.working_directory, drain_on_exit: true, env };
        let pty = tty::new(&pty_options, size.window_size(), 0).context("cannot open PTY")?;
        let pid = pty.child().id();

        let config = term::Config {
            scrolling_history: if options.scrollback == 0 { 10_000 } else { options.scrollback },
            ..Default::default()
        };
        let term = Arc::new(FairMutex::new(Term::new(config, &size, listener.clone())));
        let event_loop =
            EventLoop::new(term.clone(), listener.clone(), pty, true, false).context("cannot start terminal I/O")?;
        let sender = event_loop.channel();
        *listener.sender.lock().unwrap() = Some(event_loop.channel());
        event_loop.spawn();

        Ok(Self { term, sender, size: size_cell, pid })
    }

    /// Current working directory of the shell (Linux: `/proc/<pid>/cwd`).
    pub fn current_dir(&self) -> Option<PathBuf> {
        std::fs::read_link(format!("/proc/{}/cwd", self.pid)).ok()
    }

    pub fn size(&self) -> GridSize {
        *self.size.lock().unwrap()
    }

    pub fn mode(&self) -> TermMode {
        *self.term.lock().mode()
    }

    /// Sends raw input to the program. Typing also scrolls back to the
    /// bottom, like VS Code.
    pub fn write(&self, bytes: impl Into<Cow<'static, [u8]>>) {
        {
            let mut term = self.term.lock();
            if term.grid().display_offset() != 0 {
                term.scroll_display(Scroll::Bottom);
            }
            term.selection = None;
        }
        let _ = self.sender.send(Msg::Input(bytes.into()));
    }

    /// Sends input without touching viewport or selection (mouse reports,
    /// alternate scroll).
    pub fn write_raw(&self, bytes: impl Into<Cow<'static, [u8]>>) {
        let _ = self.sender.send(Msg::Input(bytes.into()));
    }

    pub fn resize(&self, size: GridSize) {
        let size = GridSize { cols: size.cols.max(2), lines: size.lines.max(1), ..size };
        {
            let mut current = self.size.lock().unwrap();
            if *current == size {
                return;
            }
            *current = size;
        }
        self.term.lock().resize(size);
        let _ = self.sender.send(Msg::Resize(size.window_size()));
    }

    /// Scrolls the viewport through the scrollback (positive = up).
    pub fn scroll(&self, lines: i32) {
        self.term.lock().scroll_display(Scroll::Delta(lines));
    }

    pub fn display_offset(&self) -> usize {
        self.term.lock().grid().display_offset()
    }

    pub fn history_size(&self) -> usize {
        self.term.lock().grid().history_size()
    }

    /// Starts a selection at a viewport cell. `kind`: 1 = characters,
    /// 2 = words, 3 = lines (click count).
    pub fn select_start(&self, col: usize, line: usize, right_half: bool, kind: u8) {
        let mut term = self.term.lock();
        let point = viewport_point(&term, col, line);
        let ty = match kind {
            2 => SelectionType::Semantic,
            3 => SelectionType::Lines,
            _ => SelectionType::Simple,
        };
        let side = if right_half { Side::Right } else { Side::Left };
        term.selection = Some(Selection::new(ty, point, side));
    }

    pub fn select_update(&self, col: usize, line: usize, right_half: bool) {
        let mut term = self.term.lock();
        let point = viewport_point(&term, col, line);
        let side = if right_half { Side::Right } else { Side::Left };
        if let Some(selection) = term.selection.as_mut() {
            selection.update(point, side);
        }
    }

    pub fn select_clear(&self) {
        self.term.lock().selection = None;
    }

    pub fn selection_text(&self) -> Option<String> {
        self.term.lock().selection_to_string().filter(|s| !s.is_empty())
    }

    /// Plain text of the screen plus up to `history` lines of scrollback,
    /// without trailing blanks (for "copy output into a note").
    pub fn text(&self, history: usize) -> String {
        let term = self.term.lock();
        let grid = term.grid();
        let first = -(grid.history_size().min(history) as i32);
        let mut lines: Vec<String> = Vec::new();
        // A row ending in a soft wrap continues on the next one.
        let mut continued = false;
        for line in first..grid.screen_lines() as i32 {
            let row = &grid[Line(line)];
            let mut text = String::new();
            for col in 0..grid.columns() {
                let cell = &row[Column(col)];
                if cell.flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
                    continue;
                }
                text.push(cell.c);
            }
            let wraps = row[Column(grid.columns() - 1)].flags.contains(Flags::WRAPLINE);
            let text = if wraps { text } else { text.trim_end().to_owned() };
            match lines.last_mut() {
                Some(last) if continued => last.push_str(&text),
                _ => lines.push(text),
            }
            continued = wraps;
        }
        while lines.last().is_some_and(|l| l.is_empty()) {
            lines.pop();
        }
        while lines.first().is_some_and(|l| l.is_empty()) {
            lines.remove(0);
        }
        lines.join("\n")
    }

    /// Takes a snapshot of the visible screen.
    pub fn frame(&self, focused: bool) -> Frame {
        Frame::build(&self.term.lock(), focused)
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = self.sender.send(Msg::Shutdown);
    }
}

/// Shell for new terminals: `ATLAS_SHELL`, then the login shell from the
/// user database, then `$SHELL`, then `/bin/sh`.
///
/// The login shell comes before `$SHELL` because development environments
/// (e.g. `nix develop`) export a non-interactive `bash` as `$SHELL`.
pub fn default_shell() -> String {
    let usable = |s: &str| !s.is_empty() && std::path::Path::new(s).exists();
    if let Ok(shell) = std::env::var("ATLAS_SHELL")
        && usable(&shell)
    {
        return shell;
    }
    if let Some(shell) = login_shell()
        && usable(&shell)
    {
        return shell;
    }
    match std::env::var("SHELL") {
        Ok(shell) if usable(&shell) => shell,
        _ => "/bin/sh".to_owned(),
    }
}

fn login_shell() -> Option<String> {
    let mut buf = vec![0 as libc::c_char; 4096];
    let mut pwd: libc::passwd = unsafe { std::mem::zeroed() };
    let mut result: *mut libc::passwd = std::ptr::null_mut();
    // SAFETY: all pointers refer to live, correctly sized buffers.
    let rc = unsafe { libc::getpwuid_r(libc::getuid(), &mut pwd, buf.as_mut_ptr(), buf.len(), &mut result) };
    if rc != 0 || result.is_null() || pwd.pw_shell.is_null() {
        return None;
    }
    // SAFETY: getpwuid_r succeeded, pw_shell points into `buf`.
    let shell = unsafe { std::ffi::CStr::from_ptr(pwd.pw_shell) };
    shell.to_str().ok().map(str::to_owned)
}

fn viewport_point<T>(term: &Term<T>, col: usize, line: usize) -> Point {
    let line = line.min(term.screen_lines().saturating_sub(1));
    let col = col.min(term.columns().saturating_sub(1));
    let offset = term.grid().display_offset() as i32;
    Point::new(Line(line as i32 - offset), Column(col))
}

/// A run of cells on one line with identical style. Runs of plain ASCII are
/// merged so that they can be drawn with one text call; every other
/// character gets its own run so fallback fonts cannot shift the grid.
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    pub col: u16,
    /// Number of grid cells covered.
    pub cells: u16,
    pub text: String,
    pub fg: Rgb,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikeout: bool,
}

/// A rectangle of non-default background.
#[derive(Debug, Clone, PartialEq)]
pub struct Background {
    pub col: u16,
    pub cells: u16,
    pub color: Rgb,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cursor {
    pub col: u16,
    pub line: u16,
    pub shape: CursorShape,
    pub wide: bool,
}

#[derive(Debug, Clone, Default)]
pub struct FrameLine {
    pub backgrounds: Vec<Background>,
    pub runs: Vec<Run>,
}

#[derive(Debug, Clone, Default)]
pub struct Frame {
    pub cols: u16,
    pub lines: Vec<FrameLine>,
    pub cursor: Option<Cursor>,
    pub background: Rgb,
    pub cursor_color: Rgb,
    pub display_offset: usize,
    pub history_size: usize,
}

impl Frame {
    fn build<T: EventListener>(term: &Term<T>, focused: bool) -> Self {
        let content = term.renderable_content();
        let colors = content.colors;
        let background = colors[alacritty_terminal::vte::ansi::NamedColor::Background].unwrap_or(colors::BACKGROUND);
        let screen_lines = term.screen_lines();
        let mut lines = vec![FrameLine::default(); screen_lines];
        let selection = content.selection;
        let offset = content.display_offset;

        for indexed in content.display_iter {
            let Some(vp) = term::point_to_viewport(offset, indexed.point) else { continue };
            if vp.line >= screen_lines {
                continue;
            }
            let cell = indexed.cell;
            let flags = cell.flags;
            if flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
                continue;
            }
            let bold = flags.contains(Flags::BOLD);
            let mut fg = colors::resolve(cell.fg, colors, bold);
            let mut bg = colors::resolve(cell.bg, colors, false);
            if flags.contains(Flags::DIM) {
                fg = Rgb { r: fg.r / 3 * 2, g: fg.g / 3 * 2, b: fg.b / 3 * 2 };
            }
            if flags.contains(Flags::INVERSE) {
                std::mem::swap(&mut fg, &mut bg);
            }
            let selected =
                selection.is_some_and(|s| s.contains_cell(&indexed, content.cursor.point, content.cursor.shape));
            if selected {
                bg = colors::SELECTION;
            }
            let width: u16 = if flags.contains(Flags::WIDE_CHAR) { 2 } else { 1 };
            let col = indexed.point.column.0 as u16;
            let line = &mut lines[vp.line];

            if bg != background {
                match line.backgrounds.last_mut() {
                    Some(last) if last.color == bg && last.col + last.cells == col => last.cells += width,
                    _ => line.backgrounds.push(Background { col, cells: width, color: bg }),
                }
            }

            let c = cell.c;
            if flags.contains(Flags::HIDDEN)
                || (c == ' ' && !flags.intersects(Flags::ALL_UNDERLINES | Flags::STRIKEOUT))
            {
                continue;
            }
            let mut text = String::new();
            text.push(c);
            if let Some(zerowidth) = cell.zerowidth() {
                text.extend(zerowidth);
            }
            let run = Run {
                col,
                cells: width,
                text,
                fg,
                bold,
                italic: flags.contains(Flags::ITALIC),
                underline: flags.intersects(Flags::ALL_UNDERLINES),
                strikeout: flags.contains(Flags::STRIKEOUT),
            };
            let ascii = c.is_ascii() && width == 1 && run.text.len() == 1;
            match line.runs.last_mut() {
                Some(last)
                    if ascii
                        && last.col + last.cells == col
                        && last.text.is_ascii()
                        && last.cells as usize == last.text.len()
                        && last.fg == run.fg
                        && (last.bold, last.italic, last.underline, last.strikeout)
                            == (run.bold, run.italic, run.underline, run.strikeout) =>
                {
                    last.text.push(c);
                    last.cells += 1;
                }
                _ => line.runs.push(run),
            }
        }

        let cursor = {
            let point = content.cursor.point;
            let shape = content.cursor.shape;
            let visible = content.mode.contains(TermMode::SHOW_CURSOR) && shape != CursorShape::Hidden;
            term::point_to_viewport(offset, point).filter(|vp| visible && vp.line < screen_lines).map(|vp| {
                let wide = term.grid()[point].flags.contains(Flags::WIDE_CHAR);
                Cursor {
                    col: vp.column.0 as u16,
                    line: vp.line as u16,
                    // Unfocused terminals show a hollow block, like VS Code.
                    shape: if focused { shape } else { CursorShape::HollowBlock },
                    wide,
                }
            })
        };

        Frame {
            cols: term.columns() as u16,
            lines,
            cursor,
            background,
            cursor_color: colors[alacritty_terminal::vte::ansi::NamedColor::Cursor].unwrap_or(colors::CURSOR),
            display_offset: offset,
            history_size: term.grid().history_size(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn wait_for(term: &Terminal, mut pred: impl FnMut(&Frame) -> bool) -> Frame {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let frame = term.frame(true);
            if pred(&frame) {
                return frame;
            }
            assert!(Instant::now() < deadline, "timeout; last frame: {:#?}", frame.lines);
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn line_text(frame: &Frame, line: usize) -> String {
        let mut out = String::new();
        for run in &frame.lines[line].runs {
            while out.chars().count() < run.col as usize {
                out.push(' ');
            }
            out.push_str(&run.text);
        }
        out
    }

    #[test]
    fn runs_program_with_colors_and_unicode() {
        let size = GridSize { cols: 40, lines: 5, cell_width: 8, cell_height: 16 };
        let options = Options {
            shell: Some(("/bin/sh".into(), vec!["-c".into(), r"printf '\033[31mred\033[0m ä€😀\n'; sleep 5".into()])),
            ..Default::default()
        };
        let term = Terminal::spawn(options, size, |_| {}).unwrap();
        let frame = wait_for(&term, |f| line_text(f, 0).starts_with("red"));
        let first = &frame.lines[0].runs[0];
        assert_eq!(first.text, "red");
        assert_eq!(first.fg, colors::indexed(1));
        // The emoji occupies two cells; each non-ASCII char is its own run.
        let emoji = frame.lines[0].runs.iter().find(|r| r.text == "😀").unwrap();
        assert_eq!(emoji.cells, 2);
        assert_eq!(emoji.col, 6);
    }

    #[test]
    fn tracks_current_directory() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().canonicalize().unwrap();
        let size = GridSize { cols: 40, lines: 5, cell_width: 8, cell_height: 16 };
        let script = format!("cd {} && sleep 5", target.display());
        let options = Options { shell: Some(("/bin/sh".into(), vec!["-c".into(), script])), ..Default::default() };
        let term = Terminal::spawn(options, size, |_| {}).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while term.current_dir().as_deref() != Some(target.as_path()) {
            assert!(Instant::now() < deadline, "cwd: {:?}", term.current_dir());
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn resize_reaches_program() {
        let size = GridSize { cols: 40, lines: 5, cell_width: 8, cell_height: 16 };
        let options = Options {
            shell: Some(("/bin/sh".into(), vec!["-c".into(), "sleep 0.3; stty size; sleep 5".into()])),
            ..Default::default()
        };
        let term = Terminal::spawn(options, size, |_| {}).unwrap();
        term.resize(GridSize { cols: 60, lines: 7, ..size });
        wait_for(&term, |f| (0..f.lines.len()).any(|l| line_text(f, l).contains("7 60")));
    }

    #[test]
    fn exports_screen_text() {
        let size = GridSize { cols: 40, lines: 5, cell_width: 8, cell_height: 16 };
        let options = Options {
            shell: Some((
                "/bin/sh".into(),
                vec![
                    "-c".into(),
                    "printf 'eins\\nzwei  \\nä😀x\\n0123456789abcdefghijklmnopqrstuvwxyz0123456789\\n'; sleep 5".into(),
                ],
            )),
            ..Default::default()
        };
        let term = Terminal::spawn(options, size, |_| {}).unwrap();
        wait_for(&term, |f| (0..f.lines.len()).any(|l| line_text(f, l).starts_with("456789")));
        // The last line is wider than the terminal: it wraps, but exports as
        // one line.
        assert_eq!(term.text(100), "eins\nzwei\nä😀x\n0123456789abcdefghijklmnopqrstuvwxyz0123456789");
    }

    /// Bash, Zsh and Fish start interactively and run commands (each only
    /// if installed; `nix shell nixpkgs#zsh nixpkgs#fish` provides them).
    #[test]
    fn interactive_shells() {
        let size = GridSize { cols: 80, lines: 10, cell_width: 8, cell_height: 16 };
        let home = tempfile::tempdir().unwrap();
        for name in ["bash", "zsh", "fish"] {
            let Some(path) = std::env::var_os("PATH")
                .and_then(|p| std::env::split_paths(&p).map(|d| d.join(name)).find(|p| p.exists()))
            else {
                eprintln!("{name} not installed, skipped");
                continue;
            };
            let options = Options {
                shell: Some((path.to_string_lossy().into_owned(), Vec::new())),
                working_directory: Some(home.path().to_path_buf()),
                ..Default::default()
            };
            let term = Terminal::spawn(options, size, |_| {}).unwrap();
            // Typed like a user; the quotes keep the echo of the input itself
            // from matching.
            std::thread::sleep(Duration::from_millis(300));
            term.write(b"echo atlas\"\"-ok\r".to_vec());
            let deadline = Instant::now() + Duration::from_secs(10);
            while !term.text(100).lines().any(|l| l.trim() == "atlas-ok") {
                assert!(Instant::now() < deadline, "{name}: {}", term.text(100));
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}
