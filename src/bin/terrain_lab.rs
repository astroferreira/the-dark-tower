//! Runs the world's terrain pipeline (`terrain::generate_terrain`: tectonics -> climate ->
//! landscape -> erosion -> finishing passes)
//! without history or a window and prints drainage and relief metrics after each stage.
//!
//! Usage: terrain_lab [seed] [width] [height] [style] [out_dir]
//! Writes `<out>/terrain_<stage>.png` (hypsometric tint, closed basins in red).

use image::{Rgb, RgbImage};
use planet_generator::erosion::{self, rivers};
use planet_generator::plates::{self, WorldStyle};
use planet_generator::seeds::WorldSeeds;
use planet_generator::tilemap::Tilemap;
use planet_generator::{climate, terrain, water_bodies};
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
    if stage == "final" { island_report(hm); }
    println!(
        "[{stage:>10}] land {:5.1}%  p50 {:5.0}  p90 {:5.0}  p99 {:5.0}  max {:5.0}  HI {:.2}  basins {:5.1}% of land  big rivers trapped {:5.1}% ({} cells)",
        100.0 * land.len() as f32 / (w * h) as f32,
        q(0.5), q(0.9), q(0.99), max, hi,
        100.0 * trapped as f32 / land.len().max(1) as f32,
        100.0 * big_trapped as f32 / big.max(1) as f32, big,
    );
}

/// Islands (land bodies under 200 cells): count, and how compact they are on average
/// (perimeter^2 / (4 pi area): 1 = disc-like blob, higher = ragged with bays and headlands).
fn island_report(hm: &Tilemap<f32>) {
    let (w, h) = (hm.width, hm.height);
    let land: Vec<bool> = hm.iter().map(|(_, _, &e)| e > 0.0).collect();
    let mut seen = vec![false; w * h];
    let (mut count, mut ratio_sum, mut ratio_n, mut cells) = (0usize, 0.0f64, 0usize, 0usize);
    for s in 0..w * h {
        if !land[s] || seen[s] { continue; }
        let mut comp = vec![s];
        seen[s] = true;
        let mut k = 0;
        while k < comp.len() {
            let i = comp[k];
            for (nx, ny) in hm.neighbors(i % w, i / w) {
                let j = ny * w + nx;
                if land[j] && !seen[j] { seen[j] = true; comp.push(j); }
            }
            k += 1;
        }
        if comp.len() >= 200 { continue; }
        count += 1;
        cells += comp.len();
        if comp.len() >= 6 {
            let perim: usize = comp.iter().map(|&i| hm.neighbors(i % w, i / w).iter().filter(|&&(nx, ny)| !land[ny * w + nx]).count()).sum();
            ratio_sum += (perim * perim) as f64 / (4.0 * std::f64::consts::PI * comp.len() as f64);
            ratio_n += 1;
        }
    }
    println!("    islands: {count} ({cells} cells), raggedness of those >= 6 cells {:.2} (n={ratio_n})", ratio_sum / ratio_n.max(1) as f64);
}

