//! The survey (ROADMAP Stage 1): the ruler before the work.
//!
//! `--region-survey X,Y` measures a region: its landform and rock mix, the drainage's
//! geomorphometry against the realistic bands of `docs/reference/numerical_tests.md` and
//! `realistic_map_indicators.md`, the slopes, the geomorphons, the places with their causes and
//! the timings, and draws `survey_<seed>_<x>_<y>.png`; it also plans the window three tiles east
//! and reports how far the overlap agrees (every value of the site layer must be a pure function
//! of world position). `--place-census` counts places over a world; `check` lists the invariants
//! a place breaks; `--survey-find` picks the standard generation sites of a world.
//!
//! Nothing here runs on the generation path.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use image::{Rgb, RgbImage};

use crate::erosion::materials::RockType;
use crate::region::debug::{landform_color, landform_mix, rock_color};
use crate::region::plan::{Landform, PlaceKind};
use crate::region::zoom::{generate_zoom, ZoomParams, ZoomRegion};
use crate::world::WorldData;

// Realistic bands. Natural networks: Horton's bifurcation ratio 3-5 (Horton 1945, Strahler
// 1957); Hack's exponent 0.5-0.6 (Hack 1957; below 0.5 the basins are too round); the
// hypsometric integral 0.3-0.6 for a mature landscape (Strahler 1952; a young plateau is above,
// an old worn one below); geomorphons: ridges about as frequent as valleys, pits near zero
// (Jasiewicz & Stepinski 2013). Drainage density depends on the resolution and the river
// threshold: at ~0.6 km cells with only permanent streams drawn, 0.1-1 km/km2 is the range to
// expect (fine maps of humid basins reach 2-5).
pub const BIFURCATION_BAND: (f32, f32) = (3.0, 5.0);
pub const HACK_BAND: (f32, f32) = (0.5, 0.6);
pub const HYPSOMETRIC_BAND: (f32, f32) = (0.3, 0.6);
pub const DRAINAGE_DENSITY_BAND: (f32, f32) = (0.1, 1.0);
/// Geomorphon lookup distance (cells) and flatness threshold (degrees).
pub const GEOMORPHON_RADIUS: usize = 8;
pub const GEOMORPHON_FLAT_DEG: f32 = 1.0;
/// The ten geomorphons, in the order `geomorphons` numbers them.
pub const GEOMORPHON_NAMES: [&str; 10] = ["flat", "summit", "ridge", "shoulder", "spur", "slope", "hollow", "footslope", "valley", "pit"];
const ROCKS: [RockType; 7] = [RockType::Basalt, RockType::Granite, RockType::Sandstone, RockType::Limestone, RockType::Shale, RockType::Sediment, RockType::Ice];

