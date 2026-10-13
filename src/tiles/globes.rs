//! The worlds of a star system as globes (Tab on the heavens card, `--worlds-snapshot FILE`).
//!
//! Every rocky neighbour is made by the world's own pipeline (`terrain::generate_terrain`:
//! plates, crust, climate under its own starlight, tilt, spin and air, landscape evolution) on a
//! small map, then coloured by what it is: molten lowlands and glowing seams on a lava world,
//! grey maria and craters on bare rock, rust and polar caps on a cold desert, a hot orange
//! surface under cloud on a hothouse, a cracked shell on an ice world, seas and bare rock on a
//! lifeless super-earth. Giants are banded by their temperature class (Sudarsky) with a storm.
//! The world of the map is its own biome map. Each map is drawn on an orthographic globe lit
//! from the star on the left: the far side hatched in ink, the axis tilted, rings in front of
//! and behind the planet, the air as a haze on the limb.

use std::error::Error;
use std::f32::consts::{PI, TAU};

use noise::{NoiseFn, Perlin};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rayon::prelude::*;

use crate::astro::{Planet, PlanetKind, StarSystem};
use crate::climate::{AirForcing, ClimateConfig, PlanetForcing, RainfallLevel};
use crate::plates::WorldStyle;
use crate::seeds::WorldSeeds;
use crate::terrain::{generate_terrain, TerrainConfig};
use crate::world::WorldData;

use super::fonts::{self, Face};
use super::ui::*;

/// Map size the neighbours are made at (the pipeline takes ~0.1 s a world here).
const MAP_W: usize = 160;
const MAP_H: usize = 80;

/// A planet's face as an equirectangular map (row 0 at the north pole) and how it looks.
#[derive(Clone)]
pub struct Globe {
    pub name: String,
    pub kind: PlanetKind,
    pub w: usize,
    pub h: usize,
    pub colour: Vec<[f32; 3]>,
    /// Its own light (lava), 0..1: it shows on the night side too.
    pub glow: Vec<f32>,
    /// The air's colour on the limb.
    pub air: Option<[f32; 3]>,
    pub rings: bool,
    pub tilt_deg: f32,
    /// Turns a minute as shown (its spin, slowed for the eye; 0 for a world held to its star).
    pub turns_per_min: f32,
    /// The longitude that faces the star at rest (radians).
    pub facing: f32,
    pub radius_earth: f32,
    /// What the pipeline made of it, in a few words.
    pub note: String,
}

fn fnv(seed: u64, s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325u64 ^ seed, |a, b| (a ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3))
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Latitude and longitude (radians) of a map cell's centre.
fn cell_ll(x: usize, y: usize, w: usize, h: usize) -> (f32, f32) {
    (PI * 0.5 - (y as f32 + 0.5) / h as f32 * PI, (x as f32 + 0.5) / w as f32 * TAU)
}

/// Unit vector of a latitude and longitude.
fn unit(lat: f32, lon: f32) -> [f32; 3] { [lat.cos() * lon.sin(), lat.sin(), lat.cos() * lon.cos()] }

/// Craters on an airless face: (centre, angular radius) by a power law of sizes.
fn craters(rng: &mut impl Rng, n: usize) -> Vec<([f32; 3], f32)> {
    (0..n).map(|_| {
        let z: f32 = rng.gen_range(-1.0..1.0);
        let lon = rng.gen_range(0.0..TAU);
        let lat = z.asin();
        let size = 0.02 * (1.0 - rng.gen::<f32>()).powf(-0.6);
        (unit(lat, lon), size.min(0.35))
    }).collect()
}

/// A crater's mark at a point: darker inside, a bright rim (multiplier on brightness).
fn crater_shade(p: [f32; 3], list: &[([f32; 3], f32)]) -> f32 {
    let mut m = 1.0;
    for (c, r) in list {
        let d = (p[0] * c[0] + p[1] * c[1] + p[2] * c[2]).clamp(-1.0, 1.0).acos() / r;
        if d < 0.82 { m *= 0.86 + 0.06 * d; } else if d < 1.0 { m *= 1.16; } else if d < 1.6 { m *= 1.0 + 0.05 * (1.6 - d); }
    }
    m
}

