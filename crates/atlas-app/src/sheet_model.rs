// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! One sheet of a spreadsheet as a table model for the sheet view
//! (`SheetView.qml`). Sheets are read on a background thread
//! ([`atlas_core::spreadsheet`]); the file is never written.

use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use atlas_core::spreadsheet::{self, Align, CellKind, SheetData};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QByteArray, QHash, QHashPair_i32_QByteArray, QModelIndex, QString, QVariant};
use serde_json::json;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!(<QtCore/QAbstractTableModel>);
        type QAbstractTableModel;

        include!("cxx-qt-lib/qhash.h");
        type QHash_i32_QByteArray = cxx_qt_lib::QHash<cxx_qt_lib::QHashPair_i32_QByteArray>;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
        include!("cxx-qt-lib/qmodelindex.h");
        type QModelIndex = cxx_qt_lib::QModelIndex;
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[base = QAbstractTableModel]
        /// The spreadsheet file.
        #[qproperty(QString, source, READ, WRITE = set_source, NOTIFY)]
        /// Index of the sheet shown.
        #[qproperty(i32, sheet, READ, WRITE = set_sheet, NOTIFY)]
        /// JSON `[{name, hidden}]` of the workbook's sheets.
        #[qproperty(QString, sheets, READ, NOTIFY)]
        #[qproperty(i32, rows, READ, NOTIFY)]
        #[qproperty(i32, columns, READ, NOTIFY)]
        #[qproperty(bool, loading, READ, NOTIFY)]
        #[qproperty(QString, error, READ, NOTIFY)]
        /// The sheet has more cells than are shown.
        #[qproperty(bool, truncated, READ, NOTIFY)]
        /// Counts loads (for the view and the smoke test).
        #[qproperty(i32, revision, READ, NOTIFY)]
        type SheetModel = super::SheetModelRust;

        #[cxx_name = "setSource"]
        fn set_source(self: Pin<&mut SheetModel>, source: QString);
        #[cxx_name = "setSheet"]
        fn set_sheet(self: Pin<&mut SheetModel>, sheet: i32);

        /// Reads the file again (changed on disk).
        #[qinvokable]
        fn reload(self: Pin<&mut SheetModel>);

        #[qinvokable]
        #[cxx_name = "textAt"]
        fn text_at(self: &SheetModel, row: i32, column: i32) -> QString;

        /// The formula of a cell without "=" ("" if it has none).
        #[qinvokable]
        #[cxx_name = "formulaAt"]
        fn formula_at(self: &SheetModel, row: i32, column: i32) -> QString;

        /// "A" … "Z", "AA" …
        #[qinvokable]
        #[cxx_name = "columnName"]
        fn column_name(self: &SheetModel, column: i32) -> QString;

        /// "B5".
        #[qinvokable]
        #[cxx_name = "cellName"]
        fn cell_name(self: &SheetModel, row: i32, column: i32) -> QString;

        /// Widths of the columns in characters (JSON array), from their texts.
        #[qinvokable]
        #[cxx_name = "columnWidths"]
        fn column_widths(self: &SheetModel) -> QString;

        /// The cells of a rectangle as tab-separated text (for the clipboard).
        #[qinvokable]
        #[cxx_name = "copyText"]
        fn copy_text(self: &SheetModel, top: i32, left: i32, bottom: i32, right: i32) -> QString;

        /// The next cell after (`row`, `column`) whose text contains `text`
        /// (case-insensitive), row by row, wrapping around; `backwards`
        /// searches upwards. JSON `{row, column, count}` (`count`: cells
        /// found in the sheet) or `{}`.
        #[qinvokable]
        fn find(self: &SheetModel, text: &QString, row: i32, column: i32, backwards: bool) -> QString;
    }

    unsafe extern "RustQt" {
        #[inherit]
        #[cxx_name = "beginResetModel"]
        fn begin_reset_model(self: Pin<&mut SheetModel>);
        #[inherit]
        #[cxx_name = "endResetModel"]
        fn end_reset_model(self: Pin<&mut SheetModel>);

        #[cxx_override]
        fn data(self: &SheetModel, index: &QModelIndex, role: i32) -> QVariant;
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names(self: &SheetModel) -> QHash_i32_QByteArray;
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count(self: &SheetModel, parent: &QModelIndex) -> i32;
        #[cxx_override]
        #[cxx_name = "columnCount"]
        fn column_count(self: &SheetModel, parent: &QModelIndex) -> i32;
    }

    impl cxx_qt::Threading for SheetModel {}
}

