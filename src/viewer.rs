use minifb::{Key, MouseButton, MouseMode, Window, WindowOptions};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use std::f64::consts::PI;

use crate::{export, heightmap, plates};
use crate::plates::{Plate, PlateId};
use crate::tilemap::Tilemap;

/// View modes for the interactive viewer
#[derive(Clone, Copy, Debug, PartialEq)]
enum ViewMode {
    Terrain,      // 1 - Biome-colored terrain
    Shaded,       // 2 - Terrain with hillshading
    Globe,        // 3 - 3D globe projection
    Plates,       // 4 - Plate colors
    Stress,       // 5 - Stress at boundaries
    Heightmap,    // 6 - Raw heightmap (spectral colormap)
}

impl ViewMode {
    fn label(&self) -> &'static str {
        match self {
            ViewMode::Terrain => "Terrain (Biomes)",
            ViewMode::Shaded => "Shaded Terrain",
            ViewMode::Globe => "Globe",
            ViewMode::Plates => "Plate Colors",
            ViewMode::Stress => "Plate Stress",
            ViewMode::Heightmap => "Heightmap",
        }
    }
}

/// Cached planet data to avoid regenerating on view switch
struct PlanetData {
    heightmap: Tilemap<f32>,
    heightmap_normalized: Tilemap<f32>,
    plate_map: Tilemap<PlateId>,
    plates: Vec<Plate>,
    stress_map: Tilemap<f32>,
}

/// Run the interactive planet viewer.
/// Press 1-5 to switch views, R to regenerate, Escape to exit.
pub fn run_viewer(width: usize, height: usize, initial_seed: Option<u64>) {
    // Use a scale factor to fit reasonably on screen
    // Target ~800-1200 pixels on the larger dimension
    let target_size = 900;
    let scale = if width.max(height) > target_size {
        1
    } else {
        (target_size / width.max(height)).max(1)
    };

    let window_width = width * scale;
    let window_height = height * scale;

    let mut window = Window::new(
        "Planet Generator - 1-6: Views, R: Regenerate, Esc: Exit",
        window_width,
        window_height,
        WindowOptions {
            resize: false,
            scale: minifb::Scale::X1,
            ..WindowOptions::default()
        },
    )
    .expect("Failed to create window");

    // Limit to ~60fps
    window.set_target_fps(60);

    let mut seed = initial_seed.unwrap_or_else(rand::random);
    let mut planet = generate_planet_data(width, height, seed);
    let mut view_mode = ViewMode::Terrain;
    let mut buffer = render_view(&planet, view_mode, scale, 0.0, 0.0);

    println!("Viewer started. Controls:");
    println!("  1: Terrain (Biomes)");
    println!("  2: Shaded Terrain");
    println!("  3: Globe (drag to rotate, arrows for tilt)");
    println!("  4: Plate Colors");
    println!("  5: Plate Stress");
    println!("  6: Heightmap");
    println!("  R: Regenerate");
    println!("  Esc: Exit");

    // Globe rotation state
    let mut globe_rotation: f64 = 0.0;  // Longitude rotation
    let mut globe_tilt: f64 = 0.0;      // Latitude tilt
    let mut last_mouse_pos: Option<(f32, f32)> = None;
    let mut is_dragging = false;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let mut needs_redraw = false;

        // Check for R key to regenerate
        if window.is_key_pressed(Key::R, minifb::KeyRepeat::No) {
            seed = rand::random();
            println!("Regenerating with seed: {}", seed);
            planet = generate_planet_data(width, height, seed);
            needs_redraw = true;
        }

        // Check for view mode switches (1-6)
        let new_mode = if window.is_key_pressed(Key::Key1, minifb::KeyRepeat::No) {
            Some(ViewMode::Terrain)
        } else if window.is_key_pressed(Key::Key2, minifb::KeyRepeat::No) {
            Some(ViewMode::Shaded)
        } else if window.is_key_pressed(Key::Key3, minifb::KeyRepeat::No) {
            Some(ViewMode::Globe)
        } else if window.is_key_pressed(Key::Key4, minifb::KeyRepeat::No) {
            Some(ViewMode::Plates)
        } else if window.is_key_pressed(Key::Key5, minifb::KeyRepeat::No) {
            Some(ViewMode::Stress)
        } else if window.is_key_pressed(Key::Key6, minifb::KeyRepeat::No) {
            Some(ViewMode::Heightmap)
        } else {
            None
        };

        if let Some(mode) = new_mode {
            if mode != view_mode {
                view_mode = mode;
                println!("View: {}", view_mode.label());
                needs_redraw = true;
            }
        }

        // Handle globe rotation with mouse drag
        if view_mode == ViewMode::Globe {
            let mouse_down = window.get_mouse_down(MouseButton::Left);

            if let Some((mx, my)) = window.get_mouse_pos(MouseMode::Clamp) {
                if mouse_down {
                    if is_dragging {
                        if let Some((last_x, last_y)) = last_mouse_pos {
                            let dx = mx - last_x;
                            let dy = my - last_y;

                            // Horizontal drag rotates longitude
                            globe_rotation -= (dx as f64) * 0.01;
                            // Vertical drag tilts latitude
                            globe_tilt = (globe_tilt + (dy as f64) * 0.01).clamp(-PI / 2.0 + 0.1, PI / 2.0 - 0.1);

                            if dx.abs() > 0.5 || dy.abs() > 0.5 {
                                needs_redraw = true;
                            }
                        }
                    }
                    is_dragging = true;
                    last_mouse_pos = Some((mx, my));
                } else {
                    is_dragging = false;
                    last_mouse_pos = None;
                }
            }

            // Arrow keys for globe rotation
            if window.is_key_down(Key::Left) {
                globe_rotation += 0.05;
                needs_redraw = true;
            }
            if window.is_key_down(Key::Right) {
                globe_rotation -= 0.05;
                needs_redraw = true;
            }
            if window.is_key_down(Key::Up) {
                globe_tilt = (globe_tilt - 0.03).clamp(-PI / 2.0 + 0.1, PI / 2.0 - 0.1);
                needs_redraw = true;
            }
            if window.is_key_down(Key::Down) {
                globe_tilt = (globe_tilt + 0.03).clamp(-PI / 2.0 + 0.1, PI / 2.0 - 0.1);
                needs_redraw = true;
            }
        }

        if needs_redraw {
            buffer = render_view(&planet, view_mode, scale, globe_rotation, globe_tilt);
        }

        window
            .update_with_buffer(&buffer, window_width, window_height)
            .expect("Failed to update window");
    }
}

