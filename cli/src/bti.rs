use crate::util::{read_u16, read_u32};

type Color = [u8; 4];

pub struct BtiImage {
    pub width: u16,
    pub height: u16,
    data: Vec<Color>,
}

impl BtiImage {
    pub fn decode(data: &[u8]) -> Self {
        let format = data[0x0].clamp(0, 11);
        let width = read_u16(data, 0x2);
        let height = read_u16(data, 0x4);
        let palette_format = data[0x9];
        let num_colors = read_u16(data, 0xA);
        let palette_data_offset = read_u32(data, 0xC);
        let mipmap_count = data[0x18];
        let img_data_offset = read_u32(data, 0x1C);
        let blocks_wide = (width + BLOCK_WIDTHS[format as usize] - 1) / BLOCK_WIDTHS[format as usize];
        let blocks_tall = (height + BLOCK_HEIGHTS[format as usize] - 1) / BLOCK_HEIGHTS[format as usize];
        let mut img_data_size = blocks_wide * blocks_tall * BLOCK_DATA_SIZE[format as usize];

        let mut curr_mipmap_size = img_data_size;
        for _ in 0..mipmap_count-1 {
            curr_mipmap_size /= 4;
            img_data_size += curr_mipmap_size;
        }

        let img_data = &data[img_data_offset as usize .. img_data_offset as usize + img_data_size as usize];
        let palette_data = &data[palette_data_offset as usize .. palette_data_offset as usize + (num_colors*2) as usize];

        let mut decoded_data = vec![[0,0,0,0]; (width * height) as usize];
        let colors = decode_palettes(palette_data, palette_format, num_colors, format);

        let mut offset = 0;
        let mut block_x = 0;
        let mut block_y = 0;
        let block_size = BLOCK_DATA_SIZE[format as usize] as usize;
        while block_y < height as usize {
            let decoded_pixels = match format {
                0  => decode_i4_block(img_data, offset, block_size),
                1  => decode_i8_block(img_data, offset, block_size),
                2  => decode_ia4_block(img_data, offset, block_size),
                3  => decode_ia8_block(img_data, offset, block_size),
                4  => decode_rgb565_block(img_data, offset, block_size),
                5  => decode_rgb5a3_block(img_data, offset, block_size),
                6  => decode_rgba32_block(img_data, offset),
                8  => decode_c4_block(img_data, offset, block_size, &colors),
                9  => decode_c8_block(img_data, offset, block_size, &colors),
                10  => decode_c14x2_block(img_data, offset, block_size, &colors),
                11 => decode_cmpr_block(img_data, offset),
                _ => panic!("Unknown image format {format}"),
            };

            for (i, pixel) in decoded_pixels.iter().enumerate() {
                let x_in_block = i % BLOCK_WIDTHS[format as usize] as usize;
                let y_in_block = i / BLOCK_WIDTHS[format as usize] as usize;
                let x = block_x + x_in_block;
                let y = block_y + y_in_block;
                if x >= width as usize || y >= height as usize {
                    continue;
                }
                decoded_data[(x + y*width as usize)] = *pixel;
            }

            offset += block_size;
            block_x += BLOCK_WIDTHS[format as usize] as usize;
            if block_x >= width as usize {
                block_x = 0;
                block_y += BLOCK_HEIGHTS[format as usize] as usize;
            }
        }

        BtiImage { width, height, data: decoded_data }
    }

    pub fn pixels(&self) -> impl Iterator<Item=&[u8;4]> {
        self.data.iter()
    }
}

const BLOCK_WIDTHS:  [u16; 11] = [8, 8, 8, 4, 4, 4, 4, 8, 8, 4, 8];
const BLOCK_HEIGHTS: [u16; 11] = [8, 4, 4, 4, 4, 4, 4, 8, 4, 4, 8];
const BLOCK_DATA_SIZE: [u16; 11] = [32, 32, 32, 32, 32, 32, 64, 32, 32, 32, 32];

fn decode_palettes(palette_data: &[u8], palette_format: u8, num_colors: u16, img_format: u8) -> Vec<Color> {
    // Only these 3 formats use palettes
    if ![8,9,10].contains(&img_format) {
        return vec![];
    }

    let mut colors = Vec::with_capacity(num_colors as usize);
    for o in 0..num_colors {
        let raw_color = read_u16(palette_data, (o*2) as u32);
        let color = match palette_format {
            0 => ia8_to_color(raw_color),
            1 => rgb565_to_color(raw_color),
            2 => rgb5a3_to_color(raw_color),
            _ => panic!("Invalid palette format: {palette_format}"),
        };
        colors.push(color);
    }

    colors
}

