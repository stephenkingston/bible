//! Frame drawing: the reading pane (single, two-column and parallel) and the
//! full-screen views. The header, status line and overlays live in `chrome`;
//! text layout in `rows`; colours in `theme`.

use std::sync::Arc;

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Gauge, List, ListItem, Padding, Paragraph, Wrap};

use crate::bible::TranslationInfo;
use crate::reference::book_display;
use crate::settings::{
    ColumnLayout, DividerStyle, FULL_WIDTH, ReadingLayout, Settings, ThemePreset, VerseNumberStyle,
};
use crate::storage;

use super::chrome::{self, hint_line, sanitize_one_line, truncate};
use super::rows::{self, Row, Seg, SegKind};
use super::theme::{self, Edge, Theme};
use super::{App, ClickZone, Mode};

/// Reading pane width when the width setting is "auto".
const AUTO_WIDTH: u16 = 96;
/// Narrowest body the theme-preferred two-column layout kicks in at.
const TWO_COLUMN_MIN: u16 = 96;
const COLUMN_GAP: u16 = 5;

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let theme = theme::resolve(app.settings.theme.preset);

    // Wipe the frame area at the top of every draw. ratatui's Buffer marks
    // the second column of a wide unicode glyph as a "skip" cell; if the
    // next frame writes a 1-column ASCII char in its place, the skip cell
    // is left dangling and renders as a stray glyph. Switching from
    // Tamil/CJK to English exposes this. `Clear` calls Cell::reset on
    // every position, killing the skip markers; the terminal's diff
    // renderer means unchanged cells still aren't re-transmitted.
    f.render_widget(Clear, area);
    f.render_widget(Block::default().style(theme.base()), area);
    app.click_zones.clear();

    match app.mode {
        Mode::Manager => draw_manager(f, app, area, &theme),
        Mode::Bookmarks => draw_bookmarks(f, app, area, &theme),
        Mode::Settings => draw_settings(f, app, area, &theme),
        Mode::EditingNote => draw_note_editor_split(f, app, area, &theme),
        Mode::Plan => draw_plan_view(f, app, area, &theme),
        _ => draw_reader(f, app, area, &theme),
    }

    let overlay = matches!(
        app.mode,
        Mode::Help | Mode::Palette | Mode::PickBook | Mode::PickSecondary
    );
    if overlay || app.download.is_some() {
        chrome::fade(f.buffer_mut(), area, &theme);
    }
    if overlay && area.height > 0 {
        // The status line stays legible: it names the mode and the way out.
        let foot = Rect::new(area.x, area.bottom() - 1, area.width, 1);
        chrome::draw_footer(f, app, foot, &theme);
    }
    match app.mode {
        Mode::Help => chrome::draw_keys(f, area, &theme),
        Mode::Palette => chrome::draw_palette(f, app, area, &theme),
        Mode::PickBook => draw_book_picker(f, app, area, &theme),
        Mode::PickSecondary => draw_pick_secondary(f, app, area, &theme),
        _ => {}
    }
    if app.download.is_some() {
        draw_download_popup(f, app, area, &theme);
    }
}

