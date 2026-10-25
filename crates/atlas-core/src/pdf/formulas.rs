// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Formulas in the text of a page.
//!
//! The text layer of a PDF made with LaTeX keeps formulas only as loose
//! characters (`ng2 n` for `\frac{ng_n}{2}`). To give the text of a page
//! with LaTeX instead, the formulas are found by their characters – math
//! fonts (`CMMI10`, `CMSY10`, `LatinModernMath`, …) and math symbols –
//! plus the digits, operators and scripts next to them. Each one is then
//! cut out as a picture for the formula model ([`crate::latex_ocr`]) and
//! put back into the text as `$…$` (or `$$…$$` on a line of its own).

/// One character of the text layer: its rectangle (`[x0, y0, x1, y1]`,
/// points, origin top left), font and font size.
#[derive(Debug, Clone, PartialEq)]
pub struct Glyph {
    pub ch: char,
    pub rect: [f64; 4],
    pub font: String,
    pub size: f64,
}

/// A formula: the indices of its glyphs (in text order) and the rectangle
/// around them.
#[derive(Debug, Clone, PartialEq)]
pub struct Formula {
    pub glyphs: Vec<usize>,
    pub rect: [f64; 4],
    /// Alone on its lines (apart from an equation number): `$$…$$`.
    pub display: bool,
}

/// The text of a page with its formulas.
#[derive(Debug, Clone, Default)]
pub struct PageText {
    pub glyphs: Vec<Glyph>,
    pub formulas: Vec<Formula>,
}

/// At most this many formulas per page are recognised.
pub const MAX_FORMULAS: usize = 40;

/// Pixels per em for the formula model: it reads formulas set at about this
/// size best (rendered much larger it sees nonsense, e.g. `\underline{…}`).
pub const EM_PIXELS: f64 = 22.0;

/// Pixels per point to render text of `font_size` points for the model.
pub fn scale_for(font_size: f64) -> f64 {
    let size = if font_size > 1.0 { font_size } else { 12.0 };
    (EM_PIXELS / size).clamp(0.5, 4.0)
}

/// The font size of most glyphs in `area` (`[x0, y0, x1, y1]`), e.g. of a
/// box drawn around a formula; `None` if there is no text.
pub fn font_size_in(glyphs: &[Glyph], area: [f64; 4]) -> Option<f64> {
    let (x0, x1) = (area[0].min(area[2]), area[0].max(area[2]));
    let (y0, y1) = (area[1].min(area[3]), area[1].max(area[3]));
    let mut sizes: Vec<f64> = glyphs
        .iter()
        .filter(|g| !g.ch.is_whitespace() && g.size > 0.0)
        .filter(|g| {
            let (cx, cy) = ((g.rect[0] + g.rect[2]) / 2.0, (g.rect[1] + g.rect[3]) / 2.0);
            cx >= x0 && cx <= x1 && cy >= y0 && cy <= y1
        })
        .map(|g| g.size)
        .collect();
    if sizes.is_empty() {
        return None;
    }
    // The largest common size: scripts are smaller than the formula.
    sizes.sort_by(f64::total_cmp);
    Some(sizes[sizes.len() * 2 / 3])
}

impl PageText {
    pub fn new(glyphs: Vec<Glyph>) -> Self {
        let formulas = find(&glyphs);
        Self { glyphs, formulas }
    }

    /// The text as the PDF has it.
    pub fn plain(&self) -> String {
        self.glyphs.iter().map(|g| g.ch).collect()
    }

    /// The area to cut out for formula `i`: its rectangle with a margin.
    pub fn clip_area(&self, i: usize) -> [f64; 4] {
        let f = &self.formulas[i];
        let size = f.glyphs.iter().map(|&g| self.glyphs[g].size).fold(0.0, f64::max).max(4.0);
        let m = 0.2 * size;
        [f.rect[0] - m, f.rect[1] - m, f.rect[2] + m, f.rect[3] + m]
    }

    /// Pixels per point to cut out formula `i` with (see [`scale_for`]).
    pub fn clip_scale(&self, i: usize) -> f64 {
        scale_for(font_size_in(&self.glyphs, self.formulas[i].rect).unwrap_or(12.0))
    }

