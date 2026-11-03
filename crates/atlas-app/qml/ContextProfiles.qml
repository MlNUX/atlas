// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

pragma Singleton
import QtQuick

// The context bar as a set of blocks (docs/concept/kontextleiste.html):
// which blocks there are, and which profile lays them out for a file.
//
// A profile has lists of block names: `tabs` (after „Übersicht“),
// `overview`, `pinned` (bottom edge, on every tab), `hidden`, `collapsed`.
// They live in `[context.profiles.<name>]` of config.toml. „Allgemein“
// (`default`) applies to everything without a profile of its own and fills
// what another profile leaves out. Without any profile the built-in layout
// applies, with the old switches (`calendar = false` …) as hidden blocks.
QtObject {
    id: profiles

    /// Blocks: name, icon, whether it may be a tab or pinned, group of the
    /// gallery and a line for it.
    readonly property var catalog: ({
        annotations: { name: qsTr("Anmerkungen"), icon: "highlighter", tab: true, pin: false, group: "file",
                       detail: qsTr("Markierungen und Kommentare der PDF") },
        outline: { name: qsTr("Gliederung"), icon: "list", tab: true, pin: false, group: "file",
                   detail: qsTr("Lesezeichen der PDF bzw. Überschriften der Notiz") },
        links: { name: qsTr("Verknüpfungen"), icon: "link", tab: true, pin: false, group: "file",
                 detail: qsTr("Links und Backlinks mit Filtern") },
        tags: { name: qsTr("Tags"), icon: "bookmark", tab: false, pin: false, group: "file",
                detail: qsTr("#Tags der Notiz, Klick sucht im Projekt") },
        fileinfo: { name: qsTr("Dateiinfo"), icon: "info", tab: false, pin: false, group: "file",
                    detail: qsTr("Größe, Seiten, geändert, Pfad") },
        graph: { name: qsTr("Graph"), icon: "graph", tab: true, pin: false, group: "file",
                 detail: qsTr("Verbundene Dateien rund um diese") },
        actions: { name: qsTr("Aktionen"), icon: "play", tab: true, pin: false, group: "file",
                   detail: qsTr("Eingebaute und eigene Aktionen für die Datei") },
        today: { name: qsTr("Heute"), icon: "calendar", tab: false, pin: true, group: "plan",
                 detail: qsTr("Termine, Blöcke und Fristen von heute") },
        week: { name: qsTr("Diese Woche"), icon: "calendar", tab: false, pin: false, group: "plan",
                detail: qsTr("Fristen und Blöcke der nächsten 7 Tage") },
        tasks: { name: qsTr("Aufgaben"), icon: "check-square", tab: false, pin: true, group: "plan",
                 detail: qsTr("Offene Aufgaben abhaken und neue anlegen") },
        pomodoro: { name: qsTr("Lernzeit"), icon: "focus", tab: false, pin: true, group: "plan",
                    detail: qsTr("Pomodoro: lernen, Pause, nach vier Runden lange Pause") }
    })
    readonly property var groups: [["file", qsTr("Diese Datei")], ["plan", qsTr("Planung")], ["ext", qsTr("Erweiterungen")]]

    /// The layout without any profile (as the bar was before profiles).
    readonly property var builtin: ({
        tabs: ["links", "graph", "actions"],
        overview: ["today", "tasks", "annotations", "graph", "actions"],
        pinned: ["pomodoro"],
        hidden: [],
        collapsed: []
    })
    /// Profiles, in the order of the settings. Others than `default` exist
    /// only once set; until then „Allgemein“ applies.
    readonly property var names: ["default", "pdf", "markdown", "code", "calendar"]
    function title(name) {
        return ({ default: qsTr("Allgemein"), pdf: qsTr("PDF"), markdown: qsTr("Markdown"), code: qsTr("Code"),
                  calendar: qsTr("Kalender") })[name] || name
    }
    /// What a new profile starts with (the concept's suggestions).
    readonly property var suggestions: ({
        pdf: { tabs: ["links", "actions"], overview: ["annotations", "outline", "today", "graph", "actions"], collapsed: ["graph"] },
        markdown: { tabs: ["graph", "actions"], overview: ["links", "tags", "outline", "today"] },
        code: { tabs: ["actions"], overview: ["actions", "fileinfo"] },
        calendar: { tabs: [], overview: ["week", "tasks"] }
    })
    readonly property var keys: ["tabs", "overview", "pinned", "hidden", "collapsed"]

    /// Profile for a tab of the editor area by its kind.
    function kindOf(tab) {
        const k = tab ? tab.kind : ""
        return k === "pdf" || k === "atlasnote" ? "pdf" : k === "markdown" ? "markdown"
            : k === "text" || k === "notebook" ? "code" : k === "calendar" ? "calendar" : "default"
    }

    function info(id, extensions) {
        if (catalog[id]) return catalog[id]
        if (id.startsWith("ext:")) {
            const e = (extensions || []).find(x => "ext:" + x.name === id)
            return { name: e ? (e.title || e.name) : id.slice(4), icon: "grid", tab: false, pin: false, group: "ext",
                     detail: e ? (e.description || "") : "", letter: (e ? (e.title || e.name) : id.slice(4)).charAt(0).toUpperCase() }
        }
        return null
    }

    function copy(p) {
        const out = {}
        for (const k of keys) out[k] = (p[k] || []).slice()
        return out
    }
    function same(a, b) { return a.length === b.length && a.every((x, i) => x === b[i]) }

    /// `default` with all lists (built in, or as stored; old switches as
    /// hidden blocks while there are no profiles).
    function base(config) {
        const stored = (config.profiles || {}).default
        if (stored) {
            const out = copy(builtin)
            for (const k of keys) if (stored[k]) out[k] = stored[k].slice()
            return out
        }
        const out = copy(builtin)
        if (!config.profiles || Object.keys(config.profiles).length === 0) {
            if (config.calendar === false) out.hidden.push("today")
            if (config.tasks === false) out.hidden.push("tasks")
            if (config.pomodoro === false) out.hidden.push("pomodoro")
        }
        return out
    }
    /// Whether profile `name` is set (`default` always is).
    function isSet(config, name) { return name === "default" || !!(config.profiles || {})[name] }
    /// The profile used for `kind`.
    function profileFor(config, kind) {
        return config.by_kind !== false && kind !== "default" && isSet(config, kind) ? kind : "default"
    }
    /// All lists of profile `name`: as stored over `default`; a profile not
    /// set yet starts from the suggestion.
    function lists(config, name) {
        const d = base(config)
        if (name === "default") return d
        const stored = (config.profiles || {})[name]
        const over = stored || suggestions[name] || {}
        const out = copy(d)
        for (const k of keys) if (over[k]) out[k] = over[k].slice()
        return out
    }

    /// What the bar shows for profile `name`: {name, tabs, overview,
    /// pinned, collapsed, hidden} – only known blocks; extension cards that
    /// are shown but not placed come at the end of the overview.
    function layout(config, name, extensions) {
        const l = lists(config, name)
        const exts = extensions || []
        const hidden = l.hidden
        const known = (id) => !!catalog[id] || (id.startsWith("ext:") && exts.some(e => "ext:" + e.name === id))
        const tabs = l.tabs.filter(id => known(id) && catalog[id] && catalog[id].tab && hidden.indexOf(id) < 0)
        const pinned = l.pinned.filter(id => known(id) && catalog[id] && catalog[id].pin && hidden.indexOf(id) < 0 && tabs.indexOf(id) < 0)
        // A block may be a tab and in the overview (graph and actions are,
        // as before profiles), but not in the overview and at the bottom.
        const overview = []
        for (const id of l.overview)
            if (known(id) && hidden.indexOf(id) < 0 && pinned.indexOf(id) < 0 && overview.indexOf(id) < 0) overview.push(id)
        const mentioned = (id) => keys.some(k => k !== "collapsed" && l[k].indexOf(id) >= 0)
        for (const e of exts)
            if (e.shown && !mentioned("ext:" + e.name)) overview.push("ext:" + e.name)
        return { name: name, tabs: tabs, overview: overview, pinned: pinned, collapsed: l.collapsed, hidden: l.hidden }
    }

    /// Stores profile `name` with the lists `full`: `default` completely,
    /// another one only with what differs from `default`.
    function save(app, config, name, full) {
        let out = {}
        if (name === "default") out = copy(full)
        else {
            const d = base(config)
            for (const k of keys) if (!same(full[k] || [], d[k])) out[k] = (full[k] || []).slice()
        }
        app.setContextProfile(name, JSON.stringify(out))
    }
    /// Changes profile `name` by `change(lists)` and stores it. The first
    /// change of all also stores `default` (taking over the old switches).
    function edit(app, config, name, change) {
        const l = lists(config, name)
        change(l)
        if (name !== "default" && !(config.profiles || {}).default) save(app, config, "default", base(config))
        save(app, JSON.parse(app.contextModules || "{}"), name, l)
    }
    /// Removes `id` from every area (not from hidden / collapsed).
    function unplace(l, id) {
        for (const k of ["tabs", "overview", "pinned"]) {
            const i = l[k].indexOf(id)
            if (i >= 0) l[k].splice(i, 1)
        }
    }
    function toggle(list, id, on) {
        const i = list.indexOf(id)
        if (on && i < 0) list.push(id)
        if (!on && i >= 0) list.splice(i, 1)
    }
    /// Moves `id` within its area by `delta`, skipping blocks not shown
    /// (`visible`: the ids the bar shows there, in order).
    function move(l, area, id, delta, visible) {
        const list = l[area]
        const shown = visible || list
        const at = shown.indexOf(id)
        const to = at + delta
        if (at < 0 || to < 0 || to >= shown.length) return false
        const other = shown[to]
        // Visible order may include blocks the stored list lacks (extension
        // cards): store the visible order then.
        if (list.indexOf(id) < 0 || list.indexOf(other) < 0) {
            const order = shown.slice()
            order.splice(at, 1)
            order.splice(to, 0, id)
            const rest = list.filter(x => order.indexOf(x) < 0)
            l[area] = order.concat(rest)
            return true
        }
        const a = list.indexOf(id), b = list.indexOf(other)
        list[a] = other
        list[b] = id
        return true
    }
    /// One line for the settings: the overview, then the pinned blocks.
    function summary(config, name, extensions) {
        const l = layout(config, name, extensions)
        const names = (ids) => ids.map(id => (info(id, extensions) || { name: id }).name).join(" · ")
        return names(l.overview) + (l.pinned.length ? " · " + qsTr("unten: %1").arg(names(l.pinned)) : "")
    }
}
