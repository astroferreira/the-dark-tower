//! Z-Level quantized height system
//!
//! Transforms the continuous 2D heightmap into a 3D voxel-like system with discrete Z-levels.
//! Each Z-level represents a quantized elevation floor. Underground levels are filled solid,
//! and surface terrain follows the quantized heightmap.
//!
//! Also includes Dwarf Fortress-style cave system generation with three cavern layers,
//! cave formations (stalactites, stalagmites, pillars), and underground biomes.

use crate::tilemap::Tilemap;
use noise::{NoiseFn, Perlin};
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use rand::SeedableRng;
use std::collections::{HashMap, VecDeque};
use std::cmp::Ordering;

/// Height in meters per Z-level
pub const FLOOR_HEIGHT: f32 = 250.0;

/// Minimum Z-level (deepest)
pub const MIN_Z: i32 = -16;

/// Maximum Z-level (highest)
pub const MAX_Z: i32 = 16;

/// Z-level at sea level (0m elevation)
pub const SEA_LEVEL_Z: i32 = 0;

/// Total number of Z-levels
pub const Z_LEVEL_COUNT: usize = (MAX_Z - MIN_Z + 1) as usize;

// Cave layer constants (Dwarf Fortress style)
/// Cavern 1 (Shallow): Fungal forests, small chambers, cave moss
pub const CAVERN_1_MIN: i32 = -6;
pub const CAVERN_1_MAX: i32 = -3;

/// Cavern 2 (Middle): Large caverns, underground lakes, crystals
pub const CAVERN_2_MIN: i32 = -10;
pub const CAVERN_2_MAX: i32 = -7;

/// Cavern 3 (Deep): Magma pools, huge chambers, exotic features
pub const CAVERN_3_MIN: i32 = -14;
pub const CAVERN_3_MAX: i32 = -11;

/// Minimum rock thickness above a cave layer (tiles between surface and cave ceiling)
pub const MIN_ROCK_ABOVE_CAVE: i32 = 2;

/// Content of a tile at a specific Z-level
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ZTile {
    /// Empty space above surface
    #[default]
    Air,
    /// The visible terrain layer (surface)
    Surface,
    /// Underground (solid rock, no caves yet)
    Solid,
    /// Water at or below sea level
    Water,
    /// Large underground water reservoir
    Aquifer,
    /// Flowing underground water channel
    UndergroundRiver,
    /// Water-filled cave chamber
    WaterCave,
    /// Surface emergence point (underground water meets surface)
    Spring,

    // === Cave Structure ===
    /// Walkable cave floor
    CaveFloor,
    /// Cave wall (for texture variety)
    CaveWall,

    // === Speleothems (Cave Formations) ===
    /// Hanging formation from ceiling
    Stalactite,
    /// Rising formation from floor
    Stalagmite,
    /// Merged stalactite/stalagmite column
    Pillar,
    /// Sheet mineral deposit on floor/walls
    Flowstone,

    // === Cave Biomes ===
    /// Glowing fungi clusters
    FungalGrowth,
    /// Large mushroom (Dwarf Fortress tower caps style)
    GiantMushroom,
    /// Crystal growths
    CrystalFormation,
    /// Bioluminescent moss on walls/floor
    CaveMoss,

    // === Deep Features ===
    /// Molten rock pool
    MagmaPool,
    /// Lava tube passage
    MagmaTube,
    /// Cooled magma floor
    ObsidianFloor,

    // === Water Integration ===
    /// Underground lake in cavern
    CaveLake,
    /// Where water enters cave from above
    Waterfall,

    // === Vertical Passages ===
    /// Passage leading upward (can ascend from here)
    RampUp,
    /// Passage leading downward (can descend from here)
    RampDown,
    /// Passage connecting both up and down
    RampBoth,

    // === Human-Made Structures ===

    // Structure Walls
    /// Stone castle/ruin walls
    StoneWall,
    /// Brick city building walls
    BrickWall,
    /// Wooden cabin/village walls
    WoodWall,
    /// Decayed/ruined wall (partial)
    RuinedWall,

    // Structure Floors
    /// Castle/dungeon stone floors
    StoneFloor,
    /// Building wooden floors
    WoodFloor,
    /// Streets and plazas
    CobblestoneFloor,
    /// Simple earthen floors
    DirtFloor,

    // Structure Features
    /// Doorway (passable)
    Door,
    /// Window opening
    Window,
    /// Stairs going up
    StairsUp,
    /// Stairs going down
    StairsDown,
    /// Decorative pillar
    Column,
    /// Collapsed debris
    Rubble,
    /// Loot/storage container
    Chest,
    /// Temple/altar feature
    Altar,

    // Roads
    /// Simple dirt path
    DirtRoad,
    /// Paved stone road
    StoneRoad,
    /// Bridge over water/gaps
    Bridge,

    // Cave Structures (inside caves)
    /// Carved mine tunnel
    MinedTunnel,
    /// Excavated mine chamber
    MinedRoom,
    /// Wooden mine support beam
    MineSupport,
    /// Light source
    Torch,

    // Mining structures
    /// Vertical mine shaft (connects Z-levels)
    MineShaft,
    /// Ladder for vertical movement
    MineLadder,
    /// Mine cart tracks
    MineRails,
    /// Visible ore vein in wall
    OreVein,
    /// Rich ore deposit
    RichOreVein,
    /// Mine entrance (surface level)
    MineEntrance,
    /// Underground fortress wall (reinforced stone)
    FortressWall,
    /// Underground fortress floor
    FortressFloor,
    /// Fortress gate/portcullis
    FortressGate,
    /// Storage vault
    Vault,
    /// Underground barracks floor
    BarracksFloor,
    /// Forge/smithy area
    ForgeFloor,
    /// Underground cistern (water storage)
    Cistern,

    // === Historical Evidence ===

    // Battlefield evidence
    /// Field of bones from a battle
    BoneField,
    /// Rusted weapons and armor
    RustedWeapons,
    /// Memorial stone for a battle
    WarMemorial,
    /// Impact crater (from siege or cataclysm)
    Crater,

    // Cultural markers
    /// Faction boundary marker
    BoundaryStone,
    /// Distance marker along trade routes
    MileMarker,
    /// Small religious shrine
    Shrine,
    /// Statue of a hero or leader
    Statue,
    /// Tall stone monument
    Obelisk,

    // Monster evidence
    /// Monster's bone nest
    BoneNest,
    /// Spider web cluster
    WebCluster,
    /// Slime trail marking
    SlimeTrail,
    /// Generic territory marking
    TerritoryMarking,
    /// Giant ant mound
    AntMound,
    /// Giant bee hive
    BeeHive,
    /// Claw marks on surface
    ClawMarks,
    /// Cursed/corrupted ground
    CursedGround,
    /// Charred/burned ground (dragon)
    CharredGround,

    // Trade/resource evidence
    /// Abandoned merchant cart
    AbandonedCart,
    /// Ruined waystation
    WaystationRuin,
    /// Dried up well
    DriedWell,
    /// Overgrown garden/farm
    OvergrownGarden,

    // Graveyards
    /// Single gravestone
    Gravestone,
    /// Elaborate tomb
    Tomb,
    /// Large mausoleum building
    Mausoleum,
    /// Bone storage building
    Ossuary,
    /// Mass grave pit
    MassGrave,

    // === Artifact Containers ===

    /// Pedestal displaying a weapon or armor
    ArtifactPedestal,
    /// Treasure chest containing jewelry/treasure
    TreasureChest,
    /// Bookshelf containing tomes
    BookShelf,
    /// Shrine containing relics
    RelicShrine,
    /// Case containing scrolls
    ScrollCase,

    // === Statue Variants ===

    /// Hero statue (commemorating a notable figure)
    HeroStatue,
    /// Damaged/ruined statue
    RuinedStatue,

    // === Dungeon Markers ===

    /// Entrance to a dungeon
    DungeonEntrance,
    /// Monster's treasure hoard pile
    TreasureHoard,
}

impl ZTile {
    /// Check if this tile contains underground water
    #[allow(dead_code)]
    pub fn is_underground_water(&self) -> bool {
        matches!(self,
            ZTile::Aquifer | ZTile::UndergroundRiver | ZTile::WaterCave |
            ZTile::CaveLake | ZTile::Waterfall
        )
    }

