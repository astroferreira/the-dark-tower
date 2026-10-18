//! The world's star system, generated before the terrain: a star of some spectral type, age and
//! metallicity; the world on its orbit in the star's habitable zone; its spin, day, seasons and
//! moons; its bulk (iron core, gravity, internal heat, water, magnetic field) and its air; and
//! the neighbour planets. The climate takes its forcing (`StarSystem::climate_config`), the
//! tectonics its water (land fraction) and internal heat (mantle hotspots).
//!
//! Everything follows from the seed in separate streams (star, orbit, spin, world, planets,
//! names), so pinning one thing (`--star K3`, `--hz 0.3`) leaves the others as they were.
//! `StarSystem::sol()` is the Sun, the Earth and its neighbours; its climate forcing is the
//! original model's, bit for bit, and the dev world keeps it.

pub mod air;
mod names;
pub mod orbit;
pub mod planets;
pub mod sky;
pub mod star;

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

pub use air::Air;
pub use orbit::{Orbit, Spin};
pub use planets::{Belt, Moon, Planet, PlanetKind};
pub use star::{SpectralClass, SpectralType, Star};

use star::gauss;

/// What was asked of the system (the rest comes from the seed). Carried in world codes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SystemSpec {
    /// The Sun and the Earth.
    pub sol: bool,
    pub class: Option<SpectralClass>,
    pub subclass: Option<u8>,
    /// Place in the habitable zone: 0 at the inner edge (runaway greenhouse), 1 at the outer
    /// (maximum greenhouse); a little beyond either is allowed.
    pub hz: Option<f32>,
    pub obliquity_deg: Option<f32>,
    /// Sunrise to sunrise (hours).
    pub day_hours: Option<f32>,
    pub eccentricity: Option<f32>,
    pub metallicity: Option<f32>,
    pub age_gyr: Option<f32>,
    /// Force one face to the star (or forbid it).
    pub locked: Option<bool>,
    /// Give the star a companion (or none).
    pub binary: Option<bool>,
}

impl SystemSpec {
    pub fn sol() -> Self { SystemSpec { sol: true, ..Default::default() } }

    /// "sol", "auto", "K", "K3" (a class or type for the star, the rest from the seed).
    pub fn parse_star(s: &str) -> Option<Self> {
        let s = s.trim();
        match s.to_ascii_lowercase().as_str() {
            "sol" | "sun" | "earth" => return Some(Self::sol()),
            "auto" | "random" | "" => return Some(Self::default()),
            _ => {}
        }
        let (class, subclass) = SpectralType::parse(s)?;
        Some(SystemSpec { class: Some(class), subclass, ..Default::default() })
    }

    /// The spec as it is written in a world code: "sol", "auto", "K3", with "+h250" (place in
    /// the zone, thousandths), "+o450" (tilt, tenths of a degree), "+d3000" (day, hundredths of
    /// an hour), "+e100" (eccentricity, thousandths), "+z-200" (metallicity, thousandths of a
    /// dex), "+a450" (age, hundredths of a Gyr), "+l1" / "+l0" (locked or not). No dots: the
    /// world code is dotted.
    pub fn code(&self) -> String {
        if self.sol { return "sol".into(); }
        let mut s = match (self.class, self.subclass) {
            (Some(c), Some(k)) => format!("{}{}", c.letter(), k),
            (Some(c), None) => c.letter().to_string(),
            _ => "auto".into(),
        };
        let mut part = |tag: char, v: Option<f32>, scale: f32| if let Some(v) = v { s.push_str(&format!("+{}{}", tag, (v * scale).round() as i64)); };
        part('h', self.hz, 1000.0);
        part('o', self.obliquity_deg, 10.0);
        part('d', self.day_hours, 100.0);
        part('e', self.eccentricity, 1000.0);
        part('z', self.metallicity, 1000.0);
        part('a', self.age_gyr, 100.0);
        if let Some(l) = self.locked { s.push_str(if l { "+l1" } else { "+l0" }); }
        if let Some(b) = self.binary { s.push_str(if b { "+b1" } else { "+b0" }); }
        s
    }

