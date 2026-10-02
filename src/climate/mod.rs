//! Physical climate simulation system
//!
//! Replaces empirical noise heuristics with an Energy Balance Model (insolation, albedo, meridional diffusion),
//! 3-cell atmospheric circulation, wind-driven ocean gyres, coastal upwelling, and Semi-Lagrangian
//! moisture advection with orographic precipitation and rain shadows.

pub mod biomes;
pub mod circulation;
pub mod ebm;
pub mod moisture;
pub mod ocean;
pub mod simulation;

pub use biomes::{Biome, FuzzyBiomeResult, generate_biomes, smooth_step, ELEVATION_LAPSE_RATE};
pub use circulation::{calculate_pressure_field, calculate_surface_winds, coriolis_parameter, OMEGA};
pub use ebm::{
    calculate_annual_insolation, calculate_daily_insolation, rossby_wave_perturbation, row_latitude,
    solve_energy_balance, surface_albedo, SOLAR_CONSTANT,
};
pub use moisture::{potential_evapotranspiration, saturation_humidity, simulate_moisture_and_precipitation};
pub use ocean::{
    apply_maritime_influence, calculate_coastal_upwelling, calculate_ocean_currents,
    calculate_sea_surface_temperatures,
};
pub use simulation::{run_climate_simulation, ClimateSimulation};

use crate::tilemap::Tilemap;

// =============================================================================
// CLIMATE CONFIGURATION
// =============================================================================

/// Climate simulation mode
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ClimateMode {
    /// Globe mode: Full spherical energy-balance model with 3-cell circulation and axial tilt
    #[default]
    Globe,
    /// Flat mode: Uniform base temperature across map, elevation lapse rate only
    Flat,
    /// Temperate band: Simulates a mid-latitude region
    TemperateBand,
    /// Tropical band: Simulates an equatorial region
    TropicalBand,
}

impl ClimateMode {
    pub fn all() -> &'static [Self] {
        &[Self::Globe, Self::Flat, Self::TemperateBand, Self::TropicalBand]
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Globe => "Spherical EBM (3-cell circulation, axial tilt, seasons)",
            Self::Flat => "Uniform temperature (elevation lapse rate only)",
            Self::TemperateBand => "Mid-latitude region (temperate)",
            Self::TropicalBand => "Equatorial region (tropical)",
        }
    }
}

impl std::fmt::Display for ClimateMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Globe => write!(f, "globe"),
            Self::Flat => write!(f, "flat"),
            Self::TemperateBand => write!(f, "temperate"),
            Self::TropicalBand => write!(f, "tropical"),
        }
    }
}

/// Rainfall/moisture level preset
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RainfallLevel {
    /// Arid: Deserts dominate, reduced moisture
    Arid,
    /// Normal: Earth-like moisture distribution
    #[default]
    Normal,
    /// Wet: Enhanced rainfall, larger forests
    Wet,
    /// Tropical: High moisture everywhere
    Tropical,
}

impl RainfallLevel {
    pub fn all() -> &'static [Self] {
        &[Self::Arid, Self::Normal, Self::Wet, Self::Tropical]
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Arid => "Desert world (~20% moisture)",
            Self::Normal => "Earth-like moisture distribution",
            Self::Wet => "Rainy world (~60% moisture)",
            Self::Tropical => "Jungle world (~80% moisture)",
        }
    }

    /// Base moisture multiplier for this level
    pub fn moisture_multiplier(&self) -> f32 {
        match self {
            Self::Arid => 0.45,
            Self::Normal => 1.0,
            Self::Wet => 1.5,
            Self::Tropical => 2.0,
        }
    }

    /// Minimum moisture floor
    pub fn moisture_floor(&self) -> f32 {
        match self {
            Self::Arid => 0.01,
            Self::Normal => 0.02,
            Self::Wet => 0.15,
            Self::Tropical => 0.35,
        }
    }
}

impl std::fmt::Display for RainfallLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Arid => write!(f, "arid"),
            Self::Normal => write!(f, "normal"),
            Self::Wet => write!(f, "wet"),
            Self::Tropical => write!(f, "tropical"),
        }
    }
}

/// Combined climate configuration
#[derive(Clone, Copy, Debug)]
pub struct ClimateConfig {
    pub mode: ClimateMode,
    pub rainfall: RainfallLevel,
    /// Axial tilt in degrees (default 23.44° for Earth-like seasonal amplitude)
    pub axial_tilt_deg: f32,
    /// Solar constant in W/m^2 (default 1361.0)
    pub solar_constant: f32,
}

impl Default for ClimateConfig {
    fn default() -> Self {
        Self {
            mode: ClimateMode::Globe,
            rainfall: RainfallLevel::Normal,
            axial_tilt_deg: 23.44,
            solar_constant: SOLAR_CONSTANT,
        }
    }
}

// =============================================================================
// BACKWARD-COMPATIBLE API FUNCTIONS
// =============================================================================

