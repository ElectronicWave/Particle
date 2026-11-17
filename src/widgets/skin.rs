use std::path::Path;

use anyhow::{Context, Result, bail};
use elemental::core::auth::texture::SkinModel;

use crate::widgets::png::{PngImage, Rgba};

const TEXTURE_WIDTH: u16 = 64;
const TEXTURE_HEIGHT: u16 = 32;
const VIEW_HEIGHT: u16 = 32;

/// Which part of the skin texture to compose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkinView {
    Full,
    Head,
}

#[derive(Debug)]
struct SkinTexture {
    image: PngImage,
}

impl SkinTexture {
    fn from_png(bytes: &[u8]) -> Result<Self> {
        let image = PngImage::from_png(bytes).context("decode skin PNG")?;
        if image.width() != TEXTURE_WIDTH
            || (image.height() != TEXTURE_HEIGHT && image.height() != TEXTURE_WIDTH)
        {
            bail!("unsupported skin size {}x{}", image.width(), image.height());
        }
        Ok(Self { image })
    }

    fn height(&self) -> u16 {
        self.image.height()
    }

    fn pixel(&self, x: u16, y: u16) -> Rgba {
        self.image.pixel(x, y)
    }
}

/// Maps skin texture regions into a standalone raster image.
pub struct SkinPreview;

impl SkinPreview {
    pub fn from_path(path: &Path, model: SkinModel, view: SkinView) -> Result<PngImage> {
        let bytes = std::fs::read(path)
            .with_context(|| format!("read skin PNG from {}", path.display()))?;
        Self::from_png(&bytes, model, view)
    }

    pub fn from_png(bytes: &[u8], model: SkinModel, view: SkinView) -> Result<PngImage> {
        let texture = SkinTexture::from_png(bytes)?;
        match view {
            SkinView::Full => Self::compose_full(&texture, model),
            SkinView::Head => Ok(Self::compose_head(&texture)),
        }
    }

    fn compose_head(texture: &SkinTexture) -> PngImage {
        let mut image = PngImage::empty(8, 8);
        draw_part(
            &mut image,
            texture,
            Part {
                target: (0, 0),
                size: (8, 8),
                source: (8, 8),
                overlay: Some((40, 8)),
                mirrored: false,
            },
        );
        image
    }

    fn compose_full(texture: &SkinTexture, model: SkinModel) -> Result<PngImage> {
        let arm_width = match model {
            SkinModel::Classic => 4,
            SkinModel::Slim => 3,
        };
        let body_x = arm_width;
        let width = body_x * 2 + 8;
        let mut image = PngImage::empty(width, VIEW_HEIGHT);

        let modern = texture.height() == TEXTURE_WIDTH;
        let leg = Part {
            target: (body_x + 4, 20),
            size: (4, 12),
            source: (4, 20),
            overlay: Some((4, 36)),
            mirrored: true,
        };
        draw_part(
            &mut image,
            texture,
            Part {
                target: (body_x, 20),
                mirrored: false,
                ..leg
            },
        );
        draw_part(
            &mut image,
            texture,
            if modern {
                Part {
                    source: (20, 52),
                    overlay: Some((4, 52)),
                    mirrored: false,
                    ..leg
                }
            } else {
                leg
            },
        );

        draw_part(
            &mut image,
            texture,
            Part {
                target: (body_x, 8),
                size: (8, 12),
                source: (20, 20),
                overlay: Some((20, 36)),
                mirrored: false,
            },
        );

        let right_arm = Part {
            target: (0, 8),
            size: (arm_width, 12),
            source: (44, 20),
            overlay: Some((44, 36)),
            mirrored: false,
        };
        draw_part(&mut image, texture, right_arm);
        draw_part(
            &mut image,
            texture,
            if modern {
                Part {
                    target: (body_x + 8, 8),
                    source: (36, 52),
                    overlay: Some((52, 52)),
                    mirrored: false,
                    ..right_arm
                }
            } else {
                Part {
                    target: (body_x + 8, 8),
                    mirrored: true,
                    ..right_arm
                }
            },
        );

        draw_part(
            &mut image,
            texture,
            Part {
                target: (body_x, 0),
                size: (8, 8),
                source: (8, 8),
                overlay: Some((40, 8)),
                mirrored: false,
            },
        );

        Ok(image)
    }
}

#[derive(Debug, Clone, Copy)]
struct Part {
    target: (u16, u16),
    size: (u16, u16),
    source: (u16, u16),
    overlay: Option<(u16, u16)>,
    mirrored: bool,
}

fn draw_part(image: &mut PngImage, texture: &SkinTexture, part: Part) {
    for y in 0..part.size.1 {
        for x in 0..part.size.0 {
            let source_x = if part.mirrored {
                part.size.0 - 1 - x
            } else {
                x
            };
            let mut pixel = texture.pixel(part.source.0 + source_x, part.source.1 + y);
            if let Some(overlay) = part.overlay {
                pixel = pixel.over(texture.pixel(overlay.0 + source_x, overlay.1 + y));
            }
            let target_x = part.target.0 + x;
            let target_y = part.target.1 + y;
            if target_x < image.width() && target_y < image.height() {
                image.set_pixel(target_x, target_y, pixel);
            }
        }
    }
}
