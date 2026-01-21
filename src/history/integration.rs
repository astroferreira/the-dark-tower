//! Integration module for world history generation
//!
//! Ties together all history subsystems and provides the main entry point.

use crate::biomes::ExtendedBiome;
use crate::tilemap::Tilemap;
use crate::water_bodies::WaterBodyId;
use crate::zlevel::{Tilemap3D, ZTile};

use super::factions::{FactionRegistry, generate_factions};
use super::timeline::{Timeline, generate_timeline};
use super::territories::{TerritoryRegistry, generate_territories};
use super::monsters::{MonsterRegistry, generate_monster_lairs};
use super::trade::{TradeRegistry, generate_trade_network};
use super::heroes::{HeroRegistry, generate_heroes_biome};
use super::artifacts::{ArtifactRegistry, ArtifactLocation, generate_artifacts};
use super::dungeons::{DungeonRegistry, generate_dungeons};
use super::evidence::generate_historical_evidence;
use super::types::*;

/// Complete world history data
#[derive(Clone)]
pub struct WorldHistory {
    /// All factions and their relationships
    pub factions: FactionRegistry,
    /// Historical timeline with eras and events
    pub timeline: Timeline,
    /// Territory and settlement data
    pub territories: TerritoryRegistry,
    /// Monster lairs and ecology
    pub monsters: MonsterRegistry,
    /// Trade routes and resources
    pub trade: TradeRegistry,
    /// Notable historical figures
    pub heroes: HeroRegistry,
    /// Artifacts and lore carriers
    pub artifacts: ArtifactRegistry,
    /// Dungeons and significant locations
    pub dungeons: DungeonRegistry,
    /// Seed used for generation
    pub seed: u64,
}

impl WorldHistory {
    /// Create an empty world history (for compatibility)
    pub fn empty() -> Self {
        Self {
            factions: FactionRegistry::new(),
            timeline: Timeline::new(),
            territories: TerritoryRegistry::new(1, 1),
            monsters: MonsterRegistry::new(),
            trade: TradeRegistry::new(),
            heroes: HeroRegistry::new(),
            artifacts: ArtifactRegistry::new(),
            dungeons: DungeonRegistry::new(),
            seed: 0,
        }
    }

