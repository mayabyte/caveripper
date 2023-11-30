use std::borrow::Cow;

use clap::Args;
use image::{imageops::FilterType, Rgba, RgbaImage};

use super::{
    coords::{Offset, Origin},
    render_spawn_object,
    renderer::{Layer, Render, StickerRenderer},
    shapes::Circle,
    text::Text,
    util::{with_border, Colorize, Crop, Resize, Rows},
    Icon, RenderHelper, CAVEINFO_BOXES_FONT_SIZE, CAVEINFO_ICON_SIZE, CAVEINFO_MARGIN, CAVEINFO_UNIT_BORDER_COLOR, CAVEINFO_UNIT_MARGIN,
    CAVEINFO_WIDTH, GRID_FACTOR, HEADER_BACKGROUND, MAPTILES_BACKGROUND, OFF_BLACK,
};
use crate::{
    caveinfo::{CaveInfo, CaveUnit, ItemInfo, RoomType, TekiInfo},
    errors::CaveripperError,
    layout::SpawnObject,
    point::Point,
};

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

pub fn render_caveinfo(caveinfo: &CaveInfo, helper: &RenderHelper, options: CaveinfoRenderOptions) -> Result<RgbaImage, CaveripperError> {
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
            helper.cropped_text(caveinfo.long_name(), 88.0, 0, OFF_BLACK),
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
                helper.cropped_text(format!("{}HP", gateinfo.health.round() as u32), 13.0, 0, OFF_BLACK),
                Origin::TopCenter,
                Offset {
                    from: Origin::BottomCenter,
                    amount: Point([0.0, 0.0]),
                },
            )
            .place_relative(
                helper.cropped_text(format!("x{}", caveinfo.max_gates), 19.0, 0, OFF_BLACK),
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
    let mut spawn_object_info_boxes = Rows::new(CAVEINFO_WIDTH, CAVEINFO_MARGIN, CAVEINFO_MARGIN);

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

        spawn_object_info_boxes.add(caveinfo_entity_box(
            name,
            icon,
            group_color(group_num),
            teki.map(|info| SpawnObject::Teki(info, Point([0.0, 0.0, 0.0]))),
            group_score(group_num),
            &caveinfo.cave_cfg.game,
            helper,
        ));
    }

    // Cap teki
    if caveinfo.cap_info.len() > 0 {
        spawn_object_info_boxes.add(caveinfo_entity_box(
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
            helper,
        ));
    }

    // Treasures
    if caveinfo.item_info.len() > 0 {
        spawn_object_info_boxes.add(caveinfo_entity_box(
            "Treasure",
            Icon::Treasure,
            group_color(2),
            caveinfo.item_info.iter().map(|info| SpawnObject::Item(info)),
            0,
            &caveinfo.cave_cfg.game,
            helper,
        ));
    }

    let mut spawn_object_layer = Layer::of(spawn_object_info_boxes);
    spawn_object_layer.set_margin(CAVEINFO_MARGIN);

    renderer.place_relative(
        spawn_object_layer,
        Origin::TopLeft,
        Offset {
            from: Origin::BottomLeft,
            amount: Point([0.0, 0.0]),
        },
    );

    // -- Cave Units -- //
    // Caps and 1x1 halls
    let mut cap_and_hall_box = Rows::new(
        GRID_FACTOR * 0.75 * 3.2 + (CAVEINFO_UNIT_MARGIN * 2.0),
        CAVEINFO_UNIT_MARGIN,
        CAVEINFO_UNIT_MARGIN,
    );
    let caps_and_1x1_halls = caveinfo
        .cave_units
        .iter()
        .filter(|unit| unit.rotation == 0 && unit.room_type != RoomType::Room && unit.height == 1);
    for unit in caps_and_1x1_halls {
        cap_and_hall_box.add(render_unit_caveinfo(unit, helper));
    }

    // Rooms and larger hall units
    let mut unit_box = Rows::new(CAVEINFO_WIDTH, CAVEINFO_UNIT_MARGIN, CAVEINFO_UNIT_MARGIN);
    unit_box.add(cap_and_hall_box);

    let larger_units = caveinfo
        .cave_units
        .iter()
        .filter(|unit| unit.rotation == 0 && (unit.room_type == RoomType::Room || unit.room_type == RoomType::Hallway && unit.height > 1));
    for unit in larger_units {
        unit_box.add(render_unit_caveinfo(unit, helper));
    }

    let mut unit_layer = Layer::of(unit_box);
    unit_layer.set_background_color(MAPTILES_BACKGROUND);
    unit_layer.set_margin(CAVEINFO_UNIT_MARGIN);

    renderer.place_relative(
        unit_layer,
        Origin::TopLeft,
        Offset {
            from: Origin::BottomLeft,
            amount: Point([0.0, 0.0]),
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
    Ok(renderer.render(helper.mgr))
}

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

fn caveinfo_entity_box<'r, 'h: 'r>(
    title: impl Into<String>,
    icon: impl Render + 'r,
    color: impl Into<Rgba<u8>>,
    spawn_objects: impl Iterator<Item = SpawnObject<'r>>,
    score: u32,
    game: &str,
    helper: &'h RenderHelper,
) -> Layer<'r> {
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
        helper.cropped_text(title, CAVEINFO_BOXES_FONT_SIZE, 0, OFF_BLACK),
        Origin::TopLeft,
        Offset {
            from: Origin::TopRight,
            amount: Point([CAVEINFO_MARGIN / 2.0, 0.0]),
        },
    );

    if score > 0 {
        header_placement.place_relative(
            helper.cropped_text(format!("Score: {score}"), 20.0, 0, OFF_BLACK),
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
            let treasure_metadata = helper
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
                    helper.cropped_text(format!("{}", treasure_metadata.value), 28.0, 0, OFF_BLACK),
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
                    helper.cropped_text(
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
                helper.cropped_text(num_str, 20.0, 0, color),
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

fn render_unit_caveinfo<'h, 'r: 'h>(unit: &'r CaveUnit, helper: &'h RenderHelper) -> impl Render + 'h {
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
            font: &helper.fonts[1],
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

    this_unit_layer
}
