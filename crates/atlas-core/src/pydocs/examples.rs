// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Examples with pictures from the online documentation. An installed
//! package has its docstrings but not the pictures the documentation shows
//! (seaborn, matplotlib, scipy … draw them when the website is built; seaborn
//! keeps even the example code outside the package). On request the
//! "Examples" part of the entry's official page is fetched once – the page
//! of the installed version if there is one, else the current one – and
//! kept in the cache with its pictures, so it works offline afterwards.
//!
//! Only the documentation hosts below are contacted; the page shown gets
//! plain blocks (text, code, local picture files), never the site's HTML.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use ego_tree::NodeRef;
use scraper::{ElementRef, Html, Node, Selector};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Version of the cached files.
const FORMAT: i64 = 1;
const MAX_PAGE: u64 = 12 << 20;
const MAX_IMAGE: u64 = 8 << 20;
const MAX_IMAGES: usize = 40;

/// Hosts of the official documentation of the packages known here.
const HOSTS: &[&str] =
    &["seaborn.pydata.org", "matplotlib.org", "pandas.pydata.org", "numpy.org", "docs.scipy.org", "scikit-learn.org"];

/// One part of the examples.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Block {
    /// Text as HTML made here (escaped text, `code`, `em`, `strong`, links).
    Text(String),
    /// Source code (with `>>>` prompts and output where the page has them).
    Code(String),
    /// A picture: its address on the page, then (in the cache) its file.
    Image(String),
}

/// What is known about an entry's online examples.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cached {
    pub format: i64,
    /// The page the examples come from.
    pub url: String,
    /// Empty: the page has no examples.
    pub blocks: Vec<Block>,
}

/// The entry asked for (fields as in the index).
#[derive(Debug, Clone, Default)]
pub struct Entry {
    pub package: String,
    pub id: String,
    pub kind: String,
    pub parent: String,
    pub version: String,
}

/// Pages to try for `entry`: the installed version's first, then the
/// current documentation. Empty for modules and unknown packages.
pub fn page_urls(entry: &Entry) -> Vec<String> {
    let Entry { package, id, kind, parent, version } = entry;
    if kind == "module" || !valid_id(id) || !valid_id(parent) && !parent.is_empty() {
        return Vec::new();
    }
    let member = kind == "method" || kind == "property";
    let parts: Vec<&str> = version.split('.').collect();
    let numeric = !version.is_empty() && version.chars().all(|c| c.is_ascii_digit() || c == '.');
    let minor = (numeric && parts.len() >= 2).then(|| format!("{}.{}", parts[0], parts[1]));
    let full = numeric.then(|| version.clone());
    let mut urls = Vec::new();
    let mut add = |versioned: Option<String>, stable: String| {
        if let Some(v) = versioned {
            urls.push(v);
        }
        urls.push(stable);
    };
    match package.as_str() {
        "seaborn" => add(
            minor.map(|v| format!("https://seaborn.pydata.org/archive/{v}/generated/{id}.html")),
            format!("https://seaborn.pydata.org/generated/{id}.html"),
        ),
        "matplotlib" => {
            let page = if id.starts_with("matplotlib.pyplot.") || id.starts_with("matplotlib.axes.Axes.") || !member {
                id
            } else {
                parent
            };
            add(
                full.map(|v| format!("https://matplotlib.org/{v}/api/_as_gen/{page}.html")),
                format!("https://matplotlib.org/stable/api/_as_gen/{page}.html"),
            )
        }
        "pandas" => add(
            full.map(|v| format!("https://pandas.pydata.org/pandas-docs/version/{v}/reference/api/{id}.html")),
            format!("https://pandas.pydata.org/docs/reference/api/{id}.html"),
        ),
        "numpy" => add(
            minor.map(|v| format!("https://numpy.org/doc/{v}/reference/generated/{id}.html")),
            format!("https://numpy.org/doc/stable/reference/generated/{id}.html"),
        ),
        "scipy" => add(
            full.map(|v| format!("https://docs.scipy.org/doc/scipy-{v}/reference/generated/{id}.html")),
            format!("https://docs.scipy.org/doc/scipy/reference/generated/{id}.html"),
        ),
        "sklearn" => {
            let page = if member { parent } else { id };
            add(
                minor.map(|v| format!("https://scikit-learn.org/{v}/modules/generated/{page}.html")),
                format!("https://scikit-learn.org/stable/modules/generated/{page}.html"),
            )
        }
        _ => {}
    }
    urls
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
}

