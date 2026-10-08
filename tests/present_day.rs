//! Story tests on the dev world: the history must hand the game a present day with open threads,
//! never contradict itself, and tell the same story every run.
//!
//! They run the real binary (`--dev`, ~1 s in release) so they cover the whole pipeline: terrain,
//! climate, history, journal. If one fails after a change to history, the change flattened or
//! broke the inherited story; look at `--dev --present` and the journal before relaxing a bound.

use std::process::Command;

fn run_dev(extra: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--headless"])
        .args(extra)
        .output()
        .expect("run planet_generator");
    assert!(out.status.success(), "planet_generator failed: {}", String::from_utf8_lossy(&out.stderr));
    format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr))
}

/// The number before `label` in the present-day counts line ("..., 5 wars, 0 sieges, ...").
fn count(line: &str, label: &str) -> usize {
    let at = line.find(label).unwrap_or_else(|| panic!("no '{label}' in: {line}"));
    line[..at].trim_end().rsplit(|c: char| !c.is_ascii_digit()).next().unwrap().parse().unwrap()
}

/// Dev seeds the present-day bounds are averaged over: one seed's story reshuffles whenever a
/// change draws from the RNG, so a single seed would fail on luck; the mean over six doesn't.
const SEEDS: [&str; 6] = ["76", "11", "23", "58", "3", "5"];

#[test]
fn dev_world_present_day_has_open_threads() {
    let (mut wars, mut near, mut grudges, mut peoples, mut frontier) = (0, 0, 0, 0, 0);
    let mut sentences = std::collections::HashSet::new();
    let mut shapes = std::collections::HashSet::new();
    for seed in SEEDS {
        let out = run_dev(&["--seed", seed, "--present"]);
        // Each world has its own one-sentence description (lore::claims).
        let records = out.lines().find(|l| l.starts_with("Records: ")).expect("records line");
        assert!(sentences.insert(records.to_string()), "seed {seed} repeats another world's sentence: {records}");
        let line = out.lines().find(|l| l.starts_with("Present day (year")).expect("present-day counts line");
        wars += count(line, " wars") + count(line, " sieges");
        near += line.split("living beasts (").nth(1).map(|s| count(s, " near a town")).unwrap();
        grudges += count(line, " grudges");
        peoples += count(line, " peoples");
        frontier += count(line, " towns on the Shadow's frontier");
        // The Shadow's story has more than one shape; after a victory it can be wounded, and the
        // present knows how.
        let shape = out.lines().find(|l| l.starts_with("The check: ")).unwrap_or("The check: none").to_string();
        if shape.contains("the Last Alliance won") {
            assert!(count(line, " known weakness") >= 1, "seed {seed}: no known weakness of the Shadow after its defeat: {line}");
        }
        shapes.insert(shape);
        // Land long under the Shadow's blight dies (dead woods, ashlands) in the world's biomes.
        let scars = out.lines().find(|l| l.contains(" scarred landscapes")).expect("scars line");
        assert!(count(scars, " killed by the Shadow's blight") >= 1, "seed {seed}: no land killed by the blight: {scars}");
        assert!(out.lines().any(|l| l == "Chronicle: consistent"),
            "seed {seed}: the chronicle contradicts itself:\n{}", out.lines().skip_while(|l| !l.starts_with("Chronicle:")).take(10).collect::<Vec<_>>().join("\n"));
    }
    assert!(shapes.len() >= 2, "the Shadow's story took one shape on all six seeds: {shapes:?}");
    let n = SEEDS.len() as f32;
    let mean = |x: usize| x as f32 / n;
    assert!(mean(wars) >= 1.5, "fewer than 1.5 wars or sieges per world at the present day: {}", mean(wars));
    assert!(mean(near) >= 3.0, "fewer than 3 living beasts near a town per world: {}", mean(near));
    assert!(mean(grudges) >= 5.0, "fewer than 5 grudges per world: {}", mean(grudges));
    assert!(mean(peoples) >= 4.0, "fewer than 4 peoples left per world: {}", mean(peoples));
    assert!(mean(frontier) >= 3.0, "the Shadow has no frontier: {}", mean(frontier));
}

