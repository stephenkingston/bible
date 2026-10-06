//! `bible` — a TUI Bible reader with on-demand translation downloads.
//!
//! ![The bible reader in its themes, with the keys card and command palette](https://raw.githubusercontent.com/stephenkingston/bible/main/screenshots/screencast.gif)
//!
//! - Canonical reference handling is delegated to the
//!   [`bibleref`](https://docs.rs/bibleref) crate.
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
//!
//! # Screenshots
//!
//! The `bible` binary (`cargo install bible`) opens in Vellum, shown above:
//! an illuminated manuscript, set as running prose. The other themes include
//! Nocturne, Gilt and Daylight:
//!
//! ![John 1 in the Nocturne theme](https://raw.githubusercontent.com/stephenkingston/bible/main/screenshots/nocturne.png)
//!
//! ![John 1 in the Gilt theme](https://raw.githubusercontent.com/stephenkingston/bible/main/screenshots/gilt.png)
//!
//! ![John 1 in the Daylight theme, in two columns](https://raw.githubusercontent.com/stephenkingston/bible/main/screenshots/daylight.png)
//!
//! The command palette (`Ctrl-K`) and the keys card (`?`):
//!
//! ![The command palette](https://raw.githubusercontent.com/stephenkingston/bible/main/screenshots/palette.png)
//!
//! ![The keys card](https://raw.githubusercontent.com/stephenkingston/bible/main/screenshots/help.png)

pub mod bible;
pub mod bookmarks;
pub mod cli;
pub mod download;
pub mod error;
pub mod manifest;
pub mod parser;
pub mod plan;
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