    pub fn from_code(code: &str) -> Option<Self> {
        let mut parts = code.split('+');
        let mut spec = Self::parse_star(parts.next()?)?;
        if spec.sol { return Some(spec); }
        for p in parts {
            let (tag, num) = p.split_at(1);
            let v: i64 = num.parse().ok()?;
            match tag {
                "h" => spec.hz = Some(v as f32 / 1000.0),
                "o" => spec.obliquity_deg = Some(v as f32 / 10.0),
                "d" => spec.day_hours = Some(v as f32 / 100.0),
                "e" => spec.eccentricity = Some(v as f32 / 1000.0),
                "z" => spec.metallicity = Some(v as f32 / 1000.0),
                "a" => spec.age_gyr = Some(v as f32 / 100.0),
                "l" => spec.locked = Some(v != 0),
                "b" => spec.binary = Some(v != 0),
                _ => return None,
            }
        }
        Some(spec)
    }
}

/// The world of the map: its orbit, spin, bulk and air.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HomeWorld {
    pub name: String,
    pub orbit: Orbit,
    pub spin: Spin,
    /// Place in the habitable zone (0 inner edge .. 1 outer edge).
    pub hz_position: f32,
    /// Year-mean starlight relative to the Earth's.
    pub flux_earth: f32,
    pub mass_earth: f32,
    /// Surface gravity (g); the radius is the Earth's (the map's scale).
    pub gravity: f32,
    /// Share of the mass in the iron core (Earth 0.32).
    pub core_fraction: f32,
    /// Heat from the interior relative to Earth's today (radioactivity and the heat of
    /// formation, falling with age): volcanism, plate speed, CO2 outgassing.
    pub internal_heat: f32,
    pub magnetic_field: f32,
    /// Water relative to Earth's oceans.
    pub water: f32,
    pub air: Air,
    /// The carbonate-silicate cycle's parameters (outgassing relative to Earth's, the
    /// weathering's CO2 exponent and its temperature sensitivity per C).
    pub outgassing: f32,
    pub weathering_beta: f32,
    pub weathering_k: f32,
    pub moons: Vec<Moon>,
    /// Time for the star's tides to lock the spin (Gyr).
    pub lock_gyr: f32,
}

/// What the climate made of it (filled after the climate is simulated).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ClimateNotes {
    /// Global mean surface temperature (C).
    pub mean_c: f32,
    pub co2_bar: f32,
    pub pressure_bar: f32,
    pub greenhouse_wm2: f32,
    pub hadley_deg: f32,
    /// Belts of pressure per hemisphere (Earth: 3 cells).
    pub cells: usize,
    /// Share of the planet's surface (land and sea) under ice all year.
    pub ice_share: f32,
    /// Year-mean surface temperature by band: equator (0-10 deg) and poles (80-90 deg); for a
    /// locked world, under the star and on the night side.
    pub equator_c: f32,
    pub pole_c: f32,
    /// Hottest and coldest season means on land (C), averaged over land.
    pub summer_land_c: f32,
    pub winter_land_c: f32,
    /// Coastal tiles the tides made salt marsh and mangrove (`biomes::apply_tidal_flats`).
    pub tidal_flat_tiles: u32,
}

/// A second star. Half of Sun-like stars have one; a close pair is circled by the world as one
/// host (two suns in its sky; no planet survives inside ~2.3-3 times their separation, Holman &
/// Wiegert 1999), a wide companion is a brilliant star by night and truncates the planet-forming
/// disk at about a third of its closest approach.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Companion {
    pub star: Star,
    pub separation_au: f32,
    pub eccentricity: f32,
    pub period_days: f32,
    /// The world circles both stars.
    pub circumbinary: bool,
}

