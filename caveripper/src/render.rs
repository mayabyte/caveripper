#[cfg(test)]
mod test;

use std::borrow::Cow;
use std::cmp::max;
use std::f32::consts::PI;
use std::path::{Path, PathBuf};

use crate::caveinfo::{CapInfo, GateInfo, ItemInfo, TekiInfo, CaveInfo, CaveUnit, RoomType};
use crate::assets::{AssetManager, get_special_texture_name};
use crate::errors::CaveripperError;
use crate::layout::{Layout, SpawnObject, PlacedMapUnit};
use clap::Args;
use error_stack::{Result, ResultExt, IntoReport};
use fontdue::layout::{Layout as FontLayout, TextStyle, LayoutSettings, VerticalAlign, HorizontalAlign, WrapStyle};
use fontdue::{Font, FontSettings};
use image::imageops::colorops::brighten_in_place;
use image::imageops::{resize, rotate90, overlay};
use image::{Rgba, RgbaImage};
use image::{Pixel, imageops::FilterType};
use itertools::Itertools;
use log::{info};
use once_cell::sync::Lazy;

const RENDER_SCALE: u32 = 8;
const GRID_FACTOR: i64 = 8 * RENDER_SCALE as i64;
const COORD_FACTOR: f32 = (8.0 * RENDER_SCALE as f32) / 170.0;
const BASE_TEKI_SIZE: u32 = 32;
const GATE_SIZE: u32 = 8 * RENDER_SCALE;
const TREASURE_SIZE: u32 = 36;
const FALLING_CAP_TEKI_SIZE: u32 = 29;
const QUICKGLANCE_CIRCLE_SCALE: f32 = 2.4;
const QUICKGLANCE_TREASURE_COLOR: [u8; 4] = [230, 115, 0, 110];
const QUICKGLANCE_EXIT_COLOR: [u8; 4] = [2, 163, 69, 110];
const QUICKGLANCE_SHIP_COLOR: [u8; 4] = [255, 40, 40, 80];
const QUICKGLANCE_VIOLET_CANDYPOP_COLOR: [u8; 4] = [255, 0, 245, 80];
const QUICKGLANCE_IVORY_CANDYPOP_COLOR: [u8; 4] = [100, 100, 100, 120];
const QUICKGLANCE_ROAMING_COLOR: [u8; 4] = [200, 0, 130, 60];
const WAYPOINT_COLOR: [u8; 4] = [130, 199, 56, 150];
const CARRY_PATH_COLOR: [u8; 4] = [83, 125, 29, 200];
const WAYPOINT_DIST_TXT_COLOR: [u8; 4] = [36, 54, 14, 255];
const HEADER_BACKGROUND: [u8; 4] = [220,220,220,255];
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
#[clap(next_help_heading="Rendering options")]
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
    #[clap(long, short='s')]
    pub draw_score: bool,
}

#[derive(Default, Debug, Args)]
#[clap(next_help_heading="Rendering options")]
pub struct CaveinfoRenderOptions {
    /// Draw treasure values and carry weights.
    #[clap(long, default_value_t=true, action=clap::ArgAction::Set)]
    pub draw_treasure_info: bool,

    /// Render pathing waypoints
    #[clap(long, default_value_t=true, action=clap::ArgAction::Set)]
    pub draw_waypoints: bool,

    /// Render waypoint distances. Useful for calculating Distance Score.
    #[clap(long, default_value_t=true, action=clap::ArgAction::Set)]
    pub draw_waypoint_distances: bool,
}


pub fn render_layout(layout: &Layout, options: LayoutRenderOptions) -> Result<RgbaImage, CaveripperError> {
    info!("Drawing layout image...");

    // Find the minimum and maximum map tile coordinates in the layout.
    let max_map_x = layout.map_units.iter().map(|unit| unit.x as i64 + unit.unit.width as i64).max()
        .ok_or(CaveripperError::LayoutGenerationError)?;
    let max_map_z = layout.map_units.iter().map(|unit| unit.z as i64 + unit.unit.height as i64).max()
        .ok_or(CaveripperError::LayoutGenerationError)?;

    // Each map tile is 8x8 pixels on the radar.
    // We scale this up further so teki and treasure textures can be rendered at a decent
    // resolution on top of the generated layout images.
    let mut canvas = RgbaImage::from_pixel(
        max_map_x as u32 * 8 * RENDER_SCALE,
        max_map_z as u32 * 8 * RENDER_SCALE,
        [0, 0, 0, 255].into(),
    );

    // Draw map units
    for map_unit in layout.map_units.iter() {
        let radar_image = map_unit.get_texture(&layout.sublevel.cfg.game)
            .change_context(CaveripperError::RenderingError)?;

        // Copy the pixels of the radar image to the buffer
        let img_x = map_unit.x as i64 * GRID_FACTOR;
        let img_z = map_unit.z as i64 * GRID_FACTOR;
        overlay(&mut canvas, radar_image, img_x, img_z);
    }

    // Draw a map unit grid, if enabled
    if options.draw_grid {
        let grid_color: Rgba<u8> = [255, 0, 0, 150].into();
        let grid_size = GRID_FACTOR as u32;
        for x in 0..canvas.width() {
            for z in 0..canvas.height() {
                if x % grid_size == 0 || z % grid_size == 0 {
                    let new_pix = canvas.get_pixel_mut(x, z);
                    new_pix.blend(&grid_color);
                }
            }
        }
    }

    // Draw spawned objects
    for spawnpoint in layout.map_units.iter().flat_map(|unit| unit.spawnpoints.iter()) {
        for spawn_object in spawnpoint.contains.iter() {
            match spawn_object {
                SpawnObject::Teki(tekiinfo, (dx, dz)) => {
                    draw_object_at(&mut canvas, tekiinfo, spawnpoint.x + dx, spawnpoint.z + dz, &layout.sublevel.cfg.game, &options)
                    .change_context(CaveripperError::RenderingError)?;
                },
                SpawnObject::CapTeki(capinfo, _) if capinfo.is_falling() => {
                    draw_object_at(&mut canvas, capinfo, spawnpoint.x - 30.0, spawnpoint.z - 30.0, &layout.sublevel.cfg.game, &options)
                    .change_context(CaveripperError::RenderingError)?;
                },
                _ => {
                    draw_object_at(&mut canvas, spawn_object, spawnpoint.x, spawnpoint.z, &layout.sublevel.cfg.game, &options)
                    .change_context(CaveripperError::RenderingError)?;
                },
            }
        }
    }

    // Draw seam teki
    for door in layout.map_units.iter().flat_map(|unit| unit.doors.iter()) {
        if let Some(spawn_object) = door.borrow().seam_spawnpoint.as_ref() {
            // Adjust the door's map tile coordinates to world coordinates
            let mut x = (door.borrow().x * 170) as f32;
            let mut z = (door.borrow().z * 170) as f32;
            match door.borrow().door_unit.direction {
                0 | 2 => x += 85.0,
                1 | 3 => z += 85.0,
                _ => panic!("Invalid door direction in render"),
            }

            match spawn_object {
                SpawnObject::Gate(gateinfo) => {
                    let texture = gateinfo.get_texture(&layout.sublevel.cfg.game)
                    .change_context(CaveripperError::RenderingError)?;
                    if door.borrow().door_unit.direction % 2 == 1 {
                        draw_object_at(
                            &mut canvas,
                            &WithCustomTexture{ inner: gateinfo, custom_texture: rotate90(texture) },
                            x, z, &layout.sublevel.cfg.game, &options
                        )
                        .change_context(CaveripperError::RenderingError)?;
                    }
                    else {
                        draw_object_at(&mut canvas, gateinfo, x, z, &layout.sublevel.cfg.game, &options)
                        .change_context(CaveripperError::RenderingError)?;
                    }
                }
                _ => {
                    draw_object_at(&mut canvas, spawn_object, x, z, &layout.sublevel.cfg.game, &options)
                    .change_context(CaveripperError::RenderingError)?;
                },
            }
        }
    }

    if options.draw_score {
        for unit in layout.map_units.iter() {
            let score_text = render_small_text(&format!("{}", unit.total_score), 16.0, [170, 50, 30, 255].into());
            let img_x = ((unit.x as f32 + unit.unit.width as f32 / 2.0) * GRID_FACTOR as f32) as i64;
            let img_z = ((unit.z as f32 + unit.unit.height as f32 / 2.0) * GRID_FACTOR as f32) as i64;
            overlay(&mut canvas, &score_text, img_x - (score_text.width() / 2) as i64, img_z - (score_text.height() / 2) as i64);
        }
    }

    Ok(canvas)
}

