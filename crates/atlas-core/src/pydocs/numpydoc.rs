// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Docstrings in the numpydoc format (pandas, numpy, matplotlib, scipy,
//! scikit-learn, seaborn …): the text before the first section, and the
//! sections ("Parameters", "Returns", "See Also" …) split into fields.

/// A docstring cleaned like `inspect.cleandoc`: the first line stripped,
/// the common indentation of the others removed, blank lines at the ends
/// dropped, tabs expanded.
pub fn clean(doc: &str) -> String {
    let doc = doc.replace('\t', "        ");
    let lines: Vec<&str> = doc.lines().collect();
    if lines.is_empty() {
        return String::new();
    }
    let margin =
        lines[1..].iter().filter(|l| !l.trim().is_empty()).map(|l| l.len() - l.trim_start().len()).min().unwrap_or(0);
    let mut out: Vec<String> = vec![lines[0].trim().to_owned()];
    out.extend(
        lines[1..]
            .iter()
            .map(|l| if l.len() >= margin { l[margin..].trim_end().to_owned() } else { l.trim().to_owned() }),
    );
    while out.first().is_some_and(|l| l.is_empty()) {
        out.remove(0);
    }
    while out.last().is_some_and(|l| l.is_empty()) {
        out.pop();
    }
    out.join("\n")
}

/// Key of a section title (`params`, `returns`, … or `x:<Title>` for
/// unknown ones).
fn section_key(title: &str) -> Option<&'static str> {
    Some(match title.trim().trim_end_matches(':').to_lowercase().as_str() {
        "parameters" | "params" | "arguments" | "args" => "params",
        "other parameters" | "keyword arguments" | "kwargs" => "other",
        "returns" | "return" => "returns",
        "yields" | "yield" => "yields",
        "raises" => "raises",
        "warns" => "warns",
        "see also" => "see",
        "notes" => "notes",
        "examples" | "example" => "examples",
        "references" => "refs",
        "attributes" => "attrs",
        "methods" => "methods",
        _ => return None,
    })
}

pub struct Sections {
    /// The text before the first section.
    pub head: String,
    /// Sections in their order: key and lines.
    pub sections: Vec<(String, Vec<String>)>,
}

impl Sections {
    pub fn get(&self, key: &str) -> Vec<String> {
        self.sections.iter().filter(|(k, _)| k == key).flat_map(|(_, l)| l.iter().cloned()).collect()
    }
}

fn is_rule(line: &str) -> bool {
    let t = line.trim();
    t.len() >= 3 && t.chars().all(|c| c == '-')
}

pub fn split_sections(doc: &str) -> Sections {
    let lines: Vec<&str> = doc.lines().collect();
    let mut head = Vec::new();
    let mut sections: Vec<(String, Vec<String>)> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let next = lines.get(i + 1).copied().unwrap_or("");
        if is_rule(next) && !line.trim().is_empty() && !line.starts_with(' ') {
            let key = section_key(line).map(str::to_owned).unwrap_or_else(|| format!("x:{}", line.trim()));
            sections.push((key, Vec::new()));
            i += 2;
            continue;
        }
        match sections.last_mut() {
            Some((_, body)) => body.push(line.to_owned()),
            None => head.push(line),
        }
        i += 1;
    }
    Sections { head: head.join("\n").trim().to_owned(), sections }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub ty: String,
    pub desc: String,
}