#[test]
fn dev_world_wars_in_every_half_century() {
    let dir = std::env::temp_dir().join(format!("present_day_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("journal.html");
    run_dev(&["--journal", path.to_str().unwrap()]);
    let html = std::fs::read_to_string(&path).unwrap();
    // Year rubrics are `<span class="yr">N</span>`-style anchors; take every "declared war"
    // sentence's year from the nearest year marker before it.
    let mut year = 0u32;
    let mut per_window = [0usize; 5];
    for chunk in html.split("id=\"y").skip(1) {
        if let Some(n) = chunk.split(|c: char| !c.is_ascii_digit()).next().and_then(|d| d.parse().ok()) { year = n; }
        let wars = chunk.matches("declared war on").count() + chunk.matches("declared a holy war on").count()
            + chunk.matches("launched a holy crusade").count() + chunk.matches("unprovoked attack").count()
            + chunk.matches("marched to take back").count();
        if (201..=450).contains(&year) { per_window[((year - 201) / 50) as usize] += wars; }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(per_window.iter().all(|&n| n > 0), "a half-century without a war (years 201-450 by 50): {per_window:?}");
}

#[test]
fn dev_world_history_is_deterministic() {
    let dir = std::env::temp_dir().join(format!("determinism_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (a, b) = (dir.join("a.html"), dir.join("b.html"));
    run_dev(&["--journal", a.to_str().unwrap()]);
    run_dev(&["--journal", b.to_str().unwrap()]);
    let same = std::fs::read(&a).unwrap() == std::fs::read(&b).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(same, "two --dev runs wrote different journals");
}

#[test]
fn a_note_pinned_on_the_map_appears_in_the_journal() {
    let dir = std::env::temp_dir().join(format!("notes_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .current_dir(&dir)
        .args(["--dev", "--headless", "--note", "65,23:My grandmother was born here", "--journal", "j.html"])
        .output()
        .expect("run planet_generator");
    assert!(out.status.success(), "planet_generator failed: {}", String::from_utf8_lossy(&out.stderr));
    let html = std::fs::read_to_string(dir.join("j.html")).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    // In the margin of the annals, beside an entry of that place, and in the Marginalia part.
    let at = html.find("class=\"entry margin\"").expect("no note in the annals' margin");
    assert!(html[at..].contains("My grandmother was born here"));
    assert!(html.contains("id=\"marginalia\""), "no Marginalia part");
}

/// Every living beast is a generated monster (Dwarf Fortress style) whose description never
/// contradicts its body, with a special attack and its warning; beasts differ from each other.
#[test]
fn beasts_are_generated_monsters() {
    let out = run_dev(&["--bestiary"]);
    let lines: Vec<&str> = out.lines().filter(|l| l.contains("(lair ")).collect();
    assert!(lines.len() >= 5, "too few living beasts:\n{out}");
    let mut bodies = std::collections::HashSet::new();
    for l in &lines {
        let d = l.split("): ").nth(1).unwrap_or("");
        assert!(d.contains("Beware") || d.contains("Do not"), "no special attack: {l}");
        assert!(!(d.contains("no fur at all") && d.contains("Its fur")), "words contradict the body: {l}");
        bodies.insert(d.split('.').next().unwrap_or("").to_string());
    }
    assert!(bodies.len() >= lines.len() * 3 / 4, "beasts look alike: {lines:?}");
}

/// The world names its own ages from who held power (Dwarf Fortress style): every dev seed has
/// two or more ages, none called by a bare number, each named for something the world holds,
/// and the present day says which age it is.
#[test]
fn ages_are_named_by_their_powers() {
    let mut kinds = std::collections::HashSet::new();
    for seed in SEEDS {
        let out = run_dev(&["--seed", seed, "--present"]);
        let line = out.lines().find(|l| l.starts_with("The present age: ")).unwrap_or_else(|| panic!("seed {seed}: no present age:\n{out}"));
        let n: usize = line.rsplit('(').next().and_then(|x| x.split_whitespace().next()).and_then(|x| x.parse().ok()).unwrap_or(0);
        assert!(n >= 2, "seed {seed}: one age for the whole history: {line}");
        assert!(!line.contains("Era "), "seed {seed}: a numbered era: {line}");
        let name = line.trim_start_matches("The present age: ");
        kinds.insert(name.split(" of ").next().unwrap_or("").to_string());
    }
    assert!(!kinds.is_empty());
}

/// Every living people has its own arts (Dwarf Fortress's two layers): instruments, then works
/// each credited to a real figure, at a town, in a year; no people repeats a form's shape.
#[test]
fn peoples_have_arts_with_makers() {
    let out = run_dev(&["--arts"]);
    let heads: Vec<&str> = out.lines().filter(|l| l.starts_with("== ")).collect();
    assert!(heads.len() >= 4, "too few peoples with arts:\n{out}");
    let mut people_lines: Vec<Vec<&str>> = Vec::new();
    for l in out.lines() {
        if l.starts_with("== ") { people_lines.push(Vec::new()); } else if l.starts_with("  the ") { if let Some(v) = people_lines.last_mut() { v.push(l); } }
    }
    for works in &people_lines {
        assert!(works.len() >= 4, "a people with few works: {works:?}");
        assert!(works.iter().all(|w| w.contains("(made by ")), "a work with no maker: {works:?}");
        assert!(!works.iter().any(|w| w.contains("the the ")), "doubled article: {works:?}");
        let shapes: std::collections::HashSet<String> = works.iter().map(|w| w.split(", ").nth(1).unwrap_or("").split(" about ").next().unwrap_or("").chars().take(18).collect()).collect();
        assert_eq!(shapes.len(), works.len(), "a people repeats a form: {works:?}");
    }
}

/// Outlaw bands (Dwarf Fortress's wandering groups): the living exiles gather into bands, each led
/// by a real figure and named for the town they lost.
#[test]
fn exiles_gather_into_bands() {
    let out = run_dev(&["--present"]);
    let line = out.lines().find(|l| l.starts_with("Outlaw bands: ")).expect("bands line");
    assert!(line.contains("the Exiles of ") && line.contains(", led by "), "no band with a leader: {line}");
}

/// Generate, validate, reject (Dwarf Fortress): --require throws away worlds that miss their
/// targets, says why, and accepts the first seed that meets them.
#[test]
fn worlds_that_miss_their_targets_are_rejected() {
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--width", "96", "--height", "48", "--seed", "1", "--headless", "--no-history", "--require", "rivers=4,lakes=2,deserts=1"])
        .output()
        .expect("run planet_generator");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("World rejected: seed 1 has "), "seed 1 was not rejected:\n{text}");
    assert!(text.lines().any(|l| l.starts_with("World accepted: seed ")), "no world accepted:\n{text}");
}
