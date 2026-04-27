//! Bible reading plans. v1 ships a single deterministic plan, "Bible in
//! a Year" — every chapter of the Protestant canon (1189 of them) split
//! across 365 or 366 days in canonical order. Only progress is persisted;
//! the day → references mapping is regenerated on every launch.

use chrono::{Datelike, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use crate::error::{Error, Result};
use crate::reference::{get_bible_book_by_number, get_number_of_chapters};
use crate::storage::config_dir;

const SCHEMA_VERSION: u32 = 1;
pub const PLAN_ID: &str = "bible-in-a-year";

#[derive(Debug, Clone)]
pub struct ReadingPlan {
    pub plan_id: String,
    pub year: i32,
    pub days: Vec<DailyReading>,
}

#[derive(Debug, Clone)]
pub struct DailyReading {
    /// 1-based day-of-year.
    pub day: u16,
    pub date: NaiveDate,
    pub refs: Vec<ChapterRange>,
}

#[derive(Debug, Clone)]
pub struct ChapterRange {
    pub book_number: u8,
    pub chapter_start: u16,
    pub chapter_end: u16,
}

impl ChapterRange {
    /// Compact form e.g. "Gen 14-15" or "Exo 1".
    pub fn short(&self) -> String {
        let book = short_book_name(self.book_number);
        if self.chapter_start == self.chapter_end {
            format!("{} {}", book, self.chapter_start)
        } else {
            format!("{} {}-{}", book, self.chapter_start, self.chapter_end)
        }
    }
}

impl DailyReading {
    /// Comma-joined short form, e.g. "Gen 14-15, Exo 1".
    pub fn short(&self) -> String {
        self.refs
            .iter()
            .map(|r| r.short())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Three-letter book abbreviation (SBL-ish, fits in narrow status lines).
fn short_book_name(num: u8) -> &'static str {
    match num {
        1 => "Gen",
        2 => "Exo",
        3 => "Lev",
        4 => "Num",
        5 => "Deu",
        6 => "Jos",
        7 => "Jdg",
        8 => "Rut",
        9 => "1Sa",
        10 => "2Sa",
        11 => "1Ki",
        12 => "2Ki",
        13 => "1Ch",
        14 => "2Ch",
        15 => "Ezr",
        16 => "Neh",
        17 => "Est",
        18 => "Job",
        19 => "Psa",
        20 => "Pro",
        21 => "Ecc",
        22 => "Sng",
        23 => "Isa",
        24 => "Jer",
        25 => "Lam",
        26 => "Eze",
        27 => "Dan",
        28 => "Hos",
        29 => "Joe",
        30 => "Amo",
        31 => "Oba",
        32 => "Jon",
        33 => "Mic",
        34 => "Nah",
        35 => "Hab",
        36 => "Zep",
        37 => "Hag",
        38 => "Zec",
        39 => "Mal",
        40 => "Mat",
        41 => "Mar",
        42 => "Luk",
        43 => "Jhn",
        44 => "Act",
        45 => "Rom",
        46 => "1Co",
        47 => "2Co",
        48 => "Gal",
        49 => "Eph",
        50 => "Phi",
        51 => "Col",
        52 => "1Th",
        53 => "2Th",
        54 => "1Ti",
        55 => "2Ti",
        56 => "Tit",
        57 => "Phm",
        58 => "Heb",
        59 => "Jas",
        60 => "1Pe",
        61 => "2Pe",
        62 => "1Jn",
        63 => "2Jn",
        64 => "3Jn",
        65 => "Jud",
        66 => "Rev",
        _ => "?",
    }
}

pub fn current_year() -> i32 {
    Local::now().year()
}

/// 1-based day-of-year for today (local time).
pub fn today_day_of_year() -> u16 {
    Local::now().date_naive().ordinal() as u16
}

fn is_leap(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Build the plan deterministically — same year always yields the same
/// reading sequence regardless of who's calling or when.
pub fn generate_bible_in_a_year(year: i32) -> ReadingPlan {
    // Flatten the canon into (book, chapter) pairs in canonical order.
    let mut all: Vec<(u8, u16)> = Vec::with_capacity(1200);
    for book_num in 1..=66u8 {
        if let Some(book) = get_bible_book_by_number(book_num) {
            let nch: u32 = get_number_of_chapters(&book).into();
            for c in 1..=nch as u16 {
                all.push((book_num, c));
            }
        }
    }

    let total_days: u16 = if is_leap(year) { 366 } else { 365 };
    let total = all.len();
    let base = total / total_days as usize;
    let extra = total % total_days as usize;

    let mut days = Vec::with_capacity(total_days as usize);
    let mut idx = 0usize;
    for d in 1..=total_days {
        // Front-load the leftover chapters onto the first `extra` days.
        let n = base + if (d as usize) <= extra { 1 } else { 0 };
        let slice = &all[idx..idx + n];
        idx += n;
        let date = NaiveDate::from_yo_opt(year, d as u32)
            .expect("valid day-of-year");
        days.push(DailyReading {
            day: d,
            date,
            refs: chunk_to_ranges(slice),
        });
    }
    ReadingPlan {
        plan_id: PLAN_ID.to_string(),
        year,
        days,
    }
}

/// Coalesce consecutive (book, chapter) pairs into ChapterRanges so a day
/// like Genesis 14, Genesis 15, Exodus 1 renders as "Gen 14-15, Exo 1".
fn chunk_to_ranges(chs: &[(u8, u16)]) -> Vec<ChapterRange> {
    let mut out = Vec::new();
    if chs.is_empty() {
        return out;
    }
    let mut cur = ChapterRange {
        book_number: chs[0].0,
        chapter_start: chs[0].1,
        chapter_end: chs[0].1,
    };
    for &(b, c) in &chs[1..] {
        if b == cur.book_number && c == cur.chapter_end + 1 {
            cur.chapter_end = c;
        } else {
            out.push(cur);
            cur = ChapterRange {
                book_number: b,
                chapter_start: c,
                chapter_end: c,
            };
        }
    }
    out.push(cur);
    out
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlanFile {
    pub schema_version: u32,
    pub year: i32,
    pub plan_id: String,
    #[serde(default)]
    pub completed_days: Vec<u16>,
}

fn plan_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("plan.toml"))
}

pub fn load_progress() -> Option<PlanFile> {
    let path = plan_path().ok()?;
    let s = fs::read_to_string(&path).ok()?;
    let parsed: PlanFile = toml::from_str(&s).ok()?;
    if parsed.schema_version != SCHEMA_VERSION {
        return None;
    }
    Some(parsed)
}

pub fn save_progress(file: &PlanFile) -> Result<()> {
    let path = plan_path()?;
    let s = toml::to_string_pretty(file).map_err(|e| Error::Toml(e.to_string()))?;
    fs::write(&path, s).map_err(Error::Io)?;
    Ok(())
}

/// Restore a HashSet of completed days for `year`/`plan_id`. Anything else
/// (missing file, schema mismatch, year/plan-id mismatch) yields empty.
pub fn load_completed_for(year: i32, plan_id: &str) -> HashSet<u16> {
    match load_progress() {
        Some(f) if f.year == year && f.plan_id == plan_id => {
            f.completed_days.into_iter().collect()
        }
        _ => HashSet::new(),
    }
}
