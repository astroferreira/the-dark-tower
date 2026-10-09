//! Adventure mode on the dev world, played by its bot (`--adventure-bot N`): a commoner starts
//! small in the sewers, gets a calling, clears places, kills bosses, takes and finishes quests,
//! and the same seed tells the same story twice.

use std::process::Command;

fn run(acts: &str, seed: Option<&str>) -> String {
    let mut c = Command::new(env!("CARGO_BIN_EXE_planet_generator"));
    c.args(["--adventure-bot", acts]);
    if let Some(s) = seed { c.args(["--seed", s]); }
    let out = c.output().expect("run planet_generator");
    assert!(out.status.success(), "planet_generator failed: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// The number after `label` in the line ("level 12 (knight)" -> 12).
fn after(line: &str, label: &str) -> u32 {
    let at = line.find(label).unwrap_or_else(|| panic!("no '{label}' in: {line}")) + label.len();
    line[at..].trim_start().split(|c: char| !c.is_ascii_digit()).next().unwrap().parse().unwrap()
}

/// "kills 79" style counts.
fn count(line: &str, label: &str) -> u32 { after(line, &format!("{} ", label)) }

#[test]
fn a_commoner_grows_into_a_hero() {
    let out = run("40000", None);
    let places = out.lines().find(|l| l.starts_with("Adventure on seed")).expect("places line");
    // The world's places come from its history and its land: towns, ruins, lairs, tombs...
    for kind in [" towns", " ruins", " lairs", " tombs", " caves", " dark fortress"] {
        let n: u32 = places[..places.find(kind).unwrap()].rsplit(|c: char| !c.is_ascii_digit()).next().unwrap().parse().unwrap();
        assert!(n >= 1, "no{} in: {places}", kind);
    }
    let rows: Vec<&str> = out.lines().filter(|l| l.trim_start().starts_with("act ")).collect();
    assert!(rows.len() >= 10, "no progress rows:\n{out}");
    let last = rows.last().unwrap();
    assert!(after(last, "level") >= 12, "the hero is still small after 40000 acts: {last}");
    assert!(last.contains("(knight)") || last.contains("(paladin)") || last.contains("(sorcerer)") || last.contains("(druid)"), "no calling at level 8+: {last}");
    assert!(count(last, "bosses") >= 2, "no boss slain: {last}");
    assert!(count(last, "quests") >= 1, "no quest finished: {last}");
    assert!(count(last, "chests") >= 5, "no treasure found: {last}");
    assert!(count(last, "deaths") <= 8, "the hero dies too often: {last}");
    // Never stuck: kills rise in every stretch of the run's first half.
    let kills: Vec<u32> = rows.iter().map(|r| count(r, "kills")).collect();
    for w in kills[..kills.len() / 2].windows(2) { assert!(w[1] > w[0], "no kill for a stretch: {:?}", kills); }
    // Start small: the first floors are the town's sewers.
    assert!(out.contains("sets out from"), "{out}");
}

#[test]
fn the_same_seed_tells_the_same_adventure() {
    let a = run("6000", None);
    let b = run("6000", None);
    let strip = |s: &str| s.lines().filter(|l| !l.contains(" ms") && !l.contains(" µs")).collect::<Vec<_>>().join("\n");
    assert_eq!(strip(&a), strip(&b), "two runs diverged");
}
