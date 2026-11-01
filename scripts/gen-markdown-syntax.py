#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
"""Generate the Markdown syntax definition of Atlas' text editor.

KSyntaxHighlighting's own Markdown definition colours fenced code blocks
only for about 40 languages; everything else (```latex, ```lua, ```kotlin,
```diff …) stays one colour. This script takes that definition and adds the
languages below, so a code block in a note is coloured as in an IDE.

The result is a hidden definition "Atlas Markdown" that Atlas embeds at
`:/org.kde.syntax-highlighting/syntax-addons/`, where KSyntaxHighlighting
picks it up; the editor uses it for Markdown files.

Usage (the source of the syntax-highlighting framework, e.g. from
`nix build nixpkgs#kdePackages.syntax-highlighting.src`):

    scripts/gen-markdown-syntax.py path/to/syntax-highlighting/data/syntax \
        > crates/atlas-app/syntax-addons/atlas-markdown.xml
"""

import re
import sys
from pathlib import Path

# Info string (lower case) → KSyntaxHighlighting definition. Languages the
# upstream definition already knows keep its rules; these come after them.
EXTRA = {
    "Nix": ["nix"],
    "LaTeX": ["latex", "tex", "context", "tikz"],
    "BibTeX": ["bibtex", "bib"],
    "Lua": ["lua"],
    "Kotlin": ["kotlin", "kt", "kts"],
    "Swift": ["swift"],
    "Scala": ["scala", "sbt"],
    "Dart": ["dart"],
    "Julia": ["julia", "jl"],
    "Elixir": ["elixir", "ex", "exs"],
    "Erlang": ["erlang", "erl"],
    "Clojure": ["clojure", "clj", "cljs", "cljc", "edn"],
    "Common Lisp": ["lisp", "common-lisp", "commonlisp", "cl", "elisp", "emacs-lisp"],
    "Scheme": ["scheme", "scm", "guile"],
    "Racket": ["racket", "rkt"],
    "Objective Caml": ["ocaml", "ml", "mli"],
    "SML": ["sml", "standard-ml"],
    "FSharp": ["fsharp", "f#", "fs", "fsx", "fsi"],
    "Fortran (Free Format)": ["fortran", "f90", "f95", "f03", "f08"],
    "Fortran (Fixed Format)": ["f77", "for"],
    "Pascal": ["pascal", "delphi", "pas", "objectpascal"],
    "Ada": ["ada", "adb", "ads"],
    "Zig": ["zig"],
    "Odin": ["odin"],
    "Hare": ["hare"],
    "V": ["vlang"],
    "Crystal": ["crystal", "cr"],
    "Gleam": ["gleam"],
    "Elm": ["elm"],
    "PureScript": ["purescript", "purs"],
    "Idris": ["idris", "idr"],
    "Agda": ["agda"],
    "Haxe": ["haxe", "hx"],
    "Groovy": ["groovy", "gradle", "jenkinsfile"],
    "Objective-C": ["objective-c", "objectivec", "objc"],
    "Objective-C++": ["objective-c++", "objectivec++", "objcpp", "objc++"],
    "Vala": ["vala"],
    "OpenCL": ["opencl"],
    "GLSL": ["glsl", "vert", "frag"],
    "Solidity": ["solidity", "sol"],
    "Diff": ["diff", "patch", "udiff"],
    "Dockerfile": ["dockerfile", "docker", "containerfile"],
    "Makefile": ["makefile", "make", "mk", "mak", "gnumake"],
    "Meson": ["meson"],
    "Ninja": ["ninja"],
    "QMake": ["qmake"],
    "Just": ["just", "justfile"],
    "Starlark": ["starlark", "bazel", "bzl"],
    "Fish": ["fish"],
    "Tcsh": ["tcsh", "csh"],
    "PowerShell": ["powershell", "pwsh", "ps1", "posh", "psm1"],
    "MS-DOS Batch": ["batch", "bat", "cmd", "dosbatch", "winbatch"],
    "AWK": ["awk", "gawk"],
    "Tcl/Tk": ["tcl", "tk"],
    "Elvish": ["elvish", "elv"],
    "GNU Assembler": ["asm", "assembly", "gas", "s"],
    "Intel x86 (NASM)": ["nasm", "x86asm"],
    "ARM Assembler": ["armasm", "arm"],
    "LLVM": ["llvm", "ll"],
    "Verilog": ["verilog"],
    "SystemVerilog": ["systemverilog", "sv", "svh"],
    "VHDL": ["vhdl", "vhd"],
    "Octave": ["octave"],
    "Mathematica": ["mathematica", "wolfram", "wl", "nb"],
    "Maxima": ["maxima"],
    "scilab": ["scilab"],
    "Stata": ["stata"],
    "SAS": ["sas"],
    "Stan": ["stan"],
    "Gnuplot": ["gnuplot", "gp"],
    "dot": ["dot", "graphviz", "gv"],
    "Typst": ["typst", "typ"],
    "AsciiDoc": ["asciidoc", "adoc"],
    "Org Mode": ["org"],
    "MediaWiki": ["mediawiki", "wiki"],
    "Textile": ["textile"],
    "Troff Mandoc": ["troff", "groff", "man", "mandoc"],
    "Texinfo": ["texinfo"],
    "GNU Gettext": ["po", "pot", "gettext"],
    "Metapost/Metafont": ["metapost", "metafont", "mp"],
    "PostScript": ["postscript", "ps", "eps"],
    "Sieve": ["sieve"],
    "SCSS": ["scss"],
    "SASS": ["sass"],
    "LESSCSS": ["less"],
    "Vue": ["vue"],
    "Pug": ["pug", "jade"],
    "Jinja": ["jinja", "jinja2", "j2"],
    "Django HTML Template": ["django", "htmldjango"],
    "Twig/Twig": ["twig"],
    "XHTML": ["xhtml"],
    "DTD": ["dtd"],
    "GraphQL": ["graphql", "gql"],
    "Protobuf": ["protobuf", "proto"],
    "TextProto": ["textproto", "txtpb", "pbtxt"],
    "FlatBuffers": ["flatbuffers", "fbs"],
    "SPARQL": ["sparql", "rq"],
    "SQL (PostgreSQL)": ["postgresql", "postgres", "psql", "pgsql", "plpgsql"],
    "SQL (Oracle)": ["plsql", "oracle"],
    "Hjson": ["hjson"],
    "Jsonnet": ["jsonnet", "libsonnet"],
    "KDL": ["kdl"],
    "DotEnv": ["dotenv", "env"],
    "Java Properties": ["properties", "jproperties"],
    "Apache Configuration": ["apache", "apacheconf", "htaccess"],
    "nginx Configuration": ["nginx", "nginxconf"],
    "systemd unit": ["systemd", "service", "unit"],
    "Git Ignore": ["gitignore", "ignore"],
    "Kconfig": ["kconfig"],
    "Devicetree Source (DTS)": ["dts", "devicetree"],
    "NFTables": ["nftables", "nft"],
    "Terraform": ["terraform", "tf", "hcl"],
    "RPM Spec": ["spec", "rpmspec"],
    "Bitbake": ["bitbake", "bb"],
    "Puppet": ["puppet"],
    "Snakemake": ["snakemake", "smk"],
    "Robot": ["robot", "robotframework"],
    "Cucumber Gherkin feature": ["gherkin", "cucumber", "feature"],
    "vCard, vCalendar, iCalendar": ["ics", "ical", "icalendar", "vcard", "vcf", "vcalendar"],
    "Email": ["eml"],
    "GDB": ["gdb"],
    "Logcat": ["logcat"],
    "CSV": ["csv"],
    "TSV": ["tsv"],
    "BNF": ["bnf", "ebnf", "peg"],
    "ABNF": ["abnf"],
    "ANTLR": ["antlr", "g4"],
    "GNU M4": ["m4"],
    "GNU Linker Script": ["ld", "linkerscript"],
    "COBOL": ["cobol", "cob", "cbl"],
    "ABAP": ["abap"],
    "ANS-Forth94": ["forth", "4th", "fth"],
    "J": ["j"],
    "q": ["q", "kdb"],
    "REXX": ["rexx"],
    "Modula-2": ["modula2", "modula-2"],
    "Modula-3": ["modula3", "modula-3"],
    "Eiffel": ["eiffel"],
    "Nemerle": ["nemerle"],
    "Boo": ["boo"],
    "Pike": ["pike"],
    "Pony": ["pony"],
    "MonoBasic": ["vb", "vbnet", "vb.net", "visualbasic"],
    "FreeBASIC": ["freebasic", "basic", "bas"],
    "PureBasic": ["purebasic"],
    "AutoHotKey": ["autohotkey", "ahk"],
    "NSIS": ["nsis"],
    "InnoSetup": ["innosetup", "iss"],
    "CoffeeScript": ["coffeescript", "coffee"],
    "ActionScript 2.0": ["actionscript", "as"],
    "UnrealScript": ["unrealscript", "uc"],
    "Godot": ["gdscript", "godot"],
    "RenPy": ["renpy"],
    "Slint": ["slint"],
    "Smali": ["smali"],
    "Common Intermediate Language (CIL)": ["cil", "msil"],
    "OpenSCAD": ["openscad", "scad"],
    "POV-Ray": ["povray", "pov"],
    "G-Code": ["gcode", "nc"],
    "Wavefront OBJ": ["obj", "wavefront"],
    "Spice": ["spice"],
    "TLA+": ["tla", "tlaplus"],
    "Modelica": ["modelica"],
    "Metamath": ["metamath"],
    "GAP": ["gap"],
    "Magma": ["magma"],
    "yacas": ["yacas"],
    "Curry": ["curry"],
    "Literate Haskell": ["lhs", "literate-haskell"],
    "Raku": [],  # upstream already
    "Quarto": ["quarto", "qmd"],
    "R Markdown": ["rmd", "rmarkdown"],
    "reStructuredText": [],  # upstream already
    "SubRip Subtitles": ["srt"],
    "Web Video Text Tracks": ["vtt", "webvtt"],
    "Todo.txt": ["todotxt"],
    "ChangeLog": ["changelog"],
    "Debian Control": ["debcontrol", "control"],
    "Git Rebase": ["git-rebase", "gitrebase"],
    "fstab": ["fstab"],
    "x.org Configuration": ["xorg", "xorg.conf"],
    "XKeyboardConfig": ["xkb"],
    "WINE Config": ["reg", "registry"],
    "Varnish Configuration Language": ["vcl"],
    "Mustache/Handlebars (HTML)": [],  # upstream already
    "Velocity": ["velocity", "vm"],
    "Mako": ["mako"],
    "FTL": ["ftl", "freemarker"],
    "Hamlet": [],  # upstream already
    "Gemtext": ["gemtext", "gmi"],
    "Jira": ["jira"],
    "Fluent": ["fluent"],
    "D2": ["d2"],
    "Overpass QL": ["overpassql", "overpass"],
    "YARA": ["yara", "yar"],
    "YANG": ["yang"],
    "ASN.1": ["asn1", "asn.1", "asn"],
    "IDL": ["idl", "omg-idl"],
    "Inform": ["inform"],
    "Prolog": ["prolog", "swipl"],
    "Logtalk": ["logtalk"],
    "Euphoria": ["euphoria"],
    "Expect": ["expect"],
    "MEL": ["mel"],
    "Praat": ["praat"],
    "Pig": ["pig"],
    "k": ["k"],
    "toit": ["toit"],
    "Viper": ["viper"],
    "Zonnon": ["zonnon"],
    "Sather": ["sather"],
    "Pure": ["pure"],
    "Cg": ["cg"],
    "nesC": ["nesc"],
    "LSL": ["lsl"],
    "Clipper": ["clipper"],
    "xHarbour": ["harbour", "xharbour"],
    "CashScript": ["cashscript"],
    "Earthfile": ["earthfile"],
    "GN": ["gn"],
    "Jam": ["jam"],
    "Doxyfile": ["doxyfile"],
    "QDoc Configuration": ["qdocconf"],
    "Kate Config": ["kateconfig"],
    "Wesnoth Markup Language": ["wml-wesnoth"],
    "M3U": ["m3u", "m3u8"],
    "CUE Sheet": ["cue"],
    "LRC Lyrics": ["lrc"],
    "PGN": ["pgn"],
    "Music Publisher": ["mup"],
    "ABC": ["abc"],
    "IATA SSIM": ["ssim"],
    "Intel HEX": ["ihex", "intelhex"],
    "FASTQ": ["fastq"],
    "Snort/Suricata": ["snort", "suricata"],
    "SELinux CIL Policy": ["selinux-cil"],
    "Valgrind Suppression": ["supp"],
    "MIB": ["mib"],
    "Ansys": ["ansys"],
    "B-Method": ["bmethod"],
    "Replicode": ["replicode"],
    "RETRO": ["retro"],
    "RapidQ": ["rapidq"],
    "KBasic": ["kbasic"],
    "OORS": ["oors"],
    "OPAL": ["opal"],
    "ATS": ["ats"],
    "Crack": ["crack"],
    "E Language": ["e"],
    "Component-Pascal": ["componentpascal"],
    "PL/I": ["pli", "pl1"],
    "JCL": ["jcl"],
    "CLIST": ["clist"],
    "ILERPG": ["rpg", "rpgle"],
    "progress": ["progress", "openedge"],
    "4GL": ["4gl", "informix"],
    "AMPLE": ["ample"],
    "BrightScript": ["brightscript", "brs"],
    "TaskJuggler": ["taskjuggler", "tjp"],
    "txt2tags": ["txt2tags", "t2t"],
    "SiSU": ["sisu"],
    "SGML": ["sgml"],
    "RELAX NG": ["relaxng", "rng"],
    "RelaxNG-Compact": ["rnc"],
    "XUL": ["xul"],
    "JSP": ["jsp"],
    "ASP": ["asp"],
    "ColdFusion": ["coldfusion", "cfm", "cfml"],
    "Rich Text Format": ["rtf"],
    "R documentation": ["rd"],
    "PostScript Printer Description": ["ppd"],
    "GlossTex": ["glosstex"],
    "ConTeXt": [],  # "context" goes to LaTeX above
    "Doxygen": [],  # upstream already
    "Javadoc": ["javadoc"],
    "Java Module": [],
    "Wayland Trace": [],
    "VRML": ["vrml", "wrl"],
    "STEP": ["step", "stp"],
    "STL": ["stl"],
    "PLY": ["ply"],
    "RenderMan RIB": ["rib"],
}
# Entries without aliases only note why a language is left out.
EXTRA = {name: aliases for name, aliases in EXTRA.items() if aliases}

