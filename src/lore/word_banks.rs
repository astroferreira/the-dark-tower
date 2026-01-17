//! Rich word banks for procedural mythology generation
//!
//! Provides diverse pools of names, creatures, artifacts, rituals, and taboos
//! organized by climate, terrain, and cultural perspective.

use rand::seq::SliceRandom;
use rand::Rng;
use rand_chacha::ChaCha8Rng;

use super::types::CulturalLens;

/// Climate categories that affect word selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClimateCategory {
    Cold,
    Temperate,
    Hot,
    Wet,
    Dry,
}

/// Terrain types that affect word selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainType {
    Mountain,
    Water,
    Forest,
    Desert,
    Plains,
    Underground,
    Mystical,
    Coastal,
    Wetland,
}

// ============================================================================
// DEITY NAME COMPONENTS
// ============================================================================

const DEITY_PREFIXES: &[&str] = &[
    "The", "Great", "Old", "Silent", "Eternal", "First", "Last", "Hidden",
    "Dreaming", "Sleeping", "Watching", "Wandering", "Forgotten", "Nameless",
    "Ancient", "Pale", "Dark", "Bright", "Hollow", "Bone", "Stone", "Iron",
    "Silver", "Golden", "Twilight", "Dawn", "Dusk", "Midnight", "Storm",
    "Gentle", "Fierce", "Patient", "Hungry", "Weeping", "Laughing", "Singing",
    "Whispering", "Screaming", "Dancing", "Still", "Ever-", "Never-",
];

const DEITY_CORES: &[&str] = &[
    "Watcher", "Keeper", "Dancer", "Dreamer", "Walker", "Weaver", "Singer",
    "Binder", "Breaker", "Maker", "Shaper", "Caller", "Hunter", "Guardian",
    "Shepherd", "Father", "Mother", "Sister", "Brother", "Child", "Elder",
    "Stranger", "Wanderer", "Sleeper", "Speaker", "Listener", "Seeker",
    "Giver", "Taker", "Opener", "Closer", "Builder", "Destroyer", "Healer",
    "Devourer", "Rememberer", "Forgetter", "Judge", "Witness", "Herald",
    "Messenger", "Throne", "Crown", "Heart", "Eye", "Hand", "Voice", "Shadow",
];

const DEITY_DOMAINS: &[&str] = &[
    "of Storms", "of Stone", "of Silence", "of Stars", "of Shadows",
    "of the Deep", "of the Heights", "of the Threshold", "of Endings",
    "of Beginnings", "of the Lost", "of the Forgotten", "of Bones",
    "of Embers", "of Frost", "of Mist", "of Thunder", "of Secrets",
    "of the Hunt", "of the Harvest", "of Tides", "of Winds", "of Dreams",
    "of the Void", "of the Veil", "of Passage", "of Memory", "of Time",
    "Beneath", "Above", "Between", "Within", "Beyond", "Before All",
    "Who Waits", "Who Watches", "Who Remembers", "Who Hungers",
    "of the First Days", "of the Last Night", "of the Long Dark",
];

// Climate-specific deity words
const COLD_DEITY_WORDS: &[&str] = &[
    "Frost", "Ice", "Winter", "Pale", "Sleeping", "Frozen", "Glacier",
    "Snow", "Bitter", "Cold", "Numb", "Silent", "Still", "Hibernating",
];

const HOT_DEITY_WORDS: &[&str] = &[
    "Flame", "Sun", "Burning", "Scorching", "Blazing", "Radiant", "Ember",
    "Ash", "Searing", "Bright", "Golden", "Forge", "Molten", "Fire",
];

const WET_DEITY_WORDS: &[&str] = &[
    "Tide", "Depth", "Flow", "Drowning", "Wave", "Current", "Deluge",
    "Rain", "Flood", "Mist", "Pool", "Spring", "River", "Ocean",
];

