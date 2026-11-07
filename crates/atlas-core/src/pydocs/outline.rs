// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! The outline of one Python file: what it defines (functions, classes
//! with their members), imports, `__all__`, simple assignments and
//! docstrings – read from the tokens, nothing is executed. Statements
//! inside `if`/`try`/`with` count for the enclosing scope, bodies of
//! functions are skipped.

use super::lexer::{self, Tok};

/// One parameter: `*args`, `b: int = 1`, or the markers `*` and `/`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Param {
    /// With `*`/`**` for variadic ones.
    pub name: String,
    pub ann: Option<String>,
    pub default: Option<String>,
}

impl Param {
    fn parse(toks: &[Tok]) -> Param {
        let mut depth = 0i32;
        let (mut colon, mut eq) = (None, None);
        for (i, t) in toks.iter().enumerate() {
            if let Tok::Op(o) = t {
                match o.as_str() {
                    "(" | "[" | "{" => depth += 1,
                    ")" | "]" | "}" => depth -= 1,
                    ":" if depth == 0 && colon.is_none() && eq.is_none() => colon = Some(i),
                    "=" if depth == 0 && eq.is_none() => eq = Some(i),
                    _ => {}
                }
            }
        }
        let name_end = colon.or(eq).unwrap_or(toks.len());
        let name: String = toks[..name_end].iter().map(Tok::text).collect();
        let ann = colon.map(|c| lexer::join(&toks[c + 1..eq.unwrap_or(toks.len())]));
        let default = eq.map(|e| lexer::join(&toks[e + 1..]));
        Param { name, ann, default }
    }

    /// As written in a signature.
    pub fn text(&self) -> String {
        let mut out = self.name.clone();
        if let Some(a) = &self.ann {
            out.push_str(": ");
            out.push_str(a);
        }
        if let Some(d) = &self.default {
            out.push_str(if self.ann.is_some() { " = " } else { "=" });
            out.push_str(d);
        }
        out
    }
}

