//! Core types for the lore generation system
//!
//! Defines wanderers, cultural lenses, story seeds, landmarks, and encounters.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::biomes::ExtendedBiome;
use crate::plates::PlateId;
use crate::water_bodies::WaterBodyType;

/// Unique identifier for landmarks
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LandmarkId(pub u32);

/// Unique identifier for story seeds
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StorySeedId(pub u32);

/// Direction on the map (for orientation and movement)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Direction {
    North,
    Northeast,
    East,
    Southeast,
    South,
    Southwest,
    West,
    Northwest,
}

impl Direction {
    /// Get the offset for this direction (dx, dy)
    pub fn offset(&self) -> (i32, i32) {
        match self {
            Direction::North => (0, -1),
            Direction::Northeast => (1, -1),
            Direction::East => (1, 0),
            Direction::Southeast => (1, 1),
            Direction::South => (0, 1),
            Direction::Southwest => (-1, 1),
            Direction::West => (-1, 0),
            Direction::Northwest => (-1, -1),
        }
    }

    /// Get all 8 directions
    pub fn all() -> [Direction; 8] {
        [
            Direction::North,
            Direction::Northeast,
            Direction::East,
            Direction::Southeast,
            Direction::South,
            Direction::Southwest,
            Direction::West,
            Direction::Northwest,
        ]
    }
}

/// World location with all relevant geographic data
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorldLocation {
    pub x: usize,
    pub y: usize,
    pub km_x: f32,
    pub km_y: f32,
    pub elevation: f32,
    pub temperature: f32,
    pub moisture: f32,
    pub biome: String, // Serialized as string for JSON
    pub plate_id: u8,
    pub stress: f32,
    pub water_body_type: String,
}

impl WorldLocation {
    /// Create from tile info
    pub fn from_tile(
        x: usize,
        y: usize,
        km_x: f32,
        km_y: f32,
        elevation: f32,
        temperature: f32,
        moisture: f32,
        biome: ExtendedBiome,
        plate_id: PlateId,
        stress: f32,
        water_body_type: WaterBodyType,
    ) -> Self {
        Self {
            x,
            y,
            km_x,
            km_y,
            elevation,
            temperature,
            moisture,
            biome: format!("{:?}", biome),
            plate_id: plate_id.0,
            stress,
            water_body_type: format!("{:?}", water_body_type),
        }
    }
}

/// Cultural perspective that shapes how a wanderer interprets the world
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CulturalLens {
    /// Mountain peoples - value height, stone, endurance
    Highland {
        sacred_direction: Direction,
        ancestor_worship: bool,
    },
    /// Coastal/island peoples - value water, navigation, tides
    Maritime {
        sea_deity_name: String,
        fears_deep_water: bool,
    },
    /// Desert nomads - value oases, stars, survival
    Desert {
        follows_stars: bool,
        water_sacred: bool,
    },
    /// Forest dwellers - value trees, spirits, cycles
    Sylvan {
        tree_worship: bool,
        fears_open_sky: bool,
    },
    /// Plains peoples - value horizons, herds, wind
    Steppe {
        sky_worship: bool,
        values_movement: bool,
    },
    /// Underground/cave peoples - value depths, minerals, darkness
    Subterranean {
        fears_sunlight: bool,
        crystal_worship: bool,
    },
}

