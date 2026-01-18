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
mod local;
mod lore;
mod plates;
mod scale;
mod simulation;
mod tilemap;
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

    /// Quick launch interactive explorer (faster, less detail)
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

    /// Launch terminal explorer with full world generation (erosion, water bodies, etc.)
    #[arg(long)]
    explore: bool,

    /// Enable lore/storytelling generation
    #[arg(long)]
    lore: bool,

    /// Number of wandering storytellers for lore generation
    #[arg(long, default_value = "5")]
    lore_wanderers: usize,

    /// Maximum steps per wanderer (lower = less wandering, faster generation)
    #[arg(long, default_value = "5000")]
    lore_steps: usize,

    /// Output prefix for lore files
    #[arg(long, default_value = "lore")]
    lore_output: String,

    /// Include LLM prompt templates in lore JSON output
    #[arg(long)]
    lore_llm_prompts: bool,

    /// Separate seed for lore generation (uses world seed if not specified)
    #[arg(long)]
    lore_seed: Option<u64>,

    /// Use LLM server to generate rich stories (requires --lore)
    #[arg(long)]
    llm: bool,

    /// LLM server URL (OpenAI-compatible API)
    #[arg(long, default_value = "http://192.168.8.59:8000")]
    llm_url: String,

    /// Model name for LLM (optional, server default if not specified)
    #[arg(long)]
    llm_model: Option<String>,

    /// Maximum tokens for LLM generation
    #[arg(long, default_value = "1024")]
    llm_max_tokens: u32,

    /// Temperature for LLM generation (0.0-1.0)
    #[arg(long, default_value = "0.8")]
    llm_temperature: f32,

    /// Number of parallel LLM requests (vLLM handles these efficiently)
    #[arg(long, default_value = "8")]
    llm_parallel: usize,

    /// Generate images for stories (requires --lore)
    #[arg(long)]
    images: bool,

    /// Image generation server URL
    #[arg(long, default_value = "http://192.168.8.59:8001")]
    image_url: String,

    /// Maximum number of images to generate
    #[arg(long, default_value = "10")]
    max_images: usize,

    /// Image width for generation
    #[arg(long, default_value = "1024")]
    image_width: u32,

    /// Image height for generation
    #[arg(long, default_value = "1024")]
    image_height: u32,

    /// Generate local map for specific tile (format: x,y)
    #[arg(long, value_name = "X,Y")]
    local_map: Option<String>,

    /// Size of local map in tiles (default: 64)
    #[arg(long, default_value = "64")]
    local_size: usize,

    /// Run civilization simulation
    #[arg(long)]
    simulate: bool,

    /// Number of simulation ticks (4 ticks = 1 year)
    #[arg(long, default_value = "100")]
    sim_ticks: u64,

    /// Number of tribes to spawn
    #[arg(long, default_value = "10")]
    sim_tribes: usize,

    /// Initial population per tribe
    #[arg(long, default_value = "100")]
    sim_population: u32,

    /// Separate seed for simulation (uses world seed if not specified)
    #[arg(long)]
    sim_seed: Option<u64>,
}

