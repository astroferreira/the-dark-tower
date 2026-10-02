//! Moisture advection, evaporation, and precipitation physics
//!
//! Replaces distance/noise heuristics with physical Clausius-Clapeyron saturation,
//! ocean evaporation, Semi-Lagrangian wind advection, orographic lift, and rain shadows.

use std::f32::consts::PI;
use crate::tilemap::Tilemap;
use super::ebm::row_latitude;
use super::circulation::PLANET_RADIUS;

/// Calculate saturation specific humidity q_sat (g/kg) via Clausius-Clapeyron
pub fn saturation_humidity(temp_c: f32, elevation_m: f32) -> f32 {
    // Magnus-Tetens formula for saturation vapor pressure over liquid/ice
    let temp_clamped = temp_c.clamp(-60.0, 55.0);
    let es = 6.112 * ((17.67 * temp_clamped) / (temp_clamped + 243.5)).exp(); // in hPa

    // Atmospheric pressure decreases with elevation (barometric formula)
    let p_local = 1013.25 * (-elevation_m.max(0.0) / 8400.0).exp();

    // Specific humidity: q_sat = 0.622 * es / (p - 0.378 * es) in kg/kg -> * 1000 for g/kg
    let q = (622.0 * es) / (p_local - 0.378 * es).max(10.0);
    q.clamp(0.05, 45.0)
}

/// Potential Evapotranspiration (PET) in mm/day equivalent proxy
pub fn potential_evapotranspiration(temp_c: f32) -> f32 {
    if temp_c <= 0.0 {
        0.05
    } else {
        // Warm air has higher evaporative power
        0.10 + 0.045 * temp_c + 0.001 * temp_c * temp_c
    }
}

