use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Clear, Gauge, List, ListItem, Padding, Paragraph, Widget, Wrap,
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::reference::book_display;
use crate::settings::{
    DividerStyle, ScriptPadding, Settings, ThemePreset, VerseNumberStyle,
};
use crate::storage;

use super::{App, Mode, SPINNER_FRAMES};

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();

    // Wipe the frame area at the top of every draw. ratatui's Buffer marks
    // the second column of a wide unicode glyph as a "skip" cell; if the
    // next frame writes a 1-column ASCII char in its place, the skip cell
    // is left dangling and renders as a stray glyph. Switching from
    // Tamil/CJK to English exposes this. `Clear` calls Cell::reset on
    // every position, killing the skip markers; the terminal's diff
    // renderer means unchanged cells still aren't re-transmitted.
    f.render_widget(Clear, area);

    match app.mode {
        Mode::Manager => draw_manager(f, app, area),
        Mode::Bookmarks => draw_bookmarks(f, app, area),
        Mode::Settings => draw_settings(f, app, area),
        _ => draw_reader(f, app, area),
    }

    if app.mode == Mode::Help {
        draw_help_overlay(f, area);
    }
    if app.mode == Mode::PickSecondary {
        draw_pick_secondary(f, app, area);
    }

    if app.download.is_some() {
        draw_download_popup(f, app, area);
    }
}

fn draw_reader(f: &mut Frame, app: &App, area: Rect) {
    let theme = resolve_theme(&app.settings);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    draw_top_bar(f, app, rows[0], &theme);
    if app.mode == Mode::NoTranslation || app.bible.is_none() {
        draw_welcome(f, rows[1]);
    } else if app.parallel && app.secondary_bible.is_some() {
        let body = apply_width_cap(rows[1], &app.settings);
        let panes = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(body);
        let primary = app.bible.as_ref().unwrap();
        draw_chapter_pane(f, app, panes[0], primary, false, &theme);
        if let Some(sec) = app.secondary_bible.as_ref() {
            draw_chapter_pane(f, app, panes[1], sec, true, &theme);
        }
        apply_divider_style(f, panes[0], panes[1], &app.settings);
    } else {
        let body = apply_width_cap(rows[1], &app.settings);
        let primary = app.bible.as_ref().unwrap();
        draw_chapter_pane(f, app, body, primary, false, &theme);
    }
    draw_bottom_bar(f, app, rows[2], &theme);
}

/// If `settings.reader.max_columns` is set and smaller than the available
/// width, narrow `area` to the cap and centre it horizontally. Otherwise
/// returns `area` unchanged.
fn apply_width_cap(area: Rect, settings: &Settings) -> Rect {
    let cap = settings.reader.max_columns;
    if cap == 0 || area.width <= cap {
        return area;
    }
    let extra = area.width - cap;
    let left = extra / 2;
    Rect::new(area.x + left, area.y, cap, area.height)
}

/// Overlay the seam between the two parallel panes per `divider` setting.
/// `Single` (default) leaves both blocks' touching borders alone — terminals
/// render that as a thicker `││` line. `Double` overlays `║` and `None`
/// overlays spaces, both onto the seam columns.
fn apply_divider_style(f: &mut Frame, left: Rect, right: Rect, settings: &Settings) {
    let style = settings.parallel.divider;
    if style == DividerStyle::Single {
        return;
    }
    let (overlay, color) = match style {
        DividerStyle::Double => ("║", Color::Indexed(244)),
        DividerStyle::None => (" ", Color::Reset),
        DividerStyle::Single => unreachable!(),
    };
    // Last column of the left pane and first column of the right pane both
    // carry block borders. Replace both so the seam looks consistent.
    let seam_x_left = left.x + left.width.saturating_sub(1);
    let seam_x_right = right.x;
    let inner_top = left.y + 1;
    let inner_bottom = left.y + left.height.saturating_sub(1);
    f.render_widget(
        SeamOverlay {
            x: seam_x_left,
            y_start: inner_top,
            y_end: inner_bottom,
            symbol: overlay,
            color,
        },
        left,
    );
    f.render_widget(
        SeamOverlay {
            x: seam_x_right,
            y_start: inner_top,
            y_end: inner_bottom,
            symbol: overlay,
            color,
        },
        right,
    );
}

struct SeamOverlay {
    x: u16,
    y_start: u16,
    y_end: u16,
    symbol: &'static str,
    color: Color,
}

impl Widget for SeamOverlay {
    fn render(self, _area: Rect, buf: &mut Buffer) {
        for y in self.y_start..self.y_end {
            if let Some(cell) = buf.cell_mut((self.x, y)) {
                cell.set_symbol(self.symbol)
                    .set_style(Style::default().fg(self.color));
                cell.set_skip(false);
            }
        }
    }
}

fn draw_top_bar(f: &mut Frame, app: &App, area: Rect, theme: &ResolvedTheme) {
    let translation = app
        .bible
        .as_ref()
        .map(|b| b.translation.display_name.as_str())
        .unwrap_or("—");
    let position = match app.current.as_ref() {
        Some(cr) => format!("{} {}", book_display(&cr.book()), cr.chapter()),
        None => "—".to_string(),
    };
    let line = Line::from(vec![
        Span::styled(
            " bible ",
            Style::default().bg(theme.border).fg(Color::White).bold(),
        ),
        Span::raw(" "),
        Span::styled(translation, Style::default().fg(theme.title_book).bold()),
        Span::raw(" │ "),
        Span::styled(position, Style::default().fg(theme.title_chapter)),
        Span::raw("   "),
        Span::styled("?", Style::default().add_modifier(Modifier::DIM)),
        Span::styled(" help", Style::default().add_modifier(Modifier::DIM)),
        Span::raw("  "),
        Span::styled("T", Style::default().add_modifier(Modifier::DIM)),
        Span::styled(" translations", Style::default().add_modifier(Modifier::DIM)),
        Span::raw("  "),
        Span::styled("q", Style::default().add_modifier(Modifier::DIM)),
        Span::styled(" quit", Style::default().add_modifier(Modifier::DIM)),
    ]);
    f.render_widget(line, area);
}

