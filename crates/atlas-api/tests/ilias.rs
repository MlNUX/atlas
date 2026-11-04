// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! P2.10: ILIAS through the facade without network: settings, the stored
//! index and downloads, the remembered login (with a `systemd-creds`
//! stand-in, also on macOS instead of the keychain), errors of a scan.

use std::os::unix::fs::PermissionsExt;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use atlas_api::app::Atlas;
use atlas_api::events::{Event, EventSink};
use atlas_api::ilias::{Ilias, IliasLogin};

struct Quiet;
impl EventSink for Quiet {
    fn on_event(&self, _: Event) {}
}

fn ilias() -> (MutexGuard<'static, ()>, Arc<Ilias>) {
    static DIRS: OnceLock<tempfile::TempDir> = OnceLock::new();
    static LOCK: Mutex<()> = Mutex::new(());
    let guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = DIRS.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        // ROT13 stand-in for systemd-creds (as in atlas-core's tests).
        let creds = dir.path().join("fake-creds");
        std::fs::write(
            &creds,
            "#!/bin/sh\n[ \"$1\" = --user ] || exit 9\ncase \"$2\" in\n  encrypt) tr a-z n-za-m > \"$5\" ;;\n  decrypt) tr a-z n-za-m < \"$4\" ;;\n  *) exit 8 ;;\nesac\n",
        )
        .unwrap();
        std::fs::set_permissions(&creds, std::fs::Permissions::from_mode(0o755)).unwrap();
        // SAFETY: set once, before any test of this binary reads them.
        unsafe {
            std::env::set_var("ATLAS_STATE_DIR", dir.path().join("state"));
            std::env::set_var("ATLAS_CONFIG_DIR", dir.path().join("config"));
            std::env::set_var("ATLAS_SYSTEMD_CREDS", &creds);
        }
        std::fs::create_dir_all(dir.path().join("config")).unwrap();
        dir
    });
    let _ = dir;
    (guard, Atlas::new(Arc::new(Quiet)).ilias())
}

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    use std::task::{Context, Poll, Wake, Waker};
    struct Unpark(std::thread::Thread);
    impl Wake for Unpark {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(Unpark(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => std::thread::park(),
        }
    }
}

#[test]
fn settings_round_trip() {
    let (_g, ilias) = ilias();
    let mut c = ilias.config();
    c.base_url = " https://ilias.example.org".into();
    c.login_host = "IdP.Example.org ".into();
    c.download_dir = "/tmp/ilias".into();
    ilias.save_config(c).unwrap();
    let c = ilias.config();
    assert_eq!(c.base_url, "https://ilias.example.org/");
    assert_eq!(c.login_host, "idp.example.org");
    assert!(ilias.save_config(atlas_api::ilias::IliasConfig { base_url: "kein url".into(), ..c }).is_err());
}

#[test]
fn stored_index_and_downloads() {
    let (_g, ilias) = ilias();
    let index = r#"{"base_url":"https://ilias.example.org/","scanned":5,"courses":[
        {"kind":"crs","id":"1","name":"Analysis","url":"u1","children":[
            {"kind":"file","id":"2","name":"Blatt 1","url":"u2","download":"d2","info":"PDF · 1 MB"}]}]}"#;
    atlas_ilias::save("index", index).unwrap();
    let last = ilias.last_index().unwrap();
    assert_eq!(last.scanned, 5);
    assert_eq!(last.courses[0].title, "Analysis");
    let file = &last.courses[0].children[0];
    assert_eq!((file.kind.as_str(), file.download.as_deref(), file.info.as_str()), ("file", Some("d2"), "PDF · 1 MB"));
    assert!(file.children.is_empty());

    atlas_ilias::record_download("u2", std::path::Path::new("/tmp/Blatt 1.pdf"), "PDF · 1 MB").unwrap();
    let d = ilias.downloads();
    assert!(d.iter().any(|d| d.url == "u2" && d.path == "/tmp/Blatt 1.pdf" && d.time > 0));
}

#[test]
fn remembers_and_forgets_the_login() {
    let (_g, ilias) = ilias();
    assert!(ilias.remember_login(IliasLogin { user: " ".into(), password: "x".into() }).is_err());
    ilias.remember_login(IliasLogin { user: "uxxxx".into(), password: "geheim".into() }).unwrap();
    assert!(ilias.config().login_saved);
    assert_eq!(ilias.remembered_login().unwrap(), Some(IliasLogin { user: "uxxxx".into(), password: "geheim".into() }));
    ilias.forget_login().unwrap();
    assert!(!ilias.config().login_saved);
    assert_eq!(ilias.remembered_login().unwrap(), None);
}

#[test]
fn scanning_an_unreachable_server_fails_cleanly() {
    let (_g, ilias) = ilias();
    let c = ilias.config();
    ilias.save_config(atlas_api::ilias::IliasConfig { base_url: "http://127.0.0.1:9/".into(), ..c }).unwrap();
    ilias.set_session(
        vec![atlas_api::ilias::Cookie { name: "PHPSESSID".into(), value: "abc".into(), domain: "127.0.0.1".into() }],
        String::new(),
    );
    let err = block_on(ilias.scan()).unwrap_err();
    assert!(matches!(err, atlas_api::AtlasError::Failed { .. } | atlas_api::AtlasError::Auth { .. }), "{err:?}");
    assert!(std::path::Path::new(&ilias.debug_log()).exists());
}
