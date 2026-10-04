//! The world's terrain pipeline in one place: tectonics -> climate -> landscape evolution ->
//! erosion -> finishing passes -> drainage repair. Used by `main`, the `terrain_lab` binary and
//! the comparison-grid exporter, so they cannot drift apart.

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use crate::climate::{self, ClimateConfig, ClimateSimulation};
use crate::erosion::{self, landscape::LandscapeParams, ErosionPreset};
use crate::heightmap::{self, VolcanoLocation};
use crate::plates::{self, simulation::CrustFields, Plate, PlateId, WorldStyle};
use crate::scale::MapScale;
use crate::seeds::WorldSeeds;
use crate::tilemap::Tilemap;
use crate::coastline;

/// What to generate.
#[derive(Clone)]
pub struct TerrainConfig {
    pub width: usize,
    pub height: usize,
    pub style: WorldStyle,
    /// Number of tectonic plates (random for the style if `None`).
    pub plates: Option<usize>,
    /// Simulated tectonic history (Myr).
    pub tectonic_myr: f32,
    /// The old static-plate + noise terrain instead of the simulation (no landscape step,
    /// sea-level relevel or drainage repair).
    pub legacy_tectonics: bool,
    pub erosion_preset: ErosionPreset,
    pub climate: ClimateConfig,
    /// Run the legacy erosion at the map's own resolution instead of 4x.
    pub no_hires: bool,
    /// Run the legacy erosion pass (particles, river carving, glaciers, mostly at 4x) after the
    /// landscape evolution. Off by default: ~45 s of a 512x256 world for little visible change
    /// (seed 42: rivers slightly denser, every depression flat-filled so fewer lakes). The legacy
    /// tectonics path still uses it (it has no landscape step).
    pub legacy_erosion: bool,
    pub landscape: LandscapeParams,
    /// Break up small islands' coasts (`heightmap::apply_island_coasts`).
    pub island_coasts: bool,
}

impl TerrainConfig {
    pub fn new(width: usize, height: usize, style: WorldStyle) -> Self {
        Self {
            width,
            height,
            style,
            plates: None,
            tectonic_myr: 200.0,
            legacy_tectonics: false,
            erosion_preset: ErosionPreset::Normal,
            climate: ClimateConfig::default(),
            no_hires: false,
            legacy_erosion: false,
            landscape: LandscapeParams::default(),
            island_coasts: true,
        }
    }
}

/// The generated terrain and the fields later stages (water bodies, biomes, history) need.
pub struct Terrain {
    pub plate_map: Tilemap<PlateId>,
    pub plates: Vec<Plate>,
    pub stress_map: Tilemap<f32>,
    pub heightmap: Tilemap<f32>,
    /// The planet's ocean as a global equivalent layer (m); `None` on the legacy path.
    pub ocean_gel_m: Option<f32>,
    pub climate: ClimateSimulation,
    pub hardness: Tilemap<f32>,
    /// Flow accumulation from the erosion pass.
    pub flow_accumulation: Tilemap<f32>,
    pub volcanoes: Vec<VolcanoLocation>,
}

/// A look at the terrain between stages, for diagnostics (`terrain_lab`). Stage names, in
/// order: "tectonic", "climate", "landscape", "erosion", "coastline", "fjords", "noise",
/// "volcanoes", "islands", "beaches", "final".
pub struct Stage<'a> {
    pub name: &'static str,
    pub heightmap: &'a Tilemap<f32>,
    pub stress_map: &'a Tilemap<f32>,
    /// The simulated crust (tectonic stage of the simulated path only).
    pub crust: Option<&'a CrustFields>,
    /// The climate (from the "climate" stage on).
    pub climate: Option<&'a ClimateSimulation>,
    pub ocean_gel_m: Option<f32>,
}

