//! World lore projected onto geography: named features (gazetteer) and, built from the
//! history simulation, settlements, roads and ruins at every map scale.

pub mod bard;
pub mod gazetteer;
pub mod journal;
pub mod resources;
pub mod settle;

pub use gazetteer::{build_gazetteer, Feature, FeatureKind, Gazetteer};
pub use settle::{paint_region, region_lore, RegionLore, Site};
pub use resources::{compute_resources, resource_color, resource_name, Deposit, ResourceMap};
