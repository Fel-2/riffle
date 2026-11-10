//! Image decoding and metadata shared by the preview and info panels.
//!
//! The `image` crate covers every format riffle shows except JPEG XL, which is
//! decoded with `jxl-oxide` and handed back as an [`image::DynamicImage`] so the
//! rest of the UI stays format-agnostic. The decoder is optional at build time
//! (`--no-default-features`): it is by far the largest dependency and only pays
//! off if you actually have JPEG XL files.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use image::{DynamicImage, Limits};

#[cfg(feature = "jxl")]
mod jxl;

/// Largest file riffle will look at when previewing an image.
pub const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
/// Largest edge (in pixels) riffle is willing to decode.
const MAX_EDGE: u32 = 8000;
/// Cap on a single decoded buffer, applied by the `image` crate.
const MAX_ALLOC: u64 = 128 * 1024 * 1024;

/// Pixel dimensions of an image, read from its header without decoding pixels.
pub fn dimensions(path: &Path) -> Option<(u32, u32)> {
    #[cfg(feature = "jxl")]
    if let Some(dims) = jxl::dimensions(path) {
        return Some(dims);
    }
    // Guess from content rather than the extension, like `decode` does, so
    // misnamed files still report their real size.
    let reader = image::ImageReader::new(BufReader::new(File::open(path).ok()?))
        .with_guessed_format()
        .ok()?;
    reader.into_dimensions().ok()
}

/// Fully decodes an image, honouring the size limits above.
pub fn decode(path: &Path) -> Option<DynamicImage> {
    if std::fs::metadata(path).ok()?.len() > MAX_FILE_BYTES {
        return None;
    }
    #[cfg(feature = "jxl")]
    if let Some(image) = jxl::decode(path) {
        return Some(image);
    }
    // A `.jxl` name on some other format still gets a chance here.
    let mut reader = image::ImageReader::new(BufReader::new(File::open(path).ok()?))
        .with_guessed_format()
        .ok()?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_EDGE);
    limits.max_image_height = Some(MAX_EDGE);
    limits.max_alloc = Some(MAX_ALLOC);
    reader.limits(limits);
    reader.decode().ok()
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
    fn rejects_other_formats() {
        let t = tempfile::tempdir().unwrap();
        let path = write_file(t.path(), "image.png", b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR");
        assert_eq!(dimensions(&path), None);
        assert!(decode(&path).is_none());
    }

    #[test]
    fn reads_dimensions_of_misnamed_image() {
        let t = tempfile::tempdir().unwrap();
        let path = t.path().join("picture.jxl");
        let png = image::RgbImage::new(3, 2);
        png.save_with_format(&path, image::ImageFormat::Png)
            .unwrap();
        assert_eq!(dimensions(&path), Some((3, 2)));
        assert_eq!(
            decode(&path).map(|image| image.to_rgb8().dimensions()),
            Some((3, 2))
        );
    }
}
