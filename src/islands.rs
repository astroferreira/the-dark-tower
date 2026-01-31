//! Island detection and export
//!
//! Identifies connected land masses (islands) and provides functionality to
//! list them by size and export individual islands as standalone maps.

use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use crate::biomes::ExtendedBiome;
use crate::plates::PlateId;
use crate::tilemap::Tilemap;
use crate::world::WorldData;
use crate::water_bodies::WaterBodyId;

/// Island identifier (0 = water, 1+ = island ID)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct IslandId(pub u16);

impl IslandId {
    pub const NONE: IslandId = IslandId(0);

    pub fn is_none(&self) -> bool {
        self.0 == 0
    }
}

/// Information about an island (connected land mass)
#[derive(Clone, Debug)]
pub struct Island {
    pub id: IslandId,
    /// Number of land tiles in the island
    pub tile_count: usize,
    /// Minimum elevation on the island
    pub min_elevation: f32,
    /// Maximum elevation on the island
    pub max_elevation: f32,
    /// Average elevation
    pub avg_elevation: f32,
    /// Whether the island touches the north edge (polar)
    pub touches_north_edge: bool,
    /// Whether the island touches the south edge (polar)
    pub touches_south_edge: bool,
    /// Bounding box (min_x, min_y, max_x, max_y)
    pub bounds: (usize, usize, usize, usize),
    /// Center of mass (average x, y position)
    pub center: (f32, f32),
}

impl Island {
    fn new(id: IslandId) -> Self {
        Self {
            id,
            tile_count: 0,
            min_elevation: f32::MAX,
            max_elevation: f32::MIN,
            avg_elevation: 0.0,
            touches_north_edge: false,
            touches_south_edge: false,
            bounds: (usize::MAX, usize::MAX, 0, 0),
            center: (0.0, 0.0),
        }
    }

    fn add_tile(&mut self, x: usize, y: usize, elevation: f32, height: usize) {
        self.tile_count += 1;
        self.min_elevation = self.min_elevation.min(elevation);
        self.max_elevation = self.max_elevation.max(elevation);

        // Running average for elevation
        let n = self.tile_count as f32;
        self.avg_elevation = self.avg_elevation * (n - 1.0) / n + elevation / n;

        // Running average for center
        self.center.0 = self.center.0 * (n - 1.0) / n + x as f32 / n;
        self.center.1 = self.center.1 * (n - 1.0) / n + y as f32 / n;

        // Update bounds
        self.bounds.0 = self.bounds.0.min(x);
        self.bounds.1 = self.bounds.1.min(y);
        self.bounds.2 = self.bounds.2.max(x);
        self.bounds.3 = self.bounds.3.max(y);

        // Check edge touches
        if y == 0 {
            self.touches_north_edge = true;
        }
        if y == height - 1 {
            self.touches_south_edge = true;
        }
    }

    /// Get the width of the bounding box
    pub fn width(&self) -> usize {
        if self.bounds.2 >= self.bounds.0 {
            self.bounds.2 - self.bounds.0 + 1
        } else {
            0
        }
    }

    /// Get the height of the bounding box
    pub fn height(&self) -> usize {
        if self.bounds.3 >= self.bounds.1 {
            self.bounds.3 - self.bounds.1 + 1
        } else {
            0
        }
    }

    /// Check if this is a "true" island (not touching polar edges)
    pub fn is_true_island(&self) -> bool {
        !self.touches_north_edge && !self.touches_south_edge
    }

    /// Descriptive name based on size
    pub fn size_category(&self) -> &'static str {
        match self.tile_count {
            0..=10 => "Tiny",
            11..=50 => "Small",
            51..=200 => "Medium",
            201..=1000 => "Large",
            _ => "Continent",
        }
    }
}

/// Detect all islands (connected land masses) in the world.
///
/// Uses 4-connectivity flood-fill starting from each unvisited land tile.
/// Returns a tilemap of island IDs and a list of island metadata.
pub fn detect_islands(heightmap: &Tilemap<f32>) -> (Tilemap<IslandId>, Vec<Island>) {
    let width = heightmap.width;
    let height = heightmap.height;

    let mut island_map = Tilemap::new_with(width, height, IslandId::NONE);
    let mut islands = Vec::new();
    let mut visited = Tilemap::new_with(width, height, false);
    let mut next_id = 1u16;

    for start_y in 0..height {
        for start_x in 0..width {
            // Skip if already visited or not land
            if *visited.get(start_x, start_y) {
                continue;
            }

            let elevation = *heightmap.get(start_x, start_y);
            if elevation < 0.0 {
                // Water tile, mark visited but no island
                visited.set(start_x, start_y, true);
                continue;
            }

            // Found a new island - flood fill to find all connected land
            let island_id = IslandId(next_id);
            next_id += 1;

            let mut island = Island::new(island_id);
            let mut queue = VecDeque::new();

            queue.push_back((start_x, start_y));
            visited.set(start_x, start_y, true);

            while let Some((x, y)) = queue.pop_front() {
                let h = *heightmap.get(x, y);
                island_map.set(x, y, island_id);
                island.add_tile(x, y, h, height);

                // Check 4-connected neighbors
                for (nx, ny) in heightmap.neighbors(x, y) {
                    if !*visited.get(nx, ny) {
                        let neighbor_h = *heightmap.get(nx, ny);
                        if neighbor_h >= 0.0 {
                            // Land tile, add to this island
                            visited.set(nx, ny, true);
                            queue.push_back((nx, ny));
                        }
                    }
                }
            }

            islands.push(island);
        }
    }

    (island_map, islands)
}

