//! Debug tool for comparing erosion parameters visually
//! Generates a grid of shaded heightmaps with different erosion settings

use image::{ImageBuffer, Rgb, RgbImage};
use planet_generator::erosion::{self, ErosionParams, RiverErosionParams};
use planet_generator::tilemap::Tilemap;
use planet_generator::{climate, heightmap, plates};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

const WIDTH: usize = 512;
const HEIGHT: usize = 256;
const SEED: u64 = 42;

fn main() {
    println!("Generating erosion comparison grid...");

    // Generate base terrain once
    let mut rng = ChaCha8Rng::seed_from_u64(SEED);
    let (plate_map, plates_vec) = plates::generate_plates(WIDTH, HEIGHT, None, &mut rng);
    let stress_map = plates::calculate_stress(&plate_map, &plates_vec);
    let base_heightmap = heightmap::generate_heightmap(&plate_map, &plates_vec, &stress_map, SEED);
    let temperature = climate::generate_temperature(&base_heightmap, WIDTH, HEIGHT);

    // Define erosion variants to test
    // First variant uses standard ErosionParams for consistency with main generator
    let variants: Vec<(&str, Box<dyn Fn(&mut Tilemap<f32>, &mut ChaCha8Rng)>)> = vec![
        ("1. No Erosion", Box::new(|_hm: &mut Tilemap<f32>, _rng: &mut ChaCha8Rng| {})),

        ("2. Default Params", Box::new(|hm: &mut Tilemap<f32>, rng: &mut ChaCha8Rng| {
            // Use exactly the same params as planet_generator
            run_full_erosion(hm, &plate_map, &plates_vec, &stress_map, &temperature, rng, SEED);
        })),

        ("3. Rivers Only", Box::new(|hm: &mut Tilemap<f32>, _rng: &mut ChaCha8Rng| {
            // Rivers only (no hydraulic)
            run_rivers_only(hm, 50.0, 1.0, 15.0);
        })),

        ("4. Acc 100 Er 0.5", Box::new(|hm: &mut Tilemap<f32>, _rng: &mut ChaCha8Rng| {
            run_rivers_only(hm, 100.0, 0.5, 10.0);
        })),

        ("5. Acc 25 Er 0.8", Box::new(|hm: &mut Tilemap<f32>, _rng: &mut ChaCha8Rng| {
            run_rivers_only(hm, 25.0, 0.8, 15.0);
        })),

        ("6. Acc 10 Er 1.0", Box::new(|hm: &mut Tilemap<f32>, _rng: &mut ChaCha8Rng| {
            run_rivers_only(hm, 10.0, 1.0, 20.0);
        })),

        ("7. Dense Rivers", Box::new(|hm: &mut Tilemap<f32>, _rng: &mut ChaCha8Rng| {
            run_rivers_only(hm, 5.0, 0.8, 15.0);
        })),

        ("8. Hydraulic Only", Box::new(|hm: &mut Tilemap<f32>, rng: &mut ChaCha8Rng| {
            run_hydraulic_only(hm, rng);
        })),

        ("9. Rivers+Hydraulic", Box::new(|hm: &mut Tilemap<f32>, rng: &mut ChaCha8Rng| {
            run_rivers_only(hm, 50.0, 1.0, 15.0);
            run_hydraulic_only(hm, rng);
        })),
    ];

    // Generate images for each variant
    let mut images: Vec<(String, RgbImage)> = Vec::new();

    for (name, erosion_fn) in &variants {
        println!("  Processing: {}", name);
        let mut hm = base_heightmap.clone();
        let mut variant_rng = ChaCha8Rng::seed_from_u64(SEED);
        erosion_fn(&mut hm, &mut variant_rng);
        let img = render_shaded_heightmap(&hm);
        images.push((name.to_string(), img));
    }

    // Create 3x3 grid
    let grid = create_grid(&images, 3, 3);
    grid.save("erosion_comparison.png").expect("Failed to save grid");

    println!("Saved erosion_comparison.png");
}

