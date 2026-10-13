//! Towns on an embark stand in three dimensions (`local/structures.rs`, DF's site realizations):
//! the dev world's capital has a keep with a roof to walk on, upper storeys and cellars, all
//! walked to from the street, and the ground stays solid but for what was cut; a stone wall has
//! towers. Runs the real binary (`--local-snapshot`, a few seconds once the dev region is cached).

use std::process::Command;

fn snapshot(name: &str, extra: &[&str], env: &[(&str, &str)]) -> String {
    let dir = std::env::temp_dir().join(format!("towns_{}_{}", name, std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let prefix = dir.join("t");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_planet_generator"));
    cmd.args(["--dev", "--headless", "--local-snapshot", prefix.to_str().unwrap()]).args(extra);
    for (k, v) in env { cmd.env(k, v); }
    let out = cmd.output().expect("run planet_generator");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(out.status.success(), "planet_generator failed: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// "N of M walked to from the street" for a line starting with `what`.
fn walked(text: &str, what: &str) -> (usize, usize) {
    let line = text.lines().find(|l| l.trim_start().starts_with(what)).unwrap_or_else(|| panic!("no '{what}' line:\n{text}"));
    let nums: Vec<usize> = line.split(|c: char| !c.is_ascii_digit()).filter_map(|n| n.parse().ok()).collect();
    (nums[0], nums[1])
}

#[test]
fn towns_stand_on_several_levels() {
    // The dev world's largest town (a capital): a keep, storeys, cellars.
    let text = snapshot("capital", &[], &[]);
    let town = text.lines().find(|l| l.starts_with("Town in three dimensions")).unwrap_or_else(|| panic!("no town line:\n{text}"));
    let nums: Vec<usize> = town.split(|c: char| !c.is_ascii_digit()).filter_map(|n| n.parse().ok()).collect();
    // buildings, ruined, two storeys, lofts, keeps, towers, cellars, ...
    assert!(nums[2] >= 100, "few upper storeys: {town}");
    assert_eq!(nums[4], 1, "no keep: {town}");
    assert!(nums[6] >= 50, "few cellars: {town}");
    assert_eq!(walked(&text, "keep roofs:"), (1, 1), "the keep's roof can't be reached:\n{text}");
    // A few houses on the embark's edge have their doors off the map.
    let (a, b) = walked(&text, "upper floors:");
    assert!(a * 100 >= b * 97, "upper floors out of reach: {a} of {b}");
    let (a, b) = walked(&text, "cellars:");
    assert!(a * 100 >= b * 97, "cellars out of reach: {a} of {b}");
    assert!(text.contains("below the ground: 0 cells open"), "ground cut where nothing was dug:\n{text}");

    // Dreammere's stone wall (731 m out from its middle): towers a level above the wall's walk.
    let text = snapshot("wall", &["--tiles-center", "71,12"], &[("PLANET_LOCAL_OFFSET", "731,0")]);
    let (a, b) = walked(&text, "tower roofs:");
    assert!(b >= 2 && a == b, "towers on the wall not reached: {a} of {b}\n{text}");
}

/// Neighbouring embarks join (card 'Neighbouring embarks join seamlessly'): the levels across the
/// edge between two embarks step no more than inside one, within a tile, across a tile's border
/// and across the date line; and the same embark made from two regions (one across the date line)
/// is the same ground.
#[test]
fn neighbouring_embarks_join() {
    for tile in ["48,18", "95,10"] {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_planet_generator"))
            .args(["--dev", "--embark-seams", "--tiles-center", tile]).output().expect("run planet_generator");
        assert!(out.status.success());
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        let seams: Vec<&str> = text.lines().filter(|l| l.starts_with("Seam ")).collect();
        assert_eq!(seams.len(), 3, "{text}");
        for l in &seams {
            let num = |after: &str| -> f32 { l.split(after).nth(1).and_then(|r| r.trim_start().split(|c: char| !(c.is_ascii_digit() || c == '.')).next()).and_then(|v| v.parse().ok()).unwrap_or(f32::MAX) };
            let (across, inside, most) = (num("differ "), num("inside one embark "), num("at most "));
            assert!(across <= inside + 0.15 && most <= 2.0, "{tile}: {l}");
        }
        let two = text.lines().find(|l| l.starts_with("Two regions")).unwrap_or_else(|| panic!("{text}"));
        assert!(two.contains("differs 0.00 m on average"), "{tile}: {two}");
    }
}
