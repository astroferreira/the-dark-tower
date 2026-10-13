//! The star: spectral type, mass, radius, luminosity and temperature on the main sequence, its
//! age and metallicity, its activity, and the habitable zone around it.
//!
//! Main-sequence dwarfs follow Pecaut & Mamajek's mean dwarf sequence (F0V-M6V: temperature,
//! radius, mass), with the luminosity from the radius and temperature (L = R^2 (T/5772)^4).
//! A star brightens as it ages (Gough 1981: 30% fainter at birth than at the Sun's present
//! fraction of its life). The habitable zone's edges are Kopparapu et al. (2014): the runaway
//! greenhouse inside, the maximum CO2 greenhouse outside, as stellar fluxes that depend on the
//! star's temperature (red light warms a planet more: its ice and water vapour absorb it).

use rand::Rng;
use serde::{Deserialize, Serialize};

/// The Sun's effective temperature (K).
pub const SUN_TEFF: f32 = 5772.0;
/// The Sun's main-sequence lifetime (Gyr) and its age now.
const SUN_LIFETIME_GYR: f32 = 10.0;
const SUN_AGE_GYR: f32 = 4.57;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SpectralClass { F, G, K, M }

impl SpectralClass {
    pub fn all() -> &'static [SpectralClass] { &[Self::F, Self::G, Self::K, Self::M] }

    fn base_index(self) -> f32 {
        match self { Self::F => 0.0, Self::G => 10.0, Self::K => 20.0, Self::M => 30.0 }
    }

    /// The highest subclass generated (M7 and later fall below the habitable-zone fits).
    pub fn max_subclass(self) -> u8 { if self == Self::M { 6 } else { 9 } }

    pub fn letter(self) -> char {
        match self { Self::F => 'F', Self::G => 'G', Self::K => 'K', Self::M => 'M' }
    }

    /// What the star looks like to the eye.
    pub fn colour_word(self) -> &'static str {
        match self { Self::F => "yellow-white", Self::G => "white-gold", Self::K => "orange", Self::M => "red" }
    }

    pub fn dwarf_word(self) -> &'static str {
        match self { Self::F => "yellow-white dwarf", Self::G => "yellow dwarf", Self::K => "orange dwarf", Self::M => "red dwarf" }
    }
}

/// A main-sequence spectral type, F0 to M6.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpectralType {
    pub class: SpectralClass,
    /// 0-9 (0 hottest).
    pub subclass: u8,
}

impl SpectralType {
    pub const SUN: SpectralType = SpectralType { class: SpectralClass::G, subclass: 2 };

    /// Position on the sequence: F0 = 0, G0 = 10, K0 = 20, M0 = 30.
    pub fn index(&self) -> f32 { self.class.base_index() + self.subclass as f32 }

    /// "K3", "g2", "M": a class with an optional subclass (None for the subclass when absent).
    pub fn parse(s: &str) -> Option<(SpectralClass, Option<u8>)> {
        let s = s.trim();
        let mut chars = s.chars();
        let class = match chars.next()?.to_ascii_uppercase() {
            'F' => SpectralClass::F, 'G' => SpectralClass::G, 'K' => SpectralClass::K, 'M' => SpectralClass::M,
            _ => return None,
        };
        let rest: String = chars.collect();
        let rest = rest.trim_end_matches(['V', 'v']);
        if rest.is_empty() { return Some((class, None)); }
        let sub: u8 = rest.parse().ok()?;
        if sub > class.max_subclass() { return None; }
        Some((class, Some(sub)))
    }

    /// "K3V".
    pub fn label(&self) -> String { format!("{}{}V", self.class.letter(), self.subclass) }

    /// The dwarf type of a mass (F0 to M6; lighter stars are M6).
    pub fn for_mass(mass_msun: f32) -> Self {
        let m = mass_msun.clamp(DWARFS[DWARFS.len() - 1].3, DWARFS[0].3);
        let k = DWARFS.windows(2).position(|w| m >= w[1].3).unwrap_or(DWARFS.len() - 2);
        let (a, b) = (DWARFS[k], DWARFS[k + 1]);
        let t = (m.ln() - a.3.ln()) / (b.3.ln() - a.3.ln());
        let index = (a.0 + (b.0 - a.0) * t).round().clamp(0.0, 36.0) as u8;
        let class = match index { 0..=9 => SpectralClass::F, 10..=19 => SpectralClass::G, 20..=29 => SpectralClass::K, _ => SpectralClass::M };
        SpectralType { class, subclass: index - class.base_index() as u8 }
    }
}

