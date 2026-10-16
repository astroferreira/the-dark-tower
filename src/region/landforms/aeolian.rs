//! Wind in dry country with bare ground: dune fields shaped by the prevailing wind (transverse
//! ridges across it, linear dunes along it), sand seas in the heart of the deserts, yardangs
//! cut into soft rock, desert pavement on flat stony ground, playas on the salt flats and the
//! floors of closed basins (ROADMAP Stage 2).
//!
//! Dry country: the world's moisture under `DRY_MOIST` at the cell and a desert biome or desert
//! soil on its tile (steppe stays grass). The wind comes from the latitude's belt
//! (`volcanic::wind_dir`: trades, westerlies, polar easterlies; the climate's wind field is not
//! kept on `WorldData`). The dune pattern is a wave in absolute cells along (or across) the
//! wind, warped by noise, so two chunks shape the same dune the same way.

use noise::{NoiseFn, Perlin};

use crate::biomes::ExtendedBiome;
use crate::erosion::materials::RockType;
use crate::region::plan::Landform;
use crate::soils::SoilKind;
use crate::world::WorldData;

use super::volcanic::wind_dir;
use super::{ChunkGrid, Drainage, RegionFields};

/// Moisture index below which the ground is dry enough for sand to move.
const DRY_MOIST: f32 = 0.2;
/// A dune field needs this much sand (the supply noise, 0..1) and ground this flat (m per m).
const SAND_MIN: f32 = 0.35;
const DUNE_SLOPE: f32 = 0.03;
/// Dune (draa) spacing (m) and the tallest dune (m) at 610 m cells.
const DUNE_SPACING_M: f32 = 2000.0;
const DUNE_HEIGHT_M: f32 = 30.0;
/// A dune field whose crests rise less than this (m) is only labelled, not shaped.
const DUNE_LABEL_M: f32 = 3.0;
/// Share of 4x4-tile blocks whose dunes run across the wind (the rest along it).
const TRANSVERSE_SHARE: f32 = 0.6;
/// Playas: lake water in moisture below this is a dry crust most of the year.
const PLAYA_MOIST: f32 = 0.15;
/// Flat ground (m per m) for pavement and playas.
const PAVEMENT_SLOPE: f32 = 0.03;
const PLAYA_SLOPE: f32 = 0.01;