const DRY_DEITY_WORDS: &[&str] = &[
    "Dust", "Bone", "Wind", "Thirst", "Sand", "Mirage", "Heat",
    "Desiccation", "Withered", "Parched", "Salt", "Cracked", "Empty",
];

// ============================================================================
// CREATURE COMPONENTS
// ============================================================================

const CREATURE_ADJECTIVES: &[&str] = &[
    "shadow", "frost", "ancient", "pale", "dark", "silent", "howling",
    "creeping", "lurking", "dancing", "wandering", "weeping", "laughing",
    "hungry", "patient", "swift", "massive", "tiny", "translucent",
    "bone", "stone", "iron", "silver", "golden", "fire", "ice", "thunder",
    "mist", "dream", "nightmare", "twilight", "midnight", "dawn", "dusk",
    "vengeful", "sorrowful", "joyful", "mad", "wise", "forgotten",
    "eternal", "dying", "reborn", "shifting", "many-faced", "faceless",
];

const CREATURE_TYPES: &[&str] = &[
    "spirits", "wraiths", "serpents", "wolves", "ravens", "spiders",
    "worms", "giants", "dwarves", "elementals", "guardians", "watchers",
    "hunters", "dancers", "singers", "weavers", "walkers", "crawlers",
    "swimmers", "flyers", "burrowers", "stalkers", "feeders", "dreamers",
    "echoes", "shadows", "reflections", "memories", "ancestors", "children",
    "beasts", "dragons", "wyrms", "leviathans", "behemoths", "phoenixes",
    "golems", "constructs", "shades", "specters", "phantoms", "haunts",
    "wisps", "will-o-wisps", "lantern bearers", "night things", "old ones",
];

// Climate-specific creatures
const COLD_CREATURES: &[&str] = &[
    "frost giants", "ice wraiths", "snow stalkers", "glacier spirits",
    "frozen dead", "winter wolves", "pale spiders", "rime serpents",
    "aurora dancers", "blizzard elementals", "sleeping ancients",
];

const HOT_CREATURES: &[&str] = &[
    "fire salamanders", "ash wraiths", "sun serpents", "ember spirits",
    "flame dancers", "heat mirages", "burning dead", "forge beasts",
    "lava swimmers", "smoke phantoms", "cinder wolves",
];

const WET_CREATURES: &[&str] = &[
    "tide spirits", "depth lurkers", "drowned dead", "water serpents",
    "mist walkers", "rain singers", "flood giants", "current dancers",
    "kelp weavers", "pearl guardians", "wave riders",
];

const DRY_CREATURES: &[&str] = &[
    "dust devils", "bone walkers", "sand serpents", "mirage spirits",
    "wind howlers", "thirst demons", "salt wraiths", "desiccated dead",
    "oasis guardians", "star-touched", "dune swimmers",
];

// Terrain-specific creatures
const MOUNTAIN_CREATURES: &[&str] = &[
    "stone giants", "peak eagles", "echo spirits", "cliff crawlers",
    "thunder beasts", "avalanche dead", "height singers", "crag wolves",
];

const FOREST_CREATURES: &[&str] = &[
    "tree spirits", "root walkers", "branch weavers", "leaf dancers",
    "bark golems", "moss ancients", "shadow stalkers", "canopy flyers",
];

const UNDERGROUND_CREATURES: &[&str] = &[
    "crystal spiders", "deep worms", "echo hunters", "darkness swimmers",
    "gem guardians", "tunnel crawlers", "blind prophets", "stone eaters",
];

const MYSTICAL_CREATURES: &[&str] = &[
    "void touched", "reality weavers", "dream walkers", "fate spinners",
    "ether beasts", "star eaters", "time echoes", "dimension dancers",
];

// ============================================================================
// ARTIFACT COMPONENTS
// ============================================================================

