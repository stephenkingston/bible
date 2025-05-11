#![doc = include_str!("../README.md")]

pub(crate) mod reader;
pub mod util;

use crate::reader::read_bible;
use bincode::{Decode, Encode};
use std::fmt::{self, Display};

#[derive(Default, Encode, Decode)]
pub struct Bible {
    pub books: Vec<Book>,
    pub edition: Edition,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VerseReference {
    pub book_name: BookName,
    pub chapter_number: u32,
    pub verse_number: u32,
}

impl Display for VerseReference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {}:{}",
            self.book_name, self.chapter_number, self.verse_number
        )
    }
}

impl VerseReference {
    pub fn new(book_name: BookName, chapter_number: u32, verse_number: u32) -> VerseReference {
        VerseReference {
            book_name,
            chapter_number,
            verse_number,
        }
    }

    pub fn get_chapter_reference(&self) -> ChapterReference {
        ChapterReference::new(self.book_name, self.chapter_number)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChapterReference {
    pub book_name: BookName,
    pub chapter_number: u32,
}

impl ChapterReference {
    pub fn new(book_name: BookName, chapter_number: u32) -> ChapterReference {
        ChapterReference {
            book_name,
            chapter_number,
        }
    }
}

pub struct SearchResults {
    pub references: Vec<VerseReference>,
}

impl Display for SearchResults {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for reference in &self.references {
            write!(f, "{:?}", reference)?;
        }
        Ok(())
    }
}

impl Bible {
    pub fn get_chapter(&self, reference: &ChapterReference) -> Option<&Chapter> {
        self.books
            .iter()
            .find(|book| book.name == reference.book_name)
            .and_then(|book| {
                book.chapters
                    .iter()
                    .find(|chapter| chapter.number == reference.chapter_number)
            })
    }

    pub fn get_verse(&self, reference: &VerseReference) -> Option<&Verse> {
        self.get_chapter(&reference.get_chapter_reference())
            .and_then(|chapter| {
                chapter
                    .verses
                    .iter()
                    .find(|verse| verse.number == reference.verse_number)
            })
    }

    pub fn search(&self, query: &str) -> SearchResults {
        let mut results = SearchResults {
            references: Vec::new(),
        };

        for book in &self.books {
            for chapter in &book.chapters {
                for verse in &chapter.verses {
                    if verse.text.contains(query) {
                        results.references.push(VerseReference::new(
                            book.name,
                            chapter.number,
                            verse.number,
                        ));
                    }
                }
            }
        }

        results
    }
}

impl Bible {
    pub fn new(edition: Edition) -> Bible {
        read_bible(edition)
    }
}

#[derive(Clone, Encode, Decode)]
pub enum Edition {
    EnglishKingJames,
    EnglishASV,
}

impl Default for Edition {
    fn default() -> Self {
        Edition::EnglishKingJames
    }
}
#[derive(Encode, Decode)]
pub struct Book {
    pub name: BookName,
    pub chapters: Vec<Chapter>,
}

#[derive(Encode, Decode, Debug)]
pub struct Chapter {
    pub number: u32,
    pub verses: Vec<Verse>,
}

impl Display for Chapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Chapter {}", self.number)?;
        for verse in &self.verses {
            write!(f, "\n\t{}", verse)?;
        }
        Ok(())
    }
}

#[derive(Encode, Decode, Debug)]
pub struct Verse {
    pub number: u32,
    pub text: String,
}

impl Display for Verse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.text)
    }
}