    /// The text with formula `i` replaced by `latex[i]` (`None`: left as
    /// in the PDF).
    pub fn with_latex(&self, latex: &[Option<String>]) -> String {
        let n = self.glyphs.len();
        let mut owner = vec![None; n];
        for (i, f) in self.formulas.iter().enumerate() {
            if latex.get(i).is_some_and(Option::is_some) {
                for &g in &f.glyphs {
                    owner[g] = Some(i);
                }
            }
        }
        // Whitespace between two glyphs of the same formula goes with it.
        let mut previous: Option<usize> = None;
        let mut owner_before = vec![None; n];
        for (i, g) in self.glyphs.iter().enumerate() {
            owner_before[i] = previous;
            if !g.ch.is_whitespace() {
                previous = owner[i];
            }
        }
        let mut next: Option<usize> = None;
        for i in (0..n).rev() {
            if self.glyphs[i].ch.is_whitespace() {
                if owner_before[i].is_some() && owner_before[i] == next {
                    owner[i] = next;
                }
            } else {
                next = owner[i];
            }
        }

        // Lines that held nothing but parts of formulas disappear.
        let mut lines: Vec<(String, bool)> = vec![(String::new(), false)];
        let mut emitted = vec![false; self.formulas.len()];
        for (i, g) in self.glyphs.iter().enumerate() {
            match owner[i] {
                Some(f) => {
                    let line = lines.last_mut().expect("a line");
                    line.1 = true;
                    if !emitted[f] {
                        emitted[f] = true;
                        let tex = latex[f].as_deref().unwrap_or_default().trim();
                        if self.formulas[f].display {
                            line.0.push_str(&format!("$$ {tex} $$"));
                        } else {
                            line.0.push_str(&format!("${tex}$"));
                        }
                    }
                }
                None if g.ch == '\n' => lines.push((String::new(), false)),
                None => lines.last_mut().expect("a line").0.push(g.ch),
            }
        }
        let kept: Vec<&str> = lines
            .iter()
            .filter(|(text, touched)| !(*touched && text.trim().is_empty()))
            .map(|(t, _)| t.as_str())
            .collect();
        kept.join("\n")
    }
}

/// The model's LaTeX without what belongs to the sentence or does nothing
/// at the end: a full stop or comma, trailing spacing (`\,`, `\;`, `\!`,
/// `\quad`).
pub fn tidy(latex: &str) -> String {
    const ENDS: [&str; 7] = ["\\qquad", "\\quad", "\\,", "\\;", "\\!", ".", ","];
    let mut s = latex.trim();
    // `\ ` (a space) became a lone backslash with the trim.
    while let Some(rest) = s.strip_suffix('\\').filter(|r| !r.ends_with('\\')) {
        s = rest.trim_end();
    }
    while let Some(rest) = ENDS.iter().find_map(|end| s.strip_suffix(end)) {
        s = rest.trim_end();
        while let Some(rest) = s.strip_suffix('\\').filter(|r| !r.ends_with('\\')) {
            s = rest.trim_end();
        }
    }
    s.to_owned()
}

/// Font name without the subset tag (`ABCDEF+CMMI10` → `CMMI10`), upper
/// case.
fn base_font(name: &str) -> String {
    let name = match name.split_once('+') {
        Some((tag, rest)) if tag.len() == 6 && tag.chars().all(|c| c.is_ascii_uppercase()) => rest,
        _ => name,
    };
    name.to_ascii_uppercase()
}

/// TeX's math fonts and the OpenType math fonts.
fn math_font(name: &str) -> bool {
    const MATH: &[&str] = &[
        "CMMI", "CMSY", "CMEX", "CMBSY", "MSAM", "MSBM", "EUFM", "EUFB", "EUSM", "EUEX", "EURM", "EURB", "RSFS",
        "WASY", "STMARY", "ESINT", "TXSY", "TXMI", "TXEX", "PXSY", "PXMI", "PXEX", "MATH", "SYMBOL", "BBM", "BBOLD",
        "DSROM", "MNSYMBOL",
    ];
    let name = base_font(name);
    MATH.iter().any(|m| name.contains(m))
}

