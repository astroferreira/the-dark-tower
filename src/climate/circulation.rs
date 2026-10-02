//! Atmospheric circulation and planetary wind field simulation
//!
//! Models 3-cell circulation (Hadley, Ferrel, Polar), shifting pressure belts,
//! land-sea thermal pressure contrast, and Coriolis-deflected surface winds.

use std::f32::consts::PI;
use rayon::prelude::*;
use crate::tilemap::Tilemap;
use super::ebm::row_latitude;

/// Planetary rotation angular velocity (Earth: 7.292e-5 rad/s)
pub const OMEGA: f32 = 7.292e-5;

/// Air density at sea level (kg/m^3)
pub const AIR_DENSITY: f32 = 1.225;

/// Planetary radius (meters)
pub const PLANET_RADIUS: f32 = 6.371e6;

/// Calculate Coriolis parameter f = 2 * Omega * sin(phi)
pub fn coriolis_parameter(lat_rad: f32) -> f32 {
    2.0 * OMEGA * lat_rad.sin()
}

/// Calculate base zonal sea-level pressure (hPa) for a given latitude and ITCZ latitude
pub fn base_zonal_pressure(lat_rad: f32, itcz_lat_rad: f32) -> f32 {
    let lat_deg = lat_rad.to_degrees();
    let itcz_deg = itcz_lat_rad.to_degrees();

    // Relative latitude from the thermal equator (ITCZ)
    let rel_lat = (lat_deg - itcz_deg).clamp(-90.0, 90.0);
    let abs_rel = rel_lat.abs();

    // 3-Cell Circulation Pressure Belts:
    // 1. ITCZ Low (~0-5° relative): 1006 hPa
    // 2. Subtropical High (~25-35° relative): 1022 hPa (Horse latitudes, dry descending air)
    // 3. Subpolar Low (~55-65° relative): 998 hPa (Polar front, cyclonic storm tracks)
    // 4. Polar High (~80-90°): 1018 hPa (Cold polar anticyclone)
    if abs_rel < 30.0 {
        // Between ITCZ Low (1006 hPa) and Subtropical High (1022 hPa)
        let t = abs_rel / 30.0;
        let smooth_t = t * t * (3.0 - 2.0 * t);
        1006.0 + 16.0 * smooth_t
    } else if abs_rel < 60.0 {
        // Between Subtropical High (1022 hPa) and Subpolar Low (998 hPa)
        let t = (abs_rel - 30.0) / 30.0;
        let smooth_t = t * t * (3.0 - 2.0 * t);
        1022.0 - 24.0 * smooth_t
    } else {
        // Between Subpolar Low (998 hPa) and Polar High (1018 hPa)
        let t = ((abs_rel - 60.0) / 30.0).clamp(0.0, 1.0);
        let smooth_t = t * t * (3.0 - 2.0 * t);
        998.0 + 20.0 * smooth_t
    }
}

/// Calculate atmospheric pressure field (hPa) combining zonal belts and land-sea thermal contrast
pub fn calculate_pressure_field(
    heightmap: &Tilemap<f32>,
    t_sealevel: &Tilemap<f32>,
    itcz_lat_rad: f32,
) -> Tilemap<f32> {
    let width = heightmap.width;
    let height = heightmap.height;

    // Step 1: Compute zonal average sea-level temperature
    let mut zonal_t = vec![0.0f32; height];
    for y in 0..height {
        let mut sum = 0.0f32;
        for x in 0..width {
            sum += *t_sealevel.get(x, y);
        }
        zonal_t[y] = sum / width as f32;
    }

    // Step 2: Compute combined base pressure and thermal perturbation
    let mut pressure = Tilemap::new_with(width, height, 1013.25f32);
    for y in 0..height {
        let lat = row_latitude(y, height);
        let p_zonal = base_zonal_pressure(lat, itcz_lat_rad);
        let z_temp = zonal_t[y];

        for x in 0..width {
            let elev = *heightmap.get(x, y);
            let sl_temp = *t_sealevel.get(x, y);
            let is_land = elev > 0.0;

            // Thermal pressure perturbation:
            // Warmer surface air expands -> thermal low (e.g. summer continental monsoon)
            // Cooler surface air contracts -> thermal high (e.g. winter Siberian high)
            let temp_anomaly = sl_temp - z_temp;
            let thermal_strength = if is_land { 0.7 } else { 0.25 };
            let p_thermal = -thermal_strength * temp_anomaly;

            pressure.set(x, y, (p_zonal + p_thermal).clamp(960.0, 1060.0));
        }
    }

    // Gentle 3x3 gaussian smoothing for synoptic-scale pressure structures
    smooth_pressure_map(&pressure)
}