impl CulturalLens {
    /// Get a human-readable culture name
    pub fn culture_name(&self) -> &'static str {
        match self {
            CulturalLens::Highland { .. } => "Highland Folk",
            CulturalLens::Maritime { .. } => "Sea People",
            CulturalLens::Desert { .. } => "Desert Nomads",
            CulturalLens::Sylvan { .. } => "Forest Dwellers",
            CulturalLens::Steppe { .. } => "Steppe Riders",
            CulturalLens::Subterranean { .. } => "Deep Dwellers",
        }
    }

    /// What this culture values (for story generation)
    pub fn values(&self) -> Vec<&'static str> {
        match self {
            CulturalLens::Highland { ancestor_worship, .. } => {
                let mut v = vec!["endurance", "stone", "heights", "cold"];
                if *ancestor_worship {
                    v.push("ancestors");
                }
                v
            }
            CulturalLens::Maritime { .. } => {
                vec!["water", "tides", "navigation", "fish", "salt"]
            }
            CulturalLens::Desert { water_sacred, .. } => {
                let mut v = vec!["stars", "survival", "shade", "heat"];
                if *water_sacred {
                    v.push("oases");
                }
                v
            }
            CulturalLens::Sylvan { tree_worship, .. } => {
                let mut v = vec!["trees", "cycles", "spirits", "growth"];
                if *tree_worship {
                    v.push("ancient trees");
                }
                v
            }
            CulturalLens::Steppe { sky_worship, .. } => {
                let mut v = vec!["horizons", "wind", "herds", "freedom"];
                if *sky_worship {
                    v.push("the open sky");
                }
                v
            }
            CulturalLens::Subterranean { crystal_worship, .. } => {
                let mut v = vec!["depths", "darkness", "minerals", "secrets"];
                if *crystal_worship {
                    v.push("crystals");
                }
                v
            }
        }
    }

    /// What this culture fears or avoids
    pub fn taboos(&self) -> Vec<&'static str> {
        match self {
            CulturalLens::Highland { .. } => vec!["descending too far", "stagnant water"],
            CulturalLens::Maritime { fears_deep_water, .. } => {
                let mut t = vec!["landlocked places", "dust"];
                if *fears_deep_water {
                    t.push("the deep abyss");
                }
                t
            }
            CulturalLens::Desert { .. } => vec!["wasting water", "staying too long"],
            CulturalLens::Sylvan { fears_open_sky, .. } => {
                let mut t = vec!["fire", "axes", "clearings"];
                if *fears_open_sky {
                    t.push("open sky");
                }
                t
            }
            CulturalLens::Steppe { .. } => vec!["walls", "confinement", "roots"],
            CulturalLens::Subterranean { fears_sunlight, .. } => {
                let mut t = vec!["surface dwellers", "bright colors"];
                if *fears_sunlight {
                    t.push("direct sunlight");
                }
                t
            }
        }
    }

    /// Terrain preference multiplier for pathfinding (1.0 = neutral, >1 = attractive, <1 = repulsive)
    pub fn terrain_preference(&self, biome: ExtendedBiome, elevation: f32, is_water: bool) -> f32 {
        let base = match self {
            CulturalLens::Highland { .. } => {
                if elevation > 1500.0 {
                    2.0
                } else if elevation > 500.0 {
                    1.5
                } else if is_water {
                    0.3
                } else {
                    0.8
                }
            }
            CulturalLens::Maritime { fears_deep_water, .. } => {
                if is_water {
                    if *fears_deep_water && elevation < -1000.0 {
                        0.5
                    } else {
                        2.0
                    }
                } else if elevation > 500.0 {
                    0.5
                } else {
                    1.0
                }
            }
            CulturalLens::Desert { .. } => {
                if matches!(
                    biome,
                    ExtendedBiome::Desert | ExtendedBiome::SaltFlats | ExtendedBiome::Oasis
                ) {
                    2.0
                } else if is_water {
                    1.5 // Drawn to water
                } else {
                    0.8
                }
            }
            CulturalLens::Sylvan { fears_open_sky, .. } => {
                if matches!(
                    biome,
                    ExtendedBiome::BorealForest
                        | ExtendedBiome::TemperateForest
                        | ExtendedBiome::TropicalForest
                        | ExtendedBiome::TropicalRainforest
                        | ExtendedBiome::AncientGrove
                        | ExtendedBiome::MushroomForest
                ) {
                    2.0
                } else if *fears_open_sky
                    && matches!(
                        biome,
                        ExtendedBiome::TemperateGrassland
                            | ExtendedBiome::Savanna
                            | ExtendedBiome::Desert
                    )
                {
                    0.3
                } else {
                    0.7
                }
            }
            CulturalLens::Steppe { .. } => {
                if matches!(
                    biome,
                    ExtendedBiome::TemperateGrassland | ExtendedBiome::Savanna | ExtendedBiome::Tundra
                ) {
                    2.0
                } else if matches!(
                    biome,
                    ExtendedBiome::BorealForest
                        | ExtendedBiome::TemperateForest
                        | ExtendedBiome::TropicalRainforest
                ) {
                    0.5
                } else {
                    1.0
                }
            }
            CulturalLens::Subterranean { .. } => {
                if matches!(
                    biome,
                    ExtendedBiome::Sinkhole
                        | ExtendedBiome::CaveEntrance
                        | ExtendedBiome::HollowEarth
                        | ExtendedBiome::Cenote
                ) {
                    3.0
                } else if elevation < 0.0 {
                    1.5
                } else if elevation > 1000.0 {
                    0.5
                } else {
                    0.8
                }
            }
        };

        base
    }
}