fn draw_welcome(f: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Indexed(24)));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Welcome to bible",
            Style::default().bold().fg(Color::Cyan),
        )),
        Line::from(""),
        Line::from("No translations are installed yet."),
        Line::from(""),
        Line::from(vec![
            Span::raw("Press "),
            Span::styled("i", Style::default().bold().fg(Color::Yellow)),
            Span::raw(" to install English KJV, or "),
            Span::styled("T", Style::default().bold().fg(Color::Yellow)),
            Span::raw(" to browse the catalog."),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("Or from the shell: "),
            Span::styled("bible install kjv", Style::default().bold().fg(Color::Green)),
        ]),
    ];
    let p = Paragraph::new(text)
        .alignment(ratatui::layout::Alignment::Center)
        .wrap(Wrap { trim: true });
    f.render_widget(p, inner);
}

/// Render a single chapter pane (block + scrolling chapter content). Used
/// for both single-pane reading and each side of the parallel view.
///
/// `is_secondary` flips the title border colour and embeds the translation
/// id in the title so the user can tell the two panes apart at a glance.
fn draw_chapter_pane(
    f: &mut Frame,
    app: &App,
    area: Rect,
    bible: &crate::bible::Bible,
    is_secondary: bool,
    theme: &ResolvedTheme,
) {
    let Some(cr) = app.current.as_ref() else {
        return;
    };

    let border_color = if is_secondary {
        theme.secondary_border
    } else {
        theme.border
    };

    let mut title_spans = vec![Span::raw(" ")];
    if app.parallel || is_secondary {
        title_spans.push(Span::styled(
            bible.translation.id.clone(),
            Style::default().fg(theme.title_translation).bold(),
        ));
        title_spans.push(Span::raw(" │ "));
    }
    title_spans.push(Span::styled(
        book_display(&cr.book()),
        Style::default().fg(theme.title_book).bold(),
    ));
    title_spans.push(Span::raw(" "));
    title_spans.push(Span::styled(
        cr.chapter().to_string(),
        Style::default().fg(theme.title_chapter).bold(),
    ));
    title_spans.push(Span::raw(" "));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        // Asymmetric padding by request: keep top/left tight (0/1), triple
        // right/bottom (3/3) so text breathes from the border on those sides.
        .padding(Padding::new(1, 3, 0, 3))
        .title(Line::from(title_spans));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let chapter = match bible.get_chapter(cr) {
        Some(c) => c,
        None => {
            for row_idx in 0..inner.height {
                let row_area = Rect::new(inner.x, inner.y + row_idx, inner.width, 1);
                let (prefix, content) = if row_idx == 0 {
                    ("", "(chapter not present in this translation)")
                } else {
                    ("", "")
                };
                f.render_widget(
                    ChapterRow {
                        prefix,
                        prefix_style: Style::default(),
                        content,
                        content_style: Style::default().add_modifier(Modifier::DIM),
                        settings: &app.settings,
                    },
                    row_area,
                );
            }
            return;
        }
    };

    let highlight_verse = if is_secondary {
        None
    } else {
        highlighted_verse_for(app, cr)
    };
    let prefix_w: usize = match app.settings.typography.verse_number_style {
        VerseNumberStyle::InlineBold | VerseNumberStyle::Superscript => 4,
        VerseNumberStyle::Hidden => 0,
    };
    let avail = (inner.width as usize).saturating_sub(prefix_w).max(1);
    let rows = build_rows(chapter, avail, highlight_verse, &app.settings);

    // Single-pane reads `app.scroll` (line offset). Parallel mode reads
    // `app.verse_anchor` and finds each pane's own row index for that verse,
    // so different wrap shapes don't desync.
    let start = if app.parallel {
        first_row_for_verse(&rows, app.verse_anchor.max(1))
    } else {
        app.scroll as usize
    };

    let visible = inner.height as usize;
    for (row_idx, row) in rows.iter().skip(start).take(visible).enumerate() {
        let row_area = Rect::new(inner.x, inner.y + row_idx as u16, inner.width, 1);
        let (num_style, text_style) = row_styles(row, theme);
        f.render_widget(
            ChapterRow {
                prefix: &row.prefix,
                prefix_style: num_style,
                content: &row.content,
                content_style: text_style,
                settings: &app.settings,
            },
            row_area,
        );
    }

    // Blank any trailing rows when the chapter is shorter than the viewport.
    let rendered = rows.len().saturating_sub(start).min(visible);
    for row_idx in rendered..visible {
        let row_area = Rect::new(inner.x, inner.y + row_idx as u16, inner.width, 1);
        f.render_widget(
            ChapterRow {
                prefix: "",
                prefix_style: Style::default(),
                content: "",
                content_style: Style::default(),
                settings: &app.settings,
            },
            row_area,
        );
    }
}

fn row_styles(row: &Row, theme: &ResolvedTheme) -> (Style, Style) {
    let num_style = if row.highlighted {
        Style::default()
            .fg(theme.current_verse_fg)
            .bg(theme.current_verse_bg)
            .bold()
    } else if row.is_super_prefix {
        Style::default()
            .fg(theme.verse_number_super)
            .add_modifier(Modifier::DIM)
    } else {
        Style::default().fg(theme.verse_number)
    };
    let text_style = if row.highlighted {
        Style::default().bold()
    } else {
        Style::default()
    };
    (num_style, text_style)
}

/// Custom row renderer for the chapter content. Bypasses Line/Paragraph and
/// goes straight to `Buffer::cell_mut` so we can:
///
/// 1. Hard-reset every cell in the row, *including* unsetting the `skip`
///    flag. ratatui's `Cell::reset()` clears the symbol/style but leaves
///    `skip` untouched. Wide-char graphemes from a prior frame leave
///    `skip = true` on the trailing cell, and the diff renderer literally
///    skips those cells — so a stale glyph survives every subsequent
///    redraw, no matter how many times we write a space over it.
/// 2. Then write the prefix, then call `write_graphemes` to lay down each
///    content grapheme cluster with a display width that gives complex
///    scripts (Tamil, Devanagari, Arabic …) a 2-cell minimum so the
///    terminal's glyph rendering doesn't overlap into the next character.
struct ChapterRow<'a> {
    prefix: &'a str,
    prefix_style: Style,
    content: &'a str,
    content_style: Style,
    settings: &'a Settings,
}

