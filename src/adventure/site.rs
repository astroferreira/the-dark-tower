//! Places the adventurer can enter, realized on demand from their seed and their cause (DF's
//! `sitemap_create_realization`: a site's layout is a pure function of its seed, so the same
//! ruin is always the same ruin). A `SiteSpec` says what a place is and why it is there (a beast
//! of the history laired here, the dead of a battle buried here, a town razed in 412); `realize`
//! builds its floors, its monsters, its treasure and its boss.

use super::actor::{Monster, Npc};
use super::data::data;
use super::item::Item;
use super::map::{Feature, Floor, Ground, Tile, Wall, DIRS4, DIRS8};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SiteKind { Town, Cave, Lair, Mine, Ruin, Tomb, Temple, Shrine, Castle, Labyrinth, Camp, Halls, DarkFortress, Wilds }

impl SiteKind {
    pub fn word(self) -> &'static str {
        match self {
            SiteKind::Town => "a town", SiteKind::Cave => "a cave", SiteKind::Lair => "a lair", SiteKind::Mine => "an old mine", SiteKind::Ruin => "a ruin",
            SiteKind::Tomb => "a tomb", SiteKind::Temple => "a temple", SiteKind::Shrine => "a cult's shrine", SiteKind::Castle => "a castle",
            SiteKind::Labyrinth => "a labyrinth", SiteKind::Camp => "a war camp", SiteKind::Halls => "halls under the mountain",
            SiteKind::DarkFortress => "the dark fortress", SiteKind::Wilds => "the wilds",
        }
    }
    /// The habitats its floors draw monsters from, by depth (the first floor, then below).
    fn habitats(self) -> (&'static [&'static str], &'static [&'static str]) {
        match self {
            SiteKind::Town => (&[], &["sewer"]),
            SiteKind::Cave => (&["cave"], &["cave"]),
            SiteKind::Lair => (&["cave", "lair"], &["lair", "cave"]),
            SiteKind::Mine => (&["mine"], &["mine", "cave"]),
            SiteKind::Ruin => (&["ruin"], &["ruin", "cave"]),
            SiteKind::Tomb => (&["tomb"], &["tomb"]),
            SiteKind::Temple => (&["temple_crypt"], &["temple_crypt", "tomb"]),
            SiteKind::Shrine => (&["shrine"], &["shrine", "temple_crypt"]),
            SiteKind::Castle => (&["castle"], &["castle", "tomb"]),
            SiteKind::Labyrinth => (&["labyrinth"], &["labyrinth"]),
            SiteKind::Camp => (&["camp"], &["camp", "cave"]),
            SiteKind::Halls => (&["halls"], &["halls", "cave"]),
            SiteKind::DarkFortress => (&["dark_fortress"], &["dark_fortress"]),
            SiteKind::Wilds => (&["wilds"], &["wilds"]),
        }
    }
}

/// A named enemy at the bottom of a place: a beast of the history (with its generated monster),
/// a risen captain, a cult's high priest, the Shadow's lord.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct BossSpec {
    pub def: String,
    pub name: String,
    pub scale: f32,
    pub legend: Option<crate::monsters::Monster>,
    pub hoard: Vec<Item>,
    /// What it is, for the look panel and the quest.
    pub story: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SiteSpec {
    pub id: u32,
    pub kind: SiteKind,
    pub name: String,
    /// The world tile.
    pub tile: (usize, usize),
    pub seed: u64,
    /// How dangerous (1 rats and goblins .. 6 the Shadow's own).
    pub tier: u32,
    /// Why it is here, from the history ("Bornith laired here; it killed 14").
    pub cause: String,
    pub boss: Option<BossSpec>,
    /// Treasures of the history that lie here (a lost artifact).
    pub treasures: Vec<Item>,
    /// The land's ground and rock (for the surface and the walls).
    pub surface: Ground,
    pub rock: String,
    pub floors: usize,
    /// A town's people (race), its god, its lord's people.
    pub people: String,
    pub god: String,
    /// What a town has heard of the world (its people's accounts), for rumours.
    pub news: Vec<String>,
}

/// A realized place.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Place {
    pub spec: SiteSpec,
    pub floors: Vec<Floor>,
    pub monsters: Vec<Monster>,
    pub npcs: Vec<Npc>,
    /// Where one arrives (floor 0).
    pub entry: (i32, i32),
    pub next_uid: u32,
}

/// The rooms a layout made (for furnishing): x, y, w, h.
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct Room { pub x: i32, pub y: i32, pub w: i32, pub h: i32 }

impl Room {
    pub fn centre(&self) -> (i32, i32) { (self.x + self.w / 2, self.y + self.h / 2) }
    fn cells(&self) -> impl Iterator<Item = (i32, i32)> + '_ { (self.y..self.y + self.h).flat_map(move |y| (self.x..self.x + self.w).map(move |x| (x, y))) }
}

pub struct Builder { pub rng: ChaCha8Rng }

impl Builder {
    pub fn chance(&mut self, p: f64) -> bool { self.rng.gen_bool(p.clamp(0.0, 1.0)) }
    pub fn range(&mut self, a: i32, b: i32) -> i32 { if b <= a { a } else { self.rng.gen_range(a..=b) } }
    pub fn pick<T: Clone>(&mut self, v: &[T]) -> Option<T> { if v.is_empty() { None } else { Some(v[self.rng.gen_range(0..v.len())].clone()) } }
}

/// Wall and ground for a kind's underground floors.
fn under_style(kind: SiteKind) -> (Wall, Ground, Ground) {
    match kind {
        SiteKind::Cave | SiteKind::Lair | SiteKind::Wilds => (Wall::Rock, Ground::Rock, Ground::Rock),
        SiteKind::Mine => (Wall::Rock, Ground::Earth, Ground::Earth),
        SiteKind::Tomb | SiteKind::Temple | SiteKind::Shrine => (Wall::Brick, Ground::Flags, Ground::Flags),
        SiteKind::Castle | SiteKind::DarkFortress => (Wall::Brick, Ground::Flags, Ground::Flags),
        SiteKind::Halls => (Wall::Rock, Ground::Marble, Ground::Flags),
        SiteKind::Labyrinth => (Wall::Brick, Ground::Flags, Ground::Flags),
        SiteKind::Ruin | SiteKind::Camp => (Wall::Rock, Ground::Earth, Ground::Flags),
        SiteKind::Town => (Wall::Brick, Ground::Flags, Ground::Flags),
    }
}

