// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

@preconcurrency import SwiftTerm
import SwiftUI

/// Terminal spike (P0.5): the same demo script in both variants side by side.
///   A: SwiftTerm (own PTY and emulator, Swift)
///   B: atlas-terminal (alacritty_terminal in Rust) + CoreText renderer
@main
struct TerminalSpikeApp: App {
    var body: some Scene {
        WindowGroup("Atlas Terminal-Spike") {
            SpikeView().frame(minWidth: 1500, minHeight: 900)
        }
    }
}

enum Demo {
    /// `bash demo.sh` from the bundle, then an interactive shell.
    static var command: [String] {
        let script = Bundle.main.path(forResource: "demo", ofType: "sh") ?? "/dev/null"
        return ["/bin/bash", "-c", "bash '\(script)'; exec /bin/zsh -i"]
    }
}

struct SpikeView: View {
    var body: some View {
        HStack(spacing: 1) {
            pane("A · SwiftTerm") { SwiftTermPane(command: Demo.command) }
            pane("B · atlas-terminal (alacritty) + CoreText") { CoreTerminalPane(command: Demo.command) }
        }
        .background(Color.secondary.opacity(0.3))
    }

    private func pane(_ title: String, @ViewBuilder content: () -> some View) -> some View {
        VStack(spacing: 0) {
            Text(title).font(.headline).frame(maxWidth: .infinity).padding(6).background(.bar)
            content()
        }
    }
}

/// Variant A: SwiftTerm's `LocalProcessTerminalView`.
struct SwiftTermPane: NSViewRepresentable {
    let command: [String]

    func makeNSView(context: Context) -> LocalProcessTerminalView {
        let view = LocalProcessTerminalView(frame: CGRect(x: 0, y: 0, width: 700, height: 800))
        view.font = .monospacedSystemFont(ofSize: 13, weight: .regular)
        view.getTerminal().setCursorStyle(.steadyBlock)
        var env = Terminal.getEnvironmentVariables(termName: "xterm-256color")
        env.append("COLORTERM=truecolor")
        view.startProcess(executable: command[0], args: Array(command.dropFirst()), environment: env)
        return view
    }

    func updateNSView(_ view: LocalProcessTerminalView, context: Context) {}
}
