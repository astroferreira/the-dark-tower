//! Moisture advection, evaporation, and precipitation physics
//!
//! Replaces distance/noise heuristics with physical Clausius-Clapeyron saturation,
//! ocean evaporation, Semi-Lagrangian wind advection, orographic lift, and rain shadows.

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

/// Annual potential evapotranspiration (mm/yr) from mean temperature: a fit to Thornthwaite /
/// Hargreaves annual totals (about 300 mm at 0 C, 800 at 10 C, 1550 at 25 C), with a floor for
/// sublimation in the cold.
pub fn pet_mm(temp_c: f32) -> f32 {
    (300.0 + 50.0 * temp_c).clamp(60.0, 1900.0)
}

/// Moisture index in [0, 1] from annual precipitation and temperature: P / (P + PET), the UNEP
/// aridity index AI = P / PET mapped to AI / (1 + AI). Hyper-arid AI < 0.05 (index < 0.05),
/// arid < 0.2 (< 0.17), semi-arid < 0.5 (< 0.33), dry sub-humid < 0.65 (< 0.39), humid above;
/// rainforests have AI > 2 (index > 0.67).
pub fn moisture_index(precip_mm: f32, temp_c: f32) -> f32 {
    let p = precip_mm.max(0.0);
    p / (p + pet_mm(temp_c))
}

/// Annual runoff (mm/yr): precipitation minus actual evapotranspiration, which follows the
/// Budyko curve in Fu's form (w = 2.6). Wet, cool land sheds most of its rain; where
/// evaporative demand far exceeds rain almost nothing runs off.
pub fn runoff_mm(precip_mm: f32, temp_c: f32) -> f32 {
    const W: f32 = 2.6;
    let p = precip_mm.max(0.0);
    if p <= 0.0 { return 0.0; }
    let phi = pet_mm(temp_c) / p;
    let aet_ratio = 1.0 + phi - (1.0 + phi.powf(W)).powf(1.0 / W);
    (p * (1.0 - aet_ratio)).max(0.0)
}

/// Large-scale vertical motion from sea-level pressure: low pressure (ITCZ, subpolar storm
/// tracks, summer thermal lows) means rising air and rain, high pressure (subtropical and polar
/// highs) sinking air and drought. 1.0 at the ITCZ's 1006 hPa; capped just above it so the
/// deeper subpolar lows don't out-rain the vapour-rich tropics.
fn ascent(pressure_hpa: f32, cap: f32) -> f32 {
    ((1021.0 - pressure_hpa) / 15.0).clamp(0.06, cap)
}

