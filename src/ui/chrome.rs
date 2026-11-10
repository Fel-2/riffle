use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::state::{App, Focus, Mode};
use crate::theme::Theme;
use crate::utils;

pub fn render_tabbar(f: &mut Frame, area: Rect, app: &mut App, theme: &Theme) {
    if area.width == 0 {
        return;
    }
    let mut spans = vec![Span::styled(
        " riffle ",
        Style::default()
            .fg(theme.accent)
            .bg(theme.panel)
            .add_modifier(Modifier::BOLD),
    )];
    let mut x = area.x + 8;
    let right_width = if app.dual { 8 } else { 0 };
    let max_x = area.x + area.width.saturating_sub(right_width);
    let active = app.tab;

    for (index, tab) in app.tabs.iter().enumerate() {
        let pane = if app.dual && tab.focus_right { 1 } else { 0 };
        let dir = &tab.panes[pane].dir;
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "/".into());
        let title = format!(" {} {} ", index + 1, name);
        let width = utils::display_width(&title) as u16;
        if x + width > max_x {
            break;
        }
        let style = if index == active {
            Style::default()
                .fg(theme.bright)
                .bg(theme.highlight)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.dim).bg(theme.panel)
        };
        spans.push(Span::styled(title, style));
        app.layout.tab_hits.push((
            Rect {
                x,
                y: area.y,
                width,
                height: 1,
            },
            index,
        ));
        x += width;
    }

    f.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(theme.panel)),
        area,
    );

    if app.dual {
        let badge = " DUAL ";
        let bx = area.x + area.width.saturating_sub(badge.len() as u16);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                badge,
                Style::default()
                    .fg(theme.bg)
                    .bg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )))
            .style(Style::default().bg(theme.panel)),
            Rect {
                x: bx,
                y: area.y,
                width: area.width.saturating_sub(bx - area.x),
                height: 1,
            },
        );
    }
}

pub fn render_status(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    if area.width == 0 {
        return;
    }
    let pane = app.active_pane();
    let total = pane.view_len();
    let cursor = if total == 0 { 0 } else { pane.cursor + 1 };

    let mut left: Vec<Span> = Vec::new();
    left.push(Span::styled(
        format!(" {cursor}/{total} "),
        Style::default()
            .fg(theme.accent)
            .bg(theme.panel)
            .add_modifier(Modifier::BOLD),
    ));
    left.push(Span::styled(
        format!("{} files · {} dirs", pane.file_count, pane.dir_count),
        Style::default().fg(theme.muted).bg(theme.panel),
    ));
    left.push(Span::styled(
        format!(" · {}", utils::format_size(pane.total_size)),
        Style::default().fg(theme.dim).bg(theme.panel),
    ));
    if !pane.selection.is_empty() {
        left.push(Span::styled(
            format!(" · {} selected", pane.selection.len()),
            Style::default()
                .fg(theme.yellow)
                .bg(theme.panel)
                .add_modifier(Modifier::BOLD),
        ));
    }
    if let Some(clipboard) = &app.clipboard {
        left.push(Span::styled(
            format!(
                " · {} {}",
                if clipboard.cut { "cut" } else { "copy" },
                clipboard.paths.len()
            ),
            Style::default().fg(theme.cyan).bg(theme.panel),
        ));
    }
    if !pane.filter.is_empty() {
        left.push(Span::styled(
            format!(" · /{}", pane.filter),
            Style::default().fg(theme.accent).bg(theme.panel),
        ));
    }
    if app.show_hidden {
        left.push(Span::styled(
            " · hidden",
            Style::default().fg(theme.dim).bg(theme.panel),
        ));
    }

    let right = right_status(app, theme);
    let (left, left_width) = clip_spans(left, area.width as usize);
    let right_width = utils::display_width(&right.0);
    let mut spans = left;
    if left_width + right_width < area.width as usize {
        spans.push(Span::styled(
            " ".repeat(area.width as usize - left_width - right_width),
            Style::default().bg(theme.panel),
        ));
        spans.push(Span::styled(right.0, right.1));
    } else if right_width > 0 {
        let available = area.width as usize;
        let clipped = utils::truncate(&right.0, available);
        let width = utils::display_width(&clipped);
        spans.push(Span::styled(
            " ".repeat(available.saturating_sub(width)),
            Style::default().bg(theme.panel),
        ));
        spans.push(Span::styled(clipped, right.1));
    }
    f.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(theme.panel)),
        area,
    );
}

