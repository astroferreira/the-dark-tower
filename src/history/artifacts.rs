//! Artifacts as lore carriers
//!
//! Artifacts are not just items - they're vessels for information that move through
//! the world: books contain philosophies, weapons have ownership chains, relics
//! carry religious beliefs.

use std::collections::HashMap;

use rand::Rng;
use rand_chacha::ChaCha8Rng;
use rand::SeedableRng;

use super::types::*;
use super::naming::NameGenerator;
use super::factions::FactionRegistry;
use super::heroes::{Hero, HeroRegistry, HeroRole};
use super::monsters::{MonsterRegistry, BiomeCategory};

/// Category of artifact
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ArtifactCategory {
    Weapon,
    Armor,
    Jewelry,
    Relic,
    Tome,
    Treasure,
    Instrument,
}

impl ArtifactCategory {
    pub fn name(&self) -> &'static str {
        match self {
            ArtifactCategory::Weapon => "Weapon",
            ArtifactCategory::Armor => "Armor",
            ArtifactCategory::Jewelry => "Jewelry",
            ArtifactCategory::Relic => "Relic",
            ArtifactCategory::Tome => "Tome",
            ArtifactCategory::Treasure => "Treasure",
            ArtifactCategory::Instrument => "Instrument",
        }
    }
}

/// Specific type of artifact
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ArtifactType {
    // Weapons
    Sword,
    Axe,
    Hammer,
    Bow,
    Staff,
    Spear,
    Dagger,
    // Armor
    Helm,
    Shield,
    Breastplate,
    Gauntlets,
    Cloak,
    // Jewelry
    Ring,
    Amulet,
    Crown,
    Scepter,
    Circlet,
    // Relics
    HolySymbol,
    Chalice,
    Orb,
    Bone,
    Urn,
    // Tomes
    Spellbook,
    HistoryTome,
    ProphecyScroll,
    Codex,
    Treatise,
    PhilosophyTome,
    // Treasures
    Gem,
    GoldStatuette,
    Goblet,
    Idol,
    // Instruments
    Harp,
    Horn,
    Drum,
}

impl ArtifactType {
    pub fn name(&self) -> &'static str {
        match self {
            ArtifactType::Sword => "Sword",
            ArtifactType::Axe => "Axe",
            ArtifactType::Hammer => "Hammer",
            ArtifactType::Bow => "Bow",
            ArtifactType::Staff => "Staff",
            ArtifactType::Spear => "Spear",
            ArtifactType::Dagger => "Dagger",
            ArtifactType::Helm => "Helm",
            ArtifactType::Shield => "Shield",
            ArtifactType::Breastplate => "Breastplate",
            ArtifactType::Gauntlets => "Gauntlets",
            ArtifactType::Cloak => "Cloak",
            ArtifactType::Ring => "Ring",
            ArtifactType::Amulet => "Amulet",
            ArtifactType::Crown => "Crown",
            ArtifactType::Scepter => "Scepter",
            ArtifactType::Circlet => "Circlet",
            ArtifactType::HolySymbol => "Holy Symbol",
            ArtifactType::Chalice => "Chalice",
            ArtifactType::Orb => "Orb",
            ArtifactType::Bone => "Sacred Bone",
            ArtifactType::Urn => "Urn",
            ArtifactType::Spellbook => "Spellbook",
            ArtifactType::HistoryTome => "History Tome",
            ArtifactType::ProphecyScroll => "Prophecy Scroll",
            ArtifactType::Codex => "Codex",
            ArtifactType::Treatise => "Treatise",
            ArtifactType::PhilosophyTome => "Philosophy Tome",
            ArtifactType::Gem => "Gem",
            ArtifactType::GoldStatuette => "Gold Statuette",
            ArtifactType::Goblet => "Goblet",
            ArtifactType::Idol => "Idol",
            ArtifactType::Harp => "Harp",
            ArtifactType::Horn => "Horn",
            ArtifactType::Drum => "Drum",
        }
    }

    pub fn category(&self) -> ArtifactCategory {
        match self {
            ArtifactType::Sword | ArtifactType::Axe | ArtifactType::Hammer |
            ArtifactType::Bow | ArtifactType::Staff | ArtifactType::Spear |
            ArtifactType::Dagger => ArtifactCategory::Weapon,

            ArtifactType::Helm | ArtifactType::Shield | ArtifactType::Breastplate |
            ArtifactType::Gauntlets | ArtifactType::Cloak => ArtifactCategory::Armor,

            ArtifactType::Ring | ArtifactType::Amulet | ArtifactType::Crown |
            ArtifactType::Scepter | ArtifactType::Circlet => ArtifactCategory::Jewelry,

            ArtifactType::HolySymbol | ArtifactType::Chalice | ArtifactType::Orb |
            ArtifactType::Bone | ArtifactType::Urn => ArtifactCategory::Relic,

            ArtifactType::Spellbook | ArtifactType::HistoryTome | ArtifactType::ProphecyScroll |
            ArtifactType::Codex | ArtifactType::Treatise | ArtifactType::PhilosophyTome => ArtifactCategory::Tome,

            ArtifactType::Gem | ArtifactType::GoldStatuette | ArtifactType::Goblet |
            ArtifactType::Idol => ArtifactCategory::Treasure,

            ArtifactType::Harp | ArtifactType::Horn | ArtifactType::Drum => ArtifactCategory::Instrument,
        }
    }
}

/// Rarity of an artifact
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ArtifactRarity {
    Common,     // 60%
    Uncommon,   // 25%
    Rare,       // 10%
    Epic,       // 4%
    Legendary,  // 1%
}

