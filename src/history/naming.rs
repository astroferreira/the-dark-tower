//! Procedural naming system for factions, settlements, and artifacts
//!
//! Generates culturally-appropriate names based on species and culture type.

use rand::Rng;
use rand_chacha::ChaCha8Rng;

use super::types::{Species, CultureType};
use super::monsters::BiomeCategory;

/// Word banks for procedural name generation
pub struct NameGenerator {
    seed: u64,
}

impl NameGenerator {
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }

    /// Generate a faction name
    pub fn faction_name(&self, species: Species, culture: CultureType, rng: &mut ChaCha8Rng) -> String {
        let prefix = self.faction_prefix(species, culture, rng);
        let suffix = self.faction_suffix(species, culture, rng);
        format!("{} {}", prefix, suffix)
    }

    /// Generate a settlement name
    pub fn settlement_name(&self, species: Species, rng: &mut ChaCha8Rng) -> String {
        let base = self.place_root(species, rng);
        let suffix = self.place_suffix(species, rng);
        format!("{}{}", base, suffix)
    }

    /// Generate a landmark name (mountain, river, etc.)
    pub fn landmark_name(&self, landmark_type: &str, species: Species, rng: &mut ChaCha8Rng) -> String {
        let adjective = self.nature_adjective(species, rng);
        let noun = match landmark_type {
            "mountain" => self.mountain_noun(species, rng),
            "river" => self.river_noun(species, rng),
            "forest" => self.forest_noun(species, rng),
            "lake" => self.lake_noun(species, rng),
            _ => landmark_type.to_string(),
        };
        format!("{} {}", adjective, noun)
    }

    /// Generate an artifact name
    pub fn artifact_name(&self, species: Species, rng: &mut ChaCha8Rng) -> String {
        let prefix = self.artifact_prefix(species, rng);
        let noun = self.artifact_noun(species, rng);
        format!("{} of {}", noun, prefix)
    }

    /// Generate a personal name (for historical figures)
    pub fn personal_name(&self, species: Species, rng: &mut ChaCha8Rng) -> String {
        let first = self.first_name(species, rng);
        if rng.gen_bool(0.6) {
            let epithet = self.epithet(species, rng);
            format!("{} {}", first, epithet)
        } else {
            first
        }
    }

    /// Generate a battle name
    pub fn battle_name(&self, location: &str, rng: &mut ChaCha8Rng) -> String {
        let prefix = pick(rng, &["Battle of", "Siege of", "Sack of", "Fall of", "Defense of"]);
        format!("{} {}", prefix, location)
    }

    /// Generate an era name
    pub fn era_name(&self, rng: &mut ChaCha8Rng) -> String {
        let adjective = pick(rng, &[
            "Golden", "Dark", "Iron", "Silver", "Bronze",
            "Crimson", "Twilight", "Dawn", "Shadow", "Crystal",
        ]);
        let noun = pick(rng, &[
            "Age", "Era", "Epoch", "Period", "Time",
        ]);
        format!("The {} {}", adjective, noun)
    }

    // === Private helper methods ===

    fn faction_prefix(&self, species: Species, culture: CultureType, rng: &mut ChaCha8Rng) -> String {
        match species {
            Species::Human => pick(rng, &[
                "The", "Great", "Holy", "Royal", "Imperial",
                "Ancient", "Noble", "Glorious", "Eternal", "United",
            ]).to_string(),
            Species::Dwarf => pick(rng, &[
                "Iron", "Stone", "Deep", "Mountain", "Forge",
                "Gold", "Silver", "Hammer", "Anvil", "Granite",
            ]).to_string(),
            Species::Elf => pick(rng, &[
                "Star", "Moon", "Sun", "Silver", "Golden",
                "Ancient", "Eternal", "Twilight", "Dawn", "Verdant",
            ]).to_string(),
            Species::Orc => pick(rng, &[
                "Blood", "Iron", "Bone", "Skull", "War",
                "Rage", "Storm", "Thunder", "Fury", "Dark",
            ]).to_string(),
            Species::Goblin => pick(rng, &[
                "Shadow", "Night", "Dark", "Cunning", "Swift",
                "Sharp", "Sly", "Twisted", "Hidden", "Cave",
            ]).to_string(),
            Species::Giant => pick(rng, &[
                "Mighty", "Towering", "Storm", "Thunder", "Mountain",
                "Sky", "Ancient", "Titan", "Colossal", "Prime",
            ]).to_string(),
            Species::DragonKin => pick(rng, &[
                "Flame", "Scale", "Wing", "Claw", "Fire",
                "Ember", "Ash", "Smoke", "Burning", "Dragon",
            ]).to_string(),
            Species::Undead => pick(rng, &[
                "Eternal", "Deathless", "Shadow", "Pale", "Hollow",
                "Bone", "Grave", "Tomb", "Lich", "Phantom",
            ]).to_string(),
            Species::Elemental => pick(rng, &[
                "Primal", "Eternal", "Pure", "Elemental", "Raw",
                "Storm", "Flame", "Frost", "Stone", "Spirit",
            ]).to_string(),
        }
    }

    fn faction_suffix(&self, species: Species, culture: CultureType, rng: &mut ChaCha8Rng) -> String {
        match culture {
            CultureType::Militaristic => pick(rng, &[
                "Legion", "Horde", "Army", "Warband", "Host",
                "Guard", "Warriors", "Vanguard", "Legion", "Force",
            ]).to_string(),
            CultureType::Mercantile => pick(rng, &[
                "Consortium", "Guild", "Company", "Trading House", "Merchants",
                "Exchange", "Cartel", "League", "Union", "Coalition",
            ]).to_string(),
            CultureType::Scholarly => pick(rng, &[
                "Academy", "Order", "Circle", "Conclave", "Institute",
                "Library", "Archive", "Council", "Keepers", "Seekers",
            ]).to_string(),
            CultureType::Religious => pick(rng, &[
                "Temple", "Faith", "Order", "Brotherhood", "Chosen",
                "Blessed", "Holy See", "Covenant", "Congregation", "Clergy",
            ]).to_string(),
            CultureType::Nomadic => pick(rng, &[
                "Wanderers", "Tribe", "Clan", "Band", "Pack",
                "Rovers", "Travelers", "Nomads", "Migrants", "Wayfarers",
            ]).to_string(),
            CultureType::Industrial => pick(rng, &[
                "Works", "Foundry", "Combine", "Syndicate", "Factory",
                "Forge", "Craft", "Industry", "Makers", "Builders",
            ]).to_string(),
            CultureType::Isolationist => match species {
                Species::Elf => pick(rng, &["Court", "Grove", "Sanctuary", "Haven", "Realm"]).to_string(),
                Species::Dwarf => pick(rng, &["Hold", "Stronghold", "Citadel", "Halls", "Depths"]).to_string(),
                _ => pick(rng, &["Enclave", "Domain", "Realm", "Territory", "Kingdom"]).to_string(),
            },
            CultureType::Expansionist => pick(rng, &[
                "Empire", "Dominion", "Imperium", "Kingdom", "Dynasty",
                "Sovereignty", "Hegemony", "Supremacy", "Ascendancy", "Reign",
            ]).to_string(),
        }
    }

    fn place_root(&self, species: Species, rng: &mut ChaCha8Rng) -> String {
        match species {
            Species::Human => pick(rng, &[
                "North", "South", "East", "West", "King", "Queen",
                "High", "Low", "New", "Old", "Green", "White",
                "Black", "Red", "Stone", "Wood", "River", "Hill",
                "Vale", "Glen", "Marsh", "Mead", "Wheat", "Corn",
            ]).to_string(),
            Species::Dwarf => pick(rng, &[
                "Khaz", "Karak", "Barak", "Durn", "Zhuf", "Grom",
                "Thund", "Krag", "Gor", "Zul", "Kar", "Dur",
                "Bel", "Mal", "Tor", "Gund", "Azg", "Khrum",
            ]).to_string(),
            Species::Elf => pick(rng, &[
                "Elen", "Silv", "Lor", "Gala", "Cele", "Ara",
                "Itha", "Mith", "Quell", "Thel", "Vera", "Luna",
                "Sola", "Aura", "Fael", "Rial", "Nym", "Syl",
            ]).to_string(),
            Species::Orc => pick(rng, &[
                "Grak", "Mork", "Gork", "Skul", "Krug", "Zug",
                "Thrak", "Grim", "Mog", "Gul", "Rok", "Drak",
                "Gash", "Krag", "Zorn", "Grot", "Skar", "Vrak",
            ]).to_string(),
            Species::Goblin => pick(rng, &[
                "Snik", "Grot", "Skab", "Nit", "Grib", "Snak",
                "Trik", "Pik", "Nik", "Gik", "Zik", "Rik",
            ]).to_string(),
            Species::Giant => pick(rng, &[
                "Thun", "Jotun", "Titan", "Kol", "Berg", "Fjell",
                "Stein", "Grav", "Mos", "Ur", "Hrim", "Surt",
            ]).to_string(),
            Species::DragonKin => pick(rng, &[
                "Drakon", "Sear", "Pyre", "Cinder", "Scald", "Char",
                "Blaze", "Wyrm", "Scale", "Fang", "Talon", "Wing",
            ]).to_string(),
            Species::Undead => pick(rng, &[
                "Mort", "Necro", "Shade", "Crypt", "Tomb", "Grave",
                "Hollow", "Wither", "Blight", "Dusk", "Veil", "Gloom",
            ]).to_string(),
            Species::Elemental => pick(rng, &[
                "Pyro", "Hydro", "Terra", "Aero", "Storm", "Frost",
                "Magma", "Tide", "Quake", "Gale", "Void", "Flux",
            ]).to_string(),
        }
    }

    fn place_suffix(&self, species: Species, rng: &mut ChaCha8Rng) -> String {
        match species {
            Species::Human => pick(rng, &[
                "ton", "ham", "bury", "ford", "bridge", "castle",
                "shire", "dale", "vale", "wood", "field", "haven",
                "port", "keep", "hold", "watch", "wall", "gate",
            ]).to_string(),
            Species::Dwarf => pick(rng, &[
                "ak", "im", "ul", "un", "gor", "az",
                "dum", "rim", "grim", "barak", "hold", "deep",
            ]).to_string(),
            Species::Elf => pick(rng, &[
                "dil", "nost", "wen", "thil", "las", "dor",
                "ion", "iel", "rin", "duin", "loth", "mir",
            ]).to_string(),
            Species::Orc => pick(rng, &[
                "gor", "gash", "pit", "maw", "skull", "tooth",
                "kamp", "kraal", "fort", "den", "lair", "hole",
            ]).to_string(),
            Species::Goblin => pick(rng, &[
                "cave", "hole", "pit", "den", "nest", "warren",
                "burrow", "lair", "hive", "mound", "heap", "dump",
            ]).to_string(),
            Species::Giant => pick(rng, &[
                "heim", "gard", "hall", "throne", "seat", "peak",
                "top", "ridge", "summit", "crest", "dome", "spire",
            ]).to_string(),
            Species::DragonKin => pick(rng, &[
                "roost", "aerie", "nest", "lair", "den", "hollow",
                "peak", "spire", "throne", "keep", "hold", "sanctum",
            ]).to_string(),
            Species::Undead => pick(rng, &[
                "barrow", "cairn", "crypt", "tomb", "mausoleum", "ossuary",
                "catacomb", "sepulcher", "necropolis", "grave", "pit", "depth",
            ]).to_string(),
            Species::Elemental => pick(rng, &[
                "nexus", "core", "heart", "font", "well", "spring",
                "focus", "node", "rift", "gate", "portal", "vortex",
            ]).to_string(),
        }
    }

    fn nature_adjective(&self, species: Species, rng: &mut ChaCha8Rng) -> String {
        match species {
            Species::Elf => pick(rng, &[
                "Whispering", "Silver", "Golden", "Moonlit", "Starlit",
                "Ancient", "Eternal", "Sacred", "Hidden", "Enchanted",
            ]).to_string(),
            Species::Dwarf => pick(rng, &[
                "Iron", "Stone", "Dark", "Deep", "Frozen",
                "Jagged", "Mighty", "Ancient", "Hidden", "Lost",
            ]).to_string(),
            _ => pick(rng, &[
                "Great", "Dark", "Misty", "Cold", "Frozen",
                "Burning", "Shadowed", "Ancient", "Wild", "Cursed",
            ]).to_string(),
        }
    }

    fn mountain_noun(&self, species: Species, rng: &mut ChaCha8Rng) -> String {
        pick(rng, &[
            "Peak", "Mountain", "Summit", "Spire", "Heights",
            "Crown", "Tooth", "Horn", "Ridge", "Crag",
        ]).to_string()
    }

    fn river_noun(&self, species: Species, rng: &mut ChaCha8Rng) -> String {
        pick(rng, &[
            "River", "Stream", "Waters", "Flow", "Rapids",
            "Cascade", "Falls", "Current", "Channel", "Run",
        ]).to_string()
    }

    fn forest_noun(&self, species: Species, rng: &mut ChaCha8Rng) -> String {
        pick(rng, &[
            "Forest", "Woods", "Grove", "Thicket", "Glade",
            "Wilds", "Woodland", "Timberland", "Weald", "Copse",
        ]).to_string()
    }

    fn lake_noun(&self, species: Species, rng: &mut ChaCha8Rng) -> String {
        pick(rng, &[
            "Lake", "Pool", "Mere", "Tarn", "Loch",
            "Waters", "Basin", "Pond", "Depths", "Mirror",
        ]).to_string()
    }

    fn artifact_prefix(&self, species: Species, rng: &mut ChaCha8Rng) -> String {
        match species {
            Species::Dwarf => pick(rng, &[
                "the Deep", "the Mountain King", "Endless Gold", "Iron Will",
                "the Forge Father", "Stone Hearts", "Eternal Craft",
            ]).to_string(),
            Species::Elf => pick(rng, &[
                "Starlight", "the Moon", "Eternal Dawn", "Silver Dreams",
                "the Forest Queen", "Ancient Stars", "Twilight's End",
            ]).to_string(),
            _ => pick(rng, &[
                "Power", "Ages", "Kings", "the Ancients", "Doom",
                "Light", "Shadow", "Eternity", "the Lost", "Prophecy",
            ]).to_string(),
        }
    }

    fn artifact_noun(&self, species: Species, rng: &mut ChaCha8Rng) -> String {
        match species {
            Species::Dwarf => pick(rng, &[
                "Hammer", "Axe", "Helm", "Shield", "Ring",
                "Crown", "Anvil", "Pick", "Gauntlet", "Armor",
            ]).to_string(),
            Species::Elf => pick(rng, &[
                "Bow", "Crown", "Ring", "Blade", "Staff",
                "Cloak", "Gem", "Mirror", "Harp", "Chalice",
            ]).to_string(),
            _ => pick(rng, &[
                "Sword", "Crown", "Ring", "Amulet", "Scepter",
                "Orb", "Blade", "Staff", "Tome", "Chalice",
            ]).to_string(),
        }
    }

    fn first_name(&self, species: Species, rng: &mut ChaCha8Rng) -> String {
        match species {
            Species::Human => pick(rng, &[
                "Aldric", "Bertram", "Cedric", "Edmund", "Frederick",
                "Godric", "Harold", "Leofric", "Oswald", "William",
                "Aelwyn", "Beatrix", "Cordelia", "Elspeth", "Gwendolyn",
                "Helena", "Isolde", "Morgana", "Rowena", "Vivienne",
            ]).to_string(),
            Species::Dwarf => pick(rng, &[
                "Thorin", "Balin", "Dwalin", "Gimli", "Gloin",
                "Durin", "Thrain", "Dain", "Nori", "Ori",
                "Disa", "Greta", "Hilda", "Ingrid", "Brunhild",
            ]).to_string(),
            Species::Elf => pick(rng, &[
                "Aelindel", "Caeleth", "Elrohir", "Faelar", "Galathil",
                "Ithilwen", "Laereth", "Melian", "Nimrodel", "Silmeth",
                "Arwen", "Celebrian", "Galadriel", "Idril", "Luthien",
            ]).to_string(),
            Species::Orc => pick(rng, &[
                "Grakk", "Thokk", "Urzog", "Grimbash", "Skullcrusher",
                "Bloodfang", "Ironjaw", "Bonesnapper", "Gorefist", "Warchief",
            ]).to_string(),
            Species::Goblin => pick(rng, &[
                "Snikkit", "Gribble", "Nikkle", "Skabrat", "Gritfang",
                "Sneekle", "Pikkit", "Nozrat", "Slyfoot", "Quicknick",
            ]).to_string(),
            Species::Giant => pick(rng, &[
                "Thrym", "Skadi", "Utgard", "Jotun", "Bergelmir",
                "Hrungnir", "Thiazi", "Surtr", "Ymir", "Angrboda",
            ]).to_string(),
            Species::DragonKin => pick(rng, &[
                "Fyraxxis", "Scorath", "Cindermaw", "Blazewing", "Pyraxis",
                "Emberclaw", "Ashscale", "Flameheart", "Searing", "Infernus",
            ]).to_string(),
            Species::Undead => pick(rng, &[
                "Morteus", "Vexrath", "Lichborne", "Gravewalker", "Soulbane",
                "Cryptlord", "Boneking", "Deathwhisper", "Shademaster", "Doomhex",
            ]).to_string(),
            Species::Elemental => pick(rng, &[
                "Pyroclasm", "Tideborn", "Stoneheart", "Windweaver", "Frostbane",
                "Thundercall", "Magmacore", "Stormfury", "Earthshaker", "Voidwalker",
            ]).to_string(),
        }
    }

    fn epithet(&self, species: Species, rng: &mut ChaCha8Rng) -> String {
        pick(rng, &[
            "the Great", "the Bold", "the Wise", "the Mighty", "the Cruel",
            "the Just", "the Conqueror", "the Builder", "the Destroyer", "the Unifier",
            "Ironhand", "Goldeneye", "Shadowbane", "Dragonslayer", "Worldshaker",
            "the Cursed", "the Blessed", "the Eternal", "the Last", "the First",
        ]).to_string()
    }

    /// Generate a book/tome title
    pub fn book_title(&self, species: Species, subject: &str, rng: &mut ChaCha8Rng) -> String {
        let prefixes: &[&str] = match species {
            Species::Dwarf => &["Treatise on", "Meditations on", "The Craft of", "Deep Wisdom of", "The Secrets of"],
            Species::Elf => &["Reflections on", "Songs of", "The Eternal", "Whispers of", "The Light of"],
            Species::Human => &["The Art of", "Principles of", "A Study of", "The Path to", "Foundations of"],
            _ => &["The Book of", "Writings on", "The Tome of", "Studies in", "The Wisdom of"],
        };

        let prefix = pick(rng, prefixes);
        format!("{} {}", prefix, subject)
    }

    /// Generate a prophecy text
    pub fn prophecy_text(&self, rng: &mut ChaCha8Rng) -> String {
        let prophecies = &[
            "When the stars align and the moon bleeds red, the sleeper shall awaken",
            "Three kings shall fall before the child of shadow rises",
            "The sword that was broken shall be reforged when darkness returns",
            "From the depths shall come salvation, from the heights shall come ruin",
            "The last of the line shall bear the burden of ages",
            "When fire meets water, a new age begins",
            "The forgotten one remembers, and in remembering, destroys",
            "Seven seals must break before the ending of days",
            "The heir of nothing shall inherit everything",
            "In the hour of greatest darkness, light shall bloom from stone",
        ];
        pick(rng, prophecies).to_string()
    }

    /// Generate a spell name
    pub fn spell_name(&self, rng: &mut ChaCha8Rng) -> String {
        let prefixes = &["Arcane", "Shadow", "Flame", "Frost", "Storm", "Void", "Divine", "Primal"];
        let suffixes = &["Bolt", "Shield", "Wave", "Strike", "Blessing", "Curse", "Ward", "Fury"];
        format!("{} {}", pick(rng, prefixes), pick(rng, suffixes))
    }

    /// Generate a religious tenet
    pub fn religious_tenet(&self, species: Species, rng: &mut ChaCha8Rng) -> String {
        let tenets: &[&str] = match species {
            Species::Dwarf => &[
                "Honor thy ancestors in all deeds",
                "The forge purifies the soul",
                "Gold is the blessing of the deep gods",
                "Stand firm as the mountain",
                "Craft with purpose, not with haste",
            ],
            Species::Elf => &[
                "Harmony with nature is the highest virtue",
                "The stars guide the faithful",
                "Time reveals all truths",
                "Beauty is a reflection of the divine",
                "Memory is sacred, forgetting is sin",
            ],
            Species::Human => &[
                "Serve the realm before thyself",
                "Justice must be swift and fair",
                "The gods reward the faithful",
                "Charity purifies the soul",
                "Truth is the foundation of honor",
            ],
            _ => &[
                "Power flows to the worthy",
                "Balance must be maintained",
                "Change is the only constant",
                "The cycle continues eternally",
                "Strength is its own virtue",
            ],
        };
        pick(rng, tenets).to_string()
    }

    /// Generate a dungeon name
    pub fn dungeon_name(&self, origin: &str, rng: &mut ChaCha8Rng) -> String {
        let adjectives = &["Ancient", "Forgotten", "Cursed", "Lost", "Ruined", "Haunted", "Dark", "Deep"];
        let suffixes = match origin {
            "tomb" => &["Tomb", "Crypt", "Barrow", "Mausoleum", "Catacomb", "Sepulcher"],
            "mine" => &["Mine", "Quarry", "Dig", "Excavation", "Shaft", "Tunnels"],
            "fortress" => &["Fortress", "Stronghold", "Citadel", "Keep", "Bastion", "Hold"],
            "temple" => &["Temple", "Shrine", "Sanctum", "Chapel", "Fane", "Monastery"],
            "cave" => &["Cavern", "Grotto", "Lair", "Den", "Hollow", "Depths"],
            _ => &["Ruins", "Chambers", "Halls", "Dungeons", "Vaults", "Pits"],
        };

        let adj = pick(rng, adjectives);
        let suffix = pick(rng, suffixes);
        format!("The {} {}", adj, suffix)
    }

    /// Generate first name for a hero (public alias)
    pub fn hero_first_name(&self, species: Species, rng: &mut ChaCha8Rng) -> String {
        self.first_name(species, rng)
    }

    // === BIOME-AWARE NAMING FUNCTIONS ===

    /// Get an adjective appropriate for a biome category
    pub fn biome_adjective(&self, category: BiomeCategory, rng: &mut ChaCha8Rng) -> String {
        let adjectives: &[&str] = match category {
            BiomeCategory::Volcanic => &[
                "Ash", "Ember", "Smoke", "Cinder", "Scorched",
                "Molten", "Burning", "Charred", "Sulfurous", "Blazing",
            ],
            BiomeCategory::Tundra => &[
                "Frost", "Ice", "Bitter", "Pale", "Frozen",
                "Winter", "Glacial", "Howling", "White", "Cold",
            ],
            BiomeCategory::Desert => &[
                "Sun", "Sand", "Scorched", "Gold", "Parched",
                "Burning", "Dry", "Dusty", "Ancient", "Shifting",
            ],
            BiomeCategory::Swamp => &[
                "Murk", "Fen", "Damp", "Foggy", "Rotting",
                "Drowned", "Mire", "Fetid", "Dark", "Sunken",
            ],
            BiomeCategory::Forest => &[
                "Sylvan", "Green", "Shade", "Wild", "Ancient",
                "Mossy", "Verdant", "Tangled", "Deep", "Quiet",
            ],
            BiomeCategory::Mountain => &[
                "High", "Stone", "Crag", "Peak", "Iron",
                "Thunder", "Wind", "Snow", "Rocky", "Grey",
            ],
            BiomeCategory::Coastal => &[
                "Sea", "Salt", "Tide", "Harbor", "Storm",
                "Wave", "Coral", "Pearl", "Azure", "Misty",
            ],
            BiomeCategory::Cave => &[
                "Deep", "Dark", "Shadow", "Hidden", "Blind",
                "Echo", "Stone", "Crystal", "Hollow", "Black",
            ],
            BiomeCategory::Hills => &[
                "Rolling", "Wind", "Heather", "Green", "Gentle",
                "Golden", "Tumbled", "Barrow", "Old", "Grassy",
            ],
            BiomeCategory::Grassland => &[
                "Golden", "Wide", "Open", "Wind", "Amber",
                "Tall", "Sun", "Plains", "Free", "Endless",
            ],
            BiomeCategory::Ruins => &[
                "Fallen", "Ancient", "Cursed", "Haunted", "Broken",
                "Forgotten", "Lost", "Crumbling", "Silent", "Dead",
            ],
            BiomeCategory::Ocean => &[
                "Deep", "Dark", "Abyssal", "Crushing", "Silent",
                "Endless", "Cold", "Midnight", "Sunken", "Vast",
            ],
            BiomeCategory::Mystical => &[
                "Ethereal", "Glowing", "Strange", "Arcane", "Twisted",
                "Shimmering", "Void", "Star", "Prismatic", "Eldritch",
            ],
        };
        pick(rng, adjectives).to_string()
    }

    /// Get a place suffix appropriate for a biome category
    pub fn biome_place_suffix(&self, category: BiomeCategory, rng: &mut ChaCha8Rng) -> String {
        let suffixes: &[&str] = match category {
            BiomeCategory::Volcanic => &["forge", "furnace", "caldera", "pit", "vent", "hearth", "cinder", "pyre"],
            BiomeCategory::Tundra => &["frost", "drift", "cold", "ice", "glacier", "floe", "chill", "white"],
            BiomeCategory::Desert => &["oasis", "well", "shade", "dune", "waste", "sand", "sun", "dust"],
            BiomeCategory::Swamp => &["marsh", "mire", "bog", "fen", "murk", "rot", "drown", "muck"],
            BiomeCategory::Forest => &["grove", "glade", "hollow", "dell", "bower", "thicket", "shade", "wood"],
            BiomeCategory::Mountain => &["hold", "eyrie", "spire", "peak", "crag", "ridge", "stone", "height"],
            BiomeCategory::Coastal => &["port", "bay", "haven", "cove", "reef", "tide", "shore", "harbor"],
            BiomeCategory::Cave => &["depths", "cavern", "abyss", "dark", "hollow", "void", "pit", "tunnel"],
            BiomeCategory::Hills => &["barrow", "tor", "down", "knoll", "rise", "mound", "cairn", "hill"],
            BiomeCategory::Grassland => &["field", "plain", "meadow", "lea", "range", "steppe", "vale", "green"],
            BiomeCategory::Ruins => &["tomb", "crypt", "ruin", "barrow", "hall", "throne", "vault", "grave"],
            BiomeCategory::Ocean => &["deep", "trench", "abyss", "floor", "reef", "shelf", "maw", "void"],
            BiomeCategory::Mystical => &["nexus", "rift", "veil", "gate", "well", "star", "dream", "void"],
        };
        pick(rng, suffixes).to_string()
    }

    /// Get a material appropriate for a biome category
    pub fn biome_material(&self, category: BiomeCategory, rng: &mut ChaCha8Rng) -> String {
        let materials: &[&str] = match category {
            BiomeCategory::Volcanic => &["obsidian", "basalt", "magma-steel", "ash-iron", "fire-opal", "pumice", "slag-iron"],
            BiomeCategory::Tundra => &["frost-crystal", "glacial ice", "winter-steel", "frozen silver", "ice-iron", "permafrost-bone"],
            BiomeCategory::Desert => &["sun-bronze", "desert glass", "sand-gold", "sun-steel", "amber", "sandstone"],
            BiomeCategory::Swamp => &["bog-iron", "petrified wood", "swamp-copper", "rot-silver", "peat-bronze", "marsh-steel"],
            BiomeCategory::Forest => &["living wood", "amber", "greenwood", "ironbark", "heartwood", "leaf-silver", "thorn-iron"],
            BiomeCategory::Mountain => &["mithril", "adamantine", "sky-iron", "mountain-silver", "granite", "deep-steel"],
            BiomeCategory::Coastal => &["sea-steel", "pearl", "coral", "drift-silver", "salt-crystal", "wave-glass"],
            BiomeCategory::Cave => &["deep-crystal", "shadow-steel", "cave-silver", "dark-iron", "blind-stone", "echo-metal"],
            BiomeCategory::Hills => &["barrow-bronze", "hill-iron", "copper", "heather-stone", "tumbled-silver", "cairn-gold"],
            BiomeCategory::Grassland => &["plains-bronze", "wind-steel", "grass-copper", "golden-iron", "amber-steel"],
            BiomeCategory::Ruins => &["cursed iron", "grave-silver", "tomb-gold", "shadow-steel", "death-bronze", "bone"],
            BiomeCategory::Ocean => &["abyssal-steel", "deep-coral", "pressure-iron", "brine-silver", "leviathan-bone"],
            BiomeCategory::Mystical => &["star-metal", "void-silver", "dream-crystal", "ether-steel", "spirit-glass", "arcane-gold"],
        };
        pick(rng, materials).to_string()
    }

    /// Generate a settlement name incorporating biome flavor
    pub fn settlement_name_biome(&self, species: Species, category: BiomeCategory, rng: &mut ChaCha8Rng) -> String {
        // 50% chance to use biome-flavored name
        if rng.gen_bool(0.5) {
            let adj = self.biome_adjective(category, rng);
            let suffix = self.place_suffix(species, rng);
            format!("{}{}", adj, suffix)
        } else {
            let base = self.place_root(species, rng);
            let biome_suffix = self.biome_place_suffix(category, rng);
            format!("{}{}", base, biome_suffix)
        }
    }

    /// Generate a lair name incorporating biome flavor
    pub fn lair_name_biome(&self, category: BiomeCategory, rng: &mut ChaCha8Rng) -> String {
        let adjective = self.biome_adjective(category, rng);
        let suffix = self.biome_place_suffix(category, rng);
        format!("The {} {}", adjective, suffix.chars().next().unwrap().to_uppercase().collect::<String>() + &suffix[1..])
    }

    /// Generate a dungeon name incorporating biome flavor
    pub fn dungeon_name_biome(&self, origin: &str, category: BiomeCategory, rng: &mut ChaCha8Rng) -> String {
        let biome_adj = self.biome_adjective(category, rng);

        let suffixes = match origin {
            "tomb" => &["Tomb", "Crypt", "Barrow", "Mausoleum", "Catacomb", "Sepulcher"],
            "mine" => &["Mine", "Quarry", "Dig", "Excavation", "Shaft", "Tunnels"],
            "fortress" => &["Fortress", "Stronghold", "Citadel", "Keep", "Bastion", "Hold"],
            "temple" => &["Temple", "Shrine", "Sanctum", "Chapel", "Fane", "Monastery"],
            "cave" => &["Cavern", "Grotto", "Lair", "Den", "Hollow", "Depths"],
            _ => &["Ruins", "Chambers", "Halls", "Dungeons", "Vaults", "Pits"],
        };

        let suffix = pick(rng, suffixes);
        format!("The {} {}", biome_adj, suffix)
    }

    /// Generate a biome-appropriate epithet for a hero
    pub fn biome_epithet(&self, role: &str, category: BiomeCategory, rng: &mut ChaCha8Rng) -> String {
        // Combine role with biome for unique epithets
        let epithets: &[&str] = match (role, category) {
            ("Warrior", BiomeCategory::Tundra) => &["Frostblade", "Ice-Born", "Winter's Wrath", "the Frozen", "Glacier-Heart"],
            ("Warrior", BiomeCategory::Volcanic) => &["Ashbringer", "Ember-Forged", "the Scorched", "Flame-Tempered", "Cinder-Blood"],
            ("Warrior", BiomeCategory::Desert) => &["Sandwalker", "Sun-Scorched", "the Parched", "Dune-Runner", "Heat-Born"],
            ("Warrior", BiomeCategory::Swamp) => &["Bog-Walker", "Mire-Born", "the Drowned", "Fen-Fighter", "Marsh-Blood"],
            ("Warrior", BiomeCategory::Forest) => &["Greenwood", "Shade-Walker", "the Wild", "Forest-Born", "Leaf-Blade"],
            ("Warrior", BiomeCategory::Mountain) => &["Stone-Born", "Peak-Climber", "the High", "Crag-Fighter", "Mountain-Heart"],
            ("Warrior", BiomeCategory::Coastal) => &["Storm-Rider", "Salt-Blood", "the Tidal", "Wave-Born", "Sea-Tempered"],
            ("Explorer", BiomeCategory::Cave) => &["Deep-Delver", "Shadow-Walker", "the Lightless", "Cave-Born", "Dark-Finder"],
            ("Explorer", BiomeCategory::Tundra) => &["Frost-Finder", "Ice-Walker", "the Far-Traveled", "Winter-Scout", "Cold-Seeker"],
            ("Explorer", BiomeCategory::Desert) => &["Sand-Finder", "Dune-Walker", "the Wanderer", "Oasis-Seeker", "Heat-Traveler"],
            ("Scholar", BiomeCategory::Mystical) => &["Star-Reader", "Void-Touched", "the Enlightened", "Rift-Seer", "Ether-Sage"],
            ("Scholar", BiomeCategory::Ruins) => &["Tomb-Reader", "Ruin-Walker", "the Ancient-Seeker", "Lore-Finder", "Grave-Scholar"],
            ("Priest", BiomeCategory::Mountain) => &["High-Blessed", "Peak-Prophet", "Stone-Touched", "Mountain-Voice", "Crag-Saint"],
            ("Priest", BiomeCategory::Volcanic) => &["Flame-Blessed", "Ash-Prophet", "Fire-Touched", "Ember-Voice", "Forge-Saint"],
            _ => {
                // Generic biome-based epithets
                match category {
                    BiomeCategory::Volcanic => &["the Ash-Born", "Fire-Touched", "the Scorched"],
                    BiomeCategory::Tundra => &["the Frost-Born", "Ice-Touched", "the Frozen"],
                    BiomeCategory::Desert => &["the Sand-Born", "Sun-Touched", "the Parched"],
                    BiomeCategory::Swamp => &["the Bog-Born", "Mire-Touched", "the Drowned"],
                    BiomeCategory::Forest => &["the Wood-Born", "Green-Touched", "the Wild"],
                    BiomeCategory::Mountain => &["the Stone-Born", "Peak-Touched", "the High"],
                    BiomeCategory::Coastal => &["the Sea-Born", "Salt-Touched", "the Tidal"],
                    BiomeCategory::Cave => &["the Deep-Born", "Shadow-Touched", "the Dark"],
                    BiomeCategory::Mystical => &["the Star-Born", "Void-Touched", "the Strange"],
                    _ => &["the Great", "the Bold", "the Wise"],
                }
            }
        };
        pick(rng, epithets).to_string()
    }
}

/// Helper function to pick a random element from a slice
fn pick<'a>(rng: &mut ChaCha8Rng, options: &[&'a str]) -> &'a str {
    options[rng.gen_range(0..options.len())]
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn test_faction_names() {
        let gen = NameGenerator::new(42);
        let mut rng = ChaCha8Rng::seed_from_u64(42);

        for species in Species::all() {
            for culture in CultureType::all() {
                let name = gen.faction_name(*species, *culture, &mut rng);
                assert!(!name.is_empty(), "Faction name should not be empty");
                println!("{:?} {:?}: {}", species, culture, name);
            }
        }
    }

    #[test]
    fn test_settlement_names() {
        let gen = NameGenerator::new(42);
        let mut rng = ChaCha8Rng::seed_from_u64(42);

        for species in Species::all() {
            let name = gen.settlement_name(*species, &mut rng);
            assert!(!name.is_empty(), "Settlement name should not be empty");
            println!("{:?}: {}", species, name);
        }
    }
}
