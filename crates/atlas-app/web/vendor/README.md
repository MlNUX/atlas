<!--
SPDX-FileCopyrightText: 2026 Minux
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Gebündelte Web-Bibliotheken

Für die Markdown-Vorschau (QtWebEngine) werden folgende Bibliotheken
unverändert aus den npm-Paketen übernommen und in die Anwendung eingebettet,
damit die Vorschau ohne Netzzugriff funktioniert:

| Bibliothek | Version | Lizenz | Dateien |
|---|---|---|---|
| KaTeX | 0.19.0 | MIT | `katex/katex.min.js`, `katex.min.css`, `auto-render.min.js`, `fonts/*.woff2` |
| Mermaid | 12.2.0 | MIT | `mermaid/mermaid.min.js` |
| highlight.js | 11.12.0 (`@highlightjs/cdn-assets`) | BSD-3-Clause | `highlight/highlight.min.js`, `github-dark.min.css` |

Die jeweiligen Lizenztexte liegen als `LICENSE` in den Unterordnern. Alle drei
Lizenzen sind mit GPL-3.0-or-later vereinbar.
