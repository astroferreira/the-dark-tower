//! World history initialization.
//!
//! Sets up the initial state: races, cultures, factions, settlements,
//! creature species, legendary creatures, religions, etc.

use rand::Rng;
use crate::biomes::ExtendedBiome;
use crate::world::WorldData;
use crate::history::*;
use crate::history::config::HistoryConfig;
use crate::history::data::GameData;
use crate::history::time::Date;
use crate::history::naming::styles::{NamingStyle, NamingArchetype};
use crate::history::naming::generator::NameGenerator;
use crate::history::entities::races::{Race, RaceType};
use crate::history::entities::culture::Culture;
use crate::history::entities::traits::{Personality, DeathCause};
use crate::history::entities::figures::Figure;
use crate::history::entities::lineage::Dynasty;
use crate::history::civilizations::faction::Faction;
use crate::history::civilizations::settlement::{Settlement, SettlementType, WallLevel, BuildingType};
use crate::history::civilizations::economy::ResourceType;
use crate::history::civilizations::government::SuccessionLaw;
use crate::history::creatures::generator::CreatureSpecies;
use crate::history::creatures::legendary::{LegendaryCreature, generate_legendary_name};
use crate::history::creatures::populations::CreaturePopulation;
use crate::history::religion::deity::Deity;
use crate::history::religion::worship::{Religion, Doctrine};
use crate::history::events::types::{Event, EventType};
use crate::history::world_state::WorldHistory;
use crate::seasons::Season;

/// Initialize the world history with starting entities.
pub fn initialize_world(
    world: &WorldData,
    config: HistoryConfig,
    game_data: &GameData,
    rng: &mut impl Rng,
) -> WorldHistory {
    let start_date = Date::new(config.prehistory_depth + 1, Season::Spring);
    let mut history = WorldHistory::new(
        config.clone(),
        world.width,
        world.height,
        start_date,
    );

    // 1. Create naming styles (needed before races)
    let naming_styles = create_naming_styles(&mut history);

    // 2. Create races (needs naming_style ids)
    let races = create_races(&mut history, &naming_styles, game_data, rng);

    // 3. Create creature species for various biomes
    create_creature_species(&mut history, world, rng);

    // 4. Create legendary creatures
    create_legendary_creatures(&mut history, world, rng);

    // 5. Create factions with settlements and prehistory lineage
    create_factions(&mut history, world, &races, &naming_styles, game_data, rng);

    // 6. Create initial religions
    create_religions(&mut history, rng);

    history
}

fn create_naming_styles(
    history: &mut WorldHistory,
) -> Vec<(NamingStyleId, NamingStyle)> {
    let archetypes = [
        NamingArchetype::Harsh,
        NamingArchetype::Flowing,
        NamingArchetype::Compound,
        NamingArchetype::Guttural,
        NamingArchetype::Mystical,
        NamingArchetype::Sibilant,
        NamingArchetype::Ancient,
    ];

    let mut result = Vec::new();
    for archetype in &archetypes {
        let id = history.id_generators.next_naming_style();
        let style = NamingStyle::from_archetype(id, *archetype);
        result.push((id, style));
    }
    result
}

fn create_races(
    history: &mut WorldHistory,
    naming_styles: &[(NamingStyleId, NamingStyle)],
    game_data: &GameData,
    rng: &mut impl Rng,
) -> Vec<(RaceId, RaceType)> {
    let race_types = RaceType::all();

    let mut result = Vec::new();
    for race_type in race_types {
        let id = history.id_generators.next_race();
        let culture_id = history.id_generators.next_culture();

        // Pick naming style for race
        let naming_style_id = pick_naming_style_id(race_type, naming_styles);

        // Generate culture
        let culture = Culture::generate_with_data(
            culture_id,
            format!("{} Culture", race_type.plural_name()),
            naming_style_id,
            race_type,
            Some(game_data),
            rng,
        );
        let gov_type = culture.government_preference;
        history.cultures.insert(culture_id, culture);

        let race = Race::new(id, race_type.clone(), culture_id, race_type.plural_name().to_string());
        result.push((id, race_type.clone()));
        history.races.insert(id, race);
    }
    result
}

/// Find suitable land tiles for settlement placement, prioritizing attractive locations.
fn find_settlement_sites(
    world: &WorldData,
    count: usize,
    rng: &mut impl Rng,
) -> Vec<(usize, usize)> {
    let mut candidates: Vec<((usize, usize), i32)> = Vec::new();

    for y in 0..world.height {
        for x in 0..world.width {
            let biome = *world.biomes.get(x, y);
            let height = *world.heightmap.get(x, y);

            // Must be land, not extreme terrain
            let is_water = matches!(biome,
                ExtendedBiome::DeepOcean | ExtendedBiome::Ocean |
                ExtendedBiome::CoastalWater | ExtendedBiome::Lagoon
            );
            let is_extreme = matches!(biome,
                ExtendedBiome::Ice | ExtendedBiome::SnowyPeaks |
                ExtendedBiome::VolcanicWasteland | ExtendedBiome::LavaLake
            );

            if !is_water && !is_extreme && height > 0.0 && height < 0.85 {
                // Calculate desirability score
                let mut score = 10;
                
                // Prioritize rivers heavily (fresh water source)
                let has_river = world.river_network.as_ref()
                    .map_or(false, |rn| rn.has_significant_flow(x, y));
                if has_river {
                    score += 50;
                }

                // Prioritize coast (trade, fishing)
                let is_coastal = world.heightmap.neighbors_8(x, y).into_iter().any(|(nx, ny)| {
                    *world.heightmap.get(nx, ny) < 0.0
                });
                if is_coastal {
                    score += 20;
                }

                // Prioritize flat/low fertile land
                if height < 0.3 {
                    score += 5;
                }

                candidates.push(((x, y), score));
            }
        }
    }

    // Sort by score descending to pick best sites first
    candidates.sort_by(|a, b| b.1.cmp(&a.1));

    // Pick sites with minimum distance between them
    let mut sites = Vec::new();
    let min_dist_sq = ((world.width.min(world.height)) / (count * 2)).max(1).pow(2) as i64;

    // Try to pick from top candidates first
    // Take top 30% of candidates as the pool
    let pool_size = (candidates.len() / 3).max(count * 2);
    let pool = &candidates[0..pool_size.min(candidates.len())];

    // If pool is too small, use all candidates
    let search_pool = if pool.is_empty() { &candidates } else { pool };

    for _ in 0..count * 50 {
        if sites.len() >= count {
            break;
        }
        
        // Weighted random selection from pool could be better, but simple random from top tier is fine
        let idx = rng.gen_range(0..search_pool.len());
        let ((cx, cy), _) = search_pool[idx];

        let too_close = sites.iter().any(|&(sx, sy): &(usize, usize)| {
            let dx = cx as i64 - sx as i64;
            let dy = cy as i64 - sy as i64;
            dx * dx + dy * dy < min_dist_sq
        });

        if !too_close {
            sites.push((cx, cy));
        }
    }

    // Fallback if we couldn't find enough sites due to distance constraints
    if sites.len() < count {
         for _ in 0..(count - sites.len()) * 10 {
            if sites.len() >= count { break; }
            let idx = rng.gen_range(0..candidates.len()); // Search entire map
             let ((cx, cy), _) = candidates[idx];
             
             // Relax distance constraint slightly
             let too_close = sites.iter().any(|&(sx, sy): &(usize, usize)| {
                let dx = cx as i64 - sx as i64;
                let dy = cy as i64 - sy as i64;
                dx * dx + dy * dy < (min_dist_sq / 2)
            });
            if !too_close {
                sites.push((cx, cy));
            }
         }
    }

    sites
}

