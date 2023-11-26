mod canvas;
mod coords;
mod pixel_ext;
mod renderer;
mod shapes;
mod text;
mod util;

#[cfg(test)]
mod test;

use std::{
    borrow::Cow,
    cell::RefCell,
    cmp::max,
    path::{Path, PathBuf},
};

use clap::Args;
use error_stack::{Result, ResultExt};
use fontdue::{Font, FontSettings};
use image::{
    imageops::{colorops::brighten_in_place, resize, rotate90, FilterType},
    Rgba, RgbaImage,
};
use log::info;

use self::{
    canvas::CanvasView,
    coords::Offset,
    renderer::Render,
    shapes::Rectangle,
    util::{with_border, Colorize, Crop, Resize},
};
use crate::{
    assets::{get_special_texture_name, AssetManager},
    caveinfo::{CapInfo, CaveInfo, CaveUnit, ItemInfo, TekiInfo},
    errors::CaveripperError,
    layout::{Layout, PlacedMapUnit, SpawnObject},
    point::Point,
    render::{
        coords::Origin,
        renderer::{Layer, StickerRenderer},
        shapes::{Circle, Line},
        text::Text,
    },
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
const WAYPOINT_DIST_TXT_COLOR: [u8; 4] = [36, 54, 14, 255];
const HEADER_BACKGROUND: [u8; 4] = [220, 220, 220, 255];
const MAPTILES_BACKGROUND: [u8; 4] = [20, 20, 20, 255];
const GRID_COLOR: [u8; 4] = [255, 0, 0, 150];
const SCORE_TEXT_COLOR: [u8; 4] = [59, 255, 226, 255];
const DISTANCE_SCORE_LINE_COLOR: [u8; 4] = [255, 56, 129, 255];
const CAVEINFO_MARGIN: f32 = RENDER_SCALE / 2.0;
const CAVEINFO_ICON_SIZE: f32 = 64.0;
const BLACK: [u8; 4] = [0, 0, 0, 255];
const OFF_BLACK: [u8; 4] = [0, 0, 0, 255];
const CAVEINFO_BOXES_FONT_SIZE: f32 = 42.0;

const fn group_color(group: u32) -> [u8; 4] {
    match group {
        0 => [250, 87, 207, 255],  // Easy Teki (120 Alpha for circles)
        1 => [201, 2, 52, 255],    // Hard Teki
        2 => [230, 115, 0, 255],   // Treasures
        5 => [133, 133, 133, 255], // Seam Teki
        6 => [59, 148, 90, 255],   // Plants
        7 => [230, 50, 86, 255],   // Ship spawns
        8 => [89, 6, 138, 255],    // Special teki
        9 => [45, 173, 167, 255],  // Fake capteki / hallway spawnpoint group
        _ => panic!("Invalid teki group in tekiinfo"),
    }
}

const fn group_score(group: u32) -> u32 {
    match group {
        0 => 2,
        1 => 10,
        5 => 5,
        _ => 0,
    }
}

#[derive(Default, Debug, Args)]
#[clap(next_help_heading = "Rendering options")]
pub struct LayoutRenderOptions {
    /// Draw grid lines corresponding to map unit grid boundaries.
    #[clap(long)]
    pub draw_grid: bool,

    /// Draw highlight circles behind important objects in layouts.
    #[clap(long, short='q', default_value_t=true, action=clap::ArgAction::Set)]
    pub quickglance: bool,

    /// Draw circles indicating gauge activation range around treasures.
    /// The larger circle indicates when the gauge needle will start to go
    /// up, and the smaller circle indicates when you'll start to get
    /// audible gauge pings.
    #[clap(long)]
    pub draw_gauge_range: bool,

    /// Draws the score of each unit in the layout.
    #[clap(long, short = 's')]
    pub draw_score: bool,

    /// Draws waypoints and their connections in the layout
    #[clap(long, short = 'w')]
    pub draw_waypoints: bool,

    #[clap(long, short = 'c')]
    pub draw_comedown_square: bool,
}

#[derive(Default, Debug, Args)]
#[clap(next_help_heading = "Rendering options")]
pub struct CaveinfoRenderOptions {
    /// Draw treasure values and carry weights.
    #[clap(long, default_value_t=true, action=clap::ArgAction::Set)]
    pub draw_treasure_info: bool,

    /// Render pathing waypoints
    #[clap(long, short='w', default_value_t=true, action=clap::ArgAction::Set)]
    pub draw_waypoints: bool,

    /// Render waypoint distances. Useful for calculating Distance Score.
    #[clap(long, default_value_t=true, action=clap::ArgAction::Set)]
    pub draw_waypoint_distances: bool,
}

pub struct Renderer<'a> {
    mgr: &'a AssetManager,
    fonts: Vec<Font>,
}

impl<'k, 'a: 'k, 'l: 'a> Renderer<'a> {
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

