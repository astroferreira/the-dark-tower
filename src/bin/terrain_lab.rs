//! Runs the world's terrain pipeline (tectonics -> climate -> erosion -> finishing passes)
//! without history or a window and prints drainage and relief metrics after each stage.
//!
//! Usage: terrain_lab [seed] [width] [height] [style] [out_dir]
//! Writes `<out>/terrain_<stage>.png` (hypsometric tint, closed basins in red).

use image::{Rgb, RgbImage};
use planet_generator::erosion::{self, rivers};
use planet_generator::plates::{self, WorldStyle};
use planet_generator::seeds::WorldSeeds;
use planet_generator::tilemap::Tilemap;
use planet_generator::{climate, coastline, heightmap, scale, water_bodies};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

fn lerp(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn tint(e: f32) -> [f32; 3] {
    if e <= 0.0 {
        let t = (-e / 6000.0).clamp(0.0, 1.0);
        lerp([90.0, 160.0, 210.0], [10.0, 25.0, 80.0], t.sqrt())
    } else if e < 500.0 {
        lerp([60.0, 140.0, 70.0], [160.0, 170.0, 90.0], e / 500.0)
    } else if e < 2500.0 {
        lerp([160.0, 170.0, 90.0], [130.0, 90.0, 55.0], (e - 500.0) / 2000.0)
    } else {
        lerp([130.0, 90.0, 55.0], [250.0, 250.0, 250.0], ((e - 2500.0) / 3000.0).clamp(0.0, 1.0))
    }
}

/// Prints land %, elevation quantiles, hypsometric integral, closed-basin share and the
/// share of big-river cells (filled-routing accumulation) whose raw descent reaches the sea.
fn report(stage: &str, hm: &Tilemap<f32>, out: &str, precip: Option<&Tilemap<f32>>) {
    let (w, h) = (hm.width, hm.height);
    let mut land: Vec<f32> = hm.iter().map(|(_, _, &e)| e).filter(|&e| e > 0.0).collect();
    land.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let q = |p: f32| land.get(((land.len().max(1) - 1) as f32 * p) as usize).copied().unwrap_or(0.0);
    let max = q(1.0);
    let hi = if max > 0.0 { land.iter().sum::<f32>() / land.len() as f32 / max } else { 0.0 };

    let filled = rivers::fill_depressions_public(hm);
    let ld = erosion::landscape::lake_depth(hm);
    let lem_trapped = ld.iter().filter(|(x, y, &d)| d > 1.0 && *hm.get(*x, *y) > 0.0).count();
    let lem_wet = ld.iter().filter(|(_, _, &d)| d > 1.0).count();
    println!("    lem view: land cells in basins {lem_trapped}, all lake cells {lem_wet}");
    let (_, acc, _) = rivers::compute_flow_with_filled_routing(hm);
    let mut trapped = 0usize;
    let (mut big, mut big_trapped) = (0usize, 0usize);
    let big_t = 50.0 * (w as f32 / 512.0).powi(2);
    let mut img = RgbImage::new(w as u32, h as u32);
    for (x, y, &e) in hm.iter() {
        let depth = *filled.get(x, y) - e;
        let mut c = tint(e);
        if e > 0.0 {
            let ex = *hm.get((x + 1) % w, y) - *hm.get((x + w - 1) % w, y);
            let ey = *hm.get(x, (y + 1).min(h - 1)) - *hm.get(x, y.saturating_sub(1));
            let shade = (1.0 + (-ex - ey) / 1500.0).clamp(0.45, 1.5);
            c = [c[0] * shade, c[1] * shade, c[2] * shade].map(|v| v.min(255.0));
        }
        if e > 0.0 {
            let t = depth > 1.0;
            if t { trapped += 1; c = lerp(c, [230.0, 30.0, 30.0], 0.75); }
            if *acc.get(x, y) >= big_t {
                big += 1;
                if t { big_trapped += 1; }
                c = lerp(c, [20.0, 40.0, 160.0], 0.7);
            }
        }
        img.put_pixel(x as u32, y as u32, Rgb([c[0] as u8, c[1] as u8, c[2] as u8]));
    }
    let mut depths: Vec<f32> = hm.iter().filter(|(_, _, &e)| e > 0.0).map(|(x, y, &e)| *filled.get(x, y) - e).filter(|&d| d > 1.0).collect();
    depths.sort_by(|a, b| a.partial_cmp(b).unwrap());
    if !depths.is_empty() {
        let dq = |p: f32| depths[((depths.len() - 1) as f32 * p) as usize];
        println!("    basin depth p50 {:.0} p90 {:.0} max {:.0}", dq(0.5), dq(0.9), dq(1.0));
    }
    if let Some(p) = precip {
        let (mut a, mut na, mut b, mut nb) = (0.0f64, 0, 0.0f64, 0);
        for (x, y, &d) in ld.iter() {
            if *hm.get(x, y) <= 0.0 { continue; }
            if d > 1.0 { a += *p.get(x, y) as f64; na += 1; } else { b += *p.get(x, y) as f64; nb += 1; }
        }
        println!("    precipitation: basins {:.0} mm/yr, draining land {:.0} mm/yr", a / na.max(1) as f64, b / nb.max(1) as f64);
    }
    // Raw steepest descent from every big-river cell: does the water reach the sea, a lake
    // (hollow of 4+ cells), or stop in a pit?
    let (mut to_sea, mut to_lake, mut to_pit) = (0usize, 0usize, 0usize);
    let lake_cells = |x: usize, y: usize| *ld.get(x, y) > 0.5;
    for (x0, y0, &a) in acc.iter() {
        if a < big_t || *hm.get(x0, y0) <= 0.0 { continue; }
        let (mut x, mut y) = (x0, y0);
        let mut outcome = 2;
        for _ in 0..w * h {
            let e = *hm.get(x, y);
            if e <= 0.0 { outcome = 0; break; }
            if lake_cells(x, y) { outcome = 1; break; }
            let mut best = (0.0f32, x, y);
            for (nx, ny) in hm.neighbors_8(x, y) {
                let d = if nx != x && ny != y { 1.414 } else { 1.0 };
                let s = (e - *hm.get(nx, ny)) / d;
                if s > best.0 { best = (s, nx, ny); }
            }
            if best.0 <= 0.0 { break; }
            x = best.1; y = best.2;
        }
        match outcome { 0 => to_sea += 1, 1 => to_lake += 1, _ => to_pit += 1 }
    }
    let tot = (to_sea + to_lake + to_pit).max(1) as f32;
    println!("    big-river descent: sea {:.1}%  lake {:.1}%  pit/flat {:.1}%", 100.0 * to_sea as f32 / tot, 100.0 * to_lake as f32 / tot, 100.0 * to_pit as f32 / tot);
    let _ = img.save(format!("{out}/terrain_{stage}.png"));
    println!(
        "[{stage:>10}] land {:5.1}%  p50 {:5.0}  p90 {:5.0}  p99 {:5.0}  max {:5.0}  HI {:.2}  basins {:5.1}% of land  big rivers trapped {:5.1}% ({} cells)",
        100.0 * land.len() as f32 / (w * h) as f32,
        q(0.5), q(0.9), q(0.99), max, hi,
        100.0 * trapped as f32 / land.len().max(1) as f32,
        100.0 * big_trapped as f32 / big.max(1) as f32, big,
    );
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let arg = |i: usize, d: &str| a.get(i).cloned().unwrap_or_else(|| d.to_string());
    let seed: u64 = arg(1, "42").parse().unwrap();
    let w: usize = arg(2, "512").parse().unwrap();
    let h: usize = arg(3, "256").parse().unwrap();
    let style = WorldStyle::from_str(&arg(4, "earthlike")).unwrap_or_default();
    let out = arg(5, ".");
    let verbose = std::env::var("LAB_VERBOSE").is_ok();
    std::fs::create_dir_all(&out).ok();
    let seeds = WorldSeeds::from_master(seed);
    let t0 = std::time::Instant::now();

    let params = plates::TectonicParams::default();
    let t = plates::generate_tectonic_terrain(w, h, None, style, &seeds, &params);
    let mut hm = t.heightmap;
    report("tectonic", &hm, &out, None);

    let climate_config = climate::ClimateConfig::default();
    let sim = climate::run_climate_simulation(&hm, &climate_config, seeds.climate);

    if std::env::var("LAB_NO_LEM").is_err() {
        let t1 = std::time::Instant::now();
        let mut lp = erosion::landscape::LandscapeParams::default();
        let env = |k: &str| std::env::var(k).ok().and_then(|v| v.parse::<f32>().ok());
        if let Some(v) = env("LEM_K") { lp.k_fluvial = v; }
        if let Some(v) = env("LEM_U") { lp.uplift_m_per_yr = v; }
        if let Some(v) = env("LEM_D") { lp.diffusion_m2_per_yr = v; }
        if let Some(v) = env("LEM_T") { lp.duration_yr = v; }
        if let Some(v) = env("LEM_STEPS") { lp.steps = v as usize; }
        if let Some(v) = env("LEM_FLEX") { lp.flexure_km = v; }
        let r = erosion::landscape::evolve(&mut hm, &sim.annual_precipitation, &t.stress_map, &lp);
        println!("landscape: {:?} in {:.2}s", r, t1.elapsed().as_secs_f32());
        report("landscape", &hm, &out, Some(&sim.annual_precipitation));
    }
    if std::env::var("LAB_ONLY_LEM").is_ok() { return; }

    let mut ep = erosion::ErosionParams::from_preset(erosion::ErosionPreset::Normal);
    ep.tune_for_heightmap(&hm);
    let mut rng = ChaCha8Rng::seed_from_u64(seeds.erosion);
    erosion::simulate_erosion(&mut hm, &t.plate_map, &t.plates, &t.stress_map, &sim.mean_temperature, &ep, &mut rng, seeds.erosion);
    report("erosion", &hm, &out, None);

    let map_scale = scale::MapScale::default();
    let cp = coastline::CoastlineParams::default();
    let net = coastline::generate_coastline_network(&hm, &cp, seeds.coastline);
    coastline::apply_coastline_to_heightmap(&net, &mut hm, cp.blend_width);
    if verbose { report("coastline", &hm, &out, None); }
    heightmap::apply_fjord_incisions(&mut hm, seeds.heightmap, &map_scale);
    if verbose { report("fjords", &hm, &out, None); }
    heightmap::apply_regional_noise_stacks(&mut hm, &t.stress_map, seeds.heightmap);
    if verbose { report("noise", &hm, &out, None); }
    heightmap::apply_volcano_pass(&mut hm, &t.stress_map, seeds.heightmap);
    if verbose { report("volcanoes", &hm, &out, None); }
    heightmap::apply_coastal_beaches(&mut hm, &t.stress_map, &map_scale, erosion::landscape::tile_km(w));
    if verbose { report("beaches", &hm, &out, None); }
    let filled = erosion::landscape::fill_pits(&mut hm, 4, 10.0);
    println!("filled {filled} hollow cells");
    report("final", &hm, &out, None);

    let (_, bodies, _, _, _) = water_bodies::detect_water_bodies_climate(&hm, &sim.mean_temperature, &sim.mean_moisture);
    let st = water_bodies::water_body_stats(&bodies);
    println!("lakes {}, river tiles {}, total {:.1}s", water_bodies::count_lakes(&bodies), st.river_tiles, t0.elapsed().as_secs_f32());
}