pub fn render_caveinfo(caveinfo: &CaveInfo, options: CaveinfoRenderOptions) -> Result<RgbaImage, CaveripperError> {
    let mut canvas_header = RgbaImage::from_pixel(1060, 310, HEADER_BACKGROUND.into());

    // Sublevel name
    let sublevel_title = render_text(&caveinfo.long_name(), 64.0, [0,0,0, 255].into(), None);
    overlay(&mut canvas_header, &sublevel_title, CAVEINFO_MARGIN * 2, -8);

    // Metadata icons - ship, hole plugged/unplugged, geyser yes/no, num gates
    let mut metadata_icons = Vec::new();
    metadata_icons.push(resize(SpawnObject::Ship.get_texture(&caveinfo.cave_cfg.game).change_context(CaveripperError::RenderingError)?,
        CAVEINFO_ICON_SIZE, CAVEINFO_ICON_SIZE, FilterType::Lanczos3));
    if !caveinfo.is_final_floor {
        metadata_icons.push(
            resize(
                SpawnObject::Hole(caveinfo.exit_plugged)
                    .get_texture(&caveinfo.cave_cfg.game)
                    .change_context(CaveripperError::RenderingError)?,
                CAVEINFO_ICON_SIZE, CAVEINFO_ICON_SIZE, FilterType::Lanczos3)
            );
    }
    if caveinfo.is_final_floor || caveinfo.has_geyser {
        metadata_icons.push(
            resize(
                SpawnObject::Geyser(
                    caveinfo.is_challenge_mode() && caveinfo.is_final_floor
                ).get_texture(&caveinfo.cave_cfg.game)
                .change_context(CaveripperError::RenderingError)?,
                CAVEINFO_ICON_SIZE,
                CAVEINFO_ICON_SIZE,
                FilterType::Lanczos3
            )
        );
    }
    let num_gates = caveinfo.max_gates;
    for gateinfo in caveinfo.gate_info.iter() {
        let gate_icon = resize(
            gateinfo.get_texture(&caveinfo.cave_cfg.game).change_context(CaveripperError::RenderingError)?,
            CAVEINFO_ICON_SIZE, CAVEINFO_ICON_SIZE, FilterType::Lanczos3);
        let num_txt = render_small_text(&format!("x{num_gates}"), 19.0, [20, 20, 20, 255].into());
        let hp_txt = render_small_text(&format!("{}HP", gateinfo.health as u32), 13.0, [20, 20, 20, 255].into());
        let mut final_gate_icon = RgbaImage::new(CAVEINFO_ICON_SIZE, CAVEINFO_ICON_SIZE);
        overlay(&mut final_gate_icon, &gate_icon, 0, -12);
        overlay(&mut final_gate_icon, &hp_txt, CAVEINFO_ICON_SIZE as i64 / 2 - hp_txt.width() as i64 / 2, CAVEINFO_ICON_SIZE as i64 - 33);
        overlay(&mut final_gate_icon, &num_txt, CAVEINFO_ICON_SIZE as i64 / 2 - num_txt.width() as i64 / 2, CAVEINFO_ICON_SIZE as i64 - 24);
        metadata_icons.push(final_gate_icon);
    }

    for (i, icon) in metadata_icons.into_iter().enumerate() {
        overlay(
            &mut canvas_header,
            &icon,
            35 + sublevel_title.width() as i64 + i as i64 * (CAVEINFO_ICON_SIZE as i64 + CAVEINFO_MARGIN*3),
            CAVEINFO_MARGIN + 12
        );
    }

    let poko_icon = resize(
        AssetManager::get_img("resources/enemytex_special/Poko_icon.png").change_context(CaveripperError::RenderingError)?,
        16, 19, FilterType::Lanczos3);

    // Teki section
    let mut base_y =  64 + CAVEINFO_MARGIN * 2;
    let teki_header = render_text(&format!("Teki (max {})", caveinfo.max_main_objects), 48.0, [225,0,0, 255].into(), None);
    overlay(&mut canvas_header, &teki_header, CAVEINFO_MARGIN * 2, base_y);
    let mut base_x = (CAVEINFO_MARGIN * 4) + teki_header.width() as i64;
    base_y += (64 - CAVEINFO_ICON_SIZE as i64) / 2;

    for group in [8, 1, 0, 6, 5] {
        for tekiinfo in caveinfo.teki_group(group) {
            let texture = resize(tekiinfo.get_texture(&caveinfo.cave_cfg.game).change_context(CaveripperError::RenderingError)?,
                CAVEINFO_ICON_SIZE, CAVEINFO_ICON_SIZE, FilterType::Lanczos3);

            // If we overflow the width of the image, wrap to the next line.
            if base_x + CAVEINFO_ICON_SIZE as i64 + CAVEINFO_MARGIN > canvas_header.width() as i64 {
                base_x = (CAVEINFO_MARGIN * 4) + teki_header.width() as i64;
                base_y += 70;

                // Expand the header to make room for the other rows
                expand_canvas(&mut canvas_header, 0, 70 + CAVEINFO_MARGIN as u32, Some([220,220,220,255].into()));
            }

            overlay(&mut canvas_header, &texture, base_x, base_y);

            let mut extra_width = 0;
            for modifier in tekiinfo.get_texture_modifiers().iter() {
                match modifier {
                    TextureModifier::Falling => {
                        let falling_icon_texture = resize(
                            AssetManager::get_img("resources/enemytex_special/falling_icon.png").change_context(CaveripperError::RenderingError)?,
                            24, 24, FilterType::Nearest
                        );
                        overlay(&mut canvas_header, &falling_icon_texture, base_x - 8, base_y - 2);
                    },
                    TextureModifier::Carrying(carrying) => {
                        let treasure = AssetManager::treasure_list().change_context(CaveripperError::RenderingError)?.iter().find(|t| t.internal_name.eq_ignore_ascii_case(carrying))
                            .expect("Teki carrying unknown or invalid treasure!");

                        let carried_treasure_icon = resize(
                            AssetManager::get_img(
                                &PathBuf::from(&caveinfo.cave_cfg.game).join("treasures").join(format!("{carrying}.png"))
                            ).change_context(CaveripperError::RenderingError)?,
                            CAVEINFO_ICON_SIZE - 10, CAVEINFO_ICON_SIZE - 10, FilterType::Lanczos3
                        );
                        overlay(&mut canvas_header, &carried_treasure_icon, base_x + 18, base_y + 14);

                        // Treasure value/carry text
                        if options.draw_treasure_info {
                            let value_text = render_text(&format!("{}", treasure.value), 20.0, [20,20,20, 255].into(), None);
                            let carriers_text = render_text(&format!("{}/{}", treasure.min_carry, treasure.max_carry), 20.0, [20, 20, 20, 255].into(), None);

                            let sidetext_x = base_x + texture.width() as i64 + 5;
                            let text_width = max(poko_icon.width() as i64 + value_text.width() as i64, carriers_text.width() as i64) + CAVEINFO_MARGIN * 2;
                            if sidetext_x + text_width > canvas_header.width() as i64 {
                                let header_width = canvas_header.width() as i64;
                                expand_canvas(&mut canvas_header, (sidetext_x + text_width - header_width) as u32, 0, Some([220,220,220,255].into()));
                            }

                            overlay(&mut canvas_header, &poko_icon, sidetext_x, base_y + 4);
                            overlay(&mut canvas_header, &value_text,
                                sidetext_x + poko_icon.width() as i64 + 3,
                                base_y - value_text.height() as i64 / 2 + poko_icon.height() as i64 / 2 + 4
                            );

                            overlay(&mut canvas_header, &carriers_text, sidetext_x, base_y + poko_icon.height() as i64 + 2);

                            base_x += text_width;
                            extra_width += text_width;
                        }
                    },
                    _ => {}
                }
            }


            let teki_subtext = if tekiinfo.filler_distribution_weight > 0 && tekiinfo.minimum_amount > 0 {
                format!("x{} w{}", tekiinfo.minimum_amount, tekiinfo.filler_distribution_weight)
            }
            else if tekiinfo.minimum_amount == 0 && tekiinfo.filler_distribution_weight > 0 {
                format!("w{}", tekiinfo.filler_distribution_weight)
            }
            else {
                format!("x{}", tekiinfo.minimum_amount)
            };

            let subtext_color = group_color(tekiinfo.group).into();

            let teki_subtext_texture = render_text(&teki_subtext, 24.0, subtext_color, None);
            overlay(
                &mut canvas_header,
                &teki_subtext_texture,
                base_x + CAVEINFO_ICON_SIZE as i64 / 2 - teki_subtext_texture.width() as i64 / 2 - extra_width / 2,
                base_y + CAVEINFO_ICON_SIZE as i64 - 8
            );

            base_x += CAVEINFO_ICON_SIZE as i64 + CAVEINFO_MARGIN;
        }
    }

    base_y += teki_header.height() as i64 + CAVEINFO_MARGIN;

    // Treasures section
    let treasure_header = render_text("Treasures", 48.0, [207, 105, 33, 255].into(), None);
    overlay(&mut canvas_header, &treasure_header, CAVEINFO_MARGIN * 2, base_y);

    let mut base_x = treasure_header.width() as i64 + CAVEINFO_MARGIN;
    for treasureinfo in caveinfo.item_info.iter() {
        let treasure = AssetManager::treasure_list().change_context(CaveripperError::RenderingError)?.iter().find(|t| t.internal_name.eq_ignore_ascii_case(&treasureinfo.internal_name))
            .expect("Unknown or invalid treasure!");

        let treasure_texture = resize(treasureinfo.get_texture(&caveinfo.cave_cfg.game).change_context(CaveripperError::RenderingError)?, CAVEINFO_ICON_SIZE, CAVEINFO_ICON_SIZE, FilterType::Lanczos3);
        let x = base_x + CAVEINFO_MARGIN * 4;
        let y = base_y + CAVEINFO_MARGIN + (64 - CAVEINFO_ICON_SIZE as i64) / 2;
        overlay(&mut canvas_header, &treasure_texture, x, y);

        let mut extra_width = 0;
        if options.draw_treasure_info {
            let value_text = render_text(&format!("{}", treasure.value), 20.0, [20,20,20, 255].into(), None);
            let sidetext_x = x + treasure_texture.width() as i64 + 2;
            overlay(&mut canvas_header, &poko_icon, sidetext_x, y + 4);
            overlay(&mut canvas_header, &value_text,
                sidetext_x + poko_icon.width() as i64 + 3,
                y - value_text.height() as i64 / 2 + poko_icon.height() as i64 / 2 + 4
            );

            let carriers_text = render_text(&format!("{}/{}", treasure.min_carry, treasure.max_carry), 20.0, [20, 20, 20, 255].into(), None);
            overlay(&mut canvas_header, &carriers_text, sidetext_x, y + poko_icon.height() as i64 + 2);

            extra_width += max(poko_icon.width() as i64 + value_text.width() as i64, carriers_text.width() as i64) + 4;
        }

        if caveinfo.is_challenge_mode() {
            let subtext_color = group_color(2).into();
            let treasure_subtext = format!("x{}", treasureinfo.min_amount);
            let treasure_subtext_texture = render_text(&treasure_subtext, 24.0, subtext_color, None);
            overlay(
                &mut canvas_header,
                &treasure_subtext_texture,
                x + (CAVEINFO_ICON_SIZE as i64 / 2) - (treasure_subtext_texture.width() as i64 / 2) + (extra_width / 2),
                y + CAVEINFO_ICON_SIZE as i64 - 12
            );
        }

        base_x+= treasure_texture.width() as i64 + extra_width;
    }

    base_y += treasure_header.height() as i64;

    // Make room for treasure number text
    if caveinfo.is_challenge_mode() {
        base_y += CAVEINFO_MARGIN;
    }

    // Capteki section
    let capteki_color = group_color(9).into();
    let capteki_header = render_text("Cap Teki", 48.0, capteki_color, None);
    overlay(&mut canvas_header, &capteki_header, CAVEINFO_MARGIN * 2, base_y);
    for (i, capinfo) in caveinfo.cap_info.iter().enumerate() {
        let texture = resize(capinfo.get_texture(&caveinfo.cave_cfg.game).change_context(CaveripperError::RenderingError)?, CAVEINFO_ICON_SIZE, CAVEINFO_ICON_SIZE, FilterType::Lanczos3);
        let x = (CAVEINFO_MARGIN * 5) + capteki_header.width() as i64 + i as i64 * (CAVEINFO_ICON_SIZE as i64 + CAVEINFO_MARGIN * 2);
        let y = base_y + (64 - CAVEINFO_ICON_SIZE as i64) / 2;
        overlay(&mut canvas_header, &texture, x, y);

        for modifier in capinfo.get_texture_modifiers().iter() {
            if let TextureModifier::Falling = modifier {
                let falling_icon_texture = resize(
                    AssetManager::get_img("resources/enemytex_special/falling_icon.png").change_context(CaveripperError::RenderingError)?,
                    24, 24, FilterType::Nearest
                );
                overlay(&mut canvas_header, &falling_icon_texture, x - 8, y - 2);
            }
        }

        let capteki_subtext = if capinfo.filler_distribution_weight > 0 && capinfo.minimum_amount > 0 {
            format!("x{} w{}", capinfo.minimum_amount, capinfo.filler_distribution_weight)
        }
        else if capinfo.minimum_amount == 0 && capinfo.filler_distribution_weight > 0 {
            format!("w{}", capinfo.filler_distribution_weight)
        }
        else {
            format!("x{}", capinfo.minimum_amount)
        };

        let capteki_subtext_texture = render_text(&capteki_subtext, 24.0, capteki_color, None);
        overlay(&mut canvas_header, &capteki_subtext_texture, x + CAVEINFO_ICON_SIZE as i64 / 2 - capteki_subtext_texture.width() as i64 / 2, y + CAVEINFO_ICON_SIZE as i64 - 10);
    }

    // Done with header section
    // Start Map Tile section

    let mut canvas_maptiles = RgbaImage::from_pixel(canvas_header.width(), 200, MAPTILES_BACKGROUND.into());

    let maptiles_metadata_txt = render_text(
        &format!(
            "Num Rooms: {}     CorridorBetweenRoomsProb: {}%     CapOpenDoorsProb: {}%",
            caveinfo.num_rooms, caveinfo.corridor_probability * 100.0, caveinfo.cap_probability * 100.0
        ),
        24.0,
        [220,220,220,255].into(),
        Some(canvas_maptiles.width())
    );
    overlay(&mut canvas_maptiles, &maptiles_metadata_txt, canvas_header.width() as i64 / 2 - maptiles_metadata_txt.width() as i64 / 2, 0);

    let maptile_margin = (RENDER_SCALE * 4) as i64;
    let mut base_x = maptile_margin;
    let mut base_y = maptiles_metadata_txt.height() as i64 + maptile_margin;
    let mut max_y = base_y;

    let rooms = caveinfo.cave_units.iter()
        .filter(|unit| unit.rotation == 0)
        .filter(|unit| unit.room_type == RoomType::Room);

    let caps: Vec<_> = caveinfo.cave_units.iter()
        .filter(|unit| unit.rotation == 0)
        .filter(|unit| unit.room_type == RoomType::DeadEnd)
        .collect();

    for (i, unit) in caps.iter().enumerate() {
        let unit_texture = unit.get_texture(&caveinfo.cave_cfg.game).change_context(CaveripperError::RenderingError)?;
        let y = base_y + i as i64 * ((RENDER_SCALE * 8) as i64 + maptile_margin);

        if y + unit_texture.height() as i64 > canvas_maptiles.height() as i64 {
            let h = canvas_maptiles.height();
            expand_canvas(
                &mut canvas_maptiles,
                0,
                y as u32 + unit_texture.height() + (maptile_margin as u32) * 2 - h,
                Some([20, 20, 20, 255].into())
            );
        }

        overlay(&mut canvas_maptiles, unit_texture, base_x, y);
        draw_border(
            &mut canvas_maptiles,
            base_x as u32,
            y as u32,
            base_x as u32 + (RENDER_SCALE * 8),
            y as u32 + (RENDER_SCALE * 8),
        );

        for spawnpoint in unit.spawnpoints.iter() {
            let sp_x = (spawnpoint.pos_x * COORD_FACTOR) as i64 + (unit_texture.width() / 2) as i64;
            let sp_z = (spawnpoint.pos_z * COORD_FACTOR) as i64 + (unit_texture.height() / 2) as i64;

            let sp_img = match spawnpoint.group {
                6 => colorize(resize(AssetManager::get_img("resources/enemytex_special/leaf_icon.png").change_context(CaveripperError::RenderingError)?, 10, 10, FilterType::Lanczos3), group_color(6).into()),
                9 => circle(5, group_color(9).into()),
                _ => circle(5, [255,0,0,255].into()),
            };

            overlay(&mut canvas_maptiles, &sp_img, base_x + sp_x - (sp_img.width() / 2) as i64 , y + sp_z - (sp_img.height() / 2) as i64);
        }
    }

    if !caps.is_empty() {
        base_x += (RENDER_SCALE * 8) as i64 + maptile_margin;
    }

    for unit in rooms {
        let mut unit_texture = unit.get_texture(&caveinfo.cave_cfg.game).change_context(CaveripperError::RenderingError)?.clone();

        // If the unit is just too big, we have to expand the whole image
        if unit_texture.width() + 2 > canvas_maptiles.width() {
            let expand_by = (unit_texture.width() + (maptile_margin as u32 * 2) + 2) - canvas_maptiles.width();
            expand_canvas(&mut canvas_maptiles, expand_by, 0, Some(MAPTILES_BACKGROUND.into()));
            expand_canvas(&mut canvas_header, expand_by, 0, Some(HEADER_BACKGROUND.into()));
        }
        // Normal case: we just overran in this row
        if base_x + unit_texture.width() as i64 + 2 > canvas_maptiles.width() as i64 {
            base_x = maptile_margin;
            base_y = max_y + maptile_margin;
        }
        // This next tile teeeechnically fits, so we just fudge it a little by expanding the width
        else if base_x + unit_texture.width() as i64 + maptile_margin + 2 > canvas_maptiles.width() as i64 {
            let expand_by = (base_x + maptile_margin) as u32 + unit_texture.width() - canvas_maptiles.width();
            expand_canvas(&mut canvas_maptiles, expand_by, 0, Some(MAPTILES_BACKGROUND.into()));
            expand_canvas(&mut canvas_header, expand_by, 0, Some(HEADER_BACKGROUND.into()));
        }

        let unit_name_text = render_text(&unit.unit_folder_name, 14.0, [220,220,220,255].into(), Some(unit_texture.width()));

        if base_y + (unit_texture.height() + unit_name_text.height()) as i64 > canvas_maptiles.height() as i64 {
            let h = canvas_maptiles.height();
            expand_canvas(
                &mut canvas_maptiles,
                0,
                base_y as u32 + unit_texture.height() + unit_name_text.height() + (maptile_margin as u32) - h,
                Some([20, 20, 20, 255].into())
            );
        }

        if options.draw_waypoints {
            for waypoint in unit.waypoints.iter() {
                let wp_x = (waypoint.x * COORD_FACTOR) + (unit_texture.width() as f32 / 2.0);
                let wp_z = (waypoint.z * COORD_FACTOR) + (unit_texture.height() as f32 / 2.0);
                let wp_img_radius = (waypoint.r * COORD_FACTOR).log2() * 3.0;

                let wp_img = circle(wp_img_radius as u32, WAYPOINT_COLOR.into());
                overlay(&mut unit_texture, &wp_img, wp_x as i64 - (wp_img.width() / 2) as i64, wp_z as i64 - (wp_img.height() / 2) as i64);

                for link in waypoint.links.iter() {
                    let dest_wp = unit.waypoints.iter().find(|wp| wp.index == *link).unwrap();
                    let dest_x = (dest_wp.x * COORD_FACTOR) + (unit_texture.width() as f32 / 2.0);
                    let dest_z = (dest_wp.z * COORD_FACTOR) + (unit_texture.height() as f32 / 2.0);
                    draw_arrow_line(&mut unit_texture, dest_x, dest_z, wp_x, wp_z, CARRY_PATH_COLOR.into());

                    if options.draw_waypoint_distances {
                        let distance_text = render_small_text(
                            &format!("{}",waypoint.dist(dest_wp) as u32 / 10),
                            10.0, WAYPOINT_DIST_TXT_COLOR.into()
                        );
                        overlay(
                            &mut unit_texture,
                            &distance_text,
                            (wp_x - (wp_x - dest_x) / 2.0) as i64 - (distance_text.width() / 2) as i64,
                            (wp_z - (wp_z - dest_z) / 2.0) as i64  - (distance_text.height() / 2) as i64
                        )
                    }
                }
            }
        }

        for spawnpoint in unit.spawnpoints.iter().sorted_by_key(|sp| sp.group) {
            let sp_x = (spawnpoint.pos_x * COORD_FACTOR) as i64 + (unit_texture.width() / 2) as i64;
            let sp_z = (spawnpoint.pos_z * COORD_FACTOR) as i64 + (unit_texture.height() / 2) as i64;

            let sp_img = match spawnpoint.group {
                0 => circle((spawnpoint.radius * COORD_FACTOR) as u32, group_color(0).into()),
                1 => circle(5, group_color(1).into()),
                2 => colorize(resize(AssetManager::get_img("resources/enemytex_special/duck.png").change_context(CaveripperError::RenderingError)?, 14, 14, FilterType::Lanczos3), group_color(2).into()), // treasure
                4 => resize(AssetManager::get_img("resources/enemytex_special/cave_white.png").change_context(CaveripperError::RenderingError)?, 18, 18, FilterType::Lanczos3),
                6 => colorize(resize(AssetManager::get_img("resources/enemytex_special/leaf_icon.png").change_context(CaveripperError::RenderingError)?, 10, 10, FilterType::Lanczos3), group_color(6).into()),
                7 => resize(AssetManager::get_img("resources/enemytex_special/ship.png").change_context(CaveripperError::RenderingError)?, 16, 16, FilterType::Lanczos3),
                8 => colorize(resize(AssetManager::get_img("resources/enemytex_special/star.png").change_context(CaveripperError::RenderingError)?, 16, 16, FilterType::Lanczos3), group_color(8).into()),
                _ => circle(5, [255,0,0,255].into()),
            };

            overlay(&mut unit_texture, &sp_img, sp_x - (sp_img.width() / 2) as i64 , sp_z - (sp_img.height() / 2) as i64);
        }

        overlay(&mut canvas_maptiles, &unit_texture, base_x, base_y);
        draw_border(&mut canvas_maptiles, base_x as u32 - 1, base_y as u32 - 1, base_x as u32 + unit_texture.width(), base_y as u32 + unit_texture.height());
        overlay(&mut canvas_maptiles, &unit_name_text, base_x, base_y + unit_texture.height() as i64);

        max_y = max(max_y, base_y + unit_texture.height() as i64);
        base_x += unit_texture.width() as i64 + maptile_margin;
    }


    // Combine sections
    let header_height = canvas_header.height() as i64;
    expand_canvas(&mut canvas_header, 0, canvas_maptiles.height(), None);
    overlay(&mut canvas_header, &canvas_maptiles, 0, header_height);

    Ok(canvas_header)
}

