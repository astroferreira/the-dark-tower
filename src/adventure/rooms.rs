//! Authored rooms in the procedural floors (`data/defaults/adventure_rooms.json`): a flooded
//! crypt, an ossuary, a reliquary behind a hidden door, a throne of bones, a lever vault, a riddle
//! chapel, a corridor of pressure plates... Each is cut into solid rock where it fits and joined to
//! the floor by a passage from its one way in (a door, a hidden door, a riddle door or a gap).
//!
//! They tell the place's history: an engraving in a tomb names the dead of its battle, a
//! temple's hymn names its god, a ruin's or a keep's tells who held it and how it fell, a lair's
//! remains are what its beast ate. And they hold the puzzles (one kind for each kind of place):
//! levers to pull in the order an engraving gives (labyrinths, halls, keeps), a door that asks a
//! riddle (temples, shrines, cellars), plates that loose darts unless one treads where the dead are
//! marked (tombs, ruins, the Shadow's fortress).

use super::item::Item;
use super::map::{Feature, Floor, Ground, Tile, Wall};
use super::site::{Builder, SiteKind, SiteSpec};
use serde::Deserialize;
use std::sync::OnceLock;

const JSON: &str = include_str!("../../data/defaults/adventure_rooms.json");

#[derive(Clone, Debug, Deserialize)]
pub struct RoomDef { pub id: String, pub kinds: Vec<String>, pub floor: String, pub text: String, pub rows: Vec<String> }

#[derive(Clone, Debug, Deserialize)]
pub struct Riddle { pub q: String, pub answers: Vec<String>, pub right: usize }

#[derive(Debug, Deserialize)]
pub struct Rooms { pub rooms: Vec<RoomDef>, pub riddles: Vec<Riddle> }

pub fn data() -> &'static Rooms {
    static D: OnceLock<Rooms> = OnceLock::new();
    D.get_or_init(|| serde_json::from_str(JSON).expect("adventure_rooms.json"))
}

/// A room as stamped: where, and what one is told on first coming into it.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Stamped { pub z: usize, pub rect: (i32, i32, i32, i32), pub text: String, pub seen: bool }

/// Levers to pull in an order (a gate opens when it is done).
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct LeverPuzzle { pub z: usize, pub order: Vec<u32>, pub pulled: Vec<u32>, pub gate: u32 }

/// Ways in on a room's border.
const ENTRIES: &str = "Dd+Q";

/// The engraving a place carves: its history, by kind.
fn engraving(spec: &SiteSpec) -> String {
    let cause = spec.cause.trim();
    match spec.kind {
        SiteKind::Tomb => if cause.is_empty() { "Names, worn almost away, and a date.".into() } else { format!("Carved in the stone: {}", cause) },
        SiteKind::Temple => format!("A hymn to {} cut in the stone: \"Light in the dark, and the dark to its place.\" {}", if spec.god.is_empty() { "the old gods" } else { &spec.god }, cause),
        SiteKind::Castle | SiteKind::Ruin => if cause.is_empty() { "The arms of the house that held this place, chiselled away by someone who hated it.".into() } else { format!("Cut over the door, half defaced: {}", cause) },
        SiteKind::Halls | SiteKind::Mine => "Dwarven runes: the names of the kings under the mountain, and a curse on thieves.".into(),
        SiteKind::Shrine | SiteKind::Cellar | SiteKind::DarkFortress => format!("Scratched over and over, in many hands: {}.", if spec.god.is_empty() { "a name you will not say aloud" } else { &spec.god }),
        SiteKind::Labyrinth => "A map scratched into the wall by someone lost here. Half of it is wrong.".into(),
        _ => if cause.is_empty() { "Marks in the stone, old as the stone.".into() } else { cause.to_string() },
    }
}

