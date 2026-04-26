use std::io::Read;
use std::time::Duration;

use crate::bible::Bible;
use crate::error::{Error, Result};
use crate::storage;

const RAW_URL_BASE: &str =
    "https://raw.githubusercontent.com/Beblia/Holy-Bible-XML-Format/master/";

/// Callback signature: `progress(bytes_so_far, total_bytes_if_known)`.
pub type Progress<'a> = &'a mut dyn FnMut(u64, Option<u64>);

pub fn install(id: &str, mut progress: Option<Progress<'_>>) -> Result<Bible> {
    let url = format!("{RAW_URL_BASE}{id}.xml");
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(60))
        .user_agent(concat!("bible-tui/", env!("CARGO_PKG_VERSION")))
        .build();

    let resp = agent
        .get(&url)
        .call()
        .map_err(|e| Error::Http(e.to_string()))?;

    let total = resp
        .header("Content-Length")
        .and_then(|v| v.parse::<u64>().ok());

    let mut reader = resp.into_reader();
    let mut buf = Vec::with_capacity(total.unwrap_or(2 * 1024 * 1024) as usize);
    let mut chunk = [0u8; 16 * 1024];
    let mut bytes: u64 = 0;
    loop {
        let n = reader
            .read(&mut chunk)
            .map_err(|e| Error::Http(e.to_string()))?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        bytes += n as u64;
        if let Some(p) = progress.as_deref_mut() {
            p(bytes, total);
        }
    }

    let xml = String::from_utf8(buf).map_err(|e| Error::Xml(format!("not utf-8: {e}")))?;
    let bible = crate::parser::parse_xml(&xml, id)?;
    storage::save_bible(&bible)?;
    Ok(bible)
}