/// Emotional tone of an encounter or story
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmotionalTone {
    Awe,
    Fear,
    Wonder,
    Reverence,
    Dread,
    Melancholy,
    Joy,
    Curiosity,
    Unease,
}

impl EmotionalTone {
    /// Convert to LLM prompt guidance
    pub fn to_prompt_guidance(&self) -> &'static str {
        match self {
            EmotionalTone::Awe => "Epic, grandiose, overwhelming beauty or power",
            EmotionalTone::Fear => "Tense, dangerous, survival-focused",
            EmotionalTone::Wonder => "Magical, curious, childlike discovery",
            EmotionalTone::Reverence => "Sacred, solemn, spiritual depth",
            EmotionalTone::Dread => "Ominous, foreboding, cosmic horror undertones",
            EmotionalTone::Melancholy => "Nostalgic, bittersweet, lost glory",
            EmotionalTone::Joy => "Celebratory, abundant, life-affirming",
            EmotionalTone::Curiosity => "Mysterious, puzzling, inviting exploration",
            EmotionalTone::Unease => "Unsettling, wrong, subtly threatening",
        }
    }
}

/// Classification of significant geographic features
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GeographicFeature {
    // Elevation features
    MountainPeak {
        height: f32,
        is_volcanic: bool,
    },
    MountainRange {
        peak_count: usize,
        highest: f32,
    },
    Valley {
        depth: f32,
        river_carved: bool,
    },
    Plateau {
        elevation: f32,
        area: usize,
    },
    Cliff {
        drop: f32,
    },

    // Water features
    RiverSource {
        flow_strength: f32,
    },
    RiverMouth {
        delta: bool,
    },
    RiverConfluence,
    Lake {
        area: usize,
        depth: f32,
    },
    Waterfall {
        height: f32,
    },
    HotSpring,

    // Coastal features
    Peninsula,
    Bay,
    Island {
        area: usize,
    },
    Strait,
    Coast,

    // Tectonic features
    PlateBoundary {
        stress: f32,
        convergent: bool,
    },
    Rift {
        depth: f32,
    },
    Volcano {
        active: bool,
    },

    // Rare biome features
    AncientSite {
        biome: String,
    },
    MysticalAnomaly {
        biome: String,
    },
    PrimordialRemnant {
        biome: String,
    },

    // Climate features
    GlacialField,
    DesertHeart,
    JungleCore,
    FrozenWaste,

    // Generic
    BiomeTransition {
        from: String,
        to: String,
    },
}