/// Pick a biome-appropriate race for a location.
fn pick_race_for_biome(
    biome: ExtendedBiome,
    races: &[(RaceId, RaceType)],
    rng: &mut impl Rng,
) -> (RaceId, RaceType) {
    // Weighted selection based on biome compatibility
    let preferences: Vec<(RaceId, &RaceType, f32)> = races.iter()
        .map(|(id, rt)| {
            let weight = match (rt, biome) {
                (RaceType::Dwarf, ExtendedBiome::SnowyPeaks | ExtendedBiome::AlpineTundra |
                 ExtendedBiome::Foothills) => 5.0,
                (RaceType::Elf, ExtendedBiome::TemperateForest | ExtendedBiome::TemperateRainforest |
                 ExtendedBiome::AncientGrove | ExtendedBiome::CloudForest) => 5.0,
                (RaceType::Orc, ExtendedBiome::Savanna |
                 ExtendedBiome::TemperateGrassland) => 5.0,
                (RaceType::Goblin, ExtendedBiome::Swamp | ExtendedBiome::Marsh |
                 ExtendedBiome::Bog) => 5.0,
                (RaceType::Human, _) => 3.0, // Humans can live anywhere
                (RaceType::Construct, _) => 0.1,
                (RaceType::Elemental, _) => 0.2,
                _ => 1.0,
            };
            (*id, rt, weight)
        })
        .collect();

    let total: f32 = preferences.iter().map(|p| p.2).sum();
    let mut roll: f32 = rng.gen::<f32>() * total;
    for &(id, rt, weight) in &preferences {
        roll -= weight;
        if roll <= 0.0 {
            return (id, rt.clone());
        }
    }
    (preferences[0].0, preferences[0].1.clone())
}

/// Pick the naming style ID that fits a race.
fn pick_naming_style_id(
    race_type: &RaceType,
    styles: &[(NamingStyleId, NamingStyle)],
) -> NamingStyleId {
    let idx = match race_type {
        RaceType::Dwarf => 0,       // Harsh
        RaceType::Elf => 1,         // Flowing
        RaceType::Human => 2,       // Compound
        RaceType::Orc | RaceType::Goblin => 3, // Guttural
        RaceType::Fey => 4,         // Mystical
        RaceType::Reptilian => 5,   // Sibilant
        RaceType::Giant | RaceType::Undead | RaceType::Construct => 6, // Ancient
        _ => 2,                     // Default to Compound
    };
    let safe_idx = idx.min(styles.len() - 1);
    styles[safe_idx].0
}

/// Get the NamingStyle reference for a race.
fn get_naming_style<'a>(
    race_type: &RaceType,
    styles: &'a [(NamingStyleId, NamingStyle)],
) -> &'a NamingStyle {
    let idx = match race_type {
        RaceType::Dwarf => 0,
        RaceType::Elf => 1,
        RaceType::Human => 2,
        RaceType::Orc | RaceType::Goblin => 3,
        RaceType::Fey => 4,
        RaceType::Reptilian => 5,
        RaceType::Giant | RaceType::Undead | RaceType::Construct => 6,
        _ => 2,
    };
    let safe_idx = idx.min(styles.len() - 1);
    &styles[safe_idx].1
}

fn create_creature_species(
    history: &mut WorldHistory,
    world: &WorldData,
    rng: &mut impl Rng,
) {
    // Create one species per major biome type present in the world
    let mut seen_biomes = std::collections::HashSet::new();
    let sample_count = 200.min(world.width * world.height / 50);

    for _ in 0..sample_count {
        let x = rng.gen_range(0..world.width);
        let y = rng.gen_range(0..world.height);
        let biome = *world.biomes.get(x, y);
        if seen_biomes.insert(biome) {
            let id = history.id_generators.next_creature_species();
            let species = CreatureSpecies::generate(id, biome, rng);
            history.creature_species.insert(id, species);
        }
    }
}

fn create_legendary_creatures(
    history: &mut WorldHistory,
    world: &WorldData,
    rng: &mut impl Rng,
) {
    let count = history.config.initial_legendary_creatures as usize;
    let species_ids: Vec<CreatureSpeciesId> = history.creature_species.keys().copied().collect();
    if species_ids.is_empty() {
        return;
    }

    let prehistory_depth = history.config.prehistory_depth;
    let start_date = history.current_date;

    for _ in 0..count {
        let species_id = species_ids[rng.gen_range(0..species_ids.len())];
        let id = history.id_generators.next_legendary_creature();
        let (name, epithet) = generate_legendary_name(rng);

        // Spread creature birth dates across the prehistory period
        let birth_year = if prehistory_depth > 1 {
            rng.gen_range(1..=start_date.year)
        } else {
            start_date.year
        };
        let birth_date = Date::new(birth_year, random_season(rng));

        let mut creature = LegendaryCreature::new(
            id, species_id, name, epithet, Some(birth_date),
        );
        creature.generate_unique_abilities(rng);
        creature.generate_size_multiplier(rng);

        // Place in a random land tile
        for _ in 0..50 {
            let x = rng.gen_range(0..world.width);
            let y = rng.gen_range(0..world.height);
            let biome = *world.biomes.get(x, y);
            let is_water = matches!(biome,
                ExtendedBiome::DeepOcean | ExtendedBiome::Ocean |
                ExtendedBiome::CoastalWater
            );
            if !is_water {
                creature.lair_location = Some((x, y));
                creature.territory.push((x, y));

                // Record lair event
                let event_id = history.id_generators.next_event();
                let event = Event::new(
                    event_id,
                    EventType::LairEstablished,
                    birth_date,
                    format!("{} claims its lair", creature.full_name()),
                    format!("The legendary {} established its lair.", creature.full_name()),
                )
                .at_location(x, y)
                .with_participant(EntityId::LegendaryCreature(id));
                history.chronicle.record(event);
                history.tile_history.record_event(x, y, event_id);
                break;
            }
        }

        // Create a population around this creature
        let pop_id = history.id_generators.next_population();
        let pop_location = creature.lair_location.unwrap_or((0, 0));
        let mut pop = CreaturePopulation::new(
            pop_id, species_id, rng.gen_range(10..100), pop_location,
        );
        pop.set_leader(id);
        history.populations.insert(pop_id, pop);

        history.legendary_creatures.insert(id, creature);
    }
}

