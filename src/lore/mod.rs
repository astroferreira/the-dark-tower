//! World lore projected onto geography: named features (gazetteer) and, built from the
//! history simulation, settlements, roads and ruins at every map scale.

pub mod gazetteer;
pub mod settle;

pub use gazetteer::{build_gazetteer, Feature, FeatureKind, Gazetteer};
pub use settle::{paint_region, region_lore, RegionLore, Site};