impl Companion {
    /// Inside this distance (AU) from a close pair no orbit lasts.
    pub fn stability_limit_au(&self) -> f32 { (2.3 + 3.0 * self.eccentricity) * self.separation_au }

    /// Beyond this distance (AU) from the primary a wide companion strips the disk.
    pub fn truncation_au(&self) -> f32 { self.separation_au * (1.0 - self.eccentricity) / 3.0 }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StarSystem {
    pub spec: SystemSpec,
    pub star: Star,
    /// A second star (none for the Sun).
    pub companion: Option<Companion>,
    pub world: HomeWorld,
    /// Every planet from the star outward, the world among them (`PlanetKind::Home`).
    pub planets: Vec<Planet>,
    pub belts: Vec<Belt>,
    pub climate: Option<ClimateNotes>,
}

impl StarSystem {
    /// The Sun, the Earth and the planets.
    pub fn sol() -> Self {
        let star = Star::sun();
        let p = |name: &str, kind, mass: f32, radius: f32, a: f32, e: f32, eq: f32, rings: bool, moons: u32| Planet {
            name: name.into(), kind, mass_earth: mass, radius_earth: radius, semi_major_au: a, eccentricity: e,
            period_days: a.powf(1.5) * orbit::EARTH_YEAR_DAYS, eq_temp_k: eq, rings, moons,
        };
        let planets = vec![
            p("Mercury", PlanetKind::Barren, 0.055, 0.383, 0.387, 0.206, 440.0, false, 0),
            p("Venus", PlanetKind::Hothouse, 0.815, 0.949, 0.723, 0.007, 232.0, false, 0),
            p("Earth", PlanetKind::Home, 1.0, 1.0, 1.0, 0.0167, 255.0, false, 1),
            p("Mars", PlanetKind::ColdDesert, 0.107, 0.532, 1.524, 0.093, 210.0, false, 2),
            p("Jupiter", PlanetKind::GasGiant, 317.8, 11.21, 5.203, 0.048, 110.0, false, 95),
            p("Saturn", PlanetKind::GasGiant, 95.2, 9.45, 9.537, 0.054, 81.0, true, 146),
            p("Uranus", PlanetKind::IceGiant, 14.5, 4.01, 19.19, 0.047, 58.0, false, 28),
            p("Neptune", PlanetKind::IceGiant, 17.1, 3.88, 30.07, 0.009, 46.0, false, 16),
        ];
        let belts = vec![
            Belt { name: "the asteroid belt".into(), inner_au: 2.1, outer_au: 3.3, icy: false },
            Belt { name: "the Kuiper belt".into(), inner_au: 30.0, outer_au: 50.0, icy: true },
        ];
        let world = HomeWorld {
            name: "Earth".into(),
            orbit: Orbit::earth(),
            spin: Spin::earth(),
            hz_position: hz_position_of(&star, 1.0),
            flux_earth: 1.0,
            mass_earth: 1.0,
            gravity: 1.0,
            core_fraction: 0.32,
            internal_heat: 1.0,
            magnetic_field: 1.0,
            water: 1.0,
            air: Air::earth(),
            outgassing: 1.0,
            weathering_beta: 0.5,
            weathering_k: 0.09,
            moons: vec![Moon::the_moon()],
            lock_gyr: orbit::tidal_lock_gyr(6.0, 1.0, 1.0, 1.0, 1.0),
        };
        StarSystem { spec: SystemSpec::sol(), star, companion: None, world, planets, belts, climate: None }
    }

    pub fn is_sol(&self) -> bool { self.spec.sol }

    /// What the world circles, as one star: the primary, or a close pair's summed light and
    /// mass.
    pub fn host(&self) -> Star {
        match &self.companion {
            Some(c) if c.circumbinary => Star::pair(&self.star, &c.star),
            _ => self.star.clone(),
        }
    }

