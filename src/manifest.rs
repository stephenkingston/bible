//! Catalog of available translations.
//!
//! A small static fallback ships embedded in the binary for offline browsing of
//! the most-likely picks. `refresh()` hits the GitHub Trees API once to fetch the
//! full Beblia catalog and caches it under the user's config dir.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use crate::error::{Error, Result};
use crate::storage::config_dir;

const STATIC_MANIFEST: &str = include_str!("../assets/manifest.json");
const TREES_URL: &str = "https://api.github.com/repos/Beblia/Holy-Bible-XML-Format/git/trees/master?recursive=1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableTranslation {
    pub id: String,
    pub display_name: String,
    pub language: String,
    #[serde(default)]
    pub blob_sha: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedManifest {
    #[serde(default)]
    pub last_refreshed: Option<String>,
    #[serde(default)]
    pub tree_sha: Option<String>,
    pub translations: Vec<AvailableTranslation>,
}

fn manifest_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("manifest.json"))
}

pub fn cached() -> Option<CachedManifest> {
    let p = manifest_path().ok()?;
    let s = fs::read_to_string(p).ok()?;
    serde_json::from_str(&s).ok()
}

pub fn list_available() -> Vec<AvailableTranslation> {
    if let Some(m) = cached() {
        if !m.translations.is_empty() {
            return m.translations;
        }
    }
    serde_json::from_str::<CachedManifest>(STATIC_MANIFEST)
        .map(|m| m.translations)
        .unwrap_or_default()
}

#[derive(Debug, Deserialize)]
struct TreeResponse {
    sha: String,
    tree: Vec<TreeEntry>,
}

#[derive(Debug, Deserialize)]
struct TreeEntry {
    path: String,
    #[serde(rename = "type")]
    kind: String,
    sha: String,
}

pub fn refresh() -> Result<CachedManifest> {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        .user_agent(concat!("bible-tui/", env!("CARGO_PKG_VERSION")))
        .build();
    let agent: ureq::Agent = config.into();

    let mut resp = agent
        .get(TREES_URL)
        .call()
        .map_err(|e| Error::Http(e.to_string()))?;

    let parsed: TreeResponse = resp
        .body_mut()
        .read_json()
        .map_err(|e| Error::Http(e.to_string()))?;

    let mut translations: Vec<AvailableTranslation> = parsed
        .tree
        .into_iter()
        .filter(|e| e.kind == "blob" && e.path.ends_with("Bible.xml") && !e.path.contains('/'))
        .map(|e| {
            let id = e.path.trim_end_matches(".xml").to_string();
            let display_name = crate::parser::friendly_name(&id);
            let language = crate::parser::guess_language(&id);
            AvailableTranslation {
                id,
                display_name,
                language,
                blob_sha: Some(e.sha),
            }
        })
        .collect();
    translations.sort_by(|a, b| a.id.cmp(&b.id));

    let cache = CachedManifest {
        last_refreshed: Some(chrono::Utc::now().to_rfc3339()),
        tree_sha: Some(parsed.sha),
        translations,
    };

    let path = manifest_path()?;
    fs::write(&path, serde_json::to_string_pretty(&cache)?)?;
    Ok(cache)
}

const ALIASES: &[(&str, &str)] = &[
    ("kjv", "EnglishKJBible"),
    ("asv", "EnglishASVBible"),
    ("web", "EnglishWEBBible"),
    ("ylt", "EnglishYLTBible"),
];

/// Resolve a user-supplied alias or partial id to a canonical Beblia file id.
/// Lookup order: exact id → case-insensitive alias → case-insensitive prefix
/// match against installed and available lists.
pub fn resolve_id(input: &str) -> Result<String> {
    use crate::storage;

    if storage::is_installed(input) {
        return Ok(input.to_string());
    }
    for (alias, full) in ALIASES {
        if alias.eq_ignore_ascii_case(input) {
            return Ok((*full).to_string());
        }
    }
    let needle = input.to_ascii_lowercase();
    let installed = storage::list_installed().unwrap_or_default();
    if let Some(t) = installed
        .iter()
        .find(|t| t.id.to_ascii_lowercase().starts_with(&needle))
    {
        return Ok(t.id.clone());
    }
    let available = list_available();
    if let Some(t) = available
        .iter()
        .find(|t| t.id.eq_ignore_ascii_case(input))
    {
        return Ok(t.id.clone());
    }
    if let Some(t) = available
        .iter()
        .find(|t| t.id.to_ascii_lowercase().starts_with(&needle))
    {
        return Ok(t.id.clone());
    }
    Err(Error::UnknownTranslation(input.to_string()))
}
