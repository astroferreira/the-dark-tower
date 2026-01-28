//! Name generation from naming styles.
//!
//! Generates character names, place names, and epithets using a culture's
//! naming style traits. Each generated name is phonetically consistent
//! with its culture.

use rand::Rng;
use super::styles::NamingStyle;

/// Name generator that produces names from a `NamingStyle`.
pub struct NameGenerator;

impl NameGenerator {
    /// Generate a personal name (e.g., "Krath", "Aelindra", "Grukash").
    pub fn personal_name(style: &NamingStyle, rng: &mut impl Rng) -> String {
        let syllables = rng.gen_range(style.syllable_range.0..=style.syllable_range.1);
        let mut name = String::new();

        for i in 0..syllables {
            // Optionally insert a separator between syllables
            if i > 0 && syllables > 1 {
                let roll: f32 = rng.gen();
                if style.uses_apostrophes && roll < 0.15 {
                    name.push('\'');
                } else if style.uses_hyphens && roll < 0.10 {
                    name.push('-');
                }
            }

            // Onset consonant (skip sometimes for first syllable to get vowel-leading names)
            let skip_onset = i == 0 && rng.gen_bool(0.2);
            if !skip_onset && !style.onset_consonants.is_empty() {
                let c = &style.onset_consonants[rng.gen_range(0..style.onset_consonants.len())];
                if i == 0 {
                    // Capitalize first letter
                    let mut chars = c.chars();
                    if let Some(first) = chars.next() {
                        name.push(first.to_uppercase().next().unwrap_or(first));
                        name.extend(chars);
                    }
                } else {
                    name.push_str(c);
                }
            } else if i == 0 {
                // Vowel-leading name: capitalize the vowel
                let v = &style.vowels[rng.gen_range(0..style.vowels.len())];
                let mut chars = v.chars();
                if let Some(first) = chars.next() {
                    name.push(first.to_uppercase().next().unwrap_or(first));
                    name.extend(chars);
                }
                // Add a coda to finish the syllable
                if !style.coda_consonants.is_empty() && rng.gen_bool(0.6) {
                    let c = &style.coda_consonants[rng.gen_range(0..style.coda_consonants.len())];
                    name.push_str(c);
                }
                continue;
            }

            // Vowel nucleus
            if !style.vowels.is_empty() {
                let v = &style.vowels[rng.gen_range(0..style.vowels.len())];
                name.push_str(v);
            }

            // Coda consonant (not always present, especially for flowing styles)
            let coda_chance = if i == syllables - 1 { 0.7 } else { 0.4 };
            if !style.coda_consonants.is_empty() && rng.gen_bool(coda_chance) {
                let c = &style.coda_consonants[rng.gen_range(0..style.coda_consonants.len())];
                name.push_str(c);
            }
        }

        // Ensure name is at least 2 characters
        if name.len() < 2 {
            name.push('a');
        }

        name
    }

    /// Generate a place name (e.g., "Ironhold", "Silverdale", "Bloodmaw").
    ///
    /// Uses compound prefix+suffix pattern with a chance of falling back to
    /// a syllable-based name with a place suffix.
    pub fn place_name(style: &NamingStyle, rng: &mut impl Rng) -> String {
        let use_compound = !style.place_prefixes.is_empty()
            && !style.place_suffixes.is_empty()
            && rng.gen_bool(0.6);

        if use_compound {
            let prefix = &style.place_prefixes[rng.gen_range(0..style.place_prefixes.len())];
            let suffix = &style.place_suffixes[rng.gen_range(0..style.place_suffixes.len())];
            format!("{}{}", prefix, suffix)
        } else {
            // Syllable-based name, optionally with a place suffix
            let base = Self::personal_name(style, rng);
            if !style.place_suffixes.is_empty() && rng.gen_bool(0.5) {
                let suffix = &style.place_suffixes[rng.gen_range(0..style.place_suffixes.len())];
                format!("{}{}", base, suffix)
            } else {
                base
            }
        }
    }

