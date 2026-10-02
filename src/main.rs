// Suppress warnings for unused code - many utilities are kept for future use
#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(unreachable_patterns)]

use clap::Parser;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

mod ascii;
mod biome_feathering;
mod biomes;
mod cartography;
mod climate;
mod coastline;
mod erosion;
mod explorer;
mod exr_export;
mod grid_export;
mod heightmap;
mod history;
mod islands;
mod local;
mod lore;
mod map_export;
mod menu;
mod microclimate;
mod plates;
mod region;
mod scale;
mod seasons;
mod seeds;
mod tilemap;
mod tiles;
mod underground_water;
mod water_bodies;
mod weather_zones;
mod world;
mod world_export;

use menu::{MenuResult, WorldConfig};
use seeds::WorldSeeds;
use tilemap::Tilemap;

#[derive(Parser, Debug)]
#[command(name = "planet_generator")]
#[command(about = "Generate procedural planet maps with tectonic plates")]
struct Args {
    /// Width of the tilemap in pixels
    #[arg(short = 'W', long, default_value = "512")]
    width: usize,

    /// Height of the tilemap in pixels
    #[arg(short = 'H', long, default_value = "256")]
    height: usize,

    /// Master seed (derives all other seeds if not overridden)
    #[arg(short, long)]
    seed: Option<u64>,

    /// Number of tectonic plates (random based on world style if not specified)
    #[arg(short = 'p', long)]
    plates: Option<usize>,

    /// World style preset controlling land/ocean distribution
    /// Options: earthlike, archipelago, islands, pangaea, continental, waterworld
    #[arg(short = 'w', long, default_value = "earthlike")]
    world_style: String,

    /// Fantasy/special biome intensity, 0 (fully natural) to 1 (full fantasy)
    #[arg(long, default_value_t = biomes::DEFAULT_FANTASY_INTENSITY)]
    fantasy: f32,

    /// Save the generated world (and history, if simulated) to this file
    #[arg(long)]
    save_world: Option<String>,

    /// Load a world saved with --save-world instead of generating one (seconds, not minutes)
    #[arg(long)]
    load_world: Option<String>,

    /// Season for tile-viewer snapshots: spring, summer, autumn or winter
    #[arg(long, default_value = "summer")]
    season: String,

    /// Don't simulate history for the tile viewer (no settlements, roads or ruins)
    #[arg(long)]
    no_history: bool,

    /// Print the named geography (rivers, ranges, seas, regions...) after history
    #[arg(long)]
    gazetteer: bool,

    /// Print the land-biome mix and fragmentation after biome generation
    #[arg(long)]
    biome_stats: bool,

    /// Open the graphical tile viewer (pixel-art tiles, mouse zoom/pan) instead of the
    /// terminal explorer
    #[arg(long)]
    tiles: bool,

    /// Tileset PNG for the tile viewer: an edited atlas from --export-tileset, or a
    /// Dwarf Fortress style 16x16 CP437 sheet
    #[arg(long)]
    tileset: Option<String>,

    /// World tile "X,Y" to centre the tile viewer (and its snapshots) on
    #[arg(long)]
    tiles_center: Option<String>,

    /// Render tile-viewer frames to <PREFIX>_overview/_16px/_32px.png without opening a window
    #[arg(long)]
    tiles_snapshot: Option<String>,

    /// Render a playable area (embark) at --tiles-center to <PREFIX>_surface/_z*/_section.png
    #[arg(long)]
    local_snapshot: Option<String>,

    /// Write the built-in tile atlas to this PNG (one row per tile kind, 4 variants) and exit
    #[arg(long)]
    export_tileset: Option<String>,

    /// Re-simulate a window of world tiles at high resolution (rivers, lakes, valleys).
    /// Value: "X,Y" = world tile at the window centre (as shown by the explorer's W:(x,y)),
    /// or "auto" to pick a river-rich, mountainous window.
    #[arg(long)]
    zoom: Option<String>,

    /// Zoom window size in world tiles
    #[arg(long, default_value = "8")]
    zoom_tiles: usize,

    /// Cells per world tile in the zoomed region
    #[arg(long, default_value = "128")]
    zoom_scale: usize,

    /// Landscape-evolution (stream-power erosion) iterations for the zoomed region
    #[arg(long, default_value = "40")]
    zoom_erosion: usize,

    /// Use the legacy static-plate + noise terrain instead of the tectonic simulation
    #[arg(long)]
    legacy_tectonics: bool,

    /// Simulated tectonic history in millions of years (longer = more collisions and rifting)
    #[arg(long, default_value = "200")]
    tectonic_myr: f32,

    // === Individual seed overrides ===

    /// Seed for tectonic plate generation
    #[arg(long)]
    seed_tectonics: Option<u64>,

    /// Seed for heightmap/terrain generation
    #[arg(long)]
    seed_heightmap: Option<u64>,

    /// Seed for erosion simulation
    #[arg(long)]
    seed_erosion: Option<u64>,

    /// Seed for climate patterns
    #[arg(long)]
    seed_climate: Option<u64>,

    /// Seed for biome generation
    #[arg(long)]
    seed_biomes: Option<u64>,

    /// Seed for coastline jittering
    #[arg(long)]
    seed_coastline: Option<u64>,

    /// Seed for river network
    #[arg(long)]
    seed_rivers: Option<u64>,

    /// Seed for rock materials
    #[arg(long)]
    seed_materials: Option<u64>,

    /// Show all seed values used
    #[arg(long)]
    show_seeds: bool,

    /// Export comparison grid of erosion presets
    #[arg(long)]
    export_erosion_grid: bool,

    /// Export comparison grid of climate modes
    #[arg(long)]
    export_climate_grid: bool,

    /// Export comparison grid of rainfall levels
    #[arg(long)]
    export_rainfall_grid: bool,

    /// Export full comparison grid (erosion x climate)
    #[arg(long)]
    export_full_grid: bool,

    /// Export all comparison grids
    #[arg(long)]
    export_all_grids: bool,

    /// Output filename prefix for grid exports
    #[arg(long, default_value = "comparison")]
    grid_prefix: String,

