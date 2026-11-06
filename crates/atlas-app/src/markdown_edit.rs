// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Editing help for Markdown notes for QML (singleton `MarkdownEditCore`),
//! from `atlas_core::markdown_edit`: lists, tables, tasks, completion.
//!
//! One entry point, `call(op, args)`, JSON in and out; positions are UTF-16
//! like QML strings. Operations that change the text return an edit
//! `{start, end, text, anchor, cursor}` (or `null`: the key does what it
//! usually does), which `TextEditor.qml` applies.

use anyhow::{Context, Result, bail};
use atlas_core::markdown_edit as me;
use cxx_qt_lib::QString;
use serde_json::{Value, json};

use crate::util::qs;

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
        type MarkdownEditCore = super::MarkdownEditCoreRust;

        /// Runs operation `op` with `args` (JSON object) and returns JSON
        /// (`null` when nothing applies or on errors, which are logged).
        #[qinvokable]
        fn call(self: &MarkdownEditCore, op: &QString, args: &QString) -> QString;
    }
}

#[derive(Default)]
pub struct MarkdownEditCoreRust;

impl qobject::MarkdownEditCore {
    pub fn call(&self, op: &QString, args: &QString) -> QString {
        let op = op.to_string();
        match dispatch(&op, &args.to_string()) {
            Ok(value) => qs(value.to_string()),
            Err(err) => {
                log::warn!("MarkdownEditCore.call({op}): {err:#}");
                qs("null")
            }
        }
    }
}

struct Args(Value);

impl Args {
    fn str(&self, key: &str) -> &str {
        self.0.get(key).and_then(Value::as_str).unwrap_or("")
    }
    fn num(&self, key: &str) -> usize {
        self.0.get(key).and_then(Value::as_u64).unwrap_or(0) as usize
    }
    fn flag(&self, key: &str) -> bool {
        self.0.get(key).and_then(Value::as_bool).unwrap_or(false)
    }
}

pub fn dispatch(op: &str, args: &str) -> Result<Value> {
    let a = Args(serde_json::from_str(if args.trim().is_empty() { "{}" } else { args }).context("args")?);
    let text = a.str("text");
    Ok(match op {
        "indentListItems" => json!(me::indent_list_items(text, a.num("anchor"), a.num("cursor"), a.flag("outdent"))),
        "continueList" => json!(me::continue_list(text, a.num("pos"))),
        "toggleTask" => json!(me::toggle_task(text, a.num("line"), a.flag("done"))),
        "alignTable" => json!(me::align_table(a.str("markdown"))),
        "tableAt" => {
            let b = me::byte_at(text, a.num("pos"));
            json!(me::table_at(text, b).map(|t| json!({
                "start": me::utf16_at(text, t.start),
                "end": me::utf16_at(text, t.end),
            })))
        }
        "tableTab" => json!(me::table_tab(text, a.num("pos"), a.flag("back"))),
        "tableEnter" => json!(me::table_enter(text, a.num("pos"))),
        "alignTableAt" => json!(me::align_table_at(text, a.num("pos"), a.num("cursor"))),
        "alignTables" => json!(me::align_tables(text, a.num("cursor"))),
        // Very large notes are not drawn as tables (the text stays).
        "drawnTables" => json!(if text.len() > 2_000_000 { Vec::new() } else { me::drawn_tables(text) }),
        "completion" => json!(me::completion_at(text, a.num("pos"))),
        "tagSuggestions" => {
            let tags: Vec<me::Tag> =
                serde_json::from_value(a.0.get("tags").cloned().unwrap_or(json!([]))).context("tags")?;
            json!(me::tag_suggestions(&tags, a.str("typed")))
        }
        "acceptTag" => json!(me::accept_tag(text, a.num("start"), a.num("end"), a.str("tag"))),
        "acceptWiki" => json!(me::accept_wiki(text, a.num("start"), a.num("end"), a.str("relative"))),
        _ => bail!("unknown operation"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_round_trip() {
        let e = dispatch("continueList", r#"{"text": "- eins", "pos": 6}"#).unwrap();
        assert_eq!(e["text"], "\n- ");
        assert_eq!(e["cursor"], 9);
        assert!(dispatch("continueList", r#"{"text": "Text", "pos": 4}"#).unwrap().is_null());
        let t = dispatch("tableAt", r#"{"text": "x\n| a |\n|---|", "pos": 3}"#).unwrap();
        assert_eq!(t, json!({"start": 2, "end": 13}));
        assert_eq!(dispatch("alignTable", r#"{"markdown": "| a |\n|-|"}"#).unwrap(), "| a   |\n| --- |");
        assert!(dispatch("nope", "{}").is_err());
    }
}
