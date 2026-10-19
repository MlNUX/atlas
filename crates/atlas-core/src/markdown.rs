// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Markdown: rendering for the preview, links, tags and link rewriting.
//!
//! Link syntax (see PHASES.md, Phase 1 § 4):
//! - a leading `/` is the project root: `[Parser](/src/parser.rs#L120)`
//! - other relative links are relative to the Markdown file
//! - `#L120` / `#L120-L145` address code lines, `#page=8` PDF pages,
//!   any other fragment a heading (GitHub style anchors)
//! - paths use `/`, UTF-8 and percent-encoding where needed
//!
//! Tags live in the YAML front matter (`tags: [a, architektur/backend]`);
//! they are compared case-insensitively, the spelling in the file is kept.

use std::ops::Range;
use std::path::{Component, Path, PathBuf};

use pulldown_cmark::{CodeBlockKind, CowStr, Event, LinkType, MetadataBlockKind, Options, Parser, Tag, TagEnd};

/// Where an internal link points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Anchor {
    None,
    Heading(String),
    Lines(u32, u32),
    Page(u32),
    /// A PDF annotation (`#page=8&annot=<id>`); the page is the fallback.
    Annotation(u32, String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// http(s), mailto, … – opened by the system.
    External(String),
    /// A heading in the same document.
    SameDocument(String),
    /// A file (or folder) on disk.
    File { path: PathBuf, anchor: Anchor, root_relative: bool },
}

/// How a link is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkKind {
    /// `[text](dest)`
    Inline,
    /// `[text][label]` – the destination is in a [`LinkKind::Definition`].
    Reference,
    /// `[label]: dest` (only used for rewriting, not counted as a link).
    Definition,
    /// `[[/pfad/notiz.md#Überschrift|Alias]]` – no percent-encoding, the
    /// `.md` extension may be left out.
    Wiki,
    /// `![alt](bild.png)`
    Image,
    /// `<https://…>`
    Autolink,
}

impl LinkKind {
    pub fn as_str(self) -> &'static str {
        match self {
            LinkKind::Inline => "link",
            LinkKind::Reference => "reference",
            LinkKind::Definition => "definition",
            LinkKind::Wiki => "wiki",
            LinkKind::Image => "image",
            LinkKind::Autolink => "autolink",
        }
    }
}

/// A link as written in a Markdown file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// Destination as written (`/src/parser.rs#L120`).
    pub raw: String,
    pub target: Target,
    /// 1-based line of the link.
    pub line: usize,
    /// Byte range of the destination inside the source (for rewriting);
    /// `None` for reference-style uses and autolinks.
    pub dest_range: Option<Range<usize>>,
    pub kind: LinkKind,
}

impl Link {
    pub fn is_broken(&self) -> bool {
        matches!(&self.target, Target::File { path, .. } if path.symlink_metadata().is_err())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub slug: String,
    pub line: usize,
}

#[derive(Debug, Clone, Default)]
pub struct Rendered {
    pub html: String,
    /// Tags from the front matter, as written.
    pub tags: Vec<String>,
    pub title: Option<String>,
    pub headings: Vec<Heading>,
    pub links: Vec<Link>,
    pub front_matter: FrontMatter,
}

/// URL prefix under which the preview serves local files (images).
pub const LOCAL_FILE_URL: &str = "atlas://file";

fn options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_SMART_PUNCTUATION
        | Options::ENABLE_HEADING_ATTRIBUTES
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
        | Options::ENABLE_MATH
        | Options::ENABLE_GFM
        | Options::ENABLE_WIKILINKS
}

/// GitHub style heading anchor: lowercase, spaces to `-`, punctuation dropped.
pub fn slugify(text: &str) -> String {
    text.trim()
        .to_lowercase()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            c if c.is_alphanumeric() || c == '-' || c == '_' => Some(c),
            _ => None,
        })
        .collect()
}