/// Pecaut & Mamajek's dwarf sequence: (index, Teff K, radius Rsun, mass Msun).
const DWARFS: [(f32, f32, f32, f32); 19] = [
    (0.0, 7220.0, 1.728, 1.61),
    (2.0, 6810.0, 1.609, 1.46),
    (5.0, 6510.0, 1.473, 1.33),
    (8.0, 6170.0, 1.240, 1.18),
    (10.0, 5920.0, 1.100, 1.06),
    (12.0, 5772.0, 1.000, 1.00),
    (15.0, 5660.0, 0.977, 0.98),
    (18.0, 5490.0, 0.914, 0.94),
    (20.0, 5280.0, 0.874, 0.88),
    (22.0, 5040.0, 0.799, 0.82),
    (25.0, 4440.0, 0.704, 0.69),
    (27.0, 4050.0, 0.646, 0.63),
    (30.0, 3850.0, 0.588, 0.57),
    (31.0, 3660.0, 0.501, 0.50),
    (32.0, 3560.0, 0.446, 0.44),
    (33.0, 3430.0, 0.361, 0.37),
    (34.0, 3210.0, 0.274, 0.23),
    (35.0, 3060.0, 0.196, 0.162),
    (36.0, 2810.0, 0.137, 0.102),
];

/// Teff, radius and mass of a dwarf at a sequence index (temperature linear, radius and mass
/// log-linear between the table's rows).
fn dwarf_at(index: f32) -> (f32, f32, f32) {
    let i = index.clamp(DWARFS[0].0, DWARFS[DWARFS.len() - 1].0);
    let k = DWARFS.windows(2).position(|w| i <= w[1].0).unwrap_or(DWARFS.len() - 2);
    let (a, b) = (DWARFS[k], DWARFS[k + 1]);
    let t = (i - a.0) / (b.0 - a.0);
    let lerp = |x: f32, y: f32| x + (y - x) * t;
    let loglerp = |x: f32, y: f32| (x.ln() + (y.ln() - x.ln()) * t).exp();
    (lerp(a.1, b.1), loglerp(a.2, b.2), loglerp(a.3, b.3))
}

/// The star of a system.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Star {
    pub name: String,
    pub spectral: SpectralType,
    pub mass_msun: f32,
    pub radius_rsun: f32,
    pub luminosity_lsun: f32,
    pub teff_k: f32,
    pub age_gyr: f32,
    pub lifetime_gyr: f32,
    /// [Fe/H]: log10 of the star's iron abundance relative to the Sun's.
    pub metallicity: f32,
    /// X-ray and far-UV output as a share of its light, relative to the Sun's today: flares,
    /// coronal storms, the wind that strips air.
    pub activity: f32,
}

impl Star {
    /// The Sun today.
    pub fn sun() -> Self {
        Star {
            name: "the Sun".into(),
            spectral: SpectralType::SUN,
            mass_msun: 1.0,
            radius_rsun: 1.0,
            luminosity_lsun: 1.0,
            teff_k: SUN_TEFF,
            age_gyr: SUN_AGE_GYR,
            lifetime_gyr: SUN_LIFETIME_GYR,
            metallicity: 0.0,
            activity: 1.0,
        }
    }

