use auto_impl::auto_impl;
use image::{Rgba, RgbaImage};
use num::clamp;

use super::{
    canvas::Canvas,
    coords::{Bounds, Offset, Origin},
};
use crate::{assets::AssetManager, point::Point, render::canvas::CanvasView};

pub struct StickerRenderer<'r> {
    root_layer: Layer<'r>,
}

impl<'r> StickerRenderer<'r> {
    pub fn new() -> Self {
        Self { root_layer: Layer::new() }
    }

    pub fn set_global_background_color(&mut self, color: impl Into<Rgba<u8>>) {
        self.root_layer.set_background_color(color.into());
    }

    /// Adds a layer at (0,0)
    pub fn add_layer(&mut self, layer: Layer<'r>) {
        self.root_layer.place(layer, Point([0.0, 0.0]), Origin::TopLeft);
    }

    pub fn place<'a>(&'a mut self, layer: Layer<'r>, pos: Point<2, f32>, origin: Origin) -> &'a mut Self {
        self.root_layer.place(layer, pos, origin);
        self
    }

    pub fn place_relative(&mut self, layer: Layer<'r>, origin: Origin, offset: Offset) -> &mut Self {
        self.root_layer.place_relative(layer, origin, offset);
        self
    }

    pub fn render(&self, helper: &AssetManager) -> RgbaImage {
        let final_bounds = self.root_layer.bounds();
        let final_dims = final_bounds.dims();

        let mut canvas = Canvas::new(final_dims);

        self.root_layer.render(canvas.view(Point([0.0, 0.0])), helper);
        canvas.into_inner()
    }
}

/// A grouping of renderables to be drawn together.
///
/// Layers have transparent backgrounds by default similar to layers in Photoshop. They can
/// optionally be given a background color, margins, borders, and opacity.
///
/// [Layer] also implements [Render], so you can compose Layers inside of Layers to
/// create nested groupings of related renderables.
pub struct Layer<'r> {
    renderables: Vec<(Box<dyn Render + 'r>, Bounds)>,
    background_color: Rgba<u8>,
    opacity: f32,
    margin: f32,
    border: f32,
    border_color: Rgba<u8>,
    prev_bounds: Bounds, // Bounds of the previosly placed renderable for relative placement
}

impl<'r> Layer<'r> {
    pub fn new() -> Self {
        Self {
            renderables: vec![],
            background_color: [0, 0, 0, 0].into(),
            opacity: 1.0,
            margin: 0.0,
            border: 0.0,
            border_color: [0, 0, 0, 0].into(),
            prev_bounds: Bounds {
                topleft: Point([0.0, 0.0]),
                bottomright: Point([0.0, 0.0]),
            },
        }
    }

    /// Creates a layer with just the provided renderable placed at the origin.
    pub fn of(renderable: impl Render + 'r) -> Self {
        let mut layer = Self::new();
        layer.place(renderable, Point([0.0, 0.0]), Origin::TopLeft);
        layer
    }

    pub fn set_background_color(&mut self, color: impl Into<Rgba<u8>>) {
        self.background_color = color.into();
    }

    /// Sets the opacity of this entire layer. Clamps to [0, 1].
    pub fn set_opacity(&mut self, opacity: f32) {
        self.opacity = clamp(opacity, 0.0, 1.0);
    }

    /// Sets the margin for this layer. Clamps to 0 if negative.
    pub fn set_margin(&mut self, padding: f32) {
        self.margin = f32::max(0.0, padding);
    }

    pub fn set_border(&mut self, thickness: f32, color: impl Into<Rgba<u8>>) {
        self.border = thickness;
        self.border_color = color.into();
    }

    pub fn place(&mut self, renderable: impl Render + 'r, pos: Point<2, f32>, origin: Origin) -> &mut Self {
        let bounds = origin.to_bounds(&renderable, pos);
        self.renderables.push((Box::new(renderable), bounds));
        self.prev_bounds = bounds;
        self
    }

    /// Places the renderable relative to the position and bounds of the previously placed renderable.
    pub fn place_relative(&mut self, renderable: impl Render + 'r, origin: Origin, offset: Offset) -> &mut Self {
        let place_at = self.prev_bounds.topleft + offset.from.offset_from_top_left(self.prev_bounds.dims()) + offset.amount;
        self.place(renderable, place_at, origin)
    }

    /// Offsets the previously placed location used by [place_relative] to the given origin.
    /// Useful when you want to place renderables in a loop without special handling for the first object.
    pub fn anchor_next(&mut self, origin: Origin) -> &mut Self {
        self.place_relative(
            (),
            Origin::TopLeft,
            Offset {
                from: origin,
                amount: Point([0.0, 0.0]),
            },
        )
    }

    /// Bounds of the renderable space in this layer, not including border or margin
    fn drawable_bounds(&self) -> Bounds {
        let mut bounds = self
            .renderables
            .iter()
            .map(|(_, bounds)| *bounds)
            .reduce(|acc, bounds| acc.combine(bounds))
            .unwrap_or_default();

        // If a background color will be visible then we always need to include (0,0) in the
        // bounds so it doesn't get clipped out unexpectedly.
        if self.background_color.0[3] > 0 {
            bounds = bounds.combine(Bounds {
                topleft: Point([0.0, 0.0]),
                bottomright: Point([0.0, 0.0]),
            });
        }

        bounds
    }

    /// Total space this layer occupies including border and margin
    fn bounds(&self) -> Bounds {
        self.drawable_bounds().expand_by(self.margin + self.border)
    }
}

impl<'r> Render for Layer<'r> {
    fn render(&self, mut canvas: CanvasView, helper: &AssetManager) {
        let mut canvas2 =
            if self.opacity != 1.0 {
                canvas.with_opacity(self.opacity)
            } else {
                canvas
            };

        let b = self.bounds() + Point([self.margin + self.border, self.margin + self.border]);

        // Background color
        if self.background_color.0[3] > 0 {
            canvas2.fill(b.topleft, b.bottomright, self.background_color);
        }

        // Border
        if self.border > 0.0 {
            // Top
            canvas2.fill(b.topleft, Point([b.bottomright[0], b.topleft[1] + self.border]), self.border_color);

            // Left
            canvas2.fill(b.topleft, Point([b.topleft[0] + self.border, b.bottomright[1]]), self.border_color);

            // Bottom
            canvas2.fill(
                Point([b.topleft[0] + self.border, b.bottomright[1] - self.border]),
                b.bottomright,
                self.border_color,
            );

            // Right
            canvas2.fill(
                Point([b.bottomright[0] - self.border, b.topleft[1]]),
                b.bottomright,
                self.border_color,
            );
        }

        // Normal Renderables
        for (renderable, bounds) in self.renderables.iter() {
            let sub_view = canvas2.sub_view(bounds.topleft + self.margin + self.border);
            renderable.render(sub_view, helper);
        }
    }

    fn dimensions(&self) -> Point<2, f32> {
        self.bounds().dims()
    }
}

#[auto_impl(&, &mut, Box)]
pub trait Render {
    fn render(&self, canvas: CanvasView, helper: &AssetManager);

    /// The dimensions of the image produced by [render].
    fn dimensions(&self) -> Point<2, f32>;
}

impl Render for () {
    fn render(&self, _: CanvasView, _: &AssetManager) {}

    fn dimensions(&self) -> Point<2, f32> {
        Point([0.0, 0.0])
    }
}
