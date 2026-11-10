use std::io::Write;
use std::path::Path;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::OnceLock;

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use crate::config::ImageMode;
use crate::state::{
    App, ImageKey, ImageMsg, ImagePayload, ImageRequest, PreviewContent, RenderMode,
};
use crate::theme::Theme;
use crate::utils;

const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_PIXELS: u64 = 25_000_000;
const MAX_DECODE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_KITTY_EDGE: u32 = 1920;

pub fn resolved(app: &App) -> RenderMode {
    match app.config.ui.image_mode {
        ImageMode::Kitty => {
            if is_kitty() {
                RenderMode::Kitty
            } else {
                RenderMode::None
            }
        }
        ImageMode::Chafa => {
            if chafa_available() {
                RenderMode::Chafa
            } else {
                RenderMode::None
            }
        }
        ImageMode::Blocks => RenderMode::Blocks,
        ImageMode::None => RenderMode::None,
        ImageMode::Auto => {
            if is_kitty() {
                RenderMode::Kitty
            } else {
                RenderMode::Blocks
            }
        }
    }
}

pub fn is_kitty() -> bool {
    static KITTY: OnceLock<bool> = OnceLock::new();
    *KITTY.get_or_init(|| {
        if std::env::var_os("KITTY_WINDOW_ID").is_some() || std::env::var_os("KITTY_PID").is_some()
        {
            return true;
        }
        if let Ok(program) = std::env::var("TERM_PROGRAM") {
            let program = program.to_ascii_lowercase();
            if program.contains("kitty")
                || program.contains("ghostty")
                || program.contains("wezterm")
            {
                return true;
            }
        }
        std::env::var("TERM")
            .map(|term| term.contains("kitty"))
            .unwrap_or(false)
    })
}

pub fn chafa_available() -> bool {
    static CHAFA: OnceLock<bool> = OnceLock::new();
    *CHAFA.get_or_init(|| {
        std::process::Command::new("chafa")
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    })
}

pub fn ensure(app: &mut App, path: &Path, area: Rect, theme: &Theme, mode: RenderMode) {
    if area.width == 0 || area.height == 0 || mode == RenderMode::None {
        return;
    }
    let key = ImageKey {
        path: path.to_path_buf(),
        cols: area.width,
        rows: area.height,
        mode,
        bg: theme.bg,
        bg_rgb: theme.bg_rgb(),
    };
    if app.image.key.as_ref() == Some(&key) {
        return;
    }
    app.image.generation += 1;
    app.image.key = Some(key.clone());
    app.image.ready = None;
    app.image.pending = true;
    app.image.error = None;
    if app
        .image_tx
        .send(ImageRequest {
            generation: app.image.generation,
            key,
        })
        .is_err()
    {
        app.image.pending = false;
        app.image.error = Some("image worker stopped".into());
    }
}

pub fn spawn_worker(requests: Receiver<ImageRequest>, results: Sender<ImageMsg>) {
    std::thread::spawn(move || {
        while let Ok(mut request) = requests.recv() {
            while let Ok(next) = requests.try_recv() {
                request = next;
            }
            let (dims, payload) = build(&request.key);
            if results
                .send(ImageMsg {
                    generation: request.generation,
                    key: request.key,
                    dims,
                    payload,
                })
                .is_err()
            {
                break;
            }
        }
    });
}

fn build(key: &ImageKey) -> (Option<(u32, u32)>, ImagePayload) {
    let Some(size) = std::fs::metadata(&key.path).ok().map(|meta| meta.len()) else {
        return (
            None,
            ImagePayload::Failed {
                reason: "cannot read image".into(),
            },
        );
    };
    if size > MAX_FILE_BYTES {
        return (
            None,
            ImagePayload::Failed {
                reason: "image is too large to preview".into(),
            },
        );
    }
    let dims = image::image_dimensions(&key.path).ok();
    if let Some((width, height)) = dims {
        if u64::from(width) * u64::from(height) > MAX_PIXELS {
            return (
                Some((width, height)),
                ImagePayload::Failed {
                    reason: "image is too large to preview".into(),
                },
            );
        }
    }
    let payload = match key.mode {
        RenderMode::Blocks => build_blocks(key),
        RenderMode::Kitty => build_kitty(key),
        RenderMode::Chafa => match build_chafa(key) {
            ImagePayload::Failed { .. } => build_blocks(key),
            payload => payload,
        },
        RenderMode::None => ImagePayload::Failed {
            reason: "preview disabled".into(),
        },
    };
    (dims, payload)
}

fn decode_limited(path: &Path) -> Option<image::DynamicImage> {
    let mut reader = image::ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8000);
    limits.max_image_height = Some(8000);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    reader.limits(limits);
    reader.decode().ok()
}