fn create_factions(
    history: &mut WorldHistory,
    world: &WorldData,
    races: &[(RaceId, RaceType)],
    naming_styles: &[(NamingStyleId, NamingStyle)],
    game_data: &GameData,
    rng: &mut impl Rng,
) {
    let count = history.config.initial_civilizations as usize;
    let sites = find_settlement_sites(world, count, rng);
    let start_date = history.current_date;
    let prehistory_depth = history.config.prehistory_depth;
    let prehistory_generations = history.config.prehistory_generations;

    for &(sx, sy) in sites.iter() {
        let biome = *world.biomes.get(sx, sy);
        let (race_id, race_type) = pick_race_for_biome(biome, races, rng);
        let style = get_naming_style(&race_type, naming_styles);

        // Get government type from the race's culture
        let gov_type = history.races.get(&race_id)
            .and_then(|r| history.cultures.get(&r.culture_id))
            .map(|c| c.government_preference)
            .unwrap_or(crate::history::entities::culture::GovernmentType::Monarchy);

        // Vary founding date within the prehistory window
        let founding_age = if prehistory_depth > 20 {
            rng.gen_range(20..=prehistory_depth)
        } else if prehistory_depth > 0 {
            rng.gen_range(1..=prehistory_depth)
        } else {
            0
        };
        let founding_date = Date::new(
            start_date.year.saturating_sub(founding_age),
            random_season(rng),
        );

        // Create faction
        let faction_id = history.id_generators.next_faction();
        let race_label = format!("{:?}", race_type);
        let faction_name = NameGenerator::faction_name(&style, rng, &race_label);
        let succession_law = SuccessionLaw::for_government(gov_type, rng);
        let mut faction = Faction::new(
            faction_id, faction_name.clone(), race_id,
            founding_date, gov_type, succession_law,
        );

        // Create founding settlement (capital)
        let settlement_id = history.id_generators.next_settlement();
        let settlement_name = NameGenerator::place_name(&style, rng);
        let local_resources = ResourceType::from_biome(biome);
        let mut settlement = Settlement::new(
            settlement_id, settlement_name.clone(),
            SettlementType::Capital, (sx, sy),
            faction_id, founding_date, local_resources,
        );

        // Scale settlement by age: compound 2% growth per year
        if founding_age > 0 {
            let growth_factor = (1.02_f64).powi(founding_age as i32);
            let grown_pop = (settlement.population as f64 * growth_factor) as u32;
            settlement.population = grown_pop.min(settlement.population_cap);

            // Add walls based on age
            settlement.walls = if founding_age >= 150 {
                WallLevel::Fortified
            } else if founding_age >= 100 {
                WallLevel::StoneWall
            } else if founding_age >= 50 {
                WallLevel::Palisade
            } else {
                WallLevel::None
            };

            // Add buildings based on age
            if founding_age >= 30 {
                settlement.buildings.push(BuildingType::Granary);
            }
            if founding_age >= 60 {
                settlement.buildings.push(BuildingType::Market);
            }
            if founding_age >= 90 {
                settlement.buildings.push(BuildingType::Barracks);
            }
            if founding_age >= 120 {
                settlement.buildings.push(BuildingType::Smithy);
            }
        }

        faction.add_settlement(settlement_id);
        faction.total_population = settlement.population;

        // Scale faction wealth and military by age
        faction.wealth = 100 + founding_age * 5;
        faction.military_strength = founding_age * 2;

        // Create dynasty
        let dynasty_id = history.id_generators.next_dynasty();

        // Generate prehistory lineage (ancestors + current leader)
        let (founder_id, current_leader_id, dynasty) = create_prehistory_lineage(
            history,
            &style,
            race_id,
            &race_type,
            game_data,
            faction_id,
            dynasty_id,
            &faction_name,
            founding_date,
            start_date,
            succession_law,
            prehistory_generations,
            (sx, sy),
            rng,
        );

        // Set up the current leader
        faction.ruling_dynasty = Some(dynasty_id);
        faction.current_leader = Some(current_leader_id);
        faction.notable_figures.push(founder_id);
        if current_leader_id != founder_id {
            faction.notable_figures.push(current_leader_id);
        }

        // Set territory
        history.tile_history.set_owner(sx, sy, faction_id, founding_date);

        // Record founding events (at the faction's founding date, not start_date)
        let founder_name = history.figures.get(&founder_id)
            .map(|f| f.name.clone())
            .unwrap_or_default();
        let event_id = history.id_generators.next_event();
        let event = Event::new(
            event_id,
            EventType::FactionFounded,
            founding_date,
            format!("Founding of {}", faction_name),
            format!("{} founded {} at {}.", founder_name, faction_name, settlement_name),
        )
        .at_location(sx, sy)
        .with_faction(faction_id)
        .with_participant(EntityId::Faction(faction_id))
        .with_participant(EntityId::Figure(founder_id));
        history.chronicle.record(event);
        history.tile_history.record_event(sx, sy, event_id);

        let settle_event_id = history.id_generators.next_event();
        let settle_event = Event::new(
            settle_event_id,
            EventType::SettlementFounded,
            founding_date,
            format!("Founding of {}", settlement_name),
            format!("{} was established as the capital of {}.", settlement_name, faction_name),
        )
        .at_location(sx, sy)
        .with_faction(faction_id)
        .with_participant(EntityId::Settlement(settlement_id))
        .caused_by(event_id);
        history.chronicle.record(settle_event);
        history.chronicle.link_cause_effect(event_id, settle_event_id);

        // Store remaining entities
        history.dynasties.insert(dynasty_id, dynasty);
        history.settlements.insert(settlement_id, settlement);
        history.factions.insert(faction_id, faction);
    }
}

