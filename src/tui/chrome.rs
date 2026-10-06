//! Reader chrome: the header, the status line, the keys card and the
//! command palette, plus the shared hint helpers.

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};
use unicode_width::UnicodeWidthStr;

use crate::bible::TranslationInfo;
use crate::reference::book_display;

use super::theme::{Edge, Header, Theme};
use super::{App, Mode, SPINNER_FRAMES, palette};

pub(super) fn header_height(theme: &Theme) -> u16 {
    if theme.header == Header::RunningHead {
        2
    } else {
        1
    }
}

/// Short label for a translation: "KJV", "NIV", "Darby".
pub(super) fn short_name(t: &TranslationInfo) -> String {
    let rest = t
        .display_name
        .strip_prefix(t.language.as_str())
        .map(str::trim_start)
        .unwrap_or(&t.display_name);
    if let Some(word) = rest.split_whitespace().next() {
        let caps =
            word.chars().any(|c| c.is_uppercase()) && !word.chars().any(|c| c.is_lowercase());
        if caps && word.chars().count() <= 6 {
            return word.to_string();
        }
    }
    let id = t.id.strip_suffix("Bible").unwrap_or(&t.id);
    let id = id.strip_prefix(t.language.as_str()).unwrap_or(id);
    if !id.is_empty() && id.chars().count() <= 12 {
        return id.to_string();
    }
    t.language.clone()
}

fn today(app: &App) -> Option<(String, bool)> {
    let idx = crate::plan::today_day_of_year().saturating_sub(1) as usize;
    app.plan
        .days
        .get(idx)
        .map(|d| (d.short(), app.plan_completed.contains(&d.day)))
}

fn position(app: &App) -> Option<(String, u32)> {
    app.current
        .as_ref()
        .map(|cr| (book_display(&cr.book()).to_string(), cr.chapter().into()))
}

pub(super) fn width_of(spans: &[Span]) -> u16 {
    spans.iter().map(|s| s.width() as u16).sum()
}

/// Left and right content on one row, right side kept when they collide.
fn left_right(f: &mut Frame, area: Rect, left: Vec<Span<'static>>, right: Vec<Span<'static>>) {
    let rw = width_of(&right).min(area.width);
    let lw = area.width.saturating_sub(rw);
    f.render_widget(Line::from(left), Rect::new(area.x, area.y, lw, 1));
    f.render_widget(
        Line::from(right),
        Rect::new(area.x + area.width - rw, area.y, rw, 1),
    );
}

