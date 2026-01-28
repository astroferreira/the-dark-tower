//! History serialization and export.
//!
//! Provides save/load of WorldHistory to bincode files with world seed
//! reference, and export of legends to readable text/markdown.

use std::fs;
use std::io;
use std::path::Path;

use crate::history::world_state::WorldHistory;
use crate::history::IdGenerator;
use crate::history::legends::queries::legends_export;

use bincode;

/// Metadata wrapper for the save file format.
/// Includes the world seed so terrain can be regenerated.
#[derive(serde::Serialize, serde::Deserialize)]
struct HistorySaveFile {
    /// Format version for forward compatibility
    version: u32,
    /// The world terrain seed (for regenerating terrain)
    world_seed: u64,
    /// The history simulation seed
    history_seed: u64,
    /// The complete world history
    history: WorldHistory,
}

const SAVE_VERSION: u32 = 1;

/// Save world history to a binary file using bincode.
///
/// The file includes the world seed so terrain can be regenerated
/// alongside the history data.
pub fn save_history(
    history: &WorldHistory,
    world_seed: u64,
    history_seed: u64,
    path: &Path,
) -> io::Result<()> {
    let save = HistorySaveFile {
        version: SAVE_VERSION,
        world_seed,
        history_seed,
        history: history.clone(),
    };

    let bytes = bincode::serialize(&save).map_err(|e| {
        io::Error::new(io::ErrorKind::Other, format!("Serialization failed: {}", e))
    })?;

    fs::write(path, bytes)
}

/// Load results from loading a history file.
pub struct LoadedHistory {
    /// The loaded world history
    pub history: WorldHistory,
    /// The world seed for terrain regeneration
    pub world_seed: u64,
    /// The history simulation seed
    pub history_seed: u64,
}

/// Load world history from a binary file.
///
/// After loading, the ID generators are rebuilt from the maximum
/// existing IDs so new entities can be created.
pub fn load_history(path: &Path) -> io::Result<LoadedHistory> {
    let bytes = fs::read(path)?;

    let save: HistorySaveFile = bincode::deserialize(&bytes).map_err(|e| {
        io::Error::new(io::ErrorKind::InvalidData, format!("Deserialization failed: {}", e))
    })?;

    if save.version > SAVE_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Save file version {} is newer than supported version {}",
                save.version, SAVE_VERSION,
            ),
        ));
    }

    let mut history = save.history;

    // Rebuild ID generators from max existing IDs
    rebuild_id_generators(&mut history);

    Ok(LoadedHistory {
        history,
        world_seed: save.world_seed,
        history_seed: save.history_seed,
    })
}

/// Rebuild ID generators so they start after the highest existing IDs.
fn rebuild_id_generators(history: &mut WorldHistory) {
    let gens = &mut history.id_generators;

    gens.faction = IdGenerator::starting_at(
        history.factions.keys().map(|id| id.0 + 1).max().unwrap_or(0)
    );
    gens.settlement = IdGenerator::starting_at(
        history.settlements.keys().map(|id| id.0 + 1).max().unwrap_or(0)
    );
    gens.figure = IdGenerator::starting_at(
        history.figures.keys().map(|id| id.0 + 1).max().unwrap_or(0)
    );
    gens.dynasty = IdGenerator::starting_at(
        history.dynasties.keys().map(|id| id.0 + 1).max().unwrap_or(0)
    );
    gens.race = IdGenerator::starting_at(
        history.races.keys().map(|id| id.0 + 1).max().unwrap_or(0)
    );
    gens.culture = IdGenerator::starting_at(
        history.cultures.keys().map(|id| id.0 + 1).max().unwrap_or(0)
    );
    gens.creature_species = IdGenerator::starting_at(
        history.creature_species.keys().map(|id| id.0 + 1).max().unwrap_or(0)
    );
    gens.legendary_creature = IdGenerator::starting_at(
        history.legendary_creatures.keys().map(|id| id.0 + 1).max().unwrap_or(0)
    );
    gens.population = IdGenerator::starting_at(
        history.populations.keys().map(|id| id.0 + 1).max().unwrap_or(0)
    );
    gens.deity = IdGenerator::starting_at(
        history.deities.keys().map(|id| id.0 + 1).max().unwrap_or(0)
    );
    gens.religion = IdGenerator::starting_at(
        history.religions.keys().map(|id| id.0 + 1).max().unwrap_or(0)
    );
    gens.cult = IdGenerator::starting_at(
        history.cults.keys().map(|id| id.0 + 1).max().unwrap_or(0)
    );
    gens.artifact = IdGenerator::starting_at(
        history.artifacts.keys().map(|id| id.0 + 1).max().unwrap_or(0)
    );
    gens.monument = IdGenerator::starting_at(
        history.monuments.keys().map(|id| id.0 + 1).max().unwrap_or(0)
    );
    gens.event = IdGenerator::starting_at(
        history.chronicle.events.iter().map(|e| e.id.0 + 1).max().unwrap_or(0)
    );
    gens.era = IdGenerator::starting_at(
        history.timeline.eras.iter().map(|e| e.id.0 + 1).max().unwrap_or(0)
    );
    gens.army = IdGenerator::starting_at(
        history.armies.keys().map(|id| id.0 + 1).max().unwrap_or(0)
    );
    gens.war = IdGenerator::starting_at(
        history.wars.keys().map(|id| id.0 + 1).max().unwrap_or(0)
    );
}

