//! User-configurable settings — typography, theme, reader width, parallel
//! divider. TOML at `<config_dir>/settings.toml`. Schema-versioned; on
//! mismatch the file is ignored and defaults are used (file is left intact).

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::error::{Error, Result};
use crate::storage::config_dir;

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub schema_version: u32,
    #[serde(default)]
    pub typography: Typography,
    #[serde(default)]
    pub theme: ThemeSettings,
    #[serde(default)]
    pub reader: ReaderSettings,
    #[serde(default)]
    pub parallel: ParallelSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            typography: Typography::default(),
            theme: ThemeSettings::default(),
            reader: ReaderSettings::default(),
            parallel: ParallelSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Typography {
    #[serde(default)]
    pub script_letter_padding: ScriptPadding,
    #[serde(default)]
    pub word_padding: u8,
    #[serde(default = "default_verse_spacing")]
    pub verse_spacing: u8,
    #[serde(default)]
    pub line_spacing: u8,
    #[serde(default)]
    pub verse_number_style: VerseNumberStyle,
    #[serde(default = "default_true")]
    pub justify: bool,
}

impl Default for Typography {
    fn default() -> Self {
        Self {
            script_letter_padding: ScriptPadding::default(),
            word_padding: 0,
            verse_spacing: default_verse_spacing(),
            line_spacing: 0,
            verse_number_style: VerseNumberStyle::default(),
            justify: default_true(),
        }
    }
}

fn default_verse_spacing() -> u8 {
    1
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScriptPadding {
    #[serde(default)]
    pub default: u8,
    #[serde(default)]
    pub tamil: u8,
    #[serde(default)]
    pub devanagari: u8,
    #[serde(default)]
    pub arabic: u8,
    #[serde(default)]
    pub hebrew: u8,
    #[serde(default)]
    pub cjk: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VerseNumberStyle {
    InlineBold,
    Superscript,
    Hidden,
}

impl Default for VerseNumberStyle {
    fn default() -> Self {
        VerseNumberStyle::InlineBold
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ThemeSettings {
    #[serde(default)]
    pub preset: ThemePreset,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemePreset {
    /// Settings files written before themes existed say `default`, which was
    /// only ever the default, not a choice; it follows the current default.
    #[default]
    #[serde(alias = "default")]
    Nocturne,
    Vellum,
    Gilt,
    Daylight,
    /// The original look (bordered panes, terminal colours).
    Classic,
    SolarizedDark,
    HighContrast,
}

impl ThemePreset {
    pub const ALL: [ThemePreset; 7] = [
        ThemePreset::Nocturne,
        ThemePreset::Vellum,
        ThemePreset::Gilt,
        ThemePreset::Daylight,
        ThemePreset::Classic,
        ThemePreset::SolarizedDark,
        ThemePreset::HighContrast,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ThemePreset::Nocturne => "Nocturne",
            ThemePreset::Vellum => "Vellum",
            ThemePreset::Gilt => "Gilt",
            ThemePreset::Daylight => "Daylight",
            ThemePreset::Classic => "Classic",
            ThemePreset::SolarizedDark => "Solarized dark",
            ThemePreset::HighContrast => "High contrast",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReaderSettings {
    /// Cap on the reading pane's width in cells: `0` = auto (a comfortable
    /// measure), [`FULL_WIDTH`] = use the whole terminal.
    #[serde(default)]
    pub max_columns: u16,
    /// Translation id loaded on startup when no saved position exists. Empty
    /// string = "first installed alphabetically".
    #[serde(default)]
    pub default_translation: String,
    #[serde(default)]
    pub layout: ReadingLayout,
    #[serde(default)]
    pub columns: ColumnLayout,
    /// Large chapter numeral at the start of a chapter in prose layout.
    #[serde(default = "default_true")]
    pub drop_cap: bool,
    /// Mouse wheel scrolls, click focuses a verse. Off restores the
    /// terminal's own text selection.
    #[serde(default = "default_true")]
    pub mouse: bool,
}

impl Default for ReaderSettings {
    fn default() -> Self {
        Self {
            max_columns: 0,
            default_translation: String::new(),
            layout: ReadingLayout::default(),
            columns: ColumnLayout::default(),
            drop_cap: true,
            mouse: true,
        }
    }
}

/// `max_columns` value meaning "no cap".
pub const FULL_WIDTH: u16 = u16::MAX;

/// How a chapter is laid out. `Auto` follows the theme.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReadingLayout {
    #[default]
    Auto,
    /// One verse per block, number in a gutter.
    Verses,
    /// Verses run together into paragraphs with superscript numbers.
    Prose,
}

/// Single or two-column reading. `Auto` uses two columns when the theme
/// prefers them and the terminal is wide enough.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ColumnLayout {
    #[default]
    Auto,
    One,
    Two,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParallelSettings {
    #[serde(default)]
    pub divider: DividerStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DividerStyle {
    Single,
    Double,
    None,
}

impl Default for DividerStyle {
    fn default() -> Self {
        DividerStyle::Single
    }
}

fn settings_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("settings.toml"))
}

pub fn load() -> Settings {
    let Ok(path) = settings_path() else {
        return Settings::default();
    };
    let Ok(s) = fs::read_to_string(&path) else {
        return Settings::default();
    };
    let Ok(parsed) = toml::from_str::<Settings>(&s) else {
        return Settings::default();
    };
    if parsed.schema_version != SCHEMA_VERSION {
        return Settings::default();
    }
    parsed
}

pub fn save(s: &Settings) -> Result<()> {
    let path = settings_path()?;
    let toml_str = toml::to_string_pretty(s).map_err(|e| Error::Toml(e.to_string()))?;
    fs::write(&path, toml_str).map_err(Error::Io)?;
    Ok(())
}
