# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Builds the local index of the docs search: imports each package with the
# chosen Python, walks its public API and writes one JSON file per package
# (<out>/<package>.json) plus <out>/index.json (what was built, with which
# Python). Docstrings in the numpydoc format (pandas, numpy, matplotlib,
# scipy, scikit-learn, seaborn …) are split into parameters, returns and
# the other sections, so the view can filter by them.
#
#   python index.py <out> <package> [<package> …]
#
# Runs offline; nothing is downloaded. Prints progress lines for the log.

import importlib
import inspect
import json
import os
import pkgutil
import re
import sys
import time
import warnings

FORMAT = 1
# Submodules imported in addition to those a package imports itself (the
# public API lives there, e.g. matplotlib.pyplot).
EXTRA_MODULES = {
    "matplotlib": ["pyplot", "axes", "artist", "figure", "colors", "cm", "ticker", "dates", "patches", "lines",
                   "collections", "text", "gridspec", "animation", "legend", "image", "markers", "transforms"],
    "scipy": ["stats", "optimize", "integrate", "interpolate", "linalg", "signal", "sparse", "spatial",
              "special", "fft", "ndimage", "cluster", "io", "constants"],
    "sklearn": ["base", "cluster", "compose", "datasets", "decomposition", "ensemble", "feature_extraction",
                "feature_selection", "impute", "inspection", "linear_model", "metrics", "model_selection",
                "naive_bayes", "neighbors", "neural_network", "pipeline", "preprocessing", "svm", "tree", "utils"],
    "pandas": ["api.types", "plotting", "testing", "errors", "tseries.offsets"],
    "numpy": ["linalg", "random", "fft", "polynomial", "ma", "char", "strings", "testing", "lib.stride_tricks"],
    "seaborn": ["objects"],
}
# Never walked (slow, side effects or not public).
SKIP_PARTS = {"tests", "test", "testing_utils", "conftest", "setup", "compat", "core", "_core", "libs",
              "experimental", "externals", "distutils", "f2py", "array_api", "backends", "sphinxext", "conv"}
SKIP_PARTS_TOP = {"tests", "conftest", "setup", "distutils", "f2py", "sphinxext", "backends", "externals"}
MAX_PER_PACKAGE = 40000
# Descriptors whose value is a class with methods of its own (pandas).
ACCESSORS = {"Accessor", "CachedAccessor"}
MAX_DOC = 60000

SECTION_NAMES = {
    "parameters": "params", "params": "params", "arguments": "params", "args": "params",
    "other parameters": "other", "keyword arguments": "other", "kwargs": "other",
    "returns": "returns", "return": "returns", "yields": "yields", "yield": "yields",
    "raises": "raises", "warns": "warns", "see also": "see", "notes": "notes",
    "examples": "examples", "example": "examples", "references": "refs", "attributes": "attrs",
    "methods": "methods",
}


def log(text):
    print(text, flush=True)


def clean(doc):
    if not isinstance(doc, str):
        return ""
    return inspect.cleandoc(doc).strip()


def split_sections(doc):
    """Splits a numpydoc docstring: (text before the first section, {key: lines})."""
    lines = doc.splitlines()
    head, sections, current, i = [], {}, None, 0
    while i < len(lines):
        line = lines[i]
        nxt = lines[i + 1] if i + 1 < len(lines) else ""
        title = line.strip().rstrip(":").lower()
        is_rule = bool(re.fullmatch(r"\s*-{3,}\s*", nxt)) and line.strip() and not line.startswith(" ")
        if is_rule and title in SECTION_NAMES:
            current = SECTION_NAMES[title]
            sections.setdefault(current, [])
            i += 2
            continue
        if is_rule:
            # Unknown section (e.g. "Version", "Warnings"): kept as notes.
            current = "x:" + line.strip()
            sections.setdefault(current, [])
            i += 2
            continue
        (sections[current] if current else head).append(line)
        i += 1
    return "\n".join(head).strip(), sections


