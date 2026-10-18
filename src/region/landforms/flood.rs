//! Floods that shaped the land (ROADMAP Stage 7; only with upheaval):
//!
//! - **Scablands**: where the ice sheet's edge once dammed its meltwater into lakes (the zone
//!   just beyond the old ice, `glacial::IceMap::once` between 0.15 and 0.55), the dams failed
//!   and the lakes emptied in days: the valleys below were scoured to bare rock, with giant
//!   current ripples, dry cataracts and plunge pools (the Channeled Scablands, the Altai). One
//!   tile in `SCABLAND_ONE_IN` of that zone, on a big valley's floor (catchment over a tenth of a
//!   tile, 25 m or less above its channel).
//! - **Abandoned channels**: a river on its floodplain, fan or delta leaves its bed for a lower
//!   course (an avulsion) and the old bed is left as a dry or swampy trough, sinuous, across the
//!   plain: where a floodplain's cells lie on the zero line of a slow noise, off the live channel.

use noise::{NoiseFn, Perlin};

use crate::erosion::materials::RockType;
use crate::region::plan::Landform;

use super::rock::unit;
use super::RegionFields;

const SCABLAND_ONE_IN: f32 = 3.0;

pub fn classify(f: &RegionFields, _rock: &[RockType], land: &mut [Landform]) {
    if f.world.upheaval().is_none() { return; }
    let ice = super::glacial::IceMap::new(f.world);
    let tile_cells = (f.s * f.s) as f32;
    let chans = super::fluvial::channels(f);
    let noise = Perlin::new((f.seed ^ 0xAB4D) as u32);
    for j in 0..f.gh {
        for i in 0..f.gw {
            let k = f.idx(i, j);
            if f.is_sea(k) || f.lake_depth(k) > 0.0 { continue; }
            let (u, v) = f.to_world(i, j);
            let (tx, ty) = f.tile_of(i, j);
            let once = ice.once(u, v);
            if (0.15..0.55).contains(&once) && unit(tx as i64, ty as i64, 0x5CAB) < 1.0 / SCABLAND_ONE_IN {
                let nk = chans.nearest[k];
                if nk != u32::MAX && chans.hand[k] < 25.0 && f.acc[nk as usize] > 0.1 * tile_cells
                    && matches!(land[k], Landform::None | Landform::Outwash | Landform::Terrace | Landform::Braided | Landform::Kettle | Landform::Floodplain) {
                    land[k] = Landform::Scabland;
                    continue;
                }
            }
            if matches!(land[k], Landform::Floodplain | Landform::Fan | Landform::Delta | Landform::Backswamp) && !chans.mask[k] {
                let (ax, ay) = f.abs_cell(i, j);
                let n = noise.get([ax as f64 / 9.0, ay as f64 / 9.0]) as f32;
                if n.abs() < 0.05 && chans.steps[k] > 2 { land[k] = Landform::AbandonedChannel; }
            }
        }
    }
}
