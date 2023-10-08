use std::cmp::max;

use image::{
    imageops::{resize, FilterType},
    Rgba, RgbaImage,
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

    pub fn new_sq(renderable: R, side: f32, filter: FilterType) -> Resize<R> {
        Resize::new(renderable, side, side, filter)
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

#[derive(Default)]
pub struct Crop<R: Render> {
    pub inner: R,
    pub top: f32,
    pub left: f32,
    pub right: f32,
    pub bottom: f32,
}

impl<R: Render> Render for Crop<R> {
    fn render(&self, mut canvas: CanvasView, helper: &AssetManager) {
        let mut sub_canvas = Canvas::new(self.dimensions());
        self.inner.render(sub_canvas.view(Point([-self.left, -self.top])), helper);
        canvas.overlay(&sub_canvas.into_inner(), Point([0.0, 0.0]));
    }

    fn dimensions(&self) -> Point<2, f32> {
        let inner_dims = self.inner.dimensions();
        Point([inner_dims[0] - self.right - self.left, inner_dims[1] - self.bottom - self.top])
    }
}

pub struct Colorize<R: Render> {
    pub renderable: R,
    pub color: Rgba<u8>,
}

impl<R: Render> Render for Colorize<R> {
    fn render(&self, mut canvas: CanvasView, helper: &AssetManager) {
        let mut subcanvas = Canvas::new(self.renderable.dimensions());
        self.renderable.render(subcanvas.view(Point([0.0, 0.0])), helper);

        let mut img = subcanvas.into_inner();
        img.enumerate_pixels_mut().for_each(|px| {
            px.2 .0[0] = self.color.0[0];
            px.2 .0[1] = self.color.0[1];
            px.2 .0[2] = self.color.0[2];
        });

        canvas.overlay(&img, Point([0.0, 0.0]));
    }

    fn dimensions(&self) -> Point<2, f32> {
        self.renderable.dimensions()
    }
}