def parse_fields(lines, returns=False):
    """Entries "name : type" (or a bare type for returns) with indented text."""
    out, entry = [], None
    for line in lines:
        if not line.strip():
            if entry is not None:
                entry["desc"].append("")
            continue
        if not line.startswith((" ", "\t")):
            if ":" in line and not line.strip().startswith(("http", "{")):
                name, _, typ = line.partition(":")
                name, typ = name.strip(), typ.strip()
            elif returns:
                name, typ = "", line.strip()
            else:
                name, typ = line.strip(), ""
            entry = {"n": name, "t": typ, "desc": []}
            out.append(entry)
        elif entry is not None:
            entry["desc"].append(line.strip())
    for e in out:
        e["desc"] = re.sub(r"\n{3,}", "\n\n", "\n".join(e["desc"]).strip())
        e["desc"] = e["desc"][:4000]
    return out


def summary_of(head):
    para = []
    for line in head.splitlines():
        if not line.strip():
            if para:
                break
            continue
        para.append(line.strip())
    text = " ".join(para)
    # A signature line at the top (C functions, ufuncs) is no summary.
    if re.match(r"^[\w.]+\(.*\)(\s*->.*)?$", text):
        return ""
    return text[:400]


def annotation_text(a):
    if a is inspect.Parameter.empty or a is inspect.Signature.empty:
        return ""
    if isinstance(a, str):
        return a
    if isinstance(a, type):
        return a.__name__
    return re.sub(r"\b(typing|collections\.abc|numpy\.typing|pandas\._typing)\.", "", str(a))


def default_text(d):
    if d is inspect.Parameter.empty:
        return None
    try:
        text = repr(d)
    except Exception:
        return "…"
    if text.startswith("<") and " at 0x" in text:
        text = re.sub(r" at 0x[0-9a-f]+", "", text)
    if "no_default" in text or "NoDefault" in text or "_NoValue" in text:
        return "<kein Standard>"
    return text[:120]


def signature_of(obj):
    try:
        return inspect.signature(obj)
    except (TypeError, ValueError, RecursionError):
        return None
    except Exception:
        return None


def signature_from_doc(name, head):
    """`add(x1, x2, /, out=None, *, …)` at the start of C docstrings."""
    first = head.splitlines()[0].strip() if head else ""
    m = re.match(r"^" + re.escape(name) + r"\((.*)\)", first)
    return "(" + m.group(1) + ")" if m else ""


def kind_of(obj, in_class):
    if inspect.ismodule(obj):
        return "module"
    if inspect.isclass(obj):
        return "exception" if issubclass(obj, BaseException) else "class"
    if isinstance(obj, (property, getattr(sys.modules.get("functools"), "cached_property", property))):
        return "property"
    if callable(obj):
        return "method" if in_class else "function"
    return "attribute"


