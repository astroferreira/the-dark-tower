//! The places of the site layer (ROADMAP Stages 1 and 4, `region::sites`): a census over the
//! dev world finds places with causes that keep their invariants, and enough of them.

use std::process::Command;

/// The roadmap's floors (Stage 4: 5 places of 3 kinds a window); the dev census gives 19-89 a
/// window.
const MIN_PLACES_PER_WINDOW: f32 = 5.0;
const MIN_KINDS: usize = 3;

fn cmd() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_planet_generator"));
    c.env_remove("PLANET_TERRAIN_V1");
    c.env("PLANET_CHUNK_CACHE", std::env::temp_dir().join("planet_test_chunks").to_string_lossy().to_string());
    c
}

fn run(c: &mut Command) -> String {
    let out = c.output().expect("the binary runs");
    let text = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "the binary failed:\n{text}");
    text
}

#[test]
fn the_dev_census_has_caused_places_that_hold() {
    let text = run(cmd().args(["--dev", "--headless", "--no-history", "--place-census", "6"]));
    let head = text.lines().find(|l| l.starts_with("Place census:")).expect("the census line");
    // "Place census: 6 windows of 8 tiles on seed 10: N places of K kinds; M windows with none"
    let after = head.split(": ").nth(2).expect("the counts");
    let places: f32 = after.split(' ').next().unwrap().parse().unwrap();
    let kinds: usize = after.split(" of ").nth(1).unwrap().split(' ').next().unwrap().parse().unwrap();
    assert!(places / 6.0 >= MIN_PLACES_PER_WINDOW, "{places} places over 6 windows");
    assert!(kinds >= MIN_KINDS, "{kinds} kinds");
    assert!(text.contains("cause coverage: 100%"), "every place has a cause:\n{text}");
    assert!(text.contains("check: 0 violations"), "the places keep their invariants:\n{text}");
}

/// Every place of a window is planned the same from a window three tiles over.
#[test]
fn places_do_not_depend_on_the_window() {
    let text = run(cmd().args(["--dev", "--headless", "--no-history", "--region-survey", "36,23", "--map-output-dir", &std::env::temp_dir().to_string_lossy()]));
    let agree = text.lines().find(|l| l.contains("two windows 3 tiles apart agree")).expect("the agreement line");
    let p = agree.split("places ").nth(1).expect("places a of b");
    let mut it = p.split(" of ");
    let both: f32 = it.next().unwrap().trim().parse().unwrap();
    let either: f32 = it.next().unwrap().trim().parse().unwrap();
    assert!(either == 0.0 || both / either >= 0.95, "places agree {both} of {either}:\n{agree}");
}

/// Every carvable place of the dev cave region is shown in its embark at its cell (ROADMAP
/// Stage 6: views agree).
#[test]
fn views_agree() {
    let text = run(cmd().args(["--dev", "--headless", "--no-history", "--views-agree", "42,32"]));
    let line = text.lines().find(|l| l.starts_with("views agree:")).expect("the tally");
    let a: f32 = line.split(' ').nth(2).unwrap().parse().unwrap();
    let b: f32 = line.split(" of ").nth(1).unwrap().split(' ').next().unwrap().parse().unwrap();
    assert!(b >= 5.0 && a / b >= 0.8, "{line}\n{text}");
}
