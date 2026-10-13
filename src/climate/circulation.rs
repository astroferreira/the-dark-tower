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

/// The planet's circulation cells: the pressure belts they leave at the surface, the zonal
/// winds between them, the storm track and its Rossby waves. Earth's three cells (Hadley to
/// 30 deg, Ferrel to 60, polar) come from its spin: the Hadley cell's edge and the width of the
/// eddy-driven cells (the Rhines scale) both shrink as 1 / sqrt(spin) (Held & Hou 1980; GCM
/// studies give exponents between 1/3 and 1), so a fast spinner has more, narrower bands
/// (Jupiter-like) and a slow one a single Hadley cell from equator to pole. A locked world has
/// no belts: air rises under the star and sinks on the night side.
#[derive(Clone, Debug)]
pub struct Circulation {
    /// Angular speed of the spin (rad/s).
    pub omega: f32,
    /// Sea-level pressure belts from the thermal equator poleward: (degrees from it, hPa),
    /// from 0 to 90.
    pub belts: Vec<(f32, f32)>,
    /// Background zonal surface wind by latitude band: (from deg, to deg, peak m/s at the
    /// band's middle; negative is easterly).
    pub zonal: Vec<(f32, f32, f32)>,
    /// The storm track (degrees, width) and its Rossby waves (C, wavenumbers).
    pub storm_lat_deg: f32,
    pub storm_width_deg: f32,
    pub rossby_amp: f32,
    pub waves: [f32; 3],
    /// Latitudes (deg) of coastal upwelling under the trade winds.
    pub upwelling: (f32, f32),
    pub hadley_deg: f32,
    /// One face to the star: pressure from the day-night contrast, a superrotating jet.
    pub locked: bool,
    /// Peak equatorward surface wind in the tropics' cell (m/s): on a slow spinner the
    /// overturning's low branch blows toward the equator with little Coriolis to turn it into
    /// trades (0 at Earth's spin, where the trades carry it).
    pub meridional: f32,
}

impl Circulation {
    /// Earth's three cells.
    pub fn earth() -> Self {
        Circulation {
            omega: OMEGA,
            belts: vec![(0.0, 1006.0), (30.0, 1022.0), (60.0, 998.0), (90.0, 1018.0)],
            zonal: vec![(0.0, 30.0, -6.0), (30.0, 60.0, 10.0), (60.0, 90.0, -4.0)],
            storm_lat_deg: 48.0,
            storm_width_deg: 16.0,
            rossby_amp: 8.0,
            waves: [3.0, 4.0, 5.0],
            upwelling: (12.0, 48.0),
            hadley_deg: 30.0,
            locked: false,
            meridional: 0.0,
        }
    }

    pub fn for_planet(f: &super::forcing::PlanetForcing) -> Self {
        if f.legacy { return Self::earth(); }
        let r = f.rotation.max(1e-3);
        let scale = 1.0 / r.sqrt();
        let zs = r.sqrt().clamp(0.3, 1.5);
        let omega = OMEGA * r;
        if f.locked.is_some() {
            return Circulation {
                omega,
                belts: vec![(0.0, 1008.0), (90.0, 1016.0)],
                zonal: vec![(0.0, 90.0, 0.0)],
                storm_lat_deg: 45.0,
                storm_width_deg: 16.0,
                rossby_amp: 0.0,
                waves: [1.0, 2.0, 3.0],
                upwelling: (90.0, 90.0),
                hadley_deg: 90.0,
                locked: true,
                meridional: 0.0,
            };
        }
        let hadley = (30.0 * scale).clamp(10.0, 90.0);
        if hadley >= 70.0 {
            // One cell from the equator to the pole: trades everywhere, no storm track.
            return Circulation {
                omega,
                belts: vec![(0.0, 1006.0), (90.0, 1020.0)],
                zonal: vec![(0.0, 90.0, -4.0 * zs)],
                storm_lat_deg: 60.0,
                storm_width_deg: 16.0,
                rossby_amp: 0.0,
                waves: [1.0, 2.0, 3.0],
                upwelling: (0.4 * hadley, 1.6 * hadley),
                hadley_deg: 90.0,
                locked: false,
                meridional: 6.0 * (1.0 - r.sqrt()).clamp(0.0, 1.0),
            };
        }
        let width0 = (30.0 * scale).clamp(6.0, 90.0);
        let k = ((90.0 - hadley) / (2.0 * width0)).round().max(1.0) as usize;
        let w = (90.0 - hadley) / (2 * k) as f32;
        let mut belts = vec![(0.0, 1006.0), (hadley, 1022.0)];
        let mut zonal = vec![(0.0, hadley, -6.0 * zs)];
        for j in 0..k {
            let low = if j == 0 { 998.0 } else { 1002.0 };
            let high = if j + 1 == k { 1018.0 } else { 1014.0 };
            let b = hadley + 2.0 * j as f32 * w;
            belts.push((b + w, low));
            belts.push((if j + 1 == k { 90.0 } else { b + 2.0 * w }, high));
            // High to low poleward: westerlies; low to high: easterlies.
            zonal.push((b, b + w, if j == 0 { 10.0 } else { 6.0 } * zs));
            zonal.push((b + w, if j + 1 == k { 90.0 } else { b + 2.0 * w }, -4.0 * zs));
        }
        let waves = [3.0f32, 4.0, 5.0].map(|n| (n * r.sqrt()).round().max(1.0));
        Circulation {
            omega,
            belts,
            zonal,
            storm_lat_deg: hadley + 0.6 * w,
            storm_width_deg: w * 16.0 / 30.0,
            rossby_amp: 8.0 * r.sqrt().min(1.3),
            waves,
            upwelling: (0.4 * hadley, (1.6 * hadley).min(80.0)),
            hadley_deg: hadley,
            locked: false,
            meridional: 6.0 * (1.0 - r.sqrt()).clamp(0.0, 1.0),
        }
    }

