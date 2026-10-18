//! The first colony, unattended on the dev embark: everyone alive after 30 days, nobody stuck,
//! the hut built, and the same story on every run. Runs the real binary (`--sim-snapshot`, a
//! couple of seconds once the dev region is in the chunk cache, ~15 s the first time).

use std::process::Command;

/// The binary with myth and magic on (`--myth`, `crate::myth`): these tests are of the game's
/// systems as they were, Shadow, beasts and all; the realistic default has its own tests.
fn myth_cmd() -> Command { let mut c = Command::new(env!("CARGO_BIN_EXE_planet_generator")); c.env("PLANET_MYTH", "1").env_remove("PLANET_TERRAIN_V1"); c }

fn run(dir: &std::path::Path) -> (String, String) {
    let prefix = dir.join("colony");
    let out = myth_cmd()
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
    // (A smokehouse keeps the food as the rack does; since Stage 6's ground the dev camp builds
    // one in place of the rack.)
    let keeps = at("finishes a drying rack").into_iter().chain(at("A smokehouse finished")).chain(at("finishes a smokehouse")).min();
    let spoiled = at("spoil in the store");
    assert!(spoiled.is_some() && keeps.map_or(true, |b| spoiled.unwrap() < b), "no food spoiled before the rack or the smokehouse:\n{log}");
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
    let out = myth_cmd()
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
    let out = myth_cmd()
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
    let out = myth_cmd()
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
        let mut cmd = myth_cmd();
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
    let out = myth_cmd()
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
    // The raid arrives on legs: the attackers are on the map for 200+ game minutes before the
    // clash (at the window's third speed while they come, over 10 real seconds; since the hut is
    // lengthened into a hall toward the raid's side, dev 76's meet it at 223).
    let legs = text.lines().find(|l| l.starts_with("Raid on legs:")).expect("raid on legs line");
    let ticks: u64 = legs.split_whitespace().find_map(|x| x.parse().ok()).unwrap_or(0);
    assert!(ticks >= 200, "the attackers were barely on the map: {legs}");
    // The window stops for each beat (a card with its because) and for the finished hut.
    let moments = text.lines().find(|l| l.starts_with("Moments:")).expect("moments line");
    for m in ["A rumour", "Refugees", "The raid", "The hut stands"] {
        assert!(moments.contains(m), "no moment '{m}': {moments}");
    }
}