/// A rocky neighbour made by the terrain pipeline under its own sky.
fn rocky(sys: &StarSystem, p: &Planet, seed: u64, rng: &mut ChaCha8Rng) -> Globe {
    let host = sys.host();
    let dry = matches!(p.kind, PlanetKind::Barren | PlanetKind::Lava | PlanetKind::ColdDesert | PlanetKind::Hothouse);
    let style = match p.kind { PlanetKind::Ice => WorldStyle::Waterworld, PlanetKind::SuperEarth => WorldStyle::Earthlike, _ => WorldStyle::Continental };
    let tilt = rng.gen_range(0.0..35.0f32);
    let mut cfg = TerrainConfig::new(MAP_W, MAP_H, style);
    cfg.tectonic_myr = 150.0;
    cfg.land_scale = if dry { 1.5 } else if p.kind == PlanetKind::SuperEarth { rng.gen_range(0.6..1.4) } else { 1.0 };
    cfg.hotspots = match p.kind { PlanetKind::Lava => 20, PlanetKind::Barren => 2, PlanetKind::ColdDesert => 5, PlanetKind::Hothouse => 12, _ => 8 };
    let air = match p.kind {
        PlanetKind::Hothouse => AirForcing::Fixed { co2_bar: 8.0, dry_bar: 3.0 },
        PlanetKind::ColdDesert => AirForcing::Fixed { co2_bar: 0.006, dry_bar: 0.05 },
        PlanetKind::SuperEarth => AirForcing::Fixed { co2_bar: 0.01, dry_bar: 1.5 },
        _ => AirForcing::Fixed { co2_bar: 1e-5, dry_bar: 0.05 },
    };
    cfg.climate = ClimateConfig {
        rainfall: if dry { RainfallLevel::Arid } else { RainfallLevel::Normal },
        axial_tilt_deg: tilt,
        solar_constant: crate::climate::ebm::SOLAR_CONSTANT * host.luminosity_lsun / (p.semi_major_au * p.semi_major_au),
        planet: PlanetForcing {
            legacy: false,
            eccentricity: p.eccentricity,
            rotation: rng.gen_range(0.05..3.0),
            year: p.period_days / crate::astro::orbit::EARTH_YEAR_DAYS,
            teff: host.teff_k,
            air,
            ..PlanetForcing::default()
        },
        ..ClimateConfig::default()
    };
    let t = generate_terrain(&cfg, &WorldSeeds::builder(seed).build(), &mut |_| true).expect("terrain generation runs to the end");
    let (w, h) = (MAP_W, MAP_H);
    let crater_list = match p.kind {
        PlanetKind::Barren => craters(rng, 260),
        PlanetKind::ColdDesert => craters(rng, 110),
        PlanetKind::Ice => craters(rng, 40),
        PlanetKind::Lava | PlanetKind::Hothouse | PlanetKind::SuperEarth => Vec::new(),
        _ => Vec::new(),
    };
    let perlin = Perlin::new((seed & 0xFFFF_FFFF) as u32);
    let max_flow = t.flow_accumulation.iter().map(|(_, _, &f)| f).fold(1.0f32, f32::max).ln();
    let mut colour = vec![[0.0f32; 3]; w * h];
    let mut glow = vec![0.0f32; w * h];
    let (mut wet, mut ice_cells, mut molten) = (0usize, 0usize, 0usize);
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let e = *t.heightmap.get(x, y);
            let temp = *t.climate.mean_temperature.get(x, y);
            let stress = t.stress_map.get(x, y).abs();
            let (lat, lon) = cell_ll(x, y, w, h);
            let pt = unit(lat, lon);
            let n = perlin.get([pt[0] as f64 * 6.0, pt[1] as f64 * 6.0, pt[2] as f64 * 6.0]) as f32;
            let relief = smooth(-2500.0, 3500.0, e);
            let mut c = match p.kind {
                PlanetKind::Barren => lerp3([74.0, 72.0, 70.0], [158.0, 152.0, 144.0], relief + 0.1 * n),
                PlanetKind::Lava => {
                    if e < 300.0 {
                        molten += 1;
                        // Rafts of cooling crust drift on the molten seas.
                        let skin = perlin.get([pt[0] as f64 * 7.0 + 3.0, pt[1] as f64 * 7.0, pt[2] as f64 * 7.0]) as f32;
                        if skin > 0.25 {
                            glow[i] = 0.25;
                            lerp3([70.0, 38.0, 30.0], [110.0, 52.0, 34.0], skin)
                        } else {
                            glow[i] = (0.7 + 0.3 * n).clamp(0.4, 1.0);
                            lerp3([196.0, 66.0, 22.0], [252.0, 156.0, 56.0], 0.5 + 0.5 * n)
                        }
                    } else {
                        if stress > 0.3 { glow[i] = 0.6 * smooth(0.3, 0.7, stress); }
                        lerp3([40.0, 33.0, 31.0], [96.0, 84.0, 78.0], relief + 0.15 * n)
                    }
                }
                PlanetKind::ColdDesert => {
                    let mut c = lerp3([110.0, 56.0, 38.0], [206.0, 136.0, 88.0], relief + 0.12 * n);
                    let flow = t.flow_accumulation.get(x, y).max(1.0).ln() / max_flow;
                    if flow > 0.55 { c = lerp3(c, [86.0, 46.0, 34.0], (flow - 0.55) * 1.6); }
                    if lat.abs() > 1.27 - 0.06 * n || e > 7000.0 { ice_cells += 1; c = [236.0, 232.0, 226.0]; }
                    c
                }
                PlanetKind::Hothouse => lerp3([132.0, 78.0, 44.0], [216.0, 160.0, 98.0], relief + 0.1 * n),
                PlanetKind::Ice => {
                    ice_cells += 1;
                    let mut c = lerp3([196.0, 214.0, 224.0], [238.0, 240.0, 236.0], relief + 0.2 * n);
                    if stress > 0.22 { c = lerp3(c, [150.0, 104.0, 84.0], smooth(0.22, 0.5, stress)); }
                    c
                }
                _ => {
                    // A lifeless world with seas: water, sea ice, bare rock, ice caps.
                    if e <= 0.0 {
                        if temp < -2.0 { ice_cells += 1; [224.0, 232.0, 236.0] } else { wet += 1; lerp3([34.0, 62.0, 104.0], [76.0, 118.0, 152.0], smooth(-4000.0, 0.0, e)) }
                    } else if temp < -8.0 { ice_cells += 1; [236.0, 238.0, 236.0] } else {
                        lerp3([118.0, 108.0, 94.0], [174.0, 164.0, 144.0], relief + 0.12 * n)
                    }
                }
            };
            if !crater_list.is_empty() {
                let m = crater_shade(pt, &crater_list);
                c = [c[0] * m, c[1] * m, c[2] * m];
            }
            colour[i] = c;
        }
    }
    // A hothouse's cloud deck over its surface.
    if p.kind == PlanetKind::Hothouse {
        for y in 0..h {
            for x in 0..w {
                let (lat, lon) = cell_ll(x, y, w, h);
                let pt = unit(lat, lon + 0.4 * lat.sin());
                let n = perlin.get([pt[0] as f64 * 3.0 + 9.0, pt[1] as f64 * 9.0, pt[2] as f64 * 3.0]) as f32;
                let i = y * w + x;
                colour[i] = lerp3(colour[i], [234.0, 216.0, 164.0], 0.42 + 0.22 * n);
            }
        }
    }
    let cells = (w * h) as f32;
    let note = match p.kind {
        PlanetKind::Lava => format!("molten seas over {:.0}% of it", 100.0 * molten as f32 / cells),
        PlanetKind::ColdDesert => format!("dry valleys, ice on {:.0}%", 100.0 * ice_cells as f32 / cells),
        PlanetKind::Barren => format!("{} craters on bare rock", crater_list.len()),
        PlanetKind::Hothouse => "a hot surface under unbroken cloud".to_string(),
        PlanetKind::Ice => "a shell of ice cracked along its plates".to_string(),
        _ => format!("seas over {:.0}%, ice on {:.0}%, no life", 100.0 * wet as f32 / cells, 100.0 * ice_cells as f32 / cells),
    };
    Globe {
        name: p.name.clone(), kind: p.kind, w, h, colour, glow,
        air: match p.kind { PlanetKind::Hothouse => Some([236.0, 206.0, 140.0]), PlanetKind::ColdDesert => Some([214.0, 160.0, 120.0]), PlanetKind::SuperEarth => Some([150.0, 186.0, 226.0]), _ => None },
        rings: false, tilt_deg: tilt, turns_per_min: rng.gen_range(0.6..2.0), facing: 0.0, radius_earth: p.radius_earth, note,
    }
}

