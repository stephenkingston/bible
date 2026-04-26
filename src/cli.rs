//! Command-line dispatch.
//!
//! With no subcommand and a TTY on stdout, launches the TUI.
//! With no subcommand and a non-TTY (piped) stdout, prints `--help`.
//! Subcommands always run headlessly.

use std::io::IsTerminal;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};

use crate::bible::Bible;
use crate::manifest;
use crate::reference::{BibleReference, BibleReferenceRepresentation, book_display};
use crate::storage;

#[derive(Parser)]
#[command(name = "bible", version, about = "A beautiful TUI Bible reader")]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Translation id or alias (e.g. `kjv`, `EnglishKJBible`)
    #[arg(short, long, global = true)]
    translation: Option<String>,
}

#[derive(Subcommand)]
enum Command {
    /// Print a verse or chapter, e.g. `bible read "John 3:16"`
    Read { reference: String },
    /// Search the active translation for a substring
    Search { query: String },
    /// List installed translations (or `--available` for downloadable)
    List {
        #[arg(long)]
        available: bool,
    },
    /// Install a translation from Beblia
    Install { id_or_alias: String },
    /// Uninstall a translation
    Uninstall { id_or_alias: String },
    /// Refresh the translation catalog from GitHub
    Refresh,
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Read { reference }) => cmd_read(cli.translation.as_deref(), &reference),
        Some(Command::Search { query }) => cmd_search(cli.translation.as_deref(), &query),
        Some(Command::List { available }) => cmd_list(available),
        Some(Command::Install { id_or_alias }) => cmd_install(&id_or_alias),
        Some(Command::Uninstall { id_or_alias }) => cmd_uninstall(&id_or_alias),
        Some(Command::Refresh) => cmd_refresh(),
        None => {
            if std::io::stdout().is_terminal() {
                crate::tui::run(cli.translation).map_err(Into::into)
            } else {
                use clap::CommandFactory;
                Cli::command().print_help()?;
                println!();
                Ok(())
            }
        }
    }
}

fn pick_translation(explicit: Option<&str>) -> Result<String> {
    if let Some(s) = explicit {
        return Ok(manifest::resolve_id(s)?);
    }
    let installed = storage::list_installed().unwrap_or_default();
    match installed.len() {
        0 => bail!("no translations installed — run `bible install kjv`"),
        1 => Ok(installed[0].id.clone()),
        _ => bail!(
            "multiple translations installed; pass --translation <id>. installed: {}",
            installed
                .iter()
                .map(|t| t.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn cmd_read(translation: Option<&str>, reference: &str) -> Result<()> {
    let id = pick_translation(translation)?;
    let bible = Bible::load(&id).with_context(|| format!("loading {id}"))?;
    let parsed = crate::reference::parse(reference)?;
    match parsed {
        BibleReferenceRepresentation::Single(r) => print_single(&bible, &r),
        BibleReferenceRepresentation::Range(_) => {
            bail!("ranges aren't supported yet — try a single chapter or verse")
        }
    }
}

fn print_single(bible: &Bible, r: &BibleReference) -> Result<()> {
    match r {
        BibleReference::BibleVerse(vr) => {
            let v = bible
                .get_verse(vr)
                .with_context(|| format!("verse not found in {}", bible.translation.id))?;
            println!(
                "{} {}:{}\n{}",
                book_display(&vr.book()),
                vr.chapter(),
                vr.verse(),
                v.text
            );
        }
        BibleReference::BibleChapter(cr) => {
            let ch = bible
                .get_chapter(cr)
                .with_context(|| format!("chapter not found in {}", bible.translation.id))?;
            println!("{} {}", book_display(&cr.book()), cr.chapter());
            for v in &ch.verses {
                println!("{:>3} {}", v.number, v.text);
            }
        }
        BibleReference::BibleBook(_br) => {
            bail!("printing whole books isn't supported — try a chapter");
        }
    }
    Ok(())
}

fn cmd_search(translation: Option<&str>, query: &str) -> Result<()> {
    let id = pick_translation(translation)?;
    let bible = Bible::load(&id)?;
    let hits = bible.search_substring(query);
    if hits.is_empty() {
        println!("no matches");
        return Ok(());
    }
    for hit in &hits {
        let r = &hit.reference;
        println!(
            "{} {}:{}  {}",
            book_display(&r.book()),
            r.chapter(),
            r.verse(),
            hit.text
        );
    }
    eprintln!("\n{} match(es)", hits.len());
    Ok(())
}

fn cmd_list(available: bool) -> Result<()> {
    if available {
        let m = manifest::list_available();
        if m.is_empty() {
            println!("no available translations cached — try `bible refresh`");
            return Ok(());
        }
        for t in m {
            println!("{:30}  {}  ({})", t.id, t.display_name, t.language);
        }
    } else {
        let installed = storage::list_installed()?;
        if installed.is_empty() {
            println!("no translations installed — run `bible install kjv`");
            return Ok(());
        }
        for t in installed {
            println!("{:30}  {}  ({})", t.id, t.display_name, t.language);
        }
    }
    Ok(())
}

fn cmd_install(input: &str) -> Result<()> {
    let id = manifest::resolve_id(input)?;
    if storage::is_installed(&id) {
        println!("already installed: {id}");
        return Ok(());
    }
    eprintln!("installing {id} from Beblia…");
    let mut last_pct: u8 = 255;
    let mut progress = |bytes: u64, total: Option<u64>| {
        if let Some(total) = total {
            if total > 0 {
                let pct = ((bytes * 100) / total).min(100) as u8;
                if pct != last_pct && pct % 5 == 0 {
                    eprint!("\r  {pct:>3}% ({}/{} KiB)", bytes / 1024, total / 1024);
                    last_pct = pct;
                }
            }
        }
    };
    let bible = crate::download::install(&id, Some(&mut progress))?;
    eprintln!();
    println!(
        "installed: {} ({} books, {})",
        bible.translation.id,
        bible.books.len(),
        bible.translation.display_name
    );
    Ok(())
}

fn cmd_uninstall(input: &str) -> Result<()> {
    let id = manifest::resolve_id(input)?;
    if !storage::is_installed(&id) {
        println!("not installed: {id}");
        return Ok(());
    }
    storage::uninstall(&id)?;
    println!("uninstalled: {id}");
    Ok(())
}

fn cmd_refresh() -> Result<()> {
    eprintln!("refreshing manifest from GitHub…");
    let m = manifest::refresh()?;
    println!(
        "{} translation(s) cached (tree {})",
        m.translations.len(),
        m.tree_sha.as_deref().unwrap_or("?")
    );
    Ok(())
}