    /// Check if this tile is passable by water
    #[allow(dead_code)]
    pub fn is_water_permeable(&self) -> bool {
        matches!(
            self,
            ZTile::Air | ZTile::Water | ZTile::Aquifer |
            ZTile::UndergroundRiver | ZTile::WaterCave | ZTile::Spring |
            ZTile::CaveFloor | ZTile::CaveLake | ZTile::Waterfall
        )
    }

    /// Check if this tile is a cave tile (open space in cave system)
    pub fn is_cave(&self) -> bool {
        matches!(
            self,
            ZTile::CaveFloor | ZTile::CaveWall | ZTile::Stalactite |
            ZTile::Stalagmite | ZTile::Pillar | ZTile::Flowstone |
            ZTile::FungalGrowth | ZTile::GiantMushroom | ZTile::CrystalFormation |
            ZTile::CaveMoss | ZTile::MagmaPool | ZTile::MagmaTube |
            ZTile::ObsidianFloor | ZTile::CaveLake | ZTile::Waterfall |
            ZTile::RampUp | ZTile::RampDown | ZTile::RampBoth
        )
    }

    /// Check if this tile is passable (can walk/move through)
    #[allow(dead_code)]
    pub fn is_passable(&self) -> bool {
        matches!(
            self,
            ZTile::Air | ZTile::CaveFloor | ZTile::Flowstone |
            ZTile::FungalGrowth | ZTile::CaveMoss | ZTile::ObsidianFloor |
            ZTile::MagmaTube | ZTile::Surface |
            ZTile::RampUp | ZTile::RampDown | ZTile::RampBoth |
            // Structure tiles that are passable
            ZTile::StoneFloor | ZTile::WoodFloor | ZTile::CobblestoneFloor |
            ZTile::DirtFloor | ZTile::Door | ZTile::StairsUp | ZTile::StairsDown |
            ZTile::DirtRoad | ZTile::StoneRoad | ZTile::Bridge |
            ZTile::MinedTunnel | ZTile::MinedRoom | ZTile::MineShaft | ZTile::MineLadder |
            ZTile::MineRails | ZTile::MineEntrance |
            ZTile::FortressFloor | ZTile::FortressGate | ZTile::BarracksFloor | ZTile::ForgeFloor
        )
    }

    /// Check if this tile is a human-made structure
    #[allow(dead_code)]
    pub fn is_structure(&self) -> bool {
        matches!(
            self,
            ZTile::StoneWall | ZTile::BrickWall | ZTile::WoodWall | ZTile::RuinedWall |
            ZTile::StoneFloor | ZTile::WoodFloor | ZTile::CobblestoneFloor | ZTile::DirtFloor |
            ZTile::Door | ZTile::Window | ZTile::StairsUp | ZTile::StairsDown |
            ZTile::Column | ZTile::Rubble | ZTile::Chest | ZTile::Altar |
            ZTile::DirtRoad | ZTile::StoneRoad | ZTile::Bridge |
            ZTile::MinedTunnel | ZTile::MinedRoom | ZTile::MineSupport | ZTile::Torch |
            ZTile::MineShaft | ZTile::MineLadder | ZTile::MineRails | ZTile::OreVein |
            ZTile::RichOreVein | ZTile::MineEntrance |
            ZTile::FortressWall | ZTile::FortressFloor | ZTile::FortressGate |
            ZTile::Vault | ZTile::BarracksFloor | ZTile::ForgeFloor | ZTile::Cistern
        )
    }

    /// Check if this tile is a wall (blocks movement)
    #[allow(dead_code)]
    pub fn is_wall(&self) -> bool {
        matches!(
            self,
            ZTile::StoneWall | ZTile::BrickWall | ZTile::WoodWall | ZTile::RuinedWall |
            ZTile::CaveWall | ZTile::Solid | ZTile::Column | ZTile::MineSupport |
            ZTile::FortressWall | ZTile::OreVein | ZTile::RichOreVein
        )
    }

    /// Check if this tile is a road
    #[allow(dead_code)]
    pub fn is_road(&self) -> bool {
        matches!(self, ZTile::DirtRoad | ZTile::StoneRoad | ZTile::Bridge | ZTile::MineRails)
    }

    /// Check if this tile is a floor (passable structure surface)
    #[allow(dead_code)]
    pub fn is_floor(&self) -> bool {
        matches!(
            self,
            ZTile::StoneFloor | ZTile::WoodFloor | ZTile::CobblestoneFloor |
            ZTile::DirtFloor | ZTile::MinedTunnel | ZTile::MinedRoom |
            ZTile::CaveFloor | ZTile::ObsidianFloor | ZTile::Flowstone |
            ZTile::FortressFloor | ZTile::BarracksFloor | ZTile::ForgeFloor
        )
    }

    /// Check if this tile is a mining structure
    #[allow(dead_code)]
    pub fn is_mine(&self) -> bool {
        matches!(
            self,
            ZTile::MinedTunnel | ZTile::MinedRoom | ZTile::MineSupport |
            ZTile::MineShaft | ZTile::MineLadder | ZTile::MineRails |
            ZTile::OreVein | ZTile::RichOreVein | ZTile::MineEntrance
        )
    }

    /// Check if this tile is an underground fortress structure
    #[allow(dead_code)]
    pub fn is_fortress(&self) -> bool {
        matches!(
            self,
            ZTile::FortressWall | ZTile::FortressFloor | ZTile::FortressGate |
            ZTile::Vault | ZTile::BarracksFloor | ZTile::ForgeFloor | ZTile::Cistern
        )
    }

    /// Check if this tile allows vertical movement upward
    #[allow(dead_code)]
    pub fn allows_ascent(&self) -> bool {
        matches!(self, ZTile::RampUp | ZTile::RampBoth | ZTile::MineLadder | ZTile::MineShaft)
    }

    /// Check if this tile allows vertical movement downward
    #[allow(dead_code)]
    pub fn allows_descent(&self) -> bool {
        matches!(self, ZTile::RampDown | ZTile::RampBoth | ZTile::MineLadder | ZTile::MineShaft)
    }

    /// Check if this tile is a cave formation (speleothem)
    #[allow(dead_code)]
    pub fn is_formation(&self) -> bool {
        matches!(
            self,
            ZTile::Stalactite | ZTile::Stalagmite | ZTile::Pillar | ZTile::Flowstone
        )
    }

    /// Check if this tile is a cave biome feature
    #[allow(dead_code)]
    pub fn is_cave_biome(&self) -> bool {
        matches!(
            self,
            ZTile::FungalGrowth | ZTile::GiantMushroom | ZTile::CrystalFormation |
            ZTile::CaveMoss | ZTile::MagmaPool | ZTile::ObsidianFloor
        )
    }

    /// Check if this tile is dangerous (magma, etc.)
    #[allow(dead_code)]
    pub fn is_dangerous(&self) -> bool {
        matches!(self, ZTile::MagmaPool)
    }

    /// Check if this tile can be carved into (solid rock types)
    #[allow(dead_code)]
    pub fn is_carvable(&self) -> bool {
        matches!(self, ZTile::Solid)
    }
}

/// 3D tilemap storing Z-level data
///
/// The coordinate system uses (x, y, z) where:
/// - x: horizontal position (wraps)
/// - y: vertical position on the 2D map (north-south)
/// - z: elevation level (-16 to +16, where 0 is sea level)
#[derive(Clone)]
pub struct Tilemap3D<T> {
    /// Map width in tiles
    pub width: usize,
    /// Map height in tiles
    pub height: usize,
    /// Number of Z-levels
    #[allow(dead_code)]
    pub depth: usize,
    /// Lowest Z-level index
    pub min_z: i32,
    /// Highest Z-level index
    pub max_z: i32,
    /// Internal data storage (x + y * width + z_index * width * height)
    data: Vec<T>,
}

impl<T: Clone + Default> Tilemap3D<T> {
    /// Create a new 3D tilemap with the given dimensions
    pub fn new(width: usize, height: usize, min_z: i32, max_z: i32) -> Self {
        let depth = (max_z - min_z + 1) as usize;
        let size = width * height * depth;
        Self {
            width,
            height,
            depth,
            min_z,
            max_z,
            data: vec![T::default(); size],
        }
    }
}