/// Generate planet data (all the maps we need)
fn generate_planet_data(width: usize, height: usize, seed: u64) -> PlanetData {
    println!("Generating planet with seed: {}...", seed);

    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    // Generate plates
    let (plate_map, plates) = plates::generate_plates(width, height, None, &mut rng);

    // Calculate stress
    let stress_map = plates::calculate_stress(&plate_map, &plates);

    // Generate heightmap
    let heightmap = heightmap::generate_heightmap(&plate_map, &plates, &stress_map, seed);
    let heightmap_normalized = heightmap::normalize_heightmap(&heightmap);

    println!("Done! Map size: {}x{}", width, height);

    PlanetData {
        heightmap,
        heightmap_normalized,
        plate_map,
        plates,
        stress_map,
    }
}

/// Render globe with full 3D rotation (longitude and latitude tilt)
fn render_globe_3d(
    heightmap: &Tilemap<f32>,
    _plate_map: &Tilemap<PlateId>,
    _plates: &[Plate],
    rotation: f64,
    tilt: f64,
    target_width: usize,
    target_height: usize,
) -> image::RgbImage {
    use image::{ImageBuffer, Rgb, RgbImage};

    // Make the globe fill the window better
    let size = target_width.min(target_height);
    let mut img: RgbImage = ImageBuffer::new(target_width as u32, target_height as u32);

    let radius = size as f64 / 2.0 - 10.0;
    let center_x = target_width as f64 / 2.0;
    let center_y = target_height as f64 / 2.0;

    // Light direction (from upper-right-front)
    let light_dir = normalize_vec3_f64(1.0, 1.0, 0.8);

    // Precompute rotation matrices
    let cos_rot = rotation.cos();
    let sin_rot = rotation.sin();
    let cos_tilt = tilt.cos();
    let sin_tilt = tilt.sin();

    for py in 0..target_height {
        for px in 0..target_width {
            let x = (px as f64 - center_x) / radius;
            let y = (center_y - py as f64) / radius;

            let r_squared = x * x + y * y;
            if r_squared > 1.0 {
                // Background - dark space
                img.put_pixel(px as u32, py as u32, Rgb([5, 5, 15]));
                continue;
            }

            // Z coordinate on sphere surface (pointing towards viewer)
            let z = (1.0 - r_squared).sqrt();

            // Apply inverse rotation to find the point on the original sphere
            // First apply inverse tilt (rotation around X axis)
            let y2 = y * cos_tilt + z * sin_tilt;
            let z2 = -y * sin_tilt + z * cos_tilt;

            // Then apply inverse longitude rotation (rotation around Y axis)
            let x3 = x * cos_rot - z2 * sin_rot;
            let z3 = x * sin_rot + z2 * cos_rot;

            // Convert the rotated point to latitude/longitude
            let lat = y2.asin();  // -PI/2 to PI/2
            let lon = x3.atan2(z3);  // -PI to PI

            // Normalize longitude to 0..2*PI
            let lon = ((lon % (2.0 * PI)) + 2.0 * PI) % (2.0 * PI);

            // Convert to map coordinates
            let map_x = (lon / (2.0 * PI) * heightmap.width as f64) as usize % heightmap.width;
            let map_y = ((0.5 - lat / PI) * heightmap.height as f64)
                .clamp(0.0, heightmap.height as f64 - 1.0) as usize;

            // Get height and color
            let height = *heightmap.get(map_x, map_y);

            // Use terrain colors based on height
            let base_color = terrain_color_for_height(height);

            // Calculate lighting (Lambert shading) using original surface normal
            let normal = (x, y, z);
            let diffuse = (normal.0 * light_dir.0 + normal.1 * light_dir.1 + normal.2 * light_dir.2)
                .max(0.0);

            // Ambient + diffuse lighting
            let ambient = 0.3;
            let light_intensity = ambient + (1.0 - ambient) * diffuse;

            // Apply lighting to color
            let r = ((base_color[0] as f64 * light_intensity).clamp(0.0, 255.0)) as u8;
            let g = ((base_color[1] as f64 * light_intensity).clamp(0.0, 255.0)) as u8;
            let b = ((base_color[2] as f64 * light_intensity).clamp(0.0, 255.0)) as u8;

            img.put_pixel(px as u32, py as u32, Rgb([r, g, b]));
        }
    }

    // Add atmosphere glow
    let glow_radius = radius * 1.15;
    for py in 0..target_height {
        for px in 0..target_width {
            let x = px as f64 - center_x;
            let y = py as f64 - center_y;
            let dist = (x * x + y * y).sqrt();

            if dist > radius && dist < glow_radius {
                let t = (dist - radius) / (glow_radius - radius);
                let glow_strength = (1.0 - t).powi(2) * 0.4;

                let pixel = img.get_pixel(px as u32, py as u32);
                let r = (pixel[0] as f64 + 100.0 * glow_strength).min(255.0) as u8;
                let g = (pixel[1] as f64 + 150.0 * glow_strength).min(255.0) as u8;
                let b = (pixel[2] as f64 + 255.0 * glow_strength).min(255.0) as u8;
                img.put_pixel(px as u32, py as u32, Rgb([r, g, b]));
            }
        }
    }

    img
}