fn draw_reader(f: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let rows = Layout::vertical([
        Constraint::Length(chrome::header_height(theme)),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(area);
    let body = rows[1];
    let parallel = app.parallel && app.secondary_bible.is_some();
    let two = !parallel && two_columns(&app.settings, theme, body.width);
    let pane = reading_pane(body, &app.settings, parallel || two);

    chrome::draw_header(f, app, rows[0], pane, theme);
    if app.mode == Mode::NoTranslation || app.bible.is_none() {
        chrome::draw_welcome(f, body, theme);
    } else if parallel {
        draw_parallel(f, app, pane, theme);
    } else {
        draw_chapter(f, app, pane, theme, two);
    }
    chrome::draw_footer(f, app, rows[2], theme);
}

/// Narrow `body` to the reading-width setting, centred. `wide` doubles the
/// cap for two columns or parallel panes.
fn reading_pane(body: Rect, settings: &Settings, wide: bool) -> Rect {
    let cap = match settings.reader.max_columns {
        0 => AUTO_WIDTH,
        FULL_WIDTH => return body,
        n => n,
    };
    let cap = if wide { cap.saturating_mul(2) } else { cap };
    if body.width <= cap {
        return body;
    }
    Rect::new(body.x + (body.width - cap) / 2, body.y, cap, body.height)
}

fn two_columns(settings: &Settings, theme: &Theme, width: u16) -> bool {
    match settings.reader.columns {
        ColumnLayout::One => false,
        ColumnLayout::Two => width >= 2 * 30 + COLUMN_GAP + 8,
        ColumnLayout::Auto => theme.prefers_columns && width >= TWO_COLUMN_MIN,
    }
}

pub(super) fn uses_prose(settings: &Settings, theme: &Theme) -> bool {
    match settings.reader.layout {
        ReadingLayout::Verses => false,
        ReadingLayout::Prose => true,
        ReadingLayout::Auto => theme.prefers_prose,
    }
}

/// Title for a framed pane: "John 1", or "KJV │ John 1" in parallel.
fn pane_title(app: &App, theme: &Theme, translation: Option<&TranslationInfo>) -> Line<'static> {
    let Some(cr) = app.current.as_ref() else {
        return Line::default();
    };
    let book = book_display(&cr.book());
    let ch: u32 = cr.chapter().into();
    let mut spans = vec![Span::raw(" ")];
    if let Some(t) = translation {
        spans.push(Span::styled(
            chrome::short_name(t),
            Style::default().fg(theme.accent2).bold(),
        ));
        spans.push(Span::raw(" │ "));
    }
    if theme.spaced {
        spans.push(Span::styled(
            format!("{}   {ch}", theme.heading(book)),
            Style::default().fg(theme.title).bold(),
        ));
        spans.push(Span::raw(" "));
        return Line::from(spans).centered();
    }
    spans.push(Span::styled(
        book.to_string(),
        Style::default().fg(theme.title).bold(),
    ));
    spans.push(Span::raw(" "));
    spans.push(Span::styled(
        ch.to_string(),
        Style::default().fg(theme.title2).bold(),
    ));
    spans.push(Span::raw(" "));
    Line::from(spans)
}

/// Draw the pane's frame, or leave margins for borderless themes. Returns
/// the text area, the column for the scroll indicator, and whether framed.
fn frame_pane(
    f: &mut Frame,
    pane: Rect,
    theme: &Theme,
    title: Line<'static>,
    border: Color,
) -> (Rect, u16, bool) {
    if theme.frame == Edge::None {
        let inner = Rect::new(
            pane.x + 2,
            pane.y,
            pane.width.saturating_sub(5),
            pane.height,
        );
        return (inner, pane.right().saturating_sub(2), false);
    }
    let block = theme
        .block(theme.frame, border)
        .padding(Padding::new(1, 3, 0, 1))
        .title(title);
    let inner = block.inner(pane);
    f.render_widget(block, pane);
    (inner, pane.right().saturating_sub(1), true)
}

/// The single reading pane, in one or two columns.
fn draw_chapter(f: &mut Frame, app: &mut App, pane: Rect, theme: &Theme, two: bool) {
    let Some(bible) = app.bible.as_ref().map(Arc::clone) else {
        return;
    };
    let Some(cr) = app.current.clone() else {
        return;
    };
    let title = pane_title(app, theme, None);
    let (inner, bar_x, framed) = frame_pane(f, pane, theme, title, theme.border);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let Some(chapter) = bible.get_chapter(&cr) else {
        let y = inner.y + u16::from(inner.height > 1);
        f.render_widget(
            Span::styled("(chapter not present in this translation)", theme.dim()),
            Rect::new(inner.x, y, inner.width, 1),
        );
        return;
    };

    let ncols: u16 = if two && inner.width >= 2 * 20 + COLUMN_GAP {
        2
    } else {
        1
    };
    let col_w = if ncols == 2 {
        (inner.width - COLUMN_GAP) / 2
    } else {
        inner.width
    };
    let settings = &app.settings;
    let rows = if uses_prose(settings, theme) {
        rows::prose_rows(chapter, col_w as usize, settings, settings.reader.drop_cap)
    } else {
        let gutter = rows::verse_prefix_width(settings.typography.verse_number_style);
        rows::verse_rows(
            chapter,
            (col_w as usize).saturating_sub(gutter).max(1),
            settings,
        )
    };

    let focus = app.focus_verse.max(1);
    let hit = highlighted_verse_for(app, &cr);
    let height = inner.height as usize;
    let visible = height * ncols as usize;
    let start = rows::compute_scroll(&rows, focus, visible, app.scroll as usize, app.pin_focus);
    let mut zones = Vec::new();
    for c in 0..ncols {
        let col = Rect::new(
            inner.x + c * (col_w + COLUMN_GAP),
            inner.y,
            col_w,
            inner.height,
        );
        let from = (start + c as usize * height).min(rows.len());
        let to = (from + height).min(rows.len());
        render_rows(
            f.buffer_mut(),
            &rows[from..to],
            col,
            focus,
            hit,
            theme,
            settings,
            &mut zones,
        );
    }
    if ncols == 2 {
        let x = inner.x + col_w + COLUMN_GAP / 2;
        for y in inner.top()..inner.bottom() {
            if let Some(cell) = f.buffer_mut().cell_mut((x, y)) {
                cell.set_symbol("│")
                    .set_style(Style::default().fg(theme.faint));
            }
        }
    }
    scrollbar(
        f.buffer_mut(),
        bar_x,
        inner.y,
        inner.height,
        rows.len(),
        start,
        visible,
        theme,
        framed,
    );

    app.scroll = start as u16;
    app.pin_focus = false;
    app.click_zones.extend(zones);
}

/// Two translations side by side, each pane starting at the focused verse.
fn draw_parallel(f: &mut Frame, app: &mut App, pane: Rect, theme: &Theme) {
    let (Some(primary), Some(secondary)) = (
        app.bible.as_ref().map(Arc::clone),
        app.secondary_bible.as_ref().map(Arc::clone),
    ) else {
        return;
    };
    let Some(cr) = app.current.clone() else {
        return;
    };
    let halves =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).split(pane);
    let focus = app.focus_verse.max(1);
    let hit = highlighted_verse_for(app, &cr);
    let divider = app.settings.parallel.divider;
    let mut zones = Vec::new();

    for (i, bible) in [&primary, &secondary].into_iter().enumerate() {
        let half = halves[i];
        let label_color = if i == 0 { theme.title } else { theme.accent2 };
        let inner = if theme.frame == Edge::None {
            // Borderless: a label row, then text; the divider sits on the
            // right half's first column.
            let label = vec![
                Span::styled(
                    chrome::short_name(&bible.translation),
                    Style::default().fg(label_color).bold(),
                ),
                Span::raw("  "),
                Span::styled(
                    truncate(
                        &bible.translation.display_name,
                        half.width.saturating_sub(14) as usize,
                    ),
                    theme.dim(),
                ),
            ];
            f.render_widget(
                Line::from(label),
                Rect::new(half.x + 2, half.y, half.width.saturating_sub(4), 1),
            );
            Rect::new(
                half.x + 2,
                half.y + 2,
                half.width.saturating_sub(5),
                half.height.saturating_sub(2),
            )
        } else {
            let border = if i == 0 { theme.border } else { theme.accent2 };
            frame_pane(
                f,
                half,
                theme,
                pane_title(app, theme, Some(&bible.translation)),
                border,
            )
            .0
        };
        if inner.width == 0 || inner.height == 0 {
            continue;
        }
        let Some(chapter) = bible.get_chapter(&cr) else {
            f.render_widget(
                Span::styled("(chapter not present in this translation)", theme.dim()),
                Rect::new(inner.x, inner.y, inner.width, 1),
            );
            continue;
        };
        let gutter = rows::verse_prefix_width(app.settings.typography.verse_number_style);
        let rows = rows::verse_rows(
            chapter,
            (inner.width as usize).saturating_sub(gutter).max(1),
            &app.settings,
        );
        let start = rows::first_row_for_verse(&rows, focus);
        render_rows(
            f.buffer_mut(),
            &rows[start..],
            inner,
            focus,
            hit,
            theme,
            &app.settings,
            &mut zones,
        );
    }

    if theme.frame == Edge::None {
        let glyph = match divider {
            DividerStyle::Single => "│",
            DividerStyle::Double => "║",
            DividerStyle::None => " ",
        };
        let x = halves[1].x;
        for y in pane.top()..pane.bottom() {
            if let Some(cell) = f.buffer_mut().cell_mut((x, y)) {
                cell.set_symbol(glyph)
                    .set_style(Style::default().fg(theme.faint));
            }
        }
    } else {
        apply_divider_style(f.buffer_mut(), halves[0], halves[1], divider, theme);
    }
    app.click_zones.extend(zones);
}

