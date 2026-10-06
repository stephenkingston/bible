# bible

A beautiful, fast TUI Bible reader for the terminal — 200+ languages, 1000+
translations, all downloaded on demand from the
[Beblia Holy-Bible-XML-Format](https://github.com/Beblia/Holy-Bible-XML-Format)
repository. No texts are embedded in the binary.

![Screencast](https://raw.githubusercontent.com/stephenkingston/bible/main/screenshots/screencast.gif)

## Install

```sh
cargo install bible
```

Then:

```sh
bible
```

On first run the reader has no translations. Press `i` to install English KJV
straight away, or `T` to browse the full catalog. `?` shows every key at any
time, `Ctrl-K` opens the command palette, and `q` quits. You can also install
from the shell:

```sh
bible install kjv
```

## Screenshots

**Nocturne**, the default theme: borderless, with the focused verse marked in
the margin and a status line that always shows how to get help and quit.

![Nocturne](https://raw.githubusercontent.com/stephenkingston/bible/main/screenshots/reader.png)

**Vellum**: an illuminated manuscript, set as running prose with a drop-cap
chapter numeral and red verse numbers.

![Vellum](https://raw.githubusercontent.com/stephenkingston/bible/main/screenshots/vellum.png)

**Gilt**: black and gold, with your installed translations as tabs.

![Gilt](https://raw.githubusercontent.com/stephenkingston/bible/main/screenshots/gilt.png)

**Daylight**: a light, printed-page theme that sets the text in two columns
on wide terminals.

![Daylight](https://raw.githubusercontent.com/stephenkingston/bible/main/screenshots/daylight.png)

The command palette (`Ctrl-K`) takes a reference, a search or any command:

![Palette](https://raw.githubusercontent.com/stephenkingston/bible/main/screenshots/palette.png)

Every key on one card (`?`), sized to fit an 80×24 terminal:

![Keys](https://raw.githubusercontent.com/stephenkingston/bible/main/screenshots/help.png)

Search across the whole text, settings with a live preview, and the
Bible-in-a-Year plan:

![Search](https://raw.githubusercontent.com/stephenkingston/bible/main/screenshots/search.png)

![Settings](https://raw.githubusercontent.com/stephenkingston/bible/main/screenshots/settings.png)

![Year plan](https://raw.githubusercontent.com/stephenkingston/bible/main/screenshots/year-plan.png)

## Highlights

- **Seven themes**: Nocturne, Vellum, Gilt and Daylight, plus Classic,
  Solarized dark and High contrast. Switch from Settings (`,`) or the palette.
- **Prose or verse-per-line layout**, with drop-cap chapter numerals and
  two columns on wide terminals.
- **Command palette** (`Ctrl-K`): type `ps 23`, a search, or the name of any
  command, theme or translation.
- **Mouse**: the wheel moves through verses; click a verse to focus it.
- **Verse cursor** with `↑/↓`, viewport scrolls only when the cursor leaves
  the visible area; mild bg highlight is always on so you can't lose the
  focus.
- **Substring search** with `/`, jumps between hits with `n`/`N`, multilingual.
- **Reference jumps** with `:John 3:16` (also `Jn 3,16` and other forms,
  any installed language).
- **Bookmarks** with multi-line notes: `b` for chapter, `:b 16 note` opens
  a split-pane editor while you keep reading the chapter on the left.
- **Bible-in-a-Year reading plan** generated for the current year on launch;
  `p` jumps to today and marks it done; `P` opens the full-year list.
- **Parallel view** (`|`) with two installed translations side-by-side,
  scroll-locked by verse.
- **Settings modal** (`,`) with live preview: theme, layout, columns,
  reading width, typography, per-script letter padding.
- **Bookmarks, settings, reading state, plan progress** all in
  human-editable TOML under your platform's config directory.

Canonical reference handling (parsing `John 3:16`, validating
chapters/verses, multilingual references) is delegated to the
[`bibleref`](https://crates.io/crates/bibleref) crate.

## Use as a TUI

```sh
bible                       # launches the TUI when stdout is a TTY
```

### Keybindings (Reader)

| Key                  | Action                                                            |
| -------------------- | ----------------------------------------------------------------- |
| `↑` / `↓`            | move verse cursor (focused verse highlighted, viewport follows)   |
| `←` / `→`            | previous / next chapter                                           |
| `Shift+←` / `Shift+→`| previous / next book                                              |
| `PgUp` / `PgDn`      | jump 5 verses                                                     |
| `Home` / `End`       | first / last verse of chapter                                     |
| `:`                  | jump to a reference (e.g. `:John 3:16`)                           |
| `/`, `n`, `N`        | search, next, previous match                                      |
| `Ctrl-O` / `Tab`     | back / forward through reference history (browser-style)          |
| `t`                  | cycle installed translations                                      |
| `T`                  | open Translation Manager                                          |
| `b`                  | bookmark current chapter (no prompt)                              |
| `B`                  | open the bookmarks list                                           |
| `y`                  | copy focused verse to clipboard                                   |
| `p`                  | jump to today's Bible-in-a-Year reading (auto-marks done)         |
| `P`                  | open the Bible-in-a-Year plan view                                |
| `\|`                 | toggle parallel view (opens chooser if no secondary picked)       |
| `\`                  | re-open the secondary-translation chooser to swap                 |
| `,`                  | open Settings (theme, layout, typography, width, parallel divider)|
| `Ctrl-K`             | command palette                                                   |
| `?`                  | keys card                                                         |
| `Esc`                | clear the search highlight                                        |
| `q` or `:q`          | quit                                                              |

The status line always ends with `? keys` and `q quit`. With the mouse on
(the default), the wheel moves the verse cursor and clicking a verse focuses
it; turn it off in Settings to get your terminal's own text selection back.

Vim-style fallbacks (`hjkl`, `gg`/`G`, `H`/`L`, `Ctrl-d`/`Ctrl-u`) are also
wired up silently for muscle memory; the arrow keys above are the
documented surface.

`:b` extensions on the jump bar:

| Input          | Action                                                                 |
| -------------- | ---------------------------------------------------------------------- |
| `:b`           | bookmark current chapter, no note                                      |
| `:b 16`        | bookmark verse 16 of current chapter, no note                          |
| `:b note`      | bookmark current chapter, opens a multi-line note editor               |
| `:b 16 note`   | bookmark verse 16, opens a multi-line note editor                      |

The note editor is a split-pane view: chapter on the left, editor on
the right. `Tab` toggles which pane has the keyboard, so you can scroll
the chapter for context while writing.

| Key             | Action                                                   |
| --------------- | -------------------------------------------------------- |
| `Ctrl-S`        | save and close                                           |
| `Esc`           | cancel (discards a not-yet-saved bookmark; reverts an edit) |
| `Tab`           | toggle focus between editor and reader                   |
| In editor pane  | `Enter` newline, arrows / Home / End / PgUp / PgDn move  |
| In reader pane  | `↑/↓` verse, `←/→` chapter, `Shift+←/→` book             |

`:y` extensions for copying to the clipboard:

| Input        | Action                                                  |
| ------------ | ------------------------------------------------------- |
| `y` (key)    | copy the focused verse (search hit, else last jump)     |
| `:y`         | same as `y`                                             |
| `:y 16`      | copy verse 16 of the current chapter                    |
| `:y 1-12`    | copy verses 1 through 12                                |
| `:y all`     | copy the entire chapter                                 |

The clipboard payload is the verse text plus an attribution line:
`"<text>\n\n— Book Chap:Verse (Translation)"`.

### Command palette (`Ctrl-K`)

One box for everything. Type a reference (`ps 23`, `jn 3:16`) and the first
result goes there; type anything else and you get a search for it, along with
every command, theme, layout and installed translation whose name matches.
`↑`/`↓` choose, `Enter` runs, `Esc` closes.

### Themes

| Theme            | Look                                                              |
| ---------------- | ----------------------------------------------------------------- |
| `nocturne`       | the default: dark, borderless, periwinkle accents                 |
| `vellum`         | illuminated manuscript: umber, rubricated verse numbers, prose    |
| `gilt`           | black and gold, rounded frame, translation tabs                   |
| `daylight`       | paper and ink, prose in two columns on wide terminals             |
| `classic`        | the original bordered look in your terminal's colours             |
| `solarized-dark` | Solarized accents                                                 |
| `high-contrast`  | white on black                                                    |

Themes use 24-bit colour where the terminal supports it (iTerm2, Ghostty,
kitty, WezTerm, and most terminals that set `COLORTERM=truecolor`) and fall
back to the nearest 256-colour shades elsewhere, such as macOS Terminal.
Set `BIBLE_TRUECOLOR=1` or `0` to override the detection.

### Tamil

Terminals start every glyph on a cell boundary, but ordinary Tamil glyphs
are fractions of a cell wide, so no layout can space Tamil evenly in a
normal terminal font: each syllable either crowds its neighbour or leaves
a gap. `bible` measures at startup how your terminal sizes syllables with
vowel signs and gives each syllable room for its glyph, so text never
collides, but spacing within words is uneven.

### Bookmarks (`B`)

| Key            | Action                                              |
| -------------- | --------------------------------------------------- |
| `↑` / `↓`      | move selection                                      |
| `Enter`        | jump to bookmark (switches translation if needed)   |
| `e`            | edit the highlighted bookmark's note (multi-line)   |
| `PgUp` / `PgDn`| scroll the note-preview pane                        |
| `d`            | delete the highlighted bookmark                     |
| `Esc` / `q`    | close                                               |

The list shows the first line of each note plus a `(N lines)` badge if
there's more; the full note of the selected bookmark renders in a
preview pane below the list.

### Translation Manager

The filter is always focused — every letter you type feeds it, so
names like "James" or "Jeremiah" filter cleanly. Commands use arrow
keys plus modifier-keyed letters.

| Key             | Action                            |
| --------------- | --------------------------------- |
| any text        | filter (id / name / language)     |
| `↑` / `↓`       | move selection                    |
| `PgUp` / `PgDn` | jump 10 entries                   |
| `Enter`         | install or uninstall              |
| `Ctrl-R`        | refresh catalog from GitHub       |
| `Esc`           | back to Reader                    |

### Reading plan (`p` / `P`)

A "Bible in a Year" plan is generated for the current year on launch —
1189 chapters split across 365 (or 366) days in canonical order. The
reader's header always shows today's reading, e.g. `today  Gen 14-15, Exo 1  ○`
(the circle fills in once the day is done).

| Key in reader  | Action                                                            |
| -------------- | ----------------------------------------------------------------- |
| `p`            | jump to today's reading (first chapter) and auto-mark it complete |
| `P`            | open the full-year plan view                                      |

In the plan view:

| Key            | Action                                                            |
| -------------- | ----------------------------------------------------------------- |
| `↑` / `↓`      | move selection                                                    |
| `PgUp` / `PgDn`| jump 10 days                                                      |
| `g` / `G`      | first / last day                                                  |
| `t`            | recentre on today                                                 |
| `Enter`        | jump to that day's reading and mark it complete                   |
| `m`            | toggle complete / incomplete on the highlighted day               |
| `Esc` / `q`    | back to reader                                                    |

Progress lives in `<config>/plan.toml`. The plan content itself is
deterministic from the year alone, so only progress is persisted; on
year change (Jan 1) the file is regenerated and last year's progress
is discarded.

### Settings (`,`)

A live-preview modal: the chapter pane stays visible on the left while you
adjust settings on the right.

| Key             | Action                                |
| --------------- | ------------------------------------- |
| `↑` / `↓`       | move selection                        |
| `←` / `→`       | change option (decrement / increment) |
| `Enter`         | same as `→` (cycle forward)           |
| `Esc` / `q`     | save and close                        |

What's tunable:

- **Typography** — justify text (on by default), word padding, verse
  spacing, line spacing, verse-number style (`inline-bold` /
  `superscript` / `hidden`).
- **Letter padding (per script)** — extra cells after each grapheme for
  Tamil, Devanagari, Arabic, Hebrew, CJK, plus a `default` for any other
  non-Latin script, for terminal fonts whose glyphs still overlap. Tamil
  already gets room for each syllable's glyph, and the reader measures at
  startup how your terminal sizes syllables with vowel signs, so text lines
  up whichever way it does.
- **Theme**: any of the seven above.
- **Reading**: layout (`auto` follows the theme, or `verse per line` /
  `prose`), columns (`auto` / `one` / `two`), drop cap, reading width
  (`auto`, a fixed width, or `full width`), default translation on startup,
  mouse.
- **Parallel** — divider style between panes (`single` / `double` / `none`).

Settings are saved to `<config_dir>/settings.toml` on close. The file is
human-editable TOML and schema-versioned.

### Resume

The reader remembers your last position (translation, book, chapter, scroll, and
parallel-view state) and restores it on the next launch. Stored in
`<config_dir>/state.toml`.

## Use as a CLI

When stdout is not a TTY (piped/redirected), or when a subcommand is given,
`bible` runs headlessly:

```sh
bible read "John 3:16"
bible read "Acts 2"
bible search "Jesus"
bible list                 # installed
bible list --available     # downloadable (fetches the catalog on first use)
bible install kjv
bible uninstall kjv
bible refresh              # re-fetch the catalog from GitHub
```

Pass `--translation <id>` to target a specific translation when several are
installed.

## Use as a library

```rust
use bible::{Bible, reference};

fn main() -> bible::Result<()> {
    let bible = Bible::load("EnglishKJBible")?;

    // bibleref handles parsing of `John 3:16`, `Jn 3,16`, etc.
    let parsed = reference::parse("John 3:16")?;

    // Search the whole text (case-insensitive, diacritic-folded).
    let hits = bible.search_substring("love");
    println!("{} hits", hits.len());

    Ok(())
}
```

## Where your data lives

`bible` keeps everything in two directories chosen by
[`directories::ProjectDirs`](https://docs.rs/directories) — a **data**
directory for the heavy bible binaries, and a **config** directory for
your light, hand-editable preferences and reading state. Nothing is
written outside these two directories.

### Platform paths

| OS      | Data directory                                             | Config directory                          |
| ------- | ---------------------------------------------------------- | ----------------------------------------- |
| Linux   | `~/.local/share/bible/`                                    | `~/.config/bible/`                        |
| macOS   | `~/Library/Application Support/com.stephenkingston.bible/` | same as data                              |
| Windows | `%APPDATA%\stephenkingston\bible\data\`                    | `%APPDATA%\stephenkingston\bible\config\` |

### What's in each

**Bible translations** — `<data>/translations/<id>/`, one folder per
installed translation. Each contains:

- `bible.bin` — the parsed Bible serialised with `bincode`
- `meta.json` — translation id, display name, language

**Settings, bookmarks, and reading state** — all under `<config>/`,
hand-editable TOML, schema-versioned (on version mismatch the file is
ignored and defaults are used; the file is never clobbered):

| File              | Source of truth for                                            |
| ----------------- | -------------------------------------------------------------- |
| `settings.toml`   | theme, reading layout, columns, width, mouse, typography, parallel divider |
| `bookmarks.toml`  | every bookmark you've made — chapter / verse + multi-line note |
| `state.toml`      | last reading position (translation, book, chapter, focus verse) and your parallel-view pair, restored on next launch |
| `plan.toml`       | reading-plan progress for the current year (which days you've completed) |
| `manifest.json`   | cached catalog of available translations, fetched on first use or by `bible refresh` |

**Not persisted** — command history (`:` and `/` recall via `↑`/`↓`),
back/forward navigation history (`Ctrl-O` / `Tab`), and search results
all live in memory only and reset between launches.

## Limitations

- v1 supports the 66-book Protestant canon only. Apocryphal / deuterocanonical
  books shipped by some Beblia translations are dropped on import with a
  warning. Hybrid canon support is a planned follow-up.
- Reference *ranges* (`John 3:16-18`) parse but aren't fully wired into the
  reader yet.

## Credit

Bible XML files come from
[Holy Bible XML Format](https://github.com/Beblia/Holy-Bible-XML-Format) by
Andrey at Beblia. Reference parsing uses the
[`bibleref`](https://crates.io/crates/bibleref) crate.

## Licence

GPL-2.0-or-later.