#[derive(Debug, Clone, Default)]
pub struct Def {
    pub name: String,
    pub class: bool,
    pub params: Vec<Param>,
    /// Return annotation as written.
    pub returns: Option<String>,
    /// Decorators without `@` (`property`, `_api.deprecated("3.8")`).
    pub decorators: Vec<String>,
    /// The same as tokens.
    pub decorator_toks: Vec<Vec<Tok>>,
    pub doc: Option<String>,
    /// Base classes as written (keywords like `metaclass=` left out).
    pub bases: Vec<String>,
    /// Members of a class, in order.
    pub members: Vec<Def>,
    /// Class attributes `name = <expression>` (aliases, accessors).
    pub assigns: Vec<(String, Vec<Tok>)>,
    /// Defined with `@overload` (a typing stub, not the implementation).
    pub overload: bool,
    /// Read from a `.pyi` stub (set by the package, not the parser).
    pub stub: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Import {
    /// `import a.b as c` (`c` → module `a.b`), `from a import b` where `b`
    /// may be a module or a name: (module, Some(name)).
    Module(String),
    Name(String, String),
}

#[derive(Debug, Clone, Default)]
pub struct Outline {
    pub doc: Option<String>,
    pub defs: Vec<Def>,
    /// Name bound by an import, with what it refers to; relative module
    /// names keep their dots (`.frame`), resolved by the caller.
    pub imports: Vec<(String, Import)>,
    pub star_imports: Vec<String>,
    /// `__all__`, if it is a literal list (with `+= other.__all__` parts).
    pub all: Option<Vec<String>>,
    /// `__all__ += mod.__all__` / `__all__.extend(mod.__all__)`: `mod`
    /// (dotted: `lib._histograms_impl`).
    pub all_from: Vec<String>,
    /// `__all__` is computed in a way that cannot be read.
    pub all_dynamic: bool,
    /// Module level `name = <expression>`.
    pub assigns: Vec<(String, Vec<Tok>)>,
    /// `name: annotation` without a value (stubs: ufuncs in numpy).
    pub annotated: Vec<(String, String)>,
    /// `add_newdoc(module, name, doc)` and `(module, name, (attr, doc))`.
    pub newdocs: Vec<(String, String, Option<String>, String)>,
    /// `name.__doc__ = "…"`.
    pub doc_assigns: Vec<(String, String)>,
    /// matplotlib's `_docstring.interpd.register(key=value, …)`: the
    /// texts its `%(key)s` placeholders are filled with.
    pub interpd: Vec<(String, Vec<Tok>)>,
}

enum Scope {
    Module,
    /// Index path into `defs` (class, nested class …).
    Class(Vec<usize>),
    Function,
}

pub fn parse(src: &str) -> Outline {
    let lines = lexer::lines(src);
    let mut out = Outline::default();
    // (indent of the header, scope of the body)
    let mut stack: Vec<(usize, Scope)> = Vec::new();
    let mut decorators: Vec<Vec<Tok>> = Vec::new();
    // A def/class waiting for its docstring: where it is.
    let mut pending_doc: Option<(usize, DocTarget)> = None;
    let mut first_module_line = true;
    for line in &lines {
        while let Some((indent, _)) = stack.last() {
            if line.indent <= *indent {
                stack.pop();
            } else {
                break;
            }
        }
        // Docstring: the first statement of a body.
        if let Some((header_indent, target)) = pending_doc.take() {
            if line.indent > header_indent {
                if let Some(doc) = lexer::strings_value(&line.toks) {
                    set_doc(&mut out, &target, doc);
                    continue;
                }
            }
        }
        let scope = stack.last().map(|(_, s)| s).unwrap_or(&Scope::Module);
        if matches!(scope, Scope::Function) {
            continue;
        }
        let class_path = match scope {
            Scope::Class(p) => Some(p.clone()),
            _ => None,
        };
        if first_module_line && class_path.is_none() {
            first_module_line = false;
            if let Some(doc) = lexer::strings_value(&line.toks) {
                out.doc = Some(doc);
                continue;
            }
        }
        let t = &line.toks;
        let mut k = 0;
        if t[0].is_op("@") {
            decorators.push(t[1..].to_vec());
            continue;
        }
        if t[0].is_name("async") {
            k = 1;
        }
        let is_def = t.get(k).is_some_and(|x| x.is_name("def"));
        let is_class = t.get(k).is_some_and(|x| x.is_name("class"));
        if (is_def || is_class) && t.get(k + 1).and_then(Tok::name).is_some() {
            let def = header(&t[k..], is_class, std::mem::take(&mut decorators));
            let one_liner = header_has_body(&t[k..]);
            let target = add_def(&mut out, class_path.as_deref(), def);
            if !one_liner {
                pending_doc = Some((line.indent, target.clone()));
                stack.push((
                    line.indent,
                    if is_class {
                        match target {
                            DocTarget::Def(p) => Scope::Class(p),
                        }
                    } else {
                        Scope::Function
                    },
                ));
            }
            continue;
        }
        decorators.clear();
        // Compound statements (if/try/with/for/else): their bodies belong
        // to the enclosing scope, nothing to do here.
        if let Some(path) = class_path {
            if let Some((name, value)) = simple_assign(t) {
                class_at(&mut out, &path).assigns.push((name, value.to_vec()));
            }
            continue;
        }
        module_statement(&mut out, t);
    }
    out
}

#[derive(Clone)]
enum DocTarget {
    Def(Vec<usize>),
}

fn class_at<'a>(out: &'a mut Outline, path: &[usize]) -> &'a mut Def {
    let mut def = &mut out.defs[path[0]];
    for &i in &path[1..] {
        def = &mut def.members[i];
    }
    def
}

fn add_def(out: &mut Outline, class: Option<&[usize]>, def: Def) -> DocTarget {
    match class {
        None => {
            out.defs.push(def);
            DocTarget::Def(vec![out.defs.len() - 1])
        }
        Some(path) => {
            let c = class_at(out, path);
            c.members.push(def);
            let mut p = path.to_vec();
            p.push(c.members.len() - 1);
            DocTarget::Def(p)
        }
    }
}

