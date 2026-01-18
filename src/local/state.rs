//! Local map state tracking for dynamic modifications
//!
//! Tracks changes to local maps during simulation - removed trees, built structures,
//! depleted resources, etc.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use crate::simulation::types::TileCoord;
use crate::simulation::TribeId;
use super::terrain::LocalFeature;

/// Modification to a feature at a local tile
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FeatureModification {
    /// Feature was removed (tree chopped, boulder cleared)
    Removed {
        original: LocalFeature,
        tick_removed: u64,
    },
    /// Feature was added (structure built, plant grown)
    Added(LocalFeature),
    /// Feature was replaced with another
    Replaced {
        original: LocalFeature,
        replacement: LocalFeature,
    },
}

/// A structure placed on a local tile
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LocalStructure {
    /// The feature type representing this structure
    pub feature: LocalFeature,
    /// Owner tribe (if any)
    pub owner: Option<TribeId>,
    /// Health (0.0 = destroyed, 1.0 = full)
    pub health: f32,
    /// Construction progress (0.0 = not started, 1.0 = complete)
    pub construction_progress: f32,
    /// Tick when construction started
    pub started_tick: u64,
    /// Tick when completed (if complete)
    pub completed_tick: Option<u64>,
}

impl LocalStructure {
    pub fn new(feature: LocalFeature, owner: Option<TribeId>, tick: u64) -> Self {
        LocalStructure {
            feature,
            owner,
            health: 1.0,
            construction_progress: 0.0,
            started_tick: tick,
            completed_tick: None,
        }
    }

    pub fn is_complete(&self) -> bool {
        self.construction_progress >= 1.0
    }

    pub fn is_destroyed(&self) -> bool {
        self.health <= 0.0
    }
}

/// Work site where colonists gather resources
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LocalWorkSite {
    /// Position within local map
    pub x: usize,
    pub y: usize,
    /// Type of work being done
    pub work_type: WorkSiteType,
    /// Current depletion (0.0 = full, 1.0 = depleted)
    pub depletion: f32,
    /// How many times this site has been worked
    pub times_worked: u32,
    /// Last tick this was worked
    pub last_worked_tick: u64,
}

/// Type of work site
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkSiteType {
    /// Tree being chopped
    Logging,
    /// Stone/ore being mined
    Mining,
    /// Field being farmed
    Farming,
    /// Fishing spot
    Fishing,
    /// Hunting ground
    Hunting,
    /// Gathering berries/herbs
    Gathering,
}

impl WorkSiteType {
    /// Get depletion rate per work action
    pub fn depletion_rate(&self) -> f32 {
        match self {
            WorkSiteType::Logging => 0.5,    // Trees take 2 work actions to fell
            WorkSiteType::Mining => 0.1,     // Mines deplete slowly
            WorkSiteType::Farming => 0.0,    // Farms don't deplete (renewable)
            WorkSiteType::Fishing => 0.05,   // Fishing spots deplete very slowly
            WorkSiteType::Hunting => 0.2,    // Hunting grounds deplete moderately
            WorkSiteType::Gathering => 0.25, // Gathering depletes moderately
        }
    }

    /// Get regeneration rate per tick
    pub fn regen_rate(&self) -> f32 {
        match self {
            WorkSiteType::Logging => 0.001,  // Trees regrow very slowly
            WorkSiteType::Mining => 0.0,     // Mines don't regenerate
            WorkSiteType::Farming => 0.0,    // N/A
            WorkSiteType::Fishing => 0.01,   // Fish repopulate
            WorkSiteType::Hunting => 0.005,  // Animals return slowly
            WorkSiteType::Gathering => 0.02, // Plants regrow
        }
    }
}

/// State of a local map including all modifications
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LocalMapState {
    /// World tile this state belongs to
    pub world_tile: TileCoord,

    /// Feature modifications by local (x, y) coordinate
    pub feature_mods: HashMap<(usize, usize), FeatureModification>,

    /// Structures placed on local tiles
    pub structures: HashMap<(usize, usize), LocalStructure>,

    /// Active work sites
    pub work_sites: HashMap<(usize, usize), LocalWorkSite>,

    /// Resource depletion per tile (0.0 = full, 1.0 = depleted)
    pub depletion: HashMap<(usize, usize), f32>,

    /// Tick when this state was last modified
    pub last_modified_tick: u64,

    /// Whether this state needs to be synced to the cache
    pub needs_render_update: bool,
}

impl LocalMapState {
    pub fn new(world_tile: TileCoord) -> Self {
        LocalMapState {
            world_tile,
            feature_mods: HashMap::new(),
            structures: HashMap::new(),
            work_sites: HashMap::new(),
            depletion: HashMap::new(),
            last_modified_tick: 0,
            needs_render_update: false,
        }
    }

    /// Remove a feature at the given coordinates
    pub fn remove_feature(&mut self, x: usize, y: usize, original: LocalFeature, tick: u64) {
        self.feature_mods.insert((x, y), FeatureModification::Removed {
            original,
            tick_removed: tick,
        });
        self.last_modified_tick = tick;
        self.needs_render_update = true;
    }