/// A region's numbers.
#[derive(Clone, Debug, Default)]
pub struct Survey {
    pub land_cells: usize,
    pub river_cells: usize,
    pub lake_cells: usize,
    /// (name, cells), most common first; plain ground included.
    pub landforms: Vec<(&'static str, usize)>,
    pub rocks: Vec<(RockType, usize)>,
    /// km of channel per km2 of land.
    pub drainage_density: f32,
    /// Streams per Strahler order, index 0 = order 1.
    pub streams_by_order: Vec<usize>,
    pub bifurcation_ratio: Option<f32>,
    /// Hack's exponent and the number of basins it was fitted on.
    pub hack: Option<(f32, usize)>,
    pub hypsometric_integral: f32,
    /// Slope percentiles over the land, in degrees: p10, p50, p90, p99.
    pub slope_deg: [f32; 4],
    /// Share of land cells per geomorphon (`GEOMORPHON_NAMES`).
    pub geomorphons: [f32; 10],
    pub places: usize,
    pub caves: usize,
    /// Seconds: chunks, blend, climate and hydrology, classify and plan, the survey itself.
    pub timings: [f32; 5],
}

// ---------------------------------------------------------------------------------------------
// Geomorphometry
// ---------------------------------------------------------------------------------------------

fn channel(r: &ZoomRegion, k: usize) -> bool { r.river_width_m[k] > 0.0 }

/// Length (km) of the step from cell `k` to its receiver, on the sphere's metric.
fn step_km(r: &ZoomRegion, k: usize, rc: usize) -> f32 {
    let w = r.width;
    let (dx, dy) = ((rc % w) as i64 - (k % w) as i64, (rc / w) as i64 - (k / w) as i64);
    let ew = dx as f32 * r.lat_cos(k / w);
    r.cell_m / 1000.0 * (ew * ew + (dy * dy) as f32).sqrt()
}

/// Cells in upstream-to-downstream order (a receiver accumulates more area than any cell that
/// drains into it, so sorting by contributing area is a topological order).
fn upstream_order(r: &ZoomRegion, cells: impl Iterator<Item = usize>) -> Vec<usize> {
    let mut idx: Vec<usize> = cells.collect();
    idx.sort_by(|&a, &b| r.drainage_cells[a].partial_cmp(&r.drainage_cells[b]).unwrap_or(std::cmp::Ordering::Equal).then(a.cmp(&b)));
    idx
}

/// Strahler order of every river cell (0 off the channels).
pub fn strahler_orders(r: &ZoomRegion) -> Vec<u8> {
    let n = r.width * r.height;
    let (mut order, mut best, mut ties) = (vec![0u8; n], vec![0u8; n], vec![0u8; n]);
    for k in upstream_order(r, (0..n).filter(|&k| channel(r, k))) {
        let o = if best[k] == 0 { 1 } else if ties[k] >= 2 { best[k] + 1 } else { best[k] };
        order[k] = o;
        let rc = r.receiver[k];
        if rc == u32::MAX { continue; }
        let rc = rc as usize;
        if !channel(r, rc) { continue; }
        if o > best[rc] { best[rc] = o; ties[rc] = 1; } else if o == best[rc] { ties[rc] += 1; }
    }
    order
}

/// Streams (segments) per Strahler order: a segment begins where a cell's order is above every
/// channel draining into it. Index 0 is order 1.
pub fn streams_by_order(r: &ZoomRegion, order: &[u8]) -> Vec<usize> {
    let n = r.width * r.height;
    let mut best_in = vec![0u8; n];
    for k in 0..n {
        if order[k] == 0 { continue; }
        let rc = r.receiver[k];
        if rc != u32::MAX && order[rc as usize] > 0 { best_in[rc as usize] = best_in[rc as usize].max(order[k]); }
    }
    let max = order.iter().copied().max().unwrap_or(0) as usize;
    let mut counts = vec![0usize; max];
    for k in 0..n {
        if order[k] > 0 && best_in[k] < order[k] { counts[order[k] as usize - 1] += 1; }
    }
    counts
}

/// Horton's bifurcation ratio: the mean of N_u / N_(u+1) over the orders that have streams.
pub fn bifurcation_ratio(streams: &[usize]) -> Option<f32> {
    let ratios: Vec<f32> = streams.windows(2).filter(|w| w[1] > 0).map(|w| w[0] as f32 / w[1] as f32).collect();
    if ratios.is_empty() { None } else { Some(ratios.iter().sum::<f32>() / ratios.len() as f32) }
}

/// Hack's exponent h in L = c A^h, fitted (least squares in logs) over the basins of the
/// window's outlets holding `min_cells` or more, with L the longest flow path in km and A the
/// contributing area in km2. Returns the exponent and the number of basins.
pub fn hack_exponent(r: &ZoomRegion, min_cells: f32) -> Option<(f32, usize)> {
    let n = r.width * r.height;
    let land: Vec<usize> = (0..n).filter(|&k| r.elevation_m[k] > 0.0).collect();
    let mut longest = vec![0.0f32; n];
    for k in upstream_order(r, land.iter().copied()) {
        let rc = r.receiver[k];
        if rc == u32::MAX { continue; }
        let rc = rc as usize;
        let l = longest[k] + step_km(r, k, rc);
        if l > longest[rc] { longest[rc] = l; }
    }
    let cell_km2 = (r.cell_m / 1000.0).powi(2);
    let pts: Vec<(f32, f32)> = land.iter().copied()
        .filter(|&k| r.receiver[k] == u32::MAX && r.drainage_cells[k] >= min_cells && longest[k] > 0.0)
        .map(|k| ((r.drainage_cells[k] * cell_km2 * r.lat_cos(k / r.width)).ln(), longest[k].ln()))
        .collect();
    if pts.len() < 5 { return None; }
    let m = pts.len() as f32;
    let (sx, sy) = (pts.iter().map(|p| p.0).sum::<f32>() / m, pts.iter().map(|p| p.1).sum::<f32>() / m);
    let (mut sxy, mut sxx) = (0.0, 0.0);
    for (x, y) in &pts { sxy += (x - sx) * (y - sy); sxx += (x - sx) * (x - sx); }
    if sxx <= 0.0 { return None; }
    Some((sxy / sxx, pts.len()))
}

/// km of channel per km2 of land.
pub fn drainage_density(r: &ZoomRegion) -> f32 {
    let n = r.width * r.height;
    let (mut length_km, mut area_km2) = (0.0f32, 0.0f32);
    let cell_km2 = (r.cell_m / 1000.0).powi(2);
    for k in 0..n {
        if r.elevation_m[k] <= 0.0 { continue; }
        area_km2 += cell_km2 * r.lat_cos(k / r.width);
        if !channel(r, k) { continue; }
        let rc = r.receiver[k];
        length_km += if rc == u32::MAX { r.cell_m / 1000.0 } else { step_km(r, k, rc as usize) };
    }
    if area_km2 > 0.0 { length_km / area_km2 } else { 0.0 }
}

/// The elevation-relief ratio (mean - min) / (max - min) over the land: Pike and Wilson's
/// estimate of the hypsometric integral.
pub fn hypsometric_integral(r: &ZoomRegion) -> f32 {
    let (mut lo, mut hi, mut sum, mut cnt) = (f32::MAX, f32::MIN, 0.0f64, 0usize);
    for &e in &r.elevation_m {
        if e <= 0.0 { continue; }
        lo = lo.min(e); hi = hi.max(e); sum += e as f64; cnt += 1;
    }
    if cnt == 0 || hi <= lo { return 0.0; }
    ((sum / cnt as f64) as f32 - lo) / (hi - lo)
}

/// Slope (m per m) of every cell by central differences, the sphere's metric east-west.
pub fn slope_field(r: &ZoomRegion) -> Vec<f32> {
    let (w, h) = (r.width, r.height);
    let mut out = vec![0.0f32; w * h];
    for j in 0..h {
        let (j0, j1) = (j.saturating_sub(1), (j + 1).min(h - 1));
        let ew = r.cell_m * r.lat_cos(j);
        for i in 0..w {
            let (i0, i1) = (i.saturating_sub(1), (i + 1).min(w - 1));
            let dx = (r.elevation_m[j * w + i1] - r.elevation_m[j * w + i0]) / ((i1 - i0).max(1) as f32 * ew);
            let dy = (r.elevation_m[j1 * w + i] - r.elevation_m[j0 * w + i]) / ((j1 - j0).max(1) as f32 * r.cell_m);
            out[j * w + i] = (dx * dx + dy * dy).sqrt();
        }
    }
    out
}

/// p10, p50, p90, p99 of the land's slopes in degrees.
pub fn slope_percentiles(r: &ZoomRegion, slope: &[f32]) -> [f32; 4] {
    let mut v: Vec<f32> = slope.iter().zip(&r.elevation_m).filter(|(_, &e)| e > 0.0).map(|(s, _)| s.atan().to_degrees()).collect();
    if v.is_empty() { return [0.0; 4]; }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p = |q: f32| v[((v.len() - 1) as f32 * q).round() as usize];
    [p(0.1), p(0.5), p(0.9), p(0.99)]
}

// Jasiewicz and Stepinski's lookup: rows are the number of directions in which the ground
// falls away ("-"), columns the number in which it rises ("+"); 255 is impossible.
const GEOMORPHON_TABLE: [[u8; 9]; 9] = [
    [0, 0, 0, 7, 7, 8, 8, 8, 9],
    [0, 0, 7, 7, 7, 8, 8, 8, 255],
    [0, 3, 5, 5, 6, 6, 8, 255, 255],
    [3, 3, 5, 5, 5, 6, 255, 255, 255],
    [3, 3, 4, 5, 5, 255, 255, 255, 255],
    [2, 2, 4, 4, 255, 255, 255, 255, 255],
    [2, 2, 2, 255, 255, 255, 255, 255, 255],
    [2, 2, 255, 255, 255, 255, 255, 255, 255],
    [1, 255, 255, 255, 255, 255, 255, 255, 255],
];

/// The geomorphon of every land cell (255 at sea): in each of eight directions the ground
/// within `GEOMORPHON_RADIUS` cells is seen as rising, falling or flat (the larger of the zenith
/// and nadir angles wins by more than `GEOMORPHON_FLAT_DEG`), and the pair of counts names the
/// form.
pub fn geomorphons(r: &ZoomRegion) -> Vec<u8> {
    let (w, h) = (r.width as i64, r.height as i64);
    let n = (w * h) as usize;
    let t = GEOMORPHON_FLAT_DEG.to_radians();
    let dirs: [(i64, i64); 8] = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)];
    let mut out = vec![255u8; n];
    for j in 0..h {
        let ew = r.lat_cos(j as usize);
        for i in 0..w {
            let k = (j * w + i) as usize;
            let e0 = r.elevation_m[k];
            if e0 <= 0.0 { continue; }
            let (mut plus, mut minus) = (0usize, 0usize);
            for (dx, dy) in dirs {
                let unit = r.cell_m * ((dx as f32 * ew).powi(2) + (dy * dy) as f32).sqrt();
                let (mut up, mut down) = (0.0f32, 0.0f32);
                for s in 1..=GEOMORPHON_RADIUS as i64 {
                    let (x, y) = (i + dx * s, j + dy * s);
                    if x < 0 || y < 0 || x >= w || y >= h { break; }
                    let a = ((r.elevation_m[(y * w + x) as usize] - e0) / (s as f32 * unit)).atan();
                    up = up.max(a);
                    down = down.max(-a);
                }
                if up - down > t { plus += 1; } else if down - up > t { minus += 1; }
            }
            out[k] = GEOMORPHON_TABLE[minus][plus];
        }
    }
    out
}

