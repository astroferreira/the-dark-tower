//! The ages before the chronicle: a light prehistory per world tile (hunters, herders and farmers, builders of stone, the first metal, the last age) leaving sites placed by terrain logic, never by roll: rock shelters and painted caves, kill sites and bone beds, shell middens, flint mines, long barrows on false crests, round-barrow cemeteries on ridgeways, stone circles and standing stones on hard plateaus, rock art, enclosures and henges on terraces, hill forts on spurs, crannogs, bog hoards and bodies, copper workings, desert kites, ridgeways and causeways, fords, lynchets, clearance cairns, watchtowers, quarries, mill ruins, drowned villages (ROADMAP).
//!
//! Deterministic per world tile: reads only the tile and its neighbours, keys every choice on
//! absolute position.

use crate::erosion::materials::RockType;
use crate::region::landforms::RegionFields;
use crate::region::plan::Landform;

use super::{Core, SitePlan};

pub fn plan(_f: &RegionFields, _land: &[Landform], _rock: &[RockType], _core: Core, _out: &mut SitePlan) {}