/// A giant's bands, coloured by its temperature (Sudarsky's classes: ammonia clouds, water
/// clouds, clear blue skies, then dark and glowing).
fn banded(p: &Planet, seed: u64, rng: &mut ChaCha8Rng) -> Globe {
    let (w, h) = (MAP_W, MAP_H);
    let t = p.eq_temp_k;
    let (palette, note): (Vec<[f32; 3]>, &str) = match p.kind {
        PlanetKind::GasGiant if t < 160.0 => (vec![[220.0, 196.0, 156.0], [178.0, 128.0, 88.0], [240.0, 228.0, 204.0], [152.0, 104.0, 74.0], [206.0, 170.0, 126.0]], "ammonia clouds in bands"),
        PlanetKind::GasGiant if t < 320.0 => (vec![[236.0, 234.0, 226.0], [204.0, 204.0, 198.0], [246.0, 244.0, 238.0], [214.0, 206.0, 190.0]], "white water clouds"),
        PlanetKind::GasGiant if t < 800.0 => (vec![[104.0, 146.0, 200.0], [80.0, 118.0, 178.0], [136.0, 172.0, 214.0]], "a clear blue sky of hydrogen"),
        PlanetKind::GasGiant => (vec![[62.0, 60.0, 82.0], [46.0, 48.0, 66.0], [112.0, 76.0, 66.0]], "dark, burning on its night side"),
        PlanetKind::IceGiant if t < 70.0 => (vec![[78.0, 116.0, 198.0], [62.0, 98.0, 178.0], [104.0, 144.0, 214.0]], "deep blue methane haze"),
        PlanetKind::IceGiant => (vec![[150.0, 200.0, 214.0], [132.0, 186.0, 204.0], [172.0, 214.0, 224.0]], "pale methane haze"),
        _ => (vec![[170.0, 198.0, 196.0], [150.0, 184.0, 186.0], [188.0, 210.0, 204.0]], "a deep hazy air"),
    };
    let perlin = Perlin::new((seed & 0xFFFF_FFFF) as u32);
    let bands = rng.gen_range(5.0..12.0f32);
    let calm = if p.kind == PlanetKind::GasGiant { 1.0 } else { 0.35 };
    let storm = (rng.gen_range(-0.6..0.6f32), rng.gen_range(0.0..TAU), rng.gen_range(0.08..0.16f32));
    let storm_col = if t < 160.0 { [196.0, 108.0, 76.0] } else { [44.0, 62.0, 126.0] };
    let mut colour = vec![[0.0f32; 3]; w * h];
    let mut glow = vec![0.0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let (lat, lon) = cell_ll(x, y, w, h);
            let pt = unit(lat, lon);
            let turb = perlin.get([pt[0] as f64 * 4.0, pt[1] as f64 * 12.0, pt[2] as f64 * 4.0]) as f32;
            let fine = perlin.get([pt[0] as f64 * 11.0 + 5.0, pt[1] as f64 * 30.0, pt[2] as f64 * 11.0]) as f32;
            let b = (lat / PI + 0.5) * bands + calm * (0.35 * turb + 0.12 * fine) * lat.cos();
            let k = b.floor() as i64;
            let f = smooth(0.0, 1.0, b - b.floor());
            let c0 = palette[k.rem_euclid(palette.len() as i64) as usize];
            let c1 = palette[(k + 1).rem_euclid(palette.len() as i64) as usize];
            let mut c = lerp3(c0, c1, f);
            // A great storm: an oval swirling in its band.
            let (dlat, dlon) = ((lat - storm.0) / (storm.2 * 0.55), ((lon - storm.1 + PI).rem_euclid(TAU) - PI) / storm.2);
            let d = (dlat * dlat + dlon * dlon).sqrt();
            if d < 1.0 && p.kind != PlanetKind::MiniNeptune { c = lerp3(c, storm_col, (1.0 - d) * 1.4); }
            colour[y * w + x] = c;
            if p.kind == PlanetKind::GasGiant && t > 800.0 { glow[y * w + x] = 0.25 + 0.15 * turb; }
        }
    }
    Globe {
        name: p.name.clone(), kind: p.kind, w, h, colour, glow, air: Some(lerp3(palette[0], [255.0; 3], 0.4)), rings: p.rings,
        tilt_deg: rng.gen_range(0.0..30.0), turns_per_min: rng.gen_range(2.0..4.0), facing: 0.0, radius_earth: p.radius_earth, note: note.to_string(),
    }
}

