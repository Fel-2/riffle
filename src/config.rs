use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// User configuration loaded from ~/.config/riffle/config.toml.
/// All fields have sensible defaults so a missing or partial file is fine.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct Config {
    pub apps:      AppsConfig,
    pub ui:        UiConfig,
    pub theme:     crate::theme::ThemeConfig,
    pub bookmarks: Vec<BookmarkEntry>,
}

/// Which program to use for each file category.
/// Empty string means fall back to xdg-open (or $EDITOR for text).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct AppsConfig {
    /// Text editor — also overrides $EDITOR / $VISUAL.
    pub editor:   String,
    /// Terminal emulator opened with `T`. Empty = auto-detect.
    pub terminal: String,
    /// Image viewer. Empty = xdg-open.
    pub image:    String,
    /// Video player. Empty = xdg-open.
    pub video:    String,
    /// Audio player. Empty = xdg-open.
    pub audio:    String,
    /// PDF viewer. Empty = xdg-open.
    pub pdf:      String,
    /// Archive tool for extraction. Empty = auto-detect (tar/unzip/7z).
    pub archive:  String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    /// Show hidden files (dot-files) on startup.
    pub show_hidden:     bool,
    /// Sort field on startup: "name", "size", "date", "type".
    pub sort_by:         String,
    /// Sort ascending on startup.
    pub sort_ascending:  bool,
    /// Open with preview pane enabled on startup.
    pub preview_enabled: bool,
    /// Width in columns of the left sidebar.
    pub sidebar_width:   u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmarkEntry {
    pub name: String,
    pub path: String,
}

// ── Default implementations ──────────────────────────────────────────────────



impl Default for UiConfig {
    fn default() -> Self {
        Self {
            show_hidden:     false,
            sort_by:         "name".into(),
            sort_ascending:  true,
            preview_enabled: false,
            sidebar_width:   20,
        }
    }
}

// ── Path helper ───────────────────────────────────────────────────────────────

pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("riffle")
        .join("config.toml")
}

// ── Load / save ───────────────────────────────────────────────────────────────

/// Load config from disk, generating a default file if none exists.
pub fn load() -> Config {
    let path = config_path();

    if !path.exists() {
        // Write a commented default config so the user has a reference
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&path, default_config_toml());
        return Config::default();
    }

    let raw = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return Config::default(),
    };

    toml::from_str(&raw).unwrap_or_else(|_| Config::default())
}

/// The commented default config written on first run.
fn default_config_toml() -> &'static str {
    r##"# riffle configuration
# Located at ~/.config/riffle/config.toml
#
# All values shown are the built-in defaults.
# Remove the leading # to activate a line.

[apps]
# Text editor used for 'e' and when opening text/code files.
# Falls back to $EDITOR, then $VISUAL, then nvim.
editor = ""

# Terminal emulator opened with 'T'.
# Leave empty to auto-detect (konsole, gnome-terminal, alacritty, kitty, xfce4-terminal, xterm).
terminal = ""

# Per-type openers. Leave empty to use xdg-open.
image   = ""   # e.g. "imv", "feh", "eog"
video   = ""   # e.g. "mpv", "vlc"
audio   = ""   # e.g. "mpv", "cmus"
pdf     = ""   # e.g. "zathura", "evince", "okular"
archive = ""   # extraction tool; empty = auto (tar/unzip/7z)

[ui]
# Start with hidden files visible.
show_hidden = false

# Initial sort field: "name", "size", "date", "type"
sort_by = "name"

# Sort direction: true = ascending (A→Z, small→large).
sort_ascending = true

# Open with preview pane already visible.
preview_enabled = false

# Width (in columns) of the bookmarks/drives sidebar.
sidebar_width = 20

# [theme]
# Override any color with "#rrggbb". Defaults are Tokyo Night.
# bg           = "#1a1b26"   # main background
# bg_panel     = "#1f2335"   # bars, sidebar
# bg_highlight = "#292e42"   # cursor row, inactive borders
# bg_selection = "#364a82"   # selected entries
# fg           = "#a9b1d6"   # normal text
# fg_bright    = "#c0caf5"   # emphasized text
# dim          = "#565f89"   # hidden files, secondary info
# muted        = "#9696b4"   # file sizes
# accent       = "#7aa2f7"   # borders, headers, code files
# green        = "#9ece6a"   # executables, git added, success
# yellow       = "#e0af68"   # audio files, git modified
# orange       = "#ff9e64"   # archives
# red          = "#f7768e"   # git deleted, confirm modal
# purple       = "#bb9af7"   # images
# cyan         = "#7dcfff"   # directories

# [[bookmarks]]
# Custom bookmarks appear above the auto-detected ones.
# name = "Projects"
# path = "/home/user/projects"
#
# [[bookmarks]]
# name = "NAS"
# path = "/mnt/nas"
"##
}

// ── Helpers used by the rest of the app ──────────────────────────────────────

impl Config {
    /// Resolve the editor: config > $EDITOR > $VISUAL > nvim
    pub fn editor(&self) -> String {
        if !self.apps.editor.is_empty() {
            return self.apps.editor.clone();
        }
        std::env::var("EDITOR")
            .or_else(|_| std::env::var("VISUAL"))
            .unwrap_or_else(|_| "nvim".into())
    }

    /// Resolve the opener for a given file extension.
    /// Returns None to mean "use xdg-open".
    pub fn opener_for_ext(&self, ext: &str) -> Option<String> {
        let ext = ext.to_lowercase();
        let app = match ext.as_str() {
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "avif" | "tiff" | "svg"
                => &self.apps.image,
            "mp4" | "mkv" | "avi" | "mov" | "webm" | "flv" | "wmv"
                => &self.apps.video,
            "mp3" | "flac" | "ogg" | "wav" | "aac" | "m4a" | "opus"
                => &self.apps.audio,
            "pdf" => &self.apps.pdf,
            _ => return None,
        };
        if app.is_empty() { None } else { Some(app.clone()) }
    }
}