    pub fn render_layout(&self, layout: &Layout, options: LayoutRenderOptions) -> Result<RgbaImage, CaveripperError> {
        info!("Drawing layout image...");

        let mut renderer = StickerRenderer::new();
        renderer.set_global_background_color(LAYOUT_BACKGROUND_COLOR);

        /* Map Units */
        let (map_unit_layer, mut waterbox_layer) = render_map_units(layout.map_units.iter());
        waterbox_layer.set_opacity(0.2);
        renderer.add_layer(map_unit_layer);
        renderer.add_layer(waterbox_layer);

        /* Waypoints */
        if options.draw_waypoints {
            let mut waypoint_circle_layer = Layer::new();
            waypoint_circle_layer.set_opacity(0.6);
            for wp in layout.waypoint_graph().iter() {
                waypoint_circle_layer.place(
                    Circle {
                        radius: wp.r * COORD_FACTOR / 1.7,
                        color: WAYPOINT_COLOR.into(),
                    },
                    wp.pos.two_d() * COORD_FACTOR,
                    Origin::Center,
                );
            }
            renderer.add_layer(waypoint_circle_layer);

            let mut waypoint_arrow_layer = Layer::new();
            for wp in layout.waypoint_graph().iter() {
                if let Some(backlink) = layout.waypoint_graph().backlink(wp) {
                    if backlink.pos.dist(&wp.pos) < 0.01 {
                        continue;
                    }
                    waypoint_arrow_layer.add_direct_renderable(Line {
                        start: (wp.pos * COORD_FACTOR).two_d(),
                        end: (backlink.pos * COORD_FACTOR).two_d(),
                        shorten_start: 6.0,
                        shorten_end: 6.0,
                        forward_arrow: true,
                        color: CARRY_PATH_COLOR.into(),
                        ..Default::default()
                    });
                }
            }
            renderer.add_layer(waypoint_arrow_layer);
        }

        /* Spawn Objects */
        let mut spawn_object_layer = Layer::new();
        let mut quickglance_circle_layer = Layer::new();
        quickglance_circle_layer.set_opacity(0.45);

        for (spawn_object, pos) in layout.get_spawn_objects() {
            let so_renderable = render_spawn_object(Cow::Borrowed(spawn_object));
            spawn_object_layer.place(so_renderable, pos.two_d() * COORD_FACTOR, Origin::Center);

            // Quickglance Circles
            if options.quickglance {
                let color = match spawn_object {
                    SpawnObject::Teki(TekiInfo { carrying: Some(_), .. }, _) | SpawnObject::Item(_) => Some(QUICKGLANCE_TREASURE_COLOR),
                    SpawnObject::Teki(TekiInfo { internal_name, .. }, _) | SpawnObject::CapTeki(CapInfo { internal_name, .. }, _) => {
                        match internal_name.to_ascii_lowercase().as_str() {
                            "whitepom" => Some(QUICKGLANCE_IVORY_CANDYPOP_COLOR),
                            "blackpom" => Some(QUICKGLANCE_VIOLET_CANDYPOP_COLOR),
                            "minihoudai" | "kumochappy" | "leafchappy" => Some(QUICKGLANCE_ROAMING_COLOR),
                            _ => None,
                        }
                    }
                    SpawnObject::Hole(_) | SpawnObject::Geyser(_) => Some(QUICKGLANCE_EXIT_COLOR),
                    SpawnObject::Ship => Some(QUICKGLANCE_SHIP_COLOR),
                    _ => None,
                };
                if let Some(color) = color {
                    quickglance_circle_layer.place(
                        Circle {
                            radius: QUICKGLANCE_CIRCLE_RADIUS,
                            color: color.into(),
                        },
                        pos.two_d() * COORD_FACTOR,
                        Origin::Center,
                    );
                }
            }
        }
        renderer.add_layer(quickglance_circle_layer);
        renderer.add_layer(spawn_object_layer);

        /* Unit Grid */
        if options.draw_grid {
            let mut grid_layer = Layer::new();
            let map_dims = layout.map_units.iter().fold((0, 0), |dims, unit| {
                (
                    max(dims.0, unit.x + unit.unit.width as i32),
                    max(dims.1, unit.z + unit.unit.height as i32),
                )
            });

            for x in 0..map_dims.0 {
                grid_layer.add_direct_renderable(Line {
                    start: Point([x as f32 * GRID_FACTOR, 0.0]),
                    end: Point([x as f32 * GRID_FACTOR, map_dims.1 as f32 * GRID_FACTOR]),
                    color: GRID_COLOR.into(),
                    ..Default::default()
                });
            }

            for y in 0..map_dims.1 {
                grid_layer.add_direct_renderable(Line {
                    start: Point([0.0, y as f32 * GRID_FACTOR]),
                    end: Point([map_dims.0 as f32 * GRID_FACTOR, y as f32 * GRID_FACTOR]),
                    color: GRID_COLOR.into(),
                    ..Default::default()
                })
            }

            renderer.add_layer(grid_layer);
        }

        /* Score */
        if options.draw_score {
            let mut distance_score_line_layer = Layer::new();
            let mut distance_score_text_layer = Layer::new();
            let mut score_text_layer = Layer::new();

            for unit in layout.map_units.iter() {
                // Total unit score
                let text = if unit.teki_score > 0 {
                    format!("{} (Teki: {})", unit.total_score, unit.teki_score)
                } else {
                    format!("{}", unit.total_score)
                };
                score_text_layer.place(
                    Text {
                        text,
                        font: &self.fonts[1],
                        size: 24.0,
                        color: SCORE_TEXT_COLOR.into(),
                        outline: 2,
                    },
                    Point([
                        (unit.x as f32 + (unit.unit.width as f32 / 2.0)) * GRID_FACTOR,
                        (unit.z as f32 + (unit.unit.height as f32 / 2.0)) * GRID_FACTOR,
                    ]),
                    Origin::Center,
                );

                // Distance score
                for door in unit.doors.iter() {
                    let door = RefCell::borrow(door);
                    for link in door.door_unit.door_links.iter() {
                        let this_door_pos = door.center();
                        let other_door_pos = RefCell::borrow(&unit.doors[link.door_id]).center();
                        distance_score_line_layer.add_direct_renderable(Line {
                            start: this_door_pos.two_d() * COORD_FACTOR,
                            end: other_door_pos.two_d() * COORD_FACTOR,
                            shorten_start: 8.0,
                            shorten_end: 8.0,
                            color: DISTANCE_SCORE_LINE_COLOR.into(),
                            ..Default::default()
                        });

                        let midpoint = ((this_door_pos + other_door_pos) / 2.0) * COORD_FACTOR;
                        let distance_score = (link.distance / 10.0).round() as u32;
                        distance_score_text_layer.place(
                            Text {
                                text: format!("{}", distance_score),
                                font: &self.fonts[1],
                                size: 24.0,
                                color: DISTANCE_SCORE_LINE_COLOR.into(),
                                outline: 2,
                            },
                            midpoint.two_d(),
                            Origin::Center,
                        );
                    }
                }
            }

            renderer.add_layer(distance_score_line_layer);
            renderer.add_layer(distance_score_text_layer);
            renderer.add_layer(score_text_layer);
        }

        Ok(renderer.render(self.mgr))
    }

