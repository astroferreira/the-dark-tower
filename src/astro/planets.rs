//! The other planets of the system, and the world's moons.
//!
//! Orbits are laid out from the world outward and inward with the spacing of real systems
//! (each orbit 1.4-2.2 times the last, like Kepler's compact multiples and the Titius-Bode
//! run of our own). Inside the snow line planets are rock; beyond it ices let cores grow into
//! ice giants and, if the disk was rich in metals, gas giants: the chance of a giant rises
//! tenfold over a dex of metallicity (Fischer & Valenti 2005, Johnson et al. 2010) and with
//! the star's mass. Pairs too close to be stable for billions of years (closer than ~9 mutual
//! Hill radii, 12 with a giant) lose the lighter planet, never the world itself.

use rand::Rng;
use serde::{Deserialize, Serialize};

use super::names::Namer;
use super::star::{gauss, Star};

/// Earth masses in a Jupiter mass, and in a solar mass.
pub const JUPITER_EARTHS: f32 = 317.8;
const SUN_EARTHS: f32 = 332_946.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanetKind {
    /// Molten rock under a too-near star.
    Lava,
    /// A hothouse wrapped in cloud (Venus).
    Hothouse,
    /// Bare rock with little or no air (Mercury, the Moon).
    Barren,
    /// A cold desert with thin air (Mars).
    ColdDesert,
    /// A world of ice beyond the snow line.
    Ice,
    /// A rocky world larger than this one.
    SuperEarth,
    /// A small world wrapped in deep hydrogen air.
    MiniNeptune,
    IceGiant,
    GasGiant,
    /// The world of the map.
    Home,
}

impl PlanetKind {
    pub fn word(self) -> &'static str {
        match self {
            Self::Lava => "lava world",
            Self::Hothouse => "hothouse world",
            Self::Barren => "barren rock",
            Self::ColdDesert => "cold desert world",
            Self::Ice => "ice world",
            Self::SuperEarth => "super-earth",
            Self::MiniNeptune => "mini-neptune",
            Self::IceGiant => "ice giant",
            Self::GasGiant => "gas giant",
            Self::Home => "this world",
        }
    }

    pub fn is_giant(self) -> bool { matches!(self, Self::GasGiant | Self::IceGiant) }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Planet {
    pub name: String,
    pub kind: PlanetKind,
    pub mass_earth: f32,
    pub radius_earth: f32,
    pub semi_major_au: f32,
    pub eccentricity: f32,
    pub period_days: f32,
    /// Blackbody equilibrium temperature (K) with its albedo.
    pub eq_temp_k: f32,
    pub rings: bool,
    pub moons: u32,
}

/// A belt of asteroids or comets.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Belt {
    pub name: String,
    pub inner_au: f32,
    pub outer_au: f32,
    pub icy: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Moon {
    pub name: String,
    /// Mass in the Earth's Moon's masses.
    pub mass_moons: f32,
    pub radius_km: f32,
    /// Distance from the world in its radii (the Moon: 60.3).
    pub distance_radii: f32,
    /// Orbit around the world (days of 24 hours).
    pub period_days: f32,
}

impl Moon {
    pub fn the_moon() -> Self {
        Moon { name: "the Moon".into(), mass_moons: 1.0, radius_km: 1737.0, distance_radii: 60.3, period_days: 27.32 }
    }

    /// Angular diameter in the sky (degrees).
    pub fn angular_deg(&self, world_radius_km: f32) -> f32 {
        (2.0 * self.radius_km / (self.distance_radii * world_radius_km)).to_degrees()
    }

    /// Its tide relative to the Moon's on Earth: mass over distance cubed.
    pub fn tide(&self) -> f32 { self.mass_moons * (60.3 / self.distance_radii).powi(3) }
}

