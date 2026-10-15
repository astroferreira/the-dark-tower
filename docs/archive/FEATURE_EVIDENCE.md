# Feature Evidence: Enhanced Simulation Realism

This document demonstrates the new features added to increase simulation realism:
- Fauna System (28 animal species)
- Colonist Daily Routines
- Character Relationships
- Enhanced Local Map Features

---

## 1. Fauna System

### Overview
A comprehensive wildlife system that populates the world with passive and neutral animals. Animals have realistic behaviors including grazing, fleeing, breeding, and migration.

### Species (28 total)

| Category | Species | Characteristics |
|----------|---------|-----------------|
| **Forest** | Deer, Rabbit, Squirrel, Boar, Fox | Herbivores & predators |
| **Plains** | Bison, Horse, Elk, PrairieDog | Herd animals |
| **Mountain** | MountainGoat, Eagle, Marmot | High-altitude adapted |
| **Arctic** | ArcticHare, Caribou, Seal, Penguin | Cold-resistant |
| **Desert** | Camel, Lizard, Vulture | Heat-tolerant |
| **Swamp** | Frog, Heron, Alligator | Amphibious |
| **Tropical** | Monkey, Parrot, Tapir | Jungle dwellers |
| **Aquatic** | Fish, Salmon, Crab | Water-bound |

### Implementation Location
- `src/simulation/fauna/types.rs` - Core types and species definitions
- `src/simulation/fauna/behavior.rs` - AI behavior system
- `src/simulation/fauna/mod.rs` - FaunaManager and integration

### Code Example: Fauna Entity Structure
```rust
// From src/simulation/fauna/types.rs:17-40
pub struct Fauna {
    pub id: FaunaId,
    pub species: FaunaSpecies,
    pub location: TileCoord,
    pub local_position: GlobalLocalCoord,
    pub health: f32,
    pub max_health: f32,
    pub home_range_center: TileCoord,
    pub home_range_radius: usize,
    pub state: FaunaState,
    pub age: u32,
    pub hunger: f32,
    pub is_male: bool,
    pub last_breed_tick: u64,
    pub current_activity: FaunaActivity,
    pub last_action_tick: u64,
}
```

### Code Example: Species Stats (Deer)
```rust
// From src/simulation/fauna/types.rs:268-281
FaunaSpecies::Deer => FaunaStats {
    health: 40.0,
    speed: 1.5,
    home_range: 10,
    herd_size_min: 3,
    herd_size_max: 8,
    alertness: 0.7,
    food_value: 30.0,
    material_value: 15.0,
    maturity_age: 8,
    breed_cooldown: 16,
    offspring_count: 1,
    diet: FaunaDiet::Herbivore,
},
```

### Fauna Activities
```rust
pub enum FaunaActivity {
    Grazing,    // Eating grass/plants
    Resting,    // Idle/sleeping
    Running,    // Fleeing from threats
    Migrating,  // Moving to new territory
    Breeding,   // Reproduction
    Hunting,    // Predators only
    Swimming,   // Aquatic movement
    Foraging,   // Searching for food
}
```

### Behavior System
The fauna AI processes each tick with the following state machine:
1. **Check for threats** - Detect nearby monsters, hunters, or hostile tribes
2. **Flee if threatened** - Move away from danger at increased speed
3. **Grazing/Foraging** - When hungry, search for food
4. **Breeding** - When conditions allow (adult, well-fed, potential mates nearby)
5. **Migration** - Seasonal movement or when resources deplete
6. **Idle/Resting** - Default state when no other needs

---

## 2. Colonist Daily Routines

### Overview
Colonists now follow realistic daily schedules based on time of day, their role, and life stage. This makes the simulation visually engaging with characters going about their daily lives.

### Implementation Location
- `src/simulation/colonists/routines.rs` - Time system and activity determination

### Time of Day System
```rust
// From src/simulation/colonists/routines.rs:17-24
pub enum TimeOfDay {
    Dawn,      // 6-8 AM - Waking up
    Morning,   // 8-12 PM - Work time
    Midday,    // 12-2 PM - Lunch/social
    Afternoon, // 2-6 PM - Work time
    Evening,   // 6-10 PM - Social/leisure
    Night,     // 10 PM - 6 AM - Sleep
}
```

### Detailed Activities (45+ types)
```rust
// From src/simulation/colonists/routines.rs:58-124
pub enum DetailedActivity {
    // Rest activities
    Sleeping, WakingUp, Resting,

    // Personal care
    Eating, Drinking, Bathing, Dressing,

    // Work activities
    Farming, Mining, Woodcutting, Hunting, Fishing,
    Building, Crafting, Smithing, Healing, Researching,
    Guarding, Patrolling, Scouting, Training,

    // Social activities
    Talking, Trading, Teaching, Learning,
    Celebrating, Mourning, Praying, Storytelling,

    // Movement
    Walking, Running, Riding, Swimming,

    // Recreation
    Playing, Relaxing, DrinkingSocially, Gambling, Singing, Dancing,

    // Leadership
    Commanding, Judging, Planning, Inspecting,

    // Child activities
    BeingCaredFor, PlayingGames, Exploring,

    // Elder activities
    Advising, Reminiscing, MentoringYouth,
}
```