    /// Generate the system for a world seed and what was asked.
    pub fn generate(seed: u64, spec: &SystemSpec) -> Self {
        if spec.sol { return Self::sol(); }
        let base = seed ^ 0x5354_4152_5359_5354;
        let stream = |k: u64| ChaCha8Rng::seed_from_u64(base.wrapping_add(k.wrapping_mul(0x9E37_79B9_7F4A_7C15)));
        let (mut r_star, mut r_orbit, mut r_spin, mut r_world, mut r_planets, mut r_names) = (stream(1), stream(2), stream(3), stream(4), stream(5), stream(6));
        let mut namer = names::Namer::new();
        let star_name = namer.next(&mut r_names);
        let world_name = namer.next(&mut r_names);

        let mut star = Star::random(&mut r_star, spec.class, spec.subclass, spec.metallicity, star_name);
        if let Some(age) = spec.age_gyr {
            star = Star::new(star.name.clone(), star.spectral, age.clamp(0.05, star.lifetime_gyr), star.metallicity);
        }
        let feh = star.metallicity;

        // A companion (stream 7): a quarter of the stars (fewer of the light ones), three in ten
        // of them close pairs the world circles together.
        let mut r_pair = stream(7);
        let binary = spec.binary.unwrap_or_else(|| r_pair.gen_bool((0.28 * star.mass_msun.sqrt()).clamp(0.1, 0.35) as f64));
        let mut companion = if binary {
            let m2 = star.mass_msun * r_pair.gen_range(0.25..0.95);
            let name = namer.next(&mut r_names);
            let second = Star::new(name, SpectralType::for_mass(m2), star.age_gyr, feh);
            let circumbinary = r_pair.gen_bool(0.3);
            let (sep, ecc) = if circumbinary {
                (r_pair.gen_range(0.04..0.25) * (star.mass_msun + second.mass_msun).powf(1.0 / 3.0), r_pair.gen_range(0.0..0.3))
            } else {
                ((r_pair.gen_range(40.0f32.ln()..1000.0f32.ln())).exp(), r_pair.gen_range(0.0..0.7))
            };
            let years = (sep.powi(3) / (star.mass_msun + second.mass_msun)).sqrt();
            Some(Companion { star: second, separation_au: sep, eccentricity: ecc, period_days: years * orbit::EARTH_YEAR_DAYS, circumbinary })
        } else { None };
        // The light and the pull the world answers to.
        let host = match &companion { Some(c) if c.circumbinary => Star::pair(&star, &c.star), _ => star.clone() };

        // The orbit: a place in the habitable zone (most worlds in its warm inner part, some
        // near either edge), an eccentricity (most nearly round, one in ten stirred by a
        // neighbour), the season of perihelion.
        let hz = spec.hz.unwrap_or_else(|| {
            let r: f32 = r_orbit.gen();
            if r < 0.22 { r_orbit.gen_range(-0.06..0.04) } else if r < 0.8 { r_orbit.gen_range(0.04..0.2) } else { r_orbit.gen_range(0.2..0.36) }
        });
        let mut e = spec.eccentricity.unwrap_or_else(|| {
            let base = (0.045 * gauss(&mut r_orbit)).abs();
            if r_orbit.gen_bool(0.1) { base + r_orbit.gen_range(0.08..0.25) } else { base }
        }).clamp(0.0, 0.6);
        let perihelion = r_orbit.gen_range(0.0..360.0f32);
        let (s_in, s_out) = host.hz_flux();
        let flux = s_in * (s_out / s_in).powf(hz);

        // The world's bulk: the radius is the Earth's (the map's scale); a richer star gives
        // more iron, a denser world and stronger gravity.
        let core = (0.32 + 0.12 * feh + 0.04 * gauss(&mut r_world)).clamp(0.18, 0.5);
        let density = |c: f32| 1.0 / (c / 11.0 + (1.0 - c) / 4.4);
        let mass = density(core) / density(0.32);
        let gravity = mass;

        // Spin: born fast, slowed by its moon's tides, and by the star's: a world close to a
        // dim star is despun to one face toward it within a few hundred million years.
        let primordial = (12.0f32.ln() + 0.45 * gauss(&mut r_spin)).exp().clamp(5.0, 40.0);
        let a_guess = (host.luminosity_lsun / (flux * (1.0 - e * e).sqrt())).sqrt();
        let lock_gyr = orbit::tidal_lock_gyr(primordial, a_guess, host.mass_msun, mass, 1.0);
        // An air's thermal tide (the day side's heated bulge, pulled ahead by the spin) can hold
        // a world off synchronous rotation around K and early M dwarfs, as Venus's thick air
        // holds it (Leconte et al. 2015): about half such worlds keep turning, slowly.
        let tidally_locked = star.age_gyr > lock_gyr;
        let thermal_tide = tidally_locked && star.mass_msun > 0.45 && r_spin.gen_bool(0.55);
        // On an eccentric orbit the tides catch the spin in a 3:2 resonance instead, as they
        // caught Mercury's: the tidal torque is strongest at perihelion.
        let resonant = tidally_locked && !thermal_tide && spec.locked.is_none() && e > 0.08 && r_spin.gen_bool(0.65);
        // A day asked for is a world that turns.
        let locked = spec.locked.unwrap_or(spec.day_hours.is_none() && tidally_locked && !thermal_tide && !resonant);
        // Tides that lock the spin also round the orbit.
        if locked && spec.eccentricity.is_none() { e = e.min(0.04) * 0.5; }
        let a = (host.luminosity_lsun / (flux * (1.0 - e * e).sqrt())).sqrt();
        // A close pair: the pair shrinks (it migrated in) until the world's orbit is stable.
        if let Some(c) = companion.as_mut().filter(|c| c.circumbinary) {
            let limit = (2.3 + 3.0 * c.eccentricity) * c.separation_au;
            if limit > 0.75 * a {
                c.separation_au *= 0.75 * a / limit;
                c.period_days = (c.separation_au.powi(3) / host.mass_msun).sqrt() * orbit::EARTH_YEAR_DAYS;
            }
        }
        // A wide one: far enough not to strip the world's own orbit.
        if let Some(c) = companion.as_mut().filter(|c| !c.circumbinary) {
            let need = 6.0 * a / (1.0 - c.eccentricity);
            if c.separation_au < need {
                c.separation_au = need;
                c.period_days = (c.separation_au.powi(3) / (star.mass_msun + c.star.mass_msun)).sqrt() * orbit::EARTH_YEAR_DAYS;
            }
        }
        let orbit = Orbit::new(a, e, perihelion, host.mass_msun);

        let moons = planets::moons(locked, mass, &mut namer, &mut r_names);
        let moon_tide: f32 = moons.iter().map(|m| m.tide()).sum();
        let big_moon = moons.iter().any(|m| m.mass_moons > 0.2);

        let retrograde = !locked && !resonant && r_spin.gen_bool(0.07);
        let obliquity = spec.obliquity_deg.unwrap_or_else(|| {
            if locked || resonant { r_spin.gen_range(0.0..3.0) }
            else if big_moon { (23.0 + 9.0 * gauss(&mut r_spin)).clamp(3.0, 45.0) }
            else {
                // Without a great moon the tilt wanders chaotically (Laskar & Robutel 1993).
                let r: f32 = r_spin.gen();
                if r < 0.35 { r_spin.gen_range(0.0..20.0) } else if r < 0.77 { r_spin.gen_range(20.0..45.0) } else { r_spin.gen_range(45.0..85.0) }
            }
        }).clamp(0.0, 90.0);
        let year_h = orbit.period_days * 24.0;
        let sidereal = if locked { year_h } else if resonant { year_h * 2.0 / 3.0 } else if thermal_tide && spec.day_hours.is_none() {
            // Spun down to a slow, steady day of a few to some tens of Earth days.
            let day = (r_spin.gen_range(4.0f32.ln()..40.0f32.ln())).exp() * 24.0;
            1.0 / (1.0 / day + 1.0 / year_h)
        } else if let Some(day) = spec.day_hours {
            // The solar day asked for, as a spin.
            let sign = if retrograde { -1.0 } else { 1.0 };
            let w = 1.0 / day.max(1.0) + sign / year_h;
            1.0 / w.abs().max(1e-6)
        } else {
            let braked = primordial * (1.0 + 1.6 * (star.age_gyr / 4.5) * moon_tide.powf(0.6));
            // The star's tides take part of the way to synchronous.
            let (w0, n) = (1.0 / braked, 1.0 / year_h);
            let w = w0 - (w0 - n) * (star.age_gyr / lock_gyr).min(0.97);
            1.0 / w
        };
        let substellar = if locked || resonant { r_spin.gen_range(0.0..360.0) } else { 0.0 };
        let spin = Spin { sidereal_hours: sidereal, retrograde, obliquity_deg: obliquity, locked, substellar_lon_deg: substellar, primordial_hours: primordial, resonance: resonant };

        // Interior heat: radioactivity (richer stars, more uranium and thorium) and the heat
        // of formation, both falling with age. It drives volcanoes and outgassing.
        let internal_heat = (mass.sqrt() * (-(star.age_gyr - 4.5) / 8.0).exp() * 10f32.powf(0.2 * feh)).clamp(0.3, 3.0);
        let magnetic_field = ((core / 0.32) * spin.rotation_factor().powf(0.25).clamp(0.3, 1.4) * if star.age_gyr > 9.0 { 0.7 } else { 1.0 }).clamp(0.1, 2.5);
        // Water: delivered from beyond the snow line (worlds farther out in the zone get more),
        // more in a metal-rich disk; a red dwarf's long bright youth boils much of it away
        // before the star settles (Luger & Barnes 2015).
        let snow_ratio = a / host.snow_line_au();
        let youth = match star.spectral.class { SpectralClass::M => r_world.gen_range(0.35..0.9), SpectralClass::K => r_world.gen_range(0.8..1.0), _ => 1.0 };
        let water = ((0.35 * gauss(&mut r_world)).exp() * (2.0 * (snow_ratio - 0.37)).exp().clamp(0.5, 2.5) * 10f32.powf(0.25 * feh) * youth).clamp(0.25, 3.0);

        // Air: nitrogen with the volatiles, held by gravity, stripped by an active star where the
        // field is weak; oxygen once life has had a couple of billion years.
        let xuv = star.activity * flux;
        let retention = 1.0 / (1.0 + 0.004 * xuv / magnetic_field.powf(1.5));
        let n2 = (0.78 * (0.4 * gauss(&mut r_world)).exp() * gravity * retention).clamp(0.2, 4.0);
        let o2 = if star.age_gyr > 1.8 { 0.21 * r_world.gen_range(0.4..1.6) } else { r_world.gen_range(0.0..0.02) };
        let air = Air { n2_bar: n2, o2_bar: o2, co2_bar: air::EARTH_CO2_BAR, argon_bar: 0.0093 * n2 / 0.78 };
        let outgassing = (internal_heat * (0.25 * gauss(&mut r_world)).exp()).clamp(0.1, 6.0);
        let weathering_beta = r_world.gen_range(0.3..0.55);
        let weathering_k = r_world.gen_range(0.07..0.1);

        let (inner_min, outer_max) = match &companion {
            Some(c) if c.circumbinary => (c.stability_limit_au(), f32::MAX),
            Some(c) => (0.0, c.truncation_au()),
            None => (0.0, f32::MAX),
        };
        let (planets, belts) = planets::generate(&host, &world_name, a, mass, orbit.period_days, inner_min, outer_max, &mut namer, &mut r_planets);
        let world = HomeWorld {
            name: world_name, orbit, spin, hz_position: hz, flux_earth: flux, mass_earth: mass, gravity, core_fraction: core,
            internal_heat, magnetic_field, water, air, outgassing, weathering_beta, weathering_k, moons, lock_gyr,
        };
        StarSystem { spec: spec.clone(), star, companion, world, planets, belts, climate: None }
    }

