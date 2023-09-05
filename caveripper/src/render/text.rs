
use float_ord::FloatOrd;
use fontdue::{layout::{Layout as FontLayout, LayoutSettings, HorizontalAlign, VerticalAlign, WrapStyle, TextStyle}, Font};
use image::Rgba;

use crate::assets::AssetManager;

use super::{sticker::{Render, canvas::{CanvasView, Canvas}}, util::outline};

pub struct Text<'f> {
    pub text: String,
    pub font: &'f Font,
    pub size: f32,
    pub color: Rgba<u8>,
    pub max_width: Option<u32>,
    pub outline: u32,
}

impl Text<'_> {
    fn layout(&self) -> FontLayout {
        let mut layout = FontLayout::new(fontdue::layout::CoordinateSystem::PositiveYDown);
        layout.reset(&LayoutSettings {
            x: 0f32,
            y: 0f32,
            line_height: 1.0,
            max_width: self.max_width.map(|w| w as f32),
            max_height: None,
            horizontal_align: HorizontalAlign::Left,
            vertical_align: VerticalAlign::Top,
            wrap_style: WrapStyle::Letter,
            wrap_hard_breaks: true,
        });
        layout.append(&[self.font], &TextStyle::new(&self.text, self.size, 0));
        layout
    }
}

fn width_from_layout(layout: &FontLayout) -> f32 {
    layout
        .glyphs()
        .iter()
        .map(|g| FloatOrd(g.x + g.width as f32))
        .max()
        .unwrap_or(FloatOrd(0.0))
        .0
}

impl Render<AssetManager> for Text<'_> {
    fn render(&self, mut canvas: CanvasView, _helper: &AssetManager) {
        let layout = self.layout();

        for glyph in layout.glyphs().iter() {
            let mut base_glyph_canvas = Canvas::new(glyph.width as u32, glyph.height as u32);

            let (metrics, bitmap) = self.font.rasterize_config(glyph.key);
            for (i, v) in bitmap.into_iter().enumerate() {
                let x = (i % metrics.width) as u32;
                let y = (i / metrics.width) as u32;
                base_glyph_canvas.draw_pixel(
                    x as u32, y as u32,
                    [
                        self.color.0[0].saturating_add(255 - v),
                        self.color.0[1].saturating_add(255 - v),
                        self.color.0[2].saturating_add(255 - v),
                        v,
                    ]
                    .into(),
                );
            }

            let base_glyph = base_glyph_canvas.into_inner();

            let outline_canvas = outline(&base_glyph, self.outline);
            canvas.overlay(&outline_canvas, glyph.x - self.outline as f32, glyph.y - self.outline as f32);
            canvas.overlay(&base_glyph, glyph.x, glyph.y);
        }
    }

    fn dimensions(&self) -> (f32, f32) {
        let layout = self.layout();
        (width_from_layout(&layout) + (self.outline as f32 * 2.0), layout.height() + (self.outline as f32 * 2.0))
    }
}