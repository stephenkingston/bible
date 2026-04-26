use directories::ProjectDirs;
use std::fs;
use std::path::PathBuf;

use crate::bible::{Bible, TranslationInfo};
use crate::error::{Error, Result};

const QUALIFIER: &str = "com";
const ORG: &str = "stephenkingston";
const APP: &str = "bible";

pub fn project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from(QUALIFIER, ORG, APP).ok_or(Error::NoDataDir)
}

pub fn data_dir() -> Result<PathBuf> {
    let p = project_dirs()?.data_dir().to_path_buf();
    fs::create_dir_all(&p)?;
    Ok(p)
}

pub fn config_dir() -> Result<PathBuf> {
    let p = project_dirs()?.config_dir().to_path_buf();
    fs::create_dir_all(&p)?;
    Ok(p)
}

pub fn translations_dir() -> Result<PathBuf> {
    let p = data_dir()?.join("translations");
    fs::create_dir_all(&p)?;
    Ok(p)
}

pub fn translation_dir(id: &str) -> Result<PathBuf> {
    Ok(translations_dir()?.join(id))
}

pub fn save_bible(bible: &Bible) -> Result<()> {
    let dir = translation_dir(&bible.translation.id)?;
    fs::create_dir_all(&dir)?;

    let bin = bincode::serde::encode_to_vec(bible, bincode::config::standard())
        .map_err(|e| Error::Bincode(e.to_string()))?;
    fs::write(dir.join("bible.bin"), bin)?;

    let meta = serde_json::to_string_pretty(&bible.translation)?;
    fs::write(dir.join("meta.json"), meta)?;

    Ok(())
}

pub fn load_bible(id: &str) -> Result<Bible> {
    let path = translation_dir(id)?.join("bible.bin");
    if !path.exists() {
        return Err(Error::NotInstalled(id.to_string()));
    }
    let bytes = fs::read(&path)?;
    let (bible, _): (Bible, usize) =
        bincode::serde::decode_from_slice(&bytes, bincode::config::standard())
            .map_err(|e| Error::Bincode(e.to_string()))?;
    Ok(bible)
}

pub fn is_installed(id: &str) -> bool {
    translation_dir(id)
        .map(|d| d.join("bible.bin").exists())
        .unwrap_or(false)
}

pub fn list_installed() -> Result<Vec<TranslationInfo>> {
    let dir = match translations_dir() {
        Ok(d) => d,
        Err(_) => return Ok(Vec::new()),
    };
    let mut out = Vec::new();
    if !dir.exists() {
        return Ok(out);
    }
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let meta_path = entry.path().join("meta.json");
        if let Ok(s) = fs::read_to_string(&meta_path) {
            if let Ok(info) = serde_json::from_str::<TranslationInfo>(&s) {
                out.push(info);
            }
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

pub fn uninstall(id: &str) -> Result<()> {
    let dir = translation_dir(id)?;
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    Ok(())
}
