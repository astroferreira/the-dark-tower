//! Coasts: cliffs on hard rock facing the swell, with stacks, arches and blowholes; beaches, spits, bars, barrier islands and lagoons on soft coasts; dunes behind the beaches; mangrove and salt marsh in warm sheltered water; raised beaches where the land rose; a drowned forest (ROADMAP Stage 2).
//!
//! Every pass is a pure function of world position (noise keyed through `ChunkGrid::to_world` /
//! `RegionFields::to_world`) and reads nothing beyond the region's margin.

use crate::erosion::materials::RockType;
use crate::region::plan::Landform;

use super::{ChunkGrid, Drainage, RegionFields};

pub fn shape_before(_h: &mut [f32], _g: &ChunkGrid) {}

pub fn shape_after(_h: &mut [f32], _g: &ChunkGrid, _d: &Drainage, _acc: &[f32]) {}

pub fn classify(_f: &RegionFields, _rock: &[RockType], _land: &mut [Landform]) {}
