//! Creature species generation.
//!
//! Generates procedural creature species with coherent anatomy,
//! size, intelligence, and biome-appropriate traits.

use rand::Rng;
use serde::{Serialize, Deserialize};
use crate::biomes::ExtendedBiome;
use crate::history::CreatureSpeciesId;
use super::anatomy::*;
use super::behavior::CreatureBehavior;

/// A complete creature species definition.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreatureSpecies {
    pub id: CreatureSpeciesId,
    pub name: String,
    pub description: String,

    // Anatomy
    pub body_parts: Vec<BodyPart>,
    pub size: CreatureSize,
    pub locomotion: Vec<Locomotion>,

    // Mind
    pub intelligence: Intelligence,
    pub behavior: CreatureBehavior,

    // Combat
    pub attacks: Vec<AttackType>,
    pub defenses: Vec<DefenseType>,
    pub immunities: Vec<DamageType>,
    pub vulnerabilities: Vec<DamageType>,

    // Ecology
    pub habitat: Vec<ExtendedBiome>,
    pub diet: Diet,
    pub can_lead_population: bool,
    pub population_role: PopulationRole,

    // Magic
    pub magical_abilities: Vec<MagicAbility>,
    pub is_magical_origin: bool,
}

/// Biome category for creature template selection.
#[derive(Clone, Copy, Debug)]
enum BiomeCategory {
    Forest,
    Mountain,
    Desert,
    Swamp,
    Tundra,
    Ocean,
    Volcanic,
    Magical,
    Underground,
    Grassland,
}

impl CreatureSpecies {
    /// Generate a random creature species appropriate for a given biome.
    pub fn generate(
        id: CreatureSpeciesId,
        primary_biome: ExtendedBiome,
        rng: &mut impl Rng,
    ) -> Self {
        let category = categorize_biome(primary_biome);
        let size = CreatureSize::random_weighted(rng);
        let intelligence = random_intelligence(rng);
        let behavior = CreatureBehavior::generate(size, intelligence, rng);

        let body_parts = generate_body_plan(category, size, rng);
        let locomotion = derive_locomotion(&body_parts, category);
        let attacks = derive_attacks(&body_parts, size);
        let defenses = derive_defenses(&body_parts);
        let (immunities, vulnerabilities) = derive_resistances(category, &body_parts, rng);
        let diet = derive_diet(size, intelligence, category, rng);

        let is_magical = matches!(category, BiomeCategory::Magical) || rng.gen_bool(0.15);
        let magical_abilities = if is_magical {
            generate_magic_abilities(intelligence, rng)
        } else {
            Vec::new()
        };

        let can_lead = intelligence.can_lead() && size >= CreatureSize::Medium;
        let population_role = if size >= CreatureSize::Huge {
            PopulationRole::Solitary
        } else if behavior.pack_tendency > 0.6 {
            PopulationRole::PackMember
        } else {
            PopulationRole::Solitary
        };

        let habitat = expand_habitat(primary_biome);
        let name = generate_species_name(category, size, &body_parts, rng);
        let description = generate_description(&body_parts, size, intelligence, &locomotion);

        Self {
            id,
            name,
            description,
            body_parts,
            size,
            locomotion,
            intelligence,
            behavior,
            attacks,
            defenses,
            immunities,
            vulnerabilities,
            habitat,
            diet,
            can_lead_population: can_lead,
            population_role,
            magical_abilities,
            is_magical_origin: is_magical,
        }
    }
}