/// Share of the land per geomorphon.
pub fn geomorphon_shares(forms: &[u8]) -> [f32; 10] {
    let mut counts = [0usize; 10];
    for &f in forms { if (f as usize) < 10 { counts[f as usize] += 1; } }
    let total = counts.iter().sum::<usize>().max(1) as f32;
    let mut out = [0.0f32; 10];
    for i in 0..10 { out[i] = counts[i] as f32 / total; }
    out
}

// ---------------------------------------------------------------------------------------------
// The survey
// ---------------------------------------------------------------------------------------------

/// Measure a generated region (`timings[0..4]` from `zoom::last_timings`, the survey's own last).
pub fn survey(r: &ZoomRegion, gen_timings: [f32; 4]) -> Survey {
    let t0 = std::time::Instant::now();
    let n = r.width * r.height;
    let mut rocks: Vec<(RockType, usize)> = ROCKS.iter().map(|&k| (k, 0)).collect();
    for &rk in &r.rock {
        if let Some(e) = rocks.iter_mut().find(|(k, _)| *k == rk) { e.1 += 1; }
    }
    rocks.retain(|(_, c)| *c > 0);
    rocks.sort_by(|a, b| b.1.cmp(&a.1));
    let order = strahler_orders(r);
    let streams = streams_by_order(r, &order);
    let slope = slope_field(r);
    let forms = geomorphons(r);
    let mut s = Survey {
        land_cells: (0..n).filter(|&k| r.elevation_m[k] > 0.0).count(),
        river_cells: (0..n).filter(|&k| channel(r, k)).count(),
        lake_cells: (0..n).filter(|&k| r.lake_depth_m[k] > 0.0).count(),
        landforms: landform_mix(r),
        rocks,
        drainage_density: drainage_density(r),
        bifurcation_ratio: bifurcation_ratio(&streams),
        streams_by_order: streams,
        hack: hack_exponent(r, 50.0),
        hypsometric_integral: hypsometric_integral(r),
        slope_deg: slope_percentiles(r, &slope),
        geomorphons: geomorphon_shares(&forms),
        places: r.places.len(),
        caves: r.caves.len(),
        timings: [gen_timings[0], gen_timings[1], gen_timings[2], gen_timings[3], 0.0],
    };
    s.timings[4] = t0.elapsed().as_secs_f32();
    s
}

fn band(v: f32, b: (f32, f32)) -> &'static str { if v < b.0 { " (below the band)" } else if v > b.1 { " (above the band)" } else { "" } }

/// The survey as text: one labelled line per measure, the places and caves after.
pub fn report(r: &ZoomRegion, s: &Survey) -> String {
    let mut o = String::new();
    let n = (r.width * r.height) as f32;
    let pct = |c: usize| c as f32 * 100.0 / n;
    let _ = writeln!(o, "Survey of {}x{} cells ({:.0} m a cell): land {:.1}%, river cells {}, lake cells {}", r.width, r.height, r.cell_m, pct(s.land_cells), s.river_cells, s.lake_cells);
    let _ = writeln!(o, "  landforms: {}", s.landforms.iter().map(|(name, c)| format!("{name} {:.1}%", pct(*c))).collect::<Vec<_>>().join(", "));
    let _ = writeln!(o, "  rock: {}", s.rocks.iter().map(|(k, c)| format!("{k:?} {:.1}%", pct(*c))).collect::<Vec<_>>().join(", "));
    let _ = writeln!(o, "  drainage density: {:.3} km/km2{}", s.drainage_density, band(s.drainage_density, DRAINAGE_DENSITY_BAND));
    let _ = writeln!(o, "  streams by order: {}", s.streams_by_order.iter().enumerate().map(|(i, c)| format!("{}:{c}", i + 1)).collect::<Vec<_>>().join(" "));
    match s.bifurcation_ratio {
        Some(b) => { let _ = writeln!(o, "  bifurcation ratio: {b:.2}{}", band(b, BIFURCATION_BAND)); }
        None => { let _ = writeln!(o, "  bifurcation ratio: none (one order)"); }
    }
    match s.hack {
        Some((h, b)) => { let _ = writeln!(o, "  Hack's exponent: {h:.2} over {b} basins{}", band(h, HACK_BAND)); }
        None => { let _ = writeln!(o, "  Hack's exponent: none (under 5 basins of 50+ cells)"); }
    }
    let _ = writeln!(o, "  hypsometric integral: {:.2}{}", s.hypsometric_integral, band(s.hypsometric_integral, HYPSOMETRIC_BAND));
    let _ = writeln!(o, "  slope (degrees) p10 {:.2} p50 {:.2} p90 {:.2} p99 {:.2}", s.slope_deg[0], s.slope_deg[1], s.slope_deg[2], s.slope_deg[3]);
    let _ = writeln!(o, "  geomorphons: {}", GEOMORPHON_NAMES.iter().zip(&s.geomorphons).map(|(name, v)| format!("{name} {:.1}%", v * 100.0)).collect::<Vec<_>>().join(", "));
    let _ = writeln!(o, "  places: {}, caves: {}", s.places, s.caves);
    let sc = (r.params.cells_per_tile.max(8) & !1) as i64;
    for p in &r.places {
        let (i, j) = (p.x - (r.world_x0 * sc) as f64, p.y - (r.world_y0 * sc) as f64);
        let z = p.z_m.map(|z| format!(", floor {z:.0} m")).unwrap_or_default();
        let _ = writeln!(o, "    {:?}: {} at cell {:.0},{:.0} (tile {},{}){z}: {}", p.kind, p.title(), i, j, p.tile.0, p.tile.1, if p.cause.is_empty() { "(no cause)" } else { &p.cause });
    }
    for c in &r.caves {
        let _ = writeln!(o, "    cave {:?} {}: {} nodes, {} passages (tile {},{})", c.kind, if c.name.is_empty() { "unnamed" } else { &c.name }, c.nodes.len(), c.passages.len(), c.tile.0, c.tile.1);
    }
    let t = s.timings;
    let _ = writeln!(o, "  timings: chunks {:.2}s, blend {:.2}s, climate and hydrology {:.2}s, classify and plan {:.2}s, survey {:.2}s", t[0], t[1], t[2], t[3], t[4]);
    o
}