    /// The climate's configuration under this sky (Sol: the original Earth model).
    pub fn climate_config(&self, base: crate::climate::ClimateConfig) -> crate::climate::ClimateConfig {
        use crate::climate::{AirForcing, PlanetForcing};
        if self.is_sol() { return base; }
        let w = &self.world;
        let planet = PlanetForcing {
            eccentricity: w.orbit.eccentricity,
            perihelion_lon: w.orbit.perihelion_lon_deg.to_radians(),
            rotation: w.spin.rotation_factor(),
            year: w.orbit.years(),
            locked: if w.spin.locked { Some(w.spin.substellar_lon_deg.to_radians()) } else { None },
            hot_longitudes: if w.spin.resonance { Some((w.spin.substellar_lon_deg.to_radians(), (1.5 * w.orbit.eccentricity).min(0.4))) } else { None },
            teff: self.host().teff_k,
            lapse_rate: crate::climate::ebm::ELEVATION_LAPSE_RATE * w.gravity,
            air: AirForcing::Cycle { dry_bar: w.air.dry_bar(), outgassing: w.outgassing, beta: w.weathering_beta, k_per_c: w.weathering_k },
            legacy: false,
        };
        crate::climate::ClimateConfig {
            axial_tilt_deg: w.spin.obliquity_deg,
            solar_constant: crate::climate::ebm::SOLAR_CONSTANT * self.host().luminosity_lsun / (w.orbit.semi_major_au * w.orbit.semi_major_au),
            planet,
            ..base
        }
    }

