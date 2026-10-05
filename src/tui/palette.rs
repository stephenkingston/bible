//! Ctrl-K command palette: one input that takes a reference ("ps 23"), a
//! search, or any command, theme, layout or installed translation.

use crate::bible::TranslationInfo;
use crate::reference::{BibleReference, BibleReferenceRepresentation, book_display};
use crate::settings::{ColumnLayout, ReadingLayout, ThemePreset};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Cmd {
    GoTo,
    Search,
    PickBook,
    Today,
    Parallel,
    SwapParallel,
    NextTranslation,
    Translations,
    InstallKjv,
    Bookmark,
    Bookmarks,
    CopyVerse,
    Plan,
    Back,
    Forward,
    ClearSearch,
    Settings,
    Keys,
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Action {
    Jump(String),
    Search(String),
    Run(Cmd),
    Theme(ThemePreset),
    Layout(ReadingLayout),
    Columns(ColumnLayout),
    Translation(String),
}

#[derive(Debug, Clone)]
pub(crate) struct Item {
    pub label: String,
    /// Key that does the same thing outside the palette, or a short note.
    pub hint: String,
    pub action: Action,
}

const COMMANDS: &[(Cmd, &str, &str)] = &[
    (Cmd::GoTo, "Go to reference…", ":"),
    (Cmd::Search, "Search…", "/"),
    (Cmd::PickBook, "Pick a book", "g"),
    (Cmd::Today, "Today's reading", "p"),
    (Cmd::Parallel, "Toggle parallel view", "|"),
    (Cmd::SwapParallel, "Choose parallel translation", "\\"),
    (Cmd::NextTranslation, "Next translation", "t"),
    (Cmd::Translations, "Manage translations", "T"),
    (Cmd::InstallKjv, "Install the King James Version", "i"),
    (Cmd::Bookmark, "Bookmark this chapter", "b"),
    (Cmd::Bookmarks, "Bookmarks", "B"),
    (Cmd::CopyVerse, "Copy verse", "y"),
    (Cmd::Plan, "Reading plan", "P"),
    (Cmd::Back, "Back", "^O"),
    (Cmd::Forward, "Forward", "Tab"),
    (Cmd::ClearSearch, "Clear search highlight", "Esc"),
    (Cmd::Settings, "Settings", ","),
    (Cmd::Keys, "Keys", "?"),
    (Cmd::Quit, "Quit", "q"),
];

pub(crate) struct Context<'a> {
    pub installed: &'a [TranslationInfo],
    pub current_translation: Option<&'a str>,
    pub theme: ThemePreset,
    pub layout: ReadingLayout,
    pub columns: ColumnLayout,
}

/// Everything the palette can do, filtered and ranked for `query`.
pub(crate) fn items(query: &str, ctx: &Context) -> Vec<Item> {
    let all = catalogue(ctx);
    let q = query.trim();
    if q.is_empty() {
        return all;
    }
    let needle = q.to_lowercase();
    let mut scored: Vec<(u8, usize, Item)> = all
        .into_iter()
        .enumerate()
        .filter_map(|(i, it)| score(&it.label.to_lowercase(), &needle).map(|s| (s, i, it)))
        .collect();
    scored.sort_by_key(|(s, i, _)| (*s, *i));

    let mut out = Vec::new();
    if let Some(label) = reference_label(q) {
        out.push(Item { label: format!("Go to {label}"), hint: "enter".into(), action: Action::Jump(q.to_string()) });
    }
    // Commands whose name starts with what was typed beat the search
    // fallback; looser matches come after it.
    let (strong, weak): (Vec<_>, Vec<_>) = scored.into_iter().partition(|(s, _, _)| *s <= 1);
    out.extend(strong.into_iter().map(|(_, _, it)| it));
    out.push(Item {
        label: format!("Search “{q}”"),
        hint: "/".into(),
        action: Action::Search(q.to_string()),
    });
    out.extend(weak.into_iter().map(|(_, _, it)| it));
    out
}