fn math_char(c: char) -> bool {
    matches!(u32::from(c),
        0x0370..=0x03FF       // Greek
        | 0x2032..=0x2037     // primes
        | 0x2190..=0x22FF     // arrows, operators
        | 0x2308..=0x230B     // ceilings, floors
        | 0x27C0..=0x27EF
        | 0x2980..=0x2AFF
        | 0x1D400..=0x1D7FF   // math letters
        | 0xE000..=0xF8FF) // private use: unnamed glyphs of math fonts
}

fn is_math(g: &Glyph) -> bool {
    // Without a place (the text layer had no rectangles) it can't be cut out.
    let placed = g.rect[2] > g.rect[0] && g.rect[3] > g.rect[1];
    placed && !g.ch.is_whitespace() && (math_font(&g.font) || math_char(g.ch))
}

/// May join a formula next to `other`: digits, operators, brackets and
/// smaller characters (sub- and superscripts).
fn joinable(g: &Glyph, other: &Glyph) -> bool {
    !g.ch.is_whitespace()
        && (g.ch.is_ascii_digit()
            || "=+-−*/()[]{}<>,.;:|!'^_~".contains(g.ch)
            || is_math(g)
            || g.size > 0.0 && g.size < 0.8 * other.size)
}

fn size(g: &Glyph) -> f64 {
    if g.size > 0.0 { g.size } else { (g.rect[3] - g.rect[1]).max(1.0) }
}

enum Near {
    No,
    /// On the same line, a small gap apart.
    Line,
    /// Above or below each other (fractions, limits).
    Stack,
}

fn near(a: &Glyph, b: &Glyph) -> Near {
    let s = size(a).max(size(b));
    let (ra, rb) = (a.rect, b.rect);
    let hgap = (ra[0].max(rb[0]) - ra[2].min(rb[2])).max(0.0);
    let vgap = (ra[1].max(rb[1]) - ra[3].min(rb[3])).max(0.0);
    let voverlap = ra[3].min(rb[3]) - ra[1].max(rb[1]);
    let hoverlap = ra[2].min(rb[2]) - ra[0].max(rb[0]);
    let lower = (ra[3] - ra[1]).min(rb[3] - rb[1]).max(0.1);
    if voverlap > 0.3 * lower && hgap <= 0.5 * s {
        Near::Line
    } else if (extension(a) || extension(b)) && hgap <= 0.3 * s && vgap <= 0.3 * s {
        // A big root or bracket: the text layer puts it as a small box
        // above the line, diagonally next to what it encloses.
        Near::Line
    } else if hoverlap > 0.0 && vgap <= 0.5 * s {
        Near::Stack
    } else {
        Near::No
    }
}

/// Glyph of TeX's extension fonts (big roots, brackets, sums).
fn extension(g: &Glyph) -> bool {
    let font = base_font(&g.font);
    ["CMEX", "TXEX", "PXEX", "EUEX", "ESINT"].iter().any(|f| font.contains(f))
}

fn same_line(a: &[f64; 4], b: &[f64; 4]) -> bool {
    let overlap = a[3].min(b[3]) - a[1].max(b[1]);
    overlap > 0.3 * (a[3] - a[1]).min(b[3] - b[1]).max(0.1)
}

