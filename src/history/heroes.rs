//! Notable people and hero generation
//!
//! Generates historical figures with beliefs, philosophies, and achievements.

use std::collections::HashMap;

use rand::Rng;
use rand_chacha::ChaCha8Rng;
use rand::SeedableRng;

use crate::biomes::ExtendedBiome;
use crate::tilemap::Tilemap;

use super::types::*;
use super::naming::NameGenerator;
use super::factions::FactionRegistry;
use super::timeline::{Timeline, EventType};
use super::monsters::{BiomeCategory, categorize_biome};
use super::territories::TerritoryRegistry;

/// Role of a notable figure in history
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HeroRole {
    Warrior,
    Ruler,
    Scholar,
    Priest,
    Craftsman,
    Explorer,
    Villain,
    General,
}

impl HeroRole {
    pub fn all() -> &'static [HeroRole] {
        &[
            HeroRole::Warrior,
            HeroRole::Ruler,
            HeroRole::Scholar,
            HeroRole::Priest,
            HeroRole::Craftsman,
            HeroRole::Explorer,
            HeroRole::Villain,
            HeroRole::General,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            HeroRole::Warrior => "Warrior",
            HeroRole::Ruler => "Ruler",
            HeroRole::Scholar => "Scholar",
            HeroRole::Priest => "Priest",
            HeroRole::Craftsman => "Craftsman",
            HeroRole::Explorer => "Explorer",
            HeroRole::Villain => "Villain",
            HeroRole::General => "General",
        }
    }

    /// Get the probability weight for this role
    pub fn weight(&self) -> u32 {
        match self {
            HeroRole::Warrior => 25,
            HeroRole::Ruler => 15,
            HeroRole::Scholar => 15,
            HeroRole::Priest => 15,
            HeroRole::Craftsman => 10,
            HeroRole::Explorer => 10,
            HeroRole::Villain => 5,
            HeroRole::General => 15,
        }
    }
}

/// A notable historical figure
#[derive(Clone, Debug)]
pub struct Hero {
    pub id: HeroId,
    pub name: String,
    pub epithet: Option<String>,
    pub species: Species,
    pub faction: FactionId,
    pub role: HeroRole,
    pub birth_year: Year,
    pub death_year: Option<Year>,
    pub death_location: Option<(usize, usize)>,
    pub titles: Vec<String>,
    pub achievements: Vec<EventId>,
    pub artifacts_created: Vec<ArtifactId>,
    pub burial_site: Option<(usize, usize, i32)>,
    pub fame: u32,
    /// Biome category of hero's homeland (for biome-aware epithets)
    pub homeland_biome: Option<BiomeCategory>,

    // Lore content
    pub philosophy: Option<String>,
    pub military_doctrine: Option<String>,
    pub religious_beliefs: Option<String>,
}

impl Hero {
    /// Get a summary of this hero's lore content
    pub fn lore_summary(&self) -> Option<String> {
        if let Some(ref philosophy) = self.philosophy {
            return Some(format!("Philosophy: \"{}\"", philosophy));
        }
        if let Some(ref doctrine) = self.military_doctrine {
            return Some(format!("Doctrine: \"{}\"", doctrine));
        }
        if let Some(ref beliefs) = self.religious_beliefs {
            return Some(format!("Faith: \"{}\"", beliefs));
        }
        None
    }

    /// Check if this hero is alive at a given year
    pub fn alive_at(&self, year: Year) -> bool {
        year >= self.birth_year && self.death_year.map_or(true, |death| year < death)
    }

    /// Get the hero's full name with epithet
    pub fn full_name(&self) -> String {
        if let Some(ref epithet) = self.epithet {
            format!("{} {}", self.name, epithet)
        } else {
            self.name.clone()
        }
    }
}

/// Registry of all heroes
#[derive(Clone, Debug, Default)]
pub struct HeroRegistry {
    pub heroes: HashMap<HeroId, Hero>,
    pub heroes_by_faction: HashMap<FactionId, Vec<HeroId>>,
    next_id: u32,
}

impl HeroRegistry {
    pub fn new() -> Self {
        Self {
            heroes: HashMap::new(),
            heroes_by_faction: HashMap::new(),
            next_id: 0,
        }
    }

    /// Add a hero to the registry
    pub fn add(&mut self, hero: Hero) {
        let id = hero.id;
        let faction = hero.faction;
        self.heroes_by_faction.entry(faction).or_default().push(id);
        self.heroes.insert(id, hero);
    }