pub(super) fn draw_header(f: &mut Frame, app: &App, area: Rect, pane: Rect, theme: &Theme) {
    let translation = app.bible.as_ref().map(|b| &b.translation);
    let pos = position(app);
    let today_spans = |label_dim: bool| -> Vec<Span<'static>> {
        let Some((short, done)) = today(app) else {
            return Vec::new();
        };
        let mark = if done { "●" } else { "○" };
        let mark_color = if done { theme.ok } else { theme.accent };
        let label = Style::default().fg(if label_dim { theme.dim } else { theme.fg });
        vec![
            Span::styled("today  ", theme.dim()),
            Span::styled(short, label),
            Span::styled(format!("  {mark} "), Style::default().fg(mark_color)),
        ]
    };
    // Everything but the classic badge lines up with the text column.
    let row = if theme.header == Header::Badge {
        Rect::new(area.x, area.y, area.width, 1)
    } else {
        Rect::new(pane.x + 1, area.y, pane.width.saturating_sub(2), 1)
    };
    match theme.header {
        Header::Badge => {
            let mut left = vec![
                Span::styled(
                    " bible ",
                    Style::default()
                        .bg(theme.badge_bg)
                        .fg(theme.badge_fg)
                        .bold(),
                ),
                Span::raw(" "),
                Span::styled(
                    translation.map_or("—".to_string(), |t| t.display_name.clone()),
                    Style::default().fg(theme.title).bold(),
                ),
                Span::raw(" │ "),
            ];
            left.push(Span::styled(
                pos.map_or("—".to_string(), |(b, c)| format!("{b} {c}")),
                Style::default().fg(theme.title2),
            ));
            let right = match today(app) {
                Some((short, done)) => vec![
                    Span::raw("📖 "),
                    Span::styled("Today: ", theme.dim()),
                    Span::styled(
                        short,
                        Style::default()
                            .fg(if done { theme.ok } else { theme.title2 })
                            .bold(),
                    ),
                    Span::raw(" "),
                ],
                None => Vec::new(),
            };
            left_right(f, row, left, right);
        }
        Header::Title => {
            let mut left = vec![Span::raw(" ")];
            if let Some((book, ch)) = pos {
                left.push(Span::styled(book, Style::default().fg(theme.title).bold()));
                left.push(Span::raw(" "));
                left.push(Span::styled(
                    ch.to_string(),
                    Style::default().fg(theme.title2).bold(),
                ));
                left.push(Span::raw("   "));
            }
            if let Some(t) = translation {
                left.push(Span::styled(t.display_name.clone(), theme.dim()));
            }
            left_right(f, row, left, today_spans(false));
        }
        Header::RunningHead => {
            let left = vec![
                Span::raw("  "),
                Span::styled(
                    translation.map_or(String::new(), |t| {
                        truncate(&t.display_name.to_uppercase(), 30)
                    }),
                    theme.dim(),
                ),
            ];
            left_right(f, row, left, today_spans(true));
            if let Some((book, ch)) = pos {
                let head = format!(" {}   {} ", theme.heading(&book), ch);
                let w = head.width() as u16;
                if w + 4 < row.width {
                    let x = row.x + (row.width - w) / 2;
                    f.render_widget(
                        Span::styled(head, Style::default().fg(theme.title).bold()),
                        Rect::new(x, area.y, w, 1),
                    );
                }
            }
            if area.height > 1 {
                let rule_y = area.y + 1;
                let rule = Rect::new(pane.x + 2, rule_y, pane.width.saturating_sub(4), 1);
                f.render_widget(
                    Span::styled(
                        "─".repeat(rule.width as usize),
                        Style::default().fg(theme.faint),
                    ),
                    rule,
                );
                if let Some(orn) = theme.ornament {
                    let s = format!(" {orn} ");
                    let w = s.width() as u16;
                    let x = rule.x + rule.width.saturating_sub(w) / 2;
                    f.render_widget(
                        Span::styled(s, Style::default().fg(theme.accent)),
                        Rect::new(x, rule_y, w, 1),
                    );
                }
            }
        }
        Header::Chips => {
            let mut left = vec![
                Span::raw(" "),
                Span::styled(
                    theme.ornament.unwrap_or("•"),
                    Style::default().fg(theme.accent).bold(),
                ),
                Span::raw(" "),
            ];
            let current = translation.map(|t| t.id.as_str());
            let mut used = width_of(&left);
            for t in &app.installed {
                let chip = format!(" {} ", short_name(t));
                let w = chip.width() as u16 + 1;
                if used + w > row.width / 2 {
                    break;
                }
                used += w;
                let style = if Some(t.id.as_str()) == current {
                    Style::default()
                        .fg(theme.badge_fg)
                        .bg(theme.badge_bg)
                        .bold()
                } else {
                    theme.dim()
                };
                left.push(Span::styled(chip, style));
                left.push(Span::raw(" "));
            }
            left_right(f, row, left, today_spans(true));
        }
    }
}

fn mode_label(mode: Mode) -> &'static str {
    match mode {
        Mode::Jump => "GO",
        Mode::Search => "FIND",
        Mode::Help => "KEYS",
        Mode::Palette => "PALETTE",
        Mode::PickBook => "BOOK",
        Mode::PickSecondary => "PARALLEL",
        Mode::NoTranslation => "START",
        _ => "READ",
    }
}