impl GeographicFeature {
    /// Get a description for story generation
    pub fn description(&self) -> String {
        match self {
            GeographicFeature::MountainPeak { height, is_volcanic } => {
                if *is_volcanic {
                    format!("a volcanic peak reaching {}m", *height as i32)
                } else {
                    format!("a mountain summit at {}m", *height as i32)
                }
            }
            GeographicFeature::MountainRange { peak_count, highest } => {
                format!(
                    "a mountain range with {} peaks, the tallest at {}m",
                    peak_count, *highest as i32
                )
            }
            GeographicFeature::Valley { depth, river_carved } => {
                if *river_carved {
                    format!("a river-carved valley {}m deep", (*depth).abs() as i32)
                } else {
                    format!("a valley depression of {}m", (*depth).abs() as i32)
                }
            }
            GeographicFeature::Lake { area, depth } => {
                format!(
                    "a lake spanning {} tiles, {}m deep",
                    area,
                    (*depth).abs() as i32
                )
            }
            GeographicFeature::PlateBoundary { convergent, .. } => {
                if *convergent {
                    "a zone where landmasses collide".to_string()
                } else {
                    "a rift where the earth tears apart".to_string()
                }
            }
            GeographicFeature::Volcano { active } => {
                if *active {
                    "an active volcano".to_string()
                } else {
                    "a dormant volcano".to_string()
                }
            }
            GeographicFeature::AncientSite { biome } => {
                format!("an ancient site of type {}", biome)
            }
            GeographicFeature::MysticalAnomaly { biome } => {
                format!("a mystical anomaly: {}", biome)
            }
            GeographicFeature::Coast => "a coastline where land meets sea".to_string(),
            _ => format!("{:?}", self),
        }
    }
}

/// Cosmic scale for creation myths
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CosmicScale {
    Local,      // A single feature
    Regional,   // A region or territory
    Continental, // A major landmass
    Cosmic,     // The whole world
}

/// Type of heroic journey
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum JourneyType {
    Ascent,       // Climbing to heights
    Descent,      // Going into depths
    Crossing,     // Traversing dangerous terrain
    Quest,        // Seeking something specific
    Exile,        // Forced wandering
    Pilgrimage,   // Sacred journey
    CosmicBattle, // Conflict with primordial forces
}

/// Moral theme for parables
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoralTheme {
    Hubris,
    Sacrifice,
    Wisdom,
    Courage,
    Patience,
    Balance,
    Transformation,
    Harmony,
}

/// Type of disaster for cataclysm myths
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisasterType {
    VolcanicEruption,
    Flood,
    Earthquake,
    WorldRift,
    CosmicImpact,
    DivineWrath,
    Corruption,
}

/// Source of sanctity for sacred places
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SanctitySource {
    ClosenessToSky,
    AncientPresence,
    ElementalConvergence,
    DivineManifestation,
    FirstForest,
    SacredWaters,
    AncestralBurial,
}

/// Type of danger for forbidden zones
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DangerType {
    PhysicalHazard,
    CursedGround,
    DwellingOfMonsters,
    TooFarFromSea,
    ForbiddenKnowledge,
    ThinReality,
}

/// Reason for civilization's fall
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FallCause {
    Hubris,
    NaturalDisaster,
    War,
    Corruption,
    Abandonment,
    Unknown,
}

/// A structured story element generated from an encounter
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StorySeed {
    pub id: StorySeedId,
    pub seed_type: StorySeedType,
    pub primary_location: WorldLocation,
    pub related_landmarks: Vec<LandmarkId>,
    pub themes: Vec<NarrativeTheme>,
    pub archetypes: Vec<Archetype>,
    pub emotional_tone: EmotionalTone,
    pub source_wanderers: Vec<u32>,
    pub suggested_elements: SuggestedElements,
}

/// Type of story seed
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum StorySeedType {
    CreationMyth {
        origin_feature: GeographicFeature,
        cosmic_scale: CosmicScale,
    },
    HeroLegend {
        journey_type: JourneyType,
        trial_features: Vec<String>,
    },
    Parable {
        moral_theme: MoralTheme,
        setting_feature: String,
    },
    OriginStory {
        people_or_creature: String,
        birthplace_feature: String,
    },
    CataclysmMyth {
        disaster_type: DisasterType,
        affected_region_description: String,
    },
    SacredPlace {
        sanctity_source: SanctitySource,
        pilgrimage_worthy: bool,
    },
    ForbiddenZone {
        danger_type: DangerType,
        warning_signs: Vec<String>,
    },
    LostCivilization {
        ruin_biome: String,
        fall_cause: FallCause,
    },
}

/// Narrative themes for stories
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NarrativeTheme {
    Creation,
    Destruction,
    Transformation,
    Sacrifice,
    Journey,
    Conflict,
    Discovery,
    Loss,
    Rebirth,
    Mystery,
    Power,
    Nature,
}

