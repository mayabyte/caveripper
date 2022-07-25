use std::fs::read;

use crate::caveinfo::{CapInfo, GateInfo, ItemInfo, TekiInfo, CaveInfo};
use crate::assets::{ASSETS, get_special_texture_name};
use crate::errors::{RenderError, AssetError};
use super::{Layout, SpawnObject, PlacedMapUnit};
use clap::Args;
use fontdue::layout::{Layout as FontLayout, TextStyle};
use fontdue::{Font, FontSettings};
use image::imageops::{resize, rotate90};
use image::{Rgba, RgbaImage};
use image::{Pixel, imageops::FilterType};
use itertools::Itertools;
use log::{info};
use once_cell::sync::Lazy;

const RENDER_SCALE: u32 = 8;
const GRID_FACTOR: i32 = 8 * RENDER_SCALE as i32;
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
const CAVEINFO_MARGIN: i32 = 16;
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
    let max_map_x = layout.map_units.iter().map(|unit| unit.x + unit.unit.width as i32).max()
        .ok_or_else(|| RenderError::InvalidLayout(layout.cave_name.to_string(), layout.starting_seed))?;
    let max_map_z = layout.map_units.iter().map(|unit| unit.z + unit.unit.height as i32).max()
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
        let img_x = (map_unit.x * GRID_FACTOR) as u32;
        let img_z = (map_unit.z * GRID_FACTOR) as u32;
        for (radar_x, radar_z, pixel) in radar_image.enumerate_pixels() {
            let src_pixel = canvas.get_pixel_mut(img_x + radar_x, img_z + radar_z);
            src_pixel.blend(pixel);
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
            let square = RgbaImage::from_pixel((x2 - x1) as u32, (z2 - z1) as u32, [0, 100, 230, 50].into());
            blend(&mut canvas, &square, img_x as i32 + (x1 + w) as i32, img_z as i32 + (z1 + h) as i32);
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

pub fn render_caveinfo(caveinfo: &CaveInfo, options: RenderOptions) -> Result<RgbaImage, RenderError> {
    let mut canvas_header = RgbaImage::from_pixel(1280, 400, [220,220,220,255].into());

    let sublevel_title = render_text(&caveinfo.sublevel.as_ref().unwrap().long_name(), 64.0, [0,0,0]);
    blend(&mut canvas_header, &sublevel_title, CAVEINFO_MARGIN, 0);

    let teki_header = render_text(&format!("Teki (max {})", caveinfo.max_main_objects), 48.0, [225,0,0]);
    blend(&mut canvas_header, &teki_header, CAVEINFO_MARGIN, 64 + CAVEINFO_MARGIN);
    let mut i = 0;
    for group in [8, 1, 0, 6, 5] {
        for tekiinfo in caveinfo.teki_group(group) {
            let texture = resize(&tekiinfo.get_texture()?, CAVEINFO_ICON_SIZE, CAVEINFO_ICON_SIZE, FilterType::Lanczos3);
            let x = (CAVEINFO_MARGIN * 2) + teki_header.width() as i32 + i as i32 * (CAVEINFO_ICON_SIZE as i32 + CAVEINFO_MARGIN);
            let y = 64 + CAVEINFO_MARGIN + (64 - CAVEINFO_ICON_SIZE as i32) / 2;

            blend(&mut canvas_header, &texture, x, y);

            for modifier in tekiinfo.get_texture_modifiers().iter() {
                match modifier {
                    TextureModifier::Falling => {
                        let falling_icon_texture = resize(
                            &*ASSETS.get_img("resources/enemytex_special/falling_icon.png")?,
                            24, 24, FilterType::Nearest
                        );
                        blend(&mut canvas_header, &falling_icon_texture, x - 8, y - 2);
                    },
                    TextureModifier::Carrying(carrying) => {
                        let carried_treasure_icon = resize(
                            &*ASSETS.get_img(&format!("assets/resulttex/us/arc.d/{}/texture.bti.png", carrying))?,
                            CAVEINFO_ICON_SIZE - 6, CAVEINFO_ICON_SIZE - 6, FilterType::Lanczos3
                        );
                        blend(&mut canvas_header, &carried_treasure_icon, x + 18, y + 14);
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

            let teki_subtext_texture = render_text(&teki_subtext, 20.0, subtext_color);
            blend(&mut canvas_header, &teki_subtext_texture, x + CAVEINFO_ICON_SIZE as i32 / 2 - teki_subtext_texture.width() as i32 / 2, y + CAVEINFO_ICON_SIZE as i32 - 2);

            i += 1;
        }
    }

    let treasure_header = render_text("Treasures", 48.0, [207, 105, 33]);
    blend(&mut canvas_header, &treasure_header, CAVEINFO_MARGIN, 64 + CAVEINFO_MARGIN + 64 + CAVEINFO_MARGIN);

    Ok(canvas_header)
}

/// Saves a layout image to disc.
/// Filename should not include an extension.
pub fn save_image(img: &RgbaImage, filename: String) -> Result<(), RenderError> {
    let _ = std::fs::create_dir("./output");
    let filename = format!("./output/{}.png", filename);
    img.save_with_format(&filename, image::ImageFormat::Png)
        .map_err(|_| RenderError::IoError(filename.clone()))?;
    println!("🍞 Saved layout image as \"{}\"", filename);

    Ok(())
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
                blend(
                    image_buffer, 
                    &circle_tex, 
                    ((x * COORD_FACTOR) - circle_size) as i32, 
                    ((z * COORD_FACTOR) - circle_size) as i32
                );
            },
            TextureModifier::Scale(xsize, zsize) => {
                texture = resize(&texture, *xsize, *zsize, FilterType::Lanczos3);
            },
            _ => {}
        }
    }

    let img_x = ((x * COORD_FACTOR) - (texture.width() as f32 / 2.0)) as i32;
    let img_z = ((z * COORD_FACTOR ) - (texture.height() as f32 / 2.0)) as i32;

    // Draw the main texture
    blend(image_buffer, &texture, img_x, img_z);

    // Modifiers to be applied after ('above') the main texture
    for modifier in obj.get_texture_modifiers().iter() {
        match modifier {
            TextureModifier::Falling => {
                let falling_icon_texture = resize(
                    &*ASSETS.get_img("resources/enemytex_special/falling_icon.png")?,
                    18, 18, FilterType::Lanczos3
                );
                blend(image_buffer, &falling_icon_texture, img_x - 5, img_z);
            },
            TextureModifier::Carrying(carrying) => {
                let carried_treasure_icon = resize(
                    &*ASSETS.get_img(&format!("assets/resulttex/us/arc.d/{}/texture.bti.png", carrying))?,
                    24, 24, FilterType::Lanczos3
                );
                blend(image_buffer, &carried_treasure_icon, img_x + 15, img_z + 15);
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
            let x = (i % metrics.width) as i32 + glyph.x as i32;
            let y = (i / metrics.width) as i32 + glyph.y as i32;
            if x >= 0 && x < img.width() as i32 && y >= 0 && y < img.height() as i32 {
                let coverage = (cr as f32 + cg as f32 + cb as f32) / 3.0;
                img.put_pixel(x as u32, y as u32, [color[0].saturating_add(255-cr), color[1].saturating_add(255-cg), color[2].saturating_add(255-cb), coverage as u8].into());
            }
        }
    }

    img
}

fn blend(base: &mut RgbaImage, top: &RgbaImage, x: i32, z: i32) {
    for (top_x, top_z, pixel) in top.enumerate_pixels() {
        // Skip this pixel if it's out-of-bounds
        if x + (top_x as i32) < 0 || x + (top_x as i32) >= (base.width() as i32) || z + (top_z as i32) < 0 || z + (top_z as i32) >= (base.height() as i32) {
            continue;
        }

        // blend_pixel is deprecated for some silly reason so we have to do it like this
        let source_pixel = base.get_pixel_mut((x + top_x as i32) as u32, (z + top_z as i32) as u32);
        source_pixel.blend(pixel);
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
        let filename = format!("assets/arc/{}/arc.d/texture.bti.png", &self.unit.unit_folder_name);
        Ok(ASSETS.get_img(&filename)?.to_owned())
    }

    fn get_texture_modifiers(&self) -> Vec<TextureModifier> {
        Vec::new()
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
                        blend(&mut hole_icon, &plug_icon, 0, 0);
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
