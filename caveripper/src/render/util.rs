use std::cmp::max;

use image::{
    imageops::{resize, FilterType},
    Rgba, RgbaImage,
};
use num::clamp;

use super::{
    canvas::{Canvas, CanvasView},
    coords::{Offset, Origin},
    renderer::{Layer, Render},
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

pub fn with_border<'a>(renderable: impl Render + 'a, thickness: f32, color: impl Into<Rgba<u8>>) -> impl Render + 'a {
    let mut layer = Layer::new();
    layer.set_border(thickness, color);
    layer.place(renderable, Point([0.0, 0.0]), Origin::TopLeft);
    layer
}

pub struct Rows<'a> {
    max_width: f32,
    h_margin: f32,
    v_margin: f32,
    renderables: Vec<Box<dyn Render + 'a>>,
}

impl<'a> Rows<'a> {
    pub fn new(max_width: f32, h_margin: f32, v_margin: f32) -> Rows<'a> {
        Rows {
            max_width,
            h_margin,
            v_margin,
            renderables: vec![],
        }
    }

    pub fn add(&mut self, renderable: impl Render + 'a) {
        self.renderables.push(Box::new(renderable));
    }

    fn split_rows(&self) -> Vec<Vec<&(dyn Render + 'a)>> {
        // Inefficient, but I couldn't figure out the right lifetimes for returning a nested iterator
        // using Itertools::batching or similar
        let mut rows = vec![vec![]];
        let mut row_width = 0.0;
        for renderable in self.renderables.iter().map(AsRef::as_ref) {
            row_width += renderable.dimensions()[0];
            if row_width > self.max_width {
                rows.push(vec![]);
                row_width = renderable.dimensions()[0];
            }
            rows.last_mut().unwrap().push(renderable);
            row_width += self.h_margin;
        }
        rows
    }
}

impl<'a> Render for Rows<'a> {
    fn render(&self, canvas: CanvasView, helper: &AssetManager) {
        let mut parent_layer = Layer::new();
        let mut first_row = true;
        for row in self.split_rows().into_iter() {
            let mut layer = Layer::new();
            let mut first_in_row = true;
            for renderable in row.into_iter() {
                let margin = if first_in_row {
                    first_in_row = false;
                    0.0
                } else {
                    self.h_margin
                };
                layer.place_relative(
                    renderable,
                    Origin::TopLeft,
                    Offset {
                        from: Origin::TopRight,
                        amount: Point([margin, 0.0]),
                    },
                );
            }

            let margin = if first_row {
                first_row = false;
                0.0
            } else {
                self.v_margin
            };
            parent_layer.place_relative(
                layer,
                Origin::TopLeft,
                Offset {
                    from: Origin::BottomLeft,
                    amount: Point([0.0, margin]),
                },
            );
        }
        parent_layer.render(canvas, helper);
    }

    fn dimensions(&self) -> Point<2, f32> {
        // Inefficient because it has to iterate over rows twice, but I didn't want to hand-write
        // the loops for this because it'd be more error prone.
        let rows = self.split_rows();
        let width: f32 = rows
            .iter()
            .map(|row| row.iter().map(|r| r.dimensions()[0]).sum::<f32>() + ((row.len() - 1) as f32 * self.h_margin))
            .max_by(f32::total_cmp)
            .unwrap_or_default();
        let height: f32 = rows
            .iter()
            .map(|row| row.iter().map(|r| r.dimensions()[1]).max_by(f32::total_cmp).unwrap_or_default())
            .sum::<f32>()
            + ((rows.len() - 1) as f32 * self.v_margin);

        Point([width, height])
    }
}
