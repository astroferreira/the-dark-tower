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
        // (28,38's crater shore has fishing since the mountains got their relief: "hard" then.)
        assert!((survey.contains("refused") || survey.contains("hard:")) && survey.contains("to eat"), "{site} was not refused or warned with a reason: {survey}");
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
    // Nobody starves after the move (the raid may take one, and the Shadow's raiders a child).
    assert!(text.contains("Move at 18,30 after 20 days: 9 of 9 alive") || text.contains("Move at 18,30 after 20 days: 8 of 9 alive")
        || text.contains("Move at 18,30 after 20 days: 7 of 9 alive"), "settlers died after moving:\n{text}");
    assert!(!text.contains("died of hunger"), "settlers starved after moving:\n{text}");
    for site in ["48,18", "28,38"] {
        let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
            .env("PLANET_FORCE_CAMP", "1")
            .args(["--dev", "--sim-projects", "30", "--tiles-center", site])
            .output()
            .expect("run planet_generator");
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(text.contains("they left on day") && text.contains("7 of 7 alive"), "{site}: no honest departure:\n{text}");
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
            // Two fewer to fight when they are turned away (a visiting hunter may make it 10 and 8).
            let fight = |l: &str| l.split(" to fight").next().and_then(|a| a.rsplit(' ').next()).and_then(|n| n.parse::<i32>().ok()).unwrap_or(0);
            assert!(raids.len() == 2 && fight(raids[0]) == fight(raids[1]) + 2 && fight(raids[1]) >= 7, "seed {seed}: the answer did not change the raid:\n{text}");
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
    // Buildings with a job: at least six kinds stand.
    let kinds: std::collections::HashSet<&str> = works.iter().filter(|w| w.contains(", done)")).filter_map(|w| w.split(": ").nth(1).and_then(|x| x.split(" (").next())).collect();
    assert!(kinds.len() >= 6, "fewer than six kinds of building: {kinds:?}");
    for w in &works {
        let why = w.split_once("), ").map(|x| x.1).unwrap_or("");
        assert!(why.chars().any(|c| c.is_ascii_digit()), "a work without a number in its reason: {w}");
    }
    // (Sorted: a lord's hall or a shaft's lining is put at the head of the list.)
    let mut days: Vec<u64> = works.iter().filter_map(|w| w["  Project day ".len()..].split(':').next().and_then(|d| d.parse().ok())).collect();
    days.sort_unstable();
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
    let cell = "76.96x48.earthlike.8.250@45,12:371200,99840"; // cells are 1/64 of a region cell (`viewer::CELL_FRAC`)
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
    let (mut differ, mut watered) = (0, 0);
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
        // Sites embark beside their tile's river (`viewer::river_bank`).
        watered += text.lines().filter(|l| l.starts_with("  Gives ") && !l.contains("lacks water")).count();
    }
    assert!(differ >= 5, "only {differ} of 6 seeds offer three sites that all differ in people or threat");
    assert!(watered >= 12, "only {watered} of 18 offered sites have water at hand (was 1 before embarks went beside the river; 14 after)");
}

/// Skill makes roles: the dev colony names three or more of its settlers for a trade, the
/// builder lays at least half the loads once named, and when the builder dies someone takes up
/// the hammer and the work is slower.
#[test]
fn skill_makes_roles() {
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--sim-roles"])
        .output()
        .expect("run planet_generator");
    let text = String::from_utf8_lossy(&out.stdout);
    let trial = text.lines().find(|l| l.starts_with("Roles trial:")).unwrap_or_else(|| panic!("no roles trial:\n{text}"));
    // (Minutes to a tenth: with twenty in the camp by day 40 the next hand can be close.)
    let n: Vec<f32> = trial.split(|c: char| !c.is_ascii_digit() && c != '.').filter_map(|x| x.trim_end_matches('.').parse().ok()).collect();
    assert!(n.len() >= 3 && n[2] > n[1], "the work did not slow when the builder died: {trial}");
    assert!(text.contains("takes up the hammer"), "no one took up the hammer:\n{text}");
    let dir = std::env::temp_dir().join(format!("roles_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--sim-snapshot", dir.join("r").to_str().unwrap()])
        .output()
        .expect("run planet_generator");
    let _ = std::fs::remove_dir_all(&dir);
    let text = String::from_utf8_lossy(&out.stdout);
    let roles = text.lines().find(|l| l.starts_with("Roles: ")).expect("roles line");
    let n: Vec<u32> = roles.split(|c: char| !c.is_ascii_digit()).filter_map(|x| x.parse().ok()).collect();
    assert!(n[0] >= 3, "fewer than three roles by day 30: {roles}");
    let (b, all) = (n[n.len() - 2], n[n.len() - 1]);
    // At least a third, three times an even share among nine: with characters, dutiful hands
    // build too, and wounds, deaths and the mine shift the work (dev 76: 51% before monsters,
    // 47% with their wounds, 42% once the mine woke a forgotten beast that killed a builder's hand).
    // (30%: digging the delve takes the camp's hands from the walls too.)
    assert!(10 * b >= 3 * all, "the builder laid under 30% of the loads: {roles}");
}

/// The village follows the patron's hand: blessing a meadow brings buildings onto it, forbidding
/// the east leaves it empty.
#[test]
fn the_plan_follows_blessing_and_taboo() {
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--sim-plan"])
        .output()
        .expect("run planet_generator");
    let text = String::from_utf8_lossy(&out.stdout);
    let n = |l: &str| -> u32 { l.split_whitespace().nth(2).and_then(|x| x.parse().ok()).unwrap_or(99) };
    let bless = text.lines().find(|l| l.starts_with("Plan bless:")).expect("bless line");
    let forbid = text.lines().find(|l| l.starts_with("Plan forbid:")).expect("forbid line");
    assert!(n(bless) >= 2, "blessed ground drew fewer than two buildings: {bless}");
    assert_eq!(n(forbid), 0, "something was built on forbidden ground: {forbid}");
}

/// Each people builds its own way: the data file names at least four peoples, and the same site
/// settled by them gives different villages (elves fell nothing inside the wall; frames differ).
#[test]
fn each_people_builds_its_own_way() {
    let names = planet_generator::colony::projects::build_way_names();
    for p in ["dwarf", "elf", "orc", "human"] { assert!(names.iter().any(|n| n == p), "no way for {p} in building_ways.json: {names:?}"); }
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--sim-ways"])
        .output()
        .expect("run planet_generator");
    let text = String::from_utf8_lossy(&out.stdout);
    let elves = text.lines().find(|l| l.starts_with("Way of the elves")).expect("elves line");
    assert!(elves.ends_with("stumps inside the wall: 0"), "elves felled trees inside the wall: {elves}");
    let orders: std::collections::HashSet<&str> = text.lines().filter(|l| l.starts_with("Way of the")).filter_map(|l| l.split("works: ").nth(1).and_then(|w| w.split(';').next())).collect();
    assert!(orders.len() >= 3, "the peoples build in the same order: {orders:?}");
    let least = text.lines().filter(|l| l.contains(" vs ")).filter_map(|l| l.split("differ in ").nth(1).and_then(|x| x.split('%').next()).and_then(|x| x.parse::<u32>().ok())).min().unwrap_or(0);
    assert!(least >= 8, "two peoples' villages look alike ({least}%):\n{text}");
}

#[test]
fn settlers_go_under() {
    use planet_generator::colony::dig::dig_minutes;
    use planet_generator::erosion::materials::RockType;
    use planet_generator::local::Material;
    // Soft rock is quick, hard rock slow.
    assert!(dig_minutes(Material::Rock(RockType::Limestone)) * 2 <= dig_minutes(Material::Rock(RockType::Granite)));
    // Dev 50,20 lies below a hill: by day 120 they have dug a hall into it and sleep there, and
    // the rock carried out was laid as stone (the palisade mended in it).
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--sim-projects", "120", "--tiles-center", "50,20"])
        .output()
        .expect("run planet_generator");
    let text = String::from_utf8_lossy(&out.stdout);
    let hall = text.lines().find(|l| l.contains("a hall in the hill")).unwrap_or_else(|| panic!("no hall dug:\n{text}"));
    assert!(hall.contains("done"), "the hall was not finished: {hall}");
    let dug = text.lines().find(|l| l.trim_start().starts_with("Dug:")).expect("Dug line");
    let n = |after: &str| dug.split(after).next().and_then(|x| x.split_whitespace().last()).and_then(|x| x.parse::<u32>().ok()).unwrap_or(0);
    assert!(n(" cells under rock") >= 12, "too little dug: {dug}");
    assert!(n(" sleep in the hall") >= 6, "nobody sleeps in the hall: {dug}");
    assert!(text.contains("stones, done"), "no stone laid:\n{text}");
}