impl Widget for ChapterRow<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }
        let y = area.y;
        for x in area.left()..area.right() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.reset();
                cell.set_symbol(" ");
                cell.set_skip(false);
            }
        }
        // Prefix is always ASCII or Unicode-superscript digits, both
        // narrow — set_string measures correctly for advancing.
        buf.set_string(area.x, y, self.prefix, self.prefix_style);
        let prefix_w = UnicodeWidthStr::width(self.prefix) as u16;
        if prefix_w < area.width && !self.content.is_empty() {
            write_graphemes(
                buf,
                area.x + prefix_w,
                y,
                area.right(),
                self.content,
                self.content_style,
                self.settings,
            );
        }
    }
}

/// Write `text` into the buffer one grapheme cluster at a time, advancing
/// by `display_width()` columns per grapheme. Trailing cells of multi-cell
/// graphemes get `set_skip(true)` so the diff renderer doesn't emit a write
/// for them and the terminal can render the glyph across both cells.
fn write_graphemes(
    buf: &mut Buffer,
    mut x: u16,
    y: u16,
    x_end: u16,
    text: &str,
    style: Style,
    settings: &Settings,
) {
    for g in UnicodeSegmentation::graphemes(text, true) {
        let w = display_width(g, settings) as u16;
        if w == 0 {
            continue;
        }
        if x + w > x_end {
            break;
        }
        if let Some(cell) = buf.cell_mut((x, y)) {
            cell.set_symbol(g).set_style(style);
        }
        for i in 1..w {
            if let Some(cell) = buf.cell_mut((x + i, y)) {
                cell.set_symbol("").set_skip(true).set_style(style);
            }
        }
        x += w;
    }
}

