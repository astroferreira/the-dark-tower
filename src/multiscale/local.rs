//! Local-scale chunk generation (Dwarf Fortress style).
//!
//! Local chunks are embark sites: 48×48 tiles per world tile, with full z-level
//! geology from surface down to magma sea. Generation derives directly from world data.

use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use noise::{NoiseFn, Perlin};

use crate::biomes::ExtendedBiome;
use crate::world::WorldData;
use crate::zlevel::{self, ZTile};

use super::LOCAL_SIZE;
use super::coords::{LocalCoord, chunk_seed};
use super::geology::{GeologyParams, derive_geology, biome_soil_type, biome_surface_material};

/// Material types for local tiles
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Material {
    #[default]
    Air,
    Grass,
    Dirt,
    Sand,
    Mud,
    Ice,
    Snow,
    Stone,
    Water,
    Magma,
}

/// Soil types for underground
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SoilType {
    #[default]
    Loam,
    Clay,
    Sand,
    Silt,
    Peat,
    Gravel,
    Permafrost,
    Ash,
}

/// Stone types for deeper underground
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum StoneType {
    #[default]
    Limestone,
    Granite,
    Sandstone,
    Slate,
    Marble,
    Basalt,
    Obsidian,
    Shale,
}

/// Features that can be placed on local tiles
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LocalFeature {
    /// No special feature
    #[default]
    None,
    /// Tree (type varies by biome)
    Tree { height: u8 },
    /// Bush/shrub
    Bush,
    /// Boulder/large rock
    Boulder,
    /// Door (can be opened/closed)
    Door { open: bool },
    /// Chest/container
    Chest,
    /// Altar or shrine
    Altar,
    /// Stairs going up
    StairsUp,
    /// Stairs going down
    StairsDown,
    /// Ramp up (natural slope)
    RampUp,
    /// Ramp down (natural slope)
    RampDown,
    /// Ladder (vertical movement)
    Ladder,
    /// Torch/light source
    Torch,
    /// Pillar (structural or natural)
    Pillar,
    /// Rubble/debris
    Rubble,
    /// Stalactite (hanging formation)
    Stalactite,
    /// Stalagmite (floor formation)
    Stalagmite,
    /// Mushroom (surface or cave)
    Mushroom,
    /// Giant mushroom (cave)
    GiantMushroom,
    /// Furniture/misc
    Table,
    Chair,
    Bed,
    Bookshelf,
    Barrel,
    WeaponRack,
    Fountain,
    Well,
    Statue,
    /// Traps
    Trap { hidden: bool },
    /// Lever (for mechanisms)
    Lever { active: bool },
    /// Crystal formation
    Crystal,
    /// Ore vein
    OreVein,
}

impl LocalFeature {
    /// Check if this feature blocks movement
    pub fn is_blocking(&self) -> bool {
        matches!(
            self,
            LocalFeature::Tree { .. }
                | LocalFeature::Boulder
                | LocalFeature::Pillar
                | LocalFeature::Stalactite
                | LocalFeature::Stalagmite
                | LocalFeature::Crystal
        )
    }

    /// Check if this feature provides light
    pub fn is_light_source(&self) -> bool {
        matches!(self, LocalFeature::Torch | LocalFeature::Crystal)
    }

    /// Check if this feature allows vertical movement
    pub fn allows_vertical(&self) -> Option<bool> {
        match self {
            LocalFeature::StairsUp | LocalFeature::RampUp => Some(true),
            LocalFeature::StairsDown | LocalFeature::RampDown => Some(false),
            LocalFeature::Ladder => Some(true), // Both directions
            _ => None,
        }
    }
}

/// Terrain type at local scale
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LocalTerrain {
    /// Empty space (sky, open cavern)
    #[default]
    Air,
    /// Soil (various types)
    Soil { soil_type: SoilType },
    /// Stone (various types)
    Stone { stone_type: StoneType },
    /// Grass surface
    Grass,
    /// Sand surface
    Sand,
    /// Mud/swamp surface
    Mud,
    /// Ice/frozen surface
    Ice,
    /// Snow cover
    Snow,
    /// Gravel/loose rock
    Gravel,
    /// Dense vegetation
    DenseVegetation,
    /// Water (shallow)
    ShallowWater,
    /// Water (deep)
    DeepWater,
    /// Flowing water (river)
    FlowingWater,
    /// Magma/lava
    Magma,
    /// Lava (alias)
    Lava,
    /// Cave floor (open underground space)
    CaveFloor,
    /// Cave wall
    CaveWall,
    /// Stone floor (constructed)
    StoneFloor,
    /// Dirt floor
    DirtFloor,
    /// Wood floor
    WoodFloor,
    /// Cobblestone
    Cobblestone,
    /// Stone wall
    StoneWall,
    /// Brick wall
    BrickWall,
    /// Wood wall
    WoodWall,
    /// Constructed floor (various materials)
    ConstructedFloor { material: Material },
    /// Constructed wall
    ConstructedWall { material: Material },
}

impl LocalTerrain {
    /// Check if this terrain is passable (can walk through)
    pub fn is_passable(&self) -> bool {
        matches!(
            self,
            LocalTerrain::Air
                | LocalTerrain::Grass
                | LocalTerrain::Sand
                | LocalTerrain::Mud
                | LocalTerrain::Ice
                | LocalTerrain::Snow
                | LocalTerrain::ShallowWater
                | LocalTerrain::CaveFloor
                | LocalTerrain::ConstructedFloor { .. }
        )
    }

    /// Check if this terrain is solid (blocks movement)
    pub fn is_solid(&self) -> bool {
        matches!(
            self,
            LocalTerrain::Soil { .. }
                | LocalTerrain::Stone { .. }
                | LocalTerrain::ConstructedWall { .. }
        )
    }

    /// Check if this terrain is water
    pub fn is_water(&self) -> bool {
        matches!(
            self,
            LocalTerrain::ShallowWater | LocalTerrain::DeepWater | LocalTerrain::FlowingWater
        )
    }

    /// Check if this terrain is dangerous
    pub fn is_dangerous(&self) -> bool {
        matches!(self, LocalTerrain::Magma | LocalTerrain::DeepWater)
    }
}

/// A single tile at local scale with full geology
#[derive(Clone, Copy, Debug, Default)]
pub struct LocalTile {
    /// Terrain type
    pub terrain: LocalTerrain,
    /// Feature on this tile
    pub feature: LocalFeature,
    /// Material (for rendering/interaction)
    pub material: Material,
    /// Temperature (for frozen water, etc.)
    pub temperature: f32,
    /// Light level (0-255, 0 = dark, 255 = bright)
    pub light: u8,
    /// Is this tile visible (for fog of war)
    pub visible: bool,
    /// Has this tile been explored
    pub explored: bool,
}

impl LocalTile {
    /// Create a new local tile with terrain and material
    pub fn new(terrain: LocalTerrain, material: Material) -> Self {
        Self {
            terrain,
            feature: LocalFeature::None,
            material,
            temperature: 15.0,
            light: 0,
            visible: false,
            explored: false,
        }
    }

