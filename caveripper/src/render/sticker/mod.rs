pub mod shapes;
pub mod canvas;

use std::{borrow::Cow, cmp::max, collections::HashMap, cell::OnceCell};

use float_ord::FloatOrd;
use image::{
    imageops::{overlay, FilterType},
    Rgba, RgbaImage,
};
use itertools::Itertools;
use num::clamp;

use self::canvas::{Canvas, CanvasView};


/// Renderer for the Sticker framework. 
/// 
/// `'k` is the lifetime for Sticker keys.
/// 
/// `'i` is the lifetime for borrowed renderable objects placed inside Stickers.
pub struct StickerRenderer<'k, 'i, H> {
    stickers: HashMap<Cow<'k, str>, Sticker<'i, H>>,
    layers: Vec<Layer<'k>>,
    background_color: Rgba<u8>,
}

impl<'k, 'i, H> StickerRenderer<'k, 'i, H> {
    pub fn new(background_color: Option<Rgba<u8>>) -> Self {
        Self {
            stickers: HashMap::new(),
            layers: Vec::new(),
            background_color: background_color.unwrap_or([0, 0, 0, 0].into()),
        }
    }

    /// Adds a layer to the top of the image
    pub fn add_layer(&mut self, layer: Layer<'k>) {
        self.layers.push(layer);
    }

    /// Registers a new Sticker if it's not already present
    pub fn add_sticker_with<F: FnOnce() -> Sticker<'i, H>>(
        &mut self,
        key: impl Into<Cow<'k, str>>,
        f: F,
    ) -> Cow<'k, str> {
        let key = key.into();
        if !self.stickers.contains_key(&key) {
            self.stickers.insert(key.clone(), f());
        }
        key
    }

    pub fn render(&self, helper: &H) -> RgbaImage {
        self.layers.iter().fold(
            RgbaImage::from_pixel(0, 0, self.background_color),
            |mut base, layer| {
                let (layer_rendered, offset_x, offset_y) = layer.render(helper, &self.stickers);

                if layer_rendered.width() > base.width() || layer_rendered.height() > base.height()
                {
                    let new_w = max(layer_rendered.width(), base.width());
                    let new_h = max(layer_rendered.height(), base.height());
                    let mut new_base = RgbaImage::from_pixel(new_w, new_h, self.background_color);
                    overlay(&mut new_base, &base, 0, 0);
                    base = new_base;
                }

                overlay(&mut base, &layer_rendered, offset_x as i64, offset_y as i64);
                base
            },
        )
    }
}

pub struct Layer<'k> {
    stickers: Vec<(Cow<'k, str>, f32, f32)>,
    direct_renderables: Vec<Box<dyn DirectRender>>,
    opacity: f32,
}

impl<'k> Layer<'k> {
    pub fn new() -> Self {
        Self { 
            stickers: vec![], 
            direct_renderables: vec![],
            opacity: 1.0,
        }
    }

    /// Sets the opacity of this entire layer. Clamps to [0, 1].
    pub fn set_opacity(&mut self, opacity: f32) {
        self.opacity = clamp(opacity, 0.0, 1.0);
    }

    pub fn add(&mut self, sticker_name: Cow<'k, str>, x: f32, y: f32) {
        self.stickers.push((sticker_name, x, y));
    }

    pub fn add_direct_renderable(&mut self, renderable: impl DirectRender + 'static) {
        self.direct_renderables.push(Box::new(renderable));
    }