/// Climate diagnostics (LAB_CLIMATE): precipitation by latitude and distance from the coast,
/// runoff, the biome mix, and `precip.png`.
fn climate_report(hm: &Tilemap<f32>, sim: &climate::ClimateSimulation, stress: &Tilemap<f32>, out: &str, seeds: &WorldSeeds) {
    let (w, h) = (hm.width, hm.height);
    let p = &sim.annual_precipitation;
    let mut land: Vec<f32> = hm.iter().filter(|(_, _, &e)| e > 0.0).map(|(x, y, _)| *p.get(x, y)).collect();
    land.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let q = |f: f32| land[((land.len() - 1) as f32 * f) as usize];
    let mean = land.iter().sum::<f32>() / land.len() as f32;
    let ocean: Vec<f32> = hm.iter().filter(|(_, _, &e)| e <= 0.0).map(|(x, y, _)| *p.get(x, y)).collect();
    println!("land precip mm/yr: mean {:.0} p10 {:.0} p25 {:.0} p50 {:.0} p75 {:.0} p90 {:.0} p99 {:.0}; ocean mean {:.0}",
        mean, q(0.1), q(0.25), q(0.5), q(0.75), q(0.9), q(0.99), ocean.iter().sum::<f32>() / ocean.len().max(1) as f32);
    print!("zonal ocean precip (signed lat bands of 10, N to S):");
    for b in 0..18 {
        let (mut sum, mut cnt) = (0.0f32, 0);
        for (x, y, &e) in hm.iter() {
            let lat = 90.0 - (y as f32 + 0.5) / h as f32 * 180.0;
            if e <= 0.0 && ((90.0 - lat) / 10.0) as usize == b { sum += *p.get(x, y); cnt += 1; }
        }
        print!(" {}:{:.0}", 80 - 10 * b as i32, if cnt > 0 { sum / cnt as f32 } else { f32::NAN });
    }
    println!();
    print!("zonal land precip (|lat| bands of 10):");
    for b in 0..9 {
        let (mut sum, mut cnt) = (0.0f32, 0);
        for (x, y, &e) in hm.iter() {
            let lat = (90.0 - (y as f32 + 0.5) / h as f32 * 180.0).abs();
            if e > 0.0 && (lat / 10.0) as usize == b { sum += *p.get(x, y); cnt += 1; }
        }
        print!(" {}0s:{:.0}", b, if cnt > 0 { sum / cnt as f32 } else { f32::NAN });
    }
    println!();
    {
        // Precipitation and wind speed by distance from the coast (cells).
        use std::collections::VecDeque;
        let mut d = vec![usize::MAX; w * h];
        let mut qd = VecDeque::new();
        for (x, y, &e) in hm.iter() { if e <= 0.0 { d[y * w + x] = 0; qd.push_back((x, y)); } }
        while let Some((x, y)) = qd.pop_front() {
            for (nx, ny) in hm.neighbors(x, y) {
                if d[ny * w + nx] == usize::MAX { d[ny * w + nx] = d[y * w + x] + 1; qd.push_back((nx, ny)); }
            }
        }
        print!("by coast distance (precip mm / wind m/s):");
        for (lo, hi) in [(1, 2), (2, 4), (4, 8), (8, 16), (16, 32), (32, 999)] {
            let (mut sp, mut sw, mut c) = (0.0f32, 0.0f32, 0);
            for (x, y, _) in hm.iter() {
                let di = d[y * w + x];
                if di >= lo && di < hi { sp += *p.get(x, y); let (u, v) = *sim.prevailing_winds.get(x, y); sw += (u * u + v * v).sqrt(); c += 1; }
            }
            if c > 0 { print!(" {lo}-{hi}: {:.0}/{:.1} ({c})", sp / c as f32, sw / c as f32); }
        }
        println!();
    }
    {
        let mut img = RgbImage::new(w as u32, h as u32);
        for (x, y, &e) in hm.iter() {
            let v = *p.get(x, y);
            let c = if e <= 0.0 {
                let t = (v / 3000.0).clamp(0.0, 1.0);
                lerp([30.0, 40.0, 70.0], [60.0, 90.0, 160.0], t)
            } else {
                let t = (v.max(1.0).log10() - 1.0) / 2.6; // 10 mm .. 4000 mm
                let t = t.clamp(0.0, 1.0);
                if t < 0.5 { lerp([200.0, 160.0, 90.0], [230.0, 220.0, 120.0], t * 2.0) } else { lerp([230.0, 220.0, 120.0], [20.0, 120.0, 40.0], t * 2.0 - 1.0) }
            };
            img.put_pixel(x as u32, y as u32, Rgb([c[0] as u8, c[1] as u8, c[2] as u8]));
        }
        let _ = img.save(format!("{out}/precip.png"));
    }
    {
        let heur = climate::compute_effective_runoff(&hm, &sim.mean_temperature, &sim.mean_moisture);
        let (mut a, mut b, mut n) = (0.0f64, 0.0f64, 0usize);
        let mut phys: Vec<f32> = Vec::new();
        for (x, y, &e) in hm.iter() {
            if e <= 0.0 { continue; }
            a += *heur.get(x, y) as f64;
            let r = climate::runoff_mm(*p.get(x, y), *sim.mean_temperature.get(x, y));
            b += r as f64; n += 1; phys.push(r);
        }
        phys.sort_by(|x, y| x.partial_cmp(y).unwrap());
        let q = |f: f32| phys[((phys.len() - 1) as f32 * f) as usize];
        println!("runoff: heuristic mean {:.3}; Budyko mean {:.0} mm/yr (p25 {:.0}, p50 {:.0}, p75 {:.0}, p90 {:.0}, p99 {:.0})", a / n as f64, b / n as f64, q(0.25), q(0.5), q(0.75), q(0.9), q(0.99));
    }
    {
        let warm = sim.warmest_season();
        print!("warmest season on land by |lat| band (C):");
        for b in 0..9 {
            let (mut sum, mut cnt) = (0.0f32, 0);
            for (x, y, &e) in hm.iter() {
                let lat = (90.0 - (y as f32 + 0.5) / h as f32 * 180.0).abs();
                if e > 0.0 && (lat / 10.0) as usize == b { sum += *warm.get(x, y); cnt += 1; }
            }
            print!(" {}0s:{:.1}", b, if cnt > 0 { sum / cnt as f32 } else { f32::NAN });
        }
        println!();
    }
    let cfg = planet_generator::biomes::WorldBiomeConfig { fantasy_intensity: 0.0, ..Default::default() };
    let b = planet_generator::biomes::generate_extended_biomes(&hm, &sim.mean_temperature, &sim.mean_moisture, Some(&sim.warmest_season()), stress, &cfg, seeds.biomes);
    planet_generator::biomes::print_biome_stats(&b, &hm, &sim.mean_temperature, &sim.mean_moisture);
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
    let flag = |k: &str| std::env::var(k).is_ok();
    std::fs::create_dir_all(&out).ok();
    let seeds = WorldSeeds::from_master(seed);
    let t0 = std::time::Instant::now();

    if std::env::var("LAB_CRUST_TIME").is_ok() {
        use rand::SeedableRng;
        let mut rng = ChaCha8Rng::seed_from_u64(seeds.tectonics);
        let (pm, pl) = plates::generate_plates(w, h, None, style, &mut rng);
        let params = plates::TectonicParams { target_land_fraction: style.target_land_fraction() as f32, ..Default::default() };
        let mut sim = plates::TectonicSim::new(&pm, &pl, &mut rng, &params);
        let bands = |f: &Tilemap<f32>| {
            let mut out = String::new();
            for b in 0..9 {
                let (mut c, mut tot) = (0, 0);
                for (_, y, &k) in f.iter() {
                    let lat = (90.0 - (y as f32 + 0.5) / h as f32 * 180.0).abs();
                    if (lat / 10.0) as usize == b { tot += 1; if k >= 25.0 { c += 1; } }
                }
                out += &format!(" {}0s:{:.0}%", b, 100.0 * c as f32 / tot.max(1) as f32);
            }
            out
        };
        println!("step {:3}:{}", 0, bands(&sim.crust_fields().thickness_km));
        let every = (sim.steps_total() / 5).max(1);
        while sim.steps_done() < sim.steps_total() {
            sim.step();
            if sim.steps_done() % every == 0 { println!("step {:3}:{}", sim.steps_done(), bands(&sim.crust_fields().thickness_km)); }
        }
        return;
    }

    let mut cfg = terrain::TerrainConfig::new(w, h, style);
    let env = |k: &str| std::env::var(k).ok().and_then(|v| v.parse::<f32>().ok());
    if let Some(v) = env("LEM_K") { cfg.landscape.k_fluvial = v; }
    if let Some(v) = env("LEM_U") { cfg.landscape.uplift_m_per_yr = v; }
    if let Some(v) = env("LEM_D") { cfg.landscape.diffusion_m2_per_yr = v; }
    if let Some(v) = env("LEM_T") { cfg.landscape.duration_yr = v; }
    if let Some(v) = env("LEM_STEPS") { cfg.landscape.steps = v as usize; }
    if let Some(v) = env("LEM_FLEX") { cfg.landscape.flexure_km = v; }
    if flag("LAB_NO_LEM") { cfg.landscape.steps = 0; }
    cfg.island_coasts = !flag("LAB_NO_ISLANDS");
    if flag("LAB_LEGACY_EROSION") { cfg.legacy_erosion = true; }

    let result = terrain::generate_terrain(&cfg, &seeds, &mut |st: &terrain::Stage| {
        let hm = st.heightmap;
        match st.name {
            "tectonic" => {
                report("tectonic", hm, &out, None);
                if let Some(crust) = st.crust {
                    if flag("LAB_GEL") {
                        let mut v: Vec<f32> = hm.iter().map(|(_, _, &e)| e).collect();
                        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
                        let target = style.target_land_fraction();
                        let q = v[((1.0 - target) * (v.len() - 1) as f64) as usize];
                        let ocean = plates::crust::ocean_volume_gel(hm, 0.0);
                        let cont = crust.thickness_km.iter().filter(|(_, _, &k)| k >= 20.0).count() as f32 / (w * h) as f32;
                        let cont_e: Vec<f32> = hm.iter().filter(|(x, y, _)| *crust.thickness_km.get(*x, *y) >= 30.0).map(|(_, _, &e)| e).collect();
                        let mean_cont = cont_e.iter().sum::<f32>() / cont_e.len().max(1) as f32;
                        println!("GEL now {:.0} m; for {:.0}% land: sea level {:+.0} m, GEL {:.0} m; continental crust {:.1}% of map, mean elevation of crust >=30 km {:.0} m",
                            ocean, target * 100.0, q, plates::crust::ocean_volume_gel(hm, q), cont * 100.0, mean_cont);
                    }
                    print!("continental crust (>=25 km) by |lat| band:");
                    for b in 0..9 {
                        let (mut c, mut tot) = (0, 0);
                        for (_, y, &k) in crust.thickness_km.iter() {
                            let lat = (90.0 - (y as f32 + 0.5) / h as f32 * 180.0).abs();
                            if (lat / 10.0) as usize == b { tot += 1; if k >= 25.0 { c += 1; } }
                        }
                        print!(" {}0s:{:.0}%", b, 100.0 * c as f32 / tot.max(1) as f32);
                    }
                    println!();
                }
                let band = (h / 10).max(1);
                let frac = |rows: std::ops::Range<usize>| {
                    let (mut l, mut t) = (0, 0);
                    for y in rows { for x in 0..w { t += 1; if *hm.get(x, y) > 0.0 { l += 1; } } }
                    100.0 * l as f32 / t as f32
                };
                println!("polar land: north 10% of rows {:.1}%, south {:.1}%, edge rows {:.1}% / {:.1}%",
                    frac(0..band), frac(h - band..h), frac(0..1), frac(h - 1..h));
                island_report(hm);
                print!("cells above 5000 m by |lat| band:");
                for b in 0..9 {
                    let c = hm.iter().filter(|(_, y, &e)| e > 5000.0 && ((90.0 - (*y as f32 + 0.5) / h as f32 * 180.0).abs() / 10.0) as usize == b).count();
                    print!(" {}0s:{}", b, c);
                }
                println!();
                !flag("LAB_ONLY_TECTONIC")
            }
            "climate" => {
                if flag("LAB_CLIMATE") {
                    climate_report(hm, st.climate.unwrap(), st.stress_map, &out, &seeds);
                    return false;
                }
                true
            }
            "landscape" => {
                report("landscape", hm, &out, st.climate.map(|c| &c.annual_precipitation));
                !flag("LAB_ONLY_LEM")
            }
            "erosion" => { report("erosion", hm, &out, None); true }
            "beaches" => {
                if verbose { report("beaches", hm, &out, None); }
                if let Some(gel) = st.ocean_gel_m {
                    println!("ocean volume after erosion and finishing passes: {:.0} m GEL (born with {:.0})", plates::crust::ocean_volume_gel(hm, 0.0), gel);
                }
                true
            }
            "final" => { report("final", hm, &out, None); true }
            name => { if verbose { report(name, hm, &out, None); } true }
        }
    });
    let Some(t) = result else { return };

    let sim = &t.climate;
    let (_, bodies, _, _, _) = water_bodies::detect_water_bodies_climate(&t.heightmap, &sim.mean_temperature, &sim.mean_moisture, Some(&sim.annual_precipitation));
    let mut lakes: Vec<_> = bodies.iter().filter(|b| b.id.is_lake()).collect();
    lakes.sort_by_key(|b| std::cmp::Reverse(b.tile_count));
    for b in lakes.iter().take(8) {
        let (x0, y0, x1, y1) = b.bounds;
        println!("lake {:?}: {} tiles, bounds ({x0},{y0})-({x1},{y1}), centre {},{}, depth {:.0} m{}", b.id.0, b.tile_count, (x0 + x1) / 2, (y0 + y1) / 2, b.max_depth, if b.is_endorheic { ", endorheic" } else { "" });
    }
    let st = water_bodies::water_body_stats(&bodies);
    println!("lakes {}, river tiles {}, total {:.1}s", water_bodies::count_lakes(&bodies), st.river_tiles, t0.elapsed().as_secs_f32());
}
