use std::cmp::max;
use std::fs::read;

use crate::caveinfo::{CapInfo, GateInfo, ItemInfo, TekiInfo, CaveInfo, CaveUnit};
use crate::assets::{ASSETS, get_special_texture_name};
use crate::errors::{RenderError, AssetError};
use super::{Layout, SpawnObject, PlacedMapUnit};
use clap::Args;
use fontdue::layout::{Layout as FontLayout, TextStyle};
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
const QUICKGLANCE_TREASURE_COLOR: [u8; 4] = [245, 150, 0, 110];
const QUICKGLANCE_EXIT_COLOR: [u8; 4] = [10, 225, 100, 95];
const QUICKGLANCE_SHIP_COLOR: [u8; 4] = [255, 40, 40, 80];
const QUICKGLANCE_VIOLET_CANDYPOP_COLOR: [u8; 4] = [255, 0, 245, 80];
const QUICKGLANCE_IVORY_CANDYPOP_COLOR: [u8; 4] = [100, 100, 100, 120];
const QUICKGLANCE_ROAMING_COLOR: [u8; 4] = [200, 0, 130, 60];
const CAVEINFO_MARGIN: i64 = 4;
const CAVEINFO_ICON_SIZE: u32 = 48;


#[derive(Default, Debug, Args)]
pub struct RenderOptions {
    #[clap(long)]
    pub draw_grid: bool,

    #[clap(long, short='q')]
    pub quickglance: bool,
}


pub fn render_layout(layout: &Layout, options: RenderOptions) -> Result<RgbaImage, RenderError> {
    info!("Drawing layout image...");

    // Find the minimum and maximum map tile coordinates in the layout.
    let max_map_x = layout.map_units.iter().map(|unit| unit.x as i64 + unit.unit.width as i64).max()
        .ok_or_else(|| RenderError::InvalidLayout(layout.cave_name.to_string(), layout.starting_seed))?;
    let max_map_z = layout.map_units.iter().map(|unit| unit.z as i64 + unit.unit.height as i64).max()
        .ok_or_else(|| RenderError::InvalidLayout(layout.cave_name.to_string(), layout.starting_seed))?;

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
        let mut radar_image = map_unit.get_texture()?.clone();
        brighten_in_place(&mut radar_image, 45);
        
        for _ in 0..map_unit.unit.rotation {
            radar_image = rotate90(&radar_image);
        }

        let radar_image = resize(
            &radar_image,
            radar_image.width() * RENDER_SCALE, 
            radar_image.height() * RENDER_SCALE, 
            FilterType::Nearest
        );

        // Copy the pixels of the radar image to the buffer
        let img_x = map_unit.x as i64 * GRID_FACTOR;
        let img_z = map_unit.z as i64 * GRID_FACTOR;
        overlay(&mut canvas, &radar_image, img_x, img_z);

        for waterbox in map_unit.unit.waterboxes.iter() {
            let (x1, z1, x2, z2) = match map_unit.unit.rotation {
                0 => (waterbox.x1, waterbox.z1, waterbox.x2, waterbox.z2),
                1 => (-waterbox.z2, waterbox.x1, -waterbox.x1, waterbox.x2),
                2 => (-waterbox.x2, -waterbox.z2, -waterbox.x1, -waterbox.z1),
                3 => (waterbox.z1, -waterbox.x2, waterbox.z2, -waterbox.x1),
                _ => return Err(RenderError::InvalidLayout(layout.cave_name.to_string(), layout.starting_seed)),
            };
            let x1 = x1 * COORD_FACTOR;
            let z1 = z1 * COORD_FACTOR;
            let x2 = x2 * COORD_FACTOR;
            let z2 = z2 * COORD_FACTOR;
            let w = (map_unit.unit.width as i64 * GRID_FACTOR) as f32 / 2.0;
            let h = (map_unit.unit.height as i64 * GRID_FACTOR) as f32 / 2.0;
            let square = RgbaImage::from_pixel((x2 - x1) as u32, (z2 - z1) as u32, [0, 100, 230, 50].into());
            overlay(&mut canvas, &square, img_x as i64 + (x1 + w) as i64, img_z as i64 + (z1 + h) as i64);
        }
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
                    draw_object_at(&mut canvas, tekiinfo, spawnpoint.x + dx, spawnpoint.z + dz, &options)?;
                },
                SpawnObject::CapTeki(capinfo, _) if capinfo.is_falling() => {
                    draw_object_at(&mut canvas, capinfo, spawnpoint.x - 30.0, spawnpoint.z - 30.0, &options)?;
                },
                _ => {
                    draw_object_at(&mut canvas, spawn_object, spawnpoint.x, spawnpoint.z, &options)?;
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
                    let texture = gateinfo.get_texture()?;
                    if door.borrow().door_unit.direction % 2 == 1 {
                        draw_object_at(&mut canvas, &WithCustomTexture{ inner: gateinfo.clone(), custom_texture: rotate90(&texture) }, x, z, &options)?;
                    }
                    else {
                        draw_object_at(&mut canvas, gateinfo, x, z, &options)?;
                    }
                }
                _ => {
                    draw_object_at(&mut canvas, spawn_object, x, z, &options)?;
                },
            }
        }
    }

    Ok(canvas)
}

