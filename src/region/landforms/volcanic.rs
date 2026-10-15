//! Volcanic ground near the world's volcanoes (their kind and age known from the crust): cones with craters, lava fields with pressure ridges and tumuli, cinder cones on the flanks, calderas with lakes, columnar basalt in old flows, ash plains; hot springs, geysers, fumaroles and mud pots on faults (stress) over young volcanism, caused instead of rolled by `biomes.rs::should_spawn` (ROADMAP Stage 2).
//!
//! Every pass is a pure function of world position (noise keyed through `ChunkGrid::to_world` /
//! `RegionFields::to_world`) and reads nothing beyond the region's margin.

use crate::erosion::materials::RockType;
use crate::region::plan::Landform;

use super::{ChunkGrid, Drainage, RegionFields};

pub fn shape_before(_h: &mut [f32], _g: &ChunkGrid) {}

pub fn shape_after(_h: &mut [f32], _g: &ChunkGrid, _d: &Drainage, _acc: &[f32]) {}

pub fn classify(_f: &RegionFields, _rock: &[RockType], _land: &mut [Landform]) {}