def describe(qual, obj, pkg, in_class, raw=None):
    kind = kind_of(raw if raw is not None else obj, in_class)
    target = raw.fget if kind == "property" and isinstance(raw, property) and raw.fget else obj
    if kind == "property" and raw is not None and hasattr(raw, "func"):
        target = raw.func
    doc = clean(getattr(raw if raw is not None else obj, "__doc__", None)) if kind in ("property", "attribute") \
        else clean(inspect.getdoc(obj) if kind != "attribute" else "")
    if kind == "attribute":
        doc = ""
    if len(doc) > MAX_DOC:
        doc = doc[:MAX_DOC] + "\n\n…"
    head, sections = split_sections(doc)
    short = qual.rsplit(".", 1)[-1]
    entry = {"id": qual, "k": kind, "sum": summary_of(head)}
    params, sig_text, ret_ann = [], "", ""
    if kind in ("function", "method", "class"):
        sig_obj = obj
        if kind == "class":
            sig_obj = obj
        sig = signature_of(sig_obj)
        if sig is not None:
            items = list(sig.parameters.values())
            if kind == "method" and items and items[0].name in ("self", "cls"):
                items = items[1:]
            sig = sig.replace(parameters=items)
            try:
                sig_text = str(sig)
            except Exception:
                sig_text = ""
            ret_ann = annotation_text(sig.return_annotation)
            for p in items:
                prefix = "*" if p.kind == p.VAR_POSITIONAL else "**" if p.kind == p.VAR_KEYWORD else ""
                params.append({"n": prefix + p.name, "t": annotation_text(p.annotation), "d": default_text(p.default),
                               "kw": p.kind == p.KEYWORD_ONLY})
        else:
            sig_text = signature_from_doc(short, head)
        if sig_text and re.match(r"^" + re.escape(short) + r"\(", head.strip()):
            # The repeated signature line is no part of the text.
            head = "\n".join(head.splitlines()[1:]).strip()
            entry["sum"] = summary_of(head)
        if len(sig_text) > 1500:
            sig_text = sig_text[:1500] + " …)"
    entry["sig"] = sig_text
    # Parameters: signature order, types and descriptions from the docstring.
    documented = []
    for key in ("params", "other"):
        for f in parse_fields(sections.get(key, [])):
            for name in [n.strip() for n in f["n"].split(",") if n.strip()]:
                documented.append({"n": name, "t": f["t"], "desc": f["desc"], "other": key == "other"})
    by_name = {d["n"].lstrip("*"): d for d in documented}
    merged = []
    for p in params:
        d = by_name.pop(p["n"].lstrip("*"), None)
        item = {"n": p["n"]}
        typ = (d or {}).get("t") or p["t"]
        if typ:
            item["t"] = typ
        if p["d"] is not None:
            item["d"] = p["d"]
        if d and d["desc"]:
            item["desc"] = d["desc"]
        if p.get("kw"):
            item["kw"] = True
        merged.append(item)
    for d in documented:
        if d["n"].lstrip("*") in by_name:
            item = {"n": d["n"], "doc": True}
            if d["t"]:
                item["t"] = d["t"]
            if d["desc"]:
                item["desc"] = d["desc"]
            if d["other"]:
                item["other"] = True
            merged.append(item)
    if merged:
        entry["p"] = merged
    returns = parse_fields(sections.get("returns", []) + sections.get("yields", []), returns=True)
    ret = [{k: v for k, v in r.items() if v} for r in returns]
    if not ret and ret_ann and ret_ann not in ("None",):
        ret = [{"t": ret_ann}]
    elif ret_ann and ret and not ret[0].get("t"):
        ret[0]["t"] = ret_ann
    if kind == "class":
        ret = [{"t": short}]
    if ret:
        entry["r"] = ret
    if ret_ann:
        entry["ra"] = ret_ann
    # The rest of the text, sections in their order.
    rest = []
    if head:
        rest.append(head)
    for key, lines in sections.items():
        if key in ("params", "other", "returns", "yields"):
            continue
        text = "\n".join(lines).strip("\n")
        if text.strip():
            rest.append("§" + key + "\n" + text)
    doc_text = "\n\n".join(rest)
    if doc_text:
        entry["doc"] = doc_text
    if "see" in sections:
        see = re.findall(r"^([A-Za-z_][\w.]*)", "\n".join(l for l in sections["see"] if not l.startswith(" ")), re.M)
        if see:
            entry["see"] = see[:30]
    # Deprecated itself (not just one of its parameters): said in the text
    # before the first section.
    if re.search(r"^\s*\.\. deprecated::", head, re.M) or re.match(r"^\s*(\[?deprecated|deprecated)", entry["sum"], re.I):
        entry["dep"] = True
    return entry


def public(name):
    return not name.startswith("_") or name in ("__init__", "__call__")


def belongs(obj, pkg):
    mod = getattr(obj, "__module__", None) or ""
    if inspect.ismodule(obj):
        mod = obj.__name__
    return mod == pkg or mod.startswith(pkg + ".") or (pkg == "sklearn" and mod.startswith("sklearn"))


def import_quiet(name):
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        return importlib.import_module(name)


