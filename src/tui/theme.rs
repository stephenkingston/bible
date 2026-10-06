//! Themes: a palette plus a few chrome choices (frames, header style, focus
//! marker, key caps). Colours are written as 24-bit RGB and folded to the
//! nearest xterm-256 entry on terminals that don't advertise truecolor
//! (macOS Terminal.app, a bare tmux), so every theme still renders.

use std::sync::OnceLock;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, BorderType, Borders};

use crate::settings::ThemePreset;

/// Border treatment for the reading pane or an overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Edge {
    None,
    Plain,
    Rounded,
    Double,
}

/// What the reader's top row looks like.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Header {
    /// ` bible ` badge, translation, book and chapter (the original).
    Badge,
    /// Book and chapter on the left, translation dimmed beside them.
    Title,
    /// Printed-page running head with a rule underneath.
    RunningHead,
    /// Installed translations as chips, the active one filled.
    Chips,
}

pub(super) struct Theme {
    pub bg: Color,
    pub fg: Color,
    /// Secondary text: labels, translation names.
    pub dim: Color,
    /// Rules, scroll track, text faded behind an overlay.
    pub faint: Color,
    /// Focus marker, prompts, active elements.
    pub accent: Color,
    /// The parallel pane and other secondary emphasis.
    pub accent2: Color,
    pub key: Color,
    /// Book name and chapter number.
    pub title: Color,
    pub title2: Color,
    pub verse_num: Color,
    pub drop_cap: Color,
    pub border: Color,
    pub focus_bg: Color,
    pub hit_fg: Color,
    pub hit_bg: Color,
    /// Cards, the palette, pickers.
    pub panel_bg: Color,
    pub sel_bg: Color,
    pub bar_bg: Color,
    /// Mode pill and key caps.
    pub badge_fg: Color,
    pub badge_bg: Color,
    pub ok: Color,
    /// Drop shadow under overlays (light themes).
    pub shadow: Option<Color>,
    pub frame: Edge,
    pub overlay: Edge,
    pub header: Header,
    /// Centred on the running-head rule and in card titles.
    pub ornament: Option<&'static str>,
    /// Drawn in the margin beside the focused verse.
    pub mark: Option<&'static str>,
    /// Keys drawn as filled caps rather than coloured letters.
    pub keycaps: bool,
    /// Mode pill (READ, GO, FIND…) at the start of the status line.
    pub pill: bool,
    /// Letter-spaced uppercase headings (`J O H N`).
    pub spaced: bool,
    pub prefers_prose: bool,
    pub prefers_columns: bool,
    /// Keys reference as a full-width panel along the bottom instead of a
    /// centred card.
    pub keys_panel: bool,
}