/// A neighbour as a globe (the pipeline for rock, bands for giants).
pub fn neighbour_globe(sys: &StarSystem, i: usize, world_seed: u64) -> Globe {
    let p = &sys.planets[i];
    let seed = fnv(world_seed, &p.name);
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    match p.kind {
        PlanetKind::GasGiant | PlanetKind::IceGiant | PlanetKind::MiniNeptune => banded(p, seed, &mut rng),
        _ => rocky(sys, p, seed, &mut rng),
    }
}

/// The world of the map as a globe: its biomes, washed toward the parchment's palette.
pub fn home_globe(world: &WorldData, sys: &StarSystem) -> Globe {
    let (w, h) = (world.width, world.height);
    let mut colour = vec![[0.0f32; 3]; w * h];
    for y in 0..h {
        for x in 0..w {
            let biome = *world.biomes.get(x, y);
            use crate::biomes::ExtendedBiome as B;
            // Seas lighter than the biome table's navy, toward the map's sea ink.
            let (r, g, b) = match biome { B::DeepOcean => (56, 88, 122), B::Ocean => (72, 108, 142), B::CoastalWater => (100, 140, 166), other => other.color() };
            let e = *world.heightmap.get(x, y);
            let shade = if e > 0.0 { 1.0 - 0.18 * smooth(500.0, 4000.0, e) } else { 1.0 };
            let c = lerp3([r as f32 * shade, g as f32 * shade, b as f32 * shade], [232.0, 222.0, 196.0], 0.18);
            // What grows takes the star's leaves, as on the map.
            let name = format!("{:?}", biome);
            let alive = !["Dead", "Petrified", "Crystal", "Mushroom"].iter().any(|k| name.contains(k));
            let woods = alive && ["Forest", "Jungle", "Grove", "Mangrove"].iter().any(|k| name.contains(k));
            let grass = alive && ["Grass", "Savanna", "Shrub", "Meadow", "Swamp", "Marsh", "Foothills", "Paramo"].iter().any(|k| name.contains(k));
            colour[y * w + x] = if woods { super::ink::leafy(c) } else if grass { super::ink::leafy_by(c, super::ink::GRASS_LEAVES) } else { c };
        }
    }
    let spin = &sys.world.spin;
    let held = spin.locked || spin.resonance;
    Globe {
        name: sys.world.name.clone(), kind: PlanetKind::Home, w, h, colour, glow: vec![0.0; w * h], air: Some([150.0, 186.0, 226.0]), rings: false,
        tilt_deg: spin.obliquity_deg, turns_per_min: if held { 0.0 } else { 1.0 }, facing: spin.substellar_lon_deg.to_radians(), radius_earth: 1.0,
        note: String::new(),
    }
}

