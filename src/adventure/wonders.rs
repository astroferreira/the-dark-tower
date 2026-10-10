//! Exploration that pays (card adv-landmarks): the world's wonders as places to walk to, each
//! with one thing found nowhere else; standing stones that carry the history's wars in four
//! parts; tall things seen from far off.
//!
//! Wonders come from the world's focal biomes (`lore::focal`: ancient groves, oases, crater
//! lakes, volcanoes, hot springs, giants' bones, cyclopean ruins...) and the gazetteer's named
//! peaks, one a patch. Each is stamped in its tile (`surface::finds`) as `Feature::Wonder` with
//! its kind, and bumping it gives its gift (`Game::wonder`): the eldest tree's blessing (+10
//! life for good), a spring that mends all, a summit's view (the map inked for miles about), a
//! star-stone's iron, a fire mountain's guardian and its glass, a giant's bone and the wight that
//! keeps it, a monolith telling the oldest story, a wishing well.
//!
//! Stones: each war of the history with four located fights is told on four standing stones
//! (`Feature::Lore` look 3) near where they were fought; reading all four tells the whole tale
//! (experience and a deed).

use std::collections::BTreeMap;

use super::game::{Game, Tone};
use super::item::{stow, Item};
use super::map::Feature;
use crate::biomes::ExtendedBiome;
use crate::history::world_state::WorldHistory;
use crate::world::WorldData;

/// The kinds of wonder (`Feature::Wonder::kind`).
pub const TREE: u8 = 0;
pub const SPRING: u8 = 1;
pub const SUMMIT: u8 = 2;
pub const STAR: u8 = 3;
pub const FIRE: u8 = 4;
pub const BONES: u8 = 5;
pub const MONOLITH: u8 = 6;
pub const WELL: u8 = 7;

/// A wonder at a tile: its kind and name.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Wonder { pub kind: u8, pub name: String }

/// A standing stone at a tile: part `part` (0-3) of story `story`.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Stone { pub story: u32, pub part: u8, pub text: String }

fn kind_of(b: ExtendedBiome) -> Option<u8> {
    use ExtendedBiome::*;
    Some(match b {
        AncientGrove | OvergrownCitadel => if b == AncientGrove { TREE } else { MONOLITH },
        Oasis | HotSprings | Geysers => SPRING,
        CraterLake | StarfallCrater => STAR,
        VolcanicCone | Caldera | ShieldVolcano => FIRE,
        TitanBones => BONES,
        CyclopeanRuins => MONOLITH,
        Cenote | Sinkhole => WELL,
        TowerKarst => SUMMIT,
        _ => return None,
    })
}

fn named(kind: u8, region: &str, b: ExtendedBiome) -> String {
    let of = if region.is_empty() { String::new() } else { format!(" of {}", region) };
    match kind {
        TREE => format!("the Eldest Tree{}", of),
        SPRING => if b == ExtendedBiome::Oasis { format!("the oasis{}", of) } else { format!("the hot springs{}", of) },
        SUMMIT => format!("the stone towers{}", of),
        STAR => format!("the star-crater{}", of),
        FIRE => format!("the fire mountain{}", of),
        BONES => format!("the giant's bones{}", of),
        MONOLITH => format!("the old monolith{}", of),
        _ => format!("the deep well{}", of),
    }
}

