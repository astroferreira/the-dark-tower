use clap::Parser;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

mod climate;
mod export;
mod heightmap;
mod plates;
mod tilemap;
mod viewer;

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
    let heightmap = heightmap::generate_heightmap(&plate_map, &plates, &stress_map, seed);
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
    let heightmap_normalized = heightmap::normalize_heightmap(&heightmap);

    // Generate climate
    println!("Generating climate...");
    let temperature = climate::generate_temperature(&heightmap, args.width, args.height);
    let moisture = climate::generate_moisture(&heightmap, args.width, args.height);
    let biomes = climate::generate_biomes(&heightmap, &temperature, &moisture);
    
    // Report climate stats
    let mut min_temp = f32::MAX;
    let mut max_temp = f32::MIN;
    for (_, _, &t) in temperature.iter() {
        if t < min_temp { min_temp = t; }
        if t > max_temp { max_temp = t; }
    }
    println!("Temperature range: {:.1}°C to {:.1}°C", min_temp, max_temp);

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
        &grid_path,
        seed,
    )
    .expect("Failed to export combined grid");

    println!("Done!");
}

