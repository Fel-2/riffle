use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::state::{App, Focus};
use crate::theme::Theme;
use crate::utils;

enum Row {
    Header(&'static str),
    Spacer,
    Target(usize),
}

pub fn render(f: &mut Frame, area: Rect, app: &mut App, theme: &Theme) {
    if area.width < 4 || area.height == 0 {
        return;
    }
    f.render_widget(
        Paragraph::new("").style(Style::default().bg(theme.panel)),
        area,
    );

    let targets = app.sidebar_targets();
    let root_index = targets
        .iter()
        .position(|(_, path)| path == std::path::Path::new("/"));
    let places_end = root_index.unwrap_or(targets.len());

    let mut rows = vec![Row::Header("PLACES")];
    for index in 0..places_end {
        rows.push(Row::Target(index));
    }
    if root_index.is_some() {
        rows.push(Row::Spacer);
        rows.push(Row::Header("DEVICES"));
        for index in places_end..targets.len() {
            rows.push(Row::Target(index));
        }
    }

    let height = area.height as usize;
    let cursor_row = rows
        .iter()
        .position(|row| matches!(row, Row::Target(index) if *index == app.sidebar_cursor));
    if let Some(cursor_row) = cursor_row {
        if cursor_row < app.sidebar_scroll {
            app.sidebar_scroll = cursor_row;
        } else if cursor_row >= app.sidebar_scroll + height {
            app.sidebar_scroll = cursor_row + 1 - height;
        }
    }
    let max_scroll = rows.len().saturating_sub(height);
    if app.sidebar_scroll > max_scroll {
        app.sidebar_scroll = max_scroll;
    }

    let current = app.current_dir();
    let focused = app.focus == Focus::Sidebar;
    let cursor = app.sidebar_cursor;
    let width = area.width.saturating_sub(2) as usize;

    for (screen_row, row) in rows
        .iter()
        .skip(app.sidebar_scroll)
        .take(height)
        .enumerate()
    {
        let y = area.y + screen_row as u16;
        let row_area = Rect {
            x: area.x + 1,
            y,
            width: width as u16,
            height: 1,
        };
        match row {
            Row::Header(title) => {
                let label = format!("{} ", title);
                let used = utils::display_width(&label);
                let mut spans = vec![Span::styled(
                    label,
                    Style::default()
                        .fg(theme.dim)
                        .bg(theme.panel)
                        .add_modifier(Modifier::BOLD),
                )];
                if used < width {
                    spans.push(Span::styled(
                        "─".repeat(width - used),
                        Style::default().fg(theme.border).bg(theme.panel),
                    ));
                }
                f.render_widget(
                    Paragraph::new(Line::from(spans)).style(Style::default().bg(theme.panel)),
                    row_area,
                );
            }
            Row::Spacer => {
                f.render_widget(
                    Paragraph::new("").style(Style::default().bg(theme.panel)),
                    row_area,
                );
            }
            Row::Target(index) => {
                let Some((name, path)) = targets.get(*index) else {
                    continue;
                };
                let is_cursor = focused && *index == cursor;
                let is_current = *path == current;
                let (bg, fg) = if is_cursor {
                    (theme.highlight, theme.bright)
                } else if is_current {
                    (theme.panel, theme.accent)
                } else {
                    (theme.panel, theme.fg)
                };
                let icon = place_icon(name, path);
                let label_width = width.saturating_sub(3);
                let label = utils::truncate(name, label_width);
                let used = utils::display_width(&label);
                let mut spans = vec![
                    Span::styled(
                        format!(" {} ", icon),
                        Style::default()
                            .fg(if is_cursor { theme.accent } else { theme.dim })
                            .bg(bg),
                    ),
                    Span::styled(
                        label,
                        Style::default()
                            .fg(fg)
                            .bg(bg)
                            .add_modifier(if is_current || is_cursor {
                                Modifier::BOLD
                            } else {
                                Modifier::empty()
                            }),
                    ),
                ];
                let padding = width.saturating_sub(3 + used);
                if padding > 0 {
                    spans.push(Span::styled(" ".repeat(padding), Style::default().bg(bg)));
                }
                f.render_widget(
                    Paragraph::new(Line::from(spans)).style(Style::default().bg(bg)),
                    row_area,
                );
                app.layout.sidebar_hits.push((
                    Rect {
                        x: area.x,
                        width: area.width,
                        height: 1,
                        ..row_area
                    },
                    *index,
                ));
            }
        }
    }
}

fn place_icon(name: &str, path: &std::path::Path) -> &'static str {
    if path == std::path::Path::new("/") {
        return "\u{f0a0}";
    }
    match name.to_ascii_lowercase().as_str() {
        "home" => "\u{f015}",
        "downloads" | "download" => "\u{f019}",
        "documents" | "document" => "\u{f02d}",
        "pictures" | "photos" => "\u{f03e}",
        "music" => "\u{f001}",
        "videos" | "video" => "\u{f03d}",
        _ => "\u{f07b}",
    }
}