    /// Create air tile
    pub fn air() -> Self {
        Self::new(LocalTerrain::Air, Material::Air)
    }

    /// Create a soil tile
    pub fn soil(soil_type: SoilType) -> Self {
        Self::new(LocalTerrain::Soil { soil_type }, Material::Dirt)
    }

    /// Create a stone tile
    pub fn stone(stone_type: StoneType) -> Self {
        Self::new(LocalTerrain::Stone { stone_type }, Material::Stone)
    }

    /// Create a surface tile from material
    pub fn surface(material: Material) -> Self {
        let terrain = match material {
            Material::Grass => LocalTerrain::Grass,
            Material::Sand => LocalTerrain::Sand,
            Material::Mud => LocalTerrain::Mud,
            Material::Ice | Material::Snow => LocalTerrain::Ice,
            Material::Water => LocalTerrain::ShallowWater,
            Material::Stone => LocalTerrain::CaveFloor, // Exposed rock
            _ => LocalTerrain::Grass,
        };
        Self::new(terrain, material)
    }

    /// Check if this tile is passable
    pub fn is_passable(&self) -> bool {
        self.terrain.is_passable() && !self.feature.is_blocking()
    }
}

/// A local chunk representing an embark site (48×48 per world tile).
///
/// Contains full z-level data from MIN_Z to MAX_Z.
#[derive(Clone)]
pub struct LocalChunk {
    /// World tile X coordinate (embark location)
    pub world_x: usize,
    /// World tile Y coordinate (embark location)
    pub world_y: usize,
    /// Tile data: [z][y][x] layout for cache-friendly z-level access
    pub tiles: Vec<LocalTile>,
    /// Minimum z-level
    pub z_min: i16,
    /// Maximum z-level
    pub z_max: i16,
    /// Surface z-level (where the ground is)
    pub surface_z: i16,
    /// Geology parameters used for generation
    pub geology: GeologyParams,
    /// Whether this chunk has been generated
    pub generated: bool,
}

impl LocalChunk {
    /// Create a new empty local chunk
    pub fn new(world_x: usize, world_y: usize, surface_z: i16) -> Self {
        let z_min = zlevel::MIN_Z as i16;
        let z_max = zlevel::MAX_Z as i16;
        let z_count = (z_max - z_min + 1) as usize;
        let total_tiles = LOCAL_SIZE * LOCAL_SIZE * z_count;

        Self {
            world_x,
            world_y,
            tiles: vec![LocalTile::default(); total_tiles],
            z_min,
            z_max,
            surface_z,
            geology: GeologyParams {
                surface_z,
                biome: ExtendedBiome::TemperateGrassland,
                temperature: 15.0,
                moisture: 0.5,
                stress: 0.0,
                is_volcanic: false,
                water_body_type: crate::water_bodies::WaterBodyType::None,
                soil_depth: 4,
                primary_stone: StoneType::Limestone,
                secondary_stone: StoneType::Sandstone,
                has_caverns: [false; 3],
                has_magma: false,
                aquifer_z: None,
            },
            generated: false,
        }
    }

    /// Get the z-level count
    pub fn z_count(&self) -> usize {
        (self.z_max - self.z_min + 1) as usize
    }

    /// Get index into tile array
    #[inline]
    fn index(&self, x: usize, y: usize, z: i16) -> usize {
        debug_assert!(x < LOCAL_SIZE, "x out of bounds");
        debug_assert!(y < LOCAL_SIZE, "y out of bounds");
        debug_assert!(z >= self.z_min && z <= self.z_max, "z out of bounds");

        let z_index = (z - self.z_min) as usize;
        z_index * LOCAL_SIZE * LOCAL_SIZE + y * LOCAL_SIZE + x
    }

    /// Get a tile at local coordinates
    #[inline]
    pub fn get(&self, x: usize, y: usize, z: i16) -> &LocalTile {
        &self.tiles[self.index(x, y, z)]
    }

    /// Get a mutable tile at local coordinates
    #[inline]
    pub fn get_mut(&mut self, x: usize, y: usize, z: i16) -> &mut LocalTile {
        let idx = self.index(x, y, z);
        &mut self.tiles[idx]
    }

    /// Set a tile at local coordinates
    #[inline]
    pub fn set(&mut self, x: usize, y: usize, z: i16, tile: LocalTile) {
        let idx = self.index(x, y, z);
        self.tiles[idx] = tile;
    }

    /// Approximate memory size in bytes
    pub fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.tiles.len() * std::mem::size_of::<LocalTile>()
    }

    /// Get the coordinate for a position in this chunk
    pub fn coord_at(&self, x: usize, y: usize, z: i16) -> LocalCoord {
        LocalCoord::new(self.world_x, self.world_y, x as u8, y as u8, z)
    }

    /// Check if a z-level is above ground
    pub fn is_above_ground(&self, z: i16) -> bool {
        z > self.surface_z
    }

    /// Check if a z-level is at the surface
    pub fn is_surface(&self, z: i16) -> bool {
        z == self.surface_z
    }

    /// Check if a z-level is underground
    pub fn is_underground(&self, z: i16) -> bool {
        z < self.surface_z
    }
}

/// Monster lair type for categorizing different lair features
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LairType {
    /// Spider webs and cocoons
    WebCluster,
    /// Bone piles and animal remains
    BoneNest,
    /// Slime trails and goo
    SlimeTrail,
    /// Ant mound with tunnels
    AntMound,
    /// Bee hive structure
    BeeHive,
    /// Generic territory marking
    Generic,
}

/// Detected structure type at a world tile
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StructureType {
    None,
    // Major underground structures
    Dungeon { depth: i16 },
    Mine { depth: i16 },
    Cave { has_entrance: bool },
    UndergroundFortress { depth: i16 },
    // Surface structures
    Village,
    Building,
    Castle,
    // Minor features
    Graveyard,
    Battlefield,
    MonsterLair { lair_type: LairType },
    Shrine,
    Ruins,
    // Water features
    Spring,
    Waterfall,
    UndergroundLake,
}

