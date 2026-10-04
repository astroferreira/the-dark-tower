//! Energy Balance Model (EBM) for planetary surface temperature
//!
//! Simulates insolation, dynamic albedo with ice feedback, meridional heat
//! transport on a sphere, and land-sea thermal inertia contrast.

use std::f32::consts::PI;
use noise::{NoiseFn, Perlin, Seedable};
use rayon::prelude::*;
use crate::tilemap::Tilemap;
use super::compute_ocean_distance;

/// Standard planetary parameters for Earth-like energy balance
pub const SOLAR_CONSTANT: f32 = 1361.0;     // W/m^2
pub const OLR_A: f32 = 205.0;               // W/m^2 (Budyko-Sellers linear OLR offset)
pub const OLR_B: f32 = 2.2;                 // W/(m^2 * C) (Budyko-Sellers OLR slope)
pub const ATM_DIFFUSION_D: f32 = 0.68;      // W/(m^2 * C) (atmospheric heat diffusion coefficient)
pub const ELEVATION_LAPSE_RATE: f32 = 6.5;  // C per 1000m
/// Annual mean surface temperature (C, from the annual-mean solve) below which ice persists all
/// year (ice sheets, perennial sea ice): seasons keep its albedo. -8 C iced over the 60-70 deg
/// band (where Siberia's warm summers grow taiga); -20 C gives Earth-like polar summers.
pub const PERENNIAL_ICE_C: f32 = -20.0;

/// Calculate latitude in radians for a given grid row y
/// Returns latitude in [-PI/2, +PI/2] (North pole at y=0, South pole at y=H-1)
pub fn row_latitude(y: usize, height: usize) -> f32 {
    let normalized = (y as f32 + 0.5) / height as f32;
    PI * 0.5 - normalized * PI
}

/// Calculate daily-average insolation (W/m^2) at a given latitude for a specific solar declination.
/// Uses exact astronomical sunset hour angle on a spherical body.
pub fn calculate_daily_insolation(lat_rad: f32, declination_rad: f32, solar_const: f32) -> f32 {
    let sin_lat = lat_rad.sin();
    let cos_lat = lat_rad.cos();
    let sin_dec = declination_rad.sin();
    let cos_dec = declination_rad.cos();

    // Sunset hour angle calculation: cos(h0) = -tan(phi) * tan(delta)
    let tan_lat = lat_rad.tan();
    let tan_dec = declination_rad.tan();
    let arg = -tan_lat * tan_dec;

    if arg >= 1.0 {
        // Polar night (sun never rises)
        0.0
    } else if arg <= -1.0 {
        // Polar day / midnight sun (sun never sets, h0 = PI)
        solar_const * sin_lat * sin_dec
    } else {
        let h0 = arg.acos();
        (solar_const / PI) * (h0 * sin_lat * sin_dec + cos_lat * cos_dec * h0.sin())
    }
}

/// Calculate annual mean insolation (W/m^2) by integrating over the annual orbital cycle
pub fn calculate_annual_insolation(lat_rad: f32, axial_tilt_rad: f32, solar_const: f32) -> f32 {
    // 16-point numerical integration over the orbital year t in [0, 1]
    const STEPS: usize = 16;
    let mut sum = 0.0;
    for i in 0..STEPS {
        let t = (i as f32 + 0.5) / STEPS as f32;
        let dec = axial_tilt_rad * (2.0 * PI * t).sin();
        sum += calculate_daily_insolation(lat_rad, dec, solar_const);
    }
    sum / STEPS as f32
}

/// Dynamic surface albedo calculation based on surface type, latitude, and temperature
pub fn surface_albedo(elevation: f32, sea_level_temp: f32, lat_rad: f32) -> f32 {
    let is_ocean = elevation <= 0.0;

    // Base planetary albedo includes atmospheric Rayleigh scattering and clouds (~0.20-0.25)
    let base_albedo = if is_ocean {
        // Ocean + cloud albedo (~0.26), increasing at glancing solar angles near poles
        let cos_lat = lat_rad.cos();
        0.26 + 0.14 * (1.0 - cos_lat).powi(2)
    } else {
        // Land + cloud base albedo
        0.30
    };

    // Ice/snow albedo feedback
    // Snow/ice forms when local temperature is near or below freezing
    let local_temp = sea_level_temp - (elevation.max(0.0) / 1000.0) * ELEVATION_LAPSE_RATE;

    // Smooth transition from open surface to ice/snow between -5°C and +2°C
    let ice_fraction = ((2.0 - local_temp) / 7.0).clamp(0.0, 1.0);
    let ice_albedo = if is_ocean { 0.65 } else { 0.75 };

    (1.0 - ice_fraction) * base_albedo + ice_fraction * ice_albedo
}