impl<T: Clone> Tilemap3D<T> {
    /// Create a new 3D tilemap filled with a specific value
    #[allow(dead_code)]
    pub fn new_with(width: usize, height: usize, min_z: i32, max_z: i32, value: T) -> Self {
        let depth = (max_z - min_z + 1) as usize;
        let size = width * height * depth;
        Self {
            width,
            height,
            depth,
            min_z,
            max_z,
            data: vec![value; size],
        }
    }

    /// Get the internal index for a coordinate
    fn index(&self, x: usize, y: usize, z: i32) -> usize {
        let x = x % self.width; // Wrap horizontally
        let z_index = (z - self.min_z) as usize;
        x + y * self.width + z_index * self.width * self.height
    }

    /// Check if a Z-level is valid
    pub fn is_valid_z(&self, z: i32) -> bool {
        z >= self.min_z && z <= self.max_z
    }

    /// Get a reference to the tile at (x, y, z)
    pub fn get(&self, x: usize, y: usize, z: i32) -> &T {
        debug_assert!(self.is_valid_z(z), "Z-level {} out of bounds [{}, {}]", z, self.min_z, self.max_z);
        &self.data[self.index(x, y, z)]
    }

    /// Get a mutable reference to the tile at (x, y, z)
    #[allow(dead_code)]
    pub fn get_mut(&mut self, x: usize, y: usize, z: i32) -> &mut T {
        debug_assert!(self.is_valid_z(z), "Z-level {} out of bounds [{}, {}]", z, self.min_z, self.max_z);
        let idx = self.index(x, y, z);
        &mut self.data[idx]
    }

    /// Set the tile at (x, y, z)
    pub fn set(&mut self, x: usize, y: usize, z: i32, value: T) {
        debug_assert!(self.is_valid_z(z), "Z-level {} out of bounds [{}, {}]", z, self.min_z, self.max_z);
        let idx = self.index(x, y, z);
        self.data[idx] = value;
    }
}

/// Convert continuous height (in meters) to a discrete Z-level
///
/// # Examples
/// ```
/// use planet_generator::zlevel::height_to_z;
/// assert_eq!(height_to_z(0.0), 0);      // Sea level
/// assert_eq!(height_to_z(250.0), 1);    // One level up
/// assert_eq!(height_to_z(-250.0), -1);  // One level down
/// assert_eq!(height_to_z(125.0), 0);    // Still at sea level
/// ```
pub fn height_to_z(height: f32) -> i32 {
    (height / FLOOR_HEIGHT).floor() as i32
}

/// Get the floor elevation (in meters) for a Z-level
///
/// Returns the minimum elevation for that Z-level.
///
/// # Examples
/// ```
/// use planet_generator::zlevel::z_to_height;
/// assert_eq!(z_to_height(0), 0.0);       // Sea level floor
/// assert_eq!(z_to_height(1), 250.0);     // One level up
/// assert_eq!(z_to_height(-1), -250.0);   // One level down
/// ```
pub fn z_to_height(z: i32) -> f32 {
    z as f32 * FLOOR_HEIGHT
}

/// Get the ceiling elevation (in meters) for a Z-level
pub fn z_to_height_ceiling(z: i32) -> f32 {
    (z + 1) as f32 * FLOOR_HEIGHT
}

/// Get human-readable description of a Z-level
pub fn z_level_description(z: i32) -> &'static str {
    match z {
        z if z >= 14 => "Extreme peaks",
        z if z >= 10 => "High mountains",
        z if z >= 6 => "Mountains",
        z if z >= 3 => "Hills",
        z if z >= 1 => "Low land",
        0 => "Sea level",
        -1 => "Shallow water",
        z if z >= -4 => "Ocean shelf",
        z if z >= -8 => "Deep ocean",
        z if z >= -12 => "Ocean floor",
        _ => "Abyss",
    }
}

/// Generate Z-level data from a heightmap
///
/// For each (x, y) position:
/// - Calculate the surface Z-level from the continuous height
/// - Fill tiles below surface with Solid
/// - Set the surface Z-level to Surface
/// - Fill tiles above surface with Air
/// - For underwater positions, fill water above ocean floor up to sea level
///
/// Returns both the 3D tilemap and a 2D map of surface Z-levels.
pub fn generate_zlevels(heightmap: &Tilemap<f32>) -> (Tilemap3D<ZTile>, Tilemap<i32>) {
    let width = heightmap.width;
    let height = heightmap.height;

    let mut zlevels = Tilemap3D::new(width, height, MIN_Z, MAX_Z);
    let mut surface_z = Tilemap::new_with(width, height, 0i32);

    for y in 0..height {
        for x in 0..width {
            let elevation = *heightmap.get(x, y);
            let z_surface = height_to_z(elevation).clamp(MIN_Z, MAX_Z);
            surface_z.set(x, y, z_surface);

            for z in MIN_Z..=MAX_Z {
                let tile = if z > z_surface {
                    // Above surface
                    if z <= SEA_LEVEL_Z && elevation < 0.0 {
                        ZTile::Water // Water above ocean floor up to sea level
                    } else {
                        ZTile::Air
                    }
                } else if z == z_surface {
                    ZTile::Surface
                } else {
                    ZTile::Solid // Underground
                };
                zlevels.set(x, y, z, tile);
            }
        }
    }

    (zlevels, surface_z)
}

/// Generate underground water features (aquifers, rivers, caves, springs)
///
/// This creates a realistic underground water system:
/// - Aquifers form in porous rock where moisture is high
/// - Underground rivers connect aquifers following gradients
/// - Water-filled caves form near aquifers
/// - Springs emerge where underground water meets the surface
pub fn generate_underground_water(
    zlevels: &mut Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
    heightmap: &Tilemap<f32>,
    moisture: &Tilemap<f32>,
    seed: u64,
) {
    let mut rng = ChaCha8Rng::seed_from_u64(seed.wrapping_add(12345));

    // Step 1: Generate aquifers based on moisture and depth
    generate_aquifers(zlevels, surface_z, moisture, &mut rng);

    // Step 2: Carve underground rivers connecting aquifers
    generate_underground_rivers(zlevels, surface_z, heightmap, &mut rng);

    // Step 3: Add water-filled cave chambers near aquifers
    generate_water_caves(zlevels, surface_z, &mut rng);

    // Step 4: Create springs where underground water meets surface
    generate_springs(zlevels, surface_z);
}

/// Generate aquifers based on moisture levels and rock porosity
fn generate_aquifers(
    zlevels: &mut Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
    moisture: &Tilemap<f32>,
    rng: &mut ChaCha8Rng,
) {
    let width = moisture.width;
    let height = moisture.height;

    // Use noise for aquifer distribution (irregular shapes)
    let noise = Perlin::new(rng.gen());

    for y in 0..height {
        for x in 0..width {
            let surf_z = *surface_z.get(x, y);
            let moist = *moisture.get(x, y);

            // Only create aquifers under land (surface above sea level)
            if surf_z <= SEA_LEVEL_Z {
                continue;
            }

            // Aquifer depth range: 3 to 8 levels below surface based on moisture
            let aquifer_depth = ((moist * 5.0) as i32 + 3).min(8);
            let aquifer_z = (surf_z - aquifer_depth).max(MIN_Z);

            // Use noise to create irregular aquifer shapes
            let nx = x as f64 / 50.0;
            let ny = y as f64 / 50.0;
            let noise_val = noise.get([nx, ny]);

            // Higher moisture = higher chance of aquifer
            if noise_val + (moist as f64 * 0.5) > 0.3 {
                // Create aquifer layer (1-3 levels thick based on moisture)
                let thickness = ((moist * 3.0) as i32).max(1);
                for dz in 0..thickness {
                    let z = aquifer_z - dz;
                    if z >= MIN_Z && *zlevels.get(x, y, z) == ZTile::Solid {
                        zlevels.set(x, y, z, ZTile::Aquifer);
                    }
                }
            }
        }
    }
}

