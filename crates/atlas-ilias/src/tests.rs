// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

use super::*;

#[test]
fn names_become_safe_segments() {
    assert_eq!(safe_segment("Höhere Mathematik I / WS 26"), "Höhere Mathematik I _ WS 26");
    assert_eq!(safe_segment(".."), "_");
    assert_eq!(safe_segment("../../etc"), "_.._etc");
    assert_eq!(safe_segment("  Folien\t\n1  "), "Folien 1");
    assert_eq!(safe_segment(".versteckt"), "versteckt");
    assert_eq!(safe_segment(&"x".repeat(300)).len(), 120);
    assert_eq!(file_name("vl01.pdf", "Vorlesung 1"), "vl01.pdf");
    assert_eq!(file_name("", "Vorlesung 1"), "Vorlesung 1");
    assert_eq!(file_name("../../evil.sh", "x"), "evil.sh");
}

#[test]
fn target_dir_stays_below_root() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("ILIAS");
    let t = target_dir(&root, &["Kurs A".into(), "..".into(), "Ordner/1".into()]).unwrap();
    assert_eq!(t, root.join("Kurs A").join("_").join("Ordner_1"));
    assert!(t.is_dir() && t.starts_with(&root));
    assert!(target_dir(Path::new(""), &[]).is_err());
}

// A small ILIAS: without the cookie `sid=1` every page sends to the
// login; a course with a folder, a session (ref_id link, recognisable
// by its icon) and files; a group; navigation and breadcrumbs that
// link back up.
struct Mock {
    base: String,
    version: std::sync::Arc<std::sync::atomic::AtomicU32>,
}

fn item(kind: &str, id: &str, name: &str, version: u32) -> String {
    let (href, icon, props) = match kind {
        "file" => (
            format!("/goto.php?target=file_{id}_download&amp;client_id=test"),
            "file",
            format!(
                r#"<div class="il_ItemProperties"><span class="il_ItemProperty">pdf</span> <span class="il_ItemProperty">Version: {version}</span></div>"#
            ),
        ),
        "sess" => (format!("/ilias.php?ref_id={id}&amp;cmdClass=ilobjsessiongui"), "sess", String::new()),
        k => (format!("/goto.php?target={k}_{id}&amp;client_id=test"), k, String::new()),
    };
    format!(
        r#"<div class="il_ContainerListItem"><img src="/images/icon_{icon}.svg"><div class="il_ContainerItemTitle"><a href="{href}">{name}</a></div>{props}</div>"#
    )
}

fn page_html(title: &str, items: &str) -> String {
    format!(
        r#"<html><body><nav class="il-maincontrols"><a href="/logout.php">Abmelden</a> <a href="/goto.php?target=crs_100">Mathe</a></nav><ol class="breadcrumb"><li><a href="/goto.php?target=crs_100">Mathe</a></li></ol><div id="il_center_col"><h1>{title}</h1>{items}</div></body></html>"#
    )
}

