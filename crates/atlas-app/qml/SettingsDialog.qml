// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// General settings (stored in config.toml) and the way to the specific
// ones – external editor, calendar and Nextcloud, extensions, actions –
// each with its current state.
Dialog {
    id: dialog
    required property AppController app
    property var actions: null

    signal extensionsRequested()

    anchors.centerIn: Overlay.overlay
    width: Math.min(parent ? parent.width - 60 : 600, 600)
    height: Math.min(implicitHeight, parent ? parent.height - 40 : implicitHeight)
    modal: true
    padding: 0
    topPadding: 0
    title: ""

    readonly property int extraPlaces: dialog.app.placeNames.length - dialog.app.fixedPlaceCount
    /// Read when the dialog opens.
    property string editorText
    property int extensionCount: 0
    onAboutToShow: {
        const command = dialog.app.editorCommand()
        editorText = command ? command : qsTr("Standard: $VISUAL, $EDITOR oder das Programm des Systems")
        try { extensionCount = JSON.parse(dialog.app.extensions()).length } catch (e) { extensionCount = 0 }
    }
    readonly property var calendar: dialog.actions ? dialog.actions.calendar : null
    /// The calendars offered for new events (for the smoke test).
    readonly property alias defaultCalendarChoices: defaultCalendarRow.choices
    /// The ILIAS address field and how to apply it (for the smoke test).
    readonly property alias iliasAddressField: iliasAddress
    function applyIliasAddress() { iliasCard.applyAddress() }
    /// The login fields (for the smoke test).
    function rememberIliasLogin(user, password) { iliasUser.text = user; iliasPassword.text = password; iliasCard.rememberLogin() }
    readonly property string iliasLoginMessage: iliasCard.loginMessage
    /// Where the places come from, for the line under the switch.
    readonly property string placesText: !dialog.app.fileManagerPlaces ? qsTr("Feste Orte von Atlas")
        : dialog.app.fixedPlaceCount === 0 ? qsTr("Aus Dolphin: %1").arg(dialog.app.placeNames.join(", ") || qsTr("keine"))
        : dialog.extraPlaces > 0 ? qsTr("Keine Dolphin-Orte gefunden – aus GTK: %1").arg(dialog.app.placeNames.slice(dialog.app.fixedPlaceCount).join(", "))
        : qsTr("Keine Dolphin-Orte gefunden – feste Orte von Atlas")

    function run(f) { dialog.close(); f() }
    readonly property var context: { try { return JSON.parse(dialog.app.contextModules || "{}") } catch (e) { return {} } }

    contentItem: ColumnLayout {
        spacing: 0

        // ---- header
        RowLayout {
            Layout.fillWidth: true
            Layout.margins: 18
            Layout.bottomMargin: 14
            spacing: 12
            Rectangle {
                Layout.preferredWidth: 36
                Layout.preferredHeight: 36
                radius: 10
                color: Theme.accentSoft
                Icon { anchors.centerIn: parent; name: "settings"; size: 20; color: Theme.accent }
            }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2
                Text {
                    text: qsTr("Einstellungen")
                    color: Theme.textStrong
                    font.pixelSize: 16
                    font.weight: Font.DemiBold
                }
                Text {
                    Layout.fillWidth: true
                    text: qsTr("Gespeichert in ~/.config/atlas/config.toml")
                    color: Theme.textMuted
                    font.pixelSize: Theme.smallFontSize
                    elide: Text.ElideRight
                }
            }
            IconButton { iconName: "close"; tip: qsTr("Schließen (Esc)"); onClicked: dialog.close() }
        }
        Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.border }

        // Scrolls on small screens (the dialog never grows past the window).
        ScrollView {
            id: body
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.preferredHeight: bodyColumn.implicitHeight + 36
            contentWidth: availableWidth
            contentHeight: bodyColumn.implicitHeight + 36
            clip: true
            ColumnLayout {
                id: bodyColumn
                x: 18
                y: 18
                width: body.availableWidth - 36
                spacing: 8

                SectionLabel { text: qsTr("Seitenleiste") }
                Card {
                    Layout.fillWidth: true
                    implicitHeight: placesRow.implicitHeight + 24
                    RowLayout {
                        id: placesRow
                        anchors.fill: parent
                        anchors.margins: 12
                        spacing: 12
                        Icon { Layout.alignment: Qt.AlignTop; name: "home"; size: 18; color: Theme.accent }
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 3
                            Text {
                                text: qsTr("Orte wie in Dolphin")
                                color: Theme.textStrong
                                font.pixelSize: Theme.fontSize
                            }
                            Text {
                                Layout.fillWidth: true
                                wrapMode: Text.Wrap
                                color: Theme.textMuted
                                font.pixelSize: Theme.smallFontSize
                                text: qsTr("Dieselben Orte in derselben Reihenfolge wie in Dolphin; was du dort entfernst, fehlt auch hier. Netzwerk-Orte und Laufwerke nicht.")
                            }
                            Text {
                                Layout.fillWidth: true
                                Layout.topMargin: 2
                                text: dialog.placesText
                                color: Theme.text
                                font.pixelSize: Theme.smallFontSize
                                elide: Text.ElideRight
                            }
                        }
                        Toggle {
                            id: fileManagerToggle
                            Layout.alignment: Qt.AlignVCenter
                            checked: dialog.app.fileManagerPlaces
                            onToggled: dialog.app.fileManagerPlaces = !dialog.app.fileManagerPlaces
                        }
                    }
                }

                SectionLabel { text: qsTr("Dateien"); Layout.topMargin: 10 }
                Card {
                    Layout.fillWidth: true
                    implicitHeight: sortRow.implicitHeight + 24
                    RowLayout {
                        id: sortRow
                        anchors.fill: parent
                        anchors.margins: 12
                        spacing: 12
                        Icon { Layout.alignment: Qt.AlignTop; name: "list"; size: 18; color: Theme.accent }
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 3
                            Text { text: qsTr("Natürliche Sortierung"); color: Theme.textStrong; font.pixelSize: Theme.fontSize }
                            Text {
                                Layout.fillWidth: true
                                wrapMode: Text.Wrap
                                color: Theme.textMuted
                                font.pixelSize: Theme.smallFontSize
                                text: qsTr("Zahlen nach ihrem Wert: „2.pdf“ vor „10.pdf“. Gilt für Dateiansicht und Projektbaum; die zuletzt gewählte Sortierung merkt sich Atlas für neue Ansichten.")
                            }
                        }
                        Toggle {
                            Layout.alignment: Qt.AlignVCenter
                            checked: dialog.app.naturalSort
                            onToggled: dialog.app.naturalSort = !dialog.app.naturalSort
                        }
                    }
                }

                Card {
                    Layout.fillWidth: true
                    implicitHeight: flingRow.implicitHeight + 24
                    RowLayout {
                        id: flingRow
                        anchors.fill: parent
                        anchors.margins: 12
                        spacing: 12
                        Icon { Layout.alignment: Qt.AlignTop; name: "arrow-up"; size: 18; color: Theme.accent }
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 3
                            Text { text: qsTr("Schwung beim Touchpad-Scrollen"); color: Theme.textStrong; font.pixelSize: Theme.fontSize }
                            Text {
                                Layout.fillWidth: true
                                wrapMode: Text.Wrap
                                color: Theme.textMuted
                                font.pixelSize: Theme.smallFontSize
                                text: qsTr("Nach einem schnellen Wischen mit zwei Fingern läuft die Ansicht weiter und bremst sanft ab; erneutes Berühren hält sie an. PDFs, Editor, Projektbaum und Ordner. Mausräder bleiben unverändert.")
                            }
                        }
                        Toggle {
                            Layout.alignment: Qt.AlignVCenter
                            checked: dialog.app.touchpadFling
                            onToggled: dialog.app.touchpadFling = !dialog.app.touchpadFling
                        }
                    }
                }

                Card {
                    Layout.fillWidth: true
                    implicitHeight: spellRow.implicitHeight + 24
                    RowLayout {
                        id: spellRow
                        anchors.fill: parent
                        anchors.margins: 12
                        spacing: 12
                        Icon { Layout.alignment: Qt.AlignTop; name: "pen"; size: 18; color: Theme.accent }
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 3
                            Text { text: qsTr("Rechtschreibprüfung in Notizen"); color: Theme.textStrong; font.pixelSize: Theme.fontSize }
                            Text {
                                Layout.fillWidth: true
                                wrapMode: Text.Wrap
                                color: Theme.textMuted
                                font.pixelSize: Theme.smallFontSize
                                readonly property string languages: dialog.visible ? dialog.app.spellLanguages() : ""
                                text: (languages ? qsTr("Deutsch und Englisch zugleich (%1), auf diesem Rechner. ").arg(languages)
                                                 : qsTr("Keine Wörterbücher gefunden (Hunspell de_DE, en_US). "))
                                      + qsTr("Rechtsklick auf ein rot unterstrichenes Wort: Vorschläge oder ins eigene Wörterbuch.")
                            }
                        }
                        Toggle {
                            Layout.alignment: Qt.AlignVCenter
                            checked: dialog.app.spellcheck
                            onToggled: dialog.app.spellcheck = !dialog.app.spellcheck
                        }
                    }
                }

                SectionLabel { text: qsTr("ILIAS"); Layout.topMargin: 10 }
                Card {
                    Layout.fillWidth: true
                    id: iliasCard
                    implicitHeight: iliasColumn.implicitHeight + 24
                    readonly property var ilias: { try { return JSON.parse(dialog.app.iliasConfig) } catch (e) { return {} } }
                    property string loginMessage: ""
                    property bool loginFailed: false
                    property bool storing: false
                    function rememberLogin() {
                        if (!iliasUser.text.trim() || !iliasPassword.text || storing) return
                        storing = true
                        loginMessage = qsTr("Verschlüssele …")
                        loginFailed = false
                        dialog.app.iliasRememberLogin(iliasUser.text.trim(), iliasPassword.text)
                        iliasPassword.text = ""
                    }
                    Connections {
                        target: dialog.app
                        function onIliasLoginStored(ok, message) {
                            iliasCard.storing = false
                            iliasCard.loginFailed = !ok
                            iliasCard.loginMessage = message
                        }
                    }
                    function applyAddress() {
                        if (!iliasAddress.valid || !iliasAddress.changed) return
                        dialog.app.setIliasConfig("base_url", iliasAddress.address)
                        iliasAddress.text = Qt.binding(() => iliasCard.ilias.base_url || "")
                    }
                    ColumnLayout {
                        id: iliasColumn
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.margins: 12
                        spacing: 8
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 12
                            Icon { Layout.alignment: Qt.AlignTop; name: "school"; size: 18; color: Theme.accent }
                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 3
                                Text { text: qsTr("Favoriten als Ordner unter „Orte“"); color: Theme.textStrong; font.pixelSize: Theme.fontSize }
                                Text {
                                    Layout.fillWidth: true
                                    wrapMode: Text.Wrap
                                    color: Theme.textMuted
                                    font.pixelSize: Theme.smallFontSize
                                    text: qsTr("Die Favoriten deines ILIAS-Dashboards mit Ordnern, Gruppen, Sitzungen und Dateien; ein Klick lädt eine Datei und öffnet sie. Atlas merkt sich nur die Anmeldung, nicht das Passwort.")
                                }
                            }
                            Toggle {
                                Layout.alignment: Qt.AlignVCenter
                                checked: iliasCard.ilias.show_in_places !== false
                                onToggled: dialog.app.setIliasConfig("show_in_places", checked ? "false" : "true")
                            }
                        }
                        // The ILIAS of one's university: its start page.
                        RowLayout {
                            Layout.fillWidth: true
                            Layout.leftMargin: 30
                            spacing: 8
                            Text { text: qsTr("Adresse"); color: Theme.textMuted; font.pixelSize: Theme.smallFontSize; Layout.preferredWidth: 70 }
                            TextField {
                                id: iliasAddress
                                Layout.fillWidth: true
                                text: iliasCard.ilias.base_url || ""
                                placeholderText: "https://ilias.studium.kit.edu/"
                                font.pixelSize: Theme.smallFontSize
                                selectByMouse: true
                                inputMethodHints: Qt.ImhUrlCharactersOnly | Qt.ImhNoAutoUppercase
                                // http(s), with a host.
                                readonly property string address: {
                                    let t = text.trim()
                                    if (t && !/^[a-z]+:\/\//i.test(t)) t = "https://" + t
                                    return t
                                }
                                readonly property bool valid: { try { const u = new URL(address); return /^https?:$/.test(u.protocol) && u.host.length > 0 } catch (e) { return false } }
                                readonly property bool changed: address.replace(/\/?$/, "/") !== (iliasCard.ilias.base_url || "")
                                onAccepted: iliasCard.applyAddress()
                            }
                            Button {
                                text: qsTr("Übernehmen")
                                enabled: iliasAddress.valid && iliasAddress.changed
                                onClicked: iliasCard.applyAddress()
                            }
                        }
                        Text {
                            Layout.fillWidth: true
                            Layout.leftMargin: 108
                            wrapMode: Text.Wrap
                            font.pixelSize: Theme.smallFontSize
                            color: iliasAddress.text.trim() && !iliasAddress.valid ? Theme.error : Theme.textFaint
                            text: iliasAddress.text.trim() && !iliasAddress.valid
                                ? qsTr("Keine gültige Adresse – z. B. https://ilias.uni-beispiel.de/")
                                : qsTr("Startseite des ILIAS deiner Hochschule. Nach einer Änderung meldest du dich dort an und liest die Favoriten neu ein.")
                        }
                        // The university login, remembered only encrypted.
                        RowLayout {
                            Layout.fillWidth: true
                            Layout.leftMargin: 30
                            spacing: 8
                            Text { text: qsTr("Anmeldung"); color: Theme.textMuted; font.pixelSize: Theme.smallFontSize; Layout.preferredWidth: 70 }
                            Text {
                                visible: iliasCard.ilias.login_saved === true
                                Layout.fillWidth: true
                                text: qsTr("%1 · Passwort verschlüsselt gespeichert (systemd-creds)").arg(iliasCard.ilias.user || "")
                                color: Theme.text
                                font.pixelSize: Theme.smallFontSize
                                elide: Text.ElideRight
                            }
                            Button {
                                visible: iliasCard.ilias.login_saved === true
                                text: qsTr("Vergessen")
                                flat: true
                                onClicked: { dialog.app.iliasForgetLogin(); iliasCard.loginMessage = qsTr("Anmeldung vergessen") }
                            }
                            TextField {
                                id: iliasUser
                                visible: iliasCard.ilias.login_saved !== true
                                Layout.preferredWidth: 140
                                text: iliasCard.ilias.user || ""
                                placeholderText: qsTr("Benutzername")
                                font.pixelSize: Theme.smallFontSize
                                selectByMouse: true
                            }
                            TextField {
                                id: iliasPassword
                                visible: iliasCard.ilias.login_saved !== true
                                Layout.fillWidth: true
                                echoMode: TextInput.Password
                                placeholderText: qsTr("Passwort")
                                font.pixelSize: Theme.smallFontSize
                                onAccepted: iliasCard.rememberLogin()
                            }
                            Button {
                                visible: iliasCard.ilias.login_saved !== true
                                text: qsTr("Merken")
                                enabled: iliasUser.text.trim().length > 0 && iliasPassword.text.length > 0 && !iliasCard.storing
                                onClicked: iliasCard.rememberLogin()
                            }
                        }
                        Text {
                            Layout.fillWidth: true
                            Layout.leftMargin: 108
                            wrapMode: Text.Wrap
                            font.pixelSize: Theme.smallFontSize
                            color: iliasCard.loginFailed ? Theme.error : Theme.textFaint
                            text: iliasCard.loginMessage
                                || (iliasCard.ilias.login_saved === true
                                    ? (iliasCard.ilias.login_host
                                        ? qsTr("Wird nur auf %1 eingesetzt, wenn die Sitzung abgelaufen ist.").arg(iliasCard.ilias.login_host)
                                        : qsTr("Bei der nächsten Anmeldung fragt Atlas einmal, auf welcher Seite es die Daten einsetzen darf."))
                                    : qsTr("Optional: Atlas meldet dich dann selbst an. Das Passwort wird nur verschlüsselt mit systemd-creds gespeichert, sonst gar nicht."))
                        }
                        RowLayout {
                            Layout.fillWidth: true
                            Layout.leftMargin: 30
                            spacing: 8
                            Text { text: qsTr("Vorschlag"); color: Theme.textMuted; font.pixelSize: Theme.smallFontSize; Layout.preferredWidth: 70 }
                            Text {
                                Layout.fillWidth: true
                                text: iliasCard.ilias.download_dir || qsTr("Jeder Download fragt nach dem Ordner")
                                color: Theme.text
                                font.pixelSize: Theme.smallFontSize
                                elide: Text.ElideMiddle
                            }
                            Button { text: qsTr("Ändern …"); flat: true; onClicked: iliasDirChooser.ask("pick", iliasCard.ilias.download_dir || "", "") }
                            Button {
                                text: qsTr("Abmelden")
                                flat: true
                                enabled: !!dialog.actions && !!dialog.actions.ilias
                                onClicked: dialog.actions.ilias.logout()
                            }
                        }
                    }
                }

                SectionLabel { text: qsTr("Kontextleiste"); Layout.topMargin: 10 }
                Card {
                    Layout.fillWidth: true
                    implicitHeight: moduleRows.implicitHeight
                    ColumnLayout {
                        id: moduleRows
                        anchors.left: parent.left
                        anchors.right: parent.right
                        spacing: 0
                        ModuleRow {
                            key: "pomodoro"
                            icon: "focus"
                            title: qsTr("Lernzeit (Pomodoro)")
                            detail: qsTr("Unten in der Kontextleiste: %1 Min. lernen, %2 Min. Pause, nach vier Runden %3 Min.")
                                .arg(dialog.context.pomodoro_work || 25).arg(dialog.context.pomodoro_break || 5).arg(dialog.context.pomodoro_long_break || 15)
                            // Lengths of the phases.
                            Row {
                                visible: dialog.context.pomodoro !== false
                                spacing: 4
                                Repeater {
                                    model: [["pomodoro_work", qsTr("Lernen")], ["pomodoro_break", qsTr("Pause")], ["pomodoro_long_break", qsTr("Lange Pause")]]
                                    SpinBox {
                                        required property var modelData
                                        from: 1
                                        to: 180
                                        editable: true
                                        implicitWidth: 92
                                        value: dialog.context[modelData[0]] || 1
                                        ToolTip.visible: hovered
                                        ToolTip.text: qsTr("%1 (Minuten)").arg(modelData[1])
                                        onValueModified: dialog.app.setContextModule(modelData[0], value)
                                    }
                                }
                            }
                        }
                        ModuleRow {
                            key: "calendar"
                            icon: "calendar"
                            title: qsTr("Demnächst im Kalender")
                            detail: qsTr("Termine, Arbeitsblöcke und Fristen der nächsten sieben Tage")
                        }
                        ModuleRow {
                            key: "tasks"
                            icon: "check-square"
                            title: qsTr("Aufgaben")
                            detail: qsTr("Offene Aufgaben abhaken und neue anlegen")
                            last: true
                        }
                    }
                }

                SectionLabel { text: qsTr("Programme und Daten"); Layout.topMargin: 10 }
                Card {
                    Layout.fillWidth: true
                    implicitHeight: links.implicitHeight
                    ColumnLayout {
                        id: links
                        anchors.left: parent.left
                        anchors.right: parent.right
                        spacing: 0
                        LinkRow {
                            icon: "file-code"
                            title: qsTr("Externer Editor")
                            detail: dialog.editorText
                            onClicked: dialog.run(() => dialog.actions.configureEditor())
                        }
                        LinkRow {
                            icon: "calendar"
                            title: qsTr("Kalender: Nextcloud oder Google")
                            detail: !dialog.calendar || !dialog.calendar.connected ? qsTr("Nicht verbunden – nur der lokale Projektkalender")
                                : (dialog.calendar.google ? qsTr("Google · ") : qsTr("Nextcloud · ")) + (dialog.calendar.syncStatus || qsTr("Verbunden"))
                            onClicked: dialog.run(() => dialog.actions.calendarSync.openSettings())
                        }
                        // Where new events and tasks go.
                        RowLayout {
                            id: defaultCalendarRow
                            readonly property var choices: {
                                if (!dialog.calendar || dialog.calendar.revision < 0) return []
                                try { return JSON.parse(dialog.calendar.calendars()).filter(c => c.writable && c.visible && !c.missing) } catch (e) { return [] }
                            }
                            readonly property var current: choices.find(c => c.isDefault) || null
                            Layout.fillWidth: true
                            Layout.leftMargin: 40
                            Layout.rightMargin: 12
                            Layout.bottomMargin: 6
                            spacing: 8
                            Text { text: qsTr("Neue Termine in"); color: Theme.textMuted; font.pixelSize: Theme.smallFontSize }
                            ComboBox {
                                id: defaultCalendarBox
                                Layout.fillWidth: true
                                model: defaultCalendarRow.choices
                                textRole: "name"
                                valueRole: "id"
                                currentIndex: defaultCalendarRow.current ? defaultCalendarRow.choices.indexOf(defaultCalendarRow.current) : 0
                                // For all projects; a project's own choice is replaced.
                                onActivated: (index) => {
                                    const projectChoice = defaultCalendarRow.choices.some(c => c.projectDefault)
                                    dialog.calendar.setDefaultCalendar(defaultCalendarRow.choices[index].id, false)
                                    if (projectChoice) dialog.calendar.setDefaultCalendar("", true)
                                }
                            }
                        }
                        LinkRow {
                            icon: "grid"
                            title: qsTr("Erweiterungen")
                            detail: dialog.extensionCount === 0 ? qsTr("Keine installiert") : qsTr("%n installiert", "", dialog.extensionCount)
                            onClicked: dialog.run(() => dialog.extensionsRequested())
                        }
                        LinkRow {
                            icon: "play"
                            title: qsTr("Projektaktionen")
                            detail: dialog.app.projectRoot ? qsTr("Nur für %1 (.atlas/actions.yaml)").arg(dialog.app.projectName) : qsTr("Kein Projekt geöffnet")
                            enabled: !!dialog.app.projectRoot
                            onClicked: dialog.run(() => dialog.actions.editActions(false))
                        }
                        LinkRow {
                            icon: "play"
                            title: qsTr("Globale Aktionen")
                            detail: qsTr("Für alle Projekte (~/.config/atlas/actions.yaml)")
                            last: true
                            onClicked: dialog.run(() => dialog.actions.editActions(true))
                        }
                    }
                }
            }
        }

        Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.border }
        RowLayout {
            Layout.fillWidth: true
            Layout.margins: 14
            Layout.leftMargin: 18
            Text {
                Layout.fillWidth: true
                text: qsTr("Atlas %1").arg(dialog.app.version)
                color: Theme.textFaint
                font.pixelSize: Theme.smallFontSize
            }
            Button { text: qsTr("Schließen"); highlighted: true; onClicked: dialog.close() }
        }
    }

    component SectionLabel: Text {
        color: Theme.textMuted
        font.pixelSize: Theme.smallFontSize
        font.weight: Font.DemiBold
        font.capitalization: Font.AllUppercase
        font.letterSpacing: 0.6
    }

    component Card: Rectangle {
        radius: 10
        color: Theme.header
        border.color: Theme.border
    }

    /// An on/off switch.
    component Toggle: Rectangle {
        id: toggle
        property bool checked
        signal toggled()
        implicitWidth: 40
        implicitHeight: 22
        radius: 11
        color: checked ? Theme.accent : Theme.hover
        border.color: checked ? Theme.accent : Theme.border
        Behavior on color { ColorAnimation { duration: 120 } }
        Rectangle {
            width: 16
            height: 16
            radius: 8
            y: 3
            x: toggle.checked ? toggle.width - width - 3 : 3
            color: "white"
            Behavior on x { NumberAnimation { duration: 120 } }
        }
        MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: toggle.toggled() }
    }

    FolderChooser {
        id: iliasDirChooser
        app: dialog.app
        parent: Overlay.overlay
        pickTitle: qsTr("Vorgeschlagener Download-Ordner")
        pickHint: qsTr("Jeder Download fragt nach seinem Ordner; dieser wird zuerst angeboten.")
        pickButton: qsTr("Hier speichern")
        onChosen: (path) => dialog.app.setIliasConfig("download_dir", path)
    }

    /// A module of the context bar with its switch (and extra controls).
    component ModuleRow: Rectangle {
        id: moduleRow
        property string key
        property string icon
        property string title
        property string detail
        property bool last: false
        default property alias extra: extraArea.data
        readonly property bool on: dialog.context[key] !== false
        Layout.fillWidth: true
        implicitHeight: moduleLayout.implicitHeight + 20
        color: "transparent"
        RowLayout {
            id: moduleLayout
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            anchors.leftMargin: 12
            anchors.rightMargin: 12
            spacing: 12
            Icon { Layout.alignment: Qt.AlignTop; name: moduleRow.icon; size: 18; color: Theme.accent }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2
                Text { text: moduleRow.title; color: Theme.textStrong; font.pixelSize: Theme.fontSize }
                Text {
                    Layout.fillWidth: true
                    text: moduleRow.detail
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: Theme.textMuted
                    font.pixelSize: Theme.smallFontSize
                }
                Item {
                    id: extraArea
                    Layout.topMargin: children.length && childrenRect.height ? 4 : 0
                    implicitWidth: childrenRect.width
                    implicitHeight: childrenRect.height
                }
            }
            Toggle {
                Layout.alignment: Qt.AlignVCenter
                checked: moduleRow.on
                onToggled: dialog.app.setContextModule(moduleRow.key, moduleRow.on ? 0 : 1)
            }
        }
        Rectangle {
            visible: !moduleRow.last
            anchors.bottom: parent.bottom
            x: 42
            width: parent.width - 54
            height: 1
            color: Theme.border
        }
    }

    /// A row leading to another dialog: icon, title, current state, chevron.
    component LinkRow: Rectangle {
        id: row
        property string icon
        property string title
        property string detail
        property bool last: false
        signal clicked()
        Layout.fillWidth: true
        implicitHeight: 54
        color: rowHover.hovered && enabled ? Theme.hover : "transparent"
        radius: 10
        opacity: enabled ? 1 : 0.5
        HoverHandler { id: rowHover }
        MouseArea { anchors.fill: parent; enabled: row.enabled; cursorShape: Qt.PointingHandCursor; onClicked: row.clicked() }
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 12
            anchors.rightMargin: 12
            spacing: 12
            Icon { name: row.icon; size: 18; color: Theme.accent }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2
                Text { text: row.title; color: Theme.textStrong; font.pixelSize: Theme.fontSize }
                Text {
                    Layout.fillWidth: true
                    textFormat: Text.PlainText
                    text: row.detail
                    color: Theme.textMuted
                    font.pixelSize: Theme.smallFontSize
                    elide: Text.ElideRight
                }
            }
            Icon { name: "chevron-right"; size: 14; color: Theme.textFaint }
        }
        Rectangle {
            visible: !row.last
            anchors.bottom: parent.bottom
            x: 42
            width: parent.width - 54
            height: 1
            color: Theme.border
        }
    }
}
