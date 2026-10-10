//! Walking the land (`surface`): the 3 x 3 world tiles about the adventurer are one floor of the
//! place `LAND`, put together from their chunks and taken apart again as one walks on (the tile
//! one walks into becomes the middle; what changed in the tiles left behind, the things lying
//! there, the creatures and people, what was seen, is kept per chunk). Nothing is entered from a
//! menu: one walks out of a town's gate, down the road, over the border of the next tile into
//! the next wood, and into a cave's mouth. The world map is the fast way (DF's travel), and an
//! ambush on the road drops one onto the land where it happens.
//!
//! Day and night (`DAY` ticks, the hour from `turn`): night narrows sight under the sky, brings
//! out the things of the night by the land and its history (the dead of a battlefield, wights,
//! a werewolf at the full moon), and dawn sends them off.
//!
//! The twist, the Mapmaker: the world map starts blank but for one's home and what one has heard
//! of. Walking inks the tiles one crosses (from high ground, farther); rumours and the sage's old
//! maps sketch them in; the walked route is drawn. Travel over inked country is quick and over
//! blank country slow and dangerous; new tiles walked on foot give experience, and a sage buys
//! one's charts. Treasure maps found on the dead mark a cache in another tile, dug up with a
//! shovel.

use super::actor::{Monster, Npc};
use super::data::data;
use super::game::{Game, Tone};
use super::item::{stow, Item};
use super::map::{Feature, Floor, Ground, Tile, Wall, DIRS8};
use super::site::{realize, Builder, Place, SiteKind, SiteSpec};
use super::surface::{self, Atlas, Land, CH};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::collections::HashMap;

/// The place id of the land about the adventurer.
pub const LAND: u32 = u32::MAX - 7;
/// Ticks in a day (a step at walking pace is 100: a minute).
pub const DAY: u64 = 144_000;

/// A chunk of the land as it was left: what changed from how it was made, what lies and lives
/// there, what was seen, when its creatures were last set.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Chunk {
    pub diff: Vec<(u16, Tile)>,
    #[serde(with = "super::map::cell_map")]
    pub items: HashMap<(i32, i32), Vec<Item>>,
    pub monsters: Vec<Monster>,
    pub npcs: Vec<Npc>,
    /// Seen cells, 64 a word.
    pub seen: Vec<u64>,
    pub stocked: u64,
}

/// Chunks saved as a list of pairs (JSON keys must be strings), in order.
pub mod chunk_map {
    use super::Chunk;
    use std::collections::HashMap;
    pub fn serialize<S: serde::Serializer>(m: &HashMap<(u32, u32), Chunk>, s: S) -> Result<S::Ok, S::Error> {
        let mut v: Vec<(&(u32, u32), &Chunk)> = m.iter().collect();
        v.sort_by_key(|e| *e.0);
        serde::Serialize::serialize(&v, s)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<HashMap<(u32, u32), Chunk>, D::Error> {
        let v: Vec<((u32, u32), Chunk)> = serde::Deserialize::deserialize(d)?;
        Ok(v.into_iter().collect())
    }
}

fn hour_at(turn: u64) -> f32 { ((turn % DAY) as f32 / DAY as f32 * 24.0 + 8.0) % 24.0 }
fn night_at(turn: u64) -> bool { let h = hour_at(turn); !(5.0..20.5).contains(&h) }

impl Game {
    pub fn on_land(&self) -> bool { self.here == Some(LAND) }
    /// The hour of the day (the adventure begins at eight in the morning).
    pub fn hour(&self) -> f32 { hour_at(self.turn) }
    pub fn night(&self) -> bool { night_at(self.turn) }
    /// The moon is full one night in 28 (the werewolf's night).
    pub fn full_moon(&self) -> bool { (self.turn / DAY) % 28 == 13 }
    /// How light it is under the sky: 1 by day, a quarter at night, between at dusk and dawn.
    pub fn daylight(&self) -> f32 {
        let h = self.hour();
        let day = if h < 4.5 || h > 21.0 { 0.0 } else if h < 6.5 { (h - 4.5) / 2.0 } else if h > 19.0 { (21.0 - h) / 2.0 } else { 1.0 };
        0.25 + 0.75 * day
    }

    // -----------------------------------------------------------------------------------------
    // Where things are

    /// The world cell of the land floor's cell (0, 0).
    pub fn origin(&self) -> (i32, i32) { ((self.centre.0 as i32 - 1) * CH, (self.centre.1 as i32 - 1) * CH) }
    fn period(&self) -> i32 { self.world.w as i32 * CH }
    /// The world cell of the land floor's cell (x, y).
    pub fn global(&self, x: i32, y: i32) -> (i32, i32) { let o = self.origin(); ((o.0 + x).rem_euclid(self.period().max(1)), o.1 + y) }
    /// The land floor's cell of a world cell, when it is on the floor now.
    pub fn local(&self, g: (i32, i32)) -> Option<(i32, i32)> {
        let o = self.origin();
        let x = (g.0 - o.0).rem_euclid(self.period().max(1));
        let y = g.1 - o.1;
        if x >= 3 * CH || y < 0 || y >= 3 * CH { None } else { Some((x, y)) }
    }
    pub fn tile_of_cell(&self, g: (i32, i32)) -> (usize, usize) {
        (g.0.div_euclid(CH).rem_euclid(self.world.w.max(1) as i32) as usize, g.1.div_euclid(CH).clamp(0, self.world.h as i32 - 1) as usize)
    }
    fn chunk_key(&self, tx: i64, ty: i64) -> Option<(u32, u32)> {
        if ty < 0 || ty >= self.world.h as i64 { None } else { Some((tx.rem_euclid(self.world.w as i64) as u32, ty as u32)) }
    }
    pub fn fresh_uid(&mut self) -> u32 { if self.next_uid < 1_000_000 { self.next_uid = 1_000_000; } self.next_uid += 1; self.next_uid }

    /// What the land of a tile is called ("the woods west of Greenburg").
    pub fn region_name(&self, t: (usize, usize)) -> String {
        let w = self.world.w;
        let k = t.1 * w + t.0;
        let kind = if self.world.land.get(k).copied().unwrap_or(false) { surface::kind_of(self.world.biome[k], self.world.elevation[k]) } else { surface::Kind::Sea };
        let near = self.sites.iter().filter(|s| s.kind == SiteKind::Town).map(|s| (super::world::dist(s.tile, t, w), s)).filter(|(d, _)| *d <= 8).min_by_key(|(d, s)| (*d, s.id));
        match near {
            Some((0, s)) => format!("the {} about {}", kind.word(), s.name),
            Some((_, s)) => format!("the {} {} of {}", kind.word(), super::quest::direction(s.tile, t, w), s.name),
            None => format!("the {}", kind.word()),
        }
    }

    /// What the land floor stands for at tile t: its town (its people talk as townsfolk), or the
    /// wild land with the tile's danger.
    pub fn land_spec(&self, t: (usize, usize)) -> SiteSpec {
        if let Some(s) = self.sites.iter().find(|s| s.tile == t && s.kind == SiteKind::Town) { return s.clone(); }
        let k = t.1 * self.world.w + t.0;
        let danger = self.world.danger.get(k).copied().unwrap_or(0) as u32;
        SiteSpec { id: LAND, kind: SiteKind::Wilds, name: self.region_name(t), tile: t, seed: 0, tier: (1 + danger * 3 / 255).clamp(1, 5), cause: String::new(), boss: None, treasures: Vec::new(),
            surface: self.world.ground.get(k).copied().unwrap_or(Ground::Grass), rock: "granite".into(), floors: 1, people: String::new(), god: String::new(), news: Vec::new(), lord: None, town: None, settlement: None, creature: None, notes: Vec::new() }
    }