/// Get a list of all islands sorted by tile count (smallest first)
pub fn list_islands_by_size(islands: &[Island], ascending: bool) -> Vec<&Island> {
    let mut sorted: Vec<&Island> = islands.iter().collect();
    if ascending {
        sorted.sort_by_key(|i| i.tile_count);
    } else {
        sorted.sort_by_key(|i| std::cmp::Reverse(i.tile_count));
    }
    sorted
}

/// Get only "true" islands (land masses not touching polar edges)
pub fn get_true_islands(islands: &[Island]) -> Vec<&Island> {
    islands.iter().filter(|i| i.is_true_island()).collect()
}

/// Filter islands by size range
pub fn filter_islands_by_size(
    islands: &[Island],
    min_tiles: usize,
    max_tiles: usize,
) -> Vec<&Island> {
    islands
        .iter()
        .filter(|i| i.tile_count >= min_tiles && i.tile_count <= max_tiles)
        .collect()
}

/// Get all tiles belonging to a specific island
pub fn get_island_tiles(
    island_map: &Tilemap<IslandId>,
    island_id: IslandId,
) -> Vec<(usize, usize)> {
    let mut tiles = Vec::new();

    for y in 0..island_map.height {
        for x in 0..island_map.width {
            if *island_map.get(x, y) == island_id {
                tiles.push((x, y));
            }
        }
    }

    tiles
}

/// Exported island data - a mini-world containing just the island and surrounding ocean
#[derive(Clone)]
pub struct ExportedIsland {
    /// Original island metadata
    pub island: Island,
    /// Width of the exported map
    pub width: usize,
    /// Height of the exported map
    pub height: usize,
    /// Offset from original map (x, y) - top-left corner
    pub offset: (usize, usize),
    /// Heightmap of the exported region
    pub heightmap: Tilemap<f32>,
    /// Temperature map
    pub temperature: Tilemap<f32>,
    /// Moisture map
    pub moisture: Tilemap<f32>,
    /// Biome map
    pub biomes: Tilemap<ExtendedBiome>,
    /// Water body map
    pub water_body_map: Tilemap<WaterBodyId>,
    /// Tectonic stress map
    pub stress_map: Tilemap<f32>,
    /// Plate assignment map
    pub plate_map: Tilemap<PlateId>,
    /// Water depth map
    pub water_depth: Tilemap<f32>,
    /// Flow accumulation (rivers) - optional
    pub flow_accumulation: Option<Tilemap<f32>>,
}

impl ExportedIsland {
    /// Convert local coordinates to world coordinates
    pub fn to_world_coords(&self, local_x: usize, local_y: usize) -> (usize, usize) {
        (
            (self.offset.0 + local_x),
            (self.offset.1 + local_y),
        )
    }

    /// Check if local coordinates are part of the island (not ocean)
    pub fn is_island_tile(&self, x: usize, y: usize) -> bool {
        *self.heightmap.get(x, y) >= 0.0
    }
}

/// Export an island and its surrounding ocean as a standalone mini-map.
///
/// `padding` specifies how many tiles of ocean to include around the island's bounding box.
/// The export handles horizontal wrapping correctly.
pub fn export_island(
    world: &WorldData,
    _island_map: &Tilemap<IslandId>,
    island: &Island,
    padding: usize,
) -> ExportedIsland {
    let world_width = world.width;
    let world_height = world.height;

    // Calculate export region with padding
    let min_x = island.bounds.0.saturating_sub(padding);
    let min_y = island.bounds.1.saturating_sub(padding);
    let max_x = (island.bounds.2 + padding).min(world_width - 1);
    let max_y = (island.bounds.3 + padding).min(world_height - 1);

    let export_width = max_x - min_x + 1;
    let export_height = max_y - min_y + 1;

    // Create new tilemaps for the exported region
    let mut heightmap = Tilemap::new_with(export_width, export_height, -100.0f32);
    let mut temperature = Tilemap::new_with(export_width, export_height, 15.0f32);
    let mut moisture = Tilemap::new_with(export_width, export_height, 0.5f32);
    let mut biomes = Tilemap::new_with(export_width, export_height, ExtendedBiome::DeepOcean);
    let mut water_body_map = Tilemap::new_with(export_width, export_height, WaterBodyId::OCEAN);
    let mut stress_map = Tilemap::new_with(export_width, export_height, 0.0f32);
    let mut plate_map = Tilemap::new_with(export_width, export_height, PlateId(0));
    let mut water_depth = Tilemap::new_with(export_width, export_height, 0.0f32);
    let mut flow_accumulation = world.flow_accumulation.as_ref()
        .map(|_| Tilemap::new_with(export_width, export_height, 0.0f32));

    // Copy data from world to exported region
    for local_y in 0..export_height {
        for local_x in 0..export_width {
            let world_x = (min_x + local_x) % world_width; // Handle horizontal wrapping
            let world_y = min_y + local_y;

            if world_y < world_height {
                heightmap.set(local_x, local_y, *world.heightmap.get(world_x, world_y));
                temperature.set(local_x, local_y, *world.temperature.get(world_x, world_y));
                moisture.set(local_x, local_y, *world.moisture.get(world_x, world_y));
                biomes.set(local_x, local_y, *world.biomes.get(world_x, world_y));
                water_body_map.set(local_x, local_y, *world.water_body_map.get(world_x, world_y));
                stress_map.set(local_x, local_y, *world.stress_map.get(world_x, world_y));
                plate_map.set(local_x, local_y, *world.plate_map.get(world_x, world_y));
                water_depth.set(local_x, local_y, *world.water_depth.get(world_x, world_y));

                if let (Some(ref mut fa_out), Some(ref fa_in)) = (&mut flow_accumulation, &world.flow_accumulation) {
                    fa_out.set(local_x, local_y, *fa_in.get(world_x, world_y));
                }
            }
        }
    }

    ExportedIsland {
        island: island.clone(),
        width: export_width,
        height: export_height,
        offset: (min_x, min_y),
        heightmap,
        temperature,
        moisture,
        biomes,
        water_body_map,
        stress_map,
        plate_map,
        water_depth,
        flow_accumulation,
    }
}

