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

use std::{borrow::Cow, marker::PhantomData, path::Path};

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
    shapes::Rectangle,
    util::{CropRelative, Resize},
};
use crate::{
    assets::{get_special_texture_name, AssetManager, ImageKind, Treasure},
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
const QUICKGLANCE_ONION_RED: [u8; 4] = [245, 39, 24, 255];
const QUICKGLANCE_ONION_YELLOW: [u8; 4] = [34, 235, 12, 255];
const QUICKGLANCE_ONION_BLUE: [u8; 4] = [34, 12, 235, 255];
const WAYPOINT_COLOR: [u8; 4] = [130, 199, 56, 255];
const WATERBOX_COLOR: [u8; 4] = [0, 100, 230, 255];
const CARRY_PATH_COLOR: [u8; 4] = [83, 125, 29, 200];
const TREASURE_PATH_COLOR: [u8; 4] = [170,100,255,255];
const CAVEINFO_WIDTH: f32 = 1250.0;
const WAYPOINT_DIST_TXT_COLOR: [u8; 4] = [36, 54, 14, 255];
const HEADER_BACKGROUND: [u8; 4] = [220, 220, 220, 255];
const MAPTILES_BACKGROUND: [u8; 4] = [20, 20, 20, 255];
const GRID_COLOR: [u8; 4] = [255, 0, 0, 150];
const SCORE_TEXT_COLOR: [u8; 4] = [59, 255, 226, 255];
const DISTANCE_SCORE_TEXT_COLOR: [u8; 4] = [99, 147, 242, 255];
const DISTANCE_SCORE_LINE_COLOR: [u8; 4] = [58, 101, 186, 255];
const CAVEINFO_MARGIN: f32 = RENDER_SCALE / 2.0;
const CAVEINFO_UNIT_MARGIN: f32 = CAVEINFO_MARGIN * 3.0;
const CAVEINFO_ICON_SIZE: f32 = 64.0;
const OFF_BLACK: [u8; 4] = [0, 0, 0, 255];
const CAVEINFO_BOXES_FONT_SIZE: f32 = 42.0;

pub struct RenderHelper<'a, M: AssetManager> {
    mgr: &'a M,
    fonts: Vec<Font>,
}

