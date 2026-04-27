//! Inline multi-line text editor for bookmark notes. Stays small enough
//! to live in the same crate without pulling tui-textarea — most of the
//! editing surface is just per-line String mutation by char index.

#[derive(Debug, Clone)]
pub(crate) struct NoteEditor {
    /// One String per line. Always non-empty (at least `[String::new()]`).
    pub lines: Vec<String>,
    /// Cursor row (0-indexed line number).
    pub cursor_line: usize,
    /// Cursor column, in **char** positions (not bytes, not graphemes).
    /// Notes are mostly Latin and char-level cursoring is good enough.
    pub cursor_col: usize,
    /// Top-of-viewport line index for the editor pane's vertical scroll.
    pub scroll: u16,
    pub target: NoteEditorTarget,
    /// Shown in the title bar, e.g. "John 3:16 (English King James Bible)".
    pub label: String,
}

#[derive(Debug, Clone)]
pub(crate) enum NoteEditorTarget {
    /// Save creates a brand-new bookmark with these fields and the editor's
    /// note text. Cancel discards.
    NewBookmark {
        translation: String,
        book_number: u8,
        chapter: u16,
        verse: Option<u16>,
    },
    /// Save overwrites `bookmarks[index].note`. Cancel reverts.
    EditExisting { index: usize },
}

impl NoteEditor {
    pub fn new(target: NoteEditorTarget, label: String, initial: &str) -> Self {
        let lines: Vec<String> = if initial.is_empty() {
            vec![String::new()]
        } else {
            initial.split('\n').map(String::from).collect()
        };
        Self {
            lines,
            cursor_line: 0,
            cursor_col: 0,
            scroll: 0,
            target,
            label,
        }
    }

    /// Concatenate lines into a single newline-separated note string.
    pub fn current_text(&self) -> String {
        self.lines.join("\n")
    }

    pub fn insert_char(&mut self, c: char) {
        let line = &mut self.lines[self.cursor_line];
        let byte = char_idx_to_byte(line, self.cursor_col);
        line.insert(byte, c);
        self.cursor_col += 1;
    }

    pub fn insert_newline(&mut self) {
        let line = &mut self.lines[self.cursor_line];
        let byte = char_idx_to_byte(line, self.cursor_col);
        let rest = line[byte..].to_string();
        line.truncate(byte);
        self.lines.insert(self.cursor_line + 1, rest);
        self.cursor_line += 1;
        self.cursor_col = 0;
    }

    pub fn backspace(&mut self) {
        if self.cursor_col > 0 {
            let line = &mut self.lines[self.cursor_line];
            let end = char_idx_to_byte(line, self.cursor_col);
            let start = char_idx_to_byte(line, self.cursor_col - 1);
            line.drain(start..end);
            self.cursor_col -= 1;
        } else if self.cursor_line > 0 {
            // Merge into previous line.
            let dropped = self.lines.remove(self.cursor_line);
            self.cursor_line -= 1;
            let cur = &mut self.lines[self.cursor_line];
            self.cursor_col = cur.chars().count();
            cur.push_str(&dropped);
        }
    }

    pub fn delete_forward(&mut self) {
        let line_chars = self.lines[self.cursor_line].chars().count();
        if self.cursor_col < line_chars {
            let line = &mut self.lines[self.cursor_line];
            let start = char_idx_to_byte(line, self.cursor_col);
            let end = char_idx_to_byte(line, self.cursor_col + 1);
            line.drain(start..end);
        } else if self.cursor_line + 1 < self.lines.len() {
            // Merge next line in.
            let dropped = self.lines.remove(self.cursor_line + 1);
            self.lines[self.cursor_line].push_str(&dropped);
        }
    }

    pub fn move_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.cursor_col = self.lines[self.cursor_line].chars().count();
        }
    }

    pub fn move_right(&mut self) {
        let line_chars = self.lines[self.cursor_line].chars().count();
        if self.cursor_col < line_chars {
            self.cursor_col += 1;
        } else if self.cursor_line + 1 < self.lines.len() {
            self.cursor_line += 1;
            self.cursor_col = 0;
        }
    }

    pub fn move_up(&mut self) {
        if self.cursor_line > 0 {
            self.cursor_line -= 1;
            let line_chars = self.lines[self.cursor_line].chars().count();
            self.cursor_col = self.cursor_col.min(line_chars);
        }
    }

    pub fn move_down(&mut self) {
        if self.cursor_line + 1 < self.lines.len() {
            self.cursor_line += 1;
            let line_chars = self.lines[self.cursor_line].chars().count();
            self.cursor_col = self.cursor_col.min(line_chars);
        }
    }

    pub fn move_home(&mut self) {
        self.cursor_col = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor_col = self.lines[self.cursor_line].chars().count();
    }

    pub fn page_down(&mut self, n: usize) {
        for _ in 0..n {
            self.move_down();
        }
    }

    pub fn page_up(&mut self, n: usize) {
        for _ in 0..n {
            self.move_up();
        }
    }
}

fn char_idx_to_byte(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(b, _)| b)
        .unwrap_or(s.len())
}
