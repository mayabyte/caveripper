use crate::point::Point;

use super::{Render, canvas::{CanvasView, Canvas}, DirectRender};
use image::Rgba;

pub struct Circle {
    pub radius: f32,
    pub color: Rgba<u8>,
}

impl<H> Render<H> for Circle {
    fn render(&self, mut canvas: CanvasView, _helper: &H) {
        for x in 0..self.radius as u32 * 2 {
            for z in 0..self.radius as u32 * 2 {
                if ((self.radius - x as f32).powi(2) + (self.radius - z as f32).powi(2)).sqrt()
                    < self.radius
                {
                    canvas.draw_pixel(x as f32, z as f32, self.color);
                }
            }
        }
    }

    fn dimensions(&self) -> (f32, f32) {
        (self.radius * 2.0, self.radius * 2.0)
    }

}

pub struct Rectangle {
    pub width: f32,
    pub height: f32,
    pub color: Rgba<u8>,
}

impl<H> Render<H> for Rectangle {
    fn render(&self, mut canvas: CanvasView, _helper: &H) {
        canvas.fill(Point([0, 0]), Point([self.width as u32, self.height as u32]), self.color);
    }

    fn dimensions(&self) -> (f32, f32) {
        (self.width, self.height)
    }

}

pub struct Line {
    pub start: Point<2, f32>,
    pub end: Point<2, f32>,
    pub shorten_start: f32, // Units, not percentage
    pub shorten_end: f32,   // Units, not percentage
    pub forward_arrow: bool,
    pub color: Rgba<u8>,
}

impl DirectRender for Line {
    fn render(&self, mut canvas: &mut Canvas) {
        let mut start = self.start;
        let mut end = self.end;
        canvas.reserve(f32::max(start[0] + 1.0, end[0] + 1.0) as u32, f32::max(start[1] + 1.0, end[1] + 1.0) as u32);

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
            canvas.draw_pixel(true_x.round() as u32, true_y.round() as u32, color);
        }
    } else {
        let slope = (y2 - y1) / (x2 - x1);

        for x in (x1.round() as u32)..(x2.round() as u32) {
            let true_x = x as f32 + 0.5;
            let true_y = y1 + (slope * (true_x - x1));
            canvas.draw_pixel(true_x.round() as u32, true_y.round() as u32, color);
        }
    }
}