/// The status line: what you're doing and where you are on the left; the
/// way out (`? keys`, `q quit`) always pinned on the right.
pub(super) fn draw_footer(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    f.render_widget(
        Block::default().style(Style::default().bg(theme.bar_bg)),
        area,
    );
    let bar = |s: Style| s.bg(theme.bar_bg);

    if let Some(s) = app.searching.as_ref() {
        let frame = SPINNER_FRAMES[s.spinner % SPINNER_FRAMES.len()];
        let elapsed = s.started_at.elapsed().as_millis();
        let line = Line::from(vec![
            Span::raw(" "),
            Span::styled(
                frame.to_string(),
                bar(Style::default().fg(theme.accent).bold()),
            ),
            Span::styled(" searching for ", bar(Style::default().fg(theme.fg))),
            Span::styled(
                format!("`{}`", sanitize_one_line(&s.query)),
                bar(Style::default().fg(theme.accent2).bold()),
            ),
            Span::styled(
                format!("  ({}.{:03}s)", elapsed / 1000, elapsed % 1000),
                bar(theme.dim()),
            ),
        ]);
        f.render_widget(line, area);
        return;
    }

    let mut left: Vec<Span<'static>> = Vec::new();
    if theme.pill {
        let pill_bg = if app.mode == Mode::Help {
            theme.accent2
        } else {
            theme.badge_bg
        };
        left.push(Span::styled(
            format!(" {} ", mode_label(app.mode)),
            Style::default().fg(theme.badge_fg).bg(pill_bg).bold(),
        ));
    }
    left.push(Span::styled(" ", bar(Style::default())));

    // Pinned hints always show; optional ones only when there's room.
    type Hints<'a> = Vec<(&'a str, &'a str)>;
    let (pinned, optional): (Hints, Hints) = match app.mode {
        Mode::Jump => (
            vec![("Enter", "go"), ("Esc", "cancel")],
            vec![("↑↓", "history")],
        ),
        Mode::Search => (
            vec![("Enter", "search"), ("Esc", "cancel")],
            vec![("↑↓", "history")],
        ),
        Mode::Palette => (
            vec![("Enter", "run"), ("Esc", "close")],
            vec![("↑↓", "choose")],
        ),
        Mode::NoTranslation => (
            vec![
                ("i", "install KJV"),
                ("T", "browse"),
                ("?", "keys"),
                ("q", "quit"),
            ],
            vec![("^K", "palette")],
        ),
        _ if app.parallel => (
            vec![("|", "single"), ("?", "keys"), ("q", "quit")],
            vec![("\\", "swap"), ("^K", "palette")],
        ),
        _ => (
            vec![("?", "keys"), ("q", "quit")],
            vec![(":", "go to"), ("/", "search"), ("^K", "palette")],
        ),
    };

    match app.mode {
        Mode::Jump | Mode::Search => {
            let prompt = if app.mode == Mode::Jump { ":" } else { "/" };
            left.push(Span::styled(
                prompt,
                bar(Style::default().fg(theme.accent).bold()),
            ));
            left.push(Span::styled(
                sanitize_one_line(app.input.value()),
                bar(Style::default().fg(theme.fg)),
            ));
            left.push(Span::styled("│", bar(Style::default().fg(theme.accent))));
        }
        _ if !app.status.is_empty() => {
            left.push(Span::styled(
                sanitize_one_line(&app.status),
                bar(Style::default().fg(theme.fg)),
            ));
        }
        _ => {
            if let (Some((book, ch)), Some(b)) = (position(app), app.bible.as_ref()) {
                left.push(Span::styled(
                    format!("{book} {ch}:{}", app.focus_verse),
                    bar(Style::default().fg(theme.fg).bold()),
                ));
                let mut label = format!("  {}", short_name(&b.translation));
                if app.parallel
                    && let Some(s) = app.secondary_bible.as_ref()
                {
                    label.push_str(&format!(" │ {}", short_name(&s.translation)));
                }
                left.push(Span::styled(label, bar(theme.dim())));
            }
        }
    }

    let right_for = |pairs: &[(&str, &str)]| -> Vec<Span<'static>> {
        let mut spans = hints(theme, pairs, Some(theme.bar_bg));
        spans.push(Span::styled(" ", bar(Style::default())));
        spans
    };
    let all: Vec<(&str, &str)> = optional.iter().chain(pinned.iter()).copied().collect();
    let full = right_for(&all);
    let right = if width_of(&left) + width_of(&full) + 2 <= area.width {
        full
    } else {
        right_for(&pinned)
    };
    left_right(f, area, left, right);
}

