//! Wind-driven ocean currents and gyre circulation
//!
//! Simulates subtropical gyres, warm western boundary currents, cold eastern boundary currents,
//! and coastal upwelling zones that generate coastal fog deserts (Atacama, Namib).

use std::f32::consts::PI;
use crate::tilemap::Tilemap;
use super::ebm::row_latitude;

/// Calculate wind-driven surface ocean currents (u, v) in m/s
pub fn calculate_ocean_currents(
    heightmap: &Tilemap<f32>,
    surface_winds: &Tilemap<(f32, f32)>,
) -> Tilemap<(f32, f32)> {
    let width = heightmap.width;
    let height = heightmap.height;
    let mut currents = Tilemap::new_with(width, height, (0.0f32, 0.0f32));

    for y in 0..height {
        let lat = row_latitude(y, height);
        let is_north = lat >= 0.0;

        for x in 0..width {
            let elev = *heightmap.get(x, y);
            if elev > 0.0 {
                // Land has no ocean current
                continue;
            }

            let (wu, wv) = *surface_winds.get(x, y);

            // Ekman deflection: surface water is pushed ~45 degrees to the right in NH, left in SH
            let ekman_angle = if is_north { -PI * 0.25 } else { PI * 0.25 };
            let cos_e = ekman_angle.cos();
            let sin_e = ekman_angle.sin();

            let ekman_u = wu * cos_e - wv * sin_e;
            let ekman_v = wu * sin_e + wv * cos_e;

            // Current speed is roughly 2.5% of wind speed
            let current_speed_factor = 0.025;
            let mut cu = ekman_u * current_speed_factor;
            let mut cv = ekman_v * current_speed_factor;

            // Boundary current effects near coastlines:
            // Check east and west neighbors to detect ocean basin boundaries
            let west_x = if x == 0 { width - 1 } else { x - 1 };
            let east_x = if x == width - 1 { 0 } else { x + 1 };

            let west_is_land = *heightmap.get(west_x, y) > 0.0;
            let east_is_land = *heightmap.get(east_x, y) > 0.0;

            if west_is_land && !east_is_land {
                // Western boundary of ocean basin (eastern coast of continent):
                // Trade winds pile up water here -> strong poleward Western Boundary Current (Gulf Stream / Kuroshio)
                let poleward_dir = if is_north { 1.0 } else { -1.0 };
                cv += poleward_dir * 0.45;
                cu *= 0.3; // deflect parallel to coast
            } else if east_is_land && !west_is_land {
                // Eastern boundary of ocean basin (western coast of continent):
                // Equatorward Eastern Boundary Current (California, Humboldt, Benguela)
                let equatorward_dir = if is_north { -1.0 } else { 1.0 };
                cv += equatorward_dir * 0.35;
                cu *= 0.3;
            }

            currents.set(x, y, (cu, cv));
        }
    }

    currents
}

/// Detect coastal upwelling zones.
/// Upwelling occurs along western continental coasts where alongshore equatorward winds
/// produce offshore Ekman transport, pulling cold deep ocean water to the surface.
///
/// Returns upwelling cooling anomaly (Celsius, negative or zero).
pub fn calculate_coastal_upwelling(
    heightmap: &Tilemap<f32>,
    surface_winds: &Tilemap<(f32, f32)>,
) -> Tilemap<f32> {
    let width = heightmap.width;
    let height = heightmap.height;
    let mut upwelling = Tilemap::new_with(width, height, 0.0f32);

    for y in 0..height {
        let lat = row_latitude(y, height);
        let lat_deg = lat.to_degrees().abs();

        // Upwelling predominantly occurs in the subtropical/trade-wind belt (15° to 40° latitude)
        if lat_deg < 12.0 || lat_deg > 48.0 {
            continue;
        }

        let is_north = lat >= 0.0;

        for x in 0..width {
            let elev = *heightmap.get(x, y);
            if elev > 0.0 {
                continue; // Only in ocean
            }

            // Check if this ocean cell is directly adjacent to land to the east (i.e. west coast of continent)
            let east_x = if x == width - 1 { 0 } else { x + 1 };
            let east_is_land = *heightmap.get(east_x, y) > 0.0;

            if east_is_land {
                let (_wu, wv) = *surface_winds.get(x, y);

                // In NH: equatorward wind is southward (wv < 0)
                // In SH: equatorward wind is northward (wv > 0)
                let equatorward_wind = if is_north { -wv } else { wv };

                if equatorward_wind > 1.5 {
                    // Strong equatorward alongshore wind drives offshore Ekman suction
                    let intensity = (equatorward_wind / 8.0).clamp(0.2, 1.0);
                    // Upwelling can cool the sea surface by 3°C to 7°C
                    let cooling = -6.0 * intensity;
                    upwelling.set(x, y, cooling);

                    // Spread cooling slightly offshore (1-2 cells west)
                    let west_x = if x == 0 { width - 1 } else { x - 1 };
                    if *heightmap.get(west_x, y) <= 0.0 {
                        let prev = *upwelling.get(west_x, y);
                        upwelling.set(west_x, y, prev.min(cooling * 0.5));
                    }
                }
            }
        }
    }

    upwelling
}