fn catalogue(ctx: &Context) -> Vec<Item> {
    let has_bible = ctx.current_translation.is_some();
    let mut all: Vec<Item> = COMMANDS
        .iter()
        .filter(|(cmd, _, _)| *cmd != Cmd::InstallKjv || ctx.installed.is_empty())
        .filter(|(cmd, _, _)| has_bible || matches!(cmd, Cmd::InstallKjv | Cmd::Translations | Cmd::Settings | Cmd::Keys | Cmd::Quit))
        .map(|(cmd, label, hint)| Item { label: label.to_string(), hint: hint.to_string(), action: Action::Run(*cmd) })
        .collect();
    for t in ctx.installed {
        if Some(t.id.as_str()) != ctx.current_translation {
            all.push(Item {
                label: format!("Read in {}", t.display_name),
                hint: String::new(),
                action: Action::Translation(t.id.clone()),
            });
        }
    }
    for p in ThemePreset::ALL {
        all.push(Item {
            label: format!("Theme: {}", p.label()),
            hint: if p == ctx.theme { "current".into() } else { String::new() },
            action: Action::Theme(p),
        });
    }
    for (l, name) in [
        (ReadingLayout::Verses, "verse per line"),
        (ReadingLayout::Prose, "prose paragraphs"),
        (ReadingLayout::Auto, "theme default"),
    ] {
        all.push(Item {
            label: format!("Layout: {name}"),
            hint: if l == ctx.layout { "current".into() } else { String::new() },
            action: Action::Layout(l),
        });
    }
    for (c, name) in [
        (ColumnLayout::One, "one"),
        (ColumnLayout::Two, "two"),
        (ColumnLayout::Auto, "auto"),
    ] {
        all.push(Item {
            label: format!("Columns: {name}"),
            hint: if c == ctx.columns { "current".into() } else { String::new() },
            action: Action::Columns(c),
        });
    }
    all
}

/// 0 = label starts with the query, 1 = a word does, 2 = it appears
/// anywhere, 3 = its letters appear in order.
fn score(label: &str, q: &str) -> Option<u8> {
    if label.starts_with(q) {
        return Some(0);
    }
    if label.split(|c: char| !c.is_alphanumeric()).any(|w| !w.is_empty() && w.starts_with(q)) {
        return Some(1);
    }
    if label.contains(q) {
        return Some(2);
    }
    let mut rest = label.chars();
    q.chars().all(|c| rest.any(|l| l == c)).then_some(3)
}

/// "Psalm 23", "John 3:16" or "Genesis" when `q` parses as a reference.
fn reference_label(q: &str) -> Option<String> {
    match crate::reference::parse(q).ok()? {
        BibleReferenceRepresentation::Single(BibleReference::BibleVerse(v)) => {
            let (c, n): (u32, u32) = (v.chapter().into(), v.verse().into());
            Some(format!("{} {c}:{n}", book_display(&v.book())))
        }
        BibleReferenceRepresentation::Single(BibleReference::BibleChapter(cr)) => {
            let c: u32 = cr.chapter().into();
            Some(format!("{} {c}", book_display(&cr.book())))
        }
        BibleReferenceRepresentation::Single(BibleReference::BibleBook(_)) => {
            super::nav::first_chapter_of_parsed(q)
                .ok()
                .map(|cr| book_display(&cr.book()).to_string())
        }
        BibleReferenceRepresentation::Range(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> Context<'static> {
        Context {
            installed: &[],
            current_translation: Some("EnglishKJBible"),
            theme: ThemePreset::Nocturne,
            layout: ReadingLayout::Auto,
            columns: ColumnLayout::Auto,
        }
    }

    #[test]
    fn reference_comes_first() {
        let items = items("ps 23", &ctx());
        assert_eq!(items[0].action, Action::Jump("ps 23".into()));
        assert_eq!(items[0].label, "Go to Psalms 23");
        assert!(matches!(items[1].action, Action::Search(_)));
    }

    #[test]
    fn typing_a_command_name_finds_it_before_search() {
        assert_eq!(items("qu", &ctx())[0].action, Action::Run(Cmd::Quit));
        assert_eq!(items("dark", &ctx())[0].action, Action::Theme(ThemePreset::SolarizedDark));
    }

    #[test]
    fn plain_words_fall_back_to_search() {
        let items = items("living water", &ctx());
        assert_eq!(items[0].action, Action::Search("living water".into()));
    }

    #[test]
    fn empty_query_lists_every_theme_and_quit() {
        let items = items("", &ctx());
        for p in ThemePreset::ALL {
            assert!(items.iter().any(|i| i.action == Action::Theme(p)));
        }
        assert!(items.iter().any(|i| i.action == Action::Run(Cmd::Quit)));
    }
}