### Activity Descriptions for Display
```rust
// From src/simulation/colonists/routines.rs:129-180
DetailedActivity::Farming => "working the fields",
DetailedActivity::Smithing => "working the forge",
DetailedActivity::Healing => "tending to the sick",
DetailedActivity::Praying => "praying at the shrine",
DetailedActivity::Storytelling => "telling stories",
DetailedActivity::Dancing => "dancing",
DetailedActivity::MentoringYouth => "mentoring the young",
```

### Role-Based Schedules
Different roles have different daily activities during work hours:

| Role | Primary Activities |
|------|-------------------|
| **Leader** | Commanding, Planning, Inspecting, Judging |
| **Champion** | Training, Patrolling, Commanding |
| **Priest** | Praying, Teaching, Healing |
| **Council Member** | Planning, Inspecting, Trading |
| **Farmer** | Farming |
| **Smith** | Smithing |
| **Guard** | Guarding, Patrolling |
| **Scout** | Scouting |

### Life Stage Variations

**Children:**
- Morning/Afternoon: PlayingGames, Learning, Exploring, BeingCaredFor
- Evening: PlayingGames, Eating

**Elders:**
- Morning: Advising, MentoringYouth, Praying, Resting
- Afternoon: Storytelling, Reminiscing, Teaching, Resting
- Evening: Storytelling, Talking, Resting

---

## 3. Character Relationships

### Overview
Colonists form meaningful relationships with each other, creating a social web that adds depth to the simulation. Relationships have types, opinions, familiarity levels, and memories.

### Implementation Location
- `src/simulation/colonists/relationships.rs` - Relationship tracking and social interactions

### Relationship Types
```rust
// From src/simulation/colonists/relationships.rs:13-36
pub enum RelationshipType {
    Parent,        // +40 opinion base
    Child,         // +40 opinion base
    Sibling,       // +25 opinion base
    Spouse,        // +60 opinion base
    Lover,         // +45 opinion base
    CloseFriend,   // +35 opinion base
    Friend,        // +20 opinion base
    Acquaintance,  // +5 opinion base
    Rival,         // -15 opinion base
    Enemy,         // -40 opinion base
    Mentor,        // +25 opinion base
    Student,       // +25 opinion base
}
```

### Relationship Structure
```rust
// From src/simulation/colonists/relationships.rs:57-71
pub struct Relationship {
    pub other_id: ColonistId,
    pub relationship_type: RelationshipType,
    pub opinion: i32,              // -100 to 100
    pub familiarity: u32,          // 0 to 100
    pub last_interaction_tick: u64,
    pub memories: Vec<RelationshipMemory>,
}
```

### Memory Events
```rust
// From src/simulation/colonists/relationships.rs:134-150
pub enum MemoryEvent {
    FirstMeeting,
    SharedMeal,
    GoodConversation,
    Argument,
    HelpedInNeed,
    Betrayal,
    SharedVictory,
    SharedLoss,
    GaveGift,
    ReceivedGift,
    WorkedTogether,
    FoughtTogether,
    SavedLife,
    Marriage,
    ChildBirth,
    Death,
}
```

### Relationship Mechanics
- **Opinion**: Ranges from -100 (hatred) to +100 (love)
- **Familiarity**: How well they know each other (0-100)
- **Decay**: Relationships decay after 20+ ticks without interaction
- **Memories**: Last 10 significant events are remembered
- **Romance**: Can develop between compatible adults based on attraction

---

## 4. Enhanced Local Map Features

### Overview
Local maps now contain 28 new feature types, including animal-related features, civilization structures, and natural details that make each area unique.

### Implementation Location
- `src/local/terrain.rs` - LocalFeature enum and properties
- `src/local/entities.rs` - Entity tracking on local maps

### New Feature Categories

#### Animal-Related Features
```rust
AnimalDen,       // Lair for wildlife
BirdNest,        // Tree-dwelling birds
Beehive,         // Source of honey
AnimalTrail,     // Worn paths (easier movement)
WateringHole,    // Gathering point for fauna
BurrowEntrance,  // Small animal homes
```

#### Civilization Features
```rust
Signpost,        // Trail markers
WellStructure,   // Water source (blocks movement)
FenceSection,    // Property boundaries (blocks movement)
Scarecrow,       // Farmland decoration
HayBale,         // Agricultural storage
Firepit,         // Outdoor cooking/warmth
StorageShed,     // Supply storage (blocks movement)
WatchTower,      // Defensive structure (blocks movement)
Bridge,          // River crossing (easier movement)
Dock,            // Water access point
```

#### Natural Details
```rust
FallenLog,       // Decaying tree
MossyRock,       // Weathered stone
Termitemound,    // Insect colony
AntHill,         // Small mound
Wildflowers,     // Colorful plants
BerryBush,       // Food source
HerbPatch,       // Medicinal plants
Driftwood,       // Coastal debris
```