/// Calculate Sea Surface Temperature (SST) in °C by applying ocean currents and upwelling
/// to base equilibrium sea-level temperatures.
pub fn calculate_sea_surface_temperatures(
    heightmap: &Tilemap<f32>,
    t_sealevel: &Tilemap<f32>,
    ocean_currents: &Tilemap<(f32, f32)>,
    upwelling_anomaly: &Tilemap<f32>,
) -> Tilemap<f32> {
    let width = heightmap.width;
    let height = heightmap.height;
    let mut sst = Tilemap::new_with(width, height, 0.0f32);

    for y in 0..height {
        let lat = row_latitude(y, height);
        let is_north = lat >= 0.0;

        for x in 0..width {
            let elev = *heightmap.get(x, y);
            let base_t = *t_sealevel.get(x, y);

            if elev > 0.0 {
                // Land tiles retain base temperature
                sst.set(x, y, base_t);
                continue;
            }

            let (_cu, cv) = *ocean_currents.get(x, y);
            let upwell = *upwelling_anomaly.get(x, y);

            // Meridional advection:
            // Poleward currents (cv > 0 in NH, cv < 0 in SH) bring warm water -> positive anomaly
            // Equatorward currents bring cold water -> negative anomaly
            let poleward_flow = if is_north { cv } else { -cv };
            let advection_anomaly = poleward_flow * 4.5; // up to ~3-4°C warming/cooling from boundary currents

            let final_sst = base_t + advection_anomaly + upwell;
            sst.set(x, y, final_sst);
        }
    }

    sst
}

/// Moderate land surface temperatures using maritime air advection.
///
/// Prevailing winds blow oceanic air masses inland:
/// - Westerlies carry warm maritime air onto west coasts in mid-latitudes (e.g. Europe).
/// - Alongshore equatorward winds carry cool upwelled air onto subtropical west coasts (e.g. Atacama, Namib).
/// - Trade winds carry warm tropical maritime air onto east coasts (e.g. Caribbean, East Asia).
/// - Maritime influence decays exponentially with distance from the coast.
pub fn apply_maritime_influence(
    heightmap: &Tilemap<f32>,
    t_surface: &Tilemap<f32>,
    sst: &Tilemap<f32>,
    surface_winds: &Tilemap<(f32, f32)>,
) -> Tilemap<f32> {
    let width = heightmap.width;
    let height = heightmap.height;
    let mut moderated = Tilemap::new_with(width, height, 0.0f32);

    for y in 0..height {
        let lat = row_latitude(y, height);
        let cos_lat = lat.cos().max(0.15);

        for x in 0..width {
            let elev = *heightmap.get(x, y);
            let t_local = *t_surface.get(x, y);

            if elev <= 0.0 {
                // Ocean retains sea surface temperature
                moderated.set(x, y, *sst.get(x, y));
                continue;
            }

            // For land cells, trace upwind along surface winds to find maritime influence
            let (wu, wv) = *surface_winds.get(x, y);
            let wind_speed = (wu * wu + wv * wv).sqrt();

            if wind_speed < 0.5 {
                moderated.set(x, y, t_local);
                continue;
            }

            // Direction toward upstream source:
            // wu > 0 blows East -> upstream is West (-x)
            // wv > 0 blows North -> upstream is South (+y in grid coordinates)
            let dir_x = -wu / (wind_speed * cos_lat);
            let dir_y = wv / wind_speed;

            // Trace upwind up to 12 steps to see if wind originated over ocean
            let mut found_ocean_sst: Option<f32> = None;
            let mut ocean_step_dist = 0.0f32;

            for step in 1..=12 {
                let s = step as f32;
                let sample_x = (x as f32 + dir_x * s).rem_euclid(width as f32);
                let sample_y = (y as f32 + dir_y * s).clamp(0.0, (height - 1) as f32);

                let sx = sample_x.round() as usize % width;
                let sy = sample_y.round() as usize;

                if *heightmap.get(sx, sy) <= 0.0 {
                    // Reached upwind ocean!
                    found_ocean_sst = Some(*sst.get(sx, sy));
                    ocean_step_dist = s;
                    break;
                }
            }

            if let Some(ocean_t) = found_ocean_sst {
                // Onshore maritime penetration:
                // Weight drops exponentially with distance from upwind coast (decay length ~ 9 tiles)
                let onshore_factor = (wind_speed / 8.0).clamp(0.25, 1.0);
                let penetration = (-ocean_step_dist / 8.5).exp() * onshore_factor;
                let blend_weight = (penetration * 0.65).clamp(0.0, 0.65);

                // Ocean air adjusts local land temperature toward maritime temperature
                // (adjusted for elevation lapse rate so mountains don't falsely warm up)
                let lapse_adj = (elev / 1000.0) * 6.5;
                let marine_target = ocean_t - lapse_adj;
                let adjusted_t = t_local * (1.0 - blend_weight) + marine_target * blend_weight;
                moderated.set(x, y, adjusted_t);
            } else {
                moderated.set(x, y, t_local);
            }
        }
    }

    moderated
}
