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
mod plates;
mod scale;
mod seeds;
mod tilemap;
mod water_bodies;
mod world;

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

    /// Number of tectonic plates (random 6-15 if not specified)
    #[arg(short = 'p', long)]
    plates: Option<usize>,

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
}

fn main() {
    let args = Args::parse();

    // Build seeds from master seed with optional overrides
    let master_seed = args.seed.unwrap_or_else(|| rand::random());
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
    println!("Map size: {}x{}", args.width, args.height);

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
    let (plate_map, plates) = plates::generate_plates(args.width, args.height, args.plates, &mut tectonic_rng);
    let continental_count = plates.iter().filter(|p| p.plate_type == plates::PlateType::Continental).count();
    let oceanic_count = plates.iter().filter(|p| p.plate_type == plates::PlateType::Oceanic).count();
    println!("Created {} plates ({} continental, {} oceanic)", plates.len(), continental_count, oceanic_count);

    // Calculate stress at plate boundaries
    println!("Calculating plate stress...");
    let stress_map = plates::calculate_stress(&plate_map, &plates);

    // Generate heightmap
    println!("Generating heightmap...");
    let land_mask = heightmap::generate_land_mask(&plate_map, &plates, seeds.heightmap);
    let land_count = (0..args.height).flat_map(|y| (0..args.width).map(move |x| (x, y)))
        .filter(|&(x, y)| *land_mask.get(x, y)).count();
    println!("Land mask: {} cells are land ({:.1}%)", land_count, 100.0 * land_count as f64 / (args.width * args.height) as f64);
    let mut heightmap = heightmap::generate_heightmap(&plate_map, &plates, &stress_map, seeds.heightmap);
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

    // Generate climate
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

    // Apply erosion
    println!("Simulating erosion...");
    let erosion_params = erosion::ErosionParams::default();
    let mut erosion_rng = ChaCha8Rng::seed_from_u64(seeds.erosion);

    let (stats, hardness_map) = erosion::simulate_erosion(
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

    // Apply terrain noise layers based on region type
    println!("Applying terrain noise layers...");
    heightmap::apply_regional_noise_stacks(&mut heightmap, &stress_map, seeds.heightmap);

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
    let map_scale = scale::MapScale::default();
    // Generate Bezier river network
    let river_network = crate::erosion::trace_bezier_rivers(&heightmap, None, seeds.rivers);

    let world_data = world::WorldData::new(
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
        Some(river_network),
        Some(biome_feather_map),
    );

    if let Err(e) = explorer::run_explorer(world_data) {
        eprintln!("Explorer error: {}", e);
    }
}