    pub fn render_caveinfo(&self, caveinfo: &CaveInfo, options: CaveinfoRenderOptions) -> Result<RgbaImage, CaveripperError> {
        let mut renderer = StickerRenderer::new();
        renderer.set_global_background_color(HEADER_BACKGROUND);

        let mut title_row = Layer::new();
        title_row.set_margin(CAVEINFO_MARGIN);

        let metadata_icon_offset = Offset {
            from: Origin::TopRight,
            amount: Point([CAVEINFO_MARGIN * 2.0, 0.0]),
        };

        title_row
            .place(
                self.cropped_text(caveinfo.long_name(), 88.0, 0, OFF_BLACK),
                Point([0.0, 0.0]),
                Origin::TopLeft,
            )
            .place_relative(
                Resize::new_sq(SpawnObject::Ship, CAVEINFO_ICON_SIZE, FilterType::Lanczos3),
                Origin::TopLeft,
                metadata_icon_offset,
            );

        // -- Metadata icons - ship, hole plugged/unplugged, geyser yes/no, num gates -- //
        if !caveinfo.is_final_floor {
            title_row.place_relative(
                Resize::new_sq(SpawnObject::Hole(caveinfo.exit_plugged), CAVEINFO_ICON_SIZE, FilterType::Lanczos3),
                Origin::TopLeft,
                metadata_icon_offset,
            );
        }

        if caveinfo.is_final_floor || caveinfo.has_geyser {
            title_row.place_relative(
                Resize::new_sq(
                    SpawnObject::Geyser(caveinfo.is_challenge_mode() && caveinfo.is_final_floor),
                    CAVEINFO_ICON_SIZE,
                    FilterType::Lanczos3,
                ),
                Origin::TopLeft,
                metadata_icon_offset,
            );
        }

        // Assume only one GateInfo
        if let Some(gateinfo) = caveinfo.gate_info.first() {
            let mut gate_metadata_icon = Layer::new();
            gate_metadata_icon
                .place(
                    Crop {
                        inner: Resize::new_sq(SpawnObject::Gate(gateinfo, 0), CAVEINFO_ICON_SIZE, FilterType::Lanczos3),
                        top: CAVEINFO_ICON_SIZE / 3.6,
                        left: 0.0,
                        right: 0.0,
                        bottom: CAVEINFO_ICON_SIZE / 3.4,
                    },
                    Point([0.0, 0.0]),
                    Origin::TopLeft,
                )
                .place_relative(
                    self.cropped_text(format!("{}HP", gateinfo.health.round() as u32), 13.0, 0, OFF_BLACK),
                    Origin::TopCenter,
                    Offset {
                        from: Origin::BottomCenter,
                        amount: Point([0.0, 0.0]),
                    },
                )
                .place_relative(
                    self.cropped_text(format!("x{}", caveinfo.max_gates), 19.0, 0, OFF_BLACK),
                    Origin::TopCenter,
                    Offset {
                        from: Origin::BottomCenter,
                        amount: Point([0.0, 0.0]),
                    },
                );
            title_row.place_relative(gate_metadata_icon, Origin::TopLeft, metadata_icon_offset);
        }

        renderer.place(title_row, Point([0.0, 0.0]), Origin::TopLeft);

        // --- Spawn Object Info Boxes -- //
        let mut spawn_object_layer = Layer::new();
        let teki_box_offset = Offset {
            from: Origin::TopRight,
            amount: Point([CAVEINFO_MARGIN, 0.0]),
        };

        let groups: [(&str, u32, Box<dyn Render>); 5] = [
            ("Special", 8, Box::new(Icon::Star)),
            (
                "Hard",
                1,
                Box::new(Circle {
                    radius: 32.0,
                    color: group_color(1).into(),
                }),
            ),
            (
                "Easy",
                0,
                Box::new(Circle {
                    radius: 32.0,
                    color: group_color(0).into(),
                }),
            ),
            ("Plant", 6, Box::new(Icon::Plant)),
            ("Seam", 5, Box::new(())), // TODO: Seam teki icon??
        ];

        for (name, group_num, icon) in groups.into_iter() {
            let mut teki = caveinfo.teki_group(group_num).peekable();
            if teki.peek().is_none() {
                continue;
            }

            spawn_object_layer.place_relative(
                self.caveinfo_entity_box(
                    name,
                    icon,
                    group_color(group_num),
                    teki.map(|info| SpawnObject::Teki(info, Point([0.0, 0.0, 0.0]))),
                    group_score(group_num),
                    &caveinfo.cave_cfg.game,
                ),
                Origin::TopLeft,
                teki_box_offset,
            );
        }

        // Cap teki
        if caveinfo.cap_info.len() > 0 {
            spawn_object_layer.place_relative(
                self.caveinfo_entity_box(
                    "Cap",
                    (),
                    group_color(9),
                    caveinfo
                        .cap_info
                        .iter()
                        // We don't want the special treatment cap teki get for layout rendering
                        .map(|info| SpawnObject::Teki(info.as_ref(), Point([0.0, 0.0, 0.0]))),
                    0,
                    &caveinfo.cave_cfg.game,
                ),
                Origin::TopLeft,
                teki_box_offset,
            );
        }

        // Treasures
        if caveinfo.item_info.len() > 0 {
            spawn_object_layer.place_relative(
                self.caveinfo_entity_box(
                    "Treasure",
                    Icon::Treasure,
                    group_color(2),
                    caveinfo.item_info.iter().map(|info| SpawnObject::Item(info)),
                    0,
                    &caveinfo.cave_cfg.game,
                ),
                Origin::TopLeft,
                teki_box_offset,
            );
        }

        renderer.place_relative(
            spawn_object_layer,
            Origin::TopLeft,
            Offset {
                from: Origin::BottomLeft,
                amount: Point([0.0, 0.0]),
            },
        );

        // -- Cave Units -- //
        let mut unit_layer = Layer::new();
        unit_layer.set_margin(CAVEINFO_MARGIN);
        unit_layer.set_background_color(MAPTILES_BACKGROUND);

        for unit in caveinfo.cave_units.iter().filter(|unit| unit.rotation == 0) {
            let mut this_unit_layer = Layer::new();
            this_unit_layer.place(
                with_border(
                    Resize::new(
                        unit,
                        unit.width as f32 * GRID_FACTOR * 0.75,
                        unit.height as f32 * GRID_FACTOR * 0.75,
                        FilterType::Nearest,
                    ),
                    1.0,
                    CAVEINFO_UNIT_BORDER_COLOR,
                ),
                Point([0.0, 0.0]),
                Origin::TopLeft,
            );
            this_unit_layer.place_relative(
                Text {
                    text: unit.unit_folder_name.clone(),
                    font: &self.fonts[1],
                    size: 14.0,
                    color: [255, 255, 255, 255].into(),
                    outline: 0,
                },
                Origin::TopLeft,
                Offset {
                    from: Origin::BottomLeft,
                    amount: Point([0.0, 0.0]),
                },
            );

            unit_layer.place_relative(
                this_unit_layer,
                Origin::TopLeft,
                Offset {
                    from: Origin::TopRight,
                    amount: Point([CAVEINFO_MARGIN * 6.0, 0.0]),
                },
            );
        }

        renderer.place_relative(
            unit_layer,
            Origin::TopLeft,
            Offset {
                from: Origin::BottomLeft,
                amount: Point([0.0, CAVEINFO_MARGIN * 2.0]),
            },
        );

        // let mut canvas_maptiles =
        //     RgbaImage::from_pixel(canvas_header.width(), 200, MAPTILES_BACKGROUND.into());

        // let maptiles_metadata_txt = self.render_text(
        //     &format!(
        //         "Num Rooms: {}     CorridorBetweenRoomsProb: {}%     CapOpenDoorsProb: {}%",
        //         caveinfo.num_rooms,
        //         caveinfo.corridor_probability * 100.0,
        //         caveinfo.cap_probability * 100.0
        //     ),
        //     24.0,
        //     [220, 220, 220, 255].into(),
        //     Some(canvas_maptiles.width()),
        // );
        // overlay(
        //     &mut canvas_maptiles,
        //     &maptiles_metadata_txt,
        //     canvas_header.width() as i64 / 2 - maptiles_metadata_txt.width() as i64 / 2,
        //     0,
        // );

        // let maptile_margin = (RENDER_SCALE * 4.0) as i64;
        // let mut base_x = maptile_margin;
        // let mut base_y = maptiles_metadata_txt.height() as i64 + maptile_margin;
        // let mut max_y = base_y;

        // let rooms = caveinfo
        //     .cave_units
        //     .iter()
        //     .filter(|unit| unit.rotation == 0)
        //     .filter(|unit| unit.room_type == RoomType::Room);

        // let caps: Vec<_> = caveinfo
        //     .cave_units
        //     .iter()
        //     .filter(|unit| unit.rotation == 0)
        //     .filter(|unit| unit.room_type == RoomType::DeadEnd)
        //     .collect();

        // for (i, unit) in caps.iter().enumerate() {
        //     let unit_texture = unit
        //         .get_texture(&caveinfo.cave_cfg.game, self.mgr)
        //         .change_context(CaveripperError::RenderingError)?;
        //     let y = base_y + i as i64 * ((RENDER_SCALE * 8.0) as i64 + maptile_margin);

        //     if y + unit_texture.height() as i64 > canvas_maptiles.height() as i64 {
        //         let h = canvas_maptiles.height();
        //         expand_canvas(
        //             &mut canvas_maptiles,
        //             0,
        //             y as u32 + unit_texture.height() + (maptile_margin as u32) * 2 - h,
        //             Some([20, 20, 20, 255].into()),
        //         );
        //     }

        //     overlay(&mut canvas_maptiles, unit_texture.as_ref(), base_x, y);
        //     draw_border(
        //         &mut canvas_maptiles,
        //         base_x as u32,
        //         y as u32,
        //         base_x as u32 + (RENDER_SCALE * 8.0) as u32,
        //         y as u32 + (RENDER_SCALE * 8.0) as u32,
        //     );

        //     for spawnpoint in unit.spawnpoints.iter() {
        //         let sp_x =
        //             (spawnpoint.pos[0] * COORD_FACTOR) as i64 + (unit_texture.width() / 2) as i64;
        //         let sp_z =
        //             (spawnpoint.pos[2] * COORD_FACTOR) as i64 + (unit_texture.height() / 2) as i64;

        //         let sp_img = match spawnpoint.group {
        //             6 => colorize(
        //                 resize(
        //                     self.mgr
        //                         .get_img("resources/enemytex_special/leaf_icon.png")
        //                         .change_context(CaveripperError::RenderingError)?,
        //                     10,
        //                     10,
        //                     FilterType::Lanczos3,
        //                 ),
        //                 group_color(6).into(),
        //             ),
        //             9 => circle(5, group_color(9).into()),
        //             _ => circle(5, [255, 0, 0, 255].into()),
        //         };

        //         overlay(
        //             &mut canvas_maptiles,
        //             &sp_img,
        //             base_x + sp_x - (sp_img.width() / 2) as i64,
        //             y + sp_z - (sp_img.height() / 2) as i64,
        //         );
        //     }
        // }

        // if !caps.is_empty() {
        //     base_x += (RENDER_SCALE * 8.0) as i64 + maptile_margin;
        // }

        // for unit in rooms {
        //     let mut unit_texture = unit
        //         .get_texture(&caveinfo.cave_cfg.game, self.mgr)
        //         .change_context(CaveripperError::RenderingError)?
        //         .clone();

        //     // If the unit is just too big, we have to expand the whole image
        //     if unit_texture.width() + 2 > canvas_maptiles.width() {
        //         let expand_by = (unit_texture.width() + (maptile_margin as u32 * 2) + 2)
        //             - canvas_maptiles.width();
        //         expand_canvas(
        //             &mut canvas_maptiles,
        //             expand_by,
        //             0,
        //             Some(MAPTILES_BACKGROUND.into()),
        //         );
        //         expand_canvas(
        //             &mut canvas_header,
        //             expand_by,
        //             0,
        //             Some(HEADER_BACKGROUND.into()),
        //         );
        //     }
        //     // Normal case: we just overran in this row
        //     if base_x + unit_texture.width() as i64 + 2 > canvas_maptiles.width() as i64 {
        //         base_x = maptile_margin;
        //         base_y = max_y + maptile_margin;
        //     }
        //     // This next tile teeeechnically fits, so we just fudge it a little by expanding the width
        //     else if base_x + unit_texture.width() as i64 + maptile_margin + 2
        //         > canvas_maptiles.width() as i64
        //     {
        //         let expand_by = (base_x + maptile_margin) as u32 + unit_texture.width()
        //             - canvas_maptiles.width();
        //         expand_canvas(
        //             &mut canvas_maptiles,
        //             expand_by,
        //             0,
        //             Some(MAPTILES_BACKGROUND.into()),
        //         );
        //         expand_canvas(
        //             &mut canvas_header,
        //             expand_by,
        //             0,
        //             Some(HEADER_BACKGROUND.into()),
        //         );
        //     }

        //     let unit_name_text = self.render_text(
        //         &unit.unit_folder_name,
        //         14.0,
        //         [220, 220, 220, 255].into(),
        //         Some(unit_texture.width()),
        //     );

        //     if base_y + (unit_texture.height() + unit_name_text.height()) as i64
        //         > canvas_maptiles.height() as i64
        //     {
        //         let h = canvas_maptiles.height();
        //         expand_canvas(
        //             &mut canvas_maptiles,
        //             0,
        //             base_y as u32
        //                 + unit_texture.height()
        //                 + unit_name_text.height()
        //                 + (maptile_margin as u32)
        //                 - h,
        //             Some([20, 20, 20, 255].into()),
        //         );
        //     }

        //     if options.draw_waypoints {
        //         for waypoint in unit.waypoints.iter() {
        //             let wp_pos = waypoint.pos * COORD_FACTOR;
        //             let wp_img_radius = (waypoint.r * COORD_FACTOR).log2() * 3.0;

        //             let wp_img = circle(wp_img_radius as u32, WAYPOINT_COLOR.into());
        //             overlay(
        //                 unit_texture.to_mut(),
        //                 &wp_img,
        //                 wp_pos[0] as i64 - (wp_img.width() / 2) as i64,
        //                 wp_pos[2] as i64 - (wp_img.height() / 2) as i64,
        //             );

        //             for link in waypoint.links.iter() {
        //                 let dest_wp = unit.waypoints.iter().find(|wp| wp.index == *link).unwrap();
        //                 let dest_wp_pos = dest_wp.pos * COORD_FACTOR;
        //                 draw_arrow_line(
        //                     unit_texture.to_mut(),
        //                     wp_pos.into(),
        //                     dest_wp_pos.into(),
        //                     CARRY_PATH_COLOR.into(),
        //                 );

        //                 if options.draw_waypoint_distances {
        //                     let distance_text = self.render_small_text(
        //                         &format!("{}", waypoint.pos.p2_dist(&dest_wp.pos) as u32 / 10),
        //                         10.0,
        //                         WAYPOINT_DIST_TXT_COLOR.into(),
        //                     );
        //                     overlay(
        //                         unit_texture.to_mut(),
        //                         &distance_text,
        //                         (wp_pos[0] - (wp_pos[0] - dest_wp_pos[0]) / 2.0) as i64
        //                             - (distance_text.width() / 2) as i64,
        //                         (wp_pos[2] - (wp_pos[2] - dest_wp_pos[2]) / 2.0) as i64
        //                             - (distance_text.height() / 2) as i64,
        //                     )
        //                 }
        //             }
        //         }
        //     }

        //     for spawnpoint in unit.spawnpoints.iter().sorted_by_key(|sp| sp.group) {
        //         let sp_x =
        //             (spawnpoint.pos[0] * COORD_FACTOR) as i64 + (unit_texture.width() / 2) as i64;
        //         let sp_z =
        //             (spawnpoint.pos[2] * COORD_FACTOR) as i64 + (unit_texture.height() / 2) as i64;

        //         let sp_img = match spawnpoint.group {
        //             0 => circle(
        //                 (spawnpoint.radius * COORD_FACTOR) as u32,
        //                 group_color(0).into(),
        //             ),
        //             1 => circle(5, group_color(1).into()),
        //             2 => colorize(
        //                 resize(
        //                     self.mgr
        //                         .get_img("resources/enemytex_special/duck.png")
        //                         .change_context(CaveripperError::RenderingError)?,
        //                     14,
        //                     14,
        //                     FilterType::Lanczos3,
        //                 ),
        //                 group_color(2).into(),
        //             ), // treasure
        //             4 => resize(
        //                 self.mgr
        //                     .get_img("resources/enemytex_special/cave_white.png")
        //                     .change_context(CaveripperError::RenderingError)?,
        //                 18,
        //                 18,
        //                 FilterType::Lanczos3,
        //             ),
        //             6 => colorize(
        //                 resize(
        //                     self.mgr
        //                         .get_img("resources/enemytex_special/leaf_icon.png")
        //                         .change_context(CaveripperError::RenderingError)?,
        //                     10,
        //                     10,
        //                     FilterType::Lanczos3,
        //                 ),
        //                 group_color(6).into(),
        //             ),
        //             7 => resize(
        //                 self.mgr
        //                     .get_img("resources/enemytex_special/ship.png")
        //                     .change_context(CaveripperError::RenderingError)?,
        //                 16,
        //                 16,
        //                 FilterType::Lanczos3,
        //             ),
        //             8 => colorize(
        //                 resize(
        //                     self.mgr
        //                         .get_img("resources/enemytex_special/star.png")
        //                         .change_context(CaveripperError::RenderingError)?,
        //                     16,
        //                     16,
        //                     FilterType::Lanczos3,
        //                 ),
        //                 group_color(8).into(),
        //             ),
        //             _ => circle(5, [255, 0, 0, 255].into()),
        //         };

        //         overlay(
        //             unit_texture.to_mut(),
        //             &sp_img,
        //             sp_x - (sp_img.width() / 2) as i64,
        //             sp_z - (sp_img.height() / 2) as i64,
        //         );
        //     }

        //     overlay(&mut canvas_maptiles, unit_texture.as_ref(), base_x, base_y);
        //     draw_border(
        //         &mut canvas_maptiles,
        //         base_x as u32 - 1,
        //         base_y as u32 - 1,
        //         base_x as u32 + unit_texture.width(),
        //         base_y as u32 + unit_texture.height(),
        //     );
        //     overlay(
        //         &mut canvas_maptiles,
        //         &unit_name_text,
        //         base_x,
        //         base_y + unit_texture.height() as i64,
        //     );

        //     // Draw door indices
        //     for (i, door) in unit.doors.iter().enumerate() {
        //         let (x, y) = match door.direction {
        //             0 => (
        //                 door.side_lateral_offset as i64 * GRID_FACTOR as i64 + 28,
        //                 -5,
        //             ),
        //             1 => (
        //                 unit.width as i64 * GRID_FACTOR as i64 - 10,
        //                 door.side_lateral_offset as i64 * GRID_FACTOR as i64 + 20,
        //             ),
        //             2 => (
        //                 door.side_lateral_offset as i64 * GRID_FACTOR as i64 + 28,
        //                 unit.height as i64 * GRID_FACTOR as i64 - 20,
        //             ),
        //             3 => (0, door.side_lateral_offset as i64 * GRID_FACTOR as i64 + 20),
        //             _ => panic!("Invalid door direction"),
        //         };
        //         let door_index_text =
        //             self.render_small_text(&format!("{i}"), 15.0, [255, 0, 0, 255].into());
        //         overlay(
        //             &mut canvas_maptiles,
        //             &door_index_text,
        //             base_x + x,
        //             base_y + y,
        //         );
        //     }

        //     max_y = max(max_y, base_y + unit_texture.height() as i64);
        //     base_x += unit_texture.width() as i64 + maptile_margin;
        // }

        // // Combine sections
        // let header_height = canvas_header.height() as i64;
        // expand_canvas(&mut canvas_header, 0, canvas_maptiles.height(), None);
        // overlay(&mut canvas_header, &canvas_maptiles, 0, header_height);

        // Ok(canvas_header)
        //renderer.add_layer(caveinfo_layer);
        Ok(renderer.render(self.mgr))
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

    fn caveinfo_entity_box<'r>(
        &'a self,
        title: impl Into<String>,
        icon: impl Render + 'r,
        color: impl Into<Rgba<u8>>,
        spawn_objects: impl Iterator<Item = SpawnObject<'r>>,
        score: u32,
        game: &str,
    ) -> Layer<'r>
    where
        'a: 'r,
    {
        let color = color.into();

        let mut layer = Layer::new();
        layer.set_border(2.0, color);
        layer.set_margin(3.0);

        let icon: Box<dyn Render> = if icon.dimensions().length() > 0.0 {
            Box::new(Colorize {
                renderable: Resize {
                    renderable: icon,
                    width: 34.0,
                    height: 34.0,
                    filter: FilterType::Lanczos3,
                },
                color,
            })
        } else {
            Box::new(())
        };

        let mut header_row = Layer::new();
        let header_placement = header_row.place(icon, Point([0.0, 0.0]), Origin::TopLeft).place_relative(
            self.cropped_text(title, CAVEINFO_BOXES_FONT_SIZE, 0, OFF_BLACK),
            Origin::TopLeft,
            Offset {
                from: Origin::TopRight,
                amount: Point([CAVEINFO_MARGIN / 2.0, 0.0]),
            },
        );

        if score > 0 {
            header_placement.place_relative(
                self.cropped_text(format!("Score: {score}"), 20.0, 0, OFF_BLACK),
                Origin::CenterLeft,
                Offset {
                    from: Origin::CenterRight,
                    amount: Point([CAVEINFO_MARGIN, 0.0]),
                },
            );
        }

        let placement = layer
            .place(header_row, Point([0.0, 0.0]), Origin::TopLeft)
            .anchor_next(Origin::BottomLeft);

        spawn_objects.fold(placement, |p, so| {
            let mut so_and_value_layer = Layer::new();
            let so_and_value_layer_p =
                so_and_value_layer.place(render_spawn_object(Cow::Owned(so.clone())), Point([0.0, 0.0]), Origin::TopLeft);

            // Carrying info
            if let SpawnObject::Item(ItemInfo { internal_name, .. })
            | SpawnObject::Teki(
                TekiInfo {
                    carrying: Some(internal_name),
                    ..
                },
                _,
            ) = so
            {
                let treasure_metadata = self
                    .mgr
                    .treaure_info(game, &internal_name)
                    .expect("Invalid treasure encountered while rendering");

                let mut metadata_layer = Layer::new();
                metadata_layer
                    .place(
                        Resize {
                            renderable: Icon::Poko,
                            width: 24.0,
                            height: 24.0,
                            filter: FilterType::Lanczos3,
                        },
                        Point([0.0, 0.0]),
                        Origin::TopLeft,
                    )
                    .place_relative(
                        self.cropped_text(format!("{}", treasure_metadata.value), 28.0, 0, OFF_BLACK),
                        Origin::TopLeft,
                        Offset {
                            from: Origin::TopRight,
                            amount: Point([CAVEINFO_MARGIN / 4.0, 0.0]),
                        },
                    );
                so_and_value_layer_p
                    .place_relative(
                        metadata_layer,
                        Origin::TopLeft,
                        Offset {
                            from: Origin::TopRight,
                            amount: Point([CAVEINFO_MARGIN / 2.0, CAVEINFO_MARGIN / 2.0]),
                        },
                    )
                    .place_relative(
                        self.cropped_text(
                            format!("{}/{}", treasure_metadata.min_carry, treasure_metadata.max_carry),
                            28.0,
                            0,
                            OFF_BLACK,
                        ),
                        Origin::TopLeft,
                        Offset {
                            from: Origin::BottomLeft,
                            amount: Point([0.0, CAVEINFO_MARGIN / 2.0]),
                        },
                    );
            }

            // Number and Weight text

            let mut full_so_layer = Layer::new();
            let full_so_layer_p = full_so_layer.place(so_and_value_layer, Point([0.0, 0.0]), Origin::TopLeft);
            if let SpawnObject::Teki(_, _) | SpawnObject::CapTeki(_, _) = so {
                let mut num_str = String::new();
                let amount = so.amount();
                let weight = so.weight();

                if amount > 0 {
                    num_str += &format!("x{amount}");
                }
                if weight > 0 {
                    num_str += &format!("w{weight}");
                }
                full_so_layer_p.place_relative(
                    self.cropped_text(num_str, 20.0, 0, color),
                    Origin::TopCenter,
                    Offset {
                        from: Origin::BottomCenter,
                        amount: Point([0.0, CAVEINFO_MARGIN / 3.0]),
                    },
                );
            }

            p.place_relative(
                full_so_layer,
                Origin::TopLeft,
                Offset {
                    from: Origin::TopRight,
                    amount: Point([CAVEINFO_MARGIN / 2.0, 0.0]),
                },
            )
        });

        layer
    }
}