/// Every world of the system, from the star outward (the map's world where it circles).
pub fn all_globes(world: &WorldData, sys: &StarSystem) -> Vec<Globe> {
    let seed = world.seed();
    let idx: Vec<usize> = (0..sys.planets.len()).collect();
    let mut out: Vec<(usize, Globe)> = idx.par_iter().map(|&i| {
        let g = if sys.planets[i].kind == PlanetKind::Home { home_globe(world, sys) } else { neighbour_globe(sys, i, seed) };
        (i, g)
    }).collect();
    out.sort_by_key(|(i, _)| *i);
    out.into_iter().map(|(_, g)| g).collect()
}

/// Sample a map bilinearly at a latitude and longitude.
fn sample(g: &Globe, lat: f32, lon: f32) -> ([f32; 3], f32) {
    let fx = (lon / TAU).rem_euclid(1.0) * g.w as f32 - 0.5;
    let fy = ((0.5 - lat / PI) * g.h as f32 - 0.5).clamp(0.0, (g.h - 1) as f32);
    let (x0, y0) = (fx.floor(), fy.floor());
    let (tx, ty) = (fx - x0, fy - y0);
    let xi = |x: f32| (x as i64).rem_euclid(g.w as i64) as usize;
    let (xa, xb) = (xi(x0), xi(x0 + 1.0));
    let (ya, yb) = (y0 as usize, (y0 as usize + 1).min(g.h - 1));
    let at = |x: usize, y: usize| (g.colour[y * g.w + x], g.glow[y * g.w + x]);
    let (c00, g00) = at(xa, ya);
    let (c10, g10) = at(xb, ya);
    let (c01, g01) = at(xa, yb);
    let (c11, g11) = at(xb, yb);
    let top = lerp3(c00, c10, tx);
    let bot = lerp3(c01, c11, tx);
    (lerp3(top, bot, ty), (g00 * (1.0 - tx) + g10 * tx) * (1.0 - ty) + (g01 * (1.0 - tx) + g11 * tx) * ty)
}

fn pack(c: [f32; 3]) -> u32 { super::ink::pack(c) }
fn unpack(c: u32) -> [f32; 3] { [((c >> 16) & 0xFF) as f32, ((c >> 8) & 0xFF) as f32, (c & 0xFF) as f32] }

/// Where the star shines from, in view space (left, a little up and toward the viewer).
const LIGHT: [f32; 3] = [-0.8219, 0.1804, 0.5513];

/// The turn that puts a held world's star-facing longitude under the light.
fn held_spin(g: &Globe) -> f32 { g.facing - LIGHT[0].atan2(LIGHT[2]) }