/// Rossby planetary wave temperature perturbation in mid-latitudes (Celsius).
///
/// Undulates the polar jet stream and isotherms north and south with dominant
/// planetary wavenumbers 3, 4, and 5.
pub fn rossby_wave_perturbation(lon_rad: f32, lat_rad: f32, seed: u64) -> f32 {
    let abs_lat = lat_rad.abs();
    // Jet stream and Rossby waves peak in mid-latitudes (35° to 65°)
    let midlat_center = 48.0f32.to_radians();
    let midlat_width = 16.0f32.to_radians();
    let lat_diff = (abs_lat - midlat_center) / midlat_width;
    let lat_weight = (-lat_diff * lat_diff).exp(); // Bell curve peaking at 48°

    if lat_weight < 0.01 {
        return 0.0;
    }

    // Phase offsets derived deterministically from seed:
    let s = (seed & 0xFFFF) as f32;
    let phase1 = (s * 0.137).sin() * PI;
    let phase2 = (s * 0.283).cos() * PI;
    let phase3 = (s * 0.419).sin() * PI;

    // Hemispheric phase difference (NH and SH waves are distinct)
    let hemi_sign = if lat_rad >= 0.0 { 1.0 } else { -1.0 };
    let h_phase = hemi_sign * 0.85;

    // Superposition of wavenumbers 3, 4, and 5:
    let wave3 = (3.0 * lon_rad + phase1 + h_phase).sin();
    let wave4 = (4.0 * lon_rad + phase2 - h_phase).sin();
    let wave5 = (5.0 * lon_rad + phase3).sin();

    let combined = 0.55 * wave3 + 0.32 * wave4 + 0.13 * wave5;

    // Amplitude of ~8.0°C (gives realistic ~18 grid row undulations)
    lat_weight * combined * 8.0
}

