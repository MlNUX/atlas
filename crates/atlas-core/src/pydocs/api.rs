// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! The public API of one installed package, read from its source files:
//! which names it exports (following imports, `__all__`, star imports and
//! `.pyi` stubs), what each one is, and its documentation – the entries of
//! the docs search. Nothing is executed.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::lexer::{self, Tok};
use super::numpydoc;
use super::outline::{self, Def, Import, Outline, Param};

/// Submodules read in addition to those the package imports itself (the
/// public API lives there, e.g. matplotlib.pyplot).
fn extra_modules(pkg: &str) -> &'static [&'static str] {
    match pkg {
        "matplotlib" => &[
            "pyplot",
            "axes",
            "artist",
            "figure",
            "colors",
            "cm",
            "ticker",
            "dates",
            "patches",
            "lines",
            "collections",
            "text",
            "gridspec",
            "animation",
            "legend",
            "image",
            "markers",
            "transforms",
        ],
        "scipy" => &[
            "stats",
            "optimize",
            "integrate",
            "interpolate",
            "linalg",
            "signal",
            "sparse",
            "spatial",
            "special",
            "fft",
            "ndimage",
            "cluster",
            "io",
            "constants",
        ],
        "sklearn" => &[
            "base",
            "cluster",
            "compose",
            "datasets",
            "decomposition",
            "ensemble",
            "feature_extraction",
            "feature_selection",
            "impute",
            "inspection",
            "linear_model",
            "metrics",
            "model_selection",
            "naive_bayes",
            "neighbors",
            "neural_network",
            "pipeline",
            "preprocessing",
            "svm",
            "tree",
            "utils",
        ],
        "pandas" => &["api.types", "plotting", "testing", "errors", "tseries.offsets"],
        "numpy" => &["linalg", "random", "fft", "polynomial", "ma", "char", "strings", "testing", "lib.stride_tricks"],
        "seaborn" => &["objects"],
        _ => &[],
    }
}
/// Never walked (slow, side effects or not public).
const SKIP_PARTS: &[&str] = &[
    "tests",
    "test",
    "testing_utils",
    "conftest",
    "setup",
    "compat",
    "core",
    "_core",
    "libs",
    "experimental",
    "externals",
    "distutils",
    "f2py",
    "array_api",
    "backends",
    "sphinxext",
    "conv",
];
const SKIP_TOP: &[&str] = &["tests", "conftest", "setup", "distutils", "f2py", "sphinxext", "backends", "externals"];
const MAX_ENTRIES: usize = 40000;
const MAX_DOC: usize = 60000;
/// Descriptors whose value is a class with methods of its own (pandas
/// `Series.str`, `DataFrame.plot`).
const ACCESSORS: &[&str] = &["Accessor", "CachedAccessor"];
const PROPERTY_DECORATORS: &[&str] = &[
    "property",
    "cache_readonly",
    "cached_property",
    "functools.cached_property",
    "abc.abstractproperty",
    "abstractproperty",
];

struct Module {
    /// `.py` (or `__init__.py`), then the `.pyi` stub if there is one.
    py: Option<Outline>,
    stub: Option<Outline>,
    is_package: bool,
}

/// What a name refers to.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Target {
    Module(String),
    /// A definition: module and the names down to it (class, method).
    Def(String, Vec<String>),
    /// `name: <annotation>` in a stub (numpy's ufuncs).
    Annotated(String, String),
}

pub struct Package {
    pub name: String,
    root: PathBuf,
    modules: HashMap<String, Option<Module>>,
    /// `add_newdoc(module, name, (attr,) doc)` of the whole package.
    newdocs: HashMap<(String, String, String), String>,
    /// Entries without text whose definition may be compiled: index in
    /// the output, module, short name, class, kind and definition.
    undocumented: Vec<Undocumented>,
    /// Texts for matplotlib's `%(key)s` placeholders.
    interpd: HashMap<String, String>,
    /// Names resolved so far, and those being resolved (cycles).
    memo: HashMap<(String, String), Option<Target>>,
    resolving: HashSet<(String, String)>,
}

struct Undocumented {
    index: usize,
    module: String,
    name: String,
    class: Option<String>,
    kind: String,
    qual: String,
    def: Def,
}

/// One entry of the index (the JSON the view reads).
pub type Entry = Value;

impl Package {
    /// `dir` is the package folder (or the single file `name.py`).
    pub fn open(name: &str, dir: &Path) -> Package {
        let root = dir.parent().unwrap_or(dir).to_path_buf();
        let mut pkg = Package {
            name: name.to_owned(),
            root,
            modules: HashMap::new(),
            newdocs: HashMap::new(),
            undocumented: Vec::new(),
            memo: HashMap::new(),
            resolving: HashSet::new(),
            interpd: HashMap::new(),
        };
        pkg.collect_newdocs(dir);
        pkg
    }

