use clap::Parser;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

mod ascii;
mod biomes;
mod climate;
mod erosion;
mod explorer;
mod export;
mod heightmap;
mod lore;
mod plates;
mod scale;
mod tilemap;
mod tileset;
mod viewer;
mod water_bodies;
mod world;

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

    /// Output file prefix
    #[arg(short, long, default_value = "output")]
    output: String,

    /// Number of iterations for stress spreading
    #[arg(long, default_value = "10")]
    stress_spread: usize,

    /// Number of tectonic plates (random 6-15 if not specified)
    #[arg(short = 'p', long)]
    plates: Option<usize>,

    /// Open interactive viewer (press R to regenerate, Esc to exit)
    #[arg(short = 'v', long)]
    view: bool,

    /// Disable erosion simulation
    #[arg(long)]
    no_erosion: bool,

    /// Number of hydraulic erosion iterations (droplets)
    #[arg(long, default_value = "200000")]
    erosion_iterations: usize,

    /// Disable flow-based river erosion
    #[arg(long)]
    no_rivers: bool,

    /// Disable particle-based hydraulic erosion
    #[arg(long)]
    no_hydraulic: bool,

    /// Disable glacial (ice) erosion
    #[arg(long)]
    no_glacial: bool,

    /// Number of glacial simulation timesteps
    #[arg(long, default_value = "500")]
    glacial_timesteps: usize,

    /// Enable geomorphometry analysis (realism scoring)
    #[arg(long)]
    analyze: bool,

    /// Print height histogram for debugging
    #[arg(long)]
    histogram: bool,

    /// Export world as ASCII text file
    #[arg(long)]
    ascii_export: Option<String>,

    /// Include verbose tile data in ASCII export
    #[arg(long)]
    verbose: bool,

    /// Export ASCII biome map as PNG image
    #[arg(long)]
    ascii_png: Option<String>,

    /// Launch terminal explorer mode
    #[arg(long)]
    explore: bool,

    /// Enable lore/storytelling generation
    #[arg(long)]
    lore: bool,

    /// Number of wandering storytellers for lore generation
    #[arg(long, default_value = "5")]
    lore_wanderers: usize,

    /// Output prefix for lore files
    #[arg(long, default_value = "lore")]
    lore_output: String,

    /// Include LLM prompt templates in lore JSON output
    #[arg(long)]
    lore_llm_prompts: bool,

    /// Separate seed for lore generation (uses world seed if not specified)
    #[arg(long)]
    lore_seed: Option<u64>,
}

