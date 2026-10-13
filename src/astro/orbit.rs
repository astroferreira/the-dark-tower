//! The world's orbit and spin: Kepler's laws for the year and the seasons, the solar day from
//! the spin and the orbit, and tides that slow the spin until one face is locked to the star.

use serde::{Deserialize, Serialize};
use std::f32::consts::{PI, TAU};

/// Earth's sidereal day (hours): its spin relative to the stars.
pub const EARTH_SIDEREAL_HOURS: f32 = 23.9345;
/// Days (of 24 hours) in Earth's year.
pub const EARTH_YEAR_DAYS: f32 = 365.256;

/// Solve Kepler's equation M = E - e sin E for the eccentric anomaly (Newton's method).
pub fn eccentric_anomaly(mean_anomaly: f32, e: f32) -> f32 {
    let m = mean_anomaly.rem_euclid(TAU);
    let mut ea = if e < 0.8 { m } else { PI };
    for _ in 0..12 {
        let f = ea - e * ea.sin() - m;
        ea -= f / (1.0 - e * ea.cos());
    }
    ea
}

/// True anomaly (angle from perihelion) at a mean anomaly.
pub fn true_anomaly(mean_anomaly: f32, e: f32) -> f32 {
    let ea = eccentric_anomaly(mean_anomaly, e);
    2.0 * ((1.0 + e).sqrt() * (ea * 0.5).sin()).atan2((1.0 - e).sqrt() * (ea * 0.5).cos())
}

/// Mean anomaly (share of the year since perihelion, as an angle) at a true anomaly.
pub fn mean_anomaly(true_anomaly: f32, e: f32) -> f32 {
    let ea = 2.0 * ((1.0 - e).sqrt() * (true_anomaly * 0.5).sin()).atan2((1.0 + e).sqrt() * (true_anomaly * 0.5).cos());
    (ea - e * ea.sin()).rem_euclid(TAU)
}

/// The star's flux at a true anomaly relative to its flux at the mean distance a: (a / r)^2.
pub fn flux_factor(true_anomaly: f32, e: f32) -> f32 {
    let k = (1.0 + e * true_anomaly.cos()) / (1.0 - e * e);
    k * k
}

