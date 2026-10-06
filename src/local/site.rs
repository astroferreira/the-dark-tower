//! A site worth settling: what an embark needs within reach, and what it has.
//!
//! A wild embark used to be a flat green square with a river and one animal trail. A colony's
//! first years need, within about 100 m of the centre: water, wood, stone, food to gather and
//! good ground, and something to look at. `furnish` adds what nature left out, in keeping with
//! the place: a spring pool on a dry site, a grove where there are too few trees (the hut needs
//! logs), an outcrop where there is no stone, berry thickets, and a landmark (graves where the
//! history fought a battle on this ground, else a standing stone). `report` lists what a site
//! holds; `--local-snapshot` prints it and walking mode shows it before embarking.

use super::wildlife::Feature;
use super::{LocalMap, Material, Plant, Shape, TreeKind, WATER_FULL};

/// The reach of a first camp: cells (2 m) from the centre.
const REACH: i64 = 48;

fn hash(a: i64, b: i64, salt: u64) -> u64 {
    let mut h = (a as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (b as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ salt;
    h ^= h >> 31;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^ (h >> 29)
}

/// A free surface cell for something new: open floor, dry, not built on, no feature yet.
fn free(map: &LocalMap, i: usize, j: usize) -> bool {
    let sz = map.surface_z[j * map.width + i];
    if sz < 0 || sz as usize + 1 >= map.depth { return false; }
    let c = map.cell(i, j, sz as usize);
    let above = map.cell(i, j, sz as usize + 1);
    c.shape == Shape::Floor && c.water == 0 && above.water == 0 && above.shape == Shape::Empty
        && !matches!(c.material, Material::Wood | Material::Block(_) | Material::Ice | Material::Snow)
        && !matches!(c.plant, Plant::Crop(_)) && map.features[j * map.width + i] == Feature::None
}

/// Cells in reach of the centre, in a fixed order.
fn reach(map: &LocalMap) -> impl Iterator<Item = (usize, usize)> + '_ {
    let (cx, cy) = (map.width as i64 / 2, map.height as i64 / 2);
    (cy - REACH..=cy + REACH).flat_map(move |y| (cx - REACH..=cx + REACH).map(move |x| (x, y)))
        .filter(move |&(x, y)| x >= 0 && y >= 0 && (x as usize) < map.width && (y as usize) < map.height)
        .map(|(x, y)| (x as usize, y as usize))
}

/// A spot in reach for a cluster of `r` cells: the free cell whose disc has the most free cells,
/// sampled deterministically.
fn spot(map: &LocalMap, salt: u64, r: i64) -> Option<(usize, usize)> {
    let mut best: Option<(usize, (usize, usize))> = None;
    let cand: Vec<(usize, usize)> = reach(map).filter(|&(i, j)| hash(i as i64, j as i64, salt) % 37 == 0 && free(map, i, j)).collect();
    for &(i, j) in &cand {
        let n = (-r..=r).flat_map(|dy| (-r..=r).map(move |dx| (dx, dy))).filter(|&(dx, dy)| dx * dx + dy * dy <= r * r)
            .filter(|&(dx, dy)| { let (x, y) = (i as i64 + dx, j as i64 + dy); x >= 0 && y >= 0 && (x as usize) < map.width && (y as usize) < map.height && free(map, x as usize, y as usize) })
            .count();
        if best.map_or(true, |b| n > b.0) { best = Some((n, (i, j))); }
    }
    best.map(|b| b.1)
}

/// Put `f` on up to `n` free cells within `r` of `at`, in hash order.
fn scatter(map: &mut LocalMap, at: (usize, usize), r: i64, n: usize, salt: u64, mut put: impl FnMut(&mut LocalMap, usize, usize)) -> usize {
    let mut cells: Vec<(u64, usize, usize)> = (-r..=r).flat_map(|dy| (-r..=r).map(move |dx| (dx, dy)))
        .filter(|&(dx, dy)| dx * dx + dy * dy <= r * r)
        .map(|(dx, dy)| (at.0 as i64 + dx, at.1 as i64 + dy))
        .filter(|&(x, y)| x >= 0 && y >= 0 && (x as usize) < map.width && (y as usize) < map.height)
        .map(|(x, y)| (hash(x, y, salt), x as usize, y as usize))
        .collect();
    cells.sort();
    let mut done = 0;
    for (_, x, y) in cells {
        if done >= n { break; }
        if !free(map, x, y) { continue; }
        put(map, x, y);
        done += 1;
    }
    done
}

/// Laid or built ground: timber floors, dressed stone (houses, streets, ruins).
fn built(m: Material) -> bool { matches!(m, Material::Wood | Material::Block(_)) }

#[derive(Default)]
struct Counts { water: usize, trees: usize, shrubs: usize, stone: usize, landmark: usize }

fn count(map: &LocalMap) -> Counts {
    let mut c = Counts::default();
    for (i, j) in reach(map) {
        let sz = map.surface_z[j * map.width + i].max(0) as usize;
        let cell = map.cell(i, j, sz);
        let above = if sz + 1 < map.depth { map.cell(i, j, sz + 1).water } else { 0 };
        if above > 0 || cell.material == Material::Ice { c.water += 1; }
        match cell.plant { Plant::Tree(_) => c.trees += 1, Plant::Shrub => c.shrubs += 1, _ => {} }
        if cell.boulder || matches!(cell.material, Material::Rock(_)) { c.stone += 1; }
        if matches!(map.features[j * map.width + i], Feature::Grave | Feature::Stone | Feature::Bones) || built(cell.material) { c.landmark += 1; }
    }
    c
}

/// Add what the site lacks within reach. `battles` are those fought on this world tile
/// (title, year, the named dead): their graves become the landmark.
pub fn furnish(map: &mut LocalMap, battles: &[(String, u32, Vec<String>)]) {
    let salt = hash(map.world_tile.0 as i64, map.world_tile.1 as i64, 0x517E);
    let have = count(map);
    map.found = [have.water, have.trees, have.stone, have.shrubs];
    // Water: a spring and its pool where the site is dry.
    if have.water == 0 {
        if let Some(at) = spot(map, salt ^ 1, 2) {
            map.furnished.push("a spring".into());
            let k = at.1 * map.width + at.0;
            map.features[k] = Feature::Spring;
            scatter(map, at, 2, 9, salt ^ 2, |m, x, y| {
                let sz = m.surface_z[y * m.width + x] as usize;
                let i = m.idx(x, y, sz + 1);
                m.cells[i].water = WATER_FULL;
                let f = m.idx(x, y, sz);
                m.cells[f].plant = Plant::None;
            });
        }
    }
    // Wood: a grove (the hut needs logs), of the trees that grow here if any do.
    if have.trees < 20 {
        let kind = reach(map).find_map(|(i, j)| match map.cell(i, j, map.surface_z[j * map.width + i].max(0) as usize).plant { Plant::Tree(k) if k != TreeKind::Dead => Some(k), _ => None })
            .unwrap_or(TreeKind::Broadleaf);
        if let Some(at) = spot(map, salt ^ 3, 6) {
            map.furnished.push("a grove".into());
            scatter(map, at, 6, 24, salt ^ 4, |m, x, y| { let i = m.idx(x, y, m.surface_z[y * m.width + x] as usize); m.cells[i].plant = Plant::Tree(kind); });
        }
    }
    // Stone: an outcrop of bare rock and boulders.
    if have.stone < 6 {
        let rock = (0..map.depth).rev().find_map(|z| match map.cell(map.width / 2, map.height / 2, z).material { Material::Rock(r) => Some(r), _ => None });
        if let (Some(at), Some(rock)) = (spot(map, salt ^ 5, 3), rock) {
            map.furnished.push("an outcrop of stone".into());
            scatter(map, at, 3, 20, salt ^ 6, |m, x, y| {
                let i = m.idx(x, y, m.surface_z[y * m.width + x] as usize);
                m.cells[i].material = Material::Rock(rock);
                m.cells[i].plant = Plant::None;
                m.cells[i].boulder = hash(x as i64, y as i64, 0xB0) % 3 == 0;
            });
        }
    }
    // Food: berry thickets.
    if have.shrubs < 15 {
        if let Some(at) = spot(map, salt ^ 7, 4) {
            map.furnished.push("a berry thicket".into());
            scatter(map, at, 4, 18, salt ^ 8, |m, x, y| { let i = m.idx(x, y, m.surface_z[y * m.width + x] as usize); m.cells[i].plant = Plant::Shrub; });
        }
    }
    // Something to look at: the graves of a battle fought here, else a standing stone.
    // Each grave says who lies there: the named dead first, then the nameless of each battle.
    let (mut words, mut nameless): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    for (title, year, named) in battles {
        let place = match title.strip_prefix("The ") { Some(rest) => format!("the {}", rest), None => title.clone() };
        // "X falls at Ripu" (a captain on the walls) names the place itself.
        let place = place.split(" falls at ").nth(1).map(|p| p.to_string()).unwrap_or(place);
        for n in named { words.push(format!("{}. Fell at {} in {}.", n, place, year)); }
        if !title.contains(" falls at ") {
            for _ in 0..3 { nameless.push(format!("A soldier whose name is lost. Fell at {} in {}.", place, year)); }
        }
    }
    words.extend(nameless);
    words.truncate(16);
    if !words.is_empty() {
        if let Some(at) = spot(map, salt ^ 9, 4) {
            let mut k = 0;
            let mut placed = Vec::new();
            scatter(map, at, 4, words.len(), salt ^ 10, |m, x, y| { m.features[y * m.width + x] = Feature::Grave; placed.push((x, y)); });
            for (x, y) in placed { if k < words.len() { map.graves.push((x, y, words[k].clone())); k += 1; } }
        }
    } else if have.landmark == 0 {
        if let Some(at) = spot(map, salt ^ 11, 1) { map.features[at.1 * map.width + at.0] = Feature::Stone; }
    }
}

/// What a site holds within reach, one item per kind of resource: "water (river)", "wood
/// (312 trees)", ...
pub fn report(map: &LocalMap) -> Vec<String> {
    let c = count(map);
    let mut out = Vec::new();
    let spring = reach(map).any(|(i, j)| map.features[j * map.width + i] == Feature::Spring);
    if c.water > 0 { out.push(format!("water ({})", if spring { "a spring" } else if c.water > 200 { "river or lake" } else { "a pool" })); }
    if c.trees > 0 { out.push(format!("wood ({} trees)", c.trees)); }
    if c.stone > 0 { out.push(format!("stone ({} cells)", c.stone)); }
    if c.shrubs > 0 { out.push(format!("berries ({} bushes)", c.shrubs)); }
    let soil = reach(map).filter(|&(i, j)| matches!(map.cell(i, j, map.surface_z[j * map.width + i].max(0) as usize).material, Material::Soil | Material::Clay)).count();
    if soil > 500 { out.push(format!("soil ({}% of the ground)", 100 * soil / ((2 * REACH as usize + 1).pow(2)))); }
    let game = reach(map).filter(|&(i, j)| matches!(map.features[j * map.width + i], Feature::Trail | Feature::Burrow | Feature::Nest | Feature::Den)).count();
    if game > 0 { out.push("game (trails and burrows)".into()); }
    let graves = reach(map).filter(|&(i, j)| map.features[j * map.width + i] == Feature::Grave).count();
    let stone = reach(map).any(|(i, j)| map.features[j * map.width + i] == Feature::Stone);
    let ruin = reach(map).any(|(i, j)| built(map.cell(i, j, map.surface_z[j * map.width + i].max(0) as usize).material));
    if graves > 0 { out.push(format!("graves ({})", graves)); }
    if reach(map).any(|(i, j)| map.features[j * map.width + i] == Feature::Bones) { out.push("old bones".into()); }
    if stone { out.push("a standing stone".into()); }
    if ruin { out.push("buildings".into()); }
    if !map.furnished.is_empty() { out.push(format!("(the land was short of these; added: {})", map.furnished.join(", "))); }
    out
}

/// One gift and one scarcity of a site, counted before furnishing: ("a river and 900 trees",
/// "stone").
pub fn gift_and_lack(map: &LocalMap) -> (String, String) {
    // Counted within 40 cells of the centre (a camp's foraging ground; the site's own reach is
    // smaller and missed rivers), less what furnishing put there.
    let (n, c) = (map.width as i64, (map.width / 2) as i64);
    let (mut water, mut trees, mut stone, mut shrubs) = (0usize, 0usize, 0usize, 0usize);
    for y in (c - 40).max(0)..(c + 40).min(n) {
        for x in (c - 40).max(0)..(c + 40).min(n) {
            let (i, j) = (x as usize, y as usize);
            let sz = map.surface_z[j * map.width + i].max(0) as usize;
            let cell = map.cell(i, j, sz);
            if sz + 1 < map.depth && map.cell(i, j, sz + 1).water > 0 { water += 1; }
            match cell.plant { Plant::Tree(_) => trees += 1, Plant::Shrub => shrubs += 1, _ => {} }
            if cell.boulder || matches!(cell.material, Material::Rock(_)) { stone += 1; }
        }
    }
    // Water a longer walk away (the river beyond the clearing).
    let mut far_water = 0usize;
    for y in (c - 70).max(0)..(c + 70).min(n) {
        for x in (c - 70).max(0)..(c + 70).min(n) {
            let (i, j) = (x as usize, y as usize);
            let sz = map.surface_z[j * map.width + i].max(0) as usize;
            if sz + 1 < map.depth && map.cell(i, j, sz + 1).water > 0 { far_water += 1; }
        }
    }
    let added = |w: &str| map.furnished.iter().any(|f| f.contains(w));
    if added("spring") { water = water.saturating_sub(9); }
    if added("grove") { trees = trees.saturating_sub(24); }
    if added("outcrop") { stone = stone.saturating_sub(20); }
    if added("thicket") { shrubs = shrubs.saturating_sub(18); }
    // Each against what a camp wants within reach.
    let scores = [(water as f32 / 300.0, "water"), (trees as f32 / 600.0, "timber"), (stone as f32 / 150.0, "stone"), (shrubs as f32 / 300.0, "berries")];
    let best = scores.iter().copied().max_by(|a, b| a.0.total_cmp(&b.0)).unwrap();
    let worst = scores.iter().copied().min_by(|a, b| a.0.total_cmp(&b.0)).unwrap();
    let gift = match best.1 {
        "water" => if water > 300 { "a river or lake at hand".to_string() } else { format!("water ({} cells)", water) },
        "timber" => format!("woods ({} trees)", trees),
        "stone" => format!("stone ({} cells of rock)", stone),
        _ => format!("berries ({} bushes)", shrubs),
    };
    let lack = match worst.1 {
        "water" if water < 10 && far_water > 10 => "water close by (a stream an hour's walk off)".to_string(),
        "water" if water < 10 => "water (only a spring)".to_string(),
        "timber" if trees < 20 => "timber (they will build in stone)".to_string(),
        "stone" if stone < 30 => "stone (little rock to quarry)".to_string(),
        "berries" if shrubs < 15 => "berries (food is short)".to_string(),
        k => format!("{} (less of it than elsewhere)", k),
    };
    (gift, lack)
}
