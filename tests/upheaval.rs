//! The violent land (ROADMAP Stage 7, `crate::upheaval`): a world under another star keeps a
//! record of craters, faults and flood basalts and shows them; the Sun's worlds have none
//! unless asked. Seed 42 at 256x128 circles a K6 dwarf (flux 1.78x the Earth's).

use std::process::Command;

fn cmd() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_planet_generator"));
    c.env_remove("PLANET_TERRAIN_V1");
    c.env_remove("PLANET_UPHEAVAL");
    c.env("PLANET_CHUNK_CACHE", std::env::temp_dir().join("planet_test_chunks").to_string_lossy().to_string());
    c
}

fn run(c: &mut Command) -> String {
    let out = c.output().expect("the binary runs");
    let text = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "the binary failed:\n{text}");
    text
}

const WORLD: [&str; 8] = ["--seed", "42", "--width", "256", "--height", "128", "--headless", "--no-history"];

#[test]
fn the_record_lists_craters_faults_and_where() {
    let text = run(cmd().args(WORLD).arg("--upheaval-report"));
    assert!(text.contains("Upheaval: impact flux"), "{text}");
    let line = text.lines().find(|l| l.starts_with("Craters:")).expect("a craters line");
    let on_land: usize = line.split(", ").nth(1).and_then(|p| p.split_whitespace().next()).and_then(|n| n.parse().ok()).expect("craters on land");
    assert!(on_land > 30, "{line}");
    for kind in ["Normal faults:", "Thrust faults:", "StrikeSlip faults:"] {
        assert!(text.lines().any(|l| l.starts_with(kind)), "no '{kind}' in:\n{text}");
    }
}

#[test]
fn the_sun_has_none_unless_asked() {
    let sun = ["--seed", "42", "--star", "sol", "--width", "128", "--height", "64", "--headless", "--no-history", "--upheaval-report"];
    assert!(run(cmd().args(sun)).contains("No upheaval"));
    assert!(run(cmd().args(sun).arg("--upheaval")).contains("Upheaval: impact flux 1.00x"));
    assert!(run(cmd().args(WORLD).args(["--no-upheaval", "--upheaval-report"])).contains("No upheaval"));
}

#[test]
fn a_crater_is_named_with_its_cause() {
    let report = run(cmd().args(WORLD).arg("--upheaval-report"));
    // The largest crater on land: its tile, then the region round it.
    let tile = report.lines().find(|l| l.starts_with("  ") && l.contains(" km ")).and_then(|l| l.trim().split(':').next()).expect("a crater line").to_string();
    let text = run(cmd().args(WORLD).args(["--zoom-tiles", "4", "--zoom-scale", "64", "--places", &tile]));
    assert!(text.contains("a stone from the sky struck here"), "no crater with its cause round {tile}:\n{text}");
}

/// Seed 7 at 512x256 (its own star): stratovolcanoes (the old draws never made one), the one at
/// 24,24 has lost a flank, and its region names it with its cause.
#[test]
fn a_volcano_has_lost_its_side() {
    let world = ["--seed", "7", "--headless", "--no-history"];
    let report = run(cmd().args(world).arg("--upheaval-report"));
    let cones = report.lines().find(|l| l.contains("stratovolcanoes on land")).expect("stratovolcanoes on land");
    assert!(cones.contains("24,24 active snowy (a fallen flank)"), "{cones}");
    let text = run(cmd().args(world).args(["--places", "24,24"]));
    assert!(text.contains("a volcano's fallen flank") && text.contains("the volcano's side gave way"), "no fallen flank at 24,24:\n{text}");
}
