//! Upheaval: the violent land (ROADMAP Stage 7, 2026-10-17). What the sky and the planet's
//! interior do to the ground in a hurry, as records the world keeps and every scale reads:
//!
//! - **Impacts** (`Impacts`): every crater a world still shows, drawn from the star system's
//!   impact flux (its belts, its age, a companion star) by the size-frequency law of the
//!   Earth's (cumulative N(>D) ~ D^-1.8, preserved for a time growing with the size, so the
//!   preserved counts go as D^-0.6 up to the age cap), each with a centre on the sphere, a
//!   diameter, an age and how worn it is. Shapes by size and gravity: simple bowls (depth 0.2 D)
//!   below the transition diameter (3.2 km / g), complex craters with a flat floor, terraced
//!   walls and a central peak, peak-ring basins above 30 times it; a rim and an ejecta blanket
//!   falling as r^-3. The world map takes the largest (over 1.5 tiles across) before its climate
//!   and rivers (`apply_world_impacts`); the region shapes the rest (`region::landforms::impact`),
//!   the embark carves the small fresh ones (`small_craters`).
//! - **Faults** (`Faults`): the plates' boundaries kept with their kind (normal where they pull
//!   apart, thrust where they meet, strike-slip where they slide past), their strike and a
//!   seismic intensity spread into a zone a few hundred km wide. The region draws scarps,
//!   troughs and sag ponds from them (`region::landforms::tectonic`); the history's earthquakes
//!   strike where they are (`history::geology`).
//! - **Provinces** (`Province`): flood basalts over a mantle plume, from the interior's heat:
//!   a plateau of stacked flows the region cuts into steps (traps).
//!
//! Who gets it (`params`): every world under another star (the star system's numbers), none
//! under the Sun unless asked (`--upheaval`, `PLANET_UPHEAVAL=1`; `--no-upheaval` or `=0`
//! turns it off everywhere), so `--star sol` and the dev world stay as they were, bit for bit,
//! as the sky's events do (`history/sky.rs`). Nothing here draws from another stage's RNG: the
//! records are hashed from the seed.

use std::sync::OnceLock;

use crate::astro::StarSystem;
use crate::heightmap::{LavaState, VolcanoLocation, VolcanoType};
use crate::plates::{Plate, PlateId};
use crate::tilemap::Tilemap;

/// The planet's violence, from its star system.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Upheaval {
    /// Impacts relative to the Earth's over the last half billion years.
    pub impact_flux: f32,
    /// Surface gravity (g).
    pub gravity: f32,
    /// The system's age (Gyr): no crater is older.
    pub age_gyr: f32,
    /// Heat from the interior relative to the Earth's: plumes, flood basalts, faults' speed.
    pub internal_heat: f32,
}

impl Upheaval {
    pub fn earth() -> Self { Upheaval { impact_flux: 1.0, gravity: 1.0, age_gyr: 4.6, internal_heat: 1.0 } }
    /// Diameter (km) above which a crater collapses into a complex one: ~3.2 km on the Earth,
    /// inversely with gravity (Melosh 1989).
    pub fn transition_km(&self) -> f32 { 3.2 / self.gravity.clamp(0.2, 5.0) }
    /// A short tag for cache keys and the journal.
    pub fn tag(&self) -> String {
        format!("u{}-{:.0}-{:.0}-{:.0}-{:.0}", SHAPE_VERSION, self.impact_flux * 100.0, self.gravity * 100.0, self.age_gyr * 100.0, self.internal_heat * 100.0)
    }
}

/// Keys the region's chunk cache on an upheaval world (with `Upheaval::tag`): bump when a
/// region pass of the violent land changes the heights it makes.
pub const SHAPE_VERSION: u32 = 5;

static FORCED: OnceLock<Option<bool>> = OnceLock::new();

/// Set at start (`main`'s `--upheaval` / `--no-upheaval`); otherwise read from
/// `PLANET_UPHEAVAL` on first use.
pub fn set_forced(v: Option<bool>) { let _ = FORCED.set(v); }

fn forced() -> Option<bool> {
    *FORCED.get_or_init(|| match std::env::var("PLANET_UPHEAVAL").ok().as_deref().map(str::trim) {
        Some("1") | Some("on") | Some("true") | Some("yes") => Some(true),
        Some("0") | Some("off") | Some("false") | Some("no") => Some(false),
        _ => None,
    })
}

/// The upheaval a world under `system` gets (none under the Sun unless forced). `None` for the
/// system reads as the Sun's.
pub fn params(system: Option<&StarSystem>) -> Option<Upheaval> {
    match forced() {
        Some(false) => None,
        Some(true) => Some(system.map(|s| s.upheaval()).unwrap_or_else(Upheaval::earth)),
        None => system.filter(|s| !s.is_sol()).map(|s| s.upheaval()),
    }
}

// ---------------------------------------------------------------------------------------------
// Hashing (no RNG: a record is a pure function of the seed).

fn mix(mut h: u64) -> u64 {
    h ^= h >> 31;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 29;
    h = h.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^ (h >> 32)
}

pub(crate) fn hash3(a: u64, b: u64, salt: u64) -> u64 {
    mix(a.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ salt.wrapping_mul(0x1656_67B1_9E37_79F9))
}

