use std::{borrow::Cow, cell::RefCell, cmp::max};

use clap::Args;
use image::{imageops::FilterType, RgbaImage};
use log::info;

use super::{util::Resize, RenderHelper};
use crate::{
    caveinfo::{CapInfo, TekiInfo},
    errors::CaveripperError,
    layout::{Layout, PlacedMapUnit, SpawnObject},
    point::Point,
    render::{
        coords::Origin,
        render_spawn_object,
        renderer::{Layer, StickerRenderer},
        shapes::{Circle, Line},
        CARRY_PATH_COLOR, COORD_FACTOR, DISTANCE_SCORE_TEXT_COLOR, GRID_COLOR, GRID_FACTOR, LAYOUT_BACKGROUND_COLOR,
        QUICKGLANCE_CIRCLE_RADIUS, QUICKGLANCE_EXIT_COLOR, QUICKGLANCE_IVORY_CANDYPOP_COLOR, QUICKGLANCE_ROAMING_COLOR,
        QUICKGLANCE_SHIP_COLOR, QUICKGLANCE_TREASURE_COLOR, QUICKGLANCE_VIOLET_CANDYPOP_COLOR, SCORE_TEXT_COLOR, WAYPOINT_COLOR,
    },
};

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

pub fn render_layout(layout: &Layout, helper: &RenderHelper, options: LayoutRenderOptions) -> Result<RgbaImage, CaveripperError> {
    info!("Drawing layout image...");

    let mut renderer = StickerRenderer::new();
    renderer.set_global_background_color(LAYOUT_BACKGROUND_COLOR);

    /* Map Units */
    let map_unit_layer = render_map_units(layout.map_units.iter());
    renderer.add_layer(map_unit_layer);

    /* Waypoints */
    if options.draw_waypoints {
        let mut waypoint_circle_layer = Layer::new();
        waypoint_circle_layer.set_opacity(0.6);
        for wp in layout.waypoint_graph().iter() {
            waypoint_circle_layer.place(
                Circle {
                    radius: wp.r * COORD_FACTOR / 1.7,
                    color: WAYPOINT_COLOR.into(),
                    ..Default::default()
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
                waypoint_arrow_layer.place(
                    Line {
                        start: (wp.pos * COORD_FACTOR).two_d(),
                        end: (backlink.pos * COORD_FACTOR).two_d(),
                        shorten_start: 6.0,
                        shorten_end: 6.0,
                        forward_arrow: true,
                        color: CARRY_PATH_COLOR.into(),
                        ..Default::default()
                    },
                    Point([0.0, 0.0]),
                    Origin::TopLeft,
                );
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
                        ..Default::default()
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
            grid_layer.place(
                Line {
                    start: Point([x as f32 * GRID_FACTOR, 0.0]),
                    end: Point([x as f32 * GRID_FACTOR, map_dims.1 as f32 * GRID_FACTOR]),
                    color: GRID_COLOR.into(),
                    ..Default::default()
                },
                Point::zero(),
                Origin::TopLeft,
            );
        }

        for y in 0..map_dims.1 {
            grid_layer.place(
                Line {
                    start: Point([0.0, y as f32 * GRID_FACTOR]),
                    end: Point([map_dims.0 as f32 * GRID_FACTOR, y as f32 * GRID_FACTOR]),
                    color: GRID_COLOR.into(),
                    ..Default::default()
                },
                Point::zero(),
                Origin::TopLeft,
            );
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
                helper.cropped_text(text, 24.0, 2, SCORE_TEXT_COLOR),
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
                    distance_score_line_layer.place(
                        Line {
                            start: this_door_pos.two_d() * COORD_FACTOR,
                            end: other_door_pos.two_d() * COORD_FACTOR,
                            shorten_start: 8.0,
                            shorten_end: 8.0,
                            color: DISTANCE_SCORE_TEXT_COLOR.into(),
                            ..Default::default()
                        },
                        Point::zero(),
                        Origin::TopLeft,
                    );

                    let midpoint = ((this_door_pos + other_door_pos) / 2.0) * COORD_FACTOR;
                    let distance_score = (link.distance / 10.0).round() as u32;
                    distance_score_text_layer.place(
                        helper.cropped_text(format!("{}", distance_score), 24.0, 2, DISTANCE_SCORE_TEXT_COLOR),
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

    Ok(renderer.render(helper.mgr))
}

/// Places map unit images for a layout
fn render_map_units<'a, 'l: 'a>(map_units: impl Iterator<Item = &'a PlacedMapUnit<'l>>) -> Layer<'a> {
    let mut radar_image_layer = Layer::new();

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
    }

    radar_image_layer
}
