//! The region's survey (ROADMAP Stage 1, `region::survey`): the numbers of the standard sites
//! stay in their bands, and two windows that overlap agree on the site layer. The pins are of
//! today's generation (2026-10-16); a change that moves them is a change to look at, not a bug,
//! unless it leaves the realistic bands further behind.

use std::process::Command;

fn cmd() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_planet_generator"));
    c.env_remove("PLANET_TERRAIN_V1");
    // A chunk cache of the test's own, so a stale one from another build never answers.
    c.env("PLANET_CHUNK_CACHE", std::env::temp_dir().join("planet_test_chunks").to_string_lossy().to_string());
    c
}

fn run(c: &mut Command) -> String {
    let out = c.output().expect("the binary runs");
    let text = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "the binary failed:\n{text}");
    text
}

/// The number after `label` on its line ("  bifurcation ratio: 12.11 (above the band)").
fn number(text: &str, label: &str) -> f32 {
    let line = text.lines().find(|l| l.trim_start().starts_with(label)).unwrap_or_else(|| panic!("no line '{label}' in:\n{text}"));
    let rest = line.trim_start()[label.len()..].trim_start();
    let tok: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect();
    tok.parse().unwrap_or_else(|_| panic!("no number after '{label}' in '{line}'"))
}

fn within(v: f32, lo: f32, hi: f32, what: &str) {
    assert!(v >= lo && v <= hi, "{what} {v} outside {lo}..{hi}");
}

/// The dev cave region (seed 10, 42,32 at 3.26 km cells): flat to the geomorphons, a long
/// river network. Pinned loosely; the bands are the realistic ones only on 512-wide worlds.
#[test]
fn the_dev_cave_region_surveys_as_pinned() {
    let text = run(cmd().args(["--dev", "--headless", "--no-history", "--region-survey", "42,32", "--map-output-dir", &std::env::temp_dir().to_string_lossy()]));
    within(number(&text, "drainage density:"), 0.004, 0.02, "drainage density (km/km2)");
    within(number(&text, "bifurcation ratio:"), 6.0, 20.0, "bifurcation ratio");
    within(number(&text, "Hack's exponent:"), 0.2, 0.6, "Hack's exponent");
    within(number(&text, "hypsometric integral:"), 0.05, 0.3, "hypsometric integral");
    assert!(text.contains("geomorphons: flat 100.0%") || text.contains("geomorphons: flat 99"), "the dev world is flat to the geomorphons:\n{text}");
    // The site layer agrees across windows: the landforms and the rock everywhere, the places
    // but for the few that read a channel distance past the margin.
    let agree = text.lines().find(|l| l.contains("two windows 3 tiles apart agree")).expect("the agreement line");
    let landform = number(agree, "two windows 3 tiles apart agree on the overlap: landform");
    within(landform, 99.5, 100.0, "landform agreement %");
    let (both, either) = {
        let p = agree.split("places ").nth(1).expect("places a of b");
        let mut it = p.split(" of ");
        (it.next().unwrap().trim().parse::<f32>().unwrap(), it.next().unwrap().trim().parse::<f32>().unwrap())
    };
    assert!(either == 0.0 || both / either >= 0.95, "places agree {both} of {either}");
    assert!(text.contains("check: 0 violations"), "the places keep their invariants:\n{text}");
}

/// Seed 42's Falls region (177,133 at 611 m cells): a river country with dolines. The
/// bifurcation ratio and the drainage density sit off the realistic bands by the region's
/// own river rule (only permanent streams are drawn); pinned where they are.
#[test]
fn the_falls_region_of_seed_42_surveys_as_pinned() {
    let text = run(cmd().args(["--seed", "42", "--star", "sol", "--width", "512", "--height", "256", "--headless", "--no-history",
        "--region-survey", "177,133", "--map-output-dir", &std::env::temp_dir().to_string_lossy()]));
    within(number(&text, "drainage density:"), 0.03, 0.1, "drainage density (km/km2)");
    within(number(&text, "bifurcation ratio:"), 4.0, 9.0, "bifurcation ratio");
    within(number(&text, "Hack's exponent:"), 0.15, 0.6, "Hack's exponent");
    within(number(&text, "hypsometric integral:"), 0.4, 0.7, "hypsometric integral");
    for class in ["marsh", "river terrace", "backswamp", "floodplain", "levee", "doline"] {
        assert!(text.contains(&format!("{class} ")), "the landform mix names {class}:\n{text}");
    }
    assert!(text.contains("check: 0 violations"), "the places keep their invariants:\n{text}");
}