// ---------------------------------------------------------------- reading the page

fn has_class(el: &ElementRef, name: &str) -> bool {
    el.value().classes().any(|c| c == name)
}

fn text_of(el: &ElementRef) -> String {
    el.text().collect::<String>()
}

fn is_examples_title(el: &ElementRef) -> bool {
    let name = el.value().name();
    let title = matches!(name, "h2" | "h3" | "h4" | "h5") || (name == "p" && has_class(el, "rubric"));
    title && matches!(text_of(el).trim().trim_end_matches(['¶', '#']).trim(), "Examples" | "Example")
}

/// Ends the examples: the next titled part or a member's entry.
fn ends_examples(el: &ElementRef) -> bool {
    let name = el.value().name();
    matches!(name, "h1" | "h2" | "h3" | "h4" | "h5" | "section")
        || (name == "p" && has_class(el, "rubric"))
        || (name == "dl" && has_class(el, "py"))
}

fn element_children<'a>(el: &ElementRef<'a>) -> impl Iterator<Item = ElementRef<'a>> {
    el.children().filter_map(ElementRef::wrap)
}

/// The elements of the "Examples" part of `entry_id`'s documentation.
fn examples_part<'a>(doc: &'a Html, entry_id: &str) -> Option<Vec<ElementRef<'a>>> {
    // Sphinx: <dl class="py …"><dt id="<entry>">…</dt><dd>…</dd></dl>.
    let dt = Selector::parse("dt[id]").ok()?;
    let container = doc
        .select(&dt)
        .find(|d| d.value().attr("id") == Some(entry_id))
        .and_then(|d| d.next_siblings().filter_map(ElementRef::wrap).find(|e| e.value().name() == "dd"));
    let container = match container {
        Some(c) => c,
        None => {
            let main = Selector::parse("article.bd-article, div[role=main], main, body").ok()?;
            doc.select(&main).next()?
        }
    };
    find_part(&container)
}

fn find_part<'a>(container: &ElementRef<'a>) -> Option<Vec<ElementRef<'a>>> {
    let children: Vec<ElementRef> = element_children(container).collect();
    if let Some(start) = children.iter().position(is_examples_title) {
        return Some(children[start + 1..].iter().take_while(|e| !ends_examples(e)).copied().collect());
    }
    // A section "Examples" (`<section id="examples"><h2>Examples</h2>…`).
    for child in &children {
        let name = child.value().name();
        if name == "section" && element_children(child).next().is_some_and(|h| is_examples_title(&h)) {
            return Some(element_children(child).skip(1).collect());
        }
        if name == "section" || (name == "div" && !has_class(child, "highlight")) {
            if let Some(found) = find_part(child) {
                return Some(found);
            }
        }
    }
    None
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Text of an element as HTML made here.
fn inline(node: NodeRef<Node>, base: &str, out: &mut String) {
    for child in node.children() {
        match child.value() {
            Node::Text(t) => out.push_str(&escape(t)),
            Node::Element(e) => {
                let el = ElementRef::wrap(child).expect("element");
                match e.name() {
                    "code" | "tt" | "kbd" | "samp" => {
                        out.push_str("<code>");
                        out.push_str(&escape(&text_of(&el)));
                        out.push_str("</code>");
                    }
                    "em" | "i" | "strong" | "b" | "sub" | "sup" => {
                        let tag = match e.name() {
                            "i" => "em",
                            "b" => "strong",
                            t => t,
                        };
                        out.push_str(&format!("<{tag}>"));
                        inline(child, base, out);
                        out.push_str(&format!("</{tag}>"));
                    }
                    "a" => match e.attr("href").and_then(|h| resolve(base, h)) {
                        Some(href) if href.starts_with("https://") || href.starts_with("http://") => {
                            out.push_str(&format!("<a class=\"ext\" href=\"{}\" target=\"_blank\">", escape(&href)));
                            inline(child, base, out);
                            out.push_str("</a>");
                        }
                        _ => inline(child, base, out),
                    },
                    "img" | "script" | "style" | "button" => {}
                    "br" => out.push(' '),
                    _ => inline(child, base, out),
                }
            }
            _ => {}
        }
    }
}