const ARTIFACT_MATERIALS: &[&str] = &[
    "bone", "crystal", "iron", "bronze", "silver", "gold", "obsidian",
    "jade", "amber", "ivory", "stone", "wood", "leather", "feather",
    "scale", "tooth", "claw", "horn", "pearl", "coral", "meteor",
    "star-metal", "shadow-forged", "dream-woven", "blood-tempered",
    "frost-touched", "fire-kissed", "void-born", "ancient", "petrified",
];

const ARTIFACT_FORMS: &[&str] = &[
    "crown", "blade", "chalice", "amulet", "ring", "staff", "orb",
    "mask", "mirror", "key", "chain", "book", "scroll", "tablet",
    "bell", "horn", "drum", "flute", "needle", "thread", "loom",
    "hammer", "anvil", "cauldron", "lantern", "compass", "map",
    "throne", "gate", "pillar", "statue", "heart", "eye", "hand",
];

const ARTIFACT_QUALITIES: &[&str] = &[
    "of binding", "of breaking", "of passage", "of calling", "of silence",
    "of the first king", "of the last prophet", "of endless night",
    "that sees", "that speaks", "that hungers", "that remembers",
    "of forbidden knowledge", "of lost ages", "of the covenant",
    "never-resting", "ever-watching", "all-seeing", "truth-speaking",
];

// ============================================================================
// RITUAL COMPONENTS
// ============================================================================

const RITUAL_VERBS: &[&str] = &[
    "calling", "binding", "offering", "burning", "drowning", "burying",
    "singing", "dancing", "walking", "watching", "waiting", "weeping",
    "laughing", "speaking", "silencing", "remembering", "forgetting",
    "marking", "cleansing", "blessing", "cursing", "sealing", "opening",
    "crossing", "returning", "ascending", "descending", "transforming",
];

const RITUAL_OBJECTS: &[&str] = &[
    "blood", "bone", "ash", "salt", "water", "fire", "earth", "wind",
    "song", "silence", "names", "secrets", "dreams", "shadows", "light",
    "darkness", "the dead", "the living", "ancestors", "children",
    "the moon", "the stars", "the sun", "the tides", "the seasons",
    "thresholds", "boundaries", "crossroads", "sacred places",
];

const RITUAL_TIMES: &[&str] = &[
    "at midnight", "at dawn", "at dusk", "under the full moon",
    "during the solstice", "at the turning of seasons", "when stars align",
    "in the deep night", "at the threshold hour", "during the storm",
    "in perfect silence", "while the wind howls", "as the tide turns",
];

// Culture-specific rituals
const HIGHLAND_RITUALS: &[&str] = &[
    "the climb of remembrance", "stone stacking for ancestors",
    "summit offerings", "echo calling", "the endurance walk",
    "peak-fire lighting", "ancestor stone blessing",
];

const MARITIME_RITUALS: &[&str] = &[
    "tide offerings", "the drowning of sorrows", "salt blessing",
    "the voyage song", "net weaving prayers", "wave watching",
    "the return of the lost", "pearl diving meditation",
];

const DESERT_RITUALS: &[&str] = &[
    "star reading", "oasis blessing", "sand walking meditation",
    "the water remembrance", "mirage hunting", "bone burial",
    "the thirst prayer", "dawn greeting", "dusk farewell",
];

const SYLVAN_RITUALS: &[&str] = &[
    "tree binding", "root communion", "the leaf falling ceremony",
    "branch weaving", "seed planting prayers", "bark reading",
    "the forest walking", "canopy meditation", "moss gathering",
];

const STEPPE_RITUALS: &[&str] = &[
    "sky watching", "wind calling", "the great ride",
    "horizon blessing", "herd song", "grass braiding",
    "the open sky meditation", "thunder greeting",
];

const SUBTERRANEAN_RITUALS: &[&str] = &[
    "crystal communion", "echo listening", "the descent",
    "darkness meditation", "gem offering", "tunnel blessing",
    "the return to stone", "depth calling",
];

// ============================================================================
// TABOO COMPONENTS
// ============================================================================