    fn render<H>(
        &self,
        helper: &H,
        stickers: &HashMap<Cow<'k, str>, Sticker<H>>,
    ) -> (RgbaImage, f32, f32) {
        if self.stickers.is_empty() && self.direct_renderables.is_empty() {
            return (RgbaImage::from_pixel(0, 0, [0, 0, 0, 0].into()), 0.0, 0.0);
        }

        // Determine required canvas size
        let (x1, x2, y1, y2): (Vec<_>, Vec<_>, Vec<_>, Vec<_>) = self
            .stickers
            .iter()
            .map(|(sticker_name, x, y)| {
                let (mut min_x, mut max_x, mut min_y, mut max_y) =
                    stickers[sticker_name].extents();
                min_x += x;
                max_x += x;
                min_y += y;
                max_y += y;
                (
                    FloatOrd(min_x),
                    FloatOrd(max_x),
                    FloatOrd(min_y),
                    FloatOrd(max_y),
                )
            })
            .multiunzip();
        let min_x = x1.into_iter().min().unwrap_or(FloatOrd(0.0)).0;
        let max_x = x2.into_iter().max().unwrap_or(FloatOrd(0.0)).0;
        let min_y = y1.into_iter().min().unwrap_or(FloatOrd(0.0)).0;
        let max_y = y2.into_iter().max().unwrap_or(FloatOrd(0.0)).0;

        let canvas_w = (max_x - min_x).round() as u32;
        let canvas_h = (max_y - min_y).round() as u32;

        let mut canvas = Canvas::new(canvas_w, canvas_h);

        for (sticker_name, x, y) in self.stickers.iter() {
            let (x_offset, _, y_offset, _) = stickers[sticker_name].extents();
            let img_x = (x + x_offset - min_x) as i64;
            let img_y = (y + y_offset - min_y) as i64;
            canvas.draw_sticker(&stickers[sticker_name], helper, img_x, img_y);
        }

        // DirectRenderables
        for renderable in self.direct_renderables.iter() {
            renderable.render(&mut canvas);
        }

        let mut buffer = canvas.into_inner();

        if self.opacity < 1.0 {
            buffer.pixels_mut().for_each(|p: &mut _| p.0[3] = (p.0[3] as f32 * self.opacity) as u8);
        }

        (buffer, min_x, min_y)
    }
}

pub struct Sticker<'i, H> {
    obj: Box<dyn Render<H> + 'i>,
    origin: Origin,
    size: Size,
    rendered: OnceCell<Canvas>,
}

impl<'i, H> Sticker<'i, H> {
    pub fn new(obj: impl Render<H> + 'i, origin: Origin, size: Size) -> Self {
        Self {
            obj: Box::new(obj),
            origin,
            size,
            rendered: OnceCell::new(),
        }
    }

    fn render(&self, helper: &H) -> &Canvas {
        self.rendered.get_or_init(|| {
            let dimensions = self.obj.dimensions();
            let mut canvas = Canvas::new(dimensions.0 as u32, dimensions.1 as u32);
            self.obj.render(canvas.view(0.0, 0.0), helper);
            match self.size {
                Size::Native => canvas,
                Size::Absolute(w, h, filter_type) => canvas.resize(w as u32, h as u32, filter_type),
            }
        })
    }

    /// Calculates the highest and lowest offsets the rendered Sticker
    /// will have given its Origin and Size configuration.
    /// Return values are (min_x, max_x, min_y, max_y)
    fn extents(&self) -> (f32, f32, f32, f32) {
        let (base_width, base_height) = self.obj.dimensions();
        let (width, height) = match self.size {
            Size::Native => (base_width, base_height),
            Size::Absolute(width, height, _) => (width, height)
        };

        match self.origin {
            Origin::TopLeft => (0.0, width, 0.0, height),
            Origin::Center => (-width / 2.0, width / 2.0, -height / 2.0, height / 2.0),
        }
    }
}

#[derive(Clone, Copy)]
pub enum Origin {
    TopLeft,
    Center,
}

#[derive(Clone, Copy)]
pub enum Size {
    /// Don't change the size of the image as produced by Render
    Native,

    /// Sets the size of the image to an absolute quantity
    Absolute(f32, f32, FilterType),
}

pub trait Render<H> {
    fn render(&self, canvas: CanvasView, helper: &H);

    /// The dimensions of the image produced by [render].
    fn dimensions(&self) -> (f32, f32);
}

impl<T: Render<H>, H> Render<H> for &T {
    fn render(&self, canvas: CanvasView, helper: &H) {
        (**self).render(canvas, helper)
    }

    fn dimensions(&self) -> (f32, f32) {
        (**self).dimensions()
    }
}

impl<T: Render<H>, H> Render<H> for &mut T {
    fn render(&self, canvas: CanvasView, helper: &H) {
        (**self).render(canvas, helper)
    }

    fn dimensions(&self) -> (f32, f32) {
        (**self).dimensions()
    }
}

/// API for rendering pixels straight onto the canvas without using the
/// Sticker machinery. When using this API, the implementor is responsible
/// for resizing the canvas to fit what is drawn.
pub trait DirectRender {
    fn render(&self, canvas: &mut Canvas);
}