//! Energy Balance Model (EBM) for planetary surface temperature
//!
//! Simulates insolation, dynamic albedo with ice feedback, meridional heat
//! transport on a sphere, and land-sea thermal inertia contrast.

use std::f32::consts::PI;
use noise::{NoiseFn, Perlin, Seedable};
use rayon::prelude::*;
use crate::tilemap::Tilemap;
use super::compute_ocean_distance;
use super::circulation::Circulation;
use super::forcing::{rotation_transport, seasonal_response, spectral_albedo, PlanetForcing};
use super::thermostat::AirState;

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
/// Extra albedo of the cloud deck over a locked world's substellar point (at the point; falls
/// off as cos^2 of the angle from it).
pub const SUBSTELLAR_CLOUD: f32 = 0.15;

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
    surface_albedo_with(elevation, sea_level_temp, lat_rad, &Albedo::EARTH)
}

/// Albedo parameters under a star: the absorbed share of open ground and sea (scaled from
/// Earth's), ice and snow albedo, the lapse rate (snow on heights), extra albedo from the air
/// (Rayleigh scattering).
#[derive(Clone, Copy, Debug)]
pub struct Albedo {
    pub absorb_scale: f32,
    pub ice_land: f32,
    pub ice_sea: f32,
    pub lapse_rate: f32,
    pub extra: f32,
}

impl Albedo {
    pub const EARTH: Albedo = Albedo { absorb_scale: 1.0, ice_land: 0.75, ice_sea: 0.65, lapse_rate: ELEVATION_LAPSE_RATE, extra: 0.0 };

    pub fn for_planet(f: &PlanetForcing, air: &AirState) -> Self {
        if f.legacy { return Self::EARTH; }
        let (absorb_scale, ice_land, ice_sea) = spectral_albedo(f.teff);
        Albedo { absorb_scale, ice_land, ice_sea, lapse_rate: f.lapse_rate, extra: air.rayleigh_albedo }
    }
}

/// `surface_albedo` with a star's and an air's parameters.
pub fn surface_albedo_with(elevation: f32, sea_level_temp: f32, lat_rad: f32, p: &Albedo) -> f32 {
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
    let base_albedo = if p.absorb_scale == 1.0 { base_albedo } else { 1.0 - (1.0 - base_albedo) * p.absorb_scale };

    // Ice/snow albedo feedback
    // Snow/ice forms when local temperature is near or below freezing
    let local_temp = sea_level_temp - (elevation.max(0.0) / 1000.0) * p.lapse_rate;

    // Smooth transition from open surface to ice/snow between -5°C and +2°C
    let ice_fraction = ((2.0 - local_temp) / 7.0).clamp(0.0, 1.0);
    let ice_albedo = if is_ocean { p.ice_sea } else { p.ice_land };

    let albedo = (1.0 - ice_fraction) * base_albedo + ice_fraction * ice_albedo;
    if p.extra == 0.0 { albedo } else { (albedo + p.extra).clamp(0.02, 0.95) }
}

/// Rossby planetary wave temperature perturbation in mid-latitudes (Celsius).
///
/// Undulates the polar jet stream and isotherms north and south with dominant
/// planetary wavenumbers 3, 4, and 5.
pub fn rossby_wave_perturbation(lon_rad: f32, lat_rad: f32, seed: u64) -> f32 {
    rossby_wave_with(lon_rad, lat_rad, seed, &Circulation::earth())
}

/// `rossby_wave_perturbation` on a planet's storm track (its latitude, width, wavenumbers).
pub fn rossby_wave_with(lon_rad: f32, lat_rad: f32, seed: u64, circ: &Circulation) -> f32 {
    if circ.rossby_amp == 0.0 { return 0.0; }
    let abs_lat = lat_rad.abs();
    // Jet stream and Rossby waves peak in mid-latitudes (35° to 65°)
    let midlat_center = circ.storm_lat_deg.to_radians();
    let midlat_width = circ.storm_width_deg.to_radians();
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
    let wave3 = (circ.waves[0] * lon_rad + phase1 + h_phase).sin();
    let wave4 = (circ.waves[1] * lon_rad + phase2 - h_phase).sin();
    let wave5 = (circ.waves[2] * lon_rad + phase3).sin();

    let combined = 0.55 * wave3 + 0.32 * wave4 + 0.13 * wave5;

    // Amplitude of ~8.0°C (gives realistic ~18 grid row undulations)
    lat_weight * combined * circ.rossby_amp
}

/// One season's place on the orbit: the star's declination and its flux relative to the flux
/// at the orbit's mean distance.
#[derive(Clone, Copy, Debug)]
pub struct SeasonPoint {
    pub declination: f32,
    pub flux_factor: f32,
}

