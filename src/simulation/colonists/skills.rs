//! Skill system for colonists
//!
//! Dwarf Fortress-style skills on a 0-20 scale with experience-based progression.

use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Types of skills colonists can have
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SkillType {
    // Production skills
    Farming,
    Mining,
    Woodcutting,
    Fishing,
    Hunting,

    // Crafting skills
    Smithing,
    Crafting,
    Cooking,
    Building,

    // Knowledge skills
    Research,
    Medicine,

    // Social/Combat skills
    Combat,
    Leadership,
    Trading,
}

impl SkillType {
    /// Get all skill types
    pub fn all() -> &'static [SkillType] {
        &[
            SkillType::Farming,
            SkillType::Mining,
            SkillType::Woodcutting,
            SkillType::Fishing,
            SkillType::Hunting,
            SkillType::Smithing,
            SkillType::Crafting,
            SkillType::Cooking,
            SkillType::Building,
            SkillType::Research,
            SkillType::Medicine,
            SkillType::Combat,
            SkillType::Leadership,
            SkillType::Trading,
        ]
    }

    /// Get the display name for this skill
    pub fn name(&self) -> &'static str {
        match self {
            SkillType::Farming => "Farming",
            SkillType::Mining => "Mining",
            SkillType::Woodcutting => "Woodcutting",
            SkillType::Fishing => "Fishing",
            SkillType::Hunting => "Hunting",
            SkillType::Smithing => "Smithing",
            SkillType::Crafting => "Crafting",
            SkillType::Cooking => "Cooking",
            SkillType::Building => "Building",
            SkillType::Research => "Research",
            SkillType::Medicine => "Medicine",
            SkillType::Combat => "Combat",
            SkillType::Leadership => "Leadership",
            SkillType::Trading => "Trading",
        }
    }

    /// Get the category of this skill
    pub fn category(&self) -> SkillCategory {
        match self {
            SkillType::Farming | SkillType::Mining | SkillType::Woodcutting |
            SkillType::Fishing | SkillType::Hunting => SkillCategory::Production,

            SkillType::Smithing | SkillType::Crafting | SkillType::Cooking |
            SkillType::Building => SkillCategory::Crafting,

            SkillType::Research | SkillType::Medicine => SkillCategory::Knowledge,

            SkillType::Combat | SkillType::Leadership | SkillType::Trading => SkillCategory::Social,
        }
    }

    /// Experience required to reach each level
    pub fn exp_for_level(level: u8) -> u32 {
        // Exponential scaling: each level requires more XP
        // Level 1: 100, Level 5: 500, Level 10: 2500, Level 15: 8000, Level 20: 20000
        match level {
            0 => 0,
            1 => 100,
            2 => 200,
            3 => 350,
            4 => 500,
            5 => 700,
            6 => 1000,
            7 => 1400,
            8 => 1900,
            9 => 2500,
            10 => 3500,
            11 => 4700,
            12 => 6000,
            13 => 7500,
            14 => 9500,
            15 => 12000,
            16 => 14500,
            17 => 17000,
            18 => 19500,
            19 => 22000,
            _ => 25000,
        }
    }
}

/// Categories of skills
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SkillCategory {
    /// Resource gathering
    Production,
    /// Item creation
    Crafting,
    /// Research and healing
    Knowledge,
    /// Combat and social
    Social,
}

/// Skill level names (DF-style)
pub fn skill_level_name(level: u8) -> &'static str {
    match level {
        0 => "Dabbling",
        1..=2 => "Novice",
        3..=4 => "Adequate",
        5..=6 => "Competent",
        7..=8 => "Skilled",
        9..=10 => "Proficient",
        11..=12 => "Talented",
        13..=14 => "Adept",
        15..=16 => "Expert",
        17..=18 => "Master",
        19..=20 => "Legendary",
        _ => "Legendary",
    }
}

/// Get productivity multiplier for a skill level
/// Level 0 = 0.5x, Level 10 = 1.5x, Level 20 = 3.0x
pub fn skill_productivity(level: u8) -> f32 {
    match level {
        0 => 0.5,
        1 => 0.6,
        2 => 0.7,
        3 => 0.8,
        4 => 0.9,
        5 => 1.0,
        6 => 1.1,
        7 => 1.2,
        8 => 1.3,
        9 => 1.4,
        10 => 1.5,
        11 => 1.6,
        12 => 1.7,
        13 => 1.8,
        14 => 1.9,
        15 => 2.0,
        16 => 2.2,
        17 => 2.4,
        18 => 2.6,
        19 => 2.8,
        _ => 3.0,
    }
}

/// Individual skill with level and experience
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Skill {
    pub skill_type: SkillType,
    pub level: u8,
    pub experience: u32,
}

impl Skill {
    pub fn new(skill_type: SkillType) -> Self {
        Skill {
            skill_type,
            level: 0,
            experience: 0,
        }
    }

    /// Add experience and check for level up
    /// Returns true if leveled up
    pub fn add_experience(&mut self, amount: u32) -> bool {
        if self.level >= 20 {
            return false; // Already max level
        }

        self.experience += amount;

        // Check for level up with diminishing returns at higher levels
        let required = SkillType::exp_for_level(self.level + 1);
        if self.experience >= required {
            self.level += 1;
            self.experience = 0; // Reset for next level
            true
        } else {
            false
        }
    }

