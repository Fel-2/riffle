use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::theme::theme;

pub fn render(f: &mut Frame, area: Rect) {
    let t = theme();
    let popup_w = 70u16.min(area.width);
    let popup_h = 40u16.min(area.height);
    let x = (area.width.saturating_sub(popup_w)) / 2;
    let y = (area.height.saturating_sub(popup_h)) / 2;
    let popup = Rect { x, y, width: popup_w, height: popup_h };

    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Keyboard Shortcuts  [F1 / ? to close] ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(t.accent))
        .style(Style::default().bg(t.bg));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let sections: &[(&str, &[(&str, &str)])] = &[
        ("Navigation", &[
            ("j / Down",          "Move down"),
            ("k / Up",            "Move up"),
            ("h / Left / Bksp",   "Go to parent"),
            ("l / Right / Enter", "Open file/dir"),
            ("g / G",             "Top / Bottom"),
            ("PgDn / PgUp",       "Jump 20 rows"),
            ("b",                 "Focus sidebar"),
            ("Scroll wheel",      "Scroll 3 rows"),
        ]),
        ("File Ops", &[
            ("Space",   "Select / deselect"),
            ("y",       "Yank (copy)"),
            ("x",       "Cut"),
            ("p",       "Paste (background, no overwrite)"),
            ("d / Del", "Trash (recoverable)"),
            ("D",       "Delete permanently (confirm)"),
            ("Ctrl+Z",  "Undo last trash"),
            ("r / F2",  "Rename (cursor at stem)"),
            ("a / A",   "New file / New dir (named)"),
            ("e",       "Open in editor"),
            ("E",       "Extract archive"),
            ("Z",       "Compress to zip"),
            ("K",       "Share via KDE Connect"),
        ]),
        ("View", &[
            ("/",       "Search (filter)"),
            ("Ctrl+G",  "Go to path"),
            (".",       "Toggle hidden files"),
            ("s / S",   "Cycle sort / toggle order"),
            ("T",       "Open terminal here"),
            ("Ctrl+B",  "Bookmark current dir"),
            ("P",       "Toggle preview pane"),
            (":",       "Shell command"),
        ]),
        ("Panes & Tabs", &[
            ("Ctrl+W", "Toggle dual pane"),
            ("Tab",    "Switch pane focus"),
            ("Ctrl+T", "New tab"),
            ("[  /  ]", "Prev / next tab"),
            ("X",      "Close current tab"),
            ("F5 / R", "Refresh"),
            ("? / F1", "This help"),
            ("q",      "Quit"),
        ]),
    ];

    let col_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner);

    for (col, pair) in sections.chunks(2).enumerate() {
        let col_area = col_chunks[col.min(1)];
        let rows: Vec<Line> = pair.iter().flat_map(|(section, bindings)| {
            let mut lines: Vec<Line> = Vec::new();
            lines.push(Line::from(Span::styled(
                format!(" {}", section),
                Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
            )));
            for (key, desc) in *bindings {
                lines.push(Line::from(vec![
                    Span::styled(format!("  {:14}", key), Style::default().fg(t.green)),
                    Span::styled(*desc, Style::default().fg(t.fg)),
                ]));
            }
            lines.push(Line::from(""));
            lines
        }).collect();

        f.render_widget(
            Paragraph::new(rows).style(Style::default().bg(t.bg)),
            col_area,
        );
    }
}
