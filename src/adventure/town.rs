//! A town of the history, laid out in its tile of the land (`surface`): its size, walls and way
//! of building come from the history (`TownShape`: a hamlet round a green, a village along its
//! road, a walled town on a grid or in crooked lanes, a fortified city with towers), its gates
//! open where its roads leave, a port has its piers on the sea side. Round the square: the well,
//! the sign, a market; the temple of its people's god, the lord's hall, the inn, the smithy, the
//! trader's shop, the guardhouse by the main gate, the sage's house, homes. Under the square a
//! grate into two sewer floors (rats, then worse; the Rat King), where a new adventurer starts
//! small, as Tibia's do.

use super::actor::{Npc, Role};
use super::item::Item;
use super::map::{Feature, Floor, Ground, Tile, Wall, DIRS4, DIRS8};
use super::site::{Builder, Place, SiteSpec, TownShape};
use super::surface::{gate_side, town_extent, CH};
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

/// How a people builds.
struct Style { wall: Wall, floor: Ground, street: Ground, open: Ground, rich: Wall, rich_floor: Ground, grid: bool, green: bool }

fn style(arch: &str, size: u8) -> Style {
    match arch {
        "stone" | "metalwork" | "crystal" => Style { wall: Wall::Brick, floor: Ground::Wood, street: Ground::Cobbles, open: Ground::Earth, rich: Wall::Brick, rich_floor: Ground::Marble, grid: true, green: false },
        "carved" => Style { wall: Wall::Rock, floor: Ground::Flags, street: Ground::Flags, open: Ground::Rock, rich: Wall::Rock, rich_floor: Ground::Marble, grid: true, green: false },
        "earthen" | "bone" => Style { wall: Wall::Brick, floor: Ground::Earth, street: Ground::Earth, open: Ground::Earth, rich: Wall::Brick, rich_floor: Ground::Flags, grid: false, green: false },
        "living" => Style { wall: Wall::Hedge, floor: Ground::Moss, street: Ground::Grass, open: Ground::Grass, rich: Wall::Hedge, rich_floor: Ground::Moss, grid: false, green: true },
        _ => Style { wall: Wall::Timber, floor: Ground::Wood, street: if size >= 2 { Ground::Cobbles } else { Ground::Earth }, open: Ground::Grass, rich: if size >= 2 { Wall::Brick } else { Wall::Timber }, rich_floor: Ground::Flags, grid: size >= 3, green: false },
    }
}

fn shape(spec: &SiteSpec) -> TownShape {
    spec.town.clone().unwrap_or(TownShape { size: 1, walls: 1, arch: "wood".into(), population: 300, port: false, roads: 0, sea: 0, razed: None })
}

/// The sides with gates (0 north .. 3 west): where its roads leave, and the south always.
fn gate_sides(t: &TownShape) -> [bool; 4] {
    let mut s = [false; 4];
    for (k, d) in DIRS8.iter().enumerate() { if t.roads & (1 << k) != 0 { s[gate_side(*d)] = true; } }
    s[2] = true;
    s
}

/// The side the sea lies on (0 north .. 3 west), for a port's piers.
fn sea_side(t: &TownShape) -> Option<usize> { if t.sea == 0 { None } else { (0..4).find(|k| t.sea & (1 << k) != 0) } }

/// The piers of a port: (x, y, w, h) rectangles in chunk cells.
fn piers(spec: &SiteSpec) -> Vec<(i32, i32, i32, i32)> {
    let t = shape(spec);
    let Some(side) = sea_side(&t) else { return Vec::new() };
    let (hw, hh) = town_extent(t.size);
    let c = CH / 2;
    let n = 2 + t.size.min(2) as i32;
    let mut v = Vec::new();
    for k in 0..n {
        let off = (k - (n - 1) / 2) * 9 - 2;
        let len = 14;
        v.push(match side {
            0 => (c + off, c - hh - len, 2, len),
            1 => (c + hw + 1, c + off, len, 2),
            2 => (c + off, c + hh + 1, 2, len),
            _ => (c - hw - len, c + off, len, 2),
        });
    }
    v
}

pub fn is_pier(spec: &SiteSpec, x: i32, y: i32) -> bool { piers(spec).iter().any(|&(px, py, pw, ph)| x >= px && y >= py && x < px + pw && y < py + ph) }

/// The cells of its tile a town lays over the land.
pub fn footprint(spec: &SiteSpec) -> Vec<bool> {
    let t = shape(spec);
    let (hw, hh) = town_extent(t.size);
    let c = CH / 2;
    let mut m = vec![false; (CH * CH) as usize];
    for y in (c - hh - 3).max(0)..=(c + hh + 3).min(CH - 1) { for x in (c - hw - 3).max(0)..=(c + hw + 3).min(CH - 1) { m[(y * CH + x) as usize] = true; } }
    for (px, py, pw, ph) in piers(spec) { for y in py..py + ph { for x in px..px + pw { if x >= 0 && y >= 0 && x < CH && y < CH { m[(y * CH + x) as usize] = true; } } } }
    m
}

