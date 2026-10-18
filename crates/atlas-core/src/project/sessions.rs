// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Named sessions per device.
//!
//! ```text
//! .atlas/sessions/<device-id>/
//!   active.json            {"session": "standard"}
//!   standard.json          automatically created default session
//!   literaturrecherche.json
//! ```
//!
//! Every session file is a [`ProjectSession`]; the file name is a stable
//! ID derived from the name when the session was created (renaming changes
//! only the display name).

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use super::Project;
use crate::paths;
use crate::session::{FORMAT_VERSION, ProjectSession};

pub const DEFAULT_ID: &str = "standard";
pub const DEFAULT_NAME: &str = "Standard";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionInfo {
    pub id: String,
    pub name: String,
}

#[derive(Serialize, Deserialize)]
struct ActivePointer {
    session: String,
}

/// `active.json` of Phase 0 contained the session itself (detected by its
/// `layout` field).
#[derive(Deserialize)]
struct LegacySession {
    #[allow(dead_code)]
    layout: serde_json::Value,
}

fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.trim().to_lowercase().chars() {
        let mapped = match c {
            'ä' => "ae".into(),
            'ö' => "oe".into(),
            'ü' => "ue".into(),
            'ß' => "ss".into(),
            c if c.is_ascii_alphanumeric() => c.to_string(),
            _ => "-".into(),
        };
        out.push_str(&mapped);
    }
    let out: String = out.split('-').filter(|s| !s.is_empty()).collect::<Vec<_>>().join("-");
    let out: String = out.chars().take(48).collect();
    if out.is_empty() || out == "active" { "session".into() } else { out }
}

impl Project {
    pub fn sessions_dir(&self) -> PathBuf {
        self.atlas_dir().join("sessions").join(self.device())
    }

    fn session_path(&self, id: &str) -> PathBuf {
        self.sessions_dir().join(format!("{id}.json"))
    }

    fn valid_id(id: &str) -> bool {
        !id.is_empty() && id != "active" && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    }

    /// Converts a Phase 0 `active.json` (a session) into `standard.json`.
    fn migrate_legacy(&self) {
        let active = self.sessions_dir().join("active.json");
        let Ok(data) = std::fs::read(&active) else { return };
        if serde_json::from_slice::<ActivePointer>(&data).is_ok() {
            return;
        }
        if serde_json::from_slice::<LegacySession>(&data).is_ok()
            && let Ok(mut session) = serde_json::from_slice::<ProjectSession>(&data)
        {
            session.name = DEFAULT_NAME.into();
            if self.write(DEFAULT_ID, &session).is_ok() {
                let _ = self.set_active_session(DEFAULT_ID);
            }
        }
    }

    fn write(&self, id: &str, session: &ProjectSession) -> Result<()> {
        let mut session = session.clone();
        session.version = FORMAT_VERSION;
        paths::write_atomic(&self.session_path(id), &serde_json::to_vec_pretty(&session)?)
    }