/// Mass-radius relation (Chen & Kipping 2017): rocky to 2 Earth masses, Neptunian to 130,
/// then giants all about Jupiter's size.
pub fn radius_for_mass(mass_earth: f32) -> f32 {
    if mass_earth < 2.0 { mass_earth.powf(0.279) }
    else if mass_earth < 130.0 { 1.214 * (mass_earth / 2.0).powf(0.59) }
    else { 11.2 * (mass_earth / 318.0).powf(-0.04) }
}

/// Equilibrium temperature (K) at a distance from the star with a Bond albedo.
pub fn equilibrium_k(star: &Star, au: f32, albedo: f32) -> f32 {
    278.6 * star.luminosity_lsun.powf(0.25) / au.sqrt() * (1.0 - albedo).powf(0.25)
}

/// Mutual Hill radius of two orbits (AU).
fn mutual_hill(a1: f32, m1: f32, a2: f32, m2: f32, star_mass_msun: f32) -> f32 {
    ((m1 + m2) / (3.0 * star_mass_msun * SUN_EARTHS)).powf(1.0 / 3.0) * (a1 + a2) * 0.5
}

/// Lay out the system around the world at `home_au` (mass `home_mass` Earths). Returns the
/// planets in order from the star (the world among them, kind `Home`) and belts.
///
/// A companion star bounds the disk: around a close pair no planet survives inside the
/// stability limit (`inner_min`), and a wide companion truncates the disk (`outer_max`).
pub fn generate(star: &Star, home_name: &str, home_au: f32, home_mass: f32, home_period_days: f32, inner_min: f32, outer_max: f32, namer: &mut Namer, rng: &mut impl Rng) -> (Vec<Planet>, Vec<Belt>) {
    let snow = star.snow_line_au();
    let inner_edge = (0.03 * star.luminosity_lsun.sqrt()).max(0.006).max(inner_min);
    let outer_edge = (45.0 * star.mass_msun.powf(0.8)).min(outer_max).max(home_au * 1.05);
    let metal = 10f32.powf(star.metallicity);
    // Rich disks build more and bigger planets.
    let p_giant = (0.3 * star.mass_msun.powf(0.8) * metal.powf(1.2)).clamp(0.02, 0.85);
    let p_ice_giant = 0.45;
    let occupied = (0.62 + 0.15 * star.metallicity).clamp(0.4, 0.85);

    let mut slots: Vec<f32> = Vec::new();
    let mut a = home_au;
    loop {
        a /= rng.gen_range(1.4..2.2);
        if a < inner_edge { break; }
        if rng.gen_bool(occupied as f64) { slots.push(a); }
    }
    let mut a = home_au;
    loop {
        // Beyond the snow line the spacing widens (giants clear wide gaps).
        let r = if a > snow { rng.gen_range(1.5..2.4) } else { rng.gen_range(1.4..2.2) };
        a *= r;
        if a > outer_edge { break; }
        if rng.gen_bool(occupied.max(0.7) as f64) { slots.push(a); }
    }
    slots.sort_by(|x, y| x.partial_cmp(y).unwrap());

    let mut bodies: Vec<(f32, f32, PlanetKind)> = Vec::new();
    for &a in &slots {
        let (mass, kind) = if a < snow {
            // Rock: log-uniform 0.05-2.5 Earths, bigger in rich disks; some grow past two.
            let m = (rng.gen_range(0.05f32.ln()..2.5f32.ln())).exp() * metal.powf(0.3);
            if m > 2.0 && rng.gen_bool((0.3 * metal).min(0.8) as f64) { (m * rng.gen_range(1.5..4.0), PlanetKind::MiniNeptune) } else { (m, PlanetKind::Barren) }
        } else if rng.gen_bool(p_giant as f64) {
            let mj = (rng.gen_range(0.15f32.ln()..3.0f32.ln())).exp() * metal.powf(0.5);
            (mj * JUPITER_EARTHS, PlanetKind::GasGiant)
        } else if rng.gen_bool(p_ice_giant) {
            (rng.gen_range(8.0..25.0), PlanetKind::IceGiant)
        } else {
            ((rng.gen_range(0.01f32.ln()..1.5f32.ln())).exp(), PlanetKind::Ice)
        };
        bodies.push((a, mass, kind));
    }
    bodies.push((home_au, home_mass, PlanetKind::Home));
    bodies.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap());

    // Stability: drop the lighter of a pair too close together (never the world).
    loop {
        let mut drop: Option<usize> = None;
        for i in 0..bodies.len().saturating_sub(1) {
            let (a1, m1, k1) = bodies[i];
            let (a2, m2, k2) = bodies[i + 1];
            let need = if k1.is_giant() || k2.is_giant() { 12.0 } else { 9.0 };
            if (a2 - a1) < need * mutual_hill(a1, m1, a2, m2, star.mass_msun) {
                drop = Some(if k1 == PlanetKind::Home { i + 1 } else if k2 == PlanetKind::Home { i } else if m1 < m2 { i } else { i + 1 });
                break;
            }
        }
        match drop { Some(i) => { bodies.remove(i); } None => break }
    }

    let mut planets = Vec::new();
    for (a, mass, kind) in bodies {
        let e = (0.03 + 0.04 * gauss(rng).abs()).min(0.25);
        let radius = if kind == PlanetKind::Home { 1.0 } else { radius_for_mass(mass) };
        let albedo = match kind { PlanetKind::GasGiant => 0.34, PlanetKind::IceGiant => 0.3, PlanetKind::Ice => 0.6, PlanetKind::MiniNeptune => 0.35, _ => 0.15 };
        let eq = equilibrium_k(star, a, albedo);
        // Rocky worlds by how warm they are and how large.
        let kind = match kind {
            PlanetKind::Barren if eq > 900.0 => PlanetKind::Lava,
            PlanetKind::Barren if mass > 1.6 => PlanetKind::SuperEarth,
            PlanetKind::Barren if eq > 290.0 && mass > 0.4 => PlanetKind::Hothouse,
            PlanetKind::Barren if eq < 240.0 && mass > 0.08 => PlanetKind::ColdDesert,
            k => k,
        };
        let rings = match kind { PlanetKind::GasGiant => rng.gen_bool(0.4), PlanetKind::IceGiant => rng.gen_bool(0.3), _ => false };
        let moons = match kind {
            PlanetKind::GasGiant => rng.gen_range(3..9),
            PlanetKind::IceGiant => rng.gen_range(2..7),
            PlanetKind::MiniNeptune | PlanetKind::SuperEarth => rng.gen_range(0..3),
            PlanetKind::Home => 0,
            _ => if rng.gen_bool(0.3) { rng.gen_range(1..3) } else { 0 },
        };
        let period_days = if kind == PlanetKind::Home { home_period_days } else { (a.powi(3) / star.mass_msun).sqrt() * super::orbit::EARTH_YEAR_DAYS };
        let name = if kind == PlanetKind::Home { home_name.to_string() } else { namer.next(rng) };
        planets.push(Planet { name, kind, mass_earth: mass, radius_earth: radius, semi_major_au: a, eccentricity: e, period_days, eq_temp_k: eq, rings, moons });
    }

    // Belts: rubble where a giant kept a planet from forming (inside the first giant, outside
    // the last rock), and a field of comets beyond the last giant.
    let mut belts = Vec::new();
    if let Some(g) = planets.iter().position(|p| p.kind == PlanetKind::GasGiant) {
        let inner_rock = planets[..g].iter().rev().find(|p| p.semi_major_au < snow).map(|p| p.semi_major_au);
        let ga = planets[g].semi_major_au;
        let lo = inner_rock.map(|r| r * 1.3).unwrap_or(ga * 0.35);
        let hi = ga * 0.7;
        if hi > lo * 1.2 {
            belts.push(Belt { name: format!("the Shards of {}", namer.next(rng)), inner_au: lo, outer_au: hi, icy: false });
        }
    }
    if let Some(last) = planets.iter().rev().find(|p| p.kind.is_giant()) {
        let a = last.semi_major_au;
        belts.push(Belt { name: format!("the Drift of {}", namer.next(rng)), inner_au: a * 1.4, outer_au: a * 2.2, icy: true });
    }
    (planets, belts)
}

