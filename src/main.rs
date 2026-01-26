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
mod climate;
mod coastline;
mod erosion;
mod explorer;
mod exr_export;
mod grid_export;
mod heightmap;
mod map_export;
mod menu;
mod microclimate;
mod plates;
mod region;
mod scale;
mod seasons;
mod seeds;
mod tilemap;
mod underground_water;
mod water_bodies;
mod weather_zones;
mod world;

use menu::{MenuResult, WorldConfig};
use seeds::WorldSeeds;

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
}

fn main() {
    let args = Args::parse();

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

    // Determine configuration: use menu if no seed provided (interactive mode),
    // otherwise use CLI args directly (batch mode)
    let (width, height, master_seed, plates_count, world_style, erosion_preset, climate_config) = if args.seed.is_some() {
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

    // Initialize RNG for tectonics (plates need RNG)
    let mut tectonic_rng = ChaCha8Rng::seed_from_u64(seeds.tectonics);

    // Generate tectonic plates
    println!("Generating tectonic plates...");
    let (plate_map, plates) = plates::generate_plates(width, height, plates_count, world_style, &mut tectonic_rng);
    let continental_count = plates.iter().filter(|p| p.plate_type == plates::PlateType::Continental).count();
    let oceanic_count = plates.iter().filter(|p| p.plate_type == plates::PlateType::Oceanic).count();
    println!("Created {} plates ({} continental, {} oceanic)", plates.len(), continental_count, oceanic_count);

    // Calculate stress at plate boundaries
    println!("Calculating plate stress...");
    let stress_map = plates::calculate_stress(&plate_map, &plates);

    // Create map scale for coordinate scaling (used throughout generation)
    let map_scale = scale::MapScale::default();

    // Generate heightmap
    println!("Generating heightmap...");
    let land_mask = heightmap::generate_land_mask(&plate_map, &plates, seeds.heightmap);
    let land_count = (0..height).flat_map(|y| (0..width).map(move |x| (x, y)))
        .filter(|&(x, y)| *land_mask.get(x, y)).count();
    println!("Land mask: {} cells are land ({:.1}%)", land_count, 100.0 * land_count as f64 / (width * height) as f64);
    let mut heightmap = heightmap::generate_heightmap(&plate_map, &plates, &stress_map, seeds.heightmap);
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

    // Generate climate with domain warping for organic zone boundaries
    println!("Generating climate (mode: {}, rainfall: {})...", climate_config.mode, climate_config.rainfall);
    let temperature = climate::generate_temperature_with_seed(&heightmap, width, height, climate_config.mode, seeds.climate);
    let moisture = climate::generate_moisture_with_config_and_seed(&heightmap, width, height, &climate_config, seeds.climate);

    // Report climate stats
    let mut min_temp = f32::MAX;
    let mut max_temp = f32::MIN;
    for (_, _, &t) in temperature.iter() {
        if t < min_temp { min_temp = t; }
        if t > max_temp { max_temp = t; }
    }
    println!("Temperature range: {:.1}°C to {:.1}°C", min_temp, max_temp);

    // Apply erosion
    println!("Simulating erosion (preset: {})...", erosion_preset);
    let mut erosion_params = erosion::ErosionParams::from_preset(erosion_preset);

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

    // Apply archipelago pass to add islands in shallow ocean near stress zones
    println!("Applying archipelago pass...");
    heightmap::apply_archipelago_pass(&mut heightmap, &stress_map, seeds.heightmap);

    // Expand single-tile islands into proper multi-tile islands
    println!("Expanding island clusters...");
    heightmap::expand_island_clusters(&mut heightmap, &stress_map, seeds.heightmap);

    // Apply volcano pass to add volcanic cones based on tectonic stress
    println!("Placing volcanoes...");
    let volcanoes = heightmap::apply_volcano_pass(&mut heightmap, &stress_map, seeds.heightmap);

    // Generate lava for active volcanoes
    println!("Generating lava flows...");
    let lava_map = heightmap::generate_lava_map(&heightmap, &volcanoes, seeds.heightmap);

    // CRITICAL: Final depression fill to ensure river connectivity
    // Post-processing steps (coastline, noise) may have created new pits
    println!("Final depression fill for river connectivity...");
    let filled = erosion::rivers::fill_depressions_public(&heightmap);
    for y in 0..height {
        for x in 0..width {
            heightmap.set(x, y, *filled.get(x, y));
        }
    }

    // Detect water bodies (lakes, rivers, ocean) with water depth
    println!("Detecting water bodies...");
    let (water_body_map, water_bodies_list, water_depth) = water_bodies::detect_water_bodies(&heightmap);
    let lake_count = water_bodies::count_lakes(&water_bodies_list);
    let wb_stats = water_bodies::water_body_stats(&water_bodies_list);
    println!("Found {} lakes, {} river tiles, {} ocean tiles",
        lake_count, wb_stats.river_tiles, wb_stats.ocean_tiles);

    // Generate extended biomes for explorer
    let biome_config = biomes::WorldBiomeConfig::default();
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
    // Generate Bezier river network
    let river_network = crate::erosion::trace_bezier_rivers(&heightmap, None, seeds.rivers);

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
    world_data.set_flow_accumulation(flow_accumulation);
    world_data.set_volcanic_features(lava_map, volcanoes);

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
        };

        if let Err(e) = map_export::export_all_maps(&world_data, output_dir, master_seed, &config) {
            eprintln!("Failed to export maps: {}", e);
        }
    }

    // Skip explorer in headless mode
    if args.headless {
        return;
    }

    if let Err(e) = explorer::run_explorer(world_data) {
        eprintln!("Explorer error: {}", e);
    }
}