/// Generate a chain of ancestor rulers for a faction's prehistory.
///
/// Returns `(founder_id, current_leader_id, dynasty)`.
/// All ancestor figures and their events are inserted directly into `history`.
fn create_prehistory_lineage(
    history: &mut WorldHistory,
    style: &NamingStyle,
    race_id: RaceId,
    race_type: &RaceType,
    game_data: &GameData,
    faction_id: FactionId,
    dynasty_id: DynastyId,
    faction_name: &str,
    founding_date: Date,
    start_date: Date,
    succession_law: SuccessionLaw,
    max_generations: u32,
    location: (usize, usize),
    rng: &mut impl Rng,
) -> (FigureId, FigureId, Dynasty) {
    let faction_age = start_date.year.saturating_sub(founding_date.year);
    let (lifespan_min, lifespan_max) = race_type.lifespan();
    let maturity = race_type.maturity_age();

    // For immortal races or zero-age factions, just create a single founder/leader
    let effective_lifespan = if lifespan_max == 0 { 500 } else { lifespan_max };
    let ruling_span = effective_lifespan.saturating_sub(maturity);

    // Determine how many generations fit.
    // Rulers typically don't reign for their entire adult life — cap avg reign
    // at 40 years to produce more realistic multi-generation dynasties.
    let gen_count = if faction_age == 0 || ruling_span == 0 {
        1
    } else {
        let raw_avg_reign = if lifespan_max > 0 {
            (lifespan_min.saturating_sub(maturity) + ruling_span) / 2
        } else {
            ruling_span
        };
        let avg_reign = raw_avg_reign.min(40);
        let natural_gens = if avg_reign > 0 { faction_age / avg_reign + 1 } else { 1 };
        natural_gens.min(max_generations).max(1)
    };

    let tag = race_type.tag();
    let ruler_title = game_data.backstory.random_ruler_title(tag, rng);

    // First, create the dynasty founder's name for the dynasty name
    let founder_name_str = NameGenerator::personal_name(style, rng);
    let dynasty_name = game_data.backstory.dynasty_name(tag, &founder_name_str, rng);

    let mut prev_figure_id: Option<FigureId> = None;
    let mut prev_name: Option<String> = None;
    let mut founder_id: Option<FigureId> = None;
    let mut dynasty_members: Vec<FigureId> = Vec::new();

    // Work through generations from oldest ancestor to current
    for gen in 0..gen_count {
        let is_last = gen == gen_count - 1;
        let figure_id = history.id_generators.next_figure();

        let name = if gen == 0 {
            founder_name_str.clone()
        } else {
            NameGenerator::personal_name(style, rng)
        };

        // Calculate birth/death dates for this generation
        let (birth_date, death_date, reign_start) = if gen_count == 1 {
            // Single generation: born before founding, still alive
            let birth_year = founding_date.year.saturating_sub(maturity + rng.gen_range(0..10));
            (
                Date::new(birth_year.max(1), random_season(rng)),
                None,
                founding_date,
            )
        } else {
            // Spread generations across the faction's age
            let gen_span = faction_age / gen_count;
            let gen_start_year = founding_date.year + gen * gen_span;

            let birth_year = if gen == 0 {
                // Founder born before founding
                founding_date.year.saturating_sub(maturity + rng.gen_range(0..10))
            } else {
                // Born during previous ruler's reign
                gen_start_year.saturating_sub(maturity + rng.gen_range(0..5))
            };

            let reign_start_year = if gen == 0 {
                founding_date.year
            } else {
                gen_start_year
            };

            if is_last {
                // Current ruler: alive
                (
                    Date::new(birth_year.max(1), random_season(rng)),
                    None,
                    Date::new(reign_start_year.max(1), random_season(rng)),
                )
            } else {
                // Past ruler: dead
                let death_year = reign_start_year + gen_span + rng.gen_range(0..5);
                let death_year = death_year.min(start_date.year - 1).max(reign_start_year + 1);
                (
                    Date::new(birth_year.max(1), random_season(rng)),
                    Some(Date::new(death_year, random_season(rng))),
                    Date::new(reign_start_year.max(1), random_season(rng)),
                )
            }
        };

        let personality = Personality::random(rng);
        let mut figure = Figure::new(figure_id, name.clone(), race_id, birth_date, personality);
        figure.faction = Some(faction_id);
        figure.dynasty = Some(dynasty_id);

        // Wire parent-child links
        if let Some(parent_id) = prev_figure_id {
            figure.parents.0 = Some(parent_id);
            if let Some(parent) = history.figures.get_mut(&parent_id) {
                parent.add_child(figure_id);
            }
        }

        // Assign an epithet to dead rulers (and occasionally the living one)
        if !is_last || rng.gen_bool(0.3) {
            figure.epithet = Some(game_data.backstory.random_epithet(tag, rng));
        }

        // Assign titles — race-specific and role-specific
        if gen == 0 {
            figure.titles.push(format!("Founder of {}", faction_name));
            figure.titles.push(format!("First {} of {}", ruler_title, faction_name));
        }
        if is_last {
            figure.titles.push(format!("{} of {}", ruler_title, faction_name));
        } else {
            // Don't use generic "Former Ruler" — the title itself is descriptive
        }

        // Record coronation event with varied descriptions
        let crown_event_id = history.id_generators.next_event();
        let (crown_title, crown_desc) = game_data.backstory.coronation_description(
            &name, faction_name, &ruler_title, gen, prev_name.as_deref(), tag, rng,
        );
        let crown_event = Event::new(
            crown_event_id,
            EventType::RulerCrowned,
            reign_start,
            crown_title,
            crown_desc,
        )
        .at_location(location.0, location.1)
        .with_faction(faction_id)
        .with_participant(EntityId::Figure(figure_id));
        history.chronicle.record(crown_event);
        figure.events.push(crown_event_id);

        // Generate mid-reign backstory events (1-3 per generation for dead rulers)
        if !is_last {
            let reign_end_year = death_date.map(|d| d.year).unwrap_or(start_date.year);
            let reign_years = reign_end_year.saturating_sub(reign_start.year);
            if reign_years > 2 {
                let num_events = rng.gen_range(1..=3u32).min(reign_years / 3);
                for _ in 0..num_events {
                    let event_year = rng.gen_range(reign_start.year + 1..reign_end_year);
                    let event_date = Date::new(event_year, random_season(rng));
                    generate_reign_event(
                        history, &figure, faction_id, faction_name, event_date,
                        location, style, race_type, game_data, rng,
                    );
                }
            }
        }

        // Record death for past rulers with varied descriptions
        if let Some(d_date) = death_date {
            let cause = random_death_cause(rng);
            figure.kill(d_date, cause);

            let death_event_id = history.id_generators.next_event();
            let cause_str = format!("{:?}", cause);
            let (death_title, death_desc) = game_data.backstory.death_description(
                &figure.full_name(), &name, faction_name, &cause_str, rng,
            );
            let death_event = Event::new(
                death_event_id,
                EventType::HeroDied,
                d_date,
                death_title,
                death_desc,
            )
            .at_location(location.0, location.1)
            .with_faction(faction_id)
            .with_participant(EntityId::Figure(figure_id));
            history.chronicle.record(death_event);
            figure.events.push(death_event_id);
        }

        if gen == 0 {
            founder_id = Some(figure_id);
        }
        dynasty_members.push(figure_id);
        prev_name = Some(name);
        history.figures.insert(figure_id, figure);
        prev_figure_id = Some(figure_id);
    }

    let f_id = founder_id.unwrap();
    let current_leader_id = prev_figure_id.unwrap();

    let mut dynasty = Dynasty::new(
        dynasty_id, dynasty_name, founding_date, f_id, succession_law,
    );
    dynasty.current_head = Some(current_leader_id);
    dynasty.generations = gen_count;
    dynasty.factions_ruled.push(faction_id);
    for &member in &dynasty_members {
        if member != f_id {
            dynasty.add_member(member);
        }
    }

    (f_id, current_leader_id, dynasty)
}

/// Generate a backstory event during a ruler's reign.
fn generate_reign_event(
    history: &mut WorldHistory,
    ruler: &Figure,
    faction_id: FactionId,
    faction_name: &str,
    date: Date,
    location: (usize, usize),
    style: &NamingStyle,
    race_type: &RaceType,
    game_data: &GameData,
    rng: &mut impl Rng,
) {
    let ruler_name = ruler.full_name();
    let event_id = history.id_generators.next_event();
    let tag = race_type.tag();
    let bs = &game_data.backstory;

    // Pick a random reign event template from the data
    let templates = &bs.reign_event_templates;
    let template = &templates[rng.gen_range(0..templates.len())];

    // Resolve template placeholders
    let place = NameGenerator::place_name(style, rng);
    let enemy = bs.random_enemy(tag, rng);
    let plague = bs.random_plague(rng);
    let beast = bs.random_beast(rng);
    let adj = bs.random_faction_adjective(rng);
    let rebel = NameGenerator::personal_name(style, rng);
    let artifact = NameGenerator::artifact_name(style, rng);

    let title = template.title
        .replace("{PLACE}", &place)
        .replace("{RULER}", &ruler_name)
        .replace("{NAME}", &ruler.name)
        .replace("{FACTION}", faction_name)
        .replace("{ENEMY}", &enemy)
        .replace("{PLAGUE}", &plague)
        .replace("{BEAST}", &beast)
        .replace("{ADJ}", &adj)
        .replace("{REBEL}", &rebel)
        .replace("{ARTIFACT}", &artifact);

    let description = template.desc
        .replace("{PLACE}", &place)
        .replace("{RULER}", &ruler_name)
        .replace("{NAME}", &ruler.name)
        .replace("{FACTION}", faction_name)
        .replace("{ENEMY}", &enemy)
        .replace("{PLAGUE}", &plague)
        .replace("{BEAST}", &beast)
        .replace("{ADJ}", &adj)
        .replace("{REBEL}", &rebel)
        .replace("{ARTIFACT}", &artifact);

    // Map event type string from template to actual EventType
    let event_type = match template.event_type.as_str() {
        "BattleFought" => EventType::BattleFought,
        "Raid" => EventType::Raid,
        "SettlementGrew" => EventType::SettlementGrew,
        "MonumentBuilt" => EventType::MonumentBuilt,
        "TempleBuilt" => EventType::TempleBuilt,
        "TreatySigned" => EventType::TreatySigned,
        "TradeRouteEstablished" => EventType::TradeRouteEstablished,
        "Plague" => EventType::Plague,
        "Drought" => EventType::Drought,
        "Flood" => EventType::Flood,
        "QuestCompleted" => EventType::QuestCompleted,
        "ArtifactFound" => EventType::ArtifactFound,
        "ArtifactCreated" => EventType::ArtifactCreated,
        "Rebellion" => EventType::Rebellion,
        "Coup" => EventType::Coup,
        "Miracle" => EventType::Miracle,
        "SpellInvented" => EventType::SpellInvented,
        "MonsterRaid" => EventType::MonsterRaid,
        "MasterworkCreated" => EventType::MasterworkCreated,
        "WarDeclared" => EventType::WarDeclared,
        _ => EventType::Other,
    };

    let event = Event::new(event_id, event_type, date, title, description)
        .at_location(location.0, location.1)
        .with_faction(faction_id)
        .with_participant(EntityId::Figure(ruler.id));
    history.chronicle.record(event);
}

