pub mod canvas;
pub mod shapes;

use std::cmp::max;

use auto_impl::auto_impl;
use image::{imageops::overlay, Rgba, RgbaImage};
use num::clamp;

use crate::{assets::AssetManager, point::Point};

use self::canvas::{Canvas, CanvasView};

/// Renderer for the Sticker framework.
///
/// `'k` is the lifetime for Sticker keys.
///
/// `'i` is the lifetime for borrowed renderable objects placed inside Stickers.
pub struct StickerRenderer<'r> {
    layers: Vec<Layer<'r>>,
    background_color: Rgba<u8>,
}

impl<'r> StickerRenderer<'r> {
    pub fn new(background_color: Option<Rgba<u8>>) -> Self {
        Self {
            layers: Vec::new(),
            background_color: background_color.unwrap_or([0, 0, 0, 0].into()),
        }
    }

    /// Adds a layer to the top of the image
    pub fn add_layer(&mut self, layer: Layer<'r>) {
        self.layers.push(layer);
    }

    pub fn render(&self, helper: &AssetManager) -> RgbaImage {
        self.layers
            .iter()
            .fold(RgbaImage::from_pixel(0, 0, self.background_color), |mut base, layer| {
                let (layer_rendered, offset) = layer.render(helper);

                if layer_rendered.width() > base.width() || layer_rendered.height() > base.height() {
                    let new_w = max(layer_rendered.width(), base.width());
                    let new_h = max(layer_rendered.height(), base.height());
                    let mut new_base = RgbaImage::from_pixel(new_w, new_h, self.background_color);
                    overlay(&mut new_base, &base, 0, 0);
                    base = new_base;
                }

                overlay(&mut base, &layer_rendered, offset[0] as i64, offset[1] as i64);
                base
            })
    }
}

pub struct Layer<'r> {
    renderables: Vec<(Box<dyn Render + 'r>, Bounds)>,
    direct_renderables: Vec<Box<dyn DirectRender>>,
    opacity: f32,
    padding: f32,
}

impl<'r> Layer<'r> {
    pub fn new() -> Self {
        Self {
            renderables: vec![],
            direct_renderables: vec![],
            opacity: 1.0,
            padding: 0.0,
        }
    }

    /// Sets the opacity of this entire layer. Clamps to [0, 1].
    pub fn set_opacity(&mut self, opacity: f32) {
        self.opacity = clamp(opacity, 0.0, 1.0);
    }

    pub fn set_padding(&mut self, padding: f32) {
        self.padding = f32::max(0.0, padding);
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

    fn render(&self, helper: &AssetManager) -> (RgbaImage, Point<2, f32>) {
        if self.renderables.is_empty() && self.direct_renderables.is_empty() {
            return (RgbaImage::from_pixel(0, 0, [0, 0, 0, 0].into()), Point([0.0, 0.0]));
        }

        // Determine required canvas size
        let canvas_bounds = self
            .renderables
            .iter()
            .map(|(_, bounds)| *bounds)
            .reduce(|acc, bounds| acc.combine(bounds))
            .unwrap();
        let mut canvas = Canvas::new(canvas_bounds.dims() + (self.padding * 2.0));

        for (renderable, bounds) in self.renderables.iter() {
            let canvas_view = canvas.view((bounds.topleft - canvas_bounds.topleft) + self.padding);
            renderable.render(canvas_view, helper);
        }

        // DirectRenderables
        for renderable in self.direct_renderables.iter() {
            renderable.render(&mut canvas);
        }

        let mut buffer = canvas.into_inner();
        if self.opacity < 1.0 {
            buffer
                .pixels_mut()
                .for_each(|p: &mut _| p.0[3] = (p.0[3] as f32 * self.opacity) as u8);
        }

        (buffer, canvas_bounds.topleft)
    }
}

pub struct LayerView<'l, 'r: 'l> {
    layer: &'l mut Layer<'r>,
    previous_bounds: Bounds,
}

impl<'l, 'r: 'l> LayerView<'l, 'r> {
    pub fn place_relative(self, renderable: impl Render + 'r, origin: Origin, offset: Offset) -> LayerView<'l, 'r> {
        let place_at = self.previous_bounds.topleft + offset.from.offset_from_top_left(self.previous_bounds.dims()) + offset.amount;
        self.layer.place(renderable, place_at, origin)
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Bounds {
    pub topleft: Point<2, f32>,
    pub bottomright: Point<2, f32>,
}

fn max_per_dim(a: Point<2, f32>, b: Point<2, f32>) -> Point<2, f32> {
    Point([f32::max(a[0], b[0]), f32::max(a[1], b[1])])
}

fn min_per_dim(a: Point<2, f32>, b: Point<2, f32>) -> Point<2, f32> {
    Point([f32::min(a[0], b[0]), f32::min(a[1], b[1])])
}

impl Bounds {
    pub fn combine(self, other: Bounds) -> Bounds {
        Bounds {
            topleft: min_per_dim(self.topleft, other.topleft),
            bottomright: max_per_dim(self.bottomright, other.bottomright),
        }
    }

    pub fn dims(&self) -> Point<2, f32> {
        self.bottomright - self.topleft
    }
}

pub struct Offset {
    pub from: Origin,
    pub amount: Point<2, f32>,
}

#[derive(Clone, Copy)]
pub enum Origin {
    TopLeft,
    TopRight,
    CenterLeft,
    Center,
    CenterRight,
}

impl Origin {
    /// The delta between the top left of the given object and the chosen origin.
    /// Subtract from the desired position to get the top left coordinate.
    fn offset_from_top_left(&self, dims: Point<2, f32>) -> Point<2, f32> {
        match self {
            Origin::TopLeft => Point([0.0, 0.0]),
            Origin::TopRight => Point([dims[0], 0.0]),
            Origin::Center => dims / 2.0,
            Origin::CenterLeft => Point([0.0, dims[1] / 2.0]),
            Origin::CenterRight => Point([dims[0], dims[1] / 2.0]),
        }
    }

    /// Calculates the bounding box occupied by the given renderable placed at `pos`.
    /// `pos` is the non-normalized position provided by the user.
    fn to_bounds(&self, renderable: &impl Render, pos: Point<2, f32>) -> Bounds {
        let offset = self.offset_from_top_left(renderable.dimensions());
        let topleft = pos - offset;
        Bounds {
            topleft,
            bottomright: topleft + renderable.dimensions(),
        }
    }
}

// pub struct Placement {
//     coords_topleft: Point<2, f32>,
//     coords_bottomright: Point<2, f32>,
// }

// pub struct RenderChain<H> {
//     _phantom_helper: PhantomData<H>,
//     renderables: Vec<(Box<dyn Render<H>>, Placement)>,
// }

// impl<H> Render<H> for RenderChain<H> {
//     fn render(&self, canvas: CanvasView, helper: &H) {

//     }

//     fn dimensions(&self) -> (f32, f32) {
//         let (min_pos, max_pos) = self.renderables.iter()
//             .flat_map(|(_, placement)| [placement.coords_topleft, placement.coords_bottomright])
//             .minmax()
//             .into_option()
//             .unwrap_or_default();
//         let dims = max_pos - min_pos;
//         (dims[0], dims[1])
//     }
// }

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