/// The world's wonders, by tile, and its four-part stories (stones by tile, and the tales'
/// names).
pub fn find(world: &WorldData, history: Option<&WorldHistory>, seed: u64, land: &[bool]) -> (BTreeMap<usize, Wonder>, BTreeMap<usize, Stone>, Vec<String>) {
    let (w, h) = (world.width, world.height);
    let gaz = crate::lore::build_gazetteer(world, history, seed);
    let region = |x: usize, y: usize| -> String {
        let id = *gaz.region.get(x, y);
        gaz.feature(id).map(|f| f.name.clone()).or_else(|| gaz.feature(*gaz.landmass.get(x, y)).map(|f| f.name.clone())).unwrap_or_default()
    };
    let mut wonders: BTreeMap<usize, Wonder> = BTreeMap::new();
    let near = |ws: &BTreeMap<usize, Wonder>, x: usize, y: usize, r: i32| ws.keys().any(|&k| super::world::dist((k % w, k / w), (x, y), w) <= r);
    for y in 0..h { for x in 0..w {
        let b = *world.biomes.get(x, y);
        let Some(kind) = kind_of(b) else { continue };
        if !land[y * w + x] || near(&wonders, x, y, 3) { continue; }
        wonders.insert(y * w + x, Wonder { kind, name: named(kind, &region(x, y), b) });
    } }
    // The named peaks: a cairn on the summit.
    let mut peaks: Vec<&crate::lore::gazetteer::Feature> = gaz.features.iter().filter(|f| f.kind == crate::lore::gazetteer::FeatureKind::Peak).collect();
    peaks.sort_by(|a, b| b.height_m.partial_cmp(&a.height_m).unwrap_or(std::cmp::Ordering::Equal).then(a.id.cmp(&b.id)));
    for p in peaks {
        let (x, y) = p.anchor;
        if x >= w || y >= h || !land[y * w + x] || near(&wonders, x, y, 2) { continue; }
        wonders.insert(y * w + x, Wonder { kind: SUMMIT, name: format!("the summit of {}", p.name) });
    }
    // Stories: each war with four fights where they were fought.
    let mut stones: BTreeMap<usize, Stone> = BTreeMap::new();
    let mut names: Vec<String> = Vec::new();
    if let Some(hist) = history {
        let mut wars: Vec<&crate::history::civilizations::military::War> = hist.wars.values().collect();
        wars.sort_by_key(|wr| wr.id.0);
        for war in wars {
            let evs: Vec<&crate::history::events::types::Event> = crate::history::collections::war_events(hist, war).into_iter().filter_map(|id| hist.chronicle.get(id)).filter(|e| e.location.map_or(false, |(x, y)| x < w && y < h)).collect();
            if evs.len() < 4 { continue; }
            // The first, two between, the last.
            let pick = [0, evs.len() / 3, evs.len() * 2 / 3, evs.len() - 1];
            let story = names.len() as u32;
            let mut placed = 0;
            for (part, &i) in pick.iter().enumerate() {
                let e = evs[i];
                let (ex, ey) = e.location.unwrap();
                // The nearest land tile with no stone (spiralling out).
                let spot = (0..6i32).flat_map(|r| (-r..=r).flat_map(move |dy| (-r..=r).map(move |dx| (dx, dy)))).map(|(dx, dy)| ((ex as i32 + dx).rem_euclid(w as i32) as usize, (ey as i32 + dy).clamp(0, h as i32 - 1) as usize)).find(|&(x, y)| land[y * w + x] && !stones.contains_key(&(y * w + x)));
                let Some((x, y)) = spot else { continue };
                let head = ["It began so", "Then", "And after", "At the last"][part];
                let text = format!("A standing stone, carved: the tale of {}, the {} of four. {}: {} ({}).", war.name, ["first", "second", "third", "fourth"][part], head, e.title.trim_end_matches('.'), e.date.year);
                stones.insert(y * w + x, Stone { story, part: part as u8, text });
                placed += 1;
            }
            if placed == 4 { names.push(war.name.clone()); } else {
                stones.retain(|_, s| s.story != story);
            }
            if names.len() >= 40 { break; }
        }
    }
    (wonders, stones, names)
}