/// Enum representing the books of the Bible in canonical order (Old and New Testament).
/// Each variant corresponds to a book, with explicit indices starting from 1 for Genesis.
#[derive(Debug, Encode, Decode, PartialEq, Clone, Copy)]
pub enum BookName {
    Genesis = 1,
    Exodus,
    Leviticus,
    Numbers,
    Deuteronomy,
    Joshua,
    Judges,
    Ruth,
    FirstSamuel,
    SecondSamuel,
    FirstKings,
    SecondKings,
    FirstChronicles,
    SecondChronicles,
    Ezra,
    Nehemiah,
    Esther,
    Job,
    Psalms,
    Proverbs,
    Ecclesiastes,
    SongOfSolomon,
    Isaiah,
    Jeremiah,
    Lamentations,
    Ezekiel,
    Daniel,
    Hosea,
    Joel,
    Amos,
    Obadiah,
    Jonah,
    Micah,
    Nahum,
    Habakkuk,
    Zephaniah,
    Haggai,
    Zechariah,
    Malachi,
    // New Testament
    Matthew,
    Mark,
    Luke,
    John,
    Acts,
    Romans,
    FirstCorinthians,
    SecondCorinthians,
    Galatians,
    Ephesians,
    Philippians,
    Colossians,
    FirstThessalonians,
    SecondThessalonians,
    FirstTimothy,
    SecondTimothy,
    Titus,
    Philemon,
    Hebrews,
    James,
    FirstPeter,
    SecondPeter,
    FirstJohn,
    SecondJohn,
    ThirdJohn,
    Jude,
    Revelation,
}

impl fmt::Display for BookName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let book_str = match self {
            BookName::Genesis => "Genesis",
            BookName::Exodus => "Exodus",
            BookName::Leviticus => "Leviticus",
            BookName::Numbers => "Numbers",
            BookName::Deuteronomy => "Deuteronomy",
            BookName::Joshua => "Joshua",
            BookName::Judges => "Judges",
            BookName::Ruth => "Ruth",
            BookName::FirstSamuel => "I Samuel",
            BookName::SecondSamuel => "II Samuel",
            BookName::FirstKings => "I Kings",
            BookName::SecondKings => "II Kings",
            BookName::FirstChronicles => "I Chronicles",
            BookName::SecondChronicles => "II Chronicles",
            BookName::Ezra => "Ezra",
            BookName::Nehemiah => "Nehemiah",
            BookName::Esther => "Esther",
            BookName::Job => "Job",
            BookName::Psalms => "Psalms",
            BookName::Proverbs => "Proverbs",
            BookName::Ecclesiastes => "Ecclesiastes",
            BookName::SongOfSolomon => "Song of Solomon",
            BookName::Isaiah => "Isaiah",
            BookName::Jeremiah => "Jeremiah",
            BookName::Lamentations => "Lamentations",
            BookName::Ezekiel => "Ezekiel",
            BookName::Daniel => "Daniel",
            BookName::Hosea => "Hosea",
            BookName::Joel => "Joel",
            BookName::Amos => "Amos",
            BookName::Obadiah => "Obadiah",
            BookName::Jonah => "Jonah",
            BookName::Micah => "Micah",
            BookName::Nahum => "Nahum",
            BookName::Habakkuk => "Habakkuk",
            BookName::Zephaniah => "Zephaniah",
            BookName::Haggai => "Haggai",
            BookName::Zechariah => "Zechariah",
            BookName::Malachi => "Malachi",
            BookName::Matthew => "Matthew",
            BookName::Mark => "Mark",
            BookName::Luke => "Luke",
            BookName::John => "John",
            BookName::Acts => "Acts",
            BookName::Romans => "Romans",
            BookName::FirstCorinthians => "I Corinthians",
            BookName::SecondCorinthians => "II Corinthians",
            BookName::Galatians => "Galatians",
            BookName::Ephesians => "Ephesians",
            BookName::Philippians => "Philippians",
            BookName::Colossians => "Colossians",
            BookName::FirstThessalonians => "I Thessalonians",
            BookName::SecondThessalonians => "II Thessalonians",
            BookName::FirstTimothy => "I Timothy",
            BookName::SecondTimothy => "II Timothy",
            BookName::Titus => "Titus",
            BookName::Philemon => "Philemon",
            BookName::Hebrews => "Hebrews",
            BookName::James => "James",
            BookName::FirstPeter => "I Peter",
            BookName::SecondPeter => "II Peter",
            BookName::FirstJohn => "I John",
            BookName::SecondJohn => "II John",
            BookName::ThirdJohn => "III John",
            BookName::Jude => "Jude",
            BookName::Revelation => "Revelation",
        };
        write!(f, "{}", book_str)
    }
}