/// Smooth pressure field using 3x3 box/gaussian kernel with horizontal wrap
fn smooth_pressure_map(pressure: &Tilemap<f32>) -> Tilemap<f32> {
    let width = pressure.width;
    let height = pressure.height;
    let mut smoothed = Tilemap::new_with(width, height, 1013.25f32);

    for y in 0..height {
        for x in 0..width {
            let mut sum = 0.0f32;
            let mut weight_sum = 0.0f32;

            for dy in -1i32..=1 {
                let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;
                for dx in -1i32..=1 {
                    let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
                    let weight = match (dx.abs(), dy.abs()) {
                        (0, 0) => 4.0,
                        (1, 0) | (0, 1) => 2.0,
                        _ => 1.0,
                    };
                    sum += *pressure.get(nx, ny) * weight;
                    weight_sum += weight;
                }
            }

            smoothed.set(x, y, sum / weight_sum);
        }
    }

    smoothed
}

/// Calculate surface wind vectors (u, v) in m/s across the globe.
///
/// u is zonal wind (positive = eastward / westerlies)
/// v is meridional wind (positive = northward)
pub fn calculate_surface_winds(
    pressure: &Tilemap<f32>,
    heightmap: &Tilemap<f32>,
) -> Tilemap<(f32, f32)> {
    let width = pressure.width;
    let height = pressure.height;
    let mut winds = Tilemap::new_with(width, height, (0.0f32, 0.0f32));

    let delta_lambda = 2.0 * PI / width as f32;
    let delta_phi = PI / height as f32;

    let rows: Vec<Vec<(f32, f32)>> = (0..height)
        .into_par_iter()
        .map(|y| {
            let lat = row_latitude(y, height);
            let cos_lat = lat.cos().max(0.08); // avoid division by zero near poles
            let f = coriolis_parameter(lat);

            let dy_dist = PLANET_RADIUS * delta_phi;
            let dx_dist = PLANET_RADIUS * cos_lat * delta_lambda;

            (0..width)
                .map(|x| {
                    let elev = *heightmap.get(x, y);
                    let is_land = elev > 0.0;

                    // Surface friction coefficient (kappa):
                    // Lower over ocean (~1.5e-4 s^-1), higher over rough terrain (~3.5e-4 s^-1)
                    let kappa = if is_land { 3.5e-4 } else { 1.5e-4 };

                    // Central differences for horizontal pressure gradients (convert hPa to Pa: 1 hPa = 100 Pa)
                    let west_x = if x == 0 { width - 1 } else { x - 1 };
                    let east_x = if x == width - 1 { 0 } else { x + 1 };
                    let dp_dx = (*pressure.get(east_x, y) - *pressure.get(west_x, y)) * 100.0 / (2.0 * dx_dist);

                    let north_y = y.saturating_sub(1);
                    let south_y = (y + 1).min(height - 1);
                    let dp_dy = (*pressure.get(x, north_y) - *pressure.get(x, south_y)) * 100.0 / (2.0 * dy_dist);

                    // Solve the boundary layer momentum equations:
                    //  f * u + kappa * v = - (1 / rho) * dp/dy
                    // -f * v + kappa * u = - (1 / rho) * dp/dx
                    //
                    // In matrix form: [ kappa   f ] [ u ] = [ -1/rho * dp/dx ]
                    //                 [ -f  kappa ] [ v ] = [ -1/rho * dp/dy ]
                    //
                    // Determinant: D = kappa^2 + f^2
                    // u = (1 / D) * [ -kappa * (1/rho * dp/dx) - f * (1/rho * dp/dy) ]
                    // v = (1 / D) * [  f * (1/rho * dp/dx) - kappa * (1/rho * dp/dy) ]

                    let inv_rho = 1.0 / AIR_DENSITY;
                    let fx = -inv_rho * dp_dx;
                    let fy = -inv_rho * dp_dy;

                    let det = kappa * kappa + f * f;
                    let mut u = (kappa * fx - f * fy) / det;
                    let mut v = (f * fx + kappa * fy) / det;

                    // Add zonal background flow for robust 3-cell structure
                    let lat_deg = lat.to_degrees().abs();
                    let zonal_bias = if lat_deg < 30.0 {
                        // Trade winds: westward (easterlies)
                        -6.0 * (1.0 - (lat_deg - 15.0).abs() / 15.0).max(0.0)
                    } else if lat_deg < 60.0 {
                        // Westerlies: eastward
                        10.0 * (1.0 - (lat_deg - 45.0).abs() / 15.0).max(0.0)
                    } else {
                        // Polar easterlies: westward
                        -4.0 * (1.0 - (lat_deg - 75.0).abs() / 15.0).max(0.0)
                    };

                    u += zonal_bias;

                    // Clamp to realistic physical surface wind speeds (0.5 to 30 m/s)
                    let speed = (u * u + v * v).sqrt();
                    let max_speed = if is_land { 22.0 } else { 32.0 };
                    if speed > max_speed {
                        let scale = max_speed / speed;
                        u *= scale;
                        v *= scale;
                    }

                    (u, v)
                })
                .collect()
        })
        .collect();

    for y in 0..height {
        for x in 0..width {
            winds.set(x, y, rows[y][x]);
        }
    }

    winds
}
