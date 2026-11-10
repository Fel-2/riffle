//! JPEG XL decoding, built on the pure-Rust `jxl-oxide` decoder.
//!
//! Everything here returns `None` for files that are not JPEG XL, so callers can
//! simply try this first and fall back to the `image` crate.

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use image::{DynamicImage, GrayAlphaImage, GrayImage, RgbImage, RgbaImage};
use jxl_oxide::{AllocTracker, EnumColourEncoding, JxlImage, PixelFormat, RenderingIntent};

use super::{MAX_EDGE, MAX_FILE_BYTES};

/// `jxl-oxide` keeps multiple full-size `f32` planes alive while rendering, so
/// its budget is wider than a single decoded frame: a 25 MP image (the largest
/// the preview worker accepts) peaks somewhere around 800 MB.
const MAX_ALLOC: usize = 1024 * 1024 * 1024;

/// Magic bytes of a bare JPEG XL codestream.
const CODESTREAM_SIGNATURE: [u8; 2] = [0xff, 0x0a];
/// Magic bytes of a JPEG XL container (the signature box).
const CONTAINER_SIGNATURE: [u8; 12] = [
    0x00, 0x00, 0x00, 0x0c, b'J', b'X', b'L', b' ', 0x0d, 0x0a, 0x87, 0x0a,
];

/// Dimensions of a JPEG XL file, read from the header without decoding pixels.
pub(super) fn dimensions(path: &Path) -> Option<(u32, u32)> {
    // Reaching the first frame's TOC means buffering most of the codestream, so
    // stay away from files that are too big to preview at all.
    if !is_jxl(path) || std::fs::metadata(path).ok()?.len() > MAX_FILE_BYTES {
        return None;
    }
    let file = File::open(path).ok()?;
    let image = JxlImage::builder().read(BufReader::new(file)).ok()?;
    Some((image.width(), image.height()))
}

/// Decodes a JPEG XL file into memory.
pub(super) fn decode(path: &Path) -> Option<DynamicImage> {
    if !is_jxl(path) {
        return None;
    }
    let file = File::open(path).ok()?;
    let mut image = JxlImage::builder()
        .alloc_tracker(AllocTracker::with_limit(MAX_ALLOC))
        .read(BufReader::new(file))
        .ok()?;
    if image.width() > MAX_EDGE || image.height() > MAX_EDGE {
        return None;
    }
    // JPEG XL commonly stores linear or wide-gamut data; without asking for
    // sRGB the preview would come out washed out or oversaturated.
    image.request_color_encoding(EnumColourEncoding::srgb(RenderingIntent::Perceptual));
    let render = image.render_frame(0).ok()?;
    let mut stream = render.stream();
    let width = stream.width();
    let height = stream.height();
    let channels = stream.channels() as usize;
    if width == 0 || height == 0 || channels == 0 {
        return None;
    }
    let mut samples = vec![0u8; width as usize * height as usize * channels];
    if stream.write_to_buffer(&mut samples) != samples.len() {
        return None;
    }
    Some(match image.pixel_format() {
        PixelFormat::Rgba => DynamicImage::ImageRgba8(RgbaImage::from_raw(width, height, samples)?),
        PixelFormat::Rgb => DynamicImage::ImageRgb8(RgbImage::from_raw(width, height, samples)?),
        PixelFormat::Gray => DynamicImage::ImageLuma8(GrayImage::from_raw(width, height, samples)?),
        PixelFormat::Graya => {
            DynamicImage::ImageLumaA8(GrayAlphaImage::from_raw(width, height, samples)?)
        }
        // CMYK input is converted to sRGB above; anything else is unexpected.
        _ => return None,
    })
}

/// Whether a file is JPEG XL, judged by content first so files with a wrong
/// extension still preview correctly.
fn is_jxl(path: &Path) -> bool {
    if has_jxl_signature(path) {
        return true;
    }
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("jxl"))
}

fn has_jxl_signature(path: &Path) -> bool {
    let mut head = [0u8; 12];
    let read = File::open(path).and_then(|mut file| file.read_exact(&mut head));
    read.is_ok() && (head.starts_with(&CODESTREAM_SIGNATURE) || head == CONTAINER_SIGNATURE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_file(dir: &Path, name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let path = dir.join(name);
        let mut file = File::create(&path).unwrap();
        file.write_all(bytes).unwrap();
        path
    }

    #[test]
    fn detects_container_signature() {
        let t = tempfile::tempdir().unwrap();
        let path = write_file(t.path(), "image.bin", &CONTAINER_SIGNATURE);
        assert!(is_jxl(&path));
    }

    #[test]
    fn detects_bare_codestream() {
        let t = tempfile::tempdir().unwrap();
        let mut bytes = CODESTREAM_SIGNATURE.to_vec();
        bytes.extend_from_slice(&[0u8; 32]);
        let path = write_file(t.path(), "image.bin", &bytes);
        assert!(is_jxl(&path));
    }

    #[test]
    fn falls_back_to_extension_for_unknown_content() {
        let t = tempfile::tempdir().unwrap();
        let path = write_file(t.path(), "image.JXL", b"not really a jpeg xl file");
        assert!(is_jxl(&path));
    }

    #[test]
    fn ignores_other_formats() {
        let t = tempfile::tempdir().unwrap();
        let path = write_file(t.path(), "image.png", b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR");
        assert!(!is_jxl(&path));
        assert_eq!(dimensions(&path), None);
        assert!(decode(&path).is_none());
    }
}