fn categorize_biome(biome: ExtendedBiome) -> BiomeCategory {
    match biome {
        ExtendedBiome::TemperateForest | ExtendedBiome::TemperateRainforest |
        ExtendedBiome::BorealForest | ExtendedBiome::TropicalForest |
        ExtendedBiome::TropicalRainforest | ExtendedBiome::MontaneForest |
        ExtendedBiome::CloudForest | ExtendedBiome::SubalpineForest |
        ExtendedBiome::DeadForest | ExtendedBiome::PetrifiedForest |
        ExtendedBiome::AncientGrove => BiomeCategory::Forest,

        ExtendedBiome::AlpineTundra | ExtendedBiome::SnowyPeaks |
        ExtendedBiome::AlpineMeadow | ExtendedBiome::RazorPeaks |
        ExtendedBiome::Foothills | ExtendedBiome::Paramo |
        ExtendedBiome::BasaltColumns => BiomeCategory::Mountain,

        ExtendedBiome::Desert | ExtendedBiome::SaltFlats |
        ExtendedBiome::SingingDunes | ExtendedBiome::GlassDesert |
        ExtendedBiome::CrystalWasteland => BiomeCategory::Desert,

        ExtendedBiome::Swamp | ExtendedBiome::Marsh | ExtendedBiome::Bog |
        ExtendedBiome::MangroveSaltmarsh | ExtendedBiome::Shadowfen |
        ExtendedBiome::CarnivorousBog | ExtendedBiome::SpiritMarsh => BiomeCategory::Swamp,

        ExtendedBiome::Tundra | ExtendedBiome::Ice => BiomeCategory::Tundra,

        ExtendedBiome::DeepOcean | ExtendedBiome::Ocean |
        ExtendedBiome::CoastalWater | ExtendedBiome::Lagoon |
        ExtendedBiome::AbyssalVents | ExtendedBiome::Sargasso |
        ExtendedBiome::KelpTowers | ExtendedBiome::InkSea |
        ExtendedBiome::PhosphorShallows => BiomeCategory::Ocean,

        ExtendedBiome::VolcanicWasteland | ExtendedBiome::Ashlands |
        ExtendedBiome::ObsidianFields | ExtendedBiome::SulfurVents |
        ExtendedBiome::Geysers | ExtendedBiome::LavaLake => BiomeCategory::Volcanic,

        ExtendedBiome::CrystalForest | ExtendedBiome::BioluminescentForest |
        ExtendedBiome::MushroomForest | ExtendedBiome::EtherealMist |
        ExtendedBiome::LeyNexus | ExtendedBiome::FloatingStones |
        ExtendedBiome::PrismaticPools | ExtendedBiome::AuroraWastes |
        ExtendedBiome::StarfallCrater | ExtendedBiome::WhisperingStones |
        ExtendedBiome::FungalBloom | ExtendedBiome::BioluminescentWater |
        ExtendedBiome::ColossalHive => BiomeCategory::Magical,

        ExtendedBiome::TarPits | ExtendedBiome::SinkholeLakes => BiomeCategory::Underground,

        _ => BiomeCategory::Grassland,
    }
}

fn random_intelligence(rng: &mut impl Rng) -> Intelligence {
    let roll: f32 = rng.gen();
    if roll < 0.20 { Intelligence::Mindless }
    else if roll < 0.50 { Intelligence::Instinctual }
    else if roll < 0.75 { Intelligence::Cunning }
    else if roll < 0.92 { Intelligence::Sapient }
    else { Intelligence::Genius }
}