/// Overlay the seam between framed parallel panes. `Single` leaves both
/// touching borders (a thicker `││` line); `Double` overlays `║` and `None`
/// blanks the seam.
fn apply_divider_style(
    buf: &mut Buffer,
    left: Rect,
    right: Rect,
    style: DividerStyle,
    theme: &Theme,
) {
    let glyph = match style {
        DividerStyle::Single => return,
        DividerStyle::Double => "║",
        DividerStyle::None => " ",
    };
    let top = left.y + 1;
    let bottom = left.y + left.height.saturating_sub(1);
    for x in [left.x + left.width.saturating_sub(1), right.x] {
        for y in top..bottom {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_symbol(glyph)
                    .set_style(Style::default().fg(theme.faint));
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_rows(
    buf: &mut Buffer,
    rows: &[Row],
    col: Rect,
    focus: u16,
    hit: Option<u16>,
    theme: &Theme,
    settings: &Settings,
    zones: &mut Vec<ClickZone>,
) {
    for (i, row) in rows.iter().enumerate() {
        let y = col.y + i as u16;
        if y >= col.bottom() {
            break;
        }
        let row_bg = match row.fill {
            Some(v) if Some(v) == hit => theme.hit_bg,
            Some(v) if v == focus => theme.focus_bg,
            _ => theme.bg,
        };
        // Hard-reset each cell, including the skip flag a wide grapheme
        // from a previous frame may have left behind.
        for x in col.left()..col.right() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.reset();
                cell.set_symbol(" ")
                    .set_style(Style::default().fg(theme.fg).bg(row_bg));
            }
        }
        let mut x = col.x;
        for seg in &row.segs {
            let x0 = x;
            x = rows::write_graphemes(
                buf,
                x,
                y,
                col.right(),
                &seg.text,
                seg_style(seg, row_bg, focus, hit, theme),
                settings,
            );
            if row.fill.is_none()
                && let Some(v) = seg.verse
                && x > x0
            {
                zones.push(ClickZone {
                    y,
                    x0,
                    x1: x,
                    verse: v,
                });
            }
        }
        if let Some(v) = row.fill {
            zones.push(ClickZone {
                y,
                x0: col.x,
                x1: col.right(),
                verse: v,
            });
        }
        if let Some(mark) = theme.mark
            && row.covers(focus)
            && col.x > 0
            && let Some(cell) = buf.cell_mut((col.x - 1, y))
        {
            cell.set_symbol(mark)
                .set_style(Style::default().fg(theme.accent));
        }
    }
}

fn seg_style(seg: &Seg, row_bg: Color, focus: u16, hit: Option<u16>, theme: &Theme) -> Style {
    let is_hit = hit.is_some() && seg.verse == hit;
    let is_focus = seg.verse == Some(focus);
    let bg = if is_hit {
        theme.hit_bg
    } else if is_focus {
        theme.focus_bg
    } else {
        row_bg
    };
    let fg = match seg.kind {
        SegKind::Num | SegKind::SuperNum => theme.verse_num,
        SegKind::DropCap => theme.drop_cap,
        SegKind::Text | SegKind::Space => theme.fg,
    };
    let style = Style::default().fg(fg).bg(bg);
    let number = matches!(seg.kind, SegKind::Num | SegKind::SuperNum);
    if is_hit {
        style.fg(theme.hit_fg).add_modifier(Modifier::BOLD)
    } else if is_focus && number && theme.mark.is_some() {
        style.fg(theme.accent).add_modifier(Modifier::BOLD)
    } else {
        style
    }
}

/// A thumb on the pane's right edge showing where the viewport sits in the
/// chapter. Framed panes draw it over the border.
#[allow(clippy::too_many_arguments)]
fn scrollbar(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    h: u16,
    total: usize,
    start: usize,
    visible: usize,
    theme: &Theme,
    framed: bool,
) {
    let h = h as usize;
    if total <= visible || h == 0 {
        return;
    }
    let thumb = (visible * h / total).clamp(1, h);
    let max_start = total - visible;
    let pos = (start.min(max_start) * (h - thumb) + max_start / 2) / max_start;
    for i in 0..h {
        let on = i >= pos && i < pos + thumb;
        let Some(cell) = buf.cell_mut((x, y + i as u16)) else {
            continue;
        };
        if on {
            cell.set_symbol("┃")
                .set_style(Style::default().fg(theme.accent));
        } else if !framed {
            cell.set_symbol("│")
                .set_style(Style::default().fg(theme.faint));
        }
    }
}

fn highlighted_verse_for(app: &App, cr: &crate::reference::BibleChapterReference) -> Option<u16> {
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

/// Bordered full-screen view with a title.
fn screen_block(theme: &Theme, title: Line<'static>) -> Block<'static> {
    let edge = if theme.overlay == Edge::None {
        Edge::Plain
    } else {
        theme.overlay
    };
    theme.block(edge, theme.border).title(title)
}

fn title_line(theme: &Theme, name: &str, color: Color, extra: String) -> Line<'static> {
    Line::from(vec![
        Span::raw(" "),
        Span::styled(theme.heading(name), Style::default().fg(color).bold()),
        Span::raw(" "),
        Span::styled(extra, theme.dim()),
        Span::raw(" "),
    ])
}

/// The hint row of a full-screen view; a fresh status message takes it
/// until it expires, like the reader's status line.
fn status_or_hints(app: &App, theme: &Theme, pairs: &[(&str, &str)]) -> Line<'static> {
    if app.status.is_empty() {
        hint_line(theme, pairs)
    } else {
        Line::from(Span::styled(
            format!(" {}", sanitize_one_line(&app.status)),
            Style::default().fg(theme.fg),
        ))
    }
}

fn draw_manager(f: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let counts = format!(
        "({} available, {} installed{})",
        app.available.len(),
        app.installed.len(),
        if app.refreshing_catalog {
            ", fetching catalog…"
        } else {
            ""
        }
    );
    let block = screen_block(
        theme,
        title_line(theme, "Translations", theme.accent2, counts),
    );
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(inner);

    let filter_line = Line::from(vec![
        Span::styled(
            " filter ",
            Style::default().bg(theme.badge_bg).fg(theme.badge_fg),
        ),
        Span::raw(" "),
        Span::styled(
            app.manager_filter.value().to_string(),
            Style::default().fg(theme.fg),
        ),
        Span::styled("│", Style::default().fg(theme.accent)),
    ]);
    f.render_widget(Paragraph::new(filter_line), rows[0]);

    let indices = app.filtered_indices();
    let items: Vec<ListItem> = indices
        .iter()
        .map(|&idx| {
            let t = &app.available[idx];
            let installed = storage::is_installed(&t.id);
            let (mark, mark_style) = if installed {
                ("●", Style::default().fg(theme.ok).bold())
            } else {
                ("○", Style::default().fg(theme.dim))
            };
            ListItem::new(Line::from(vec![
                Span::raw(" "),
                Span::styled(mark.to_string(), mark_style),
                Span::raw("  "),
                Span::styled(format!("{:30}", t.id), Style::default().fg(theme.accent)),
                Span::raw("  "),
                Span::styled(t.display_name.clone(), Style::default().fg(theme.fg)),
                Span::raw("  "),
                Span::styled(format!("({})", t.language), theme.dim()),
            ]))
        })
        .collect();

    // Stateful render so ratatui auto-scrolls the list to keep the
    // highlighted row visible. Without this the cursor walks off-screen
    // for catalogs longer than the pane.
    let total = indices.len();
    let cursor = if total == 0 {
        None
    } else {
        Some(app.manager_cursor.min(total - 1))
    };
    app.manager_list_state.select(cursor);
    let list = List::new(items).highlight_style(Style::default().bg(theme.sel_bg));
    f.render_stateful_widget(list, rows[1], &mut app.manager_list_state);

    let bottom = status_or_hints(
        app,
        theme,
        &[
            ("Enter", "install/uninstall"),
            ("^R", "refresh"),
            ("↑↓", "move"),
            ("Esc", "back"),
        ],
    );
    f.render_widget(Paragraph::new(bottom), rows[2]);
}

fn draw_bookmarks(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let block = screen_block(
        theme,
        title_line(
            theme,
            "Bookmarks",
            theme.title2,
            format!("({})", app.bookmarks.len()),
        ),
    );
    let inner = block.inner(area);
    f.render_widget(block, area);

    let footer = [
        ("Enter", "jump"),
        ("e", "edit note"),
        ("d", "delete"),
        ("↑↓", "move"),
        ("PgUp/Dn", "scroll note"),
        ("Esc", "back"),
    ];

    if app.bookmarks.is_empty() {
        let rows = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(inner);
        let empty = Line::from(vec![
            Span::styled("  No bookmarks yet — press ", theme.dim()),
            theme.key_span("b"),
            Span::styled(" on a chapter, or ", theme.dim()),
            theme.key_span(":b note"),
            Span::styled(" to add one with a multi-line note.", theme.dim()),
        ]);
        f.render_widget(empty, rows[0]);
        f.render_widget(
            Paragraph::new(status_or_hints(app, theme, &footer[..5])),
            rows[1],
        );
        return;
    }

    // List on top, full note of selected bookmark below, then a footer.
    let layout = Layout::vertical([
        Constraint::Min(5),
        Constraint::Length(1),
        Constraint::Length(8),
        Constraint::Length(1),
    ])
    .split(inner);

    let items: Vec<ListItem> = app
        .bookmarks
        .iter()
        .enumerate()
        .map(|(i, bm)| {
            let book_label = crate::reference::book_from_number(bm.book_number)
                .ok()
                .map(|b| book_display(&b))
                .unwrap_or("?");
            let ref_str = match bm.verse {
                Some(v) => format!("{} {}:{}", book_label, bm.chapter, v),
                None => format!("{} {}", book_label, bm.chapter),
            };
            let row_style = if i == app.bookmarks_cursor {
                Style::default().bg(theme.sel_bg)
            } else {
                Style::default()
            };
            let note_first_line = bm.note.split('\n').next().unwrap_or("");
            let line_count = if bm.note.is_empty() {
                0
            } else {
                bm.note.split('\n').count()
            };
            let mut spans = vec![
                Span::styled(" ★ ", Style::default().fg(theme.title2)),
                Span::styled(
                    format!("{:24}", bm.translation),
                    Style::default().fg(theme.accent),
                ),
                Span::styled(" · ", theme.dim()),
                Span::styled(ref_str, Style::default().fg(theme.title2).bold()),
            ];
            if line_count > 0 {
                spans.push(Span::styled(" · ", theme.dim()));
                spans.push(Span::styled(
                    truncate(note_first_line, 60),
                    Style::default().fg(theme.fg),
                ));
                if line_count > 1 {
                    spans.push(Span::styled(
                        format!("  ({} lines)", line_count),
                        theme.dim(),
                    ));
                }
            }
            ListItem::new(Line::from(spans)).style(row_style)
        })
        .collect();
    f.render_widget(List::new(items), layout[0]);

    f.render_widget(
        Span::styled(
            "─".repeat(layout[1].width as usize),
            Style::default().fg(theme.faint),
        ),
        layout[1],
    );

    if let Some(bm) = app.bookmarks.get(app.bookmarks_cursor) {
        let (note_text, note_style) = if bm.note.is_empty() {
            ("(no note)".to_string(), theme.dim())
        } else {
            (bm.note.clone(), Style::default().fg(theme.fg))
        };
        let para = Paragraph::new(note_text)
            .style(note_style)
            .wrap(Wrap { trim: false })
            .scroll((app.bookmarks_note_scroll, 0));
        f.render_widget(para, layout[2]);
    }

    f.render_widget(
        Paragraph::new(status_or_hints(app, theme, &footer)),
        layout[3],
    );
}

/// Split-screen layout for the note-editing mode: chapter pane on the
/// left, editor pane on the right. `Tab` toggles which pane has focus;
/// the focused pane gets a brighter border and (for the editor) a
/// blinking terminal cursor.
fn draw_note_editor_split(f: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let rows = Layout::vertical([
        Constraint::Length(chrome::header_height(theme)),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(area);
    let panes =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).split(rows[1]);
    chrome::draw_header(f, app, rows[0], rows[0], theme);

    if app.bible.is_some() {
        draw_chapter(f, app, panes[0], theme, false);
    } else {
        chrome::draw_welcome(f, panes[0], theme);
    }

    let editor_focused = !app.note_editor_focus_reader;
    draw_note_editor_pane(f, app, panes[1], editor_focused, theme);

    let hint = if app.note_editor_focus_reader {
        hint_line(
            theme,
            &[
                ("Tab", "switch to editor"),
                ("↑↓", "verse"),
                ("←→", "chapter"),
                ("^S", "save note"),
                ("Esc", "cancel"),
            ],
        )
    } else {
        hint_line(
            theme,
            &[
                ("Tab", "switch to reader"),
                ("^S", "save"),
                ("Esc", "cancel"),
                ("Enter", "newline"),
                ("↑↓←→", "move"),
            ],
        )
    };
    f.render_widget(hint, rows[2]);
}

fn draw_note_editor_pane(f: &mut Frame, app: &App, area: Rect, focused: bool, theme: &Theme) {
    let Some(editor) = app.note_editor.as_ref() else {
        return;
    };
    let border_color = if focused { theme.title2 } else { theme.faint };
    let edge = if theme.overlay == Edge::None {
        Edge::Plain
    } else {
        theme.overlay
    };
    let block = theme.block(edge, border_color).title(Line::from(vec![
        Span::raw(" "),
        Span::styled("Note", Style::default().fg(theme.title2).bold()),
        Span::styled(" — ", theme.dim()),
        Span::styled(
            editor.label.clone(),
            Style::default().fg(theme.accent).bold(),
        ),
        Span::raw(" "),
    ]));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let visible_h = inner.height as usize;
    if visible_h == 0 {
        return;
    }

    // Compute scroll locally each frame from cursor + visible. (Editor
    // doesn't persist scroll itself — we don't have &mut App here.)
    let scroll = compute_editor_scroll(editor.scroll as usize, editor.cursor_line, visible_h);

    for (i, line) in editor.lines.iter().enumerate().skip(scroll).take(visible_h) {
        let row_idx = (i - scroll) as u16;
        let row_area = Rect::new(inner.x, inner.y + row_idx, inner.width, 1);
        let prefix = format!("{:>3} │ ", i + 1);
        let prefix_w = prefix.chars().count() as u16;
        f.render_widget(
            Line::from(vec![
                Span::styled(prefix, Style::default().fg(theme.faint)),
                Span::styled(line.clone(), Style::default().fg(theme.fg)),
            ]),
            row_area,
        );
        // Place the terminal cursor only when the editor pane is focused.
        if focused && i == editor.cursor_line {
            let cur_x = inner.x + prefix_w + editor.cursor_col as u16;
            let cur_y = inner.y + row_idx;
            let cur_x = cur_x.min(inner.x + inner.width.saturating_sub(1));
            f.set_cursor_position((cur_x, cur_y));
        }
    }
}

fn draw_plan_view(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let total = app.plan.days.len();
    let done = app.plan_completed.len();
    let pct = (done * 100).checked_div(total).unwrap_or(0);

    let block = screen_block(
        theme,
        Line::from(vec![
            Span::raw(" "),
            Span::styled(
                format!("{} — {}", theme.heading("Bible in a Year"), app.plan.year),
                Style::default().fg(theme.title).bold(),
            ),
            Span::styled("  ·  ", theme.dim()),
            Span::styled(
                format!("{}/{} days", done, total),
                Style::default().fg(theme.title2).bold(),
            ),
            Span::styled(format!(" ({}%) ", pct), theme.dim()),
        ]),
    );
    let inner = block.inner(area);
    f.render_widget(block, area);

    let layout = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(inner);

    let visible = layout[0].height as usize;
    if visible == 0 || total == 0 {
        return;
    }
    let scroll = compute_list_scroll(app.plan_cursor, total, visible);
    let today = crate::plan::today_day_of_year();

    for (i, daily) in app.plan.days.iter().enumerate().skip(scroll).take(visible) {
        let row_idx = (i - scroll) as u16;
        let row_area = Rect::new(layout[0].x, layout[0].y + row_idx, layout[0].width, 1);
        let is_cursor = i == app.plan_cursor;
        let is_today = daily.day == today;
        let is_done = app.plan_completed.contains(&daily.day);

        let row_style = if is_cursor {
            Style::default().bg(theme.sel_bg)
        } else {
            Style::default()
        };
        let (mark, mark_style) = if is_done {
            ("✓", Style::default().fg(theme.ok).bold())
        } else {
            ("·", theme.dim())
        };
        let day_label_style = if is_today {
            Style::default().fg(theme.title2).bold()
        } else {
            Style::default().fg(theme.accent)
        };
        let date_str = daily.date.format("%b %-d").to_string();

        let mut spans = vec![
            Span::raw(" "),
            Span::styled(mark.to_string(), mark_style),
            Span::raw("  "),
            Span::styled(format!("Day {:>3}", daily.day), day_label_style),
            Span::raw("  "),
            Span::styled(format!("{:>6}", date_str), theme.dim()),
            Span::raw("  "),
            Span::styled(daily.short(), Style::default().fg(theme.fg)),
        ];
        if is_today {
            spans.push(Span::raw("   "));
            spans.push(Span::styled("← today", Style::default().fg(theme.title2)));
        }
        f.render_widget(Line::from(spans).style(row_style), row_area);
    }

    let hint = status_or_hints(
        app,
        theme,
        &[
            ("Enter", "jump"),
            ("m", "mark/unmark"),
            ("t", "today"),
            ("↑↓", "move"),
            ("PgUp/Dn", "page"),
            ("Esc", "back"),
        ],
    );
    f.render_widget(hint, layout[1]);
}

/// Scroll for fixed-height list views (plan, pickers): keeps the cursor
/// centred where possible, clamped to the ends.
fn compute_list_scroll(cursor: usize, total: usize, visible: usize) -> usize {
    if visible == 0 || total == 0 {
        return 0;
    }
    let max_scroll = total.saturating_sub(visible);
    cursor.saturating_sub(visible / 2).min(max_scroll)
}

/// Editor scroll = previous scroll, adjusted to keep the cursor line in
/// view. Mirror of the chapter-pane scroll, but per-line not per-verse.
fn compute_editor_scroll(prev: usize, cursor_line: usize, visible: usize) -> usize {
    if visible == 0 {
        return prev;
    }
    if cursor_line < prev {
        cursor_line
    } else if cursor_line >= prev + visible {
        cursor_line + 1 - visible
    } else {
        prev
    }
}

fn draw_book_picker(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let w = 40u16.min(area.width.saturating_sub(4));
    let h = (area.height * 7 / 10)
        .max(8)
        .min(area.height.saturating_sub(2));
    let rect = chrome::centered(area, w, h);
    let title = Line::from(Span::styled(
        format!(" {} ", theme.heading("Books")),
        Style::default().fg(theme.title).bold(),
    ));
    let inner = chrome::card(f, rect, Some(title), theme);

    let layout = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(inner);
    let list_area = layout[0];
    let visible = list_area.height as usize;
    if visible == 0 {
        return;
    }

    let total = 66usize;
    let cur_book_num = app.current.as_ref().map(|cr| cr.book().number());
    let scroll = compute_list_scroll(app.book_picker_cursor, total, visible);

    for i in scroll..(scroll + visible).min(total) {
        let row_idx = (i - scroll) as u16;
        let row_area = Rect::new(list_area.x, list_area.y + row_idx, list_area.width, 1);
        let book_num = (i + 1) as u8;
        let Ok(book) = crate::reference::book_from_number(book_num) else {
            continue;
        };
        let is_cursor = i == app.book_picker_cursor;
        let is_current = cur_book_num == Some(book_num);
        let row_style = if is_cursor {
            Style::default().bg(theme.sel_bg)
        } else {
            Style::default()
        };
        let mark = if is_current { "•" } else { " " };
        let name_style = if is_current {
            Style::default().fg(theme.title2).bold()
        } else {
            Style::default().fg(theme.fg)
        };
        let spans = vec![
            Span::styled(
                if is_cursor { "▌" } else { " " },
                Style::default().fg(theme.accent),
            ),
            Span::styled(mark.to_string(), Style::default().fg(theme.title2)),
            Span::raw(" "),
            Span::styled(format!("{:>2}", book_num), theme.dim()),
            Span::raw("  "),
            Span::styled(book_display(&book), name_style),
        ];
        f.render_widget(Line::from(spans).style(row_style), row_area);
    }

    f.render_widget(
        hint_line(theme, &[("Enter", "jump"), ("↑↓", "move"), ("Esc", "back")]),
        layout[1],
    );
}

fn draw_pick_secondary(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let rect = chrome::centered(
        area,
        72.min(area.width.saturating_sub(4)),
        (area.height * 6 / 10).max(6),
    );
    let title = Line::from(Span::styled(
        format!(" {} ", theme.heading("Parallel translation")),
        Style::default().fg(theme.accent2).bold(),
    ));
    let inner = chrome::card(f, rect, Some(title), theme);

    let primary_id = app
        .bible
        .as_ref()
        .map(|b| b.translation.id.as_str())
        .unwrap_or("");
    let candidates: Vec<&TranslationInfo> = app
        .installed
        .iter()
        .filter(|t| t.id != primary_id)
        .collect();

    if candidates.is_empty() {
        f.render_widget(
            Span::styled(
                "  Install another translation first (T → install).",
                theme.dim(),
            ),
            inner,
        );
        return;
    }

    let items: Vec<ListItem> = candidates
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let sel = i == app.secondary_picker_cursor;
            let style = if sel {
                Style::default().bg(theme.sel_bg)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(vec![
                Span::styled(
                    if sel { "▌" } else { " " },
                    Style::default().fg(theme.accent),
                ),
                Span::styled(
                    format!(" {:8}", chrome::short_name(t)),
                    Style::default().fg(theme.accent2).bold(),
                ),
                Span::raw(" "),
                Span::styled(t.display_name.clone(), Style::default().fg(theme.fg)),
                Span::raw("  "),
                Span::styled(format!("({})", t.language), theme.dim()),
            ]))
            .style(style)
        })
        .collect();
    f.render_widget(List::new(items), inner);
}