/// Key/label pairs as spans: keys in the theme's key style, labels dim.
pub(super) fn hints(
    theme: &Theme,
    pairs: &[(&str, &str)],
    bg: Option<Color>,
) -> Vec<Span<'static>> {
    let with_bg = |s: Style| match bg {
        Some(c) => s.bg(c),
        None => s,
    };
    let mut spans = Vec::with_capacity(pairs.len() * 4);
    for (i, (key, label)) in pairs.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("   ", with_bg(Style::default())));
        }
        let k = theme.key_span(key);
        let k_style = if theme.keycaps {
            k.style
        } else {
            with_bg(k.style)
        };
        spans.push(Span::styled(k.content.into_owned(), k_style));
        spans.push(Span::styled(" ", with_bg(Style::default())));
        spans.push(Span::styled(label.to_string(), with_bg(theme.dim())));
    }
    spans
}

/// A one-line hint footer for full-screen views.
pub(super) fn hint_line(theme: &Theme, pairs: &[(&str, &str)]) -> Line<'static> {
    let mut spans = vec![Span::raw(" ")];
    spans.extend(hints(theme, pairs, None));
    Line::from(spans)
}

pub(super) fn sanitize_one_line(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\n' | '\r' | '\t' => ' ',
            c if c.is_control() => '·',
            c => c,
        })
        .collect()
}

pub(super) fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// Recolour everything already drawn so an overlay stands out.
pub(super) fn fade(buf: &mut Buffer, area: Rect, theme: &Theme) {
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if let Some(c) = buf.cell_mut((x, y)) {
                c.fg = theme.faint;
                c.modifier = Modifier::empty();
                if c.bg == theme.focus_bg || c.bg == theme.hit_bg {
                    c.bg = theme.bg;
                }
            }
        }
    }
}

/// Clear `rect`, give it the panel background and the theme's overlay
/// border, with an optional drop shadow. Returns the area inside.
pub(super) fn card(f: &mut Frame, rect: Rect, title: Option<Line<'static>>, theme: &Theme) -> Rect {
    let screen = f.area();
    if let Some(shadow) = theme.shadow {
        let sx = (rect.x + 1).min(screen.right());
        let sy = (rect.y + 1).min(screen.bottom());
        let s = Rect::new(
            sx,
            sy,
            rect.width.min(screen.right() - sx),
            rect.height.min(screen.bottom() - sy),
        );
        for y in s.top()..s.bottom() {
            for x in s.left()..s.right() {
                if let Some(c) = f.buffer_mut().cell_mut((x, y)) {
                    c.bg = shadow;
                }
            }
        }
    }
    f.render_widget(Clear, rect);
    let edge = if theme.overlay == Edge::None {
        Edge::Plain
    } else {
        theme.overlay
    };
    let mut block = theme.block(edge, theme.border).style(theme.panel());
    if let Some(t) = title {
        block = block.title(t.centered());
    }
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    inner
}

pub(super) fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    )
}

fn card_title(theme: &Theme, text: &str) -> Line<'static> {
    let style = Style::default().fg(theme.title).bold();
    let label = theme.heading(text);
    match theme.ornament {
        Some(o) if theme.spaced => Line::from(Span::styled(format!(" {o}  {label}  {o} "), style)),
        _ => Line::from(Span::styled(format!(" {label} "), style)),
    }
}

