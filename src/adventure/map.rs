//! A place realized for the adventurer: floors of cells, each a ground, maybe a wall, maybe a
//! feature (a door, stairs, a chest, an altar...), with things lying on it. Floors stack: stairs
//! down at (x, y) on floor z arrive at stairs up at the same (x, y) on floor z + 1 (Tibia's
//! rule), so a dungeon is read floor by floor like an old plan.

use super::item::Item;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ground { Rock, Flags, Earth, Grass, Sand, Snow, Wood, Shallows, Water, Lava, Rubble, Mud, Marble, Cobbles, Moss, Ash, Carpet, Void }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Wall { None, Rock, Brick, Timber, Tree, Bars, Palisade, Hedge, Shadow }

#[derive(Clone, Debug, PartialEq)]
pub enum Feature {
    None,
    /// A door: open or shut, locked by a key's tag (0: no lock).
    Door { open: bool, lock: u32 },
    /// A portcullis a lever lifts.
    Gate { open: bool, lever: u32 },
    Lever { id: u32, pulled: bool },
    StairsDown,
    StairsUp,
    /// A hole down (one comes back up only by a rope at the rope spot below).
    Hole,
    /// Below a hole: a rope climbs back up.
    RopeSpot,
    LadderUp,
    LadderDown,
    /// The way out of the place (to the world map).
    Exit,
    Chest { items: Vec<Item>, opened: bool, lock: u32, quest: u32 },
    /// "You may only choose one" (Tibia's quest chests): one of its rewards per adventurer.
    QuestChest { choices: Vec<Item>, taken: bool, quest: u32 },
    Sarcophagus { items: Vec<Item>, opened: bool },
    Altar,
    Fountain,
    Brazier,
    Statue,
    Pillar,
    Bones,
    Web,
    Table,
    Bed,
    Barrel,
    Crate,
    Bookshelf,
    Throne,
    Anvil,
    Counter,
    Well,
    Grave,
    Tent,
    Campfire,
    /// A wall sconce's torch: light about it.
    Sconce,
    /// A plinth that held or holds a treasure.
    Plinth { item: Option<Item> },
    /// A sewer grate or trapdoor into the place below a town.
    Grate,
    Sign { text: String },
    /// A pressure plate: a dart, a fall of stones.
    Trap { armed: bool, damage: i32 },
    Rail,
}