// ---------------------------------------------------------------------------------------------
// Invariants
// ---------------------------------------------------------------------------------------------

fn region_cell(r: &ZoomRegion, x: f64, y: f64) -> Option<usize> {
    let sc = (r.params.cells_per_tile.max(8) & !1) as i64;
    let (i, j) = ((x - (r.world_x0 * sc) as f64).round() as i64, (y - (r.world_y0 * sc) as f64).round() as i64);
    if i < 0 || j < 0 || i >= r.width as i64 || j >= r.height as i64 { None } else { Some(j as usize * r.width + i as usize) }
}

/// The invariants a place breaks, one line each: every place lies in the window and on land
/// (unless its kind stands in water), a waterfall is on a channel with a drop within three
/// steps, a spring is near a channel head or a slope foot, a place that opens into the rock has
/// a kind that does, a cave index points at a cave whose mouth is beside the place. Kinds with
/// no rule pass; the stages add their own.
pub fn check(r: &ZoomRegion) -> Vec<String> {
    use PlaceKind::*;
    let mut out = Vec::new();
    let slope = slope_field(r);
    let w = r.width;
    for (pi, p) in r.places.iter().enumerate() {
        let who = format!("{} ({:?} at {:.0},{:.0})", p.title(), p.kind, p.x, p.y);
        let Some(k) = region_cell(r, p.x, p.y) else { out.push(format!("{who}: outside the window")); continue };
        let in_water = matches!(p.kind, Crannog | DrownedVillage | SeaStack | Blowhole | CraterLake | Tarn | Oxbow);
        if !in_water && (r.elevation_m[k] <= 0.0 || r.lake_depth_m[k] > 0.0) { out.push(format!("{who}: not on land")); }
        match p.kind {
            Waterfall => {
                if !channel(r, k) { out.push(format!("{who}: not on a channel")); }
                let (mut c, mut drop, mut steps) = (k, 0.0f32, 0);
                while steps < 3 {
                    let rc = r.receiver[c];
                    if rc == u32::MAX { break; }
                    drop = drop.max(r.elevation_m[k] - r.elevation_m[rc as usize]);
                    c = rc as usize;
                    steps += 1;
                }
                if drop < 2.0 { out.push(format!("{who}: drops only {drop:.1} m within three steps")); }
            }
            Spring => {
                let (i, j) = ((k % w) as i64, (k / w) as i64);
                let mut near_head = false;
                let mut foot = false;
                for dj in -2i64..=2 {
                    for di in -2i64..=2 {
                        let (x, y) = (i + di, j + dj);
                        if x < 0 || y < 0 || x >= w as i64 || y >= r.height as i64 { continue; }
                        let c = y as usize * w + x as usize;
                        if channel(r, c) {
                            // A head: no channel drains into it.
                            let fed = (0..r.width * r.height).any(|u| r.receiver[u] == c as u32 && channel(r, u));
                            if !fed { near_head = true; }
                        }
                        if (di.abs() <= 1 && dj.abs() <= 1) && slope[c] > 0.08 && slope[k] < 0.04 { foot = true; }
                    }
                }
                if !near_head && !foot { out.push(format!("{who}: neither by a channel head nor at a slope foot")); }
            }
            CaveMouth => { if !p.kind.is_underground() { out.push(format!("{who}: a cave mouth that opens nowhere")); } }
            _ => {}
        }
        if let Some(ci) = p.cave {
            match r.caves.get(ci as usize) {
                None => out.push(format!("{who}: cave {ci} does not exist")),
                Some(c) => {
                    let near = c.nodes.iter().any(|nd| nd.kind == crate::region::plan::CaveNodeKind::Mouth && (nd.x - p.x).abs() <= 2.0 && (nd.y - p.y).abs() <= 2.0);
                    if !near { out.push(format!("{who}: cave {ci} has no mouth within 2 cells")); }
                }
            }
        }
        let _ = pi;
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Two windows agree
// ---------------------------------------------------------------------------------------------

/// How far the site layer agrees between this window and the one three tiles east: cells of
/// the overlap whose landform and rock match, and places (kind and cell) found in both.
#[derive(Clone, Copy, Debug, Default)]
pub struct Agreement {
    pub cells: usize,
    pub landform_same: usize,
    pub rock_same: usize,
    pub places_both: usize,
    pub places_either: usize,
}

pub fn agreement(world: &WorldData, params: &ZoomParams) -> Agreement {
    let shift = 3usize;
    if params.tiles <= shift { return Agreement::default(); }
    let a = generate_zoom(world, params);
    let mut p2 = params.clone();
    p2.center_x = (params.center_x + shift) % world.width;
    let b = generate_zoom(world, &p2);
    let s = a.params.cells_per_tile.max(8) & !1;
    let ov = (params.tiles - shift) * s;
    let mut ag = Agreement::default();
    for j in 0..a.height {
        for i in 0..ov {
            let (ka, kb) = (j * a.width + i + shift * s, j * b.width + i);
            ag.cells += 1;
            if a.landform[ka] == b.landform[kb] { ag.landform_same += 1; }
            if a.rock[ka] == b.rock[kb] { ag.rock_same += 1; }
        }
    }
    let key = |r: &ZoomRegion, p: &crate::region::plan::Place| -> Option<(String, i64, i64)> {
        let sc = s as i64;
        let (i, j) = ((p.x - (r.world_x0 * sc) as f64).round() as i64, (p.y - (r.world_y0 * sc) as f64).round() as i64);
        Some((format!("{:?}", p.kind), i, j))
    };
    let mut set_a: Vec<(String, i64, i64)> = a.places.iter().filter_map(|p| key(&a, p)).filter(|(_, i, _)| *i >= (shift * s) as i64).map(|(k, i, j)| (k, i - (shift * s) as i64, j)).collect();
    let mut set_b: Vec<(String, i64, i64)> = b.places.iter().filter_map(|p| key(&b, p)).filter(|(_, i, _)| *i < ov as i64).collect();
    set_a.sort(); set_b.sort();
    let both = set_a.iter().filter(|k| set_b.binary_search(k).is_ok()).count();
    ag.places_both = both;
    ag.places_either = set_a.len() + set_b.len() - both;
    ag
}

// ---------------------------------------------------------------------------------------------
// The picture
// ---------------------------------------------------------------------------------------------

/// A 3x5 capital, rows top to bottom, 3 bits a row.
fn glyph(c: char) -> u16 {
    match c {
        'A' => 0b010_101_111_101_101, 'B' => 0b110_101_110_101_110, 'C' => 0b011_100_100_100_011, 'D' => 0b110_101_101_101_110,
        'E' => 0b111_100_110_100_111, 'F' => 0b111_100_110_100_100, 'G' => 0b011_100_101_101_011, 'H' => 0b101_101_111_101_101,
        'I' => 0b111_010_010_010_111, 'J' => 0b001_001_001_101_010, 'K' => 0b101_101_110_101_101, 'L' => 0b100_100_100_100_111,
        'M' => 0b101_111_111_101_101, 'N' => 0b110_101_101_101_101, 'O' => 0b010_101_101_101_010, 'P' => 0b110_101_110_100_100,
        'Q' => 0b010_101_101_111_011, 'R' => 0b110_101_110_101_101, 'S' => 0b011_100_010_001_110, 'T' => 0b111_010_010_010_010,
        'U' => 0b101_101_101_101_011, 'V' => 0b101_101_101_101_010, 'W' => 0b101_101_111_111_101, 'X' => 0b101_101_010_101_101,
        'Y' => 0b101_101_010_010_010, 'Z' => 0b111_001_010_100_111, _ => 0b111_101_101_101_111,
    }
}

fn put(img: &mut RgbImage, x: i64, y: i64, c: [u8; 3]) {
    if x >= 0 && y >= 0 && x < img.width() as i64 && y < img.height() as i64 { img.put_pixel(x as u32, y as u32, Rgb(c)); }
}

/// `survey_<seed>_<x>_<y>.png`: four panels, hillshaded height, the landform classes, the
/// slope, and the places as lettered marks on a faint map.
pub fn save_png(r: &ZoomRegion, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let (w, h) = (r.width, r.height);
    let gap = 4usize;
    let mut img = RgbImage::from_pixel((4 * w + 3 * gap) as u32, h as u32, Rgb([40, 40, 40]));
    let slope = slope_field(r);
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for &e in &r.elevation_m { if e > 0.0 { lo = lo.min(e); hi = hi.max(e); } }
    let span = (hi - lo).max(1.0);
    let shade_at = |k: usize| -> f32 {
        let (i, j) = (k % w, k / w);
        let e = r.elevation_m[k];
        let ex = if i > 0 { r.elevation_m[k - 1] } else { e };
        let ey = if j > 0 { r.elevation_m[k - w] } else { e };
        (((e - ex) + (e - ey)) / r.cell_m * 6.0).clamp(-0.35, 0.35)
    };
    let shaded = |c: [u8; 3], s: f32| -> [u8; 3] {
        let f = 1.0 + s;
        [(c[0] as f32 * f).clamp(0.0, 255.0) as u8, (c[1] as f32 * f).clamp(0.0, 255.0) as u8, (c[2] as f32 * f).clamp(0.0, 255.0) as u8]
    };
    let water = [70, 110, 160];
    for j in 0..h {
        for i in 0..w {
            let k = j * w + i;
            let e = r.elevation_m[k];
            let wet = e <= 0.0 || r.lake_depth_m[k] > 0.0 || channel(r, k);
            let s = shade_at(k);
            // 1. Height: a green-to-tan-to-white ramp, hillshaded.
            let c1 = if wet { water } else {
                let t = (e - lo) / span;
                let (a, b, tt) = if t < 0.5 { ([120, 150, 90], [200, 180, 120], t * 2.0) } else { ([200, 180, 120], [245, 245, 245], (t - 0.5) * 2.0) };
                let mix = |x: u8, y: u8| (x as f32 * (1.0 - tt) + y as f32 * tt) as u8;
                shaded([mix(a[0], b[0]), mix(a[1], b[1]), mix(a[2], b[2])], s)
            };
            put(&mut img, i as i64, j as i64, c1);
            // 2. Landforms.
            let c2 = if wet { water } else { shaded(landform_color(r.landform[k]), s) };
            put(&mut img, (w + gap + i) as i64, j as i64, c2);
            // 3. Slope: pale to dark red by degrees (30 degrees and over black-red).
            let c3 = if e <= 0.0 { water } else {
                let d = (slope[k].atan().to_degrees() / 30.0).clamp(0.0, 1.0);
                [(250.0 - 150.0 * d) as u8, (240.0 - 200.0 * d) as u8, (220.0 - 200.0 * d) as u8]
            };
            put(&mut img, (2 * (w + gap) + i) as i64, j as i64, c3);
            // 4. A faint map for the places.
            let c4 = if wet { [180, 200, 220] } else { shaded([236, 230, 214], s * 0.5) };
            put(&mut img, (3 * (w + gap) + i) as i64, j as i64, c4);
        }
    }
    let sc = (r.params.cells_per_tile.max(8) & !1) as i64;
    let x4 = (3 * (w + gap)) as i64;
    for p in &r.places {
        let (px, py) = ((p.x - (r.world_x0 * sc) as f64).round() as i64, (p.y - (r.world_y0 * sc) as f64).round() as i64);
        let ink = [160, 30, 30];
        for dy in -3i64..=3 {
            for dx in -3i64..=3 {
                if dx * dx + dy * dy <= 9 { put(&mut img, x4 + px + dx, py + dy, ink); }
            }
        }
        let letter = format!("{:?}", p.kind).chars().next().unwrap_or('?');
        let g = glyph(letter);
        for row in 0..5 {
            for col in 0..3 {
                if g >> (14 - (row * 3 + col)) & 1 == 1 {
                    for sy in 0..2 { for sx in 0..2 { put(&mut img, x4 + px + 5 + col as i64 * 2 + sx, py - 5 + row as i64 * 2 + sy, [20, 20, 20]); } }
                }
            }
        }
    }
    img.save(path)?;
    Ok(())
}

/// Run the survey of a window, print everything and write the picture: what `--region-survey`
/// does.
pub fn run(world: &WorldData, params: &ZoomParams, out_dir: &Path) -> String {
    let t0 = std::time::Instant::now();
    let r = generate_zoom(world, params);
    let gen = t0.elapsed().as_secs_f32();
    let s = survey(&r, crate::region::zoom::last_timings());
    let mut text = format!("Region survey of {}x{} tiles centred on {},{} ({gen:.2}s to generate)\n", params.tiles, params.tiles, params.center_x, params.center_y);
    text.push_str(&report(&r, &s));
    let problems = check(&r);
    let _ = writeln!(text, "  check: {} violation{}", problems.len(), if problems.len() == 1 { "" } else { "s" });
    for p in &problems { let _ = writeln!(text, "    {p}"); }
    let ag = agreement(world, params);
    if ag.cells > 0 {
        let _ = writeln!(text, "  two windows 3 tiles apart agree on the overlap: landform {:.1}% of {} cells, rock {:.1}%, places {} of {}",
            ag.landform_same as f32 * 100.0 / ag.cells as f32, ag.cells, ag.rock_same as f32 * 100.0 / ag.cells as f32, ag.places_both, ag.places_either);
    }
    let path = out_dir.join(format!("survey_{}_{}_{}.png", world.seed(), params.center_x, params.center_y));
    match save_png(&r, &path) {
        Ok(()) => { let _ = writeln!(text, "  saved {}", path.display()); }
        Err(e) => { let _ = writeln!(text, "  failed to save {}: {e}", path.display()); }
    }
    text
}

// ---------------------------------------------------------------------------------------------
// The census
// ---------------------------------------------------------------------------------------------

fn hash(a: u64, b: u64) -> u64 {
    let mut h = a.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h ^= h >> 31;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^ (h >> 29)
}

/// The windows a census samples: `n` land tiles chosen by hash of the seed (0: every window,
/// the map tiled in steps of `tiles`), as window centres whose window fits the map's rows.
pub fn census_windows(world: &WorldData, n: usize, tiles: usize) -> Vec<(usize, usize)> {
    let (w, h) = (world.width, world.height);
    let half = tiles / 2;
    let fits = |y: usize| y >= half && y + (tiles - half) <= h;
    let land = |x: usize, y: usize| *world.heightmap.get(x, y) > 0.0;
    if n == 0 {
        let mut out = Vec::new();
        for y in (half..h).step_by(tiles) {
            for x in (half..w).step_by(tiles) {
                if fits(y) && land(x, y) { out.push((x, y)); }
            }
        }
        return out;
    }
    let candidates: Vec<(usize, usize)> = (0..h).filter(|&y| fits(y)).flat_map(|y| (0..w).map(move |x| (x, y))).filter(|&(x, y)| land(x, y)).collect();
    let mut out = Vec::new();
    let mut i = 0u64;
    while out.len() < n.min(candidates.len()) && i < 100_000 {
        let c = candidates[(hash(world.seed(), i) % candidates.len() as u64) as usize];
        if !out.contains(&c) { out.push(c); }
        i += 1;
    }
    out
}

#[derive(Clone, Debug, Default)]
pub struct Census {
    pub windows: Vec<((usize, usize), usize)>,
    pub by_kind: BTreeMap<String, usize>,
    pub by_age: BTreeMap<String, usize>,
    pub total: usize,
    pub with_cause: usize,
    pub violations: Vec<String>,
    /// Median distance (km) from a place to the nearest of its kind in the same window.
    pub spacing_km: BTreeMap<String, f32>,
}

/// Count the places over `n` sampled windows of `tiles` across (what `--place-census` does).
pub fn census(world: &WorldData, n: usize, tiles: usize, seed: u64) -> Census {
    let mut c = Census::default();
    let mut nearest: BTreeMap<String, Vec<f32>> = BTreeMap::new();
    for (cx, cy) in census_windows(world, n, tiles) {
        let params = ZoomParams { center_x: cx, center_y: cy, tiles, cells_per_tile: 128, erosion_iterations: 40, seed };
        let r = generate_zoom(world, &params);
        c.windows.push(((cx, cy), r.places.len()));
        c.total += r.places.len();
        for (i, p) in r.places.iter().enumerate() {
            let kind = format!("{:?}", p.kind);
            *c.by_kind.entry(kind.clone()).or_default() += 1;
            *c.by_age.entry(format!("{:?}", p.kind.age())).or_default() += 1;
            if !p.cause.is_empty() { c.with_cause += 1; }
            let d = r.places.iter().enumerate().filter(|(j, q)| *j != i && q.kind == p.kind)
                .map(|(_, q)| (((q.x - p.x).powi(2) + (q.y - p.y).powi(2)).sqrt() * r.cell_m as f64 / 1000.0) as f32)
                .fold(f32::MAX, f32::min);
            if d < f32::MAX { nearest.entry(kind).or_default().push(d); }
        }
        c.violations.extend(check(&r).into_iter().map(|v| format!("window {cx},{cy}: {v}")));
    }
    for (k, mut v) in nearest {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        c.spacing_km.insert(k, v[v.len() / 2]);
    }
    c
}

pub fn census_report(c: &Census, seed: u64, tiles: usize) -> String {
    let mut o = String::new();
    let empty = c.windows.iter().filter(|(_, n)| *n == 0).count();
    let _ = writeln!(o, "Place census: {} windows of {tiles} tiles on seed {seed}: {} places of {} kinds; {} window{} with none",
        c.windows.len(), c.total, c.by_kind.len(), empty, if empty == 1 { "" } else { "s" });
    let _ = writeln!(o, "  by kind: {}", if c.by_kind.is_empty() { "none".to_string() } else { c.by_kind.iter().map(|(k, n)| format!("{k} {n}")).collect::<Vec<_>>().join(", ") });
    let _ = writeln!(o, "  by age: {}", if c.by_age.is_empty() { "none".to_string() } else { c.by_age.iter().map(|(k, n)| format!("{k} {n}")).collect::<Vec<_>>().join(", ") });
    let _ = writeln!(o, "  cause coverage: {:.0}% ({} of {})", if c.total > 0 { c.with_cause as f32 * 100.0 / c.total as f32 } else { 100.0 }, c.with_cause, c.total);
    let _ = writeln!(o, "  spacing (median km to the nearest of the kind): {}", if c.spacing_km.is_empty() { "none".to_string() } else { c.spacing_km.iter().map(|(k, d)| format!("{k} {d:.1}")).collect::<Vec<_>>().join(", ") });
    let _ = writeln!(o, "  check: {} violation{}", c.violations.len(), if c.violations.len() == 1 { "" } else { "s" });
    for v in &c.violations { let _ = writeln!(o, "    {v}"); }
    let _ = writeln!(o, "  windows: {}", c.windows.iter().map(|((x, y), n)| format!("{x},{y}:{n}")).collect::<Vec<_>>().join(" "));
    o
}

// ---------------------------------------------------------------------------------------------
// The standard sites
// ---------------------------------------------------------------------------------------------

/// A standard generation site: what it is, where, how well the tile fits, and why.
#[derive(Clone, Debug)]
pub struct Site {
    pub kind: &'static str,
    pub x: usize,
    pub y: usize,
    pub score: f32,
    pub note: String,
}

/// Pick the best tile of each kind the roadmap's gallery wants (a limestone plateau, a young
/// volcano, a glaciated valley, a desert interior, a big river's floodplain and its delta, a hard
/// and a soft coast, a bog, a rift lake, a Mediterranean hillside). Windows of 8 tiles must fit
/// the rows, so the poles' four rows are left out.
pub fn find_sites(world: &WorldData) -> Vec<Site> {
    use crate::biomes::ExtendedBiome;
    let (w, h) = (world.width, world.height);
    let hm = &world.heightmap;
    let land = |x: usize, y: usize| *hm.get(x, y) > 0.0;
    let fits = |y: usize| y >= 4 && y + 4 < h;
    let lat = |y: usize| (0.5 - (y as f32 + 0.5) / h as f32) * 180.0;
    let relief = |x: usize, y: usize| -> f32 {
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for dy in -1i64..=1 {
            for dx in -1i64..=1 {
                let (xx, yy) = ((x as i64 + dx).rem_euclid(w as i64) as usize, (y as i64 + dy).clamp(0, h as i64 - 1) as usize);
                let e = *hm.get(xx, yy);
                lo = lo.min(e); hi = hi.max(e);
            }
        }
        hi - lo
    };
    let hardness = |x: usize, y: usize| world.hardness_map.as_ref().map(|m| *m.get(x, y)).unwrap_or(0.5);
    let top_rock = |x: usize, y: usize| world.handshakes.as_ref().and_then(|hs| hs.get(x, y).tile.rock_stack.first().map(|l| l.rock_type)).unwrap_or_default();
    // Limestone within the top two beds: the regolith is often a thin sediment bed over it.
    let limestone_near_top = |x: usize, y: usize| world.handshakes.as_ref().map(|hs| hs.get(x, y).tile.rock_stack.iter().take(2).any(|l| l.rock_type == RockType::Limestone)).unwrap_or(false);
    // Distance to the coast in tiles (breadth-first from the sea over the land).
    let mut coast = vec![u32::MAX; w * h];
    let mut queue = std::collections::VecDeque::new();
    for y in 0..h { for x in 0..w { if !land(x, y) { coast[y * w + x] = 0; queue.push_back((x, y)); } } }
    while let Some((x, y)) = queue.pop_front() {
        let d = coast[y * w + x];
        for (dx, dy) in [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)] {
            let (xx, yy) = ((x as i64 + dx).rem_euclid(w as i64) as usize, y as i64 + dy);
            if yy < 0 || yy >= h as i64 { continue; }
            let k = yy as usize * w + xx;
            if coast[k] == u32::MAX { coast[k] = d + 1; queue.push_back((xx, yy as usize)); }
        }
    }
    let coast = |x: usize, y: usize| coast[y * w + x];
    let flow_acc = |x: usize, y: usize| world.flow_accumulation.as_ref().map(|m| *m.get(x, y)).unwrap_or(0.0);
    let river_threshold = crate::water_bodies::river_flow_threshold(w);
    let soils = world.soils();
    let mut best: BTreeMap<&'static str, Site> = BTreeMap::new();
    let mut offer = |kind: &'static str, x: usize, y: usize, score: f32, note: String| {
        if score <= 0.0 { return; }
        let better = best.get(kind).map(|s| score > s.score).unwrap_or(true);
        if better { best.insert(kind, Site { kind, x, y, score, note }); }
    };
    for y in 0..h {
        if !fits(y) { continue; }
        for x in 0..w {
            if !land(x, y) {
                // A rift lake: a lake tile under tension.
                if world.water_body_map.get(x, y).is_lake() {
                    let st = *world.stress_map.get(x, y);
                    if st < -0.03 { offer("rift lake", x, y, -st, format!("a lake at stress {st:+.3}")); }
                }
                continue;
            }
            let e = *hm.get(x, y);
            let rl = relief(x, y);
            let hd = hardness(x, y);
            let biome = *world.biomes.get(x, y);
            let coastal = world.is_coastal(x, y);
            if (top_rock(x, y) == RockType::Limestone || limestone_near_top(x, y)) && (200.0..=1800.0).contains(&e) {
                // Limestone at the surface, as flat and as high above the sea as can be had.
                offer("limestone plateau", x, y, 1.0 / (1.0 + rl / 300.0) + 0.3 * (1.0 - (e - 700.0).abs() / 1100.0).max(0.0), format!("limestone top rock, {e:.0} m, relief {rl:.0} m"));
            }
            // Where the ice reached (the glacial pass's own rule), with relief and a river to
            // have carved a trunk valley.
            let once = crate::region::landforms::glacial::glaciated(world, (x, y)).map(|g| g.once).unwrap_or(0.0);
            if once > 0.5 && rl > 600.0 && flow_acc(x, y) > river_threshold / 4.0 {
                offer("glaciated valley", x, y, rl / 1000.0 * (1.0 + (4.0 * flow_acc(x, y) / river_threshold).ln().max(0.0) * 0.2) * once, format!("ice {once:.2}, latitude {:.0}, relief {rl:.0} m, a stream", lat(y)));
            }
            if biome == ExtendedBiome::Desert {
                offer("desert interior", x, y, coast(x, y) as f32, format!("desert {} tiles from the coast", coast(x, y)));
            }
            let slope = rl / (world.scale.km_per_tile * 1000.0 * 2.0);
            if coast(x, y) >= 3 && slope < 0.003 && flow_acc(x, y) > river_threshold {
                offer("big river floodplain", x, y, flow_acc(x, y) * (1.0 - slope / 0.003), format!("flow {:.0} units, slope {:.4}, {} tiles inland", flow_acc(x, y), slope, coast(x, y)));
            }
            if coastal && hd > 0.7 { offer("hard coast", x, y, hd - 0.7 + rl / 2000.0, format!("hardness {hd:.2}, relief {rl:.0} m")); }
            if coastal && hd < 0.4 { offer("soft coast", x, y, 0.4 - hd + 0.05 * (1.0 - rl / 200.0).max(0.0), format!("hardness {hd:.2}, relief {rl:.0} m")); }
            if *soils.kind.get(x, y) == crate::soils::SoilKind::Peat {
                let m = *world.moisture.get(x, y);
                offer("bog", x, y, m * (1.0 - rl / 300.0).max(0.1), format!("peat, moisture {m:.2}, relief {rl:.0} m"));
            }
            if biome == ExtendedBiome::MediterraneanShrubland && (200.0..=800.0).contains(&rl) {
                offer("Mediterranean hillside", x, y, rl / 800.0, format!("winter-rain scrub, relief {rl:.0} m"));
            }
        }
    }
    // The delta: follow the floodplain's river to the sea.
    if let Some(fp) = best.get("big river floodplain").cloned() {
        let dir = crate::erosion::rivers::compute_flow_direction(hm);
        let (mut x, mut y) = (fp.x, fp.y);
        let mut last = (x, y);
        for _ in 0..4000 {
            let d = *dir.get(x, y);
            if d == crate::erosion::rivers::NO_FLOW { break; }
            let (nx, ny) = ((x as i64 + crate::erosion::rivers::DX[d as usize] as i64).rem_euclid(w as i64) as usize, (y as i64 + crate::erosion::rivers::DY[d as usize] as i64).clamp(0, h as i64 - 1) as usize);
            if !land(nx, ny) { break; }
            last = (nx, ny);
            x = nx; y = ny;
        }
        if last != (fp.x, fp.y) && fits(last.1) {
            best.insert("river delta", Site { kind: "river delta", x: last.0, y: last.1, score: 1.0, note: format!("the mouth of the floodplain's river, flow {:.0} units", flow_acc(last.0, last.1)) });
        }
    }
    // The volcano: active and tall first.
    if let Some(v) = world.volcanoes.iter().filter(|v| fits(v.y)).max_by(|a, b| (a.is_active as u8, a.peak_height).partial_cmp(&(b.is_active as u8, b.peak_height)).unwrap()) {
        best.insert("young volcano", Site { kind: "young volcano", x: v.x, y: v.y, score: v.peak_height, note: format!("{:?}, {}, {:.0} m above its ground", v.volcano_type, if v.is_active { "active" } else { "quiet" }, v.peak_height) });
    }
    let order = ["limestone plateau", "young volcano", "glaciated valley", "desert interior", "big river floodplain", "river delta", "hard coast", "soft coast", "bog", "rift lake", "Mediterranean hillside"];
    order.iter().filter_map(|k| best.get(k).cloned()).collect()
}