/// Normalised tag for comparisons: lowercase, no leading `#`, no
/// surrounding slashes.
pub fn normalize_tag(tag: &str) -> String {
    tag.trim().trim_start_matches('#').trim_matches('/').to_lowercase()
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes.get(i + 1..i + 3).and_then(|h| std::str::from_utf8(h).ok());
        if bytes[i] == b'%'
            && let Some(Ok(v)) = hex.map(|h| u8::from_str_radix(h, 16))
        {
            out.push(v);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn percent_encode_path(path: &str) -> String {
    let mut out = String::new();
    for c in path.chars() {
        match c {
            ' ' => out.push_str("%20"),
            '(' => out.push_str("%28"),
            ')' => out.push_str("%29"),
            '#' => out.push_str("%23"),
            '?' => out.push_str("%3F"),
            '%' => out.push_str("%25"),
            c => out.push(c),
        }
    }
    out
}

/// Removes `.` and `..` components without touching the file system.
pub fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

fn parse_anchor(fragment: &str) -> Anchor {
    if fragment.is_empty() {
        return Anchor::None;
    }
    if let Some(params) = fragment.strip_prefix("page=") {
        let mut parts = params.split('&');
        if let Some(Ok(page)) = parts.next().map(str::parse::<u32>) {
            let annot = parts.find_map(|p| p.strip_prefix("annot=")).filter(|id| !id.is_empty());
            return match annot {
                Some(id) => Anchor::Annotation(page, percent_decode(id)),
                None => Anchor::Page(page),
            };
        }
    }
    if let Some(lines) = fragment.strip_prefix('L') {
        let mut parts = lines.splitn(2, "-L");
        if let Some(Ok(start)) = parts.next().map(str::parse::<u32>) {
            let end = parts.next().and_then(|e| e.parse().ok()).unwrap_or(start);
            return Anchor::Lines(start, end.max(start));
        }
    }
    Anchor::Heading(percent_decode(fragment))
}

/// Resolves a link destination written in `file`.
pub fn resolve(raw: &str, file: &Path, root: Option<&Path>) -> Target {
    let raw = raw.trim();
    let lower = raw.to_ascii_lowercase();
    if lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("mailto:")
        || lower.starts_with("ftp://")
        || lower.starts_with("tel:")
    {
        return Target::External(raw.to_owned());
    }
    if let Some(fragment) = raw.strip_prefix('#') {
        return Target::SameDocument(percent_decode(fragment));
    }
    let raw = raw.strip_prefix("file://").unwrap_or(raw);
    let (path_part, fragment) = match raw.split_once('#') {
        Some((p, f)) => (p, f),
        None => (raw, ""),
    };
    // `datei.pdf?page=3` is accepted as well.
    let (path_part, query_page) = match path_part.split_once("?page=") {
        Some((p, page)) => (p, page.parse::<u32>().ok()),
        None => (path_part, None),
    };
    let decoded = percent_decode(path_part);
    let root_relative = decoded.starts_with('/') && root.is_some();
    let path = if root_relative {
        root.unwrap_or(Path::new("/")).join(decoded.trim_start_matches('/'))
    } else if decoded.starts_with('/') {
        PathBuf::from(&decoded)
    } else {
        file.parent().unwrap_or(Path::new("/")).join(&decoded)
    };
    let anchor = match query_page {
        Some(page) => Anchor::Page(page),
        None => parse_anchor(fragment),
    };
    Target::File { path: normalize(&path), anchor, root_relative }
}

/// Resolves the target of a wiki link (`[[/docs/a.md#Abschnitt]]`):
/// `/` is the project root, otherwise relative to the note; written
/// literally (no percent-encoding). A target without extension means a
/// Markdown file (`[[/docs/architektur]]` → `architektur.md`) unless a file
/// or folder of exactly that name exists.
pub fn resolve_wiki(raw: &str, file: &Path, root: Option<&Path>) -> Target {
    let raw = raw.trim();
    let (path_part, fragment) = match raw.split_once('#') {
        Some((p, f)) => (p.trim(), f.trim()),
        None => (raw, ""),
    };
    if path_part.is_empty() {
        return Target::SameDocument(fragment.to_owned());
    }
    let root_relative = path_part.starts_with('/') && root.is_some();
    let path = if root_relative {
        root.unwrap_or(Path::new("/")).join(path_part.trim_start_matches('/'))
    } else if path_part.starts_with('/') {
        PathBuf::from(path_part)
    } else {
        file.parent().unwrap_or(Path::new("/")).join(path_part)
    };
    let mut path = normalize(&path);
    if path.extension().is_none() && path.symlink_metadata().is_err() {
        path.set_extension("md");
    }
    Target::File { path, anchor: parse_anchor(fragment), root_relative }
}

/// Link text of a wiki link without alias: the note's name and the
/// heading (`[[/docs/architektur#Parser]]` → „architektur › Parser“).
pub fn wiki_display(raw: &str) -> String {
    let (path, fragment) = raw.split_once('#').unwrap_or((raw, ""));
    let name = path.trim().trim_end_matches('/').rsplit('/').next().unwrap_or_default();
    let name = name.strip_suffix(".md").unwrap_or(name);
    match (name.is_empty(), fragment.trim().is_empty()) {
        (true, _) => fragment.trim().to_owned(),
        (false, true) => name.to_owned(),
        (false, false) => format!("{name} › {}", fragment.trim()),
    }
}

/// Destination part inside `[[dest|alias]]`.
fn wiki_destination_range(src: &str, range: &Range<usize>) -> Option<Range<usize>> {
    let slice = src.get(range.clone())?;
    let start = slice.find("[[")? + 2;
    let end = slice.rfind("]]")?;
    let inner = slice.get(start..end)?;
    let len = inner.find('|').unwrap_or(inner.len());
    let lead = inner.len() - inner.trim_start().len();
    let dest = inner[..len].trim();
    let begin = range.start + start + lead;
    Some(begin..begin + dest.len())
}

/// Destination inside a reference definition `[label]: <dest> "title"`.
fn definition_destination_range(src: &str, span: &Range<usize>) -> Option<Range<usize>> {
    let slice = src.get(span.clone())?;
    let colon = slice.find("]:")? + 2;
    let rest = &slice[colon..];
    let lead = rest.len() - rest.trim_start().len();
    let trimmed = rest.trim_start();
    let (start, len) = if let Some(inner) = trimmed.strip_prefix('<') {
        (lead + 1, inner.find('>')?)
    } else {
        (lead, trimmed.find(char::is_whitespace).unwrap_or(trimmed.len()))
    };
    let begin = span.start + colon + start;
    Some(begin..begin + len)
}

fn line_starts(src: &str) -> Vec<usize> {
    std::iter::once(0).chain(src.match_indices('\n').map(|(i, _)| i + 1)).collect()
}

fn line_of(starts: &[usize], offset: usize) -> usize {
    starts.partition_point(|&s| s <= offset)
}

/// Finds the destination inside `[text](dest "title")` for rewriting.
fn destination_range(src: &str, range: &Range<usize>) -> Option<Range<usize>> {
    let slice = src.get(range.clone())?;
    if !slice.ends_with(')') {
        return None;
    }
    let open = slice.rfind("](")? + 2;
    let inner = &slice[open..slice.len() - 1];
    let lead = inner.len() - inner.trim_start().len();
    let trimmed = inner.trim_start();
    let (start, len) = if let Some(rest) = trimmed.strip_prefix('<') {
        (lead + 1, rest.find('>')?)
    } else {
        // Up to the first whitespace (a title may follow).
        (lead, trimmed.find(char::is_whitespace).unwrap_or(trimmed.len()))
    };
    let begin = range.start + open + start;
    Some(begin..begin + len)
}

/// What Atlas reads from the YAML front matter.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FrontMatter {
    pub tags: Vec<String>,
    pub title: Option<String>,
    /// Planning data shown in the calendar (`due: 2026-10-18`,
    /// `status: planned`, `estimate: 2h`).
    pub due: Option<String>,
    pub status: Option<String>,
    pub estimate: Option<String>,
}

fn front_matter(yaml: &str) -> FrontMatter {
    let Ok(docs) = yaml_rust2::YamlLoader::load_from_str(yaml) else { return FrontMatter::default() };
    let Some(doc) = docs.first() else { return FrontMatter::default() };
    let tags = match &doc["tags"] {
        yaml_rust2::Yaml::Array(items) => items.iter().filter_map(|t| t.as_str().map(str::to_owned)).collect(),
        yaml_rust2::Yaml::String(s) => s.split([',', ' ']).filter(|t| !t.is_empty()).map(str::to_owned).collect(),
        _ => Vec::new(),
    };
    let tags =
        tags.into_iter().map(|t| t.trim().trim_start_matches('#').to_owned()).filter(|t| !t.is_empty()).collect();
    let scalar = |key: &str| match &doc[key] {
        yaml_rust2::Yaml::String(s) | yaml_rust2::Yaml::Real(s) => Some(s.trim().to_owned()),
        yaml_rust2::Yaml::Integer(i) => Some(i.to_string()),
        _ => None,
    };
    FrontMatter {
        tags,
        title: doc["title"].as_str().map(str::to_owned),
        due: scalar("due"),
        status: scalar("status"),
        estimate: scalar("estimate"),
    }
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    pulldown_cmark_escape::escape_html(&mut out, s).ok();
    out
}

fn escape_attr(s: &str) -> String {
    escape(s).replace('"', "&quot;")
}

/// Extracts links, headings and tags without rendering.
pub fn analyze(src: &str, file: &Path, root: Option<&Path>) -> Rendered {
    render_impl(src, file, root, false)
}

/// Renders Markdown to HTML for the preview.
///
/// - internal links carry `data-atlas-path`/`-line`/`-page`/`-anchor`
///   attributes and the class `broken` if the target does not exist
/// - code blocks keep their language class (highlight.js), `mermaid`
///   blocks become `<pre class="mermaid">`, math becomes
///   `<span class="math" data-tex=…>` for KaTeX
/// - every top level block gets a `data-line` marker for scroll sync
pub fn render(src: &str, file: &Path, root: Option<&Path>) -> Rendered {
    render_impl(src, file, root, true)
}

fn render_impl(src: &str, file: &Path, root: Option<&Path>, html: bool) -> Rendered {
    let starts = line_starts(src);
    let mut result = Rendered::default();
    let mut events: Vec<Event> = Vec::new();
    let mut depth = 0usize;
    let mut in_metadata = false;
    let mut metadata = String::new();
    let mut code: Option<(String, String)> = None; // (language, text)
    let mut heading: Option<(u8, usize, String, usize)> = None; // level, line, text, event index
    let mut slugs: Vec<String> = Vec::new();
    // Display text of the current wiki link without alias.
    let mut wiki_label: Option<String> = None;

    let mut parser = Parser::new_ext(src, options()).into_offset_iter();
    for (event, range) in parser.by_ref() {
        let line = line_of(&starts, range.start);
        match &event {
            Event::Start(tag) => {
                if depth == 0 && html && !matches!(tag, Tag::MetadataBlock(_)) {
                    events.push(Event::Html(format!("<a class=\"line-marker\" data-line=\"{line}\"></a>").into()));
                }
                depth += 1;
            }
            Event::End(_) => depth = depth.saturating_sub(1),
            _ => {}
        }
        match event {
            Event::Start(Tag::MetadataBlock(MetadataBlockKind::YamlStyle)) => in_metadata = true,
            Event::End(TagEnd::MetadataBlock(_)) => {
                in_metadata = false;
                let fm = front_matter(&metadata);
                result.tags = fm.tags.clone();
                result.title = fm.title.clone();
                result.front_matter = fm;
            }
            Event::Text(text) if in_metadata => metadata.push_str(&text),
            // The text of a wiki link without alias is replaced by its label.
            Event::Text(_) | Event::Code(_) if wiki_label.is_some() => {}
            Event::Start(Tag::CodeBlock(kind)) => {
                let lang = match kind {
                    CodeBlockKind::Fenced(info) => info.split_whitespace().next().unwrap_or("").to_owned(),
                    CodeBlockKind::Indented => String::new(),
                };
                code = Some((lang, String::new()));
            }
            Event::Text(text) if code.is_some() => {
                if let Some((_, buf)) = code.as_mut() {
                    buf.push_str(&text);
                }
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some((lang, text)) = code.take() {
                    let block = if lang.eq_ignore_ascii_case("mermaid") {
                        format!("<pre class=\"mermaid\">{}</pre>\n", escape(&text))
                    } else if lang.is_empty() {
                        format!("<pre><code class=\"nohighlight\">{}</code></pre>\n", escape(&text))
                    } else {
                        format!("<pre><code class=\"language-{}\">{}</code></pre>\n", escape_attr(&lang), escape(&text))
                    };
                    events.push(Event::Html(block.into()));
                }
            }
            Event::InlineMath(tex) => events.push(Event::InlineHtml(
                format!("<span class=\"math\" data-tex=\"{}\"></span>", escape_attr(&tex)).into(),
            )),
            Event::DisplayMath(tex) => events.push(Event::InlineHtml(
                format!("<span class=\"math display\" data-tex=\"{}\"></span>", escape_attr(&tex)).into(),
            )),
            Event::Start(Tag::Heading { level, .. }) => {
                let level = level as u8;
                heading = Some((level, line, String::new(), events.len()));
                events.push(Event::Html(CowStr::from(""))); // placeholder for the opening tag
            }
            Event::Text(text) | Event::Code(text) if heading.is_some() => {
                if let Some((_, _, buf, _)) = heading.as_mut() {
                    buf.push_str(&text);
                }
                events.push(Event::Text(text));
            }
            Event::End(TagEnd::Heading(level)) => {
                if let Some((lvl, hline, text, index)) = heading.take() {
                    let mut slug = slugify(&text);
                    let base = slug.clone();
                    let mut n = 1;
                    while slugs.contains(&slug) {
                        slug = format!("{base}-{n}");
                        n += 1;
                    }
                    slugs.push(slug.clone());
                    events[index] = Event::Html(format!("<h{lvl} id=\"{}\">", escape_attr(&slug)).into());
                    events.push(Event::Html(format!("</h{}>\n", level as u8).into()));
                    result.headings.push(Heading { level: lvl, text, slug, line: hline });
                }
            }
            Event::Start(Tag::Link { link_type, dest_url, title, .. }) => {
                let raw = dest_url.to_string();
                let wiki = matches!(link_type, LinkType::WikiLink { .. });
                let target = if wiki { resolve_wiki(&raw, file, root) } else { resolve(&raw, file, root) };
                let (kind, dest_range) = match link_type {
                    LinkType::Inline => (LinkKind::Inline, destination_range(src, &range)),
                    LinkType::WikiLink { .. } => (LinkKind::Wiki, wiki_destination_range(src, &range)),
                    LinkType::Autolink | LinkType::Email => (LinkKind::Autolink, None),
                    _ => (LinkKind::Reference, None),
                };
                if let LinkType::WikiLink { has_pothole: false } = link_type {
                    wiki_label = Some(wiki_display(&raw));
                }
                let link = Link { raw: raw.clone(), target: target.clone(), line, dest_range, kind };
                let broken = link.is_broken();
                result.links.push(link);
                let mut attrs = format!(" href=\"{}\"", escape_attr(&raw));
                if !title.is_empty() {
                    attrs += &format!(" title=\"{}\"", escape_attr(&title));
                }
                match &target {
                    Target::External(url) => attrs += &format!(" data-atlas-external=\"{}\"", escape_attr(url)),
                    Target::SameDocument(anchor) => {
                        attrs += &format!(" data-atlas-anchor=\"{}\"", escape_attr(&slugify(anchor)))
                    }
                    Target::File { path, anchor, .. } => {
                        attrs += &format!(" data-atlas-path=\"{}\"", escape_attr(&path.to_string_lossy()));
                        match anchor {
                            Anchor::Lines(a, _) => attrs += &format!(" data-atlas-line=\"{a}\""),
                            Anchor::Page(p) => attrs += &format!(" data-atlas-page=\"{p}\""),
                            Anchor::Annotation(p, id) => {
                                attrs += &format!(" data-atlas-page=\"{p}\" data-atlas-annot=\"{}\"", escape_attr(id))
                            }
                            Anchor::Heading(h) => {
                                attrs += &format!(" data-atlas-anchor=\"{}\"", escape_attr(&slugify(h)))
                            }
                            Anchor::None => {}
                        }
                        if broken {
                            attrs += " data-broken=\"1\"";
                        }
                    }
                }
                let classes: Vec<&str> =
                    [(wiki, "wikilink"), (broken, "broken")].into_iter().filter(|c| c.0).map(|c| c.1).collect();
                if !classes.is_empty() {
                    attrs += &format!(" class=\"{}\"", classes.join(" "));
                }
                events.push(Event::InlineHtml(format!("<a{attrs}>").into()));
            }
            Event::End(TagEnd::Link) => {
                if let Some(label) = wiki_label.take() {
                    events.push(Event::Text(label.into()));
                }
                events.push(Event::InlineHtml("</a>".into()))
            }
            Event::Start(Tag::Image { link_type, dest_url, title, .. }) => {
                // Images are links too: they follow moves and count as
                // references to the file.
                let image_target = resolve(&dest_url, file, root);
                result.links.push(Link {
                    raw: dest_url.to_string(),
                    target: image_target,
                    line,
                    dest_range: match link_type {
                        LinkType::Inline => destination_range(src, &range),
                        _ => None,
                    },
                    kind: LinkKind::Image,
                });
                // Local images are served by the preview through `atlas://file/…`.
                let dest = match resolve(&dest_url, file, root) {
                    Target::File { path, .. } => {
                        format!("{LOCAL_FILE_URL}{}", percent_encode_path(&path.to_string_lossy()))
                    }
                    Target::External(url) => url,
                    Target::SameDocument(a) => format!("#{a}"),
                };
                events.push(Event::Start(Tag::Image {
                    link_type: LinkType::Inline,
                    dest_url: dest.into(),
                    title,
                    id: CowStr::from(""),
                }));
            }
            other => events.push(other),
        }
    }
    // Destinations of reference-style links live in their definitions.
    for (_, def) in parser.reference_definitions().iter() {
        let raw = def.dest.to_string();
        result.links.push(Link {
            target: resolve(&raw, file, root),
            raw,
            line: line_of(&starts, def.span.start),
            dest_range: definition_destination_range(src, &def.span),
            kind: LinkKind::Definition,
        });
    }

    if html {
        let mut out = String::with_capacity(src.len() * 2);
        if !result.tags.is_empty() {
            out.push_str("<div class=\"atlas-tags\">");
            for tag in &result.tags {
                out += &format!("<span class=\"tag\">#{}</span>", escape(tag));
            }
            out.push_str("</div>\n");
        }
        pulldown_cmark::html::push_html(&mut out, events.into_iter());
        result.html = out;
    }
    result
}

/// Relative path from `from_dir` to `to` (both absolute, normalised).
pub fn relative_path(from_dir: &Path, to: &Path) -> PathBuf {
    let from: Vec<_> = from_dir.components().collect();
    let to_c: Vec<_> = to.components().collect();
    let common = from.iter().zip(&to_c).take_while(|(a, b)| a == b).count();
    let mut out = PathBuf::new();
    for _ in common..from.len() {
        out.push("..");
    }
    for c in &to_c[common..] {
        out.push(c);
    }
    out
}

fn anchor_suffix(raw: &str) -> &str {
    // Keep the fragment / query exactly as written.
    match raw.find(['#', '?']) {
        Some(i) => &raw[i..],
        None => "",
    }
}

/// Rewrites internal links of a Markdown file after files were moved.
///
/// - `old_file` is where this Markdown file was when its relative links were
///   written (equal to `new_file` unless the file itself moved)
/// - `map` returns the new absolute location of a moved target
///
/// Root-relative links (`/…`) stay root-relative, relative ones stay
/// relative to the (new) file. Returns the new source and the number of
/// changed links.
pub fn rewrite_links(
    src: &str,
    old_file: &Path,
    new_file: &Path,
    root: Option<&Path>,
    map: impl Fn(&Path) -> Option<PathBuf>,
) -> (String, usize) {
    let analysis = analyze(src, old_file, root);
    let mut edits: Vec<(Range<usize>, String)> = Vec::new();
    for link in analysis.links {
        let (Some(range), Target::File { path, root_relative, .. }) = (link.dest_range, &link.target) else {
            continue;
        };
        let moved_target = map(path);
        let file_moved = old_file != new_file;
        if moved_target.is_none() && (!file_moved || *root_relative) {
            continue;
        }
        let target = moved_target.unwrap_or_else(|| path.clone());
        let suffix = anchor_suffix(&link.raw);
        let new_path = match (root_relative, root) {
            (true, Some(root)) => {
                format!("/{}", target.strip_prefix(root).unwrap_or(&target).to_string_lossy())
            }
            _ => relative_path(new_file.parent().unwrap_or(Path::new("/")), &target).to_string_lossy().into_owned(),
        };
        let new_raw = if link.kind == LinkKind::Wiki {
            // Written literally; keep a left-out `.md` left out.
            let written = link.raw.split('#').next().unwrap_or_default().trim();
            let new_path = match new_path.strip_suffix(".md") {
                Some(stem) if Path::new(written).extension().is_none() => stem.to_owned(),
                _ => new_path,
            };
            format!("{new_path}{suffix}")
        } else {
            format!("{}{suffix}", percent_encode_path(&new_path))
        };
        if new_raw != link.raw {
            edits.push((range, new_raw));
        }
    }
    edits.sort_by_key(|e| e.0.start);
    edits.dedup_by_key(|e| e.0.start);
    let count = edits.len();
    let mut out = src.to_owned();
    for (range, text) in edits.into_iter().rev() {
        out.replace_range(range, &text);
    }
    (out, count)
}

/// Wraps `text` in a fenced code block (fence longer than any backtick run
/// inside, so terminal output containing ``` stays intact).
pub fn code_block(text: &str, language: &str) -> String {
    let longest = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat(longest.max(2) + 1);
    format!("{fence}{language}\n{}\n{fence}\n", text.trim_end_matches('\n'))
}

/// Appends a block to Markdown source, separated by one blank line.
pub fn append_block(source: &str, block: &str) -> String {
    let mut out = source.trim_end_matches('\n').to_owned();
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    out.push_str(block);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOT: &str = "/proj";

    fn file(rel: &str) -> PathBuf {
        Path::new(ROOT).join(rel)
    }

    #[test]
    fn resolves_link_forms() {
        let f = file("notes/a.md");
        let root = Some(Path::new(ROOT));
        assert_eq!(
            resolve("/src/parser.rs#L120-L145", &f, root),
            Target::File { path: file("src/parser.rs"), anchor: Anchor::Lines(120, 145), root_relative: true }
        );
        assert_eq!(
            resolve("../docs/Mein%20Text.pdf#page=8", &f, root),
            Target::File { path: file("docs/Mein Text.pdf"), anchor: Anchor::Page(8), root_relative: false }
        );
        assert_eq!(
            resolve("b.md#Einleitung", &f, root),
            Target::File {
                path: file("notes/b.md"),
                anchor: Anchor::Heading("Einleitung".into()),
                root_relative: false
            }
        );
        assert_eq!(resolve("#ziel", &f, root), Target::SameDocument("ziel".into()));
        assert!(matches!(resolve("https://example.org", &f, root), Target::External(_)));
        assert!(matches!(resolve("a.pdf?page=3", &f, root), Target::File { anchor: Anchor::Page(3), .. }));
        assert_eq!(
            resolve("/a.pdf#page=3&annot=ab%2012", &f, root),
            Target::File { path: file("a.pdf"), anchor: Anchor::Annotation(3, "ab 12".into()), root_relative: true }
        );
    }

    #[test]
    fn renders_tags_headings_math_mermaid_and_links() {
        let src = "---\ntitle: Notiz\ntags: [Architektur/Backend, rust]\n---\n\n# Einleitung\n\nText mit $x^2$ und [Code](/src/main.rs#L3) und [weg](/fehlt.md).\n\n```mermaid\ngraph TD; A-->B\n```\n\n```rust\nfn main() {}\n```\n";
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/main.rs"), "").unwrap();
        let note = dir.path().join("note.md");
        let r = render(src, &note, Some(dir.path()));
        assert_eq!(r.tags, ["Architektur/Backend", "rust"]);
        assert_eq!(r.title.as_deref(), Some("Notiz"));
        assert_eq!(r.headings[0].slug, "einleitung");
        assert!(r.html.contains("<h1 id=\"einleitung\">Einleitung</h1>"), "{}", r.html);
        assert!(r.html.contains("class=\"math\" data-tex=\"x^2\""));
        assert!(r.html.contains("<pre class=\"mermaid\">graph TD; A--&gt;B\n</pre>"));
        assert!(r.html.contains("class=\"language-rust\""));
        assert!(r.html.contains("data-atlas-line=\"3\""));
        assert!(r.html.contains("class=\"broken\""), "missing target is marked");
        assert!(r.html.contains("data-line=\"6\""), "line markers for scroll sync");
        assert!(!r.html.contains("tags:"), "front matter is not rendered");
        assert_eq!(r.links.iter().filter(|l| l.is_broken()).count(), 1);
        assert_eq!(normalize_tag("#Architektur/Backend/"), "architektur/backend");
    }

    #[test]
    fn rewrites_links_after_moves() {
        let root = Path::new(ROOT);
        let src = "Siehe [P](/src/parser.rs#L1), [B](../docs/b.pdf#page=2 \"Titel\") und [X](https://x.org).";
        let note = file("notes/a.md");
        // The target moved: src/parser.rs -> src/parse/parser.rs
        let (out, n) = rewrite_links(src, &note, &note, Some(root), |p| {
            (p == file("src/parser.rs")).then(|| file("src/parse/parser.rs"))
        });
        assert_eq!(n, 1);
        assert_eq!(
            out,
            "Siehe [P](/src/parse/parser.rs#L1), [B](../docs/b.pdf#page=2 \"Titel\") und [X](https://x.org)."
        );

        // The note itself moved one level deeper: relative links follow.
        let new_note = file("notes/old/a.md");
        let (out, n) = rewrite_links(src, &note, &new_note, Some(root), |_| None);
        assert_eq!(n, 1);
        assert!(out.contains("[B](../../docs/b.pdf#page=2 \"Titel\")"), "{out}");
        assert!(out.contains("[P](/src/parser.rs#L1)"), "root-relative links stay");
    }

    #[test]
    fn wiki_links_images_and_definitions() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("docs")).unwrap();
        std::fs::write(root.join("docs/architektur.md"), "# Parser\n").unwrap();
        std::fs::write(root.join("docs/bild.png"), "x").unwrap();
        let note = root.join("notes/a.md");
        let src = "Siehe [[/docs/architektur.md#Parser|Architektur]] und [[/docs/architektur]] \
                   und [[../docs/architektur.md]] und [[fehlt]].\n\n![Bild](../docs/bild.png)\n\n\
                   [Ref][arch]\n\n[arch]: ../docs/architektur.md \"Titel\"\n";
        let r = render(src, &note, Some(root));
        let wiki: Vec<_> = r.links.iter().filter(|l| l.kind == LinkKind::Wiki).collect();
        assert_eq!(wiki.len(), 4);
        assert_eq!(
            wiki[0].target,
            Target::File {
                path: root.join("docs/architektur.md"),
                anchor: Anchor::Heading("Parser".into()),
                root_relative: true
            }
        );
        assert_eq!(&src[wiki[0].dest_range.clone().unwrap()], "/docs/architektur.md#Parser");
        assert!(matches!(&wiki[1].target, Target::File { path, .. } if path == &root.join("docs/architektur.md")));
        assert!(!wiki[2].is_broken() && wiki[3].is_broken(), "without extension: .md");
        assert!(r.html.contains("class=\"wikilink\""), "{}", r.html);
        assert!(r.html.contains(">Architektur</a>"), "alias is the link text: {}", r.html);
        assert!(r.html.contains(">architektur</a>"), "without alias: the note's name: {}", r.html);
        assert_eq!(wiki_display("/docs/a.md#Parser"), "a › Parser");
        assert_eq!(wiki_display("#Abschnitt"), "Abschnitt");
        assert!(r.html.contains("data-atlas-anchor=\"parser\""));
        assert!(r.html.contains("class=\"wikilink broken\""));
        assert!(r.links.iter().any(|l| l.kind == LinkKind::Image && !l.is_broken()));
        let def = r.links.iter().find(|l| l.kind == LinkKind::Definition).unwrap();
        assert_eq!(&src[def.dest_range.clone().unwrap()], "../docs/architektur.md");

        // Everything follows when docs/ becomes doc/.
        let (out, n) = rewrite_links(src, &note, &note, Some(root), |p| {
            p.strip_prefix(root.join("docs")).ok().map(|rest| root.join("doc").join(rest))
        });
        assert_eq!(n, 5, "{out}");
        assert!(out.contains("[[/doc/architektur.md#Parser|Architektur]]"), "{out}");
        assert!(out.contains("[[/doc/architektur]]"), "a left-out extension stays left out: {out}");
        assert!(out.contains("[[../doc/architektur.md]]"), "{out}");
        assert!(out.contains("![Bild](../doc/bild.png)"), "{out}");
        assert!(out.contains("[arch]: ../doc/architektur.md \"Titel\""), "{out}");
        assert!(out.contains("[[fehlt]]"), "unrelated links stay: {out}");
    }

    #[test]
    fn relative_paths() {
        assert_eq!(relative_path(Path::new("/p/notes"), Path::new("/p/docs/a.pdf")), PathBuf::from("../docs/a.pdf"));
        assert_eq!(relative_path(Path::new("/p"), Path::new("/p/a.md")), PathBuf::from("a.md"));
    }

    #[test]
    fn code_blocks_for_terminal_output() {
        assert_eq!(code_block("$ ls\na b\n", "console"), "```console\n$ ls\na b\n```\n");
        assert_eq!(code_block("x ``` y", ""), "````\nx ``` y\n````\n");
        assert_eq!(append_block("# Notiz\n", "```\nx\n```\n"), "# Notiz\n\n```\nx\n```\n");
        assert_eq!(append_block("", "b\n"), "b\n");
    }
}
