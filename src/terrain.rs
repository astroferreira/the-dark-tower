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
    /// From the star system: the water's scale on the style's land fraction and the mantle's
    /// hotspots (1.0 and 8: the Earth's).
    pub land_scale: f32,
    pub hotspots: usize,
    /// The violent land (`crate::upheaval`): the largest craters and the flood basalts shaped
    /// before the climate; none (the default) leaves the terrain as it was.
    pub upheaval: Option<crate::upheaval::Upheaval>,
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
            land_scale: 1.0,
            hotspots: 8,
            upheaval: None,
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
/// order: "tectonic", "volcanoes", "noise", "islands", "coastline", "climate", "landscape",
/// "erosion", "fjords", "beaches", "final" (terrain revision 1, `crate::terrain_rev`:
/// "tectonic", "climate", "landscape", "erosion", "coastline", "fjords", "noise", "volcanoes",
/// "islands", "beaches", "final").
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
///
/// The order is causal (`audit.md` 1.1): what builds the land (tectonics, volcanoes, regional
/// relief, island and coast shapes) comes first, then the climate on that land, then the rivers
/// and hillslopes that wear it down, so they carve gullies down the volcanoes and drain the
/// relief; only the fjords (carved by ice into the finished valleys) and the beaches come after,
/// and a last pit fill and flow routing leave the drainage the same as the map. The rock's
/// hardness comes from the crust (1.2) and steers the rivers; the climate's temperatures follow
/// the final heights by the lapse rate (1.3). Terrain revision 1 (`crate::terrain_rev`) is the
/// old order, bit for bit.
pub fn generate_terrain(
    cfg: &TerrainConfig,
    seeds: &WorldSeeds,
    on_stage: &mut dyn FnMut(&Stage) -> bool,
) -> Option<Terrain> {
    if crate::terrain_rev::v1() { return generate_terrain_v1(cfg, seeds, on_stage); }
    let (width, height) = (cfg.width, cfg.height);
    let map_scale = MapScale::default();
    let (plate_map, plates, stress_map, mut heightmap, ocean_gel, crust) = tectonics(cfg, seeds, &map_scale);
    let (min_h, max_h) = range(&heightmap);
    let above = heightmap.iter().filter(|(_, _, &h)| h > 0.0).count();
    println!("Heightmap range: {:.1}m to {:.1}m ({:.1}% above sea level)", min_h, max_h, 100.0 * above as f64 / (width * height) as f64);
    if !on_stage(&stage("tectonic", &heightmap, &stress_map, None, crust.as_ref(), ocean_gel)) { return None; }

    // What builds the land, before the rivers wear it.
    println!("Placing volcanoes...");
    // An upheaval world draws the volcanoes' kinds over the whole range (stratovolcanoes too).
    let volcanoes = heightmap::apply_volcano_pass_with(&mut heightmap, &stress_map, seeds.heightmap, cfg.upheaval.is_some());
    if !on_stage(&stage("volcanoes", &heightmap, &stress_map, None, None, ocean_gel)) { return None; }
    println!("Applying terrain noise layers...");
    heightmap::apply_regional_noise_stacks(&mut heightmap, &stress_map, seeds.heightmap);
    if !on_stage(&stage("noise", &heightmap, &stress_map, None, None, ocean_gel)) { return None; }
    if cfg.island_coasts {
        println!("Shaping island coasts...");
        heightmap::apply_island_coasts(&mut heightmap, seeds.heightmap);
    }
    if !on_stage(&stage("islands", &heightmap, &stress_map, None, None, ocean_gel)) { return None; }
    println!("Applying coastline jittering...");
    let coastline_params = coastline::CoastlineParams::default();
    let network = coastline::generate_coastline_network(&heightmap, &coastline_params, seeds.coastline);
    coastline::apply_coastline_to_heightmap(&network, &mut heightmap, coastline_params.blend_width);
    if !on_stage(&stage("coastline", &heightmap, &stress_map, None, None, ocean_gel)) { return None; }
    // The violent land: flood basalts over plumes and the largest craters, before the rivers
    // wear them (`crate::upheaval`; nothing without upheaval).
    if let Some(up) = &cfg.upheaval {
        let lips = crate::upheaval::provinces(width, height, seeds.master, up);
        let raised = crate::upheaval::apply_world_provinces(&mut heightmap, &lips);
        let impacts = crate::upheaval::Impacts::generate(width, height, seeds.master, up);
        let shaped = crate::upheaval::apply_world_impacts(&mut heightmap, &impacts);
        println!("Upheaval (impact flux {:.2}x the Earth's): {} craters on record, {} shaped on the map; {} flood basalts over {} tiles",
            up.impact_flux, impacts.craters.len(), shaped, lips.len(), raised);
        if !on_stage(&stage("upheaval", &heightmap, &stress_map, None, None, ocean_gel)) { return None; }
    }

    // Physical climate on the built land.
    println!("Simulating physical climate (mode: {}, rainfall: {})...", cfg.climate.mode, cfg.climate.rainfall);
    let mut climate_sim = climate::run_climate_simulation(&heightmap, &cfg.climate, seeds.climate);
    let climate_heights = heightmap.clone();
    let (t0, t1) = range(&climate_sim.mean_temperature);
    let (p0, p1) = range(&climate_sim.annual_precipitation);
    println!("Temperature range: {:.1}°C to {:.1}°C", t0, t1);
    println!("Precipitation range: {:.0}mm to {:.0}mm/yr", p0, p1);
    if !on_stage(&stage("climate", &heightmap, &stress_map, Some(&climate_sim), None, ocean_gel)) { return None; }

    // The rock: cratons and intrusions hard, fold belts middling, basins soft (`audit.md` 1.2).
    let mut hardness = bedrock_hardness(crust.as_ref(), &heightmap, &stress_map, seeds.heightmap);
    if !cfg.legacy_tectonics {
        println!("Evolving the landscape (uplift, rivers, sediment, isostasy)...");
        let lp = &cfg.landscape;
        let r = erosion::landscape::evolve_with_hardness(&mut heightmap, &climate_sim.annual_precipitation, &stress_map, Some(&hardness), lp);
        println!("  {:.1} Myr: eroded {:.2}M km3 (lakes {:.2}M, shelves {:.2}M, deep sea {:.2}M), uplifted {:.2}M km3; {} land cells hold lakes",
            lp.duration_yr / 1e6, r.eroded_km3 / 1e6, r.into_lakes_km3 / 1e6, r.onto_shelves_km3 / 1e6,
            r.to_deep_sea_km3 / 1e6, r.uplifted_km3 / 1e6, r.lake_cells);
    }
    // The planet's water is a free choice: as much as leaves the style's share of land dry
    // once the rivers have worked (the relief built before them now sheds two to three times
    // the sediment onto the shelves, and keeping the birth volume drowned 4-5% of the planet's
    // land; revision 1 kept the volume).
    let ocean_gel = ocean_gel.map(|_| {
        let target = plates::crust::land_fraction_for(cfg.style, cfg.land_scale);
        let s = plates::crust::sea_level_for_land_fraction(&heightmap, target);
        for (_, _, e) in heightmap.iter_mut() { *e -= s; }
        let gel = plates::crust::ocean_volume_gel(&heightmap, 0.0);
        println!("  Sea level {:+.1} m ({:.0}% land; ocean {:.0} m as a global layer)", s, target * 100.0, gel);
        gel
    });
    lay_alluvium(&mut hardness, &climate_heights, &heightmap, &stress_map);
    if !on_stage(&stage("landscape", &heightmap, &stress_map, Some(&climate_sim), None, ocean_gel)) { return None; }

    if cfg.legacy_erosion {
        let (h, _) = legacy_erosion(cfg, seeds, &mut heightmap, &plate_map, &plates, &stress_map, &climate_sim);
        hardness = h;
    } else {
        println!("Legacy erosion skipped");
    }
    if !on_stage(&stage("erosion", &heightmap, &stress_map, Some(&climate_sim), None, ocean_gel)) { return None; }
    // Basins too young for the rivers to have filled: ice-scoured shields and rift grabens.
    if !cfg.legacy_tectonics {
        let (ice, rift) = heightmap::carve_young_basins(&mut heightmap, &climate_sim.mean_temperature, &stress_map, &hardness, seeds.heightmap);
        println!("Young basins: {} cells scoured by ice, {} sunk along rifts", ice, rift);
    }
    // The lakes the rivers left: the last pit fill keeps them and fills what comes after.
    let lakes = erosion::landscape::lake_depth(&heightmap);

    // Ice carved the fjords into the finished valleys; the sea built the beaches.
    println!("Carving fjord channels...");
    heightmap::apply_fjord_incisions(&mut heightmap, seeds.heightmap, &map_scale);
    if !on_stage(&stage("fjords", &heightmap, &stress_map, Some(&climate_sim), None, ocean_gel)) { return None; }
    println!("Applying coastal beach pass...");
    heightmap::apply_coastal_beaches(&mut heightmap, &stress_map, &map_scale, erosion::landscape::tile_km(width));
    if !on_stage(&stage("beaches", &heightmap, &stress_map, Some(&climate_sim), None, ocean_gel)) { return None; }

    // The last passes' pits are filled whatever their size; the rivers' own lakes stay.
    if !cfg.legacy_tectonics {
        // A lake must be as big on the ground as 4 cells at 512 wide (~24,000 km2), at least a
        // cell (revision 1 asked 4 cells at any size, ~100,000 km2 at 256 wide).
        let min_cells = ((4.0 * (width as f32 / 512.0).powi(2)).round() as usize).max(1);
        let filled = erosion::landscape::fill_pits_keeping(&mut heightmap, min_cells, 10.0, Some(&lakes));
        println!("Restored drainage: filled {} pit cells", filled);
    }
    let (_, flow_accumulation, _) = erosion::rivers::compute_flow_with_filled_routing(&heightmap);
    let cooled = follow_heights(&mut climate_sim, &climate_heights, &heightmap);
    if cooled > 0 { println!("Climate follows the final heights: {} cells re-tempered by the lapse rate", cooled); }
    if !on_stage(&stage("final", &heightmap, &stress_map, Some(&climate_sim), None, ocean_gel)) { return None; }

    Some(Terrain { plate_map, plates, stress_map, heightmap, ocean_gel_m: ocean_gel, climate: climate_sim, hardness, flow_accumulation, volcanoes })
}

