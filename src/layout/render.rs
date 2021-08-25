use crate::assets::get_file_bytes;
use super::{Layout, SpawnObject};
use image::{DynamicImage, GenericImage, Pixel};

pub fn render_layout(layout: &Layout) {
    let min_map_x = layout.map_units.iter().map(|unit| unit.x).min().unwrap();
    let max_map_x = layout.map_units.iter().map(|unit| unit.x + unit.unit.width as isize).max().unwrap();
    let min_map_z = layout.map_units.iter().map(|unit| unit.z).min().unwrap();
    let max_map_z = layout.map_units.iter().map(|unit| unit.z + unit.unit.height as isize).max().unwrap();

    let mut image_buffer = DynamicImage::new_rgb8(
        (max_map_x - min_map_x) as u32 * 8,
        (max_map_z - min_map_z) as u32 * 8
    );

    // Draw map units
    for map_unit in layout.map_units.iter() {
        // Open the radar image for the map unit
        let radar_image_bytes = get_file_bytes(&format!("assets/gcn/arc/{}/arc.d/texture.bti.png", map_unit.unit.unit_folder_name)).unwrap();
        let mut radar_image = image::load_from_memory(radar_image_bytes.as_ref()).unwrap();
        for _ in 0..map_unit.unit.rotation {
            radar_image = radar_image.rotate90();
        }

        let radar_image = radar_image.into_rgba8();

        // Copy the pixels of the radar image to the buffer
        let img_x = ((map_unit.x - min_map_x) * 8) as u32;
        let img_z = ((map_unit.z - min_map_z) * 8) as u32;
        for (radar_x, radar_z, pixel) in radar_image.enumerate_pixels() {
            image_buffer.put_pixel(img_x + radar_x, img_z + radar_z, pixel.clone());
        }
    }

    // Draw spawned objects
    for spawn_point in layout.map_units.iter().flat_map(|unit| unit.spawnpoints.iter()) {
        match spawn_point.contains {
            None => continue,
            Some(SpawnObject::Ship) => {
                image_buffer.put_pixel(
                    (((spawn_point.x / 170.0) - min_map_x as f32) * 8.0) as u32,
                    (((spawn_point.z / 170.0) - min_map_z as f32) * 8.0) as u32,
                    Pixel::from_channels(255, 0, 0, 255)
                );
            },
            _ => panic!("unrecognized drawable spawn item!"),
        }
    }

    image_buffer.save_with_format("./caveripper_output/layout.png", image::ImageFormat::Png).unwrap();
}
