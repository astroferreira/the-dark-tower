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
mod heightmap;
mod history;
mod multiscale;
mod plates;
mod scale;
mod structures;
mod tilemap;
mod water_bodies;
mod world;
mod zlevel;

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

    /// Random seed (uses random seed if not specified)
    #[arg(short, long)]
    seed: Option<u64>,

    /// Number of tectonic plates (random 6-15 if not specified)
    #[arg(short = 'p', long)]
    plates: Option<usize>,

    /// Export timeline to a text file (e.g., "chronicle.txt")
    #[arg(long)]
    export_timeline: Option<String>,

    /// Export local maps to PNG (specify output path)
    #[arg(long)]
    export_local: Option<String>,

    /// Center X coordinate for local export (default: center of map)
    #[arg(long)]
    export_local_x: Option<usize>,

    /// Center Y coordinate for local export (default: center of map)
    #[arg(long)]
    export_local_y: Option<usize>,

    /// Radius in chunks for local export (default: 5)
    #[arg(long, default_value = "5")]
    export_local_radius: usize,

    /// Scale factor for local export (1-4, default: 1)
    #[arg(long, default_value = "1")]
    export_local_scale: u32,

    /// Show chunk grid in local export
    #[arg(long)]
    export_local_grid: bool,

    /// Export debug info for local maps (text file for analysis)
    #[arg(long)]
    debug_local: Option<String>,

    /// X coordinate for debug export (default: center of map)
    #[arg(long)]
    debug_local_x: Option<usize>,

    /// Y coordinate for debug export (default: center of map)
    #[arg(long)]
    debug_local_y: Option<usize>,
}

