//! The star system through the real binary: every world gets one from its seed, the Sun keeps
//! the original climate (the dev world's), world codes and saved worlds carry the sky with them.

use std::process::Command;

/// The binary with myth and magic on (`--myth`, `crate::myth`): these tests are of the game's
/// systems as they were, Shadow, beasts and all; the realistic default has its own tests.
fn myth_cmd() -> Command { let mut c = Command::new(env!("CARGO_BIN_EXE_planet_generator")); c.env("PLANET_MYTH", "1").env_remove("PLANET_TERRAIN_V1"); c }

fn run(args: &[&str]) -> String {
    let out = myth_cmd().args(args).output().expect("run planet_generator");
    assert!(out.status.success(), "planet_generator {:?} failed: {}", args, String::from_utf8_lossy(&out.stderr));
    format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr))
}

fn line<'a>(out: &'a str, start: &str) -> &'a str {
    out.lines().find(|l| l.starts_with(start)).unwrap_or_else(|| panic!("no '{start}' line in:\n{out}"))
}

#[test]
fn a_seed_draws_a_sky_and_its_climate_follows() {
    let out = run(&["--seed", "6", "--width", "96", "--height", "48", "--headless", "--no-history", "--system"]);
    let star = line(&out, "Star system: ");
    assert!(star.contains("red dwarf") && star.contains("locked to its star"), "seed 6 circles a red dwarf, locked: {star}");
    for section in ["== The sun", "== Orbit and day", "== The world", "== The air", "== Climate", "== The sky", "== Moons and tides", "== Neighbours"] {
        assert!(out.contains(section), "no {section}:\n{out}");
    }
    assert!(out.contains("Under the sun") && out.contains("on the night side"), "a locked world's climate has a day side and a night side");
    // The same seed under a pinned class and orbit: another star, the same draw for the rest.
    let k = run(&["--seed", "6", "--width", "96", "--height", "48", "--headless", "--no-history", "--system", "--star", "K2", "--hz", "0.1"]);
    assert!(line(&k, "Star system: ").contains("K2V orange dwarf"));
    assert!(line(&k, "Star system: ").contains("zone position 0.10"));
}

#[test]
fn the_sun_keeps_the_original_climate() {
    let out = run(&["--seed", "6", "--width", "96", "--height", "48", "--headless", "--no-history", "--system", "--star", "sol"]);
    assert!(line(&out, "Star system: ").starts_with("Star system: Earth circles the Sun, a G2V"));
    assert!(line(&out, "Under the Sun: ").contains("+0 W/m2"), "the Earth's air adds no forcing");
    // The dev world is the Sun's unless a star is asked for.
    let dev = run(&["--dev", "--headless", "--no-history", "--system"]);
    assert!(line(&dev, "Star system: ").contains("circles the Sun"));
}

#[test]
fn world_codes_and_saved_worlds_carry_the_sky() {
    let free = run(&["--seed", "11", "--width", "96", "--height", "48", "--headless", "--no-history", "--system"]);
    // A code's sixth part is the star; a code without one is the Sun's (codes made before).
    let coded = run(&["--code", "11.96x48.earthlike.8.0.auto@10,10", "--system"]);
    assert_eq!(line(&free, "Star system: "), line(&coded, "Star system: "));
    let old = run(&["--code", "11.96x48.earthlike.8.0@10,10", "--system"]);
    assert!(line(&old, "Star system: ").contains("circles the Sun"));

    let dir = std::env::temp_dir().join(format!("heavens_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("w.world");
    let card = dir.join("heavens.png");
    run(&["--seed", "11", "--width", "96", "--height", "48", "--headless", "--no-history", "--save-world", path.to_str().unwrap()]);
    let loaded = run(&["--load-world", path.to_str().unwrap(), "--headless", "--system", "--heavens-snapshot", card.to_str().unwrap()]);
    let sun_line = free.lines().find(|l| l.starts_with("  ") && l.contains(" the Sun's mass")).expect("sun line");
    assert!(loaded.contains(sun_line), "the loaded world lost its star:\n{loaded}");
    let snap = run(&["--load-world", path.to_str().unwrap(), "--headless", "--heavens-snapshot", card.to_str().unwrap()]);
    assert!(snap.contains("Heavens drawn to"), "{snap}");
    assert!(std::fs::metadata(&card).map(|m| m.len() > 10_000).unwrap_or(false), "no heavens card");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_worlds_are_drawn_as_globes() {
    let dir = std::env::temp_dir().join(format!("worlds_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let png = dir.join("worlds.png");
    // Seed 6: a red dwarf with lava, hothouse, barren and cold desert worlds, ringed ice giants.
    let out = run(&["--seed", "6", "--width", "96", "--height", "48", "--headless", "--no-history", "--worlds-snapshot", png.to_str().unwrap()]);
    assert!(out.contains("Worlds drawn to"), "{out}");
    // The neighbours went through the terrain pipeline.
    assert!(out.matches("Simulating plate tectonics").count() >= 5, "neighbours not made by the pipeline");
    assert!(std::fs::metadata(&png).map(|m| m.len() > 100_000).unwrap_or(false), "no worlds picture");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_sky_writes_itself_into_the_chronicle() {
    let dir = std::env::temp_dir().join(format!("sky_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let dump = dir.join("chronicle.txt");
    // Seed 4: two suns (a red dwarf companion that flares), a great moon that covers them, comets.
    run(&["--seed", "4", "--width", "96", "--height", "48", "--headless", "--history-years", "250", "--civilizations", "8", "--chronicle-dump", dump.to_str().unwrap()]);
    let text = std::fs::read_to_string(&dump).unwrap();
    let count = |k: &str| text.matches(k).count();
    assert!(count("The Burning Sky over") >= 1 && count("The Eclipse over") >= 1 && count("The Two Suns Meet") >= 1, "sky events missing");
    let all = count("The Burning Sky over") + count("The Eclipse over") + count("The Comet of") + count("The Two Suns Meet") + count("The Year Without Night");
    assert!(all <= 20, "{all} sky events in 250 years is too many");
    // Under the Sun, none.
    run(&["--dev", "--headless", "--chronicle-dump", dump.to_str().unwrap()]);
    let sol = std::fs::read_to_string(&dump).unwrap();
    assert!(!sol.contains("The Burning Sky") && !sol.contains("The Eclipse over") && !sol.contains("The Comet of"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn dev_worlds_under_other_skies() {
    for (name, want) in [("eyeball", "locked to its star"), ("slow", "600.0-hour day"), ("tilted", "tilt 70"), ("twosuns", "F1V")] {
        let out = run(&["--dev-sky", name, "--headless", "--no-history", "--system"]);
        assert!(line(&out, "Star system: ").contains(want), "--dev-sky {name}: {}", line(&out, "Star system: "));
    }
    let twosuns = run(&["--dev-sky", "twosuns", "--headless", "--no-history", "--system"]);
    assert!(twosuns.contains("Two suns"));
    // The eyeball world's dev embark founds a camp on its day side that lives.
    let camp = run(&["--dev-sky", "eyeball", "--sim-projects", "10"]);
    let l = line(&camp, "Projects at ");
    assert!(l.contains("after 10 days"), "{l}");
}