fn generate_body_plan(category: BiomeCategory, size: CreatureSize, rng: &mut impl Rng) -> Vec<BodyPart> {
    let mut parts = Vec::new();
    let material = pick_material(category, rng);

    // Base body plan
    let plan: u8 = rng.gen_range(0..6);
    match plan {
        0 => {
            // Quadruped beast
            parts.push(BodyPart::new(BodyPartType::Head, 1, material));
            parts.push(BodyPart::new(BodyPartType::Torso, 1, material));
            parts.push(BodyPart::new(BodyPartType::Legs, 4, material));
            if rng.gen_bool(0.3) {
                parts.push(BodyPart::new(BodyPartType::Tail, 1, material));
            }
        }
        1 => {
            // Serpentine
            let heads = if rng.gen_bool(0.1) { rng.gen_range(2..=5) } else { 1 };
            parts.push(BodyPart::new(BodyPartType::Head, heads, material));
            parts.push(BodyPart::new(BodyPartType::Torso, 1, material).with_size(BodyPartSize::Large));
            parts.push(BodyPart::new(BodyPartType::Tail, 1, material));
        }
        2 => {
            // Insectoid
            parts.push(BodyPart::new(BodyPartType::Head, 1, BodyMaterial::Chitin));
            parts.push(BodyPart::new(BodyPartType::Torso, 1, BodyMaterial::Chitin));
            parts.push(BodyPart::new(BodyPartType::Legs, rng.gen_range(4..=8), BodyMaterial::Chitin));
            if rng.gen_bool(0.4) {
                parts.push(BodyPart::new(BodyPartType::Wings, 2, BodyMaterial::Chitin));
            }
            if rng.gen_bool(0.5) {
                parts.push(BodyPart::new(BodyPartType::Mandibles, 1, BodyMaterial::Chitin)
                    .with_special(BodyPartSpecial::Venomous));
            }
        }
        3 => {
            // Winged (dragon-like)
            parts.push(BodyPart::new(BodyPartType::Head, 1, material));
            parts.push(BodyPart::new(BodyPartType::Torso, 1, material));
            parts.push(BodyPart::new(BodyPartType::Legs, rng.gen_range(2..=4), material));
            parts.push(BodyPart::new(BodyPartType::Wings, 2, material));
            parts.push(BodyPart::new(BodyPartType::Tail, 1, material));
            if size >= CreatureSize::Large && rng.gen_bool(0.4) {
                parts.push(BodyPart::new(BodyPartType::Horns, rng.gen_range(1..=3), material));
            }
        }
        4 => {
            // Tentacled aberration
            parts.push(BodyPart::new(BodyPartType::Head, 1, material)
                .with_size(if rng.gen_bool(0.3) { BodyPartSize::Large } else { BodyPartSize::Normal }));
            parts.push(BodyPart::new(BodyPartType::Torso, 1, material));
            parts.push(BodyPart::new(BodyPartType::Tentacles, rng.gen_range(4..=12), material)
                .with_special(BodyPartSpecial::Grasping));
            parts.push(BodyPart::new(BodyPartType::Eyes, rng.gen_range(1..=8), material));
        }
        _ => {
            // Amorphous blob
            parts.push(BodyPart::new(BodyPartType::Torso, 1, BodyMaterial::Ooze)
                .with_size(BodyPartSize::Large)
                .with_special(BodyPartSpecial::Regenerating));
            if rng.gen_bool(0.3) {
                parts.push(BodyPart::new(BodyPartType::Eyes, rng.gen_range(0..=3), BodyMaterial::Ooze));
            }
            if rng.gen_bool(0.4) {
                parts.push(BodyPart::new(BodyPartType::Tentacles, rng.gen_range(2..=6), BodyMaterial::Ooze));
            }
        }
    }

    // Category-specific additions
    match category {
        BiomeCategory::Volcanic => {
            if let Some(head) = parts.iter_mut().find(|p| p.part_type == BodyPartType::Head) {
                if rng.gen_bool(0.5) {
                    head.specials.push(BodyPartSpecial::FireBreathing);
                }
            }
        }
        BiomeCategory::Tundra => {
            if let Some(head) = parts.iter_mut().find(|p| p.part_type == BodyPartType::Head) {
                if rng.gen_bool(0.4) {
                    head.specials.push(BodyPartSpecial::IceBreathing);
                }
            }
        }
        BiomeCategory::Swamp => {
            for part in parts.iter_mut() {
                if rng.gen_bool(0.3) {
                    part.specials.push(BodyPartSpecial::Venomous);
                    break;
                }
            }
        }
        BiomeCategory::Magical => {
            for part in parts.iter_mut() {
                if rng.gen_bool(0.3) {
                    part.specials.push(BodyPartSpecial::Magical);
                    break;
                }
            }
            if rng.gen_bool(0.3) {
                if let Some(body) = parts.iter_mut().find(|p| p.part_type == BodyPartType::Torso) {
                    body.specials.push(BodyPartSpecial::Bioluminescent);
                }
            }
        }
        BiomeCategory::Ocean => {
            parts.push(BodyPart::new(BodyPartType::Fins, rng.gen_range(2..=4), material));
        }
        _ => {}
    }

    // Scale parts for size
    if size >= CreatureSize::Gargantuan {
        for part in parts.iter_mut() {
            if part.size == BodyPartSize::Normal {
                part.size = BodyPartSize::Large;
            }
        }
    }

    parts
}

