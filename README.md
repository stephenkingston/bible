# Bible

Use the Bible in Rust!

## Examples

```rust
use bible::{
    Bible, ChapterReference, Edition,
    VerseReference,
    BookName::Acts
};

fn main() {
    let bible = Bible::new(Edition::EnglishKingJames);

    // Get a full chapter
    println!(
        "{:?}",
        bible.get_chapter(ChapterReference::new(Acts, 2))
    );

    // Get a verse
    println!(
        "{:?}",
        bible.get_verse(VerseReference::new(Acts, 2, 1))
    );

    // Search - returns a vector of VerseReference(s)
    let results: SearchResults = bible.search("Jesus");
    for reference in results.references {
        println!("");
        println!("{:?}", reference);
        println!("{:?}", bible.get_verse(reference).unwrap());
    }
}

```
