mod canvas;
mod coords;
mod pixel_ext;
mod render_caveinfo;
mod render_layout;
mod renderer;
mod shapes;
mod text;
mod util;

#[cfg(test)]
mod test;

use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

use error_stack::{Result, ResultExt};
use fontdue::{Font, FontSettings};
use image::{
    imageops::{colorops::brighten_in_place, resize, rotate90, FilterType},
    Rgba, RgbaImage,
};
pub use render_caveinfo::*;
pub use render_layout::*;

use self::{
    canvas::CanvasView,
    renderer::Render,
    util::{Crop, Resize},
};
use crate::{
    assets::{get_special_texture_name, AssetManager},
    caveinfo::{CapInfo, CaveUnit, TekiInfo},
    errors::CaveripperError,
    layout::SpawnObject,
    point::Point,
    render::{coords::Origin, renderer::Layer, text::Text},
};

/// Controls how scaled up the whole image is.
/// Only change this to increase or decrease the resolution;
/// all other parameters should depend on this.
const RENDER_SCALE: f32 = 16.0;

const GRID_FACTOR: f32 = 8.0 * RENDER_SCALE;
const COORD_FACTOR: f32 = (8.0 * RENDER_SCALE) / 170.0;
const TEKI_SIZE: f32 = 4.0 * RENDER_SCALE;
const GATE_SIZE: f32 = 8.0 * RENDER_SCALE;
const CARRIED_TREASURE_SIZE: f32 = TEKI_SIZE * 0.75;
const FALLING_CAP_TEKI_SIZE: f32 = TEKI_SIZE * 0.8;
const FALLING_ICON_SIZE: f32 = 1.6 * RENDER_SCALE;
const QUICKGLANCE_CIRCLE_RADIUS: f32 = 5.0 * RENDER_SCALE;
const LAYOUT_BACKGROUND_COLOR: [u8; 4] = [15, 15, 15, 255];
const CAVEINFO_UNIT_BORDER_COLOR: [u8; 4] = [225, 0, 0, 255];
const QUICKGLANCE_TREASURE_COLOR: [u8; 4] = [230, 115, 0, 255];
const QUICKGLANCE_EXIT_COLOR: [u8; 4] = [2, 163, 69, 255];
const QUICKGLANCE_SHIP_COLOR: [u8; 4] = [255, 40, 40, 255];
const QUICKGLANCE_VIOLET_CANDYPOP_COLOR: [u8; 4] = [255, 0, 245, 255];
const QUICKGLANCE_IVORY_CANDYPOP_COLOR: [u8; 4] = [100, 100, 100, 255];
const QUICKGLANCE_ROAMING_COLOR: [u8; 4] = [200, 0, 130, 255];
const WAYPOINT_COLOR: [u8; 4] = [130, 199, 56, 255];
const WATERBOX_COLOR: [u8; 4] = [0, 100, 230, 255];
const CARRY_PATH_COLOR: [u8; 4] = [83, 125, 29, 200];
const CAVEINFO_WIDTH: f32 = 1200.0;
const WAYPOINT_DIST_TXT_COLOR: [u8; 4] = [36, 54, 14, 255];
const HEADER_BACKGROUND: [u8; 4] = [220, 220, 220, 255];
const MAPTILES_BACKGROUND: [u8; 4] = [20, 20, 20, 255];
const GRID_COLOR: [u8; 4] = [255, 0, 0, 150];
const SCORE_TEXT_COLOR: [u8; 4] = [59, 255, 226, 255];
const DISTANCE_SCORE_LINE_COLOR: [u8; 4] = [255, 56, 129, 255];
const CAVEINFO_MARGIN: f32 = RENDER_SCALE / 2.0;
const CAVEINFO_UNIT_MARGIN: f32 = CAVEINFO_MARGIN * 6.0;
const CAVEINFO_ICON_SIZE: f32 = 64.0;
const BLACK: [u8; 4] = [0, 0, 0, 255];
const OFF_BLACK: [u8; 4] = [0, 0, 0, 255];
const CAVEINFO_BOXES_FONT_SIZE: f32 = 42.0;

pub struct RenderHelper<'a> {
    mgr: &'a AssetManager,
    fonts: Vec<Font>,
}

impl<'a> RenderHelper<'a> {
    pub fn new(mgr: &'a AssetManager) -> Self {
        let read_font = |path: &str| -> Font {
            let font_bytes = mgr.get_bytes(path).expect("Missing font file!");
            Font::from_bytes(font_bytes.as_slice(), FontSettings::default()).expect("Failed to create font!")
        };
        Self {
            mgr,
            fonts: vec![
                read_font("resources/BalooChettan2-SemiBold.ttf"),
                read_font("resources/BalooChettan2-ExtraBold.ttf"),
            ],
        }
    }

