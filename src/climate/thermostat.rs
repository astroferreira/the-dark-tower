//! The carbonate-silicate thermostat: how much CO2 a world's air holds.
//!
//! Volcanoes breathe CO2 out; rain dissolves it and weathers silicate rock, and the carbonate
//! ends up on the sea floor. Weathering runs faster the warmer and wetter the world and the
//! more CO2 there is (Walker, Hays & Kasting 1981): W = (pCO2 / pCO2_E)^beta x
//! exp(k (T - T_E)) x land / land_E. The air settles where weathering matches outgassing, and
//! the temperature there is the climate's at that CO2: a cold world far out in its zone
//! weathers slowly and builds up a thick CO2 blanket, a hot one near the inner edge draws it
//! down to almost nothing. That balance is found here with a zonal-mean version of the
//! energy-balance model (`ebm.rs`'s physics on bands of latitude, or of distance from the
//! substellar point for a locked world), by bisection on the global mean temperature.
//!
//! The weathering law is calibrated so that an Earth twin, this same land under the Sun with
//! Earth's air, keeps 280 ppm: `reference_c` is the zonal model's temperature for that case.

use std::f32::consts::PI;

use crate::astro::air::{broadening_wm2, co2_forcing_wm2, rayleigh_albedo, EARTH_CO2_BAR};
use crate::tilemap::Tilemap;

use super::ebm::{annual_insolation_orbit, calculate_daily_insolation, row_latitude, surface_albedo_with, Albedo, OLR_A, OLR_B, PERENNIAL_ICE_C, SUBSTELLAR_CLOUD};
use super::forcing::{rotation_transport, seasonal_response, spectral_albedo, AirForcing, PlanetForcing};

/// The air the climate runs with.
#[derive(Clone, Copy, Debug)]
pub struct AirState {
    pub co2_bar: f32,
    pub pressure_bar: f32,
    /// Greenhouse forcing beyond Earth's (W/m2: CO2 and pressure broadening).
    pub greenhouse_wm2: f32,
    /// Extra planetary albedo from Rayleigh scattering.
    pub rayleigh_albedo: f32,
    /// Heat transport relative to Earth's air (a thicker column carries more).
    pub transport: f32,
    /// The zonal model's global mean temperature at balance, and an Earth twin's (C).
    pub balance_c: f32,
    pub reference_c: f32,
}

impl AirState {
    pub fn earth() -> Self {
        AirState { co2_bar: EARTH_CO2_BAR, pressure_bar: 1.0, greenhouse_wm2: 0.0, rayleigh_albedo: 0.0, transport: 1.0, balance_c: f32::NAN, reference_c: f32::NAN }
    }
}

/// CO2 the thermostat allows (bar): ten parts per million to a thick 8 bar.
const CO2_MIN: f32 = 1e-5;
const CO2_MAX: f32 = 8.0;
/// Earth's land share (weathering scales with land area).
const EARTH_LAND: f32 = 0.29;
/// Zones of the zonal model (2.5 degrees each).
const ZONES: usize = 72;

struct Zone {
    weight: f32,
    land: f32,
    land_elev: f32,
    /// Latitude, or angle from the substellar point (radians, centre of the zone).
    coord: f32,
}

/// Bin the map into zones of latitude (or of distance from the substellar point).
fn zones(heightmap: &Tilemap<f32>, locked: Option<f32>) -> Vec<Zone> {
    let (w, h) = (heightmap.width, heightmap.height);
    let mut acc = vec![(0.0f64, 0.0f64, 0.0f64); ZONES];
    for y in 0..h {
        let lat = row_latitude(y, h);
        let area = lat.cos().max(0.001) as f64;
        for x in 0..w {
            let k = match locked {
                Some(lon0) => {
                    let lon = (x as f32 + 0.5) / w as f32 * 2.0 * PI;
                    let theta = (lat.cos() * (lon - lon0).cos()).clamp(-1.0, 1.0).acos();
                    ((theta / PI * ZONES as f32) as usize).min(ZONES - 1)
                }
                None => (((lat + PI * 0.5) / PI * ZONES as f32) as usize).min(ZONES - 1),
            };
            let e = *heightmap.get(x, y);
            acc[k].0 += area;
            if e > 0.0 {
                acc[k].1 += area;
                acc[k].2 += area * e as f64;
            }
        }
    }
    acc.iter().enumerate().map(|(k, &(wt, land, elev))| {
        let centre = (k as f32 + 0.5) / ZONES as f32 * PI;
        Zone {
            weight: wt as f32,
            land: if wt > 0.0 { (land / wt) as f32 } else { 0.0 },
            land_elev: if land > 0.0 { (elev / land) as f32 } else { 0.0 },
            coord: if locked.is_some() { centre } else { centre - PI * 0.5 },
        }
    }).collect()
}

