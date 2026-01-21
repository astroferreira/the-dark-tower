//! Historical timeline generation
//!
//! Generates a timeline of historical events for the world, organized into eras.
//! Events create physical evidence that can be placed in the world.

use std::collections::HashMap;

use rand::Rng;
use rand_chacha::ChaCha8Rng;
use rand::SeedableRng;

use super::factions::FactionRegistry;
use super::naming::NameGenerator;
use super::types::*;

/// A historical event that occurred in the world
#[derive(Clone, Debug)]
pub struct HistoricalEvent {
    /// Unique identifier
    pub id: EventId,
    /// When this event occurred
    pub year: Year,
    /// Type of event
    pub event_type: EventType,
    /// Primary faction involved (if any)
    pub faction: Option<FactionId>,
    /// Secondary faction (for wars, alliances, etc.)
    pub other_faction: Option<FactionId>,
    /// Location of the event (if any)
    pub location: Option<(usize, usize)>,
    /// Settlement involved (if any)
    pub settlement: Option<SettlementId>,
    /// Name of the event (e.g., "Battle of Thornwall")
    pub name: String,
    /// Brief description
    pub description: String,
    /// Casualties (for battles, plagues, etc.)
    pub casualties: u32,
    /// Whether this event left physical evidence
    pub has_evidence: bool,
}

/// Types of historical events
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EventType {
    // Settlement events
    SettlementFounded,
    SettlementAbandoned,
    SettlementConquered,
    SettlementExpanded,
    SettlementDestroyed,

    // Military events
    Battle,
    Siege,
    Raid,
    Massacre,

    // Diplomatic events
    AllianceFormed,
    WarDeclared,
    TreatySigned,
    Betrayal,

    // Cataclysms
    VolcanicEruption,
    Earthquake,
    Plague,
    DragonAttack,
    MonsterInvasion,
    Flood,
    Famine,

    // Cultural events
    MonumentBuilt,
    ReligionFounded,
    GreatDiscovery,
    ArtifactCreated,
    HeroBorn,
    HeroDeath,

    // Faction events
    FactionFounded,
    FactionCollapsed,
    LeaderCrowned,
    CivilWar,
}

impl EventType {
    /// Get all event types
    pub fn all() -> &'static [EventType] {
        &[
            EventType::SettlementFounded,
            EventType::SettlementAbandoned,
            EventType::SettlementConquered,
            EventType::SettlementExpanded,
            EventType::SettlementDestroyed,
            EventType::Battle,
            EventType::Siege,
            EventType::Raid,
            EventType::Massacre,
            EventType::AllianceFormed,
            EventType::WarDeclared,
            EventType::TreatySigned,
            EventType::Betrayal,
            EventType::VolcanicEruption,
            EventType::Earthquake,
            EventType::Plague,
            EventType::DragonAttack,
            EventType::MonsterInvasion,
            EventType::Flood,
            EventType::Famine,
            EventType::MonumentBuilt,
            EventType::ReligionFounded,
            EventType::GreatDiscovery,
            EventType::ArtifactCreated,
            EventType::HeroBorn,
            EventType::HeroDeath,
            EventType::FactionFounded,
            EventType::FactionCollapsed,
            EventType::LeaderCrowned,
            EventType::CivilWar,
        ]
    }

    /// Check if this event leaves physical evidence
    pub fn leaves_evidence(&self) -> bool {
        matches!(self,
            EventType::Battle | EventType::Siege | EventType::Massacre |
            EventType::VolcanicEruption | EventType::Earthquake |
            EventType::DragonAttack | EventType::MonsterInvasion |
            EventType::MonumentBuilt | EventType::SettlementDestroyed |
            EventType::SettlementAbandoned | EventType::SettlementConquered |
            EventType::ArtifactCreated
        )
    }

    /// Get display name
    pub fn name(&self) -> &'static str {
        match self {
            EventType::SettlementFounded => "Settlement Founded",
            EventType::SettlementAbandoned => "Settlement Abandoned",
            EventType::SettlementConquered => "Settlement Conquered",
            EventType::SettlementExpanded => "Settlement Expanded",
            EventType::SettlementDestroyed => "Settlement Destroyed",
            EventType::Battle => "Battle",
            EventType::Siege => "Siege",
            EventType::Raid => "Raid",
            EventType::Massacre => "Massacre",
            EventType::AllianceFormed => "Alliance Formed",
            EventType::WarDeclared => "War Declared",
            EventType::TreatySigned => "Treaty Signed",
            EventType::Betrayal => "Betrayal",
            EventType::VolcanicEruption => "Volcanic Eruption",
            EventType::Earthquake => "Earthquake",
            EventType::Plague => "Plague",
            EventType::DragonAttack => "Dragon Attack",
            EventType::MonsterInvasion => "Monster Invasion",
            EventType::Flood => "Flood",
            EventType::Famine => "Famine",
            EventType::MonumentBuilt => "Monument Built",
            EventType::ReligionFounded => "Religion Founded",
            EventType::GreatDiscovery => "Great Discovery",
            EventType::ArtifactCreated => "Artifact Created",
            EventType::HeroBorn => "Hero Born",
            EventType::HeroDeath => "Hero Death",
            EventType::FactionFounded => "Faction Founded",
            EventType::FactionCollapsed => "Faction Collapsed",
            EventType::LeaderCrowned => "Leader Crowned",
            EventType::CivilWar => "Civil War",
        }
    }
}