/// Generate underground rivers that connect aquifers following terrain gradients
fn generate_underground_rivers(
    zlevels: &mut Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
    heightmap: &Tilemap<f32>,
    rng: &mut ChaCha8Rng,
) {
    let width = heightmap.width;
    let height = heightmap.height;

    // Find all aquifer cells with their positions
    let mut aquifer_cells: Vec<(usize, usize, i32, f32)> = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let surf_z = *surface_z.get(x, y);
            for z in MIN_Z..surf_z {
                if *zlevels.get(x, y, z) == ZTile::Aquifer {
                    let elevation = *heightmap.get(x, y);
                    aquifer_cells.push((x, y, z, elevation));
                    break; // Only count the top aquifer cell per column
                }
            }
        }
    }

    if aquifer_cells.is_empty() {
        return;
    }

    // Sort by elevation (highest first) to trace rivers downhill
    aquifer_cells.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap());

    // Trace rivers from high aquifers to lower ones
    let num_rivers = (aquifer_cells.len() / 100).max(5).min(50);

    for _ in 0..num_rivers {
        if aquifer_cells.is_empty() {
            break;
        }

        // Start from a random high-elevation aquifer
        let start_idx = rng.gen_range(0..aquifer_cells.len().min(20));
        let (mut cx, mut cy, _, _) = aquifer_cells[start_idx];

        // Trace downhill for up to 100 steps
        for _ in 0..100 {
            // Find the lowest neighboring cell
            let mut best_neighbor: Option<(usize, usize, i32)> = None;
            let mut best_height = *heightmap.get(cx, cy);

            for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                let nx = ((cx as i32 + dx).rem_euclid(width as i32)) as usize;
                let ny = (cy as i32 + dy).clamp(0, height as i32 - 1) as usize;
                let nheight = *heightmap.get(nx, ny);
                let nsurf_z = *surface_z.get(nx, ny);

                // Only continue through underground cells
                if nsurf_z <= SEA_LEVEL_Z {
                    continue;
                }

                // Check if this neighbor is lower
                if nheight < best_height {
                    // Find a suitable z-level for the river (1-6 below surface)
                    let river_z = (nsurf_z - rng.gen_range(1..6)).max(MIN_Z);
                    if *zlevels.get(nx, ny, river_z) == ZTile::Solid
                        || *zlevels.get(nx, ny, river_z) == ZTile::Aquifer
                    {
                        best_neighbor = Some((nx, ny, river_z));
                        best_height = nheight;
                    }
                }
            }

            match best_neighbor {
                Some((nx, ny, nz)) => {
                    // Carve underground river
                    zlevels.set(nx, ny, nz, ZTile::UndergroundRiver);
                    cx = nx;
                    cy = ny;
                }
                None => break, // No valid neighbor, end the river
            }
        }
    }
}

/// Generate water-filled cave chambers near aquifers
fn generate_water_caves(
    zlevels: &mut Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
    rng: &mut ChaCha8Rng,
) {
    let width = zlevels.width;
    let height = zlevels.height;

    // Use noise for cave placement
    let cave_noise = Perlin::new(rng.gen());

    for y in 0..height {
        for x in 0..width {
            let surf_z = *surface_z.get(x, y);

            // Only place caves under land
            if surf_z <= SEA_LEVEL_Z {
                continue;
            }

            // Check Z levels 2-5 below surface for cave potential
            for dz in 2..=5 {
                let z = surf_z - dz;
                if z < MIN_Z {
                    continue;
                }

                // Use noise to determine cave placement
                let nx = x as f64 / 30.0;
                let ny = y as f64 / 30.0;
                let nz = z as f64 / 5.0;
                let noise_val = cave_noise.get([nx, ny, nz]);

                // Only place caves near aquifers (check adjacent cells)
                let mut near_water = false;
                for (dx, dy, dz_check) in [
                    (-1, 0, 0), (1, 0, 0), (0, -1, 0), (0, 1, 0),
                    (0, 0, -1), (0, 0, 1),
                ] {
                    let check_x = ((x as i32 + dx).rem_euclid(width as i32)) as usize;
                    let check_y = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;
                    let check_z = z + dz_check;
                    if check_z >= MIN_Z && check_z <= MAX_Z {
                        let tile = *zlevels.get(check_x, check_y, check_z);
                        if tile == ZTile::Aquifer || tile == ZTile::UndergroundRiver {
                            near_water = true;
                            break;
                        }
                    }
                }

                // High noise + near water = water cave
                if near_water && noise_val > 0.4 && *zlevels.get(x, y, z) == ZTile::Solid {
                    zlevels.set(x, y, z, ZTile::WaterCave);
                }
            }
        }
    }
}

/// Generate springs where underground water meets the surface
fn generate_springs(
    zlevels: &mut Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
) {
    let width = zlevels.width;
    let height = zlevels.height;

    for y in 0..height {
        for x in 0..width {
            let surf_z = *surface_z.get(x, y);

            // Only create springs on land
            if surf_z <= SEA_LEVEL_Z {
                continue;
            }

            // Check tile just below surface
            let below_z = surf_z - 1;
            if below_z >= MIN_Z {
                let below_tile = *zlevels.get(x, y, below_z);
                if below_tile.is_underground_water() {
                    // This is a spring emergence point
                    zlevels.set(x, y, surf_z, ZTile::Spring);
                }
            }
        }
    }
}

// =============================================================================
// DWARF FORTRESS-STYLE CAVE SYSTEM GENERATION
// =============================================================================

/// Parameters for a single cavern layer
#[allow(dead_code)]
struct CavernLayerParams {
    /// Layer name for debugging
    name: &'static str,
    /// Z-level range minimum
    z_min: i32,
    /// Z-level range maximum
    z_max: i32,
    /// Noise frequency (lower = larger chambers)
    noise_freq: f64,
    /// Noise threshold for cave carving (lower = more caves)
    noise_threshold: f64,
    /// Minimum chamber size (in tiles) to keep
    min_chamber_size: usize,
    /// Whether this layer has fungal biomes
    has_fungi: bool,
    /// Whether this layer has crystal formations
    has_crystals: bool,
    /// Whether this layer has magma features
    has_magma: bool,
}

/// Represents a cave chamber for connectivity calculations
#[derive(Clone, Debug)]
struct CaveChamber {
    id: usize,
    tiles: Vec<(usize, usize, i32)>,
    centroid: (f64, f64, f64),
    layer: usize,
}

/// Edge for Minimum Spanning Tree
#[derive(Clone, Copy)]
struct Edge {
    from: usize,
    to: usize,
    weight: f64,
}

impl PartialEq for Edge {
    fn eq(&self, other: &Self) -> bool {
        self.weight == other.weight
    }
}

impl Eq for Edge {}

impl PartialOrd for Edge {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Edge {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering for min-heap behavior
        other.weight.partial_cmp(&self.weight).unwrap_or(Ordering::Equal)
    }
}

/// Generate Dwarf Fortress-style cave systems
///
/// Creates three cavern layers with increasing depth:
/// - Cavern 1 (shallow): Small chambers, fungal forests, cave moss
/// - Cavern 2 (middle): Large caverns, underground lakes, crystals
/// - Cavern 3 (deep): Huge chambers, magma pools, exotic features
pub fn generate_caves(
    zlevels: &mut Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
    heightmap: &Tilemap<f32>,
    moisture: &Tilemap<f32>,
    stress_map: &Tilemap<f32>,
    seed: u64,
) {
    let mut rng = ChaCha8Rng::seed_from_u64(seed.wrapping_add(99999));

    // Define cavern layer parameters
    let layers = [
        CavernLayerParams {
            name: "Cavern 1 (Shallow)",
            z_min: CAVERN_1_MIN,
            z_max: CAVERN_1_MAX,
            noise_freq: 0.08,
            noise_threshold: 0.35,
            min_chamber_size: 15,
            has_fungi: true,
            has_crystals: false,
            has_magma: false,
        },
        CavernLayerParams {
            name: "Cavern 2 (Middle)",
            z_min: CAVERN_2_MIN,
            z_max: CAVERN_2_MAX,
            noise_freq: 0.05,
            noise_threshold: 0.30,
            min_chamber_size: 30,
            has_fungi: true,
            has_crystals: true,
            has_magma: false,
        },
        CavernLayerParams {
            name: "Cavern 3 (Deep)",
            z_min: CAVERN_3_MIN,
            z_max: CAVERN_3_MAX,
            noise_freq: 0.03,
            noise_threshold: 0.25,
            min_chamber_size: 50,
            has_fungi: false,
            has_crystals: true,
            has_magma: true,
        },
    ];

    // Phase 1: Carve cavern chambers for each layer
    let mut all_chambers: Vec<CaveChamber> = Vec::new();
    for (layer_idx, layer) in layers.iter().enumerate() {
        let chambers = carve_cavern_layer(
            zlevels,
            surface_z,
            layer,
            layer_idx,
            &mut rng,
        );
        all_chambers.extend(chambers);
    }

    // Phase 2: Connect chambers within each layer with tunnels
    connect_chambers_within_layers(zlevels, &all_chambers, &mut rng);

    // Phase 3: Create vertical passages between layers
    create_vertical_passages(zlevels, &all_chambers, surface_z, &mut rng);

    // Phase 4: Place cave formations (stalactites, stalagmites, pillars)
    place_formations(zlevels, surface_z, &mut rng);

    // Phase 5: Place cave biome features (fungi, crystals, magma)
    for (layer_idx, layer) in layers.iter().enumerate() {
        place_cave_biomes(
            zlevels,
            surface_z,
            moisture,
            stress_map,
            layer,
            layer_idx,
            &mut rng,
        );
    }

    // Phase 6: Integrate with water system
    integrate_caves_with_water(zlevels, surface_z, moisture, &mut rng);

    // Phase 7: Mark vertical passages (ramps) for Z-level navigation
    mark_vertical_passages(zlevels);
}