fn create_religions(
    history: &mut WorldHistory,
    rng: &mut impl Rng,
) {
    let faction_ids: Vec<FactionId> = history.factions.keys().copied().collect();

    // Each faction gets a patron deity and religion
    for &faction_id in &faction_ids {
        let (faction_name, faction_founded_year) = history.factions.get(&faction_id)
            .map(|f| (f.name.clone(), f.founded.year))
            .unwrap_or_default();

        // Vary religion founding date relative to faction founding (-30 to +50 years)
        let offset = rng.gen_range(-30i32..50);
        let religion_year = (faction_founded_year as i32 + offset).max(1) as u32;
        let religion_date = Date::new(religion_year, random_season(rng));

        // Create deity
        let deity_id = history.id_generators.next_deity();
        let deity_name = generate_deity_name(rng);
        let deity = Deity::new_god(deity_id, deity_name, rng);

        // Create religion
        let religion_id = history.id_generators.next_religion();
        let religion_name = generate_religion_name(&deity.name, rng);

        // Find the ancestor figure alive at the religion's founding date, or fall back to current leader
        let founder_id = find_figure_alive_at(history, faction_id, &religion_date)
            .or_else(|| history.factions.get(&faction_id).and_then(|f| f.current_leader));

        let mut religion = Religion::new(
            religion_id,
            religion_name,
            vec![deity_id],
            religion_date,
            founder_id,
        );
        // Assign 1-3 doctrines biased by the faction's culture values
        let culture_values = history.factions.get(&faction_id)
            .and_then(|f| history.races.get(&f.race_id))
            .and_then(|r| history.cultures.get(&r.culture_id))
            .map(|c| c.values.clone());
        let num_doctrines = rng.gen_range(1..=3u32);
        let assigned = pick_doctrines(culture_values.as_ref(), num_doctrines, rng);
        religion.doctrines = assigned;

        religion.add_follower_faction(faction_id);
        religion.follower_count = history.factions.get(&faction_id)
            .map(|f| f.total_population)
            .unwrap_or(0);

        // Set faction's state religion
        if let Some(faction) = history.factions.get_mut(&faction_id) {
            faction.state_religion = Some(religion_id);
        }

        // Record event
        let event_id = history.id_generators.next_event();
        let event = Event::new(
            event_id,
            EventType::ReligionFounded,
            religion_date,
            format!("Founding of {}", religion.name),
            format!("{} was founded by {}.", religion.name, faction_name),
        )
        .with_faction(faction_id);
        history.chronicle.record(event);

        history.deities.insert(deity_id, deity);
        history.religions.insert(religion_id, religion);
    }
}

/// Find a figure belonging to a faction who was alive at the given date.
fn find_figure_alive_at(
    history: &WorldHistory,
    faction_id: FactionId,
    date: &Date,
) -> Option<FigureId> {
    history.figures.values()
        .filter(|f| f.faction == Some(faction_id))
        .filter(|f| f.birth_date <= *date)
        .filter(|f| match f.death_date {
            Some(d) => d >= *date,
            None => true,
        })
        .map(|f| f.id)
        .next()
}

// === Helper functions ===

/// Pick doctrines for a religion, biased by the founding culture's values.
fn pick_doctrines(
    culture: Option<&crate::history::entities::culture::CultureValues>,
    count: u32,
    rng: &mut impl Rng,
) -> Vec<Doctrine> {
    let all_doctrines = [
        Doctrine::Pacifism,
        Doctrine::HolyWar,
        Doctrine::Asceticism,
        Doctrine::Indulgence,
        Doctrine::Proselytizing,
        Doctrine::Isolationism,
        Doctrine::AncestorVeneration,
        Doctrine::NatureWorship,
        Doctrine::SacrificeRequired,
        Doctrine::MagicForbidden,
        Doctrine::MagicEncouraged,
        Doctrine::MonasticTradition,
    ];

    // Build weighted list based on culture
    let weights: Vec<(Doctrine, f32)> = all_doctrines.iter().map(|&d| {
        let w = if let Some(cv) = culture {
            match d {
                Doctrine::HolyWar => 0.5 + cv.martial * 2.0,
                Doctrine::Pacifism => 0.5 + (1.0 - cv.martial) * 2.0,
                Doctrine::Asceticism => 0.5 + cv.tradition * 1.5,
                Doctrine::Indulgence => 0.5 + cv.wealth * 1.5,
                Doctrine::Proselytizing => 0.5 + (1.0 - cv.xenophobia) * 1.5,
                Doctrine::Isolationism => 0.5 + cv.xenophobia * 1.5,
                Doctrine::AncestorVeneration => 0.5 + cv.tradition * 1.5,
                Doctrine::NatureWorship => 0.5 + cv.nature_harmony * 2.0,
                Doctrine::SacrificeRequired => 0.3 + cv.martial * 1.0,
                Doctrine::MagicForbidden => 0.5 + (1.0 - cv.magic_acceptance) * 2.0,
                Doctrine::MagicEncouraged => 0.5 + cv.magic_acceptance * 2.0,
                Doctrine::MonasticTradition => 0.5 + cv.collectivism * 1.5,
            }
        } else {
            1.0
        };
        (d, w)
    }).collect();

    // Exclude contradictory pairs
    let contradictions: &[(Doctrine, Doctrine)] = &[
        (Doctrine::Pacifism, Doctrine::HolyWar),
        (Doctrine::Asceticism, Doctrine::Indulgence),
        (Doctrine::Proselytizing, Doctrine::Isolationism),
        (Doctrine::MagicForbidden, Doctrine::MagicEncouraged),
    ];

    let mut selected: Vec<Doctrine> = Vec::new();
    for _ in 0..count {
        // Filter out already-selected and contradictory doctrines
        let available: Vec<(Doctrine, f32)> = weights.iter()
            .filter(|(d, _)| !selected.contains(d))
            .filter(|(d, _)| {
                !selected.iter().any(|s| {
                    contradictions.iter().any(|(a, b)| {
                        (*a == *d && *b == *s) || (*b == *d && *a == *s)
                    })
                })
            })
            .copied()
            .collect();

        if available.is_empty() {
            break;
        }

        let total: f32 = available.iter().map(|(_, w)| w).sum();
        let mut roll = rng.gen::<f32>() * total;
        for (d, w) in &available {
            roll -= w;
            if roll <= 0.0 {
                selected.push(*d);
                break;
            }
        }
    }
    selected
}

/// Generate a deity name by combining syllables.
fn generate_deity_name(rng: &mut impl Rng) -> String {
    let prefixes = [
        "Ael", "Bal", "Cor", "Dra", "Eth", "Fal", "Gol", "Hel", "Ith", "Kal",
        "Lor", "Mor", "Nul", "Oth", "Pyr", "Quel", "Ral", "Sol", "Tyr", "Ul",
        "Val", "Wyr", "Xar", "Yol", "Zar", "Ash", "Bel", "Cyr", "Dur", "Gar",
    ];
    let middles = [
        "an", "en", "ar", "or", "el", "al", "ur", "ir", "on", "in",
        "ath", "eth", "ith", "oth", "uth", "ak", "ek", "ok", "uk", "",
    ];
    let suffixes = [
        "us", "os", "is", "as", "es", "ion", "eon", "ius", "ath", "or",
        "ar", "el", "al", "un", "en", "ir", "ax", "ix", "ox", "ur",
        "iel", "ael", "oth", "esh", "orn", "and", "ond", "ith", "ym", "an",
    ];
    let prefix = prefixes[rng.gen_range(0..prefixes.len())];
    let middle = middles[rng.gen_range(0..middles.len())];
    let suffix = suffixes[rng.gen_range(0..suffixes.len())];
    format!("{}{}{}", prefix, middle, suffix)
}