pub(crate) fn unit3(a: u64, b: u64, salt: u64) -> f32 { (hash3(a, b, salt) >> 40) as f32 / (1u64 << 24) as f32 }

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A world field sampled bilinearly at tile coordinates (tile centres at integers, x wrapped,
/// y clamped).
pub fn sample_world(map: &Tilemap<f32>, u: f32, v: f32) -> f32 {
    let (w, h) = (map.width as i64, map.height as i64);
    let (x0, y0) = (u.floor(), v.floor());
    let (tx, ty) = (u - x0, v - y0);
    let at = |x: i64, y: i64| *map.get(x.rem_euclid(w) as usize, y.clamp(0, h - 1) as usize);
    let (x0, y0) = (x0 as i64, y0 as i64);
    (at(x0, y0) * (1.0 - tx) + at(x0 + 1, y0) * tx) * (1.0 - ty) + (at(x0, y0 + 1) * (1.0 - tx) + at(x0 + 1, y0 + 1) * tx) * ty
}

/// Kilometres across a world tile on a map `width` tiles round.
pub fn tile_km(width: usize) -> f32 { crate::erosion::landscape::tile_km(width) }

/// Great-circle-ish distance (km) between tile coordinates (tile centres at integers; x wraps):
/// the east-west step shrinks by cos latitude.
pub fn distance_km(width: usize, height: usize, u0: f32, v0: f32, u1: f32, v1: f32) -> f32 {
    let w = width as f32;
    let mut du = u1 - u0;
    du -= (du / w).round() * w;
    let lat = std::f32::consts::FRAC_PI_2 - ((v0 + v1) * 0.5 + 0.5) / height as f32 * std::f32::consts::PI;
    let km = tile_km(width);
    ((du * lat.cos().abs().max(0.02)).powi(2) + (v1 - v0).powi(2)).sqrt() * km
}

/// The x offset (in tiles, -w/2..w/2) from u0 to u1 across the date line.
fn wrap_du(width: usize, u0: f32, u1: f32) -> f32 {
    let w = width as f32;
    let du = u1 - u0;
    du - (du / w).round() * w
}

// ---------------------------------------------------------------------------------------------
// Impacts.

/// Preserved craters at least 1 km across per km2 at the Earth's flux (the rate of craters 20 km
/// and over, 5.5e-15 per km2 a year (Grieve and Shoemaker 1994), carried down to 1 km by the
/// D^-1.8 law and kept ~1 Myr at 1 km).
const DENSITY_1KM: f64 = 1.2e-6;
/// Preserved craters' cumulative exponent (production 1.8 less preservation 1.2).
const PRESERVED_EXPONENT: f32 = 0.6;
/// Erosion keeps a crater of D km about `KEEP_MYR * D^1.2` Myr, at most `KEEP_CAP_MYR`.
const KEEP_MYR: f32 = 1.0;
const KEEP_CAP_MYR: f32 = 800.0;
/// The largest basin a world keeps (km).
const D_MAX_KM: f32 = 320.0;
/// Craters at least this many tiles across are shaped on the world map itself.
pub const WORLD_SCALE_TILES: f32 = 1.5;
/// The small fresh craters (the embark's: 50 m to 1.2 km) per km2 at the Earth's flux: the same
/// law below 1 km, kept for `KEEP_MYR * D^1.2` (a 100 m crater some 60 thousand years).
const SMALL_MIN_KM: f32 = 0.05;
pub const SMALL_MAX_KM: f32 = 1.2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CraterKind {
    /// A bowl.
    Simple,
    /// A flat floor, terraced walls and a central peak.
    Complex,
    /// A ring of peaks inside the rim.
    PeakRing,
}

#[derive(Clone, Debug)]
pub struct Crater {
    pub id: u32,
    /// Centre in world tile coordinates (tile centres at integers).
    pub u: f32,
    pub v: f32,
    pub d_km: f32,
    pub age_myr: f32,
    /// 0 fresh .. 1 nearly worn away.
    pub wear: f32,
    pub kind: CraterKind,
    /// Simple-to-complex transition (km), for the profile.
    pub transition_km: f32,
}

impl Crater {
    pub fn r_km(&self) -> f32 { self.d_km * 0.5 }
    /// How far its rim and ejecta reach (km).
    pub fn reach_km(&self) -> f32 { self.d_km * 1.5 }
    /// Rim crest to floor (km), as it struck.
    pub fn depth_km(&self) -> f32 {
        let dt = self.transition_km;
        if self.d_km < dt { 0.2 * self.d_km } else { 0.2 * dt * (self.d_km / dt).powf(0.3) }
    }
    pub fn rim_km(&self) -> f32 {
        if self.kind == CraterKind::Simple { 0.04 * self.d_km } else { 0.25 * self.depth_km() }
    }
    /// Sediment laid in the bowl since it struck (km): ~18 m a million years, never above the
    /// ground round it.
    pub fn infill_km(&self) -> f32 {
        let d = self.depth_km() * (1.0 - 0.5 * self.wear);
        let hr = self.rim_km() * (1.0 - 0.85 * self.wear);
        (self.age_myr * 0.018).min((d - hr).max(0.0) * 0.97)
    }
    /// Depth below the ground round it now (m): what infill and the rivers left.
    pub fn floor_depth_m(&self) -> f32 {
        let d = self.depth_km() * (1.0 - 0.5 * self.wear);
        let hr = self.rim_km() * (1.0 - 0.85 * self.wear);
        ((d - hr) - self.infill_km()).max(0.0) * 1000.0
    }
    /// A number in [0, 1) of this crater and a salt.
    fn u(&self, salt: u64) -> f32 { unit3(self.id as u64, (self.d_km * 1000.0) as u64, salt) }
    /// The rim's reach at angle `theta` relative to the mean (scalloped by slumping), the rim's
    /// height there relative to the mean, and how far into a breach the angle lies (0 none ..
    /// 1 the breach's middle): the rivers cut one breach into a worn rim, two into a very worn one.
    fn rim_at(&self, theta: f32) -> (f32, f32, f32) {
        let amp = if self.kind == CraterKind::Simple { 0.025 } else { 0.06 };
        let reach = 1.0 + amp * (5.0 * theta + 6.28 * self.u(1)).sin() + 0.5 * amp * (11.0 * theta + 6.28 * self.u(2)).sin();
        let height = 0.75 + 0.25 * (3.0 * theta + 6.28 * self.u(3)).sin() + 0.1 * (7.0 * theta + 6.28 * self.u(4)).sin();
        let breaches = if self.wear > 0.5 { 2 } else if self.wear > 0.12 { 1 } else { 0 };
        let mut br = 0.0f32;
        for b in 0..breaches {
            let at = 6.28 * self.u(10 + b as u64);
            let mut dt = (theta - at).rem_euclid(std::f32::consts::TAU);
            if dt > std::f32::consts::PI { dt -= std::f32::consts::TAU; }
            let half = 0.1 + 0.2 * self.wear;
            br = br.max(1.0 - smoothstep(0.3 * half, half, dt.abs()));
        }
        (reach, height, br)
    }
    /// Distance from the centre in rims (1 on the rim) at `r_km` and angle `theta`, and the
    /// ejecta lobe's reach factor there.
    pub fn x_at(&self, r_km: f32, theta: f32) -> (f32, f32) {
        let (reach, _, _) = self.rim_at(theta);
        let lobe = 0.7 + 0.5 * (4.0 * theta + 6.28 * self.u(5)).sin().max(-0.6) + 0.25 * (9.0 * theta + 6.28 * self.u(6)).sin();
        (r_km / (self.r_km().max(0.01) * reach), lobe.max(0.4))
    }

    /// The crater's height (m) relative to the ground before it struck, at `r_km` from its
    /// centre, and how much of the old ground it replaces (1 inside the walls, 0 beyond the rim:
    /// there the ejecta are added to the ground). Without an angle: the mean profile.
    pub fn profile(&self, r_km: f32) -> (f32, f32) { self.profile_at(r_km, 0.0, false) }

    /// The profile at angle `theta` (radians, from east toward south in tile coordinates), with
    /// the rim's scallops, heights and breaches and the ejecta's lobes when `rough`.
    pub fn profile_at(&self, r_km: f32, theta: f32, rough: bool) -> (f32, f32) {
        let (reach, rim_h, breach) = if rough { self.rim_at(theta) } else { (1.0, 1.0, 0.0) };
        let r = self.r_km().max(0.01) * reach;
        let x = r_km / r;
        let wear = self.wear.clamp(0.0, 1.0);
        let d = self.depth_km() * (1.0 - 0.5 * wear);
        let hr0 = self.rim_km() * (1.0 - 0.85 * wear);
        // A breach takes the rim down to the ground (the river's way out).
        let hr = hr0 * rim_h * (1.0 - breach);
        if x >= 1.0 {
            // Ejecta in lobes: thicker and farther in some directions.
            let lobe = if rough { 0.7 + 0.5 * (4.0 * theta + 6.28 * self.u(5)).sin().max(-0.6) + 0.25 * (9.0 * theta + 6.28 * self.u(6)).sin() } else { 1.0 };
            let xe = x / lobe.max(0.4);
            if xe > 3.0 { return (0.0, 0.0); }
            let t = hr * xe.powi(-3) * (1.0 - smoothstep(2.2, 3.0, xe));
            return (t * 1000.0, 0.0);
        }
        let w = 1.0 - smoothstep(0.82, 1.0, x);
        let fill = self.infill_km();
        let floor = -(d - hr0);
        let t = match self.kind {
            CraterKind::Simple => floor + d * x * x,
            CraterKind::Complex | CraterKind::PeakRing => {
                let f = if self.kind == CraterKind::Complex { 0.45 } else { 0.55 };
                let peak = if self.kind == CraterKind::Complex {
                    0.45 * self.depth_km() * (1.0 - 0.5 * wear) * (-(x / 0.13).powi(2)).exp()
                } else {
                    0.3 * self.depth_km() * (1.0 - 0.5 * wear) * (-((x - 0.42) / 0.07).powi(2)).exp()
                };
                if x < f {
                    floor + peak
                } else {
                    // Terraced walls: slumped blocks, three steps, softened by wear.
                    let s = (x - f) / (1.0 - f);
                    let n = 3.0;
                    let q = s * n;
                    let k = q.floor();
                    let st = (k + smoothstep(0.55, 1.0, q - k)) / n;
                    let s2 = (0.6 - 0.4 * wear) * st + (0.4 + 0.4 * wear) * s;
                    floor + d * s2 + peak
                }
            }
        };
        // Sediment fills the bowl flat to its level; peaks stand out of it. Where the wall meets
        // a breach, the wall rises only to the ground.
        let t = t.max(floor + fill);
        let t = if breach > 0.0 && x > 0.6 { t.min(hr0 * rim_h * (1.0 - breach) + (floor + fill) * breach * (1.0 - x) / 0.4) } else { t };
        (t * 1000.0, w)
    }
}

/// Every crater the world shows, with an index by tile.
#[derive(Clone, Debug, Default)]
pub struct Impacts {
    pub craters: Vec<Crater>,
    pub width: usize,
    pub height: usize,
    /// For each tile, the craters whose reach touches it.
    by_tile: Vec<Vec<u32>>,
    pub flux: f32,
}

impl Impacts {
    /// The record of a world `width` x `height` from its seed and upheaval.
    pub fn generate(width: usize, height: usize, seed: u64, up: &Upheaval) -> Impacts {
        let km = tile_km(width) as f64;
        let radius = km * width as f64 / (2.0 * std::f64::consts::PI);
        let area = 4.0 * std::f64::consts::PI * radius * radius;
        let lambda = DENSITY_1KM * area * up.impact_flux as f64;
        let n = (lambda + unit3(seed, 0, 0xC0A7) as f64).floor() as u64;
        let dt = up.transition_km();
        let age_cap = (up.age_gyr * 1000.0).max(1.0);
        let mut craters = Vec::new();
        let mut i = 0u64;
        while (craters.len() as u64) < n && i < n * 4 + 16 {
            i += 1;
            let u0 = unit3(seed, i, 0x1D1A);
            let mut d = (1.0 - u0).max(1e-6).powf(-1.0 / PRESERVED_EXPONENT);
            if d > D_MAX_KM { continue; }
            // Above the age cap, preservation no longer grows with size: the production law.
            let cap_d = (KEEP_CAP_MYR / KEEP_MYR).powf(1.0 / 1.2);
            if d > cap_d && unit3(seed, i, 0xCA9) > (cap_d / d).powf(1.2) { continue; }
            d = d.max(1.0);
            let keep = (KEEP_MYR * d.powf(1.2)).min(KEEP_CAP_MYR).min(age_cap);
            let age = unit3(seed, i, 0xA6E) * keep;
            let wear = (age / (KEEP_MYR * d.powf(1.2)).min(KEEP_CAP_MYR)).clamp(0.0, 0.98);
            // Uniform on the sphere.
            let lon = unit3(seed, i, 0x10A);
            let sin_lat = 2.0 * unit3(seed, i, 0x1A7) - 1.0;
            let lat = sin_lat.asin();
            let u = lon * width as f32 - 0.5;
            let v = (0.5 - lat / std::f32::consts::PI) * height as f32 - 0.5;
            let kind = if d < dt { CraterKind::Simple } else if d < 30.0 * dt { CraterKind::Complex } else { CraterKind::PeakRing };
            craters.push(Crater { id: craters.len() as u32, u, v, d_km: d, age_myr: age, wear, kind, transition_km: dt });
        }
        let mut out = Impacts { craters, width, height, by_tile: vec![Vec::new(); width * height], flux: up.impact_flux };
        out.index();
        out
    }

    fn index(&mut self) {
        let (w, h) = (self.width, self.height);
        let km = tile_km(w);
        for c in &self.craters {
            let reach_v = c.reach_km() / km + 1.0;
            let lat = std::f32::consts::FRAC_PI_2 - (c.v + 0.5) / h as f32 * std::f32::consts::PI;
            let reach_u = (reach_v / lat.cos().abs().max(0.05)).min(w as f32 * 0.5);
            let (v0, v1) = ((c.v - reach_v).floor().max(0.0) as i64, (c.v + reach_v).ceil().min(h as f32 - 1.0) as i64);
            let (u0, u1) = ((c.u - reach_u).floor() as i64, (c.u + reach_u).ceil() as i64);
            for y in v0..=v1 {
                for x in u0..=u1 {
                    let k = y as usize * w + x.rem_euclid(w as i64) as usize;
                    if !self.by_tile[k].contains(&c.id) { self.by_tile[k].push(c.id); }
                }
            }
        }
    }

    /// The craters whose reach touches world tile (x, y).
    pub fn near(&self, x: usize, y: usize) -> impl Iterator<Item = &Crater> + '_ {
        let k = y.min(self.height.saturating_sub(1)) * self.width + x % self.width.max(1);
        self.by_tile.get(k).into_iter().flatten().map(move |&i| &self.craters[i as usize])
    }

    /// The craters within `tiles` of world tile (x, y).
    pub fn around(&self, x: usize, y: usize, tiles: i64) -> Vec<&Crater> {
        let mut out: Vec<&Crater> = Vec::new();
        for dy in -tiles..=tiles {
            let yy = y as i64 + dy;
            if yy < 0 || yy >= self.height as i64 { continue; }
            for dx in -tiles..=tiles {
                let xx = (x as i64 + dx).rem_euclid(self.width as i64) as usize;
                for c in self.near(xx, yy as usize) {
                    if !out.iter().any(|o| o.id == c.id) { out.push(c); }
                }
            }
        }
        out
    }

    pub fn distance_km(&self, c: &Crater, u: f32, v: f32) -> f32 { distance_km(self.width, self.height, c.u, c.v, u, v) }

    /// The tile a crater's centre lies on.
    pub fn tile_of(&self, c: &Crater) -> (usize, usize) {
        ((c.u.round() as i64).rem_euclid(self.width as i64) as usize, (c.v.round() as i64).clamp(0, self.height as i64 - 1) as usize)
    }
}