/// Display width for a single grapheme cluster.
///
/// `unicode-width` reflects the Unicode East Asian Width property and is
/// agnostic to font rendering. Many monospace terminal fonts render
/// complex-script glyphs wider than the property suggests, leaving the
/// next character to overlap into the glyph. We compensate with two tiers
/// of bump:
///
/// - Non-ASCII graphemes that contain a *wide-extending* mark (Tamil
///   vowel signs that stretch horizontally beyond the base letter, like
///   `ா`, `ை`, `ெ`) get 3 columns. 2 isn't enough — base + extending
///   sign visually consumes more than two cells in most Tamil fonts.
/// - Other non-ASCII graphemes get 2 columns. Covers single-codepoint
///   Tamil letters (`ஆ`), narrow-vowel combinations (`தி` where `ி`
///   sits above the base), CJK already-wide letters (no-op), Latin
///   diacritics, etc.
/// - Plus an optional per-script bump from settings, for users whose
///   fonts still don't separate the glyphs cleanly.
///
/// Superscript digits (used as verse numbers when that style is on) are
/// whitelisted to their raw 1-cell width so the prefix aligns.
fn display_width(g: &str, settings: &Settings) -> usize {
    let raw = UnicodeWidthStr::width(g);
    if raw == 0 {
        return 0;
    }
    if g.bytes().all(|b| b < 0x80) {
        return raw;
    }
    if g.chars().all(is_super_digit) {
        return raw;
    }
    let base = if g.chars().any(is_wide_extending_mark) {
        raw.max(3)
    } else {
        raw.max(2)
    };
    base + script_padding_cells(g, &settings.typography.script_letter_padding) as usize
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

/// True for combining marks that visually *extend* horizontally beyond the
/// base letter (as opposed to compact marks that sit above/below). Hand-
/// curated for Tamil — the only complex script the user has reported
/// trouble with so far. Easy to extend to Devanagari/Bengali/Sinhala/
/// etc. as needed.
fn is_wide_extending_mark(c: char) -> bool {
    matches!(c,
        // Tamil aa-sign
        '\u{0BBE}'
        // Tamil e-, ee-, ai-sign (left-side, two-part with ai)
        | '\u{0BC6}'..='\u{0BC8}'
        // Tamil o-, oo-, au-sign (two-part)
        | '\u{0BCA}'..='\u{0BCC}'
        // Tamil au-length-mark
        | '\u{0BD7}'
    )
}

struct Row {
    highlighted: bool,
    prefix: String,
    content: String,
    /// Verse number this row belongs to. `None` for blank/spacer rows. Used
    /// by parallel view to align both panes by verse.
    verse: Option<u16>,
    /// The prefix is rendered as superscript digits — used to dim it.
    is_super_prefix: bool,
}

impl Row {
    fn blank() -> Self {
        Row {
            highlighted: false,
            prefix: String::new(),
            content: String::new(),
            verse: None,
            is_super_prefix: false,
        }
    }
}

/// Build the wrapped, prefix-decorated row list for a chapter at a given
/// inner content width. Used by both single-pane and parallel-pane render.
fn build_rows(
    chapter: &crate::bible::Chapter,
    avail: usize,
    highlight_verse: Option<u16>,
    settings: &Settings,
) -> Vec<Row> {
    let style = settings.typography.verse_number_style;
    let verse_spacing = settings.typography.verse_spacing as usize;
    let line_spacing = settings.typography.line_spacing as usize;

    let mut rows: Vec<Row> = Vec::new();
    rows.push(Row::blank());
    let last_idx = chapter.verses.len().saturating_sub(1);
    for (vi, verse) in chapter.verses.iter().enumerate() {
        let highlighted = Some(verse.number) == highlight_verse;
        let wrapped = wrap_to_width(&verse.text, avail, settings);
        let pieces = if wrapped.is_empty() {
            vec![String::new()]
        } else {
            wrapped
        };
        let last_piece_idx = pieces.len().saturating_sub(1);
        for (i, content) in pieces.into_iter().enumerate() {
            let prefix = format_verse_prefix(verse.number, style, i != 0);
            let is_super_prefix =
                style == VerseNumberStyle::Superscript && i == 0;
            rows.push(Row {
                highlighted,
                prefix,
                content,
                verse: Some(verse.number),
                is_super_prefix,
            });
            if line_spacing > 0 && i < last_piece_idx {
                for _ in 0..line_spacing {
                    rows.push(Row::blank());
                }
            }
        }
        if verse_spacing > 0 && vi < last_idx {
            for _ in 0..verse_spacing {
                rows.push(Row::blank());
            }
        }
    }
    rows
}

fn format_verse_prefix(n: u16, style: VerseNumberStyle, continuation: bool) -> String {
    match style {
        VerseNumberStyle::Hidden => String::new(),
        VerseNumberStyle::InlineBold => {
            if continuation {
                "    ".to_string()
            } else {
                format!("{:>3} ", n)
            }
        }
        VerseNumberStyle::Superscript => {
            if continuation {
                "    ".to_string()
            } else {
                let s = to_super_digits(n);
                format!("{:>3} ", s)
            }
        }
    }
}

fn to_super_digits(n: u16) -> String {
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

/// Find the first row in `rows` whose verse number is `>= target`. Used to
/// align two parallel panes by verse — each pane finds its own scroll-start
/// independently, but they share the same verse anchor.
fn first_row_for_verse(rows: &[Row], target: u16) -> usize {
    for (i, r) in rows.iter().enumerate() {
        if let Some(v) = r.verse {
            if v >= target {
                return i;
            }
        }
    }
    rows.len().saturating_sub(1)
}

/// Word-wrap `text` to lines whose display width does not exceed `max_width`,
/// using the same `display_width` function that `write_graphemes` uses at
/// render time. Words longer than `max_width` are broken grapheme-by-
/// grapheme. `word_padding` extends the inter-word gap.
fn wrap_to_width(text: &str, max_width: usize, settings: &Settings) -> Vec<String> {
    if max_width == 0 {
        return vec![text.to_string()];
    }
    let gap_width: usize = 1 + settings.typography.word_padding as usize;
    let gap_str: String = " ".repeat(gap_width);

    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut cur_w: usize = 0;

    for word in text.split_whitespace() {
        let word_w: usize = UnicodeSegmentation::graphemes(word, true)
            .map(|g| display_width(g, settings))
            .sum();
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
    let total_word_w: usize = words
        .iter()
        .map(|w| {
            UnicodeSegmentation::graphemes(*w, true)
                .map(|g| display_width(g, settings))
                .sum::<usize>()
        })
        .sum();
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

fn highlighted_verse_for(
    app: &App,
    cr: &crate::reference::BibleChapterReference,
) -> Option<u16> {
    let vr = app.search_hits.get(app.search_idx)?;
    if vr.book().number() != cr.book().number() {
        return None;
    }
    let cur_chap: u32 = cr.chapter().into();
    let hit_chap: u32 = vr.chapter().into();
    if cur_chap != hit_chap {
        return None;
    }
    let v: u32 = vr.verse().into();
    u16::try_from(v).ok()
}

fn draw_bottom_bar(f: &mut Frame, app: &App, area: Rect, theme: &ResolvedTheme) {
    if let Some(s) = app.searching.as_ref() {
        let frame = SPINNER_FRAMES[s.spinner % SPINNER_FRAMES.len()];
        let elapsed = s.started_at.elapsed().as_millis();
        let line = Line::from(vec![
            Span::raw(" "),
            Span::styled(
                frame.to_string(),
                Style::default().fg(theme.title_chapter).bold(),
            ),
            Span::raw(" searching for "),
            Span::styled(
                format!("`{}`", sanitize_one_line(&s.query)),
                Style::default().fg(theme.title_translation).bold(),
            ),
            Span::raw("  "),
            Span::styled(
                format!("({}.{:03}s)", elapsed / 1000, elapsed % 1000),
                Style::default().add_modifier(Modifier::DIM),
            ),
        ]);
        f.render_widget(line, area);
        return;
    }

    let line = match app.mode {
        Mode::Jump => Line::from(vec![
            Span::styled(":", Style::default().fg(theme.title_book).bold()),
            Span::raw(sanitize_one_line(app.input.value())),
            Span::styled("│", Style::default().fg(theme.title_book)),
        ]),
        Mode::Search => Line::from(vec![
            Span::styled("/", Style::default().fg(theme.title_translation).bold()),
            Span::raw(sanitize_one_line(app.input.value())),
            Span::styled("│", Style::default().fg(theme.title_translation)),
        ]),
        _ => {
            if !app.status.is_empty() {
                Line::from(Span::styled(
                    sanitize_one_line(&app.status),
                    Style::default().fg(theme.status_bar_dim),
                ))
            } else if app.parallel {
                // Surface the parallel-close hint inline; without it `|` is
                // hard to remember and there's no other obvious way out.
                let mut spans = vec![Span::raw(" ")];
                spans.extend(hint_spans(&[
                    (":", "ref"),
                    ("/", "search"),
                    ("↑↓", "change verse"),
                    ("←→", "change chapter"),
                ]));
                spans.push(Span::raw("  "));
                spans.push(Span::styled(
                    "|",
                    Style::default().fg(theme.title_translation).bold(),
                ));
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    "close parallel",
                    Style::default().fg(theme.title_translation).bold(),
                ));
                spans.push(Span::raw("  "));
                spans.extend(hint_spans(&[
                    ("\\", "swap"),
                    ("q", "quit"),
                ]));
                spans.push(Span::raw(" "));
                Line::from(spans)
            } else {
                let mut spans = vec![Span::raw(" ")];
                spans.extend(hint_spans(&[
                    (":", "ref"),
                    ("/", "search"),
                    ("↑↓", "scroll"),
                    ("←→", "change chapter"),
                    (",", "settings"),
                    ("q", "quit"),
                    ("?", "help"),
                ]));
                spans.push(Span::raw(" "));
                Line::from(spans)
            }
        }
    };
    f.render_widget(line, area);
}

/// Build a list of spans for a status-bar / modal-footer hint line. Each
/// pair is (key-glyph, label); keys are coloured cyan + bold, labels are
/// dim. Caller owns the leading/trailing-space framing of the line.
fn hint_spans(pairs: &[(&'static str, &'static str)]) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::with_capacity(pairs.len() * 4);
    for (i, (key, label)) in pairs.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(
            *key,
            Style::default().fg(Color::Cyan).bold(),
        ));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            *label,
            Style::default().add_modifier(Modifier::DIM),
        ));
    }
    spans
}

/// Wrap `hint_spans` output with a leading + trailing space and turn into a
/// `Line`. Used for footer hints in modals.
fn hint_line(pairs: &[(&'static str, &'static str)]) -> Line<'static> {
    let mut spans = vec![Span::raw(" ")];
    spans.extend(hint_spans(pairs));
    spans.push(Span::raw(" "));
    Line::from(spans)
}

fn sanitize_one_line(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\n' | '\r' | '\t' => ' ',
            c if c.is_control() => '·',
            c => c,
        })
        .collect()
}

