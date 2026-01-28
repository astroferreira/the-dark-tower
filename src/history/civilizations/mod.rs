//! Civilization systems: factions, settlements, territory, economy, diplomacy, military, government.

pub mod faction;
pub mod settlement;
pub mod territory;
pub mod economy;
pub mod diplomacy;
pub mod military;
pub mod government;

pub use faction::Faction;
pub use settlement::{Settlement, SettlementType};
pub use territory::Territory;
pub use economy::{ResourceType, TradeRoute};
pub use diplomacy::{DiplomaticRelation, DiplomaticStance};
pub use military::{Army, War};
pub use government::SuccessionLaw;
