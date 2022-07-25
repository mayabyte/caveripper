use std::ops::Deref;

use crate::caveinfo::{CapInfo, GateInfo, ItemInfo, TekiInfo};
use crate::assets::{ASSETS, get_special_texture_name};
use crate::errors::{RenderError, AssetError};
use super::{Layout, SpawnObject, PlacedMapUnit};
use clap::Args;
use dashmap::mapref::one::Ref;
use image::{Rgba, RgbaImage};
use image::{DynamicImage, GenericImage, GenericImageView, Pixel, imageops::FilterType};
use log::{info};

const RENDER_SCALE: u32 = 8;
const GRID_FACTOR: i32 = 8 * RENDER_SCALE as i32;
const COORD_FACTOR: f32 = (8.0 * RENDER_SCALE as f32) / 170.0;
const GATE_SCALE: f32 = 1.7;
const TREASURE_SCALE: f32 = 1.1;
const FALLING_CAP_TEKI_SCALE: f32 = 0.9;
const QUICKGLANCE_CIRCLE_SCALE: f32 = 2.4;
const QUICKGLANCE_TREASURE_COLOR: [u8; 4] = [245, 150, 0, 110];
const QUICKGLANCE_EXIT_COLOR: [u8; 4] = [10, 225, 100, 95];
const QUICKGLANCE_SHIP_COLOR: [u8; 4] = [255, 40, 40, 80];
const QUICKGLANCE_VIOLET_CANDYPOP_COLOR: [u8; 4] = [255, 0, 245, 80];
const QUICKGLANCE_IVORY_CANDYPOP_COLOR: [u8; 4] = [100, 100, 100, 120];
const QUICKGLANCE_ROAMING_COLOR: [u8; 4] = [200, 0, 130, 60];


#[derive(Default, Debug, Args)]
pub struct RenderOptions {
    #[clap(long)]
    pub draw_grid: bool,

    #[clap(long, short='q')]
    pub quickglance: bool,
}


