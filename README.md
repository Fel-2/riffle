# riffle

A fast terminal file manager written in Rust, built with [ratatui](https://github.com/ratatui-org/ratatui).

![Tokyo Night theme, image preview, git status indicators]

## Features

- **Tokyo Night** color scheme throughout
- **Nerd Font icons** for file types
- **Image preview** via kitty graphics protocol (PNG, JPEG, WebP — aspect ratio preserved)
- **Text preview** for code and document files
- **Git status indicators** (`M` modified, `A` added, `D` deleted, `?` untracked)
- **Dual pane** side-by-side view
- **Tabs** with full independent navigation per tab
- **Fuzzy search** / live filter
- **Symlink** detection and target display
- **Bookmarks** sidebar with common directories and mounted drives

## Requirements

- A [Nerd Font](https://www.nerdfonts.com/) in your terminal
- [kitty](https://sw.kovidgoyal.net/kitty/) terminal for image preview

## Installation

```bash
git clone https://github.com/yourname/riffle
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
| Mouse wheel | Scroll |

### File Operations
| Key | Action |
|-----|--------|
| `Space` | Select / deselect |
| `y` | Yank (copy) |
| `x` | Cut |
| `p` | Paste |
| `d` / `Del` | Delete permanently |
| `D` | Move to trash (`~/.local/share/Trash`) |
| `r` / `F2` | Rename |
| `e` | Open in `$EDITOR` |
| `a` / `A` | New file / new directory |
| `E` | Extract archive (zip, tar, 7z, rar) |

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

## Memory Usage

~4 MB RSS (release build). No GPU, no OpenGL, no Wayland compositor overhead.
