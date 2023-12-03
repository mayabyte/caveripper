use std::borrow::Cow;

use clap::Args;
use image::{imageops::FilterType, Rgba, RgbaImage};
use itertools::Itertools;

use super::{
    coords::{Offset, Origin},
    render_spawn_object,
    renderer::{Layer, Render, StickerRenderer},
    shapes::Circle,
    text::Text,
    util::{with_border, Colorize, CropRelative, Resize, Rows},
    Icon, RenderHelper, CAVEINFO_BOXES_FONT_SIZE, CAVEINFO_ICON_SIZE, CAVEINFO_MARGIN, CAVEINFO_UNIT_BORDER_COLOR, CAVEINFO_UNIT_MARGIN,
    CAVEINFO_WIDTH, COORD_FACTOR, GRID_FACTOR, HEADER_BACKGROUND, MAPTILES_BACKGROUND, OFF_BLACK,
};
use crate::{
    caveinfo::{CaveInfo, CaveUnit, ItemInfo, RoomType, TekiInfo},
    errors::CaveripperError,
    layout::SpawnObject,
    point::Point,
    render::{coords::Bounds, shapes::Line, util::CropAbsolute, CARRY_PATH_COLOR, RENDER_SCALE, WAYPOINT_COLOR, WAYPOINT_DIST_TXT_COLOR},
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
    #[clap(long, default_value_t=false, action=clap::ArgAction::Set)]
    pub draw_waypoint_distances: bool,

    #[clap(long, default_value_t=false, action=clap::ArgAction::Set)]
    pub hide_small_units: bool,
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
                CropRelative {
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
                ..Default::default()
            }),
        ),
        ("Easy", 0, Box::new(easy_teki_rings(32.0))),
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

    let spawn_obj_layer_width = spawn_object_layer.dimensions()[0];
    renderer.place_relative(
        spawn_object_layer,
        Origin::TopLeft,
        Offset {
            from: Origin::BottomLeft,
            amount: Point([0.0, 0.0]),
        },
    );

    // -- Cave Units -- //
    let mut unit_box = Rows::new(CAVEINFO_WIDTH, CAVEINFO_UNIT_MARGIN, CAVEINFO_UNIT_MARGIN);

    // Caps and 1x1 halls
    if !options.hide_small_units {
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
            cap_and_hall_box.add(render_unit_caveinfo(unit, helper, &options));
        }

        unit_box.add(cap_and_hall_box);
    }

    // Rooms and larger hall units
    let larger_units = caveinfo
        .cave_units
        .iter()
        .filter(|unit| unit.rotation == 0 && (unit.room_type == RoomType::Room || unit.room_type == RoomType::Hallway && unit.height > 1));
    for unit in larger_units {
        if unit.room_type != RoomType::Room && options.hide_small_units {
            continue;
        }
        unit_box.add(render_unit_caveinfo(unit, helper, &options));
    }

    let mut unit_layer = Layer::new();
    unit_layer.set_margin(CAVEINFO_UNIT_MARGIN);

    // Unit metadata text
    let mut text = format!(
        "Num Rooms: {}      CorridorBetweenRoomsProb: {}%      CapOpenDoorsProb: {}%",
        caveinfo.num_rooms,
        caveinfo.corridor_probability * 100.0,
        caveinfo.cap_probability * 100.0
    );
    // If we're not showing alcoves, we need to indicate whether there are caps and/or non-item caps
    // because it's important for score reading.
    if options.hide_small_units {
        let has_item_caps = caveinfo
            .cave_units
            .iter()
            .find(|unit| unit.unit_folder_name.starts_with("item"))
            .is_some();
        let has_non_item_caps = caveinfo
            .cave_units
            .iter()
            .find(|unit| unit.unit_folder_name.starts_with("cap"))
            .is_some();
        text += &format!(
            "\nItem Alcoves: {}      Non-Item Caps: {}",
            has_item_caps.to_string(),
            has_non_item_caps.to_string()
        );
    }
    let unit_metadata_text = helper.cropped_text(text, 24.0, 0, HEADER_BACKGROUND);
    unit_layer.place(unit_metadata_text, Point([0.0, 0.0]), Origin::TopLeft);

    unit_layer.place_relative(
        unit_box,
        Origin::TopLeft,
        Offset {
            from: Origin::BottomLeft,
            amount: Point([0.0, CAVEINFO_UNIT_MARGIN]),
        },
    );

    // Make sure the background extends all the way to the right of the image
    let unit_layer_width = unit_layer.dimensions()[0];
    if unit_layer_width < spawn_obj_layer_width {
        // Negative crop = expanding the bounds
        unit_layer = Layer::of(CropRelative {
            inner: unit_layer,
            right: unit_layer_width - spawn_obj_layer_width,
            left: 0.0,
            top: 0.0,
            bottom: 0.0,
        });
    }
    unit_layer.set_background_color(MAPTILES_BACKGROUND);

    renderer.place_relative(
        unit_layer,
        Origin::TopLeft,
        Offset {
            from: Origin::BottomLeft,
            amount: Point([0.0, 0.0]),
        },
    );

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
                helper.cropped_text(num_str, 24.0, 0, color),
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

