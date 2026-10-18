//! Snow, ice and fire that kill (ROADMAP Stage 7; only with upheaval):
//!
//! - **Crevasse fields** on a glacier where it bends over a step in its bed or spreads at its
//!   snout: glacier cells steeper than 0.12 (at 610 m cells), or 0.06 at its edge.
//! - **Avalanche paths**: on wooded slopes of 24-50 degrees with a snowy winter (mean under 4 C,
//!   moisture over 0.35) the snow comes down the same chutes every year and the trees never
//!   grow back in them: strips down the slope.
//! - **Burned woods**: in the woods of the fire climates (boreal forest, savanna, the
//!   Mediterranean's dry summers: summer-dry and warm enough to burn, 0.42-0.7 moisture)
//!   lightning sets fires that burn patches of forest; the dead trees stand for decades.

use noise::{NoiseFn, Perlin};

use crate::biomes::ExtendedBiome;
use crate::erosion::materials::RockType;
use crate::region::plan::Landform;

use super::rock::{scale, unit};
use super::RegionFields;

pub fn classify(f: &RegionFields, _rock: &[RockType], land: &mut [Landform]) {
    if f.world.upheaval().is_none() { return; }
    let k_scale = scale(f.cell_m);
    let noise = Perlin::new((f.seed ^ 0x4A2D) as u32);
    let src = land.to_vec();
    for j in 0..f.gh {
        for i in 0..f.gw {
            let k = f.idx(i, j);
            if f.is_sea(k) || f.lake_depth(k) > 0.0 { continue; }
            let slope = f.slope(i, j);
            let (ax, ay) = f.abs_cell(i, j);
            let (t, m) = (f.temp[k], f.moist[k]);
            if src[k] == Landform::Glacier {
                let edge = (-1i64..=1).any(|dy| (-1i64..=1).any(|dx| {
                    let (x, y) = (i as i64 + dx, j as i64 + dy);
                    x >= 0 && y >= 0 && (x as usize) < f.gw && (y as usize) < f.gh && src[f.idx(x as usize, y as usize)] != Landform::Glacier
                }));
                if slope > 0.12 * k_scale || (edge && slope > 0.06 * k_scale && unit(ax, ay, 0xC4E) < 0.3) { land[k] = Landform::Crevasses; }
                continue;
            }
            if src[k] != Landform::None && src[k] != Landform::UValley && src[k] != Landform::Cirque { continue; }
            let deg = slope.atan().to_degrees();
            if t < 4.0 && t > -6.0 && m > 0.35 && (24.0..50.0).contains(&deg) {
                // Chutes: narrow strips where a slow noise crosses zero, broken along their length.
                let n = noise.get([ax as f64 / 5.0, ay as f64 / 5.0]) as f32;
                if n.abs() < 0.12 { land[k] = Landform::AvalanchePath; continue; }
            }
            let (tx, ty) = f.tile_of(i, j);
            let biome = *f.world.biomes.get(tx, ty);
            let fire_climate = matches!(biome, ExtendedBiome::BorealForest | ExtendedBiome::Savanna | ExtendedBiome::SubalpineForest)
                || (t > 12.0 && m > 0.42 && m < 0.6);
            if fire_climate && m > 0.42 && m < 0.7 && t > -2.0 && slope < 0.4 {
                let n = noise.get([ax as f64 / 22.0 + 50.0, ay as f64 / 22.0]) as f32;
                if n > 0.42 && unit(tx as i64, ty as i64, 0xF12E) < 0.6 { land[k] = Landform::BurnScar; }
            }
        }
    }
}