/// Export multiple islands at once
pub fn export_islands(
    world: &WorldData,
    island_map: &Tilemap<IslandId>,
    islands: &[&Island],
    padding: usize,
) -> Vec<ExportedIsland> {
    islands
        .iter()
        .map(|island| export_island(world, island_map, island, padding))
        .collect()
}

/// Print a summary of all islands to stdout
pub fn print_island_summary(islands: &[Island]) {
    println!("\n=== Island Summary ===");
    println!("Total land masses: {}", islands.len());

    let true_islands: Vec<_> = islands.iter().filter(|i| i.is_true_island()).collect();
    let continents: Vec<_> = islands.iter().filter(|i| !i.is_true_island()).collect();

    println!("True islands (not touching poles): {}", true_islands.len());
    println!("Continents/polar land: {}", continents.len());

    if !true_islands.is_empty() {
        println!("\n--- True Islands (sorted by size) ---");
        let mut sorted = true_islands.clone();
        sorted.sort_by_key(|i| std::cmp::Reverse(i.tile_count));

        for (i, island) in sorted.iter().enumerate().take(20) {
            println!(
                "  #{}: {} - {} tiles, elev {:.0}-{:.0}m, bounds ({},{}) to ({},{})",
                i + 1,
                island.size_category(),
                island.tile_count,
                island.min_elevation,
                island.max_elevation,
                island.bounds.0,
                island.bounds.1,
                island.bounds.2,
                island.bounds.3,
            );
        }

        if sorted.len() > 20 {
            println!("  ... and {} more islands", sorted.len() - 20);
        }
    }

    // Size distribution
    let tiny = islands.iter().filter(|i| i.tile_count <= 10).count();
    let small = islands.iter().filter(|i| i.tile_count > 10 && i.tile_count <= 50).count();
    let medium = islands.iter().filter(|i| i.tile_count > 50 && i.tile_count <= 200).count();
    let large = islands.iter().filter(|i| i.tile_count > 200 && i.tile_count <= 1000).count();
    let continent = islands.iter().filter(|i| i.tile_count > 1000).count();

    println!("\n--- Size Distribution ---");
    println!("  Tiny (1-10 tiles): {}", tiny);
    println!("  Small (11-50 tiles): {}", small);
    println!("  Medium (51-200 tiles): {}", medium);
    println!("  Large (201-1000 tiles): {}", large);
    println!("  Continent (1000+ tiles): {}", continent);
}

/// Island statistics
#[derive(Clone, Debug, Default)]
pub struct IslandStats {
    pub total_count: usize,
    pub true_island_count: usize,
    pub continent_count: usize,
    pub total_land_tiles: usize,
    pub smallest_island: usize,
    pub largest_island: usize,
    pub avg_island_size: f32,
}

/// Calculate statistics about islands
pub fn island_stats(islands: &[Island]) -> IslandStats {
    let mut stats = IslandStats {
        smallest_island: usize::MAX,
        ..Default::default()
    };

    for island in islands {
        stats.total_count += 1;
        stats.total_land_tiles += island.tile_count;

        if island.is_true_island() {
            stats.true_island_count += 1;
        } else {
            stats.continent_count += 1;
        }

        stats.smallest_island = stats.smallest_island.min(island.tile_count);
        stats.largest_island = stats.largest_island.max(island.tile_count);
    }

    if stats.total_count > 0 {
        stats.avg_island_size = stats.total_land_tiles as f32 / stats.total_count as f32;
    }

    if stats.smallest_island == usize::MAX {
        stats.smallest_island = 0;
    }

    stats
}

/// Save an exported island to PNG files (visual map with biomes and ocean)
pub fn save_island_png(
    island: &ExportedIsland,
    output_path: &std::path::Path,
) -> std::io::Result<()> {
    use image::{ImageBuffer, Rgb};

    let mut img = ImageBuffer::new(island.width as u32, island.height as u32);

    for y in 0..island.height {
        for x in 0..island.width {
            let h = *island.heightmap.get(x, y);
            let biome = *island.biomes.get(x, y);

            let (r, g, b) = if h < 0.0 {
                // Ocean - depth-based blue color
                let depth_factor = ((-h) / 500.0).min(1.0);
                let r = (20.0 + depth_factor * 10.0) as u8;
                let g = (50.0 + depth_factor * 30.0) as u8;
                let b = (100.0 + depth_factor * 100.0) as u8;
                (r, g, b)
            } else {
                // Land - use biome colors
                biome.color()
            };

            img.put_pixel(x as u32, y as u32, Rgb([r, g, b]));
        }
    }

    img.save(output_path)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    Ok(())
}

