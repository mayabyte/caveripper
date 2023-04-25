use super::Render;
use image::{Rgba, RgbaImage};

pub struct Circle {
    pub radius: f32,
    pub color: Rgba<u8>,
}

impl<H> Render<H> for Circle {
    fn render(&self, _helper: &H) -> RgbaImage {
        let mut buffer = RgbaImage::new(self.radius as u32 * 2, self.radius as u32 * 2);
        for x in 0..self.radius as u32 * 2 {
            for z in 0..self.radius as u32 * 2 {
                if ((self.radius - x as f32).powi(2) + (self.radius - z as f32).powi(2)).sqrt()
                    < self.radius
                {
                    buffer.put_pixel(x, z, self.color);
                }
            }
        }
        buffer
    }
}

pub struct Rectangle {
    pub width: f32,
    pub height: f32,
    pub color: Rgba<u8>,
}

impl<H> Render<H> for Rectangle {
    fn render(&self, _helper: &H) -> RgbaImage {
        RgbaImage::from_pixel(
            self.width.round() as u32,
            self.height.round() as u32,
            self.color,
        )
    }
}