/// The small fresh craters of world tile (x, y) (the embark's scale): centre (tile coordinates),
/// diameter (km), wear. A pure function of the tile and the seed.
pub fn small_craters(width: usize, height: usize, seed: u64, up: &Upheaval, x: usize, y: usize) -> Vec<(f32, f32, f32, f32)> {
    let km = tile_km(width);
    let lat = std::f32::consts::FRAC_PI_2 - (y as f32 + 0.5) / height as f32 * std::f32::consts::PI;
    let area = (km * km * lat.cos().abs().max(0.02)) as f64;
    // Craters 50 m - 1.2 km preserved, by the same law as the large ones.
    let per = DENSITY_1KM * ((SMALL_MIN_KM.powf(-PRESERVED_EXPONENT) - SMALL_MAX_KM.powf(-PRESERVED_EXPONENT)) as f64);
    let lambda = per * area * up.impact_flux as f64;
    let key = (y as u64) << 32 | x as u64;
    let n = (lambda + unit3(seed, key, 0x5A11) as f64).floor() as u64;
    (0..n.min(12)).map(|i| {
        let u0 = unit3(seed ^ key, i, 0x5D1);
        let lo = SMALL_MAX_KM.powf(-PRESERVED_EXPONENT);
        let hi = SMALL_MIN_KM.powf(-PRESERVED_EXPONENT);
        let d = (lo + (hi - lo) * u0).powf(-1.0 / PRESERVED_EXPONENT);
        let cu = x as f32 - 0.5 + unit3(seed ^ key, i, 0x5D2);
        let cv = y as f32 - 0.5 + unit3(seed ^ key, i, 0x5D3);
        let wear = unit3(seed ^ key, i, 0x5D4) * 0.9;
        (cu, cv, d, wear)
    }).collect()
}

