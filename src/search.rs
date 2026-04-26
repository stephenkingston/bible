use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

use crate::bible::Bible;
use crate::reference::{BibleVerseReference, book_from_number};

#[derive(Debug, Clone)]
pub struct SearchHit {
    pub reference: BibleVerseReference,
    pub text: String,
}

impl Bible {
    /// Case-insensitive, diacritic-folded substring search.
    /// O(N) over all verses — fine for ~31k verses (single-digit ms).
    pub fn search_substring(&self, query: &str) -> Vec<SearchHit> {
        let q = normalize(query);
        if q.is_empty() {
            return Vec::new();
        }
        let mut hits = Vec::new();
        for book in &self.books {
            let Ok(book_enum) = book_from_number(book.book_number) else {
                continue;
            };
            for chapter in &book.chapters {
                for verse in &chapter.verses {
                    if !normalize(&verse.text).contains(&q) {
                        continue;
                    }
                    let Ok(chap) = u8::try_from(chapter.number) else {
                        continue;
                    };
                    let Ok(vnum) = u8::try_from(verse.number) else {
                        continue;
                    };
                    if let Ok(r) = BibleVerseReference::new(book_enum.clone(), chap, vnum) {
                        hits.push(SearchHit {
                            reference: r,
                            text: verse.text.clone(),
                        });
                    }
                }
            }
        }
        hits
    }
}

fn normalize(s: &str) -> String {
    s.nfkd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(|c| c.to_lowercase())
        .collect()
}