/// Remains that tell something.
fn remains(spec: &SiteSpec, b: &mut Builder) -> String {
    if matches!(spec.kind, SiteKind::Lair | SiteKind::Cave) {
        if let Some(boss) = &spec.boss { return format!("Gnawed bones, cracked for the marrow: what {} ate. {}", boss.name, spec.cause); }
    }
    let notes = [
        "In a dead hand, a scrap of paper: \"the third torch is false; we went left and it was wrong\".",
        "A shield with a crest you do not know, split in two, and the arm still in its straps.",
        "A purse, empty, and a letter to someone's mother that was never sent.",
        "Three arrows in the ribs; whatever shot them was waiting.",
        "Chalk on the floor beside them: \"DON'T TRUST THE\". Then nothing.",
        "Bones in good boots. The boots were worth more than the man, in the end.",
    ];
    notes[b.range(0, notes.len() as i32 - 1) as usize].to_string()
}

/// Cut rooms into floor `z` of `spec` (of `n` floors): returns them, the lever puzzle if any, and
/// the cells where the room's own monsters stand.
pub fn stamp(b: &mut Builder, f: &mut Floor, spec: &SiteSpec, z: usize, n: usize, lock_id: &mut u32, wall: Wall, ground: Ground) -> (Vec<Stamped>, Option<LeverPuzzle>, Vec<(i32, i32)>) {
    let mut out = Vec::new();
    let mut puzzle = None;
    let mut spots = Vec::new();
    if f.outdoor { return (out, puzzle, spots); }
    let kind = format!("{:?}", spec.kind);
    let fits = |r: &RoomDef| r.kinds.iter().any(|k| *k == kind) && match r.floor.as_str() { "first" => z == 0, "last" => z + 1 == n, "deep" => z >= 1, _ => true };
    let cands: Vec<&RoomDef> = data().rooms.iter().filter(|r| fits(r)).collect();
    if cands.is_empty() { return (out, puzzle, spots); }
    let count = if b.chance(0.75) { 1 } else { 0 } + if z + 1 == n && b.chance(0.5) { 1 } else { 0 };
    for _ in 0..count {
        let r = cands[b.range(0, cands.len() as i32 - 1) as usize];
        let (rh, rw) = (r.rows.len() as i32, r.rows.iter().map(|l| l.chars().count()).max().unwrap_or(0) as i32);
        // Where it fits: all solid rock round it, and a passage to the floor from its way in.
        let mut places: Vec<(i32, i32)> = Vec::new();
        for y in 2..f.h as i32 - rh - 2 { for x in 2..f.w as i32 - rw - 2 {
            if (y - 1..=y + rh).all(|yy| (x - 1..=x + rw).all(|xx| f.at(xx, yy).wall != Wall::None && f.at(xx, yy).feature == Feature::None)) { places.push((x, y)); }
        } }
        if places.is_empty() { continue; }
        let (ox, oy) = places[b.range(0, places.len() as i32 - 1) as usize];
        let grid: Vec<Vec<char>> = r.rows.iter().map(|l| l.chars().collect()).collect();
        // The way in, on the border, and the way out of it.
        let mut entry = None;
        for (y, row) in grid.iter().enumerate() { for (x, c) in row.iter().enumerate() {
            let border = y == 0 || x == 0 || y + 1 == grid.len() || x + 1 == row.len();
            if border && ENTRIES.contains(*c) { entry = Some((x as i32, y as i32)); }
        } }
        let Some((ex, ey)) = entry else { continue };
        let out_dir = if ey == 0 { (0, -1) } else if ey == rh - 1 { (0, 1) } else if ex == 0 { (-1, 0) } else { (1, 0) };
        let saved: Vec<Tile> = (oy - 1..=oy + rh).flat_map(|y| (ox - 1..=ox + rw).map(move |x| (x, y))).map(|(x, y)| f.at(x, y).clone()).collect();
        let tier = spec.tier;
        let mut levers: Vec<(i32, i32, u32)> = Vec::new();
        let gate_id = { *lock_id += 1; *lock_id };
        let mut has_plates = false;
        let mut engravings: Vec<(i32, i32)> = Vec::new();
        for (y, row) in grid.iter().enumerate() { for (x, c) in row.iter().enumerate() {
            let (cx, cy) = (ox + x as i32, oy + y as i32);
            let fl = |g: Ground, feat: Feature| Tile { ground: g, wall: Wall::None, feature: feat };
            let t = match c {
                '#' => Tile::wall(wall, ground),
                '~' => Tile::floor(Ground::Shallows),
                'w' => Tile::floor(Ground::Water),
                'r' => Tile::floor(Ground::Rubble),
                'S' => fl(ground, Feature::Sarcophagus { items: super::site::treasure(b, tier), opened: false }),
                'C' => fl(ground, Feature::Chest { items: super::site::treasure(b, tier + 1), opened: false, lock: 0, quest: 0 }),
                '*' => fl(ground, Feature::Plinth { item: Some(super::site::gear(b, tier + 1)) }),
                'A' => fl(ground, Feature::Altar), 'B' => fl(ground, Feature::Brazier), 'P' => fl(ground, Feature::Pillar), 'x' => fl(ground, Feature::Statue),
                'b' => fl(ground, Feature::Bones), 'W' => fl(ground, Feature::Web), 'T' => fl(ground, Feature::Trap { armed: true, damage: 8 + tier as i32 * 6 }),
                'D' => fl(ground, Feature::Door { open: false, lock: 0 }),
                'd' => Tile { ground, wall, feature: Feature::SecretDoor },
                'E' => { engravings.push((cx, cy)); fl(ground, Feature::Lore { text: engraving(spec), look: 0 }) }
                'R' => fl(ground, Feature::Lore { text: remains(spec, b), look: 1 }),
                'p' => { has_plates = true; fl(ground, Feature::Plate { safe: false }) }
                'o' => { has_plates = true; fl(ground, Feature::Plate { safe: true }) }
                'L' => { *lock_id += 1; levers.push((cx, cy, *lock_id)); fl(ground, Feature::Lever { id: *lock_id, pulled: false }) }
                'G' => fl(ground, Feature::Gate { open: false, lever: gate_id }),
                'Q' => fl(ground, Feature::RiddleDoor { riddle: b.range(0, data().riddles.len() as i32 - 1) as u32, open: false }),
                'M' => { spots.push((cx, cy)); Tile::floor(ground) }
                'h' => fl(ground, Feature::Throne), 'k' => fl(ground, Feature::Bookshelf), 't' => fl(ground, Feature::Table), 'l' => fl(ground, Feature::Sconce),
                'f' => fl(ground, Feature::Fountain), 'c' => fl(ground, Feature::Crate), 'a' => fl(ground, Feature::Barrel),
                _ => Tile::floor(ground),
            };
            f.set(cx, cy, t);
        } }
        // The passage from outside its way in to the nearest open ground (through rock only).
        let start = (ox + ex + out_dir.0, oy + ey + out_dir.1);
        let in_room = |x: i32, y: i32| x >= ox && y >= oy && x < ox + rw && y < oy + rh;
        let (w, h) = (f.w, f.h);
        let mut prev = vec![usize::MAX; w * h];
        let mut q = std::collections::VecDeque::new();
        let idx = |x: i32, y: i32| y as usize * w + x as usize;
        prev[idx(start.0, start.1)] = idx(start.0, start.1);
        q.push_back(start);
        let mut end = None;
        while let Some((x, y)) = q.pop_front() {
            if f.at(x, y).walkable() && !in_room(x, y) { end = Some((x, y)); break; }
            for (dx, dy) in super::map::DIRS4 {
                let (nx, ny) = (x + dx, y + dy);
                if nx < 1 || ny < 1 || nx >= w as i32 - 1 || ny >= h as i32 - 1 || in_room(nx, ny) || prev[idx(nx, ny)] != usize::MAX { continue; }
                let t = f.at(nx, ny);
                if t.wall == Wall::None && !t.walkable() { continue; }
                prev[idx(nx, ny)] = idx(x, y);
                q.push_back((nx, ny));
            }
        }
        let Some(mut at) = end else {
            // No way to join it: the rock as it was.
            let mut k = 0;
            for y in oy - 1..=oy + rh { for x in ox - 1..=ox + rw { f.set(x, y, saved[k].clone()); k += 1; } }
            spots.retain(|&(x, y)| !in_room(x, y));
            continue;
        };
        while at != start {
            let p = prev[idx(at.0, at.1)];
            at = ((p % w) as i32, (p / w) as i32);
            if f.at(at.0, at.1).wall != Wall::None { let g = f.at(at.0, at.1).ground; f.set(at.0, at.1, Tile::floor(if matches!(g, Ground::Void) { ground } else { g })); }
        }
        // The puzzle's hint, cut in the room's engraving.
        if levers.len() >= 2 {
            let mut order: Vec<u32> = levers.iter().map(|l| l.2).collect();
            for i in (1..order.len()).rev() { let j = b.range(0, i as i32) as usize; order.swap(i, j); }
            let mut by_x = levers.clone();
            by_x.sort_by_key(|l| (l.0, l.1));
            let word = |id: u32| { let k = by_x.iter().position(|l| l.2 == id).unwrap_or(0); match (k, by_x.len()) { (0, _) => "western", (k, n) if k + 1 == n => "eastern", _ => "middle" } };
            let hint = format!("Cut over the levers: \"First the {} lever, then the {}, and the {} last; and the way opens to the patient.\"", word(order[0]), word(order[1]), word(*order.last().unwrap()));
            for &(x, y) in &engravings { f.at_mut(x, y).feature = Feature::Lore { text: hint.clone(), look: 0 }; }
            puzzle = Some(LeverPuzzle { z, order, pulled: Vec::new(), gate: gate_id });
        }
        if has_plates {
            for &(x, y) in &engravings { f.at_mut(x, y).feature = Feature::Lore { text: "Cut over the hall: \"Tread only where the dead are marked, and you will not join them.\"".into(), look: 0 }; }
        }
        out.push(Stamped { z, rect: (ox, oy, rw, rh), text: r.text.clone(), seen: false });
        let _ = Item::new("gold", 1);
    }
    (out, puzzle, spots)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The library: twenty rooms or more, each with one way in on its border and only known marks.
    #[test]
    fn rooms_are_whole() {
        let d = data();
        assert!(d.rooms.len() >= 20, "{} rooms", d.rooms.len());
        assert!(d.riddles.len() >= 6);
        for r in &d.rooms {
            let grid: Vec<Vec<char>> = r.rows.iter().map(|l| l.chars().collect()).collect();
            let mut ways = 0;
            for (y, row) in grid.iter().enumerate() { for (x, c) in row.iter().enumerate() {
                assert!("#.~wrSC*ABPxbWTDdERpoLGQMhktlfca+".contains(*c), "{}: unknown mark {}", r.id, c);
                let border = y == 0 || x == 0 || y + 1 == grid.len() || x + 1 == row.len();
                if border && ENTRIES.contains(*c) { ways += 1; }
                if !border { assert!(!"Dd+Q".contains(*c) || *c == 'D', "{}: a way in inside", r.id); }
            } }
            assert_eq!(ways, 1, "{} has {} ways in", r.id, ways);
        }
    }
}

