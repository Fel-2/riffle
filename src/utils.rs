use std::path::{Path, PathBuf};
use std::time::SystemTime;

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub fn display_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

pub fn truncate(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if display_width(s) <= max {
        return s.to_string();
    }
    let mut out = String::new();
    let mut width = 0;
    for ch in s.chars() {
        let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + cw > max.saturating_sub(1) {
            break;
        }
        out.push(ch);
        width += cw;
    }
    out.push('…');
    out
}

pub fn pad_end(s: &str, width: usize) -> String {
    let w = display_width(s);
    if w >= width {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(width - w))
    }
}

pub fn pad_start(s: &str, width: usize) -> String {
    let w = display_width(s);
    if w >= width {
        s.to_string()
    } else {
        format!("{}{}", " ".repeat(width - w), s)
    }
}

pub fn shorten_home(path: &Path) -> String {
    let Some(home) = dirs::home_dir() else {
        return path.to_string_lossy().into_owned();
    };
    match path.strip_prefix(&home) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.to_string_lossy().into_owned(),
    }
}

pub fn absolutize(path: &Path, base: &Path) -> PathBuf {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    };
    let mut out = PathBuf::new();
    for component in joined.components() {
        use std::path::Component::*;
        match component {
            CurDir => {}
            ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

pub fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "K", "M", "G", "T", "P"];
    if bytes < 1024 {
        return format!("{} B", bytes);
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if value < 10.0 {
        format!("{:.1} {}", value, UNITS[unit])
    } else {
        format!("{:.0} {}", value, UNITS[unit])
    }
}

pub fn format_count(n: usize, singular: &str) -> String {
    if n == 1 {
        format!("1 {}", singular)
    } else {
        format!("{} {}s", n, singular)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateStyle {
    Relative,
    Iso,
    Short,
}

pub fn format_time(time: SystemTime, style: DateStyle) -> String {
    let dt: chrono::DateTime<chrono::Local> = time.into();
    match style {
        DateStyle::Iso => dt.format("%Y-%m-%d").to_string(),
        DateStyle::Short => dt.format("%b %d %H:%M").to_string(),
        DateStyle::Relative => {
            let now = SystemTime::now();
            let Ok(age) = now.duration_since(time) else {
                return dt.format("%Y-%m-%d").to_string();
            };
            let secs = age.as_secs();
            if secs < 60 {
                "now".to_string()
            } else if secs < 3600 {
                format!("{}m", secs / 60)
            } else if secs < 86_400 {
                format!("{}h", secs / 3600)
            } else if secs < 86_400 * 7 {
                format!("{}d", secs / 86_400)
            } else if secs < 86_400 * 365 {
                dt.format("%b %d").to_string()
            } else {
                dt.format("%Y-%m-%d").to_string()
            }
        }
    }
}

pub fn format_mode(mode: u32) -> String {
    let mut out = String::with_capacity(9);
    for shift in [6u32, 3, 0] {
        let bits = (mode >> shift) & 0o7;
        out.push(if bits & 0o4 != 0 { 'r' } else { '-' });
        out.push(if bits & 0o2 != 0 { 'w' } else { '-' });
        out.push(if bits & 0o1 != 0 { 'x' } else { '-' });
    }
    out
}

#[cfg(unix)]
pub fn format_type_mode(meta: &std::fs::Metadata) -> String {
    use std::os::unix::fs::{FileTypeExt, MetadataExt};
    let ft = meta.file_type();
    let kind = if ft.is_dir() {
        'd'
    } else if ft.is_symlink() {
        'l'
    } else if ft.is_fifo() {
        'p'
    } else if ft.is_socket() {
        's'
    } else if ft.is_block_device() {
        'b'
    } else if ft.is_char_device() {
        'c'
    } else {
        '-'
    };
    format!("{}{}", kind, format_mode(meta.mode()))
}

#[cfg(not(unix))]
pub fn format_type_mode(meta: &std::fs::Metadata) -> String {
    if meta.is_dir() {
        "d---------".to_string()
    } else {
        "----------".to_string()
    }
}

pub fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let combined = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[((combined >> 18) & 0x3f) as usize] as char);
        out.push(ALPHABET[((combined >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[((combined >> 6) & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[(combined & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

pub fn osc52(text: &str) -> std::io::Result<()> {
    use std::io::Write;
    let encoded = base64_encode(text.as_bytes());
    let mut stdout = std::io::stdout().lock();
    write!(stdout, "\x1b]52;c;{}\x07", encoded)?;
    stdout.flush()
}

pub fn url_encode(path: &str) -> String {
    let mut out = String::new();
    for byte in path.bytes() {
        let keep = byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.' | b'~');
        if keep {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{:02X}", byte));
        }
    }
    out
}

pub fn hex_dump(bytes: &[u8], max_rows: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for (row, chunk) in bytes.chunks(16).take(max_rows).enumerate() {
        let mut hex = String::new();
        let mut ascii = String::new();
        for (i, b) in chunk.iter().enumerate() {
            if i == 8 {
                hex.push(' ');
            }
            hex.push_str(&format!("{:02x} ", b));
            ascii.push(if b.is_ascii_graphic() || *b == b' ' {
                *b as char
            } else {
                '.'
            });
        }
        lines.push(format!("{:08x}  {:<49} {}", row * 16, hex, ascii));
    }
    lines
}

pub fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_respects_display_width() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello", 3), "he…");
        assert_eq!(truncate("hello", 0), "");
        assert_eq!(display_width(&truncate("日本語", 4)), 3);
    }

    #[test]
    fn size_formatting() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(1023), "1023 B");
        assert_eq!(format_size(1024), "1.0 K");
        assert_eq!(format_size(1536), "1.5 K");
        assert_eq!(format_size(10 * 1024 * 1024), "10 M");
    }

    #[test]
    fn mode_strings() {
        assert_eq!(format_mode(0o755), "rwxr-xr-x");
        assert_eq!(format_mode(0o644), "rw-r--r--");
    }

    #[test]
    fn base64_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn url_encoding_escapes_spaces() {
        assert_eq!(url_encode("/a b/c"), "/a%20b/c");
    }

    #[test]
    fn path_normalization() {
        let base = Path::new("/a/b");
        assert_eq!(absolutize(Path::new("../c"), base), PathBuf::from("/a/c"));
        assert_eq!(absolutize(Path::new("/x/./y"), base), PathBuf::from("/x/y"));
    }
}
