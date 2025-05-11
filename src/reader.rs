use crate::{Bible, Edition};

#[cfg(feature = "english-kjv")]
const ENGLISH_KJV: &[u8] = include_bytes!("../src/bibles/EnglishKJBible.bin");

#[cfg(feature = "english-asv")]
const ENGLISH_ASV: &[u8] = include_bytes!("../src/bibles/EnglishASVBible.bin");

pub(crate) fn read_bible(edition: Edition) -> Bible {
    match edition {
        #[cfg(feature = "english-kjv")]
        Edition::EnglishKingJames => {
            let (bible, _): (Bible, usize) =
                bincode::decode_from_slice(&ENGLISH_KJV, bincode::config::standard()).unwrap();
            bible
        }
        #[cfg(feature = "english-asv")]
        Edition::EnglishASV => {
            let (bible, _): (Bible, usize) =
                bincode::decode_from_slice(&ENGLISH_ASV, bincode::config::standard()).unwrap();
            bible
        }
        _ => {
            panic!("Requested Bible edition was not enabled as a feature, please do so")
        }
    }
}