    fn cropped_text(&self, text: impl Into<String>, size: f32, outline: u32, color: impl Into<Rgba<u8>>) -> impl Render + '_ {
        Crop {
            inner: Text {
                text: text.into(),
                font: if size < 20.0 { &self.fonts[1] } else { &self.fonts[0] },
                size,
                color: color.into(),
                outline,
            },
            top: 0.375 * size,
            left: 0.03125 * size,
            right: 0.0,
            bottom: 0.175 * size,
        }
    }
}

/// Saves a layout image to disc.
/// Filename must end with a `.png` extension.
pub fn save_image<P: AsRef<Path>>(img: &RgbaImage, filename: P) -> Result<(), CaveripperError> {
    img.save_with_format(&filename, image::ImageFormat::Png)
        .change_context(CaveripperError::RenderingError)?;
    Ok(())
}

// fn draw_ring(canvas: &mut RgbaImage, pos: Point<2, f32>, r: f32, color: Rgba<u8>) {
//     for i in 0..=(r as i32) {
//         let offset = i as f32;
//         let height = (r.powi(2) - offset.powi(2)).sqrt();
//         try_blend(canvas, (pos[0] - offset) as i64, (pos[1] + height) as i64, color);
//         try_blend(canvas, (pos[0] - offset) as i64, (pos[1] - height) as i64, color);
//         try_blend(canvas, (pos[0] + offset) as i64, (pos[1] + height) as i64, color);
//         try_blend(canvas, (pos[0] + offset) as i64, (pos[1] - height) as i64, color);
//         try_blend(canvas, (pos[0] - height) as i64, (pos[1] + offset) as i64, color);
//         try_blend(canvas, (pos[0] - height) as i64, (pos[1] - offset) as i64, color);
//         try_blend(canvas, (pos[0] + height) as i64, (pos[1] + offset) as i64, color);
//         try_blend(canvas, (pos[0] + height) as i64, (pos[1] - offset) as i64, color);
//     }
// }

fn joke_time() -> bool {
    use chrono::{Datelike, Duration};
    let now = chrono::Utc::now();
    [-12, 0, 1].into_iter().any(|offset| {
        let time = now + Duration::hours(offset);
        time.month() == 4 && time.day() == 1
    })
}

impl Render for CaveUnit {
    fn render(&self, mut canvas: CanvasView, helper: &AssetManager) {
        // TODO: pass game somehow
        let filename = PathBuf::from_iter(["assets", "pikmin2", "mapunits", &self.unit_folder_name, "arc", "texture.png"]);
        let mut img = helper.get_img(&filename).unwrap().to_owned();

        // Radar images are somewhat dark by default; this improves visibility.
        brighten_in_place(&mut img, 75);

        for _ in 0..self.rotation {
            img = rotate90(&img);
        }

        canvas.overlay(&img, Point([0.0, 0.0]));
    }

    fn dimensions(&self) -> Point<2, f32> {
        Point([self.width as f32 * 8.0, self.height as f32 * 8.0])
    }
}

impl Render for SpawnObject<'_> {
    fn render(&self, mut canvas: CanvasView, helper: &AssetManager) {
        match self {
            SpawnObject::Teki(_, _) | SpawnObject::CapTeki(_, _) => {
                let filename = match get_special_texture_name(self.name()) {
                    Some(special_name) => PathBuf::from_iter(["resources", "enemytex_special", special_name]),
                    None => PathBuf::from_iter(["assets", "pikmin2", "teki", &format!("{}.png", self.name().to_ascii_lowercase())]),
                };
                let teki_img = resize(helper.get_img(filename).unwrap(), 40, 40, FilterType::Lanczos3);
                canvas.overlay(&teki_img, Point([0.0, 0.0]));
            }
            SpawnObject::Item(info) => TreasureRenderer(&info.internal_name).render(canvas, helper),
            SpawnObject::Gate(_, rotation) => {
                let filename = "resources/enemytex_special/Gray_bramble_gate_icon.png";
                let mut img = Cow::Borrowed(helper.get_img(filename).unwrap());
                if rotation % 2 == 1 {
                    img = Cow::Owned(rotate90(img.as_ref()));
                }

                canvas.overlay(img.as_ref(), Point([0.0, 0.0]));
            }
            SpawnObject::Hole(plugged) | SpawnObject::Geyser(plugged) => {
                let filename =
                    match self {
                        SpawnObject::Hole(_) => "resources/enemytex_special/Cave_icon.png",
                        SpawnObject::Geyser(_) => "resources/enemytex_special/Geyser_icon.png",
                        _ => unreachable!(),
                    };
                let img = helper.get_img(filename).unwrap();
                canvas.overlay(img, Point([0.0, 0.0]));
                if *plugged {
                    let plug_filename = "resources/enemytex_special/36px-Clog_icon.png";
                    let plug_icon = helper.get_img(plug_filename).unwrap();
                    canvas.overlay(&plug_icon, Point([0.0, 0.0]));
                }
            }
            SpawnObject::Ship => {
                let filename = "resources/enemytex_special/pod_icon.png";
                canvas.overlay(helper.get_img(filename).unwrap(), Point([0.0, 0.0]));
            }
        }
    }

    fn dimensions(&self) -> Point<2, f32> {
        match self {
            // TODO: Boss teki and potentially some romhack teki have larger
            // image dimensions. Currently these are all scaled to 40x40 but
            // quality could be better if this can be avoided.
            SpawnObject::Teki(_, _) | SpawnObject::CapTeki(_, _) => Point([40.0, 40.0]),
            SpawnObject::Item(info) => TreasureRenderer(&info.internal_name).dimensions(),
            SpawnObject::Gate(_, _rotation) => Point([48.0, 48.0]),
            SpawnObject::Hole(_) => Point([32.0, 32.0]),
            SpawnObject::Geyser(_) => Point([40.0, 40.0]),
            SpawnObject::Ship => Point([30.0, 30.0]),
        }
    }
}

