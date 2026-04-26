//! Terminal UI: Reader + Translation Manager.
//!
//! Single-file by design — keeps the event loop, state, and drawing in one
//! place where they can be read top-to-bottom. Cross-platform via
//! `crossterm`. Downloads run on a worker thread that pushes progress
//! through an `mpsc::Sender<AppEvent>` consumed by the main loop.

mod draw;
mod nav;

use std::io::{self, Stdout};
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Normal,
    Jump,
    Search,
    Manager,
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
    pub bible: Option<Bible>,
    pub installed: Vec<TranslationInfo>,
    pub available: Vec<AvailableTranslation>,
    pub current: Option<BibleChapterReference>,
    pub scroll: u16,
    pub mode: Mode,
    pub input: Input,
    pub status: String,
    pub status_at: Instant,

    pub search_hits: Vec<BibleVerseReference>,
    pub search_idx: usize,
    pub last_search: String,

    pub pending_g: bool,

    pub manager_filter: Input,
    pub manager_cursor: usize,

    pub download: Option<DownloadState>,
    pub event_tx: Sender<AppEvent>,

    /// Vim-style command history for the `:` jump bar.
    /// Newest entry is last. `_idx` points at the entry currently shown in
    /// the input; `None` means the user is composing a fresh entry.
    pub jump_history: Vec<String>,
    pub jump_history_idx: Option<usize>,
    pub search_history: Vec<String>,
    pub search_history_idx: Option<usize>,
}

pub fn run(initial_translation: Option<String>) -> Result<()> {
    install_panic_hook();
    // Suppress library-side `eprintln!` so stderr writes don't scroll the
    // alternate-screen TUI display and visually push the header off-screen.
    crate::set_quiet(true);
    let mut terminal = setup_terminal()?;
    let result = run_app(&mut terminal, initial_translation);
    let _ = teardown_terminal();
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
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = teardown_terminal();
        original(info);
    }));
}