/// Floor names by depth.
fn floor_name(kind: SiteKind, z: usize, n: usize) -> String {
    let deep = ["first", "second", "third", "fourth", "fifth", "sixth", "seventh"];
    let d = deep[z.min(6)];
    let last = z + 1 == n && n > 1;
    match kind {
        SiteKind::Town => if z == 0 { "the streets".into() } else { format!("the {} sewer", d) },
        SiteKind::Cave => if last { "the deepest hollow".into() } else { format!("the {} cave", d) },
        SiteKind::Lair => if last { "the lair itself".into() } else { format!("the {} tunnel", d) },
        SiteKind::Mine => format!("the {} gallery", d),
        SiteKind::Ruin => if z == 0 { "the broken walls".into() } else { format!("the {} cellar", d) },
        SiteKind::Tomb => if z == 0 { "the mausoleum".into() } else if last { "the burial chamber".into() } else { format!("the {} catacomb", d) },
        SiteKind::Temple | SiteKind::Shrine => if z == 0 { "the nave".into() } else if last { "the inner sanctum".into() } else { format!("the {} crypt", d) },
        SiteKind::Castle => if z == 0 { "the bailey".into() } else if last { "the deepest dungeon".into() } else { format!("the {} dungeon", d) },
        SiteKind::Labyrinth => if last { "the heart of the maze".into() } else { format!("the {} maze", d) },
        SiteKind::Camp => if z == 0 { "the camp".into() } else { format!("the {} warren", d) },
        SiteKind::Halls => if last { "the throne hall".into() } else { format!("the {} hall", d) },
        SiteKind::DarkFortress => if z == 0 { "the outer ward".into() } else if last { "the seat of the Shadow".into() } else { format!("the {} undercroft", d) },
        SiteKind::Wilds => "the wilds".into(),
    }
}

// ---------------------------------------------------------------------------------------------
// Layouts

/// Cellular caves (DF's caverns, roguelike style): random rock, smoothed, the largest open
/// region kept (and joined to `arrive`).
fn caves(b: &mut Builder, w: usize, h: usize, wall: Wall, ground: Ground, open: f64) -> Floor {
    let mut f = Floor::new(w, h, Tile::wall(wall, ground), "", false);
    let mut cell = vec![false; w * h];
    for y in 1..h - 1 { for x in 1..w - 1 { cell[y * w + x] = b.chance(open); } }
    for _ in 0..5 {
        let prev = cell.clone();
        for y in 1..h - 1 { for x in 1..w - 1 {
            let n = DIRS8.iter().filter(|(dx, dy)| prev[(y as i32 + dy) as usize * w + (x as i32 + dx) as usize]).count();
            cell[y * w + x] = if prev[y * w + x] { n >= 4 } else { n >= 5 };
        } }
    }
    for y in 0..h { for x in 0..w { if cell[y * w + x] { f.set(x as i32, y as i32, Tile::floor(ground)); } } }
    f
}

/// Rooms joined by corridors (L-shaped), doors where corridors meet rooms.
fn rooms(b: &mut Builder, w: usize, h: usize, wall: Wall, ground: Ground, room_ground: Ground, n: usize, doors: bool) -> (Floor, Vec<Room>) {
    let mut f = Floor::new(w, h, Tile::wall(wall, ground), "", false);
    let mut rs: Vec<Room> = Vec::new();
    for _ in 0..n * 8 {
        if rs.len() >= n { break; }
        let (rw, rh) = (b.range(4, 9), b.range(3, 7));
        let (rx, ry) = (b.range(2, w as i32 - rw - 3), b.range(2, h as i32 - rh - 3));
        let r = Room { x: rx, y: ry, w: rw, h: rh };
        if rs.iter().any(|o| r.x - 2 < o.x + o.w && o.x - 2 < r.x + r.w && r.y - 2 < o.y + o.h && o.y - 2 < r.y + r.h) { continue; }
        for (x, y) in r.cells() { f.set(x, y, Tile::floor(room_ground)); }
        rs.push(r);
    }
    // Join each room to the nearest already joined (a tree), then a few loops.
    let mut joined = vec![0usize];
    for k in 1..rs.len() {
        let near = *joined.iter().min_by_key(|&&j| { let (a, c) = (rs[k].centre(), rs[j].centre()); (a.0 - c.0).abs() + (a.1 - c.1).abs() }).unwrap();
        corridor(b, &mut f, rs[k].centre(), rs[near].centre(), ground);
        joined.push(k);
    }
    for _ in 0..rs.len() / 4 {
        let (a, c) = (b.range(0, rs.len() as i32 - 1) as usize, b.range(0, rs.len() as i32 - 1) as usize);
        if a != c { corridor(b, &mut f, rs[a].centre(), rs[c].centre(), ground); }
    }
    if doors {
        // A door where a corridor cell meets a room's edge between two walls.
        for r in &rs {
            for x in r.x - 1..=r.x + r.w { for y in [r.y - 1, r.y + r.h] { door_if_gap(b, &mut f, x, y, true); } }
            for y in r.y - 1..=r.y + r.h { for x in [r.x - 1, r.x + r.w] { door_if_gap(b, &mut f, x, y, false); } }
        }
    }
    (f, rs)
}

fn door_if_gap(b: &mut Builder, f: &mut Floor, x: i32, y: i32, horizontal_wall: bool) {
    if !f.inside(x, y) || f.at(x, y).wall != Wall::None || f.at(x, y).feature != Feature::None { return; }
    let (a, c) = if horizontal_wall { ((x - 1, y), (x + 1, y)) } else { ((x, y - 1), (x, y + 1)) };
    if f.at(a.0, a.1).wall != Wall::None && f.at(c.0, c.1).wall != Wall::None && b.chance(0.7) {
        f.at_mut(x, y).feature = Feature::Door { open: false, lock: 0 };
    }
}

fn corridor(b: &mut Builder, f: &mut Floor, a: (i32, i32), c: (i32, i32), ground: Ground) {
    let horizontal_first = b.chance(0.5);
    let dig = |x: i32, y: i32, f: &mut Floor| { if f.inside(x, y) && x > 0 && y > 0 && x < f.w as i32 - 1 && y < f.h as i32 - 1 && f.at(x, y).wall != Wall::None { f.set(x, y, Tile::floor(ground)); } };
    let (mut x, mut y) = a;
    if horizontal_first {
        while x != c.0 { dig(x, y, f); x += (c.0 - x).signum(); }
        while y != c.1 { dig(x, y, f); y += (c.1 - y).signum(); }
    } else {
        while y != c.1 { dig(x, y, f); y += (c.1 - y).signum(); }
        while x != c.0 { dig(x, y, f); x += (c.0 - x).signum(); }
    }
    dig(c.0, c.1, f);
}