const KEY_GROUPS: [(&str, &[(&str, &str)]); 4] = [
    (
        "Read",
        &[
            ("↑ ↓", "verse"),
            ("← →", "chapter"),
            ("⇧← ⇧→", "book"),
            ("g", "pick a book"),
            ("PgUp/Dn", "five verses"),
            ("Home/End", "first / last"),
        ],
    ),
    (
        "Find",
        &[
            (":", "go to reference"),
            ("/", "search"),
            ("n N", "next / prev hit"),
            ("^K", "palette"),
            ("^O Tab", "back / forward"),
        ],
    ),
    (
        "Library",
        &[
            ("t", "next translation"),
            ("|", "parallel view"),
            ("\\", "swap parallel"),
            ("T", "translations"),
            ("b", "bookmark"),
            ("B", "bookmarks"),
        ],
    ),
    (
        "More",
        &[
            ("y", "copy verse"),
            ("p", "today's reading"),
            ("P", "reading plan"),
            (",", "settings"),
            ("Esc", "clear search"),
        ],
    ),
];

const KEY_EXIT: [(&str, &str); 3] = [("q", "quit"), ("Esc", "close"), ("^K", "palette")];

/// The keys reference. Groups flow into as many columns as fit, so it fits
/// an 80×24 terminal; Nocturne docks it along the bottom.
pub(super) fn draw_keys(f: &mut Frame, area: Rect, theme: &Theme) {
    // Filled key caps stacked in a column run together, so the card
    // always uses plain coloured keys.
    let card_key = |k: &str| Span::styled(k.to_string(), Style::default().fg(theme.key).bold());
    let key_w = KEY_GROUPS
        .iter()
        .flat_map(|(_, items)| items.iter())
        .map(|(k, _)| k.width())
        .max()
        .unwrap_or(1) as u16
        + 2;
    let label_w = KEY_GROUPS
        .iter()
        .flat_map(|(_, items)| items.iter())
        .map(|(_, l)| l.width())
        .max()
        .unwrap_or(1) as u16;
    let col_w = key_w + label_w;
    let gutter = 4u16;
    let avail = area.width.saturating_sub(8);
    let ncols = (1..=4u16)
        .rev()
        .find(|n| n * col_w + (n - 1) * gutter <= avail)
        .unwrap_or(1) as usize;

    // Balance groups across columns, keeping their order.
    let heights: Vec<u16> = KEY_GROUPS
        .iter()
        .map(|(_, items)| items.len() as u16 + 1)
        .collect();
    let mut columns: Vec<Vec<usize>> = vec![Vec::new(); ncols];
    let mut col_h = vec![0u16; ncols];
    for (gi, h) in heights.iter().enumerate() {
        let target = if gi < ncols {
            gi
        } else {
            (0..ncols).min_by_key(|&c| col_h[c]).unwrap_or(0)
        };
        if !columns[target].is_empty() {
            col_h[target] += 1;
        }
        col_h[target] += h;
        columns[target].push(gi);
    }
    let content_h = col_h.iter().copied().max().unwrap_or(0);
    let content_w = ncols as u16 * col_w + (ncols as u16 - 1) * gutter;

    let inner = if theme.keys_panel {
        let h = (content_h + 4).min(area.height.saturating_sub(1));
        let rect = Rect::new(area.x, area.bottom().saturating_sub(1 + h), area.width, h);
        f.render_widget(Clear, rect);
        f.render_widget(Block::default().style(theme.panel()), rect);
        let rule = Line::from(vec![
            Span::styled("──", Style::default().fg(theme.faint)),
            Span::styled(" keys ", Style::default().fg(theme.accent).bold()),
            Span::styled(
                "─".repeat(rect.width.saturating_sub(20) as usize),
                Style::default().fg(theme.faint),
            ),
            Span::styled(" esc close ", theme.dim()),
        ]);
        f.render_widget(rule, Rect::new(rect.x, rect.y, rect.width, 1));
        Rect::new(
            rect.x + 4,
            rect.y + 2,
            rect.width.saturating_sub(8),
            rect.height.saturating_sub(2),
        )
    } else {
        let rect = centered(area, content_w + 8, content_h + 6);
        let inner = card(f, rect, Some(card_title(theme, "Keys")), theme);
        Rect::new(
            inner.x + 3,
            inner.y + 1,
            inner.width.saturating_sub(6),
            inner.height.saturating_sub(1),
        )
    };

    for (ci, groups) in columns.iter().enumerate() {
        let x = inner.x + ci as u16 * (col_w + gutter);
        let mut y = inner.y;
        for (n, &gi) in groups.iter().enumerate() {
            if n > 0 {
                y += 1;
            }
            let (title, items) = KEY_GROUPS[gi];
            let heading = if theme.spaced {
                theme.heading(title)
            } else {
                title.to_string()
            };
            put(
                f,
                x,
                y,
                inner,
                Span::styled(heading, Style::default().fg(theme.title2).bold()),
            );
            y += 1;
            for (k, l) in items.iter() {
                put(f, x, y, inner, card_key(k));
                put(
                    f,
                    x + key_w,
                    y,
                    inner,
                    Span::styled(l.to_string(), Style::default().fg(theme.fg)),
                );
                y += 1;
            }
        }
    }

    // The way out, on its own line under a rule.
    let exit_y = if theme.keys_panel {
        inner.y + content_h + 1
    } else {
        inner.bottom().saturating_sub(1)
    };
    if !theme.keys_panel && exit_y > inner.y {
        let rule_y = exit_y - 1;
        put(
            f,
            inner.x,
            rule_y,
            inner,
            Span::styled(
                "─".repeat(inner.width as usize),
                Style::default().fg(theme.faint),
            ),
        );
    }
    let mut exit: Vec<Span<'static>> = Vec::new();
    for (i, (k, l)) in KEY_EXIT.iter().enumerate() {
        if i > 0 {
            exit.push(Span::raw("     "));
        }
        exit.push(theme.key_span(k));
        exit.push(Span::raw(" "));
        let style = if *k == "q" {
            Style::default().fg(theme.fg).bold()
        } else {
            Style::default().fg(theme.fg)
        };
        exit.push(Span::styled(l.to_string(), style));
    }
    let w = width_of(&exit);
    let x = if theme.keys_panel {
        inner.x
    } else {
        inner.x + inner.width.saturating_sub(w) / 2
    };
    if exit_y < area.bottom() {
        f.render_widget(
            Line::from(exit),
            Rect::new(x, exit_y, w.min(inner.width), 1),
        );
    }
}