fn normalize_vec3_f64(x: f64, y: f64, z: f64) -> (f64, f64, f64) {
    let len = (x * x + y * y + z * z).sqrt();
    (x / len, y / len, z / len)
}

/// Get terrain color based on height (similar to export.rs biome colors)
fn terrain_color_for_height(height: f32) -> [u8; 3] {
    if height < -500.0 {
        [20, 40, 80]       // Deep ocean
    } else if height < -100.0 {
        [30, 60, 120]      // Ocean
    } else if height < 0.0 {
        [60, 100, 150]     // Shallow water
    } else if height < 10.0 {
        [210, 190, 140]    // Beach
    } else if height < 40.0 {
        [80, 160, 60]      // Lowland
    } else if height < 80.0 {
        [100, 180, 80]     // Plains
    } else if height < 130.0 {
        [40, 120, 50]      // Forest
    } else if height < 200.0 {
        [110, 140, 70]     // Hills
    } else if height < 300.0 {
        [140, 130, 100]    // Highland
    } else if height < 450.0 {
        [120, 110, 100]    // Mountain
    } else {
        [240, 240, 245]    // Snowy peak
    }
}

/// Render the current view mode to a pixel buffer
fn render_view(planet: &PlanetData, mode: ViewMode, scale: usize, rotation: f64, tilt: f64) -> Vec<u32> {
    let map_width = planet.heightmap.width;
    let map_height = planet.heightmap.height;
    let out_width = map_width * scale;
    let out_height = map_height * scale;

    let img = match mode {
        ViewMode::Terrain => export::render_terrain_map(&planet.heightmap),
        ViewMode::Shaded => export::render_terrain_shaded(&planet.heightmap),
        ViewMode::Globe => render_globe_3d(
            &planet.heightmap,
            &planet.plate_map,
            &planet.plates,
            rotation,
            tilt,
            map_width,
            map_height,
        ),
        ViewMode::Plates => export::render_plate_map(&planet.plate_map, &planet.plates),
        ViewMode::Stress => export::render_stress_map(&planet.stress_map),
        ViewMode::Heightmap => export::render_heightmap(&planet.heightmap),
    };

    let img_width = img.width() as usize;
    let img_height = img.height() as usize;

    // Start with dark background
    let bg_color: u32 = (5 << 16) | (5 << 8) | 15; // Dark space color
    let mut buffer = vec![bg_color; out_width * out_height];

    // Calculate offset to center the image if it's smaller than the window
    let offset_x = (out_width.saturating_sub(img_width * scale)) / 2;
    let offset_y = (out_height.saturating_sub(img_height * scale)) / 2;

    // Scale and convert to u32 buffer, centered
    for iy in 0..img_height {
        for ix in 0..img_width {
            let pixel = img.get_pixel(ix as u32, iy as u32);
            let r = pixel[0] as u32;
            let g = pixel[1] as u32;
            let b = pixel[2] as u32;
            let color = (r << 16) | (g << 8) | b;

            // Draw scaled pixel
            for sy in 0..scale {
                for sx in 0..scale {
                    let ox = offset_x + ix * scale + sx;
                    let oy = offset_y + iy * scale + sy;
                    if ox < out_width && oy < out_height {
                        buffer[oy * out_width + ox] = color;
                    }
                }
            }
        }
    }

    buffer
}
