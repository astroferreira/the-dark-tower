//! Turns simulated crust into terrain.
//!
//! Elevation is derived from physics rather than painted on:
//! * continental crust floats by Airy isostasy (thicker root => higher surface),
//! * oceanic crust follows the half-space cooling law (depth grows with sqrt(age)),
//! * trenches mark recent subduction, and volcanic edifices (hotspot chains, arcs) sit on top.
//! The planet's water is then poured into the ocean basins: the volume is chosen at birth so the
//! world style's land fraction is reached, and sea level is wherever that water stands (closed
//! basins inland stay dry unless the sea overtops their rim). The landscape evolution keeps that
//! volume (`relevel_to_volume`). A thin layer of procedural relief adds detail the coarse crust model
//! cannot resolve.

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
/// Revision 2's trench, cut after the blur: 4.5 km below the floor beside it at full
/// subduction (the Mariana Trench lies ~5 km below the Pacific floor).
const TRENCH_DEPTH_M_V2: f32 = 4500.0;
/// Interior uplift: metres gained from the continental margin to the deep interior, over how
/// many cells (at 512 wide), and the crust thickness that counts as continental for it.
const INLAND_UPLIFT_M: f32 = 600.0;
const INLAND_RANGE_CELLS: f32 = 30.0;
const INLAND_CRUST_KM: f32 = 26.0;
/// Minimum fall per cell (m) imposed on the broad-scale relief so it drains to the ocean.
const DRAINAGE_GRADIENT_M: f32 = 0.5;
/// Closed hollows shallower than this (m) are filled; deeper ones remain as lake basins.
const SHALLOW_BASIN_M: f32 = 50.0;
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
    /// The planet's ocean water as a global equivalent layer (m; Earth ~2,640). Fixed at birth;
    /// re-applied with `relevel_to_volume` after the landscape evolution.
    pub ocean_gel_m: f32,
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
    let land_fraction = land_fraction_for(style, params.land_scale);
    let params = TectonicParams { target_land_fraction: land_fraction as f32, ..params.clone() };
    let mut sim = TectonicSim::new(&plate_map, &plates, &mut rng, &params);
    sim.run();
    let result = sim.finish(plates);
    let heightmap = build_heightmap(&result.crust, seeds.heightmap, land_fraction);
    let ocean_gel_m = ocean_volume_gel(&heightmap, 0.0);
    TectonicTerrain {
        ocean_gel_m,
        plate_map: result.plate_map,
        plates: result.plates,
        stress_map: result.stress_map,
        heightmap,
        crust: result.crust,
    }
}