    /// Surface pressure belt (hPa) at a latitude for the thermal equator's latitude.
    pub fn zonal_pressure(&self, lat_rad: f32, itcz_lat_rad: f32) -> f32 {
        let lat_deg = lat_rad.to_degrees();
        let itcz_deg = itcz_lat_rad.to_degrees();
        // Relative latitude from the thermal equator (ITCZ)
        let rel_lat = (lat_deg - itcz_deg).clamp(-90.0, 90.0);
        let abs_rel = rel_lat.abs();
        let last = self.belts.len() - 2;
        for i in 0..=last {
            let (b0, p0) = self.belts[i];
            let (b1, p1) = self.belts[i + 1];
            if abs_rel < b1 || i == last {
                let mut t = (abs_rel - b0) / (b1 - b0);
                if i == last { t = t.clamp(0.0, 1.0); }
                let smooth_t = t * t * (3.0 - 2.0 * t);
                return p0 + (p1 - p0) * smooth_t;
            }
        }
        1013.25
    }

    /// Background zonal wind (m/s, eastward positive) at a latitude (degrees, absolute).
    fn zonal_wind(&self, lat_deg: f32) -> f32 {
        let last = self.zonal.len() - 1;
        for (i, &(lo, hi, amp)) in self.zonal.iter().enumerate() {
            if lat_deg < hi || i == last {
                let c = (lo + hi) / 2.0;
                let hw = (hi - lo) / 2.0;
                return amp * (1.0 - (lat_deg - c).abs() / hw).max(0.0);
            }
        }
        0.0
    }
}

/// Calculate base zonal sea-level pressure (hPa) for a given latitude and ITCZ latitude
/// (Earth's three cells).
pub fn base_zonal_pressure(lat_rad: f32, itcz_lat_rad: f32) -> f32 {
    Circulation::earth().zonal_pressure(lat_rad, itcz_lat_rad)
}

/// Calculate atmospheric pressure field (hPa) combining zonal belts and land-sea thermal contrast
pub fn calculate_pressure_field(
    heightmap: &Tilemap<f32>,
    t_sealevel: &Tilemap<f32>,
    itcz_lat_rad: f32,
    circ: &Circulation,
) -> Tilemap<f32> {
    let width = heightmap.width;
    let height = heightmap.height;

    if circ.locked {
        // Day-night contrast: warm air rises under the star (a broad low), cold air sinks over
        // the night side (a high), land a little more than sea.
        let n = (width * height) as f32;
        let mean_t = t_sealevel.iter().map(|(_, _, &t)| t).sum::<f32>() / n;
        let mut pressure = Tilemap::new_with(width, height, 1013.25f32);
        for y in 0..height {
            for x in 0..width {
                let anomaly = *t_sealevel.get(x, y) - mean_t;
                let k = if *heightmap.get(x, y) > 0.0 { 0.32 } else { 0.24 };
                pressure.set(x, y, (1012.0 - k * anomaly).clamp(985.0, 1035.0));
            }
        }
        return smooth_pressure_map(&smooth_pressure_map(&pressure));
    }

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
        let p_zonal = circ.zonal_pressure(lat, itcz_lat_rad);
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
    circ: &Circulation,
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
            let f = 2.0 * circ.omega * lat.sin();

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

                    // Add zonal background flow for robust 3-cell structure: trades, westerlies,
                    // polar easterlies (or the planet's own bands); a locked world's
                    // superrotating equatorial jet.
                    let lat_deg = lat.to_degrees().abs();
                    let zonal_bias = if circ.locked {
                        5.0 * (-(lat_deg / 25.0).powi(2)).exp()
                    } else {
                        circ.zonal_wind(lat_deg)
                    };

                    u += zonal_bias;
                    // A slow spinner's Hadley flow, toward the equator within the cell.
                    if circ.meridional > 0.0 && lat_deg < circ.hadley_deg {
                        let shape = (std::f32::consts::PI * lat_deg / circ.hadley_deg).sin();
                        v += -lat.signum() * circ.meridional * shape;
                    }

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