    /// numpy documents C functions with `add_newdoc(...)` in separate files.
    fn collect_newdocs(&mut self, dir: &Path) {
        if !dir.is_dir() {
            return;
        }
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            for entry in std::fs::read_dir(&d).into_iter().flatten().flatten() {
                let p = entry.path();
                let name = entry.file_name().to_string_lossy().into_owned();
                if p.is_dir() {
                    if name != "tests" && !name.starts_with('.') {
                        stack.push(p);
                    }
                } else if name.ends_with(".py") {
                    let Ok(text) = std::fs::read_to_string(&p) else { continue };
                    if !text.contains("add_newdoc") && !text.contains("interpd.") {
                        continue;
                    }
                    let o = outline::parse(&text);
                    for (module, name, attr, doc) in o.newdocs.iter().cloned() {
                        self.newdocs.insert((module, name, attr.unwrap_or_default()), doc);
                    }
                    for (key, value) in &o.interpd {
                        // A string, or a module level name holding one.
                        let text = leading_strings(value).or_else(|| {
                            let name = value.first().and_then(Tok::name).filter(|_| value.len() == 1)?;
                            o.assigns.iter().rev().find(|(n, _)| n == name).and_then(|(_, v)| leading_strings(v))
                        });
                        if let Some(text) = text {
                            self.interpd.insert(key.clone(), numpydoc::clean(&text));
                        }
                    }
                }
            }
        }
    }

    fn module(&mut self, name: &str) -> Option<&Module> {
        if !self.modules.contains_key(name) {
            let loaded = self.load(name);
            self.modules.insert(name.to_owned(), loaded);
        }
        self.modules.get(name).and_then(Option::as_ref)
    }

    fn load(&self, name: &str) -> Option<Module> {
        let rel: PathBuf = name.split('.').collect();
        let base = self.root.join(&rel);
        let read = |p: PathBuf| std::fs::read_to_string(p).ok().map(|t| outline::parse(&t));
        if base.is_dir() {
            let py = read(base.join("__init__.py"));
            let stub = read(base.join("__init__.pyi"));
            if py.is_none() && stub.is_none() {
                return None;
            }
            return Some(Module { py, stub, is_package: true });
        }
        let py = read(base.with_extension("py"));
        let stub = read(base.with_extension("pyi"));
        if py.is_none() && stub.is_none() {
            return None;
        }
        Some(Module { py, stub, is_package: false })
    }

    fn exists(&mut self, module: &str) -> bool {
        self.module(module).is_some()
    }

    /// Absolute name of `from <spec> import …` written in `module`.
    fn absolute(&mut self, module: &str, spec: &str) -> String {
        let dots = spec.chars().take_while(|c| *c == '.').count();
        if dots == 0 {
            return spec.to_owned();
        }
        let is_package = self.module(module).is_some_and(|m| m.is_package);
        let mut parts: Vec<&str> = module.split('.').collect();
        // In a package's __init__ one dot is the package itself.
        let up = if is_package { dots - 1 } else { dots };
        for _ in 0..up {
            parts.pop();
        }
        let rest = &spec[dots..];
        let mut out = parts.join(".");
        if !rest.is_empty() {
            if !out.is_empty() {
                out.push('.');
            }
            out.push_str(rest);
        }
        out
    }

    fn in_package(&self, module: &str) -> bool {
        module == self.name || module.starts_with(&format!("{}.", self.name))
    }

    /// Names a module exports: `__all__` if it has one, else every public
    /// name it defines or imports.
    fn public_names(&mut self, module: &str, seen: &mut HashSet<String>) -> Vec<String> {
        if !seen.insert(module.to_owned()) {
            return Vec::new();
        }
        let Some(m) = self.module(module) else { return Vec::new() };
        let outlines: Vec<Outline> = m.py.iter().chain(m.stub.iter()).cloned().collect();
        let mut names = Vec::new();
        let mut all_seen = false;
        for o in &outlines {
            if let Some(all) = &o.all {
                all_seen = true;
                names.extend(all.iter().cloned());
                for from in o.all_from.clone() {
                    // `__all__ += numeric.__all__`
                    let toks = dotted_tokens(&from);
                    if let Some(Target::Module(sub)) = self.resolve_expr(module, &toks, 0) {
                        names.extend(self.public_names(&sub, seen));
                    }
                }
            }
            if all_seen {
                break;
            }
        }
        if !all_seen {
            for o in &outlines {
                names.extend(o.defs.iter().map(|d| d.name.clone()));
                names.extend(o.imports.iter().map(|(n, _)| n.clone()));
                names.extend(o.assigns.iter().map(|(n, _)| n.clone()));
                names.extend(o.annotated.iter().map(|(n, _)| n.clone()));
                for star in o.star_imports.clone() {
                    let abs = self.absolute(module, &star);
                    names.extend(self.public_names(&abs, seen));
                }
            }
            names.retain(|n| !n.starts_with('_'));
        }
        let mut unique = HashSet::new();
        names.retain(|n| unique.insert(n.clone()));
        names
    }

    /// What `name` in `module` refers to.
    fn resolve(&mut self, module: &str, name: &str, depth: usize) -> Option<Target> {
        let key = (module.to_owned(), name.to_owned());
        if let Some(t) = self.memo.get(&key) {
            return t.clone();
        }
        if depth > 32 || !self.resolving.insert(key.clone()) {
            return None;
        }
        let found = self.resolve_uncached(module, name, depth);
        self.resolving.remove(&key);
        self.memo.insert(key, found.clone());
        found
    }

    fn resolve_uncached(&mut self, module: &str, name: &str, depth: usize) -> Option<Target> {
        let m = self.module(module)?;
        let outlines: Vec<Outline> = m.py.iter().chain(m.stub.iter()).cloned().collect();
        for o in &outlines {
            if pick(&o.defs, name).is_some() {
                return Some(Target::Def(module.to_owned(), vec![name.to_owned()]));
            }
            for (bound, import) in o.imports.iter().rev() {
                if bound != name {
                    continue;
                }
                // An import that leads nowhere (compiled, other package):
                // the stub may still declare the name.
                let found = match import {
                    Import::Module(m) => Some(Target::Module(m.clone())),
                    Import::Name(from, n) => {
                        let abs = self.absolute(module, from);
                        let sub = if abs.is_empty() { n.clone() } else { format!("{abs}.{n}") };
                        // `from . import plotting`: a submodule, unless the
                        // module defines something of that name itself.
                        if self.in_package(&sub) && self.exists(&sub) && !self.defines(&abs, n) {
                            Some(Target::Module(sub))
                        } else if self.in_package(&abs) && abs != module {
                            self.resolve(&abs, n, depth + 1)
                        } else {
                            None
                        }
                    }
                };
                if found.is_some() {
                    return found;
                }
                break;
            }
            if let Some((_, value)) = o.assigns.iter().rev().find(|(n, _)| n == name) {
                if let Some(t) = self.resolve_expr(module, value, depth) {
                    return Some(t);
                }
            }
        }
        for o in &outlines {
            for star in &o.star_imports {
                let abs = self.absolute(module, star);
                if !self.in_package(&abs) {
                    continue;
                }
                let names = self.public_names(&abs, &mut HashSet::new());
                if names.iter().any(|n| n == name) {
                    if let Some(t) = self.resolve(&abs, name, depth + 1) {
                        return Some(t);
                    }
                }
            }
        }
        for o in &outlines {
            if let Some((_, ann)) = o.annotated.iter().find(|(n, _)| n == name) {
                return Some(Target::Annotated(module.to_owned(), format!("{name}:{ann}")));
            }
        }
        // `__all__` computed at run time (numpy's `extend_all(umath)`):
        // the star modules may still have it.
        for o in &outlines {
            for star in &o.star_imports {
                let abs = self.absolute(module, star);
                if self.in_package(&abs) && abs != module {
                    if let Some(t) = self.resolve(&abs, name, depth + 1) {
                        return Some(t);
                    }
                }
            }
        }
        // A submodule that was imported elsewhere.
        let sub = format!("{module}.{name}");
        if self.exists(&sub) {
            return Some(Target::Module(sub));
        }
        None
    }

    fn defines(&mut self, module: &str, name: &str) -> bool {
        self.module(module).is_some_and(|m| m.py.iter().chain(m.stub.iter()).any(|o| pick(&o.defs, name).is_some()))
    }

    /// `other`, `mod.attr`, `Class.method` on the right of an assignment.
    fn resolve_expr(&mut self, module: &str, value: &[Tok], depth: usize) -> Option<Target> {
        let names: Vec<&str> = value.iter().filter_map(Tok::name).collect();
        let dotted = value.iter().enumerate().all(|(i, t)| if i % 2 == 0 { t.name().is_some() } else { t.is_op(".") });
        if !dotted || names.is_empty() {
            return None;
        }
        let mut target = self.resolve(module, names[0], depth + 1)?;
        for attr in &names[1..] {
            target = match target {
                Target::Module(m) => self.resolve(&m, attr, depth + 1)?,
                Target::Def(m, path) => {
                    let mut own = path.clone();
                    own.push((*attr).to_owned());
                    if self.def(&m, &own).is_some() {
                        Target::Def(m, own)
                    } else {
                        // Inherited (`Axes.grid` is `_AxesBase.grid`).
                        let (bm, mut bp, _) = mro(self, &m, &path)
                            .into_iter()
                            .skip(1)
                            .find(|(_, _, d)| pick(&d.members, attr).is_some())?;
                        bp.push((*attr).to_owned());
                        Target::Def(bm, bp)
                    }
                }
                // `_rand: RandomState` … `beta = _rand.beta` (numpy.random).
                Target::Annotated(m, ann) => {
                    let ty = ann.split_once(':').map(|(_, t)| t.trim().to_owned()).unwrap_or_default();
                    let ty = ty.split('[').next().unwrap_or_default().trim().to_owned();
                    let Some(Target::Def(cm, cp)) = self.resolve_expr(&m, &dotted_tokens(&ty), depth + 1) else {
                        return None;
                    };
                    let mut own = cp.clone();
                    own.push((*attr).to_owned());
                    self.def(&cm, &own)?;
                    Target::Def(cm, own)
                }
            };
        }
        Some(target)
    }

    /// The definition at `path` in `module` (implementation over stub).
    fn def(&mut self, module: &str, path: &[String]) -> Option<Def> {
        let m = self.module(module)?;
        for (index, o) in m.py.iter().chain(m.stub.iter()).enumerate() {
            let from_stub = index == 1 || m.py.is_none();
            let Some(mut d) = pick(&o.defs, &path[0]) else { continue };
            let mut ok = true;
            for name in &path[1..] {
                match pick(&d.members, name) {
                    Some(x) => d = x,
                    None => {
                        ok = false;
                        break;
                    }
                }
            }
            if ok {
                let mut d = d.clone();
                fn mark(d: &mut Def, stub: bool) {
                    d.stub = stub;
                    for m in &mut d.members {
                        mark(m, stub);
                    }
                }
                mark(&mut d, from_stub);
                // Signature from the stub when the implementation has none
                // worth showing (`*args, **kwargs`).
                if m.py.is_some() && d.doc.is_some() {
                    if let Some(stub) = &m.stub {
                        if let Some(s) = lookup(&stub.defs, path) {
                            if only_variadic(&d.params) && !s.params.is_empty() {
                                d.params = s.params.clone();
                                d.returns = d.returns.or(s.returns.clone());
                            }
                        }
                    }
                }
                if d.doc.is_none() {
                    if let Some(stub) = &m.stub {
                        if let Some(s) = lookup(&stub.defs, path) {
                            d.doc = s.doc.clone();
                        }
                    }
                }
                return Some(d);
            }
        }
        None
    }

    fn module_doc(&mut self, module: &str) -> Option<String> {
        let m = self.module(module)?;
        m.py.as_ref().and_then(|o| o.doc.clone()).or_else(|| m.stub.as_ref().and_then(|o| o.doc.clone()))
    }

    /// `name.__doc__ = "…"` in `module`.
    fn assigned_doc(&mut self, module: &str, name: &str) -> Option<String> {
        let m = self.module(module)?;
        m.py.iter().flat_map(|o| o.doc_assigns.iter()).find(|(n, _)| n == name).map(|(_, d)| d.clone())
    }

    /// The documentation of a definition: its docstring, or one given by
    /// `add_newdoc`, `name.__doc__ =`, or copied by a decorator
    /// (`@_copy_docstring_and_deprecators(Axes.plot)`, `@functools.wraps`).
    fn doc_of(&mut self, module: &str, path: &[String], def: &Def, depth: usize) -> String {
        if let Some(d) = &def.doc {
            return d.clone();
        }
        if path.len() == 1 {
            if let Some(d) = self.assigned_doc(module, &path[0]) {
                return d;
            }
        }
        let (name, attr) = match path {
            [n] => (n.clone(), String::new()),
            [.., c, a] => (c.clone(), a.clone()),
            _ => (String::new(), String::new()),
        };
        // add_newdoc names the module where the C object is defined; any
        // module of the package with that name and attribute counts.
        if let Some(d) = self.newdocs.iter().find(|((_, n, a), _)| *n == name && *a == attr).map(|(_, d)| d.clone()) {
            return d;
        }
        if depth < 4 {
            for toks in &def.decorator_toks {
                let callee: String = toks.iter().take_while(|t| !t.is_op("(")).map(Tok::text).collect();
                if !(callee.ends_with("copy_docstring_and_deprecators")
                    || callee.ends_with("wraps")
                    || callee.ends_with("_copy_docstring")
                    || callee.ends_with("docstring.copy"))
                {
                    continue;
                }
                let Some(open) = toks.iter().position(|t| t.is_op("(")) else { continue };
                let close = lexer::closing(toks, open);
                let arg = lexer::split_commas(&toks[open + 1..close.min(toks.len())])
                    .first()
                    .map(|a| a.to_vec())
                    .unwrap_or_default();
                if let Some(Target::Def(m, p)) = self.resolve_expr(module, &arg, 0) {
                    if let Some(d) = self.def(&m, &p) {
                        return self.doc_of(&m, &p, &d, depth + 1);
                    }
                }
            }
        }
        String::new()
    }
}