impl ArtifactRarity {
    pub fn name(&self) -> &'static str {
        match self {
            ArtifactRarity::Common => "Common",
            ArtifactRarity::Uncommon => "Uncommon",
            ArtifactRarity::Rare => "Rare",
            ArtifactRarity::Epic => "Epic",
            ArtifactRarity::Legendary => "Legendary",
        }
    }

    /// Get weighted probability for generation
    pub fn weight(&self) -> u32 {
        match self {
            ArtifactRarity::Common => 60,
            ArtifactRarity::Uncommon => 25,
            ArtifactRarity::Rare => 10,
            ArtifactRarity::Epic => 4,
            ArtifactRarity::Legendary => 1,
        }
    }
}

/// An event in the artifact's history
#[derive(Clone, Debug)]
pub struct ArtifactEvent {
    pub year: Year,
    pub event_type: ArtifactEventType,
    pub location: Option<(usize, usize, i32)>,
    pub person: Option<HeroId>,
    pub description: String,
}

/// Type of artifact event
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtifactEventType {
    Created,
    Gifted,
    Inherited,
    Won,
    Stolen,
    Lost,
    Found,
    Captured,
    Destroyed,
    Hidden,
    Enshrined,
}

impl ArtifactEventType {
    pub fn name(&self) -> &'static str {
        match self {
            ArtifactEventType::Created => "Created",
            ArtifactEventType::Gifted => "Gifted",
            ArtifactEventType::Inherited => "Inherited",
            ArtifactEventType::Won => "Won in battle",
            ArtifactEventType::Stolen => "Stolen",
            ArtifactEventType::Lost => "Lost",
            ArtifactEventType::Found => "Found",
            ArtifactEventType::Captured => "Captured by monster",
            ArtifactEventType::Destroyed => "Destroyed",
            ArtifactEventType::Hidden => "Hidden",
            ArtifactEventType::Enshrined => "Enshrined",
        }
    }
}

/// Subject of a philosophy
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PhilosophySubject {
    Ethics,
    Metaphysics,
    Politics,
    War,
    Nature,
    Magic,
    Religion,
    History,
}

impl PhilosophySubject {
    pub fn name(&self) -> &'static str {
        match self {
            PhilosophySubject::Ethics => "Ethics",
            PhilosophySubject::Metaphysics => "Metaphysics",
            PhilosophySubject::Politics => "Politics",
            PhilosophySubject::War => "War",
            PhilosophySubject::Nature => "Nature",
            PhilosophySubject::Magic => "Magic",
            PhilosophySubject::Religion => "Religion",
            PhilosophySubject::History => "History",
        }
    }
}

/// The lore/information contained in an artifact
#[derive(Clone, Debug)]
pub enum ArtifactLore {
    /// No special lore
    None,

    /// Contains a philosophy/teaching (books, scrolls)
    Philosophy {
        author: HeroId,
        teaching: String,
        subject: PhilosophySubject,
    },

    /// Contains religious doctrine (relics, holy symbols)
    Religion {
        faith: String,
        tenets: Vec<String>,
    },

    /// Contains military history (weapons, armor)
    BattleHistory {
        battles_fought: Vec<EventId>,
        kills: u32,
        defeats: u32,
    },

    /// Contains historical records (tomes, codices)
    HistoricalRecord {
        era: String,
        events_recorded: Vec<EventId>,
    },

    /// Contains magical knowledge (spellbooks)
    MagicalKnowledge {
        school: String,
        spells: Vec<String>,
    },

    /// Contains prophecy (scrolls)
    Prophecy {
        prediction: String,
        fulfilled: bool,
    },

    /// Contains map/location info
    LocationKnowledge {
        known_locations: Vec<(usize, usize, String)>,
    },
}

impl ArtifactLore {
    /// Get a short description of the lore
    pub fn summary(&self) -> Option<String> {
        match self {
            ArtifactLore::None => None,
            ArtifactLore::Philosophy { teaching, subject, .. } => {
                Some(format!("{}: \"{}\"", subject.name(), truncate(teaching, 50)))
            }
            ArtifactLore::Religion { faith, .. } => {
                Some(format!("Faith of {}", faith))
            }
            ArtifactLore::BattleHistory { battles_fought, kills, .. } => {
                Some(format!("{} battles, {} kills", battles_fought.len(), kills))
            }
            ArtifactLore::HistoricalRecord { era, events_recorded } => {
                Some(format!("Records of {} ({} events)", era, events_recorded.len()))
            }
            ArtifactLore::MagicalKnowledge { school, spells } => {
                Some(format!("{} magic ({} spells)", school, spells.len()))
            }
            ArtifactLore::Prophecy { prediction, fulfilled } => {
                let status = if *fulfilled { "fulfilled" } else { "unfulfilled" };
                Some(format!("Prophecy ({}): \"{}\"", status, truncate(prediction, 40)))
            }
            ArtifactLore::LocationKnowledge { known_locations } => {
                Some(format!("Map of {} locations", known_locations.len()))
            }
        }
    }
}