/// Mythological archetypes
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Archetype {
    Creator,
    Destroyer,
    Trickster,
    Hero,
    Monster,
    Sage,
    Guardian,
    Wanderer,
    Innocent,
    Shadow,
}

/// Suggested story elements based on geographic context
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SuggestedElements {
    pub deity_names: Vec<String>,
    pub creature_types: Vec<String>,
    pub artifact_types: Vec<String>,
    pub ritual_types: Vec<String>,
    pub taboos: Vec<String>,
}

/// Extent of a landmark (single point, cluster, or region)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum LandmarkExtent {
    Point { x: usize, y: usize },
    Cluster { tiles: Vec<(usize, usize)>, center: (usize, usize) },
    Region { bounds: (usize, usize, usize, usize), representative_tile: (usize, usize) },
}

/// How a wanderer with their cultural lens interprets a landmark
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LandmarkInterpretation {
    pub wanderer_id: u32,
    pub cultural_lens_type: String,
    pub perceived_name: String,
    pub mythological_role: String,
    pub emotional_response: EmotionalTone,
}

/// A named geographic feature discovered by wanderers
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Landmark {
    pub id: LandmarkId,
    pub name: String,
    pub primary_location: WorldLocation,
    pub extent: LandmarkExtent,
    pub feature_type: GeographicFeature,
    pub discovered_by: Vec<u32>,
    pub interpretations: Vec<LandmarkInterpretation>,
}

/// Type of encounter during wanderer's journey
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EncounterType {
    FirstSighting { feature: GeographicFeature },
    BiomeTransition { from: String, to: String },
    RareDiscovery { biome: String },
    ClimateExtreme { extreme_type: String },
    TectonicEvidence { stress: f32 },
    WaterCrossing { water_type: String },
    ElevationMilestone { reached: f32, direction: String },
    PathConvergence { other_wanderer: u32 },
    ReturnToKnown,
}

/// Wanderer's reaction to an encounter
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WandererReaction {
    pub emotional_response: EmotionalTone,
    pub interpretation: String,
    pub cultural_significance: f32, // 0.0-1.0
}

/// An encounter event during a wanderer's journey
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Encounter {
    pub location: WorldLocation,
    pub encounter_type: EncounterType,
    pub feature_discovered: Option<GeographicFeature>,
    pub landmark_discovered: Option<LandmarkId>,
    pub story_seed_generated: Option<StorySeedId>,
    pub wanderer_reaction: WandererReaction,
    pub step_number: usize,
}

/// A wandering storyteller agent
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Wanderer {
    pub id: u32,
    pub name: String,
    pub origin: WorldLocation,
    pub current_position: (usize, usize),
    pub cultural_lens: CulturalLens,
    pub path_history: Vec<(usize, usize)>,
    pub encounters: Vec<Encounter>,
    pub discovered_landmarks: Vec<LandmarkId>,
    pub steps_taken: usize,
    #[serde(skip)]
    pub visited_biomes: HashSet<String>,
    pub fatigue: f32,
    pub wonder_meter: f32,
}

impl Wanderer {
    /// Create a new wanderer
    pub fn new(
        id: u32,
        name: String,
        origin: WorldLocation,
        cultural_lens: CulturalLens,
    ) -> Self {
        let pos = (origin.x, origin.y);
        let mut visited = HashSet::new();
        visited.insert(origin.biome.clone());

        Self {
            id,
            name,
            origin,
            current_position: pos,
            cultural_lens,
            path_history: vec![pos],
            encounters: Vec::new(),
            discovered_landmarks: Vec::new(),
            steps_taken: 0,
            visited_biomes: visited,
            fatigue: 0.0,
            wonder_meter: 0.0,
        }
    }

    /// Record an encounter
    pub fn add_encounter(&mut self, encounter: Encounter) {
        if let Some(landmark_id) = encounter.landmark_discovered {
            if !self.discovered_landmarks.contains(&landmark_id) {
                self.discovered_landmarks.push(landmark_id);
            }
        }
        self.encounters.push(encounter);
    }
}
