//! Rivers at region scale: floodplains with meander belts, oxbows, levees and backswamps on low gradients; braided reaches below the mountains; terraces (the valley's incision history); waterfalls at knickpoints and hard bands (the world's `detect_waterfalls` brought down and placed on the channel), plunge pools, rapids, gorges; alluvial fans at range fronts; deltas (bird's-foot or arcuate by wave energy) and estuaries with tidal flats at the mouths (ROADMAP Stage 2).
//!
//! Every pass is a pure function of world position (noise keyed through `ChunkGrid::to_world` /
//! `RegionFields::to_world`) and reads nothing beyond the region's margin.

use crate::erosion::materials::RockType;
use crate::region::plan::Landform;

use super::{ChunkGrid, RegionFields};

pub fn shape_before(_h: &mut [f32], _g: &ChunkGrid) {}

pub fn shape_after(_h: &mut [f32], _g: &ChunkGrid) {}

pub fn classify(_f: &RegionFields, _rock: &[RockType], _land: &mut [Landform]) {}
