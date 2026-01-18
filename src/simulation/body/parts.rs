//! Body part definitions and related types
//!
//! Defines the core structures for representing body parts including their
//! categories, sizes, tissues, and functions.

use std::collections::HashSet;

/// Unique identifier for a body part within a body
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BodyPartId(pub u32);

impl BodyPartId {
    pub fn new(id: u32) -> Self {
        Self(id)
    }
}

/// Categories of body parts
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BodyPartCategory {
    Head,
    Torso,
    UpperLimb,
    LowerLimb,
    Extremity,
    Sensory,
    Internal,
    Special,
    Tail,
    Wing,
}

impl BodyPartCategory {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Head => "head",
            Self::Torso => "torso",
            Self::UpperLimb => "upper limb",
            Self::LowerLimb => "lower limb",
            Self::Extremity => "extremity",
            Self::Sensory => "sensory organ",
            Self::Internal => "internal organ",
            Self::Special => "special",
            Self::Tail => "tail",
            Self::Wing => "wing",
        }
    }
}

/// Size of a body part, affecting hit probability
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyPartSize {
    Tiny,   // 5% hit chance weight
    Small,  // 10% hit chance weight
    Medium, // 20% hit chance weight
    Large,  // 25% hit chance weight
    Huge,   // 40% hit chance weight
}

impl BodyPartSize {
    /// Returns the hit chance weight (0.0-1.0 scale factor)
    pub fn hit_weight(&self) -> f32 {
        match self {
            Self::Tiny => 0.05,
            Self::Small => 0.10,
            Self::Medium => 0.20,
            Self::Large => 0.25,
            Self::Huge => 0.40,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Tiny => "tiny",
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
            Self::Huge => "huge",
        }
    }
}

/// Tissue type affects damage resistance and wound types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tissue {
    Flesh,
    Bone,
    Scale,
    Chitin,
    Stone,
    Spirit,
    Ice,
    Fire,
}

impl Tissue {
    /// Returns damage resistance multiplier (lower = more resistant)
    pub fn slash_resistance(&self) -> f32 {
        match self {
            Self::Flesh => 1.0,
            Self::Bone => 0.6,
            Self::Scale => 0.4,
            Self::Chitin => 0.5,
            Self::Stone => 0.2,
            Self::Spirit => 0.1,
            Self::Ice => 0.8,
            Self::Fire => 0.9,
        }
    }

    pub fn blunt_resistance(&self) -> f32 {
        match self {
            Self::Flesh => 0.8,
            Self::Bone => 1.2, // Bones break from blunt
            Self::Scale => 0.7,
            Self::Chitin => 1.0,
            Self::Stone => 0.3,
            Self::Spirit => 0.1,
            Self::Ice => 1.1,
            Self::Fire => 0.5,
        }
    }

    pub fn pierce_resistance(&self) -> f32 {
        match self {
            Self::Flesh => 1.0,
            Self::Bone => 0.4,
            Self::Scale => 0.6,
            Self::Chitin => 0.7,
            Self::Stone => 0.1,
            Self::Spirit => 0.1,
            Self::Ice => 0.9,
            Self::Fire => 0.7,
        }
    }

    pub fn fire_resistance(&self) -> f32 {
        match self {
            Self::Flesh => 1.0,
            Self::Bone => 0.6,
            Self::Scale => 0.5,
            Self::Chitin => 0.8,
            Self::Stone => 0.2,
            Self::Spirit => 0.0,
            Self::Ice => 2.0, // Very vulnerable
            Self::Fire => 0.0, // Immune
        }
    }

    pub fn cold_resistance(&self) -> f32 {
        match self {
            Self::Flesh => 1.0,
            Self::Bone => 0.5,
            Self::Scale => 0.7,
            Self::Chitin => 0.9,
            Self::Stone => 0.3,
            Self::Spirit => 0.0,
            Self::Ice => 0.0, // Immune
            Self::Fire => 2.0, // Very vulnerable
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Flesh => "flesh",
            Self::Bone => "bone",
            Self::Scale => "scales",
            Self::Chitin => "chitin",
            Self::Stone => "stone",
            Self::Spirit => "spirit",
            Self::Ice => "ice",
            Self::Fire => "fire",
        }
    }
}

/// Functions a body part can provide
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BodyPartFunction {
    Locomotion,  // Walking, running
    Grasping,    // Holding weapons, items
    Attacking,   // Claws, bite
    Breathing,   // Required to live
    Thinking,    // Brain - required to live
    Vision,      // Sight
    Hearing,     // Sound detection
    Flight,      // Flying ability
    Balance,     // Affects agility
    FireBreath,  // Dragon breath attack
}

impl BodyPartFunction {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Locomotion => "locomotion",
            Self::Grasping => "grasping",
            Self::Attacking => "attacking",
            Self::Breathing => "breathing",
            Self::Thinking => "thinking",
            Self::Vision => "vision",
            Self::Hearing => "hearing",
            Self::Flight => "flight",
            Self::Balance => "balance",
            Self::FireBreath => "fire breath",
        }
    }
}

/// A single body part
#[derive(Debug, Clone)]
pub struct BodyPart {
    pub id: BodyPartId,
    pub name: String,
    pub category: BodyPartCategory,
    pub size: BodyPartSize,
    pub tissue: Tissue,
    pub functions: HashSet<BodyPartFunction>,
    pub parent: Option<BodyPartId>,
    pub children: Vec<BodyPartId>,
    pub health: f32,
    pub max_health: f32,
    pub is_severed: bool,
    pub vital: bool,
}

impl BodyPart {
    pub fn new(
        id: BodyPartId,
        name: impl Into<String>,
        category: BodyPartCategory,
        size: BodyPartSize,
        tissue: Tissue,
        vital: bool,
    ) -> Self {
        let max_health = match size {
            BodyPartSize::Tiny => 10.0,
            BodyPartSize::Small => 20.0,
            BodyPartSize::Medium => 40.0,
            BodyPartSize::Large => 60.0,
            BodyPartSize::Huge => 100.0,
        };

        Self {
            id,
            name: name.into(),
            category,
            size,
            tissue,
            functions: HashSet::new(),
            parent: None,
            children: Vec::new(),
            health: max_health,
            max_health,
            is_severed: false,
            vital,
        }
    }

    pub fn with_functions(mut self, functions: &[BodyPartFunction]) -> Self {
        self.functions = functions.iter().copied().collect();
        self
    }

    pub fn with_parent(mut self, parent: BodyPartId) -> Self {
        self.parent = Some(parent);
        self
    }

    /// Check if this part is functional (not severed and has health)
    pub fn is_functional(&self) -> bool {
        !self.is_severed && self.health > 0.0
    }

    /// Get the impairment level (0.0 = fully functional, 1.0 = non-functional)
    pub fn impairment(&self) -> f32 {
        if self.is_severed {
            1.0
        } else {
            1.0 - (self.health / self.max_health).clamp(0.0, 1.0)
        }
    }

    /// Check if this part has a specific function
    pub fn has_function(&self, function: BodyPartFunction) -> bool {
        self.functions.contains(&function)
    }

    /// Apply damage to this part, returns true if destroyed
    pub fn apply_damage(&mut self, damage: f32) -> bool {
        self.health = (self.health - damage).max(0.0);
        self.health <= 0.0
    }

    /// Heal this part
    pub fn heal(&mut self, amount: f32) {
        if !self.is_severed {
            self.health = (self.health + amount).min(self.max_health);
        }
    }
}
