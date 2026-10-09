//! The world as the adventurer meets it: which tiles are land, what ground they are, how
//! dangerous the road is, and the places to enter, every one with its cause in the history
//! (DF's sites): living towns (temples, shops, lords with work), towns razed (ruins, the keeps of
//! fallen capitals as castles), beasts of the history in their lairs with their hoards, the dead
//! of battles in their tombs, gods' holy places (temples with crypts), cults' shrines, the
//! Shadow's seat; and what the land itself holds: caves in the hills, old mines, a labyrinth,
//! war camps of the raiding peoples, halls under the mountains.

use super::item::Item;
use super::map::Ground;
use super::site::{BossSpec, SiteKind, SiteSpec};
use crate::biomes::ExtendedBiome;
use crate::history::world_state::WorldHistory;
use crate::world::WorldData;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct WorldInfo {
    pub w: usize,
    pub h: usize,
    pub land: Vec<bool>,
    pub ground: Vec<Ground>,
    /// 0 safe .. 255 deadly: far from towns, under the Shadow, near lairs.
    pub danger: Vec<u8>,
    pub elevation: Vec<f32>,
    /// What the land is, for its ground, growth and perils.
    pub biome: Vec<ExtendedBiome>,
    pub temperature: Vec<f32>,
    pub moisture: Vec<f32>,
    /// A river runs through the tile, and the way it flows (0-7, `map::DIRS8`; 255 none).
    pub river: Vec<bool>,
    pub downhill: Vec<u8>,
    /// A road of the history crosses the tile.
    pub road: Vec<bool>,
    /// Forest cover and farmland (0-255) from the history's ecology; the Shadow's corruption 0-1.
    pub forest: Vec<u8>,
    pub farmland: Vec<u8>,
    pub shadow: Vec<f32>,
    /// Battles fought on the tile (the restless dead walk there at night).
    pub battles: Vec<u8>,
    /// What the history remembers of a tile (its battles), told when one walks onto it.
    #[serde(default)]
    pub tales: std::collections::BTreeMap<usize, String>,
}

impl Default for WorldInfo {
    fn default() -> Self { WorldInfo { w: 0, h: 0, land: vec![], ground: vec![], danger: vec![], elevation: vec![], biome: vec![], temperature: vec![], moisture: vec![], river: vec![], downhill: vec![], road: vec![], forest: vec![], farmland: vec![], shadow: vec![], battles: vec![], tales: Default::default() } }
}

impl WorldInfo {
    /// The first step (dx, dy) on a shortest walk over land from a to b.
    pub fn step_toward(&self, a: (usize, usize), b: (usize, usize)) -> Option<(i32, i32)> {
        let (w, h) = (self.w, self.h);
        let mut prev = vec![usize::MAX; w * h];
        let mut q = std::collections::VecDeque::new();
        prev[b.1 * w + b.0] = b.1 * w + b.0;
        q.push_back(b);
        // From the goal back toward a: the step is a's neighbour nearer the goal.
        while let Some((x, y)) = q.pop_front() {
            if (x, y) == a { break; }
            for dy in -1i32..=1 { for dx in -1i32..=1 {
                let ny = y as i32 + dy;
                if ny < 0 || ny >= h as i32 || (dx, dy) == (0, 0) { continue; }
                let nx = (x as i32 + dx).rem_euclid(w as i32) as usize;
                let k = ny as usize * w + nx;
                if prev[k] == usize::MAX && (self.land[k] || (nx, ny as usize) == a) { prev[k] = y * w + x; q.push_back((nx, ny as usize)); }
            } }
        }
        let p = prev[a.1 * w + a.0];
        if p == usize::MAX || p == a.1 * w + a.0 { return None; }
        let (px, py) = (p % w, p / w);
        let mut dx = px as i32 - a.0 as i32;
        if dx > 1 { dx -= w as i32; } else if dx < -1 { dx += w as i32; }
        Some((dx, py as i32 - a.1 as i32))
    }

    /// Whether b can be walked to from a over land (8-way, the map wrapping east-west).
    pub fn reachable(&self, a: (usize, usize), b: (usize, usize)) -> bool {
        let (w, h) = (self.w, self.h);
        let mut seen = vec![false; w * h];
        let mut q = std::collections::VecDeque::new();
        seen[a.1 * w + a.0] = true;
        q.push_back(a);
        while let Some((x, y)) = q.pop_front() {
            if (x, y) == b { return true; }
            for dy in -1i32..=1 { for dx in -1i32..=1 {
                let ny = y as i32 + dy;
                if ny < 0 || ny >= h as i32 { continue; }
                let nx = (x as i32 + dx).rem_euclid(w as i32) as usize;
                let k = ny as usize * w + nx;
                if !seen[k] && self.land[k] { seen[k] = true; q.push_back((nx, ny as usize)); }
            } }
        }
        false
    }
}