fn main() {
    let args = Args::parse();

    // Initialize RNG
    let seed = args.seed.unwrap_or_else(|| rand::random());
    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    println!("Generating planet with seed: {}", seed);
    println!("Map size: {}x{}", args.width, args.height);

    // Generate tectonic plates
    println!("Generating tectonic plates...");
    let (plate_map, plates) = plates::generate_plates(args.width, args.height, args.plates, &mut rng);
    let continental_count = plates.iter().filter(|p| p.plate_type == plates::PlateType::Continental).count();
    let oceanic_count = plates.iter().filter(|p| p.plate_type == plates::PlateType::Oceanic).count();
    println!("Created {} plates ({} continental, {} oceanic)", plates.len(), continental_count, oceanic_count);

    // Calculate stress at plate boundaries
    println!("Calculating plate stress...");
    let stress_map = plates::calculate_stress(&plate_map, &plates);

    // Generate heightmap
    println!("Generating heightmap...");
    let land_mask = heightmap::generate_land_mask(&plate_map, &plates, seed);
    let land_count = (0..args.height).flat_map(|y| (0..args.width).map(move |x| (x, y)))
        .filter(|&(x, y)| *land_mask.get(x, y)).count();
    println!("Land mask: {} cells are land ({:.1}%)", land_count, 100.0 * land_count as f64 / (args.width * args.height) as f64);
    let mut heightmap = heightmap::generate_heightmap(&plate_map, &plates, &stress_map, seed);
    let mut min_h = f32::MAX;
    let mut max_h = f32::MIN;
    for (_, _, &h) in heightmap.iter() {
        if h < min_h { min_h = h; }
        if h > max_h { max_h = h; }
    }
    let above_sea = (0..args.height).flat_map(|y| (0..args.width).map(move |x| (x, y)))
        .filter(|&(x, y)| *heightmap.get(x, y) > 0.0).count();
    println!("Heightmap range: {:.1}m to {:.1}m ({:.1}% above sea level)", min_h, max_h,
        100.0 * above_sea as f64 / (args.width * args.height) as f64);

    // Generate climate (needed for glacial erosion temperature zones)
    println!("Generating climate...");
    let temperature = climate::generate_temperature(&heightmap, args.width, args.height);
    let moisture = climate::generate_moisture(&heightmap, args.width, args.height);

    // Report climate stats
    let mut min_temp = f32::MAX;
    let mut max_temp = f32::MIN;
    for (_, _, &t) in temperature.iter() {
        if t < min_temp { min_temp = t; }
        if t > max_temp { max_temp = t; }
    }
    println!("Temperature range: {:.1}°C to {:.1}°C", min_temp, max_temp);

    // Hardness map (defaults to 0.5 if erosion is disabled/not run)
    let mut hardness_map = tilemap::Tilemap::new_with(args.width, args.height, 0.5f32);

    // Apply erosion
    println!("Simulating erosion...");
    let erosion_params = erosion::ErosionParams::default();

    let (stats, h_map) = erosion::simulate_erosion(
        &mut heightmap,
        &plate_map,
        &plates,
        &stress_map,
        &temperature,
        &erosion_params,
        &mut rng,
        seed,
    );
    hardness_map = h_map;

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
    let coastline_network = coastline::generate_coastline_network(&heightmap, &coastline_params, seed);
    coastline::apply_coastline_to_heightmap(&coastline_network, &mut heightmap, coastline_params.blend_width);

    // Apply terrain noise layers based on region type
    println!("Applying terrain noise layers...");
    heightmap::apply_regional_noise_stacks(&mut heightmap, &stress_map, seed);

    // Detect water bodies (lakes, rivers, ocean)
    println!("Detecting water bodies...");
    let (water_body_map, water_bodies_list) = water_bodies::detect_water_bodies(&heightmap);
    let lake_count = water_bodies::count_lakes(&water_bodies_list);
    let stats = water_bodies::water_body_stats(&water_bodies_list);
    println!("Found {} lakes, {} river tiles, {} ocean tiles",
        lake_count, stats.river_tiles, stats.ocean_tiles);

    // Generate extended biomes for explorer
    let biome_config = biomes::WorldBiomeConfig::default();
    let mut extended_biomes = biomes::generate_extended_biomes(
        &heightmap,
        &temperature,
        &moisture,
        &stress_map,
        &biome_config,
        seed,
    );

    // Apply biome replacement rules (rare biomes replace common ones)
    println!("Applying rare biome replacements...");
    let rare_biome_clusters = biomes::apply_biome_replacements(
        &mut extended_biomes,
        &heightmap,
        &temperature,
        &moisture,
        &stress_map,
        seed,
    );
    println!("Created {} rare biome clusters", rare_biome_clusters);

    // Apply fantasy lake conversions (transform entire lakes to LavaLake, FrozenLake, etc.)
    let fantasy_lakes_converted = water_bodies::apply_fantasy_lake_conversions(
        &mut extended_biomes,
        &water_bodies_list,
        &water_body_map,
        &temperature,
        &stress_map,
        seed,
    );
    if fantasy_lakes_converted > 0 {
        println!("Converted {} lakes to fantasy biomes", fantasy_lakes_converted);
    }

    // Place unique biomes (exactly one per map)
    let unique_biomes_placed = biomes::place_unique_biomes(
        &mut extended_biomes,
        &heightmap,
        seed,
    );
    if unique_biomes_placed > 0 {
        println!("Placed {} unique biomes", unique_biomes_placed);
    }

    // Compute biome feathering map for smooth transitions
    println!("Computing biome feathering map...");
    let feather_config = biome_feathering::FeatherConfig::default();
    let biome_feather_map = biome_feathering::compute_biome_feathering(
        &extended_biomes,
        &feather_config,
        seed,
    );

    // Generate Z-level data
    println!("Generating Z-level data...");
    let (mut zlevels, surface_z) = zlevel::generate_zlevels(&heightmap);
    println!("Z-levels: {} to {} ({} levels)", zlevel::MIN_Z, zlevel::MAX_Z, zlevel::Z_LEVEL_COUNT);

    // Generate underground water system
    println!("Generating underground water...");
    zlevel::generate_underground_water(
        &mut zlevels,
        &surface_z,
        &heightmap,
        &moisture,
        seed,
    );

    // Generate Dwarf Fortress-style cave system
    println!("Generating cave system...");
    zlevel::generate_caves(
        &mut zlevels,
        &surface_z,
        &heightmap,
        &moisture,
        &stress_map,
        seed,
    );

    // Generate human-made structures (castles, cities, villages, roads)
    println!("Generating structures...");
    let _placed_structures = structures::generate_structures(
        &mut zlevels,
        &surface_z,
        &heightmap,
        &moisture,
        &temperature,
        &extended_biomes,
        &stress_map,
        &water_body_map,
        seed,
    );

    // Generate world history (factions, events, settlements, monsters, trade)
    println!("Generating world history...");
    let world_history = history::generate_world_history(
        &mut zlevels,
        &surface_z,
        &heightmap,
        &extended_biomes,
        &water_body_map,
        &stress_map,
        seed,
    );

    // Export timeline if requested
    if let Some(ref filename) = args.export_timeline {
        if let Err(e) = world_history.export_timeline(filename) {
            eprintln!("Failed to export timeline: {}", e);
        }
    }

    // Launch explorer
    println!("Launching terminal explorer...");
    let map_scale = scale::MapScale::default();
    // Generate Bezier river network (Phase 1)
    let river_network = crate::erosion::trace_bezier_rivers(&heightmap, None, seed);

    let world_data = world::WorldData::new(
        seed,
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
        zlevels,
        surface_z,
        Some(world_history),
        Some(river_network),
        Some(biome_feather_map),
    );

    // Export local maps if requested
    if let Some(ref export_path) = args.export_local {
        use multiscale::{export_local_area, ExportOptions};

        let center_x = args.export_local_x.unwrap_or(args.width / 2);
        let center_y = args.export_local_y.unwrap_or(args.height / 2);
        let radius = args.export_local_radius;

        println!("Exporting local maps...");
        println!("  Center: ({}, {})", center_x, center_y);
        println!("  Radius: {} chunks", radius);
        println!("  Scale: {}x", args.export_local_scale);

        let options = ExportOptions {
            z_level: None,
            auto_surface: true,
            show_features: true,
            scale: args.export_local_scale.clamp(1, 4),
            show_chunk_grid: args.export_local_grid,
        };

        match export_local_area(&world_data, center_x, center_y, radius, export_path, &options) {
            Ok((width, height)) => {
                println!("Exported local maps to: {}", export_path);
                println!("  Image size: {}x{} pixels", width, height);
            }
            Err(e) => {
                eprintln!("Failed to export local maps: {}", e);
            }
        }
    }

    // Export debug info for local maps if requested
    if let Some(ref debug_path) = args.debug_local {
        use multiscale::export_debug_local_maps;

        let center_x = args.debug_local_x.unwrap_or(args.width / 2);
        let center_y = args.debug_local_y.unwrap_or(args.height / 2);

        println!("Exporting debug local map info...");
        println!("  Center: ({}, {})", center_x, center_y);

        match export_debug_local_maps(&world_data, center_x, center_y, debug_path) {
            Ok(()) => {
                println!("Debug export saved to: {}", debug_path);
            }
            Err(e) => {
                eprintln!("Failed to export debug info: {}", e);
            }
        }

        // Exit without launching explorer when debug exporting
        return;
    }

    // Export local maps exits early too
    if args.export_local.is_some() {
        return;
    }

    if let Err(e) = explorer::run_explorer(world_data) {
        eprintln!("Explorer error: {}", e);
    }
}