// What a cell of the town plan is.
const OPEN: u8 = 0;
const STREET: u8 = 1;
const BUILT: u8 = 2;
const WALL: u8 = 3;
const PLAZA: u8 = 4;

struct Plan { f: Floor, used: Vec<u8>, st: Style }

impl Plan {
    fn u(&self, x: i32, y: i32) -> u8 { if x < 0 || y < 0 || x >= CH || y >= CH { WALL } else { self.used[(y * CH + x) as usize] } }
    fn mark(&mut self, x: i32, y: i32, v: u8) { if x >= 0 && y >= 0 && x < CH && y < CH { self.used[(y * CH + x) as usize] = v; } }
    fn street(&mut self, x: i32, y: i32) {
        if !matches!(self.u(x, y), OPEN | STREET) { return; }
        let g = self.st.street;
        self.f.set(x, y, Tile::floor(g));
        self.mark(x, y, STREET);
    }
    fn put(&mut self, x: i32, y: i32, feat: Feature) { if self.f.inside(x, y) && self.f.at(x, y).wall == Wall::None { self.f.at_mut(x, y).feature = feat; } }
}

/// A building: its walls round (x, y, w, h), a door on `side` (0 north .. 3 west).
fn building(p: &mut Plan, x: i32, y: i32, w: i32, h: i32, wall: Wall, floor: Ground, side: usize) -> (i32, i32) {
    for yy in y..y + h { for xx in x..x + w {
        let edge = xx == x || yy == y || xx == x + w - 1 || yy == y + h - 1;
        p.f.set(xx, yy, if edge { Tile::wall(wall, floor) } else { Tile::floor(floor) });
        p.mark(xx, yy, BUILT);
    } }
    let d = match side { 0 => (x + w / 2, y), 1 => (x + w - 1, y + h / 2), 2 => (x + w / 2, y + h - 1), _ => (x, y + h / 2) };
    p.f.set(d.0, d.1, Tile { ground: floor, wall: Wall::None, feature: Feature::Door { open: false, lock: 0 } });
    d
}

/// Walk from the cell outside a door to the nearest street over open ground, paving it.
fn path_to_street(p: &mut Plan, from: (i32, i32)) {
    if matches!(p.u(from.0, from.1), STREET | PLAZA) { return; }
    let n = (CH * CH) as usize;
    let mut prev = vec![usize::MAX; n];
    let k0 = (from.1 * CH + from.0) as usize;
    if from.0 < 0 || from.1 < 0 || from.0 >= CH || from.1 >= CH || p.u(from.0, from.1) != OPEN { return; }
    prev[k0] = k0;
    let mut q = std::collections::VecDeque::from([from]);
    let mut end = None;
    while let Some((x, y)) = q.pop_front() {
        if matches!(p.u(x, y), STREET | PLAZA) { end = Some((x, y)); break; }
        for (dx, dy) in DIRS4 {
            let (nx, ny) = (x + dx, y + dy);
            if nx < 0 || ny < 0 || nx >= CH || ny >= CH { continue; }
            let k = (ny * CH + nx) as usize;
            if prev[k] != usize::MAX || !matches!(p.u(nx, ny), OPEN | STREET | PLAZA) { continue; }
            prev[k] = (y * CH + x) as usize;
            q.push_back((nx, ny));
        }
    }
    let Some((ex, ey)) = end else { return };
    let mut k = (ey * CH + ex) as usize;
    while k != k0 { k = prev[k]; let (x, y) = ((k as i32) % CH, (k as i32) / CH); p.street(x, y); }
}