/// The zones' starlight: the year's mean and each season's (the 2-D model's seasonal blend of
/// the daily mean into the annual by the sea's and the ground's thermal inertia), and the
/// seasons' shares of the year.
struct ZoneSun {
    annual: Vec<f32>,
    seasons: [Vec<f32>; 4],
    weights: [f32; 4],
}

fn zone_sun(zones: &[Zone], f: &PlanetForcing, tilt: f32, solar_const: f32) -> ZoneSun {
    let mean_flux = 1.0 / (1.0 - f.eccentricity * f.eccentricity).sqrt();
    let annual: Vec<f32> = zones.iter().map(|z| match f.locked {
        Some(_) => solar_const * mean_flux * z.coord.cos().max(0.0),
        None => annual_insolation_orbit(z.coord, tilt, solar_const, f.eccentricity, f.perihelion_lon),
    }).collect();
    let orbit = crate::astro::orbit::Orbit { semi_major_au: 1.0, eccentricity: f.eccentricity, perihelion_lon_deg: f.perihelion_lon.to_degrees(), period_days: 1.0 };
    let (flux, weights) = if f.eccentricity > 0.0 { (orbit.season_flux(), orbit.season_shares()) } else { ([1.0; 4], [0.25; 4]) };
    let (resp_sea, resp_land) = (seasonal_response(f.year, 0.6), seasonal_response(f.year, 0.21));
    let decs = [0.0, tilt, 0.0, -tilt];
    let seasons = [0, 1, 2, 3].map(|k| zones.iter().enumerate().map(|(i, z)| {
        let daily = match f.locked {
            Some(_) => solar_const * flux[k] * z.coord.cos().max(0.0),
            None => calculate_daily_insolation(z.coord, decs[k], solar_const * flux[k]),
        };
        // Coasts like the sea, inland ground (continentality ~0.55 on average) more.
        let alpha = (0.20 * resp_sea + z.land * 0.40 * 0.55 * resp_land).min(0.95);
        annual[i] + alpha * (daily - annual[i])
    }).collect());
    ZoneSun { annual, seasons, weights }
}

