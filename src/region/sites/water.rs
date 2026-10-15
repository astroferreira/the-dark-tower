//! The world's springs and waterfalls (`underground_water.rs`, `WorldData::tile_water_features` or its equivalent) brought down to the region and placed on its channels and slopes: the first caused places, and the first `views_agree` cases (ROADMAP).
//!
//! Deterministic per world tile: reads only the tile and its neighbours, keys every choice on
//! absolute position.

use crate::erosion::materials::RockType;
use crate::region::landforms::RegionFields;
use crate::region::plan::Landform;

use super::{Core, SitePlan};

pub fn plan(_f: &RegionFields, _land: &[Landform], _rock: &[RockType], _core: Core, _out: &mut SitePlan) {}
