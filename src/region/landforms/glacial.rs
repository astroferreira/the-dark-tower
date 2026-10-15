//! What the ice left where the world's ice reached (past ~40 degrees; the audit's scoured basins and fjords give the extent): U-valleys with hanging side valleys, cirques and tarns, aretes, moraines across the valley damming lakes, drumlin fields, erratics, outwash plains, kettle holes; living glaciers and snowfields on ground that stays below freezing; patterned ground past the tree line (ROADMAP Stage 2).
//!
//! Every pass is a pure function of world position (noise keyed through `ChunkGrid::to_world` /
//! `RegionFields::to_world`) and reads nothing beyond the region's margin.

use crate::erosion::materials::RockType;
use crate::region::plan::Landform;

use super::{ChunkGrid, RegionFields};

pub fn shape_before(_h: &mut [f32], _g: &ChunkGrid) {}

pub fn shape_after(_h: &mut [f32], _g: &ChunkGrid) {}

pub fn classify(_f: &RegionFields, _rock: &[RockType], _land: &mut [Landform]) {}