/// Export legends summary to a plain text file.
pub fn export_legends_text(history: &WorldHistory, path: &Path) -> io::Result<()> {
    let text = legends_export(history);
    fs::write(path, text)
}

/// Export legends summary to a markdown file.
pub fn export_legends_markdown(history: &WorldHistory, path: &Path) -> io::Result<()> {
    let summary = history.summary();
    let mut md = String::new();

    md.push_str("# World History Legends\n\n");
    md.push_str(&format!("**Years simulated:** {}\n\n", summary.years_simulated));
    md.push_str("## Summary\n\n");
    md.push_str(&format!("| Stat | Value |\n"));
    md.push_str(&format!("|------|-------|\n"));
    md.push_str(&format!("| Events | {} ({} major) |\n", summary.total_events, summary.major_events));
    md.push_str(&format!("| Factions | {} ({} active) |\n", summary.total_factions, summary.active_factions));
    md.push_str(&format!("| Settlements | {} |\n", summary.total_settlements));
    md.push_str(&format!("| Figures | {} ({} living) |\n", summary.total_figures, summary.living_figures));
    md.push_str(&format!("| Dynasties | {} |\n", summary.total_dynasties));
    md.push_str(&format!("| Legendary Creatures | {} ({} alive) |\n", summary.legendary_creatures, summary.living_legendary));
    md.push_str(&format!("| Artifacts | {} |\n", summary.artifacts));
    md.push_str(&format!("| Monuments | {} |\n", summary.monuments));
    md.push_str(&format!("| Religions | {} |\n", summary.religions));
    md.push_str(&format!("| Wars | {} |\n", summary.wars));
    md.push_str(&format!("| Population | {} |\n", summary.total_population));

    // Eras
    if !history.timeline.eras.is_empty() {
        md.push_str("\n## Eras\n\n");
        for era in &history.timeline.eras {
            let end_str = era.end
                .map(|d| format!("Year {}", d.year))
                .unwrap_or_else(|| "present".to_string());
            md.push_str(&format!("- **{}**: Year {} - {}\n", era.name, era.start.year, end_str));
        }
    }

    // Factions
    md.push_str("\n## Factions\n\n");
    let mut factions: Vec<_> = history.factions.values().collect();
    factions.sort_by_key(|f| f.id.0);
    for faction in &factions {
        let status = if faction.is_active() { "active" } else { "dissolved" };
        md.push_str(&format!("### {} ({})\n\n", faction.name, status));
        md.push_str(&format!("- Founded: Year {}\n", faction.founded.year));
        md.push_str(&format!("- Settlements: {}\n", faction.settlements.len()));
        if let Some(leader_id) = faction.current_leader {
            if let Some(leader) = history.figures.get(&leader_id) {
                md.push_str(&format!("- Current leader: {}\n", leader.name));
            }
        }
        md.push_str("\n");
    }

    // Notable figures
    let notable: Vec<_> = history.figures.values()
        .filter(|f| !f.titles.is_empty() || !f.kills.is_empty())
        .collect();
    if !notable.is_empty() {
        md.push_str("## Notable Figures\n\n");
        for figure in notable {
            let alive = if figure.is_alive() { "living" } else { "deceased" };
            let epithet = figure.epithet.as_deref().unwrap_or("");
            let name_str = if epithet.is_empty() {
                figure.name.clone()
            } else {
                format!("{} {}", figure.name, epithet)
            };
            md.push_str(&format!("- **{}** ({})\n", name_str, alive));
        }
        md.push_str("\n");
    }

    // Legendary creatures
    if !history.legendary_creatures.is_empty() {
        md.push_str("## Legendary Creatures\n\n");
        for creature in history.legendary_creatures.values() {
            let alive = if creature.is_alive() { "alive" } else { "slain" };
            md.push_str(&format!("- **{} {}** ({})\n", creature.name, creature.epithet, alive));
        }
        md.push_str("\n");
    }

    // Wars
    if !history.wars.is_empty() {
        md.push_str("## Wars\n\n");
        let mut wars: Vec<_> = history.wars.values().collect();
        wars.sort_by_key(|w| w.started.year);
        for war in &wars {
            let end_str = war.ended
                .map(|d| format!("Year {}", d.year))
                .unwrap_or_else(|| "ongoing".to_string());
            md.push_str(&format!("- **{}**: Year {} - {}\n", war.name, war.started.year, end_str));
        }
        md.push_str("\n");
    }

    // Major events
    let major_events = history.chronicle.major_events();
    if !major_events.is_empty() {
        md.push_str("## Major Events\n\n");
        for event in major_events {
            md.push_str(&format!(
                "- **Year {}, {}**: {}\n",
                event.date.year,
                event.date.season.name(),
                event.title,
            ));
        }
    }

    fs::write(path, md)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::config::HistoryConfig;
    use crate::history::simulation::HistoryEngine;
    use crate::biomes::ExtendedBiome;
    use crate::tilemap::Tilemap;
    use crate::seeds::WorldSeeds;
    use crate::scale::MapScale;
    use crate::plates::types::PlateId;
    use crate::water_bodies::WaterBodyId;
    use crate::world::WorldData;
    use std::path::PathBuf;

    fn make_test_world() -> WorldData {
        let width = 64;
        let height = 32;
        let mut heightmap = Tilemap::new_with(width, height, 0.3);
        let mut biomes = Tilemap::new_with(width, height, ExtendedBiome::TemperateGrassland);
        for x in 0..width {
            *biomes.get_mut(x, 0) = ExtendedBiome::Ocean;
            *heightmap.get_mut(x, 0) = -0.1;
        }
        let seeds = WorldSeeds::from_master(42);
        let scale = MapScale::new(1.0);
        let temperature = Tilemap::new_with(width, height, 15.0);
        let moisture = Tilemap::new_with(width, height, 0.5);
        let stress_map = Tilemap::new_with(width, height, 0.0);
        let plate_map = Tilemap::new_with(width, height, PlateId(0));
        let water_body_map = Tilemap::new_with(width, height, WaterBodyId::NONE);
        let water_depth = Tilemap::new_with(width, height, 0.0);
        WorldData::new(
            seeds, scale, heightmap, temperature, moisture,
            biomes, stress_map, plate_map, Vec::new(),
            None, water_body_map, Vec::new(), water_depth,
            None, None,
        )
    }

    fn make_test_history() -> WorldHistory {
        let world = make_test_world();
        let config = HistoryConfig {
            simulation_years: 50,
            initial_civilizations: 3,
            ..HistoryConfig::default()
        };
        let mut engine = HistoryEngine::new(42);
        engine.simulate(&world, config)
    }

    #[test]
    fn test_save_and_load() {
        let history = make_test_history();
        let original_summary = history.summary();

        let tmp = std::env::temp_dir().join("planet_gen_test_history.bin");

        // Save
        save_history(&history, 42, 1042, &tmp).expect("save failed");

        // Verify file exists and is non-empty
        let metadata = fs::metadata(&tmp).expect("file should exist");
        assert!(metadata.len() > 100);

        // Load
        let loaded = load_history(&tmp).expect("load failed");
        assert_eq!(loaded.world_seed, 42);
        assert_eq!(loaded.history_seed, 1042);

        let loaded_summary = loaded.history.summary();
        assert_eq!(loaded_summary.years_simulated, original_summary.years_simulated);
        assert_eq!(loaded_summary.total_events, original_summary.total_events);
        assert_eq!(loaded_summary.total_factions, original_summary.total_factions);
        assert_eq!(loaded_summary.total_figures, original_summary.total_figures);

        // Verify ID generators were rebuilt (can generate new IDs)
        let mut h = loaded.history;
        let new_faction_id = h.id_generators.next_faction();
        assert!(new_faction_id.0 >= original_summary.total_factions as u64);

        // Cleanup
        let _ = fs::remove_file(&tmp);
    }

    #[test]
    fn test_export_text() {
        let history = make_test_history();
        let tmp = std::env::temp_dir().join("planet_gen_test_legends.txt");

        export_legends_text(&history, &tmp).expect("export failed");

        let content = fs::read_to_string(&tmp).expect("read failed");
        assert!(!content.is_empty());
        assert!(content.contains("World History"));

        let _ = fs::remove_file(&tmp);
    }

    #[test]
    fn test_export_markdown() {
        let history = make_test_history();
        let tmp = std::env::temp_dir().join("planet_gen_test_legends.md");

        export_legends_markdown(&history, &tmp).expect("export failed");

        let content = fs::read_to_string(&tmp).expect("read failed");
        assert!(content.starts_with("# World History"));
        assert!(content.contains("## Summary"));
        assert!(content.contains("## Factions"));

        let _ = fs::remove_file(&tmp);
    }
}