fn build_blocks(key: &ImageKey) -> ImagePayload {
    let Some(image) = decode_limited(&key.path) else {
        return ImagePayload::Failed {
            reason: "could not decode image".into(),
        };
    };
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    if width == 0 || height == 0 {
        return ImagePayload::Failed {
            reason: "empty image".into(),
        };
    }
    let max_cols = key.cols as f64;
    let max_rows = key.rows as f64;
    let scale = (max_cols / width as f64).min(max_rows / (height as f64 / 2.0));
    let target_cols = ((width as f64 * scale).round() as u32).clamp(1, key.cols as u32);
    let target_rows = ((height as f64 * scale / 2.0).round() as u32).clamp(1, key.rows as u32);
    let resized = image::imageops::resize(
        &rgba,
        target_cols,
        target_rows * 2,
        image::imageops::FilterType::Triangle,
    );
    let (bg_r, bg_g, bg_b) = key.bg_rgb;
    let left_pad = ((key.cols as u32).saturating_sub(target_cols) / 2) as usize;
    let top_pad = ((key.rows as u32).saturating_sub(target_rows) / 2) as usize;
    let blank = Line::from(Span::styled(
        " ".repeat(key.cols as usize),
        Style::default().bg(key.bg),
    ));
    let mut lines = Vec::new();
    for _ in 0..top_pad {
        lines.push(blank.clone());
    }
    for row in 0..target_rows {
        let mut spans = Vec::new();
        if left_pad > 0 {
            spans.push(Span::styled(
                " ".repeat(left_pad),
                Style::default().bg(key.bg),
            ));
        }
        let mut run = String::new();
        let mut run_fg = (0u8, 0u8, 0u8);
        let mut run_bg = (0u8, 0u8, 0u8);
        for col in 0..target_cols {
            let top = resized.get_pixel(col, row * 2);
            let bottom = resized.get_pixel(col, row * 2 + 1);
            let top = blend(top.0, bg_r, bg_g, bg_b);
            let bottom = blend(bottom.0, bg_r, bg_g, bg_b);
            if run.is_empty() {
                run_fg = top;
                run_bg = bottom;
                run.push('▀');
            } else if run_fg == top && run_bg == bottom {
                run.push('▀');
            } else {
                spans.push(Span::styled(
                    std::mem::take(&mut run),
                    Style::default()
                        .fg(Color::Rgb(run_fg.0, run_fg.1, run_fg.2))
                        .bg(Color::Rgb(run_bg.0, run_bg.1, run_bg.2)),
                ));
                run_fg = top;
                run_bg = bottom;
                run.push('▀');
            }
        }
        if !run.is_empty() {
            spans.push(Span::styled(
                run,
                Style::default()
                    .fg(Color::Rgb(run_fg.0, run_fg.1, run_fg.2))
                    .bg(Color::Rgb(run_bg.0, run_bg.1, run_bg.2)),
            ));
        }
        let used = left_pad + target_cols as usize;
        let right_pad = (key.cols as usize).saturating_sub(used);
        if right_pad > 0 {
            spans.push(Span::styled(
                " ".repeat(right_pad),
                Style::default().bg(key.bg),
            ));
        }
        lines.push(Line::from(spans));
    }
    while lines.len() < key.rows as usize {
        lines.push(blank.clone());
    }
    ImagePayload::Blocks { lines }
}

fn blend(pixel: [u8; 4], bg_r: u8, bg_g: u8, bg_b: u8) -> (u8, u8, u8) {
    let alpha = pixel[3] as u32;
    if alpha == 255 {
        return (pixel[0], pixel[1], pixel[2]);
    }
    let mix =
        |fg: u8, bg: u8| -> u8 { ((fg as u32 * alpha + bg as u32 * (255 - alpha)) / 255) as u8 };
    (
        mix(pixel[0], bg_r),
        mix(pixel[1], bg_g),
        mix(pixel[2], bg_b),
    )
}

fn build_kitty(key: &ImageKey) -> ImagePayload {
    let Some(image) = decode_limited(&key.path) else {
        return ImagePayload::Failed {
            reason: "could not decode image".into(),
        };
    };
    let (width, height) = (image.width(), image.height());
    let scale = (MAX_KITTY_EDGE as f64 / width.max(1) as f64)
        .min(MAX_KITTY_EDGE as f64 / height.max(1) as f64)
        .min(1.0);
    let resized = if scale < 1.0 {
        let width = (width as f64 * scale).round().max(1.0) as u32;
        let height = (height as f64 * scale).round().max(1.0) as u32;
        image.resize(width, height, image::imageops::FilterType::Triangle)
    } else {
        image
    };
    let mut png = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut png);
    if resized.write_with_encoder(encoder).is_err() {
        return ImagePayload::Failed {
            reason: "could not encode image".into(),
        };
    }
    ImagePayload::Kitty {
        data: utils::base64_encode(&png),
    }
}

fn build_chafa(key: &ImageKey) -> ImagePayload {
    let output = std::process::Command::new("chafa")
        .args([
            "--format",
            "symbols",
            "--colors",
            "256",
            "--animate",
            "off",
            "--size",
            &format!("{}x{}", key.cols, key.rows),
            key.path.to_str().unwrap_or(""),
        ])
        .stdin(std::process::Stdio::null())
        .output();
    match output {
        Ok(output) if output.status.success() && !output.stdout.is_empty() => ImagePayload::Chafa {
            output: String::from_utf8_lossy(&output.stdout).into_owned(),
        },
        _ => ImagePayload::Failed {
            reason: "chafa failed".into(),
        },
    }
}

