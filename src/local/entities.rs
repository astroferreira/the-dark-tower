//! Entity tracking for local maps
//!
//! Tracks the positions of fauna, monsters, and colonists within local maps
//! for rendering and interaction purposes.

use serde::{Deserialize, Serialize};

use crate::simulation::colonists::types::{ColonistId, ColonistActivityState};
use crate::simulation::fauna::{FaunaId, FaunaSpecies, FaunaActivity};
use crate::simulation::monsters::{MonsterId, MonsterSpecies, MonsterState};
use crate::simulation::types::TileCoord;

/// Position within a local map (0-63 typically)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LocalPosition {
    pub x: u32,
    pub y: u32,
}

impl LocalPosition {
    pub fn new(x: u32, y: u32) -> Self {
        LocalPosition { x, y }
    }

    /// Create from global local coordinates, extracting the local part
    pub fn from_global(global_x: u32, global_y: u32, local_map_size: u32) -> Self {
        LocalPosition {
            x: global_x % local_map_size,
            y: global_y % local_map_size,
        }
    }
}

/// A visible entity on the local map
#[derive(Clone, Debug)]
pub enum LocalEntity {
    Colonist {
        id: ColonistId,
        name: String,
        activity: ColonistActivityState,
        activity_description: String,
        position: LocalPosition,
    },
    Monster {
        id: MonsterId,
        species: MonsterSpecies,
        state: MonsterState,
        health_percent: f32,
        position: LocalPosition,
    },
    Fauna {
        id: FaunaId,
        species: FaunaSpecies,
        activity: FaunaActivity,
        health_percent: f32,
        position: LocalPosition,
    },
}

impl LocalEntity {
    /// Get the display character for this entity
    pub fn display_char(&self) -> char {
        match self {
            LocalEntity::Colonist { activity, .. } => match activity {
                ColonistActivityState::Idle => '@',
                ColonistActivityState::Working => 'W',
                ColonistActivityState::Traveling => 'T',
                ColonistActivityState::Returning => 'R',
                ColonistActivityState::Socializing => 'S',
                ColonistActivityState::Fleeing => '!',
                ColonistActivityState::Patrolling => 'P',
                ColonistActivityState::Scouting => 'c',
            },
            LocalEntity::Monster { species, .. } => species.map_char(),
            LocalEntity::Fauna { species, .. } => species.map_char(),
        }
    }

    /// Get the display color for this entity (RGB)
    pub fn display_color(&self) -> (u8, u8, u8) {
        match self {
            LocalEntity::Colonist { .. } => (255, 255, 100), // Yellow for colonists
            LocalEntity::Monster { species, .. } => species.color(),
            LocalEntity::Fauna { species, .. } => species.color(),
        }
    }

    /// Get the position
    pub fn position(&self) -> LocalPosition {
        match self {
            LocalEntity::Colonist { position, .. } => *position,
            LocalEntity::Monster { position, .. } => *position,
            LocalEntity::Fauna { position, .. } => *position,
        }
    }

    /// Get a description for display
    pub fn description(&self) -> String {
        match self {
            LocalEntity::Colonist {
                name,
                activity_description,
                ..
            } => format!("{} ({})", name, activity_description),
            LocalEntity::Monster {
                species,
                state,
                health_percent,
                ..
            } => {
                let state_str = match state {
                    MonsterState::Idle => "idle",
                    MonsterState::Roaming => "roaming",
                    MonsterState::Hunting => "hunting",
                    MonsterState::Attacking(_) => "attacking",
                    MonsterState::Fleeing => "fleeing",
                    MonsterState::Dead => "dead",
                };
                format!(
                    "{} ({}, {:.0}% health)",
                    species.name(),
                    state_str,
                    health_percent * 100.0
                )
            }
            LocalEntity::Fauna {
                species,
                activity,
                health_percent,
                ..
            } => {
                format!(
                    "{} ({}, {:.0}% health)",
                    species.name(),
                    activity.description(),
                    health_percent * 100.0
                )
            }
        }
    }

    /// Is this entity a threat?
    pub fn is_threat(&self) -> bool {
        match self {
            LocalEntity::Monster { state, .. } => matches!(
                state,
                MonsterState::Hunting | MonsterState::Attacking(_)
            ),
            LocalEntity::Fauna { species, .. } => {
                // Only predators can be threats
                matches!(
                    species,
                    FaunaSpecies::Fox
                        | FaunaSpecies::Alligator
                        | FaunaSpecies::Eagle
                )
            }
            _ => false,
        }
    }
}

/// Collection of entities visible on a local map
#[derive(Clone, Debug, Default)]
pub struct LocalMapEntities {
    pub entities: Vec<LocalEntity>,
}

impl LocalMapEntities {
    pub fn new() -> Self {
        LocalMapEntities {
            entities: Vec::new(),
        }
    }

    /// Add a colonist entity
    pub fn add_colonist(
        &mut self,
        id: ColonistId,
        name: String,
        activity: ColonistActivityState,
        activity_description: String,
        position: LocalPosition,
    ) {
        self.entities.push(LocalEntity::Colonist {
            id,
            name,
            activity,
            activity_description,
            position,
        });
    }

