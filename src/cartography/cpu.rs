//! High-performance CPU software shader pipeline for terrain rendering and shaded relief
//!
//! Focuses purely on natural physical terrain: multi-directional Eduard Imhof Swiss hillshading,
//! elevation-driven hypsometric tinting (eliminating horizontal latitude striping),
//! sharp alpine cordilleras, coastal shelf bathymetry, glacial fjords, and vector river ribbons.

use image::{ImageBuffer, Rgb};
use noise::{NoiseFn, Perlin, Seedable};
use rayon::prelude::*;

use crate::biomes::ExtendedBiome;
use crate::tilemap::Tilemap;
use crate::world::WorldData;
use super::params::{CartographyParams, PaperStyle};
use super::decorations::{
    render_rhumb_lines, render_graticule, render_compass_rose, render_vintage_border,
    find_ocean_centers, draw_line_alpha, INK_PRIMARY, INK_SEPIA,
};

/// Color triplet (RGB floats in 0.0 - 1.0)
#[derive(Clone, Copy, Debug)]
pub struct Color3(pub f32, pub f32, pub f32);

impl Color3 {
    pub fn new(r: f32, g: f32, b: f32) -> Self {
        Self(r, g, b)
    }

    pub fn from_u8(r: u8, g: u8, b: u8) -> Self {
        Self(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
    }

    pub fn to_rgb(&self) -> Rgb<u8> {
        Rgb([
            (self.0.clamp(0.0, 1.0) * 255.0).round() as u8,
            (self.1.clamp(0.0, 1.0) * 255.0).round() as u8,
            (self.2.clamp(0.0, 1.0) * 255.0).round() as u8,
        ])
    }

    pub fn lerp(&self, other: Color3, t: f32) -> Color3 {
        let t = t.clamp(0.0, 1.0);
        Color3(
            self.0 + (other.0 - self.0) * t,
            self.1 + (other.1 - self.1) * t,
            self.2 + (other.2 - self.2) * t,
        )
    }

    pub fn multiply(&self, other: Color3) -> Color3 {
        Color3(self.0 * other.0, self.1 * other.1, self.2 * other.2)
    }
}

/// 1D Catmull-Rom cubic spline interpolation with overshoot clamping
#[inline(always)]
pub fn catmull_rom_1d(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    let t2 = t * t;
    let t3 = t2 * t;
    let val = 0.5 * ((2.0 * p1) +
           (-p0 + p2) * t +
           (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2 +
           (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3);
    // Prevent cubic spline overshoot (crucial across sharp bathymetric drops and coastlines)
    let min_val = p1.min(p2).min(p0.min(p3));
    let max_val = p1.max(p2).max(p0.max(p3));
    val.clamp(min_val, max_val)
}

/// Sample continuous sub-pixel elevation or scalar field using Catmull-Rom bicubic spline
/// with periodic longitudinal (X) wrapping and clamped latitudinal (Y) boundaries.
#[inline(always)]
pub fn sample_tilemap_catmull_rom(map: &Tilemap<f32>, u: f32, v: f32) -> f32 {
    let w = map.width as i32;
    let h = map.height as i32;

    let x0 = u.floor() as i32;
    let y0 = v.floor() as i32;
    let tx = u - x0 as f32;
    let ty = v - y0 as f32;

    let mut col_vals = [0.0f32; 4];
    for (j, dy) in (-1..=2).enumerate() {
        let y_sample = (y0 + dy).clamp(0, h - 1) as usize;
        let p0 = *map.get((x0 - 1).rem_euclid(w) as usize, y_sample);
        let p1 = *map.get(x0.rem_euclid(w) as usize, y_sample);
        let p2 = *map.get((x0 + 1).rem_euclid(w) as usize, y_sample);
        let p3 = *map.get((x0 + 2).rem_euclid(w) as usize, y_sample);
        col_vals[j] = catmull_rom_1d(p0, p1, p2, p3, tx);
    }

    catmull_rom_1d(col_vals[0], col_vals[1], col_vals[2], col_vals[3], ty)
}

/// Compute exact Euclidean distance from coastline for all cells using 8-point vector propagation
pub fn compute_water_coast_distance(
    heightmap: &Tilemap<f32>,
    water_depth: &Tilemap<f32>,
    max_dist: usize,
) -> Tilemap<f32> {
    let width = heightmap.width;
    let height = heightmap.height;
    let max_d_f32 = max_dist as f32;

    let is_land = |x: usize, y: usize| -> bool {
        let h = *heightmap.get(x, y);
        let wd = *water_depth.get(x, y);
        h >= 0.0 && wd <= 0.5
    };

    // Store closest coast offset vector (dx, dy)
    let mut vec_map = vec![(f32::MAX, f32::MAX); width * height];

    for y in 0..height {
        for x in 0..width {
            if !is_land(x, y) {
                let borders_land = heightmap.neighbors_8(x, y).into_iter().any(|(nx, ny)| is_land(nx, ny));
                if borders_land {
                    vec_map[y * width + x] = (0.0, 0.0);
                }
            }
        }
    }

    // Helper to test neighbor vector candidate
    let update_cell = |vecs: &mut [(f32, f32)], x: usize, y: usize, nx: usize, ny: usize, offset_x: f32, offset_y: f32| {
        let (ndx, ndy) = vecs[ny * width + nx];
        if ndx != f32::MAX {
            let cand_dx = ndx + offset_x;
            let cand_dy = ndy + offset_y;
            let cand_dist_sq = cand_dx * cand_dx + cand_dy * cand_dy;
            let (cur_dx, cur_dy) = vecs[y * width + x];
            let cur_dist_sq = if cur_dx == f32::MAX { f32::MAX } else { cur_dx * cur_dx + cur_dy * cur_dy };
            if cand_dist_sq < cur_dist_sq {
                vecs[y * width + x] = (cand_dx, cand_dy);
            }
        }
    };

    // Forward pass (top to bottom, left to right)
    for y in 0..height {
        for x in 0..width {
            if is_land(x, y) { continue; }
            let x_left = if x == 0 { width - 1 } else { x - 1 };
            let x_right = if x + 1 == width { 0 } else { x + 1 };
            if y > 0 {
                update_cell(&mut vec_map, x, y, x_left, y - 1, -1.0, -1.0);
                update_cell(&mut vec_map, x, y, x, y - 1, 0.0, -1.0);
                update_cell(&mut vec_map, x, y, x_right, y - 1, 1.0, -1.0);
            }
            update_cell(&mut vec_map, x, y, x_left, y, -1.0, 0.0);
        }
    }

    // Backward pass (bottom to top, right to left)
    for y in (0..height).rev() {
        for x in (0..width).rev() {
            if is_land(x, y) { continue; }
            let x_left = if x == 0 { width - 1 } else { x - 1 };
            let x_right = if x + 1 == width { 0 } else { x + 1 };
            update_cell(&mut vec_map, x, y, x_right, y, 1.0, 0.0);
            if y + 1 < height {
                update_cell(&mut vec_map, x, y, x_left, y + 1, -1.0, 1.0);
                update_cell(&mut vec_map, x, y, x, y + 1, 0.0, 1.0);
                update_cell(&mut vec_map, x, y, x_right, y + 1, 1.0, 1.0);
            }
        }
    }

    // Convert to smooth continuous Euclidean distance map, bounded cleanly
    let mut dist_map = Tilemap::new_with(width, height, max_d_f32);
    for y in 0..height {
        for x in 0..width {
            if is_land(x, y) {
                dist_map.set(x, y, 0.0);
            } else {
                let (dx, dy) = vec_map[y * width + x];
                if dx != f32::MAX {
                    dist_map.set(x, y, ((dx * dx + dy * dy).sqrt() + 0.5).min(max_d_f32));
                }
            }
        }
    }

    dist_map
}

/// Compute natural physical terrain color based on elevation, moisture, and slope.
/// Eliminates harsh horizontal latitude banding in favor of real physical geography.
pub fn compute_physical_terrain_color(
    elevation: f32,
    moisture: f32,
    slope: f32,
    ny: f64,
) -> Color3 {
    let lat_factor = ((ny - 0.5).abs() * 2.0) as f32; // 0.0 at equator, 1.0 at poles

    // Snowline altitude drops naturally from ~3200m at equator to ~550m at poles
    let snowline = 3200.0 - lat_factor * 2650.0;

    if elevation >= snowline {
        // High Glacial Ice & Perennial Snowcaps
        let snow_color = Color3::from_u8(248, 250, 254);
        let rock_shadow = Color3::from_u8(172, 180, 194);
        if slope > 30.0 {
            snow_color.lerp(rock_shadow, 0.45)
        } else {
            snow_color
        }
    } else if elevation >= 1350.0 {
        // High Alpine Mountain Zone: Weathered granite, rocky crags, scree slopes
        let granite = Color3::from_u8(150, 144, 136);
        let dark_crag = Color3::from_u8(116, 108, 102);
        let alpine_meadow = Color3::from_u8(178, 174, 134);
        
        let t_elevation = ((elevation - 1350.0) / (snowline - 1350.0).max(100.0)).clamp(0.0, 1.0);
        let base_rock = alpine_meadow.lerp(granite, t_elevation);
        if slope > 22.0 {
            base_rock.lerp(dark_crag, ((slope - 22.0) / 35.0).min(0.70))
        } else {
            base_rock
        }
    } else if elevation >= 550.0 {
        // Montane Belts, High Plateaus & Highlands: Terracotta, sandstone ochres, montane forest
        let plateau_ochre = Color3::from_u8(206, 174, 124);
        let montane_forest = Color3::from_u8(138, 156, 106);
        let highland_slate = Color3::from_u8(166, 152, 136);

        let t = ((elevation - 550.0) / 800.0).clamp(0.0, 1.0);
        let veg = (moisture - 0.40).clamp(0.0, 0.5) * 2.0;
        let base = plateau_ochre.lerp(montane_forest, veg);
        base.lerp(highland_slate, t * 0.50)
    } else if elevation >= 150.0 {
        // Rolling Plains, Steppes & Prairies: Golden amber, warm straw, fertile grasslands
        let golden_prairie = Color3::from_u8(218, 194, 134);
        let temperate_grass = Color3::from_u8(168, 178, 114);
        let arid_steppe = Color3::from_u8(228, 202, 144);

        let t = ((elevation - 150.0) / 400.0).clamp(0.0, 1.0);
        let base_plains = if moisture > 0.48 {
            golden_prairie.lerp(temperate_grass, ((moisture - 0.48) * 2.5).min(1.0))
        } else if moisture < 0.35 {
            golden_prairie.lerp(arid_steppe, ((0.35 - moisture) * 2.5).min(1.0))
        } else {
            golden_prairie
        };

        let plateau_tint = Color3::from_u8(206, 174, 124);
        base_plains.lerp(plateau_tint, t * 0.45)
    } else {
        // Coastal Lowlands, Valleys & Floodplains (0 - 150m): Pastoral greens, lush river meadows
        let coastal_sand = Color3::from_u8(226, 206, 156);
        let river_meadow = Color3::from_u8(146, 176, 112);
        let deep_forest = Color3::from_u8(118, 152, 92);

        let moist_t = (moisture - 0.38).clamp(0.0, 0.6) * 1.6;
        let lush_green = river_meadow.lerp(deep_forest, moist_t);

        if elevation < 20.0 {
            // Near coast: gentle transition to coastal sand
            lush_green.lerp(coastal_sand, (1.0 - elevation / 20.0) * 0.55)
        } else {
            let golden_prairie = Color3::from_u8(218, 194, 134);
            let t = ((elevation - 20.0) / 130.0).clamp(0.0, 1.0);
            lush_green.lerp(golden_prairie, t * 0.45)
        }
    }
}

/// Sample 3D cylindrical continuous multi-octave micro-fractal noise
#[inline(always)]
fn sample_micro_fractal_noise_3d(
    noise: &Perlin,
    nx: f64,
    ny: f64,
    base_freq: f64,
) -> f32 {
    let theta = nx * std::f64::consts::TAU;
    let cx = theta.cos();
    let cz = theta.sin();
    let cy = ny * 2.0;

    let o1 = noise.get([cx * base_freq, cy * base_freq, cz * base_freq]);
    let o2 = noise.get([cx * (base_freq * 2.1), cy * (base_freq * 2.1), cz * (base_freq * 2.1)]) * 0.5;
    let o3 = noise.get([cx * (base_freq * 4.3), cy * (base_freq * 4.3), cz * (base_freq * 4.3)]) * 0.25;

    ((o1 + o2 + o3) / 1.75) as f32
}

/// Sample continuous elevation with procedural micro-fractal detail in mountain and upland zones
#[inline(always)]
fn sample_elevation_with_microdetail(
    heightmap: &Tilemap<f32>,
    micro_fractal: &Perlin,
    sim_u: f32,
    sim_v: f32,
    nx: f64,
    ny: f64,
    avg_scale: f32,
) -> f32 {
    let base_h = sample_tilemap_catmull_rom(heightmap, sim_u, sim_v);
    if base_h <= 0.0 || avg_scale <= 1.0 {
        return base_h;
    }

    let mountain_factor = ((base_h - 120.0) / 700.0).clamp(0.0, 1.0);
    if mountain_factor <= 0.0 {
        return base_h;
    }

    let freq = 18.0 * (avg_scale as f64).sqrt();
    let m1 = sample_micro_fractal_noise_3d(micro_fractal, nx, ny, freq);
    let m2 = sample_micro_fractal_noise_3d(micro_fractal, nx * 2.2 + 19.3, ny * 2.2 + 7.1, freq * 2.3) * 0.5;
    let micro_noise = m1 + m2;

    let amp = mountain_factor.powf(1.3) * 90.0;
    base_h + micro_noise * amp
}

/// Render the complete antique cartography map on CPU using Rayon with procedural super-resolution upscaling
pub fn render_cartography_cpu(
    world: &WorldData,
    params: &CartographyParams,
) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
    let sim_w = world.width;
    let sim_h = world.height;

    // Determine target resolution from parameters
    let upscale_factor = params.upscale_factor.max(1);
    let (out_w, out_h) = match params.target_resolution {
        Some((tw, th)) => (tw.max(16), th.max(16)),
        None => (sim_w * upscale_factor, sim_h * upscale_factor),
    };

    let scale_x = out_w as f32 / sim_w as f32;
    let scale_y = out_h as f32 / sim_h as f32;
    let avg_scale = (scale_x + scale_y) * 0.5;

    let seed = world.seeds.master;
    let paper_noise = Perlin::new(1).set_seed((seed ^ 0x9e3779b9) as u32);
    let fiber_noise = Perlin::new(2).set_seed((seed + 101) as u32);
    let micro_fractal = Perlin::new(4).set_seed((seed + 303) as u32);

    // Precompute exact Euclidean distance from coast for smooth circular waterlining
    let water_dist = compute_water_coast_distance(&world.heightmap, &world.water_depth, 64);

    // Base paper palette according to style preset
    let (paper_light, paper_mid, paper_dark) = match params.style {
        PaperStyle::AgedParchment => (
            Color3::from_u8(246, 238, 218),
            Color3::from_u8(232, 218, 188),
            Color3::from_u8(208, 188, 150),
        ),
        PaperStyle::Atlas17thCentury => (
            Color3::from_u8(250, 245, 230),
            Color3::from_u8(238, 226, 198),
            Color3::from_u8(214, 198, 162),
        ),
        PaperStyle::CopperplateEngraving => (
            Color3::from_u8(246, 242, 232),
            Color3::from_u8(228, 220, 206),
            Color3::from_u8(200, 192, 174),
        ),
        PaperStyle::AntiquarianPatina => (
            Color3::from_u8(240, 222, 182),
            Color3::from_u8(218, 194, 146),
            Color3::from_u8(190, 160, 110),
        ),
    };

    let ink_coast = Color3::from_u8(INK_PRIMARY[0], INK_PRIMARY[1], INK_PRIMARY[2]);

    // Primary Northwest Sunlight Direction (315 deg azimuth, 45 deg altitude)
    let sun_dir = (-0.7071f32, -0.7071f32, 1.0f32);
    let sun_len = (sun_dir.0 * sun_dir.0 + sun_dir.1 * sun_dir.1 + sun_dir.2 * sun_dir.2).sqrt();
    let lx = sun_dir.0 / sun_len;
    let ly = sun_dir.1 / sun_len;
    let lz = sun_dir.2 / sun_len;

    // Secondary Southwest Fill Light (225 deg azimuth, 35 deg altitude)
    let fill_dir = (-0.573f32, 0.573f32, 0.70f32);
    let fill_len = (fill_dir.0 * fill_dir.0 + fill_dir.1 * fill_dir.1 + fill_dir.2 * fill_dir.2).sqrt();
    let flx = fill_dir.0 / fill_len;
    let fly = fill_dir.1 / fill_len;
    let flz = fill_dir.2 / fill_len;

    let delta = 0.5f32; // In simulation coordinates

    // Parallel per-pixel rendering across output resolution rows
    let mut raw_pixels: Vec<u8> = vec![0; out_w * out_h * 3];

    raw_pixels
        .par_chunks_exact_mut(out_w * 3)
        .enumerate()
        .for_each(|(y_out, row_bytes)| {
            let ny = y_out as f64 / out_h as f64;
            let sim_v = ((y_out as f32 + 0.5) / out_h as f32) * (sim_h as f32);

            for x_out in 0..out_w {
                let nx = x_out as f64 / out_w as f64;
                let sim_u = ((x_out as f32 + 0.5) / out_w as f32) * (sim_w as f32);

                // Continuous Catmull-Rom sampling of elevation, moisture, and water depth
                let refined_h = sample_elevation_with_microdetail(&world.heightmap, &micro_fractal, sim_u, sim_v, nx, ny, avg_scale);
                let base_moist = sample_tilemap_catmull_rom(&world.moisture, sim_u, sim_v);
                let base_wd = sample_tilemap_catmull_rom(&world.water_depth, sim_u, sim_v);

                // Subpixel finite-difference slope and surface normal calculation
                let h_r = sample_elevation_with_microdetail(&world.heightmap, &micro_fractal, sim_u + delta, sim_v, nx, ny, avg_scale);
                let h_l = sample_elevation_with_microdetail(&world.heightmap, &micro_fractal, sim_u - delta, sim_v, nx, ny, avg_scale);
                let h_d = sample_elevation_with_microdetail(&world.heightmap, &micro_fractal, sim_u, sim_v + delta, nx, ny, avg_scale);
                let h_u = sample_elevation_with_microdetail(&world.heightmap, &micro_fractal, sim_u, sim_v - delta, nx, ny, avg_scale);

                let dzdx = (h_r - h_l) / (2.0 * delta);
                let dzdy = (h_d - h_u) / (2.0 * delta);
                let slope = (dzdx * dzdx + dzdy * dzdy).sqrt();

                // Standard DEM hillshade Z-factor scaling (calibrated for meters per simulation tile)
                let z_scale = 0.0022f32;
                let nx_surf = -dzdx * z_scale;
                let ny_surf = -dzdy * z_scale;
                let nz_surf = 1.0f32;
                let n_len = (nx_surf * nx_surf + ny_surf * ny_surf + nz_surf * nz_surf).sqrt();
                let norm_x = nx_surf / n_len;
                let norm_y = ny_surf / n_len;
                let norm_z = nz_surf / n_len;

                // Multi-directional Eduard Imhof Swiss Shaded Relief Lighting
                let dot_sun = (norm_x * lx + norm_y * ly + norm_z * lz).clamp(-1.0, 1.0);
                let dot_fill = (norm_x * flx + norm_y * fly + norm_z * flz).clamp(-1.0, 1.0);

                // Difference from flat horizontal plane illumination (lz and flz)
                let sun_diff = dot_sun - lz;
                let fill_diff = (dot_fill - flz).max(0.0) * 0.25;

                // Aerial perspective: flat ground stays neutral (1.0), high peaks have dramatic contrast
                let elevation_contrast = 0.55 + ((refined_h - 100.0) / 1400.0).clamp(0.0, 1.0) * 0.65;
                let shaded_val = (1.0 + (sun_diff * 0.85 + fill_diff) * elevation_contrast).clamp(0.55, 1.35);

                // Lake detection: only substantial, true inland lakes
                let nearest_x = (sim_u.round() as usize).rem_euclid(sim_w);
                let nearest_y = (sim_v.round() as usize).clamp(0, sim_h - 1);
                let wb_id = *world.water_body_map.get(nearest_x, nearest_y);
                let is_significant_lake = if let Some(wb) = world.water_bodies.iter().find(|b| b.id == wb_id) {
                    wb.body_type == crate::water_bodies::WaterBodyType::Lake && wb.tile_count >= 12
                } else {
                    false
                };
                let is_lake = refined_h >= 0.0 && is_significant_lake && base_wd > 2.0;
                let is_land = refined_h >= 0.0 && !is_lake;

                // 1. Procedural Parchment Paper Base Texture
                let p_macro = paper_noise.get([nx * 10.0, ny * 10.0, 1.4]);
                let p_fibers = fiber_noise.get([nx * (60.0 * (avg_scale as f64).sqrt()), ny * (60.0 * (avg_scale as f64).sqrt()), 3.2]);
                let paper_val = (p_macro * 0.65 + p_fibers * 0.35) as f32 * params.paper_roughness;
                let mut current_color = if paper_val >= 0.0 {
                    paper_mid.lerp(paper_light, paper_val)
                } else {
                    paper_mid.lerp(paper_dark, -paper_val)
                };

                // Gentle edge vignette
                if params.vignette_strength > 0.05 {
                    let dist_x = nx.min(1.0 - nx) * 2.0;
                    let dist_y = ny.min(1.0 - ny) * 2.0;
                    let edge_dist = (dist_x.min(dist_y) as f32).clamp(0.0, 1.0);
                    let vignette = (1.0 - edge_dist).powf(1.8) * params.vignette_strength * 0.40;
                    current_color = current_color.lerp(paper_dark.multiply(Color3::new(0.70, 0.58, 0.45)), vignette);
                }

                if is_land {
                    // 2. Physical Topographic Hypsometric Tinting (No Horizontal Stripes)
                    let terrain_tint = compute_physical_terrain_color(refined_h, base_moist, slope, ny);

                    let lit_terrain = Color3::new(
                        (terrain_tint.0 * shaded_val).clamp(0.0, 1.15),
                        (terrain_tint.1 * shaded_val).clamp(0.0, 1.15),
                        (terrain_tint.2 * shaded_val).clamp(0.0, 1.15),
                    );

                    // Blend terrain with antique paper tone
                    let blended_wash = current_color.multiply(lit_terrain).lerp(lit_terrain, 0.45);
                    current_color = current_color.lerp(blended_wash, params.watercolor_opacity);

                    // Crisp knife-edge mountain crest highlights on highest summits
                    if refined_h > 1500.0 && slope > 160.0 && dot_sun < 0.25 {
                        let ridge_shadow = Color3::from_u8(72, 64, 58);
                        current_color = current_color.lerp(ridge_shadow, 0.25);
                    }
                } else if is_lake {
                    // Tranquil deep inland lake wash
                    let lake_water = Color3::from_u8(162, 194, 206);
                    current_color = current_color.lerp(lake_water, 0.75);
                } else {
                    // 3. Ocean Bathymetry & Coastal Shelf Gradient
                    let d_cells = sample_tilemap_catmull_rom(&water_dist, sim_u, sim_v);
                    let d_px = d_cells * avg_scale;

                    let shelf_shallow = Color3::from_u8(210, 226, 222); // Shallow luminous coastal shelf
                    let shelf_deep = Color3::from_u8(196, 214, 218);    // Continental shelf
                    let ocean_deep = Color3::from_u8(188, 206, 214);    // Oceanic basin

                    let ocean_tint = if d_px < 10.0 {
                        let t = d_px / 10.0;
                        shelf_shallow.lerp(shelf_deep, t)
                    } else if d_px < 35.0 {
                        let t = (d_px - 10.0) / 25.0;
                        shelf_deep.lerp(ocean_deep, t)
                    } else {
                        ocean_deep
                    };

                    current_color = current_color.multiply(ocean_tint).lerp(ocean_tint, 0.35);
                }

                // 4. Coastline Crisp Ink Edge (Zero-contour Proximity)
                let dh_px = (dzdx.abs() / scale_x + dzdy.abs() / scale_y) * 0.75;
                if dh_px > 0.001 && !is_lake {
                    let is_near_coast = if refined_h >= 0.0 {
                        refined_h < dh_px
                    } else {
                        -refined_h < dh_px && sample_tilemap_catmull_rom(&water_dist, sim_u, sim_v) <= 1.0
                    };

                    if is_near_coast {
                        let t = (refined_h.abs() / dh_px).clamp(0.0, 1.0);
                        let coast_strength = (1.0 - t).powf(1.4) * 0.80;
                        current_color = current_color.lerp(ink_coast, coast_strength);
                    }
                }

                // Write RGB bytes
                let rgb = current_color.to_rgb();
                let px_idx = x_out * 3;
                row_bytes[px_idx] = rgb[0];
                row_bytes[px_idx + 1] = rgb[1];
                row_bytes[px_idx + 2] = rgb[2];
            }
        });

    let mut img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_raw(out_w as u32, out_h as u32, raw_pixels)
        .expect("Buffer size matches image dimensions");

    // 5. Draw Hand-Drawn Iron-Gall Ink Rivers using Vector Geometry
    let (ink_capillary, ink_creek, ink_bank, water_wash) = match params.style {
        PaperStyle::CopperplateEngraving => (
            [48, 44, 40, 160],
            [36, 32, 28, 215],
            [24, 20, 16, 240],
            [85, 88, 92, 170],
        ),
        PaperStyle::AntiquarianPatina => (
            [55, 48, 40, 160],
            [40, 32, 24, 220],
            [28, 22, 16, 245],
            [95, 128, 142, 200],
        ),
        PaperStyle::Atlas17thCentury => (
            [45, 58, 74, 165],
            [32, 44, 62, 225],
            [22, 34, 52, 245],
            [110, 155, 185, 215],
        ),
        PaperStyle::AgedParchment => (
            [42, 55, 72, 165],
            [30, 42, 58, 225],
            [20, 32, 48, 250],
            [105, 148, 175, 210],
        ),
    };

    if let Some(river_net) = &world.river_network {
        for segment in &river_net.segments {
            let x_start = segment.p0.world_x * scale_x;
            let y_start = segment.p0.world_y * scale_y;
            let x_end = segment.p3.world_x * scale_x;
            let y_end = segment.p3.world_y * scale_y;

            if (x_start - x_end).abs() > out_w as f32 * 0.4 {
                continue;
            }

            let seg_len = ((x_end - x_start).powi(2) + (y_end - y_start).powi(2)).sqrt();
            let samples = ((seg_len * 1.5) as usize).clamp(8, 64);

            let mut prev_pt = segment.evaluate(0.0);
            for i in 1..=samples {
                let t = i as f32 / samples as f32;
                let cur_pt = segment.evaluate(t);

                let x0 = prev_pt.world_x * scale_x;
                let y0 = prev_pt.world_y * scale_y;
                let x1 = cur_pt.world_x * scale_x;
                let y1 = cur_pt.world_y * scale_y;

                if (x0 - x1).abs() < out_w as f32 * 0.4 {
                    let dx = x1 - x0;
                    let dy = y1 - y0;
                    let step_dist = (dx * dx + dy * dy).sqrt();

                    if step_dist >= 0.2 {
                        let nx = -dy / step_dist;
                        let ny = dx / step_dist;

                        match segment.stream_order {
                            1 => {
                                // Order 1: Delicate capillary hairline
                                draw_line_alpha(&mut img, x0, y0, x1, y1, ink_capillary);
                            }
                            2 => {
                                // Order 2: Crisp creek line
                                draw_line_alpha(&mut img, x0, y0, x1, y1, ink_creek);
                                if avg_scale >= 3.0 {
                                    draw_line_alpha(
                                        &mut img,
                                        x0 + nx * 0.5,
                                        y0 + ny * 0.5,
                                        x1 + nx * 0.5,
                                        y1 + ny * 0.5,
                                        [ink_creek[0], ink_creek[1], ink_creek[2], 110],
                                    );
                                }
                            }
                            3 => {
                                // Order 3: Dual riverbanks + watercolor core
                                let half_w = (avg_scale * 0.30).clamp(0.8, 2.0);
                                draw_line_alpha(&mut img, x0, y0, x1, y1, water_wash);
                                draw_line_alpha(
                                    &mut img,
                                    x0 + nx * half_w,
                                    y0 + ny * half_w,
                                    x1 + nx * half_w,
                                    y1 + ny * half_w,
                                    ink_bank,
                                );
                                draw_line_alpha(
                                    &mut img,
                                    x0 - nx * half_w,
                                    y0 - ny * half_w,
                                    x1 - nx * half_w,
                                    y1 - ny * half_w,
                                    ink_bank,
                                );
                            }
                            _ => {
                                // Order 4+: Broad continental trunk river
                                let river_w = (cur_pt.width * avg_scale * 0.45).clamp(2.2, 8.5);
                                let half_w = river_w * 0.5;

                                let fill_steps = (half_w * 1.5).round().max(1.0) as usize;
                                for step in 0..=fill_steps {
                                    let frac = (step as f32 / fill_steps as f32) * 2.0 - 1.0;
                                    let off = frac * (half_w - 0.4);
                                    draw_line_alpha(
                                        &mut img,
                                        x0 + nx * off,
                                        y0 + ny * off,
                                        x1 + nx * off,
                                        y1 + ny * off,
                                        water_wash,
                                    );
                                }

                                draw_line_alpha(
                                    &mut img,
                                    x0 + nx * half_w,
                                    y0 + ny * half_w,
                                    x1 + nx * half_w,
                                    y1 + ny * half_w,
                                    ink_bank,
                                );
                                draw_line_alpha(
                                    &mut img,
                                    x0 - nx * half_w,
                                    y0 - ny * half_w,
                                    x1 - nx * half_w,
                                    y1 - ny * half_w,
                                    ink_bank,
                                );
                            }
                        }
                    }
                }

                prev_pt = cur_pt;
            }
        }
    } else if let Some(flow_acc) = &world.flow_accumulation {
        for y in 0..sim_h {
            for x in 0..sim_w {
                let fa = *flow_acc.get(x, y);
                let h = *world.heightmap.get(x, y);
                if h >= 0.0 && fa > 50.0 {
                    let mut lowest_h = h;
                    let mut target_nx = x;
                    let mut target_ny = y;
                    for (nx, ny) in world.heightmap.neighbors_8(x, y) {
                        let nh = *world.heightmap.get(nx, ny);
                        if nh < lowest_h {
                            lowest_h = nh;
                            target_nx = nx;
                            target_ny = ny;
                        }
                    }
                    if target_nx != x || target_ny != y {
                        let x0 = x as f32 * scale_x;
                        let y0 = y as f32 * scale_y;
                        let x1 = target_nx as f32 * scale_x;
                        let y1 = target_ny as f32 * scale_y;
                        if (x0 - x1).abs() < (out_w as f32 * 0.4) {
                            draw_line_alpha(&mut img, x0, y0, x1, y1, ink_creek);
                        }
                    }
                }
            }
        }
    }

    // 6. Optional Vector Embellishments (Disabled by default so user can focus on pure terrain)
    let scaled_border_w = if params.show_vintage_border {
        ((params.border_width as f32) * avg_scale).round() as usize
    } else {
        0
    };

    let ocean_centers_sim = find_ocean_centers(&world.heightmap, 3);
    let ocean_centers_target: Vec<(usize, usize)> = ocean_centers_sim
        .iter()
        .map(|&(cx, cy)| {
            (
                ((cx as f32 * scale_x).round() as usize).min(out_w - 1),
                ((cy as f32 * scale_y).round() as usize).min(out_h - 1),
            )
        })
        .collect();

    if params.show_rhumb_lines {
        render_rhumb_lines(&mut img, &ocean_centers_target, scaled_border_w);
    }

    if params.show_graticule {
        render_graticule(&mut img, scaled_border_w);
    }

    if params.show_compass_rose {
        if let Some(&center) = ocean_centers_target.first() {
            let radius = ((out_w.min(out_h) as f32) * 0.14).clamp(12.0, 240.0);
            render_compass_rose(&mut img, center, radius);
        }
    }

    if params.show_vintage_border {
        render_vintage_border(&mut img, scaled_border_w);
    }

    img
}