/// Saves a layout image to disc.
/// Filename must end with a `.png` extension.
pub fn save_image<P: AsRef<Path>>(img: &RgbaImage, filename: P) -> Result<(), CaveripperError> {
    img.save_with_format(&filename, image::ImageFormat::Png)
        .into_report().change_context(CaveripperError::RenderingError)?;
    Ok(())
}

fn colorize(mut img: RgbaImage, color: Rgba<u8>) -> RgbaImage {
    img.enumerate_pixels_mut().for_each(|px| {
        px.2.0[0] = color.0[0];
        px.2.0[1] = color.0[1];
        px.2.0[2] = color.0[2];
    });
    img
}

fn expand_canvas(canvas: &mut RgbaImage, w: u32, h: u32, fill_color: Option<Rgba<u8>>) {
    let mut new_canvas = RgbaImage::from_pixel(canvas.width() + w, canvas.height() + h, fill_color.unwrap_or_else(|| [0,0,0,0].into()));
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

fn draw_arrow_line(canvas: &mut RgbaImage, mut x1: f32, mut y1: f32, mut x2: f32, mut y2: f32, color: Rgba<u8>) {
    let steep = (y2 - y1).abs() > (x2 - x1).abs();
    if (steep && y1 > y2) || (!steep && x1 > x2) {
        (x1, x2) = (x2, x1);
        (y1, y2) = (y2, y1);
    }
    // Shorten the line slightly to make room for the arrows at the end
    if steep {
        let slope = (x2 - x1) / (y2 - y1);
        y1 += slope.cos() * 6.0;
        y2 -= slope.cos() * 6.0;
        x1 += slope.sin() * 6.0;
        x2 -= slope.sin() * 6.0;

        // Draw an arrow at each end
        draw_line(canvas, x2 - (slope + PI / 8.0).sin() * 8.0, y2 - (slope + PI / 8.0).cos() * 8.0, x2, y2, color);
        draw_line(canvas, x2 - (slope - PI / 8.0).sin() * 8.0, y2 - (slope - PI / 8.0).cos() * 8.0, x2, y2, color);
        draw_line(canvas, x1 + (slope + PI / 8.0).sin() * 8.0, y1 + (slope + PI / 8.0).cos() * 8.0, x1, y1, color);
        draw_line(canvas, x1 + (slope - PI / 8.0).sin() * 8.0, y1 + (slope - PI / 8.0).cos() * 8.0, x1, y1, color);
    }
    else {
        let slope = (y2 - y1) / (x2 - x1);
        x1 += slope.cos() * 6.0;
        x2 -= slope.cos() * 6.0;
        y1 += slope.sin() * 6.0;
        y2 -= slope.sin() * 6.0;

        // Draw an arrow at each end
        draw_line(canvas, x2 - (slope + PI / 8.0).cos() * 8.0, y2 - (slope + PI / 8.0).sin() * 8.0, x2, y2, color);
        draw_line(canvas, x2 - (slope - PI / 8.0).cos() * 8.0, y2 - (slope - PI / 8.0).sin() * 8.0, x2, y2, color);
        draw_line(canvas, x1 + (slope + PI / 8.0).cos() * 8.0, y1 + (slope + PI / 8.0).sin() * 8.0, x1, y1, color);
        draw_line(canvas, x1 + (slope - PI / 8.0).cos() * 8.0, y1 + (slope - PI / 8.0).sin() * 8.0, x1, y1, color);
    }

    // Draw main line
    draw_line(canvas, x1, y1, x2, y2, color);
}

fn draw_line(canvas: &mut RgbaImage, mut x1: f32, mut y1: f32, mut x2: f32, mut y2: f32, color: Rgba<u8>) {
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
    }
    else {
        let slope = (y2 - y1) / (x2 - x1);

        for x in (x1.round() as u32)..(x2.round() as u32) {
            let true_x = x as f32 + 0.5;
            let true_y = y1 + (slope * (true_x - x1));
            try_blend(canvas, true_x.round() as i64, true_y.round() as i64, color);
        }
    }
}