def walk(pkg_name):
    top = import_quiet(pkg_name)
    version = str(getattr(top, "__version__", "") or "")
    modules = [top]
    for sub in EXTRA_MODULES.get(pkg_name, []):
        try:
            modules.append(import_quiet(pkg_name + "." + sub))
        except Exception as err:
            log(f"  {pkg_name}.{sub}: {type(err).__name__}")
    # Public submodules one level down, for packages not listed above.
    if pkg_name not in EXTRA_MODULES and hasattr(top, "__path__"):
        for info in pkgutil.iter_modules(top.__path__):
            if info.name.startswith("_") or info.name in SKIP_PARTS_TOP:
                continue
            try:
                modules.append(import_quiet(pkg_name + "." + info.name))
            except Exception as err:
                log(f"  {pkg_name}.{info.name}: {type(err).__name__}")

    # Pass 1: every public path to every object. Breadth first, so the
    # shortest path comes first.
    primary = {m.__name__ for m in modules}
    found = {}  # id -> (obj, [paths])
    order = []
    queue = [(m.__name__, m) for m in modules]
    queued = set(primary)
    walked = set()
    while queue:
        mod_name, mod = queue.pop(0)
        if id(mod) in walked:
            continue
        walked.add(id(mod))
        if id(mod) not in found:
            found[id(mod)] = (mod, [mod_name])
            order.append(id(mod))
        names = getattr(mod, "__all__", None)
        if not isinstance(names, (list, tuple)):
            names = [n for n in dir(mod) if public(n)]
        for name in sorted(set(n for n in names if isinstance(n, str) and not n.startswith("_"))):
            try:
                with warnings.catch_warnings():
                    warnings.simplefilter("ignore")
                    obj = getattr(mod, name)
            except Exception:
                continue
            if inspect.ismodule(obj):
                parts = obj.__name__.split(".")
                if belongs(obj, pkg_name) and obj.__name__ not in queued and len(parts) <= 3 \
                        and not (set(parts) & SKIP_PARTS) and not any(p.startswith("_") for p in parts):
                    queued.add(obj.__name__)
                    queue.append((obj.__name__, obj))
                continue
            if not (inspect.isclass(obj) or callable(obj)):
                continue
            # Objects of other packages re-exported here are not ours
            # (numpy's ufuncs say nothing about their module, though).
            if not belongs(obj, pkg_name) and not (pkg_name == "numpy" and type(obj).__name__ == "ufunc"):
                continue
            key = id(obj)
            if key not in found:
                found[key] = (obj, [])
                order.append(key)
            found[key][1].append(mod_name + "." + name)

    def best(obj, paths):
        """At the top of the package (pandas.read_csv), else in the module
        closest to where the object is defined (matplotlib.axes.Axes, not
        matplotlib.pyplot.Axes), else in a module the user imports."""
        home = getattr(obj, "__module__", "") or ""
        def score(path):
            mod = path.rsplit(".", 1)[0]
            common = 0
            for a, b in zip(mod.split("."), home.split(".")):
                if a != b:
                    break
                common += 1
            return (mod != pkg_name, -common, mod not in primary, path.count("."), len(path))
        return min(paths, key=score)

    named = {}
    for key in order:
        obj, paths = found[key]
        named[key] = paths[0] if inspect.ismodule(obj) else best(obj, paths)
    classes = {key for key in order if inspect.isclass(found[key][0])}

    # Pass 2: describe them, classes with their members.
    entries = []
    for key in order:
        if len(entries) >= MAX_PER_PACKAGE:
            break
        obj, _ = found[key]
        qual = named[key]
        try:
            entries.append(describe(qual, obj, pkg_name, False))
        except Exception as err:
            log(f"  {qual}: {type(err).__name__}: {err}")
            continue
        if key in classes:
            entries.extend(members(qual, obj, pkg_name, lambda c: id(c) in classes and c is not obj))
    return version, entries


