use std::cmp::max;
use image::RgbaImage;
use num::clamp;


pub fn outline(img: &RgbaImage, thickness: u32) -> RgbaImage {
    let mut border_img = RgbaImage::new(
        img.width() + (thickness * 2),
        img.height() + (thickness * 2)
    );
    img.enumerate_pixels().for_each(|(x, y, pix)| {
        if pix.0[3] == 0 {
            return;
        }
        for bx in x..=(x + thickness * 2) {
            let bx = clamp(bx, 0, border_img.width() - 1);
            for by in y..=(y + thickness * 2) {
                let by = clamp(by, 0, border_img.height() - 1);
                let current_alpha = border_img.get_pixel(bx, by).0[3];
                border_img.get_pixel_mut(bx, by).0[3] = max(current_alpha, pix.0[3]);
            }
        }
    });
    border_img
}