fn draw_download_popup(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let Some(d) = app.download.as_ref() else {
        return;
    };
    let rect = chrome::centered(area, 60.min(area.width.saturating_sub(4)), 6);
    let title = Line::from(Span::styled(
        format!(" {} ", theme.heading("Downloading")),
        Style::default().fg(theme.title).bold(),
    ));
    let inner = chrome::card(f, rect, Some(title), theme);

    let rows = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .split(inner);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw(" "),
            Span::styled(d.id.clone(), Style::default().fg(theme.accent).bold()),
        ])),
        rows[0],
    );

    let pct = match d.total {
        Some(total) if total > 0 => ((d.bytes * 100) / total).min(100) as u16,
        _ => 0,
    };
    let label = match d.total {
        Some(total) => format!("{:>3}%  {} / {} KiB", pct, d.bytes / 1024, total / 1024),
        None => format!("{} KiB", d.bytes / 1024),
    };
    let g = Gauge::default()
        .gauge_style(Style::default().fg(theme.ok).bg(theme.sel_bg))
        .percent(pct)
        .label(label);
    f.render_widget(
        g,
        Rect::new(rows[1].x + 1, rows[1].y, rows[1].width.saturating_sub(2), 1),
    );
}

// ─────────────────────────────────────────────────────────────────────────
// Settings
// ─────────────────────────────────────────────────────────────────────────

