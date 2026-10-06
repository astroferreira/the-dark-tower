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
    // Spoilage is real: food spoils before the drying rack is built (the cold is pinned by the
    // woodpile trial in after_the_hut_the_colony_sets_its_own_work).
    let at = |needle: &str| log.lines().position(|l| l.contains(needle));
    assert!(matches!((at("spoil in the store"), at("finishes a drying rack")), (Some(a), Some(b)) if a < b), "no food spoiled before the rack:\n{log}");
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
    for verb in ["forbid", "bless", "favourite", "dream", "lift", "late hall"] {
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
    // The beats keep no timetable (rumour 2-5, refugees 5-9, raid 11-18 or sooner), but come in order.
    let day_of = |beat: &str| text.lines().find(|l| l.starts_with("Arc day") && l.contains(beat))
        .and_then(|l| l["Arc day ".len()..].split(':').next()).and_then(|d| d.parse::<u32>().ok());
    let (r, f, d) = (day_of(": A rumour"), day_of(": Refugees"), day_of(": The raid"));
    assert!(matches!((r, f, d), (Some(r), Some(f), Some(d)) if r < f && f < d && d <= 18), "beats missing or out of order ({r:?}, {f:?}, {d:?}):\n{text}");
    // Every step says why, and the rumour points into the world's chronicle.
    assert!(text.lines().filter(|l| l.starts_with("Arc day")).all(|l| l.contains("(because")), "a step without a cause:\n{text}");
    assert!(tale.contains("In the chronicle:"), "the tale doesn't reach back into the history:\n{tale}");
    // Pasts act: veterans keep the watch (per head, at least twice the others), and at least
    // three log lines give a settler's past as the reason for what they did.
    let pasts = text.lines().find(|l| l.starts_with("Pasts: ")).expect("pasts line");
    let n: Vec<f32> = pasts.split(|c: char| !c.is_ascii_digit()).filter_map(|x| x.parse().ok()).collect();
    assert!(n.len() >= 5 && n[0] / n[1].max(1.0) >= 2.0 * n[2] / n[3].max(1.0) && n[4] >= 3.0, "pasts don't act: {pasts}");
    // The camp reads at a glance: no two living settlers alike on the map, and night is dark.
    let figures = text.lines().find(|l| l.starts_with("Figures: ") && l.contains(" of ")).expect("figures line");
    let n: Vec<usize> = figures.split(|c: char| !c.is_ascii_digit()).filter_map(|x| x.parse().ok()).collect();
    assert!(n.len() >= 2 && n[0] == n[1], "two settlers look alike on the map: {figures}");
    let night = text.lines().find(|l| l.starts_with("Night: ")).expect("night line");
    let pct: u32 = night.split(" is ").nth(1).and_then(|x| x.split('%').next()).and_then(|x| x.parse().ok()).unwrap_or(0);
    assert!(pct >= 25, "night is not dark enough: {night}");
    // The raid arrives on legs: the attackers are on the map for 300+ game minutes before the
    // clash (at the window's third speed while they come, about 17-20 real seconds).
    let legs = text.lines().find(|l| l.starts_with("Raid on legs:")).expect("raid on legs line");
    let ticks: u64 = legs.split_whitespace().find_map(|x| x.parse().ok()).unwrap_or(0);
    assert!(ticks >= 300, "the attackers were barely on the map: {legs}");
    // The window stops for each beat (a card with its because) and for the finished hut.
    let moments = text.lines().find(|l| l.starts_with("Moments:")).expect("moments line");
    for m in ["A rumour", "Refugees", "The raid", "The hut stands"] {
        assert!(moments.contains(m), "no moment '{m}': {moments}");
    }
}

