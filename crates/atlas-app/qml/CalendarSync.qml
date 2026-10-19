// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// Dialogs around the project calendar: Nextcloud connection and calendar
// choice, the app password for this Atlas start, conflicts, and taking
// Markdown checklists over as tasks.
Item {
    id: sync
    required property AppController app
    required property var model
    property var actions: null

    function message(text) { if (actions) actions.showMessage(text) }

    function openSettings() { settings.open() }
    function askPassword() { passwordDialog.open() }
    function showConflicts() { conflictDialog.open() }
    function importChecklist(path) { checklistDialog.show(path) }

    Connections {
        target: sync.model
        function onSyncFinished(ok, text) { sync.message(text) }
        function onCalendarCreated(ok, text, url) {
            sync.message(text)
            if (ok) settings.refresh()
            else { settings.problem = text }
        }
    }

    // ------------------------------------------------------ app password

    Dialog {
        id: passwordDialog
        anchors.centerIn: Overlay.overlay
        width: 420
        modal: true
        title: qsTr("Nextcloud-Kalender")
        standardButtons: Dialog.Ok | Dialog.Cancel
        onAboutToShow: { passwordField.text = ""; Qt.callLater(passwordField.forceActiveFocus) }
        onAccepted: if (passwordField.text) {
            sync.model.rememberPassword(rememberOnce.checked)
            sync.model.setPassword(passwordField.text)
        }
        ColumnLayout {
            anchors.fill: parent
            spacing: 8
            Label {
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                text: qsTr("App-Passwort für %1 bei %2. Gemerkt wird es verschlüsselt mit systemd-creds (an diesen Rechner gebunden), sonst nur für diese Atlas-Sitzung im Arbeitsspeicher gehalten.")
                    .arg(sync.accountInfo().user || "?").arg(sync.accountInfo().server || sync.accountInfo().davUrl || "?")
            }
            Label {
                Layout.fillWidth: true
                visible: sync.model.passwordError !== ""
                wrapMode: Text.Wrap
                color: Theme.error
                font.pixelSize: Theme.smallFontSize
                text: qsTr("Passwort-Befehl fehlgeschlagen: %1").arg(sync.model.passwordError)
            }
            TextField {
                id: passwordField
                Layout.fillWidth: true
                echoMode: TextInput.Password
                placeholderText: qsTr("App-Passwort")
                onAccepted: passwordDialog.accept()
            }
            CheckBox {
                id: rememberOnce
                text: qsTr("Auf diesem Rechner merken")
                checked: true
            }
        }
    }

    function accountInfo() {
        try { return JSON.parse(model.account()) } catch (e) { return {} }
    }

    // --------------------------------------------------------- settings

    Dialog {
        id: settings
        anchors.centerIn: Overlay.overlay
        width: Math.min(640, Overlay.overlay ? Overlay.overlay.width - 40 : 640)
        height: Math.min(720, Overlay.overlay ? Overlay.overlay.height - 40 : 720)
        modal: true
        title: qsTr("Kalender-Einstellungen")
        standardButtons: Dialog.Close
        property var calendars: []
        /// Calendars found by "Verbindung testen" (before saving).
        property var found: []
        property string problem: ""
        property string status: ""
        property bool testing: false
        property bool advanced: false

        function refresh() {
            try { calendars = JSON.parse(sync.model.calendars()) } catch (e) { calendars = [] }
        }
        onAboutToShow: {
            const a = sync.accountInfo()
            serverField.text = a.server || ""
            userField.text = a.user || ""
            davField.text = a.davUrl || ""
            commandField.text = a.passwordCommand || ""
            rememberBox.checked = a.rememberPassword !== undefined && sync.model.connected ? a.rememberPassword : true
            advanced = !!a.davUrl
            appPassword.text = ""
            found = []
            problem = ""
            status = ""
            refresh()
        }
        Connections {
            target: sync.model
            function onRevisionChanged() { if (settings.visible) settings.refresh() }
            function onConnectionTested(ok, text, list) {
                settings.testing = false
                settings.status = ok ? text : ""
                settings.problem = ok ? "" : text
                try { settings.found = ok ? JSON.parse(list) : [] } catch (e) { settings.found = [] }
            }
        }

        contentItem: ScrollView {
            id: settingsScroll
            contentWidth: availableWidth
            clip: true
            ColumnLayout {
                width: settingsScroll.availableWidth
                spacing: 8

                Label { text: qsTr("NEXTCLOUD-VERBINDUNG"); font.bold: true; color: Theme.text; font.pixelSize: Theme.smallFontSize }
                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    color: Theme.textMuted
                    font.pixelSize: Theme.smallFontSize
                    text: qsTr("Atlas nutzt CalDAV, keine Nextcloud-App nötig. Lege in Nextcloud unter Einstellungen → Sicherheit ein App-Passwort an. Atlas merkt es sich auf Wunsch verschlüsselt auf diesem Rechner; alternativ holt ein Passwort-Befehl es z. B. aus pass oder rbw.")
                }
                GridLayout {
                    Layout.fillWidth: true
                    columns: 2
                    Label { text: qsTr("Server"); color: Theme.textMuted }
                    TextField { id: serverField; Layout.fillWidth: true; placeholderText: "https://cloud.example.com" }
                    Label { text: qsTr("Benutzer"); color: Theme.textMuted }
                    TextField { id: userField; Layout.fillWidth: true }
                    Label { text: qsTr("App-Passwort"); color: Theme.textMuted }
                    TextField {
                        id: appPassword
                        Layout.fillWidth: true
                        echoMode: TextInput.Password
                        placeholderText: sync.model.unlocked ? qsTr("bekannt für diese Sitzung") : ""
                    }
                    Item { width: 1; height: 1 }
                    RowLayout {
                        Layout.fillWidth: true
                        CheckBox {
                            id: rememberBox
                            text: qsTr("Auf diesem Rechner merken")
                            onToggled: if (sync.model.connected) sync.model.rememberPassword(checked)
                        }
                        Label {
                            Layout.fillWidth: true
                            elide: Text.ElideRight
                            font.pixelSize: Theme.smallFontSize
                            color: sync.model.remembered === "file" ? Theme.warn : Theme.textMuted
                            text: sync.model.remembered === "encrypted" ? qsTr("✓ verschlüsselt gespeichert (systemd-creds, nur auf diesem Rechner)")
                                : sync.model.remembered === "file" ? qsTr("gespeichert ohne Verschlüsselung (systemd-creds fehlt), nur für deinen Benutzer lesbar")
                                : ""
                        }
                    }
                    Label { text: qsTr("Passwort-Befehl"); color: Theme.textMuted }
                    TextField {
                        id: commandField
                        Layout.fillWidth: true
                        placeholderText: qsTr("optional, z. B. pass show nextcloud/atlas")
                        ToolTip.visible: hovered
                        ToolTip.delay: 600
                        ToolTip.text: qsTr("Wird beim Start ohne Shell ausgeführt; die erste Zeile der Ausgabe ist das App-Passwort. Beispiele: rbw get nextcloud · keepassxc-cli show -sa password db.kdbx Nextcloud · systemd-creds --user decrypt nextcloud.cred")
                    }
                    Label { text: qsTr("CalDAV-URL"); color: Theme.textMuted; visible: settings.advanced }
                    TextField {
                        id: davField
                        visible: settings.advanced
                        Layout.fillWidth: true
                        placeholderText: qsTr("nur falls die automatische Erkennung scheitert")
                    }
                }
                RowLayout {
                    Button {
                        text: settings.testing ? qsTr("Prüfe …") : qsTr("Verbindung testen")
                        enabled: !settings.testing && userField.text && (serverField.text || davField.text) && (appPassword.text || sync.model.unlocked || commandField.text.trim())
                        onClicked: {
                            settings.testing = true
                            settings.problem = ""
                            settings.status = ""
                            sync.model.testConnection(serverField.text, userField.text, appPassword.text, davField.text, commandField.text)
                        }
                    }
                    Button {
                        text: sync.model.connected ? qsTr("Speichern") : qsTr("Verbinden")
                        highlighted: settings.found.length > 0
                        enabled: settings.found.length > 0 || (sync.model.connected && appPassword.text === "")
                            || (sync.model.connected && commandField.text.trim() !== (sync.accountInfo().passwordCommand || ""))
                        onClicked: {
                            // Newly found calendars are visible; known ones keep their choice.
                            const known = settings.calendars.filter(c => c.remote)
                            const list = settings.found.length
                                ? settings.found.map(c => ({ url: c.url, name: c.name, visible: (known.find(k => k.id === c.url) || { visible: true }).visible }))
                                : known.map(c => ({ url: c.id, name: c.name, visible: c.visible }))
                            sync.model.rememberPassword(rememberBox.checked)
                            sync.model.saveAccount(serverField.text, userField.text, appPassword.text, davField.text, JSON.stringify(list), commandField.text)
                            appPassword.text = ""
                            settings.found = []
                            settings.status = qsTr("Gespeichert")
                            settings.refresh()
                        }
                    }
                    Item { Layout.fillWidth: true }
                    CheckBox { text: qsTr("Erweitert"); checked: settings.advanced; onToggled: settings.advanced = checked }
                }
                Label { visible: settings.status !== ""; text: "✓ " + settings.status; color: Theme.ok; Layout.fillWidth: true; wrapMode: Text.Wrap }
                Label { visible: settings.problem !== ""; text: "⚠ " + settings.problem; color: Theme.error; Layout.fillWidth: true; wrapMode: Text.Wrap }
                Repeater {
                    model: settings.found
                    Label {
                        required property var modelData
                        text: "  • " + modelData.name + (modelData.writable ? "" : qsTr(" (schreibgeschützt)"))
                        color: Theme.textMuted
                        font.pixelSize: Theme.smallFontSize
                    }
                }

                Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.border; Layout.topMargin: 6 }
                Label { text: qsTr("KALENDER"); font.bold: true; color: Theme.text; font.pixelSize: Theme.smallFontSize }
                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    color: Theme.textMuted
                    font.pixelSize: Theme.smallFontSize
                    text: qsTr("Sichtbare Kalender werden hier angezeigt und synchronisiert. Der Standard gilt für neue Einträge – für alle Projekte oder nur für dieses.")
                }
                Repeater {
                    model: settings.calendars
                    RowLayout {
                        required property var modelData
                        Layout.fillWidth: true
                        spacing: 6
                        Rectangle { width: 12; height: 12; radius: 6; color: modelData.color }
                        Label {
                            Layout.fillWidth: true
                            text: modelData.name + (modelData.missing ? qsTr(" – nicht mehr auf dem Server") : "")
                            color: modelData.missing ? Theme.warn : Theme.text
                            elide: Text.ElideRight
                        }
                        Label {
                            text: modelData.writable ? "✎" : "🔒"
                            color: Theme.textMuted
                            ToolTip.visible: accessHover.hovered
                            ToolTip.text: modelData.writable ? qsTr("Lesen und schreiben") : qsTr("Nur lesen")
                            HoverHandler { id: accessHover }
                        }
                        CheckBox {
                            visible: modelData.remote
                            text: qsTr("sichtbar")
                            checked: modelData.visible
                            onToggled: sync.model.setCalendarVisible(modelData.id, checked)
                        }
                        CheckBox {
                            visible: modelData.remote
                            text: qsTr("nur hier")
                            checked: modelData.projectOnly
                            ToolTip.visible: hovered
                            ToolTip.text: qsTr("Nur in diesem Projekt anzeigen")
                            onToggled: sync.model.setCalendarProjectOnly(modelData.id, checked)
                        }
                        Button {
                            flat: true
                            enabled: modelData.writable && modelData.visible
                            text: modelData.isDefault ? (modelData.projectDefault ? qsTr("★ Standard (Projekt)") : qsTr("★ Standard")) : qsTr("☆")
                            ToolTip.visible: hovered
                            ToolTip.text: qsTr("Als Standard für neue Einträge festlegen")
                            onClicked: defaultMenu.popupFor(modelData.id)
                        }
                    }
                }
                Menu {
                    id: defaultMenu
                    property string calendarId
                    function popupFor(id) { calendarId = id; popup() }
                    MenuItem { text: qsTr("Standard für alle Projekte"); onTriggered: { sync.model.setDefaultCalendar(defaultMenu.calendarId, false); sync.model.setDefaultCalendar("", true) } }
                    MenuItem { text: qsTr("Standard nur für dieses Projekt"); onTriggered: sync.model.setDefaultCalendar(defaultMenu.calendarId, true) }
                }

                Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.border; Layout.topMargin: 6; visible: sync.model.connected }
                Label { visible: sync.model.connected; text: qsTr("SYNCHRONISIERUNG"); font.bold: true; color: Theme.text; font.pixelSize: Theme.smallFontSize }
                Label {
                    visible: sync.model.connected
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    text: sync.model.syncStatus + (sync.model.pending ? qsTr(" · %n Änderung(en) warten", "", sync.model.pending) : "")
                        + (sync.model.conflictCount ? qsTr(" · %n Konflikt(e)", "", sync.model.conflictCount) : "")
                    color: sync.model.syncFailed ? Theme.warn : Theme.textMuted
                }
                Flow {
                    visible: sync.model.connected
                    Layout.fillWidth: true
                    spacing: 6
                    Button { text: qsTr("Jetzt synchronisieren"); enabled: sync.model.unlocked && !sync.model.syncing; onClicked: sync.model.syncNow() }
                    Button { text: qsTr("App-Passwort neu eingeben …"); onClicked: passwordDialog.open() }
                    Button { text: qsTr("Konflikte lösen …"); visible: sync.model.conflictCount > 0; onClicked: conflictDialog.open() }
                    Button { text: qsTr("„Atlas – Projektarbeit“ anlegen …"); enabled: sync.model.unlocked; onClicked: createDialog.open() }
                    Button { text: qsTr("Lokalen Cache leeren"); onClicked: confirm.ask(qsTr("Cache leeren?"),
                        qsTr("Die zwischengespeicherten Nextcloud-Termine dieses Projekts werden entfernt und neu geladen.") + (sync.model.pending
                            ? qsTr(" %n noch nicht übertragene Änderung(en) gehen verloren.", "", sync.model.pending) : ""),
                        qsTr("Leeren"), () => sync.model.clearCache()) }
                    Button { text: qsTr("Konto trennen"); onClicked: confirm.ask(qsTr("Konto trennen?"),
                        qsTr("Atlas vergisst Verbindung, App-Passwort und Cache. Auf dem Nextcloud-Server wird nichts gelöscht."),
                        qsTr("Trennen"), () => { sync.model.disconnectAccount(); settings.refresh() }) }
                }
            }
        }
    }

    Dialog {
        id: createDialog
        anchors.centerIn: Overlay.overlay
        width: 440
        modal: true
        title: qsTr("Kalender in Nextcloud anlegen?")
        standardButtons: Dialog.Cancel
        onAboutToShow: nameField.text = qsTr("Atlas – Projektarbeit")
        ColumnLayout {
            anchors.fill: parent
            Label {
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                text: qsTr("Atlas legt in deinem Nextcloud-Konto einen neuen Kalender an. Er ist danach auch in anderen Programmen sichtbar.")
            }
            TextField { id: nameField; Layout.fillWidth: true }
        }
        footer: DialogButtonBox {
            Button { text: qsTr("Abbrechen"); DialogButtonBox.buttonRole: DialogButtonBox.RejectRole }
            Button {
                text: qsTr("Anlegen")
                DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
                onClicked: if (nameField.text.trim()) sync.model.createCalendar(nameField.text.trim())
            }
        }
    }

    Dialog {
        id: confirm
        property var run: null
        property alias text: confirmText.text
        property alias action: confirmButton.text
        anchors.centerIn: Overlay.overlay
        width: 420
        modal: true
        function ask(title, text, action, run) { confirm.title = title; confirm.text = text; confirm.action = action; confirm.run = run; open() }
        Label { id: confirmText; width: parent.width; wrapMode: Text.Wrap }
        footer: DialogButtonBox {
            Button { text: qsTr("Abbrechen"); DialogButtonBox.buttonRole: DialogButtonBox.RejectRole }
            Button { id: confirmButton; DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole; onClicked: if (confirm.run) confirm.run() }
        }
    }

    // -------------------------------------------------------- conflicts

    Dialog {
        id: conflictDialog
        anchors.centerIn: Overlay.overlay
        width: Math.min(640, Overlay.overlay ? Overlay.overlay.width - 40 : 640)
        height: Math.min(600, Overlay.overlay ? Overlay.overlay.height - 40 : 600)
        modal: true
        title: qsTr("Kalenderkonflikte")
        standardButtons: Dialog.Close
        property var list: []
        function refresh() { try { list = JSON.parse(sync.model.conflicts()) } catch (e) { list = [] } }
        onAboutToShow: refresh()
        Connections { target: sync.model; function onRevisionChanged() { if (conflictDialog.visible) conflictDialog.refresh() } }

        contentItem: ListView {
            id: conflictList
            clip: true
            spacing: 10
            model: conflictDialog.list
            ScrollBar.vertical: ScrollBar {}
            Label {
                anchors.centerIn: parent
                visible: conflictList.count === 0
                text: qsTr("Keine Konflikte.")
                color: Theme.textFaint
            }
            delegate: Rectangle {
                id: conflict
                required property var modelData
                width: ListView.view.width
                height: conflictColumn.implicitHeight + 16
                radius: 4
                color: Theme.sidebar
                border.color: Theme.border
                function version(v, deleted) {
                    if (!v) return deleted
                    return v.title + (v.when ? "\n" + v.when : "") + (v.description ? "\n" + v.description.slice(0, 120) : "")
                }
                ColumnLayout {
                    id: conflictColumn
                    x: 8
                    y: 8
                    width: parent.width - 16
                    spacing: 6
                    Label {
                        Layout.fillWidth: true
                        wrapMode: Text.Wrap
                        font.bold: true
                        color: Theme.textStrong
                        text: conflict.modelData.message
                            ? qsTr("„%1“ (%2): Nextcloud hat die Änderung abgelehnt – %3").arg(conflict.modelData.title).arg(conflict.modelData.calendar).arg(conflict.modelData.message)
                            : qsTr("„%1“ wurde in Nextcloud und Atlas geändert (%2).").arg(conflict.modelData.title).arg(conflict.modelData.calendar)
                    }
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 8
                        Repeater {
                            model: [{ t: qsTr("NEXTCLOUD"), v: conflict.modelData.remote, d: qsTr("auf dem Server gelöscht") },
                                    { t: qsTr("ATLAS"), v: conflict.modelData.local, d: qsTr("in Atlas gelöscht") }]
                            Rectangle {
                                required property var modelData
                                Layout.fillWidth: true
                                Layout.preferredHeight: versionText.implicitHeight + 26
                                radius: 3
                                color: Theme.editor
                                Label { x: 6; y: 4; text: parent.modelData.t; color: Theme.textMuted; font.pixelSize: 10; font.bold: true }
                                Label {
                                    id: versionText
                                    x: 6
                                    y: 20
                                    width: parent.width - 12
                                    wrapMode: Text.Wrap
                                    text: conflict.version(parent.modelData.v, parent.modelData.d)
                                    color: parent.modelData.v ? Theme.text : Theme.textFaint
                                    font.pixelSize: Theme.smallFontSize
                                }
                            }
                        }
                    }
                    RowLayout {
                        Layout.alignment: Qt.AlignRight
                        Button { text: qsTr("Nextcloud-Version"); onClicked: sync.model.resolveConflict(conflict.modelData.id, "remote") }
                        Button { text: qsTr("Lokale Version"); onClicked: sync.model.resolveConflict(conflict.modelData.id, "local") }
                        Button {
                            text: qsTr("Beide behalten")
                            enabled: !!conflict.modelData.local && !!conflict.modelData.remote
                            onClicked: sync.model.resolveConflict(conflict.modelData.id, "both")
                        }
                    }
                }
            }
        }
    }

    // ---------------------------------------- Markdown checklist → tasks

    Dialog {
        id: checklistDialog
        anchors.centerIn: Overlay.overlay
        width: Math.min(560, Overlay.overlay ? Overlay.overlay.width - 40 : 560)
        height: Math.min(520, Overlay.overlay ? Overlay.overlay.height - 40 : 520)
        modal: true
        property string path
        property var items: []
        property var chosen: ({})
        title: qsTr("Checkliste als Aufgaben übernehmen")
        function show(p) {
            path = p
            try { items = JSON.parse(sync.model.checklistOf(p)) } catch (e) { items = [] }
            const c = {}
            for (const it of items) c[it.line] = !it.done && !it.planned
            chosen = c
            open()
        }
        contentItem: ColumnLayout {
            spacing: 6
            Label {
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                color: Theme.textMuted
                text: checklistDialog.items.length
                    ? qsTr("Gewählte Punkte aus %1 werden zu Aufgaben unter „Noch nicht eingeplant“ und mit der Datei verknüpft. Die Datei bleibt unverändert.").arg(sync.app.relativePath(checklistDialog.path))
                    : qsTr("%1 enthält keine Checkliste (- [ ] …).").arg(sync.app.fileName(checklistDialog.path))
            }
            ListView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                model: checklistDialog.items
                ScrollBar.vertical: ScrollBar {}
                delegate: CheckBox {
                    required property var modelData
                    width: ListView.view.width
                    text: (modelData.done ? "✓ " : "") + modelData.text + (modelData.planned ? qsTr("  (schon im Kalender)") : "")
                    checked: !!checklistDialog.chosen[modelData.line]
                    onToggled: { const c = Object.assign({}, checklistDialog.chosen); c[modelData.line] = checked; checklistDialog.chosen = c }
                }
            }
        }
        footer: DialogButtonBox {
            Button { text: qsTr("Abbrechen"); DialogButtonBox.buttonRole: DialogButtonBox.RejectRole }
            Button {
                text: qsTr("Übernehmen")
                enabled: checklistDialog.items.some(it => checklistDialog.chosen[it.line])
                DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
                onClicked: {
                    const picked = checklistDialog.items.filter(it => checklistDialog.chosen[it.line]).map(it => ({ line: it.line, text: it.text }))
                    const n = sync.model.addTasks(checklistDialog.path, JSON.stringify(picked))
                    sync.message(qsTr("%n Aufgabe(n) in den Kalender übernommen", "", n))
                }
            }
        }
    }
}