/// The strings at the start of an expression: `"""…""" % params` → the
/// text (its `%(…)s` placeholders are filled later).
fn leading_strings(value: &[Tok]) -> Option<String> {
    let n = value.iter().take_while(|t| matches!(t, Tok::Str(_))).count();
    if n == 0 || (n < value.len() && !value[n].is_op("%")) {
        return None;
    }
    lexer::strings_value(&value[..n])
}

fn dotted_tokens(text: &str) -> Vec<Tok> {
    lexer::lines(&format!("{text}\n")).into_iter().next().map(|l| l.toks).unwrap_or_default()
}

/// The definition named `name`: the implementation rather than `@overload`
/// stubs, the getter rather than `@x.setter`.
fn pick<'a>(defs: &'a [Def], name: &str) -> Option<&'a Def> {
    let all: Vec<&Def> = defs.iter().filter(|d| d.name == name).collect();
    all.iter()
        .rev()
        .find(|d| !d.overload && !d.decorators.iter().any(|x| x.ends_with(".setter") || x.ends_with(".deleter")))
        .or_else(|| {
            all.iter().find(|d| !d.decorators.iter().any(|x| x.ends_with(".setter") || x.ends_with(".deleter")))
        })
        .or(all.first())
        .copied()
}

fn lookup<'a>(defs: &'a [Def], path: &[String]) -> Option<&'a Def> {
    let mut d = pick(defs, &path[0])?;
    for name in &path[1..] {
        d = pick(&d.members, name)?;
    }
    Some(d)
}

fn only_variadic(params: &[Param]) -> bool {
    params.iter().all(|p| p.name.starts_with('*') || p.name == "self" || p.name == "cls" || p.name == "/")
}

fn is_property(def: &Def) -> bool {
    def.decorators.iter().any(|d| PROPERTY_DECORATORS.contains(&d.as_str()) || d.ends_with(".cache_readonly"))
}

fn is_deprecated(def: &Def) -> bool {
    def.decorators.iter().any(|d| d.split('(').next().is_some_and(|c| c.ends_with("deprecated")))
}

/// matplotlib's `%(name)s` placeholders, filled at import time: with the
/// registered texts (`interpd.register`), `%(Line2D:kwdoc)s` with a
/// pointer to the class whose properties it lists, others with "…".
fn fill_placeholders(doc: &str, texts: &HashMap<String, String>) -> String {
    if !doc.contains("%(") {
        return doc.to_owned();
    }
    let re = regex::Regex::new(r"%\(([A-Za-z0-9_.]+)(:kwdoc)?\)s").expect("valid regex");
    let mut text = doc.to_owned();
    for _ in 0..3 {
        if !text.contains("%(") {
            break;
        }
        text = re
            .replace_all(&text, |c: &regex::Captures| {
                let name = &c[1];
                if c.get(2).is_some() {
                    return format!("(Eigenschaften von `{name}`, siehe dort)");
                }
                match texts.get(name) {
                    // Indented like the placeholder (it stands on a line of its own).
                    Some(t) => t.clone(),
                    None => "…".to_owned(),
                }
            })
            .into_owned();
    }
    text.replace("%%", "%").replace("%…", "…")
}

// ------------------------------------------------------------ describing

/// The entry for a definition (`kind`: module, function, method, class,
/// exception, property).
fn describe(qual: &str, kind: &str, def: &Def, doc: &str) -> Entry {
    describe_with(qual, kind, def, doc, &HashMap::new())
}