/// A historical era (period of time with a theme)
#[derive(Clone, Debug)]
pub struct Era {
    /// Name of the era
    pub name: String,
    /// Type of era (affects event generation)
    pub era_type: EraType,
    /// Start year
    pub start: Year,
    /// End year
    pub end: Year,
    /// Events that occurred during this era
    pub events: Vec<EventId>,
}

impl Era {
    pub fn duration(&self) -> i32 {
        self.end.0 - self.start.0
    }
}

/// Complete timeline of world history
#[derive(Clone, Debug)]
pub struct Timeline {
    /// All eras in chronological order
    pub eras: Vec<Era>,
    /// All events by ID
    pub events: HashMap<EventId, HistoricalEvent>,
    /// Events by location
    pub events_by_location: HashMap<(usize, usize), Vec<EventId>>,
    /// Events by faction
    pub events_by_faction: HashMap<FactionId, Vec<EventId>>,
    /// Next available event ID
    next_id: u32,
}

impl Default for Timeline {
    fn default() -> Self {
        Self::new()
    }
}

impl Timeline {
    pub fn new() -> Self {
        Self {
            eras: Vec::new(),
            events: HashMap::new(),
            events_by_location: HashMap::new(),
            events_by_faction: HashMap::new(),
            next_id: 0,
        }
    }

    /// Add an event to the timeline
    pub fn add_event(&mut self, event: HistoricalEvent) {
        let id = event.id;

        // Index by location
        if let Some(loc) = event.location {
            self.events_by_location.entry(loc).or_default().push(id);
        }

        // Index by faction
        if let Some(faction) = event.faction {
            self.events_by_faction.entry(faction).or_default().push(id);
        }
        if let Some(faction) = event.other_faction {
            self.events_by_faction.entry(faction).or_default().push(id);
        }

        self.events.insert(id, event);
    }

