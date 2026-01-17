//! Landmark detection and naming system
//!
//! Manages discovered landmarks and generates appropriate names based on features.

use rand::Rng;
use rand_chacha::ChaCha8Rng;
use std::collections::HashMap;

use super::types::{
    CulturalLens, EmotionalTone, GeographicFeature, Landmark, LandmarkExtent, LandmarkId,
    LandmarkInterpretation, WorldLocation,
};

/// Name components for different feature types
const MOUNTAIN_PREFIXES: &[&str] = &[
    "Mount", "Peak", "Summit", "Spire", "Tooth", "Horn", "Crown", "Throne",
];
const MOUNTAIN_NAMES: &[&str] = &[
    "Thunder", "Storm", "Sky", "Cloud", "Eagle", "Frozen", "Ancient", "Lone",
    "Twin", "Broken", "Sleeping", "Watching", "Silver", "Iron", "Stone",
];

const WATER_PREFIXES: &[&str] = &[
    "Lake", "Sea", "Pool", "Waters", "Depths", "Falls", "Springs",
];
const WATER_NAMES: &[&str] = &[
    "Mirror", "Crystal", "Serpent", "Mist", "Moon", "Star", "Shadow", "Silver",
    "Tears", "Dreams", "Whisper", "Echo", "Reflection", "Stillness",
];

const VALLEY_PREFIXES: &[&str] = &[
    "Vale", "Valley", "Glen", "Dell", "Hollow", "Gorge", "Canyon",
];
const VALLEY_NAMES: &[&str] = &[
    "Shadow", "Hidden", "Lost", "Forgotten", "Verdant", "Silent", "Echoing",
    "Winding", "Deep", "Sacred", "Ancient", "Twilight",
];

const VOLCANIC_PREFIXES: &[&str] = &[
    "Mount", "Caldera", "Furnace", "Forge", "Hearth",
];
const VOLCANIC_NAMES: &[&str] = &[
    "Flame", "Fire", "Ember", "Ash", "Cinder", "Molten", "Burning", "Infernal",
    "Wrath", "Fury", "Dragon", "Phoenix", "Doom",
];

const MYSTICAL_PREFIXES: &[&str] = &[
    "The", "Sacred", "Cursed", "Blessed", "Eternal", "Ancient",
];
const MYSTICAL_NAMES: &[&str] = &[
    "Nexus", "Sanctum", "Threshold", "Veil", "Gate", "Heart", "Eye", "Wound",
    "Scar", "Font", "Well", "Shrine", "Altar",
];

const RUIN_PREFIXES: &[&str] = &[
    "Ruins of", "Fallen", "Lost", "Sunken", "Buried", "Forgotten",
];
const RUIN_NAMES: &[&str] = &[
    "Citadel", "Temple", "Palace", "Tower", "City", "Fortress", "Sanctuary",
    "Halls", "Throne", "Spire", "Catacombs",
];

/// Registry for tracking discovered landmarks
pub struct LandmarkRegistry {
    landmarks: HashMap<LandmarkId, Landmark>,
    next_id: u32,
    min_separation: usize,
    // Spatial index: grid cells to landmark IDs
    spatial_grid: HashMap<(usize, usize), Vec<LandmarkId>>,
    grid_cell_size: usize,
}

impl LandmarkRegistry {
    /// Create a new landmark registry
    pub fn new(min_separation: usize) -> Self {
        Self {
            landmarks: HashMap::new(),
            next_id: 0,
            min_separation,
            spatial_grid: HashMap::new(),
            grid_cell_size: min_separation.max(10),
        }
    }

    /// Get grid cell for a position
    fn grid_cell(&self, x: usize, y: usize) -> (usize, usize) {
        (x / self.grid_cell_size, y / self.grid_cell_size)
    }