fn draw_settings(f: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let rows = Layout::vertical([
        Constraint::Length(chrome::header_height(theme)),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(area);

    // 60/40 split: live-preview pane on the left, settings list on the right.
    let panes =
        Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)]).split(rows[1]);
    chrome::draw_header(f, app, rows[0], rows[0], theme);

    if app.bible.is_some() {
        draw_chapter(f, app, panes[0], theme, false);
    } else {
        chrome::draw_welcome(f, panes[0], theme);
    }
    draw_settings_panel(f, app, panes[1], theme);

    f.render_widget(
        hint_line(
            theme,
            &[("↑↓", "move"), ("←→", "change"), ("Esc", "save & close")],
        ),
        rows[2],
    );
}

fn draw_settings_panel(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let block = screen_block(
        theme,
        title_line(theme, "Settings", theme.title, String::new()),
    );
    let inner = block.inner(area);
    f.render_widget(block, area);

    let layout = settings_layout();
    // Map cursor index (in items only) to the row in the layout list.
    let cursor_layout_row = layout
        .iter()
        .enumerate()
        .filter(|(_, r)| matches!(r, SettingsRow::Item(_)))
        .nth(app.settings_cursor)
        .map_or(0, |(i, _)| i);

    let lines: Vec<Line> = layout
        .iter()
        .enumerate()
        .map(|(i, row)| match row {
            SettingsRow::Header(h) => Line::from(vec![
                Span::raw(" "),
                Span::styled(theme.heading(h), Style::default().fg(theme.title2).bold()),
            ]),
            SettingsRow::Item(it) => {
                let selected = i == cursor_layout_row;
                let row_style = if selected {
                    Style::default().bg(theme.sel_bg)
                } else {
                    Style::default()
                };
                let label_style = if selected {
                    Style::default().fg(theme.fg).bold()
                } else {
                    Style::default().fg(theme.fg)
                };
                let value_style = if selected {
                    Style::default().fg(theme.title2).bold()
                } else {
                    Style::default().fg(theme.accent)
                };
                let arrows = if selected { "‹  ›" } else { "    " };
                Line::from(vec![
                    Span::styled(
                        if selected { " ▌" } else { "  " },
                        Style::default().fg(theme.accent).patch(row_style),
                    ),
                    Span::styled(format!("{:<20}", it.label()), label_style.patch(row_style)),
                    Span::styled(it.value(&app.settings), value_style.patch(row_style)),
                    Span::raw("  "),
                    Span::styled(arrows.to_string(), theme.dim().patch(row_style)),
                ])
                .style(row_style)
            }
        })
        .collect();
    // Keep the cursor row (and the one under it) in view.
    let visible = inner.height as usize;
    let scroll = (cursor_layout_row + 2).saturating_sub(visible);
    f.render_widget(Paragraph::new(lines).scroll((scroll as u16, 0)), inner);
}