const TABOO_ACTIONS: &[&str] = &[
    "speaking", "touching", "looking at", "naming", "crossing",
    "entering", "leaving", "eating", "drinking", "sleeping near",
    "dreaming of", "forgetting", "remembering", "counting", "pointing at",
    "walking past", "ignoring", "mocking", "imitating", "stealing from",
    "turning your back to", "casting shadow upon", "singing near",
];

const TABOO_SUBJECTS: &[&str] = &[
    "the dead", "running water", "iron", "salt", "fire", "shadows",
    "mirrors", "crossroads", "thresholds", "sacred stones", "old trees",
    "deep pools", "mountain peaks", "the moon", "certain stars",
    "ancient ruins", "bone piles", "unmarked graves", "standing stones",
    "the void places", "names of the fallen", "forbidden songs",
];

const TABOO_CONSEQUENCES: &[&str] = &[
    "lest the spirits notice", "or invite the hungry ones",
    "for fear of awakening what sleeps", "to avoid the curse",
    "or be marked for death", "lest you be taken",
    "or lose your way forever", "for the old ways demand it",
    "or bring doom upon your kin", "as the ancestors commanded",
];

// ============================================================================
// MORAL THEMES FOR PARABLES
// ============================================================================

/// All available moral themes for parables
pub const MORAL_THEMES: &[&str] = &[
    "Wisdom", "Sacrifice", "Courage", "Patience", "Humility", "Greed",
    "Pride", "Folly", "Loyalty", "Betrayal", "Compassion", "Justice",
    "Perseverance", "Balance", "Acceptance", "Transformation", "Unity",
    "Respect", "Gratitude", "Forgiveness", "Honesty", "Hope",
];

// ============================================================================
// GENERATOR FUNCTIONS
// ============================================================================

/// Generate a deity name based on context
pub fn generate_deity_name(
    climate: Option<ClimateCategory>,
    terrain: Option<TerrainType>,
    cultural_lens: &CulturalLens,
    rng: &mut ChaCha8Rng,
) -> String {
    let mut components: Vec<&str> = Vec::new();

    // Choose a prefix (50% chance)
    if rng.gen_bool(0.5) {
        components.push(DEITY_PREFIXES.choose(rng).unwrap());
    }

    // Add climate-influenced word (30% chance if climate specified)
    if let Some(c) = climate {
        if rng.gen_bool(0.3) {
            let climate_words = match c {
                ClimateCategory::Cold => COLD_DEITY_WORDS,
                ClimateCategory::Hot => HOT_DEITY_WORDS,
                ClimateCategory::Wet => WET_DEITY_WORDS,
                ClimateCategory::Dry => DRY_DEITY_WORDS,
                ClimateCategory::Temperate => DEITY_CORES,
            };
            components.push(climate_words.choose(rng).unwrap());
        }
    }

    // Add cultural flavor (40% chance)
    if rng.gen_bool(0.4) {
        let cultural_word = match cultural_lens {
            CulturalLens::Highland { .. } => {
                ["Stone", "Peak", "Ancestor", "Height", "Enduring"].choose(rng).unwrap()
            }
            CulturalLens::Maritime { .. } => {
                ["Tide", "Salt", "Depth", "Wave", "Navigator"].choose(rng).unwrap()
            }
            CulturalLens::Desert { .. } => {
                ["Star", "Sand", "Oasis", "Mirage", "Wandering"].choose(rng).unwrap()
            }
            CulturalLens::Sylvan { .. } => {
                ["Root", "Branch", "Leaf", "Grove", "Green"].choose(rng).unwrap()
            }
            CulturalLens::Steppe { .. } => {
                ["Wind", "Sky", "Horizon", "Thunder", "Swift"].choose(rng).unwrap()
            }
            CulturalLens::Subterranean { .. } => {
                ["Crystal", "Depth", "Echo", "Dark", "Hidden"].choose(rng).unwrap()
            }
        };
        components.push(cultural_word);
    }

    // Always add a core noun
    components.push(DEITY_CORES.choose(rng).unwrap());

    // Add domain (70% chance)
    if rng.gen_bool(0.7) {
        // Prefer terrain-appropriate domains
        let domain = if let Some(t) = terrain {
            match t {
                TerrainType::Mountain => {
                    ["of the Heights", "of Stone", "of Thunder", "of the Peak"].choose(rng).unwrap()
                }
                TerrainType::Water | TerrainType::Coastal => {
                    ["of Tides", "of the Deep", "of Waves", "of the Abyss"].choose(rng).unwrap()
                }
                TerrainType::Forest => {
                    ["of the Grove", "of Shadows", "of the Hunt", "of Roots"].choose(rng).unwrap()
                }
                TerrainType::Desert | TerrainType::Plains => {
                    ["of Winds", "of Stars", "of the Horizon", "of Sand"].choose(rng).unwrap()
                }
                TerrainType::Underground => {
                    ["of the Deep", "Beneath", "of Crystal", "of Darkness"].choose(rng).unwrap()
                }
                TerrainType::Mystical => {
                    ["of the Void", "Beyond", "of Dreams", "of the Veil"].choose(rng).unwrap()
                }
                TerrainType::Wetland => {
                    ["of Mist", "of the Marsh", "of Still Waters", "of Reeds"].choose(rng).unwrap()
                }
            }
        } else {
            DEITY_DOMAINS.choose(rng).unwrap()
        };
        components.push(domain);
    }

    components.join(" ")
}