const fn rgb(hex: u32) -> Color {
    Color::Rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

pub(super) fn resolve(preset: ThemePreset) -> Theme {
    let t = palette(preset);
    if truecolor() { t } else { t.folded() }
}

fn palette(preset: ThemePreset) -> Theme {
    match preset {
        ThemePreset::Nocturne => Theme {
            bg: rgb(0x0d0f15),
            fg: rgb(0xc9d0dd),
            dim: rgb(0x8d96aa),
            faint: rgb(0x2c3242),
            accent: rgb(0x8fa3ff),
            accent2: rgb(0x6fd6c8),
            key: rgb(0x8fa3ff),
            title: rgb(0xe6eaf2),
            title2: rgb(0x8fa3ff),
            verse_num: rgb(0x6a7387),
            drop_cap: rgb(0x8fa3ff),
            border: rgb(0x2c3242),
            focus_bg: rgb(0x161b27),
            hit_fg: rgb(0x0d0f15),
            hit_bg: rgb(0xe8c46a),
            panel_bg: rgb(0x11151f),
            sel_bg: rgb(0x1e2537),
            bar_bg: rgb(0x141824),
            badge_fg: rgb(0x0d0f15),
            badge_bg: rgb(0x8fa3ff),
            ok: rgb(0x7fd18b),
            shadow: None,
            frame: Edge::None,
            overlay: Edge::Rounded,
            header: Header::Title,
            ornament: None,
            mark: Some("▌"),
            keycaps: false,
            pill: true,
            spaced: false,
            prefers_prose: false,
            prefers_columns: false,
            keys_panel: true,
        },
        // Illuminated manuscript: umber ground, red rubrication, gold leaf.
        ThemePreset::Vellum => Theme {
            bg: rgb(0x16110d),
            fg: rgb(0xe7dcc5),
            dim: rgb(0xa09279),
            faint: rgb(0x46392c),
            accent: rgb(0xcda54e),
            accent2: rgb(0xd6593d),
            key: rgb(0xcda54e),
            title: rgb(0xcda54e),
            title2: rgb(0xd6593d),
            verse_num: rgb(0xd6593d),
            drop_cap: rgb(0xd6593d),
            border: rgb(0x5a4a36),
            focus_bg: rgb(0x2b2119),
            hit_fg: rgb(0x16110d),
            hit_bg: rgb(0xcda54e),
            panel_bg: rgb(0x1f1812),
            sel_bg: rgb(0x33281d),
            bar_bg: rgb(0x16110d),
            badge_fg: rgb(0x16110d),
            badge_bg: rgb(0xcda54e),
            ok: rgb(0x9fbf6a),
            shadow: None,
            frame: Edge::None,
            overlay: Edge::Double,
            header: Header::RunningHead,
            ornament: Some("◆"),
            mark: None,
            keycaps: false,
            pill: false,
            spaced: true,
            prefers_prose: true,
            prefers_columns: false,
            keys_panel: false,
        },
        // Black and gold.
        ThemePreset::Gilt => Theme {
            bg: rgb(0x0a0a0b),
            fg: rgb(0xece6d7),
            dim: rgb(0x8f897d),
            faint: rgb(0x3a362e),
            accent: rgb(0xd9b45b),
            accent2: rgb(0xc9a96e),
            key: rgb(0xd9b45b),
            title: rgb(0xd9b45b),
            title2: rgb(0xd9b45b),
            verse_num: rgb(0xd9b45b),
            drop_cap: rgb(0xd9b45b),
            border: rgb(0x80692f),
            focus_bg: rgb(0x19160e),
            hit_fg: rgb(0x0a0a0b),
            hit_bg: rgb(0xd9b45b),
            panel_bg: rgb(0x121112),
            sel_bg: rgb(0x241e12),
            bar_bg: rgb(0x0a0a0b),
            badge_fg: rgb(0x0a0a0b),
            badge_bg: rgb(0xd9b45b),
            ok: rgb(0xa8c47a),
            shadow: None,
            frame: Edge::Rounded,
            overlay: Edge::Rounded,
            header: Header::Chips,
            ornament: Some("◆"),
            mark: Some("▸"),
            keycaps: true,
            pill: false,
            spaced: true,
            prefers_prose: false,
            prefers_columns: false,
            keys_panel: false,
        },
        // Printed page: paper, ink, vermilion headings.
        ThemePreset::Daylight => Theme {
            bg: rgb(0xf6f1e6),
            fg: rgb(0x2b2723),
            dim: rgb(0x6f675b),
            faint: rgb(0xc9bfac),
            accent: rgb(0xb0432a),
            accent2: rgb(0x2c5a8a),
            key: rgb(0x2c5a8a),
            title: rgb(0xb0432a),
            title2: rgb(0xb0432a),
            verse_num: rgb(0xb0432a),
            drop_cap: rgb(0xb0432a),
            border: rgb(0xcfc5b3),
            focus_bg: rgb(0xebe0ca),
            hit_fg: rgb(0xfffbf3),
            hit_bg: rgb(0x2c5a8a),
            panel_bg: rgb(0xfffbf3),
            sel_bg: rgb(0xf1e3c3),
            bar_bg: rgb(0xf6f1e6),
            badge_fg: rgb(0xfffbf3),
            badge_bg: rgb(0xb0432a),
            ok: rgb(0x3f7d3a),
            shadow: Some(rgb(0xddd3c0)),
            frame: Edge::None,
            overlay: Edge::Plain,
            header: Header::RunningHead,
            ornament: None,
            mark: None,
            keycaps: false,
            pill: false,
            spaced: true,
            prefers_prose: true,
            prefers_columns: true,
            keys_panel: false,
        },
        ThemePreset::Classic => Theme {
            bg: Color::Reset,
            fg: Color::Reset,
            dim: Color::Indexed(244),
            faint: Color::Indexed(239),
            accent: Color::Cyan,
            accent2: Color::Magenta,
            key: Color::Cyan,
            title: Color::Cyan,
            title2: Color::Yellow,
            verse_num: Color::Indexed(244),
            drop_cap: Color::Yellow,
            border: Color::Indexed(24),
            focus_bg: Color::Indexed(237),
            hit_fg: Color::Black,
            hit_bg: Color::Yellow,
            panel_bg: Color::Reset,
            sel_bg: Color::Indexed(236),
            bar_bg: Color::Reset,
            badge_fg: Color::White,
            badge_bg: Color::Indexed(24),
            ok: Color::Green,
            ..terminal_chrome()
        },
        // Solarized Dark accents: base16 yellow/magenta/cyan/blue.
        ThemePreset::SolarizedDark => Theme {
            bg: Color::Reset,
            fg: Color::Reset,
            dim: Color::Indexed(243),
            faint: Color::Indexed(239),
            accent: Color::Indexed(37),
            accent2: Color::Indexed(125),
            key: Color::Indexed(37),
            title: Color::Indexed(37),
            title2: Color::Indexed(136),
            verse_num: Color::Indexed(243),
            drop_cap: Color::Indexed(136),
            border: Color::Indexed(33),
            focus_bg: Color::Indexed(235),
            hit_fg: Color::Indexed(235),
            hit_bg: Color::Indexed(136),
            panel_bg: Color::Reset,
            sel_bg: Color::Indexed(236),
            bar_bg: Color::Reset,
            badge_fg: Color::White,
            badge_bg: Color::Indexed(33),
            ok: Color::Indexed(64),
            ..terminal_chrome()
        },
        ThemePreset::HighContrast => Theme {
            bg: Color::Reset,
            fg: Color::White,
            dim: Color::Gray,
            faint: Color::Gray,
            accent: Color::White,
            accent2: Color::White,
            key: Color::White,
            title: Color::White,
            title2: Color::White,
            verse_num: Color::White,
            drop_cap: Color::White,
            border: Color::White,
            focus_bg: Color::DarkGray,
            hit_fg: Color::Black,
            hit_bg: Color::White,
            panel_bg: Color::Reset,
            sel_bg: Color::DarkGray,
            bar_bg: Color::Reset,
            badge_fg: Color::Black,
            badge_bg: Color::White,
            ok: Color::White,
            ..terminal_chrome()
        },
    }
}

/// Chrome shared by the themes that use the terminal's own colours: bordered
/// panes, the badge header, no extras.
fn terminal_chrome() -> Theme {
    Theme {
        bg: Color::Reset,
        fg: Color::Reset,
        dim: Color::Reset,
        faint: Color::Reset,
        accent: Color::Reset,
        accent2: Color::Reset,
        key: Color::Reset,
        title: Color::Reset,
        title2: Color::Reset,
        verse_num: Color::Reset,
        drop_cap: Color::Reset,
        border: Color::Reset,
        focus_bg: Color::Reset,
        hit_fg: Color::Reset,
        hit_bg: Color::Reset,
        panel_bg: Color::Reset,
        sel_bg: Color::Reset,
        bar_bg: Color::Reset,
        badge_fg: Color::Reset,
        badge_bg: Color::Reset,
        ok: Color::Reset,
        shadow: None,
        frame: Edge::Plain,
        overlay: Edge::Plain,
        header: Header::Badge,
        ornament: None,
        mark: None,
        keycaps: false,
        pill: false,
        spaced: false,
        prefers_prose: false,
        prefers_columns: false,
        keys_panel: false,
    }
}

impl Theme {
    fn folded(self) -> Self {
        Theme {
            bg: fold(self.bg),
            fg: fold(self.fg),
            dim: fold(self.dim),
            faint: fold(self.faint),
            accent: fold(self.accent),
            accent2: fold(self.accent2),
            key: fold(self.key),
            title: fold(self.title),
            title2: fold(self.title2),
            verse_num: fold(self.verse_num),
            drop_cap: fold(self.drop_cap),
            border: fold(self.border),
            focus_bg: fold(self.focus_bg),
            hit_fg: fold(self.hit_fg),
            hit_bg: fold(self.hit_bg),
            panel_bg: fold(self.panel_bg),
            sel_bg: fold(self.sel_bg),
            bar_bg: fold(self.bar_bg),
            badge_fg: fold(self.badge_fg),
            badge_bg: fold(self.badge_bg),
            ok: fold(self.ok),
            shadow: self.shadow.map(fold),
            ..self
        }
    }

    /// Body text on the screen background.
    pub fn base(&self) -> Style {
        Style::default().fg(self.fg).bg(self.bg)
    }

    /// Text inside a card, palette or picker.
    pub fn panel(&self) -> Style {
        Style::default().fg(self.fg).bg(self.panel_bg)
    }

    pub fn dim(&self) -> Style {
        Style::default().fg(self.dim)
    }

    pub fn block(&self, edge: Edge, color: Color) -> Block<'static> {
        let kind = match edge {
            Edge::None => return Block::default(),
            Edge::Plain => BorderType::Plain,
            Edge::Rounded => BorderType::Rounded,
            Edge::Double => BorderType::Double,
        };
        Block::default()
            .borders(Borders::ALL)
            .border_type(kind)
            .border_style(Style::default().fg(color))
    }

    /// A key as it appears in hints and the keys card.
    pub fn key_span(&self, key: &str) -> Span<'static> {
        if self.keycaps {
            Span::styled(
                format!(" {key} "),
                Style::default()
                    .fg(self.badge_fg)
                    .bg(self.badge_bg)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(
                key.to_string(),
                Style::default().fg(self.key).add_modifier(Modifier::BOLD),
            )
        }
    }

    /// Width of [`Self::key_span`] for `key`.
    pub fn key_width(&self, key: &str) -> usize {
        unicode_width::UnicodeWidthStr::width(key) + if self.keycaps { 2 } else { 0 }
    }

    /// A heading in this theme's voice: `J O H N` when letter-spaced.
    pub fn heading(&self, s: &str) -> String {
        if self.spaced {
            letter_spaced(&s.to_uppercase())
        } else {
            s.to_string()
        }
    }
}