fn pick_material(category: BiomeCategory, rng: &mut impl Rng) -> BodyMaterial {
    let options: &[BodyMaterial] = match category {
        BiomeCategory::Forest => &[BodyMaterial::Flesh, BodyMaterial::Scales, BodyMaterial::Feathers],
        BiomeCategory::Mountain => &[BodyMaterial::Stone, BodyMaterial::Scales, BodyMaterial::Flesh],
        BiomeCategory::Desert => &[BodyMaterial::Chitin, BodyMaterial::Scales, BodyMaterial::Bone],
        BiomeCategory::Swamp => &[BodyMaterial::Flesh, BodyMaterial::Ooze, BodyMaterial::Scales],
        BiomeCategory::Tundra => &[BodyMaterial::Flesh, BodyMaterial::Ice, BodyMaterial::Feathers],
        BiomeCategory::Ocean => &[BodyMaterial::Scales, BodyMaterial::Flesh, BodyMaterial::Chitin],
        BiomeCategory::Volcanic => &[BodyMaterial::Stone, BodyMaterial::Metal, BodyMaterial::Flame],
        BiomeCategory::Magical => &[BodyMaterial::Crystal, BodyMaterial::Shadow, BodyMaterial::Flame, BodyMaterial::Ooze],
        BiomeCategory::Underground => &[BodyMaterial::Stone, BodyMaterial::Chitin, BodyMaterial::Fungal],
        BiomeCategory::Grassland => &[BodyMaterial::Flesh, BodyMaterial::Scales, BodyMaterial::Feathers],
    };
    options[rng.gen_range(0..options.len())]
}

fn derive_locomotion(parts: &[BodyPart], category: BiomeCategory) -> Vec<Locomotion> {
    let mut loco = Vec::new();
    let has_wings = parts.iter().any(|p| p.part_type == BodyPartType::Wings);
    let has_legs = parts.iter().any(|p| p.part_type == BodyPartType::Legs);
    let has_fins = parts.iter().any(|p| p.part_type == BodyPartType::Fins);
    let has_tentacles = parts.iter().any(|p| p.part_type == BodyPartType::Tentacles);

    if has_legs { loco.push(Locomotion::Walking); }
    if has_wings { loco.push(Locomotion::Flying); }
    if has_fins { loco.push(Locomotion::Swimming); }
    if has_tentacles {
        loco.push(Locomotion::Climbing);
        if matches!(category, BiomeCategory::Ocean) {
            loco.push(Locomotion::Swimming);
        }
    }
    if !has_legs && !has_wings && !has_fins && !has_tentacles {
        loco.push(Locomotion::Slithering);
    }
    if matches!(category, BiomeCategory::Underground) {
        loco.push(Locomotion::Burrowing);
    }
    loco.dedup();
    loco
}

fn derive_attacks(parts: &[BodyPart], size: CreatureSize) -> Vec<AttackType> {
    let mut attacks = Vec::new();
    for part in parts {
        match part.part_type {
            BodyPartType::Head | BodyPartType::Mouth => {
                attacks.push(AttackType::Bite);
                if size >= CreatureSize::Huge {
                    attacks.push(AttackType::Swallow);
                }
                if part.specials.contains(&BodyPartSpecial::FireBreathing) {
                    attacks.push(AttackType::BreathWeapon);
                }
                if part.specials.contains(&BodyPartSpecial::IceBreathing) {
                    attacks.push(AttackType::BreathWeapon);
                }
            }
            BodyPartType::Arms => { attacks.push(AttackType::Claw); }
            BodyPartType::Mandibles => { attacks.push(AttackType::Bite); }
            BodyPartType::Horns => { attacks.push(AttackType::Gore); }
            BodyPartType::Tail => { attacks.push(AttackType::TailSwipe); }
            BodyPartType::Tentacles => { attacks.push(AttackType::Constrict); }
            BodyPartType::Eyes if part.specials.contains(&BodyPartSpecial::Magical) => {
                attacks.push(AttackType::Gaze);
            }
            _ => {}
        }
        if part.specials.contains(&BodyPartSpecial::Venomous) {
            attacks.push(AttackType::Sting);
        }
        if part.specials.contains(&BodyPartSpecial::Acidic) {
            attacks.push(AttackType::Spit);
        }
    }
    if size >= CreatureSize::Huge {
        attacks.push(AttackType::Trample);
    }
    attacks.sort_by_key(|a| *a as u8);
    attacks.dedup();
    attacks
}