    /// Disable high-resolution erosion simulation (faster but lower quality rivers)
    #[arg(long)]
    no_hires: bool,

    /// Export freshwater network image (rivers + lakes only) before launching explorer
    #[arg(long)]
    export_rivers: bool,

    /// Export base map image (flat biome colors + rivers) before launching explorer
    #[arg(long)]
    export_base_map: bool,

    /// Export antique old paper cartography map image
    #[arg(long)]
    export_cartography: bool,

    /// Visual style for antique cartography export (parchment, atlas, copperplate, patina)
    #[arg(long, default_value = "parchment")]
    cartography_style: String,

    /// Force CPU software shader rendering for cartography (bypass GPU)
    #[arg(long)]
    cartography_cpu: bool,

    /// Skip launching the explorer (for batch/headless export)
    #[arg(long)]
    headless: bool,

    /// Export heightmap and river map to EXR files
    #[arg(long)]
    export_exr: bool,

    /// Output directory for EXR files (default: current directory)
    #[arg(long, default_value = ".")]
    exr_output_dir: String,

    /// Export all map variants (visual, data, legend) using improved export
    #[arg(long)]
    export_maps: bool,

    /// Output directory for PNG map exports (default: current directory)
    #[arg(long, default_value = ".")]
    map_output_dir: String,

    /// Disable LUT-based coloring (use discrete biome colors only)
    #[arg(long)]
    no_lut: bool,

    /// Disable border dithering
    #[arg(long)]
    no_dither: bool,

    /// Number of years of history to simulate (0 to skip history)
    #[arg(long, default_value = "0")]
    history_years: u32,

    /// Seed for history simulation (derived from master seed if not specified)
    #[arg(long)]
    history_seed: Option<u64>,

    /// Load history from a save file instead of simulating
    #[arg(long)]
    load_history: Option<String>,

    /// Save history to a file after simulation
    #[arg(long)]
    save_history: Option<String>,

    /// Export legends summary as markdown
    #[arg(long)]
    export_legends: Option<String>,

    /// Run N benchmark simulations and print aggregate quality metrics
    #[arg(long)]
    benchmark: Option<u32>,

    // === Island Commands ===

    /// List all islands with their sizes and locations
    #[arg(long)]
    list_islands: bool,

    /// Export individual islands as standalone map files
    #[arg(long)]
    export_islands: bool,

    /// Maximum island size (in tiles) to include when listing/exporting (default: all)
    #[arg(long)]
    island_max_size: Option<usize>,

    /// Minimum island size (in tiles) to include when listing/exporting (default: 1)
    #[arg(long, default_value = "1")]
    island_min_size: usize,

    /// Padding (ocean tiles) around exported islands (default: 10)
    #[arg(long, default_value = "10")]
    island_padding: usize,

    /// Output directory for island exports (default: ./islands)
    #[arg(long, default_value = "./islands")]
    island_output_dir: String,

    /// Only include "true" islands (not touching polar edges)
    #[arg(long)]
    islands_only: bool,

    /// Export all islands in a grid layout (single PNG image)
    #[arg(long)]
    export_island_grid: bool,

    /// Number of columns in the island grid (default: auto based on count)
    #[arg(long, default_value = "5")]
    island_grid_columns: usize,

    /// Export complete WorldData for each island (all data layers)
    #[arg(long)]
    export_island_data: bool,

    /// Use CSV format for island data export (default: binary)
    #[arg(long)]
    island_data_csv: bool,

    // === World Data Export ===

    /// Export complete world data for external applications
    #[arg(long)]
    export_world_data: bool,

    /// Output directory for world data export (default: ./world_data)
    #[arg(long, default_value = "./world_data")]
    world_data_dir: String,

    /// Use CSV format instead of binary for world data arrays
    #[arg(long)]
    world_data_csv: bool,

    /// Include per-tile detailed JSON (large file, for debugging)
    #[arg(long)]
    world_data_per_tile: bool,

    /// Upscaling factor for exported maps (1 = native, 2 = 2x, 4 = 4x, etc.)
    #[arg(long, default_value = "1")]
    upscale_factor: usize,
}

/// Years of history the tile viewer simulates when --history-years isn't given.
const DEFAULT_VIEWER_HISTORY_YEARS: u32 = 250;