fn draw_ring(canvas: &mut RgbaImage, x: f32, y: f32, r: f32, color: Rgba<u8>) {
    for i in 0..=(r as i32) {
        let offset = i as f32;
        let height = (r.powi(2) - offset.powi(2)).sqrt();
        try_blend(canvas, (x - offset) as i64, (y + height) as i64, color);
        try_blend(canvas, (x - offset) as i64, (y - height) as i64, color);
        try_blend(canvas, (x + offset) as i64, (y + height) as i64, color);
        try_blend(canvas, (x + offset) as i64, (y - height) as i64, color);
        try_blend(canvas, (x - height) as i64, (y + offset) as i64, color);
        try_blend(canvas, (x - height) as i64, (y - offset) as i64, color);
        try_blend(canvas, (x + height) as i64, (y + offset) as i64, color);
        try_blend(canvas, (x + height) as i64, (y - offset) as i64, color);
    }
}

/// Blends the pixel at the given coordinates, if they are in bounds. Otherwise
/// does nothing.
fn try_blend(canvas: &mut RgbaImage, x: i64, y: i64, color: Rgba<u8>) {
    if x > 0 && y > 0 && x < canvas.width() as i64 && y < canvas.height() as i64 {
        canvas.get_pixel_mut(x as u32, y as u32).blend(&color);
    }
}