/// The share of the planet's surface left dry: the style's, scaled by the star system's water
/// (`land_scale`, 1 for the Earth's).
pub fn land_fraction_for(style: WorldStyle, land_scale: f32) -> f64 {
    if land_scale == 1.0 { style.target_land_fraction() } else { (style.target_land_fraction() * land_scale as f64).clamp(0.04, 0.75) }
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Isostatic surface elevation (m, relative to the un-levelled reference) of a crustal column.
pub fn isostatic_elevation_m(thickness_km: f32, age_myr: f32, volcanic_m: f32) -> f32 {
    // Parsons & Sclater (1977) ocean-floor subsidence; revision 2 the GDH1 plate model (Stein &
    // Stein 1992: the same half-space sqrt(t) to 20 Myr, then flattening with e^(-0.0278 t)),
    // which fits the Earth's old floor better (60 Myr: 5185 m, not 5427).
    let depth = if !crate::terrain_rev::v1() {
        if age_myr < 20.0 { 2600.0 + 365.0 * age_myr.max(0.0).sqrt() } else { 5651.0 - 2473.0 * (-0.0278 * age_myr).exp() }
    } else if age_myr < 70.0 {
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
    // Revision 2: each octave weighted by the one before (Musgrave; `audit.md` 3.3), so crags
    // grow on the crests and the valleys between stay smooth.
    let weighted = !crate::terrain_rev::v1();
    let mut weight = 1.0f64;
    for _ in 0..octaves {
        let n = 1.0 - noise.get([p[0] * f + 5.1, p[1] * f + 9.7, p[2] * f + 2.3]).abs();
        let mut n = n * n;
        if weighted {
            n *= weight;
            weight = (n * crate::heightmap::RIDGE_GAIN).clamp(0.0, 1.0);
        }
        sum += amp * n;
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

/// Priority-flood from the sea (`landscape::sea_mask` at `sea_level`: bodies big enough to be
/// seas, wherever they are): every other cell is raised just enough to drain toward the sea with
/// at least `gradient` metres of fall per cell, so closed basins (including inland pockets below
/// sea level) gain an outlet.
fn drain_to_ocean(map: &Tilemap<f32>, sea_level: f32, gradient: f32) -> Tilemap<f32> {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let (w, h) = (map.width, map.height);
    let key = |e: f32| Reverse(((e as f64 + 20_000.0) * 1000.0) as u64);
    let mut out = map.clone();
    let mut done = Tilemap::new_with(w, h, false);
    let mut heap = BinaryHeap::new();

    let sea = crate::erosion::landscape::sea_mask(map, sea_level);
    for (x, y, &is_sea) in sea.iter() {
        if is_sea {
            done.set(x, y, true);
            heap.push((key(*map.get(x, y)), x, y));
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

/// Flexural parameter of the continental lithosphere (km) and the share of a thrust belt's
/// excess height that shows as the foreland basin's depth beside a broad belt.
const FLEXURE_ALPHA_KM: f32 = 110.0;
const FORELAND_SHARE: f32 = 0.12;

/// Foreland basins (`audit.md` 3.4): the load of thickened crust (its height above normal
/// continental crust) bends the lithosphere over the flexural length, w(r) = w0 e^(-r/a)
/// cos(r/a): a trough beside the belt (Ganges, Po, Llanos) and a low forebulge beyond. Airy
/// isostasy already compensates the belt itself, so only the ground off the load moves.
fn foreland_flexure(base: &mut Tilemap<f32>, thickness: &Tilemap<f32>) {
    let (w, h) = (base.width, base.height);
    let km = crate::erosion::landscape::tile_km(w);
    let a = FLEXURE_ALPHA_KM / km;
    let r_max = (2.4 * a).ceil() as i32;
    if r_max < 1 || a < 0.35 { return; }
    let load: Vec<f32> = thickness.iter().map(|(_, _, &t)| CONTINENT_M_PER_KM * (t - 38.0).max(0.0)).collect();
    if load.iter().all(|&l| l == 0.0) { return; }
    let rows: Vec<f32> = (0..h).map(|y| (std::f32::consts::FRAC_PI_2 - (y as f32 + 0.5) / h as f32 * std::f32::consts::PI).cos().max(0.15)).collect();
    // Normalised so a load spread wider than the kernel deflects the ground under it by its
    // full height (w0 = load).
    let kernel = |r: f32| (-r / a).exp() * (r / a).cos();
    let mut norm = 0.0f32;
    for dy in -r_max..=r_max { for dx in -r_max..=r_max { norm += kernel(((dx * dx + dy * dy) as f32).sqrt()); } }
    let mut out = vec![0.0f32; w * h];
    for y in 0..h {
        let c = rows[y];
        let rx = ((r_max as f32 / c).ceil() as i32).min(w as i32 / 2);
        for x in 0..w {
            let i = y * w + x;
            if load[i] > 300.0 || *thickness.get(x, y) < 20.0 { continue; }
            let mut d = 0.0f32;
            for dy in -r_max..=r_max {
                let ny = y as i32 + dy;
                if ny < 0 || ny >= h as i32 { continue; }
                for dx in -rx..=rx {
                    let r = ((dx as f32 * c).powi(2) + (dy * dy) as f32).sqrt();
                    if r > r_max as f32 { continue; }
                    let nx = (x as i32 + dx).rem_euclid(w as i32) as usize;
                    let l = load[ny as usize * w + nx];
                    if l > 0.0 { d += l * kernel(r) * c; }
                }
            }
            out[i] = -FORELAND_SHARE * d / norm * (1.0 - smoothstep(0.0, 300.0, load[i]));
        }
    }
    for (i, (_, _, e)) in base.iter_mut().enumerate() { *e += out[i]; }
}

/// How far pelagic sediment raises the sea floor (m) by `age_myr`: it builds up toward ~700 m
/// (e-fold 80 Myr: the old Pacific's half a kilometre and more), and a column of sediment
/// over water-filled crust stands at 0.4 of its thickness ((rho_m - rho_s) / (rho_m - rho_w)).
pub fn sediment_lift_m(age_myr: f32) -> f32 {
    0.4 * 700.0 * (1.0 - (-age_myr.max(0.0) / 80.0).exp())
}

/// The seafloor's ages remapped, in their order, to the Earth's steady-state age-area
/// distribution (Parsons 1982): area falls linearly with age to nothing at 180 Myr (the
/// oldest floor subducted), a mean of 60 Myr (the Earth's is ~64). The simulation spreads new
/// floor at its ridges but subducts too little of the old: its oceans averaged 69-111 Myr with
/// 10% or more at the 180 Myr cap, so the age-depth law sank half the ocean below 5.5 km. Area
/// weighted (cos latitude); ties share their mean rank. Crust of 26 km and more is unchanged.
pub fn steady_state_ages(crust: &CrustFields) -> Tilemap<f32> {
    const T_MAX: f64 = 180.0;
    let (w, h) = (crust.age_myr.width, crust.age_myr.height);
    let rows = row_weights(h);
    let mut out = crust.age_myr.clone();
    let mut cells: Vec<(f32, usize)> = crust.age_myr.iter().enumerate()
        .filter(|(i, _)| *crust.thickness_km.get(i % w, i / w) < 26.0)
        .map(|(i, (_, _, &a))| (a, i)).collect();
    if cells.is_empty() { return out; }
    cells.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then(a.1.cmp(&b.1)));
    let total: f64 = cells.iter().map(|&(_, i)| rows[i / w]).sum();
    let mut below = 0.0f64;
    let mut k = 0;
    while k < cells.len() {
        let mut j = k;
        let mut group = 0.0;
        while j < cells.len() && cells[j].0 == cells[k].0 { group += rows[cells[j].1 / w]; j += 1; }
        let f = ((below + group * 0.5) / total).clamp(0.0, 1.0);
        let age = (T_MAX * (1.0 - (1.0 - f).sqrt())) as f32;
        for &(_, i) in &cells[k..j] { out.set(i % w, i / w, age); }
        below += group;
        k = j;
    }
    out
}

/// An orogenic plateau's ceiling: thickened crust spreads sideways rather than rising past
/// ~4.5-5.5 km (Tibet, the Altiplano), so heights above 4.5 km approach 5.5 km.
fn plateau_ceiling(e: f32) -> f32 {
    if e <= 4500.0 { e } else { 4500.0 + 1000.0 * ((e - 4500.0) / 1000.0).tanh() }
}

/// Cells from each continental-crust cell to the nearest non-continental crust (0 elsewhere).
fn continental_interior_distance(thickness: &Tilemap<f32>) -> Tilemap<f32> {
    use std::collections::VecDeque;
    let (w, h) = (thickness.width, thickness.height);
    let is_cont = |x: usize, y: usize| *thickness.get(x, y) >= INLAND_CRUST_KM;
    if !crate::terrain_rev::v1() {
        // Revision 2: the distance on the sphere (`audit.md` 3.2), not a square-stepped count.
        let margin = (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).filter(|&(x, y)| !is_cont(x, y));
        let mut d = crate::tilemap::geodesic_distance(w, h, margin, |_, _| true);
        for (_, _, v) in d.iter_mut() { if *v == f32::MAX { *v = 0.0; } }
        return d;
    }
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
    if !crate::terrain_rev::v1() {
        return crate::tilemap::geodesic_distance(w, h, q.drain(..), |_, _| true);
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

/// Build the heightmap (metres, 0 = sea level) from simulated crust, with as much ocean water as
/// leaves `target_land_fraction` of the map dry.
pub fn build_heightmap(crust: &CrustFields, seed: u64, target_land_fraction: f64) -> Tilemap<f32> {
    let (w, h) = (crust.thickness_km.width, crust.thickness_km.height);
    let v2 = !crate::terrain_rev::v1();
    // Revision 2: the seafloor's ages in the Earth's steady state (`steady_state_ages`).
    let ages = if v2 { steady_state_ages(crust) } else { crust.age_myr.clone() };

    // Isostatic base, softened so the sharp continent/ocean step becomes a shelf and slope.
    let mut base = Tilemap::new_with(w, h, 0.0f32);
    for y in 0..h {
        for x in 0..w {
            let t = *crust.thickness_km.get(x, y);
            let e = isostatic_elevation_m(t, *ages.get(x, y), *crust.volcanic_m.get(x, y));
            // Trenches only cut un-thickened oceanic crust.
            let trench = if t < 12.0 { TRENCH_DEPTH_M * *crust.trench.get(x, y) } else { 0.0 };
            // Revision 2: the trench is cut after the blur (which left a one-cell trench at a
            // seventh of its depth: no ocean below 6 km), and old floor carries its sediment.
            if v2 {
                let ocean = 1.0 - smoothstep(14.0, 26.0, t);
                base.set(x, y, e + ocean * sediment_lift_m(*ages.get(x, y)));
            } else {
                base.set(x, y, e - trench);
            }
        }
    }
    let mut base = blur(&base, 2);
    if v2 {
        for (x, y, e) in base.iter_mut() {
            if *crust.thickness_km.get(x, y) < 12.0 { *e -= TRENCH_DEPTH_M_V2 * *crust.trench.get(x, y); }
        }
    }
    if v2 {
        // Revision 2 (`audit.md` 3.4): the thrust belts' weight bends the plate beside them into
        // a foreland basin (with a low bulge beyond), and thickened crust stops rising at an
        // orogenic plateau's ceiling.
        foreland_flexure(&mut base, &crust.thickness_km);
        for (_, _, e) in base.iter_mut() { *e = plateau_ceiling(*e); }
    }

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
    let provisional_sea = sea_level_for_land_fraction(&base, target_land_fraction);
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
            elev.set(x, y, e);
        }
    }
    if v2 {
        // Fracture zones: transform faults' scars across the ocean floor from the ridges.
        let km = crate::erosion::landscape::tile_km(w);
        let mut ocean = ages.clone();
        for (x, y, a) in ocean.iter_mut() { if *crust.thickness_km.get(x, y) >= 14.0 { *a = 200.0; } }
        let zones = crate::heightmap::carve_fracture_zones(&mut elev, &ocean, 8.0, km, 450.0f32.max(4.0 * km), 1800.0f32.max(10.0 * km), seed);
        if std::env::var("PLANET_TERRAIN_DEBUG").is_ok() { eprintln!("fracture zones: {zones}"); }
    }

    // Sea level: pour in enough water to leave the target land fraction dry. Water stands only
    // where it joins the ocean, so closed hollows inland stay dry even below sea level.
    let sea_level = sea_level_for_land_fraction(&elev, target_land_fraction);
    if std::env::var("PLANET_TERRAIN_DEBUG").is_ok() {
        let mut a: Vec<f32> = ages.iter().filter(|(x, y, _)| *crust.thickness_km.get(*x, *y) < 14.0).map(|(_, _, &a)| a).collect();
        a.sort_by(|p, q| p.partial_cmp(q).unwrap());
        let q = |f: f32| a.get(((a.len() as f32 - 1.0) * f) as usize).copied().unwrap_or(0.0);
        eprintln!("sea level {:+.0} m above the datum; ocean ages p10 {:.0} p50 {:.0} p90 {:.0} mean {:.0} Myr ({} cells)", sea_level, q(0.1), q(0.5), q(0.9), a.iter().sum::<f32>() / a.len().max(1) as f32, a.len());
    }
    // Revision 2: the ocean floor's depth is the age-depth law's below sea level, as it is
    // measured on the Earth; the water poured in for the land fraction stood ~860 m above the
    // law's datum and sank every abyss by as much (35-47% of the ocean below 6 km, Earth ~2%).
    // The continents keep their height above the sea; the shift fades across the margin as the
    // isostatic blend does (crust 14-26 km, blurred as the base was).
    let ocean_lift = if v2 {
        let mut wgt = Tilemap::new_with(w, h, 0.0f32);
        for (x, y, &t) in crust.thickness_km.iter() { wgt.set(x, y, 1.0 - smoothstep(14.0, 26.0, t)); }
        Some(blur(&wgt, 2))
    } else { None };

    // Fractal coastline roughness in a band around the shoreline: breaks straight rift edges and
    // round volcanic islands into headlands, coves and islets. It is confined by distance to
    // the coast (not by elevation) so flat lowland interiors near sea level are not pocked
    // with flooded hollows, and it fades on steep or deep ground.
    let coast_dist = distance_to_coast(&elev, sea_level);
    let band = COAST_BAND_CELLS * (w as f32 / 512.0).max(0.5);
    let coast = Perlin::new(s.wrapping_add(53));
    let mut out = Tilemap::new_with(w, h, 0.0f32);
    let km = crate::erosion::landscape::tile_km(w);
    for (x, y, &v) in elev.iter() {
        let mut e = v - sea_level;
        if let Some(l) = &ocean_lift { e += sea_level * *l.get(x, y); }
        if v2 && e < 0.0 {
            // Revision 2 (`audit.md` 3.5): past the shelf a passive margin falls as a steep
            // continental slope to its foot at -3.2 km, and the turbidites shed down it build
            // a rise that grades into the abyss over a few hundred km. Raise-only, so wide
            // shelves on drowned continental crust stay; trench margins keep their trench.
            let d_km = *coast_dist.get(x, y) * km;
            let passive = 1.0 - smoothstep(0.1, 0.4, *crust.trench.get(x, y));
            if d_km >= 80.0 && passive > 0.0 {
                let apron = crate::heightmap::margin_depth(d_km, e);
                if apron > e { e += (apron - e) * passive; }
            }
        }
        let p = cell_dir(x, y, w, h);
        let in_band = (-(*coast_dist.get(x, y) / band).powi(2)).exp();
        let gentle = (-(e / COAST_ROUGHNESS_FALLOFF_M).powi(2)).exp();
        e += COAST_ROUGHNESS_M * in_band * gentle * fbm(&coast, p, 7.0, 6);
        // Isostasy alone overshoots the very highest peaks; real ranges are limited by erosion
        // and crustal strength, so compress the extreme tail (above the plateau ceiling, where
        // only the ridges' peaks reach, in revision 2).
        let knee = if v2 { 5200.0 } else { 4500.0 };
        out.set(x, y, if e > knee { knee + (e - knee) * 0.55 } else { e });
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

/// Share of the planet's surface in each map row (equirectangular rows shrink toward the poles).
fn row_weights(h: usize) -> Vec<f64> {
    (0..h)
        .map(|y| (std::f64::consts::PI / 2.0 - (y as f64 + 0.5) / h as f64 * std::f64::consts::PI).cos())
        .collect()
}

/// For every cell, the lowest water level at which it joins the world ocean: a priority flood
/// from the deepest cell, where a cell's level is the highest point on the lowest path to it.
/// Closed basins below sea level (a Dead Sea) only join once the sea overtops their rim.
fn ocean_join_level(elev: &Tilemap<f32>) -> Vec<f32> {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let (w, h) = (elev.width, elev.height);
    let key = |e: f32| Reverse(((e as f64 + 20_000.0) * 1000.0) as u64);
    let e: Vec<f32> = elev.iter().map(|(_, _, &v)| v).collect();
    let start = (0..e.len()).min_by(|&a, &b| e[a].partial_cmp(&e[b]).unwrap()).unwrap_or(0);
    let mut level = vec![f32::MAX; e.len()];
    level[start] = e[start];
    let mut heap = BinaryHeap::new();
    heap.push((key(e[start]), start));
    while let Some((_, i)) = heap.pop() {
        let (x, y) = (i % w, i / w);
        for (nx, ny) in elev.neighbors_8(x, y) {
            let j = ny * w + nx;
            if level[j] != f32::MAX { continue; }
            level[j] = e[j].max(level[i]);
            heap.push((key(level[j]), j));
        }
    }
    level
}

/// Ocean volume at sea level `sea_level`, as a global equivalent layer: the depth (m) the water
/// would have if spread over the whole planet (Earth's oceans: ~2,640 m).
pub fn ocean_volume_gel(elev: &Tilemap<f32>, sea_level: f32) -> f32 {
    let level = ocean_join_level(elev);
    gel_at(elev, &level, &row_weights(elev.height), sea_level) as f32
}

fn gel_at(elev: &Tilemap<f32>, level: &[f32], rows: &[f64], s: f32) -> f64 {
    let (mut vol, mut area) = (0.0f64, 0.0f64);
    for (i, (_, y, &e)) in elev.iter().enumerate() {
        area += rows[y];
        if level[i] < s { vol += (s - e) as f64 * rows[y]; }
    }
    vol / area.max(1e-9)
}

/// The sea level (same datum as `elev`) at which the world ocean holds `gel_m` metres of global
/// equivalent water. Water fills the ocean basins and any closed basin it overtops; lower closed
/// basins inland stay dry.
pub fn sea_level_for_volume(elev: &Tilemap<f32>, gel_m: f32) -> f32 {
    let level = ocean_join_level(elev);
    let rows = row_weights(elev.height);
    let (mut lo, mut hi) = elev.iter().fold((f32::MAX, f32::MIN), |(a, b), (_, _, &e)| (a.min(e), b.max(e)));
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if gel_at(elev, &level, &rows, mid) < gel_m as f64 { lo = mid; } else { hi = mid; }
    }
    0.5 * (lo + hi)
}

/// The sea level at which `land_fraction` of the planet's surface is not ocean (cells below sea
/// level in closed basins the sea does not reach count as land).
pub fn sea_level_for_land_fraction(elev: &Tilemap<f32>, land_fraction: f64) -> f32 {
    // A cell is ocean once the sea rises above its join level. Sort cells by join level and
    // flood them in order until the ocean covers (1 - land_fraction) of the planet's area (rows
    // weighted by cos(latitude), so polar tiles count for the little area they cover).
    let level = ocean_join_level(elev);
    let rows = row_weights(elev.height);
    let w = elev.width;
    let mut order: Vec<usize> = (0..level.len()).collect();
    order.sort_by(|&a, &b| level[a].partial_cmp(&level[b]).unwrap());
    let total: f64 = rows.iter().sum::<f64>() * w as f64;
    let target = (1.0 - land_fraction) * total;
    let mut area = 0.0;
    for &i in &order {
        area += rows[i / w];
        if area >= target { return level[i]; }
    }
    level[*order.last().unwrap()]
}

/// Move the datum so sea level is 0 with the ocean holding `gel_m` metres of global equivalent
/// water; returns the rise in sea level (m) this caused. Passes that move rock into or out of the
/// sea (sediment on shelves, erosion of coasts) change what the basins hold; this keeps the
/// planet's water constant.
pub fn relevel_to_volume(elev: &mut Tilemap<f32>, gel_m: f32) -> f32 {
    let s = sea_level_for_volume(elev, gel_m);
    for (_, _, e) in elev.iter_mut() { *e -= s; }
    s
}
