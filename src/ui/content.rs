use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use crate::filesystem::FileType;
use crate::state::{AppState, InputMode};
use crate::utils;

// Tokyo Night colors
const C_DIR:    Color = Color::Rgb(125, 207, 255);
const C_IMAGE:  Color = Color::Rgb(187, 154, 247);
const C_EXEC:   Color = Color::Rgb(158, 206, 106);
const C_ARCH:   Color = Color::Rgb(255, 158, 100);
const C_AUDIO:  Color = Color::Rgb(224, 175, 104);
const C_CODE:   Color = Color::Rgb(122, 162, 247);
const C_HIDDEN: Color = Color::Rgb(86, 95, 137);
const C_FG:     Color = Color::Rgb(169, 177, 214);
const C_BG:     Color = Color::Rgb(26, 27, 38);
const C_SEL_BG: Color = Color::Rgb(54, 74, 130);
const C_CUR_BG: Color = Color::Rgb(41, 46, 66);
const C_ACCENT: Color = Color::Rgb(122, 162, 247);

fn entry_color(is_dir: bool, is_hidden: bool, ft: &FileType) -> Color {
    if is_dir    { return C_DIR; }
    if is_hidden { return C_HIDDEN; }
    match ft {
        FileType::Image      => C_IMAGE,
        FileType::Executable => C_EXEC,
        FileType::Archive    => C_ARCH,
        FileType::Audio      => C_AUDIO,
        FileType::Code       => C_CODE,
        _                    => C_FG,
    }
}

fn git_char_style(ch: char) -> Style {
    match ch {
        'M' => Style::default().fg(Color::Yellow),
        'A' => Style::default().fg(Color::Rgb(158, 206, 106)),
        'D' => Style::default().fg(Color::Rgb(247, 118, 142)),
        'R' => Style::default().fg(Color::Rgb(122, 162, 247)),
        'U' => Style::default().fg(Color::Rgb(255, 158, 100)),
        '?' => Style::default().fg(Color::Rgb(86, 95, 137)),
        _   => Style::default().fg(Color::Rgb(86, 95, 137)),
    }
}

pub fn render_pane(f: &mut Frame, area: Rect, state: &mut AppState, tab_idx: usize, focused: bool) {
    let border_style = if focused {
        Style::default().fg(C_ACCENT)
    } else {
        Style::default().fg(Color::Rgb(41, 46, 66))
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .style(Style::default().bg(C_BG));

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
    let left = if header.len() > left_width as usize {
        header[..left_width as usize].to_string()
    } else {
        format!("{:<width$}", header, width = left_width as usize)
    };

    let line = Line::from(vec![
        Span::styled(left, Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD)),
        Span::styled(right, Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD)),
    ]);
    f.render_widget(Paragraph::new(line).style(Style::default().bg(Color::Rgb(31, 35, 53))), area);
}

fn render_list(f: &mut Frame, area: Rect, state: &mut AppState, tab_idx: usize) {
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

        let bg = if is_cursor && is_selected { C_SEL_BG }
            else if is_cursor   { C_CUR_BG }
            else if is_selected { C_SEL_BG }
            else                { C_BG };

        let name_color = entry_color(is_dir, is_hidden, &file_type);

        // Check if this row is being renamed
        let is_renaming = is_renaming_mode && is_cursor
            && state.tabs[tab_idx].renaming.as_ref().map(|(i, _)| *i == display_idx).unwrap_or(false);

        let line = if is_renaming {
            let buf = state.tabs[tab_idx].renaming.as_ref().map(|(_, b)| b.as_str()).unwrap_or("").to_string();
            Line::from(vec![
                Span::styled(format!(" {} ", type_icon), Style::default().fg(C_ACCENT).bg(bg)),
                Span::styled(format!("{}_", buf), Style::default().fg(Color::Yellow).bg(bg).add_modifier(Modifier::BOLD)),
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

            let display_name = if max_name > 3 && base_name.len() > max_name {
                format!("{}~", &base_name[..max_name.saturating_sub(1)])
            } else {
                base_name
            };
            let padded_name = format!("{:<width$}", display_name, width = max_name.max(1));

            let git_str = format!("{} ", if git_ch == ' ' { ' ' } else { git_ch });
            let git_style = git_char_style(git_ch).bg(bg);

            Line::from(vec![
                Span::styled(format!("{}{} ", sel_marker, type_icon), Style::default().fg(if is_selected { Color::Yellow } else { C_HIDDEN }).bg(bg)),
                Span::styled(git_str, git_style),
                Span::styled(padded_name, Style::default().fg(name_color).bg(bg)),
                Span::styled(size_str, Style::default().fg(Color::Rgb(150, 150, 180)).bg(bg)),
                Span::styled("  ", Style::default().bg(bg)),
                Span::styled(date_str, Style::default().fg(Color::Rgb(86, 95, 137)).bg(bg)),
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
            .style(Style::default().fg(C_HIDDEN).bg(C_BG))
            .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(para, msg_area);
    }
}