/// Generate a varied religion name from its deity name.
/// Produces names like "The Sacred Order of X", "The Path of X",
/// "Followers of the Eternal X", "The X Covenant", etc.
fn generate_religion_name(deity_name: &str, rng: &mut impl Rng) -> String {
    let patterns: &[fn(&str, &mut dyn FnMut(usize) -> usize) -> String] = &[
        |name, _rng| format!("The Sacred Order of {}", name),
        |name, _rng| format!("The Path of {}", name),
        |name, _rng| format!("The {} Covenant", name),
        |name, _rng| format!("Children of {}", name),
        |name, _rng| format!("The {} Doctrine", name),
        |name, _rng| format!("Followers of the Eternal {}", name),
        |name, _rng| format!("The Church of {}", name),
        |name, _rng| format!("The {} Mysteries", name),
        |name, _rng| format!("The Cult of {}", name),
        |name, _rng| format!("Disciples of {}", name),
        |name, _rng| format!("The {} Revelation", name),
        |name, _rng| format!("Brotherhood of {}", name),
        |name, _rng| format!("Seekers of {}", name),
        |name, _rng| format!("The {} Communion", name),
        |name, _rng| format!("The Way of {}", name),
        |name, _rng| format!("The {} Ascendancy", name),
    ];
    let idx = rng.gen_range(0..patterns.len());
    let mut counter_fn = |max: usize| rng.gen_range(0..max);
    patterns[idx](deity_name, &mut counter_fn)
}

/// Pick a random season.
fn random_season(rng: &mut impl Rng) -> Season {
    match rng.gen_range(0..4u8) {
        0 => Season::Spring,
        1 => Season::Summer,
        2 => Season::Autumn,
        _ => Season::Winter,
    }
}

/// Race-specific ruler title.
fn ruler_title_for_race(race_type: &RaceType, rng: &mut impl Rng) -> String {
    let titles: &[&str] = match race_type {
        RaceType::Human => &["King", "Queen", "Lord", "Duke", "Emperor", "Sovereign", "Regent"],
        RaceType::Dwarf => &["Thane", "High King", "Lord Under Mountain", "Forge-Lord", "Iron King"],
        RaceType::Elf => &["High Lord", "Archon", "Elder Sovereign", "Star-Lord", "Warden"],
        RaceType::Orc => &["Warlord", "Warchief", "Overlord", "Blood King", "Skull-Thane"],
        RaceType::Goblin => &["Great Boss", "Tyrant", "Despot", "Under-King", "Sneak-Lord"],
        RaceType::Halfling => &["Mayor", "Burgher", "Elder", "Steward", "Provost"],
        RaceType::Reptilian => &["Scale-Lord", "Brood-King", "Sun-Sovereign", "Fang-Lord"],
        RaceType::Fey => &["Faerie Lord", "Dream-Sovereign", "Twilight Monarch", "Archfey"],
        RaceType::Undead => &["Lich-Lord", "Death-King", "Bone Sovereign", "Dread Monarch"],
        RaceType::Elemental => &["Primarch", "Elemental Lord", "Storm-Sovereign", "Essence-King"],
        RaceType::Beastfolk => &["Alpha", "Pack-Lord", "Chieftain", "Horn-King", "Fang-Chief"],
        RaceType::Giant => &["Titan-Lord", "Mountain-King", "Jarl", "Storm-King", "Stone-Father"],
        RaceType::Construct => &["Prime Architect", "Core-Sovereign", "Grand Automaton", "Logic-Lord"],
        RaceType::Custom(_) => &["King", "Lord", "Ruler", "Sovereign"],
    };
    titles[rng.gen_range(0..titles.len())].to_string()
}

/// Race-flavored dynasty name.
fn dynasty_name_for_race(race_type: &RaceType, founder: &str, rng: &mut impl Rng) -> String {
    let patterns: &[&str] = match race_type {
        RaceType::Dwarf => &["Clan {}", "The {}-forge Line", "House of {}", "The {} Halls"],
        RaceType::Elf => &["House of {}", "The {}-star Line", "The Lineage of {}", "The {} Court"],
        RaceType::Orc => &["The Blood of {}", "Clan {}", "{}'s Horde", "The {} War-Line"],
        RaceType::Goblin => &["{}'s Brood", "The {} Gang", "Clan {}", "The {} Clutch"],
        RaceType::Fey => &["The {} Dream-Line", "Court of {}", "The {} Bloom", "Circle of {}"],
        RaceType::Giant => &["The {} Lineage", "Kin of {}", "{}'s Bloodline", "The {} Stone-Line"],
        RaceType::Undead => &["The {} Crypt-Line", "Legacy of {}", "The Eternal {}", "The {} Pact"],
        _ => &["House of {}", "The {} Dynasty", "The Line of {}", "House {}"],
    };
    let pattern = patterns[rng.gen_range(0..patterns.len())];
    pattern.replace("{}", founder)
}

/// Generate a varied coronation event description.
fn coronation_description(
    name: &str,
    faction_name: &str,
    ruler_title: &str,
    generation: u32,
    predecessor_name: Option<&str>,
    race_type: &RaceType,
    rng: &mut impl Rng,
) -> (String, String) {
    if generation == 0 {
        // Founding ruler — unique flavor
        let templates: &[(&str, &str)] = match race_type {
            RaceType::Dwarf => &[
                ("The Founding of {F}", "{N} struck the first anvil and claimed the mountain halls, becoming the first {T} of {F}."),
                ("{N} declares the founding of {F}", "With hammer raised and oath sworn before the deep stone, {N} founded {F} and took the title of {T}."),
                ("The First Forging of {F}", "In the heart of the mountain, {N} lit the Great Forge and declared the founding of {F}."),
            ],
            RaceType::Elf => &[
                ("The Awakening of {F}", "Under the starlit canopy, {N} was chosen by the eldest trees to become the first {T} of {F}."),
                ("{N} founds {F}", "{N} spoke the Words of Binding and wove the first wards, founding {F} in the ancient grove."),
                ("The Planting of {F}", "With a seed from the World-Tree, {N} planted the first grove and declared the founding of {F}."),
            ],
            RaceType::Orc => &[
                ("{N} seizes power", "{N} defeated all challengers in single combat and claimed the title of {T}, founding {F} in blood."),
                ("The Founding of {F}", "By crushing all rivals, {N} unified the scattered warbands into {F}."),
                ("Rise of {N}", "{N} raised the war-banner and the tribes rallied, marking the brutal founding of {F}."),
            ],
            _ => &[
                ("The Founding of {F}", "{N} gathered followers and established {F}, becoming its first {T}."),
                ("{N} founds {F}", "With vision and determination, {N} laid the foundations of {F} and was proclaimed its first {T}."),
                ("Rise of {F}", "From humble beginnings, {N} united the people and declared the founding of {F}."),
            ],
        };
        let (t_tpl, d_tpl) = templates[rng.gen_range(0..templates.len())];
        let title = t_tpl.replace("{N}", name).replace("{F}", faction_name).replace("{T}", ruler_title);
        let desc = d_tpl.replace("{N}", name).replace("{F}", faction_name).replace("{T}", ruler_title);
        (title, desc)
    } else {
        // Successor ruler — varied succession flavor
        let pred = predecessor_name.unwrap_or("the previous ruler");
        let templates: &[(&str, &str)] = &[
            ("{N} ascends to the throne of {F}",
             "Following the passing of {P}, {N} was crowned {T} of {F} in a solemn ceremony."),
            ("Coronation of {N}",
             "The elders proclaimed {N} as the new {T} of {F}, successor to {P}."),
            ("{N} inherits rule of {F}",
             "By right of blood, {N} inherited the mantle of {T} from {P} and swore the ancient oaths."),
            ("The council chooses {N}",
             "After the death of {P}, a council of advisors chose {N} to lead {F} as its next {T}."),
            ("{N} seizes the throne",
             "In the turmoil following {P}'s death, {N} moved swiftly to claim the title of {T} of {F}."),
            ("{N} crowned {T} of {F}",
             "With the crown of their forebears upon their brow, {N} became {T} of {F}, continuing the legacy of {P}."),
            ("A new {T} for {F}",
             "The people of {F} rallied behind {N}, child of {P}, as their new {T}."),
        ];
        let (t_tpl, d_tpl) = templates[rng.gen_range(0..templates.len())];
        let title = t_tpl.replace("{N}", name).replace("{F}", faction_name).replace("{T}", ruler_title).replace("{P}", pred);
        let desc = d_tpl.replace("{N}", name).replace("{F}", faction_name).replace("{T}", ruler_title).replace("{P}", pred);
        (title, desc)
    }
}