pub fn render_layout(layout: &Layout, options: RenderOptions) -> Result<DynamicImage, RenderError> {
    info!("Drawing layout image...");

    // Find the minimum and maximum map tile coordinates in the layout.
    let max_map_x = layout.map_units.iter().map(|unit| unit.x + unit.unit.width as i32).max()
        .ok_or_else(|| RenderError::InvalidLayout(layout.cave_name.to_string(), layout.starting_seed))?;
    let max_map_z = layout.map_units.iter().map(|unit| unit.z + unit.unit.height as i32).max()
        .ok_or_else(|| RenderError::InvalidLayout(layout.cave_name.to_string(), layout.starting_seed))?;

    // Each map tile is 8x8 pixels on the radar.
    // We scale this up further so teki and treasure textures can be rendered at a decent
    // resolution on top of the generated layout images.
    let mut image_buffer = DynamicImage::new_rgb8(
        max_map_x as u32 * 8 * RENDER_SCALE,
        max_map_z as u32 * 8 * RENDER_SCALE
    );

    // Draw map units
    for map_unit in layout.map_units.iter() {
        let mut radar_image = map_unit.get_texture()?.clone();
        
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
        let img_x = (map_unit.x * GRID_FACTOR) as u32;
        let img_z = (map_unit.z * GRID_FACTOR) as u32;
        for (radar_x, radar_z, pixel) in radar_image.enumerate_pixels() {
            image_buffer.put_pixel(img_x + radar_x, img_z + radar_z, pixel.clone());
        }

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
            let w = (map_unit.unit.width as i32 * GRID_FACTOR) as f32 / 2.0;
            let h = (map_unit.unit.height as i32 * GRID_FACTOR) as f32 / 2.0;
            let mut square: DynamicImage = RgbaImage::new((x2 - x1) as u32, (z2 - z1) as u32).into();
            for i in 0..(x2 - x1) as u32 {
                for j in 0..(z2 - z1) as u32 {
                    square.put_pixel(i, j, [0, 100, 230, 50].into());
                }
            }
            blend(&mut image_buffer, &square, img_x as i32 + (x1 + w) as i32, img_z as i32 + (z1 + h) as i32);
        }
    }

    // Draw a map unit grid, if enabled
    if options.draw_grid {
        let grid_color: Rgba<u8> = [255, 0, 0, 150].into();
        let grid_size = GRID_FACTOR as u32;
        for x in 0..image_buffer.width() {
            for z in 0..image_buffer.height() {
                if x % grid_size == 0 || z % grid_size == 0 {
                    let mut new_pix = image_buffer.get_pixel(x, z);
                    new_pix.blend(&grid_color);
                    image_buffer.put_pixel(x, z, new_pix);
                }
            }
        }
    }

    // Draw spawned objects
    for spawnpoint in layout.map_units.iter().flat_map(|unit| unit.spawnpoints.iter()) {
        for spawn_object in spawnpoint.contains.iter() {
            match spawn_object {
                SpawnObject::Teki(tekiinfo, (dx, dz)) => {
                    draw_object_at(&mut image_buffer, tekiinfo, spawnpoint.x + dx, spawnpoint.z + dz, 1.0, &options)?;
                },
                SpawnObject::Item(iteminfo) => {
                    draw_object_at(&mut image_buffer, iteminfo, spawnpoint.x, spawnpoint.z, TREASURE_SCALE, &options)?;
                },
                SpawnObject::CapTeki(capinfo, _) if capinfo.is_falling() => {
                    draw_object_at(&mut image_buffer, capinfo, spawnpoint.x - 30.0, spawnpoint.z - 30.0, FALLING_CAP_TEKI_SCALE, &options)?;
                },
                _ => {
                    draw_object_at(&mut image_buffer, spawn_object, spawnpoint.x, spawnpoint.z, 1.0, &options)?;
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
                        draw_object_at(&mut image_buffer, &&(texture.rotate90()), x, z, GATE_SCALE, &options)?;
                    }
                    else {
                        draw_object_at(&mut image_buffer, &texture.value(), x, z, GATE_SCALE, &options)?;
                    }
                }
                _ => {
                    draw_object_at(&mut image_buffer, spawn_object, x, z, 1.0, &options)?;
                },
            }
        }
    }

    Ok(image_buffer)
}

/// Saves a layout image to disc.
/// Filename should not include an extension.
pub fn save_image(img: &DynamicImage, filename: String) -> Result<(), RenderError> {
    let _ = std::fs::create_dir("./output");
    let filename = format!("./output/{}.png", filename);
    img.save_with_format(&filename, image::ImageFormat::Png)
        .map_err(|_| RenderError::IoError(filename.clone()))?;
    println!("🍞 Saved layout image as \"{}\"", filename);

    Ok(())
}

