//! A town of the history, realized for the adventurer: a square with a well, the temple of its
//! people's god, a smithy, a trader's shop, an inn, the lord's hall and a guardhouse, houses,
//! its wall and gate; under the square a grate into the sewers, where the rats are (where a new
//! adventurer starts small, as Tibia's do).

use super::actor::{Npc, Role};
use super::item::Item;
use super::map::{Feature, Floor, Ground, Tile, Wall, DIRS4};
use super::site::{Builder, Place, SiteKind, SiteSpec};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// A personal name in the tongue of `race`.
pub fn person_name(race: &str, seed: u64) -> String {
    use crate::history::entities::races::RaceType;
    use crate::history::naming::{generator::NameGenerator, styles::NamingStyle};
    let arche = RaceType::from_tag(race).default_naming_archetype();
    let style = NamingStyle::from_archetype(crate::history::NamingStyleId(0), arche);
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    NameGenerator::personal_name(&style, &mut rng).split_whitespace().next().unwrap_or("Ash").to_string()
}

fn house(f: &mut Floor, x: i32, y: i32, w: i32, h: i32, wall: Wall, ground: Ground, door_south: bool) -> (i32, i32) {
    for yy in y..y + h { for xx in x..x + w {
        let edge = xx == x || yy == y || xx == x + w - 1 || yy == y + h - 1;
        f.set(xx, yy, if edge { Tile::wall(wall, ground) } else { Tile::floor(ground) });
    } }
    let d = if door_south { (x + w / 2, y + h - 1) } else { (x + w / 2, y) };
    f.set(d.0, d.1, Tile { ground, wall: Wall::None, feature: Feature::Door { open: false, lock: 0 } });
    d
}

