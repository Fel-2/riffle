//! Color theme. Defaults to Tokyo Night; every color can be overridden from
//! the `[theme]` section of config.toml with "#rrggbb" hex strings.

use std::sync::OnceLock;

use ratatui::style::Color;
use serde::{Deserialize, Serialize};

pub struct Theme {
    /// Main background.
    pub bg: Color,
    /// Bars, sidebar, header background.
    pub bg_panel: Color,
    /// Cursor row background, inactive borders, separators.
    pub bg_highlight: Color,
    /// Selected entry background.
    pub bg_selection: Color,
    /// Normal text.
    pub fg: Color,
    /// Emphasized text (modal prompts).
    pub fg_bright: Color,
    /// Comments, hidden files, secondary info.
    pub dim: Color,
    /// File sizes column.
    pub muted: Color,
    /// Primary accent: borders, headers, directories cursor.
    pub accent: Color,
    pub green: Color,
    pub yellow: Color,
    pub orange: Color,
    pub red: Color,
    pub purple: Color,
    pub cyan: Color,
}

impl Default for Theme {
    fn default() -> Self {
        // Tokyo Night
        Self {
            bg:           Color::Rgb(26, 27, 38),    // #1a1b26
            bg_panel:     Color::Rgb(31, 35, 53),    // #1f2335
            bg_highlight: Color::Rgb(41, 46, 66),    // #292e42
            bg_selection: Color::Rgb(54, 74, 130),   // #364a82
            fg:           Color::Rgb(169, 177, 214), // #a9b1d6
            fg_bright:    Color::Rgb(192, 202, 245), // #c0caf5
            dim:          Color::Rgb(86, 95, 137),   // #565f89
            muted:        Color::Rgb(150, 150, 180), // #9696b4
            accent:       Color::Rgb(122, 162, 247), // #7aa2f7
            green:        Color::Rgb(158, 206, 106), // #9ece6a
            yellow:       Color::Rgb(224, 175, 104), // #e0af68
            orange:       Color::Rgb(255, 158, 100), // #ff9e64
            red:          Color::Rgb(247, 118, 142), // #f7768e
            purple:       Color::Rgb(187, 154, 247), // #bb9af7
            cyan:         Color::Rgb(125, 207, 255), // #7dcfff
        }
    }
}

/// `[theme]` section of config.toml. Every field is an optional "#rrggbb"
/// string; unset or unparsable values keep the Tokyo Night default.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeConfig {
    pub bg:           Option<String>,
    pub bg_panel:     Option<String>,
    pub bg_highlight: Option<String>,
    pub bg_selection: Option<String>,
    pub fg:           Option<String>,
    pub fg_bright:    Option<String>,
    pub dim:          Option<String>,
    pub muted:        Option<String>,
    pub accent:       Option<String>,
    pub green:        Option<String>,
    pub yellow:       Option<String>,
    pub orange:       Option<String>,
    pub red:          Option<String>,
    pub purple:       Option<String>,
    pub cyan:         Option<String>,
}

fn parse_hex(s: &str) -> Option<Color> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 { return None; }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}

impl Theme {
    pub fn from_config(cfg: &ThemeConfig) -> Self {
        let mut t = Theme::default();
        let apply = |slot: &mut Color, v: &Option<String>| {
            if let Some(c) = v.as_deref().and_then(parse_hex) { *slot = c; }
        };
        apply(&mut t.bg, &cfg.bg);
        apply(&mut t.bg_panel, &cfg.bg_panel);
        apply(&mut t.bg_highlight, &cfg.bg_highlight);
        apply(&mut t.bg_selection, &cfg.bg_selection);
        apply(&mut t.fg, &cfg.fg);
        apply(&mut t.fg_bright, &cfg.fg_bright);
        apply(&mut t.dim, &cfg.dim);
        apply(&mut t.muted, &cfg.muted);
        apply(&mut t.accent, &cfg.accent);
        apply(&mut t.green, &cfg.green);
        apply(&mut t.yellow, &cfg.yellow);
        apply(&mut t.orange, &cfg.orange);
        apply(&mut t.red, &cfg.red);
        apply(&mut t.purple, &cfg.purple);
        apply(&mut t.cyan, &cfg.cyan);
        t
    }
}

static THEME: OnceLock<Theme> = OnceLock::new();

/// Install the theme once at startup, before the first render.
pub fn init(cfg: &ThemeConfig) {
    let _ = THEME.set(Theme::from_config(cfg));
}

/// Current theme; Tokyo Night defaults if `init` was never called.
pub fn theme() -> &'static Theme {
    THEME.get_or_init(Theme::default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_with_and_without_hash() {
        assert_eq!(parse_hex("#ff0080"), Some(Color::Rgb(255, 0, 128)));
        assert_eq!(parse_hex("ff0080"), Some(Color::Rgb(255, 0, 128)));
        assert_eq!(parse_hex("xyz"), None);
        assert_eq!(parse_hex("#ff00"), None);
    }

    #[test]
    fn config_overrides_only_set_fields() {
        let cfg = ThemeConfig { accent: Some("#ff0000".into()), ..Default::default() };
        let t = Theme::from_config(&cfg);
        assert_eq!(t.accent, Color::Rgb(255, 0, 0));
        assert_eq!(t.bg, Theme::default().bg);
    }
}