fn describe_with(qual: &str, kind: &str, def: &Def, doc: &str, texts: &HashMap<String, String>) -> Entry {
    let short = qual.rsplit('.').next().unwrap_or(qual);
    let mut doc = numpydoc::clean(doc);
    if doc.contains("%(") {
        doc = indent_fill(&doc, texts);
    }
    if doc.len() > MAX_DOC {
        let mut cut = MAX_DOC;
        while !doc.is_char_boundary(cut) {
            cut -= 1;
        }
        doc.truncate(cut);
        doc.push_str("\n\n…");
    }
    let sections = numpydoc::split_sections(&doc);
    let mut head = sections.head.clone();
    let mut entry = serde_json::Map::new();
    entry.insert("id".into(), json!(qual));
    entry.insert("k".into(), json!(kind));
    // Parameters of the signature (without self/cls of methods).
    let mut params: Vec<Param> = def.params.clone();
    let class_like = kind == "class" || kind == "exception";
    let drop_first = (kind == "method" || class_like || kind == "property")
        && !def.decorators.iter().any(|d| d == "staticmethod")
        && params.first().is_some_and(|p| !p.name.starts_with('*') && p.name != "/");
    if drop_first {
        params.remove(0);
        // `(self, /, loc=0.0)` → `(loc=0.0)`
        if params.first().is_some_and(|p| p.name == "/") {
            params.remove(0);
        }
    }
    let mut sig = String::new();
    // C functions: the signature is the first line of their docstring,
    // in C modules followed by a line "--" (`$self, /` for methods).
    if let Some((first, rest)) = head.split_once("\n--\n") {
        if numpydoc::looks_like_signature(&first.replace('\n', " ")) {
            head = format!(
                "{}\n\n{}",
                first.replace("$self, ", "").replace("$self", "").replace("$module, ", ""),
                rest.trim_start()
            );
        }
    }
    let first_para = head.split("\n\n").next().unwrap_or_default().replace('\n', " ");
    let doc_sig =
        numpydoc::looks_like_signature(&first_para) && first_para.trim_start().starts_with(&format!("{short}("));
    if doc_sig {
        if def.params.is_empty() || def.stub || only_variadic(&def.params) {
            let t = first_para.trim();
            let collapsed: String = t[short.len()..].split_whitespace().collect::<Vec<_>>().join(" ");
            sig = collapsed.replace("( ", "(").replace(" )", ")").replace(" ,", ",");
            params = signature_params(&sig);
        }
        head = head.split_once("\n\n").map(|(_, rest)| rest.trim().to_owned()).unwrap_or_default();
    }
    let wants_sig = match kind {
        "property" | "module" => false,
        _ if class_like => !params.is_empty(),
        _ => true,
    };
    if sig.is_empty() && wants_sig {
        sig = format!("({})", params.iter().map(Param::text).collect::<Vec<_>>().join(", "));
        if let Some(r) = &def.returns {
            if !class_like {
                sig.push_str(" -> ");
                sig.push_str(r);
            }
        }
    }
    if sig.len() > 1500 {
        let mut cut = 1500;
        while !sig.is_char_boundary(cut) {
            cut -= 1;
        }
        sig.truncate(cut);
        sig.push_str(" …)");
    }
    entry.insert("sum".into(), json!(numpydoc::summary(&head)));
    entry.insert("sig".into(), json!(sig));
    // Parameters: signature order; types and descriptions from the text.
    let mut documented: Vec<(String, String, String, bool)> = Vec::new();
    for (key, other) in [("params", false), ("other", true)] {
        for f in numpydoc::parse_fields(&sections.get(key), false) {
            for name in f.name.split(',').map(str::trim).filter(|n| !n.is_empty()) {
                documented.push((name.to_owned(), f.ty.clone(), f.desc.clone(), other));
            }
        }
    }
    let mut used = HashSet::new();
    let mut list = Vec::new();
    let mut keyword_only = false;
    for p in &params {
        if p.name == "*" {
            keyword_only = true;
            continue;
        }
        if p.name == "/" {
            continue;
        }
        let bare = p.name.trim_start_matches('*');
        if p.name.starts_with('*') && !p.name.starts_with("**") {
            keyword_only = true;
        }
        let d = documented.iter().find(|d| d.0.trim_start_matches('*') == bare);
        if d.is_some() {
            used.insert(bare.to_owned());
        }
        let mut item = serde_json::Map::new();
        item.insert("n".into(), json!(p.name));
        let ty = d
            .map(|d| d.1.clone())
            .filter(|t| !t.is_empty())
            .or_else(|| p.ann.clone().map(|a| a.trim_matches(['"', '\'']).to_owned()));
        if let Some(t) = ty.filter(|t| !t.is_empty()) {
            item.insert("t".into(), json!(t));
        }
        if let Some(def) = &p.default {
            item.insert("d".into(), json!(def));
        }
        if let Some(d) = d.filter(|d| !d.2.is_empty()) {
            item.insert("desc".into(), json!(d.2));
        }
        if keyword_only && !p.name.starts_with('*') {
            item.insert("kw".into(), json!(true));
        }
        list.push(Value::Object(item));
    }
    for (name, ty, desc, other) in &documented {
        if used.contains(name.trim_start_matches('*'))
            || params.iter().any(|p| p.name.trim_start_matches('*') == name.trim_start_matches('*'))
        {
            continue;
        }
        let mut item = serde_json::Map::new();
        item.insert("n".into(), json!(name));
        item.insert("doc".into(), json!(true));
        if !ty.is_empty() {
            item.insert("t".into(), json!(ty));
        }
        if !desc.is_empty() {
            item.insert("desc".into(), json!(desc));
        }
        if *other {
            item.insert("other".into(), json!(true));
        }
        list.push(Value::Object(item));
    }
    if !list.is_empty() {
        entry.insert("p".into(), Value::Array(list));
    }
    // Returns: from the text, else the annotation; a class gives itself.
    let ret_ann = def.returns.clone().map(|r| r.trim_matches(['"', '\'']).to_owned()).filter(|r| r != "None");
    let mut rets: Vec<Value> =
        numpydoc::parse_fields(&[sections.get("returns"), sections.get("yields")].concat(), true)
            .into_iter()
            .map(|f| {
                let mut m = serde_json::Map::new();
                if !f.name.is_empty() {
                    m.insert("n".into(), json!(f.name));
                }
                if !f.ty.is_empty() {
                    m.insert("t".into(), json!(f.ty));
                }
                if !f.desc.is_empty() {
                    m.insert("desc".into(), json!(f.desc));
                }
                Value::Object(m)
            })
            .collect();
    if class_like {
        rets = vec![json!({ "t": short })];
    } else if rets.is_empty() {
        if let Some(r) = &ret_ann {
            rets.push(json!({ "t": r }));
        }
    } else if let (Some(r), Some(first)) = (&ret_ann, rets.first_mut()) {
        if first.get("t").is_none() {
            first["t"] = json!(r);
        }
    }
    if !rets.is_empty() {
        entry.insert("r".into(), Value::Array(rets));
    }
    if let Some(r) = ret_ann.filter(|_| !class_like) {
        entry.insert("ra".into(), json!(r));
    }
    // The rest of the text, sections in their order.
    let mut rest = Vec::new();
    if !head.is_empty() {
        rest.push(head.clone());
    }
    for (key, lines) in &sections.sections {
        if matches!(key.as_str(), "params" | "other" | "returns" | "yields") {
            continue;
        }
        let text = lines.join("\n");
        if !text.trim().is_empty() {
            rest.push(format!("§{key}\n{}", text.trim_matches('\n')));
        }
    }
    let text = rest.join("\n\n");
    if !text.is_empty() {
        entry.insert("doc".into(), json!(text));
    }
    let see: Vec<String> = sections
        .get("see")
        .iter()
        .filter(|l| !l.starts_with(' ') && !l.trim().is_empty())
        .filter_map(|l| {
            let n: String = l.chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '.').collect();
            (!n.is_empty()).then_some(n)
        })
        .take(30)
        .collect();
    if !see.is_empty() {
        entry.insert("see".into(), json!(see));
    }
    let summary = entry["sum"].as_str().unwrap_or_default().to_lowercase();
    if is_deprecated(def)
        || head.lines().any(|l| l.trim_start().starts_with(".. deprecated::"))
        || summary.starts_with("deprecated")
        || summary.starts_with("[deprecated")
    {
        entry.insert("dep".into(), json!(true));
    }
    Value::Object(entry)
}