// x and z are world coordinates, not image or map unit coordinates
fn draw_object_at<Tex: Textured>(image_buffer: &mut DynamicImage, obj: &Tex, x: f32, z: f32, scale: f32, options: &RenderOptions) -> Result<(), AssetError> {
    let texture = obj.get_texture()?;
    let texture = texture.resize(
        (32.0 * scale) as u32, (32.0 * scale) as u32,
        FilterType::Lanczos3
    );

    let img_x = ((x * COORD_FACTOR) - (texture.width() as f32 / 2.0)) as i32;
    let img_z = ((z * COORD_FACTOR ) - (texture.height() as f32 / 2.0)) as i32;

    // Modifiers to be applied before ('under') the main texture
    for modifier in obj.get_texture_modifiers().iter() {
        match modifier {
            TextureModifier::QuickGlance(color) if options.quickglance => {
                let circle_size = 32.0 * QUICKGLANCE_CIRCLE_SCALE / 2.0;
                let circle_tex = DynamicImage::from(circle(circle_size as u32, *color));
                blend(
                    image_buffer, 
                    &&circle_tex, 
                    ((x * COORD_FACTOR) - circle_size) as i32, 
                    ((z * COORD_FACTOR) - circle_size) as i32
                );
            },
            _ => {}
        }
    }

    // Draw the main texture
    blend(image_buffer, &texture, img_x, img_z);

    // Modifiers to be applied after ('above') the main texture
    for modifier in obj.get_texture_modifiers().iter() {
        match modifier {
            TextureModifier::Falling => {
                let falling_icon_texture = ASSETS.get_img("resources/enemytex_special/falling_icon.png")?
                    .resize(18, 18, FilterType::Lanczos3);
                blend(image_buffer, &falling_icon_texture, img_x - 5, img_z);
            },
            TextureModifier::Carrying(carrying) => {
                let carried_treasure_icon = ASSETS.get_img(&format!("assets/resulttex/us/arc.d/{}/texture.bti.png", carrying))?
                    .resize(24, 24, FilterType::Lanczos3);
                blend(image_buffer, &carried_treasure_icon, img_x + 15, img_z + 15);
            },
            _ => {}
        }
    }

    Ok(())
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
    Falling,
    Carrying(String),
    QuickGlance(Rgba<u8>),
}

trait Textured {
    type Texture: Deref<Target=DynamicImage>;
    fn get_texture(&self) -> Result<Self::Texture, AssetError>;
    fn get_texture_modifiers(&self) -> Vec<TextureModifier>;
}

impl Textured for PlacedMapUnit {
    type Texture = Ref<'static, String, DynamicImage>;
    fn get_texture(&self) -> Result<Self::Texture, AssetError> {
        let filename = format!("assets/arc/{}/arc.d/texture.bti.png", &self.unit.unit_folder_name);
        ASSETS.get_img(&filename)
    }

    fn get_texture_modifiers(&self) -> Vec<TextureModifier> {
        Vec::new()
    }
}

impl Textured for TekiInfo {
    type Texture = Ref<'static, String, DynamicImage>;
    fn get_texture(&self) -> Result<Self::Texture, AssetError> {
        match get_special_texture_name(&self.internal_name) {
            Some(special_name) => {
                let filename = format!("resources/enemytex_special/{}", special_name);
                ASSETS.get_img(&filename)
            },
            None => {
                let filename = format!("assets/enemytex/arc.d/{}/texture.bti.png", &self.internal_name.to_ascii_lowercase());
                ASSETS.get_img(&filename)
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
            modifiers.push(TextureModifier::QuickGlance(QUICKGLANCE_TREASURE_COLOR.into()));
        }
        match self.internal_name.to_ascii_lowercase().as_str() {
            "blackpom" /* Violet Candypop */ => modifiers.push(TextureModifier::QuickGlance(QUICKGLANCE_VIOLET_CANDYPOP_COLOR.into())),
            "whitepom" /* Ivory Candypop */ => modifiers.push(TextureModifier::QuickGlance(QUICKGLANCE_IVORY_CANDYPOP_COLOR.into())),
            "minihoudai" /* Groink */ => modifiers.push(TextureModifier::QuickGlance(QUICKGLANCE_ROAMING_COLOR.into())),
            _ => {}
        }
        modifiers
    }
}

impl Textured for CapInfo {
    type Texture = Ref<'static, String, DynamicImage>;
    fn get_texture(&self) -> Result<Self::Texture, AssetError> {
        // We don't consider the possibility of treasures spawning in CapInfo here since that
        // is never done in the vanilla game. May need to fix in the future for romhack support.
        match get_special_texture_name(&self.internal_name) {
            Some(special_name) => {
                let filename = format!("resources/enemytex_special/{}", special_name);
                ASSETS.get_img(&filename)
            },
            None => {
                let filename = format!("assets/enemytex/arc.d/{}/texture.bti.png", self.internal_name);
                ASSETS.get_img(&filename)
            }
        }
    }

