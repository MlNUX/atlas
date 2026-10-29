// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! `TerminalView`: an embedded terminal as a `QQuickPaintedItem`.
//!
//! Emulation and PTY live in `atlas_core::terminal`; this item only maps
//! QML input to escape sequences and paints [`Frame`] snapshots. Keyboard
//! and mouse events are forwarded from QML (`Keys`, `MouseArea`,
//! `WheelHandler`) through invokables.

use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use alacritty_terminal::vte::ansi::{CursorShape, Rgb};
use atlas_core::terminal::input::{self, Mods, MouseAction, MouseButton};
use atlas_core::terminal::{Frame, GridSize, Mode, Options, TermEvent, Terminal};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QColor, QFont, QPoint, QRectF, QString, QStringList};

use crate::qt_helpers;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!(<QtQuick/QQuickPaintedItem>);
        type QQuickPaintedItem;
        include!("cxx-qt-lib/qpainter.h");
        type QPainter = cxx_qt_lib::QPainter;
        include!("cxx-qt-lib/qstring.h");
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
        type QString = cxx_qt_lib::QString;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[base = QQuickPaintedItem]
        #[qproperty(QString, working_directory, cxx_name = "workingDirectory", READ, WRITE, NOTIFY)]
        /// Program and arguments to run instead of the login shell.
        #[qproperty(QStringList, command, READ, WRITE, NOTIFY)]
        #[qproperty(QString, font_family, cxx_name = "fontFamily", READ, WRITE, NOTIFY)]
        #[qproperty(i32, font_pixel_size, cxx_name = "fontPixelSize", READ, WRITE, NOTIFY)]
        #[qproperty(QString, title, READ, NOTIFY)]
        #[qproperty(bool, running, READ, NOTIFY)]
        #[qproperty(bool, focused, READ, WRITE, NOTIFY)]
        #[qproperty(i32, columns, READ, NOTIFY)]
        #[qproperty(i32, lines, READ, NOTIFY)]
        #[qproperty(i32, display_offset, cxx_name = "displayOffset", READ, NOTIFY)]
        #[qproperty(i32, history_size, cxx_name = "historySize", READ, NOTIFY)]
        #[qproperty(bool, mouse_reporting, cxx_name = "mouseReporting", READ, NOTIFY)]
        type TerminalView = super::TerminalViewRust;

        #[cxx_override]
        unsafe fn paint(self: Pin<&mut TerminalView>, painter: *mut QPainter);

        #[inherit]
        fn update(self: Pin<&mut TerminalView>);
        #[inherit]
        fn width(self: &TerminalView) -> f64;
        #[inherit]
        fn height(self: &TerminalView) -> f64;

        /// Starts the shell (once the item has a size).
        #[qinvokable]
        fn start(self: Pin<&mut TerminalView>) -> bool;

        /// Recomputes rows/columns from the item size and resizes the PTY.
        #[qinvokable]
        fn relayout(self: Pin<&mut TerminalView>);

        /// Handles a key press; returns whether it was consumed.
        #[qinvokable]
        #[cxx_name = "sendKey"]
        fn send_key(self: Pin<&mut TerminalView>, key: i32, modifiers: i32, text: &QString) -> bool;

        /// Sends text as if typed (no bracketed paste).
        #[qinvokable]
        #[cxx_name = "sendText"]
        fn send_text(self: Pin<&mut TerminalView>, text: &QString);

        #[qinvokable]
        fn paste(self: Pin<&mut TerminalView>);

        /// Copies the selection; returns false if nothing is selected.
        #[qinvokable]
        fn copy(self: Pin<&mut TerminalView>) -> bool;

        #[qinvokable]
        #[cxx_name = "hasSelection"]
        fn has_selection(self: &TerminalView) -> bool;

        #[qinvokable]
        #[cxx_name = "clearSelection"]
        fn clear_selection(self: Pin<&mut TerminalView>);

        /// `button`: Qt::MouseButton value; `clicks`: 1–3.
        #[qinvokable]
        #[cxx_name = "mousePress"]
        fn mouse_press(self: Pin<&mut TerminalView>, x: f64, y: f64, button: i32, modifiers: i32, clicks: i32);

        #[qinvokable]
        #[cxx_name = "mouseMove"]
        fn mouse_move(self: Pin<&mut TerminalView>, x: f64, y: f64, buttons: i32, modifiers: i32);

        #[qinvokable]
        #[cxx_name = "mouseRelease"]
        fn mouse_release(self: Pin<&mut TerminalView>, x: f64, y: f64, button: i32, modifiers: i32);

        /// `delta_y` in eighths of a degree (QWheelEvent::angleDelta).
        #[qinvokable]
        fn wheel(self: Pin<&mut TerminalView>, x: f64, y: f64, delta_y: f64, modifiers: i32);

        /// Scrolls the scrollback so that `offset` lines are hidden below.
        #[qinvokable]
        #[cxx_name = "scrollTo"]
        fn scroll_to(self: Pin<&mut TerminalView>, offset: i32);

        /// Selected text ("" if nothing is selected).
        #[qinvokable]
        #[cxx_name = "selectedText"]
        fn selected_text(self: &TerminalView) -> QString;

        /// Screen text plus up to `history` lines of scrollback.
        #[qinvokable]
        #[cxx_name = "screenText"]
        fn screen_text(self: &TerminalView, history: i32) -> QString;

        /// Current working directory of the shell ("" if unknown).
        #[qinvokable]
        #[cxx_name = "currentDirectory"]
        fn current_directory(self: &TerminalView) -> QString;

        #[qsignal]
        fn exited(self: Pin<&mut TerminalView>, code: i32);

        #[qsignal]
        fn bell(self: Pin<&mut TerminalView>);
    }

    impl cxx_qt::Threading for TerminalView {}
    impl cxx_qt::Initialize for TerminalView {}
}