/// Where the artifact currently is
#[derive(Clone, Debug)]
pub enum ArtifactLocation {
    /// Carried by a hero
    WithHero(HeroId),
    /// In a settlement
    InSettlement { settlement: SettlementId, building: String },
    /// In a dungeon/cave
    InDungeon { x: usize, y: usize, z: i32, dungeon_name: String },
    /// In a monster lair
    InMonsterLair { lair: LairId, monster_name: String },
    /// At a shrine/temple
    AtShrine { x: usize, y: usize, z: i32 },
    /// Lost at a battlefield
    AtBattlefield { x: usize, y: usize, battle_name: String },
    /// Hidden location
    Hidden { x: usize, y: usize, z: i32 },
    /// Buried with owner
    InTomb { x: usize, y: usize, z: i32, buried_with: HeroId },
    /// Destroyed
    Destroyed,
}

impl ArtifactLocation {
    /// Get the coordinates if the artifact has a known location
    pub fn coordinates(&self) -> Option<(usize, usize, i32)> {
        match self {
            ArtifactLocation::InDungeon { x, y, z, .. } => Some((*x, *y, *z)),
            ArtifactLocation::AtShrine { x, y, z } => Some((*x, *y, *z)),
            ArtifactLocation::AtBattlefield { x, y, .. } => Some((*x, *y, 0)),
            ArtifactLocation::Hidden { x, y, z } => Some((*x, *y, *z)),
            ArtifactLocation::InTomb { x, y, z, .. } => Some((*x, *y, *z)),
            _ => None,
        }
    }

    /// Get a description of the location
    pub fn description(&self) -> String {
        match self {
            ArtifactLocation::WithHero(id) => format!("Carried by {}", id),
            ArtifactLocation::InSettlement { building, .. } => format!("In {} of a settlement", building),
            ArtifactLocation::InDungeon { dungeon_name, x, y, z, .. } => {
                format!("{} ({}, {}, z={})", dungeon_name, x, y, z)
            }
            ArtifactLocation::InMonsterLair { monster_name, .. } => format!("{}'s Hoard", monster_name),
            ArtifactLocation::AtShrine { x, y, z } => format!("Shrine at ({}, {}, z={})", x, y, z),
            ArtifactLocation::AtBattlefield { battle_name, .. } => format!("Lost at {}", battle_name),
            ArtifactLocation::Hidden { x, y, z } => format!("Hidden at ({}, {}, z={})", x, y, z),
            ArtifactLocation::InTomb { x, y, z, .. } => format!("Tomb at ({}, {}, z={})", x, y, z),
            ArtifactLocation::Destroyed => "Destroyed".to_string(),
        }
    }
}

/// Main artifact structure - a lore carrier
#[derive(Clone, Debug)]
pub struct Artifact {
    pub id: ArtifactId,
    pub name: String,
    pub artifact_type: ArtifactType,
    pub category: ArtifactCategory,
    pub rarity: ArtifactRarity,

    // Creation
    pub creator: Option<HeroId>,
    pub created_for: Option<HeroId>,
    pub creation_year: Year,
    pub faction_origin: FactionId,

    // Lore content
    pub contained_lore: ArtifactLore,

    // Ownership history
    pub history: Vec<ArtifactEvent>,

    // Current state
    pub current_owner: Option<HeroId>,
    pub current_location: ArtifactLocation,
    pub is_destroyed: bool,

    // Flavor
    pub description: String,
    pub powers: Vec<String>,
}

impl Artifact {
    /// Get a summary of this artifact
    pub fn summary(&self) -> String {
        format!("{} ({} {})", self.name, self.rarity.name(), self.artifact_type.name())
    }

    /// Get the number of owners this artifact has had
    pub fn owner_count(&self) -> usize {
        self.history.iter()
            .filter(|e| matches!(e.event_type,
                ArtifactEventType::Created |
                ArtifactEventType::Gifted |
                ArtifactEventType::Inherited |
                ArtifactEventType::Won |
                ArtifactEventType::Found
            ))
            .count()
    }
}

/// Registry of all artifacts
#[derive(Clone, Debug, Default)]
pub struct ArtifactRegistry {
    pub artifacts: HashMap<ArtifactId, Artifact>,
    pub artifacts_by_location: HashMap<(usize, usize, i32), Vec<ArtifactId>>,
    pub artifacts_by_lair: HashMap<LairId, Vec<ArtifactId>>,
    pub artifacts_by_dungeon: HashMap<DungeonId, Vec<ArtifactId>>,
    pub artifacts_by_hero: HashMap<HeroId, Vec<ArtifactId>>,
    next_id: u32,
}

impl ArtifactRegistry {
    pub fn new() -> Self {
        Self {
            artifacts: HashMap::new(),
            artifacts_by_location: HashMap::new(),
            artifacts_by_lair: HashMap::new(),
            artifacts_by_dungeon: HashMap::new(),
            artifacts_by_hero: HashMap::new(),
            next_id: 0,
        }
    }

    /// Add an artifact to the registry
    pub fn add(&mut self, artifact: Artifact) {
        let id = artifact.id;

        // Index by location if available
        if let Some(coords) = artifact.current_location.coordinates() {
            self.artifacts_by_location.entry(coords).or_default().push(id);
        }

        // Index by lair if in monster lair
        if let ArtifactLocation::InMonsterLair { lair, .. } = artifact.current_location {
            self.artifacts_by_lair.entry(lair).or_default().push(id);
        }

        // Index by hero if carried
        if let Some(hero_id) = artifact.current_owner {
            self.artifacts_by_hero.entry(hero_id).or_default().push(id);
        }

        self.artifacts.insert(id, artifact);
    }

    /// Get an artifact by ID
    pub fn get(&self, id: ArtifactId) -> Option<&Artifact> {
        self.artifacts.get(&id)
    }