fn derive_defenses(parts: &[BodyPart]) -> Vec<DefenseType> {
    let mut defenses = Vec::new();
    for part in parts {
        match part.material {
            BodyMaterial::Chitin | BodyMaterial::Stone | BodyMaterial::Metal => {
                if !defenses.contains(&DefenseType::Shell) {
                    defenses.push(DefenseType::Shell);
                }
            }
            BodyMaterial::Scales => {
                if !defenses.contains(&DefenseType::Scales) {
                    defenses.push(DefenseType::Scales);
                }
            }
            _ => {}
        }
        if part.specials.contains(&BodyPartSpecial::Regenerating) {
            if !defenses.contains(&DefenseType::Regeneration) {
                defenses.push(DefenseType::Regeneration);
            }
        }
        if part.specials.contains(&BodyPartSpecial::Camouflaged) {
            if !defenses.contains(&DefenseType::Camouflage) {
                defenses.push(DefenseType::Camouflage);
            }
        }
        if part.specials.contains(&BodyPartSpecial::Armored) {
            if !defenses.contains(&DefenseType::ThickHide) {
                defenses.push(DefenseType::ThickHide);
            }
        }
    }
    defenses
}

fn derive_resistances(
    category: BiomeCategory,
    parts: &[BodyPart],
    rng: &mut impl Rng,
) -> (Vec<DamageType>, Vec<DamageType>) {
    let mut immunities = Vec::new();
    let mut vulnerabilities = Vec::new();

    match category {
        BiomeCategory::Volcanic => {
            immunities.push(DamageType::Fire);
            if rng.gen_bool(0.5) { vulnerabilities.push(DamageType::Ice); }
        }
        BiomeCategory::Tundra => {
            immunities.push(DamageType::Ice);
            if rng.gen_bool(0.5) { vulnerabilities.push(DamageType::Fire); }
        }
        BiomeCategory::Magical => {
            if rng.gen_bool(0.3) { immunities.push(DamageType::Magic); }
        }
        _ => {}
    }

    let has_venom = parts.iter().any(|p| p.specials.contains(&BodyPartSpecial::Venomous));
    if has_venom {
        immunities.push(DamageType::Poison);
    }

    let material_is_stone = parts.iter().any(|p| p.material == BodyMaterial::Stone);
    if material_is_stone {
        immunities.push(DamageType::Poison);
        vulnerabilities.push(DamageType::Lightning);
    }

    (immunities, vulnerabilities)
}

fn derive_diet(size: CreatureSize, intelligence: Intelligence, category: BiomeCategory, rng: &mut impl Rng) -> Diet {
    if matches!(category, BiomeCategory::Magical) && rng.gen_bool(0.3) {
        return Diet::MagicDrainer;
    }
    match intelligence {
        Intelligence::Mindless => {
            if rng.gen_bool(0.5) { Diet::Absorber } else { Diet::Scavenger }
        }
        _ => match size {
            CreatureSize::Tiny | CreatureSize::Small => {
                *[Diet::Omnivore, Diet::Herbivore, Diet::Scavenger]
                    .get(rng.gen_range(0..3)).unwrap_or(&Diet::Omnivore)
            }
            CreatureSize::Medium | CreatureSize::Large => {
                if rng.gen_bool(0.6) { Diet::Carnivore } else { Diet::Omnivore }
            }
            _ => Diet::Carnivore,
        }
    }
}

