//! The sprite sheet (`--sprite-sheet`): every family of sprites draws, on three pages, and the
//! sprites are not blank (each page has a fair share of ink on its parchment).

use std::process::Command;

#[test]
fn the_sprite_sheet_draws_every_family() {
    let dir = std::env::temp_dir().join(format!("sprites_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("sheet.png");
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator")).args(["--sprite-sheet", path.to_str().unwrap()]).output().expect("run planet_generator");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let n: usize = text.split_whitespace().next().and_then(|x| x.parse().ok()).unwrap_or(0);
    assert!(n >= 190, "only {n} sprites: {text}");
    for page in ["sheet.png", "sheet_folk.png", "sheet_moments.png"] {
        let img = image::open(dir.join(page)).unwrap_or_else(|e| panic!("{page}: {e}")).to_rgb8();
        // Ink: pixels much darker than the parchment.
        let dark = img.pixels().filter(|p| (p[0] as u32 + p[1] as u32 + p[2] as u32) < 300).count();
        let share = dark as f32 / (img.width() * img.height()) as f32;
        assert!(share > 0.01, "{page}: only {:.2}% ink", share * 100.0);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The colony's ledger driven through the window's own click handling (`--ui-drive`): tabs, a
/// settler's sheet stopping the clock, the bar's tools applied on the map, a dream sent from
/// its card, the bell, the clock's buttons, a settler clicked on the map. Every step must do
/// what it should.
#[test]
fn the_ledger_drives_by_clicks() {
    let dir = std::env::temp_dir().join(format!("ui_drive_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator")).args(["--dev", "--ui-drive", dir.join("d").to_str().unwrap()]).output().expect("run planet_generator");
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "the drive failed:\n{text}");
    let line = text.lines().find(|l| l.starts_with("UI drive: ")).unwrap_or_else(|| panic!("no summary:\n{text}"));
    let n: Vec<usize> = line.split_whitespace().filter_map(|w| w.parse().ok()).collect();
    assert!(n.len() >= 2 && n[0] == n[1] && n[1] >= 20, "{line}\n{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The world map's living things: monuments, outlaw camps and a war host on the dev world.
#[test]
fn the_world_map_shows_its_living_things() {
    let dir = std::env::temp_dir().join(format!("world_life_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator")).args(["--dev", "--headless", "--tiles-snapshot", dir.join("w").to_str().unwrap()]).output().expect("run planet_generator");
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let line = text.lines().find(|l| l.starts_with("World life: ")).unwrap_or_else(|| panic!("no world life line:\n{text}"));
    let n: Vec<usize> = line.split(|c: char| !c.is_ascii_digit()).filter_map(|w| w.parse().ok()).collect();
    // monuments, war hosts, sieges, outlaw camps, caravans, wild, cults, beasts
    assert!(n.len() >= 8 && n[0] >= 10 && n[3] >= 1 && n[7] >= 1, "{line}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Every simulated kind placed on the test camp and drawn in the game's own frame
/// (`--inventory`): none left out.
#[test]
fn every_kind_draws_in_the_game_frame() {
    let dir = std::env::temp_dir().join(format!("inventory_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator")).args(["--dev", "--inventory", dir.join("i").to_str().unwrap()]).output().expect("run planet_generator");
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let line = text.lines().find(|l| l.starts_with("Inventory: ")).unwrap_or_else(|| panic!("no inventory line:\n{text}"));
    let n: Vec<usize> = line.split(|c: char| !c.is_ascii_digit()).filter_map(|w| w.parse().ok()).collect();
    assert!(n.len() >= 2 && n[0] >= 60 && n[1] == 0, "{line}");
    assert!(dir.join("i_inventory.png").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn every_simulated_kind_names_its_drawing() {
    let t = planet_generator::tiles::coverage::table();
    let types: std::collections::BTreeSet<&str> = t.iter().map(|r| r.0).collect();
    assert!(types.len() >= 24, "only {} types covered", types.len());
    assert!(t.len() >= 250, "only {} kinds", t.len());
    for (ty, k, d) in &t {
        assert!(!k.is_empty() && !d.is_empty(), "{} {:?} has no drawing", ty, k);
    }
    // Each project kind is listed once.
    let projects: Vec<&String> = t.iter().filter(|r| r.0 == "ProjectKind").map(|r| &r.1).collect();
    let uniq: std::collections::BTreeSet<_> = projects.iter().collect();
    assert_eq!(projects.len(), uniq.len());
}