/// The world's moons: a great moon from an early impact (one world in three, as the Moon is
/// thought to have formed), maybe small captured ones. A world locked to its star keeps none
/// large: its tides drag them down or let them go.
pub fn moons(locked: bool, world_mass: f32, namer: &mut Namer, rng: &mut impl Rng) -> Vec<Moon> {
    let mut out = Vec::new();
    let r_world_km = 6371.0;
    let period = |d_radii: f32| {
        let d = d_radii as f64 * r_world_km as f64 * 1000.0;
        let gm = 6.674e-11 * world_mass as f64 * 5.972e24;
        (std::f64::consts::TAU * (d.powi(3) / gm).sqrt() / 86400.0) as f32
    };
    if !locked && rng.gen_bool(0.38) {
        let mass = (rng.gen_range(0.3f32.ln()..2.2f32.ln())).exp();
        let radius_km = 1737.0 * mass.powf(1.0 / 3.0);
        let d = rng.gen_range(35.0..75.0);
        out.push(Moon { name: namer.next(rng), mass_moons: mass, radius_km, distance_radii: d, period_days: period(d) });
    }
    let small = if locked { if rng.gen_bool(0.3) { 1 } else { 0 } } else { rng.gen_range(0..3) };
    for _ in 0..small {
        let mass = (rng.gen_range(1e-6f32.ln()..0.02f32.ln())).exp();
        let radius_km = (1737.0 * mass.powf(1.0 / 3.0)).max(6.0);
        let d = if locked { rng.gen_range(3.0..12.0) } else { rng.gen_range(3.0..110.0) };
        out.push(Moon { name: namer.next(rng), mass_moons: mass, radius_km, distance_radii: d, period_days: period(d) });
    }
    out.sort_by(|a, b| a.distance_radii.partial_cmp(&b.distance_radii).unwrap());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;
    use crate::astro::star::{SpectralClass, SpectralType};

    #[test]
    fn metal_rich_systems_have_more_giants() {
        let count = |feh: f32| -> usize {
            (0..300u64).map(|s| {
                let mut rng = ChaCha8Rng::seed_from_u64(s);
                let star = Star::new("x".into(), SpectralType { class: SpectralClass::G, subclass: 4 }, 4.0, feh);
                let (inner, _) = star.hz_au();
                let (p, _) = generate(&star, "Home", inner * 1.1, 1.0, 365.0, 0.0, f32::MAX, &mut Namer::new(), &mut rng);
                p.iter().filter(|p| p.kind == PlanetKind::GasGiant).count()
            }).sum()
        };
        let (poor, rich) = (count(-0.5), count(0.35));
        assert!(rich > poor * 2, "giants: metal-poor {poor}, metal-rich {rich}");
    }

    #[test]
    fn systems_are_stable_and_hold_the_world() {
        for s in 0..200u64 {
            let mut rng = ChaCha8Rng::seed_from_u64(s);
            let star = Star::random(&mut rng, None, None, None, "x".into());
            let (inner, outer) = star.hz_au();
            let home = (inner * outer).sqrt();
            let (p, _) = generate(&star, "Home", home, 1.0, 365.0, 0.0, f32::MAX, &mut Namer::new(), &mut rng);
            assert_eq!(p.iter().filter(|p| p.kind == PlanetKind::Home).count(), 1);
            for w in p.windows(2) {
                assert!(w[1].semi_major_au > w[0].semi_major_au);
                let need = if w[0].kind.is_giant() || w[1].kind.is_giant() { 12.0 } else { 9.0 };
                let h = mutual_hill(w[0].semi_major_au, w[0].mass_earth, w[1].semi_major_au, w[1].mass_earth, star.mass_msun);
                assert!(w[1].semi_major_au - w[0].semi_major_au >= need * h * 0.999, "seed {s}: unstable pair");
            }
        }
    }
}