    /// Export the complete timeline to a text file
    pub fn export_timeline(&self, filename: &str) -> std::io::Result<()> {
        use std::io::Write;
        let mut file = std::fs::File::create(filename)?;

        writeln!(file, "╔══════════════════════════════════════════════════════════════════════════════╗")?;
        writeln!(file, "║                         CHRONICLE OF THE WORLD                               ║")?;
        writeln!(file, "║                           Seed: {:>10}                                    ║", self.seed)?;
        writeln!(file, "╚══════════════════════════════════════════════════════════════════════════════╝")?;
        writeln!(file)?;

        // Write faction summary
        writeln!(file, "═══════════════════════════════════════════════════════════════════════════════")?;
        writeln!(file, "                              THE FACTIONS")?;
        writeln!(file, "═══════════════════════════════════════════════════════════════════════════════")?;
        writeln!(file)?;

        for faction in self.factions.all() {
            let status = if faction.collapsed.is_some() {
                format!("Collapsed in {}", faction.collapsed.unwrap())
            } else {
                "Active".to_string()
            };
            writeln!(file, "  {} [{}]", faction.name, status)?;
            writeln!(file, "    Species: {:?} | Culture: {:?} | Architecture: {:?}",
                faction.species, faction.culture, faction.architecture)?;
            let capital_str = faction.capital
                .map(|id| format!("Settlement #{}", id.0))
                .unwrap_or_else(|| "None".to_string());
            writeln!(file, "    Founded: Year {} | Capital: {}",
                faction.founded, capital_str)?;
            writeln!(file)?;
        }

        // Write eras and events
        writeln!(file, "═══════════════════════════════════════════════════════════════════════════════")?;
        writeln!(file, "                           TIMELINE OF AGES")?;
        writeln!(file, "═══════════════════════════════════════════════════════════════════════════════")?;
        writeln!(file)?;

        for era in &self.timeline.eras {
            writeln!(file, "┌─────────────────────────────────────────────────────────────────────────────┐")?;
            writeln!(file, "│  {} ({} to {})", era.name, era.start, era.end)?;
            writeln!(file, "│  Type: {:?} | Duration: {} years", era.era_type, era.duration())?;
            writeln!(file, "└─────────────────────────────────────────────────────────────────────────────┘")?;
            writeln!(file)?;

            // Get events for this era, sorted by year
            let mut era_events: Vec<_> = era.events.iter()
                .filter_map(|id| self.timeline.events.get(id))
                .collect();
            era_events.sort_by_key(|e| e.year.0);

            for event in era_events {
                let location_str = event.location
                    .map(|(x, y)| format!(" at ({}, {})", x, y))
                    .unwrap_or_default();

                let faction_str = event.faction
                    .and_then(|id| self.factions.get(id))
                    .map(|f| format!(" [{}]", f.name))
                    .unwrap_or_default();

                writeln!(file, "  Year {:>5}: {}{}{}",
                    event.year, event.name, faction_str, location_str)?;

                if !event.description.is_empty() {
                    writeln!(file, "              {}", event.description)?;
                }

                if event.casualties > 0 {
                    writeln!(file, "              Casualties: {}", event.casualties)?;
                }
                writeln!(file)?;
            }
        }

        // Write settlements summary
        writeln!(file, "═══════════════════════════════════════════════════════════════════════════════")?;
        writeln!(file, "                             SETTLEMENTS")?;
        writeln!(file, "═══════════════════════════════════════════════════════════════════════════════")?;
        writeln!(file)?;

        let mut settlements: Vec<_> = self.territories.settlements.values().collect();
        settlements.sort_by_key(|s| s.founded.0);

        for settlement in settlements {
            let faction_name = self.factions.get(settlement.original_faction)
                .map(|f| f.name.as_str())
                .unwrap_or("Unknown");

            writeln!(file, "  {} ({:?})", settlement.name, settlement.state)?;
            writeln!(file, "    Location: ({}, {}) | Type: {:?}",
                settlement.x, settlement.y, settlement.settlement_type)?;
            writeln!(file, "    Founded: Year {} by {}", settlement.founded, faction_name)?;
            if let Some(reason) = settlement.abandonment_reason {
                writeln!(file, "    Abandoned due to: {:?}", reason)?;
            }
            writeln!(file)?;
        }

        // Write monster lairs
        writeln!(file, "═══════════════════════════════════════════════════════════════════════════════")?;
        writeln!(file, "                            MONSTER LAIRS")?;
        writeln!(file, "═══════════════════════════════════════════════════════════════════════════════")?;
        writeln!(file)?;

        for lair in self.monsters.lairs.values() {
            writeln!(file, "  {} - {:?}", lair.name, lair.species)?;
            writeln!(file, "    Location: ({}, {}, z={}) | Active: {}",
                lair.x, lair.y, lair.z, lair.active)?;
            writeln!(file, "    Territory Size: {} tiles | Danger Level: {}",
                lair.territory.len(), lair.danger)?;
            writeln!(file)?;
        }

        // Write trade routes
        writeln!(file, "═══════════════════════════════════════════════════════════════════════════════")?;
        writeln!(file, "                            TRADE ROUTES")?;
        writeln!(file, "═══════════════════════════════════════════════════════════════════════════════")?;
        writeln!(file)?;

        for route in self.trade.routes.values() {
            writeln!(file, "  Route from ({}, {}) to ({}, {})",
                route.start.0, route.start.1, route.end.0, route.end.1)?;
            writeln!(file, "    Length: {} tiles | Active: {} | Waypoints: {}",
                route.path.len(), route.active, route.waypoints.len())?;
            writeln!(file)?;
        }

        // Write resource sites
        writeln!(file, "═══════════════════════════════════════════════════════════════════════════════")?;
        writeln!(file, "                           RESOURCE SITES")?;
        writeln!(file, "═══════════════════════════════════════════════════════════════════════════════")?;
        writeln!(file)?;

        for site in &self.trade.resources {
            writeln!(file, "  {:?} at ({}, {})", site.resource, site.x, site.y)?;
            let depleted_str = if site.depleted {
                site.depleted_year.map(|y| format!("Year {}", y)).unwrap_or("Yes".to_string())
            } else {
                "No".to_string()
            };
            writeln!(file, "    Discovered: Year {} | Depleted: {}",
                site.discovered, depleted_str)?;
            writeln!(file)?;
        }

        // Write notable heroes
        writeln!(file, "═══════════════════════════════════════════════════════════════════════════════")?;
        writeln!(file, "                           NOTABLE HEROES")?;
        writeln!(file, "═══════════════════════════════════════════════════════════════════════════════")?;
        writeln!(file)?;

        let mut heroes: Vec<_> = self.heroes.all().collect();
        heroes.sort_by_key(|h| std::cmp::Reverse(h.fame));

        for hero in heroes {
            let faction_name = self.factions.get(hero.faction)
                .map(|f| f.name.as_str())
                .unwrap_or("Unknown");

            let life_span = if let Some(death) = hero.death_year {
                format!("{} to {}", hero.birth_year, death)
            } else {
                format!("{} - present", hero.birth_year)
            };

            writeln!(file, "  {} [{} {}]", hero.full_name(), hero.species.name(), hero.role.name())?;
            writeln!(file, "    Faction: {} | Fame: {} | Life: {}", faction_name, hero.fame, life_span)?;

            if !hero.titles.is_empty() {
                writeln!(file, "    Titles: {}", hero.titles.join(", "))?;
            }

            if let Some(ref philosophy) = hero.philosophy {
                writeln!(file, "    Philosophy: \"{}\"", philosophy)?;
            }
            if let Some(ref doctrine) = hero.military_doctrine {
                writeln!(file, "    Doctrine: \"{}\"", doctrine)?;
            }
            if let Some(ref beliefs) = hero.religious_beliefs {
                writeln!(file, "    Faith: \"{}\"", beliefs)?;
            }

            if let Some((x, y, z)) = hero.burial_site {
                writeln!(file, "    Buried at: ({}, {}, z={})", x, y, z)?;
            }
            writeln!(file)?;
        }

        // Write artifacts
        writeln!(file, "═══════════════════════════════════════════════════════════════════════════════")?;
        writeln!(file, "                              ARTIFACTS")?;
        writeln!(file, "═══════════════════════════════════════════════════════════════════════════════")?;
        writeln!(file)?;

        let mut artifacts: Vec<_> = self.artifacts.all().collect();
        artifacts.sort_by_key(|a| std::cmp::Reverse(a.rarity));

        for artifact in artifacts {
            let faction_name = self.factions.get(artifact.faction_origin)
                .map(|f| f.name.as_str())
                .unwrap_or("Unknown");

            writeln!(file, "  {} ({} {})", artifact.name, artifact.rarity.name(), artifact.artifact_type.name())?;
            writeln!(file, "    {}", artifact.description)?;
            writeln!(file, "    Origin: {} | Created: {}", faction_name, artifact.creation_year)?;

            if let Some(creator_id) = artifact.creator {
                if let Some(creator) = self.heroes.get(creator_id) {
                    writeln!(file, "    Creator: {}", creator.full_name())?;
                }
            }

            if let Some(lore_summary) = artifact.contained_lore.summary() {
                writeln!(file, "    Lore: {}", lore_summary)?;
            }

            if !artifact.powers.is_empty() {
                writeln!(file, "    Powers: {}", artifact.powers.join("; "))?;
            }

            // Write history
            writeln!(file, "    History:")?;
            for event in &artifact.history {
                let location_str = event.location
                    .map(|(x, y, z)| format!(" at ({}, {}, z={})", x, y, z))
                    .unwrap_or_default();
                writeln!(file, "      {} - {}{}", event.year, event.description, location_str)?;
            }

            writeln!(file, "    Current Location: {}", artifact.current_location.description())?;
            writeln!(file)?;
        }

        // Write dungeons
        writeln!(file, "═══════════════════════════════════════════════════════════════════════════════")?;
        writeln!(file, "                              DUNGEONS")?;
        writeln!(file, "═══════════════════════════════════════════════════════════════════════════════")?;
        writeln!(file)?;

        for dungeon in self.dungeons.all() {
            writeln!(file, "  {} ({:?})", dungeon.name, dungeon.original_purpose)?;
            writeln!(file, "    Location: ({}, {}) | Depth: {}",
                dungeon.location.0, dungeon.location.1, dungeon.depth_range_str())?;
            writeln!(file, "    Founded: {} | Size: ~{} tiles",
                dungeon.founded_year, dungeon.size)?;

            if let Some(abandoned) = dungeon.abandoned_year {
                writeln!(file, "    Abandoned: {}", abandoned)?;
            }

            if !dungeon.artifacts_present.is_empty() {
                let artifact_names: Vec<String> = dungeon.artifacts_present.iter()
                    .filter_map(|id| self.artifacts.get(*id))
                    .map(|a| a.name.clone())
                    .collect();
                writeln!(file, "    Artifacts: {}", artifact_names.join(", "))?;
            }

            writeln!(file, "    History:")?;
            for entry in &dungeon.history {
                writeln!(file, "      {}", entry)?;
            }
            writeln!(file)?;
        }

        // Write monster hoards
        writeln!(file, "═══════════════════════════════════════════════════════════════════════════════")?;
        writeln!(file, "                           MONSTER HOARDS")?;
        writeln!(file, "═══════════════════════════════════════════════════════════════════════════════")?;
        writeln!(file)?;

        for lair in self.monsters.lairs.values() {
            if lair.hoard.is_empty() {
                continue;
            }

            writeln!(file, "  {} ({:?})", lair.name, lair.species)?;
            writeln!(file, "    Location: ({}, {}, z={}) | Active: {}",
                lair.x, lair.y, lair.z, lair.active)?;
            writeln!(file, "    Hoard ({} items):", lair.hoard.len())?;

            for artifact_id in &lair.hoard {
                if let Some(artifact) = self.artifacts.get(*artifact_id) {
                    writeln!(file, "      - {} ({} {})",
                        artifact.name, artifact.rarity.name(), artifact.artifact_type.name())?;
                }
            }
            writeln!(file)?;
        }

        writeln!(file, "═══════════════════════════════════════════════════════════════════════════════")?;
        writeln!(file, "                          END OF CHRONICLE")?;
        writeln!(file, "═══════════════════════════════════════════════════════════════════════════════")?;

        println!("Timeline exported to {}", filename);
        Ok(())
    }