#[derive(Debug, Clone, Copy)]
enum SettingsRow {
    Header(&'static str),
    Item(SettingItem),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SettingItem {
    ThemePreset,
    Layout,
    Columns,
    DropCap,
    MaxColumns,
    DefaultTranslation,
    Mouse,
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
    ParallelDivider,
}

const fn settings_layout() -> &'static [SettingsRow] {
    &[
        SettingsRow::Header("Look"),
        SettingsRow::Item(SettingItem::ThemePreset),
        SettingsRow::Header("Reading"),
        SettingsRow::Item(SettingItem::Layout),
        SettingsRow::Item(SettingItem::Columns),
        SettingsRow::Item(SettingItem::DropCap),
        SettingsRow::Item(SettingItem::MaxColumns),
        SettingsRow::Item(SettingItem::DefaultTranslation),
        SettingsRow::Item(SettingItem::Mouse),
        SettingsRow::Header("Typography"),
        SettingsRow::Item(SettingItem::JustifyText),
        SettingsRow::Item(SettingItem::WordPadding),
        SettingsRow::Item(SettingItem::VerseSpacing),
        SettingsRow::Item(SettingItem::LineSpacing),
        SettingsRow::Item(SettingItem::VerseNumberStyle),
        SettingsRow::Header("Letter padding"),
        SettingsRow::Item(SettingItem::PaddingDefault),
        SettingsRow::Item(SettingItem::PaddingTamil),
        SettingsRow::Item(SettingItem::PaddingDevanagari),
        SettingsRow::Item(SettingItem::PaddingArabic),
        SettingsRow::Item(SettingItem::PaddingHebrew),
        SettingsRow::Item(SettingItem::PaddingCjk),
        SettingsRow::Header("Parallel"),
        SettingsRow::Item(SettingItem::ParallelDivider),
    ]
}

pub(super) const SETTINGS_ITEMS: &[SettingItem] = &[
    SettingItem::ThemePreset,
    SettingItem::Layout,
    SettingItem::Columns,
    SettingItem::DropCap,
    SettingItem::MaxColumns,
    SettingItem::DefaultTranslation,
    SettingItem::Mouse,
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
    SettingItem::ParallelDivider,
];

fn on_off(b: bool) -> String {
    if b {
        "on".to_string()
    } else {
        "off".to_string()
    }
}

impl SettingItem {
    fn label(&self) -> &'static str {
        match self {
            SettingItem::ThemePreset => "Theme",
            SettingItem::Layout => "Layout",
            SettingItem::Columns => "Columns",
            SettingItem::DropCap => "Drop cap",
            SettingItem::MaxColumns => "Reading width",
            SettingItem::DefaultTranslation => "Default translation",
            SettingItem::Mouse => "Mouse",
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
            SettingItem::ParallelDivider => "Divider",
        }
    }