/// Perform Semi-Lagrangian advection and moisture precipitation simulation.
///
/// Water vapour evaporates from the sea, is carried by the wind (about one cell per step), and
/// rains out at a rate set by relative humidity and large-scale ascent (from `pressure`), plus
/// orographic lift where the wind climbs. Over land about half the rain is evapotranspired back
/// into the air, which carries moisture deep into continents. Precipitation is averaged over the
/// steps after a spin-up and scaled to mm/yr.
///
/// Returns (precipitation in mm/year, moisture index in [0.0, 1.0]).
pub fn simulate_moisture_and_precipitation(
    heightmap: &Tilemap<f32>,
    surface_temp: &Tilemap<f32>,
    sst: &Tilemap<f32>,
    surface_winds: &Tilemap<(f32, f32)>,
    pressure: &Tilemap<f32>,
    rainfall_multiplier: f32,
    rainfall_floor: f32,
) -> (Tilemap<f32>, Tilemap<f32>) {
    /// Mean residence time of water vapour in the atmosphere (Earth: ~9 days); a fully humid,
    /// strongly rising column rains out faster by RAIN_BOOST.
    const RESIDENCE_S: f32 = 9.0 * 86_400.0;
    const RAIN_BOOST: f32 = 3.0;
    /// Vapour rides the steering-level winds (850-700 hPa), faster than the surface wind.
    const TRANSPORT: f32 = 3.5;
    /// Cap on large-scale ascent (see `ascent`).
    const ASCENT_CAP: f32 = 1.15;
    /// Orographic rain: share of the vapour rained out per metre of climb along the wind
    /// (1/CLIMB_M), at most CLIMB_MAX per step.
    const CLIMB_M: f32 = 3000.0;
    const CLIMB_MAX: f32 = 0.3;
    /// Marine air is recharged toward this relative humidity over the sea.
    const SEA_RH: f32 = 0.8;
    /// Precipitable water (mm) per g/kg of near-surface specific humidity (vapour scale height).
    const PW_MM_PER_GKG: f32 = 2.5;
    /// Share of land rain returned to the air by evapotranspiration, scaled by warmth (from 0.4
    /// in the cold to 1 above 20 C). Land recycling is what carries rain into the interiors.
    const RECYCLE: f32 = 0.92;
    const COLD_RECYCLE: f32 = 0.4;
    /// Steps: the air moves about a cell per step; rain is averaged after SPIN_UP.
    const STEPS: usize = 120;
    const SPIN_UP: usize = 40;
    const REF_SPEED: f32 = 7.5;

    let width = heightmap.width;
    let height = heightmap.height;
    let n = width * height;
    // One step = the time the reference wind takes to cross a cell.
    let step_s = 2.0 * std::f32::consts::PI * PLANET_RADIUS / width as f32 / REF_SPEED;
    let rain_rate = (RAIN_BOOST * step_s / RESIDENCE_S).min(0.9);
    let mm_per_unit = PW_MM_PER_GKG * 365.0 * 86_400.0 / step_s;
    let elev: Vec<f32> = heightmap.iter().map(|(_, _, &e)| e).collect();
    let temp: Vec<f32> = surface_temp.iter().map(|(_, _, &t)| t).collect();
    let q_sat: Vec<f32> = (0..n).map(|i| saturation_humidity(temp[i], elev[i])).collect();
    let q_sat_sea: Vec<f32> = sst.iter().map(|(_, _, &t)| saturation_humidity(t, 0.0)).collect();
    let lift: Vec<f32> = pressure.iter().map(|(_, _, &p)| ascent(p, ASCENT_CAP)).collect();
    // Orographic lift sees the broad terrain, not single-cell bumps (which would streak).
    let smooth = {
        let mut a = elev.iter().map(|&e| e.max(0.0)).collect::<Vec<f32>>();
        for _ in 0..2 {
            let b = a.clone();
            for y in 0..height {
                for x in 0..width {
                    let mut sum = 0.0;
                    for dy in -1i32..=1 {
                        let yy = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;
                        for dx in -1i32..=1 {
                            sum += b[yy * width + (x as i32 + dx).rem_euclid(width as i32) as usize];
                        }
                    }
                    a[y * width + x] = sum / 9.0;
                }
            }
        }
        a
    };
    let warmth: Vec<f32> = temp.iter().map(|&t| ((t + 5.0) / 25.0).clamp(COLD_RECYCLE, 1.0)).collect();

    // Upwind source of each cell (semi-Lagrangian, bilinear), fixed for the season.
    let mut src: Vec<[(usize, f32); 4]> = Vec::with_capacity(n);
    for y in 0..height {
        let lat = row_latitude(y, height);
        let cos_lat = lat.cos().max(0.18);
        for x in 0..width {
            let (wu, wv) = *surface_winds.get(x, y);
            let sx = (x as f32 - (TRANSPORT * wu / (REF_SPEED * cos_lat)).clamp(-4.0, 4.0)).rem_euclid(width as f32);
            let sy = (y as f32 + (TRANSPORT * wv / REF_SPEED).clamp(-3.0, 3.0)).clamp(0.0, (height - 1) as f32);
            let (x0, y0) = (sx.floor() as usize % width, sy.floor() as usize);
            let (x1, y1) = ((x0 + 1) % width, (y0 + 1).min(height - 1));
            let (fx, fy) = (sx - sx.floor(), sy - sy.floor());
            src.push([
                (y0 * width + x0, (1.0 - fx) * (1.0 - fy)),
                (y0 * width + x1, fx * (1.0 - fy)),
                (y1 * width + x0, (1.0 - fx) * fy),
                (y1 * width + x1, fx * fy),
            ]);
        }
    }

    let mut q: Vec<f32> = (0..n).map(|i| if elev[i] <= 0.0 { q_sat_sea[i] * 0.8 } else { q_sat[i] * 0.3 }).collect();
    let mut next = vec![0.0f32; n];
    let mut rain_sum = vec![0.0f32; n];

    for step in 0..STEPS {
        for i in 0..n {
            // Advection: vapour and the elevation it was carried from.
            let mut qi = 0.0;
            let mut src_h = 0.0;
            for &(j, w) in &src[i] {
                qi += w * q[j];
                src_h += w * smooth[j];
            }
            let land = elev[i] > 0.0;
            if !land {
                // Evaporation recharges marine air toward 80% humidity.
                qi += (q_sat_sea[i] * SEA_RH - qi).max(0.0) * 0.4;
            }

            // Rain: humid air in rising columns rains out; climbing air rains more.
            let rh = qi / q_sat[i];
            let humid = ((rh - 0.3) / 0.7).clamp(0.0, 1.0);
            let climb = if land { ((smooth[i] - src_h) / CLIMB_M).clamp(0.0, CLIMB_MAX) } else { 0.0 };
            let mut rain = qi * (rain_rate * lift[i] * humid * humid + climb * rh.min(1.0));
            // Supersaturated air condenses its excess at once.
            rain += (qi - rain - q_sat[i]).max(0.0);
            rain = rain.min(qi);
            qi -= rain;
            if land {
                qi += rain * RECYCLE * warmth[i];
            }
            next[i] = qi;
            if step >= SPIN_UP {
                rain_sum[i] += rain;
            }
        }
        std::mem::swap(&mut q, &mut next);
    }

    let steps = (STEPS - SPIN_UP) as f32;
    // A light 3x3 smoothing: at this cell size rainfall varies smoothly, and the semi-Lagrangian
    // scheme leaves streaks where winds turn sharply between pressure belts.
    for _ in 0..2 {
        let b = rain_sum.clone();
        for y in 0..height {
            for x in 0..width {
                let mut sum = 0.0;
                let mut wsum = 0.0;
                for dy in -1i32..=1 {
                    let yy = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;
                    for dx in -1i32..=1 {
                        let wgt = if dx == 0 && dy == 0 { 4.0 } else if dx == 0 || dy == 0 { 2.0 } else { 1.0 };
                        sum += wgt * b[yy * width + (x as i32 + dx).rem_euclid(width as i32) as usize];
                        wsum += wgt;
                    }
                }
                rain_sum[y * width + x] = sum / wsum;
            }
        }
    }
    let mut precip_map = Tilemap::new_with(width, height, 0.0f32);
    let mut moisture_map = Tilemap::new_with(width, height, 0.0f32);
    for (i, (_, _, p)) in precip_map.iter_mut().enumerate() {
        *p = (rain_sum[i] / steps * mm_per_unit * rainfall_multiplier).clamp(10.0, 6000.0);
    }
    for (i, (_, _, m)) in moisture_map.iter_mut().enumerate() {
        let p = *precip_map.get(i % width, i / width);
        *m = if elev[i] <= 0.0 { 1.0 } else { moisture_index(p, temp[i]).clamp(rainfall_floor, 1.0) };
    }
    (precip_map, moisture_map)
}
