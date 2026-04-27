//! `bible` — a TUI Bible reader with on-demand translation downloads.
//!
//! - Canonical reference handling is delegated to the [`bibleref`] crate.
//! - Translation files are fetched on demand from the
//!   [Beblia Holy-Bible-XML-Format repo](https://github.com/Beblia/Holy-Bible-XML-Format)
//!   and stored under the user's data directory.
//! - The binary `bible` provides a `ratatui` + `crossterm` TUI by default and
//!   headless CLI subcommands when stdout is not a TTY.
//!
//! # Quick example
//!
//! ```no_run
//! use bible::{Bible, reference};
//!
//! # fn main() -> bible::Result<()> {
//! let bible = Bible::load("EnglishKJBible")?;
//! let parsed = reference::parse("John 3:16")?;
//! # Ok(())
//! # }
//! ```

pub mod bible;
pub mod bookmarks;
pub mod cli;
pub mod download;
pub mod error;
pub mod manifest;
pub mod parser;
pub mod reference;
pub mod search;
pub mod settings;
pub mod state;
pub mod storage;
pub mod tui;

pub use crate::bible::{Bible, Book, Chapter, TranslationInfo, Verse};
pub use crate::error::{Error, Result};
pub use crate::reference::{
    BibleBook, BibleChapterReference, BibleReference, BibleReferenceRepresentation,
    BibleVerseReference,
};
pub use crate::search::SearchHit;

use std::sync::atomic::{AtomicBool, Ordering};

static QUIET: AtomicBool = AtomicBool::new(false);

/// Silence library-side `eprintln!` warnings (used by the TUI so that stderr
/// writes don't corrupt the alternate-screen display).
pub fn set_quiet(v: bool) {
    QUIET.store(v, Ordering::Relaxed);
}

pub(crate) fn is_quiet() -> bool {
    QUIET.load(Ordering::Relaxed)
}
