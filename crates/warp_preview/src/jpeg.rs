//! JPEG frames.

use std::io::Cursor;
use std::sync::Arc;

use image::codecs::jpeg::JpegEncoder;
use image::{ExtendedColorType, ImageFormat, ImageReader};

use crate::Frame;

/// JPEG quality for frames. Previews are for watching, so this favors size over detail.
pub const QUALITY: u8 = 72;

/// Builds a frame from JPEG bytes, reading the picture's size from its header.
pub fn frame_from_jpeg(jpeg: Vec<u8>) -> Option<Frame> {
    let (width, height) = ImageReader::with_format(Cursor::new(&jpeg), ImageFormat::Jpeg)
        .into_dimensions()
        .ok()?;
    Some(Frame {
        jpeg: Arc::from(jpeg),
        width,
        height,
    })
}

/// Encodes tightly packed RGB pixels as a frame.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn frame_from_rgb(rgb: &[u8], width: u32, height: u32) -> Option<Frame> {
    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, QUALITY)
        .encode(rgb, width, height, ExtendedColorType::Rgb8)
        .ok()?;
    Some(Frame {
        jpeg: Arc::from(jpeg),
        width,
        height,
    })
}

/// Re-encodes `jpeg` so neither side exceeds `max_side`. Pictures already within the limit are
/// returned unchanged.
pub fn shrink(jpeg: &[u8], max_side: u32) -> Option<Vec<u8>> {
    let image = image::load_from_memory_with_format(jpeg, ImageFormat::Jpeg).ok()?;
    let (width, height) = crate::geometry::scale_within((image.width(), image.height()), max_side);
    if (width, height) == (image.width(), image.height()) {
        return Some(jpeg.to_vec());
    }
    let resized = image
        .resize_exact(width, height, image::imageops::FilterType::Triangle)
        .to_rgb8();
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, QUALITY)
        .encode(resized.as_raw(), width, height, ExtendedColorType::Rgb8)
        .ok()?;
    Some(out)
}

#[cfg(test)]
#[path = "jpeg_tests.rs"]
mod tests;
