//! Body part system for procedural creature anatomy.

use serde::{Serialize, Deserialize};

/// Body part categories.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BodyPartType {
    Head,
    Torso,
    Arms,
    Legs,
    Wings,
    Tail,
    Tentacles,
    Fins,
    Horns,
    Mandibles,
    Eyes,
    Mouth,
}

/// Relative size of a body part.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BodyPartSize {
    Vestigial,
    Small,
    Normal,
    Large,
    Massive,
}

/// Material composition of a body part.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BodyMaterial {
    Flesh,
    Chitin,
    Scales,
    Feathers,
    Stone,
    Metal,
    Crystal,
    Shadow,
    Flame,
    Ooze,
    Bone,
    Ice,
    Fungal,
}

/// Special properties of a body part.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BodyPartSpecial {
    Venomous,
    Acidic,
    FireBreathing,
    IceBreathing,
    Grasping,
    Regenerating,
    Armored,
    Camouflaged,
    Bioluminescent,
    Prehensile,
    Magical,
    Paralyzing,
    Absorbing,
}

/// A specific body part with count, size, material, and specials.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BodyPart {
    pub part_type: BodyPartType,
    pub count: u8,
    pub size: BodyPartSize,
    pub material: BodyMaterial,
    pub specials: Vec<BodyPartSpecial>,
}

impl BodyPart {
    pub fn new(part_type: BodyPartType, count: u8, material: BodyMaterial) -> Self {
        Self {
            part_type,
            count,
            size: BodyPartSize::Normal,
            material,
            specials: Vec::new(),
        }
    }

    pub fn with_size(mut self, size: BodyPartSize) -> Self {
        self.size = size;
        self
    }

    pub fn with_special(mut self, special: BodyPartSpecial) -> Self {
        if !self.specials.contains(&special) {
            self.specials.push(special);
        }
        self
    }
}

/// Creature size categories with rarity weights.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CreatureSize {
    Tiny,
    Small,
    Medium,
    Large,
    Huge,
    Gargantuan,
    Colossal,
}

impl CreatureSize {
    /// Probability weight for generating this size.
    pub fn rarity_weight(&self) -> f32 {
        match self {
            CreatureSize::Tiny => 0.15,
            CreatureSize::Small => 0.25,
            CreatureSize::Medium => 0.30,
            CreatureSize::Large => 0.18,
            CreatureSize::Huge => 0.08,
            CreatureSize::Gargantuan => 0.03,
            CreatureSize::Colossal => 0.001,
        }
    }

    /// All sizes in order.
    pub fn all() -> &'static [CreatureSize] {
        &[
            CreatureSize::Tiny, CreatureSize::Small, CreatureSize::Medium,
            CreatureSize::Large, CreatureSize::Huge, CreatureSize::Gargantuan,
            CreatureSize::Colossal,
        ]
    }

    /// Pick a random size using rarity weights.
    pub fn random_weighted(rng: &mut impl rand::Rng) -> Self {
        let total: f32 = Self::all().iter().map(|s| s.rarity_weight()).sum();
        let mut roll: f32 = rng.gen_range(0.0..total);
        for size in Self::all() {
            roll -= size.rarity_weight();
            if roll <= 0.0 {
                return *size;
            }
        }
        CreatureSize::Medium
    }

    /// Descriptive label.
    pub fn label(&self) -> &'static str {
        match self {
            CreatureSize::Tiny => "tiny",
            CreatureSize::Small => "small",
            CreatureSize::Medium => "medium",
            CreatureSize::Large => "large",
            CreatureSize::Huge => "huge",
            CreatureSize::Gargantuan => "gargantuan",
            CreatureSize::Colossal => "colossal (kaiju)",
        }
    }
}

/// Intelligence levels for creatures.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Intelligence {
    Mindless,
    Instinctual,
    Cunning,
    Sapient,
    Genius,
}

impl Intelligence {
    /// Whether the creature can lead a population.
    pub fn can_lead(&self) -> bool {
        *self >= Intelligence::Cunning
    }

    /// Whether the creature can communicate with civilizations.
    pub fn can_communicate(&self) -> bool {
        *self >= Intelligence::Sapient
    }
}

/// How a creature moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Locomotion {
    Walking,
    Flying,
    Swimming,
    Burrowing,
    Climbing,
    Slithering,
    Floating,
    Teleporting,
}

/// What a creature eats.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Diet {
    Carnivore,
    Herbivore,
    Omnivore,
    Scavenger,
    Absorber,
    Photosynthetic,
    MagicDrainer,
    None,
}

/// Attack types available to a creature.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AttackType {
    Bite,
    Claw,
    Sting,
    Constrict,
    Trample,
    Gore,
    Spit,
    BreathWeapon,
    MagicBolt,
    Gaze,
    Swallow,
    TailSwipe,
}

/// Defense types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DefenseType {
    ThickHide,
    Shell,
    Scales,
    Regeneration,
    Evasion,
    MagicShield,
    PoisonCloud,
    Camouflage,
    Burrowing,
}

/// Damage types for immunities/vulnerabilities.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DamageType {
    Physical,
    Fire,
    Ice,
    Lightning,
    Poison,
    Acid,
    Magic,
    Holy,
    Necrotic,
}

/// Magic abilities for creatures.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MagicAbility {
    Spellcasting,
    Illusions,
    Shapeshifting,
    Teleportation,
    MindControl,
    Necromancy,
    ElementalControl,
    TimeManipulation,
    CurseWeaving,
    HealingAura,
}

/// Role in a creature population.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PopulationRole {
    Solitary,
    PackMember,
    AlphaLeader,
    HiveWorker,
    HiveQueen,
    Symbiotic,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn test_size_random_weighted() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let mut counts = std::collections::HashMap::new();
        for _ in 0..10000 {
            let size = CreatureSize::random_weighted(&mut rng);
            *counts.entry(size).or_insert(0u32) += 1;
        }
        // Medium should be most common
        assert!(counts[&CreatureSize::Medium] > counts.get(&CreatureSize::Colossal).copied().unwrap_or(0));
        // Colossal should be very rare (< 0.5% of 10000 = < 50)
        let colossal = counts.get(&CreatureSize::Colossal).copied().unwrap_or(0);
        assert!(colossal < 50, "Colossal too common: {} out of 10000", colossal);
    }

    #[test]
    fn test_intelligence_ordering() {
        assert!(Intelligence::Mindless < Intelligence::Genius);
        assert!(Intelligence::Cunning.can_lead());
        assert!(!Intelligence::Instinctual.can_lead());
        assert!(Intelligence::Sapient.can_communicate());
        assert!(!Intelligence::Cunning.can_communicate());
    }

    #[test]
    fn test_body_part_builder() {
        let part = BodyPart::new(BodyPartType::Head, 3, BodyMaterial::Crystal)
            .with_size(BodyPartSize::Large)
            .with_special(BodyPartSpecial::FireBreathing)
            .with_special(BodyPartSpecial::Armored);
        assert_eq!(part.count, 3);
        assert_eq!(part.material, BodyMaterial::Crystal);
        assert_eq!(part.specials.len(), 2);
    }
}