pub fn render_caveinfo(caveinfo: &CaveInfo, _options: RenderOptions) -> Result<RgbaImage, RenderError> {
    let mut canvas_header = RgbaImage::from_pixel(980, 310, [220,220,220,255].into());

    // Sublevel name
    let sublevel_title = render_text(&caveinfo.sublevel.as_ref().unwrap().long_name(), 64.0, [0,0,0]);
    overlay(&mut canvas_header, &sublevel_title, CAVEINFO_MARGIN * 2, -8);

    // Metadata icons - ship, hole plugged/unplugged, geyser yes/no, num gates
    let mut metadata_icons = Vec::new();
    metadata_icons.push(resize(&SpawnObject::Ship.get_texture()?, CAVEINFO_ICON_SIZE, CAVEINFO_ICON_SIZE, FilterType::Lanczos3));
    if !caveinfo.is_final_floor {
        metadata_icons.push(resize(&SpawnObject::Hole(caveinfo.exit_plugged).get_texture()?, CAVEINFO_ICON_SIZE, CAVEINFO_ICON_SIZE, FilterType::Lanczos3));
    }
    if caveinfo.is_final_floor || caveinfo.has_geyser {
        metadata_icons.push(resize(&SpawnObject::Geyser.get_texture()?, CAVEINFO_ICON_SIZE, CAVEINFO_ICON_SIZE, FilterType::Lanczos3));
    }
    let num_gates = caveinfo.max_gates;
    if num_gates > 0 {
        let gate_icon = resize(&SpawnObject::Gate(caveinfo.gate_info[0].clone()).get_texture()?, CAVEINFO_ICON_SIZE, CAVEINFO_ICON_SIZE, FilterType::Lanczos3);
        let num_txt = render_text(&format!("x{}", num_gates), 24.0, [20, 20, 20]);
        let mut final_gate_icon = RgbaImage::new(CAVEINFO_ICON_SIZE, CAVEINFO_ICON_SIZE);
        overlay(&mut final_gate_icon, &gate_icon, 0, -8);
        overlay(&mut final_gate_icon, &num_txt, CAVEINFO_ICON_SIZE as i64 / 2 - num_txt.width() as i64 / 2, CAVEINFO_ICON_SIZE as i64 - 32);
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

    let mut base_y =  64 + CAVEINFO_MARGIN * 2;

    // Teki section
    let teki_header = render_text(&format!("Teki (max {})", caveinfo.max_main_objects), 48.0, [225,0,0]);
    overlay(&mut canvas_header, &teki_header, CAVEINFO_MARGIN * 2, base_y);
    let mut i = 0;
    for group in [8, 1, 0, 6, 5] {
        for tekiinfo in caveinfo.teki_group(group) {
            let texture = resize(&tekiinfo.get_texture()?, CAVEINFO_ICON_SIZE, CAVEINFO_ICON_SIZE, FilterType::Lanczos3);
            let mut x = (CAVEINFO_MARGIN * 4) + teki_header.width() as i64 + i as i64 * (CAVEINFO_ICON_SIZE as i64 + CAVEINFO_MARGIN);
            let mut y = base_y + (64 - CAVEINFO_ICON_SIZE as i64) / 2;

            // If we overflow the width of the image, wrap to the next line.
            if x + CAVEINFO_ICON_SIZE as i64 > canvas_header.width() as i64 {
                x = (CAVEINFO_MARGIN * 4) + teki_header.width() as i64;
                y += 70;
                base_y += 70;
                i = 0;

                // Expand the header to make room for the other rows
                expand_canvas(&mut canvas_header, 0, 70 + CAVEINFO_MARGIN as u32, Some([220,220,220,255].into()));
            }

            overlay(&mut canvas_header, &texture, x, y);

            for modifier in tekiinfo.get_texture_modifiers().iter() {
                match modifier {
                    TextureModifier::Falling => {
                        let falling_icon_texture = resize(
                            &*ASSETS.get_img("resources/enemytex_special/falling_icon.png")?,
                            24, 24, FilterType::Nearest
                        );
                        overlay(&mut canvas_header, &falling_icon_texture, x - 8, y - 2);
                    },
                    TextureModifier::Carrying(carrying) => {
                        let carried_treasure_icon = resize(
                            &*ASSETS.get_img(&format!("assets/resulttex/us/arc.d/{}/texture.bti.png", carrying))?,
                            CAVEINFO_ICON_SIZE - 6, CAVEINFO_ICON_SIZE - 6, FilterType::Lanczos3
                        );
                        overlay(&mut canvas_header, &carried_treasure_icon, x + 18, y + 14);
                    },
                    _ => {}
                }
            }

            let teki_subtext = if tekiinfo.filler_distribution_weight > 0 {
                format!("x{} w{}", tekiinfo.minimum_amount, tekiinfo.filler_distribution_weight)
            }
            else {
                format!("x{}", tekiinfo.minimum_amount)
            };

            let subtext_color = match tekiinfo.group {
                0 => [47, 99, 245],
                1 => [201, 2, 52],
                8 => [148, 2, 201],
                6 => [59, 148, 90],
                5 => [133, 133, 133],
                _ => panic!("Invalid teki group in tekiinfo"),
            };

            let teki_subtext_texture = render_text(&teki_subtext, 24.0, subtext_color);
            overlay(&mut canvas_header, &teki_subtext_texture, x + CAVEINFO_ICON_SIZE as i64 / 2 - teki_subtext_texture.width() as i64 / 2, y + CAVEINFO_ICON_SIZE as i64 - 8);

            i += 1;
        }
    }

    base_y += teki_header.height() as i64 + CAVEINFO_MARGIN;

    // Treasures section
    let treasure_header = render_text("Treasures", 48.0, [207, 105, 33]);
    let poko_icon = resize(&*ASSETS.get_img("resources/enemytex_special/Poko_icon.png")?, 16, 19, FilterType::Lanczos3);
    overlay(&mut canvas_header, &treasure_header, CAVEINFO_MARGIN * 2, base_y);
    
    let mut base_x = treasure_header.width() as i64 + CAVEINFO_MARGIN;
    for treasureinfo in caveinfo.item_info.iter() {
        let treasure = ASSETS.treasures.iter().find(|t| t.internal_name.eq_ignore_ascii_case(&treasureinfo.internal_name))
            .expect("Teki carrying unknown or invalid treasure!");

        let treasure_texture = resize(&treasureinfo.get_texture()?, CAVEINFO_ICON_SIZE, CAVEINFO_ICON_SIZE, FilterType::Lanczos3);
        let x = base_x + CAVEINFO_MARGIN * 4;
        let y = base_y + CAVEINFO_MARGIN + (64 - CAVEINFO_ICON_SIZE as i64) / 2;
        overlay(&mut canvas_header, &treasure_texture, x, y);

        let value_text = render_text(&format!("{}", treasure.value), 20.0, [20,20,20]);
        let sidetext_x = x + treasure_texture.width() as i64 + 2;
        overlay(&mut canvas_header, &poko_icon, sidetext_x, y + 4);
        overlay(&mut canvas_header, &value_text,
            sidetext_x + poko_icon.width() as i64 + 3,
            y - value_text.height() as i64 / 2 + poko_icon.height() as i64 / 2 + 4
        );

        let carriers_text = render_text(&format!("{}/{}", treasure.min_carry, treasure.max_carry), 20.0, [20, 20, 20]);
        overlay(&mut canvas_header, &carriers_text, sidetext_x, y + poko_icon.height() as i64 + 2);

        base_x = sidetext_x + max(poko_icon.width() as i64 + value_text.width() as i64, carriers_text.width() as i64);
    }

    base_y += treasure_header.height() as i64;

    // Capteki section
    let capteki_color = [45, 173, 167];
    let capteki_header = render_text("Cap Teki", 48.0, capteki_color);
    overlay(&mut canvas_header, &capteki_header, CAVEINFO_MARGIN * 2, base_y);
    for (i, capinfo) in caveinfo.cap_info.iter().enumerate() {
        let texture = resize(&capinfo.get_texture()?, CAVEINFO_ICON_SIZE, CAVEINFO_ICON_SIZE, FilterType::Lanczos3);
        let x = (CAVEINFO_MARGIN * 5) + capteki_header.width() as i64 + i as i64 * (CAVEINFO_ICON_SIZE as i64 + CAVEINFO_MARGIN * 2);
        let y = base_y + (64 - CAVEINFO_ICON_SIZE as i64) / 2;
        overlay(&mut canvas_header, &texture, x, y);

        for modifier in capinfo.get_texture_modifiers().iter() {
            match modifier {
                TextureModifier::Falling => {
                    let falling_icon_texture = resize(
                        &*ASSETS.get_img("resources/enemytex_special/falling_icon.png")?,
                        24, 24, FilterType::Nearest
                    );
                    overlay(&mut canvas_header, &falling_icon_texture, x - 8, y - 2);
                },
                _ => {}
            }
        }

        let capteki_subtext = if capinfo.filler_distribution_weight > 0 {
            format!("x{} w{}", capinfo.minimum_amount, capinfo.filler_distribution_weight)
        }
        else {
            format!("x{}", capinfo.minimum_amount)
        };

        let capteki_subtext_texture = render_text(&capteki_subtext, 24.0, capteki_color);
        overlay(&mut canvas_header, &capteki_subtext_texture, x + CAVEINFO_ICON_SIZE as i64 / 2 - capteki_subtext_texture.width() as i64 / 2, y + CAVEINFO_ICON_SIZE as i64 - 10);
    }

    // Done with header section
    // Start Map Tile section

    let mut canvas_maptiles = RgbaImage::from_pixel(canvas_header.width(), 500, [20, 20, 20, 255].into());

    let maptiles_metadata_txt = render_text(
        &format!(
            "Num Rooms: {}     CorridorBetweenRoomsProb: {}%     CapOpenDoorsProb: {}%", 
            caveinfo.num_rooms, caveinfo.corridor_probability * 100.0, caveinfo.cap_probability * 100.0
        ), 
        24.0, 
        [220,220,220]
    );
    overlay(&mut canvas_maptiles, &maptiles_metadata_txt, canvas_header.width() as i64 / 2 - maptiles_metadata_txt.width() as i64 / 2, 0);

    // for unit in caveinfo.cave_units.iter() {
        
    // }


    // Combine sections
    let header_height = canvas_header.height() as i64;
    expand_canvas(&mut canvas_header, 0, canvas_maptiles.height(), None);
    overlay(&mut canvas_header, &canvas_maptiles, 0, header_height);

    Ok(canvas_header)
}

/// Saves a layout image to disc.
/// Filename should not include an extension.
pub fn save_image(img: &RgbaImage, filename: String) -> Result<(), RenderError> {
    let _ = std::fs::create_dir("./output");
    let filename = format!("./output/{}.png", filename);
    img.save_with_format(&filename, image::ImageFormat::Png)
        .map_err(|_| RenderError::IoError(filename.clone()))?;
    println!("???? Saved layout image as \"{}\"", filename);

    Ok(())
}

fn expand_canvas(canvas: &mut RgbaImage, w: u32, h: u32, fill_color: Option<Rgba<u8>>) {
    let mut new_canvas = RgbaImage::from_pixel(canvas.width() + w, canvas.height() + h, fill_color.unwrap_or_else(|| [0,0,0,0].into()));
    overlay(&mut new_canvas, canvas, 0, 0);
    *canvas = new_canvas;
}

// x and z are world coordinates, not image or map unit coordinates
fn draw_object_at<Tex: Textured>(image_buffer: &mut RgbaImage, obj: &Tex, x: f32, z: f32, options: &RenderOptions) -> Result<(), AssetError> {
    let mut texture = obj.get_texture()?;

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
                texture = resize(&texture, *xsize, *zsize, FilterType::Lanczos3);
            },
            _ => {}
        }
    }

    let img_x = ((x * COORD_FACTOR) - (texture.width() as f32 / 2.0)) as i64;
    let img_z = ((z * COORD_FACTOR ) - (texture.height() as f32 / 2.0)) as i64;

    // Draw the main texture
    overlay(image_buffer, &texture, img_x, img_z);

    // Modifiers to be applied after ('above') the main texture
    for modifier in obj.get_texture_modifiers().iter() {
        match modifier {
            TextureModifier::Falling => {
                let falling_icon_texture = resize(
                    &*ASSETS.get_img("resources/enemytex_special/falling_icon.png")?,
                    18, 18, FilterType::Lanczos3
                );
                overlay(image_buffer, &falling_icon_texture, img_x - 5, img_z);
            },
            TextureModifier::Carrying(carrying) => {
                let carried_treasure_icon = resize(
                    &*ASSETS.get_img(&format!("assets/resulttex/us/arc.d/{}/texture.bti.png", carrying))?,
                    24, 24, FilterType::Lanczos3
                );
                overlay(image_buffer, &carried_treasure_icon, img_x + 15, img_z + 15);
            },
            _ => {}
        }
    }

    Ok(())
}

