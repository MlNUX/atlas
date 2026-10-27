// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Folder trees as text, like `tree` prints them:
//!
//! ```text
//! /datapool
//! ├── immich
//! │   ├── data
//! │   └── database
//! └── media
//! ```
//!
//! Drawn from an indented list (a ```` ```tree ```` block in a note) or
//! from a real folder ("Ordnerbaum einfügen").

use std::path::Path;

struct Node {
    name: String,
    children: Vec<Node>,
}

fn draw_children(children: &[Node], prefix: &str, out: &mut String) {
    for (i, child) in children.iter().enumerate() {
        let last = i + 1 == children.len();
        out.push_str(prefix);
        out.push_str(if last { "└── " } else { "├── " });
        out.push_str(&child.name);
        out.push('\n');
        let deeper = format!("{prefix}{}", if last { "    " } else { "│   " });
        draw_children(&child.children, &deeper, out);
    }
}

fn draw_roots(roots: &[Node]) -> String {
    let mut out = String::new();
    for root in roots {
        out.push_str(&root.name);
        out.push('\n');
        draw_children(&root.children, "", &mut out);
    }
    out
}

/// Draws an indented list (spaces or tabs, optional `-`/`*` bullets) as a
/// tree. Text that already has tree lines stays as it is.
pub fn draw(text: &str) -> String {
    if text.contains(['├', '└', '│']) {
        return text.to_owned();
    }
    let mut roots: Vec<Node> = Vec::new();
    // Indent and index path of the lines the next one may belong to.
    let mut stack: Vec<(usize, Vec<usize>)> = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let indent: usize = line.chars().take_while(|c| c.is_whitespace()).map(|c| if c == '\t' { 4 } else { 1 }).sum();
        let name = line.trim();
        let name = name
            .strip_prefix("- ")
            .or_else(|| name.strip_prefix("* "))
            .or_else(|| name.strip_prefix("+ "))
            .unwrap_or(name);
        let node = Node { name: name.trim().to_owned(), children: Vec::new() };
        while stack.last().is_some_and(|(i, _)| *i >= indent) {
            stack.pop();
        }
        let path = match stack.last() {
            None => {
                roots.push(node);
                vec![roots.len() - 1]
            }
            Some((_, parent)) => {
                let mut list = &mut roots;
                for &i in parent {
                    list = &mut list[i].children;
                }
                list.push(node);
                let mut p = parent.clone();
                p.push(list.len() - 1);
                p
            }
        };
        stack.push((indent, path));
    }
    draw_roots(&roots)
}

/// The tree of folder `dir`, at most `depth` levels and `limit` entries
/// (then `…`); hidden files and build folders are left out, folders first.
pub fn of_folder(dir: &Path, depth: usize, limit: usize) -> String {
    fn walk(dir: &Path, depth: usize, budget: &mut usize) -> Vec<Node> {
        let Ok(entries) = crate::fs_tree::list_dir(dir, false) else { return Vec::new() };
        let mut out = Vec::new();
        for e in entries {
            if matches!(e.name.as_str(), "node_modules" | "target" | "__pycache__") {
                continue;
            }
            if *budget == 0 {
                out.push(Node { name: "…".into(), children: Vec::new() });
                break;
            }
            *budget -= 1;
            let children = if e.is_dir && depth > 1 { walk(&e.path, depth - 1, budget) } else { Vec::new() };
            out.push(Node { name: e.name.clone(), children });
        }
        out
    }
    let mut budget = limit;
    let name = dir.to_string_lossy().into_owned();
    draw_roots(&[Node { name, children: walk(dir, depth, &mut budget) }])
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPECTED: &str = "/datapool\n├── immich\n│   ├── data\n│   └── database\n├── backups\n└── media\n    ├── movies\n    └── series\n";

    #[test]
    fn draws_an_indented_list() {
        let list = "/datapool\n  immich\n    data\n    database\n  backups\n  media\n    movies\n    series\n";
        assert_eq!(draw(list), EXPECTED);
        let bullets =
            "- /datapool\n  - immich\n    - data\n    - database\n  - backups\n  - media\n    - movies\n    - series";
        assert_eq!(draw(bullets), EXPECTED);
        assert_eq!(draw(EXPECTED), EXPECTED, "drawn trees stay");
        assert_eq!(draw("a\nb\n  c\n"), "a\nb\n└── c\n");
    }

    #[test]
    fn draws_a_real_folder() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("datapool");
        for d in ["immich/data", "immich/database", "media/movies", ".versteckt", "target/x"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        std::fs::write(root.join("liesmich.txt"), "").unwrap();
        let tree = of_folder(&root, 3, 100);
        let expected = format!(
            "{}\n├── immich\n│   ├── data\n│   └── database\n├── media\n│   └── movies\n└── liesmich.txt\n",
            root.display()
        );
        assert_eq!(tree, expected);
        assert!(of_folder(&root, 3, 2).contains("…"));
    }
}