fn draw_manager(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta))
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled("Translations", Style::default().fg(Color::Magenta).bold()),
            Span::raw(" "),
            Span::styled(
                format!("({} available, {} installed)",
                    app.available.len(),
                    app.installed.len()),
                Style::default().add_modifier(Modifier::DIM),
            ),
            Span::raw(" "),
        ]));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(inner);

    let filter_line = Line::from(vec![
        Span::styled(" filter ", Style::default().bg(Color::Indexed(238)).fg(Color::White)),
        Span::raw(" "),
        Span::raw(app.manager_filter.value().to_string()),
        Span::styled("│", Style::default().fg(Color::Magenta)),
    ]);
    f.render_widget(Paragraph::new(filter_line), rows[0]);

    let indices = app.filtered_indices();
    let items: Vec<ListItem> = indices
        .iter()
        .enumerate()
        .map(|(row, &idx)| {
            let t = &app.available[idx];
            let installed = storage::is_installed(&t.id);
            let mark = if installed { "●" } else { "○" };
            let mark_style = if installed {
                Style::default().fg(Color::Green).bold()
            } else {
                Style::default().fg(Color::Indexed(244))
            };
            let row_style = if row == app.manager_cursor {
                Style::default().bg(Color::Indexed(236))
            } else {
                Style::default()
            };
            ListItem::new(Line::from(vec![
                Span::raw(" "),
                Span::styled(mark.to_string(), mark_style),
                Span::raw("  "),
                Span::styled(
                    format!("{:30}", t.id),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw("  "),
                Span::raw(t.display_name.clone()),
                Span::raw("  "),
                Span::styled(
                    format!("({})", t.language),
                    Style::default().add_modifier(Modifier::DIM),
                ),
            ]))
            .style(row_style)
        })
        .collect();

    let list = List::new(items);
    f.render_widget(list, rows[1]);

    let hint = hint_line(&[
        ("Enter", "install/uninstall"),
        ("r", "refresh"),
        ("↑↓", "move"),
        ("Esc", "back"),
    ]);
    f.render_widget(Paragraph::new(hint), rows[2]);
}

fn draw_bookmarks(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled("Bookmarks", Style::default().fg(Color::Yellow).bold()),
            Span::raw(" "),
            Span::styled(
                format!("({})", app.bookmarks.len()),
                Style::default().add_modifier(Modifier::DIM),
            ),
            Span::raw(" "),
        ]));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(inner);

    if app.bookmarks.is_empty() {
        let empty = Line::from(vec![
            Span::raw("  "),
            Span::styled(
                "No bookmarks yet — press ",
                Style::default().add_modifier(Modifier::DIM),
            ),
            Span::styled("b", Style::default().fg(Color::Yellow).bold()),
            Span::styled(
                " on a chapter, or ",
                Style::default().add_modifier(Modifier::DIM),
            ),
            Span::styled(":b 16 my note", Style::default().fg(Color::Cyan).bold()),
            Span::styled(
                " to add one.",
                Style::default().add_modifier(Modifier::DIM),
            ),
        ]);
        f.render_widget(empty, rows[0]);
    } else {
        let items: Vec<ListItem> = app
            .bookmarks
            .iter()
            .enumerate()
            .map(|(i, bm)| {
                let book_label = crate::reference::book_from_number(bm.book_number)
                    .ok()
                    .map(|b| crate::reference::book_display(&b))
                    .unwrap_or("?");
                let ref_str = match bm.verse {
                    Some(v) => format!("{} {}:{}", book_label, bm.chapter, v),
                    None => format!("{} {}", book_label, bm.chapter),
                };
                let row_style = if i == app.bookmarks_cursor {
                    Style::default().bg(Color::Indexed(236))
                } else {
                    Style::default()
                };
                let mut spans = vec![
                    Span::raw(" ★ "),
                    Span::styled(
                        format!("{:24}", bm.translation),
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::raw(" · "),
                    Span::styled(ref_str, Style::default().fg(Color::Yellow).bold()),
                ];
                if !bm.note.is_empty() {
                    spans.push(Span::raw(" · "));
                    spans.push(Span::styled(
                        format!("\"{}\"", bm.note),
                        Style::default(),
                    ));
                }
                ListItem::new(Line::from(spans)).style(row_style)
            })
            .collect();
        let list = List::new(items);
        f.render_widget(list, rows[0]);
    }

    let hint = hint_line(&[
        ("Enter", "jump"),
        ("d", "delete"),
        ("↑↓", "move"),
        ("Esc", "back"),
    ]);
    f.render_widget(Paragraph::new(hint), rows[1]);
}

fn draw_help_overlay(f: &mut Frame, area: Rect) {
    let popup = centered_rect(60, 70, area);
    f.render_widget(Clear, popup);
    let block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let header = |s: &'static str| {
        Line::from(Span::styled(s, Style::default().bold().fg(Color::Yellow)))
    };
    let row = |key: &'static str, label: &'static str| {
        Line::from(vec![
            Span::raw("  "),
            Span::styled(format!("{:<14}", key), Style::default().fg(Color::Cyan).bold()),
            Span::styled(label, Style::default().add_modifier(Modifier::DIM)),
        ])
    };
    let lines = vec![
        header("Reading"),
        row("↑ / ↓", "scroll line"),
        row("PgUp / PgDn", "half-page"),
        row("Home / End", "top / bottom of chapter"),
        row("← / →", "previous / next chapter"),
        row("Shift+← / →", "previous / next book"),
        Line::from(""),
        header("Lookup"),
        row(":", "jump to reference (e.g. :John 3:16)"),
        row("/", "search current translation"),
        row("↑ / ↓", "(in : or /) browse command history"),
        row("n / N", "next / previous search match"),
        Line::from(""),
        header("Bookmarks"),
        row("b", "bookmark current chapter"),
        row(":b N <note>", "bookmark verse N with optional note"),
        row("B", "open bookmarks list  (Enter jump, d delete)"),
        Line::from(""),
        header("Translations"),
        row("T", "translation manager"),
        row("t", "cycle installed translations"),
        row("|", "toggle parallel view (two translations side-by-side)"),
        row("\\", "swap the parallel-view secondary translation"),
        Line::from(""),
        header("Misc"),
        row(",", "settings (typography, theme, width, divider)"),
        row("?", "this help (Esc to close)"),
        row("q", "quit"),
    ];
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn draw_pick_secondary(f: &mut Frame, app: &App, area: Rect) {
    let popup = centered_rect(60, 60, area);
    f.render_widget(Clear, popup);
    let block = Block::default()
        .title(" Pick parallel translation ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let primary_id = app
        .bible
        .as_ref()
        .map(|b| b.translation.id.as_str())
        .unwrap_or("");
    let candidates: Vec<&crate::bible::TranslationInfo> = app
        .installed
        .iter()
        .filter(|t| t.id != primary_id)
        .collect();

    if candidates.is_empty() {
        let msg = Line::from(Span::styled(
            "  Install another translation first (T → install).",
            Style::default().add_modifier(Modifier::DIM),
        ));
        f.render_widget(msg, inner);
        return;
    }

    let items: Vec<ListItem> = candidates
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let style = if i == app.secondary_picker_cursor {
                Style::default().bg(Color::Indexed(236))
            } else {
                Style::default()
            };
            ListItem::new(Line::from(vec![
                Span::raw(" "),
                Span::styled(
                    format!("{:30}", t.id),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw("  "),
                Span::raw(t.display_name.clone()),
                Span::raw("  "),
                Span::styled(
                    format!("({})", t.language),
                    Style::default().add_modifier(Modifier::DIM),
                ),
            ]))
            .style(style)
        })
        .collect();
    f.render_widget(List::new(items), inner);
}

