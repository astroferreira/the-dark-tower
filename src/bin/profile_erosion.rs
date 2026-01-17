//! Profiling tool to identify performance bottlenecks

use std::time::Instant;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use planet_generator::{
    climate, erosion, heightmap, plates,
    erosion::ErosionParams,
};

fn main() {
    let width = 512;
    let height = 256;
    let seed = 1337u64;

    println!("=== Performance Profiling ===");
    println!("Map size: {}x{} ({} cells)", width, height, width * height);
    println!();

    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    // Profile plate generation
    let start = Instant::now();
    let (plate_map, plates) = plates::generate_plates(width, height, None, &mut rng);
    let plate_time = start.elapsed();
    println!("Plate generation: {:?}", plate_time);

    // Profile stress calculation
    let start = Instant::now();
    let stress_map = plates::calculate_stress(&plate_map, &plates);
    let stress_time = start.elapsed();
    println!("Stress calculation: {:?}", stress_time);

    // Profile heightmap generation
    let start = Instant::now();
    let mut heightmap = heightmap::generate_heightmap(&plate_map, &plates, &stress_map, seed);
    let heightmap_time = start.elapsed();
    println!("Heightmap generation: {:?}", heightmap_time);

    // Profile climate generation
    let start = Instant::now();
    let temperature = climate::generate_temperature(&heightmap, width, height);
    let _moisture = climate::generate_moisture(&heightmap, width, height);
    let climate_time = start.elapsed();
    println!("Climate generation: {:?}", climate_time);

    // Profile erosion (the big one)
    let params = ErosionParams::default();
    println!("\nErosion parameters:");
    println!("  Hydraulic iterations: {}", params.hydraulic_iterations);
    println!("  River erosion: {}", params.enable_rivers);
    println!("  Hydraulic erosion: {}", params.enable_hydraulic);
    println!();

    let start = Instant::now();
    let (stats, _hardness) = erosion::simulate_erosion(
        &mut heightmap,
        &plate_map,
        &plates,
        &stress_map,
        &temperature,
        &params,
        &mut rng,
        seed,
    );
    let erosion_time = start.elapsed();
    println!("Total erosion simulation: {:?}", erosion_time);
    println!("  Eroded: {:.0} units", stats.total_eroded);
    println!("  Deposited: {:.0} units", stats.total_deposited);

    // Summary
    let total = plate_time + stress_time + heightmap_time + climate_time + erosion_time;
    println!("\n=== Summary ===");
    println!("Plate generation: {:>8.2}% ({:?})", 100.0 * plate_time.as_secs_f64() / total.as_secs_f64(), plate_time);
    println!("Stress calc:      {:>8.2}% ({:?})", 100.0 * stress_time.as_secs_f64() / total.as_secs_f64(), stress_time);
    println!("Heightmap:        {:>8.2}% ({:?})", 100.0 * heightmap_time.as_secs_f64() / total.as_secs_f64(), heightmap_time);
    println!("Climate:          {:>8.2}% ({:?})", 100.0 * climate_time.as_secs_f64() / total.as_secs_f64(), climate_time);
    println!("Erosion:          {:>8.2}% ({:?})", 100.0 * erosion_time.as_secs_f64() / total.as_secs_f64(), erosion_time);
    println!("─────────────────────────────────");
    println!("TOTAL:            {:>8}  {:?}", "100%", total);
}