const ROLE_DISPLAY: i32 = 0;
const ROLE_KIND: i32 = 0x0101;
const ROLE_BOLD: i32 = 0x0102;
const ROLE_ITALIC: i32 = 0x0103;
const ROLE_UNDERLINE: i32 = 0x0104;
const ROLE_STRIKE: i32 = 0x0105;
const ROLE_COLOR: i32 = 0x0106;
const ROLE_COLOR_DARK: i32 = 0x0107;
const ROLE_FILL: i32 = 0x0108;
const ROLE_ALIGN: i32 = 0x0109;
const ROLE_WRAP: i32 = 0x010a;
const ROLE_FORMULA: i32 = 0x010b;

/// The minimum shown: an empty sheet still has a few cells.
const MIN_ROWS: u32 = 30;
const MIN_COLUMNS: u32 = 8;

#[derive(Default)]
pub struct SheetModelRust {
    source: QString,
    sheet: i32,
    sheets: QString,
    rows: i32,
    columns: i32,
    loading: bool,
    error: QString,
    truncated: bool,
    revision: i32,
    data: Option<Arc<SheetData>>,
    /// Number of the load in flight; older results are dropped.
    generation: u64,
    /// The names of the sheets still have to be read (a newer load that
    /// replaces the first one must not lose them).
    need_list: bool,
}

fn hex(color: u32) -> String {
    format!("#{color:06x}")
}

/// A text colour for the dark design: dark colours (meant for white)
/// lightened, keeping their hue.
fn on_dark(color: u32) -> u32 {
    let (r, g, b) = atlas_core::atlasnote::rgb(color);
    let luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    if luminance >= 0.45 {
        return color;
    }
    let (r, g, b) = atlas_core::atlasnote::light_ink(r, g, b);
    let byte = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    (byte(r) << 16) | (byte(g) << 8) | byte(b)
}

impl qobject::SheetModel {
    pub fn set_source(mut self: Pin<&mut Self>, source: QString) {
        if self.source == source {
            return;
        }
        self.as_mut().rust_mut().source = source;
        self.as_mut().source_changed();
        self.as_mut().rust_mut().sheet = 0;
        self.as_mut().sheet_changed();
        self.reload();
    }

    pub fn set_sheet(mut self: Pin<&mut Self>, sheet: i32) {
        if self.sheet == sheet {
            return;
        }
        self.as_mut().rust_mut().sheet = sheet.max(0);
        self.as_mut().sheet_changed();
        self.load(false);
    }

    pub fn reload(self: Pin<&mut Self>) {
        self.load(true);
    }