fn decode_i4_block(img_data: &[u8], offset: usize, block_data_size: usize) -> Vec<Color> {
    let mut pixels = Vec::with_capacity(block_data_size * 2);
    for i in 0..block_data_size {
        let b = img_data[offset + i];
        pixels.push(i4_to_color((b>>4) & 0xF));
        pixels.push(i4_to_color(b & 0xF));
    }
    pixels
}

const fn i4_to_color(c: u8) -> Color {
    [
        swizzle_4_to_8(c),
        swizzle_4_to_8(c),
        swizzle_4_to_8(c),
        swizzle_4_to_8(c),
    ]
}

fn decode_i8_block(img_data: &[u8], offset: usize, block_data_size: usize) -> Vec<Color> {
    let mut pixels = Vec::with_capacity(block_data_size);
    for i in 0..block_data_size {
        pixels.push(i8_to_color(img_data[offset + i]));
    }
    pixels
}

const fn i8_to_color(c: u8) -> Color {
    [c,c,c,c]
}

fn decode_ia4_block(img_data: &[u8], offset: usize, block_data_size: usize) -> Vec<Color> {
    let mut pixels = Vec::with_capacity(block_data_size);
    for i in 0..block_data_size {
        pixels.push(ia4_to_color(img_data[offset + i]));
    }
    pixels
}

const fn ia4_to_color(c: u8) -> Color {
    [
        swizzle_4_to_8(c & 0xF),
        swizzle_4_to_8(c & 0xF),
        swizzle_4_to_8(c & 0xF),
        swizzle_4_to_8((c >> 4) & 0xF),
    ]
}

fn decode_ia8_block(img_data: &[u8], offset: usize, block_data_size: usize) -> Vec<Color> {
    let mut pixels = Vec::with_capacity(block_data_size / 2);
    for i in 0..block_data_size / 2 {
        pixels.push(ia8_to_color(read_u16(img_data, (offset + i*2) as u32)));
    }
    pixels
}

const fn ia8_to_color(c: u16) -> Color {
    [
        (c & 0xFF) as u8,
        (c & 0xFF) as u8,
        (c & 0xFF) as u8,
        ((c >> 8) & 0xFF) as u8,
    ]
}

fn decode_rgb565_block(img_data: &[u8], offset: usize, block_data_size: usize) -> Vec<Color> {
    let mut pixels = Vec::with_capacity(block_data_size / 2);
    for i in 0..block_data_size / 2 {
        pixels.push(rgb565_to_color(read_u16(img_data, (offset + i*2) as u32)));
    }
    pixels
}

const fn rgb565_to_color(c: u16) -> Color {
    [
        swizzle_5_to_8(((c >> 11) & 0x1F) as u8),
        swizzle_6_to_8(((c >> 5) & 0x3F) as u8),
        swizzle_5_to_8((c & 0x1F) as u8),
        255
    ]
}

fn decode_rgb5a3_block(img_data: &[u8], offset: usize, block_data_size: usize) -> Vec<Color> {
    let mut pixels = Vec::with_capacity(block_data_size / 2);
    for i in 0..block_data_size / 2 {
        pixels.push(rgb5a3_to_color(read_u16(img_data, (offset + i*2) as u32)));
    }
    pixels
}

const fn rgb5a3_to_color(c: u16) -> Color {
    if c & 0x8000 == 0 {
        [
            swizzle_4_to_8(((c >> 8) & 0xF) as u8),
            swizzle_4_to_8(((c >> 4) & 0xF) as u8),
            swizzle_4_to_8((c & 0xF) as u8),
            swizzle_3_to_8(((c >> 12) & 0x7) as u8),
        ]
    }
    else {
        [
            swizzle_5_to_8(((c >> 10) & 0x1F) as u8),
            swizzle_5_to_8(((c >> 5) & 0x1F) as u8),
            swizzle_5_to_8((c & 0x1F) as u8),
            255
        ]
    }
}

