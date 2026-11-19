use std::path::Path;

use anyhow::{Context, Result};

use crate::widgets::png::PngImage;

const CAPE_WIDTH: u16 = 10;
const CAPE_HEIGHT: u16 = 16;

/// Extracts the front face of a Minecraft cape texture for display.
pub struct CapePreview;

impl CapePreview {
    pub fn from_path(path: &Path) -> Result<PngImage> {
        let bytes = std::fs::read(path)
            .with_context(|| format!("read cape PNG from {}", path.display()))?;
        Self::from_png(&bytes)
    }

    pub fn from_png(bytes: &[u8]) -> Result<PngImage> {
        let texture = PngImage::from_png(bytes).context("decode cape PNG")?;
        if texture.width() < CAPE_WIDTH || texture.height() < CAPE_HEIGHT {
            return Ok(texture);
        }

        let mut face = PngImage::empty(CAPE_WIDTH, CAPE_HEIGHT);
        for y in 0..CAPE_HEIGHT {
            for x in 0..CAPE_WIDTH {
                face.set_pixel(x, y, texture.pixel(x, y));
            }
        }
        Ok(face)
    }
}
