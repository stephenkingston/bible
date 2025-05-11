use quick_xml::events::Event;
use quick_xml::reader::Reader;

use crate::{Bible, EnglishVersion, Language};

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

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                current_tag = String::from_utf8(e.name().0.to_vec()).unwrap();
            }

            Ok(Event::Text(ref e)) => {
                if !current_tag.is_empty() {
                    let text = e.unescape().unwrap().into_owned();
                    println!("{}: {}", current_tag, text);
                }
            }

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
