// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Formulas from PDFs as LaTeX: a picture of the marked area goes to
//! [pix2tex](https://github.com/lukas-blecher/LaTeX-OCR), which runs locally
//! (nothing leaves the machine).
//!
//! pix2tex is a Python program with PyTorch; loading its model takes a few
//! seconds. A worker process therefore loads it once and then answers one
//! JSON line per picture on stdin/stdout. It is started on first use and
//! kept while Atlas runs; if it dies, the next request starts it again.
//!
//! Python: `[pdf] latex_ocr_python` in `config.toml`, else the venv
//! `$XDG_DATA_HOME/atlas/pix2tex` (see docs), else `python3`.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::Duration;

use anyhow::{Context, Result, bail};

/// The worker: loads pix2tex once, then `{"image": path}` → `{"latex": …}`
/// or `{"error": …}`, one line each. Prints from the library go to stderr.
const WORKER: &str = r#"
import sys, json
out = sys.stdout
sys.stdout = sys.stderr
try:
    from PIL import Image
    from pix2tex.cli import LatexOCR
    model = LatexOCR()
except Exception as e:
    out.write(json.dumps({"error": "pix2tex lässt sich nicht laden: %s" % e}) + "\n")
    out.flush()
    sys.exit(1)
out.write(json.dumps({"ready": True}) + "\n")
out.flush()
for line in sys.stdin:
    try:
        request = json.loads(line)
        latex = model(Image.open(request["image"]))
        out.write(json.dumps({"latex": latex}) + "\n")
    except Exception as e:
        out.write(json.dumps({"error": str(e)}) + "\n")
    out.flush()
"#;

/// The first start may download the model (about 100 MB).
const START_TIMEOUT: Duration = Duration::from_secs(300);
const ANSWER_TIMEOUT: Duration = Duration::from_secs(60);

/// The Python interpreter that has pix2tex.
pub fn python() -> String {
    let configured = crate::config::Config::load().pdf.latex_ocr_python;
    if !configured.trim().is_empty() {
        return configured.trim().to_owned();
    }
    let venv = venv_dir().join("bin/python");
    if venv.exists() { venv.to_string_lossy().into_owned() } else { "python3".into() }
}

/// Where the docs suggest installing pix2tex.
pub fn venv_dir() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("atlas/pix2tex")
}

struct Worker {
    command: Vec<String>,
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
}