/// Detect what kind of structure exists at a world tile by checking WorldHistory metadata.
/// Returns a list of detected structures with their z-levels (priority-ordered).
///
/// Structures are no longer stored in zlevels - only metadata in WorldHistory registries.
/// This function reads from those registries to determine what to generate locally.
fn detect_all_structures(world: &WorldData, world_x: usize, world_y: usize) -> Vec<(StructureType, i16)> {
    let surface_z = *world.surface_z.get(world_x, world_y) as i16;
    let mut structures = Vec::new();

    // Check WorldHistory for structures at this location
    if let Some(ref history) = world.history {
        // Check for dungeons
        if let Some(_dungeon_id) = history.dungeons.dungeons_by_location.get(&(world_x, world_y)) {
            if let Some(dungeon) = history.dungeons.dungeons.get(_dungeon_id) {
                let depth = dungeon.depth_min as i16;
                structures.push((StructureType::Dungeon { depth }, depth));
            }
        }

        // Check for settlements (villages, cities)
        for settlement in history.territories.settlements.values() {
            // Check if this tile is within the settlement area
            let dx = (settlement.x as i32 - world_x as i32).abs();
            let dy = (settlement.y as i32 - world_y as i32).abs();
            if dx <= 2 && dy <= 2 {
                // Settlement found at or near this tile
                structures.push((StructureType::Village, surface_z));
                break;
            }
        }

        // Check for monster lairs
        for lair in history.monsters.lairs.values() {
            if lair.x == world_x && lair.y == world_y {
                use crate::history::monsters::MonsterSpecies;
                let lair_type = match lair.species {
                    MonsterSpecies::GiantSpider => LairType::WebCluster,
                    MonsterSpecies::Troll | MonsterSpecies::Ogre |
                    MonsterSpecies::Werewolf | MonsterSpecies::Dragon => LairType::BoneNest,
                    MonsterSpecies::CaveCrawler | MonsterSpecies::DeepWorm => LairType::SlimeTrail,
                    MonsterSpecies::GiantAnt => LairType::AntMound,
                    MonsterSpecies::GiantBee => LairType::BeeHive,
                    _ => LairType::Generic,
                };
                structures.push((StructureType::MonsterLair { lair_type }, lair.z as i16));
            }
        }

        // Check for battle sites (graveyards and battlefields)
        for event in history.timeline.events.values() {
            if let Some((ex, ey)) = event.location {
                if ex == world_x && ey == world_y {
                    match event.event_type {
                        crate::history::timeline::EventType::Battle |
                        crate::history::timeline::EventType::Siege |
                        crate::history::timeline::EventType::Massacre => {
                            structures.push((StructureType::Battlefield, surface_z));
                        }
                        crate::history::timeline::EventType::MonumentBuilt => {
                            structures.push((StructureType::Shrine, surface_z));
                        }
                        _ => {}
                    }
                }
            }
        }

        // Check settlements for graveyards (near non-thriving settlements)
        for settlement in history.territories.settlements.values() {
            use crate::history::types::SettlementState;
            if settlement.state != SettlementState::Thriving {
                // Graveyard offset from settlement
                let graveyard_x = settlement.x.saturating_add(2);
                let graveyard_y = settlement.y.saturating_add(1);
                if world_x == graveyard_x && world_y == graveyard_y {
                    structures.push((StructureType::Graveyard, surface_z));
                }
            }
        }
    }

    // Still scan zlevels for NATURAL features (caves, water) - these are still generated there
    let mut found_cave = false;
    let mut found_spring = false;
    let mut found_waterfall = false;
    let mut found_cave_lake = false;

    for z in zlevel::MIN_Z..=zlevel::MAX_Z {
        let ztile = *world.zlevels.get(world_x, world_y, z);
        let z16 = z as i16;

        match ztile {
            // === Cave markers (natural, still in zlevels) ===
            ZTile::CaveFloor | ZTile::CaveWall | ZTile::Stalactite |
            ZTile::Stalagmite | ZTile::Pillar | ZTile::Flowstone |
            ZTile::FungalGrowth | ZTile::GiantMushroom | ZTile::CrystalFormation |
            ZTile::CaveMoss | ZTile::MagmaPool | ZTile::MagmaTube |
            ZTile::ObsidianFloor | ZTile::RampUp | ZTile::RampDown | ZTile::RampBoth => {
                if !found_cave {
                    let has_entrance = z >= (surface_z as i32) - 2;
                    structures.push((StructureType::Cave { has_entrance }, z16));
                    found_cave = true;
                }
            }

            // === Water features (natural, still in zlevels) ===
            ZTile::Spring => {
                if !found_spring {
                    structures.push((StructureType::Spring, z16));
                    found_spring = true;
                }
            }
            ZTile::Waterfall => {
                if !found_waterfall {
                    structures.push((StructureType::Waterfall, z16));
                    found_waterfall = true;
                }
            }
            ZTile::CaveLake | ZTile::WaterCave => {
                if !found_cave_lake {
                    structures.push((StructureType::UndergroundLake, z16));
                    found_cave_lake = true;
                }
            }

            _ => {}
        }
    }

    structures
}

/// Get the primary structure type at a world tile (for backward compatibility)
fn detect_structure_type(world: &WorldData, world_x: usize, world_y: usize) -> (StructureType, i16) {
    let structures = detect_all_structures(world, world_x, world_y);

    // Return the first (highest priority) structure, or None
    if let Some(&(structure_type, z)) = structures.first() {
        (structure_type, z)
    } else {
        let surface_z = *world.surface_z.get(world_x, world_y) as i16;
        (StructureType::None, surface_z)
    }
}