impl Game {
    /// Bump a wonder at (x, y): its gift.
    pub fn wonder(&mut self, x: i32, y: i32, kind: u8, used: bool) -> Option<i32> {
        let name = self.wonder_name(x, y);
        let set_used = |g: &mut Game| { let z = g.z; if let Some(p) = g.place_mut() { if let Feature::Wonder { used, .. } = &mut p.floors[z].at_mut(x, y).feature { *used = true; } } };
        match kind {
            TREE => {
                if used || self.hero.boons.iter().any(|b| b == "grove") { self.say(Tone::Info, format!("{}: the old tree stands silent. You have had its blessing.", cap(&name))); return None; }
                self.hero.boons.push("grove".into());
                self.hero.hp = self.hero.max_hp();
                set_used(self);
                self.say(Tone::Level, format!("Under {} a voice like wind in leaves: \"Few walk this far, little one. Be stronger.\" (+10 life for good)", name));
                self.wonder_deed(&name);
                Some(300)
            }
            SPRING => {
                self.hero.hp = self.hero.max_hp();
                self.hero.mana = self.hero.max_mana();
                self.hero.poisoned = 0;
                self.hero.wounds.clear();
                self.hero.fed = self.hero.fed.max(3000);
                self.say(Tone::Level, format!("You bathe in {}. The water is warm and every hurt goes out of you.", name));
                if !used { set_used(self); self.wonder_deed(&name); }
                Some(400)
            }
            SUMMIT => {
                if used { self.say(Tone::Info, format!("The cairn on {}. The whole country lies below.", name)); return None; }
                set_used(self);
                let (tx, ty) = (self.tile.0 as i32, self.tile.1 as i32);
                let (w, h) = (self.world.w as i32, self.world.h as i32);
                let mut n = 0;
                for dy in -7..=7 { for dx in -7..=7 {
                    if dx * dx + dy * dy > 49 { continue; }
                    let (x, y) = ((tx + dx).rem_euclid(w), ty + dy);
                    if y < 0 || y >= h { continue; }
                    let k = y as usize * w as usize + x as usize;
                    if self.mapped.get(k).copied().unwrap_or(2) < 2 { self.mapped[k] = 2; n += 1; }
                } }
                let xp = 60 + 20 * n as u64;
                self.wonder_xp(xp);
                self.say(Tone::Level, format!("From {} you see the world laid out like a map, and you ink {} lands of it. ({} experience)", name, n, xp));
                self.wonder_deed(&name);
                Some(300)
            }
            STAR => {
                if used { self.say(Tone::Info, format!("The star-stone of {}, its heart cut out.", name)); return None; }
                set_used(self);
                stow(&mut self.hero.pack, Item::new("star_iron", 1));
                self.say(Tone::Loot, format!("In {} a black stone fell from the sky, still warm after all the years. You break a lump of star-iron from it.", name));
                self.wonder_deed(&name);
                Some(300)
            }
            FIRE => {
                if used { self.say(Tone::Info, format!("The vent of {} smokes on.", name)); return None; }
                set_used(self);
                self.say(Tone::Danger, format!("The vent of {} roars, and something of fire climbs out of it!", name));
                let (x0, y0, z) = (self.x, self.y, self.z);
                let spot = self.open_near(x0 + 2, y0);
                let uid = self.fresh_uid();
                let mut m = super::actor::Monster::new(uid, "salamander", spot.0, spot.1, z);
                m.awake = true;
                if let Some(p) = self.place_mut() { p.monsters.push(m); }
                stow(&mut self.hero.pack, Item::new("volcanic_glass", 1));
                self.wonder_deed(&name);
                Some(200)
            }
            BONES => {
                if used { self.say(Tone::Info, format!("The bones of {}, picked clean.", name)); return None; }
                set_used(self);
                stow(&mut self.hero.pack, Item::new("giant_bone", 1));
                self.say(Tone::Danger, format!("You prise a bone from {}. The ground stirs: its keeper will not have it!", name));
                let (x0, y0, z) = (self.x, self.y, self.z);
                let spot = self.open_near(x0 - 2, y0 + 1);
                let uid = self.fresh_uid();
                let mut m = super::actor::Monster::new(uid, "wight", spot.0, spot.1, z);
                m.awake = true;
                if let Some(p) = self.place_mut() { p.monsters.push(m); }
                self.wonder_deed(&name);
                Some(200)
            }
            MONOLITH => {
                let oldest = self.history.as_ref().and_then(|h| h.chronicle.events.iter().filter(|e| e.location.is_some()).min_by_key(|e| (e.date.year, e.id.0)).map(|e| format!("{} ({})", e.title.trim_end_matches('.'), e.date.year)));
                let text = oldest.unwrap_or_else(|| "a tongue no one speaks now".into());
                self.say(Tone::Quest, format!("{} is carved from end to end in the oldest letters. What you can read tells of the first deed remembered: {}.", cap(&name), text));
                if !used { set_used(self); self.wonder_xp(150); self.say(Tone::Level, "To read the oldest story is worth something. (150 experience)"); self.wonder_deed(&name); }
                Some(300)
            }
            _ => {
                if used { self.say(Tone::Info, format!("{}: the coin you threw is long gone.", cap(&name))); return None; }
                if self.hero.gold() < 1 { self.say(Tone::Info, "You have no coin to throw."); return None; }
                self.hero.spend("gold", 1);
                set_used(self);
                let r = super::surface::hash(self.seed, x as i64, y as i64, self.turn / 100) % 3;
                match r {
                    0 => { stow(&mut self.hero.pack, Item::new("gold", 10 + self.hero.level * 5)); self.say(Tone::Loot, "You throw a coin into the deep well. A moment later, coins come up with the water: the well pays back."); }
                    1 => { stow(&mut self.hero.pack, Item::new("strong_health_potion", 1)); self.say(Tone::Loot, "You throw a coin into the deep well, and find a flask on its rim that was not there before."); }
                    _ => self.say(Tone::Info, "You throw a coin into the deep well. It falls a long time. Nothing comes back."),
                }
                self.wonder_deed(&name);
                Some(150)
            }
        }
    }

    /// The name of the wonder whose tile holds land cell (x, y).
    fn wonder_name(&self, x: i32, y: i32) -> String {
        let k = self.cell_tile(x, y);
        k.and_then(|k| self.world.wonders.get(&k)).map(|w| w.name.clone()).unwrap_or_else(|| "the wonder".into())
    }