/// Draw a globe of radius r centred at (cx, cy), turned by `spin` radians, lit from the left.
pub fn draw_globe(g: &Globe, buf: &mut [u32], w: usize, h: usize, cx: f32, cy: f32, r: f32, spin: f32) {
    let tilt = g.tilt_deg.to_radians();
    let incl = 0.32f32;
    // View -> planet: undo the view's inclination, then the axis tilt (toward the star).
    let (si, ci) = incl.sin_cos();
    let (st, ct) = tilt.sin_cos();
    let to_planet = |v: [f32; 3]| {
        // Rx(-incl)
        let a = [v[0], v[1] * ci + v[2] * si, -v[1] * si + v[2] * ci];
        // Rz(-tilt)
        [a[0] * ct + a[1] * st, -a[0] * st + a[1] * ct, a[2]]
    };
    let light = LIGHT;
    let ring_in = 1.3f32;
    let ring_out = 2.25f32;
    let reach = if g.rings { ring_out + 0.05 } else { 1.08 };
    let x0 = (cx - r * reach).floor().max(0.0) as usize;
    let x1 = ((cx + r * reach).ceil() as usize).min(w.saturating_sub(1));
    let y0 = (cy - r * reach).floor().max(0.0) as usize;
    let y1 = ((cy + r * reach).ceil() as usize).min(h.saturating_sub(1));
    if x0 >= x1 || y0 >= y1 { return; }
    let rows: Vec<(usize, Vec<(usize, u32)>)> = (y0..=y1).into_par_iter().map(|py| {
        let mut out = Vec::new();
        for px in x0..=x1 {
            let u = (px as f32 + 0.5 - cx) / r;
            let v = -(py as f32 + 0.5 - cy) / r;
            let d2 = u * u + v * v;
            let k = py * w + px;
            let bg = unpack(buf[k]);
            // Rings: where the line of sight meets the equatorial plane.
            let mut ring: Option<([f32; 3], f32, f32)> = None;
            if g.rings {
                // Planet-frame y along the view ray (u, v, t): linear in t.
                let p0 = to_planet([u, v, 0.0]);
                let p1 = to_planet([u, v, 1.0]);
                let dy = p1[1] - p0[1];
                if dy.abs() > 1e-4 {
                    let t = -p0[1] / dy;
                    let q = [p0[0] + (p1[0] - p0[0]) * t, 0.0, p0[2] + (p1[2] - p0[2]) * t];
                    let rho = (q[0] * q[0] + q[2] * q[2]).sqrt();
                    if rho > ring_in && rho < ring_out {
                        let band = (rho * 23.0).sin() * 0.5 + 0.5;
                        let gap = if (rho - 1.85).abs() < 0.04 { 0.15 } else { 1.0 };
                        let alpha = (0.45 + 0.4 * band) * gap * smooth(ring_in, ring_in + 0.08, rho) * (1.0 - smooth(ring_out - 0.1, ring_out, rho));
                        // In the planet's shadow (behind it, away from the star)?
                        let world_pt = [u, v, t];
                        let along = world_pt[0] * light[0] + world_pt[1] * light[1] + world_pt[2] * light[2];
                        let perp2 = (u * u + v * v + t * t) - along * along;
                        let lit = if along < 0.0 && perp2 < 1.0 { 0.35 } else { 1.0 };
                        ring = Some(([214.0 * lit, 200.0 * lit, 178.0 * lit], alpha, t));
                    }
                }
            }
            let mut col: Option<[f32; 3]> = None;
            if d2 <= 1.0 {
                let z = (1.0 - d2).sqrt();
                let n = [u, v, z];
                let q = to_planet(n);
                let lat = q[1].clamp(-1.0, 1.0).asin();
                let lon = q[0].atan2(q[2]) + spin;
                let (c, glow) = sample(g, lat, lon);
                let dl = n[0] * light[0] + n[1] * light[1] + n[2] * light[2];
                let day = smooth(-0.08, 0.14, dl);
                let lambert = 0.38 + 0.62 * dl.max(0.0);
                let day_c = [c[0] * lambert, c[1] * lambert, c[2] * lambert];
                let mut night = lerp3([c[0] * 0.3, c[1] * 0.3, c[2] * 0.3], unpack(INK), 0.35);
                if (px as i64 - py as i64).rem_euclid(3) == 0 { night = lerp3(night, unpack(INK), 0.45); }
                let mut c = lerp3(night, day_c, day);
                // Its own light: faint by day, the only light by night.
                if glow > 0.0 { c = lerp3(c, lerp3([210.0, 70.0, 26.0], [255.0, 140.0, 48.0], day), glow * (0.15 + 0.5 * (1.0 - day))); }
                if let Some(air) = g.air {
                    let rim = (1.0 - z).powi(3);
                    c = lerp3(c, air, rim * 0.7 * (0.3 + 0.7 * day));
                }
                // Ink rim.
                if d2 > (1.0 - 1.4 / r).powi(2) { c = lerp3(c, unpack(INK), 0.85); }
                col = Some(c);
                // The ring in front of the disc covers it; behind it, the disc covers the ring.
                if let Some((rc, a, t)) = ring { if t > z { col = Some(lerp3(c, rc, a)); } }
            } else {
                let rad = d2.sqrt();
                let mut c = bg;
                let mut touched = false;
                if let Some(air) = g.air {
                    if rad < 1.06 {
                        c = lerp3(c, air, (1.0 - (rad - 1.0) / 0.06) * 0.45);
                        touched = true;
                    }
                }
                if let Some((rc, a, _)) = ring { c = lerp3(c, rc, a); touched = true; }
                if touched { col = Some(c); }
            }
            if let Some(c) = col { out.push((k, pack(c))); }
        }
        (py, out)
    }).collect();
    for (_, px) in rows { for (k, c) in px { buf[k] = c; } }
}