impl<'a, M: AssetManager> RenderHelper<'a, M> {
    pub fn new(mgr: &'a M) -> Self {
        let read_font = |path: &str| -> Font {
            let font_bytes = mgr.load_raw(path).expect("Missing font file!");
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

    fn cropped_text(&self, text: impl Into<String>, size: f32, outline: u32, color: impl Into<Rgba<u8>>) -> impl Render<M> + '_ {
        CropRelative {
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
            phantom: PhantomData,
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

impl<M: AssetManager> Render<M> for CaveUnit {
    fn render(&self, mut canvas: CanvasView, helper: &M) {
        let mut img = helper
            .load_image(ImageKind::CaveUnit, &self.game, &self.unit_folder_name)
            .unwrap()
            .to_owned();

        // Radar images are somewhat dark by default; this improves visibility.
        brighten_in_place(&mut img, 75);

        for _ in 0..self.rotation {
            img = rotate90(&img);
        }

        img = resize(
            &img,
            (self.width as f32 * GRID_FACTOR) as u32,
            (self.height as f32 * GRID_FACTOR) as u32,
            FilterType::Nearest,
        );
        canvas.overlay(&img, Point([0.0, 0.0]));

        // Waterboxes
        for waterbox in self.waterboxes.iter() {
            let mut view = canvas.sub_view((self.center() * GRID_FACTOR) + (waterbox.p1.two_d() * COORD_FACTOR));
            let view2 = view.with_opacity(0.2);
            Rectangle {
                width: waterbox.width() * COORD_FACTOR,
                height: waterbox.height() * COORD_FACTOR,
                color: WATERBOX_COLOR.into(),
            }
            .render(view2, helper);
        }
    }

    fn dimensions(&self) -> Point<2, f32> {
        Point([self.width as f32 * GRID_FACTOR, self.height as f32 * GRID_FACTOR])
    }
}

impl<M: AssetManager> Render<M> for SpawnObject<'_> {
    fn render(&self, mut canvas: CanvasView, helper: &M) {
        match self {
            SpawnObject::Teki(TekiInfo { game, .. }, _) | SpawnObject::CapTeki(CapInfo { game, .. }, _) => {
                let (name, kind) = get_special_texture_name(self.name())
                    .map(ToOwned::to_owned)
                    .map(|special_name| (special_name, ImageKind::Special))
                    .unwrap_or_else(|| (self.name().to_ascii_lowercase(), ImageKind::Teki));
                let teki_img = resize(helper.load_image(kind, game, &name).unwrap(), 40, 40, FilterType::Lanczos3);
                canvas.overlay(&teki_img, Point([0.0, 0.0]));
            }
            SpawnObject::Item(info) => TreasureRenderer {
                treasure: helper
                    .get_treasure_info(&info.game, &info.internal_name)
                    .expect(&format!("Couldn't find treasure {}", &info.internal_name)),
            }
            .render(canvas, helper),
            SpawnObject::Gate(_, rotation) => {
                let mut img = Cow::Borrowed(helper.load_image(ImageKind::Special, "pikmin2", "Gray_bramble_gate_icon").unwrap());
                if rotation % 2 == 1 {
                    img = Cow::Owned(rotate90(img.as_ref()));
                }

                canvas.overlay(img.as_ref(), Point([0.0, 0.0]));
            }
            SpawnObject::Hole(plugged) | SpawnObject::Geyser(plugged) => {
                let name = match self {
                    SpawnObject::Hole(_) => "Cave_icon",
                    SpawnObject::Geyser(_) => "Geyser_icon",
                    _ => unreachable!(),
                };
                let img = helper.load_image(ImageKind::Special, "pikmin2", name).unwrap();
                canvas.overlay(img, Point([0.0, 0.0]));
                if *plugged {
                    let plug_icon = helper.load_image(ImageKind::Special, "pikmin2", "36px-Clog_icon").unwrap();
                    canvas.overlay(&plug_icon, Point([0.0, 0.0]));
                }
            }
            SpawnObject::Ship => {
                canvas.overlay(
                    helper.load_image(ImageKind::Special, "pikmin2", "pod_icon").unwrap(),
                    Point([0.0, 0.0]),
                );
            }
            SpawnObject::Onion(color) => {
                canvas.overlay(
                    helper.load_image(ImageKind::Special, "pikmin2", &format!("onion{color}")).unwrap(),
                    Point([0.0, 0.0]),
                );
            }
        }
    }

    fn dimensions(&self) -> Point<2, f32> {
        match self {
            // TODO: Boss teki and potentially some romhack teki have larger
            // image dimensions. Currently these are all scaled to 40x40 but
            // quality could be better if this can be avoided.
            SpawnObject::Teki(_, _) | SpawnObject::CapTeki(_, _) => Point([40.0, 40.0]),
            SpawnObject::Item(_) => {
                let renderer = TreasureRenderer {
                    treasure: &Treasure::default(),
                };
                <TreasureRenderer as Render<M>>::dimensions(&renderer)
            }
            SpawnObject::Gate(_, _rotation) => Point([48.0, 48.0]),
            SpawnObject::Hole(_) => Point([32.0, 32.0]),
            SpawnObject::Geyser(_) => Point([40.0, 40.0]),
            SpawnObject::Ship => Point([30.0, 30.0]),
            SpawnObject::Onion(_) => Point([24.0, 24.0]),
        }
    }
}

/// Helper to reduce asset manager lookups
struct TreasureRenderer<'a> {
    pub treasure: &'a Treasure,
}
impl<M: AssetManager> Render<M> for TreasureRenderer<'_> {
    fn render(&self, mut canvas: CanvasView, helper: &M) {
        canvas.overlay(
            helper
                .load_image(
                    ImageKind::Treasure,
                    &self.treasure.game,
                    &self.treasure.internal_name.to_ascii_lowercase(),
                )
                .unwrap(),
            Point([0.0, 0.0]),
        );
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
    Ship,
    Exit,
}

impl<M: AssetManager> Render<M> for Icon {
    fn render(&self, mut canvas: CanvasView, helper: &M) {
        let name = match self {
            Icon::Falling => "falling_icon",
            Icon::Star => "star",
            Icon::Plant => "leaf_icon",
            Icon::Treasure => "duck",
            Icon::Poko => "Poko_icon",
            Icon::Ship => "ship",
            Icon::Exit => "cave_white",
        };
        canvas.overlay(helper.load_image(ImageKind::Special, "pikmin2", name).unwrap(), Point([0.0, 0.0]));
    }

    fn dimensions(&self) -> Point<2, f32> {
        Point(match self {
            Icon::Falling => [20.0, 20.0],
            Icon::Star => [64.0, 64.0],
            Icon::Plant => [32.0, 32.0],
            Icon::Treasure => [32.0, 32.0],
            Icon::Poko => [10.0, 12.0],
            Icon::Ship => [24.0, 24.0],
            Icon::Exit => [32.0, 32.0],
        })
    }
}

fn render_spawn_object<'a, 'b: 'a, M: AssetManager>(spawn_object: Cow<'a, SpawnObject<'b>>, mgr: &'a M) -> impl Render<M> + 'a {
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
            carrying: Some(treasure),
            game,
            ..
        },
        _,
    ) = spawn_object.as_ref()
    {
        layer.place(
            Resize::new(
                TreasureRenderer {
                    treasure: mgr
                        .get_treasure_info(game, &treasure)
                        .expect(&format!("Couldn't load treasure {treasure}")),
                },
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
