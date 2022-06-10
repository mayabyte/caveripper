use std::error::Error;

use crate::assets::get_file_bytes;
use crate::caveinfo::{CapInfo, GateInfo, ItemInfo, TekiInfo, get_resource_file_bytes, get_special_texture_name};
use super::{Layout, SpawnObject, PlacedMapUnit};
use image::RgbImage;
use image::{ImageFormat, DynamicImage, GenericImage, GenericImageView, Pixel, imageops::FilterType};
use log::debug;

const RENDER_SCALE: u32 = 8;
const GATE_SCALE: f32 = 1.7;
const TREASURE_SCALE: f32 = 1.1;


pub fn render_layout(layout: &Layout) {
    debug!("Generating layout image.");

    // Find the minimum and maximum map tile coordinates in the layout.
    let min_map_x = layout.map_units.iter().map(|unit| unit.x).min().unwrap();
    let max_map_x = layout.map_units.iter().map(|unit| unit.x + unit.unit.width as isize).max().unwrap();
    let min_map_z = layout.map_units.iter().map(|unit| unit.z).min().unwrap();
    let max_map_z = layout.map_units.iter().map(|unit| unit.z + unit.unit.height as isize).max().unwrap();

    println!("min: {} {}", min_map_x, min_map_z);

    // Each map tile is 8x8 pixels on the radar.
    // We scale this up further so teki and treasure textures can be rendered at a decent
    // resolution on top of the generated layout images.
    let mut image_buffer = DynamicImage::new_rgb8(
        (max_map_x - min_map_x) as u32 * 8 * RENDER_SCALE,
        (max_map_z - min_map_z) as u32 * 8 * RENDER_SCALE
    );

    // Draw map units
    for map_unit in layout.map_units.iter() {
        let mut radar_image = map_unit.get_texture();
        
        for _ in 0..map_unit.unit.rotation {
            radar_image = radar_image.rotate90();
        }

        let radar_image = radar_image.resize(
            radar_image.width() * RENDER_SCALE, 
            radar_image.height() * RENDER_SCALE, 
            FilterType::Nearest
        );

        let radar_image = radar_image.into_rgba8();

        // Copy the pixels of the radar image to the buffer
        let img_x = ((map_unit.x - min_map_x) * 8 * (RENDER_SCALE as isize)) as u32;
        let img_z = ((map_unit.z - min_map_z) * 8 * (RENDER_SCALE as isize)) as u32;
        for (radar_x, radar_z, pixel) in radar_image.enumerate_pixels() {
            image_buffer.put_pixel(img_x + radar_x, img_z + radar_z, pixel.clone());
        }
    }

    // Draw spawned objects
    for spawn_point in layout.map_units.iter().flat_map(|unit| unit.spawnpoints.iter()) {
        if let Some(spawn_object) = spawn_point.contains.as_ref() {
            match spawn_object {
                SpawnObject::TekiBunch(teki_list) => {
                    for (tekiinfo, (dx, _, dz)) in teki_list.iter() {
                        draw_object_at(&mut image_buffer, tekiinfo, spawn_point.x + dx, spawn_point.z + dz, min_map_x, min_map_z, 1.0);
                    }
                },
                SpawnObject::Item(iteminfo) => {
                    draw_object_at(&mut image_buffer, iteminfo, spawn_point.x, spawn_point.z, min_map_x, min_map_z, TREASURE_SCALE);
                },
                _ => {
                    draw_object_at(&mut image_buffer, spawn_object, spawn_point.x, spawn_point.z, min_map_x, min_map_z, 1.0);
                },
            }
        }
        
        // Draw falling cap teki
        if let Some(spawn_object) = spawn_point.falling_cap_teki.as_ref() {
            draw_object_at(&mut image_buffer, spawn_object, spawn_point.x - 30.0, spawn_point.z - 30.0, min_map_x, min_map_z, 1.0);
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
                SpawnObject::TekiBunch(teki_list) => {
                    for (tekiinfo, (dx, _, dz)) in teki_list.iter() {
                        draw_object_at(&mut image_buffer, tekiinfo, x + dx, z + dz, min_map_x, min_map_z, 1.0);
                    }
                },
                SpawnObject::Gate(gateinfo) => {
                    let mut texture = gateinfo.get_texture();
                    if door.borrow().door_unit.direction % 2 == 1 {
                        texture = texture.rotate90();
                    }
                    draw_object_at(&mut image_buffer, &texture, x, z, min_map_x, min_map_z, GATE_SCALE);
                }
                _ => {
                    draw_object_at(&mut image_buffer, spawn_object, x, z, min_map_x, min_map_z, 1.0);
                },
            }
        }
    }

    image_buffer.save_with_format("./caveripper_output/layout.png", image::ImageFormat::Png).unwrap();
}

