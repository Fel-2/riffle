mod chrome;
mod filelist;
mod help;
pub mod image;
mod modal;
mod preview;
mod sidebar;

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::filesystem::FileClass;
use crate::proc::GitStatus;
use crate::state::{App, Layout as AppLayout, Mode};
use crate::theme::Theme;
use crate::utils;

pub fn render(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let theme = app.theme;
    f.render_widget(Block::default().style(Style::default().bg(theme.bg)), area);

    let hint_height = if area.height >= 10 { 1 } else { 0 };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(hint_height),
        ])
        .split(area);

    app.layout = AppLayout::default();

    chrome::render_tabbar(f, chunks[0], app, &theme);

    let mut body = chunks[1];
    let show_sidebar = app.show_sidebar && area.width >= 60 && body.width >= 30;
    let show_preview = app.show_preview && !app.dual && area.width >= 90;

    if show_sidebar {
        let width = app
            .config
            .ui
            .sidebar_width
            .clamp(14, 36)
            .min(body.width.saturating_sub(24));
        let split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(width), Constraint::Min(1)])
            .split(body);
        app.layout.sidebar = Some(split[0]);
        sidebar::render(f, split[0], app, &theme);
        body = split[1];
    }

    if app.dual {
        let split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Length(1),
                Constraint::Percentage(50),
            ])
            .split(body);
        divider(f, split[1], &theme);
        app.layout.panes[0] = Some(filelist::render_pane(f, split[0], app, 0, &theme));
        app.layout.panes[1] = Some(filelist::render_pane(f, split[2], app, 1, &theme));
    } else if show_preview {
        let percent = app.config.ui.preview_width.clamp(25, 70);
        let split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(100 - percent),
                Constraint::Length(1),
                Constraint::Percentage(percent),
            ])
            .split(body);
        divider(f, split[1], &theme);
        app.layout.panes[0] = Some(filelist::render_pane(f, split[0], app, 0, &theme));
        app.layout.preview = Some(split[2]);
        preview::render(f, split[2], app, &theme);
    } else {
        app.layout.panes[0] = Some(filelist::render_pane(f, body, app, 0, &theme));
    }

    chrome::render_status(f, chunks[2], app, &theme);
    if hint_height > 0 {
        chrome::render_hints(f, chunks[3], app, &theme);
    }

    match app.mode {
        Mode::Prompt => modal::render_prompt(f, app, &theme),
        Mode::Confirm => modal::render_confirm(f, app, &theme),
        Mode::Palette => modal::render_palette(f, app, &theme),
        Mode::Info => modal::render_info(f, app, &theme),
        Mode::Help => help::render(f, app, &theme),
        Mode::Normal => {}
    }
}

fn divider(f: &mut Frame, area: Rect, theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let lines: Vec<Line> = (0..area.height)
        .map(|_| Line::from(Span::styled("│", Style::default().fg(theme.border))))
        .collect();
    f.render_widget(
        Paragraph::new(lines).style(Style::default().bg(theme.bg)),
        area,
    );
}

pub fn class_color(
    class: FileClass,
    is_dir: bool,
    hidden: bool,
    broken: bool,
    theme: &Theme,
) -> Color {
    if broken {
        return theme.red;
    }
    if is_dir {
        return if hidden { theme.dim } else { theme.cyan };
    }
    if hidden {
        return theme.dim;
    }
    match class {
        FileClass::Image => theme.purple,
        FileClass::Video => theme.purple,
        FileClass::Audio => theme.yellow,
        FileClass::Archive => theme.orange,
        FileClass::Executable => theme.green,
        FileClass::Code => theme.accent,
        FileClass::Config => theme.muted,
        FileClass::Font => theme.cyan,
        FileClass::Document | FileClass::Text | FileClass::Other => theme.fg,
        FileClass::Directory => theme.cyan,
    }
}

pub fn git_style(status: GitStatus, theme: &Theme) -> Style {
    let color = match status {
        GitStatus::Modified => theme.yellow,
        GitStatus::Added => theme.green,
        GitStatus::Deleted => theme.red,
        GitStatus::Renamed => theme.accent,
        GitStatus::Untracked => theme.dim,
        GitStatus::Conflicted => theme.orange,
    };
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

pub fn highlighted_name(
    name: &str,
    matches: &[usize],
    base: Style,
    theme: &Theme,
) -> Vec<Span<'static>> {
    if matches.is_empty() {
        return vec![Span::styled(name.to_string(), base)];
    }
    let chars: Vec<char> = name.chars().collect();
    let mut spans = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < chars.len() {
        if matches.contains(&i) {
            if start < i {
                spans.push(Span::styled(
                    chars[start..i].iter().collect::<String>(),
                    base,
                ));
            }
            let begin = i;
            while i < chars.len() && matches.contains(&i) {
                i += 1;
            }
            spans.push(Span::styled(
                chars[begin..i].iter().collect::<String>(),
                base.fg(theme.accent).add_modifier(Modifier::BOLD),
            ));
            start = i;
        } else {
            i += 1;
        }
    }
    if start < chars.len() {
        spans.push(Span::styled(
            chars[start..].iter().collect::<String>(),
            base,
        ));
    }
    spans
}

pub fn truncate_with_matches(name: &str, width: usize, matches: &[usize]) -> (String, Vec<usize>) {
    let shown = utils::truncate(name, width);
    let shown_len = shown.chars().count();
    let kept: Vec<usize> = matches
        .iter()
        .copied()
        .filter(|index| *index < shown_len)
        .collect();
    (shown, kept)
}

pub fn mode_label(mode: Mode) -> &'static str {
    match mode {
        Mode::Normal => "",
        Mode::Prompt => "PROMPT",
        Mode::Confirm => "CONFIRM",
        Mode::Palette => "PALETTE",
        Mode::Help => "HELP",
        Mode::Info => "INFO",
    }
}
