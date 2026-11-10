use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::filesystem::FileEntry;
use crate::state::{App, Column, Focus, Pane, PaneLayout};
use crate::theme::Theme;
use crate::utils;
use crate::utils::DateStyle;

use super::{class_color, git_style, highlighted_name, truncate_with_matches};

struct Columns {
    bar: u16,
    icon: u16,
    git: u16,
    name: u16,
    size: u16,
    date: u16,
    gap: u16,
}

fn columns(width: u16, icons: bool) -> Columns {
    let bar = 1;
    let icon = if icons { 2 } else { 0 };
    let git = 2;
    let size = if width >= 42 { 8 } else { 0 };
    let date = if width >= 62 { 12 } else { 0 };
    let gap = if size > 0 || date > 0 { 1 } else { 0 };
    let name = width
        .saturating_sub(bar + icon + git + size + date + gap)
        .max(3);
    Columns {
        bar,
        icon,
        git,
        name,
        size,
        date,
        gap,
    }
}

pub fn render_pane(
    f: &mut Frame,
    area: Rect,
    app: &mut App,
    pane_index: usize,
    theme: &Theme,
) -> PaneLayout {
    let tab_index = app.tab;
    let focused = app.focus == Focus::Files && app.pane_index() == pane_index;
    let icons = app.config.ui.icons;
    let date_style = app.date_style();
    let dual = app.dual;
    let pane = &mut app.tabs[tab_index].panes[pane_index];

    let header_height = if area.height >= 4 { 2 } else { 1 };
    let header_area = Rect {
        height: header_height,
        ..area
    };
    let list_area = Rect {
        y: area.y + header_height,
        height: area.height.saturating_sub(header_height),
        ..area
    };
    pane.height = list_area.height as usize;
    pane.ensure_scroll();

    let cols = columns(area.width, icons);
    let (crumbs, column_hits) = render_header(
        f,
        header_area,
        pane,
        focused,
        dual,
        pane_index,
        &cols,
        theme,
    );
    render_list(f, list_area, pane, focused, &cols, date_style, icons, theme);

    PaneLayout {
        header: header_area,
        list: list_area,
        crumbs,
        columns: column_hits,
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn render_header(
    f: &mut Frame,
    area: Rect,
    pane: &Pane,
    focused: bool,
    dual: bool,
    pane_index: usize,
    cols: &Columns,
    theme: &Theme,
) -> (Vec<(Rect, std::path::PathBuf)>, Vec<(Rect, Column)>) {
    let mut crumbs = Vec::new();
    let mut spans: Vec<Span> = Vec::new();
    let mut x = area.x;

    if dual {
        let label = format!("{} ", pane_index + 1);
        let width = utils::display_width(&label) as u16;
        spans.push(Span::styled(
            label,
            Style::default()
                .fg(if focused { theme.accent } else { theme.dim })
                .bg(theme.panel)
                .add_modifier(Modifier::BOLD),
        ));
        x += width;
    }

    let right = header_right(pane, theme);
    let right_width = utils::display_width(&right.0) as u16;
    let available = area.width.saturating_sub(x - area.x + right_width + 1);
    let components = path_components(&pane.dir);
    let mut chosen: Vec<&(String, std::path::PathBuf)> = Vec::new();
    let mut used = 0u16;
    for component in components.iter().rev() {
        let width = utils::display_width(&component.0) as u16;
        let extra = if chosen.is_empty() { width } else { width + 3 };
        if used + extra > available {
            break;
        }
        used += extra;
        chosen.push(component);
    }
    chosen.reverse();
    let elided = chosen.len() < components.len();
    if elided && available > 2 {
        spans.push(Span::styled(
            "…",
            Style::default().fg(theme.dim).bg(theme.panel),
        ));
        x += 1;
        if !chosen.is_empty() {
            spans.push(Span::styled(
                " › ",
                Style::default().fg(theme.border).bg(theme.panel),
            ));
            x += 3;
        }
    }
    let last_index = chosen.len().saturating_sub(1);
    for (index, (label, path)) in chosen.iter().enumerate() {
        if index > 0 {
            let separator = if index == 1 && chosen[0].0 == "/" {
                ""
            } else {
                " › "
            };
            if !separator.is_empty() {
                spans.push(Span::styled(
                    separator,
                    Style::default().fg(theme.border).bg(theme.panel),
                ));
                x += 3;
            }
        }
        let is_last = index == last_index;
        let style = if is_last {
            Style::default()
                .fg(if focused { theme.bright } else { theme.fg })
                .bg(theme.panel)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.muted).bg(theme.panel)
        };
        let width = utils::display_width(label) as u16;
        crumbs.push((
            Rect {
                x,
                y: area.y,
                width,
                height: 1,
            },
            path.clone(),
        ));
        spans.push(Span::styled(label.clone(), style));
        x += width;
    }

    if !right.0.is_empty() {
        let pad = area
            .x
            .saturating_add(area.width)
            .saturating_sub(x + right_width + 1);
        spans.push(Span::styled(
            " ".repeat(pad as usize),
            Style::default().bg(theme.panel),
        ));
        spans.push(Span::styled(right.0, right.1));
    }
    f.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(theme.panel)),
        Rect { height: 1, ..area },
    );

    let mut column_hits = Vec::new();
    if area.height < 2 {
        return (crumbs, column_hits);
    }

    let mut head: Vec<Span> = Vec::new();
    let left_pad = cols.bar + cols.icon + cols.git;
    head.push(Span::styled(
        " ".repeat(left_pad as usize),
        Style::default().bg(theme.panel),
    ));
    let name_style = sort_style(pane, crate::filesystem::SortKey::Name, theme);
    head.push(Span::styled("NAME".to_string(), name_style));
    let arrow = if pane.ascending { " ↑" } else { " ↓" };
    if pane.sort == crate::filesystem::SortKey::Name {
        head.push(Span::styled(arrow, name_style));
    }
    let name_used = utils::display_width("NAME")
        + if pane.sort == crate::filesystem::SortKey::Name {
            2
        } else {
            0
        };
    head.push(Span::styled(
        " ".repeat((cols.name as usize).saturating_sub(name_used)),
        Style::default().bg(theme.panel),
    ));
    if cols.size > 0 {
        head.push(Span::styled(
            " ".repeat(cols.gap as usize),
            Style::default().bg(theme.panel),
        ));
        let style = sort_style(pane, crate::filesystem::SortKey::Size, theme);
        let label = if pane.sort == crate::filesystem::SortKey::Size {
            format!("SIZE{}", if pane.ascending { "↑" } else { "↓" })
        } else {
            "SIZE".into()
        };
        head.push(Span::styled(
            utils::pad_start(&label, cols.size as usize),
            style,
        ));
    }
    if cols.date > 0 {
        let style = sort_style(pane, crate::filesystem::SortKey::Modified, theme);
        let label = if pane.sort == crate::filesystem::SortKey::Modified {
            format!("MODIFIED{}", if pane.ascending { "↑" } else { "↓" })
        } else {
            "MODIFIED".into()
        };
        head.push(Span::styled(
            utils::pad_start(&label, cols.date as usize),
            style,
        ));
    }
    f.render_widget(
        Paragraph::new(Line::from(head)).style(Style::default().bg(theme.panel)),
        Rect {
            y: area.y + 1,
            height: 1,
            ..area
        },
    );

    let name_x = area.x + cols.bar + cols.icon + cols.git;
    column_hits.push((
        Rect {
            x: area.x,
            y: area.y + 1,
            width: left_pad + cols.name,
            height: 1,
        },
        Column::Name,
    ));
    if cols.size > 0 {
        let x = name_x + cols.name + cols.gap;
        column_hits.push((
            Rect {
                x,
                y: area.y + 1,
                width: cols.size,
                height: 1,
            },
            Column::Size,
        ));
    }
    if cols.date > 0 {
        let x = name_x + cols.name + cols.gap + cols.size;
        column_hits.push((
            Rect {
                x,
                y: area.y + 1,
                width: cols.date,
                height: 1,
            },
            Column::Modified,
        ));
    }

    (crumbs, column_hits)
}

