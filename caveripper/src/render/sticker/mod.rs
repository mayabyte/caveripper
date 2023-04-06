pub mod shapes;

use std::{borrow::Cow, cmp::max, collections::HashMap};

use float_ord::FloatOrd;
use image::{
    imageops::{overlay, resize, FilterType},
    Rgba, RgbaImage,
};
use itertools::Itertools;
use once_cell::sync::OnceCell;

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
}

impl<'k> Layer<'k> {
    pub fn new() -> Self {
        Self { stickers: vec![] }
    }

    pub fn add(&mut self, sticker_name: Cow<'k, str>, x: f32, y: f32) {
        self.stickers.push((sticker_name, x, y));
    }

    fn render<H>(
        &self,
        helper: &H,
        stickers: &HashMap<Cow<'k, str>, Sticker<'_, H>>,
    ) -> (RgbaImage, f32, f32) {
        if self.stickers.is_empty() {
            return (RgbaImage::from_pixel(0, 0, [0, 0, 0, 0].into()), 0.0, 0.0);
        }

        // Determine required canvas size
        let (x1, x2, y1, y2): (Vec<_>, Vec<_>, Vec<_>, Vec<_>) = self
            .stickers
            .iter()
            .map(|(sticker_name, x, y)| {
                let (mut min_x, mut max_x, mut min_y, mut max_y) =
                    stickers[sticker_name].extents(helper);
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
        let min_x = x1.into_iter().min().unwrap().0;
        let max_x = x2.into_iter().max().unwrap().0;
        let min_y = y1.into_iter().min().unwrap().0;
        let max_y = y2.into_iter().max().unwrap().0;

        let canvas_w = (max_x - min_x).round() as u32;
        let canvas_h = (max_y - min_y).round() as u32;

        let mut canvas = Canvas::new(canvas_w, canvas_h);

        for (sticker_name, x, y) in self.stickers.iter() {
            let (x_offset, _, y_offset, _) = stickers[sticker_name].extents(helper);
            let img_x = (x + x_offset - min_x) as i64;
            let img_y = (y + y_offset - min_y) as i64;
            canvas.draw(&stickers[sticker_name], helper, img_x, img_y);
        }

        (canvas.0, min_x, min_y)
    }
}

pub struct Sticker<'i, H> {
    obj: Renderable<'i, H>,
    origin: Origin,
    size: Size,
    rendered: OnceCell<RgbaImage>,
}

impl<'i, H> Sticker<'i, H> {
    pub fn new(obj: Renderable<'i, H>, origin: Origin, size: Size) -> Self {
        Self {
            obj,
            origin,
            size,
            rendered: OnceCell::new(),
        }
    }

    fn render(&self, helper: &H) -> &RgbaImage {
        self.rendered.get_or_init(|| {
            let img = self.obj.get().render(helper);
            match self.size {
                Size::Native => img,
                Size::Scaled(factor) => resize(
                    &img,
                    (img.width() as f32 * factor) as u32,
                    (img.height() as f32 * factor) as u32,
                    FilterType::Lanczos3, // TODO: make this configurable?
                ),
                Size::Absolute(w, h) => resize(&img, w as u32, h as u32, FilterType::Nearest),
            }
        })
    }

    /// Calculates the highest and lowest offsets of the rendered Sticker
    /// given its Origin configuration.
    /// Return values are (min_x, max_x, min_y, max_y)
    fn extents(&self, helper: &H) -> (f32, f32, f32, f32) {
        let rendered = self.render(helper);
        let width = rendered.width() as f32;
        let height = rendered.height() as f32;

        match self.origin {
            Origin::TopLeft => (0.0, width, 0.0, height),
            Origin::TopRight => (-width, 0.0, 0.0, height),
            Origin::Center => (-width / 2.0, width / 2.0, -height / 2.0, height / 2.0),
            _ => todo!(),
        }
    }
}

pub enum Renderable<'i, H> {
    Borrowed(&'i dyn Render<H>),
    Owned(Box<dyn Render<H> + 'i>),
}

impl<H> Renderable<'_, H> {
    fn get(&self) -> &dyn Render<H> {
        match self {
            Self::Borrowed(v) => *v,
            Self::Owned(v) => v.as_ref(),
        }
    }
}

#[derive(Clone, Copy)]
pub enum Origin {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Center,
    TopCenter,
    BottomCenter,
    LeftCenter,
    RightCenter,
}

#[derive(Clone, Copy)]
pub enum Size {
    /// Don't change the size of the image as produced by Render
    Native,

    /// Multiplies the native size of the image
    Scaled(f32),

    /// Sets the size of the image to an absolute quantity
    Absolute(f32, f32),
}

struct Canvas(RgbaImage);

impl Canvas {
    pub fn new(w: u32, h: u32) -> Self {
        Self(RgbaImage::from_pixel(w, h, [0, 0, 0, 0].into()))
    }

    pub fn draw<H>(&mut self, sticker: &Sticker<H>, helper: &H, x: i64, y: i64) {
        let sticker_rendered = sticker.render(helper);
        overlay(&mut self.0, sticker_rendered, x, y);
    }
}

pub trait Render<H> {
    fn render(&self, helper: &H) -> RgbaImage;
}