/// Generate a local chunk directly from world data.
///
/// This is the main entry point for local map generation. It derives geology
/// from the world tile and generates the full z-column for each (x, y) position.
pub fn generate_local_chunk(
    world: &WorldData,
    world_x: usize,
    world_y: usize,
) -> LocalChunk {
    use super::biome_terrain::{get_biome_config, AdjacentBiomes, generate_blended_biome_surface, add_blended_biome_features};

    let geology = derive_geology(world, world_x, world_y);
    let seed = chunk_seed(world.seed, world_x, world_y);
    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    let mut chunk = LocalChunk::new(world_x, world_y, geology.surface_z);
    chunk.geology = geology.clone();

    // Detect structures at this world tile
    let (structure_type, structure_z) = detect_structure_type(world, world_x, world_y);

    // Create noise generators for terrain variation
    let surface_noise = Perlin::new(rng.gen());
    let cave_noise = Perlin::new(rng.gen());

    // Get biome configuration for this tile
    let biome_config = get_biome_config(geology.biome);

    // Get adjacent biomes for blending
    let adjacent_biomes = AdjacentBiomes::from_world(&world.biomes, world_x, world_y);

    // Generate terrain using the biome-specific blended system
    generate_blended_biome_surface(&mut chunk, &geology, &biome_config, &adjacent_biomes, &surface_noise, &mut rng);

    // Generate underground (caves, etc.) for each column
    for y in 0..LOCAL_SIZE {
        for x in 0..LOCAL_SIZE {
            generate_underground_column(
                &mut chunk,
                x,
                y,
                world,
                &geology,
                &cave_noise,
                &mut rng,
            );
        }
    }

    // Detect ALL structures at this world tile
    let all_structures = detect_all_structures(world, world_x, world_y);

    // Track if we should add surface features (skip if major structure present)
    let mut has_major_structure = false;

    // Helper to find actual local surface z at a given (x, y) position
    // This accounts for terrain variation from biome generation
    let find_local_surface = |chunk: &LocalChunk, x: usize, y: usize| -> i16 {
        for z in (chunk.z_min..=chunk.z_max).rev() {
            let tile = chunk.get(x, y, z);
            // Surface is the highest non-air, non-water passable tile
            if tile.terrain != LocalTerrain::Air && !tile.terrain.is_water() {
                return z;
            }
        }
        geology.surface_z // Fallback
    };

    // Extract surface_z before the loop to avoid borrow checker issues
    let surface_z = chunk.surface_z;

    // Process each detected structure
    for (structure_type, structure_z) in &all_structures {
        match structure_type {
            StructureType::Dungeon { depth } => {
                has_major_structure = true;
                // Generate multi-level dungeon starting below surface
                let dungeon_start = (*depth).min(geology.surface_z - 3);
                let num_levels = rng.gen_range(3..6);
                let entrance_pos = super::structures::generate_dungeon_levels(&mut chunk, dungeon_start, num_levels, &mut rng);

                // Add entrance stairs at the ACTUAL local surface
                if let Some((entrance_x, entrance_y)) = entrance_pos {
                    let local_surface = find_local_surface(&chunk, entrance_x, entrance_y);

                    // Clear terrain at entrance and place stone floor with stairs
                    chunk.set(entrance_x, entrance_y, local_surface,
                        LocalTile::new(LocalTerrain::StoneFloor, Material::Stone));
                    chunk.get_mut(entrance_x, entrance_y, local_surface).feature = LocalFeature::StairsDown;

                    // Carve down to first dungeon level (create connecting shaft if needed)
                    for z in (dungeon_start + 1)..local_surface {
                        let mut tile = LocalTile::new(LocalTerrain::StoneFloor, Material::Stone);
                        if z == dungeon_start + 1 {
                            tile.feature = LocalFeature::StairsUp;
                        } else {
                            tile.feature = LocalFeature::StairsDown;
                        }
                        chunk.set(entrance_x, entrance_y, z, tile);
                    }

                    // Ensure first dungeon level has stairs up
                    chunk.get_mut(entrance_x, entrance_y, dungeon_start).feature = LocalFeature::StairsUp;
                }
            }
            StructureType::UndergroundFortress { depth } => {
                has_major_structure = true;
                let fortress_start = (*depth).min(geology.surface_z - 3);
                let num_levels = rng.gen_range(3..6);
                let entrance_pos = super::structures::generate_dungeon_levels(&mut chunk, fortress_start, num_levels, &mut rng);

                if let Some((entrance_x, entrance_y)) = entrance_pos {
                    let local_surface = find_local_surface(&chunk, entrance_x, entrance_y);

                    chunk.set(entrance_x, entrance_y, local_surface,
                        LocalTile::new(LocalTerrain::StoneFloor, Material::Stone));
                    chunk.get_mut(entrance_x, entrance_y, local_surface).feature = LocalFeature::StairsDown;

                    for z in (fortress_start + 1)..local_surface {
                        let mut tile = LocalTile::new(LocalTerrain::StoneFloor, Material::Stone);
                        tile.feature = if z == fortress_start + 1 {
                            LocalFeature::StairsUp
                        } else {
                            LocalFeature::StairsDown
                        };
                        chunk.set(entrance_x, entrance_y, z, tile);
                    }

                    chunk.get_mut(entrance_x, entrance_y, fortress_start).feature = LocalFeature::StairsUp;
                }
            }
            StructureType::Mine { depth: _ } => {
                has_major_structure = true;
                // Generate mine with entrance building and underground tunnels
                let num_levels = rng.gen_range(3..6);
                super::structures::generate_mine(&mut chunk, surface_z, num_levels, &mut rng);
            }
            StructureType::Cave { has_entrance } => {
                // Copy cave data from world zlevels and expand
                copy_cave_from_world(&mut chunk, world, world_x, world_y, &geology);
                if *has_entrance {
                    // Add cave entrance at surface
                    add_cave_entrance(&mut chunk, surface_z, *structure_z, &mut rng);
                }
            }
            StructureType::Village => {
                has_major_structure = true;
                // Generate village with buildings, roads, and plaza
                super::structures::generate_village(&mut chunk, surface_z, &mut rng);
            }
            StructureType::Castle => {
                has_major_structure = true;
                // Generate castle/fortress with walls, towers, and keep
                super::structures::generate_castle(&mut chunk, surface_z, &mut rng);
            }
            StructureType::Building => {
                has_major_structure = true;
                // Single building - use BSP for interior
                super::structures::generate_dungeon_bsp(&mut chunk, surface_z, &mut rng);
            }
            StructureType::Graveyard => {
                super::structures::generate_graveyard(&mut chunk, surface_z, &mut rng);
            }
            StructureType::Battlefield => {
                super::structures::generate_battlefield(&mut chunk, surface_z, &mut rng);
            }
            StructureType::MonsterLair { lair_type } => {
                super::structures::generate_monster_lair(&mut chunk, surface_z, *lair_type, &mut rng);
            }
            StructureType::Shrine => {
                super::structures::generate_shrine(&mut chunk, surface_z, &mut rng);
            }
            StructureType::Ruins => {
                super::structures::generate_ruins(&mut chunk, surface_z, &mut rng);
            }
            StructureType::Spring => {
                generate_spring_feature(&mut chunk, surface_z, &mut rng);
            }
            StructureType::Waterfall => {
                generate_waterfall_feature(&mut chunk, *structure_z, &mut rng);
            }
            StructureType::UndergroundLake => {
                // Underground lake is handled by cave copy
            }
            StructureType::None => {
                // No structure
            }
        }
    }

    // Add surface features (trees, boulders, etc.) only if no major structure
    // Uses biome-specific blended features
    if !has_major_structure {
        add_blended_biome_features(&mut chunk, &geology, &biome_config, &adjacent_biomes, &mut rng);
    }

    // Add cave features (stalactites, crystals, etc.)
    add_cave_features(&mut chunk, &geology, &mut rng);

    chunk.generated = true;
    chunk
}

/// Sample 3D cave noise for cave generation
fn sample_3d_cave(
    cave_noise: &Perlin,
    x: usize,
    y: usize,
    z: i16,
    world_x: usize,
    world_y: usize,
) -> f64 {
    // Create a unique 3D coordinate that incorporates world position
    let nx = (world_x as f64 * LOCAL_SIZE as f64 + x as f64) * 0.05;
    let ny = (world_y as f64 * LOCAL_SIZE as f64 + y as f64) * 0.05;
    let nz = z as f64 * 0.08;

    cave_noise.get([nx, ny, nz])
}

