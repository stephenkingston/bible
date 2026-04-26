use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Gauge, List, ListItem, Paragraph, Wrap};

use crate::reference::book_display;
use crate::storage;

use super::{App, Mode};

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();

    match app.mode {
        Mode::Manager => draw_manager(f, app, area),
        _ => draw_reader(f, app, area),
    }

    if app.mode == Mode::Help {
        draw_help_overlay(f, area);
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
    } else {
        draw_chapter(f, app, rows[1]);
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
    f.render_widget(Paragraph::new(line), area);
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

fn draw_chapter(f: &mut Frame, app: &App, area: Rect) {
    let bible = app.bible.as_ref().unwrap();
    let Some(cr) = app.current.as_ref() else {
        return;
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Indexed(24)))
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                book_display(&cr.book()),
                Style::default().fg(Color::Cyan).bold(),
            ),
            Span::raw(" "),
            Span::styled(
                cr.chapter().to_string(),
                Style::default().fg(Color::Yellow).bold(),
            ),
            Span::raw(" "),
        ]));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let chapter = match bible.get_chapter(cr) {
        Some(c) => c,
        None => {
            let msg = Paragraph::new("(chapter not present in this translation)")
                .style(Style::default().add_modifier(Modifier::DIM));
            f.render_widget(msg, inner);
            return;
        }
    };

    let mut lines: Vec<Line> = Vec::with_capacity(chapter.verses.len() + 2);
    lines.push(Line::from(""));
    let highlight_verse = highlighted_verse_for(app, cr);
    for verse in &chapter.verses {
        let highlighted = Some(verse.number) == highlight_verse;
        let num_style = if highlighted {
            Style::default().fg(Color::Black).bg(Color::Yellow).bold()
        } else {
            Style::default().fg(Color::Indexed(244))
        };
        let text_style = if highlighted {
            Style::default().bold()
        } else {
            Style::default()
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{:>3} ", verse.number), num_style),
            Span::styled(verse.text.clone(), text_style),
        ]));
    }

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((app.scroll, 0));
    f.render_widget(p, inner);
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
    // Paragraph (no .wrap()) doesn't soft-wrap, but does honor explicit
    // newlines as line breaks. We strip them above so the bar is always
    // exactly one row regardless of error message contents.
    f.render_widget(Paragraph::new(line), area);
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
        Line::from(Span::styled("Translations", Style::default().bold().fg(Color::Yellow))),
        Line::from("  T              translation manager"),
        Line::from("  t              cycle installed translations"),
        Line::from(""),
        Line::from(Span::styled("Misc", Style::default().bold().fg(Color::Yellow))),
        Line::from("  ?              this help (Esc to close)"),
        Line::from("  q              quit"),
    ];
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
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
