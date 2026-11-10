use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use unicode_width::UnicodeWidthChar;

use crate::state::{App, ImagePayload, PreviewContent, RenderMode};
use crate::syntax::{Token, TokenKind};
use crate::theme::Theme;
use crate::utils;

use super::class_color;
use super::image;

pub fn render(f: &mut Frame, area: Rect, app: &mut App, theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    f.render_widget(
        Paragraph::new("").style(Style::default().bg(theme.bg)),
        area,
    );

    let header_height = if area.height >= 2 { 1 } else { 0 };
    let content = Rect {
        y: area.y + header_height,
        height: area.height.saturating_sub(header_height),
        ..area
    };
    app.layout.preview_content = Some(content);

    if header_height == 1 {
        render_header(f, area, app, theme);
    }

    let line_count = app.preview.content.line_count();
    let height = content.height as usize;
    let max_scroll = line_count.saturating_sub(height);
    if app.preview_scroll > max_scroll {
        app.preview_scroll = max_scroll;
    }
    let scroll = app.preview_scroll;

    match &app.preview.content {
        PreviewContent::Empty => {
            centered(
                f,
                content,
                theme,
                &[
                    ("No selection", theme.muted, Modifier::BOLD),
                    ("", theme.dim, Modifier::empty()),
                    ("j/k to browse", theme.dim, Modifier::empty()),
                ],
            );
        }
        PreviewContent::Loading => {
            centered(
                f,
                content,
                theme,
                &[("Loading…", theme.dim, Modifier::empty())],
            );
        }
        PreviewContent::Text { tokens, .. } => {
            render_text(f, content, tokens, scroll, app.preview_hscroll, theme);
        }
        PreviewContent::Binary { hex, .. } => {
            render_binary(f, content, hex, scroll, theme);
        }
        PreviewContent::Directory { count, entries } => {
            render_directory(f, content, *count, entries, scroll, theme);
        }
        PreviewContent::Image { dims, size } => {
            let dims = *dims;
            let size = *size;
            render_image(f, content, app, &dims, size, theme);
        }
        PreviewContent::Unsupported { reason, size } => {
            render_unsupported(f, content, reason, *size, theme);
        }
    }
}

fn render_header(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let name = app
        .preview
        .path
        .as_ref()
        .and_then(|path| path.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Preview".into());
    let meta = match &app.preview.content {
        PreviewContent::Text {
            line_count,
            truncated,
            lang,
            ..
        } => {
            let lang = format!("{lang:?}").to_lowercase();
            if *truncated {
                format!("{line_count} lines · {lang} · truncated")
            } else {
                format!("{line_count} lines · {lang}")
            }
        }
        PreviewContent::Image { dims, size } => match dims {
            Some((w, h)) => format!("{w} × {h} · {}", utils::format_size(*size)),
            None => utils::format_size(*size),
        },
        PreviewContent::Directory { count, .. } => utils::format_count(*count, "item"),
        PreviewContent::Binary { size, .. } | PreviewContent::Unsupported { size, .. } => {
            utils::format_size(*size)
        }
        PreviewContent::Empty | PreviewContent::Loading => String::new(),
    };
    let focused = app.focus == crate::state::Focus::Preview;
    let title = utils::truncate(&format!("▸ {name}"), area.width as usize);
    let title_width = utils::display_width(&title) as u16;
    let mut spans = vec![Span::styled(
        title,
        Style::default()
            .fg(if focused { theme.accent } else { theme.bright })
            .bg(theme.panel)
            .add_modifier(Modifier::BOLD),
    )];
    if !meta.is_empty() {
        let meta = utils::truncate(&meta, area.width.saturating_sub(title_width + 1) as usize);
        let meta_width = utils::display_width(&meta);
        spans.push(Span::styled(
            " ".repeat(area.width.saturating_sub(title_width + meta_width as u16) as usize),
            Style::default().bg(theme.panel),
        ));
        spans.push(Span::styled(
            meta,
            Style::default().fg(theme.dim).bg(theme.panel),
        ));
    }
    f.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(theme.panel)),
        Rect { height: 1, ..area },
    );
}

fn render_text(
    f: &mut Frame,
    area: Rect,
    tokens: &[Vec<Token>],
    scroll: usize,
    hscroll: usize,
    theme: &Theme,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let line_numbers = tokens.len().to_string().len().max(3);
    let text_width = area.width.saturating_sub(line_numbers as u16 + 2) as usize;
    let mut lines: Vec<Line> = Vec::new();
    for (index, line_tokens) in tokens
        .iter()
        .enumerate()
        .skip(scroll)
        .take(area.height as usize)
    {
        let mut spans = vec![Span::styled(
            utils::pad_start(&format!("{}", index + 1), line_numbers),
            Style::default().fg(theme.dim).bg(theme.bg),
        )];
        spans.push(Span::styled(" ", Style::default().bg(theme.bg)));
        spans.extend(token_spans(line_tokens, hscroll, text_width, theme));
        lines.push(Line::from(spans));
    }
    f.render_widget(
        Paragraph::new(lines).style(Style::default().bg(theme.bg)),
        area,
    );
    scrollbar(f, area, tokens.len(), area.height as usize, scroll, theme);
}

