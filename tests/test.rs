use bible::Bible;
use bible::parser;
use bible::reference::{
    BibleBook, BibleChapterReference, BibleReference, BibleReferenceRepresentation,
    BibleVerseReference,
};

fn fixture_xml() -> String {
    std::fs::read_to_string("tests/fixtures/MiniBible.xml").expect("fixture present")
}

fn fixture_bible() -> Bible {
    parser::parse_xml(&fixture_xml(), "MiniBible").expect("parses")
}

#[test]
fn parses_fixture_metadata() {
    let bible = fixture_bible();
    assert_eq!(bible.translation.id, "MiniBible");
    assert_eq!(bible.translation.display_name, "Test KJV");
    assert_eq!(bible.translation.status, "Public Domain");
    assert!(bible.books.iter().any(|b| b.book_number == 1)); // Genesis
    assert!(bible.books.iter().any(|b| b.book_number == 43)); // John
}

#[test]
fn round_trips_through_bincode() {
    let bible = fixture_bible();
    let bin = bincode::serde::encode_to_vec(&bible, bincode::config::standard()).unwrap();
    let (back, _): (Bible, _) =
        bincode::serde::decode_from_slice(&bin, bincode::config::standard()).unwrap();
    assert_eq!(back.books.len(), bible.books.len());
    assert_eq!(back.translation.id, bible.translation.id);
}

#[test]
fn looks_up_known_verse() {
    let bible = fixture_bible();
    let vr = BibleVerseReference::new(BibleBook::John, 3, 16).expect("valid verse");
    let v = bible.get_verse(&vr).expect("present");
    assert!(v.text.contains("only begotten"));
}

#[test]
fn looks_up_known_chapter() {
    let bible = fixture_bible();
    let cr = BibleChapterReference::new(BibleBook::Genesis, 1).expect("valid chapter");
    let ch = bible.get_chapter(&cr).expect("present");
    assert_eq!(ch.verses.len(), 3);
    assert!(ch.verses[0].text.contains("In the beginning"));
}

#[test]
fn parses_reference_via_bibleref() {
    let parsed = bible::reference::parse("John 3:16").expect("parses");
    match parsed {
        BibleReferenceRepresentation::Single(BibleReference::BibleVerse(vr)) => {
            assert_eq!(vr.book().number(), 43);
            let chap: u32 = vr.chapter().into();
            assert_eq!(chap, 3);
            let v: u32 = vr.verse().into();
            assert_eq!(v, 16);
        }
        _ => panic!("expected single verse"),
    }
}

#[test]
fn case_insensitive_search_matches() {
    let bible = fixture_bible();
    let hits = bible.search_substring("LOVED");
    assert!(!hits.is_empty(), "expected at least one match for `LOVED`");
    assert!(
        hits.iter()
            .any(|h| h.reference.book().number() == 43 && h.text.contains("loved"))
    );
}

#[test]
fn empty_search_returns_no_hits() {
    let bible = fixture_bible();
    assert!(bible.search_substring("").is_empty());
}
