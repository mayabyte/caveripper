use image::Rgba;

pub trait PixelExt {
    fn mul_alpha(&self, multiplier: f32) -> Self;
}

impl PixelExt for Rgba<u8> {
    fn mul_alpha(&self, mut multiplier: f32) -> Self {
        multiplier = multiplier.clamp(0.0, 1.0);
        let mut new = *self;
        new.0[3] = (self.0[3] as f32 * multiplier).round() as u8;
        new
     }
}