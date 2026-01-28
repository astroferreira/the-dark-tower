//! Personality, skill, and ability definitions for figures.

use serde::{Serialize, Deserialize};
use rand::Rng;

/// Personality traits for notable figures (all values 0.0 to 1.0).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Personality {
    pub bravery: f32,
    pub cruelty: f32,
    pub ambition: f32,
    pub honor: f32,
    pub piety: f32,
    pub cunning: f32,
    pub charisma: f32,
    pub paranoia: f32,
    pub patience: f32,
    pub greed: f32,
}

impl Personality {
    /// Generate a random personality.
    pub fn random(rng: &mut impl Rng) -> Self {
        Self {
            bravery: rng.gen(),
            cruelty: rng.gen(),
            ambition: rng.gen(),
            honor: rng.gen(),
            piety: rng.gen(),
            cunning: rng.gen(),
            charisma: rng.gen(),
            paranoia: rng.gen(),
            patience: rng.gen(),
            greed: rng.gen(),
        }
    }

    /// Generate a personality biased by cultural values.
    pub fn from_culture(culture_values: &super::culture::CultureValues, rng: &mut impl Rng) -> Self {
        fn bias(base: f32, rng: &mut impl Rng) -> f32 {
            let random: f32 = rng.gen();
            (base * 0.7 + random * 0.3).clamp(0.0, 1.0)
        }

        Self {
            bravery: bias(culture_values.martial, rng),
            cruelty: bias(1.0 - culture_values.honor_value, rng),
            ambition: bias(culture_values.wealth, rng),
            honor: bias(culture_values.honor_value, rng),
            piety: bias(culture_values.tradition, rng),
            cunning: bias(1.0 - culture_values.tradition, rng),
            charisma: rng.gen(),
            paranoia: bias(culture_values.xenophobia, rng),
            patience: bias(culture_values.collectivism, rng),
            greed: bias(culture_values.wealth, rng),
        }
    }

    // === Composite personality scoring methods ===
    // These combine multiple traits into decision-relevant scores (0.0–1.0).

    /// Inclination toward starting wars: bravery + ambition + (1-patience) + cruelty, averaged.
    pub fn war_inclination(&self) -> f32 {
        (self.bravery + self.ambition + (1.0 - self.patience) + self.cruelty) / 4.0
    }

    /// Inclination toward peaceful diplomacy: cunning + patience + charisma + honor, averaged.
    pub fn diplomacy_inclination(&self) -> f32 {
        (self.cunning + self.patience + self.charisma + self.honor) / 4.0
    }

    /// Religious fervor: piety + honor + (1-cunning), averaged.
    pub fn religious_fervor(&self) -> f32 {
        (self.piety + self.honor + (1.0 - self.cunning)) / 3.0
    }

    /// Drive to accumulate wealth: greed + ambition + cunning, averaged.
    pub fn wealth_drive(&self) -> f32 {
        (self.greed + self.ambition + self.cunning) / 3.0
    }

    /// Tendency toward tyranny: cruelty + paranoia + greed + (1-honor), averaged.
    pub fn tyranny(&self) -> f32 {
        (self.cruelty + self.paranoia + self.greed + (1.0 - self.honor)) / 4.0
    }

    /// Inclination to build monuments/temples: piety + ambition + patience + (1-greed), averaged.
    pub fn builder_inclination(&self) -> f32 {
        (self.piety + self.ambition + self.patience + (1.0 - self.greed)) / 4.0
    }

    /// Convert a composite score (0.0–1.0) into a multiplier in range [min_mult, max_mult].
    /// score=0.5 → 1.0 (neutral), score=0.0 → min_mult, score=1.0 → max_mult.
    pub fn score_to_multiplier(score: f32, min_mult: f32, max_mult: f32) -> f32 {
        min_mult + score * (max_mult - min_mult)
    }

    /// Dominant trait description for display.
    pub fn dominant_trait(&self) -> &'static str {
        let traits = [
            (self.bravery, "brave"),
            (self.cruelty, "cruel"),
            (self.ambition, "ambitious"),
            (self.honor, "honorable"),
            (self.piety, "pious"),
            (self.cunning, "cunning"),
            (self.charisma, "charismatic"),
            (self.paranoia, "paranoid"),
            (self.patience, "patient"),
            (self.greed, "greedy"),
        ];

        traits.iter()
            .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
            .map(|t| t.1)
            .unwrap_or("unremarkable")
    }
}

impl Default for Personality {
    fn default() -> Self {
        Self {
            bravery: 0.5,
            cruelty: 0.5,
            ambition: 0.5,
            honor: 0.5,
            piety: 0.5,
            cunning: 0.5,
            charisma: 0.5,
            paranoia: 0.5,
            patience: 0.5,
            greed: 0.5,
        }
    }
}

/// Skills that figures can possess.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Skill {
    Combat,
    Leadership,
    Diplomacy,
    Strategy,
    Crafting,
    Magic,
    Lore,
    Stealth,
    Healing,
    Navigation,
    Survival,
    Persuasion,
    Engineering,
    Farming,
    Mining,
    Trading,
}

impl Skill {
    pub fn all() -> &'static [Skill] {
        &[
            Skill::Combat, Skill::Leadership, Skill::Diplomacy, Skill::Strategy,
            Skill::Crafting, Skill::Magic, Skill::Lore, Skill::Stealth,
            Skill::Healing, Skill::Navigation, Skill::Survival, Skill::Persuasion,
            Skill::Engineering, Skill::Farming, Skill::Mining, Skill::Trading,
        ]
    }
}

/// Special abilities (innate or learned).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Ability {
    // Racial innate
    DarkVision,
    NaturalArmor,
    WaterBreathing,
    FlightCapable,
    PoisonResistance,
    ColdResistance,
    HeatResistance,
    Regeneration,
    Longevity,
    StoneAffinity,

    // Learned/granted
    BattleRage,
    TacticalGenius,
    MasterCrafter,
    DivineFavor,
    ArcaneGift,
    BeastSpeaker,
    Shapeshifter,
    ShadowWalker,
    MountainEndurance,
    SeafaringExpert,
}

/// Cause of death for a figure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeathCause {
    Natural,
    Battle,
    Assassination,
    Execution,
    Duel,
    Monster,
    Disease,
    Magic,
    Accident,
    Suicide,
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn test_random_personality() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let p = Personality::random(&mut rng);
        assert!((0.0..=1.0).contains(&p.bravery));
        assert!((0.0..=1.0).contains(&p.cruelty));
        assert!(!p.dominant_trait().is_empty());
    }

    #[test]
    fn test_composite_scores_range() {
        let mut rng = ChaCha8Rng::seed_from_u64(99);
        for _ in 0..100 {
            let p = Personality::random(&mut rng);
            assert!((0.0..=1.0).contains(&p.war_inclination()));
            assert!((0.0..=1.0).contains(&p.diplomacy_inclination()));
            assert!((0.0..=1.0).contains(&p.religious_fervor()));
            assert!((0.0..=1.0).contains(&p.wealth_drive()));
            assert!((0.0..=1.0).contains(&p.tyranny()));
            assert!((0.0..=1.0).contains(&p.builder_inclination()));
        }
    }

    #[test]
    fn test_score_to_multiplier() {
        assert!((Personality::score_to_multiplier(0.0, 0.5, 2.0) - 0.5).abs() < 0.001);
        assert!((Personality::score_to_multiplier(0.5, 0.5, 2.0) - 1.25).abs() < 0.001);
        assert!((Personality::score_to_multiplier(1.0, 0.5, 2.0) - 2.0).abs() < 0.001);
    }
}
