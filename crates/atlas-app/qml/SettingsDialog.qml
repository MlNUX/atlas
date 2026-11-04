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
    /// „Anpassen …“ of a profile of the context bar.
    signal customizeContext(string profile)

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
    /// Extensions with settings (shown with their form).
    property var configurable: []
    /// Extensions that come with Atlas.
    property var bundled: []
    function reloadExtensions() {
        let list = []
        try { list = JSON.parse(dialog.app.extensions()) } catch (e) {}
        try { bundled = JSON.parse(dialog.app.bundledExtensions()) } catch (e) { bundled = [] }
        extensionCount = list.length
        configurable = list.filter(e => e.settings.length > 0 && e.status !== "broken")
    }
    onAboutToShow: {
        const command = dialog.app.editorCommand()
        editorText = command ? command : qsTr("Standard: $VISUAL, $EDITOR oder das Programm des Systems")
        reloadExtensions()
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
    readonly property var contextExtensions: { dialog.app.contextModules; dialog.app.untrustedExtensions
        try { return JSON.parse(dialog.app.contextExtensions()).filter(e => e.entries.length > 0) } catch (e) { return [] } }

    contentItem: ColumnLayout {
        spacing: 0

        // ---- header
        DialogHeader {
            icon: "settings"
            title: qsTr("Einstellungen")
            subtitle: qsTr("Gespeichert in ~/.config/atlas/config.toml")
            IconButton { iconName: "close"; tip: qsTr("Schließen (Esc)"); onClicked: dialog.close() }
        }

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

                SectionLabel { text: qsTr("Darstellung") }
                Card {
                    Layout.fillWidth: true
                    implicitHeight: themeRow.implicitHeight + 24
                    RowLayout {
                        id: themeRow
                        anchors.fill: parent
                        anchors.margins: 12
                        spacing: 12
                        Icon { Layout.alignment: Qt.AlignTop; name: Theme.light ? "sun" : "moon"; size: 18; color: Theme.accent }
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 3
                            Text { text: qsTr("Farbschema"); color: Theme.textStrong; font.pixelSize: Theme.fontSize }
                            Text {
                                Layout.fillWidth: true
                                wrapMode: Text.Wrap
                                color: Theme.textMuted
                                font.pixelSize: Theme.smallFontSize
                                text: qsTr("Dunkel oder hell für alle Fenster, Vorschau und Terminal; „System“ folgt der Einstellung des Desktops. Der Dunkelmodus für PDF-Seiten bleibt davon getrennt.")
                            }
                        }
                        Segmented {
                            Layout.alignment: Qt.AlignVCenter
                            current: dialog.app.theme
                            choices: [
                                { value: "dark", text: qsTr("Dunkel") },
                                { value: "light", text: qsTr("Hell") },
                                { value: "system", text: qsTr("System") },
                            ]
                            onChosen: (value) => dialog.app.theme = value
                        }
                    }
                }

                SectionLabel { text: qsTr("Seitenleiste"); Layout.topMargin: 10 }
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
                                readonly property string languages: dialog.visible ? Spelling.languages() : ""
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
                    readonly property var ilias: { try { return JSON.parse(IliasBackend.config) } catch (e) { return {} } }
                    property string loginMessage: ""
                    property bool loginFailed: false
                    property bool storing: false
                    function rememberLogin() {
                        if (!iliasUser.text.trim() || !iliasPassword.text || storing) return
                        storing = true
                        loginMessage = qsTr("Verschlüssele …")
                        loginFailed = false
                        IliasBackend.rememberLogin(iliasUser.text.trim(), iliasPassword.text)
                        iliasPassword.text = ""
                    }
                    Connections {
                        target: IliasBackend
                        function onLoginStored(ok, message) {
                            iliasCard.storing = false
                            iliasCard.loginFailed = !ok
                            iliasCard.loginMessage = message
                        }
                    }
                    function applyAddress() {
                        if (!iliasAddress.valid || !iliasAddress.changed) return
                        IliasBackend.setConfig("base_url", iliasAddress.address)
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
                                onToggled: IliasBackend.setConfig("show_in_places", checked ? "false" : "true")
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
                                onClicked: { IliasBackend.forgetLogin(); iliasCard.loginMessage = qsTr("Anmeldung vergessen") }
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

                SectionLabel { text: qsTr("Kontextleiste · Profile"); Layout.topMargin: 10 }
                Card {
                    Layout.fillWidth: true
                    implicitHeight: profileRows.implicitHeight
                    ColumnLayout {
                        id: profileRows
                        anchors.left: parent.left
                        anchors.right: parent.right
                        spacing: 0
                        Repeater {
                            model: ContextProfiles.names
                            Rectangle {
                                id: profileRow
                                required property string modelData
                                required property int index
                                readonly property bool set: ContextProfiles.isSet(dialog.context, modelData)
                                Layout.fillWidth: true
                                implicitHeight: profileColumn.implicitHeight + 20
                                color: "transparent"
                                RowLayout {
                                    anchors.fill: parent
                                    anchors.leftMargin: 12
                                    anchors.rightMargin: 12
                                    spacing: 12
                                    Icon {
                                        Layout.alignment: Qt.AlignVCenter
                                        name: ({ default: "grid", pdf: "file-pdf", markdown: "file-text", code: "file-code", calendar: "calendar" })[profileRow.modelData]
                                        size: 18
                                        color: profileRow.set ? Theme.accent : Theme.textFaint
                                    }
                                    ColumnLayout {
                                        id: profileColumn
                                        Layout.fillWidth: true
                                        spacing: 2
                                        Text {
                                            text: ContextProfiles.title(profileRow.modelData) + (profileRow.modelData === "default" ? " · " + qsTr("Standard") : "")
                                            color: Theme.textStrong
                                            font.pixelSize: Theme.fontSize
                                        }
                                        Text {
                                            Layout.fillWidth: true
                                            text: profileRow.set || profileRow.modelData === "default"
                                                ? ContextProfiles.summary(dialog.context, profileRow.modelData, dialog.contextExtensions)
                                                : qsTr("Nicht eingestellt – wie Allgemein")
                                            textFormat: Text.PlainText
                                            wrapMode: Text.Wrap
                                            color: Theme.textMuted
                                            font.pixelSize: Theme.smallFontSize
                                        }
                                    }
                                    Button {
                                        Layout.alignment: Qt.AlignVCenter
                                        flat: true
                                        text: profileRow.set ? qsTr("Anpassen …") : qsTr("Einstellen …")
                                        onClicked: dialog.customizeContext(profileRow.modelData)
                                    }
                                }
                                Rectangle {
                                    visible: profileRow.index < ContextProfiles.names.length - 1
                                    anchors.bottom: parent.bottom
                                    x: 42
                                    width: parent.width - 54
                                    height: 1
                                    color: Theme.border
                                }
                            }
                        }
                    }
                }

                SectionLabel { text: qsTr("Kontextleiste · Verhalten"); Layout.topMargin: 10 }
                Card {
                    Layout.fillWidth: true
                    implicitHeight: behaviourRows.implicitHeight
                    ColumnLayout {
                        id: behaviourRows
                        anchors.left: parent.left
                        anchors.right: parent.right
                        spacing: 0
                        OptionRow {
                            icon: "collapse"
                            title: qsTr("Leere Bausteine")
                            detail: qsTr("z. B. „Anmerkungen“ bei einer PDF ohne Anmerkungen")
                            Segmented {
                                current: dialog.context.empty || "collapse"
                                choices: [{ value: "show", text: qsTr("Zeigen") }, { value: "collapse", text: qsTr("Einklappen") },
                                          { value: "hide", text: qsTr("Ausblenden") }]
                                onChosen: (value) => dialog.app.setContextOption("empty", value)
                            }
                        }
                        OptionRow {
                            icon: "list"
                            title: qsTr("Reiterbeschriftung")
                            detail: qsTr("Automatisch: Symbole, wenn die Leiste zu schmal ist")
                            Segmented {
                                current: dialog.context.tab_labels || "auto"
                                choices: [{ value: "auto", text: qsTr("Automatisch") }, { value: "text", text: qsTr("Text") },
                                          { value: "icons", text: qsTr("Symbole") }]
                                onChosen: (value) => dialog.app.setContextOption("tab_labels", value)
                            }
                        }
                        OptionRow {
                            icon: "documents"
                            title: qsTr("Profile nach Dateityp")
                            detail: qsTr("Aus: überall gilt „Allgemein“")
                            Toggle {
                                checked: dialog.context.by_kind !== false
                                onToggled: dialog.app.setContextOption("by_kind", checked ? "false" : "true")
                            }
                        }
                        OptionRow {
                            icon: "focus"
                            title: qsTr("Lernzeit")
                            detail: qsTr("Minuten lernen, Pause, lange Pause nach vier Runden")
                            last: true
                            Row {
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
                    }
                }
                Button {
                    flat: true
                    text: qsTr("Alle Profile zurücksetzen")
                    enabled: Object.keys(dialog.context.profiles || {}).length > 0
                    onClicked: { for (const name of Object.keys(dialog.context.profiles || {})) dialog.app.setContextProfile(name, "") }
                }

                // Settings the extensions declare (API 2).
                SectionLabel { text: qsTr("Erweiterungen"); Layout.topMargin: 10 }
                Repeater {
                    model: dialog.configurable
                    Card {
                        id: extCard
                        required property var modelData
                        Layout.fillWidth: true
                        implicitHeight: extColumn.implicitHeight + 24
                        ColumnLayout {
                            id: extColumn
                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.top: parent.top
                            anchors.margins: 12
                            spacing: 8
                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 12
                                Icon { name: "grid"; size: 18; color: Theme.accent }
                                Text {
                                    Layout.fillWidth: true
                                    text: extCard.modelData.title
                                    color: Theme.textStrong
                                    font.pixelSize: Theme.fontSize
                                    elide: Text.ElideRight
                                }
                                Text {
                                    text: extCard.modelData.status === "active" ? qsTr("Aktiv")
                                        : extCard.modelData.status === "disabled" ? qsTr("Ausgeschaltet") : qsTr("Nicht freigegeben")
                                    color: extCard.modelData.status === "active" ? Theme.ok : Theme.textFaint
                                    font.pixelSize: Theme.smallFontSize
                                }
                                // Second click uninstalls (folder to the trash).
                                Button {
                                    id: uninstallButton
                                    property bool armed: false
                                    flat: true
                                    text: armed ? qsTr("Wirklich deinstallieren?") : qsTr("Deinstallieren")
                                    onClicked: {
                                        if (!armed) { armed = true; disarm.restart(); return }
                                        const error = dialog.app.uninstallExtension(extCard.modelData.dir)
                                        if (dialog.actions) dialog.actions.showMessage(error || qsTr("„%1“ deinstalliert – der Ordner liegt im Papierkorb").arg(extCard.modelData.title))
                                        dialog.reloadExtensions()
                                    }
                                    Timer { id: disarm; interval: 4000; onTriggered: uninstallButton.armed = false }
                                }
                            }
                            ExtensionSettings {
                                Layout.fillWidth: true
                                app: dialog.app
                                actions: dialog.actions
                                ext: extCard.modelData
                                onChanged: dialog.reloadExtensions()
                            }
                        }
                    }
                }
                // Extensions that come with Atlas, installed with one click.
                Card {
                    Layout.fillWidth: true
                    implicitHeight: bundledColumn.implicitHeight + 24
                    ColumnLayout {
                        id: bundledColumn
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.margins: 12
                        spacing: 8
                        Text {
                            Layout.fillWidth: true
                            wrapMode: Text.Wrap
                            color: Theme.textMuted
                            font.pixelSize: Theme.smallFontSize
                            text: dialog.configurable.length === 0
                                ? qsTr("Keine Erweiterung mit Einstellungen installiert. Mitgeliefert:")
                                : qsTr("Mitgeliefert:")
                        }
                        Repeater {
                            model: dialog.bundled
                            RowLayout {
                                id: bundledRow
                                required property var modelData
                                Layout.fillWidth: true
                                spacing: 12
                                Icon { name: "grid"; size: 18; color: Theme.accent }
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    spacing: 1
                                    Text { text: bundledRow.modelData.title; color: Theme.textStrong; font.pixelSize: Theme.fontSize }
                                    Text {
                                        Layout.fillWidth: true
                                        text: bundledRow.modelData.description
                                        color: Theme.textMuted
                                        font.pixelSize: Theme.smallFontSize
                                        wrapMode: Text.Wrap
                                    }
                                }
                                Button {
                                    text: bundledRow.modelData.installed ? qsTr("Aktualisieren") : qsTr("Installieren")
                                    flat: bundledRow.modelData.installed
                                    onClicked: {
                                        const error = dialog.app.installBundled(bundledRow.modelData.name)
                                        if (dialog.actions) dialog.actions.showMessage(error || qsTr("„%1“ installiert").arg(bundledRow.modelData.title))
                                        dialog.reloadExtensions()
                                    }
                                }
                            }
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

    /// A choice of a few values side by side.
    component Segmented: Rectangle {
        id: segmented
        property var choices: []
        property string current
        signal chosen(string value)
        implicitWidth: segmentRow.implicitWidth + 6
        implicitHeight: 28
        radius: 8
        color: Theme.raised
        border.color: Theme.border
        Row {
            id: segmentRow
            anchors.centerIn: parent
            spacing: 2
            Repeater {
                model: segmented.choices
                Rectangle {
                    id: segment
                    required property var modelData
                    readonly property bool selected: segmented.current === modelData.value
                    width: segmentText.implicitWidth + 20
                    height: 22
                    radius: 6
                    color: selected ? Theme.accent : segmentHover.hovered ? Theme.hover : "transparent"
                    HoverHandler { id: segmentHover }
                    Text {
                        id: segmentText
                        anchors.centerIn: parent
                        text: segment.modelData.text
                        color: segment.selected ? "white" : Theme.text
                        font.pixelSize: Theme.smallFontSize
                    }
                    MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: segmented.chosen(segment.modelData.value) }
                }
            }
        }
    }

    FolderChooser {
        id: iliasDirChooser
        app: dialog.app
        parent: Overlay.overlay
        pickTitle: qsTr("Vorgeschlagener Download-Ordner")
        pickHint: qsTr("Jeder Download fragt nach seinem Ordner; dieser wird zuerst angeboten.")
        pickButton: qsTr("Hier speichern")
        onChosen: (path) => IliasBackend.setConfig("download_dir", path)
    }

    /// A setting: icon, title, a line of explanation, the control on the right.
    component OptionRow: Rectangle {
        id: optionRow
        property string icon
        property string title
        property string detail
        property bool last: false
        default property alias control: controlArea.data
        Layout.fillWidth: true
        implicitHeight: Math.max(optionText.implicitHeight, controlArea.implicitHeight) + 22
        color: "transparent"
        RowLayout {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            anchors.leftMargin: 12
            anchors.rightMargin: 12
            spacing: 12
            Icon { Layout.alignment: Qt.AlignVCenter; name: optionRow.icon; size: 18; color: Theme.accent }
            ColumnLayout {
                id: optionText
                Layout.fillWidth: true
                spacing: 2
                Text { text: optionRow.title; color: Theme.textStrong; font.pixelSize: Theme.fontSize }
                Text { Layout.fillWidth: true; text: optionRow.detail; wrapMode: Text.Wrap; color: Theme.textMuted; font.pixelSize: Theme.smallFontSize }
            }
            Item {
                id: controlArea
                Layout.alignment: Qt.AlignVCenter
                implicitWidth: childrenRect.width
                implicitHeight: childrenRect.height
            }
        }
        Rectangle {
            visible: !optionRow.last
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