fn draw_download_popup(f: &mut Frame, app: &App, area: Rect) {
    let Some(d) = app.download.as_ref() else {
        return;
    };
    let popup = centered_rect(60, 20, area);
    f.render_widget(Clear, popup);
    let block = Block::default()
        .title(" Downloading ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Length(1), Constraint::Min(0)])
        .split(inner);

    let label = Line::from(vec![
        Span::raw(" "),
        Span::styled(d.id.clone(), Style::default().fg(Color::Cyan).bold()),
    ]);
    f.render_widget(Paragraph::new(label), rows[0]);

    let pct = match d.total {
        Some(total) if total > 0 => ((d.bytes * 100) / total).min(100) as u16,
        _ => 0,
    };
    let label = match d.total {
        Some(total) => format!("{:>3}%  {} / {} KiB", pct, d.bytes / 1024, total / 1024),
        None => format!("{} KiB", d.bytes / 1024),
    };
    let g = Gauge::default()
        .gauge_style(Style::default().fg(Color::Green))
        .percent(pct)
        .label(label);
    f.render_widget(g, rows[1]);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

// ─────────────────────────────────────────────────────────────────────────
// Theme resolution
// ─────────────────────────────────────────────────────────────────────────

pub(super) struct ResolvedTheme {
    pub border: Color,
    pub secondary_border: Color,
    pub title_translation: Color,
    pub title_book: Color,
    pub title_chapter: Color,
    pub verse_number: Color,
    pub verse_number_super: Color,
    pub current_verse_fg: Color,
    pub current_verse_bg: Color,
    pub status_bar_dim: Color,
}

fn resolve_theme(s: &Settings) -> ResolvedTheme {
    match s.theme.preset {
        ThemePreset::Default => ResolvedTheme {
            border: Color::Indexed(24),
            secondary_border: Color::Magenta,
            title_translation: Color::Magenta,
            title_book: Color::Cyan,
            title_chapter: Color::Yellow,
            verse_number: Color::Indexed(244),
            verse_number_super: Color::Indexed(244),
            current_verse_fg: Color::Black,
            current_verse_bg: Color::Yellow,
            status_bar_dim: Color::Indexed(244),
        },
        // Solarized Dark accent palette — base16 yellow/magenta/cyan/blue.
        ThemePreset::SolarizedDark => ResolvedTheme {
            border: Color::Indexed(33),     // blue
            secondary_border: Color::Indexed(125), // magenta
            title_translation: Color::Indexed(125),
            title_book: Color::Indexed(37), // cyan
            title_chapter: Color::Indexed(136), // yellow
            verse_number: Color::Indexed(243), // base01
            verse_number_super: Color::Indexed(243),
            current_verse_fg: Color::Indexed(235), // base02
            current_verse_bg: Color::Indexed(136),
            status_bar_dim: Color::Indexed(243),
        },
        ThemePreset::HighContrast => ResolvedTheme {
            border: Color::White,
            secondary_border: Color::White,
            title_translation: Color::White,
            title_book: Color::White,
            title_chapter: Color::White,
            verse_number: Color::White,
            verse_number_super: Color::Gray,
            current_verse_fg: Color::Black,
            current_verse_bg: Color::White,
            status_bar_dim: Color::Gray,
        },
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Settings modal
// ─────────────────────────────────────────────────────────────────────────

fn draw_settings(f: &mut Frame, app: &App, area: Rect) {
    let theme = resolve_theme(&app.settings);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    draw_top_bar(f, app, rows[0], &theme);

    // 60/40 split: live-preview pane on the left, settings list on the right.
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(rows[1]);

    if let Some(b) = app.bible.as_ref() {
        draw_chapter_pane(f, app, panes[0], b, false, &theme);
    } else {
        draw_welcome(f, panes[0]);
    }
    draw_settings_panel(f, app, panes[1]);

    let hint = hint_line(&[
        ("↑↓", "move"),
        ("←→", "change option"),
        ("Esc", "save & close"),
    ]);
    f.render_widget(hint, rows[2]);
}

fn draw_settings_panel(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Indexed(33)))
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled("Settings", Style::default().fg(Color::Indexed(33)).bold()),
            Span::raw(" "),
        ]));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let layout = settings_layout();
    // Map cursor index (in items only) to the row in the layout list.
    let mut cursor_layout_row = 0usize;
    let mut item_seen = 0usize;
    for (i, row) in layout.iter().enumerate() {
        if matches!(row, SettingsRow::Item(_)) {
            if item_seen == app.settings_cursor {
                cursor_layout_row = i;
                break;
            }
            item_seen += 1;
        }
    }

    let lines: Vec<Line> = layout
        .iter()
        .enumerate()
        .map(|(i, row)| match row {
            SettingsRow::Header(h) => Line::from(vec![
                Span::raw(" "),
                Span::styled(
                    h.to_string(),
                    Style::default()
                        .fg(Color::Indexed(33))
                        .bold()
                        .add_modifier(Modifier::UNDERLINED),
                ),
            ]),
            SettingsRow::Item(it) => {
                let selected = i == cursor_layout_row;
                let label = it.label();
                let value = it.value(&app.settings);
                let row_style = if selected {
                    Style::default().bg(Color::Indexed(236))
                } else {
                    Style::default()
                };
                let label_style = if selected {
                    Style::default().fg(Color::White).bold()
                } else {
                    Style::default()
                };
                let value_style = if selected {
                    Style::default().fg(Color::Yellow).bold()
                } else {
                    Style::default().fg(Color::Cyan)
                };
                let arrows = if selected { "‹  ›" } else { "    " };
                Line::from(vec![
                    Span::styled("  ", row_style),
                    Span::styled(format!("{:<22}", label), label_style.patch(row_style)),
                    Span::styled(value, value_style.patch(row_style)),
                    Span::raw("  "),
                    Span::styled(
                        arrows.to_string(),
                        Style::default()
                            .add_modifier(Modifier::DIM)
                            .patch(row_style),
                    ),
                ])
                .style(row_style)
            }
        })
        .collect();
    // No wrap — long values are truncated cleanly at the right edge so the
    // value-column alignment is preserved.
    f.render_widget(Paragraph::new(lines), inner);
}

