//! Terminal UI: Reader + Translation Manager.
//!
//! Single-file by design — keeps the event loop, state, and drawing in one
//! place where they can be read top-to-bottom. Cross-platform via
//! `crossterm`. Downloads run on a worker thread that pushes progress
//! through an `mpsc::Sender<AppEvent>` consumed by the main loop.

mod draw;
mod nav;
mod note_editor;
mod stderr_redirect;

pub(crate) use note_editor::{NoteEditor, NoteEditorTarget};

use std::io::{self, Stdout};
use std::sync::Arc;
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tui_input::Input;
use tui_input::backend::crossterm::EventHandler;

use crate::bible::{Bible, TranslationInfo};
use crate::manifest::{self, AvailableTranslation};
use crate::reference::{
    BibleChapterReference, BibleReference, BibleReferenceRepresentation, BibleVerseReference,
    book_from_number, get_bible_book_by_number,
};
use crate::storage;

pub(crate) type Tui = Terminal<CrosstermBackend<Stdout>>;

#[derive(Debug)]
pub(crate) enum AppEvent {
    Key(KeyEvent),
    Tick,
    DownloadProgress { id: String, bytes: u64, total: Option<u64> },
    DownloadDone { id: String, result: std::result::Result<TranslationInfo, String> },
    SearchDone { query: String, hits: Vec<BibleVerseReference> },
}

pub(crate) const SPINNER_FRAMES: &[&str] = &["|", "/", "-", "\\"];

pub(crate) struct SearchingState {
    pub query: String,
    pub spinner: usize,
    pub started_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Normal,
    Jump,
    Search,
    Manager,
    Bookmarks,
    PickBook,
    PickSecondary,
    Settings,
    EditingNote,
    Plan,
    Help,
    NoTranslation,
    Quit,
}

pub(crate) struct DownloadState {
    pub id: String,
    pub bytes: u64,
    pub total: Option<u64>,
}

const MAX_HISTORY: usize = 50;

/// "Genesis 1" or "John 3:16" — used in status messages where we don't
/// have a `BibleChapterReference` handy (e.g. saving a bookmark from the
/// note editor's deferred target).
fn format_bookmark_ref(book_number: u8, chapter: u16, verse: Option<u16>) -> String {
    let book = crate::reference::book_from_number(book_number)
        .ok()
        .map(|b| crate::reference::book_display(&b))
        .unwrap_or("?");
    match verse {
        Some(v) => format!("{} {}:{}", book, chapter, v),
        None => format!("{} {}", book, chapter),
    }
}

fn same_chapter(a: &NavSnapshot, b: &NavSnapshot) -> bool {
    a.translation == b.translation
        && a.book_number == b.book_number
        && a.chapter == b.chapter
}

/// Cross-platform clipboard write. Returns Err with a human-readable
/// message on failure (e.g. headless tty with no DISPLAY).
fn copy_to_clipboard(text: &str) -> std::result::Result<(), String> {
    let mut cb = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    cb.set_text(text.to_string()).map_err(|e| e.to_string())?;
    Ok(())
}

fn push_history(history: &mut Vec<String>, entry: &str) {
    let entry = entry.trim();
    if entry.is_empty() {
        return;
    }
    if history.last().is_some_and(|last| last == entry) {
        return;
    }
    history.push(entry.to_string());
    if history.len() > MAX_HISTORY {
        history.remove(0);
    }
}

pub(crate) struct App {
    /// Wrapped in `Arc` so a search worker thread can hold a cheap clone
    /// without copying the Bible's ~4MB of text.
    pub bible: Option<Arc<Bible>>,
    pub installed: Vec<TranslationInfo>,
    pub available: Vec<AvailableTranslation>,
    pub current: Option<BibleChapterReference>,
    /// 1-indexed verse cursor — the verse the user is currently focused on.
    /// Moves with `↑/↓`, drives the on-screen highlight, and (in single-
    /// pane mode) pulls the viewport along when it leaves the visible area.
    pub focus_verse: u16,
    /// Top row of the single-pane viewport (line index into the rendered
    /// row list). Adjusted *during draw* — input handlers never touch it
    /// directly. Unused in parallel mode (each pane derives its own start
    /// from `focus_verse`).
    pub scroll: u16,
    /// One-shot flag: on the next draw, pin scroll so the focused verse
    /// sits at the top. Set on chapter-changing navigation (`:ref`, ←/→,
    /// search-hit, bookmark, translation switch); cleared the next frame.
    /// Without this, `↑/↓` flow naturally — cursor moves inside the
    /// viewport until it hits an edge, then scrolls.
    pub pin_focus: bool,
    pub mode: Mode,
    pub input: Input,
    pub status: String,
    pub status_at: Instant,

    pub search_hits: Vec<BibleVerseReference>,
    pub search_idx: usize,
    pub last_search: String,
    pub searching: Option<SearchingState>,

    /// 0-indexed cursor into the 1..=66 book list shown by the `g` book
    /// picker. Set to the current book's index when the picker opens.
    pub book_picker_cursor: usize,

    pub manager_filter: Input,
    pub manager_cursor: usize,
    /// Preserved across frames so ratatui auto-scrolls the list to keep
    /// the cursor in view (the cursor index alone doesn't carry offset).
    pub manager_list_state: ratatui::widgets::ListState,

    pub bookmarks: Vec<crate::bookmarks::Bookmark>,
    pub bookmarks_cursor: usize,
    /// Vertical scroll for the note-preview pane in the bookmarks list view.
    pub bookmarks_note_scroll: u16,

    /// Active multi-line note editor, if open. `Mode::EditingNote` ↔
    /// `note_editor.is_some()` invariant.
    pub note_editor: Option<NoteEditor>,

    /// Bible-in-a-Year plan generated for the current year on launch.
    /// Plan content is deterministic from the year alone — only progress
    /// (`plan_completed`) is persisted.
    pub plan: crate::plan::ReadingPlan,
    pub plan_completed: std::collections::HashSet<u16>,
    /// 0-indexed day cursor in the plan list view.
    pub plan_cursor: usize,
    /// In `Mode::EditingNote`, which pane has the keyboard? `false`
    /// (default) = the editor; `true` = the reader pane on the left, so
    /// the user can scroll the chapter for context while writing. `Tab`
    /// toggles.
    pub note_editor_focus_reader: bool,

    /// User preferences — typography, theme, reader width, parallel divider.
    /// Mutated live by the Settings modal; flushed to disk on close.
    pub settings: crate::settings::Settings,
    pub settings_cursor: usize,

    /// Parallel-view state. `parallel` is on iff both this is true *and*
    /// `secondary_bible` is loaded. The shared verse anchor between the two
    /// panes is `focus_verse`.
    pub parallel: bool,
    pub secondary_bible: Option<Arc<Bible>>,
    pub secondary_id: Option<String>,
    pub secondary_picker_cursor: usize,

    pub download: Option<DownloadState>,
    pub event_tx: Sender<AppEvent>,

    /// Set when the next render should fully repaint (terminal.clear). Used
    /// after switching translations so wide-char skip cells from the old
    /// translation can't leak through ratatui's diff renderer.
    pub needs_clear: bool,

    /// Vim-style command history for the `:` jump bar.
    /// Newest entry is last. `_idx` points at the entry currently shown in
    /// the input; `None` means the user is composing a fresh entry.
    pub jump_history: Vec<String>,
    pub jump_history_idx: Option<usize>,
    pub search_history: Vec<String>,
    pub search_history_idx: Option<usize>,

    /// Browser-style back/forward over reading positions. Each user-driven
    /// chapter-level navigation pushes the *previous* position onto
    /// `back_stack` and clears `forward_stack`. Ctrl-O pops back; Ctrl-I
    /// pops forward. Dedupe is on (translation, book, chapter) so scrolling
    /// inside one chapter doesn't pollute the history.
    pub back_stack: Vec<NavSnapshot>,
    pub forward_stack: Vec<NavSnapshot>,
}

#[derive(Debug, Clone)]
pub(crate) struct NavSnapshot {
    pub translation: String,
    pub book_number: u8,
    pub chapter: u16,
    pub focus_verse: u16,
}