fn respond(path: &str, cookie: &str, version: u32) -> (u16, Vec<(String, String)>, Vec<u8>) {
    let html = |s: String| (200, vec![("Content-Type".into(), "text/html; charset=utf-8".into())], s.into_bytes());
    if path.starts_with("/login.php") {
        return html("<html><body>Anmeldung</body></html>".into());
    }
    if !cookie.contains("sid=1") {
        return (302, vec![("Location".into(), "/login.php".into())], Vec::new());
    }
    let files = [
        ("102", "Organisatorisches", "orga.pdf"),
        ("104", "Folien", "folien 1.pdf"),
        ("106", "Blatt 2", "blatt02.pdf"),
        ("108", "Skript", ""),
    ];
    for (id, title, file) in files {
        if path.starts_with(&format!("/goto.php?target=file_{id}_download")) {
            let v = if id == "102" { version } else { 1 };
            let mut headers = vec![("Content-Type".to_owned(), "application/pdf".to_owned())];
            if !file.is_empty() {
                headers.push((
                    "Content-Disposition".into(),
                    format!("attachment; filename*=UTF-8''{}", file.replace(' ', "%20")),
                ));
            }
            return (200, headers, format!("{title} Version {v}").into_bytes());
        }
    }
    // Only the favourites are read; the membership views list more.
    // Their address redirects and renews the session cookie: followed
    // with it.
    if path.starts_with("/ilias.php?baseClass=ilDashboardGUI&cmd=jumpToSelectedItems") {
        let headers = vec![
            ("Location".to_owned(), "/ilias.php?baseClass=ilDashboardGUI&cmd=showFavourites".to_owned()),
            ("Set-Cookie".to_owned(), "sid=1; Path=/; HttpOnly".to_owned()),
        ];
        return (302, headers, Vec::new());
    }
    if path.starts_with("/ilias.php?baseClass=ilDashboardGUI&cmd=showFavourites") {
        return html(page_html(
            "Favoriten",
            &(item("crs", "100", "Höhere Mathematik I", 1) + &item("grp", "200", "Tutorium 3", 1)),
        ));
    }
    if path.starts_with("/ilias.php?baseClass=ilDashboardGUI")
        || path.starts_with("/ilias.php?baseClass=ilmembershipoverviewgui")
    {
        return html(page_html("Meine Kurse", &item("crs", "500", "Kein Favorit", 1)));
    }
    let pages = [
        (
            "/goto.php?target=crs_100",
            vec![
                item("fold", "101", "Vorlesung", 1),
                item("file", "102", "Organisatorisches", version),
                item("sess", "103", "Sitzung 1", 1),
                // A side block (news) linking another course's folder.
                format!(r#"<div class="ilBlock">{}</div>"#, item("fold", "900", "Fremder Ordner", 1)),
            ],
        ),
        (
            "/goto.php?target=fold_101",
            vec![
                item("file", "104", "Folien", 1),
                item("fold", "105", "Übungen", 1),
                item("fold", "101", "Vorlesung", 1),
            ],
        ),
        ("/goto.php?target=fold_105", vec![item("file", "106", "Blatt 2", 1)]),
        ("/ilias.php?ref_id=103", vec![item("file", "108", "Skript", 1)]),
        ("/goto.php?target=grp_200", vec![item("crs", "300", "Anderer Kurs", 1)]),
    ];
    for (p, items) in pages {
        if path.starts_with(p) {
            return html(page_html("Seite", &items.concat()));
        }
    }
    (404, Vec::new(), b"nicht gefunden".to_vec())
}

impl Mock {
    fn start() -> Mock {
        use std::io::BufRead;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}/", listener.local_addr().unwrap());
        let version = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(1));
        let shared = version.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let version = shared.clone();
                std::thread::spawn(move || -> std::io::Result<()> {
                    let mut stream = stream;
                    let mut reader = std::io::BufReader::new(stream.try_clone()?);
                    let mut line = String::new();
                    reader.read_line(&mut line)?;
                    let path = line.split_whitespace().nth(1).unwrap_or("").to_owned();
                    let mut cookie = String::new();
                    loop {
                        let mut h = String::new();
                        reader.read_line(&mut h)?;
                        if h.trim().is_empty() {
                            break;
                        }
                        if let Some((n, v)) = h.split_once(':')
                            && n.eq_ignore_ascii_case("cookie")
                        {
                            cookie = v.trim().to_owned();
                        }
                    }
                    let (status, headers, body) =
                        respond(&path, &cookie, version.load(std::sync::atomic::Ordering::SeqCst));
                    let mut out =
                        format!("HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n", body.len());
                    for (n, v) in headers {
                        out.push_str(&format!("{n}: {v}\r\n"));
                    }
                    out.push_str("\r\n");
                    stream.write_all(out.as_bytes())?;
                    stream.write_all(&body)
                });
            }
        });
        Mock { base, version }
    }
}

