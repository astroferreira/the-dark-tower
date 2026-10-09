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
    let mut sheet = Sheet::new(cols, (n + cols - 1) / cols, 240, 190);
    sheet.title("The bestiary: game, cave life, the night's things and the beasts of the deep");
    let draw = |sheet: &mut Sheet, label: &str, look: &beasts::Look| {
        let (cx, cy) = sheet.cell(label);
        let small = beasts::px_for(look, 1.0);
        let big = (small * 1.8).clamp(80.0, 150.0);
        let mut put = sheet.put();
        beasts::draw(&mut put, look, cx - 36.0, cy + 34.0, big, false, Pose::Walk(true), 1.0);
        beasts::draw(&mut put, look, cx + 80.0, cy + 44.0, small, true, Pose::Stand, 1.0);
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
    let n2 = save_people_and_things(&path.replace(".png", "_folk.png"))?;
    let n3 = save_vignettes(&path.replace(".png", "_moments.png"))?;
    Ok(n + n2 + n3)
}

/// The second page: the bubbles over settlers, strangers, furniture and fittings.
fn save_people_and_things(path: &str) -> Result<usize, Box<dyn std::error::Error>> {
    use super::status_ink::Emblem;
    let emblems = [Emblem::Tantrum, Emblem::Despair, Emblem::Lost, Emblem::Fey, Emblem::Secretive, Emblem::Possessed, Emblem::Macabre, Emblem::Fell,
        Emblem::Hurt, Emblem::Pray, Emblem::Talk, Emblem::Rest, Emblem::Watch, Emblem::Admire, Emblem::Walk, Emblem::Thrill, Emblem::Help, Emblem::Learn,
        Emblem::Think, Emblem::Merry, Emblem::Tale, Emblem::Martial, Emblem::Whittle, Emblem::Busy, Emblem::Drink, Emblem::Meal, Emblem::Gloom, Emblem::Love, Emblem::Grief, Emblem::Acquire, Emblem::Dreaming];
    let folk: Vec<(&str, super::folk::Folk)> = vec![
        ("the Shadow's raider", super::folk::raider(None, "raiders of the Shadow of Skullfang", 0, None)),
        ("the Shadow's axeman", super::folk::raider(None, "raiders of the Shadow of Skullfang", 2, None)),
        ("a war band's spear", super::folk::raider(None, "a war band of The Git Clans", 1, None)),
        ("a war band's axe", super::folk::raider(None, "a war band of The Git Clans", 3, None)),
        ("an outlaw with a club", super::folk::raider(None, "a band of outlaws", 0, None)),
        ("an outlaw with a bow", super::folk::raider(None, "a band of outlaws", 1, None)),
        ("a trader", super::folk::trader("traders of Ripu", 0)),
        ("a porter", super::folk::trader("traders of Ripu", 1)),
    ];
    let furn: [(&str, fn(&mut super::ink::Pen)); 14] = [
        ("a bed", |p| super::furniture::bed(p, [92.0, 120.0, 82.0], false)), ("a fine bed", |p| super::furniture::bed(p, [150.0, 66.0, 52.0], true)),
        ("a pallet", |p| super::furniture::pallet(p)), ("a coffin", |p| super::furniture::coffin(p, true)), ("a bench", |p| super::furniture::bench(p, true)),
        ("fungus beds", |p| super::furniture::fungus_bed(p, 3)), ("the mason's", |p| super::furniture::mason(p)), ("the carpenter's", |p| super::furniture::carpenter(p)),
        ("the smelter", |p| super::furniture::smelter(p, [190.0, 112.0, 70.0])), ("the kiln", |p| super::furniture::kiln(p)), ("the forge", |p| super::furniture::forge(p, false)),
        ("the cellar", |p| super::furniture::cellar_stores(p, 0)), ("the hatch", |p| super::furniture::hatch(p)), ("a lair's hoard", |p| super::furniture::lair(p)),
    ];
    let arts = ["a figurine", "a chest", "pipes", "a drum", "a harp", "a crown", "a ring", "a goblet", "a sword", "a book", "a carved stone"];
    let glyph_all = super::glyphs::Glyph::ALL;
    let n = emblems.len() + folk.len() + 10 + furn.len() + arts.len() + glyph_all.len();
    let cols = 9;
    let mut sheet = Sheet::new(cols, (n + cols - 1) / cols, 150, 130);
    sheet.title("People and things: what settlers go through, strangers, furniture below, artifacts");
    for e in emblems {
        let (cx, cy) = sheet.cell(&format!("{:?}", e).to_lowercase());
        let mut put = sheet.put();
        super::status_ink::draw_bubble(&mut put, e, cx - 30.0, cy + 20.0, 2.4);
        super::status_ink::draw_bubble(&mut put, e, cx + 40.0, cy + 20.0, 0.85);
    }
    // Guests from the world, by their calling: a settler's figure with the guest's marks.
    for calling in ["a monster hunter of Titankeep", "a teller of tales of Ripu", "a loremaster of the Git Clans", "a seeker of lost things", "a sellsword of Badgerd"] {
        let (cx, cy) = sheet.cell(calling.split(" of ").next().unwrap_or(calling));
        let mut put = sheet.put();
        let mut f = super::folk::trader(calling, 5);
        f.arm = super::folk::Arm::None; f.helm = super::folk::Helm::None;
        let sc = 3.0;
        super::folk::draw(&mut put, &f, cx, cy + 10.0, sc, false, false, 1.0);
        // The head's centre and radius as folk::draw has them (unit box 22 px at scale 1).
        let size = 22.0 * sc;
        let (hx, hy, hr) = (cx, cy + 10.0 - 2.0 * sc + (-0.32) * size / 2.0, 0.38 * size / 2.0);
        let mut pen = super::ink::Pen::new(&mut put, hx, hy, hr * 2.0);
        super::status_ink::guest_marks(&mut pen, calling);
    }
    for office in ["Lord of the camp", "Keeps the temple of Ishra", "Speaks for the camp", "Tends the wounded", "Cooks for the camp"] {
        let (cx, cy) = sheet.cell(office);
        let mut put = sheet.put();
        let mut f = super::folk::trader(office, 6);
        f.arm = super::folk::Arm::None; f.helm = super::folk::Helm::None;
        let sc = 3.0;
        super::folk::draw(&mut put, &f, cx, cy + 10.0, sc, false, false, 1.0);
        let size = 22.0 * sc;
        let (hx, hy, hr) = (cx, cy + 10.0 - 2.0 * sc + (-0.32) * size / 2.0, 0.38 * size / 2.0);
        let mut pen = super::ink::Pen::new(&mut put, hx, hy, hr * 2.0);
        super::status_ink::office_marks(&mut pen, office);
        drop(pen);
        // And at the size the camp draws it.
        let sc = 1.25;
        super::folk::draw(&mut put, &f, cx + 70.0, cy + 30.0, sc, false, false, 1.0);
        let size = 22.0 * sc;
        let (hx, hy, hr) = (cx + 70.0, cy + 30.0 - 2.0 * sc + (-0.32) * size / 2.0, 0.38 * size / 2.0);
        let mut pen = super::ink::Pen::new(&mut put, hx, hy - (hr.max(7.0) - hr) * 0.5, hr.max(7.0) * 2.0);
        super::status_ink::office_marks(&mut pen, office);
    }
    for (label, f) in &folk {
        let (cx, cy) = sheet.cell(label);
        let mut put = sheet.put();
        super::folk::draw(&mut put, f, cx - 25.0, cy + 10.0, 3.0, false, label.contains("axe") || label.contains("club"), 1.0);
        super::folk::draw(&mut put, f, cx + 45.0, cy + 20.0, 1.2, true, false, 1.0);
    }
    for (label, f) in furn {
        let (cx, cy) = sheet.cell(label);
        let mut put = sheet.put();
        let mut pen = super::ink::Pen::new(&mut put, cx - 40.0, cy - 30.0, 64.0);
        f(&mut pen);
    }
    for a in arts {
        let (cx, cy) = sheet.cell(a);
        let mut put = sheet.put();
        let mut pen = super::ink::Pen::new(&mut put, cx - 32.0, cy - 36.0, 128.0);
        super::furniture::artifact(&mut pen, a);
    }
    for g in glyph_all {
        let (cx, cy) = sheet.cell(&format!("{:?}", g).to_lowercase());
        let mut put = sheet.put();
        super::glyphs::draw(&mut put, g, cx - 22.0, cy, 48.0, None);
        super::glyphs::draw(&mut put, g, cx + 40.0, cy + 10.0, 17.0, None);
    }
    sheet.save(path)?;
    Ok(n)
}