/// Places map unit images for a layout
fn render_map_units<'a, 'l: 'a>(map_units: impl Iterator<Item = &'a PlacedMapUnit<'l>>) -> (Layer<'a>, Layer<'a>) {
    let mut radar_image_layer = Layer::new();
    let mut waterbox_layer = Layer::new();

    for map_unit in map_units {
        let unit_def = map_unit.unit;
        let render_pos_x = map_unit.x as f32 * GRID_FACTOR;
        let render_pos_z = map_unit.z as f32 * GRID_FACTOR;

        // Radar images
        let unit_img_width = unit_def.width as f32 * GRID_FACTOR;
        let unit_img_height = unit_def.height as f32 * GRID_FACTOR;
        radar_image_layer.place(
            Resize::new(unit_def, unit_img_width, unit_img_height, FilterType::Nearest),
            Point([render_pos_x, render_pos_z]),
            Origin::TopLeft,
        );

        // Waterboxes
        for waterbox in unit_def.waterboxes.iter() {
            waterbox_layer.place(
                Rectangle {
                    width: waterbox.width() * COORD_FACTOR,
                    height: waterbox.height() * COORD_FACTOR,
                    color: WATERBOX_COLOR.into(),
                },
                Point([
                    render_pos_x + (unit_img_width / 2.0) + (waterbox.p1[0] * COORD_FACTOR),
                    render_pos_z + (unit_img_height / 2.0) + (waterbox.p1[2] * COORD_FACTOR),
                ]),
                Origin::TopLeft,
            );
        }
    }

    (radar_image_layer, waterbox_layer)
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