/// Generate annual mean temperature map based on the physical energy balance model.
/// Returns temperature in Celsius.
pub fn generate_temperature(
    heightmap: &Tilemap<f32>,
    _width: usize,
    _height: usize,
) -> Tilemap<f32> {
    generate_temperature_with_seed(heightmap, heightmap.width, heightmap.height, ClimateMode::Globe, 0)
}

/// Generate temperature map with configurable climate mode
pub fn generate_temperature_with_config(
    heightmap: &Tilemap<f32>,
    _width: usize,
    _height: usize,
    mode: ClimateMode,
) -> Tilemap<f32> {
    generate_temperature_with_seed(heightmap, heightmap.width, heightmap.height, mode, 0)
}

/// Generate temperature map using physical simulation with explicit seed
pub fn generate_temperature_with_seed(
    heightmap: &Tilemap<f32>,
    _width: usize,
    _height: usize,
    mode: ClimateMode,
    seed: u64,
) -> Tilemap<f32> {
    let config = ClimateConfig {
        mode,
        ..Default::default()
    };
    let sim = run_climate_simulation(heightmap, &config, seed);
    sim.mean_temperature
}

/// Generate annual mean moisture map based on physical advection and precipitation.
/// Returns moisture index in [0.0, 1.0].
pub fn generate_moisture(
    heightmap: &Tilemap<f32>,
    _width: usize,
    _height: usize,
) -> Tilemap<f32> {
    generate_moisture_with_config(heightmap, heightmap.width, heightmap.height, &ClimateConfig::default())
}

/// Generate moisture map with full climate configuration
pub fn generate_moisture_with_config(
    heightmap: &Tilemap<f32>,
    _width: usize,
    _height: usize,
    config: &ClimateConfig,
) -> Tilemap<f32> {
    generate_moisture_with_config_and_seed(heightmap, heightmap.width, heightmap.height, config, 0)
}

/// Generate moisture map with full configuration and explicit seed
pub fn generate_moisture_with_config_and_seed(
    heightmap: &Tilemap<f32>,
    _width: usize,
    _height: usize,
    config: &ClimateConfig,
    seed: u64,
) -> Tilemap<f32> {
    let sim = run_climate_simulation(heightmap, config, seed);
    sim.mean_moisture
}

/// Compute ocean distance field using BFS (kept for backward compatibility and analysis tools).
pub fn compute_ocean_distance(heightmap: &Tilemap<f32>) -> Tilemap<f32> {
    use std::collections::VecDeque;

    let width = heightmap.width;
    let height = heightmap.height;
    let mut ocean_distance = Tilemap::new_with(width, height, f32::MAX);
    let mut queue: VecDeque<(usize, usize, f32)> = VecDeque::new();

    for y in 0..height {
        for x in 0..width {
            if *heightmap.get(x, y) <= 0.0 {
                ocean_distance.set(x, y, 0.0);
                queue.push_back((x, y, 0.0));
            }
        }
    }

    while let Some((x, y, dist)) = queue.pop_front() {
        let neighbors = [
            (if x == 0 { width - 1 } else { x - 1 }, y),
            ((x + 1) % width, y),
            (x, y.saturating_sub(1)),
            (x, (y + 1).min(height - 1)),
        ];

        for (nx, ny) in neighbors {
            let new_dist = dist + 1.0;
            if new_dist < *ocean_distance.get(nx, ny) {
                ocean_distance.set(nx, ny, new_dist);
                queue.push_back((nx, ny, new_dist));
            }
        }
    }

    ocean_distance
}

/// Compute effective surface runoff per cell based on physical precipitation, temperature, and elevation.
/// Land cells produce positive runoff, while ocean cells produce 0.0.
pub fn compute_effective_runoff(
    heightmap: &Tilemap<f32>,
    temperature: &Tilemap<f32>,
    moisture: &Tilemap<f32>,
) -> Tilemap<f32> {
    let width = heightmap.width;
    let height = heightmap.height;
    let mut runoff = Tilemap::new_with(width, height, 0.0f32);

    for y in 0..height {
        for x in 0..width {
            let h = *heightmap.get(x, y);
            if h <= 0.0 {
                continue;
            }

            let temp = *temperature.get(x, y);
            let m = *moisture.get(x, y);

            // Orographic enhancement: high terrain captures extra condensation and precipitation
            let orographic = (h / 2500.0).clamp(0.0, 0.6);
            let precip = (m * (1.0 + orographic)).clamp(0.02, 2.0);

            // Potential Evapotranspiration (PET)
            let pet = potential_evapotranspiration(temp);

            // Temperature effects on snowpack and melt
            let eff_runoff = if temp < -8.0 {
                // Persistent deep freeze: modest base runoff
                precip * 0.15
            } else if temp < 1.0 {
                // Alpine / subpolar: freeze-thaw and snowmelt
                precip * 0.45
            } else {
                // Liquid runoff: precipitation minus evaporation
                (precip - pet * 0.55).max(0.04)
            };

            runoff.set(x, y, eff_runoff);
        }
    }

    runoff
}