#[derive(Debug, Clone, Copy)]
enum SettingsRow {
    Header(&'static str),
    Item(SettingItem),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SettingItem {
    JustifyText,
    WordPadding,
    VerseSpacing,
    LineSpacing,
    VerseNumberStyle,
    PaddingDefault,
    PaddingTamil,
    PaddingDevanagari,
    PaddingArabic,
    PaddingHebrew,
    PaddingCjk,
    ThemePreset,
    MaxColumns,
    DefaultTranslation,
    ParallelDivider,
}

const fn settings_layout() -> &'static [SettingsRow] {
    &[
        SettingsRow::Header("Typography"),
        SettingsRow::Item(SettingItem::JustifyText),
        SettingsRow::Item(SettingItem::WordPadding),
        SettingsRow::Item(SettingItem::VerseSpacing),
        SettingsRow::Item(SettingItem::LineSpacing),
        SettingsRow::Item(SettingItem::VerseNumberStyle),
        SettingsRow::Header("Letter padding (per script)"),
        SettingsRow::Item(SettingItem::PaddingDefault),
        SettingsRow::Item(SettingItem::PaddingTamil),
        SettingsRow::Item(SettingItem::PaddingDevanagari),
        SettingsRow::Item(SettingItem::PaddingArabic),
        SettingsRow::Item(SettingItem::PaddingHebrew),
        SettingsRow::Item(SettingItem::PaddingCjk),
        SettingsRow::Header("Theme"),
        SettingsRow::Item(SettingItem::ThemePreset),
        SettingsRow::Header("Reader"),
        SettingsRow::Item(SettingItem::MaxColumns),
        SettingsRow::Item(SettingItem::DefaultTranslation),
        SettingsRow::Header("Parallel"),
        SettingsRow::Item(SettingItem::ParallelDivider),
    ]
}

pub(super) const SETTINGS_ITEMS: &[SettingItem] = &[
    SettingItem::JustifyText,
    SettingItem::WordPadding,
    SettingItem::VerseSpacing,
    SettingItem::LineSpacing,
    SettingItem::VerseNumberStyle,
    SettingItem::PaddingDefault,
    SettingItem::PaddingTamil,
    SettingItem::PaddingDevanagari,
    SettingItem::PaddingArabic,
    SettingItem::PaddingHebrew,
    SettingItem::PaddingCjk,
    SettingItem::ThemePreset,
    SettingItem::MaxColumns,
    SettingItem::DefaultTranslation,
    SettingItem::ParallelDivider,
];

impl SettingItem {
    fn label(&self) -> &'static str {
        match self {
            SettingItem::JustifyText => "Justify text",
            SettingItem::WordPadding => "Word padding",
            SettingItem::VerseSpacing => "Verse spacing",
            SettingItem::LineSpacing => "Line spacing",
            SettingItem::VerseNumberStyle => "Verse numbers",
            SettingItem::PaddingDefault => "  default",
            SettingItem::PaddingTamil => "  Tamil",
            SettingItem::PaddingDevanagari => "  Devanagari",
            SettingItem::PaddingArabic => "  Arabic",
            SettingItem::PaddingHebrew => "  Hebrew",
            SettingItem::PaddingCjk => "  CJK",
            SettingItem::ThemePreset => "Preset",
            SettingItem::MaxColumns => "Max columns",
            SettingItem::DefaultTranslation => "Default translation",
            SettingItem::ParallelDivider => "Divider",
        }
    }

    fn value(&self, s: &Settings) -> String {
        match self {
            SettingItem::JustifyText => {
                if s.typography.justify { "on".to_string() } else { "off".to_string() }
            }
            SettingItem::WordPadding => format!("+{}", s.typography.word_padding),
            SettingItem::VerseSpacing => format!("{} line(s)", s.typography.verse_spacing),
            SettingItem::LineSpacing => format!("{} line(s)", s.typography.line_spacing),
            SettingItem::VerseNumberStyle => match s.typography.verse_number_style {
                VerseNumberStyle::InlineBold => "inline-bold".to_string(),
                VerseNumberStyle::Superscript => "superscript".to_string(),
                VerseNumberStyle::Hidden => "hidden".to_string(),
            },
            SettingItem::PaddingDefault => {
                format!("+{}", s.typography.script_letter_padding.default)
            }
            SettingItem::PaddingTamil => {
                format!("+{}", s.typography.script_letter_padding.tamil)
            }
            SettingItem::PaddingDevanagari => {
                format!("+{}", s.typography.script_letter_padding.devanagari)
            }
            SettingItem::PaddingArabic => {
                format!("+{}", s.typography.script_letter_padding.arabic)
            }
            SettingItem::PaddingHebrew => {
                format!("+{}", s.typography.script_letter_padding.hebrew)
            }
            SettingItem::PaddingCjk => {
                format!("+{}", s.typography.script_letter_padding.cjk)
            }
            SettingItem::ThemePreset => match s.theme.preset {
                ThemePreset::Default => "default".to_string(),
                ThemePreset::SolarizedDark => "solarized-dark".to_string(),
                ThemePreset::HighContrast => "high-contrast".to_string(),
            },
            SettingItem::MaxColumns => match s.reader.max_columns {
                0 => "no cap".to_string(),
                n => n.to_string(),
            },
            SettingItem::DefaultTranslation => {
                if s.reader.default_translation.is_empty() {
                    "(first installed)".to_string()
                } else {
                    s.reader.default_translation.clone()
                }
            }
            SettingItem::ParallelDivider => match s.parallel.divider {
                DividerStyle::Single => "single".to_string(),
                DividerStyle::Double => "double".to_string(),
                DividerStyle::None => "none".to_string(),
            },
        }
    }

