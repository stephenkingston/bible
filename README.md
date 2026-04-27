# bible

A beautiful TUI Bible reader, with translations downloaded on demand from the
[Beblia Holy-Bible-XML-Format](https://github.com/Beblia/Holy-Bible-XML-Format)
repository (200+ languages, 1000+ versions). No texts are embedded in the
binary.

Canonical reference handling (parsing `John 3:16`, validating chapters/verses,
multilingual references) is delegated to the
[`bibleref`](https://crates.io/crates/bibleref) crate.

## Install

```sh
cargo install bible
```

## Use as a TUI

```sh
bible                       # launches the TUI when stdout is a TTY
```

On first run there are no translations. Press `i` to install English KJV, or
`T` to browse the catalog. From the shell:

```sh
bible install kjv
```

### Keybindings (Reader)

| Key            | Action                                                            |
| -------------- | ----------------------------------------------------------------- |
| `j` / `k`      | scroll one line (in parallel view: advance verse anchor)          |
| `Ctrl-d/u`     | half-page scroll                                                  |
| `gg` / `G`     | top / bottom of chapter                                           |
| `h` / `l`      | previous / next chapter                                           |
| `H` / `L`      | previous / next book                                              |
| `:`            | jump to a reference (e.g. `:John 3:16`)                           |
| `/`, `n`, `N`  | search, next, previous match                                      |
| `t`            | cycle installed translations                                      |
| `T`            | open Translation Manager                                          |
| `b`            | bookmark current chapter (no prompt)                              |
| `B`            | open the bookmarks list                                           |
| `\|`           | toggle parallel view (opens chooser if no secondary picked)       |
| `\`            | re-open the secondary-translation chooser to swap                 |
| `,`            | open Settings (typography, theme, width cap, parallel divider)    |
| `?`            | help overlay                                                      |
| `q`            | quit                                                              |

`:b` extensions on the jump bar:

| Input                    | Action                                                |
| ------------------------ | ----------------------------------------------------- |
| `:b`                     | bookmark current chapter, no note                     |
| `:b 16`                  | bookmark verse 16 of current chapter                  |
| `:b a great verse`       | bookmark current chapter with note                    |
| `:b 16 a great verse`    | bookmark verse 16 with note                           |

### Bookmarks (`B`)

| Key          | Action                                          |
| ------------ | ----------------------------------------------- |
| `j` / `k`    | move selection                                  |
| `Enter`      | jump to bookmark (switches translation if needed) |
| `d`          | delete the highlighted bookmark                 |
| `Esc` / `q`  | close                                           |

### Translation Manager

| Key       | Action                            |
| --------- | --------------------------------- |
| `j` / `k` | move selection                    |
| `Enter`   | install or uninstall              |
| `r`       | refresh catalog from GitHub       |
| any text  | filter (id / name / language)     |
| `Esc`     | back to Reader                    |

### Settings (`,`)

A live-preview modal: the chapter pane stays visible on the left while you
adjust settings on the right.

| Key                     | Action                                |
| ----------------------- | ------------------------------------- |
| `j` / `k`               | move selection                        |
| `h` / `l` (or `←` / `→`)| decrement / increment value           |
| `Enter`                 | same as `l` (increment / cycle)       |
| `Esc` / `q`             | save and close                        |

What's tunable:

- **Typography** — word padding, verse spacing, line spacing, verse-number
  style (`inline-bold` / `superscript` / `hidden`).
- **Letter padding (per script)** — extra cells around each grapheme for
  Tamil, Devanagari, Arabic, Hebrew, CJK, plus a `default` for any other
  non-Latin script. Workaround for terminal fonts that overlap glyphs.
- **Theme** — `default`, `solarized-dark`, `high-contrast`.
- **Reader** — max-columns cap (centres the pane on wide terminals), default
  translation on startup.
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
bible list --available     # downloadable (after `bible refresh`)
bible install kjv
bible uninstall kjv
bible refresh              # pull the full catalog from GitHub once
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

## Where translations live

Paths come from
[`directories::ProjectDirs`](https://docs.rs/directories) — the standard data
and config locations for each platform:

| OS      | Data                                                       | Config                                    |
| ------- | ---------------------------------------------------------- | ----------------------------------------- |
| Linux   | `~/.local/share/bible/`                                    | `~/.config/bible/`                        |
| macOS   | `~/Library/Application Support/com.stephenkingston.bible/` | same as data                              |
| Windows | `%APPDATA%\stephenkingston\bible\data\`                    | `%APPDATA%\stephenkingston\bible\config\` |

Each installed translation lives under data at `translations/<id>/`, containing
`bible.bin` (bincode-serialised) and `meta.json`. Your reader state lives under
config:

| File                              | What it holds                              |
| --------------------------------- | ------------------------------------------ |
| `<config>/state.toml`             | last reading position + parallel-view pair |
| `<config>/bookmarks.toml`         | your bookmarks (chapter or verse + note)   |
| `<config>/settings.toml`          | typography, theme, width cap, divider      |
| `<config>/manifest.json`          | cached catalog from `bible refresh`        |

`state.toml`, `bookmarks.toml`, and `settings.toml` are all human-editable
TOML and are schema-versioned: on a version mismatch the file is ignored
(defaults are used; the file is never clobbered).

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