/// Year-mean insolation (W/m^2) at a latitude on an eccentric, tilted orbit: the daily mean
/// averaged over the year in equal steps of time (mean anomaly), each step at its own distance
/// from the star and its own declination (sin d = sin(tilt) sin(solar longitude)).
pub fn annual_insolation_orbit(lat_rad: f32, axial_tilt_rad: f32, solar_const: f32, eccentricity: f32, perihelion_lon: f32) -> f32 {
    use crate::astro::orbit::{declination, flux_factor, true_anomaly};
    const STEPS: usize = 96;
    let mut sum = 0.0;
    for i in 0..STEPS {
        let m = (i as f32 + 0.5) / STEPS as f32 * 2.0 * PI;
        let nu = true_anomaly(m, eccentricity);
        let dec = declination(axial_tilt_rad, nu + perihelion_lon);
        sum += calculate_daily_insolation(lat_rad, dec, solar_const * flux_factor(nu, eccentricity));
    }
    sum / STEPS as f32
}

/// Insolation by latitude, tabulated every quarter degree for the per-cell lookups: the year's
/// mean and one season's daily mean.
struct InsolationTable {
    annual: Vec<f32>,
    season: Vec<f32>,
}

const TABLE_STEP_DEG: f32 = 0.25;

impl InsolationTable {
    fn new(f: &PlanetForcing, axial_tilt_rad: f32, solar_const: f32, season: Option<SeasonPoint>) -> Self {
        let n = (180.0 / TABLE_STEP_DEG) as usize + 1;
        let lat = |i: usize| (-90.0 + i as f32 * TABLE_STEP_DEG).to_radians();
        let annual = (0..n).map(|i| annual_insolation_orbit(lat(i), axial_tilt_rad, solar_const, f.eccentricity, f.perihelion_lon)).collect();
        let season = match season {
            Some(s) => (0..n).map(|i| calculate_daily_insolation(lat(i), s.declination, solar_const * s.flux_factor)).collect(),
            None => Vec::new(),
        };
        InsolationTable { annual, season }
    }

    fn lookup(table: &[f32], lat_rad: f32) -> f32 {
        let u = ((lat_rad.to_degrees() + 90.0) / TABLE_STEP_DEG).clamp(0.0, (table.len() - 1) as f32);
        let i = (u as usize).min(table.len() - 2);
        let t = u - i as f32;
        table[i] * (1.0 - t) + table[i + 1] * t
    }
}