/// Run full erosion pipeline with standard ErosionParams (same as planet_generator)
fn run_full_erosion(
    heightmap: &mut Tilemap<f32>,
    plate_map: &Tilemap<planet_generator::plates::PlateId>,
    plates: &[planet_generator::plates::Plate],
    stress_map: &Tilemap<f32>,
    temperature: &Tilemap<f32>,
    rng: &mut ChaCha8Rng,
    seed: u64,
) {
    let params = ErosionParams::default();
    erosion::simulate_erosion(heightmap, plate_map, plates, stress_map, temperature, &params, rng, seed);
}

/// Run only river erosion with custom params
fn run_rivers_only(
    heightmap: &mut Tilemap<f32>,
    source_min_acc: f32,
    erosion_rate: f32,
    capacity_factor: f32,
) {
    let hardness = Tilemap::new_with(heightmap.width, heightmap.height, 0.3f32);
    let river_params = RiverErosionParams {
        source_min_accumulation: source_min_acc,
        source_min_elevation: 100.0,
        erosion_rate,
        capacity_factor,
        max_erosion: 150.0,
        channel_width: 2,
        passes: 1,
        ..Default::default()
    };
    erosion::rivers::erode_rivers(heightmap, &hardness, &river_params);
}

/// Run only hydraulic erosion
fn run_hydraulic_only(
    heightmap: &mut Tilemap<f32>,
    rng: &mut ChaCha8Rng,
) {
    let hardness = Tilemap::new_with(heightmap.width, heightmap.height, 0.3f32);
    let params = ErosionParams::default();
    erosion::hydraulic::simulate(heightmap, &hardness, &params, rng);
}

fn render_shaded_heightmap(heightmap: &Tilemap<f32>) -> RgbImage {
    let width = heightmap.width;
    let height = heightmap.height;

    // Find height range
    let mut min_h = f32::MAX;
    let mut max_h = f32::MIN;
    for (_, _, &h) in heightmap.iter() {
        if h < min_h { min_h = h; }
        if h > max_h { max_h = h; }
    }
    let range = (max_h - min_h).max(1.0);

    let mut img = ImageBuffer::new(width as u32, height as u32);

    // Light direction (from upper-left)
    let light_x = -0.7f32;
    let light_y = -0.7f32;
    let light_z = 0.5f32;
    let light_len = (light_x * light_x + light_y * light_y + light_z * light_z).sqrt();
    let (lx, ly, lz) = (light_x / light_len, light_y / light_len, light_z / light_len);

    for y in 0..height {
        for x in 0..width {
            let h = *heightmap.get(x, y);
            let normalized = (h - min_h) / range;

            // Calculate surface normal from neighbors
            let h_left = if x > 0 { *heightmap.get(x - 1, y) } else { h };
            let h_right = if x < width - 1 { *heightmap.get(x + 1, y) } else { h };
            let h_up = if y > 0 { *heightmap.get(x, y - 1) } else { h };
            let h_down = if y < height - 1 { *heightmap.get(x, y + 1) } else { h };

            // Normal from height differences (scale for visibility)
            let scale = 0.01;
            let nx = (h_left - h_right) * scale;
            let ny = (h_up - h_down) * scale;
            let nz = 1.0f32;
            let nlen = (nx * nx + ny * ny + nz * nz).sqrt();
            let (nx, ny, nz) = (nx / nlen, ny / nlen, nz / nlen);

            // Diffuse lighting
            let diffuse = (nx * lx + ny * ly + nz * lz).max(0.0);
            let ambient = 0.3;
            let lighting = (ambient + (1.0 - ambient) * diffuse).min(1.0);

            // Color based on elevation
            let color = if h < 0.0 {
                // Ocean - blue, darker with depth
                let depth = (-h / 4000.0).min(1.0);
                let blue = (200.0 - depth * 150.0) as u8;
                let green = (150.0 - depth * 100.0) as u8;
                Rgb([30, green, blue])
            } else {
                // Land - green to brown to white
                let base = if normalized < 0.3 {
                    // Low land - green
                    Rgb([80, 140, 60])
                } else if normalized < 0.6 {
                    // Mid elevation - brown/tan
                    let t = (normalized - 0.3) / 0.3;
                    let r = (80.0 + t * 80.0) as u8;
                    let g = (140.0 - t * 60.0) as u8;
                    let b = (60.0 - t * 20.0) as u8;
                    Rgb([r, g, b])
                } else if normalized < 0.85 {
                    // High elevation - gray rock
                    let t = (normalized - 0.6) / 0.25;
                    let v = (160.0 - t * 40.0) as u8;
                    Rgb([v, v - 10, v - 20])
                } else {
                    // Peak - snow
                    Rgb([240, 240, 245])
                };

                // Apply lighting
                let r = (base[0] as f32 * lighting) as u8;
                let g = (base[1] as f32 * lighting) as u8;
                let b = (base[2] as f32 * lighting) as u8;
                Rgb([r, g, b])
            };

            img.put_pixel(x as u32, y as u32, color);
        }
    }

    img
}

