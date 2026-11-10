use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;
use unicode_width::UnicodeWidthChar;

use crate::state::{App, TextInput};
use crate::theme::Theme;
use crate::utils;

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

fn input_line(input: &TextInput, theme: &Theme, max_width: usize) -> Line<'static> {
    let width_of = |ch: char| UnicodeWidthChar::width(ch).unwrap_or(0);
    let mut start = 0;
    let mut cursor_width: usize = input.chars[..input.cursor]
        .iter()
        .map(|ch| width_of(*ch))
        .sum();
    while cursor_width >= max_width && start < input.cursor {
        cursor_width -= width_of(input.chars[start]);
        start += 1;
    }
    let mut width = 0;
    let mut end = start;
    while end < input.chars.len() {
        let char_width = width_of(input.chars[end]);
        if width + char_width > max_width {
            break;
        }
        width += char_width;
        end += 1;
    }

    let mut spans = Vec::new();
    let selection = input.selection();
    for index in start..end {
        let ch = input.chars[index];
        let is_cursor = index == input.cursor;
        let selected = selection
            .map(|(sel_start, sel_end)| index >= sel_start && index < sel_end)
            .unwrap_or(false);
        let style = if is_cursor {
            Style::default()
                .fg(theme.bg)
                .bg(theme.accent)
                .add_modifier(Modifier::BOLD)
        } else if selected {
            Style::default().fg(theme.bright).bg(theme.selection)
        } else {
            Style::default().fg(theme.fg)
        };
        spans.push(Span::styled(ch.to_string(), style));
    }
    if input.cursor >= end {
        spans.push(Span::styled(
            "▏",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ));
    }
    Line::from(spans)
}

pub fn render_prompt(f: &mut Frame, app: &App, theme: &Theme) {
    let Some(prompt) = &app.prompt else {
        return;
    };
    let area = f.area();
    let width = (prompt.title.len() as u16 + 12)
        .max(52)
        .min(area.width.saturating_sub(4));
    let popup = centered(area, width, 5);
    f.render_widget(Clear, popup);
    let block = Block::default()
        .title(format!(" {} ", prompt.title))
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
    if inner.height == 0 {
        return;
    }

    let hint = if matches!(prompt.kind, crate::state::PromptKind::Filter) {
        let count = app.active_pane().view_len();
        format!("{} · {} match(es)", prompt.hint, count)
    } else {
        prompt.hint.clone()
    };
    let lines = vec![
        input_line(&prompt.input, theme, inner.width as usize),
        Line::from(Span::styled(hint, Style::default().fg(theme.dim))),
    ];
    f.render_widget(
        Paragraph::new(lines).style(Style::default().bg(theme.bg)),
        Rect {
            height: 2.min(inner.height),
            ..inner
        },
    );
}

pub fn render_confirm(f: &mut Frame, app: &App, theme: &Theme) {
    let Some(confirm) = &app.confirm else {
        return;
    };
    let area = f.area();
    let width = (confirm.message.len() as u16 + 8)
        .max(54)
        .min(area.width.saturating_sub(4));
    let popup = centered(area, width, 7);
    f.render_widget(Clear, popup);
    let color = if confirm.destructive {
        theme.red
    } else {
        theme.accent
    };
    let block = Block::default()
        .title(format!(" {} ", confirm.title))
        .title_style(Style::default().fg(color).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(color))
        .style(Style::default().bg(theme.bg));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            utils::truncate(&confirm.message, inner.width as usize),
            Style::default()
                .fg(theme.bright)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            utils::truncate(&confirm.detail, inner.width as usize),
            Style::default().fg(theme.dim),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "y / ⏎",
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" confirm      ", Style::default().fg(theme.dim)),
            Span::styled(
                "n / esc",
                Style::default()
                    .fg(theme.muted)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" cancel", Style::default().fg(theme.dim)),
        ]),
    ];
    f.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(theme.bg))
            .alignment(Alignment::Center),
        inner,
    );
}