/// One zonal solve of the energy balance: temperature per zone (C, sea level). It runs the 2-D
/// model's own scheme (four half-relaxed steps of the ice-albedo feedback from 15 C, as
/// `solve_energy_balance` was tuned with), so the two agree on what a given air does; with the
/// year's mean (`perennial`), ice that never melts keeps its albedo through the summer.
fn zonal_solve(zones: &[Zone], insolation: &[f32], albedo: &Albedo, olr_a: f32, d: f32, locked: bool, perennial: Option<&[f32]>) -> Vec<f32> {
    let n = zones.len();
    let total: f32 = zones.iter().map(|z| z.weight).sum();
    let mut t = vec![15.0f32; n];
    for _ in 0..4 {
        let asr: Vec<f32> = (0..n).map(|k| {
            let z = &zones[k];
            let elev = z.land_elev.max(1.0);
            let mut land = surface_albedo_with(elev, t[k], z.coord, albedo);
            let mut sea = surface_albedo_with(0.0, t[k], z.coord, albedo);
            if let Some(annual) = perennial {
                if annual[k] - elev / 1000.0 * albedo.lapse_rate < PERENNIAL_ICE_C { land = land.max(albedo.ice_land); }
                if annual[k] < PERENNIAL_ICE_C { sea = sea.max(albedo.ice_sea); }
            }
            let mut a = z.land * land + (1.0 - z.land) * sea;
            if locked { a = (a + SUBSTELLAR_CLOUD * z.coord.cos().max(0.0).powi(2)).min(0.9); }
            insolation[k] * (1.0 - a)
        }).collect();
        let mean_asr = (0..n).map(|k| asr[k] * zones[k].weight).sum::<f32>() / total;
        let tg = (mean_asr - olr_a) / OLR_B;
        let teq: Vec<f32> = (0..n).map(|k| ((asr[k] - olr_a + d * tg) / (OLR_B + d)).clamp(-85.0, 58.0)).collect();
        for k in 0..n {
            let (p, q) = (teq[k.saturating_sub(1)], teq[(k + 1).min(n - 1)]);
            let diffused = teq[k] + 0.25 * (p + q - 2.0 * teq[k]);
            t[k] = 0.5 * t[k] + 0.5 * diffused;
        }
    }
    t
}

/// The zonal climate's global mean surface temperature (C): the year's mean solved first, then
/// each season against it, averaged by the seasons' lengths (as `run_climate_simulation` does).
fn zonal_climate(zones: &[Zone], sun: &ZoneSun, albedo: &Albedo, olr_a: f32, d: f32, locked: bool) -> f32 {
    let total: f32 = zones.iter().map(|z| z.weight).sum();
    let mean = |t: &[f32]| t.iter().zip(zones).map(|(t, z)| t * z.weight).sum::<f32>() / total;
    let annual = zonal_solve(zones, &sun.annual, albedo, olr_a, d, locked, None);
    (0..4).map(|k| sun.weights[k] * mean(&zonal_solve(zones, &sun.seasons[k], albedo, olr_a, d, locked, Some(&annual)))).sum::<f32>()
        / sun.weights.iter().sum::<f32>()
}