/// Catacombs: galleries on a grid, burial niches in their walls, a few chambers.
fn catacombs(b: &mut Builder, w: usize, h: usize, wall: Wall, ground: Ground) -> (Floor, Vec<Room>) {
    let mut f = Floor::new(w, h, Tile::wall(wall, ground), "", false);
    let step = 5;
    for y in (3..h as i32 - 3).step_by(step) { for x in 3..w as i32 - 3 { if b.chance(0.97) { f.set(x, y, Tile::floor(ground)); } } }
    for x in (3..w as i32 - 3).step_by(step) { for y in 3..h as i32 - 3 { if b.chance(0.97) { f.set(x, y, Tile::floor(ground)); } } }
    // Chambers at some crossings.
    let mut rs = Vec::new();
    for _ in 0..(w * h / 300).max(3) {
        let (cx, cy) = (3 + step as i32 * b.range(0, (w as i32 - 7) / step as i32), 3 + step as i32 * b.range(0, (h as i32 - 7) / step as i32));
        let r = Room { x: cx - 2, y: cy - 2, w: 5, h: 5 };
        if r.x < 1 || r.y < 1 || r.x + r.w >= w as i32 - 1 || r.y + r.h >= h as i32 - 1 { continue; }
        for (x, y) in r.cells() { f.set(x, y, Tile::floor(ground)); }
        rs.push(r);
    }
    // Niches: a cell cut into a gallery's wall, holding a sarcophagus, a coffin's bones.
    let galleries = f.cells(|t| t.wall == Wall::None);
    for (x, y) in galleries {
        for (dx, dy) in DIRS4 {
            let (nx, ny) = (x + dx, y + dy);
            if nx < 1 || ny < 1 || nx >= w as i32 - 1 || ny >= h as i32 - 1 || f.at(nx, ny).wall == Wall::None { continue; }
            let behind = (nx + dx, ny + dy);
            if f.at(behind.0, behind.1).wall == Wall::None { continue; }
            if b.chance(0.09) {
                let feat = if b.chance(0.45) { Feature::Sarcophagus { items: Vec::new(), opened: false } } else { Feature::Bones };
                f.set(nx, ny, Tile { ground, wall: Wall::None, feature: feat });
            }
        }
    }
    (f, rs)
}

/// A maze (recursive backtracker on odd cells), a few walls knocked through, a chamber at its heart.
fn maze(b: &mut Builder, w: usize, h: usize, wall: Wall, ground: Ground) -> (Floor, Vec<Room>) {
    let mut f = Floor::new(w, h, Tile::wall(wall, ground), "", false);
    let (cw, ch) = ((w as i32 - 1) / 2, (h as i32 - 1) / 2);
    let mut seen = vec![false; (cw * ch) as usize];
    let mut stack = vec![(b.range(0, cw - 1), b.range(0, ch - 1))];
    seen[(stack[0].1 * cw + stack[0].0) as usize] = true;
    f.set(stack[0].0 * 2 + 1, stack[0].1 * 2 + 1, Tile::floor(ground));
    while let Some(&(cx, cy)) = stack.last() {
        let mut opts = Vec::new();
        for (dx, dy) in DIRS4 { let (nx, ny) = (cx + dx, cy + dy); if nx >= 0 && ny >= 0 && nx < cw && ny < ch && !seen[(ny * cw + nx) as usize] { opts.push((nx, ny, dx, dy)); } }
        match b.pick(&opts) {
            Some((nx, ny, dx, dy)) => {
                seen[(ny * cw + nx) as usize] = true;
                f.set(cx * 2 + 1 + dx, cy * 2 + 1 + dy, Tile::floor(ground));
                f.set(nx * 2 + 1, ny * 2 + 1, Tile::floor(ground));
                stack.push((nx, ny));
            }
            None => { stack.pop(); }
        }
    }
    for _ in 0..(w * h / 40) { let (x, y) = (b.range(2, w as i32 - 3), b.range(2, h as i32 - 3)); if f.at(x, y).wall != Wall::None { f.set(x, y, Tile::floor(ground)); } }
    let r = Room { x: w as i32 / 2 - 3, y: h as i32 / 2 - 2, w: 7, h: 5 };
    for (x, y) in r.cells() { f.set(x, y, Tile::floor(Ground::Flags)); }
    (f, vec![r])
}

/// Mine galleries: straight tunnels turning now and then, with rails, side chambers.
fn mine(b: &mut Builder, w: usize, h: usize) -> (Floor, Vec<Room>) {
    let mut f = Floor::new(w, h, Tile::wall(Wall::Rock, Ground::Earth), "", false);
    let mut rs = Vec::new();
    let (mut x, mut y) = (w as i32 / 2, h as i32 / 2);
    let mut d = DIRS4[b.range(0, 3) as usize];
    for k in 0..(w * h / 4) {
        if x <= 2 || y <= 2 || x >= w as i32 - 3 || y >= h as i32 - 3 { d = (-d.0, -d.1); x = x.clamp(3, w as i32 - 4); y = y.clamp(3, h as i32 - 4); }
        f.set(x, y, Tile { ground: Ground::Earth, wall: Wall::None, feature: if k % 3 == 0 && b.chance(0.5) { Feature::Rail } else { Feature::None } });
        if b.chance(0.08) { d = DIRS4[b.range(0, 3) as usize]; }
        if b.chance(0.015) {
            let r = Room { x: x - 2, y: y - 2, w: b.range(3, 6), h: b.range(3, 5) };
            if r.x > 1 && r.y > 1 && r.x + r.w < w as i32 - 1 && r.y + r.h < h as i32 - 1 { for (cx, cy) in r.cells() { f.set(cx, cy, Tile::floor(Ground::Earth)); } rs.push(r); }
        }
        x += d.0; y += d.1;
    }
    (f, rs)
}

/// Open ground of the land's kind, with trees and boulders, and (maybe) a building in its middle.
fn surface(b: &mut Builder, w: usize, h: usize, ground: Ground, trees: f64) -> Floor {
    let mut f = Floor::new(w, h, Tile::floor(ground), "", true);
    for y in 0..h as i32 { for x in 0..w as i32 {
        let edge = x == 0 || y == 0 || x == w as i32 - 1 || y == h as i32 - 1;
        if edge { f.set(x, y, Tile::wall(Wall::Tree, ground)); continue; }
        if b.chance(trees) { f.set(x, y, Tile::wall(Wall::Tree, ground)); }
        else if b.chance(0.01) { f.set(x, y, Tile::wall(Wall::Rock, ground)); }
    } }
    f
}

/// A building on open ground: brick or timber walls, rooms inside, a door on the south side.
/// Ruined: walls broken and rubble strewn. Returns its rooms (the whole hall first).
fn building(b: &mut Builder, f: &mut Floor, r: Room, wall: Wall, ground: Ground, ruined: bool, split: bool) -> Vec<Room> {
    for y in r.y..r.y + r.h { for x in r.x..r.x + r.w {
        let edge = x == r.x || y == r.y || x == r.x + r.w - 1 || y == r.y + r.h - 1;
        let t = if edge { Tile::wall(wall, ground) } else { Tile::floor(ground) };
        f.set(x, y, t);
    } }
    let mut out = vec![Room { x: r.x + 1, y: r.y + 1, w: r.w - 2, h: r.h - 2 }];
    if split && r.w >= 12 {
        // Side rooms along the east and west walls behind partitions with doors.
        for side in [r.x + 4, r.x + r.w - 5] {
            for y in r.y + 1..r.y + r.h - 1 { f.set(side, y, Tile::wall(wall, ground)); }
            let dy = r.y + r.h / 2;
            f.set(side, dy, Tile { ground, wall: Wall::None, feature: Feature::Door { open: false, lock: 0 } });
        }
        out.push(Room { x: r.x + 1, y: r.y + 1, w: 3, h: r.h - 2 });
        out.push(Room { x: r.x + r.w - 4, y: r.y + 1, w: 3, h: r.h - 2 });
        out[0] = Room { x: r.x + 5, y: r.y + 1, w: r.w - 10, h: r.h - 2 };
    }
    let door = (r.x + r.w / 2, r.y + r.h - 1);
    f.set(door.0, door.1, Tile { ground, wall: Wall::None, feature: if ruined { Feature::None } else { Feature::Door { open: false, lock: 0 } } });
    if ruined {
        for y in r.y..r.y + r.h { for x in r.x..r.x + r.w {
            if f.at(x, y).wall == wall && b.chance(0.3) { f.set(x, y, Tile::floor(Ground::Rubble)); }
            else if f.at(x, y).wall == Wall::None && b.chance(0.12) { f.at_mut(x, y).ground = Ground::Rubble; }
        } }
    }
    out
}