#[test]
fn ilias9_dashboard_inside_mainspacekeeper() {
    let page = Url::parse("https://ilias.example/ilias.php?baseClass=ilmembershipoverviewgui").unwrap();
    let html = r#"<html><body><div id="mainspacekeeper"><main class="il-layout-page-content">
        <div class="il-item il-std-item"><div class="il-item-icon"><img class="icon crs medium" src="./templates/default/images/standard/icon_crs.svg"></div>
          <h4 class="il-item-title"><a href="https://ilias.example/goto.php/crs/2512345">Lineare Algebra</a></h4>
          <div class="il-item-properties"><span class="il-item-property-value">WS 26/27</span></div></div>
        <div class="il-item il-std-item"><img class="icon grp medium" src="./templates/default/images/standard/icon_grp.svg">
          <h4 class="il-item-title"><a href="ilias.php?baseClass=ilrepositorygui&amp;ref_id=777">Übungsgruppe 4</a></h4></div>
        </main></div></body></html>"#;
    let found = objects(html, &page);
    let keys: Vec<_> = found.iter().map(|o| (o.key(), o.name.as_str())).collect();
    assert_eq!(keys, [("crs2512345".into(), "Lineare Algebra"), ("grp777".into(), "Übungsgruppe 4")]);
    assert_eq!(found[0].info, "WS 26/27");
    let why = explain(
        "<html><body><main><a href='/ilias.php?cmdClass=x'>a</a><nav><a href='/n'>n</a></nav></main></body></html>",
        &page,
    );
    assert!(
        why.contains("gelesen: <main>") && why.contains("2 von 2 Links") && why.contains("1 in Navigation"),
        "{why}"
    );
    assert!(why.contains("/ilias.php?cmdClass=x"), "{why}");
}

#[test]
fn tile_view_cards() {
    let page = Url::parse("https://ilias.example/ilias.php?baseClass=ilrepositorygui&ref_id=10").unwrap();
    let html = r#"<html><body><div id="il_center_col">
        <div class="il-card thumbnail"><a href="ilias.php?baseClass=ilrepositorygui&amp;ref_id=11"><img src="./data/tile.png"></a>
          <div class="card-body"><div class="il-card-repository-head"><img class="icon fold medium" src="./assets/images/standard/icon_x.svg"></div>
          <h5 class="card-title"><a href="ilias.php?baseClass=ilrepositorygui&amp;ref_id=11">Übungsblätter</a></h5></div></div>
        <div class="il-card thumbnail"><div class="card-body"><img src="./templates/default/images/standard/icon_file.svg">
          <h5 class="card-title"><a href="ilias.php?baseClass=ilrepositorygui&amp;cmd=sendfile&amp;ref_id=12">Skript</a></h5></div></div>
        </div></body></html>"#;
    let found = objects(html, &page);
    let keys: Vec<_> = found.iter().map(|o| (o.key(), o.name.as_str())).collect();
    assert_eq!(keys, [("fold11".into(), "Übungsblätter"), ("file12".into(), "Skript")]);
    assert!(found[1].download.as_deref().unwrap().contains("cmd=sendfile"));
}

#[test]
fn links_are_classified() {
    assert_eq!(classify("https://x/goto.php?target=fold_12&client_id=k", None), Some(("fold".into(), "12".into())));
    assert_eq!(classify("https://x/goto_produktiv_crs_7.html", None), Some(("crs".into(), "7".into())));
    assert_eq!(classify("https://x/go/grp/9", None), Some(("grp".into(), "9".into())));
    assert_eq!(classify("https://x/goto.php%3Ftarget%3Dfile_5_download", None), Some(("file".into(), "5".into())));
    assert_eq!(
        classify("https://x/ilias.php?ref_id=3&cmd=x", Some("/img/icon_sess.svg")),
        Some(("sess".into(), "3".into()))
    );
    assert_eq!(classify("https://x/ilias.php?ref_id=3", Some("/img/icon_tst.svg")), None);
    assert_eq!(classify("https://x/ilias.php?ref_id=3", None), None);
    let kit =
        "https://x/ilias.php?baseClass=ilrepositorygui&cmdNode=xu:ng&cmdClass=ilObjFileGUI&cmd=sendfile&ref_id=2938639";
    assert_eq!(classify(kit, None), Some(("file".into(), "2938639".into())));
    let fold = "https://x/ilias.php?baseClass=ilrepositorygui&cmdNode=xu:nl&cmdClass=ilobjfoldergui&ref_id=3042915&item_ref_id=0";
    assert_eq!(classify(fold, None), Some(("fold".into(), "3042915".into())));
    let info = "https://x/ilias.php?baseClass=ilrepositorygui&cmdNode=xu:nl:f6&cmdClass=ilinfoscreengui&cmd=showSummary&ref_id=3";
    assert_eq!(classify(info, None), None, "info page is no object");
}

