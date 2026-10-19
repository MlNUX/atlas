// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Command line options.

use std::path::PathBuf;
use std::sync::OnceLock;

pub const USAGE: &str = "\
Usage: atlas [OPTIONS] [PROJECT]

Opens PROJECT (a folder) or the last used project.

Options:
  --open FILE[:LINE]   Open FILE (at LINE) in a new pane (repeatable; relative
                       to PROJECT); `--open calendar:` shows the calendar
  --no-session         Neither restore nor remember the last project/session
  --search PATTERN     Start a project search for PATTERN
  --screenshot FILE    Save a PNG of the window after startup (for tests)
  --screenshot-delay MS  Delay before the screenshot (default 2500)
  --screenshot-dialog NAME  Open a dialog for the screenshot (sessions,
                       calendar-settings, extensions, properties, entry,
                       editor, palette, menu, context-menu, conflicts,
                       send-to-note, rename, confirm, open-folder, new-session,
                       quick-look)
  --quit-after MS      Quit MS milliseconds after startup (for tests)
  --self-test          Run the built-in smoke test against PROJECT and exit
                       with its result (0 = passed; for tests)
  -h, --help           Show this help
  -V, --version        Show the version
";

#[derive(Debug, Default)]
pub struct Cli {
    pub project: Option<PathBuf>,
    pub open: Vec<PathBuf>,
    /// Line per entry of `open` (0 = none).
    pub open_lines: Vec<u32>,
    pub no_session: bool,
    pub screenshot: Option<PathBuf>,
    pub screenshot_delay_ms: Option<u32>,
    /// Dialog shown before the screenshot (design checks).
    pub screenshot_dialog: Option<String>,
    pub search: Option<String>,
    pub quit_after_ms: Option<u32>,
    pub self_test: bool,
}

static CLI: OnceLock<Cli> = OnceLock::new();

impl Cli {
    /// Parses `std::env::args`, exiting on `--help`, `--version` or errors.
    pub fn init() -> &'static Cli {
        CLI.get_or_init(|| match Self::parse(std::env::args().skip(1)) {
            Ok(cli) => cli,
            Err(msg) => {
                if !msg.is_empty() {
                    eprintln!("atlas: {msg}\n");
                }
                eprint!("{USAGE}");
                std::process::exit(if msg.is_empty() { 0 } else { 2 });
            }
        })
    }

    pub fn get() -> &'static Cli {
        CLI.get_or_init(Cli::default)
    }

    fn parse(args: impl Iterator<Item = String>) -> Result<Cli, String> {
        let mut cli = Cli::default();
        let mut args = args.peekable();
        while let Some(arg) = args.next() {
            let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
            match arg.as_str() {
                "-h" | "--help" => return Err(String::new()),
                "-V" | "--version" => {
                    println!("atlas {}", env!("CARGO_PKG_VERSION"));
                    std::process::exit(0);
                }
                "--open" => {
                    let (path, line) = split_line(&value("--open")?);
                    cli.open.push(path);
                    cli.open_lines.push(line);
                }
                "--no-session" => cli.no_session = true,
                "--self-test" => cli.self_test = true,
                "--screenshot" => cli.screenshot = Some(value("--screenshot")?.into()),
                "--screenshot-delay" => {
                    let v = value("--screenshot-delay")?;
                    cli.screenshot_delay_ms = Some(v.parse().map_err(|_| format!("invalid --screenshot-delay: {v}"))?);
                }
                "--screenshot-dialog" => cli.screenshot_dialog = Some(value("--screenshot-dialog")?),
                "--search" => cli.search = Some(value("--search")?),
                "--quit-after" => {
                    let v = value("--quit-after")?;
                    cli.quit_after_ms = Some(v.parse().map_err(|_| format!("invalid --quit-after: {v}"))?);
                }
                // Qt's own options (e.g. `-platform wayland`) are left to Qt.
                "-platform" | "-platformtheme" | "-plugin" | "-style" | "-qwindowgeometry" | "-qwindowtitle"
                | "-qwindowicon" => {
                    args.next();
                }
                s if s.starts_with('-') && !s.starts_with("--") => {}
                s if s.starts_with("--") => return Err(format!("unknown option {s}")),
                s => {
                    if cli.project.is_some() {
                        return Err(format!("unexpected argument {s}"));
                    }
                    cli.project = Some(s.into());
                }
            }
        }
        if let Some(project) = &cli.project {
            cli.open = cli
                .open
                .iter()
                .map(|f| if f.is_relative() && f.as_os_str() != "calendar:" { project.join(f) } else { f.clone() })
                .map(|f| f.canonicalize().unwrap_or(f))
                .collect();
        }
        Ok(cli)
    }
}

/// `src/main.rs:42` → (`src/main.rs`, 42), unless a file with the full name
/// exists.
fn split_line(arg: &str) -> (PathBuf, u32) {
    if let Some((path, line)) = arg.rsplit_once(':')
        && let Ok(line) = line.parse::<u32>()
        && !std::path::Path::new(arg).exists()
    {
        return (PathBuf::from(path), line);
    }
    (PathBuf::from(arg), 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_file_and_line() {
        let cli = Cli::parse(["--open", "a.rs:12", "--open", "b.pdf"].into_iter().map(String::from)).unwrap();
        assert_eq!(cli.open, [PathBuf::from("a.rs"), PathBuf::from("b.pdf")]);
        assert_eq!(cli.open_lines, [12, 0]);
    }
}