/// Qt::MouseButton values.
const QT_LEFT: i32 = 0x1;
const QT_RIGHT: i32 = 0x2;
const QT_MIDDLE: i32 = 0x4;

struct Metrics {
    cell_width: f64,
    cell_height: f64,
    ascent: f64,
}

pub struct TerminalViewRust {
    working_directory: QString,
    command: QStringList,
    font_family: QString,
    font_pixel_size: i32,
    title: QString,
    running: bool,
    focused: bool,
    columns: i32,
    lines: i32,
    display_offset: i32,
    history_size: i32,
    mouse_reporting: bool,

    terminal: Option<Terminal>,
    font: QFont,
    bold_font: QFont,
    italic_font: QFont,
    metrics: Metrics,
    /// Set while a redraw is queued, so a flood of PTY output results in
    /// one repaint per event loop iteration.
    redraw_queued: Arc<AtomicBool>,
    selecting: bool,
    pressed_button: Option<MouseButton>,
    scroll_remainder: f64,
}

impl Default for TerminalViewRust {
    fn default() -> Self {
        Self {
            working_directory: QString::default(),
            command: QStringList::default(),
            font_family: QString::from("monospace"),
            font_pixel_size: 14,
            title: QString::default(),
            running: false,
            focused: false,
            columns: 0,
            lines: 0,
            display_offset: 0,
            history_size: 0,
            mouse_reporting: false,
            terminal: None,
            font: QFont::default(),
            bold_font: QFont::default(),
            italic_font: QFont::default(),
            metrics: Metrics { cell_width: 8.0, cell_height: 16.0, ascent: 12.0 },
            redraw_queued: Arc::default(),
            selecting: false,
            pressed_button: None,
            scroll_remainder: 0.0,
        }
    }
}

impl cxx_qt::Initialize for qobject::TerminalView {
    fn initialize(mut self: Pin<&mut Self>) {
        self.as_mut().update_font();
        self.as_mut()
            .on_font_family_changed(|t| {
                t.update_font();
            })
            .release();
        self.as_mut()
            .on_font_pixel_size_changed(|t| {
                t.update_font();
            })
            .release();
        self.as_mut().on_focused_changed(|t| t.update()).release();
    }
}

fn color(rgb: Rgb) -> QColor {
    QColor::from_rgb(rgb.r as i32, rgb.g as i32, rgb.b as i32)
}