/// Generate the terrain. `on_stage` sees the heightmap after every stage and returns `false`
/// to stop early (then the result is `None`).
pub fn generate_terrain(
    cfg: &TerrainConfig,
    seeds: &WorldSeeds,
    on_stage: &mut dyn FnMut(&Stage) -> bool,
) -> Option<Terrain> {
    let (width, height) = (cfg.width, cfg.height);
    let map_scale = MapScale::default();

    let (plate_map, plates, stress_map, mut heightmap, ocean_gel, crust) = if cfg.legacy_tectonics {
        // Legacy path: static Voronoi-like plates, boundary stress and noise-driven terrain.
        let mut tectonic_rng = ChaCha8Rng::seed_from_u64(seeds.tectonics);
        println!("Generating tectonic plates (legacy)...");
        let (plate_map, plates) = plates::generate_plates(width, height, cfg.plates, cfg.style, &mut tectonic_rng);
        let continental = plates.iter().filter(|p| p.plate_type == plates::PlateType::Continental).count();
        println!("Created {} plates ({} continental, {} oceanic)", plates.len(), continental, plates.len() - continental);
        let stress_map = plates::calculate_stress(&plate_map, &plates);
        let mut hm = heightmap::generate_heightmap(&plate_map, &plates, &stress_map, seeds.heightmap);
        heightmap::apply_inland_uplift(&mut hm, &stress_map, &map_scale);
        (plate_map, plates, stress_map, hm, None, None)
    } else {
        // Tectonic simulation: plates drift on a sphere, collide, subduct and rift; terrain
        // follows from crustal thickness (isostasy) and seafloor age.
        println!("Simulating plate tectonics ({} Myr)...", cfg.tectonic_myr);
        let params = plates::TectonicParams { total_myr: cfg.tectonic_myr, ..Default::default() };
        let t = plates::generate_tectonic_terrain(width, height, cfg.plates, cfg.style, seeds, &params);
        let continental = t.plates.iter().filter(|p| p.plate_type == plates::PlateType::Continental).count();
        println!("Tectonic history complete: {} plates survive ({} continental, {} oceanic)", t.plates.len(), continental, t.plates.len() - continental);
        println!("Ocean: {:.0} m of water as a global layer (Earth ~2640 m)", t.ocean_gel_m);
        (t.plate_map, t.plates, t.stress_map, t.heightmap, Some(t.ocean_gel_m), Some(t.crust))
    };
    let (min_h, max_h) = range(&heightmap);
    let above = heightmap.iter().filter(|(_, _, &h)| h > 0.0).count();
    println!("Heightmap range: {:.1}m to {:.1}m ({:.1}% above sea level)", min_h, max_h, 100.0 * above as f64 / (width * height) as f64);
    if !on_stage(&stage("tectonic", &heightmap, &stress_map, None, crust.as_ref(), ocean_gel)) { return None; }

    // Physical climate: energy balance, 3-cell circulation, winds, ocean currents, moisture.
    println!("Simulating physical climate (mode: {}, rainfall: {})...", cfg.climate.mode, cfg.climate.rainfall);
    let climate_sim = climate::run_climate_simulation(&heightmap, &cfg.climate, seeds.climate);
    let (t0, t1) = range(&climate_sim.mean_temperature);
    let (p0, p1) = range(&climate_sim.annual_precipitation);
    println!("Temperature range: {:.1}°C to {:.1}°C", t0, t1);
    println!("Precipitation range: {:.0}mm to {:.0}mm/yr", p0, p1);
    if !on_stage(&stage("climate", &heightmap, &stress_map, Some(&climate_sim), None, ocean_gel)) { return None; }

    // Landscape evolution: uplift where plates converge against river incision driven by the
    // climate's precipitation, with flexural rebound and sediment filling lakes and shelves.
    if !cfg.legacy_tectonics {
        println!("Evolving the landscape (uplift, rivers, sediment, isostasy)...");
        let lp = &cfg.landscape;
        let r = erosion::landscape::evolve(&mut heightmap, &climate_sim.annual_precipitation, &stress_map, lp);
        println!("  {:.1} Myr: eroded {:.2}M km3 (lakes {:.2}M, shelves {:.2}M, deep sea {:.2}M), uplifted {:.2}M km3; {} land cells hold lakes",
            lp.duration_yr / 1e6, r.eroded_km3 / 1e6, r.into_lakes_km3 / 1e6, r.onto_shelves_km3 / 1e6,
            r.to_deep_sea_km3 / 1e6, r.uplifted_km3 / 1e6, r.lake_cells);
    }
    if let Some(gel) = ocean_gel {
        // Sediment on the shelves displaced sea water: the same ocean now stands higher.
        let rise = plates::crust::relevel_to_volume(&mut heightmap, gel);
        println!("  Sea level {:+.1} m (same ocean volume)", rise);
    }
    if !on_stage(&stage("landscape", &heightmap, &stress_map, Some(&climate_sim), None, ocean_gel)) { return None; }

    // Legacy erosion (particles, river carving, glaciers), mostly at 4x resolution.
    let (hardness, flow_accumulation) = if !cfg.legacy_erosion {
        println!("Legacy erosion skipped");
        let (_, acc, _) = erosion::rivers::compute_flow_with_filled_routing(&heightmap);
        (Tilemap::new_with(width, height, 0.3f32), acc)
    } else {
    println!("Simulating erosion (preset: {})...", cfg.erosion_preset);
    let mut erosion_params = erosion::ErosionParams::from_preset(cfg.erosion_preset);
    erosion_params.tune_for_heightmap(&heightmap);
    if cfg.no_hires {
        erosion_params.simulation_scale = 1;
        println!("  High-resolution erosion disabled (--no-hires)");
    }
    let mut erosion_rng = ChaCha8Rng::seed_from_u64(seeds.erosion);
    let (stats, hardness, flow_accumulation) = erosion::simulate_erosion(
        &mut heightmap, &plate_map, &plates, &stress_map, &climate_sim.mean_temperature,
        &erosion_params, &mut erosion_rng, seeds.erosion,
    );
    println!("Erosion complete:");
    println!("  Total eroded: {:.1} units", stats.total_eroded);
    println!("  Total deposited: {:.1} units", stats.total_deposited);
    println!("  Max erosion: {:.2} units", stats.max_erosion);
    println!("  Max deposition: {:.2} units", stats.max_deposition);
    let (min_h, max_h) = range(&heightmap);
    println!("Post-erosion heightmap range: {:.1}m to {:.1}m", min_h, max_h);
    (hardness, flow_accumulation)
    };
    if !on_stage(&stage("erosion", &heightmap, &stress_map, Some(&climate_sim), None, ocean_gel)) { return None; }

    // Finishing passes.
    println!("Applying coastline jittering...");
    let coastline_params = coastline::CoastlineParams::default();
    let network = coastline::generate_coastline_network(&heightmap, &coastline_params, seeds.coastline);
    coastline::apply_coastline_to_heightmap(&network, &mut heightmap, coastline_params.blend_width);
    if !on_stage(&stage("coastline", &heightmap, &stress_map, Some(&climate_sim), None, ocean_gel)) { return None; }

    // Fjord channels: narrow inlets like Norwegian fjords.
    println!("Carving fjord channels...");
    heightmap::apply_fjord_incisions(&mut heightmap, seeds.heightmap, &map_scale);
    if !on_stage(&stage("fjords", &heightmap, &stress_map, Some(&climate_sim), None, ocean_gel)) { return None; }

    println!("Applying terrain noise layers...");
    heightmap::apply_regional_noise_stacks(&mut heightmap, &stress_map, seeds.heightmap);
    if !on_stage(&stage("noise", &heightmap, &stress_map, Some(&climate_sim), None, ocean_gel)) { return None; }

    println!("Placing volcanoes...");
    let volcanoes = heightmap::apply_volcano_pass(&mut heightmap, &stress_map, seeds.heightmap);
    if !on_stage(&stage("volcanoes", &heightmap, &stress_map, Some(&climate_sim), None, ocean_gel)) { return None; }

    if cfg.island_coasts {
        println!("Shaping island coasts...");
        heightmap::apply_island_coasts(&mut heightmap, seeds.heightmap);
    }
    if !on_stage(&stage("islands", &heightmap, &stress_map, Some(&climate_sim), None, ocean_gel)) { return None; }

    // Coastal beach strips near sea level (except high-stress cliffs).
    println!("Applying coastal beach pass...");
    heightmap::apply_coastal_beaches(&mut heightmap, &stress_map, &map_scale, erosion::landscape::tile_km(width));
    if !on_stage(&stage("beaches", &heightmap, &stress_map, Some(&climate_sim), None, ocean_gel)) { return None; }

    // The detail passes above leave pits that would end rivers; fill them so the land drains
    // to the sea again (larger hollows stay as lakes).
    if !cfg.legacy_tectonics {
        let filled = erosion::landscape::fill_pits(&mut heightmap, 4, 10.0);
        println!("Restored drainage: filled {} pit cells", filled);
    }
    if !on_stage(&stage("final", &heightmap, &stress_map, Some(&climate_sim), None, ocean_gel)) { return None; }

    Some(Terrain {
        plate_map,
        plates,
        stress_map,
        heightmap,
        ocean_gel_m: ocean_gel,
        climate: climate_sim,
        hardness,
        flow_accumulation,
        volcanoes,
    })
}

fn stage<'a>(
    name: &'static str,
    heightmap: &'a Tilemap<f32>,
    stress_map: &'a Tilemap<f32>,
    climate: Option<&'a ClimateSimulation>,
    crust: Option<&'a CrustFields>,
    ocean_gel_m: Option<f32>,
) -> Stage<'a> {
    Stage { name, heightmap, stress_map, crust, climate, ocean_gel_m }
}

fn range(map: &Tilemap<f32>) -> (f32, f32) {
    map.iter().fold((f32::MAX, f32::MIN), |(a, b), (_, _, &v)| (a.min(v), b.max(v)))
}