#[cfg(test)]
mod made {
    use super::super::site::{realize, tests::spec, SiteKind};
    use super::super::map::Feature;

    /// Rooms are cut into places: over many places of each kind, some, with hidden doors among
    /// them, and a tomb's engraving names its dead.
    #[test]
    fn places_get_their_rooms() {
        let (mut rooms, mut secrets, mut named) = (0, 0, false);
        for kind in [SiteKind::Tomb, SiteKind::Temple, SiteKind::Castle, SiteKind::Halls, SiteKind::Labyrinth, SiteKind::Cave, SiteKind::Shrine] {
            for seed in 0..12u64 {
                let mut s = spec(kind, 3, 2, seed * 31 + kind as u64);
                s.cause = "The dead of the Battle of Ripu Field (247) were laid here: Aster the Bold, Vell Greyhand.".into();
                let p = realize(&s);
                rooms += p.rooms.len();
                secrets += p.floors.iter().flat_map(|f| f.tiles.iter()).filter(|t| t.feature == Feature::SecretDoor).count();
                if kind == SiteKind::Tomb { named |= p.floors.iter().flat_map(|f| f.tiles.iter()).any(|t| matches!(&t.feature, Feature::Lore { text, look: 0 } if text.contains("Aster the Bold"))); }
            }
        }
        assert!(rooms >= 40, "{} rooms", rooms);
        assert!(secrets >= 3, "{} hidden doors", secrets);
        assert!(named, "no tomb names its dead");
    }
}
