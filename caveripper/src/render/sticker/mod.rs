use float_ord::FloatOrd;
use image::{RgbaImage, imageops::overlay};
use itertools::Itertools;
use once_cell::sync::OnceCell;

pub struct StickerRenderer<'i> {
    layers: Vec<Layer<'i>>,
    image_fetcher: fn(&str) -> RgbaImage,
}

impl<'i> StickerRenderer<'i> {
    pub fn new(layers: Vec<Layer<'i>>, image_fetcher: fn(&str) -> RgbaImage) -> Self {
        Self {
            layers,
            image_fetcher,
        }
    }
}

pub struct Layer<'i> {
    stickers: Vec<(Sticker<'i>, f32, f32)>,
}

impl<'i> Layer<'i> {
    pub fn new() -> Self {
        Self {
            stickers: vec![],
        }
    }

    pub fn add(&mut self, sticker: Sticker<'i>, x: f32, y: f32) {
        self.stickers.push((sticker, x, y));
    }

    fn render(&self, image_fetcher: fn(&str) -> RgbaImage) -> RgbaImage {
        if self.stickers.is_empty() {
            return RgbaImage::from_pixel(0, 0, [0,0,0,0].into());
        }

        // Determine required canvas size
        let (x1, x2, y1, y2): (Vec<_>, Vec<_>, Vec<_>, Vec<_>) = self.stickers.iter()
            .map(|(sticker, x, y)| {
                let (mut min_x, mut max_x, mut min_y, mut max_y) = sticker.extents(image_fetcher);
                min_x += x;
                max_x += x;
                min_y += y;
                max_y += y;
                (FloatOrd(min_x), FloatOrd(max_x), FloatOrd(min_y), FloatOrd(max_y))
            })
            .multiunzip();
        let min_x = x1.into_iter().min().unwrap().0;
        let max_x = x2.into_iter().max().unwrap().0;
        let min_y = y1.into_iter().min().unwrap().0;
        let max_y = y2.into_iter().max().unwrap().0;

        let canvas_w = (max_x - min_x).round() as u32;
        let canvas_h = (max_y - min_y).round() as u32;

        let mut canvas = Canvas::new(canvas_w, canvas_h);

        for (sticker, x, y) in self.stickers.iter() {
            let (x_offset, _, y_offset, _) = sticker.extents(image_fetcher);
            let img_x = (x - x_offset) as i64;
            let img_y = (y - y_offset) as i64;
            canvas.draw(sticker, img_x, img_y, image_fetcher);
        }

        canvas.0
    }
}

pub struct Sticker<'i> {
    obj: &'i dyn Render,
    origin: Origin,
    rendered: OnceCell<RgbaImage>,
}

impl<'i> Sticker<'i> {
    pub fn new(obj: &'i dyn Render, origin: Origin) -> Self {
        Self {
            obj,
            origin,
            rendered: OnceCell::new(),
        }
    }

    fn render(&self, image_fetcher: fn(&str) -> RgbaImage) -> &RgbaImage {
        self.rendered.get_or_init(|| self.obj.render(image_fetcher))
    }

    /// Calculates the highest and lowest offsets of the rendered Sticker
    /// given its Origin configuration.
    /// Return values are (min_x, max_x, min_y, max_y)
    fn extents(&self, image_fetcher: fn(&str) -> RgbaImage) -> (f32, f32, f32, f32) {
        let rendered = self.render(image_fetcher);
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

struct Canvas(RgbaImage);

impl Canvas {
    pub fn new(w: u32, h: u32) -> Self {
        Self(RgbaImage::from_pixel(w, h, [0,0,0,0].into()))
    }

    pub fn draw(&mut self, sticker: &Sticker, x: i64, y: i64, image_fetcher: fn(&str) -> RgbaImage) {
        let sticker_rendered = sticker.render(image_fetcher);
        overlay(&mut self.0, sticker_rendered, x, y);
    }
}

pub trait Render {
    fn render(&self, image_fetcher: fn(&str) -> RgbaImage) -> RgbaImage;
}
