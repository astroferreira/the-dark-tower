# History Simulation System - Implementation Plan

> **Version**: 2.0 (Final Design)  
> **Created**: 2026-01-26  
> **Project**: `planet_generation` - Procedural world generator in Rust  
> **Goal**: Dwarf Fortress-style world history simulation with emergent narrative

---

## 1. Introduction

### 1.1 Project Context

This document describes the implementation of a **history simulation system** for the `planet_generation` project. The existing codebase generates procedural terrain including:

- Tectonic plate simulation with realistic mountain ranges
- Hydraulic and glacial erosion
- Climate simulation (temperature, moisture, seasons)
- 100+ biome types (forests, deserts, oceans, volcanic, fantasy, ruins, etc.)
- Water bodies (oceans, lakes, rivers)
- An interactive terminal-based explorer using `ratatui`

The history simulation will run **on top of** this generated terrain, populating it with civilizations, creatures, notable figures, and events. The terrain provides the stage; history provides the actors and drama.

### 1.2 Inspiration: Dwarf Fortress Legends Mode

The primary inspiration is [Dwarf Fortress](https://www.bay12games.com/dwarves/)'s world generation and Legends Mode. In Dwarf Fortress:

1. **World generation** creates terrain, civilizations, and creatures
2. **History simulation** runs for hundreds of years, generating events
3. **Legends Mode** allows players to explore the generated history
4. Events connect to each other through **causality** - wars have causes, heroes have motivations
5. Artifacts and monuments encode history through **inscriptions**

Our implementation aims to capture this depth while being **sparse** (we don't track every individual, only notable ones) and **modular** (each system can be developed and tested independently).

### 1.3 Core Philosophy: Sparse Simulation

> [!IMPORTANT]
> **Sparse Simulation** means we track **aggregates** rather than individuals, generating specific entities only when needed for events.

This is critical for performance and manageability:

| What we DO track | What we DON'T track |
|------------------|---------------------|
| Faction total population (e.g., 50,000) | Every individual citizen |
| Notable figures (rulers, heroes, villains) | Common farmers or soldiers |
| Major events (wars, coronations, cataclysms) | Daily life events |
| City locations and populations | Every building in a city |
| Trade route connections | Every caravan trip |
| Legendary creatures | Every random animal |

When an event requires a specific person (e.g., "an assassin killed the king"), we generate that assassin **on demand** with appropriate traits, rather than simulating thousands of potential assassins. Unless, there is already a notorious assassin involved with the king.

### 1.4 Tile-Based History

History is **anchored to world tiles**. Every event, settlement, and artifact has a location on the world map (which uses an equirectangular projection with horizontal wrapping). This allows:

- Querying "what happened at this location?"
- Showing faction territories on the map
- Placing monster lairs in appropriate biomes
- Trade routes following pathfindable terrain

The history save file will reference the world seed so terrain can be regenerated, but history itself is a separate file that logs everything that occurred.

---

## 2. Design Decisions (Detailed Rationale)

Each design decision below was made collaboratively to balance depth, coherence, and implementation complexity.

### 2.1 Timescale: Seasonal Calendar

**Decision**: Use a calendar with **4 seasons per year** (Spring, Summer, Autumn, Winter), simulating 500 years by default (configurable via `--history-years N`).

**Rationale**:
- Seasons matter for agriculture, warfare, and migration. A battle in winter is different from summer.
- 500 years provides enough time for civilizations to rise and fall, but isn't so long that the event log becomes unmanageable.
- The seasonal granularity (4 steps per year = 2000 total steps for 500 years) allows meaningful temporal patterns without being as expensive as daily simulation.
- Each simulation step = 1 season. Events occur within seasons.

**Example event**: "In the Winter of Year 247, the Siege of Ironhold began. The defenders held through the Spring but fell in Summer when supplies ran out."

### 2.2 Race System: Hybrid (Fixed Base Types + Procedural Culture)

**Decision**: Use **fixed base race types** (Human, Dwarf, Elf, Orc, Reptilian, Fey, Undead, etc.) combined with **procedurally generated culture** (language, values, customs).

**Rationale**:
- **Fixed base types** provide familiar archetypes. Players understand that "Dwarves" probably like mining and mountains. This creates intuitive coherence.
- **Procedural culture** ensures each playthrough is unique. "The Irondelve Dwarves" might be peaceful merchants while "The Bitterforge Dwarves" are warlike expansionists.
- Cultural values (martial vs peaceful, traditional vs innovative, xenophobic vs welcoming) drive behavior and create faction variety.
- Each culture has its own **procedural language** for naming characters, places, and artifacts.

**Why not fully procedural races?** Generating coherent anatomy, culture, and behavior from scratch often produces alien or nonsensical results. Hybrid anchors creativity to familiar foundations.

### 2.3 Creature Anatomy: Parts-Based Procedural System

**Decision**: Creatures are built from **body parts** (heads, torsos, limbs, wings, tails, tentacles, etc.) with procedural combinations, materials, and abilities.

**Rationale**:
- Unlike civilized races, **monsters should feel strange and varied**. A parts-based system creates creatures like "a six-legged beast with a crystalline carapace and three venomous mandibles."
- Each part has properties:
  - **Count**: How many (2 arms, 8 legs, 3 heads)
  - **Material**: Flesh, chitin, scales, stone, crystal, flame
  - **Special traits**: Venomous, fire-breathing, regenerating, armored
- Part combinations imply locomotion, attacks, and habitat preferences.

**Size spectrum**: Creatures range from Tiny (rat-sized) to Colossal (kaiju). **Kaiju are ultra-rare** (0.1% spawn chance) to make them legendary world threats when they appear.

**Intelligence spectrum**: Ranges from Mindless (slimes, some undead) through Instinctual (beasts) to Cunning (wolves) to Sapient (can communicate) to Genius (dragons, ancient liches). Higher intelligence enables leadership and complex behavior.

### 2.4 Monster Populations and Leaders

**Decision**: Monsters can form **populations** (groups of the same species), but only become organized threats when they have a **leader** (a legendary creature or sapient individual).

**Rationale**:
- Leaderless monsters are scattered threats - wolves attack travelers, bats infest caves
- When a legendary creature (e.g., "Vrathok the Shadowmaw") takes over a population, they become an organized force that can conduct raids, control territory, and even be worshipped
- This creates emergent gameplay: killing a monster leader may scatter the horde, or a new leader may rise

### 2.5 Legendary Creatures: Unique Named Beings

**Decision**: Some monsters are unique named individuals (like Smaug, Shelob, or a specific ancient dragon) rather than generic species members.

**Rationale**:
- Legendary creatures are **narrative anchors**. "The hero Aldric slew Vrakorath the Devourer" is more compelling than "The hero killed a large wyrm."
- They have personal histories: when they were born, what they killed, what artifacts they hoard
- They can be worshipped as gods by cults (see Religion below)
- They have lairs placed on the world map in appropriate biomes

### 2.6 Civilization Interactions: Hybrid Diplomacy

**Decision**: Use **hybrid diplomacy** - basic diplomatic states (War, Hostile, Neutral, Friendly, Allied) driven by **underlying factors** (cultural similarity, territorial pressure, trade value, historical grievances).

**Rationale**:
- Pure direct diplomacy (explicit treaties) can feel mechanical
- Pure emergent systems can produce inexplicable behavior
- Hybrid approach: underlying factors (Is their culture similar? Do we share borders? Have they wronged us?) push diplomatic stance toward war or peace
- Explicit events (signing a treaty, declaring war) provide narrative clarity
- Cultural similarity calculation creates natural friend/enemy patterns

**Example**: Two Dwarf factions might start friendly (cultural similarity) but drift toward war as they compete for the same mountain territory.

### 2.7 Religion: Complex with Monster Worship

**Decision**: Civilizations have religions with **deities** from multiple sources, including the option to **worship local legendary creatures as gods**.

**Rationale**:
- Religion provides cultural depth and drives events (holy wars, temple construction, heresies)
- Deity types include:
  - **Gods**: Traditional divine beings with domains (war, death, nature)
  - **Ancestors**: Deified historical figures
  - **Spirits**: Nature and place spirits
  - **Monsters**: Living legendary creatures worshipped as divine beings

**Monster worship** creates fascinating dynamics:
- A village near a dragon's lair might form a cult to appease it
- The cult offers sacrifices, and in return the dragon doesn't destroy them
- If the dragon is killed, the cult might fragment or seek vengeance
- The creature becomes a "living god" with political influence

### 2.8 Magic System

**Decision**: Magic exists and affects events, artifacts, and creatures.

**Rationale**:
- The biome system already includes fantasy biomes (Crystal Forests, Bioluminescent Waters, Ethereal Mist)
- Magical events add narrative variety: curses, enchantments, magical catastrophes
- Artifacts can be enchanted with magical properties
- Some creatures are magical in origin
- Magic schools/types provide variety in wizard/sorcerer characters

### 2.9 Terrain Influences Events (Not Vice Versa)

**Decision**: Terrain **triggers and shapes events**, but events do **NOT permanently modify terrain**. Volcanic eruptions, floods, and other natural disasters are **transient events** that affect civilizations without altering the underlying world map.

**Rationale**:
- The terrain generation system produces a carefully balanced world. Modifying it during history would break that balance.
- Separating terrain from history keeps the systems modular and the history file lightweight.
- Transient events still have narrative impact: "The eruption of Mount Ashfall destroyed the city of Ironhold in Year 156" - the volcano was always there, the event is the destruction.
- Simpler implementation: no need to track terrain deltas or apply/unapply changes.

**How terrain influences events**:
- **Volcanoes** (from volcanic biomes): Can erupt, destroying nearby settlements and killing populations
- **Flood plains** (low coastal/river areas): Subject to flooding events that damage cities
- **Harsh biomes** (deserts, tundra): Cause famines, limit population growth
- **Mountains**: Create natural borders, affect trade route costs
- **Resources** (from biomes): Drive conflict and settlement placement

**What events DO NOT change**:
- Heightmap values
- Biome types
- Water bodies
- Any terrain tile data

Events are **overlays** on unchanging terrain, recorded in history but not altering the physical world.

### 2.10 Political Succession: Detailed with Quirks

**Decision**: Implement detailed succession laws and political mechanics to generate **weird quirks, inheritance disputes, and political struggles**.

**Rationale**:
- Succession crises are major historical drivers (wars of succession, usurpers, child rulers)
- Multiple succession types create variety:
  - Primogeniture (eldest child inherits)
  - Male/Female preference variants
  - Elective (nobles vote)
  - Designation (ruler chooses successor)
  - Seniority (oldest family member)
  - Tanistry (Celtic style - elected from extended family)
  - Open succession (anyone can claim, often by force)
- Different laws create different political dynamics
- Personality traits of figures affect succession outcomes (ambitious younger sibling vs loyal heir)

### 2.11 Trade Resources: Specific Named Types

**Decision**: Use **specific named trade resources** rather than abstract "trade value."

**Rationale**:
- Specific resources create narrative: "The war was fought over the iron mines of Blackstone Pass"
- Resources are tied to biomes/terrain: gold in mountains, spices in tropics, fish on coasts
- Trade routes form between complementary economies
- Resource scarcity drives conflict
- Monster-derived resources (dragon scales, monster bones) create hunting motivations

**Resource categories**:
- Basic: Food, Wood, Stone
- Metals: Iron, Copper, Gold, Silver, Mithril, Adamantine
- Gems: Diamonds, Rubies, Emeralds
- Luxury: Spices, Silk, Wine
- Magical: Magical components, Ancient relics
- Monster-derived: Dragon scales, Monster bones, Ichor

### 2.12 Naming Conventions (Simplified Procedural Languages)

**Decision**: Each civilization has **naming traits** that affect how names are generated - this is a lightweight system for creating culturally distinct names, NOT a full language generator.

**Rationale**:
- We want "Krath-Morul" to sound different from "Aelindria" without implementing actual linguistics
- Names are the primary user-visible output; we don't need translatable inscriptions
- Simpler implementation: traits modify a name generator rather than defining grammar

**Naming traits per civilization**:
- **Phoneme preferences**: Harsh consonants (k, g, r) vs soft sounds (l, n, s)
- **Syllable count tendency**: Short (1-2) vs long (3-4) names
- **Common prefixes/suffixes**: "-hold", "-heim", "El-", "Kha-"
- **Compound style**: Whether to use compound names ("Blackstone", "Ironforge")
- **Apostrophe usage**: Some cultures use breaks ("D'kari", "T'shan")

**What this affects**:
- Character names
- Settlement names  
- Faction names
- Artifact names
- Monster epithets

**What we DON'T do**:
- Full grammar systems
- Translatable inscriptions in made-up languages
- Vocabulary generation beyond names

### 2.13 Events: Rich Linked System with Causality

**Decision**: Events track **causes and effects**, enabling queries like "Why did this war start?" or "What happened because of this assassination?"

**Rationale**:
- Simple event logs feel disconnected: "War happened. Battle happened. Peace happened."
- Linked events create narrative: "The assassination of King Aldric (Year 203) led to the War of Succession (Year 204-211), which ended with the Treaty of Blackstone (Year 211)"
- Each event stores:
  - `causes`: Events that contributed to this
  - `triggered_by`: The immediate cause
  - `consequences`: What resulted
  - `triggered_events`: Events this directly caused
- Enables Legends Mode queries and understanding history

### 2.14 Artifacts and Monuments with Inscriptions

**Decision**: Artifacts and monuments **encode their history** through inscriptions written in procedural languages.

**Rationale**:
- "This sword was made in Year 87 by the smith Toran for King Valdric III to commemorate his victory over the Orcs of Bloodfang" is more interesting than "a magic sword"
- Inscriptions reference the creating culture's language
- Reading an artifact reveals history
- Monuments commemorate events, honor figures, or mark territorial claims
- Lost artifacts can be found, their history decoded

### 2.15 Explorer Integration

**Decision**: Add a **Factions view mode** to the existing terminal explorer showing:
- Current territory (color-coded by faction)
- Settlement markers (simple icons, details in tile info panel)
- Trade route lines connecting cities
- Monster lair markers

**Rationale**:
- Leverages existing explorer infrastructure
- Provides visual feedback during/after simulation
- Settlement icons keep map readable; detailed info goes in the side panel
- Trade routes as visible lines help understand economic connections

### 2.16 Save Format: Separate File with World Reference

**Decision**: Save history to a **separate file** from terrain, but include the world seed for terrain regeneration.

**Rationale**:
- Separation allows reading history logs without loading full terrain
- Smaller history file for sharing/backup
- World seed ensures terrain can be deterministically regenerated
- Format: JSON or MessagePack for human readability vs size tradeoff
- History file includes: `world_seed`, `history_seed`, all entities, all events

---

## 3. Design Decisions Summary Table

| Decision | Choice |
|----------|--------|
| **Timescale** | Seasonal calendar (4 seasons/year), 500 years default |
| **Race system** | Hybrid: fixed base types + procedural culture |
| **Creature anatomy** | Parts-based procedural (heads, limbs, wings, tails) |
| **Creature sizes** | Tiny to Kaiju (kaiju ultra-rare: 0.1% chance) |
| **Monster populations** | Can form organized populations only if they have a leader |
| **Legendary creatures** | Yes, unique named beings with personal history |
| **Civilization interactions** | Hybrid diplomacy with underlying cultural/resource factors |
| **Explorer view** | Current territories, simple icons, visible trade routes |
| **Event system** | Rich linked events with causes/effects for causality queries |
| **Magic** | Yes, affects events, artifacts, and creatures |
| **Religion** | Complex, with traditional deities and monster worship cults |
| **Terrain influence** | Terrain triggers events (eruptions, floods); events do NOT modify terrain |
| **Political succession** | 10+ succession laws for complex inheritance drama |
| **Trade resources** | Specific named resources tied to terrain |
| **Naming conventions** | Civilization traits for culturally distinct name generation |
| **Save format** | Separate file, references world seed |

---

## Module Architecture

```
src/
├── history/                    # New history simulation module
│   ├── mod.rs                  # Module exports and HistoryEngine
│   ├── config.rs               # HistoryConfig parameters
│   │
│   ├── time/                   # Calendar and timeline
│   │   ├── mod.rs
│   │   ├── calendar.rs         # Season, Year, Date
│   │   ├── timeline.rs         # Historical timeline, eras
│   │   └── age.rs              # Named ages/epochs
│   │
│   ├── naming/                 # Name generation
│   │   ├── mod.rs
│   │   ├── styles.rs           # Naming style traits
│   │   └── generator.rs        # Name generation
│   │
│   ├── entities/               # Living beings
│   │   ├── mod.rs
│   │   ├── races.rs            # Race definitions (base types)
│   │   ├── culture.rs          # Procedural culture traits
│   │   ├── figures.rs          # Notable individuals
│   │   ├── lineage.rs          # Family trees, succession
│   │   └── traits.rs           # Personality, abilities
│   │
│   ├── creatures/              # Monsters and beasts
│   │   ├── mod.rs
│   │   ├── anatomy.rs          # Body part system
│   │   ├── generator.rs        # Creature generation
│   │   ├── behavior.rs         # AI patterns
│   │   ├── populations.rs      # Monster populations
│   │   └── legendary.rs        # Unique named creatures
│   │
│   ├── civilizations/          # Organized societies
│   │   ├── mod.rs
│   │   ├── faction.rs          # Faction definition
│   │   ├── settlement.rs       # Cities, villages, camps
│   │   ├── territory.rs        # Land control, borders
│   │   ├── economy.rs          # Resources, trade routes
│   │   ├── diplomacy.rs        # Relations, treaties
│   │   ├── military.rs         # Armies, sieges, wars
│   │   └── government.rs       # Leadership, succession
│   │
│   ├── religion/               # Belief systems
│   │   ├── mod.rs
│   │   ├── deity.rs            # Gods and spirits
│   │   ├── worship.rs          # Temples, rituals
│   │   └── monster_cults.rs    # Monster-based religions
│   │
│   ├── magic/                  # Magic system
│   │   ├── mod.rs
│   │   ├── schools.rs          # Magic types/schools
│   │   ├── spells.rs           # Spell effects
│   │   └── enchantment.rs      # Item enchanting
│   │
│   ├── events/                 # Historical events
│   │   ├── mod.rs
│   │   ├── types.rs            # Event type definitions
│   │   ├── generator.rs        # Event generation
│   │   ├── consequences.rs     # Event effects
│   │   ├── chronicle.rs        # Event logging
│   │   └── causality.rs        # Cause-effect chains
│   │
│   ├── objects/                # Created artifacts
│   │   ├── mod.rs
│   │   ├── artifacts.rs        # Named magical items
│   │   ├── monuments.rs        # Structures, memorials
│   │   └── inscriptions.rs     # Encoded history/lore
│   │
│   ├── world_state/            # Tile-level history
│   │   ├── mod.rs
│   │   └── tile_history.rs     # Per-tile historical data (ownership, events)
│   │
│   ├── simulation/             # Simulation engine
│   │   ├── mod.rs
│   │   ├── engine.rs           # Main simulation loop
│   │   ├── step.rs             # Single step execution
│   │   └── playback.rs         # Pause, resume, speed
│   │
│   ├── legends/                # Legends mode
│   │   ├── mod.rs
│   │   ├── mode.rs             # UI state machine
│   │   ├── queries.rs          # Search and filter
│   │   └── renderer.rs         # Display formatting
│   │
│   └── persistence/            # Save/load
│       ├── mod.rs
│       ├── serialize.rs        # History serialization
│       └── format.rs           # File format definition
│
├── explorer.rs                 # Add faction view mode
└── world.rs                    # Add history reference
```

---

## Core Data Structures

### Time System

```rust
/// A season in the calendar
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

/// A specific date in the world calendar
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Date {
    pub year: u32,
    pub season: Season,
}

impl Date {
    pub fn new(year: u32, season: Season) -> Self {
        Self { year, season }
    }
    
    pub fn advance(&mut self) {
        match self.season {
            Season::Spring => self.season = Season::Summer,
            Season::Summer => self.season = Season::Autumn,
            Season::Autumn => self.season = Season::Winter,
            Season::Winter => {
                self.season = Season::Spring;
                self.year += 1;
            }
        }
    }
    
    pub fn seasons_since(&self, other: &Date) -> i32 {
        let self_total = self.year as i32 * 4 + self.season as i32;
        let other_total = other.year as i32 * 4 + other.season as i32;
        self_total - other_total
    }
}

/// A named historical era
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Era {
    pub id: EraId,
    pub name: String,           // "The Age of Strife"
    pub start: Date,
    pub end: Option<Date>,      // None if ongoing
    pub defining_events: Vec<EventId>,
}
```

### Naming System

```rust
/// Naming style traits for a civilization
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NamingStyle {
    pub id: NamingStyleId,
    
    // Sound preferences
    pub preferred_consonants: Vec<char>,    // ['k', 'r', 'th'] for harsh, ['l', 'n', 's'] for soft
    pub preferred_vowels: Vec<char>,        // ['a', 'o', 'u'] vs ['e', 'i']
    pub uses_apostrophes: bool,             // "D'kari", "T'shan"
    pub uses_hyphens: bool,                 // "Krath-Morul"
    
    // Structure preferences
    pub syllable_count: (u8, u8),           // (min, max) syllables
    pub common_prefixes: Vec<String>,       // ["El", "Kha", "Iron"]
    pub common_suffixes: Vec<String>,       // ["hold", "heim", "ia"]
    pub compound_names: bool,               // "Blackstone", "Ironforge"
    
    // Name type modifiers
    pub place_suffixes: Vec<String>,        // ["ton", "burg", "dale"]
    pub title_patterns: Vec<String>,        // ["the Great", "Slayer of"]
}

impl NamingStyle {
    pub fn generate_name(&self, rng: &mut impl Rng) -> String { ... }
    pub fn generate_place_name(&self, rng: &mut impl Rng) -> String { ... }
    pub fn generate_epithet(&self, rng: &mut impl Rng) -> String { ... }
}
```
```

### Race and Culture

```rust
/// Base race types (fixed archetypes)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RaceType {
    // Humanoid types
    Human,
    Dwarf,
    Elf,
    Orc,
    Goblin,
    Halfling,
    
    // Exotic types
    Reptilian,    // Lizardfolk, Dragonborn
    Fey,          // Sprites, Dryads
    Undead,       // Vampires, Liches (civilized)
    Elemental,    // Genasi-like
    Beastfolk,    // Animal-humanoid hybrids
    
    // Ancient/rare
    Giant,
    Construct,    // Living constructs
}

/// Procedurally generated cultural traits
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Culture {
    pub id: CultureId,
    pub name: String,
    pub naming_style: NamingStyleId,    // How this culture names things
    
    // Values (0.0 to 1.0)
    pub values: CultureValues,
    
    // Aesthetic preferences
    pub preferred_materials: Vec<ResourceType>,
    pub architecture_style: ArchitectureStyle,
    pub art_motifs: Vec<ArtMotif>,
    
    // Social structure
    pub government_preference: GovernmentType,
    pub gender_roles: GenderRoles,
    pub family_structure: FamilyStructure,
    
    // Religious tendencies
    pub religiosity: f32,           // 0.0 = secular, 1.0 = theocracy
    pub monster_worship_tendency: f32,  // Likelihood to worship local monsters
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CultureValues {
    pub martial: f32,        // War vs Peace
    pub tradition: f32,      // Tradition vs Innovation
    pub collectivism: f32,   // Community vs Individual
    pub nature_harmony: f32, // Domination vs Harmony with nature
    pub magic_acceptance: f32,
    pub xenophobia: f32,     // Distrust of outsiders
    pub honor: f32,
    pub wealth: f32,
}

/// A complete race definition
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Race {
    pub id: RaceId,
    pub base_type: RaceType,
    pub culture: Culture,
    pub name: String,           // "The Irondelve Dwarves"
    
    // Physical traits
    pub lifespan: (u32, u32),   // (min, max) years
    pub maturity_age: u32,
    pub height_range: (f32, f32), // meters
    
    // Gameplay traits
    pub preferred_biomes: Vec<ExtendedBiome>,
    pub terrain_penalties: HashMap<ExtendedBiome, f32>,
    pub innate_abilities: Vec<Ability>,
}
```

### Creature Anatomy System

```rust
/// Body part categories
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BodyPartType {
    Head,
    Torso,
    Arms,
    Legs,
    Wings,
    Tail,
    Tentacles,
    Fins,
    Horns,
    Mandibles,
    Eyes,
    Mouth,
}

/// A specific body part with properties
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BodyPart {
    pub part_type: BodyPartType,
    pub count: u8,              // Number of this part (2 arms, 8 legs, etc.)
    pub size: BodyPartSize,     // Relative size
    pub material: BodyMaterial, // Flesh, chitin, stone, etc.
    pub special: Vec<BodyPartSpecial>, // Venomous, fire-breathing, etc.
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum BodyPartSize {
    Vestigial,
    Small,
    Normal,
    Large,
    Massive,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum BodyMaterial {
    Flesh,
    Chitin,
    Scales,
    Feathers,
    Stone,
    Metal,
    Crystal,
    Shadow,
    Flame,
    Ooze,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum BodyPartSpecial {
    Venomous,
    Acidic,
    FireBreathing,
    IceBreathing,
    Grasping,
    Regenerating,
    Armored,
    Camouflaged,
    Bioluminescent,
    Prehensile,
    Magical,
}

/// Size categories for creatures
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CreatureSize {
    Tiny,       // Rat, insect
    Small,      // Dog, goblin
    Medium,     // Human
    Large,      // Horse, ogre
    Huge,       // Elephant, giant
    Gargantuan, // Dragon
    Colossal,   // Kaiju (ULTRA RARE)
}

impl CreatureSize {
    /// Probability weight for generating this size (Kaiju = 0.001)
    pub fn rarity_weight(&self) -> f32 {
        match self {
            CreatureSize::Tiny => 0.15,
            CreatureSize::Small => 0.25,
            CreatureSize::Medium => 0.30,
            CreatureSize::Large => 0.18,
            CreatureSize::Huge => 0.08,
            CreatureSize::Gargantuan => 0.03,
            CreatureSize::Colossal => 0.001, // Ultra-rare kaiju
        }
    }
}

/// Intelligence levels
#[derive(Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum Intelligence {
    Mindless,       // Slimes, some undead
    Instinctual,    // Beasts
    Cunning,        // Wolves, some monsters
    Sapient,        // Can learn, communicate
    Genius,         // Dragons, liches
}

/// Creature behavior patterns
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreatureBehavior {
    pub aggression: f32,        // 0.0 = docile, 1.0 = always attacks
    pub territoriality: f32,    // Defends lair/area
    pub pack_tendency: f32,     // Forms groups
    pub ambush_tendency: f32,   // Prefers surprise
    pub treasure_hoarding: f32, // Collects valuables
    pub lair_building: f32,     // Creates/modifies lair
    pub migration: f32,         // Moves around
}

/// A complete creature definition
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreatureSpecies {
    pub id: CreatureSpeciesId,
    pub name: String,                   // "Shadowmaw Wyrm"
    pub description: String,
    
    // Anatomy
    pub body_parts: Vec<BodyPart>,
    pub size: CreatureSize,
    pub locomotion: Vec<Locomotion>,    // How it moves
    
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
    pub can_lead_population: bool,      // Can organize others
    pub population_role: PopulationRole,
    
    // Magical
    pub magical_abilities: Vec<MagicAbility>,
    pub is_magical_origin: bool,        // Created by magic
}

/// A unique legendary creature
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LegendaryCreature {
    pub id: LegendaryCreatureId,
    pub species: CreatureSpeciesId,
    pub name: String,                   // "Vrakorath the Devourer"
    pub epithet: String,                // "the Devourer"
    
    // Unique traits beyond species
    pub unique_abilities: Vec<MagicAbility>,
    pub size_multiplier: f32,           // May be larger than normal
    
    // History
    pub birth_date: Option<Date>,
    pub death_date: Option<Date>,
    pub lair_location: Option<(usize, usize)>,
    pub territory: Vec<(usize, usize)>,
    pub kills: Vec<EntityId>,           // Major kills (heroes, etc.)
    pub artifacts_owned: Vec<ArtifactId>,
    
    // Worshippers
    pub cult_faction: Option<FactionId>,
    pub worshipper_count: u32,
}

/// A population of creatures led by a leader
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreaturePopulation {
    pub id: PopulationId,
    pub species: CreatureSpeciesId,
    pub count: u32,
    pub location: (usize, usize),       // Primary lair
    pub territory: Vec<(usize, usize)>,
    pub leader: Option<LegendaryCreatureId>,  // If has leader, organized
    pub aggression_level: f32,
    pub last_raid: Option<Date>,
}
```

### Civilization and Settlements

```rust
/// A political faction (nation/kingdom/tribe)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Faction {
    pub id: FactionId,
    pub name: String,
    pub race: RaceId,
    pub founded: Date,
    pub dissolved: Option<Date>,
    
    // Territory
    pub capital: Option<SettlementId>,
    pub settlements: Vec<SettlementId>,
    pub territory: Vec<(usize, usize)>,  // All controlled tiles
    pub claimed_tiles: HashSet<(usize, usize)>,
    
    // Leadership
    pub government: GovernmentType,
    pub current_leader: Option<FigureId>,
    pub ruling_dynasty: Option<DynastyId>,
    pub succession_law: SuccessionLaw,
    
    // Economy
    pub resources: HashMap<ResourceType, u32>,
    pub trade_routes: Vec<TradeRouteId>,
    pub wealth: u32,
    
    // Religion
    pub state_religion: Option<ReligionId>,
    pub religious_tolerance: f32,
    
    // Military
    pub military_strength: u32,
    pub armies: Vec<ArmyId>,
    pub wars: Vec<WarId>,
    
    // Diplomacy
    pub relations: HashMap<FactionId, DiplomaticRelation>,
    pub treaties: Vec<TreatyId>,
    
    // History
    pub events: Vec<EventId>,
    pub notable_figures: Vec<FigureId>,
}

/// Settlement types
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SettlementType {
    Capital,
    City,
    Town,
    Village,
    Fort,
    Outpost,
    Camp,
    Temple,
    Mine,
    Port,
}

/// A settlement (city, town, village, etc.)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settlement {
    pub id: SettlementId,
    pub name: String,
    pub settlement_type: SettlementType,
    pub location: (usize, usize),
    pub faction: FactionId,
    pub founded: Date,
    pub destroyed: Option<Date>,
    
    // Population
    pub population: u32,
    pub population_cap: u32,        // Based on location resources
    pub growth_rate: f32,
    
    // Infrastructure
    pub buildings: Vec<BuildingType>,
    pub walls: WallLevel,
    pub trade_hub: bool,
    
    // Economy
    pub local_resources: Vec<ResourceType>,
    pub production: HashMap<ResourceType, f32>,
    pub trade_connections: Vec<SettlementId>,
    
    // Culture
    pub monuments: Vec<MonumentId>,
    pub temples: Vec<TempleId>,
    pub artifacts_present: Vec<ArtifactId>,
    
    // History
    pub sieges: Vec<SiegeId>,
    pub events: Vec<EventId>,
}

/// Trade route between settlements
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TradeRoute {
    pub id: TradeRouteId,
    pub endpoints: (SettlementId, SettlementId),
    pub path: Vec<(usize, usize)>,       // Tile path
    pub established: Date,
    pub dissolved: Option<Date>,
    pub goods_traded: Vec<ResourceType>,
    pub value: u32,
    pub safety: f32,                     // Affected by monsters, bandits
}

/// Resource types for trade
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceType {
    // Basic
    Food,
    Wood,
    Stone,
    
    // Metals
    Iron,
    Copper,
    Gold,
    Silver,
    Mithril,
    Adamantine,
    
    // Gems
    Gems,
    Diamonds,
    Rubies,
    Emeralds,
    
    // Special
    Spices,
    Silk,
    Wine,
    Salt,
    Herbs,
    MagicalComponents,
    AncientRelics,
    
    // Monster-derived
    DragonScale,
    MonsterBones,
    Ichor,
}

/// Diplomatic relation between factions
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiplomaticRelation {
    pub stance: DiplomaticStance,
    pub opinion: i32,                   // -100 to +100
    pub treaties: Vec<TreatyId>,
    pub last_war: Option<WarId>,
    pub trade_value: u32,
    pub cultural_similarity: f32,       // Affects opinion drift
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiplomaticStance {
    War,
    Hostile,
    Neutral,
    Friendly,
    Allied,
    Vassal,
    Overlord,
}
```

### Religion System

```rust
/// A deity or worshipped entity
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Deity {
    pub id: DeityId,
    pub name: String,
    pub epithets: Vec<String>,          // "of the Forge", "the Deathless"
    
    // Nature
    pub deity_type: DeityType,
    pub domains: Vec<Domain>,
    pub alignment: Alignment,
    
    // Origin
    pub origin: DeityOrigin,
    pub associated_monster: Option<LegendaryCreatureId>,  // For monster-worship
    
    // Worship
    pub holy_symbols: Vec<String>,
    pub sacred_animals: Vec<CreatureSpeciesId>,
    pub sacred_places: Vec<(usize, usize)>,
    pub rituals: Vec<RitualType>,
    
    // Power
    pub miracles: Vec<MiracleType>,
    pub divine_artifacts: Vec<ArtifactId>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum DeityType {
    God,            // Traditional deity
    Spirit,         // Nature spirit
    Ancestor,       // Deified ancestor
    Monster,        // Worshipped creature
    Concept,        // Abstract force
    Demon,          // Malevolent entity
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Domain {
    War,
    Death,
    Life,
    Nature,
    Fire,
    Water,
    Earth,
    Air,
    Magic,
    Knowledge,
    Crafts,
    Trade,
    Chaos,
    Order,
    Love,
    Vengeance,
    Trickery,
    Darkness,
    Light,
}

/// A religion practiced by factions
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Religion {
    pub id: ReligionId,
    pub name: String,
    pub deities: Vec<DeityId>,          // Pantheon or single deity
    pub origin_date: Date,
    pub founder: Option<FigureId>,
    
    // Structure
    pub religious_head: Option<FigureId>,
    pub temples: Vec<TempleId>,
    pub holy_sites: Vec<(usize, usize)>,
    
    // Practices
    pub doctrines: Vec<Doctrine>,
    pub forbidden: Vec<ForbiddenAct>,
    pub holidays: Vec<Holiday>,
    
    // Relations
    pub heresies: Vec<ReligionId>,      // Splinter faiths
    pub hostile_religions: Vec<ReligionId>,
    
    // Followers
    pub follower_factions: Vec<FactionId>,
    pub follower_count: u32,
}

/// Monster worship cult
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MonsterCult {
    pub id: CultId,
    pub worshipped_creature: LegendaryCreatureId,
    pub name: String,
    pub founded: Date,
    pub founder: Option<FigureId>,
    
    // Membership
    pub members: Vec<FigureId>,
    pub member_count: u32,
    pub secret: bool,                   // Hidden cult?
    
    // Activities
    pub sacrifices: bool,
    pub offerings: Vec<ResourceType>,
    pub rituals: Vec<RitualType>,
    pub granted_powers: Vec<MagicAbility>, // Powers from creature
    
    // Location
    pub headquarters: Option<(usize, usize)>,
    pub shrines: Vec<(usize, usize)>,
}
```

### Notable Figures

```rust
/// A notable individual in history
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Figure {
    pub id: FigureId,
    pub name: String,
    pub epithet: Option<String>,        // "the Great", "Kinslayer"
    
    // Identity
    pub race: RaceId,
    pub faction: Option<FactionId>,
    pub birth_date: Date,
    pub death_date: Option<Date>,
    pub cause_of_death: Option<DeathCause>,
    
    // Family
    pub parents: (Option<FigureId>, Option<FigureId>),
    pub spouse: Option<FigureId>,
    pub children: Vec<FigureId>,
    pub dynasty: Option<DynastyId>,
    
    // Traits
    pub personality: Personality,
    pub skills: HashMap<Skill, u8>,     // 0-100
    pub abilities: Vec<Ability>,
    
    // Positions
    pub titles: Vec<Title>,
    pub current_position: Option<Position>,
    pub position_history: Vec<(Position, Date, Date)>,
    
    // Possessions
    pub artifacts: Vec<ArtifactId>,
    
    // Relationships
    pub relationships: HashMap<FigureId, Relationship>,
    pub enemies: Vec<FigureId>,
    pub mentors: Vec<FigureId>,
    pub students: Vec<FigureId>,
    
    // History
    pub events: Vec<EventId>,
    pub kills: Vec<EntityId>,           // Slain foes
    pub achievements: Vec<Achievement>,
}

/// Dynasty/noble house
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dynasty {
    pub id: DynastyId,
    pub name: String,
    pub founded: Date,
    pub founder: FigureId,
    pub current_head: Option<FigureId>,
    pub succession_law: SuccessionLaw,
    
    // Members
    pub members: Vec<FigureId>,
    pub generations: u32,
    
    // Holdings
    pub factions_ruled: Vec<FactionId>,
    pub ancestral_seats: Vec<SettlementId>,
    pub heirlooms: Vec<ArtifactId>,
    
    // Reputation
    pub prestige: u32,
    pub scandals: Vec<EventId>,
    pub achievements: Vec<Achievement>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum SuccessionLaw {
    Primogeniture,          // Eldest child
    Ultimogeniture,         // Youngest child
    MalePrimogeniture,      // Eldest son
    FemalePrimogeniture,    // Eldest daughter
    Elective,               // Nobles vote
    ElectiveMonarchy,       // Council chooses from family
    Designation,            // Ruler chooses
    Seniority,              // Oldest dynasty member
    Tanistry,               // Elected from extended family
    OpenSuccession,         // Anyone can claim (usually war)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Personality {
    pub bravery: f32,
    pub cruelty: f32,
    pub ambition: f32,
    pub honor: f32,
    pub piety: f32,
    pub cunning: f32,
    pub charisma: f32,
    pub paranoia: f32,
    pub patience: f32,
    pub greed: f32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum DeathCause {
    Natural,
    Battle,
    Assassination,
    Execution,
    Duel,
    Monster,
    Disease,
    Magic,
    Accident,
    Suicide,
    Unknown,
}
```

### Event System

```rust
/// A historical event with full causality tracking
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub id: EventId,
    pub event_type: EventType,
    pub date: Date,
    
    // Location
    pub location: Option<(usize, usize)>,
    pub region: Option<String>,
    
    // Participants
    pub primary_participants: Vec<EntityId>,
    pub secondary_participants: Vec<EntityId>,
    pub factions_involved: Vec<FactionId>,
    
    // Causality
    pub causes: Vec<EventId>,           // Events that led to this
    pub triggered_by: Option<EventId>,  // Immediate cause
    pub consequences: Vec<Consequence>,
    pub triggered_events: Vec<EventId>, // Events this caused
    
    // Results
    pub outcome: EventOutcome,
    pub artifacts_created: Vec<ArtifactId>,
    pub monuments_created: Vec<MonumentId>,
    pub terrain_changes: Vec<TerrainChange>,
    
    // Description
    pub title: String,
    pub description: String,
    pub is_major: bool,                 // Era-defining event
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EventType {
    // Civilization lifecycle
    FactionFounded,
    FactionDestroyed,
    SettlementFounded,
    SettlementDestroyed,
    SettlementGrew,
    
    // Diplomacy
    TreatySignad,
    TreatyBroken,
    AllianceFormed,
    AllianceBroken,
    TradeRouteEstablished,
    
    // Conflict
    WarDeclared,
    WarEnded,
    BattleFought,
    SiegeBegun,
    SiegeEnded,
    Raid,
    Massacre,
    
    // Politics
    RulerCrowned,
    RulerDeposed,
    SuccessionCrisis,
    Rebellion,
    Coup,
    Assassination,
    
    // Religion
    ReligionFounded,
    Miracle,
    HolyWarDeclared,
    TempleBuilt,
    TempleProfaned,
    CultFormed,
    
    // Monsters
    CreatureAppeared,
    CreatureSlain,
    MonsterRaid,
    LairEstablished,
    LairDestroyed,
    PopulationMigrated,
    
    // Notable figures
    HeroBorn,
    HeroDied,
    QuestBegun,
    QuestCompleted,
    MasterworkCreated,
    
    // Artifacts
    ArtifactCreated,
    ArtifactLost,
    ArtifactFound,
    ArtifactDestroyed,
    
    // Monuments
    MonumentBuilt,
    MonumentDestroyed,
    
    // Terrain
    VolcanoErupted,
    Earthquake,
    Flood,
    Drought,
    Plague,
    MagicalCatastrophe,
    
    // Magic
    SpellInvented,
    MagicalExperiment,
    CurseApplied,
    CurseLifted,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Consequence {
    PopulationChange(FactionId, i32),
    TerritoryChange(FactionId, Vec<(usize, usize)>, bool), // gain or lose
    RelationChange(FactionId, FactionId, i32),
    ResourceChange(FactionId, ResourceType, i32),
    FigureDeath(FigureId, DeathCause),
    ArtifactTransfer(ArtifactId, EntityId, EntityId),
    TerrainModification(TerrainChange),
    SettlementStatusChange(SettlementId, SettlementStatus),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TerrainChange {
    pub location: (usize, usize),
    pub change_type: TerrainChangeType,
    pub new_biome: Option<ExtendedBiome>,
    pub elevation_delta: Option<f32>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum TerrainChangeType {
    VolcanicEruption,
    Flooding,
    Desertification,
    Deforestation,
    Reforestation,
    MagicalCorruption,
    MagicalPurification,
    CraterFormation,
    RiverDiverted,
    LakeFormed,
    LakeDrained,
}
```

### Artifacts and Monuments

```rust
/// A named artifact with history
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Artifact {
    pub id: ArtifactId,
    pub name: String,
    pub item_type: ArtifactType,
    pub material: Vec<ArtifactMaterial>,
    pub description: String,
    
    // Creation
    pub creation_date: Date,
    pub creator: Option<FigureId>,
    pub creation_event: EventId,
    pub creation_location: (usize, usize),
    
    // Properties
    pub quality: ArtifactQuality,
    pub enchantments: Vec<Enchantment>,
    pub decorations: Vec<Decoration>,
    pub inscriptions: Vec<Inscription>,
    
    // History
    pub owner_history: Vec<(EntityId, Date, Date, AcquisitionMethod)>,
    pub current_owner: Option<EntityId>,
    pub current_location: Option<(usize, usize)>,
    pub lost: bool,
    pub destroyed: bool,
    
    // Value
    pub monetary_value: u32,
    pub historical_importance: u32,
    
    // Events
    pub involved_in: Vec<EventId>,
}

/// Inscription on an artifact or monument
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Inscription {
    pub language: LanguageId,
    pub text: String,                   // In procedural language
    pub translation: String,            // Human-readable
    pub refers_to: Vec<EntityId>,       // People/events mentioned
    pub date_inscribed: Date,
}

/// A monument or structure
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Monument {
    pub id: MonumentId,
    pub name: String,
    pub monument_type: MonumentType,
    pub location: (usize, usize),
    
    // Construction
    pub built_date: Date,
    pub builder: Option<FigureId>,
    pub commissioned_by: Option<FigureId>,
    pub faction: FactionId,
    pub construction_event: EventId,
    
    // Purpose
    pub commemorates: Option<EventId>,
    pub honors: Vec<EntityId>,
    pub purpose: MonumentPurpose,
    
    // Physical
    pub materials: Vec<ResourceType>,
    pub size: MonumentSize,
    pub decorations: Vec<Decoration>,
    pub inscriptions: Vec<Inscription>,
    
    // Status
    pub intact: bool,
    pub destruction_date: Option<Date>,
    pub destruction_event: Option<EventId>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum MonumentType {
    Statue,
    Obelisk,
    Tomb,
    Pyramid,
    Temple,
    Castle,
    Wall,
    Tower,
    Bridge,
    Fountain,
    Memorial,
    Trophy,
    Altar,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum MonumentPurpose {
    CommemorateVictory,
    HonorDead,
    ReligiousWorship,
    Defense,
    MarkTerritory,
    CelebratePeace,
    WarnOthers,
    ArtisticExpression,
}
```

### Tile History

```rust
/// Historical data for a world tile
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TileHistory {
    // Current state
    pub current_owner: Option<FactionId>,
    pub settlement: Option<SettlementId>,
    pub lair: Option<LairId>,
    pub resources: Vec<ResourceType>,
    
    // Historical
    pub ownership_history: Vec<(FactionId, Date, Option<Date>)>,
    pub former_settlements: Vec<SettlementId>,
    
    // Objects present
    pub monuments: Vec<MonumentId>,
    pub artifacts_lost_here: Vec<ArtifactId>,
    
    // Events
    pub events: Vec<EventId>,
    pub battles_fought: u32,
    
    // Lore
    pub names_throughout_history: Vec<(String, LanguageId, Date)>,
}

/// The complete history database
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorldHistory {
    pub seed: u64,
    pub world_seed: u64,                // Reference to terrain
    pub config: HistoryConfig,
    pub current_date: Date,
    
    // Time
    pub eras: Vec<Era>,
    
    // Entities
    pub races: HashMap<RaceId, Race>,
    pub cultures: HashMap<CultureId, Culture>,
    pub languages: HashMap<LanguageId, Language>,
    pub factions: HashMap<FactionId, Faction>,
    pub settlements: HashMap<SettlementId, Settlement>,
    pub figures: HashMap<FigureId, Figure>,
    pub dynasties: HashMap<DynastyId, Dynasty>,
    
    // Creatures
    pub creature_species: HashMap<CreatureSpeciesId, CreatureSpecies>,
    pub legendary_creatures: HashMap<LegendaryCreatureId, LegendaryCreature>,
    pub creature_populations: HashMap<PopulationId, CreaturePopulation>,
    
    // Religion
    pub deities: HashMap<DeityId, Deity>,
    pub religions: HashMap<ReligionId, Religion>,
    pub cults: HashMap<CultId, MonsterCult>,
    
    // Objects
    pub artifacts: HashMap<ArtifactId, Artifact>,
    pub monuments: HashMap<MonumentId, Monument>,
    
    // Events
    pub events: Vec<Event>,
    pub events_by_date: BTreeMap<Date, Vec<EventId>>,
    pub events_by_location: HashMap<(usize, usize), Vec<EventId>>,
    
    // Trade
    pub trade_routes: HashMap<TradeRouteId, TradeRoute>,
    
    // Tile data
    pub tile_history: HashMap<(usize, usize), TileHistory>,
    
    // Wars
    pub wars: HashMap<WarId, War>,
    pub treaties: HashMap<TreatyId, Treaty>,
}
```

### History Configuration
```rust
/// Configuration for history simulation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryConfig {
    /// Number of years to simulate (default: 500)
    pub simulation_years: u32,
    
    /// Initial number of civilizations to spawn
    pub initial_civilizations: u32,
    
    /// Initial legendary creatures
    pub initial_legendary_creatures: u32,
    
    /// Event frequency multipliers
    pub war_frequency: f32,
    pub monster_activity: f32,
    pub artifact_creation_rate: f32,
    
    /// Kaiju spawn probability (ultra-rare)
    pub kaiju_spawn_chance: f32,        // Default: 0.001
    
    /// Magic intensity
    pub magic_level: f32,
    
    /// Religion complexity
    pub religion_complexity: f32,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            simulation_years: 500,
            initial_civilizations: 8,
            initial_legendary_creatures: 15,
            war_frequency: 1.0,
            monster_activity: 1.0,
            artifact_creation_rate: 1.0,
            kaiju_spawn_chance: 0.001,
            magic_level: 1.0,
            religion_complexity: 1.0,
        }
    }
}
```

---

## Implementation Phases

### Phase 1: Foundation (Days 1-3)
**Goal**: Core ID system, time, and configuration

- [ ] Create `src/history/mod.rs` with module exports
- [ ] Implement ID types (`FactionId`, `FigureId`, etc.) using newtype pattern
- [ ] Implement `Date`, `Season`, `Era` in `time/`
- [ ] Implement `HistoryConfig` with CLI parameters
- [ ] Add `--history-years` CLI option to main.rs

**Files**:
- `src/history/mod.rs`
- `src/history/config.rs`
- `src/history/time/mod.rs`
- `src/history/time/calendar.rs`
- `src/history/time/timeline.rs`

---

### Phase 2: Naming System (Days 4-5)
**Goal**: Culturally distinct name generation

- [ ] Implement `NamingStyle` struct with traits
- [ ] Implement name generator with style parameters
- [ ] Create 5+ naming archetypes (harsh, flowing, compound, etc.)
- [ ] Generate character, place, and epithet names
- [ ] Test name generation variety and cultural distinctiveness

**Files**:
- `src/history/naming/mod.rs`
- `src/history/naming/styles.rs`
- `src/history/naming/generator.rs`

---

### Phase 3: Race and Culture System (Days 6-8)
**Goal**: Intelligent civilization races

- [ ] Define `RaceType` enum with all base types
- [ ] Implement `CultureValues` with trait ranges
- [ ] Implement `Culture` generation
- [ ] Implement full `Race` struct
- [ ] Create race-biome preference mapping
- [ ] Test race generation variety

**Files**:
- `src/history/entities/mod.rs`
- `src/history/entities/races.rs`
- `src/history/entities/culture.rs`
- `src/history/entities/traits.rs`

---

### Phase 4: Creature Anatomy System (Days 9-12)
**Goal**: Procedural monster generation

- [ ] Implement `BodyPart`, `BodyPartType`, `BodyMaterial`
- [ ] Implement `CreatureSpecies` with anatomy
- [ ] Create creature generation algorithm
- [ ] Implement `CreatureSize` with rarity weights (Kaiju = 0.001)
- [ ] Implement `Intelligence` levels
- [ ] Implement `CreatureBehavior` patterns
- [ ] Create biome-appropriate creature templates
- [ ] Test creature generation variety and coherence

**Files**:
- `src/history/creatures/mod.rs`
- `src/history/creatures/anatomy.rs`
- `src/history/creatures/generator.rs`
- `src/history/creatures/behavior.rs`

---

### Phase 5: Legendary Creatures (Days 13-14)
**Goal**: Unique named monsters

- [ ] Implement `LegendaryCreature` struct
- [ ] Implement unique name generation with epithets
- [ ] Generate legendary creature abilities
- [ ] Place lairs on world map (using biomes)
- [ ] Implement creature territory system
- [ ] Add legendary creature history tracking

**Files**:
- `src/history/creatures/legendary.rs`
- `src/history/creatures/populations.rs`

---

### Phase 6: Civilization Basics (Days 15-18)
**Goal**: Factions and settlements

- [ ] Implement `Faction` struct
- [ ] Implement `Settlement` struct with types
- [ ] Civilization placement algorithm (suitable biomes)
- [ ] Territory claiming (flood-fill from settlements)
- [ ] Population simulation basics
- [ ] Add faction to tile history

**Files**:
- `src/history/civilizations/mod.rs`
- `src/history/civilizations/faction.rs`
- `src/history/civilizations/settlement.rs`
- `src/history/civilizations/territory.rs`

---

### Phase 7: Economy and Trade (Days 19-21)
**Goal**: Resources and trade routes

- [ ] Implement `ResourceType` enum
- [ ] Map resources to biomes
- [ ] Implement `TradeRoute` struct
- [ ] Trade route pathfinding (A* over terrain)
- [ ] Trade value calculation
- [ ] Settlement production

**Files**:
- `src/history/civilizations/economy.rs`

---

### Phase 8: Diplomacy System (Days 22-24)
**Goal**: Faction interactions

- [ ] Implement `DiplomaticRelation` struct
- [ ] Implement `DiplomaticStance` enum
- [ ] Cultural similarity calculation
- [ ] Opinion drift mechanics
- [ ] Treaty system
- [ ] War declaration triggers

**Files**:
- `src/history/civilizations/diplomacy.rs`
- `src/history/civilizations/military.rs`

---

### Phase 9: Religion System (Days 25-27)
**Goal**: Faiths and monster cults

- [ ] Implement `Deity` struct
- [ ] Implement `Religion` struct
- [ ] Implement `MonsterCult` struct
- [ ] Deity generation from domains
- [ ] Monster-based deity creation
- [ ] Temple placement
- [ ] Religious spread mechanics

**Files**:
- `src/history/religion/mod.rs`
- `src/history/religion/deity.rs`
- `src/history/religion/worship.rs`
- `src/history/religion/monster_cults.rs`

---

### Phase 10: Notable Figures (Days 28-31)
**Goal**: Heroes, leaders, lineages

- [ ] Implement `Figure` struct
- [ ] Implement `Dynasty` struct
- [ ] Implement `Personality` traits
- [ ] Figure generation for events
- [ ] Lineage tracking
- [ ] Succession law implementation
- [ ] Death cause variety
- [ ] Relationship system

**Files**:
- `src/history/entities/figures.rs`
- `src/history/entities/lineage.rs`
- `src/history/civilizations/government.rs`

---

### Phase 11: Event System (Days 32-35)
**Goal**: Rich historical events

- [ ] Implement `Event` struct with causality
- [ ] Implement all `EventType` variants
- [ ] Implement `Consequence` system
- [ ] Event generation logic
- [ ] Event chaining (triggers)
- [ ] Major event detection (era-defining)
- [ ] Event description generation

**Files**:
- `src/history/events/mod.rs`
- `src/history/events/types.rs`
- `src/history/events/generator.rs`
- `src/history/events/consequences.rs`
- `src/history/events/causality.rs`
- `src/history/events/chronicle.rs`

---

### Phase 12: Tile History (Days 36-37)
**Goal**: Track historical data per tile

- [ ] Implement `TileHistory` struct
- [ ] Track ownership history per tile
- [ ] Track events that occurred at each tile
- [ ] Track former settlements and monuments
- [ ] Enable location-based history queries

**Files**:
- `src/history/world_state/mod.rs`
- `src/history/world_state/tile_history.rs`

---

### Phase 13: Artifacts and Monuments (Days 38-40)
**Goal**: Objects with encoded history

- [ ] Implement `Artifact` struct
- [ ] Implement `Monument` struct
- [ ] Implement `Inscription` with language
- [ ] Artifact creation triggers
- [ ] Monument placement
- [ ] Ownership tracking
- [ ] Artifact loss/discovery

**Files**:
- `src/history/objects/mod.rs`
- `src/history/objects/artifacts.rs`
- `src/history/objects/monuments.rs`
- `src/history/objects/inscriptions.rs`

---

### Phase 14: Simulation Engine (Days 41-43)
**Goal**: Main simulation loop

- [ ] Implement `HistoryEngine` struct
- [ ] Season-by-season simulation step
- [ ] Event queue processing
- [ ] Faction AI decision making
- [ ] War/battle resolution
- [ ] Population growth/decline
- [ ] Creature population dynamics
- [ ] End condition detection

**Files**:
- `src/history/simulation/mod.rs`
- `src/history/simulation/engine.rs`
- `src/history/simulation/step.rs`

---

### Phase 15: Explorer Integration (Days 44-45)
**Goal**: Faction view mode and markers

- [ ] Add `ViewMode::Factions` to explorer
- [ ] Territory coloring by faction
- [ ] Settlement markers (icons)
- [ ] Trade route rendering (lines)
- [ ] Tile info panel: owner, settlements, events
- [ ] Lair markers for legendary creatures

**Files**:
- Modify `src/explorer.rs`

---

### Phase 16: Legends Mode (Days 46-48)
**Goal**: History exploration UI

- [ ] Create legends mode state machine
- [ ] Entity browser (factions, figures, creatures)
- [ ] Event timeline view
- [ ] Search and filter system
- [ ] Entity detail view
- [ ] "Why did X happen?" query
- [ ] Artifact/monument inspection

**Files**:
- `src/history/legends/mod.rs`
- `src/history/legends/mode.rs`
- `src/history/legends/queries.rs`
- `src/history/legends/renderer.rs`

---

### Phase 17: Simulation Playback (Days 48-50)
**Goal**: Step-by-step viewing

- [ ] Pause/resume simulation
- [ ] Step forward (1 season, 1 year, 10 years)
- [ ] Event log live display
- [ ] Speed controls
- [ ] Current world state snapshot

**Files**:
- `src/history/simulation/playback.rs`

---

### Phase 18: Persistence (Days 50-52)
**Goal**: Save/load history

- [ ] Define history file format (JSON or binary)
- [ ] Serialize `WorldHistory`
- [ ] Reference world seed for terrain
- [ ] Load history and reconstruct state
- [ ] Export legends to readable text/markdown

**Files**:
- `src/history/persistence/mod.rs`
- `src/history/persistence/serialize.rs`
- `src/history/persistence/format.rs`

---

## Estimated Timeline

| Phase | Days | Cumulative |
|-------|------|------------|
| 1. Foundation | 3 | 3 |
| 2. Naming System | 2 | 5 |
| 3. Race/Culture | 3 | 8 |
| 4. Creature Anatomy | 4 | 12 |
| 5. Legendary Creatures | 2 | 14 |
| 6. Civilization Basics | 4 | 18 |
| 7. Economy/Trade | 3 | 21 |
| 8. Diplomacy | 3 | 24 |
| 9. Religion | 3 | 27 |
| 10. Notable Figures | 4 | 31 |
| 11. Event System | 4 | 35 |
| 12. Tile History | 2 | 37 |
| 13. Artifacts/Monuments | 3 | 40 |
| 14. Simulation Engine | 3 | 43 |
| 15. Explorer Integration | 2 | 45 |
| 16. Legends Mode | 3 | 48 |
| 17. Playback | 2 | 50 |
| 18. Persistence | 2 | 52 |

**Total: ~52 development days**

---

## Success Criteria

- [ ] Simulate 500 years of history in < 30 seconds
- [ ] Generate 5+ civilizations with unique cultures
- [ ] Generate 10+ legendary creatures with unique anatomies
- [ ] Produce coherent cause-effect event chains
- [ ] Create artifacts with readable inscriptions
- [ ] Show faction territories in explorer
- [ ] Enable legends mode exploration
- [ ] Support step-by-step simulation viewing
- [ ] Save/load history separately from terrain

---

## Risk Areas

1. **Performance**: Many entities + events = slow simulation
   - Mitigation: Lazy generation, spatial indexing, batch updates

2. **Coherence**: Procedural content may feel random
   - Mitigation: Strong typing, constraint systems, templates

3. **Complexity**: Many interacting systems
   - Mitigation: Phase-by-phase implementation, extensive testing

4. **UI**: Legends mode is complex
   - Mitigation: Start simple, iterate

---

## Next Steps

1. Review this plan for any concerns or modifications
2. Begin Phase 1: Foundation
3. Commit after each phase for easy rollback