    /// Add a monster entity
    pub fn add_monster(
        &mut self,
        id: MonsterId,
        species: MonsterSpecies,
        state: MonsterState,
        health_percent: f32,
        position: LocalPosition,
    ) {
        self.entities.push(LocalEntity::Monster {
            id,
            species,
            state,
            health_percent,
            position,
        });
    }

    /// Add a fauna entity
    pub fn add_fauna(
        &mut self,
        id: FaunaId,
        species: FaunaSpecies,
        activity: FaunaActivity,
        health_percent: f32,
        position: LocalPosition,
    ) {
        self.entities.push(LocalEntity::Fauna {
            id,
            species,
            activity,
            health_percent,
            position,
        });
    }

    /// Get entities at a specific position
    pub fn at_position(&self, pos: LocalPosition) -> Vec<&LocalEntity> {
        self.entities
            .iter()
            .filter(|e| e.position() == pos)
            .collect()
    }

    /// Get all entities
    pub fn all(&self) -> &[LocalEntity] {
        &self.entities
    }

    /// Count entities by type
    pub fn count_by_type(&self) -> (usize, usize, usize) {
        let mut colonists = 0;
        let mut monsters = 0;
        let mut fauna = 0;

        for entity in &self.entities {
            match entity {
                LocalEntity::Colonist { .. } => colonists += 1,
                LocalEntity::Monster { .. } => monsters += 1,
                LocalEntity::Fauna { .. } => fauna += 1,
            }
        }

        (colonists, monsters, fauna)
    }

    /// Get threats in the area
    pub fn threats(&self) -> Vec<&LocalEntity> {
        self.entities.iter().filter(|e| e.is_threat()).collect()
    }

    /// Sort entities by position for efficient rendering
    pub fn sort_by_position(&mut self) {
        self.entities.sort_by_key(|e| {
            let pos = e.position();
            (pos.y, pos.x)
        });
    }
}

/// Gather entities for a specific world tile from the simulation state
pub fn gather_local_entities(
    world_coord: TileCoord,
    local_map_size: u32,
    sim_state: &crate::simulation::SimulationState,
) -> LocalMapEntities {
    let mut entities = LocalMapEntities::new();

    // Gather colonists from all tribes in this tile
    for (tribe_id, tribe) in &sim_state.tribes {
        if !tribe.is_alive {
            continue;
        }

        for (col_id, colonist) in &tribe.notable_colonists.colonists {
            if colonist.location == world_coord {
                // Convert global local position to local map position
                let local_pos = LocalPosition::from_global(
                    colonist.local_position.x,
                    colonist.local_position.y,
                    local_map_size,
                );

                let activity_desc = match colonist.activity_state {
                    ColonistActivityState::Idle => "resting",
                    ColonistActivityState::Working => "working",
                    ColonistActivityState::Traveling => "traveling",
                    ColonistActivityState::Returning => "returning home",
                    ColonistActivityState::Socializing => "socializing",
                    ColonistActivityState::Fleeing => "fleeing",
                    ColonistActivityState::Patrolling => "patrolling",
                    ColonistActivityState::Scouting => "scouting",
                };

                entities.add_colonist(
                    *col_id,
                    colonist.name.clone(),
                    colonist.activity_state,
                    activity_desc.to_string(),
                    local_pos,
                );
            }
        }
    }

    // Gather monsters in this tile
    if let Some(monster) = sim_state.monsters.get_at(&world_coord) {
        let local_pos = LocalPosition::from_global(
            monster.local_position.x,
            monster.local_position.y,
            local_map_size,
        );

        entities.add_monster(
            monster.id,
            monster.species,
            monster.state,
            monster.health / monster.max_health,
            local_pos,
        );
    }

    // Gather fauna in this tile
    for fauna in sim_state.fauna.get_at(&world_coord) {
        let local_pos = LocalPosition::from_global(
            fauna.local_position.x,
            fauna.local_position.y,
            local_map_size,
        );

        entities.add_fauna(
            fauna.id,
            fauna.species,
            fauna.current_activity,
            fauna.health / fauna.max_health,
            local_pos,
        );
    }

    entities.sort_by_position();
    entities
}

/// Get a summary description of entities in a local map
pub fn entity_summary(entities: &LocalMapEntities) -> String {
    let (colonists, monsters, fauna) = entities.count_by_type();
    let mut parts = Vec::new();

    if colonists > 0 {
        parts.push(format!("{} colonist{}", colonists, if colonists == 1 { "" } else { "s" }));
    }
    if monsters > 0 {
        parts.push(format!("{} monster{}", monsters, if monsters == 1 { "" } else { "s" }));
    }
    if fauna > 0 {
        parts.push(format!("{} animal{}", fauna, if fauna == 1 { "" } else { "s" }));
    }

    if parts.is_empty() {
        "No visible entities".to_string()
    } else {
        parts.join(", ")
    }
}
