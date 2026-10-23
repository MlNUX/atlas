#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
"""A tiny stand-in for ILIAS, for the smoke test of the ILIAS view.

Usage: ilias-mock.py PORT

Without the session cookie every page sends to /login.php (like the
university's login); /dologin sets it. The dashboard lists a course and a
group; containers hold folders, a session (linked by ref_id, recognisable
by its icon only) and files. Navigation and breadcrumbs link back up and
must be ignored. Files download with a file name; /bump gives file 102 a
new version.
"""

import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse

state = {"version": 1}

FILES = {
    "102": ("Organisatorisches", "orga.pdf"),
    "104": ("Folien Kapitel 1", "kapitel1.pdf"),
    "106": ("Blatt 10", "blatt10.pdf"),
    "107": ("Blatt 2", "blatt02.pdf"),
    "108": ("Skript", "skript.pdf"),
    "201": ("Gruppeneinteilung", "gruppen.pdf"),
}

# container key -> list of (kind, id) children
TREE = {
    "crs_100": [("fold", "101"), ("file", "102"), ("sess", "103")],
    "fold_101": [("file", "104"), ("fold", "105")],
    "fold_105": [("file", "106"), ("file", "107")],
    "sess_103": [("file", "108")],
    "grp_200": [("file", "201")],
}
NAMES = {
    "crs_100": "Höhere Mathematik I",
    "grp_200": "Tutorium 3",
    "fold_101": "Vorlesung",
    "fold_105": "Übungen",
    "sess_103": "Sitzung 1: Einführung",
}


def pdf(text):
    """A one-page PDF showing `text`, with a correct xref table."""
    stream = f"BT /F1 24 Tf 72 700 Td ({text}) Tj ET".encode()
    objs = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>",
        b"<< /Length %d >>\nstream\n" % len(stream) + stream + b"\nendstream",
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
    ]
    out = b"%PDF-1.4\n"
    offsets = []
    for i, o in enumerate(objs):
        offsets.append(len(out))
        out += b"%d 0 obj\n" % (i + 1) + o + b"\nendobj\n"
    xref = len(out)
    out += b"xref\n0 %d\n0000000000 65535 f \n" % (len(objs) + 1)
    for off in offsets:
        out += b"%010d 00000 n \n" % off
    out += b"trailer\n<< /Size %d /Root 1 0 R >>\nstartxref\n%d\n%%%%EOF\n" % (len(objs) + 1, xref)
    return out


def item(kind, oid):
    if kind == "file":
        title, _ = FILES[oid]
        version = state["version"] if oid == "102" else 1
        href = f"/goto.php?target=file_{oid}_download&client_id=test"
        props = f'<div class="il_ItemProperties"><span class="il_ItemProperty">pdf</span> <span class="il_ItemProperty">1 KB</span> <span class="il_ItemProperty">Version: {version}</span></div>'
        icon = "icon_file.svg"
    elif kind == "sess":
        title = NAMES[f"sess_{oid}"]
        href = f"/ilias.php?ref_id={oid}&cmdClass=ilobjsessiongui&baseClass=ilrepositorygui"
        props = '<div class="il_ItemProperties"><span class="il_ItemProperty">20. Okt 2026, 10:00</span></div>'
        icon = "icon_sess.svg"
    else:
        title = NAMES[f"{kind}_{oid}"]
        href = f"/goto.php?target={kind}_{oid}&client_id=test"
        props = ""
        icon = f"icon_{kind}.svg"
    return (f'<div class="il_ContainerListItem"><img class="ilListItemIcon" src="/templates/default/images/{icon}" alt="">'
            f'<div class="il_ContainerItemTitle"><a href="{href}">{title}</a></div>{props}</div>')


def page(title, items):
    # Logout as in ILIAS 8 and later (cmd=doLogout, capital L).
    nav = ('<nav class="il-maincontrols"><a href="/ilias.php?baseClass=ilStartUpGUI&amp;cmd=doLogout">Abmelden</a> '
           '<a href="/goto.php?target=crs_100">Höhere Mathematik I</a></nav>'
           '<ol class="breadcrumb"><li><a href="/ilias.php?baseClass=ilDashboardGUI">Dashboard</a></li>'
           '<li><a href="/goto.php?target=crs_100">Höhere Mathematik I</a></li></ol>')
    body = "".join(items)
    return f'<!doctype html><html><head><meta charset="utf-8"><title>{title}</title></head><body>{nav}<div id="il_center_col"><h1>{title}</h1>{body}</div></body></html>'


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def send(self, code, body=b"", ctype="text/html; charset=utf-8", headers=()):
        self.send_response(code)
        for k, v in headers:
            self.send_header(k, v)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def redirect(self, to, headers=()):
        self.send(302, b"", headers=[("Location", to), *headers])

    def do_GET(self):
        url = urlparse(self.path)
        q = parse_qs(url.query)
        logged_in = "sid=1" in (self.headers.get("Cookie") or "")
        if url.path == "/login.php":
            return self.send(200, b'<!doctype html><html><body><h1>Anmeldung</h1><a id="login" href="/dologin">Anmelden mit KIT-Konto</a>'
                                  b'<form action="/login.php?cmd=post"><input type="password" name="pw"></form></body></html>')
        if url.path == "/dologin":
            return self.redirect("/ilias.php?baseClass=ilDashboardGUI&cmd=show", [("Set-Cookie", "sid=1; Path=/")])
        if url.path == "/logout.php" or (q.get("cmd") or [""])[0] == "doLogout":
            return self.redirect("/login.php", [("Set-Cookie", "sid=; Path=/; Max-Age=0")])
        if url.path == "/bump":
            state["version"] += 1
            return self.send(200, b"ok", "text/plain")
        if not logged_in:
            return self.redirect("/login.php")
        if url.path in ("/", "/index.php"):
            return self.redirect("/ilias.php?baseClass=ilDashboardGUI&cmd=show")
        target = (q.get("target") or [""])[0]
        # Like ILIAS: some dashboard views redirect (and renew the cookie).
        if url.path == "/ilias.php" and (q.get("cmd") or [""])[0] == "jumpToMemberships":
            return self.redirect("/ilias.php?baseClass=ilDashboardGUI&cmd=show", [("Set-Cookie", "sid=1; Path=/; HttpOnly")])
        if url.path == "/ilias.php" and (q.get("baseClass") or [""])[0].lower() in ("ildashboardgui", "ilmembershipoverviewgui"):
            return self.send(200, page("Dashboard", [item("crs", "100"), item("grp", "200")]).encode())
        if url.path == "/ilias.php" and q.get("ref_id"):
            key = f"sess_{q['ref_id'][0]}"
            if key in TREE:
                return self.send(200, page(NAMES[key], [item(k, i) for k, i in TREE[key]]).encode())
        if url.path == "/goto.php" and target:
            if target.startswith("file_") and target.endswith("_download"):
                oid = target[5:-9]
                if oid in FILES:
                    version = state["version"] if oid == "102" else 1
                    return self.send(200, pdf(f"{FILES[oid][0]} Version {version}"), "application/pdf",
                                     [("Content-Disposition", f'attachment; filename="{FILES[oid][1]}"')])
            if target in TREE:
                return self.send(200, page(NAMES[target], [item(k, i) for k, i in TREE[target]]).encode())
        self.send(404, b"<!doctype html><html><body>Nicht gefunden <a href='/ilias.php?baseClass=ilStartUpGUI&amp;cmd=doLogout'>Abmelden</a></body></html>")


if __name__ == "__main__":
    ThreadingHTTPServer(("127.0.0.1", int(sys.argv[1])), Handler).serve_forever()
