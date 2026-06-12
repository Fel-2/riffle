use std::path::Path;
use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use crate::state::{AppState, PreviewContent};
use crate::theme::theme;
use crate::utils;

pub fn render(f: &mut Frame, area: Rect, state: &mut AppState) {
    let t = theme();
    let block = Block::default()
        .title(" Preview ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(t.bg_highlight))
        .style(Style::default().bg(t.bg));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Save INNER area (inside border) for kitty image placement
    state.preview_rect = Some((inner.x, inner.y, inner.width, inner.height));

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    match &state.preview_content.clone() {
        PreviewContent::None => {
            let para = Paragraph::new("No preview")
                .style(Style::default().fg(t.dim).bg(t.bg))
                .alignment(Alignment::Center);
            f.render_widget(para, inner);
        }
        PreviewContent::Text(lines) => {
            let display_lines: Vec<Line> = lines.iter()
                .map(|l| Line::from(Span::styled(l.clone(), Style::default().fg(t.fg))))
                .collect();
            let para = Paragraph::new(display_lines)
                .style(Style::default().bg(t.bg));
            f.render_widget(para, inner);
        }
        PreviewContent::Image(_) => {
            // Actual image drawn via kitty protocol after frame; show placeholder
            let para = Paragraph::new("[ Image Preview ]")
                .style(Style::default().fg(t.accent).add_modifier(Modifier::BOLD).bg(t.bg))
                .alignment(Alignment::Center);
            f.render_widget(para, inner);
        }
        PreviewContent::Binary { size } => {
            let text = format!("Binary file  {}", utils::format_size(*size));
            let para = Paragraph::new(text)
                .style(Style::default().fg(t.dim).bg(t.bg))
                .alignment(Alignment::Center);
            f.render_widget(para, inner);
        }
        PreviewContent::Directory { count } => {
            let text = format!("Directory  {} items", count);
            let para = Paragraph::new(text)
                .style(Style::default().fg(t.accent).bg(t.bg))
                .alignment(Alignment::Center);
            f.render_widget(para, inner);
        }
    }
}

pub fn is_kitty() -> bool {
    std::env::var("KITTY_WINDOW_ID").is_ok()
        || std::env::var("TERM").map(|t| t.contains("kitty")).unwrap_or(false)
}

/// Read pixel dimensions from image header without loading the full image.
/// Returns None if format is unrecognised or the header can't be parsed.
fn image_pixel_size(path: &Path) -> Option<(u32, u32)> {
    use std::io::Read;
    let mut buf = [0u8; 32];
    let n = std::fs::File::open(path).ok()?.read(&mut buf).ok()?;
    let buf = &buf[..n];

    // ── PNG ─────────────────────────────────────────────────────────────────
    // Magic (8) + IHDR chunk: len(4) + "IHDR"(4) + width(4) + height(4)
    if buf.len() >= 24 && &buf[0..8] == b"\x89PNG\r\n\x1a\n" {
        let w = u32::from_be_bytes([buf[16], buf[17], buf[18], buf[19]]);
        let h = u32::from_be_bytes([buf[20], buf[21], buf[22], buf[23]]);
        return Some((w, h));
    }

    // ── JPEG ─────────────────────────────────────────────────────────────────
    // Scan markers to find any SOF (Start Of Frame) segment.
    if buf.len() >= 4 && buf[0] == 0xFF && buf[1] == 0xD8 {
        let mut data = Vec::new();
        if std::fs::File::open(path).ok()?.read_to_end(&mut data).is_err() {
            return None;
        }
        let mut i = 2usize;
        while i + 1 < data.len() {
            if data[i] != 0xFF { break; }
            let marker = data[i + 1];

            // Standalone markers — no length field (FF D0-D9, FF 01, FF 00)
            if matches!(marker, 0x00 | 0x01 | 0xD0..=0xD9) {
                i += 2;
                continue;
            }
            // SOS (Start of Scan) — compressed image data follows, stop here
            if marker == 0xDA { break; }

            // All other markers have a 2-byte length field
            if i + 3 >= data.len() { break; }
            let seg_len = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
            if seg_len < 2 { break; } // malformed

            // SOF markers: C0-C3, C5-C7, C9-CB, CD-CF
            // Structure: FF marker(1) len(2) precision(1) height(2) width(2) …
            if matches!(marker,
                0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF)
            {
                if i + 8 < data.len() {
                    let h = u16::from_be_bytes([data[i + 5], data[i + 6]]) as u32;
                    let w = u16::from_be_bytes([data[i + 7], data[i + 8]]) as u32;
                    return Some((w, h));
                }
                break;
            }

            i += 2 + seg_len;
        }
        return None;
    }

    // ── GIF ──────────────────────────────────────────────────────────────────
    // Header: "GIF87a" or "GIF89a" then width(2 LE) + height(2 LE)
    if buf.len() >= 10 && &buf[0..4] == b"GIF8" {
        let w = u16::from_le_bytes([buf[6], buf[7]]) as u32;
        let h = u16::from_le_bytes([buf[8], buf[9]]) as u32;
        return Some((w, h));
    }

    // ── BMP ──────────────────────────────────────────────────────────────────
    // Header: "BM" then file size(4), reserved(4), offset(4), DIB header size(4),
    // width(4 LE signed) + height(4 LE signed) at offset 18
    if buf.len() >= 26 && &buf[0..2] == b"BM" {
        let w = i32::from_le_bytes([buf[18], buf[19], buf[20], buf[21]]).unsigned_abs();
        let h = i32::from_le_bytes([buf[22], buf[23], buf[24], buf[25]]).unsigned_abs();
        return Some((w, h));
    }

    // ── WebP ────────────────────────────────────────────────────────────────
    if buf.len() >= 30 && &buf[0..4] == b"RIFF" && &buf[8..12] == b"WEBP" {
        // Lossy (VP8 ): frame tag(3) + start_code(3) then 14-bit width/height
        if &buf[12..16] == b"VP8 " {
            let w = (u16::from_le_bytes([buf[26], buf[27]]) & 0x3FFF) as u32;
            let h = (u16::from_le_bytes([buf[28], buf[29]]) & 0x3FFF) as u32;
            if w > 0 && h > 0 { return Some((w, h)); }
        }
        // Lossless (VP8L): signature(1) then width-1(14 bits) | height-1(14 bits)
        if &buf[12..16] == b"VP8L" && buf[20] == 0x2F {
            let bits = u32::from_le_bytes([buf[21], buf[22], buf[23], buf[24]]);
            let w = (bits & 0x3FFF) + 1;
            let h = ((bits >> 14) & 0x3FFF) + 1;
            return Some((w, h));
        }
    }

    None
}

/// Compute display size in terminal cells that fits the image inside the pane
/// while preserving aspect ratio. Uses real terminal pixel metrics when available.
fn fit_in_cells(img_w: u32, img_h: u32, max_cols: u16, max_rows: u16) -> (u16, u16) {
    if img_w == 0 || img_h == 0 { return (max_cols, max_rows); }

    // Get real cell pixel dimensions from the terminal
    let (cell_w, cell_h) = crossterm::terminal::window_size()
        .ok()
        .filter(|ws| ws.columns > 0 && ws.rows > 0 && ws.width > 0 && ws.height > 0)
        .map(|ws| (
            ws.width  as f64 / ws.columns as f64,
            ws.height as f64 / ws.rows    as f64,
        ))
        .unwrap_or((8.0, 16.0)); // sane fallback

    let pane_px_w = max_cols as f64 * cell_w;
    let pane_px_h = max_rows as f64 * cell_h;

    let scale = f64::min(pane_px_w / img_w as f64, pane_px_h / img_h as f64);

    let disp_px_w = img_w as f64 * scale;
    let disp_px_h = img_h as f64 * scale;

    let cols = ((disp_px_w / cell_w).round() as u16).max(1).min(max_cols);
    let rows = ((disp_px_h / cell_h).round() as u16).max(1).min(max_rows);

    (cols, rows)
}

/// Load an image file and return PNG-encoded bytes.
/// If the file is already a PNG it is returned as-is; otherwise
/// the `image` crate decodes it and re-encodes to PNG.
fn load_as_png(path: &Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let raw = std::fs::read(path)?;

    // Already PNG — skip re-encoding
    if raw.len() >= 8 && &raw[0..8] == b"\x89PNG\r\n\x1a\n" {
        return Ok(raw);
    }

    // Decode whatever format the `image` crate supports and re-encode as PNG
    let img = image::load_from_memory(&raw)?;
    let mut png_buf: Vec<u8> = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut png_buf);
    img.write_with_encoder(encoder)?;
    Ok(png_buf)
}

