//! Persistent bookmarks. TOML at `<config_dir>/bookmarks.toml`.
//!
//! Granularity: chapter-level by default, with optional verse and note.
//! Schema-versioned; on mismatch the file is ignored and bookmarks start
//! fresh (the user's existing bookmarks file is left intact, not clobbered).

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::error::{Error, Result};
use crate::storage::config_dir;

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmarksFile {
    pub schema_version: u32,
    #[serde(default)]
    pub bookmarks: Vec<Bookmark>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub translation: String,
    pub book_number: u8,
    pub chapter: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verse: Option<u16>,
    #[serde(default)]
    pub note: String,
    pub created_at: String,
}

fn path() -> Result<PathBuf> {
    Ok(config_dir()?.join("bookmarks.toml"))
}

pub fn load() -> Vec<Bookmark> {
    let Ok(p) = path() else {
        return Vec::new();
    };
    let Ok(s) = fs::read_to_string(&p) else {
        return Vec::new();
    };
    let Ok(parsed) = toml::from_str::<BookmarksFile>(&s) else {
        return Vec::new();
    };
    if parsed.schema_version != SCHEMA_VERSION {
        return Vec::new();
    }
    parsed.bookmarks
}

pub fn save(bookmarks: &[Bookmark]) -> Result<()> {
    let p = path()?;
    let file = BookmarksFile {
        schema_version: SCHEMA_VERSION,
        bookmarks: bookmarks.to_vec(),
    };
    let s = toml::to_string_pretty(&file).map_err(|e| Error::Toml(e.to_string()))?;
    fs::write(&p, s).map_err(Error::Io)?;
    Ok(())
}