/// Shape the world map's largest craters (over `WORLD_SCALE_TILES` across), before its climate
/// and rivers: they wear the rims and fill the bowls. Returns how many were shaped.
pub fn apply_world_impacts(heightmap: &mut Tilemap<f32>, impacts: &Impacts) -> usize {
    let (w, h) = (heightmap.width, heightmap.height);
    let km = tile_km(w);
    let before = heightmap.clone();
    let mut shaped = 0;
    for c in &impacts.craters {
        if c.d_km < WORLD_SCALE_TILES * km { continue; }
        let href = sample_world(&before, c.u, c.v);
        if href <= 0.0 { continue; }
        shaped += 1;
        let reach_v = (c.reach_km() / km).ceil() as i64 + 1;
        let lat = std::f32::consts::FRAC_PI_2 - (c.v + 0.5) / h as f32 * std::f32::consts::PI;
        let reach_u = ((reach_v as f32) / lat.cos().abs().max(0.05)).ceil() as i64;
        for dy in -reach_v..=reach_v {
            let y = c.v.round() as i64 + dy;
            if y < 0 || y >= h as i64 { continue; }
            for dx in -reach_u..=reach_u {
                let x = (c.u.round() as i64 + dx).rem_euclid(w as i64) as usize;
                let r = impacts.distance_km(c, x as f32, y as f32);
                if r > c.reach_km() { continue; }
                let lat = std::f32::consts::FRAC_PI_2 - (y as f32 + 0.5) / h as f32 * std::f32::consts::PI;
                let theta = (y as f32 - c.v).atan2(wrap_du(w, c.u, x as f32) * lat.cos().abs().max(0.02));
                let (t, wgt) = c.profile_at(r, theta, true);
                let cur = *heightmap.get(x, y as usize);
                let old = cur + (href - cur) * wgt;
                heightmap.set(x, y as usize, old + t);
            }
        }
    }
    shaped
}