/// Generate a varied death event description.
fn death_description(
    full_name: &str,
    short_name: &str,
    faction_name: &str,
    cause: DeathCause,
    _race_type: &RaceType,
    rng: &mut impl Rng,
) -> (String, String) {
    match cause {
        DeathCause::Natural => {
            let templates: &[(&str, &str)] = &[
                ("The passing of {N}", "{N} died peacefully in their chambers, surrounded by loyal attendants. The people of {F} mourned for a season."),
                ("{S} passes into legend", "After a long life, {N} breathed their last. {F} observed a year of mourning."),
                ("Death of {N}", "{N} succumbed to the weight of years, their rule remembered as a time of stability for {F}."),
                ("The final rest of {S}", "Old and weary, {S} retired to private chambers and never emerged. {F} honored their memory with a grand funeral."),
            ];
            let (t, d) = templates[rng.gen_range(0..templates.len())];
            (t.replace("{N}", full_name).replace("{S}", short_name).replace("{F}", faction_name),
             d.replace("{N}", full_name).replace("{S}", short_name).replace("{F}", faction_name))
        }
        DeathCause::Battle => {
            let templates: &[(&str, &str)] = &[
                ("{N} falls in battle", "{N} was struck down on the battlefield while leading the forces of {F}. Their body was carried home on their shield."),
                ("The last stand of {N}", "{N} made a heroic last stand against overwhelming odds, buying time for the retreat of {F}'s armies."),
                ("{S} slain in combat", "In the chaos of battle, {N} fell to an enemy blade. The warriors of {F} fought bitterly to recover the body."),
                ("Death of {N} at the front", "{N} refused to command from the rear and paid the ultimate price, falling amid the din of battle."),
            ];
            let (t, d) = templates[rng.gen_range(0..templates.len())];
            (t.replace("{N}", full_name).replace("{S}", short_name).replace("{F}", faction_name),
             d.replace("{N}", full_name).replace("{S}", short_name).replace("{F}", faction_name))
        }
        DeathCause::Disease => {
            let diseases = ["the Crimson Fever", "the Wasting Sickness", "the Grey Pox", "a plague from the east",
                           "the Bone Rot", "an unknown malady", "the Shaking Death", "the Blood Cough"];
            let disease = diseases[rng.gen_range(0..diseases.len())];
            let templates: &[(&str, &str)] = &[
                ("{N} succumbs to {D}", "{N} was struck down by {D}. Despite the efforts of healers, the ruler of {F} could not be saved."),
                ("Plague claims {N}", "{D} claimed {N}, plunging {F} into grief and fear as the sickness spread."),
                ("The sickness of {S}", "{N} fell ill with {D} and lingered for weeks before death took them. {F} prayed for deliverance."),
            ];
            let (t, d) = templates[rng.gen_range(0..templates.len())];
            (t.replace("{N}", full_name).replace("{S}", short_name).replace("{F}", faction_name).replace("{D}", disease),
             d.replace("{N}", full_name).replace("{S}", short_name).replace("{F}", faction_name).replace("{D}", disease))
        }
        DeathCause::Assassination => {
            let templates: &[(&str, &str)] = &[
                ("Assassination of {N}", "{N} was found dead in their chambers, a poisoned blade beside them. The assassin was never caught, and {F} erupted in suspicion."),
                ("{N} murdered", "A shadowy conspiracy claimed the life of {N}. The court of {F} descended into paranoia as loyalists hunted for the killers."),
                ("The betrayal of {S}", "{N} was betrayed by a trusted advisor and murdered in the dead of night. {F} teetered on the brink of chaos."),
                ("{S} poisoned", "During a feast, {N} was poisoned by an unknown hand. The subsequent purge of the court left {F} diminished."),
            ];
            let (t, d) = templates[rng.gen_range(0..templates.len())];
            (t.replace("{N}", full_name).replace("{S}", short_name).replace("{F}", faction_name),
             d.replace("{N}", full_name).replace("{S}", short_name).replace("{F}", faction_name))
        }
        DeathCause::Duel => {
            let templates: &[(&str, &str)] = &[
                ("{N} slain in a duel", "{N} accepted a challenge of honor and fell to their opponent's blade. The duelists of {F} sang laments for a year."),
                ("The duel of {S}", "Challenged by a rival claimant, {N} fought with valor but was mortally wounded. {F} buried their ruler with full honors."),
                ("{S} dies by the sword", "In a dispute over honor, {N} was challenged and slain. The victorious challenger fled the wrath of {F}."),
            ];
            let (t, d) = templates[rng.gen_range(0..templates.len())];
            (t.replace("{N}", full_name).replace("{S}", short_name).replace("{F}", faction_name),
             d.replace("{N}", full_name).replace("{S}", short_name).replace("{F}", faction_name))
        }
        _ => {
            let templates: &[(&str, &str)] = &[
                ("The mysterious disappearance of {N}", "{N} vanished without a trace. Whether they met a hidden fate or chose exile, {F} was left leaderless."),
                ("{S} lost to history", "The circumstances of {N}'s death remain unknown. Some say they fell to treachery; others whisper of darker fates."),
                ("The unknown fate of {N}", "{N} embarked on a journey and never returned. {F} searched for years, but no trace was ever found."),
            ];
            let (t, d) = templates[rng.gen_range(0..templates.len())];
            (t.replace("{N}", full_name).replace("{S}", short_name).replace("{F}", faction_name),
             d.replace("{N}", full_name).replace("{S}", short_name).replace("{F}", faction_name))
        }
    }
}

/// Generate a random ruler epithet, with race-specific flavor.
fn random_epithet(race_type: &RaceType, rng: &mut impl Rng) -> String {
    let common = [
        "the Bold", "the Wise", "the Cruel", "the Just", "the Brave",
        "the Old", "the Young", "the Great", "the Terrible", "the Pious",
        "the Conqueror", "the Builder", "the Unifier", "the Stern",
        "the Merciful", "the Silent", "the Cunning", "the Fair",
        "the Ironhanded", "the Peacemaker", "the Magnificent", "the Dreaded",
        "the Benevolent", "the Relentless", "the Scarred", "the Wanderer",
        "the Resolute", "the Unyielding", "the Lawgiver", "the Oathbreaker",
        "the Accursed", "the Beloved", "the Feared", "the Uncrowned",
        "the Usurper", "the Restorer", "the Reformer", "the Damned",
        "the Golden", "the Silver-tongued",
    ];

    let race_specific: &[&str] = match race_type {
        RaceType::Dwarf => &[
            "Stonefist", "Ironbeard", "the Deep Delver", "Goldvein", "Hammersong",
            "the Anvil", "the Tunneler", "Shieldwall", "Forgeborn", "Mountainheart",
        ],
        RaceType::Elf => &[
            "the Starborn", "Moonwhisper", "the Ageless", "Leafsinger", "Dawnbringer",
            "the Luminous", "Sunshadow", "the Eternal Watcher", "Windwalker", "Graceblade",
        ],
        RaceType::Orc => &[
            "Skullcrusher", "Bloodfang", "the Savage", "Bonecruncher", "Ironjaw",
            "the Butcher", "Doomhammer", "the Merciless", "Warscream", "Goreclaw",
        ],
        RaceType::Goblin => &[
            "the Sneak", "Poisonfinger", "Backstabber", "the Rat", "Quickknife",
            "the Slippery", "Shadowbite", "the Schemer", "Wormtongue", "the Trickster",
        ],
        RaceType::Fey => &[
            "the Dreaming", "Thornweaver", "Mistshroud", "the Enchanted", "Moonpetal",
            "the Illusionist", "Dewdancer", "the Whimsical", "Shimmerscale", "Wildbloom",
        ],
        RaceType::Giant => &[
            "Earthshaker", "the Mountain", "Thunderstride", "Skyreacher", "the Colossus",
            "Boulderfist", "the Immovable", "Stormcaller", "Peakbreaker", "the Vast",
        ],
        RaceType::Undead => &[
            "the Deathless", "Soulstealer", "the Withered", "Gravecaller", "the Eternal",
            "Bonelord", "the Hollow", "Nightbringer", "the Cursed", "Dustwalker",
        ],
        _ => &[],
    };

    // 40% chance of race-specific epithet if available
    if !race_specific.is_empty() && rng.gen_bool(0.4) {
        race_specific[rng.gen_range(0..race_specific.len())].to_string()
    } else {
        common[rng.gen_range(0..common.len())].to_string()
    }
}

