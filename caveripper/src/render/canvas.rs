use std::cmp::max;

use image::{
    imageops::{overlay, resize, FilterType},
    Pixel, Rgba, RgbaImage,
};

use super::pixel_ext::PixelExt;
use crate::point::Point;

#[derive(Clone)]
pub struct Canvas {
    buffer: RgbaImage,
}

#[allow(dead_code)]
impl Canvas {
    pub fn new(dims: Point<2, f32>) -> Self {
        Self {
            buffer: RgbaImage::from_pixel(dims[0] as u32, dims[1] as u32, [0, 0, 0, 0].into()),
        }
    }

    pub fn into_inner(self) -> RgbaImage {
        self.buffer
    }

    /// Create a [CanvasView] into this Canvas that treats `offset` as (0,0).
    pub fn view(&mut self, offset: Point<2, f32>) -> CanvasView {
        CanvasView {
            canvas: self,
            offset,
            opacity: 1.0,
        }
    }

    pub fn width(&self) -> u32 {
        self.buffer.width()
    }

    pub fn height(&self) -> u32 {
        self.buffer.height()
    }

    /// Expands the right and bottom of the canvas to reach the desired width and height.
    ///
    /// If a provided dimension is less than the current one, the current one is preserved.
    /// I.e. this will not crop the image at all.
    pub fn reserve(&mut self, w: u32, h: u32) {
        if w > self.buffer.width() || h > self.buffer.height() {
            let mut new_buffer = RgbaImage::new(max(w, self.buffer.width()), max(h, self.buffer.height()));
            overlay(&mut new_buffer, &self.buffer, 0, 0);
            self.buffer = new_buffer;
        }
    }

    /// Squashes and stretches the canvas into the desired dimensions.
    pub fn resize(&self, w: u32, h: u32, filter_type: FilterType) -> Canvas {
        let new_buffer = resize(&self.buffer, w, h, filter_type);
        Canvas { buffer: new_buffer }
    }

    // pub fn draw_sticker<H>(&mut self, sticker: &Sticker<H>, helper: &H, x: i64, y: i64) {
    //     let sticker_rendered = sticker.render(helper);
    //     overlay(&mut self.buffer, &sticker_rendered.buffer, x, y);
    // }

    pub fn draw_pixel(&mut self, pos: Point<2, f32>, color: Rgba<u8>) {
        let x = pos[0].round() as u32;
        let y = pos[1].round() as u32;
        if x < self.buffer.width() && y < self.buffer.height() {
            let mut pixel = *self.buffer.get_pixel_mut(x, y);
            pixel.blend(&color);
            self.buffer.put_pixel(x, y, pixel);
        }
    }

    pub fn fill(&mut self, start: Point<2, f32>, end: Point<2, f32>, color: Rgba<u8>) {
        for x in (start[0].round() as u32)..(end[0].round() as u32) {
            for y in (start[1].round() as u32)..(end[1].round() as u32) {
                self.draw_pixel(Point([x as f32, y as f32]), color);
            }
        }
    }

    pub fn overlay(&mut self, top: &RgbaImage, pos: Point<2, f32>) {
        overlay(&mut self.buffer, top, pos[0].round() as i64, pos[1].round() as i64);
    }
}

impl From<RgbaImage> for Canvas {
    fn from(buffer: RgbaImage) -> Self {
        Self { buffer }
    }
}

/// A helper for editing a particular location on a Canvas as if it were at (0,0).
///
/// Negative coordinates are valid and represent the appropriate locations on the
/// parent Canvas. You can assume a CanvasView has infinite size.
pub struct CanvasView<'c> {
    canvas: &'c mut Canvas,
    offset: Point<2, f32>,
    opacity: f32, // [0,1] inclusive
}

impl<'c> CanvasView<'c> {
    pub fn draw_pixel(&mut self, pos: Point<2, f32>, color: Rgba<u8>) {
        self.canvas.draw_pixel(pos + self.offset, color.mul_alpha(self.opacity));
    }

    pub fn fill(&mut self, start: Point<2, f32>, end: Point<2, f32>, color: Rgba<u8>) {
        self.canvas
            .fill(start + self.offset, end + self.offset, color.mul_alpha(self.opacity));
    }

    pub fn overlay(&mut self, top: &RgbaImage, pos: Point<2, f32>) {
        if self.opacity < 1.0 {
            let mut top2 = top.clone();
            top2.pixels_mut().for_each(|pixel| {
                *pixel = pixel.mul_alpha(self.opacity);
            });
            self.canvas.overlay(&top2, pos + self.offset);
        }
        self.canvas.overlay(top, pos + self.offset);
    }
}

impl<'d, 'c: 'd> CanvasView<'c> {
    pub fn sub_view(&'d mut self, offset: Point<2, f32>) -> CanvasView<'d> {
        CanvasView {
            canvas: &mut *self.canvas,
            offset: self.offset + offset,
            opacity: self.opacity,
        }
    }

    pub fn with_opacity(&'d mut self, opacity: f32) -> CanvasView<'d> {
        let mut new_view = self.sub_view(Point([0.0, 0.0]));
        new_view.opacity = opacity.clamp(0.0, 1.0);
        new_view
    }
}