/// Carve cavern chambers for a single layer using 3D noise + cellular automata
fn carve_cavern_layer(
    zlevels: &mut Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
    params: &CavernLayerParams,
    layer_idx: usize,
    rng: &mut ChaCha8Rng,
) -> Vec<CaveChamber> {
    let width = zlevels.width;
    let height = zlevels.height;

    // Step 1: Generate initial cave regions using 3D Perlin noise
    let noise = Perlin::new(rng.gen::<u32>());
    let noise2 = Perlin::new(rng.gen::<u32>()); // Secondary noise for variation

    // Track which cells are carved (temporary 3D boolean map)
    let mut carved: HashMap<(usize, usize, i32), bool> = HashMap::new();

    for z in params.z_min..=params.z_max {
        for y in 0..height {
            for x in 0..width {
                let surf_z = *surface_z.get(x, y);

                // Only carve under land with sufficient rock above
                if surf_z <= SEA_LEVEL_Z || z > surf_z - MIN_ROCK_ABOVE_CAVE {
                    continue;
                }

                // Skip if not solid rock
                if *zlevels.get(x, y, z) != ZTile::Solid {
                    continue;
                }

                // 3D noise sampling
                let nx = x as f64 * params.noise_freq;
                let ny = y as f64 * params.noise_freq;
                let nz = z as f64 * params.noise_freq * 2.0; // Stretch vertically

                // Combine two noise octaves for more natural shapes
                let noise_val = noise.get([nx, ny, nz]) * 0.7
                    + noise2.get([nx * 2.0, ny * 2.0, nz * 2.0]) * 0.3;

                // Apply threshold with depth variation
                let depth_factor = (z - params.z_min) as f64 / (params.z_max - params.z_min) as f64;
                let threshold = params.noise_threshold - depth_factor * 0.05;

                if noise_val > threshold {
                    carved.insert((x, y, z), true);
                }
            }
        }
    }

    // Step 2: Cellular automata smoothing (4 iterations)
    for _ in 0..4 {
        let mut new_carved: HashMap<(usize, usize, i32), bool> = HashMap::new();

        for z in params.z_min..=params.z_max {
            for y in 0..height {
                for x in 0..width {
                    let surf_z = *surface_z.get(x, y);
                    if surf_z <= SEA_LEVEL_Z || z > surf_z - MIN_ROCK_ABOVE_CAVE {
                        continue;
                    }

                    // Count carved neighbors in 3x3x3 cube
                    let mut carved_neighbors = 0;
                    let mut total_neighbors = 0;

                    for dz in -1i32..=1 {
                        for dy in -1i32..=1 {
                            for dx in -1i32..=1 {
                                if dx == 0 && dy == 0 && dz == 0 {
                                    continue;
                                }

                                let nz = z + dz;
                                if nz < params.z_min || nz > params.z_max {
                                    continue;
                                }

                                let nx = ((x as i32 + dx).rem_euclid(width as i32)) as usize;
                                let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;

                                total_neighbors += 1;
                                if carved.contains_key(&(nx, ny, nz)) {
                                    carved_neighbors += 1;
                                }
                            }
                        }
                    }

                    // Smoothing rule: keep if majority of neighbors are carved
                    let was_carved = carved.contains_key(&(x, y, z));
                    let neighbor_ratio = carved_neighbors as f32 / total_neighbors.max(1) as f32;

                    if was_carved && neighbor_ratio >= 0.3 {
                        new_carved.insert((x, y, z), true);
                    } else if !was_carved && neighbor_ratio >= 0.6 {
                        new_carved.insert((x, y, z), true);
                    }
                }
            }
        }

        carved = new_carved;
    }

    // Step 3: Flood-fill to identify distinct chambers
    let mut chamber_map: HashMap<(usize, usize, i32), usize> = HashMap::new();
    let mut chambers: Vec<CaveChamber> = Vec::new();
    let mut chamber_id = 0;

    for &(x, y, z) in carved.keys() {
        if chamber_map.contains_key(&(x, y, z)) {
            continue;
        }

        // BFS flood fill
        let mut queue = VecDeque::new();
        let mut chamber_tiles = Vec::new();
        queue.push_back((x, y, z));
        chamber_map.insert((x, y, z), chamber_id);

        while let Some((cx, cy, cz)) = queue.pop_front() {
            chamber_tiles.push((cx, cy, cz));

            // Check 6 neighbors (cardinal directions)
            for (dx, dy, dz) in [(-1, 0, 0), (1, 0, 0), (0, -1, 0), (0, 1, 0), (0, 0, -1), (0, 0, 1)] {
                let nz = cz + dz;
                if nz < params.z_min || nz > params.z_max {
                    continue;
                }

                let nx = ((cx as i32 + dx).rem_euclid(width as i32)) as usize;
                let ny = (cy as i32 + dy).clamp(0, height as i32 - 1) as usize;

                if carved.contains_key(&(nx, ny, nz)) && !chamber_map.contains_key(&(nx, ny, nz)) {
                    chamber_map.insert((nx, ny, nz), chamber_id);
                    queue.push_back((nx, ny, nz));
                }
            }
        }

        // Only keep chambers above minimum size
        if chamber_tiles.len() >= params.min_chamber_size {
            // Calculate centroid
            let sum_x: f64 = chamber_tiles.iter().map(|(x, _, _)| *x as f64).sum();
            let sum_y: f64 = chamber_tiles.iter().map(|(_, y, _)| *y as f64).sum();
            let sum_z: f64 = chamber_tiles.iter().map(|(_, _, z)| *z as f64).sum();
            let count = chamber_tiles.len() as f64;

            chambers.push(CaveChamber {
                id: chamber_id,
                tiles: chamber_tiles,
                centroid: (sum_x / count, sum_y / count, sum_z / count),
                layer: layer_idx,
            });
            chamber_id += 1;
        }
    }

    // Step 4: Apply carving to actual zlevel map (only for kept chambers)
    for chamber in &chambers {
        for &(x, y, z) in &chamber.tiles {
            zlevels.set(x, y, z, ZTile::CaveFloor);
        }
    }

    chambers
}