/// Join the floor's open regions to the one at `from` (opened if it is rock) by tunnels; scraps
/// under eight cells become rock.
fn connect(f: &mut Floor, from: (i32, i32), wall: Wall) {
    if f.at(from.0, from.1).wall != Wall::None { let g = f.at(from.0, from.1).ground; f.set(from.0, from.1, Tile::floor(g)); }
    let pass = |f: &Floor, x: i32, y: i32| { let t = f.at(x, y); t.wall == Wall::None && !matches!(t.ground, Ground::Void) };
    let (w, h) = (f.w, f.h);
    for _round in 0..3 {
        // Label regions.
        let mut label = vec![u32::MAX; w * h];
        let mut sizes: Vec<(u32, (i32, i32))> = Vec::new();
        for y in 0..h as i32 { for x in 0..w as i32 {
            if !pass(f, x, y) || label[y as usize * w + x as usize] != u32::MAX { continue; }
            let id = sizes.len() as u32;
            let mut q = vec![(x, y)];
            label[y as usize * w + x as usize] = id;
            let mut n = 0u32;
            while let Some((cx, cy)) = q.pop() {
                n += 1;
                for (dx, dy) in DIRS4 { let (nx, ny) = (cx + dx, cy + dy); if f.inside(nx, ny) && pass(f, nx, ny) && label[ny as usize * w + nx as usize] == u32::MAX { label[ny as usize * w + nx as usize] = id; q.push((nx, ny)); } }
            }
            sizes.push((n, (x, y)));
        } }
        let main = label[from.1 as usize * w + from.0 as usize];
        let mut joined = false;
        for (id, &(n, start)) in sizes.iter().enumerate() {
            if id as u32 == main || n < 8 { continue; }
            // The nearest cell of the main region to this region's first cell.
            let mut best = (from, i32::MAX);
            for y in 0..h as i32 { for x in 0..w as i32 { if label[y as usize * w + x as usize] == main { let d = (x - start.0).abs() + (y - start.1).abs(); if d < best.1 { best = ((x, y), d); } } } }
            let g = f.at(start.0, start.1).ground;
            let (mut x, mut y) = start;
            let to = best.0;
            while x != to.0 { x += (to.0 - x).signum(); if f.at(x, y).wall != Wall::None && x > 0 && x < w as i32 - 1 { f.set(x, y, Tile::floor(g)); } }
            while y != to.1 { y += (to.1 - y).signum(); if f.at(x, y).wall != Wall::None && y > 0 && y < h as i32 - 1 { f.set(x, y, Tile::floor(g)); } }
            joined = true;
        }
        if !joined {
            // Scraps become rock.
            for y in 0..h as i32 { for x in 0..w as i32 { let k = y as usize * w + x as usize; if pass(f, x, y) && label[k] != main { let g = f.at(x, y).ground; f.set(x, y, Tile::wall(wall, g)); } } }
            return;
        }
    }
    // After three rounds: whatever is still apart becomes rock.
    let d = f.distances(from.0, from.1, i32::MAX - 1, |x, y| pass(f, x, y));
    for y in 0..h as i32 { for x in 0..w as i32 { if pass(f, x, y) && d[y as usize * w + x as usize] == i32::MAX { let g = f.at(x, y).ground; f.set(x, y, Tile::wall(wall, g)); } } }
}

/// The open cell farthest (by walking) from `from`.
fn farthest(f: &Floor, from: (i32, i32), avoid: &[(i32, i32)]) -> (i32, i32) {
    let d = f.distances(from.0, from.1, i32::MAX - 1, |x, y| f.at(x, y).wall == Wall::None && f.at(x, y).ground != Ground::Water);
    let mut best = (from, 0);
    for y in 1..f.h as i32 - 1 { for x in 1..f.w as i32 - 1 {
        let k = d[y as usize * f.w + x as usize];
        if k == i32::MAX || f.at(x, y).feature != Feature::None || !f.at(x, y).walkable() { continue; }
        if avoid.iter().any(|a| (a.0 - x).abs() + (a.1 - y).abs() < 4) { continue; }
        // Prefer a cell with walls on three sides (a dead end) a little.
        let walls = DIRS4.iter().filter(|(dx, dy)| f.at(x + dx, y + dy).wall != Wall::None).count() as i32;
        let score = k + walls * 2;
        if score > best.1 { best = ((x, y), score); }
    } }
    best.0
}

fn open_cells(f: &Floor) -> Vec<(i32, i32)> { f.cells(|t| t.walkable() && t.feature == Feature::None) }

// ---------------------------------------------------------------------------------------------
// Treasure

/// Gear of the place's tier: a metal and a quality to match.
pub fn gear(b: &mut Builder, tier: u32) -> Item {
    let ids: &[&str] = match tier {
        0 | 1 => &["dagger", "short_sword", "hatchet", "club", "leather_helmet", "leather_armor", "leather_legs", "leather_boots", "wooden_shield", "spear"],
        2 => &["sword", "axe", "mace", "chain_helmet", "brass_armor", "chain_legs", "studded_shield", "viking_helmet", "bow", "spear"],
        3 => &["sword", "axe", "mace", "scale_armor", "chain_armor", "chain_legs", "round_shield", "viking_helmet", "crossbow", "silver_ring", "wand_of_embers", "snakebite_rod"],
        4 => &["broadsword", "battle_axe", "battle_hammer", "chain_armor", "plate_legs", "round_shield", "steel_helmet", "silver_amulet", "power_ring"],
        _ => &["broadsword", "battle_axe", "battle_hammer", "plate_armor", "plate_legs", "dragon_shield", "steel_helmet", "power_ring", "wyvern_talisman"],
    };
    let id = b.pick(ids).unwrap();
    let def = data().item(id).unwrap();
    let metal = def.material.as_deref().map_or(false, |m| matches!(m, "iron" | "copper" | "steel" | "bronze"));
    let mut it = Item::new(id, 1);
    if metal {
        let mats: &[&str] = match tier { 0 | 1 => &["copper", "bronze", "iron"], 2 => &["bronze", "iron", "iron"], 3 => &["iron", "iron", "steel"], 4 => &["iron", "steel", "steel", "silver"], _ => &["steel", "steel", "silver", "adamantine"] };
        it.material = Some(b.pick(mats).unwrap().to_string());
    }
    let roll = b.range(0, 99);
    it.quality = match roll { 0..=54 => 0, 55..=74 => 1, 75..=86 => 2, 87..=94 => 3, 95..=98 => 4, _ => 5 };
    if tier >= 4 && it.quality < 2 { it.quality += 1; }
    it
}

