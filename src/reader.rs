use quick_xml::events::Event;
use quick_xml::reader::Reader;

use crate::{Bible, Book, Chapter, EnglishVersion, Language};

const ASV_BIBLE: &str = include_str!("./bibles/english/EnglishASVBible.xml");
const KJV_BIBLE: &str = include_str!("./bibles/english/EnglishKJBible.xml");

pub(crate) fn read_bible(language: Language) -> Bible {
    let mut reader = match &language {
        Language::English(version) => match version {
            EnglishVersion::KingJames => Reader::from_str(KJV_BIBLE),
            EnglishVersion::AmericanStandard => Reader::from_str(ASV_BIBLE),
        },
    };

    let mut buf = Vec::new();
    let mut current_tag = String::new();

    let mut bible = Bible {
        books: Vec::new(),
        language: language.clone(),
    };
    let mut current_book = Some(Book {
        name: String::new(),
        chapters: Vec::new(),
    });

    let mut current_chapter = Chapter {
        number: 0,
        verses: Vec::new(),
    };

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                current_tag = String::from_utf8(e.name().0.to_vec()).unwrap();
                if current_tag == "book" {
                    let attr = e.attributes();
                    for a in attr {
                        let attr = a.unwrap();
                        let value = attr.value;
                        let name = attr.key;

                        println!("{:?} {:?}", value.to_ascii_lowercase(), name);
                        // Set book
                    }
                }
            }

            Ok(Event::Text(ref e)) => {
                if !current_tag.is_empty() {
                    let text = e.unescape().unwrap().into_owned();
                    println!("{}: {}", current_tag, text);
                }
            }

            Ok(Event::End(ref e)) => {}

            Ok(Event::Eof) => break,

            Err(e) => panic!("Error at position {}: {:?}", reader.buffer_position(), e),

            _ => {}
        }

        buf.clear();
    }

    Bible {
        books: Vec::new(),
        language,
    }
}