/// Generate underground portion of a column (caves, etc.)
/// Surface is already generated by the biome terrain system.
fn generate_underground_column(
    chunk: &mut LocalChunk,
    x: usize,
    y: usize,
    world: &WorldData,
    geology: &GeologyParams,
    cave_noise: &Perlin,
    rng: &mut ChaCha8Rng,
) {
    // Find the surface at this position (already generated by biome terrain)
    let mut local_surface_z = geology.surface_z;
    for z in (chunk.z_min..=chunk.z_max).rev() {
        let tile = chunk.get(x, y, z);
        if tile.terrain != LocalTerrain::Air && !tile.terrain.is_water() {
            local_surface_z = z;
            break;
        }
    }

    // Process underground tiles for caves
    for z in chunk.z_min..local_surface_z {
        let tile = chunk.get(x, y, z);

        // Skip if already air (from biome terrain) or water
        if tile.terrain == LocalTerrain::Air || tile.terrain.is_water() {
            continue;
        }

        // Check for cave generation using noise
        let cave_threshold = 0.6;
        let cave_value = sample_3d_cave(cave_noise, x, y, z, chunk.world_x, chunk.world_y);

        if cave_value > cave_threshold {
            // This is a cave tile
            let mut cave_tile = LocalTile::new(LocalTerrain::CaveFloor, Material::Stone);
            cave_tile.temperature = geology.temperature;
            chunk.set(x, y, z, cave_tile);
        }
    }
}

/// Generate a single vertical column at (x, y)
fn generate_column(
    chunk: &mut LocalChunk,
    x: usize,
    y: usize,
    world: &WorldData,
    geology: &GeologyParams,
    surface_noise: &Perlin,
    cave_noise: &Perlin,
    rng: &mut ChaCha8Rng,
) {
    // Add micro-terrain variation to surface (±1-2 z-levels)
    let nx = x as f64 / LOCAL_SIZE as f64 * 4.0;
    let ny = y as f64 / LOCAL_SIZE as f64 * 4.0;
    let surface_variation = (surface_noise.get([nx, ny]) * 1.5) as i16;
    let local_surface_z = (geology.surface_z + surface_variation).clamp(chunk.z_min, chunk.z_max);

    // Get the world tile's ZTile data at the ACTUAL surface z (not the varied one)
    // This ensures we read the correct biome/terrain type from the world
    let world_ztile_surface = *world.zlevels.get(chunk.world_x, chunk.world_y, geology.surface_z as i32);

    // Generate each z-level in the column
    for z in chunk.z_min..=chunk.z_max {
        let tile = generate_tile_at_z(
            x, y, z,
            local_surface_z,
            geology,
            world,
            chunk.world_x,
            chunk.world_y,
            cave_noise,
            world_ztile_surface,
            rng,
        );
        chunk.set(x, y, z, tile);
    }
}

/// Generate a single tile at a specific z-level
fn generate_tile_at_z(
    x: usize,
    y: usize,
    z: i16,
    local_surface_z: i16,
    geology: &GeologyParams,
    world: &WorldData,
    world_x: usize,
    world_y: usize,
    cave_noise: &Perlin,
    world_ztile: ZTile,
    rng: &mut ChaCha8Rng,
) -> LocalTile {
    // Above surface: air (or snow/rain features later)
    if z > local_surface_z {
        return LocalTile::air();
    }

    // At surface: biome-dependent terrain
    if z == local_surface_z {
        return generate_surface_tile(geology, world_ztile, rng);
    }

    // Underground: soil, stone, or cave
    generate_underground_tile(x, y, z, local_surface_z, geology, world, world_x, world_y, cave_noise, rng)
}

/// Generate a surface tile based on biome and world data
fn generate_surface_tile(
    geology: &GeologyParams,
    world_ztile: ZTile,
    rng: &mut ChaCha8Rng,
) -> LocalTile {
    // Check for water
    if matches!(geology.water_body_type, crate::water_bodies::WaterBodyType::Ocean | crate::water_bodies::WaterBodyType::Lake) {
        return LocalTile::new(LocalTerrain::DeepWater, Material::Water);
    }
    if matches!(geology.water_body_type, crate::water_bodies::WaterBodyType::River) {
        return LocalTile::new(LocalTerrain::FlowingWater, Material::Water);
    }

    // Check for structure tiles from world
    if world_ztile.is_structure() {
        return world_ztile_to_local(world_ztile);
    }

    // Natural surface based on biome
    let material = biome_surface_material(geology.biome, false);
    let mut tile = LocalTile::surface(material);
    tile.temperature = geology.temperature;
    tile
}

/// Convert world ZTile to local tile (for structures)
fn world_ztile_to_local(ztile: ZTile) -> LocalTile {
    match ztile {
        ZTile::StoneFloor | ZTile::CobblestoneFloor => {
            LocalTile::new(LocalTerrain::ConstructedFloor { material: Material::Stone }, Material::Stone)
        }
        ZTile::WoodFloor => {
            LocalTile::new(LocalTerrain::ConstructedFloor { material: Material::Dirt }, Material::Dirt)
        }
        ZTile::DirtFloor => {
            LocalTile::new(LocalTerrain::ConstructedFloor { material: Material::Dirt }, Material::Dirt)
        }
        ZTile::StoneWall | ZTile::BrickWall | ZTile::FortressWall => {
            LocalTile::new(LocalTerrain::ConstructedWall { material: Material::Stone }, Material::Stone)
        }
        ZTile::WoodWall => {
            LocalTile::new(LocalTerrain::ConstructedWall { material: Material::Dirt }, Material::Dirt)
        }
        ZTile::DirtRoad | ZTile::StoneRoad => {
            LocalTile::new(LocalTerrain::ConstructedFloor { material: Material::Stone }, Material::Stone)
        }
        ZTile::Water => {
            LocalTile::new(LocalTerrain::DeepWater, Material::Water)
        }
        ZTile::CaveFloor => {
            LocalTile::new(LocalTerrain::CaveFloor, Material::Stone)
        }
        _ => LocalTile::surface(Material::Grass),
    }
}

/// Generate an underground tile (soil, stone, cave, or magma)
fn generate_underground_tile(
    x: usize,
    y: usize,
    z: i16,
    local_surface_z: i16,
    geology: &GeologyParams,
    world: &WorldData,
    world_x: usize,
    world_y: usize,
    cave_noise: &Perlin,
    rng: &mut ChaCha8Rng,
) -> LocalTile {
    let depth = local_surface_z - z;

    // Check world zlevel data for caves/structures at this z-level
    let world_ztile = *world.zlevels.get(world_x, world_y, z as i32);

    // If world has a structure at this z-level, use it
    if world_ztile.is_structure() {
        return world_ztile_to_local(world_ztile);
    }

    // If world has a cave at this location, use cave floor
    if world_ztile.is_cave() {
        let mut tile = LocalTile::new(LocalTerrain::CaveFloor, Material::Stone);
        tile.temperature = geology.temperature - depth as f32 * 0.5; // Gets cooler underground
        return tile;
    }

    // Magma at deep levels in volcanic areas
    if z <= (zlevel::CAVERN_3_MIN - 2) as i16 && geology.has_magma {
        let noise_val = cave_noise.get([x as f64 * 0.1, y as f64 * 0.1, z as f64 * 0.2]);
        if noise_val > 0.3 {
            return LocalTile::new(LocalTerrain::Magma, Material::Magma);
        }
    }

    // Soil layers (near surface)
    if depth <= geology.soil_depth as i16 {
        let soil_type = biome_soil_type(geology.biome, depth, geology.moisture);
        let mut tile = LocalTile::soil(soil_type);
        tile.temperature = geology.temperature - depth as f32 * 0.3;
        return tile;
    }

    // Aquifer layer
    if let Some(aquifer_z) = geology.aquifer_z {
        if z == aquifer_z || z == aquifer_z - 1 {
            return LocalTile::new(LocalTerrain::ShallowWater, Material::Water);
        }
    }

    // Stone layers (use noise for variety)
    let stone_noise = cave_noise.get([x as f64 * 0.05, y as f64 * 0.05, z as f64 * 0.1]);
    let stone_type = if stone_noise > 0.3 {
        geology.secondary_stone
    } else {
        geology.primary_stone
    };

    let mut tile = LocalTile::stone(stone_type);
    tile.temperature = geology.temperature - depth as f32 * 0.2;
    tile
}

