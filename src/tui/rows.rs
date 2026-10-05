//! Chapter text layout: turns a chapter into screen rows for either the
//! verse-per-line layout or running prose, and the width/wrapping helpers
//! both share with the renderer.

use std::num::NonZeroU16;
use std::sync::OnceLock;

use ratatui::buffer::{Buffer, CellDiffOption};
use ratatui::style::Style;
use unicode_properties::{GeneralCategory, UnicodeGeneralCategory};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::bible::Chapter;
use crate::settings::{ScriptPadding, Settings, VerseNumberStyle};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SegKind {
    /// Verse number in the gutter (`  4 `).
    Num,
    /// Superscript verse number, gutter or inline.
    SuperNum,
    Text,
    /// Inter-word space in prose; belongs to a verse only when both
    /// neighbours do, so a highlight never bleeds into the next verse.
    Space,
    /// One row of the big chapter numeral.
    DropCap,
}

#[derive(Debug, Clone)]
pub(super) struct Seg {
    pub text: String,
    pub kind: SegKind,
    pub verse: Option<u16>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct Row {
    pub segs: Vec<Seg>,
    /// First and last verse with text on this row; `None` for spacer rows.
    pub verses: Option<(u16, u16)>,
    /// Verse whose highlight spans the whole row (verse layout only).
    pub fill: Option<u16>,
}

impl Row {
    pub fn covers(&self, v: u16) -> bool {
        self.verses.is_some_and(|(a, b)| a <= v && v <= b)
    }
}

/// One verse per block: number in a 4-cell gutter, text wrapped beside it.
pub(super) fn verse_rows(chapter: &Chapter, avail: usize, settings: &Settings) -> Vec<Row> {
    let style = settings.typography.verse_number_style;
    let verse_spacing = settings.typography.verse_spacing as usize;
    let line_spacing = settings.typography.line_spacing as usize;

    let mut rows = vec![Row::default()];
    let last_idx = chapter.verses.len().saturating_sub(1);
    for (vi, verse) in chapter.verses.iter().enumerate() {
        let n = verse.number;
        let pieces = wrap_to_width(&verse.text, avail, settings);
        let last_piece = pieces.len().saturating_sub(1);
        for (i, content) in pieces.into_iter().enumerate() {
            let (prefix, kind) = verse_prefix(n, style, i != 0);
            rows.push(Row {
                segs: vec![
                    Seg { text: prefix, kind, verse: Some(n) },
                    Seg { text: content, kind: SegKind::Text, verse: Some(n) },
                ],
                verses: Some((n, n)),
                fill: Some(n),
            });
            if i < last_piece {
                for _ in 0..line_spacing {
                    rows.push(Row::default());
                }
            }
        }
        if vi < last_idx {
            for _ in 0..verse_spacing {
                rows.push(Row::default());
            }
        }
    }
    rows
}

/// Gutter width the verse layout reserves for numbers.
pub(super) fn verse_prefix_width(style: VerseNumberStyle) -> usize {
    match style {
        VerseNumberStyle::InlineBold | VerseNumberStyle::Superscript => 4,
        VerseNumberStyle::Hidden => 0,
    }
}

fn verse_prefix(n: u16, style: VerseNumberStyle, continuation: bool) -> (String, SegKind) {
    match style {
        VerseNumberStyle::Hidden => (String::new(), SegKind::Space),
        _ if continuation => ("    ".to_string(), SegKind::Space),
        VerseNumberStyle::InlineBold => (format!("{:>3} ", n), SegKind::Num),
        VerseNumberStyle::Superscript => (format!("{:>3} ", to_super_digits(n)), SegKind::SuperNum),
    }
}

/// Paragraphs are not marked in the source texts, so prose breaks after a
/// verse that ends a sentence once the paragraph has run this long.
const PARAGRAPH_TARGET: usize = 360;

/// Cells between the drop cap and the text beside it.
const DROP_CAP_GAP: usize = 2;

struct Token {
    segs: Vec<Seg>,
    width: usize,
    verse: u16,
}

/// Running prose: verses flow into paragraphs with superscript numbers.
/// With `drop_cap`, the chapter number opens the chapter as a three-row
/// numeral and verse 1's first words are set in capitals.
pub(super) fn prose_rows(
    chapter: &Chapter,
    width: usize,
    settings: &Settings,
    drop_cap: bool,
) -> Vec<Row> {
    let numbers = settings.typography.verse_number_style != VerseNumberStyle::Hidden;
    let gap = 1 + settings.typography.word_padding as usize;
    let cap = (drop_cap && !chapter.verses.is_empty()).then(|| drop_cap_lines(chapter.number));
    let cap_w = cap.as_ref().map_or(0, |c| c[0].chars().count() + DROP_CAP_GAP);

    // Paragraphs of tokens.
    let mut paragraphs: Vec<Vec<Token>> = vec![Vec::new()];
    let mut para_len = 0;
    for (vi, verse) in chapter.verses.iter().enumerate() {
        if vi > 0 && para_len >= PARAGRAPH_TARGET && ends_sentence(&chapter.verses[vi - 1].text) {
            paragraphs.push(Vec::new());
            para_len = 0;
        }
        para_len += verse.text.len();
        let opening = cap.is_some() && vi == 0;
        for (wi, word) in verse.text.split_whitespace().enumerate() {
            let mut segs = Vec::with_capacity(2);
            if wi == 0 && numbers && !opening {
                segs.push(Seg {
                    text: to_super_digits(verse.number),
                    kind: SegKind::SuperNum,
                    verse: Some(verse.number),
                });
            }
            let text = if opening && wi < 3 { word.to_uppercase() } else { word.to_string() };
            segs.push(Seg { text, kind: SegKind::Text, verse: Some(verse.number) });
            let width = segs.iter().map(|s| str_width(&s.text, settings)).sum();
            paragraphs.last_mut().unwrap().push(Token { segs, width, verse: verse.number });
        }
    }

    let line_spacing = settings.typography.line_spacing as usize;
    let para_spacing = usize::from(settings.typography.verse_spacing > 0);
    let mut rows = vec![Row::default()];
    // Rows emitted after the leading blank; the numeral occupies 0..3.
    let mut vrow = 0usize;
    let step = 1 + line_spacing;
    for (pi, para) in paragraphs.iter().filter(|p| !p.is_empty()).enumerate() {
        if pi > 0 {
            for _ in 0..para_spacing {
                rows.push(cap_row(&cap, vrow));
                vrow += 1;
            }
        }
        let start = vrow;
        let avail = |k: usize| {
            if cap.is_some() && start + k * step < 3 { width.saturating_sub(cap_w) } else { width }
        };
        let lines = fill_lines(para, gap, avail);
        let last = lines.len().saturating_sub(1);
        for (li, line) in lines.iter().enumerate() {
            let mut row = cap_row(&cap, vrow);
            let justify = settings.typography.justify && li < last;
            push_tokens(&mut row, line, avail(li), gap, justify);
            rows.push(row);
            vrow += 1;
            if li < last {
                for _ in 0..line_spacing {
                    rows.push(cap_row(&cap, vrow));
                    vrow += 1;
                }
            }
        }
    }
    // A chapter shorter than the numeral still shows all of it.
    while cap.is_some() && vrow < 3 {
        rows.push(cap_row(&cap, vrow));
        vrow += 1;
    }
    rows
}

/// A row that starts with the drop cap's `line`-th slice, when there is one.
fn cap_row(cap: &Option<[String; 3]>, line: usize) -> Row {
    let mut row = Row::default();
    if let Some(cap) = cap
        && line < 3
    {
        row.segs.push(Seg {
            text: format!("{}{}", cap[line], " ".repeat(DROP_CAP_GAP)),
            kind: SegKind::DropCap,
            verse: None,
        });
    }
    row
}

/// Greedy line fill; `avail(k)` is the width of the k-th line.
fn fill_lines(tokens: &[Token], gap: usize, avail: impl Fn(usize) -> usize) -> Vec<Vec<&Token>> {
    let mut lines: Vec<Vec<&Token>> = Vec::new();
    let mut cur: Vec<&Token> = Vec::new();
    let mut cur_w = 0;
    for t in tokens {
        let need = if cur.is_empty() { t.width } else { cur_w + gap + t.width };
        if !cur.is_empty() && need > avail(lines.len()) {
            lines.push(std::mem::take(&mut cur));
            cur_w = t.width;
        } else {
            cur_w = need;
        }
        cur.push(t);
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

fn push_tokens(row: &mut Row, line: &[&Token], width: usize, gap: usize, justify: bool) {
    let words_w: usize = line.iter().map(|t| t.width).sum();
    let gaps = line.len().saturating_sub(1);
    let total_gap = if justify && gaps > 0 { width.saturating_sub(words_w).max(gaps * gap) } else { gaps * gap };
    let base = total_gap.checked_div(gaps).unwrap_or(0);
    let extra = total_gap.checked_rem(gaps).unwrap_or(0);
    for (i, t) in line.iter().enumerate() {
        if i > 0 {
            let w = base + usize::from(i <= extra);
            let shared = (line[i - 1].verse == t.verse).then_some(t.verse);
            row.segs.push(Seg { text: " ".repeat(w), kind: SegKind::Space, verse: shared });
        }
        row.segs.extend(t.segs.iter().cloned());
    }
    let first = line.iter().map(|t| t.verse).min();
    let last = line.iter().map(|t| t.verse).max();
    if let (Some(a), Some(b)) = (first, last) {
        row.verses = Some((a, b));
    }
}

fn ends_sentence(text: &str) -> bool {
    let trimmed = text.trim_end().trim_end_matches(['"', '\'', '’', '”', ')']);
    trimmed.ends_with(['.', '?', '!'])
}

/// Glyphs for the drop cap: each digit is 4×6 pixels (bit 3 = left
/// column), drawn three rows tall with half blocks.
const DIGITS: [[u8; 6]; 10] = [
    [6, 9, 9, 9, 9, 6],
    [6, 14, 6, 6, 6, 15],
    [6, 9, 1, 2, 4, 15],
    [14, 1, 6, 1, 1, 14],
    [9, 9, 9, 15, 1, 1],
    [15, 8, 14, 1, 1, 14],
    [6, 8, 14, 9, 9, 6],
    [15, 1, 2, 2, 4, 4],
    [6, 9, 6, 9, 9, 6],
    [6, 9, 9, 7, 1, 6],
];

pub(super) fn drop_cap_lines(n: u16) -> [String; 3] {
    let mut out: [String; 3] = Default::default();
    for (di, d) in n.to_string().bytes().map(|b| (b - b'0') as usize).enumerate() {
        for (row, line) in out.iter_mut().enumerate() {
            if di > 0 {
                line.push(' ');
            }
            let (top, bottom) = (DIGITS[d][row * 2], DIGITS[d][row * 2 + 1]);
            for bit in (0..4).rev() {
                line.push(match (top >> bit & 1, bottom >> bit & 1) {
                    (1, 1) => '█',
                    (1, 0) => '▀',
                    (0, 1) => '▄',
                    _ => ' ',
                });
            }
        }
    }
    out
}

/// Viewport top row that keeps the focused verse visible.
///
/// - `pin`: jump-style, focus near the top with a row of context above.
/// - otherwise keep `prev` while the focus is inside the viewport and scroll
///   just enough when it leaves an edge. A verse taller than the viewport
///   anchors at its first row.
pub(super) fn compute_scroll(rows: &[Row], focus: u16, visible: usize, prev: usize, pin: bool) -> usize {
    if visible == 0 || rows.is_empty() {
        return prev;
    }
    let first = rows.iter().position(|r| r.covers(focus));
    let last = rows.iter().rposition(|r| r.covers(focus));
    let (Some(first), Some(last)) = (first, last) else {
        return prev;
    };
    let max_scroll = rows.len().saturating_sub(visible);
    let new = if pin {
        // Jumps land near the top with a row of context; an early verse
        // shows the chapter's opening instead.
        if first < visible / 3 { 0 } else { first.saturating_sub(1) }
    } else if first < prev {
        first
    } else if last >= prev + visible {
        if last + 1 - first > visible { first } else { (last + 1).saturating_sub(visible) }
    } else {
        prev
    };
    new.min(max_scroll)
}

/// First row holding verse `target` or a later one. Aligns parallel panes.
pub(super) fn first_row_for_verse(rows: &[Row], target: u16) -> usize {
    rows.iter()
        .position(|r| r.verses.is_some_and(|(_, b)| b >= target))
        .unwrap_or_else(|| rows.len().saturating_sub(1))
}

/// How the terminal sizes a grapheme with a spacing vowel sign (General
/// Category Mc: Tamil கி, Devanagari कि). Terminals disagree: some give the
/// sign its own cell, others draw it on its consonant. Measured once at
/// startup; see `tui::probe_mark_widths`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MarkWidths {
    /// Signs `unicode-width` counts as a cell (Tamil ி).
    pub spacing: bool,
    /// Signs `unicode-width` counts as zero because they extend the
    /// grapheme (Tamil ா).
    pub extending: bool,
}

static MARK_WIDTHS: OnceLock<MarkWidths> = OnceLock::new();

pub(crate) fn set_mark_widths(m: MarkWidths) {
    let _ = MARK_WIDTHS.set(m);
}

/// Cells the terminal advances for grapheme `g`: `unicode-width`'s answer,
/// corrected for spacing marks by `marks` (no measurement means trusting
/// `unicode-width`).
fn width_with(g: &str, marks: Option<MarkWidths>) -> usize {
    let base = UnicodeWidthStr::width(g);
    let Some(m) = marks else { return base };
    let mut adjust = 0isize;
    for c in g.chars().skip(1) {
        if c.general_category() != GeneralCategory::SpacingMark {
            continue;
        }
        let counted = c.width() == Some(1);
        let terminal = if counted { m.spacing } else { m.extending };
        adjust += terminal as isize - counted as isize;
    }
    if base == 0 { 0 } else { (base as isize + adjust).max(1) as usize }
}

/// Write `text` one grapheme cluster at a time and return the next column.
///
/// Each grapheme takes the cells the terminal will actually advance; when
/// that differs from what ratatui would assume (it trusts `unicode-width`),
/// the cell carries a forced width so ratatui's diff and cursor tracking
/// agree with the terminal. Cells under a wide glyph are skipped; the room
/// a glyph needs beyond that is written as real spaces so it is painted.
pub(super) fn write_graphemes(
    buf: &mut Buffer,
    mut x: u16,
    y: u16,
    x_end: u16,
    text: &str,
    style: Style,
    settings: &Settings,
) -> u16 {
    for (g, term, cells) in grapheme_cells(text, settings) {
        let w = term as u16;
        if w == 0 {
            continue;
        }
        let pad = (cells as u16).saturating_sub(w);
        if x + w + pad > x_end {
            break;
        }
        if let Some(cell) = buf.cell_mut((x, y)) {
            cell.set_symbol(g).set_style(style);
            if usize::from(w) != UnicodeWidthStr::width(g)
                && let Some(nz) = NonZeroU16::new(w)
            {
                cell.set_diff_option(CellDiffOption::ForcedWidth(nz));
            }
        }
        for i in 1..w {
            if let Some(cell) = buf.cell_mut((x + i, y)) {
                cell.set_symbol("").set_diff_option(CellDiffOption::Skip).set_style(style);
            }
        }
        for i in 0..pad {
            if let Some(cell) = buf.cell_mut((x + w + i, y)) {
                cell.set_symbol(" ").set_style(style);
            }
        }
        x += w + pad;
    }
    x
}

pub(super) fn str_width(s: &str, settings: &Settings) -> usize {
    grapheme_cells(s, settings).iter().map(|(_, _, cells)| cells).sum()
}

/// Each grapheme of `text` with the cells the terminal advances for it and
/// the cells it occupies on screen (the same, unless it needs room for its
/// glyph or letter padding).
pub(super) fn grapheme_cells<'a>(text: &'a str, settings: &Settings) -> Vec<(&'a str, usize, usize)> {
    cells_with(text, settings, MARK_WIDTHS.get().copied())
}

/// How far a Tamil glyph may run into the next syllable's cell. A terminal
/// draws every glyph from a cell boundary, so a syllable gets the fewest
/// whole cells that hold its glyph to within this much; side bearings keep
/// that small an overlap from touching the next letter.
const GLYPH_OVERLAP: f32 = 0.4;

fn cells_with<'a>(text: &'a str, settings: &Settings, marks: Option<MarkWidths>) -> Vec<(&'a str, usize, usize)> {
    UnicodeSegmentation::graphemes(text, true)
        .map(|g| {
            let term = width_with(g, marks);
            if term == 0 {
                return (g, 0, 0);
            }
            let glyph = super::tamil::width(g).map_or(term, |w| (w - GLYPH_OVERLAP).ceil().max(1.0) as usize);
            (g, term, glyph.max(term) + padding_cells(g, settings))
        })
        .collect()
}