// ---------------------------------------------------------------------------------------------
// Faults.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
pub enum FaultKind {
    #[default]
    None,
    /// Pulling apart: tilted blocks, scarps facing the low side, grabens.
    Normal,
    /// Pushing together: scarps facing out, folds over the fault.
    Thrust,
    /// Sliding past: a straight trough, sag ponds, streams bent along it.
    StrikeSlip,
}

impl FaultKind {
    pub fn word(self) -> &'static str {
        match self {
            FaultKind::None => "no fault",
            FaultKind::Normal => "a normal fault, where the crust pulls apart",
            FaultKind::Thrust => "a thrust fault, where the plates meet",
            FaultKind::StrikeSlip => "a strike-slip fault, where the plates slide past each other",
        }
    }
}

/// A sample of the fault field at a point.
#[derive(Clone, Copy, Debug)]
pub struct FaultSample {
    pub kind: FaultKind,
    /// The faults' strike (radians, east = 0, south = pi/2: tile coordinates).
    pub strike: f32,
    /// Seismic intensity 0..1.
    pub intensity: f32,
}

/// The faults of a world, per tile.
#[derive(Clone, Debug, Default)]
pub struct Faults {
    pub width: usize,
    pub height: usize,
    pub kind: Vec<FaultKind>,
    /// The strike as a doubled angle (cos 2t, sin 2t), so that t and t + pi average alike.
    pub strike2: Vec<(f32, f32)>,
    pub intensity: Vec<f32>,
}

/// Width of a fault zone (km) either side of the boundary.
const ZONE_KM: f32 = 260.0;