// x and z are world coordinates, not image or map unit coordinates
fn draw_object_at<Tex: Textured>(image_buffer: &mut RgbaImage, obj: &Tex, x: f32, z: f32, game: &str, options: &LayoutRenderOptions) -> Result<(), CaveripperError> {
    let mut texture = Cow::Borrowed(obj.get_texture(game)?);

    // Modifiers to be applied before ('under') the main texture, or to the texture itself
    for modifier in obj.get_texture_modifiers().iter() {
        match modifier {
            TextureModifier::QuickGlance(color) if options.quickglance => {
                let circle_size = BASE_TEKI_SIZE as f32 * QUICKGLANCE_CIRCLE_SCALE / 2.0;
                let circle_tex = circle(circle_size as u32, *color);
                overlay(
                    image_buffer,
                    &circle_tex,
                    ((x * COORD_FACTOR) - circle_size) as i64,
                    ((z * COORD_FACTOR) - circle_size) as i64
                );
            },
            TextureModifier::Scale(xsize, zsize) => {
                *texture.to_mut() = resize(&*texture, *xsize, *zsize, FilterType::Lanczos3);
            },
            _ => {}
        }
    }

    let img_x = ((x * COORD_FACTOR) - (texture.width() as f32 / 2.0)) as i64;
    let img_z = ((z * COORD_FACTOR) - (texture.height() as f32 / 2.0)) as i64;

    // Draw the main texture
    overlay(image_buffer, &*texture, img_x, img_z);

    // Modifiers to be applied after ('above') the main texture
    for modifier in obj.get_texture_modifiers().iter() {
        match modifier {
            TextureModifier::Falling => {
                let falling_icon_texture = resize(
                    AssetManager::get_img("resources/enemytex_special/falling_icon.png")?,
                    18, 18, FilterType::Lanczos3
                );
                overlay(image_buffer, &falling_icon_texture, img_x - 5, img_z);
            },
            TextureModifier::Carrying(carrying) => {
                let carried_treasure_icon = resize(
                    AssetManager::get_img(&PathBuf::from(game).join("treasures").join(format!("{carrying}.png")))?,
                    24, 24, FilterType::Lanczos3
                );
                overlay(image_buffer, &carried_treasure_icon, img_x + 15, img_z + 15);
            },
            TextureModifier::GaugeRing if options.draw_gauge_range => {
                let radius1 = 775.0 * COORD_FACTOR;  // Radius at which the gauge needle starts to go up
                let radius2 = 450.0 * COORD_FACTOR;  // Radius at which you start to get audible pings
                draw_ring(image_buffer, x * COORD_FACTOR, z * COORD_FACTOR, radius1, [210,0,240,120].into());
                draw_ring(image_buffer, x * COORD_FACTOR, z * COORD_FACTOR, radius2, [210,0,120,120].into());
            },
            _ => {}
        }
    }

    Ok(())
}