fn generate_magic_abilities(intelligence: Intelligence, rng: &mut impl Rng) -> Vec<MagicAbility> {
    let max = match intelligence {
        Intelligence::Mindless => 0,
        Intelligence::Instinctual => 1,
        Intelligence::Cunning => 1,
        Intelligence::Sapient => 2,
        Intelligence::Genius => 3,
    };
    if max == 0 { return Vec::new(); }

    let all = [
        MagicAbility::Spellcasting, MagicAbility::Illusions, MagicAbility::Shapeshifting,
        MagicAbility::Teleportation, MagicAbility::MindControl, MagicAbility::Necromancy,
        MagicAbility::ElementalControl, MagicAbility::CurseWeaving, MagicAbility::HealingAura,
    ];
    let count = rng.gen_range(1..=max);
    let mut abilities = Vec::new();
    for _ in 0..count {
        let a = all[rng.gen_range(0..all.len())].clone();
        if !abilities.contains(&a) {
            abilities.push(a);
        }
    }
    abilities
}

fn expand_habitat(primary: ExtendedBiome) -> Vec<ExtendedBiome> {
    vec![primary]
}

fn generate_species_name(
    category: BiomeCategory,
    size: CreatureSize,
    parts: &[BodyPart],
    rng: &mut impl Rng,
) -> String {
    let prefixes: &[&str] = match category {
        BiomeCategory::Forest => &["Shadow", "Timber", "Thorn", "Moss", "Bark", "Grove"],
        BiomeCategory::Mountain => &["Stone", "Crag", "Peak", "Iron", "Granite", "Ridge"],
        BiomeCategory::Desert => &["Sand", "Dust", "Sun", "Dune", "Scorch", "Dry"],
        BiomeCategory::Swamp => &["Mire", "Bog", "Murk", "Rot", "Slime", "Fen"],
        BiomeCategory::Tundra => &["Frost", "Ice", "Snow", "Pale", "Bitter", "White"],
        BiomeCategory::Ocean => &["Deep", "Tide", "Storm", "Brine", "Coral", "Abyss"],
        BiomeCategory::Volcanic => &["Ash", "Ember", "Magma", "Cinder", "Smolder", "Char"],
        BiomeCategory::Magical => &["Arcane", "Ether", "Void", "Shimmer", "Crystal", "Phase"],
        BiomeCategory::Underground => &["Cave", "Tunnel", "Deep", "Dark", "Blind", "Root"],
        BiomeCategory::Grassland => &["Plains", "Prairie", "Steppe", "Wild", "Wind", "Golden"],
    };

    let has_wings = parts.iter().any(|p| p.part_type == BodyPartType::Wings);
    let has_tentacles = parts.iter().any(|p| p.part_type == BodyPartType::Tentacles);
    let is_serpentine = !parts.iter().any(|p| p.part_type == BodyPartType::Legs)
        && parts.iter().any(|p| p.part_type == BodyPartType::Tail);

    let suffixes: &[&str] = if size >= CreatureSize::Gargantuan {
        if has_wings { &["Dragon", "Wyrm", "Drake"] }
        else if has_tentacles { &["Leviathan", "Kraken", "Horror"] }
        else { &["Titan", "Colossus", "Behemoth"] }
    } else if has_wings {
        &["Hawk", "Raptor", "Wyvern", "Bat", "Moth"]
    } else if has_tentacles {
        &["Lurker", "Crawler", "Horror", "Grasp"]
    } else if is_serpentine {
        &["Serpent", "Wyrm", "Adder", "Naga"]
    } else {
        &["Beast", "Prowler", "Stalker", "Maw", "Hunter", "Fiend"]
    };

    let prefix = prefixes[rng.gen_range(0..prefixes.len())];
    let suffix = suffixes[rng.gen_range(0..suffixes.len())];
    format!("{}{}", prefix, suffix.to_lowercase())
}

