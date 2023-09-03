mod sticker;

#[cfg(test)]
mod test;

use crate::{
    assets::{get_special_texture_name, AssetManager, Treasure},
    caveinfo::{CapInfo, CaveInfo, CaveUnit, GateInfo, ItemInfo, RoomType, TekiInfo},
    errors::CaveripperError,
    layout::{
        waypoint::{self, WaypointGraph},
        Layout, PlacedMapUnit, SpawnObject,
    },
    point::Point,
    render::{
        sticker::shapes::{Circle, Rectangle, Line},
    },
};
use clap::Args;
use error_stack::{Result, ResultExt};
use fontdue::{
    layout::{
        HorizontalAlign, Layout as FontLayout, LayoutSettings, TextStyle, VerticalAlign, WrapStyle,
    },
    Font, FontSettings,
};
use image::{
    imageops::{colorops::brighten_in_place, overlay, resize, rotate90, FilterType},
    Pixel, Rgba, RgbaImage,
};
use itertools::Itertools;
use log::info;
use std::{
    borrow::Cow,
    cmp::max,
    path::{Path, PathBuf},
};
use sticker::*;

use self::sticker::canvas::CanvasView;

/// Controls how scaled up the whole image is.
/// Only change this to increase or decrease the resolution;
/// all other parameters should depend on this.
const RENDER_SCALE: f32 = 16.0;

const GRID_FACTOR: f32 = 8.0 * RENDER_SCALE;
const COORD_FACTOR: f32 = (8.0 * RENDER_SCALE) / 170.0;
const TEKI_SIZE: f32 = 4.0 * RENDER_SCALE;
const GATE_SIZE: f32 = 8.0 * RENDER_SCALE;
const CARRIED_TREASURE_SIZE: f32 = TEKI_SIZE * 3.0 / 4.0;
const FALLING_CAP_TEKI_SIZE: u32 = 29;
const QUICKGLANCE_CIRCLE_RADIUS: f32 = 5.0 * RENDER_SCALE;
const QUICKGLANCE_TREASURE_COLOR: [u8; 4] = [230, 115, 0, 110];
const QUICKGLANCE_EXIT_COLOR: [u8; 4] = [2, 163, 69, 110];
const QUICKGLANCE_SHIP_COLOR: [u8; 4] = [255, 40, 40, 80];
const QUICKGLANCE_VIOLET_CANDYPOP_COLOR: [u8; 4] = [255, 0, 245, 80];
const QUICKGLANCE_IVORY_CANDYPOP_COLOR: [u8; 4] = [100, 100, 100, 120];
const QUICKGLANCE_ROAMING_COLOR: [u8; 4] = [200, 0, 130, 60];
const WAYPOINT_COLOR: [u8; 4] = [130, 199, 56, 150];
const WATERBOX_COLOR: [u8; 4] = [0, 100, 230, 50];
const CARRY_PATH_COLOR: [u8; 4] = [83, 125, 29, 200];
const WAYPOINT_DIST_TXT_COLOR: [u8; 4] = [36, 54, 14, 255];
const HEADER_BACKGROUND: [u8; 4] = [220, 220, 220, 255];
const MAPTILES_BACKGROUND: [u8; 4] = [20, 20, 20, 255];
const CAVEINFO_MARGIN: i64 = 4;
const CAVEINFO_ICON_SIZE: u32 = 48;

