//! The realistic default (`crate::myth` off, 2026-10-13): a medieval world with no Shadow, no
//! beasts of legend or the deep, no peoples but humans, no ghosts, risen dead, werebeasts,
//! vampires or hauntings, caves holding only what caves hold, no magma sea or adamantine, and
//! adventure mode closed. The other test files run with myth on (`PLANET_MYTH=1`).

use std::process::Command;

/// The binary as it starts by default: myth off and the audited terrain, whatever the
/// environment says.
fn real_cmd() -> Command { let mut c = Command::new(env!("CARGO_BIN_EXE_planet_generator")); c.env_remove("PLANET_MYTH").env_remove("PLANET_TERRAIN_V1"); c }

/// What it prints, on both streams (the history's summary goes to stderr).
fn stdout(c: &mut Command) -> String { let o = c.output().expect("run planet_generator"); format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)) }

/// The dev world's history without myth: every people human, no beasts of legend, no Shadow.
#[test]
fn the_world_is_human_and_has_no_shadow() {
    for seed in ["76", "11", "3"] {
        let text = stdout(real_cmd().args(["--dev", "--seed", seed, "--headless", "--present"]));
        let kinds = text.lines().find(|l| l.starts_with("Peoples by kind: ")).unwrap_or_else(|| panic!("seed {seed}: no peoples line:\n{text}"));
        assert!(kinds.split(": ").nth(1).map_or(false, |k| k.split(", ").all(|x| x.ends_with(" human"))), "seed {seed}: a people not human: {kinds}");
        assert!(text.contains("Legendary creatures: 0 (0 alive)"), "seed {seed}: beasts of legend in a realistic world");
        assert!(!text.contains("The Shadow's story:") && !text.contains("living beasts"), "seed {seed}: the Shadow or beasts reported:\n{text}");
    }
    // With myth the old world comes back: other peoples and beasts.
    let mut c = Command::new(env!("CARGO_BIN_EXE_planet_generator"));
    let myth = stdout(c.env("PLANET_MYTH", "1").env_remove("PLANET_TERRAIN_V1").args(["--dev", "--headless", "--present"]));
    assert!(myth.lines().any(|l| l.starts_with("Peoples by kind: ") && !l.split(": ").nth(1).unwrap_or("").split(", ").all(|x| x.ends_with(" human"))), "myth brought no other peoples");
    assert!(!myth.contains("Legendary creatures: 0 (0 alive)"), "myth brought no beasts");
}

/// A camp lives a year in the realistic world without anything uncanny: no ghosts or risen
/// dead, no werebeast or vampire, no forgotten beast, demon or hollow, no adamantine, magma or
/// fungus trees, no possession.
#[test]
fn the_camp_meets_nothing_uncanny() {
    let dir = std::env::temp_dir().join(format!("realism_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for seed in ["76", "11", "3", "5"] {
        let log = dir.join(format!("log_{seed}.txt"));
        let out = real_cmd().args(["--dev", "--headless", "--seed", seed, "--sim-projects", "200"]).env("PLANET_DUMP_LOG", &log).output().expect("run planet_generator");
        assert!(out.status.success(), "seed {seed}: the run failed");
        let text = std::fs::read_to_string(&log).unwrap_or_default().to_lowercase();
        assert!(text.lines().count() > 100, "seed {seed}: a short log");
        for word in ["ghost", "risen", "werebeast", "under the full moon", "vampire", "drank the blood", "forgotten beast", "demon", "the hollow", "adamantine",
            "magma", "fungus tree", "a possession", "the shadow", "giant cave spider", "pale light walks", "something sleeps in the deep",
            "fey mood", "god of magic", " dwarf", " elves", " orc ", " orcs", "goblin"] {
            assert!(!text.contains(word), "seed {seed}: '{word}' in a realistic camp's log:\n{}", text.lines().filter(|l| l.contains(word)).take(3).collect::<Vec<_>>().join("\n"));
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Caves without myth hold only what caves hold, and there is no sea of fire under the world.
#[test]
fn caves_hold_only_what_caves_hold() {
    // (42,32: caverns at 11, 26 and 43 levels down.)
    let text = stdout(real_cmd().args(["--dev", "--sim-projects", "1", "--tiles-center", "42,32"]));
    let caverns: Vec<&str> = text.lines().filter(|l| l.trim_start().starts_with("Cavern:")).collect();
    assert!(!caverns.is_empty(), "no caverns at all:\n{text}");
    for c in &caverns {
        assert!(c.contains("(0 fungus trees") && !c.contains("sleeps there") && !c.contains("giant") && !c.contains("horror"), "a cavern of myth: {c}");
    }
    assert!(!text.lines().any(|l| l.trim_start().starts_with("Magma:") && !l.contains("none")), "a magma sea in a realistic embark:\n{text}");
}

/// Adventure mode (monsters, spells, dragons) opens only with myth.
#[test]
fn adventure_mode_needs_myth() {
    let out = real_cmd().args(["--adventure-bot", "1"]).output().expect("run planet_generator");
    assert!(!out.status.success(), "adventure ran without myth");
    assert!(String::from_utf8_lossy(&out.stderr).contains("opens only with --myth"), "no word on why");
}