### Movement Cost Modifiers
```rust
// From src/local/terrain.rs (movement_cost_modifier)
AnimalTrail => -0.2,  // Easier to walk on
Bridge => -0.5,       // Much easier to cross
BerryBush => 0.3,     // Slight obstacle
FallenLog => 0.4,     // Moderate obstacle
WateringHole => 0.5,  // Muddy edges
```

### Entity Tracking on Local Maps
```rust
// From src/local/entities.rs:36-58
pub enum LocalEntity {
    Colonist {
        id: ColonistId,
        name: String,
        activity: ColonistActivityState,
        activity_description: String,
        position: LocalPosition,
    },
    Monster {
        id: MonsterId,
        species: MonsterSpecies,
        state: MonsterState,
        health_percent: f32,
        position: LocalPosition,
    },
    Fauna {
        id: FaunaId,
        species: FaunaSpecies,
        activity: FaunaActivity,
        health_percent: f32,
        position: LocalPosition,
    },
}
```

### Entity Display Characters
```rust
// Colonist display by activity state
'@' - Idle
'W' - Working
'T' - Traveling
'R' - Returning
'S' - Socializing
'!' - Fleeing
'P' - Patrolling
'c' - Scouting

// Fauna display by species
'd' - Deer
'r' - Rabbit
'B' - Bison
'E' - Elk
'e' - Eagle
'~' - Fish
'A' - Alligator
// ... and 21 more
```

---

## 5. Integration with Simulation

### SimulationState Integration
```rust
// From src/simulation/simulation.rs:63-66
pub struct SimulationState {
    // ... existing fields ...
    pub fauna: FaunaManager,  // NEW: Wildlife tracking
    // ...
}
```

### Simulation Statistics
```rust
// From src/simulation/simulation.rs:96-98
pub total_fauna_spawned: u32,
pub total_fauna_hunted: u32,
pub current_fauna_count: u32,
```

### Tick Processing
Each simulation tick now processes:
1. Colonist routines (time-of-day activities)
2. Colonist relationships (social interactions)
3. Fauna behavior (grazing, fleeing, breeding)
4. Fauna spawning (biome-appropriate species)

---

## 6. Visual Demonstration

### Local Map with Entities
```
╔════════════════════════════════════════════════╗
║  Forest Clearing - Spring                       ║
╠════════════════════════════════════════════════╣
║  ♣ ♣ · ♣ ♣ · · · · ▲ ▲ · · · · ♣ ♣ ♣           ║
║  ♣ · · · · d d · · · ▲ · · · · · ♣ ·           ║
║  · · @ · · d · · · · · · · · · · · ·   <- Colonist (idle)
║  · · · · · · · · ○ · · · · · · · · ·   <- WateringHole
║  ♣ · · · · · · · · · · · r r · · · ·   <- Rabbits
║  ♣ ♣ · W · · · · · · · · · · · · ♣ ·   <- Colonist (working)
║  · · · · · · ◎ · · · · · · · · · · ·   <- AnimalDen
║  ♣ ♣ · · · · · · · · · ✿ · · · ♣ ♣ ·   <- BerryBush
╠════════════════════════════════════════════════╣
║  Entities: 2 colonists, 4 animals               ║
║  Features: watering hole, animal den, berry bush║
╚════════════════════════════════════════════════╝
```

### Entity Summary Output
```
gather_local_entities() returns:
- John (working the fields)
- Mary (socializing)
- Deer (grazing, 100% health)
- Deer (grazing, 85% health)
- Rabbit (resting, 100% health)
- Rabbit (running, 70% health)  <- Fleeing!
```

---

## 7. Files Changed

| File | Change Type | Description |
|------|-------------|-------------|
| `src/simulation/fauna/types.rs` | **NEW** | 28 species, stats, colors, behaviors |
| `src/simulation/fauna/behavior.rs` | **NEW** | AI state machine, threat detection |
| `src/simulation/fauna/mod.rs` | **NEW** | FaunaManager, spawning, integration |
| `src/simulation/colonists/routines.rs` | **NEW** | Time system, 45+ activities |
| `src/simulation/colonists/relationships.rs` | **NEW** | Relationships, memories, romance |
| `src/local/entities.rs` | **NEW** | Entity tracking for local maps |
| `src/local/terrain.rs` | **MODIFIED** | 28 new LocalFeature variants |
| `src/local/mod.rs` | **MODIFIED** | Export new modules |
| `src/simulation/colonists/mod.rs` | **MODIFIED** | Export routines & relationships |
| `src/simulation/mod.rs` | **MODIFIED** | Export fauna module |
| `src/simulation/simulation.rs` | **MODIFIED** | Integrate fauna, add stats |

---

## 8. Testing

Build verification:
```bash
cargo build --release
# Compiles successfully with only warnings (unused imports)
```

Run simulation:
```bash
cargo run --release -- --simulate --sim-ticks 20 --sim-tribes 5 --seed 42
```

Launch explorer:
```bash
cargo run --release -- --seed 42 --explore
# Press Enter on a tile to see local map with entities
# Press Shift+S to start simulation
# Watch colonists and fauna move about
```
