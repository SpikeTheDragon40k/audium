//! Cover images: embedded pictures and sidecar image files.

use std::io::Read;
use std::path::Path;

use lofty::config::ParseOptions;
use lofty::file::TaggedFileExt;
use lofty::picture::{MimeType, PictureType};
use lofty::probe::Probe;

use crate::limits;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cover {
    pub data: Vec<u8>,
    /// File extension matching the image format ("jpg", "png", ...).
    pub extension: &'static str,
}

/// The front cover embedded in an audio file (or its first picture).
pub fn read_embedded_cover(path: &Path) -> Option<Cover> {
    let tagged = Probe::open(path)
        .ok()?
        .options(ParseOptions::new().read_cover_art(true))
        .guess_file_type()
        .ok()?
        .read()
        .ok()?;
    let pictures: Vec<_> = tagged.tags().iter().flat_map(|t| t.pictures()).collect();
    let picture = pictures
        .iter()
        .find(|p| p.pic_type() == PictureType::CoverFront)
        .or_else(|| pictures.first())?;
    let data = picture.data();
    if data.is_empty() || data.len() as u64 > limits::COVER_BYTES {
        return None;
    }
    let extension = match picture.mime_type() {
        Some(MimeType::Png) => "png",
        Some(MimeType::Jpeg) => "jpg",
        Some(MimeType::Gif) => "gif",
        Some(MimeType::Bmp) => "bmp",
        _ => sniff_extension(data)?,
    };
    Some(Cover {
        data: data.to_vec(),
        extension,
    })
}

/// Reads a sidecar image file, checking that it really is an image.
pub fn read_cover_file(path: &Path) -> Option<Cover> {
    let file = std::fs::File::open(path).ok()?;
    if file.metadata().ok()?.len() > limits::COVER_BYTES {
        return None;
    }
    let mut data = Vec::new();
    file.take(limits::COVER_BYTES).read_to_end(&mut data).ok()?;
    let extension = sniff_extension(&data)?;
    Some(Cover { data, extension })
}

/// Identifies common image formats by their magic bytes.
pub fn sniff_extension(data: &[u8]) -> Option<&'static str> {
    if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        Some("webp")
    } else if data.starts_with(b"GIF8") {
        Some("gif")
    } else if data.starts_with(b"BM") {
        Some("bmp")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniffs_formats() {
        assert_eq!(sniff_extension(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("jpg"));
        assert_eq!(sniff_extension(b"\x89PNG\r\n\x1a\nrest"), Some("png"));
        assert_eq!(sniff_extension(b"RIFF\0\0\0\0WEBPVP8 "), Some("webp"));
        assert_eq!(sniff_extension(b"not an image"), None);
        assert_eq!(sniff_extension(&[]), None);
    }
}