/// The plates and the land they build: the simulated crust, or the legacy static plates.
#[allow(clippy::type_complexity)]
fn tectonics(cfg: &TerrainConfig, seeds: &WorldSeeds, map_scale: &MapScale)
    -> (Tilemap<PlateId>, Vec<Plate>, Tilemap<f32>, Tilemap<f32>, Option<f32>, Option<CrustFields>) {
    let (width, height) = (cfg.width, cfg.height);
    if cfg.legacy_tectonics {
        let mut tectonic_rng = ChaCha8Rng::seed_from_u64(seeds.tectonics);
        println!("Generating tectonic plates (legacy)...");
        let (plate_map, plates) = plates::generate_plates(width, height, cfg.plates, cfg.style, &mut tectonic_rng);
        let continental = plates.iter().filter(|p| p.plate_type == plates::PlateType::Continental).count();
        println!("Created {} plates ({} continental, {} oceanic)", plates.len(), continental, plates.len() - continental);
        let stress_map = plates::calculate_stress(&plate_map, &plates);
        let mut hm = heightmap::generate_heightmap(&plate_map, &plates, &stress_map, seeds.heightmap);
        heightmap::apply_inland_uplift(&mut hm, &stress_map, map_scale);
        (plate_map, plates, stress_map, hm, None, None)
    } else {
        println!("Simulating plate tectonics ({} Myr)...", cfg.tectonic_myr);
        let params = plates::TectonicParams { total_myr: cfg.tectonic_myr, hotspots: cfg.hotspots, land_scale: cfg.land_scale, ..Default::default() };
        let t = plates::generate_tectonic_terrain(width, height, cfg.plates, cfg.style, seeds, &params);
        let continental = t.plates.iter().filter(|p| p.plate_type == plates::PlateType::Continental).count();
        println!("Tectonic history complete: {} plates survive ({} continental, {} oceanic)", t.plates.len(), continental, t.plates.len() - continental);
        println!("Ocean: {:.0} m of water as a global layer (Earth ~2640 m)", t.ocean_gel_m);
        (t.plate_map, t.plates, t.stress_map, t.heightmap, Some(t.ocean_gel_m), Some(t.crust))
    }
}