/// Save an exported island heightmap to PNG (grayscale with ocean in blue)
pub fn save_island_heightmap_png(
    island: &ExportedIsland,
    output_path: &std::path::Path,
) -> std::io::Result<()> {
    use image::{ImageBuffer, Rgb};

    // Find land height range for normalization
    let mut min_land_h = f32::MAX;
    let mut max_land_h = f32::MIN;
    for (_, _, h) in island.heightmap.iter() {
        if *h >= 0.0 {
            min_land_h = min_land_h.min(*h);
            max_land_h = max_land_h.max(*h);
        }
    }
    let land_range = (max_land_h - min_land_h).max(1.0);

    let mut img = ImageBuffer::new(island.width as u32, island.height as u32);

    for y in 0..island.height {
        for x in 0..island.width {
            let h = *island.heightmap.get(x, y);

            let (r, g, b) = if h < 0.0 {
                // Ocean - blue tones based on depth
                let depth_factor = ((-h) / 500.0).min(1.0);
                let r = (20.0 + depth_factor * 10.0) as u8;
                let g = (40.0 + depth_factor * 30.0) as u8;
                let b = (80.0 + depth_factor * 120.0) as u8;
                (r, g, b)
            } else {
                // Land - grayscale/terrain colors based on height
                let normalized = ((h - min_land_h) / land_range).clamp(0.0, 1.0);

                // Use terrain-like colors: green lowlands -> brown highlands -> white peaks
                if normalized < 0.3 {
                    // Lowlands - greenish
                    let t = normalized / 0.3;
                    let r = (80.0 + t * 40.0) as u8;
                    let g = (120.0 + t * 30.0) as u8;
                    let b = (60.0 + t * 20.0) as u8;
                    (r, g, b)
                } else if normalized < 0.7 {
                    // Mid elevations - brownish
                    let t = (normalized - 0.3) / 0.4;
                    let r = (120.0 + t * 60.0) as u8;
                    let g = (150.0 - t * 30.0) as u8;
                    let b = (80.0 - t * 20.0) as u8;
                    (r, g, b)
                } else {
                    // High elevations - gray to white
                    let t = (normalized - 0.7) / 0.3;
                    let base = (180.0 + t * 75.0) as u8;
                    (base, base, base)
                }
            };

            img.put_pixel(x as u32, y as u32, Rgb([r, g, b]));
        }
    }

    img.save(output_path)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    Ok(())
}

/// Configuration for island image export
pub struct IslandExportConfig {
    /// Export biome map PNG
    pub export_biomes: bool,
    /// Export heightmap PNG
    pub export_heightmap: bool,
    /// Export CSV data files
    pub export_csv: bool,
    /// Export JSON metadata
    pub export_metadata: bool,
}

impl Default for IslandExportConfig {
    fn default() -> Self {
        Self {
            export_biomes: true,
            export_heightmap: true,
            export_csv: false,
            export_metadata: true,
        }
    }
}

/// Export an island with all configured formats
pub fn export_island_files(
    island: &ExportedIsland,
    output_dir: &std::path::Path,
    config: &IslandExportConfig,
) -> std::io::Result<Vec<String>> {
    let mut exported_files = Vec::new();
    let base_name = format!("island_{}_{}tiles", island.island.id.0, island.island.tile_count);

    // Export biome PNG
    if config.export_biomes {
        let path = output_dir.join(format!("{}_biomes.png", base_name));
        save_island_png(island, &path)?;
        exported_files.push(path.display().to_string());
    }

    // Export heightmap PNG
    if config.export_heightmap {
        let path = output_dir.join(format!("{}_heightmap.png", base_name));
        save_island_heightmap_png(island, &path)?;
        exported_files.push(path.display().to_string());
    }

    // Export heightmap CSV
    if config.export_csv {
        use std::io::Write;

        let path = output_dir.join(format!("{}_heightmap.csv", base_name));
        let mut file = std::fs::File::create(&path)?;
        for y in 0..island.height {
            let row: Vec<String> = (0..island.width)
                .map(|x| format!("{:.1}", island.heightmap.get(x, y)))
                .collect();
            writeln!(file, "{}", row.join(","))?;
        }
        exported_files.push(path.display().to_string());

        let path = output_dir.join(format!("{}_biomes.csv", base_name));
        let mut file = std::fs::File::create(&path)?;
        for y in 0..island.height {
            let row: Vec<String> = (0..island.width)
                .map(|x| format!("{}", *island.biomes.get(x, y) as u8))
                .collect();
            writeln!(file, "{}", row.join(","))?;
        }
        exported_files.push(path.display().to_string());
    }

    // Export metadata JSON
    if config.export_metadata {
        use std::io::Write;

        let path = output_dir.join(format!("{}_meta.json", base_name));
        let mut file = std::fs::File::create(&path)?;
        writeln!(file, "{{")?;
        writeln!(file, "  \"island_id\": {},", island.island.id.0)?;
        writeln!(file, "  \"tile_count\": {},", island.island.tile_count)?;
        writeln!(file, "  \"export_width\": {},", island.width)?;
        writeln!(file, "  \"export_height\": {},", island.height)?;
        writeln!(file, "  \"offset_x\": {},", island.offset.0)?;
        writeln!(file, "  \"offset_y\": {},", island.offset.1)?;
        writeln!(file, "  \"min_elevation\": {:.1},", island.island.min_elevation)?;
        writeln!(file, "  \"max_elevation\": {:.1},", island.island.max_elevation)?;
        writeln!(file, "  \"avg_elevation\": {:.1},", island.island.avg_elevation)?;
        writeln!(file, "  \"center_x\": {:.1},", island.island.center.0)?;
        writeln!(file, "  \"center_y\": {:.1},", island.island.center.1)?;
        writeln!(file, "  \"bounds_min_x\": {},", island.island.bounds.0)?;
        writeln!(file, "  \"bounds_min_y\": {},", island.island.bounds.1)?;
        writeln!(file, "  \"bounds_max_x\": {},", island.island.bounds.2)?;
        writeln!(file, "  \"bounds_max_y\": {},", island.island.bounds.3)?;
        writeln!(file, "  \"touches_north\": {},", island.island.touches_north_edge)?;
        writeln!(file, "  \"touches_south\": {},", island.island.touches_south_edge)?;
        writeln!(file, "  \"is_true_island\": {},", island.island.is_true_island())?;
        writeln!(file, "  \"size_category\": \"{}\"", island.island.size_category())?;
        writeln!(file, "}}")?;
        exported_files.push(path.display().to_string());
    }

    Ok(exported_files)
}

