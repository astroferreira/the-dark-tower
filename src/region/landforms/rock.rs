//! Rock at the surface, and what hardness makes of it: a surface rock per cell with a dip from
//! the tile's rock stack and the world's hardness; cliffs and scree where slope and hardness pass
//! a threshold; benches where strata outcrop; mesas, slot canyons and hoodoos under hard caprock
//! in dry country; badlands on soft shale; tors on old worn granite (ROADMAP Stage 2).

use crate::erosion::materials::RockType;
use crate::region::plan::Landform;

use super::{ChunkGrid, RegionFields};

/// The rock at the surface of every cell. Until the dip and the strata are modelled: the top
/// layer of the cell's world tile's rock stack (`region::handshake`), sandstone where there is
/// none.
pub fn surface_rock(f: &RegionFields) -> Vec<RockType> {
    let n = f.gw * f.gh;
    let Some(hs) = f.world.handshakes.as_ref() else { return vec![RockType::Sandstone; n] };
    let mut out = Vec::with_capacity(n);
    for j in 0..f.gh {
        for i in 0..f.gw {
            let (tx, ty) = f.tile_of(i, j);
            let stack = &hs.get(tx, ty).tile.rock_stack;
            out.push(stack.first().map(|l| l.rock_type).unwrap_or_default());
        }
    }
    out
}

pub fn shape_before(_h: &mut [f32], _g: &ChunkGrid) {}

pub fn shape_after(_h: &mut [f32], _g: &ChunkGrid) {}

pub fn classify(_f: &RegionFields, _rock: &[RockType], _land: &mut [Landform]) {}
