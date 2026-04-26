//! Re-exports of `bibleref` reference types and small helpers.
//!
//! All canonical reference handling (parsing "John 3:16", validating chapter/verse
//! ranges, multilingual output) is delegated to the `bibleref` crate. This module is
//! a thin adapter that wires bibleref errors into our `Error` type and offers a few
//! conveniences (book number → enum, pretty book name).

pub use bibleref::bible::validate::get_number_of_chapters;
pub use bibleref::bible::{
    BibleBook, BibleBookReference, BibleChapter, BibleChapterReference, BibleReference,
    BibleReferenceRepresentation, BibleVerse, BibleVerseReference, get_bible_book_by_number,
};

use crate::error::{Error, Result};

/// Parse a free-form reference string like "John 3:16" via `bibleref`.
///
/// `bibleref` 0.4's parser is case-sensitive and only accepts its exact
/// variant names (e.g. `Psalm`, not `Psalms`; `Genesis`, not `genesis`). We
/// retry with a few common normalisations before giving up so users can type
/// references the natural way.
pub fn parse(s: &str) -> Result<BibleReferenceRepresentation> {
    let s = s.trim();
    if s.is_empty() {
        return Err(Error::Reference("empty reference".into()));
    }

    let mut last_err: Option<String> = None;
    for candidate in candidate_forms(s) {
        match bibleref::parse(&candidate) {
            Ok(r) => return Ok(r),
            Err(e) => last_err = Some(e.to_string()),
        }
    }
    Err(Error::Reference(
        last_err.unwrap_or_else(|| format!("could not parse `{s}`")),
    ))
}

fn candidate_forms(s: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![s.to_string()];
    let mut push = |c: String| {
        if !out.contains(&c) {
            out.push(c);
        }
    };

    // Capitalise only the first character (covers `psalm 5` → `Psalm 5`
    // without mangling things like `1 samuel 3`).
    let mut chars = s.chars();
    if let Some(first) = chars.next() {
        if first.is_lowercase() {
            push(first.to_uppercase().chain(chars).collect());
        }
    }

    // Title-case the leading book-name run (everything before the first
    // digit or colon). Handles `first samuel 3` → `First Samuel 3` etc.
    push(title_case_book_part(s));

    // bibleref's variant for the Psalter is singular (`Psalm`); users
    // commonly type `Psalms`. Try both regardless of input case.
    let lower = s.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix("psalms") {
        push(format!("Psalm{rest}"));
        push(format!("Psalms{rest}"));
    } else if let Some(rest) = lower.strip_prefix("psalm") {
        push(format!("Psalms{rest}"));
        push(format!("Psalm{rest}"));
    }

    out
}

fn title_case_book_part(s: &str) -> String {
    let split = s
        .find(|c: char| c.is_ascii_digit() || c == ':')
        .unwrap_or(s.len());
    // Don't title-case if the input starts with a digit (e.g. "1 Samuel" —
    // the digit prefix is already canonical).
    if s.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return s.to_string();
    }
    let head = &s[..split];
    let tail = &s[split..];
    let titled: Vec<String> = head
        .split_whitespace()
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first
                    .to_uppercase()
                    .chain(chars.flat_map(|c| c.to_lowercase()))
                    .collect(),
            }
        })
        .collect();
    if titled.is_empty() {
        return s.to_string();
    }
    let joined = titled.join(" ");
    if tail.is_empty() {
        joined
    } else {
        format!("{joined} {}", tail.trim_start())
    }
}

/// Convert a 1-based book number to a `BibleBook`, returning a typed error.
pub fn book_from_number(n: u8) -> Result<BibleBook> {
    get_bible_book_by_number(n).ok_or(Error::InvalidBookNumber(n))
}

/// Pretty book name in English (e.g. "1 Samuel", "Song of Solomon", "Revelation").
/// `bibleref::BibleBook` uses variant names like `ISamuel`/`SongofSolomon` that are
/// awkward to print directly.
pub fn book_display(b: &BibleBook) -> &'static str {
    use BibleBook::*;
    match b {
        Genesis => "Genesis",
        Exodus => "Exodus",
        Leviticus => "Leviticus",
        Numbers => "Numbers",
        Deuteronomy => "Deuteronomy",
        Joshua => "Joshua",
        Judges => "Judges",
        Ruth => "Ruth",
        ISamuel => "1 Samuel",
        IISamuel => "2 Samuel",
        IKings => "1 Kings",
        IIKings => "2 Kings",
        IChronicles => "1 Chronicles",
        IIChronicles => "2 Chronicles",
        Ezra => "Ezra",
        Nehemiah => "Nehemiah",
        Esther => "Esther",
        Job => "Job",
        Psalm => "Psalms",
        Proverbs => "Proverbs",
        Ecclesiastes => "Ecclesiastes",
        SongofSolomon => "Song of Solomon",
        Isaiah => "Isaiah",
        Jeremiah => "Jeremiah",
        Lamentations => "Lamentations",
        Ezekiel => "Ezekiel",
        Daniel => "Daniel",
        Hosea => "Hosea",
        Joel => "Joel",
        Amos => "Amos",
        Obadiah => "Obadiah",
        Jonah => "Jonah",
        Micah => "Micah",
        Nahum => "Nahum",
        Habakkuk => "Habakkuk",
        Zephaniah => "Zephaniah",
        Haggai => "Haggai",
        Zechariah => "Zechariah",
        Malachi => "Malachi",
        Matthew => "Matthew",
        Mark => "Mark",
        Luke => "Luke",
        John => "John",
        Acts => "Acts",
        Romans => "Romans",
        ICorinthians => "1 Corinthians",
        IICorinthians => "2 Corinthians",
        Galatians => "Galatians",
        Ephesians => "Ephesians",
        Philippians => "Philippians",
        Colossians => "Colossians",
        IThessalonians => "1 Thessalonians",
        IIThessalonians => "2 Thessalonians",
        ITimothy => "1 Timothy",
        IITimothy => "2 Timothy",
        Titus => "Titus",
        Philemon => "Philemon",
        Hebrews => "Hebrews",
        James => "James",
        IPeter => "1 Peter",
        IIPeter => "2 Peter",
        IJohn => "1 John",
        IIJohn => "2 John",
        IIIJohn => "3 John",
        Jude => "Jude",
        Revelation => "Revelation",
    }
}