fn blocks_of(el: &ElementRef, base: &str, out: &mut Vec<Block>) {
    let name = el.value().name();
    let classes: Vec<&str> = el.value().classes().collect();
    if classes.iter().any(|c| c.starts_with("highlight")) || name == "pre" {
        let pre = Selector::parse("pre").expect("selector");
        let code = if name == "pre" { text_of(el) } else { el.select(&pre).map(|p| text_of(&p)).collect() };
        let code = code.trim_end().to_owned();
        if !code.trim().is_empty() {
            out.push(Block::Code(code));
        }
        return;
    }
    match name {
        "img" => {
            if let Some(src) = el.value().attr("src").and_then(|s| resolve(base, s)) {
                out.push(Block::Image(src));
            }
        }
        "p" | "figcaption" | "caption" => {
            let mut html = String::new();
            inline(**el, base, &mut html);
            let plain = text_of(el);
            let plain = plain.trim();
            // matplotlib's "(Source code, png, hires.png, pdf)" under its plots.
            if !plain.is_empty() && !plain.starts_with("(Source code") {
                out.push(Block::Text(html.trim().to_owned()));
            }
            let img = Selector::parse("img").expect("selector");
            for i in el.select(&img) {
                blocks_of(&i, base, out);
            }
        }
        "ul" | "ol" => {
            let items: Vec<String> = element_children(el)
                .filter(|li| li.value().name() == "li")
                .map(|li| {
                    let mut html = String::new();
                    inline(*li, base, &mut html);
                    format!("<li>{}</li>", html.trim())
                })
                .collect();
            if !items.is_empty() {
                out.push(Block::Text(format!("<{name}>{}</{name}>", items.concat())));
            }
        }
        "script" | "style" | "table" | "form" | "button" | "nav" => {}
        _ => {
            for child in element_children(el) {
                blocks_of(&child, base, out);
            }
        }
    }
}

/// The examples of `entry_id` on the page `html` (found at `url`); `None`
/// if the page has no such part.
pub fn extract(html: &str, url: &str, entry_id: &str) -> Option<Vec<Block>> {
    let doc = Html::parse_document(html);
    let part = examples_part(&doc, entry_id)?;
    let mut out = Vec::new();
    for el in &part {
        blocks_of(el, url, &mut out);
    }
    // The same picture twice (a link around it, srcset …) once.
    out.dedup();
    Some(out)
}

/// `href` relative to `base`; only http(s) addresses.
fn resolve(base: &str, href: &str) -> Option<String> {
    let href = href.trim();
    if href.starts_with("https://") || href.starts_with("http://") {
        return Some(href.to_owned());
    }
    if href.is_empty() || href.starts_with('#') || href.contains(':') && !href.starts_with('/') {
        return None;
    }
    let (scheme, rest) = base.split_once("://")?;
    let host_end = rest.find('/').unwrap_or(rest.len());
    let (host, path) = rest.split_at(host_end);
    let href = href.split('#').next().unwrap_or_default();
    let joined = if href.starts_with('/') {
        href.to_owned()
    } else {
        let dir = &path[..path.rfind('/').map_or(0, |i| i + 1)];
        format!("{dir}{href}")
    };
    let mut parts: Vec<&str> = Vec::new();
    for part in joined.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            p => parts.push(p),
        }
    }
    Some(format!("{scheme}://{host}/{}", parts.join("/")))
}

fn allowed(url: &str) -> bool {
    url.strip_prefix("https://").and_then(|r| r.split('/').next()).is_some_and(|host| HOSTS.contains(&host))
}

// ---------------------------------------------------------------- fetching

