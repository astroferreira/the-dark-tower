//! Complete planetary climate simulation orchestration
//!
//! Solves seasonal insolation, energy balance, atmospheric circulation,
//! ocean gyres, and moisture advection to generate full planetary climate fields.

use std::f32::consts::PI;
use crate::tilemap::Tilemap;
use super::ebm::{solve_energy_balance, SeasonPoint, ELEVATION_LAPSE_RATE};
use super::circulation::{calculate_pressure_field, calculate_surface_winds, Circulation};
use super::forcing::PlanetForcing;
use super::thermostat::{solve_air, AirState};
use super::ocean::{
    calculate_ocean_currents, calculate_coastal_upwelling, calculate_sea_surface_temperatures,
    apply_maritime_influence,
};
use super::moisture::simulate_moisture_and_precipitation;
use super::{ClimateConfig, ClimateMode};

/// Full output of a physical planetary climate simulation
#[derive(Clone)]
pub struct ClimateSimulation {
    /// Annual mean ground surface temperature in Celsius
    pub mean_temperature: Tilemap<f32>,
    /// Annual mean moisture index in [0.0, 1.0]
    pub mean_moisture: Tilemap<f32>,
    /// Annual precipitation in mm/year
    pub annual_precipitation: Tilemap<f32>,
    /// Annual prevailing surface winds (u, v) in m/s
    pub prevailing_winds: Tilemap<(f32, f32)>,
    /// Wind-driven ocean currents (u, v) in m/s
    pub ocean_currents: Tilemap<(f32, f32)>,
    /// Sea surface temperature in Celsius
    pub sea_surface_temperature: Tilemap<f32>,
    /// Seasonal ground surface temperatures: [Spring, Summer, Autumn, Winter] in Celsius
    pub seasonal_temperatures: [Tilemap<f32>; 4],
    /// Seasonal moisture indexes: [Spring, Summer, Autumn, Winter] in [0.0, 1.0]
    pub seasonal_moistures: [Tilemap<f32>; 4],
    /// Seasonal precipitation: [Spring, Summer, Autumn, Winter] in mm/year
    pub seasonal_precipitation: [Tilemap<f32>; 4],
    /// Seasonal wind fields: [Spring, Summer, Autumn, Winter] in m/s
    pub seasonal_winds: [Tilemap<(f32, f32)>; 4],
    /// Temperature amplitude (half of annual range) in Celsius
    pub temp_amplitude: Tilemap<f32>,
    /// Moisture amplitude (half of annual range) in [0.0, 1.0]
    pub moisture_amplitude: Tilemap<f32>,
    /// Moisture phase offset (0 = wet summer, PI = wet winter / Mediterranean)
    pub moisture_phase: Tilemap<f32>,
    /// What drove it: the star, orbit and spin, the air the thermostat settled on, the
    /// circulation cells.
    pub forcing: PlanetForcing,
    pub air: AirState,
    pub circulation: Circulation,
    /// Each season's share of the year (equal unless the orbit is eccentric).
    pub season_shares: [f32; 4],
}

impl ClimateSimulation {
    /// The warmest season's mean temperature per cell (northern summer in the north, southern
    /// summer in the south): what sets the tree line.
    pub fn warmest_season(&self) -> Tilemap<f32> {
        let (w, h) = (self.mean_temperature.width, self.mean_temperature.height);
        let mut out = Tilemap::new_with(w, h, 0.0f32);
        for y in 0..h {
            for x in 0..w {
                let t = self.seasonal_temperatures.iter().map(|s| *s.get(x, y)).fold(f32::MIN, f32::max);
                out.set(x, y, t);
            }
        }
        out
    }
}

