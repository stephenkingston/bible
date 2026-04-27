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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Typography {
    #[serde(default)]
    pub script_letter_padding: ScriptPadding,
    #[serde(default)]
    pub word_padding: u8,
    #[serde(default)]
    pub verse_spacing: u8,
    #[serde(default)]
    pub line_spacing: u8,
    #[serde(default)]
    pub verse_number_style: VerseNumberStyle,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemePreset {
    Default,
    SolarizedDark,
    HighContrast,
}

impl Default for ThemePreset {
    fn default() -> Self {
        ThemePreset::Default
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReaderSettings {
    /// Hard cap on chapter-pane width in cells. `0` = no cap.
    #[serde(default)]
    pub max_columns: u16,
    /// Translation id loaded on startup when no saved position exists. Empty
    /// string = "first installed alphabetically".
    #[serde(default)]
    pub default_translation: String,
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