fn put(f: &mut Frame, x: u16, y: u16, clip: Rect, span: Span<'static>) {
    if y >= clip.bottom() || x >= clip.right() {
        return;
    }
    let w = (span.width() as u16).min(clip.right() - x);
    f.render_widget(span, Rect::new(x, y, w, 1));
}

pub(super) fn palette_items(app: &App) -> Vec<palette::Item> {
    let ctx = palette::Context {
        installed: &app.installed,
        current_translation: app.bible.as_ref().map(|b| b.translation.id.as_str()),
        theme: app.settings.theme.preset,
        layout: app.settings.reader.layout,
        columns: app.settings.reader.columns,
    };
    palette::items(app.palette_input.value(), &ctx)
}

pub(super) fn draw_palette(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let items = palette_items(app);
    let w = area.width.saturating_sub(4).min(68);
    let max_rows = (area.height as usize).saturating_sub(9).clamp(3, 12);
    let shown = items.len().clamp(1, max_rows) as u16;
    let h = shown + 4;
    let y = area.y + (area.height.saturating_sub(h)) / 5 + 1;
    let rect = Rect::new(
        area.x + (area.width - w) / 2,
        y.min(area.bottom().saturating_sub(h)),
        w,
        h,
    );
    let inner = card(f, rect, None, theme);
    if inner.height < 3 || inner.width < 10 {
        return;
    }

    // Input row.
    let prompt = Span::styled("› ", Style::default().fg(theme.accent).bold());
    let value = app.palette_input.value();
    let input_area = Rect::new(inner.x + 1, inner.y, inner.width.saturating_sub(2), 1);
    if value.is_empty() {
        f.render_widget(
            Line::from(vec![
                prompt,
                Span::styled("a reference, a search, or a command", theme.dim()),
            ]),
            input_area,
        );
        f.set_cursor_position((input_area.x + 2, input_area.y));
    } else {
        f.render_widget(
            Line::from(vec![
                prompt,
                Span::styled(value.to_string(), Style::default().fg(theme.fg)),
            ]),
            input_area,
        );
        let cur = input_area.x + 2 + app.palette_input.visual_cursor() as u16;
        f.set_cursor_position((cur.min(input_area.right().saturating_sub(1)), input_area.y));
    }
    f.render_widget(
        Span::styled(
            "─".repeat(inner.width as usize),
            Style::default().fg(theme.faint),
        ),
        Rect::new(inner.x, inner.y + 1, inner.width, 1),
    );

    let list = Rect::new(
        inner.x,
        inner.y + 2,
        inner.width,
        inner.height.saturating_sub(2),
    );
    if items.is_empty() {
        f.render_widget(Span::styled("  nothing matches", theme.dim()), list);
        return;
    }
    let cursor = app.palette_cursor.min(items.len() - 1);
    let visible = list.height as usize;
    let scroll = cursor.saturating_sub(visible.saturating_sub(1));
    for (i, it) in items.iter().enumerate().skip(scroll).take(visible) {
        let ry = list.y + (i - scroll) as u16;
        let row = Rect::new(list.x, ry, list.width, 1);
        let sel = i == cursor;
        if sel {
            f.render_widget(
                Block::default().style(Style::default().bg(theme.sel_bg)),
                row,
            );
            f.render_widget(
                Span::styled("▌", Style::default().fg(theme.accent)),
                Rect::new(row.x, ry, 1, 1),
            );
        }
        let hint_w = it.hint.width() as u16;
        let label_w = row.width.saturating_sub(hint_w + 6);
        let label_style = if sel {
            Style::default().fg(theme.fg).bold()
        } else {
            Style::default().fg(theme.fg)
        };
        f.render_widget(
            Span::styled(truncate(&it.label, label_w as usize), label_style),
            Rect::new(row.x + 2, ry, label_w, 1),
        );
        if hint_w > 0 {
            let hint_style = if it.hint == "current" || it.hint == "enter" {
                theme.dim()
            } else {
                Style::default().fg(theme.key).bold()
            };
            f.render_widget(
                Span::styled(it.hint.clone(), hint_style),
                Rect::new(row.right().saturating_sub(hint_w + 2), ry, hint_w, 1),
            );
        }
    }
}