# Left out: sed, Cap'n Proto, Lex/Flex, Yacc/Bison, GTK Blueprint, Cabal and
# LDIF keep their state past the closing fence, so the rest of the note would
# be coloured as code; Mermaid likewise (Atlas draws those blocks anyway).

# Definitions whose first context only looks for a shebang and falls through
# to the real one: include that one (a fallthrough would leave the code block
# and miss its closing fence).
START = {"Kotlin": "Normal", "Swift": "Normal", "SAS": "Normal", "Prolog": "clause"}

ALIAS_CHARS = re.compile(r"^[a-z0-9+#.\-_]+$")


def definitions(syntax_dir: Path) -> dict[str, str]:
    """Name → identifier for the context names, of every definition."""
    out = {}
    for path in syntax_dir.glob("*.xml"):
        head = path.read_text(encoding="utf-8")[:6000]
        m = re.search(r"<language\b[^>]*\bname=\"([^\"]+)\"", head, re.S)
        if m:
            out[m.group(1).replace("&apos;", "'").replace("&amp;", "&")] = path.stem
    return out


def xml_attr(text: str) -> str:
    return text.replace("&", "&amp;").replace('"', "&quot;").replace("'", "&apos;").replace("<", "&lt;")


def main() -> None:
    syntax_dir = Path(sys.argv[1])
    source = (syntax_dir / "markdown.xml").read_text(encoding="utf-8")
    known = definitions(syntax_dir)

    # Languages upstream already includes, and the info strings it matches
    # (its rules come first; an alias here that it matches is never reached).
    upstream = set(re.findall(r'<IncludeRules context="##([^"]+)" includeAttrib="true"/>', source))

    rules, contexts, seen = [], [], set()
    for name, aliases in EXTRA.items():
        if name in upstream:
            continue
        if name not in known:
            sys.exit(f"unknown definition: {name}")
        aliases = [a for a in aliases if a not in seen]
        for a in aliases:
            if not ALIAS_CHARS.match(a):
                sys.exit(f"bad alias for {name}: {a}")
            seen.add(a)
        if not aliases:
            continue
        ctx = "x-" + re.sub(r"[^a-z0-9]+", "-", name.lower()).strip("-") + "-code"
        alt = "|".join(re.sub(r"([+#.\-])", r"\\\1", a) for a in aliases)
        rules.append(
            f'        <RegExpr attribute="Fenced Code" context="#pop!{ctx}" '
            f'String="&fcode;\\s*(?:{alt})&info;" insensitive="true" beginRegion="code-block"/>'
        )
        contexts.append(
            f'      <context attribute="Normal Text" lineEndContext="#stay" name="{ctx}">\n'
            f'        <IncludeRules context="code"/>\n'
            f'        <IncludeRules context="{START.get(name, "")}##{xml_attr(name)}" includeAttrib="true"/>\n'
            f"      </context>"
        )

    out = source
    # After the language, attributes may follow (```python title="a.py").
    out = out.replace('<!ENTITY end "\\s*$">', '<!ENTITY end "\\s*$">\n<!ENTITY info "(?:\\s.*)?$">', 1)
    assert "&info;" not in source and "<!ENTITY info " in out, "entities changed"
    i = out.index('<context name="find-lang-fenced-code"')
    j = out.index("</context>", i)
    out = out[:i] + out[i:j].replace('&end;" insensitive="true"', '&info;" insensitive="true"') + out[j:]
    # Nix code is Nix, not Bash.
    out, n = re.subn(r"\|nix\)&info;", ")&info;", out, count=1)
    assert n == 1, "bash rule changed"
    # Hidden, under its own name: never chosen for a file by itself.
    out, n = re.subn(
        r'<language name="Markdown" version="(\d+)"[^>]*>',
        lambda m: (
            f'<language name="Atlas Markdown" version="{m.group(1)}" kateversion="{kateversion(source)}" '
            'section="Markup" extensions="" hidden="true" author="Darrin Yeager, Claes Holmerson; Atlas" license="GPL,BSD">'
        ),
        out,
        count=1,
    )
    assert n == 1, "language header changed"
    catch_all = '        <RegExpr attribute="Fenced Code" context="#pop!code" String="&fcode;.*$" beginRegion="code-block"/>'
    assert catch_all in out, "catch-all rule changed"
    out = out.replace(catch_all, "\n".join(rules) + "\n" + catch_all, 1)
    anchor = '<context name="code" attribute="Code" lineEndContext="#stay">'
    assert anchor in out, "code context changed"
    i = out.index(anchor)
    i = out.index("</context>", i) + len("</context>")
    out = out[:i] + "\n" + "\n".join(contexts) + out[i:]
    note = (
        "<!--\n    Generated by scripts/gen-markdown-syntax.py from markdown.xml of\n"
        "    KSyntaxHighlighting: fenced code blocks for many more languages.\n    Do not edit.\n-->\n"
    )
    out = out.replace("<!DOCTYPE language", note + "<!DOCTYPE language", 1)
    sys.stdout.write(out)


def kateversion(source: str) -> str:
    return re.search(r'<language name="Markdown"[^>]*kateversion="([^"]+)"', source).group(1)


if __name__ == "__main__":
    main()
