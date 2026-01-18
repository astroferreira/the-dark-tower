//! Combat resolution system
//!
//! Handles attack rolls, hit determination, and combat round resolution.

use rand::Rng;

use crate::simulation::body::CombatEffect;
use crate::simulation::characters::{
    armor_for_character, weapon_for_character, Armor, Character, CharacterOrigin,
    Weapon,
};
use crate::simulation::technology::Age;

use super::damage::{apply_damage_to_part, check_death, select_target_part};
use super::log::{CombatAction, CombatLogEntry, CombatResult, CombatantRef};

/// Stamina cost for attacking
const ATTACK_STAMINA_COST: f32 = 15.0;

/// Base hit chance
const BASE_HIT_CHANCE: f32 = 0.5;

/// Get the tech age from a character's origin if it's a tribe member
fn get_character_age(character: &Character) -> Option<Age> {
    match &character.origin {
        CharacterOrigin::TribeMember { .. } => Some(Age::Iron), // Default to Iron if we don't have tribe info
        CharacterOrigin::MonsterIndividual { .. } => None,
    }
}

/// Resolve a single attack from attacker to defender
pub fn resolve_attack<R: Rng>(
    attacker: &mut Character,
    defender: &mut Character,
    tick: u64,
    rng: &mut R,
) -> CombatLogEntry {
    let attacker_ref = CombatantRef {
        id: attacker.id.0,
        name: attacker.name.clone(),
        faction: attacker.faction(),
    };

    let defender_ref = CombatantRef {
        id: defender.id.0,
        name: defender.name.clone(),
        faction: defender.faction(),
    };

    // Check if attacker can attack
    if !attacker.can_attack() {
        return CombatLogEntry {
            tick,
            attacker: attacker_ref,
            defender: defender_ref,
            action: CombatAction::Unable,
            target_part: None,
            damage: None,
            wound_type: None,
            wound_severity: None,
            result: CombatResult::Miss,
            effects: vec![],
            narrative: format!("{} is unable to attack", attacker.name),
        };
    }

    // Consume stamina
    attacker.consume_stamina(ATTACK_STAMINA_COST);

    // Get weapon and armor
    let attacker_age = get_character_age(attacker);
    let defender_age = get_character_age(defender);
    let weapon = weapon_for_character(attacker, attacker_age);
    let armor = armor_for_character(defender, defender_age);

    // Calculate hit chance
    let hit_chance = calculate_hit_chance(attacker, defender, &weapon, &armor);

    // Roll to hit
    let hit_roll: f32 = rng.gen();

    if hit_roll > hit_chance {
        // Miss
        return CombatLogEntry {
            tick,
            attacker: attacker_ref,
            defender: defender_ref,
            action: CombatAction::Attack {
                weapon: weapon.weapon_type.display_name().to_string(),
                damage_type: weapon.weapon_type.damage_type().display_name().to_string(),
            },
            target_part: None,
            damage: None,
            wound_type: None,
            wound_severity: None,
            result: CombatResult::Miss,
            effects: vec![],
            narrative: format!(
                "{} {} {}, missing",
                attacker.name,
                weapon.weapon_type.attack_verb(),
                defender.name
            ),
        };
    }

    // Select target body part
    let target_part_id = match select_target_part(&defender.body, rng) {
        Some(id) => id,
        None => {
            return CombatLogEntry {
                tick,
                attacker: attacker_ref,
                defender: defender_ref,
                action: CombatAction::Attack {
                    weapon: weapon.weapon_type.display_name().to_string(),
                    damage_type: weapon.weapon_type.damage_type().display_name().to_string(),
                },
                target_part: None,
                damage: None,
                wound_type: None,
                wound_severity: None,
                result: CombatResult::Miss,
                effects: vec![],
                narrative: format!("{} finds no valid target on {}", attacker.name, defender.name),
            };
        }
    };

    let target_part_name = defender
        .body
        .get_part(target_part_id)
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "body".to_string());

    let target_part_category = defender
        .body
        .get_part(target_part_id)
        .map(|p| p.category);

    // Calculate damage
    let base_damage = weapon.damage_with_strength(attacker.attributes.strength_modifier());
    let damage_type = weapon.weapon_type.damage_type();

    // Apply armor reduction
    let post_armor_damage = if let Some(category) = target_part_category {
        armor.reduce_damage(base_damage, category, weapon.armor_pierce)
    } else {
        base_damage
    };

    // Apply toughness reduction
    let final_raw_damage = post_armor_damage / defender.attributes.toughness_modifier();

    // Apply damage to body part
    let damage_result = apply_damage_to_part(
        &mut defender.body,
        target_part_id,
        final_raw_damage,
        damage_type,
        tick,
        rng,
    );

    match damage_result {
        Some(result) => {
            // Add pain to defender
            if let Some(ref wound) = result.wound {
                defender.add_pain(wound.pain);
            }

            // Check for death
            let is_kill = check_death(&defender.body, &result.effects);
            if is_kill {
                defender.is_alive = false;
                defender.is_conscious = false;
            }

            // Determine combat result
            let combat_result = if is_kill {
                let cause = result
                    .effects
                    .iter()
                    .find_map(|e| {
                        if let CombatEffect::Dead { cause } = e {
                            Some(cause.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| "wounds".to_string());
                CombatResult::Kill { cause }
            } else if result.wound.is_some() {
                CombatResult::Wound
            } else {
                CombatResult::Hit
            };

            // Build narrative
            let narrative = build_attack_narrative(
                &attacker.name,
                &defender.name,
                &weapon,
                &target_part_name,
                result.final_damage,
                result.wound.as_ref(),
                &result.effects,
                is_kill,
            );

            CombatLogEntry {
                tick,
                attacker: attacker_ref,
                defender: defender_ref,
                action: CombatAction::Attack {
                    weapon: weapon.weapon_type.display_name().to_string(),
                    damage_type: damage_type.display_name().to_string(),
                },
                target_part: Some(target_part_name),
                damage: Some(result.final_damage),
                wound_type: result.wound.as_ref().map(|w| w.wound_type.display_name().to_string()),
                wound_severity: result.wound.as_ref().map(|w| w.severity.display_name().to_string()),
                result: combat_result,
                effects: result.effects,
                narrative,
            }
        }
        None => CombatLogEntry {
            tick,
            attacker: attacker_ref,
            defender: defender_ref,
            action: CombatAction::Attack {
                weapon: weapon.weapon_type.display_name().to_string(),
                damage_type: damage_type.display_name().to_string(),
            },
            target_part: Some(target_part_name),
            damage: None,
            wound_type: None,
            wound_severity: None,
            result: CombatResult::Miss,
            effects: vec![],
            narrative: format!("{}'s attack fails to connect", attacker.name),
        },
    }
}

/// Calculate hit chance based on attacker/defender stats
fn calculate_hit_chance(
    attacker: &Character,
    defender: &Character,
    weapon: &Weapon,
    armor: &Armor,
) -> f32 {
    let agility_diff = attacker.attributes.agility as f32 - defender.attributes.agility as f32;
    let agility_modifier = agility_diff * 0.003; // Each point of agility diff = 0.3%

    // Weapon accuracy
    let weapon_accuracy = weapon.accuracy;

    // Armor speed penalty affects dodge
    let armor_penalty = armor.speed_penalty;

    // Body impairment affects both
    let attacker_impairment = attacker.body.function_impairment(
        crate::simulation::body::BodyPartFunction::Attacking,
    );
    let defender_impairment = attacker.body.function_impairment(
        crate::simulation::body::BodyPartFunction::Locomotion,
    );

    let hit_chance = BASE_HIT_CHANCE
        + agility_modifier
        + (weapon_accuracy - 0.5) * 0.5 // Weapon accuracy contribution
        + armor_penalty * 0.3 // Armor makes dodging harder
        - attacker_impairment * 0.3 // Injuries reduce accuracy
        + defender_impairment * 0.2; // Injured defenders are easier to hit

    hit_chance.clamp(0.1, 0.95)
}

/// Build a narrative string for an attack
fn build_attack_narrative(
    attacker_name: &str,
    defender_name: &str,
    weapon: &Weapon,
    target_part: &str,
    damage: f32,
    wound: Option<&crate::simulation::body::Wound>,
    effects: &[CombatEffect],
    is_kill: bool,
) -> String {
    let mut narrative = format!(
        "{} {} {}'s {}",
        attacker_name,
        weapon.weapon_type.attack_verb(),
        defender_name,
        target_part
    );

    if let Some(w) = wound {
        narrative.push_str(&format!(
            " with {} {}, {} for {:.1} damage",
            if weapon.weapon_type.is_natural() {
                "its"
            } else {
                "a"
            },
            weapon.weapon_type.display_name(),
            w.wound_type.infliction_participle(),
            damage
        ));
    } else {
        narrative.push_str(&format!(
            " with {} {} for {:.1} damage",
            if weapon.weapon_type.is_natural() {
                "its"
            } else {
                "a"
            },
            weapon.weapon_type.display_name(),
            damage
        ));
    }

    // Add effects
    for effect in effects {
        match effect {
            CombatEffect::Staggered => narrative.push_str(". The blow staggers them"),
            CombatEffect::Knockdown => narrative.push_str(". They are knocked to the ground"),
            CombatEffect::Stunned => narrative.push_str(". They are stunned"),
            CombatEffect::LimbSevered { part_name } => {
                narrative.push_str(&format!(", severing the {}!", part_name))
            }
            CombatEffect::Blinded => narrative.push_str(". They are blinded"),
            CombatEffect::Unconscious => narrative.push_str(". They fall unconscious"),
            CombatEffect::Dead { cause } => {
                narrative.push_str(&format!(", killing them ({})!", cause))
            }
            _ => {}
        }
    }

    if is_kill && !effects.iter().any(|e| matches!(e, CombatEffect::Dead { .. })) {
        narrative.push_str(", killing them!");
    } else if !narrative.ends_with('!') && !narrative.ends_with('.') {
        narrative.push('.');
    }

    narrative
}