/// First-run screen: what to press to get going (and to leave).
pub(super) fn draw_welcome(f: &mut Frame, area: Rect, theme: &Theme) {
    let key = |k: &str, label: &str| {
        let mut spans = vec![Span::raw("    "), theme.key_span(k)];
        let pad = 6usize.saturating_sub(theme.key_width(k));
        spans.push(Span::raw(" ".repeat(pad)));
        spans.push(Span::styled(
            label.to_string(),
            Style::default().fg(theme.fg),
        ));
        Line::from(spans)
    };
    let title = if theme.spaced {
        theme.heading("bible")
    } else {
        "bible".to_string()
    };
    let lines = vec![
        Line::from(Span::styled(title, Style::default().fg(theme.title).bold())).centered(),
        Line::from(""),
        Line::from(Span::styled(
            "No translations are installed yet.",
            theme.dim(),
        ))
        .centered(),
        Line::from(""),
        key("i", "install the King James Version"),
        key("T", "browse 1,000+ translations"),
        key("^K", "command palette"),
        key("?", "all keys"),
        key("q", "quit"),
        Line::from(""),
        Line::from(vec![
            Span::styled("or from a shell:  ", theme.dim()),
            Span::styled(
                "bible install kjv",
                Style::default().fg(theme.accent2).bold(),
            ),
        ])
        .centered(),
    ];
    let h = lines.len() as u16;
    let w = 44u16.min(area.width);
    let rect = Rect::new(
        area.x + area.width.saturating_sub(w) / 2,
        area.y + area.height.saturating_sub(h) / 2,
        w,
        h.min(area.height),
    );
    f.render_widget(Paragraph::new(lines), rect);
}