fn h64(a: u64, b: u64) -> u64 { let mut x = a.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.wrapping_mul(0xC2B2_AE3D_27D4_EB4F); x ^= x >> 31; x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9); x ^ (x >> 29) }

pub fn ground_of(b: ExtendedBiome) -> Ground {
    use ExtendedBiome::*;
    match b {
        Ice | Tundra | AlpineTundra | SnowyPeaks | FrozenLake | AuroraWastes => Ground::Snow,
        Desert | SaltFlats | SingingDunes | GlassDesert | Oasis | PaintedHills => Ground::Sand,
        Ashlands | VolcanicWasteland | ObsidianFields | BasaltColumns | SulfurVents | VoidScar => Ground::Ash,
        Swamp | Marsh | Bog | MangroveSaltmarsh | Shadowfen | CarnivorousBog | SpiritMarsh | TarPits => Ground::Mud,
        _ => Ground::Grass,
    }
}

pub fn dist(a: (usize, usize), b: (usize, usize), w: usize) -> i32 {
    let dx = (a.0 as i32 - b.0 as i32).abs();
    let dx = dx.min(w as i32 - dx);
    dx.max((a.1 as i32 - b.1 as i32).abs())
}

/// The race tag of a faction's people.
fn people_of(h: &WorldHistory, f: crate::history::FactionId) -> String {
    h.factions.get(&f).and_then(|f| h.races.get(&f.race_id)).map(|r| r.base_type.tag().to_string()).unwrap_or_else(|| "human".into())
}

fn god_of(h: &WorldHistory, f: crate::history::FactionId) -> String {
    h.factions.get(&f).and_then(|f| f.state_religion).and_then(|r| h.religions.get(&r)).and_then(|r| r.deities.first()).and_then(|d| h.deities.get(d))
        .map(|d| match d.epithets.first() { Some(e) => format!("{} {}", d.name, e), None => d.name.clone() }).unwrap_or_else(|| "the old gods".into())
}

/// An artifact of the history as an item the adventurer can carry.
pub fn artifact_item(a: &crate::history::objects::artifacts::Artifact) -> Item {
    let t = format!("{:?}", a.item_type).to_lowercase();
    let id = if t.contains("sword") || t.contains("blade") { "sword" } else if t.contains("axe") { "battle_axe" } else if t.contains("hammer") || t.contains("mace") { "battle_hammer" }
        else if t.contains("spear") || t.contains("lance") { "spear" } else if t.contains("bow") { "bow" } else if t.contains("shield") { "round_shield" }
        else if t.contains("helm") || t.contains("crown") { "steel_helmet" } else if t.contains("armor") || t.contains("armour") || t.contains("mail") { "plate_armor" }
        else if t.contains("ring") { "power_ring" } else if t.contains("amulet") || t.contains("necklace") || t.contains("pendant") { "silver_amulet" }
        else if t.contains("staff") || t.contains("wand") || t.contains("rod") { "wand_of_embers" } else { "gem_large" };
    let mut it = Item::new(id, 1);
    it.quality = 6;
    if matches!(id, "sword" | "battle_axe" | "battle_hammer" | "spear" | "round_shield" | "steel_helmet" | "plate_armor") { it.material = Some("steel".into()); }
    it.name = Some(a.name.clone());
    it.story = Some(a.description.clone());
    it.tag = a.id.0 as u32;
    it
}

/// What a beast of the history is in the adventure's bestiary (its body by size).
fn beast_def(m: &crate::monsters::Monster) -> &'static str {
    let b = m.base.to_lowercase();
    if b.contains("spider") || b.contains("insect") { "giant_spider" }
    else if b.contains("serpent") || b.contains("wyrm") || b.contains("dragon") || b.contains("lizard") { if m.size >= 2.0 { "dragon" } else { "wyvern" } }
    else if b.contains("giant") || b.contains("humanoid") || b.contains("ogre") { "cyclops" }
    else if m.size >= 2.2 { "dragon" } else if m.size >= 1.4 { "cyclops" } else { "troll" }
}

pub struct Built { pub info: WorldInfo, pub sites: Vec<SiteSpec>, pub start: u32 }