/// Configuration for comprehensive island data export
#[derive(Clone, Debug)]
pub struct IslandDataExportConfig {
    /// Use binary format (more compact) vs CSV (human readable)
    pub binary_format: bool,
    /// Export heightmap
    pub heightmap: bool,
    /// Export temperature
    pub temperature: bool,
    /// Export moisture
    pub moisture: bool,
    /// Export biomes
    pub biomes: bool,
    /// Export tectonic stress
    pub stress: bool,
    /// Export plate assignment
    pub plates: bool,
    /// Export water depth
    pub water_depth: bool,
    /// Export flow accumulation (rivers)
    pub flow_accumulation: bool,
    /// Export PNG visualizations
    pub png_images: bool,
}

impl Default for IslandDataExportConfig {
    fn default() -> Self {
        Self {
            binary_format: true,
            heightmap: true,
            temperature: true,
            moisture: true,
            biomes: true,
            stress: true,
            plates: true,
            water_depth: true,
            flow_accumulation: true,
            png_images: true,
        }
    }
}

/// Export complete WorldData for an island to a directory
pub fn export_island_world_data(
    island: &ExportedIsland,
    output_dir: &Path,
    config: &IslandDataExportConfig,
) -> std::io::Result<Vec<String>> {
    std::fs::create_dir_all(output_dir)?;

    let mut exported_files = Vec::new();
    let base_name = format!("island_{}", island.island.id.0);

    // Export metadata JSON
    let meta_path = output_dir.join(format!("{}_meta.json", base_name));
    export_island_metadata(island, &meta_path)?;
    exported_files.push(meta_path.display().to_string());

    // Export data arrays
    if config.heightmap {
        let ext = if config.binary_format { "bin" } else { "csv" };
        let path = output_dir.join(format!("{}_heightmap.{}", base_name, ext));
        if config.binary_format {
            export_f32_tilemap_binary(&island.heightmap, &path)?;
        } else {
            export_f32_tilemap_csv(&island.heightmap, &path)?;
        }
        exported_files.push(path.display().to_string());
    }

    if config.temperature {
        let ext = if config.binary_format { "bin" } else { "csv" };
        let path = output_dir.join(format!("{}_temperature.{}", base_name, ext));
        if config.binary_format {
            export_f32_tilemap_binary(&island.temperature, &path)?;
        } else {
            export_f32_tilemap_csv(&island.temperature, &path)?;
        }
        exported_files.push(path.display().to_string());
    }

    if config.moisture {
        let ext = if config.binary_format { "bin" } else { "csv" };
        let path = output_dir.join(format!("{}_moisture.{}", base_name, ext));
        if config.binary_format {
            export_f32_tilemap_binary(&island.moisture, &path)?;
        } else {
            export_f32_tilemap_csv(&island.moisture, &path)?;
        }
        exported_files.push(path.display().to_string());
    }

    if config.biomes {
        let ext = if config.binary_format { "bin" } else { "csv" };
        let path = output_dir.join(format!("{}_biomes.{}", base_name, ext));
        if config.binary_format {
            export_biomes_tilemap_binary(&island.biomes, &path)?;
        } else {
            export_biomes_tilemap_csv(&island.biomes, &path)?;
        }
        // Export biome legend
        let legend_path = output_dir.join(format!("{}_biomes_legend.json", base_name));
        export_island_biome_legend(&island.biomes, &legend_path)?;
        exported_files.push(path.display().to_string());
        exported_files.push(legend_path.display().to_string());
    }

    if config.stress {
        let ext = if config.binary_format { "bin" } else { "csv" };
        let path = output_dir.join(format!("{}_stress.{}", base_name, ext));
        if config.binary_format {
            export_f32_tilemap_binary(&island.stress_map, &path)?;
        } else {
            export_f32_tilemap_csv(&island.stress_map, &path)?;
        }
        exported_files.push(path.display().to_string());
    }

    if config.plates {
        let ext = if config.binary_format { "bin" } else { "csv" };
        let path = output_dir.join(format!("{}_plates.{}", base_name, ext));
        if config.binary_format {
            export_plates_tilemap_binary(&island.plate_map, &path)?;
        } else {
            export_plates_tilemap_csv(&island.plate_map, &path)?;
        }
        exported_files.push(path.display().to_string());
    }

    if config.water_depth {
        let ext = if config.binary_format { "bin" } else { "csv" };
        let path = output_dir.join(format!("{}_water_depth.{}", base_name, ext));
        if config.binary_format {
            export_f32_tilemap_binary(&island.water_depth, &path)?;
        } else {
            export_f32_tilemap_csv(&island.water_depth, &path)?;
        }
        exported_files.push(path.display().to_string());
    }

    if config.flow_accumulation {
        if let Some(ref flow_acc) = island.flow_accumulation {
            let ext = if config.binary_format { "bin" } else { "csv" };
            let path = output_dir.join(format!("{}_flow.{}", base_name, ext));
            if config.binary_format {
                export_f32_tilemap_binary(flow_acc, &path)?;
            } else {
                export_f32_tilemap_csv(flow_acc, &path)?;
            }
            exported_files.push(path.display().to_string());
        }
    }

    // Export PNG visualizations
    if config.png_images {
        let biome_path = output_dir.join(format!("{}_biomes.png", base_name));
        save_island_png(island, &biome_path)?;
        exported_files.push(biome_path.display().to_string());

        let height_path = output_dir.join(format!("{}_heightmap.png", base_name));
        save_island_heightmap_png(island, &height_path)?;
        exported_files.push(height_path.display().to_string());
    }

    Ok(exported_files)
}