    /// Get faction controlling a tile
    pub fn faction_at(&self, x: usize, y: usize) -> Option<&super::factions::Faction> {
        self.territories.faction_at(x, y)
            .and_then(|id| self.factions.get(id))
    }

    /// Get settlement at a tile
    pub fn settlement_at(&self, x: usize, y: usize) -> Option<&super::territories::Settlement> {
        self.territories.settlement_at(x, y)
    }

    /// Get monster lair at a tile
    pub fn lair_at(&self, x: usize, y: usize) -> Option<&super::monsters::MonsterLair> {
        self.monsters.lair_at(x, y)
    }

    /// Get historical events at a location
    pub fn events_at(&self, x: usize, y: usize) -> Vec<&super::timeline::HistoricalEvent> {
        self.timeline.events_at(x, y)
    }

    /// Check if a tile is on a trade route
    pub fn is_on_trade_route(&self, x: usize, y: usize) -> bool {
        self.trade.is_on_route(x, y)
    }

    /// Get resource at a tile
    pub fn resource_at(&self, x: usize, y: usize) -> Option<&super::trade::ResourceSite> {
        self.trade.resource_at(x, y)
    }

    /// Get extended tile information for display
    pub fn tile_info(&self, x: usize, y: usize) -> TileHistoryInfo {
        // Get dungeon info
        let dungeon = self.dungeons.dungeon_at(x, y)
            .map(|d| (d.name.clone(), d.original_purpose));

        // Get artifacts at location (checking all z-levels)
        let artifacts: Vec<_> = self.artifacts.all()
            .filter(|a| {
                if let Some((ax, ay, _)) = a.current_location.coordinates() {
                    ax == x && ay == y
                } else {
                    false
                }
            })
            .map(|a| (a.name.clone(), a.rarity.name().to_string()))
            .collect();

        // Get hero buried here
        let hero_buried = self.heroes.all()
            .find(|h| {
                if let Some((hx, hy, _)) = h.burial_site {
                    hx == x && hy == y
                } else {
                    false
                }
            })
            .map(|h| h.full_name());

        TileHistoryInfo {
            faction: self.faction_at(x, y).map(|f| f.name.clone()),
            settlement: self.settlement_at(x, y).map(|s| (s.name.clone(), s.state)),
            lair: self.lair_at(x, y).map(|l| (l.name.clone(), l.species)),
            events: self.events_at(x, y).iter()
                .map(|e| (e.name.clone(), e.year))
                .collect(),
            trade_route: self.is_on_trade_route(x, y),
            resource: self.resource_at(x, y).map(|r| r.resource),
            dungeon,
            artifacts,
            hero_buried,
        }
    }