static BALOO_CHETTAN_SEMIBOLD: Lazy<Font> = Lazy::new(|| {
    let font_bytes = AssetManager::get_bytes("resources/BalooChettan2-SemiBold.ttf").expect("Missing font file!");
    Font::from_bytes(font_bytes.as_slice(), FontSettings::default()).expect("Failed to create font!")
});
static BALOO_CHETTAN_EXTRABOLD: Lazy<Font> = Lazy::new(|| {
    let font_bytes = AssetManager::get_bytes("resources/BalooChettan2-ExtraBold.ttf").expect("Missing font file!");
    Font::from_bytes(font_bytes.as_slice(), FontSettings::default()).expect("Failed to create font!")
});

fn render_text(text: &str, size: f32, color: Rgba<u8>, max_width: Option<u32>) -> RgbaImage {
    let mut layout = FontLayout::new(fontdue::layout::CoordinateSystem::PositiveYDown);
    layout.reset(&LayoutSettings {
        x: 0f32,
        y: 0f32,
        max_width: max_width.map(|w| w as f32),
        max_height: None,
        horizontal_align: HorizontalAlign::Left,
        vertical_align: VerticalAlign::Top,
        wrap_style: WrapStyle::Letter,
        wrap_hard_breaks: true,
    });
    layout.append(&[&*BALOO_CHETTAN_SEMIBOLD], &TextStyle::new(text, size, 0));
    let width = layout.glyphs().iter().map(|g| g.x as usize + g.width).max().unwrap_or(0);
    let mut img = RgbaImage::new(width as u32, layout.height() as u32);

    for glyph in layout.glyphs().iter() {
        let (metrics, bitmap) = BALOO_CHETTAN_SEMIBOLD.rasterize_config_subpixel(glyph.key);
        for (i, (cr, cg, cb)) in bitmap.into_iter().tuples().enumerate() {
            let x = (i % metrics.width) as i64 + glyph.x as i64;
            let y = (i / metrics.width) as i64 + glyph.y as i64;
            if x >= 0 && x < img.width() as i64 && y >= 0 && y < img.height() as i64 {
                let coverage = (cr as f32 + cg as f32 + cb as f32) / 3.0;
                img.put_pixel(
                    x as u32,
                    y as u32,
                    [
                        color.0[0].saturating_add(255-cr),
                        color.0[1].saturating_add(255-cg),
                        color.0[2].saturating_add(255-cb),
                        coverage as u8
                    ].into()
                );
            }
        }
    }
    img
}

