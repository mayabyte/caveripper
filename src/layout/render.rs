use super::Layout;
use image::{ImageBuffer, Rgb, io::Reader as ImageReader};

pub fn render_layout(layout: &Layout) {
    let mut image_buffer = ImageBuffer::from_pixel(256, 256, Rgb::from([0,0,0]));
    for map_unit in layout.map_units.iter() {
        // Open the radar image for the map unit
        let radar_image_file = format!("./arc/{}/arc.d/texture.bti.png", map_unit.unit.unit_folder_name);
        let mut radar_image = ImageReader::open(radar_image_file).unwrap()
            .decode().unwrap();
        for _ in 0..map_unit.unit.rotation {
            radar_image = radar_image.rotate90();
        }

        let radar_image = radar_image.into_rgb8();

        // Copy the pixels of the radar image to the buffer
        let img_x = 8 * map_unit.x;
        let img_z = 8 * map_unit.z;
        for (radar_x, radar_z, pixel) in radar_image.enumerate_pixels() {
            image_buffer.put_pixel(img_x as u32 + radar_x, img_z as u32 + radar_z, pixel.clone());
        }
    }
    image_buffer.save_with_format("./layout.png", image::ImageFormat::Png).unwrap();
}