pub fn realize(spec: &SiteSpec) -> Place {
    let mut b = Builder { rng: ChaCha8Rng::seed_from_u64(spec.seed ^ 0x70_3A) };
    let (w, h) = (72usize, 56usize);
    let mut f = Floor::new(w, h, Tile::floor(spec.surface), "the streets", true);
    // Ring of trees beyond the wall, the wall, the gate to the south.
    let wall = if spec.tier == 0 { Wall::Palisade } else { Wall::Brick };
    for y in 0..h as i32 { for x in 0..w as i32 {
        let edge = x <= 1 || y <= 1 || x >= w as i32 - 2 || y >= h as i32 - 2;
        if edge { f.set(x, y, Tile::wall(Wall::Tree, spec.surface)); continue; }
        let ring = x == 4 || y == 4 || x == w as i32 - 5 || y == h as i32 - 5;
        if ring && x >= 4 && y >= 4 && x <= w as i32 - 5 && y <= h as i32 - 5 { f.set(x, y, Tile::wall(wall, Ground::Earth)); }
        else if (x < 4 || y < 4 || x > w as i32 - 5 || y > h as i32 - 5) && b.chance(0.12) { f.set(x, y, Tile::wall(Wall::Tree, spec.surface)); }
    } }
    let (gx, gy) = (w as i32 / 2, h as i32 - 5);
    for x in gx - 1..=gx + 1 { f.set(x, gy, Tile::floor(Ground::Cobbles)); }
    for y in gy..h as i32 - 1 { for x in gx - 1..=gx + 1 { f.set(x, y, Tile::floor(Ground::Cobbles)); } }
    f.at_mut(gx, h as i32 - 3).feature = Feature::Exit;
    // Streets: a cross through the square.
    let (sx, sy) = (w as i32 / 2, h as i32 / 2);
    for x in 5..w as i32 - 5 { for dy in -1..=1 { f.set(x, sy + dy, Tile::floor(Ground::Cobbles)); } }
    for y in 5..h as i32 - 5 { for dx in -1..=1 { f.set(sx + dx, y, Tile::floor(Ground::Cobbles)); } }
    for y in sy - 5..=sy + 5 { for x in sx - 7..=sx + 7 { f.set(x, y, Tile::floor(Ground::Cobbles)); } }
    f.at_mut(sx, sy).feature = Feature::Well;
    // The grate into the sewers by the well.
    f.at_mut(sx + 3, sy + 2).feature = Feature::Grate;
    f.at_mut(sx - 3, sy - 3).feature = Feature::Sign { text: format!("{}. The temple is to the north-west, the inn north-east; the smith and the trader keep shop south of the square. Mind the grate.", spec.name) };
    // Buildings: (role, x, y, w, h).
    let stone = Wall::Brick;
    let timber = Wall::Timber;
    let lots: [(Option<Role>, i32, i32, i32, i32, Wall, Ground); 10] = [
        (Some(Role::Priest), 8, 7, 18, 12, stone, Ground::Marble),
        (Some(Role::Innkeeper), 46, 7, 16, 11, timber, Ground::Wood),
        (Some(Role::Smith), 8, 34, 12, 9, stone, Ground::Flags),
        (Some(Role::Trader), 22, 34, 11, 9, timber, Ground::Wood),
        (Some(Role::Lord), 28, 7, 16, 10, stone, Ground::Carpet),
        (Some(Role::Guard), 52, 34, 10, 8, stone, Ground::Flags),
        (Some(Role::Sage), 40, 34, 9, 8, timber, Ground::Wood),
        (None, 8, 21, 8, 7, timber, Ground::Wood),
        (None, 52, 21, 9, 7, timber, Ground::Wood),
        (None, 43, 44, 8, 6, timber, Ground::Wood),
    ];
    let race = spec.people.clone();
    let mut npcs = Vec::new();
    for (k, &(role, x, y, bw, bh, wl, g)) in lots.iter().enumerate() {
        let south = y < sy;
        let door = house(&mut f, x, y, bw, bh, wl, g, south);
        // A path from the door to the nearest street.
        let step = if south { 1 } else { -1 };
        let (mut px, mut py) = (door.0, door.1 + step);
        for _ in 0..12 { if f.at(px, py).ground == Ground::Cobbles || !f.inside(px, py) { break; } f.set(px, py, Tile::floor(Ground::Cobbles)); py += step; }
        let _ = &mut px;
        let (ix, iy) = (x + bw / 2, if south { y + 2 } else { y + bh - 3 });
        match role {
            Some(Role::Priest) => { f.at_mut(ix, y + 2).feature = Feature::Altar; for xx in [x + 3, x + bw - 4] { f.at_mut(xx, y + 2).feature = Feature::Brazier; } for yy in (y + 4..y + bh - 2).step_by(2) { f.at_mut(ix - 3, yy).feature = Feature::Table; f.at_mut(ix + 3, yy).feature = Feature::Table; } }
            Some(Role::Innkeeper) => { for xx in (x + 2..x + bw - 2).step_by(3) { f.at_mut(xx, y + 5).feature = Feature::Table; } for xx in x + 2..x + 6 { f.at_mut(xx, y + 2).feature = Feature::Counter; } f.at_mut(x + bw - 2, y + 1).feature = Feature::Barrel; f.at_mut(x + bw - 3, y + 1).feature = Feature::Barrel; for xx in (x + 9..x + bw - 1).step_by(2) { f.at_mut(xx, y + bh - 2).feature = Feature::Bed; } }
            Some(Role::Smith) => { f.at_mut(x + 2, y + 2).feature = Feature::Anvil; f.at_mut(x + 4, y + 1).feature = Feature::Brazier; for xx in x + 6..x + 10 { f.at_mut(xx, iy + 2).feature = Feature::Counter; } }
            Some(Role::Trader) => { for xx in x + 2..x + 8 { f.at_mut(xx, iy + 2).feature = Feature::Counter; } f.at_mut(x + 1, y + 1).feature = Feature::Crate; f.at_mut(x + 2, y + 1).feature = Feature::Barrel; f.at_mut(x + bw - 2, y + 1).feature = Feature::Crate; }
            Some(Role::Lord) => { f.at_mut(ix, y + 2).feature = Feature::Throne; for xx in [x + 2, x + bw - 3] { f.at_mut(xx, y + 2).feature = Feature::Sconce; } for xx in (x + 3..x + bw - 3).step_by(3) { f.at_mut(xx, y + 5).feature = Feature::Table; } }
            Some(Role::Guard) => { f.at_mut(x + 1, y + 1).feature = Feature::Barrel; f.at_mut(x + bw - 2, y + 1).feature = Feature::Table; f.at_mut(x + 2, y + bh - 3).feature = Feature::Bed; }
            Some(Role::Sage) => { for xx in x + 1..x + bw - 1 { f.at_mut(xx, y + 1).feature = Feature::Bookshelf; } f.at_mut(ix, iy + 1).feature = Feature::Table; }
            _ => { f.at_mut(x + 1, y + 1).feature = Feature::Bed; f.at_mut(x + bw - 2, y + bh - 2).feature = Feature::Barrel; f.at_mut(x + bw / 2, y + bh / 2).feature = Feature::Table; }
        }
        let role = role.unwrap_or(Role::Townsfolk);
        // Where they stand: behind the counter, before the altar, by the throne.
        let post = match role {
            Role::Priest => (ix, y + 3), Role::Lord => (ix, y + 3), Role::Innkeeper => (x + 3, y + 1), Role::Smith | Role::Trader => (x + 4, iy + 1),
            Role::Guard => (x + 4, y + 3), Role::Sage => (ix, iy + 2), _ => (x + 2, y + bh / 2),
        };
        let post = if f.walkable(post.0, post.1) { post } else { (ix, iy) };
        let female = b.rng.gen_bool(0.5);
        let name = match (role, &spec.lord) { (Role::Lord, Some((n, _))) => n.clone(), _ => person_name(&race, spec.seed ^ (k as u64 * 0x9E37 + 11)) };
        let of = match (role, &spec.lord) { (Role::Priest, _) => spec.god.clone(), (Role::Lord, Some((_, title))) => title.clone(), _ => spec.people.clone() };
        npcs.push(Npc { name, role, x: post.0, y: post.1, z: 0, post, race: race.clone(), female, of });
    }
    // Townsfolk in the street.
    for k in 0..4 {
        let (x, y) = (sx - 6 + k * 4, sy + 4);
        npcs.push(Npc { name: person_name(&race, spec.seed ^ (0xF0 + k as u64)), role: Role::Townsfolk, x, y, z: 0, post: (x, y), race: race.clone(), female: k % 2 == 0, of: spec.people.clone() });
    }
    // The sewers: galleries on a grid with a channel down their middle, rats and worse.
    let mut floors = vec![f];
    let mut monsters = Vec::new();
    let mut uid = 1u32;
    let mut arrive = (sx + 3, sy + 2);
    let depth = 2usize;
    for z in 1..=depth {
        let (sw, sh) = (w, h);
        let mut s = Floor::new(sw, sh, Tile::wall(Wall::Brick, Ground::Flags), &format!("the {} sewer", if z == 1 { "upper" } else { "lower" }), false);
        for y in (6..sh as i32 - 6).step_by(6) { for x in 4..sw as i32 - 4 { for dy in -1..=1 { s.set(x, y + dy, Tile::floor(if dy == 0 { Ground::Shallows } else { Ground::Flags })); } } }
        for x in (6..sw as i32 - 6).step_by(8) { for y in 4..sh as i32 - 4 { for dx in -1..=1 { s.set(x + dx, y, Tile::floor(if dx == 0 { Ground::Shallows } else { Ground::Flags })); } } }
        // Side rooms off the galleries: cisterns and old store rooms.
        for _ in 0..8 {
            let (rx, ry) = (b.rng.gen_range(5..sw as i32 - 12), b.rng.gen_range(5..sh as i32 - 10));
            for y in ry..ry + 4 { for x in rx..rx + 6 { if s.at(x, y).wall != Wall::None { s.set(x, y, Tile::floor(Ground::Flags)); } } }
            if b.rng.gen_bool(0.5) { s.at_mut(rx + 5, ry).feature = Feature::Chest { items: super::site::treasure(&mut b, 1), opened: false, lock: 0, quest: 0 }; }
            if b.rng.gen_bool(0.5) { s.at_mut(rx, ry + 3).feature = Feature::Barrel; }
        }
        // Where the ladder comes down: open around it, joined to the galleries.
        for y in arrive.1 - 1..=arrive.1 + 1 { for x in arrive.0 - 1..=arrive.0 + 1 { s.set(x, y, Tile::floor(Ground::Flags)); } }
        let (mut cx, mut cy) = arrive;
        while s.at(cx, cy + 1).ground != Ground::Shallows && cy < sh as i32 - 6 { cy += 1; s.set(cx, cy, Tile::floor(Ground::Flags)); }
        let _ = &mut cx;
        s.at_mut(arrive.0, arrive.1).feature = Feature::LadderUp;
        // Bones and webs.
        for (x, y) in s.cells(|t| t.walkable() && t.feature == Feature::None && t.ground == Ground::Flags) {
            let p: f64 = b.rng.gen();
            if p < 0.01 { s.at_mut(x, y).feature = Feature::Bones; } else if p < 0.016 && DIRS4.iter().filter(|(dx, dy)| s.at(x + dx, y + dy).wall != Wall::None).count() >= 2 { s.at_mut(x, y).feature = Feature::Web; }
        }
        // The way further down, at the far end.
        let next = if z < depth {
            let d = s.distances(arrive.0, arrive.1, 10_000, |x, y| s.at(x, y).walkable());
            let mut best = (arrive, 0);
            for (x, y) in s.cells(|t| t.walkable() && t.feature == Feature::None && t.ground == Ground::Flags) { let k = d[y as usize * sw + x as usize]; if k != i32::MAX && k > best.1 { best = ((x, y), k); } }
            s.at_mut(best.0 .0, best.0 .1).feature = Feature::LadderDown;
            Some(best.0)
        } else { None };
        // Rats (and in the lower sewer, what eats them).
        // The upper sewer is for the newly come: rats and the odd bat. Worse below.
        let table: &[&str] = if z == 1 { &["rat", "rat", "rat", "rat", "rat", "bat"] } else { &["rat", "cave_rat", "cave_rat", "spider", "snake", "poison_spider", "goblin"] };
        let cells = s.cells(|t| t.walkable() && t.feature == Feature::None);
        for _ in 0..(cells.len() / if z == 1 { 45 } else { 32 }) {
            let (x, y) = cells[b.rng.gen_range(0..cells.len())];
            if (x - arrive.0).abs() + (y - arrive.1).abs() < 6 { continue; }
            let id = table[b.rng.gen_range(0..table.len())];
            monsters.push(super::actor::Monster::new(uid, id, x, y, z));
            uid += 1;
        }
        if z == depth {
            // The rat king of the lowest sewer, and what it hoards.
            let d = s.distances(arrive.0, arrive.1, 10_000, |x, y| s.at(x, y).walkable());
            let mut best = (arrive, 0);
            for (x, y) in s.cells(|t| t.walkable() && t.feature == Feature::None) { let k = d[y as usize * sw + x as usize]; if k != i32::MAX && k > best.1 { best = ((x, y), k); } }
            let mut m = super::actor::Monster::boss(uid, "cave_rat", &format!("the Rat King of {}", spec.name), 2.2, best.0 .0, best.0 .1, z);
            m.carries = vec![Item::new("gold", 60), Item::of("short_sword", "bronze", 2)];
            monsters.push(m);
            uid += 1;
        }
        floors.push(s);
        if let Some(n) = next { arrive = n; }
    }
    let entry = (gx, h as i32 - 3);
    Place { spec: spec.clone(), floors, monsters, npcs, entry, next_uid: uid }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_town_has_its_people_and_its_sewers() {
        let spec = SiteSpec { id: 1, kind: SiteKind::Town, name: "Greenburg".into(), tile: (0, 0), seed: 42, tier: 1, cause: String::new(), boss: None, treasures: vec![],
            surface: Ground::Grass, rock: "granite".into(), floors: 3, people: "human".into(), god: "Balorn".into(), news: Vec::new(), lord: None };
        let p = super::super::site::realize(&spec);
        assert_eq!(p.floors.len(), 3);
        for role in [Role::Priest, Role::Smith, Role::Trader, Role::Innkeeper, Role::Lord] { assert!(p.npcs.iter().any(|n| n.role == role), "{:?}", role); }
        let f = &p.floors[0];
        let d = f.distances(p.entry.0, p.entry.1, 10_000, |x, y| { let t = f.at(x, y); t.wall == Wall::None });
        for n in &p.npcs { assert!(d[n.y as usize * f.w + n.x as usize] < i32::MAX, "{} the {} can be reached", n.name, n.role.word()); }
        let g = f.find(|t| matches!(t, Feature::Grate)).unwrap();
        assert!(d[g.1 as usize * f.w + g.0 as usize] < i32::MAX);
        assert!(p.monsters.iter().filter(|m| m.z == 1).count() >= 10);
        assert!(p.monsters.iter().any(|m| m.boss));
    }
}

