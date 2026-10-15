//! The audited terrain (terrain revision 2, `crate::terrain_rev`, `audit.md`, 2026-10-13): the
//! pipeline's causal order, rock hardness from the crust, the climate following the final
//! heights, drainage fresh at the end, and the dev worlds and world codes made on it. The other
//! test files run on it too (re-pinned 2026-10-14; they had run on revision 1 since the audit).

use std::process::Command;

use planet_generator::{erosion, terrain, terrain_rev, tilemap::Tilemap};

/// The binary on the audited terrain, whatever the environment says.
fn cmd() -> Command { let mut c = Command::new(env!("CARGO_BIN_EXE_planet_generator")); c.env_remove("PLANET_TERRAIN_V1"); c }

fn run(c: &mut Command) -> String {
    let o = c.output().expect("run planet_generator");
    format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

/// A 128x64 world on the audited terrain, and the stages it went through.
fn world(seed: u64) -> (terrain::Terrain, Vec<&'static str>, Tilemap<f32>) {
    terrain_rev::set_v1(false);
    assert!(!terrain_rev::v1(), "these tests need the audited terrain");
    let cfg = terrain::TerrainConfig::new(128, 64, planet_generator::plates::WorldStyle::Earthlike);
    let mut names = Vec::new();
    let mut climate_heights = None;
    let t = terrain::generate_terrain(&cfg, &planet_generator::seeds::WorldSeeds::from_master(seed), &mut |st| {
        names.push(st.name);
        if st.name == "climate" { climate_heights = Some(st.heightmap.clone()); }
        true
    }).expect("the terrain runs to the end");
    (t, names, climate_heights.unwrap())
}

/// What builds the land comes before the climate and the rivers; fjords and beaches after
/// (`audit.md` 1.1). The final flow is that of the final map (1.2), the rock is not one
/// hardness everywhere (1.2) and the climate's temperatures follow the final heights (1.3).
#[test]
fn the_pipeline_is_causal() {
    let (t, names, before) = world(3);
    assert_eq!(names, ["tectonic", "volcanoes", "noise", "islands", "coastline", "climate", "landscape", "erosion", "fjords", "beaches", "final"]);
    let (_, acc, _) = erosion::rivers::compute_flow_with_filled_routing(&t.heightmap);
    assert!(acc.iter().zip(t.flow_accumulation.iter()).all(|(a, b)| a.2 == b.2), "flow accumulation is stale");
    let land: Vec<f32> = t.heightmap.iter().filter(|(_, _, &e)| e > 0.0).map(|(x, y, _)| *t.hardness.get(x, y)).collect();
    let (lo, hi) = land.iter().fold((1.0f32, 0.0f32), |(a, b), &v| (a.min(v), b.max(v)));
    assert!(lo < 0.4 && hi > 0.75, "hardness on land spans {lo:.2}..{hi:.2}: soft basins and hard shields expected");
    // The heights moved between the climate and the end, and the temperatures follow them by
    // the lapse rate: a cell raised a kilometre is that much colder.
    let lapse = t.climate.forcing.lapse_rate / 1000.0;
    let moved = t.heightmap.iter().filter(|(x, y, &e)| { let b = *before.get(*x, *y); b > 0.0 && e > 0.0 && (b - e).abs() > 300.0 }).count();
    assert!(moved > 0, "no land cell moved 300 m between the climate and the end");
    let (x, y) = t.heightmap.iter().find(|(_, _, &e)| e > 100.0).map(|(x, y, _)| (x, y)).unwrap();
    let mut sim = t.climate.clone();
    let mut raised = t.heightmap.clone();
    raised.set(x, y, *t.heightmap.get(x, y) + 1000.0);
    terrain::follow_heights(&mut sim, &t.heightmap, &raised);
    let dt = *sim.mean_temperature.get(x, y) - *t.climate.mean_temperature.get(x, y);
    assert!((dt + lapse * 1000.0).abs() < 1e-3, "a kilometre higher: {dt:+.2} C, want {:+.2}", -lapse * 1000.0);
    assert!((*sim.seasonal_temperatures[1].get(x, y) - *t.climate.seasonal_temperatures[1].get(x, y) - dt).abs() < 1e-3, "the seasons follow too");
}

/// The dev world on the audited terrain keeps its landmarks through the history (seed 10:
/// 7 rivers, 5 lakes, 6 ranges, a desert, an island, 3 continents).
#[test]
fn the_dev_world_has_its_landmarks() {
    let text = run(cmd().args(["--dev", "--headless", "--gazetteer"]));
    let count = |kind: &str| text.lines().find_map(|l| l.trim_start().strip_prefix(&format!("{kind} x")).and_then(|r| r.split_whitespace().next()?.parse::<usize>().ok())).unwrap_or(0);
    assert!(text.contains("master seed: 10"), "the dev world is seed 10 on the audited terrain");
    assert!(count("River") >= 4 && count("Lake") >= 2 && count("MountainRange") >= 4 && count("Desert") >= 1 && count("Continent") >= 2,
        "landmarks: rivers {} lakes {} ranges {} deserts {} continents {}", count("River"), count("Lake"), count("MountainRange"), count("Desert"), count("Continent"));
}

/// Each dev sky's seed meets its requirements on the audited terrain (no rejected world).
#[test]
fn the_dev_skies_meet_their_requirements() {
    for (sky, req) in [("eyeball", "rivers=2,forests=1,lakes=1"), ("slow", "rivers=2,forests=1"), ("tilted", "rivers=2,forests=1"), ("twosuns", "rivers=2,forests=1,lakes=1")] {
        let text = run(cmd().args(["--dev-sky", sky, "--headless", "--no-history", "--require", req]));
        assert!(!text.contains("World rejected"), "{sky}: {}", text.lines().filter(|l| l.contains("rejected")).collect::<Vec<_>>().join("\n"));
    }
}

/// World codes mark the audited terrain (".t2"); a code without the mark is a world as it was
/// made before the audit, on revision 1.
#[test]
fn world_codes_carry_the_terrain() {
    let dir = std::env::temp_dir().join(format!("terrain_codes_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let code = |c: &mut Command| run(c).lines().find_map(|l| l.strip_prefix("World code: ").map(str::to_string)).unwrap_or_default();
    let new = code(cmd().args(["--dev", "--sim-snapshot"]).arg(dir.join("a")));
    assert!(new.starts_with("10.96x48.earthlike.8.250.t2@"), "audited dev world's code: {new}");
    let old = code(cmd().env("PLANET_MYTH", "1").args(["--code", "76.96x48.earthlike.8.250@45,12", "--sim-snapshot"]).arg(dir.join("b")));
    assert_eq!(old, "76.96x48.earthlike.8.250@45,12", "an old code is the old world");
    let _ = std::fs::remove_dir_all(&dir);
}