/// Export island metadata as JSON
fn export_island_metadata(island: &ExportedIsland, path: &Path) -> std::io::Result<()> {
    let mut file = BufWriter::new(File::create(path)?);

    // Calculate stats
    let (min_h, max_h) = tilemap_stats(&island.heightmap);
    let (min_t, max_t) = tilemap_stats(&island.temperature);
    let land_tiles = island.heightmap.iter().filter(|(_, _, &h)| h >= 0.0).count();

    writeln!(file, "{{")?;
    writeln!(file, "  \"format_version\": \"1.0\",")?;
    writeln!(file, "  \"island_id\": {},", island.island.id.0)?;
    writeln!(file, "  \"tile_count\": {},", island.island.tile_count)?;
    writeln!(file, "  \"width\": {},", island.width)?;
    writeln!(file, "  \"height\": {},", island.height)?;
    writeln!(file, "  \"world_offset_x\": {},", island.offset.0)?;
    writeln!(file, "  \"world_offset_y\": {},", island.offset.1)?;
    writeln!(file, "  \"bounds\": {{")?;
    writeln!(file, "    \"min_x\": {},", island.island.bounds.0)?;
    writeln!(file, "    \"min_y\": {},", island.island.bounds.1)?;
    writeln!(file, "    \"max_x\": {},", island.island.bounds.2)?;
    writeln!(file, "    \"max_y\": {}", island.island.bounds.3)?;
    writeln!(file, "  }},")?;
    writeln!(file, "  \"center\": {{")?;
    writeln!(file, "    \"x\": {:.1},", island.island.center.0)?;
    writeln!(file, "    \"y\": {:.1}", island.island.center.1)?;
    writeln!(file, "  }},")?;
    writeln!(file, "  \"elevation\": {{")?;
    writeln!(file, "    \"min\": {:.1},", min_h)?;
    writeln!(file, "    \"max\": {:.1},", max_h)?;
    writeln!(file, "    \"island_min\": {:.1},", island.island.min_elevation)?;
    writeln!(file, "    \"island_max\": {:.1},", island.island.max_elevation)?;
    writeln!(file, "    \"island_avg\": {:.1}", island.island.avg_elevation)?;
    writeln!(file, "  }},")?;
    writeln!(file, "  \"temperature\": {{")?;
    writeln!(file, "    \"min\": {:.1},", min_t)?;
    writeln!(file, "    \"max\": {:.1}", max_t)?;
    writeln!(file, "  }},")?;
    writeln!(file, "  \"land_tiles\": {},", land_tiles)?;
    writeln!(file, "  \"ocean_tiles\": {},", island.width * island.height - land_tiles)?;
    writeln!(file, "  \"touches_north\": {},", island.island.touches_north_edge)?;
    writeln!(file, "  \"touches_south\": {},", island.island.touches_south_edge)?;
    writeln!(file, "  \"is_true_island\": {},", island.island.is_true_island())?;
    writeln!(file, "  \"size_category\": \"{}\",", island.island.size_category())?;
    writeln!(file, "  \"data_format\": {{")?;
    writeln!(file, "    \"heightmap\": \"f32 little-endian, row-major (y * width + x)\",")?;
    writeln!(file, "    \"temperature\": \"f32 little-endian, row-major (Celsius)\",")?;
    writeln!(file, "    \"moisture\": \"f32 little-endian, row-major (0.0-1.0)\",")?;
    writeln!(file, "    \"biomes\": \"u8 row-major (see biomes_legend.json)\",")?;
    writeln!(file, "    \"stress\": \"f32 little-endian (-1.0 to +1.0)\",")?;
    writeln!(file, "    \"plates\": \"u8 row-major (plate ID)\",")?;
    writeln!(file, "    \"water_depth\": \"f32 little-endian (meters)\",")?;
    writeln!(file, "    \"flow\": \"f32 little-endian (drainage area)\"")?;
    writeln!(file, "  }}")?;
    writeln!(file, "}}")?;

    Ok(())
}

/// Export f32 tilemap as binary (little-endian)
fn export_f32_tilemap_binary(tilemap: &Tilemap<f32>, path: &Path) -> std::io::Result<()> {
    let mut file = BufWriter::new(File::create(path)?);
    for y in 0..tilemap.height {
        for x in 0..tilemap.width {
            let value = *tilemap.get(x, y);
            file.write_all(&value.to_le_bytes())?;
        }
    }
    Ok(())
}

/// Export f32 tilemap as CSV
fn export_f32_tilemap_csv(tilemap: &Tilemap<f32>, path: &Path) -> std::io::Result<()> {
    let mut file = BufWriter::new(File::create(path)?);
    for y in 0..tilemap.height {
        let row: Vec<String> = (0..tilemap.width)
            .map(|x| format!("{:.2}", tilemap.get(x, y)))
            .collect();
        writeln!(file, "{}", row.join(","))?;
    }
    Ok(())
}

/// Export biome tilemap as binary (u8)
fn export_biomes_tilemap_binary(tilemap: &Tilemap<ExtendedBiome>, path: &Path) -> std::io::Result<()> {
    let mut file = BufWriter::new(File::create(path)?);
    for y in 0..tilemap.height {
        for x in 0..tilemap.width {
            let biome = *tilemap.get(x, y);
            file.write_all(&[biome as u8])?;
        }
    }
    Ok(())
}