pub fn run(initial_translation: Option<String>) -> Result<()> {
    install_panic_hook();
    // Suppress library-side `eprintln!` so stderr writes don't scroll the
    // alternate-screen TUI display and visually push the header off-screen.
    crate::set_quiet(true);
    // Also redirect stderr at the OS level: some deps (e.g. bibleref 0.4)
    // call `dbg!()` from inside their parser, bypassing our quiet flag.
    stderr_redirect::silence();
    let mut terminal = setup_terminal()?;
    let result = run_app(&mut terminal, initial_translation);
    let _ = teardown_terminal();
    stderr_redirect::restore();
    crate::set_quiet(false);
    result
}

fn setup_terminal() -> Result<Tui> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    Ok(Terminal::new(CrosstermBackend::new(stdout))?)
}

fn teardown_terminal() -> Result<()> {
    let mut stdout = io::stdout();
    execute!(stdout, LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}

fn install_panic_hook() {
    let main_thread = thread::current().id();
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if thread::current().id() == main_thread {
            // Main-thread panic: restore stderr and the terminal first so
            // the trace is visible, then run the default printer.
            stderr_redirect::restore();
            let _ = teardown_terminal();
            original(info);
        }
        // Worker-thread panic: stay silent. catch_unwind in the worker
        // captures the payload and reports it via mpsc; printing here
        // would scroll the alternate-screen TUI.
    }));
}