/// What a living town of the history is: its size, walls, way of building, gates and sea side.
pub fn town_shape(hist: &WorldHistory, s: &crate::history::civilizations::settlement::Settlement, land: &[bool], road: &[bool], w: usize, h: usize) -> super::site::TownShape {
    use crate::history::civilizations::settlement::{SettlementType as T, WallLevel as WL};
    let (x, y) = s.location;
    let size = match s.settlement_type { T::Capital | T::City => 3, T::Town | T::Port | T::Fort => 2, T::Village | T::Temple | T::Mine => 1, _ => 0 };
    let size = if s.population > 6000 { size.max(3) } else if s.population > 1500 { size.max(2) } else { size };
    let walls = match s.walls { WL::None => 0, WL::Palisade => 1, WL::StoneWall => 2, WL::Fortified | WL::Citadel => 3 };
    let arch = hist.factions.get(&s.faction).and_then(|f| hist.races.get(&f.race_id)).and_then(|r| hist.cultures.get(&r.culture_id)).map(|c| format!("{:?}", c.architecture).to_lowercase()).unwrap_or_else(|| "wood".into());
    let mut roads = 0u8;
    for (k, (dx, dy)) in super::map::DIRS8.iter().enumerate() { let ny = y as i32 + dy; if ny >= 0 && ny < h as i32 && road.get(ny as usize * w + (x as i32 + dx).rem_euclid(w as i32) as usize).copied().unwrap_or(false) { roads |= 1 << k; } }
    let mut sea = 0u8;
    for (k, (dx, dy)) in super::map::DIRS4.iter().enumerate() { let ny = y as i32 + dy; if ny >= 0 && ny < h as i32 && !land.get(ny as usize * w + (x as i32 + dx).rem_euclid(w as i32) as usize).copied().unwrap_or(true) { sea |= 1 << k; } }
    super::site::TownShape { size, walls, arch, population: s.population, port: matches!(s.settlement_type, T::Port), roads, sea, razed: None }
}

/// What a town has heard of the world lately, as its people tell it.
pub fn town_news(knowledge: &crate::history::knowledge::Knowledge, s: &crate::history::civilizations::settlement::Settlement) -> Vec<String> {
    knowledge.news_of_town(s.id, 40, 6).iter().map(|t| { let a = t.as_told(); if a.is_empty() { format!("{}.", t.line()) } else { format!("{}, {}.", t.line(), a) } }).collect()
}

/// A capital's ruler, who holds court in its hall: (name, title).
pub fn town_lord(hist: &WorldHistory, s: &crate::history::civilizations::settlement::Settlement) -> Option<(String, String)> {
    hist.factions.get(&s.faction).filter(|f| f.capital == Some(s.id)).and_then(|f| f.current_leader.and_then(|l| hist.figures.get(&l)).map(|fig| (fig.full_name(), fig.titles.first().cloned().unwrap_or_else(|| format!("ruler of {}", f.name)))))
}

/// The people (race tag) and god of a town of the history.
pub fn town_people(hist: &WorldHistory, s: &crate::history::civilizations::settlement::Settlement) -> (String, String) { (people_of(hist, s.faction), god_of(hist, s.faction)) }

/// A site's seed (the same rule as at the start).
pub fn site_seed(seed: u64, id: u32, tile: (usize, usize)) -> u64 { h64(seed ^ id as u64, (tile.0 * 7919 + tile.1) as u64) }