fn agent() -> ureq::Agent {
    let tls = ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier).build();
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(30)))
        .timeout_connect(Some(Duration::from_secs(10)))
        .user_agent(format!("Atlas/{} (Doku-Suche)", env!("CARGO_PKG_VERSION")))
        .tls_config(tls)
        .build()
        .into()
}

/// The body of `url`; `None` if the page does not exist (404).
fn fetch(agent: &ureq::Agent, url: &str, limit: u64) -> Result<Option<Vec<u8>>> {
    if !allowed(url) {
        bail!("{url} gehört nicht zu einer bekannten Dokumentation");
    }
    let mut res = agent.get(url).call().with_context(|| format!("{url} nicht erreichbar"))?;
    match res.status().as_u16() {
        200 => Ok(Some(res.body_mut().with_config().limit(limit).read_to_vec()?)),
        404 | 410 => Ok(None),
        code => bail!("{url}: HTTP {code}"),
    }
}

/// Folder of an entry's examples in the cache.
fn entry_dir(cache: &Path, entry: &Entry) -> PathBuf {
    let version: String = entry.version.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '.').collect();
    cache.join("examples").join(&entry.package).join(if version.is_empty() { "-" } else { &version }).join(&entry.id)
}

/// Fetches the examples of `entry` with their pictures into `dir`.
fn download(entry: &Entry, dir: &Path) -> Result<Cached> {
    let urls = page_urls(entry);
    if urls.is_empty() {
        bail!("Für {} gibt es keine bekannte Online-Doku", entry.id);
    }
    let agent = agent();
    let mut page = None;
    for url in &urls {
        if let Some(body) = fetch(&agent, url, MAX_PAGE)? {
            page = Some((url.clone(), String::from_utf8_lossy(&body).into_owned()));
            break;
        }
    }
    let Some((url, html)) = page else {
        return Ok(Cached { format: FORMAT, url: urls.last().cloned().unwrap_or_default(), blocks: Vec::new() });
    };
    let mut blocks = extract(&html, &url, &entry.id).unwrap_or_default();
    std::fs::create_dir_all(dir)?;
    let mut count = 0;
    let mut files: HashMap<String, PathBuf> = HashMap::new();
    for block in &mut blocks {
        let Block::Image(src) = block else { continue };
        if let Some(file) = files.get(src.as_str()) {
            *src = file.to_string_lossy().into_owned();
            continue;
        }
        count += 1;
        if count > MAX_IMAGES || !allowed(src) {
            continue;
        }
        let name: String = src
            .rsplit('/')
            .next()
            .unwrap_or_default()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
            .collect();
        let extension = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
        if !matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp") {
            continue;
        }
        let Some(bytes) = fetch(&agent, src, MAX_IMAGE)? else { continue };
        let file = dir.join(format!("{count:02}-{name}"));
        crate::paths::write_atomic(&file, &bytes)?;
        files.insert(src.clone(), file.clone());
        *src = file.to_string_lossy().into_owned();
    }
    // Pictures that could not be fetched are left out.
    blocks.retain(|b| !matches!(b, Block::Image(src) if src.starts_with("http")));
    Ok(Cached { format: FORMAT, url, blocks })
}

// ---------------------------------------------------------------- jobs

/// Entries being fetched, and the last error per entry (not kept on disk:
/// tried again next time).
static JOBS: Mutex<Option<HashMap<PathBuf, Option<String>>>> = Mutex::new(None);

fn read_cached(dir: &Path) -> Option<Cached> {
    let text = std::fs::read_to_string(dir.join("examples.json")).ok()?;
    serde_json::from_str::<Cached>(&text).ok().filter(|c| c.format == FORMAT)
}