/// Connect chambers within the same layer using minimum spanning tree + tunnels
fn connect_chambers_within_layers(
    zlevels: &mut Tilemap3D<ZTile>,
    chambers: &[CaveChamber],
    rng: &mut ChaCha8Rng,
) {
    // Group chambers by layer
    let mut layers: HashMap<usize, Vec<usize>> = HashMap::new();
    for (idx, chamber) in chambers.iter().enumerate() {
        layers.entry(chamber.layer).or_default().push(idx);
    }

    for (_layer_idx, chamber_indices) in layers {
        if chamber_indices.len() < 2 {
            continue;
        }

        // Build complete graph of distances between chambers
        let mut edges: Vec<Edge> = Vec::new();
        for i in 0..chamber_indices.len() {
            for j in (i + 1)..chamber_indices.len() {
                let c1 = &chambers[chamber_indices[i]];
                let c2 = &chambers[chamber_indices[j]];

                let dx = c1.centroid.0 - c2.centroid.0;
                let dy = c1.centroid.1 - c2.centroid.1;
                let dz = c1.centroid.2 - c2.centroid.2;
                let dist = (dx * dx + dy * dy + dz * dz).sqrt();

                edges.push(Edge {
                    from: chamber_indices[i],
                    to: chamber_indices[j],
                    weight: dist,
                });
            }
        }

        // Kruskal's MST using Union-Find
        let mst_edges = kruskal_mst(&edges, chambers.len());

        // Carve tunnels for MST edges
        for edge in &mst_edges {
            carve_tunnel(
                zlevels,
                &chambers[edge.from],
                &chambers[edge.to],
                rng,
            );
        }

        // Add 20% random secondary connections (loops)
        let extra_count = (mst_edges.len() as f32 * 0.2).ceil() as usize;
        let non_mst_edges: Vec<_> = edges.iter()
            .filter(|e| !mst_edges.iter().any(|m| (m.from == e.from && m.to == e.to) || (m.from == e.to && m.to == e.from)))
            .collect();

        for _ in 0..extra_count.min(non_mst_edges.len()) {
            if !non_mst_edges.is_empty() {
                let edge = non_mst_edges[rng.gen_range(0..non_mst_edges.len())];
                carve_tunnel(
                    zlevels,
                    &chambers[edge.from],
                    &chambers[edge.to],
                    rng,
                );
            }
        }
    }
}

/// Kruskal's minimum spanning tree algorithm using Union-Find
fn kruskal_mst(edges: &[Edge], num_nodes: usize) -> Vec<Edge> {
    let mut result = Vec::new();
    let mut parent: Vec<usize> = (0..num_nodes).collect();
    let mut rank = vec![0usize; num_nodes];

    fn find(parent: &mut [usize], i: usize) -> usize {
        if parent[i] != i {
            parent[i] = find(parent, parent[i]);
        }
        parent[i]
    }

    fn union(parent: &mut [usize], rank: &mut [usize], x: usize, y: usize) {
        let xroot = find(parent, x);
        let yroot = find(parent, y);
        if rank[xroot] < rank[yroot] {
            parent[xroot] = yroot;
        } else if rank[xroot] > rank[yroot] {
            parent[yroot] = xroot;
        } else {
            parent[yroot] = xroot;
            rank[xroot] += 1;
        }
    }

    // Sort edges by weight
    let mut sorted_edges = edges.to_vec();
    sorted_edges.sort_by(|a, b| a.weight.partial_cmp(&b.weight).unwrap_or(Ordering::Equal));

    for edge in sorted_edges {
        let x = find(&mut parent, edge.from);
        let y = find(&mut parent, edge.to);

        if x != y {
            result.push(edge);
            union(&mut parent, &mut rank, x, y);
        }
    }

    result
}

/// Carve a tunnel between two chambers using 3D Bresenham with noise wiggle
fn carve_tunnel(
    zlevels: &mut Tilemap3D<ZTile>,
    from: &CaveChamber,
    to: &CaveChamber,
    rng: &mut ChaCha8Rng,
) {
    let width = zlevels.width;
    let height = zlevels.height;

    // Find closest tiles between the two chambers (to avoid long tunnels through air)
    let (start_tile, end_tile) = find_closest_tiles(from, to);

    // 3D Bresenham line with noise wiggle
    let mut x = start_tile.0 as f64;
    let mut y = start_tile.1 as f64;
    let mut z = start_tile.2 as f64;

    let dx = end_tile.0 as f64 - x;
    let dy = end_tile.1 as f64 - y;
    let dz = end_tile.2 as f64 - z;
    let dist = (dx * dx + dy * dy + dz * dz).sqrt();

    if dist < 1.0 {
        return;
    }

    let steps = dist.ceil() as usize;
    let step_x = dx / steps as f64;
    let step_y = dy / steps as f64;
    let step_z = dz / steps as f64;

    let noise = Perlin::new(rng.gen::<u32>());

    for i in 0..=steps {
        // Add noise wiggle perpendicular to the tunnel direction
        let t = i as f64 / steps as f64;
        let wiggle_x = noise.get([t * 5.0, 0.0, 0.0]) * 1.5;
        let wiggle_y = noise.get([0.0, t * 5.0, 0.0]) * 1.5;

        let tx = ((x + wiggle_x).round() as i32).rem_euclid(width as i32) as usize;
        let ty = ((y + wiggle_y).round() as i32).clamp(0, height as i32 - 1) as usize;
        let tz = z.round() as i32;

        if tz >= MIN_Z && tz <= MAX_Z {
            let current_tile = *zlevels.get(tx, ty, tz);
            // Only carve through solid rock
            if current_tile == ZTile::Solid {
                zlevels.set(tx, ty, tz, ZTile::CaveFloor);
            }

            // Tunnel width: carve adjacent tiles for wider passages
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let nx = ((tx as i32 + dx).rem_euclid(width as i32)) as usize;
                let ny = (ty as i32 + dy).clamp(0, height as i32 - 1) as usize;
                if *zlevels.get(nx, ny, tz) == ZTile::Solid && rng.gen_bool(0.3) {
                    zlevels.set(nx, ny, tz, ZTile::CaveFloor);
                }
            }
        }

        x += step_x;
        y += step_y;
        z += step_z;
    }
}

/// Find the closest pair of tiles between two chambers
fn find_closest_tiles(from: &CaveChamber, to: &CaveChamber) -> ((usize, usize, i32), (usize, usize, i32)) {
    let mut min_dist = f64::MAX;
    let mut best_from = from.tiles[0];
    let mut best_to = to.tiles[0];

    // Sample tiles to avoid O(n*m) complexity for large chambers
    let from_sample: Vec<_> = from.tiles.iter().step_by((from.tiles.len() / 20).max(1)).collect();
    let to_sample: Vec<_> = to.tiles.iter().step_by((to.tiles.len() / 20).max(1)).collect();

    for &f in &from_sample {
        for &t in &to_sample {
            let dx = f.0 as f64 - t.0 as f64;
            let dy = f.1 as f64 - t.1 as f64;
            let dz = f.2 as f64 - t.2 as f64;
            let dist = dx * dx + dy * dy + dz * dz;

            if dist < min_dist {
                min_dist = dist;
                best_from = *f;
                best_to = *t;
            }
        }
    }

    (best_from, best_to)
}

