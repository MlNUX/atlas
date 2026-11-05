// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import SwiftUI

extension FocusedValues {
    /// The PDF of the key window's active tab.
    @Entry var pdf: PDFModel?
}

/// Menu „PDF“ (P3.2). Shortcuts: docs/shortcuts.md, Mac column (E10).
struct PDFCommands: Commands {
    @FocusedValue(\.pdf) private var pdf

    var body: some Commands {
        CommandMenu("PDF") {
            Button("Auswahl markieren") { pdf?.mark(.highlight) }
                .keyboardShortcut("h", modifiers: [.command, .shift])
                .disabled(pdf == nil)
            Button("Auswahl unterstreichen") { pdf?.mark(.underline) }
                .keyboardShortcut("u", modifiers: [.command, .shift])
                .disabled(pdf == nil)
            Button("Auswahl durchstreichen") { pdf?.mark(.strikeout) }
                .keyboardShortcut("x", modifiers: [.command, .shift])
                .disabled(pdf == nil)
            Divider()
            Button("Anmerkung löschen") { pdf?.removeSelectedAnnotation() }
                .disabled(pdf?.selectedAnnotation == nil)
            Divider()
            Button("Miniaturen ein/aus") { pdf?.sidebarVisible.toggle() }
                .keyboardShortcut("t", modifiers: [.command, .option])
                .disabled(pdf == nil)
        }
    }
}
