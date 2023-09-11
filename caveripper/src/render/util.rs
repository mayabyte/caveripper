use std::cmp::max;

use image::{
    imageops::{resize, FilterType},
    RgbaImage,
};
use num::clamp;

use super::{
    canvas::{Canvas, CanvasView},
    renderer::Render,
};
use crate::{assets::AssetManager, point::Point};

pub fn outline(img: &RgbaImage, thickness: u32) -> RgbaImage {
    let mut border_img = RgbaImage::new(img.width() + (thickness * 2), img.height() + (thickness * 2));
    img.enumerate_pixels().for_each(|(x, y, pix)| {
        if pix.0[3] == 0 {
            return;
        }
        for bx in x..=(x + thickness * 2) {
            let bx = clamp(bx, 0, border_img.width() - 1);
            for by in y..=(y + thickness * 2) {
                let by = clamp(by, 0, border_img.height() - 1);
                let current_alpha = border_img.get_pixel(bx, by).0[3];
                border_img.get_pixel_mut(bx, by).0[3] = max(current_alpha, pix.0[3]);
            }
        }
    });
    border_img
}

pub struct Resize<R: Render> {
    pub renderable: R,
    pub width: f32,
    pub height: f32,
    pub filter: FilterType,
}

impl<R: Render> Resize<R> {
    pub fn new(renderable: R, width: f32, height: f32, filter: FilterType) -> Resize<R> {
        Resize {
            renderable,
            width,
            height,
            filter,
        }
    }
}

impl<R: Render> Render for Resize<R> {
    fn render(&self, mut canvas: CanvasView, helper: &AssetManager) {
        let mut subcanvas = Canvas::new(self.renderable.dimensions());
        self.renderable.render(subcanvas.view(Point([0.0, 0.0])), helper);
        let buffer = resize(
            &subcanvas.into_inner(),
            self.width.round() as u32,
            self.height.round() as u32,
            self.filter,
        );
        canvas.overlay(&buffer, Point([0.0, 0.0]));
    }

    fn dimensions(&self) -> Point<2, f32> {
        Point([self.width, self.height])
    }
}