impl Worker {
    fn start(command: &[String]) -> Result<Worker> {
        let (program, args) = command.split_first().context("kein Befehl")?;
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("„{program}“ lässt sich nicht starten"))?;
        let stdin = child.stdin.take().context("stdin")?;
        let stdout = child.stdout.take().context("stdout")?;
        let (tx, lines) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let mut worker = Worker { command: command.to_vec(), child, stdin, lines };
        let first = worker.answer(START_TIMEOUT).context("pix2tex startet nicht")?;
        if first.get("ready").and_then(|v| v.as_bool()) != Some(true) {
            let _ = worker.child.kill();
            bail!("{}", first.get("error").and_then(|v| v.as_str()).unwrap_or("pix2tex startet nicht"));
        }
        Ok(worker)
    }

    fn answer(&mut self, timeout: Duration) -> Result<serde_json::Value> {
        loop {
            let line = match self.lines.recv_timeout(timeout) {
                Ok(line) => line,
                Err(RecvTimeoutError::Timeout) => bail!("keine Antwort nach {} s", timeout.as_secs()),
                Err(RecvTimeoutError::Disconnected) => bail!("pix2tex hat sich beendet"),
            };
            // Anything that is no JSON object is the library talking.
            if let Ok(value @ serde_json::Value::Object(_)) = serde_json::from_str(&line) {
                return Ok(value);
            }
        }
    }

    fn recognize(&mut self, image: &Path) -> Result<String> {
        let request = serde_json::json!({ "image": image });
        writeln!(self.stdin, "{request}").context("pix2tex nimmt nichts mehr an")?;
        self.stdin.flush()?;
        let reply = self.answer(ANSWER_TIMEOUT)?;
        if let Some(latex) = reply.get("latex").and_then(|v| v.as_str()) {
            return Ok(latex.trim().to_owned());
        }
        bail!("{}", reply.get("error").and_then(|v| v.as_str()).unwrap_or("unbekannte Antwort"))
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

static WORKER_SLOT: Mutex<Option<Worker>> = Mutex::new(None);

/// LaTeX of the formula in the picture `image`, with the worker of
/// `command` (`[program, args…]`; [`default_command`] normally).
pub fn recognize_with(command: &[String], image: &Path) -> Result<String> {
    let mut slot = WORKER_SLOT.lock().unwrap_or_else(|e| e.into_inner());
    if slot.as_ref().is_some_and(|w| w.command != command) {
        *slot = None;
    }
    if slot.is_none() {
        *slot = Some(Worker::start(command)?);
    }
    let result = slot.as_mut().map(|w| w.recognize(image)).unwrap_or_else(|| bail!("kein pix2tex"));
    // A broken worker is started anew next time.
    if result.is_err() && slot.as_mut().is_some_and(|w| w.child.try_wait().ok().flatten().is_some()) {
        *slot = None;
    }
    result
}

/// Whether a worker is running (the model is loaded).
pub fn is_running() -> bool {
    WORKER_SLOT.lock().map(|s| s.is_some()).unwrap_or(false)
}

/// `python -I -c <worker>` with the configured Python.
pub fn default_command() -> Vec<String> {
    vec![python(), "-I".into(), "-c".into(), WORKER.into()]
}

/// LaTeX of the formula in `image` with pix2tex.
pub fn recognize(image: &Path) -> Result<String> {
    recognize_with(&default_command(), image)
}

/// Light clean-up of pix2tex' output: no spaces before `_`, `^`, braces
/// and after `\command` where they change nothing.
pub fn tidy(latex: &str) -> String {
    let mut out = String::with_capacity(latex.len());
    let chars: Vec<char> = latex.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c == ' ' {
            let prev = if i > 0 { chars[i - 1] } else { ' ' };
            let next = chars.get(i + 1).copied().unwrap_or(' ');
            let drop = matches!(next, '_' | '^' | '{' | '}' | ')' | ']' | ',')
                || matches!(prev, '{' | '(' | '[' | '_' | '^')
                || (prev == ' ');
            if drop {
                continue;
            }
        }
        out.push(c);
    }
    out.trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Stands in for the Python worker (no PyTorch in tests).
    fn fake_worker(dir: &Path, name: &str, script: &str) -> Vec<String> {
        let path = dir.join(name);
        std::fs::write(&path, script).unwrap();
        vec!["sh".into(), path.to_string_lossy().into_owned()]
    }

    #[test]
    fn worker_answers_and_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        let count = dir.path().join("starts");
        let command = fake_worker(
            dir.path(),
            "worker.sh",
            &format!(
                "echo start >> '{}'\n echo 'Lade Modell …'\n echo '{{\"ready\": true}}'\n\
                 while read line; do echo '{{\"latex\": \"\\\\frac {{a}} {{b}} ^ {{2}}\"}}'; done\n",
                count.display()
            ),
        );
        let image = dir.path().join("x.png");
        assert_eq!(recognize_with(&command, &image).unwrap(), "\\frac {a} {b} ^ {2}");
        assert_eq!(recognize_with(&command, &image).unwrap(), "\\frac {a} {b} ^ {2}");
        assert_eq!(std::fs::read_to_string(&count).unwrap().lines().count(), 1, "the model is loaded once");

        // pix2tex missing: a clear error, then a new start next time.
        let broken = fake_worker(
            dir.path(),
            "broken.sh",
            "echo '{\"error\": \"pix2tex lässt sich nicht laden: No module named pix2tex\"}'\nexit 1\n",
        );
        let err = recognize_with(&broken, &image).unwrap_err().to_string();
        assert!(err.contains("No module named pix2tex"), "{err}");
        assert!(recognize_with(&["/gibt/es/nicht".to_owned()], &image).is_err());
    }

    #[test]
    fn output_is_tidied() {
        assert_eq!(tidy("\\frac {a} {b} ^ {2}"), "\\frac{a}{b}^{2}");
        assert_eq!(tidy("x _ {i} + y"), "x_{i} + y");
        assert_eq!(tidy("\\sum _ {k = 1} ^ {n} k"), "\\sum_{k = 1}^{n} k");
    }
}