// x and y are world coordinates, not image or map unit coordinates
fn draw_object_at<Tex: Textured>(image_buffer: &mut DynamicImage, obj: &Tex, x: f32, z: f32, min_map_x: isize, min_map_z: isize, scale: f32) {
    let texture = obj.get_texture();
    let texture = texture.resize(
        (32.0 * scale) as u32, (32.0 * scale) as u32,
        FilterType::Lanczos3
    );

    let img_x = (((x / 170.0) - min_map_x as f32) * 8.0 * (RENDER_SCALE as f32) - (texture.width() as f32 / 2.0)) as i32;
    let img_z = (((z / 170.0) - min_map_z as f32) * 8.0 * (RENDER_SCALE as f32) - (texture.height() as f32 / 2.0)) as i32;

    blend(image_buffer, &texture, img_x, img_z);

    for modifier in obj.get_texture_modifiers().iter() {
        match modifier {
            TextureModifier::Falling => {
                let falling_icon_texture = image::load_from_memory(get_resource_file_bytes("resources/enemytex_special/falling_icon.png")
                    .unwrap().as_ref()).unwrap()
                    .resize(14, 14, FilterType::Lanczos3);
                blend(image_buffer, &falling_icon_texture, img_x - 5, img_z);
            },
            TextureModifier::Carrying(carrying) => {
                let carried_treasure_icon = read_image_file(&format!("assets/resulttex/us/arc.d/{}/texture.bti.png", carrying))
                    .unwrap()
                    .resize(24, 24, FilterType::Lanczos3);
                blend(image_buffer, &carried_treasure_icon, img_x + 10, img_z + 10);
            }
        }
    }
}

fn blend(base: &mut DynamicImage, top: &DynamicImage, x: i32, z: i32) {
    for (top_x, top_z, pixel) in top.to_rgba8().enumerate_pixels() {
        // Skip this pixel if it's out-of-bounds
        if x + (top_x as i32) < 0 || x + (top_x as i32) >= (base.width() as i32) || z + (top_z as i32) < 0 || z + (top_z as i32) >= (base.height() as i32) {
            continue;
        }

        // blend_pixel is deprecated for some silly reason so we have to do it like this
        let mut source_pixel = base.get_pixel((x + top_x as i32) as u32, (z + top_z as i32) as u32);
        source_pixel.blend(pixel);
        base.put_pixel((x + top_x as i32) as u32, (z + top_z as i32) as u32, source_pixel);
    }
}

fn read_image_file(path: &str) -> Result<DynamicImage, Box<dyn Error>> {
    Ok(
        image::load_from_memory_with_format(
            get_file_bytes(path)
            .ok_or(format!("Couldn't find texture file '{}'", path))?
            .as_ref(),
            ImageFormat::Png
        )?
    )
}

enum TextureModifier {
    Falling,
    Carrying(String),
}

trait Textured {
    fn get_texture(&self) -> DynamicImage;
    fn get_texture_modifiers(&self) -> Vec<TextureModifier>;
}

impl Textured for PlacedMapUnit {
    fn get_texture(&self) -> DynamicImage {
        let filename = format!("assets/arc/{}/arc.d/texture.bti.png", &self.unit.unit_folder_name);
        read_image_file(&filename).unwrap()
    }

    fn get_texture_modifiers(&self) -> Vec<TextureModifier> {
        Vec::new()
    }
}

impl Textured for TekiInfo {
    fn get_texture(&self) -> DynamicImage {
        match get_special_texture_name(&self.internal_name) {
            Some(special_name) => {
                let filename = format!("resources/enemytex_special/{}", special_name);
                image::load_from_memory(get_resource_file_bytes(&filename).unwrap().as_ref()).unwrap()
            },
            None => {
                let filename = format!("assets/enemytex/arc.d/{}/texture.bti.png", &self.internal_name);
                read_image_file(&filename).unwrap()
            }
        }
    }

    fn get_texture_modifiers(&self) -> Vec<TextureModifier> {
        let mut modifiers = Vec::new();
        if self.spawn_method.is_some() {
            modifiers.push(TextureModifier::Falling);
        }
        if let Some(carrying) = self.carrying.clone() {
            modifiers.push(TextureModifier::Carrying(carrying));
        }
        modifiers
    }
}

