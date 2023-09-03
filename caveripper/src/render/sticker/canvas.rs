use std::cmp::max;

use image::{RgbaImage, imageops::{overlay, resize, FilterType}, Rgba};

use crate::point::Point;

use super::Sticker;

#[derive(Clone)]
pub struct Canvas {
    buffer: RgbaImage, // TODO: store in a RwLock or something so stickers can be rendered in parallel
}

impl Canvas {
    pub fn new(w: u32, h: u32) -> Self {
        Self{
            buffer: RgbaImage::from_pixel(w, h, [0, 0, 0, 0].into()),
        }
    }

    pub fn into_inner(self) -> RgbaImage {
        self.buffer
    }

    pub fn view(&mut self, offset_x: f32, offset_y: f32) -> CanvasView {
        CanvasView {
            canvas: self,
            offset_x,
            offset_y,
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

    pub fn draw_sticker<H>(&mut self, sticker: &Sticker<H>, helper: &H, x: i64, y: i64) {
        let sticker_rendered = sticker.render(helper);
        overlay(&mut self.buffer, &sticker_rendered.buffer, x, y);
    }

    pub fn draw_pixel(&mut self, x: u32, y: u32, color: Rgba<u8>) {
        if x < self.buffer.width() && y < self.buffer.height() {
            self.buffer.put_pixel(x, y, color);
        }
    }

    pub fn fill(&mut self, start: Point<2, u32>, end: Point<2, u32>, color: Rgba<u8>) {
        for x in start[0]..end[0] {
            for y in start[1]..end[1] {
                self.draw_pixel(x, y, color);
            }
        }
    }

    pub fn overlay(&mut self, top: &RgbaImage, x: f32, y: f32) {
        overlay(&mut self.buffer, top, x as i64, y as i64);
    }
}

/// A helper for editing a particular location on a Canvas as if it were at (0,0).
///
/// Negative coordinates are valid and represent the appropriate locations on the
/// parent Canvas. You can assume a CanvasView has infinite size.
pub struct CanvasView<'c> {
    canvas: &'c mut Canvas,
    offset_x: f32,
    offset_y: f32,
}

impl<'c> CanvasView<'c> {
    pub fn draw_pixel(&mut self, x: f32, y: f32, color: Rgba<u8>) {
        self.canvas.draw_pixel((x + self.offset_x).round() as u32, (y + self.offset_y).round() as u32, color);
    }

    pub fn fill(&mut self, start: Point<2, u32>, end: Point<2, u32>, color: Rgba<u8>) {
        let offset = Point([self.offset_x as u32, self.offset_y as u32]);
        self.canvas.fill(start + offset, end + offset, color);
    }

    pub fn overlay(&mut self, top: &RgbaImage, x: f32, y: f32) {
        self.canvas.overlay(top, x + self.offset_x, y + self.offset_y);
    }
}