const fn group_color(group: u32) -> [u8; 4] {
    match group {
        0 => [250, 87, 207, 120],  // Easy Teki
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

    /// Draw the paths treasures will take to the ship.
    #[clap(long, short = 'p')]
    pub draw_paths: bool,

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

impl<'a> Renderer<'a> {
    pub fn new(mgr: &'a AssetManager) -> Self {
        let read_font = |path: &str| -> Font {
            let font_bytes = mgr.get_bytes(path).expect("Missing font file!");
            Font::from_bytes(font_bytes.as_slice(), FontSettings::default())
                .expect("Failed to create font!")
        };
        Self {
            mgr,
            fonts: vec![
                read_font("resources/BalooChettan2-SemiBold.ttf"),
                read_font("resources/BalooChettan2-ExtraBold.ttf"),
            ],
        }
    }

    pub fn render_layout(
        &self,
        layout: &Layout,
        options: LayoutRenderOptions,
    ) -> Result<RgbaImage, CaveripperError> {
        info!("Drawing layout image...");

        let mut renderer = StickerRenderer::new(Some([15, 15, 15, 255].into()));

        /* Map Units */
        let mut map_unit_layer = Layer::new();
        for map_unit in layout.map_units.iter() {
            render_map_unit(
                &mut renderer,
                &mut map_unit_layer,
                map_unit.unit,
                Point([
                    map_unit.x as f32 * GRID_FACTOR,
                    map_unit.z as f32 * GRID_FACTOR,
                ]),
            );
        }
        renderer.add_layer(map_unit_layer);

        /* Waypoints */
        if options.draw_waypoints {
            let mut waypoint_circle_layer = Layer::new();
            for wp in layout.waypoint_graph().iter() {
                let wp_sticker = renderer.add_sticker_with(format!("wp_{}", wp.r), || {
                    Sticker::new(
                        Renderable::Owned(Box::new(Circle {
                            radius: wp.r * COORD_FACTOR / 1.7,
                            color: WAYPOINT_COLOR.into(),
                        })),
                        Origin::Center,
                        Size::Native,
                    )
                });
                waypoint_circle_layer.add(
                    wp_sticker,
                    wp.pos[0] * COORD_FACTOR,
                    wp.pos[2] * COORD_FACTOR,
                );
            }
            renderer.add_layer(waypoint_circle_layer);

            let mut waypoint_arrow_layer = Layer::new();
            for wp in layout.waypoint_graph().iter() {
                if let Some(backlink) = layout.waypoint_graph().backlink(wp) {
                    let wp_arrow_sticker = renderer.add_sticker_with(format!("wp_arrow_{}_{}", wp.pos, backlink.pos), || {
                        Sticker::new(
                            Renderable::Owned(Box::new(Line {
                                start: (wp.pos * COORD_FACTOR).two_d(),
                                end: (backlink.pos * COORD_FACTOR).two_d(),
                                shorten_start: 6.0,
                                shorten_end: 6.0,
                                forward_arrow: true,
                                color: CARRY_PATH_COLOR.into()
                            })),
                            Origin::TopLeft,
                            Size::Native,
                        )
                    });
                    waypoint_arrow_layer.add(wp_arrow_sticker, wp.pos[0] * COORD_FACTOR, wp.pos[2] * COORD_FACTOR);
                }
            }
            renderer.add_layer(waypoint_arrow_layer);
        }

        /* Spawn Objects */
        let mut spawn_object_layer = Layer::new();
        let mut quickglance_circle_layer = Layer::new();
        for (spawn_object, pos) in layout.get_spawn_objects() {
            render_spawn_object(
                &mut renderer,
                &mut spawn_object_layer,
                spawn_object,
                pos.two_d(),
            );

            // Quickglance Circles
            if options.quickglance {
                let color = match spawn_object {
                    SpawnObject::Teki(
                        TekiInfo {
                            carrying: Some(_), ..
                        },
                        _,
                    )
                    | SpawnObject::Item(_) => Some(QUICKGLANCE_TREASURE_COLOR),
                    SpawnObject::Teki(TekiInfo { internal_name, .. }, _)
                    | SpawnObject::CapTeki(CapInfo { internal_name, .. }, _) => {
                        match internal_name.to_ascii_lowercase().as_str() {
                            "whitepom" => Some(QUICKGLANCE_IVORY_CANDYPOP_COLOR),
                            "blackpom" => Some(QUICKGLANCE_VIOLET_CANDYPOP_COLOR),
                            "minihoudai" | "kumochappy" => Some(QUICKGLANCE_ROAMING_COLOR),
                            _ => None,
                        }
                    }
                    SpawnObject::Hole(_) | SpawnObject::Geyser(_) => Some(QUICKGLANCE_EXIT_COLOR),
                    SpawnObject::Ship => Some(QUICKGLANCE_SHIP_COLOR),
                    _ => None,
                };
                if let Some(color) = color {
                    let qg_sticker = renderer.add_sticker_with(
                        format!(
                            "qg_{:x},{:x},{:x},{:x}",
                            color[0], color[1], color[2], color[3]
                        ),
                        || {
                            Sticker::new(
                                Renderable::Owned(Box::new(Circle {
                                    radius: QUICKGLANCE_CIRCLE_RADIUS,
                                    color: color.into(),
                                })),
                                Origin::Center,
                                Size::Native,
                            )
                        },
                    );
                    quickglance_circle_layer.add(
                        qg_sticker,
                        pos[0] * COORD_FACTOR,
                        pos[2] * COORD_FACTOR,
                    );
                }
            }
        }
        renderer.add_layer(quickglance_circle_layer);
        renderer.add_layer(spawn_object_layer);

        // TODO: grid

        // // Draw a map unit grid, if enabled
        // if options.draw_grid {
        //     let grid_color: Rgba<u8> = [255, 0, 0, 150].into();
        //     let grid_size = GRID_FACTOR as u32;
        //     for x in 0..canvas.width() {
        //         for z in 0..canvas.height() {
        //             if x % grid_size == 0 || z % grid_size == 0 {
        //                 let new_pix = canvas.get_pixel_mut(x, z);
        //                 new_pix.blend(&grid_color);
        //             }
        //         }
        //     }
        // }

        // // Draw carry paths for treasures, if enabled
        // if options.draw_paths {
        //     let treasure_locations = layout.get_spawn_objects()
        //         .filter(|(so, _pos)| {
        //             matches!(so, SpawnObject::Item(_))
        //             || matches!(so, SpawnObject::Teki(TekiInfo { carrying: Some(_), .. }, _))
        //             || matches!(so, SpawnObject::Hole(_))
        //         })
        //         .map(|(_, pos)| pos);
        //     for pos in treasure_locations {
        //         let wp_path = layout.waypoint_graph().carry_path_wps(pos);
        //         for (wp1, wp2) in wp_path.tuple_windows() {
        //             draw_arrow_line(
        //                 &mut canvas,
        //                 (wp1 * COORD_FACTOR).into(),
        //                 (wp2 * COORD_FACTOR).into(),
        //                 WAYPOINT_DIST_TXT_COLOR.into()
        //             );
        //         }
        //     }
        // }

        // if options.draw_score {
        //     for unit in layout.map_units.iter() {
        //         let score_text = self.render_small_text(&format!("{}", unit.total_score), 16.0, [170, 50, 30, 255].into());
        //         let img_x = ((unit.x as f32 + unit.unit.width as f32 / 2.0) * GRID_FACTOR as f32) as i64;
        //         let img_z = ((unit.z as f32 + unit.unit.height as f32 / 2.0) * GRID_FACTOR as f32) as i64;
        //         overlay(&mut canvas, &score_text, img_x - (score_text.width() / 2) as i64, img_z - (score_text.height() / 2) as i64);
        //     }
        // }

        // Ok(canvas)

        Ok(renderer.render(self.mgr))
    }

    pub fn render_caveinfo(
        &self,
        caveinfo: &CaveInfo,
        options: CaveinfoRenderOptions,
    ) -> Result<RgbaImage, CaveripperError> {
        let mut canvas_header = RgbaImage::from_pixel(1060, 310, HEADER_BACKGROUND.into());

        // Sublevel name
        let sublevel_title =
            self.render_text(&caveinfo.long_name(), 64.0, [0, 0, 0, 255].into(), None);
        overlay(&mut canvas_header, &sublevel_title, CAVEINFO_MARGIN * 2, -8);

        // Metadata icons - ship, hole plugged/unplugged, geyser yes/no, num gates
        let mut metadata_icons = Vec::new();
        metadata_icons.push(resize(
            SpawnObject::Ship
                .get_texture(&caveinfo.cave_cfg.game, self.mgr)
                .change_context(CaveripperError::RenderingError)?
                .as_ref(),
            CAVEINFO_ICON_SIZE,
            CAVEINFO_ICON_SIZE,
            FilterType::Lanczos3,
        ));
        if !caveinfo.is_final_floor {
            metadata_icons.push(resize(
                SpawnObject::Hole(caveinfo.exit_plugged)
                    .get_texture(&caveinfo.cave_cfg.game, self.mgr)
                    .change_context(CaveripperError::RenderingError)?
                    .as_ref(),
                CAVEINFO_ICON_SIZE,
                CAVEINFO_ICON_SIZE,
                FilterType::Lanczos3,
            ));
        }
        if caveinfo.is_final_floor || caveinfo.has_geyser {
            metadata_icons.push(resize(
                SpawnObject::Geyser(caveinfo.is_challenge_mode() && caveinfo.is_final_floor)
                    .get_texture(&caveinfo.cave_cfg.game, self.mgr)
                    .change_context(CaveripperError::RenderingError)?
                    .as_ref(),
                CAVEINFO_ICON_SIZE,
                CAVEINFO_ICON_SIZE,
                FilterType::Lanczos3,
            ));
        }
        let num_gates = caveinfo.max_gates;
        for gateinfo in caveinfo.gate_info.iter() {
            let gate_icon = resize(
                gateinfo
                    .get_texture(&caveinfo.cave_cfg.game, self.mgr)
                    .change_context(CaveripperError::RenderingError)?
                    .as_ref(),
                CAVEINFO_ICON_SIZE,
                CAVEINFO_ICON_SIZE,
                FilterType::Lanczos3,
            );
            let num_txt =
                self.render_small_text(&format!("x{num_gates}"), 19.0, [20, 20, 20, 255].into());
            let hp_txt = self.render_small_text(
                &format!("{}HP", gateinfo.health as u32),
                13.0,
                [20, 20, 20, 255].into(),
            );
            let mut final_gate_icon = RgbaImage::new(CAVEINFO_ICON_SIZE, CAVEINFO_ICON_SIZE);
            overlay(&mut final_gate_icon, &gate_icon, 0, -12);
            overlay(
                &mut final_gate_icon,
                &hp_txt,
                CAVEINFO_ICON_SIZE as i64 / 2 - hp_txt.width() as i64 / 2,
                CAVEINFO_ICON_SIZE as i64 - 33,
            );
            overlay(
                &mut final_gate_icon,
                &num_txt,
                CAVEINFO_ICON_SIZE as i64 / 2 - num_txt.width() as i64 / 2,
                CAVEINFO_ICON_SIZE as i64 - 24,
            );
            metadata_icons.push(final_gate_icon);
        }

        for (i, icon) in metadata_icons.into_iter().enumerate() {
            overlay(
                &mut canvas_header,
                &icon,
                35 + sublevel_title.width() as i64
                    + i as i64 * (CAVEINFO_ICON_SIZE as i64 + CAVEINFO_MARGIN * 3),
                CAVEINFO_MARGIN + 12,
            );
        }

        let poko_icon = resize(
            self.mgr
                .get_img("resources/enemytex_special/Poko_icon.png")
                .change_context(CaveripperError::RenderingError)?,
            16,
            19,
            FilterType::Lanczos3,
        );

        // Teki section
        let mut base_y = 64 + CAVEINFO_MARGIN * 2;
        let teki_header = self.render_text(
            &format!("Teki (max {})", caveinfo.max_main_objects),
            48.0,
            [225, 0, 0, 255].into(),
            None,
        );
        overlay(
            &mut canvas_header,
            &teki_header,
            CAVEINFO_MARGIN * 2,
            base_y,
        );
        let mut base_x = (CAVEINFO_MARGIN * 4) + teki_header.width() as i64;
        base_y += (64 - CAVEINFO_ICON_SIZE as i64) / 2;

        for group in [8, 1, 0, 6, 5] {
            for tekiinfo in caveinfo.teki_group(group) {
                let texture = resize(
                    tekiinfo
                        .get_texture(&caveinfo.cave_cfg.game, self.mgr)
                        .change_context(CaveripperError::RenderingError)?
                        .as_ref(),
                    CAVEINFO_ICON_SIZE,
                    CAVEINFO_ICON_SIZE,
                    FilterType::Lanczos3,
                );

                // If we overflow the width of the image, wrap to the next line.
                if base_x + CAVEINFO_ICON_SIZE as i64 + CAVEINFO_MARGIN
                    > canvas_header.width() as i64
                {
                    base_x = (CAVEINFO_MARGIN * 4) + teki_header.width() as i64;
                    base_y += 70;

                    // Expand the header to make room for the other rows
                    expand_canvas(
                        &mut canvas_header,
                        0,
                        70 + CAVEINFO_MARGIN as u32,
                        Some([220, 220, 220, 255].into()),
                    );
                }

                overlay(&mut canvas_header, &texture, base_x, base_y);

                let mut extra_width = 0;
                for modifier in tekiinfo.get_texture_modifiers().iter() {
                    match modifier {
                        TextureModifier::Falling => {
                            let falling_icon_texture = resize(
                                self.mgr
                                    .get_img("resources/enemytex_special/falling_icon.png")
                                    .change_context(CaveripperError::RenderingError)?,
                                24,
                                24,
                                FilterType::Nearest,
                            );
                            overlay(
                                &mut canvas_header,
                                &falling_icon_texture,
                                base_x - 8,
                                base_y - 2,
                            );
                        }
                        TextureModifier::Carrying(carrying) => {
                            let treasure = self
                                .mgr
                                .treasure_list(&caveinfo.cave_cfg.game)
                                .change_context(CaveripperError::RenderingError)?
                                .iter()
                                .find(|t| t.internal_name.eq_ignore_ascii_case(carrying))
                                .unwrap_or_else(|| panic!("Teki carrying unknown or invalid treasure \"{carrying}\""));

                            let carried_treasure_icon = resize(
                                self.mgr
                                    .get_img(&PathBuf::from_iter([
                                        "assets",
                                        &caveinfo.cave_cfg.game,
                                        "treasures",
                                        &format!("{carrying}.png"),
                                    ]))
                                    .change_context(CaveripperError::RenderingError)?,
                                CAVEINFO_ICON_SIZE - 10,
                                CAVEINFO_ICON_SIZE - 10,
                                FilterType::Lanczos3,
                            );
                            overlay(
                                &mut canvas_header,
                                &carried_treasure_icon,
                                base_x + 18,
                                base_y + 14,
                            );

                            // Treasure value/carry text
                            if options.draw_treasure_info {
                                let value_text = self.render_text(
                                    &format!("{}", treasure.value),
                                    20.0,
                                    [20, 20, 20, 255].into(),
                                    None,
                                );
                                let carriers_text = self.render_text(
                                    &format!("{}/{}", treasure.min_carry, treasure.max_carry),
                                    20.0,
                                    [20, 20, 20, 255].into(),
                                    None,
                                );

                                let sidetext_x = base_x + texture.width() as i64 + 5;
                                let text_width = max(
                                    poko_icon.width() as i64 + value_text.width() as i64,
                                    carriers_text.width() as i64,
                                ) + CAVEINFO_MARGIN * 2;
                                if sidetext_x + text_width > canvas_header.width() as i64 {
                                    let header_width = canvas_header.width() as i64;
                                    expand_canvas(
                                        &mut canvas_header,
                                        (sidetext_x + text_width - header_width) as u32,
                                        0,
                                        Some([220, 220, 220, 255].into()),
                                    );
                                }

                                overlay(&mut canvas_header, &poko_icon, sidetext_x, base_y + 4);
                                overlay(
                                    &mut canvas_header,
                                    &value_text,
                                    sidetext_x + poko_icon.width() as i64 + 3,
                                    base_y - value_text.height() as i64 / 2
                                        + poko_icon.height() as i64 / 2
                                        + 4,
                                );

                                overlay(
                                    &mut canvas_header,
                                    &carriers_text,
                                    sidetext_x,
                                    base_y + poko_icon.height() as i64 + 2,
                                );

                                base_x += text_width;
                                extra_width += text_width;
                            }
                        }
                        _ => {}
                    }
                }

                let teki_subtext = if tekiinfo.filler_distribution_weight > 0
                    && tekiinfo.minimum_amount > 0
                {
                    format!(
                        "x{} w{}",
                        tekiinfo.minimum_amount, tekiinfo.filler_distribution_weight
                    )
                } else if tekiinfo.minimum_amount == 0 && tekiinfo.filler_distribution_weight > 0 {
                    format!("w{}", tekiinfo.filler_distribution_weight)
                } else {
                    format!("x{}", tekiinfo.minimum_amount)
                };

                let subtext_color = group_color(tekiinfo.group).into();

                let teki_subtext_texture =
                    self.render_text(&teki_subtext, 24.0, subtext_color, None);
                overlay(
                    &mut canvas_header,
                    &teki_subtext_texture,
                    base_x + CAVEINFO_ICON_SIZE as i64 / 2
                        - teki_subtext_texture.width() as i64 / 2
                        - extra_width / 2,
                    base_y + CAVEINFO_ICON_SIZE as i64 - 8,
                );

                base_x += CAVEINFO_ICON_SIZE as i64 + CAVEINFO_MARGIN;
            }
        }

        base_y += teki_header.height() as i64 + CAVEINFO_MARGIN;

        // Treasures section
        let treasure_header = self.render_text("Treasures", 48.0, [207, 105, 33, 255].into(), None);
        overlay(
            &mut canvas_header,
            &treasure_header,
            CAVEINFO_MARGIN * 2,
            base_y,
        );

        let mut base_x = treasure_header.width() as i64 + CAVEINFO_MARGIN;
        for treasureinfo in caveinfo.item_info.iter() {
            let treasure = self
                .mgr
                .treasure_list(&caveinfo.cave_cfg.game)
                .change_context(CaveripperError::RenderingError)?
                .iter()
                .find(|t| {
                    t.internal_name
                        .eq_ignore_ascii_case(&treasureinfo.internal_name)
                })
                .expect("Unknown or invalid treasure!");

            let treasure_texture = resize(
                treasureinfo
                    .get_texture(&caveinfo.cave_cfg.game, self.mgr)
                    .change_context(CaveripperError::RenderingError)?
                    .as_ref(),
                CAVEINFO_ICON_SIZE,
                CAVEINFO_ICON_SIZE,
                FilterType::Lanczos3,
            );
            let x = base_x + CAVEINFO_MARGIN * 4;
            let y = base_y + CAVEINFO_MARGIN + (64 - CAVEINFO_ICON_SIZE as i64) / 2;
            overlay(&mut canvas_header, &treasure_texture, x, y);

            let mut extra_width = 0;
            if options.draw_treasure_info {
                let value_text = self.render_text(
                    &format!("{}", treasure.value),
                    20.0,
                    [20, 20, 20, 255].into(),
                    None,
                );
                let sidetext_x = x + treasure_texture.width() as i64 + 2;
                overlay(&mut canvas_header, &poko_icon, sidetext_x, y + 4);
                overlay(
                    &mut canvas_header,
                    &value_text,
                    sidetext_x + poko_icon.width() as i64 + 3,
                    y - value_text.height() as i64 / 2 + poko_icon.height() as i64 / 2 + 4,
                );

                let carriers_text = self.render_text(
                    &format!("{}/{}", treasure.min_carry, treasure.max_carry),
                    20.0,
                    [20, 20, 20, 255].into(),
                    None,
                );
                overlay(
                    &mut canvas_header,
                    &carriers_text,
                    sidetext_x,
                    y + poko_icon.height() as i64 + 2,
                );

                extra_width += max(
                    poko_icon.width() as i64 + value_text.width() as i64,
                    carriers_text.width() as i64,
                ) + 4;
            }

            if caveinfo.is_challenge_mode() {
                let subtext_color = group_color(2).into();
                let treasure_subtext = format!("x{}", treasureinfo.min_amount);
                let treasure_subtext_texture =
                    self.render_text(&treasure_subtext, 24.0, subtext_color, None);
                overlay(
                    &mut canvas_header,
                    &treasure_subtext_texture,
                    x + (CAVEINFO_ICON_SIZE as i64 / 2)
                        - (treasure_subtext_texture.width() as i64 / 2)
                        + (extra_width / 2),
                    y + CAVEINFO_ICON_SIZE as i64 - 12,
                );
            }

            base_x += treasure_texture.width() as i64 + extra_width;
        }

        base_y += treasure_header.height() as i64;

        // Make room for treasure number text
        if caveinfo.is_challenge_mode() {
            base_y += CAVEINFO_MARGIN;
        }

        // Capteki section
        let capteki_color = group_color(9).into();
        let capteki_header = self.render_text("Cap Teki", 48.0, capteki_color, None);
        overlay(
            &mut canvas_header,
            &capteki_header,
            CAVEINFO_MARGIN * 2,
            base_y,
        );
        for (i, capinfo) in caveinfo.cap_info.iter().enumerate() {
            let texture = resize(
                capinfo
                    .get_texture(&caveinfo.cave_cfg.game, self.mgr)
                    .change_context(CaveripperError::RenderingError)?
                    .as_ref(),
                CAVEINFO_ICON_SIZE,
                CAVEINFO_ICON_SIZE,
                FilterType::Lanczos3,
            );
            let x = (CAVEINFO_MARGIN * 5)
                + capteki_header.width() as i64
                + i as i64 * (CAVEINFO_ICON_SIZE as i64 + CAVEINFO_MARGIN * 2);
            let y = base_y + (64 - CAVEINFO_ICON_SIZE as i64) / 2;
            overlay(&mut canvas_header, &texture, x, y);

            for modifier in capinfo.get_texture_modifiers().iter() {
                if let TextureModifier::Falling = modifier {
                    let falling_icon_texture = resize(
                        self.mgr
                            .get_img("resources/enemytex_special/falling_icon.png")
                            .change_context(CaveripperError::RenderingError)?,
                        24,
                        24,
                        FilterType::Nearest,
                    );
                    overlay(&mut canvas_header, &falling_icon_texture, x - 8, y - 2);
                }
            }

            let capteki_subtext =
                if capinfo.filler_distribution_weight > 0 && capinfo.minimum_amount > 0 {
                    format!(
                        "x{} w{}",
                        capinfo.minimum_amount, capinfo.filler_distribution_weight
                    )
                } else if capinfo.minimum_amount == 0 && capinfo.filler_distribution_weight > 0 {
                    format!("w{}", capinfo.filler_distribution_weight)
                } else {
                    format!("x{}", capinfo.minimum_amount)
                };

            let capteki_subtext_texture =
                self.render_text(&capteki_subtext, 24.0, capteki_color, None);
            overlay(
                &mut canvas_header,
                &capteki_subtext_texture,
                x + CAVEINFO_ICON_SIZE as i64 / 2 - capteki_subtext_texture.width() as i64 / 2,
                y + CAVEINFO_ICON_SIZE as i64 - 10,
            );
        }

        // Done with header section
        // Start Map Tile section

        let mut canvas_maptiles =
            RgbaImage::from_pixel(canvas_header.width(), 200, MAPTILES_BACKGROUND.into());

        let maptiles_metadata_txt = self.render_text(
            &format!(
                "Num Rooms: {}     CorridorBetweenRoomsProb: {}%     CapOpenDoorsProb: {}%",
                caveinfo.num_rooms,
                caveinfo.corridor_probability * 100.0,
                caveinfo.cap_probability * 100.0
            ),
            24.0,
            [220, 220, 220, 255].into(),
            Some(canvas_maptiles.width()),
        );
        overlay(
            &mut canvas_maptiles,
            &maptiles_metadata_txt,
            canvas_header.width() as i64 / 2 - maptiles_metadata_txt.width() as i64 / 2,
            0,
        );

        let maptile_margin = (RENDER_SCALE * 4.0) as i64;
        let mut base_x = maptile_margin;
        let mut base_y = maptiles_metadata_txt.height() as i64 + maptile_margin;
        let mut max_y = base_y;

        let rooms = caveinfo
            .cave_units
            .iter()
            .filter(|unit| unit.rotation == 0)
            .filter(|unit| unit.room_type == RoomType::Room);

        let caps: Vec<_> = caveinfo
            .cave_units
            .iter()
            .filter(|unit| unit.rotation == 0)
            .filter(|unit| unit.room_type == RoomType::DeadEnd)
            .collect();

        for (i, unit) in caps.iter().enumerate() {
            let unit_texture = unit
                .get_texture(&caveinfo.cave_cfg.game, self.mgr)
                .change_context(CaveripperError::RenderingError)?;
            let y = base_y + i as i64 * ((RENDER_SCALE * 8.0) as i64 + maptile_margin);

            if y + unit_texture.height() as i64 > canvas_maptiles.height() as i64 {
                let h = canvas_maptiles.height();
                expand_canvas(
                    &mut canvas_maptiles,
                    0,
                    y as u32 + unit_texture.height() + (maptile_margin as u32) * 2 - h,
                    Some([20, 20, 20, 255].into()),
                );
            }

            overlay(&mut canvas_maptiles, unit_texture.as_ref(), base_x, y);
            draw_border(
                &mut canvas_maptiles,
                base_x as u32,
                y as u32,
                base_x as u32 + (RENDER_SCALE * 8.0) as u32,
                y as u32 + (RENDER_SCALE * 8.0) as u32,
            );

            for spawnpoint in unit.spawnpoints.iter() {
                let sp_x =
                    (spawnpoint.pos[0] * COORD_FACTOR) as i64 + (unit_texture.width() / 2) as i64;
                let sp_z =
                    (spawnpoint.pos[2] * COORD_FACTOR) as i64 + (unit_texture.height() / 2) as i64;

                let sp_img = match spawnpoint.group {
                    6 => colorize(
                        resize(
                            self.mgr
                                .get_img("resources/enemytex_special/leaf_icon.png")
                                .change_context(CaveripperError::RenderingError)?,
                            10,
                            10,
                            FilterType::Lanczos3,
                        ),
                        group_color(6).into(),
                    ),
                    9 => circle(5, group_color(9).into()),
                    _ => circle(5, [255, 0, 0, 255].into()),
                };

                overlay(
                    &mut canvas_maptiles,
                    &sp_img,
                    base_x + sp_x - (sp_img.width() / 2) as i64,
                    y + sp_z - (sp_img.height() / 2) as i64,
                );
            }
        }

        if !caps.is_empty() {
            base_x += (RENDER_SCALE * 8.0) as i64 + maptile_margin;
        }

        for unit in rooms {
            let mut unit_texture = unit
                .get_texture(&caveinfo.cave_cfg.game, self.mgr)
                .change_context(CaveripperError::RenderingError)?
                .clone();

            // If the unit is just too big, we have to expand the whole image
            if unit_texture.width() + 2 > canvas_maptiles.width() {
                let expand_by = (unit_texture.width() + (maptile_margin as u32 * 2) + 2)
                    - canvas_maptiles.width();
                expand_canvas(
                    &mut canvas_maptiles,
                    expand_by,
                    0,
                    Some(MAPTILES_BACKGROUND.into()),
                );
                expand_canvas(
                    &mut canvas_header,
                    expand_by,
                    0,
                    Some(HEADER_BACKGROUND.into()),
                );
            }
            // Normal case: we just overran in this row
            if base_x + unit_texture.width() as i64 + 2 > canvas_maptiles.width() as i64 {
                base_x = maptile_margin;
                base_y = max_y + maptile_margin;
            }
            // This next tile teeeechnically fits, so we just fudge it a little by expanding the width
            else if base_x + unit_texture.width() as i64 + maptile_margin + 2
                > canvas_maptiles.width() as i64
            {
                let expand_by = (base_x + maptile_margin) as u32 + unit_texture.width()
                    - canvas_maptiles.width();
                expand_canvas(
                    &mut canvas_maptiles,
                    expand_by,
                    0,
                    Some(MAPTILES_BACKGROUND.into()),
                );
                expand_canvas(
                    &mut canvas_header,
                    expand_by,
                    0,
                    Some(HEADER_BACKGROUND.into()),
                );
            }

            let unit_name_text = self.render_text(
                &unit.unit_folder_name,
                14.0,
                [220, 220, 220, 255].into(),
                Some(unit_texture.width()),
            );

            if base_y + (unit_texture.height() + unit_name_text.height()) as i64
                > canvas_maptiles.height() as i64
            {
                let h = canvas_maptiles.height();
                expand_canvas(
                    &mut canvas_maptiles,
                    0,
                    base_y as u32
                        + unit_texture.height()
                        + unit_name_text.height()
                        + (maptile_margin as u32)
                        - h,
                    Some([20, 20, 20, 255].into()),
                );
            }

            if options.draw_waypoints {
                for waypoint in unit.waypoints.iter() {
                    let wp_pos = waypoint.pos * COORD_FACTOR;
                    let wp_img_radius = (waypoint.r * COORD_FACTOR).log2() * 3.0;

                    let wp_img = circle(wp_img_radius as u32, WAYPOINT_COLOR.into());
                    overlay(
                        unit_texture.to_mut(),
                        &wp_img,
                        wp_pos[0] as i64 - (wp_img.width() / 2) as i64,
                        wp_pos[2] as i64 - (wp_img.height() / 2) as i64,
                    );

                    for link in waypoint.links.iter() {
                        let dest_wp = unit.waypoints.iter().find(|wp| wp.index == *link).unwrap();
                        let dest_wp_pos = dest_wp.pos * COORD_FACTOR;
                        draw_arrow_line(
                            unit_texture.to_mut(),
                            wp_pos.into(),
                            dest_wp_pos.into(),
                            CARRY_PATH_COLOR.into(),
                        );

                        if options.draw_waypoint_distances {
                            let distance_text = self.render_small_text(
                                &format!("{}", waypoint.pos.p2_dist(&dest_wp.pos) as u32 / 10),
                                10.0,
                                WAYPOINT_DIST_TXT_COLOR.into(),
                            );
                            overlay(
                                unit_texture.to_mut(),
                                &distance_text,
                                (wp_pos[0] - (wp_pos[0] - dest_wp_pos[0]) / 2.0) as i64
                                    - (distance_text.width() / 2) as i64,
                                (wp_pos[2] - (wp_pos[2] - dest_wp_pos[2]) / 2.0) as i64
                                    - (distance_text.height() / 2) as i64,
                            )
                        }
                    }
                }
            }

            for spawnpoint in unit.spawnpoints.iter().sorted_by_key(|sp| sp.group) {
                let sp_x =
                    (spawnpoint.pos[0] * COORD_FACTOR) as i64 + (unit_texture.width() / 2) as i64;
                let sp_z =
                    (spawnpoint.pos[2] * COORD_FACTOR) as i64 + (unit_texture.height() / 2) as i64;

                let sp_img = match spawnpoint.group {
                    0 => circle(
                        (spawnpoint.radius * COORD_FACTOR) as u32,
                        group_color(0).into(),
                    ),
                    1 => circle(5, group_color(1).into()),
                    2 => colorize(
                        resize(
                            self.mgr
                                .get_img("resources/enemytex_special/duck.png")
                                .change_context(CaveripperError::RenderingError)?,
                            14,
                            14,
                            FilterType::Lanczos3,
                        ),
                        group_color(2).into(),
                    ), // treasure
                    4 => resize(
                        self.mgr
                            .get_img("resources/enemytex_special/cave_white.png")
                            .change_context(CaveripperError::RenderingError)?,
                        18,
                        18,
                        FilterType::Lanczos3,
                    ),
                    6 => colorize(
                        resize(
                            self.mgr
                                .get_img("resources/enemytex_special/leaf_icon.png")
                                .change_context(CaveripperError::RenderingError)?,
                            10,
                            10,
                            FilterType::Lanczos3,
                        ),
                        group_color(6).into(),
                    ),
                    7 => resize(
                        self.mgr
                            .get_img("resources/enemytex_special/ship.png")
                            .change_context(CaveripperError::RenderingError)?,
                        16,
                        16,
                        FilterType::Lanczos3,
                    ),
                    8 => colorize(
                        resize(
                            self.mgr
                                .get_img("resources/enemytex_special/star.png")
                                .change_context(CaveripperError::RenderingError)?,
                            16,
                            16,
                            FilterType::Lanczos3,
                        ),
                        group_color(8).into(),
                    ),
                    _ => circle(5, [255, 0, 0, 255].into()),
                };

                overlay(
                    unit_texture.to_mut(),
                    &sp_img,
                    sp_x - (sp_img.width() / 2) as i64,
                    sp_z - (sp_img.height() / 2) as i64,
                );
            }

            overlay(&mut canvas_maptiles, unit_texture.as_ref(), base_x, base_y);
            draw_border(
                &mut canvas_maptiles,
                base_x as u32 - 1,
                base_y as u32 - 1,
                base_x as u32 + unit_texture.width(),
                base_y as u32 + unit_texture.height(),
            );
            overlay(
                &mut canvas_maptiles,
                &unit_name_text,
                base_x,
                base_y + unit_texture.height() as i64,
            );

            // Draw door indices
            for (i, door) in unit.doors.iter().enumerate() {
                let (x, y) = match door.direction {
                    0 => (
                        door.side_lateral_offset as i64 * GRID_FACTOR as i64 + 28,
                        -5,
                    ),
                    1 => (
                        unit.width as i64 * GRID_FACTOR as i64 - 10,
                        door.side_lateral_offset as i64 * GRID_FACTOR as i64 + 20,
                    ),
                    2 => (
                        door.side_lateral_offset as i64 * GRID_FACTOR as i64 + 28,
                        unit.height as i64 * GRID_FACTOR as i64 - 20,
                    ),
                    3 => (0, door.side_lateral_offset as i64 * GRID_FACTOR as i64 + 20),
                    _ => panic!("Invalid door direction"),
                };
                let door_index_text =
                    self.render_small_text(&format!("{i}"), 15.0, [255, 0, 0, 255].into());
                overlay(
                    &mut canvas_maptiles,
                    &door_index_text,
                    base_x + x,
                    base_y + y,
                );
            }

            max_y = max(max_y, base_y + unit_texture.height() as i64);
            base_x += unit_texture.width() as i64 + maptile_margin;
        }

        // Combine sections
        let header_height = canvas_header.height() as i64;
        expand_canvas(&mut canvas_header, 0, canvas_maptiles.height(), None);
        overlay(&mut canvas_header, &canvas_maptiles, 0, header_height);

        Ok(canvas_header)
    }

    // x and z are world coordinates, not image or map unit coordinates
    fn draw_object_at<Tex: Textured>(
        &self,
        image_buffer: &mut RgbaImage,
        obj: &Tex,
        pos: Point<2, f32>,
        game: &str,
        options: &LayoutRenderOptions,
    ) -> Result<(), CaveripperError> {
        let mut texture = obj.get_texture(game, self.mgr)?;

        // Modifiers to be applied before ('under') the main texture, or to the texture itself
        for modifier in obj.get_texture_modifiers().iter() {
            match modifier {
                TextureModifier::QuickGlance(color) if options.quickglance => {
                    let circle_size = QUICKGLANCE_CIRCLE_RADIUS;
                    let circle_tex = circle(circle_size as u32, *color);
                    overlay(
                        image_buffer,
                        &circle_tex,
                        ((pos[0] * COORD_FACTOR) - circle_size) as i64,
                        ((pos[1] * COORD_FACTOR) - circle_size) as i64,
                    );
                }
                TextureModifier::Scale(xsize, zsize) => {
                    *texture.to_mut() = resize(&*texture, *xsize, *zsize, FilterType::Lanczos3);
                }
                _ => {}
            }
        }

        let img_x = ((pos[0] * COORD_FACTOR) - (texture.width() as f32 / 2.0)) as i64;
        let img_z = ((pos[1] * COORD_FACTOR) - (texture.height() as f32 / 2.0)) as i64;

        // Draw the main texture
        overlay(image_buffer, &*texture, img_x, img_z);

        // Modifiers to be applied after ('above') the main texture
        for modifier in obj.get_texture_modifiers().iter() {
            match modifier {
                TextureModifier::Falling => {
                    let falling_icon_texture = resize(
                        self.mgr
                            .get_img("resources/enemytex_special/falling_icon.png")?,
                        18,
                        18,
                        FilterType::Lanczos3,
                    );
                    overlay(image_buffer, &falling_icon_texture, img_x - 5, img_z);
                }
                TextureModifier::Carrying(carrying) => {
                    let carried_treasure_icon = resize(
                        self.mgr.get_img(&PathBuf::from_iter([
                            "assets",
                            game,
                            "treasures",
                            &format!("{carrying}.png"),
                        ]))?,
                        24,
                        24,
                        FilterType::Lanczos3,
                    );
                    overlay(image_buffer, &carried_treasure_icon, img_x + 15, img_z + 15);
                }
                TextureModifier::GaugeRing if options.draw_gauge_range => {
                    let radius1 = 775.0 * COORD_FACTOR; // Radius at which the gauge needle starts to go up
                    let radius2 = 450.0 * COORD_FACTOR; // Radius at which you start to get audible pings
                    draw_ring(
                        image_buffer,
                        pos * COORD_FACTOR,
                        radius1,
                        [210, 0, 240, 120].into(),
                    );
                    draw_ring(
                        image_buffer,
                        pos * COORD_FACTOR,
                        radius2,
                        [210, 0, 120, 120].into(),
                    );
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn render_text(
        &self,
        text: &str,
        size: f32,
        color: Rgba<u8>,
        max_width: Option<u32>,
    ) -> RgbaImage {
        let mut layout = FontLayout::new(fontdue::layout::CoordinateSystem::PositiveYDown);
        layout.reset(&LayoutSettings {
            x: 0f32,
            y: 0f32,
            line_height: 1.0,
            max_width: max_width.map(|w| w as f32),
            max_height: None,
            horizontal_align: HorizontalAlign::Left,
            vertical_align: VerticalAlign::Top,
            wrap_style: WrapStyle::Letter,
            wrap_hard_breaks: true,
        });
        layout.append(&[&self.fonts[0]], &TextStyle::new(text, size, 0));
        let width = layout
            .glyphs()
            .iter()
            .map(|g| g.x as usize + g.width)
            .max()
            .unwrap_or(0);
        let mut img = RgbaImage::new(width as u32, layout.height() as u32);

        for glyph in layout.glyphs().iter() {
            let (metrics, bitmap) = self.fonts[0].rasterize_config_subpixel(glyph.key);
            for (i, (cr, cg, cb)) in bitmap.into_iter().tuples().enumerate() {
                let x = (i % metrics.width) as i64 + glyph.x as i64;
                let y = (i / metrics.width) as i64 + glyph.y as i64;
                if x >= 0 && x < img.width() as i64 && y >= 0 && y < img.height() as i64 {
                    let coverage = (cr as f32 + cg as f32 + cb as f32) / 3.0;
                    img.put_pixel(
                        x as u32,
                        y as u32,
                        [
                            color.0[0].saturating_add(255 - cr),
                            color.0[1].saturating_add(255 - cg),
                            color.0[2].saturating_add(255 - cb),
                            coverage as u8,
                        ]
                        .into(),
                    );
                }
            }
        }
        img
    }

    /// Renders text with settings more suited for very small font sizes
    /// (No subpixel rendering, bolder font)
    fn render_small_text(&self, text: &str, size: f32, color: Rgba<u8>) -> RgbaImage {
        let mut layout = FontLayout::new(fontdue::layout::CoordinateSystem::PositiveYDown);
        layout.reset(&LayoutSettings {
            x: 0f32,
            y: 0f32,
            line_height: 1.0,
            max_width: None,
            max_height: None,
            horizontal_align: HorizontalAlign::Left,
            vertical_align: VerticalAlign::Top,
            wrap_style: WrapStyle::Letter,
            wrap_hard_breaks: true,
        });
        layout.append(&[&self.fonts[1]], &TextStyle::new(text, size, 0));
        let width = layout
            .glyphs()
            .iter()
            .map(|g| g.x as usize + g.width)
            .max()
            .unwrap_or(0);
        let mut img = RgbaImage::new(width as u32, layout.height() as u32);

        for glyph in layout.glyphs().iter() {
            let (metrics, bitmap) = self.fonts[1].rasterize_config(glyph.key);
            for (i, v) in bitmap.into_iter().enumerate() {
                let x = (i % metrics.width) as i64 + glyph.x as i64;
                let y = (i / metrics.width) as i64 + glyph.y as i64;
                if x >= 0 && x < img.width() as i64 && y >= 0 && y < img.height() as i64 {
                    img.put_pixel(
                        x as u32,
                        y as u32,
                        [
                            color.0[0].saturating_add(255 - v),
                            color.0[1].saturating_add(255 - v),
                            color.0[2].saturating_add(255 - v),
                            v,
                        ]
                        .into(),
                    );
                }
            }
        }
        img
    }
}

/// Saves a layout image to disc.
/// Filename must end with a `.png` extension.
pub fn save_image<P: AsRef<Path>>(img: &RgbaImage, filename: P) -> Result<(), CaveripperError> {
    img.save_with_format(&filename, image::ImageFormat::Png)
        .change_context(CaveripperError::RenderingError)?;
    Ok(())
}

fn colorize(mut img: RgbaImage, color: Rgba<u8>) -> RgbaImage {
    img.enumerate_pixels_mut().for_each(|px| {
        px.2 .0[0] = color.0[0];
        px.2 .0[1] = color.0[1];
        px.2 .0[2] = color.0[2];
    });
    img
}

fn expand_canvas(canvas: &mut RgbaImage, w: u32, h: u32, fill_color: Option<Rgba<u8>>) {
    let mut new_canvas = RgbaImage::from_pixel(
        canvas.width() + w,
        canvas.height() + h,
        fill_color.unwrap_or_else(|| [0, 0, 0, 0].into()),
    );
    overlay(&mut new_canvas, canvas, 0, 0);
    *canvas = new_canvas;
}

fn draw_border(canvas: &mut RgbaImage, x1: u32, y1: u32, x2: u32, y2: u32) {
    let color = [255, 30, 30, 150].into();
    for x in x1..=x2 {
        canvas.put_pixel(x, y1, color);
        canvas.put_pixel(x, y2, color);
    }
    for y in y1..=y2 {
        canvas.put_pixel(x1, y, color);
        canvas.put_pixel(x2, y, color);
    }
}

fn draw_arrow_line(
    canvas: &mut RgbaImage,
    start: Point<2, f32>,
    end: Point<2, f32>,
    color: Rgba<u8>,
) {
    // Shorten the line slightly on both sides
    let vector = (end - start).normalized() * 6.0;
    let start = start + vector;
    let end = end - vector;

    // Draw main line
    draw_line(canvas, start, end, color);

    // Draw arrow arms
    let arrow_start_left = end - vector + (vector.perpendicular() / 2.0);
    let arrow_start_right = end - vector - (vector.perpendicular() / 2.0);
    draw_line(canvas, arrow_start_left, end, color);
    draw_line(canvas, arrow_start_right, end, color);
}

fn draw_line(canvas: &mut RgbaImage, start: Point<2, f32>, end: Point<2, f32>, color: Rgba<u8>) {
    let (mut x1, mut y1, mut x2, mut y2) = (start[0], start[1], end[0], end[1]);
    let steep = (y2 - y1).abs() > (x2 - x1).abs();

    if (steep && y1 > y2) || (!steep && x1 > x2) {
        (x1, x2) = (x2, x1);
        (y1, y2) = (y2, y1);
    }

    if steep {
        let slope = (x2 - x1) / (y2 - y1);

        for y in (y1.round() as u32)..(y2.round() as u32) {
            let true_y = y as f32 + 0.5;
            let true_x = x1 + (slope * (true_y - y1));
            try_blend(canvas, true_x.round() as i64, true_y.round() as i64, color);
        }
    } else {
        let slope = (y2 - y1) / (x2 - x1);

        for x in (x1.round() as u32)..(x2.round() as u32) {
            let true_x = x as f32 + 0.5;
            let true_y = y1 + (slope * (true_x - x1));
            try_blend(canvas, true_x.round() as i64, true_y.round() as i64, color);
        }
    }
}

fn draw_ring(canvas: &mut RgbaImage, pos: Point<2, f32>, r: f32, color: Rgba<u8>) {
    for i in 0..=(r as i32) {
        let offset = i as f32;
        let height = (r.powi(2) - offset.powi(2)).sqrt();
        try_blend(
            canvas,
            (pos[0] - offset) as i64,
            (pos[1] + height) as i64,
            color,
        );
        try_blend(
            canvas,
            (pos[0] - offset) as i64,
            (pos[1] - height) as i64,
            color,
        );
        try_blend(
            canvas,
            (pos[0] + offset) as i64,
            (pos[1] + height) as i64,
            color,
        );
        try_blend(
            canvas,
            (pos[0] + offset) as i64,
            (pos[1] - height) as i64,
            color,
        );
        try_blend(
            canvas,
            (pos[0] - height) as i64,
            (pos[1] + offset) as i64,
            color,
        );
        try_blend(
            canvas,
            (pos[0] - height) as i64,
            (pos[1] - offset) as i64,
            color,
        );
        try_blend(
            canvas,
            (pos[0] + height) as i64,
            (pos[1] + offset) as i64,
            color,
        );
        try_blend(
            canvas,
            (pos[0] + height) as i64,
            (pos[1] - offset) as i64,
            color,
        );
    }
}

/// Blends the pixel at the given coordinates, if they are in bounds. Otherwise
/// does nothing.
fn try_blend(canvas: &mut RgbaImage, x: i64, y: i64, color: Rgba<u8>) {
    if x > 0 && y > 0 && x < canvas.width() as i64 && y < canvas.height() as i64 {
        canvas.get_pixel_mut(x as u32, y as u32).blend(&color);
    }
}

fn circle(radius: u32, color: Rgba<u8>) -> RgbaImage {
    let mut buffer = RgbaImage::new(radius * 2, radius * 2);
    for x in 0..radius * 2 {
        for z in 0..radius * 2 {
            let r = radius as f32;
            if ((r - x as f32).powi(2) + (r - z as f32).powi(2)).sqrt() < r {
                buffer.put_pixel(x, z, color);
            }
        }
    }
    buffer
}

fn render_map_unit<'k, 'i: 'k>(
    renderer: &mut StickerRenderer<'k, 'i, AssetManager>,
    layer: &mut Layer<'k>,
    unit: &'i CaveUnit,
    pos: Point<2, f32>,
) {
    let key = format!("{}_{}", unit.unit_folder_name, unit.rotation);
    let unit_img_width = unit.width as f32 * GRID_FACTOR;
    let unit_img_height = unit.height as f32 * GRID_FACTOR;
    let sticker = renderer.add_sticker_with(key, || {
        Sticker::new(
            Renderable::Borrowed(unit),
            Origin::TopLeft,
            Size::Absolute(unit_img_width, unit_img_height, FilterType::Nearest),
        )
    });
    layer.add(sticker, pos[0], pos[1]);

    // Waterboxes
    for waterbox in unit.waterboxes.iter() {
        let key = format!("waterbox_{}_{}", waterbox.width(), waterbox.height());
        let waterbox_sticker = renderer.add_sticker_with(key, || {
            Sticker::new(
                Renderable::Owned(Box::new(Rectangle {
                    width: waterbox.width() * COORD_FACTOR,
                    height: waterbox.height() * COORD_FACTOR,
                    color: WATERBOX_COLOR.into(),
                })),
                Origin::TopLeft,
                Size::Native,
            )
        });
        layer.add(
            waterbox_sticker,
            pos[0] + (unit_img_width / 2.0) + (waterbox.p1[0] * COORD_FACTOR),
            pos[1] + (unit_img_height / 2.0) + (waterbox.p1[2] * COORD_FACTOR),
        );
    }
}

fn render_spawn_object<'k, 'i: 'k>(
    renderer: &mut StickerRenderer<'k, 'i, AssetManager>,
    layer: &mut Layer<'k>,
    spawn_object: &'i SpawnObject<'k>,
    pos: Point<2, f32>,
) {
    let (key, size) = if let SpawnObject::Gate(_, rotation) = spawn_object {
        (
            Cow::Owned(format!("{}_{}", spawn_object.name(), rotation)),
            GATE_SIZE,
        )
    } else {
        (Cow::Borrowed(spawn_object.name()), TEKI_SIZE)
    };
    let sticker = renderer.add_sticker_with(key, || {
        Sticker::new(
            Renderable::Borrowed(spawn_object),
            Origin::Center,
            Size::Absolute(size, size, FilterType::Lanczos3),
        )
    });
    layer.add(sticker, pos[0] * COORD_FACTOR, pos[1] * COORD_FACTOR);

    // Carrying Treasures
    if let SpawnObject::Teki(
        TekiInfo {
            carrying: Some(treasure),
            ..
        },
        _,
    ) = spawn_object
    {
        let carrying_sticker = renderer.add_sticker_with(treasure, || {
            Sticker::new(
                Renderable::Owned(Box::new(TreasureRenderer(treasure))),
                Origin::Center,
                Size::Absolute(CARRIED_TREASURE_SIZE, CARRIED_TREASURE_SIZE, FilterType::Lanczos3),
            )
        });
        layer.add(
            carrying_sticker,
            pos[0] * COORD_FACTOR + (size * 0.4),
            pos[1] * COORD_FACTOR + (size * 0.4),
        );
    }
}

enum TextureModifier {
    Scale(u32, u32),
    Falling,
    Carrying(String),
    QuickGlance(Rgba<u8>),
    GaugeRing,
}

trait Textured {
    fn get_texture<'a>(
        &self,
        game: &str,
        mgr: &'a AssetManager,
    ) -> Result<Cow<'a, RgbaImage>, CaveripperError>;
    fn get_texture_modifiers(&self) -> Vec<TextureModifier>;
}

impl<T: Textured> Textured for &T {
    fn get_texture<'a>(
        &self,
        game: &str,
        mgr: &'a AssetManager,
    ) -> Result<Cow<'a, RgbaImage>, CaveripperError> {
        (*self).get_texture(game, mgr)
    }
    fn get_texture_modifiers(&self) -> Vec<TextureModifier> {
        (*self).get_texture_modifiers()
    }
}

impl Textured for PlacedMapUnit<'_> {
    fn get_texture<'a>(
        &self,
        game: &str,
        mgr: &'a AssetManager,
    ) -> Result<Cow<'a, RgbaImage>, CaveripperError> {
        self.unit.get_texture(game, mgr)
    }

    fn get_texture_modifiers(&self) -> Vec<TextureModifier> {
        self.unit.get_texture_modifiers()
    }
}