fn main() {
    let args = Args::parse();

    if let Some(path) = &args.export_tileset {
        match tiles::Atlas::generated().save_png(std::path::Path::new(path)) {
            Ok(()) => println!("Wrote tile atlas to {path} (rows: {:?})", tiles::atlas::ALL_KINDS),
            Err(e) => eprintln!("Failed to write tile atlas: {e}"),
        }
        return;
    }

    // Handle grid export commands
    if args.export_erosion_grid || args.export_climate_grid || args.export_rainfall_grid
        || args.export_full_grid || args.export_all_grids
    {
        let grid_config = grid_export::GridExportConfig {
            width: args.width.min(512),  // Cap size for grid exports
            height: args.height.min(256),
            seed: args.seed.unwrap_or(42),
            world_style: plates::WorldStyle::from_str(&args.world_style).unwrap_or_default(),
            plates: args.plates,
            ..Default::default()
        };

        if args.export_all_grids {
            if let Err(e) = grid_export::export_all_grids(&grid_config, &args.grid_prefix) {
                eprintln!("Grid export error: {}", e);
                std::process::exit(1);
            }
            return;
        }

        if args.export_erosion_grid {
            let filename = format!("{}_erosion.png", args.grid_prefix);
            if let Err(e) = grid_export::export_erosion_grid(&grid_config, &filename) {
                eprintln!("Grid export error: {}", e);
                std::process::exit(1);
            }
        }

        if args.export_climate_grid {
            let filename = format!("{}_climate.png", args.grid_prefix);
            if let Err(e) = grid_export::export_climate_grid(&grid_config, &filename) {
                eprintln!("Grid export error: {}", e);
                std::process::exit(1);
            }
        }

        if args.export_rainfall_grid {
            let filename = format!("{}_rainfall.png", args.grid_prefix);
            if let Err(e) = grid_export::export_rainfall_grid(&grid_config, &filename) {
                eprintln!("Grid export error: {}", e);
                std::process::exit(1);
            }
        }

        if args.export_full_grid {
            let filename = format!("{}_full.png", args.grid_prefix);
            if let Err(e) = grid_export::export_full_grid(&grid_config, &filename) {
                eprintln!("Grid export error: {}", e);
                std::process::exit(1);
            }
        }

        return;
    }

    // A saved world replaces generation entirely.
    let mut loaded_history: Option<history::world_state::WorldHistory> = None;
    let loaded_world: Option<world::WorldData> = match &args.load_world {
        Some(path) => {
            let t0 = std::time::Instant::now();
            match world::load_world(std::path::Path::new(path)) {
                Ok((w, h)) => {
                    eprintln!("Loaded world {} ({}x{}, seed {}{}) in {:.1}s", path, w.width, w.height, w.seed(),
                        if h.is_some() { ", with history" } else { "" }, t0.elapsed().as_secs_f32());
                    loaded_history = h;
                    Some(w)
                }
                Err(e) => {
                    eprintln!("Could not load world {path}: {e}");
                    return;
                }
            }
        }
        None => None,
    };

    // Determine configuration: use menu if no seed provided (interactive mode),
    // otherwise use CLI args directly (batch mode)
    let (width, height, master_seed, plates_count, world_style, erosion_preset, climate_config) = if let Some(w) = &loaded_world {
        (w.width, w.height, w.seed(), None, plates::WorldStyle::default(), erosion::ErosionPreset::Normal, climate::ClimateConfig::default())
    } else if args.seed.is_some() {
        // Batch mode: use CLI args directly (use defaults for new options)
        let world_style = plates::WorldStyle::from_str(&args.world_style).unwrap_or_else(|| {
            eprintln!("Unknown world style '{}'. Available options:", args.world_style);
            for style in plates::WorldStyle::all() {
                eprintln!("  {}: {}", style, style.description());
            }
            std::process::exit(1);
        });
        (
            args.width,
            args.height,
            args.seed.unwrap(),
            args.plates,
            world_style,
            erosion::ErosionPreset::Normal,
            climate::ClimateConfig::default(),
        )
    } else {
        // Interactive mode: show menu
        let initial_config = WorldConfig {
            width: args.width,
            height: args.height,
            seed: None,
            plates: args.plates,
            world_style: plates::WorldStyle::from_str(&args.world_style).unwrap_or_default(),
            ..Default::default()
        };

        match menu::run_menu(initial_config) {
            Ok(MenuResult::Generate(config)) => {
                let seed = config.seed.unwrap_or_else(|| rand::random());
                let climate_config = climate::ClimateConfig {
                    mode: config.climate_mode,
                    rainfall: config.rainfall,
                    ..Default::default()
                };
                (
                    config.width,
                    config.height,
                    seed,
                    config.plates,
                    config.world_style,
                    config.erosion_preset,
                    climate_config,
                )
            }
            Ok(MenuResult::Quit) => {
                return;
            }
            Err(e) => {
                eprintln!("Menu error: {}", e);
                std::process::exit(1);
            }
        }
    };

    let mut world_data = 'build: {
        if let Some(w) = loaded_world {
            break 'build w;
        }
    // Build seeds from master seed with optional overrides
    let mut builder = WorldSeeds::builder(master_seed);

    if let Some(s) = args.seed_tectonics { builder = builder.tectonics(s); }
    if let Some(s) = args.seed_heightmap { builder = builder.heightmap(s); }
    if let Some(s) = args.seed_erosion { builder = builder.erosion(s); }
    if let Some(s) = args.seed_climate { builder = builder.climate(s); }
    if let Some(s) = args.seed_biomes { builder = builder.biomes(s); }
    if let Some(s) = args.seed_coastline { builder = builder.coastline(s); }
    if let Some(s) = args.seed_rivers { builder = builder.rivers(s); }
    if let Some(s) = args.seed_materials { builder = builder.materials(s); }

    let seeds = builder.build();

    println!("Generating planet with master seed: {}", seeds.master);
    println!("World style: {} ({})", world_style, world_style.description());
    println!("Map size: {}x{}", width, height);

    if args.show_seeds {
        println!("Seeds:");
        println!("  Tectonics: {}", seeds.tectonics);
        println!("  Heightmap: {}", seeds.heightmap);
        println!("  Erosion:   {}", seeds.erosion);
        println!("  Climate:   {}", seeds.climate);
        println!("  Biomes:    {}", seeds.biomes);
        println!("  Coastline: {}", seeds.coastline);
        println!("  Rivers:    {}", seeds.rivers);
        println!("  Materials: {}", seeds.materials);
    }

    // Create map scale for coordinate scaling (used throughout generation)
    let map_scale = scale::MapScale::default();

    let (plate_map, plates, stress_map, mut heightmap) = if args.legacy_tectonics {
        // Legacy path: static Voronoi-like plates, boundary stress and noise-driven terrain.
        let mut tectonic_rng = ChaCha8Rng::seed_from_u64(seeds.tectonics);
        println!("Generating tectonic plates (legacy)...");
        let (plate_map, plates) = plates::generate_plates(width, height, plates_count, world_style, &mut tectonic_rng);
        let continental_count = plates.iter().filter(|p| p.plate_type == plates::PlateType::Continental).count();
        let oceanic_count = plates.iter().filter(|p| p.plate_type == plates::PlateType::Oceanic).count();
        println!("Created {} plates ({} continental, {} oceanic)", plates.len(), continental_count, oceanic_count);

        println!("Calculating plate stress...");
        let stress_map = plates::calculate_stress(&plate_map, &plates);

        println!("Generating heightmap...");
        let land_mask = heightmap::generate_land_mask(&plate_map, &plates, seeds.heightmap);
        let land_count = (0..height).flat_map(|y| (0..width).map(move |x| (x, y)))
            .filter(|&(x, y)| *land_mask.get(x, y)).count();
        println!("Land mask: {} cells are land ({:.1}%)", land_count, 100.0 * land_count as f64 / (width * height) as f64);
        let mut heightmap = heightmap::generate_heightmap(&plate_map, &plates, &stress_map, seeds.heightmap);
        heightmap::apply_inland_uplift(&mut heightmap, &stress_map, &map_scale);
        (plate_map, plates, stress_map, heightmap)
    } else {
        // Tectonic simulation: plates drift on a sphere, collide, subduct and rift; terrain
        // follows from crustal thickness (isostasy) and seafloor age.
        println!("Simulating plate tectonics ({} Myr)...", args.tectonic_myr);
        let params = plates::TectonicParams { total_myr: args.tectonic_myr, ..Default::default() };
        let t = plates::generate_tectonic_terrain(width, height, plates_count, world_style, &seeds, &params);
        let continental_count = t.plates.iter().filter(|p| p.plate_type == plates::PlateType::Continental).count();
        let oceanic_count = t.plates.len() - continental_count;
        println!("Tectonic history complete: {} plates survive ({} continental, {} oceanic)", t.plates.len(), continental_count, oceanic_count);
        // (Interior seaward slope is built into the tectonic terrain itself, before sea level.)
        (t.plate_map, t.plates, t.stress_map, t.heightmap)
    };
    let mut min_h = f32::MAX;
    let mut max_h = f32::MIN;
    for (_, _, &h) in heightmap.iter() {
        if h < min_h { min_h = h; }
        if h > max_h { max_h = h; }
    }
    let above_sea = (0..height).flat_map(|y| (0..width).map(move |x| (x, y)))
        .filter(|&(x, y)| *heightmap.get(x, y) > 0.0).count();
    println!("Heightmap range: {:.1}m to {:.1}m ({:.1}% above sea level)", min_h, max_h,
        100.0 * above_sea as f64 / (width * height) as f64);

    // Generate physical climate (energy balance, 3-cell circulation, winds, ocean currents, moisture advection)
    println!("Simulating physical climate (mode: {}, rainfall: {})...", climate_config.mode, climate_config.rainfall);
    let climate_sim = climate::run_climate_simulation(&heightmap, &climate_config, seeds.climate);
    let temperature = climate_sim.mean_temperature.clone();
    let moisture = climate_sim.mean_moisture.clone();

    // Report climate stats
    let mut min_temp = f32::MAX;
    let mut max_temp = f32::MIN;
    for (_, _, &t) in temperature.iter() {
        if t < min_temp { min_temp = t; }
        if t > max_temp { max_temp = t; }
    }
    let mut min_precip = f32::MAX;
    let mut max_precip = f32::MIN;
    for (_, _, &p) in climate_sim.annual_precipitation.iter() {
        if p < min_precip { min_precip = p; }
        if p > max_precip { max_precip = p; }
    }
    println!("Temperature range: {:.1}°C to {:.1}°C", min_temp, max_temp);
    println!("Precipitation range: {:.0}mm to {:.0}mm/yr", min_precip, max_precip);

    // Apply erosion
    println!("Simulating erosion (preset: {})...", erosion_preset);
    let mut erosion_params = erosion::ErosionParams::from_preset(erosion_preset);
    erosion_params.tune_for_heightmap(&heightmap);

    // Override simulation_scale if --no-hires flag is set
    if args.no_hires {
        erosion_params.simulation_scale = 1;
        println!("  High-resolution erosion disabled (--no-hires)");
    }

    let mut erosion_rng = ChaCha8Rng::seed_from_u64(seeds.erosion);

    let (stats, hardness_map, flow_accumulation) = erosion::simulate_erosion(
        &mut heightmap,
        &plate_map,
        &plates,
        &stress_map,
        &temperature,
        &erosion_params,
        &mut erosion_rng,
        seeds.erosion,
    );

    println!("Erosion complete:");
    println!("  Total eroded: {:.1} units", stats.total_eroded);
    println!("  Total deposited: {:.1} units", stats.total_deposited);
    println!("  Max erosion: {:.2} units", stats.max_erosion);
    println!("  Max deposition: {:.2} units", stats.max_deposition);

    // Update heightmap stats after erosion
    min_h = f32::MAX;
    max_h = f32::MIN;
    for (_, _, &h) in heightmap.iter() {
        if h < min_h { min_h = h; }
        if h > max_h { max_h = h; }
    }
    println!("Post-erosion heightmap range: {:.1}m to {:.1}m", min_h, max_h);

    // Apply coastline jittering for more organic shorelines
    println!("Applying coastline jittering...");
    let coastline_params = coastline::CoastlineParams::default();
    let coastline_network = coastline::generate_coastline_network(&heightmap, &coastline_params, seeds.coastline);
    coastline::apply_coastline_to_heightmap(&coastline_network, &mut heightmap, coastline_params.blend_width);

    // Carve fjord channels into coastal terrain (creates narrow inlets like Norwegian fjords)
    println!("Carving fjord channels...");
    heightmap::apply_fjord_incisions(&mut heightmap, seeds.heightmap, &map_scale);

    // Apply terrain noise layers based on region type
    println!("Applying terrain noise layers...");
    heightmap::apply_regional_noise_stacks(&mut heightmap, &stress_map, seeds.heightmap);


    // Apply volcano pass to add volcanic cones based on tectonic stress
    println!("Placing volcanoes...");
    let volcanoes = heightmap::apply_volcano_pass(&mut heightmap, &stress_map, seeds.heightmap);

    // Enforce coastal beach strips near sea level (except high-stress cliffs)
    println!("Applying coastal beach pass...");
    heightmap::apply_coastal_beaches(&mut heightmap, &stress_map, &map_scale);

    // Generate lava for active volcanoes
    println!("Generating lava flows...");
    let lava_map = heightmap::generate_lava_map(&heightmap, &volcanoes, seeds.heightmap);

    // Use mean surface temperature from climate simulation (including Rossby waves, continentality, and maritime moderation)
    let temperature = climate_sim.mean_temperature.clone();

    // Detect water bodies (lakes, rivers, ocean) with water depth and climate coupling
    println!("Detecting water bodies with hydrological routing...");
    let (water_body_map, water_bodies_list, water_depth, flow_acc, flow_dir) =
        water_bodies::detect_water_bodies_climate(&heightmap, &temperature, &moisture);
    let lake_count = water_bodies::count_lakes(&water_bodies_list);
    let wb_stats = water_bodies::water_body_stats(&water_bodies_list);
    println!("Found {} lakes, {} river tiles, {} ocean tiles",
        lake_count, wb_stats.river_tiles, wb_stats.ocean_tiles);

    // Generate extended biomes for explorer
    let biome_config = biomes::WorldBiomeConfig {
        fantasy_intensity: args.fantasy.clamp(0.0, 1.0),
        ..biomes::WorldBiomeConfig::default()
    };
    let mut extended_biomes = biomes::generate_extended_biomes(
        &heightmap,
        &temperature,
        &moisture,
        &stress_map,
        &biome_config,
        seeds.biomes,
    );

    // Apply biome replacement rules (rare biomes replace common ones)
    println!("Applying rare biome replacements...");
    let rare_biome_clusters = biomes::apply_biome_replacements(
        &mut extended_biomes,
        &heightmap,
        &temperature,
        &moisture,
        &stress_map,
        biome_config.fantasy_intensity,
        seeds.biomes,
    );
    println!("Created {} rare biome clusters", rare_biome_clusters);

    // Apply fantasy lake conversions (transform entire lakes to LavaLake, FrozenLake, etc.)
    let fantasy_lakes_converted = water_bodies::apply_fantasy_lake_conversions(
        &mut extended_biomes,
        &water_bodies_list,
        &water_body_map,
        &temperature,
        &stress_map,
        seeds.biomes,
    );
    if fantasy_lakes_converted > 0 {
        println!("Converted {} lakes to fantasy biomes", fantasy_lakes_converted);
    }

    // Place unique biomes (exactly one per map)
    let unique_biomes_placed = biomes::place_unique_biomes(
        &mut extended_biomes,
        &heightmap,
        seeds.biomes,
    );
    if unique_biomes_placed > 0 {
        println!("Placed {} unique biomes", unique_biomes_placed);
    }

    // Apply volcanic biomes based on lava map
    let volcanic_tiles = biomes::apply_volcanic_biomes(
        &mut extended_biomes,
        &lava_map,
        &volcanoes,
        &heightmap,
        seeds.biomes,
    );
    if volcanic_tiles > 0 {
        println!("Converted {} tiles to volcanic biomes", volcanic_tiles);
    }

    if args.biome_stats {
        biomes::print_biome_stats(&extended_biomes, &heightmap, &temperature, &moisture);
    }

    // Compute biome feathering map for smooth transitions
    println!("Computing biome feathering map...");
    let feather_config = biome_feathering::FeatherConfig::default();
    let biome_feather_map = biome_feathering::compute_biome_feathering(
        &extended_biomes,
        &feather_config,
        seeds.biomes,
    );

    // Launch explorer
    println!("Launching terminal explorer...");
    // Generate Bezier river network with true flow accumulation and lake connectivity
    let river_network = crate::erosion::trace_bezier_rivers_with_flow(
        &heightmap,
        &flow_acc,
        &flow_dir,
        Some(&water_body_map),
        Some(&water_bodies_list),
        None,
        seeds.rivers,
    );

    // Calculate region handshakes for hierarchical zoom
    println!("Calculating region handshakes...");
    let handshake_input = region::HandshakeInput {
        heightmap: &heightmap,
        moisture: &moisture,
        temperature: &temperature,
        stress_map: &stress_map,
        biomes: &extended_biomes,
        hardness_map: Some(&hardness_map),
    };
    let mut world_handshakes = region::calculate_world_handshakes_full(&handshake_input);
    region::rivers::calculate_river_crossings(&mut world_handshakes.handshakes, &river_network);

    // Generate underground water features (aquifers, springs, waterfalls)
    println!("Generating underground water features...");
    let underground_water_params = underground_water::UndergroundWaterParams::default();
    let underground_water_features = underground_water::UndergroundWater::generate(
        &heightmap,
        &moisture,
        &stress_map,
        Some(&hardness_map),
        &underground_water_params,
    );

    // Log underground water statistics
    let uw_stats = underground_water_features.stats();
    println!("Underground water: {} aquifer tiles ({} unconfined, {} confined, {} perched)",
             uw_stats.aquifer_tiles,
             uw_stats.unconfined_aquifers,
             uw_stats.confined_aquifers,
             uw_stats.perched_aquifers);
    println!("Springs: {} total ({} seepage, {} artesian, {} thermal, {} karst)",
             uw_stats.spring_count,
             uw_stats.seepage_springs,
             uw_stats.artesian_springs,
             uw_stats.thermal_springs,
             uw_stats.karst_springs);
    if uw_stats.waterfall_count > 0 {
        println!("Waterfalls: {} (max height: {:.0}m)", uw_stats.waterfall_count, uw_stats.max_waterfall_height);
    }

    let mut world_data = world::WorldData::new(
        seeds.clone(),
        map_scale,
        heightmap,
        temperature,
        moisture,
        extended_biomes,
        stress_map,
        plate_map,
        plates,
        Some(hardness_map),
        water_body_map,
        water_bodies_list,
        water_depth,
        Some(river_network),
        Some(biome_feather_map),
    );
    world_data.handshakes = Some(world_handshakes);
    world_data.underground_water = Some(underground_water_features);
    world_data.set_flow_accumulation(flow_acc);
    world_data.set_volcanic_features(lava_map, volcanoes);
    let seasonal_climate = seasons::SeasonalClimate::from_simulation(&climate_sim, &world_data.heightmap);
    world_data.seasonal_climate = Some(seasonal_climate);

    world_data
    };

    // Export freshwater network if requested
    if args.export_rivers {
        let filename = format!("freshwater_{}.png", master_seed);
        if let Err(e) = explorer::export_freshwater_network_image(&world_data, &filename) {
            eprintln!("Failed to export freshwater network: {}", e);
        }
    }

    // Export base map if requested
    if args.export_base_map {
        let filename = format!("world_base_{}.png", master_seed);
        if let Err(e) = explorer::export_base_map_image(&world_data, &filename) {
            eprintln!("Failed to export base map: {}", e);
        }
    }

    // Export antique old paper cartography map if requested
    if args.export_cartography {
        let filename = format!("world_cartography_{}.png", master_seed);
        let mut params = cartography::CartographyParams::default();
        params.use_gpu = !args.cartography_cpu;
        params.style = match args.cartography_style.to_lowercase().as_str() {
            "atlas" => cartography::PaperStyle::Atlas17thCentury,
            "copperplate" | "engraving" => cartography::PaperStyle::CopperplateEngraving,
            "patina" | "antiquarian" => cartography::PaperStyle::AntiquarianPatina,
            _ => cartography::PaperStyle::AgedParchment,
        };

        if let Err(e) = cartography::export_cartography_image(&world_data, &filename, Some(&params)) {
            eprintln!("Failed to export cartography map: {}", e);
        }
    }

    // Export EXR files if requested
    if args.export_exr {
        let output_dir = std::path::Path::new(&args.exr_output_dir);
        if let Some(ref flow_acc) = world_data.flow_accumulation {
            if let Err(e) = exr_export::export_world_exr(
                &world_data.heightmap,
                flow_acc,
                &world_data.biomes,
                output_dir,
                master_seed,
            ) {
                eprintln!("Failed to export EXR files: {}", e);
            }
        } else {
            eprintln!("Warning: Flow accumulation not available, skipping river map export");
            // Export just the heightmap and biomes
            let heightmap_path = output_dir.join(format!("world_{}_heightmap.exr", master_seed));
            if let Err(e) = exr_export::export_heightmap_exr(&world_data.heightmap, &heightmap_path) {
                eprintln!("Failed to export heightmap EXR: {}", e);
            } else {
                println!("Exported heightmap to: {}", heightmap_path.display());
            }
            let biome_path = output_dir.join(format!("world_{}_biomes.exr", master_seed));
            if let Err(e) = exr_export::export_biome_map_exr(&world_data.biomes, &biome_path) {
                eprintln!("Failed to export biome EXR: {}", e);
            } else {
                println!("Exported biomes to: {}", biome_path.display());
            }
        }
    }

    // Export improved PNG maps if requested
    if args.export_maps {
        let output_dir = std::path::Path::new(&args.map_output_dir);
        let config = map_export::MapExportConfig {
            use_lut: !args.no_lut,
            dithering: !args.no_dither,
            dither_seed: master_seed,
            hillshade: true,
            hillshade_intensity: 0.6,
            height_exaggeration: 0.035,
            lut_biome_blend: 0.35, // 35% biome color, 65% LUT for smooth natural look
            upscale_factor: args.upscale_factor,
            target_resolution: None,
        };

        if let Err(e) = map_export::export_all_maps(&world_data, output_dir, master_seed, &config) {
            eprintln!("Failed to export maps: {}", e);
        }
    }

    // High-resolution zoom of a window of world tiles
    if let Some(spec) = &args.zoom {
        let tiles = args.zoom_tiles.max(1);
        let center = if spec.eq_ignore_ascii_case("auto") {
            Some(region::zoom::pick_interesting_window(&world_data, tiles))
        } else {
            let parts: Vec<_> = spec.split(',').map(|p| p.trim().parse::<usize>()).collect();
            match parts.as_slice() {
                [Ok(x), Ok(y)] if *x < width && *y < height => Some((*x, *y)),
                _ => {
                    eprintln!("--zoom expects \"X,Y\" (world tile inside {}x{}) or \"auto\", got '{}'", width, height, spec);
                    None
                }
            }
        };
        if let Some((cx, cy)) = center {
            let params = region::zoom::ZoomParams {
                center_x: cx,
                center_y: cy,
                tiles,
                cells_per_tile: args.zoom_scale,
                erosion_iterations: args.zoom_erosion,
                seed: master_seed,
            };
            println!(
                "Zooming into {}x{} world tiles centred on ({}, {}) at {} cells/tile...",
                tiles, tiles, cx, cy, args.zoom_scale
            );
            let t0 = std::time::Instant::now();
            let zoom = region::zoom::generate_zoom(&world_data, &params);
            let (river_cells, lake_cells) = zoom.stats();
            println!(
                "  {}x{} cells, {:.0} m/cell, {} river cells, {} lake cells ({:.1}s)",
                zoom.width, zoom.height, zoom.cell_m, river_cells, lake_cells, t0.elapsed().as_secs_f32()
            );
            let dir = std::path::Path::new(&args.map_output_dir);
            let stem = format!("zoom_{}_{}_{}", master_seed, cx, cy);
            match zoom.save_png(&dir.join(format!("{stem}.png"))) {
                Ok(()) => println!("  Saved {}", dir.join(format!("{stem}.png")).display()),
                Err(e) => eprintln!("  Failed to save zoom image: {e}"),
            }
            match zoom.save_heightmap16(&dir.join(format!("{stem}_height16.png"))) {
                Ok((lo, hi)) => println!("  Saved 16-bit heightmap ({:.0} m .. {:.0} m)", lo, hi),
                Err(e) => eprintln!("  Failed to save zoom heightmap: {e}"),
            }
        }
    }

    // Handle island listing and export
    if args.list_islands || args.export_islands {
        println!("\nDetecting islands...");
        let (island_map, all_islands) = islands::detect_islands(&world_data.heightmap);

        // Filter islands based on options
        let filtered_islands: Vec<&islands::Island> = all_islands.iter()
            .filter(|i| {
                // Filter by true islands if requested
                if args.islands_only && !i.is_true_island() {
                    return false;
                }
                // Filter by min size
                if i.tile_count < args.island_min_size {
                    return false;
                }
                // Filter by max size
                if let Some(max_size) = args.island_max_size {
                    if i.tile_count > max_size {
                        return false;
                    }
                }
                true
            })
            .collect();

        if args.list_islands {
            islands::print_island_summary(&all_islands);

            if args.islands_only || args.island_max_size.is_some() || args.island_min_size > 1 {
                println!("\n--- Filtered Results ({} islands match criteria) ---", filtered_islands.len());
                let mut sorted = filtered_islands.clone();
                sorted.sort_by_key(|i| std::cmp::Reverse(i.tile_count));
                for (i, island) in sorted.iter().enumerate() {
                    println!(
                        "  #{}: ID={} {} - {} tiles, elev {:.0}-{:.0}m, center ({:.0},{:.0})",
                        i + 1,
                        island.id.0,
                        island.size_category(),
                        island.tile_count,
                        island.min_elevation,
                        island.max_elevation,
                        island.center.0,
                        island.center.1,
                    );
                }
            }
        }

        if args.export_islands {
            let output_dir = std::path::Path::new(&args.island_output_dir);
            if !output_dir.exists() {
                if let Err(e) = std::fs::create_dir_all(output_dir) {
                    eprintln!("Failed to create island output directory: {}", e);
                } else {
                    println!("Created island output directory: {}", output_dir.display());
                }
            }

            println!("\nExporting {} islands to {}...", filtered_islands.len(), output_dir.display());

            // Configure export: PNG images + metadata by default
            let export_config = islands::IslandExportConfig {
                export_biomes: true,
                export_heightmap: true,
                export_csv: false,
                export_metadata: true,
            };

            for island in &filtered_islands {
                let exported = islands::export_island(&world_data, &island_map, island, args.island_padding);

                match islands::export_island_files(&exported, output_dir, &export_config) {
                    Ok(files) => {
                        for file in files {
                            println!("  Exported: {}", file);
                        }
                    }
                    Err(e) => {
                        eprintln!("  Failed to export island {}: {}", island.id.0, e);
                    }
                }
            }

            println!("Island export complete!");
        }

        // Export island grid
        if args.export_island_grid && !filtered_islands.is_empty() {
            let output_dir = std::path::Path::new(&args.island_output_dir);
            if !output_dir.exists() {
                let _ = std::fs::create_dir_all(output_dir);
            }

            // Export all filtered islands
            let exported_islands: Vec<_> = filtered_islands.iter()
                .map(|island| islands::export_island(&world_data, &island_map, island, args.island_padding))
                .collect();

            // Biome grid
            let biome_grid_path = output_dir.join("islands_grid_biomes.png");
            match islands::export_islands_grid(&exported_islands, &biome_grid_path, 4, args.island_grid_columns) {
                Ok(()) => println!("Exported island grid (biomes): {}", biome_grid_path.display()),
                Err(e) => eprintln!("Failed to export biome grid: {}", e),
            }

            // Heightmap grid
            let height_grid_path = output_dir.join("islands_grid_heightmap.png");
            match islands::export_islands_grid_heightmap(&exported_islands, &height_grid_path, 4, args.island_grid_columns) {
                Ok(()) => println!("Exported island grid (heightmap): {}", height_grid_path.display()),
                Err(e) => eprintln!("Failed to export heightmap grid: {}", e),
            }
        }

        // Export complete island WorldData
        if args.export_island_data && !filtered_islands.is_empty() {
            let base_output_dir = std::path::Path::new(&args.island_output_dir);

            println!("\nExporting complete WorldData for {} islands...", filtered_islands.len());

            let export_config = islands::IslandDataExportConfig {
                binary_format: !args.island_data_csv,
                png_images: true,
                ..Default::default()
            };

            for island in &filtered_islands {
                let exported = islands::export_island(&world_data, &island_map, island, args.island_padding);

                // Create subdirectory for each island
                let island_dir = base_output_dir.join(format!("island_{}_{}tiles", island.id.0, island.tile_count));

                match islands::export_island_world_data(&exported, &island_dir, &export_config) {
                    Ok(files) => {
                        println!("Island {} ({} tiles): {} files exported to {}",
                            island.id.0, island.tile_count, files.len(), island_dir.display());
                    }
                    Err(e) => {
                        eprintln!("Failed to export island {} data: {}", island.id.0, e);
                    }
                }
            }

            println!("Island data export complete!");
        }
    }

    // Export world data if requested
    if args.export_world_data {
        let output_dir = std::path::Path::new(&args.world_data_dir);
        println!("\nExporting world data to {}...", output_dir.display());

        let export_config = world_export::WorldExportConfig {
            binary_arrays: !args.world_data_csv,
            per_tile_details: args.world_data_per_tile,
            ..Default::default()
        };

        match world_export::export_world_data(&world_data, output_dir, &export_config) {
            Ok(files) => {
                println!("Exported {} files:", files.len());
                for file in files {
                    println!("  {}", file);
                }
            }
            Err(e) => eprintln!("Failed to export world data: {}", e),
        }
    }

    // Run benchmark mode if requested (early return)
    if let Some(num_runs) = args.benchmark {
        let history_seed = args.history_seed.unwrap_or(master_seed.wrapping_add(1000));
        let history_years = if args.history_years > 0 { args.history_years } else { 200 };
        let config = history::config::HistoryConfig {
            simulation_years: history_years,
            ..history::config::HistoryConfig::default()
        };
        let game_data = history::data::GameData::load_from(std::path::Path::new("data"));

        eprintln!("Running {} benchmark simulations ({} years each)...", num_runs, history_years);

        let batch_config = history::simulation::harness::BatchConfig {
            name: "benchmark".to_string(),
            history_config: config,
            num_runs,
            base_seed: history_seed,
        };

        let results = history::simulation::harness::run_batch(&world_data, &batch_config, &game_data);
        eprintln!("{}", results.report());
        return;
    }

    // Load or simulate history
    let wants_tiles = args.tiles || args.tiles_snapshot.is_some() || args.local_snapshot.is_some();
    let history = if loaded_history.is_some() {
        loaded_history.take()
    } else if let Some(ref load_path) = args.load_history {
        eprintln!("Loading history from {}...", load_path);
        match history::persistence::load_history(std::path::Path::new(load_path)) {
            Ok(loaded) => {
                let summary = loaded.history.summary();
                eprintln!("{}", summary);
                Some(loaded.history)
            }
            Err(e) => {
                eprintln!("Failed to load history: {}", e);
                None
            }
        }
    } else if args.history_years > 0 || (wants_tiles && !args.no_history) {
        let history_seed = args.history_seed.unwrap_or(master_seed.wrapping_add(1000));
        // The tile viewer shows history on the land, so it simulates some by default.
        let years = if args.history_years > 0 { args.history_years } else { DEFAULT_VIEWER_HISTORY_YEARS };
        let config = history::config::HistoryConfig {
            simulation_years: years,
            ..history::config::HistoryConfig::default()
        };
        // Load game data (embedded defaults + optional data/ directory overrides)
        let game_data = history::data::GameData::load_from(std::path::Path::new("data"));
        eprintln!("Simulating {} years of history (seed: {})...", years, history_seed);
        let mut engine = history::simulation::HistoryEngine::new(history_seed);
        let hist = engine.simulate_with_data(&world_data, config, &game_data);
        let summary = hist.summary();
        eprintln!("{}", summary);
        Some(hist)
    } else {
        None
    };

    if args.gazetteer {
        let t0 = std::time::Instant::now();
        let gaz = lore::build_gazetteer(&world_data, history.as_ref(), master_seed);
        println!("Gazetteer: {} named features in {:.2}s", gaz.features.len(), t0.elapsed().as_secs_f32());
        if let Some(h) = history.as_ref() {
            let living = h.settlements.values().filter(|s| !s.is_destroyed()).count();
            let ruins = h.settlements.len() - living;
            let roads = (0..width * height).filter(|&i| h.tile_history.has_road(i % width, i / width)).count();
            let owned = (0..width * height).filter(|&i| h.tile_history.get(i % width, i / width).current_owner.is_some()).count();
            let named = gaz.features.iter().filter(|f| f.named_by.is_some()).count();
            println!("History on the map: {} living settlements, {} ruins, {} road tiles, {} claimed tiles; {} features named by a living culture",
                living, ruins, roads, owned, named);
        }
        let mut by_kind: std::collections::BTreeMap<String, Vec<&lore::Feature>> = Default::default();
        for f in &gaz.features {
            by_kind.entry(format!("{:?}", f.kind)).or_default().push(f);
        }
        for (kind, mut fs) in by_kind {
            fs.sort_by_key(|f| std::cmp::Reverse(f.size));
            let sample: Vec<String> = fs.iter().take(6).map(|f| {
                let by = f.named_by.and_then(|id| history.as_ref().and_then(|h| h.factions.get(&id)).map(|fa| fa.name.clone()));
                match by { Some(b) => format!("{} [{}]", f.name, b), None => f.name.clone() }
            }).collect();
            println!("  {:>14} x{:<4} {}", kind, fs.len(), sample.join("; "));
        }
    }

    if let Some(path) = &args.save_world {
        let t0 = std::time::Instant::now();
        match world::save_world(&world_data, history.as_ref(), std::path::Path::new(path)) {
            Ok(()) => {
                let mb = std::fs::metadata(path).map(|m| m.len() as f64 / 1.0e6).unwrap_or(0.0);
                eprintln!("Saved world to {} ({:.0} MB) in {:.1}s", path, mb, t0.elapsed().as_secs_f32());
            }
            Err(e) => eprintln!("Failed to save world: {e}"),
        }
    }

    // Save history if requested
    if let (Some(ref hist), Some(ref save_path)) = (&history, &args.save_history) {
        let history_seed = args.history_seed.unwrap_or(master_seed.wrapping_add(1000));
        eprintln!("Saving history to {}...", save_path);
        if let Err(e) = history::persistence::save_history(
            hist, master_seed, history_seed, std::path::Path::new(save_path),
        ) {
            eprintln!("Failed to save history: {}", e);
        } else {
            eprintln!("History saved.");
        }
    }

    // Export legends if requested
    if let (Some(ref hist), Some(ref export_path)) = (&history, &args.export_legends) {
        eprintln!("Exporting legends to {}...", export_path);
        let path = std::path::Path::new(export_path);
        let result = if export_path.ends_with(".md") {
            history::persistence::export_legends_markdown(hist, path)
        } else {
            history::persistence::export_legends_text(hist, path)
        };
        if let Err(e) = result {
            eprintln!("Failed to export legends: {}", e);
        } else {
            eprintln!("Legends exported.");
        }
    }

    // Skip explorer in headless mode
    if args.headless {
        return;
    }

    if args.tiles || args.tiles_snapshot.is_some() || args.local_snapshot.is_some() {
        let atlas = match &args.tileset {
            Some(path) => match tiles::Atlas::load_png(std::path::Path::new(path)) {
                Ok(a) => a,
                Err(e) => {
                    eprintln!("Could not load tileset {path}: {e}; using the built-in tiles");
                    tiles::Atlas::generated()
                }
            },
            None => tiles::Atlas::generated(),
        };
        let center = args.tiles_center.as_deref().and_then(|spec| {
            let p: Vec<_> = spec.split(',').map(|v| v.trim().parse::<usize>()).collect();
            match p.as_slice() {
                [Ok(x), Ok(y)] if *x < width && *y < height => Some((*x, *y)),
                _ => {
                    eprintln!("--tiles-center expects \"X,Y\" inside {}x{}, got '{}'", width, height, spec);
                    None
                }
            }
        });
        if let Some(prefix) = &args.local_snapshot {
            match tiles::viewer::save_local_snapshots(&world_data, history.as_ref(), &atlas, center, prefix) {
                Ok(files) => println!("Saved playable-area snapshots: {}", files.join(", ")),
                Err(e) => eprintln!("Playable-area snapshot failed: {e}"),
            }
            return;
        }
        if let Some(prefix) = &args.tiles_snapshot {
            match tiles::viewer::save_snapshots(&world_data, history.as_ref(), &atlas, prefix, center, match args.season.to_lowercase().as_str() {
                "spring" => seasons::Season::Spring,
                "autumn" | "fall" => seasons::Season::Autumn,
                "winter" => seasons::Season::Winter,
                _ => seasons::Season::Summer,
            }) {
                Ok(files) => println!("Saved tile snapshots: {}", files.join(", ")),
                Err(e) => eprintln!("Tile snapshot failed: {e}"),
            }
            return;
        }
        if let Err(e) = tiles::run_tile_viewer(&world_data, history.as_ref(), atlas, center) {
            eprintln!("Tile viewer error: {}", e);
        }
        return;
    }

    if let Err(e) = explorer::run_explorer(world_data, history) {
        eprintln!("Explorer error: {}", e);
    }
}