    /// The share of the style's land the water leaves dry (1 for Earth's oceans): a dry world
    /// shows more land, a wet one drowns it.
    pub fn land_scale(&self) -> f32 {
        if self.is_sol() { 1.0 } else { self.world.water.powf(-0.7).clamp(0.45, 1.9) }
    }

    /// Mantle hotspots for the tectonics (Earth: 8).
    pub fn hotspots(&self) -> usize {
        if self.is_sol() { 8 } else { (8.0 * self.world.internal_heat.powf(0.8)).round().clamp(2.0, 20.0) as usize }
    }

    /// How violent the sky and the interior are (`crate::upheaval`): the impact flux from the
    /// system's age (a young system is still sweeping up its debris: the flux falls with an
    /// e-folding of 0.25 Gyr to the Earth's after ~1.5 Gyr), its belts (rocky ones near the
    /// world deliver asteroids, icy ones comets), a giant to stir them (none: comets fall in
    /// freely, as they would without Jupiter's shepherding) and a companion star (it shakes the
    /// comet cloud), its metals (dustier discs); the gravity and the interior's heat. The Sun's
    /// is the Earth's.
    pub fn upheaval(&self) -> crate::upheaval::Upheaval {
        if self.is_sol() { return crate::upheaval::Upheaval::earth(); }
        let age = self.star.age_gyr.max(0.05);
        let young = 1.0 + 30.0 * (-age / 0.25).exp();
        let rocky = self.belts.iter().filter(|b| !b.icy).count() as f32;
        let icy = self.belts.iter().filter(|b| b.icy).count() as f32;
        let giants = self.planets.iter().filter(|p| p.kind.is_giant()).count();
        let mut flux = young * (0.6 + 0.35 * rocky + 0.2 * icy);
        if giants == 0 { flux *= 1.6; }
        if self.companion.is_some() { flux *= 1.4; }
        flux *= (1.0 + 0.6 * self.star.metallicity).clamp(0.5, 1.6);
        crate::upheaval::Upheaval {
            impact_flux: flux.clamp(0.3, 25.0),
            gravity: self.world.gravity,
            age_gyr: age,
            internal_heat: self.world.internal_heat,
        }
    }