impl qobject::TerminalView {
    fn update_font(mut self: Pin<&mut Self>) {
        let mut font = QFont::default();
        font.set_family(&self.font_family);
        font.set_pixel_size(self.font_pixel_size.max(6));
        font.set_style_hint(cxx_qt_lib::QFontStyleHint::Monospace, cxx_qt_lib::QFontStyleStrategy::PreferDefault);
        font.set_fixed_pitch(true);
        font.set_kerning(false);
        let mut bold = font.clone();
        bold.set_bold(true);
        let mut italic = font.clone();
        italic.set_italic(true);
        // Integer cell sizes keep glyphs on the pixel grid. Runs of ASCII are
        // drawn with one text call, so each font variant gets a letter
        // spacing that makes its advance exactly one cell; otherwise long
        // lines drift away from the cursor and background cells.
        let cell_width = qt_helpers::font_cell_width(&font).round().max(1.0);
        for f in [&mut font, &mut bold, &mut italic] {
            let advance = qt_helpers::font_cell_width(f);
            f.set_letter_spacing(cxx_qt_lib::QFontSpacingType::AbsoluteSpacing, cell_width - advance);
        }
        let metrics = Metrics {
            cell_width,
            cell_height: qt_helpers::font_line_height(&font).ceil().max(1.0),
            ascent: qt_helpers::font_ascent(&font).round(),
        };
        {
            let mut this = self.as_mut().rust_mut();
            this.font = font;
            this.bold_font = bold;
            this.italic_font = italic;
            this.metrics = metrics;
        }
        self.as_mut().relayout();
        self.update();
    }

    fn grid_size(&self) -> GridSize {
        let m = &self.metrics;
        GridSize {
            cols: ((self.width() / m.cell_width).floor() as u16).max(2),
            lines: ((self.height() / m.cell_height).floor() as u16).max(1),
            cell_width: m.cell_width as u16,
            cell_height: m.cell_height as u16,
        }
    }

    pub fn start(mut self: Pin<&mut Self>) -> bool {
        if self.terminal.is_some() {
            return true;
        }
        if self.width() < 1.0 || self.height() < 1.0 {
            return false;
        }
        let size = self.grid_size();
        let wd = self.working_directory.to_string();
        let mut command = self.command.iter().map(|s| s.to_string());
        let shell = command.next().map(|program| (program, command.collect())).or_else(|| {
            let configured = atlas_core::config::Config::load().terminal.shell;
            (!configured.is_empty() && std::path::Path::new(&configured).exists()).then(|| (configured, Vec::new()))
        });
        let options = Options { working_directory: (!wd.is_empty()).then(|| wd.into()), shell, ..Default::default() };
        let thread = self.qt_thread();
        let redraw_queued = self.redraw_queued.clone();
        let on_event = move |event: TermEvent| {
            if matches!(event, TermEvent::Redraw) && redraw_queued.swap(true, Ordering::AcqRel) {
                return;
            }
            let _ = thread.queue(move |view| view.on_event(event));
        };
        match Terminal::spawn(options, size, on_event) {
            Ok(terminal) => {
                {
                    let mut this = self.as_mut().rust_mut();
                    this.terminal = Some(terminal);
                    this.running = true;
                    this.columns = size.cols as i32;
                    this.lines = size.lines as i32;
                }
                self.as_mut().running_changed();
                self.as_mut().columns_changed();
                self.as_mut().lines_changed();
                self.update();
                true
            }
            Err(err) => {
                log::error!("cannot start terminal: {err:#}");
                false
            }
        }
    }

    fn on_event(mut self: Pin<&mut Self>, event: TermEvent) {
        match event {
            TermEvent::Redraw => {
                self.redraw_queued.store(false, Ordering::Release);
                self.as_mut().sync_scroll_state();
                self.update();
            }
            TermEvent::Title(title) => {
                self.as_mut().rust_mut().title = QString::from(title.as_str());
                self.title_changed();
            }
            TermEvent::Bell => self.bell(),
            TermEvent::ClipboardStore(text) => qt_helpers::clipboard_set_text(&QString::from(text.as_str())),
            TermEvent::Exited(code) => {
                if self.running {
                    {
                        let mut this = self.as_mut().rust_mut();
                        this.running = false;
                        this.terminal = None;
                    }
                    self.as_mut().running_changed();
                    self.exited(code.unwrap_or(-1));
                }
            }
        }
    }

    fn sync_scroll_state(mut self: Pin<&mut Self>) {
        let Some(term) = self.terminal.as_ref() else { return };
        let offset = term.display_offset() as i32;
        let history = term.history_size() as i32;
        let reporting = term.mode().intersects(Mode::MOUSE_MODE);
        if offset != self.display_offset {
            self.as_mut().rust_mut().display_offset = offset;
            self.as_mut().display_offset_changed();
        }
        if history != self.history_size {
            self.as_mut().rust_mut().history_size = history;
            self.as_mut().history_size_changed();
        }
        if reporting != self.mouse_reporting {
            self.as_mut().rust_mut().mouse_reporting = reporting;
            self.as_mut().mouse_reporting_changed();
        }
    }

