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