/// Cells a single grapheme occupies on its own; see [`grapheme_cells`].
pub(super) fn display_width(g: &str, settings: &Settings) -> usize {
    grapheme_cells(g, settings).first().map_or(0, |(_, _, cells)| *cells)
}

/// Extra cells after `g` from the per-script letter padding setting.
fn padding_cells(g: &str, settings: &Settings) -> usize {
    if g.bytes().all(|b| b < 0x80) || g.chars().all(is_super_digit) {
        return 0;
    }
    script_padding_cells(g, &settings.typography.script_letter_padding) as usize
}

fn is_super_digit(c: char) -> bool {
    matches!(c,
        '\u{2070}'              // ⁰
        | '\u{00B9}'            // ¹
        | '\u{00B2}'            // ²
        | '\u{00B3}'            // ³
        | '\u{2074}'..='\u{2079}'   // ⁴-⁹
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Script {
    Latin,
    Tamil,
    Devanagari,
    Arabic,
    Hebrew,
    Cjk,
    Other,
}

/// Detect the dominant script of a grapheme cluster from its first
/// non-combining char. Combining marks attach to the base letter, so the
/// base char is what determines per-script padding.
fn grapheme_script(g: &str) -> Script {
    for c in g.chars() {
        // Skip combining marks — let the base letter decide.
        if matches!(c, '\u{0300}'..='\u{036F}') {
            continue;
        }
        return match c {
            c if c.is_ascii() => Script::Latin,
            '\u{0B80}'..='\u{0BFF}' => Script::Tamil,
            '\u{0900}'..='\u{097F}' => Script::Devanagari,
            '\u{0600}'..='\u{06FF}' | '\u{0750}'..='\u{077F}' => Script::Arabic,
            '\u{0590}'..='\u{05FF}' => Script::Hebrew,
            '\u{4E00}'..='\u{9FFF}'
            | '\u{3040}'..='\u{30FF}'
            | '\u{AC00}'..='\u{D7AF}' => Script::Cjk,
            _ => Script::Other,
        };
    }
    Script::Other
}

fn script_padding_cells(g: &str, p: &ScriptPadding) -> u8 {
    match grapheme_script(g) {
        Script::Latin => 0,
        Script::Tamil => p.tamil,
        Script::Devanagari => p.devanagari,
        Script::Arabic => p.arabic,
        Script::Hebrew => p.hebrew,
        Script::Cjk => p.cjk,
        Script::Other => p.default,
    }
}

pub(super) fn to_super_digits(n: u16) -> String {
    n.to_string()
        .chars()
        .map(|c| match c {
            '0' => '\u{2070}',
            '1' => '\u{00B9}',
            '2' => '\u{00B2}',
            '3' => '\u{00B3}',
            '4' => '\u{2074}',
            '5' => '\u{2075}',
            '6' => '\u{2076}',
            '7' => '\u{2077}',
            '8' => '\u{2078}',
            '9' => '\u{2079}',
            c => c,
        })
        .collect()
}

/// Word-wrap `text` to lines whose display width does not exceed `max_width`,
/// using the same `display_width` function that `write_graphemes` uses at
/// render time. Words longer than `max_width` are broken grapheme-by-
/// grapheme. `word_padding` extends the inter-word gap.
pub(super) fn wrap_to_width(text: &str, max_width: usize, settings: &Settings) -> Vec<String> {
    if max_width == 0 {
        return vec![text.to_string()];
    }
    let gap_width: usize = 1 + settings.typography.word_padding as usize;
    let gap_str: String = " ".repeat(gap_width);

    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut cur_w: usize = 0;

    for word in text.split_whitespace() {
        let word_w = str_width(word, settings);
        let needed = if cur.is_empty() {
            word_w
        } else {
            cur_w + gap_width + word_w
        };
        if needed > max_width && !cur.is_empty() {
            lines.push(std::mem::take(&mut cur));
            cur_w = 0;
        }
        if word_w > max_width {
            // Word doesn't fit even on its own line — break grapheme by
            // grapheme. Flush any pending cur first.
            if !cur.is_empty() {
                lines.push(std::mem::take(&mut cur));
                cur_w = 0;
            }
            for g in UnicodeSegmentation::graphemes(word, true) {
                let gw = display_width(g, settings);
                if cur_w + gw > max_width && !cur.is_empty() {
                    lines.push(std::mem::take(&mut cur));
                    cur_w = 0;
                }
                cur.push_str(g);
                cur_w += gw;
            }
            continue;
        }
        if !cur.is_empty() {
            cur.push_str(&gap_str);
            cur_w += gap_width;
        }
        cur.push_str(word);
        cur_w += word_w;
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    if settings.typography.justify && lines.len() > 1 {
        let last = lines.len() - 1;
        for line in lines.iter_mut().take(last) {
            *line = justify_line(line, max_width, settings);
        }
    }
    lines
}

/// Re-distribute spaces between words so the line spans `target` columns.
/// Returns the line unchanged when justification doesn't apply: lines with
/// fewer than two whitespace-separated tokens (single word, blank, or a
/// grapheme-broken super-long word), or lines whose words already meet/
/// exceed the target.
fn justify_line(line: &str, target: usize, settings: &Settings) -> String {
    let words: Vec<&str> = line.split_whitespace().collect();
    if words.len() < 2 {
        return line.to_string();
    }
    let total_word_w: usize = words.iter().map(|w| str_width(w, settings)).sum();
    if total_word_w >= target {
        return line.to_string();
    }
    let n_gaps = words.len() - 1;
    let total_gap = target - total_word_w;
    let base = total_gap / n_gaps;
    let extra = total_gap % n_gaps;
    let mut out = String::new();
    for (j, word) in words.iter().enumerate() {
        if j > 0 {
            // Front-load the leftover columns onto the first `extra` gaps —
            // standard justification convention.
            let gap_w = base + if j <= extra { 1 } else { 0 };
            out.push_str(&" ".repeat(gap_w));
        }
        out.push_str(word);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bible::Verse;

    fn chapter(number: u16, texts: &[&str]) -> Chapter {
        Chapter {
            number,
            verses: texts
                .iter()
                .enumerate()
                .map(|(i, t)| Verse { number: i as u16 + 1, text: t.to_string() })
                .collect(),
        }
    }

    fn row_width(row: &Row, s: &Settings) -> usize {
        row.segs.iter().map(|g| str_width(&g.text, s)).sum()
    }

    #[test]
    fn tamil_syllables_get_room_for_their_glyphs() {
        let s = Settings::default();
        let word = "இருந்தது";
        let on_base = Some(MarkWidths { spacing: false, extending: false });
        for marks in [on_base, None] {
            for (g, term, cells) in cells_with(word, &s, marks) {
                let w = super::super::tamil::width(g).unwrap();
                // Never less than the terminal advances, never so little the
                // glyph runs well into the next syllable, never a spare cell.
                assert!(cells >= term);
                assert!(cells as f32 >= w - GLYPH_OVERLAP, "{g}: {cells} for {w}");
                assert!(cells == term || (cells as f32) < w - GLYPH_OVERLAP + 1.0, "{g}");
            }
        }
    }

    #[test]
    fn spacing_marks_follow_the_measured_terminal() {
        let ki = "\u{0B95}\u{0BBF}"; // கி: unicode-width says 2
        let kaa = "\u{0B95}\u{0BBE}"; // கா: unicode-width says 1
        let pulli = "\u{0B95}\u{0BCD}"; // க்: virama, never a cell
        assert_eq!((width_with(ki, None), width_with(kaa, None)), (2, 1));
        // tmux, macOS wcwidth: signs sit on their consonant.
        let on_base = Some(MarkWidths { spacing: false, extending: false });
        assert_eq!((width_with(ki, on_base), width_with(kaa, on_base)), (1, 1));
        // Terminals that give every spacing sign a cell.
        let own_cell = Some(MarkWidths { spacing: true, extending: true });
        assert_eq!((width_with(ki, own_cell), width_with(kaa, own_cell)), (2, 2));
        for m in [None, on_base, own_cell] {
            assert_eq!(width_with(pulli, m), 1);
            assert_eq!(width_with("中", m), 2);
            assert_eq!(width_with("a", m), 1);
        }
    }

    #[test]
    fn drop_cap_digits_are_three_rows_of_equal_width() {
        let one = drop_cap_lines(1);
        assert_eq!(one, ["▄██ ".to_string(), " ██ ".to_string(), "▄██▄".to_string()]);
        let big = drop_cap_lines(119);
        assert!(big.iter().all(|l| l.chars().count() == 14));
    }

    #[test]
    fn prose_fits_width_and_covers_every_verse_in_order() {
        let s = Settings::default();
        let ch = chapter(
            3,
            &[
                "For God so loved the world, that he gave his only begotten Son.",
                "For God sent not his Son into the world to condemn the world.",
                "He that believeth on him is not condemned.",
            ],
        );
        for width in [20, 37, 60] {
            let rows = prose_rows(&ch, width, &s, true);
            assert!(rows.iter().all(|r| row_width(r, &s) <= width), "width {width}");
            let mut seen: Vec<u16> = rows.iter().filter_map(|r| r.verses).flat_map(|(a, b)| a..=b).collect();
            seen.dedup();
            assert_eq!(seen, vec![1, 2, 3]);
            // Opening words in capitals beside the numeral.
            assert!(rows[1].segs[0].kind == SegKind::DropCap);
            assert!(rows[1].segs.iter().any(|g| g.text == "FOR"));
        }
    }

    #[test]
    fn prose_highlight_spaces_stay_inside_a_verse() {
        let s = Settings::default();
        let ch = chapter(1, &["a b", "c d"]);
        let rows = prose_rows(&ch, 40, &s, false);
        let spaces: Vec<Option<u16>> = rows[1]
            .segs
            .iter()
            .filter(|g| g.kind == SegKind::Space)
            .map(|g| g.verse)
            .collect();
        assert_eq!(spaces, vec![Some(1), None, Some(2)]);
    }

    #[test]
    fn short_chapter_still_shows_whole_drop_cap() {
        let s = Settings::default();
        let rows = prose_rows(&chapter(117, &["O praise the LORD."]), 40, &s, true);
        let caps = rows.iter().filter(|r| r.segs.first().is_some_and(|g| g.kind == SegKind::DropCap)).count();
        assert_eq!(caps, 3);
    }

    #[test]
    fn scroll_keeps_multi_row_focus_in_view() {
        let s = Settings::default();
        let long = "word ".repeat(40);
        let ch = chapter(1, &[&long, &long, &long, &long]);
        let rows = verse_rows(&ch, 20, &s);
        let top = compute_scroll(&rows, 4, 6, 0, false);
        let last = rows.iter().rposition(|r| r.covers(4)).unwrap();
        assert!(last < top + 6 || rows.iter().position(|r| r.covers(4)).unwrap() == top);
    }
}
