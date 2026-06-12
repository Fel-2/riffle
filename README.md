# riffle

A fast terminal file manager written in Rust, built with [ratatui](https://github.com/ratatui-org/ratatui).

![Tokyo Night theme, image preview, git status indicators]

## Features

- **Tokyo Night** color scheme throughout
- **Nerd Font icons** for file types
- **Image preview** via kitty graphics protocol, with [chafa](https://hpjansson.org/chafa/) fallback for other terminals (PNG, JPEG, WebP — aspect ratio preserved)
- **Text preview** for code and document files
- **Git status indicators** (`M` modified, `A` added, `D` deleted, `?` untracked)
- **Dual pane** side-by-side view
- **Tabs** with full independent navigation per tab
- **Fuzzy search** / live filter
- **Symlink** detection and target display
- **Bookmarks** sidebar with common directories and mounted drives

## Requirements

- A [Nerd Font](https://www.nerdfonts.com/) in your terminal
- For image preview: [kitty](https://sw.kovidgoyal.net/kitty/) terminal, or `chafa` installed for any other terminal

## Installation

```bash
git clone https://codeberg.org/Fel/riffle
cd riffle
cargo build --release
./target/release/riffle
```

## Keybindings

### Navigation
| Key | Action |
|-----|--------|
| `j` / `↓` | Move down |
| `k` / `↑` | Move up |
| `h` / `←` / `Backspace` | Go to parent directory |
| `l` / `→` / `Enter` | Open file or enter directory |
| `g` / `G` | Jump to top / bottom |
| `PgDn` / `PgUp` | Jump 20 rows |
| `b` | Focus sidebar (then `j`/`k` move, `Enter` jump, `Esc` leave) |
| Mouse wheel | Scroll |

### File Operations
| Key | Action |
|-----|--------|
| `Space` | Select / deselect |
| `y` | Yank (copy) |
| `x` | Cut |
| `p` | Paste (runs in background, auto-renames on conflict, never overwrites) |
| `d` / `Del` | Move to trash (`~/.local/share/Trash`) |
| `D` | Delete permanently (asks for confirmation) |
| `Ctrl+Z` | Undo last trash (restores files) |
| `r` / `F2` | Rename (cursor starts before the extension) |
| `e` | Open in `$EDITOR` |
| `a` / `A` | New file / new directory (prompts for name) |
| `E` | Extract archive (zip, tar, 7z, rar) |
| `Z` | Compress to zip (falls back to tar.gz) |
| `K` | Share via KDE Connect |

### View
| Key | Action |
|-----|--------|
| `/` | Search / filter |
| `Ctrl+G` | Go to path |
| `.` | Toggle hidden files |
| `s` / `S` | Cycle sort field / toggle sort order |
| `P` | Toggle preview pane |
| `T` | Open terminal here |
| `Ctrl+B` | Bookmark current directory |
| `F5` / `R` | Refresh |

### Panes & Tabs
| Key | Action |
|-----|--------|
| `Ctrl+T` | New tab |
| `[` / `]` | Previous / next tab |
| `X` | Close current tab |
| `Ctrl+W` | Toggle dual pane |
| `Tab` | Switch pane focus |

### Other
| Key | Action |
|-----|--------|
| `:` | Shell command |
| `?` / `F1` | Help |
| `q` | Quit |

## Configuration

riffle generates a commented config file on first run at:

```
~/.config/riffle/config.toml
```

### Apps

```toml
[apps]
# Text editor for 'e' and text files. Falls back to $EDITOR, then nvim.
editor = ""

# Terminal emulator for 'T'. Empty = auto-detect.
terminal = ""

# Per-type openers. Empty = xdg-open.
image   = ""   # e.g. "imv", "feh"
video   = ""   # e.g. "mpv", "vlc"
audio   = ""   # e.g. "mpv", "cmus"
pdf     = ""   # e.g. "zathura", "evince"
```

### UI

```toml
[ui]
show_hidden     = false   # show dot-files on startup
sort_by         = "name"  # "name", "size", "date", "type"
sort_ascending  = true
preview_enabled = false   # open with preview pane visible
sidebar_width   = 20      # bookmark sidebar width in columns
```

### Theme

Every color can be overridden in the `[theme]` section with `"#rrggbb"` values.
Defaults are Tokyo Night.

```toml
[theme]
accent = "#7aa2f7"  # borders, headers
cyan   = "#7dcfff"  # directories
# bg, bg_panel, bg_highlight, bg_selection, fg, fg_bright, dim, muted,
# green, yellow, orange, red, purple — see generated config.toml for the full list
```

### Bookmarks

Custom bookmarks appear at the top of the sidebar above the auto-detected ones.

```toml
[[bookmarks]]
name = "Projects"
path = "/home/user/projects"

[[bookmarks]]
name = "NAS"
path = "/mnt/nas"
```

## Memory Usage

~4 MB RSS (release build). No GPU, no OpenGL, no Wayland compositor overhead.