#[test]
fn after_the_hut_the_colony_sets_its_own_work() {
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--sim-projects", "60"])
        .output()
        .expect("run planet_generator");
    assert!(out.status.success(), "planet_generator failed: {}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout);
    let projects: Vec<&str> = text.lines().filter(|l| l.starts_with("  Project day")).collect();
    assert!(projects.len() >= 3, "fewer than three projects in 60 days:\n{text}");
    // Each with its reason, after the comma ("a palisade (24 of 24 logs, done), for fear of ...").
    assert!(projects.iter().all(|l| l.rsplit("), ").next().map_or(false, |why| why.len() > 8)), "a project without a reason:\n{text}");
    // The woodpile keeps the cold off: forbid its ground and more nights end chilled.
    let cold = text.lines().find(|l| l.starts_with("Cold: ")).expect("cold line");
    let n: Vec<usize> = cold.split(|c: char| !c.is_ascii_digit()).filter_map(|x| x.parse().ok()).collect();
    assert!(n.len() >= 3 && n[2] > n[0], "the woodpile made no difference to the cold: {cold}");
}

/// The raid is the patron's to win or lose: on six dev seeds a careful patron (the watch post
/// blessed, a veteran favoured to captain it, dreams of the walls) changes the night's outcome
/// on at least four against no patron, and a careless one (the watch post forbidden, dreams of
/// rest) loses a settler on at least three.
#[test]
fn the_raid_is_the_patrons_to_win_or_lose() {
    let (mut changed, mut lost) = (0, 0);
    for seed in ["76", "11", "23", "58", "3", "5"] {
        let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
            .args(["--dev", "--seed", seed, "--sim-raid"])
            .output()
            .expect("run planet_generator");
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        let outcome = |style: &str| -> String {
            let line = text.lines().find(|l| l.starts_with(&format!("Raid {}:", style))).unwrap_or_else(|| panic!("seed {seed}: no {style} raid in:\n{text}"));
            line.split_whitespace().nth(2).unwrap().to_string()
        };
        let (absent, careful, careless) = (outcome("Absent"), outcome("Careful"), outcome("Careless"));
        if careful != absent { changed += 1; }
        if careless == "death" { lost += 1; }
        // The raid's line says what the camp had ready.
        assert!(text.contains("nights of watch kept") || text.contains("night of watch kept"), "seed {seed}: no readiness tally:\n{text}");
    }
    assert!(changed >= 4, "a careful patron changed the raid on only {changed} of 6 seeds");
    assert!(lost >= 3, "a careless patron lost a settler on only {lost} of 6 seeds");
}

/// A doomed site says so: the dev world's mountain (48,18) and crater (28,38) have nothing to eat
/// within reach of a camp, and are refused with the reason instead of starving seven.
#[test]
fn a_doomed_site_says_so() {
    for site in ["48,18", "28,38"] {
        let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
            .args(["--dev", "--sim-projects", "30", "--tiles-center", site])
            .output()
            .expect("run planet_generator");
        let text = String::from_utf8_lossy(&out.stdout);
        let survey = text.lines().find(|l| l.starts_with("Survey at")).unwrap_or_else(|| panic!("{site}: no survey:\n{text}"));
        assert!(survey.contains("refused") && survey.contains("to eat"), "{site} was not refused with a reason: {survey}");
        assert!(!text.contains("0 of 9 alive"), "{site}: a camp was made and starved");
    }
}

/// Settlers save themselves: a camp whose berries are gone moves to ground that feeds it (a
/// moment with its reason, the old camp left as a mark), and a camp on land that can feed no
/// one anywhere they can walk gives it up on day 1 instead of starving.
#[test]
fn settlers_save_themselves_before_they_build() {
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--sim-move", "20", "--tiles-center", "18,30"])
        .output()
        .expect("run planet_generator");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("They strike camp"), "the camp did not move:\n{text}");
    assert!(text.contains("Moment: They move the camp (because"), "no moment for the move:\n{text}");
    assert!(text.contains("Mark: The old camp"), "the old camp left no mark:\n{text}");
    // Nobody starves after the move (the raid may take one).
    assert!(text.contains("Move at 18,30 after 20 days: 9 of 9 alive") || text.contains("Move at 18,30 after 20 days: 8 of 9 alive"), "settlers died after moving:\n{text}");
    assert!(!text.contains("died of hunger"), "settlers starved after moving:\n{text}");
    for site in ["48,18", "28,38"] {
        let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
            .env("PLANET_FORCE_CAMP", "1")
            .args(["--dev", "--sim-projects", "30", "--tiles-center", site])
            .output()
            .expect("run planet_generator");
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(text.contains("they left on day 1") && text.contains("7 of 7 alive"), "{site}: no honest departure:\n{text}");
    }
}


/// The refugees are a choice: both answers change the log within days and the tally at the
/// raid, and six dev seeds keep at least four different timetables.
#[test]
fn the_refugees_are_a_choice() {
    let mut schedules = std::collections::HashSet::new();
    for seed in ["76", "11", "23", "58", "3", "5"] {
        let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
            .args(["--dev", "--seed", seed, "--sim-refugees"])
            .output()
            .expect("run planet_generator");
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        if let Some(b) = text.lines().find(|l| l.starts_with("Beats: ")) { schedules.insert(b.to_string()); }
        if text.contains("Refugees taken in") {
            assert!(text.contains("are turned away (your doing)") && text.contains("takes it hard"), "seed {seed}: turning away left no mark in the log:\n{text}");
            let raids: Vec<&str> = text.lines().filter(|l| l.starts_with("  Raid (day")).collect();
            assert!(raids.len() == 2 && raids[0].contains("9 to fight") && raids[1].contains("7 to fight"), "seed {seed}: the answer did not change the raid:\n{text}");
        }
    }
    assert!(schedules.len() >= 4, "only {} different timetables over six seeds: {:?}", schedules.len(), schedules);
}

