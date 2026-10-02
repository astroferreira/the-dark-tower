//! Tool to generate 10 world maps and stitch them into a mosaic image

use std::error::Error;
use image::{ImageBuffer, Rgb};
use planet_generator::world::generate_world;
use planet_generator::explorer::export_base_map_image;

// Minimal 5x7 bitmap font for rendering alphanumeric labels
const FONT_5X7: [(char, [u8; 7]); 40] = [
    (' ', [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]),
    ('0', [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E]),
    ('1', [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E]),
    ('2', [0x0E, 0x11, 0x01, 0x06, 0x08, 0x10, 0x1F]),
    ('3', [0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E]),
    ('4', [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02]),
    ('5', [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E]),
    ('6', [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E]),
    ('7', [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08]),
    ('8', [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E]),
    ('9', [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C]),
    ('A', [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11]),
    ('B', [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E]),
    ('C', [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E]),
    ('D', [0x1C, 0x12, 0x11, 0x11, 0x11, 0x12, 0x1C]),
    ('E', [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F]),
    ('F', [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10]),
    ('G', [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F]),
    ('H', [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11]),
    ('I', [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E]),
    ('L', [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F]),
    ('M', [0x11, 0x1B, 0x15, 0x11, 0x11, 0x11, 0x11]),
    ('N', [0x11, 0x11, 0x19, 0x15, 0x13, 0x11, 0x11]),
    ('O', [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E]),
    ('P', [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10]),
    ('R', [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11]),
    ('S', [0x0E, 0x11, 0x10, 0x0E, 0x01, 0x11, 0x0E]),
    ('T', [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04]),
    ('U', [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E]),
    ('W', [0x11, 0x11, 0x11, 0x11, 0x15, 0x1B, 0x11]),
    ('Y', [0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04]),
    (':', [0x00, 0x0C, 0x0C, 0x00, 0x0C, 0x0C, 0x00]),
    ('-', [0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00]),
    ('#', [0x0A, 0x0A, 0x1F, 0x0A, 0x1F, 0x0A, 0x0A]),
    ('.', [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C]),
    ('(', [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02]),
    (')', [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08]),
    ('/', [0x01, 0x02, 0x02, 0x04, 0x08, 0x08, 0x10]),
    ('K', [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11]),
    ('D', [0x1C, 0x12, 0x11, 0x11, 0x11, 0x12, 0x1C]),
];

fn draw_char(
    img: &mut ImageBuffer<Rgb<u8>, Vec<u8>>,
    c: char,
    start_x: u32,
    start_y: u32,
    scale: u32,
    color: Rgb<u8>,
) {
    let upper = c.to_ascii_uppercase();
    if let Some((_, rows)) = FONT_5X7.iter().find(|(ch, _)| *ch == upper) {
        for (row_idx, &row) in rows.iter().enumerate() {
            for col_idx in 0..5 {
                if (row & (1 << (4 - col_idx))) != 0 {
                    for dy in 0..scale {
                        for dx in 0..scale {
                            let px = start_x + col_idx as u32 * scale + dx;
                            let py = start_y + row_idx as u32 * scale + dy;
                            if px < img.width() && py < img.height() {
                                img.put_pixel(px, py, color);
                            }
                        }
                    }
                }
            }
        }
    }
}

fn draw_text(
    img: &mut ImageBuffer<Rgb<u8>, Vec<u8>>,
    text: &str,
    start_x: u32,
    start_y: u32,
    scale: u32,
    color: Rgb<u8>,
) {
    let mut cur_x = start_x;
    for c in text.chars() {
        draw_char(img, c, cur_x, start_y, scale, color);
        cur_x += (5 + 1) * scale;
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let seeds: [u64; 10] = [
        42,
        101,
        202,
        333,
        404,
        555,
        777,
        888,
        999,
        12345,
    ];

    println!("Generating 10 diverse world maps...");

    let map_w = 512u32;
    let map_h = 256u32;
    let cols = 2u32;
    let rows = 5u32;

    let margin_x = 24u32;
    let margin_y = 32u32;
    let header_h = 28u32;
    let title_h = 60u32;

    let total_w = margin_x * (cols + 1) + map_w * cols;
    let total_h = title_h + margin_y * (rows + 1) + (map_h + header_h) * rows;

    // Background color: deep dark slate
    let bg_color = Rgb([15, 20, 28]);
    let border_color = Rgb([50, 65, 85]);
    let text_title_color = Rgb([235, 240, 255]);
    let text_sub_color = Rgb([160, 185, 220]);

    let mut mosaic = ImageBuffer::from_pixel(total_w, total_h, bg_color);

    // Draw Main Title Header
    draw_text(&mut mosaic, "PLANETARY WORLDS MOSAIC - 10 GEOLOGICAL GENERATIONS", margin_x + 8, 20, 2, text_title_color);

    for (idx, &seed) in seeds.iter().enumerate() {
        let col = (idx as u32) % cols;
        let row = (idx as u32) / cols;

        let cell_x = margin_x + col * (map_w + margin_x);
        let cell_y = title_h + margin_y + row * (map_h + header_h + margin_y);

        println!("Generating Planet #{}: Seed {} ({}/{})", idx + 1, seed, idx + 1, seeds.len());

        let temp_filename = format!("temp_mosaic_{}.png", seed);
        let world = generate_world(map_w as usize, map_h as usize, seed);
        export_base_map_image(&world, &temp_filename)?;

        // Load the generated image
        let img = image::open(&temp_filename)?.to_rgb8();
        let _ = std::fs::remove_file(&temp_filename);

        // Draw header text for this map
        let label = format!("WORLD #{}: SEED {}", idx + 1, seed);
        draw_text(&mut mosaic, &label, cell_x, cell_y, 2, text_sub_color);

        let map_start_y = cell_y + header_h;

        // Draw subtle border around map
        for bx in cell_x.saturating_sub(1)..=(cell_x + map_w) {
            if bx < total_w {
                if map_start_y > 0 { mosaic.put_pixel(bx, map_start_y - 1, border_color); }
                if map_start_y + map_h < total_h { mosaic.put_pixel(bx, map_start_y + map_h, border_color); }
            }
        }
        for by in map_start_y.saturating_sub(1)..=(map_start_y + map_h) {
            if by < total_h {
                if cell_x > 0 { mosaic.put_pixel(cell_x - 1, by, border_color); }
                if cell_x + map_w < total_w { mosaic.put_pixel(cell_x + map_w, by, border_color); }
            }
        }

        // Copy map pixels into mosaic
        for my in 0..map_h {
            for mx in 0..map_w {
                let pixel = img.get_pixel(mx, my);
                mosaic.put_pixel(cell_x + mx, map_start_y + my, *pixel);
            }
        }
    }

    let output_path = "worlds_mosaic.png";
    mosaic.save(output_path)?;
    println!("\nSuccessfully generated 10-world mosaic: {}", output_path);

    Ok(())
}
