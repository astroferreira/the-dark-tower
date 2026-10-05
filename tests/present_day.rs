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

#[test]
fn dev_world_present_day_has_open_threads() {
    let out = run_dev(&[]);
    let line = out.lines().find(|l| l.starts_with("Present day (year")).expect("present-day counts line");
    let (wars, sieges) = (count(line, " wars"), count(line, " sieges"));
    let near_town = line.split("living beasts (").nth(1).map(|s| count(s, " near a town")).unwrap();
    assert!(wars + sieges >= 2, "fewer than 2 wars or sieges at the present day: {line}");
    assert!(near_town >= 3, "fewer than 3 living beasts near a town: {line}");
    assert!(count(line, " grudges") >= 5, "fewer than 5 grudges: {line}");
    assert!(count(line, " peoples") >= 4, "fewer than 4 peoples left: {line}");
    assert!(count(line, " towns on the Shadow's frontier") >= 3, "the Shadow has no frontier: {line}");
    assert!(out.lines().any(|l| l == "Chronicle: consistent"),
        "the chronicle contradicts itself:\n{}", out.lines().skip_while(|l| !l.starts_with("Chronicle:")).take(10).collect::<Vec<_>>().join("\n"));
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