impl Feature {
    pub fn blocks(&self) -> bool {
        match self {
            Feature::Door { open, .. } | Feature::Gate { open, .. } => !*open,
            Feature::Statue | Feature::Pillar | Feature::Bookshelf | Feature::Anvil | Feature::Counter | Feature::Well
            | Feature::Barrel | Feature::Crate | Feature::Tent | Feature::Fountain | Feature::Sarcophagus { .. } | Feature::Plinth { .. }
            | Feature::Chest { .. } | Feature::QuestChest { .. } | Feature::Lever { .. } | Feature::Altar | Feature::Throne => true,
            _ => false,
        }
    }
    pub fn blocks_sight(&self) -> bool {
        match self {
            Feature::Door { open, .. } => !*open,
            Feature::Bookshelf | Feature::Tent => true,
            _ => false,
        }
    }
    pub fn word(&self) -> &'static str {
        match self {
            Feature::None => "", Feature::Door { open: true, .. } => "an open door", Feature::Door { lock, .. } if *lock > 0 => "a locked door",
            Feature::Door { .. } => "a door", Feature::Gate { open: true, .. } => "a raised portcullis", Feature::Gate { .. } => "a portcullis",
            Feature::Lever { .. } => "a lever", Feature::StairsDown => "stairs down", Feature::StairsUp => "stairs up", Feature::Hole => "a hole in the floor",
            Feature::RopeSpot => "a rope spot", Feature::LadderUp => "a ladder up", Feature::LadderDown => "a ladder down", Feature::Exit => "the way out",
            Feature::Chest { opened: true, .. } => "an open chest", Feature::Chest { .. } => "a chest", Feature::QuestChest { .. } => "an iron-bound chest",
            Feature::Sarcophagus { opened: true, .. } => "an opened sarcophagus", Feature::Sarcophagus { .. } => "a sarcophagus", Feature::Altar => "an altar",
            Feature::Fountain => "a fountain", Feature::Brazier => "a brazier", Feature::Statue => "a statue", Feature::Pillar => "a pillar",
            Feature::Bones => "bones", Feature::Web => "a web", Feature::Table => "a table", Feature::Bed => "a bed", Feature::Barrel => "a barrel",
            Feature::Crate => "a crate", Feature::Bookshelf => "a bookshelf", Feature::Throne => "a throne", Feature::Anvil => "an anvil",
            Feature::Counter => "a counter", Feature::Well => "a well", Feature::Grave => "a grave", Feature::Tent => "a tent", Feature::Campfire => "a campfire",
            Feature::Sconce => "a torch in a sconce", Feature::Plinth { item: Some(_) } => "a plinth with something on it", Feature::Plinth { .. } => "an empty plinth",
            Feature::Grate => "a grate", Feature::Sign { .. } => "a sign", Feature::Trap { .. } => "a pressure plate", Feature::Rail => "rails",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tile { pub ground: Ground, pub wall: Wall, pub feature: Feature }

impl Tile {
    pub fn floor(g: Ground) -> Tile { Tile { ground: g, wall: Wall::None, feature: Feature::None } }
    pub fn wall(w: Wall, g: Ground) -> Tile { Tile { ground: g, wall: w, feature: Feature::None } }
    pub fn walkable(&self) -> bool { self.wall == Wall::None && !matches!(self.ground, Ground::Water | Ground::Lava | Ground::Void) && !self.feature.blocks() }
    pub fn opaque(&self) -> bool { !matches!(self.wall, Wall::None | Wall::Bars | Wall::Palisade) || self.feature.blocks_sight() }
}

/// One floor of a place.
#[derive(Clone, Debug)]
pub struct Floor {
    pub w: usize,
    pub h: usize,
    pub tiles: Vec<Tile>,
    /// Its name ("the second cellar", "the burial gallery").
    pub name: String,
    /// Open sky: lit by day, the whole floor in sight to the light's reach.
    pub outdoor: bool,
    /// Things lying on cells.
    pub items: HashMap<(i32, i32), Vec<Item>>,
    /// Cells the adventurer has seen (remembered on the map).
    pub seen: Vec<bool>,
}

impl Floor {
    pub fn new(w: usize, h: usize, fill: Tile, name: &str, outdoor: bool) -> Floor {
        Floor { w, h, tiles: vec![fill; w * h], name: name.into(), outdoor, items: HashMap::new(), seen: vec![false; w * h] }
    }
    pub fn inside(&self, x: i32, y: i32) -> bool { x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h }
    pub fn at(&self, x: i32, y: i32) -> &Tile {
        static VOID: std::sync::OnceLock<Tile> = std::sync::OnceLock::new();
        if self.inside(x, y) { &self.tiles[y as usize * self.w + x as usize] } else { VOID.get_or_init(|| Tile::wall(Wall::Rock, Ground::Void)) }
    }
    pub fn at_mut(&mut self, x: i32, y: i32) -> &mut Tile { let w = self.w; &mut self.tiles[y as usize * w + x as usize] }
    pub fn set(&mut self, x: i32, y: i32, t: Tile) { if self.inside(x, y) { *self.at_mut(x, y) = t; } }
    pub fn walkable(&self, x: i32, y: i32) -> bool { self.inside(x, y) && self.at(x, y).walkable() }
    pub fn drop_item(&mut self, x: i32, y: i32, it: Item) { super::item::stow(self.items.entry((x, y)).or_default(), it); }
    /// Cells where `pred` holds.
    pub fn cells(&self, pred: impl Fn(&Tile) -> bool) -> Vec<(i32, i32)> {
        let mut v = Vec::new();
        for y in 0..self.h as i32 { for x in 0..self.w as i32 { if pred(self.at(x, y)) { v.push((x, y)); } } }
        v
    }
    pub fn find(&self, pred: impl Fn(&Feature) -> bool) -> Option<(i32, i32)> {
        for y in 0..self.h as i32 { for x in 0..self.w as i32 { if pred(&self.at(x, y).feature) { return Some((x, y)); } } }
        None
    }

    /// What the adventurer at (x, y) sees with light reaching `r` cells: rays to every cell on the
    /// square of radius r (walls stop them, the wall itself is seen).
    pub fn sight(&self, x: i32, y: i32, r: i32) -> Vec<bool> {
        let mut vis = vec![false; self.w * self.h];
        if !self.inside(x, y) { return vis; }
        vis[y as usize * self.w + x as usize] = true;
        let mut ray = |tx: i32, ty: i32| {
            // Bresenham from the centre of (x, y) to (tx, ty).
            let (dx, dy) = ((tx - x).abs(), -(ty - y).abs());
            let (sx, sy) = (if tx > x { 1 } else { -1 }, if ty > y { 1 } else { -1 });
            let (mut cx, mut cy, mut err) = (x, y, dx + dy);
            loop {
                if cx == tx && cy == ty { break; }
                let e2 = 2 * err;
                if e2 >= dy { err += dy; cx += sx; }
                if e2 <= dx { err += dx; cy += sy; }
                if !self.inside(cx, cy) { break; }
                if (cx - x) * (cx - x) + (cy - y) * (cy - y) > r * r + r { break; }
                vis[cy as usize * self.w + cx as usize] = true;
                if self.at(cx, cy).opaque() { break; }
            }
        };
        for k in -r..=r { ray(x + k, y - r); ray(x + k, y + r); ray(x - r, y + k); ray(x + r, y + k); }
        vis
    }

    /// Whether a missile flies from a to b unblocked.
    pub fn clear_line(&self, a: (i32, i32), b: (i32, i32)) -> bool {
        let (dx, dy) = ((b.0 - a.0).abs(), -(b.1 - a.1).abs());
        let (sx, sy) = (if b.0 > a.0 { 1 } else { -1 }, if b.1 > a.1 { 1 } else { -1 });
        let (mut cx, mut cy, mut err) = (a.0, a.1, dx + dy);
        loop {
            if (cx, cy) == b { return true; }
            let e2 = 2 * err;
            if e2 >= dy { err += dy; cx += sx; }
            if e2 <= dx { err += dx; cy += sy; }
            if (cx, cy) != b && self.at(cx, cy).opaque() { return false; }
        }
    }

    /// Steps from (x, y) to every cell within `max` steps (8-way; i32::MAX where unreached):
    /// monsters walk down it toward the adventurer.
    pub fn distances(&self, x: i32, y: i32, max: i32, pass: impl Fn(i32, i32) -> bool) -> Vec<i32> {
        let mut d = vec![i32::MAX; self.w * self.h];
        if !self.inside(x, y) { return d; }
        let mut q = std::collections::VecDeque::new();
        d[y as usize * self.w + x as usize] = 0;
        q.push_back((x, y));
        while let Some((cx, cy)) = q.pop_front() {
            let cd = d[cy as usize * self.w + cx as usize];
            if cd >= max { continue; }
            for (ddx, ddy) in DIRS8 {
                let (nx, ny) = (cx + ddx, cy + ddy);
                if !self.inside(nx, ny) { continue; }
                let k = ny as usize * self.w + nx as usize;
                if d[k] != i32::MAX || !pass(nx, ny) { continue; }
                // No cutting a corner between two walls.
                if ddx != 0 && ddy != 0 && !self.at(cx + ddx, cy).walkable() && !self.at(cx, cy + ddy).walkable() { continue; }
                d[k] = cd + 1;
                q.push_back((nx, ny));
            }
        }
        d
    }
}

pub const DIRS8: [(i32, i32); 8] = [(0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1)];
pub const DIRS4: [(i32, i32); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
