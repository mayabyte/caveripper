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

    macro_rules! draw_pixel {
        ($x:expr, $z:expr, $r:expr, $g:expr, $b:expr) => {
            image_buffer.put_pixel(
                ((($x / 170.0) - min_map_x as f32) * 8.0) as u32,
                ((($z / 170.0) - min_map_z as f32) * 8.0) as u32,
                Pixel::from_channels($r, $g, $b, 255)
            );
        }
    }

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
        match spawn_point.contains.to_owned().into_inner() {
            None => continue,
            Some(SpawnObject::Ship) => {
                draw_pixel!(spawn_point.x, spawn_point.z, 255, 0, 0);
            },
            Some(SpawnObject::Hole) => {
                draw_pixel!(spawn_point.x, spawn_point.z, 0, 255, 0);
            },
            Some(SpawnObject::Geyser) => {
                draw_pixel!(spawn_point.x, spawn_point.z, 0, 0, 255);
            },
            Some(SpawnObject::Teki(_)) => {
                draw_pixel!(spawn_point.x, spawn_point.z, 255, 255, 0);
            },
            Some(SpawnObject::TekiBunch(teki_list)) => {
                for (_, (dx, _, dz)) in teki_list.iter() {
                    draw_pixel!(spawn_point.x + dx, spawn_point.z + dz, 255, 200, 0);
                }
            },
            Some(SpawnObject::PlantTeki(_) | SpawnObject::CapTeki(_, _)) => {
                draw_pixel!(spawn_point.x, spawn_point.z, 0, 160, 0);
            },
            Some(SpawnObject::Item(_)) => {
                draw_pixel!(spawn_point.x, spawn_point.z, 0, 255, 255);
            }
            _ => panic!("unrecognized drawable spawn item!"),
        }
    }

    // Draw seam teki
    for door in layout.map_units.iter().flat_map(|unit| unit.doors.iter()) {
        match door.borrow().seam_spawnpoint {
            None => continue,
            Some(SpawnObject::Teki(_)) => {
                let mut x = (((door.borrow().x) - min_map_x) * 8) as u32;
                let mut z = (((door.borrow().z) - min_map_z) * 8) as u32;
                match door.borrow().door_unit.direction {
                    0 | 2 => x += 4,
                    1 | 3 => z += 4,
                    _ => panic!("Invalid door direction in render"),
                }

                image_buffer.put_pixel(x, z, Pixel::from_channels(255, 0, 255, 255));
            },
            Some(SpawnObject::TekiDuplicate) => {/* do nothing */},
            _ => panic!("unrecognized seam teki!"),
        }
    }

    image_buffer.save_with_format("./caveripper_output/layout.png", image::ImageFormat::Png).unwrap();
}