/// Read the world and its history into the adventure's map and places. `race` picks the start
/// town's people when possible.
pub fn build(world: &WorldData, history: Option<&WorldHistory>, seed: u64, race: Option<&str>) -> Built {
    let (w, h) = (world.width, world.height);
    let mut land = vec![false; w * h];
    let mut ground = vec![Ground::Grass; w * h];
    let mut elevation = vec![0.0f32; w * h];
    let mut biome = vec![ExtendedBiome::TemperateGrassland; w * h];
    let mut temperature = vec![10.0f32; w * h];
    let mut moisture = vec![0.5f32; w * h];
    let mut river = vec![false; w * h];
    for y in 0..h { for x in 0..w {
        let e = *world.heightmap.get(x, y);
        land[y * w + x] = e > 0.0;
        elevation[y * w + x] = e;
        biome[y * w + x] = *world.biomes.get(x, y);
        ground[y * w + x] = ground_of(*world.biomes.get(x, y));
        temperature[y * w + x] = *world.temperature.get(x, y);
        moisture[y * w + x] = *world.moisture.get(x, y);
        river[y * w + x] = e > 0.0 && world.river_tile_cache.as_ref().map_or(false, |r| *r.get(x, y));
    } }
    // Which way the water runs: the lowest neighbour.
    let mut downhill = vec![255u8; w * h];
    for y in 0..h { for x in 0..w {
        let e0 = elevation[y * w + x];
        let mut best = (255u8, e0);
        for (k, (dx, dy)) in super::map::DIRS8.iter().enumerate() {
            let ny = y as i32 + dy;
            if ny < 0 || ny >= h as i32 { continue; }
            let nx = (x as i32 + dx).rem_euclid(w as i32) as usize;
            let e = elevation[ny as usize * w + nx];
            if e < best.1 { best = (k as u8, e); }
        }
        downhill[y * w + x] = best.0;
    } }
    let mut road = vec![false; w * h];
    let mut forest = vec![0u8; w * h];
    let mut farmland = vec![0u8; w * h];
    let mut shadow_v = vec![0.0f32; w * h];
    let mut battles = vec![0u8; w * h];
    let mut tales: std::collections::BTreeMap<usize, String> = Default::default();
    if let Some(hist) = history {
        let ov = crate::tiles::classify::HistoryOverlay::from_history(hist, w, h);
        for k in 0..w * h {
            road[k] = ov.road.get(k).copied().unwrap_or(false);
            forest[k] = ov.cover.get(k).copied().unwrap_or(0);
            farmland[k] = ov.farmland.get(k).copied().unwrap_or(0);
            shadow_v[k] = ov.shadow.get(k).map_or(0.0, |v| *v as f32 / 255.0);
        }
        for e in hist.chronicle.events.iter().filter(|e| e.event_type == crate::history::events::types::EventType::BattleFought) {
            if let Some((x, y)) = e.location { if x < w && y < h {
                battles[y * w + x] = battles[y * w + x].saturating_add(1);
                let t = tales.entry(y * w + x).or_insert_with(String::new);
                if t.len() < 300 { if !t.is_empty() { t.push(' '); } t.push_str(&format!("Here was fought {} in {}.", e.title.trim_end_matches('.'), e.date.year)); }
            } }
        }
    }
    let mut sites: Vec<SiteSpec> = Vec::new();
    let mut next = 1u32;
    let mut add = |sites: &mut Vec<SiteSpec>, kind: SiteKind, name: String, tile: (usize, usize), tier: u32, cause: String, boss: Option<BossSpec>, floors: usize, people: String, god: String| -> u32 {
        let id = next;
        next += 1;
        let surface = ground[tile.1 * w + tile.0];
        sites.push(SiteSpec { id, kind, name, tile, seed: h64(seed ^ id as u64, (tile.0 * 7919 + tile.1) as u64), tier, cause, boss, treasures: Vec::new(), surface, rock: "granite".into(), floors, people, god, news: Vec::new(), lord: None, town: None, settlement: None, creature: None });
        id
    };
    let year = history.map_or(0, |hh| hh.current_date.year);
    if let Some(hist) = history {
        // Towns living and razed.
        let knowledge = crate::history::knowledge::Knowledge::new(hist);
        let mut setts: Vec<&crate::history::civilizations::settlement::Settlement> = hist.settlements.values().collect();
        setts.sort_by_key(|s| s.id.0);
        for s in setts {
            let (x, y) = s.location;
            if x >= w || y >= h { continue; }
            let people = people_of(hist, s.faction);
            let god = god_of(hist, s.faction);
            match s.destroyed {
                None => {
                    let id = add(&mut sites, SiteKind::Town, s.name.clone(), (x, y), 1, String::new(), None, 3, people, god);
                    if let Some(sp) = sites.iter_mut().find(|q| q.id == id) {
                        sp.town = Some(town_shape(hist, s, &land, &road, w, h));
                        sp.news = town_news(&knowledge, s);
                        sp.lord = town_lord(hist, s);
                        sp.settlement = Some(s.id.0);
                    }
                }
                Some(d) => {
                    let big = matches!(s.settlement_type, crate::history::civilizations::settlement::SettlementType::Capital | crate::history::civilizations::settlement::SettlementType::Fort);
                    let kind = if big { SiteKind::Castle } else { SiteKind::Ruin };
                    let ago = year.saturating_sub(d.year);
                    let cause = format!("{} was {} in {} ({} years ago); its {} are a ruin now{}.", s.name, if ago > 100 { "abandoned" } else { "razed" }, d.year, ago,
                        if big { "keep and walls" } else { "houses" }, if ago > 80 { ", and the dead do not lie quiet in it" } else { "" });
                    let boss = if big { Some(BossSpec { def: if ago > 80 { "vampire".into() } else { "ghost".into() }, name: format!("the {} of {}", if ago > 80 { "Pale Lord" } else { "Ghost Captain" }, s.name), scale: 1.2, legend: None, hoard: vec![Item::new("gold", 150)], story: format!("What is left of the lord of {}.", s.name) }) }
                        else { Some(BossSpec { def: "bandit".into(), name: format!("the Chief of the {} Ruins", s.name), scale: 1.6, legend: None, hoard: vec![Item::new("gold", 80)], story: "Outlaws hold the ruin now.".into() }) };
                    let id = add(&mut sites, kind, format!("the ruins of {}", s.name), (x, y), if big { 3 } else { 2 }, cause, boss, if big { 4 } else { 3 }, people, god);
                    if let Some(sp) = sites.iter_mut().find(|q| q.id == id) { sp.settlement = Some(s.id.0); }
                }
            }
        }
        // Beasts of the history in their lairs.
        let mut beasts: Vec<&crate::history::creatures::legendary::LegendaryCreature> = hist.legendary_creatures.values().filter(|c| c.death_date.is_none() && c.lair_location.is_some()).collect();
        beasts.sort_by_key(|c| c.id.0);
        for c in beasts {
            let (x, y) = c.lair_location.unwrap();
            if x >= w || y >= h { continue; }
            let m = crate::monsters::of_legend(hist, c);
            let def = beast_def(&m).to_string();
            let hoard: Vec<Item> = c.artifacts_owned.iter().filter_map(|a| hist.artifacts.get(a)).map(artifact_item).collect();
            let tier = (3.0 + c.size_multiplier).round().clamp(3.0, 6.0) as u32;
            let cause = format!("{} laired here.{} {}", c.full_name(), if c.kills.is_empty() { String::new() } else { format!(" It has killed {}.", c.kills.len()) }, if hoard.is_empty() { String::new() } else { format!("It sleeps on {}.", hoard.iter().map(|i| i.short()).collect::<Vec<_>>().join(", ")) });
            let boss = BossSpec { def, name: c.full_name(), scale: c.size_multiplier.clamp(1.0, 2.2), legend: Some(m.clone()), hoard: { let mut v = hoard; v.push(Item::new("gold", 100 * tier)); v }, story: m.description.clone() };
            let id = add(&mut sites, SiteKind::Lair, format!("the lair of {}", c.name), (x, y), tier, cause, Some(boss), 3, String::new(), String::new());
            if let Some(sp) = sites.iter_mut().find(|q| q.id == id) { sp.creature = Some(c.id.0); }
        }
        // The dead of battles in their tombs: each figure who fell in battle, by the battle fought
        // on the day they died; the tile of the battle with the most named dead gets its tomb.
        let mut fallen: std::collections::BTreeMap<(i32, u32), Vec<String>> = Default::default();
        let mut figs: Vec<&crate::history::entities::figures::Figure> = hist.figures.values().filter(|f| f.cause_of_death == Some(crate::history::entities::traits::DeathCause::Battle) && f.death_date.is_some()).collect();
        figs.sort_by_key(|f| f.id.0);
        for f in figs { let d = f.death_date.unwrap(); fallen.entry((d.year as i32, 0)).or_default().push(f.full_name()); }
        let mut by_tile: std::collections::BTreeMap<(usize, usize), (String, u32, Vec<String>)> = Default::default();
        for e in hist.chronicle.events.iter().filter(|e| e.event_type == crate::history::events::types::EventType::BattleFought) {
            let Some((x, y)) = e.location else { continue };
            let Some(dead) = fallen.get(&(e.date.year as i32, 0)) else { continue };
            let entry = by_tile.entry((x, y)).or_insert((e.title.clone(), e.date.year, Vec::new()));
            if dead.len() > entry.2.len() { *entry = (e.title.clone(), e.date.year, dead.clone()); }
        }
        let mut tombs: Vec<((usize, usize), (String, u32, Vec<String>))> = by_tile.into_iter().collect();
        tombs.sort_by_key(|(t, (_, _, d))| (std::cmp::Reverse(d.len()), *t));
        for ((bx, by), (title, yr, dead)) in tombs.into_iter().take(12) {
            if bx >= w || by >= h { continue; }
            // Beside the field (a battle at a town's walls is buried outside them).
            let spot = (0..=2i32).flat_map(|r| (-r..=r).flat_map(move |dy| (-r..=r).map(move |dx| (dx, dy)))).map(|(dx, dy)| (((bx as i32 + dx).rem_euclid(w as i32)) as usize, by as i32 + dy))
                .filter(|&(_, y)| y >= 0 && y < h as i32).map(|(x, y)| (x, y as usize)).find(|&(x, y)| land[y * w + x] && !sites.iter().any(|s| s.tile == (x, y)));
            let Some((x, y)) = spot else { continue };
            let captain = dead[0].clone();
            let cause = format!("The dead of the {} ({}) were laid here: {}.", title.trim_start_matches("The ").trim_start_matches("the "), yr, crate::persona::list(&dead.iter().take(4).cloned().collect::<Vec<_>>()));
            let boss = BossSpec { def: if dead.len() >= 3 { "mummy".into() } else { "skeleton".into() }, name: format!("{}, risen", captain), scale: 1.5, legend: None, hoard: vec![Item::new("gold", 120)], story: format!("{} fell in {} and was buried here. They do not rest.", captain, title) };
            add(&mut sites, SiteKind::Tomb, format!("the tomb of {}", captain.split(" the ").next().unwrap_or(&captain)), (x, y), 2 + (dead.len() as u32 / 3).min(2), cause, Some(boss), 3, String::new(), String::new());
        }
        // Monuments of the history: temples (crypts), keeps and towers (castles), tombs and
        // pyramids, altars and obelisks (shrines).
        let mut mons: Vec<&crate::history::objects::monuments::Monument> = hist.monuments.values().collect();
        mons.sort_by_key(|m| m.id.0);
        for m in mons {
            let (x, y) = m.location;
            if x >= w || y >= h || sites.iter().any(|s| s.tile == (x, y) && s.kind != SiteKind::Town) { continue; }
            use crate::history::objects::monuments::MonumentType as M;
            let built = m.built_date.year;
            let ago = year.saturating_sub(built);
            let people = people_of(hist, m.faction);
            let god = god_of(hist, m.faction);
            let state = if m.intact { format!("built in {}", built) } else { format!("built in {}, broken in {}", built, m.destruction_date.map_or(built, |d| d.year)) };
            let (kind, name, tier, boss) = match m.monument_type {
                M::Temple => (SiteKind::Temple, format!("the temple of {}", m.name.trim_start_matches("The ").trim_start_matches("the ")), 3,
                    BossSpec { def: "stone_golem".into(), name: format!("the Warden of {}", m.name), scale: 1.2, legend: None, hoard: vec![Item::new("gold", 160)], story: format!("Set to keep the inner sanctum of {}.", god) }),
                M::Castle | M::Tower | M::Wall => (SiteKind::Castle, m.name.clone(), 3,
                    BossSpec { def: if ago > 120 { "vampire".into() } else { "ghost".into() }, name: format!("the {} of {}", if ago > 120 { "Pale Lord" } else { "Ghost Captain" }, m.name), scale: 1.2, legend: None, hoard: vec![Item::new("gold", 220)], story: format!("What is left of the lords of {}.", m.name) }),
                M::Tomb | M::Pyramid => (SiteKind::Tomb, m.name.clone(), 3,
                    BossSpec { def: "mummy".into(), name: format!("the Sleeper of {}", m.name), scale: 1.7, legend: None, hoard: vec![Item::new("gold", 260), Item::new("gem_small", 2)], story: format!("Whoever {} was raised for, they do not sleep.", m.name) }),
                M::Altar | M::Obelisk => (SiteKind::Shrine, m.name.clone(), 3,
                    BossSpec { def: "cultist".into(), name: format!("the Keeper of {}", m.name), scale: 1.6, legend: None, hoard: vec![Item::new("gold", 140), Item::new("mana_potion", 2)], story: "A cult has taken the old stone for its own.".into() }),
                _ => continue,
            };
            let cause = format!("{} ({}) {}.", m.name, state, if m.intact { "stands still, and something has moved in" } else { "lies broken, and something lairs in its ruin" });
            add(&mut sites, kind, name, (x, y), tier, cause, Some(boss), if kind == SiteKind::Castle { 4 } else { 3 }, people, god);
        }
        // Gods' holy places outside towns: temples with crypts. Cults' shrines.
        let mut rels: Vec<&crate::history::religion::worship::Religion> = hist.religions.values().collect();
        rels.sort_by_key(|r| r.id.0);
        for r in rels {
            for &(x, y) in r.holy_sites.iter().take(2) {
                if x >= w || y >= h || sites.iter().any(|s| s.tile == (x, y)) { continue; }
                let god = r.deities.first().and_then(|d| hist.deities.get(d)).map(|d| d.name.clone()).unwrap_or_else(|| "a forgotten god".into());
                let boss = BossSpec { def: "stone_golem".into(), name: format!("the Warden of {}", god), scale: 1.2, legend: None, hoard: vec![Item::new("gold", 160)], story: format!("Set to guard the inner sanctum of {}.", god) };
                add(&mut sites, SiteKind::Temple, format!("the old temple of {}", god), (x, y), 3, format!("A holy place of {} ({}), long left; its crypts are not empty.", god, r.name), Some(boss), 3, String::new(), god);
            }
        }
        let mut cults: Vec<&crate::history::religion::monster_cults::MonsterCult> = hist.cults.values().collect();
        cults.sort_by_key(|c| c.id.0);
        for c in cults {
            let Some((x, y)) = c.headquarters else { continue };
            if x >= w || y >= h || sites.iter().any(|s| s.tile == (x, y)) { continue; }
            let beast = hist.legendary_creatures.get(&c.worshipped_creature).map(|b| b.full_name()).unwrap_or_else(|| "something below".into());
            let boss = BossSpec { def: "cultist".into(), name: format!("the High Priest of {}", c.name), scale: 1.8, legend: None, hoard: vec![Item::new("gold", 140), Item::new("great_mana_potion", 2)], story: format!("They worship {}.", beast) };
            add(&mut sites, SiteKind::Shrine, format!("the shrine of {}", c.name), (x, y), 3, format!("{} worship {} here{}.", c.name, beast, if c.sacrifices { ", with sacrifices" } else { "" }), Some(boss), 3, String::new(), beast);
        }
        // The Shadow's seat, and shrines of its cult on its land near it.
        if let Some(sh) = &hist.shadow {
            if sh.broken.is_none() {
                let mut placed = 0;
                for r in 3..12i32 { for k in 0..(8 * r) {
                    if placed >= 3 { break; }
                    let (dx, dy) = match k / r { 0 => (k % r - r / 2, -r), 1 => (r, k % r - r / 2), 2 => (k % r - r / 2, r), _ => (-r, k % r - r / 2) };
                    let (x, y) = ((sh.seat.0 as i32 + dx).rem_euclid(w as i32) as usize, sh.seat.1 as i32 + dy);
                    if y < 1 || y >= h as i32 - 1 { continue; }
                    let y = y as usize;
                    if !land[y * w + x] || sh.at(x, y) < 0.3 || sites.iter().any(|s| crate::adventure::world::dist(s.tile, (x, y), w) < 3) || h64(seed ^ 0x5AD0, (y * w + x) as u64) % 5 != 0 { continue; }
                    let boss = BossSpec { def: "cultist".into(), name: format!("the Herald of {}", sh.name), scale: 2.0, legend: None, hoard: vec![Item::new("gold", 300), Item::new("strong_mana_potion", 2)], story: format!("A voice of {} among the living.", sh.name) };
                    add(&mut sites, SiteKind::Shrine, format!("a shrine of {}", sh.name), (x, y), 4, format!("Those who serve {} gather here in the dark of its land.", sh.name), Some(boss), 3, String::new(), sh.name.clone());
                    placed += 1;
                } }
                let (x, y) = sh.seat;
                let boss = BossSpec { def: "demon".into(), name: format!("the Lord of {}", sh.name), scale: 1.4, legend: None, hoard: vec![Item::new("gold", 2000), Item::new("magic_plate_armor", 1)], story: format!("The power behind {}.", sh.name) };
                sites.retain(|s| s.tile != (x, y));
                add(&mut sites, SiteKind::DarkFortress, format!("the seat of {}", sh.name), (x, y), 6, format!("From here {} reaches over the land. No one who went in has come back.", sh.name), Some(boss), 5, String::new(), String::new());
            }
        }
        // Lost treasures of the history: in the place nearest where they were lost, else a ruin there.
        let mut lost: Vec<&crate::history::objects::artifacts::Artifact> = hist.artifacts.values().filter(|a| a.lost && !a.destroyed && a.current_location.is_some()).collect();
        lost.sort_by_key(|a| a.id.0);
        for a in lost {
            let at = a.current_location.unwrap();
            let it = artifact_item(a);
            if let Some(s) = sites.iter_mut().filter(|s| matches!(s.kind, SiteKind::Ruin | SiteKind::Tomb | SiteKind::Castle | SiteKind::Temple | SiteKind::Cave) && dist(s.tile, at, w) <= 2).min_by_key(|s| dist(s.tile, at, w)) {
                s.treasures.push(it);
                if !s.cause.contains(&a.name) { s.cause.push_str(&format!(" {} was lost here, they say.", a.name)); }
            } else if at.0 < w && at.1 < h && land[at.1 * w + at.0] && !sites.iter().any(|s| s.tile == at) {
                let id = add(&mut sites, SiteKind::Ruin, format!("the ruin where {} was lost", a.name), at, 3, format!("{} was lost here.", a.name), None, 2, String::new(), String::new());
                if let Some(s) = sites.iter_mut().find(|s| s.id == id) { s.treasures.push(it); }
            }
        }
    }
    // What the land holds: caves in the hills, old mines, a labyrinth, war camps, mountain halls.
    let towns: Vec<(usize, usize)> = sites.iter().filter(|s| s.kind == SiteKind::Town).map(|s| s.tile).collect();
    let cave_words = ["Hollow", "Grotto", "Deeps", "Warren", "Pit", "Caverns"];
    let cave_adj = ["Damp", "Echoing", "Black", "Weeping", "Bone", "Wolf", "Moss", "Bat", "Drowned", "Crooked"];
    for y in 1..h - 1 { for x in 0..w {
        let k = y * w + x;
        if !land[k] || sites.iter().any(|s| s.tile == (x, y)) { continue; }
        let e = elevation[k];
        let r = h64(seed ^ 0xCAFE, k as u64);
        let near_town = towns.iter().map(|&t| dist(t, (x, y), w)).min().unwrap_or(99);
        let kind = if e > 900.0 && r % 70 == 0 { Some(SiteKind::Halls) }
            else if e > 250.0 && r % 45 == 1 { Some(SiteKind::Cave) }
            else if e > 150.0 && r % 45 == 2 && near_town <= 4 { Some(SiteKind::Mine) }
            else if e > 200.0 && r % 260 == 3 { Some(SiteKind::Labyrinth) }
            else if r % 90 == 4 && near_town >= 3 { Some(SiteKind::Camp) }
            else if near_town <= 2 && near_town >= 1 && r % 14 == 5 { Some(SiteKind::Cave) }
            else { None };
        let Some(kind) = kind else { continue };
        let adj = cave_adj[(r >> 8) as usize % cave_adj.len()];
        let word = cave_words[(r >> 16) as usize % cave_words.len()];
        let who = super::town::person_name(if kind == SiteKind::Halls { "dwarf" } else { "human" }, r);
        let (name, tier, floors, cause, boss) = match kind {
            SiteKind::Cave => (format!("the {} {}", adj, word), 1 + (near_town.max(1) as u32 - 1) / 3, 3, String::new(), Some(BossSpec { def: if near_town <= 2 { "bear".into() } else { "troll".into() }, name: format!("the beast of the {} {}", adj, word), scale: 1.3, legend: None, hoard: vec![Item::new("gold", 60)], story: "It has made the deepest hollow its den.".into() })),
            SiteKind::Mine => (format!("{}'s old mine", who), 2, 3, format!("Worked out and left; something moved in."), Some(BossSpec { def: "goblin".into(), name: format!("the Goblin Boss of {}'s mine", who), scale: 1.8, legend: None, hoard: vec![Item::new("gold", 70), Item::new("iron_ore", 3)], story: "Goblins hold the deep galleries.".into() })),
            SiteKind::Labyrinth => (format!("the Maze of {}", who), 3, 3, "A maze cut into the hill by no one remembers whom; the horned ones live in it.".into(), Some(BossSpec { def: "minotaur".into(), name: format!("the Minotaur Mage of {}", who), scale: 1.7, legend: None, hoard: vec![Item::new("gold", 200)], story: "The oldest of the horned ones.".into() })),
            SiteKind::Camp => (format!("a war camp of the hills"), 2, 2, "Orcs and goblins gather here to raid the roads.".into(), Some(BossSpec { def: "orc_warrior".into(), name: format!("{} the Warlord", super::town::person_name("orc", r)), scale: 1.5, legend: None, hoard: vec![Item::new("gold", 120)], story: "He leads the raids.".into() })),
            SiteKind::Halls => (format!("the Halls of {}", who), 4, 4, format!("Dwarves of the house of {} cut these halls; the house is gone.", who), Some(BossSpec { def: "stone_golem".into(), name: format!("the Iron Guardian of {}", who), scale: 1.5, legend: None, hoard: vec![Item::new("gold", 300), Item::of("battle_axe", "steel", 4)], story: "It still keeps the throne hall.".into() })),
            _ => continue,
        };
        // (Names are never shared: a second Damp Hollow is someone's.)
        let name = if sites.iter().any(|s| s.name == name) { format!("{} of {}", name, who) } else { name };
        let boss = boss.map(|mut b| { if sites.iter().any(|s| s.boss.as_ref().map_or(false, |o| o.name == b.name)) { b.name = format!("{} of {}", b.name, who); } b });
        add(&mut sites, kind, name, (x, y), tier.max(1), cause, boss, floors, String::new(), String::new());
    } }
    // Danger: distance from the nearest town, the Shadow's corruption, lairs near.
    let mut danger = vec![0u8; w * h];
    for y in 0..h { for x in 0..w {
        let d = towns.iter().map(|&t| dist(t, (x, y), w)).min().unwrap_or(30);
        let shadow = history.and_then(|hh| hh.shadow.as_ref()).map_or(0.0, |s| s.at(x, y));
        let lair = sites.iter().filter(|s| s.kind == SiteKind::Lair).map(|s| dist(s.tile, (x, y), w)).min().unwrap_or(99);
        let v = d as f32 * 18.0 + shadow * 220.0 + if lair <= 2 { 60.0 } else { 0.0 };
        danger[y * w + x] = v.clamp(0.0, 255.0) as u8;
    } }
    // Tiers grow with the distance from the start town.
    let start = {
        let mut towns: Vec<&SiteSpec> = sites.iter().filter(|s| s.kind == SiteKind::Town).collect();
        towns.sort_by_key(|s| {
            let near = sites.iter().filter(|o| o.kind != SiteKind::Town && dist(o.tile, s.tile, w) <= 3).count() as i32;
            let shadow = danger[s.tile.1 * w + s.tile.0] as i32;
            let wrong_race = race.map_or(0, |r| if s.people == r { 0 } else { 40 });
            (-near * 6 + shadow / 4 + wrong_race, s.id)
        });
        towns.first().map(|s| s.id).unwrap_or(0)
    };
    if let Some(home) = sites.iter().find(|s| s.id == start).map(|s| s.tile) {
        for s in sites.iter_mut() {
            if matches!(s.kind, SiteKind::Town | SiteKind::DarkFortress) { continue; }
            let d = dist(s.tile, home, w) as u32;
            s.tier = (s.tier + d / 8).clamp(1, 6);
        }
    }
    let mut built = Built { info: WorldInfo { w, h, land, ground, danger, elevation, biome, temperature, moisture, river, downhill, road, forest, farmland, shadow: shadow_v, battles, tales }, sites, start };
    // A world with no towns at all: one is made where the land is best.
    if built.start == 0 {
        let k = (0..w * h).find(|&k| built.info.land[k] && built.info.ground[k] == Ground::Grass).unwrap_or(0);
        let id = built.sites.iter().map(|s| s.id).max().unwrap_or(0) + 1;
        built.sites.push(SiteSpec { id, kind: SiteKind::Town, name: "Hearthwater".into(), tile: (k % w, k / w), seed: h64(seed, 7), tier: 1, cause: String::new(), boss: None, treasures: Vec::new(), surface: Ground::Grass, rock: "granite".into(), floors: 3, people: "human".into(), god: "the old gods".into(), news: Vec::new(), lord: None, town: None, settlement: None, creature: None });
        built.start = id;
    }
    built
}