fn sort_style(pane: &Pane, key: crate::filesystem::SortKey, theme: &Theme) -> Style {
    if pane.sort == key {
        Style::default()
            .fg(theme.accent)
            .bg(theme.panel)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.dim).bg(theme.panel)
    }
}

fn header_right(pane: &Pane, theme: &Theme) -> (String, Style) {
    let mut parts: Vec<String> = Vec::new();
    if !pane.filter.is_empty() {
        parts.push(format!("filter: {}", pane.filter));
    }
    parts.push(format!(
        "{} {}",
        pane.sort.label(),
        if pane.ascending { "↑" } else { "↓" }
    ));
    if let Some(repo) = &pane.repo {
        parts.push(repo.branch.clone());
    }
    if pane.loading {
        parts.push("↻".into());
    }
    let text = parts.join(" · ");
    let style = if !pane.filter.is_empty() {
        Style::default()
            .fg(theme.yellow)
            .bg(theme.panel)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.dim).bg(theme.panel)
    };
    (text, style)
}

fn path_components(path: &std::path::Path) -> Vec<(String, std::path::PathBuf)> {
    let home = utils::home_dir();
    let mut out = Vec::new();
    if let Ok(rest) = path.strip_prefix(&home) {
        out.push(("~".to_string(), home.clone()));
        let mut current = home;
        for component in rest.components() {
            current.push(component);
            out.push((
                component.as_os_str().to_string_lossy().into_owned(),
                current.clone(),
            ));
        }
    } else {
        out.push(("/".to_string(), std::path::PathBuf::from("/")));
        let mut current = std::path::PathBuf::from("/");
        for component in path.components().skip(1) {
            current.push(component);
            out.push((
                component.as_os_str().to_string_lossy().into_owned(),
                current.clone(),
            ));
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn render_list(
    f: &mut Frame,
    area: Rect,
    pane: &Pane,
    focused: bool,
    cols: &Columns,
    date_style: DateStyle,
    icons: bool,
    theme: &Theme,
) {
    if pane.view.is_empty() {
        render_empty(f, area, pane, theme);
        return;
    }
    let height = area.height as usize;
    for row in 0..height {
        let display_index = pane.scroll + row;
        let Some(view) = pane.view.get(display_index) else {
            break;
        };
        let Some(entry) = pane.entries.get(view.index) else {
            continue;
        };
        let row_area = Rect {
            y: area.y + row as u16,
            height: 1,
            ..area
        };
        let is_cursor = display_index == pane.cursor;
        let is_selected = pane.selection.contains(&entry.path);
        let git = pane.git.get(&entry.name).copied();
        let bg = if is_cursor {
            theme.highlight
        } else if is_selected {
            theme.selection
        } else {
            theme.bg
        };
        let line = row_line(
            entry,
            view,
            git,
            is_cursor,
            is_selected,
            focused,
            cols,
            date_style,
            icons,
            bg,
            theme,
        );
        f.render_widget(
            Paragraph::new(line).style(Style::default().bg(bg)),
            row_area,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn row_line(
    entry: &FileEntry,
    view: &crate::filesystem::ViewRow,
    git: Option<crate::proc::GitStatus>,
    is_cursor: bool,
    is_selected: bool,
    focused: bool,
    cols: &Columns,
    date_style: DateStyle,
    icons: bool,
    bg: ratatui::style::Color,
    theme: &Theme,
) -> Line<'static> {
    let mut spans = Vec::new();
    let dim = if focused {
        Modifier::empty()
    } else {
        Modifier::DIM
    };

    let marker = if is_cursor {
        if focused {
            Span::styled("▌", Style::default().fg(theme.accent).bg(bg))
        } else {
            Span::styled("│", Style::default().fg(theme.muted).bg(bg))
        }
    } else if is_selected {
        Span::styled("✓", Style::default().fg(theme.green).bg(bg))
    } else {
        Span::styled(" ", Style::default().bg(bg))
    };
    spans.push(marker);

    if icons {
        let color = class_color(entry.class, entry.is_dir, entry.hidden, entry.broken, theme);
        spans.push(Span::styled(
            format!("{} ", entry.icon()),
            Style::default().fg(color).bg(bg).add_modifier(dim),
        ));
    }

    let git_span = match git {
        Some(status) => Span::styled(
            format!("{} ", status.marker()),
            git_style(status, theme).bg(bg),
        ),
        None => Span::styled("  ", Style::default().bg(bg)),
    };
    spans.push(git_span);

    let name_color = class_color(entry.class, entry.is_dir, entry.hidden, entry.broken, theme);
    let mut base = Style::default().fg(name_color).bg(bg).add_modifier(dim);
    if is_selected {
        base = base.add_modifier(Modifier::BOLD);
    }
    let name_width = cols.name as usize;
    let (shown, matches) = truncate_with_matches(&entry.name, name_width, &view.matches);
    let mut used = utils::display_width(&shown);
    spans.extend(highlighted_name(&shown, &matches, base, theme));

    if entry.is_symlink {
        if let Some(target) = &entry.symlink_target {
            let remaining = name_width.saturating_sub(used);
            if remaining > 4 {
                let text = utils::truncate(&format!(" → {}", target.display()), remaining);
                used += utils::display_width(&text);
                spans.push(Span::styled(
                    text,
                    Style::default().fg(theme.dim).bg(bg).add_modifier(dim),
                ));
            }
        }
    }
    if used < name_width {
        spans.push(Span::styled(
            " ".repeat(name_width - used),
            Style::default().bg(bg),
        ));
    }

    if cols.size > 0 {
        spans.push(Span::styled(
            " ".repeat(cols.gap as usize),
            Style::default().bg(bg),
        ));
        let size = if entry.is_dir {
            "—".to_string()
        } else {
            utils::format_size(entry.size)
        };
        spans.push(Span::styled(
            utils::pad_start(&size, cols.size as usize),
            Style::default().fg(theme.muted).bg(bg),
        ));
    }
    if cols.date > 0 {
        let date = entry
            .modified
            .map(|time| utils::format_time(time, date_style))
            .unwrap_or_else(|| "—".into());
        spans.push(Span::styled(
            utils::pad_start(
                &utils::truncate(&date, cols.date as usize),
                cols.date as usize,
            ),
            Style::default().fg(theme.dim).bg(bg),
        ));
    }
    Line::from(spans)
}

fn render_empty(f: &mut Frame, area: Rect, pane: &Pane, theme: &Theme) {
    let lines: Vec<Line> = if let Some(error) = &pane.error {
        vec![
            Line::from(Span::styled(error.clone(), Style::default().fg(theme.red))),
            Line::from(""),
            Line::from(Span::styled("R to retry", Style::default().fg(theme.dim))),
        ]
    } else if pane.loading {
        vec![Line::from(Span::styled(
            "Loading…",
            Style::default().fg(theme.dim),
        ))]
    } else if !pane.filter.is_empty() {
        vec![
            Line::from(Span::styled(
                format!("No matches for “{}”", pane.filter),
                Style::default().fg(theme.muted),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "esc clear filter",
                Style::default().fg(theme.dim),
            )),
        ]
    } else {
        vec![
            Line::from(Span::styled(
                "Empty directory",
                Style::default()
                    .fg(theme.muted)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "a new file · A new directory · p paste",
                Style::default().fg(theme.dim),
            )),
        ]
    };
    let height = lines.len() as u16;
    let y = area.y + area.height.saturating_sub(height) / 2;
    f.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(theme.bg))
            .alignment(Alignment::Center),
        Rect {
            y,
            height: height.min(area.height),
            ..area
        },
    );
}
