//! Society types and government systems
//!
//! Different society types affect tribe behavior, production, military strength,
//! and available options.

use rand::Rng;
use serde::{Deserialize, Serialize};

/// Types of society/government that a tribe can have
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SocietyType {
    /// Religious leadership focused on research and spiritual bonuses
    Theocracy,
    /// Hereditary rule with stability and military focus
    Monarchy,
    /// Elected leaders with balanced production but slow decisions
    Democracy,
    /// Traditional elder council with wisdom bonuses
    TribalCouncil,
    /// Military rule with high aggression and forced labor
    MilitaryDictatorship,
    /// Trade-focused republic with wealth accumulation
    MerchantRepublic,
}

impl SocietyType {
    /// Get a random society type for initialization
    pub fn random<R: Rng>(rng: &mut R) -> Self {
        match rng.gen_range(0..6) {
            0 => SocietyType::Theocracy,
            1 => SocietyType::Monarchy,
            2 => SocietyType::Democracy,
            3 => SocietyType::TribalCouncil,
            4 => SocietyType::MilitaryDictatorship,
            _ => SocietyType::MerchantRepublic,
        }
    }

    /// Get society type weighted by biome/culture (more realistic distribution)
    pub fn random_weighted<R: Rng>(rng: &mut R, is_coastal: bool, is_warlike: bool) -> Self {
        let weights = if is_coastal {
            // Coastal areas favor merchant republics
            [10, 15, 15, 20, 5, 35] // Theocracy, Monarchy, Democracy, TribalCouncil, Military, Merchant
        } else if is_warlike {
            // Warlike cultures favor military governments
            [5, 25, 5, 15, 40, 10]
        } else {
            // Default distribution
            [15, 20, 10, 30, 10, 15]
        };

        let total: u32 = weights.iter().sum();
        let roll = rng.gen_range(0..total);
        let mut cumulative = 0;

        for (i, &weight) in weights.iter().enumerate() {
            cumulative += weight;
            if roll < cumulative {
                return match i {
                    0 => SocietyType::Theocracy,
                    1 => SocietyType::Monarchy,
                    2 => SocietyType::Democracy,
                    3 => SocietyType::TribalCouncil,
                    4 => SocietyType::MilitaryDictatorship,
                    _ => SocietyType::MerchantRepublic,
                };
            }
        }

        SocietyType::TribalCouncil
    }

    /// Get the configuration for this society type
    pub fn config(&self) -> SocietyConfig {
        match self {
            SocietyType::Theocracy => SocietyConfig {
                name: "Theocracy",
                production_mult: 0.9,
                military_mult: 1.0,
                research_mult: 1.3,
                trade_mult: 0.9,
                expansion_mult: 0.8,
                morale_bonus: 0.1,
                succession: SuccessionMethod::Divine,
                special_bonuses: vec![SpecialBonus::TempleBonus, SpecialBonus::ResearchBoost],
            },
            SocietyType::Monarchy => SocietyConfig {
                name: "Monarchy",
                production_mult: 1.0,
                military_mult: 1.2,
                research_mult: 1.0,
                trade_mult: 1.0,
                expansion_mult: 1.1,
                morale_bonus: 0.0,
                succession: SuccessionMethod::Hereditary,
                special_bonuses: vec![SpecialBonus::StabilityBonus],
            },
            SocietyType::Democracy => SocietyConfig {
                name: "Democracy",
                production_mult: 1.1,
                military_mult: 0.9,
                research_mult: 1.1,
                trade_mult: 1.1,
                expansion_mult: 0.9,
                morale_bonus: 0.15,
                succession: SuccessionMethod::Election,
                special_bonuses: vec![SpecialBonus::HappinessBonus],
            },
            SocietyType::TribalCouncil => SocietyConfig {
                name: "Tribal Council",
                production_mult: 0.9,
                military_mult: 1.0,
                research_mult: 1.0,
                trade_mult: 0.9,
                expansion_mult: 1.0,
                morale_bonus: 0.05,
                succession: SuccessionMethod::ElderCouncil,
                special_bonuses: vec![SpecialBonus::WisdomBonus, SpecialBonus::TraditionBonus],
            },
            SocietyType::MilitaryDictatorship => SocietyConfig {
                name: "Military Dictatorship",
                production_mult: 0.8,
                military_mult: 1.5,
                research_mult: 0.7,
                trade_mult: 0.7,
                expansion_mult: 1.4,
                morale_bonus: -0.1,
                succession: SuccessionMethod::Coup,
                special_bonuses: vec![SpecialBonus::ForcedLabor, SpecialBonus::AggressionBonus],
            },
            SocietyType::MerchantRepublic => SocietyConfig {
                name: "Merchant Republic",
                production_mult: 1.0,
                military_mult: 0.8,
                research_mult: 1.0,
                trade_mult: 1.5,
                expansion_mult: 0.8,
                morale_bonus: 0.1,
                succession: SuccessionMethod::WealthElection,
                special_bonuses: vec![SpecialBonus::TradeBonus, SpecialBonus::WealthAccumulation],
            },
        }
    }