/// A chest's worth at `tier`: gold, potions or food, sometimes gear, sometimes a gem.
pub fn treasure(b: &mut Builder, tier: u32) -> Vec<Item> {
    let mut v = vec![Item::new("gold", (b.range(5, 25) as u32) * tier.max(1) * tier.max(1))];
    if b.chance(0.6) { v.push(Item::new(if tier >= 4 { "strong_health_potion" } else { "health_potion" }, b.range(1, 2) as u32)); }
    if b.chance(0.3) { v.push(Item::new(if tier >= 4 { "strong_mana_potion" } else { "mana_potion" }, 1)); }
    if b.chance(0.35) { v.push(gear(b, tier)); }
    if b.chance(0.25) { v.push(Item::new(*b.pick(&["bread", "cheese", "meat", "torch", "torch", "rope"]).as_ref().unwrap(), b.range(1, 3) as u32)); }
    if tier >= 3 && b.chance(0.3) { v.push(Item::new(if tier >= 5 { "gem_large" } else { "gem_small" }, 1)); }
    v
}

// ---------------------------------------------------------------------------------------------

fn floor_size(kind: SiteKind, z: usize, b: &mut Builder) -> (usize, usize) {
    match kind {
        SiteKind::Wilds => (48, 36),
        SiteKind::Town => if z == 0 { (72, 56) } else { (56, 44) },
        SiteKind::Labyrinth => (61, 45),
        SiteKind::DarkFortress | SiteKind::Halls | SiteKind::Castle => (64, 48),
        _ => (b.range(46, 60) as usize, b.range(34, 46) as usize),
    }
}

/// Lay out floor `z` of a place of `kind`; `arrive` is where one comes down from above (open,
/// joined to the rest). Returns the floor and its rooms.
fn layout(b: &mut Builder, spec: &SiteSpec, z: usize, arrive: Option<(i32, i32)>, size: (usize, usize)) -> (Floor, Vec<Room>) {
    let kind = spec.kind;
    let (w, h) = size;
    let (wall, ground, room_ground) = under_style(kind);
    let surface_kinds = matches!(kind, SiteKind::Ruin | SiteKind::Tomb | SiteKind::Temple | SiteKind::Shrine | SiteKind::Castle | SiteKind::Camp | SiteKind::DarkFortress | SiteKind::Wilds);
    let (mut f, rs) = if z == 0 && surface_kinds {
        let trees = match spec.surface { Ground::Sand | Ground::Snow | Ground::Ash => 0.01, _ => 0.06 };
        let mut f = surface(b, w, h, spec.surface, if kind == SiteKind::Wilds { trees * 2.0 } else { trees });
        let cx = w as i32 / 2;
        let rs = match kind {
            SiteKind::Wilds => {
                // Glades and thickets: a few clearings.
                (0..4).map(|_| { let r = Room { x: b.range(4, w as i32 - 12), y: b.range(4, h as i32 - 10), w: 7, h: 5 }; for (x, y) in r.cells() { f.set(x, y, Tile::floor(spec.surface)); } r }).collect()
            }
            SiteKind::Camp => {
                // A palisade ring with tents and a fire; a cave mouth behind it.
                let r = Room { x: cx - 14, y: 8, w: 28, h: 20 };
                for y in r.y..r.y + r.h { for x in r.x..r.x + r.w {
                    let edge = x == r.x || y == r.y || x == r.x + r.w - 1 || y == r.y + r.h - 1;
                    f.set(x, y, if edge { Tile::wall(Wall::Palisade, Ground::Earth) } else { Tile::floor(Ground::Earth) });
                } }
                f.set(cx, r.y + r.h - 1, Tile::floor(Ground::Earth));
                f.set(cx + 1, r.y + r.h - 1, Tile::floor(Ground::Earth));
                f.at_mut(cx, r.y + r.h / 2).feature = Feature::Campfire;
                for k in 0..6 { let (tx, ty) = (r.x + 3 + (k % 3) * 9, r.y + 3 + (k / 3) * 11); if f.at(tx, ty).walkable() { f.at_mut(tx, ty).feature = Feature::Tent; } }
                vec![Room { x: r.x + 1, y: r.y + 1, w: r.w - 2, h: r.h - 2 }]
            }
            _ => {
                let (bw, bh) = match kind { SiteKind::Castle | SiteKind::DarkFortress => (30, 22), SiteKind::Temple | SiteKind::Shrine => (21, 15), SiteKind::Tomb => (11, 9), _ => (19, 13) };
                let r = Room { x: cx - bw / 2, y: (h as i32 - bh) / 2 - 2, w: bw, h: bh };
                let bwall = match kind { SiteKind::DarkFortress => Wall::Shadow, SiteKind::Ruin => Wall::Brick, _ => Wall::Brick };
                let bground = match kind { SiteKind::Temple | SiteKind::Shrine => Ground::Marble, SiteKind::Ruin => Ground::Flags, SiteKind::DarkFortress => Ground::Ash, _ => Ground::Flags };
                let rooms = building(b, &mut f, r, bwall, bground, kind == SiteKind::Ruin || (kind == SiteKind::Castle && spec.cause.contains("ruin")), matches!(kind, SiteKind::Temple | SiteKind::Shrine | SiteKind::Castle | SiteKind::DarkFortress));
                // Castle towers at the corners.
                if matches!(kind, SiteKind::Castle | SiteKind::DarkFortress) {
                    for (tx, ty) in [(r.x - 1, r.y - 1), (r.x + r.w - 2, r.y - 1), (r.x - 1, r.y + r.h - 2), (r.x + r.w - 2, r.y + r.h - 2)] {
                        for y in ty..ty + 3 { for x in tx..tx + 3 { f.set(x, y, Tile::wall(bwall, bground)); } }
                    }
                    // A gate instead of a door.
                    let d = (r.x + r.w / 2, r.y + r.h - 1);
                    f.set(d.0, d.1, Tile { ground: bground, wall: Wall::None, feature: Feature::Door { open: true, lock: 0 } });
                }
                rooms
            }
        };
        (f, rs)
    } else {
        match kind {
            SiteKind::Cave | SiteKind::Lair | SiteKind::Wilds => { let f = caves(b, w, h, wall, ground, 0.55); (f, Vec::new()) }
            SiteKind::Mine => mine(b, w, h),
            SiteKind::Tomb => catacombs(b, w, h, wall, ground),
            SiteKind::Labyrinth => maze(b, w, h, wall, ground),
            SiteKind::Temple | SiteKind::Shrine => if z % 2 == 1 { catacombs(b, w, h, wall, ground) } else { rooms(b, w, h, wall, ground, room_ground, 9, true) },
            SiteKind::Camp => { let f = caves(b, w, h, wall, Ground::Earth, 0.56); (f, Vec::new()) }
            SiteKind::Halls => rooms(b, w, h, wall, ground, Ground::Marble, 8, true),
            _ => rooms(b, w, h, wall, ground, room_ground, 10, true),
        }
    };
    f.name = floor_name(kind, z, spec.floors);
    let start = arrive.unwrap_or_else(|| {
        if f.outdoor { (w as i32 / 2, h as i32 - 2) } else { open_cells(&f).first().copied().unwrap_or((w as i32 / 2, h as i32 / 2)) }
    });
    // The way in from the south edge on an open floor: a path up to the building's door.
    if z == 0 && f.outdoor {
        let g = f.at(start.0, start.1).ground;
        for y in (h as i32 / 2)..h as i32 - 1 { let t = f.at(w as i32 / 2, y).clone(); if t.wall == Wall::Tree || t.wall == Wall::Rock { f.set(w as i32 / 2, y, Tile::floor(g)); } }
    }
    connect(&mut f, start, wall);
    (f, rs)
}