impl Textured for TekiInfo {
    fn get_texture<'a>(
        &self,
        game: &str,
        mgr: &'a AssetManager,
    ) -> Result<Cow<'a, RgbaImage>, CaveripperError> {
        match get_special_texture_name(&self.internal_name) {
            Some(special_name) => {
                let filename = format!("resources/enemytex_special/{special_name}");
                Ok(Cow::Borrowed(mgr.get_img(filename)?))
            }
            None => {
                let filename = PathBuf::from_iter([
                    "assets",
                    game,
                    "teki",
                    &format!("{}.png", self.internal_name.to_ascii_lowercase()),
                ]);
                Ok(Cow::Borrowed(mgr.get_img(filename)?))
            }
        }
    }

    fn get_texture_modifiers(&self) -> Vec<TextureModifier> {
        let mut modifiers = Vec::new();
        if self.spawn_method.is_some() {
            modifiers.push(TextureModifier::Falling);
        }
        if let Some(carrying) = self.carrying.as_ref() {
            modifiers.push(TextureModifier::Carrying(carrying.clone()));
            modifiers.push(TextureModifier::QuickGlance(
                QUICKGLANCE_TREASURE_COLOR.into(),
            ));
            modifiers.push(TextureModifier::GaugeRing);
        }
        match self.internal_name.to_ascii_lowercase().as_str() {
            "blackpom" /* Violet Candypop */ => modifiers.push(TextureModifier::QuickGlance(QUICKGLANCE_VIOLET_CANDYPOP_COLOR.into())),
            "whitepom" /* Ivory Candypop */ => modifiers.push(TextureModifier::QuickGlance(QUICKGLANCE_IVORY_CANDYPOP_COLOR.into())),
            "minihoudai" /* Groink */ => modifiers.push(TextureModifier::QuickGlance(QUICKGLANCE_ROAMING_COLOR.into())),
            _ => {}
        }
        modifiers.push(TextureModifier::Scale(TEKI_SIZE as u32, TEKI_SIZE as u32));
        modifiers
    }
}

