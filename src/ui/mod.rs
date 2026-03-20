mod content;
mod help;
mod sidebar;
mod statusbar;
pub mod preview;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    Frame,
};
use crate::state::AppState;

pub fn render(f: &mut Frame, state: &mut AppState) {
    let area = f.area();

    // Help overlay (modal)
    if state.show_help {
        help::render(f, area);
        return;
    }

    // Main layout: top path bar (1), middle (rest), bottom status (1) + maybe input
    let input_visible = matches!(state.input_mode,
        crate::state::InputMode::Search
        | crate::state::InputMode::GoTo
        | crate::state::InputMode::Rename
        | crate::state::InputMode::Shell);

    let v_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // path/tab bar
            Constraint::Min(1),    // main area
            Constraint::Length(if input_visible { 1 } else { 0 }), // input bar
            Constraint::Length(1), // status bar
        ])
        .split(area);

    // Path bar
    render_path_bar(f, v_chunks[0], state);

    // Main area: sidebar + content (possibly dual pane or preview)
    let h_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(state.sidebar_width),
            Constraint::Min(1),
        ])
        .split(v_chunks[1]);

    sidebar::render(f, h_chunks[0], state);

    if state.dual_pane {
        let pane_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(h_chunks[1]);
        let active = state.active_tab;
        let right = state.right_tab;
        let right_focused = state.right_focused;
        content::render_pane(f, pane_chunks[0], state, active, !right_focused);
        content::render_pane(f, pane_chunks[1], state, right, right_focused);
        // No preview in dual pane
        state.preview_rect = None;
    } else if state.preview_enabled {
        let pane_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
            .split(h_chunks[1]);
        let active = state.active_tab;
        content::render_pane(f, pane_chunks[0], state, active, true);
        // Preview sets state.preview_rect internally
        preview::render(f, pane_chunks[1], state);
    } else {
        let active = state.active_tab;
        content::render_pane(f, h_chunks[1], state, active, true);
        state.preview_rect = None;
    }

    // Input bar
    if input_visible {
        render_input_bar(f, v_chunks[2], state);
    }

    // Status bar
    statusbar::render(f, v_chunks[3], state);
}

fn render_path_bar(f: &mut Frame, area: Rect, state: &AppState) {
    use ratatui::widgets::Paragraph;
    use ratatui::style::{Color, Modifier, Style};
    use ratatui::text::{Line, Span};

    let tab = state.focused_tab();
    let path_str = tab.current_dir.to_string_lossy().into_owned();

    // Tab indicators
    let mut spans: Vec<Span> = Vec::new();
    for (i, t) in state.tabs.iter().enumerate() {
        let is_active = i == state.active_tab;
        let name = t.current_dir.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("/");
        let style = if is_active {
            Style::default().fg(Color::Rgb(122, 162, 247)).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Rgb(86, 95, 137))
        };
        spans.push(Span::styled(format!(" {} ", name), style));
        if i < state.tabs.len() - 1 {
            spans.push(Span::styled("|", Style::default().fg(Color::Rgb(41, 46, 66))));
        }
    }
    spans.push(Span::styled("  ", Style::default()));
    spans.push(Span::styled(path_str, Style::default().fg(Color::Rgb(169, 177, 214))));

    let line = Line::from(spans);
    let para = Paragraph::new(line)
        .style(Style::default().bg(Color::Rgb(31, 35, 53)));
    f.render_widget(para, area);
}

fn render_input_bar(f: &mut Frame, area: Rect, state: &AppState) {
    use ratatui::widgets::Paragraph;
    use ratatui::style::{Color, Style};
    use ratatui::text::{Line, Span};

    let (label, content) = match state.input_mode {
        crate::state::InputMode::Search => {
            let q = state.focused_tab().search_query.clone();
            (" / ", format!("{}_", q))
        }
        crate::state::InputMode::GoTo => (" > ", format!("{}_", state.goto_buf)),
        crate::state::InputMode::Rename => {
            let buf = state.focused_tab().renaming.as_ref().map(|(_, b)| b.clone()).unwrap_or_default();
            (" r ", format!("{}_", buf))
        }
        crate::state::InputMode::Shell => (" : ", format!("{}_", state.shell_buf)),
        _ => return,
    };

    let line = Line::from(vec![
        Span::styled(label, Style::default().fg(Color::Rgb(122, 162, 247)).bg(Color::Rgb(31, 35, 53))),
        Span::styled(content, Style::default().fg(Color::Rgb(169, 177, 214)).bg(Color::Rgb(31, 35, 53))),
    ]);
    f.render_widget(Paragraph::new(line).style(Style::default().bg(Color::Rgb(31, 35, 53))), area);
}