/// Create vertical passages (shafts) connecting cavern layers
fn create_vertical_passages(
    zlevels: &mut Tilemap3D<ZTile>,
    chambers: &[CaveChamber],
    surface_z: &Tilemap<i32>,
    rng: &mut ChaCha8Rng,
) {
    let width = zlevels.width;
    let height = zlevels.height;

    // Find chambers that overlap vertically between layers
    let layer_ranges = [
        (0, CAVERN_1_MIN, CAVERN_1_MAX),
        (1, CAVERN_2_MIN, CAVERN_2_MAX),
        (2, CAVERN_3_MIN, CAVERN_3_MAX),
    ];

    // Create shafts between layers 0-1 and 1-2
    for layer_pair in [(0usize, 1usize), (1usize, 2usize)] {
        let upper_chambers: Vec<_> = chambers.iter().filter(|c| c.layer == layer_pair.0).collect();
        let lower_chambers: Vec<_> = chambers.iter().filter(|c| c.layer == layer_pair.1).collect();

        if upper_chambers.is_empty() || lower_chambers.is_empty() {
            continue;
        }

        // Find positions where chambers align vertically
        let mut shaft_candidates: Vec<(usize, usize)> = Vec::new();

        for upper in &upper_chambers {
            for &(ux, uy, _uz) in &upper.tiles {
                for lower in &lower_chambers {
                    // Check if there's a tile in the lower chamber at similar x,y
                    for &(lx, ly, _lz) in &lower.tiles {
                        if (ux as i32 - lx as i32).abs() <= 3 && (uy as i32 - ly as i32).abs() <= 3 {
                            shaft_candidates.push((ux, uy));
                            break;
                        }
                    }
                }
            }
        }

        // Create 1-3 shafts per layer boundary
        let num_shafts = rng.gen_range(1..=3).min(shaft_candidates.len());
        shaft_candidates.sort();
        shaft_candidates.dedup();

        for _ in 0..num_shafts {
            if shaft_candidates.is_empty() {
                break;
            }

            let idx = rng.gen_range(0..shaft_candidates.len());
            let (sx, sy) = shaft_candidates.remove(idx);

            // Carve vertical shaft
            let upper_z_min = layer_ranges[layer_pair.0].1;
            let lower_z_max = layer_ranges[layer_pair.1].2;

            for z in lower_z_max..=upper_z_min {
                let surf_z = *surface_z.get(sx, sy);
                if z > surf_z - MIN_ROCK_ABOVE_CAVE {
                    continue;
                }

                if *zlevels.get(sx, sy, z) == ZTile::Solid {
                    zlevels.set(sx, sy, z, ZTile::CaveFloor);

                    // Small horizontal variation for natural look
                    for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                        let nx = ((sx as i32 + dx).rem_euclid(width as i32)) as usize;
                        let ny = (sy as i32 + dy).clamp(0, height as i32 - 1) as usize;
                        if *zlevels.get(nx, ny, z) == ZTile::Solid && rng.gen_bool(0.2) {
                            zlevels.set(nx, ny, z, ZTile::CaveFloor);
                        }
                    }
                }
            }
        }
    }

    // Ensure at least one path from Cavern 1 to surface (natural cave entrance)
    let cavern1_chambers: Vec<_> = chambers.iter().filter(|c| c.layer == 0).collect();
    if !cavern1_chambers.is_empty() {
        // Pick a random cavern 1 chamber and create an entrance shaft
        let entrance_chamber = cavern1_chambers[rng.gen_range(0..cavern1_chambers.len())];
        let entrance_tile = entrance_chamber.tiles[rng.gen_range(0..entrance_chamber.tiles.len())];

        let surf_z = *surface_z.get(entrance_tile.0, entrance_tile.1);
        if surf_z > SEA_LEVEL_Z && entrance_tile.2 < surf_z - MIN_ROCK_ABOVE_CAVE {
            // Carve from surface down to the cave
            for z in entrance_tile.2..surf_z {
                if *zlevels.get(entrance_tile.0, entrance_tile.1, z) == ZTile::Solid {
                    zlevels.set(entrance_tile.0, entrance_tile.1, z, ZTile::CaveFloor);
                }
            }
        }
    }
}

/// Place cave formations (stalactites, stalagmites, pillars, flowstone)
fn place_formations(
    zlevels: &mut Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
    rng: &mut ChaCha8Rng,
) {
    let width = zlevels.width;
    let height = zlevels.height;

    let noise = Perlin::new(rng.gen::<u32>());

    // Iterate through all cave floors
    for z in MIN_Z..MAX_Z {
        for y in 0..height {
            for x in 0..width {
                let tile = *zlevels.get(x, y, z);
                if tile != ZTile::CaveFloor {
                    continue;
                }

                let surf_z = *surface_z.get(x, y);
                if z > surf_z - MIN_ROCK_ABOVE_CAVE {
                    continue;
                }

                // Use noise for formation placement (more likely near edges)
                let nx = x as f64 * 0.1;
                let ny = y as f64 * 0.1;
                let nz = z as f64 * 0.2;
                let noise_val = noise.get([nx, ny, nz]);

                // Check if this is an edge tile (has solid neighbor)
                let is_edge = has_solid_neighbor(zlevels, x, y, z, width, height);

                // Higher chance of formations at edges
                let formation_chance = if is_edge { 0.15 } else { 0.03 };

                if noise_val > 0.3 && rng.gen_bool(formation_chance) {
                    // Check ceiling (tile above)
                    let has_ceiling = z + 1 <= MAX_Z &&
                        matches!(*zlevels.get(x, y, z + 1), ZTile::Solid | ZTile::Surface);

                    // Check floor stability
                    let has_floor = z - 1 >= MIN_Z &&
                        matches!(*zlevels.get(x, y, z - 1), ZTile::Solid);

                    // Determine formation type
                    if has_ceiling && has_floor && rng.gen_bool(0.3) {
                        // Pillar (merged formation)
                        zlevels.set(x, y, z, ZTile::Pillar);
                    } else if has_ceiling && rng.gen_bool(0.5) {
                        // Stalactite (hanging from ceiling)
                        zlevels.set(x, y, z, ZTile::Stalactite);
                    } else if rng.gen_bool(0.6) {
                        // Stalagmite (rising from floor)
                        zlevels.set(x, y, z, ZTile::Stalagmite);
                    } else {
                        // Flowstone (sheet deposit)
                        zlevels.set(x, y, z, ZTile::Flowstone);
                    }
                }
            }
        }
    }
}

/// Check if a tile has a solid neighbor (used for edge detection)
fn has_solid_neighbor(
    zlevels: &Tilemap3D<ZTile>,
    x: usize,
    y: usize,
    z: i32,
    width: usize,
    height: usize,
) -> bool {
    for (dx, dy, dz) in [(-1, 0, 0), (1, 0, 0), (0, -1, 0), (0, 1, 0), (0, 0, -1), (0, 0, 1)] {
        let nz = z + dz;
        if nz < MIN_Z || nz > MAX_Z {
            continue;
        }

        let nx = ((x as i32 + dx).rem_euclid(width as i32)) as usize;
        let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;

        if *zlevels.get(nx, ny, nz) == ZTile::Solid {
            return true;
        }
    }
    false
}

/// Place cave biome features (fungi, crystals, magma) based on layer
fn place_cave_biomes(
    zlevels: &mut Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
    moisture: &Tilemap<f32>,
    stress_map: &Tilemap<f32>,
    params: &CavernLayerParams,
    _layer_idx: usize,
    rng: &mut ChaCha8Rng,
) {
    let width = zlevels.width;
    let height = zlevels.height;

    let fungi_noise = Perlin::new(rng.gen::<u32>());
    let crystal_noise = Perlin::new(rng.gen::<u32>());
    let magma_noise = Perlin::new(rng.gen::<u32>());

    for z in params.z_min..=params.z_max {
        for y in 0..height {
            for x in 0..width {
                let tile = *zlevels.get(x, y, z);
                if tile != ZTile::CaveFloor {
                    continue;
                }

                let surf_z = *surface_z.get(x, y);
                if z > surf_z - MIN_ROCK_ABOVE_CAVE {
                    continue;
                }

                let moist = *moisture.get(x, y);
                let stress = *stress_map.get(x, y);

                // Fungal growth (moist areas in layers 1 and 2)
                if params.has_fungi && moist > 0.4 {
                    let noise_val = fungi_noise.get([x as f64 * 0.05, y as f64 * 0.05, z as f64 * 0.1]);
                    if noise_val > 0.3 {
                        if rng.gen_bool(0.15) {
                            zlevels.set(x, y, z, ZTile::GiantMushroom);
                        } else if rng.gen_bool(0.25) {
                            zlevels.set(x, y, z, ZTile::FungalGrowth);
                        } else if rng.gen_bool(0.3) {
                            zlevels.set(x, y, z, ZTile::CaveMoss);
                        }
                    }
                }

                // Crystal formations (layers 2 and 3)
                if params.has_crystals {
                    let noise_val = crystal_noise.get([x as f64 * 0.03, y as f64 * 0.03, z as f64 * 0.08]);
                    if noise_val > 0.5 && rng.gen_bool(0.08) {
                        zlevels.set(x, y, z, ZTile::CrystalFormation);
                    }
                }

                // Magma features (layer 3 only, influenced by tectonic stress)
                if params.has_magma && stress > 0.3 {
                    let noise_val = magma_noise.get([x as f64 * 0.04, y as f64 * 0.04, z as f64 * 0.1]);
                    let magma_chance = stress * 0.15;

                    if noise_val > 0.4 {
                        if rng.gen_bool(magma_chance as f64 * 0.5) {
                            zlevels.set(x, y, z, ZTile::MagmaPool);
                        } else if rng.gen_bool(magma_chance as f64) {
                            zlevels.set(x, y, z, ZTile::ObsidianFloor);
                        } else if rng.gen_bool(0.05) {
                            zlevels.set(x, y, z, ZTile::MagmaTube);
                        }
                    }
                }
            }
        }
    }
}