/// Generate a creature name based on context
pub fn generate_creature(
    climate: Option<ClimateCategory>,
    terrain: Option<TerrainType>,
    _cultural_lens: &CulturalLens,
    rng: &mut ChaCha8Rng,
) -> String {
    // First try terrain-specific creatures (40% chance)
    if let Some(t) = terrain {
        if rng.gen_bool(0.4) {
            let terrain_creatures = match t {
                TerrainType::Mountain => MOUNTAIN_CREATURES,
                TerrainType::Forest => FOREST_CREATURES,
                TerrainType::Underground => UNDERGROUND_CREATURES,
                TerrainType::Mystical => MYSTICAL_CREATURES,
                _ => CREATURE_TYPES,
            };
            return terrain_creatures.choose(rng).unwrap().to_string();
        }
    }

    // Then try climate-specific creatures (40% chance)
    if let Some(c) = climate {
        if rng.gen_bool(0.4) {
            let climate_creatures = match c {
                ClimateCategory::Cold => COLD_CREATURES,
                ClimateCategory::Hot => HOT_CREATURES,
                ClimateCategory::Wet => WET_CREATURES,
                ClimateCategory::Dry => DRY_CREATURES,
                ClimateCategory::Temperate => CREATURE_TYPES,
            };
            return climate_creatures.choose(rng).unwrap().to_string();
        }
    }

    // Otherwise generate adjective + type combination
    let adj = CREATURE_ADJECTIVES.choose(rng).unwrap();
    let creature = CREATURE_TYPES.choose(rng).unwrap();
    format!("{} {}", adj, creature)
}

/// Generate multiple creatures
pub fn generate_creatures(
    climate: Option<ClimateCategory>,
    terrain: Option<TerrainType>,
    cultural_lens: &CulturalLens,
    count: usize,
    rng: &mut ChaCha8Rng,
) -> Vec<String> {
    let mut creatures = Vec::new();
    for _ in 0..count {
        let creature = generate_creature(climate, terrain, cultural_lens, rng);
        if !creatures.contains(&creature) {
            creatures.push(creature);
        }
    }
    creatures
}

