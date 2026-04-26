use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Gauge, List, ListItem, Paragraph, Widget, Wrap};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::reference::book_display;
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
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    draw_top_bar(f, app, rows[0]);
    if app.mode == Mode::NoTranslation || app.bible.is_none() {
        draw_welcome(f, rows[1]);
    } else if app.parallel && app.secondary_bible.is_some() {
        let panes = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(rows[1]);
        let primary = app.bible.as_ref().unwrap();
        draw_chapter_pane(f, app, panes[0], primary, false);
        if let Some(sec) = app.secondary_bible.as_ref() {
            draw_chapter_pane(f, app, panes[1], sec, true);
        }
    } else {
        let primary = app.bible.as_ref().unwrap();
        draw_chapter_pane(f, app, rows[1], primary, false);
    }
    draw_bottom_bar(f, app, rows[2]);
}

fn draw_top_bar(f: &mut Frame, app: &App, area: Rect) {
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
            Style::default().bg(Color::Indexed(24)).fg(Color::White).bold(),
        ),
        Span::raw(" "),
        Span::styled(translation, Style::default().fg(Color::Cyan).bold()),
        Span::raw(" │ "),
        Span::styled(position, Style::default().fg(Color::Yellow)),
        Span::raw("   "),
        Span::styled(
            "?",
            Style::default().add_modifier(Modifier::DIM),
        ),
        Span::styled(" help", Style::default().add_modifier(Modifier::DIM)),
        Span::raw("  "),
        Span::styled(
            "T",
            Style::default().add_modifier(Modifier::DIM),
        ),
        Span::styled(
            " translations",
            Style::default().add_modifier(Modifier::DIM),
        ),
        Span::raw("  "),
        Span::styled(
            "q",
            Style::default().add_modifier(Modifier::DIM),
        ),
        Span::styled(" quit", Style::default().add_modifier(Modifier::DIM)),
    ]);
    // Render Line directly (not via Paragraph). Line is a Widget in
    // ratatui 0.30 and is documented to "always render as a single line",
    // truncating at the right edge — no chance of consuming extra rows.
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
) {
    let Some(cr) = app.current.as_ref() else {
        return;
    };

    let border_color = if is_secondary {
        Color::Magenta
    } else {
        Color::Indexed(24)
    };

    let mut title_spans = vec![Span::raw(" ")];
    if app.parallel || is_secondary {
        title_spans.push(Span::styled(
            bible.translation.id.clone(),
            Style::default().fg(Color::Magenta).bold(),
        ));
        title_spans.push(Span::raw(" │ "));
    }
    title_spans.push(Span::styled(
        book_display(&cr.book()),
        Style::default().fg(Color::Cyan).bold(),
    ));
    title_spans.push(Span::raw(" "));
    title_spans.push(Span::styled(
        cr.chapter().to_string(),
        Style::default().fg(Color::Yellow).bold(),
    ));
    title_spans.push(Span::raw(" "));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
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
    let prefix_w: usize = 4; // "{:>3} "
    let avail = (inner.width as usize).saturating_sub(prefix_w).max(1);
    let rows = build_rows(chapter, avail, highlight_verse);

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
        let num_style = if row.highlighted {
            Style::default().fg(Color::Black).bg(Color::Yellow).bold()
        } else {
            Style::default().fg(Color::Indexed(244))
        };
        let text_style = if row.highlighted {
            Style::default().bold()
        } else {
            Style::default()
        };
        f.render_widget(
            ChapterRow {
                prefix: &row.prefix,
                prefix_style: num_style,
                content: &row.content,
                content_style: text_style,
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
            },
            row_area,
        );
    }
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
///
/// Untouched cells past the content stay as the explicit space we wrote in
/// step 1, so trailing artifacts can't survive.
struct ChapterRow<'a> {
    prefix: &'a str,
    prefix_style: Style,
    content: &'a str,
    content_style: Style,
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
        // Prefix is always ASCII (verse numbers + spaces), so a plain
        // set_string is fine.
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
            );
        }
    }
}