fn main() {
    let args = Args::parse();

    // If --view flag is set, launch interactive viewer
    if args.view {
        viewer::run_viewer(args.width, args.height, args.seed);
        return;
    }

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

    // Print histogram before erosion if requested
    if args.histogram {
        println!("Pre-erosion height distribution:");
        heightmap::print_height_histogram(&heightmap, 20);
    }

    let heightmap_normalized = heightmap::normalize_heightmap(&heightmap);

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

    // Generate biomes (after erosion, if applied)
    let biomes = climate::generate_biomes(&heightmap, &temperature, &moisture);

    // Hardness map (defaults to 0.5 if erosion is disabled/not run)
    let mut hardness_map = crate::tilemap::Tilemap::new_with(args.width, args.height, 0.5f32);

    // Apply erosion (enabled by default, use --no-erosion to skip)
    if !args.no_erosion {
        println!("Simulating erosion...");

        let mut erosion_params = erosion::ErosionParams::default();
        erosion_params.hydraulic_iterations = args.erosion_iterations;
        erosion_params.glacial_timesteps = args.glacial_timesteps;
        erosion_params.enable_analysis = args.analyze;
        // Only override if explicitly disabled via CLI
        if args.no_rivers { erosion_params.enable_rivers = false; }
        if args.no_hydraulic { erosion_params.enable_hydraulic = false; }
        if args.no_glacial { erosion_params.enable_glacial = false; }

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

        // Print histogram after erosion if requested
        if args.histogram {
            println!("Post-erosion height distribution:");
            heightmap::print_height_histogram(&heightmap, 20);
        }
    }

    // Export combined grid image
    let grid_path = format!("{}.png", args.output);
    println!("Exporting combined grid to {}...", grid_path);
    export::export_combined_grid(
        &heightmap,
        &heightmap_normalized,
        &plate_map,
        &plates,
        &stress_map,
        &biomes,
        &hardness_map,
        &grid_path,
        seed,
    )
    .expect("Failed to export combined grid");

    // Detect water bodies (lakes, rivers, ocean)
    println!("Detecting water bodies...");
    let (water_body_map, water_bodies_list) = water_bodies::detect_water_bodies(&heightmap);
    let lake_count = water_bodies::count_lakes(&water_bodies_list);
    let stats = water_bodies::water_body_stats(&water_bodies_list);
    println!("Found {} lakes, {} river tiles, {} ocean tiles",
        lake_count, stats.river_tiles, stats.ocean_tiles);

    // Generate extended biomes for ASCII export and explorer
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

    // Generate lore if requested
    if args.lore {
        println!("Generating world lore...");

        let lore_seed = args.lore_seed.unwrap_or(seed);
        let mut lore_rng = ChaCha8Rng::seed_from_u64(lore_seed);

        let mut lore_params = lore::LoreParams::default();
        lore_params.num_wanderers = args.lore_wanderers;
        lore_params.include_llm_prompts = args.lore_llm_prompts;

        // Create WorldData for lore generation
        let map_scale = scale::MapScale::default();
        let world_data = world::WorldData::new(
            seed,
            map_scale.clone(),
            heightmap.clone(),
            temperature.clone(),
            moisture.clone(),
            extended_biomes.clone(),
            stress_map.clone(),
            plate_map.clone(),
            plates.clone(),
            Some(hardness_map.clone()),
            water_body_map.clone(),
            water_bodies_list.clone(),
        );

        let lore_result = lore::generate_lore(&world_data, &lore_params, &mut lore_rng);

        println!("Lore generation complete:");
        println!("  Wanderers: {}", lore_result.stats.wanderers_created);
        println!("  Total steps: {}", lore_result.stats.total_steps_taken);
        println!("  Landmarks discovered: {}", lore_result.stats.landmarks_discovered);
        println!("  Story seeds: {}", lore_result.stats.story_seeds_generated);
        println!("  Encounters: {}", lore_result.stats.encounters_processed);

        // Export JSON
        let json_path = format!("{}_lore.json", args.lore_output);
        println!("Exporting lore to {}...", json_path);
        if let Err(e) = lore::export_json(&lore_result, &json_path, &world_data, &lore_params) {
            eprintln!("Failed to export lore JSON: {}", e);
        }

        // Export narrative text
        let narrative_path = format!("{}_narrative.txt", args.lore_output);
        println!("Exporting narrative to {}...", narrative_path);
        if let Err(e) = lore::export_narrative(&lore_result, &narrative_path) {
            eprintln!("Failed to export narrative: {}", e);
        }

        println!("Lore export complete!");
    }

    // ASCII export if requested
    if let Some(ascii_path) = &args.ascii_export {
        println!("Exporting ASCII world to {}...", ascii_path);
        let map_scale = scale::MapScale::default();
        ascii::export_world_file(
            &heightmap,
            &extended_biomes,
            &temperature,
            &moisture,
            &stress_map,
            &plate_map,
            &plates,
            &map_scale,
            seed,
            ascii_path,
            args.verbose,
        )
        .expect("Failed to export ASCII world file");
        println!("ASCII export complete!");
    }

    // ASCII PNG export if requested
    if let Some(png_path) = &args.ascii_png {
        println!("Exporting ASCII biome map as PNG to {}...", png_path);
        ascii::export_ascii_png(&extended_biomes, png_path)
            .expect("Failed to export ASCII PNG");
        println!("ASCII PNG export complete!");
    }

    // Launch explorer if requested
    if args.explore {
        println!("Launching terminal explorer...");

        let map_scale = scale::MapScale::default();
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
        );
        if let Err(e) = explorer::run_explorer(world_data) {
            eprintln!("Explorer error: {}", e);
        }
        return;
    }

    println!("Done!");
}