impl Textured for CapInfo {
    fn get_texture<'a>(
        &self,
        game: &str,
        mgr: &'a AssetManager,
    ) -> Result<Cow<'a, RgbaImage>, CaveripperError> {
        // We don't consider the possibility of treasures spawning in CapInfo here since that
        // is never done in the vanilla game. May need to fix in the future for romhack support.
        match get_special_texture_name(&self.internal_name) {
            Some(special_name) => {
                let filename = format!("resources/enemytex_special/{special_name}");
                Ok(Cow::Borrowed(mgr.get_img(filename)?))
            }
            None => {
                let filename = PathBuf::from_iter([
                    "assets",
                    game,
                    "teki",
                    &format!("{}.png", self.internal_name.to_ascii_lowercase()),
                ]);
                Ok(Cow::Borrowed(mgr.get_img(filename)?))
            }
        }
    }

    fn get_texture_modifiers(&self) -> Vec<TextureModifier> {
        let mut modifiers = Vec::new();
        if self.is_falling() {
            modifiers.push(TextureModifier::Falling);
            modifiers.push(TextureModifier::Scale(
                FALLING_CAP_TEKI_SIZE,
                FALLING_CAP_TEKI_SIZE,
            ));
        } else {
            modifiers.push(TextureModifier::Scale(TEKI_SIZE as u32, TEKI_SIZE as u32));
        }
        match self.internal_name.to_ascii_lowercase().as_str() {
            "blackpom" /* Violet Candypop */ => modifiers.push(TextureModifier::QuickGlance(QUICKGLANCE_VIOLET_CANDYPOP_COLOR.into())),
            "whitepom" /* Ivory Candypop */ => modifiers.push(TextureModifier::QuickGlance(QUICKGLANCE_IVORY_CANDYPOP_COLOR.into())),
            _ => {}
        }
        modifiers
    }
}