    fn get_texture_modifiers(&self) -> Vec<TextureModifier> {
        let mut modifiers = Vec::new();
        if self.is_falling() {
            modifiers.push(TextureModifier::Falling);
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
    type Texture = Ref<'static, String, DynamicImage>;
    fn get_texture(&self) -> Result<Self::Texture, AssetError> {
        // TODO: fix US region being hardcoded here.
        let filename = format!("assets/resulttex/us/arc.d/{}/texture.bti.png", self.internal_name);
        ASSETS.get_img(&filename)
    }

    fn get_texture_modifiers(&self) -> Vec<TextureModifier> {
        vec![TextureModifier::QuickGlance(QUICKGLANCE_TREASURE_COLOR.into())]
    }
}

impl Textured for GateInfo {
    type Texture = Ref<'static, String, DynamicImage>;
    fn get_texture(&self) -> Result<Self::Texture, AssetError> {
        let filename = "resources/enemytex_special/Gray_bramble_gate_icon.png";
        ASSETS.get_img(filename)
    }
    
    fn get_texture_modifiers(&self) -> Vec<TextureModifier> {
        Vec::new()
        // TODO: gate hp modifier
    }
}

impl Textured for SpawnObject {
    type Texture = Ref<'static, String, DynamicImage>;
    fn get_texture(&self) -> Result<Self::Texture, AssetError> {
        match self {
            SpawnObject::Teki(tekiinfo, _) => tekiinfo.get_texture(),
            SpawnObject::CapTeki(capinfo, _) => capinfo.get_texture(),
            SpawnObject::Item(iteminfo) => iteminfo.get_texture(),
            SpawnObject::Gate(gateinfo) => gateinfo.get_texture(),
            SpawnObject::Hole(plugged) => {
                ASSETS.get_custom_img("PLUGGED_HOLE").or_else(|_| {
                    let filename = "resources/enemytex_special/Cave_icon.png";
                    let mut hole_icon = ASSETS.get_img(filename)?.clone();
                    if *plugged {
                        let plug_filename = "resources/enemytex_special/36px-Clog_icon.png";
                        let plug_icon = ASSETS.get_img(plug_filename)?
                            .resize_exact(hole_icon.width(), hole_icon.height(), FilterType::Lanczos3);
                        blend(&mut hole_icon, &plug_icon, 0, 0);
                    }
                    ASSETS.cache_img("PLUGGED_HOLE", hole_icon);
                    ASSETS.get_custom_img("PLUGGED_HOLE")
                })
            },
            SpawnObject::Geyser => {
                let filename = "resources/enemytex_special/Geyser_icon.png";
                ASSETS.get_img(filename)
            },
            SpawnObject::Ship => {
                let filename = "resources/enemytex_special/pod_icon.png";
                ASSETS.get_img(filename)
            }
        }
    }

    fn get_texture_modifiers(&self) -> Vec<TextureModifier> {
        match self {
            SpawnObject::Teki(tekiinfo, _) => tekiinfo.get_texture_modifiers(),
            SpawnObject::CapTeki(capinfo, _) => capinfo.get_texture_modifiers(),
            SpawnObject::Hole(_) | SpawnObject::Geyser => {
                vec![TextureModifier::QuickGlance(QUICKGLANCE_EXIT_COLOR.into())]
            },
            SpawnObject::Ship => {
                vec![TextureModifier::QuickGlance(QUICKGLANCE_SHIP_COLOR.into())]
            },
            _ => Vec::new()
        }
    }
}

impl<'a> Textured for &'a DynamicImage {
    type Texture = &'a DynamicImage;
    fn get_texture(&self) -> Result<Self::Texture, AssetError> {
        Ok(self)
    }

    fn get_texture_modifiers(&self) -> Vec<TextureModifier> {
        Vec::new() // TODO
    }
}