    fn value(&self, s: &Settings) -> String {
        match self {
            SettingItem::ThemePreset => s.theme.preset.label().to_string(),
            SettingItem::Layout => match s.reader.layout {
                ReadingLayout::Auto => {
                    let prose = theme::resolve(s.theme.preset).prefers_prose;
                    format!("auto ({})", if prose { "prose" } else { "verses" })
                }
                ReadingLayout::Verses => "verse per line".to_string(),
                ReadingLayout::Prose => "prose".to_string(),
            },
            SettingItem::Columns => match s.reader.columns {
                ColumnLayout::Auto => "auto".to_string(),
                ColumnLayout::One => "one".to_string(),
                ColumnLayout::Two => "two".to_string(),
            },
            SettingItem::DropCap => on_off(s.reader.drop_cap),
            SettingItem::MaxColumns => match s.reader.max_columns {
                0 => "auto".to_string(),
                FULL_WIDTH => "full width".to_string(),
                n => format!("{n} cells"),
            },
            SettingItem::DefaultTranslation => {
                if s.reader.default_translation.is_empty() {
                    "(first installed)".to_string()
                } else {
                    s.reader.default_translation.clone()
                }
            }
            SettingItem::Mouse => on_off(s.reader.mouse),
            SettingItem::JustifyText => on_off(s.typography.justify),
            SettingItem::WordPadding => format!("+{}", s.typography.word_padding),
            SettingItem::VerseSpacing => format!("{} line(s)", s.typography.verse_spacing),
            SettingItem::LineSpacing => format!("{} line(s)", s.typography.line_spacing),
            SettingItem::VerseNumberStyle => match s.typography.verse_number_style {
                VerseNumberStyle::InlineBold => "inline".to_string(),
                VerseNumberStyle::Superscript => "superscript".to_string(),
                VerseNumberStyle::Hidden => "hidden".to_string(),
            },
            SettingItem::PaddingDefault => {
                format!("+{}", s.typography.script_letter_padding.default)
            }
            SettingItem::PaddingTamil => format!("+{}", s.typography.script_letter_padding.tamil),
            SettingItem::PaddingDevanagari => {
                format!("+{}", s.typography.script_letter_padding.devanagari)
            }
            SettingItem::PaddingArabic => format!("+{}", s.typography.script_letter_padding.arabic),
            SettingItem::PaddingHebrew => format!("+{}", s.typography.script_letter_padding.hebrew),
            SettingItem::PaddingCjk => format!("+{}", s.typography.script_letter_padding.cjk),
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
        if matches!(self, SettingItem::DefaultTranslation) {
            // Reads app.installed alongside the settings write.
            let cur = app.settings.reader.default_translation.clone();
            app.settings.reader.default_translation =
                cycle_default_translation(&cur, &app.installed, dir);
            return;
        }
        let s = &mut app.settings;
        match self {
            SettingItem::ThemePreset => {
                s.theme.preset = cycle_in(&ThemePreset::ALL, s.theme.preset, dir)
            }
            SettingItem::Layout => {
                let order = [
                    ReadingLayout::Auto,
                    ReadingLayout::Verses,
                    ReadingLayout::Prose,
                ];
                s.reader.layout = cycle_in(&order, s.reader.layout, dir);
            }
            SettingItem::Columns => {
                let order = [ColumnLayout::Auto, ColumnLayout::One, ColumnLayout::Two];
                s.reader.columns = cycle_in(&order, s.reader.columns, dir);
            }
            SettingItem::DropCap => s.reader.drop_cap = !s.reader.drop_cap,
            SettingItem::MaxColumns => {
                s.reader.max_columns = step_max_columns(s.reader.max_columns, dir)
            }
            SettingItem::Mouse => s.reader.mouse = !s.reader.mouse,
            // Bool toggle — direction doesn't matter, h/l/Enter all flip.
            SettingItem::JustifyText => s.typography.justify = !s.typography.justify,
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
                let order = [
                    VerseNumberStyle::InlineBold,
                    VerseNumberStyle::Superscript,
                    VerseNumberStyle::Hidden,
                ];
                s.typography.verse_number_style =
                    cycle_in(&order, s.typography.verse_number_style, dir);
            }
            SettingItem::PaddingDefault => {
                let p = &mut s.typography.script_letter_padding.default;
                *p = clamp_u8(*p, dir, 0, 3);
            }
            SettingItem::PaddingTamil => {
                let p = &mut s.typography.script_letter_padding.tamil;
                *p = clamp_u8(*p, dir, 0, 3);
            }
            SettingItem::PaddingDevanagari => {
                let p = &mut s.typography.script_letter_padding.devanagari;
                *p = clamp_u8(*p, dir, 0, 3);
            }
            SettingItem::PaddingArabic => {
                let p = &mut s.typography.script_letter_padding.arabic;
                *p = clamp_u8(*p, dir, 0, 3);
            }
            SettingItem::PaddingHebrew => {
                let p = &mut s.typography.script_letter_padding.hebrew;
                *p = clamp_u8(*p, dir, 0, 3);
            }
            SettingItem::PaddingCjk => {
                let p = &mut s.typography.script_letter_padding.cjk;
                *p = clamp_u8(*p, dir, 0, 3);
            }
            SettingItem::ParallelDivider => {
                let order = [
                    DividerStyle::Single,
                    DividerStyle::Double,
                    DividerStyle::None,
                ];
                s.parallel.divider = cycle_in(&order, s.parallel.divider, dir);
            }
            SettingItem::DefaultTranslation => {}
        }
    }
}