impl Textured for ItemInfo {
    fn get_texture<'a>(
        &self,
        game: &str,
        mgr: &'a AssetManager,
    ) -> Result<Cow<'a, RgbaImage>, CaveripperError> {
        let filename = PathBuf::from_iter([
            "assets",
            game,
            "treasures",
            &format!("{}.png", self.internal_name.to_ascii_lowercase()),
        ]);
        Ok(Cow::Borrowed(mgr.get_img(filename)?))
    }

    fn get_texture_modifiers(&self) -> Vec<TextureModifier> {
        vec![
            TextureModifier::QuickGlance(QUICKGLANCE_TREASURE_COLOR.into()),
            TextureModifier::Scale(24, 24),
            TextureModifier::GaugeRing,
        ]
    }
}

impl Textured for GateInfo {
    fn get_texture<'a>(
        &self,
        _game: &str,
        mgr: &'a AssetManager,
    ) -> Result<Cow<'a, RgbaImage>, CaveripperError> {
        let filename = "resources/enemytex_special/Gray_bramble_gate_icon.png";
        Ok(Cow::Borrowed(mgr.get_img(filename)?))
    }

    fn get_texture_modifiers(&self) -> Vec<TextureModifier> {
        vec![TextureModifier::Scale(GATE_SIZE as u32, GATE_SIZE as u32)]
        // TODO: gate hp modifier
    }
}