fn set_doc(out: &mut Outline, target: &DocTarget, doc: String) {
    let DocTarget::Def(path) = target;
    class_at(out, path).doc = Some(doc);
}

/// `def name(params) -> ret:` / `class name(bases):` (also with PEP 695
/// type parameters `def f[T](x: T)`).
fn header(t: &[Tok], class: bool, decorator_toks: Vec<Vec<Tok>>) -> Def {
    let decorators: Vec<String> = decorator_toks.iter().map(|d| lexer::join(d)).collect();
    let name = t[1].name().unwrap_or_default().to_owned();
    let mut i = 2;
    if t.get(i).is_some_and(|x| x.is_op("[")) {
        i = lexer::closing(t, i) + 1;
    }
    let mut parts: Vec<&[Tok]> = Vec::new();
    let mut after = i;
    if t.get(i).is_some_and(|x| x.is_op("(")) {
        let close = lexer::closing(t, i);
        parts = lexer::split_commas(&t[i + 1..close.min(t.len())]).into_iter().filter(|p| !p.is_empty()).collect();
        after = close + 1;
    }
    let mut returns = None;
    if !class && t.get(after).is_some_and(|x| x.is_op("->")) {
        // Up to the colon that ends the header (depth 0).
        let rest = &t[after + 1..];
        let mut depth = 0i32;
        let mut end = rest.len();
        for (j, x) in rest.iter().enumerate() {
            match x.text() {
                "(" | "[" | "{" => depth += 1,
                ")" | "]" | "}" => depth -= 1,
                ":" if depth == 0 && matches!(x, Tok::Op(_)) => {
                    end = j;
                    break;
                }
                _ => {}
            }
        }
        returns = Some(lexer::join(&rest[..end]));
    }
    let overload = decorators.iter().any(|d| d == "overload" || d == "typing.overload" || d.ends_with(".overload"));
    let (bases, params) = if class {
        // Keywords (`metaclass=…`) are no bases.
        let bases = parts
            .iter()
            .filter(|b| !(b.len() > 1 && b[1].is_op("=")) && !b[0].is_op("**"))
            .map(|b| lexer::join(b))
            .collect();
        (bases, Vec::new())
    } else {
        (Vec::new(), parts.iter().map(|p| Param::parse(p)).collect())
    };
    Def { name, class, params, returns, decorators, decorator_toks, bases, overload, ..Def::default() }
}

/// The body follows on the same line (`def f(): ...`).
fn header_has_body(t: &[Tok]) -> bool {
    let mut depth = 0i32;
    for (j, x) in t.iter().enumerate() {
        match x.text() {
            "(" | "[" | "{" if matches!(x, Tok::Op(_)) => depth += 1,
            ")" | "]" | "}" if matches!(x, Tok::Op(_)) => depth -= 1,
            ":" if depth == 0 && matches!(x, Tok::Op(_)) => return j + 1 < t.len(),
            _ => {}
        }
    }
    false
}

/// `name = value` (a single target, not `a, b = …` or `a.b = …`).
fn simple_assign(t: &[Tok]) -> Option<(String, &[Tok])> {
    let name = t.first()?.name()?;
    if matches!(name, "if" | "for" | "while" | "with" | "return" | "del" | "assert" | "raise" | "global" | "import") {
        return None;
    }
    // Optional annotation: `name: type = value`.
    let mut i = 1;
    if t.get(1).is_some_and(|x| x.is_op(":")) {
        i = t.iter().position(|x| x.is_op("=")).unwrap_or(t.len());
    }
    if t.get(i).is_some_and(|x| x.is_op("=")) && i + 1 < t.len() {
        return Some((name.to_owned(), &t[i + 1..]));
    }
    None
}

