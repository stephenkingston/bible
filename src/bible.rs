use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::reference::{BibleBook, BibleChapterReference, BibleVerseReference, book_from_number};

pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationInfo {
    pub id: String,
    pub display_name: String,
    pub language: String,
    pub status: String,
    pub installed_at: String,
    pub source_sha: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Verse {
    pub number: u16,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chapter {
    pub number: u16,
    pub verses: Vec<Verse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Book {
    /// 1..=66, the canonical book number used by `bibleref::get_bible_book_by_number`.
    pub book_number: u8,
    pub chapters: Vec<Chapter>,
}

impl Book {
    pub fn book(&self) -> Result<BibleBook> {
        book_from_number(self.book_number)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bible {
    pub format_version: u32,
    pub translation: TranslationInfo,
    pub books: Vec<Book>,
}

impl Bible {
    pub fn new(translation: TranslationInfo, books: Vec<Book>) -> Self {
        Self {
            format_version: FORMAT_VERSION,
            translation,
            books,
        }
    }

    pub fn load(id: &str) -> Result<Self> {
        crate::storage::load_bible(id)
    }

    pub fn get_chapter(&self, r: &BibleChapterReference) -> Option<&Chapter> {
        let n = r.book().number();
        let target = u32::from(r.chapter());
        self.books
            .iter()
            .find(|b| b.book_number == n)?
            .chapters
            .iter()
            .find(|c| u32::from(c.number) == target)
    }

    pub fn get_verse(&self, r: &BibleVerseReference) -> Option<&Verse> {
        let n = r.book().number();
        let cnum = u32::from(r.chapter());
        let vnum = u32::from(r.verse());
        self.books
            .iter()
            .find(|b| b.book_number == n)?
            .chapters
            .iter()
            .find(|c| u32::from(c.number) == cnum)?
            .verses
            .iter()
            .find(|v| u32::from(v.number) == vnum)
    }
}