/// Renders text with settings more suited for very small font sizes
/// (No subpixel rendering, bolder font)
fn render_small_text(text: &str, size: f32, color: Rgba<u8>) -> RgbaImage {
    let mut layout = FontLayout::new(fontdue::layout::CoordinateSystem::PositiveYDown);
    layout.reset(&LayoutSettings {
        x: 0f32,
        y: 0f32,
        max_width: None,
        max_height: None,
        horizontal_align: HorizontalAlign::Left,
        vertical_align: VerticalAlign::Top,
        wrap_style: WrapStyle::Letter,
        wrap_hard_breaks: true,
    });
    layout.append(&[&*BALOO_CHETTAN_EXTRABOLD], &TextStyle::new(text, size, 0));
    let width = layout.glyphs().iter().map(|g| g.x as usize + g.width).max().unwrap_or(0);
    let mut img = RgbaImage::new(width as u32, layout.height() as u32);

    for glyph in layout.glyphs().iter() {
        let (metrics, bitmap) = BALOO_CHETTAN_EXTRABOLD.rasterize_config(glyph.key);
        for (i, v) in bitmap.into_iter().enumerate() {
            let x = (i % metrics.width) as i64 + glyph.x as i64;
            let y = (i / metrics.width) as i64 + glyph.y as i64;
            if x >= 0 && x < img.width() as i64 && y >= 0 && y < img.height() as i64 {
                img.put_pixel(
                    x as u32,
                    y as u32,
                    [
                        color.0[0].saturating_add(255-v),
                        color.0[1].saturating_add(255-v),
                        color.0[2].saturating_add(255-v),
                        v
                    ].into()
                );
            }
        }
    }
    img
}

fn circle(radius: u32, color: Rgba<u8>) -> RgbaImage {
    let mut buffer = RgbaImage::new(radius*2, radius*2);
    for x in 0..radius*2 {
        for z in 0..radius*2 {
            let r = radius as f32;
            if ((r - x as f32).powi(2) + (r - z as f32).powi(2)).sqrt() < r {
                buffer.put_pixel(x, z, color);
            }
        }
    }
    buffer
}

enum TextureModifier {
    Scale(u32, u32),
    Falling,
    Carrying(String),
    QuickGlance(Rgba<u8>),
    GaugeRing,
}

trait Textured {
    fn get_texture(&self, game: &str) -> Result<&RgbaImage, CaveripperError>;
    fn get_texture_modifiers(&self) -> Vec<TextureModifier>;
}

impl<T: Textured> Textured for &T {
    fn get_texture(&self, game: &str) -> Result<&RgbaImage, CaveripperError> {
        (*self).get_texture(game)
    }
    fn get_texture_modifiers(&self) -> Vec<TextureModifier> {
        (*self).get_texture_modifiers()
    }
}

impl Textured for PlacedMapUnit<'_> {
    fn get_texture(&self, game: &str) -> Result<&RgbaImage, CaveripperError> {
        self.unit.get_texture(game)
    }

    fn get_texture_modifiers(&self) -> Vec<TextureModifier> {
        self.unit.get_texture_modifiers()
    }
}

impl Textured for TekiInfo {
    fn get_texture(&self, game: &str) -> Result<&RgbaImage, CaveripperError> {
        match get_special_texture_name(&self.internal_name) {
            Some(special_name) => {
                let filename = format!("resources/enemytex_special/{special_name}");
                AssetManager::get_img(filename)
            },
            None => {
                let filename = PathBuf::from(game).join("teki").join(format!("{}.png", self.internal_name.to_ascii_lowercase()));
                AssetManager::get_img(filename)
            }
        }
    }

    fn get_texture_modifiers(&self) -> Vec<TextureModifier> {
        let mut modifiers = Vec::new();
        if self.spawn_method.is_some() {
            modifiers.push(TextureModifier::Falling);
        }
        if let Some(carrying) = self.carrying.as_ref() {
            modifiers.push(TextureModifier::Carrying(carrying.internal_name.clone()));
            modifiers.push(TextureModifier::QuickGlance(QUICKGLANCE_TREASURE_COLOR.into()));
            modifiers.push(TextureModifier::GaugeRing);
        }
        match self.internal_name.to_ascii_lowercase().as_str() {
            "blackpom" /* Violet Candypop */ => modifiers.push(TextureModifier::QuickGlance(QUICKGLANCE_VIOLET_CANDYPOP_COLOR.into())),
            "whitepom" /* Ivory Candypop */ => modifiers.push(TextureModifier::QuickGlance(QUICKGLANCE_IVORY_CANDYPOP_COLOR.into())),
            "minihoudai" /* Groink */ => modifiers.push(TextureModifier::QuickGlance(QUICKGLANCE_ROAMING_COLOR.into())),
            _ => {}
        }
        modifiers.push(TextureModifier::Scale(BASE_TEKI_SIZE, BASE_TEKI_SIZE));
        modifiers
    }
}