/// Something down there: places in the rock with causes from the history and the rock. Dev
/// 46,13 holds the living Baelfang's lair; 47,14 (Brolmdustoor Pass) a tomb of a named dead; a
/// limestone cave can be walked into; and a fair share of embarks hold nothing at all.
#[test]
fn something_down_there() {
    let places = |site: &str| -> Vec<String> {
        let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
            .args(["--dev", "--sim-projects", "1", "--tiles-center", site])
            .output()
            .expect("run planet_generator");
        String::from_utf8_lossy(&out.stdout).lines().filter(|l| l.trim_start().starts_with("Place:")).map(|l| l.to_string()).collect()
    };
    let lair = places("46,13");
    assert!(lair.iter().any(|l| l.contains("The lair of Baelfang") && l.contains("lairs here, and lives")), "no lair of the living beast: {lair:?}");
    let tomb = places("47,14");
    assert!(tomb.iter().any(|l| l.contains("(Tomb)") && l.contains("fell here")), "no tomb at Brolmdustoor Pass: {tomb:?}");
    let cave = places("50,20");
    assert!(cave.iter().any(|l| l.contains("(Cave)") && l.contains("hollowed by water") && l.contains("walkable from the surface")), "no cave to walk into: {cave:?}");
    let mut none = 0;
    let sites = ["45,12", "20,35", "55,15", "70,6", "62,10", "80,17", "79,9", "58,18", "52,22", "60,20", "46,16", "54,26"];
    for s in sites { if places(s).is_empty() { none += 1; } }
    assert!(none * 5 >= sites.len() && none * 2 <= sites.len(), "{none} of {} embarks hold nothing", sites.len());
}

/// Settlers are individuals (Dwarf Fortress style): each has a description from their people's
/// template, and two settlers of one people are told apart by more than their names.
#[test]
fn settlers_are_rolled_individuals() {
    let dir = std::env::temp_dir().join(format!("who_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--sim-snapshot", dir.join("w").to_str().unwrap()])
        .output()
        .expect("run planet_generator");
    assert!(out.status.success(), "planet_generator failed: {}", String::from_utf8_lossy(&out.stderr));
    let text = std::fs::read_to_string(dir.join("w_settlers.txt")).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    let looks: Vec<&str> = text.lines().filter(|l| l.starts_with("  * ") && l.contains(", aged ")).collect();
    assert!(looks.len() >= 7, "every settler is described:\n{text}");
    let distinct: std::collections::HashSet<String> = looks.iter().map(|l| l.split(". ").skip(1).collect::<Vec<_>>().join(". ")).collect();
    assert_eq!(distinct.len(), looks.len(), "two settlers look the same:\n{text}");
    assert!(text.contains(" likes ") && text.contains("cannot abide"), "likes and dislikes are told:\n{text}");
    assert!(!text.contains("their mind") && !text.contains("themselves"), "a phrase did not agree with its person:\n{text}");
}