/// Write `text` into the buffer one grapheme cluster at a time, advancing
/// by `display_width()` columns per grapheme. Trailing cells of multi-cell
/// graphemes get `set_skip(true)` so the diff renderer doesn't emit a write
/// for them and the terminal can render the glyph across both cells.
fn write_graphemes(buf: &mut Buffer, mut x: u16, y: u16, x_end: u16, text: &str, style: Style) {
    for g in UnicodeSegmentation::graphemes(text, true) {
        let w = display_width(g) as u16;
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
///
/// Trade-off: if the font *is* narrow, we leave a visible gap; if it's
/// wide, spacing is correct. Either way, no overlap.
fn display_width(g: &str) -> usize {
    let raw = UnicodeWidthStr::width(g);
    if raw == 0 {
        return 0;
    }
    if g.bytes().all(|b| b < 0x80) {
        return raw;
    }
    if g.chars().any(is_wide_extending_mark) {
        return raw.max(3);
    }
    raw.max(2)
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
}

impl Row {
    fn blank() -> Self {
        Row {
            highlighted: false,
            prefix: String::new(),
            content: String::new(),
            verse: None,
        }
    }
}

/// Build the wrapped, prefix-decorated row list for a chapter at a given
/// inner content width. Used by both single-pane and parallel-pane render.
fn build_rows(
    chapter: &crate::bible::Chapter,
    avail: usize,
    highlight_verse: Option<u16>,
) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::new();
    rows.push(Row::blank());
    for verse in &chapter.verses {
        let highlighted = Some(verse.number) == highlight_verse;
        let wrapped = wrap_to_width(&verse.text, avail);
        let pieces = if wrapped.is_empty() {
            vec![String::new()]
        } else {
            wrapped
        };
        for (i, content) in pieces.into_iter().enumerate() {
            let prefix = if i == 0 {
                format!("{:>3} ", verse.number)
            } else {
                "    ".to_string()
            };
            rows.push(Row {
                highlighted,
                prefix,
                content,
                verse: Some(verse.number),
            });
        }
    }
    rows
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
/// grapheme.
fn wrap_to_width(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![text.to_string()];
    }
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut cur_w: usize = 0;

    for word in text.split_whitespace() {
        let word_w: usize = UnicodeSegmentation::graphemes(word, true)
            .map(display_width)
            .sum();
        let needed = if cur.is_empty() {
            word_w
        } else {
            cur_w + 1 + word_w
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
                let gw = display_width(g);
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
            cur.push(' ');
            cur_w += 1;
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
    lines
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

fn draw_bottom_bar(f: &mut Frame, app: &App, area: Rect) {
    // Active search overrides the normal status display so the spinner is
    // always visible while a worker is scanning.
    if let Some(s) = app.searching.as_ref() {
        let frame = SPINNER_FRAMES[s.spinner % SPINNER_FRAMES.len()];
        let elapsed = s.started_at.elapsed().as_millis();
        let line = Line::from(vec![
            Span::raw(" "),
            Span::styled(
                frame.to_string(),
                Style::default().fg(Color::Yellow).bold(),
            ),
            Span::raw(" searching for "),
            Span::styled(
                format!("`{}`", sanitize_one_line(&s.query)),
                Style::default().fg(Color::Magenta).bold(),
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
            Span::styled(":", Style::default().fg(Color::Cyan).bold()),
            Span::raw(sanitize_one_line(app.input.value())),
            Span::styled("│", Style::default().fg(Color::Cyan)),
        ]),
        Mode::Search => Line::from(vec![
            Span::styled("/", Style::default().fg(Color::Magenta).bold()),
            Span::raw(sanitize_one_line(app.input.value())),
            Span::styled("│", Style::default().fg(Color::Magenta)),
        ]),
        _ => {
            if !app.status.is_empty() {
                Line::from(Span::styled(
                    sanitize_one_line(&app.status),
                    Style::default().fg(Color::Indexed(244)),
                ))
            } else {
                Line::from(Span::styled(
                    " :ref  /search  hjkl move  q quit  ? help ",
                    Style::default().add_modifier(Modifier::DIM),
                ))
            }
        }
    };
    // Render Line directly. Line implements Widget and is documented to
    // always render as one row, clipping (not wrapping) at the right edge,
    // and to strip newlines on construction — so the bottom bar is
    // structurally guaranteed to occupy exactly `area.height` (= 1) rows
    // regardless of what's in the status string.
    f.render_widget(line, area);
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

    let hint = Line::from(Span::styled(
        " Enter install/uninstall  r refresh  jk move  Esc back ",
        Style::default().add_modifier(Modifier::DIM),
    ));
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

    let hint = Line::from(Span::styled(
        " Enter jump  d delete  jk move  Esc back ",
        Style::default().add_modifier(Modifier::DIM),
    ));
    f.render_widget(Paragraph::new(hint), rows[1]);
}

fn draw_help_overlay(f: &mut Frame, area: Rect) {
    let popup = centered_rect(60, 60, area);
    f.render_widget(Clear, popup);
    let block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let lines = vec![
        Line::from(Span::styled("Reading", Style::default().bold().fg(Color::Yellow))),
        Line::from("  j / k          scroll line"),
        Line::from("  Ctrl-d / Ctrl-u half-page"),
        Line::from("  gg / G         top / bottom of chapter"),
        Line::from("  h / l          previous / next chapter"),
        Line::from("  H / L          previous / next book"),
        Line::from(""),
        Line::from(Span::styled("Lookup", Style::default().bold().fg(Color::Yellow))),
        Line::from("  :              jump to reference (e.g. :John 3:16)"),
        Line::from("  /              search current translation"),
        Line::from("  ↑ / ↓          (in : or /) browse command history"),
        Line::from("  n / N          next / previous search match"),
        Line::from(""),
        Line::from(Span::styled("Bookmarks", Style::default().bold().fg(Color::Yellow))),
        Line::from("  b              bookmark current chapter"),
        Line::from("  :b N <note>    bookmark verse N with optional note"),
        Line::from("  B              open bookmarks list  (Enter jump, d delete)"),
        Line::from(""),
        Line::from(Span::styled("Translations", Style::default().bold().fg(Color::Yellow))),
        Line::from("  T              translation manager"),
        Line::from("  t              cycle installed translations"),
        Line::from("  |              toggle parallel view (two translations side-by-side)"),
        Line::from("  \\              swap the parallel-view secondary translation"),
        Line::from(""),
        Line::from(Span::styled("Misc", Style::default().bold().fg(Color::Yellow))),
        Line::from("  ?              this help (Esc to close)"),
        Line::from("  q              quit"),
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