/// Entries `name : type` (for returns also a bare type) with indented text.
pub fn parse_fields(lines: &[String], returns: bool) -> Vec<Field> {
    let mut out: Vec<(String, String, Vec<String>)> = Vec::new();
    for line in lines {
        if line.trim().is_empty() {
            if let Some(e) = out.last_mut() {
                e.2.push(String::new());
            }
            continue;
        }
        if !line.starts_with([' ', '\t']) {
            let t = line.trim();
            let (name, ty) = match t.split_once(':') {
                Some((n, ty)) if !t.starts_with("http") && !t.starts_with('{') => {
                    (n.trim().to_owned(), ty.trim().to_owned())
                }
                _ if returns => (String::new(), t.to_owned()),
                _ => (t.to_owned(), String::new()),
            };
            out.push((name, ty, Vec::new()));
        } else if let Some(e) = out.last_mut() {
            e.2.push(line.trim().to_owned());
        }
    }
    out.into_iter()
        .map(|(name, ty, desc)| {
            let mut desc = desc.join("\n").trim().to_owned();
            while desc.contains("\n\n\n") {
                desc = desc.replace("\n\n\n", "\n\n");
            }
            if desc.len() > 4000 {
                let mut cut = 4000;
                while !desc.is_char_boundary(cut) {
                    cut -= 1;
                }
                desc.truncate(cut);
            }
            Field { name, ty, desc }
        })
        .collect()
}

/// The first paragraph that is not a signature line (C functions repeat
/// theirs: `arange([start,] stop[, step,], dtype=None)`).
pub fn summary(head: &str) -> String {
    for para in paragraphs(head).into_iter().take(3) {
        let text = para.join(" ");
        if !looks_like_signature(&text) {
            return text.chars().take(400).collect();
        }
    }
    String::new()
}

fn paragraphs(text: &str) -> Vec<Vec<&str>> {
    let mut out: Vec<Vec<&str>> = vec![Vec::new()];
    for line in text.lines() {
        if line.trim().is_empty() {
            if out.last().is_some_and(|p| !p.is_empty()) {
                out.push(Vec::new());
            }
        } else if let Some(p) = out.last_mut() {
            p.push(line.trim());
        }
    }
    out.retain(|p| !p.is_empty());
    out
}

/// `add(x1, x2, /, out=None)` or `name(...) -> type` (C docstrings start so).
pub fn looks_like_signature(text: &str) -> bool {
    let t = text.trim();
    let Some(open) = t.find('(') else { return false };
    open > 0
        && t[..open].chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.')
        && (t.ends_with(')') || t[t.rfind(')').unwrap_or(0)..].trim_start_matches(')').trim().starts_with("->"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "Group DataFrame using a mapper.

    Longer text.

    Parameters
    ----------
    by : mapping, function, label
        Used to determine the groups.

        Second paragraph.
    level, axis : int, default None
    **kwargs
        Passed on.

    Returns
    -------
    DataFrameGroupBy
        Returns a groupby object.

    See Also
    --------
    resample : Resample time series.
    ";

    #[test]
    fn splits_numpydoc_sections_and_fields() {
        let doc = clean(DOC);
        assert!(doc.starts_with("Group DataFrame using a mapper.\n\nLonger text."));
        let s = split_sections(&doc);
        assert_eq!(summary(&s.head), "Group DataFrame using a mapper.");
        let keys: Vec<_> = s.sections.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, ["params", "returns", "see"]);
        let params = parse_fields(&s.get("params"), false);
        assert_eq!(params.len(), 3);
        assert_eq!(params[0].name, "by");
        assert_eq!(params[0].ty, "mapping, function, label");
        assert_eq!(params[0].desc, "Used to determine the groups.\n\nSecond paragraph.");
        assert_eq!(params[1].name, "level, axis");
        assert_eq!(params[2].name, "**kwargs");
        let ret = parse_fields(&s.get("returns"), true);
        assert_eq!(
            ret,
            [Field { name: String::new(), ty: "DataFrameGroupBy".into(), desc: "Returns a groupby object.".into() }]
        );
    }

    #[test]
    fn signature_lines_are_no_summary() {
        assert!(looks_like_signature("add(x1, x2, /, out=None, *, where=True)"));
        assert!(looks_like_signature("array(object, dtype=None) -> ndarray"));
        assert!(!looks_like_signature("Compute the mean (average)."));
        assert_eq!(summary("sqrt(x, /)\n\nSquare root."), "Square root.");
        assert_eq!(
            summary("arange([start,] stop[, step,])\n\nReturn evenly spaced values."),
            "Return evenly spaced values."
        );
    }
}