/// The legacy erosion (particles, river carving, glaciers), mostly at 4x: its hardness and flow.
fn legacy_erosion(cfg: &TerrainConfig, seeds: &WorldSeeds, heightmap: &mut Tilemap<f32>, plate_map: &Tilemap<PlateId>,
    plates: &[Plate], stress_map: &Tilemap<f32>, climate_sim: &ClimateSimulation) -> (Tilemap<f32>, Tilemap<f32>) {
    println!("Simulating erosion (preset: {})...", cfg.erosion_preset);
    let mut erosion_params = erosion::ErosionParams::from_preset(cfg.erosion_preset);
    erosion_params.tune_for_heightmap(heightmap);
    if cfg.no_hires {
        erosion_params.simulation_scale = 1;
        println!("  High-resolution erosion disabled (--no-hires)");
    }
    let mut erosion_rng = ChaCha8Rng::seed_from_u64(seeds.erosion);
    let (stats, hardness, flow_accumulation) = erosion::simulate_erosion(
        heightmap, plate_map, plates, stress_map, &climate_sim.mean_temperature,
        &erosion_params, &mut erosion_rng, seeds.erosion,
    );
    println!("Erosion complete:");
    println!("  Total eroded: {:.1} units", stats.total_eroded);
    println!("  Total deposited: {:.1} units", stats.total_deposited);
    println!("  Max erosion: {:.2} units", stats.max_erosion);
    println!("  Max deposition: {:.2} units", stats.max_deposition);
    let (min_h, max_h) = range(heightmap);
    println!("Post-erosion heightmap range: {:.1}m to {:.1}m", min_h, max_h);
    (hardness, flow_accumulation)
}

