// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Commands that run in the background (`terminal: background`, e.g. the
//! Jupyter server): no terminal, the output goes into a log file. Atlas
//! keeps one job per key (the action id), and the jobs end with Atlas –
//! when it quits ([`stop_all`]) and, as a safety net, when it crashes
//! (`PR_SET_PDEATHSIG`).

use anyhow::{Context, Result, bail};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

struct Job {
    name: String,
    cwd: PathBuf,
    log: PathBuf,
    child: Child,
    /// Exit code once ended (`-1`: ended by a signal).
    code: Option<i32>,
}

impl Job {
    fn poll(&mut self) -> bool {
        if self.code.is_none()
            && let Ok(Some(status)) = self.child.try_wait()
        {
            self.code = Some(status.code().unwrap_or(-1));
        }
        self.code.is_none()
    }
}

static JOBS: Mutex<BTreeMap<String, Job>> = Mutex::new(BTreeMap::new());

/// State of one job.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Status {
    pub key: String,
    pub name: String,
    pub cwd: String,
    pub running: bool,
    /// Exit code once ended.
    pub code: Option<i32>,
    /// End of the log (stdout and stderr), at most [`LOG_TAIL`] bytes.
    pub log: String,
}

pub const LOG_TAIL: u64 = 16 * 1024;

fn log_dir() -> PathBuf {
    dirs::cache_dir().unwrap_or_else(std::env::temp_dir).join("atlas").join("background")
}

fn log_name(key: &str) -> String {
    key.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '.' { c } else { '_' }).collect::<String>()
        + ".log"
}

/// Starts `argv` in `cwd` as job `key`; an error if it already runs.
pub fn start(key: &str, name: &str, argv: &[String], cwd: &Path, env: &[(String, String)]) -> Result<()> {
    use std::os::unix::process::CommandExt;
    let mut jobs = JOBS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(job) = jobs.get_mut(key)
        && job.poll()
    {
        bail!("„{}“ läuft schon", job.name);
    }
    let (program, args) = argv.split_first().context("leerer Befehl")?;
    let dir = log_dir();
    std::fs::create_dir_all(&dir).with_context(|| format!("{} kann nicht angelegt werden", dir.display()))?;
    let log = dir.join(log_name(key));
    let out =
        std::fs::File::create(&log).with_context(|| format!("{} kann nicht geschrieben werden", log.display()))?;
    let err = out.try_clone()?;
    let mut cmd = Command::new(program);
    cmd.args(args).current_dir(cwd).envs(env.iter().cloned()).stdin(Stdio::null()).stdout(out).stderr(err);
    // Own process group (stopped as a whole); ends when Atlas ends.
    cmd.process_group(0);
    // SAFETY: prctl is async-signal-safe.
    unsafe {
        cmd.pre_exec(|| {
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
            Ok(())
        });
    }
    let child = cmd.spawn().with_context(|| format!("„{program}“ kann nicht gestartet werden"))?;
    jobs.insert(key.to_owned(), Job { name: name.to_owned(), cwd: cwd.to_path_buf(), log, child, code: None });
    Ok(())
}

fn tail(path: &Path) -> String {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut file) = std::fs::File::open(path) else { return String::new() };
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    let start = len.saturating_sub(LOG_TAIL);
    let mut bytes = Vec::new();
    if file.seek(SeekFrom::Start(start)).is_err() || file.read_to_end(&mut bytes).is_err() {
        return String::new();
    }
    let text = String::from_utf8_lossy(&bytes).into_owned();
    // Cut off a partial first line.
    match (start > 0, text.find('\n')) {
        (true, Some(i)) => text[i + 1..].to_owned(),
        _ => text,
    }
}

fn status_of(key: &str, job: &mut Job) -> Status {
    let running = job.poll();
    Status {
        key: key.to_owned(),
        name: job.name.clone(),
        cwd: job.cwd.to_string_lossy().into_owned(),
        running,
        code: job.code,
        log: tail(&job.log),
    }
}

/// State of job `key` (`None`: never started since Atlas runs).
pub fn status(key: &str) -> Option<Status> {
    let mut jobs = JOBS.lock().unwrap_or_else(|e| e.into_inner());
    jobs.get_mut(key).map(|job| status_of(key, job))
}

/// All jobs (running and ended), without their logs.
pub fn list() -> Vec<Status> {
    let mut jobs = JOBS.lock().unwrap_or_else(|e| e.into_inner());
    jobs.iter_mut().map(|(key, job)| Status { log: String::new(), ..status_of(key, job) }).collect()
}

fn signal(job: &Job, sig: i32) {
    // The group id is the pid of the started program (`process_group(0)`).
    // SAFETY: plain system call.
    unsafe { libc::kill(-(job.child.id() as i32), sig) };
}