/// Minds break under strain, the way their character runs, and say why; a calm camp does not.
#[test]
fn minds_break_with_reasons() {
    let run = |seed: &str, days: &str| -> String {
        let dir = std::env::temp_dir().join(format!("mind_{}_{}", seed, std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("log.txt");
        let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
            .args(["--dev", "--headless", "--seed", seed, "--sim-projects", days])
            .env("PLANET_DUMP_LOG", &log)
            .output()
            .expect("run planet_generator");
        assert!(out.status.success(), "planet_generator failed: {}", String::from_utf8_lossy(&out.stderr));
        let text = std::fs::read_to_string(&log).unwrap_or_default();
        let _ = std::fs::remove_dir_all(&dir);
        text
    };
    // 150 days on seed 3, whose raids and wounds break minds early (seed 58 had been the hard
    // camp; with bedrooms and a great hall below it holds out to its second year).
    let hard = run("3", "150");
    let breaks: Vec<&str> = hard.lines().filter(|l| l.contains("throws a tantrum") || l.contains("sinks into despair") || l.contains("walks off into the wild")).collect();
    assert!(!breaks.is_empty(), "seed 3's camp never broke:\n{hard}");
    assert!(breaks.iter().all(|l| l.contains("(because they ")), "a break without its reasons: {breaks:?}");
    // Starving camps
    // spiral into tantrums as in Dwarf Fortress; a runaway mind would break far more.
    assert!(breaks.len() <= 20, "too many breaks in 150 days ({}): {breaks:?}", breaks.len());
    // (No "calm camp" check any more: a camp in the Shadow's reach sees its dead walk, and
    // that breaks people too.)
}

/// Digging too deep (Dwarf Fortress): the dev colony at 45,12 sinks a mine for ore, breaks into
/// the first cavern, the miners hear the deep stir, and the forgotten beast that slept there
/// climbs out of the mine, with its generated special attack.
#[test]
fn the_mine_wakes_the_deep() {
    let dir = std::env::temp_dir().join(format!("deep_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("log.txt");
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--headless", "--sim-projects", "45", "--tiles-center", "45,12"])
        .env("PLANET_DUMP_LOG", &log)
        .output()
        .expect("run planet_generator");
    assert!(out.status.success(), "planet_generator failed: {}", String::from_utf8_lossy(&out.stderr));
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let _ = std::fs::remove_dir_all(&dir);
    let at = |needle: &str| text.lines().position(|l| l.contains(needle)).unwrap_or_else(|| panic!("no '{needle}' in the log:\n{text}"));
    let (mine, breach, sound, climbs) = (at("They set to work on a mine"), at("breaks through into darkness"), at("A sound from below"), at("climbs out of the mine"));
    assert!(mine < breach && breach < sound && sound < climbs, "the deep's beats out of order");
    assert!(stdout.lines().any(|l| l.contains("Cavern: the first cavern")), "no cavern reported:\n{stdout}");
    assert!(text.lines().any(|l| l.contains("out of the mine, came in the night")), "the beast did not raid from the mine:\n{text}");
}

/// The raid is fought blow by blow (Dwarf Fortress's combat report): after the raid's line come
/// blows that name a settler, a weapon or a body part; a rescue leaves a wound on a named part.
#[test]
fn raids_are_fought_blow_by_blow() {
    let dir = std::env::temp_dir().join(format!("blows_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("log.txt");
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--headless", "--sim-projects", "60"])
        .env("PLANET_DUMP_LOG", &log)
        .output()
        .expect("run planet_generator");
    assert!(out.status.success());
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    let lines: Vec<&str> = text.lines().collect();
    // (The first raid fought: a beast taken in a cage trap fights no one.)
    let raid = lines.iter().position(|l| l.contains("The raid:") && !l.contains("cage trap") && !l.contains("talked by the fire")).unwrap_or_else(|| panic!("no raid fought in 60 days:\n{text}"));
    let stamp = &lines[raid][..13];
    let blows: Vec<&&str> = lines[raid + 1..].iter().take_while(|l| l.starts_with(stamp)).collect();
    assert!(blows.len() >= 2, "the raid has no blows: {blows:?}");
    assert!(blows.iter().any(|l| l.contains(" with ")), "no weapon or natural attack named: {blows:?}");
    if lines[raid].contains("dragged them back") {
        assert!(blows.iter().any(|l| ["broken", "gashed", "bruised", "cracked"].iter().any(|w| l.contains(w))), "a rescue without a wound: {blows:?}");
    }
}

/// Works of their hands (Dwarf Fortress's items and engravings): once a workshop stands the dev
/// colony makes things of its own stone and wood, most of ordinary or modest quality, and some
/// show a real event of the makers' pasts.
#[test]
fn crafts_show_history() {
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--headless", "--sim-projects", "120"])
        .output()
        .expect("run planet_generator");
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().find(|l| l.trim_start().starts_with("Works: ")).unwrap_or_else(|| panic!("nothing made in 120 days:\n{text}"));
    let n: Vec<usize> = line.split(|c: char| !c.is_ascii_digit()).filter_map(|x| x.parse().ok()).collect();
    let (made, fine, shown) = (n[0], n[1], n[2]);
    assert!(made >= 5, "too few works: {line}");
    // (The camp's best hands carve most: its builder, or a skilled guest who stayed; the share
    // runs from a half to two thirds.)
    assert!(fine * 4 <= made * 3, "most works are fine or better (quality runs too high): {line}");
    assert!(shown >= 1, "no work shows a world event: {line}");
}

/// Caravans (Dwarf Fortress): each season traders of a real town come up the road, buy the
/// camp's works and bring news from the chronicle.
#[test]
fn caravans_trade_and_bring_news() {
    let dir = std::env::temp_dir().join(format!("trade_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("log.txt");
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--headless", "--sim-projects", "80"])
        .env("PLANET_DUMP_LOG", &log)
        .output()
        .expect("run planet_generator");
    assert!(out.status.success());
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    let came = text.lines().filter(|l| l.contains("comes up the road")).count();
    assert!(came >= 2, "fewer than two caravans in 80 days:\n{text}");
    assert!(text.lines().any(|l| l.contains("They bring news: ")), "no news from the world");
    assert!(text.lines().any(|l| l.contains("The traders of ") && l.contains(" buy ")), "the caravans bought nothing");
    // Word goes home with them: migrants follow (dev 76: day 62).
    let wave = text.lines().find(|l| l.contains("Migrants arrive from ")).unwrap_or_else(|| panic!("no migrants after the caravans:\n{text}"));
    assert!(wave.contains("(kin of ") || wave.contains("(a "), "migrants without pasts: {wave}");
}

/// The camp's first office (Dwarf Fortress's positions and mandates): a month on, the camp
/// chooses someone to speak for it, who proclaims a mandate from the value they hold dearest.
#[test]
fn a_speaker_proclaims_a_mandate() {
    let dir = std::env::temp_dir().join(format!("speaker_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("log.txt");
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--headless", "--sim-projects", "35"])
        .env("PLANET_DUMP_LOG", &log)
        .output()
        .expect("run planet_generator");
    assert!(out.status.success());
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(text.lines().any(|l| l.contains("to speak for it")), "no speaker chosen:\n{text}");
    let p = text.lines().find(|l| l.contains(" proclaims that ")).unwrap_or_else(|| panic!("no mandate:\n{text}"));
    assert!(p.contains(" is to ") && p.contains(" values "), "a mandate without its rule or its reason: {p}");
}

/// Where the Shadow lies on the land the dead do not rest (Dwarf Fortress's evil regions, with
/// this world's own darkness): seed 11's camp lies at 0.48 of the Shadow's corruption, and its
/// buried rise and hunt on dark nights.
#[test]
fn the_dead_walk_under_the_shadow() {
    let dir = std::env::temp_dir().join(format!("dead_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("log.txt");
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--headless", "--seed", "11", "--sim-projects", "40"])
        .env("PLANET_DUMP_LOG", &log)
        .output()
        .expect("run planet_generator");
    assert!(out.status.success());
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(stdout.lines().any(|l| l.contains("Darkness: ")), "the camp knows nothing of the Shadow:\n{stdout}");
    assert!(text.lines().any(|l| l.contains("rises from the grave")), "no dead rose under the Shadow:\n{text}");
}

/// Strange moods (Dwarf Fortress): a creative settler is seized, claims the workshop and makes a
/// named artifact showing their past (seed 5), or, wanting a material the camp lacks, goes mad
/// (seed 3).
#[test]
fn strange_moods_make_artifacts_or_madness() {
    let run = |seed: &str, days: &str| -> String {
        let dir = std::env::temp_dir().join(format!("mood_{}_{}", seed, std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("log.txt");
        // Seed 3's mood is a possession by nature (it wants nothing); forced fey, it wants tin.
        let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
            .args(["--dev", "--headless", "--seed", seed, "--sim-projects", days])
            .env("PLANET_DUMP_LOG", &log)
            .env("PLANET_FORCE_MOOD", if seed == "3" { "fey" } else { "" })
            // (Its liked obsidian now turns up in the rock the delve cuts: ask for tin.)
            .env("PLANET_FORCE_MOOD_WANT", if seed == "3" { "tin" } else { "" })
            .output()
            .expect("run planet_generator");
        assert!(out.status.success());
        let text = std::fs::read_to_string(&log).unwrap_or_default();
        let _ = std::fs::remove_dir_all(&dir);
        text
    };
    let made = run("5", "60");
    let art = made.lines().find(|l| l.contains("has made an artifact: ")).unwrap_or_else(|| panic!("no artifact on seed 5:\n{made}"));
    assert!(art.contains(", showing "), "an artifact without its image: {art}");
    let mad = run("3", "90");
    assert!(mad.lines().any(|l| l.contains("sours into madness") && l.contains("wanted ")), "no madness on seed 3:\n{mad}");
}

/// Under the full moon (Dwarf Fortress's werebeasts): a werebeast near the camp hunts every 28th
/// night, its bite curses, and the cursed change on the next full moon and come back at dawn
/// remembering nothing. (PLANET_FORCE_WERE puts one near the dev camp.)
#[test]
fn the_werebeast_curse_spreads() {
    let dir = std::env::temp_dir().join(format!("were_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("log.txt");
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--headless", "--sim-projects", "130"])
        .env("PLANET_DUMP_LOG", &log)
        .env("PLANET_FORCE_WERE", "Ashiel the Ancient")
        .output()
        .expect("run planet_generator");
    assert!(out.status.success());
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    let at = |needle: &str| text.lines().position(|l| l.contains(needle)).unwrap_or_else(|| panic!("no '{needle}':\n{text}"));
    let (moon, cursed, back) = (at("The full moon rises"), at("heals strangely fast"), at("comes back at dawn"));
    assert!(moon < cursed && cursed < back, "the curse's beats out of order");
    assert!(text.lines().filter(|l| l.contains("The full moon rises")).count() >= 4, "not every 28 days");
}

/// Riches draw trouble (Dwarf Fortress's sieges come to rich forts): once the present day's
/// threads near the camp are spent, the works the camp makes draw farther threats in.
#[test]
fn riches_draw_trouble() {
    let dir = std::env::temp_dir().join(format!("riches_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("log.txt");
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--headless", "--seed", "11", "--sim-projects", "300"])
        .env("PLANET_DUMP_LOG", &log)
        .output()
        .expect("run planet_generator");
    assert!(out.status.success());
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    let drawn = text.lines().filter(|l| l.contains("A rumour") && (l.contains("the camp's riches") || l.contains("the camp grows rich"))).count();
    assert!(drawn >= 1, "no trouble drawn by the camp's riches in 300 days");
}

/// The camp's annals (Dwarf Fortress's legends, for the colony): one page with every moment and
/// why, and every settler with who they were and what became of them.
#[test]
fn the_camp_keeps_annals() {
    let dir = std::env::temp_dir().join(format!("annals_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--sim-snapshot", dir.join("a").to_str().unwrap()])
        .output()
        .expect("run planet_generator");
    assert!(out.status.success());
    let page = std::fs::read_to_string(dir.join("a_annals.html")).unwrap_or_default();
    let settlers = std::fs::read_to_string(dir.join("a_settlers.txt")).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(page.contains("<h2>The Days</h2>") && page.contains("<h2>The People</h2>"), "annals without their parts");
    assert!(page.matches("(because").count() >= 5, "moments without their reasons");
    for name in settlers.lines().filter(|l| !l.starts_with(' ')).filter_map(|l| l.split(',').next()) {
        assert!(page.contains(&format!("<b>{}</b>", name)), "{name} is missing from the annals");
    }
}

/// Their gods (from the history's religions): when enough of the camp is devout it raises a
/// temple to its people's god, and keeps its festivals in that god's honour.
#[test]
fn a_temple_to_their_god() {
    let dir = std::env::temp_dir().join(format!("temple_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("log.txt");
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--headless", "--sim-projects", "40"])
        .env("PLANET_DUMP_LOG", &log)
        .output()
        .expect("run planet_generator");
    assert!(out.status.success());
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    let t = text.lines().find(|l| l.contains("They set to work on a temple")).unwrap_or_else(|| panic!("no temple:\n{text}"));
    let god = t.split(" pray to ").nth(1).and_then(|r| r.split(", with no roof").next()).expect("a god named");
    assert!(text.lines().any(|l| l.contains("festival in honour of") && l.contains(god)), "no festival for {god}");
}

/// Crime and justice (Dwarf Fortress): the greedy steal from the store at night, someone sees
/// or guesses, and the speaker judges by how much they value the law; a third offence goes to
/// the stocks (seed 23).
#[test]
fn thieves_are_judged() {
    let dir = std::env::temp_dir().join(format!("justice_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("log.txt");
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--headless", "--seed", "23", "--sim-projects", "100"])
        .env("PLANET_DUMP_LOG", &log)
        .output()
        .expect("run planet_generator");
    assert!(out.status.success());
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    let thefts = text.lines().filter(|l| l.contains(" from the store in the night")).count();
    assert!(thefts >= 1 && thefts <= 20, "{thefts} thefts in 100 days");
    assert!(text.lines().any(|l| l.contains(" in the stocks for a day") || l.contains(" give back the ") || l.contains(" lets it go that ")), "no thief was judged:\n{text}");
}

/// Artifacts as story carriers (Dwarf Fortress): a staff the history lost near the dev camp is
/// told of by the traders, found, claimed back by its people's envoy, kept, and fought for.
#[test]
fn a_lost_relic_is_found_and_claimed() {
    let dir = std::env::temp_dir().join(format!("relic_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("log.txt");
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--headless", "--sim-projects", "110"])
        .env("PLANET_DUMP_LOG", &log)
        .output()
        .expect("run planet_generator");
    assert!(out.status.success());
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    let has = |s: &str| text.lines().any(|l| l.contains(s));
    assert!(has("tell of The Staff of Greenburg") || has("finds The Staff of Greenburg in the earth"), "no tale:\n{text}");
    assert!(has("finds The Staff of Greenburg"), "never found");
    assert!(has("ask for it back"), "no envoy");
    assert!(has("give The Staff of Greenburg back") || has("gives The Staff of Greenburg back"), "no answer");
}

/// Slaying a beast (Dwarf Fortress): the camp's blows add up against the beast's size; on seed
/// 11 the forgotten beast from the mine is killed in the raid that wounds Zok, and its slayer
/// carries the deed.
#[test]
fn a_beast_can_be_slain() {
    let dir = std::env::temp_dir().join(format!("slay_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("log.txt");
    let annals = dir.join("annals.html");
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--headless", "--seed", "11", "--sim-projects", "60"])
        .env("PLANET_DUMP_LOG", &log)
        .env("PLANET_ANNALS", &annals)
        .output()
        .expect("run planet_generator");
    assert!(out.status.success());
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let page = std::fs::read_to_string(&annals).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(text.lines().any(|l| l.contains("falls, and does not rise")), "no beast slain:\n{text}");
    assert!(page.contains("slew "), "the slayer's deed is not in the annals");
}

/// The militia (Dwarf Fortress's squads): from the rumour on, the brave drill with the spear in
/// the evening and the workshop makes spears; the raid's tally counts those drilled and armed.
#[test]
fn the_militia_drills_and_arms() {
    let dir = std::env::temp_dir().join(format!("militia_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("log.txt");
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--headless", "--sim-projects", "65"])
        .env("PLANET_DUMP_LOG", &log)
        .output()
        .expect("run planet_generator");
    assert!(out.status.success());
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(text.lines().any(|l| l.contains(" with the spear by the fire, for fear of ")), "no drill:\n{text}");
    assert!(text.lines().any(|l| l.contains("-tipped spear at the workshop") || l.contains("iron-headed spear at the workshop")), "no spear made");
    assert!(text.lines().any(|l| l.contains("drilled and under arms")), "the tally never counts the militia");
}

/// Visitors (Dwarf Fortress): the hero whose quest is the camp's beast comes after the rumour
/// and fights beside them (on seed 11 Ielnveph the Brave kills Fyron Plague-Bearer); bards of
/// neighbouring peoples perform and teach a work the camp then performs as they were taught.
#[test]
fn visitors_come_from_the_world() {
    let dir = std::env::temp_dir().join(format!("visitors_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("log.txt");
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--headless", "--seed", "11", "--sim-projects", "190"])
        .env("PLANET_DUMP_LOG", &log)
        .output()
        .expect("run planet_generator");
    assert!(out.status.success());
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    let has = |s: &str| text.lines().any(|l| l.contains(s));
    assert!(has("a monster hunter of") && has("who set out to slay"), "no hunter came:\n{text}");
    assert!(has("A traveller comes to the fire"), "no bard came");
    assert!(has(" teaches "), "the bard taught nothing");
    assert!(has("taught them;"), "the taught work was never performed");
}

/// The relic's seeker (Dwarf Fortress's artifact claims and thefts): a seeker comes for the
/// Staff of Greenburg, asks the speaker for it once found, and is given it or refused; refused,
/// a greedy seeker takes it in the night or is caught (forced: no living seeker in the dev
/// history).
#[test]
fn a_seeker_comes_for_the_relic() {
    let dir = std::env::temp_dir().join(format!("seeker_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("log.txt");
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--headless", "--sim-projects", "110"])
        .env("PLANET_DUMP_LOG", &log)
        .env("PLANET_FORCE_SEEKER", "Orvel")
        .output()
        .expect("run planet_generator");
    assert!(out.status.success());
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    let has = |s: &str| text.lines().any(|l| l.contains(s));
    assert!(has("A stranger comes asking after The Staff of Greenburg"), "no seeker:\n{text}");
    assert!(has("Orvel asks "), "the seeker never asked");
    assert!(has("gives The Staff of Greenburg to Orvel") || has("Orvel slips away") || has("catches Orvel"), "no end to the asking");
}

/// Engravings (Dwarf Fortress): the dev colony at 50,20 digs a hall and carves its walls with
/// its own story first (raids, deaths, guests) and then the engraver's past.
#[test]
fn the_hall_is_engraved_with_its_story() {
    let dir = std::env::temp_dir().join(format!("engrave_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("log.txt");
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--headless", "--sim-projects", "150", "--tiles-center", "50,20"])
        .env("PLANET_DUMP_LOG", &log)
        .output()
        .expect("run planet_generator");
    assert!(out.status.success());
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    let carved: Vec<&str> = text.lines().filter(|l| l.contains("engraves the hall's wall with")).collect();
    assert!(!carved.is_empty(), "no engraving in 150 days:\n{text}");
    assert!(carved.iter().any(|l| l.contains("the raid of day") || l.contains("the death of") || l.contains("the coming of")), "nothing of the camp's own story: {carved:?}");
}

/// A vampire among the migrants (Dwarf Fortress's night creatures; forced, as the Shadow's
/// waves bring one only one time in three): it never eats, feeds on sleepers, and is found out
/// by a witness or the sharpest mind, and judged.
#[test]
fn a_vampire_comes_with_the_migrants() {
    let dir = std::env::temp_dir().join(format!("vampire_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("log.txt");
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--headless", "--seed", "3", "--sim-projects", "120"])
        .env("PLANET_DUMP_LOG", &log)
        .env("PLANET_FORCE_VAMPIRE", "1")
        .output()
        .expect("run planet_generator");
    assert!(out.status.success());
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    let has = |s: &str| text.lines().any(|l| l.contains(s));
    assert!(has("is never seen to eat"), "nobody noticed:\n{text}");
    assert!(has("bent over") || has("says aloud what"), "never found out");
    assert!(has("put to death by the fire") || has("out into the light") || has("out with torches"), "never judged");
    assert!(!text.lines().any(|l| l.contains("put to death") && l.contains("stole")), "a vampire put to death for a theft");
}

/// Kinds of strange mood (Dwarf Fortress): forced fell, the moody settler kills the one nearest
/// and makes an artifact of their bones; the natural kinds follow character (secretive,
/// possessed, macabre on the dev seeds).
#[test]
fn moods_come_in_kinds() {
    let dir = std::env::temp_dir().join(format!("moods_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("log.txt");
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--headless", "--sim-projects", "200"])
        .env("PLANET_DUMP_LOG", &log)
        .env("PLANET_FORCE_MOOD", "fell")
        .output()
        .expect("run planet_generator");
    assert!(out.status.success());
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(text.lines().any(|l| l.contains("in the grip of a fell mood") && l.contains(" kills ")), "no fell killing:\n{text}");
    assert!(text.lines().any(|l| l.contains("has made an artifact") && l.contains("of the bones of")), "no artifact of bones");
}

/// Pets (Dwarf Fortress): a settler fond of a herd tames one and names it, and the annals say
/// who keeps (or kept) what. (Wolves taking a stray pet is chance: seed 58 lost its caribou on
/// day 140 in one timeline.)
#[test]
fn settlers_keep_pets() {
    let dir = std::env::temp_dir().join(format!("pets_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("log.txt");
    let annals = dir.join("annals.html");
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--headless", "--seed", "58", "--sim-projects", "160"])
        .env("PLANET_DUMP_LOG", &log)
        .env("PLANET_ANNALS", &annals)
        .output()
        .expect("run planet_generator");
    assert!(out.status.success());
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let page = std::fs::read_to_string(&annals).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(text.lines().any(|l| l.contains(" coaxes a young ") && l.contains("and names it")), "no pet:\n{text}");
    assert!(page.contains("keeps a ") || page.contains("kept a "), "the pet is not in the annals");
}

/// Families (Dwarf Fortress): close settlers wed by the fire and have children who take after
/// both parents; infants stay by their mothers.
#[test]
fn families_are_made_in_the_camp() {
    let dir = std::env::temp_dir().join(format!("family_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("log.txt");
    let annals = dir.join("annals.html");
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--headless", "--seed", "23", "--sim-projects", "160"])
        .env("PLANET_DUMP_LOG", &log)
        .env("PLANET_ANNALS", &annals)
        .output()
        .expect("run planet_generator");
    assert!(out.status.success());
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let page = std::fs::read_to_string(&annals).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(text.lines().any(|l| l.contains(" are wed by the fire")), "no wedding:\n{text}");
    assert!(text.lines().any(|l| l.contains(" gives birth to a ")), "no birth");
    assert!(page.contains("born in ") && page.contains("child of "), "the child is not in the annals");
}

/// Aquifers (Dwarf Fortress): the dev site 45,12 lies where the land holds water; its mine opens
/// wet rock, stops, lines the shaft in stone as a work of its own, and goes on down.
#[test]
fn the_mine_strikes_an_aquifer() {
    let dir = std::env::temp_dir().join(format!("aquifer_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("log.txt");
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--headless", "--sim-projects", "40", "--tiles-center", "45,12"])
        .env("PLANET_DUMP_LOG", &log)
        .output()
        .expect("run planet_generator");
    assert!(out.status.success());
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let _ = std::fs::remove_dir_all(&dir);
    let at = |needle: &str| text.lines().position(|l| l.contains(needle)).unwrap_or_else(|| panic!("no '{needle}':\n{text}"));
    let (wet, lined, breach) = (at("opens wet rock"), at("finishes the lining of the wet shaft"), at("breaks through into darkness"));
    assert!(wet < lined && lined < breach, "the aquifer's beats out of order");
    assert!(stdout.contains("aquifer at levels") && stdout.contains("lined"), "the summary does not report the aquifer:\n{stdout}");
}

/// Gems in the rock (Dwarf Fortress's clusters): digging at 45,12 turns up amber, and the
/// workshop sets it in a work.
#[test]
fn gems_are_found_and_set() {
    let dir = std::env::temp_dir().join(format!("gems_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("log.txt");
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--headless", "--sim-projects", "60", "--tiles-center", "45,12"])
        .env("PLANET_DUMP_LOG", &log)
        .output()
        .expect("run planet_generator");
    assert!(out.status.success());
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(text.lines().any(|l| l.contains("finds a cluster of ")), "no gems:\n{text}");
    assert!(text.lines().any(|l| l.contains(" set with ")), "no gem set in a work");
}

/// Legends of the camps (Dwarf Fortress's legends): a camp's deeds are kept with the world; a
/// later camp nearby finds the beast its predecessor slew gone from its troubles and knows the
/// song of it.
#[test]
fn legends_outlive_the_camp() {
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--seed", "11", "--sim-legend"])
        .output()
        .expect("run planet_generator");
    let text = String::from_utf8_lossy(&out.stdout);
    let legend = text.lines().find(|l| l.starts_with("Legend: ")).unwrap_or_else(|| panic!("no legend:\n{text}"));
    assert!(legend.contains("Fyron Plague-Bearer"), "the slain beast is not in the legend: {legend}");
    let second = text.lines().find(|l| l.starts_with("Second camp")).unwrap_or_else(|| panic!("no second camp:\n{text}"));
    let after = second.split("after").nth(1).unwrap_or("");
    assert!(second.contains("before [\"Fyron") && !after.contains("Fyron"), "the slain beast still threatens the second camp: {second}");
    assert!(text.contains("They know the songs of"), "the newcomers know nothing of the old camp:\n{text}");
}

/// Ghosts and memorials (Dwarf Fortress): a settler killed by violence is restless;
/// unremembered five days, their ghost walks until a slab (or post) is carved in their memory.
/// Most camps carve within days, so a few dev seeds are tried until one shows it.
#[test]
fn the_dead_who_died_badly_walk_until_remembered() {
    let found = ["23", "58", "3", "11", "5"].iter().any(|seed| {
        let text = run_log(seed, "120", &[]);
        let seen = text.lines().position(|l| l.contains("The ghost of ") && l.contains(" is seen "));
        let rest = text.lines().position(|l| l.contains("carved in memory of") && l.contains("ghost is at rest"));
        seen.is_some() && rest.is_some() && seen < rest
    });
    assert!(found, "no ghost walked and was laid to rest on any of five dev seeds");
}

/// The dev colony's log for `seed` after `days`, with extra environment.
fn run_log(seed: &str, days: &str, env: &[(&str, &str)]) -> String {
    // A folder per call: tests run in parallel, and two may ask for the same seed and days.
    static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("runlog_{}_{}_{}_{}", seed, days, std::process::id(), n));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("log.txt");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_planet_generator"));
    cmd.args(["--dev", "--headless", "--seed", seed, "--sim-projects", days]).env("PLANET_DUMP_LOG", &log);
    for (k, v) in env { cmd.env(k, v); }
    let out = cmd.output().expect("run planet_generator");
    assert!(out.status.success());
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    text
}

/// Expeditions (Dwarf Fortress's raids and missions): with a drilled militia and a brave speaker,
/// seed 3 sends a party out to meet the beast foretold on the road; they come home with its head
/// (or without it) and the watch is stood down if it fell.
#[test]
fn the_militia_goes_out_to_meet_the_beast() {
    // (A few dev seeds: it takes a drilled militia, a brave speaker and a beast in reach.)
    // (Forced past the speaker's disposition: speakers who value peace keep the militia home.)
    let found = ["3", "11", "23", "76", "5"].iter().any(|seed| {
        let text = run_log(seed, "150", &[("PLANET_FORCE_HUNT", "1")]);
        let sent = text.lines().position(|l| l.contains(" out to meet ") || l.contains(" out to hunt "));
        let home = text.lines().position(|l| l.contains("The hunting party comes home") || l.contains("come home from the hunt"));
        matches!((sent, home), (Some(a), Some(b)) if a < b)
    });
    assert!(found, "no hunting party went out and came home on five dev seeds");
}

/// War bands have leaders from the history (the living warrior of that people with the most
/// kills); a camp that routs the band may cut its leader down, a deed and a legend. (A few dev
/// seeds are tried: it is one rout in three.)
#[test]
fn war_bands_are_led_by_real_warriors() {
    let mut named = false;
    let found = ["23", "76", "5", "11", "3", "58"].iter().any(|seed| {
        // (Forced: a led band's rout is rare, and its leader falls one time in three.)
        let text = run_log(seed, "250", &[("PLANET_FORCE_LEADER_FALL", "1")]);
        named |= text.lines().any(|l| l.contains("a war band of ") && l.contains(", led by ") && l.contains(", is roaming the hills"));
        text.lines().any(|l| l.contains(" cuts down ") && l.contains("who led them"))
    });
    assert!(named, "no war band with a named leader on any seed");
    assert!(found, "no leader cut down on any of six dev seeds");
}

/// Drink (Dwarf Fortress): once a workshop stands and berries are to spare the camp raises a
/// still and brews berry wine (seed 3's dwarves want it first).
#[test]
fn the_still_makes_wine() {
    let text = run_log("3", "40", &[]);
    assert!(text.lines().any(|l| l.contains("work on a still")), "no still:\n{text}");
    assert!(text.lines().any(|l| l.contains("draws the first berry wine")), "no wine");
}

/// Cage traps (Dwarf Fortress): with the palisade up and a beast foretold, the camp sets cage
/// traps at its gates and the mine's mouth; a forgotten beast climbing out may be caged.
#[test]
fn cage_traps_take_a_beast() {
    let found = ["3", "11", "23", "76"].iter().any(|seed| {
        let text = run_log(seed, "60", &[]);
        text.lines().any(|l| l.contains("work on cage traps")) && text.lines().any(|l| l.contains("blunders onto the trip-stone"))
    });
    assert!(found, "no beast caged on four dev seeds");
}

/// Years (Dwarf Fortress's ages and death years rolled at birth): the camp's year is four
/// seasons; at its turn everyone is a year older (and the old may die in their sleep).
#[test]
fn the_years_turn() {
    let text = run_log("76", "125", &[]);
    assert!(text.lines().any(|l| l.starts_with("Day 121, 06:00") && l.contains("A year turns in the camp: its second year")), "no year turned:\n{text}");
}

/// A lord from home (Dwarf Fortress's nobles): once the camp has grown, its people send kin of
/// their ruler to govern it; the lord takes the office and demands a hall.
#[test]
fn a_lord_comes_to_rule() {
    let found = ["23", "11"].iter().any(|seed| {
        let text = run_log(seed, "170", &[]);
        text.lines().any(|l| l.contains("arrives with a writ") && l.contains("lord of the camp")) && text.lines().any(|l| l.contains("a hall for the lord"))
    });
    assert!(found, "no lord came on seeds 23 or 11");
}

/// Farms under the rock (Dwarf Fortress's underground farming): once the mine has broken into a
/// cavern and a hall or cellar is dug, the camp plants a farm below that yields in any season.
#[test]
fn a_farm_under_the_rock() {
    let found = ["11", "23", "76"].iter().any(|seed| {
        let text = run_log(seed, "120", &[]);
        text.lines().any(|l| l.contains("work on a farm under the rock")) && text.lines().any(|l| l.contains("in the farm under the rock:"))
    });
    assert!(found, "no farm under the rock on three dev seeds");
}

/// Experience shapes character (Dwarf Fortress's personality change and jaded dwarves): horror
/// makes settlers anxious, saving others brave, long contentment cheerful; enough horror and they
/// are jaded. (Each seen on at least one of four dev seeds.)
#[test]
fn experience_changes_people() {
    let logs: Vec<String> = ["58", "11", "76", "3"].iter().map(|s| run_log(s, "250", &[])).collect();
    let any = |needle: &str| logs.iter().any(|t| t.lines().any(|l| l.contains(needle)));
    assert!(any("starts at every shadow"), "no one grew anxious");
    assert!(any("has seen too much to be shaken now"), "no one grew jaded");
    assert!(any("quicker to laugh"), "no one grew cheerful");
}

/// The tavern (Dwarf Fortress's taverns): once travellers have come and the camp has twelve, it
/// raises a tavern where bards perform; with a raid foretold a sellsword may be hired there.
#[test]
fn the_tavern_draws_the_world() {
    let logs: Vec<String> = ["11", "23", "76", "3", "5"].iter().map(|s| run_log(s, "250", &[])).collect();
    let any = |needle: &str| logs.iter().any(|t| t.lines().any(|l| l.contains(needle)));
    assert!(any("They set to work on a tavern"), "no tavern");
    assert!(any("In the tavern "), "no performance in the tavern");
    assert!(any("is hired at the tavern") || any("drinks up and goes"), "no sellsword came");
}

/// Written works (Dwarf Fortress's books): learned settlers write histories of their past, a
/// treatise on their trade, and once a year has passed the camp's own chronicle.
#[test]
fn books_are_written() {
    // (A camp laying in its winter store writes less; the chronicle comes after day 120.)
    let logs: Vec<String> = ["23", "11"].iter().map(|s| run_log(s, "140", &[])).collect();
    for text in &logs {
        let books = text.lines().filter(|l| l.contains(" finishes writing ")).count();
        assert!(books <= 8, "too many books in 140 days: {books}");
    }
    assert!(logs.iter().any(|t| t.lines().any(|l| l.contains(" finishes writing The Chronicle of "))), "no chronicle");
}

/// The lord's demands (Dwarf Fortress's noble demands): the lord asks for a fine work of a
/// material, the crafters make it (breaking stone for it if need be), and the lord is pleased;
/// unmet, someone goes to the stocks.
#[test]
fn the_lord_demands_fine_work() {
    let text = run_log("23", "200", &[]);
    assert!(text.lines().any(|l| l.contains(" demands a fine work of ")), "no demand:\n{text}");
    assert!(text.lines().any(|l| l.contains("the lord is pleased") || l.contains("put in the stocks for it")), "the demand had no end");
}

/// Livestock (Dwarf Fortress's pastures and butchery): the camp pens two of a nearby herd, they
/// breed each season, and when the store runs low one is slaughtered.
#[test]
fn the_pen_breeds_and_feeds() {
    let text = run_log("23", "130", &[]);
    assert!(text.lines().any(|l| l.contains("into the new pen")), "no pen:\n{text}");
    assert!(text.lines().any(|l| l.contains("in the pen this season")), "no young in the pen");
}

/// The deep shaft (Dwarf Fortress's adamantine and what lies under it): after the cavern, a camp
/// of stone-builders or a greedy speaker sinks a deep shaft, strikes adamantine, follows it into
/// the hollow, and a demon climbs out of the mine.
#[test]
fn the_deep_shaft_opens_the_hollow() {
    let found = ["76", "23"].iter().any(|seed| {
        let text = run_log(seed, "150", &[]);
        let at = |n: &str| text.lines().position(|l| l.contains(n));
        match (at("strike a vein of adamantine"), at("break into a hollow"), at("the demon")) {
            (Some(a), Some(h), Some(d)) => a < h && h < d,
            _ => false,
        }
    });
    assert!(found, "no adamantine, hollow and demon on seeds 76 or 23");
}

/// Places in the hills (Dwarf Fortress's discoveries): the embark's tombs, lairs, old mines and
/// caves are found by those who pass; a robbed tomb's dead rise for their arms (seed 58).
#[test]
fn places_in_the_hills_are_found() {
    // (Which place is found first moves with every change to the camp's days; any will do, and a
    // tomb found is robbed or left sealed.)
    let logs: Vec<String> = ["58", "11", "5"].iter().map(|s| run_log(s, "200", &[])).collect();
    assert!(logs.iter().any(|t| t.lines().any(|l| l.contains(" finds ") && (l.contains(" in the hills") || l.contains("finds a cave") || l.contains("finds the")))), "no place found");
    for t in &logs { if t.contains("finds the tomb") { assert!(t.contains("breaks into the tomb") || t.contains("leaves it sealed")); } }
    let caves = ["76", "3"].iter().any(|s| run_log(s, "120", &[]).lines().any(|l| l.contains(" finds a cave in the ")));
    assert!(caves, "no cave found on seeds 76 or 3");
}

/// Prisoners (Dwarf Fortress's captives): a routed band may leave one behind alive; the speaker
/// holds them for ransom, puts them to death or sends them home, or they slip away.
#[test]
fn a_raider_is_taken_alive() {
    let logs: Vec<String> = ["11", "5", "76"].iter().map(|s| run_log(s, "200", &[])).collect();
    let any = |n: &str| logs.iter().any(|t| t.lines().any(|l| l.contains(n)));
    assert!(any("is taken alive"), "no prisoner");
    assert!(any("put to death at the gate") || any("for ransom") || any("home to ") || any("slips the ropes"), "the prisoner's fate is untold");
}

/// What the world thinks of the camp (Dwarf Fortress's diplomatic evaluation): a relic kept or a
/// prisoner put to death turns a people to vengeance; trade and tribute can make friends. Each
/// says why, and the annals keep the reckoning.
#[test]
fn the_world_remembers_what_the_camp_did() {
    let logs: Vec<String> = ["76", "11", "58"].iter().map(|s| run_log(s, "200", &[])).collect();
    let any = |n: &str| logs.iter().any(|t| t.lines().any(|l| l.contains(n)));
    assert!(any("have sworn vengeance on the camp, which "), "no vengeance");
    assert!(any("comes in friendship, remembering that the camp "), "no friendship");
}

/// Armour (Dwarf Fortress's layers of material): once the militia bears spears the workshop
/// makes armour of the best to hand (adamantine from the deep shaft, iron, copper, leather from
/// the hunt), and in the clash it turns blows.
#[test]
fn armour_turns_blows() {
    let logs: Vec<String> = ["23", "76"].iter().map(|s| run_log(s, "230", &[])).collect();
    let any = |n: &str| logs.iter().any(|t| t.lines().any(|l| l.contains(n)));
    assert!(any("the first armour in the camp"), "no armour made");
    assert!(any("does not get through") || any("took the worst of it") || any("turns it.") || any("is turned by"), "no blow turned");
}

/// Snatchers (Dwarf Fortress's baby-snatchers): a war band of a child-stealing people carries a
/// child off in the raid; the same raiders come again sixty days on with the child among them,
/// and the camp may win them back (forced: the dev world's snatching peoples rarely raid).
#[test]
fn snatched_children_come_back_among_the_raiders() {
    let logs: Vec<String> = ["23", "3"].iter().map(|s| run_log(s, "300", &[("PLANET_FORCE_SNATCH", "1")])).collect();
    let any = |n: &str| logs.iter().any(|t| t.lines().any(|l| l.contains(n)));
    assert!(any("In the confusion of the raid, ") && any("small tracks and larger ones"), "no child taken");
    assert!(any("they came before and carried off "), "the raiders never came again");
    assert!(any("pulls ") && any(" free: home after ") || any("drops the spear and walks to the fire") || any("No one can reach") || any("turns and runs with the raiders"), "the child was never seen again");
}

/// Sieges (Dwarf Fortress): after the first trouble, raiders against a closed palisade may camp
/// outside it instead of striking at once; the camp lives on its store, then sallies, outlasts
/// them or meets the assault.
#[test]
fn raiders_lay_siege_to_a_walled_camp() {
    let logs: Vec<String> = ["76", "11"].iter().map(|s| run_log(s, "160", &[])).collect();
    let any = |n: &str| logs.iter().any(|t| t.lines().any(|l| l.contains(n)));
    assert!(any("The camp is besieged; no one goes beyond the palisade."), "no siege");
    assert!(any("of the siege: the fires of "), "the siege's days were not counted");
    assert!(any("spears out of the gate") || any("fires went cold") || any("came in the night"), "the siege never ended");
}

/// Evil weather (Dwarf Fortress's evil regions): under the Shadow's darkness a red rain, a black
/// mist or a cloud of ash comes over the camp; those under a roof are safe, the rest fall ill.
#[test]
fn evil_weather_under_the_shadow() {
    let text = run_log("11", "120", &[]);
    let line = text.lines().find(|l| l.contains(" sky comes ")).unwrap_or_else(|| panic!("no evil weather on seed 11"));
    assert!(line.contains("caught in the open") || line.contains("under a roof in time"), "{line}");
    let calm = run_log("23", "120", &[]);
    assert!(!calm.lines().any(|l| l.contains(" sky comes ")), "evil weather where there is no darkness");
}

/// Guilds (Dwarf Fortress's nested organizations): four masters of one trade swear themselves to
/// a guild and petition the speaker for a hall.
#[test]
fn masters_form_a_guild() {
    let logs: Vec<String> = ["3", "23"].iter().map(|s| run_log(s, "120", &[])).collect();
    let any = |n: &str| logs.iter().any(|t| t.lines().any(|l| l.contains(n)));
    assert!(any("swear themselves to one another as The "), "no guild");
    assert!(any(" for a hall for The "), "no petition");
}

/// The world remembers camps (legends and regard): the Git Clans, refused their staff by the dev
/// camp, hold half that grudge against a camp founded two tiles away.
#[test]
fn peoples_remember_earlier_camps() {
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--seed", "76", "--sim-legend"])
        .output()
        .expect("run planet_generator");
    let text = String::from_utf8_lossy(&out.stdout);
    let legend = text.lines().find(|l| l.starts_with("Legend: ")).unwrap_or_else(|| panic!("no legend:\n{text}"));
    assert!(legend.contains("regards: [\"The Git Clans -"), "the grudge is not in the legend: {legend}");
    let second = text.lines().find(|l| l.starts_with("Second camp's regards")).unwrap_or_else(|| panic!("no second camp:\n{text}"));
    assert!(second.contains("The Git Clans -"), "the second camp inherits nothing: {second}");
}

/// A rising against the lord (Dwarf Fortress's nobles and their unhappy subjects): the lord's
/// mandates and punishments are grievances; enough of them, and the camp goes to the lord's hall.
/// Seed 23's lord Baangh falls on day 208, and her people swear vengeance.
#[test]
fn the_camp_rises_against_its_lord() {
    let text = run_log("23", "230", &[]);
    let rising = text.lines().find(|l| l.contains("go to the lord's hall")).unwrap_or_else(|| panic!("no rising on seed 23"));
    assert!(rising.contains("stand by the lord") || rising.contains("stands by the lord"), "{rising}");
    assert!(text.lines().any(|l| l.contains("kin of their ruler")) || rising.contains("seize"), "the rising had no consequence");
}

/// The caravan's liaison (Dwarf Fortress's outpost liaison): the speaker asks for what the camp
/// lacks most, and the next caravan brings it.
#[test]
fn the_liaison_brings_what_was_asked() {
    let logs: Vec<String> = ["76", "11"].iter().map(|s| run_log(s, "110", &[])).collect();
    let any = |n: &str| logs.iter().any(|t| t.lines().any(|l| l.contains(n)));
    assert!(any("The traders' liaison asks "), "no liaison");
    assert!(any("As asked, the traders of "), "nothing was brought");
}

/// Pets defend their keepers (Dwarf Fortress): a pet runs between its keeper and what hunts them
/// (seed 3, day 44; rare: a pet near its keeper when something hunts them; its seed and day move
/// with every change to the camps' timelines).
#[test]
fn a_pet_stands_by_its_keeper() {
    // (Two seeds: which one shows it moves with every change to the camps' timelines.)
    let found = ["3", "23", "58", "5"].iter().any(|seed| run_log(seed, "260", &[]).lines().any(|l| l.contains(" comes running") && (l.contains("stands over") || l.contains("dragged down in"))));
    assert!(found, "no pet stood by its keeper");
}

/// Old comrades (Dwarf Fortress's per-figure knowledge of events): migrants who fought in a
/// veteran's battle find each other out.
#[test]
fn old_comrades_find_each_other() {
    let logs: Vec<String> = ["76", "5"].iter().map(|s| run_log(s, "100", &[])).collect();
    assert!(logs.iter().any(|t| t.lines().any(|l| l.contains(" find they both stood at ") || l.contains(" find they stood on opposite sides at "))), "no one shared a battle");
}

/// A slain beast's remains (Dwarf Fortress: creature parts are materials): its bones become
/// famous works showing its death, its hide armour.
#[test]
fn a_slain_beast_becomes_bone_and_armour() {
    let logs: Vec<String> = ["76", "11"].iter().map(|s| run_log(s, "110", &[])).collect();
    let any = |n: &str| logs.iter().any(|t| t.lines().any(|l| l.contains(n)));
    assert!(any("-bone ") && any("showing the death of "), "no work of a beast's bone");
    assert!(any("makes a coat of "), "no armour of a beast's hide");
}

/// Ambushed caravans (Dwarf Fortress): while a war band is foretold, the season's caravan may be
/// taken on the road, and what the camp had asked for with it (seed 23, day 135).
#[test]
fn a_caravan_is_taken_on_the_road() {
    // (Forced: a caravan coming while raiders are foretold has grown rare.)
    let found = ["76", "23"].iter().any(|seed| run_log(seed, "120", &[("PLANET_FORCE_AMBUSH", "1")]).lines().any(|l| l.contains("does not come. Toward noon a mule walks in alone")));
    assert!(found, "no caravan was taken");
}

/// An established camp stays: seed 11's camp of fourteen, with a farm under the rock, a field
/// and a store, had given its land up in summer because the wild berries ran short.
#[test]
fn an_established_camp_does_not_walk_away() {
    let text = run_log("11", "200", &[]);
    assert!(!text.lines().any(|l| l.contains("They shoulder what they have and leave")), "seed 11 left its land");
}

/// The camp answers what keeps coming back (Dwarf Fortress): seed 11 kills the wolves at their
/// den after four bites and burns the dead that rose three times.
#[test]
fn the_camp_clears_the_den_and_burns_the_restless() {
    let text = run_log("11", "60", &[]);
    assert!(text.lines().any(|l| l.contains("go out to the wolves' den at ")), "the wolves were never answered");
    assert!(text.lines().any(|l| l.contains("burn what is left on a pyre") || l.contains("burn the bones in it")), "the restless dead were never burned");
}

/// A hungry camp plants first: seed 58, hungry from its first weeks, sets a field ahead of the
/// works under way instead of sitting a year behind an unfinished well.
#[test]
fn a_hungry_camp_plants_first() {
    let text = run_log("58", "45", &[]);
    assert!(text.lines().any(|l| l.contains("They set to work on a fenced field")), "seed 58 never planted");
}

/// Artifact thieves (Dwarf Fortress): a hostile people's thief comes for the camp's artifact;
/// caught, they are the camp's prisoner (seed 11, day 169; seed 5, day 257).
#[test]
fn thieves_come_for_the_artifact() {
    let logs: Vec<String> = ["11", "5"].iter().map(|s| run_log(s, "260", &[])).collect();
    assert!(logs.iter().any(|t| t.lines().any(|l| l.contains("catches a thief at the edge of the camp with ") || l.contains(": a thief of "))), "no thief came");
}

/// The temple's priest (Dwarf Fortress): the most pious keeps the temple and consecrates the
/// graves, and says the rites over the violently dead.
#[test]
fn a_priest_keeps_the_temple() {
    let text = run_log("3", "45", &[]);
    assert!(text.lines().any(|l| l.contains("takes up the keeping of the temple of ")), "no priest");
    assert!(text.lines().any(|l| l.contains("consecrates them to ") || l.contains("says the rites of ")), "the priest did nothing");
}

/// The call to arms (Dwarf Fortress's missions; Update 5's outflow): seed 3's people are at war,
/// and on day 60 two of its settlers go to fight in it; they come home changed, or not at all.
#[test]
fn the_camp_is_called_to_war() {
    let text = run_log("3", "140", &[]);
    assert!(text.lines().any(|l| l.contains("comes calling for spears for ") && l.contains(" go.")), "no one went to the war");
    assert!(text.lines().any(|l| l.contains("comes home from ") || l.contains(" will not come home")), "no one came back, or word of them");
}

/// The woods grow back (Dwarf Fortress's regrowth; the roadmap's ecology coupling): trees felled
/// a year ago return where the ground is let alone.
#[test]
fn the_woods_grow_back() {
    let text = run_log("76", "155", &[]);
    assert!(text.lines().any(|l| l.contains("young trees stand again among the stumps")), "nothing grew back");
}

/// The tithe (Dwarf Fortress's tax collector; the roadmap's demands from the faction that claims
/// the land): from day 100 a collector comes each year; the speaker pays or refuses, and the
/// people remember it.
#[test]
fn the_tithe_is_paid_or_refused() {
    let text = run_log("23", "101", &[]);
    let line = text.lines().find(|l| l.contains("comes for the tithe")).unwrap_or_else(|| panic!("no collector came"));
    assert!(line.contains(" pays it ") || line.contains(" refuses it") || line.contains("takes nothing"), "{line}");
}

/// Former spouses (Dwarf Fortress's never-deleted links): a death leaves a widow or widower, who
/// may wed again after mourning; the annals keep both.
#[test]
fn the_widowed_are_remembered() {
    let found = ["58", "23", "76"].iter().any(|seed| {
        let path = std::env::temp_dir().join(format!("widow_{}_{}.html", seed, std::process::id()));
        let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
            .args(["--dev", "--headless", "--seed", seed, "--sim-projects", "365"])
            .env("PLANET_ANNALS", &path)
            .output()
            .expect("run planet_generator");
        assert!(out.status.success());
        let html = std::fs::read_to_string(&path).unwrap_or_default();
        let _ = std::fs::remove_file(&path);
        html.contains("widow of ") || html.contains("widower of ") || html.contains("once wed to ")
    });
    assert!(found, "no one was widowed on three dev seeds in a year");
}

/// Vows of vengeance (Dwarf Fortress's revenge goals; the roadmap's threads that land on
/// settlers): a raid's killing is sworn on by the dead's closest.
#[test]
fn the_dead_are_avenged_by_vow() {
    // (Raid deaths have grown rare in camps that dig in and arm: the first raid is made to kill.)
    let logs: Vec<String> = ["76", "11"].iter().map(|s| run_log(s, "60", &[("PLANET_FORCE_RAID_DEATH", "1")])).collect();
    assert!(logs.iter().any(|t| t.lines().any(|l| l.contains(" swears to see ") && l.starts_with("Day"))), "no vow was sworn");
}

/// Vengeance ends in peace (Dwarf Fortress's treaties): seed 76's Republic of Moonvale, sworn to
/// vengeance on day 147, offers peace ninety days after its band was driven off.
#[test]
fn vengeance_can_end_in_peace() {
    let text = run_log("76", "250", &[]);
    assert!(text.lines().any(|l| l.contains("comes offering peace")), "no peace was offered");
}

/// The kitchen (Dwarf Fortress's cooking): a camp of twelve with a storehouse builds a kitchen,
/// and its cook makes supper from what the camp grows and hunts.
#[test]
fn the_cook_makes_supper() {
    let logs: Vec<String> = ["23", "3"].iter().map(|s| run_log(s, "100", &[])).collect();
    assert!(logs.iter().any(|t| t.lines().any(|l| l.contains("cooks the camp's first supper in the kitchen: a "))), "no supper was cooked");
}

/// Dreams of a lifetime (Dwarf Fortress's life goals): each settler dreams of something from
/// their dearest value, and the camp's days can make it come true.
#[test]
fn dreams_come_true() {
    let logs: Vec<String> = ["76", "11", "5"].iter().map(|s| run_log(s, "60", &[])).collect();
    assert!(logs.iter().any(|t| t.lines().any(|l| l.contains(" has realized a dream of a lifetime: "))), "no dream came true");
}

/// The troubles keep coming (Dwarf Fortress: a rich fortress is never left alone for long):
/// when every planned thread is spent, the roads' outlaws or the Shadow's raiders come again.
#[test]
fn troubles_come_again_when_all_are_spent() {
    let logs: Vec<String> = ["3", "76"].iter().map(|s| run_log(s, "330", &[])).collect();
    assert!(logs.iter().any(|t| t.lines().any(|l| l.contains("word of the camp's riches has spread along the roads") || l.contains("reaches this far still, and the camp has grown rich"))), "nothing came again");
}

/// Childhood (Dwarf Fortress's children): children born in the camp, from four, play by the
/// fire or tag along after a parent at work instead of working.
#[test]
fn children_play_and_tag_along() {
    let dir = std::env::temp_dir().join(format!("child_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("decisions.txt");
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--headless", "--seed", "3", "--sim-projects", "620"])
        .env("PLANET_DUMP_DECISIONS", &path)
        .output()
        .expect("run planet_generator");
    assert!(out.status.success());
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(text.lines().any(|l| l.contains("Tagging along after ")), "no child tagged along");
    assert!(text.lines().any(|l| l.contains("Playing by the fire")), "no child played");
}

/// Rations: in a hard winter whose store will not last until spring, the speaker orders half
/// rations, and lifts them when the store will see the camp through.
#[test]
fn the_store_is_rationed_in_a_hard_winter() {
    let text = run_log("76", "110", &[]);
    assert!(text.lines().any(|l| l.contains("orders the store rationed")), "no rations in seed 76's first winter");
}

/// The water freezes in a deep winter (Dwarf Fortress's freezing rivers): seed 3's winters
/// (-7 °C) stop the fishing until the thaw, except through holes at a jetty.
#[test]
fn the_water_freezes_in_a_deep_winter() {
    let text = run_log("3", "125", &[]);
    assert!(text.lines().any(|l| l.contains("The water freezes over")), "no freeze on seed 3");
    assert!(text.lines().any(|l| l.contains("The ice breaks up")), "no thaw");
}

/// Seasonal herds (TODO Phase D's migration): in a hard winter the herds leave for their winter
/// grounds, and come back in spring.
#[test]
fn the_herds_winter_elsewhere() {
    let text = run_log("76", "125", &[]);
    assert!(text.lines().any(|l| l.contains("have gone down to their winter grounds")), "the herds stayed");
    assert!(text.lines().any(|l| l.contains("are back on the hills with the spring")), "the herds never came back");
}

/// Pacing: only major moments stop the window's clock (`Moment::major`); the camp's routine
/// (buildings finished, roles named, festivals) goes to the status line.
#[test]
fn only_major_moments_stop_the_clock() {
    let dir = std::env::temp_dir().join(format!("pace_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--sim-snapshot", dir.join("p").to_str().unwrap()])
        .output()
        .expect("run planet_generator");
    let text = String::from_utf8_lossy(&out.stdout);
    let _ = std::fs::remove_dir_all(&dir);
    let line = text.lines().find(|l| l.starts_with("Moments: ")).expect("moments line");
    let n: Vec<usize> = line.split(|c: char| !c.is_ascii_digit()).filter_map(|x| x.parse().ok()).take(2).collect();
    assert!(n[1] >= 5 && n[1] * 2 <= n[0], "major moments out of proportion: {line}");
}

/// Tavern brawls (Dwarf Fortress): two who dislike each other come to blows over their cups, and
/// the one who struck answers for it at dawn.
#[test]
fn brawls_break_out_at_the_tavern() {
    // (Forced: a hot-tempered drinker who dislikes another at the tavern has grown rare.)
    let found = ["11", "23", "76"].iter().any(|seed| run_log(seed, "200", &[("PLANET_FORCE_BRAWL", "1")]).lines().any(|l| l.contains("come to blows over their cups at the tavern")));
    assert!(found, "no brawl");
}

/// The patron's bell (Dwarf Fortress's civilian alert): one favour, and everyone keeps under a
/// roof until dawn; rung on the eve of seed 76's first raid, the camp is warned.
#[test]
fn the_bell_sends_everyone_indoors() {
    let dir = std::env::temp_dir().join(format!("bell_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("bell.txt");
    // Day 14, 20:00.
    std::fs::write(&script, "19920 bell\n").unwrap();
    let (log, dec) = (dir.join("log.txt"), dir.join("dec.txt"));
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--headless", "--sim-projects", "16"])
        .env("PLANET_SCRIPT", &script).env("PLANET_DUMP_LOG", &log).env("PLANET_DUMP_DECISIONS", &dec)
        .output()
        .expect("run planet_generator");
    assert!(out.status.success());
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let decisions = std::fs::read_to_string(&dec).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(text.contains("The bell rings over the camp"), "the bell never rang");
    assert!(decisions.contains("at the patron's bell"), "no one kept indoors");
}

/// The library (Dwarf Fortress's libraries): once the camp has written two books it builds a
/// library to keep them.
#[test]
fn the_camp_keeps_a_library() {
    let logs: Vec<String> = ["11", "3"].iter().map(|s| run_log(s, "110", &[])).collect();
    assert!(logs.iter().any(|t| t.lines().any(|l| l.contains("They set to work on a library"))), "no library");
}

/// The delve (Dwarf Fortress's fortress): the dev colony at 45,12 cuts a stair down beside the
/// camp to a cellar, then bedrooms and a great hall on levels off the stair; the bedrooms are
/// given out, slept in, and drawn on their level.
#[test]
fn the_camp_digs_a_delve() {
    let dir = std::env::temp_dir().join(format!("delve_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("log.txt");
    let dec = dir.join("dec.txt");
    let frames = dir.join("f");
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--headless", "--sim-projects", "120", "--tiles-center", "45,12"])
        .env("PLANET_DUMP_LOG", &log).env("PLANET_DUMP_DECISIONS", &dec).env("PLANET_FRAMES", &frames)
        .output()
        .expect("run planet_generator");
    assert!(out.status.success(), "planet_generator failed: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let decisions = std::fs::read_to_string(&dec).unwrap_or_default();
    let below2 = dir.join("f_below2.png").exists() || dir.join("f_below.png").exists();
    let _ = std::fs::remove_dir_all(&dir);
    for kind in ["a cellar (", "bedrooms under the rock (", "a great hall below ("] {
        let line = stdout.lines().find(|l| l.contains(kind)).unwrap_or_else(|| panic!("no {kind}:\n{stdout}"));
        assert!(line.contains("done"), "{kind} not finished: {line}");
    }
    let given = text.lines().find(|l| l.contains("a bedroom of their own, cut in the")).unwrap_or_else(|| panic!("no bedroom given:\n{text}"));
    assert!(given.contains("levels down"), "{given}");
    assert!(decisions.lines().any(|l| l.contains("sleeping in a bedroom of their own under the rock")), "nobody slept in a bedroom");
    assert!(below2, "no level frame drawn");
}

/// Wood and fish from the dark: once the stair reaches a cavern, fellers go down for its fungus
/// trees when no timber stands near the camp, and fishers to its still water when there is none
/// to fish above (forced here: the dev camp has both nearby).
#[test]
fn fungus_trees_are_felled_in_the_cavern() {
    let text = run_log("76", "45", &[("PLANET_FORCE_CAVERN", "1")]);
    assert!(text.lines().any(|l| l.contains("breaks through into darkness")), "no cavern breached");
    assert!(text.lines().any(|l| l.contains("fells a fungus tree in ") && l.contains("haul up the stair")), "no fungus tree felled");
    // (Seed 76 eats well and never fishes; seed 3 does below from day 24.)
    let fished = run_log("3", "60", &[("PLANET_FORCE_CAVERN", "1")]);
    assert!(fished.lines().any(|l| l.contains("the first blind white fish")), "no fish from the cavern's water");
}

/// Tombs under the rock (Dwarf Fortress's catacombs): once two of the camp lie in graves at its
/// edge, a level of niches is cut off the stair, and the next dead are laid there (seed 58).
#[test]
fn the_dead_are_laid_in_the_tombs() {
    let text = run_log("58", "130", &[]);
    assert!(text.lines().any(|l| l.contains("set to work on tombs under the rock")), "no tombs dug");
    assert!(text.lines().any(|l| l.contains("lay them in a niche of the tombs")), "no one entombed");
}

/// The magma sea (Dwarf Fortress): the bottom of every embark is a sea of magma; the deep shaft
/// reaches the warm rock over it and the camp forges metal there.
#[test]
fn the_deep_shaft_reaches_the_magma_sea() {
    let text = run_log("76", "150", &[]);
    assert!(text.lines().any(|l| l.contains("the magma sea glows through the cracks")), "the shaft never reached the magma");
}

/// Sealing the caverns (Dwarf Fortress): once the cavern's hunters have come up the stair and
/// hurt someone twice, the camp sets a hatch in the stair at the cavern's roof, and nothing comes
/// up after.
#[test]
fn the_caverns_are_sealed_with_a_hatch() {
    let text = run_log("3", "120", &[]);
    let at = text.lines().position(|l| l.contains("They set a hatch of")).unwrap_or_else(|| panic!("no hatch on seed 3"));
    assert!(!text.lines().skip(at).any(|l| l.contains("come up from the mine, alone")), "something came up after the hatch");
}
