use std::path::Path;

use ::png::{BitDepth, ColorType, Decoder, Transformations};
use anyhow::{Context, Result, bail};
use ratatui::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Color,
    widgets::Widget,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl Rgba {
    pub const TRANSPARENT: Self = Self {
        red: 0,
        green: 0,
        blue: 0,
        alpha: 0,
    };

    pub fn is_visible(self) -> bool {
        self.alpha >= 128
    }

    pub(crate) fn over(self, top: Self) -> Self {
        if top.alpha == 0 {
            return self;
        }
        if top.alpha == 255 {
            return top;
        }
        let top_alpha = u32::from(top.alpha);
        let base_alpha = u32::from(self.alpha) * (255 - top_alpha) / 255;
        let alpha = top_alpha + base_alpha;
        let blend = |top_channel: u8, base_channel: u8| {
            ((u32::from(top_channel) * top_alpha + u32::from(base_channel) * base_alpha) / alpha)
                as u8
        };
        Self {
            red: blend(top.red, self.red),
            green: blend(top.green, self.green),
            blue: blend(top.blue, self.blue),
            alpha: alpha as u8,
        }
    }

    fn color(self) -> Color {
        Color::Rgb(self.red, self.green, self.blue)
    }
}

/// Decoded PNG pixels rendered with half-block terminal characters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PngImage {
    width: u16,
    height: u16,
    pixels: Vec<Rgba>,
}

impl PngImage {
    pub fn from_path(path: &Path) -> Result<Self> {
        let bytes =
            std::fs::read(path).with_context(|| format!("read PNG from {}", path.display()))?;
        Self::from_png(&bytes)
    }

    pub fn from_png(bytes: &[u8]) -> Result<Self> {
        let mut decoder = Decoder::new(std::io::Cursor::new(bytes));
        decoder.set_transformations(Transformations::EXPAND | Transformations::STRIP_16);
        let mut reader = decoder.read_info().context("read PNG header")?;
        let size = reader.output_buffer_size().context("PNG is too large")?;
        let mut buffer = vec![0; size];
        let info = reader.next_frame(&mut buffer).context("decode PNG")?;
        if info.bit_depth != BitDepth::Eight {
            bail!("PNG has unsupported bit depth {:?}", info.bit_depth);
        }
        let width = u16::try_from(info.width).context("PNG width exceeds 65535 pixels")?;
        let height = u16::try_from(info.height).context("PNG height exceeds 65535 pixels")?;

        let data = &buffer[..info.buffer_size()];
        let channels = match info.color_type {
            ColorType::Rgba => 4,
            ColorType::Rgb => 3,
            ColorType::Grayscale => 1,
            ColorType::GrayscaleAlpha => 2,
            ColorType::Indexed => bail!("indexed PNG was not expanded"),
        };
        let expected = usize::from(width) * usize::from(height) * channels;
        if data.len() != expected {
            bail!(
                "PNG buffer has {} bytes, expected {expected} for {width}x{height}",
                data.len()
            );
        }
        let pixels = match info.color_type {
            ColorType::Rgba => data
                .chunks_exact(4)
                .map(|pixel| Rgba {
                    red: pixel[0],
                    green: pixel[1],
                    blue: pixel[2],
                    alpha: pixel[3],
                })
                .collect(),
            ColorType::Rgb => data
                .chunks_exact(3)
                .map(|pixel| Rgba {
                    red: pixel[0],
                    green: pixel[1],
                    blue: pixel[2],
                    alpha: 255,
                })
                .collect(),
            ColorType::Grayscale => data
                .iter()
                .map(|channel| Rgba {
                    red: *channel,
                    green: *channel,
                    blue: *channel,
                    alpha: 255,
                })
                .collect(),
            ColorType::GrayscaleAlpha => data
                .chunks_exact(2)
                .map(|pixel| Rgba {
                    red: pixel[0],
                    green: pixel[0],
                    blue: pixel[0],
                    alpha: pixel[1],
                })
                .collect(),
            ColorType::Indexed => unreachable!("indexed PNG was rejected above"),
        };

        Self::new(width, height, pixels)
    }

    pub fn new(width: u16, height: u16, pixels: Vec<Rgba>) -> Result<Self> {
        let expected = usize::from(width) * usize::from(height);
        if pixels.len() != expected {
            bail!(
                "image buffer has {} pixels, expected {expected} for {width}x{height}",
                pixels.len()
            );
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    pub fn empty(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            pixels: vec![Rgba::TRANSPARENT; usize::from(width) * usize::from(height)],
        }
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    pub fn pixel(&self, x: u16, y: u16) -> Rgba {
        if x >= self.width || y >= self.height {
            return Rgba::TRANSPARENT;
        }
        self.pixels[usize::from(y) * usize::from(self.width) + usize::from(x)]
    }

    pub fn set_pixel(&mut self, x: u16, y: u16, pixel: Rgba) {
        assert!(
            x < self.width && y < self.height,
            "pixel ({x}, {y}) is outside {}x{}",
            self.width,
            self.height
        );
        self.pixels[usize::from(y) * usize::from(self.width) + usize::from(x)] = pixel;
    }
}

impl Widget for &PngImage {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let scale = (area.width / self.width.max(1))
            .min(area.height.saturating_mul(2) / self.height.max(1))
            .max(1);
        let cells_width = self.width.saturating_mul(scale);
        let cells_height = self.height.saturating_mul(scale).div_ceil(2);
        let origin_x = area.x + area.width.saturating_sub(cells_width) / 2;
        let origin_y = area.y + area.height.saturating_sub(cells_height) / 2;

        for cell_y in 0..cells_height {
            let y = origin_y + cell_y;
            if y >= area.bottom() {
                break;
            }
            let top_row = (u32::from(cell_y) * 2) / u32::from(scale);
            let bottom_row = (u32::from(cell_y) * 2 + 1) / u32::from(scale);
            for cell_x in 0..cells_width {
                let x = origin_x + cell_x;
                if x >= area.right() {
                    break;
                }
                let column = (u32::from(cell_x)) / u32::from(scale);
                let top = self.pixel(column as u16, top_row as u16);
                let bottom = self.pixel(column as u16, bottom_row as u16);
                let Some(cell) = buffer.cell_mut(Position::new(x, y)) else {
                    continue;
                };
                match (top.is_visible(), bottom.is_visible()) {
                    (false, false) => {}
                    (true, false) => {
                        cell.set_symbol("▀").set_fg(top.color());
                    }
                    (false, true) => {
                        cell.set_symbol("▄").set_fg(bottom.color());
                    }
                    (true, true) => {
                        cell.set_symbol("▀")
                            .set_fg(top.color())
                            .set_bg(bottom.color());
                    }
                }
            }
        }
    }
}