/// The town's first floor (its streets, in its tile's cells) and its people.
fn lay_out(spec: &SiteSpec, b: &mut Builder) -> (Floor, Vec<Npc>, (i32, i32), (i32, i32)) {
    let t = shape(spec);
    let size = t.size.min(3);
    let st = style(&t.arch, size);
    let (hw, hh) = town_extent(size);
    let c = CH / 2;
    let (x0, y0, x1, y1) = (c - hw, c - hh, c + hw, c + hh);
    let mut p = Plan { f: Floor::new(CH as usize, CH as usize, Tile::floor(st.open), "the streets", true), used: vec![OPEN; (CH * CH) as usize], st };
    // Outside the walls: the yard of the land (left open; the land's own trees are cleared).
    for y in 0..CH { for x in 0..CH { if x < x0 || x > x1 || y < y0 || y > y1 { p.mark(x, y, WALL); } } }
    // The walls.
    let gates = gate_sides(&t);
    let wall_kind = match t.walls { 0 => None, 1 => Some(Wall::Palisade), _ => Some(if p.st.wall == Wall::Rock { Wall::Rock } else { Wall::Brick }) };
    let thick = if t.walls >= 3 { 2 } else { 1 };
    if let Some(wk) = wall_kind {
        for y in y0..=y1 { for x in x0..=x1 {
            let ring = x < x0 + thick || y < y0 + thick || x > x1 - thick || y > y1 - thick;
            if ring { p.f.set(x, y, Tile::wall(wk, Ground::Earth)); p.mark(x, y, WALL); }
        } }
        // Towers on the corners and along a fortified wall.
        if t.walls >= 3 {
            let mut towers = vec![(x0, y0), (x1 - 2, y0), (x0, y1 - 2), (x1 - 2, y1 - 2)];
            for x in (x0 + 12..x1 - 6).step_by(12) { towers.push((x, y0)); towers.push((x, y1 - 2)); }
            for y in (y0 + 12..y1 - 6).step_by(12) { towers.push((x0, y)); towers.push((x1 - 2, y)); }
            for (tx, ty) in towers { for y in ty - 1..ty + 3 { for x in tx - 1..tx + 3 { if x >= 0 && y >= 0 && x < CH && y < CH { p.f.set(x, y, Tile::wall(wk, Ground::Earth)); p.mark(x, y, WALL); } } } }
        }
    } else {
        // No wall: a few fences and the land's edge.
        for y in y0..=y1 { for x in x0..=x1 { if (x == x0 || x == x1 || y == y0 || y == y1) && b.chance(0.3) { p.f.set(x, y, Tile::wall(Wall::Hedge, p.st.open)); p.mark(x, y, WALL); } } }
    }
    // Gates: an opening three wide at each gated side's middle.
    let mut gate_cells = Vec::new();
    for side in 0..4 {
        if !gates[side] { continue; }
        let (gx, gy) = match side { 0 => (c, y0), 1 => (x1, c), 2 => (c, y1), _ => (x0, c) };
        gate_cells.push((side, gx, gy));
        for k in -1..=1 { for d in 0..thick + 1 {
            let (x, y) = match side { 0 => (gx + k, gy + d), 1 => (gx - d, gy + k), 2 => (gx + k, gy - d), _ => (gx + d, gy + k) };
            p.mark(x, y, OPEN);
            p.street(x, y);
        } }
    }
    // The square.
    let (pw, ph) = (3 + size as i32, 2 + size as i32);
    for y in c - ph..=c + ph { for x in c - pw..=c + pw { p.mark(x, y, OPEN); p.street(x, y); p.mark(x, y, PLAZA); } }
    // Main streets from the square to each gate.
    for &(side, gx, gy) in &gate_cells {
        let (dx, dy) = DIRS4[side];
        let (mut x, mut y) = (c, c);
        while (x, y) != (gx, gy) && x >= x0 && x <= x1 && y >= y0 && y <= y1 {
            for k in -1..=1 { if dx == 0 { p.street(x + k, y); } else { p.street(x, y + k); } }
            x += dx; y += dy;
        }
    }
    // The well, the sign, the grate; a market on a big square.
    let grate = (c + 3, c + 2);
    p.put(c, c, if size >= 3 { Feature::Fountain } else { Feature::Well });
    p.put(grate.0, grate.1, Feature::Grate);
    // The sign's place (its words come when the houses stand).
    p.put(c - 3, c - 2, Feature::Sign { text: String::new() });
    if size >= 2 {
        for k in 0..(2 + size as i32) { let x = c - pw + 1 + k * 2; if x < c + pw { p.put(x, c - ph + 1, Feature::Counter); } }
        p.put(c + pw - 1, c - ph + 1, Feature::Crate);
        p.put(c + pw - 1, c + ph - 1, Feature::Barrel);
    }
    // Buildings, the important first, each where it belongs.
    let s = size as i32;
    let main_gate = gate_cells.iter().find(|g| g.0 == 2).map(|g| (g.1, g.2)).unwrap_or((c, y1));
    let mut lots: Vec<(Option<Role>, i32, i32, (i32, i32))> = vec![
        (Some(Role::Priest), 9 + 2 * s, 7 + s, (c, c - ph - 6 - s)),
        (Some(Role::Lord), 8 + 3 * s, 7 + 2 * s, (c, y0 + 6 + s * 2)),
        (Some(Role::Innkeeper), 9 + s, 7, (c + pw + 7, c - 3)),
        (Some(Role::Smith), 7, 6, (c - pw - 6, c + ph + 4)),
        (Some(Role::Trader), 7, 6, (c + pw + 5, c + ph + 4)),
        (Some(Role::Guard), 7, 6, (main_gate.0 - 8, main_gate.1 - 5)),
        (Some(Role::Sage), 6, 6, (c - pw - 9, c - 4)),
    ];
    for _ in 0..[5, 9, 16, 26][size as usize] { let (w, h) = (b.range(6, 8), b.range(5, 7)); lots.push((None, w, h, (b.range(x0 + 4, x1 - 4), b.range(y0 + 4, y1 - 4)))); }
    let mut npcs = Vec::new();
    let race = spec.people.clone();
    let mut post_of: Vec<(Role, (i32, i32), (i32, i32, i32, i32))> = Vec::new();
    for (k, &(role, w0, h0, want)) in lots.iter().enumerate() {
        // Once the great houses stand, the lesser streets are laid round them.
        if k == 7 {
        // More streets: a grid, or crooked lanes.
        if p.st.grid || size >= 3 {
            let (sx, sy) = (14 + 2 * (size as i32 - 2).max(0) + b.range(0, 2), 12 + b.range(0, 2));
            for k in 1..6 { for xx in [c - k * sx, c + k * sx] { if xx > x0 + 2 && xx < x1 - 2 { for y in y0 + thick..=y1 - thick { p.street(xx, y); p.street(xx + 1, y); } } } }
            for k in 1..6 { for yy in [c - k * sy, c + k * sy] { if yy > y0 + 2 && yy < y1 - 2 { for x in x0 + thick..=x1 - thick { p.street(x, yy); p.street(x, yy + 1); } } } }
        } else if size >= 1 {
            for _ in 0..(2 + size as i32 * 2) {
                let (mut x, mut y) = (c + b.range(-pw, pw), c + b.range(-ph, ph));
                let mut d = DIRS4[b.range(0, 3) as usize];
                for _ in 0..b.range(14, 34) {
                    if x <= x0 + thick || y <= y0 + thick || x >= x1 - thick || y >= y1 - thick { break; }
                    p.street(x, y);
                    p.street(x + d.1.abs(), y + d.0.abs());
                    if b.chance(0.12) { d = DIRS4[b.range(0, 3) as usize]; }
                    x += d.0; y += d.1;
                }
            }
        }
        }
        // The free spot nearest where it wants to be (its rect on open ground, its rim open or
        // street), smaller if it must; a trade without a house keeps a stall on the square.
        let mut found: Option<(i32, i32, i32, i32)> = None;
        for (w, h) in [(w0, h0), (w0 - 1, h0 - 1), (w0 - 2, h0 - 1), (5, 5)] {
            let mut best: Option<((i32, i32), i32)> = None;
            let step = 1;
            for y in (y0 + thick + 1..=y1 - thick - h).step_by(step) { for x in (x0 + thick + 1..=x1 - thick - w).step_by(step) {
                let d = (x + w / 2 - want.0).abs() + (y + h / 2 - want.1).abs();
                if best.map_or(false, |b| d >= b.1) { continue; }
                let clear = (y - 1..=y + h).all(|yy| (x - 1..=x + w).all(|xx| { let u = p.u(xx, yy); let rim = xx < x || yy < y || xx >= x + w || yy >= y + h; u == OPEN || (rim && u == STREET) }));
                if clear { best = Some(((x, y), d)); }
            } }
            if let Some(((x, y), _)) = best { found = Some((x, y, w, h)); break; }
        }
        let Some((x, y, w, h)) = found else {
            if let Some(role) = role {
                let spot = (0..CH * CH).map(|k| (k % CH, k / CH)).filter(|&(x, y)| p.u(x, y) == PLAZA && p.f.walkable(x, y) && p.f.at(x, y).feature == Feature::None && !npcs.iter().any(|n: &Npc| (n.x, n.y) == (x, y))).min_by_key(|&(x, y)| ((x - c).abs() + (y - c).abs(), x, y));
                if let Some((sx, sy)) = spot {
                    let name = person_name(&race, spec.seed ^ (k as u64 * 0x9E37 + 11));
                    post_of.push((role, (sx, sy), (sx, sy, 1, 1)));
                    npcs.push(Npc { name, role, x: sx, y: sy, z: 0, post: (sx, sy), race: race.clone(), female: b.rng.gen_bool(0.5), of: if role == Role::Priest { spec.god.clone() } else { spec.people.clone() }, home: spec.id, met: Default::default() });
                }
            }
            continue;
        };
        // The door on the side nearest a street.
        let sides = [(x + w / 2, y - 1), (x + w, y + h / 2), (x + w / 2, y + h), (x - 1, y + h / 2)];
        let side = (0..4).min_by_key(|&sd| {
            let (sx, sy) = sides[sd];
            let (dx, dy) = DIRS4[sd];
            (0..14).find(|&r| matches!(p.u(sx + dx * r, sy + dy * r), STREET | PLAZA)).unwrap_or(99) * 4 + sd as i32
        }).unwrap();
        let (wall, floor) = match role { Some(Role::Priest) => (p.st.rich, p.st.rich_floor), Some(Role::Lord) => (p.st.rich, Ground::Carpet), Some(Role::Smith) | Some(Role::Guard) => (if p.st.wall == Wall::Hedge { Wall::Timber } else { p.st.rich }, Ground::Flags), _ => (p.st.wall, p.st.floor) };
        let door = building(&mut p, x, y, w, h, wall, floor, side);
        let out = (door.0 + DIRS4[side].0, door.1 + DIRS4[side].1);
        path_to_street(&mut p, out);
        // What is inside.
        let (ix, iy) = (x + w / 2, y + h / 2);
        let back = match side { 0 => (ix, y + h - 2), 1 => (x + 1, iy), 2 => (ix, y + 1), _ => (x + w - 2, iy) };
        match role {
            Some(Role::Priest) => {
                p.put(back.0, back.1, Feature::Altar);
                for (bx, by) in [(x + 2, y + 1), (x + w - 3, y + 1), (x + 2, y + h - 2), (x + w - 3, y + h - 2)] { if (bx - back.0).abs() + (by - back.1).abs() > 2 && (bx - door.0).abs() + (by - door.1).abs() > 2 { p.put(bx, by, Feature::Brazier); } }
                for yy in (y + 2..y + h - 2).step_by(2) { for xx in [ix - 3, ix + 3] { if (yy - back.1).abs() > 1 && (xx - door.0).abs() + (yy - door.1).abs() > 2 { p.put(xx, yy, Feature::Table); } } }
            }
            Some(Role::Lord) => {
                p.put(back.0, back.1, Feature::Throne);
                p.put(x + 1, y + 1, Feature::Sconce); p.put(x + w - 2, y + 1, Feature::Sconce);
                for xx in (x + 3..x + w - 3).step_by(3) { if (iy - back.1).abs() > 1 { p.put(xx, iy, Feature::Table); } }
            }
            Some(Role::Innkeeper) => {
                for xx in x + 2..x + 5 { p.put(xx, back.1, Feature::Counter); }
                for xx in (x + 2..x + w - 2).step_by(3) { if (iy - door.1).abs() > 1 { p.put(xx, iy, Feature::Table); } }
                p.put(x + w - 2, y + 1, Feature::Barrel);
                for xx in (x + w / 2 + 1..x + w - 1).step_by(2) { let yy = if side == 2 { y + 1 } else { y + h - 2 }; p.put(xx, yy, Feature::Bed); }
            }
            Some(Role::Smith) => { p.put(x + 1, y + 1, Feature::Anvil); p.put(x + 3, y + 1, Feature::Brazier); p.put(x + w - 2, y + h - 2, Feature::Barrel); }
            Some(Role::Trader) => { p.put(x + 1, y + 1, Feature::Crate); p.put(x + w - 2, y + 1, Feature::Barrel); p.put(x + w - 2, y + h - 2, Feature::Crate); }
            Some(Role::Guard) => { p.put(x + 1, y + 1, Feature::Barrel); p.put(x + w - 2, y + 1, Feature::Table); p.put(x + 1, y + h - 2, Feature::Bed); }
            Some(Role::Sage) => { for xx in x + 1..x + w - 1 { if (xx, y + 1) != (door.0, door.1 + 1) && side != 0 { p.put(xx, y + 1, Feature::Bookshelf); } } p.put(ix, iy + 1, Feature::Table); }
            _ => { p.put(x + 1, y + 1, Feature::Bed); p.put(x + w - 2, y + h - 2, Feature::Barrel); if b.chance(0.5) { p.put(x + w - 2, y + 1, Feature::Table); } }
        }
        let role = role.unwrap_or(Role::Townsfolk);
        // Where they stand: before the altar, by the throne, behind the counter; else inside.
        let want = match role { Role::Priest | Role::Lord => (back.0 + (door.0 - back.0).signum(), back.1 + (door.1 - back.1).signum()), _ => (ix, iy) };
        let post = (0..3).flat_map(|r| (-r..=r).flat_map(move |dy| (-r..=r).map(move |dx| (want.0 + dx, want.1 + dy))))
            .find(|&(px, py)| px > x && py > y && px < x + w - 1 && py < y + h - 1 && p.f.walkable(px, py) && p.f.at(px, py).feature == Feature::None && (px - door.0).abs() + (py - door.1).abs() > 1)
            .unwrap_or((ix, iy));
        if !p.f.walkable(post.0, post.1) { p.f.at_mut(post.0, post.1).feature = Feature::None; }
        post_of.push((role, post, (x, y, w, h)));
        let female = b.rng.gen_bool(0.5);
        let name = match (role, &spec.lord) { (Role::Lord, Some((n, _))) => n.clone(), _ => person_name(&race, spec.seed ^ (k as u64 * 0x9E37 + 11)) };
        let of = match (role, &spec.lord) { (Role::Priest, _) => spec.god.clone(), (Role::Lord, Some((_, title))) => title.clone(), _ => spec.people.clone() };
        npcs.push(Npc { name, role, x: post.0, y: post.1, z: 0, post, race: race.clone(), female, of, home: spec.id, met: Default::default() });
    }
    // The inn's drunk, who has heard everything and gets half of it wrong.
    if let Some(&(_, _, (ix, iy, iw, ih))) = post_of.iter().find(|q| q.0 == Role::Innkeeper) {
        let spot = (iy + 1..iy + ih - 1).flat_map(|y| (ix + 1..ix + iw - 1).map(move |x| (x, y))).find(|&(x, y)| p.f.walkable(x, y) && p.f.at(x, y).feature == Feature::None && !npcs.iter().any(|n: &Npc| (n.x, n.y) == (x, y)) && (x, y) != (ix + iw / 2, iy + ih / 2));
        if let Some((x, y)) = spot { npcs.push(Npc { name: person_name(&race, spec.seed ^ 0xD2C), role: Role::Townsfolk, x, y, z: 0, post: (x, y), race: race.clone(), female: b.rng.gen_bool(0.3), of: "drunk".into(), home: spec.id, met: Default::default() }); }
    }
    // Gardens and trees in the open (a living town grows inside its hedges).
    for y in y0 + thick..=y1 - thick { for x in x0 + thick..=x1 - thick {
        if p.u(x, y) != OPEN { continue; }
        let r: f64 = b.rng.gen();
        if r < if p.st.green { 0.12 } else { 0.025 } { p.f.set(x, y, Tile::wall(Wall::Tree, p.st.open)); }
        else if r < 0.06 && !matches!(p.st.open, Ground::Rock) { p.f.at_mut(x, y).ground = Ground::Field; }
    } }
    // Piers on the sea side, with a gate through the wall to each.
    for (px, py, pw, ph) in piers(spec) {
        for y in py..py + ph { for x in px..px + pw { if p.f.inside(x, y) { p.f.set(x, y, Tile::floor(Ground::Wood)); } } }
        let side = sea_side(&shape(spec)).unwrap_or(2);
        let (dx, dy) = DIRS4[side];
        // From the pier's root back through the wall to the town.
        let (mut x, mut y) = match side { 0 => (px, py + ph), 1 => (px - 1, py), 2 => (px, py - 1), _ => (px + pw, py) };
        for _ in 0..8 { for k in 0..2 { let (cx, cy) = if dx == 0 { (x + k, y) } else { (x, y + k) }; if p.f.inside(cx, cy) && p.u(cx, cy) != BUILT { p.mark(cx, cy, OPEN); p.street(cx, cy); } } x -= dx; y -= dy; if p.u(x, y) == STREET || p.u(x, y) == PLAZA { break; } }
        let mid = (px + pw / 2, py + ph / 2);
        p.put(mid.0, mid.1, if b.chance(0.5) { Feature::Crate } else { Feature::Barrel });
    }
    // The sign by the square: where things are.
    let dir = |to: (i32, i32)| { let (dx, dy) = (to.0 - c, to.1 - c); let ns = if dy < -4 { "north" } else if dy > 4 { "south" } else { "" }; let ew = if dx < -4 { "west" } else if dx > 4 { "east" } else { "" }; if ns.is_empty() && ew.is_empty() { "by the square".to_string() } else if ns.is_empty() || ew.is_empty() { format!("to the {}{}", ns, ew) } else { format!("to the {}-{}", ns, ew) } };
    let at = |r: Role| post_of.iter().find(|q| q.0 == r).map(|q| dir(q.1));
    let mut words = format!("{}.", spec.name);
    for (r, what) in [(Role::Priest, "The temple"), (Role::Innkeeper, "the inn"), (Role::Smith, "the smith"), (Role::Trader, "the trader")] { if let Some(d) = at(r) { words.push_str(&format!(" {} {}.", what, d)); } }
    words.push_str(" Mind the grate.");
    if p.f.inside(c - 3, c - 2) { p.f.at_mut(c - 3, c - 2).feature = Feature::Sign { text: words }; }
    // Townsfolk in the streets; the watch at the gates of a big town.
    let streets: Vec<(i32, i32)> = (0..CH * CH).filter(|&k| matches!(p.used[k as usize], STREET | PLAZA)).map(|k| (k % CH, k / CH)).filter(|&(x, y)| p.f.walkable(x, y) && p.f.at(x, y).feature == Feature::None).collect();
    for k in 0..(3 + 2 * s) {
        let Some(&(x, y)) = b.pick(&streets).as_ref() else { break };
        if npcs.iter().any(|n| (n.x, n.y) == (x, y)) { continue; }
        npcs.push(Npc { name: person_name(&race, spec.seed ^ (0xF0 + k as u64)), role: Role::Townsfolk, x, y, z: 0, post: (x, y), race: race.clone(), female: k % 2 == 0, of: spec.people.clone(), home: spec.id, met: Default::default() });
    }
    let entry = (main_gate.0, main_gate.1 - thick - 1);
    (p.f, npcs, grate, entry)
}