fn render_unit_caveinfo<'h, 'r: 'h>(unit: &'r CaveUnit, helper: &'h RenderHelper, options: &CaveinfoRenderOptions) -> impl Render + 'h {
    const CAVEINFO_GRID_FACTOR: f32 = GRID_FACTOR * 0.75;
    const CAVEINFO_COORD_FACTOR: f32 = COORD_FACTOR * 0.75;

    let mut unit_layer = Layer::new();
    unit_layer.place(
        Resize::new(
            unit,
            unit.width as f32 * CAVEINFO_GRID_FACTOR,
            unit.height as f32 * CAVEINFO_GRID_FACTOR,
            FilterType::Nearest,
        ),
        Point([0.0, 0.0]),
        Origin::TopLeft,
    );

    // Waypoints
    let mut waypoint_layer = Layer::new();
    let mut waypoint_arrow_layer = Layer::new();
    let mut waypoint_distance_layer = Layer::new();
    waypoint_layer.set_opacity(0.6);
    waypoint_arrow_layer.set_opacity(0.6);

    let offset = Point([unit.width as f32 * CAVEINFO_GRID_FACTOR, unit.height as f32 * CAVEINFO_GRID_FACTOR]) / 2.0;
    for wp in unit.waypoints.iter() {
        let wp_pos = wp.pos.two_d() * CAVEINFO_COORD_FACTOR + offset;
        waypoint_layer.place(
            Circle {
                radius: wp.r.log2() * 3.0, // just what looks good
                color: WAYPOINT_COLOR.into(),
                ..Default::default()
            },
            wp_pos,
            Origin::Center,
        );

        // Arrows for links
        for link in wp.links.iter() {
            let dest_wp = unit.waypoints.iter().find(|wp| wp.index == *link).unwrap();
            let dest_wp_pos = dest_wp.pos.two_d() * CAVEINFO_COORD_FACTOR + offset;
            waypoint_arrow_layer.place(
                Line {
                    start: wp_pos,
                    end: dest_wp_pos,
                    shorten_start: 6.0,
                    shorten_end: 6.0,
                    forward_arrow: true,
                    color: CARRY_PATH_COLOR.into(),
                    ..Default::default()
                },
                Point::zero(),
                Origin::TopLeft,
            );

            if options.draw_waypoint_distances {
                let distance_score = wp.pos.p2_dist(&dest_wp.pos) as u32 / 10;
                let midpoint = (wp_pos + dest_wp_pos) / 2.0;
                waypoint_distance_layer.place(
                    helper.cropped_text(distance_score.to_string(), 14.0, 0, WAYPOINT_DIST_TXT_COLOR),
                    midpoint,
                    Origin::Center,
                );
            }
        }
    }

    // WP layer's top-left is actually the center of the room because raw WP coords
    // are centered on the center of the unit
    unit_layer
        .place(waypoint_layer, Point::zero(), Origin::TopLeft)
        .place(waypoint_arrow_layer, Point::zero(), Origin::TopLeft)
        .place(waypoint_distance_layer, Point::zero(), Origin::TopLeft);

    // Spawn points
    for sp in unit.spawnpoints.iter().sorted_by_key(|sp| sp.group) {
        let icon: Box<dyn Render> = match sp.group {
            0 => Box::new(easy_teki_rings(sp.radius * CAVEINFO_COORD_FACTOR)),
            1 => Box::new(Circle {
                radius: RENDER_SCALE * 0.5,
                color: group_color(1).into(),
                ..Default::default()
            }),
            2 => Box::new(Colorize {
                renderable: Icon::Treasure,
                color: group_color(2).into(),
            }),
            4 => Box::new(Icon::Exit),
            6 => Box::new(Resize {
                renderable: Colorize {
                    renderable: Icon::Plant,
                    color: group_color(6).into(),
                },
                width: RENDER_SCALE,
                height: RENDER_SCALE,
                filter: FilterType::Lanczos3,
            }),
            7 => Box::new(Icon::Ship),
            8 => Box::new(Resize {
                renderable: Colorize {
                    renderable: Icon::Star,
                    color: group_color(8).into(),
                },
                width: RENDER_SCALE * 1.75,
                height: RENDER_SCALE * 1.75,
                filter: FilterType::Lanczos3,
            }),
            _ => Box::new(()),
        };
        unit_layer.place(icon, sp.pos.two_d() * CAVEINFO_COORD_FACTOR + offset, Origin::Center);
    }

    // Compose everything and add unit name text
    let mut layer = Layer::of(with_border(
        CropAbsolute {
            inner: unit_layer,
            bounds: Bounds {
                topleft: Point([0.0, 0.0]),
                bottomright: Point([unit.width as f32 * CAVEINFO_GRID_FACTOR, unit.height as f32 * CAVEINFO_GRID_FACTOR]),
            },
        },
        1.0,
        CAVEINFO_UNIT_BORDER_COLOR,
    ));
    layer.place_relative(
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

    layer
}

fn easy_teki_rings(r: f32) -> impl Render {
    let mut layer = Layer::new();
    let mut radius = r;
    for _ in 0..3 {
        layer.place(
            Circle {
                radius,
                border_thickness: 1.0,
                border_color: group_color(0).into(),
                ..Default::default()
            },
            Point::zero(),
            Origin::Center,
        );
        radius *= 0.85;
    }
    // Hack to shift the circles as if they were placed with Origin::TopLeft
    CropRelative {
        inner: layer,
        top: -r,
        left: -r,
        right: r,
        bottom: r,
    }
}
