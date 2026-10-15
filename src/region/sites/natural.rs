//! Natural wonders from the landforms: cave mouths (resurgences, potholes, cliff caves, sea caves, lava tubes, ice caves), falls and pools, gorges, arches and stacks, erratics and boulder fields, hot springs, geysers and mud pots, salt springs, tar seeps, petrified forests, fossil beds, amber shores, rare meteorite craters, oxbows, tarns, tors, pavements, blowholes, giant trees, crater lakes (ROADMAP).
//!
//! Deterministic per world tile: reads only the tile and its neighbours, keys every choice on
//! absolute position.

use crate::erosion::materials::RockType;
use crate::region::landforms::RegionFields;
use crate::region::plan::Landform;

use super::{Core, SitePlan};

pub fn plan(_f: &RegionFields, _land: &[Landform], _rock: &[RockType], _core: Core, _out: &mut SitePlan) {}
