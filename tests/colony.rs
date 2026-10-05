//! The first colony, unattended on the dev embark: everyone alive after 30 days, nobody stuck,
//! the hut built, and the same story on every run. Runs the real binary (`--sim-snapshot`, a
//! couple of seconds once the dev region is in the chunk cache, ~15 s the first time).

use std::process::Command;

fn run(dir: &std::path::Path) -> (String, String) {
    let prefix = dir.join("colony");
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--sim-snapshot", prefix.to_str().unwrap()])
        .output()
        .expect("run planet_generator");
    assert!(out.status.success(), "planet_generator failed: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let log = std::fs::read_to_string(dir.join("colony_log.txt")).expect("colony log");
    (stdout, log)
}

#[test]
fn seven_settlers_live_thirty_days_unattended() {
    let dir = std::env::temp_dir().join(format!("colony_test_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (out, log) = run(&dir);
    let summary = out.lines().find(|l| l.starts_with("Colony after 30 days")).expect("summary line");
    // The first arc's raid may take one life; nobody else dies.
    // "Colony after 30 days: A of N alive, ...": at most the raid's one death.
    let nums: Vec<usize> = summary.split(|c: char| !c.is_ascii_digit()).filter_map(|n| n.parse().ok()).collect();
    assert!(nums.len() >= 3 && nums[2] >= 7 && nums[2] - nums[1] <= 1, "someone died besides the raid: {summary}\n{log}");
    assert!(summary.contains(" 0 times a settler found no way"), "settlers got stuck: {summary}");
    assert!(log.contains("finishes the hut"), "no hut after 30 days:\n{log}");
    assert!(!log.contains("is starving"), "someone starved:\n{log}");
    // The settlers come out of the history: each cites two events or more, and some share one.
    let pasts = out.lines().find(|l| l.starts_with("Settlers: ")).expect("settlers line");
    let nums: Vec<usize> = pasts.split(|c: char| !c.is_ascii_digit()).filter_map(|n| n.parse().ok()).collect();
    assert!(nums.len() >= 3 && nums[0] == 7 && nums[1] >= 2 && nums[2] >= 1, "settlers without pasts: {pasts}");
    // Deterministic: a second run tells the same story.
    let dir2 = dir.join("again");
    std::fs::create_dir_all(&dir2).unwrap();
    let (_, log2) = run(&dir2);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(log, log2, "two runs of the same colony diverged");
}

#[test]
fn patron_verbs_change_behaviour_within_a_day() {
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--sim-patron"])
        .output()
        .expect("run planet_generator");
    assert!(out.status.success(), "planet_generator failed: {}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout);
    for verb in ["forbid", "bless", "favourite", "dream"] {
        let line = text.lines().find(|l| l.starts_with(&format!("Patron {verb}:"))).unwrap_or_else(|| panic!("no {verb} line:\n{text}"));
        assert!(line.contains(": ok"), "{line}");
    }
    assert!(text.matches("(your doing)").count() >= 4, "each use is an event with the patron as its cause:\n{text}");
}

#[test]
fn founding_stones_shape_the_colony() {
    let dir = std::env::temp_dir().join(format!("founding_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--sim-founding", dir.join("f").to_str().unwrap()])
        .output()
        .expect("run planet_generator");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(out.status.success(), "planet_generator failed: {}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().find(|l| l.starts_with("Founding: layouts differ")).expect("comparison line");
    let (a, b) = line.split("hut ").nth(1).and_then(|x| x.split_once(" -> ")).expect("hut positions");
    assert_ne!(a.trim(), b.trim(), "the hall stone didn't move the hut: {line}");
    let trees: Vec<usize> = text.lines().filter(|l| l.starts_with("Founding with stones")).chain(text.lines().filter(|l| l.starts_with("Founding without stones")))
        .filter_map(|l| l.split(" trees left").next().and_then(|x| x.rsplit(' ').next()).and_then(|n| n.parse().ok())).collect();
    assert!(trees.len() == 2 && trees[0] > trees[1], "the grove stone didn't keep the grove: {trees:?}");
}

#[test]
fn the_colony_wears_its_story() {
    let dir = std::env::temp_dir().join(format!("marks_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--sim-marks", dir.join("m").to_str().unwrap()])
        .output()
        .expect("run planet_generator");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(out.status.success(), "planet_generator failed: {}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("Mark: The builders' stone") && text.contains("was raised on day"), "no builders' stone:\n{text}");
    assert!(text.contains("Mark: The grave of") && text.contains("Here lies"), "no grave:\n{text}");
}

#[test]
fn one_code_one_story() {
    let dir = std::env::temp_dir().join(format!("weekly_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("iv.txt");
    std::fs::write(&script, "0 bless 100 96 6\n400 favour 2\n1500 dream 1 Hut\n").unwrap();
    let hash = |with: bool| -> String {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planet_generator"));
        cmd.args(["--code", "76.96x48.earthlike.8.250@45,12", "--sim-snapshot", dir.join("c").to_str().unwrap()]);
        if with { cmd.args(["--interventions", script.to_str().unwrap()]); }
        let out = cmd.output().expect("run planet_generator");
        assert!(out.status.success(), "planet_generator failed: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).lines().find(|l| l.starts_with("Colony hash: ")).expect("hash line").to_string()
    };
    let (a, b, plain) = (hash(true), hash(true), hash(false));
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(a, b, "the same code and interventions told two stories");
    assert!(a.contains("(3 interventions)"), "not every intervention applied: {a}");
    assert_ne!(a.split(' ').nth(2), plain.split(' ').nth(2), "the interventions changed nothing");
}

#[test]
fn the_first_arc_reaches_the_colony() {
    let dir = std::env::temp_dir().join(format!("arc_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--sim-snapshot", dir.join("a").to_str().unwrap()])
        .output()
        .expect("run planet_generator");
    assert!(out.status.success(), "planet_generator failed: {}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout);
    let tale = std::fs::read_to_string(dir.join("a_tale.html")).unwrap_or_default();
    let saga = std::fs::metadata(dir.join("a_saga.png")).map(|m| m.len()).unwrap_or(0);
    assert!(saga > 100_000, "no saga plate");
    // Faces told apart at 48 px: the two most alike still differ in a sixth of their pixels.
    let faces = text.lines().find(|l| l.starts_with("Faces:")).expect("faces line");
    let pct: u32 = faces.split("differ in ").nth(1).and_then(|x| x.split('%').next()).and_then(|n| n.parse().ok()).unwrap_or(0);
    assert!(pct >= 15, "two settlers look alike: {faces}");
    let _ = std::fs::remove_dir_all(&dir);
    for step in ["Arc day 3: A rumour", "Arc day 6: Refugees", "Arc day 14: The raid"] {
        assert!(text.contains(step), "missing '{step}':\n{text}");
    }
    // Every step says why, and the rumour points into the world's chronicle.
    assert!(text.lines().filter(|l| l.starts_with("Arc day")).all(|l| l.contains("(because")), "a step without a cause:\n{text}");
    assert!(tale.contains("In the chronicle:"), "the tale doesn't reach back into the history:\n{tale}");
}
