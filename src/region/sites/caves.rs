//! Caves as systems (ROADMAP Stage 5): cave graphs planned per world tile in absolute region
//! cells and metres above the sea (`plan::CaveGraph`), from the karst's sinks and resurgences,
//! the volcanoes' flanks, the hard coasts and the cliffs: karst conduits down the hydraulic
//! gradient to a resurgence (vadose canyons above the water table, phreatic tubes below,
//! chambers at junctions and breakdown, shafts on joints, old dry levels), lava tubes with
//! skylights, sea caves, rock shelters, talus and ice caves, mine adits. The embark carves them
//! (`local::caves`); the region draws their mouths; neighbouring embarks share one graph.
//!
//! Deterministic per world tile: reads only the tile and its neighbours, keys every choice on
//! absolute position.

use crate::erosion::materials::RockType;
use crate::region::landforms::RegionFields;
use crate::region::plan::Landform;

use super::{Core, SitePlan};

pub fn plan(_f: &RegionFields, _land: &[Landform], _rock: &[RockType], _core: Core, _out: &mut SitePlan) {}