    /// Get hero at location (if any)
    pub fn hero_at(&self, x: usize, y: usize) -> Option<&super::heroes::Hero> {
        self.heroes.all()
            .find(|h| {
                if let Some((hx, hy, _)) = h.burial_site {
                    hx == x && hy == y
                } else {
                    false
                }
            })
    }

    /// Get artifacts at location
    pub fn artifacts_at(&self, x: usize, y: usize, z: i32) -> Vec<&super::artifacts::Artifact> {
        self.artifacts.artifacts_at(x, y, z)
    }

    /// Get dungeon at location
    pub fn dungeon_at(&self, x: usize, y: usize) -> Option<&super::dungeons::Dungeon> {
        self.dungeons.dungeon_at(x, y)
    }
}

/// Summary of historical information for a tile
#[derive(Clone, Debug)]
pub struct TileHistoryInfo {
    pub faction: Option<String>,
    pub settlement: Option<(String, SettlementState)>,
    pub lair: Option<(String, super::monsters::MonsterSpecies)>,
    pub events: Vec<(String, Year)>,
    pub trade_route: bool,
    pub resource: Option<super::trade::ResourceType>,
    pub dungeon: Option<(String, super::dungeons::DungeonOrigin)>,
    pub artifacts: Vec<(String, String)>, // (name, rarity)
    pub hero_buried: Option<String>,
}