/// Generate an artifact name
pub fn generate_artifact(
    terrain: Option<TerrainType>,
    rng: &mut ChaCha8Rng,
) -> String {
    let material = ARTIFACT_MATERIALS.choose(rng).unwrap();
    let form = ARTIFACT_FORMS.choose(rng).unwrap();

    // Add quality suffix (50% chance)
    if rng.gen_bool(0.5) {
        let quality = ARTIFACT_QUALITIES.choose(rng).unwrap();
        format!("the {} {} {}", material, form, quality)
    } else {
        // Use terrain-influenced description
        let desc = if let Some(t) = terrain {
            match t {
                TerrainType::Mountain => "of the high places",
                TerrainType::Water | TerrainType::Coastal => "from the depths",
                TerrainType::Forest => "of the old wood",
                TerrainType::Desert => "of the endless sands",
                TerrainType::Underground => "from below",
                TerrainType::Mystical => "touched by the beyond",
                _ => "of forgotten ages",
            }
        } else {
            "of ancient days"
        };
        format!("the {} {} {}", material, form, desc)
    }
}

/// Generate multiple artifacts
pub fn generate_artifacts(
    terrain: Option<TerrainType>,
    count: usize,
    rng: &mut ChaCha8Rng,
) -> Vec<String> {
    (0..count).map(|_| generate_artifact(terrain, rng)).collect()
}

/// Generate a ritual description
pub fn generate_ritual(
    cultural_lens: &CulturalLens,
    rng: &mut ChaCha8Rng,
) -> String {
    // First try culture-specific rituals (60% chance)
    if rng.gen_bool(0.6) {
        let cultural_rituals = match cultural_lens {
            CulturalLens::Highland { .. } => HIGHLAND_RITUALS,
            CulturalLens::Maritime { .. } => MARITIME_RITUALS,
            CulturalLens::Desert { .. } => DESERT_RITUALS,
            CulturalLens::Sylvan { .. } => SYLVAN_RITUALS,
            CulturalLens::Steppe { .. } => STEPPE_RITUALS,
            CulturalLens::Subterranean { .. } => SUBTERRANEAN_RITUALS,
        };
        return cultural_rituals.choose(rng).unwrap().to_string();
    }

    // Generate generic ritual
    let verb = RITUAL_VERBS.choose(rng).unwrap();
    let object = RITUAL_OBJECTS.choose(rng).unwrap();

    // Add time specification (40% chance)
    if rng.gen_bool(0.4) {
        let time = RITUAL_TIMES.choose(rng).unwrap();
        format!("the {} of {} {}", verb, object, time)
    } else {
        format!("the {} of {}", verb, object)
    }
}

/// Generate multiple rituals
pub fn generate_rituals(
    cultural_lens: &CulturalLens,
    count: usize,
    rng: &mut ChaCha8Rng,
) -> Vec<String> {
    (0..count).map(|_| generate_ritual(cultural_lens, rng)).collect()
}

/// Generate a taboo
pub fn generate_taboo(
    terrain: Option<TerrainType>,
    _cultural_lens: &CulturalLens,
    rng: &mut ChaCha8Rng,
) -> String {
    let action = TABOO_ACTIONS.choose(rng).unwrap();

    // Use terrain-appropriate subjects when possible
    let subject = if let Some(t) = terrain {
        match t {
            TerrainType::Mountain => {
                ["peak stones", "the summit", "echo caves", "high places"].choose(rng).unwrap()
            }
            TerrainType::Water | TerrainType::Coastal => {
                ["the deep", "still pools", "the tide line", "drowned things"].choose(rng).unwrap()
            }
            TerrainType::Forest => {
                ["old trees", "the heart grove", "night shadows", "roots"].choose(rng).unwrap()
            }
            TerrainType::Underground => {
                ["crystals", "the deep dark", "echo chambers", "blind things"].choose(rng).unwrap()
            }
            _ => TABOO_SUBJECTS.choose(rng).unwrap(),
        }
    } else {
        TABOO_SUBJECTS.choose(rng).unwrap()
    };

    // Add consequence (50% chance)
    if rng.gen_bool(0.5) {
        let consequence = TABOO_CONSEQUENCES.choose(rng).unwrap();
        format!("{} {} {}", action, subject, consequence)
    } else {
        format!("{} {}", action, subject)
    }
}