    /// The world tile index of land cell (x, y) (the land floor only).
    pub fn cell_tile(&self, x: i32, y: i32) -> Option<usize> {
        if !self.on_land() { return None; }
        let t = self.tile_of_cell(self.global(x, y));
        Some(t.1 * self.world.w + t.0)
    }

    fn wonder_xp(&mut self, xp: u64) {
        for l in self.hero.gain_xp(xp) { self.say(Tone::Level, format!("You advanced to level {}.", l)); }
    }

    fn wonder_deed(&mut self, name: &str) {
        let turn = self.turn;
        if !self.deeds.iter().any(|d| d.1.contains(name)) { self.deeds.push((turn, format!("walked to {}", name))); }
        self.stats.wonders += 1;
    }

    /// Read a standing stone at land cell (x, y): its part of a story; the whole story told
    /// when all four are read.
    pub fn read_stone(&mut self, x: i32, y: i32, text: &str) {
        self.say(Tone::Quest, text.to_string());
        let Some(k) = self.cell_tile(x, y) else { return };
        let Some(st) = self.world.stones.get(&k).cloned() else { return };
        if self.stones_read.contains(&(st.story, st.part)) { return; }
        self.stones_read.push((st.story, st.part));
        let have = self.stones_read.iter().filter(|s| s.0 == st.story).count();
        let name = self.world.stories.get(st.story as usize).cloned().unwrap_or_default();
        if have >= 4 {
            let xp = 200 + 40 * self.hero.level as u64;
            self.wonder_xp(xp);
            self.say(Tone::Level, format!("You have read all four stones: you know the whole tale of {} now, as no one living tells it. ({} experience)", name, xp));
            let turn = self.turn;
            self.deeds.push((turn, format!("read the whole tale of {} on its four stones", name)));
            self.stats.stories += 1;
        } else {
            // Where the other stones stand: a sketch on the map.
            let w = self.world.w;
            let others: Vec<usize> = self.world.stones.iter().filter(|(_, s)| s.story == st.story && !self.stones_read.contains(&(s.story, s.part))).map(|(k, _)| *k).collect();
            for o in &others { if self.mapped.get(*o).copied().unwrap_or(2) == 0 { self.mapped[*o] = 1; } }
            let near = others.iter().min_by_key(|o| super::world::dist((*o % w, *o / w), self.tile, w)).map(|o| (o % w, o / w));
            let hint = near.map(|t| format!(" Another stands {} days {} of here.", (super::world::dist(t, self.tile, w) + 1) / 2, super::quest::direction(self.tile, t, w))).unwrap_or_default();
            self.say(Tone::Info, format!("{} of the four stones of {} read.{}", have, name, hint));
        }
    }

    /// Tall things in sight from far off on the land (towers, peaks, smoke): their cells are
    /// seen, and they are named (`far_seen`, for the title).
    pub fn far_sight(&mut self, near_r: i32) {
        self.far_seen.clear();
        if !self.on_land() { return; }
        let night = self.night();
        let reach = if night { 24 } else { 64 };
        let (hx, hy) = (self.x, self.y);
        let far = self.far.clone();
        let mut named: Vec<(i32, String)> = Vec::new();
        if let Some(p) = self.land.as_mut() {
            let f = &mut p.floors[0];
            for ((x, y), name) in far {
                let d = (x - hx).abs().max((y - hy).abs());
                if d > reach || d <= near_r || !f.inside(x, y) { continue; }
                for yy in y - 1..=y + 1 { for xx in x - 1..=x + 1 { if f.inside(xx, yy) { let k = yy as usize * f.w + xx as usize; f.seen[k] = true; } } }
                named.push((d, format!("{} {}", name, way(x - hx, y - hy))));
            }
        }
        named.sort();
        self.far_seen = named.into_iter().take(3).map(|n| n.1).collect();
    }
}

/// Which way a cell lies from here, in words ("to the north-east").
pub fn way(dx: i32, dy: i32) -> &'static str {
    let a = (dy as f32).atan2(dx as f32).to_degrees();
    let k = (((a + 360.0 + 22.5) % 360.0) / 45.0) as usize;
    ["to the east", "to the south-east", "to the south", "to the south-west", "to the west", "to the north-west", "to the north", "to the north-east"][k % 8]
}

fn cap(s: &str) -> String { let mut c = s.chars(); match c.next() { Some(f) => f.to_uppercase().collect::<String>() + c.as_str(), None => String::new() } }