fn module_statement(out: &mut Outline, t: &[Tok]) {
    match t[0].name() {
        Some("import") => {
            for part in lexer::split_commas(&t[1..]) {
                let (path, alias) = dotted_as(part);
                if path.is_empty() {
                    continue;
                }
                match alias {
                    Some(a) => out.imports.push((a, Import::Module(path))),
                    // `import a.b` binds `a`.
                    None => {
                        let first = path.split('.').next().unwrap_or_default().to_owned();
                        out.imports.push((first.clone(), Import::Module(first)));
                    }
                }
            }
        }
        Some("from") => {
            let Some(imp) = t.iter().position(|x| x.is_name("import")) else { return };
            let module: String = t[1..imp].iter().map(Tok::text).collect();
            let mut names = &t[imp + 1..];
            if names.first().is_some_and(|x| x.is_op("(")) {
                names = &names[1..names.len().saturating_sub(1).max(1)];
            }
            if names.first().is_some_and(|x| x.is_op("*")) {
                out.star_imports.push(module);
                return;
            }
            for part in lexer::split_commas(names) {
                let (name, alias) = dotted_as(part);
                if name.is_empty() {
                    continue;
                }
                out.imports.push((alias.unwrap_or_else(|| name.clone()), Import::Name(module.clone(), name)));
            }
        }
        Some("__all__") => all_statement(out, t),
        Some(_) if is_interpd_call(t) => {
            let open = t.iter().position(|x| x.is_op("(")).unwrap_or(0);
            let close = lexer::closing(t, open);
            for arg in lexer::split_commas(&t[open + 1..close.min(t.len())]) {
                if let (Some(key), true) = (arg.first().and_then(Tok::name), arg.get(1).is_some_and(|x| x.is_op("="))) {
                    out.interpd.push((key.to_owned(), arg[2..].to_vec()));
                }
            }
        }
        Some(name) if t.get(1).is_some_and(|x| x.is_op(".")) => {
            // `name.__doc__ = "…"`, `add_newdoc` is handled below.
            if t.get(2).is_some_and(|x| x.is_name("__doc__")) && t.get(3).is_some_and(|x| x.is_op("=")) {
                if let Some(doc) = lexer::strings_value(&t[4..]) {
                    out.doc_assigns.push((name.to_owned(), doc));
                }
            }
        }
        Some("add_newdoc") | Some("add_newdoc_ufunc") if t.get(1).is_some_and(|x| x.is_op("(")) => newdoc(out, t),
        Some("__doc__") if t.get(1).is_some_and(|x| x.is_op("=")) => {
            // The module's text given later (pandas).
            if let Some(doc) = lexer::strings_value(&t[2..]) {
                out.doc = Some(doc);
            }
        }
        _ => {
            if let Some((name, value)) = simple_assign(t) {
                out.assigns.push((name, value.to_vec()));
            } else if let (Some(name), Some(true)) = (t[0].name(), t.get(1).map(|x| x.is_op(":"))) {
                out.annotated.push((name.to_owned(), lexer::join(&t[2..])));
            }
        }
    }
}

fn is_interpd_call(t: &[Tok]) -> bool {
    let callee: String = t.iter().take_while(|x| !x.is_op("(")).map(Tok::text).collect();
    callee.ends_with("interpd.register") || callee.ends_with("interpd.update")
}

/// `a.b.c as d` → ("a.b.c", Some("d")).
fn dotted_as(part: &[Tok]) -> (String, Option<String>) {
    let as_at = part.iter().position(|x| x.is_name("as"));
    let path: String = part[..as_at.unwrap_or(part.len())].iter().map(Tok::text).collect();
    let alias = as_at.and_then(|i| part.get(i + 1)).and_then(Tok::name).map(str::to_owned);
    (path, alias)
}

fn literal_names(t: &[Tok]) -> Vec<String> {
    t.iter().filter(|x| matches!(x, Tok::Str(_))).map(|x| lexer::string_value(x.text())).collect()
}