/// Helper to reduce asset manager lookups
struct TreasureRenderer<'a>(pub &'a str);
impl Render for TreasureRenderer<'_> {
    fn render(&self, mut canvas: CanvasView, helper: &AssetManager) {
        let filename = PathBuf::from_iter(["assets", "pikmin2", "treasures", &format!("{}.png", self.0.to_ascii_lowercase())]);
        canvas.overlay(helper.get_img(filename).unwrap(), Point([0.0, 0.0]));
    }

    fn dimensions(&self) -> Point<2, f32> {
        Point([32.0, 32.0])
    }
}

enum Icon {
    Falling,
    Star,
    Plant,
    Treasure,
    Poko,
}

impl Render for Icon {
    fn render(&self, mut canvas: CanvasView, helper: &AssetManager) {
        let filename = match self {
            Icon::Falling => "resources/enemytex_special/falling_icon.png",
            Icon::Star => "resources/enemytex_special/star.png",
            Icon::Plant => "resources/enemytex_special/leaf_icon.png",
            Icon::Treasure => "resources/enemytex_special/duck.png",
            Icon::Poko => "resources/enemytex_special/Poko_icon.png",
        };
        canvas.overlay(helper.get_img(filename).unwrap(), Point([0.0, 0.0]));
    }

    fn dimensions(&self) -> Point<2, f32> {
        Point(match self {
            Icon::Falling => [20.0, 20.0],
            Icon::Star => [64.0, 64.0],
            Icon::Plant => [32.0, 32.0],
            Icon::Treasure => [32.0, 32.0],
            Icon::Poko => [10.0, 12.0],
        })
    }
}

fn render_spawn_object<'a, 'b: 'a>(spawn_object: Cow<'a, SpawnObject<'b>>) -> impl Render + 'a {
    let mut layer = Layer::new();
    let mut pos = Point([0.0, 0.0]);

    // Main Spawn Object image
    let size = match spawn_object.as_ref() {
        SpawnObject::Gate(_, _) => GATE_SIZE,
        SpawnObject::CapTeki(CapInfo { spawn_method: Some(_), .. }, _) => {
            pos = pos - RENDER_SCALE;
            FALLING_CAP_TEKI_SIZE
        }
        _ => TEKI_SIZE,
    };

    layer.place(
        Resize::new(spawn_object.clone().into_owned(), size, size, FilterType::Lanczos3),
        pos,
        Origin::TopLeft,
    );

    // Carrying Treasures
    if let SpawnObject::Teki(
        TekiInfo {
            carrying: Some(treasure), ..
        },
        _,
    ) = spawn_object.as_ref()
    {
        layer.place(
            Resize::new(
                TreasureRenderer(treasure),
                CARRIED_TREASURE_SIZE,
                CARRIED_TREASURE_SIZE,
                FilterType::Lanczos3,
            ),
            pos + (size * 0.4),
            Origin::TopLeft,
        );
    }

    // Falling indicator
    if let SpawnObject::Teki(TekiInfo { spawn_method: Some(_), .. }, _) | SpawnObject::CapTeki(CapInfo { spawn_method: Some(_), .. }, _) =
        spawn_object.as_ref()
    {
        layer.place(
            Resize::new(Icon::Falling, FALLING_ICON_SIZE, FALLING_ICON_SIZE, FilterType::Lanczos3),
            pos,
            Origin::TopLeft,
        );
    }

    layer
}