/// Add surface features (trees, boulders, bushes)
fn add_surface_features(
    chunk: &mut LocalChunk,
    geology: &GeologyParams,
    rng: &mut ChaCha8Rng,
) {
    // Determine vegetation density from biome
    let (tree_chance, bush_chance, boulder_chance) = biome_feature_chances(geology.biome);

    for y in 0..LOCAL_SIZE {
        for x in 0..LOCAL_SIZE {
            let tile = chunk.get(x, y, geology.surface_z);

            // Only add features on passable natural terrain
            if !tile.terrain.is_passable() || matches!(tile.terrain, LocalTerrain::ShallowWater | LocalTerrain::DeepWater) {
                continue;
            }

            // Trees
            if rng.gen_bool(tree_chance) {
                let height = rng.gen_range(3..8);
                chunk.get_mut(x, y, geology.surface_z).feature = LocalFeature::Tree { height };
                continue;
            }

            // Bushes
            if rng.gen_bool(bush_chance) {
                chunk.get_mut(x, y, geology.surface_z).feature = LocalFeature::Bush;
                continue;
            }

            // Boulders
            if rng.gen_bool(boulder_chance) {
                chunk.get_mut(x, y, geology.surface_z).feature = LocalFeature::Boulder;
            }
        }
    }
}

/// Get feature chances by biome
fn biome_feature_chances(biome: ExtendedBiome) -> (f64, f64, f64) {
    // (tree_chance, bush_chance, boulder_chance)
    match biome {
        // Dense forests
        ExtendedBiome::TropicalRainforest |
        ExtendedBiome::TemperateRainforest => (0.25, 0.15, 0.01),

        // Regular forests
        ExtendedBiome::TemperateForest |
        ExtendedBiome::TropicalForest => (0.15, 0.10, 0.02),

        // Boreal/sparse forests
        ExtendedBiome::BorealForest => (0.08, 0.05, 0.02),

        // Grasslands
        ExtendedBiome::TemperateGrassland |
        ExtendedBiome::Savanna => (0.02, 0.05, 0.01),

        // Deserts
        ExtendedBiome::Desert |
        ExtendedBiome::SaltFlats => (0.0, 0.01, 0.02),

        // Mountains
        ExtendedBiome::SnowyPeaks |
        ExtendedBiome::AlpineTundra => (0.0, 0.01, 0.05),

        // Swamps
        ExtendedBiome::Swamp |
        ExtendedBiome::Marsh => (0.08, 0.12, 0.01),

        // Tundra
        ExtendedBiome::Tundra => (0.0, 0.02, 0.02),

        // Default
        _ => (0.05, 0.05, 0.02),
    }
}

/// Add cave features (stalactites, stalagmites, crystals, mushrooms)
fn add_cave_features(
    chunk: &mut LocalChunk,
    geology: &GeologyParams,
    rng: &mut ChaCha8Rng,
) {
    for z in chunk.z_min..chunk.z_max {
        // Skip non-underground levels
        if z >= geology.surface_z {
            continue;
        }

        for y in 0..LOCAL_SIZE {
            for x in 0..LOCAL_SIZE {
                let tile = chunk.get(x, y, z);

                // Only add features in cave spaces
                if tile.terrain != LocalTerrain::CaveFloor {
                    continue;
                }

                // Determine cavern layer for feature types
                let cavern = geology.cavern_layer(z);

                // Stalactites/stalagmites (all caves)
                if rng.gen_bool(0.05) {
                    chunk.get_mut(x, y, z).feature = if rng.gen_bool(0.5) {
                        LocalFeature::Stalactite
                    } else {
                        LocalFeature::Stalagmite
                    };
                    continue;
                }

                // Mushrooms (cavern 1 and 2)
                if matches!(cavern, Some(0) | Some(1)) && rng.gen_bool(0.03) {
                    chunk.get_mut(x, y, z).feature = if rng.gen_bool(0.2) {
                        LocalFeature::GiantMushroom
                    } else {
                        LocalFeature::Mushroom
                    };
                    continue;
                }

                // Crystals (cavern 2 and 3)
                if matches!(cavern, Some(1) | Some(2)) && rng.gen_bool(0.02) {
                    chunk.get_mut(x, y, z).feature = LocalFeature::Crystal;
                }
            }
        }
    }
}

