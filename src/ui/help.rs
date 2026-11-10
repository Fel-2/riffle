use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::state::App;
use crate::theme::Theme;

type Section = (&'static str, &'static [(&'static str, &'static str)]);

const SECTIONS: &[Section] = &[
    (
        "Navigate",
        &[
            ("j / ↓", "move down"),
            ("k / ↑", "move up"),
            ("l / → / ⏎", "open"),
            ("h / ← / ⌫", "parent directory"),
            ("g / G", "top / bottom"),
            ("PgUp / PgDn", "page"),
            ("alt+← / alt+→", "back / forward"),
            ("~", "home"),
            ("1 … 9", "switch tab"),
            ("Tab", "next pane / section"),
        ],
    ),
    (
        "Files",
        &[
            ("space", "select and move down"),
            ("V", "select all"),
            ("esc", "clear selection / filter"),
            ("y / x / p", "copy / cut / paste"),
            ("d / del", "move to trash"),
            ("D", "delete permanently"),
            ("r / F2", "rename"),
            ("a / A", "new file / directory"),
            ("e", "open in editor"),
            ("i", "file info"),
            ("Y", "copy path (OSC52)"),
            ("E / Z", "extract / compress"),
            ("K", "share via KDE Connect"),
        ],
    ),
    (
        "View",
        &[
            ("/", "fuzzy filter"),
            ("ctrl+g", "go to path"),
            (".", "hidden files"),
            ("s / S", "next sort / reverse"),
            ("P", "preview pane"),
            ("B", "sidebar"),
            ("R / F5", "refresh"),
            ("ctrl+b", "bookmark directory"),
            ("T", "open terminal here"),
            (":", "shell command"),
            ("ctrl+z", "undo trash"),
        ],
    ),
    (
        "Layout & app",
        &[
            ("ctrl+w", "dual pane"),
            ("b", "focus sidebar"),
            ("ctrl+t", "new tab"),
            ("X", "close tab"),
            ("[ / ]", "previous / next tab"),
            ("ctrl+p", "command palette"),
            ("?", "this help"),
            ("q / ctrl+c", "quit"),
        ],
    ),
];

pub fn render(f: &mut Frame, app: &mut App, theme: &Theme) {
    let Some(help) = &mut app.help else {
        return;
    };
    let area = f.area();
    let width = area.width.saturating_sub(4).min(96);
    let height = area.height.saturating_sub(2).min(36);
    let popup = Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    };
    f.render_widget(Clear, popup);
    let block = Block::default()
        .title(" Keys ")
        .title_style(
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.accent))
        .style(Style::default().bg(theme.bg));
    let inner = block.inner(popup);
    f.render_widget(block, popup);
    if inner.height == 0 || inner.width < 20 {
        return;
    }

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner);

    let left = section_lines(&SECTIONS[0..2], theme);
    let right = section_lines(&SECTIONS[2..4], theme);
    let visible = inner.height as usize;
    let total = left.len().max(right.len());
    let max_scroll = total.saturating_sub(visible);
    if help.scroll > max_scroll {
        help.scroll = max_scroll;
    }
    let scroll = help.scroll;

    f.render_widget(
        Paragraph::new(left)
            .style(Style::default().bg(theme.bg))
            .scroll((scroll as u16, 0)),
        columns[0],
    );
    f.render_widget(
        Paragraph::new(right)
            .style(Style::default().bg(theme.bg))
            .scroll((scroll as u16, 0)),
        columns[1],
    );

    if total > visible {
        let hint = format!(" {}% ", ((scroll + visible) * 100 / total).min(100));
        f.render_widget(
            Paragraph::new(Span::styled(hint, Style::default().fg(theme.dim)))
                .style(Style::default().bg(theme.bg)),
            Rect {
                x: popup.x + popup.width.saturating_sub(6),
                y: popup.y + popup.height.saturating_sub(1),
                width: 5,
                height: 1,
            },
        );
    }
}

fn section_lines(sections: &[Section], theme: &Theme) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for (index, (title, bindings)) in sections.iter().enumerate() {
        if index > 0 {
            lines.push(Line::from(""));
        }
        lines.push(Line::from(Span::styled(
            format!(" {}", title),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )));
        for (key, description) in *bindings {
            lines.push(Line::from(vec![
                Span::styled(format!("  {:<16}", key), Style::default().fg(theme.green)),
                Span::styled(*description, Style::default().fg(theme.fg)),
            ]));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  tip  ", Style::default().fg(theme.yellow)),
        Span::styled(
            "ctrl+p opens every command with fuzzy search",
            Style::default().fg(theme.dim),
        ),
    ]));
    lines
}