fn main() {
    let args = Args::parse();

    // If --view flag is set, launch interactive explorer
    if args.view {
        // Generate world quickly for explorer
        let world_data = world::generate_world(args.width, args.height, args.seed.unwrap_or_else(|| rand::random()));
        if let Err(e) = explorer::run_explorer(world_data) {
            eprintln!("Explorer error: {}", e);
        }
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
        lore_params.max_steps_per_wanderer = args.lore_steps;
        // Enable LLM prompts if using LLM or explicitly requested
        lore_params.include_llm_prompts = args.lore_llm_prompts || args.llm;

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

        // Export procedural narrative text
        let narrative_path = format!("{}_narrative.txt", args.lore_output);
        println!("Exporting procedural narrative to {}...", narrative_path);
        if let Err(e) = lore::export_narrative(&lore_result, &narrative_path) {
            eprintln!("Failed to export narrative: {}", e);
        }

        // Generate creation poem using LLM if requested
        if args.llm {
            println!("\nGenerating creation poem using LLM at {}...", args.llm_url);

            let llm_config = lore::LlmConfig {
                base_url: args.llm_url.clone(),
                model: args.llm_model.clone(),
                max_tokens: args.llm_max_tokens,
                temperature: args.llm_temperature,
                timeout_secs: 120,
                parallel_requests: args.llm_parallel,
            };

            // Check LLM server availability
            let llm_client = lore::LlmClient::new(llm_config.clone());
            if llm_client.health_check() {
                // Generate a single unified creation poem
                match lore::generate_creation_poem(&lore_result, &llm_config) {
                    Ok(poem) => {
                        // Print the poem to console
                        println!("\n═══════════════════════════════════════");
                        println!("         THE CREATION OF THE WORLD");
                        println!("═══════════════════════════════════════\n");
                        println!("{}", poem);
                        println!("\n═══════════════════════════════════════\n");

                        // Save to file
                        let poem_path = format!("{}_creation.txt", args.lore_output);
                        if let Err(e) = lore::export_creation_poem(&poem, &poem_path) {
                            eprintln!("Failed to save poem: {}", e);
                        } else {
                            println!("Saved creation poem to {}", poem_path);
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to generate creation poem: {}", e);
                    }
                }
            } else {
                eprintln!("LLM server at {} is not available.", args.llm_url);
            }
        }

        // Generate images if requested
        if args.images {
            println!("\nGenerating images using server at {}...", args.image_url);

            let image_config = lore::ImageGenConfig {
                base_url: args.image_url.clone(),
                timeout_secs: 300,
                width: args.image_width,
                height: args.image_height,
                output_dir: ".".to_string(),
            };

            let image_gen = lore::StoryImageGenerator::new(image_config);

            if image_gen.is_available() {
                // Generate landmark images
                println!("Generating landmark images...");
                let landmark_images = image_gen.generate_landmark_images(
                    &lore_result.landmarks,
                    args.max_images,
                    Some(&|current, total, msg| {
                        println!("  [{}/{}] {}", current + 1, total, msg);
                    }),
                );

                if !landmark_images.is_empty() {
                    println!("Generated {} landmark images:", landmark_images.len());
                    for (name, path) in &landmark_images {
                        println!("  - {}: {}", name, path);
                    }
                }

                // Generate story seed images
                println!("Generating story images...");
                let story_images = image_gen.generate_story_images(
                    &lore_result.story_seeds,
                    args.max_images,
                    Some(&|current, total, msg| {
                        println!("  [{}/{}] {}", current + 1, total, msg);
                    }),
                );

                if !story_images.is_empty() {
                    println!("Generated {} story images:", story_images.len());
                    for (name, path) in &story_images {
                        println!("  - {}: {}", name, path);
                    }
                }
            } else {
                eprintln!("Image generation server at {} is not available. Skipping image generation.", args.image_url);
            }
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

    // Run civilization simulation if requested
    if args.simulate {
        println!("\nRunning civilization simulation...");

        let sim_seed = args.sim_seed.unwrap_or(seed);
        let mut sim_rng = ChaCha8Rng::seed_from_u64(sim_seed);

        // Configure simulation parameters
        let mut sim_params = simulation::SimulationParams::default();
        sim_params.initial_tribe_count = args.sim_tribes;
        sim_params.initial_tribe_population = args.sim_population;

        // Create WorldData for simulation
        let map_scale = scale::MapScale::default();
        let world_data = world::WorldData::new(
            seed,
            map_scale,
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

        // Run simulation
        let sim_state = simulation::run_simulation(&world_data, &sim_params, args.sim_ticks, &mut sim_rng);

        // Print summary
        println!("\n{}", simulation::generate_summary(&sim_state));

        // Export simulation results
        let sim_json_path = format!("{}_simulation.json", args.output);
        println!("Exporting simulation results to {}...", sim_json_path);
        if let Err(e) = simulation::export_simulation(&sim_state, &sim_json_path) {
            eprintln!("Failed to export simulation: {}", e);
        } else {
            println!("Simulation export complete!");
        }
    }

    // Local map generation if requested
    if let Some(coords) = &args.local_map {
        // Parse "x,y" format
        let parts: Vec<&str> = coords.split(',').collect();
        if parts.len() != 2 {
            eprintln!("Invalid local-map format. Use: --local-map x,y (e.g., --local-map 256,128)");
            return;
        }

        let lx: usize = match parts[0].trim().parse() {
            Ok(v) => v,
            Err(_) => {
                eprintln!("Invalid x coordinate: {}", parts[0]);
                return;
            }
        };
        let ly: usize = match parts[1].trim().parse() {
            Ok(v) => v,
            Err(_) => {
                eprintln!("Invalid y coordinate: {}", parts[1]);
                return;
            }
        };

        if lx >= args.width || ly >= args.height {
            eprintln!("Coordinates ({}, {}) are outside map bounds ({}x{})",
                lx, ly, args.width, args.height);
            return;
        }

        println!("Generating local map for tile ({}, {})...", lx, ly);

        // Create WorldData for local map generation
        let map_scale = scale::MapScale::default();
        let world_data = world::WorldData::new(
            seed,
            map_scale,
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

        let local_map = local::generate_local_map(&world_data, lx, ly, args.local_size);
        let biome = world_data.biomes.get(lx, ly);

        println!("Generated {}x{} local map for {} biome",
            local_map.width, local_map.height, biome.display_name());

        // Export local map
        let local_path = format!("{}_local_{}_{}.png", args.output, lx, ly);
        local::export_local_map(&local_map, &local_path)
            .expect("Failed to export local map");
        println!("Exported local map to {}", local_path);

        // Export scaled version for better visibility
        let local_scaled_path = format!("{}_local_{}_{}_scaled.png", args.output, lx, ly);
        local::export_local_map_scaled(&local_map, &local_scaled_path, 8)
            .expect("Failed to export scaled local map");
        println!("Exported scaled local map to {}", local_scaled_path);

        return;
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