impl Textured for SpawnObject<'_> {
    fn get_texture<'a>(
        &self,
        game: &str,
        mgr: &'a AssetManager,
    ) -> Result<Cow<'a, RgbaImage>, CaveripperError> {
        match self {
            SpawnObject::Teki(tekiinfo, _) => tekiinfo.get_texture(game, mgr),
            SpawnObject::CapTeki(capinfo, _) => capinfo.get_texture(game, mgr),
            SpawnObject::Item(iteminfo) => iteminfo.get_texture(game, mgr),
            SpawnObject::Gate(gateinfo, rotation) => {
                let texture = gateinfo
                    .get_texture(game, mgr)
                    .change_context(CaveripperError::RenderingError)?;
                if rotation % 2 == 1 {
                    Ok(Cow::Owned(rotate90(texture.as_ref())))
                } else {
                    Ok(texture)
                }
            }
            SpawnObject::Hole(plugged) => {
                let filename = "resources/enemytex_special/Cave_icon.png";
                if !plugged {
                    Ok(Cow::Borrowed(mgr.get_img(filename)?))
                } else {
                    Ok(Cow::Borrowed(mgr.get_or_store_img(
                        "PLUGGED_HOLE".to_string(),
                        || {
                            let mut hole_icon = mgr.get_img(filename)?.clone();
                            if *plugged {
                                let plug_filename = "resources/enemytex_special/36px-Clog_icon.png";
                                let plug_icon = resize(
                                    mgr.get_img(plug_filename)?,
                                    hole_icon.width(),
                                    hole_icon.height(),
                                    FilterType::Lanczos3,
                                );
                                overlay(&mut hole_icon, &plug_icon, 0, 0);
                            }

                            Ok(hole_icon)
                        },
                    )?))
                }
            }
            SpawnObject::Geyser(plugged) => {
                let filename = "resources/enemytex_special/Geyser_icon.png";
                if !plugged {
                    Ok(Cow::Borrowed(mgr.get_img(filename)?))
                } else {
                    Ok(Cow::Borrowed(mgr.get_or_store_img(
                        "PLUGGED_GEYSER".to_string(),
                        || {
                            let mut hole_icon = mgr.get_img(filename)?.clone();
                            let plug_filename = "resources/enemytex_special/36px-Clog_icon.png";
                            let plug_icon = resize(
                                mgr.get_img(plug_filename)?,
                                hole_icon.width(),
                                hole_icon.height(),
                                FilterType::Lanczos3,
                            );
                            overlay(&mut hole_icon, &plug_icon, 0, 0);
                            Ok(hole_icon)
                        },
                    )?))
                }
            }
            SpawnObject::Ship => {
                let filename = "resources/enemytex_special/pod_icon.png";
                Ok(Cow::Borrowed(mgr.get_img(filename)?))
            }
        }
    }

    fn get_texture_modifiers(&self) -> Vec<TextureModifier> {
        match self {
            SpawnObject::Teki(tekiinfo, _) => tekiinfo.get_texture_modifiers(),
            SpawnObject::CapTeki(capinfo, _) => capinfo.get_texture_modifiers(),
            SpawnObject::Item(iteminfo) => iteminfo.get_texture_modifiers(),
            SpawnObject::Hole(_) | SpawnObject::Geyser(_) => {
                vec![TextureModifier::QuickGlance(QUICKGLANCE_EXIT_COLOR.into())]
            }
            SpawnObject::Ship => {
                vec![TextureModifier::QuickGlance(QUICKGLANCE_SHIP_COLOR.into())]
            }
            SpawnObject::Gate(gateinfo, _) => gateinfo.get_texture_modifiers(),
        }
    }
}