/// The third page: the moment cards' roundels for a range of moments.
fn save_vignettes(path: &str) -> Result<usize, Box<dyn std::error::Error>> {
    let moments = ["The raid", "They are coming", "A storehouse finished", "A tavern finished", "A temple finished", "A library finished", "A palisade finished",
        "A woodpile finished", "A fenced field finished", "A well finished", "The hut stands", "The ghost of Aephre", "Thano dies", "A child is born",
        "A caravan from Ripu", "Refugees", "Aephre the Wanderer comes", "A rumour", "Boutrurn's dream", "The camp's woodcutter", "The camp's builder",
        "Gaunauth speaks for the camp", "A lord comes", "Austourd breaks", "A fey mood", "Water in the rock", "Amber in the rock", "A sound from below",
        "A cave in the shale", "Winter comes", "Austourd tends the wounded", "The siege"];
    let cols = 8;
    let mut sheet = Sheet::new(cols, (moments.len() + cols - 1) / cols, 150, 150);
    sheet.title("The moments' roundels");
    for t in moments {
        let (cx, cy) = sheet.cell(t);
        let m = crate::colony::Moment { tick: 0, title: t.to_string(), text: String::new(), because: "because".into(), at: (0, 0), choice: false };
        let mut put = sheet.put();
        super::vignette::draw(&mut put, None, &m, cx, cy, 52.0);
    }
    sheet.save(path)?;
    Ok(moments.len())
}