fn token_spans(
    tokens: &[Token],
    hscroll: usize,
    max_width: usize,
    theme: &Theme,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut skip = hscroll;
    let mut width = 0usize;
    let mut clipped = false;
    for token in tokens {
        let mut text = String::new();
        for ch in token.text.chars() {
            if skip > 0 {
                skip -= 1;
                continue;
            }
            let char_width = UnicodeWidthChar::width(ch).unwrap_or(0);
            if width + char_width > max_width {
                clipped = true;
                break;
            }
            text.push(ch);
            width += char_width;
        }
        if !text.is_empty() {
            spans.push(Span::styled(text, token_style(token.kind, theme)));
        }
        if clipped {
            break;
        }
    }
    if clipped && max_width > 0 {
        spans.push(Span::styled("…", Style::default().fg(theme.dim)));
    }
    spans
}

fn token_style(kind: TokenKind, theme: &Theme) -> Style {
    match kind {
        TokenKind::Plain => Style::default().fg(theme.fg),
        TokenKind::Comment => Style::default()
            .fg(theme.dim)
            .add_modifier(Modifier::ITALIC),
        TokenKind::String => Style::default().fg(theme.green),
        TokenKind::Number => Style::default().fg(theme.orange),
        TokenKind::Keyword => Style::default().fg(theme.purple),
        TokenKind::Type => Style::default().fg(theme.cyan),
        TokenKind::Function => Style::default().fg(theme.accent),
        TokenKind::Constant => Style::default().fg(theme.yellow),
        TokenKind::Key => Style::default().fg(theme.accent),
        TokenKind::Operator => Style::default().fg(theme.muted),
        TokenKind::Heading => Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
        TokenKind::Link => Style::default()
            .fg(theme.cyan)
            .add_modifier(Modifier::UNDERLINED),
        TokenKind::Code => Style::default().fg(theme.green),
        TokenKind::Attribute => Style::default().fg(theme.yellow),
    }
}

fn render_binary(f: &mut Frame, area: Rect, hex: &[String], scroll: usize, theme: &Theme) {
    let mut lines = Vec::new();
    for line in hex.iter().skip(scroll).take(area.height as usize) {
        let clipped = utils::truncate(line, area.width as usize);
        let split = clipped.len().min(10);
        let (offset, rest) = clipped.split_at(split);
        lines.push(Line::from(vec![
            Span::styled(offset.to_string(), Style::default().fg(theme.dim)),
            Span::styled(rest.to_string(), Style::default().fg(theme.muted)),
        ]));
    }
    f.render_widget(
        Paragraph::new(lines).style(Style::default().bg(theme.bg)),
        area,
    );
    scrollbar(f, area, hex.len(), area.height as usize, scroll, theme);
}

fn render_directory(
    f: &mut Frame,
    area: Rect,
    count: usize,
    entries: &[crate::state::PreviewDirEntry],
    scroll: usize,
    theme: &Theme,
) {
    let mut lines = Vec::new();
    for entry in entries.iter().skip(scroll).take(area.height as usize) {
        let color = class_color(entry.class, entry.is_dir, false, false, theme);
        let name = utils::truncate(
            &format!("{}{}", entry.name, if entry.is_dir { "/" } else { "" }),
            area.width.saturating_sub(3) as usize,
        );
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {} ", entry.icon),
                Style::default().fg(color).bg(theme.bg),
            ),
            Span::styled(name, Style::default().fg(color).bg(theme.bg)),
        ]));
    }
    if entries.len() < count && (lines.len() as u16) < area.height {
        lines.push(Line::from(Span::styled(
            format!("  +{} more", count - entries.len()),
            Style::default().fg(theme.dim).bg(theme.bg),
        )));
    }
    f.render_widget(
        Paragraph::new(lines).style(Style::default().bg(theme.bg)),
        area,
    );
    scrollbar(f, area, entries.len(), area.height as usize, scroll, theme);
}

