use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use crate::filesystem::FileType;
use crate::state::{AppState, InputMode};
use crate::theme::theme;
use crate::utils;

fn entry_color(is_dir: bool, is_hidden: bool, ft: &FileType) -> Color {
    let t = theme();
    if is_dir    { return t.cyan; }
    if is_hidden { return t.dim; }
    match ft {
        FileType::Image      => t.purple,
        FileType::Executable => t.green,
        FileType::Archive    => t.orange,
        FileType::Audio      => t.yellow,
        FileType::Code       => t.accent,
        _                    => t.fg,
    }
}

fn git_char_style(ch: char) -> Style {
    let t = theme();
    match ch {
        'M' => Style::default().fg(t.yellow),
        'A' => Style::default().fg(t.green),
        'D' => Style::default().fg(t.red),
        'R' => Style::default().fg(t.accent),
        'U' => Style::default().fg(t.orange),
        '?' => Style::default().fg(t.dim),
        _   => Style::default().fg(t.dim),
    }
}

pub fn render_pane(f: &mut Frame, area: Rect, state: &mut AppState, tab_idx: usize, focused: bool) {
    let t = theme();
    let border_style = if focused {
        Style::default().fg(t.accent)
    } else {
        Style::default().fg(t.bg_highlight)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .style(Style::default().bg(t.bg));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 2 { return; }

    // Header row (1 line)
    let header_area = Rect { height: 1, ..inner };
    let list_area = Rect { y: inner.y + 1, height: inner.height - 1, ..inner };

    render_header(f, header_area, state, tab_idx);
    render_list(f, list_area, state, tab_idx);
}

fn render_header(f: &mut Frame, area: Rect, state: &AppState, tab_idx: usize) {
    let t = theme();
    let tab = &state.tabs[tab_idx];
    use crate::state::SortBy;
    let sort_label = match tab.sort_by {
        SortBy::Name => "Name",
        SortBy::Size => "Size",
        SortBy::Date => "Date",
        SortBy::Type => "Type",
    };
    let arrow = if tab.sort_ascending { "^" } else { "v" };
    let query = if !tab.search_query.is_empty() {
        format!("  filter: {}", tab.search_query)
    } else {
        String::new()
    };
    let header = format!("    Name{}  [{} {}]", query, sort_label, arrow);
    let right = format!("{:>10}  {:<12}", "Size", "Modified");

    // Calculate spacing
    let left_width = area.width.saturating_sub(right.len() as u16 + 1);
    let left_w = left_width as usize;
    let left = if header.chars().count() > left_w {
        header.chars().take(left_w).collect()
    } else {
        let pad = left_w.saturating_sub(header.chars().count());
        format!("{}{}", header, " ".repeat(pad))
    };

    let line = Line::from(vec![
        Span::styled(left, Style::default().fg(t.accent).add_modifier(Modifier::BOLD)),
        Span::styled(right, Style::default().fg(t.accent).add_modifier(Modifier::BOLD)),
    ]);
    f.render_widget(Paragraph::new(line).style(Style::default().bg(t.bg_panel)), area);
}

fn render_list(f: &mut Frame, area: Rect, state: &mut AppState, tab_idx: usize) {
    let t = theme();
    let visible = area.height as usize;

    // Update scroll to keep cursor visible
    state.tabs[tab_idx].ensure_scroll(visible);
    let scroll = state.tabs[tab_idx].scroll;
    let cursor = state.tabs[tab_idx].cursor;
    let count = state.tabs[tab_idx].display_count();

    let is_renaming_mode = matches!(state.input_mode, InputMode::Rename);

    for row in 0..visible {
        let display_idx = scroll + row;
        if display_idx >= count { break; }

        let y = area.y + row as u16;
        let row_area = Rect { y, height: 1, ..area };

        // Extract entry data
        let entry_data = {
            let tab = &state.tabs[tab_idx];
            tab.get_display_entry(display_idx).map(|e| (
                e.path.clone(),
                e.name.clone(),
                e.is_dir,
                e.is_hidden,
                e.file_type.clone(),
                e.size,
                e.modified,
                e.type_icon(),
                e.is_symlink,
                e.symlink_target.clone(),
            ))
        };
        let Some((path, name, is_dir, is_hidden, file_type, size, modified, type_icon, is_symlink, symlink_target)) = entry_data else { continue; };

        // Get git status char
        let git_ch = {
            let tab = &state.tabs[tab_idx];
            tab.git_status.get(&name).copied().unwrap_or(' ')
        };

        let is_cursor = display_idx == cursor;
        let is_selected = state.tabs[tab_idx].selection.contains(&path);

        let bg = if is_cursor && is_selected { t.bg_selection }
            else if is_cursor   { t.bg_highlight }
            else if is_selected { t.bg_selection }
            else                { t.bg };

        let name_color = entry_color(is_dir, is_hidden, &file_type);

        // Check if this row is being renamed
        let is_renaming = is_renaming_mode && is_cursor
            && state.tabs[tab_idx].renaming.as_ref().map(|(i, _, _)| *i == display_idx).unwrap_or(false);

        let line = if is_renaming {
            let (buf, cur) = state.tabs[tab_idx].renaming.as_ref()
                .map(|(_, b, c)| (b.clone(), *c)).unwrap_or_default();
            let before: String = buf.chars().take(cur).collect();
            let after: String = buf.chars().skip(cur).collect();
            Line::from(vec![
                Span::styled(format!(" {} ", type_icon), Style::default().fg(t.accent).bg(bg)),
                Span::styled(format!("{}\u{2502}{}", before, after), Style::default().fg(t.yellow).bg(bg).add_modifier(Modifier::BOLD)),
            ])
        } else {
            let sel_marker = if is_selected { "*" } else { " " };
            let size_str = if is_dir { "         -".to_string() } else { format!("{:>10}", utils::format_size(size)) };
            let date_str = format!("{:<12}", utils::format_time(modified));

            // Truncate name to fit (icon ~2 + git_ch ~2 + sel ~1 = 5 extra cols)
            // We subtract 32 total: 30 for size/date + 2 for git indicator
            let max_name = area.width.saturating_sub(32) as usize;

            // Build display name with symlink info
            let base_name = if is_symlink {
                if let Some(target) = &symlink_target {
                    format!("{} -> {}", name, target.display())
                } else {
                    name.clone()
                }
            } else {
                name.clone()
            };

            let display_name = if max_name > 3 && base_name.chars().count() > max_name {
                let truncated: String = base_name.chars().take(max_name.saturating_sub(1)).collect();
                format!("{}~", truncated)
            } else {
                base_name
            };
            let char_count = display_name.chars().count();
            let pad = max_name.max(1).saturating_sub(char_count);
            let padded_name = format!("{}{}", display_name, " ".repeat(pad));

            let git_str = format!("{} ", if git_ch == ' ' { ' ' } else { git_ch });
            let git_style = git_char_style(git_ch).bg(bg);

            Line::from(vec![
                Span::styled(format!("{}{} ", sel_marker, type_icon), Style::default().fg(if is_selected { t.yellow } else { t.dim }).bg(bg)),
                Span::styled(git_str, git_style),
                Span::styled(padded_name, Style::default().fg(name_color).bg(bg)),
                Span::styled(size_str, Style::default().fg(t.muted).bg(bg)),
                Span::styled("  ", Style::default().bg(bg)),
                Span::styled(date_str, Style::default().fg(t.dim).bg(bg)),
            ])
        };

        f.render_widget(Paragraph::new(line), row_area);
    }

    // Empty state
    if count == 0 {
        let msg = if !state.tabs[tab_idx].search_query.is_empty() {
            "No matches"
        } else {
            "Empty directory"
        };
        let center_y = area.y + area.height / 2;
        let msg_area = Rect { y: center_y, height: 1, ..area };
        let para = Paragraph::new(msg)
            .style(Style::default().fg(t.dim).bg(t.bg))
            .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(para, msg_area);
    }
}
