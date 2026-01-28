//! Creature behavior patterns.

use serde::{Serialize, Deserialize};
use rand::Rng;
use super::anatomy::{CreatureSize, Intelligence};

/// Behavioral tendencies for a creature species (all values 0.0 to 1.0).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreatureBehavior {
    /// 0.0 = docile, 1.0 = always attacks.
    pub aggression: f32,
    /// Defends lair/territory.
    pub territoriality: f32,
    /// Tendency to form groups.
    pub pack_tendency: f32,
    /// Prefers ambush over direct combat.
    pub ambush_tendency: f32,
    /// Collects treasure/valuables.
    pub treasure_hoarding: f32,
    /// Creates or modifies lair.
    pub lair_building: f32,
    /// Tendency to migrate/roam.
    pub migration: f32,
}

impl CreatureBehavior {
    /// Generate behavior based on size and intelligence.
    pub fn generate(size: CreatureSize, intelligence: Intelligence, rng: &mut impl Rng) -> Self {
        let base_aggression = match size {
            CreatureSize::Tiny => 0.2,
            CreatureSize::Small => 0.3,
            CreatureSize::Medium => 0.5,
            CreatureSize::Large => 0.6,
            CreatureSize::Huge => 0.7,
            CreatureSize::Gargantuan => 0.8,
            CreatureSize::Colossal => 0.9,
        };

        let pack = match size {
            CreatureSize::Tiny | CreatureSize::Small => 0.7,
            CreatureSize::Medium => 0.5,
            CreatureSize::Large => 0.3,
            CreatureSize::Huge | CreatureSize::Gargantuan | CreatureSize::Colossal => 0.1,
        };

        let hoarding = match intelligence {
            Intelligence::Mindless | Intelligence::Instinctual => 0.0,
            Intelligence::Cunning => 0.2,
            Intelligence::Sapient => 0.5,
            Intelligence::Genius => 0.8,
        };

        let lair = match intelligence {
            Intelligence::Mindless => 0.0,
            Intelligence::Instinctual => 0.3,
            Intelligence::Cunning => 0.5,
            Intelligence::Sapient | Intelligence::Genius => 0.8,
        };

        fn vary(base: f32, rng: &mut impl Rng) -> f32 {
            (base + rng.gen_range(-0.2..=0.2)).clamp(0.0, 1.0)
        }

        Self {
            aggression: vary(base_aggression, rng),
            territoriality: vary(0.5, rng),
            pack_tendency: vary(pack, rng),
            ambush_tendency: vary(if intelligence >= Intelligence::Cunning { 0.5 } else { 0.1 }, rng),
            treasure_hoarding: vary(hoarding, rng),
            lair_building: vary(lair, rng),
            migration: vary(0.3, rng),
        }
    }

    /// Whether this creature is likely to be a threat to settlements.
    pub fn is_aggressive(&self) -> bool {
        self.aggression > 0.6
    }

    /// Whether this creature hoards treasure (relevant for artifacts).
    pub fn hoards_treasure(&self) -> bool {
        self.treasure_hoarding > 0.4
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn test_behavior_generation() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let behavior = CreatureBehavior::generate(
            CreatureSize::Large,
            Intelligence::Cunning,
            &mut rng,
        );
        assert!((0.0..=1.0).contains(&behavior.aggression));
        assert!((0.0..=1.0).contains(&behavior.pack_tendency));
    }

    #[test]
    fn test_colossal_aggressive() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let behavior = CreatureBehavior::generate(
            CreatureSize::Colossal,
            Intelligence::Genius,
            &mut rng,
        );
        // Colossal creatures should be very aggressive
        assert!(behavior.aggression > 0.5);
        // Genius creatures should hoard
        assert!(behavior.hoards_treasure());
    }
}