fn run_app(terminal: &mut Tui, initial_translation: Option<String>) -> Result<()> {
    let (tx, rx) = mpsc::channel::<AppEvent>();
    spawn_input_thread(tx.clone());
    spawn_tick_thread(tx.clone());

    let mut app = App::new(tx.clone())?;
    app.choose_initial_translation(initial_translation)?;

    while app.mode != Mode::Quit {
        if app.needs_clear {
            // ANSI clear + reset both buffers → next draw is a full repaint.
            terminal.clear()?;
            app.needs_clear = false;
        }
        terminal.draw(|f| draw::draw(f, &mut app))?;
        // 100ms timeout drives the search spinner animation; idle CPU cost
        // of waking the loop 10×/sec is negligible.
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(ev) => app.handle_event(ev)?,
            Err(mpsc::RecvTimeoutError::Timeout) => app.on_idle(),
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    app.save_state();
    Ok(())
}

fn spawn_input_thread(tx: Sender<AppEvent>) {
    thread::spawn(move || {
        loop {
            match event::read() {
                Ok(Event::Key(k)) => {
                    if k.kind != KeyEventKind::Release && tx.send(AppEvent::Key(k)).is_err() {
                        break;
                    }
                }
                Ok(Event::Resize(_, _)) => {
                    let _ = tx.send(AppEvent::Tick);
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
    });
}

fn spawn_tick_thread(_tx: Sender<AppEvent>) {
    // Animation is driven by the recv_timeout in run_app via App::on_idle —
    // no separate ticker thread needed. Kept as a stub so the call in
    // run_app stays put for future use.
}

impl App {
    fn new(event_tx: Sender<AppEvent>) -> Result<Self> {
        let installed = storage::list_installed().unwrap_or_default();
        let available = manifest::list_available();
        // Plan: regenerate for current year. If the saved progress is for
        // a different year (or missing/old schema), discard and start
        // fresh — matches the "discard on year change" decision.
        let plan_year = crate::plan::current_year();
        let plan = crate::plan::generate_bible_in_a_year(plan_year);
        let plan_completed =
            crate::plan::load_completed_for(plan_year, &plan.plan_id);
        Ok(Self {
            bible: None,
            installed,
            available,
            current: None,
            focus_verse: 1,
            scroll: 0,
            pin_focus: true,
            mode: Mode::Normal,
            input: Input::default(),
            status: String::new(),
            status_at: Instant::now(),
            search_hits: Vec::new(),
            search_idx: 0,
            last_search: String::new(),
            searching: None,
            book_picker_cursor: 0,
            manager_filter: Input::default(),
            manager_cursor: 0,
            manager_list_state: ratatui::widgets::ListState::default(),
            bookmarks: crate::bookmarks::load(),
            bookmarks_cursor: 0,
            bookmarks_note_scroll: 0,
            note_editor: None,
            note_editor_focus_reader: false,
            plan,
            plan_completed,
            plan_cursor: 0,
            settings: crate::settings::load(),
            settings_cursor: 0,
            parallel: false,
            secondary_bible: None,
            secondary_id: None,
            secondary_picker_cursor: 0,
            download: None,
            event_tx,
            needs_clear: false,
            jump_history: Vec::new(),
            jump_history_idx: None,
            search_history: Vec::new(),
            search_history_idx: None,
            back_stack: Vec::new(),
            forward_stack: Vec::new(),
        })
    }

    fn choose_initial_translation(&mut self, requested: Option<String>) -> Result<()> {
        if self.installed.is_empty() {
            self.mode = Mode::NoTranslation;
            self.set_status("No translations installed. Press `i` to install KJV, or `T` to browse.");
            return Ok(());
        }

        let saved = crate::state::load();
        let saved_position = saved
            .as_ref()
            .and_then(|s| s.last_position.as_ref())
            .filter(|p| self.installed.iter().any(|t| t.id == p.translation));

        // Resolution order:
        //   1. Explicit `--translation <id>` argument.
        //   2. Last position from saved state, if its translation is still installed.
        //   3. `settings.reader.default_translation`, if set and installed.
        //   4. First installed translation alphabetically.
        let id = if let Some(req) = requested {
            manifest::resolve_id(&req).unwrap_or_else(|_| self.installed[0].id.clone())
        } else if let Some(p) = saved_position {
            p.translation.clone()
        } else {
            let default_id = self.settings.reader.default_translation.trim();
            if !default_id.is_empty()
                && self.installed.iter().any(|t| t.id == default_id)
            {
                default_id.to_string()
            } else {
                self.installed[0].id.clone()
            }
        };

        self.load_translation(&id)?;

        // After the default Genesis 1 / focus_verse=1 from load_translation,
        // restore the saved chapter and verse cursor if they belong to the
        // translation we just loaded.
        if let Some(p) = saved_position {
            if p.translation == id {
                if let Ok(book) = book_from_number(p.book_number) {
                    if let Ok(chap_u8) = u8::try_from(p.chapter) {
                        if let Ok(cr) = BibleChapterReference::new(book, chap_u8) {
                            self.current = Some(cr);
                            self.focus_verse = p.focus_verse.max(1);
                            // load_translation set needs_clear; keep that.
                        }
                    }
                }
            }
        }

        // Restore parallel-view config if the secondary is still installed.
        if let Some(sv) = saved.as_ref() {
            if let Some(p) = &sv.parallel {
                if crate::storage::is_installed(&p.secondary_translation) {
                    if let Ok(b) = Bible::load(&p.secondary_translation) {
                        self.secondary_bible = Some(Arc::new(b));
                        self.secondary_id = Some(p.secondary_translation.clone());
                        self.parallel = true;
                    }
                }
            }
        }

        Ok(())
    }

    /// Persist current reading position (translation + chapter + scroll) and
    /// parallel-view config to `state.toml`. Called on quit and after every
    /// navigation that changes the chapter, plus parallel toggles. Best-
    /// effort: any error is silently swallowed because state persistence
    /// must never block the user.
    pub(crate) fn save_state(&self) {
        let Some(bible) = self.bible.as_ref() else {
            return;
        };
        let Some(cr) = self.current.as_ref() else {
            return;
        };
        let chap_u32: u32 = cr.chapter().into();
        let Ok(chapter) = u16::try_from(chap_u32) else {
            return;
        };
        let parallel = if self.parallel {
            self.secondary_id.as_ref().map(|id| crate::state::Parallel {
                secondary_translation: id.clone(),
            })
        } else {
            None
        };
        let state = crate::state::State {
            schema_version: 2,
            last_position: Some(crate::state::LastPosition {
                translation: bible.translation.id.clone(),
                book_number: cr.book().number(),
                chapter,
                focus_verse: self.focus_verse,
            }),
            parallel,
        };
        let _ = crate::state::save(&state);
    }

    fn snapshot(&self) -> Option<NavSnapshot> {
        let bible = self.bible.as_ref()?;
        let cr = self.current.as_ref()?;
        let chap_u32: u32 = cr.chapter().into();
        let chapter = u16::try_from(chap_u32).ok()?;
        Some(NavSnapshot {
            translation: bible.translation.id.clone(),
            book_number: cr.book().number(),
            chapter,
            focus_verse: self.focus_verse,
        })
    }

    /// Record the current position before navigating elsewhere. Dedupe on
    /// (translation, book, chapter) so scrolling within one chapter doesn't
    /// fragment the back stack. Caps the stack at 100 entries; clears
    /// forward_stack since a new navigation invalidates the redo path
    /// (browser semantics).
    fn push_history(&mut self) {
        let Some(snap) = self.snapshot() else { return };
        if let Some(last) = self.back_stack.last() {
            if same_chapter(last, &snap) {
                return;
            }
        }
        self.back_stack.push(snap);
        if self.back_stack.len() > 100 {
            self.back_stack.remove(0);
        }
        self.forward_stack.clear();
    }

    fn nav_back(&mut self) {
        let Some(snap) = self.back_stack.pop() else {
            self.set_status("no further back");
            return;
        };
        if let Some(cur) = self.snapshot() {
            self.forward_stack.push(cur);
        }
        if let Err(e) = self.restore_snapshot(&snap) {
            self.set_status(format!("back failed: {e}"));
        }
    }

    fn nav_forward(&mut self) {
        let Some(snap) = self.forward_stack.pop() else {
            self.set_status("no further forward");
            return;
        };
        if let Some(cur) = self.snapshot() {
            self.back_stack.push(cur);
        }
        if let Err(e) = self.restore_snapshot(&snap) {
            self.set_status(format!("forward failed: {e}"));
        }
    }

    /// Apply a snapshot. Reloads the translation only if it differs from
    /// the currently-loaded one. Bypasses `push_history` — the caller is
    /// already traversing the stack, not creating new entries.
    fn restore_snapshot(&mut self, snap: &NavSnapshot) -> Result<()> {
        let cur_id = self
            .bible
            .as_ref()
            .map(|b| b.translation.id.clone())
            .unwrap_or_default();
        if cur_id != snap.translation {
            if !storage::is_installed(&snap.translation) {
                return Err(anyhow::anyhow!(
                    "translation `{}` is no longer installed",
                    snap.translation
                ));
            }
            self.load_translation(&snap.translation)?;
        }
        let book = book_from_number(snap.book_number)
            .map_err(|_| anyhow::anyhow!("invalid book number"))?;
        let chap_u8 = u8::try_from(snap.chapter)
            .map_err(|_| anyhow::anyhow!("invalid chapter"))?;
        let cr = BibleChapterReference::new(book, chap_u8)
            .map_err(|_| anyhow::anyhow!("invalid chapter reference"))?;
        self.current = Some(cr);
        self.focus_verse = snap.focus_verse.max(1);
        self.pin_focus = true;
        self.save_state();
        Ok(())
    }

    pub(crate) fn load_translation(&mut self, id: &str) -> Result<()> {
        let bible = Bible::load(id)?;
        let first_book = bible
            .books
            .first()
            .and_then(|b| book_from_number(b.book_number).ok())
            .unwrap_or_else(|| get_bible_book_by_number(1).expect("Genesis"));
        self.current = BibleChapterReference::new(first_book, 1).ok();
        self.bible = Some(Arc::new(bible));
        self.focus_verse = 1;
        self.pin_focus = true;
        self.mode = Mode::Normal;
        // Switching translations invalidates any in-flight search results
        // (different verse text → different hits).
        self.search_hits.clear();
        self.searching = None;
        // Force a full repaint on next frame to scrub wide-char ghosts left
        // behind by the previous translation.
        self.needs_clear = true;
        Ok(())
    }

    fn set_status(&mut self, s: impl Into<String>) {
        self.status = s.into();
        self.status_at = Instant::now();
    }

    pub(crate) fn on_idle(&mut self) {
        if let Some(s) = self.searching.as_mut() {
            s.spinner = (s.spinner + 1) % SPINNER_FRAMES.len();
        }
        if !self.status.is_empty() && self.status_at.elapsed() > Duration::from_secs(5) {
            self.status.clear();
        }
    }

    fn handle_event(&mut self, ev: AppEvent) -> Result<()> {
        match ev {
            AppEvent::Key(k) => self.handle_key(k)?,
            AppEvent::Tick => self.on_idle(),
            AppEvent::SearchDone { query, hits } => {
                // A second search may have started before this one came back.
                // Only apply if the result still matches the active query.
                let still_active = self
                    .searching
                    .as_ref()
                    .is_some_and(|s| s.query == query);
                if !still_active {
                    return Ok(());
                }
                self.searching = None;
                if hits.is_empty() {
                    self.search_hits.clear();
                    self.set_status(format!("no matches for `{query}`"));
                } else {
                    let n = hits.len();
                    self.search_hits = hits;
                    self.search_idx = 0;
                    self.last_search = query.clone();
                    self.set_status(format!("{n} hits for `{query}`"));
                    self.go_to_current_hit();
                }
            }
            AppEvent::DownloadProgress { id, bytes, total } => {
                if let Some(d) = self.download.as_mut() {
                    if d.id == id {
                        d.bytes = bytes;
                        d.total = total;
                    }
                }
            }
            AppEvent::DownloadDone { id, result } => {
                self.download = None;
                match result {
                    Ok(info) => {
                        self.installed = storage::list_installed().unwrap_or_default();
                        if self.bible.is_none() {
                            self.load_translation(&info.id)?;
                        }
                        self.set_status(format!("installed: {} ({})", info.id, info.display_name));
                    }
                    Err(e) => self.set_status(format!("install failed [{id}]: {e}")),
                }
            }
        }
        Ok(())
    }

    fn handle_key(&mut self, k: KeyEvent) -> Result<()> {
        if k.modifiers.contains(KeyModifiers::CONTROL) && matches!(k.code, KeyCode::Char('c')) {
            self.mode = Mode::Quit;
            return Ok(());
        }

        match self.mode {
            Mode::Normal => self.handle_normal(k)?,
            Mode::Jump => self.handle_jump(k),
            Mode::Search => self.handle_search(k),
            Mode::Manager => self.handle_manager(k),
            Mode::Help => match k.code {
                KeyCode::Char('?') | KeyCode::Esc | KeyCode::Char('q') => {
                    self.mode = Mode::Normal
                }
                _ => {}
            },
            Mode::NoTranslation => self.handle_no_translation(k),
            Mode::Bookmarks => self.handle_bookmarks(k),
            Mode::PickBook => self.handle_pick_book(k),
            Mode::PickSecondary => self.handle_pick_secondary(k),
            Mode::Settings => self.handle_settings(k),
            Mode::EditingNote => self.handle_editing_note(k),
            Mode::Plan => self.handle_plan(k),
            Mode::Quit => {}
        }
        Ok(())
    }

    fn handle_normal(&mut self, k: KeyEvent) -> Result<()> {
        let modless = k.modifiers.is_empty() || k.modifiers == KeyModifiers::SHIFT;
        match k.code {
            KeyCode::Char('q') if modless => self.mode = Mode::Quit,
            KeyCode::Char('?') if modless => self.mode = Mode::Help,
            KeyCode::Char(':') if modless => {
                self.input = Input::default();
                self.mode = Mode::Jump;
            }
            KeyCode::Char('/') if modless => {
                self.input = Input::default();
                self.mode = Mode::Search;
            }
            KeyCode::Char('T') if modless => {
                self.open_manager();
            }
            KeyCode::Char('t') if modless => self.cycle_translation(1),
            KeyCode::Char('j') | KeyCode::Down => self.shift_focus(1),
            KeyCode::Char('k') | KeyCode::Up => self.shift_focus(-1),
            KeyCode::Char('d') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                self.shift_focus(5);
            }
            KeyCode::Char('u') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                self.shift_focus(-5);
            }
            // Arrow + paging key surface (the user-facing one).
            KeyCode::Left if k.modifiers.contains(KeyModifiers::SHIFT) => self.go_book(-1),
            KeyCode::Right if k.modifiers.contains(KeyModifiers::SHIFT) => self.go_book(1),
            KeyCode::Left if k.modifiers.is_empty() => self.go_chapter(-1),
            KeyCode::Right if k.modifiers.is_empty() => self.go_chapter(1),
            KeyCode::PageDown => self.shift_focus(5),
            KeyCode::PageUp => self.shift_focus(-5),
            KeyCode::Home => self.focus_verse = 1,
            KeyCode::End => self.focus_verse = self.max_verse_in_chapter(),
            // Vim-style fallbacks — kept silent (no hint surface) for
            // muscle-memory; arrow keys are the documented surface.
            KeyCode::Char('h') if modless => self.go_chapter(-1),
            KeyCode::Char('l') if modless => self.go_chapter(1),
            KeyCode::Char('H') => self.go_book(-1),
            KeyCode::Char('L') => self.go_book(1),
            KeyCode::Char('g') if modless => self.open_book_picker(),
            KeyCode::Char('G') => self.focus_verse = self.max_verse_in_chapter(),
            KeyCode::Char('n') if modless => self.advance_search_hit(1),
            KeyCode::Char('N') => self.advance_search_hit(-1),
            KeyCode::Char('b') if modless => self.bookmark_current_chapter(),
            KeyCode::Char('B') => self.open_bookmarks(),
            KeyCode::Char('|') => self.toggle_parallel(),
            KeyCode::Char('\\') => self.open_secondary_picker(),
            KeyCode::Char(',') => self.open_settings(),
            // Browser-style back / forward across chapter-level jumps.
            // Vim convention is Ctrl-O / Ctrl-I, but Ctrl-I and Tab both
            // emit byte 0x09 in most terminals, so crossterm reports it as
            // `KeyCode::Tab`. Bind Tab as the working forward; keep
            // Char('i')+Ctrl for the few terminals (kitty keyboard
            // protocol, etc.) that do distinguish.
            KeyCode::Char('o') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                self.nav_back();
            }
            KeyCode::Tab => self.nav_forward(),
            KeyCode::Char('i') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                self.nav_forward();
            }
            KeyCode::Char('y') if modless => self.yank_current_verse(),
            KeyCode::Char('p') if modless => self.jump_to_today_reading(),
            KeyCode::Char('P') => self.open_plan_view(),
            _ => {}
        }
        Ok(())
    }

    fn handle_no_translation(&mut self, k: KeyEvent) {
        match k.code {
            KeyCode::Char('q') => self.mode = Mode::Quit,
            KeyCode::Char('i') => self.start_install("EnglishKJBible"),
            KeyCode::Char('T') => self.open_manager(),
            KeyCode::Char('?') => self.mode = Mode::Help,
            _ => {}
        }
    }

    fn handle_jump(&mut self, k: KeyEvent) {
        match k.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.input = Input::default();
                self.jump_history_idx = None;
            }
            KeyCode::Enter => {
                let q = self.input.value().to_string();
                self.mode = Mode::Normal;
                self.input = Input::default();
                self.jump_history_idx = None;
                push_history(&mut self.jump_history, &q);
                let trimmed = q.trim();
                if trimmed == "b" {
                    self.bookmark_current_chapter();
                } else if let Some(rest) = trimmed.strip_prefix("b ") {
                    self.handle_bookmark_command(rest.trim());
                } else if trimmed == "y" {
                    self.yank_current_verse();
                } else if let Some(rest) = trimmed.strip_prefix("y ") {
                    self.handle_yank_command(rest.trim());
                } else {
                    self.jump_to(&q);
                }
            }
            KeyCode::Up => self.history_prev(true),
            KeyCode::Down => self.history_next(true),
            _ => {
                // typing into a recalled history entry breaks the browse cursor —
                // treat the recalled value as the new fresh entry.
                self.jump_history_idx = None;
                let _ = self.input.handle_event(&Event::Key(k));
            }
        }
    }

    fn handle_search(&mut self, k: KeyEvent) {
        match k.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.input = Input::default();
                self.search_history_idx = None;
            }
            KeyCode::Enter => {
                let q = self.input.value().to_string();
                self.mode = Mode::Normal;
                self.input = Input::default();
                self.search_history_idx = None;
                push_history(&mut self.search_history, &q);
                self.run_search(&q);
            }
            KeyCode::Up => self.history_prev(false),
            KeyCode::Down => self.history_next(false),
            _ => {
                self.search_history_idx = None;
                let _ = self.input.handle_event(&Event::Key(k));
            }
        }
    }

    fn history_prev(&mut self, jump: bool) {
        let (history, idx) = if jump {
            (&self.jump_history, &mut self.jump_history_idx)
        } else {
            (&self.search_history, &mut self.search_history_idx)
        };
        if history.is_empty() {
            return;
        }
        let new_idx = match *idx {
            None => history.len() - 1,
            Some(0) => 0,
            Some(i) => i - 1,
        };
        *idx = Some(new_idx);
        self.input = Input::new(history[new_idx].clone());
    }

    fn history_next(&mut self, jump: bool) {
        let (history, idx) = if jump {
            (&self.jump_history, &mut self.jump_history_idx)
        } else {
            (&self.search_history, &mut self.search_history_idx)
        };
        let Some(cur) = *idx else { return };
        if cur + 1 < history.len() {
            *idx = Some(cur + 1);
            self.input = Input::new(history[cur + 1].clone());
        } else {
            *idx = None;
            self.input = Input::default();
        }
    }

    fn handle_manager(&mut self, k: KeyEvent) {
        // The filter is always the focus — every letter typed feeds it.
        // Commands use arrow keys, Esc, Enter, and Ctrl-modified letters
        // so they don't collide with names like "James" or "Jeremiah".
        match k.code {
            KeyCode::Esc => {
                self.mode = if self.bible.is_some() {
                    Mode::Normal
                } else {
                    Mode::NoTranslation
                };
            }
            KeyCode::Down => {
                let n = self.filtered_indices().len();
                if n > 0 {
                    self.manager_cursor = (self.manager_cursor + 1).min(n - 1);
                }
            }
            KeyCode::Up => {
                self.manager_cursor = self.manager_cursor.saturating_sub(1);
            }
            KeyCode::PageDown => {
                let n = self.filtered_indices().len();
                if n > 0 {
                    self.manager_cursor =
                        (self.manager_cursor + 10).min(n - 1);
                }
            }
            KeyCode::PageUp => {
                self.manager_cursor = self.manager_cursor.saturating_sub(10);
            }
            KeyCode::Enter => self.toggle_install_at_cursor(),
            KeyCode::Char('r') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                self.refresh_manifest_async();
            }
            KeyCode::Backspace => {
                let _ = self.manager_filter.handle_event(&Event::Key(k));
                self.manager_cursor = 0;
            }
            KeyCode::Char(_) => {
                let _ = self.manager_filter.handle_event(&Event::Key(k));
                self.manager_cursor = 0;
            }
            _ => {}
        }
    }

    fn open_manager(&mut self) {
        self.manager_filter = Input::default();
        self.manager_cursor = 0;
        self.mode = Mode::Manager;
    }

    pub(crate) fn filtered_indices(&self) -> Vec<usize> {
        let needle = self.manager_filter.value().to_ascii_lowercase();
        self.available
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                if needle.is_empty() {
                    return true;
                }
                t.id.to_ascii_lowercase().contains(&needle)
                    || t.display_name.to_ascii_lowercase().contains(&needle)
                    || t.language.to_ascii_lowercase().contains(&needle)
            })
            .map(|(i, _)| i)
            .collect()
    }

    fn toggle_install_at_cursor(&mut self) {
        let indices = self.filtered_indices();
        let Some(&idx) = indices.get(self.manager_cursor) else {
            return;
        };
        let id = self.available[idx].id.clone();
        if storage::is_installed(&id) {
            if let Err(e) = storage::uninstall(&id) {
                self.set_status(format!("uninstall failed: {e}"));
            } else {
                self.installed = storage::list_installed().unwrap_or_default();
                if self
                    .bible
                    .as_ref()
                    .is_some_and(|b| b.translation.id == id)
                {
                    self.bible = None;
                    self.current = None;
                }
                self.set_status(format!("uninstalled {id}"));
            }
        } else {
            self.start_install(&id);
        }
    }

    fn cycle_translation(&mut self, dir: i32) {
        if self.installed.len() < 2 {
            return;
        }
        let current_id = self
            .bible
            .as_ref()
            .map(|b| b.translation.id.clone())
            .unwrap_or_default();
        let pos = self
            .installed
            .iter()
            .position(|t| t.id == current_id)
            .unwrap_or(0);
        let n = self.installed.len() as i32;
        let next = (((pos as i32 + dir) % n) + n) % n;
        let id = self.installed[next as usize].id.clone();
        self.push_history();
        match self.load_translation(&id) {
            Ok(()) => {
                self.set_status(format!("switched to {id}"));
                self.save_state();
            }
            Err(e) => self.set_status(format!("load failed: {e}")),
        }
    }

    fn bookmark_current_chapter(&mut self) {
        self.add_bookmark(None, "");
    }

    /// Grammar:
    ///   `:b`              — chapter bookmark, no note
    ///   `:b N`            — verse N bookmark, no note
    ///   `:b note`         — chapter bookmark, opens multi-line editor
    ///   `:b N note`       — verse N bookmark, opens multi-line editor
    /// Anything else is a parse error.
    fn handle_bookmark_command(&mut self, args: &str) {
        if args.is_empty() {
            self.bookmark_current_chapter();
            return;
        }
        let mut parts = args.split_whitespace();
        let first = parts.next().unwrap_or("");
        let second = parts.next();
        let extra = parts.next();
        let verse: Option<u16> = first.parse::<u16>().ok();
        let opens_editor: bool = match (verse.is_some(), second, extra) {
            (false, None, _) if first == "note" => true,
            (false, _, _) if first != "note" => {
                self.set_status(
                    "syntax: :b | :b N | :b note | :b N note",
                );
                return;
            }
            (true, None, _) => false,
            (true, Some("note"), None) => true,
            _ => {
                self.set_status(
                    "syntax: :b | :b N | :b note | :b N note",
                );
                return;
            }
        };
        if opens_editor {
            self.open_note_editor_for_new(verse);
        } else {
            self.add_bookmark(verse, "");
        }
    }

    fn add_bookmark(&mut self, verse: Option<u16>, note: &str) {
        let Some(bible) = self.bible.as_ref() else {
            self.set_status("no translation loaded");
            return;
        };
        let Some(cr) = self.current.as_ref() else {
            return;
        };
        let chap_u32: u32 = cr.chapter().into();
        let Ok(chapter) = u16::try_from(chap_u32) else {
            return;
        };
        let book = cr.book();
        let label = match verse {
            Some(v) => format!("{} {}:{}", crate::reference::book_display(&book), chapter, v),
            None => format!("{} {}", crate::reference::book_display(&book), chapter),
        };
        let bm = crate::bookmarks::Bookmark {
            translation: bible.translation.id.clone(),
            book_number: book.number(),
            chapter,
            verse,
            note: note.to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        self.bookmarks.push(bm);
        match crate::bookmarks::save(&self.bookmarks) {
            Ok(()) => self.set_status(format!("bookmarked: {label}")),
            Err(e) => self.set_status(format!("bookmark save failed: {e}")),
        }
    }

    /// Open the note editor for a not-yet-saved bookmark. Cancel discards.
    fn open_note_editor_for_new(&mut self, verse: Option<u16>) {
        let Some(bible) = self.bible.as_ref() else {
            self.set_status("no translation loaded");
            return;
        };
        let Some(cr) = self.current.as_ref() else {
            return;
        };
        let chap_u32: u32 = cr.chapter().into();
        let Ok(chapter) = u16::try_from(chap_u32) else {
            return;
        };
        let book = cr.book();
        let ref_label = match verse {
            Some(v) => format!("{} {}:{}", crate::reference::book_display(&book), chapter, v),
            None => format!("{} {}", crate::reference::book_display(&book), chapter),
        };
        let label = format!("{} ({})", ref_label, bible.translation.display_name);
        let target = NoteEditorTarget::NewBookmark {
            translation: bible.translation.id.clone(),
            book_number: book.number(),
            chapter,
            verse,
        };
        self.note_editor = Some(NoteEditor::new(target, label, ""));
        self.note_editor_focus_reader = false;
        self.mode = Mode::EditingNote;
    }

    /// Open the note editor for the highlighted bookmark in the list view.
    fn open_note_editor_for_existing(&mut self) {
        let Some(bm) = self.bookmarks.get(self.bookmarks_cursor) else {
            return;
        };
        let book_label = crate::reference::book_from_number(bm.book_number)
            .ok()
            .map(|b| crate::reference::book_display(&b))
            .unwrap_or("?");
        let ref_label = match bm.verse {
            Some(v) => format!("{} {}:{}", book_label, bm.chapter, v),
            None => format!("{} {}", book_label, bm.chapter),
        };
        let label = format!("{} ({})", ref_label, bm.translation);
        let target = NoteEditorTarget::EditExisting {
            index: self.bookmarks_cursor,
        };
        self.note_editor = Some(NoteEditor::new(target, label, &bm.note));
        self.note_editor_focus_reader = false;
        self.mode = Mode::EditingNote;
    }

    fn handle_editing_note(&mut self, k: KeyEvent) {
        if self.note_editor.is_none() {
            self.mode = Mode::Normal;
            return;
        }
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        // Save / cancel / pane-toggle work from either focus.
        match (k.code, ctrl) {
            (KeyCode::Esc, _) => return self.cancel_note_editor(),
            (KeyCode::Char('s'), true) => return self.commit_note_editor(),
            (KeyCode::Tab, _) => {
                self.note_editor_focus_reader = !self.note_editor_focus_reader;
                return;
            }
            _ => {}
        }
        if self.note_editor_focus_reader {
            self.handle_editing_note_reader_keys(k);
        } else {
            self.handle_editing_note_editor_keys(k, ctrl);
        }
    }

    /// Editor pane has focus — typing/editing keys.
    fn handle_editing_note_editor_keys(&mut self, k: KeyEvent, ctrl: bool) {
        let Some(editor) = self.note_editor.as_mut() else {
            return;
        };
        match k.code {
            KeyCode::Enter => editor.insert_newline(),
            KeyCode::Backspace => editor.backspace(),
            KeyCode::Delete => editor.delete_forward(),
            KeyCode::Left => editor.move_left(),
            KeyCode::Right => editor.move_right(),
            KeyCode::Up => editor.move_up(),
            KeyCode::Down => editor.move_down(),
            KeyCode::Home => editor.move_home(),
            KeyCode::End => editor.move_end(),
            KeyCode::PageDown => editor.page_down(10),
            KeyCode::PageUp => editor.page_up(10),
            KeyCode::Char(c) if !ctrl => editor.insert_char(c),
            _ => {}
        }
    }

    /// Reader pane has focus — verse cursor + chapter navigation. Subset
    /// of `handle_normal` so destructive keys (`q`, `,`, `t`, etc.) don't
    /// hijack the editor session.
    fn handle_editing_note_reader_keys(&mut self, k: KeyEvent) {
        match k.code {
            KeyCode::Up => self.shift_focus(-1),
            KeyCode::Down => self.shift_focus(1),
            KeyCode::Left if k.modifiers.contains(KeyModifiers::SHIFT) => self.go_book(-1),
            KeyCode::Right if k.modifiers.contains(KeyModifiers::SHIFT) => self.go_book(1),
            KeyCode::Left => self.go_chapter(-1),
            KeyCode::Right => self.go_chapter(1),
            KeyCode::PageDown => self.shift_focus(5),
            KeyCode::PageUp => self.shift_focus(-5),
            KeyCode::Home => self.focus_verse = 1,
            KeyCode::End => self.focus_verse = self.max_verse_in_chapter(),
            _ => {}
        }
    }

    fn cancel_note_editor(&mut self) {
        let return_to_list = matches!(
            self.note_editor.as_ref().map(|e| &e.target),
            Some(NoteEditorTarget::EditExisting { .. })
        );
        self.note_editor = None;
        self.mode = if return_to_list {
            Mode::Bookmarks
        } else if self.bible.is_some() {
            Mode::Normal
        } else {
            Mode::NoTranslation
        };
    }

    fn commit_note_editor(&mut self) {
        let Some(editor) = self.note_editor.take() else {
            return;
        };
        let text = editor.current_text();
        let return_to_list = matches!(editor.target, NoteEditorTarget::EditExisting { .. });
        match editor.target {
            NoteEditorTarget::NewBookmark {
                translation,
                book_number,
                chapter,
                verse,
            } => {
                let label = format_bookmark_ref(book_number, chapter, verse);
                let bm = crate::bookmarks::Bookmark {
                    translation,
                    book_number,
                    chapter,
                    verse,
                    note: text,
                    created_at: chrono::Utc::now().to_rfc3339(),
                };
                self.bookmarks.push(bm);
                match crate::bookmarks::save(&self.bookmarks) {
                    Ok(()) => self.set_status(format!("bookmarked: {label}")),
                    Err(e) => self.set_status(format!("bookmark save failed: {e}")),
                }
            }
            NoteEditorTarget::EditExisting { index } => {
                if let Some(bm) = self.bookmarks.get_mut(index) {
                    bm.note = text;
                    match crate::bookmarks::save(&self.bookmarks) {
                        Ok(()) => self.set_status("note saved"),
                        Err(e) => self.set_status(format!("save failed: {e}")),
                    }
                }
            }
        }
        self.mode = if return_to_list {
            Mode::Bookmarks
        } else {
            Mode::Normal
        };
    }

    /// `y` keypress — copy the verse under the focus cursor.
    fn yank_current_verse(&mut self) {
        let v = self.focus_verse;
        self.yank_verses(v, v);
    }

    /// `:y N`, `:y N-M`, `:y all` / `:y *`.
    fn handle_yank_command(&mut self, args: &str) {
        if args.is_empty() {
            self.yank_current_verse();
            return;
        }
        if matches!(args, "all" | "*") {
            self.yank_verses(1, u16::MAX);
            return;
        }
        if let Some((s, e)) = args.split_once('-') {
            if let (Ok(start), Ok(end)) = (s.trim().parse::<u16>(), e.trim().parse::<u16>()) {
                if start == 0 || end < start {
                    self.set_status(format!("invalid range: :y {args}"));
                    return;
                }
                self.yank_verses(start, end);
                return;
            }
        }
        if let Ok(n) = args.parse::<u16>() {
            self.yank_verses(n, n);
            return;
        }
        self.set_status(format!("unknown yank: :y {args}"));
    }

    fn yank_verses(&mut self, start: u16, end: u16) {
        let Some(bible) = self.bible.as_ref() else {
            self.set_status("no translation loaded");
            return;
        };
        let Some(cr) = self.current.as_ref() else {
            return;
        };
        let Some(chapter) = bible.get_chapter(cr) else {
            self.set_status("chapter not present in this translation");
            return;
        };
        let verses: Vec<&crate::bible::Verse> = chapter
            .verses
            .iter()
            .filter(|v| v.number >= start && v.number <= end)
            .collect();
        if verses.is_empty() {
            self.set_status(format!("no verses found in {start}-{end}"));
            return;
        }
        let body: String = verses
            .iter()
            .map(|v| v.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let actual_start = verses.first().unwrap().number;
        let actual_end = verses.last().unwrap().number;
        let chap_n: u32 = cr.chapter().into();
        let book_label = crate::reference::book_display(&cr.book());
        let ref_label = if actual_start == actual_end {
            format!("{} {}:{}", book_label, chap_n, actual_start)
        } else {
            format!("{} {}:{}-{}", book_label, chap_n, actual_start, actual_end)
        };
        let payload = format!(
            "{}\n\n— {} ({})",
            body, ref_label, bible.translation.display_name
        );
        match copy_to_clipboard(&payload) {
            Ok(()) => self.set_status(format!(
                "copied {} ({} chars)",
                ref_label,
                payload.chars().count()
            )),
            Err(e) => self.set_status(format!("copy failed: {e}")),
        }
    }

    /// `p` — jump to today's reading and auto-mark today as completed.
    /// User can unmark from the plan view if they jumped by mistake.
    fn jump_to_today_reading(&mut self) {
        let today = crate::plan::today_day_of_year();
        let day_idx = (today.saturating_sub(1)) as usize;
        self.jump_to_plan_day(day_idx, true);
    }

    /// `P` — open the full-year plan list, with the cursor on today.
    fn open_plan_view(&mut self) {
        let today = crate::plan::today_day_of_year();
        self.plan_cursor = today.saturating_sub(1) as usize;
        if self.plan_cursor >= self.plan.days.len() {
            self.plan_cursor = self.plan.days.len().saturating_sub(1);
        }
        self.mode = Mode::Plan;
    }

    fn handle_plan(&mut self, k: KeyEvent) {
        let n = self.plan.days.len();
        if n == 0 {
            self.mode = Mode::Normal;
            return;
        }
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') => self.mode = Mode::Normal,
            KeyCode::Down | KeyCode::Char('j') => {
                self.plan_cursor = (self.plan_cursor + 1).min(n - 1);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.plan_cursor = self.plan_cursor.saturating_sub(1);
            }
            KeyCode::PageDown => {
                self.plan_cursor = (self.plan_cursor + 10).min(n - 1);
            }
            KeyCode::PageUp => {
                self.plan_cursor = self.plan_cursor.saturating_sub(10);
            }
            KeyCode::Home | KeyCode::Char('g') => self.plan_cursor = 0,
            KeyCode::End | KeyCode::Char('G') => self.plan_cursor = n - 1,
            KeyCode::Char('t') => {
                // Recenter on today.
                let today = crate::plan::today_day_of_year();
                self.plan_cursor = today.saturating_sub(1) as usize;
                if self.plan_cursor >= n {
                    self.plan_cursor = n - 1;
                }
            }
            KeyCode::Enter => {
                let idx = self.plan_cursor;
                self.jump_to_plan_day(idx, true);
            }
            KeyCode::Char('m') => self.toggle_plan_complete(self.plan_cursor),
            _ => {}
        }
    }

    fn jump_to_plan_day(&mut self, day_idx: usize, mark_complete: bool) {
        let Some(daily) = self.plan.days.get(day_idx).cloned() else {
            return;
        };
        let Some(first) = daily.refs.first() else {
            return;
        };
        let Ok(book) = book_from_number(first.book_number) else {
            self.set_status("plan: invalid book in reading");
            return;
        };
        let Ok(chap_u8) = u8::try_from(first.chapter_start) else {
            self.set_status("plan: invalid chapter in reading");
            return;
        };
        let Ok(cr) = BibleChapterReference::new(book, chap_u8) else {
            self.set_status("plan: invalid reference in reading");
            return;
        };
        if self.bible.is_none() {
            self.set_status("plan: install a translation first (T)");
            return;
        }
        self.push_history();
        self.current = Some(cr);
        self.focus_verse = 1;
        self.pin_focus = true;
        self.mode = Mode::Normal;
        if mark_complete {
            self.plan_completed.insert(daily.day);
            self.save_plan_progress();
        }
        self.save_state();
        self.set_status(format!(
            "📖 day {}: {}",
            daily.day,
            daily.short()
        ));
    }

    fn toggle_plan_complete(&mut self, day_idx: usize) {
        let Some(daily) = self.plan.days.get(day_idx) else {
            return;
        };
        if self.plan_completed.contains(&daily.day) {
            self.plan_completed.remove(&daily.day);
            self.set_status(format!("day {}: marked unread", daily.day));
        } else {
            self.plan_completed.insert(daily.day);
            self.set_status(format!("day {}: marked read", daily.day));
        }
        self.save_plan_progress();
    }

    fn save_plan_progress(&self) {
        let mut days: Vec<u16> = self.plan_completed.iter().copied().collect();
        days.sort();
        let file = crate::plan::PlanFile {
            schema_version: 1,
            year: self.plan.year,
            plan_id: self.plan.plan_id.clone(),
            completed_days: days,
        };
        let _ = crate::plan::save_progress(&file);
    }

    fn open_bookmarks(&mut self) {
        self.bookmarks_cursor = 0;
        self.mode = Mode::Bookmarks;
    }

    fn handle_bookmarks(&mut self, k: KeyEvent) {
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') => self.mode = Mode::Normal,
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.bookmarks.is_empty() {
                    let last = self.bookmarks.len() - 1;
                    self.bookmarks_cursor = (self.bookmarks_cursor + 1).min(last);
                    self.bookmarks_note_scroll = 0;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.bookmarks_cursor = self.bookmarks_cursor.saturating_sub(1);
                self.bookmarks_note_scroll = 0;
            }
            KeyCode::PageDown => {
                self.bookmarks_note_scroll = self.bookmarks_note_scroll.saturating_add(5);
            }
            KeyCode::PageUp => {
                self.bookmarks_note_scroll = self.bookmarks_note_scroll.saturating_sub(5);
            }
            KeyCode::Enter => self.jump_to_bookmark(),
            KeyCode::Char('d') => self.delete_bookmark(),
            KeyCode::Char('e') => self.open_note_editor_for_existing(),
            _ => {}
        }
    }

    fn jump_to_bookmark(&mut self) {
        let Some(bm) = self.bookmarks.get(self.bookmarks_cursor).cloned() else {
            return;
        };
        // Pre-validate everything so we don't push to history on a no-op.
        let Some(cr) = book_from_number(bm.book_number)
            .ok()
            .and_then(|book| u8::try_from(bm.chapter).ok().map(|c| (book, c)))
            .and_then(|(book, c)| BibleChapterReference::new(book, c).ok())
        else {
            self.set_status("invalid bookmark");
            return;
        };
        // Record the pre-jump position before any state mutation (a
        // possible translation switch comes next).
        self.push_history();
        let already_loaded = self
            .bible
            .as_ref()
            .map(|b| b.translation.id == bm.translation)
            .unwrap_or(false);
        if !already_loaded {
            if !crate::storage::is_installed(&bm.translation) {
                self.set_status(format!(
                    "translation `{}` is no longer installed",
                    bm.translation
                ));
                return;
            }
            if let Err(e) = self.load_translation(&bm.translation) {
                self.set_status(format!("load failed: {e}"));
                return;
            }
        }
        self.current = Some(cr);
        self.focus_verse = bm.verse.unwrap_or(1).max(1);
        self.pin_focus = true;
        self.mode = Mode::Normal;
        self.save_state();
    }

    fn delete_bookmark(&mut self) {
        if self.bookmarks_cursor >= self.bookmarks.len() {
            return;
        }
        self.bookmarks.remove(self.bookmarks_cursor);
        if self.bookmarks_cursor >= self.bookmarks.len() && self.bookmarks_cursor > 0 {
            self.bookmarks_cursor -= 1;
        }
        match crate::bookmarks::save(&self.bookmarks) {
            Ok(()) => self.set_status("bookmark removed"),
            Err(e) => self.set_status(format!("bookmark save failed: {e}")),
        }
    }

    fn open_settings(&mut self) {
        self.settings_cursor = 0;
        self.mode = Mode::Settings;
    }

    fn close_settings(&mut self) {
        if let Err(e) = crate::settings::save(&self.settings) {
            self.set_status(format!("settings save failed: {e}"));
        }
        // Translation/wide-grapheme padding may have changed → wipe stale
        // skip cells from the previous render.
        self.needs_clear = true;
        self.mode = Mode::Normal;
    }

    fn handle_settings(&mut self, k: KeyEvent) {
        let items = crate::tui::draw::SETTINGS_ITEMS;
        let n = items.len();
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') => self.close_settings(),
            KeyCode::Down | KeyCode::Char('j') => {
                if n > 0 {
                    self.settings_cursor = (self.settings_cursor + 1).min(n - 1);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.settings_cursor = self.settings_cursor.saturating_sub(1);
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if let Some(item) = items.get(self.settings_cursor) {
                    item.prev(self);
                }
            }
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter => {
                if let Some(item) = items.get(self.settings_cursor) {
                    item.next(self);
                }
            }
            _ => {}
        }
    }

    /// Move the verse cursor by `dir` (signed). Clamps to [1, max_verse]
    /// where `max_verse` is the count of verses in the current chapter.
    fn shift_focus(&mut self, dir: i32) {
        let cur = self.focus_verse as i32;
        let max = self.max_verse_in_chapter() as i32;
        let next = (cur + dir).clamp(1, max.max(1));
        self.focus_verse = next as u16;
    }

    /// Last verse number of the current chapter, or 1 if no chapter is
    /// loaded / the chapter is missing in this translation.
    fn max_verse_in_chapter(&self) -> u16 {
        self.bible
            .as_ref()
            .and_then(|b| self.current.as_ref().and_then(|cr| b.get_chapter(cr)))
            .and_then(|c| c.verses.last().map(|v| v.number))
            .unwrap_or(1)
    }

    fn toggle_parallel(&mut self) {
        if self.parallel {
            self.parallel = false;
            self.set_status("parallel view: off");
            self.needs_clear = true;
            self.save_state();
            return;
        }
        if self.installed.len() < 2 {
            self.set_status("install another translation first (T)");
            return;
        }
        if self.secondary_bible.is_some() {
            self.parallel = true;
            self.needs_clear = true;
            self.save_state();
            self.set_status(format!(
                "parallel: {} (| to close)",
                self.secondary_id.as_deref().unwrap_or("?"),
            ));
        } else {
            self.open_secondary_picker();
        }
    }

    fn open_book_picker(&mut self) {
        if self.bible.is_none() {
            self.set_status("install a translation first (T)");
            return;
        }
        // Pre-position the cursor on the current book so the picker opens
        // "where you are" rather than always at Genesis.
        let cur_book = self
            .current
            .as_ref()
            .map(|cr| cr.book().number())
            .unwrap_or(1);
        self.book_picker_cursor = cur_book.saturating_sub(1) as usize;
        self.mode = Mode::PickBook;
    }

    fn handle_pick_book(&mut self, k: KeyEvent) {
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') => self.mode = Mode::Normal,
            KeyCode::Down | KeyCode::Char('j') => {
                self.book_picker_cursor = (self.book_picker_cursor + 1).min(65);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.book_picker_cursor = self.book_picker_cursor.saturating_sub(1);
            }
            KeyCode::PageDown => {
                self.book_picker_cursor = (self.book_picker_cursor + 10).min(65);
            }
            KeyCode::PageUp => {
                self.book_picker_cursor = self.book_picker_cursor.saturating_sub(10);
            }
            KeyCode::Home | KeyCode::Char('g') => self.book_picker_cursor = 0,
            KeyCode::End | KeyCode::Char('G') => self.book_picker_cursor = 65,
            KeyCode::Enter => self.pick_book_at_cursor(),
            _ => {}
        }
    }

    fn pick_book_at_cursor(&mut self) {
        let n = (self.book_picker_cursor + 1) as u8;
        let Ok(book) = book_from_number(n) else {
            self.set_status("invalid book");
            return;
        };
        let Ok(cr) = BibleChapterReference::new(book, 1) else {
            self.set_status("invalid reference");
            return;
        };
        self.push_history();
        self.current = Some(cr);
        self.focus_verse = 1;
        self.pin_focus = true;
        self.mode = Mode::Normal;
        self.save_state();
    }

    fn open_secondary_picker(&mut self) {
        let primary_id = self
            .bible
            .as_ref()
            .map(|b| b.translation.id.as_str())
            .unwrap_or("");
        let other_count = self
            .installed
            .iter()
            .filter(|t| t.id != primary_id)
            .count();
        if other_count == 0 {
            self.set_status("install another translation first (T)");
            return;
        }
        self.secondary_picker_cursor = 0;
        self.mode = Mode::PickSecondary;
    }

    fn handle_pick_secondary(&mut self, k: KeyEvent) {
        let primary_id = self
            .bible
            .as_ref()
            .map(|b| b.translation.id.clone())
            .unwrap_or_default();
        let count = self
            .installed
            .iter()
            .filter(|t| t.id != primary_id)
            .count();
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') => self.mode = Mode::Normal,
            KeyCode::Down | KeyCode::Char('j') => {
                if count > 0 {
                    self.secondary_picker_cursor =
                        (self.secondary_picker_cursor + 1).min(count - 1);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.secondary_picker_cursor = self.secondary_picker_cursor.saturating_sub(1);
            }
            KeyCode::Enter => self.pick_secondary_at_cursor(),
            _ => {}
        }
    }

    fn pick_secondary_at_cursor(&mut self) {
        let primary_id = self
            .bible
            .as_ref()
            .map(|b| b.translation.id.clone())
            .unwrap_or_default();
        let candidates: Vec<String> = self
            .installed
            .iter()
            .filter(|t| t.id != primary_id)
            .map(|t| t.id.clone())
            .collect();
        let Some(id) = candidates.get(self.secondary_picker_cursor).cloned() else {
            return;
        };
        match Bible::load(&id) {
            Ok(b) => {
                self.secondary_bible = Some(Arc::new(b));
                self.secondary_id = Some(id.clone());
                self.parallel = true;
                self.mode = Mode::Normal;
                self.needs_clear = true;
                self.save_state();
                self.set_status(format!("parallel: {id} (| to close)"));
            }
            Err(e) => self.set_status(format!("load failed: {e}")),
        }
    }

    fn start_install(&mut self, id: &str) {
        if self.download.is_some() {
            self.set_status("a download is already in progress");
            return;
        }
        self.download = Some(DownloadState {
            id: id.to_string(),
            bytes: 0,
            total: None,
        });
        let id = id.to_string();
        let tx = self.event_tx.clone();
        thread::spawn(move || {
            let id_for_install = id.clone();
            let id_for_progress = id.clone();
            let tx_progress = tx.clone();
            // Catch any panic from the worker so the default panic handler
            // never gets a chance to write the panic message to stderr —
            // which would otherwise scroll the alternate-screen TUI and
            // push the header out of view.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                move || {
                    let mut progress = move |bytes: u64, total: Option<u64>| {
                        let _ = tx_progress.send(AppEvent::DownloadProgress {
                            id: id_for_progress.clone(),
                            bytes,
                            total,
                        });
                    };
                    crate::download::install(&id_for_install, Some(&mut progress))
                        .map(|b| b.translation)
                        .map_err(|e| e.to_string())
                },
            ));
            let result = match result {
                Ok(r) => r,
                Err(panic) => {
                    let msg = panic
                        .downcast_ref::<&str>()
                        .map(|s| (*s).to_string())
                        .or_else(|| panic.downcast_ref::<String>().cloned())
                        .unwrap_or_else(|| "panic during download".to_string());
                    Err(msg)
                }
            };
            let _ = tx.send(AppEvent::DownloadDone { id, result });
        });
        self.set_status("downloading…");
    }

    fn refresh_manifest_async(&mut self) {
        self.set_status("refreshing manifest…");
        match manifest::refresh() {
            Ok(m) => {
                self.available = m.translations;
                self.set_status(format!("{} translations cached", self.available.len()));
            }
            Err(e) => self.set_status(format!("refresh failed: {e}")),
        }
    }

    fn jump_to(&mut self, q: &str) {
        let q = q.trim();
        if q.is_empty() {
            return;
        }
        let mut moved = false;
        match crate::reference::parse(q) {
            Err(e) => self.set_status(format!("parse: {e}")),
            Ok(BibleReferenceRepresentation::Single(BibleReference::BibleVerse(vr))) => {
                if let Ok(cr) = BibleChapterReference::new(vr.book(), {
                    let c: u32 = vr.chapter().into();
                    match u8::try_from(c) {
                        Ok(v) => v,
                        Err(_) => return,
                    }
                }) {
                    self.push_history();
                    self.current = Some(cr);
                    let v_u32: u32 = vr.verse().into();
                    self.focus_verse = u16::try_from(v_u32).unwrap_or(1).max(1);
                    self.pin_focus = true;
                    moved = true;
                }
            }
            Ok(BibleReferenceRepresentation::Single(BibleReference::BibleChapter(cr))) => {
                self.push_history();
                self.current = Some(cr);
                self.focus_verse = 1;
                self.pin_focus = true;
                moved = true;
            }
            Ok(BibleReferenceRepresentation::Single(BibleReference::BibleBook(_))) => {
                if let Ok(cr) = nav::first_chapter_of_parsed(q) {
                    self.push_history();
                    self.current = Some(cr);
                    self.focus_verse = 1;
                    self.pin_focus = true;
                    moved = true;
                }
            }
            Ok(BibleReferenceRepresentation::Range(_)) => {
                self.set_status("ranges aren't supported in v1 — try a single verse")
            }
        }
        if moved {
            self.save_state();
        }
    }

    fn run_search(&mut self, q: &str) {
        let Some(bible) = self.bible.as_ref() else {
            return;
        };
        let q = q.trim().to_string();
        if q.is_empty() {
            return;
        }
        // Off-load the actual scan to a worker thread. With ~31k verses and
        // NFKD normalisation the main loop would otherwise stall for
        // hundreds of ms, freezing input and the spinner.
        let bible = Arc::clone(bible);
        let tx = self.event_tx.clone();
        let query_for_thread = q.clone();
        self.searching = Some(SearchingState {
            query: q.clone(),
            spinner: 0,
            started_at: Instant::now(),
        });
        self.status.clear();
        self.search_hits.clear();
        thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                bible
                    .search_substring(&query_for_thread)
                    .into_iter()
                    .map(|h| h.reference)
                    .collect::<Vec<_>>()
            }));
            let hits = result.unwrap_or_default();
            let _ = tx.send(AppEvent::SearchDone {
                query: query_for_thread,
                hits,
            });
        });
    }

    fn advance_search_hit(&mut self, dir: i32) {
        if self.search_hits.is_empty() {
            return;
        }
        let n = self.search_hits.len() as i32;
        let next = (((self.search_idx as i32 + dir) % n) + n) % n;
        self.search_idx = next as usize;
        self.go_to_current_hit();
        self.set_status(format!(
            "hit {}/{} for `{}`",
            self.search_idx + 1,
            self.search_hits.len(),
            self.last_search
        ));
    }

    fn go_to_current_hit(&mut self) {
        let Some(vr) = self.search_hits.get(self.search_idx).cloned() else {
            return;
        };
        let chap_u32: u32 = vr.chapter().into();
        let Ok(chap) = u8::try_from(chap_u32) else {
            return;
        };
        if let Ok(cr) = BibleChapterReference::new(vr.book(), chap) {
            self.push_history();
            self.current = Some(cr);
            let v_u32: u32 = vr.verse().into();
            self.focus_verse = u16::try_from(v_u32).unwrap_or(1).max(1);
            self.pin_focus = true;
            self.save_state();
        }
    }

    fn go_chapter(&mut self, dir: i32) {
        let Some(cur) = self.current.as_ref() else {
            return;
        };
        if let Some(next) = nav::shift_chapter(cur, dir) {
            self.push_history();
            self.current = Some(next);
            self.focus_verse = 1;
            self.pin_focus = true;
            self.save_state();
        }
    }

    fn go_book(&mut self, dir: i32) {
        let Some(cur) = self.current.as_ref() else {
            return;
        };
        if let Some(next) = nav::shift_book(cur, dir) {
            self.push_history();
            self.current = Some(next);
            self.focus_verse = 1;
            self.pin_focus = true;
            self.save_state();
        }
    }
}