impl Faults {
    pub fn compute(plate_map: &Tilemap<PlateId>, plates: &[Plate], stress: &Tilemap<f32>, heat: f32) -> Faults {
        let (w, h) = (plate_map.width, plate_map.height);
        let n = w * h;
        let mut kind = vec![FaultKind::None; n];
        let mut strike2 = vec![(0.0f32, 0.0f32); n];
        let mut intensity = vec![0.0f32; n];
        let mut src: Vec<usize> = Vec::new();
        let mut top_speed = 1e-6f32;
        let mut raw: Vec<(usize, FaultKind, (f32, f32), f32)> = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let me = *plate_map.get(x, y);
                if me.is_none() || me.0 as usize >= plates.len() { continue; }
                let (mut vn_sum, mut vt_sum, mut s2, mut count, mut speed) = (0.0f32, 0.0f32, (0.0f32, 0.0f32), 0, 0.0f32);
                for (dx, dy) in [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)] {
                    let ny = y as i64 + dy;
                    if ny < 0 || ny >= h as i64 { continue; }
                    let nx = (x as i64 + dx).rem_euclid(w as i64) as usize;
                    let other = *plate_map.get(nx, ny as usize);
                    if other == me || other.is_none() || other.0 as usize >= plates.len() { continue; }
                    let (a, b) = (&plates[me.0 as usize], &plates[other.0 as usize]);
                    let (rx, ry) = (a.velocity.x - b.velocity.x, a.velocity.y - b.velocity.y);
                    let (nxf, nyf) = (dx as f32, dy as f32);
                    let vn = rx * nxf + ry * nyf;
                    let vt = rx * -nyf + ry * nxf;
                    vn_sum += vn;
                    vt_sum += vt.abs();
                    // The boundary runs across the normal: strike = normal + 90 degrees.
                    let t = nyf.atan2(nxf) + std::f32::consts::FRAC_PI_2;
                    s2.0 += (2.0 * t).cos();
                    s2.1 += (2.0 * t).sin();
                    speed += (rx * rx + ry * ry).sqrt();
                    count += 1;
                }
                if count == 0 { continue; }
                let (vn, vt) = (vn_sum / count as f32, vt_sum / count as f32);
                let k = if vt > 1.5 * vn.abs() { FaultKind::StrikeSlip } else if vn > 0.0 { FaultKind::Thrust } else { FaultKind::Normal };
                let sp = speed / count as f32;
                top_speed = top_speed.max(sp);
                let len = (s2.0 * s2.0 + s2.1 * s2.1).sqrt().max(1e-6);
                raw.push((y * w + x, k, (s2.0 / len, s2.1 / len), sp));
            }
        }
        for (c, k, s, sp) in raw {
            kind[c] = k;
            strike2[c] = s;
            intensity[c] = ((0.45 + 0.55 * (sp / top_speed)) * heat.sqrt().clamp(0.6, 1.1)).min(1.0);
            src.push(c);
        }
        // Rifts inside a plate (the stress map's tension) count as normal faults: their strike
        // runs along the trough, across the stress gradient.
        for y in 1..h.saturating_sub(1) {
            for x in 0..w {
                let c = y * w + x;
                let s = *stress.get(x, y);
                if kind[c] != FaultKind::None || s > -0.25 { continue; }
                let gx = *stress.get((x + 1) % w, y) - *stress.get((x + w - 1) % w, y);
                let gy = *stress.get(x, y + 1) - *stress.get(x, y - 1);
                let t = gy.atan2(gx) + std::f32::consts::FRAC_PI_2;
                kind[c] = FaultKind::Normal;
                strike2[c] = ((2.0 * t).cos(), (2.0 * t).sin());
                intensity[c] = (-s).min(1.0) * 0.6;
                src.push(c);
            }
        }
        // Spread into the zone: a breadth-first front, intensity falling with distance.
        let km = tile_km(w);
        let zone = (ZONE_KM / km).max(1.0);
        let mut dist = vec![f32::MAX; n];
        let mut q = std::collections::VecDeque::new();
        for &c in &src { dist[c] = 0.0; q.push_back(c); }
        let base = intensity.clone();
        let mut origin: Vec<usize> = (0..n).collect();
        while let Some(c) = q.pop_front() {
            let (x, y) = (c % w, c / w);
            let d = dist[c] + 1.0;
            if d > zone { continue; }
            for (dx, dy) in [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)] {
                let ny = y as i64 + dy;
                if ny < 0 || ny >= h as i64 { continue; }
                let nc = ny as usize * w + (x as i64 + dx).rem_euclid(w as i64) as usize;
                if dist[nc] <= d { continue; }
                dist[nc] = d;
                origin[nc] = origin[c];
                q.push_back(nc);
            }
        }
        for c in 0..n {
            if dist[c] == f32::MAX || dist[c] == 0.0 { continue; }
            let o = origin[c];
            kind[c] = kind[o];
            strike2[c] = strike2[o];
            intensity[c] = base[o] * (-dist[c] / (0.5 * zone)).exp();
        }
        Faults { width: w, height: h, kind, strike2, intensity }
    }

    /// The field at tile coordinates (u, v): the kind of the nearest tile, the strike and the
    /// intensity bilinear.
    pub fn at(&self, u: f32, v: f32) -> FaultSample {
        let (w, h) = (self.width as i64, self.height as i64);
        if w == 0 { return FaultSample { kind: FaultKind::None, strike: 0.0, intensity: 0.0 }; }
        let (fu, fv) = (u.floor(), v.floor());
        let (tu, tv) = (u - fu, v - fv);
        let idx = |dx: i64, dy: i64| ((fv as i64 + dy).clamp(0, h - 1) * w + (fu as i64 + dx).rem_euclid(w)) as usize;
        let ws = [(0, 0, (1.0 - tu) * (1.0 - tv)), (1, 0, tu * (1.0 - tv)), (0, 1, (1.0 - tu) * tv), (1, 1, tu * tv)];
        let (mut i, mut s0, mut s1) = (0.0f32, 0.0f32, 0.0f32);
        let mut best = (0.0f32, FaultKind::None);
        for &(dx, dy, wt) in &ws {
            let k = idx(dx, dy);
            i += self.intensity[k] * wt;
            s0 += self.strike2[k].0 * wt * self.intensity[k];
            s1 += self.strike2[k].1 * wt * self.intensity[k];
            if self.kind[k] != FaultKind::None && wt * self.intensity[k] > best.0 { best = (wt * self.intensity[k], self.kind[k]); }
        }
        FaultSample { kind: best.1, strike: 0.5 * s1.atan2(s0), intensity: i }
    }

    pub fn tile(&self, x: usize, y: usize) -> FaultSample {
        let k = y * self.width + x;
        let s = self.strike2[k];
        FaultSample { kind: self.kind[k], strike: 0.5 * s.1.atan2(s.0), intensity: self.intensity[k] }
    }
}

// ---------------------------------------------------------------------------------------------
// Flood-basalt provinces.

#[derive(Clone, Debug)]
pub struct Province {
    pub id: u32,
    pub u: f32,
    pub v: f32,
    pub r_km: f32,
    pub age_myr: f32,
    /// Thickness of one flow (m): the height of a step in the traps.
    pub flow_m: f32,
    /// How high the plateau stands over the land it buried (m), as it is now.
    pub lift_m: f32,
}

/// The flood basalts of a world: about one a world at the Earth's heat (the young ones: the
/// Columbia, the Deccan, the Siberian traps), more under a hotter interior.
pub fn provinces(width: usize, height: usize, seed: u64, up: &Upheaval) -> Vec<Province> {
    let expect = 1.2 * up.internal_heat.max(0.0);
    let n = ((expect + unit3(seed, 7, 0x11F)) as f32).floor().clamp(0.0, 5.0) as u64;
    (0..n).map(|i| {
        let lon = unit3(seed, i, 0x11A);
        let lat = (2.0 * unit3(seed, i, 0x11B) - 1.0).asin() * 0.85;
        let age = 10.0 + 250.0 * unit3(seed, i, 0x11C).powf(1.5);
        Province {
            id: i as u32,
            u: lon * width as f32 - 0.5,
            v: (0.5 - lat / std::f32::consts::PI) * height as f32 - 0.5,
            r_km: 250.0 + 450.0 * unit3(seed, i, 0x11D),
            age_myr: age,
            flow_m: 20.0 + 40.0 * unit3(seed, i, 0x11E),
            lift_m: (900.0 - 2.5 * age).max(250.0) * (0.6 + 0.4 * unit3(seed, i, 0x120)),
        }
    }).collect()
}

/// How far into a province (0 outside .. 1 well inside) tile coordinates (u, v) lie, the edge
/// lobed by a hashed wave so the plateau's margin is not a circle.
pub fn province_share(width: usize, height: usize, p: &Province, u: f32, v: f32) -> f32 {
    let d = distance_km(width, height, p.u, p.v, u, v);
    if d > p.r_km * 1.4 { return 0.0; }
    let du = wrap_du(width, p.u, u);
    let ang = (v - p.v).atan2(du);
    let lobes = 1.0 + 0.22 * (3.0 * ang + p.id as f32).sin() + 0.12 * (7.0 * ang + 2.0 * p.id as f32).cos();
    1.0 - smoothstep(0.75, 1.0, d / (p.r_km * lobes))
}

