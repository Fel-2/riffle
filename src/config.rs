use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::filesystem::FileClass;
use crate::theme::ThemeConfig;
use crate::utils::DateStyle;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub apps: Apps,
    pub ui: Ui,
    pub behavior: Behavior,
    pub theme: ThemeConfig,
    pub bookmarks: Vec<Bookmark>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Apps {
    pub editor: String,
    pub terminal: String,
    pub image: String,
    pub video: String,
    pub audio: String,
    pub pdf: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Ui {
    pub show_hidden: bool,
    pub sort_by: String,
    pub sort_ascending: bool,
    #[serde(alias = "preview_enabled")]
    pub preview: bool,
    pub sidebar: bool,
    pub sidebar_width: u16,
    pub preview_width: u16,
    pub icons: bool,
    pub git: bool,
    pub mouse: bool,
    pub date_format: DateStyleConfig,
    pub image_mode: ImageMode,
}

impl Default for Ui {
    fn default() -> Self {
        Self {
            show_hidden: false,
            sort_by: "name".into(),
            sort_ascending: true,
            preview: true,
            sidebar: true,
            sidebar_width: 22,
            preview_width: 45,
            icons: true,
            git: true,
            mouse: true,
            date_format: DateStyleConfig::Relative,
            image_mode: ImageMode::Auto,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Behavior {
    pub fuzzy: bool,
    pub print_cwd: bool,
    pub shell: String,
    pub confirm_trash: bool,
    pub double_click_ms: u64,
    pub update_check: bool,
}

impl Default for Behavior {
    fn default() -> Self {
        Self {
            fuzzy: true,
            print_cwd: false,
            shell: String::new(),
            confirm_trash: false,
            double_click_ms: 400,
            update_check: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageMode {
    #[default]
    Auto,
    Kitty,
    Blocks,
    Chafa,
    None,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DateStyleConfig {
    #[default]
    Relative,
    Iso,
    Short,
}

impl DateStyleConfig {
    pub fn style(self) -> DateStyle {
        match self {
            Self::Relative => DateStyle::Relative,
            Self::Iso => DateStyle::Iso,
            Self::Short => DateStyle::Short,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub name: String,
    pub path: String,
}

pub struct Loaded {
    pub config: Config,
    pub error: Option<String>,
}

pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("riffle")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

fn non_empty_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn bookmarks_path() -> PathBuf {
    config_dir().join("bookmarks.toml")
}

pub fn load() -> Loaded {
    let path = config_path();
    if !path.exists() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&path, DEFAULT_CONFIG);
        return Loaded {
            config: Config::default(),
            error: None,
        };
    }
    let raw = match std::fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(err) => {
            return Loaded {
                config: Config::default(),
                error: Some(format!("could not read config: {err}")),
            }
        }
    };
    match toml::from_str(&raw) {
        Ok(config) => Loaded {
            config,
            error: None,
        },
        Err(err) => Loaded {
            config: Config::default(),
            error: Some(format!("config error: {err}")),
        },
    }
}

pub fn load_persisted_bookmarks() -> Vec<Bookmark> {
    let Ok(raw) = std::fs::read_to_string(bookmarks_path()) else {
        return Vec::new();
    };
    #[derive(Deserialize, Default)]
    struct File {
        #[serde(default)]
        bookmark: Vec<Bookmark>,
    }
    toml::from_str::<File>(&raw)
        .map(|f| f.bookmark)
        .unwrap_or_default()
}

pub fn save_persisted_bookmarks(bookmarks: &[Bookmark]) -> Result<(), String> {
    #[derive(Serialize)]
    struct File<'a> {
        bookmark: &'a [Bookmark],
    }
    let body = toml::to_string_pretty(&File {
        bookmark: bookmarks,
    })
    .map_err(|e| format!("could not serialize bookmarks: {e}"))?;
    let path = bookmarks_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, body).map_err(|e| format!("could not save bookmarks: {e}"))
}

impl Config {
    pub fn editor(&self) -> String {
        if !self.apps.editor.trim().is_empty() {
            return self.apps.editor.clone();
        }
        non_empty_env("EDITOR")
            .or_else(|| non_empty_env("VISUAL"))
            .unwrap_or_else(|| "nvim".into())
    }

    pub fn shell(&self) -> String {
        if !self.behavior.shell.trim().is_empty() {
            return self.behavior.shell.clone();
        }
        non_empty_env("SHELL").unwrap_or_else(|| "sh".into())
    }

    pub fn opener_for(&self, class: FileClass) -> Option<String> {
        let app = match class {
            FileClass::Image => &self.apps.image,
            FileClass::Video => &self.apps.video,
            FileClass::Audio => &self.apps.audio,
            FileClass::Document => &self.apps.pdf,
            _ => return None,
        };
        if app.trim().is_empty() {
            None
        } else {
            Some(app.clone())
        }
    }

    pub fn date_style(&self) -> DateStyle {
        self.ui.date_format.style()
    }
}

pub const DEFAULT_CONFIG: &str = r##"# riffle configuration
# ~/.config/riffle/config.toml

[apps]
# Editor for `e` and for opening text files. Empty = $EDITOR / $VISUAL / nvim.
# Arguments are allowed, e.g. "code --wait".
editor = ""

# Terminal for `T`. Empty = auto-detect.
terminal = ""

# Per-type openers. Empty = xdg-open.
image = ""   # e.g. "imv", "feh"
video = ""   # e.g. "mpv"
audio = ""   # e.g. "mpv"
pdf = ""     # e.g. "zathura"

[ui]
# Show dot-files on startup.
show_hidden = false

# "name", "size", "date" or "type".
sort_by = "name"
sort_ascending = true

# Preview pane and sidebar. Both hide automatically on narrow terminals.
preview = true
sidebar = true
sidebar_width = 22
# Percentage of the body width used by the preview pane (25-70).
preview_width = 45

icons = true
git = true
mouse = true

# "relative", "iso" or "short".
date_format = "relative"

# "auto", "kitty", "blocks", "chafa" or "none".
image_mode = "auto"

[behavior]
# Fuzzy filtering. Set to false for plain substring matching.
fuzzy = true

# Print the current directory on exit (for shell integration, see README).
print_cwd = false

# Shell for `:`. Empty = $SHELL.
shell = ""

# Ask before moving files to the trash.
confirm_trash = false

double_click_ms = 400

# Check GitHub for a newer release at most once a day. Set to false to disable.
update_check = true

[theme]
# Presets: tokyo-night, catppuccin, gruvbox, nord, dracula, one-dark, light.
preset = "tokyo-night"

# Use the terminal background instead of a solid color.
transparent = false

# Override individual colors with "#rrggbb" (or "reset").
# bg        = "#1a1b26"
# panel     = "#1f2335"
# highlight = "#292e42"
# selection = "#364a82"
# fg        = "#a9b1d6"
# bright    = "#c0caf5"
# dim       = "#565f89"
# muted     = "#9696b4"
# accent    = "#7aa2f7"
# green     = "#9ece6a"
# yellow    = "#e0af68"
# orange    = "#ff9e64"
# red       = "#f7768e"
# purple    = "#bb9af7"
# cyan      = "#7dcfff"
# border    = "#292e42"

# Extra bookmarks (persistent bookmarks added with Ctrl+B live in bookmarks.toml).
# [[bookmarks]]
# name = "Projects"
# path = "/home/user/projects"
"##;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_parses() {
        let cfg: Config = toml::from_str(DEFAULT_CONFIG).unwrap();
        assert_eq!(cfg.ui.sidebar_width, 22);
        assert_eq!(cfg.theme.preset.as_deref(), Some("tokyo-night"));
    }

    #[test]
    fn partial_config_uses_defaults() {
        let cfg: Config = toml::from_str("[ui]\nshow_hidden = true\n").unwrap();
        assert!(cfg.ui.show_hidden);
        assert!(cfg.ui.preview);
        assert_eq!(cfg.ui.image_mode, ImageMode::Auto);
    }

    #[test]
    fn old_preview_key_is_still_accepted() {
        let cfg: Config = toml::from_str("[ui]\npreview_enabled = false\n").unwrap();
        assert!(!cfg.ui.preview);
    }

    #[test]
    fn broken_config_is_reported_not_panicking() {
        let err = toml::from_str::<Config>("[ui\n").unwrap_err();
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn editor_falls_back_to_env() {
        let mut cfg = Config::default();
        cfg.apps.editor = "hx".into();
        assert_eq!(cfg.editor(), "hx");
    }
}
