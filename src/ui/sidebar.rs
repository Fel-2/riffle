use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use crate::state::AppState;

const C_ACCENT: Color = Color::Rgb(122, 162, 247);
const C_FG:     Color = Color::Rgb(169, 177, 214);
const C_DIM:    Color = Color::Rgb(86, 95, 137);
const C_BG:     Color = Color::Rgb(31, 35, 53);
const C_SEL:    Color = Color::Rgb(54, 74, 130);

pub fn render(f: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(Color::Rgb(41, 46, 66)))
        .style(Style::default().bg(C_BG));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let cur_dir = state.focused_tab().current_dir.clone();
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

    // Bookmarks
    row!(" Bookmarks", Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD));
    for (name, path) in &state.bookmarks {
        if !path.exists() { continue; }
        let selected = *path == cur_dir;
        let style = if selected {
            Style::default().fg(C_ACCENT).bg(C_SEL).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(C_FG).bg(C_BG)
        };
        row!(format!("  {}", name), style);
    }

    if y < max_y {
        let sep = "-".repeat(w);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(sep, Style::default().fg(Color::Rgb(41, 46, 66))))),
            Rect { y, height: 1, ..inner },
        );
        y += 1;
    }

    // Drives
    row!(" Drives", Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD));
    let root = std::path::PathBuf::from("/");
    let sel = root == cur_dir;
    let root_style = if sel { Style::default().fg(C_ACCENT).bg(C_SEL) } else { Style::default().fg(C_FG) };
    row!("  / (root)", root_style);

    for base in ["/media", "/mnt", "/run/media"] {
        let base_path = std::path::PathBuf::from(base);
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
                    let sel = p == cur_dir;
                    let style = if sel { Style::default().fg(C_ACCENT).bg(C_SEL) } else { Style::default().fg(C_FG) };
                    row!(format!("  {}", mn), style);
                }
            }
        }
    }

    if y < max_y {
        let sep = "-".repeat(w);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(sep, Style::default().fg(Color::Rgb(41, 46, 66))))),
            Rect { y, height: 1, ..inner },
        );
        y += 1;
    }

    // Path breadcrumbs
    row!(" Path", Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD));
    let ancestors: Vec<_> = cur_dir.ancestors().map(|p| p.to_path_buf()).collect();
    for ancestor in ancestors.iter().rev().take(8) {
        let name = ancestor.file_name().and_then(|n| n.to_str()).unwrap_or("/");
        let sel = ancestor == &cur_dir;
        let style = if sel {
            Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(C_DIM)
        };
        row!(format!("  {}", name), style);
    }
}
