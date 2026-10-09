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

#[test]
fn an_adventure_saves_and_loads_whole() {
    let path = std::env::temp_dir().join(format!("adv_test_{}.adv", std::process::id()));
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator")).args(["--adventure-bot", "6000"]).env("PLANET_ADV_SAVE", &path).output().expect("run");
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let _ = std::fs::remove_file(&path);
    let line = text.lines().find(|l| l.starts_with("Save:")).unwrap_or_else(|| panic!("no save line:\n{text}"));
    assert!(line.contains("round trip ok") && line.contains("the same"), "{line}");
}

/// The history goes on while the adventure is played (26,000 acts reach the first season) (towns fall, towns are founded, lords
/// change), the adventurer's deeds enter its chronicle, bards in many towns sing them, the
/// world's legends name the adventurer, and a loaded adventure replays the same history.
#[test]
fn the_world_moves_and_remembers() {
    let dir = std::env::temp_dir().join(format!("adv_legends_{}", std::process::id()));
    let save = std::env::temp_dir().join(format!("adv_world_{}.adv", std::process::id()));
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator")).args(["--adventure-bot", "26000"])
        .env("PLANET_ADV_WORLD", "1").env("PLANET_ADV_LEGENDS", &dir).env("PLANET_ADV_SAVE", &save).output().expect("run");
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let _ = std::fs::remove_file(&save);
    assert!(text.lines().any(|l| l.contains("world at day") && (l.contains("has fallen") || l.contains("new town") || l.contains(" rules "))), "the world stood still:\n{text}");
    let hist = text.lines().find(|l| l.starts_with("History:")).unwrap_or_else(|| panic!("no history line:\n{text}"));
    assert!(hist.contains("the same"), "{hist}");
    let songs = text.lines().find(|l| l.starts_with("Songs:")).expect("songs line");
    assert!(count(songs, "Songs:") >= 2, "few towns sing: {songs}");
    let hero = text.lines().find(|l| l.contains(" sets out from ")).and_then(|l| l.split(" of the ").next()).expect("hero line").to_string();
    let named = std::fs::read_dir(&dir).expect("legends dir").filter_map(|e| e.ok()).filter(|e| e.file_name().to_string_lossy().starts_with("figure-"))
        .any(|e| std::fs::read_to_string(e.path()).map_or(false, |t| t.contains(&hero) && t.contains("slew")));
    let _ = std::fs::remove_dir_all(&dir);
    assert!(named, "{} has no page of deeds in the legends", hero);
}

/// A sage and the tavern's drunk asked about the same beast of the history give two accounts;
/// the sage's "where" marks the lair on the map, the drunk's does not (they have it wrong).
#[test]
fn a_sage_and_a_drunk_tell_it_differently() {
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator")).args(["--adventure-bot", "1"]).env("PLANET_ADV_ASK", "1").output().expect("run");
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let sage = text.lines().find(|l| l.starts_with("Ask sage")).unwrap_or_else(|| panic!("no sage:\n{text}"));
    let drunk = text.lines().find(|l| l.starts_with("Ask drunk")).unwrap_or_else(|| panic!("no drunk:\n{text}"));
    assert!(sage.contains("They say") && drunk.contains("They say"), "{sage}\n{drunk}");
    let told = |l: &str| l.split_once(" about ").map(|x| x.1.to_string()).unwrap_or_default();
    assert_ne!(told(sage).split(" Its lair").next(), told(drunk).split(" Its lair").next(), "the same telling");
    assert!(sage.ends_with("[marked true]"), "{sage}");
}