fn find_root(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

/// The formulas among `glyphs`.
pub fn find(glyphs: &[Glyph]) -> Vec<Formula> {
    let n = glyphs.len();
    let mut parent: Vec<usize> = (0..n).collect();
    let mut member = vec![false; n];
    let mut queue: Vec<usize> = (0..n).filter(|&i| is_math(&glyphs[i])).collect();
    for &i in &queue {
        member[i] = true;
    }
    while let Some(a) = queue.pop() {
        for b in 0..n {
            if b == a || glyphs[b].ch.is_whitespace() {
                continue;
            }
            let (ga, gb) = (&glyphs[a], &glyphs[b]);
            let joins = match near(ga, gb) {
                Near::No => false,
                Near::Line => is_math(gb) || joinable(gb, ga),
                Near::Stack => is_math(gb) || is_math(ga) && joinable(gb, ga),
            };
            if !joins {
                continue;
            }
            let (ra, rb) = (find_root(&mut parent, a), find_root(&mut parent, b));
            parent[ra] = rb;
            if !member[b] {
                member[b] = true;
                queue.push(b);
            }
        }
    }

    let mut groups: std::collections::BTreeMap<usize, Vec<usize>> = Default::default();
    for i in (0..n).filter(|&i| member[i]) {
        let root = find_root(&mut parent, i);
        groups.entry(root).or_default().push(i);
    }

    let left_margin = glyphs
        .iter()
        .filter(|g| !g.ch.is_whitespace() && g.rect[2] > g.rect[0])
        .map(|g| g.rect[0])
        .fold(f64::MAX, f64::min);
    let mut groups: Vec<Vec<usize>> = groups.into_values().collect();
    merge_touching(glyphs, &mut groups);

    let mut formulas = Vec::new();
    for mut members in groups {
        trim(glyphs, &mut members);
        absorb(glyphs, &mut members, &member);
        // One symbol alone (π, a bullet ▸) stays as the PDF has it.
        if members.len() < 2 || !members.iter().any(|&i| is_math(&glyphs[i])) {
            continue;
        }
        members.sort_unstable();
        let rect = bounds(glyphs, &members);
        // Displayed formulas are indented or centred, not where lines start.
        let indented = rect[0] - left_margin > 2.0 * size(&glyphs[members[0]]);
        let display = indented && alone(glyphs, &members, rect);
        formulas.push(Formula { glyphs: members, rect, display });
    }
    formulas.sort_by_key(|f| f.glyphs[0]);
    formulas.truncate(MAX_FORMULAS);
    formulas
}

/// Joins parts of one formula: overlapping ones (a numerator sticking out
/// above the line) and ones on the same line with nothing but a small gap
/// between them (where the text layer lacks a glyph, e.g. a big bracket).
fn merge_touching(glyphs: &[Glyph], groups: &mut Vec<Vec<usize>>) {
    let size = |members: &[usize]| members.iter().map(|&i| size(&glyphs[i])).fold(0.0, f64::max);
    loop {
        let rects: Vec<[f64; 4]> = groups.iter().map(|m| bounds(glyphs, m)).collect();
        let mut pair = None;
        'search: for a in 0..groups.len() {
            for b in a + 1..groups.len() {
                let (ra, rb) = (rects[a], rects[b]);
                let hoverlap = ra[2].min(rb[2]) - ra[0].max(rb[0]);
                let voverlap = ra[3].min(rb[3]) - ra[1].max(rb[1]);
                let overlapping = hoverlap > 0.0 && voverlap > 0.0;
                let gap = -hoverlap;
                let beside = same_line(&ra, &rb) && gap > 0.0 && gap <= size(&groups[a]).max(size(&groups[b]));
                let empty_gap = beside && {
                    let (x0, x1) = if ra[0] < rb[0] { (ra[2], rb[0]) } else { (rb[2], ra[0]) };
                    let (y0, y1) = (ra[1].max(rb[1]), ra[3].min(rb[3]));
                    !glyphs.iter().any(|g| {
                        let (cx, cy) = ((g.rect[0] + g.rect[2]) / 2.0, (g.rect[1] + g.rect[3]) / 2.0);
                        !g.ch.is_whitespace() && cx > x0 && cx < x1 && cy > y0 && cy < y1
                    })
                };
                if overlapping || empty_gap {
                    pair = Some((a, b));
                    break 'search;
                }
            }
        }
        let Some((a, b)) = pair else { return };
        let moved = groups.remove(b);
        groups[a].extend(moved);
    }
}

fn bounds(glyphs: &[Glyph], members: &[usize]) -> [f64; 4] {
    members.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |r, &i| {
        let g = glyphs[i].rect;
        [r[0].min(g[0]), r[1].min(g[1]), r[2].max(g[2]), r[3].max(g[3])]
    })
}