/// Fills placeholders that stand on a line of their own with the text
/// indented like them, the rest inline.
fn indent_fill(doc: &str, texts: &HashMap<String, String>) -> String {
    doc.lines()
        .map(|line| {
            let t = line.trim();
            if t.starts_with("%(") && t.ends_with(")s") && !t[2..].contains("%(") {
                let indent = &line[..line.len() - line.trim_start().len()];
                fill_placeholders(t, texts)
                    .lines()
                    .map(|l| if l.is_empty() { String::new() } else { format!("{indent}{l}") })
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                fill_placeholders(line, texts)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Parameters of a signature text `(a, b=1, *, c)`.
fn signature_params(sig: &str) -> Vec<Param> {
    let sig = sig.split(" -> ").next().unwrap_or(sig);
    outline::parse(&format!("def f{sig}: ...\n")).defs.into_iter().next().map(|d| d.params).unwrap_or_default()
}

// --------------------------------------------------------------- walking

/// The entries of `pkg`, read from `dir` (its folder): modules, functions,
/// classes with their members. Returns the entries and problems (for the
/// log).
pub fn entries(pkg: &mut Package) -> Vec<Entry> {
    let name = pkg.name.clone();
    let mut modules: Vec<String> = vec![name.clone()];
    for sub in extra_modules(&name) {
        let m = format!("{name}.{sub}");
        if pkg.exists(&m) {
            modules.push(m);
        }
    }
    if extra_modules(&name).is_empty() {
        // Public submodules one level down, for packages not listed above.
        let dir = pkg.root.join(&name);
        let mut subs: Vec<String> = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let n = e.file_name().to_string_lossy().into_owned();
                let stem = n.strip_suffix(".py").unwrap_or(&n).to_owned();
                let ok = (e.path().is_dir() && e.path().join("__init__.py").is_file()) || n.ends_with(".py");
                (ok && !stem.starts_with('_') && !SKIP_TOP.contains(&stem.as_str())).then_some(stem)
            })
            .collect();
        subs.sort();
        modules.extend(subs.into_iter().map(|s| format!("{name}.{s}")));
    }
    let primary: HashSet<String> = modules.iter().cloned().collect();

    // Pass 1: every public path to every definition (breadth first).
    let mut order: Vec<Target> = Vec::new();
    let mut paths: HashMap<Target, Vec<String>> = HashMap::new();
    let mut queue: VecDeque<String> = modules.iter().cloned().collect();
    let mut queued: HashSet<String> = modules.iter().cloned().collect();
    while let Some(module) = queue.pop_front() {
        let target = Target::Module(module.clone());
        if !paths.contains_key(&target) {
            order.push(target.clone());
            paths.insert(target, vec![module.clone()]);
        }
        let mut names = pkg.public_names(&module, &mut HashSet::new());
        names.sort();
        for n in names {
            if n.starts_with('_') {
                continue;
            }
            let Some(t) = pkg.resolve(&module, &n, 0) else { continue };
            match &t {
                Target::Module(m) => {
                    let parts: Vec<&str> = m.split('.').collect();
                    if pkg.in_package(m)
                        && !queued.contains(m)
                        && parts.len() <= 3
                        && !parts.iter().any(|p| p.starts_with('_') || SKIP_PARTS.contains(p))
                        && pkg.exists(m)
                    {
                        queued.insert(m.clone());
                        queue.push_back(m.clone());
                    }
                }
                Target::Def(m, _) | Target::Annotated(m, _) => {
                    if !pkg.in_package(m) {
                        continue;
                    }
                    if let Target::Annotated(_, ann) = &t {
                        // Only ufuncs and functions declared as values.
                        if !ann.to_lowercase().contains("ufunc") {
                            continue;
                        }
                    }
                    if !paths.contains_key(&t) {
                        order.push(t.clone());
                    }
                    paths.entry(t.clone()).or_default().push(format!("{module}.{n}"));
                }
            }
        }
    }

    // The name of each definition: at the top of the package, else in the
    // module closest to where it is defined, else in a module the user
    // imports, else the shortest.
    let best = |home: &str, list: &[String]| -> String {
        list.iter()
            .min_by_key(|path| {
                let module = path.rsplit_once('.').map(|(m, _)| m).unwrap_or(path);
                let common = module.split('.').zip(home.split('.')).take_while(|(a, b)| a == b).count();
                (
                    module != name,
                    std::cmp::Reverse(common),
                    !primary.contains(module),
                    path.matches('.').count(),
                    path.len(),
                )
            })
            .cloned()
            .unwrap_or_default()
    };
    let mut named: HashMap<Target, String> = HashMap::new();
    for t in &order {
        let list = &paths[t];
        let n = match t {
            Target::Module(m) => m.clone(),
            Target::Def(home, _) | Target::Annotated(home, _) => best(home, list),
        };
        named.insert(t.clone(), n);
    }
    // Classes with a public name: their members are listed there.
    let indexed: HashMap<(String, Vec<String>), String> = named
        .iter()
        .filter_map(|(t, n)| match t {
            Target::Def(m, p) => Some(((m.clone(), p.clone()), n.clone())),
            _ => None,
        })
        .collect();

    // Pass 2: describe them, classes with their members.
    let mut out = Vec::new();
    for t in &order {
        if out.len() >= MAX_ENTRIES {
            break;
        }
        let qual = named[t].clone();
        match t {
            Target::Module(m) => {
                let doc = pkg.module_doc(m).unwrap_or_default();
                out.push(describe(&qual, "module", &Def::default(), &doc));
            }
            Target::Annotated(m, ann) => {
                // A ufunc: its text is in the compiled module (filled in
                // by fill_from_compiled), the signature follows from the
                // number of inputs and outputs (`_UFunc_Nin2_Nout1`).
                let short = qual.rsplit('.').next().unwrap_or(&qual).to_owned();
                let doc = pkg.newdocs.iter().find(|((_, n, a), _)| *n == short && a.is_empty()).map(|(_, d)| d.clone());
                let def = ufunc_def(ann);
                let e = describe(&qual, "function", &def, &doc.unwrap_or_default());
                push(pkg, &mut out, e, m, &[short], "function", &def);
            }
            Target::Def(m, path) => {
                let Some(def) = pkg.def(m, path) else { continue };
                let doc = pkg.doc_of(m, path, &def, 0);
                if def.class {
                    let mut class_def = def.clone();
                    // The signature of a class is that of __init__ (or __new__).
                    if let Some(init) =
                        find_member(pkg, m, path, "__init__").or_else(|| find_member(pkg, m, path, "__new__"))
                    {
                        class_def.params = init.params;
                    }
                    let kind = if is_exception(pkg, m, &def, 0) { "exception" } else { "class" };
                    let e = describe_with(&qual, kind, &class_def, &doc, &pkg.interpd);
                    push(pkg, &mut out, e, m, path, kind, &class_def);
                    members(pkg, &qual, m, path, &indexed, &mut out, 0);
                } else {
                    let mut def = def;
                    // A bound method exported as a function (numpy.random.beta).
                    if path.len() > 1 && def.params.first().is_some_and(|p| p.name == "self" || p.name == "cls") {
                        def.params.remove(0);
                    }
                    let e = describe_with(&qual, "function", &def, &doc, &pkg.interpd);
                    push(pkg, &mut out, e, m, path, "function", &def);
                }
            }
        }
    }
    fill_from_compiled(pkg, &mut out);
    out
}

/// The signature of a ufunc from its stub type (`_UFunc_Nin2_Nout1[…]`).
fn ufunc_def(ann: &str) -> Def {
    let num =
        |key: &str| ann.split(key).nth(1).and_then(|r| r.chars().next()).and_then(|c| c.to_digit(10)).unwrap_or(1);
    let (nin, nout) = (num("Nin"), num("Nout"));
    let mut sig =
        if nin == 1 { "x".to_owned() } else { (1..=nin).map(|i| format!("x{i}")).collect::<Vec<_>>().join(", ") };
    if nout > 1 {
        for i in 1..=nout {
            sig.push_str(&format!(", out{i}=None"));
        }
        sig.push_str(", /");
    } else {
        sig.push_str(", /, out=None");
    }
    sig.push_str(", *, where=True, casting='same_kind', order='K', dtype=None, subok=True, signature=None");
    Def { params: signature_params(&format!("({sig})")), returns: None, ..Def::default() }
}

fn is_exception(pkg: &mut Package, module: &str, def: &Def, depth: usize) -> bool {
    if depth > 8 {
        return false;
    }
    for base in &def.bases {
        let b = base.split('[').next().unwrap_or(base).trim();
        let last = b.rsplit('.').next().unwrap_or(b);
        if last.ends_with("Error")
            || last.ends_with("Exception")
            || last.ends_with("Warning")
            || last == "BaseException"
        {
            return true;
        }
        if let Some(Target::Def(m, p)) = pkg.resolve_expr(module, &dotted_tokens(b), 0) {
            if let Some(d) = pkg.def(&m, &p) {
                if is_exception(pkg, &m, &d, depth + 1) {
                    return true;
                }
            }
        }
    }
    false
}

/// The classes of `path` in `module`, then its bases (depth first, left
/// to right), as definitions in the package.
fn mro(pkg: &mut Package, module: &str, path: &[String]) -> Vec<(String, Vec<String>, Def)> {
    let mut out: Vec<(String, Vec<String>, Def)> = Vec::new();
    let mut stack = vec![(module.to_owned(), path.to_vec())];
    let mut seen = HashSet::new();
    while let Some((m, p)) = stack.pop() {
        if !seen.insert((m.clone(), p.clone())) || out.len() > 40 {
            continue;
        }
        let Some(def) = pkg.def(&m, &p) else { continue };
        let mut bases = Vec::new();
        for base in &def.bases {
            let b = base.split('[').next().unwrap_or(base).trim().to_owned();
            if let Some(Target::Def(bm, bp)) = pkg.resolve_expr(&m, &dotted_tokens(&b), 0) {
                bases.push((bm, bp));
            }
        }
        out.push((m, p, def));
        for b in bases.into_iter().rev() {
            stack.push(b);
        }
    }
    out
}

/// Adds `entry`; one without text is remembered for [`fill_from_compiled`].
fn push(pkg: &mut Package, out: &mut Vec<Entry>, entry: Entry, module: &str, path: &[String], kind: &str, def: &Def) {
    if entry.get("doc").is_none() && entry["sum"].as_str().is_none_or(str::is_empty) && kind != "module" {
        let qual = entry["id"].as_str().unwrap_or_default().to_owned();
        pkg.undocumented.push(Undocumented {
            index: out.len(),
            module: module.to_owned(),
            name: path.last().cloned().unwrap_or_else(|| qual.rsplit('.').next().unwrap_or_default().to_owned()),
            class: (path.len() > 1).then(|| path[path.len() - 2].clone()),
            kind: kind.to_owned(),
            qual,
            def: def.clone(),
        });
    }
    out.push(entry);
}

/// Strings in a compiled module (`.so`, `.pyd`) that look like
/// docstrings: Cython and C keep them as plain text.
fn docstrings_in(file: &Path) -> Vec<String> {
    let Ok(data) = std::fs::read(file) else { return Vec::new() };
    let mut out = Vec::new();
    let mut start = None;
    for (i, &b) in data.iter().enumerate() {
        let text = b == b'\t' || b == b'\n' || b == b'\r' || (0x20..0x7f).contains(&b) || b >= 0x80;
        match (text, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                if i - s >= 60 {
                    let chunk = String::from_utf8_lossy(&data[s..i]);
                    if chunk.contains('\n')
                        && ["Parameters\n", "Returns\n", "Examples\n", "-----", ">>>"].iter().any(|k| chunk.contains(k))
                        && !chunk.contains('\u{fffd}')
                    {
                        out.push(chunk.into_owned());
                    }
                }
                start = None;
            }
            _ => {}
        }
    }
    out
}

/// Calls in the examples of a docstring, by name (`>>> np.add(…)` → add).
fn example_counts(doc: &str) -> HashMap<String, usize> {
    let mut out = HashMap::new();
    for line in doc.lines().map(str::trim_start).filter(|l| l.starts_with(">>>") || l.starts_with("...")) {
        let b = line.as_bytes();
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'_' || b[i].is_ascii_alphabetic() {
                let start = i;
                while i < b.len() && (b[i] == b'_' || b[i].is_ascii_alphanumeric()) {
                    i += 1;
                }
                if b.get(i) == Some(&b'(') {
                    *out.entry(line[start..i].to_owned()).or_insert(0) += 1;
                }
            } else {
                i += 1;
            }
        }
    }
    out
}

