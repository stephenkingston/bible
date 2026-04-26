//! Persistent UI state — last reading position and parallel-view config.
//!
//! Stored as TOML at `<config_dir>/state.toml`. All operations are best-effort:
//! `load()` returns `None` on any failure (missing file, parse error, schema
//! mismatch), and `save()` returns a `Result` that callers should treat as
//! non-fatal.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::error::{Error, Result};
use crate::storage::config_dir;

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub schema_version: u32,
    #[serde(default)]
    pub last_position: Option<LastPosition>,
    #[serde(default)]
    pub parallel: Option<Parallel>,
}

impl State {
    pub fn new() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            last_position: None,
            parallel: None,
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastPosition {
    pub translation: String,
    pub book_number: u8,
    pub chapter: u16,
    pub scroll: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parallel {
    pub secondary_translation: String,
}

fn state_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("state.toml"))
}

pub fn load() -> Option<State> {
    let path = state_path().ok()?;
    let s = fs::read_to_string(&path).ok()?;
    let state: State = toml::from_str(&s).ok()?;
    if state.schema_version != SCHEMA_VERSION {
        return None;
    }
    Some(state)
}

pub fn save(state: &State) -> Result<()> {
    let path = state_path()?;
    let s = toml::to_string_pretty(state).map_err(|e| Error::Toml(e.to_string()))?;
    fs::write(&path, s).map_err(Error::Io)?;
    Ok(())
}