/// Export biome tilemap as CSV
fn export_biomes_tilemap_csv(tilemap: &Tilemap<ExtendedBiome>, path: &Path) -> std::io::Result<()> {
    let mut file = BufWriter::new(File::create(path)?);
    for y in 0..tilemap.height {
        let row: Vec<String> = (0..tilemap.width)
            .map(|x| format!("{}", *tilemap.get(x, y) as u8))
            .collect();
        writeln!(file, "{}", row.join(","))?;
    }
    Ok(())
}

/// Export plate tilemap as binary (u8)
fn export_plates_tilemap_binary(tilemap: &Tilemap<PlateId>, path: &Path) -> std::io::Result<()> {
    let mut file = BufWriter::new(File::create(path)?);
    for y in 0..tilemap.height {
        for x in 0..tilemap.width {
            let plate_id = tilemap.get(x, y).0;
            file.write_all(&[plate_id])?;
        }
    }
    Ok(())
}

/// Export plate tilemap as CSV
fn export_plates_tilemap_csv(tilemap: &Tilemap<PlateId>, path: &Path) -> std::io::Result<()> {
    let mut file = BufWriter::new(File::create(path)?);
    for y in 0..tilemap.height {
        let row: Vec<String> = (0..tilemap.width)
            .map(|x| format!("{}", tilemap.get(x, y).0))
            .collect();
        writeln!(file, "{}", row.join(","))?;
    }
    Ok(())
}

/// Export biome legend for an island
fn export_island_biome_legend(biomes: &Tilemap<ExtendedBiome>, path: &Path) -> std::io::Result<()> {
    use std::collections::BTreeMap;

    let mut biome_set: BTreeMap<u8, ExtendedBiome> = BTreeMap::new();
    for (_, _, &biome) in biomes.iter() {
        biome_set.entry(biome as u8).or_insert(biome);
    }

    let mut file = BufWriter::new(File::create(path)?);
    writeln!(file, "{{")?;
    writeln!(file, "  \"biomes\": [")?;

    let biome_list: Vec<_> = biome_set.into_iter().collect();
    for (i, (id, biome)) in biome_list.iter().enumerate() {
        let color = biome.color();
        let comma = if i < biome_list.len() - 1 { "," } else { "" };
        writeln!(
            file,
            "    {{ \"id\": {}, \"name\": \"{:?}\", \"color\": [{}, {}, {}] }}{}",
            id, biome, color.0, color.1, color.2, comma
        )?;
    }

    writeln!(file, "  ]")?;
    writeln!(file, "}}")?;
    Ok(())
}

/// Get min/max from a f32 tilemap
fn tilemap_stats(tilemap: &Tilemap<f32>) -> (f32, f32) {
    let mut min = f32::MAX;
    let mut max = f32::MIN;
    for (_, _, &v) in tilemap.iter() {
        min = min.min(v);
        max = max.max(v);
    }
    (min, max)
}

/// Export all islands in a grid layout to a single PNG image
pub fn export_islands_grid(
    islands: &[ExportedIsland],
    output_path: &std::path::Path,
    cell_padding: usize,
    max_columns: usize,
) -> std::io::Result<()> {
    use image::{ImageBuffer, Rgb};

    if islands.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "No islands to export",
        ));
    }

    // Calculate uniform cell size (largest island dimensions + padding)
    let max_width = islands.iter().map(|i| i.width).max().unwrap_or(0);
    let max_height = islands.iter().map(|i| i.height).max().unwrap_or(0);
    let cell_width = max_width + cell_padding * 2;
    let cell_height = max_height + cell_padding * 2;

    // Calculate grid dimensions
    let columns = max_columns.min(islands.len());
    let rows = (islands.len() + columns - 1) / columns;

    let grid_width = columns * cell_width;
    let grid_height = rows * cell_height;

    // Create the grid image with dark blue background (ocean)
    let mut img = ImageBuffer::from_fn(grid_width as u32, grid_height as u32, |_, _| {
        Rgb([15u8, 30u8, 60u8]) // Dark ocean background
    });

    // Place each island in the grid
    for (idx, island) in islands.iter().enumerate() {
        let col = idx % columns;
        let row = idx / columns;

        // Calculate position (centered in cell)
        let cell_x = col * cell_width;
        let cell_y = row * cell_height;
        let offset_x = cell_x + (cell_width - island.width) / 2;
        let offset_y = cell_y + (cell_height - island.height) / 2;

        // Draw the island
        for y in 0..island.height {
            for x in 0..island.width {
                let h = *island.heightmap.get(x, y);
                let biome = *island.biomes.get(x, y);

                let (r, g, b) = if h < 0.0 {
                    // Ocean - depth-based blue
                    let depth_factor = ((-h) / 500.0).min(1.0);
                    let r = (20.0 + depth_factor * 10.0) as u8;
                    let g = (50.0 + depth_factor * 30.0) as u8;
                    let b = (100.0 + depth_factor * 100.0) as u8;
                    (r, g, b)
                } else {
                    // Land - biome colors
                    biome.color()
                };

                let px = (offset_x + x) as u32;
                let py = (offset_y + y) as u32;
                if px < grid_width as u32 && py < grid_height as u32 {
                    img.put_pixel(px, py, Rgb([r, g, b]));
                }
            }
        }

        // Draw island label (tile count) at bottom of cell
        // Simple approach: draw a small label background
        let label = format!("#{} ({})", island.island.id.0, island.island.tile_count);
        let label_y = cell_y + cell_height - cell_padding / 2;
        let label_x = cell_x + cell_padding;

        // Draw label background
        for ly in 0..8 {
            for lx in 0..(label.len() * 6).min(cell_width - cell_padding * 2) {
                let px = (label_x + lx) as u32;
                let py = (label_y + ly) as u32;
                if px < grid_width as u32 && py < grid_height as u32 {
                    img.put_pixel(px, py, Rgb([0u8, 0u8, 0u8]));
                }
            }
        }
    }

    img.save(output_path)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    Ok(())
}

