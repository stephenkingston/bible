use crate::reference::{
    BibleChapterReference, BibleReference, BibleReferenceRepresentation,
    get_bible_book_by_number, get_number_of_chapters,
};

pub fn shift_chapter(r: &BibleChapterReference, dir: i32) -> Option<BibleChapterReference> {
    let book = r.book();
    let cur: u32 = r.chapter().into();
    let total: u32 = get_number_of_chapters(&book).into();

    if dir > 0 {
        if cur < total {
            BibleChapterReference::new(book, ((cur + 1) as u8).min(255)).ok()
        } else {
            let next = book.number().checked_add(1)?;
            let nb = get_bible_book_by_number(next)?;
            BibleChapterReference::new(nb, 1).ok()
        }
    } else if dir < 0 {
        if cur > 1 {
            BibleChapterReference::new(book, (cur - 1) as u8).ok()
        } else {
            let prev = book.number().checked_sub(1)?;
            if prev == 0 {
                return None;
            }
            let pb = get_bible_book_by_number(prev)?;
            let chs: u32 = get_number_of_chapters(&pb).into();
            BibleChapterReference::new(pb, chs as u8).ok()
        }
    } else {
        Some(r.clone())
    }
}

pub fn shift_book(r: &BibleChapterReference, dir: i32) -> Option<BibleChapterReference> {
    let target = if dir > 0 {
        r.book().number().checked_add(1)?
    } else if dir < 0 {
        let p = r.book().number().checked_sub(1)?;
        if p == 0 {
            return None;
        }
        p
    } else {
        r.book().number()
    };
    let nb = get_bible_book_by_number(target)?;
    BibleChapterReference::new(nb, 1).ok()
}

/// Re-parse a query and return its first chapter when it resolved to a book-only ref.
pub fn first_chapter_of_parsed(q: &str) -> crate::error::Result<BibleChapterReference> {
    use crate::error::Error;
    let parsed = crate::reference::parse(q)?;
    let single = match parsed {
        BibleReferenceRepresentation::Single(s) => s,
        BibleReferenceRepresentation::Range(_) => {
            return Err(Error::Reference("range not supported".into()));
        }
    };
    let book = match single {
        BibleReference::BibleBook(_) => {
            // BibleBookReference has no public getter, so re-parse with an
            // appended " 1" to coerce the result into a BibleChapterReference.
            let with_chapter = format!("{q} 1");
            return first_chapter_of_parsed(&with_chapter);
        }
        BibleReference::BibleChapter(cr) => return Ok(cr),
        BibleReference::BibleVerse(vr) => vr.book(),
    };
    BibleChapterReference::new(book, 1)
        .map_err(|e| crate::error::Error::Reference(format!("{e:?}")))
}