fn clamp_u8(v: u8, dir: i32, min: u8, max: u8) -> u8 {
    let next = v as i32 + dir;
    next.clamp(min as i32, max as i32) as u8
}

/// auto → 60, 65 … 200 → full width, clamped at both ends.
fn step_max_columns(v: u16, dir: i32) -> u16 {
    let mut steps: Vec<u16> = vec![0];
    steps.extend((60..=200).step_by(5));
    steps.push(FULL_WIDTH);
    let pos = steps.iter().position(|s| *s == v).unwrap_or_else(|| {
        steps
            .iter()
            .position(|s| *s >= v && *s != 0)
            .unwrap_or(steps.len() - 1)
    });
    let next = (pos as i32 + dir).clamp(0, steps.len() as i32 - 1);
    steps[next as usize]
}

fn cycle_in<T: Copy + PartialEq>(order: &[T], v: T, dir: i32) -> T {
    let n = order.len() as i32;
    let pos = order.iter().position(|x| *x == v).unwrap_or(0) as i32;
    let next = (((pos + dir) % n) + n) % n;
    order[next as usize]
}

fn cycle_default_translation(cur: &str, installed: &[TranslationInfo], dir: i32) -> String {
    if installed.is_empty() {
        return String::new();
    }
    // States: "" (first installed) → installed[0].id → ... → installed[n-1].id → "" → ...
    let mut states: Vec<String> = Vec::with_capacity(installed.len() + 1);
    states.push(String::new());
    for t in installed {
        states.push(t.id.clone());
    }
    let pos = states.iter().position(|s| s == cur).unwrap_or(0) as i32;
    let n = states.len() as i32;
    let next = (((pos + dir) % n) + n) % n;
    states[next as usize].clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_steps_run_from_auto_to_full() {
        assert_eq!(step_max_columns(0, 1), 60);
        assert_eq!(step_max_columns(0, -1), 0);
        assert_eq!(step_max_columns(200, 1), FULL_WIDTH);
        assert_eq!(step_max_columns(FULL_WIDTH, 1), FULL_WIDTH);
        assert_eq!(step_max_columns(FULL_WIDTH, -1), 200);
        // A hand-edited odd value lands on the next step.
        assert_eq!(step_max_columns(83, 1), 90);
    }

    #[test]
    fn settings_items_match_layout_order() {
        let from_layout: Vec<SettingItem> = settings_layout()
            .iter()
            .filter_map(|r| match r {
                SettingsRow::Item(it) => Some(*it),
                SettingsRow::Header(_) => None,
            })
            .collect();
        assert_eq!(from_layout, SETTINGS_ITEMS);
    }
}
