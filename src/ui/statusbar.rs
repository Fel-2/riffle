use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use crate::state::{AppState, InputMode};
use crate::utils;

pub fn render(f: &mut Frame, area: Rect, state: &AppState) {
    let tab = state.focused_tab();
    let total = tab.display_count();
    let cursor = if total == 0 { 0 } else { tab.cursor + 1 };
    let selected = tab.selection.len();

    let mut spans: Vec<Span> = Vec::new();

    // Position
    spans.push(Span::styled(
        format!(" {}/{} ", cursor, total),
        Style::default().fg(Color::Rgb(122, 162, 247)).bg(Color::Rgb(31, 35, 53)),
    ));

    // Separator
    spans.push(Span::styled("|", Style::default().fg(Color::Rgb(41, 46, 66)).bg(Color::Rgb(31, 35, 53))));

    // Dir (truncated)
    let dir = tab.current_dir.to_string_lossy().into_owned();
    let max_path = area.width.saturating_sub(40) as usize;
    let dir_str = if max_path > 3 && dir.len() > max_path {
        format!("~{}", &dir[dir.len() - max_path..])
    } else {
        dir
    };
    spans.push(Span::styled(
        format!(" {} ", dir_str),
        Style::default().fg(Color::Rgb(169, 177, 214)).bg(Color::Rgb(31, 35, 53)),
    ));

    // Selected count
    if selected > 0 {
        spans.push(Span::styled("|", Style::default().fg(Color::Rgb(41, 46, 66)).bg(Color::Rgb(31, 35, 53))));
        spans.push(Span::styled(
            format!(" {} sel ", selected),
            Style::default().fg(Color::Yellow).bg(Color::Rgb(31, 35, 53)),
        ));
    }

    // Focused entry info
    if let Some(e) = tab.cursor_entry() {
        spans.push(Span::styled("|", Style::default().fg(Color::Rgb(41, 46, 66)).bg(Color::Rgb(31, 35, 53))));
        let info = if e.is_dir {
            format!(" {} ", e.name)
        } else {
            format!(" {}  {} ", e.name, utils::format_size(e.size))
        };
        spans.push(Span::styled(info, Style::default().fg(Color::Rgb(169, 177, 214)).bg(Color::Rgb(31, 35, 53))));
    }

    // Right side: status message or mode indicator
    let right_text = if let Some((msg, _)) = &state.status {
        msg.clone()
    } else {
        match state.input_mode {
            InputMode::Search => "SEARCH".to_string(),
            InputMode::Rename => "RENAME".to_string(),
            InputMode::GoTo   => "GOTO".to_string(),
            InputMode::Shell  => "SHELL".to_string(),
            InputMode::Normal => {
                let mut parts = Vec::new();
                if state.dual_pane {
                    if state.right_focused { parts.push("DUAL [R]"); } else { parts.push("DUAL [L]"); }
                }
                if state.preview_enabled {
                    parts.push("PREVIEW");
                }
                parts.join(" | ")
            }
        }
    };

    if !right_text.is_empty() {
        let right_style = if state.status.is_some() {
            Style::default().fg(Color::Rgb(158, 206, 106)).bg(Color::Rgb(31, 35, 53))
        } else {
            Style::default().fg(Color::Rgb(122, 162, 247)).bg(Color::Rgb(31, 35, 53))
        };
        // Pad to right-align
        let left_len: usize = spans.iter().map(|s| s.content.len()).sum();
        let right_len = right_text.len() + 2;
        let pad = (area.width as usize).saturating_sub(left_len + right_len);
        spans.push(Span::styled(" ".repeat(pad), Style::default().bg(Color::Rgb(31, 35, 53))));
        spans.push(Span::styled(format!(" {} ", right_text), right_style));
    }

    f.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(Color::Rgb(31, 35, 53))),
        area,
    );
}
