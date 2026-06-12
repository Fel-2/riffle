use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use crate::state::{AppState, InputMode};
use crate::theme::theme;
use crate::utils;

pub fn render(f: &mut Frame, area: Rect, state: &AppState) {
    let t = theme();
    let tab = state.focused_tab();
    let total = tab.display_count();
    let cursor = if total == 0 { 0 } else { tab.cursor + 1 };
    let selected = tab.selection.len();

    let mut spans: Vec<Span> = Vec::new();

    // Position
    spans.push(Span::styled(
        format!(" {}/{} ", cursor, total),
        Style::default().fg(t.accent).bg(t.bg_panel),
    ));

    // Separator
    spans.push(Span::styled("|", Style::default().fg(t.bg_highlight).bg(t.bg_panel)));

    // Dir (truncated)
    let dir = tab.current_dir.to_string_lossy().into_owned();
    let max_path = area.width.saturating_sub(40) as usize;
    let dir_str = if max_path > 3 && dir.chars().count() > max_path {
        let chars: Vec<char> = dir.chars().collect();
        let tail: String = chars[chars.len().saturating_sub(max_path)..].iter().collect();
        format!("~{}", tail)
    } else {
        dir
    };
    spans.push(Span::styled(
        format!(" {} ", dir_str),
        Style::default().fg(t.fg).bg(t.bg_panel),
    ));

    // Selected count
    if selected > 0 {
        spans.push(Span::styled("|", Style::default().fg(t.bg_highlight).bg(t.bg_panel)));
        spans.push(Span::styled(
            format!(" {} sel ", selected),
            Style::default().fg(t.yellow).bg(t.bg_panel),
        ));
    }

    // Focused entry info
    if let Some(e) = tab.cursor_entry() {
        spans.push(Span::styled("|", Style::default().fg(t.bg_highlight).bg(t.bg_panel)));
        let info = if e.is_dir {
            format!(" {} ", e.name)
        } else {
            format!(" {}  {} ", e.name, utils::format_size(e.size))
        };
        spans.push(Span::styled(info, Style::default().fg(t.fg).bg(t.bg_panel)));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = std::fs::symlink_metadata(&e.path) {
                spans.push(Span::styled(
                    format!(" {} ", utils::format_mode(meta.permissions().mode())),
                    Style::default().fg(t.dim).bg(t.bg_panel),
                ));
            }
        }
    }

    // Right side: active task progress > status message > mode indicator
    let right_text = if let Some(t) = &state.task {
        let name: String = t.current.chars().take(24).collect();
        format!("{} {}/{}  {}", t.label, (t.done + 1).min(t.total.max(1)), t.total, name)
    } else if let Some((msg, _)) = &state.status {
        msg.clone()
    } else {
        match state.input_mode {
            InputMode::Search => "SEARCH".to_string(),
            InputMode::Rename => "RENAME".to_string(),
            InputMode::GoTo   => "GOTO".to_string(),
            InputMode::Shell  => "SHELL".to_string(),
            InputMode::Create => "CREATE".to_string(),
            InputMode::Confirm => "CONFIRM".to_string(),
            InputMode::Normal => {
                let mut parts = Vec::new();
                if state.sidebar_focused {
                    parts.push("SIDEBAR");
                }
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
            Style::default().fg(t.green).bg(t.bg_panel)
        } else {
            Style::default().fg(t.accent).bg(t.bg_panel)
        };
        let left_len: usize = spans.iter().map(|s| s.content.chars().count()).sum();
        let avail = area.width as usize;
        let right_chars = right_text.chars().count() + 2;
        let (display_text, pad) = if left_len + right_chars <= avail {
            (right_text.clone(), avail - left_len - right_chars)
        } else {
            let max = avail.saturating_sub(left_len).saturating_sub(3);
            let t: String = right_text.chars().take(max).collect();
            (format!("{}~", t), 0)
        };
        spans.push(Span::styled(" ".repeat(pad), Style::default().bg(t.bg_panel)));
        spans.push(Span::styled(format!(" {} ", display_text), right_style));
    }

    f.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(t.bg_panel)),
        area,
    );
}