/// Send image to the kitty terminal via the graphics protocol.
/// Dimensions are computed to preserve aspect ratio using real cell pixel metrics.
pub fn send_kitty_image(
    path: &Path,
    x: u16,
    y: u16,
    w: u16,
    h: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write as _;
    if !is_kitty() { return Ok(()); }
    if w == 0 || h == 0 { return Ok(()); }

    // Compute display size preserving aspect ratio.
    // If we can't read image dimensions, send only c= (width); kitty will
    // scale height proportionally rather than stretching to fill both axes.
    let (disp_cols, disp_rows) = if let Some((iw, ih)) = image_pixel_size(path) {
        let (c, r) = fit_in_cells(iw, ih, w, h);
        (c, r)
    } else {
        (w, 0u16) // 0 → omit r parameter below
    };

    let data = load_as_png(path)?;
    let encoded = crate::utils::base64_encode(&data);
    let bytes = encoded.as_bytes();

    let mut stdout = std::io::stdout().lock();

    // Save cursor, move to inner top-left of preview pane (ANSI: 1-indexed)
    write!(stdout, "\x1b[s\x1b[{};{}H", y + 1, x + 1)?;

    let chunks: Vec<&[u8]> = bytes.chunks(4096).collect();
    let last = chunks.len().saturating_sub(1);

    for (i, chunk) in chunks.iter().enumerate() {
        let m = if i < last { 1u8 } else { 0u8 };
        let s = std::str::from_utf8(chunk).unwrap_or("");
        if i == 0 {
            // First chunk carries the header; include r= only when we know dims
            if disp_rows > 0 {
                write!(stdout, "\x1b_Ga=T,f=100,c={},r={},q=2,m={};{}\x1b\\",
                       disp_cols, disp_rows, m, s)?;
            } else {
                write!(stdout, "\x1b_Ga=T,f=100,c={},q=2,m={};{}\x1b\\",
                       disp_cols, m, s)?;
            }
        } else {
            write!(stdout, "\x1b_Gm={};{}\x1b\\", m, s)?;
        }
    }

    // Restore cursor so ratatui's next draw is unaffected
    write!(stdout, "\x1b[u")?;
    stdout.flush()?;
    Ok(())
}