    /// Add a feature at the given coordinates
    pub fn add_feature(&mut self, x: usize, y: usize, feature: LocalFeature, tick: u64) {
        self.feature_mods.insert((x, y), FeatureModification::Added(feature));
        self.last_modified_tick = tick;
        self.needs_render_update = true;
    }

    /// Place a structure at the given coordinates
    pub fn place_structure(&mut self, x: usize, y: usize, structure: LocalStructure, tick: u64) {
        self.structures.insert((x, y), structure);
        self.last_modified_tick = tick;
        self.needs_render_update = true;
    }

    /// Work a site and return whether it's now depleted
    pub fn work_site(&mut self, x: usize, y: usize, work_type: WorkSiteType, tick: u64) -> bool {
        let site = self.work_sites.entry((x, y)).or_insert_with(|| LocalWorkSite {
            x,
            y,
            work_type,
            depletion: 0.0,
            times_worked: 0,
            last_worked_tick: tick,
        });

        site.times_worked += 1;
        site.last_worked_tick = tick;
        site.depletion = (site.depletion + work_type.depletion_rate()).min(1.0);

        // Update overall depletion
        self.depletion.insert((x, y), site.depletion);
        self.last_modified_tick = tick;
        self.needs_render_update = true;

        site.depletion >= 1.0
    }

    /// Check if a tile is depleted
    pub fn is_depleted(&self, x: usize, y: usize) -> bool {
        self.depletion.get(&(x, y)).map(|d| *d >= 1.0).unwrap_or(false)
    }

    /// Check if a feature was removed at this location
    pub fn is_feature_removed(&self, x: usize, y: usize) -> bool {
        matches!(self.feature_mods.get(&(x, y)), Some(FeatureModification::Removed { .. }))
    }

    /// Get the effective feature at a location (considering modifications)
    pub fn get_effective_feature(&self, x: usize, y: usize, base_feature: Option<LocalFeature>) -> Option<LocalFeature> {
        // Check for structure first
        if let Some(structure) = self.structures.get(&(x, y)) {
            if structure.is_complete() && !structure.is_destroyed() {
                return Some(structure.feature.clone());
            }
        }

        // Check for modifications
        match self.feature_mods.get(&(x, y)) {
            Some(FeatureModification::Removed { .. }) => None,
            Some(FeatureModification::Added(f)) => Some(f.clone()),
            Some(FeatureModification::Replaced { replacement, .. }) => Some(replacement.clone()),
            None => base_feature,
        }
    }

    /// Process regeneration for work sites
    pub fn tick_regeneration(&mut self, current_tick: u64) {
        for site in self.work_sites.values_mut() {
            let regen = site.work_type.regen_rate();
            if regen > 0.0 && site.depletion > 0.0 {
                // Only regenerate if not recently worked
                if current_tick - site.last_worked_tick > 10 {
                    site.depletion = (site.depletion - regen).max(0.0);
                }
            }
        }

        // Update depletion map
        for ((x, y), site) in &self.work_sites {
            self.depletion.insert((*x, *y), site.depletion);
        }
    }

    /// Get count of removed features
    pub fn removed_count(&self) -> usize {
        self.feature_mods.values()
            .filter(|m| matches!(m, FeatureModification::Removed { .. }))
            .count()
    }

    /// Get count of structures
    pub fn structure_count(&self) -> usize {
        self.structures.values()
            .filter(|s| s.is_complete() && !s.is_destroyed())
            .count()
    }
}

/// Manager for all local map states
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LocalMapStateManager {
    /// States by world tile coordinate
    states: HashMap<TileCoord, LocalMapState>,
}

impl LocalMapStateManager {
    pub fn new() -> Self {
        LocalMapStateManager {
            states: HashMap::new(),
        }
    }

    /// Get or create state for a world tile
    pub fn get_or_create(&mut self, world_tile: TileCoord) -> &mut LocalMapState {
        self.states.entry(world_tile).or_insert_with(|| LocalMapState::new(world_tile))
    }

    /// Get state for a world tile (if exists)
    pub fn get(&self, world_tile: &TileCoord) -> Option<&LocalMapState> {
        self.states.get(world_tile)
    }

    /// Get mutable state for a world tile (if exists)
    pub fn get_mut(&mut self, world_tile: &TileCoord) -> Option<&mut LocalMapState> {
        self.states.get_mut(world_tile)
    }

    /// Check if any modifications exist for a tile
    pub fn has_modifications(&self, world_tile: &TileCoord) -> bool {
        self.states.get(world_tile)
            .map(|s| !s.feature_mods.is_empty() || !s.structures.is_empty())
            .unwrap_or(false)
    }

    /// Process regeneration for all states
    pub fn tick_all(&mut self, current_tick: u64) {
        for state in self.states.values_mut() {
            state.tick_regeneration(current_tick);
        }
    }

    /// Get total modifications across all tiles
    pub fn total_modifications(&self) -> usize {
        self.states.values().map(|s| s.feature_mods.len()).sum()
    }

    /// Get total structures across all tiles
    pub fn total_structures(&self) -> usize {
        self.states.values().map(|s| s.structure_count()).sum()
    }
}
