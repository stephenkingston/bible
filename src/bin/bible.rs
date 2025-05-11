use bible::{Bible, ChapterReference, Edition, SearchResults, VerseReference};

fn main() {
    let bible = Bible::new(Edition::EnglishKingJames);

    println!(
        "{:?}",
        bible.get_chapter(ChapterReference::new(bible::BookName::Acts, 2))
    );
    println!(
        "{:?}",
        bible.get_verse(VerseReference::new(bible::BookName::Acts, 2, 1))
    );

    let results: SearchResults = bible.search("Jesus");
    for reference in results.references {
        println!("");
        println!("{:?}", reference);
        println!("{:?}", bible.get_verse(reference).unwrap());
    }
}