/// Copy cave system from world zlevels to local chunk with expansion
///
/// The world has a single cave marker per tile; we expand it to fill
/// the 48x48 local area with natural cave shapes using noise.
fn copy_cave_from_world(
    chunk: &mut LocalChunk,
    world: &WorldData,
    world_x: usize,
    world_y: usize,
    geology: &GeologyParams,
) {
    let cave_noise = Perlin::new(world.seed as u32);
    let detail_noise = Perlin::new(world.seed.wrapping_add(1) as u32);

    // Scan world zlevels for cave tiles
    for z in zlevel::MIN_Z..=zlevel::MAX_Z {
        let world_ztile = *world.zlevels.get(world_x, world_y, z);

        // Skip non-cave tiles
        if !world_ztile.is_cave() {
            continue;
        }

        let z16 = z as i16;

        // Expand single world cave tile to local 48x48 area
        for ly in 0..LOCAL_SIZE {
            for lx in 0..LOCAL_SIZE {
                // Use noise to create natural cave boundaries
                let nx = lx as f64 / LOCAL_SIZE as f64 * 4.0;
                let ny = ly as f64 / LOCAL_SIZE as f64 * 4.0;
                let nz = z as f64 * 0.5;

                let noise_val = cave_noise.get([nx, ny, nz]);
                let detail_val = detail_noise.get([nx * 2.0, ny * 2.0, nz]) * 0.3;

                // Edge walls - more stone near edges
                let edge_dist = (lx.min(LOCAL_SIZE - 1 - lx).min(ly).min(LOCAL_SIZE - 1 - ly)) as f64;
                let edge_factor = (edge_dist / 8.0).min(1.0);

                // Determine if this local tile should be cave floor or wall
                let combined = noise_val + detail_val + edge_factor * 0.5;

                if combined > 0.0 {
                    // Cave floor - convert world ztile to local terrain
                    let local_tile = match world_ztile {
                        ZTile::CaveFloor | ZTile::Flowstone | ZTile::CaveMoss |
                        ZTile::FungalGrowth | ZTile::ObsidianFloor => {
                            LocalTile::new(LocalTerrain::CaveFloor, Material::Stone)
                        }
                        ZTile::MagmaPool => {
                            LocalTile::new(LocalTerrain::Magma, Material::Magma)
                        }
                        ZTile::CaveLake | ZTile::WaterCave => {
                            LocalTile::new(LocalTerrain::DeepWater, Material::Water)
                        }
                        ZTile::Waterfall => {
                            LocalTile::new(LocalTerrain::FlowingWater, Material::Water)
                        }
                        ZTile::Stalactite => {
                            let mut tile = LocalTile::new(LocalTerrain::CaveFloor, Material::Stone);
                            tile.feature = LocalFeature::Stalactite;
                            tile
                        }
                        ZTile::Stalagmite => {
                            let mut tile = LocalTile::new(LocalTerrain::CaveFloor, Material::Stone);
                            tile.feature = LocalFeature::Stalagmite;
                            tile
                        }
                        ZTile::Pillar => {
                            let mut tile = LocalTile::new(LocalTerrain::CaveFloor, Material::Stone);
                            tile.feature = LocalFeature::Pillar;
                            tile
                        }
                        ZTile::GiantMushroom => {
                            let mut tile = LocalTile::new(LocalTerrain::CaveFloor, Material::Stone);
                            tile.feature = LocalFeature::GiantMushroom;
                            tile
                        }
                        ZTile::CrystalFormation => {
                            let mut tile = LocalTile::new(LocalTerrain::CaveFloor, Material::Stone);
                            tile.feature = LocalFeature::Crystal;
                            tile
                        }
                        _ => LocalTile::new(LocalTerrain::CaveFloor, Material::Stone),
                    };
                    chunk.set(lx, ly, z16, local_tile);
                } else {
                    // Cave wall (solid stone around cave)
                    chunk.set(lx, ly, z16, LocalTile::new(
                        LocalTerrain::Stone { stone_type: geology.primary_stone },
                        Material::Stone
                    ));
                }
            }
        }
    }
}

/// Add a cave entrance at the surface connecting to underground caves
fn add_cave_entrance(
    chunk: &mut LocalChunk,
    surface_z: i16,
    cave_z: i16,
    rng: &mut ChaCha8Rng,
) {
    // Place entrance in center area with some randomness
    let entrance_x = LOCAL_SIZE / 2 + rng.gen_range(0..8) - 4;
    let entrance_y = LOCAL_SIZE / 2 + rng.gen_range(0..8) - 4;

    // Create entrance opening at surface
    let radius = rng.gen_range(2..5);
    for dy in -(radius as i32)..=(radius as i32) {
        for dx in -(radius as i32)..=(radius as i32) {
            let dist = ((dx * dx + dy * dy) as f32).sqrt();
            if dist <= radius as f32 {
                let x = (entrance_x as i32 + dx).clamp(0, LOCAL_SIZE as i32 - 1) as usize;
                let y = (entrance_y as i32 + dy).clamp(0, LOCAL_SIZE as i32 - 1) as usize;

                // Create ramp/stairs going down
                if dist < (radius / 2) as f32 {
                    chunk.set(x, y, surface_z, LocalTile::new(LocalTerrain::CaveFloor, Material::Stone));
                    chunk.get_mut(x, y, surface_z).feature = LocalFeature::RampDown;
                } else {
                    chunk.set(x, y, surface_z, LocalTile::new(LocalTerrain::CaveFloor, Material::Stone));
                }
            }
        }
    }

    // Carve vertical passage from surface down to cave
    for z in cave_z..surface_z {
        chunk.set(entrance_x, entrance_y, z, LocalTile::new(LocalTerrain::CaveFloor, Material::Stone));
        // Add ramps for vertical movement
        if z == cave_z {
            chunk.get_mut(entrance_x, entrance_y, z).feature = LocalFeature::RampUp;
        }
    }
}

/// Generate a spring/water source feature at the surface
fn generate_spring_feature(
    chunk: &mut LocalChunk,
    surface_z: i16,
    rng: &mut ChaCha8Rng,
) {
    // Place spring in center area
    let spring_x = LOCAL_SIZE / 2 + rng.gen_range(0..10) - 5;
    let spring_y = LOCAL_SIZE / 2 + rng.gen_range(0..10) - 5;

    // Create a small pool around the spring
    let pool_radius = rng.gen_range(3..7);
    for dy in -(pool_radius as i32)..=(pool_radius as i32) {
        for dx in -(pool_radius as i32)..=(pool_radius as i32) {
            let dist = ((dx * dx + dy * dy) as f32).sqrt();
            if dist <= pool_radius as f32 {
                let x = (spring_x as i32 + dx).clamp(0, LOCAL_SIZE as i32 - 1) as usize;
                let y = (spring_y as i32 + dy).clamp(0, LOCAL_SIZE as i32 - 1) as usize;

                if dist < (pool_radius / 2) as f32 {
                    // Deep water in center
                    chunk.set(x, y, surface_z, LocalTile::new(LocalTerrain::DeepWater, Material::Water));
                } else {
                    // Shallow water around edges
                    chunk.set(x, y, surface_z, LocalTile::new(LocalTerrain::ShallowWater, Material::Water));
                }
            }
        }
    }

    // Mark the spring source
    chunk.get_mut(spring_x, spring_y, surface_z).feature = LocalFeature::Fountain;
}