fn decode_rgba32_block(img_data: &[u8], offset: usize) -> Vec<Color> {
    let mut colors = Vec::with_capacity(16);
    for i in 0..16 {
        let color = [
            img_data[offset + i*2],
            img_data[offset + (i*2) + 1],
            img_data[offset + (i*2) + 32],
            img_data[offset + (i*2) + 33],
        ];
        colors.push(color);
    }
    colors
}

fn decode_c4_block(img_data: &[u8], offset: usize, block_data_size: usize, palette: &Vec<Color>) -> Vec<Color> {
    let mut colors = Vec::with_capacity(block_data_size * 2);
    for i in 0..block_data_size {
        for nibble in 0..2 {
            let color_index = (img_data[offset + i] >> ((1 - nibble) * 4)) & 0xF;
            if color_index as usize > palette.len() {
                colors.push([0,0,0,0]);  // Past the edge of the image
            }
            else {
                colors.push(palette[color_index as usize]);
            }
        }
    }
    colors
}

fn decode_c8_block(img_data: &[u8], offset: usize, block_data_size: usize, palette: &Vec<Color>) -> Vec<Color> {
    let mut colors = Vec::with_capacity(block_data_size);
    for i in 0..block_data_size {
        let color_index = img_data[offset + i];
        if color_index as usize > palette.len() {
            colors.push([0,0,0,0]);  // Past the edge of the image
        }
        else {
            colors.push(palette[color_index as usize]);
        }
    }
    colors
}

fn decode_c14x2_block(img_data: &[u8], offset: usize, block_data_size: usize, palette: &Vec<Color>) -> Vec<Color> {
    let mut colors = Vec::with_capacity(block_data_size / 2);
    for i in 0..block_data_size / 2 {
        let color_index = read_u16(img_data, (offset + i) as u32) & 0x3FFF;
        if color_index as usize > palette.len() {
            colors.push([0,0,0,0]);  // Past the edge of the image
        }
        else {
            colors.push(palette[color_index as usize]);
        }
    }
    colors
}

fn decode_cmpr_block(img_data: &[u8], offset: usize) -> Vec<Color> {
    let mut colors = vec![[0,0,0,0]; 64];
    let mut sub_block_offset = offset;
    for sub_block in 0..4 {
        let x = (sub_block % 2) * 4;
        let y = (sub_block / 2) * 4;
        let color0 = read_u16(img_data, sub_block_offset as u32);
        let color1 = read_u16(img_data, sub_block_offset as u32 + 2);
        let palette = get_interpolated_cmpr_colors(color0, color1);

        let color_indexes = read_u32(img_data, sub_block_offset as u32 + 4);
        for i in 0..16 {
            let color_index = (color_indexes >> ((15-i)*2)) & 3;
            let color = palette[color_index as usize];
            let sub_x = i % 4;
            let sub_y = i / 4;
            let pixel_index = x + (y * 8) + sub_x + (sub_y * 8);
            colors[pixel_index] = color;
        }

        sub_block_offset += 8;
    }
    colors
}

const fn get_interpolated_cmpr_colors(c1b: u16, c2b: u16) -> [Color; 4] {
    let c1 = rgb565_to_color(c1b);
    let c2 = rgb565_to_color(c2b);
    if c1b > c2b {
        [
            c1,
            c2,
            [(2*c1[0] + c2[0]) / 3, (c1[1] + c2[1]) / 3, (c1[2] + c2[2]) / 3, 255],
            [(c1[0] + 2*c2[0]) / 3, (c1[1] + 2*c2[1]) / 3, (c1[2] + 2*c2[2]) / 3, 255]
        ]
    }
    else {
        [
            c1,
            c2,
            [c1[0]/2 + c2[0]/2, c1[1]/2 + c2[1]/2, c1[2]/2 + c2[2]/2, 255],
            [0,0,0,0]
        ]
    }
}

const fn swizzle_3_to_8(b: u8) -> u8 {
    (b << 5) | (b << 2) | (b >> 1)
}

const fn swizzle_4_to_8(b: u8) -> u8 {
    (b << 4) | b
}

const fn swizzle_5_to_8(b: u8) -> u8 {
    (b << 3) | (b >> 2)
}

const fn swizzle_6_to_8(b: u8) -> u8 {
    (b << 2) | (b >> 4)
}
