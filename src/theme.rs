use ratatui::style::Color;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub bg: Color,
    pub panel: Color,
    pub highlight: Color,
    pub selection: Color,
    pub fg: Color,
    pub bright: Color,
    pub dim: Color,
    pub muted: Color,
    pub accent: Color,
    pub green: Color,
    pub yellow: Color,
    pub orange: Color,
    pub red: Color,
    pub purple: Color,
    pub cyan: Color,
    pub border: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self::preset("tokyo-night").unwrap_or(Self::tokyo_night())
    }
}

impl Theme {
    pub fn preset(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().replace('_', "-").as_str() {
            "tokyo-night" | "tokyo" | "default" => Some(Self::tokyo_night()),
            "catppuccin" | "catppuccin-mocha" | "mocha" => Some(Self::catppuccin()),
            "gruvbox" | "gruvbox-dark" => Some(Self::gruvbox()),
            "nord" => Some(Self::nord()),
            "dracula" => Some(Self::dracula()),
            "one-dark" | "onedark" => Some(Self::one_dark()),
            "light" | "github-light" => Some(Self::light()),
            _ => None,
        }
    }

    #[cfg(test)]
    pub fn preset_names() -> &'static [&'static str] {
        &[
            "tokyo-night",
            "catppuccin",
            "gruvbox",
            "nord",
            "dracula",
            "one-dark",
            "light",
        ]
    }

    fn tokyo_night() -> Self {
        Self {
            bg: rgb(0x1a1b26),
            panel: rgb(0x1f2335),
            highlight: rgb(0x292e42),
            selection: rgb(0x364a82),
            fg: rgb(0xa9b1d6),
            bright: rgb(0xc0caf5),
            dim: rgb(0x565f89),
            muted: rgb(0x9696b4),
            accent: rgb(0x7aa2f7),
            green: rgb(0x9ece6a),
            yellow: rgb(0xe0af68),
            orange: rgb(0xff9e64),
            red: rgb(0xf7768e),
            purple: rgb(0xbb9af7),
            cyan: rgb(0x7dcfff),
            border: rgb(0x292e42),
        }
    }

    fn catppuccin() -> Self {
        Self {
            bg: rgb(0x1e1e2e),
            panel: rgb(0x181825),
            highlight: rgb(0x313244),
            selection: rgb(0x45475a),
            fg: rgb(0xcdd6f4),
            bright: rgb(0xf5e0dc),
            dim: rgb(0x6c7086),
            muted: rgb(0x9399b2),
            accent: rgb(0x89b4fa),
            green: rgb(0xa6e3a1),
            yellow: rgb(0xf9e2af),
            orange: rgb(0xfab387),
            red: rgb(0xf38ba8),
            purple: rgb(0xcba6f7),
            cyan: rgb(0x89dceb),
            border: rgb(0x313244),
        }
    }

    fn gruvbox() -> Self {
        Self {
            bg: rgb(0x282828),
            panel: rgb(0x1d2021),
            highlight: rgb(0x3c3836),
            selection: rgb(0x504945),
            fg: rgb(0xebdbb2),
            bright: rgb(0xfbf1c7),
            dim: rgb(0x928374),
            muted: rgb(0xa89984),
            accent: rgb(0x83a598),
            green: rgb(0xb8bb26),
            yellow: rgb(0xfabd2f),
            orange: rgb(0xfe8019),
            red: rgb(0xfb4934),
            purple: rgb(0xd3869b),
            cyan: rgb(0x8ec07c),
            border: rgb(0x3c3836),
        }
    }

    fn nord() -> Self {
        Self {
            bg: rgb(0x2e3440),
            panel: rgb(0x272c36),
            highlight: rgb(0x3b4252),
            selection: rgb(0x434c5e),
            fg: rgb(0xd8dee9),
            bright: rgb(0xeceff4),
            dim: rgb(0x616e88),
            muted: rgb(0x81a1c1),
            accent: rgb(0x88c0d0),
            green: rgb(0xa3be8c),
            yellow: rgb(0xebcb8b),
            orange: rgb(0xd08770),
            red: rgb(0xbf616a),
            purple: rgb(0xb48ead),
            cyan: rgb(0x8fbcbb),
            border: rgb(0x434c5e),
        }
    }

    fn dracula() -> Self {
        Self {
            bg: rgb(0x282a36),
            panel: rgb(0x21222c),
            highlight: rgb(0x44475a),
            selection: rgb(0x6272a4),
            fg: rgb(0xf8f8f2),
            bright: rgb(0xffffff),
            dim: rgb(0x6272a4),
            muted: rgb(0xbfbfbf),
            accent: rgb(0xbd93f9),
            green: rgb(0x50fa7b),
            yellow: rgb(0xf1fa8c),
            orange: rgb(0xffb86c),
            red: rgb(0xff5555),
            purple: rgb(0xff79c6),
            cyan: rgb(0x8be9fd),
            border: rgb(0x44475a),
        }
    }

    fn one_dark() -> Self {
        Self {
            bg: rgb(0x282c34),
            panel: rgb(0x21252b),
            highlight: rgb(0x2c313a),
            selection: rgb(0x3e4451),
            fg: rgb(0xabb2bf),
            bright: rgb(0xe6e6e6),
            dim: rgb(0x5c6370),
            muted: rgb(0x828997),
            accent: rgb(0x61afef),
            green: rgb(0x98c379),
            yellow: rgb(0xe5c07b),
            orange: rgb(0xd19a66),
            red: rgb(0xe06c75),
            purple: rgb(0xc678dd),
            cyan: rgb(0x56b6c2),
            border: rgb(0x3e4451),
        }
    }

    fn light() -> Self {
        Self {
            bg: rgb(0xffffff),
            panel: rgb(0xf6f8fa),
            highlight: rgb(0xeaeef2),
            selection: rgb(0xddf4ff),
            fg: rgb(0x1f2328),
            bright: rgb(0x000000),
            dim: rgb(0x8c959f),
            muted: rgb(0x656d76),
            accent: rgb(0x0969da),
            green: rgb(0x1a7f37),
            yellow: rgb(0x9a6700),
            orange: rgb(0xbc4c00),
            red: rgb(0xcf222e),
            purple: rgb(0x8250df),
            cyan: rgb(0x1b7c83),
            border: rgb(0xd0d7de),
        }
    }

    pub fn from_config(cfg: &ThemeConfig) -> Self {
        let mut theme = cfg
            .preset
            .as_deref()
            .and_then(Self::preset)
            .unwrap_or_default();
        if cfg.transparent.unwrap_or(false) {
            theme.bg = Color::Reset;
            theme.panel = Color::Reset;
        }
        let overrides = [
            (&cfg.bg, &mut theme.bg),
            (&cfg.panel, &mut theme.panel),
            (&cfg.highlight, &mut theme.highlight),
            (&cfg.selection, &mut theme.selection),
            (&cfg.fg, &mut theme.fg),
            (&cfg.bright, &mut theme.bright),
            (&cfg.dim, &mut theme.dim),
            (&cfg.muted, &mut theme.muted),
            (&cfg.accent, &mut theme.accent),
            (&cfg.green, &mut theme.green),
            (&cfg.yellow, &mut theme.yellow),
            (&cfg.orange, &mut theme.orange),
            (&cfg.red, &mut theme.red),
            (&cfg.purple, &mut theme.purple),
            (&cfg.cyan, &mut theme.cyan),
            (&cfg.border, &mut theme.border),
        ];
        for (value, slot) in overrides {
            if let Some(color) = value.as_deref().and_then(parse_color) {
                *slot = color;
            }
        }
        theme
    }

    pub fn bg_rgb(&self) -> (u8, u8, u8) {
        match self.bg {
            Color::Rgb(r, g, b) => (r, g, b),
            _ => (0, 0, 0),
        }
    }
}