impl Textured for CapInfo {
    fn get_texture(&self, game: &str) -> Result<&RgbaImage, CaveripperError> {
        // We don't consider the possibility of treasures spawning in CapInfo here since that
        // is never done in the vanilla game. May need to fix in the future for romhack support.
        match get_special_texture_name(&self.internal_name) {
            Some(special_name) => {
                let filename = format!("resources/enemytex_special/{special_name}");
                AssetManager::get_img(filename)
            },
            None => {
                let filename = PathBuf::from(game).join("teki").join(format!("{}.png", self.internal_name.to_ascii_lowercase()));
                AssetManager::get_img(filename)
            }
        }
    }

    fn get_texture_modifiers(&self) -> Vec<TextureModifier> {
        let mut modifiers = Vec::new();
        if self.is_falling() {
            modifiers.push(TextureModifier::Falling);
            modifiers.push(TextureModifier::Scale(FALLING_CAP_TEKI_SIZE, FALLING_CAP_TEKI_SIZE));
        }
        else {
            modifiers.push(TextureModifier::Scale(BASE_TEKI_SIZE, BASE_TEKI_SIZE));
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
    fn get_texture(&self, game: &str) -> Result<&RgbaImage, CaveripperError> {
        // TODO: fix US region being hardcoded here.
        let filename = PathBuf::from(game).join("treasures").join(format!("{}.png", self.internal_name.to_ascii_lowercase()));
        AssetManager::get_img(filename)
    }

    fn get_texture_modifiers(&self) -> Vec<TextureModifier> {
        vec![
            TextureModifier::QuickGlance(QUICKGLANCE_TREASURE_COLOR.into()),
            TextureModifier::Scale(TREASURE_SIZE, TREASURE_SIZE),
            TextureModifier::GaugeRing,
        ]
    }
}

impl Textured for GateInfo {
    fn get_texture(&self, _game: &str) -> Result<&RgbaImage, CaveripperError> {
        let filename = "resources/enemytex_special/Gray_bramble_gate_icon.png";
        AssetManager::get_img(filename)
    }

    fn get_texture_modifiers(&self) -> Vec<TextureModifier> {
        vec![TextureModifier::Scale(GATE_SIZE, GATE_SIZE)]
        // TODO: gate hp modifier
    }
}

impl Textured for SpawnObject<'_> {
    fn get_texture(&self, game: &str) -> Result<&RgbaImage, CaveripperError> {
        match self {
            SpawnObject::Teki(tekiinfo, _) => tekiinfo.get_texture(game),
            SpawnObject::CapTeki(capinfo, _) => capinfo.get_texture(game),
            SpawnObject::Item(iteminfo) => iteminfo.get_texture(game),
            SpawnObject::Gate(gateinfo) => gateinfo.get_texture(game),
            SpawnObject::Hole(plugged) => {
                AssetManager::get_or_store_img("PLUGGED_HOLE".to_string(), || {
                    let filename = "resources/enemytex_special/Cave_icon.png";
                    let mut hole_icon = AssetManager::get_img(filename)?.clone();
                    if *plugged {
                        let plug_filename = "resources/enemytex_special/36px-Clog_icon.png";
                        let plug_icon = resize(
                            AssetManager::get_img(plug_filename)?,
                            hole_icon.width(),
                            hole_icon.height(),
                            FilterType::Lanczos3,
                        );
                        overlay(&mut hole_icon, &plug_icon, 0, 0);
                    }

                    Ok(hole_icon)
                })
            },
            SpawnObject::Geyser(plugged) => {
                AssetManager::get_or_store_img("PLUGGED_GEYSER".to_string(), || {
                    let filename = "resources/enemytex_special/Geyser_icon.png";
                    let mut hole_icon = AssetManager::get_img(filename)?.clone();
                    if *plugged {
                        let plug_filename = "resources/enemytex_special/36px-Clog_icon.png";
                        let plug_icon = resize(
                            AssetManager::get_img(plug_filename)?,
                            hole_icon.width(),
                            hole_icon.height(),
                            FilterType::Lanczos3,
                        );
                        overlay(&mut hole_icon, &plug_icon, 0, 0);
                    }
                    Ok(hole_icon)
                })
            },
            SpawnObject::Ship => {
                let filename = "resources/enemytex_special/pod_icon.png";
                AssetManager::get_img(filename)
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
            },
            SpawnObject::Ship => {
                vec![TextureModifier::QuickGlance(QUICKGLANCE_SHIP_COLOR.into())]
            },
            SpawnObject::Gate(gateinfo) => gateinfo.get_texture_modifiers(),
        }
    }
}

impl Textured for CaveUnit {
    fn get_texture(&self, game: &str) -> Result<&RgbaImage, CaveripperError> {
        let filename = PathBuf::from(game).join("mapunits").join(&self.unit_folder_name).join("arc/texture.png");
        let mut img = AssetManager::get_img(&filename)?.to_owned();

        // Radar images are somewhat dark by default; this improves visibility.
        brighten_in_place(&mut img, 75);

        for _ in 0..self.rotation {
            img = rotate90(&img);
        }

        img = resize(
            &img,
            (self.width * 8) as u32 * RENDER_SCALE,
            (self.height * 8) as u32 * RENDER_SCALE,
            FilterType::Nearest
        );

        for waterbox in self.waterboxes.iter() {
            let (x1, z1, x2, z2) = match self.rotation {
                0 => (waterbox.x1, waterbox.z1, waterbox.x2, waterbox.z2),
                1 => (-waterbox.z2, waterbox.x1, -waterbox.x1, waterbox.x2),
                2 => (-waterbox.x2, -waterbox.z2, -waterbox.x1, -waterbox.z1),
                3 => (waterbox.z1, -waterbox.x2, waterbox.z2, -waterbox.x1),
                _ => panic!("Invalid rotation"),
            };
            let x1 = x1 * COORD_FACTOR;
            let z1 = z1 * COORD_FACTOR;
            let x2 = x2 * COORD_FACTOR;
            let z2 = z2 * COORD_FACTOR;
            let w = (self.width as i64 * GRID_FACTOR) as f32 / 2.0;
            let h = (self.height as i64 * GRID_FACTOR) as f32 / 2.0;
            let square = RgbaImage::from_pixel((x2 - x1) as u32, (z2 - z1) as u32, [0, 100, 230, 50].into());
            overlay(&mut img, &square, (x1 + w) as i64, (z1 + h) as i64);
        }

        AssetManager::get_or_store_img(format!("{}_r{}_prerendered", filename.to_string_lossy(), self.rotation), Box::new(|| Ok(img)))
    }

    fn get_texture_modifiers(&self) -> Vec<TextureModifier> {
        Vec::new()
    }
}


struct WithCustomTexture<T: Textured> {
    pub inner: T,
    pub custom_texture: RgbaImage,
}

impl<T: Textured> Textured for WithCustomTexture<T> {
    fn get_texture(&self, _game: &str) -> Result<&RgbaImage, CaveripperError> {
        Ok(&self.custom_texture)
    }

    fn get_texture_modifiers(&self) -> Vec<TextureModifier> {
        self.inner.get_texture_modifiers()
    }
}
