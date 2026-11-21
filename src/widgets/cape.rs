//! Minecraft cape texture layout reference:
//! <https://minecraft.wiki/w/Cape>
//!
//! A cape uses the same 64×32 texture grid as a skin. The outer 10×16 face
//! starts at `(1, 1)`; the surrounding texels belong to the side and top/bottom
//! faces.

use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::widgets::png::PngImage;

const TEXTURE_WIDTH: u16 = 64;
const TEXTURE_HEIGHT: u16 = 32;
const CAPE_X: u16 = 1;
const CAPE_Y: u16 = 1;
const CAPE_WIDTH: u16 = 10;
const CAPE_HEIGHT: u16 = 16;

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
        if (texture.width(), texture.height()) != (TEXTURE_WIDTH, TEXTURE_HEIGHT) {
            bail!(
                "unsupported cape size {}x{}, expected {TEXTURE_WIDTH}x{TEXTURE_HEIGHT}",
                texture.width(),
                texture.height()
            );
        }
        texture.crop(CAPE_X, CAPE_Y, CAPE_WIDTH, CAPE_HEIGHT)
    }
}
