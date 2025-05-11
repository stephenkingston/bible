pub(crate) mod reader;
pub(crate) mod util;

use crate::reader::read_bible;

pub struct Bible {
    pub books: Vec<Book>,
    pub language: Language,
}

impl Bible {
    pub fn from(language: Language) -> Bible {
        read_bible(language)
    }
}

pub enum EnglishVersion {
    KingJames,
    AmericanStandard,
}

pub enum Language {
    English(EnglishVersion),
}

pub struct Book {
    pub name: String,
    pub chapters: Vec<Chapter>,
}

pub struct Chapter {
    pub number: u32,
    pub verses: Vec<Verse>,
}

pub struct Verse {
    pub number: u32,
    pub text: String,
}
