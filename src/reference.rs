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
pub fn parse(s: &str) -> Result<BibleReferenceRepresentation> {
    bibleref::parse(s).map_err(|e| Error::Reference(e.to_string()))
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