pub fn draw(app: &mut App) -> std::io::Result<()> {
    let is_image = matches!(&app.preview.content, PreviewContent::Image { .. });
    let Some(area) = app.layout.preview_content else {
        clear_drawn(app);
        return Ok(());
    };
    let Some(path) = app.preview.path.clone() else {
        clear_drawn(app);
        return Ok(());
    };
    if !is_image {
        clear_drawn(app);
        return Ok(());
    }
    let mode = resolved(app);
    if !matches!(mode, RenderMode::Kitty | RenderMode::Chafa) {
        clear_drawn(app);
        return Ok(());
    }
    let Some(msg) = &app.image.ready else {
        clear_drawn(app);
        return Ok(());
    };
    if msg.generation != app.image.generation || msg.key.path != path {
        clear_drawn(app);
        return Ok(());
    }
    match (&msg.payload, mode) {
        (ImagePayload::Kitty { data }, RenderMode::Kitty) => {
            let placement = kitty_placement(msg.dims, area);
            if app.image.drawn.as_ref() == Some(&(msg.generation, path.clone(), placement)) {
                return Ok(());
            }
            clear_kitty();
            send_kitty_data(data, placement)?;
            app.image.drawn = Some((msg.generation, path, placement));
        }
        (ImagePayload::Chafa { output }, RenderMode::Chafa) => {
            if app.image.drawn.is_some() {
                clear_kitty();
                app.image.drawn = None;
            }
            write_chafa(output, area.x, area.y, area.height)?;
        }
        _ => clear_drawn(app),
    }
    Ok(())
}

fn clear_drawn(app: &mut App) {
    if app.image.drawn.is_some() {
        clear_kitty();
        app.image.drawn = None;
    }
}

fn kitty_placement(dims: Option<(u32, u32)>, area: Rect) -> Rect {
    let Some((image_w, image_h)) = dims else {
        return area;
    };
    if image_w == 0 || image_h == 0 {
        return area;
    }
    let (cell_w, cell_h) = crossterm::terminal::window_size()
        .ok()
        .filter(|size| size.columns > 0 && size.rows > 0 && size.width > 0 && size.height > 0)
        .map(|size| {
            (
                size.width as f64 / size.columns as f64,
                size.height as f64 / size.rows as f64,
            )
        })
        .unwrap_or((8.0, 16.0));
    let pane_w = area.width as f64 * cell_w;
    let pane_h = area.height as f64 * cell_h;
    let scale = (pane_w / image_w as f64).min(pane_h / image_h as f64);
    let cols = ((image_w as f64 * scale) / cell_w)
        .round()
        .clamp(1.0, area.width as f64) as u16;
    let rows = ((image_h as f64 * scale) / cell_h)
        .round()
        .clamp(1.0, area.height as f64) as u16;
    Rect {
        x: area.x + area.width.saturating_sub(cols) / 2,
        y: area.y + area.height.saturating_sub(rows) / 2,
        width: cols,
        height: rows,
    }
}

fn send_kitty_data(data: &str, rect: Rect) -> std::io::Result<()> {
    if rect.width == 0 || rect.height == 0 {
        return Ok(());
    }
    let bytes = data.as_bytes();
    let chunks: Vec<&[u8]> = bytes.chunks(4096).collect();
    let last = chunks.len().saturating_sub(1);
    let mut stdout = std::io::stdout().lock();
    write!(stdout, "\x1b[s\x1b[{};{}H", rect.y + 1, rect.x + 1)?;
    for (index, chunk) in chunks.iter().enumerate() {
        let more = if index < last { 1 } else { 0 };
        let text = std::str::from_utf8(chunk).unwrap_or("");
        if index == 0 {
            write!(
                stdout,
                "\x1b_Ga=T,f=100,c={},r={},q=2,C=1,m={};{}\x1b\\",
                rect.width, rect.height, more, text
            )?;
        } else {
            write!(stdout, "\x1b_Gm={};{}\x1b\\", more, text)?;
        }
    }
    write!(stdout, "\x1b[u")?;
    stdout.flush()
}

pub fn clear_kitty() {
    let mut stdout = std::io::stdout().lock();
    let _ = write!(stdout, "\x1b_Ga=d,d=A\x1b\\");
    let _ = stdout.flush();
}

fn write_chafa(output: &str, x: u16, y: u16, height: u16) -> std::io::Result<()> {
    let mut stdout = std::io::stdout().lock();
    write!(stdout, "\x1b[s")?;
    for (row, line) in output.lines().enumerate() {
        if row as u16 >= height {
            break;
        }
        write!(
            stdout,
            "\x1b[{};{}H{}\x1b[0m",
            y + 1 + row as u16,
            x + 1,
            line
        )?;
    }
    write!(stdout, "\x1b[u")?;
    stdout.flush()
}