/// The view: the star's limb at the left, every world in a row from it, and the chosen one
/// large below with what it is. `t` is seconds (the globes turn), `sel` the chosen world.
/// `globes` may be shorter than the system's planets while they are still being made.
pub fn draw_worlds(sys: &StarSystem, globes: &[Globe], sel: usize, t: f32, buf: &mut [u32], w: usize, h: usize) {
    let cw = 1180.min(w.saturating_sub(16));
    let ch = 780.min(h.saturating_sub(16));
    if cw < 560 || ch < 420 { return; }
    let c = Rect { x: (w - cw) / 2, y: (h - ch) / 2, w: cw, h: ch };
    card(buf, w, c);
    let x0 = c.x as f32 + 28.0;
    let mut y = c.y as f32 + 18.0;
    let title = match &sys.companion { Some(p) if p.circumbinary => format!("The Worlds of {} and {}", sys.star.name, p.star.name), _ => format!("The Worlds of {}", sys.star.name) };
    fonts::draw(buf, w, h, x0, y, &title, Face::SmallCaps, 30.0, 1.2, RUBRIC, None);
    y += 38.0;
    fonts::draw(buf, w, h, x0, y, "each world made by the same hand as this one: its plates, its air and its own sunlight", Face::Italic, 16.0, 0.0, INK_FADED, None);
    y += 26.0;
    hline(buf, w, x0 as usize, c.x + c.w - 28, y as usize, INK);
    y += 8.0;

    // The star's limb along the left edge, in its colour.
    let row_cy = y + 112.0;
    let star_col = sys.star.paint_rgb();
    let (sr, scx) = (150.0f32, c.x as f32 - 70.0);
    for py in (row_cy - 120.0) as i64..(row_cy + 120.0) as i64 {
        for px in c.x as i64..(c.x as f32 + 110.0) as i64 {
            let d = (((px as f32 - scx).powi(2) + (py as f32 - row_cy).powi(2)).sqrt() - sr) / 26.0;
            if d < 0.0 { blend_px(buf, w, h, px, py, pack(lerp3(star_col, [255.0, 250.0, 232.0], (-d * 0.08).min(0.4))), 1.0); }
            else if d < 1.0 { blend_px(buf, w, h, px, py, pack(star_col), (1.0 - d).powi(2) * 0.6); }
        }
    }
    if let Some(p) = sys.companion.as_ref().filter(|p| p.circumbinary) {
        let col = pack(p.star.paint_rgb());
        let (cr, ccx, ccy) = (34.0f32, c.x as f32 + 70.0, row_cy - 70.0);
        for py in (ccy - cr - 10.0) as i64..(ccy + cr + 10.0) as i64 {
            for px in (ccx - cr - 10.0) as i64..(ccx + cr + 10.0) as i64 {
                let d = ((px as f32 - ccx).powi(2) + (py as f32 - ccy).powi(2)).sqrt();
                if d < cr { blend_px(buf, w, h, px, py, col, 1.0); } else if d < cr + 10.0 { blend_px(buf, w, h, px, py, col, (1.0 - (d - cr) / 10.0) * 0.5); }
            }
        }
    }

    // The row: sizes as the square root of the radius (Jupiter would dwarf the rest).
    let n = sys.planets.len();
    let base: Vec<f32> = sys.planets.iter().map(|p| (26.0 * p.radius_earth.sqrt()).clamp(9.0, 64.0)).collect();
    // Half the room each takes across: its rings reach 2.25 radii.
    let reach: Vec<f32> = sys.planets.iter().map(|p| if p.rings { 2.3 } else { 1.0 }).collect();
    let avail = c.w as f32 - 150.0 - 40.0;
    let need: f32 = (0..n).map(|i| base[i] * 2.0 * reach[i]).sum::<f32>() + 18.0 * n as f32;
    let fit = (avail / need).min(1.0);
    let mut x = c.x as f32 + 140.0;
    let mut centres = Vec::new();
    for i in 0..n {
        let r = base[i] * fit;
        let half = r * reach[i];
        x += half + 9.0 * fit.max(0.6);
        centres.push((x, r));
        x += half + 9.0 * fit.max(0.6);
    }
    for (i, p) in sys.planets.iter().enumerate() {
        let (gx, r) = centres[i];
        let home = p.kind == PlanetKind::Home;
        match globes.get(i) {
            Some(g) => {
                let spin = if g.turns_per_min == 0.0 { held_spin(g) } else { t * g.turns_per_min / 60.0 * TAU };
                draw_globe(g, buf, w, h, gx, row_cy, r, spin);
            }
            None => {
                for py in (row_cy - r) as i64..=(row_cy + r) as i64 {
                    for px in (gx - r) as i64..=(gx + r) as i64 {
                        let d = ((px as f32 - gx).powi(2) + (py as f32 - row_cy).powi(2)).sqrt();
                        if d <= r { blend_px(buf, w, h, px, py, if d > r - 1.2 { INK } else { PAPER_SHADE }, 1.0); }
                    }
                }
            }
        }
        if i == sel {
            for k in 0..60 {
                let a = k as f32 / 60.0 * TAU;
                if k % 2 == 0 { blend_px(buf, w, h, (gx + a.cos() * (r + 6.0)) as i64, (row_cy + a.sin() * (r + 6.0)) as i64, RUBRIC, 0.9); }
            }
        }
        let face = if home { Face::SmallCaps } else { Face::Italic };
        let size = if home { 14.0 } else { 12.5 };
        let lw = fonts::width(&p.name, face, size, 0.0);
        let ly = row_cy + 72.0 + if i % 2 == 1 && fit < 0.85 { 15.0 } else { 0.0 };
        fonts::draw(buf, w, h, gx - lw / 2.0, ly, &p.name, face, size, 0.0, if home { RUBRIC } else { INK }, None);
    }

    // The chosen world, large, and its account.
    let sel = sel.min(n.saturating_sub(1));
    let p = &sys.planets[sel];
    let (top, bottom) = (row_cy + 100.0, (c.y + c.h) as f32 - 46.0);
    let big_r = ((bottom - top) * 0.5 - 6.0).clamp(60.0, 200.0) / if sys.planets[sel.min(n.saturating_sub(1))].rings { 2.0 } else { 1.0 };
    let (bx, by) = (c.x as f32 + 60.0 + 200.0, (top + bottom) * 0.5);
    let home = p.kind == PlanetKind::Home;
    match globes.get(sel) {
        Some(g) => {
            let spin = if g.turns_per_min == 0.0 { held_spin(g) } else { t * g.turns_per_min / 60.0 * TAU };
            draw_globe(g, buf, w, h, bx, by, big_r, spin);
        }
        None => {
            let msg = "being made...";
            fonts::draw(buf, w, h, bx - 40.0, by, msg, Face::Italic, 16.0, 0.0, INK_FADED, None);
        }
    }
    let tx = bx + 200.0 + 60.0;
    let mut ty = row_cy + 130.0;
    let tw = (c.x + c.w) as f32 - 36.0 - tx;
    fonts::draw(buf, w, h, tx, ty, &p.name, Face::SmallCaps, 24.0, 1.0, RUBRIC, None);
    ty += 32.0;
    let kind = if home { "the world of this map".to_string() } else { p.kind.word().to_string() };
    let mut lines = vec![
        format!("{}{}.", crate::astro::sky::capitalise_word(&kind), if p.rings { ", ringed" } else { "" }),
        format!("{} Earth masses, {} Earth radii, {} AU from its sun; a year of {} days; {:.0} K at equilibrium.",
            crate::astro::sky::round_sig(p.mass_earth), crate::astro::sky::round_sig(p.radius_earth), crate::astro::sky::round_sig(p.semi_major_au), crate::astro::sky::round_sig(p.period_days), p.eq_temp_k),
    ];
    if let Some(g) = globes.get(sel) { if !g.note.is_empty() { lines.push(format!("{}.", crate::astro::sky::capitalise_word(&g.note))); } }
    if !home {
        let (mag, arcsec) = sys.planet_view(sel);
        lines.push(if arcsec > 60.0 { format!("From {} at its nearest: a disc {:.0}' across, magnitude {:.1}.", sys.world.name, arcsec / 60.0, mag) } else { format!("From {} at its nearest: magnitude {:.1}.", sys.world.name, mag) });
        if p.moons > 0 { lines.push(format!("{} moon{}.", p.moons, if p.moons == 1 { "" } else { "s" })); }
        if p.kind != PlanetKind::GasGiant && p.kind != PlanetKind::IceGiant && p.kind != PlanetKind::MiniNeptune {
            let s = sys.host().luminosity_lsun / (p.semi_major_au * p.semi_major_au);
            lines.push(format!("Made as this world was, from its own plates and climate: starlight {}x the Earth's, its air {}.",
                crate::astro::sky::round_sig(s), match p.kind { PlanetKind::Hothouse => "a crushing blanket of CO2", PlanetKind::ColdDesert => "thin and dry", PlanetKind::SuperEarth => "thick", _ => "all but gone" }));
        }
    } else {
        lines.push(format!("{} moon{}; tilt {:.0}°.", sys.world.moons.len(), if sys.world.moons.len() == 1 { "" } else { "s" }, sys.world.spin.obliquity_deg));
        if let Some(cl) = &sys.climate {
            lines.push(format!("Mean {:.0} °C; {:.0}% under ice all year; air of {:.2} bar.", cl.mean_c, cl.ice_share * 100.0, cl.pressure_bar));
        }
        let day = sys.world.spin.solar_day_hours(&sys.world.orbit).map(|d| format!("A day of {:.1} hours.", d)).unwrap_or_else(|| "One face always to its sun: the globe is shown with that face lit.".into());
        lines.push(day);
    }
    for l in &lines {
        for wl in fonts::wrap(l, Face::Roman, 15.0, tw) {
            fonts::draw(buf, w, h, tx, ty, &wl, Face::Roman, 15.0, 0.0, INK, None);
            ty += 19.0;
        }
        ty += 4.0;
    }
    if globes.len() < n {
        fonts::draw(buf, w, h, tx, ty + 10.0, &format!("The neighbours are being made: {} of {}.", globes.len(), n), Face::Italic, 14.0, 0.0, INK_FADED, None);
    }
    let keys = "Left/Right choose a world    Tab the orrery    H or Esc back to the map";
    let kw = fonts::width(keys, Face::Italic, 14.0, 0.0);
    fonts::draw(buf, w, h, c.x as f32 + (c.w as f32 - kw) / 2.0, (c.y + c.h) as f32 - 28.0, keys, Face::Italic, 14.0, 0.0, INK_FADED, None);
}

/// `--worlds-snapshot FILE`: every world made, the map's world chosen, at 1280x820.
pub fn save_snapshot(world: &WorldData, path: &str) -> Result<(), Box<dyn Error>> {
    let sys = super::heavens::system_of(world);
    let globes = all_globes(world, &sys);
    let (w, h) = (1280usize, 820usize);
    let mut buf = vec![DESK; w * h];
    for (i, p) in buf.iter_mut().enumerate() {
        let grain = ((hash(i % w / 2, i / w / 9) & 0x1F) as f32 / 31.0) * 0.08;
        *p = mix(DESK, 0x0040_3428, grain);
    }
    let sel = std::env::var("PLANET_WORLDS_SEL").ok().and_then(|v| v.parse().ok())
        .unwrap_or_else(|| sys.planets.iter().position(|p| p.kind == PlanetKind::Home).unwrap_or(0));
    draw_worlds(&sys, &globes, sel, 4.0, &mut buf, w, h);
    image::RgbImage::from_fn(w as u32, h as u32, |x, y| { let p = buf[y as usize * w + x as usize]; image::Rgb([(p >> 16) as u8, (p >> 8) as u8, p as u8]) }).save(path)?;
    Ok(())
}