pub fn clear_kitty() {
    use std::io::Write as _;
    let mut stdout = std::io::stdout().lock();
    let _ = write!(stdout, "\x1b_Ga=d,d=A\x1b\\");
    let _ = stdout.flush();
}

pub fn is_chafa_available() -> bool {
    static CHAFA: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *CHAFA.get_or_init(|| {
        std::process::Command::new("chafa")
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    })
}

/// Run chafa on `path` and return the ANSI output string, or None on failure.
pub fn render_chafa_image(path: &Path, w: u16, h: u16) -> Option<String> {
    if w == 0 || h == 0 { return None; }
    let out = std::process::Command::new("chafa")
        .args([
            "--size", &format!("{}x{}", w, h),
            "--animate", "off",
            path.to_str()?,
        ])
        .output()
        .ok()?;
    if out.status.success() && !out.stdout.is_empty() {
        Some(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        None
    }
}

/// Clear the preview area by overwriting with spaces + ANSI reset.
/// Needed because chafa ANSI output is not fully cleared by ratatui redraws.
pub fn clear_chafa(x: u16, y: u16, w: u16, h: u16) {
    use std::io::Write as _;
    let mut stdout = std::io::stdout().lock();
    let _ = write!(stdout, "\x1b[s");
    let blank = " ".repeat(w as usize);
    for row in 0..h {
        let _ = write!(stdout, "\x1b[{};{}H\x1b[0m{}", y + 1 + row, x + 1, blank);
    }
    let _ = write!(stdout, "\x1b[u");
    let _ = stdout.flush();
}

/// Write a cached chafa ANSI output string to the terminal at (x, y).
/// Must be called after terminal.draw() since ratatui overwrites it each frame.
pub fn write_chafa_output(output: &str, x: u16, y: u16, h: u16) {
    use std::io::Write as _;
    let mut stdout = std::io::stdout().lock();
    let _ = write!(stdout, "\x1b[s");
    for (row, line) in output.lines().enumerate() {
        if row as u16 >= h { break; }
        let _ = write!(stdout, "\x1b[{};{}H{}", y + 1 + row as u16, x + 1, line);
    }
    let _ = write!(stdout, "\x1b[u");
    let _ = stdout.flush();
}