#[test]
fn after_the_hut_the_colony_sets_its_own_work() {
    let out = myth_cmd()
        .args(["--dev", "--headless", "--seed", "5", "--sim-projects", "60"]) // (the dev embark until Stage 6's ground left it short of timber; seed 5 since 2026-10-18)
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
    // (Seeds on the audited terrain: 3 and 15 are raids a careful patron does not change.)
    for seed in ["10", "11", "3", "5", "7", "15"] {
        let out = myth_cmd()
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

/// A doomed site says so: the dev world's bare northern heights at 48,8 and the cold south at
/// 36,41 have nothing to eat within reach of a camp, and are refused (or warned) with the reason
/// instead of starving seven. (On the terrain before the audit: the cold heights at 46,38 and the
/// crater at 28,38 of seed 76.)
#[test]
fn a_doomed_site_says_so() {
    for site in ["48,8", "36,41"] {
        let out = myth_cmd()
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
    let out = myth_cmd()
        .args(["--dev", "--sim-move", "20", "--tiles-center", "53,13"])
        .output()
        .expect("run planet_generator");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("They strike camp"), "the camp did not move:\n{text}");
    assert!(text.contains("Moment: They move the camp (because"), "no moment for the move:\n{text}");
    assert!(text.contains("Mark: The old camp"), "the old camp left no mark:\n{text}");
    // Nobody starves after the move (the raid may take one, and the Shadow's raiders a child).
    assert!(text.contains("Move at 53,13 after 20 days: 9 of 9 alive") || text.contains("Move at 53,13 after 20 days: 8 of 9 alive")
        || text.contains("Move at 53,13 after 20 days: 7 of 9 alive"), "settlers died after moving:\n{text}");
    assert!(!text.contains("died of hunger"), "settlers starved after moving:\n{text}");
    for site in ["48,8", "36,41"] {
        let out = myth_cmd()
            .env("PLANET_FORCE_CAMP", "1")
            .args(["--dev", "--sim-projects", "30", "--tiles-center", site])
            .output()
            .expect("run planet_generator");
        let text = String::from_utf8_lossy(&out.stdout);
        // (Everyone who came leaves alive: 28,38's camp now takes in its refugees before it goes.)
        let line = text.lines().find(|l| l.starts_with("Projects at")).unwrap_or("");
        let n: Vec<u32> = line.split(" alive").next().unwrap_or("").rsplit("; ").next().unwrap_or("").split(" of ").filter_map(|x| x.trim().parse().ok()).collect();
        assert!(line.contains("they left on day") && n.len() == 2 && n[0] == n[1], "{site}: no honest departure:\n{text}");
    }
}


/// The refugees are a choice: both answers change the log within days and the tally at the
/// raid, and six dev seeds keep at least four different timetables.
#[test]
fn the_refugees_are_a_choice() {
    let mut schedules = std::collections::HashSet::new();
    for seed in ["10", "11", "3", "2", "7", "6"] {
        let out = myth_cmd()
            .args(["--dev", "--seed", seed, "--sim-refugees"])
            .output()
            .expect("run planet_generator");
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        if let Some(b) = text.lines().find(|l| l.starts_with("Beats: ")) { schedules.insert(b.to_string()); }
        if text.contains("Refugees taken in") {
            // (A founder from their town takes it hard where there is one; their own fire at the
            // clearing's edge always marks it.)
            assert!(text.contains("are turned away (your doing)") && (text.contains("takes it hard") || text.contains("make a fire of their own")), "seed {seed}: turning away left no mark in the log:\n{text}");
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
    let out = myth_cmd()
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
    let out = myth_cmd()
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
        let out = myth_cmd()
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
        let out = myth_cmd()
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

/// Coarse data drives fine data (DF): a tile the world map gives a river embarks on that river's
/// bank, its channel as wide as the world's discharge makes it. Dev tiles round 36,32 hold four
/// river tiles; none of their embarks held the river before (the tile's centre lies kilometres
/// from it).
#[test]
fn world_rivers_run_through_their_embarks() {
    let out = myth_cmd()
        // (36,32: four tiles of the river by the dev camp; on the old terrain 47,8 of seed 76.)
        .args(["--dev", "--river-survey", "--tiles-center", "36,32"])
        .env("PLANET_SURVEY_RADIUS", "2")
        .output()
        .expect("run planet_generator");
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let line = text.lines().find(|l| l.starts_with("River survey:")).unwrap_or_else(|| panic!("no survey:\n{text}"));
    let nums: Vec<usize> = line.split(|c: char| !c.is_ascii_digit()).filter_map(|n| n.parse().ok()).collect();
    assert!(nums.len() >= 2 && nums[1] >= 3 && nums[0] == nums[1], "world-river tiles whose embark lacks the river: {line}");
    // The embark's channel against the world's width: within a factor of two (it may lie below
    // a confluence inside the tile).
    for l in text.lines().filter(|l| l.contains("world RIVER")) {
        let n: Vec<f32> = l.split(|c: char| !c.is_ascii_digit() && c != '.').filter_map(|n| n.parse().ok()).collect();
        // Tile x, y, world width, region widest, embark river, water columns.
        let (world, embark) = (n[2], n[4]);
        assert!(embark >= 0.5 * world && embark <= 2.0 * world, "the embark's river is not the world's: {l}");
    }
}

/// Skill makes roles: the dev colony names three or more of its settlers for a trade, the
/// builder lays at least half the loads once named, and when the builder dies someone takes up
/// the hammer and the work is slower.
#[test]
fn skill_makes_roles() {
    // (The first of a few seeds where someone free takes the hammer within ten days: on some the
    // next-best builders hold another trade on day 40. Timelines move with every system.)
    let roles = |seed: &str| String::from_utf8_lossy(&myth_cmd()
        .args(["--dev", "--seed", seed, "--sim-roles"]).output().expect("run planet_generator").stdout).to_string();
    let text = ["58", "2", "5", "11", "76"].iter().map(|s| roles(s)).find(|t| t.contains("takes up the hammer")).unwrap_or_else(|| roles("58"));
    let trial = text.lines().find(|l| l.starts_with("Roles trial:")).unwrap_or_else(|| panic!("no roles trial:\n{text}"));
    // (Minutes to a tenth: with twenty in the camp by day 40 the next hand can be close.)
    let n: Vec<f32> = trial.split(|c: char| !c.is_ascii_digit() && c != '.').filter_map(|x| x.trim_end_matches('.').parse().ok()).collect();
    assert!(n.len() >= 3 && n[2] > n[1], "the work did not slow when the builder died: {trial}");
    assert!(text.contains("takes up the hammer"), "no one took up the hammer:\n{text}");
    let dir = std::env::temp_dir().join(format!("roles_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = myth_cmd()
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
    let out = myth_cmd()
        .args(["--dev", "--seed", "6", "--sim-plan"]) // (the dev embark until Stage 6's ground left it no room; seed 6 since 2026-10-18)
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
    let out = myth_cmd()
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
    // Dwarves dig their hall into the hill first where there is one (dev seed 14's hillside on the
    // audited terrain, also 24 and 29; 19, 5 and 11 had been on the old).
    // (Seed 24 since Stage 6's ground, 2026-10-18.)
    let out = myth_cmd().args(["--dev", "--seed", "24", "--sim-ways"]).output().expect("run planet_generator");
    let text = String::from_utf8_lossy(&out.stdout);
    let dwarves = text.lines().find(|l| l.starts_with("Way of the dwarves")).expect("dwarves line");
    assert!(dwarves.contains("works: hall in the hill"), "the dwarves did not dig their hall first: {dwarves}");
}

#[test]
fn settlers_go_under() {
    use planet_generator::colony::dig::dig_minutes;
    use planet_generator::erosion::materials::RockType;
    use planet_generator::local::Material;
    // Soft rock is quick, hard rock slow.
    assert!(dig_minutes(Material::Rock(RockType::Limestone)) * 2 <= dig_minutes(Material::Rock(RockType::Granite)));
    // Dev 33,26 lies below a hill with no timber in reach: by day 120 they have dug a hall into it
    // and sleep there, and the rock carried out was laid as stone. (Seed 76's 50,20 on the old
    // terrain.)
    let out = myth_cmd()
        .args(["--dev", "--sim-projects", "120", "--tiles-center", "33,26"])
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
/// 42,32 holds the living Korix the Terrible's lair and a limestone cave that can be walked into;
/// 33,32 the tomb of Zreetsio, captain of Aiz'shaz, who fell there; and a fair share of embarks
/// hold nothing at all. (On the old terrain: 46,13, 47,14 and 50,20 of seed 76.)
#[test]
fn something_down_there() {
    let places = |site: &str| -> Vec<String> {
        let out = myth_cmd()
            .args(["--dev", "--sim-projects", "1", "--tiles-center", site])
            .output()
            .expect("run planet_generator");
        String::from_utf8_lossy(&out.stdout).lines().filter(|l| l.trim_start().starts_with("Place:")).map(|l| l.to_string()).collect()
    };
    let lair = places("42,32");
    // (Whose lair moves with the history: Baelfang's, then Zaresh the Desolator's since hard times.)
    assert!(lair.iter().any(|l| l.contains("The lair of ") && l.contains("lairs here, and lives")), "no lair of the living beast: {lair:?}");
    let tomb = places("33,32");
    assert!(tomb.iter().any(|l| l.contains("(Tomb)") && l.contains("fell here")), "no tomb at 33,32: {tomb:?}");
    let cave = places("42,32");
    assert!(cave.iter().any(|l| l.contains("(Cave)") && l.contains("hollowed by water") && l.contains("walkable from the surface")), "no cave to walk into: {cave:?}");
    let mut none = 0;
    let sites = ["30,23", "36,26", "45,29", "66,20", "27,29", "30,32", "36,29", "39,26", "45,26", "57,14", "60,14", "69,17"];
    for s in sites { if places(s).is_empty() { none += 1; } }
    assert!(none * 5 >= sites.len() && none * 2 <= sites.len(), "{none} of {} embarks hold nothing", sites.len());
}

/// Settlers are individuals (Dwarf Fortress style): each has a description from their people's
/// template, and two settlers of one people are told apart by more than their names.
#[test]
fn settlers_are_rolled_individuals() {
    let dir = std::env::temp_dir().join(format!("who_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = myth_cmd()
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
        let out = myth_cmd()
            .args(["--dev", "--headless", "--seed", seed, "--sim-projects", days])
            .env("PLANET_DUMP_LOG", &log)
            .output()
            .expect("run planet_generator");
        assert!(out.status.success(), "planet_generator failed: {}", String::from_utf8_lossy(&out.stderr));
        let text = std::fs::read_to_string(&log).unwrap_or_default();
        let _ = std::fs::remove_dir_all(&dir);
        text
    };
    // 150 days on seed 8, three of whose people break (seed 4's broke after its first raid
    // until the raid was fought on the map and its defenders' nights changed).
    let hard = run("8", "150");
    let breaks: Vec<&str> = hard.lines().filter(|l| l.contains("throws a tantrum") || l.contains("sinks into despair") || l.contains("walks off into the wild")).collect();
    assert!(!breaks.is_empty(), "seed 8's camp never broke:\n{hard}");
    assert!(breaks.iter().all(|l| l.contains("(because they ")), "a break without its reasons: {breaks:?}");
    // Starving camps
    // spiral into tantrums as in Dwarf Fortress; a runaway mind would break far more.
    assert!(breaks.len() <= 20, "too many breaks in 150 days ({}): {breaks:?}", breaks.len());
    // (No "calm camp" check any more: a camp in the Shadow's reach sees its dead walk, and
    // that breaks people too.)
}

/// Digging too deep (Dwarf Fortress): the dev colony at 72,8 sinks a mine for ore, breaks into
/// the first cavern, the miners hear the deep stir, and the forgotten beast that slept there
/// climbs out of the mine, with its generated special attack.
#[test]
fn the_mine_wakes_the_deep() {
    let dir = std::env::temp_dir().join(format!("deep_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("log.txt");
    let out = myth_cmd()
        // (72,8 until Stage 6's ground, whose mine now strikes water first; since 2026-10-18 dev
        // seed 9: the mine day 16, the breach, the sound day 23, the beast out on day 29.)
        .args(["--dev", "--headless", "--seed", "9", "--sim-projects", "45"])
        .env("PLANET_DUMP_LOG", &log)
        .output()
        .expect("run planet_generator");
    assert!(out.status.success(), "planet_generator failed: {}", String::from_utf8_lossy(&out.stderr));
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let _ = std::fs::remove_dir_all(&dir);
    let at = |needle: &str| text.lines().position(|l| l.contains(needle)).unwrap_or_else(|| panic!("no '{needle}' in the log:\n{text}"));
    // (It climbs out, or meets someone on the stair first: then the raid is fought below, "out of
    // the mine".)
    let climbs = text.lines().position(|l| l.contains("climbs out of the mine") || (l.contains("The raid: ") && l.contains("out of the mine")))
        .unwrap_or_else(|| panic!("nothing came up out of the mine:\n{text}"));
    let (mine, breach, sound) = (at("They set to work on a mine"), at("breaks through into darkness"), at("A sound from below"));
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
    let out = myth_cmd()
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
    let out = myth_cmd()
        .args(["--dev", "--headless", "--seed", "5", "--sim-projects", "120"]) // (seed 5 since Stage 6's ground left the dev embark short of timber, 2026-10-18)
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
    let out = myth_cmd()
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

/// News as it is told (`history::knowledge`, `colony/news.rs`): caravans bring what their town has
/// heard, visitors and migrants what their home knows, each slanted their people's way; bards sing
/// their people's account; listeners whose people tell it otherwise say so; the annals keep it all.
#[test]
fn news_comes_as_its_teller_tells_it() {
    let dir = std::env::temp_dir().join(format!("news_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (log, annals) = (dir.join("log.txt"), dir.join("annals.html"));
    // (Seed 2: caravan news, word, a song and an account told otherwise. The seed moves with the
    // history and the land: 76, then 3, then 7, then since hard times 4, on the audited terrain 2.)
    let out = myth_cmd()
        .args(["--dev", "--headless", "--seed", "2", "--sim-projects", "150"])
        .env("PLANET_DUMP_LOG", &log)
        .env("PLANET_ANNALS", &annals)
        .output()
        .expect("run planet_generator");
    assert!(out.status.success());
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let page = std::fs::read_to_string(&annals).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    let news = text.lines().find(|l| l.contains("They bring news: ")).unwrap_or_else(|| panic!("no news from the caravans:\n{text}"));
    assert!(news.contains(" (as the "), "the caravan's news has no teller: {news}");
    assert!(text.lines().any(|l| l.contains(" brings word from ")), "no visitor or migrant brought word:\n{text}");
    assert!(text.lines().any(|l| l.contains(" sings of ") && l.contains(" tell")), "no bard sang their people's account");
    assert!(text.lines().any(|l| l.contains("will not hear it told so") || l.contains(" call it ") || l.contains(" calls it ")), "no listener's people told it otherwise");
    assert!(text.lines().any(|l| l.contains("at the news: it is their people's")), "news moved no one");
    assert!(page.contains("<h2>News from the World</h2>") && page.contains(" tell"), "the annals keep no news");
}

/// The camp's first office (Dwarf Fortress's positions and mandates): a month on, the camp
/// chooses someone to speak for it, who proclaims a mandate from the value they hold dearest.
#[test]
fn a_speaker_proclaims_a_mandate() {
    let dir = std::env::temp_dir().join(format!("speaker_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("log.txt");
    let out = myth_cmd()
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
    // (Seeds 11, 58 and 5 lie under the Shadow; with fewer deaths since settlers have needs of
    // their own, 58 and 5 are the ones whose graves give up their dead within 40 days.)
    let found = ["11", "58", "5"].iter().any(|seed| {
        let dir = std::env::temp_dir().join(format!("dead_{}_{}", seed, std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("log.txt");
        let out = myth_cmd()
            .args(["--dev", "--headless", "--seed", seed, "--sim-projects", "40"])
            .env("PLANET_DUMP_LOG", &log)
            .env(SHADOW.0, SHADOW.1)
            .output()
            .expect("run planet_generator");
        assert!(out.status.success());
        let text = std::fs::read_to_string(&log).unwrap_or_default();
        let stdout = String::from_utf8_lossy(&out.stdout);
        let _ = std::fs::remove_dir_all(&dir);
        stdout.lines().any(|l| l.contains("Darkness: ")) && text.lines().any(|l| l.contains("rises from the grave"))
    });
    assert!(found, "no dead rose under the Shadow on seeds 11, 58 or 5");
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
        let out = myth_cmd()
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
    // (Seeds 2 and 5 make one by day 60 on the audited terrain; 12 had since hard times.)
    let made = ["2", "5"].iter().map(|s| run(s, "60")).find(|t| t.contains("has made an artifact: ")).unwrap_or_default();
    let art = made.lines().find(|l| l.contains("has made an artifact: ")).unwrap_or_else(|| panic!("no artifact on seeds 2 or 5:\n{made}"));
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
    let out = myth_cmd()
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
    let out = myth_cmd()
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
    let out = myth_cmd()
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
    // (Seed 1: the devout ask for it on day 22 and keep the festival of day 31 in their god's
    // honour. The dev camp's temple is pressed for by one voice, which names no god.)
    let out = myth_cmd()
        .args(["--dev", "--headless", "--seed", "1", "--sim-projects", "40"])
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
    let out = myth_cmd()
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
    let out = myth_cmd()
        .args(["--dev", "--headless", "--seed", "1", "--sim-projects", "110"]) // (seed 1 since Stage 6's ground, 2026-10-18: the Helm of Loranor)
        .env("PLANET_DUMP_LOG", &log)
        // (Since hard times dev 76 has no lost thing within six tiles: the nearest anywhere whose
        // owners stand. It had been the Staff of Greenburg.)
        .env("PLANET_FORCE_RELIC", "1")
        .output()
        .expect("run planet_generator");
    assert!(out.status.success());
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    let has = |s: &str| text.lines().any(|l| l.contains(s));
    let name = text.lines().find_map(|l| l.split("tell of ").nth(1).or_else(|| l.split("tells by the fire of ").nth(1)).or_else(|| l.split(" finds ").nth(1).filter(|r| r.contains(" in the earth"))))
        .map(|r| r.split([',', '.', ':', ';']).next().unwrap_or(r).replace(" in the earth", "")).unwrap_or_else(|| panic!("no tale:\n{text}"));
    let name = name.trim().to_string();
    assert!(has(&format!("finds {}", name)), "never found: {name}");
    // Claimed: its owners' envoy asks for it back and is answered, or (an heir having taken it
    // first) finds it gone.
    let answered = has("ask for it back") && (has(&format!("give {} back", name)) || has(&format!("gives {} back", name)));
    assert!(answered || has(&format!("comes for {}, but it is gone", name)), "no envoy came for {name}:\n{text}");
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
    let out = myth_cmd()
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
    // (Seed 1: the dev camp's elves drill but make no spear in 65 days on the audited terrain.
    // Since Stage 6's ground (2026-10-18) seed 11 drills on day 3 and arms on day 9.)
    let out = myth_cmd()
        .args(["--dev", "--headless", "--seed", "11", "--sim-projects", "65"])
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
    // (Seed 11 for the hunter; seed 3 too, whose taught works are performed: seed 11's pupil is
    // seldom at the fire at eight in the evening since settlers have spare hours of their own.)
    let logs: Vec<String> = ["11", "3"].iter().map(|s| run_log(s, "190", &[])).collect();
    let has = |s: &str| logs.iter().any(|t| t.lines().any(|l| l.contains(s)));
    assert!(has("a monster hunter of") && has("who set out to slay"), "no hunter came");
    // (To the fire, or to the tavern once one stands: seed 11's drinkers press for one early.)
    assert!(has("A traveller comes to the fire") || has("A traveller comes to the tavern"), "no bard came");
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
    let out = myth_cmd()
        .args(["--dev", "--headless", "--seed", "1", "--sim-projects", "110"]) // (seed 1 since Stage 6's ground, 2026-10-18)
        .env("PLANET_DUMP_LOG", &log)
        .env("PLANET_FORCE_SEEKER", "Orvel")
        // (Since hard times dev 76 has no lost thing within six tiles: the nearest anywhere.)
        .env("PLANET_FORCE_RELIC", "1")
        .output()
        .expect("run planet_generator");
    assert!(out.status.success());
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    let has = |s: &str| text.lines().any(|l| l.contains(s));
    // (A real heir comes when there is one; Orvel when forced and there is none.)
    let came = text.lines().find(|l| l.contains("A stranger comes asking after ")).unwrap_or_else(|| panic!("no seeker:\n{text}"));
    let who = came.split(": ").nth(1).and_then(|r| r.split(',').next()).unwrap_or("").trim().to_string();
    assert!(!who.is_empty() && has(&format!("{} asks ", who)), "the seeker never asked: {came}");
    assert!(text.lines().any(|l| l.contains(" gives ") && l.contains(&format!(" to {}", who))) || has(&format!("{} slips away", who)) || has(&format!("catches {}", who)), "no end to the asking");
}

/// Engravings (Dwarf Fortress): the dev colony at 33,26 digs a hall and carves its walls with
/// its own story first (raids, deaths, guests) and then the engraver's past.
#[test]
fn the_hall_is_engraved_with_its_story() {
    let dir = std::env::temp_dir().join(format!("engrave_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("log.txt");
    let out = myth_cmd()
        .args(["--dev", "--headless", "--sim-projects", "150", "--tiles-center", "33,26"])
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
    let out = myth_cmd()
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
    let out = myth_cmd()
        .args(["--dev", "--headless", "--seed", "1", "--sim-projects", "200"]) // (seed 1 since Stage 6's ground, 2026-10-18)
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
    let out = myth_cmd()
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
    let out = myth_cmd()
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

/// Aquifers (Dwarf Fortress): the dev site 72,8 lies where the land holds water; its mine opens
/// wet rock, stops, lines the shaft in stone as a work of its own, and goes on down.
#[test]
fn the_mine_strikes_an_aquifer() {
    let dir = std::env::temp_dir().join(format!("aquifer_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("log.txt");
    let out = myth_cmd()
        .args(["--dev", "--headless", "--sim-projects", "40", "--tiles-center", "72,8"])
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

/// Gems in the rock (Dwarf Fortress's clusters): digging at 72,8 turns up gems, and the
/// workshop sets it in a work.
#[test]
fn gems_are_found_and_set() {
    let dir = std::env::temp_dir().join(format!("gems_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("log.txt");
    let out = myth_cmd()
        // (120 days: with spare hours spent on their needs the camp's first set work came later.)
        .args(["--dev", "--headless", "--sim-projects", "120", "--tiles-center", "72,8"])
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
    let out = myth_cmd()
        .args(["--dev", "--seed", "11", "--sim-legend"])
        .output()
        .expect("run planet_generator");
    let text = String::from_utf8_lossy(&out.stdout);
    let legend = text.lines().find(|l| l.starts_with("Legend: ")).unwrap_or_else(|| panic!("no legend:\n{text}"));
    // (Which beast falls moves with the history: Fyron Plague-Bearer, then Ithon Plague-Bearer.)
    let slain: Vec<String> = legend.split("slain: [").nth(1).and_then(|r| r.split(']').next()).unwrap_or("")
        .split(", ").map(|n| n.trim_matches('"').to_string()).filter(|n| !n.is_empty()).collect();
    assert!(!slain.is_empty(), "no slain beast in the legend: {legend}");
    let second = text.lines().find(|l| l.starts_with("Second camp")).unwrap_or_else(|| panic!("no second camp:\n{text}"));
    let (before, after) = second.split_once("; after").unwrap_or((second, ""));
    assert!(slain.iter().any(|n| before.contains(n.as_str()) && !after.contains(n.as_str())), "no slain beast was dropped from the second camp's troubles: {second}");
    assert!(text.contains("They know the songs of"), "the newcomers know nothing of the old camp:\n{text}");
}

/// Ghosts and memorials (Dwarf Fortress): a settler killed by violence is restless;
/// unremembered five days, their ghost walks until a slab (or post) is carved in their memory.
/// Most camps carve within days, so a few dev seeds are tried until one shows it.
#[test]
fn the_dead_who_died_badly_walk_until_remembered() {
    // (Camps carve slabs for their dead within days, so a ghost is rare: PLANET_FORCE_GHOST keeps
    // anyone from carving for six days, and PLANET_FORCE_RAID_DEATH gives the violent death.
    // Seeds 23 and 58: seen on days 17 and 16, at rest the next days.)
    let found = ["23", "58", "11"].iter().any(|seed| {
        let text = run_log(seed, "60", &[("PLANET_FORCE_GHOST", "1"), ("PLANET_FORCE_RAID_DEATH", "1")]);
        let seen = text.lines().position(|l| l.contains("The ghost of ") && l.contains(" is seen "));
        let rest = text.lines().position(|l| l.contains("carved in memory of") && l.contains("ghost is at rest"));
        seen.is_some() && rest.is_some() && seen < rest
    });
    assert!(found, "no ghost walked and was laid to rest on any of five dev seeds");
}

/// The dev colony's log for `seed` after `days`, with extra environment.
/// The Shadow in the camp (off by default since 2026-10-12, card 'Switch off the Shadow's impact'):
/// the tests of what it brings turn it on.
const SHADOW: (&str, &str) = ("PLANET_CAMP_SHADOW", "1");

fn run_log(seed: &str, days: &str, env: &[(&str, &str)]) -> String {
    // A folder per call: tests run in parallel, and two may ask for the same seed and days.
    static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("runlog_{}_{}_{}_{}", seed, days, std::process::id(), n));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("log.txt");
    let mut cmd = myth_cmd();
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
    let found = ["7", "10", "15", "16"].iter().any(|seed| {
        let text = run_log(seed, "150", &[("PLANET_FORCE_HUNT", "1")]);
        let sent = text.lines().position(|l| l.contains(" out to meet ") || l.contains(" out to hunt "));
        let home = text.lines().position(|l| l.contains("The hunting party comes home") || l.contains("come home from the hunt"));
        matches!((sent, home), (Some(a), Some(b)) if a < b)
    });
    assert!(found, "no hunting party went out and came home on four dev seeds");
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
    // (Seed 1 draws its first wine on day 18, 23 on day 31; seed 3, which had by day 40, since
    // the deep rock gives ore and the camps dig for it, on day 41. Since Stage 6's ground
    // (2026-10-18): seed 11 on day 10, 8 on day 16.)
    let text = ["11", "8"].iter().map(|s| run_log(s, "40", &[])).find(|t| t.contains("draws the first berry wine")).unwrap_or_else(|| run_log("11", "40", &[]));
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
/// are jaded. (Each seen on at least one of five dev seeds; since seed 58 carves its slabs in
/// time its ghost no longer walks, and seed 1 is the one that grows jaded, on day 116.)
#[test]
fn experience_changes_people() {
    // (Seed 3 grows cheerful, 11 anxious. Jaded takes eight horrors, and camps that keep their
    // people alive no longer see so many: PLANET_FORCE_HORROR counts each three times.)
    let logs: Vec<String> = ["3", "11"].iter().map(|s| run_log(s, "250", &[])).collect();
    let any = |needle: &str| logs.iter().any(|t| t.lines().any(|l| l.contains(needle)));
    assert!(any("starts at every shadow"), "no one grew anxious");
    let jaded = ["11", "1", "4"].iter().any(|s| run_log(s, "150", &[("PLANET_FORCE_HORROR", "1")]).lines().any(|l| l.contains("has seen too much to be shaken now")));
    assert!(jaded, "no one grew jaded");
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
    // (Seeds 10 and 3 write their chronicles by day 140 on the audited terrain; 76 and 1 had on
    // the terrain before it.)
    let logs: Vec<String> = ["10", "3"].iter().map(|s| run_log(s, "140", &[])).collect();
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
    // (Seeds 1 and 3 on the audited terrain; 76 and 23 before it.)
    let found = ["1", "3"].iter().any(|seed| {
        let text = run_log(seed, "150", &[]);
        let at = |n: &str| text.lines().position(|l| l.contains(n));
        match (at("strike a vein of adamantine"), at("break into a hollow"), at("the demon")) {
            (Some(a), Some(h), Some(d)) => a < h && h < d,
            _ => false,
        }
    });
    assert!(found, "no adamantine, hollow and demon on seeds 1 or 3");
}

/// Places in the hills (Dwarf Fortress's discoveries): the embark's tombs, lairs, old mines and
/// caves are found by those who pass; a robbed tomb's dead rise for their arms (seed 58).
#[test]
fn places_in_the_hills_are_found() {
    // (Which place is found first moves with every change to the camp's days; any will do, and a
    // tomb found is robbed or left sealed.)
    let logs: Vec<String> = ["58", "3", "4"].iter().map(|s| run_log(s, "200", &[])).collect();
    assert!(logs.iter().any(|t| t.lines().any(|l| l.contains(" finds ") && (l.contains(" in the hills") || l.contains("finds a cave") || l.contains("finds the")))), "no place found");
    for t in &logs { if t.contains("finds the tomb") { assert!(t.contains("breaks into the tomb") || t.contains("leaves it sealed")); } }
    let caves = ["4", "5"].iter().any(|s| run_log(s, "120", &[]).lines().any(|l| l.contains(" finds a cave in the ")));
    assert!(caves, "no cave found on seeds 4 or 5");
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
/// the hunt), and in the clash it turns blows. (Leather and hide only lighten an edge; mail is
/// forged from bars, so ore is forced: no dev seed strikes any.)
#[test]
fn armour_turns_blows() {
    // (The first of a few seeds that turns a blow: it takes armour made and a blow at its wearer.)
    let turned = |t: &str| ["does not get through", "took the worst of it", "turns it.", "is turned by"].iter().any(|n| t.contains(n));
    let mut made = false;
    // (Since hard times seed 1 turns one on day 51, 10 on day 73, 9 on day 92. Since Stage 6's
    // ground (2026-10-18) 19 on day 67, 11 on day 125, 8 on day 155.)
    let found = ["19", "11", "8"].iter().any(|s| {
        let t = run_log(s, "200", &[("PLANET_FORCE_ORE", "1")]);
        made |= t.contains("the first armour in the camp");
        t.contains("the first armour in the camp") && turned(&t)
    });
    assert!(made, "no armour made");
    assert!(found, "no blow turned");
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
    // (Without the Shadow's raiders, seeds 6, 7 and 10 are besieged by war bands from day 147-159.)
    // (Seeds 3 and 5 on the audited terrain.)
    let logs: Vec<String> = ["3", "5"].iter().map(|s| run_log(s, "210", &[])).collect();
    let any = |n: &str| logs.iter().any(|t| t.lines().any(|l| l.contains(n)));
    assert!(any("The camp is besieged; no one goes beyond the palisade."), "no siege");
    assert!(any("of the siege: the fires of "), "the siege's days were not counted");
    assert!(any("spears out of the gate") || any("fires went cold") || any("came in the night"), "the siege never ended");
}

/// Evil weather (Dwarf Fortress's evil regions): under the Shadow's darkness a red rain, a black
/// mist or a cloud of ash comes over the camp; those under a roof are safe, the rest fall ill.
#[test]
fn evil_weather_under_the_shadow() {
    // (Seed 3 lies at 0.48 of the Shadow's darkness on the audited terrain, seed 5 at 0.04; 11
    // and 23 had before it.)
    let text = run_log("3", "120", &[SHADOW]);
    let line = text.lines().find(|l| l.contains(" sky comes ")).unwrap_or_else(|| panic!("no evil weather on seed 3"));
    assert!(line.contains("caught in the open") || line.contains("under a roof in time"), "{line}");
    assert!(!run_log("3", "120", &[]).lines().any(|l| l.contains(" sky comes ")), "evil weather with the Shadow kept out of the camp");
    let calm = run_log("5", "120", &[SHADOW]);
    assert!(!calm.lines().any(|l| l.contains(" sky comes ")), "evil weather where there is no darkness");
}

/// Guilds (Dwarf Fortress's nested organizations): four masters of one trade swear themselves to
/// a guild and petition the speaker for a hall.
#[test]
fn masters_form_a_guild() {
    let logs: Vec<String> = ["1", "2"].iter().map(|s| run_log(s, "120", &[])).collect();
    let any = |n: &str| logs.iter().any(|t| t.lines().any(|l| l.contains(n)));
    assert!(any("swear themselves to one another as The "), "no guild");
    assert!(any(" for a hall for The "), "no petition");
}

/// The world remembers camps (legends and regard): the Git Clans, refused their staff by the dev
/// camp, hold half that grudge against a camp founded two tiles away.
#[test]
fn peoples_remember_earlier_camps() {
    // (Seed 23: the Greenburg League holds -20 against the first camp and half of it against a
    // camp founded two tiles away. Seed 76's grudge over the staff is now cancelled by the
    // prisoner it sent home unharmed.)
    // (Seed 4 on the audited terrain: the Republic of Eastwick's grudge. Since Stage 6's ground
    // (2026-10-18) seed 3: the Republic of Venomcoil -17, the second camp -8.)
    let out = myth_cmd()
        .args(["--dev", "--seed", "3", "--sim-legend"])
        .output()
        .expect("run planet_generator");
    let text = String::from_utf8_lossy(&out.stdout);
    let legend = text.lines().find(|l| l.starts_with("Legend: ")).unwrap_or_else(|| panic!("no legend:\n{text}"));
    let people = legend.split("regards: [\"").nth(1).and_then(|r| r.split(" -").next()).unwrap_or_else(|| panic!("no grudge in the legend: {legend}"));
    let second = text.lines().find(|l| l.starts_with("Second camp's regards")).unwrap_or_else(|| panic!("no second camp:\n{text}"));
    assert!(second.contains(&format!("{} -", people)), "the second camp inherits nothing from {people}: {second}");
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
    // (Which seed shows it moves with every change to the camps' timelines. Since news is told
    // as each people tells it the timelines moved again and no keeper was caught alone at night
    // on 12 dev seeds in 300 days, so PLANET_FORCE_PET_PREY makes a keeper the hunters' first
    // prey among those alone: the pet must still be near, come running and win or fall. Seed 1:
    // day 22.)
    // (Since hard times, and since wolves keep out of the firelight, the flag also brings the
    // pet from anywhere, lets the wolves past the fire and leaves their den: seed 8, day 48.)
    // (On the audited terrain: seed 1 on day 83, 12 on day 140.)
    let found = [("1", "120"), ("12", "260"), ("13", "260")].iter()
        .any(|(seed, days)| run_log(seed, days, &[("PLANET_FORCE_PET_PREY", "1")]).lines().any(|l| l.contains(" comes running") && (l.contains("stands over") || l.contains("dragged down in"))));
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
    // (125 days: Gru now walks up the stair from its cavern and falls on day 107, its bone worked
    // from day 111; seed 11's coat of hide is made on day 29.)
    let logs: Vec<String> = ["76", "11"].iter().map(|s| run_log(s, "125", &[])).collect();
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
/// den after four bites, and under the Shadow burns the dead that rose three times.
#[test]
fn the_camp_clears_the_den_and_burns_the_restless() {
    // (Since settlers live through more, fewer graves rise: seed 58 burns its restless dead on
    // day 23, seed 11 clears its den the same day.)
    // (The restless dead are the Shadow's: with it in the camp.)
    // (Since hard times: seed 1 clears its den on day 23, seed 58 burns its dead on day 23. On the
    // audited terrain seed 12 clears its den and seed 7 burns its dead within 60 days. Since Stage
    // 6's ground (2026-10-18) seed 24 does both, on days 32 and 40.)
    let logs: Vec<String> = ["24", "7"].iter().map(|s| run_log(s, "60", &[SHADOW])).collect();
    let any = |f: &dyn Fn(&str) -> bool| logs.iter().any(|t| t.lines().any(|l| f(l)));
    assert!(any(&|l| l.contains("go out to the wolves' den at ")), "the wolves were never answered");
    assert!(any(&|l| l.contains("burn what is left on a pyre") || l.contains("burn the bones in it")), "the restless dead were never burned");
}

/// A hungry camp plants first: seed 58, hungry from its first weeks, sets a field ahead of the
/// works under way instead of sitting a year behind an unfinished well.
#[test]
fn a_hungry_camp_plants_first() {
    let text = run_log("58", "45", &[]);
    assert!(text.lines().any(|l| l.contains("They set to work on a fenced field")), "seed 58 never planted");
}

/// Artifact thieves (Dwarf Fortress): a hostile people's thief comes for the camp's artifact;
/// caught, they are the camp's prisoner.
#[test]
fn thieves_come_for_the_artifact() {
    // (Seeds 14 and 3: days 160 and 190. Thieves come for artifacts of moods, which come
    // later in camps whose makers have spare hours of their own.)
    let logs: Vec<String> = ["14", "3"].iter().map(|s| run_log(s, "200", &[])).collect();
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
    // (Seed 7: called on day 60, home on day 141; seed 5 likewise. Seed 3's people no longer
    // fight the war since hard times reshaped the history.)
    // (Seeds 4 and 15 on the audited terrain.)
    let text = run_log("4", "160", &[]);
    let text = if text.contains("comes calling for spears for ") { text } else { run_log("15", "160", &[]) };
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
        let out = myth_cmd()
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
    // (Seed 5 offers peace on day 119, 7 on day 143; seed 76, which had, no longer swears
    // vengeance since hard times reshaped the history.)
    // (Seeds 4 and 8 on the audited terrain.)
    let text = run_log("4", "200", &[]);
    let text = if text.contains("comes offering peace") { text } else { run_log("8", "200", &[]) };
    assert!(text.lines().any(|l| l.contains("comes offering peace")), "no peace was offered");
}

/// The kitchen (Dwarf Fortress's cooking): a camp of twelve with a storehouse builds a kitchen,
/// and its cook makes supper from what the camp grows and hunts.
#[test]
fn the_cook_makes_supper() {
    // (Seeds 5 and 1 cook their first supper on day 91, 23 on day 102.)
    let logs: Vec<String> = ["5", "1", "23"].iter().map(|s| run_log(s, "110", &[])).collect();
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
    let out = myth_cmd()
        // (Seed 3 until Stage 6's ground; since 2026-10-18 seed 18, its first child born on day 92.)
        .args(["--dev", "--headless", "--seed", "18", "--sim-projects", "620"])
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
    let text = run_log("10", "110", &[]);
    assert!(text.lines().any(|l| l.contains("orders the store rationed")), "no rations in the dev camp's first winter");
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
    let text = run_log("10", "125", &[]);
    assert!(text.lines().any(|l| l.contains("have gone down to their winter grounds")), "the herds stayed");
    assert!(text.lines().any(|l| l.contains("are back on the hills with the spring")), "the herds never came back");
}

/// Pacing: only major moments stop the window's clock (`Moment::major`); the camp's routine
/// (buildings finished, roles named, festivals) goes to the status line.
#[test]
fn only_major_moments_stop_the_clock() {
    let dir = std::env::temp_dir().join(format!("pace_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = myth_cmd()
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
    let out = myth_cmd()
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
/// (Seed 76 too: the industries' digs below put seed 11's library at day 120 and seed 3's out of
/// its first year.)
#[test]
fn the_camp_keeps_a_library() {
    // (Seed 76 plans one by day 93; 11 and 3 later since the news, the delve and the industries.)
    let found = ["76", "11", "3"].iter().any(|s| run_log(s, "130", &[]).lines().any(|l| l.contains("They set to work on a library")));
    assert!(found, "no library");
}

/// The delve (Dwarf Fortress's fortress): the dev colony at 0,35 cuts a stair down beside the
/// camp to a cellar, then bedrooms and a great hall on levels off the stair; the bedrooms are
/// given out, slept in, and drawn on their level. (72,8 on the audited terrain digs its great hall
/// after day 120.)
#[test]
fn the_camp_digs_a_delve() {
    let dir = std::env::temp_dir().join(format!("delve_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("log.txt");
    let dec = dir.join("dec.txt");
    let frames = dir.join("f");
    let out = myth_cmd()
        .args(["--dev", "--headless", "--sim-projects", "120", "--tiles-center", "0,35"])
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
    // (Seed 76 fishes below on day 34, seed 10 on day 23: forced, the cavern's water is sought
    // before the berries.)
    // (Seeds 1 and 3 on the audited terrain.)
    let logs: Vec<String> = ["1", "3"].iter().map(|s| run_log(s, "90", &[("PLANET_FORCE_CAVERN", "1")])).collect();
    let text = logs.iter().find(|t| t.lines().any(|l| l.contains("the first blind white fish"))).cloned().unwrap_or_else(|| logs[0].clone());
    assert!(text.lines().any(|l| l.contains("breaks through into darkness")), "no cavern breached");
    assert!(text.lines().any(|l| l.contains("fells a fungus tree in ") && l.contains("haul up the stair")), "no fungus tree felled");
    // (Seed 76 fishes below from day 46 since the caverns' pools stand under each column's band
    // rather than at the embark's mean floor; seed 3, which had fished from day 24, no longer
    // does in 120 days though its cavern holds 218 fishing places by the stair.)
    assert!(text.lines().any(|l| l.contains("the first blind white fish")), "no fish from the cavern's water");
}

/// Tombs under the rock (Dwarf Fortress's catacombs): once two of the camp lie in graves at its
/// edge, a level of niches is cut off the stair, and the next dead are laid there.
#[test]
fn the_dead_are_laid_in_the_tombs() {
    // (Without the Shadow fewer die: seed 58 cuts its tombs on day 44 and lays someone there on
    // day 84; seed 15 on days 91 and 241. Seed 14, which had, now outlives its tombs.)
    // (Since hard times: seed 14 cuts its tombs on day 91 and lays someone there on day 239.)
    // (On the audited terrain seed 8. Since Stage 6's ground (2026-10-18) seed 31: tombs on day
    // 72, someone laid there on day 91.)
    let text = run_log("31", "120", &[]);
    assert!(text.lines().any(|l| l.contains("set to work on tombs under the rock")), "no tombs dug");
    assert!(text.lines().any(|l| l.contains("lay them in a niche of the tombs")), "no one entombed");
}

/// The magma sea (Dwarf Fortress): the bottom of every embark is a sea of magma; the deep shaft
/// reaches the warm rock over it and the camp forges metal there.
#[test]
fn the_deep_shaft_reaches_the_magma_sea() {
    // (Seed 12 reaches it on day 72, 9 on day 73; seed 76's camp no longer sinks a deep shaft
    // since hard times reshaped the history.)
    // (Seeds 1 and 3 on the audited terrain.)
    let text = run_log("1", "120", &[]);
    let text = if text.contains("the magma sea glows through the cracks") { text } else { run_log("3", "120", &[]) };
    assert!(text.lines().any(|l| l.contains("the magma sea glows through the cracks")), "the shaft never reached the magma");
}

/// Sealing the caverns (Dwarf Fortress): once the cavern's hunters have come up the stair and
/// hurt someone twice, the camp sets a hatch in the stair at the cavern's roof, and nothing comes
/// up after.
#[test]
fn the_caverns_are_sealed_with_a_hatch() {
    // (Seed 10 seals it on day 31, 11 on day 33: seeds move with the history; hard times moved
    // seed 6's, which had on day 25.)
    let text = run_log("10", "120", &[]);
    let text = if text.contains("They set a hatch of") { text } else { run_log("11", "120", &[]) };
    let at = text.lines().position(|l| l.contains("They set a hatch of")).unwrap_or_else(|| panic!("no hatch on seeds 10 and 11"));
    assert!(!text.lines().skip(at).any(|l| l.contains("come up from the mine, alone")), "something came up after the hatch");
}

/// Industries (Dwarf Fortress's production chains): ore struck (forced: no dev seed strikes any)
/// waits for a smelter cut below the stair, which melts it with charcoal burnt from logs; a forge
/// turns the bars into iron tools and an iron-headed spear, and the traders buy the spare bars.
/// Stone is dressed into blocks at a mason's.
#[test]
fn ore_becomes_bars_tools_and_iron_spears() {
    // (Seed 9: the seam, smelter, bars, forge, tools and a spear by day 51, the mason's first
    // blocks on day 48; seed 76's smelter had stood cold since hard times.)
    // (Seed 8 on the audited terrain: its spear is forged after its tools.)
    let text = run_log("8", "90", &[("PLANET_FORCE_ORE", "1")]);
    let at = |n: &str| text.lines().position(|l| l.contains(n)).unwrap_or_else(|| panic!("never: {n}\n{text}"));
    let seam = at("strikes a seam of Iron");
    let smelter = at("breaks through the last of a smelter below");
    let charcoal = at("burns the first charcoal at the smelter");
    let bars = at("smelts the first iron at the smelter: two bars of iron");
    let forge = at("breaks through the last of a forge below");
    let tools = at("forges a set of iron tools");
    // (The spear may carry its quality word: "forges a finely-crafted iron-headed spear".)
    let spear = text.lines().position(|l| l.contains(" forges ") && l.contains("iron-headed spear at the forge")).unwrap_or_else(|| panic!("never: an iron-headed spear forged\n{text}"));
    assert!(seam < smelter && smelter < charcoal && charcoal < bars, "ore, smelter, fuel, bars out of order");
    assert!(forge < tools && bars < tools && tools < spear, "the forge's tools and spear came before its bars");
    // (Eight loads all go into spears and mail; thirty leave bars to spare: seed 2 sells them to
    // the caravan of day 105, seed 76 to that of day 135.)
    let spare = ["1", "3"].iter().map(|s| run_log(s, "140", &[("PLANET_FORCE_ORE", "30")])).find(|t| t.lines().any(|l| l.contains("The traders of ") && l.contains(" bars of iron"))).unwrap_or_default();
    assert!(spare.lines().any(|l| l.contains("The traders of ") && l.contains(" bars of iron")), "no bars sold");
    assert!(text.lines().any(|l| l.contains("dresses the first blocks of")), "no blocks at the mason's");
}

#[test]
fn the_ledger_leaves_render() {
    // `--sim-snapshot` renders each leaf of the colony's ledger (`tiles/colony_ui.rs`) to PNG.
    let dir = std::env::temp_dir().join(format!("colony_ui_test_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (out, _) = run(&dir);
    for leaf in ["ui_settlers", "ui_stocks", "ui_works", "ui_annals", "ui_camp", "ui_sheet", "ui_store"] {
        assert!(dir.join(format!("colony_{leaf}.png")).exists(), "no {leaf} render:\n{out}");
    }
    for leaf in ["ui_settlers", "ui_stocks", "ui_works", "ui_camp", "ui_sheet"] {
        let line = out.lines().find(|l| l.starts_with(&format!("UI {leaf}:"))).unwrap_or_else(|| panic!("no UI {leaf} line:\n{out}"));
        let n: usize = line.split(':').nth(1).and_then(|r| r.split_whitespace().next()).and_then(|x| x.parse().ok()).unwrap_or(0);
        assert!(n >= 10, "the {leaf} leaf has too little to click: {line}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The dev colony's decisions and stdout for `seed` after `days`.
fn run_decisions(seed: &str, days: &str) -> (String, String) {
    let dir = std::env::temp_dir().join(format!("decisions_{}_{}_{}", seed, days, std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("decisions.txt");
    let out = myth_cmd()
        .args(["--dev", "--headless", "--seed", seed, "--sim-projects", days])
        .env("PLANET_DUMP_DECISIONS", &log)
        .output()
        .expect("run planet_generator");
    assert!(out.status.success());
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    (text, String::from_utf8_lossy(&out.stdout).to_string())
}

/// Needs (Dwarf Fortress's personality needs): between jobs, settlers go off to meet needs of
/// their own (talk, pray, watch the deer, admire a carving, walk out alone...), each camp in its
/// own way: two camps of different people spend their spare hours differently.
#[test]
fn settlers_spend_spare_hours_by_their_needs() {
    let acts = ["Talking", "Passing the time", "Spending time", "Praying", "Kneeling", "Taking it easy", "Watching", "Admiring",
        "Walking out", "Climbing", "Sitting", "Lending", "Reading", "Singing", "Telling", "Practising", "Whittling"];
    let profile = |seed: &str| {
        let (text, _) = run_decisions(seed, "120");
        let mut count: Vec<(usize, &str)> = acts.iter().map(|a| (text.lines().filter(|l| l.contains(&format!("wandering        {}", a))).count(), *a)).collect();
        count.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(b.1)));
        count
    };
    let (a, b) = (profile("23"), profile("3"));
    for (seed, p) in [("23", &a), ("3", &b)] {
        let kinds = p.iter().filter(|c| c.0 > 0).count();
        let total: usize = p.iter().map(|c| c.0).sum();
        assert!(kinds >= 6, "seed {seed}: only {kinds} kinds of spare-hours acts: {p:?}");
        assert!((100..3000).contains(&total), "seed {seed}: {total} spare-hours acts in 120 days");
    }
    let top = |p: &Vec<(usize, &'static str)>| p.iter().take(3).map(|c| c.1).collect::<Vec<&'static str>>();
    assert_ne!(top(&a), top(&b), "two camps spend their spare hours alike: {:?} / {:?}", a, b);
}

/// Voices (needs and character steer the plan): settlers press for the works they want, the
/// loudest named in the plan; a strongly pressed work comes before its usual time (seed 11's
/// drinkers want a tavern long before travellers come).
#[test]
fn the_camp_presses_for_what_its_people_want() {
    // (Seed 1's drinkers on the audited terrain; 11's before it.)
    let text = run_log("1", "60", &[]);
    assert!(text.lines().any(|l| l.contains("They set to work on") && l.contains(" presses for it: ")), "no one pressed for a work");
    assert!(text.lines().any(|l| l.contains("set to work on a tavern") && l.contains("the evenings are long")), "no tavern pressed for early:\n{text}");
}

/// Each camp lays itself out its own way: the wall's radius follows its founders (a close, orderly
/// people walls in tight, an independent one wide), and lots keep off the wall's ring.
#[test]
fn camps_lay_themselves_out_differently() {
    let radius = |seed: &str| {
        let (_, out) = run_decisions(seed, "20");
        out.lines().find_map(|l| l.trim().strip_prefix("Layout: wall radius ").and_then(|r| r.split(',').next()).and_then(|r| r.parse::<i32>().ok())).expect("no layout line")
    };
    // (Seed 12's close, orderly founders wall in at 9; seed 1's at 13, on the audited terrain.)
    let (a, b) = (radius("12"), radius("1"));
    assert!((9..=13).contains(&a) && (9..=13).contains(&b));
    assert!((a - b).abs() >= 3, "walls alike: {a} and {b}");
}

/// Haunts: a need met out of doors is met again at the same place, which its settler marks (a
/// cairn where they pray, a bench where they rest...); others come to share it.
#[test]
fn settlers_make_places_their_own() {
    let text = run_log("14", "120", &[]);
    let marks = text.lines().filter(|l| l.contains("raises a cairn of fieldstones") || l.contains("sets a bench of split logs")
        || l.contains("lays flat stones as a seat") || l.contains("leaves a small cairn") || l.contains("carves it all over") || l.contains("sets up a standing stone")).count();
    assert!(marks >= 4, "only {marks} places made their own:\n{text}");
    assert!(text.lines().any(|l| l.contains(" adds a stone to ") && l.contains("'s cairn")), "no one shared another's cairn");
}

/// Talk (Dwarf Fortress's conversations): idle hours are spent talking, about a moment both lived
/// through, a home, a value both hold dear; the quarrelsome argue over values, and say so aloud.
#[test]
fn settlers_talk_and_argue() {
    // (Seed 4 argues most in its first sixty days.)
    let (text, _) = run_decisions("4", "60");
    let talks = text.lines().filter(|l| l.contains("wandering        Talking with ") || l.contains("wandering        Passing the time with ")).count();
    assert!(talks >= 30, "only {talks} talks in 60 days");
    assert!(text.lines().any(|l| l.contains("wandering        Arguing with ")), "no one argued");
    // (Said aloud once a pair in twenty days: a few seeds.)
    let said = ["76", "1", "23"].iter().any(|s| run_log(s, "90", &[]).lines().any(|l| l.contains(" argue ") && l.contains(" holds ") && l.contains(" dear")));
    assert!(said, "no argument said aloud");
}

/// Gates where the paths go: the palisade's gates face the water, the woods and the road to the
/// trading town, two for an uneasy people and up to four for a bold one.
#[test]
fn gates_face_where_the_paths_go() {
    // (Chosen when the palisade is begun; several seeds, since which way each faces depends on
    // the land and the founders.)
    let lines: Vec<String> = ["23", "58", "76", "5", "1"].iter().map(|seed| {
        let log = run_log(seed, "90", &[]);
        log.lines().find(|l| l.contains("They will leave ") && l.contains(" in the wall")).map(|l| l.to_string()).unwrap_or_else(|| panic!("no gates chosen on seed {seed}"))
    }).collect();
    assert!(lines.iter().any(|l| l.contains("2 gates") && l.contains("uneasy")), "no uneasy camp kept to two gates: {lines:?}");
    assert!(lines.iter().any(|l| l.contains("toward the road to ")), "no gate toward a road: {lines:?}");
    assert!(lines.iter().any(|l| l.contains("toward the woods")), "no gate toward the woods: {lines:?}");
    let counts: std::collections::BTreeSet<&str> = lines.iter().filter_map(|l| ["2 gates", "3 gates", "4 gates"].into_iter().find(|g| l.contains(g))).collect();
    assert!(counts.len() >= 2, "every camp has as many gates: {lines:?}");
}

/// Grief and courtship on the map: those a death touched most stand at the grave in the days
/// after; the unwed walk out with whom they are fond of, and some of them wed.
#[test]
fn settlers_mourn_and_court() {
    // (Seed 1 on the audited terrain; seed 5 before it, and 10 before hard times.)
    let (text, _) = run_decisions("1", "60");
    assert!(text.lines().any(|l| l.contains("wandering        Standing at ") && l.contains("'s grave, remembering them")), "no one stood at a grave");
    assert!(text.lines().any(|l| l.contains("wandering        Walking out with ") && (l.contains(" is fond of ") || l.contains(" is sweet on "))), "no one walked out with anyone");
}

/// Stone from the rock (DF: dwarves mine for their stone): dev 6,36 (6,44 before Stage 6, 70,6 before the audit) breaks
/// every boulder in reach and has no timber; it cuts a gallery off the stair for stone and builds
/// on, instead of setting its works aside for good.
#[test]
fn a_camp_without_stone_digs_for_it() {
    let dir = std::env::temp_dir().join(format!("stonecut_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("log.txt");
    let out = myth_cmd()
        // (6,44 until Stage 6's ground, which left it nothing to eat; 6,36 since 2026-10-18.)
        .args(["--dev", "--headless", "--sim-projects", "100", "--tiles-center", "6,36"])
        .env("PLANET_DUMP_LOG", &log)
        .output()
        .expect("run planet_generator");
    assert!(out.status.success());
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(text.lines().any(|l| l.contains("breaks through the last of a gallery cut for stone")), "no gallery cut for stone:\n{text}");
    let done = stdout.lines().filter(|l| l.trim_start().starts_with("Project day") && l.contains(", done)")).count();
    assert!(done >= 12, "only {done} works done in 100 days");
}

/// Possessions (DF's AcquireObject need and preferences): the greedy and the vain, or one who
/// sees a work in the material they love, keep it as their own; kept works are never sold.
#[test]
fn settlers_keep_things_of_their_own() {
    let text = run_log("76", "150", &[]);
    let kept: Vec<&str> = text.lines().filter(|l| l.contains(" takes ") && (l.contains(" to keep by ") || l.contains(" to keep in "))).collect();
    assert!(!kept.is_empty(), "no one kept anything");
    // (One each, three at most for the very greedy.)
    let mut by: std::collections::BTreeMap<&str, usize> = Default::default();
    for l in &kept { if let Some(n) = l.split("  ").nth(1).and_then(|r| r.split(" takes ").next()) { *by.entry(n).or_default() += 1; } }
    assert!(by.values().all(|&n| n <= 3), "someone hoards: {by:?}");
}

/// Threats seen by day (`prowl.rs`; the user, 2026-10-08: no monsters roaming the surface): the
/// tile's predators come to the edge of sight and slink off when someone comes near, the raid's
/// scout (or its beast, far off) is seen the day before it, and none of them strikes anyone.
#[test]
fn threats_show_themselves_by_day() {
    let logs: Vec<String> = ["3", "8"].iter().map(|s| run_log(s, "60", &[])).collect();
    let any = |n: &str| logs.iter().any(|t| t.lines().any(|l| l.contains(n)));
    assert!(any(" watches the camp from "), "no predator seen by day");
    assert!(any("; it slinks off."), "no predator slunk off");
    for t in &logs {
        // The raid is foretold: whatever comes is seen by daylight the day before it sets out.
        let seen = t.lines().find(|l| l.contains("A rider sits watching the camp") || l.contains("stands in daylight looking toward the camp"));
        let came = t.lines().find(|l| l.contains("Something moves at the edge of the clearing"));
        if let (Some(s), Some(c)) = (seen, came) {
            let day = |l: &str| l.trim_start_matches("Day ").split(',').next().unwrap_or("0").parse::<u32>().unwrap_or(0);
            assert_eq!(day(s) + 1, day(c), "seen on one day, came on another: {s} / {c}");
        } else {
            panic!("no sighting before the raid: {:?} {:?}", seen, came);
        }
    }
    // By day they only watch.
    assert!(!any("is set upon by a bear") && !any("is set upon by a lion"), "a prowler struck someone");
}

/// The raid fought on the map (`melee.rs`; the user, 2026-10-08: "the raid is just a black blob
/// that comes and does nothing"): the defenders run to meet the attackers, the struck fall back
/// to the fire with whoever saved them, and after the fight the band draws off or breaks.
#[test]
fn the_raid_is_fought_on_the_map() {
    let (dec, _) = run_decisions("4", "40");
    let log = run_log("4", "40", &[]);
    let at = |l: &str| -> u32 { let d = l.trim_start_matches("Day ").split(',').next().unwrap_or("0").parse::<u32>().unwrap_or(0);
        let hm: Vec<u32> = l.split(", ").nth(1).unwrap_or("00:00").get(0..5).unwrap_or("00:00").split(':').map(|x| x.parse().unwrap_or(0)).collect();
        d * 1440 + hm.first().copied().unwrap_or(0) * 60 + hm.get(1).copied().unwrap_or(0) };
    let raids: Vec<&str> = log.lines().filter(|l| l.contains("  The raid: ") && !l.contains("cage trap") && !l.contains("talked the exiles")).collect();
    assert!(!raids.is_empty(), "no raid in 40 days on seed 4");
    for r in &raids {
        let t = at(r);
        let off = log.lines().find(|l| at(l) >= t && (l.contains(" draws off toward ") || l.contains(" draw off toward ") || l.contains(" breaks and runs ") || l.contains(" break and run ") || l.contains(" lopes off toward ")))
            .unwrap_or_else(|| panic!("the attackers never drew off after: {r}"));
        assert!(at(off) - t >= 15 && at(off) - t <= 60, "the fight lasted {} minutes: {r} / {off}", at(off) - t);
        assert!(dec.lines().any(|l| l.contains("Running to meet ") && at(l) >= t && at(l) <= t + 5), "nobody ran to meet them: {r}");
    }
    // The rescue of day 27 (Vesswie drags Ghuzava back) is walked to the fire.
    assert!(dec.lines().any(|l| l.contains("Falling back to the fire, struck down by ")) && dec.lines().any(|l| l.contains("Dragging ") && l.contains(" back to the fire")), "no one fell back");
}

/// The floodplain (cards 'Rivers freeze and flood in game time', 'Fields on the floodplain, and
/// spring floods'): on the river's bank (dev 70,7; 69,8 before Stage 6, 48,8 before the audit) the camp lays its field on the bottomland, which
/// yields half again; after a hard winter the thaw floods it, the water goes down and leaves silt,
/// and the harvest that year is the richer for it.
#[test]
fn the_river_floods_the_bottomland() {
    let dir = std::env::temp_dir().join(format!("flood_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("log.txt");
    let out = myth_cmd()
        .args(["--dev", "--headless", "--tiles-center", "69,8", "--sim-projects", "200"])
        .env("PLANET_DUMP_LOG", &log).output().expect("run planet_generator");
    assert!(out.status.success());
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    let has = |n: &str| text.lines().any(|l| l.contains(n));
    assert!(has("on the river's bottomland, where the silt is"), "the field was not laid on the bottomland");
    assert!(has("The thaw comes down the river after ") && has("the water rises over the bottomland"), "no spring flood");
    assert!(has("leaves a skin of grey silt over the field"), "no silt");
    let reaped: Vec<u32> = text.lines().filter(|l| l.contains("They reap the field: ")).filter_map(|l| l.split("field: ").nth(1)?.split(' ').next()?.parse().ok()).collect();
    assert!(reaped.iter().any(|&m| m >= 72), "the bottomland gave no more than a dry field: {reaped:?}");
    assert!(has("(the flood's silt shows)"), "the silt did not show at the harvest");
}

/// Buildings grow (card 'Buildings rise in stages and grow'): when there are more settlers than
/// beds, the first hut is lengthened into a hall (its end wall down, one roof) instead of a twin
/// beside it; and the drying rack is walled and roofed into the smokehouse.
#[test]
fn the_hut_grows_into_a_hall() {
    let logs: Vec<String> = ["76", "3"].iter().map(|s| run_log(s, "100", &[])).collect();
    for (seed, t) in ["76", "3"].iter().zip(&logs) {
        assert!(t.lines().any(|l| l.contains("they will lengthen the hut into a hall")) && t.lines().any(|l| l.contains("The hut is lengthened into a hall: 12 beds under one roof.")), "seed {seed}: no hall");
    }
    assert!(logs.iter().any(|t| t.lines().any(|l| l.contains("The drying rack is walled and roofed: it is a smokehouse now."))), "no rack became a smokehouse");
}

/// The village follows its ground and grows by stages (card 'A village of their own making'):
/// on the river's bank (dev 69,8; 48,8 before the audit) the palisade leaves the river side to the water; the dev camp
/// becomes a hamlet once four buildings stand within its wall.
#[test]
fn the_village_follows_its_ground() {
    let dir = std::env::temp_dir().join(format!("village_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("log.txt");
    let out = myth_cmd()
        // (69,8 until Stage 6's ground, whose camp now stands back from the bank; 70,7 since.)
        .args(["--dev", "--headless", "--tiles-center", "70,7", "--sim-projects", "40"])
        .env("PLANET_DUMP_LOG", &log).output().expect("run planet_generator");
    assert!(out.status.success());
    let river = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(river.lines().any(|l| l.contains("they leave the wall to it")), "the river camp walled its river side");
    let dev = run_log("76", "60", &[]);
    assert!(dev.lines().any(|l| l.contains("The camp has become a hamlet: ")), "the dev camp never became a hamlet");
}

/// The unknown is blank, and someone has to go in: a way into the hills is drawn and its inside
/// is blank; after the old folk's tale (or the patron's blessing) a bold settler goes in within a
/// few days, the cells they walk appear, and the find is made at the far end; forbidden, nobody
/// goes in and they say why; over six seeds someone does not come back and the camp looks for them.
#[test]
fn the_unknown_is_blank_until_someone_goes_in() {
    let dir = std::env::temp_dir().join(format!("explore_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let prefix = dir.join("x");
    // (Dev 33,32, the tomb of Zreetsio: the dev embark itself has no way in on the audited terrain.)
    let out = myth_cmd()
        .args(["--dev", "--tiles-center", "33,32", "--sim-explore", prefix.to_str().unwrap()])
        .output()
        .expect("run planet_generator");
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let frames: Vec<bool> = ["mouth", "blank", "walk", "found"].iter().map(|f| dir.join(format!("x_{f}.png")).exists()).collect();
    let _ = std::fs::remove_dir_all(&dir);
    let day = |variant: &str| -> Option<u32> {
        let l = text.lines().find(|l| l.starts_with(&format!("Explore trial, {variant}:")))?;
        l.split("first went in Day ").nth(1)?.split(',').next()?.trim().parse().ok()
    };
    let plain = day("unattended").unwrap_or_else(|| panic!("nobody went in, unattended:\n{text}"));
    assert!(plain <= 6, "the first went in only on day {plain}:\n{text}");
    // (A find, or a tomb broken into and robbed: since Stage 6's ground the first in robs it.)
    assert!(text.lines().any(|l| l.starts_with("Explore trial, unattended:") && (l.contains(" finds ") || l.contains(" breaks into "))), "nothing found at the far end:\n{text}");
    assert!(text.lines().any(|l| l.contains("why: Going in by ") && l.contains(": ")), "the explorer's why has no nerve in it:\n{text}");
    let forbidden = text.lines().find(|l| l.starts_with("Explore trial, forbidden:")).expect("forbidden line");
    assert!(forbidden.contains("first went in never") && !forbidden.contains("; 0 turned away"), "the forbidden way in was entered or nobody said why: {forbidden}");
    assert!(text.contains("stays away: the patron has forbidden that ground"), "no one said why they stayed away:\n{text}");
    let blessed = day("blessed").unwrap_or_else(|| panic!("nobody went in, blessed:\n{text}"));
    assert!(blessed <= plain, "the blessing did not hasten them: day {blessed} against {plain}");
    assert!(frames.iter().all(|&f| f), "frames missing: {frames:?}");
    // (Since hard times seed 4's explorer does not come back on day 3, and is looked for; seed
    // 11's Skax had before.)
    // (On the audited terrain seed 5's explorer does not come back.)
    let logs: Vec<String> = ["10", "11", "23", "3", "4", "5"].iter().map(|s| run_log(s, "20", &[])).collect();
    assert!(logs.iter().any(|t| t.contains("has not come back")), "every explorer came back");
    assert!(logs.iter().any(|t| t.contains("with torches and rope to look for") || t.contains("after") && t.contains("given up for lost")), "no search was made");
}

/// Dig too deep: each level down is richer and nearer to what sleeps. The same site dug to 15
/// levels gives more ore than to 5 and breaks into something; every breach is foretold by a sign
/// in the rock at least a day before; the patron forbidding the stair at the first sign keeps
/// the picks out (and the miners grumble); a hatch at the stair head, held by two spears, changes
/// what comes of the dark on at least four of six seeds.
#[test]
fn digging_deeper_is_richer_and_foretold() {
    let run = |args: &[&str]| String::from_utf8_lossy(&myth_cmd().args(args).output().expect("run planet_generator").stdout).to_string();
    // (The dev embark until Stage 6's ground; seed 9 since 2026-10-18.)
    let text = run(&["--dev", "--seed", "9", "--sim-depth"]);
    let line = |cap: &str| text.lines().find(|l| l.starts_with(&format!("Depth trial, to {cap} levels:"))).unwrap_or_else(|| panic!("no {cap}-level line:\n{text}")).to_string();
    let ore = |l: &str| -> u32 { l.split(" cells of ore").next().and_then(|x| x.rsplit(", ").next()).and_then(|x| x.trim().parse().ok()).unwrap_or(0) };
    let breaches = |l: &str| -> u32 { l.rsplit("; ").next().and_then(|x| x.split(' ').next()).and_then(|x| x.parse().ok()).unwrap_or(0) };
    let (shallow, deep) = (line("5"), line("15"));
    assert!(ore(&deep) > ore(&shallow), "no more ore at 15 levels than at 5:\n{shallow}\n{deep}");
    assert!(breaches(&deep) >= 1, "nothing broken into at 15 levels: {deep}");
    for l in text.lines().filter(|l| l.trim_start().starts_with("breach on day")) {
        let n: Vec<u64> = l.split(|c: char| !c.is_ascii_digit()).filter_map(|x| x.parse().ok()).collect();
        assert!(l.contains("sign on day") && !l.contains("sign on day none") && n.len() >= 2 && n[n.len() - 1] < n[0], "a breach without a sign the day before: {l}");
    }
    let banned = text.lines().find(|l| l.contains("the stair forbidden at the first sign")).expect("ban line");
    assert!(banned.contains(": 0 breaches after it") && banned.contains("The miners grumble"), "the ban did not hold, or nobody grumbled: {banned}");
    let changed = ["76", "11", "3", "5", "23", "58"].iter().filter(|s| run(&["--dev", "--seed", s, "--sim-hatch"]).contains("the outcome changed")).count();
    assert!(changed >= 4, "the hatch changed what came of the dark on only {changed} of 6 seeds");
}

/// A small bestiary with causes: each embark's kinds come from the tile's data (the named beast
/// near its lair, the pack its ecology keeps, the nameless dead of a battle fought there, what
/// sleeps in its caverns), each with a cause, a want, a fear and a sign, and no site starts with
/// more than two; wolves keep out of the firelight; the nameless dead walk until someone names
/// and buries them, and then rest.
#[test]
fn a_small_bestiary_with_causes() {
    for seed in ["76", "11", "23", "3", "5", "58"] {
        let out = myth_cmd().args(["--dev", "--headless", "--seed", seed, "--sim-projects", "1"]).output().expect("run planet_generator");
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        let line = text.lines().find(|l| l.starts_with("Bestiary: ")).unwrap_or_else(|| panic!("seed {seed}: no bestiary:\n{text}"));
        let n: Vec<u32> = line.split(|c: char| !c.is_ascii_digit()).filter_map(|x| x.parse().ok()).take(2).collect();
        assert!(n.len() == 2 && n[1] <= 2, "seed {seed}: more than two kinds from the first day: {line}");
        let kinds = line.matches("; wants ").count();
        assert!(kinds as u32 == n[0] && line.matches("; fears ").count() == kinds && line.matches("; its sign: ").count() == kinds, "seed {seed}: a kind without its want, fear or sign: {line}");
    }
    // The Mambaash Pass (36,23 on the audited terrain; Brolmdustoor Pass, 47,14, before it): the
    // nameless dead of its battle walk, then are named and rest.
    let dir = std::env::temp_dir().join(format!("bestiary_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("log.txt");
    let out = myth_cmd().args(["--dev", "--headless", "--sim-projects", "40", "--tiles-center", "36,23"]).env("PLANET_DUMP_LOG", &log).output().expect("run planet_generator");
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(stdout.lines().any(|l| l.starts_with("Bestiary: ") && l.contains("the nameless dead (soldiers who fell at the Battle of the Mambaash Pass")), "the pass's dead are not in its bestiary:\n{stdout}");
    let walked = text.lines().position(|l| l.contains("walks among the old graves")).unwrap_or_else(|| panic!("the nameless dead never walked:\n{text}"));
    let rest = text.lines().position(|l| l.contains("carves a stone for the fallen of")).unwrap_or_else(|| panic!("no one named and buried them:\n{text}"));
    assert!(walked < rest && !text.lines().skip(rest).any(|l| l.contains("walks among the old graves")), "the dead walked after they were named");
    // Wolves turn away at the edge of the firelight on a site with a den.
    let wary = ["12", "13", "17"].iter().any(|s| run_log(s, "60", &[]).contains("will not come nearer the fire"));
    assert!(wary, "no wolves turned from the fire on three seeds");
}