/// Run full physical climate simulation
pub fn run_climate_simulation(
    heightmap: &Tilemap<f32>,
    config: &ClimateConfig,
    seed: u64,
) -> ClimateSimulation {
    let width = heightmap.width;
    let height = heightmap.height;

    let axial_tilt_rad = config.axial_tilt_deg.to_radians();
    let forcing = config.planet;
    let solar_const = config.solar_constant;
    let rain_mult = config.rainfall.moisture_multiplier();
    let rain_floor = config.rainfall.moisture_floor();

    match config.mode {
        ClimateMode::Globe => {
            // Simulate 4 seasons:
            // Season 0: Spring Equinox (t=0.0, delta=0)
            // Season 1: Northern Summer Solstice (t=0.25, delta=+tilt)
            // Season 2: Autumn Equinox (t=0.50, delta=0)
            // Season 3: Northern Winter Solstice (t=0.75, delta=-tilt)
            let season_decs = [
                0.0f32,
                axial_tilt_rad,
                0.0f32,
                -axial_tilt_rad,
            ];

            let itcz_shifts = if forcing.locked.is_some() { [0.0f32; 4] } else { [
                0.0f32,
                axial_tilt_rad * 0.45,
                0.0f32,
                -axial_tilt_rad * 0.45,
            ] };

            // An eccentric orbit: each season at its own distance from the star, and the seasons
            // of unequal length (Kepler's second law) in the year's means.
            let equal = forcing.legacy || forcing.eccentricity == 0.0;
            let orbit = crate::astro::orbit::Orbit { semi_major_au: 1.0, eccentricity: forcing.eccentricity, perihelion_lon_deg: forcing.perihelion_lon.to_degrees(), period_days: 1.0 };
            let season_flux = if equal { [1.0f32; 4] } else { orbit.season_flux() };
            let season_shares = if equal { [0.25f32; 4] } else { orbit.season_shares() };
            let (weights, norm) = if equal { ([1.0f32; 4], 4.0f32) } else { (season_shares, season_shares.iter().sum::<f32>()) };

            // The circulation cells for the spin, and the air the carbonate-silicate cycle settles on.
            let circ = Circulation::for_planet(&forcing);
            let air = solve_air(heightmap, &forcing, axial_tilt_rad, solar_const);

            // The annual mean first: it decides where ice persists through the summer.
            let (annual_sl, _) = solve_energy_balance(heightmap, None, axial_tilt_rad, solar_const, seed, None, &forcing, &air, &circ);

            let mut seasonal_temps: Vec<Tilemap<f32>> = Vec::with_capacity(4);
            let mut seasonal_moists: Vec<Tilemap<f32>> = Vec::with_capacity(4);
            let mut seasonal_precips: Vec<Tilemap<f32>> = Vec::with_capacity(4);
            let mut seasonal_winds: Vec<Tilemap<(f32, f32)>> = Vec::with_capacity(4);
            let mut seasonal_ssts: Vec<Tilemap<f32>> = Vec::with_capacity(4);
            let mut seasonal_currents: Vec<Tilemap<(f32, f32)>> = Vec::with_capacity(4);

            for s in 0..4 {
                let dec = season_decs[s];
                let itcz_lat = itcz_shifts[s];
                let season_seed = seed;

                // 1. Energy Balance Model (with insolation warping, continentality, and Rossby waves)
                let (t_sl, t_surf) = solve_energy_balance(
                    heightmap,
                    Some(SeasonPoint { declination: dec, flux_factor: season_flux[s] }),
                    axial_tilt_rad,
                    solar_const,
                    season_seed,
                    Some(&annual_sl),
                    &forcing,
                    &air,
                    &circ,
                );

                // 2. Pressure & Winds (derives closed pressure cells from 2D thermal anomalies)
                let pressure = calculate_pressure_field(heightmap, &t_sl, itcz_lat, &circ);
                let winds = calculate_surface_winds(&pressure, heightmap, &circ);

                // 3. Ocean Currents & Upwelling
                let currents = calculate_ocean_currents(heightmap, &winds);
                let upwelling = calculate_coastal_upwelling(heightmap, &winds, circ.upwelling);
                let sst = calculate_sea_surface_temperatures(heightmap, &t_sl, &currents, &upwelling);

                // 3b. Maritime Air Thermal Moderation
                // Advects oceanic temperature buffer onto adjacent landmasses along wind vectors
                let t_surf_moderated = apply_maritime_influence(heightmap, &t_surf, &sst, &winds, forcing.lapse_rate);

                // 4. Moisture Advection & Orographic Precipitation
                let (precip, moist) = simulate_moisture_and_precipitation(
                    heightmap,
                    &t_surf_moderated,
                    &sst,
                    &winds,
                    &pressure,
                    rain_mult,
                    rain_floor,
                );

                seasonal_temps.push(t_surf_moderated);
                seasonal_moists.push(moist);
                seasonal_precips.push(precip);
                seasonal_winds.push(winds);
                seasonal_ssts.push(sst);
                seasonal_currents.push(currents);
            }

            // Calculate annual means
            let mut mean_temp = Tilemap::new_with(width, height, 0.0f32);
            let mut mean_moist = Tilemap::new_with(width, height, 0.0f32);
            let mut annual_precip = Tilemap::new_with(width, height, 0.0f32);
            let mut prevailing_winds = Tilemap::new_with(width, height, (0.0f32, 0.0f32));
            let mut mean_currents = Tilemap::new_with(width, height, (0.0f32, 0.0f32));
            let mut mean_sst = Tilemap::new_with(width, height, 0.0f32);

            let mut temp_amp = Tilemap::new_with(width, height, 0.0f32);
            let mut moist_amp = Tilemap::new_with(width, height, 0.0f32);
            let mut moist_phase = Tilemap::new_with(width, height, 0.0f32);

            for y in 0..height {
                for x in 0..width {
                    let mut sum_t = 0.0f32;
                    let mut sum_m = 0.0f32;
                    let mut sum_p = 0.0f32;
                    let mut sum_wu = 0.0f32;
                    let mut sum_wv = 0.0f32;
                    let mut sum_cu = 0.0f32;
                    let mut sum_cv = 0.0f32;
                    let mut sum_sst = 0.0f32;

                    let mut min_t = f32::MAX;
                    let mut max_t = f32::MIN;

                    let m_summer = *seasonal_moists[1].get(x, y);
                    let m_winter = *seasonal_moists[3].get(x, y);

                    for s in 0..4 {
                        let wk = weights[s];
                        let t = *seasonal_temps[s].get(x, y);
                        sum_t += t * wk;
                        if t < min_t { min_t = t; }
                        if t > max_t { max_t = t; }

                        sum_m += *seasonal_moists[s].get(x, y) * wk;
                        sum_p += *seasonal_precips[s].get(x, y) * wk;

                        let (wu, wv) = *seasonal_winds[s].get(x, y);
                        sum_wu += wu * wk;
                        sum_wv += wv * wk;

                        let (cu, cv) = *seasonal_currents[s].get(x, y);
                        sum_cu += cu * wk;
                        sum_cv += cv * wk;

                        sum_sst += *seasonal_ssts[s].get(x, y) * wk;
                    }

                    mean_temp.set(x, y, sum_t / norm);
                    mean_moist.set(x, y, sum_m / norm);
                    annual_precip.set(x, y, sum_p / norm);
                    prevailing_winds.set(x, y, (sum_wu / norm, sum_wv / norm));
                    mean_currents.set(x, y, (sum_cu / norm, sum_cv / norm));
                    mean_sst.set(x, y, sum_sst / norm);

                    // Seasonal amplitudes
                    let t_amplitude = (max_t - min_t) * 0.5;
                    temp_amp.set(x, y, t_amplitude);

                    let m_amplitude = (m_summer - m_winter).abs() * 0.5;
                    moist_amp.set(x, y, m_amplitude);

                    // Moisture phase:
                    // If winter is wetter than summer -> Mediterranean phase (PI)
                    // If summer is wetter than winter -> Monsoon / Tropical phase (0)
                    let phase = if m_winter > m_summer + 0.04 {
                        PI
                    } else {
                        0.0
                    };
                    moist_phase.set(x, y, phase);
                }
            }

            ClimateSimulation {
                mean_temperature: mean_temp,
                mean_moisture: mean_moist,
                annual_precipitation: annual_precip,
                prevailing_winds,
                ocean_currents: mean_currents,
                sea_surface_temperature: mean_sst,
                seasonal_temperatures: [
                    seasonal_temps.remove(0),
                    seasonal_temps.remove(0),
                    seasonal_temps.remove(0),
                    seasonal_temps.remove(0),
                ],
                seasonal_moistures: [
                    seasonal_moists.remove(0),
                    seasonal_moists.remove(0),
                    seasonal_moists.remove(0),
                    seasonal_moists.remove(0),
                ],
                seasonal_precipitation: [
                    seasonal_precips.remove(0),
                    seasonal_precips.remove(0),
                    seasonal_precips.remove(0),
                    seasonal_precips.remove(0),
                ],
                seasonal_winds: [
                    seasonal_winds.remove(0),
                    seasonal_winds.remove(0),
                    seasonal_winds.remove(0),
                    seasonal_winds.remove(0),
                ],
                temp_amplitude: temp_amp,
                moisture_amplitude: moist_amp,
                moisture_phase: moist_phase,
                forcing,
                air,
                circulation: circ,
                season_shares,
            }
        }
        ClimateMode::Flat | ClimateMode::TemperateBand | ClimateMode::TropicalBand => {
            // Regional / non-globe modes: uniform or prescribed base temperatures
            let base_target = match config.mode {
                ClimateMode::Flat => 15.0f32,
                ClimateMode::TemperateBand => 12.0f32,
                ClimateMode::TropicalBand => 27.0f32,
                _ => 15.0f32,
            };

            let mut mean_temp = Tilemap::new_with(width, height, 0.0f32);
            for y in 0..height {
                for x in 0..width {
                    let elev = *heightmap.get(x, y);
                    let cooling = if elev > 0.0 {
                        (elev / 1000.0) * ELEVATION_LAPSE_RATE
                    } else {
                        0.0
                    };
                    mean_temp.set(x, y, base_target - cooling);
                }
            }

            // Simple prevailing westerlies
            let winds = Tilemap::new_with(width, height, (8.0f32, 0.0f32));
            let pressure = Tilemap::new_with(width, height, 1010.0f32);
            let (precip, moist) = simulate_moisture_and_precipitation(
                heightmap,
                &mean_temp,
                &mean_temp,
                &winds,
                &pressure,
                rain_mult,
                rain_floor,
            );

            ClimateSimulation {
                mean_temperature: mean_temp.clone(),
                mean_moisture: moist.clone(),
                annual_precipitation: precip.clone(),
                prevailing_winds: winds.clone(),
                ocean_currents: Tilemap::new_with(width, height, (0.0, 0.0)),
                sea_surface_temperature: mean_temp.clone(),
                seasonal_temperatures: [mean_temp.clone(), mean_temp.clone(), mean_temp.clone(), mean_temp.clone()],
                seasonal_moistures: [moist.clone(), moist.clone(), moist.clone(), moist.clone()],
                seasonal_precipitation: [precip.clone(), precip.clone(), precip.clone(), precip.clone()],
                seasonal_winds: [winds.clone(), winds.clone(), winds.clone(), winds.clone()],
                temp_amplitude: Tilemap::new_with(width, height, 3.0),
                moisture_amplitude: Tilemap::new_with(width, height, 0.05),
                moisture_phase: Tilemap::new_with(width, height, 0.0),
                forcing,
                air: AirState::earth(),
                circulation: Circulation::earth(),
                season_shares: [0.25; 4],
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ebm_equator_hotter_than_poles() {
        let width = 64;
        let height = 32;
        let heightmap = Tilemap::new_with(width, height, 0.0f32); // ocean planet
        let config = ClimateConfig::default();

        let sim = run_climate_simulation(&heightmap, &config, 42);

        let north_pole_t = *sim.mean_temperature.get(width / 2, 0);
        let equator_t = *sim.mean_temperature.get(width / 2, height / 2);
        let south_pole_t = *sim.mean_temperature.get(width / 2, height - 1);

        assert!(equator_t > north_pole_t + 20.0, "Equator ({:.1}°C) must be much warmer than North Pole ({:.1}°C)", equator_t, north_pole_t);
        assert!(equator_t > south_pole_t + 20.0, "Equator ({:.1}°C) must be much warmer than South Pole ({:.1}°C)", equator_t, south_pole_t);
        assert!(equator_t > 18.0, "Tropical temperatures should be warm at sea level");
        assert!(north_pole_t < 5.0, "Polar temperatures should be cold, got {:.1}°C", north_pole_t);
    }

    #[test]
    fn test_axial_tilt_seasons() {
        let width = 64;
        let height = 32;
        let heightmap = Tilemap::new_with(width, height, 100.0f32); // land planet
        let config = ClimateConfig::default();

        let sim = run_climate_simulation(&heightmap, &config, 42);

        // Season 1: Northern Summer, Season 3: Northern Winter
        let nh_y = height / 4; // ~45°N
        let sh_y = height * 3 / 4; // ~45°S
        let x = width / 2;

        let nh_summer_t = *sim.seasonal_temperatures[1].get(x, nh_y);
        let nh_winter_t = *sim.seasonal_temperatures[3].get(x, nh_y);
        assert!(nh_summer_t > nh_winter_t + 5.0, "NH Summer ({:.1}°C) must be warmer than NH Winter ({:.1}°C)", nh_summer_t, nh_winter_t);

        let sh_summer_t = *sim.seasonal_temperatures[3].get(x, sh_y);
        let sh_winter_t = *sim.seasonal_temperatures[1].get(x, sh_y);
        assert!(sh_summer_t > sh_winter_t + 5.0, "SH Summer ({:.1}°C) must be warmer than SH Winter ({:.1}°C)", sh_summer_t, sh_winter_t);
    }

    #[test]
    fn test_three_cell_winds() {
        let width = 64;
        let height = 32;
        let heightmap = Tilemap::new_with(width, height, 0.0f32);
        let config = ClimateConfig::default();

        let sim = run_climate_simulation(&heightmap, &config, 42);

        // Check zonal winds u:
        // Trade winds: ~15°N (y around 13) -> u < 0 (Easterlies)
        let trade_y = (height as f32 * (0.5 - 15.0 / 180.0)) as usize;
        let (trade_u, _) = *sim.prevailing_winds.get(width / 2, trade_y);
        assert!(trade_u < 0.0, "Trade winds at 15°N should be westward (u < 0), got {:.2}", trade_u);

        // Westerlies: ~45°N (y around 8) -> u > 0 (Westerlies)
        let west_y = (height as f32 * (0.5 - 45.0 / 180.0)) as usize;
        let (west_u, _) = *sim.prevailing_winds.get(width / 2, west_y);
        assert!(west_u > 0.0, "Mid-latitude winds at 45°N should be eastward (u > 0), got {:.2}", west_u);

        // Polar easterlies: ~75°N (y around 2) -> u < 0 (Easterlies)
        let polar_y = (height as f32 * (0.5 - 75.0 / 180.0)) as usize;
        let (polar_u, _) = *sim.prevailing_winds.get(width / 2, polar_y);
        assert!(polar_u < 0.0, "Polar winds at 75°N should be westward (u < 0), got {:.2}", polar_u);
    }

    #[test]
    fn test_orographic_rain_shadow() {
        let width = 64;
        let height = 32;
        let mut heightmap = Tilemap::new_with(width, height, 0.0f32);

        // In the mid-latitude Westerlies (~45°N, y around 8), winds blow from West to East (u > 0).
        // Place a continent with a North-South mountain range at x = 32:
        // Land from x = 20 to 50
        // Mountains from x = 30 to 34 (height 2500m)
        let mid_y = 8;
        for y in (mid_y - 3)..=(mid_y + 3) {
            for x in 20..=50 {
                heightmap.set(x, y, 150.0);
            }
            for x in 30..=34 {
                heightmap.set(x, y, 2500.0);
            }
        }

        let config = ClimateConfig::default();
        let sim = run_climate_simulation(&heightmap, &config, 42);

        // Windward slope of mountains (x = 30) should have high orographic precipitation
        // compared to leeward rain shadow (x = 38)
        let windward_p = *sim.annual_precipitation.get(30, mid_y);
        let leeward_p = *sim.annual_precipitation.get(38, mid_y);

        assert!(
            windward_p > leeward_p,
            "Windward precipitation ({:.1}mm) must exceed leeward rain shadow ({:.1}mm)",
            windward_p,
            leeward_p
        );
    }

    fn planet(f: PlanetForcing, tilt: f32) -> ClimateConfig {
        ClimateConfig { axial_tilt_deg: tilt, planet: PlanetForcing { legacy: false, ..f }, ..ClimateConfig::default() }
    }

    fn row_mean(t: &Tilemap<f32>, y: usize) -> f32 { (0..t.width).map(|x| *t.get(x, y)).sum::<f32>() / t.width as f32 }

    #[test]
    fn a_steep_tilt_warms_the_poles_past_the_equator() {
        let hm = Tilemap::new_with(64, 32, -3000.0f32);
        let sim = run_climate_simulation(&hm, &planet(PlanetForcing::default(), 80.0), 42);
        let (pole, equator) = (0.5 * (row_mean(&sim.mean_temperature, 0) + row_mean(&sim.mean_temperature, 31)), row_mean(&sim.mean_temperature, 16));
        assert!(pole > equator, "tilted 80 deg: poles {pole:.1} C, equator {equator:.1} C");
        let earth = run_climate_simulation(&hm, &planet(PlanetForcing::default(), 23.44), 42);
        assert!(row_mean(&earth.mean_temperature, 0) < row_mean(&earth.mean_temperature, 16) - 20.0);
    }

    #[test]
    fn a_locked_world_burns_under_its_star_and_freezes_behind() {
        let hm = Tilemap::new_with(64, 32, -3000.0f32);
        let f = PlanetForcing { locked: Some(0.25 * std::f32::consts::PI), rotation: 0.05, year: 0.08, ..PlanetForcing::default() };
        let sim = run_climate_simulation(&hm, &planet(f, 0.0), 42);
        // The substellar point at 45 deg E (x = 8 of 64); the antistellar at x = 40.
        let (day, night) = (*sim.mean_temperature.get(8, 16), *sim.mean_temperature.get(40, 16));
        assert!(day > night + 25.0, "day side {day:.1} C, night side {night:.1} C");
        let (wet, dry) = (*sim.annual_precipitation.get(8, 16), *sim.annual_precipitation.get(40, 16));
        assert!(wet > 2.0 * dry, "rain under the star {wet:.0} mm, behind it {dry:.0} mm");
        assert!(sim.circulation.locked && sim.circulation.rossby_amp == 0.0);
    }

    #[test]
    fn the_spin_sets_the_cells() {
        let cells = |r: f32| Circulation::for_planet(&PlanetForcing { legacy: false, rotation: r, ..PlanetForcing::default() }).belts.len() - 1;
        assert_eq!(cells(1.0), 3, "Earth's three cells");
        assert_eq!(cells(0.05), 1, "a slow spinner's single Hadley cell");
        assert!(cells(4.0) >= 5, "a fast spinner's bands: {}", cells(4.0));
        let c = Circulation::for_planet(&PlanetForcing { legacy: false, ..PlanetForcing::default() });
        assert_eq!(c.belts, Circulation::earth().belts);
        assert_eq!(c.zonal, Circulation::earth().zonal);
    }

    #[test]
    fn an_eccentric_orbit_makes_one_hemisphere_harsher() {
        let hm = Tilemap::new_with(64, 32, 200.0f32);
        // Perihelion at the northern summer solstice: northern summers near the star, southern
        // summers far from it.
        let f = PlanetForcing { eccentricity: 0.3, perihelion_lon: 0.5 * std::f32::consts::PI, ..PlanetForcing::default() };
        let sim = run_climate_simulation(&hm, &planet(f, 23.44), 42);
        let (n, s) = (8, 23);
        let north_summer = row_mean(&sim.seasonal_temperatures[1], n);
        let south_summer = row_mean(&sim.seasonal_temperatures[3], s);
        assert!(north_summer > south_summer + 3.0, "northern summer {north_summer:.1} C, southern {south_summer:.1} C");
        assert!(sim.season_shares[1] < sim.season_shares[3], "the summer at perihelion is the shorter: {:?}", sim.season_shares);
    }

    #[test]
    fn a_short_year_damps_the_seasons() {
        let hm = Tilemap::new_with(64, 32, 200.0f32);
        let range = |year: f32| {
            let sim = run_climate_simulation(&hm, &planet(PlanetForcing { year, ..PlanetForcing::default() }, 23.44), 42);
            row_mean(&sim.seasonal_temperatures[1], 8) - row_mean(&sim.seasonal_temperatures[3], 8)
        };
        let (short, earth, long) = (range(0.1), range(1.0), range(3.0));
        assert!(short < 0.5 * earth && long > earth, "summer-winter at 45 N: 0.1 yr {short:.1}, 1 yr {earth:.1}, 3 yr {long:.1}");
    }

    #[test]
    fn red_light_darkens_ice() {
        let (sun_abs, sun_ice, _) = super::super::forcing::spectral_albedo(5772.0);
        let (m_abs, m_ice, m_sea_ice) = super::super::forcing::spectral_albedo(3000.0);
        assert!((sun_abs - 1.0).abs() < 0.01 && (sun_ice - 0.75).abs() < 0.01);
        assert!(m_abs > 1.1 && m_ice < 0.55 && m_sea_ice < 0.5, "M dwarf: absorb x{m_abs}, ice {m_ice}, sea ice {m_sea_ice}");
    }
}