impl TileHistoryInfo {
    /// Check if there's any historical information for this tile
    pub fn has_history(&self) -> bool {
        self.faction.is_some() ||
        self.settlement.is_some() ||
        self.lair.is_some() ||
        !self.events.is_empty() ||
        self.trade_route ||
        self.resource.is_some() ||
        self.dungeon.is_some() ||
        !self.artifacts.is_empty() ||
        self.hero_buried.is_some()
    }

    /// Generate a summary string for display
    pub fn summary(&self) -> Option<String> {
        let mut parts = Vec::new();

        if let Some(ref name) = self.faction {
            parts.push(format!("Territory: {}", name));
        }

        if let Some((ref name, state)) = self.settlement {
            parts.push(format!("{} ({})", name, state.name()));
        }

        if let Some((ref name, species)) = self.lair {
            parts.push(format!("{} - {}", name, species.name()));
        }

        if let Some((ref name, origin)) = self.dungeon {
            parts.push(format!("{} ({:?})", name, origin));
        }

        if !self.artifacts.is_empty() {
            let (ref name, ref rarity) = self.artifacts[0];
            if self.artifacts.len() > 1 {
                parts.push(format!("{} ({}) +{} more", name, rarity, self.artifacts.len() - 1));
            } else {
                parts.push(format!("{} ({})", name, rarity));
            }
        }

        if let Some(ref hero_name) = self.hero_buried {
            parts.push(format!("Tomb of {}", hero_name));
        }

        if !self.events.is_empty() {
            let event = &self.events[0];
            parts.push(format!("{} ({})", event.0, event.1));
        }

        if self.trade_route {
            parts.push("Trade Route".to_string());
        }

        if let Some(resource) = self.resource {
            parts.push(format!("{} deposit", resource.name()));
        }

        if parts.is_empty() {
            None
        } else {
            Some(parts.join(" | "))
        }
    }
}

