//! Damage calculation and application
//!
//! Handles damage types, tissue interactions, and wound creation.

use rand::Rng;

use crate::simulation::body::{
    Body, BodyPartId, DamageType, Tissue, Wound, WoundSeverity, WoundType, CombatEffect,
};

/// Result of applying damage to a body part
#[derive(Debug)]
pub struct DamageResult {
    pub final_damage: f32,
    pub wound: Option<Wound>,
    pub part_destroyed: bool,
    pub effects: Vec<CombatEffect>,
}

/// Calculate tissue resistance for a damage type
pub fn tissue_resistance(tissue: Tissue, damage_type: DamageType) -> f32 {
    match damage_type {
        DamageType::Slash => tissue.slash_resistance(),
        DamageType::Blunt => tissue.blunt_resistance(),
        DamageType::Pierce => tissue.pierce_resistance(),
        DamageType::Fire => tissue.fire_resistance(),
        DamageType::Cold => tissue.cold_resistance(),
        DamageType::Poison => 1.0, // Poison ignores physical resistance
    }
}

/// Apply damage to a body part and create appropriate wound
pub fn apply_damage_to_part<R: Rng>(
    body: &mut Body,
    part_id: BodyPartId,
    raw_damage: f32,
    damage_type: DamageType,
    tick: u64,
    rng: &mut R,
) -> Option<DamageResult> {
    let part = body.get_part(part_id)?;

    // Calculate resistance
    let resistance = tissue_resistance(part.tissue, damage_type);
    let resisted_damage = raw_damage * resistance;

    // Some randomization
    let variance = rng.gen_range(0.8..1.2);
    let final_damage = resisted_damage * variance;

    // Get part info before mutable borrow
    let part_name = part.name.clone();
    let part_vital = part.vital;
    let part_max_health = part.max_health;

    // Apply damage
    let part = body.get_part_mut(part_id)?;
    let part_destroyed = part.apply_damage(final_damage);

    // Determine wound severity
    let damage_ratio = final_damage / part_max_health;
    let severity = WoundSeverity::from_damage_ratio(damage_ratio);

    // Create wound
    let wound_type = if part_destroyed {
        if damage_type == DamageType::Slash && severity == WoundSeverity::Critical {
            WoundType::Severed
        } else {
            WoundType::Destroyed
        }
    } else {
        damage_type.wound_type(severity)
    };

    let wound = Wound::new(wound_type, severity, damage_type, tick);

    // Handle bleeding
    if wound.bleeding_rate > 0.0 {
        body.add_bleeding(wound.bleeding_rate);
    }

    // Determine effects
    let mut effects = Vec::new();

    if part_destroyed {
        if wound_type == WoundType::Severed {
            if let Some(part) = body.get_part_mut(part_id) {
                part.is_severed = true;
            }
            effects.push(CombatEffect::LimbSevered {
                part_name: part_name.clone(),
            });
        }

        if part_vital {
            effects.push(CombatEffect::Dead {
                cause: format!("{} destroyed", part_name),
            });
        }
    } else {
        // Non-lethal effects based on severity and damage
        match severity {
            WoundSeverity::Critical => {
                // High chance of stagger or knockdown
                if rng.gen_bool(0.7) {
                    effects.push(CombatEffect::Knockdown);
                } else {
                    effects.push(CombatEffect::Staggered);
                }
            }
            WoundSeverity::Severe => {
                // Moderate chance of stagger
                if rng.gen_bool(0.5) {
                    effects.push(CombatEffect::Staggered);
                }
            }
            WoundSeverity::Moderate => {
                // Small chance of stagger
                if rng.gen_bool(0.2) {
                    effects.push(CombatEffect::Staggered);
                }
            }
            WoundSeverity::Minor => {
                // No effect
            }
        }

        // Special effects for certain wound types
        if wound_type == WoundType::Fracture || wound_type == WoundType::CompoundFracture {
            effects.push(CombatEffect::Stunned);
        }
    }

    Some(DamageResult {
        final_damage,
        wound: Some(wound),
        part_destroyed,
        effects,
    })
}

/// Select a target body part based on size weights
pub fn select_target_part<R: Rng>(body: &Body, rng: &mut R) -> Option<BodyPartId> {
    let targetable: Vec<_> = body.targetable_parts();
    if targetable.is_empty() {
        return None;
    }

    // Calculate total weight
    let total_weight: f32 = targetable.iter().map(|p| p.size.hit_weight()).sum();

    // Random selection weighted by size
    let mut roll = rng.gen_range(0.0..total_weight);
    for part in &targetable {
        roll -= part.size.hit_weight();
        if roll <= 0.0 {
            return Some(part.id);
        }
    }

    // Fallback to first part
    targetable.first().map(|p| p.id)
}

/// Select a specific category of body part if available
pub fn select_part_by_category<R: Rng>(
    body: &Body,
    category: crate::simulation::body::BodyPartCategory,
    rng: &mut R,
) -> Option<BodyPartId> {
    let matching: Vec<_> = body
        .targetable_parts()
        .into_iter()
        .filter(|p| p.category == category)
        .collect();

    if matching.is_empty() {
        // Fall back to random part
        return select_target_part(body, rng);
    }

    // Random among matching
    matching.get(rng.gen_range(0..matching.len())).map(|p| p.id)
}

/// Check if character should be considered dead based on body state and effects
pub fn check_death(body: &Body, effects: &[CombatEffect]) -> bool {
    // Check body death conditions
    if body.is_dead() {
        return true;
    }

    // Check for death effects
    effects.iter().any(|e| matches!(e, CombatEffect::Dead { .. }))
}