/// Drops punctuation and opening brackets at the right end and closing
/// brackets at the left end: they belong to the sentence.
fn trim(glyphs: &[Glyph], members: &mut Vec<usize>) {
    loop {
        let r = bounds(glyphs, members);
        let before = members.len();
        let count = |set: &str| members.iter().filter(|&&i| set.contains(glyphs[i].ch)).count();
        let (opening, closing) = (count("([{"), count(")]}"));
        members.retain(|&i| {
            let g = &glyphs[i];
            let unmatched_closing = closing > opening && ")]}".contains(g.ch);
            let unmatched_opening = opening > closing && "([{".contains(g.ch);
            let right = g.rect[2] >= r[2] - 0.01 && (",.;:([{".contains(g.ch) || unmatched_closing);
            let left = g.rect[0] <= r[0] + 0.01 && (",.;:)]}".contains(g.ch) || unmatched_opening);
            !(right || left)
        });
        if members.len() == before || members.is_empty() {
            return;
        }
    }
}

/// Takes in characters between parts of the formula on the same line
/// (`\sin`, `\text{…}` set in the text font).
fn absorb(glyphs: &[Glyph], members: &mut Vec<usize>, taken: &[bool]) {
    let mut extra = Vec::new();
    for (i, g) in glyphs.iter().enumerate() {
        if taken[i] || g.ch.is_whitespace() {
            continue;
        }
        let (cx, line) = ((g.rect[0] + g.rect[2]) / 2.0, &g.rect);
        let on_line = |j: &&usize| same_line(&glyphs[**j].rect, line);
        let left = members.iter().filter(on_line).any(|&j| glyphs[j].rect[2] <= cx);
        let right = members.iter().filter(on_line).any(|&j| glyphs[j].rect[0] >= cx);
        if left && right {
            // Only small gaps: not a whole sentence between two formulas.
            let s = size(g);
            let close = members.iter().filter(on_line).any(|&j| {
                let r = glyphs[j].rect;
                (r[0] - g.rect[2]).abs() < 1.5 * s || (g.rect[0] - r[2]).abs() < 1.5 * s
            });
            if close {
                extra.push(i);
            }
        }
    }
    members.extend(extra);
}