/// Asks job `key` to end (SIGTERM to its process group; SIGKILL after five
/// seconds). `false` if it does not run.
pub fn stop(key: &str) -> bool {
    let mut jobs = JOBS.lock().unwrap_or_else(|e| e.into_inner());
    let Some(job) = jobs.get_mut(key) else { return false };
    if !job.poll() {
        return false;
    }
    signal(job, libc::SIGTERM);
    let key = key.to_owned();
    // This job only: the key may get a new job (e.g. a restart) meanwhile.
    let pid = job.child.id();
    let still_running = move |jobs: &mut BTreeMap<String, Job>| {
        jobs.get_mut(&key).filter(|j| j.child.id() == pid).is_some_and(|j| j.poll())
    };
    std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(100));
            if !still_running(&mut JOBS.lock().unwrap_or_else(|e| e.into_inner())) {
                return;
            }
        }
        let mut jobs = JOBS.lock().unwrap_or_else(|e| e.into_inner());
        if still_running(&mut jobs) {
            // SAFETY: plain system call; the group is this job's.
            unsafe { libc::kill(-(pid as i32), libc::SIGKILL) };
        }
    });
    true
}

/// Ends all jobs (when Atlas quits): SIGTERM, up to three seconds to finish,
/// then SIGKILL.
pub fn stop_all() {
    let mut jobs = JOBS.lock().unwrap_or_else(|e| e.into_inner());
    let running: Vec<String> = jobs.iter_mut().filter_map(|(k, j)| j.poll().then(|| k.clone())).collect();
    if running.is_empty() {
        return;
    }
    for key in &running {
        signal(&jobs[key], libc::SIGTERM);
    }
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline && running.iter().any(|k| jobs.get_mut(k).is_some_and(Job::poll)) {
        std::thread::sleep(Duration::from_millis(50));
    }
    for key in &running {
        if let Some(job) = jobs.get_mut(key)
            && job.poll()
        {
            signal(job, libc::SIGKILL);
            let _ = job.child.wait();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wait_until(cond: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !cond() {
            assert!(Instant::now() < deadline, "timeout");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn runs_logs_stops_and_reports_exit_codes() {
        let dir = tempfile::tempdir().unwrap();
        let sh = |script: &str| vec!["sh".to_owned(), "-c".to_owned(), script.to_owned()];
        let env = [("ATLAS_TEST_X".to_owned(), "42".to_owned())];
        start("test/lang", "Lang", &sh("echo start $ATLAS_TEST_X; pwd; sleep 30 & wait"), dir.path(), &env).unwrap();
        assert!(
            start("test/lang", "Lang", &sh("true"), dir.path(), &[]).unwrap_err().to_string().contains("läuft schon")
        );
        wait_until(|| status("test/lang").unwrap().log.contains(&dir.path().to_string_lossy().into_owned()));
        let s = status("test/lang").unwrap();
        assert!(s.running && s.code.is_none() && s.log.starts_with("start 42\n"), "{s:?}");
        assert!(list().iter().any(|j| j.key == "test/lang" && j.running && j.log.is_empty()));
        // Stopping ends the whole group (also the `sleep` child).
        assert!(stop("test/lang"));
        wait_until(|| !status("test/lang").unwrap().running);
        assert_eq!(status("test/lang").unwrap().code, Some(-1));
        assert!(!stop("test/lang"));

        // Errors: exit code and stderr in the log; may start again.
        start("test/fehler", "Fehler", &sh("echo kaputt >&2; exit 3"), dir.path(), &[]).unwrap();
        wait_until(|| !status("test/fehler").unwrap().running);
        let s = status("test/fehler").unwrap();
        assert_eq!((s.code, s.log.as_str()), (Some(3), "kaputt\n"));
        start("test/fehler", "Fehler", &sh("exit 0"), dir.path(), &[]).unwrap();
        wait_until(|| status("test/fehler").unwrap().code == Some(0));
        assert!(status("test/unbekannt").is_none());

        // Stopped and started again right away (a restart): the SIGKILL timer
        // of the old job must not hit the new one.
        start("test/neu", "Neu", &sh("sleep 30"), dir.path(), &[]).unwrap();
        assert!(stop("test/neu"));
        wait_until(|| !status("test/neu").unwrap().running);
        start("test/neu", "Neu", &sh("sleep 30"), dir.path(), &[]).unwrap();
        std::thread::sleep(Duration::from_millis(5600));
        assert!(status("test/neu").unwrap().running, "the new job survives");
        assert!(stop("test/neu"));

        start("test/ende", "Ende", &sh("sleep 30"), dir.path(), &[]).unwrap();
        stop_all();
        assert!(!status("test/ende").unwrap().running);
    }
}
