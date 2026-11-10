# riffle

[![CI](https://github.com/Fel-2/riffle/actions/workflows/ci.yml/badge.svg)](https://github.com/Fel-2/riffle/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/Fel-2/riffle?display_name=tag&sort=semver)](https://github.com/Fel-2/riffle/releases)

A fast, good-looking terminal file manager written in Rust with
[ratatui](https://github.com/ratatui-org/ratatui).

## Features

- **Clean, focused layout** — breadcrumbs, sortable columns, a collapsible
  sidebar, a context-sensitive key hint bar and one-line status.
- **Previews that work everywhere** — syntax-highlighted text, image previews
  (including JPEG XL; kitty graphics, half-block fallback, optional chafa),
  directory contents, binary hex dumps and file metadata.
- **Fast by default** — directory listings, previews and `git status` all run
  off the UI thread, so large directories and network mounts never freeze the
  interface.
- **Fuzzy filtering** with match highlighting, plus sorting by name, size,
  date or type.
- **Tabs and dual pane** — independent navigation history per pane.
- **Real file operations** — copy/cut/paste with progress and cancellation,
  safe rename/create (never silently overwrites), trash with freedesktop
  metadata and undo, permanent delete with confirmation, archive
  extract/compress and KDE Connect sharing.
- **Git awareness** — branch name and per-file status (`M`, `A`, `D`, `?`, `U`)
  in the pane header and file list.
- **Command palette** — every action is searchable with `Ctrl+P`, so you do not
  have to memorise keys.
- **Themes** — Tokyo Night, Catppuccin, Gruvbox, Nord, Dracula, One Dark and a
  light theme, with per-color overrides and a transparent-background mode.
- **Mouse support** — click rows, double-click to open, click column headers to
  sort, click breadcrumbs/tabs/sidebar, scroll with the wheel.
- **Shell integration** — `riffle -p` prints the final directory so a shell
  function can `cd` there on exit.
- **Update check** — optionally checks GitHub once a day for a newer release and
  shows it in the status bar. Disable with `update_check = false`.

## Requirements

- A [Nerd Font](https://www.nerdfonts.com/) for the file-type icons (disable
  icons with `icons = false` in the config if you do not have one).
- Optional: `chafa` for the `chafa` image mode. The default `blocks` fallback
  needs nothing beyond a truecolor terminal.
- Optional: `git`, `zip`/`tar`/`unzip`/`7z`, `kdeconnect-cli` for the
  corresponding features.

## Installation

### Quick install

```bash
curl -fsSL https://raw.githubusercontent.com/Fel-2/riffle/main/install.sh | sh
```

This installs the latest release into `~/.local/bin`. Make sure that directory
is on your `PATH`. To pin a version or install system-wide:

```bash
curl -fsSL https://raw.githubusercontent.com/Fel-2/riffle/main/install.sh | VERSION=v0.2.1 sh
curl -fsSL https://raw.githubusercontent.com/Fel-2/riffle/main/install.sh | sudo PREFIX=/usr/local sh
```

The installer downloads the static musl build on x86_64 and the glibc build on
aarch64, and verifies the release checksum.

### From source

```bash
git clone https://github.com/Fel-2/riffle
cd riffle
cargo build --release
./target/release/riffle
```

JPEG XL previews are decoded by the pure-Rust `jxl-oxide` crate, which makes up
roughly 2 MB of the binary — the largest of any dependency. If you never preview
JPEG XL files, build without it:

```bash
cargo build --release --no-default-features
```

Prebuilt Linux binaries are attached to each
[GitHub release](https://github.com/Fel-2/riffle/releases). The
[Codeberg repository](https://codeberg.org/Fel/riffle) is a mirror.

Or install it into `~/.cargo/bin`:

```bash
cargo install --path .
```

## Usage

```text
riffle [OPTIONS] [PATH]

PATH    Directory to open, or a file to reveal
-p, --print-cwd    Print the final directory on exit
-h, --help         Print help
-V, --version      Print version
```

### Shell integration

Add this to your shell config so quitting riffle leaves you in the directory
you were browsing:

```sh
riffle() {
    local dir
    dir="$(command riffle -p "$@")" && cd "$dir"
}
```

## Keybindings

### Navigation

| Key | Action |
|-----|--------|
| `j` / `↓` | Move down |
| `k` / `↑` | Move up |
| `g` / `Home` | Top |
| `G` / `End` | Bottom |
| `PgUp` / `PgDn` | Page |
| `Ctrl+U` / `Ctrl+D` | Page up / down |
| `l` / `→` / `Enter` | Open file or enter directory |
| `h` / `←` / `Backspace` | Parent directory |
| `Alt+←` / `-` | Back in history |
| `Alt+→` | Forward in history |
| `~` | Home directory |
| `1` … `9` | Switch tab |
| `Tab` | Next pane (dual pane) or next section |
| `Shift+Tab` | Previous section |

### Files

| Key | Action |
|-----|--------|
| `Space` | Select and move down |
| `V` | Select all visible files |
| `Esc` | Clear selection and filter |
| `y` / `x` / `p` | Copy / cut / paste |
| `d` / `Del` | Move to trash |
| `D` | Delete permanently (confirmation) |
| `Ctrl+Z` | Undo last trash |
| `r` / `F2` | Rename (the stem is preselected) |
| `a` / `A` | New file / new directory |
| `e` | Open in `$EDITOR` |
| `i` | File info |
| `Y` | Copy path to the system clipboard (OSC 52) |
| `E` | Extract archive into a new directory |
| `Z` | Compress selection |
| `K` | Share via KDE Connect |

### View

| Key | Action |
|-----|--------|
| `/` | Fuzzy filter |
| `Ctrl+G` | Go to path (`~` and relative paths work) |
| `.` | Toggle hidden files |
| `s` / `S` | Next sort field / reverse order |
| `P` | Toggle preview pane |
| `B` | Toggle sidebar |
| `b` | Focus sidebar |
| `R` / `F5` | Refresh |
| `T` | Open terminal here |
| `:` | Run a shell command |
| `Ctrl+B` | Bookmark the current directory |

### Layout and app

| Key | Action |
|-----|--------|
| `Ctrl+W` | Toggle dual pane |
| `Tab` | Switch pane (dual) |
| `Ctrl+T` | New tab |
| `X` | Close tab |
| `[` / `]` | Previous / next tab |
| `Ctrl+P` | Command palette |
| `?` / `F1` | All keybindings |
| `q` / `Ctrl+C` | Quit |

### In the sidebar

`j`/`k` move, `g`/`G` jump, `Enter`/`l` open, `Esc`/`h`/`b` return to the
files, `Tab` moves to the next section.

### In the preview

`j`/`k` scroll, `PgUp`/`PgDn` page, `g`/`G` top/bottom, `h`/`l` pan long
lines, `Esc` returns to the files, `Tab` moves to the next section.

### Prompts

`Enter` apply, `Esc` cancel, arrows/Home/End move, `Ctrl+A` select all,
`Ctrl+U` clear, `Ctrl+W` delete a word, `Ctrl+K` delete to end.

### Mouse

- Click a row to select it, double-click to open.
- Click a column header to sort by it (click again to reverse).
- Click a breadcrumb, tab or sidebar entry to go there.
- Wheel over the list, sidebar or preview to scroll that section.

## Configuration

riffle writes a commented default config to `~/.config/riffle/config.toml` on
first run. Every option is optional; missing values use the defaults below.

```toml
[apps]
# Empty = $EDITOR / $VISUAL / nvim. Arguments are allowed.
editor = ""
# Empty = auto-detect. Arguments are allowed.
terminal = ""
# Per-type openers. Empty = xdg-open.
image = ""
video = ""
audio = ""
pdf = ""

[ui]
show_hidden = false
sort_by = "name"          # name, size, date, type
sort_ascending = true
preview = true            # old `preview_enabled` is still accepted
sidebar = true
sidebar_width = 22
preview_width = 45        # percentage of the body, 25-70
icons = true
git = true
mouse = true
date_format = "relative"  # relative, iso, short
image_mode = "auto"       # auto, kitty, blocks, chafa, none

[behavior]
fuzzy = true
print_cwd = false
shell = ""                # empty = $SHELL
confirm_trash = false
double_click_ms = 400
# Check GitHub for a newer release at most once a day.
update_check = true

[theme]
preset = "tokyo-night"    # tokyo-night, catppuccin, gruvbox, nord, dracula, one-dark, light
transparent = false
# bg = "#1a1b26"
# accent = "#7aa2f7"
# ... every theme color can be overridden with "#rrggbb" or "reset"
```

Bookmarks added with `Ctrl+B` are stored in `~/.config/riffle/bookmarks.toml`
and appear at the top of the sidebar. Bookmarks from `[[bookmarks]]` in
`config.toml` are merged in as well.

### Image preview modes

| Mode | Behaviour |
|------|-----------|
| `auto` | kitty graphics in kitty/Ghostty/WezTerm, half-blocks everywhere else |
| `kitty` | kitty graphics only |
| `blocks` | half-block rendering with truecolor (works in any terminal) |
| `chafa` | use the `chafa` command for higher-quality symbol art |
| `none` | show image metadata instead |

Half-block rendering needs no external programs and preserves the aspect ratio.
Kitty graphics give the sharpest result.

Image decoding runs on a background worker, so browsing a directory full of
large photos never blocks the interface. Images above 25 megapixels (or 64 MB)
are shown as metadata instead of being decoded, which prevents decompression
bombs and accidental freezes.

## Development

```bash
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

CI runs the same checks on GitHub Actions (`.github/workflows/ci.yml`). The
Codeberg mirror runs the equivalent Woodpecker pipeline (`.woodpecker.yml`).

## License

MIT