struct Compiled {
    doc: String,
    first_line: String,
    calls: HashMap<String, usize>,
}

/// Gives entries without text the docstrings of compiled modules: one
/// starting with their signature (`dirichlet(alpha, size=None)`), else
/// one whose examples call them clearly more often than anything else
/// that is missing. The module's own file first, then its folder, then
/// the rest of the package.
fn fill_from_compiled(pkg: &mut Package, out: &mut [Entry]) {
    let missing = std::mem::take(&mut pkg.undocumented);
    if missing.is_empty() {
        return;
    }
    let mut files: Vec<PathBuf> = Vec::new();
    let mut stack = vec![pkg.root.join(&pkg.name)];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            let n = e.file_name().to_string_lossy().into_owned();
            if p.is_dir() {
                if n != "tests" && !n.starts_with('.') {
                    stack.push(p);
                }
            } else if (n.ends_with(".so") || n.ends_with(".pyd")) && !n.contains("_tests") {
                files.push(p);
            }
        }
    }
    files.sort();
    if files.is_empty() {
        return;
    }
    let docs: HashMap<PathBuf, Vec<Compiled>> = files
        .iter()
        .map(|f| {
            let list = docstrings_in(f)
                .into_iter()
                .map(|doc| Compiled {
                    first_line: doc.trim_start().lines().next().unwrap_or_default().to_owned(),
                    calls: example_counts(&doc),
                    doc,
                })
                .collect();
            (f.clone(), list)
        })
        .collect();
    // Rivals of a name in the examples: the other missing ufuncs, or the
    // other missing members of the same class.
    let is_ufunc = |u: &Undocumented| u.def.params.iter().any(|p| p.name == "signature") && u.class.is_none();
    let group = |u: &Undocumented| {
        if is_ufunc(u) { "ufunc".to_owned() } else { format!("{}.{}", u.module, u.class.clone().unwrap_or_default()) }
    };
    let mut names_of: HashMap<String, HashSet<String>> = HashMap::new();
    for u in &missing {
        names_of.entry(group(u)).or_default().insert(u.name.clone());
    }
    let mut assigned: HashMap<usize, String> = HashMap::new();
    // Candidates of the example rule: (missing, file, docstring), best first.
    let mut candidates: Vec<(bool, usize, usize, PathBuf, usize)> = Vec::new();
    for (ui, u) in missing.iter().enumerate() {
        let rel: PathBuf = u.module.split('.').collect();
        let base = pkg.root.join(&rel);
        let dir = if base.is_dir() { base.clone() } else { base.parent().map(Path::to_path_buf).unwrap_or_default() };
        let stem = u.module.rsplit('.').next().unwrap_or_default().to_owned();
        let rank = |f: &PathBuf| {
            let own_dir = f.parent() == Some(dir.as_path());
            let own = own_dir && f.file_name().is_some_and(|n| n.to_string_lossy().starts_with(&format!("{stem}.")));
            (!own, !own_dir)
        };
        let mut ordered: Vec<&PathBuf> = files.iter().collect();
        ordered.sort_by_key(|f| rank(f));
        let names: Vec<String> = [u.class.as_ref().map(|c| format!("{c}.{}(", u.name)), Some(format!("{}(", u.name))]
            .into_iter()
            .flatten()
            .collect();
        // 1. A docstring starting with its signature.
        if let Some(c) = ordered
            .iter()
            .flat_map(|f| docs[*f].iter())
            .find(|c| names.iter().any(|n| c.first_line.starts_with(n.as_str())))
        {
            assigned.insert(ui, c.doc.clone());
            continue;
        }
        // 2. A class: the docstring in its own file that documents the
        //    parameters of its constructor (from the stub).
        if u.kind == "class" || u.kind == "exception" {
            let wanted: HashSet<String> = u
                .def
                .params
                .iter()
                .map(|p| p.name.trim_start_matches('*').to_owned())
                .filter(|n| !matches!(n.as_str(), "self" | "cls" | "/" | "" | "args" | "kwargs"))
                .collect();
            if wanted.len() < 2 {
                continue;
            }
            let mut best: Option<(usize, &Compiled)> = None;
            let mut tie = false;
            for file in ordered.iter().filter(|f| rank(f).1 == false) {
                for c in &docs[*file] {
                    let sections = numpydoc::split_sections(&numpydoc::clean(&c.doc));
                    let documented: HashSet<String> = numpydoc::parse_fields(&sections.get("params"), false)
                        .into_iter()
                        .flat_map(|f| {
                            f.name.split(',').map(|n| n.trim().trim_start_matches('*').to_owned()).collect::<Vec<_>>()
                        })
                        .collect();
                    let hits = documented.intersection(&wanted).count();
                    // Most of what it documents are constructor parameters …
                    if hits < 2 || hits * 2 < documented.len() {
                        continue;
                    }
                    // … and its examples build the class more than they call
                    // anything else (`Timestamp.replace` documents year,
                    // month … too, but calls `replace`).
                    let built = c.calls.get(&u.name).copied().unwrap_or(0);
                    if built == 0 || c.calls.iter().any(|(k, v)| k != &u.name && *v > built) {
                        continue;
                    }
                    match best {
                        Some((b, _)) if hits == b => tie = true,
                        Some((b, _)) if hits < b => {}
                        _ => {
                            best = Some((hits, c));
                            tie = false;
                        }
                    }
                }
            }
            if let (Some((_, c)), false) = (best, tie) {
                assigned.insert(ui, c.doc.clone());
            }
            continue;
        }
        // 3. Examples – only for functions and methods: in the module's own
        //    file; ufuncs in the ufunc modules (`*umath*`, `*ufunc*`).
        let rivals = &names_of[&group(u)];
        for file in ordered {
            let ufunc_file = file.file_name().is_some_and(|n| {
                let n = n.to_string_lossy();
                n.contains("umath") || n.contains("ufunc")
            });
            if !(if is_ufunc(u) { ufunc_file } else { rank(file) == (false, false) }) {
                continue;
            }
            for (di, c) in docs[file].iter().enumerate() {
                let n = c.calls.get(&u.name).copied().unwrap_or(0);
                if n == 0 {
                    continue;
                }
                let rival = c
                    .calls
                    .iter()
                    .filter(|(k, _)| *k != &u.name && rivals.contains(k.as_str()))
                    .map(|(_, v)| *v)
                    .max()
                    .unwrap_or(0);
                if n >= 2 * rival.max(1) || rival == 0 {
                    candidates.push((rival == 0, n, ui, file.clone(), di));
                }
            }
        }
    }
    // Each docstring to one name, each name one docstring: those without
    // rivals in their examples first, then by how often they are called.
    candidates.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    let mut used: HashSet<(PathBuf, usize)> = HashSet::new();
    for (_, _, ui, file, di) in candidates {
        if assigned.contains_key(&ui) || used.contains(&(file.clone(), di)) {
            continue;
        }
        used.insert((file.clone(), di));
        assigned.insert(ui, docs[&file][di].doc.clone());
    }
    for (ui, doc) in assigned {
        let u = &missing[ui];
        let mut e = describe_with(&u.qual, &u.kind, &u.def, &doc, &pkg.interpd);
        if u.kind == "property" {
            e["sig"] = json!("");
            if let Some(o) = e.as_object_mut() {
                o.remove("p");
            }
        }
        if let Some(v) = out[u.index].get("from").cloned() {
            e["from"] = v;
        }
        out[u.index] = e;
    }
}