    /// Check if a position is too close to existing landmarks
    fn is_too_close(&self, x: usize, y: usize) -> Option<LandmarkId> {
        let (gx, gy) = self.grid_cell(x, y);

        // Check surrounding grid cells
        for dx in -1i32..=1 {
            for dy in -1i32..=1 {
                let cx = (gx as i32 + dx) as usize;
                let cy = (gy as i32 + dy) as usize;

                if let Some(ids) = self.spatial_grid.get(&(cx, cy)) {
                    for id in ids {
                        if let Some(landmark) = self.landmarks.get(id) {
                            let lx = landmark.primary_location.x;
                            let ly = landmark.primary_location.y;
                            let dist = ((x as i32 - lx as i32).abs()
                                + (y as i32 - ly as i32).abs())
                                as usize;
                            if dist < self.min_separation {
                                return Some(*id);
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// Register a new landmark or return existing one if too close
    pub fn register_or_get(
        &mut self,
        location: WorldLocation,
        feature: &GeographicFeature,
        wanderer_id: u32,
        cultural_lens: &CulturalLens,
        rng: &mut ChaCha8Rng,
    ) -> LandmarkId {
        let x = location.x;
        let y = location.y;

        // Check for nearby existing landmark
        if let Some(existing_id) = self.is_too_close(x, y) {
            // Add this wanderer's interpretation to existing landmark
            if let Some(landmark) = self.landmarks.get_mut(&existing_id) {
                if !landmark.discovered_by.contains(&wanderer_id) {
                    landmark.discovered_by.push(wanderer_id);

                    // Add cultural interpretation
                    let interpretation = create_interpretation(
                        wanderer_id,
                        cultural_lens,
                        feature,
                        &landmark.name,
                        rng,
                    );
                    landmark.interpretations.push(interpretation);
                }
            }
            return existing_id;
        }

        // Create new landmark
        let id = LandmarkId(self.next_id);
        self.next_id += 1;

        let name = generate_name(feature, rng);
        let interpretation = create_interpretation(wanderer_id, cultural_lens, feature, &name, rng);

        let landmark = Landmark {
            id,
            name,
            primary_location: location,
            extent: LandmarkExtent::Point { x, y },
            feature_type: feature.clone(),
            discovered_by: vec![wanderer_id],
            interpretations: vec![interpretation],
        };

        // Add to spatial grid
        let cell = self.grid_cell(x, y);
        self.spatial_grid
            .entry(cell)
            .or_insert_with(Vec::new)
            .push(id);

        self.landmarks.insert(id, landmark);
        id
    }

    /// Get a landmark by ID
    pub fn get(&self, id: LandmarkId) -> Option<&Landmark> {
        self.landmarks.get(&id)
    }

    /// Finalize and return all landmarks
    pub fn finalize(self) -> Vec<Landmark> {
        self.landmarks.into_values().collect()
    }
}

/// Generate a name for a geographic feature
fn generate_name(feature: &GeographicFeature, rng: &mut ChaCha8Rng) -> String {
    match feature {
        GeographicFeature::MountainPeak { is_volcanic, .. } => {
            if *is_volcanic {
                let prefix = VOLCANIC_PREFIXES[rng.gen_range(0..VOLCANIC_PREFIXES.len())];
                let name = VOLCANIC_NAMES[rng.gen_range(0..VOLCANIC_NAMES.len())];
                format!("{} {}", prefix, name)
            } else {
                let prefix = MOUNTAIN_PREFIXES[rng.gen_range(0..MOUNTAIN_PREFIXES.len())];
                let name = MOUNTAIN_NAMES[rng.gen_range(0..MOUNTAIN_NAMES.len())];
                format!("{} {}", prefix, name)
            }
        }

        GeographicFeature::MountainRange { .. } => {
            let name = MOUNTAIN_NAMES[rng.gen_range(0..MOUNTAIN_NAMES.len())];
            let suffix = ["Mountains", "Range", "Peaks", "Heights"][rng.gen_range(0..4)];
            format!("The {} {}", name, suffix)
        }

        GeographicFeature::Valley { .. } => {
            let prefix = VALLEY_PREFIXES[rng.gen_range(0..VALLEY_PREFIXES.len())];
            let name = VALLEY_NAMES[rng.gen_range(0..VALLEY_NAMES.len())];
            format!("{} of {}", prefix, name)
        }

        GeographicFeature::Lake { .. } | GeographicFeature::HotSpring => {
            let prefix = WATER_PREFIXES[rng.gen_range(0..WATER_PREFIXES.len())];
            let name = WATER_NAMES[rng.gen_range(0..WATER_NAMES.len())];
            format!("{} of {}", prefix, name)
        }

        GeographicFeature::Volcano { active } => {
            let prefix = VOLCANIC_PREFIXES[rng.gen_range(0..VOLCANIC_PREFIXES.len())];
            let name = if *active {
                VOLCANIC_NAMES[rng.gen_range(0..VOLCANIC_NAMES.len())]
            } else {
                ["Dormant", "Sleeping", "Silent", "Cold"][rng.gen_range(0..4)]
            };
            format!("{} {}", prefix, name)
        }

        GeographicFeature::PlateBoundary { convergent, .. } => {
            if *convergent {
                let name = ["Collision", "Clash", "Meeting", "Union"][rng.gen_range(0..4)];
                format!("The {} of Lands", name)
            } else {
                let name = ["Rift", "Tear", "Wound", "Divide"][rng.gen_range(0..4)];
                format!("The Great {}", name)
            }
        }

        GeographicFeature::AncientSite { biome } | GeographicFeature::MysticalAnomaly { biome } => {
            let prefix = MYSTICAL_PREFIXES[rng.gen_range(0..MYSTICAL_PREFIXES.len())];
            let name = MYSTICAL_NAMES[rng.gen_range(0..MYSTICAL_NAMES.len())];
            format!("{} {} of {}", prefix, name, biome)
        }

        GeographicFeature::PrimordialRemnant { .. } => {
            let prefix = RUIN_PREFIXES[rng.gen_range(0..RUIN_PREFIXES.len())];
            let name = RUIN_NAMES[rng.gen_range(0..RUIN_NAMES.len())];
            format!("{} {}", prefix, name)
        }

        GeographicFeature::Coast => {
            let name = ["Windswept", "Eternal", "Storm", "Forgotten", "Silver"][rng.gen_range(0..5)];
            let suffix = ["Shore", "Coast", "Strand", "Edge"][rng.gen_range(0..4)];
            format!("The {} {}", name, suffix)
        }

        _ => {
            // Generic naming for other features
            let prefix = MYSTICAL_PREFIXES[rng.gen_range(0..MYSTICAL_PREFIXES.len())];
            let name = MYSTICAL_NAMES[rng.gen_range(0..MYSTICAL_NAMES.len())];
            format!("{} {}", prefix, name)
        }
    }
}

/// Create a cultural interpretation of a landmark
fn create_interpretation(
    wanderer_id: u32,
    cultural_lens: &CulturalLens,
    feature: &GeographicFeature,
    original_name: &str,
    rng: &mut ChaCha8Rng,
) -> LandmarkInterpretation {
    let (perceived_name, mythological_role, emotional_response) = match (cultural_lens, feature) {
        // Highland interpretations
        (CulturalLens::Highland { ancestor_worship, .. }, GeographicFeature::MountainPeak { .. }) => {
            let name = if *ancestor_worship {
                "Ancestor's Throne"
            } else {
                "Sky Father's Seat"
            };
            (name.to_string(), "World Pillar".to_string(), EmotionalTone::Reverence)
        }

        (CulturalLens::Highland { .. }, GeographicFeature::Valley { .. }) => {
            ("The Low Place".to_string(), "Realm of Spirits".to_string(), EmotionalTone::Unease)
        }

        // Maritime interpretations
        (CulturalLens::Maritime { sea_deity_name, .. }, GeographicFeature::Lake { .. }) => {
            let name = format!("{}'s Mirror", sea_deity_name);
            (name, "Sacred Waters".to_string(), EmotionalTone::Reverence)
        }

        (CulturalLens::Maritime { .. }, GeographicFeature::MountainPeak { .. }) => {
            ("The Forbidden Heights".to_string(), "Barrier".to_string(), EmotionalTone::Dread)
        }

        // Desert interpretations
        (CulturalLens::Desert { water_sacred, .. }, GeographicFeature::Lake { .. }) => {
            if *water_sacred {
                ("Gift of the Stars".to_string(), "Life Source".to_string(), EmotionalTone::Reverence)
            } else {
                ("Oasis".to_string(), "Waypoint".to_string(), EmotionalTone::Joy)
            }
        }

        (CulturalLens::Desert { .. }, GeographicFeature::Volcano { .. }) => {
            ("Sun's Forge".to_string(), "Birth of Fire".to_string(), EmotionalTone::Awe)
        }

        // Sylvan interpretations
        (CulturalLens::Sylvan { tree_worship, .. }, GeographicFeature::AncientSite { .. }) => {
            if *tree_worship {
                ("The First Grove".to_string(), "Origin".to_string(), EmotionalTone::Reverence)
            } else {
                ("Elder Place".to_string(), "Spirit Home".to_string(), EmotionalTone::Wonder)
            }
        }

        (CulturalLens::Sylvan { fears_open_sky, .. }, GeographicFeature::Valley { .. }) => {
            if *fears_open_sky {
                ("Sheltered Dell".to_string(), "Safe Haven".to_string(), EmotionalTone::Joy)
            } else {
                ("Green Hollow".to_string(), "Gathering Place".to_string(), EmotionalTone::Wonder)
            }
        }

        // Steppe interpretations
        (CulturalLens::Steppe { sky_worship, .. }, GeographicFeature::MountainPeak { .. }) => {
            if *sky_worship {
                ("Sky Pillar".to_string(), "Bridge to Heaven".to_string(), EmotionalTone::Awe)
            } else {
                ("The Obstacle".to_string(), "Boundary".to_string(), EmotionalTone::Fear)
            }
        }

        // Subterranean interpretations
        (CulturalLens::Subterranean { crystal_worship, .. }, GeographicFeature::Valley { .. }) => {
            if *crystal_worship {
                ("Crystal Vein".to_string(), "Gateway to Depths".to_string(), EmotionalTone::Wonder)
            } else {
                ("Path Below".to_string(), "Entrance".to_string(), EmotionalTone::Curiosity)
            }
        }

        (CulturalLens::Subterranean { .. }, GeographicFeature::MountainPeak { .. }) => {
            ("The Exposed".to_string(), "Unsafe Heights".to_string(), EmotionalTone::Dread)
        }

        // Default: use original name with cultural suffix
        _ => {
            let emotion = match cultural_lens {
                CulturalLens::Highland { .. } => EmotionalTone::Curiosity,
                CulturalLens::Maritime { .. } => EmotionalTone::Wonder,
                CulturalLens::Desert { .. } => EmotionalTone::Awe,
                CulturalLens::Sylvan { .. } => EmotionalTone::Wonder,
                CulturalLens::Steppe { .. } => EmotionalTone::Curiosity,
                CulturalLens::Subterranean { .. } => EmotionalTone::Curiosity,
            };
            (original_name.to_string(), "Unknown Site".to_string(), emotion)
        }
    };

    LandmarkInterpretation {
        wanderer_id,
        cultural_lens_type: cultural_lens.culture_name().to_string(),
        perceived_name,
        mythological_role,
        emotional_response,
    }
}
