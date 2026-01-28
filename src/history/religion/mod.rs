//! Religion system: deities, worship, and monster cults.

pub mod deity;
pub mod worship;
pub mod monster_cults;

pub use deity::{Deity, DeityType, Domain};
pub use worship::Religion;
pub use monster_cults::MonsterCult;