/// `name` in the class or its bases.
fn find_member(pkg: &mut Package, module: &str, path: &[String], name: &str) -> Option<Def> {
    mro(pkg, module, path).into_iter().find_map(|(_, _, def)| pick(&def.members, name).cloned())
}

/// Members of a class: its own and those inherited from bases without an
/// entry of their own (DataFrame.head from NDFrame), accessors with their
/// methods (Series.str.contains), aliases (`isnull = isna`).
fn members(
    pkg: &mut Package,
    qual: &str,
    module: &str,
    path: &[String],
    indexed: &HashMap<(String, Vec<String>), String>,
    out: &mut Vec<Entry>,
    depth: usize,
) {
    let chain = mro(pkg, module, path);
    let mut done: HashSet<String> = HashSet::new();
    for (i, (m, p, def)) in chain.iter().enumerate() {
        let inherited = i > 0;
        // Bases with an entry of their own list their members there.
        if inherited && indexed.contains_key(&(m.clone(), p.clone())) {
            for d in &def.members {
                done.insert(d.name.clone());
            }
            for (n, _) in &def.assigns {
                done.insert(n.clone());
            }
            continue;
        }
        let owner = indexed.get(&(m.clone(), p.clone())).cloned().unwrap_or_else(|| format!("{m}.{}", p.join(".")));
        let mut names: Vec<String> = def.members.iter().map(|d| d.name.clone()).collect();
        let mut unique = HashSet::new();
        names.retain(|n| unique.insert(n.clone()));
        for name in names {
            if name.starts_with('_') || !done.insert(name.clone()) {
                continue;
            }
            let Some(d) = pick(&def.members, &name).cloned() else { continue };
            if d.class {
                continue;
            }
            let mut mp = p.clone();
            mp.push(name.clone());
            let mut doc = pkg.doc_of(m, &mp, &d, 0);
            if doc.is_empty() {
                // Like inspect.getdoc: the text of the method it overrides.
                for (bm, bp, bdef) in &chain[i + 1..] {
                    if let Some(bd) = pick(&bdef.members, &name) {
                        let mut bmp = bp.clone();
                        bmp.push(name.clone());
                        doc = pkg.doc_of(bm, &bmp, &bd.clone(), 0);
                        if !doc.is_empty() {
                            break;
                        }
                    }
                }
            }
            let kind = if is_property(&d) { "property" } else { "method" };
            let mut e = describe_with(&format!("{qual}.{name}"), kind, &d, &doc, &pkg.interpd);
            if kind == "property" {
                if let Some(o) = e.as_object_mut() {
                    o.insert("sig".into(), json!(""));
                    o.remove("p");
                }
            }
            if inherited {
                e["from"] = json!(owner);
            }
            push(pkg, out, e, m, &mp, kind, &d);
        }
        for (name, value) in def.assigns.clone() {
            if name.starts_with('_') || done.contains(&name) {
                continue;
            }
            let callee: String = value.iter().take_while(|t| !t.is_op("(")).map(Tok::text).collect();
            let last = callee.rsplit('.').next().unwrap_or(&callee).to_owned();
            if ACCESSORS.contains(&last.as_str()) && depth == 0 {
                // `str = Accessor("str", StringMethods)`
                let Some(open) = value.iter().position(|t| t.is_op("(")) else { continue };
                let close = lexer::closing(&value, open);
                let args = lexer::split_commas(&value[open + 1..close.min(value.len())]);
                let Some(cls) = args.get(1).map(|a| a.to_vec()) else { continue };
                if let Some(Target::Def(am, ap)) = pkg.resolve_expr(m, &cls, 0) {
                    if let Some(adef) = pkg.def(&am, &ap) {
                        done.insert(name.clone());
                        let doc = pkg.doc_of(&am, &ap, &adef, 0);
                        let mut e = describe(&format!("{qual}.{name}"), "property", &Def::default(), &doc);
                        if let Some(o) = e.as_object_mut() {
                            o.insert("sig".into(), json!(""));
                            o.remove("r");
                        }
                        out.push(e);
                        members(pkg, &format!("{qual}.{name}"), &am, &ap, indexed, out, depth + 1);
                    }
                }
                continue;
            }
            // `isnull = isna`: an alias of a method of this class.
            if value.len() == 1 {
                if let Some(target) = value[0].name() {
                    if let Some(d) = pick(&def.members, target).cloned() {
                        done.insert(name.clone());
                        let mut mp = p.clone();
                        mp.push(target.to_owned());
                        let doc = pkg.doc_of(m, &mp, &d, 0);
                        let kind = if is_property(&d) { "property" } else { "method" };
                        let mut e = describe_with(&format!("{qual}.{name}"), kind, &d, &doc, &pkg.interpd);
                        if inherited {
                            e["from"] = json!(owner);
                        }
                        out.push(e);
                    }
                    continue;
                }
            }
            // `stackplot = _preprocess_data()(_make_axes_method(mstack.stackplot))`:
            // a function of another module used as a method.
            if let Some((fm, fp, fdef)) = wrapped_function(pkg, m, &value) {
                done.insert(name.clone());
                let doc = pkg.doc_of(&fm, &fp, &fdef, 0);
                let mut e = describe_with(&format!("{qual}.{name}"), "method", &fdef, &doc, &pkg.interpd);
                if inherited {
                    e["from"] = json!(owner);
                }
                out.push(e);
                continue;
            }
            // `year = _field_accessor("year", "Y", "The year of the datetime.")`
            if last.to_lowercase().ends_with("accessor") || last.ends_with("_property") {
                let doc = value
                    .iter()
                    .filter(|t| matches!(t, Tok::Str(_)))
                    .map(|t| lexer::string_value(t.text()))
                    .filter(|s| s.contains(' '))
                    .last();
                if let Some(doc) = doc {
                    done.insert(name.clone());
                    let mut e = describe(&format!("{qual}.{name}"), "property", &Def::default(), &doc);
                    e["sig"] = json!("");
                    if inherited {
                        e["from"] = json!(owner);
                    }
                    out.push(e);
                }
            }
        }
    }
}