/// Solve the spherical Energy Balance Model for a given heightmap and solar declination.
///
/// Returns sea-level temperature map and actual ground surface temperature map.
pub fn solve_energy_balance(
    heightmap: &Tilemap<f32>,
    declination_rad: f32,
    axial_tilt_rad: f32,
    solar_const: f32,
    is_annual_mean: bool,
    seed: u64,
    perennial: Option<&Tilemap<f32>>,
) -> (Tilemap<f32>, Tilemap<f32>) {
    let width = heightmap.width;
    let height = heightmap.height;

    let perlin = Perlin::new((seed ^ 0x9E3779B97F4A7C15) as u32);
    let ocean_dist = compute_ocean_distance(heightmap);

    // Compute continentality index (0.0 at coast/ocean, saturating to 1.0 inland)
    let mut continentality = Tilemap::new_with(width, height, 0.0f32);
    for y in 0..height {
        for x in 0..width {
            let elev = *heightmap.get(x, y);
            if elev > 0.0 {
                let dist = *ocean_dist.get(x, y);
                let cont = 1.0 - (-dist / 10.0).exp();
                continentality.set(x, y, cont.clamp(0.0, 1.0));
            }
        }
    }

    // Precompute warped effective latitude and insolation per cell
    let mut effective_lat_map = Tilemap::new_with(width, height, 0.0f32);
    let mut insolation_map = Tilemap::new_with(width, height, 0.0f32);
    let mut annual_insolation_map = Tilemap::new_with(width, height, 0.0f32);

    for y in 0..height {
        let lat_rad = row_latitude(y, height);
        let cos_phi = lat_rad.cos();
        let sin_phi = lat_rad.sin();

        for x in 0..width {
            let lon_rad = (x as f32 + 0.5) / width as f32 * 2.0 * PI;

            // 3D Cartesian coordinates on unit sphere for seamless noise
            let sx = cos_phi * lon_rad.cos();
            let sy = cos_phi * lon_rad.sin();
            let sz = sin_phi;

            // Multi-octave smooth synoptic coordinate warp (~12° latitude)
            let warp1 = perlin.get([sx as f64 * 1.8, sy as f64 * 1.8, sz as f64 * 1.8]) as f32;
            let warp2 = perlin.get([sx as f64 * 3.8 + 12.0, sy as f64 * 3.8 + 12.0, sz as f64 * 3.8 + 12.0]) as f32;
            let warp_val = warp1 * 0.15 + warp2 * 0.07;
            let eff_lat = (lat_rad + warp_val).clamp(-PI * 0.5 + 0.01, PI * 0.5 - 0.01);
            effective_lat_map.set(x, y, eff_lat);

            let s_annual = calculate_annual_insolation(eff_lat, axial_tilt_rad, solar_const);
            annual_insolation_map.set(x, y, s_annual);

            let s = if is_annual_mean {
                s_annual
            } else {
                let s_daily = calculate_daily_insolation(eff_lat, declination_rad, solar_const);
                let delta_s = s_daily - s_annual;
                let cont = *continentality.get(x, y);
                // Effective seasonal insolation amplitude: damped by ocean and continental heat capacity
                let inertia_alpha = if *heightmap.get(x, y) <= 0.0 {
                    0.20 // Ocean mixed layer thermal inertia
                } else {
                    0.20 + 0.40 * cont // Continental thermal inertia
                };
                s_annual + inertia_alpha * delta_s
            };
            insolation_map.set(x, y, s);
        }
    }

    // Area weights on sphere: w(y) = cos(lat)
    let row_weights: Vec<f32> = (0..height)
        .map(|y| row_latitude(y, height).cos().max(0.001))
        .collect();
    let total_weight: f32 = row_weights.iter().sum::<f32>() * width as f32;

    // Meridional heat transport (Budyko-Sellers, W/(m^2 C)). Budyko's Earth fit is ~3.8, but
    // with the continentality terms below 3.0 gives zonal land temperatures closest to Earth's
    // (2.2 left 50-70 degrees 7-8 C too cold and the map 30-40% tundra).
    const MERIDIONAL_D: f32 = 3.0;

    let mut t_sealevel = Tilemap::new_with(width, height, 15.0f32);

    // Iterative convergence for dynamic ice-albedo feedback (4 iterations)
    for _iter in 0..4 {
        // 1. Calculate Absorbed Solar Radiation (ASR) for all cells
        let mut asr_map = Tilemap::new_with(width, height, 0.0f32);
        let mut total_asr = 0.0f32;

        for y in 0..height {
            let w = row_weights[y];

            for x in 0..width {
                let elev = *heightmap.get(x, y);
                let curr_t = *t_sealevel.get(x, y);
                let eff_lat = *effective_lat_map.get(x, y);
                let mut albedo = surface_albedo(elev, curr_t, eff_lat);
                // Ice sheets persist through the summer: where the annual mean at the surface is
                // ice-sheet cold, a season keeps ice albedo whatever its own temperature (solving
                // each season from scratch let polar summers melt their ice and warm by ~25 C).
                if let Some(annual) = perennial {
                    let annual_surface = *annual.get(x, y) - (elev.max(0.0) / 1000.0) * ELEVATION_LAPSE_RATE;
                    if annual_surface < PERENNIAL_ICE_C {
                        albedo = albedo.max(if elev <= 0.0 { 0.65 } else { 0.75 });
                    }
                }
                let s = *insolation_map.get(x, y);
                let asr = s * (1.0 - albedo);
                asr_map.set(x, y, asr);
                total_asr += asr * w;
            }
        }

        let global_mean_asr = total_asr / total_weight;
        let global_mean_t = (global_mean_asr - OLR_A) / OLR_B;

        let mut next_t = Tilemap::new_with(width, height, 0.0f32);

        // 2. Compute local sea-level temperature based on Budyko balance + Rossby waves + Continentality
        for y in 0..height {
            let lat_rad = row_latitude(y, height);
            let lat_deg = lat_rad.to_degrees().abs();

            for x in 0..width {
                let elev = *heightmap.get(x, y);
                let asr = *asr_map.get(x, y);
                let lon_rad = (x as f32 + 0.5) / width as f32 * 2.0 * PI;

                // Budyko local equilibrium temperature
                let t_eq_base = (asr - OLR_A + MERIDIONAL_D * global_mean_t) / (OLR_B + MERIDIONAL_D);

                // Planetary Rossby wave undulation
                let t_rossby = rossby_wave_perturbation(lon_rad, lat_rad, seed);

                // Land thermal inertia and continentality contrast
                let t_cont = if elev > 0.0 {
                    let cont = *continentality.get(x, y);
                    let subtrop_heat = 4.0 * cont * (1.0 - (lat_deg / 40.0).powi(2)).max(0.0);
                    let subpolar_cool = if lat_deg > 38.0 {
                        -6.0 * cont * ((lat_deg - 38.0) / 52.0).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    subtrop_heat + subpolar_cool
                } else {
                    0.0
                };

                let final_t = (t_eq_base + t_rossby + t_cont).clamp(-60.0, 50.0);
                next_t.set(x, y, final_t);
            }
        }

        // Apply meridional spatial diffusion between rows to guarantee continuous temperature fields
        let mut diffused_t = Tilemap::new_with(width, height, 0.0f32);
        for y in 0..height {
            let y_prev = y.saturating_sub(1);
            let y_next = (y + 1).min(height - 1);
            for x in 0..width {
                let t_curr = *next_t.get(x, y);
                let t_p = *next_t.get(x, y_prev);
                let t_n = *next_t.get(x, y_next);
                let laplacian = t_p + t_n - 2.0 * t_curr;
                let diffused = t_curr + 0.25 * laplacian;
                diffused_t.set(x, y, diffused);
            }
        }

        // Under-relaxation: blend 50% old, 50% new to eliminate limit cycles and sharp cliffs
        for y in 0..height {
            for x in 0..width {
                let old_val = *t_sealevel.get(x, y);
                let new_val = *diffused_t.get(x, y);
                t_sealevel.set(x, y, 0.5 * old_val + 0.5 * new_val);
            }
        }
    }

    // Step 4: Apply environmental lapse rate for ground elevation
    let mut t_surface = Tilemap::new_with(width, height, 0.0f32);
    for y in 0..height {
        for x in 0..width {
            let sl_temp = *t_sealevel.get(x, y);
            let elev = *heightmap.get(x, y);
            let lapse_cooling = if elev > 0.0 {
                (elev / 1000.0) * ELEVATION_LAPSE_RATE
            } else {
                0.0
            };
            t_surface.set(x, y, sl_temp - lapse_cooling);
        }
    }

    (t_sealevel, t_surface)
}