    /// All sessions, the default one first, then by name. Creates the
    /// default session if there is none.
    pub fn list_sessions(&self) -> Vec<SessionInfo> {
        self.migrate_legacy();
        let mut list: Vec<SessionInfo> = std::fs::read_dir(self.sessions_dir())
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                let id = name.strip_suffix(".json")?.to_owned();
                if !Self::valid_id(&id) {
                    return None;
                }
                let session = self.load_named(&id);
                Some(SessionInfo { name: if session.name.is_empty() { id.clone() } else { session.name }, id })
            })
            .collect();
        if list.is_empty() {
            let session = ProjectSession { name: DEFAULT_NAME.into(), ..Default::default() };
            if self.write(DEFAULT_ID, &session).is_ok() {
                list.push(SessionInfo { id: DEFAULT_ID.into(), name: DEFAULT_NAME.into() });
            }
        }
        list.sort_by(|a, b| {
            (b.id == DEFAULT_ID)
                .cmp(&(a.id == DEFAULT_ID))
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        list
    }

    /// ID of the active session (the first existing one if the pointer is
    /// missing or stale).
    pub fn active_session(&self) -> String {
        let list = self.list_sessions();
        let pointer = std::fs::read(self.sessions_dir().join("active.json"))
            .ok()
            .and_then(|d| serde_json::from_slice::<ActivePointer>(&d).ok())
            .map(|p| p.session);
        match pointer {
            Some(id) if list.iter().any(|s| s.id == id) => id,
            _ => list.first().map(|s| s.id.clone()).unwrap_or_else(|| DEFAULT_ID.into()),
        }
    }

    pub fn set_active_session(&self, id: &str) -> Result<()> {
        if !Self::valid_id(id) || !self.session_path(id).exists() {
            bail!("Session „{id}“ existiert nicht.");
        }
        let pointer = serde_json::to_vec_pretty(&ActivePointer { session: id.into() })?;
        paths::write_atomic(&self.sessions_dir().join("active.json"), &pointer)
    }

    pub fn load_named(&self, id: &str) -> ProjectSession {
        if !Self::valid_id(id) {
            return ProjectSession::default();
        }
        let file = self.session_path(id);
        match std::fs::read(&file) {
            Ok(data) => serde_json::from_slice(&data).unwrap_or_else(|err| {
                log::warn!("ignoring unreadable session {}: {err}", file.display());
                ProjectSession::default()
            }),
            Err(_) => ProjectSession::default(),
        }
    }

    /// Saves a session; keeps its display name if `session.name` is empty.
    pub fn save_named(&self, id: &str, session: &ProjectSession) -> Result<()> {
        if !Self::valid_id(id) {
            bail!("ungültige Session-ID „{id}“");
        }
        let mut session = session.clone();
        if session.name.is_empty() {
            session.name = self.load_named(id).name;
        }
        if session.name.is_empty() {
            session.name = if id == DEFAULT_ID { DEFAULT_NAME.into() } else { id.into() };
        }
        self.write(id, &session)
    }

    /// Creates a session (empty, or a copy of `from`) and returns its ID.
    pub fn create_session(&self, name: &str, from: Option<&ProjectSession>) -> Result<String> {
        let name = name.trim();
        if name.is_empty() {
            bail!("Der Name darf nicht leer sein.");
        }
        if self.list_sessions().iter().any(|s| s.name.eq_ignore_ascii_case(name)) {
            bail!("Eine Session „{name}“ gibt es bereits.");
        }
        let base = slug(name);
        let mut id = base.clone();
        let mut n = 2;
        while self.session_path(&id).exists() {
            id = format!("{base}-{n}");
            n += 1;
        }
        let mut session = from.cloned().unwrap_or_default();
        session.name = name.into();
        self.write(&id, &session)?;
        Ok(id)
    }

    pub fn rename_session(&self, id: &str, name: &str) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            bail!("Der Name darf nicht leer sein.");
        }
        if self.list_sessions().iter().any(|s| s.id != id && s.name.eq_ignore_ascii_case(name)) {
            bail!("Eine Session „{name}“ gibt es bereits.");
        }
        let mut session = self.load_named(id);
        session.name = name.into();
        self.write(id, &session)
    }

    /// Deletes a session. The active session cannot be deleted: activate
    /// another one first.
    pub fn delete_session(&self, id: &str) -> Result<()> {
        if self.active_session() == id {
            bail!("Die aktive Session kann nicht gelöscht werden. Bitte zuerst eine andere Session aktivieren.");
        }
        if !Self::valid_id(id) {
            bail!("ungültige Session-ID „{id}“");
        }
        std::fs::remove_file(self.session_path(id)).context("Session konnte nicht gelöscht werden")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> (tempfile::TempDir, Project) {
        let dir = tempfile::tempdir().unwrap();
        let p = Project::open_as(dir.path(), "dev").unwrap();
        (dir, p)
    }

    #[test]
    fn default_session_is_created_and_active() {
        let (_d, p) = project();
        assert_eq!(p.list_sessions(), vec![SessionInfo { id: "standard".into(), name: "Standard".into() }]);
        assert_eq!(p.active_session(), "standard");
    }

    #[test]
    fn create_duplicate_rename_switch_delete() {
        let (_d, p) = project();
        let mut current = p.load_named("standard");
        current.layout = serde_json::json!({ "tabs": 3 });
        p.save_named("standard", &current).unwrap();

        let empty = p.create_session("Literaturrecherche", None).unwrap();
        assert_eq!(empty, "literaturrecherche");
        assert!(p.load_named(&empty).layout.is_null());
        let copy = p.create_session("Schreiben", Some(&current)).unwrap();
        assert_eq!(p.load_named(&copy).layout, current.layout);
        assert!(p.create_session("schreiben", None).is_err(), "names are unique");

        p.set_active_session(&copy).unwrap();
        assert_eq!(p.active_session(), copy);
        assert!(p.delete_session(&copy).is_err(), "active session is protected");
        p.rename_session(&empty, "Recherche").unwrap();
        assert_eq!(p.load_named(&empty).name, "Recherche");
        p.delete_session(&empty).unwrap();
        let names: Vec<_> = p.list_sessions().into_iter().map(|s| s.name).collect();
        assert_eq!(names, ["Standard", "Schreiben"]);
        // The original session is unchanged by all of this.
        assert_eq!(p.load_named("standard").layout, current.layout);
    }

    #[test]
    fn migrates_phase0_active_json() {
        let (_d, p) = project();
        std::fs::create_dir_all(p.sessions_dir()).unwrap();
        std::fs::write(
            p.sessions_dir().join("active.json"),
            r#"{"version":1,"layout":{"editor":1},"expanded":["src"]}"#,
        )
        .unwrap();
        assert_eq!(p.active_session(), "standard");
        let s = p.load_named("standard");
        assert_eq!(s.layout, serde_json::json!({ "editor": 1 }));
        assert_eq!(s.expanded, vec![PathBuf::from("src")]);
    }

    #[test]
    fn slugs() {
        assert_eq!(slug("Literatur & Recherche"), "literatur-recherche");
        assert_eq!(slug("Übung Größe"), "uebung-groesse");
        assert_eq!(slug("   "), "session");
        assert_eq!(slug("active"), "session");
    }
}