    /// A main-sequence star of a type, an age (Gyr) and a metallicity.
    pub fn new(name: String, spectral: SpectralType, age_gyr: f32, metallicity: f32) -> Self {
        let (teff0, radius, mass) = dwarf_at(spectral.index());
        let lifetime = main_sequence_lifetime(mass, metallicity);
        let l_table = radius * radius * (teff0 / SUN_TEFF).powi(4);
        // The table is a field star's: about the Sun's fraction of its life, or the age of the
        // field for long-lived stars. Brighten or dim from there (Gough's law).
        let tau_ref = (SUN_AGE_GYR / lifetime).min(SUN_AGE_GYR / SUN_LIFETIME_GYR);
        let tau = (age_gyr / lifetime).min(0.95);
        let gough = |t: f32| 1.0 / (1.0 + 0.4 * (1.0 - t / (SUN_AGE_GYR / SUN_LIFETIME_GYR)));
        let luminosity = l_table * gough(tau) / gough(tau_ref);
        // Metal-rich stars are a little cooler and fainter at the same mass (more opacity).
        let luminosity = luminosity * 10f32.powf(-0.15 * metallicity);
        let teff = SUN_TEFF * (luminosity / (radius * radius)).powf(0.25);
        Star {
            name,
            spectral,
            mass_msun: mass,
            radius_rsun: radius,
            luminosity_lsun: luminosity,
            teff_k: teff,
            age_gyr,
            lifetime_gyr: lifetime,
            metallicity,
            activity: activity(mass, age_gyr),
        }
    }

    /// Draw a star: the class (weights F 13%, G 34%, K 35%, M 18%: dimmer stars are common,
    /// though fewer than in the galaxy, where three in four are red dwarfs) unless given, a
    /// subclass, an age at which life has had time (at least 0.8 Gyr, at most 85% of its life
    /// or 11 Gyr) and the solar neighbourhood's metallicity (mean -0.05, spread 0.22 dex).
    pub fn random(rng: &mut impl Rng, class: Option<SpectralClass>, subclass: Option<u8>, metallicity: Option<f32>, name: String) -> Self {
        let class = class.unwrap_or_else(|| {
            let r: f32 = rng.gen();
            if r < 0.13 { SpectralClass::F } else if r < 0.47 { SpectralClass::G } else if r < 0.82 { SpectralClass::K } else { SpectralClass::M }
        });
        let subclass = subclass.unwrap_or_else(|| rng.gen_range(0..=class.max_subclass()));
        let spectral = SpectralType { class, subclass };
        let feh = metallicity.unwrap_or_else(|| (-0.05 + 0.22 * gauss(rng)).clamp(-0.8, 0.5));
        let (_, _, mass) = dwarf_at(spectral.index());
        let lifetime = main_sequence_lifetime(mass, feh);
        let oldest = (0.85 * lifetime).min(11.0).max(1.0);
        let age = rng.gen_range(0.8..=oldest);
        Star::new(name, spectral, age, feh)
    }

    /// Flux (relative to the Earth's from the Sun) at the habitable zone's inner edge (runaway
    /// greenhouse) and outer edge (maximum greenhouse), Kopparapu et al. 2014 for an Earth mass.
    pub fn hz_flux(&self) -> (f32, f32) { (hz_flux(self.teff_k, &RUNAWAY), hz_flux(self.teff_k, &MAX_GREENHOUSE)) }

    /// Distances (AU) of the habitable zone's edges.
    pub fn hz_au(&self) -> (f32, f32) {
        let (inner, outer) = self.hz_flux();
        ((self.luminosity_lsun / inner).sqrt(), (self.luminosity_lsun / outer).sqrt())
    }

    /// Where water ice condensed in the planet-forming disk (AU; Hayashi's 2.7 AU for the Sun,
    /// scaled by the square root of the star's light).
    pub fn snow_line_au(&self) -> f32 { 2.7 * self.luminosity_lsun.sqrt() }

    /// The star's colour as sRGB (a blackbody's, Helland's fit).
    pub fn rgb(&self) -> [u8; 3] { blackbody_rgb(self.teff_k) }

    /// The colour a painter would give it (the blackbody's hue, saturated so an orange dwarf
    /// reads orange in ink on parchment, not pale peach).
    pub fn paint_rgb(&self) -> [f32; 3] {
        const STOPS: [(f32, [f32; 3]); 5] = [(2800.0, [196.0, 72.0, 52.0]), (3800.0, [226.0, 112.0, 64.0]), (4800.0, [246.0, 168.0, 82.0]), (5800.0, [252.0, 222.0, 130.0]), (7200.0, [244.0, 244.0, 236.0])];
        let t = self.teff_k.clamp(STOPS[0].0, STOPS[4].0);
        let k = STOPS.windows(2).position(|w| t <= w[1].0).unwrap_or(3);
        let (a, b) = (STOPS[k], STOPS[k + 1]);
        let f = (t - a.0) / (b.0 - a.0);
        [0, 1, 2].map(|i| a.1[i] + (b.1[i] - a.1[i]) * f)
    }