    pub fn relayout(mut self: Pin<&mut Self>) {
        if self.width() < 1.0 || self.height() < 1.0 {
            return;
        }
        let size = self.grid_size();
        if let Some(term) = self.terminal.as_ref() {
            term.resize(size);
        }
        if self.columns != size.cols as i32 || self.lines != size.lines as i32 {
            self.as_mut().rust_mut().columns = size.cols as i32;
            self.as_mut().rust_mut().lines = size.lines as i32;
            self.as_mut().columns_changed();
            self.as_mut().lines_changed();
        }
        self.update();
    }

    pub fn send_key(mut self: Pin<&mut Self>, key: i32, modifiers: i32, text: &QString) -> bool {
        let Some(term) = self.terminal.as_ref() else { return false };
        let mods = Mods::from_qt(modifiers);
        // Shift+PageUp/Down scroll the scrollback, like in most terminals.
        if mods.shift && !mods.ctrl && (key == input::qt_key::PageUp || key == input::qt_key::PageDown) {
            let page = self.lines.max(1) - 1;
            term.scroll(if key == input::qt_key::PageUp { page } else { -page });
            self.as_mut().sync_scroll_state();
            self.update();
            return true;
        }
        match input::encode_key(key, mods, &text.to_string(), term.mode()) {
            Some(bytes) => {
                term.write(bytes);
                true
            }
            None => false,
        }
    }

    pub fn send_text(self: Pin<&mut Self>, text: &QString) {
        if let Some(term) = self.terminal.as_ref() {
            term.write(text.to_string().into_bytes());
        }
    }

    pub fn paste(self: Pin<&mut Self>) {
        let Some(term) = self.terminal.as_ref() else { return };
        let text = qt_helpers::clipboard_text().to_string();
        if !text.is_empty() {
            term.write(input::encode_paste(&text, term.mode()));
        }
    }

    pub fn copy(self: Pin<&mut Self>) -> bool {
        match self.terminal.as_ref().and_then(Terminal::selection_text) {
            Some(text) => {
                qt_helpers::clipboard_set_text(&QString::from(text.as_str()));
                true
            }
            None => false,
        }
    }

    pub fn has_selection(&self) -> bool {
        self.terminal.as_ref().and_then(Terminal::selection_text).is_some()
    }

    pub fn clear_selection(self: Pin<&mut Self>) {
        if let Some(term) = self.terminal.as_ref() {
            term.select_clear();
        }
        self.update();
    }

    /// Viewport cell under a point, and whether it is in the right half.
    fn cell_at(&self, x: f64, y: f64) -> (usize, usize, bool) {
        let m = &self.metrics;
        let cols = self.columns.max(1) as usize;
        let lines = self.lines.max(1) as usize;
        let fx = (x / m.cell_width).max(0.0);
        let col = (fx.floor() as usize).min(cols - 1);
        let line = ((y / m.cell_height).max(0.0).floor() as usize).min(lines - 1);
        (col, line, fx.fract() >= 0.5)
    }

    /// Whether mouse events go to the application (mouse tracking enabled
    /// and Shift not held – Shift forces local selection, as in xterm).
    fn reports_mouse(&self, mods: Mods) -> bool {
        !mods.shift && self.terminal.as_ref().is_some_and(|t| t.mode().intersects(Mode::MOUSE_MODE))
    }

    fn report(&self, button: MouseButton, action: MouseAction, x: f64, y: f64, mods: Mods) {
        let Some(term) = self.terminal.as_ref() else { return };
        let (col, line, _) = self.cell_at(x, y);
        if let Some(bytes) = input::encode_mouse(button, action, col, line, mods, term.mode()) {
            // Mouse reports must not reset the scroll position or selection.
            term.write_raw(bytes);
        }
    }

