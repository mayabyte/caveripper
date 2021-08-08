use super::Layout;
use image::{DynamicImage, GenericImage, io::Reader as ImageReader};

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
        let radar_image_file = format!("./arc/{}/arc.d/texture.bti.png", map_unit.unit.unit_folder_name);
        let mut radar_image = ImageReader::open(radar_image_file).unwrap()
            .decode().unwrap();
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

    image_buffer.save_with_format("./layout.png", image::ImageFormat::Png).unwrap();
}