pub fn render_palette(f: &mut Frame, app: &mut App, theme: &Theme) {
    let Some(palette) = &mut app.palette else {
        return;
    };
    let area = f.area();
    let width = area.width.saturating_sub(8).clamp(24, 76).min(area.width);
    let height = area.height.saturating_sub(6).clamp(7, 20).min(area.height);
    let popup = Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + 2,
        width,
        height,
    };
    f.render_widget(Clear, popup);
    let block = Block::default()
        .title(" Commands ")
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
    if inner.height < 3 {
        return;
    }

    let input_area = Rect { height: 1, ..inner };
    f.render_widget(
        Paragraph::new(input_line(&palette.input, theme, inner.width as usize))
            .style(Style::default().bg(theme.bg)),
        input_area,
    );
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "─".repeat(inner.width as usize),
            Style::default().fg(theme.border),
        )))
        .style(Style::default().bg(theme.bg)),
        Rect {
            y: inner.y + 1,
            height: 1,
            ..inner
        },
    );

    let list_area = Rect {
        y: inner.y + 2,
        height: inner.height - 2,
        ..inner
    };
    let visible = list_area.height as usize;
    if palette.cursor < palette.scroll {
        palette.scroll = palette.cursor;
    } else if palette.cursor >= palette.scroll + visible {
        palette.scroll = palette.cursor + 1 - visible;
    }
    if palette.filtered.is_empty() {
        f.render_widget(
            Paragraph::new(Span::styled(
                "  no matching command",
                Style::default().fg(theme.dim),
            ))
            .style(Style::default().bg(theme.bg)),
            list_area,
        );
        return;
    }

    for (screen_row, (index, _)) in palette
        .filtered
        .iter()
        .skip(palette.scroll)
        .take(visible)
        .enumerate()
    {
        let action = palette.items[*index];
        let row = Rect {
            y: list_area.y + screen_row as u16,
            height: 1,
            ..list_area
        };
        let selected = palette.scroll + screen_row == palette.cursor;
        let bg = if selected { theme.highlight } else { theme.bg };
        let shortcut = action.shortcut();
        let shortcut_width = utils::display_width(shortcut);
        let label_width = (list_area.width as usize).saturating_sub(shortcut_width + 4);
        let label = utils::truncate(action.label(), label_width);
        let used = utils::display_width(&label);
        let mut spans = vec![
            Span::styled(
                if selected { " ▸ " } else { "   " },
                Style::default()
                    .fg(if selected { theme.accent } else { theme.dim })
                    .bg(bg),
            ),
            Span::styled(
                label,
                Style::default()
                    .fg(if selected { theme.bright } else { theme.fg })
                    .bg(bg)
                    .add_modifier(if selected {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
            Span::styled(
                " ".repeat(
                    (list_area.width as usize).saturating_sub(3 + used + shortcut_width + 1),
                ),
                Style::default().bg(bg),
            ),
            Span::styled(
                shortcut.to_string(),
                Style::default()
                    .fg(if selected { theme.accent } else { theme.dim })
                    .bg(bg),
            ),
            Span::styled(" ", Style::default().bg(bg)),
        ];
        if !selected {
            spans.push(Span::styled(" ", Style::default().fg(theme.dim).bg(bg)));
        }
        f.render_widget(
            Paragraph::new(Line::from(spans)).style(Style::default().bg(bg)),
            row,
        );
    }
}

pub fn render_info(f: &mut Frame, app: &mut App, theme: &Theme) {
    let Some(info) = &mut app.info else {
        return;
    };
    let area = f.area();
    let width = area.width.saturating_sub(8).clamp(30, 76).min(area.width);
    let height = (info.rows.len() as u16 + 4)
        .min(area.height.saturating_sub(4))
        .max(6)
        .min(area.height);
    let popup = centered(area, width, height);
    f.render_widget(Clear, popup);
    let block = Block::default()
        .title(format!(" {} ", info.title))
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
    if inner.height == 0 {
        return;
    }
    let visible = inner.height as usize;
    let max_scroll = info.rows.len().saturating_sub(visible);
    if info.scroll > max_scroll {
        info.scroll = max_scroll;
    }
    let label_width = 12usize;
    let lines: Vec<Line> = info
        .rows
        .iter()
        .skip(info.scroll)
        .take(visible)
        .map(|(label, value)| {
            Line::from(vec![
                Span::styled(
                    utils::pad_end(label, label_width),
                    Style::default().fg(theme.dim),
                ),
                Span::styled(
                    utils::truncate(value, (inner.width as usize).saturating_sub(label_width)),
                    Style::default().fg(theme.fg),
                ),
            ])
        })
        .collect();
    f.render_widget(
        Paragraph::new(lines).style(Style::default().bg(theme.bg)),
        inner,
    );
}