    /// Get a mutable reference to an artifact by ID
    pub fn get_mut(&mut self, id: ArtifactId) -> Option<&mut Artifact> {
        self.artifacts.get_mut(&id)
    }

    /// Generate a new artifact ID
    pub fn new_id(&mut self) -> ArtifactId {
        let id = ArtifactId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Get artifacts at a specific location
    pub fn artifacts_at(&self, x: usize, y: usize, z: i32) -> Vec<&Artifact> {
        self.artifacts_by_location.get(&(x, y, z))
            .map(|ids| ids.iter().filter_map(|id| self.artifacts.get(id)).collect())
            .unwrap_or_default()
    }

    /// Get artifacts in a monster lair
    pub fn artifacts_in_lair(&self, lair: LairId) -> Vec<&Artifact> {
        self.artifacts_by_lair.get(&lair)
            .map(|ids| ids.iter().filter_map(|id| self.artifacts.get(id)).collect())
            .unwrap_or_default()
    }

    /// Get artifacts by rarity
    pub fn artifacts_by_rarity(&self, rarity: ArtifactRarity) -> Vec<&Artifact> {
        self.artifacts.values()
            .filter(|a| a.rarity == rarity)
            .collect()
    }

    /// Get all artifacts
    pub fn all(&self) -> impl Iterator<Item = &Artifact> {
        self.artifacts.values()
    }

    /// Get legendary artifacts
    pub fn legendary_artifacts(&self) -> Vec<&Artifact> {
        self.artifacts_by_rarity(ArtifactRarity::Legendary)
    }
}

/// Generate artifacts for the world
pub fn generate_artifacts(
    factions: &FactionRegistry,
    heroes: &HeroRegistry,
    monsters: &MonsterRegistry,
    seed: u64,
) -> ArtifactRegistry {
    let mut rng = ChaCha8Rng::seed_from_u64(seed.wrapping_add(0xAB71FAC7));
    let name_gen = NameGenerator::new(seed);
    let mut registry = ArtifactRegistry::new();

    println!("  Generating artifacts...");

    // Generate artifacts created by heroes
    for hero in heroes.all() {
        // Craftsmen create more artifacts
        let num_artifacts = match hero.role {
            HeroRole::Craftsman => rng.gen_range(2..=5),
            HeroRole::Scholar => rng.gen_range(1..=3),
            HeroRole::Priest => rng.gen_range(1..=2),
            HeroRole::Ruler => rng.gen_range(0..=2),
            _ => if rng.gen_bool(0.3) { 1 } else { 0 },
        };

        for _ in 0..num_artifacts {
            let artifact = create_artifact_by_hero(
                &mut registry,
                hero,
                factions,
                heroes,
                &name_gen,
                &mut rng,
            );
            registry.add(artifact);
        }
    }

    // Generate some artifacts not tied to specific heroes
    let num_unclaimed = rng.gen_range(10..30);
    for _ in 0..num_unclaimed {
        if let Some(faction) = factions.all().next() {
            let artifact = create_unclaimed_artifact(
                &mut registry,
                faction.id,
                faction.species,
                &name_gen,
                &mut rng,
            );
            registry.add(artifact);
        }
    }

    // Simulate artifact history (movement through time)
    simulate_artifact_histories(&mut registry, heroes, monsters, &mut rng);

    // Link artifacts to heroes
    link_artifacts_to_heroes(&mut registry, heroes);

    println!("    {} artifacts generated", registry.artifacts.len());
    registry
}

/// Create an artifact made by a hero
fn create_artifact_by_hero(
    registry: &mut ArtifactRegistry,
    hero: &Hero,
    factions: &FactionRegistry,
    heroes: &HeroRegistry,
    name_gen: &NameGenerator,
    rng: &mut ChaCha8Rng,
) -> Artifact {
    let id = registry.new_id();
    let rarity = pick_rarity(hero.fame, rng);

    // Artifact type based on hero role
    let artifact_type = pick_artifact_type_for_role(hero.role, rng);
    let category = artifact_type.category();

    // Generate name
    let name = generate_artifact_name(artifact_type, hero.species, &name_gen, rng);

    // Creation year (during hero's lifetime)
    let creation_years_ago = rng.gen_range(
        hero.death_year.map(|d| d.age()).unwrap_or(0)..=hero.birth_year.age()
    );
    let creation_year = Year::years_ago(creation_years_ago);

    // Find a recipient if appropriate
    let created_for = if rng.gen_bool(0.5) {
        heroes.heroes_of_faction(hero.faction)
            .iter()
            .filter(|h| h.id != hero.id && h.alive_at(creation_year))
            .next()
            .map(|h| h.id)
    } else {
        None
    };

    // Generate lore content
    let contained_lore = generate_lore_for_artifact(artifact_type, hero, rng);

    // Initial history event
    let creation_event = ArtifactEvent {
        year: creation_year,
        event_type: ArtifactEventType::Created,
        location: None,
        person: Some(hero.id),
        description: format!("Created by {}", hero.full_name()),
    };

    // Generate description and powers (use hero's homeland biome if available)
    let description = generate_artifact_description_biome(artifact_type, rarity, hero.species, hero.homeland_biome, rng);
    let powers = generate_artifact_powers(artifact_type, rarity, rng);

    Artifact {
        id,
        name,
        artifact_type,
        category,
        rarity,
        creator: Some(hero.id),
        created_for,
        creation_year,
        faction_origin: hero.faction,
        contained_lore,
        history: vec![creation_event],
        current_owner: created_for.or(Some(hero.id)),
        current_location: ArtifactLocation::WithHero(created_for.unwrap_or(hero.id)),
        is_destroyed: false,
        description,
        powers,
    }
}

/// Create an artifact not tied to a specific hero
fn create_unclaimed_artifact(
    registry: &mut ArtifactRegistry,
    faction: FactionId,
    species: Species,
    name_gen: &NameGenerator,
    rng: &mut ChaCha8Rng,
) -> Artifact {
    let id = registry.new_id();
    let rarity = pick_rarity(50, rng); // Average fame for unclaimed

    let artifact_type = pick_random_artifact_type(rng);
    let category = artifact_type.category();

    let name = generate_artifact_name(artifact_type, species, name_gen, rng);
    let creation_year = Year::years_ago(rng.gen_range(100..1000));

    let creation_event = ArtifactEvent {
        year: creation_year,
        event_type: ArtifactEventType::Created,
        location: None,
        person: None,
        description: "Created by unknown artisans".to_string(),
    };

    let description = generate_artifact_description(artifact_type, rarity, species, rng);
    let powers = generate_artifact_powers(artifact_type, rarity, rng);

    // Random initial location
    let x = rng.gen_range(0..512);
    let y = rng.gen_range(0..256);
    let z = rng.gen_range(-5..=0);

    Artifact {
        id,
        name,
        artifact_type,
        category,
        rarity,
        creator: None,
        created_for: None,
        creation_year,
        faction_origin: faction,
        contained_lore: ArtifactLore::None,
        history: vec![creation_event],
        current_owner: None,
        current_location: ArtifactLocation::Hidden { x, y, z },
        is_destroyed: false,
        description,
        powers,
    }
}

/// Pick a rarity based on creator fame
fn pick_rarity(fame: u32, rng: &mut ChaCha8Rng) -> ArtifactRarity {
    // Higher fame increases chance of better rarity
    let fame_bonus = fame as f32 / 100.0;
    let roll = rng.gen::<f32>() - fame_bonus * 0.2;

    if roll < 0.01 {
        ArtifactRarity::Legendary
    } else if roll < 0.05 {
        ArtifactRarity::Epic
    } else if roll < 0.15 {
        ArtifactRarity::Rare
    } else if roll < 0.40 {
        ArtifactRarity::Uncommon
    } else {
        ArtifactRarity::Common
    }
}

/// Pick artifact type based on hero role
fn pick_artifact_type_for_role(role: HeroRole, rng: &mut ChaCha8Rng) -> ArtifactType {
    let types: &[ArtifactType] = match role {
        HeroRole::Warrior | HeroRole::General => &[
            ArtifactType::Sword, ArtifactType::Axe, ArtifactType::Hammer,
            ArtifactType::Shield, ArtifactType::Helm, ArtifactType::Breastplate,
        ],
        HeroRole::Scholar => &[
            ArtifactType::PhilosophyTome, ArtifactType::HistoryTome, ArtifactType::Codex,
            ArtifactType::Treatise, ArtifactType::Spellbook, ArtifactType::Staff,
        ],
        HeroRole::Priest => &[
            ArtifactType::HolySymbol, ArtifactType::Chalice, ArtifactType::Urn,
            ArtifactType::Staff, ArtifactType::Ring, ArtifactType::Amulet,
        ],
        HeroRole::Craftsman => &[
            ArtifactType::Hammer, ArtifactType::Ring, ArtifactType::Crown,
            ArtifactType::GoldStatuette, ArtifactType::Gem, ArtifactType::Goblet,
        ],
        HeroRole::Ruler => &[
            ArtifactType::Crown, ArtifactType::Scepter, ArtifactType::Ring,
            ArtifactType::Sword, ArtifactType::Cloak, ArtifactType::Circlet,
        ],
        HeroRole::Explorer => &[
            ArtifactType::Bow, ArtifactType::Cloak, ArtifactType::Ring,
            ArtifactType::Amulet, ArtifactType::Dagger, ArtifactType::Horn,
        ],
        HeroRole::Villain => &[
            ArtifactType::Dagger, ArtifactType::Ring, ArtifactType::Amulet,
            ArtifactType::Orb, ArtifactType::Staff, ArtifactType::Bone,
        ],
    };

    types[rng.gen_range(0..types.len())]
}

/// Pick a random artifact type
fn pick_random_artifact_type(rng: &mut ChaCha8Rng) -> ArtifactType {
    let types = [
        ArtifactType::Sword, ArtifactType::Axe, ArtifactType::Hammer,
        ArtifactType::Ring, ArtifactType::Amulet, ArtifactType::Crown,
        ArtifactType::Gem, ArtifactType::Goblet, ArtifactType::Shield,
    ];
    types[rng.gen_range(0..types.len())]
}

/// Generate an artifact name
fn generate_artifact_name(
    artifact_type: ArtifactType,
    species: Species,
    name_gen: &NameGenerator,
    rng: &mut ChaCha8Rng,
) -> String {
    let prefixes: &[&str] = match species {
        Species::Dwarf => &["Iron", "Stone", "Gold", "Deep", "Mountain", "Forge"],
        Species::Elf => &["Star", "Moon", "Silver", "Dawn", "Twilight", "Eternal"],
        Species::Human => &["Royal", "Sacred", "Ancient", "Blessed", "Noble", "Grand"],
        Species::Orc => &["Blood", "War", "Skull", "Rage", "Doom", "Dread"],
        _ => &["Ancient", "Mystic", "Primal", "Shadow", "Flame", "Storm"],
    };

    let suffixes: &[&str] = match artifact_type.category() {
        ArtifactCategory::Weapon => &["Slayer", "Bane", "Fury", "Edge", "Wrath"],
        ArtifactCategory::Armor => &["Guardian", "Protector", "Shield", "Bulwark", "Ward"],
        ArtifactCategory::Jewelry => &["Light", "Binding", "Power", "Glory", "Destiny"],
        ArtifactCategory::Relic => &["Faith", "Blessing", "Sanctity", "Grace", "Divinity"],
        ArtifactCategory::Tome => &["Wisdom", "Knowledge", "Truth", "Secrets", "Lore"],
        ArtifactCategory::Treasure => &["Wealth", "Fortune", "Splendor", "Majesty", "Opulence"],
        ArtifactCategory::Instrument => &["Song", "Harmony", "Voice", "Echo", "Resonance"],
    };

    let prefix = prefixes[rng.gen_range(0..prefixes.len())];
    let suffix = suffixes[rng.gen_range(0..suffixes.len())];

    format!("{}{}", prefix, suffix)
}

/// Generate lore content based on artifact type and creator
fn generate_lore_for_artifact(
    artifact_type: ArtifactType,
    creator: &Hero,
    rng: &mut ChaCha8Rng,
) -> ArtifactLore {
    match artifact_type.category() {
        ArtifactCategory::Tome => {
            if let Some(ref philosophy) = creator.philosophy {
                let subjects = [
                    PhilosophySubject::Ethics, PhilosophySubject::Metaphysics,
                    PhilosophySubject::Politics, PhilosophySubject::War,
                    PhilosophySubject::Nature, PhilosophySubject::Magic,
                ];
                ArtifactLore::Philosophy {
                    author: creator.id,
                    teaching: philosophy.clone(),
                    subject: subjects[rng.gen_range(0..subjects.len())],
                }
            } else {
                ArtifactLore::None
            }
        }
        ArtifactCategory::Relic => {
            if let Some(ref beliefs) = creator.religious_beliefs {
                ArtifactLore::Religion {
                    faith: format!("Faith of {}", creator.full_name()),
                    tenets: vec![beliefs.clone()],
                }
            } else {
                ArtifactLore::None
            }
        }
        ArtifactCategory::Weapon | ArtifactCategory::Armor => {
            if creator.role == HeroRole::Warrior || creator.role == HeroRole::General {
                ArtifactLore::BattleHistory {
                    battles_fought: creator.achievements.clone(),
                    kills: rng.gen_range(10..100),
                    defeats: rng.gen_range(0..5),
                }
            } else {
                ArtifactLore::None
            }
        }
        _ => ArtifactLore::None,
    }
}

/// Generate artifact description
fn generate_artifact_description(
    artifact_type: ArtifactType,
    rarity: ArtifactRarity,
    species: Species,
    rng: &mut ChaCha8Rng,
) -> String {
    generate_artifact_description_biome(artifact_type, rarity, species, None, rng)
}

/// Generate artifact description with biome-aware materials
fn generate_artifact_description_biome(
    artifact_type: ArtifactType,
    rarity: ArtifactRarity,
    species: Species,
    biome_category: Option<BiomeCategory>,
    rng: &mut ChaCha8Rng,
) -> String {
    // If we have biome data, use biome-specific materials (60% chance)
    let material = if let Some(biome) = biome_category {
        if rng.gen_bool(0.6) {
            biome_material(biome, rng)
        } else {
            species_material(species, rng)
        }
    } else {
        species_material(species, rng)
    };

    let quality = match rarity {
        ArtifactRarity::Legendary => "legendary",
        ArtifactRarity::Epic => "magnificent",
        ArtifactRarity::Rare => "exceptional",
        ArtifactRarity::Uncommon => "fine",
        ArtifactRarity::Common => "well-crafted",
    };

    // Add biome flavor to the description
    let flavor = if let Some(biome) = biome_category {
        match biome {
            BiomeCategory::Volcanic => ", forged in volcanic heat",
            BiomeCategory::Tundra => ", tempered by glacial cold",
            BiomeCategory::Desert => ", sun-baked in desert sands",
            BiomeCategory::Swamp => ", preserved in ancient peat",
            BiomeCategory::Forest => ", shaped by forest magic",
            BiomeCategory::Mountain => ", carved from mountain stone",
            BiomeCategory::Coastal => ", blessed by sea winds",
            BiomeCategory::Cave => ", found in the deep darkness",
            BiomeCategory::Mystical => ", touched by arcane forces",
            _ => "",
        }
    } else {
        ""
    };

    format!("A {} {} {}{}, crafted with great skill.", quality, material, artifact_type.name().to_lowercase(), flavor)
}

/// Get material based on species
fn species_material(species: Species, rng: &mut ChaCha8Rng) -> String {
    let material = match species {
        Species::Dwarf => pick(rng, &["mithril", "adamantine", "deep iron", "rune-etched steel"]),
        Species::Elf => pick(rng, &["starsilver", "moonstone", "living wood", "crystal"]),
        Species::Human => pick(rng, &["fine steel", "blessed silver", "gilded bronze", "tempered iron"]),
        Species::Orc => pick(rng, &["black iron", "bone", "blood-forged steel", "obsidian"]),
        Species::Goblin => pick(rng, &["scrap iron", "salvaged steel", "tarnished bronze", "crude alloy"]),
        Species::Giant => pick(rng, &["titan-forged iron", "mountain bronze", "storm-touched steel", "ancient stone"]),
        Species::DragonKin => pick(rng, &["dragon-scale", "fire-gold", "ember-steel", "molten bronze"]),
        Species::Undead => pick(rng, &["grave-iron", "bone-white steel", "corpse-cold silver", "death-touched bronze"]),
        Species::Elemental => pick(rng, &["primal essence", "elemental core", "spirit-bound metal", "raw mana"]),
    };
    material.to_string()
}

/// Get material based on biome
fn biome_material(biome: BiomeCategory, rng: &mut ChaCha8Rng) -> String {
    let material = match biome {
        BiomeCategory::Volcanic => pick(rng, &["obsidian", "basalt", "magma-steel", "ash-iron", "fire-opal", "pumice"]),
        BiomeCategory::Tundra => pick(rng, &["frost-crystal", "glacial ice", "winter-steel", "frozen silver", "ice-iron"]),
        BiomeCategory::Desert => pick(rng, &["sun-bronze", "desert glass", "sand-gold", "sun-steel", "amber"]),
        BiomeCategory::Swamp => pick(rng, &["bog-iron", "petrified wood", "swamp-copper", "rot-silver", "peat-bronze"]),
        BiomeCategory::Forest => pick(rng, &["living wood", "amber", "greenwood", "ironbark", "heartwood", "leaf-silver"]),
        BiomeCategory::Mountain => pick(rng, &["mithril", "adamantine", "sky-iron", "mountain-silver", "granite", "deep-steel"]),
        BiomeCategory::Coastal => pick(rng, &["sea-steel", "pearl", "coral", "drift-silver", "salt-crystal", "wave-glass"]),
        BiomeCategory::Cave => pick(rng, &["deep-crystal", "shadow-steel", "cave-silver", "dark-iron", "blind-stone"]),
        BiomeCategory::Hills => pick(rng, &["barrow-bronze", "hill-iron", "copper", "heather-stone", "tumbled-silver"]),
        BiomeCategory::Grassland => pick(rng, &["plains-bronze", "wind-steel", "grass-copper", "golden-iron"]),
        BiomeCategory::Ruins => pick(rng, &["cursed iron", "grave-silver", "tomb-gold", "shadow-steel", "bone"]),
        BiomeCategory::Ocean => pick(rng, &["abyssal-steel", "deep-coral", "pressure-iron", "brine-silver"]),
        BiomeCategory::Mystical => pick(rng, &["star-metal", "void-silver", "dream-crystal", "ether-steel", "spirit-glass"]),
    };
    material.to_string()
}

/// Generate artifact powers
fn generate_artifact_powers(
    artifact_type: ArtifactType,
    rarity: ArtifactRarity,
    rng: &mut ChaCha8Rng,
) -> Vec<String> {
    let num_powers = match rarity {
        ArtifactRarity::Legendary => rng.gen_range(2..=4),
        ArtifactRarity::Epic => rng.gen_range(1..=3),
        ArtifactRarity::Rare => rng.gen_range(1..=2),
        ArtifactRarity::Uncommon => rng.gen_range(0..=1),
        ArtifactRarity::Common => 0,
    };

    let power_pool: &[&str] = match artifact_type.category() {
        ArtifactCategory::Weapon => &[
            "Deals extra damage to undead",
            "Flames on command",
            "Never dulls",
            "Returns when thrown",
            "Drains life from enemies",
            "Cuts through armor",
        ],
        ArtifactCategory::Armor => &[
            "Protects against fire",
            "Deflects arrows",
            "Grants strength to the wearer",
            "Repairs itself",
            "Turns invisible in darkness",
            "Resists magic",
        ],
        ArtifactCategory::Jewelry => &[
            "Grants wisdom",
            "Detects lies",
            "Allows flight",
            "Protects the mind",
            "Enhances charisma",
            "Grants longevity",
        ],
        ArtifactCategory::Relic => &[
            "Heals wounds",
            "Wards off evil",
            "Grants visions",
            "Purifies corruption",
            "Speaks the will of the gods",
            "Protects the faithful",
        ],
        ArtifactCategory::Tome => &[
            "Teaches forgotten spells",
            "Contains prophecies",
            "Reveals hidden knowledge",
            "Translates any language",
            "Records new information",
            "Answers questions",
        ],
        _ => &[
            "Glows in darkness",
            "Never breaks",
            "Attracts fortune",
            "Speaks to the wielder",
            "Grants strange dreams",
        ],
    };

    (0..num_powers)
        .filter_map(|_| {
            let power = power_pool[rng.gen_range(0..power_pool.len())];
            Some(power.to_string())
        })
        .collect()
}

/// Simulate artifact movement through history
fn simulate_artifact_histories(
    registry: &mut ArtifactRegistry,
    heroes: &HeroRegistry,
    monsters: &MonsterRegistry,
    rng: &mut ChaCha8Rng,
) {
    let artifact_ids: Vec<ArtifactId> = registry.artifacts.keys().copied().collect();

    for artifact_id in artifact_ids {
        let artifact = registry.artifacts.get(&artifact_id).unwrap();
        let creation_year = artifact.creation_year;
        let mut current_year = creation_year;
        let mut current_owner = artifact.current_owner;
        let mut history = artifact.history.clone();
        let mut location = artifact.current_location.clone();
        let mut is_destroyed = false;

        // Simulate through time in decade increments
        while current_year.age() > 0 && !is_destroyed {
            let years_passed = 10.min(current_year.age());
            current_year = Year::years_ago(current_year.age() - years_passed);

            // Check if current owner died
            if let Some(owner_id) = current_owner {
                if let Some(owner) = heroes.get(owner_id) {
                    if let Some(death_year) = owner.death_year {
                        if current_year >= death_year {
                            // Owner died - what happens to artifact?
                            let fate = rng.gen_range(0..100);
                            let event = if fate < 50 {
                                // Inherited
                                let new_owner = heroes.heroes_of_faction(owner.faction)
                                    .iter()
                                    .filter(|h| h.alive_at(current_year))
                                    .next()
                                    .map(|h| h.id);

                                current_owner = new_owner;
                                if let Some(new_id) = new_owner {
                                    location = ArtifactLocation::WithHero(new_id);
                                }

                                ArtifactEvent {
                                    year: death_year,
                                    event_type: ArtifactEventType::Inherited,
                                    location: None,
                                    person: new_owner,
                                    description: format!("Inherited after death of {}", owner.full_name()),
                                }
                            } else if fate < 70 {
                                // Buried with owner
                                current_owner = None;
                                let (x, y, z) = owner.burial_site.unwrap_or((
                                    rng.gen_range(0..512),
                                    rng.gen_range(0..256),
                                    rng.gen_range(-5..0),
                                ));
                                location = ArtifactLocation::InTomb { x, y, z, buried_with: owner_id };

                                ArtifactEvent {
                                    year: death_year,
                                    event_type: ArtifactEventType::Enshrined,
                                    location: Some((x, y, z)),
                                    person: Some(owner_id),
                                    description: format!("Buried with {}", owner.full_name()),
                                }
                            } else {
                                // Lost
                                current_owner = None;
                                let x = rng.gen_range(0..512);
                                let y = rng.gen_range(0..256);
                                location = ArtifactLocation::AtBattlefield {
                                    x, y,
                                    battle_name: "unknown battle".to_string(),
                                };

                                ArtifactEvent {
                                    year: death_year,
                                    event_type: ArtifactEventType::Lost,
                                    location: Some((x, y, 0)),
                                    person: None,
                                    description: format!("Lost when {} fell", owner.full_name()),
                                }
                            };

                            history.push(event);
                        }
                    }
                }
            }

            // Random events for unowned artifacts
            if current_owner.is_none() && rng.gen_bool(0.05) {
                let event_type = rng.gen_range(0..100);

                if event_type < 30 {
                    // Found by explorer
                    let finder = heroes.heroes_by_role(HeroRole::Explorer)
                        .into_iter()
                        .filter(|h| h.alive_at(current_year))
                        .next();

                    if let Some(hero) = finder {
                        current_owner = Some(hero.id);
                        location = ArtifactLocation::WithHero(hero.id);
                        history.push(ArtifactEvent {
                            year: current_year,
                            event_type: ArtifactEventType::Found,
                            location: None,
                            person: Some(hero.id),
                            description: format!("Found by {}", hero.full_name()),
                        });
                    }
                } else if event_type < 50 {
                    // Captured by monster
                    if let Some(lair) = monsters.lairs.values().next() {
                        location = ArtifactLocation::InMonsterLair {
                            lair: lair.id,
                            monster_name: lair.species.name().to_string(),
                        };
                        history.push(ArtifactEvent {
                            year: current_year,
                            event_type: ArtifactEventType::Captured,
                            location: Some((lair.x, lair.y, lair.z)),
                            person: None,
                            description: format!("Captured by {}", lair.name),
                        });
                    }
                }
            }
        }

        // Update the artifact
        if let Some(artifact) = registry.artifacts.get_mut(&artifact_id) {
            artifact.history = history;
            artifact.current_owner = current_owner;
            artifact.current_location = location;
            artifact.is_destroyed = is_destroyed;
        }
    }
}

/// Link artifacts back to their creator heroes
fn link_artifacts_to_heroes(registry: &mut ArtifactRegistry, heroes: &HeroRegistry) {
    // This is called after artifact generation to update hero records
    // Note: We'd need mutable access to heroes, which we don't have here
    // Instead, this info is tracked via artifacts_by_hero in the registry
}

/// Helper to pick a random element
fn pick<'a>(rng: &mut ChaCha8Rng, options: &[&'a str]) -> &'a str {
    options[rng.gen_range(0..options.len())]
}