    /// Reads the sheet (and with `list` the names of all sheets) in the background.
    fn load(mut self: Pin<&mut Self>, list: bool) {
        if list {
            self.as_mut().rust_mut().need_list = true;
        }
        let list = self.need_list;
        let path = PathBuf::from(self.source.to_string());
        let sheet = self.sheet.max(0) as usize;
        let generation = self.generation + 1;
        self.as_mut().rust_mut().generation = generation;
        self.as_mut().rust_mut().loading = true;
        self.as_mut().loading_changed();
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let names = if list { Some(spreadsheet::sheets(&path)) } else { None };
            let result = spreadsheet::load_sheet(&path, sheet);
            let _ = thread.queue(move |mut model| {
                if model.generation != generation {
                    return;
                }
                if let Some(names) = names {
                    let list = match &names {
                        Ok(list) => {
                            json!(
                                list.iter().map(|s| json!({ "name": s.name, "hidden": s.hidden })).collect::<Vec<_>>()
                            )
                        }
                        Err(_) => json!([]),
                    };
                    model.as_mut().rust_mut().sheets = QString::from(list.to_string().as_str());
                    model.as_mut().rust_mut().need_list = false;
                    model.as_mut().sheets_changed();
                }
                model.as_mut().begin_reset_model();
                let error = match result {
                    Ok(data) => {
                        let mut this = model.as_mut().rust_mut();
                        this.rows = data.rows.max(MIN_ROWS) as i32;
                        this.columns = data.cols.max(MIN_COLUMNS) as i32;
                        this.truncated = data.truncated;
                        this.data = Some(Arc::new(data));
                        String::new()
                    }
                    Err(err) => {
                        let mut this = model.as_mut().rust_mut();
                        this.rows = 0;
                        this.columns = 0;
                        this.truncated = false;
                        this.data = None;
                        format!("{err:#}")
                    }
                };
                model.as_mut().end_reset_model();
                let revision = model.revision + 1;
                {
                    let mut this = model.as_mut().rust_mut();
                    this.error = QString::from(error.as_str());
                    this.loading = false;
                    this.revision = revision;
                }
                model.as_mut().rows_changed();
                model.as_mut().columns_changed();
                model.as_mut().truncated_changed();
                model.as_mut().error_changed();
                model.as_mut().loading_changed();
                model.as_mut().revision_changed();
            });
        });
    }

    fn cell(&self, row: i32, column: i32) -> Option<&spreadsheet::Cell> {
        self.data.as_ref()?.cell(u32::try_from(row).ok()?, u32::try_from(column).ok()?)
    }

    pub fn text_at(&self, row: i32, column: i32) -> QString {
        QString::from(self.cell(row, column).map(|c| c.text.as_str()).unwrap_or_default())
    }

    pub fn formula_at(&self, row: i32, column: i32) -> QString {
        let (Ok(r), Ok(c)) = (u32::try_from(row), u32::try_from(column)) else { return QString::default() };
        QString::from(self.data.as_ref().and_then(|d| d.formulas.get(&(r, c))).map(String::as_str).unwrap_or_default())
    }

    pub fn column_name(&self, column: i32) -> QString {
        QString::from(spreadsheet::column_name(column.max(0) as u32).as_str())
    }

    pub fn cell_name(&self, row: i32, column: i32) -> QString {
        QString::from(spreadsheet::cell_name(row.max(0) as u32, column.max(0) as u32).as_str())
    }

    pub fn column_widths(&self) -> QString {
        let columns = self.columns.max(0) as usize;
        let mut widths = vec![0usize; columns];
        if let Some(data) = &self.data {
            for row in data.cells.iter().take(3000) {
                for (c, cell) in row.iter().enumerate() {
                    if let Some(w) = widths.get_mut(c) {
                        let longest = cell.text.lines().map(|l| l.chars().count()).max().unwrap_or(0);
                        *w = (*w).max(longest);
                    }
                }
            }
        }
        let widths: Vec<usize> = widths.into_iter().map(|w| w.clamp(4, 48)).collect();
        QString::from(json!(widths).to_string().as_str())
    }

    pub fn copy_text(&self, top: i32, left: i32, bottom: i32, right: i32) -> QString {
        let (top, bottom) = (top.min(bottom).max(0), top.max(bottom).max(0));
        let (left, right) = (left.min(right).max(0), left.max(right).max(0));
        let mut out = String::new();
        for r in top..=bottom.min(top + 100_000) {
            let row: Vec<String> = (left..=right.min(left + 1000))
                .map(|c| {
                    let text = self.cell(r, c).map(|c| c.text.as_str()).unwrap_or_default();
                    // Tabs and line breaks would split the cell when pasted.
                    if text.contains(['\t', '\n', '"']) {
                        format!("\"{}\"", text.replace('"', "\"\""))
                    } else {
                        text.to_owned()
                    }
                })
                .collect();
            out.push_str(&row.join("\t"));
            if r < bottom {
                out.push('\n');
            }
        }
        QString::from(out.as_str())
    }

    pub fn find(&self, text: &QString, row: i32, column: i32, backwards: bool) -> QString {
        let needle = text.to_string().to_lowercase();
        let Some(data) = &self.data else { return QString::from("{}") };
        if needle.is_empty() {
            return QString::from("{}");
        }
        let mut hits: Vec<(i32, i32)> = Vec::new();
        for (r, cells) in data.cells.iter().enumerate() {
            for (c, cell) in cells.iter().enumerate() {
                if cell.text.to_lowercase().contains(&needle) {
                    hits.push((r as i32, c as i32));
                }
            }
        }
        let here = (row, column);
        let next = if backwards {
            hits.iter().rev().find(|&&h| h < here).or(hits.last())
        } else {
            hits.iter().find(|&&h| h > here).or(hits.first())
        };
        QString::from(match next {
            Some(&(r, c)) => json!({ "row": r, "column": c, "count": hits.len() }).to_string(),
            None => "{}".to_owned(),
        })
    }

    pub fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let Some(data) = &self.data else { return QVariant::default() };
        let Some(cell) = self.cell(index.row(), index.column()) else {
            return match role {
                ROLE_DISPLAY | ROLE_KIND | ROLE_COLOR | ROLE_COLOR_DARK | ROLE_FILL => {
                    QVariant::from(&QString::default())
                }
                ROLE_ALIGN => QVariant::from(&0i32),
                ROLE_BOLD | ROLE_ITALIC | ROLE_UNDERLINE | ROLE_STRIKE | ROLE_WRAP | ROLE_FORMULA => {
                    QVariant::from(&false)
                }
                _ => QVariant::default(),
            };
        };
        let style = data.style(cell);
        let text = |s: &str| QVariant::from(&QString::from(s));
        match role {
            ROLE_DISPLAY => text(&cell.text),
            ROLE_KIND => text(match cell.kind {
                CellKind::Empty => "",
                CellKind::Text => "text",
                CellKind::Number => "number",
                CellKind::Date => "date",
                CellKind::Bool => "bool",
                CellKind::Error => "error",
            }),
            ROLE_BOLD => QVariant::from(&style.bold),
            ROLE_ITALIC => QVariant::from(&style.italic),
            ROLE_UNDERLINE => QVariant::from(&style.underline),
            ROLE_STRIKE => QVariant::from(&style.strike),
            ROLE_COLOR => text(&style.color.map(hex).unwrap_or_default()),
            ROLE_COLOR_DARK => text(&style.color.map(on_dark).map(hex).unwrap_or_default()),
            ROLE_FILL => text(&style.fill.map(hex).unwrap_or_default()),
            ROLE_ALIGN => QVariant::from(&match style.align {
                Align::Left => 0i32,
                Align::Center => 1,
                Align::Right => 2,
                Align::Auto => match cell.kind {
                    CellKind::Number | CellKind::Date => 2,
                    CellKind::Bool | CellKind::Error => 1,
                    _ => 0,
                },
            }),
            ROLE_WRAP => QVariant::from(&style.wrap),
            ROLE_FORMULA => QVariant::from(
                &(index.row() >= 0
                    && index.column() >= 0
                    && data.formulas.contains_key(&(index.row() as u32, index.column() as u32))),
            ),
            _ => QVariant::default(),
        }
    }

    pub fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        let mut roles = QHash::<QHashPair_i32_QByteArray>::default();
        for (role, name) in [
            (ROLE_DISPLAY, "display"),
            (ROLE_KIND, "kind"),
            (ROLE_BOLD, "bold"),
            (ROLE_ITALIC, "italic"),
            (ROLE_UNDERLINE, "underline"),
            (ROLE_STRIKE, "strike"),
            (ROLE_COLOR, "textColor"),
            (ROLE_COLOR_DARK, "textColorDark"),
            (ROLE_FILL, "fill"),
            (ROLE_ALIGN, "align"),
            (ROLE_WRAP, "wrap"),
            (ROLE_FORMULA, "hasFormula"),
        ] {
            roles.insert(role, QByteArray::from(name));
        }
        roles
    }

    pub fn row_count(&self, _parent: &QModelIndex) -> i32 {
        self.rows
    }

    pub fn column_count(&self, _parent: &QModelIndex) -> i32 {
        self.columns
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_text_colours_are_lightened() {
        assert_eq!(on_dark(0xffff00), 0xffff00, "light colours stay");
        assert_eq!(on_dark(0x000000), 0xffffff);
        let red = on_dark(0xc00000);
        assert!(red >> 16 > 0xc0 && (red >> 8 & 0xff) > 0x30, "{red:06x}");
    }
}
