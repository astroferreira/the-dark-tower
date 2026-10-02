//! Renders the tectonic simulation to PNGs for inspection.
//!
//! Usage: tectonic_preview [seed] [width] [height] [style] [myr] [steps] [out_dir]
//! Writes `<out>/final.png` (plates | thickness | age | elevation | stress) and
//! `<out>/timelapse.png` (elevation every few steps).

use image::{Rgb, RgbImage};
use planet_generator::plates::crust::build_heightmap;
use planet_generator::plates::{
    calculate_stress, generate_plates, TectonicParams, TectonicSim, WorldStyle,
};
use planet_generator::seeds::WorldSeeds;
use planet_generator::tilemap::Tilemap;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

fn lerp(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn hypsometric(e: f32) -> Rgb<u8> {
    let c = if e < 0.0 {
        let t = (-e / 6500.0).clamp(0.0, 1.0);
        lerp([70.0, 150.0, 210.0], [8.0, 20.0, 80.0], t.sqrt())
    } else if e < 500.0 {
        lerp([60.0, 150.0, 70.0], [150.0, 170.0, 80.0], e / 500.0)
    } else if e < 2500.0 {
        lerp([150.0, 170.0, 80.0], [130.0, 95.0, 60.0], (e - 500.0) / 2000.0)
    } else {
        lerp([130.0, 95.0, 60.0], [250.0, 250.0, 250.0], ((e - 2500.0) / 3500.0).clamp(0.0, 1.0))
    };
    Rgb([c[0] as u8, c[1] as u8, c[2] as u8])
}

fn ramp(t: f32) -> Rgb<u8> {
    let c = lerp([20.0, 10.0, 80.0], [250.0, 230.0, 60.0], t.clamp(0.0, 1.0));
    Rgb([c[0] as u8, c[1] as u8, c[2] as u8])
}

fn diverging(v: f32) -> Rgb<u8> {
    let t = v.clamp(-1.0, 1.0);
    let c = if t >= 0.0 { lerp([240.0, 240.0, 240.0], [200.0, 30.0, 30.0], t) } else { lerp([240.0, 240.0, 240.0], [30.0, 60.0, 200.0], -t) };
    Rgb([c[0] as u8, c[1] as u8, c[2] as u8])
}

fn paint<T: Clone>(img: &mut RgbImage, ox: u32, oy: u32, map: &Tilemap<T>, f: impl Fn(&T) -> Rgb<u8>) {
    for (x, y, v) in map.iter() {
        img.put_pixel(ox + x as u32, oy + y as u32, f(v));
    }
}

fn stats(name: &str, map: &Tilemap<f32>) {
    let mut v: Vec<f32> = map.iter().map(|(_, _, &x)| x).collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let q = |p: f32| v[((v.len() - 1) as f32 * p) as usize];
    println!("{name:>10}: min {:8.2} p05 {:8.2} p50 {:8.2} p95 {:8.2} max {:8.2}", q(0.0), q(0.05), q(0.5), q(0.95), q(1.0));
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let arg = |i: usize, d: &str| a.get(i).cloned().unwrap_or_else(|| d.to_string());
    let seed: u64 = arg(1, "42").parse().unwrap();
    let w: usize = arg(2, "512").parse().unwrap();
    let h: usize = arg(3, "256").parse().unwrap();
    let style = WorldStyle::from_str(&arg(4, "earthlike")).unwrap_or_default();
    let myr: f32 = arg(5, "200").parse().unwrap();
    let steps: usize = arg(6, "60").parse().unwrap();
    let out = arg(7, ".");

    let seeds = WorldSeeds::from_master(seed);
    let mut rng = ChaCha8Rng::seed_from_u64(seeds.tectonics);
    let (pm0, plates0) = generate_plates(w, h, None, style, &mut rng);
    let legacy_stress = calculate_stress(&pm0, &plates0);
    let plates_legacy = plates0.clone();
    let params = TectonicParams { total_myr: myr, steps, target_land_fraction: style.target_land_fraction() as f32, ..Default::default() };
    let mut sim = TectonicSim::new(&pm0, &plates0, &mut rng, &params);

    let target = style.target_land_fraction();
    let frames_every = (steps / 6).max(1);
    let cont_pct = |sim: &TectonicSim| {
        let f = sim.crust_fields();
        f.thickness_km.iter().filter(|(_, _, &t)| t >= 20.0).count() as f32 / (w * h) as f32 * 100.0
    };
    println!("continental crust area: start {:.1}%", cont_pct(&sim));
    let mut sheet: Vec<Tilemap<f32>> = vec![build_heightmap(&sim.crust_fields(), seeds.heightmap, target)];
    let t0 = std::time::Instant::now();
    while sim.steps_done() < sim.steps_total() {
        sim.step();
        if sim.steps_done() % frames_every == 0 {
            sheet.push(build_heightmap(&sim.crust_fields(), seeds.heightmap, target));
            println!("  step {:3}: continental crust {:.1}%", sim.steps_done(), cont_pct(&sim));
        }
    }
    println!("simulated {} steps ({} Myr) in {:.2}s", sim.steps_total(), myr, t0.elapsed().as_secs_f32());

    let res = sim.finish(plates0);
    let elev = build_heightmap(&res.crust, seeds.heightmap, target);

    stats("elevation", &elev);
    stats("thickness", &res.crust.thickness_km);
    stats("age", &res.crust.age_myr);
    stats("stress", &res.stress_map);
    stats("legacy str", &legacy_stress);
    let land = elev.iter().filter(|(_, _, &e)| e > 0.0).count() as f32 / (w * h) as f32;
    println!("land fraction {:.1}% (target {:.0}%), plates {}", land * 100.0, target * 100.0, res.plates.len());
    let cont = res.plates.iter().filter(|p| p.plate_type == planet_generator::plates::PlateType::Continental).count();
    let edge = |rows: std::ops::Range<usize>| {
        let mut land = 0; let mut tot = 0;
        for y in rows { for x in 0..w { tot += 1; if *elev.get(x, y) > 0.0 { land += 1; } } }
        100.0 * land as f32 / tot as f32
    };
    println!("land in top 3 rows {:.0}%, bottom 3 rows {:.0}%", edge(0..3), edge(h - 3..h));
    let frac = |m: &Tilemap<f32>, t: f32| m.iter().filter(|(_, _, &v)| v > t).count() as f32 / (w * h) as f32;
    println!("cells with stress>0.04: sim {:.1}%  legacy {:.1}%   stress>0.25: sim {:.1}%  legacy {:.1}%   stress<-0.04: sim {:.1}% legacy {:.1}%",
        frac(&res.stress_map, 0.04) * 100.0, frac(&legacy_stress, 0.04) * 100.0,
        frac(&res.stress_map, 0.25) * 100.0, frac(&legacy_stress, 0.25) * 100.0,
        (1.0 - frac(&res.stress_map, -0.04)) * 100.0, (1.0 - frac(&legacy_stress, -0.04)) * 100.0);
    println!("continental plates {}, oceanic {}", cont, res.plates.len() - cont);

    // Drainage diagnostic: land trapped in closed basins (depression fill deeper than 1 m).
    let basin_report = |name: &str, hm: &Tilemap<f32>, file: &str| {
        let filled = planet_generator::erosion::rivers::fill_depressions_public(hm);
        let (mut land, mut trapped) = (0usize, 0usize);
        let mut img = RgbImage::new(w as u32, h as u32);
        for (x, y, &e) in hm.iter() {
            let depth = *filled.get(x, y) - e;
            let c = if e <= 0.0 {
                Rgb([20, 40, 90])
            } else {
                land += 1;
                if depth > 1.0 {
                    trapped += 1;
                    let t = (depth / 300.0).min(1.0);
                    Rgb([(255.0 * t) as u8 + 0, 60, 60])
                } else {
                    Rgb([150, 150, 150])
                }
            };
            img.put_pixel(x as u32, y as u32, c);
        }
        img.save(format!("{out}/{file}")).unwrap();
        println!("{name}: {:.1}% of land is inside closed basins", 100.0 * trapped as f32 / land.max(1) as f32);
    };
    basin_report("sim basins", &elev, "basins_sim.png");
    {
        use planet_generator::heightmap;
        let lm = heightmap::generate_heightmap(&pm0, &plates_legacy, &legacy_stress, seeds.heightmap);
        let mut lm = lm;
        heightmap::apply_inland_uplift(&mut lm, &legacy_stress, &planet_generator::scale::MapScale::default());
        basin_report("legacy basins", &lm, "basins_legacy.png");
    }

    let (wu, hu) = (w as u32, h as u32);
    let mut img = RgbImage::new(wu * 2, hu * 3);
    paint(&mut img, 0, 0, &res.plate_map, |id| {
        if id.is_none() { Rgb([0, 0, 0]) } else { let c = res.plates[id.0 as usize].color; Rgb(c) }
    });
    paint(&mut img, wu, 0, &res.crust.thickness_km, |t| ramp((t - 5.0) / 70.0));
    paint(&mut img, 0, hu, &res.crust.age_myr, |t| ramp(t / 180.0));
    paint(&mut img, wu, hu, &elev, |e| hypsometric(*e));
    paint(&mut img, 0, hu * 2, &res.stress_map, |s| diverging(*s * 1.5));
    paint(&mut img, wu, hu * 2, &legacy_stress, |s| diverging(*s * 1.5));
    img.save(format!("{out}/final.png")).unwrap();

    let cols = 2u32;
    let rows = ((sheet.len() as u32) + cols - 1) / cols;
    let mut tl = RgbImage::new(wu * cols, hu * rows);
    for (i, m) in sheet.iter().enumerate() {
        paint(&mut tl, (i as u32 % cols) * wu, (i as u32 / cols) * hu, m, |e| hypsometric(*e));
    }
    tl.save(format!("{out}/timelapse.png")).unwrap();
}