/// The air for a planet over this land: Earth's, a fixed one, or the thermostat's balance.
pub fn solve_air(heightmap: &Tilemap<f32>, f: &PlanetForcing, axial_tilt_rad: f32, solar_const: f32) -> AirState {
    if f.legacy { return AirState::earth(); }
    let (dry_bar, cycle) = match f.air {
        AirForcing::Earth => return AirState::earth(),
        AirForcing::Fixed { co2_bar, dry_bar } => (dry_bar, Err(co2_bar)),
        AirForcing::Cycle { dry_bar, outgassing, beta, k_per_c } => (dry_bar, Ok((outgassing, beta, k_per_c))),
    };
    let planet_zones = zones(heightmap, f.locked);
    let insolation = zone_sun(&planet_zones, f, axial_tilt_rad, solar_const);
    let (absorb_scale, ice_land, ice_sea) = spectral_albedo(f.teff);
    let rot_d = rotation_transport(f.rotation);

    // The air at a CO2 level, and the zonal climate it gives.
    let air_at = |co2: f32| {
        let p = dry_bar + co2;
        let greenhouse = co2_forcing_wm2(co2) + broadening_wm2(p);
        let rayleigh = rayleigh_albedo(p, f.teff);
        let transport = p.sqrt().clamp(0.3, 3.0);
        let albedo = Albedo { absorb_scale, ice_land, ice_sea, lapse_rate: f.lapse_rate, extra: rayleigh };
        let t = zonal_climate(&planet_zones, &insolation, &albedo, OLR_A - greenhouse, 3.0 * rot_d * transport, f.locked.is_some());
        (AirState { co2_bar: co2, pressure_bar: p, greenhouse_wm2: greenhouse, rayleigh_albedo: rayleigh, transport, balance_c: t, reference_c: f32::NAN }, t)
    };

    // An Earth twin: this land under the Sun with Earth's air.
    let earth_zones = zones(heightmap, None);
    let earth_f = PlanetForcing { legacy: false, ..PlanetForcing::default() };
    let earth_ins = zone_sun(&earth_zones, &earth_f, 23.44f32.to_radians(), super::ebm::SOLAR_CONSTANT);
    let reference = zonal_climate(&earth_zones, &earth_ins, &Albedo::EARTH, OLR_A, 3.0, false);

    let (outgassing, beta, k) = match cycle {
        Err(co2) => { let (mut a, _) = air_at(co2); a.reference_c = reference; return a; }
        Ok(c) => c,
    };
    let total: f32 = planet_zones.iter().map(|z| z.weight).sum();
    let land = planet_zones.iter().map(|z| z.land * z.weight).sum::<f32>() / total;
    let land_factor = (land / EARTH_LAND).clamp(0.15, 3.0);
    // CO2 at which weathering at temperature t matches the outgassing.
    let co2_for = |t: f32| (EARTH_CO2_BAR * (outgassing / (land_factor * (k * (t - reference)).exp())).powf(1.0 / beta)).clamp(CO2_MIN, CO2_MAX);
    let gap = |t: f32| air_at(co2_for(t)).1 - t;
    let (mut lo, mut hi) = (-80.0f32, 60.0f32);
    let t = if gap(lo) <= 0.0 { lo } else if gap(hi) >= 0.0 { hi } else {
        for _ in 0..32 {
            let mid = 0.5 * (lo + hi);
            if gap(mid) > 0.0 { lo = mid; } else { hi = mid; }
        }
        0.5 * (lo + hi)
    };
    let (mut a, _) = air_at(co2_for(t));
    a.reference_c = reference;
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    fn earthlike_land(w: usize, h: usize) -> Tilemap<f32> {
        let mut m = Tilemap::new_with(w, h, -3000.0f32);
        for y in 0..h { for x in 0..w { if (x * 7 + y * 3) % 10 < 3 { m.set(x, y, 400.0); } } }
        m
    }

    fn cycle() -> AirForcing { AirForcing::Cycle { dry_bar: 1.0, outgassing: 1.0, beta: 0.5, k_per_c: 0.09 } }

    #[test]
    fn an_earth_twin_keeps_earths_co2() {
        let hm = earthlike_land(96, 48);
        let f = PlanetForcing { legacy: false, air: cycle(), ..PlanetForcing::default() };
        let a = solve_air(&hm, &f, 23.44f32.to_radians(), 1361.0);
        assert!((a.co2_bar / EARTH_CO2_BAR - 1.0).abs() < 0.2, "co2 {} bar", a.co2_bar);
        assert!((a.balance_c - a.reference_c).abs() < 0.5);
    }

    #[test]
    fn the_thermostat_resists_dimmer_and_brighter_suns() {
        let hm = earthlike_land(96, 48);
        let f = PlanetForcing { legacy: false, air: cycle(), ..PlanetForcing::default() };
        let tilt = 23.44f32.to_radians();
        let earth = solve_air(&hm, &f, tilt, 1361.0);
        let dim = solve_air(&hm, &f, tilt, 1361.0 * 0.85);
        let bright = solve_air(&hm, &f, tilt, 1361.0 * 1.1);
        assert!(dim.co2_bar > earth.co2_bar * 5.0, "a dim sun builds CO2: {} bar", dim.co2_bar);
        assert!(bright.co2_bar < earth.co2_bar, "a bright sun draws it down: {} bar", bright.co2_bar);
        assert!(dim.balance_c < earth.balance_c && bright.balance_c > earth.balance_c);
        // Without the cycle, the dim world would be far colder.
        let fixed = PlanetForcing { legacy: false, air: AirForcing::Fixed { co2_bar: EARTH_CO2_BAR, dry_bar: 1.0 }, ..PlanetForcing::default() };
        let dim_fixed = solve_air(&hm, &fixed, tilt, 1361.0 * 0.85);
        assert!(dim_fixed.balance_c < dim.balance_c - 3.0, "fixed {} vs cycle {}", dim_fixed.balance_c, dim.balance_c);
    }
}