    /// Generate an epithet (e.g., "the Unyielding", "Skullcrusher").
    pub fn epithet(style: &NamingStyle, rng: &mut impl Rng) -> String {
        if style.epithet_patterns.is_empty() {
            return "the Great".to_string();
        }
        let idx = rng.gen_range(0..style.epithet_patterns.len());
        style.epithet_patterns[idx].clone()
    }

    /// Generate a full name with optional epithet (e.g., "Krath the Unyielding").
    pub fn full_name(style: &NamingStyle, rng: &mut impl Rng, include_epithet: bool) -> String {
        let name = Self::personal_name(style, rng);
        if include_epithet {
            let ep = Self::epithet(style, rng);
            format!("{} {}", name, ep)
        } else {
            name
        }
    }

    /// Generate a faction/civilization name (e.g., "The Irondelve Dwarves").
    /// Takes a race name to append.
    pub fn faction_name(style: &NamingStyle, rng: &mut impl Rng, race_name: &str) -> String {
        let place = Self::place_name(style, rng);
        format!("The {} {}", place, race_name)
    }

    /// Generate an artifact name (e.g., "Grimjaw's Wrath", "Starweaver").
    pub fn artifact_name(style: &NamingStyle, rng: &mut impl Rng) -> String {
        let roll: f32 = rng.gen();
        if roll < 0.4 {
            // Named after a concept: single compound word
            Self::place_name(style, rng)
        } else if roll < 0.7 {
            // "The <Epithet-word>"
            let ep = Self::epithet(style, rng);
            // Strip "the " if present to avoid "The the X"
            let clean = ep.strip_prefix("the ").unwrap_or(&ep);
            let mut chars = clean.chars();
            if let Some(first) = chars.next() {
                format!("The {}{}", first.to_uppercase().next().unwrap_or(first), chars.as_str())
            } else {
                format!("The {}", clean)
            }
        } else {
            // "<Name>'s <Suffix>"
            let name = Self::personal_name(style, rng);
            let suffixes = ["Wrath", "Bane", "Fury", "Edge", "Light", "Shadow",
                           "Song", "Crown", "Heart", "Fang"];
            let suffix = suffixes[rng.gen_range(0..suffixes.len())];
            format!("{}'s {}", name, suffix)
        }
    }

