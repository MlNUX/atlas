// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Reading the favourites of the dashboard with everything in them.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Condvar, Mutex};

use anyhow::Result;

use super::*;

/// Progress of reading: pages read so far and what is read now.
#[derive(Debug, Clone, PartialEq)]
pub struct Progress {
    pub pages: usize,
    pub text: String,
}

/// What is read: the favourites of the dashboard ("Favoriten") – the
/// courses, groups, sessions, folders and files placed there.
const FAVOURITES: &str = "ilias.php?baseClass=ilDashboardGUI&cmd=jumpToSelectedItems";
const MAX_DEPTH: usize = 10;
/// Pages read at most in one reading (a safety net).
const MAX_PAGES: usize = 2000;
const PARALLEL: usize = 4;

/// Reads the favourites of the dashboard with everything in them (groups,
/// sessions, folders, files), several pages at a time. Nothing else is
/// read: no other dashboard view, no side blocks, no links to other
/// courses inside a course.
pub fn scan(client: &Client, progress: &(dyn Fn(Progress) + Sync)) -> Result<Vec<Node>, IliasError> {
    let url = client.base.join(FAVOURITES).map_err(|e| IliasError::Other(e.to_string()))?;
    let (html, at) = match client.page(url.as_str()) {
        Ok(p) => p,
        Err(IliasError::Login) => {
            trace("Abbruch: nicht angemeldet");
            return Err(IliasError::Login);
        }
        Err(e) => {
            trace(&format!("Favoriten nicht lesbar: {e}"));
            return Err(e);
        }
    };
    let mut courses = objects(&html, &at);
    trace(&format!("  Favoriten: {} (Kopie: {})", summary(&courses), dump(&html, &at)));
    if courses.is_empty() {
        trace(&format!("    keine Favoriten erkannt: {}", explain(&html, &at)));
    }
    trace(&format!(
        "Favoriten: {}",
        if courses.is_empty() {
            "keine".to_owned()
        } else {
            courses.iter().map(|c| format!("{} ({})", c.name, c.key())).collect::<Vec<_>>().join(" · ")
        }
    ));
    progress(Progress { pages: 0, text: format!("{} Favoriten", courses.len()) });

    /// A container to read: key, address, depth, name, and the course it
    /// belongs to.
    struct Task {
        key: String,
        url: String,
        depth: usize,
        name: String,
        course: String,
    }
    struct Work {
        queue: VecDeque<Task>,
        children: HashMap<String, Vec<Node>>,
        errors: HashMap<String, String>,
        visited: HashSet<String>,
        active: usize,
        pages: usize,
        login: bool,
        /// Course key → pages read, files found.
        per_course: HashMap<String, (usize, usize)>,
        /// Containers left out because of MAX_PAGES.
        skipped: usize,
    }
    let work = Mutex::new(Work {
        queue: courses
            .iter()
            .filter(|c| c.is_container())
            .map(|c| Task { key: c.key(), url: c.url.clone(), depth: 0, name: c.name.clone(), course: c.key() })
            .collect(),
        children: HashMap::new(),
        errors: HashMap::new(),
        visited: courses.iter().map(Node::key).collect(),
        active: 0,
        pages: 0,
        login: false,
        per_course: HashMap::new(),
        skipped: 0,
    });
    let wake = Condvar::new();
    std::thread::scope(|s| {
        for _ in 0..PARALLEL {
            s.spawn(|| {
                loop {
                    let task = {
                        let mut w = work.lock().unwrap();
                        loop {
                            if w.login {
                                return;
                            }
                            if let Some(t) = w.queue.pop_front() {
                                w.active += 1;
                                break t;
                            }
                            if w.active == 0 {
                                return;
                            }
                            w = wake.wait(w).unwrap();
                        }
                    };
                    // Read and parse without the lock: only the bookkeeping
                    // below is shared.
                    let result = client.page(&task.url).map(|(html, at)| {
                        let found: Vec<Node> = objects(&html, &at)
                            .into_iter()
                            .filter(|o| o.kind != "crs" && o.key() != task.key)
                            .collect();
                        let empty =
                            if found.is_empty() { format!(" · leer: {}", explain(&html, &at)) } else { String::new() };
                        (found, empty)
                    });
                    let mut log = None;
                    let mut report = None;
                    {
                        let mut w = work.lock().unwrap();
                        w.active -= 1;
                        match result {
                            Ok((found, empty)) => {
                                w.pages += 1;
                                let pages = w.pages;
                                let mut kept = Vec::new();
                                let mut queued = 0;
                                for o in found {
                                    if o.is_container() {
                                        // Each container once (it may be linked twice).
                                        if !w.visited.insert(o.key()) {
                                            continue;
                                        }
                                        if task.depth >= MAX_DEPTH {
                                            // (listed, not read)
                                        } else if w.pages + w.queue.len() + w.active >= MAX_PAGES {
                                            w.skipped += 1;
                                        } else {
                                            queued += 1;
                                            w.queue.push_back(Task {
                                                key: o.key(),
                                                url: o.url.clone(),
                                                depth: task.depth + 1,
                                                name: o.name.clone(),
                                                course: task.course.clone(),
                                            });
                                        }
                                    }
                                    kept.push(o);
                                }
                                let files = kept.iter().filter(|o| o.kind == "file").count();
                                let entry = w.per_course.entry(task.course.clone()).or_default();
                                entry.0 += 1;
                                entry.1 += files;
                                let detail = if !empty.is_empty() {
                                    empty
                                } else if kept.is_empty() {
                                    " · leer (alles schon gelesen)".to_owned()
                                } else {
                                    String::new()
                                };
                                log = Some(format!(
                                    "  {}{} ({}): {}, {queued} zum Lesen{detail}",
                                    "  ".repeat(task.depth),
                                    task.name,
                                    task.key,
                                    summary(&kept)
                                ));
                                w.children.insert(task.key, kept);
                                if pages.is_multiple_of(5) {
                                    report = Some(Progress { pages, text: task.name });
                                }
                            }
                            Err(IliasError::Login) => w.login = true,
                            Err(IliasError::Other(e)) => {
                                log = Some(format!("  {} ({}): Fehler {e}", task.name, task.key));
                                w.errors.insert(task.key, e);
                            }
                        }
                    }
                    wake.notify_all();
                    if let Some(line) = log {
                        trace(&line);
                    }
                    if let Some(p) = report {
                        progress(p);
                    }
                }
            });
        }
    });
    let mut w = work.into_inner().unwrap();
    if w.login {
        trace("Abbruch: Sitzung unterwegs verloren");
        return Err(IliasError::Login);
    }
    for c in courses.iter().filter(|c| c.is_container()) {
        let (pages, files) = w.per_course.get(&c.key()).copied().unwrap_or_default();
        trace(&format!("Favorit {} ({}): {pages} Seiten, {files} Dateien", c.name, c.key()));
    }
    if w.skipped > 0 {
        trace(&format!("Grenze von {MAX_PAGES} Seiten erreicht: {} Ordner nicht gelesen", w.skipped));
    }
    trace(&format!("Fertig: {} Seiten, {} Fehler", w.pages, w.errors.len()));
    fn build(node: &mut Node, w: &mut Work) {
        if !node.is_container() {
            return;
        }
        let key = node.key();
        if let Some(e) = w.errors.remove(&key) {
            node.error = e;
        }
        let mut children = w.children.remove(&key).unwrap_or_default();
        for c in &mut children {
            build(c, w);
        }
        node.children = Some(children);
    }
    for c in &mut courses {
        build(c, &mut w);
    }
    Ok(courses)
}