    pub fn mouse_press(mut self: Pin<&mut Self>, x: f64, y: f64, button: i32, modifiers: i32, clicks: i32) {
        let mods = Mods::from_qt(modifiers);
        let button = match button {
            QT_LEFT => MouseButton::Left,
            QT_RIGHT => MouseButton::Right,
            QT_MIDDLE => MouseButton::Middle,
            _ => return,
        };
        if self.reports_mouse(mods) {
            self.as_mut().rust_mut().pressed_button = Some(button);
            self.report(button, MouseAction::Press, x, y, mods);
            return;
        }
        match button {
            MouseButton::Left => {
                let (col, line, right) = self.cell_at(x, y);
                if let Some(term) = self.terminal.as_ref() {
                    term.select_start(col, line, right, clicks.clamp(1, 3) as u8);
                }
                self.as_mut().rust_mut().selecting = true;
                self.update();
            }
            // Middle click pastes, like in other Linux terminals.
            MouseButton::Middle => self.paste(),
            _ => {}
        }
    }

    pub fn mouse_move(self: Pin<&mut Self>, x: f64, y: f64, buttons: i32, modifiers: i32) {
        let mods = Mods::from_qt(modifiers);
        if self.selecting {
            let (col, line, right) = self.cell_at(x, y);
            if let Some(term) = self.terminal.as_ref() {
                term.select_update(col, line, right);
            }
            self.update();
            return;
        }
        if self.reports_mouse(mods) {
            let button = if buttons & QT_LEFT != 0 {
                MouseButton::Left
            } else if buttons & QT_MIDDLE != 0 {
                MouseButton::Middle
            } else if buttons & QT_RIGHT != 0 {
                MouseButton::Right
            } else {
                MouseButton::None
            };
            self.report(button, MouseAction::Motion, x, y, mods);
        }
    }

    pub fn mouse_release(mut self: Pin<&mut Self>, x: f64, y: f64, _button: i32, modifiers: i32) {
        let mods = Mods::from_qt(modifiers);
        if self.selecting {
            self.as_mut().rust_mut().selecting = false;
            return;
        }
        if let Some(button) = self.as_mut().rust_mut().pressed_button.take() {
            self.report(button, MouseAction::Release, x, y, mods);
        }
    }

    pub fn wheel(mut self: Pin<&mut Self>, x: f64, y: f64, delta_y: f64, modifiers: i32) {
        let mods = Mods::from_qt(modifiers);
        // 120 units = one notch = 3 lines.
        let lines_f = delta_y / 40.0 + self.scroll_remainder;
        let lines = lines_f.trunc() as i32;
        self.as_mut().rust_mut().scroll_remainder = lines_f.fract();
        if lines == 0 {
            return;
        }
        let Some(term) = self.terminal.as_ref() else { return };
        let mode = term.mode();
        if self.reports_mouse(mods) {
            let button = if lines > 0 { MouseButton::WheelUp } else { MouseButton::WheelDown };
            for _ in 0..lines.unsigned_abs() {
                self.report(button, MouseAction::Press, x, y, mods);
            }
        } else if mode.contains(Mode::ALT_SCREEN) && mode.contains(Mode::ALTERNATE_SCROLL) {
            // Full screen programs without mouse support (less, man) get
            // cursor keys instead of a scrolled viewport.
            let seq: &[u8] = match (lines > 0, mode.contains(Mode::APP_CURSOR)) {
                (true, true) => b"\x1bOA",
                (true, false) => b"\x1b[A",
                (false, true) => b"\x1bOB",
                (false, false) => b"\x1b[B",
            };
            term.write_raw(seq.repeat(lines.unsigned_abs() as usize));
        } else {
            term.scroll(lines);
            self.as_mut().sync_scroll_state();
            self.update();
        }
    }

    pub fn selected_text(&self) -> QString {
        let text = self.terminal.as_ref().and_then(Terminal::selection_text).unwrap_or_default();
        QString::from(text.as_str())
    }

    pub fn screen_text(&self, history: i32) -> QString {
        let text = self.terminal.as_ref().map(|t| t.text(history.max(0) as usize)).unwrap_or_default();
        QString::from(text.as_str())
    }

    pub fn current_directory(&self) -> QString {
        self.terminal
            .as_ref()
            .and_then(Terminal::current_dir)
            .map(|d| QString::from(d.to_string_lossy().as_ref()))
            .unwrap_or_default()
    }

    pub fn scroll_to(mut self: Pin<&mut Self>, offset: i32) {
        let Some(term) = self.terminal.as_ref() else { return };
        let current = term.display_offset() as i32;
        term.scroll(offset.max(0) - current);
        self.as_mut().sync_scroll_state();
        self.update();
    }