    pub(super) fn next(&self, app: &mut App) {
        self.shift(app, 1);
    }

    pub(super) fn prev(&self, app: &mut App) {
        self.shift(app, -1);
    }

    fn shift(&self, app: &mut App, dir: i32) {
        // Explicit scope: the &mut borrow of app.settings must end before
        // the post-match block (which reads app.installed alongside writing
        // app.settings.reader.default_translation).
        {
        let s = &mut app.settings;
        match self {
            SettingItem::JustifyText => {
                // Bool toggle — direction doesn't matter, h/l/Enter all flip.
                s.typography.justify = !s.typography.justify;
            }
            SettingItem::WordPadding => {
                s.typography.word_padding = clamp_u8(s.typography.word_padding, dir, 0, 3);
            }
            SettingItem::VerseSpacing => {
                s.typography.verse_spacing = clamp_u8(s.typography.verse_spacing, dir, 0, 2);
            }
            SettingItem::LineSpacing => {
                s.typography.line_spacing = clamp_u8(s.typography.line_spacing, dir, 0, 1);
            }
            SettingItem::VerseNumberStyle => {
                s.typography.verse_number_style = cycle_verse_style(
                    s.typography.verse_number_style,
                    dir,
                );
            }
            SettingItem::PaddingDefault => {
                s.typography.script_letter_padding.default =
                    clamp_u8(s.typography.script_letter_padding.default, dir, 0, 3);
            }
            SettingItem::PaddingTamil => {
                s.typography.script_letter_padding.tamil =
                    clamp_u8(s.typography.script_letter_padding.tamil, dir, 0, 3);
            }
            SettingItem::PaddingDevanagari => {
                s.typography.script_letter_padding.devanagari =
                    clamp_u8(s.typography.script_letter_padding.devanagari, dir, 0, 3);
            }
            SettingItem::PaddingArabic => {
                s.typography.script_letter_padding.arabic =
                    clamp_u8(s.typography.script_letter_padding.arabic, dir, 0, 3);
            }
            SettingItem::PaddingHebrew => {
                s.typography.script_letter_padding.hebrew =
                    clamp_u8(s.typography.script_letter_padding.hebrew, dir, 0, 3);
            }
            SettingItem::PaddingCjk => {
                s.typography.script_letter_padding.cjk =
                    clamp_u8(s.typography.script_letter_padding.cjk, dir, 0, 3);
            }
            SettingItem::ThemePreset => {
                s.theme.preset = cycle_theme(s.theme.preset, dir);
            }
            SettingItem::MaxColumns => {
                s.reader.max_columns = step_max_columns(s.reader.max_columns, dir);
            }
            SettingItem::ParallelDivider => {
                s.parallel.divider = cycle_divider(s.parallel.divider, dir);
            }
            // Computed below after the &mut borrow on settings ends.
            SettingItem::DefaultTranslation => {}
        }
        }
        // DefaultTranslation needs to read app.installed alongside the
        // settings write — handle it here, after the `s` borrow has ended.
        if matches!(self, SettingItem::DefaultTranslation) {
            let cur = app.settings.reader.default_translation.clone();
            app.settings.reader.default_translation =
                cycle_default_translation(&cur, &app.installed, dir);
        }
    }
}

fn clamp_u8(v: u8, dir: i32, min: u8, max: u8) -> u8 {
    let next = v as i32 + dir;
    next.clamp(min as i32, max as i32) as u8
}

fn step_max_columns(v: u16, dir: i32) -> u16 {
    // Step in 5-column increments. Below 40 → "no cap" (0). Cap at 200.
    if dir < 0 {
        if v == 0 {
            return 0;
        }
        let next = v.saturating_sub(5);
        if next < 40 { 0 } else { next }
    } else {
        if v == 0 {
            return 60;
        }
        (v + 5).min(200)
    }
}

fn cycle_verse_style(v: VerseNumberStyle, dir: i32) -> VerseNumberStyle {
    let order = [
        VerseNumberStyle::InlineBold,
        VerseNumberStyle::Superscript,
        VerseNumberStyle::Hidden,
    ];
    cycle_in(&order, v, dir)
}

fn cycle_theme(v: ThemePreset, dir: i32) -> ThemePreset {
    let order = [
        ThemePreset::Default,
        ThemePreset::SolarizedDark,
        ThemePreset::HighContrast,
    ];
    cycle_in(&order, v, dir)
}

fn cycle_divider(v: DividerStyle, dir: i32) -> DividerStyle {
    let order = [DividerStyle::Single, DividerStyle::Double, DividerStyle::None];
    cycle_in(&order, v, dir)
}

fn cycle_in<T: Copy + PartialEq>(order: &[T], v: T, dir: i32) -> T {
    let n = order.len() as i32;
    let pos = order.iter().position(|x| *x == v).unwrap_or(0) as i32;
    let next = (((pos + dir) % n) + n) % n;
    order[next as usize]
}

fn cycle_default_translation(
    cur: &str,
    installed: &[crate::bible::TranslationInfo],
    dir: i32,
) -> String {
    if installed.is_empty() {
        return String::new();
    }
    // States: "" (first installed) → installed[0].id → ... → installed[n-1].id → "" → ...
    let mut states: Vec<String> = Vec::with_capacity(installed.len() + 1);
    states.push(String::new());
    for t in installed {
        states.push(t.id.clone());
    }
    let pos = states
        .iter()
        .position(|s| s == cur)
        .unwrap_or(0) as i32;
    let n = states.len() as i32;
    let next = (((pos + dir) % n) + n) % n;
    states[next as usize].clone()
}