pub(super) fn letter_spaced(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for (i, c) in s.chars().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

/// Whether the terminal takes 24-bit colour. `BIBLE_TRUECOLOR=0/1`
/// overrides the detection.
pub(super) fn truecolor() -> bool {
    static TC: OnceLock<bool> = OnceLock::new();
    *TC.get_or_init(|| {
        if let Ok(v) = std::env::var("BIBLE_TRUECOLOR") {
            return v != "0";
        }
        if matches!(
            std::env::var("COLORTERM").as_deref(),
            Ok("truecolor" | "24bit")
        ) {
            return true;
        }
        matches!(
            std::env::var("TERM_PROGRAM").as_deref(),
            Ok("iTerm.app" | "WezTerm" | "ghostty" | "vscode")
        )
    })
}

fn fold(c: Color) -> Color {
    match c {
        Color::Rgb(r, g, b) => Color::Indexed(rgb_to_xterm(r, g, b)),
        other => other,
    }
}

/// Nearest xterm-256 colour: the closer of the 6×6×6 cube entry and the
/// grey ramp entry.
pub(super) fn rgb_to_xterm(r: u8, g: u8, b: u8) -> u8 {
    const STEPS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    let nearest = |v: u8| -> usize {
        (0..6)
            .min_by_key(|&i| (STEPS[i] as i32 - v as i32).abs())
            .unwrap_or(0)
    };
    let dist = |(x, y, z): (u8, u8, u8)| -> i32 {
        let (dr, dg, db) = (
            x as i32 - r as i32,
            y as i32 - g as i32,
            z as i32 - b as i32,
        );
        dr * dr + dg * dg + db * db
    };
    let (ri, gi, bi) = (nearest(r), nearest(g), nearest(b));
    let cube = (STEPS[ri], STEPS[gi], STEPS[bi]);
    let avg = (r as u32 + g as u32 + b as u32) / 3;
    let grey_i = if avg < 8 { 0 } else { ((avg - 8) / 10).min(23) };
    let grey_v = (8 + 10 * grey_i) as u8;
    if dist((grey_v, grey_v, grey_v)) < dist(cube) {
        232 + grey_i as u8
    } else {
        16 + 36 * ri as u8 + 6 * gi as u8 + bi as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xterm_folding_hits_cube_corners_and_greys() {
        assert_eq!(rgb_to_xterm(0, 0, 0), 16);
        assert_eq!(rgb_to_xterm(255, 255, 255), 231);
        assert_eq!(rgb_to_xterm(255, 0, 0), 196);
        assert_eq!(rgb_to_xterm(128, 128, 128), 244);
    }

    #[test]
    fn folded_theme_has_no_rgb_left() {
        for preset in ThemePreset::ALL {
            let t = palette(preset).folded();
            for c in [
                t.bg, t.fg, t.dim, t.faint, t.accent, t.focus_bg, t.panel_bg, t.sel_bg,
            ] {
                assert!(!matches!(c, Color::Rgb(..)), "{preset:?} kept {c:?}");
            }
        }
    }

    #[test]
    fn legacy_default_preset_follows_the_default() {
        #[derive(serde::Deserialize)]
        struct T {
            preset: ThemePreset,
        }
        let t: T = toml::from_str("preset = \"default\"").unwrap();
        assert_eq!(t.preset, ThemePreset::default());
        assert_eq!(t.preset, ThemePreset::Vellum);
        let t: T = toml::from_str("preset = \"solarized-dark\"").unwrap();
        assert_eq!(t.preset, ThemePreset::SolarizedDark);
    }
}