    unsafe fn paint(self: Pin<&mut Self>, painter: *mut cxx_qt_lib::QPainter) {
        // SAFETY: Qt passes a valid, active painter for the duration of paint().
        let Some(painter) = (unsafe { painter.as_mut() }) else { return };
        let mut painter = unsafe { Pin::new_unchecked(painter) };
        let (w, h) = (self.width(), self.height());
        let Some(term) = self.terminal.as_ref() else {
            painter.fill_rect(&QRectF::new(0.0, 0.0, w, h), &color(atlas_core::terminal::colors::background()));
            return;
        };
        let frame = term.frame(self.focused);
        self.paint_frame(painter.as_mut(), &frame, w, h);
    }

    fn paint_frame(&self, mut painter: Pin<&mut cxx_qt_lib::QPainter>, frame: &Frame, w: f64, h: f64) {
        let m = &self.metrics;
        painter.as_mut().fill_rect(&QRectF::new(0.0, 0.0, w, h), &color(frame.background));

        for (line_index, line) in frame.lines.iter().enumerate() {
            let top = line_index as f64 * m.cell_height;
            for bg in &line.backgrounds {
                let rect =
                    QRectF::new(bg.col as f64 * m.cell_width, top, bg.cells as f64 * m.cell_width, m.cell_height);
                painter.as_mut().fill_rect(&rect, &color(bg.color));
            }
        }

        if let Some(cursor) = &frame.cursor {
            let x = cursor.col as f64 * m.cell_width;
            let y = cursor.line as f64 * m.cell_height;
            let cw = m.cell_width * if cursor.wide { 2.0 } else { 1.0 };
            let c = color(frame.cursor_color);
            match cursor.shape {
                CursorShape::Block => painter.as_mut().fill_rect(&QRectF::new(x, y, cw, m.cell_height), &c),
                CursorShape::Beam => painter.as_mut().fill_rect(&QRectF::new(x, y, 2.0, m.cell_height), &c),
                CursorShape::Underline => {
                    painter.as_mut().fill_rect(&QRectF::new(x, y + m.cell_height - 2.0, cw, 2.0), &c)
                }
                CursorShape::HollowBlock => {
                    painter.as_mut().fill_rect(&QRectF::new(x, y, cw, 1.0), &c);
                    painter.as_mut().fill_rect(&QRectF::new(x, y + m.cell_height - 1.0, cw, 1.0), &c);
                    painter.as_mut().fill_rect(&QRectF::new(x, y, 1.0, m.cell_height), &c);
                    painter.as_mut().fill_rect(&QRectF::new(x + cw - 1.0, y, 1.0, m.cell_height), &c);
                }
                CursorShape::Hidden => {}
            }
        }
        let block_cursor = frame.cursor.as_ref().filter(|c| c.shape == CursorShape::Block).map(|c| (c.line, c.col));

        let mut current_font = 0u8; // 0 regular, 1 bold, 2 italic
        painter.as_mut().set_font(&self.font);
        for (line_index, line) in frame.lines.iter().enumerate() {
            let top = line_index as f64 * m.cell_height;
            let baseline = (top + m.ascent) as i32;
            for run in &line.runs {
                let wanted = if run.bold {
                    1
                } else if run.italic {
                    2
                } else {
                    0
                };
                if wanted != current_font {
                    painter.as_mut().set_font(match wanted {
                        1 => &self.bold_font,
                        2 => &self.italic_font,
                        _ => &self.font,
                    });
                    current_font = wanted;
                }
                // Text under a block cursor is drawn in the background color.
                let under_cursor = block_cursor
                    .is_some_and(|(l, c)| l as usize == line_index && c >= run.col && c < run.col + run.cells);
                let fg = if under_cursor && run.cells == 1 { frame.background } else { run.fg };
                let mut pen = cxx_qt_lib::QPen::default();
                pen.set_color(&color(fg));
                painter.as_mut().set_pen(&pen);
                let x = (run.col as f64 * m.cell_width) as i32;
                painter.as_mut().draw_text(&QPoint::new(x, baseline), &QString::from(run.text.as_str()));
                if run.underline || run.strikeout {
                    let width = run.cells as f64 * m.cell_width;
                    let y = if run.underline { top + m.ascent + 2.0 } else { top + m.cell_height / 2.0 };
                    painter
                        .as_mut()
                        .fill_rect(&QRectF::new(x as f64, y.min(top + m.cell_height - 1.0), width, 1.0), &color(fg));
                }
            }
        }
    }
}