impl Textured for CapInfo {
    fn get_texture(&self) -> DynamicImage {
        // We don't consider the possibility of treasures spawning in CapInfo here since that
        // is never done in the vanilla game. May need to fix in the future for romhack support.
        match get_special_texture_name(&self.internal_name) {
            Some(special_name) => {
                let filename = format!("resources/enemytex_special/{}", special_name);
                image::load_from_memory(get_resource_file_bytes(&filename).unwrap().as_ref()).unwrap()
            },
            None => {
                let filename = format!("assets/enemytex/arc.d/{}/texture.bti.png", self.internal_name);
                read_image_file(&filename).unwrap()
            }
        }
    }

    fn get_texture_modifiers(&self) -> Vec<TextureModifier> {
        let mut modifiers = Vec::new();
        if self.is_falling() {
            modifiers.push(TextureModifier::Falling);
        }
        modifiers
    }
}

impl Textured for ItemInfo {
    fn get_texture(&self) -> DynamicImage {
        // TODO: fix US region being hardcoded here.
        let filename = format!("assets/resulttex/us/arc.d/{}/texture.bti.png", self.internal_name);
        read_image_file(&filename).unwrap()
    }

    fn get_texture_modifiers(&self) -> Vec<TextureModifier> {
        Vec::new()
    }
}

impl Textured for GateInfo {
    fn get_texture(&self) -> DynamicImage {
        let filename = "resources/enemytex_special/Gray_bramble_gate_icon.png";
        image::load_from_memory(get_resource_file_bytes(filename).unwrap().as_ref()).unwrap()
    }
    
    fn get_texture_modifiers(&self) -> Vec<TextureModifier> {
        Vec::new()
        // TODO: gate hp modifier
    }
}

impl Textured for SpawnObject {
    fn get_texture(&self) -> DynamicImage {
        match self {
            SpawnObject::Teki(tekiinfo) | SpawnObject::PlantTeki(tekiinfo) => tekiinfo.get_texture(),
            SpawnObject::TekiBunch(tekis) => {
                // All teki in a bunch will have the same texture
                let (first_teki, _) = tekis.first().unwrap();
                first_teki.get_texture()
            },
            SpawnObject::CapTeki(capinfo, _) => capinfo.get_texture(),
            SpawnObject::Item(iteminfo) => iteminfo.get_texture(),
            SpawnObject::Gate(gateinfo) => gateinfo.get_texture(),
            SpawnObject::Hole(plugged) => {
                let filename = "resources/enemytex_special/Cave_icon.png";
                let mut base_texture = image::load_from_memory(get_resource_file_bytes(filename).unwrap().as_ref()).unwrap();
                if *plugged {
                    let plug_filename = "resources/enemytex_special/36px-Clog_icon.png";
                    let plug_icon = image::load_from_memory(get_resource_file_bytes(plug_filename).unwrap().as_ref()).unwrap()
                        .resize_exact(base_texture.width(), base_texture.height(), FilterType::Lanczos3);
                    blend(&mut base_texture, &plug_icon, 0, 0);
                }
                base_texture
            },
            SpawnObject::Geyser => {
                let filename = "resources/enemytex_special/Geyser_icon.png";
                image::load_from_memory(get_resource_file_bytes(filename).unwrap().as_ref()).unwrap()
            },
            SpawnObject::Ship => {
                let filename = "resources/enemytex_special/pod_icon.png";
                image::load_from_memory(get_resource_file_bytes(filename).unwrap().as_ref()).unwrap()
            },
            _ => DynamicImage::ImageRgb8(RgbImage::new(0, 0)),
        }
    }

    fn get_texture_modifiers(&self) -> Vec<TextureModifier> {
        match self {
            SpawnObject::Teki(tekiinfo) => tekiinfo.get_texture_modifiers(),
            SpawnObject::TekiBunch(tekis) => {
                // All teki in a bunch will have the same spawn method
                let (first_teki, _) = tekis.first().unwrap();
                first_teki.get_texture_modifiers()
            },
            SpawnObject::CapTeki(capinfo, _) => capinfo.get_texture_modifiers(),
            _ => Vec::new()
        }
    }
}

impl Textured for DynamicImage {
    fn get_texture(&self) -> DynamicImage {
        self.clone()
    }

    fn get_texture_modifiers(&self) -> Vec<TextureModifier> {
        Vec::new()
    }
}