    /// Get display name
    pub fn name(&self) -> &'static str {
        self.config().name
    }

    /// Get all society types
    pub fn all() -> &'static [SocietyType] {
        &[
            SocietyType::Theocracy,
            SocietyType::Monarchy,
            SocietyType::Democracy,
            SocietyType::TribalCouncil,
            SocietyType::MilitaryDictatorship,
            SocietyType::MerchantRepublic,
        ]
    }
}

impl Default for SocietyType {
    fn default() -> Self {
        SocietyType::TribalCouncil
    }
}

/// Configuration values for a society type
#[derive(Clone, Debug)]
pub struct SocietyConfig {
    /// Display name
    pub name: &'static str,
    /// Multiplier for resource production (1.0 = normal)
    pub production_mult: f32,
    /// Multiplier for military strength
    pub military_mult: f32,
    /// Multiplier for research speed
    pub research_mult: f32,
    /// Multiplier for trade efficiency
    pub trade_mult: f32,
    /// Multiplier for territorial expansion
    pub expansion_mult: f32,
    /// Bonus/penalty to base morale
    pub morale_bonus: f32,
    /// How leaders are chosen
    pub succession: SuccessionMethod,
    /// Special bonuses unique to this society type
    pub special_bonuses: Vec<SpecialBonus>,
}

/// How leadership succession occurs
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SuccessionMethod {
    /// Leaders are chosen by divine signs/priests
    Divine,
    /// Leadership passes to family members
    Hereditary,
    /// Leaders are elected by citizens
    Election,
    /// Elders choose the next leader
    ElderCouncil,
    /// Strongest military leader takes power
    Coup,
    /// Wealthiest citizens choose among themselves
    WealthElection,
}

impl SuccessionMethod {
    /// Get the stability modifier for this succession method
    /// Higher = more stable, less chance of unrest
    pub fn stability_modifier(&self) -> f32 {
        match self {
            SuccessionMethod::Divine => 0.9,
            SuccessionMethod::Hereditary => 1.1,
            SuccessionMethod::Election => 1.0,
            SuccessionMethod::ElderCouncil => 1.0,
            SuccessionMethod::Coup => 0.7,
            SuccessionMethod::WealthElection => 0.85,
        }
    }

    /// Duration of succession crisis (in ticks)
    pub fn succession_crisis_duration(&self) -> u64 {
        match self {
            SuccessionMethod::Divine => 2,      // Quick divine selection
            SuccessionMethod::Hereditary => 1,  // Very quick, clear heir
            SuccessionMethod::Election => 4,    // Election takes time
            SuccessionMethod::ElderCouncil => 3,
            SuccessionMethod::Coup => 8,        // Civil unrest possible
            SuccessionMethod::WealthElection => 4,
        }
    }
}

/// Special bonuses that society types can provide
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SpecialBonus {
    /// Temples provide additional bonuses
    TempleBonus,
    /// Research is faster
    ResearchBoost,
    /// Less unrest, more predictable
    StabilityBonus,
    /// Population is happier
    HappinessBonus,
    /// Elders provide wisdom bonuses
    WisdomBonus,
    /// Traditional methods work better
    TraditionBonus,
    /// Can force workers to work harder
    ForcedLabor,
    /// More aggressive in conflicts
    AggressionBonus,
    /// Better trade deals
    TradeBonus,
    /// Accumulates wealth faster
    WealthAccumulation,
}