/// Raise the provinces' plateaus on the world map (before its climate and rivers).
pub fn apply_world_provinces(heightmap: &mut Tilemap<f32>, provinces: &[Province]) -> usize {
    let (w, h) = (heightmap.width, heightmap.height);
    let mut cells = 0;
    for p in provinces {
        for y in 0..h {
            for x in 0..w {
                let s = province_share(w, h, p, x as f32, y as f32);
                if s <= 0.0 { continue; }
                let e = *heightmap.get(x, y);
                // Basalt floods the land and the shallow sea; the deep ocean is left.
                if e < -400.0 { continue; }
                heightmap.set(x, y, e + p.lift_m * s);
                cells += 1;
            }
        }
    }
    cells
}

// ---------------------------------------------------------------------------------------------
// Lava that runs down the valleys.

/// From each active vent, lava runs down the steepest descent as far as the kind lets it (a
/// shield's fluid basalt 6 tiles, a caldera's 3, a stratovolcano's sticky lava 2), spreading
/// a tile either side where the ground is flat.
pub fn run_lava_downhill(lava: &mut Tilemap<LavaState>, heightmap: &Tilemap<f32>, volcanoes: &[VolcanoLocation]) -> usize {
    let (w, h) = (heightmap.width, heightmap.height);
    let km = tile_km(w);
    let mut cells = 0;
    for vol in volcanoes.iter().filter(|v| v.is_active) {
        let reach_km = match vol.volcano_type { VolcanoType::Shield => 450.0, VolcanoType::Caldera => 230.0, VolcanoType::Stratovolcano => 150.0 };
        let steps = (reach_km / km).ceil().max(1.0) as usize;
        let (mut x, mut y) = (vol.x, vol.y);
        for step in 0..steps {
            let e = *heightmap.get(x, y);
            if e <= 0.0 { break; }
            let state = if step == 0 { LavaState::Molten } else if step <= steps / 2 { LavaState::Flowing } else { LavaState::Cooled };
            if *lava.get(x, y) == LavaState::None { lava.set(x, y, state); cells += 1; }
            let mut best: Option<(usize, usize, f32)> = None;
            for (nx, ny) in heightmap.neighbors_8(x, y) {
                let ne = *heightmap.get(nx, ny);
                if ne < e && best.map_or(true, |b| ne < b.2) { best = Some((nx, ny, ne)); }
            }
            let Some((nx, ny, ne)) = best else { break };
            // On gentle ground the flow spreads to the side cells as low as it.
            if (e - ne) < 60.0 {
                for (sx, sy) in heightmap.neighbors(x, y) {
                    if *heightmap.get(sx, sy) <= e && *heightmap.get(sx, sy) > 0.0 && *lava.get(sx, sy) == LavaState::None {
                        lava.set(sx, sy, LavaState::Cooled);
                        cells += 1;
                    }
                }
            }
            x = nx;
            y = ny;
        }
    }
    cells
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn craters_follow_the_size_law() {
        let up = Upheaval::earth();
        let rec = Impacts::generate(512, 256, 42, &up);
        let n = rec.craters.len();
        // About 600 craters of 1 km and more on an Earth's globe.
        assert!((350..900).contains(&n), "{n} craters");
        let big = rec.craters.iter().filter(|c| c.d_km >= 20.0).count();
        let huge = rec.craters.iter().filter(|c| c.d_km >= 100.0).count();
        assert!(big > huge && huge < n / 8, "{big} over 20 km, {huge} over 100 km");
        // Simple, complex and (rarely) peak rings all appear.
        assert!(rec.craters.iter().any(|c| c.kind == CraterKind::Simple));
        assert!(rec.craters.iter().any(|c| c.kind == CraterKind::Complex));
        // A young, dusty system strikes more.
        let violent = Upheaval { impact_flux: 6.0, ..up };
        assert!(Impacts::generate(512, 256, 42, &violent).craters.len() > 4 * n);
    }

    #[test]
    fn a_crater_is_a_bowl_with_a_rim() {
        let up = Upheaval::earth();
        let c = Crater { id: 0, u: 0.0, v: 0.0, d_km: 1.2, age_myr: 0.05, wear: 0.0, kind: CraterKind::Simple, transition_km: up.transition_km() };
        let (floor, w0) = c.profile(0.0);
        let (rim, _) = c.profile(0.6);
        let (out, w2) = c.profile(1.5);
        assert!(floor < -150.0 && floor > -250.0, "floor {floor}");
        assert!(rim > 30.0 && rim < 60.0, "rim {rim}");
        assert!(out > 0.0 && out < rim, "ejecta {out}");
        assert_eq!(w0, 1.0);
        assert_eq!(w2, 0.0);
        // A complex crater has a peak above its floor.
        let big = Crater { d_km: 40.0, kind: CraterKind::Complex, ..c.clone() };
        let (centre, _) = big.profile(0.0);
        let (floor, _) = big.profile(0.35 * big.r_km());
        assert!(centre > floor + 100.0, "peak {centre} floor {floor}");
    }

    #[test]
    fn faults_keep_their_kind() {
        use crate::plates::{Vec2, PlateType};
        // Two plates side by side: moving apart, together and past each other.
        let (w, h) = (32, 16);
        let mut pm = Tilemap::new_with(w, h, PlateId(0));
        for y in 0..h { for x in 16..w { pm.set(x, y, PlateId(1)); } }
        let stress = Tilemap::new_with(w, h, 0.0f32);
        let plate = |id: u8, vx: f32, vy: f32| Plate { id: PlateId(id), plate_type: PlateType::Continental, velocity: Vec2::new(vx, vy), base_elevation: 0.0, color: [0; 3] };
        let apart = Faults::compute(&pm, &[plate(0, -1.0, 0.0), plate(1, 1.0, 0.0)], &stress, 1.0);
        assert_eq!(apart.kind[5 * w + 15], FaultKind::Normal);
        let together = Faults::compute(&pm, &[plate(0, 1.0, 0.0), plate(1, -1.0, 0.0)], &stress, 1.0);
        assert_eq!(together.kind[5 * w + 15], FaultKind::Thrust);
        let past = Faults::compute(&pm, &[plate(0, 0.0, 1.0), plate(1, 0.0, -1.0)], &stress, 1.0);
        assert_eq!(past.kind[5 * w + 15], FaultKind::StrikeSlip);
        // The strike runs north-south along the boundary.
        let s = past.tile(15, 5).strike;
        assert!((s.sin().abs() - 1.0).abs() < 0.05, "strike {s}");
        // Intensity falls off away from the boundary.
        assert!(past.intensity[5 * w + 15] > past.intensity[5 * w + 10]);
    }
}