/// Perform Semi-Lagrangian advection and moisture precipitation simulation.
///
/// Returns (precipitation in mm/year, moisture index in [0.0, 1.0]).
pub fn simulate_moisture_and_precipitation(
    heightmap: &Tilemap<f32>,
    surface_temp: &Tilemap<f32>,
    sst: &Tilemap<f32>,
    surface_winds: &Tilemap<(f32, f32)>,
    rainfall_multiplier: f32,
    rainfall_floor: f32,
) -> (Tilemap<f32>, Tilemap<f32>) {
    let width = heightmap.width;
    let height = heightmap.height;

    let delta_lambda = 2.0 * PI / width as f32;
    let delta_phi = PI / height as f32;

    // Time step for advection iterations
    let dt = 1800.0; // 30 minutes in seconds
    let total_steps = 24; // 12 hours total simulation time to reach steady moisture equilibrium

    // Initialize atmospheric specific humidity q (g/kg)
    let mut q_air = Tilemap::new_with(width, height, 0.0f32);
    for y in 0..height {
        for x in 0..width {
            let elev = *heightmap.get(x, y);
            let t = *surface_temp.get(x, y);
            let q_sat = saturation_humidity(t, elev);
            if elev <= 0.0 {
                // Ocean starts saturated
                q_air.set(x, y, q_sat * 0.95);
            } else {
                q_air.set(x, y, q_sat * 0.30);
            }
        }
    }

    // Accumulators for precipitation over the simulation
    let mut total_precip = Tilemap::new_with(width, height, 0.0f32);

    // Number of advection sweeps to transport moisture across continents
    let total_steps = 32;

    for _step in 0..total_steps {
        let mut next_q = Tilemap::new_with(width, height, 0.0f32);
        let mut next_src_elev = Tilemap::new_with(width, height, 0.0f32);

        // Sub-step 1: Semi-Lagrangian Advection
        // Each step advances moisture by roughly 0.8-1.2 cells along the wind vector
        const REF_SPEED: f32 = 7.5;

        for y in 0..height {
            let lat = row_latitude(y, height);
            let cos_lat = lat.cos().max(0.18);

            for x in 0..width {
                let (wu, wv) = *surface_winds.get(x, y);

                // Wind vector in grid-cell units:
                // wu > 0 blows East (to +x), so upstream source is West (-wu)
                // wv > 0 blows North (to -y), so upstream source is South (+wv)
                let delta_x_cells = -(wu / (REF_SPEED * cos_lat)).clamp(-3.0, 3.0);
                let delta_y_cells = (wv / REF_SPEED).clamp(-2.0, 2.0);

                let src_x = (x as f32 + delta_x_cells).rem_euclid(width as f32);
                let src_y = (y as f32 + delta_y_cells).clamp(0.0, (height - 1) as f32);

                // Bilinear interpolation of upwind moisture and upwind elevation
                let x0 = src_x.floor() as usize;
                let x1 = (x0 + 1) % width;
                let y0 = src_y.floor() as usize;
                let y1 = (y0 + 1).min(height - 1);

                let fx = src_x - x0 as f32;
                let fy = src_y - y0 as f32;

                let q00 = *q_air.get(x0, y0);
                let q10 = *q_air.get(x1, y0);
                let q01 = *q_air.get(x0, y1);
                let q11 = *q_air.get(x1, y1);

                let advected_q = (1.0 - fx) * (1.0 - fy) * q00
                    + fx * (1.0 - fy) * q10
                    + (1.0 - fx) * fy * q01
                    + fx * fy * q11;

                let h00 = *heightmap.get(x0, y0);
                let h10 = *heightmap.get(x1, y0);
                let h01 = *heightmap.get(x0, y1);
                let h11 = *heightmap.get(x1, y1);

                let advected_h = (1.0 - fx) * (1.0 - fy) * h00
                    + fx * (1.0 - fy) * h10
                    + (1.0 - fx) * fy * h01
                    + fx * fy * h11;

                next_q.set(x, y, advected_q);
                next_src_elev.set(x, y, advected_h);
            }
        }

        // Sub-step 2: Evaporation, Orographic Lift, and Precipitation
        for y in 0..height {
            for x in 0..width {
                let elev = *heightmap.get(x, y);
                let t = *surface_temp.get(x, y);
                let src_h = *next_src_elev.get(x, y);
                let mut q = *next_q.get(x, y);

                let is_ocean = elev <= 0.0;
                let mut step_precip = 0.0f32;

                if is_ocean {
                    // 2a. Evaporation over water:
                    let sea_t = *sst.get(x, y);
                    let q_sat_sea = saturation_humidity(sea_t, 0.0);

                    // Recharges air moisture toward sea saturation
                    let deficit = (q_sat_sea * 0.95 - q).max(0.0);
                    q += deficit * 0.40;

                    // Convergence precipitation over ocean (e.g. ITCZ)
                    if q > q_sat_sea * 0.88 {
                        let excess = q - q_sat_sea * 0.88;
                        let condensed = excess * 0.35;
                        q -= condensed;
                        step_precip += condensed * 1.5;
                    }
                } else {
                    // 2b. Orographic Lift & Rain Shadows:
                    // Elevation change along the wind trajectory: delta_h = elev - src_h
                    let delta_h = elev - src_h;
                    let q_sat_local = saturation_humidity(t, elev);

                    if delta_h > 20.0 {
                        // Air moving uphill: adiabatic expansion and cooling
                        let cooling = (delta_h / 1000.0) * 6.5;
                        let q_sat_lifted = saturation_humidity(t - cooling, elev);

                        if q > q_sat_lifted {
                            let excess = q - q_sat_lifted;
                            let condensed = excess * 0.85; // high orographic precipitation efficiency
                            q -= condensed;
                            step_precip += condensed * 3.5;
                        }
                    } else if delta_h < -20.0 {
                        // Air moving downhill: adiabatic warming (foehn/chinook wind)
                        // Relative humidity plummets -> zero rain, dry air creates rain shadow!
                        // No condensation occurs when descending
                    }

                    // 2c. Convective / Frontal Condensation
                    if q > q_sat_local * 0.82 {
                        let excess = q - q_sat_local * 0.82;
                        let condensed = excess * 0.50;
                        q -= condensed;
                        step_precip += condensed * 1.8;
                    }

                    // 2d. Continental Evapotranspiration Recycling:
                    // Only occurs if there is local soil moisture from previous precipitation
                    let prev_precip = *total_precip.get(x, y);
                    if prev_precip > 0.05 {
                        let pet = potential_evapotranspiration(t);
                        let moisture_avail = (prev_precip * 0.15).min(1.0);
                        let deficit = (q_sat_local * 0.70 - q).max(0.0);
                        let et_rate = (pet * 0.08 * moisture_avail).min(deficit * 0.10);
                        q += et_rate;
                    }
                }

                let current_total = *total_precip.get(x, y);
                total_precip.set(x, y, current_total + step_precip);
                next_q.set(x, y, q);
            }
        }

        q_air = next_q;
    }

    // Step 3: Compute annual precipitation (mm/year) and normalized moisture (0.0 to 1.0)
    let mut precip_map = Tilemap::new_with(width, height, 0.0f32);
    let mut moisture_map = Tilemap::new_with(width, height, 0.0f32);

    for y in 0..height {
        for x in 0..width {
            let elev = *heightmap.get(x, y);
            let t = *surface_temp.get(x, y);
            let is_ocean = elev <= 0.0;

            if is_ocean {
                precip_map.set(x, y, 1200.0);
                moisture_map.set(x, y, 1.0);
                continue;
            }

            // Convert accumulated simulation precipitation to annual mm/year
            // Scaled so average wet temperate regions receive ~800-1500 mm, deserts < 200 mm
            let raw_precip = *total_precip.get(x, y);
            let annual_mm = (raw_precip * 85.0 * rainfall_multiplier).clamp(20.0, 5000.0);
            precip_map.set(x, y, annual_mm);

            // Aridity Index calculation:
            // Moisture = P / (P + PET)
            // Hot deserts have high PET and low P -> Moisture < 0.15
            // Temperate forests have P ~ PET -> Moisture ~ 0.5 - 0.7
            // Tropical rainforests have P >> PET -> Moisture > 0.8
            let pet = potential_evapotranspiration(t) * 365.0; // annual PET in mm
            let aridity_moisture = annual_mm / (annual_mm + pet * 0.7);

            let final_moisture = aridity_moisture
                .clamp(rainfall_floor, 1.0);

            moisture_map.set(x, y, final_moisture);
        }
    }

    (precip_map, moisture_map)
}
