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
    for seed in SEEDS {
        let out = run_dev(&["--seed", seed]);
        // Each world has its own one-sentence description (lore::claims).
        let records = out.lines().find(|l| l.starts_with("Records: ")).expect("records line");
        assert!(sentences.insert(records.to_string()), "seed {seed} repeats another world's sentence: {records}");
        let line = out.lines().find(|l| l.starts_with("Present day (year")).expect("present-day counts line");
        wars += count(line, " wars") + count(line, " sieges");
        near += line.split("living beasts (").nth(1).map(|s| count(s, " near a town")).unwrap();
        grudges += count(line, " grudges");
        peoples += count(line, " peoples");
        frontier += count(line, " towns on the Shadow's frontier");
        // The Shadow's story ends on a cliffhanger: it can be wounded, and the present knows how.
        assert!(count(line, " known weakness") >= 1, "seed {seed}: no known weakness of the Shadow: {line}");
        assert!(out.lines().any(|l| l == "Chronicle: consistent"),
            "seed {seed}: the chronicle contradicts itself:\n{}", out.lines().skip_while(|l| !l.starts_with("Chronicle:")).take(10).collect::<Vec<_>>().join("\n"));
    }
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