/// Helper to truncate a string
fn truncate(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len {
        s
    } else {
        &s[..max_len.min(s.len())]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_artifact_generation() {
        use super::super::factions::{Faction, FactionRegistry};
        use super::super::heroes::generate_heroes;
        use super::super::timeline::Timeline;

        let mut factions = FactionRegistry::new();
        let faction = Faction {
            id: factions.new_id(),
            name: "Test Kingdom".to_string(),
            species: Species::Human,
            culture: CultureType::Militaristic,
            architecture: ArchitectureStyle::Imperial,
            founded: Year::years_ago(500),
            collapsed: None,
            capital: None,
            relations: HashMap::new(),
            population_peak: 10000,
        };
        factions.add(faction);

        let timeline = Timeline::new();
        let heroes = generate_heroes(&factions, &timeline, 42);
        let monsters = MonsterRegistry::new();

        let artifacts = generate_artifacts(&factions, &heroes, &monsters, 42);

        assert!(!artifacts.artifacts.is_empty(), "Should have generated artifacts");

        for artifact in artifacts.all() {
            println!("{}", artifact.summary());
            if let Some(lore) = artifact.contained_lore.summary() {
                println!("  Lore: {}", lore);
            }
            println!("  Location: {}", artifact.current_location.description());
            println!("  History: {} events", artifact.history.len());
        }
    }
}