/// Generate multiple taboos
pub fn generate_taboos(
    terrain: Option<TerrainType>,
    cultural_lens: &CulturalLens,
    count: usize,
    rng: &mut ChaCha8Rng,
) -> Vec<String> {
    (0..count).map(|_| generate_taboo(terrain, cultural_lens, rng)).collect()
}

/// Pick a random moral theme for parables
pub fn pick_moral_theme(rng: &mut ChaCha8Rng) -> &'static str {
    MORAL_THEMES.choose(rng).unwrap()
}

/// Determine climate category from temperature
pub fn climate_from_temperature(temp: f32) -> ClimateCategory {
    if temp < 0.25 {
        ClimateCategory::Cold
    } else if temp > 0.75 {
        ClimateCategory::Hot
    } else {
        ClimateCategory::Temperate
    }
}

/// Determine climate category from moisture
pub fn climate_from_moisture(moisture: f32) -> ClimateCategory {
    if moisture < 0.3 {
        ClimateCategory::Dry
    } else if moisture > 0.7 {
        ClimateCategory::Wet
    } else {
        ClimateCategory::Temperate
    }
}

/// Determine terrain type from geographic feature description
pub fn terrain_from_feature(feature_desc: &str) -> TerrainType {
    let desc_lower = feature_desc.to_lowercase();

    if desc_lower.contains("mountain") || desc_lower.contains("peak") || desc_lower.contains("volcano") {
        TerrainType::Mountain
    } else if desc_lower.contains("lake") || desc_lower.contains("river") || desc_lower.contains("ocean") {
        TerrainType::Water
    } else if desc_lower.contains("coast") || desc_lower.contains("shore") {
        TerrainType::Coastal
    } else if desc_lower.contains("forest") || desc_lower.contains("grove") || desc_lower.contains("wood") {
        TerrainType::Forest
    } else if desc_lower.contains("desert") || desc_lower.contains("dune") || desc_lower.contains("sand") {
        TerrainType::Desert
    } else if desc_lower.contains("plain") || desc_lower.contains("steppe") || desc_lower.contains("prairie") {
        TerrainType::Plains
    } else if desc_lower.contains("cave") || desc_lower.contains("underground") || desc_lower.contains("deep") {
        TerrainType::Underground
    } else if desc_lower.contains("marsh") || desc_lower.contains("swamp") || desc_lower.contains("fen") {
        TerrainType::Wetland
    } else if desc_lower.contains("void") || desc_lower.contains("anomaly") || desc_lower.contains("ley")
           || desc_lower.contains("mystical") || desc_lower.contains("ethereal") {
        TerrainType::Mystical
    } else {
        TerrainType::Plains // Default
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_chacha::ChaCha8Rng;
    use rand::SeedableRng;

    #[test]
    fn test_deity_name_generation() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let lens = CulturalLens::Highland {
            sacred_direction: crate::lore::types::Direction::North,
            ancestor_worship: true,
        };

        let name = generate_deity_name(Some(ClimateCategory::Cold), Some(TerrainType::Mountain), &lens, &mut rng);
        assert!(!name.is_empty());
        assert!(!name.contains("Unknown")); // Should not be generic
    }

    #[test]
    fn test_creature_generation_variety() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let lens = CulturalLens::Maritime {
            sea_deity_name: "Tidefather".to_string(),
            fears_deep_water: false,
        };

        let creatures: Vec<_> = (0..10)
            .map(|_| generate_creature(Some(ClimateCategory::Wet), Some(TerrainType::Water), &lens, &mut rng))
            .collect();

        // Should have variety (not all the same)
        let unique: std::collections::HashSet<_> = creatures.iter().collect();
        assert!(unique.len() > 3);
    }
}
