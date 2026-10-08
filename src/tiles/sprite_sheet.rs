//! `--sprite-sheet FILE`: every sprite of the game on one parchment sheet, named, at the size
//! the embark draws it and at four times that, so the whole set can be judged side by side for
//! one hand (and edited against).

use super::beasts::{self, Pose};
use super::ink::Sheet;

const GAME: [&str; 24] = ["red deer", "caribou", "elk", "moose", "wild boar", "pig", "aurochs", "cow", "bison", "antelope", "ibex", "goat",
    "sheep", "horse", "mule", "wolf", "dog", "fox", "bear", "lion", "cat", "hare", "mammoth", "tortoise"];
const CAVE: [&str; 16] = ["bats", "cave crickets", "pale spiders", "glowworms", "blind cave fish", "pale crabs", "cave salamanders", "giant cave spiders",
    "rock grubs", "crawling moles", "giant cave toads", "pale eels", "pale serpents", "drowned crawlers", "things that hunt by sound", "burrowing horrors"];
const NIGHT: [&str; 4] = ["Aephre, risen", "the restless dead", "a werewolf under the full moon", "Thano, changed"];

/// Monsters made on each body base and some of their parts, as the forgotten beasts are.
fn monsters() -> Vec<crate::monsters::Monster> {
    let asks: [(&str, &[&str], &[&str]); 16] = [
        ("quadruped", &["wings"], &["fire"]), ("insect", &["wings", "stinger"], &["plague"]), ("spider", &[], &["darkness"]), ("serpent", &["hood"], &["death"]),
        ("worm", &["mandibles"], &["earth"]), ("humanoid", &["horns"], &["fire"]), ("bird", &[], &["storm"]), ("blob", &["tentacles"], &["water"]),
        ("crustacean", &[], &["water"]), ("lizard", &["wings"], &["fire"]), ("quadruped", &["shell", "spines"], &["earth"]), ("quadruped", &["trunk", "tusks"], &["nature"]),
        ("lizard", &["spines"], &["cold"]), ("humanoid", &["tusks"], &["darkness"]), ("insect", &["mandibles"], &["night"]), ("serpent", &["wings"], &["sky"]),
    ];
    asks.iter().enumerate().map(|(k, (base, must, spheres))| {
        let req = crate::monsters::Request { kind: "forgotten".into(), spheres: spheres.iter().map(|s| s.to_string()).collect(), size: 2.0 + k as f32 * 0.1,
            must: must.iter().map(|s| s.to_string()).collect(), base: Some(base.to_string()), evil: k % 2 == 0, ..Default::default() };
        crate::monsters::generate(&req, 0x5EE7 + k as u64 * 7919)
    }).collect()
}

pub fn save(path: &str) -> Result<usize, Box<dyn std::error::Error>> {
    let mons = monsters();
    let n = GAME.len() + CAVE.len() + NIGHT.len() + mons.len();
    let cols = 8;
    let mut sheet = Sheet::new(cols, (n + cols - 1) / cols, 210, 170);
    sheet.title("The bestiary: game, cave life, the night's things and the beasts of the deep");
    let draw = |sheet: &mut Sheet, label: &str, look: &beasts::Look| {
        let (cx, cy) = sheet.cell(label);
        let small = beasts::px_for(look, 1.0);
        let big = (small * 2.4).clamp(60.0, 130.0);
        let mut put = sheet.put();
        beasts::draw(&mut put, look, cx - 30.0, cy + 30.0, big, false, Pose::Walk(true), 1.0);
        beasts::draw(&mut put, look, cx + 72.0, cy + 40.0, small, true, Pose::Stand, 1.0);
    };
    for name in GAME { draw(&mut sheet, name, &beasts::of_name(name)); }
    for name in CAVE { draw(&mut sheet, name, &beasts::of_name(name)); }
    for name in NIGHT { draw(&mut sheet, name, &beasts::of_name(name)); }
    for m in &mons {
        let short = m.short.trim_start_matches("a ").trim_start_matches("an ");
        let label: String = short.chars().take(26).collect();
        draw(&mut sheet, &label, &beasts::of_monster(m));
    }
    sheet.save(path)?;
    Ok(n)
}