#[test]
fn objects_of_a_page() {
    let page = Url::parse("https://ilias.example/ilias.php?ref_id=100").unwrap();
    let html = page_html(
        "Kurs",
        &(item("fold", "101", "  Vorlesung\n ", 1)
            + &item("file", "102", "Orga", 3)
            + &item("sess", "103", "Sitzung", 1)
            + r#"<a href="https://anders.example/goto.php?target=fold_9">fremd</a><a href="/goto.php?target=fold_101">Vorlesung</a>"#),
    );
    let found = objects(&html, &page);
    let keys: Vec<_> = found.iter().map(|o| (o.key(), o.name.as_str())).collect();
    assert_eq!(keys, [("fold101".into(), "Vorlesung"), ("file102".into(), "Orga"), ("sess103".into(), "Sitzung")]);
    assert_eq!(found[1].info, "pdf Version: 3");
    assert_eq!(
        found[1].download.as_deref(),
        Some("https://ilias.example/goto.php?target=file_102_download&client_id=test")
    );
    assert_eq!(found[2].url, "https://ilias.example/ilias.php?ref_id=103&cmdClass=ilobjsessiongui");
    assert!(logged_in(&html, &page));
    let ilias9 = r#"<a href="ilias.php?baseClass=ilstartupgui&cmd=doLogout">Abmelden</a>"#;
    assert!(logged_in(ilias9, &page));
    assert!(logged_in("<html><body>Dashboard</body></html>", &page), "user menu loaded later");
    let login = Url::parse("https://ilias.example/login.php?target=&cmd=force_login").unwrap();
    assert!(!logged_in("<html><body>Anmeldung</body></html>", &login));
    assert!(!logged_in(r#"<a href="/shib_login.php?target=">Anmelden</a>"#, &page));
    assert!(!logged_in(r#"<form><input type="password" name="pw"></form>"#, &page));
    assert_eq!(
        download_url("https://i.ex/goto.php?target=file_7&client_id=k", "7", &page),
        "https://i.ex/goto.php?target=file_7_download&client_id=k"
    );
    assert_eq!(download_url("https://i.ex/x", "7", &page), "https://ilias.example/goto.php?target=file_7_download");
}

#[test]
fn cookies_from_the_profile_database() {
    let dir = tempfile::tempdir().unwrap();
    let conn = rusqlite::Connection::open(dir.path().join("Cookies")).unwrap();
    conn.execute_batch(
        "CREATE TABLE cookies (host_key TEXT, name TEXT, value TEXT, path TEXT, is_secure INTEGER,
                               expires_utc INTEGER, encrypted_value BLOB);
         INSERT INTO cookies VALUES ('ilias.example', 'PHPSESSID', 'abc', '/', 1, 0, x'');
         INSERT INTO cookies VALUES ('.example', 'shared', 'd', '/', 0, 99999999999999999, x'');
         INSERT INTO cookies VALUES ('ilias.example', 'old', 'x', '/', 0, 1, x'');
         INSERT INTO cookies VALUES ('ilias.example', 'deep', 'y', '/other', 0, 0, x'');
         INSERT INTO cookies VALUES ('idp.example', 'idp', 'z', '/', 0, 0, x'');
         INSERT INTO cookies VALUES ('ilias.example', 'secret', '', '/', 0, 0, x'763130');",
    )
    .unwrap();
    drop(conn);
    let url = Url::parse("https://ilias.example/ilias.php?x").unwrap();
    let found = profile_cookies(dir.path(), &url).unwrap();
    assert_eq!(found, [("PHPSESSID".to_owned(), "abc".to_owned()), ("shared".to_owned(), "d".to_owned())]);
    let plain = Url::parse("http://ilias.example/").unwrap();
    assert_eq!(
        profile_cookies(dir.path(), &plain).unwrap(),
        [("shared".to_owned(), "d".to_owned())],
        "secure only via https"
    );
    // Cookies set since the start win.
    assert_eq!(session_cookies(dir.path(), &url, "PHPSESSID=neu; lang=de"), "shared=d; PHPSESSID=neu; lang=de");
    assert_eq!(session_cookies(Path::new(""), &url, "a=1"), "a=1");
    assert!(profile_cookies(&dir.path().join("fehlt"), &url).is_err());
}

#[test]
fn disposition_names() {
    assert_eq!(disposition_name(r#"attachment; filename="vl 01.pdf""#).as_deref(), Some("vl 01.pdf"));
    assert_eq!(disposition_name("attachment; filename=a.pdf; size=3").as_deref(), Some("a.pdf"));
    assert_eq!(
        disposition_name(r#"attachment; filename="Ubung.pdf"; filename*=UTF-8''%C3%9Cbung.pdf"#).as_deref(),
        Some("Übung.pdf")
    );
    assert_eq!(disposition_name("inline"), None);
}

#[test]
fn scan_needs_a_session() {
    let mock = Mock::start();
    let client = Client::new(&mock.base, "", "").unwrap();
    assert_eq!(scan(&client, &|_| {}), Err(IliasError::Login));
    let dir = tempfile::tempdir().unwrap();
    let url = format!("{}goto.php?target=file_102_download", mock.base);
    assert_eq!(download(&client, &url, dir.path(), "Orga", None, &|_, _| {}), Err(IliasError::Login));
}

#[test]
fn scan_reads_the_tree_and_downloads() {
    let mock = Mock::start();
    let client = Client::new(&mock.base, "lang=de; sid=1", "Mozilla/5.0 Test").unwrap();
    let messages = Mutex::new(Vec::new());
    let tree = scan(&client, &|p| messages.lock().unwrap().push(p.text)).unwrap();
    assert_eq!(messages.lock().unwrap()[0], "2 Favoriten");

    fn outline(nodes: &[Node], depth: usize, out: &mut Vec<String>) {
        for n in nodes {
            out.push(format!("{}{} {}", " ".repeat(depth), n.key(), n.name));
            outline(n.children.as_deref().unwrap_or_default(), depth + 1, out);
        }
    }
    let mut lines = Vec::new();
    outline(&tree, 0, &mut lines);
    assert_eq!(
        lines,
        [
            "crs100 Höhere Mathematik I",
            " fold101 Vorlesung",
            "  file104 Folien",
            "  fold105 Übungen",
            "   file106 Blatt 2",
            " file102 Organisatorisches",
            " sess103 Sitzung 1",
            "  file108 Skript",
            "grp200 Tutorium 3",
        ]
    );
    assert_eq!(tree[1].children, Some(Vec::new()), "courses in a group are not followed");
    assert!(tree[0].children.as_ref().unwrap()[1].children.is_none(), "files have no children");
    let json = serde_json::to_string(&tree).unwrap();
    assert_eq!(serde_json::from_str::<Vec<Node>>(&json).unwrap(), tree);

    let dir = tempfile::tempdir().unwrap();
    let orga = &tree[0].children.as_ref().unwrap()[1];
    let seen = Mutex::new(0u64);
    let path = download(&client, orga.download.as_deref().unwrap(), dir.path(), &orga.name, None, &|n, total| {
        assert_eq!(total, Some(27));
        *seen.lock().unwrap() = n;
    })
    .unwrap();
    assert_eq!(path, dir.path().join("orga.pdf"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "Organisatorisches Version 1");
    assert_eq!(*seen.lock().unwrap(), 27);

    // A new version replaces the file; no partial file stays behind.
    mock.version.store(2, std::sync::atomic::Ordering::SeqCst);
    download(&client, orga.download.as_deref().unwrap(), dir.path(), &orga.name, Some(&path), &|_, _| {}).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "Organisatorisches Version 2");
    let names: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(names, ["orga.pdf"]);

    // Names from the header (percent-encoded) or else the title.
    let folien = format!("{}goto.php?target=file_104_download", mock.base);
    assert_eq!(
        download(&client, &folien, dir.path(), "Folien", None, &|_, _| {}).unwrap(),
        dir.path().join("folien 1.pdf")
    );
    let skript = format!("{}goto.php?target=file_108_download", mock.base);
    assert_eq!(download(&client, &skript, dir.path(), "Skript", None, &|_, _| {}).unwrap(), dir.path().join("Skript"));
    // Another file of the same name (another course's): numbered, the
    // first stays.
    let other = download(&client, orga.download.as_deref().unwrap(), dir.path(), &orga.name, None, &|_, _| {}).unwrap();
    assert_eq!(other, dir.path().join("orga (2).pdf"));
    assert!(path.is_file());
    assert_eq!(free_name(dir.path(), "Skript"), dir.path().join("Skript (2)"));
    // An ILIAS page instead of a file.
    let page = format!("{}goto.php?target=crs_100", mock.base);
    assert!(matches!(download(&client, &page, dir.path(), "Kurs", None, &|_, _| {}), Err(IliasError::Other(_))));
}