/// After the raid the story goes on: in 120 days on the dev colony at least three more arc
/// beats (each from a thread of the present day), no ten days without a log line, and winter
/// comes with a reckoning of the store.
#[test]
fn after_the_raid_the_next_trouble_and_winter() {
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--sim-projects", "120"])
        .output()
        .expect("run planet_generator");
    let text = String::from_utf8_lossy(&out.stdout);
    let after = text.lines().find(|l| l.starts_with("  Arc after the first raid: ")).expect("arc line");
    let beats: usize = after.trim_start_matches("  Arc after the first raid: ").split_whitespace().next().and_then(|n| n.parse().ok()).unwrap_or(0);
    assert!(beats >= 3, "fewer than three beats after the first raid: {after}");
    let quiet = text.lines().find(|l| l.starts_with("  Longest quiet: ")).expect("quiet line");
    let days: u64 = quiet.split_whitespace().nth(2).and_then(|n| n.parse().ok()).unwrap_or(99);
    assert!(days < 10, "ten days without a log line: {quiet}");
    assert!(text.contains("Winter comes"), "no winter in 120 days:\n{text}");
}

/// Settlers plan ahead: a year on the dev colony gives at least ten works, each with a reason
/// that names a number (a horizon), and in its first hundred days no two months pass without one
/// begun.
#[test]
fn settlers_plan_ahead() {
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--sim-projects", "365"])
        .output()
        .expect("run planet_generator");
    let text = String::from_utf8_lossy(&out.stdout);
    let works: Vec<&str> = text.lines().filter(|l| l.starts_with("  Project day")).collect();
    assert!(works.len() >= 10, "fewer than ten works in a year:\n{text}");
    for w in &works {
        let why = w.split_once("), ").map(|x| x.1).unwrap_or("");
        assert!(why.chars().any(|c| c.is_ascii_digit()), "a work without a number in its reason: {w}");
    }
    let days: Vec<u64> = works.iter().filter_map(|w| w["  Project day ".len()..].split(':').next().and_then(|d| d.parse().ok())).collect();
    // Over the first hundred days (a starving camp rightly builds nothing; dev 76 goes hungry at
    // the end of its first winter).
    let gap = days.windows(2).filter(|p| p[0] < 100).map(|p| p[1] - p[0]).max().unwrap_or(0);
    assert!(gap < 60, "two months without new work (gap {gap} days):\n{text}");
}

/// A colony made where the walker stood is found again from its code: the code carries the
/// cell, and the same code tells the same story (hash), a different one from the tile's centre.
#[test]
fn a_colony_is_found_again_by_its_code() {
    let dir = std::env::temp_dir().join(format!("code_cell_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let hash = |code: &str| {
        let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
            .args(["--code", code, "--sim-snapshot", dir.join("k").to_str().unwrap()])
            .output()
            .expect("run planet_generator");
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        let h = text.lines().find(|l| l.starts_with("Colony hash:")).map(|l| l.to_string()).unwrap_or_default();
        let c = text.lines().find(|l| l.starts_with("Colony code:")).map(|l| l.to_string()).unwrap_or_default();
        (h, c)
    };
    let cell = "76.96x48.earthlike.8.250@45,12:5800,1560";
    let (a, code_a) = hash(cell);
    let (b, _) = hash(cell);
    let (c, _) = hash("76.96x48.earthlike.8.250@45,12");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(!a.is_empty() && a == b, "the same code told two stories: {a} / {b}");
    assert!(a != c, "the cell made no difference: {a}");
    assert!(code_a.contains(cell), "the colony's code does not carry its cell: {code_a}");
}

/// Choose where to settle: the end of the history offers three livable sites that differ in
/// their threat or their settlers' people, each with who, trouble and land.
#[test]
fn three_sites_to_choose_from() {
    let mut differ = 0;
    for seed in ["76", "11", "23", "58", "3", "5"] {
        let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
            .args(["--dev", "--seed", seed, "--sites"])
            .output()
            .expect("run planet_generator");
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        let sites: Vec<&str> = text.lines().filter(|l| l.starts_with("Site ")).collect();
        assert_eq!(sites.len(), 3, "seed {seed}: not three sites:\n{text}");
        let who: Vec<&str> = text.lines().filter(|l| l.starts_with("  Seven of")).collect();
        let trouble: Vec<&str> = text.lines().filter(|l| l.starts_with("  ") && !l.starts_with("  Seven") && !l.starts_with("  Gives")).collect();
        assert!(text.lines().filter(|l| l.starts_with("  Gives ")).count() == 3, "seed {seed}: a site without its land:\n{text}");
        // Each pair of sites differs in its people or its threat.
        let pairs: std::collections::HashSet<(&str, &str)> = who.iter().zip(&trouble).map(|(w, t)| (w.split(':').next().unwrap(), t.split(':').next().unwrap())).collect();
        if pairs.len() == 3 { differ += 1; }
    }
    assert!(differ >= 5, "only {differ} of 6 seeds offer three sites that all differ in people or threat");
}