/// The online examples of `entry`, for the view:
/// `{state: "ready", url, blocks: [{text|code|image: …}]}` (images as local
/// files), `{state: "none", url}` without examples, `{state: "loading"}`
/// while they are fetched in the background (ask again), `{state:
/// "error", message}` if that failed (`retry` fetches again).
pub fn get(cache: &Path, entry: &Entry, retry: bool) -> Value {
    let dir = entry_dir(cache, entry);
    if let Some(cached) = read_cached(&dir) {
        let state = if cached.blocks.is_empty() { "none" } else { "ready" };
        return json!({ "state": state, "url": cached.url, "blocks": cached.blocks });
    }
    if page_urls(entry).is_empty() {
        return json!({ "state": "none", "url": "" });
    }
    let mut jobs = JOBS.lock().unwrap_or_else(|e| e.into_inner());
    let jobs = jobs.get_or_insert_with(HashMap::new);
    match jobs.get(&dir) {
        Some(None) => return json!({ "state": "loading" }),
        Some(Some(error)) if !retry => return json!({ "state": "error", "message": error }),
        _ => {}
    }
    jobs.insert(dir.clone(), None);
    let entry = entry.clone();
    std::thread::spawn(move || {
        let result = download(&entry, &dir).and_then(|cached| {
            std::fs::create_dir_all(&dir)?;
            crate::paths::write_atomic(&dir.join("examples.json"), serde_json::to_string(&cached)?.as_bytes())?;
            Ok(())
        });
        let mut jobs = JOBS.lock().unwrap_or_else(|e| e.into_inner());
        let jobs = jobs.get_or_insert_with(HashMap::new);
        match result {
            Ok(()) => {
                jobs.remove(&dir);
            }
            Err(err) => {
                log::warn!("pydocs: Beispiele für {}: {err:#}", entry.id);
                jobs.insert(dir, Some(format!("{err:#}")));
            }
        }
    });
    json!({ "state": "loading" })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEABORN: &str = r#"<html><body><article class="bd-article"><section id="seaborn-heatmap">
<dl class="py function"><dt class="sig sig-object py" id="seaborn.heatmap"><span>heatmap</span></dt>
<dd><p>Plot rectangular data as a color-encoded matrix.</p>
<dl class="field-list"><dt>Parameters</dt><dd><p>data</p></dd></dl>
<div class="admonition seealso"><p class="admonition-title">See also</p></div>
<p class="rubric">Examples</p>
<p>Pass a <code class="xref"><span class="pre">DataFrame</span></code> to plot:</p>
<div class="highlight-ipython3 notranslate"><div class="highlight"><pre><span></span><span class="n">sns</span>.heatmap(glue)
</pre></div></div>
<img alt="../_images/heatmap_1_0.png" src="../_images/heatmap_1_0.png" />
<p>Use methods on the <a class="reference external" href="https://matplotlib.org/stable/api/x.html#y"><code>Axes</code></a> &amp; more:</p>
<script>alert(1)</script>
<div class="highlight-ipython3"><div class="highlight"><pre>ax = sns.heatmap(glue, annot=True)</pre></div></div>
<figure><a href="../_images/heatmap_3_0.png"><img src="../_images/heatmap_3_0.png"></a></figure>
<p class="rubric">Notes</p><p>nicht mehr</p>
</dd></dl></section></article></body></html>"#;

    #[test]
    fn takes_the_examples_part_as_plain_blocks() {
        let url = "https://seaborn.pydata.org/generated/seaborn.heatmap.html";
        let blocks = extract(SEABORN, url, "seaborn.heatmap").unwrap();
        assert_eq!(
            blocks,
            vec![
                Block::Text("Pass a <code>DataFrame</code> to plot:".into()),
                Block::Code("sns.heatmap(glue)".into()),
                Block::Image("https://seaborn.pydata.org/_images/heatmap_1_0.png".into()),
                Block::Text(
                    "Use methods on the <a class=\"ext\" href=\"https://matplotlib.org/stable/api/x.html#y\" \
                     target=\"_blank\"><code>Axes</code></a> &amp; more:"
                        .into()
                ),
                Block::Code("ax = sns.heatmap(glue, annot=True)".into()),
                Block::Image("https://seaborn.pydata.org/_images/heatmap_3_0.png".into()),
            ]
        );
        assert_eq!(extract("<html><body><p>nichts</p></body></html>", url, "seaborn.heatmap"), None);
    }

    #[test]
    fn member_examples_stop_at_the_next_member() {
        let html = r#"<html><body><dl class="py class"><dt id="sklearn.cluster.KMeans">KMeans</dt><dd>
<p class="rubric">Examples</p><div class="highlight-default"><div class="highlight"><pre>&gt;&gt;&gt; KMeans(2)</pre></div></div>
<dl class="py method"><dt id="sklearn.cluster.KMeans.fit">fit</dt><dd><p class="rubric">Examples</p>
<pre>&gt;&gt;&gt; km.fit(X)</pre></dd></dl></dd></dl></body></html>"#;
        let url = "https://scikit-learn.org/stable/modules/generated/sklearn.cluster.KMeans.html";
        assert_eq!(extract(html, url, "sklearn.cluster.KMeans").unwrap(), vec![Block::Code(">>> KMeans(2)".into())]);
        assert_eq!(
            extract(html, url, "sklearn.cluster.KMeans.fit").unwrap(),
            vec![Block::Code(">>> km.fit(X)".into())]
        );
    }

    #[test]
    fn pages_of_the_installed_version_first() {
        let entry = |package: &str, id: &str, kind: &str, parent: &str| Entry {
            package: package.into(),
            id: id.into(),
            kind: kind.into(),
            parent: parent.into(),
            version: "1.5.2".into(),
        };
        assert_eq!(
            page_urls(&entry("seaborn", "seaborn.heatmap", "function", "seaborn")),
            [
                "https://seaborn.pydata.org/archive/1.5/generated/seaborn.heatmap.html",
                "https://seaborn.pydata.org/generated/seaborn.heatmap.html"
            ]
        );
        assert_eq!(
            page_urls(&entry("sklearn", "sklearn.cluster.KMeans.fit", "method", "sklearn.cluster.KMeans"))[1],
            "https://scikit-learn.org/stable/modules/generated/sklearn.cluster.KMeans.html"
        );
        assert_eq!(
            page_urls(&entry("matplotlib", "matplotlib.axes.Axes.hist", "method", "matplotlib.axes.Axes"))[0],
            "https://matplotlib.org/1.5.2/api/_as_gen/matplotlib.axes.Axes.hist.html"
        );
        assert!(page_urls(&entry("seaborn", "seaborn", "module", "")).is_empty());
        assert!(page_urls(&entry("unbekannt", "unbekannt.f", "function", "unbekannt")).is_empty());
        assert!(page_urls(&entry("seaborn", "x/../y", "function", "seaborn")).is_empty());
    }

    #[test]
    fn addresses_and_hosts() {
        let base = "https://seaborn.pydata.org/archive/0.13/generated/seaborn.heatmap.html";
        assert_eq!(
            resolve(base, "../_images/a.png").as_deref(),
            Some("https://seaborn.pydata.org/archive/0.13/_images/a.png")
        );
        assert_eq!(resolve(base, "/x/b.png#k").as_deref(), Some("https://seaborn.pydata.org/x/b.png"));
        assert_eq!(resolve(base, "javascript:alert(1)"), None);
        assert_eq!(resolve(base, "data:image/png;base64,xx"), None);
        assert!(allowed("https://matplotlib.org/stable/a.png"));
        assert!(!allowed("https://matplotlib.org.evil.com/a.png") && !allowed("http://matplotlib.org/a.png"));
    }

    #[test]
    fn cached_examples_are_answered_without_network() {
        let dir = tempfile::tempdir().unwrap();
        let entry = Entry {
            package: "seaborn".into(),
            id: "seaborn.heatmap".into(),
            kind: "function".into(),
            parent: "seaborn".into(),
            version: "0.13.2".into(),
        };
        let folder = entry_dir(dir.path(), &entry);
        std::fs::create_dir_all(&folder).unwrap();
        let cached = Cached { format: FORMAT, url: "https://x".into(), blocks: vec![Block::Code("a".into())] };
        std::fs::write(folder.join("examples.json"), serde_json::to_string(&cached).unwrap()).unwrap();
        let answer = get(dir.path(), &entry, false);
        assert_eq!(answer["state"], "ready");
        assert_eq!(answer["blocks"][0]["code"], "a");
        let module = Entry { kind: "module".into(), id: "seaborn".into(), ..entry };
        assert_eq!(get(dir.path(), &module, false)["state"], "none");
    }
}