/// No other text on the lines of the formula, except an equation number to
/// its right.
fn alone(glyphs: &[Glyph], members: &[usize], rect: [f64; 4]) -> bool {
    let set: std::collections::HashSet<usize> = members.iter().copied().collect();
    glyphs.iter().enumerate().all(|(i, g)| {
        if set.contains(&i) || g.ch.is_whitespace() {
            return true;
        }
        let cy = (g.rect[1] + g.rect[3]) / 2.0;
        let beside = cy > rect[1] && cy < rect[3];
        !beside || g.rect[0] >= rect[2] && "()0123456789.*".contains(g.ch)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Glyphs of `text` on one line at `y`, starting at `x`, all in `font`.
    fn run(text: &str, x: f64, y: f64, font: &str, size: f64) -> Vec<Glyph> {
        let w = 0.5 * size;
        text.chars()
            .enumerate()
            .map(|(i, ch)| {
                let x0 = x + i as f64 * w;
                Glyph { ch, rect: [x0, y, x0 + w, y + size], font: font.into(), size }
            })
            .collect()
    }

    fn nl(x: f64, y: f64) -> Glyph {
        Glyph { ch: '\n', rect: [x, y, x, y + 10.0], font: String::new(), size: 10.0 }
    }

    #[test]
    fn tidies_the_end_of_recognised_latex() {
        assert_eq!(tidy("g_{2n}^{2}."), "g_{2n}^{2}");
        assert_eq!(tidy("\\frac{a}{2}\\,."), "\\frac{a}{2}");
        assert_eq!(tidy(" x+y\\quad "), "x+y");
        assert_eq!(tidy("a\\cdot b"), "a\\cdot b");
    }

    #[test]
    fn formula_at_the_start_of_a_line_is_inline() {
        // The end of a sentence wrapped onto its own line: "x2."
        let mut g = run("Text davor", 0.0, 0.0, "Helvetica", 10.0);
        g.push(nl(50.0, 0.0));
        g.extend(run("x", 0.0, 14.0, "CMMI10", 10.0));
        g.extend(run("2", 5.0, 12.0, "CMR7", 7.0));
        g.extend(run(".", 9.0, 14.0, "Helvetica", 10.0));
        let page = PageText::new(g);
        assert_eq!(page.formulas.len(), 1, "{:?}", page.formulas);
        assert!(!page.formulas[0].display);
        assert_eq!(page.with_latex(&[Some("x^2".into())]), "Text davor\n$x^2$.");
    }

    #[test]
    fn recognises_math_fonts() {
        assert!(math_font("ABCDEF+CMMI10"));
        assert!(math_font("CMSY7"));
        assert!(math_font("LatinModernMath-Regular"));
        assert!(math_font("XYZABC+MSBM10"));
        assert!(!math_font("ABCDEF+CMR10"));
        assert!(!math_font("Helvetica"));
        assert!(!math_font("ABCDEF+CMSS10"));
    }

    #[test]
    fn inline_formula_between_words() {
        // "Fläche x = y2 und mehr", x and y in CMMI, = and 2 in CMR.
        let mut g = run("Fläche ", 0.0, 0.0, "Helvetica", 10.0);
        g.extend(run("x", 35.0, 0.0, "ABCDEF+CMMI10", 10.0));
        g.extend(run(" ", 40.0, 0.0, "", 10.0));
        g.extend(run("=", 42.0, 0.0, "ABCDEF+CMR10", 10.0));
        g.extend(run(" ", 47.0, 0.0, "", 10.0));
        g.extend(run("y", 49.0, 0.0, "ABCDEF+CMMI10", 10.0));
        g.extend(run("2", 54.0, -2.0, "ABCDEF+CMR7", 7.0));
        g.extend(run(" und mehr.", 58.0, 0.0, "Helvetica", 10.0));
        let page = PageText::new(g);
        assert_eq!(page.formulas.len(), 1, "{:?}", page.formulas);
        assert!(!page.formulas[0].display);
        assert_eq!(page.plain(), "Fläche x = y2 und mehr.");
        assert_eq!(page.with_latex(&[Some("x = y^2".into())]), "Fläche $x = y^2$ und mehr.");
        assert_eq!(page.with_latex(&[None]), page.plain());
    }

    #[test]
    fn displayed_fraction_over_several_text_lines() {
        // A heading, then a fraction alone on its lines with an equation
        // number: the text layer has it as three lines.
        let mut g = run("Rekursion:", 0.0, 0.0, "Helvetica", 10.0);
        g.push(nl(50.0, 0.0));
        g.extend(run("ab", 100.0, 20.0, "CMMI10", 10.0));
        g.extend(run("   (1)", 200.0, 26.0, "Helvetica", 10.0));
        g.push(nl(230.0, 26.0));
        g.extend(run("2", 102.0, 33.0, "CMR10", 10.0));
        g.push(nl(110.0, 33.0));
        g.extend(run("Ende", 0.0, 60.0, "Helvetica", 10.0));
        let page = PageText::new(g);
        assert_eq!(page.formulas.len(), 1, "{:?}", page.formulas);
        let f = &page.formulas[0];
        assert!(f.display);
        assert_eq!(f.glyphs.len(), 3);
        assert_eq!(f.rect, [100.0, 20.0, 110.0, 43.0]);
        let area = page.clip_area(0);
        assert!(area[0] < 100.0 && area[3] > 43.0);
        assert_eq!(page.with_latex(&[Some("\\frac{ab}{2}".into())]), "Rekursion:\n$$ \\frac{ab}{2} $$   (1)\nEnde");
    }

    #[test]
    fn single_symbols_bullets_and_plain_numbers_stay() {
        let mut g = run("▸ Kreis mit π und 2/20", 0.0, 0.0, "Helvetica", 10.0);
        g[0].font = "MSAM10".into();
        let page = PageText::new(g);
        assert!(page.formulas.is_empty(), "{:?}", page.formulas);
    }

    #[test]
    fn sentence_punctuation_and_brackets_stay_outside() {
        // "(also x+y)," – the brackets and the comma are the sentence's.
        let mut g = run("(also ", 0.0, 0.0, "Helvetica", 10.0);
        g.extend(run("x", 30.0, 0.0, "CMMI10", 10.0));
        g.extend(run("+", 35.0, 0.0, "CMR10", 10.0));
        g.extend(run("y", 40.0, 0.0, "CMMI10", 10.0));
        g.extend(run("),", 45.0, 0.0, "Helvetica", 10.0));
        let page = PageText::new(g);
        assert_eq!(page.formulas.len(), 1);
        assert_eq!(page.with_latex(&[Some("x+y".into())]), "(also $x+y$),");
    }
}