    /// Get productivity multiplier for this skill
    pub fn productivity(&self) -> f32 {
        skill_productivity(self.level)
    }

    /// Get the level name
    pub fn level_name(&self) -> &'static str {
        skill_level_name(self.level)
    }

    /// Progress to next level (0.0 - 1.0)
    pub fn progress(&self) -> f32 {
        if self.level >= 20 {
            1.0
        } else {
            let required = SkillType::exp_for_level(self.level + 1);
            self.experience as f32 / required as f32
        }
    }
}

/// Collection of all skills for a colonist
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkillSet {
    skills: HashMap<SkillType, Skill>,
}

impl SkillSet {
    /// Create a new empty skill set
    pub fn new() -> Self {
        let mut skills = HashMap::new();
        for skill_type in SkillType::all() {
            skills.insert(*skill_type, Skill::new(*skill_type));
        }
        SkillSet { skills }
    }

    /// Create a skill set with random starting levels
    pub fn random<R: Rng>(rng: &mut R) -> Self {
        let mut set = Self::new();

        // Give 2-4 skills a starting bonus
        let num_bonuses = rng.gen_range(2..=4);
        let all_skills = SkillType::all();

        for _ in 0..num_bonuses {
            let skill_type = all_skills[rng.gen_range(0..all_skills.len())];
            let level = rng.gen_range(1..=5);
            if let Some(skill) = set.skills.get_mut(&skill_type) {
                skill.level = skill.level.max(level);
            }
        }

        set
    }

    /// Get skill level
    pub fn get_level(&self, skill_type: SkillType) -> u8 {
        self.skills.get(&skill_type).map(|s| s.level).unwrap_or(0)
    }

    /// Get skill reference
    pub fn get(&self, skill_type: SkillType) -> Option<&Skill> {
        self.skills.get(&skill_type)
    }

    /// Get mutable skill reference
    pub fn get_mut(&mut self, skill_type: SkillType) -> Option<&mut Skill> {
        self.skills.get_mut(&skill_type)
    }

    /// Add experience to a skill
    pub fn add_experience(&mut self, skill_type: SkillType, amount: u32) -> bool {
        if let Some(skill) = self.skills.get_mut(&skill_type) {
            skill.add_experience(amount)
        } else {
            false
        }
    }

    /// Get productivity for a skill
    pub fn productivity(&self, skill_type: SkillType) -> f32 {
        self.skills.get(&skill_type).map(|s| s.productivity()).unwrap_or(0.5)
    }

    /// Get the highest level skill
    pub fn best_skill(&self) -> Option<(&SkillType, &Skill)> {
        self.skills.iter()
            .max_by_key(|(_, s)| s.level)
    }

    /// Get all skills above a certain level
    pub fn skills_above_level(&self, min_level: u8) -> Vec<(&SkillType, &Skill)> {
        self.skills.iter()
            .filter(|(_, s)| s.level >= min_level)
            .collect()
    }

    /// Get all skills in a category
    pub fn skills_in_category(&self, category: SkillCategory) -> Vec<(&SkillType, &Skill)> {
        self.skills.iter()
            .filter(|(t, _)| t.category() == category)
            .collect()
    }

    /// Get average skill level
    pub fn average_level(&self) -> f32 {
        let total: u32 = self.skills.values().map(|s| s.level as u32).sum();
        total as f32 / self.skills.len() as f32
    }

    /// Get the best skill for a job type
    pub fn best_for_job(&self, job_type: crate::simulation::jobs::types::JobType) -> u8 {
        use crate::simulation::jobs::types::JobType;

        let skill_type = match job_type {
            JobType::Farmer => SkillType::Farming,
            JobType::Miner => SkillType::Mining,
            JobType::Woodcutter => SkillType::Woodcutting,
            JobType::Fisher => SkillType::Fishing,
            JobType::Hunter => SkillType::Hunting,
            JobType::Smith => SkillType::Smithing,
            JobType::Craftsperson => SkillType::Crafting,
            JobType::Cook => SkillType::Cooking,
            JobType::Builder => SkillType::Building,
            JobType::Scholar => SkillType::Research,
            JobType::Healer => SkillType::Medicine,
            JobType::Guard | JobType::Warrior | JobType::Scout => SkillType::Combat,
            JobType::Priest => SkillType::Research,
            JobType::Hauler | JobType::Idle => return 5, // Default for unskilled jobs
        };

        self.get_level(skill_type)
    }
}

impl Default for SkillSet {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_experience() {
        let mut skill = Skill::new(SkillType::Farming);
        assert_eq!(skill.level, 0);

        // Add enough XP for level 1
        assert!(skill.add_experience(100));
        assert_eq!(skill.level, 1);
        assert_eq!(skill.experience, 0);
    }

    #[test]
    fn test_skill_set_random() {
        let mut rng = rand::thread_rng();
        let set = SkillSet::random(&mut rng);

        // Should have at least one skill above 0
        let skills_above_0 = set.skills_above_level(1);
        assert!(!skills_above_0.is_empty());
    }

    #[test]
    fn test_skill_productivity() {
        assert!(skill_productivity(0) < skill_productivity(10));
        assert!(skill_productivity(10) < skill_productivity(20));
    }
}