    /// Record what the climate made of this sky.
    pub fn note_climate(&mut self, sim: &crate::climate::ClimateSimulation, heightmap: &crate::tilemap::Tilemap<f32>) {
        self.climate = Some(sky::climate_notes(sim, heightmap));
        if !self.is_sol() { self.world.air.co2_bar = sim.air.co2_bar; }
    }
}

/// Place in a star's habitable zone of a world receiving `flux` (relative to Earth's).
pub fn hz_position_of(star: &Star, flux: f32) -> f32 {
    let (s_in, s_out) = star.hz_flux();
    (flux / s_in).ln() / (s_out / s_in).ln()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specs_round_trip_through_codes() {
        let spec = SystemSpec { class: Some(SpectralClass::K), subclass: Some(3), hz: Some(0.25), obliquity_deg: Some(62.5), day_hours: Some(31.0), eccentricity: Some(0.12), metallicity: Some(-0.2), age_gyr: Some(6.5), locked: Some(false), binary: Some(true), sol: false };
        let code = spec.code();
        assert!(!code.contains('.'), "{code}");
        assert_eq!(SystemSpec::from_code(&code), Some(spec));
        assert_eq!(SystemSpec::from_code("sol"), Some(SystemSpec::sol()));
        assert_eq!(SystemSpec::from_code("auto").map(|s| s.code()), Some("auto".to_string()));
    }

    #[test]
    fn systems_are_deterministic_and_varied() {
        let a = StarSystem::generate(42, &SystemSpec::default());
        let b = StarSystem::generate(42, &SystemSpec::default());
        assert_eq!(a.star.name, b.star.name);
        assert_eq!(a.world.orbit.semi_major_au, b.world.orbit.semi_major_au);
        let mut classes = std::collections::HashSet::new();
        let mut locked = 0;
        for s in 0..200 {
            let sys = StarSystem::generate(s, &SystemSpec::default());
            classes.insert(sys.star.spectral.class);
            if sys.world.spin.locked { locked += 1; }
            let (inner, outer) = sys.host().hz_au();
            assert!(sys.world.orbit.semi_major_au > inner * 0.9 && sys.world.orbit.semi_major_au < outer, "seed {s}: off the zone");
        }
        assert_eq!(classes.len(), 4);
        assert!(locked > 10 && locked < 120, "{locked} of 200 locked");
    }

    #[test]
    fn pinning_the_star_keeps_the_rest() {
        let a = StarSystem::generate(7, &SystemSpec::default());
        let b = StarSystem::generate(7, &SystemSpec { hz: Some(0.3), ..Default::default() });
        assert_eq!(a.star.name, b.star.name);
        assert_eq!(a.star.spectral, b.star.spectral);
        assert!((b.world.hz_position - 0.3).abs() < 1e-6);
    }

    #[test]
    fn red_dwarf_worlds_lock() {
        let m = StarSystem::generate(3, &SystemSpec { class: Some(SpectralClass::M), subclass: Some(4), ..Default::default() });
        assert!(m.world.spin.locked, "an M4's habitable world is locked (lock in {} Gyr)", m.world.lock_gyr);
        assert!(m.world.spin.solar_day_hours(&m.world.orbit).is_none());
        let g = StarSystem::generate(3, &SystemSpec { class: Some(SpectralClass::G), ..Default::default() });
        assert!(!g.world.spin.locked);
    }

    #[test]
    fn sol_forces_the_original_climate() {
        let sol = StarSystem::sol();
        let base = crate::climate::ClimateConfig::default();
        let c = sol.climate_config(base);
        assert_eq!(c.planet, base.planet);
        assert_eq!(c.solar_constant, base.solar_constant);
        assert_eq!(sol.land_scale(), 1.0);
        assert_eq!(sol.hotspots(), 8);
    }
}
