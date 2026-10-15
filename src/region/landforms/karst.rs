//! Karst on carbonate rock: limestone pavements on plateaus, dolines (closed hollows drained underground), dry valleys, sinking streams that resurge at the plateau's foot (the region's drainage routed through the ground), blind valleys, gorges where a cave roof fell, tower karst and cockpits in the wet tropics, cenotes on flat young limestone. The cave graph (Stage 5, `region::sites`) starts from the sinks and resurgences labelled here (ROADMAP Stage 2).
//!
//! Every pass is a pure function of world position (noise keyed through `ChunkGrid::to_world` /
//! `RegionFields::to_world`) and reads nothing beyond the region's margin.

use crate::erosion::materials::RockType;
use crate::region::plan::Landform;

use super::{ChunkGrid, RegionFields};

pub fn shape_before(_h: &mut [f32], _g: &ChunkGrid) {}

pub fn shape_after(_h: &mut [f32], _g: &ChunkGrid) {}

pub fn classify(_f: &RegionFields, _rock: &[RockType], _land: &mut [Landform]) {}