def members(qual, cls, pkg_name, indexed, depth=0):
    out = []
    names = sorted(n for n in dir(cls) if public(n) and not n.startswith("__"))
    for name in names:
        raw = None
        for base in inspect.getmro(cls):
            if name in base.__dict__:
                raw = base.__dict__[name]
                owner = base
                break
        if raw is None:
            continue
        # Members of object, Exception & co. are noise.
        if owner in (object, BaseException, Exception, type, dict, list, tuple, int, float, str):
            continue
        # Inherited from a class that has its own entry (Line2D.set_alpha
        # is Artist.set_alpha): found there. From private bases (NDFrame
        # for DataFrame.head) they are listed here.
        if owner is not cls and indexed(owner):
            continue
        try:
            with warnings.catch_warnings():
                warnings.simplefilter("ignore")
                obj = getattr(cls, name)
        except Exception:
            obj = raw
        if inspect.isclass(obj) and type(raw).__name__ in ACCESSORS and depth == 0:
            # pandas accessors: Series.str.contains, DataFrame.plot.bar …
            e = describe(qual + "." + name, obj, pkg_name, True)
            e["k"] = "property"
            e.pop("sig", None)
            e.pop("p", None)
            e.pop("r", None)
            out.append(e)
            out.extend(members(qual + "." + name, obj, pkg_name, indexed, depth=1))
            continue
        if inspect.isclass(obj) or inspect.ismodule(obj):
            continue
        func = obj
        if isinstance(raw, (staticmethod, classmethod)):
            func = raw.__func__
        is_prop = isinstance(raw, property) or type(raw).__name__ in ("cached_property", "CachedAccessor", "Accessor",
                                                                   "AxisProperty", "cache_readonly")
        if not (callable(obj) or is_prop):
            continue
        try:
            e = describe(qual + "." + name, func, pkg_name, True, raw=raw if is_prop else None)
        except Exception:
            continue
        if is_prop:
            e["k"] = "property"
        if owner is not cls:
            e["from"] = getattr(owner, "__module__", "") + "." + owner.__qualname__
        out.append(e)
    return out


def main():
    if len(sys.argv) < 3:
        print("Aufruf: index.py <ordner> <paket> …", file=sys.stderr)
        return 2
    out_dir = sys.argv[1]
    wanted = [p for arg in sys.argv[2:] for p in re.split(r"[\s,;]+", arg) if p]
    os.makedirs(out_dir, exist_ok=True)
    os.environ.setdefault("MPLBACKEND", "Agg")
    summary_path = os.path.join(out_dir, "index.json")
    try:
        with open(summary_path, encoding="utf-8") as f:
            summary = json.load(f)
    except Exception:
        summary = {}
    packages = summary.get("packages", {}) if summary.get("format") == FORMAT else {}
    log(f"Python {sys.version.split()[0]} ({sys.executable})")
    failed = 0
    for name in wanted:
        start = time.time()
        log(f"{name}: wird gelesen …")
        try:
            version, entries = walk(name)
        except Exception as err:
            log(f"{name}: nicht installiert oder nicht importierbar ({type(err).__name__}: {err})")
            packages[name] = {"error": f"{type(err).__name__}: {err}", "python": sys.executable, "built": int(time.time())}
            failed += 1
            continue
        data = {"format": FORMAT, "package": name, "version": version, "python": sys.executable,
                "built": int(time.time()), "entries": entries}
        path = os.path.join(out_dir, name + ".json")
        tmp = path + ".tmp"
        with open(tmp, "w", encoding="utf-8") as f:
            json.dump(data, f, ensure_ascii=False, separators=(",", ":"))
        os.replace(tmp, path)
        size = os.path.getsize(path)
        packages[name] = {"version": version, "count": len(entries), "python": sys.executable,
                          "built": data["built"], "size": size}
        log(f"{name} {version}: {len(entries)} Einträge in {time.time() - start:.1f} s ({size // 1024} KB)")
    summary = {"format": FORMAT, "python": sys.executable, "pythonVersion": sys.version.split()[0],
               "built": int(time.time()), "packages": packages}
    tmp = summary_path + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(summary, f, ensure_ascii=False, indent=1)
    os.replace(tmp, summary_path)
    log("fertig")
    return 1 if failed == len(wanted) else 0


if __name__ == "__main__":
    sys.exit(main())