fn hash(a: i64, b: i64, salt: u64) -> u64 {
    let mut h = (a as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (b as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ salt.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 31;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^ (h >> 29)
}

fn unit(a: i64, b: i64, salt: u64) -> f32 {
    (hash(a, b, salt) % 1_000_000) as f32 / 1_000_000.0
}

fn fbm(noise: &Perlin, x: f64, y: f64, octaves: u32) -> f32 {
    let (mut sum, mut amp, mut freq, mut norm) = (0.0, 1.0, 1.0, 0.0);
    for _ in 0..octaves {
        sum += amp * noise.get([x * freq, y * freq]);
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    (sum / norm) as f32
}

/// Bilinear world moisture at fractional tile coordinates (x wrapped).
fn moisture_at(world: &WorldData, u: f32, v: f32) -> f32 {
    let (ww, wh) = (world.width as i64, world.height as i64);
    let (fu, fv) = (u.floor(), v.floor());
    let (tu, tv) = (u - fu, v - fv);
    let at = |dx: i64, dy: i64| -> f32 {
        let (x, y) = ((fu as i64 + dx).rem_euclid(ww) as usize, (fv as i64 + dy).clamp(0, wh - 1) as usize);
        *world.moisture.get(x, y)
    };
    at(0, 0) * (1.0 - tu) * (1.0 - tv) + at(1, 0) * tu * (1.0 - tv) + at(0, 1) * (1.0 - tu) * tv + at(1, 1) * tu * tv
}

/// Whether a world tile is desert: a desert biome, or desert soil, or very dry.
fn desert_tile(world: &WorldData, t: (usize, usize)) -> bool {
    matches!(*world.biomes.get(t.0, t.1), ExtendedBiome::Desert | ExtendedBiome::SaltFlats | ExtendedBiome::SingingDunes | ExtendedBiome::GlassDesert)
        || matches!(*world.soils().kind.get(t.0, t.1), SoilKind::DesertSoil)
        || *world.moisture.get(t.0, t.1) < DRY_MOIST * 0.75
}

/// Bend of a tile field's border between tiles, in tiles: a field read through it has ragged
/// edges, not the tile grid's.
const BORDER_WARP: f64 = 0.18;

/// A 0/1 tile field read at tile coordinates (u, v): bilinear between the four tiles round it,
/// the lookup warped by noise so the field's border wanders. A pure function of position.
fn tile_field_at(world: &WorldData, noise: &Perlin, u: f32, v: f32, salt: f64, field: impl Fn((usize, usize)) -> bool) -> f32 {
    let (ww, wh) = (world.width as i64, world.height as i64);
    let un = u.rem_euclid(ww as f32) as f64;
    let wu = u as f64 + BORDER_WARP * fbm(noise, un * 1.7 + salt, v as f64 * 1.7, 2) as f64;
    let wv = v as f64 + BORDER_WARP * fbm(noise, un * 1.7, v as f64 * 1.7 + salt + 16.0, 2) as f64;
    let (fu, fv) = (wu.floor(), wv.floor());
    let (tu, tv) = ((wu - fu) as f32, (wv - fv) as f32);
    let at = |dx: i64, dy: i64| -> f32 {
        let t = ((fu as i64 + dx).rem_euclid(ww) as usize, (fv as i64 + dy).clamp(0, wh - 1) as usize);
        field(t) as u8 as f32
    };
    at(0, 0) * (1.0 - tu) * (1.0 - tv) + at(1, 0) * tu * (1.0 - tv) + at(0, 1) * (1.0 - tu) * tv + at(1, 1) * tu * tv
}

/// How much of a desert the ground at (u, v) is (0..1): the tile field through its warp.
fn desert_at(world: &WorldData, noise: &Perlin, u: f32, v: f32) -> f32 {
    tile_field_at(world, noise, u, v, 31.0, |t| desert_tile(world, t))
}

/// How much the ground at (u, v) is the world's salt flats (0..1).
fn salt_at(world: &WorldData, noise: &Perlin, u: f32, v: f32) -> f32 {
    tile_field_at(world, noise, u, v, 59.0, |t| *world.biomes.get(t.0, t.1) == ExtendedBiome::SaltFlats)
}

/// The dune field at absolute cell (ax, ay) with world coordinates (u, v): the crest height
/// (m) of the wave there, 0 outside any field. A pure function of position.
fn dune_height(world: &WorldData, noise: &Perlin, s: usize, cell_m: f32, ax: i64, ay: i64, u: f32, v: f32) -> f32 {
    if moisture_at(world, u, v) >= DRY_MOIST { return 0.0; }
    if desert_at(world, noise, u, v) < 0.5 { return 0.0; }
    let wu = u.rem_euclid(world.width as f32) as f64;
    let sand = (0.5 + 0.5 * fbm(noise, wu * 3.0, v as f64 * 3.0, 3)).clamp(0.0, 1.0);
    if sand < SAND_MIN { return 0.0; }
    let lat = (v + 0.5) / world.height as f32 * 180.0 - 90.0;
    let (wx, wy) = wind_dir(lat);
    let block = 4 * s as i64;
    let transverse = unit(ax.div_euclid(block), ay.div_euclid(block), 0xa01) < TRANSVERSE_SHARE;
    let along = if transverse { ax as f32 * wx + ay as f32 * wy } else { -(ax as f32) * wy + ay as f32 * wx };
    let spacing = (DUNE_SPACING_M / cell_m).max(2.5);
    let phase = along / spacing + 0.6 * fbm(noise, wu * 25.0 + 11.0, v as f64 * 25.0, 2);
    let amp = DUNE_HEIGHT_M * (610.0 / cell_m).clamp(0.4, 1.0) * ((sand - SAND_MIN) / (1.0 - SAND_MIN)).clamp(0.0, 1.0);
    amp * (0.5 + 0.5 * (phase * std::f32::consts::TAU).cos())
}

/// Dunes, before the rivers (which are rare where dunes are).
pub fn shape_before(h: &mut [f32], g: &ChunkGrid) {
    let gw = g.gw;
    let noise = Perlin::new((g.seed as u32).wrapping_add(808));
    // A quick look: any dry ground at all in this chunk's tiles?
    let mut dry = false;
    'outer: for tj in 0..=2 {
        for ti in 0..=2 {
            if desert_tile(g.world, g.tile_of(ti * g.s + g.s / 2, tj * g.s + g.s / 2)) { dry = true; break 'outer; }
        }
    }
    if !dry { return; }
    for j in 0..gw {
        for i in 0..gw {
            let k = j * gw + i;
            if h[k] <= 2.0 { continue; }
            // Flat ground only: the base's own slope across the cell.
            if i > 0 && i + 1 < gw && j > 0 && j + 1 < gw {
                let slope = ((h[k + 1] - h[k - 1]).abs().max((h[k + gw] - h[k - gw]).abs())) / (2.0 * g.cell_m);
                if slope > DUNE_SLOPE { continue; }
            }
            let (u, v) = g.to_world(i, j);
            let (ax, ay) = g.abs_cell(i, j);
            let rise = dune_height(g.world, &noise, g.s, g.cell_m, ax, ay, u, v);
            if rise > 0.0 { h[k] += rise; }
        }
    }
}

pub fn shape_after(_h: &mut [f32], _g: &ChunkGrid, _d: &Drainage, _acc: &[f32]) {}

pub fn classify(f: &RegionFields, rock: &[RockType], land: &mut [Landform]) {
    let (gw, gh) = (f.gw, f.gh);
    let noise = Perlin::new((f.seed as u32).wrapping_add(808));
    let soils = f.world.soils();
    let ww = f.world.width as f32;
    // The heart of a desert: desert a tile and a half out on every side.
    let deep_desert = |u: f32, v: f32| -> bool {
        [(0.0f32, 0.0f32), (1.5, 0.0), (-1.5, 0.0), (0.0, 1.5), (0.0, -1.5)].iter().all(|&(du, dv)| desert_at(f.world, &noise, u + du, v + dv) > 0.5)
    };
    for j in 0..gh {
        for i in 0..gw {
            let k = j * gw + i;
            if land[k] != Landform::None || f.h[k] <= 0.0 { continue; }
            let t = f.tile_of(i, j);
            let lake = f.lake_depth(k);
            let slope = f.slope(i, j);
            let (u, v) = f.to_world(i, j);
            // Playas: the salt flats, and lakes that are dry crust most of the year.
            if slope < PLAYA_SLOPE && salt_at(f.world, &noise, u, v) > 0.5 {
                land[k] = Landform::Playa;
                continue;
            }
            if lake > 0.0 {
                if f.moist[k] < PLAYA_MOIST { land[k] = Landform::Playa; }
                continue;
            }
            let dry = f.moist[k] < DRY_MOIST && desert_at(f.world, &noise, u, v) > 0.5;
            if !dry { continue; }
            // A river's own cells keep their channel through the dunes (the region's river rule).
            if f.acc[k] > 0.02 * (f.s * f.s) as f32 / (0.25 + f.moist[k]) { continue; }
            let (ax, ay) = f.abs_cell(i, j);
            if slope <= DUNE_SLOPE * 2.0 {
                let crest = dune_height(f.world, &noise, f.s, f.cell_m, ax, ay, u, v);
                // The field's own top: the wave's amplitude at this spot (the crest at phase 0).
                if crest > 0.0 {
                    let wu = u.rem_euclid(ww) as f64;
                    let sand = (0.5 + 0.5 * fbm(&noise, wu * 3.0, v as f64 * 3.0, 3)).clamp(0.0, 1.0);
                    let amp = DUNE_HEIGHT_M * (610.0 / f.cell_m).clamp(0.4, 1.0) * ((sand - SAND_MIN) / (1.0 - SAND_MIN)).clamp(0.0, 1.0);
                    if amp >= DUNE_LABEL_M {
                        land[k] = if deep_desert(u, v) { Landform::SandSea } else { Landform::Dune };
                        continue;
                    }
                }
            }
            let wu = u.rem_euclid(ww) as f64;
            if matches!(rock[k], RockType::Shale | RockType::Sediment) && slope < 0.05 {
                // Streaks along the wind, cut into the soft rock.
                let lat = (v + 0.5) / f.world.height as f32 * 180.0 - 90.0;
                let (wx, wy) = wind_dir(lat);
                let across = -(ax as f32) * wy + ay as f32 * wx;
                let streak = (across / 2.0 + 0.5 * fbm(&noise, wu * 30.0, v as f64 * 30.0, 2)).sin();
                if streak > 0.5 {
                    land[k] = Landform::Yardang;
                    continue;
                }
            }
            if slope < PAVEMENT_SLOPE && !matches!(*soils.kind.get(t.0, t.1), SoilKind::Alluvium) && matches!(rock[k], RockType::Basalt | RockType::Granite | RockType::Sandstone | RockType::Limestone)
                && fbm(&noise, wu * 5.0 + 7.0, v as f64 * 5.0, 2) > 0.1
            {
                land[k] = Landform::DesertPavement;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A world with a dry band: the moisture zeroed and the biome set to desert over a block
    /// of tiles.
    fn dry_world() -> WorldData {
        let mut w = crate::world::generate_world_with_style(64, 32, 5, crate::plates::WorldStyle::Earthlike);
        for y in 8..20 {
            for x in 10..30 {
                w.moisture.set(x, y, 0.05);
                w.biomes.set(x, y, ExtendedBiome::Desert);
                w.heightmap.set(x, y, 300.0);
            }
        }
        w
    }

    /// Two chunks a tile apart raise the same dunes on the ground they share.
    #[test]
    fn dune_shaping_agrees_between_overlapping_chunks() {
        let w = dry_world();
        let s = 16usize;
        let gw = 3 * s;
        let g1 = ChunkGrid { world: &w, gw, s, gx0: 14, gy0: 12, cell_m: 610.0, seed: 1 };
        let g2 = ChunkGrid { world: &w, gw, s, gx0: 15, gy0: 12, cell_m: 610.0, seed: 1 };
        let (mut h1, mut h2) = (vec![300.0f32; gw * gw], vec![300.0f32; gw * gw]);
        shape_before(&mut h1, &g1);
        shape_before(&mut h2, &g2);
        let mut raised = 0;
        for j in 0..gw {
            for i in s..gw {
                assert_eq!(h1[j * gw + i].to_bits(), h2[j * gw + i - s].to_bits(), "chunks differ at ({i},{j})");
                if h1[j * gw + i] > 300.0 { raised += 1; }
            }
        }
        assert!(raised > 50, "dunes rose on the dry ground: {raised}");
        let top = h1.iter().cloned().fold(0.0f32, f32::max) - 300.0;
        assert!(top > 3.0 && top <= DUNE_HEIGHT_M + 0.01, "crests {top:.1} m");
    }

    /// Dunes are labelled only in the desert; wet ground gets nothing from the wind.
    #[test]
    fn dunes_only_in_the_desert() {
        let w = dry_world();
        let (s, gw, gh) = (16usize, 96usize, 96usize);
        let h = vec![300.0f32; gw * gh];
        let filled = h.clone();
        let recv = vec![u32::MAX; gw * gh];
        let acc = vec![1.0f32; gw * gh];
        let temp = vec![25.0f32; gw * gh];
        // The window spans tiles 12..18 in x: desert from 10 to 30, so the whole window is dry;
        // the region moisture decides the rest: dry on the left, wet on the right.
        let moist: Vec<f32> = (0..gw * gh).map(|k| if k % gw < gw / 2 { 0.05 } else { 0.6 }).collect();
        let f = RegionFields { world: &w, gw, gh, s, gx0: 12, gy0: 10, margin: 1, cell_m: 610.0, seed: 1, h: &h, filled: &filled, recv: &recv, acc: &acc, temp: &temp, moist: &moist };
        let rock = vec![RockType::Sandstone; gw * gh];
        let mut land = vec![Landform::None; gw * gh];
        classify(&f, &rock, &mut land);
        let left = (0..gw * gh).filter(|&k| k % gw < gw / 2 && matches!(land[k], Landform::Dune | Landform::SandSea)).count();
        let right = (0..gw * gh).filter(|&k| k % gw >= gw / 2 && land[k] != Landform::None).count();
        assert!(left > 100, "dunes on the dry half: {left}");
        assert_eq!(right, 0, "nothing on the wet half");
    }
}
