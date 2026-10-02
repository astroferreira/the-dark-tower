//! Turns simulated crust into terrain.
//!
//! Elevation is derived from physics rather than painted on:
//! * continental crust floats by Airy isostasy (thicker root => higher surface),
//! * oceanic crust follows the half-space cooling law (depth grows with sqrt(age)),
//! * trenches mark recent subduction, and volcanic edifices (hotspot chains, arcs) sit on top.
//! Sea level is then solved so the planet hits the world style's land fraction, and a thin
//! layer of procedural relief is added for detail the coarse crust model cannot resolve.

use noise::{NoiseFn, Perlin};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use crate::seeds::WorldSeeds;
use crate::tilemap::Tilemap;

use super::generation::generate_plates;
use super::simulation::{
    cell_dir, CrustFields, TectonicParams, TectonicSim, CONTINENTAL_CRUST_KM, OCEANIC_CRUST_KM,
};
use super::types::{Plate, PlateId, WorldStyle};

/// Density contrast factors (1 - rho_crust / rho_mantle), in metres of elevation per km of crust.
const CONTINENT_M_PER_KM: f32 = 152.0; // rho 2800 / 3300
const OCEANIC_EXCESS_M_PER_KM: f32 = 121.0; // rho 2900 / 3300
/// Elevation of equilibrium-thickness continental crust above the reference level.
const CONTINENT_BASE_M: f32 = 500.0;
const TRENCH_DEPTH_M: f32 = 3200.0;
/// Interior uplift: metres gained from the continental margin to the deep interior, over how
/// many cells (at 512 wide), and the crust thickness that counts as continental for it.
const INLAND_UPLIFT_M: f32 = 600.0;
const INLAND_RANGE_CELLS: f32 = 30.0;
const INLAND_CRUST_KM: f32 = 26.0;
/// Minimum fall per cell (m) imposed on the broad-scale relief so it drains to the ocean.
const DRAINAGE_GRADIENT_M: f32 = 0.5;
/// Closed hollows shallower than this (m) are filled; deeper ones remain as lake basins.
const SHALLOW_BASIN_M: f32 = 50.0;
/// Fraction of map height (from each pole) over which terrain blends into a shallow polar sea, and that sea's depth.
const POLAR_MARGIN: f32 = 0.10;
const POLAR_SEA_FLOOR_M: f32 = -1500.0;
/// Lowest sea level (m relative to the isostatic zero), roughly a glacial lowstand.
const MIN_SEA_LEVEL_M: f32 = -300.0;
/// Amplitude of coastline roughness and the elevation band (around sea level) it acts in.
const COAST_ROUGHNESS_M: f32 = 800.0;
const COAST_ROUGHNESS_FALLOFF_M: f32 = 1500.0;
/// Width (cells at 512 wide) of the band around the shoreline that gets roughened.
const COAST_BAND_CELLS: f32 = 6.0;

pub struct TectonicTerrain {
    pub plate_map: Tilemap<PlateId>,
    pub plates: Vec<Plate>,
    pub stress_map: Tilemap<f32>,
    pub heightmap: Tilemap<f32>,
    pub crust: CrustFields,
}

