//! Colonist lifecycle management
//!
//! Handles birth, aging, death, and notable colonist promotion/demotion.

use rand::Rng;
use std::collections::HashMap;

use crate::simulation::colonists::types::{
    Colonist, ColonistId, ColonistRole, Gender, LifeStage, NameGenerator,
};
use crate::simulation::colonists::pool::PopulationPool;
use crate::simulation::colonists::mood::MoodModifierType;
use crate::simulation::types::TileCoord;

/// Manager for notable colonists
#[derive(Clone, Debug, Default)]
pub struct NotableColonists {
    /// All notable colonists
    pub colonists: HashMap<ColonistId, Colonist>,
    /// Spatial index: tile -> colonist at that tile
    pub colonist_map: HashMap<TileCoord, ColonistId>,
    /// Next colonist ID
    pub next_id: u64,
    /// Name generator
    name_gen: NameGenerator,
}

impl NotableColonists {
    pub fn new() -> Self {
        NotableColonists {
            colonists: HashMap::new(),
            colonist_map: HashMap::new(),
            next_id: 0,
            name_gen: NameGenerator::default(),
        }
    }

    /// Get the next available ID
    fn next_colonist_id(&mut self) -> ColonistId {
        let id = ColonistId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Create a new notable colonist
    pub fn create_colonist<R: Rng>(
        &mut self,
        age: u32,
        gender: Gender,
        current_tick: u64,
        location: TileCoord,
        rng: &mut R,
    ) -> ColonistId {
        let id = self.next_colonist_id();
        let name = self.name_gen.generate(gender, rng);
        let colonist = Colonist::new(id, name, age, gender, current_tick, location, rng);
        self.colonist_map.insert(location, id);
        self.colonists.insert(id, colonist);
        id
    }

    /// Create a child colonist from two parents
    pub fn create_child<R: Rng>(
        &mut self,
        parent_a_id: ColonistId,
        parent_b_id: ColonistId,
        current_tick: u64,
        rng: &mut R,
    ) -> Option<ColonistId> {
        let parent_a = self.colonists.get(&parent_a_id)?.clone();
        let parent_b = self.colonists.get(&parent_b_id)?.clone();

        // Get parent's surname
        let surname = parent_a.name.split_whitespace().last().unwrap_or("Unknown");
        // Child spawns at mother's location
        let location = parent_a.location;

        let id = self.next_colonist_id();
        let gender = Gender::random(rng);
        let name = self.name_gen.generate_child_name(gender, surname, rng);

        let child = Colonist::create_child(id, name, &parent_a, &parent_b, current_tick, location, rng);

        // Update parent records
        if let Some(p) = self.colonists.get_mut(&parent_a_id) {
            p.children.push(id);
            p.mood.add_modifier(MoodModifierType::ChildBorn);
        }
        if let Some(p) = self.colonists.get_mut(&parent_b_id) {
            p.children.push(id);
            p.mood.add_modifier(MoodModifierType::ChildBorn);
        }

        self.colonist_map.insert(location, id);
        self.colonists.insert(id, child);
        Some(id)
    }

    /// Get a colonist by ID
    pub fn get(&self, id: ColonistId) -> Option<&Colonist> {
        self.colonists.get(&id)
    }

    /// Get a mutable colonist by ID
    pub fn get_mut(&mut self, id: ColonistId) -> Option<&mut Colonist> {
        self.colonists.get_mut(&id)
    }

    /// Get all living colonists
    pub fn living(&self) -> impl Iterator<Item = &Colonist> {
        self.colonists.values().filter(|c| c.is_alive)
    }

    /// Get all colonists with a specific role
    pub fn with_role(&self, role: ColonistRole) -> impl Iterator<Item = &Colonist> {
        self.colonists.values().filter(move |c| c.is_alive && c.role == role)
    }

    /// Get the current leader (if any)
    pub fn leader(&self) -> Option<&Colonist> {
        self.colonists.values().find(|c| c.is_alive && c.role == ColonistRole::Leader)
    }

    /// Get count of notable colonists
    pub fn count(&self) -> usize {
        self.colonists.values().filter(|c| c.is_alive).count()
    }

    /// Get workers (adult colonists who can work)
    pub fn workers(&self) -> impl Iterator<Item = &Colonist> {
        self.colonists.values().filter(|c| c.is_alive && c.can_work())
    }

    /// Get worker count
    pub fn worker_count(&self) -> usize {
        self.workers().count()
    }

    /// Get all colonist names for succession purposes
    pub fn notable_names(&self) -> Vec<String> {
        self.living().map(|c| c.name.clone()).collect()
    }

    /// Remove dead colonists from active tracking
    pub fn cleanup_dead(&mut self) {
        // We keep dead colonists for historical records but mark them
        // This could be changed to actually remove them if memory is a concern
    }

    /// Kill a colonist
    pub fn kill_colonist(&mut self, id: ColonistId, cause: &str) {
        if let Some(colonist) = self.colonists.get_mut(&id) {
            colonist.is_alive = false;
            colonist.health = 0.0;
            colonist.add_event(format!("Died: {}", cause));

            // Notify family members
            let spouse_id = colonist.spouse;
            let children_ids: Vec<_> = colonist.children.clone();
            let parent_ids = colonist.parents;

            // Apply mourning to spouse
            if let Some(spouse_id) = spouse_id {
                if let Some(spouse) = self.colonists.get_mut(&spouse_id) {
                    spouse.mood.add_modifier(MoodModifierType::DeathOfFamily);
                    spouse.spouse = None;
                }
            }

            // Apply mourning to children
            for child_id in children_ids {
                if let Some(child) = self.colonists.get_mut(&child_id) {
                    child.mood.add_modifier(MoodModifierType::DeathOfFamily);
                }
            }

            // Apply mourning to parents
            if let Some(parent_id) = parent_ids.0 {
                if let Some(parent) = self.colonists.get_mut(&parent_id) {
                    parent.mood.add_modifier(MoodModifierType::DeathOfFamily);
                }
            }
            if let Some(parent_id) = parent_ids.1 {
                if let Some(parent) = self.colonists.get_mut(&parent_id) {
                    parent.mood.add_modifier(MoodModifierType::DeathOfFamily);
                }
            }
        }
    }

    /// Update the spatial index from colonist positions
    pub fn update_colonist_map(&mut self) {
        self.colonist_map.clear();
        for (id, colonist) in &self.colonists {
            if colonist.is_alive {
                self.colonist_map.insert(colonist.location, *id);
            }
        }
    }

    /// Get a colonist at a specific tile coordinate
    pub fn get_at(&self, coord: &TileCoord) -> Option<&Colonist> {
        self.colonist_map
            .get(coord)
            .and_then(|id| self.colonists.get(id))
    }

    /// Get a mutable colonist at a specific tile coordinate
    pub fn get_at_mut(&mut self, coord: &TileCoord) -> Option<&mut Colonist> {
        let id = self.colonist_map.get(coord).copied();
        id.and_then(move |id| self.colonists.get_mut(&id))
    }
}

/// Process lifecycle for notable colonists (aging, death checks)
pub fn process_notable_lifecycle<R: Rng>(
    notables: &mut NotableColonists,
    current_tick: u64,
    health_satisfaction: f32,
    rng: &mut R,
) -> LifecycleResult {
    let mut result = LifecycleResult::default();

    // Age colonists yearly (every 4 ticks)
    if current_tick % 4 == 0 {
        let ids: Vec<_> = notables.colonists.keys().copied().collect();

        for id in ids {
            if let Some(colonist) = notables.colonists.get_mut(&id) {
                if !colonist.is_alive {
                    continue;
                }

                colonist.age_one_year();
                result.aged += 1;

                // Check for death
                let death_chance = calculate_death_chance(colonist, health_satisfaction);
                if rng.gen::<f32>() < death_chance {
                    let cause = determine_death_cause(colonist, rng);
                    result.deaths.push((id, cause.clone()));
                }

                // Track life stage transitions
                if colonist.life_stage == LifeStage::Adult && colonist.age == 16 {
                    result.became_adults.push(id);
                    colonist.add_event("Came of age".to_string());
                }
                if colonist.life_stage == LifeStage::Elder && colonist.age == 65 {
                    result.became_elders.push(id);
                    colonist.add_event("Became an elder".to_string());
                }
            }
        }
    }

    // Process recorded deaths
    for (id, cause) in &result.deaths {
        notables.kill_colonist(*id, cause);
    }

    // Update moods
    for colonist in notables.colonists.values_mut() {
        if colonist.is_alive {
            colonist.mood.tick();
        }
    }

    result
}

/// Calculate death chance based on age and conditions
/// Note: Rates reduced significantly for more stable population and longer-lived colonists
fn calculate_death_chance(colonist: &Colonist, health_satisfaction: f32) -> f32 {
    let age = colonist.age;
    let health = colonist.health;

    // Base death rate by age (reduced by ~10x for longer-lived colonists)
    let base_rate = if age < 5 {
        0.002  // Child mortality (reduced)
    } else if age < 16 {
        0.0002  // Very low for children
    } else if age < 40 {
        0.0003  // Prime of life - very low
    } else if age < 60 {
        0.001   // Middle age
    } else if age < 70 {
        0.003   // Getting older
    } else if age < 80 {
        0.008   // Elderly
    } else if age < 90 {
        0.02    // Very old
    } else {
        0.05    // Ancient
    };

    // Modify by health and conditions
    let health_mod = if health < 0.3 {
        2.0  // Badly wounded (reduced impact)
    } else if health < 0.7 {
        1.3  // Wounded
    } else {
        1.0
    };

    let condition_mod = 1.0 + (1.0 - health_satisfaction) * 1.0; // Reduced impact of conditions

    base_rate * health_mod * condition_mod
}

/// Determine cause of death
fn determine_death_cause<R: Rng>(colonist: &Colonist, rng: &mut R) -> String {
    if colonist.health < 0.3 {
        return "wounds".to_string();
    }

    if colonist.age >= 70 {
        let causes = ["old age", "heart failure", "natural causes", "peaceful death"];
        return causes[rng.gen_range(0..causes.len())].to_string();
    }

    if colonist.age >= 50 {
        let causes = ["illness", "disease", "infection", "natural causes"];
        return causes[rng.gen_range(0..causes.len())].to_string();
    }

    let causes = ["illness", "accident", "disease", "infection"];
    causes[rng.gen_range(0..causes.len())].to_string()
}

/// Process potential births among notable colonists
pub fn process_notable_births<R: Rng>(
    notables: &mut NotableColonists,
    current_tick: u64,
    food_satisfaction: f32,
    birth_rate: f32,
    rng: &mut R,
) -> Vec<ColonistId> {
    let mut new_children = Vec::new();

    // Find potential parent pairs (married couples with female of childbearing age)
    let potential_mothers: Vec<_> = notables.colonists.values()
        .filter(|c| {
            c.is_alive &&
            c.gender == Gender::Female &&
            c.life_stage == LifeStage::Adult &&
            c.age >= 16 && c.age <= 45 &&
            c.spouse.is_some()
        })
        .map(|c| (c.id, c.spouse.unwrap()))
        .collect();

    for (mother_id, father_id) in potential_mothers {
        // Check if father is alive
        let father_alive = notables.colonists.get(&father_id)
            .map(|f| f.is_alive)
            .unwrap_or(false);

        if !father_alive {
            continue;
        }

        // Birth chance based on conditions
        let birth_chance = birth_rate * food_satisfaction * 0.25; // Lower for individuals

        if rng.gen::<f32>() < birth_chance {
            if let Some(child_id) = notables.create_child(mother_id, father_id, current_tick, rng) {
                new_children.push(child_id);
            }
        }
    }

    new_children
}

/// Promote a colonist from the pool to notable status
pub fn promote_to_notable<R: Rng>(
    notables: &mut NotableColonists,
    pool: &mut PopulationPool,
    role: ColonistRole,
    current_tick: u64,
    capital: TileCoord,
    rng: &mut R,
) -> Option<ColonistId> {
    // Can only promote if pool has adults
    if pool.adults.count == 0 {
        return None;
    }

    // Remove one from pool
    pool.adults.remove(1);

    // Create a new notable at the capital
    let gender = Gender::random(rng);
    let age = 20 + rng.gen_range(0..30); // Adult age

    let id = notables.create_colonist(age, gender, current_tick, capital, rng);

    // Set role
    if let Some(colonist) = notables.get_mut(id) {
        colonist.role = role;
        colonist.add_event(format!("Became a notable as {:?}", role));
    }

    Some(id)
}

/// Calculate target number of notables based on population
pub fn target_notable_count(total_population: u32) -> usize {
    // ~5% of population, with minimum of 3 and max of 50
    ((total_population as f32 * 0.05) as usize).clamp(3, 50)
}

/// Result of lifecycle processing
#[derive(Clone, Debug, Default)]
pub struct LifecycleResult {
    pub aged: u32,
    pub deaths: Vec<(ColonistId, String)>,
    pub became_adults: Vec<ColonistId>,
    pub became_elders: Vec<ColonistId>,
}

impl LifecycleResult {
    pub fn death_count(&self) -> usize {
        self.deaths.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notable_colonist_creation() {
        let mut notables = NotableColonists::new();
        let mut rng = rand::thread_rng();
        let location = TileCoord::new(10, 10);

        let id = notables.create_colonist(25, Gender::Male, 0, location, &mut rng);

        let colonist = notables.get(id).unwrap();
        assert_eq!(colonist.age, 25);
        assert!(colonist.is_alive);
        assert_eq!(colonist.location, location);
    }

    #[test]
    fn test_target_notable_count() {
        assert_eq!(target_notable_count(50), 3);   // Minimum
        assert_eq!(target_notable_count(100), 5);
        assert_eq!(target_notable_count(1000), 50); // Maximum
    }

    #[test]
    fn test_death_chance() {
        let mut rng = rand::thread_rng();
        let location = TileCoord::new(10, 10);

        // Young healthy colonist
        let young = Colonist::new(ColonistId(1), "Test".to_string(), 25, Gender::Male, 0, location, &mut rng);
        let young_chance = calculate_death_chance(&young, 1.0);

        // Old colonist
        let old = Colonist::new(ColonistId(2), "Test".to_string(), 80, Gender::Male, 0, location, &mut rng);
        let old_chance = calculate_death_chance(&old, 1.0);

        assert!(old_chance > young_chance);
    }
}