/// Generate complete world history
///
/// This is the main entry point for the history system.
/// Call this after terrain generation but before structure generation
/// to place historical evidence in the world.
pub fn generate_world_history(
    zlevels: &mut Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
    heightmap: &Tilemap<f32>,
    biomes: &Tilemap<ExtendedBiome>,
    water_bodies: &Tilemap<WaterBodyId>,
    stress_map: &Tilemap<f32>,
    seed: u64,
) -> WorldHistory {
    println!("Generating world history...");

    let width = heightmap.width;
    let height = heightmap.height;

    // Phase 1: Generate factions
    let factions = generate_factions(heightmap, biomes, seed);
    println!("  {} factions created", factions.factions.len());

    // Phase 2: Generate timeline
    let timeline = generate_timeline(&factions, width, height, seed);
    println!("  {} historical events recorded", timeline.events.len());

    // Phase 3: Generate territories and settlements (needed for hero biome assignment)
    let territories = generate_territories(&factions, heightmap, biomes, water_bodies, seed);
    println!("  {} settlements placed", territories.settlements.len());

    // Phase 3.5: Generate heroes with biome-aware features (now that we have territories)
    let heroes = generate_heroes_biome(&factions, &timeline, Some(&territories), Some(biomes), Some(heightmap), seed);
    println!("  {} notable heroes generated", heroes.heroes.len());

    // Phase 4: Generate monster lairs
    let mut monsters = generate_monster_lairs(heightmap, biomes, stress_map, seed);
    println!("  {} monster lairs placed", monsters.lairs.len());

    // Phase 5: Generate trade network
    let trade = generate_trade_network(&territories, heightmap, water_bodies, biomes, seed);
    println!("  {} trade routes established", trade.routes.len());

    // Phase 6: Generate dungeons
    let mut dungeons = generate_dungeons(&territories, heightmap, biomes, seed);
    println!("  {} dungeons generated", dungeons.dungeons.len());

    // Phase 6.5: Generate artifacts with full histories
    let mut artifacts = generate_artifacts(&factions, &heroes, &monsters, seed);
    println!("  {} artifacts created", artifacts.artifacts.len());

    // Phase 6.6: Link artifacts to monster hoards and dungeons
    link_artifacts_to_locations(&mut artifacts, &mut monsters, &mut dungeons, seed);

    // Phase 7: Place physical evidence in the world
    generate_historical_evidence(
        zlevels,
        surface_z,
        &factions,
        &timeline,
        &territories,
        &monsters,
        &trade,
        seed,
    );

    // Phase 7.5: Place artifact-related evidence
    place_artifact_evidence(zlevels, surface_z, &artifacts, &dungeons, seed);

    println!("World history generation complete.");

    WorldHistory {
        factions,
        timeline,
        territories,
        monsters,
        trade,
        heroes,
        artifacts,
        dungeons,
        seed,
    }
}

/// Link artifacts to monster hoards and dungeons based on their current location
fn link_artifacts_to_locations(
    artifacts: &mut ArtifactRegistry,
    monsters: &mut MonsterRegistry,
    dungeons: &mut DungeonRegistry,
    seed: u64,
) {
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;
    use rand::Rng;

    let mut rng = ChaCha8Rng::seed_from_u64(seed.wrapping_add(0xAF71FAC3));

    // Collect artifact IDs and their locations
    let artifact_info: Vec<_> = artifacts.artifacts.iter()
        .map(|(id, a)| (*id, a.current_location.clone()))
        .collect();

    for (artifact_id, location) in artifact_info {
        match location {
            ArtifactLocation::InMonsterLair { lair, .. } => {
                // Add to monster's hoard
                if let Some(monster_lair) = monsters.lairs.get_mut(&lair) {
                    monster_lair.hoard.push(artifact_id);
                    monster_lair.hoard_sources.push((artifact_id, "captured from adventurers".to_string()));
                }
            }
            ArtifactLocation::InDungeon { x, y, .. } => {
                // Try to find a dungeon at this location and add artifact
                if let Some(dungeon_id) = dungeons.dungeons_by_location.get(&(x, y)).copied() {
                    if let Some(dungeon) = dungeons.dungeons.get_mut(&dungeon_id) {
                        dungeon.artifacts_present.push(artifact_id);
                        artifacts.artifacts_by_dungeon.entry(dungeon_id).or_default().push(artifact_id);
                    }
                }
            }
            _ => {}
        }
    }

    // Distribute some artifacts to lairs and dungeons that don't have any
    let lair_ids: Vec<LairId> = monsters.lairs.keys().copied().collect();
    let dungeon_ids: Vec<DungeonId> = dungeons.dungeons.keys().copied().collect();

    // Find artifacts that are hidden or in tombs (potential redistribution candidates)
    let relocatable: Vec<ArtifactId> = artifacts.artifacts.iter()
        .filter(|(_, a)| matches!(a.current_location, ArtifactLocation::Hidden { .. }))
        .map(|(id, _)| *id)
        .collect();

    // Assign some to monster lairs (treasure hoarding monsters)
    let hoard_species = [
        super::monsters::MonsterSpecies::Dragon,
        super::monsters::MonsterSpecies::Troll,
        super::monsters::MonsterSpecies::Ogre,
    ];

    for lair_id in &lair_ids {
        if let Some(lair) = monsters.lairs.get_mut(lair_id) {
            if hoard_species.contains(&lair.species) && lair.hoard.is_empty() {
                // Try to grab an artifact
                if !relocatable.is_empty() && rng.gen_bool(0.5) {
                    let idx = rng.gen_range(0..relocatable.len());
                    let artifact_id = relocatable[idx];

                    if let Some(artifact) = artifacts.artifacts.get_mut(&artifact_id) {
                        artifact.current_location = ArtifactLocation::InMonsterLair {
                            lair: *lair_id,
                            monster_name: lair.name.clone(),
                        };
                        lair.hoard.push(artifact_id);
                        lair.hoard_sources.push((artifact_id, "hoarded by creature".to_string()));
                    }
                }
            }
        }
    }

    // Assign some to dungeons
    for dungeon_id in &dungeon_ids {
        if let Some(dungeon) = dungeons.dungeons.get_mut(dungeon_id) {
            let (min, max) = dungeon.original_purpose.artifact_capacity();
            let target = rng.gen_range(min..=max);

            while dungeon.artifacts_present.len() < target && !relocatable.is_empty() {
                let idx = rng.gen_range(0..relocatable.len().max(1));
                if idx < relocatable.len() {
                    let artifact_id = relocatable[idx];

                    if let Some(artifact) = artifacts.artifacts.get_mut(&artifact_id) {
                        if matches!(artifact.current_location, ArtifactLocation::Hidden { .. }) {
                            artifact.current_location = ArtifactLocation::InDungeon {
                                x: dungeon.location.0,
                                y: dungeon.location.1,
                                z: dungeon.depth_min,
                                dungeon_name: dungeon.name.clone(),
                            };
                            dungeon.artifacts_present.push(artifact_id);
                        }
                    }
                }
                // Exit loop if we've tried enough times
                if dungeon.artifacts_present.len() >= target || rng.gen_bool(0.3) {
                    break;
                }
            }
        }
    }
}