/// Generate a waterfall feature connecting z-levels
fn generate_waterfall_feature(
    chunk: &mut LocalChunk,
    waterfall_z: i16,
    rng: &mut ChaCha8Rng,
) {
    // Place waterfall in center area
    let waterfall_x = LOCAL_SIZE / 2 + rng.gen_range(0..10) - 5;
    let waterfall_y = LOCAL_SIZE / 2 + rng.gen_range(0..10) - 5;

    // Create waterfall column going down several z-levels
    let drop_height = rng.gen_range(2..5);
    for dz in 0..drop_height {
        let z = waterfall_z - dz;
        if z >= chunk.z_min {
            // Central water flow
            chunk.set(waterfall_x, waterfall_y, z, LocalTile::new(LocalTerrain::FlowingWater, Material::Water));

            // Spray/mist around waterfall
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let x = (waterfall_x as i32 + dx).clamp(0, LOCAL_SIZE as i32 - 1) as usize;
                let y = (waterfall_y as i32 + dy).clamp(0, LOCAL_SIZE as i32 - 1) as usize;
                if rng.gen_bool(0.5) {
                    chunk.set(x, y, z, LocalTile::new(LocalTerrain::ShallowWater, Material::Water));
                }
            }
        }
    }

    // Pool at the bottom
    let bottom_z = waterfall_z - drop_height;
    if bottom_z >= chunk.z_min {
        let pool_radius = rng.gen_range(2..5);
        for dy in -(pool_radius as i32)..=(pool_radius as i32) {
            for dx in -(pool_radius as i32)..=(pool_radius as i32) {
                let dist = ((dx * dx + dy * dy) as f32).sqrt();
                if dist <= pool_radius as f32 {
                    let x = (waterfall_x as i32 + dx).clamp(0, LOCAL_SIZE as i32 - 1) as usize;
                    let y = (waterfall_y as i32 + dy).clamp(0, LOCAL_SIZE as i32 - 1) as usize;
                    chunk.set(x, y, bottom_z, LocalTile::new(LocalTerrain::ShallowWater, Material::Water));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_chunk_basic() {
        let chunk = LocalChunk::new(10, 20, 5);
        assert_eq!(chunk.world_x, 10);
        assert_eq!(chunk.world_y, 20);
        assert_eq!(chunk.surface_z, 5);
        assert_eq!(chunk.z_min, zlevel::MIN_Z as i16);
        assert_eq!(chunk.z_max, zlevel::MAX_Z as i16);
    }

    #[test]
    fn test_integration_biome_consistency() {
        // Generate a small test world
        use crate::world::generate_world;

        let world = generate_world(64, 32, 42);

        // Find a land tile (not ocean)
        let mut test_x = 32;
        let mut test_y = 16;
        for y in 0..32 {
            for x in 0..64 {
                let biome = *world.biomes.get(x, y);
                if !matches!(biome, ExtendedBiome::DeepOcean | ExtendedBiome::Ocean | ExtendedBiome::CoastalWater) {
                    test_x = x;
                    test_y = y;
                    break;
                }
            }
        }

        let world_biome = *world.biomes.get(test_x, test_y);
        let chunk = generate_local_chunk(&world, test_x, test_y);

        // The chunk should be generated
        assert!(chunk.generated);

        // Get the biome config used
        let config = super::super::biome_terrain::get_biome_config(world_biome);

        // Check that surface has appropriate terrain for the biome
        let center = LOCAL_SIZE / 2;
        let surface_tile = chunk.get(center, center, chunk.surface_z);

        // Surface should be passable (not solid rock) or be water for water biomes
        // Either the surface is passable, has water, or has a structure on it
        let is_ok = surface_tile.terrain.is_passable() ||
                    surface_tile.terrain.is_water() ||
                    matches!(surface_tile.terrain, LocalTerrain::ConstructedFloor { .. }) ||
                    matches!(surface_tile.terrain, LocalTerrain::ConstructedWall { .. }) ||
                    matches!(surface_tile.terrain, LocalTerrain::Cobblestone) ||
                    matches!(surface_tile.terrain, LocalTerrain::WoodFloor);
        assert!(is_ok, "Biome {:?} has impassable surface terrain: {:?}", world_biome, surface_tile.terrain);

        println!("✓ Biome consistency test passed for {:?} at ({}, {})", world_biome, test_x, test_y);
    }

    #[test]
    fn test_integration_dungeon_accessibility() {
        use crate::world::generate_world;

        let world = generate_world(64, 32, 42);

        // Find a location with a dungeon
        let mut dungeon_location = None;
        if let Some(ref history) = world.history {
            for (&(x, y), _dungeon_id) in &history.dungeons.dungeons_by_location {
                if x < 64 && y < 32 {
                    dungeon_location = Some((x, y));
                    break;
                }
            }
        }

        if let Some((dx, dy)) = dungeon_location {
            let chunk = generate_local_chunk(&world, dx, dy);

            // Find stairs down on the surface level
            let mut found_stairs_down = false;
            for y in 0..LOCAL_SIZE {
                for x in 0..LOCAL_SIZE {
                    let tile = chunk.get(x, y, chunk.surface_z);
                    if tile.feature == LocalFeature::StairsDown {
                        found_stairs_down = true;

                        // Check there's a path down (stairs up on level below)
                        let below_tile = chunk.get(x, y, chunk.surface_z - 1);
                        let has_connection = below_tile.feature == LocalFeature::StairsUp ||
                                            below_tile.feature == LocalFeature::StairsDown ||
                                            below_tile.terrain.is_passable();
                        assert!(has_connection, "Stairs down at ({}, {}) have no accessible level below", x, y);
                        break;
                    }
                }
                if found_stairs_down { break; }
            }

            // Should have found stairs
            assert!(found_stairs_down, "Dungeon at ({}, {}) has no stairs down on surface", dx, dy);
            println!("✓ Dungeon accessibility test passed at ({}, {})", dx, dy);
        } else {
            println!("⚠ No dungeon found in test world, skipping dungeon test");
        }
    }

    #[test]
    fn test_integration_multiple_biomes() {
        use crate::world::generate_world;

        let world = generate_world(128, 64, 12345);

        // Test several different biomes
        let mut tested_biomes = std::collections::HashSet::new();
        let mut tests_passed = 0;

        for y in (0..64).step_by(8) {
            for x in (0..128).step_by(8) {
                let biome = *world.biomes.get(x, y);

                // Skip if we already tested this biome
                if tested_biomes.contains(&biome) {
                    continue;
                }

                let chunk = generate_local_chunk(&world, x, y);
                assert!(chunk.generated);

                // Surface should exist and be reasonable
                let center = LOCAL_SIZE / 2;
                let _surface_tile = chunk.get(center, center, chunk.surface_z);

                tested_biomes.insert(biome);
                tests_passed += 1;

                if tests_passed >= 10 {
                    break;
                }
            }
            if tests_passed >= 10 {
                break;
            }
        }

        println!("✓ Tested {} different biomes: {:?}", tested_biomes.len(),
            tested_biomes.iter().take(5).collect::<Vec<_>>());
        assert!(tested_biomes.len() >= 3, "Should have tested at least 3 different biomes");
    }

    #[test]
    fn test_local_tile_access() {
        let mut chunk = LocalChunk::new(0, 0, 0);

        let tile = LocalTile::new(LocalTerrain::CaveFloor, Material::Stone);
        chunk.set(25, 30, -5, tile);

        assert_eq!(chunk.get(25, 30, -5).terrain, LocalTerrain::CaveFloor);
        assert_eq!(chunk.get(25, 30, -5).material, Material::Stone);
    }

    #[test]
    fn test_z_level_queries() {
        let chunk = LocalChunk::new(0, 0, 5);

        assert!(chunk.is_above_ground(6));
        assert!(chunk.is_above_ground(10));
        assert!(chunk.is_surface(5));
        assert!(chunk.is_underground(4));
        assert!(chunk.is_underground(-10));
    }

    #[test]
    fn test_local_terrain_properties() {
        assert!(LocalTerrain::Grass.is_passable());
        assert!(!LocalTerrain::Stone { stone_type: StoneType::Granite }.is_passable());
        assert!(LocalTerrain::Stone { stone_type: StoneType::Granite }.is_solid());
        assert!(LocalTerrain::DeepWater.is_water());
        assert!(LocalTerrain::Magma.is_dangerous());
    }
}
