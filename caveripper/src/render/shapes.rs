use image::Rgba;

use super::{
    canvas::{Canvas, CanvasView},
    renderer::{DirectRender, Render},
};
use crate::{assets::AssetManager, point::Point, render::util::outline};

pub struct Circle {
    pub radius: f32,
    pub color: Rgba<u8>,
}

impl Render for Circle {
    fn render(&self, mut canvas: CanvasView, _helper: &AssetManager) {
        for x in 0..self.radius as u32 * 2 {
            for z in 0..self.radius as u32 * 2 {
                if ((self.radius - x as f32).powi(2) + (self.radius - z as f32).powi(2)).sqrt() < self.radius {
                    canvas.draw_pixel(Point([x as f32, z as f32]), self.color);
                }
            }
        }
    }

    fn dimensions(&self) -> Point<2, f32> {
        Point([self.radius * 2.0, self.radius * 2.0])
    }
}

pub struct Rectangle {
    pub width: f32,
    pub height: f32,
    pub color: Rgba<u8>,
}

impl Render for Rectangle {
    fn render(&self, mut canvas: CanvasView, _helper: &AssetManager) {
        canvas.fill(Point([0.0, 0.0]), Point([self.width, self.height]), self.color);
    }

    fn dimensions(&self) -> Point<2, f32> {
        Point([self.width, self.height])
    }
}

pub struct Line {
    pub start: Point<2, f32>,
    pub end: Point<2, f32>,
    pub shorten_start: f32, // Units, not percentage
    pub shorten_end: f32,   // Units, not percentage
    pub forward_arrow: bool,
    pub outline: u32, // very broken rn do not use
    pub color: Rgba<u8>,
}

impl Default for Line {
    fn default() -> Self {
        Self {
            start: Point([0.0, 0.0]),
            end: Point([0.0, 0.0]),
            shorten_start: 0.0,
            shorten_end: 0.0,
            forward_arrow: false,
            outline: 0,
            color: [255, 255, 255, 255].into(),
        }
    }
}

impl DirectRender for Line {
    fn render(&self, mut canvas: &mut Canvas) {
        let mut start = self.start;
        let mut end = self.end;
        canvas.reserve(
            f32::max(start[0] + 1.0, end[0] + 1.0) as u32,
            f32::max(start[1] + 1.0, end[1] + 1.0) as u32,
        );

        let vector = (end - start).normalized(); // Unit vector in the direction of the line
        if vector.0.iter().any(|v| v.is_nan()) {
            return;
        }

        // Shorten the line slightly on each end without changing the origin point
        start += vector * self.shorten_start;
        end -= vector * self.shorten_end;

        // Draw main line
        render_basic_line(&mut canvas, start, end, self.color);

        // Draw arrow arms
        if self.forward_arrow {
            let arrow_start_left = end - (vector * 12.0) + (vector.perpendicular() * 6.0);
            let arrow_start_right = end - (vector * 12.0) - (vector.perpendicular() * 6.0);
            render_basic_line(&mut canvas, arrow_start_left, end, self.color);
            render_basic_line(&mut canvas, arrow_start_right, end, self.color);
        }

        if self.outline > 0 {
            let mut outline_canvas = Canvas::from(outline(&canvas.clone().into_inner(), self.outline));
            outline_canvas.overlay(&canvas.clone().into_inner(), Point([self.outline as f32, self.outline as f32]));
            *canvas = outline_canvas;
        }
    }
}

fn render_basic_line(canvas: &mut Canvas, start: Point<2, f32>, end: Point<2, f32>, color: Rgba<u8>) {
    let (mut x1, mut y1, mut x2, mut y2) = (start[0], start[1], end[0], end[1]);
    let steep = (y2 - y1).abs() > (x2 - x1).abs();

    if (steep && y1 > y2) || (!steep && x1 > x2) {
        (x1, x2) = (x2, x1);
        (y1, y2) = (y2, y1);
    }

    if steep {
        let slope = (x2 - x1) / (y2 - y1);

        for y in (y1.round() as u32)..(y2.round() as u32) {
            let true_y = y as f32 + 0.5;
            let true_x = x1 + (slope * (true_y - y1));
            canvas.draw_pixel(Point([true_x, true_y]), color);
        }
    } else {
        let slope = (y2 - y1) / (x2 - x1);

        for x in (x1.round() as u32)..(x2.round() as u32) {
            let true_x = x as f32 + 0.5;
            let true_y = y1 + (slope * (true_x - x1));
            canvas.draw_pixel(Point([true_x, true_y]), color);
        }
    }
}