fn rgb(hex: u32) -> Color {
    Color::Rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

pub fn parse_color(s: &str) -> Option<Color> {
    let s = s.trim();
    if s.eq_ignore_ascii_case("reset") || s.eq_ignore_ascii_case("none") {
        return Some(Color::Reset);
    }
    let named = match s.to_ascii_lowercase().as_str() {
        "black" => Some(Color::Black),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "blue" => Some(Color::Blue),
        "magenta" => Some(Color::Magenta),
        "cyan" => Some(Color::Cyan),
        "gray" | "grey" | "white" => Some(Color::White),
        "darkgray" | "darkgrey" => Some(Color::DarkGray),
        _ => None,
    };
    if named.is_some() {
        return named;
    }
    let hex = s.trim_start_matches('#');
    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            Some(Color::Rgb(r, g, b))
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some(Color::Rgb(r, g, b))
        }
        _ => None,
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeConfig {
    pub preset: Option<String>,
    pub transparent: Option<bool>,
    pub bg: Option<String>,
    pub panel: Option<String>,
    pub highlight: Option<String>,
    pub selection: Option<String>,
    pub fg: Option<String>,
    pub bright: Option<String>,
    pub dim: Option<String>,
    pub muted: Option<String>,
    pub accent: Option<String>,
    pub green: Option<String>,
    pub yellow: Option<String>,
    pub orange: Option<String>,
    pub red: Option<String>,
    pub purple: Option<String>,
    pub cyan: Option<String>,
    pub border: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_forms() {
        assert_eq!(parse_color("#ff0080"), Some(Color::Rgb(255, 0, 128)));
        assert_eq!(parse_color("f08"), Some(Color::Rgb(255, 0, 136)));
        assert_eq!(parse_color("reset"), Some(Color::Reset));
        assert_eq!(parse_color("nope"), None);
    }

    #[test]
    fn overrides_apply_on_top_of_preset() {
        let cfg = ThemeConfig {
            preset: Some("nord".into()),
            accent: Some("#ff0000".into()),
            ..Default::default()
        };
        let theme = Theme::from_config(&cfg);
        assert_eq!(theme.accent, Color::Rgb(255, 0, 0));
        assert_eq!(theme.bg, Theme::preset("nord").unwrap().bg);
    }

    #[test]
    fn all_presets_exist() {
        for name in Theme::preset_names() {
            assert!(Theme::preset(name).is_some(), "{}", name);
        }
    }
}