    /// Generate a new unique event ID
    pub fn new_id(&mut self) -> EventId {
        let id = EventId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Get all events at a location
    pub fn events_at(&self, x: usize, y: usize) -> Vec<&HistoricalEvent> {
        self.events_by_location
            .get(&(x, y))
            .map(|ids| ids.iter().filter_map(|id| self.events.get(id)).collect())
            .unwrap_or_default()
    }

    /// Get all events for a faction
    pub fn events_for_faction(&self, faction: FactionId) -> Vec<&HistoricalEvent> {
        self.events_by_faction
            .get(&faction)
            .map(|ids| ids.iter().filter_map(|id| self.events.get(id)).collect())
            .unwrap_or_default()
    }

    /// Get events that leave evidence
    pub fn evidence_events(&self) -> Vec<&HistoricalEvent> {
        self.events.values().filter(|e| e.has_evidence).collect()
    }

    /// Get the current era
    pub fn current_era(&self) -> Option<&Era> {
        self.eras.last()
    }

    /// Get era for a given year
    pub fn era_for_year(&self, year: Year) -> Option<&Era> {
        self.eras.iter().find(|e| year.0 >= e.start.0 && year.0 <= e.end.0)
    }
}

/// Generate a complete timeline for the world
pub fn generate_timeline(
    factions: &FactionRegistry,
    map_width: usize,
    map_height: usize,
    seed: u64,
) -> Timeline {
    let mut rng = ChaCha8Rng::seed_from_u64(seed.wrapping_add(0x71BE11AE));
    let name_gen = NameGenerator::new(seed);
    let mut timeline = Timeline::new();

    // Determine timeline span based on oldest faction
    let oldest_year = factions.all()
        .map(|f| f.founded.0)
        .min()
        .unwrap_or(-1000);

    // Generate eras
    let eras = generate_eras(oldest_year, &mut rng, &name_gen);
    timeline.eras = eras;

    // Generate events for each era
    let era_count = timeline.eras.len();
    for era_idx in 0..era_count {
        let era = &timeline.eras[era_idx];
        let era_type = era.era_type;
        let start = era.start;
        let end = era.end;

        let events = generate_era_events(
            era_type,
            start,
            end,
            factions,
            map_width,
            map_height,
            &name_gen,
            &mut timeline,
            &mut rng,
        );

        // Add event IDs to era
        let era = &mut timeline.eras[era_idx];
        era.events = events;
    }

    // Generate faction-specific events (founding, collapse)
    generate_faction_events(factions, &name_gen, &mut timeline, &mut rng);

    println!("  Generated {} eras with {} events",
        timeline.eras.len(),
        timeline.events.len()
    );

    timeline
}

/// Generate eras for the timeline
fn generate_eras(oldest_year: i32, rng: &mut ChaCha8Rng, name_gen: &NameGenerator) -> Vec<Era> {
    let mut eras = Vec::new();
    let mut current_year = oldest_year;

    // Era sequence patterns
    let era_patterns = [
        EraType::Primordial,
        EraType::GoldenAge,
        EraType::GreatWar,
        EraType::DarkAge,
        EraType::Renaissance,
        EraType::Modern,
    ];

    // Generate 3-5 eras
    let num_eras = rng.gen_range(3..=5);
    let total_years = -oldest_year;
    let avg_era_length = total_years / num_eras as i32;

    for i in 0..num_eras {
        let era_type = era_patterns[i % era_patterns.len()];
        let era_length = rng.gen_range(avg_era_length / 2..avg_era_length * 3 / 2).max(50);

        let start = Year(current_year);
        let end = Year((current_year + era_length).min(0));

        let name = if rng.gen_bool(0.5) {
            name_gen.era_name(rng)
        } else {
            format!("The {}", era_type.name())
        };

        eras.push(Era {
            name,
            era_type,
            start,
            end,
            events: Vec::new(),
        });

        current_year = end.0;

        if current_year >= 0 {
            break;
        }
    }

    eras
}

/// Generate events for a specific era
fn generate_era_events(
    era_type: EraType,
    start: Year,
    end: Year,
    factions: &FactionRegistry,
    map_width: usize,
    map_height: usize,
    name_gen: &NameGenerator,
    timeline: &mut Timeline,
    rng: &mut ChaCha8Rng,
) -> Vec<EventId> {
    let mut event_ids = Vec::new();
    let duration = (end.0 - start.0).abs();

    // Number of events based on era duration and type
    let events_per_century = match era_type {
        EraType::Primordial => 2,
        EraType::GoldenAge => 5,
        EraType::GreatWar => 15,
        EraType::DarkAge => 8,
        EraType::Renaissance => 4,
        EraType::Modern => 3,
    };

    let num_events = ((duration * events_per_century / 100) as usize).max(3).min(50);

    // Get faction list for event generation
    let faction_ids: Vec<FactionId> = factions.factions.keys().copied().collect();

    for _ in 0..num_events {
        // Pick event type based on era
        let event_type = pick_event_type(era_type, rng);

        // Pick a random year within the era
        let year = Year(rng.gen_range(start.0..=end.0));

        // Pick random location
        let location = Some((
            rng.gen_range(0..map_width),
            rng.gen_range(0..map_height),
        ));

        // Pick faction(s) involved
        let faction = if faction_ids.is_empty() {
            None
        } else {
            Some(faction_ids[rng.gen_range(0..faction_ids.len())])
        };

        let other_faction = if needs_second_faction(event_type) && faction_ids.len() > 1 {
            let mut other = faction_ids[rng.gen_range(0..faction_ids.len())];
            while Some(other) == faction {
                other = faction_ids[rng.gen_range(0..faction_ids.len())];
            }
            Some(other)
        } else {
            None
        };

        // Generate event name
        let name = generate_event_name(event_type, faction, other_faction, factions, name_gen, rng);

        // Generate description
        let description = generate_event_description(event_type, &name, rng);

        // Calculate casualties
        let casualties = match event_type {
            EventType::Battle => rng.gen_range(100..5000),
            EventType::Siege => rng.gen_range(500..10000),
            EventType::Massacre => rng.gen_range(1000..20000),
            EventType::Plague => rng.gen_range(5000..100000),
            EventType::VolcanicEruption | EventType::Earthquake => rng.gen_range(100..10000),
            EventType::DragonAttack => rng.gen_range(50..2000),
            EventType::Famine => rng.gen_range(1000..50000),
            _ => 0,
        };

        let id = timeline.new_id();
        let event = HistoricalEvent {
            id,
            year,
            event_type,
            faction,
            other_faction,
            location,
            settlement: None,
            name,
            description,
            casualties,
            has_evidence: event_type.leaves_evidence(),
        };

        timeline.add_event(event);
        event_ids.push(id);
    }

    event_ids
}

/// Generate faction-specific events (founding, collapse, etc.)
fn generate_faction_events(
    factions: &FactionRegistry,
    name_gen: &NameGenerator,
    timeline: &mut Timeline,
    rng: &mut ChaCha8Rng,
) {
    for faction in factions.all() {
        // Faction founded event
        let id = timeline.new_id();
        let event = HistoricalEvent {
            id,
            year: faction.founded,
            event_type: EventType::FactionFounded,
            faction: Some(faction.id),
            other_faction: None,
            location: None,
            settlement: None,
            name: format!("Founding of the {}", faction.name),
            description: format!("The {} was established.", faction.name),
            casualties: 0,
            has_evidence: false,
        };
        timeline.add_event(event);

        // Faction collapse event
        if let Some(collapse_year) = faction.collapsed {
            let id = timeline.new_id();
            let reason = faction.collapse_reason.map(|r| r.name()).unwrap_or("unknown causes");
            let event = HistoricalEvent {
                id,
                year: collapse_year,
                event_type: EventType::FactionCollapsed,
                faction: Some(faction.id),
                other_faction: None,
                location: None,
                settlement: None,
                name: format!("Fall of the {}", faction.name),
                description: format!("The {} collapsed due to {}.", faction.name, reason),
                casualties: rng.gen_range(100..faction.peak_population / 10),
                has_evidence: true,
            };
            timeline.add_event(event);
        }
    }
}

/// Pick an event type appropriate for the era
fn pick_event_type(era_type: EraType, rng: &mut ChaCha8Rng) -> EventType {
    let options: Vec<(EventType, u32)> = match era_type {
        EraType::Primordial => vec![
            (EventType::SettlementFounded, 30),
            (EventType::MonumentBuilt, 15),
            (EventType::GreatDiscovery, 20),
            (EventType::MonsterInvasion, 15),
            (EventType::ReligionFounded, 10),
            (EventType::HeroBorn, 10),
        ],
        EraType::GoldenAge => vec![
            (EventType::SettlementFounded, 25),
            (EventType::SettlementExpanded, 20),
            (EventType::MonumentBuilt, 20),
            (EventType::AllianceFormed, 15),
            (EventType::ArtifactCreated, 10),
            (EventType::GreatDiscovery, 10),
        ],
        EraType::GreatWar => vec![
            (EventType::Battle, 25),
            (EventType::Siege, 20),
            (EventType::WarDeclared, 15),
            (EventType::SettlementConquered, 15),
            (EventType::SettlementDestroyed, 10),
            (EventType::Massacre, 8),
            (EventType::HeroDeath, 7),
        ],
        EraType::DarkAge => vec![
            (EventType::SettlementAbandoned, 20),
            (EventType::Plague, 15),
            (EventType::MonsterInvasion, 15),
            (EventType::Famine, 15),
            (EventType::Raid, 15),
            (EventType::SettlementDestroyed, 10),
            (EventType::FactionCollapsed, 10),
        ],
        EraType::Renaissance => vec![
            (EventType::SettlementFounded, 25),
            (EventType::TreatySigned, 20),
            (EventType::AllianceFormed, 15),
            (EventType::MonumentBuilt, 15),
            (EventType::GreatDiscovery, 15),
            (EventType::LeaderCrowned, 10),
        ],
        EraType::Modern => vec![
            (EventType::SettlementExpanded, 20),
            (EventType::AllianceFormed, 15),
            (EventType::TreatySigned, 15),
            (EventType::Battle, 10),
            (EventType::Raid, 10),
            (EventType::LeaderCrowned, 10),
            (EventType::MonumentBuilt, 10),
            (EventType::GreatDiscovery, 10),
        ],
    };

    let total: u32 = options.iter().map(|(_, w)| w).sum();
    let mut r = rng.gen_range(0..total);

    for (event_type, weight) in options {
        if r < weight {
            return event_type;
        }
        r -= weight;
    }

    EventType::Battle // Fallback
}

/// Check if an event type needs a second faction
fn needs_second_faction(event_type: EventType) -> bool {
    matches!(event_type,
        EventType::Battle | EventType::Siege | EventType::WarDeclared |
        EventType::AllianceFormed | EventType::TreatySigned |
        EventType::SettlementConquered | EventType::Betrayal
    )
}

/// Generate a name for an event
fn generate_event_name(
    event_type: EventType,
    faction: Option<FactionId>,
    other_faction: Option<FactionId>,
    factions: &FactionRegistry,
    name_gen: &NameGenerator,
    rng: &mut ChaCha8Rng,
) -> String {
    match event_type {
        EventType::Battle | EventType::Siege => {
            let location_name = name_gen.settlement_name(
                faction.and_then(|f| factions.get(f)).map(|f| f.species).unwrap_or(Species::Human),
                rng
            );
            name_gen.battle_name(&location_name, rng)
        }
        EventType::AllianceFormed => {
            let f1 = faction.and_then(|f| factions.get(f)).map(|f| f.name.as_str()).unwrap_or("Unknown");
            let f2 = other_faction.and_then(|f| factions.get(f)).map(|f| f.name.as_str()).unwrap_or("Unknown");
            format!("Alliance of {} and {}", f1, f2)
        }
        EventType::WarDeclared => {
            let f1 = faction.and_then(|f| factions.get(f)).map(|f| f.name.as_str()).unwrap_or("Unknown");
            let f2 = other_faction.and_then(|f| factions.get(f)).map(|f| f.name.as_str()).unwrap_or("Unknown");
            format!("{}-{} War", f1, f2)
        }
        EventType::TreatySigned => {
            let adjective = pick_random(rng, &["Great", "Eternal", "Sacred", "Iron", "Golden"]);
            format!("The {} Treaty", adjective)
        }
        EventType::Plague => {
            let adjective = pick_random(rng, &["Red", "Black", "White", "Silent", "Bloody"]);
            format!("The {} Plague", adjective)
        }
        EventType::DragonAttack => {
            let species = faction.and_then(|f| factions.get(f)).map(|f| f.species).unwrap_or(Species::Human);
            let dragon_name = name_gen.personal_name(Species::DragonKin, rng);
            format!("{}'s Rampage", dragon_name)
        }
        EventType::MonumentBuilt => {
            let noun = pick_random(rng, &["Tower", "Statue", "Temple", "Obelisk", "Tomb", "Arch"]);
            let adjective = pick_random(rng, &["Great", "Eternal", "Sacred", "Ancient", "Mighty"]);
            format!("The {} {}", adjective, noun)
        }
        EventType::ArtifactCreated => {
            let species = faction.and_then(|f| factions.get(f)).map(|f| f.species).unwrap_or(Species::Human);
            name_gen.artifact_name(species, rng)
        }
        EventType::HeroBorn | EventType::HeroDeath => {
            let species = faction.and_then(|f| factions.get(f)).map(|f| f.species).unwrap_or(Species::Human);
            let name = name_gen.personal_name(species, rng);
            if event_type == EventType::HeroBorn {
                format!("Birth of {}", name)
            } else {
                format!("Death of {}", name)
            }
        }
        _ => event_type.name().to_string(),
    }
}

/// Generate a description for an event
fn generate_event_description(event_type: EventType, name: &str, rng: &mut ChaCha8Rng) -> String {
    match event_type {
        EventType::Battle => {
            let outcome = pick_random(rng, &[
                "resulted in a decisive victory",
                "ended in a bloody stalemate",
                "saw heavy losses on both sides",
                "marked a turning point in the war",
            ]);
            format!("The {} {}.", name, outcome)
        }
        EventType::Siege => {
            let duration = rng.gen_range(1..24);
            format!("The siege lasted {} months.", duration)
        }
        EventType::Plague => {
            format!("{} swept across the land, leaving devastation in its wake.", name)
        }
        EventType::MonumentBuilt => {
            format!("{} was constructed to commemorate the era.", name)
        }
        _ => format!("{}.", name),
    }
}

/// Helper to pick a random element
fn pick_random<'a>(rng: &mut ChaCha8Rng, options: &[&'a str]) -> &'a str {
    options[rng.gen_range(0..options.len())]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tilemap::Tilemap;
    use crate::biomes::ExtendedBiome;
    use crate::history::factions::generate_factions;

    #[test]
    fn test_timeline_generation() {
        let heightmap = Tilemap::new_with(64, 32, 100.0f32);
        let biomes = Tilemap::new_with(64, 32, ExtendedBiome::TemperateGrassland);
        let factions = generate_factions(&heightmap, &biomes, 42);

        let timeline = generate_timeline(&factions, 64, 32, 42);

        assert!(!timeline.eras.is_empty(), "Should have at least one era");
        assert!(!timeline.events.is_empty(), "Should have events");

        println!("Eras: {}", timeline.eras.len());
        for era in &timeline.eras {
            println!("  {} ({} to {}): {} events",
                era.name, era.start, era.end, era.events.len()
            );
        }
    }
}
