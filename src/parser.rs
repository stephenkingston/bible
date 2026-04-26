//! Beblia XML → `Bible`.
//!
//! Streams the document with quick-xml's event API. Books outside the
//! 66-book Protestant canon (anything `bibleref::get_bible_book_by_number`
//! rejects) are dropped with a stderr warning.

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::bible::{Bible, Book, Chapter, FORMAT_VERSION, TranslationInfo, Verse};
use crate::error::{Error, Result};
use crate::reference::book_from_number;

pub fn parse_xml(xml: &str, id: &str) -> Result<Bible> {
    let xml = xml.trim_start_matches('\u{FEFF}');
    let mut reader = Reader::from_str(xml);
    // Note: we don't enable trim_text — whitespace between elements arrives as
    // Text events that we ignore anyway (current_verse_num is None outside
    // <verse>), and trim_text's API has shifted across quick-xml versions.

    let mut translation_name = String::new();
    let mut status = String::new();

    let mut books: Vec<Book> = Vec::new();
    let mut current_book: Option<Book> = None;
    let mut current_chapter: Option<Chapter> = None;
    let mut current_verse_num: Option<u16> = None;
    let mut current_verse_text = String::new();
    let mut skip_book = false;
    let mut skipped_count: u32 = 0;

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Err(e) => {
                return Err(Error::Xml(format!(
                    "{} at byte {}",
                    e,
                    reader.buffer_position()
                )));
            }
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => match e.name().as_ref() {
                b"bible" => {
                    for attr in e.attributes().flatten() {
                        let key = attr.key.as_ref();
                        let val = attr
                            .unescape_value()
                            .map_err(|err| Error::Xml(err.to_string()))?;
                        match key {
                            b"translation" => translation_name = val.into_owned(),
                            b"status" => status = val.into_owned(),
                            _ => {}
                        }
                    }
                }
                b"book" => {
                    let n = number_attr::<u8>(&e)?;
                    if book_from_number(n).is_ok() {
                        current_book = Some(Book {
                            book_number: n,
                            chapters: Vec::new(),
                        });
                        skip_book = false;
                    } else {
                        skipped_count += 1;
                        skip_book = true;
                        current_book = None;
                    }
                }
                b"chapter" if !skip_book => {
                    let n = number_attr::<u16>(&e)?;
                    current_chapter = Some(Chapter {
                        number: n,
                        verses: Vec::new(),
                    });
                }
                b"verse" if !skip_book => {
                    let n = number_attr::<u16>(&e)?;
                    current_verse_num = Some(n);
                    current_verse_text.clear();
                }
                _ => {}
            },
            Ok(Event::Text(t)) => {
                if current_verse_num.is_some() && !skip_book {
                    let s = t.unescape().map_err(|e| Error::Xml(e.to_string()))?;
                    current_verse_text.push_str(&s);
                }
            }
            Ok(Event::CData(c)) => {
                if current_verse_num.is_some() && !skip_book {
                    let s = std::str::from_utf8(&c).map_err(|e| Error::Xml(e.to_string()))?;
                    current_verse_text.push_str(s);
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"verse" => {
                    if let Some(num) = current_verse_num.take() {
                        if let Some(ch) = current_chapter.as_mut() {
                            ch.verses.push(Verse {
                                number: num,
                                text: std::mem::take(&mut current_verse_text),
                            });
                        }
                    }
                }
                b"chapter" => {
                    if let Some(ch) = current_chapter.take() {
                        if let Some(b) = current_book.as_mut() {
                            b.chapters.push(ch);
                        }
                    }
                }
                b"book" => {
                    if let Some(b) = current_book.take() {
                        books.push(b);
                    }
                }
                _ => {}
            },
            _ => {}
        }
        buf.clear();
    }

    if skipped_count > 0 && !crate::is_quiet() {
        eprintln!(
            "bible: skipped {skipped_count} non-canonical book(s) in {id} (apocrypha not yet supported)"
        );
    }

    let language = guess_language(id);

    Ok(Bible {
        format_version: FORMAT_VERSION,
        translation: TranslationInfo {
            id: id.to_string(),
            display_name: if translation_name.is_empty() {
                friendly_name(id)
            } else {
                translation_name
            },
            language,
            status,
            installed_at: chrono::Utc::now().to_rfc3339(),
            source_sha: None,
        },
        books,
    })
}

fn number_attr<T>(e: &BytesStart<'_>) -> Result<T>
where
    T: std::str::FromStr,
    <T as std::str::FromStr>::Err: std::fmt::Display,
{
    for attr in e.attributes().flatten() {
        if attr.key.as_ref() == b"number" {
            let v = attr
                .unescape_value()
                .map_err(|e| Error::Xml(e.to_string()))?;
            return v
                .parse::<T>()
                .map_err(|e| Error::Xml(format!("number: {e}")));
        }
    }
    Err(Error::Xml("missing 'number' attribute".to_string()))
}

/// Best-effort language guess from a Beblia filename like "EnglishKJBible".
/// Takes the leading run of letters before the next uppercase + lowercase.
pub(crate) fn guess_language(id: &str) -> String {
    let stripped = id.trim_end_matches("Bible");
    let chars: Vec<char> = stripped.chars().collect();
    let mut end = chars.len();
    for i in 1..chars.len() {
        if chars[i].is_ascii_uppercase()
            && chars[i - 1].is_ascii_lowercase()
            && chars
                .get(i + 1)
                .is_none_or(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        {
            end = i;
            break;
        }
    }
    let s: String = chars[..end].iter().collect();
    if s.is_empty() {
        "Unknown".to_string()
    } else {
        s
    }
}

/// Convert "EnglishKJBible" → "English KJ" (trailing "Bible" stripped, camel-spaced).
pub(crate) fn friendly_name(id: &str) -> String {
    let stripped = id.trim_end_matches("Bible");
    let chars: Vec<char> = stripped.chars().collect();
    let mut out = String::with_capacity(stripped.len() + 4);
    for (i, &c) in chars.iter().enumerate() {
        if i > 0
            && c.is_ascii_uppercase()
            && (chars[i - 1].is_ascii_lowercase() || chars[i - 1].is_ascii_digit())
        {
            out.push(' ');
        }
        out.push(c);
    }
    out
}