/// Export all islands in a grid layout with heightmap coloring
pub fn export_islands_grid_heightmap(
    islands: &[ExportedIsland],
    output_path: &std::path::Path,
    cell_padding: usize,
    max_columns: usize,
) -> std::io::Result<()> {
    use image::{ImageBuffer, Rgb};

    if islands.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "No islands to export",
        ));
    }

    // Find global elevation range for consistent coloring
    let mut global_min_land = f32::MAX;
    let mut global_max_land = f32::MIN;
    for island in islands {
        for (_, _, h) in island.heightmap.iter() {
            if *h >= 0.0 {
                global_min_land = global_min_land.min(*h);
                global_max_land = global_max_land.max(*h);
            }
        }
    }
    let land_range = (global_max_land - global_min_land).max(1.0);

    // Calculate uniform cell size
    let max_width = islands.iter().map(|i| i.width).max().unwrap_or(0);
    let max_height = islands.iter().map(|i| i.height).max().unwrap_or(0);
    let cell_width = max_width + cell_padding * 2;
    let cell_height = max_height + cell_padding * 2;

    let columns = max_columns.min(islands.len());
    let rows = (islands.len() + columns - 1) / columns;

    let grid_width = columns * cell_width;
    let grid_height = rows * cell_height;

    let mut img = ImageBuffer::from_fn(grid_width as u32, grid_height as u32, |_, _| {
        Rgb([10u8, 20u8, 40u8])
    });

    for (idx, island) in islands.iter().enumerate() {
        let col = idx % columns;
        let row = idx / columns;

        let cell_x = col * cell_width;
        let cell_y = row * cell_height;
        let offset_x = cell_x + (cell_width - island.width) / 2;
        let offset_y = cell_y + (cell_height - island.height) / 2;

        for y in 0..island.height {
            for x in 0..island.width {
                let h = *island.heightmap.get(x, y);

                let (r, g, b) = if h < 0.0 {
                    let depth_factor = ((-h) / 500.0).min(1.0);
                    (
                        (20.0 + depth_factor * 10.0) as u8,
                        (40.0 + depth_factor * 30.0) as u8,
                        (80.0 + depth_factor * 120.0) as u8,
                    )
                } else {
                    let normalized = ((h - global_min_land) / land_range).clamp(0.0, 1.0);
                    if normalized < 0.3 {
                        let t = normalized / 0.3;
                        ((80.0 + t * 40.0) as u8, (120.0 + t * 30.0) as u8, (60.0 + t * 20.0) as u8)
                    } else if normalized < 0.7 {
                        let t = (normalized - 0.3) / 0.4;
                        ((120.0 + t * 60.0) as u8, (150.0 - t * 30.0) as u8, (80.0 - t * 20.0) as u8)
                    } else {
                        let t = (normalized - 0.7) / 0.3;
                        let base = (180.0 + t * 75.0) as u8;
                        (base, base, base)
                    }
                };

                let px = (offset_x + x) as u32;
                let py = (offset_y + y) as u32;
                if px < grid_width as u32 && py < grid_height as u32 {
                    img.put_pixel(px, py, Rgb([r, g, b]));
                }
            }
        }
    }

    img.save(output_path)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_island_detection() {
        // Create a simple 10x10 heightmap with two islands
        let mut heightmap = Tilemap::new_with(10, 10, -50.0);

        // Island 1: 3x3 in top-left (not touching edges)
        for y in 2..5 {
            for x in 2..5 {
                heightmap.set(x, y, 100.0);
            }
        }

        // Island 2: 2x2 in bottom-right (not touching edges)
        for y in 7..9 {
            for x in 7..9 {
                heightmap.set(x, y, 50.0);
            }
        }

        let (island_map, islands) = detect_islands(&heightmap);

        // Should have 2 islands
        assert_eq!(islands.len(), 2);

        // First island should have 9 tiles (3x3)
        let island1 = islands.iter().find(|i| i.tile_count == 9);
        assert!(island1.is_some());

        // Second island should have 4 tiles (2x2)
        let island2 = islands.iter().find(|i| i.tile_count == 4);
        assert!(island2.is_some());

        // Both should be "true" islands (not touching edges)
        for island in &islands {
            assert!(island.is_true_island());
        }

        // Check that island tiles are correctly assigned
        assert!(!island_map.get(2, 2).is_none());
        assert!(!island_map.get(7, 7).is_none());
        assert!(island_map.get(0, 0).is_none()); // Ocean
    }

    #[test]
    fn test_continent_detection() {
        // Create a heightmap where land touches north edge
        let mut heightmap = Tilemap::new_with(10, 10, -50.0);

        // Land touching north edge
        for x in 3..7 {
            heightmap.set(x, 0, 100.0);
            heightmap.set(x, 1, 100.0);
        }

        let (_, islands) = detect_islands(&heightmap);

        assert_eq!(islands.len(), 1);
        assert!(!islands[0].is_true_island()); // Should be a "continent"
        assert!(islands[0].touches_north_edge);
    }
}
