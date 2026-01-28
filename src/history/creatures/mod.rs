//! Monster and beast system with parts-based procedural anatomy.

pub mod anatomy;
pub mod behavior;
pub mod generator;
pub mod legendary;
pub mod populations;

pub use anatomy::{BodyPart, BodyPartType, BodyMaterial, BodyPartSpecial, CreatureSize, Intelligence};
pub use behavior::CreatureBehavior;
pub use generator::CreatureSpecies;
pub use legendary::LegendaryCreature;
pub use populations::CreaturePopulation;