/// Whether the stratovolcano at tile (x, y) has lost a flank (one in three; the region's
/// `landforms::mass` cuts it with the region's seed, the world's master seed).
pub fn flank_fell(width: usize, x: usize, y: usize, seed: u64) -> bool {
    let key = (y * width + x) as i64;
    crate::region::landforms::rock_unit(key, 0, seed ^ 0xF1A4) <= 0.34
}

// ---------------------------------------------------------------------------------------------
// The report (`--upheaval-report`).

/// What a world's violence is and where to look at it: the record's numbers, the largest
/// craters on land, the strongest fault zones of each kind on land and the provinces.
pub fn report(world: &crate::world::WorldData) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    let Some(up) = world.upheaval() else {
        return "No upheaval on this world (the Sun's: --upheaval or PLANET_UPHEAVAL=1 to ask for it)\n".into();
    };
    let _ = writeln!(out, "Upheaval: impact flux {:.2}x the Earth's, gravity {:.2} g (craters turn complex from {:.1} km), {:.2} Gyr old, interior heat {:.2}x",
        up.impact_flux, up.gravity, up.transition_km(), up.age_gyr, up.internal_heat);
    let land = |u: f32, v: f32| sample_world(&world.heightmap, u, v) > 0.0;
    if let Some(rec) = world.impacts() {
        let on_land: Vec<&Crater> = rec.craters.iter().filter(|c| land(c.u, c.v)).collect();
        let bins = [(1.0, 5.0), (5.0, 20.0), (20.0, 100.0), (100.0, 1e9)];
        let counts: Vec<String> = bins.iter().map(|&(a, b)| {
            let n = on_land.iter().filter(|c| c.d_km >= a && c.d_km < b).count();
            if b > 1e8 { format!("{n} over {a:.0} km") } else { format!("{n} of {a:.0}-{b:.0} km") }
        }).collect();
        let km = tile_km(world.width);
        let shaped = on_land.iter().filter(|c| c.d_km >= WORLD_SCALE_TILES * km).count();
        let _ = writeln!(out, "Craters: {} on record, {} on land ({}); {} shaped on the world map, the rest in the regions",
            rec.craters.len(), on_land.len(), counts.join(", "), shaped);
        let mut big = on_land.clone();
        big.sort_by(|a, b| b.d_km.partial_cmp(&a.d_km).unwrap());
        for c in big.iter().take(6) {
            let (x, y) = rec.tile_of(c);
            let _ = writeln!(out, "  {x},{y}: {:.1} km {:?}, {:.1} Myr old, {:.0}% worn", c.d_km, c.kind, c.age_myr, c.wear * 100.0);
        }
        let mut fresh: Vec<&&Crater> = on_land.iter().filter(|c| c.wear < 0.25).collect();
        fresh.sort_by(|a, b| b.d_km.partial_cmp(&a.d_km).unwrap());
        if let Some(c) = fresh.first() {
            let (x, y) = rec.tile_of(c);
            let _ = writeln!(out, "  freshest large: {x},{y}: {:.1} km, {:.2} Myr old", c.d_km, c.age_myr);
        }
    }
    let f = world.faults();
    for kind in [FaultKind::Normal, FaultKind::Thrust, FaultKind::StrikeSlip] {
        let mut tiles: Vec<(usize, f32)> = (0..world.width * world.height)
            .filter(|&k| f.kind[k] == kind && *world.heightmap.get(k % world.width, k / world.width) > 0.0)
            .map(|k| (k, f.intensity[k])).collect();
        let n = tiles.len();
        tiles.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        let top: Vec<String> = tiles.iter().take(3).map(|&(k, i)| format!("{},{} ({:.2})", k % world.width, k / world.width, i)).collect();
        let _ = writeln!(out, "{:?} faults: {} land tiles in their zones; strongest {}", kind, n, top.join(", "));
    }
    for p in world.provinces() {
        let (x, y) = (((p.u.round() as i64).rem_euclid(world.width as i64)), (p.v.round() as i64).clamp(0, world.height as i64 - 1));
        let _ = writeln!(out, "Flood basalt: centre {x},{y}, {:.0} km across, {:.0} Myr old, flows {:.0} m thick, {} land", p.r_km * 2.0, p.age_myr, p.flow_m,
            if land(p.u, p.v) { "on" } else { "off" });
    }
    let stratos = world.volcanoes.iter().filter(|v| v.volcano_type == VolcanoType::Stratovolcano).count();
    let _ = writeln!(out, "Volcanoes: {} ({} stratovolcanoes, about a third with a fallen flank), {} active", world.volcanoes.len(), stratos, world.volcanoes.iter().filter(|v| v.is_active).count());
    let cones: Vec<String> = world.volcanoes.iter().filter(|v| v.volcano_type == VolcanoType::Stratovolcano && *world.heightmap.get(v.x, v.y) > 0.0)
        .take(12).map(|v| format!("{},{}{}{}{}", v.x, v.y, if v.is_active { " active" } else { "" }, if *world.temperature.get(v.x, v.y) < 4.0 { " snowy" } else { "" },
            if flank_fell(world.width, v.x, v.y, world.seed()) { " (a fallen flank)" } else { "" })).collect();
    if !cones.is_empty() { let _ = writeln!(out, "  stratovolcanoes on land: {}", cones.join(", ")); }
    out
}