fn right_status(app: &App, theme: &Theme) -> (String, Style) {
    if let Some(task) = &app.task {
        let ratio = if task.total == 0 {
            0.0
        } else {
            (task.done as f64 / task.total as f64).clamp(0.0, 1.0)
        };
        let filled = (ratio * 10.0).round() as usize;
        let bar = format!(
            "{}{}",
            "█".repeat(filled),
            "░".repeat(10usize.saturating_sub(filled))
        );
        let current = utils::truncate(&task.current, 20);
        let elapsed = task.started.elapsed().as_secs();
        let text = format!(
            " {} {}/{} {} {} · {}s ",
            task.label,
            (task.done + 1).min(task.total.max(1)),
            task.total,
            bar,
            current,
            elapsed
        );
        return (
            text,
            Style::default()
                .fg(theme.accent)
                .bg(theme.panel)
                .add_modifier(Modifier::BOLD),
        );
    }
    if let Some(status) = &app.status {
        let color = match status.kind {
            crate::state::StatusKind::Info => theme.fg,
            crate::state::StatusKind::Success => theme.green,
            crate::state::StatusKind::Warn => theme.yellow,
            crate::state::StatusKind::Error => theme.red,
        };
        return (
            format!(" {} ", status.text),
            Style::default()
                .fg(color)
                .bg(theme.panel)
                .add_modifier(Modifier::BOLD),
        );
    }
    if app.mode == Mode::Normal && app.focus == Focus::Files && !app.dual {
        if let Some(latest) = &app.update.latest {
            return (
                format!(" ↑ {latest} available "),
                Style::default()
                    .fg(theme.yellow)
                    .bg(theme.panel)
                    .add_modifier(Modifier::BOLD),
            );
        }
        return (
            format!(" v{} ", env!("CARGO_PKG_VERSION")),
            Style::default().fg(theme.dim).bg(theme.panel),
        );
    }
    let label = match app.mode {
        Mode::Normal => match app.focus {
            Focus::Files => {
                if app.dual {
                    if app.active_tab().focus_right {
                        " DUAL · right "
                    } else {
                        " DUAL · left "
                    }
                } else {
                    ""
                }
            }
            Focus::Sidebar => " SIDEBAR ",
            Focus::Preview => " PREVIEW ",
        },
        mode => crate::ui::mode_label(mode),
    };
    (
        label.to_string(),
        Style::default()
            .fg(theme.accent)
            .bg(theme.panel)
            .add_modifier(Modifier::BOLD),
    )
}

pub fn render_hints(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    if area.width == 0 {
        return;
    }
    let hints: &[(&str, &str)] = if app.task.is_some() {
        &[("esc", "cancel")]
    } else {
        match app.mode {
            Mode::Prompt => &[
                ("⏎", "apply"),
                ("esc", "cancel"),
                ("ctrl+u", "clear"),
                ("ctrl+w", "delete word"),
            ],
            Mode::Confirm => &[("y", "confirm"), ("n / esc", "cancel")],
            Mode::Palette => &[("↑↓", "select"), ("⏎", "run"), ("esc", "close")],
            Mode::Help => &[("j/k", "scroll"), ("esc", "close")],
            Mode::Info => &[("j/k", "scroll"), ("esc", "close")],
            Mode::Normal => match app.focus {
                Focus::Sidebar => &[("j/k", "move"), ("⏎", "open"), ("esc", "back")],
                Focus::Preview => &[
                    ("j/k", "scroll"),
                    ("h/l", "pan"),
                    ("g/G", "top/bottom"),
                    ("esc", "back"),
                ],
                Focus::Files => {
                    if app.dual {
                        &[
                            ("j/k", "move"),
                            ("⏎", "open"),
                            ("space", "select"),
                            ("tab", "pane"),
                            ("y", "copy"),
                            ("p", "paste"),
                            ("/", "filter"),
                            ("?", "help"),
                        ]
                    } else {
                        &[
                            ("j/k", "move"),
                            ("⏎", "open"),
                            ("space", "select"),
                            ("y", "copy"),
                            ("p", "paste"),
                            ("/", "filter"),
                            ("P", "preview"),
                            ("?", "help"),
                        ]
                    }
                }
            },
        }
    };

    let mut spans = Vec::new();
    let mut width = 0usize;
    for (key, label) in hints {
        let key_width = utils::display_width(key);
        let label_width = utils::display_width(label);
        let needed = key_width + 1 + label_width + 2;
        if width + needed > area.width as usize {
            break;
        }
        if !spans.is_empty() {
            spans.push(Span::styled("  ", Style::default().bg(theme.panel)));
            width += 2;
        }
        spans.push(Span::styled(
            (*key).to_string(),
            Style::default()
                .fg(theme.accent)
                .bg(theme.panel)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(" ", Style::default().bg(theme.panel)));
        spans.push(Span::styled(
            (*label).to_string(),
            Style::default().fg(theme.dim).bg(theme.panel),
        ));
        width += key_width + 1 + label_width;
    }

    f.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(theme.panel)),
        area,
    );
}

fn clip_spans(spans: Vec<Span<'static>>, max: usize) -> (Vec<Span<'static>>, usize) {
    let mut out = Vec::new();
    let mut width = 0;
    for span in spans {
        if width >= max {
            break;
        }
        let text = span.content.to_string();
        let remaining = max - width;
        let clipped = utils::truncate(&text, remaining);
        width += utils::display_width(&clipped);
        out.push(Span::styled(clipped, span.style));
    }
    (out, width)
}