    /// Generate a creature epithet (e.g., "the Devourer", "Shadowmaw").
    pub fn creature_epithet(style: &NamingStyle, rng: &mut impl Rng) -> String {
        let creature_epithets = [
            "the Devourer", "the Eternal", "the Ravenous", "the Undying",
            "the Terrible", "the Ancient", "the Dreaded", "the Corrupted",
            "Worldeater", "Flameborn", "Deathbringer", "Plaguemaw",
            "Nightstalker", "Stormbringer", "Doombringer", "Soulreaper",
        ];

        // Mix style epithets with generic creature epithets
        if !style.epithet_patterns.is_empty() && rng.gen_bool(0.4) {
            Self::epithet(style, rng)
        } else {
            creature_epithets[rng.gen_range(0..creature_epithets.len())].to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::NamingStyleId;
    use crate::history::naming::styles::{NamingStyle, NamingArchetype};
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn test_personal_name_not_empty() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        for archetype in NamingArchetype::all() {
            let style = NamingStyle::from_archetype(NamingStyleId(0), *archetype);
            for _ in 0..20 {
                let name = NameGenerator::personal_name(&style, &mut rng);
                assert!(!name.is_empty(), "Empty name for {:?}", archetype);
                assert!(name.len() >= 2, "Name too short: '{}' for {:?}", name, archetype);
                // First character should be uppercase
                assert!(
                    name.chars().next().unwrap().is_uppercase(),
                    "Name '{}' should start with uppercase for {:?}", name, archetype
                );
            }
        }
    }

    #[test]
    fn test_place_name_not_empty() {
        let mut rng = ChaCha8Rng::seed_from_u64(123);
        for archetype in NamingArchetype::all() {
            let style = NamingStyle::from_archetype(NamingStyleId(0), *archetype);
            for _ in 0..10 {
                let name = NameGenerator::place_name(&style, &mut rng);
                assert!(!name.is_empty(), "Empty place name for {:?}", archetype);
            }
        }
    }

    #[test]
    fn test_names_are_varied() {
        let mut rng = ChaCha8Rng::seed_from_u64(99);
        let style = NamingStyle::from_archetype(NamingStyleId(0), NamingArchetype::Harsh);

        let names: Vec<String> = (0..20)
            .map(|_| NameGenerator::personal_name(&style, &mut rng))
            .collect();

        // With 20 names, we should have mostly unique ones
        let unique: std::collections::HashSet<&String> = names.iter().collect();
        assert!(
            unique.len() >= 10,
            "Too few unique names: {} out of 20. Names: {:?}",
            unique.len(), names
        );
    }

    #[test]
    fn test_archetypes_sound_different() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let harsh = NamingStyle::from_archetype(NamingStyleId(0), NamingArchetype::Harsh);
        let flowing = NamingStyle::from_archetype(NamingStyleId(1), NamingArchetype::Flowing);

        // Generate some names and compare average length
        // Flowing names should tend to be longer (more syllables)
        let harsh_names: Vec<String> = (0..50)
            .map(|_| NameGenerator::personal_name(&harsh, &mut rng))
            .collect();
        let flowing_names: Vec<String> = (0..50)
            .map(|_| NameGenerator::personal_name(&flowing, &mut rng))
            .collect();

        let harsh_avg: f32 = harsh_names.iter().map(|n| n.len() as f32).sum::<f32>() / 50.0;
        let flowing_avg: f32 = flowing_names.iter().map(|n| n.len() as f32).sum::<f32>() / 50.0;

        // Flowing (2-4 syllables) should average longer than Harsh (1-3 syllables)
        assert!(
            flowing_avg > harsh_avg,
            "Flowing names (avg {:.1}) should be longer than harsh (avg {:.1})",
            flowing_avg, harsh_avg
        );
    }

    #[test]
    fn test_full_name_with_epithet() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let style = NamingStyle::from_archetype(NamingStyleId(0), NamingArchetype::Harsh);
        let full = NameGenerator::full_name(&style, &mut rng, true);
        assert!(full.contains(' '), "Full name should contain space: '{}'", full);
    }

    #[test]
    fn test_faction_name() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let style = NamingStyle::from_archetype(NamingStyleId(0), NamingArchetype::Harsh);
        let name = NameGenerator::faction_name(&style, &mut rng, "Dwarves");
        assert!(name.starts_with("The "), "Faction name should start with 'The': '{}'", name);
        assert!(name.ends_with("Dwarves"), "Faction name should end with race: '{}'", name);
    }

    #[test]
    fn test_sample_output() {
        // This test prints sample names for manual inspection during development.
        // It always passes but gives visibility into name quality.
        let mut rng = ChaCha8Rng::seed_from_u64(42);

        let archetypes = [
            ("Harsh/Dwarven", NamingArchetype::Harsh),
            ("Flowing/Elven", NamingArchetype::Flowing),
            ("Compound/Human", NamingArchetype::Compound),
            ("Guttural/Orcish", NamingArchetype::Guttural),
            ("Mystical/Fey", NamingArchetype::Mystical),
            ("Sibilant/Reptilian", NamingArchetype::Sibilant),
            ("Ancient/Giant", NamingArchetype::Ancient),
        ];

        for (label, archetype) in &archetypes {
            let style = NamingStyle::from_archetype(NamingStyleId(0), *archetype);
            let names: Vec<String> = (0..5)
                .map(|_| NameGenerator::personal_name(&style, &mut rng))
                .collect();
            let places: Vec<String> = (0..3)
                .map(|_| NameGenerator::place_name(&style, &mut rng))
                .collect();
            eprintln!("  {} names:  {:?}", label, names);
            eprintln!("  {} places: {:?}", label, places);
        }
    }
}
