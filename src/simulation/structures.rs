//! Overworld structures - visible buildings on the world map at settlement locations

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::simulation::types::{TileCoord, TribeId};

/// Unique identifier for a structure
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StructureId(pub u32);

/// A physical structure on the world map
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Structure {
    pub id: StructureId,
    pub structure_type: StructureType,
    pub location: TileCoord,
    pub owner: Option<TribeId>,
    pub health: f32,
    pub built_tick: u64,
}

impl Structure {
    pub fn new(
        id: StructureId,
        structure_type: StructureType,
        location: TileCoord,
        owner: Option<TribeId>,
        built_tick: u64,
    ) -> Self {
        Structure {
            id,
            structure_type,
            location,
            owner,
            health: structure_type.max_health(),
            built_tick,
        }
    }

    /// Check if structure is destroyed
    pub fn is_destroyed(&self) -> bool {
        self.health <= 0.0
    }

    /// Take damage, return true if destroyed
    pub fn take_damage(&mut self, damage: f32) -> bool {
        self.health = (self.health - damage).max(0.0);
        self.is_destroyed()
    }
}

/// Types of structures that can appear on the world map
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StructureType {
    TownCenter,    // '@' - Settlement marker (capital)
    Hut,           // 'h' - Basic dwelling
    WoodenHouse,   // 'H' - Improved dwelling
    Shrine,        // 's' - Religious site
    Temple,        // 'T' - Major religious site
    Forge,         // 'f' - Metalworking
    Wall,          // '#' - Defensive wall
    Watchtower,    // '!' - Early warning
    Castle,        // 'C' - Major fortification
    Cathedral,     // '&' - Grand religious building
    Ruins,         // '%' - Abandoned structure
    Monument,      // '*' - Cultural landmark
}

impl StructureType {
    /// Get the ASCII character for this structure type
    pub fn map_char(&self) -> char {
        match self {
            StructureType::TownCenter => '@',
            StructureType::Hut => 'h',
            StructureType::WoodenHouse => 'H',
            StructureType::Shrine => 's',
            StructureType::Temple => 'T',
            StructureType::Forge => 'f',
            StructureType::Wall => '#',
            StructureType::Watchtower => '!',
            StructureType::Castle => 'C',
            StructureType::Cathedral => '&',
            StructureType::Ruins => '%',
            StructureType::Monument => '*',
        }
    }

    /// Get the color for this structure type (RGB)
    pub fn color(&self) -> (u8, u8, u8) {
        match self {
            StructureType::TownCenter => (255, 215, 0),    // Gold
            StructureType::Hut => (139, 90, 43),          // Brown
            StructureType::WoodenHouse => (160, 82, 45),  // Sienna
            StructureType::Shrine => (200, 200, 255),     // Light blue
            StructureType::Temple => (255, 255, 200),     // Light yellow
            StructureType::Forge => (255, 100, 0),        // Orange-red
            StructureType::Wall => (128, 128, 128),       // Gray
            StructureType::Watchtower => (192, 192, 192), // Silver
            StructureType::Castle => (220, 220, 220),     // Light gray
            StructureType::Cathedral => (255, 223, 186),  // Peach
            StructureType::Ruins => (100, 100, 80),       // Dark gray-brown
            StructureType::Monument => (255, 255, 255),   // White
        }
    }

    /// Maximum health for this structure type
    pub fn max_health(&self) -> f32 {
        match self {
            StructureType::TownCenter => 200.0,
            StructureType::Hut => 30.0,
            StructureType::WoodenHouse => 50.0,
            StructureType::Shrine => 40.0,
            StructureType::Temple => 80.0,
            StructureType::Forge => 60.0,
            StructureType::Wall => 100.0,
            StructureType::Watchtower => 50.0,
            StructureType::Castle => 300.0,
            StructureType::Cathedral => 150.0,
            StructureType::Ruins => 10.0,
            StructureType::Monument => 100.0,
        }
    }

    /// Convert from building type name to structure type (if applicable)
    pub fn from_building_name(name: &str) -> Option<StructureType> {
        match name {
            "Hut" => Some(StructureType::Hut),
            "Wooden House" | "WoodenHouse" => Some(StructureType::WoodenHouse),
            "Shrine" => Some(StructureType::Shrine),
            "Temple" => Some(StructureType::Temple),
            "Forge" => Some(StructureType::Forge),
            "Wall" => Some(StructureType::Wall),
            "Watchtower" => Some(StructureType::Watchtower),
            "Castle" => Some(StructureType::Castle),
            "Cathedral" => Some(StructureType::Cathedral),
            _ => None,
        }
    }
}

/// Manager for all structures in the simulation
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StructureManager {
    pub structures: HashMap<StructureId, Structure>,
    pub structure_map: HashMap<TileCoord, StructureId>,
    pub next_structure_id: u32,
}

impl StructureManager {
    pub fn new() -> Self {
        StructureManager {
            structures: HashMap::new(),
            structure_map: HashMap::new(),
            next_structure_id: 0,
        }
    }

    /// Create a new structure at the given location
    pub fn create_structure(
        &mut self,
        structure_type: StructureType,
        location: TileCoord,
        owner: Option<TribeId>,
        current_tick: u64,
    ) -> StructureId {
        let id = StructureId(self.next_structure_id);
        self.next_structure_id += 1;

        let structure = Structure::new(id, structure_type, location, owner, current_tick);
        self.structures.insert(id, structure);
        self.structure_map.insert(location, id);

        id
    }

    /// Get structure at a location
    pub fn get_at(&self, coord: &TileCoord) -> Option<&Structure> {
        self.structure_map
            .get(coord)
            .and_then(|id| self.structures.get(id))
    }

    /// Get mutable structure at a location
    pub fn get_at_mut(&mut self, coord: &TileCoord) -> Option<&mut Structure> {
        if let Some(&id) = self.structure_map.get(coord) {
            self.structures.get_mut(&id)
        } else {
            None
        }
    }

    /// Remove a structure
    pub fn remove(&mut self, id: StructureId) {
        if let Some(structure) = self.structures.remove(&id) {
            self.structure_map.remove(&structure.location);
        }
    }

    /// Convert a structure to ruins
    pub fn convert_to_ruins(&mut self, id: StructureId, current_tick: u64) {
        if let Some(structure) = self.structures.get_mut(&id) {
            structure.structure_type = StructureType::Ruins;
            structure.owner = None;
            structure.health = StructureType::Ruins.max_health();
            structure.built_tick = current_tick;
        }
    }

    /// Get all structures owned by a tribe
    pub fn structures_owned_by(&self, tribe_id: TribeId) -> Vec<&Structure> {
        self.structures
            .values()
            .filter(|s| s.owner == Some(tribe_id))
            .collect()
    }

    /// Check if there's a structure at a location
    pub fn has_structure_at(&self, coord: &TileCoord) -> bool {
        self.structure_map.contains_key(coord)
    }
}