impl Textured for CaveUnit {
    fn get_texture<'a>(
        &self,
        game: &str,
        mgr: &'a AssetManager,
    ) -> Result<Cow<'a, RgbaImage>, CaveripperError> {
        let joke = self.unit_folder_name.contains("cap_") && joke_time();
        let filename = if joke {
            use rand::Rng;
            let which = rand::thread_rng().gen_range(1..=4);
            PathBuf::from_iter(["resources", "kaps", &format!("{which}.png")])
        }
        else {
            PathBuf::from_iter([
                "assets",
                game,
                "mapunits",
                &self.unit_folder_name,
                "arc",
                "texture.png",
            ])
        };
        let mut img = mgr.get_img(&filename)?.to_owned();

        // Radar images are somewhat dark by default; this improves visibility.
        if !joke {
            brighten_in_place(&mut img, 75);
        }


        for _ in 0..self.rotation {
            img = rotate90(&img);
        }

        img = resize(
            &img,
            (self.width * 8) as u32 * RENDER_SCALE as u32,
            (self.height * 8) as u32 * RENDER_SCALE as u32,
            FilterType::Nearest,
        );

        for waterbox in self.waterboxes.iter() {
            let (x1, z1, x2, z2) = match self.rotation {
                0 => (
                    waterbox.p1[0],
                    waterbox.p1[2],
                    waterbox.p2[0],
                    waterbox.p2[2],
                ),
                1 => (
                    -waterbox.p2[2],
                    waterbox.p1[0],
                    -waterbox.p1[0],
                    waterbox.p2[0],
                ),
                2 => (
                    -waterbox.p2[0],
                    -waterbox.p2[2],
                    -waterbox.p1[0],
                    -waterbox.p1[2],
                ),
                3 => (
                    waterbox.p1[2],
                    -waterbox.p2[0],
                    waterbox.p2[2],
                    -waterbox.p1[0],
                ),
                _ => panic!("Invalid rotation"),
            };
            let x1 = x1 * COORD_FACTOR;
            let z1 = z1 * COORD_FACTOR;
            let x2 = x2 * COORD_FACTOR;
            let z2 = z2 * COORD_FACTOR;
            let w = (self.width as f32 * GRID_FACTOR) / 2.0;
            let h = (self.height as f32 * GRID_FACTOR) / 2.0;
            let square =
                RgbaImage::from_pixel((x2 - x1) as u32, (z2 - z1) as u32, [0, 100, 230, 50].into());
            overlay(&mut img, &square, (x1 + w) as i64, (z1 + h) as i64);
        }

        Ok(Cow::Borrowed(mgr.get_or_store_img(
            format!(
                "{}_r{}_prerendered",
                filename.to_string_lossy(),
                self.rotation
            ),
            Box::new(|| Ok(img)),
        )?))
    }

    fn get_texture_modifiers(&self) -> Vec<TextureModifier> {
        Vec::new()
    }
}