    /// Two stars seen from far enough to be one host: summed light and mass, the light-weighted
    /// temperature (what the planet's albedo sees), the brighter one's activity.
    pub fn pair(a: &Star, b: &Star) -> Star {
        let l = a.luminosity_lsun + b.luminosity_lsun;
        let teff = (a.luminosity_lsun * a.teff_k + b.luminosity_lsun * b.teff_k) / l;
        Star {
            name: format!("{} and {}", a.name, b.name),
            spectral: a.spectral,
            mass_msun: a.mass_msun + b.mass_msun,
            radius_rsun: (l / (teff / SUN_TEFF).powi(4)).sqrt(),
            luminosity_lsun: l,
            teff_k: teff,
            age_gyr: a.age_gyr,
            lifetime_gyr: a.lifetime_gyr.min(b.lifetime_gyr),
            metallicity: a.metallicity,
            activity: a.activity.max(b.activity),
        }
    }

    /// Share of the star's light that falls in the visible band (400-700 nm), relative to the
    /// Sun's: how bright its day looks to the eye for the same warmth.
    pub fn visible_share(&self) -> f32 { visible_fraction(self.teff_k) / visible_fraction(SUN_TEFF) }
}

/// Main-sequence lifetime (Gyr): 10 Gyr x M^-2.5, a little longer for metal-rich stars.
pub fn main_sequence_lifetime(mass_msun: f32, metallicity: f32) -> f32 {
    SUN_LIFETIME_GYR * mass_msun.powf(-2.5) * 10f32.powf(0.1 * metallicity)
}

/// High-energy output relative to the Sun's share today. Young and low-mass stars stay active
/// longer (they spin down slowly): saturated at ~2000x, falling as age^-1.5, stronger for
/// lower mass (M^-3).
pub fn activity(mass_msun: f32, age_gyr: f32) -> f32 {
    ((age_gyr / SUN_AGE_GYR).max(0.02).powf(-1.5) * mass_msun.max(0.08).powf(-3.0)).min(2000.0)
}

/// Kopparapu et al. 2014 coefficients: S = S_sun + a T + b T^2 + c T^3 + d T^4, T = Teff - 5780.
const RUNAWAY: [f32; 5] = [1.107, 1.332e-4, 1.580e-8, -8.308e-12, -1.931e-15];
const MAX_GREENHOUSE: [f32; 5] = [0.356, 6.171e-5, 1.698e-9, -3.198e-12, -5.575e-16];

fn hz_flux(teff: f32, c: &[f32; 5]) -> f32 {
    let t = (teff.clamp(2600.0, 7200.0) - 5780.0) as f64;
    (c[0] as f64 + c[1] as f64 * t + c[2] as f64 * t * t + c[3] as f64 * t.powi(3) + c[4] as f64 * t.powi(4)) as f32
}

/// The flux at the inner edge relative to the Sun's: below 1 for red stars (their light is
/// absorbed more readily, so the same flux warms more), above 1 for blue-white ones.
pub fn hz_inner_relative(teff: f32) -> f32 { hz_flux(teff, &RUNAWAY) / hz_flux(5780.0, &RUNAWAY) }

/// A standard normal draw (Box-Muller).
pub(crate) fn gauss(rng: &mut impl Rng) -> f32 {
    let u: f32 = rng.gen_range(1e-6..1.0);
    let v: f32 = rng.gen();
    (-2.0 * u.ln()).sqrt() * (std::f32::consts::TAU * v).cos()
}

/// Share of a blackbody's power between 400 and 700 nm.
fn visible_fraction(teff: f32) -> f32 {
    let (h, c, k) = (6.626e-34f64, 2.998e8f64, 1.381e-23f64);
    let t = teff as f64;
    let planck = |l: f64| 1.0 / (l.powi(5) * ((h * c / (l * k * t)).exp() - 1.0));
    let (mut vis, mut all) = (0.0, 0.0);
    for i in 0..600 {
        let l = 100e-9 + i as f64 * 10e-9; // 100 nm .. 6 um
        let b = planck(l);
        all += b;
        if (400e-9..700e-9).contains(&l) { vis += b; }
    }
    (vis / all) as f32
}