impl SpecialBonus {
    /// Get the numeric effect of this bonus
    pub fn effect_value(&self) -> f32 {
        match self {
            SpecialBonus::TempleBonus => 0.2,       // +20% temple effectiveness
            SpecialBonus::ResearchBoost => 0.15,   // +15% research speed
            SpecialBonus::StabilityBonus => 0.25,  // +25% stability
            SpecialBonus::HappinessBonus => 0.1,   // +10% morale
            SpecialBonus::WisdomBonus => 0.1,      // +10% from elders
            SpecialBonus::TraditionBonus => 0.1,   // +10% traditional methods
            SpecialBonus::ForcedLabor => 0.2,      // +20% production (at morale cost)
            SpecialBonus::AggressionBonus => 0.2,  // +20% raid success
            SpecialBonus::TradeBonus => 0.25,      // +25% trade value
            SpecialBonus::WealthAccumulation => 0.15, // +15% wealth gain
        }
    }

    /// Get the description of this bonus
    pub fn description(&self) -> &'static str {
        match self {
            SpecialBonus::TempleBonus => "Temples provide additional research and morale",
            SpecialBonus::ResearchBoost => "Increased research output",
            SpecialBonus::StabilityBonus => "Reduced chance of unrest and rebellion",
            SpecialBonus::HappinessBonus => "Population is generally happier",
            SpecialBonus::WisdomBonus => "Elder knowledge improves decisions",
            SpecialBonus::TraditionBonus => "Traditional methods are more effective",
            SpecialBonus::ForcedLabor => "Can extract more labor at morale cost",
            SpecialBonus::AggressionBonus => "More effective in raids and wars",
            SpecialBonus::TradeBonus => "Better prices in trade deals",
            SpecialBonus::WealthAccumulation => "Accumulates gold and luxury faster",
        }
    }
}

/// Society state tracking leadership and government
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SocietyState {
    /// Current society type
    pub society_type: SocietyType,
    /// Current leader (colonist ID if tracked individually)
    pub leader_id: Option<u64>,
    /// Leader's name (for display)
    pub leader_name: String,
    /// Leader's age (in years)
    pub leader_age: u32,
    /// Ticks the current leader has been in power
    pub leader_tenure: u64,
    /// Whether currently in succession crisis
    pub in_succession_crisis: bool,
    /// Ticks remaining in succession crisis
    pub succession_crisis_ticks: u64,
    /// Stability level (0.0 - 1.0)
    pub stability: f32,
    /// Unrest level (0.0 - 1.0)
    pub unrest: f32,
    /// Revolution progress (0.0 - 1.0, at 1.0 government changes)
    pub revolution_progress: f32,
}

impl SocietyState {
    /// Create a new society state
    pub fn new(society_type: SocietyType, leader_name: String) -> Self {
        SocietyState {
            society_type,
            leader_id: None,
            leader_name,
            leader_age: 25 + rand::random::<u32>() % 30, // 25-54 years old
            leader_tenure: 0,
            in_succession_crisis: false,
            succession_crisis_ticks: 0,
            stability: 0.7,
            unrest: 0.0,
            revolution_progress: 0.0,
        }
    }

    /// Get the current society config
    pub fn config(&self) -> SocietyConfig {
        self.society_type.config()
    }

    /// Check if society has a specific bonus
    pub fn has_bonus(&self, bonus: SpecialBonus) -> bool {
        self.config().special_bonuses.contains(&bonus)
    }

    /// Update society state each tick
    pub fn tick(&mut self) {
        self.leader_tenure += 1;

        // Age leader each year (4 ticks)
        if self.leader_tenure % 4 == 0 {
            self.leader_age += 1;
        }

        // Handle succession crisis countdown
        if self.in_succession_crisis && self.succession_crisis_ticks > 0 {
            self.succession_crisis_ticks -= 1;
            if self.succession_crisis_ticks == 0 {
                self.in_succession_crisis = false;
            }
        }

        // Drift stability towards normal
        let target_stability = self.society_type.config().succession.stability_modifier();
        self.stability += (target_stability - self.stability) * 0.05;

        // Unrest naturally decreases
        self.unrest *= 0.95;

        // Revolution progress decreases if stability is high
        if self.stability > 0.5 {
            self.revolution_progress *= 0.98;
        }
    }

    /// Trigger a succession (leader death or removal)
    pub fn trigger_succession(&mut self) {
        self.in_succession_crisis = true;
        self.succession_crisis_ticks = self.config().succession.succession_crisis_duration();
        self.leader_tenure = 0;
        self.stability *= 0.7; // Stability hit during transition
    }