    // -----------------------------------------------------------------------------------------
    // Chunks

    /// Make sure chunk `key` is made (and, the first time, its places, people, finds and
    /// creatures set).
    fn ensure_chunk(&mut self, key: (u32, u32)) {
        if self.pristine.contains_key(&key) && self.chunks.contains_key(&key) && self.tall.contains_key(&key) { return; }
        let sites: Vec<SiteSpec> = self.sites.iter().filter(|s| s.tile == (key.0 as usize, key.1 as usize) && s.kind != SiteKind::Wilds).cloned().collect();
        let refs: Vec<&SiteSpec> = sites.iter().collect();
        let gen = { let land = Land { info: &self.world, atlas: &self.atlas }; surface::generate(&land, key.0 as i64, key.1 as i64, &refs) };
        if !self.chunks.contains_key(&key) {
            for p in gen.places { self.places.entry(p.spec.id).or_insert(p); }
            let mut monsters = gen.monsters;
            for m in monsters.iter_mut() { m.uid = self.fresh_uid(); }
            let mut items: HashMap<(i32, i32), Vec<Item>> = HashMap::new();
            for (pos, it) in gen.items { stow(items.entry(pos).or_default(), it); }
            let mut ch = Chunk { diff: Vec::new(), items, monsters, npcs: gen.npcs, seen: vec![0; (CH * CH / 64) as usize], stocked: self.turn };
            let mut r = ChaCha8Rng::seed_from_u64(surface::hash(self.seed, key.0 as i64, key.1 as i64, 0x5704));
            self.stock(&mut ch, key, &gen.tiles, &gen.safe, &mut r);
            self.chunks.insert(key, ch);
        }
        if self.pristine.len() > 40 {
            let keep: Vec<(u32, u32)> = (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (dx, dy))).filter_map(|(dx, dy)| self.chunk_key(self.centre.0 as i64 + dx, self.centre.1 as i64 + dy)).collect();
            self.pristine.retain(|k, _| keep.contains(k));
            self.tall.retain(|k, _| keep.contains(k));
        }
        self.pristine.insert(key, gen.tiles);
        self.tall.insert(key, gen.tall);
    }

    /// Set the land's creatures in a chunk up to what its perils hold (by day).
    fn stock(&mut self, ch: &mut Chunk, key: (u32, u32), tiles: &[Tile], safe: &[bool], r: &mut ChaCha8Rng) {
        let k = key.1 as usize * self.world.w + key.0 as usize;
        if !self.world.land[k] { return; }
        let (table, bands, _) = { let land = Land { info: &self.world, atlas: &self.atlas }; surface::perils(&land, k, false, false) };
        if table.is_empty() { return; }
        let have = ch.monsters.iter().filter(|m| !m.boss && !m.night && m.hp > 0).count();
        let want = bands * 2;
        if have >= want { return; }
        // Not on top of the adventurer.
        let me = if self.on_land() { self.global(self.x, self.y) } else { (i32::MIN / 2, 0) };
        let (bx, by) = (key.0 as i32 * CH, key.1 as i32 * CH);
        let mut placed = have;
        for _ in 0..bands * 4 {
            if placed >= want { break; }
            let def = table[r.gen_range(0..table.len())];
            let (x, y) = (r.gen_range(2..CH - 2), r.gen_range(2..CH - 2));
            let i = (y * CH + x) as usize;
            if safe[i] || !tiles[i].walkable() || tiles[i].feature != Feature::None { continue; }
            if (bx + x - me.0).abs().max((by + y - me.1).abs()) < 20 { continue; }
            let pack = data().monster(def).map_or(1, |m| m.pack.max(1)) as i32;
            for p in 0..pack {
                let (px, py) = (x + p % 2, y + p / 2);
                if px >= CH || py >= CH || !tiles[(py * CH + px) as usize].walkable() || ch.monsters.iter().any(|m| (m.x, m.y) == (px, py)) { continue; }
                let uid = self.fresh_uid();
                ch.monsters.push(Monster::new(uid, def, px, py, 0));
                placed += 1;
            }
        }
        ch.stocked = self.turn;
    }