/// sRGB of a blackbody (Tanner Helland's fit, good from 1000 to 40000 K).
pub fn blackbody_rgb(teff: f32) -> [u8; 3] {
    let t = teff / 100.0;
    let r = if t <= 66.0 { 255.0 } else { 329.7 * (t - 60.0).powf(-0.1332) };
    let g = if t <= 66.0 { 99.47 * t.ln() - 161.12 } else { 288.12 * (t - 60.0).powf(-0.0755) };
    let b = if t >= 66.0 { 255.0 } else if t <= 19.0 { 0.0 } else { 138.52 * (t - 10.0).ln() - 305.04 };
    [r.clamp(0.0, 255.0) as u8, g.clamp(0.0, 255.0) as u8, b.clamp(0.0, 255.0) as u8]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_g2_dwarf_of_the_suns_age_is_the_sun() {
        let s = Star::new("x".into(), SpectralType::SUN, 4.57, 0.0);
        assert!((s.luminosity_lsun - 1.0).abs() < 0.02, "L = {}", s.luminosity_lsun);
        assert!((s.teff_k - 5772.0).abs() < 10.0, "T = {}", s.teff_k);
        let (inner, outer) = Star::sun().hz_au();
        assert!((0.93..0.97).contains(&inner) && (1.6..1.75).contains(&outer), "Sun's HZ {inner}-{outer} AU");
    }

    #[test]
    fn red_dwarfs_are_dim_and_their_zone_is_close() {
        let m = Star::new("x".into(), SpectralType { class: SpectralClass::M, subclass: 4 }, 5.0, 0.0);
        assert!(m.luminosity_lsun < 0.02 && m.teff_k < 3400.0);
        let (inner, outer) = m.hz_au();
        assert!(inner < 0.1 && outer < 0.25, "M4 HZ {inner}-{outer} AU");
        // Red light warms more: the inner edge sits at a lower flux than the Sun's.
        assert!(m.hz_flux().0 < 0.95);
        assert!(m.activity > 10.0, "M4 activity {}", m.activity);
    }

    #[test]
    fn stars_brighten_as_they_age() {
        let young = Star::new("x".into(), SpectralType::SUN, 1.0, 0.0);
        let old = Star::new("x".into(), SpectralType::SUN, 8.0, 0.0);
        assert!(young.luminosity_lsun < 0.85 && old.luminosity_lsun > 1.2, "{} {}", young.luminosity_lsun, old.luminosity_lsun);
    }

    #[test]
    fn types_follow_mass() {
        assert_eq!(SpectralType::for_mass(1.0), SpectralType::SUN);
        assert_eq!(SpectralType::for_mass(0.57).class, SpectralClass::M);
        assert_eq!(SpectralType::for_mass(0.7).class, SpectralClass::K);
        assert_eq!(SpectralType::for_mass(0.05), SpectralType { class: SpectralClass::M, subclass: 6 });
        let a = Star::new("a".into(), SpectralType::SUN, 4.6, 0.0);
        let b = Star::new("b".into(), SpectralType { class: SpectralClass::M, subclass: 2 }, 4.6, 0.0);
        let p = Star::pair(&a, &b);
        assert!(p.luminosity_lsun > a.luminosity_lsun && p.teff_k < a.teff_k && p.teff_k > 5000.0);
    }

    #[test]
    fn spectral_types_parse() {
        assert_eq!(SpectralType::parse("K3"), Some((SpectralClass::K, Some(3))));
        assert_eq!(SpectralType::parse("g2v"), Some((SpectralClass::G, Some(2))));
        assert_eq!(SpectralType::parse("M"), Some((SpectralClass::M, None)));
        assert_eq!(SpectralType::parse("M9"), None);
        assert_eq!(SpectralType::parse("O5"), None);
    }

    #[test]
    fn colours_run_from_white_to_red() {
        let f = blackbody_rgb(7000.0);
        let m = blackbody_rgb(3000.0);
        assert!(f[2] > 200 && m[2] < 120 && m[0] == 255);
        let vis_m = visible_fraction(3000.0) / visible_fraction(5772.0);
        assert!(vis_m < 0.5, "an M dwarf's light is mostly infrared: {vis_m}");
    }
}