/// Monster kinds for a floor: those of its habitats up to the floor's tier, favouring the tier.
fn spawn_table(kind: SiteKind, tier: u32, z: usize) -> Vec<&'static str> {
    let (first, below) = kind.habitats();
    let habs = if z == 0 { first } else { below };
    let t = (tier + z as u32 / 2).min(6);
    let mut v: Vec<&'static str> = Vec::new();
    for hab in habs {
        for m in data().living_in(hab, t) {
            // Weight: the floor's own tier 3, a tier below 2, older ones 1.
            let wgt = match t.saturating_sub(m.tier) { 0 => 3, 1 => 2, 2 => 1, _ => 0 };
            for _ in 0..wgt { v.push(m.id.as_str()); }
        }
    }
    v
}

/// Build the place.
pub fn realize(spec: &SiteSpec) -> Place {
    if spec.kind == SiteKind::Town { return super::town::realize(spec); }
    let mut b = Builder { rng: ChaCha8Rng::seed_from_u64(spec.seed ^ 0xAD7E_0001) };
    let n = spec.floors.max(1);
    let mut floors: Vec<Floor> = Vec::new();
    let mut monsters: Vec<Monster> = Vec::new();
    let mut uid = 1u32;
    let mut arrive: Option<(i32, i32)> = None;
    let mut entry = (0, 0);
    let mut lock_id = 1u32;
    // (One size for every floor: stairs land where they leave.)
    let size = floor_size(spec.kind, 0, &mut b);
    for z in 0..n {
        let (mut f, rs) = layout(&mut b, spec, z, arrive, size);
        let start = arrive.unwrap_or_else(|| if f.outdoor { (f.w as i32 / 2, f.h as i32 - 2) } else { open_cells(&f)[0] });
        if z == 0 {
            entry = start;
            // Underground first floors (caves, mines, labyrinths): the way out where one came in.
            f.at_mut(start.0, start.1).feature = Feature::Exit;
            if f.outdoor { f.at_mut(start.0, start.1).feature = Feature::Exit; }
        } else {
            // How one came down: stairs, a ladder, or a hole (with a rope spot to climb back).
            let up = match spec.kind { SiteKind::Cave | SiteKind::Lair => if b.chance(0.5) { Feature::RopeSpot } else { Feature::LadderUp }, SiteKind::Mine => Feature::LadderUp, _ => Feature::StairsUp };
            f.at_mut(start.0, start.1).feature = up;
        }
        // The way down, far from where one arrived.
        let mut down = None;
        if z + 1 < n {
            let d = farthest(&f, start, &[]);
            let feat = match spec.kind { SiteKind::Cave | SiteKind::Lair => if b.chance(0.5) { Feature::Hole } else { Feature::LadderDown }, SiteKind::Mine => Feature::LadderDown, _ => Feature::StairsDown };
            // In a surface building the stairs go in its main room.
            let d = if z == 0 && f.outdoor && !rs.is_empty() && spec.kind != SiteKind::Wilds { let c = rs[0].centre(); (c.0, rs[0].y + 1) } else { d };
            let g = f.at(d.0, d.1).ground;
            f.set(d.0, d.1, Tile { ground: g, wall: Wall::None, feature: if feat == Feature::Hole && spec.kind != SiteKind::Lair && spec.kind != SiteKind::Cave { Feature::StairsDown } else { feat.clone() } });
            // The way to the deepest floor is locked: its key lies on this floor (a chest or a guard).
            if z + 2 == n && n >= 2 && matches!(spec.kind, SiteKind::Tomb | SiteKind::Temple | SiteKind::Shrine | SiteKind::Castle | SiteKind::DarkFortress | SiteKind::Halls | SiteKind::Ruin) {
                // A door in a wall cell beside the stairs: wall the stairs in with a locked door.
                let (dx, dy) = d;
                let g = f.at(dx, dy).ground;
                let wall = under_style(spec.kind).0;
                // The door on the side one comes from (nearest the arrival by walking); the rest walled.
                let dmap = f.distances(start.0, start.1, i32::MAX - 1, |x, y| f.at(x, y).wall == Wall::None);
                let door_at = DIRS4.iter().map(|(ox, oy)| (dx + ox, dy + oy)).filter(|&(nx, ny)| f.inside(nx, ny) && f.at(nx, ny).walkable() && f.at(nx, ny).feature == Feature::None)
                    .min_by_key(|&(nx, ny)| dmap[ny as usize * f.w + nx as usize]);
                for (ox, oy) in DIRS8 { let (nx, ny) = (dx + ox, dy + oy); if Some((nx, ny)) != door_at && f.inside(nx, ny) && f.at(nx, ny).walkable() && f.at(nx, ny).feature == Feature::None { f.set(nx, ny, Tile::wall(wall, g)); } }
                if let Some((x, y)) = door_at {
                    f.set(x, y, Tile { ground: g, wall: Wall::None, feature: Feature::Door { open: false, lock: lock_id } });
                    // The key: in a chest at the far end of the floor from the door.
                    let kpos = farthest(&f, start, &[d, (x, y)]);
                    let key = Item::key(lock_id, &format!("{}", floor_name(spec.kind, z + 1, n)));
                    if b.chance(0.5) { f.at_mut(kpos.0, kpos.1).feature = Feature::Chest { items: vec![key], opened: false, lock: 0, quest: 0 }; }
                    else {
                        // Carried by the strongest thing here: drop it on its cell (it is guarded).
                        f.drop_item(kpos.0, kpos.1, key);
                    }
                    lock_id += 1;
                }
            }
            // Labyrinths and castles: a portcullis on the way down, a lever somewhere on the floor.
            if matches!(spec.kind, SiteKind::Labyrinth | SiteKind::Halls) && z > 0 || spec.kind == SiteKind::Labyrinth {
                let (dx, dy) = d;
                for (ox, oy) in DIRS4 {
                    let (nx, ny) = (dx + ox, dy + oy);
                    if f.at(nx, ny).walkable() && f.at(nx, ny).feature == Feature::None {
                        let g = f.at(nx, ny).ground;
                        f.set(nx, ny, Tile { ground: g, wall: Wall::None, feature: Feature::Gate { open: false, lever: lock_id } });
                        // Seal the other sides.
                        for (px, py) in DIRS8 { let (qx, qy) = (dx + px, dy + py); if (qx, qy) != (nx, ny) && f.at(qx, qy).walkable() && f.at(qx, qy).feature == Feature::None { f.set(qx, qy, Tile::wall(under_style(spec.kind).0, g)); } }
                        let lp = farthest(&f, start, &[d]);
                        f.at_mut(lp.0, lp.1).feature = Feature::Lever { id: lock_id, pulled: false };
                        lock_id += 1;
                        break;
                    }
                }
            }
            down = Some(d);
        }
        furnish(&mut b, &mut f, spec, z, &rs, start);
        // Monsters.
        let table = spawn_table(spec.kind, spec.tier, z);
        let cells = open_cells(&f);
        let count = (cells.len() / if spec.kind == SiteKind::Wilds { 90 } else { 38 }).max(3) + z * 2;
        for _ in 0..count {
            let Some(id) = b.pick(&table) else { break };
            let Some(&(x, y)) = b.pick(&cells).as_ref() else { break };
            if (x - start.0).abs() + (y - start.1).abs() < 8 { continue; }
            let pack = data().monster(id).map_or(1, |m| m.pack.max(1));
            for k in 0..pack {
                let (px, py) = (x + (k as i32 % 2), y + (k as i32 / 2));
                if !f.walkable(px, py) || monsters.iter().any(|m| m.z == z && m.x == px && m.y == py) { continue; }
                monsters.push(Monster::new(uid, id, px, py, z));
                uid += 1;
            }
        }
        // The boss on the last floor, at its far end, with the hoard and the "choose one" chest.
        if z + 1 == n {
            let bp = farthest(&f, start, &[]);
            if let Some(boss) = &spec.boss {
                let mut m = Monster::boss(uid, &boss.def, &boss.name, boss.scale, bp.0, bp.1, z);
                m.legend = boss.legend.clone();
                m.carries = boss.hoard.clone();
                uid += 1;
                monsters.retain(|o| !(o.z == z && (o.x - bp.0).abs() + (o.y - bp.1).abs() < 3));
                monsters.push(m);
            }
            // A quest chest beside it: three rewards, one may be taken (Tibia).
            let near = open_cells(&f).into_iter().filter(|&(x, y)| (x - bp.0).abs() + (y - bp.1).abs() <= 4 && (x, y) != bp).min_by_key(|&(x, y)| (x - bp.0).abs() + (y - bp.1).abs());
            if let Some((x, y)) = near {
                let t = spec.tier + 1;
                let mut choices = vec![gear(&mut b, t), gear(&mut b, t), gear(&mut b, t)];
                for c in choices.iter_mut() { c.quality = c.quality.max(2); }
                choices.extend(spec.treasures.iter().cloned());
                f.at_mut(x, y).feature = Feature::QuestChest { choices, taken: false, quest: spec.id };
            }
        }
        // Below a hole: a ladder back up somewhere far off (one is never shut in without a rope).
        if z > 0 && f.at(start.0, start.1).feature == Feature::RopeSpot {
            let above = &floors[z - 1];
            let d = f.distances(start.0, start.1, i32::MAX - 1, |x, y| f.at(x, y).walkable());
            let mut best: Option<((i32, i32), i32)> = None;
            for y in 1..f.h as i32 - 1 { for x in 1..f.w as i32 - 1 {
                let k = d[y as usize * f.w + x as usize];
                if k == i32::MAX || k < 12 || f.at(x, y).feature != Feature::None || !f.at(x, y).walkable() { continue; }
                if !above.walkable(x, y) || above.at(x, y).feature != Feature::None { continue; }
                if best.map_or(true, |b| k > b.1) { best = Some(((x, y), k)); }
            } }
            if let Some(((x, y), _)) = best {
                f.at_mut(x, y).feature = Feature::LadderUp;
                floors[z - 1].at_mut(x, y).feature = Feature::LadderDown;
            }
        }
        let _ = down;
        arrive = down;
        floors.push(f);
    }
    Place { spec: spec.clone(), floors, monsters, npcs: Vec::new(), entry, next_uid: uid }
}