/// Solar declination (radians) at a solar longitude for an obliquity: sin d = sin e sin L.
pub fn declination(obliquity: f32, solar_longitude: f32) -> f32 {
    (obliquity.sin() * solar_longitude.sin()).clamp(-1.0, 1.0).asin()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Orbit {
    pub semi_major_au: f32,
    pub eccentricity: f32,
    /// Solar longitude of perihelion (degrees, from the northern spring equinox): the season in
    /// which the world passes closest to its star (Earth: 283, early January).
    pub perihelion_lon_deg: f32,
    /// Sidereal year in days of 24 hours.
    pub period_days: f32,
}

impl Orbit {
    pub fn earth() -> Self {
        Orbit { semi_major_au: 1.0, eccentricity: 0.0167, perihelion_lon_deg: 283.0, period_days: EARTH_YEAR_DAYS }
    }

    /// Kepler's third law: P^2 = a^3 / M (years, AU, solar masses).
    pub fn new(semi_major_au: f32, eccentricity: f32, perihelion_lon_deg: f32, star_mass_msun: f32) -> Self {
        let years = (semi_major_au.powi(3) / star_mass_msun).sqrt();
        Orbit { semi_major_au, eccentricity, perihelion_lon_deg, period_days: years * EARTH_YEAR_DAYS }
    }

    pub fn years(&self) -> f32 { self.period_days / EARTH_YEAR_DAYS }

    /// Year-mean flux relative to the flux at the mean distance: 1 / sqrt(1 - e^2).
    pub fn mean_flux_factor(&self) -> f32 { 1.0 / (1.0 - self.eccentricity * self.eccentricity).sqrt() }

    /// Closest and farthest distance (AU).
    pub fn peri_apo_au(&self) -> (f32, f32) {
        (self.semi_major_au * (1.0 - self.eccentricity), self.semi_major_au * (1.0 + self.eccentricity))
    }

    /// The four seasons' share of the year (northern spring, summer, autumn, winter: the
    /// quarters of solar longitude centred on the equinoxes and solstices). Kepler's second law
    /// makes the seasons near aphelion long and those near perihelion short.
    pub fn season_shares(&self) -> [f32; 4] {
        let e = self.eccentricity;
        let peri = self.perihelion_lon_deg.to_radians();
        let mut out = [0.25f32; 4];
        if e <= 0.0 { return out; }
        for (k, share) in out.iter_mut().enumerate() {
            let centre = k as f32 * PI * 0.5;
            let m0 = mean_anomaly(centre - PI * 0.25 - peri, e);
            let m1 = mean_anomaly(centre + PI * 0.25 - peri, e);
            *share = (m1 - m0).rem_euclid(TAU) / TAU;
        }
        out
    }

    /// The flux factor (a / r)^2 at each season's middle.
    pub fn season_flux(&self) -> [f32; 4] {
        let peri = self.perihelion_lon_deg.to_radians();
        [0, 1, 2, 3].map(|k| flux_factor(k as f32 * PI * 0.5 - peri, self.eccentricity))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Spin {
    /// Rotation relative to the stars (hours); for a locked world, the year.
    pub sidereal_hours: f32,
    /// Spins against its orbit (the star rises in the west).
    pub retrograde: bool,
    pub obliquity_deg: f32,
    /// One face always to the star.
    pub locked: bool,
    /// Longitude (degrees east of the map's left edge) under the star, for a locked world.
    pub substellar_lon_deg: f32,
    /// The day it was born with, before tides slowed it (hours).
    pub primordial_hours: f32,
    /// Caught in a 3:2 spin-orbit resonance (as Mercury): three turns every two years, so a
    /// day lasts two years. `substellar_lon_deg` is then a hot longitude, under the star at
    /// every perihelion.
    pub resonance: bool,
}

impl Spin {
    pub fn earth() -> Self {
        Spin { sidereal_hours: EARTH_SIDEREAL_HOURS, retrograde: false, obliquity_deg: 23.44, locked: false, substellar_lon_deg: 0.0, primordial_hours: 6.0, resonance: false }
    }

    /// Angular speed relative to Earth's (sets the Coriolis force).
    pub fn rotation_factor(&self) -> f32 { EARTH_SIDEREAL_HOURS / self.sidereal_hours }

    /// Sunrise to sunrise (hours); None for a locked world (the star never moves).
    pub fn solar_day_hours(&self, orbit: &Orbit) -> Option<f32> {
        if self.locked { return None; }
        let year_h = orbit.period_days * 24.0;
        let w = 1.0 / self.sidereal_hours;
        let n = 1.0 / year_h;
        let rate = if self.retrograde { w + n } else { w - n };
        if rate.abs() < 1e-9 { None } else { Some(1.0 / rate.abs()) }
    }

    /// Local days in a year.
    pub fn days_per_year(&self, orbit: &Orbit) -> Option<f32> {
        self.solar_day_hours(orbit).map(|d| orbit.period_days * 24.0 / d)
    }
}

/// Time (Gyr) for the star's tides to despin a planet from `spin_hours` to synchronous rotation
/// (Gladman et al. 1996): w a^6 I Q / (3 G M*^2 k2 R^5), with I = 0.33 M R^2, Q = 100, k2 = 0.3.
pub fn tidal_lock_gyr(spin_hours: f32, semi_major_au: f32, star_mass_msun: f32, planet_mass_earth: f32, planet_radius_earth: f32) -> f32 {
    const G: f64 = 6.674e-11;
    const AU: f64 = 1.496e11;
    const MSUN: f64 = 1.989e30;
    const MEARTH: f64 = 5.972e24;
    const REARTH: f64 = 6.371e6;
    let w = TAU as f64 / (spin_hours as f64 * 3600.0);
    let a = semi_major_au as f64 * AU;
    let m = planet_mass_earth as f64 * MEARTH;
    let r = planet_radius_earth as f64 * REARTH;
    let ms = star_mass_msun as f64 * MSUN;
    let inertia = 0.33 * m * r * r;
    let (q, k2) = (100.0, 0.3);
    let seconds = w * a.powi(6) * inertia * q / (3.0 * G * ms * ms * k2 * r.powi(5));
    (seconds / 3.156e16) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kepler_round_trips() {
        for &e in &[0.0f32, 0.1, 0.3, 0.6] {
            for i in 0..12 {
                let m = i as f32 * 0.5;
                let nu = true_anomaly(m, e);
                assert!((mean_anomaly(nu, e) - m.rem_euclid(TAU)).abs() < 1e-3, "e {e} m {m}");
            }
        }
    }

    #[test]
    fn eccentric_seasons_are_unequal() {
        let o = Orbit { semi_major_au: 1.0, eccentricity: 0.2, perihelion_lon_deg: 270.0, period_days: 365.0 };
        let s = o.season_shares();
        assert!((s.iter().sum::<f32>() - 1.0).abs() < 1e-4);
        // Perihelion in northern winter: that winter is short and the summer long.
        assert!(s[3] < 0.22 && s[1] > 0.28, "{s:?}");
        let f = o.season_flux();
        assert!(f[3] > 1.3 && f[1] < 0.8, "{f:?}");
        // The Earth's are nearly equal: winter (perihelion) a few days shorter than summer.
        let e = Orbit::earth().season_shares();
        assert!(e[1] > e[3] && e[1] - e[3] < 0.02, "{e:?}");
    }

    #[test]
    fn the_earths_day_and_year() {
        let (o, s) = (Orbit::earth(), Spin::earth());
        let day = s.solar_day_hours(&o).unwrap();
        assert!((day - 24.0).abs() < 0.01, "solar day {day}");
        assert!((s.days_per_year(&o).unwrap() - 365.25).abs() < 0.1);
        let mut r = s.clone();
        r.retrograde = true;
        assert!(r.solar_day_hours(&o).unwrap() < 24.0);
    }

    #[test]
    fn red_dwarf_worlds_lock_and_suns_do_not() {
        assert!(tidal_lock_gyr(12.0, 0.15, 0.3, 1.0, 1.0) < 0.5, "an M dwarf's habitable world locks fast");
        assert!(tidal_lock_gyr(12.0, 1.0, 1.0, 1.0, 1.0) > 50.0, "the Earth never locks to the Sun");
    }
}