/// Place artifact-related evidence in the world
fn place_artifact_evidence(
    zlevels: &mut Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
    artifacts: &ArtifactRegistry,
    dungeons: &DungeonRegistry,
    seed: u64,
) {
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;
    use rand::Rng;

    let mut rng = ChaCha8Rng::seed_from_u64(seed.wrapping_add(0xAF71FAC4));
    let width = surface_z.width;
    let height = surface_z.height;

    println!("  Placing artifact evidence...");

    // Place dungeon entrances
    for dungeon in dungeons.all() {
        let (x, y) = dungeon.location;
        let surf_z = *surface_z.get(x, y);

        // Place dungeon entrance at surface
        if *zlevels.get(x, y, surf_z) == ZTile::Surface {
            zlevels.set(x, y, surf_z, ZTile::DungeonEntrance);
        }

        // Place treasure hoards inside dungeon
        if !dungeon.artifacts_present.is_empty() {
            let hoard_z = dungeon.depth_min;
            if *zlevels.get(x, y, hoard_z) == ZTile::CaveFloor ||
               *zlevels.get(x, y, hoard_z) == ZTile::Solid {
                zlevels.set(x, y, hoard_z, ZTile::TreasureHoard);
            }
        }
    }

    // Place artifact containers based on artifact type and location
    for artifact in artifacts.all() {
        if let Some((x, y, z)) = artifact.current_location.coordinates() {
            // Skip if out of bounds
            if x >= width || y >= height {
                continue;
            }

            // Determine appropriate container
            let container = match artifact.category {
                super::artifacts::ArtifactCategory::Weapon |
                super::artifacts::ArtifactCategory::Armor => ZTile::ArtifactPedestal,
                super::artifacts::ArtifactCategory::Jewelry |
                super::artifacts::ArtifactCategory::Treasure => ZTile::TreasureChest,
                super::artifacts::ArtifactCategory::Tome => ZTile::BookShelf,
                super::artifacts::ArtifactCategory::Relic => ZTile::RelicShrine,
                super::artifacts::ArtifactCategory::Instrument => ZTile::TreasureChest,
            };

            // Only place if the location is solid or a cave floor
            let current = *zlevels.get(x, y, z);
            if current == ZTile::Solid || current == ZTile::CaveFloor {
                zlevels.set(x, y, z, container);
            }
        }
    }

    // Place hero statues for famous heroes
    // (This would typically be at settlements, but for simplicity we place near burial sites)
    // Note: This is a simplified implementation

    println!("    Artifact evidence placed");
}
