//! What the star, the orbit and the spin hand the climate: eccentricity and the season of
//! perihelion, the spin (Coriolis, Hadley cells, jets, heat transport), the year's length
//! (how much of each season's warmth the ground and sea take up), a face locked to the star,
//! the star's colour (albedo of ground, sea and ice), the air (greenhouse, scattering, heat
//! transport) and gravity (lapse rate).
//!
//! `PlanetForcing::default()` is the Earth under the Sun with the arithmetic of the original
//! model (`legacy`): it gives the climate the generator always made, bit for bit.

use super::ebm::ELEVATION_LAPSE_RATE;
use crate::astro::star::{hz_inner_relative, SUN_TEFF};

/// What sets the air's greenhouse.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AirForcing {
    /// Earth's air: the model's own greenhouse, nothing added.
    Earth,
    /// A fixed composition (bar).
    Fixed { co2_bar: f32, dry_bar: f32 },
    /// CO2 balanced by the carbonate-silicate cycle (`thermostat`): outgassing relative to
    /// Earth's, the weathering's CO2 exponent and its temperature sensitivity (per C).
    Cycle { dry_bar: f32, outgassing: f32, beta: f32, k_per_c: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlanetForcing {
    pub eccentricity: f32,
    /// Solar longitude of perihelion (radians from the northern spring equinox).
    pub perihelion_lon: f32,
    /// Sidereal spin relative to Earth's.
    pub rotation: f32,
    /// Orbital period in Earth years.
    pub year: f32,
    /// Longitude under the star (radians from the map's left edge) for a world locked to it.
    pub locked: Option<f32>,
    /// A 3:2 spin-orbit resonance's hot longitudes (radians; the star is overhead there and
    /// half a turn away at every perihelion) and the year-mean excess of sunlight they get.
    pub hot_longitudes: Option<(f32, f32)>,
    /// The star's effective temperature (K).
    pub teff: f32,
    /// C per km of height.
    pub lapse_rate: f32,
    pub air: AirForcing,
    /// The original model's arithmetic (declination as tilt x sin, equal seasons, per-cell
    /// insolation): only for the Earth under the Sun.
    pub legacy: bool,
}

impl Default for PlanetForcing {
    fn default() -> Self {
        PlanetForcing {
            eccentricity: 0.0,
            perihelion_lon: 0.0,
            rotation: 1.0,
            year: 1.0,
            locked: None,
            hot_longitudes: None,
            teff: SUN_TEFF,
            lapse_rate: ELEVATION_LAPSE_RATE,
            air: AirForcing::Earth,
            legacy: true,
        }
    }
}

/// Albedo under a star of temperature `teff`: (absorbed-share scale for open ground and sea,
/// land ice albedo, sea ice albedo). Red light is absorbed by water vapour and by ice, blue
/// light is scattered: the absorbed share scales as the inverse of Kopparapu's inner-edge flux
/// (so the habitable zone's edges come out where his 1-D models put them), and snow and ice
/// are darker under a red star (Shields et al. 2013: their albedo falls well below 0.5 in the
/// near infrared), which weakens the ice-albedo feedback around M dwarfs.
pub fn spectral_albedo(teff: f32) -> (f32, f32, f32) {
    let absorb = 1.0 / hz_inner_relative(teff);
    let red = ((SUN_TEFF - teff) / 2800.0).clamp(-0.5, 1.0);
    let ice_land = (0.75 - 0.25 * red).min(0.82);
    let ice_sea = (0.65 - 0.22 * red).min(0.72);
    (absorb, ice_land, ice_sea)
}

/// How a year of `years` Earth years damps the seasons relative to Earth's: the amplitude of a
/// layer with relaxation time tau driven at the year's period, A = 1 / sqrt(1 + (2 pi tau / P)^2),
/// over its amplitude at one year. Sea (tau ~0.6 yr) and inland ground (~0.2 yr).
pub fn seasonal_response(years: f32, tau_years: f32) -> f32 {
    let amp = |p: f32| 1.0 / (1.0 + (std::f32::consts::TAU * tau_years / p).powi(2)).sqrt();
    amp(years.max(0.01)) / amp(1.0)
}

/// Heat transport relative to Earth's for a spin (baroclinic eddies carry heat less well on a
/// fast-spinning world, Stone 1972; slow spinners and locked worlds carry it far more easily).
pub fn rotation_transport(rotation: f32) -> f32 {
    rotation.max(0.01).powf(-0.6).clamp(0.35, 5.0)
}