/// Solve the spherical Energy Balance Model for a given heightmap and one season of the orbit
/// (`None`: the year's mean), under a planet's forcing, air and circulation.
///
/// Returns sea-level temperature map and actual ground surface temperature map.
pub fn solve_energy_balance(
    heightmap: &Tilemap<f32>,
    season: Option<SeasonPoint>,
    axial_tilt_rad: f32,
    solar_const: f32,
    seed: u64,
    perennial: Option<&Tilemap<f32>>,
    forcing: &PlanetForcing,
    air: &AirState,
    circ: &Circulation,
) -> (Tilemap<f32>, Tilemap<f32>) {
    let width = heightmap.width;
    let height = heightmap.height;
    let legacy = forcing.legacy;
    let is_annual_mean = season.is_none();
    let declination_rad = season.map(|s| s.declination).unwrap_or(0.0);
    let albedo_p = Albedo::for_planet(forcing, air);

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

    // How much of a season's warmth the sea and inland ground take up depends on the year's
    // length: a short year's seasons pass before they warm the sea, a long year's sink in.
    let (resp_sea, resp_land) = if legacy { (1.0, 1.0) } else { (seasonal_response(forcing.year, 0.6), seasonal_response(forcing.year, 0.21)) };
    let table = if legacy || forcing.locked.is_some() { None } else { Some(InsolationTable::new(forcing, axial_tilt_rad, solar_const, season)) };
    let mean_flux = 1.0 / (1.0 - forcing.eccentricity * forcing.eccentricity).sqrt();
    let season_flux = season.map(|s| s.flux_factor).unwrap_or(mean_flux);

    // Precompute warped effective latitude and insolation per cell. On a locked world the
    // "latitude" that matters is the angle from the point under the star.
    let mut effective_lat_map = Tilemap::new_with(width, height, 0.0f32);
    let mut insolation_map = Tilemap::new_with(width, height, 0.0f32);
    let mut zone_deg = Tilemap::new_with(width, height, 0.0f32);

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
            let cont = *continentality.get(x, y);
            let ocean = *heightmap.get(x, y) <= 0.0;
            // Effective seasonal insolation amplitude: damped by ocean and continental heat capacity
            let inertia_alpha = if ocean {
                0.20 * resp_sea // Ocean mixed layer thermal inertia
            } else {
                0.20 * resp_sea + 0.40 * cont * resp_land // Continental thermal inertia
            };
            let inertia_alpha = inertia_alpha.min(0.95);

            if let Some(lon0) = forcing.locked {
                let theta = (cos_phi * (lon_rad - lon0).cos()).clamp(-1.0, 1.0).acos();
                let eff = (theta + warp_val).clamp(0.0, PI);
                effective_lat_map.set(x, y, eff);
                zone_deg.set(x, y, theta.to_degrees().min(90.0));
                let s_annual = solar_const * mean_flux * eff.cos().max(0.0);
                let s = if is_annual_mean { s_annual } else { s_annual + inertia_alpha * (solar_const * season_flux * eff.cos().max(0.0) - s_annual) };
                insolation_map.set(x, y, s);
                continue;
            }

            let eff_lat = (lat_rad + warp_val).clamp(-PI * 0.5 + 0.01, PI * 0.5 - 0.01);
            effective_lat_map.set(x, y, eff_lat);
            zone_deg.set(x, y, lat_rad.to_degrees().abs());

            let s_annual = match &table {
                None => calculate_annual_insolation(eff_lat, axial_tilt_rad, solar_const),
                Some(t) => InsolationTable::lookup(&t.annual, eff_lat),
            };

            let s = if is_annual_mean {
                s_annual
            } else {
                let s_daily = match &table {
                    None => calculate_daily_insolation(eff_lat, declination_rad, solar_const),
                    Some(t) => InsolationTable::lookup(&t.season, eff_lat),
                };
                let delta_s = s_daily - s_annual;
                s_annual + inertia_alpha * delta_s
            };
            // A 3:2 resonance: the longitudes under the star at perihelion get more of its light
            // over the years, those a quarter turn away less (Mercury's hot and warm poles).
            let s = match forcing.hot_longitudes { Some((lon0, amp)) => s * (1.0 + amp * (2.0 * (lon_rad - lon0)).cos()), None => s };
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
    // (2.2 left 50-70 degrees 7-8 C too cold and the map 30-40% tundra). A slower spin or a
    // thicker air carries more.
    const MERIDIONAL_D: f32 = 3.0;
    let meridional_d = if legacy { MERIDIONAL_D } else { MERIDIONAL_D * rotation_transport(forcing.rotation) * air.transport };
    // The greenhouse beyond Earth's lowers the outgoing longwave at a given temperature.
    let olr_a = if legacy { OLR_A } else { OLR_A - air.greenhouse_wm2 };
    let (t_min, t_max) = if legacy { (-60.0, 50.0) } else { (-85.0, 58.0) };

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
                let mut albedo = surface_albedo_with(elev, curr_t, eff_lat, &albedo_p);
                // Ice sheets persist through the summer: where the annual mean at the surface is
                // ice-sheet cold, a season keeps ice albedo whatever its own temperature (solving
                // each season from scratch let polar summers melt their ice and warm by ~25 C).
                if let Some(annual) = perennial {
                    let annual_surface = *annual.get(x, y) - (elev.max(0.0) / 1000.0) * albedo_p.lapse_rate;
                    if annual_surface < PERENNIAL_ICE_C {
                        albedo = albedo.max(if elev <= 0.0 { albedo_p.ice_sea } else { albedo_p.ice_land });
                    }
                }
                // Under a fixed star, deep convection builds a bright cloud deck over the
                // substellar point (Yang et al. 2013), which keeps the day side from boiling.
                if forcing.locked.is_some() {
                    albedo = (albedo + SUBSTELLAR_CLOUD * eff_lat.cos().max(0.0).powi(2)).min(0.9);
                }
                let s = *insolation_map.get(x, y);
                let asr = s * (1.0 - albedo);
                asr_map.set(x, y, asr);
                total_asr += asr * w;
            }
        }

        let global_mean_asr = total_asr / total_weight;
        let global_mean_t = (global_mean_asr - olr_a) / OLR_B;

        let mut next_t = Tilemap::new_with(width, height, 0.0f32);

        // 2. Compute local sea-level temperature based on Budyko balance + Rossby waves + Continentality
        for y in 0..height {
            let lat_rad = row_latitude(y, height);

            for x in 0..width {
                let elev = *heightmap.get(x, y);
                let asr = *asr_map.get(x, y);
                let lon_rad = (x as f32 + 0.5) / width as f32 * 2.0 * PI;
                let lat_deg = *zone_deg.get(x, y);

                // Budyko local equilibrium temperature
                let t_eq_base = (asr - olr_a + meridional_d * global_mean_t) / (OLR_B + meridional_d);

                // Planetary Rossby wave undulation
                let t_rossby = rossby_wave_with(lon_rad, lat_rad, seed, circ);

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

                let final_t = (t_eq_base + t_rossby + t_cont).clamp(t_min, t_max);
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
        // A locked world's pattern runs east-west as much as north-south: diffuse along rows too.
        if forcing.locked.is_some() {
            let rows = diffused_t.clone();
            for y in 0..height {
                for x in 0..width {
                    let l = *rows.get((x + width - 1) % width, y);
                    let r = *rows.get((x + 1) % width, y);
                    let c = *rows.get(x, y);
                    diffused_t.set(x, y, c + 0.25 * (l + r - 2.0 * c));
                }
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
                (elev / 1000.0) * albedo_p.lapse_rate
            } else {
                0.0
            };
            t_surface.set(x, y, sl_temp - lapse_cooling);
        }
    }

    (t_sealevel, t_surface)
}