fn run_app(terminal: &mut Tui, initial_translation: Option<String>) -> Result<()> {
    let (tx, rx) = mpsc::channel::<AppEvent>();
    spawn_input_thread(tx.clone());
    spawn_tick_thread(tx.clone());

    let mut app = App::new(tx.clone())?;
    app.choose_initial_translation(initial_translation)?;

    while app.mode != Mode::Quit {
        terminal.draw(|f| draw::draw(f, &app))?;
        match rx.recv_timeout(Duration::from_millis(250)) {
            Ok(ev) => app.handle_event(ev)?,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
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

fn spawn_tick_thread(tx: Sender<AppEvent>) {
    thread::spawn(move || {
        while tx.send(AppEvent::Tick).is_ok() {
            thread::sleep(Duration::from_millis(500));
        }
    });
}

impl App {
    fn new(event_tx: Sender<AppEvent>) -> Result<Self> {
        let installed = storage::list_installed().unwrap_or_default();
        let available = manifest::list_available();
        Ok(Self {
            bible: None,
            installed,
            available,
            current: None,
            scroll: 0,
            mode: Mode::Normal,
            input: Input::default(),
            status: String::new(),
            status_at: Instant::now(),
            search_hits: Vec::new(),
            search_idx: 0,
            last_search: String::new(),
            pending_g: false,
            manager_filter: Input::default(),
            manager_cursor: 0,
            download: None,
            event_tx,
            jump_history: Vec::new(),
            jump_history_idx: None,
            search_history: Vec::new(),
            search_history_idx: None,
        })
    }

    fn choose_initial_translation(&mut self, requested: Option<String>) -> Result<()> {
        if self.installed.is_empty() {
            self.mode = Mode::NoTranslation;
            self.set_status("No translations installed. Press `i` to install KJV, or `T` to browse.");
            return Ok(());
        }
        let id = if let Some(req) = requested {
            manifest::resolve_id(&req).unwrap_or(self.installed[0].id.clone())
        } else {
            self.installed[0].id.clone()
        };
        self.load_translation(&id)
    }

    pub(crate) fn load_translation(&mut self, id: &str) -> Result<()> {
        let bible = Bible::load(id)?;
        let first_book = bible
            .books
            .first()
            .and_then(|b| book_from_number(b.book_number).ok())
            .unwrap_or_else(|| get_bible_book_by_number(1).expect("Genesis"));
        self.current = BibleChapterReference::new(first_book, 1).ok();
        self.bible = Some(bible);
        self.scroll = 0;
        self.mode = Mode::Normal;
        Ok(())
    }

    fn set_status(&mut self, s: impl Into<String>) {
        self.status = s.into();
        self.status_at = Instant::now();
    }

    fn handle_event(&mut self, ev: AppEvent) -> Result<()> {
        match ev {
            AppEvent::Key(k) => self.handle_key(k)?,
            AppEvent::Tick => {
                if !self.status.is_empty() && self.status_at.elapsed() > Duration::from_secs(5) {
                    self.status.clear();
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
            KeyCode::Char('j') | KeyCode::Down => self.scroll = self.scroll.saturating_add(1),
            KeyCode::Char('k') | KeyCode::Up => self.scroll = self.scroll.saturating_sub(1),
            KeyCode::Char('d') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                self.scroll = self.scroll.saturating_add(10)
            }
            KeyCode::Char('u') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                self.scroll = self.scroll.saturating_sub(10)
            }
            KeyCode::Char('h') if modless => self.go_chapter(-1),
            KeyCode::Char('l') if modless => self.go_chapter(1),
            KeyCode::Char('H') => self.go_book(-1),
            KeyCode::Char('L') => self.go_book(1),
            KeyCode::Char('g') if modless => {
                if self.pending_g {
                    self.scroll = 0;
                    self.pending_g = false;
                } else {
                    self.pending_g = true;
                }
                return Ok(());
            }
            KeyCode::Char('G') => self.scroll = u16::MAX / 2,
            KeyCode::Char('n') if modless => self.advance_search_hit(1),
            KeyCode::Char('N') => self.advance_search_hit(-1),
            _ => {}
        }
        if !matches!(k.code, KeyCode::Char('g')) {
            self.pending_g = false;
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
                self.jump_to(&q);
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
        // While the filter input is active (any printable key), feed it.
        // Toggle list-vs-filter focus with Tab.
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.mode = if self.bible.is_some() {
                    Mode::Normal
                } else {
                    Mode::NoTranslation
                };
            }
            KeyCode::Down | KeyCode::Char('j') if k.modifiers.is_empty() => {
                let n = self.filtered_indices().len();
                if n > 0 {
                    self.manager_cursor = (self.manager_cursor + 1).min(n - 1);
                }
            }
            KeyCode::Up | KeyCode::Char('k') if k.modifiers.is_empty() => {
                self.manager_cursor = self.manager_cursor.saturating_sub(1);
            }
            KeyCode::Enter => self.toggle_install_at_cursor(),
            KeyCode::Char('r') if k.modifiers.is_empty() => self.refresh_manifest_async(),
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
        match self.load_translation(&id) {
            Ok(()) => self.set_status(format!("switched to {id}")),
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
            let id_for_progress = id.clone();
            let tx_progress = tx.clone();
            let mut progress = move |bytes: u64, total: Option<u64>| {
                let _ = tx_progress.send(AppEvent::DownloadProgress {
                    id: id_for_progress.clone(),
                    bytes,
                    total,
                });
            };
            let result = crate::download::install(&id, Some(&mut progress))
                .map(|b| b.translation)
                .map_err(|e| e.to_string());
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
                    self.current = Some(cr);
                    self.scroll = nav::verse_scroll(&vr);
                }
            }
            Ok(BibleReferenceRepresentation::Single(BibleReference::BibleChapter(cr))) => {
                self.current = Some(cr);
                self.scroll = 0;
            }
            Ok(BibleReferenceRepresentation::Single(BibleReference::BibleBook(_))) => {
                if let Ok(cr) = nav::first_chapter_of_parsed(q) {
                    self.current = Some(cr);
                    self.scroll = 0;
                }
            }
            Ok(BibleReferenceRepresentation::Range(_)) => {
                self.set_status("ranges aren't supported in v1 — try a single verse")
            }
        }
    }

    fn run_search(&mut self, q: &str) {
        let Some(bible) = self.bible.as_ref() else {
            return;
        };
        let q = q.trim();
        if q.is_empty() {
            return;
        }
        let hits = bible.search_substring(q);
        if hits.is_empty() {
            self.set_status(format!("no matches for `{q}`"));
            self.search_hits.clear();
            return;
        }
        self.search_hits = hits.into_iter().map(|h| h.reference).collect();
        self.search_idx = 0;
        self.last_search = q.to_string();
        self.set_status(format!("{} hits for `{q}`", self.search_hits.len()));
        self.go_to_current_hit();
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
            self.current = Some(cr);
            self.scroll = nav::verse_scroll(&vr);
        }
    }

    fn go_chapter(&mut self, dir: i32) {
        let Some(cur) = self.current.as_ref() else {
            return;
        };
        if let Some(next) = nav::shift_chapter(cur, dir) {
            self.current = Some(next);
            self.scroll = 0;
        }
    }

    fn go_book(&mut self, dir: i32) {
        let Some(cur) = self.current.as_ref() else {
            return;
        };
        if let Some(next) = nav::shift_book(cur, dir) {
            self.current = Some(next);
            self.scroll = 0;
        }
    }
}