    /// Put the 3 x 3 chunks about `centre` together as the land floor.
    pub fn build_land(&mut self) {
        let (cx, cy) = self.centre;
        let keys: Vec<((i32, i32), Option<(u32, u32)>)> = (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (dx, dy))).map(|(dx, dy)| ((dx, dy), self.chunk_key(cx as i64 + dx as i64, cy as i64 + dy as i64))).collect();
        for (_, k) in &keys { if let Some(k) = k { self.ensure_chunk(*k); } }
        // Creatures come back to land long left (by day; the night's own are gone).
        let night = self.night();
        for (_, k) in &keys { if let Some(k) = *k {
            let mut ch = self.chunks.remove(&k).unwrap();
            if !night { ch.monsters.retain(|m| !m.night); }
            if self.turn > ch.stocked + 40_000 {
                let tiles = self.pristine[&k].clone();
                let mut r = self.roll_for(k.0 as u64 * 7919 + k.1 as u64);
                let safe = vec![false; tiles.len()];
                // (Not in towns: their streets are kept by the watch.)
                let town = self.sites.iter().any(|s| s.kind == SiteKind::Town && s.tile == (k.0 as usize, k.1 as usize));
                if !town { self.stock(&mut ch, k, &tiles, &safe, &mut r); } else { ch.stocked = self.turn; }
            }
            self.chunks.insert(k, ch);
        } }
        let n = 3 * CH as usize;
        let name = self.region_name(self.tile);
        let mut f = Floor::new(n, n, Tile::wall(Wall::Rock, Ground::Ice), &name, true);
        let mut monsters = Vec::new();
        let mut npcs = Vec::new();
        let mut far = Vec::new();
        for ((dx, dy), k) in keys {
            let (ox, oy) = ((dx + 1) * CH, (dy + 1) * CH);
            let Some(k) = k else { continue };
            let pr = &self.pristine[&k];
            let ch = &self.chunks[&k];
            for y in 0..CH { let row = (oy + y) as usize * n + ox as usize; f.tiles[row..row + CH as usize].clone_from_slice(&pr[(y * CH) as usize..((y + 1) * CH) as usize]); }
            for (i, t) in &ch.diff { let (x, y) = (*i as i32 % CH, *i as i32 / CH); f.tiles[(oy + y) as usize * n + (ox + x) as usize] = t.clone(); }
            for ((x, y), items) in &ch.items { f.items.insert((x + ox, y + oy), items.clone()); }
            for m in &ch.monsters { let mut m = m.clone(); m.x += ox; m.y += oy; m.home = (m.home.0 + ox, m.home.1 + oy); monsters.push(m); }
            for p in &ch.npcs { let mut p = p.clone(); p.x += ox; p.y += oy; p.post = (p.post.0 + ox, p.post.1 + oy); npcs.push(p); }
            for ((x, y), name) in self.tall.get(&k).into_iter().flatten() { far.push(((x + ox, y + oy), name.clone())); }
            for y in 0..CH { for x in 0..CH { let i = (y * CH + x) as usize; if ch.seen.get(i / 64).map_or(false, |w| w >> (i % 64) & 1 == 1) { f.seen[(oy + y) as usize * n + (ox + x) as usize] = true; } } }
        }
        self.far = far;
        let spec = self.land_spec(self.tile);
        let origin = self.origin();
        self.land = Some(Place { spec, floors: vec![f], monsters, npcs, entry: (CH + CH / 2, CH + CH / 2), next_uid: 0, top: 0, origin: Some(origin), mouth: None, rooms: Vec::new(), levers: Vec::new() });
    }

    /// Write the land floor back into its chunks (it stays as it is).
    pub fn store_land(&mut self) {
        let Some(p) = self.land.take() else { return };
        let f = &p.floors[0];
        let n = 3 * CH as usize;
        let (cx, cy) = self.centre;
        for dy in -1..=1i32 { for dx in -1..=1i32 {
            let Some(k) = self.chunk_key(cx as i64 + dx as i64, cy as i64 + dy as i64) else { continue };
            let (ox, oy) = ((dx + 1) * CH, (dy + 1) * CH);
            let Some(pr) = self.pristine.get(&k) else { continue };
            let mut ch = self.chunks.remove(&k).unwrap_or_default();
            ch.diff.clear();
            let mut seen = vec![0u64; (CH * CH / 64) as usize];
            for y in 0..CH { for x in 0..CH {
                let i = (y * CH + x) as usize;
                let t = &f.tiles[(oy + y) as usize * n + (ox + x) as usize];
                if *t != pr[i] { ch.diff.push((i as u16, t.clone())); }
                if f.seen[(oy + y) as usize * n + (ox + x) as usize] { seen[i / 64] |= 1 << (i % 64); }
            } }
            ch.seen = seen;
            ch.items = f.items.iter().filter(|((x, y), v)| !v.is_empty() && *x >= ox && *y >= oy && *x < ox + CH && *y < oy + CH).map(|((x, y), v)| ((x - ox, y - oy), v.clone())).collect();
            ch.monsters = p.monsters.iter().filter(|m| m.hp > 0 && m.x.div_euclid(CH) == dx + 1 && m.y.div_euclid(CH) == dy + 1).map(|m| { let mut m = m.clone(); m.x -= ox; m.y -= oy; m.home = (m.home.0 - ox, m.home.1 - oy); m }).collect();
            ch.npcs = p.npcs.iter().filter(|q| q.x.div_euclid(CH) == dx + 1 && q.y.div_euclid(CH) == dy + 1).map(|q| { let mut q = q.clone(); q.x -= ox; q.y -= oy; q.post = (q.post.0 - ox, q.post.1 - oy); q }).collect();
            self.chunks.insert(k, ch);
        } }
        self.land = Some(p);
    }

    /// Shift the land by a tile (the tile walked into becomes the middle).
    fn recentre(&mut self, sx: i32, sy: i32) {
        let sy = if (self.centre.1 as i32 + sy) < 0 || (self.centre.1 as i32 + sy) >= self.world.h as i32 { 0 } else { sy };
        if sx == 0 && sy == 0 { return; }
        self.store_land();
        self.centre = ((self.centre.0 as i32 + sx).rem_euclid(self.world.w as i32) as usize, (self.centre.1 as i32 + sy) as usize);
        let (dx, dy) = (-sx * CH, -sy * CH);
        self.x += dx;
        self.y += dy;
        if let Some(c) = self.companion.as_mut() { c.x += dx; c.y += dy; }
        if let Some(v) = self.corpses.get_mut(&LAND) { for c in v.iter_mut() { c.x += dx; c.y += dy; } v.retain(|c| c.x >= 0 && c.y >= 0 && c.x < 3 * CH && c.y < 3 * CH); }
        self.effects.clear();
        self.shifted = (self.shifted.0 + dx, self.shifted.1 + dy);
        self.build_land();
    }

    /// After a step on the land: a new middle when far into a side tile; a new tile's news.
    pub fn after_land_step(&mut self) {
        let (lo, hi) = (CH - 4, 2 * CH + 4);
        let sx = if self.x < lo { -1 } else if self.x >= hi { 1 } else { 0 };
        let sy = if self.y < lo { -1 } else if self.y >= hi { 1 } else { 0 };
        if sx != 0 || sy != 0 { self.recentre(sx, sy); }
        let t = self.tile_of_cell(self.global(self.x, self.y));
        if t != self.tile { self.enter_tile(t, true); }
    }

    /// The adventurer is on tile t now: its land, its places, its history; the map inks it.
    pub fn enter_tile(&mut self, t: (usize, usize), on_foot: bool) {
        let before = self.region_name(self.tile);
        self.tile = t;
        let spec = self.land_spec(t);
        let name = self.region_name(t);
        if let Some(p) = self.land.as_mut() { p.spec = spec; p.floors[0].name = name; }
        if self.route.last() != Some(&(t.0 as u16, t.1 as u16)) { self.route.push((t.0 as u16, t.1 as u16)); if self.route.len() > 6000 { self.route.drain(0..1000); } }
        let fresh = self.ink(t, on_foot);
        super::tales::on_tile(self);
        if let Some(town) = self.sites.iter().find(|s| s.tile == t && s.kind == SiteKind::Town).map(|s| s.id) { self.at_the_gate(town); }
        let now = self.region_name(t);
        if on_foot && now != before { self.say(Tone::Info, format!("You come into {}.", now)); }
        if on_foot && fresh > 0 { if let Some(tale) = self.world.tales.get(&(t.1 * self.world.w + t.0)).cloned() { self.say(Tone::Quest, format!("{} Old bones still come up in the grass.", tale)); } }
        let here: Vec<u32> = self.sites.iter().filter(|s| s.tile == t && s.kind != SiteKind::Wilds).map(|s| s.id).collect();
        for id in here {
            if self.known.contains(&id) || self.site(id).map_or(false, |s| s.kind == SiteKind::Cellar) { continue; }
            self.known.push(id);
            let s = self.site(id).unwrap();
            let line = format!("You come upon {}: {}.{}", s.name, s.kind.word(), if s.cause.is_empty() { String::new() } else { format!(" {}", s.cause) });
            self.say(Tone::Quest, line);
        }
    }

    /// Ink tile t on the map (and sketch what is about it; from the heights, farther). Tiles
    /// first inked on foot are the Mapmaker's: experience now, gold from a sage later.
    pub fn ink(&mut self, t: (usize, usize), on_foot: bool) -> u32 {
        let (w, h) = (self.world.w, self.world.h);
        if self.mapped.len() != w * h { self.mapped = vec![0; w * h]; }
        let high = self.world.elevation.get(t.1 * w + t.0).copied().unwrap_or(0.0) > 1800.0;
        let r = if high { 2 } else { 1 };
        let mut fresh = 0;
        for dy in -r..=r { for dx in -r..=r {
            let y = t.1 as i32 + dy;
            if y < 0 || y >= h as i32 { continue; }
            let k = y as usize * w + (t.0 as i32 + dx).rem_euclid(w as i32) as usize;
            let want = if (dx, dy) == (0, 0) || (high && dx.abs().max(dy.abs()) <= 1) { 2 } else { 1 };
            if self.mapped[k] < want { if want == 2 { fresh += 1; } self.mapped[k] = want; }
        } }
        if on_foot && fresh > 0 {
            self.charted += fresh;
            let xp = fresh as u64 * 4;
            if high && fresh > 1 { self.say(Tone::Info, format!("From the heights you see the land for miles, and put it on your map. ({} experience)", xp)); }
            for l in self.hero.gain_xp(xp) { self.say(Tone::Level, format!("You advanced from level {} to level {}.", l - 1, l)); }
        }
        fresh
    }

    /// Mark a tile as heard of on the map.
    pub fn rumour(&mut self, t: (usize, usize)) {
        let (w, h) = (self.world.w, self.world.h);
        if self.mapped.len() != w * h { self.mapped = vec![0; w * h]; }
        let k = t.1 * w + t.0;
        if k < self.mapped.len() && self.mapped[k] == 0 { self.mapped[k] = 1; }
    }

    // -----------------------------------------------------------------------------------------
    // Coming and going

    /// Where one comes onto tile t's land: outside a town's south gate, else where its roads meet.
    fn arrival_cell(&self, t: (usize, usize)) -> (i32, i32) {
        let base = (t.0 as i32 * CH, t.1 as i32 * CH);
        if let Some(s) = self.sites.iter().find(|s| s.tile == t && s.kind == SiteKind::Town) {
            let g = surface::gate_cell(s.town.as_ref().map_or(1, |q| q.size), 2);
            return (base.0 + g.0, base.1 + g.1 + 1);
        }
        let land = Land { info: &self.world, atlas: &self.atlas };
        let a = land.anchor(t.0 as i64, t.1 as i64);
        (a.0 as i32, a.1 as i32)
    }

    /// The open cell nearest (x, y) on the floor (no one standing there, nothing on it).
    pub fn open_near(&self, x: i32, y: i32) -> (i32, i32) {
        let Some(f) = self.floor() else { return (x, y) };
        let p = self.place();
        let taken = |a: i32, b: i32| p.map_or(false, |p| p.monsters.iter().any(|m| m.z == self.z && m.hp > 0 && (m.x, m.y) == (a, b)) || p.npcs.iter().any(|n| n.z == self.z && (n.x, n.y) == (a, b)));
        for r in 0..30i32 { for dy in -r..=r { for dx in -r..=r {
            if dx.abs().max(dy.abs()) != r { continue; }
            let (a, b) = (x + dx, y + dy);
            if f.walkable(a, b) && !taken(a, b) && matches!(f.at(a, b).feature, Feature::None) { return (a, b); }
        } } }
        (x, y)
    }

    /// Put the adventurer on the land at tile t (at world cell `at`, or where one arrives).
    pub fn land_at(&mut self, t: (usize, usize), at: Option<(i32, i32)>) {
        if self.on_land() { self.store_land(); }
        self.land = None;
        self.here = Some(LAND);
        self.z = 0;
        self.talk = None;
        self.centre = t;
        self.tile = t;
        self.build_land();
        let g = at.unwrap_or_else(|| self.arrival_cell(t));
        let (x, y) = self.local(g).unwrap_or((CH + CH / 2, CH + CH / 2));
        // On the way in or out itself when one comes out of it; else a clear cell.
        let on_way = at.is_some() && self.floor().map_or(false, |f| f.walkable(x, y));
        let (x, y) = if on_way { (x, y) } else { self.open_near(x, y) };
        self.x = x;
        self.y = y;
        self.enter_tile(t, false);
        self.companion_follow(true);
        self.look();
    }

    /// Go in by a way in the land: into place `site` at floor z (from world cell `at`).
    pub fn go_in(&mut self, site: u32, z: usize, at: (i32, i32)) {
        if !self.places.contains_key(&site) {
            let Some(spec) = self.site(site).cloned() else { return };
            self.places.insert(site, realize(&spec));
        }
        self.store_land();
        self.land = None;
        // Bring back what was slain long ago (Tibia's respawns; bosses stay dead).
        let turn = self.turn;
        let back: Vec<Monster> = self.respawn.iter().filter(|(p, m, t)| *p == site && !m.boss && turn > t + 12_000).map(|(_, m, _)| m.clone()).collect();
        self.respawn.retain(|(p, m, t)| !(*p == site && !m.boss && turn > t + 12_000));
        let p = self.places.get_mut(&site).unwrap();
        for mut m in back { m.hp = m.max_hp; m.x = m.home.0; m.y = m.home.1; m.awake = false; p.monsters.push(m); }
        if p.top == 0 { p.mouth = Some(at); }
        let z = z.min(p.floors.len() - 1).max(p.top);
        self.here = Some(site);
        self.z = z;
        self.talk = None;
        let (x, y) = match p.origin {
            Some(o) if z >= 1 => { let mut lx = (at.0 - o.0).rem_euclid(self.world.w as i32 * CH); if lx >= p.floors[z].w as i32 { lx -= self.world.w as i32 * CH; } (lx, at.1 - o.1) }
            _ => {
                // One step inside the way in.
                let f = &p.floors[z];
                let (ex, ey) = p.entry;
                [(0, -1), (-1, -1), (1, -1), (-1, 0), (1, 0), (0, 1)].iter().map(|(dx, dy)| (ex + dx, ey + dy)).find(|&(x, y)| f.walkable(x, y) && f.at(x, y).feature == Feature::None).unwrap_or((ex, ey))
            }
        };
        let (x, y) = if self.places[&site].floors[z].walkable(x, y) { (x, y) } else { self.places[&site].entry };
        self.x = x;
        self.y = y;
        if !self.known.contains(&site) { self.known.push(site); }
        self.stats.sites_entered += 1;
        let p = &self.places[&site];
        let (name, kind, cause, floor) = (p.spec.name.clone(), p.spec.kind, p.spec.cause.clone(), p.floors[z].name.clone());
        let verb = if kind == SiteKind::Town { "You lift the grate and climb down into the sewers of".to_string() } else if z == 0 { "You go into".into() } else { "You go down into".into() };
        self.say(Tone::Info, format!("{} {} ({}).{}", verb, name, floor, if cause.is_empty() || kind == SiteKind::Town { String::new() } else { format!(" {}", cause) }));
        if z > 0 { self.stats.floors_seen = self.stats.floors_seen.max(z as u32 + 1); }
        super::tales::on_enter(self, site);
        self.companion_follow(true);
        self.look();
    }

    /// Come up out of the place one is in, onto the land where its way in is.
    pub fn come_out(&mut self) {
        let Some(p) = self.place() else { return };
        let at = match (p.top, p.origin, p.mouth) {
            (top, Some(o), _) if top >= 1 => (o.0 + self.x, o.1 + self.y),
            (_, _, Some(m)) => m,
            _ => { let t = p.spec.tile; (t.0 as i32 * CH + CH / 2, t.1 as i32 * CH + CH / 2) }
        };
        let name = p.spec.name.clone();
        let t = self.tile_of_cell((at.0.rem_euclid(self.period().max(1)), at.1));
        self.say(Tone::Info, format!("You come up out of {} {}.", name, if self.night() { "into the night" } else { "into the daylight" }));
        self.land_at(t, Some((at.0.rem_euclid(self.period().max(1)), at.1)));
    }

    /// Leave the land for the world map (travel): not with enemies at one's heels.
    pub fn to_world_map(&mut self) -> bool {
        if !self.on_land() { self.say(Tone::Info, "Come out under the sky first."); return false; }
        // (Only what can walk to one within a few steps: a bandit across a river does not hold one.)
        let near = self.place().map_or(false, |p| {
            let f = &p.floors[0];
            let d = f.distances(self.x, self.y, 12, |x, y| f.at(x, y).walkable());
            p.monsters.iter().any(|m| m.hp > 0 && m.awake && !m.def().ai.contains("coward") && DIRS8.iter().any(|(dx, dy)| { let (x, y) = (m.x + dx, m.y + dy); f.inside(x, y) && d[y as usize * f.w + x as usize] <= 10 }))
        });
        if near { self.say(Tone::Danger, "Not with enemies at your heels."); return false; }
        self.store_land();
        self.land = None;
        self.here = None;
        self.talk = None;
        self.say(Tone::Info, "You take to the road. (On the world map arrows travel; Enter walks the land where you are.)");
        true
    }

    /// From the world map onto the land here, beside place `site` if one is named.
    pub fn land_here(&mut self, site: Option<u32>) -> bool {
        if self.here.is_some() { return false; }
        let t = self.tile;
        self.land_at(t, None);
        if let Some(id) = site {
            let spot = self.floor().and_then(|f| f.find(|x| matches!(x, Feature::Entrance { site, .. } if *site == id)));
            if let Some((ex, ey)) = spot {
                let (x, y) = self.open_near(ex, ey + 1);
                self.x = x; self.y = y;
                self.companion_follow(true);
                self.look();
            }
        }
        true
    }

    /// The adventurer set upon on the road: onto the land, enemies about them.
    pub fn ambush(&mut self, tier: u32) {
        let t = self.tile;
        self.land_at(t, None);
        let k = t.1 * self.world.w + t.0;
        let (table, _, _) = { let land = Land { info: &self.world, atlas: &self.atlas }; surface::perils(&land, k, self.night(), self.full_moon()) };
        let table: Vec<&str> = table.into_iter().filter(|d| data().monster(d).map_or(false, |m| m.tier <= tier && m.tier >= tier.saturating_sub(1).max(1) && !m.ai.contains("coward"))).collect();
        let table = if table.is_empty() { vec!["bandit"] } else { table };
        let mut r = self.roll_for(0xA3B);
        let n = r.gen_range(2..=3 + tier as usize / 2);
        for _ in 0..n {
            let def = table[r.gen_range(0..table.len())];
            let a = r.gen_range(0.0..std::f32::consts::TAU);
            let d = r.gen_range(4.0..7.0f32);
            let (x, y) = (self.x + (a.cos() * d) as i32, self.y + (a.sin() * d) as i32);
            let (x, y) = self.open_near(x, y);
            let uid = self.fresh_uid();
            let mut m = Monster::new(uid, def, x, y, 0);
            m.awake = true;
            if let Some(p) = self.land.as_mut() { p.monsters.push(m); }
        }
        self.look();
    }

    /// Set upon by `n` of `def` named `name` (landed among them, awake).
    pub fn ambush_of(&mut self, def: &str, n: usize, name: &str) {
        let t = self.tile;
        self.land_at(t, None);
        for k in 0..n {
            let (x, y) = self.open_near(self.x + 5 - 2 * k as i32, self.y - 4);
            let uid = self.fresh_uid();
            let mut m = Monster::new(uid, def, x, y, 0);
            m.name = name.to_string();
            m.awake = true;
            if let Some(p) = self.land.as_mut() { p.monsters.push(m); }
        }
        self.look();
    }

    /// The land's hours: dusk and dawn; the things of the night come out of the dark.
    pub fn land_tick(&mut self, before: u64) {
        if !self.on_land() { return; }
        let (was, now) = (night_at(before), self.night());
        if was != now {
            if now { self.say(Tone::Danger, if self.full_moon() { "The sun goes down and the moon comes up full. Something howls." } else { "The sun goes down. The land is dark, and things stir in it." }); }
            else {
                let gone = self.land.as_ref().map_or(0, |p| p.monsters.iter().filter(|m| m.night && m.hp > 0).count());
                if let Some(p) = self.land.as_mut() { p.monsters.retain(|m| !m.night); }
                self.say(Tone::Info, if gone > 0 { "Dawn. The things of the night sink back into the earth." } else { "Dawn comes up over the land." });
            }
        }
        if !now || before / 3000 == self.turn / 3000 { return; }
        // (The watch keeps a town's streets.)
        if self.place().map_or(false, |p| p.spec.kind == SiteKind::Town) { return; }
        let k = self.tile.1 * self.world.w + self.tile.0;
        let mut r = self.roll_for(0x41647);
        let info = &self.world;
        let p = 0.2 + info.danger[k] as f64 / 255.0 * 0.35 + if info.battles[k] > 0 { 0.3 } else { 0.0 } + info.shadow[k] as f64 * 0.4;
        if !r.gen_bool(p.min(0.9)) { return; }
        let full = self.full_moon();
        let kd = { let land = Land { info: &self.world, atlas: &self.atlas }; land.kind(k) };
        let cap = (1 + self.hero.level / 5).min(5);
        let mut habs = vec!["night".to_string(), format!("{}_night", kd.habitat())];
        if info.battles[k] > 0 { habs.push("battlefield".into()); }
        if full { habs.push("full_moon".into()); }
        let tier = ((1 + info.danger[k] as u32 * 3 / 255) + 1).min(cap).max(1);
        let table: Vec<&'static str> = habs.iter().flat_map(|h| data().living_in(h, tier)).map(|m| m.id.as_str()).collect();
        if table.is_empty() { return; }
        let def = table[r.gen_range(0..table.len())];
        let pack = data().monster(def).map_or(1, |m| m.pack.max(1)).min(3);
        // Out of the dark, out of sight.
        for _ in 0..20 {
            let a = r.gen_range(0.0..std::f32::consts::TAU);
            let d = r.gen_range(10.0..15.0f32);
            let (x, y) = (self.x + (a.cos() * d) as i32, self.y + (a.sin() * d) as i32);
            let ok = self.floor().map_or(false, |f| f.walkable(x, y)) && !self.visible(x, y);
            if !ok { continue; }
            for q in 0..pack {
                let (px, py) = self.open_near(x + q as i32 % 2, y + q as i32 / 2);
                let uid = self.fresh_uid();
                let mut m = Monster::new(uid, def, px, py, 0);
                m.night = true;
                m.awake = true;
                if let Some(p) = self.land.as_mut() { p.monsters.push(m); }
            }
            break;
        }
    }

    /// A roll seeded by the act and a salt.
    pub fn roll_for(&mut self, salt: u64) -> ChaCha8Rng { let s = self.rng.gen::<u64>() ^ self.turn.wrapping_mul(0x9E37_79B9) ^ salt; ChaCha8Rng::seed_from_u64(s) }

    // -----------------------------------------------------------------------------------------
    // Treasure maps

    /// A new treasure map's mark: a tile some way from here.
    pub fn map_target(&mut self) -> u32 {
        let mut b = Builder { rng: self.roll_for(0x3A9) };
        let land = Land { info: &self.world, atlas: &self.atlas };
        surface::treasure_target(&land, self.tile.0 as i64, self.tile.1 as i64, &mut b)
    }

    /// Read a treasure map: its mark goes on the world map.
    pub fn read_map(&mut self, k: usize) -> Option<i32> {
        let it = self.hero.pack.get(k)?.clone();
        let t = ((it.tag & 0xFFFF) as usize, (it.tag >> 16) as usize);
        if t.0 >= self.world.w || t.1 >= self.world.h { return None; }
        self.rumour(t);
        if !self.marks.contains(&it.tag) { self.marks.push(it.tag); }
        let d = super::world::dist(self.tile, t, self.world.w);
        let dir = super::quest::direction(self.tile, t, self.world.w);
        self.say(Tone::Quest, if d == 0 { "The map's cross is here, in this very land. Look for the spot and dig (a shovel).".to_string() } else { format!("A cross on the map, {} tiles to the {}, in {}. It is on your map now. (A shovel digs.)", d, dir, self.region_name(t)) });
        Some(60)
    }

    /// Dig with a shovel: a treasure map's cache, if one stands on its spot.
    pub fn dig(&mut self) -> Option<i32> {
        if !self.on_land() { self.say(Tone::Info, "The floor here is stone."); return None; }
        let me = self.global(self.x, self.y);
        let t = self.tile_of_cell(me);
        let cell = { let land = Land { info: &self.world, atlas: &self.atlas }; surface::cache_cell(&land, t) };
        let spot = (t.0 as i32 * CH + cell.0, t.1 as i32 * CH + cell.1);
        let tag = (t.1 as u32) << 16 | t.0 as u32;
        let has = self.hero.pack.iter().position(|i| i.id == "treasure_map" && i.tag == tag);
        let near = (me.0 - spot.0).abs().max((me.1 - spot.1).abs());
        match has {
            Some(k) if near <= 1 && !self.dug.contains(&tag) => {
                self.hero.pack.remove(k);
                self.dug.push(tag);
                self.marks.retain(|m| *m != tag);
                let tier = (1 + self.world.danger[t.1 * self.world.w + t.0] as u32 * 3 / 255).clamp(1, 5) + 1;
                let mut b = Builder { rng: self.roll_for(0xD16) };
                let mut loot = super::site::treasure(&mut b, tier);
                loot.push(super::site::gear(&mut b, tier));
                let list: Vec<String> = loot.iter().map(|i| i.describe()).collect();
                self.say(Tone::Loot, format!("The shovel strikes wood: a buried chest! {}.", list.join(", ")));
                for it in loot { if it.id == "gold" { self.stats.gold_found += it.count; } stow(&mut self.hero.pack, it); }
                self.stats.chests += 1;
                let turn = self.turn;
                let place = self.region_name(t);
                self.deeds.push((turn, format!("dug up a buried chest in {} by an old map", place)));
            }
            Some(_) if !self.dug.contains(&tag) => {
                let dir = { let (dx, dy) = (spot.0 - me.0, spot.1 - me.1); let ns = if dy < -2 { "north" } else if dy > 2 { "south" } else { "" }; let ew = if dx < -2 { "west" } else if dx > 2 { "east" } else { "" }; format!("{}{}{}", ns, if !ns.is_empty() && !ew.is_empty() { "-" } else { "" }, ew) };
                self.say(Tone::Info, format!("You dig, and find only earth. By the map, the cross lies some {} paces {}.", near, dir));
            }
            _ => self.say(Tone::Info, "You dig a hole, and find only earth and worms."),
        }
        Some(300)
    }

    /// Bring an old adventure (saved before the land was walkable) into the walkable land: the
    /// adventurer wakes in their temple; places will be made anew.
    pub fn into_the_land(&mut self) {
        self.places.clear();
        self.chunks.clear();
        self.respawn.clear();
        self.corpses.clear();
        self.seamless = true;
        self.start_map();
    }

    /// The map one begins with: home and its country inked, the towns one knows of sketched.
    pub fn start_map(&mut self) {
        let (w, h) = (self.world.w, self.world.h);
        self.mapped = vec![0; w * h];
        let home = self.site(self.hero.temple).map(|s| s.tile).unwrap_or(self.tile);
        for dy in -3..=3i32 { for dx in -3..=3i32 {
            let y = home.1 as i32 + dy;
            if y < 0 || y >= h as i32 { continue; }
            let k = y as usize * w + (home.0 as i32 + dx).rem_euclid(w as i32) as usize;
            self.mapped[k] = if dx.abs().max(dy.abs()) <= 2 { 2 } else { 1 };
        } }
        let known: Vec<(usize, usize)> = self.known.iter().filter_map(|id| self.site(*id)).map(|s| s.tile).collect();
        for t in known { self.rumour(t); }
    }

    /// The people of town `town` wherever they are kept (on the land floor, in its chunks).
    pub fn town_npcs_mut(&mut self, town: u32) -> Vec<&mut Npc> {
        let mut v: Vec<&mut Npc> = Vec::new();
        if let Some(p) = self.land.as_mut() { v.extend(p.npcs.iter_mut().filter(|n| n.home == town)); }
        let on_land: Vec<String> = v.iter().map(|n| n.name.clone()).collect();
        for ch in self.chunks.values_mut() { v.extend(ch.npcs.iter_mut().filter(|n| n.home == town && !on_land.contains(&n.name))); }
        v
    }

    /// Someone leaves town `town` for good (the land floor and its chunks).
    pub fn remove_npc(&mut self, town: u32, name: &str, line: &str) {
        if let Some(p) = self.land.as_mut() { p.npcs.retain(|n| !(n.home == town && n.name == name)); }
        for ch in self.chunks.values_mut() { ch.npcs.retain(|n| !(n.home == town && n.name == name)); }
        self.talk = None;
        self.say(Tone::Quest, line.to_string());
    }

    /// Someone comes to live in town `town` (by its square).
    pub fn add_townsperson(&mut self, town: u32, name: &str) {
        let Some(s) = self.site(town).cloned() else { return };
        let key = (s.tile.0 as u32, s.tile.1 as u32);
        let n = Npc { name: name.into(), role: super::actor::Role::Townsfolk, x: CH / 2 + 2, y: CH / 2 + 3, z: 0, post: (CH / 2 + 2, CH / 2 + 3), race: s.people.clone(), female: name.len() % 2 == 0, of: s.people.clone(), home: town, met: super::people::Met { times: 1, last: self.turn, helped: vec!["coming home".into()], wronged: 0 } };
        let on = self.on_land() && self.chunk_key(s.tile.0 as i64, s.tile.1 as i64).map_or(false, |k| { let (cx, cy) = self.centre; ((k.0 as i32 - cx as i32).rem_euclid(self.world.w as i32) <= 1 || (cx as i32 - k.0 as i32).rem_euclid(self.world.w as i32) <= 1) && (k.1 as i32 - cy as i32).abs() <= 1 });
        if on {
            let o = self.origin();
            if let Some((x, y)) = self.local((key.0 as i32 * CH + n.x, key.1 as i32 * CH + n.y)) {
                let (x, y) = self.open_near(x, y);
                let mut n = n; n.x = x; n.y = y; n.post = (x, y);
                if let Some(p) = self.land.as_mut() { p.npcs.push(n); }
            }
            let _ = o;
        } else if let Some(ch) = self.chunks.get_mut(&key) { ch.npcs.push(n); }
    }

    /// The land's own (the atlas) from the sites; called once the world is set.
    pub fn set_atlas(&mut self) { self.atlas = Atlas::new(&self.world, &self.sites, self.seed); }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::adventure::game::Action;
    use crate::adventure::site::TownShape;
    use crate::biomes::ExtendedBiome;

    fn world() -> (crate::adventure::world::WorldInfo, Vec<SiteSpec>) {
        let (w, h) = (10usize, 6usize);
        let mut i = crate::adventure::world::WorldInfo::default();
        i.w = w; i.h = h;
        i.land = (0..w * h).map(|k| { let y = k / w; y > 0 && y < h - 1 }).collect();
        i.ground = vec![Ground::Grass; w * h];
        i.danger = (0..w * h).map(|k| ((k % w) * 12) as u8).collect();
        i.elevation = (0..w * h).map(|k| if i.land[k] { 300.0 } else { -500.0 }).collect();
        i.biome = (0..w * h).map(|k| if !i.land[k] { ExtendedBiome::Ocean } else if k % 4 == 0 { ExtendedBiome::TemperateForest } else { ExtendedBiome::TemperateGrassland }).collect();
        i.temperature = vec![14.0; w * h];
        i.moisture = vec![0.5; w * h];
        i.river = vec![false; w * h];
        i.downhill = vec![255; w * h];
        i.road = (0..w * h).map(|k| k / w == 2 && i.land[k]).collect();
        i.forest = vec![0; w * h];
        i.farmland = vec![0; w * h];
        i.shadow = vec![0.0; w * h];
        i.battles = vec![0; w * h];
        let town = SiteSpec { id: 1, kind: SiteKind::Town, name: "Greenburg".into(), tile: (3, 2), seed: 7, tier: 1, cause: String::new(), boss: None, treasures: vec![],
            surface: Ground::Grass, rock: "granite".into(), floors: 3, people: "human".into(), god: "Balorn".into(), news: Vec::new(), lord: None,
            town: Some(TownShape { size: 1, walls: 1, arch: "wood".into(), population: 500, port: false, roads: 0b0100_0100, sea: 0, razed: None }), settlement: None, creature: None, notes: Vec::new() };
        let cave = SiteSpec { id: 2, kind: SiteKind::Cave, name: "the Bat Hole".into(), tile: (6, 3), seed: 11, tier: 1, cause: String::new(), boss: None, treasures: vec![],
            surface: Ground::Grass, rock: "granite".into(), floors: 2, people: String::new(), god: String::new(), news: Vec::new(), lord: None, town: None, settlement: None, creature: None, notes: Vec::new() };
        (i, vec![town, cave])
    }

    /// A castle's towers stand over the land: seen and named from 60 cells off by day (not past
    /// 64), its cells inked on the floor.
    #[test]
    fn a_tower_is_seen_from_far_off() {
        let (info, mut sites) = world();
        let mut castle = sites[1].clone();
        castle.id = 3; castle.kind = SiteKind::Castle; castle.name = "Highkeep".into(); castle.tile = (5, 2); castle.seed = 13; castle.tier = 2;
        sites.push(castle);
        let mut g = Game::new(info, sites, crate::adventure::hero::Hero::new("Tess", "human", true, 5), 1, 5);
        g.land_at((4, 2), None);
        let (tx, ty) = g.far.iter().find(|f| f.1.contains("Highkeep")).expect("the castle stands tall in the land").0;
        g.x = tx - 60; g.y = ty;
        g.look();
        assert!(g.far_seen.iter().any(|s| s.contains("the towers of Highkeep to the east")), "not seen from 60 cells: {:?}", g.far_seen);
        let f = &g.land.as_ref().unwrap().floors[0];
        assert!(f.seen[ty as usize * f.w + tx as usize], "the towers are not on the map");
        g.x = tx - 70;
        g.look();
        assert!(!g.far_seen.iter().any(|s| s.contains("Highkeep")), "seen from too far: {:?}", g.far_seen);
    }

    /// A war told on four standing stones: each read is remembered, the last tells the whole tale
    /// (experience, a deed).
    #[test]
    fn four_stones_tell_a_tale() {
        use crate::adventure::wonders::Stone;
        let (mut info, sites) = world();
        for (p, t) in [(0u8, (2usize, 2usize)), (1, (3, 1)), (2, (4, 2)), (3, (4, 3))] { info.stones.insert(t.1 * info.w + t.0, Stone { story: 0, part: p, text: format!("The tale of the War of Tests, part {}.", p + 1) }); }
        info.stories = vec!["the War of Tests".into()];
        let mut g = Game::new(info, sites, crate::adventure::hero::Hero::new("Tess", "human", true, 5), 1, 5);
        g.land_at((3, 2), None);
        let xp = g.hero.xp;
        let f = g.land.as_ref().unwrap().floors[0].clone();
        let stones: Vec<(i32, i32, String)> = (0..f.h as i32).flat_map(|y| (0..f.w as i32).map(move |x| (x, y))).filter_map(|(x, y)| if let Feature::Lore { text, look: 3 } = &f.at(x, y).feature { Some((x, y, text.clone())) } else { None }).collect();
        assert_eq!(stones.len(), 4, "four stones stand in the land");
        for (x, y, t) in &stones { g.read_stone(*x, *y, t); }
        assert_eq!(g.stones_read.len(), 4);
        assert_eq!(g.stats.stories, 1, "the whole tale was not told");
        assert!(g.hero.xp > xp && g.deeds.iter().any(|d| d.1.contains("whole tale of the War of Tests")));
    }

    pub(crate) fn game() -> Game {
        let (info, sites) = world();
        let hero = crate::adventure::hero::Hero::new("Tess", "human", true, 5);
        Game::new(info, sites, hero, 1, 5)
    }

    /// Out of the town's gate and east over three tiles' borders: every step moves one cell in
    /// the world (no jump, no menu), the land floor is put together anew about each tile walked
    /// into, and what stands under the adventurer is the land as made there.
    #[test]
    fn walking_crosses_tiles_without_a_seam() {
        let mut g = game();
        g.land_at((3, 2), None);
        assert!(g.on_land());
        let mut last = g.global(g.x, g.y);
        let (mut tiles, mut centres) = (vec![g.tile], vec![g.centre]);
        let period = g.world.w as i32 * CH;
        for _ in 0..330 {
            // (A walk about seams, not a fight: the walker is kept whole.)
            g.hero.hp = g.hero.max_hp();
            let before = (g.x, g.y, g.here);
            for (dx, dy) in [(1, 0), (1, 1), (1, -1), (0, 1), (0, -1)] { g.act(Action::Move(dx, dy)); if (g.x, g.y, g.here) != before { break; } }
            if g.banner.is_some() || g.here != Some(LAND) { g.banner = None; continue; }
            let now = g.global(g.x, g.y);
            let dx = (now.0 - last.0).rem_euclid(period).min((last.0 - now.0).rem_euclid(period));
            assert!(dx <= 1 && (now.1 - last.1).abs() <= 1, "a jump from {:?} to {:?}", last, now);
            last = now;
            if tiles.last() != Some(&g.tile) { tiles.push(g.tile); }
            if centres.last() != Some(&g.centre) { centres.push(g.centre); }
        }
        assert!(tiles.len() >= 3, "walked over too few tiles: {:?}", tiles);
        assert!(centres.len() >= 3, "the land was not put together anew: {:?}", centres);
        // The cell underfoot is the land as made at that world cell (nothing changed it).
        let (gx, gy) = g.global(g.x, g.y);
        let key = ((gx.div_euclid(CH)) as u32, (gy.div_euclid(CH)) as u32);
        let made = &g.pristine[&key];
        let here = g.floor().unwrap().at(g.x, g.y).clone();
        assert_eq!(here, made[(gy.rem_euclid(CH) * CH + gx.rem_euclid(CH)) as usize]);
    }

    /// Into a cave by its mouth in the land, and out again where one went in.
    #[test]
    fn a_cave_mouth_leads_in_and_out() {
        let mut g = game();
        g.land_at((6, 3), None);
        let f = g.floor().unwrap();
        let (ex, ey) = f.find(|x| matches!(x, Feature::Entrance { site: 2, .. })).expect("the cave's mouth stands in its tile");
        let mouth = g.global(ex, ey);
        let (sx, sy) = g.open_near(ex, ey + 1);
        g.x = sx; g.y = sy;
        // Step onto the mouth.
        let (dx, dy) = ((ex - g.x).signum(), (ey - g.y).signum());
        let mut tries = 0;
        while g.here == Some(LAND) && tries < 20 {
            let (dx, dy) = if (g.x + dx, g.y + dy) == (ex, ey) || tries > 0 { ((ex - g.x).signum(), (ey - g.y).signum()) } else { (dx, dy) };
            g.act(Action::Move(dx, dy));
            tries += 1;
        }
        assert_eq!(g.here, Some(2), "went into the cave");
        // Back to its way out and up.
        let exit = g.floor().unwrap().find(|x| matches!(x, Feature::Exit)).unwrap();
        g.x = exit.0; g.y = exit.1;
        g.act(Action::Climb);
        assert!(g.on_land(), "came out onto the land");
        assert_eq!(g.global(g.x, g.y), mouth, "came out of the mouth one went into");
    }

    /// Two saves of the land: changes made to a chunk (a thing dropped) are there when one comes
    /// back to it, after walking away and the land being put together elsewhere.
    #[test]
    fn the_land_remembers() {
        let mut g = game();
        g.land_at((5, 2), None);
        let at = g.global(g.x, g.y);
        g.hero.pack.push(crate::adventure::item::Item::new("rope", 1));
        let k = g.hero.pack.len() - 1;
        g.act(Action::Drop(k));
        g.to_world_map();
        for _ in 0..3 { g.act(Action::Travel(1, 0)); if g.here.is_some() { g.to_world_map(); } }
        g.tile = (5, 2);
        g.land_at((5, 2), Some(at));
        let f = g.floor().unwrap();
        assert!(f.items.get(&(g.x, g.y)).map_or(false, |v| v.iter().any(|i| i.id == "rope")), "the rope lies where it was dropped");
    }

    /// A townsperson remembers: met once, they greet one as known; done a deed for, they name it.
    #[test]
    fn townsfolk_remember_the_hero() {
        let mut g = game();
        g.land_at((3, 2), None);
        let k = g.place().unwrap().npcs.iter().position(|n| n.role == crate::adventure::actor::Role::Lord).expect("a lord");
        crate::adventure::npc::greet(&mut g, k);
        g.talk = None;
        assert_eq!(g.place().unwrap().npcs[k].met.times, 1);
        g.place_mut().unwrap().npcs[k].met.helped.push("Slay the beast of the Bat Pit".into());
        crate::adventure::npc::greet(&mut g, k);
        let said = g.talk.as_ref().unwrap().said.clone();
        assert!(said.contains("Tess") && said.to_lowercase().contains("beast of the bat pit"), "{}", said);
        // And it is kept with the land when one walks away and back.
        g.talk = None;
        g.store_land();
        let name = g.place().unwrap().npcs[k].name.clone();
        g.to_world_map();
        g.land_at((3, 2), None);
        let n = g.place().unwrap().npcs.iter().find(|n| n.name == name).unwrap();
        assert_eq!(n.met.times, 2);
    }

    /// A feud's tale: siding with the one who asked sends the other house out of town; making
    /// peace keeps both, and the town remembers it in its news.
    #[test]
    fn a_feud_changes_the_town() {
        use crate::adventure::actor::Role;
        for (pick, leaves) in [(1u8, true), (3u8, false)] {
            let mut g = game();
            g.land_at((3, 2), None);
            g.hero.pack.push(crate::adventure::item::Item::new("gold", 100));
            let folk: Vec<String> = g.place().unwrap().npcs.iter().filter(|n| n.home == 1 && n.role == Role::Townsfolk && n.of != "drunk").map(|n| n.name.clone()).collect();
            let q = crate::adventure::tales::offer(&g, 1, &folk[1], Role::Townsfolk).expect("a feud");
            let other = match &q.goal { crate::adventure::quest::Goal::Tale(t) => t.other.clone(), _ => unreachable!() };
            g.quests.push(q);
            let k = g.place().unwrap().npcs.iter().position(|n| n.name == other).unwrap();
            crate::adventure::npc::greet(&mut g, k);
            let c = g.choice.clone().expect("the other house hears it out");
            let i = c.options.iter().position(|o| o.1 == pick).expect("that way");
            g.act(crate::adventure::game::Action::Decide(i));
            let gone = !g.place().unwrap().npcs.iter().any(|n| n.name == other);
            assert_eq!(gone, leaves, "pick {}: {} gone {}", pick, other, gone);
            assert!(!g.site(1).unwrap().notes.is_empty(), "the town remembers");
        }
    }

    /// Regard: striking and killing the town's guard raises its prices, bars its people's talk and
    /// brings the watch to the gate; a beast slain near a town makes every townsperson greet the
    /// hero by the deed.
    #[test]
    fn the_town_remembers_blood_and_deeds() {
        use crate::adventure::actor::Role;
        let mut g = game();
        g.land_at((3, 2), None);
        let price = |g: &Game| crate::adventure::regard::price_factor(g.regard_of(1));
        let before = price(&g);
        let k = g.place().unwrap().npcs.iter().position(|n| n.role == Role::Guard && n.home == 1).expect("a guard");
        g.assault(k);
        let uid = g.place().unwrap().monsters.iter().find(|m| m.town == 1 && m.def == "watchman" && m.hp > 0).map(|m| m.uid).unwrap();
        let i = g.place().unwrap().monsters.iter().position(|m| m.uid == uid).unwrap();
        g.place_mut().unwrap().monsters[i].hp = 1;
        g.place_mut().unwrap().monsters[i].x = g.x + 1; g.place_mut().unwrap().monsters[i].y = g.y;
        for _ in 0..20 { if !g.place().unwrap().monsters.iter().any(|m| m.uid == uid) { break; } g.act(Action::Attack(uid)); }
        assert!(g.regard_of(1) <= -60, "regard {}", g.regard_of(1));
        assert!(price(&g) > before, "prices rose: {} -> {}", before, price(&g));
        // Talk is barred; the watch meets them at the gate when they come back.
        let k = g.place().unwrap().npcs.iter().position(|n| n.role == Role::Trader && n.home == 1).unwrap();
        crate::adventure::npc::greet(&mut g, k);
        assert!(g.talk.as_ref().unwrap().options.len() == 1, "barred");
        g.talk = None;
        g.place_mut().unwrap().monsters.retain(|m| m.town != 1);
        g.to_world_map();
        g.land_at((3, 2), None);
        assert!(g.place().unwrap().monsters.iter().filter(|m| m.town == 1).count() >= 2, "the watch at the gate");
        // Another town's people are grateful for a beast slain near them.
        let mut g = game();
        g.land_at((3, 2), None);
        g.beast_slain((4, 2), "Gnash the Old");
        let k = g.place().unwrap().npcs.iter().position(|n| n.role == Role::Priest && n.home == 1).unwrap();
        crate::adventure::npc::greet(&mut g, k);
        let said = g.talk.as_ref().unwrap().said.to_lowercase();
        assert!(said.contains("tess") && said.contains("gnash"), "{}", said);
    }
}