fn render_image(
    f: &mut Frame,
    area: Rect,
    app: &mut App,
    dims: &Option<(u32, u32)>,
    size: u64,
    theme: &Theme,
) {
    let Some(path) = app.preview.path.clone() else {
        return;
    };
    let mode = image::resolved(app);
    if mode == RenderMode::None {
        image_placeholder(f, area, dims, size, theme);
        return;
    }
    image::ensure(app, &path, area, theme, mode);
    if app.image.ready.is_none() {
        if let Some(error) = &app.image.error {
            centered(
                f,
                area,
                theme,
                &[
                    ("󰋩", theme.purple, Modifier::empty()),
                    ("", theme.dim, Modifier::empty()),
                    (error, theme.muted, Modifier::BOLD),
                ],
            );
            return;
        }
    }
    let ready = app
        .image
        .ready
        .as_ref()
        .filter(|msg| msg.generation == app.image.generation && msg.key.path == path);
    match ready.map(|msg| &msg.payload) {
        Some(ImagePayload::Blocks { lines }) => {
            f.render_widget(
                Paragraph::new(lines.clone()).style(Style::default().bg(theme.bg)),
                area,
            );
        }
        Some(ImagePayload::Kitty { .. }) | Some(ImagePayload::Chafa { .. }) => {
            blank(f, area);
        }
        Some(ImagePayload::Failed { reason }) => {
            centered(
                f,
                area,
                theme,
                &[
                    ("󰋩", theme.purple, Modifier::empty()),
                    ("", theme.dim, Modifier::empty()),
                    (reason, theme.muted, Modifier::BOLD),
                    ("", theme.dim, Modifier::empty()),
                    ("image preview skipped", theme.dim, Modifier::empty()),
                ],
            );
        }
        None => {
            let dims_text = dims.map(|(w, h)| format!("{w} × {h}")).unwrap_or_default();
            centered(
                f,
                area,
                theme,
                &[
                    ("󰋩", theme.purple, Modifier::empty()),
                    ("", theme.dim, Modifier::empty()),
                    ("loading image…", theme.muted, Modifier::empty()),
                    (dims_text.as_str(), theme.dim, Modifier::empty()),
                ],
            );
        }
    }
}

fn image_placeholder(
    f: &mut Frame,
    area: Rect,
    dims: &Option<(u32, u32)>,
    size: u64,
    theme: &Theme,
) {
    let dims = dims
        .map(|(w, h)| format!("{w} × {h}"))
        .unwrap_or_else(|| "unknown size".into());
    centered(
        f,
        area,
        theme,
        &[
            ("󰋩", theme.purple, Modifier::empty()),
            ("", theme.dim, Modifier::empty()),
            (&dims, theme.muted, Modifier::empty()),
            (&utils::format_size(size), theme.dim, Modifier::empty()),
            ("", theme.dim, Modifier::empty()),
            ("image preview disabled", theme.dim, Modifier::empty()),
        ],
    );
}

fn render_unsupported(f: &mut Frame, area: Rect, reason: &str, size: u64, theme: &Theme) {
    let hint = if reason.contains("archive") {
        "press E to extract"
    } else {
        "press ⏎ to open"
    };
    centered(
        f,
        area,
        theme,
        &[
            ("󰈔", theme.muted, Modifier::empty()),
            ("", theme.dim, Modifier::empty()),
            (reason, theme.fg, Modifier::BOLD),
            (&utils::format_size(size), theme.dim, Modifier::empty()),
            ("", theme.dim, Modifier::empty()),
            (hint, theme.dim, Modifier::empty()),
        ],
    );
}

fn blank(f: &mut Frame, area: Rect) {
    let line = " ".repeat(area.width as usize);
    let lines: Vec<Line> = (0..area.height)
        .map(|_| {
            Line::from(Span::styled(
                line.clone(),
                Style::default().bg(Color::Reset),
            ))
        })
        .collect();
    f.render_widget(
        Paragraph::new(lines).style(Style::default().bg(Color::Reset)),
        area,
    );
}

fn centered(f: &mut Frame, area: Rect, theme: &Theme, lines: &[(&str, Color, Modifier)]) {
    let height = lines.len() as u16;
    let y = area.y + area.height.saturating_sub(height) / 2;
    let rendered: Vec<Line> = lines
        .iter()
        .map(|(text, color, modifier)| {
            Line::from(Span::styled(
                text.to_string(),
                Style::default()
                    .fg(*color)
                    .bg(theme.bg)
                    .add_modifier(*modifier),
            ))
        })
        .collect();
    f.render_widget(
        Paragraph::new(rendered)
            .style(Style::default().bg(theme.bg))
            .alignment(Alignment::Center),
        Rect {
            y,
            height: height.min(area.height),
            ..area
        },
    );
}

fn scrollbar(
    f: &mut Frame,
    area: Rect,
    total: usize,
    visible: usize,
    offset: usize,
    theme: &Theme,
) {
    if total <= visible || visible == 0 || area.width == 0 {
        return;
    }
    let thumb = (visible * visible / total).max(1).min(visible);
    let max_offset = total - visible;
    let position = offset * (visible - thumb) / max_offset.max(1);
    let lines: Vec<Line> = (0..visible)
        .map(|index| {
            let in_thumb = index >= position && index < position + thumb;
            Line::from(Span::styled(
                if in_thumb { "█" } else { "│" },
                Style::default()
                    .fg(if in_thumb { theme.accent } else { theme.border })
                    .bg(theme.bg),
            ))
        })
        .collect();
    f.render_widget(
        Paragraph::new(lines).style(Style::default().bg(theme.bg)),
        Rect {
            x: area.x + area.width - 1,
            width: 1,
            ..area
        },
    );
}