/// Pick a random death cause with weighted distribution.
fn random_death_cause(rng: &mut impl Rng) -> DeathCause {
    let roll = rng.gen_range(0..100u32);
    match roll {
        0..=39 => DeathCause::Natural,
        40..=59 => DeathCause::Battle,
        60..=74 => DeathCause::Disease,
        75..=84 => DeathCause::Assassination,
        85..=89 => DeathCause::Duel,
        _ => DeathCause::Unknown,
    }
}

/// Random enemy descriptor for backstory events.
fn random_enemy_name(race_type: &RaceType, rng: &mut impl Rng) -> &'static str {
    let enemies: &[&str] = match race_type {
        RaceType::Dwarf => &["goblin", "orc", "troll", "dark elf", "drake"],
        RaceType::Elf => &["orc", "troll", "undead", "dark fey", "spider-kin"],
        RaceType::Orc => &["human", "elf", "dwarf", "rival orc", "ogre"],
        RaceType::Goblin => &["dwarf", "human", "rival goblin", "kobold", "gnoll"],
        RaceType::Fey => &["undead", "iron-wielder", "shadow creature", "blighted beast", "mortal"],
        RaceType::Undead => &["paladin", "cleric", "living", "radiant fey", "exorcist"],
        _ => &["barbarian", "bandit", "marauder", "pirate", "raider", "nomad", "warlord"],
    };
    enemies[rng.gen_range(0..enemies.len())]
}

/// Random adjective for a neighboring faction in backstory events.
fn random_faction_adjective(rng: &mut impl Rng) -> &'static str {
    let adjectives = [
        "northern", "southern", "eastern", "western", "highland", "lowland",
        "river", "mountain", "forest", "coastal", "desert", "marsh",
        "iron", "golden", "silver", "storm", "shadow", "frost",
    ];
    adjectives[rng.gen_range(0..adjectives.len())]
}

/// Random plague/disease name for backstory events.
fn random_plague_name(rng: &mut impl Rng) -> &'static str {
    let plagues = [
        "Crimson Fever", "Grey Pox", "Bone Rot", "Wasting Sickness",
        "Blood Cough", "Shaking Death", "Pale Plague", "Shadow Blight",
        "Iron Sickness", "Weeping Pox", "Rat Fever", "Spore Lung",
        "Corpse Chill", "Night Sweats", "Scale Rot", "Moon Madness",
    ];
    plagues[rng.gen_range(0..plagues.len())]
}

/// Random beast/monster name for backstory events.
fn random_beast_name(rng: &mut impl Rng) -> &'static str {
    let beasts = [
        "wyrm", "troll", "giant spider", "dire wolf", "basilisk",
        "chimera", "wyvern", "manticore", "hydra", "drake",
        "griffon", "cockatrice", "behemoth", "kraken",
        "thunderbird", "shadow stalker", "bone golem", "cave bear",
        "frost giant", "fire elemental", "swamp thing", "barrow wight",
    ];
    beasts[rng.gen_range(0..beasts.len())]
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;
    use crate::tilemap::Tilemap;
    use crate::seeds::WorldSeeds;
    use crate::scale::MapScale;
    use crate::plates::PlateId;
    use crate::water_bodies::WaterBodyId;

    fn make_test_world() -> WorldData {
        let width = 64;
        let height = 32;
        let mut heightmap = Tilemap::new_with(width, height, 0.3);
        let mut biomes = Tilemap::new_with(width, height, ExtendedBiome::TemperateGrassland);

        // Add some water in the first row
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

    #[test]
    fn test_initialize_world() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let world = make_test_world();
        let game_data = crate::history::data::GameData::defaults();
        let config = HistoryConfig {
            initial_civilizations: 3,
            initial_legendary_creatures: 5,
            ..HistoryConfig::default()
        };
        let history = initialize_world(&world, config, &game_data, &mut rng);

        // Should have factions
        assert!(history.factions.len() >= 1, "Expected at least 1 faction, got {}", history.factions.len());
        // Each faction should have a capital
        for faction in history.factions.values() {
            assert!(faction.capital.is_some());
            assert!(!faction.settlements.is_empty());
            assert!(faction.current_leader.is_some());
        }
        // Should have races
        assert_eq!(history.races.len(), 13);
        // Should have events
        assert!(!history.chronicle.is_empty());
        // Should have creatures
        assert!(!history.creature_species.is_empty());
        // Should have legendary creatures
        assert!(!history.legendary_creatures.is_empty());
        // Should have religions
        assert!(!history.religions.is_empty());

        // Prehistory: start date should be offset
        assert_eq!(history.current_date.year, 201); // default prehistory_depth=200

        // Factions should have varied founding dates (not all the same)
        let founding_years: Vec<u32> = history.factions.values()
            .map(|f| f.founded.year)
            .collect();
        if founding_years.len() > 1 {
            let all_same = founding_years.windows(2).all(|w| w[0] == w[1]);
            assert!(!all_same, "Faction founding years should vary: {:?}", founding_years);
        }

        // Should have dead ancestor figures
        let dead_count = history.figures.values().filter(|f| !f.is_alive()).count();
        assert!(dead_count > 0, "Expected dead ancestor figures from prehistory");

        // Dynasties should have generations > 1 (at least some)
        let multi_gen = history.dynasties.values().any(|d| d.generations > 1);
        assert!(multi_gen, "Expected at least one dynasty with multiple generations");

        eprintln!("{}", history.summary());
    }

    #[test]
    fn test_initialize_world_no_prehistory() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let world = make_test_world();
        let game_data = crate::history::data::GameData::defaults();
        let config = HistoryConfig {
            initial_civilizations: 3,
            initial_legendary_creatures: 5,
            prehistory_depth: 0,
            prehistory_generations: 1,
            ..HistoryConfig::default()
        };
        let history = initialize_world(&world, config, &game_data, &mut rng);

        // With no prehistory, start date should be year 1
        assert_eq!(history.current_date.year, 1);

        // All factions should be founded at year 1
        for faction in history.factions.values() {
            assert_eq!(faction.founded.year, 1);
        }

        // No dead ancestors (all figures alive)
        let dead_count = history.figures.values().filter(|f| !f.is_alive()).count();
        assert_eq!(dead_count, 0, "No dead ancestors expected with prehistory_depth=0");
    }

    #[test]
    fn test_find_settlement_sites() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let world = make_test_world();
        let sites = find_settlement_sites(&world, 5, &mut rng);
        assert!(!sites.is_empty());
        // All sites should be on land
        for &(x, y) in &sites {
            let biome = *world.biomes.get(x, y);
            assert!(!matches!(biome, ExtendedBiome::Ocean | ExtendedBiome::DeepOcean));
        }
    }
}
