// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! The web view's session: its cookies for ILIAS, read from the profile.

use std::path::Path;
use std::time::Duration;

use anyhow::{Result, bail};
use url::Url;

use super::*;

/// The cookies the web profile in `profile_dir` keeps for `url`: read from
/// its `Cookies` database (Chromium's, which QtWebEngine writes
/// unencrypted). Qt reports cookies only when they are set, not those it
/// read at start – after a restart the login is only here. Read-only;
/// Atlas keeps no copy.
pub fn profile_cookies(profile_dir: &Path, url: &Url) -> Result<Vec<(String, String)>> {
    let db = profile_dir.join("Cookies");
    if !db.is_file() {
        bail!("keine Cookie-Datenbank in {}", profile_dir.display());
    }
    let flags = rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let read = |path: &Path| -> Result<Vec<StoredCookie>> {
        let conn = rusqlite::Connection::open_with_flags(path, flags)?;
        conn.busy_timeout(Duration::from_secs(2))?;
        let mut stmt = conn.prepare(
            "SELECT host_key, name, value, path, is_secure, expires_utc, length(encrypted_value) FROM cookies",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok(StoredCookie {
                    host_key: r.get(0)?,
                    name: r.get(1)?,
                    value: r.get(2)?,
                    path: r.get(3)?,
                    secure: r.get::<_, i64>(4)? != 0,
                    expires: r.get(5)?,
                    encrypted: r.get::<_, i64>(6)? > 0,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    };
    // While the web engine holds the database locked: from a copy (only
    // the owner can read it, removed right away).
    let rows = match read(&db) {
        Ok(rows) => rows,
        Err(err) => {
            trace(&format!("  Cookie-Datenbank gesperrt ({err:#}) – lese eine Kopie"));
            let copy = state_dir().join(".cookies-copy");
            std::fs::create_dir_all(state_dir())?;
            std::fs::copy(&db, &copy)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&copy, std::fs::Permissions::from_mode(0o600))?;
            }
            let rows = read(&copy);
            let _ = std::fs::remove_file(&copy);
            rows?
        }
    };
    // Chromium's time: microseconds since 1601; 0 = until the browser ends.
    let now =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let now = (now + 11_644_473_600) * 1_000_000;
    let host = url.host_str().unwrap_or_default().to_lowercase();
    let path = url.path();
    let mut out = Vec::new();
    for c in rows {
        let domain = c.host_key.trim_start_matches('.').to_lowercase();
        let matches = host == domain || (c.host_key.starts_with('.') && host.ends_with(&format!(".{domain}")));
        let expired = c.expires != 0 && c.expires < now;
        if !matches || !path.starts_with(&c.path) || (c.secure && url.scheme() != "https") || expired {
            continue;
        }
        if c.value.is_empty() && c.encrypted {
            trace(&format!("  Cookie {}@{} ist verschlüsselt gespeichert – übergangen", c.name, c.host_key));
            continue;
        }
        out.push((c.name, c.value));
    }
    Ok(out)
}

/// A row of Chromium's `cookies` table.
struct StoredCookie {
    host_key: String,
    name: String,
    value: String,
    path: String,
    secure: bool,
    expires: i64,
    encrypted: bool,
}

/// The `Cookie` header for ILIAS: the profile's stored cookies, replaced
/// by those reported since the start (`live`, newer).
pub fn session_cookies(profile_dir: &Path, url: &Url, live: &str) -> String {
    let mut cookies: Vec<(String, String)> = if profile_dir.as_os_str().is_empty() {
        trace("Web-Profil ohne Ordner – nur Cookies seit dem Start");
        Vec::new()
    } else {
        match profile_cookies(profile_dir, url) {
            Ok(c) => {
                trace(&format!("Cookie-Datenbank des Profils: {}", cookie_names(&header(&c))));
                c
            }
            Err(err) => {
                trace(&format!("Cookie-Datenbank des Profils nicht lesbar: {err:#}"));
                Vec::new()
            }
        }
    };
    for part in live.split(';') {
        if let Some((n, v)) = part.split_once('=') {
            let (n, v) = (n.trim(), v.trim());
            cookies.retain(|(name, _)| name != n);
            cookies.push((n.to_owned(), v.to_owned()));
        }
    }
    header(&cookies)
}

fn header(cookies: &[(String, String)]) -> String {
    cookies.iter().map(|(n, v)| format!("{n}={v}")).collect::<Vec<_>>().join("; ")
}