static FONT: Lazy<Font> = Lazy::new(|| {
    let font_bytes = read("resources/BalooChettan2-SemiBold.ttf").expect("Missing font file!");
    Font::from_bytes(font_bytes.as_slice(), FontSettings::default()).expect("Failed to create font!")
});

fn render_text(text: &str, size: f32, color: [u8; 3]) -> RgbaImage {
    let mut layout = FontLayout::new(fontdue::layout::CoordinateSystem::PositiveYDown);
    layout.append(&[&*FONT], &TextStyle::new(text, size, 0));
    let width = layout.glyphs().iter().map(|g| g.x as usize + g.width).max().unwrap_or(0);
    let mut img = RgbaImage::new(width as u32, layout.height() as u32);

    for glyph in layout.glyphs().iter() {
        let (metrics, bitmap) = FONT.rasterize_config_subpixel(glyph.key);
        for (i, (cr, cg, cb)) in bitmap.into_iter().tuples().enumerate() {
            let x = (i % metrics.width) as i64 + glyph.x as i64;
            let y = (i / metrics.width) as i64 + glyph.y as i64;
            if x >= 0 && x < img.width() as i64 && y >= 0 && y < img.height() as i64 {
                let coverage = (cr as f32 + cg as f32 + cb as f32) / 3.0;
                img.put_pixel(x as u32, y as u32, [color[0].saturating_add(255-cr), color[1].saturating_add(255-cg), color[2].saturating_add(255-cb), coverage as u8].into());
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
}

trait Textured {
    fn get_texture(&self) -> Result<RgbaImage, AssetError>;
    fn get_texture_modifiers(&self) -> Vec<TextureModifier>;
}

impl Textured for PlacedMapUnit {
    fn get_texture(&self) -> Result<RgbaImage, AssetError> {
        self.unit.get_texture()
    }

    fn get_texture_modifiers(&self) -> Vec<TextureModifier> {
        self.unit.get_texture_modifiers()
    }
}

impl Textured for TekiInfo {
    fn get_texture(&self) -> Result<RgbaImage, AssetError> {
        match get_special_texture_name(&self.internal_name) {
            Some(special_name) => {
                let filename = format!("resources/enemytex_special/{}", special_name);
                Ok(ASSETS.get_img(&filename)?.to_owned())
            },
            None => {
                let filename = format!("assets/enemytex/arc.d/{}/texture.bti.png", &self.internal_name.to_ascii_lowercase());
                Ok(ASSETS.get_img(&filename)?.to_owned())
            }
        }
    }

    fn get_texture_modifiers(&self) -> Vec<TextureModifier> {
        let mut modifiers = Vec::new();
        if self.spawn_method.is_some() {
            modifiers.push(TextureModifier::Falling);
        }
        if let Some(carrying) = self.carrying.clone() {
            modifiers.push(TextureModifier::Carrying(carrying.internal_name));
            modifiers.push(TextureModifier::QuickGlance(QUICKGLANCE_TREASURE_COLOR.into()));
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
    fn get_texture(&self) -> Result<RgbaImage, AssetError> {
        // We don't consider the possibility of treasures spawning in CapInfo here since that
        // is never done in the vanilla game. May need to fix in the future for romhack support.
        match get_special_texture_name(&self.internal_name) {
            Some(special_name) => {
                let filename = format!("resources/enemytex_special/{}", special_name);
                Ok(ASSETS.get_img(&filename)?.to_owned())
            },
            None => {
                let filename = format!("assets/enemytex/arc.d/{}/texture.bti.png", self.internal_name);
                Ok(ASSETS.get_img(&filename)?.to_owned())
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
    fn get_texture(&self) -> Result<RgbaImage, AssetError> {
        // TODO: fix US region being hardcoded here.
        let filename = format!("assets/resulttex/us/arc.d/{}/texture.bti.png", self.internal_name);
        Ok(ASSETS.get_img(&filename)?.to_owned())
    }

    fn get_texture_modifiers(&self) -> Vec<TextureModifier> {
        vec![
            TextureModifier::QuickGlance(QUICKGLANCE_TREASURE_COLOR.into()), 
            TextureModifier::Scale(TREASURE_SIZE, TREASURE_SIZE)
        ]
    }
}

impl Textured for GateInfo {
    fn get_texture(&self) -> Result<RgbaImage, AssetError> {
        let filename = "resources/enemytex_special/Gray_bramble_gate_icon.png";
        Ok(ASSETS.get_img(filename)?.to_owned())
    }
    
    fn get_texture_modifiers(&self) -> Vec<TextureModifier> {
        vec![TextureModifier::Scale(GATE_SIZE, GATE_SIZE)]
        // TODO: gate hp modifier
    }
}

impl Textured for SpawnObject {
    fn get_texture(&self) -> Result<RgbaImage, AssetError> {
        match self {
            SpawnObject::Teki(tekiinfo, _) => tekiinfo.get_texture(),
            SpawnObject::CapTeki(capinfo, _) => capinfo.get_texture(),
            SpawnObject::Item(iteminfo) => iteminfo.get_texture(),
            SpawnObject::Gate(gateinfo) => gateinfo.get_texture(),
            SpawnObject::Hole(plugged) => {
                ASSETS.get_custom_img("PLUGGED_HOLE").map(|i| i.to_owned()).or_else(|_| {
                    let filename = "resources/enemytex_special/Cave_icon.png";
                    let mut hole_icon = ASSETS.get_img(filename)?.clone();
                    if *plugged {
                        let plug_filename = "resources/enemytex_special/36px-Clog_icon.png";
                        let plug_icon = resize(
                            &*ASSETS.get_img(plug_filename)?,
                            hole_icon.width(), 
                            hole_icon.height(), 
                            FilterType::Lanczos3,
                        );
                        overlay(&mut hole_icon, &plug_icon, 0, 0);
                    }
                    ASSETS.cache_img("PLUGGED_HOLE", hole_icon);
                    Ok(ASSETS.get_custom_img("PLUGGED_HOLE")?.to_owned())
                })
            },
            SpawnObject::Geyser => {
                let filename = "resources/enemytex_special/Geyser_icon.png";
                Ok(ASSETS.get_img(filename)?.to_owned())
            },
            SpawnObject::Ship => {
                let filename = "resources/enemytex_special/pod_icon.png";
                Ok(ASSETS.get_img(filename)?.to_owned())
            }
        }
    }

    fn get_texture_modifiers(&self) -> Vec<TextureModifier> {
        match self {
            SpawnObject::Teki(tekiinfo, _) => tekiinfo.get_texture_modifiers(),
            SpawnObject::CapTeki(capinfo, _) => capinfo.get_texture_modifiers(),
            SpawnObject::Item(iteminfo) => iteminfo.get_texture_modifiers(),
            SpawnObject::Hole(_) | SpawnObject::Geyser => {
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
    fn get_texture(&self) -> Result<RgbaImage, AssetError> {
        let filename = format!("assets/arc/{}/arc.d/texture.bti.png", &self.unit_folder_name);
        Ok(ASSETS.get_img(&filename)?.to_owned())
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
    fn get_texture(&self) -> Result<RgbaImage, AssetError> {
        Ok(self.custom_texture.clone())
    }

    fn get_texture_modifiers(&self) -> Vec<TextureModifier> {
        self.inner.get_texture_modifiers()
    }
}