pub fn sites_report(world: &WorldData, sites: &[Site]) -> String {
    let mut o = String::new();
    let _ = writeln!(o, "Generation sites on seed {} ({}x{}):", world.seed(), world.width, world.height);
    for s in sites { let _ = writeln!(o, "  {:<24} {:>3},{:<3} (score {:.2}: {})", s.kind, s.x, s.y, s.score, s.note); }
    let _ = writeln!(o, "CLAUDE.md: generation sites (seed {}, {}x{}): {}", world.seed(), world.width, world.height, sites.iter().map(|s| format!("{} {},{}", s.kind, s.x, s.y)).collect::<Vec<_>>().join("; "));
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_geomorphon_table_is_consistent() {
        // Every possible pair of counts names a form; impossible pairs are marked.
        for m in 0..=8 { for p in 0..=8 { assert_eq!(GEOMORPHON_TABLE[m][p] == 255, m + p > 8, "minus {m} plus {p}"); } }
        assert_eq!(GEOMORPHON_TABLE[8][0], 1, "all lower: a summit");
        assert_eq!(GEOMORPHON_TABLE[0][8], 9, "all higher: a pit");
        assert_eq!(GEOMORPHON_TABLE[0][0], 0, "all level: flat");
    }

    #[test]
    fn bifurcation_is_the_mean_of_ratios() {
        assert!((bifurcation_ratio(&[40, 10, 2]).unwrap() - 4.5).abs() < 1e-6);
        assert!(bifurcation_ratio(&[7]).is_none());
        assert!((bifurcation_ratio(&[8, 2, 0]).unwrap() - 4.0).abs() < 1e-6, "an empty order is skipped");
    }

    #[test]
    fn every_capital_has_a_glyph() {
        for c in 'A'..='Z' { assert_ne!(glyph(c), glyph('?'), "{c}"); }
    }
}