fn generate_description(
    parts: &[BodyPart],
    size: CreatureSize,
    intelligence: Intelligence,
    locomotion: &[Locomotion],
) -> String {
    let size_desc = size.label();
    let material = parts.first().map(|p| format!("{:?}", p.material).to_lowercase()).unwrap_or_default();

    let legs = parts.iter().find(|p| p.part_type == BodyPartType::Legs).map(|p| p.count);
    let heads = parts.iter().find(|p| p.part_type == BodyPartType::Head).map(|p| p.count).unwrap_or(0);
    let has_wings = parts.iter().any(|p| p.part_type == BodyPartType::Wings);
    let has_tentacles = parts.iter().any(|p| p.part_type == BodyPartType::Tentacles);

    let mut desc = format!("A {} creature", size_desc);

    if let Some(leg_count) = legs {
        desc.push_str(&format!(" with {} {}-covered legs", leg_count, material));
    }

    if heads > 1 {
        desc.push_str(&format!(" and {} heads", heads));
    }

    if has_wings {
        desc.push_str(", capable of flight");
    }

    if has_tentacles {
        let tent_count = parts.iter()
            .find(|p| p.part_type == BodyPartType::Tentacles)
            .map(|p| p.count).unwrap_or(0);
        desc.push_str(&format!(" with {} grasping tentacles", tent_count));
    }

    let specials: Vec<&str> = parts.iter()
        .flat_map(|p| p.specials.iter())
        .map(|s| match s {
            BodyPartSpecial::Venomous => "venomous",
            BodyPartSpecial::FireBreathing => "fire-breathing",
            BodyPartSpecial::IceBreathing => "ice-breathing",
            BodyPartSpecial::Regenerating => "regenerating",
            BodyPartSpecial::Bioluminescent => "bioluminescent",
            BodyPartSpecial::Magical => "magical",
            _ => "",
        })
        .filter(|s| !s.is_empty())
        .collect();

    if !specials.is_empty() {
        let unique: Vec<&str> = {
            let mut v = specials;
            v.dedup();
            v
        };
        desc.push_str(&format!(". It is {}", unique.join(", ")));
    }

    desc.push_str(&format!(". Intelligence: {:?}", intelligence));
    desc.push('.');
    desc
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn test_generate_creature() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let species = CreatureSpecies::generate(
            CreatureSpeciesId(0),
            ExtendedBiome::TemperateForest,
            &mut rng,
        );
        assert!(!species.name.is_empty());
        assert!(!species.body_parts.is_empty());
        assert!(!species.locomotion.is_empty());
        assert!(!species.attacks.is_empty());
        assert!(!species.habitat.is_empty());
    }

    #[test]
    fn test_generate_various_biomes() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let biomes = [
            ExtendedBiome::TemperateForest, ExtendedBiome::Desert,
            ExtendedBiome::DeepOcean, ExtendedBiome::VolcanicWasteland,
            ExtendedBiome::Tundra, ExtendedBiome::Swamp,
            ExtendedBiome::CrystalForest, ExtendedBiome::SnowyPeaks,
        ];
        for biome in &biomes {
            let species = CreatureSpecies::generate(CreatureSpeciesId(0), *biome, &mut rng);
            assert!(!species.name.is_empty(), "Empty name for {:?}", biome);
            assert!(!species.body_parts.is_empty(), "No body parts for {:?}", biome);
        }
    }

    #[test]
    fn test_sample_creatures() {
        let mut rng = ChaCha8Rng::seed_from_u64(99);
        for i in 0..10 {
            let biome = [
                ExtendedBiome::TemperateForest, ExtendedBiome::Desert,
                ExtendedBiome::DeepOcean, ExtendedBiome::VolcanicWasteland,
                ExtendedBiome::CrystalForest, ExtendedBiome::Tundra,
                ExtendedBiome::Swamp, ExtendedBiome::SnowyPeaks,
                ExtendedBiome::MushroomForest, ExtendedBiome::Savanna,
            ][i];
            let species = CreatureSpecies::generate(CreatureSpeciesId(i as u64), biome, &mut rng);
            eprintln!("  [{:?}] {} ({}, {:?}) - {}",
                biome, species.name, species.size.label(),
                species.intelligence, species.description);
        }
    }
}