fn all_statement(out: &mut Outline, t: &[Tok]) {
    let Some(op) = t.get(1) else { return };
    // `__all__ = [...] + other.__all__`, `__all__ += [...]`,
    // `__all__.extend(...)`, numpy's `list(set(_core.__all__) | …)`.
    let (value, replace) = match op.text() {
        "=" => (&t[2..], true),
        "+=" => (&t[2..], false),
        "." if t.get(2).is_some_and(|x| x.is_name("extend") || x.is_name("append")) => (&t[3..], false),
        _ => return,
    };
    // `mod.sub.__all__` parts, with what is left.
    let mut refs = Vec::new();
    let mut rest: Vec<&Tok> = Vec::new();
    let mut i = 0;
    while i < value.len() {
        if value[i].name().is_some() {
            let start = i;
            while value.get(i + 1).is_some_and(|x| x.is_op(".")) && value.get(i + 2).and_then(Tok::name).is_some() {
                i += 2;
            }
            if value[i].is_name("__all__") && i > start {
                refs.push(value[start..i - 1].iter().map(Tok::text).collect::<String>());
                i += 1;
                continue;
            }
            for x in &value[start..=i] {
                rest.push(x);
            }
            i += 1;
            continue;
        }
        rest.push(&value[i]);
        i += 1;
    }
    // Only strings, brackets, + and | (and list/set/sorted/tuple around
    // them) can be read; anything else is computed: unknown.
    let readable = rest.iter().all(|x| match x {
        Tok::Str(_) => true,
        Tok::Op(o) => matches!(o.as_str(), "[" | "]" | "(" | ")" | "," | "+" | "|" | "{" | "}" | "."),
        Tok::Name(n) => matches!(n.as_str(), "list" | "set" | "sorted" | "tuple" | "extend" | "append"),
        Tok::Num(_) => false,
    });
    if !readable {
        out.all = None;
        out.all_dynamic = true;
        return;
    }
    if out.all_dynamic {
        return;
    }
    if replace {
        out.all = Some(Vec::new());
        out.all_from.clear();
    }
    out.all.get_or_insert_with(Vec::new).extend(literal_names(value));
    out.all_from.extend(refs);
}