/// Rock hardness 0..1 from the crust (`audit.md` 1.2): old stable continental crust (cratons,
/// shields of granite and gneiss) 0.75-0.9; volcanic edifices and the young crust of accreted
/// arcs (basalt, andesite, intrusions) ~0.85; fold belts, the deformed sedimentary and
/// metamorphic rock of active and standing orogens, 0.5-0.7; low-lying continental basins and
/// thinned (rifted) crust, filled with sediment, 0.15-0.35; the ocean floor basalt 0.7. A little
/// noise varies the beds so rivers find soft bands between hard ones. Without the crust (the
/// legacy tectonics) thickness is read from elevation.
pub fn bedrock_hardness(crust: Option<&CrustFields>, heightmap: &Tilemap<f32>, stress: &Tilemap<f32>, seed: u64) -> Tilemap<f32> {
    use noise::{NoiseFn, Perlin, Seedable};
    let (w, h) = (heightmap.width, heightmap.height);
    let beds = Perlin::new(1).set_seed(seed.wrapping_add(0x4a7d) as u32);
    let ss = |e0: f32, e1: f32, x: f32| { let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0); t * t * (3.0 - 2.0 * t) };
    let mut out = Tilemap::new_with(w, h, 0.5f32);
    for y in 0..h {
        for x in 0..w {
            let e = *heightmap.get(x, y);
            // Compression folds the rock; tension (a rift) lets a graben fill with sediment.
            let raw_s = *stress.get(x, y);
            let s = raw_s.max(0.0);
            let (thick, age, volc) = match crust {
                Some(c) => (*c.thickness_km.get(x, y), *c.age_myr.get(x, y), *c.volcanic_m.get(x, y)),
                None => (if e > -1500.0 { 35.0 } else { 7.0 }, 180.0, 0.0),
            };
            let v = if thick < 20.0 {
                // Ocean crust: basalt and gabbro, harder still where it stands as a volcanic island.
                0.7 + 0.15 * ss(-200.0, 200.0, e)
            } else {
                let fold = ss(0.08, 0.3, s).max(ss(38.0, 48.0, thick));
                let basin = ss(350.0, 60.0, e).max(ss(33.0, 28.0, thick)).max(ss(0.03, 0.15, -raw_s)) * (1.0 - fold);
                let igneous = ss(30.0, 300.0, volc).max(ss(170.0, 110.0, age));
                let mut v = 0.82;
                v += (0.6 - v) * fold;
                v += (0.25 - v) * basin;
                v += (0.85 - v) * igneous;
                v
            };
            let p = crate::tilemap::sphere_point(x, y, w, h, w as f64 / 40.0);
            let bed = beds.get(p) as f32;
            out.set(x, y, (v + 0.08 * bed).clamp(0.1, 0.95));
        }
    }
    out
}

/// Where the rivers laid sediment (land raised by more than 30 m outside the uplifting belts),
/// the rock is alluvium: hardness 0.15-0.2.
fn lay_alluvium(hardness: &mut Tilemap<f32>, before: &Tilemap<f32>, after: &Tilemap<f32>, stress: &Tilemap<f32>) {
    for y in 0..hardness.height {
        for x in 0..hardness.width {
            let (b, a) = (*before.get(x, y), *after.get(x, y));
            if a <= 0.0 || *stress.get(x, y) > 0.12 { continue; }
            let t = ((a - b - 30.0) / 120.0).clamp(0.0, 1.0);
            if t > 0.0 {
                let v = *hardness.get(x, y);
                hardness.set(x, y, v + (0.18 - v) * t);
            }
        }
    }
}

/// Temperatures follow the final heights (`audit.md` 1.3): each cell's mean and seasonal
/// temperatures move by the lapse rate times the change in its land height since the climate
/// ran (the sea surface counts as height 0). Returns the cells changed by more than 0.1 C.
pub fn follow_heights(sim: &mut ClimateSimulation, before: &Tilemap<f32>, after: &Tilemap<f32>) -> usize {
    let lapse = sim.forcing.lapse_rate / 1000.0;
    let mut changed = 0;
    for y in 0..after.height {
        for x in 0..after.width {
            let dt = -lapse * (after.get(x, y).max(0.0) - before.get(x, y).max(0.0));
            if dt == 0.0 { continue; }
            if dt.abs() > 0.1 { changed += 1; }
            let m = *sim.mean_temperature.get(x, y);
            sim.mean_temperature.set(x, y, m + dt);
            for season in sim.seasonal_temperatures.iter_mut() {
                let t = *season.get(x, y);
                season.set(x, y, t + dt);
            }
        }
    }
    changed
}

/// Terrain revision 1: the climate and the rivers first, the finishing passes after.
fn generate_terrain_v1(
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
        let params = plates::TectonicParams { total_myr: cfg.tectonic_myr, hotspots: cfg.hotspots, land_scale: cfg.land_scale, ..Default::default() };
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
