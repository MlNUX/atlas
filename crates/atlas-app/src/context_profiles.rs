// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rules of the context bar's profiles for QML (singleton
//! `ContextProfilesCore`), from `atlas_core::context_profiles`.
//! `ContextProfiles.qml` wraps it with the texts of the interface.
//!
//! One entry point, `call(op, args)`, with JSON in and out keeps the bridge
//! small; operations that change lists take `lists` and return
//! `{"lists": …, "result": …}`.

use anyhow::{Context, Result, bail};
use atlas_core::config::ContextConfig;
use atlas_core::context_profiles as cp;
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
        type ContextProfilesCore = super::ContextProfilesCoreRust;

        /// Runs operation `op` with `args` (JSON object) and returns JSON
        /// (`null` on errors, which are logged).
        #[qinvokable]
        fn call(self: &ContextProfilesCore, op: &QString, args: &QString) -> QString;
    }
}

#[derive(Default)]
pub struct ContextProfilesCoreRust;

impl qobject::ContextProfilesCore {
    pub fn call(&self, op: &QString, args: &QString) -> QString {
        let op = op.to_string();
        match dispatch(&op, &args.to_string()) {
            Ok(value) => qs(value.to_string()),
            Err(err) => {
                log::warn!("ContextProfilesCore.call({op}): {err:#}");
                qs("null")
            }
        }
    }
}

/// Arguments of an operation (JSON object; missing fields are empty).
struct Args(Value);

impl Args {
    fn str(&self, key: &str) -> &str {
        self.0.get(key).and_then(Value::as_str).unwrap_or("")
    }

    fn config(&self) -> Result<ContextConfig> {
        match self.0.get("config") {
            Some(v) if !v.is_null() => serde_json::from_value(v.clone()).context("config"),
            _ => Ok(ContextConfig::default()),
        }
    }

    fn lists(&self) -> Result<cp::Lists> {
        serde_json::from_value(self.0.get("lists").cloned().context("lists missing")?).context("lists")
    }

    fn extensions(&self) -> Result<Vec<cp::Extension>> {
        match self.0.get("extensions") {
            Some(v) if !v.is_null() => serde_json::from_value(v.clone()).context("extensions"),
            _ => Ok(Vec::new()),
        }
    }

    fn visible(&self) -> Option<Vec<String>> {
        self.0.get("visible").and_then(|v| serde_json::from_value(v.clone()).ok())
    }
}

fn changed(lists: &cp::Lists, result: Value) -> Result<Value> {
    Ok(json!({ "lists": lists, "result": result }))
}

pub fn dispatch(op: &str, args: &str) -> Result<Value> {
    let a = Args(serde_json::from_str(if args.trim().is_empty() { "{}" } else { args }).context("args")?);
    Ok(match op {
        "catalog" => json!({
            "blocks": cp::CATALOG,
            "groups": cp::GROUPS,
            "names": cp::NAMES,
            "keys": cp::KEYS,
            "fullBlocks": cp::FULL_BLOCKS,
            "builtin": cp::Lists::builtin(),
        }),
        "kindOf" => json!(cp::kind_of(a.str("kind"))),
        "isSet" => json!(cp::is_set(&a.config()?, a.str("name"))),
        "profileFor" => json!(cp::profile_for(&a.config()?, a.str("kind"))),
        "base" => json!(cp::base(&a.config()?)),
        "lists" => json!(cp::lists(&a.config()?, a.str("name"))),
        "layout" => json!(cp::layout(&a.config()?, a.str("name"), &a.extensions()?)),
        "pageOf" => json!(cp::page_of(&a.lists()?, a.str("id"))),
        "save" => {
            let l = a.lists()?;
            json!(cp::save(&a.config()?, a.str("name"), &l))
        }
        "commit" => {
            let l = a.lists()?;
            json!(cp::commit(&a.config()?, a.str("name"), &l))
        }
        "areaList" => {
            let mut l = a.lists()?;
            cp::area_list(&mut l, a.str("area"));
            changed(&l, Value::Null)?
        }
        "newPage" => {
            let mut l = a.lists()?;
            let id = cp::new_page(&mut l, a.str("title"), &cp::page_suffix_now());
            changed(&l, json!(id))?
        }
        "renamePage" => {
            let mut l = a.lists()?;
            cp::rename_page(&mut l, a.str("id"), a.str("title"));
            changed(&l, Value::Null)?
        }
        "removeTab" => {
            let mut l = a.lists()?;
            cp::remove_tab(&mut l, a.str("id"));
            changed(&l, Value::Null)?
        }
        "moveTo" => {
            let mut l = a.lists()?;
            cp::move_to(&mut l, a.str("id"), a.str("from"), a.str("to"));
            changed(&l, Value::Null)?
        }
        "unplace" => {
            let mut l = a.lists()?;
            cp::unplace(&mut l, a.str("id"));
            changed(&l, Value::Null)?
        }
        "move" => {
            let mut l = a.lists()?;
            let moved = cp::move_by(
                &mut l,
                a.str("area"),
                a.str("id"),
                a.0.get("delta").and_then(Value::as_i64).unwrap_or(0),
                a.visible().as_deref(),
            );
            changed(&l, json!(moved))?
        }
        _ => bail!("unknown operation"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_round_trip() {
        let lists = dispatch("lists", r#"{"config": {}, "name": "pdf"}"#).unwrap();
        assert_eq!(lists["tabs"], json!(["links", "actions"]));
        let args = json!({ "lists": lists, "id": "week", "from": "hidden", "to": "overview" }).to_string();
        let out = dispatch("moveTo", &args).unwrap();
        assert_eq!(out["lists"]["overview"].as_array().unwrap().last().unwrap(), "week");
        let commit =
            dispatch("commit", &json!({ "config": {}, "name": "pdf", "lists": out["lists"] }).to_string()).unwrap();
        assert_eq!(commit[0][0], "default");
        assert_eq!(commit[1][0], "pdf");
        assert!(dispatch("nope", "{}").is_err());
        assert_eq!(dispatch("catalog", "").unwrap()["blocks"][0]["id"], "annotations");
    }
}