/// Integrate cave system with underground water features
fn integrate_caves_with_water(
    zlevels: &mut Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
    moisture: &Tilemap<f32>,
    rng: &mut ChaCha8Rng,
) {
    let width = zlevels.width;
    let height = zlevels.height;

    // Pass 1: Convert some cave floors to cave lakes in low-lying moist areas (Cavern 2)
    for z in CAVERN_2_MIN..=CAVERN_2_MAX {
        for y in 0..height {
            for x in 0..width {
                let tile = *zlevels.get(x, y, z);
                if tile != ZTile::CaveFloor {
                    continue;
                }

                let moist = *moisture.get(x, y);
                let surf_z = *surface_z.get(x, y);

                // Low areas within layer are more likely to flood
                let depth_in_layer = (z - CAVERN_2_MIN) as f32 / (CAVERN_2_MAX - CAVERN_2_MIN) as f32;

                if moist > 0.5 && depth_in_layer < 0.4 && rng.gen_bool(0.1) {
                    zlevels.set(x, y, z, ZTile::CaveLake);
                }
            }
        }
    }

    // Pass 2: Create waterfalls where aquifers touch cave chambers
    for z in MIN_Z..MAX_Z {
        for y in 0..height {
            for x in 0..width {
                let tile = *zlevels.get(x, y, z);
                if !tile.is_cave() {
                    continue;
                }

                // Check if aquifer or underground river above
                if z + 1 <= MAX_Z {
                    let above = *zlevels.get(x, y, z + 1);
                    if matches!(above, ZTile::Aquifer | ZTile::UndergroundRiver | ZTile::WaterCave) {
                        if rng.gen_bool(0.3) {
                            zlevels.set(x, y, z, ZTile::Waterfall);
                        }
                    }
                }
            }
        }
    }
}

/// Mark vertical passages (ramps) where movement between Z-levels is possible
///
/// Only places a few ramps per cave region to mark key transition points,
/// not every tile that could theoretically connect.
fn mark_vertical_passages(zlevels: &mut Tilemap3D<ZTile>) {
    let width = zlevels.width;
    let height = zlevels.height;

    // Grid size for limiting ramps (one ramp per grid cell max)
    const GRID_SIZE: usize = 20;

    // Track which grid cells already have a ramp at each z-level
    // Key: (grid_x, grid_y, z) -> has_ramp
    let mut ramp_grid: HashMap<(usize, usize, i32), bool> = HashMap::new();

    // Collect candidate ramp positions with their type
    let mut candidates: Vec<(usize, usize, i32, ZTile, bool)> = Vec::new(); // x, y, z, tile_type, is_edge

    for z in MIN_Z..=MAX_Z {
        for y in 0..height {
            for x in 0..width {
                let tile = *zlevels.get(x, y, z);

                // Only check cave floor tiles
                if tile != ZTile::CaveFloor {
                    continue;
                }

                // Check if there's passable cave space above
                let can_go_up = if z + 1 <= MAX_Z {
                    let above = *zlevels.get(x, y, z + 1);
                    is_passable_cave_tile(above)
                } else {
                    false
                };

                // Check if there's passable cave space below
                let can_go_down = if z - 1 >= MIN_Z {
                    let below = *zlevels.get(x, y, z - 1);
                    is_passable_cave_tile(below)
                } else {
                    false
                };

                if !can_go_up && !can_go_down {
                    continue;
                }

                // Determine ramp type
                let ramp_type = match (can_go_up, can_go_down) {
                    (true, true) => ZTile::RampBoth,
                    (true, false) => ZTile::RampUp,
                    (false, true) => ZTile::RampDown,
                    _ => continue,
                };

                // Check if this is an "edge" tile (has solid rock neighbor) - preferred for ramps
                let is_edge = has_solid_neighbor(zlevels, x, y, z, width, height);

                candidates.push((x, y, z, ramp_type, is_edge));
            }
        }
    }

    // Sort candidates: prefer edge tiles, then by position for consistency
    candidates.sort_by(|a, b| {
        // Edge tiles first
        b.4.cmp(&a.4)
            .then_with(|| a.2.cmp(&b.2)) // then by z
            .then_with(|| a.1.cmp(&b.1)) // then by y
            .then_with(|| a.0.cmp(&b.0)) // then by x
    });

    // Place ramps, limiting to one per grid cell per z-level
    for (x, y, z, ramp_type, _is_edge) in candidates {
        let grid_x = x / GRID_SIZE;
        let grid_y = y / GRID_SIZE;
        let grid_key = (grid_x, grid_y, z);

        // Skip if this grid cell already has a ramp
        if ramp_grid.contains_key(&grid_key) {
            continue;
        }

        // Place the ramp
        zlevels.set(x, y, z, ramp_type);
        ramp_grid.insert(grid_key, true);
    }
}

/// Check if a tile is passable for vertical movement purposes
fn is_passable_cave_tile(tile: ZTile) -> bool {
    matches!(tile,
        ZTile::CaveFloor | ZTile::RampUp | ZTile::RampDown | ZTile::RampBoth |
        ZTile::FungalGrowth | ZTile::CaveMoss | ZTile::Flowstone |
        ZTile::ObsidianFloor | ZTile::MagmaTube | ZTile::Air | ZTile::Surface
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_height_to_z() {
        assert_eq!(height_to_z(0.0), 0);
        assert_eq!(height_to_z(249.0), 0);
        assert_eq!(height_to_z(250.0), 1);
        assert_eq!(height_to_z(500.0), 2);
        assert_eq!(height_to_z(-1.0), -1);
        assert_eq!(height_to_z(-250.0), -1);
        assert_eq!(height_to_z(-251.0), -2);
    }

    #[test]
    fn test_z_to_height() {
        assert_eq!(z_to_height(0), 0.0);
        assert_eq!(z_to_height(1), 250.0);
        assert_eq!(z_to_height(-1), -250.0);
        assert_eq!(z_to_height(16), 4000.0);
        assert_eq!(z_to_height(-16), -4000.0);
    }

    #[test]
    fn test_tilemap3d_basic() {
        let mut map: Tilemap3D<ZTile> = Tilemap3D::new(10, 10, MIN_Z, MAX_Z);

        // Default should be Air
        assert_eq!(*map.get(0, 0, 0), ZTile::Air);

        // Set and get
        map.set(5, 5, 0, ZTile::Surface);
        assert_eq!(*map.get(5, 5, 0), ZTile::Surface);

        // Horizontal wrapping
        map.set(0, 0, 0, ZTile::Solid);
        assert_eq!(*map.get(10, 0, 0), ZTile::Solid); // Wraps
    }

    #[test]
    fn test_generate_zlevels() {
        let mut heightmap = Tilemap::new_with(4, 4, 0.0f32);

        // Set various elevations
        heightmap.set(0, 0, 100.0);   // Low land (Z=0)
        heightmap.set(1, 0, 500.0);   // Hills (Z=2)
        heightmap.set(2, 0, -500.0);  // Underwater (Z=-2)
        heightmap.set(3, 0, 1000.0);  // Mountains (Z=4)

        let (zlevels, surface_z) = generate_zlevels(&heightmap);

        // Check surface levels
        assert_eq!(*surface_z.get(0, 0), 0);
        assert_eq!(*surface_z.get(1, 0), 2);
        assert_eq!(*surface_z.get(2, 0), -2);
        assert_eq!(*surface_z.get(3, 0), 4);

        // Check tile types
        assert_eq!(*zlevels.get(0, 0, 0), ZTile::Surface);
        assert_eq!(*zlevels.get(0, 0, 1), ZTile::Air);
        assert_eq!(*zlevels.get(0, 0, -1), ZTile::Solid);

        // Check underwater location
        assert_eq!(*zlevels.get(2, 0, -2), ZTile::Surface);  // Ocean floor
        assert_eq!(*zlevels.get(2, 0, -1), ZTile::Water);    // Water above floor
        assert_eq!(*zlevels.get(2, 0, 0), ZTile::Water);     // Water at sea level
        assert_eq!(*zlevels.get(2, 0, 1), ZTile::Air);       // Air above water
    }
}