/// Run the full tectonic pipeline: initial plates -> time-stepped simulation -> terrain.
pub fn generate_tectonic_terrain(
    width: usize,
    height: usize,
    plates_count: Option<usize>,
    style: WorldStyle,
    seeds: &WorldSeeds,
    params: &TectonicParams,
) -> TectonicTerrain {
    let mut rng = ChaCha8Rng::seed_from_u64(seeds.tectonics);
    let (plate_map, plates) = generate_plates(width, height, plates_count, style, &mut rng);
    let params = TectonicParams { target_land_fraction: style.target_land_fraction() as f32, ..params.clone() };
    let mut sim = TectonicSim::new(&plate_map, &plates, &mut rng, &params);
    sim.run();
    let result = sim.finish(plates);
    let heightmap = build_heightmap(&result.crust, seeds.heightmap, style.target_land_fraction());
    TectonicTerrain {
        plate_map: result.plate_map,
        plates: result.plates,
        stress_map: result.stress_map,
        heightmap,
        crust: result.crust,
    }
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Isostatic surface elevation (m, relative to the un-levelled reference) of a crustal column.
pub fn isostatic_elevation_m(thickness_km: f32, age_myr: f32, volcanic_m: f32) -> f32 {
    // Parsons & Sclater (1977) ocean-floor subsidence.
    let depth = if age_myr < 70.0 {
        2600.0 + 365.0 * age_myr.max(0.0).sqrt()
    } else {
        5651.0 - 2473.0 * (-age_myr / 36.0).exp()
    };
    let oceanic = -depth + OCEANIC_EXCESS_M_PER_KM * (thickness_km - OCEANIC_CRUST_KM);
    let continental = CONTINENT_M_PER_KM * (thickness_km - CONTINENTAL_CRUST_KM) + CONTINENT_BASE_M;
    let w = smoothstep(14.0, 26.0, thickness_km);
    oceanic * (1.0 - w) + continental * w + volcanic_m
}

/// [1 2 1]/4 blur applied `passes` times, wrapping in x and clamping in y.
fn blur(map: &Tilemap<f32>, passes: usize) -> Tilemap<f32> {
    let (w, h) = (map.width, map.height);
    let mut cur = map.clone();
    for _ in 0..passes {
        let mut tmp = cur.clone();
        for y in 0..h {
            for x in 0..w {
                let l = *cur.get((x + w - 1) % w, y);
                let r = *cur.get((x + 1) % w, y);
                tmp.set(x, y, 0.25 * l + 0.5 * *cur.get(x, y) + 0.25 * r);
            }
        }
        for y in 0..h {
            let up = y.saturating_sub(1);
            let dn = (y + 1).min(h - 1);
            for x in 0..w {
                cur.set(x, y, 0.25 * *tmp.get(x, up) + 0.5 * *tmp.get(x, y) + 0.25 * *tmp.get(x, dn));
            }
        }
    }
    cur
}

fn ridged(noise: &Perlin, p: [f64; 3], freq: f64, octaves: u32) -> f32 {
    let (mut amp, mut f, mut sum, mut norm) = (1.0, freq, 0.0, 0.0);
    for _ in 0..octaves {
        let n = 1.0 - noise.get([p[0] * f + 5.1, p[1] * f + 9.7, p[2] * f + 2.3]).abs();
        sum += amp * n * n;
        norm += amp;
        amp *= 0.5;
        f *= 2.1;
    }
    (sum / norm) as f32
}

fn fbm(noise: &Perlin, p: [f64; 3], freq: f64, octaves: u32) -> f32 {
    let (mut amp, mut f, mut sum, mut norm) = (1.0, freq, 0.0, 0.0);
    for _ in 0..octaves {
        sum += amp * noise.get([p[0] * f + 3.3, p[1] * f + 1.9, p[2] * f + 8.2]);
        norm += amp;
        amp *= 0.5;
        f *= 2.0;
    }
    (sum / norm) as f32
}

/// Value below which `fraction` of the cells lie.
fn quantile(map: &Tilemap<f32>, fraction: f64) -> f32 {
    let mut v: Vec<f32> = map.iter().map(|(_, _, &e)| e).collect();
    let idx = ((fraction * v.len() as f64) as usize).min(v.len() - 1);
    let (_, q, _) = v.select_nth_unstable_by(idx, |a, b| a.partial_cmp(b).unwrap());
    *q
}

/// Priority-flood from the open ocean (sea cells connected to the polar map edges): every other
/// cell is raised just enough to drain toward the ocean with at least `gradient` metres of fall
/// per cell, so closed basins (including inland pockets below sea level) gain an outlet.
fn drain_to_ocean(map: &Tilemap<f32>, sea_level: f32, gradient: f32) -> Tilemap<f32> {
    use std::cmp::Reverse;
    use std::collections::{BinaryHeap, VecDeque};
    let (w, h) = (map.width, map.height);
    let key = |e: f32| Reverse(((e as f64 + 20_000.0) * 1000.0) as u64);
    let mut out = map.clone();
    let mut done = Tilemap::new_with(w, h, false);
    let mut heap = BinaryHeap::new();

    // Open ocean: sea cells reachable from the top/bottom rows.
    let mut q = VecDeque::new();
    for x in 0..w {
        for y in [0, h - 1] {
            if *map.get(x, y) <= sea_level && !*done.get(x, y) {
                done.set(x, y, true);
                q.push_back((x, y));
            }
        }
    }
    while let Some((x, y)) = q.pop_front() {
        heap.push((key(*map.get(x, y)), x, y));
        for (nx, ny) in map.neighbors_8(x, y) {
            if !*done.get(nx, ny) && *map.get(nx, ny) <= sea_level {
                done.set(nx, ny, true);
                q.push_back((nx, ny));
            }
        }
    }

    while let Some((_, x, y)) = heap.pop() {
        let level = (*out.get(x, y)).max(sea_level);
        for (nx, ny) in map.neighbors_8(x, y) {
            if *done.get(nx, ny) { continue; }
            done.set(nx, ny, true);
            let e = (*map.get(nx, ny)).max(level + gradient);
            out.set(nx, ny, e);
            heap.push((key(e), nx, ny));
        }
    }
    out
}

/// Cells from each continental-crust cell to the nearest non-continental crust (0 elsewhere).
fn continental_interior_distance(thickness: &Tilemap<f32>) -> Tilemap<f32> {
    use std::collections::VecDeque;
    let (w, h) = (thickness.width, thickness.height);
    let is_cont = |x: usize, y: usize| *thickness.get(x, y) >= INLAND_CRUST_KM;
    let mut dist = Tilemap::new_with(w, h, f32::MAX);
    let mut q = VecDeque::new();
    for y in 0..h {
        for x in 0..w {
            if !is_cont(x, y) {
                dist.set(x, y, 0.0);
                q.push_back((x, y));
            }
        }
    }
    while let Some((x, y)) = q.pop_front() {
        let d = *dist.get(x, y) + 1.0;
        for (nx, ny) in thickness.neighbors_8(x, y) {
            if d < *dist.get(nx, ny) {
                dist.set(nx, ny, d);
                q.push_back((nx, ny));
            }
        }
    }
    for y in 0..h {
        for x in 0..w {
            if *dist.get(x, y) == f32::MAX { dist.set(x, y, 0.0); }
        }
    }
    dist
}

/// Cells from each cell to the nearest shoreline (land/sea transition at `sea_level`).
fn distance_to_coast(elev: &Tilemap<f32>, sea_level: f32) -> Tilemap<f32> {
    use std::collections::VecDeque;
    let (w, h) = (elev.width, elev.height);
    let mut dist = Tilemap::new_with(w, h, f32::MAX);
    let mut q = VecDeque::new();
    for y in 0..h {
        for x in 0..w {
            let land = *elev.get(x, y) > sea_level;
            if elev.neighbors(x, y).iter().any(|&(nx, ny)| (*elev.get(nx, ny) > sea_level) != land) {
                dist.set(x, y, 0.0);
                q.push_back((x, y));
            }
        }
    }
    while let Some((x, y)) = q.pop_front() {
        let d = *dist.get(x, y) + 1.0;
        for (nx, ny) in elev.neighbors_8(x, y) {
            if d < *dist.get(nx, ny) {
                dist.set(nx, ny, d);
                q.push_back((nx, ny));
            }
        }
    }
    dist
}

/// Build the heightmap (metres, 0 = sea level) from simulated crust.
pub fn build_heightmap(crust: &CrustFields, seed: u64, target_land_fraction: f64) -> Tilemap<f32> {
    let (w, h) = (crust.thickness_km.width, crust.thickness_km.height);

    // Isostatic base, softened so the sharp continent/ocean step becomes a shelf and slope.
    let mut base = Tilemap::new_with(w, h, 0.0f32);
    for y in 0..h {
        for x in 0..w {
            let t = *crust.thickness_km.get(x, y);
            let e = isostatic_elevation_m(t, *crust.age_myr.get(x, y), *crust.volcanic_m.get(x, y));
            // Trenches only cut un-thickened oceanic crust.
            let trench = if t < 12.0 { TRENCH_DEPTH_M * *crust.trench.get(x, y) } else { 0.0 };
            base.set(x, y, e - trench);
        }
    }
    let mut base = blur(&base, 2);

    // Continental interiors stand higher than their margins (thicker, colder lithosphere and
    // long-wavelength dynamic support), giving every continent a gentle seaward slope. This
    // must happen before sea level is solved so the sea floods shelves, not interior basins,
    // and so rivers have a drainage direction across otherwise flat cratons.
    let mut inland = Tilemap::new_with(w, h, 0.0f32);
    let interior = continental_interior_distance(&crust.thickness_km);
    let range = INLAND_RANGE_CELLS * (w as f32 / 512.0).max(0.5);
    for (x, y, &d) in interior.iter() {
        // A dome that keeps rising all the way in (no flat top), so deep interiors still drain.
        inland.set(x, y, INLAND_UPLIFT_M * (d / range).sqrt().min(1.6));
    }
    let inland = blur(&inland, 4);
    for y in 0..h {
        for x in 0..w {
            let v = *base.get(x, y) + *inland.get(x, y);
            base.set(x, y, v);
        }
    }

    // Drainage integration: over geological time rivers fill or breach large closed basins, so
    // the broad-scale relief drains to the open ocean. Fill it from the ocean with a gentle
    // gradient before adding detail, which keeps small local lakes but no continent-sized sinks.
    let provisional_sea = quantile(&base, 1.0 - target_land_fraction).max(MIN_SEA_LEVEL_M);
    let base = drain_to_ocean(&base, provisional_sea, DRAINAGE_GRADIENT_M);

    // Relief the crust model cannot resolve: lowland texture, ridged mountains where the crust
    // is thick, abyssal hills offshore.
    let s = seed as u32;
    let low = Perlin::new(s.wrapping_add(11));
    let ridge = Perlin::new(s.wrapping_add(23));
    let abyss = Perlin::new(s.wrapping_add(37));
    let mut elev = Tilemap::new_with(w, h, 0.0f32);
    for y in 0..h {
        for x in 0..w {
            let b = *base.get(x, y);
            let t = *crust.thickness_km.get(x, y);
            let p = cell_dir(x, y, w, h);
            let mut e = b;
            if t >= 20.0 {
                e += 60.0 * fbm(&low, p, 5.0, 4);
                let mountain = smoothstep(38.0, 62.0, t);
                e += 600.0 * mountain * ridged(&ridge, p, 9.0, 4);
            } else {
                e += 110.0 * fbm(&abyss, p, 10.0, 3);
            }
            // The map does not wrap at the poles and downstream flow routing needs an ocean
            // reaching the top and bottom edges, so continents taper into polar ocean with a
            // ragged margin instead of running off the map.
            let edge = (y.min(h - 1 - y) as f32 + 0.5) / h as f32;
            let edge = edge + 0.06 * fbm(&low, p, 2.5, 4);
            let keep = smoothstep(0.0, POLAR_MARGIN, edge);
            e = e * keep + POLAR_SEA_FLOOR_M * (1.0 - keep);
            elev.set(x, y, e);
        }
    }

    // Sea level: choose the water depth that leaves the requested land fraction exposed.
    let mut sorted: Vec<f32> = elev.iter().map(|(_, _, &v)| v).collect();
    let idx = (((1.0 - target_land_fraction) * sorted.len() as f64) as usize).min(sorted.len() - 1);
    let (_, sea_level, _) = sorted.select_nth_unstable_by(idx, |a, b| a.partial_cmp(b).unwrap());
    // Sea level is a water-volume question: it may rise to drown continental shelves for
    // water-rich styles, but it cannot fall far below the isostatic zero without exposing the
    // ocean floor, so the land fraction is a target rather than a guarantee.
    let sea_level = (*sea_level).max(MIN_SEA_LEVEL_M);

    // Fractal coastline roughness in a band around the shoreline: breaks straight rift edges and
    // round volcanic islands into headlands, coves and islets. It is confined by distance to
    // the coast (not by elevation) so flat lowland interiors near sea level are not pocked
    // with flooded hollows, and it fades on steep or deep ground.
    let coast_dist = distance_to_coast(&elev, sea_level);
    let band = COAST_BAND_CELLS * (w as f32 / 512.0).max(0.5);
    let coast = Perlin::new(s.wrapping_add(53));
    let mut out = Tilemap::new_with(w, h, 0.0f32);
    for (x, y, &v) in elev.iter() {
        let mut e = v - sea_level;
        let p = cell_dir(x, y, w, h);
        let in_band = (-(*coast_dist.get(x, y) / band).powi(2)).exp();
        let gentle = (-(e / COAST_ROUGHNESS_FALLOFF_M).powi(2)).exp();
        e += COAST_ROUGHNESS_M * in_band * gentle * fbm(&coast, p, 7.0, 6);
        // Isostasy alone overshoots the very highest peaks; real ranges are limited by erosion
        // and crustal strength, so compress the extreme tail.
        out.set(x, y, if e > 4500.0 { 4500.0 + (e - 4500.0) * 0.55 } else { e });
    }

    // Shallow hollows left by the detail relief would have silted up or been breached long ago;
    // fill those to their spill level (with a slight gradient) and keep only genuinely deep
    // basins as lakes.
    let drained = drain_to_ocean(&out, 0.0, 0.05);
    let mut result = out.clone();
    for (x, y, &e) in out.iter() {
        let fill = *drained.get(x, y) - e;
        if fill > 0.0 && fill < SHALLOW_BASIN_M {
            result.set(x, y, *drained.get(x, y));
        }
    }
    result
}