    /// Set a new leader
    pub fn set_leader(&mut self, id: Option<u64>, name: String, age: u32) {
        self.leader_id = id;
        self.leader_name = name;
        self.leader_age = age;
        self.leader_tenure = 0;
        self.in_succession_crisis = false;
        self.succession_crisis_ticks = 0;
    }

    /// Add unrest (from various causes)
    pub fn add_unrest(&mut self, amount: f32) {
        self.unrest = (self.unrest + amount).clamp(0.0, 1.0);

        // High unrest increases revolution progress
        if self.unrest > 0.5 {
            self.revolution_progress += (self.unrest - 0.5) * 0.1;
        }
    }

    /// Check if a revolution should occur
    pub fn should_revolt(&self) -> bool {
        self.revolution_progress >= 1.0
    }

    /// Get production modifier including society effects
    pub fn production_modifier(&self) -> f32 {
        let base = self.config().production_mult;
        let stability_mod = if self.in_succession_crisis { 0.7 } else { 1.0 };
        let unrest_mod = 1.0 - (self.unrest * 0.3);

        // Forced labor bonus
        let forced_labor = if self.has_bonus(SpecialBonus::ForcedLabor) {
            1.0 + SpecialBonus::ForcedLabor.effect_value()
        } else {
            1.0
        };

        base * stability_mod * unrest_mod * forced_labor
    }

    /// Get military modifier including society effects
    pub fn military_modifier(&self) -> f32 {
        let base = self.config().military_mult;
        let stability_mod = if self.in_succession_crisis { 0.6 } else { 1.0 };

        // Aggression bonus
        let aggression = if self.has_bonus(SpecialBonus::AggressionBonus) {
            1.0 + SpecialBonus::AggressionBonus.effect_value()
        } else {
            1.0
        };

        base * stability_mod * aggression
    }

    /// Get research modifier including society effects
    pub fn research_modifier(&self) -> f32 {
        let base = self.config().research_mult;

        // Research boost
        let boost = if self.has_bonus(SpecialBonus::ResearchBoost) {
            1.0 + SpecialBonus::ResearchBoost.effect_value()
        } else {
            1.0
        };

        // Wisdom bonus from elders
        let wisdom = if self.has_bonus(SpecialBonus::WisdomBonus) {
            1.0 + SpecialBonus::WisdomBonus.effect_value()
        } else {
            1.0
        };

        base * boost * wisdom
    }

    /// Get trade modifier including society effects
    pub fn trade_modifier(&self) -> f32 {
        let base = self.config().trade_mult;

        // Trade bonus
        let bonus = if self.has_bonus(SpecialBonus::TradeBonus) {
            1.0 + SpecialBonus::TradeBonus.effect_value()
        } else {
            1.0
        };

        base * bonus
    }

    /// Get morale modifier from society type
    pub fn morale_modifier(&self) -> f32 {
        let base = self.config().morale_bonus;

        // Happiness bonus
        let happiness = if self.has_bonus(SpecialBonus::HappinessBonus) {
            SpecialBonus::HappinessBonus.effect_value()
        } else {
            0.0
        };

        // Forced labor penalty
        let forced_penalty = if self.has_bonus(SpecialBonus::ForcedLabor) {
            -0.1
        } else {
            0.0
        };

        base + happiness + forced_penalty - (self.unrest * 0.2)
    }
}

impl Default for SocietyState {
    fn default() -> Self {
        SocietyState::new(SocietyType::TribalCouncil, "Chief".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_society_config() {
        for society_type in SocietyType::all() {
            let config = society_type.config();
            assert!(config.production_mult > 0.0);
            assert!(config.military_mult > 0.0);
        }
    }

    #[test]
    fn test_society_state_tick() {
        let mut state = SocietyState::new(SocietyType::Monarchy, "King Arthur".to_string());
        let initial_tenure = state.leader_tenure;
        state.tick();
        assert_eq!(state.leader_tenure, initial_tenure + 1);
    }

    #[test]
    fn test_succession_crisis() {
        let mut state = SocietyState::new(SocietyType::Monarchy, "King Arthur".to_string());
        state.trigger_succession();
        assert!(state.in_succession_crisis);
        assert!(state.succession_crisis_ticks > 0);
    }
}