/// The innermost dotted name in `value` that is passed (not called) and
/// names a function of the package.
fn wrapped_function(pkg: &mut Package, module: &str, value: &[Tok]) -> Option<(String, Vec<String>, Def)> {
    let mut candidates = Vec::new();
    let mut i = 0;
    while i < value.len() {
        if value[i].name().is_some() && !(i > 0 && value[i - 1].is_op(".")) {
            let start = i;
            while value.get(i + 1).is_some_and(|x| x.is_op(".")) && value.get(i + 2).and_then(Tok::name).is_some() {
                i += 2;
            }
            let called = value.get(i + 1).is_some_and(|x| x.is_op("("));
            if !called {
                candidates.push(value[start..=i].to_vec());
            }
        }
        i += 1;
    }
    for c in candidates.into_iter().rev() {
        if let Some(Target::Def(m, p)) = pkg.resolve_expr(module, &c, 0) {
            if let Some(d) = pkg.def(&m, &p) {
                if !d.class {
                    return Some((m, p, d));
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, text: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    /// A package laid out like pandas/numpy/matplotlib in small.
    fn demo(root: &Path) {
        write(
            root,
            "demo/__init__.py",
            "\"\"\"Demo package.\"\"\"\nfrom demo.core.api import *\nfrom demo import plotting\nfrom ._core.multiarray import array\nfrom ._core.umath import add\nfrom demo.axes import Axes\n__version__ = '1.0'\n",
        );
        write(root, "demo/core/__init__.py", "");
        write(
            root,
            "demo/core/api.py",
            "from demo.core.frame import Frame\nfrom .tools import merge as merge\n__all__ = ['Frame', 'merge']\n",
        );
        write(
            root,
            "demo/core/generic.py",
            r#"
class NDBase:
    def head(self, n: int = 5) -> "Self":
        """Return the first `n` rows.

        Parameters
        ----------
        n : int, default 5
            Number of rows.

        Returns
        -------
        same type as caller
        """
"#,
        );
        write(
            root,
            "demo/core/frame.py",
            r#"
from demo.core.generic import NDBase
from demo.core.strings import StringMethods
from demo.core.accessor import Accessor

class Frame(NDBase):
    """Two-dimensional table.

    Parameters
    ----------
    data : dict
        The data.
    """
    str = Accessor("str", StringMethods)
    isnull = isna

    def __init__(self, data=None, *, copy: bool = False):
        pass

    @property
    def shape(self) -> tuple[int, int]:
        """Return a tuple of the dimensions."""

    @overload
    def merge(self, right: int) -> int: ...
    def merge(self, right: "Frame", how: str = "inner", *, on=None) -> "Frame":
        """Merge with another frame.

        Parameters
        ----------
        right : Frame
            The other one.
        how : {'left', 'inner'}, default 'inner'
            Type of merge.
        **kwargs
            Passed on.

        Returns
        -------
        Frame
            The merged frame.
        """

    def isna(self) -> "Frame":
        """Detect missing values."""

    def _private(self):
        """Not listed."""
"#,
        );
        write(
            root,
            "demo/core/strings.py",
            "class StringMethods:\n    \"\"\"String functions.\"\"\"\n    def contains(self, pat: str, case: bool = True):\n        \"\"\"Test if pattern is contained.\"\"\"\n",
        );
        write(root, "demo/core/accessor.py", "class Accessor:\n    pass\n");
        write(
            root,
            "demo/core/tools.py",
            "def merge(left, right, how='inner'):\n    \"\"\"Merge two frames.\n\n    Returns\n    -------\n    Frame\n    \"\"\"\n",
        );
        write(
            root,
            "demo/plotting.py",
            "from demo.axes import Axes\n__all__ = ['plot']\n@_copy_docstring_and_deprecators(Axes.plot)\ndef plot(*args, scalex=True, **kwargs):\n    return gca().plot(*args, **kwargs)\n",
        );
        write(
            root,
            "demo/axes.py",
            "import demo.stack as mstack\nclass Axes:\n    def plot(self, *args, **kwargs):\n        \"\"\"Plot y versus x.\n\n        %(Line2D:kwdoc)s\n        \"\"\"\n    stackplot = _preprocess_data()(mstack.stackplot)\n    @_docstring.interpd\n    def quiver(self, *args, **kwargs):\n        \"\"\"%(quiver_doc)s\"\"\"\n\n_quiver_doc = r\"\"\"\nPlot a 2D field of arrows.\n\nParameters\n----------\nX : array\n\"\"\"\n_docstring.interpd.register(quiver_doc=_quiver_doc)\n",
        );
        write(
            root,
            "demo/stack.py",
            "def stackplot(axes, x, *args, labels=()):\n    \"\"\"Draw a stacked area plot.\"\"\"\n",
        );
        write(root, "demo/_core/__init__.py", "");
        write(root, "demo/_core/multiarray.py", "from ._multi import *\n");
        write(
            root,
            "demo/_core/multiarray.pyi",
            "def array(object, dtype=None, *, copy: bool = True) -> ndarray: ...\n",
        );
        write(
            root,
            "demo/_core/_add_newdocs.py",
            "add_newdoc('demo._core.multiarray', 'array',\n    \"\"\"\n    array(object, dtype=None, *, copy=True)\n\n    Create an array.\n\n    Parameters\n    ----------\n    object : array_like\n        Input.\n    \"\"\")\n",
        );
        write(root, "demo/_core/umath.pyi", "add: _UFunc_Nin2_Nout1\n");
        write(root, "demo/tests/test_x.py", "def test_x(): pass\n");
    }

    fn by_id(entries: &[Entry]) -> HashMap<String, Entry> {
        entries.iter().map(|e| (e["id"].as_str().unwrap().to_owned(), e.clone())).collect()
    }

    #[test]
    fn walks_the_public_api_without_running_anything() {
        let dir = tempfile::tempdir().unwrap();
        demo(dir.path());
        let mut pkg = Package::open("demo", &dir.path().join("demo"));
        let entries = entries(&mut pkg);
        let e = by_id(&entries);
        let mut ids: Vec<&String> = e.keys().collect();
        ids.sort();
        // Re-exported at the top under the shortest name.
        assert!(e.contains_key("demo.Frame"), "{ids:?}");
        assert!(!e.contains_key("demo.core.frame.Frame"));
        assert_eq!(e["demo"]["k"], "module");
        // Class: signature of __init__, parameters from the class text.
        let frame = &e["demo.Frame"];
        assert_eq!(frame["k"], "class");
        assert_eq!(frame["sig"], "(data=None, *, copy: bool = False)");
        assert_eq!(frame["p"][0]["desc"], "The data.");
        assert_eq!(frame["p"][1]["kw"], true);
        // Method: implementation over @overload, types from the text.
        let merge = &e["demo.Frame.merge"];
        assert_eq!(merge["k"], "method");
        assert_eq!(merge["sig"], "(right: \"Frame\", how: str = \"inner\", *, on=None) -> \"Frame\"");
        assert_eq!(merge["p"][1]["t"], "{'left', 'inner'}, default 'inner'");
        assert_eq!(merge["p"][3]["n"], "**kwargs");
        assert_eq!(merge["p"][3]["doc"], true);
        assert_eq!(merge["r"][0]["t"], "Frame");
        assert_eq!(merge["sum"], "Merge with another frame.");
        // Property, inherited method of a private base, alias, accessor.
        assert_eq!(e["demo.Frame.shape"]["k"], "property");
        assert_eq!(e["demo.Frame.head"]["from"], "demo.core.generic.NDBase");
        assert_eq!(e["demo.Frame.head"]["r"][0]["t"], "same type as caller");
        assert_eq!(e["demo.Frame.isnull"]["sum"], "Detect missing values.");
        assert_eq!(e["demo.Frame.str"]["k"], "property");
        assert_eq!(e["demo.Frame.str.contains"]["p"][0]["n"], "pat");
        assert!(!e.contains_key("demo.Frame._private"));
        // Module function re-exported with `as`, returns from the text.
        assert_eq!(e["demo.merge"]["r"][0]["t"], "Frame");
        // Copied docstring (pyplot) with matplotlib's placeholder.
        let plot = &e["demo.plotting.plot"];
        assert_eq!(plot["sum"], "Plot y versus x.");
        assert!(plot["doc"].as_str().unwrap().contains("Eigenschaften von `Line2D`"));
        // C function: signature from add_newdoc's first line, doc from it.
        let array = &e["demo.array"];
        assert_eq!(array["sig"], "(object, dtype=None, *, copy=True)");
        assert_eq!(array["sum"], "Create an array.");
        assert_eq!(array["p"][0]["t"], "array_like");
        // A function of another module assigned as a method (matplotlib).
        assert_eq!(e["demo.Axes.stackplot"]["sig"], "(x, *args, labels=())");
        assert_eq!(e["demo.Axes.stackplot"]["sum"], "Draw a stacked area plot.");
        // matplotlib's registered texts fill %(quiver_doc)s.
        assert_eq!(e["demo.Axes.quiver"]["sum"], "Plot a 2D field of arrows.");
        // A ufunc declared in a stub.
        assert_eq!(e["demo.add"]["k"], "function");
        assert!(!ids.iter().any(|i| i.contains("test_x")));
    }
}
