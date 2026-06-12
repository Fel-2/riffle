mod content;
mod help;
pub mod sidebar;
mod statusbar;
pub mod preview;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    Frame,
};
use crate::state::AppState;
use crate::theme::theme;

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
        | crate::state::InputMode::Shell
        | crate::state::InputMode::Create);

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

    // Confirmation modal (overlay, drawn last)
    if state.input_mode == crate::state::InputMode::Confirm {
        if let Some((prompt, _)) = &state.confirm {
            render_confirm(f, area, prompt);
        }
    }
}

fn render_confirm(f: &mut Frame, area: Rect, prompt: &str) {
    let t = theme();
    use ratatui::widgets::{Block, Borders, Clear, Paragraph};
    use ratatui::style::{Modifier, Style};
    use ratatui::text::{Line, Span};
    use ratatui::layout::Alignment;

    let hint = "y / Enter = yes      n / Esc = no";
    let w = (prompt.chars().count().max(hint.len()) as u16 + 6).min(area.width.saturating_sub(2));
    let h = 5u16;
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let modal = Rect { x, y, width: w, height: h };

    f.render_widget(Clear, modal);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(t.red))
        .style(Style::default().bg(t.bg));

    let lines = vec![
        Line::from(Span::styled(
            prompt.to_string(),
            Style::default().fg(t.fg_bright).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(hint, Style::default().fg(t.dim))),
    ];

    let para = Paragraph::new(lines)
        .block(block)
        .alignment(Alignment::Center);
    f.render_widget(para, modal);
}

fn render_path_bar(f: &mut Frame, area: Rect, state: &AppState) {
    let t = theme();
    use ratatui::widgets::Paragraph;
    use ratatui::style::{Modifier, Style};
    use ratatui::text::{Line, Span};

    let tab = state.focused_tab();
    let path_str = tab.current_dir.to_string_lossy().into_owned();

    // Tab indicators
    let mut spans: Vec<Span> = Vec::new();
    for (i, tb) in state.tabs.iter().enumerate() {
        let is_active = i == state.active_tab;
        let name = tb.current_dir.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("/");
        let style = if is_active {
            Style::default().fg(t.accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(t.dim)
        };
        spans.push(Span::styled(format!(" {} ", name), style));
        if i < state.tabs.len() - 1 {
            spans.push(Span::styled("|", Style::default().fg(t.bg_highlight)));
        }
    }
    spans.push(Span::styled("  ", Style::default()));
    spans.push(Span::styled(path_str, Style::default().fg(t.fg)));

    let line = Line::from(spans);
    let para = Paragraph::new(line)
        .style(Style::default().bg(t.bg_panel));
    f.render_widget(para, area);
}

fn render_input_bar(f: &mut Frame, area: Rect, state: &AppState) {
    let t = theme();
    use ratatui::widgets::Paragraph;
    use ratatui::style::Style;
    use ratatui::text::{Line, Span};

    let (label, content) = match state.input_mode {
        crate::state::InputMode::Search => {
            let q = state.focused_tab().search_query.clone();
            (" / ", format!("{}_", q))
        }
        crate::state::InputMode::GoTo => (" > ", format!("{}_", state.goto_buf)),
        crate::state::InputMode::Rename => {
            let (buf, cur) = state.focused_tab().renaming.as_ref()
                .map(|(_, b, c)| (b.clone(), *c)).unwrap_or_default();
            let before: String = buf.chars().take(cur).collect();
            let after: String = buf.chars().skip(cur).collect();
            // U+2502 box-drawing char marks the caret position mid-string
            (" r ", format!("{}\u{2502}{}", before, after))
        }
        crate::state::InputMode::Shell => (" : ", format!("{}_", state.shell_buf)),
        crate::state::InputMode::Create => {
            let label = if state.create_is_dir { " mkdir " } else { " new " };
            (label, format!("{}_", state.create_buf))
        }
        _ => return,
    };

    let line = Line::from(vec![
        Span::styled(label, Style::default().fg(t.accent).bg(t.bg_panel)),
        Span::styled(content, Style::default().fg(t.fg).bg(t.bg_panel)),
    ]);
    f.render_widget(Paragraph::new(line).style(Style::default().bg(t.bg_panel)), area);
}
