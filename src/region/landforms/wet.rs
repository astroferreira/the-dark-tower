//! Wet ground: bogs where cold wet flats hold water (peat soils), fens on flats fed by the
//! rivers' groundwater, marsh on lake margins, reed beds on shallow lake edges (ROADMAP Stage
//! 2). Labels only, last in the order, and only on cells no other process claimed.

use crate::erosion::materials::RockType;
use crate::region::plan::Landform;
use crate::soils::SoilKind;

use super::{ChunkGrid, Drainage, RegionFields};

/// A bog: colder than this (C) and wetter than this (moisture index), on ground flatter than
/// this (m per m).
const BOG_TEMP_C: f32 = 10.0;
const BOG_MOIST: f32 = 0.6;
const BOG_SLOPE: f32 = 0.02;
/// On peat soil the bog needs less: the soil says the ground is waterlogged.
const PEAT_TEMP_C: f32 = 14.0;
const PEAT_MOIST: f32 = 0.45;
/// A fen lies within this many cells of a channel, flatter than this, wetter than this.
const FEN_STEPS: u16 = 3;
const FEN_SLOPE: f32 = 0.015;
const FEN_MOIST: f32 = 0.5;
const FEN_TEMP_C: f32 = 18.0;
/// Reed beds stand in water this deep (m).
const REED_DEPTH_M: (f32, f32) = (0.5, 2.0);

pub fn shape_before(_h: &mut [f32], _g: &ChunkGrid) {}

pub fn shape_after(_h: &mut [f32], _g: &ChunkGrid, _d: &Drainage, _acc: &[f32]) {}

pub fn classify(f: &RegionFields, _rock: &[RockType], land: &mut [Landform]) {
    let (gw, gh) = (f.gw, f.gh);
    let ch = super::fluvial::channels(f);
    let soils = f.world.soils();
    for j in 0..gh {
        for i in 0..gw {
            let k = j * gw + i;
            if land[k] != Landform::None || f.h[k] <= 0.0 { continue; }
            let lake = f.lake_depth(k);
            if lake > 0.0 {
                // Reed beds on a lake's shallow edge (a land neighbour within a cell).
                if lake >= REED_DEPTH_M.0 && lake <= REED_DEPTH_M.1 && f.temp[k] > 2.0 && beside(f, i, j, |w| f.lake_depth(w) == 0.0 && f.h[w] > 0.0) {
                    land[k] = Landform::ReedBed;
                }
                continue;
            }
            if ch.mask[k] { continue; }
            let slope = f.slope(i, j);
            // Marsh on a lake's margin.
            if slope < BOG_SLOPE && beside(f, i, j, |w| f.lake_depth(w) > 0.0) {
                land[k] = Landform::Marsh;
                continue;
            }
            let (tx, ty) = f.tile_of(i, j);
            let peat = *soils.kind.get(tx, ty) == SoilKind::Peat;
            let (t, m) = (f.temp[k], f.moist[k]);
            let boggy = (t < BOG_TEMP_C && m > BOG_MOIST) || (peat && t < PEAT_TEMP_C && m > PEAT_MOIST);
            if boggy && slope < BOG_SLOPE {
                land[k] = Landform::Bog;
                continue;
            }
            if ch.steps[k] <= FEN_STEPS && slope < FEN_SLOPE && m > FEN_MOIST && t < FEN_TEMP_C && ch.hand[k] < 3.0 {
                land[k] = Landform::Fen;
            }
        }
    }
}

/// Any of the 8 neighbours satisfies `pred`.
fn beside(f: &RegionFields, i: usize, j: usize, pred: impl Fn(usize) -> bool) -> bool {
    for dy in -1i64..=1 {
        for dx in -1i64..=1 {
            if dx == 0 && dy == 0 { continue; }
            let (x, y) = (i as i64 + dx, j as i64 + dy);
            if x < 0 || y < 0 || x >= f.gw as i64 || y >= f.gh as i64 { continue; }
            if pred(y as usize * f.gw + x as usize) { return true; }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::zoom::{accumulate, route};

    /// A cold wet flat with a shallow lake in a hollow: bog round about, marsh on the lake's
    /// margin, reeds in its shallows; nothing on the sea.
    #[test]
    fn cold_wet_flats_are_bog_and_lakes_have_margins() {
        let world = crate::world::generate_world_with_style(64, 32, 5, crate::plates::WorldStyle::Earthlike);
        let (s, gw, gh) = (32usize, 64usize, 64usize);
        let h: Vec<f32> = (0..gw * gh)
            .map(|k| {
                let (x, y) = ((k % gw) as f32, (k / gw) as f32);
                if y >= gh as f32 - 4.0 { return -3.0; }
                let d = ((x - 30.0).powi(2) + (y - 30.0).powi(2)).sqrt();
                let mut e = 20.0 + y * 0.01;
                if d < 6.0 { e -= 3.0 * (1.0 - d / 6.0); }
                e
            })
            .collect();
        let d = route(&h, gw, gh, 0.0);
        let acc = accumulate(&d, &vec![0.0f32; gw * gh]);
        let temp = vec![4.0f32; gw * gh];
        let moist = vec![0.8f32; gw * gh];
        let f = RegionFields { world: &world, gw, gh, s, gx0: 0, gy0: 0, margin: 1, cell_m: 610.0, seed: 1, h: &h, filled: &d.filled, recv: &d.recv, acc: &acc, temp: &temp, moist: &moist };
        let rock = vec![RockType::Sandstone; gw * gh];
        let mut land = vec![Landform::None; gw * gh];
        classify(&f, &rock, &mut land);
        let count = |l: Landform| land.iter().filter(|&&x| x == l).count();
        assert!(count(Landform::Bog) > 1000, "bog on the flat: {}", count(Landform::Bog));
        assert!(count(Landform::Marsh) > 4, "marsh on the lake's margin: {}", count(Landform::Marsh));
        assert!(count(Landform::ReedBed) > 0, "reeds in the shallows: {}", count(Landform::ReedBed));
        for k in 0..gw * gh {
            assert!(!(h[k] <= 0.0 && land[k] != Landform::None), "no wet label on the sea");
        }
    }
}
