use std::path::PathBuf;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use crate::state::AppState;
use crate::theme::theme;

/// Flat, ordered list of navigable sidebar targets (bookmarks then drives).
/// The order here must match the render order so `sidebar_cursor` lines up.
pub fn nav_targets(state: &AppState) -> Vec<(String, PathBuf)> {
    let mut out: Vec<(String, PathBuf)> = Vec::new();

    // Bookmarks
    for (name, path) in &state.bookmarks {
        if path.exists() {
            out.push((name.clone(), path.clone()));
        }
    }

    // Drives: root + mounted media
    out.push(("/ (root)".into(), PathBuf::from("/")));
    for base in ["/media", "/mnt", "/run/media"] {
        let base_path = PathBuf::from(base);
        if !base_path.exists() { continue; }
        let search = if let Ok(user) = std::env::var("USER") {
            let u = base_path.join(&user);
            if u.exists() { u } else { base_path }
        } else { base_path };
        if let Ok(rd) = std::fs::read_dir(&search) {
            for entry in rd.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    let mn = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
                    out.push((mn, p));
                }
            }
        }
    }

    out
}

pub fn render(f: &mut Frame, area: Rect, state: &AppState) {
    let t = theme();
    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(t.bg_highlight))
        .style(Style::default().bg(t.bg_panel));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let cur_dir = state.focused_tab().current_dir.clone();
    let targets = nav_targets(state);
    let focused = state.sidebar_focused;
    let cursor = state.sidebar_cursor;

    let mut y = inner.y;
    let max_y = inner.y + inner.height;
    let w = inner.width as usize;

    macro_rules! row {
        ($text:expr, $style:expr) => {
            if y < max_y {
                let s = format!("{:<width$}", $text, width = w);
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(s, $style))),
                    Rect { y, height: 1, ..inner },
                );
                y += 1;
            }
        };
    }

    // Style for a navigable target at flat index `idx`
    let target_style = |idx: usize, path: &PathBuf| {
        if focused && idx == cursor {
            Style::default().fg(t.bg_panel).bg(t.accent).add_modifier(Modifier::BOLD)
        } else if *path == cur_dir {
            Style::default().fg(t.accent).bg(t.bg_selection).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(t.fg).bg(t.bg_panel)
        }
    };

    // Split point: bookmarks are everything before the "/ (root)" entry.
    let n_bookmarks = targets.iter().position(|(n, _)| n == "/ (root)").unwrap_or(targets.len());

    // Bookmarks
    let bm_label = if focused { " Bookmarks  [j/k Enter, Esc]" } else { " Bookmarks" };
    row!(bm_label, Style::default().fg(t.accent).add_modifier(Modifier::BOLD));
    for (idx, (name, path)) in targets.iter().enumerate().take(n_bookmarks) {
        row!(format!("  {}", name), target_style(idx, path));
    }

    if y < max_y {
        let sep = "-".repeat(w);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(sep, Style::default().fg(t.bg_highlight)))),
            Rect { y, height: 1, ..inner },
        );
        y += 1;
    }

    // Drives
    row!(" Drives", Style::default().fg(t.accent).add_modifier(Modifier::BOLD));
    for (idx, (name, path)) in targets.iter().enumerate().skip(n_bookmarks) {
        row!(format!("  {}", name), target_style(idx, path));
    }

    if y < max_y {
        let sep = "-".repeat(w);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(sep, Style::default().fg(t.bg_highlight)))),
            Rect { y, height: 1, ..inner },
        );
        y += 1;
    }

    // Path breadcrumbs (display only, not navigable)
    row!(" Path", Style::default().fg(t.accent).add_modifier(Modifier::BOLD));
    let ancestors: Vec<_> = cur_dir.ancestors().map(|p| p.to_path_buf()).collect();
    for ancestor in ancestors.iter().rev().take(8) {
        let name = ancestor.file_name().and_then(|n| n.to_str()).unwrap_or("/");
        let sel = ancestor == &cur_dir;
        let style = if sel {
            Style::default().fg(t.accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(t.dim)
        };
        row!(format!("  {}", name), style);
    }
}