/// `add_newdoc('numpy._core.multiarray', 'ndarray', ('reshape', """…"""))`.
fn newdoc(out: &mut Outline, t: &[Tok]) {
    let close = lexer::closing(t, 1);
    let args = lexer::split_commas(&t[2..close.min(t.len())]);
    if args.len() < 3 {
        return;
    }
    let (Some(module), Some(name)) = (lexer::strings_value(args[0]), lexer::strings_value(args[1])) else { return };
    let third = args[2];
    if let Some(doc) = lexer::strings_value(third) {
        out.newdocs.push((module, name, None, doc));
    } else if third.first().is_some_and(|x| x.is_op("(")) {
        let inner = lexer::split_commas(&third[1..third.len().saturating_sub(1)]);
        if let [attr, doc, ..] = inner.as_slice() {
            if let (Some(attr), Some(doc)) = (lexer::strings_value(attr), lexer::strings_value(doc)) {
                out.newdocs.push((module, name, Some(attr), doc));
            }
        }
    } else if third.first().is_some_and(|x| x.is_op("[")) {
        // A list of (attr, doc) tuples.
        let inner = &third[1..third.len().saturating_sub(1)];
        for item in lexer::split_commas(inner) {
            if item.first().is_some_and(|x| x.is_op("(")) {
                let pair = lexer::split_commas(&item[1..item.len().saturating_sub(1)]);
                if let [attr, doc, ..] = pair.as_slice() {
                    if let (Some(attr), Some(doc)) = (lexer::strings_value(attr), lexer::strings_value(doc)) {
                        out.newdocs.push((module.clone(), name.clone(), Some(attr), doc));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = r#""""Module doc."""
from __future__ import annotations
import numpy as np
from .frame import DataFrame as DF, Series
from ..core import (a,
    b as c)
from .api import *
__all__ = ["DF", "Series"]
__all__ += other.__all__

if TYPE_CHECKING:
    from typing import Any

@overload
def f(x: int) -> int: ...
@deprecated("2.0")
def f(x, y=1, *, key: str | None = None) -> list[int]:
    """Do f.

    Parameters
    ----------
    x : int
    """
    def inner():
        """Not seen."""
    return 1

class Frame(Base, Generic[T], metaclass=Meta):
    """A frame."""
    str = Accessor("str", StringMethods)
    isnull = isna

    @property
    def shape(self) -> tuple[int, int]:
        """Shape."""
        return (1, 1)

    def isna(self):
        """Missing."""

    class Inner:
        def m(self): ...

def g(): return 1
g.__doc__ = "G doc."
add_newdoc('numpy._core.multiarray', 'array', """array(object)

Create an array.""")
add_newdoc('numpy._core.multiarray', 'ndarray', ('reshape', """Reshape."""))
add: _UFunc
"#;

    #[test]
    fn computed_all_lists() {
        let o = parse("__all__ = list(\n    {'x'} |\n    set(_core.__all__) |\n    set(lib._hist.__all__)\n)\n");
        assert_eq!(o.all.as_deref(), Some(&["x".to_owned()][..]));
        assert_eq!(o.all_from, ["_core", "lib._hist"]);
        let o = parse("__all__ = [n for n in dir() if not n.startswith('_')]\n__all__ += ['y']\n");
        assert!(o.all.is_none());
    }

    #[test]
    fn reads_definitions_imports_and_docstrings() {
        let o = parse(SRC);
        assert_eq!(o.doc.as_deref(), Some("Module doc."));
        assert!(o.imports.contains(&("np".into(), Import::Module("numpy".into()))));
        assert!(o.imports.contains(&("DF".into(), Import::Name(".frame".into(), "DataFrame".into()))));
        assert!(o.imports.contains(&("c".into(), Import::Name("..core".into(), "b".into()))));
        assert!(o.imports.contains(&("Any".into(), Import::Name("typing".into(), "Any".into()))));
        assert_eq!(o.star_imports, [".api"]);
        assert_eq!(o.all.as_deref(), Some(&["DF".to_owned(), "Series".to_owned()][..]));
        assert_eq!(o.all_from, ["other"]);
        let names: Vec<_> = o.defs.iter().map(|d| (d.name.as_str(), d.overload)).collect();
        assert_eq!(names, [("f", true), ("f", false), ("Frame", false), ("g", false)]);
        let f = &o.defs[1];
        let params: Vec<String> = f.params.iter().map(Param::text).collect();
        assert_eq!(params, ["x", "y=1", "*", "key: str | None = None"]);
        assert_eq!(
            f.params[3],
            Param { name: "key".into(), ann: Some("str | None".into()), default: Some("None".into()) }
        );
        assert_eq!(f.returns.as_deref(), Some("list[int]"));
        assert_eq!(f.decorators, ["deprecated(\"2.0\")"]);
        assert!(f.doc.as_deref().unwrap().starts_with("Do f."));
        let c = &o.defs[2];
        assert_eq!(c.bases, ["Base", "Generic[T]"]);
        assert_eq!(c.doc.as_deref(), Some("A frame."));
        let members: Vec<_> = c.members.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(members, ["shape", "isna", "Inner"]);
        assert_eq!(c.members[0].decorators, ["property"]);
        assert_eq!(c.members[0].doc.as_deref(), Some("Shape."));
        assert_eq!(c.members[2].members[0].name, "m");
        let assigns: Vec<_> = c.assigns.iter().map(|(n, v)| (n.as_str(), lexer::join(v))).collect();
        assert_eq!(assigns, [("str", "Accessor(\"str\", StringMethods)".to_owned()), ("isnull", "isna".to_owned())]);
        assert_eq!(o.doc_assigns, [("g".to_owned(), "G doc.".to_owned())]);
        assert_eq!(o.newdocs.len(), 2);
        assert_eq!(o.newdocs[1].2.as_deref(), Some("reshape"));
        assert_eq!(o.annotated, [("add".to_owned(), "_UFunc".to_owned())]);
    }
}
