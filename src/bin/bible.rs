use bible::{Bible, BookName::Acts, ChapterReference, Edition, SearchResults, VerseReference};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let bible = Bible::new(Edition::EnglishKingJames);

    // Get a full chapter
    let chapter = bible
        .get_chapter(&ChapterReference::new(Acts, 2))
        .ok_or("Chapter not found")?;
    println!("{}", chapter);

    // Get a verse
    let verse = bible
        .get_verse(&VerseReference::new(Acts, 2, 1))
        .ok_or("Verse not found")?;
    println!("{}", verse);

    // Search - returns a vector of VerseReference(s)
    let results: SearchResults = bible.search("Jesus");
    for reference in results.references {
        println!();
        println!("{}", &reference);
        let verse = bible.get_verse(&reference).ok_or("Verse not found")?;
        println!("{}", verse);
    }

    Ok(())
}