/// Dress the rooms and corridors of a floor by what the place is.
fn furnish(b: &mut Builder, f: &mut Floor, spec: &SiteSpec, z: usize, rs: &[Room], start: (i32, i32)) {
    let kind = spec.kind;
    let free = |f: &Floor, x: i32, y: i32| f.inside(x, y) && f.at(x, y).walkable() && f.at(x, y).feature == Feature::None && (x - start.0).abs() + (y - start.1).abs() > 2;
    let put = |f: &mut Floor, x: i32, y: i32, feat: Feature| { if free(f, x, y) { f.at_mut(x, y).feature = feat; } };
    // Rooms by kind.
    for (k, r) in rs.iter().enumerate() {
        let (cx, cy) = r.centre();
        match kind {
            SiteKind::Temple | SiteKind::Shrine if z == 0 && k == 0 => {
                put(f, cx, r.y + 1, Feature::Altar);
                for x in [r.x + 1, r.x + r.w - 2] { for y in (r.y + 2..r.y + r.h - 1).step_by(3) { put(f, x, y, Feature::Pillar); } }
                put(f, cx - 2, r.y + 1, Feature::Brazier); put(f, cx + 2, r.y + 1, Feature::Brazier);
                for y in (r.y + 3..r.y + r.h - 2).step_by(2) { put(f, cx - 2, y, Feature::Table); put(f, cx + 2, y, Feature::Table); }
            }
            SiteKind::Castle | SiteKind::DarkFortress if z == 0 && k == 0 => {
                put(f, cx, r.y + 1, Feature::Throne);
                for x in (r.x + 2..r.x + r.w - 2).step_by(4) { put(f, x, cy, Feature::Table); }
                put(f, r.x + 1, r.y + 1, Feature::Sconce); put(f, r.x + r.w - 2, r.y + 1, Feature::Sconce);
            }
            SiteKind::Tomb if z == 0 => {
                put(f, cx, cy, Feature::Sarcophagus { items: treasure(b, spec.tier), opened: false });
                put(f, r.x, r.y, Feature::Brazier);
            }
            SiteKind::Halls => {
                for x in (r.x + 1..r.x + r.w - 1).step_by(3) { put(f, x, r.y, Feature::Pillar); put(f, x, r.y + r.h - 1, Feature::Pillar); }
                if b.chance(0.3) { put(f, cx, cy, Feature::Statue); }
                if b.chance(0.25) { put(f, r.x, cy, Feature::Anvil); }
            }
            SiteKind::Ruin if z > 0 => {
                if b.chance(0.5) { put(f, r.x, r.y, Feature::Barrel); }
                if b.chance(0.4) { put(f, r.x + r.w - 1, r.y, Feature::Crate); }
                if b.chance(0.2) { put(f, cx, cy, Feature::Bookshelf); }
            }
            SiteKind::Camp if z == 0 => {}
            _ => {
                if b.chance(0.3) { put(f, r.x, r.y, Feature::Barrel); }
                if b.chance(0.2) { put(f, r.x + r.w - 1, r.y + r.h - 1, Feature::Crate); }
                if b.chance(0.25) { put(f, cx, cy, Feature::Table); }
            }
        }
        // A chest in some rooms.
        if b.chance(if z == 0 && f.outdoor { 0.25 } else { 0.35 }) {
            let (x, y) = (r.x + r.w - 1, r.y);
            put(f, x, y, Feature::Chest { items: treasure(b, spec.tier + z as u32 / 2), opened: false, lock: 0, quest: 0 });
        }
    }
    // Scattered things over the whole floor.
    let cells = open_cells(f);
    let n = cells.len();
    for &(x, y) in &cells {
        if !free(f, x, y) { continue; }
        let walls = DIRS4.iter().filter(|(dx, dy)| f.at(x + dx, y + dy).wall != Wall::None).count();
        let p = b.rng.gen_range(0..10_000) as f64 / 10_000.0;
        match kind {
            SiteKind::Cave | SiteKind::Lair | SiteKind::Camp if !f.outdoor => {
                if p < 0.012 { f.at_mut(x, y).feature = Feature::Bones; }
                else if p < 0.02 && walls >= 2 { f.at_mut(x, y).feature = Feature::Web; }
                else if p < 0.024 && walls >= 3 { f.at_mut(x, y).feature = Feature::Chest { items: treasure(b, spec.tier + z as u32 / 2), opened: false, lock: 0, quest: 0 }; }
                else if p < 0.06 { f.at_mut(x, y).ground = Ground::Moss; }
                else if p < 0.07 && spec.kind == SiteKind::Lair { f.at_mut(x, y).ground = Ground::Mud; }
            }
            SiteKind::Mine => {
                if p < 0.008 { f.at_mut(x, y).feature = Feature::Crate; }
                else if p < 0.012 && walls >= 3 { f.at_mut(x, y).feature = Feature::Chest { items: treasure(b, spec.tier), opened: false, lock: 0, quest: 0 }; }
                else if p < 0.02 { f.at_mut(x, y).feature = Feature::Bones; }
            }
            SiteKind::Tomb | SiteKind::Temple | SiteKind::Shrine | SiteKind::Castle | SiteKind::DarkFortress if !f.outdoor => {
                if p < 0.01 && walls >= 1 { f.at_mut(x, y).feature = Feature::Sconce; }
                else if p < 0.016 { f.at_mut(x, y).feature = Feature::Bones; }
                else if p < 0.019 && walls >= 3 { f.at_mut(x, y).feature = Feature::Chest { items: treasure(b, spec.tier + z as u32 / 2), opened: false, lock: 0, quest: 0 }; }
                else if p < 0.022 { f.at_mut(x, y).feature = Feature::Trap { armed: true, damage: 8 + spec.tier as i32 * 6 }; }
            }
            SiteKind::Labyrinth => {
                if p < 0.01 { f.at_mut(x, y).feature = Feature::Bones; }
                else if p < 0.016 && walls >= 3 { f.at_mut(x, y).feature = Feature::Chest { items: treasure(b, spec.tier), opened: false, lock: 0, quest: 0 }; }
            }
            _ if f.outdoor => {
                if p < 0.004 { f.at_mut(x, y).feature = Feature::Bones; }
                if kind == SiteKind::Ruin && p > 0.995 { f.at_mut(x, y).feature = Feature::Chest { items: treasure(b, spec.tier), opened: false, lock: 0, quest: 0 }; }
            }
            _ => {}
        }
    }
    // Sarcophagi in catacombs hold the dead's goods (and sometimes wake them).
    for y in 0..f.h as i32 { for x in 0..f.w as i32 {
        if let Feature::Sarcophagus { items, .. } = &mut f.at_mut(x, y).feature { if items.is_empty() && b.chance(0.4) { *items = treasure(b, spec.tier); } }
    } }
    // A lost treasure of the history lies on a plinth where there is no boss to hold it.
    if z + 1 == spec.floors && spec.boss.is_none() && !spec.treasures.is_empty() {
        let p = farthest(f, start, &[]);
        f.at_mut(p.0, p.1).feature = Feature::Plinth { item: spec.treasures.first().cloned() };
    }
    let _ = n;
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn spec(kind: SiteKind, floors: usize, tier: u32, seed: u64) -> SiteSpec {
        SiteSpec { id: 1, kind, name: "Testplace".into(), tile: (0, 0), seed, tier, cause: String::new(), boss: Some(BossSpec { def: "troll".into(), name: "Gnash the Old".into(), scale: 1.5, legend: None, hoard: vec![Item::new("gold", 200)], story: String::new() }),
            treasures: vec![], surface: Ground::Grass, rock: "granite".into(), floors, people: "human".into(), god: "the gods".into(), news: Vec::new() }
    }

    /// Every floor's way down is reachable from where one arrives (doors and gates passable,
    /// keys and levers assumed), and every place is the same twice.
    #[test]
    fn places_are_whole_and_deterministic() {
        for kind in [SiteKind::Cave, SiteKind::Lair, SiteKind::Mine, SiteKind::Ruin, SiteKind::Tomb, SiteKind::Temple, SiteKind::Shrine, SiteKind::Castle, SiteKind::Labyrinth, SiteKind::Camp, SiteKind::Halls, SiteKind::DarkFortress, SiteKind::Wilds] {
            for seed in 0..6u64 {
                let s = spec(kind, if kind == SiteKind::Wilds { 1 } else { 3 }, 2, seed * 7919 + kind as u64);
                let p = realize(&s);
                let q = realize(&s);
                assert_eq!(p.floors.len(), q.floors.len());
                assert_eq!(p.monsters.len(), q.monsters.len(), "{:?} {}", kind, seed);
                for (z, f) in p.floors.iter().enumerate() {
                    let from = if z == 0 { p.entry } else { p.floors[z - 1].find(|t| matches!(t, Feature::StairsDown | Feature::LadderDown | Feature::Hole)).expect("way down above") };
                    assert!(f.inside(from.0, from.1));
                    let d = f.distances(from.0, from.1, i32::MAX - 1, |x, y| { let t = f.at(x, y); t.wall == Wall::None && !matches!(t.ground, Ground::Water | Ground::Lava | Ground::Void) });
                    if z + 1 < p.floors.len() {
                        let down = f.find(|t| matches!(t, Feature::StairsDown | Feature::LadderDown | Feature::Hole)).unwrap_or_else(|| panic!("{:?} seed {} floor {} has no way down", kind, seed, z));
                        assert!(d[down.1 as usize * f.w + down.0 as usize] < i32::MAX, "{:?} seed {} floor {}: way down unreachable", kind, seed, z);
                    }
                    let open = f.cells(|t| t.walkable()).len();
                    assert!(open > 60, "{:?} {} floor {} too small ({})", kind, seed, z, open);
                }
                assert!(p.monsters.iter().any(|m| m.boss), "{:?} has its boss", kind);
            }
        }
    }
}
