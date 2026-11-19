use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::widgets::png::PngImage;

const TEXTURE_WIDTH: u16 = 64;
const CAPE_WIDTH: u16 = 10;
const CAPE_HEIGHT: u16 = 16;
const CAPE_SOURCE_X: u16 = 1;
const CAPE_SOURCE_Y: u16 = 1;

/// Extracts the outer face of a Minecraft cape texture for display.
pub struct CapePreview;

impl CapePreview {
    pub fn from_path(path: &Path) -> Result<PngImage> {
        let bytes = std::fs::read(path)
            .with_context(|| format!("read cape PNG from {}", path.display()))?;
        Self::from_png(&bytes)
    }

    pub fn from_png(bytes: &[u8]) -> Result<PngImage> {
        let texture = PngImage::from_png(bytes).context("decode cape PNG")?;
        let unit = texture.width() / TEXTURE_WIDTH;
        if unit == 0
            || texture.width() % TEXTURE_WIDTH != 0
            || texture.height() < (CAPE_HEIGHT + CAPE_SOURCE_Y) * unit
        {
            bail!(
                "unsupported cape size {}x{}, expected a multiple of {TEXTURE_WIDTH}x32",
                texture.width(),
                texture.height()
            );
        }

        let mut face = PngImage::empty(CAPE_WIDTH, CAPE_HEIGHT);
        for y in 0..CAPE_HEIGHT {
            for x in 0..CAPE_WIDTH {
                face.set_pixel(
                    x,
                    y,
                    texture.pixel(
                        CAPE_SOURCE_X * unit + x * unit,
                        CAPE_SOURCE_Y * unit + y * unit,
                    ),
                );
            }
        }
        Ok(face)
    }
}
