#[cfg(test)]
mod tests {
    use bible::{Bible, ChapterReference, VerseReference};

    #[test]
    fn get_chapter() {
        let bible = Bible::new(bible::Edition::EnglishKingJames);
        assert!(
            bible
                .get_chapter(ChapterReference::new(bible::BookName::Acts, 2))
                .is_some()
        );
    }

    #[test]
    fn get_verse() {
        let bible = Bible::new(bible::Edition::EnglishKingJames);
        assert!(
            bible
                .get_verse(VerseReference::new(bible::BookName::Acts, 2, 1))
                .is_some()
        );
    }

    #[test]
    fn search() {
        let bible = Bible::new(bible::Edition::EnglishKingJames);
        let results = bible.search("Jesus");

        assert!(results.references.len() > 0);
    }
}