fn joke_time() -> bool {
    use chrono::{Datelike, Duration};
    let now = chrono::Utc::now();
    [-12, 0, 1].into_iter().any(|offset| {
        let time = now + Duration::hours(offset);
        time.month() == 4 && time.day() == 1
    })
}


impl Render<AssetManager> for CaveUnit {
    fn render(&self, mut canvas: CanvasView, helper: &AssetManager) {
        // TODO: pass game somehow
        let filename = PathBuf::from_iter([
            "assets",
            "pikmin2",
            "mapunits",
            &self.unit_folder_name,
            "arc",
            "texture.png",
        ]);
        let mut img = helper.get_img(&filename).unwrap().to_owned();

        // Radar images are somewhat dark by default; this improves visibility.
        brighten_in_place(&mut img, 75);

        for _ in 0..self.rotation {
            img = rotate90(&img);
        }

        canvas.overlay(&img, 0, 0);
    }

    fn dimensions(&self) -> (f32, f32) {
        (self.width as f32 * 8.0, self.height as f32 * 8.0)
    }

}

impl Render<AssetManager> for SpawnObject<'_> {
    fn render(&self, mut canvas: CanvasView, helper: &AssetManager) {
        match self {
            SpawnObject::Teki(_, _) | SpawnObject::CapTeki(_, _) => {
                let filename = match get_special_texture_name(self.name()) {
                    Some(special_name) => {
                        PathBuf::from_iter(["resources", "enemytex_special", special_name])
                    }
                    None => PathBuf::from_iter([
                        "assets",
                        "pikmin2",
                        "teki",
                        &format!("{}.png", self.name().to_ascii_lowercase()),
                    ]),
                };
                canvas.overlay(&resize(helper.get_img(filename).unwrap(), 40, 40, FilterType::Lanczos3), 0, 0);
            }
            SpawnObject::Item(info) => TreasureRenderer(&info.internal_name).render(canvas, helper),
            SpawnObject::Gate(_, rotation) => {
                let filename = "resources/enemytex_special/Gray_bramble_gate_icon.png";
                let img = helper.get_img(filename).unwrap();
                if rotation % 2 == 1 {
                    canvas.overlay(&rotate90(img), 0, 0);
                } else {
                    canvas.overlay(img, 0, 0);
                }
            }
            SpawnObject::Hole(plugged) | SpawnObject::Geyser(plugged) => {
                let filename = match self {
                    SpawnObject::Hole(_) => "resources/enemytex_special/Cave_icon.png",
                    SpawnObject::Geyser(_) => "resources/enemytex_special/Geyser_icon.png",
                    _ => unreachable!(),
                };
                let img = helper.get_img(filename).unwrap();
                canvas.overlay(img, 0, 0);
                if *plugged {
                    let plug_filename = "resources/enemytex_special/36px-Clog_icon.png";
                    let plug_icon = resize(
                        helper.get_img(plug_filename).unwrap(),
                        img.width(),
                        img.height(),
                        FilterType::Lanczos3,
                    );
                    canvas.overlay(&plug_icon, 0, 0);
                }
            }
            SpawnObject::Ship => {
                let filename = "resources/enemytex_special/pod_icon.png";
                canvas.overlay(helper.get_img(filename).unwrap(), 0, 0);
            }
        }
    }

    fn dimensions(&self) -> (f32, f32) {
        match self {
            // TODO: Boss teki and potentially some romhack teki have larger
            // image dimensions. Currently these are all scaled to 40x40 but
            // quality could be better if this can be avoided.
            SpawnObject::Teki(_, _) | SpawnObject::CapTeki(_, _) => (40.0, 40.0),
            SpawnObject::Item(info) => TreasureRenderer(&info.internal_name).dimensions(),
            SpawnObject::Gate(_, _rotation) => (48.0, 48.0),
            SpawnObject::Hole(_) => (32.0, 32.0),
            SpawnObject::Geyser(_) => (40.0, 40.0),
            SpawnObject::Ship => (30.0, 30.0),
        }
    }

}

/// Helper to reduce asset manager lookups
struct TreasureRenderer<'a>(pub &'a str);
impl Render<AssetManager> for TreasureRenderer<'_> {
    fn render(&self, mut canvas: CanvasView, helper: &AssetManager) {
        let filename = PathBuf::from_iter([
            "assets",
            "pikmin2",
            "treasures",
            &format!("{}.png", self.0.to_ascii_lowercase()),
        ]);
        canvas.overlay(helper.get_img(filename).unwrap(), 0, 0);
    }

    fn dimensions(&self) -> (f32, f32) {
        (32.0, 32.0)
    }

}