    /// Get a hero by ID
    pub fn get(&self, id: HeroId) -> Option<&Hero> {
        self.heroes.get(&id)
    }

    /// Get a mutable reference to a hero by ID
    pub fn get_mut(&mut self, id: HeroId) -> Option<&mut Hero> {
        self.heroes.get_mut(&id)
    }

    /// Generate a new hero ID
    pub fn new_id(&mut self) -> HeroId {
        let id = HeroId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Get all heroes for a faction
    pub fn heroes_of_faction(&self, faction: FactionId) -> Vec<&Hero> {
        self.heroes_by_faction.get(&faction)
            .map(|ids| ids.iter().filter_map(|id| self.heroes.get(id)).collect())
            .unwrap_or_default()
    }

    /// Get heroes alive at a specific year
    pub fn heroes_alive_at(&self, year: Year) -> Vec<&Hero> {
        self.heroes.values().filter(|h| h.alive_at(year)).collect()
    }

    /// Get heroes by role
    pub fn heroes_by_role(&self, role: HeroRole) -> Vec<&Hero> {
        self.heroes.values().filter(|h| h.role == role).collect()
    }

    /// Get all heroes
    pub fn all(&self) -> impl Iterator<Item = &Hero> {
        self.heroes.values()
    }
}

/// Generate heroes for all factions
pub fn generate_heroes(
    factions: &FactionRegistry,
    timeline: &Timeline,
    seed: u64,
) -> HeroRegistry {
    generate_heroes_biome(factions, timeline, None, None, None, seed)
}

/// Generate heroes for all factions with biome-aware features
pub fn generate_heroes_biome(
    factions: &FactionRegistry,
    timeline: &Timeline,
    territories: Option<&TerritoryRegistry>,
    biomes: Option<&Tilemap<ExtendedBiome>>,
    heightmap: Option<&Tilemap<f32>>,
    seed: u64,
) -> HeroRegistry {
    let mut rng = ChaCha8Rng::seed_from_u64(seed.wrapping_add(0xBE501D37));
    let name_gen = NameGenerator::new(seed);
    let mut registry = HeroRegistry::new();

    println!("  Generating heroes...");

    // Generate 2-5 heroes per faction
    for faction in factions.all() {
        let num_heroes = rng.gen_range(2..=5);

        // Determine homeland biome for this faction (based on capital or random)
        let homeland_biome = determine_faction_biome(faction, territories, biomes, &mut rng);

        for _ in 0..num_heroes {
            let id = registry.new_id();
            let role = pick_hero_role(&mut rng);

            // Generate name with potential epithet
            let name = name_gen.hero_first_name(faction.species, &mut rng);

            // Generate biome-aware epithet if possible
            let epithet = if rng.gen_bool(0.7) {
                if let Some(biome) = homeland_biome {
                    // 60% chance of biome-specific epithet
                    if rng.gen_bool(0.6) {
                        Some(name_gen.biome_epithet(role.name(), biome, &mut rng))
                    } else {
                        Some(generate_epithet(role, &mut rng))
                    }
                } else {
                    Some(generate_epithet(role, &mut rng))
                }
            } else {
                None
            };

            // Calculate birth/death years
            let faction_age = faction.founded.age();
            let birth_years_ago = rng.gen_range(100..faction_age.max(200));
            let birth_year = Year::years_ago(birth_years_ago);

            let lifespan = species_lifespan(faction.species, &mut rng);
            let death_years_ago = (birth_years_ago - lifespan).max(0);
            let death_year = if death_years_ago > 0 {
                Some(Year::years_ago(death_years_ago))
            } else {
                None // Still alive
            };

            // Generate lore content based on role
            let (philosophy, military_doctrine, religious_beliefs) = generate_lore_content(
                role,
                faction.species,
                faction.culture,
                &mut rng,
            );

            // Generate titles based on role and culture
            let titles = generate_titles(role, faction.species, faction.culture, &mut rng);

            // Calculate fame based on role and faction status
            let base_fame = match role {
                HeroRole::Ruler => 70,
                HeroRole::General => 65,
                HeroRole::Scholar => 60,
                HeroRole::Priest => 55,
                HeroRole::Craftsman => 50,
                HeroRole::Warrior => 55,
                HeroRole::Explorer => 50,
                HeroRole::Villain => 60,
            };
            let fame = (base_fame + rng.gen_range(0..30)).min(100);

            // Death location and burial site - try to find appropriate biome
            let (death_location, burial_site) = if death_year.is_some() {
                let death_loc = find_suitable_location(role, homeland_biome, biomes, heightmap, &mut rng);
                let burial = if rng.gen_bool(0.7) {
                    find_burial_site(role, homeland_biome, biomes, heightmap, &mut rng)
                } else {
                    None
                };
                (Some(death_loc), burial)
            } else {
                (None, None)
            };

            let hero = Hero {
                id,
                name,
                epithet,
                species: faction.species,
                faction: faction.id,
                role,
                birth_year,
                death_year,
                death_location,
                titles,
                achievements: Vec::new(), // Will be populated later
                artifacts_created: Vec::new(), // Will be populated by artifact generation
                burial_site,
                fame,
                homeland_biome,
                philosophy,
                military_doctrine,
                religious_beliefs,
            };

            registry.add(hero);
        }
    }

    // Link heroes to historical events
    link_heroes_to_events(&mut registry, timeline, &mut rng);

    println!("    {} heroes generated", registry.heroes.len());
    registry
}

/// Determine the dominant biome for a faction based on its capital or settlements
fn determine_faction_biome(
    faction: &super::factions::Faction,
    territories: Option<&TerritoryRegistry>,
    biomes: Option<&Tilemap<ExtendedBiome>>,
    rng: &mut ChaCha8Rng,
) -> Option<BiomeCategory> {
    // If we have territory and biome data, try to find the faction's capital biome
    if let (Some(terr), Some(biome_map)) = (territories, biomes) {
        if let Some(capital_id) = faction.capital {
            if let Some(settlement) = terr.settlements.get(&capital_id) {
                let biome = *biome_map.get(settlement.x, settlement.y);
                return Some(categorize_biome(biome));
            }
        }

        // Otherwise, find any settlement for this faction
        for settlement in terr.settlements.values() {
            if settlement.original_faction == faction.id {
                let biome = *biome_map.get(settlement.x, settlement.y);
                return Some(categorize_biome(biome));
            }
        }
    }

    // Fallback: pick a biome based on species tendencies
    let typical_biomes: &[BiomeCategory] = match faction.species {
        Species::Dwarf => &[BiomeCategory::Mountain, BiomeCategory::Cave, BiomeCategory::Hills],
        Species::Elf => &[BiomeCategory::Forest, BiomeCategory::Mystical],
        Species::Orc => &[BiomeCategory::Grassland, BiomeCategory::Hills, BiomeCategory::Desert],
        Species::Human => &[BiomeCategory::Grassland, BiomeCategory::Forest, BiomeCategory::Coastal],
        Species::Goblin => &[BiomeCategory::Cave, BiomeCategory::Swamp, BiomeCategory::Hills],
        Species::Giant => &[BiomeCategory::Mountain, BiomeCategory::Tundra],
        Species::DragonKin => &[BiomeCategory::Volcanic, BiomeCategory::Mountain],
        Species::Undead => &[BiomeCategory::Ruins, BiomeCategory::Swamp],
        Species::Elemental => &[BiomeCategory::Volcanic, BiomeCategory::Mystical, BiomeCategory::Tundra],
    };

    Some(typical_biomes[rng.gen_range(0..typical_biomes.len())])
}

/// Find a suitable location for a hero event (death, etc.)
fn find_suitable_location(
    role: HeroRole,
    biome_category: Option<BiomeCategory>,
    biomes: Option<&Tilemap<ExtendedBiome>>,
    heightmap: Option<&Tilemap<f32>>,
    rng: &mut ChaCha8Rng,
) -> (usize, usize) {
    let width = biomes.map(|b| b.width).unwrap_or(512);
    let height = biomes.map(|b| b.height).unwrap_or(256);

    // Try to find a location that matches the hero's homeland biome
    if let (Some(biome_map), Some(target_category)) = (biomes, biome_category) {
        for _ in 0..50 {
            let x = rng.gen_range(10..width - 10);
            let y = rng.gen_range(10..height - 10);
            let biome = *biome_map.get(x, y);

            // Check if above water
            if let Some(hmap) = heightmap {
                if *hmap.get(x, y) < 0.0 {
                    continue;
                }
            }

            if categorize_biome(biome) == target_category {
                return (x, y);
            }
        }
    }

    // Fallback: random location on land
    for _ in 0..20 {
        let x = rng.gen_range(10..width - 10);
        let y = rng.gen_range(10..height - 10);
        if let Some(hmap) = heightmap {
            if *hmap.get(x, y) >= 0.0 {
                return (x, y);
            }
        } else {
            return (x, y);
        }
    }

    (rng.gen_range(0..width), rng.gen_range(0..height))
}

/// Find a burial site appropriate for a hero's role and homeland biome
fn find_burial_site(
    role: HeroRole,
    biome_category: Option<BiomeCategory>,
    biomes: Option<&Tilemap<ExtendedBiome>>,
    heightmap: Option<&Tilemap<f32>>,
    rng: &mut ChaCha8Rng,
) -> Option<(usize, usize, i32)> {
    let (x, y) = find_suitable_location(role, biome_category, biomes, heightmap, rng);

    // Determine z-level based on role and biome
    let z = match (role, biome_category) {
        // Rulers and priests get underground tombs
        (HeroRole::Ruler, _) | (HeroRole::Priest, _) => rng.gen_range(-5..-1),
        // Cave dwellers buried deep
        (_, Some(BiomeCategory::Cave)) => rng.gen_range(-8..-3),
        // Mountain folk on high peaks or in mountain tombs
        (_, Some(BiomeCategory::Mountain)) => {
            if rng.gen_bool(0.5) { 0 } else { rng.gen_range(-3..-1) }
        }
        // Volcanic regions - in cooled lava tubes
        (_, Some(BiomeCategory::Volcanic)) => rng.gen_range(-4..-1),
        // Others - surface burial
        _ => 0,
    };

    Some((x, y, z))
}

/// Pick a hero role based on weighted probabilities
fn pick_hero_role(rng: &mut ChaCha8Rng) -> HeroRole {
    let roles = HeroRole::all();
    let total_weight: u32 = roles.iter().map(|r| r.weight()).sum();
    let mut r = rng.gen_range(0..total_weight);

    for role in roles {
        let weight = role.weight();
        if r < weight {
            return *role;
        }
        r -= weight;
    }

    HeroRole::Warrior
}

/// Generate an epithet appropriate for the role
fn generate_epithet(role: HeroRole, rng: &mut ChaCha8Rng) -> String {
    let epithets = match role {
        HeroRole::Warrior => &[
            "the Bold", "the Brave", "Ironhand", "Dragonslayer", "the Fearless",
            "Battleborn", "the Unyielding", "Bloodfist", "the Valiant", "the Mighty",
        ][..],
        HeroRole::Ruler => &[
            "the Great", "the Wise", "the Just", "the Unifier", "the Builder",
            "the Magnificent", "the Peaceful", "the Strong", "the First", "the Last",
        ][..],
        HeroRole::Scholar => &[
            "the Wise", "the Learned", "the Sage", "Stargazer", "Lorekeeper",
            "the Illuminated", "the Philosopher", "Truthseeker", "the Eternal", "the Ancient",
        ][..],
        HeroRole::Priest => &[
            "the Holy", "the Blessed", "the Devout", "the Prophet", "Faithkeeper",
            "the Anointed", "the Radiant", "Soulguard", "the Pure", "the Chosen",
        ][..],
        HeroRole::Craftsman => &[
            "Ironhand", "Masterforge", "the Creator", "Goldfingers", "the Maker",
            "Stoneshaper", "the Artisan", "Flameheart", "the Builder", "Hammerborn",
        ][..],
        HeroRole::Explorer => &[
            "the Wanderer", "Pathfinder", "the Farstrider", "Worldwalker", "the Seeker",
            "Horizonchaser", "the Discoverer", "Starguide", "the Voyager", "the Lost",
        ][..],
        HeroRole::Villain => &[
            "the Terrible", "the Cruel", "the Betrayer", "Darkbringer", "the Mad",
            "Oathbreaker", "the Cursed", "Bloodthirst", "the Tyrant", "the Fallen",
        ][..],
        HeroRole::General => &[
            "the Conqueror", "Battlemaster", "the Strategist", "Warlord", "the Victorious",
            "Shieldbreaker", "the Undefeated", "Ironclad", "the Marshal", "Siegebreaker",
        ][..],
    };

    epithets[rng.gen_range(0..epithets.len())].to_string()
}

/// Generate lore content based on hero role
fn generate_lore_content(
    role: HeroRole,
    species: Species,
    culture: CultureType,
    rng: &mut ChaCha8Rng,
) -> (Option<String>, Option<String>, Option<String>) {
    match role {
        HeroRole::Scholar => {
            let philosophy = Some(generate_philosophy(species, culture, rng));
            (philosophy, None, None)
        }
        HeroRole::General | HeroRole::Warrior => {
            let doctrine = Some(generate_military_doctrine(species, culture, rng));
            (None, doctrine, None)
        }
        HeroRole::Priest => {
            let beliefs = Some(generate_religious_beliefs(species, culture, rng));
            (None, None, beliefs)
        }
        HeroRole::Ruler => {
            // Rulers may have any type of lore
            match rng.gen_range(0..3) {
                0 => (Some(generate_philosophy(species, culture, rng)), None, None),
                1 => (None, Some(generate_military_doctrine(species, culture, rng)), None),
                _ => (None, None, Some(generate_religious_beliefs(species, culture, rng))),
            }
        }
        _ => (None, None, None),
    }
}

/// Generate a philosophy text
fn generate_philosophy(species: Species, culture: CultureType, rng: &mut ChaCha8Rng) -> String {
    let philosophies = match species {
        Species::Dwarf => &[
            "The mountain teaches patience. As stone endures the ages, so must we endure hardship.",
            "True wealth is not gold, but the bonds forged in labor alongside kin.",
            "Honor lives in the craft of one's hands, not in the words of one's mouth.",
            "The deepest vein yields the purest ore, so too does struggle yield wisdom.",
            "A fortress is only as strong as the will of those who defend it.",
        ][..],
        Species::Elf => &[
            "Time is the river that carries all things; we are but leaves upon its current.",
            "In the silence between stars, truth reveals itself to patient hearts.",
            "Beauty is the language through which the eternal speaks to the mortal.",
            "The forest remembers what stone forgets; listen to the whispers of ages.",
            "All things are connected in the great tapestry; harm one thread, and all unravel.",
        ][..],
        Species::Human => &[
            "Greatness is achieved not by those who wait, but by those who act.",
            "A kingdom rises on the backs of its people; honor them, and prosper.",
            "Legacy is the shadow we cast into the future; make it long and bright.",
            "Unity conquers all division; a thousand arrows fly farther than one.",
            "Knowledge is the torch that lights the path through darkest night.",
        ][..],
        Species::Orc => &[
            "Strength is the only law that matters; the weak serve the strong.",
            "Honor is found in battle, glory in victory, shame only in cowardice.",
            "Blood calls to blood; never forget those who stand beside you.",
            "The world belongs to those bold enough to take it.",
            "Fear is the mind-killer; embrace pain and become unstoppable.",
        ][..],
        _ => &[
            "Power flows to those who understand its nature.",
            "Balance is the key to all things; excess leads to destruction.",
            "The past shapes the future; learn from those who came before.",
            "Truth is not found, but forged through experience.",
            "Change is the only constant; adapt or perish.",
        ][..],
    };

    philosophies[rng.gen_range(0..philosophies.len())].to_string()
}

/// Generate a military doctrine
fn generate_military_doctrine(species: Species, culture: CultureType, rng: &mut ChaCha8Rng) -> String {
    let doctrines = match species {
        Species::Dwarf => &[
            "Hold the line. The enemy breaks upon our shields like waves upon stone.",
            "Defense in depth; let them spend their strength against our walls.",
            "Underground, we are kings. Lure them into the tunnels and crush them.",
            "Artillery wins wars; the hammer falls before the sword rises.",
            "Supply lines are blood lines; starve the enemy before engaging.",
        ][..],
        Species::Elf => &[
            "Strike from the shadows, vanish like morning mist.",
            "The forest is our ally; let the trees fight for us.",
            "Patience is the deadliest weapon; wait for the perfect moment.",
            "A single arrow, placed well, defeats a thousand swords.",
            "Never meet strength with strength; flow like water around stone.",
        ][..],
        Species::Human => &[
            "Divide and conquer; an army split is an army defeated.",
            "Speed is armor; strike fast, strike first, strike hard.",
            "Know your enemy better than yourself; intelligence wins wars.",
            "Logistics determine victory; armies march on their stomachs.",
            "Morale is the multiplier; a motivated army fights like ten.",
        ][..],
        Species::Orc => &[
            "Attack! Always attack! Defense is for cowards.",
            "Break their will by breaking their champions; fear spreads faster than blades.",
            "Encircle and annihilate; leave no enemy alive.",
            "The charge wins all; momentum is unstoppable.",
            "Burn everything; leave nothing for retreat.",
        ][..],
        _ => &[
            "Adapt to the enemy; no plan survives first contact.",
            "Control the high ground; position is everything.",
            "Reserve your strength; the final blow wins the battle.",
            "Deception is the commander's greatest tool.",
            "Unity of command; one voice, one will.",
        ][..],
    };

    doctrines[rng.gen_range(0..doctrines.len())].to_string()
}

/// Generate religious beliefs
fn generate_religious_beliefs(species: Species, culture: CultureType, rng: &mut ChaCha8Rng) -> String {
    let beliefs = match species {
        Species::Dwarf => &[
            "The Mountain God forged us from living stone; to Him we return.",
            "Ancestors watch from the halls of the deep; honor them in all deeds.",
            "The Sacred Forge burns eternal; through craft, we worship.",
            "Gold is divine light made solid; treasure it as holy.",
            "The earth shelters the faithful; dig deep and find salvation.",
        ][..],
        Species::Elf => &[
            "The stars are the eyes of the Eternal Ones; they guide our path.",
            "Life flows through all things; the forest is a living prayer.",
            "Death is but transformation; our spirits join the Great Song.",
            "The Moon Mother watches over us; her light cleanses all darkness.",
            "Time is sacred; we are its stewards, not its masters.",
        ][..],
        Species::Human => &[
            "The Light judges all; live righteously and be rewarded.",
            "The gods test us through hardship; emerge stronger.",
            "Sacrifice purifies the soul; give to receive.",
            "The divine speaks through prophets; heed their words.",
            "Heaven awaits the faithful; this life is but preparation.",
        ][..],
        Species::Undead => &[
            "Death is not the end; it is merely transformation.",
            "The Living fear what they do not understand; we are eternal.",
            "Mortality is a prison; undeath is liberation.",
            "The Dark Master grants power to those who serve.",
            "Memory persists beyond flesh; we remember when all else forgets.",
        ][..],
        _ => &[
            "The cosmos is vast; we are small but significant.",
            "Balance must be maintained; light and dark in harmony.",
            "Power flows from the unseen; open your spirit to receive.",
            "The cycle continues; death feeds life feeds death.",
            "Faith shapes reality; believe and it becomes true.",
        ][..],
    };

    beliefs[rng.gen_range(0..beliefs.len())].to_string()
}

/// Generate titles based on role and culture
fn generate_titles(
    role: HeroRole,
    species: Species,
    culture: CultureType,
    rng: &mut ChaCha8Rng,
) -> Vec<String> {
    let mut titles = Vec::new();
    let num_titles = rng.gen_range(1..=3);

    let title_pool: &[&str] = match role {
        HeroRole::Ruler => match species {
            Species::Dwarf => &["High King", "Lord of the Deep", "Master of Halls", "Stone Throne", "Mountain Lord"],
            Species::Elf => &["Star Lord", "Forest Sovereign", "Moon King", "Elder of Ages", "Twilight Regent"],
            Species::Human => &["King", "Emperor", "High Lord", "Sovereign", "Arch-Duke"],
            Species::Orc => &["Warchief", "Blood King", "Skull Throne", "Supreme Warlord", "Conquerer"],
            _ => &["Overlord", "Supreme One", "High Master", "Grand Ruler", "Prime"],
        },
        HeroRole::General => &["Marshal", "High Commander", "Warden", "Battle Lord", "Siege Master"],
        HeroRole::Scholar => &["Grand Sage", "Master of Lore", "High Archivist", "Chief Librarian", "Keeper of Knowledge"],
        HeroRole::Priest => &["High Priest", "Prophet", "Oracle", "Blessed One", "Voice of the Divine"],
        HeroRole::Craftsman => &["Master Smith", "Grand Artisan", "Chief Builder", "Forge Lord", "Master of Craft"],
        HeroRole::Explorer => &["Pathfinder", "Map Lord", "Master Explorer", "Chief Scout", "Realm Walker"],
        HeroRole::Warrior => &["Champion", "First Blade", "Shield Bearer", "Battle Champion", "Sword Saint"],
        HeroRole::Villain => &["Dark Lord", "Betrayer", "Accursed One", "Bane", "Terror"],
    };

    for _ in 0..num_titles {
        if let Some(&title) = title_pool.get(rng.gen_range(0..title_pool.len())) {
            if !titles.contains(&title.to_string()) {
                titles.push(title.to_string());
            }
        }
    }

    titles
}

/// Get typical lifespan for a species
fn species_lifespan(species: Species, rng: &mut ChaCha8Rng) -> i32 {
    let (min, max) = match species {
        Species::Human => (50, 90),
        Species::Dwarf => (150, 350),
        Species::Elf => (500, 1500),
        Species::Orc => (30, 60),
        Species::Goblin => (25, 50),
        Species::Giant => (200, 500),
        Species::DragonKin => (300, 800),
        Species::Undead => (1000, 2000), // Effectively immortal
        Species::Elemental => (500, 1500),
    };
    rng.gen_range(min..=max)
}

/// Link heroes to historical events they participated in
fn link_heroes_to_events(
    registry: &mut HeroRegistry,
    timeline: &Timeline,
    rng: &mut ChaCha8Rng,
) {
    // Collect hero IDs and their faction/role/lifetime info
    let hero_info: Vec<_> = registry.heroes.values().map(|h| {
        (h.id, h.faction, h.role, h.birth_year, h.death_year)
    }).collect();

    for (hero_id, faction, role, birth, death) in hero_info {
        // Find events that this hero could have participated in
        let relevant_events: Vec<EventId> = timeline.events.iter()
            .filter(|(_, event)| {
                // Event must be within hero's lifetime
                let in_lifetime = event.year >= birth && death.map_or(true, |d| event.year < d);
                if !in_lifetime {
                    return false;
                }

                // Event must be relevant to hero's faction or neutral
                let faction_relevant = event.faction == Some(faction) || event.faction.is_none();
                if !faction_relevant {
                    return false;
                }

                // Event type should match hero role
                match role {
                    HeroRole::Warrior | HeroRole::General => {
                        matches!(event.event_type, EventType::Battle | EventType::Siege | EventType::SettlementConquered)
                    }
                    HeroRole::Ruler => {
                        matches!(event.event_type, EventType::SettlementFounded | EventType::AllianceFormed |
                            EventType::MonumentBuilt | EventType::Battle | EventType::SettlementConquered)
                    }
                    HeroRole::Scholar => {
                        matches!(event.event_type, EventType::MonumentBuilt | EventType::GreatDiscovery)
                    }
                    HeroRole::Priest => {
                        matches!(event.event_type, EventType::MonumentBuilt | EventType::ReligionFounded)
                    }
                    HeroRole::Explorer => {
                        matches!(event.event_type, EventType::GreatDiscovery)
                    }
                    _ => rng.gen_bool(0.1),
                }
            })
            .map(|(id, _)| *id)
            .collect();

        // Assign some of these events to the hero
        if !relevant_events.is_empty() {
            let num_events = rng.gen_range(1..=relevant_events.len().min(5));
            let assigned: Vec<EventId> = relevant_events.into_iter()
                .take(num_events)
                .collect();

            if let Some(hero) = registry.get_mut(hero_id) {
                hero.achievements = assigned;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hero_generation() {
        let mut factions = FactionRegistry::new();
        let timeline = Timeline::new();

        // Create a test faction
        use super::super::factions::Faction;
        let faction = Faction {
            id: factions.new_id(),
            name: "Test Kingdom".to_string(),
            species: Species::Human,
            culture: CultureType::Militaristic,
            architecture: ArchitectureStyle::Imperial,
            founded: Year::years_ago(500),
            collapsed: None,
            collapse_reason: None,
            color: (100, 100, 200),
            capital: None,
            peak_settlements: 5,
            peak_population: 10000,
        };
        factions.add(faction);

        let registry = generate_heroes(&factions, &timeline, 42);

        assert!(!registry.heroes.is_empty(), "Should have generated heroes");

        for hero in registry.all() {
            println!("{}: {} ({:?})", hero.full_name(), hero.role.name(), hero.species);
            if let Some(summary) = hero.lore_summary() {
                println!("  {}", summary);
            }
        }
    }
}