fn create_grid(images: &[(String, RgbImage)], cols: usize, rows: usize) -> RgbImage {
    if images.is_empty() {
        return ImageBuffer::new(1, 1);
    }

    let cell_width = images[0].1.width();
    let cell_height = images[0].1.height();
    let label_height = 20u32;
    let total_cell_height = cell_height + label_height;

    let grid_width = cell_width * cols as u32;
    let grid_height = total_cell_height * rows as u32;

    let mut grid: RgbImage = ImageBuffer::from_pixel(grid_width, grid_height, Rgb([40, 40, 40]));

    for (idx, (name, img)) in images.iter().enumerate() {
        let col = idx % cols;
        let row = idx / cols;
        if row >= rows {
            break;
        }

        let x_offset = col as u32 * cell_width;
        let y_offset = row as u32 * total_cell_height + label_height;

        // Copy image
        for y in 0..cell_height {
            for x in 0..cell_width {
                let pixel = img.get_pixel(x, y);
                grid.put_pixel(x_offset + x, y_offset + y, *pixel);
            }
        }

        // Draw simple label background
        for y in 0..label_height {
            for x in 0..cell_width {
                grid.put_pixel(x_offset + x, row as u32 * total_cell_height + y, Rgb([30, 30, 30]));
            }
        }

        // Draw label text (simple pixel font approximation - just show first few chars)
        draw_text(&mut grid, x_offset + 5, row as u32 * total_cell_height + 5, name);
    }

    grid
}

// Simple 5x7 bitmap font for basic characters
fn get_char_bitmap(c: char) -> [u8; 7] {
    match c {
        '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111],
        '3' => [0b01110, 0b10001, 0b00001, 0b00110, 0b00001, 0b10001, 0b01110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
        '6' => [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100],
        'A' | 'a' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'B' | 'b' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        'C' | 'c' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
        'D' | 'd' => [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
        'E' | 'e' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'F' | 'f' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
        'G' | 'g' => [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110],
        'H' | 'h' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'I' | 'i' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'J' | 'j' => [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100],
        'K' | 'k' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
        'L' | 'l' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' | 'm' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
        'N' | 'n' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
        'O' | 'o' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' | 'p' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'Q' | 'q' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101],
        'R' | 'r' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'S' | 's' => [0b01110, 0b10001, 0b10000, 0b01110, 0b00001, 0b10001, 0b01110],
        'T' | 't' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'U' | 'u' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'V' | 'v' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        'W' | 'w' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001],
        'X' | 'x' => [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
        'Y' | 'y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        'Z' | 'z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
        '.' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100],
        ':' => [0b00000, 0b01100, 0b01100, 0b00000, 0b01100, 0b01100, 0b00000],
        '+' => [0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000],
        '-' => [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000],
        '=' => [0b00000, 0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000],
        _ => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000],
    }
}

fn draw_text(img: &mut RgbImage, x: u32, y: u32, text: &str) {
    let white = Rgb([255, 255, 255]);
    let char_width = 6u32;
    let char_height = 7u32;

    for (i, c) in text.chars().enumerate() {
        let cx = x + (i as u32 * char_width);
        if cx + 5 >= img.width() {
            break;
        }

        let bitmap = get_char_bitmap(c);
        for (row, &bits) in bitmap.iter().enumerate() {
            for col in 0..5 {
                if (bits >> (4 - col)) & 1 == 1 {
                    let px = cx + col;
                    let py = y + row as u32;
                    if px < img.width() && py < img.height() {
                        img.put_pixel(px, py, white);
                    }
                }
            }
        }
    }
}