pub fn realize(spec: &SiteSpec) -> Place {
    let mut b = Builder { rng: ChaCha8Rng::seed_from_u64(spec.seed ^ 0x70_3A) };
    let (f, npcs, grate, entry) = lay_out(spec, &mut b);
    let t = shape(spec);
    let (hw, hh) = town_extent(t.size);
    let (hw, hh) = (hw.max(24), hh.max(18));
    let c = CH / 2;
    let (sx0, sy0, sx1, sy1) = (c - hw, c - hh, c + hw, c + hh);
    // The sewers: galleries on a grid with a channel down their middle, rats and worse.
    let (sw, sh) = (CH as usize, CH as usize);
    let mut floors = vec![f];
    let mut monsters = Vec::new();
    let mut uid = 1u32;
    let mut arrive = grate;
    let depth = 2usize;
    for z in 1..=depth {
        let mut s = Floor::new(sw, sh, Tile::wall(Wall::Brick, Ground::Flags), &format!("the {} sewer", if z == 1 { "upper" } else { "lower" }), false);
        for y in (sy0 + 4..sy1 - 3).step_by(6) { for x in sx0 + 2..sx1 - 2 { for dy in -1..=1 { s.set(x, y + dy, Tile::floor(if dy == 0 { Ground::Shallows } else { Ground::Flags })); } } }
        for x in (sx0 + 4..sx1 - 3).step_by(8) { for y in sy0 + 2..sy1 - 2 { for dx in -1..=1 { s.set(x + dx, y, Tile::floor(if dx == 0 { Ground::Shallows } else { Ground::Flags })); } } }
        // Side rooms off the galleries: cisterns and old store rooms.
        for _ in 0..8 {
            let (rx, ry) = (b.rng.gen_range(sx0 + 3..sx1 - 9), b.rng.gen_range(sy0 + 3..sy1 - 7));
            for y in ry..ry + 4 { for x in rx..rx + 6 { if s.at(x, y).wall != Wall::None { s.set(x, y, Tile::floor(Ground::Flags)); } } }
            if b.rng.gen_bool(0.5) { s.at_mut(rx + 5, ry).feature = Feature::Chest { items: super::site::treasure(&mut b, 1), opened: false, lock: 0, quest: 0 }; }
            if b.rng.gen_bool(0.5) { s.at_mut(rx, ry + 3).feature = Feature::Barrel; }
        }
        // Where the ladder comes down: open around it, joined to the galleries.
        for y in arrive.1 - 1..=arrive.1 + 1 { for x in arrive.0 - 1..=arrive.0 + 1 { s.set(x, y, Tile::floor(Ground::Flags)); } }
        let (cx, mut cy) = arrive;
        while s.at(cx, cy + 1).ground != Ground::Shallows && cy < sy1 - 2 { cy += 1; s.set(cx, cy, Tile::floor(Ground::Flags)); }
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
    Place { spec: spec.clone(), floors, monsters, npcs, entry, next_uid: uid, top: 0, origin: None, mouth: None }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::site::SiteKind;

    fn spec(size: u8, walls: u8, arch: &str, roads: u8, sea: u8, seed: u64) -> SiteSpec {
        SiteSpec { id: 1, kind: SiteKind::Town, name: "Greenburg".into(), tile: (0, 0), seed, tier: 1, cause: String::new(), boss: None, treasures: vec![],
            surface: Ground::Grass, rock: "granite".into(), floors: 3, people: "human".into(), god: "Balorn".into(), news: Vec::new(), lord: None,
            town: Some(TownShape { size, walls, arch: arch.into(), population: 1000, port: sea != 0, roads, sea, razed: None }), settlement: None, creature: None, notes: Vec::new() }
    }

    /// Every kind of town has its people, reachable from its gate, a grate to its sewers, and
    /// rats and a king below.
    #[test]
    fn a_town_has_its_people_and_its_sewers() {
        for (k, (size, walls, arch, roads, sea)) in [(0u8, 0u8, "wood", 0u8, 0u8), (1, 1, "earthen", 0b0100_0100, 0), (2, 2, "stone", 0b0101_0101, 0), (3, 3, "stone", 0xFF, 0b0010), (2, 2, "living", 1, 0), (2, 3, "carved", 0b0100_0001, 0)].into_iter().enumerate() {
            let s = spec(size, walls, arch, roads, sea, 42 + k as u64);
            let p = super::super::site::realize(&s);
            assert_eq!(p.floors.len(), 3);
            for role in [Role::Priest, Role::Smith, Role::Trader, Role::Innkeeper, Role::Lord, Role::Guard, Role::Sage] { assert!(p.npcs.iter().any(|n| n.role == role), "{} has its {:?}", arch, role); }
            let f = &p.floors[0];
            let d = f.distances(p.entry.0, p.entry.1, 10_000, |x, y| { let t = f.at(x, y); t.wall == Wall::None });
            for n in &p.npcs { assert!(d[n.y as usize * f.w + n.x as usize] < i32::MAX, "{}: {} the {} can be reached", arch, n.name, n.role.word()); }
            let g = f.find(|t| matches!(t, Feature::Grate)).unwrap();
            assert!(d[g.1 as usize * f.w + g.0 as usize] < i32::MAX);
            assert!(p.monsters.iter().filter(|m| m.z == 1).count() >= 8, "{}", p.monsters.iter().filter(|m| m.z == 1).count());
            // Homes, not only trades: people live here.
            let homes = p.npcs.iter().filter(|n| n.role == Role::Townsfolk).count();
            assert!(homes >= 3 + 2 * size as usize + 2, "{} {}: only {} townsfolk", size, arch, homes);
            assert!(p.monsters.iter().any(|m| m.boss));
        }
    }

    /// Towns of different shapes look different (not the same plan twice).
    #[test]
    fn towns_differ() {
        let a = super::super::site::realize(&spec(1, 1, "wood", 4, 0, 1));
        let b = super::super::site::realize(&spec(1, 1, "wood", 4, 0, 2));
        let c = super::super::site::realize(&spec(3, 3, "stone", 0x55, 0, 1));
        // (Within the walls: the land about them is the same open ground.)
        let same = |p: &Place, q: &Place| { let (hw, hh) = super::super::surface::town_extent(1); let c = CH / 2; let (mut n, mut k) = (0, 0); for y in c - hh..=c + hh { for x in c - hw..=c + hw { n += 1; if p.floors[0].at(x, y) == q.floors[0].at(x, y) { k += 1; } } } k as f32 / n as f32 };
        assert!(same(&a, &b) < 0.85, "two villages are nearly the same: {}", same(&a, &b));
        assert!(same(&a, &c) < 0.8, "a village and a city are nearly the same: {}", same(&a, &c));
    }
}

#[cfg(test)]
mod look {
    use super::*;
    /// `cargo test --release --lib town_plans -- --nocapture --ignored` prints a town of each shape.
    #[test]
    #[ignore]
    fn town_plans() {
        for (size, walls, arch, roads, sea) in [(0u8, 0u8, "wood", 0u8, 0u8), (1, 1, "earthen", 0b0100_0100, 0), (2, 2, "stone", 0b0101_0101, 0), (3, 3, "stone", 0xFF, 0b0010), (2, 2, "living", 1, 0)] {
            let s = SiteSpec { id: 1, kind: super::super::site::SiteKind::Town, name: "T".into(), tile: (0, 0), seed: 9, tier: 1, cause: String::new(), boss: None, treasures: vec![], surface: Ground::Grass, rock: "granite".into(), floors: 3, people: "human".into(), god: "G".into(), news: Vec::new(), lord: None, town: Some(TownShape { size, walls, arch: arch.into(), population: 1, port: sea != 0, roads, sea, razed: None }), settlement: None, creature: None, notes: Vec::new() };
            let p = realize(&s);
            let f = &p.floors[0];
            println!("--- size {} walls {} {}: {} people ({} townsfolk)", size, walls, arch, p.npcs.len(), p.npcs.iter().filter(|n| n.role == Role::Townsfolk).count());
            for y in 0..f.h as i32 { let mut l = String::new(); for x in 0..f.w as i32 {
                let t = f.at(x, y);
                let ch = if p.npcs.iter().any(|n| (n.x, n.y) == (x, y)) { '@' } else { match (&t.wall, &t.feature) { (Wall::None, Feature::None) => match t.ground { Ground::Cobbles | Ground::Flags => ':', Ground::Earth => '.', Ground::Wood => '=', Ground::Field => '"', Ground::Marble => '_', Ground::Carpet => '~', _ => ' ' }, (Wall::None, Feature::Door { .. }) => '+', (Wall::None, _) => '*', (Wall::Tree, _) => 't', (Wall::Palisade, _) => '|', (Wall::Hedge, _) => 'h', _ => '#' } };
                l.push(ch);
            } println!("{}", l); }
        }
    }
}
