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
    background_color: Rgba<u8>,
}

impl<'r> StickerRenderer<'r> {
    pub fn new(background_color: Option<Rgba<u8>>) -> Self {
        Self {
            root_layer: Layer::new(),
            background_color: background_color.unwrap_or([0, 0, 0, 0].into()),
        }
    }

    /// Adds a layer at (0,0)
    pub fn add_layer(&mut self, layer: Layer<'r>) {
        self.root_layer.place(layer, Point([0.0, 0.0]), Origin::TopLeft);
    }

    pub fn place<'a>(&'a mut self, layer: Layer<'r>, pos: Point<2, f32>, origin: Origin) -> LayerView<'a, 'r> {
        self.root_layer.place(layer, pos, origin)
    }

    pub fn render(&self, helper: &AssetManager) -> RgbaImage {
        let final_bounds = self.root_layer.bounds();
        let final_dims = final_bounds.dims();

        let mut canvas = Canvas::new(final_dims);
        canvas.fill(Point([0.0, 0.0]), final_dims, self.background_color);

        self.root_layer.render(canvas.view(Point([0.0, 0.0])), helper);
        canvas.into_inner()
    }
}

/// A grouping of renderables to be drawn together.
///
/// Layers have transparent backgrounds by default and can be given optional padding
/// and opacity similar to layers in Photoshop.
///
/// [Layer] also implements [Render], so you can compose Layers inside of Layers to
/// create nested groupings of related renderables.
pub struct Layer<'r> {
    renderables: Vec<(Box<dyn Render + 'r>, Bounds)>,
    direct_renderables: Vec<Box<dyn DirectRender>>,
    opacity: f32,
    margin: f32,
    border: f32,
    border_color: Rgba<u8>,
}

impl<'r> Layer<'r> {
    pub fn new() -> Self {
        Self {
            renderables: vec![],
            direct_renderables: vec![],
            opacity: 1.0,
            margin: 0.0,
            border: 0.0,
            border_color: [0, 0, 0, 0].into(),
        }
    }

    /// Sets the opacity of this entire layer. Clamps to [0, 1].
    pub fn set_opacity(&mut self, opacity: f32) {
        self.opacity = clamp(opacity, 0.0, 1.0);
    }

    pub fn set_margin(&mut self, padding: f32) {
        self.margin = f32::max(0.0, padding);
    }

    pub fn set_border(&mut self, thickness: f32, color: impl Into<Rgba<u8>>) {
        self.border = thickness;
        self.border_color = color.into();
    }

    pub fn place(&mut self, renderable: impl Render + 'r, pos: Point<2, f32>, origin: Origin) -> LayerView<'_, 'r> {
        let bounds = origin.to_bounds(&renderable, pos);
        self.renderables.push((Box::new(renderable), bounds));
        LayerView {
            layer: self,
            previous_bounds: bounds,
        }
    }

    pub fn add_direct_renderable(&mut self, renderable: impl DirectRender + 'static) {
        self.direct_renderables.push(Box::new(renderable));
    }

    /// Bounds of the renderable space in this layer, not including border or margin
    fn drawable_bounds(&self) -> Bounds {
        self.renderables
            .iter()
            .map(|(_, bounds)| *bounds)
            .reduce(|acc, bounds| acc.combine(bounds))
            .unwrap_or_default()
    }

    /// Total space this layer occupies including border and margin
    fn bounds(&self) -> Bounds {
        self.drawable_bounds().expand_by(self.margin + self.border)
    }
}

impl<'r> Render for Layer<'r> {
    fn render(&self, mut canvas: CanvasView, helper: &AssetManager) {
        // TODO: opacity

        // Border
        if self.border > 0.0 {
            let b = self.bounds() + Point([self.margin + self.border, self.margin + self.border]);
            // Top
            canvas.fill(b.topleft, Point([b.bottomright[0], b.topleft[1] + self.border]), self.border_color);

            // Left
            canvas.fill(b.topleft, Point([b.topleft[0] + self.border, b.bottomright[1]]), self.border_color);

            // Bottom
            canvas.fill(
                Point([b.topleft[0] + self.border, b.bottomright[1] - self.border]),
                b.bottomright,
                self.border_color,
            );

            // Right
            canvas.fill(
                Point([b.bottomright[0] - self.border, b.topleft[1]]),
                b.bottomright,
                self.border_color,
            );
        }

        // Normal Renderables
        for (renderable, bounds) in self.renderables.iter() {
            let sub_view = canvas.sub_view(bounds.topleft + self.margin + self.border);
            renderable.render(sub_view, helper);
        }

        // DirectRenderables
        let raw_canvas = canvas.into_raw();
        for renderable in self.direct_renderables.iter() {
            renderable.render(raw_canvas);
        }
    }

    fn dimensions(&self) -> Point<2, f32> {
        self.bounds().dims()
    }
}

/// A reference to a [Layer] with added info about the previously placed renderable. Use [place_relative]
/// to place new renderables with an offset from the previous one rather than via layer-global coordinates.
pub struct LayerView<'l, 'r: 'l> {
    layer: &'l mut Layer<'r>,
    previous_bounds: Bounds,
}

impl<'l, 'r: 'l> LayerView<'l, 'r> {
    pub fn place_relative(self, renderable: impl Render + 'r, origin: Origin, offset: Offset) -> LayerView<'l, 'r> {
        let place_at = self.previous_bounds.topleft + offset.from.offset_from_top_left(self.previous_bounds.dims()) + offset.amount;
        self.layer.place(renderable, place_at, origin)
    }

    /// Moves this LayerView to be a single point at the provided origin relative to the
    /// previously placed renderable.
    /// Useful when you want to place renderables in a loop.
    pub fn anchor_next(self, origin: Origin) -> LayerView<'l, 'r> {
        self.place_relative(
            (),
            Origin::TopLeft,
            Offset {
                from: origin,
                amount: Point([0.0, 0.0]),
            },
        )
    }
}

#[auto_impl(&, &mut, Box)]
pub trait Render {
    fn render(&self, canvas: CanvasView, helper: &AssetManager);

    /// The dimensions of the image produced by [render].
    fn dimensions(&self) -> Point<2, f32>;
}

/// API for rendering pixels straight onto the canvas without using the
/// Sticker machinery. When using this API, the implementor is responsible
/// for resizing the canvas to fit what is drawn.
pub trait DirectRender {
    fn render(&self, canvas: &mut Canvas);
}

impl Render for () {
    fn render(&self, _: CanvasView, _: &AssetManager) {}

    fn dimensions(&self) -> Point<2, f32> {
        Point([0.0, 0.0])
    }
}
