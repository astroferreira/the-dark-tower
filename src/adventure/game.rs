//! The adventure's state and its rules: one adventurer, the world's places (realized when first
//! entered and kept), time in ticks (a step at walking pace is 100), the log. Turn-based as DF's
//! adventure mode: each act costs time and every creature acts when its banked time allows.
//! Bump to act (Tibia and every roguelike): into a monster to strike it, a door to open it (with
//! its key), a chest to open it, a lever to pull it, a person to talk to them.

use super::actor::Monster;
use super::data::data;
use super::hero::{Hero, Skill, Slot};
use super::item::{stow, Item};
use super::map::{Feature, Ground, Wall, DIRS8};
use super::site::{realize, Place, SiteKind, SiteSpec};
use super::world::WorldInfo;
use super::land::LAND;
use super::surface::CH;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Tone { Info, Hit, Hurt, Loot, Level, Talk, Quest, Danger, Death }

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Line { pub turn: u64, pub text: String, pub tone: Tone }

/// Something for the window to show for a moment.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum Effect {
    /// A number over a cell (damage red, healing green, mana blue).
    Number { x: i32, y: i32, z: usize, value: i32, tone: Tone },
    Missile { from: (i32, i32), to: (i32, i32), z: usize, kind: String },
    /// Cells struck by a spell or a breath.
    Area { cells: Vec<(i32, i32)>, z: usize, kind: String },
    Speech { x: i32, y: i32, z: usize, text: String },
    Puff { x: i32, y: i32, z: usize },
}

/// A sellsword hired at an inn (DF's companions): follows the adventurer between floors and
/// places, strikes what is beside them, and can die.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Companion {
    pub name: String, pub race: String, pub hp: i32, pub max_hp: i32, pub x: i32, pub y: i32, pub energy: i32, pub left: bool, pub kills: u32, pub struck_at: u64,
    /// Their heart for the road (0 and they leave): fights won raise it, wounds and unpaid
    /// wages lower it; a day's rest mends it.
    #[serde(default = "full_morale")]
    pub morale: i32,
    /// The day their wage was last paid (the grasping ones want it every ten days).
    #[serde(default)]
    pub paid_day: u64,
}

fn full_morale() -> i32 { 100 }

impl Companion {
    /// Who they are: their temper (rolled like any townsperson's).
    pub fn temper(&self) -> super::people::Temper {
        super::people::temper(&crate::persona::Persona::roll(&self.race, None, crate::persona::seed_of(&self.name, 0xC0A1)))
    }
    /// What they say when something happens to them, in their temper's words.
    pub fn line(&self, what: &str) -> String {
        use super::people::Temper as T;
        let t = self.temper();
        let s = match (what, t) {
            ("hurt", T::Timid) => "This is madness! We will die here!",
            ("hurt", T::Gruff) | ("hurt", T::Proud) => "Is that all you have?",
            ("hurt", T::Gloomy) => "I knew it would end like this.",
            ("hurt", _) => "I'm bleeding, watch my back!",
            ("won", T::Cheerful) | ("won", T::Kind) => "Ha! We did it! Drinks are on you tonight.",
            ("won", T::Greedy) => "Now that is worth a bonus, wouldn't you say?",
            ("won", T::Gloomy) => "We live another day. Don't get used to it.",
            ("won", _) => "Well fought.",
            ("pay", T::Greedy) => "No coin, no blade. That was the bargain.",
            ("pay", _) => "My wages are late.",
            ("leave", T::Timid) => "I can't do this any more. I'm sorry. I'm going home.",
            ("leave", T::Greedy) => "Find another fool to bleed for you. I'm done.",
            ("leave", T::Proud) => "I will not follow one who leads like this. Farewell.",
            ("leave", _) => "I've had enough of this road. Good luck to you.",
            _ => "",
        };
        format!("{}: \"{}\"", self.name, s)
    }
}

impl Companion {
    pub fn attack(&self, level: u32) -> i32 { 10 + level as i32 }
    pub fn defense(&self, level: u32) -> i32 { 8 + level as i32 / 2 }
    pub fn armor(&self, level: u32) -> i32 { 3 + level as i32 / 3 }
}

/// A body left where something fell (drawn until it rots).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Corpse { pub x: i32, pub y: i32, pub z: usize, pub def: String, pub name: String, pub turn: u64 }

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Action {
    Move(i32, i32),
    Wait,
    /// Rest until healed or something comes into sight.
    Rest,
    PickUp,
    Drop(usize),
    Equip(usize),
    Unequip(Slot),
    /// Eat, drink, light, read.
    UseItem(usize),
    /// Strike or shoot at a monster by uid.
    Attack(u32),
    /// Cast a known spell (by index in `hero.spells`) at a monster (or the nearest).
    Cast(usize, Option<u32>),
    /// Take one of a quest chest's rewards (the chest at the hero's side).
    Choose(usize),
    /// On the world map: walk a tile.
    Travel(i32, i32),
    /// On the world map: go into the place on this tile.
    Enter,
    /// Take the stairs, ladder, hole, rope or way out underfoot.
    Climb,
    /// On the world map: go into this place (several may share a tile).
    EnterSite(u32),
    /// On the land: take to the road (the world map).
    WorldMap,
    /// On the world map: walk the land where one is.
    Land,
    /// Answer the choice card (`Game::choice`).
    Decide(usize),
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Game {
    pub seed: u64,
    pub hero: Hero,
    #[serde(skip)]
    pub world: WorldInfo,
    pub sites: Vec<SiteSpec>,
    pub places: HashMap<u32, Place>,
    /// Inside a place: its id; None on the world map.
    pub here: Option<u32>,
    pub z: usize,
    pub x: i32,
    pub y: i32,
    pub tile: (usize, usize),
    pub turn: u64,
    pub log: Vec<Line>,
    pub effects: Vec<Effect>,
    pub corpses: HashMap<u32, Vec<Corpse>>,
    /// Places the adventurer knows of (heard of or seen).
    pub known: Vec<u32>,
    /// Slain monsters to come back (place, monster as it was, turn slain).
    pub respawn: Vec<(u32, Monster, u64)>,
    pub quests: Vec<super::quest::Quest>,
    /// A conversation under way (with the person at this index of the place's npcs).
    pub talk: Option<super::npc::Talk>,
    /// The quest chests already chosen from (Tibia: one reward each).
    pub chosen: Vec<u32>,
    /// Facing (for waves and the figure).
    pub facing: (i32, i32),
    #[serde(skip, default = "fresh_rng")]
    pub rng: ChaCha8Rng,
    /// Visible cells of the current floor (recomputed after each act).
    #[serde(skip)]
    pub sight: Vec<bool>,
    /// What the adventurer is told on the screen when something must be answered (a death).
    pub banner: Option<(String, String)>,
    /// Statistics for the tests and the bot.
    pub stats: Stats,
    /// Named enemies slain (bosses do not come back, and no one asks for them again).
    pub slain: Vec<String>,
    #[serde(default)]
    pub companion: Option<Companion>,
    /// The Shadow's lord broken: the great deed (the adventure's end, though one may go on).
    #[serde(default)]
    pub victory: bool,
    /// The great deeds, for the adventurer's legend: (turn, words).
    #[serde(default)]
    pub deeds: Vec<(u64, String)>,
    /// The land about the adventurer while they walk it (`land`: rebuilt from `chunks`).
    #[serde(skip)]
    pub land: Option<Place>,
    /// The world tile in the middle of the land floor.
    #[serde(default)]
    pub centre: (usize, usize),
    /// The land's chunks as they were left.
    #[serde(default, with = "super::land::chunk_map")]
    pub chunks: HashMap<(u32, u32), super::land::Chunk>,
    #[serde(skip)]
    pub atlas: super::surface::Atlas,
    /// Chunks as made (the base their changes are kept against).
    #[serde(skip)]
    pub pristine: HashMap<(u32, u32), Vec<super::map::Tile>>,
    /// The Mapmaker's map: per world tile 0 blank, 1 heard of, 2 inked.
    #[serde(default)]
    pub mapped: Vec<u8>,
    /// The tiles walked, in order (drawn on the map).
    #[serde(default)]
    pub route: Vec<(u16, u16)>,
    /// Tiles inked on foot since a sage last bought the charts.
    #[serde(default)]
    pub charted: u32,
    /// Treasure maps read (their crosses on the map) and dug.
    #[serde(default)]
    pub marks: Vec<u32>,
    #[serde(default)]
    pub dug: Vec<u32>,
    /// Saved with the walkable land (older saves wake in the temple, places made anew).
    #[serde(default)]
    pub seamless: bool,
    #[serde(default)]
    pub next_uid: u32,
    /// The land floor moved under the adventurer by (dx, dy) cells (for the window's easing).
    #[serde(skip)]
    pub shifted: (i32, i32),
    /// Seasons of the history stepped while one played (`living`), and the adventurer's deeds
    /// put into it (replayed on load).
    #[serde(default)]
    pub seasons: u32,
    #[serde(default)]
    pub hero_events: Vec<super::living::HeroEvent>,
    /// Deeds waiting for the host to put them into the history.
    #[serde(skip)]
    pub deed_queue: Vec<super::living::HeroEvent>,
    /// The adventurer as a figure of the history.
    #[serde(default)]
    pub hero_figure: Option<u64>,
    /// What each town's bards sing of the adventurer (site -> lines), from the history.
    #[serde(skip)]
    pub songs: HashMap<u32, Vec<String>>,
    /// The world's history as it stands (shared by the host's `Living`), for talk.
    #[serde(skip)]
    pub history: Option<std::sync::Arc<crate::history::world_state::WorldHistory>>,
    /// A name being typed in talk (the window fills it; Enter asks).
    #[serde(skip)]
    pub typing: Option<String>,
    /// A choice to make (a tale's turning point).
    #[serde(default)]
    pub choice: Option<super::tales::Choice>,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Stats { pub kills: u32, pub bosses: u32, pub chests: u32, pub floors_seen: u32, pub deaths: u32, pub gold_found: u32, pub quests_done: u32, pub sites_entered: u32, #[serde(default)] pub tales_done: u32 }

impl Game {
    pub fn new(world: WorldInfo, sites: Vec<SiteSpec>, hero: Hero, start_town: u32, seed: u64) -> Game {
        let mut g = Game {
            seed, hero, world, sites, places: HashMap::new(), here: None, z: 0, x: 0, y: 0, tile: (0, 0), turn: 0, log: Vec::new(), effects: Vec::new(),
            corpses: HashMap::new(), known: Vec::new(), respawn: Vec::new(), quests: Vec::new(), talk: None, chosen: Vec::new(), facing: (0, 1),
            rng: ChaCha8Rng::seed_from_u64(seed ^ 0xADE0), sight: Vec::new(), banner: None, stats: Stats::default(), slain: Vec::new(), companion: None, victory: false, deeds: Vec::new(),
            land: None, centre: (0, 0), chunks: HashMap::new(), atlas: Default::default(), pristine: HashMap::new(), mapped: Vec::new(), route: Vec::new(), charted: 0, marks: Vec::new(), dug: Vec::new(), seamless: true, next_uid: 1_000_000, shifted: (0, 0),
            seasons: 0, hero_events: Vec::new(), deed_queue: Vec::new(), hero_figure: None, songs: HashMap::new(), history: None, typing: None, choice: None,
        };
        g.set_atlas();
        g.hero.temple = start_town;
        if let Some(s) = g.site(start_town) { g.tile = s.tile; }
        // They know the towns and what lies near their home.
        let home = g.tile;
        let known: Vec<u32> = g.sites.iter().filter(|s| s.kind == SiteKind::Town || (s.kind != SiteKind::Cellar && ((s.tile.0 as i32 - home.0 as i32).abs() + (s.tile.1 as i32 - home.1 as i32).abs()) <= 4)).map(|s| s.id).collect();
        g.known = known;
        g.start_map();
        // Wake in the temple.
        g.wake_at_temple();
        let town = g.site(start_town).map(|s| s.name.clone()).unwrap_or_default();
        g.say(Tone::Level, format!("{} wakes in the temple of {}. The world is wide, and you are nobody yet: a club, a patched tunic, twenty coins. Below the square, the townsfolk say, the sewers are full of rats.", g.hero.name, town));
        let who = g.hero.name.clone();
        g.chronicle(super::living::DeedKind::Arrived, format!("{} sets out from {}", who, town), format!("{} left {} to make a name in the world.", who, town));
        g
    }

    pub fn site(&self, id: u32) -> Option<&SiteSpec> { self.sites.iter().find(|s| s.id == id) }
    pub fn place(&self) -> Option<&Place> { match self.here { Some(LAND) => self.land.as_ref(), Some(id) => self.places.get(&id), None => None } }
    pub fn place_mut(&mut self) -> Option<&mut Place> { match self.here { Some(LAND) => self.land.as_mut(), Some(id) => self.places.get_mut(&id), None => None } }
    fn pl(&self, id: u32) -> &Place { if id == LAND { self.land.as_ref().unwrap() } else { &self.places[&id] } }
    fn pl_mut(&mut self, id: u32) -> &mut Place { if id == LAND { self.land.as_mut().unwrap() } else { self.places.get_mut(&id).unwrap() } }
    /// The site the adventurer stands in (a town on the land is its own; else the land).
    pub fn site_here(&self) -> u32 { self.place().map_or(0, |p| p.spec.id) }
    pub fn floor(&self) -> Option<&super::map::Floor> { self.place().and_then(|p| p.floors.get(self.z)) }
    pub fn say(&mut self, tone: Tone, text: impl Into<String>) { let t = text.into(); self.log.push(Line { turn: self.turn, text: t, tone }); if self.log.len() > 400 { self.log.drain(0..100); } }
    pub fn take_effects(&mut self) -> Vec<Effect> { std::mem::take(&mut self.effects) }

    /// A roll seeded by the act (DF reseeds from the actors and the tick so a reload repeats it).
    fn roll(&mut self, salt: u64) -> ChaCha8Rng { let s = self.rng.gen::<u64>() ^ self.turn.wrapping_mul(0x9E37_79B9) ^ salt; ChaCha8Rng::seed_from_u64(s) }

    // -----------------------------------------------------------------------------------------
    // Places

    pub fn enter_site(&mut self, id: u32, quiet: bool) {
        if !self.places.contains_key(&id) {
            let Some(spec) = self.site(id).cloned() else { return };
            let p = realize(&spec);
            self.places.insert(id, p);
        }
        // Bring back what was slain long ago (Tibia's respawns; bosses stay dead).
        let turn = self.turn;
        let back: Vec<Monster> = self.respawn.iter().filter(|(p, m, t)| *p == id && !m.boss && turn > t + 12_000).map(|(_, m, _)| m.clone()).collect();
        self.respawn.retain(|(p, m, t)| !(*p == id && !m.boss && turn > t + 12_000));
        if let Some(p) = self.places.get_mut(&id) {
            for mut m in back { m.hp = m.max_hp; m.x = m.home.0; m.y = m.home.1; m.awake = false; p.monsters.push(m); }
        }
        self.here = Some(id);
        let p = &self.places[&id];
        self.z = 0;
        // One step inside the way in (standing on the way out, one would have to step off and
        // back on to leave).
        let f = &p.floors[0];
        let (ex, ey) = p.entry;
        let inside = [(0, -1), (-1, -1), (1, -1), (-1, 0), (1, 0), (0, 1)].iter().map(|(dx, dy)| (ex + dx, ey + dy)).find(|&(x, y)| f.walkable(x, y) && f.at(x, y).feature == Feature::None).unwrap_or((ex, ey));
        self.x = inside.0;
        self.y = inside.1;
        if !self.known.contains(&id) { self.known.push(id); }
        if self.site(id).map_or(false, |s| s.kind != SiteKind::Wilds) { self.stats.sites_entered += 1; }
        if !quiet {
            let (name, kind, cause, floor) = (p.spec.name.clone(), p.spec.kind, p.spec.cause.clone(), p.floors[0].name.clone());
            let _ = &name;
            self.say(Tone::Info, format!("You come to {}{} ({}).{}", name, if kind == SiteKind::Town { String::new() } else { format!(", {}", kind.word()) }, floor, if cause.is_empty() { String::new() } else { format!(" {}", cause) }));
        }
        self.companion_follow(true);
        self.look();
    }

    fn wake_at_temple(&mut self) {
        let t = self.hero.temple;
        let tile = self.site(t).map(|s| s.tile).unwrap_or(self.tile);
        self.land_at(tile, None);
        // Before the altar of the temple (not where the priest stands).
        let spot = self.floor().and_then(|f| {
            let c = (CH + CH / 2, CH + CH / 2);
            f.cells(|x| matches!(x.feature, Feature::Altar)).into_iter().filter(|&(x, y)| (x - c.0).abs() < CH / 2 && (y - c.1).abs() < CH / 2).min_by_key(|&(x, y)| (x - c.0).abs() + (y - c.1).abs())
        });
        if let Some((ax, ay)) = spot {
            let p = self.place().unwrap();
            let f = &p.floors[0];
            let near: Vec<(i32, i32)> = (1..=3).flat_map(|r| (-r..=r).flat_map(move |dy| (-r..=r).map(move |dx| (ax + dx, ay + dy)))).collect();
            if let Some(&(x, y)) = near.iter().find(|&&(x, y)| f.walkable(x, y) && f.at(x, y).feature == Feature::None && !p.npcs.iter().any(|n| (n.x, n.y) == (x, y))) { self.x = x; self.y = y; }
        }
        self.companion_follow(true);
        self.look();
    }

    /// Bring the companion beside the adventurer (after a change of floor or place, or when left
    /// behind).
    pub fn companion_follow(&mut self, force: bool) {
        let (hx, hy) = (self.x, self.y);
        let Some(f) = self.floor() else { return };
        let occupied = |x: i32, y: i32| self.place().map_or(false, |p| p.monsters.iter().any(|m| m.z == self.z && m.x == x && m.y == y && m.hp > 0) || p.npcs.iter().any(|n| n.z == self.z && n.x == x && n.y == y));
        let Some(c) = self.companion.as_ref() else { return };
        let far = (c.x - hx).abs().max((c.y - hy).abs()) > 6 || !f.walkable(c.x, c.y);
        if !force && !far { return; }
        let spot = DIRS8.iter().map(|(dx, dy)| (hx + dx, hy + dy)).find(|&(x, y)| f.walkable(x, y) && !occupied(x, y) && f.at(x, y).feature == Feature::None);
        if let (Some((x, y)), Some(c)) = (spot, self.companion.as_mut()) { c.x = x; c.y = y; }
    }

    /// What the adventurer sees now.
    pub fn look(&mut self) {
        let Some(id) = self.here else { return };
        let night = self.night();
        let r = { let f = &self.pl(id).floors[self.z]; if f.outdoor { if night { (self.hero.light() + 2).max(if self.full_moon() { 8 } else { 5 }) } else { 12 } } else { self.hero.light() + 1 } };
        let (x, y) = (self.x, self.y);
        let z = self.z;
        let p = self.pl_mut(id);
        let f = &mut p.floors[z];
        let vis = f.sight(x, y, r);
        for (k, v) in vis.iter().enumerate() { if *v { f.seen[k] = true; } }
        // Lit places (sconces, braziers, fires) are seen when in line of sight, however far.
        self.sight = vis;
    }

    pub fn visible(&self, x: i32, y: i32) -> bool {
        self.floor().map_or(false, |f| f.inside(x, y) && self.sight.get(y as usize * f.w + x as usize).copied().unwrap_or(false))
    }

    // -----------------------------------------------------------------------------------------
    // Acting

    /// Do `a`; returns whether time passed.
    pub fn act(&mut self, a: Action) -> bool {
        if self.banner.is_some() { self.banner = None; }
        // A choice waits to be made.
        if self.choice.is_some() && !matches!(a, Action::Decide(_)) { return false; }
        let cost = match a {
            Action::Travel(dx, dy) => return self.travel(dx, dy),
            Action::Enter => return self.enter_here(),
            Action::EnterSite(id) => { if self.here.is_none() && self.site(id).map_or(false, |s| s.tile == self.tile) { return self.land_here(Some(id)); } return false; }
            Action::WorldMap => return self.to_world_map(),
            Action::Land => return self.land_here(None),
            Action::Decide(k) => return super::tales::decide(self, k),
            Action::Move(dx, dy) => self.step(dx, dy),
            Action::Wait => Some(100),
            Action::Rest => return self.rest(),
            Action::PickUp => self.pick_up(),
            Action::Drop(k) => { if k < self.hero.pack.len() { let it = self.hero.pack.remove(k); self.say(Tone::Info, format!("You drop {}.", it.describe())); let (x, y, z) = (self.x, self.y, self.z); if let Some(p) = self.place_mut() { p.floors[z].drop_item(x, y, it); } Some(50) } else { None } }
            Action::Equip(k) => match self.hero.equip(k) { Ok(l) => { self.say(Tone::Info, l); Some(60) } Err(e) => { self.say(Tone::Info, e); None } },
            Action::Unequip(s) => self.hero.unequip(s).map(|l| { self.say(Tone::Info, l); 60 }),
            Action::UseItem(k) => self.use_item(k),
            Action::Attack(uid) => self.attack(uid),
            Action::Cast(k, t) => self.cast(k, t),
            Action::Choose(k) => self.choose(k),
            Action::Climb => self.climb(),
        };
        match cost { Some(c) => { self.pass(c); true } None => false }
    }

    fn monster_at(&self, x: i32, y: i32) -> Option<usize> {
        let p = self.place()?;
        p.monsters.iter().position(|m| m.z == self.z && m.x == x && m.y == y && m.hp > 0)
    }
    fn npc_at(&self, x: i32, y: i32) -> Option<usize> {
        let p = self.place()?;
        p.npcs.iter().position(|n| n.z == self.z && n.x == x && n.y == y)
    }

    fn step(&mut self, dx: i32, dy: i32) -> Option<i32> {
        self.facing = (dx, dy);
        let (nx, ny) = (self.x + dx, self.y + dy);
        if let Some(k) = self.monster_at(nx, ny) {
            let uid = self.place().unwrap().monsters[k].uid;
            return self.melee(uid);
        }
        if let Some(k) = self.npc_at(nx, ny) { super::npc::greet(self, k); return None; }
        if self.companion.as_ref().map_or(false, |c| (c.x, c.y) == (nx, ny)) {
            let (ox, oy) = (self.x, self.y);
            if let Some(c) = self.companion.as_mut() { c.x = ox; c.y = oy; }
            self.x = nx; self.y = ny;
            self.look();
            return Some(self.hero.step_time());
        }
        let z = self.z;
        let f = self.floor()?;
        if !f.inside(nx, ny) { return None; }
        // No squeezing between two walls on a diagonal.
        if dx != 0 && dy != 0 && !f.at(self.x + dx, self.y).walkable() && !f.at(self.x, self.y + dy).walkable() { return None; }
        let t = f.at(nx, ny).clone();
        match &t.feature {
            Feature::Door { open: false, lock } => {
                let lock = *lock;
                if lock > 0 {
                    let has = self.hero.pack.iter().any(|i| i.id == "key" && i.tag == lock);
                    if !has { self.say(Tone::Info, "The door is locked. Somewhere there is a key."); return None; }
                    self.say(Tone::Info, "You turn the key in the lock.");
                }
                if let Some(p) = self.place_mut() { p.floors[z].at_mut(nx, ny).feature = Feature::Door { open: true, lock: 0 }; }
                self.look();
                return Some(60);
            }
            Feature::Gate { open: false, .. } => { self.say(Tone::Info, "A portcullis bars the way. A lever must raise it."); return None; }
            Feature::LevelDoor { level } => {
                if self.hero.level < *level { let l = *level; self.say(Tone::Info, format!("The rune glows and the door holds: \"Only those of level {} and more may pass.\"", l)); return None; }
                self.say(Tone::Info, "The rune knows you. The door swings aside.");
            }
            Feature::Lever { id, pulled } => {
                let (id, pulled) = (*id, *pulled);
                if let Some(p) = self.place_mut() {
                    let f = &mut p.floors[z];
                    f.at_mut(nx, ny).feature = Feature::Lever { id, pulled: !pulled };
                    for t in f.tiles.iter_mut() { if let Feature::Gate { lever, open } = &mut t.feature { if *lever == id { *open = !pulled; } } }
                }
                self.say(Tone::Info, if pulled { "You push the lever back. Somewhere a portcullis grinds down." } else { "You pull the lever. Somewhere a portcullis grinds up." });
                self.look();
                return Some(80);
            }
            Feature::Chest { opened: false, .. } => { self.open_chest(nx, ny); return Some(80); }
            Feature::Sarcophagus { opened: false, .. } => { self.open_sarcophagus(nx, ny); return Some(150); }
            Feature::QuestChest { taken, choices, .. } => {
                if *taken || self.chosen.contains(&self.here.unwrap_or(0)) { self.say(Tone::Info, "The chest is empty."); }
                else {
                    let list: Vec<String> = choices.iter().enumerate().map(|(k, c)| format!("{} {}", k + 1, c.describe())).collect();
                    self.say(Tone::Quest, format!("An iron-bound chest. You may take one thing of what lies in it: {}. (Press 1-{} beside it.)", list.join("; "), choices.len()));
                }
                return None;
            }
            Feature::Plinth { item: Some(it) } => {
                let it = it.clone();
                self.say(Tone::Loot, format!("You lift {} from its plinth.{}", it.describe(), it.story.as_ref().map(|s| format!(" {}", s)).unwrap_or_default()));
                if let Some(p) = self.place_mut() { p.floors[z].at_mut(nx, ny).feature = Feature::Plinth { item: None }; }
                super::quest::on_found(self, &it);
                stow(&mut self.hero.pack, it);
                return Some(100);
            }
            Feature::Sign { text } => { let t = text.clone(); self.say(Tone::Info, format!("The sign reads: \"{}\"", t)); return None; }
            Feature::Well | Feature::Fountain => { self.say(Tone::Info, "Cold, clear water. You drink."); return Some(100); }
            Feature::Altar if self.place().map_or(false, |p| p.spec.kind == SiteKind::Town) => { self.say(Tone::Info, "The altar of the temple. The priest is near."); return None; }
            Feature::Altar if self.on_land() => {
                // The old stones: rest and mend a little in their ring.
                let h = self.hero.max_hp() / 4;
                self.hero.hp = (self.hero.hp + h).min(self.hero.max_hp());
                self.hero.poisoned = 0;
                self.say(Tone::Info, "You lay a hand on the old stone. It is warm, and the warmth runs into you.");
                return Some(300);
            }
            _ => {}
        }
        if !t.walkable() {
            match t.wall { Wall::None => {} _ => {} }
            return None;
        }
        self.x = nx;
        self.y = ny;
        let mut cost = self.hero.step_time() * if dx != 0 && dy != 0 { 14 } else { 10 } / 10;
        if matches!(t.ground, Ground::Shallows | Ground::Mud) { cost = cost * 3 / 2; }
        if matches!(t.feature, Feature::Web) { cost *= 2; self.say(Tone::Info, "You tear through a web."); if let Some(p) = self.place_mut() { p.floors[z].at_mut(nx, ny).feature = Feature::None; } }
        if let Feature::Trap { armed: true, damage } = t.feature {
            let d = damage;
            self.say(Tone::Hurt, "Click. Darts hiss from the walls!");
            self.hurt(d, "a dart trap");
            if let Some(p) = self.place_mut() { p.floors[z].at_mut(nx, ny).feature = Feature::Trap { armed: false, damage: d }; }
        }
        // Floors and the way out.
        match t.feature {
            Feature::StairsDown | Feature::LadderDown | Feature::Hole | Feature::Grate => { self.change_floor(1, &t.feature); }
            Feature::StairsUp | Feature::LadderUp => { self.change_floor(-1, &t.feature); }
            Feature::RopeSpot => {
                if self.hero.count("rope") > 0 { self.say(Tone::Info, "You climb your rope back up."); self.change_floor(-1, &t.feature); }
                else { self.say(Tone::Info, "The hole you fell through is above you. Without a rope there is no climbing it; find another way."); }
            }
            Feature::Exit => { self.come_out(); }
            Feature::Entrance { site, z } => { let g = self.global(nx, ny); self.go_in(site, z, g); }
            _ => {}
        }
        if self.on_land() { self.after_land_step(); }
        // Things lying here.
        if let Some(f) = self.floor() { if let Some(items) = f.items.get(&(self.x, self.y)) { if !items.is_empty() {
            let list: Vec<String> = items.iter().take(4).map(|i| i.describe()).collect();
            self.say(Tone::Loot, format!("Here lies {}{}. (G to take.)", list.join(", "), if items.len() > 4 { ", ..." } else { "" }));
        } } }
        self.look();
        Some(cost)
    }

    fn climb(&mut self) -> Option<i32> {
        let f = self.floor()?;
        let feat = f.at(self.x, self.y).feature.clone();
        match feat {
            Feature::StairsDown | Feature::LadderDown | Feature::Hole | Feature::Grate => { self.change_floor(1, &feat); Some(100) }
            Feature::StairsUp | Feature::LadderUp => { self.change_floor(-1, &feat); Some(100) }
            Feature::RopeSpot if self.hero.count("rope") > 0 => { self.say(Tone::Info, "You climb your rope back up."); self.change_floor(-1, &feat); Some(100) }
            Feature::RopeSpot => { self.say(Tone::Info, "Without a rope there is no climbing back up."); None }
            Feature::Exit => { self.come_out(); Some(100) }
            Feature::Entrance { site, z } if self.on_land() => { let g = self.global(self.x, self.y); self.go_in(site, z, g); Some(100) }
            _ => { self.say(Tone::Info, "There is no way up or down here."); None }
        }
    }

    fn change_floor(&mut self, dz: i32, how: &Feature) {
        let n = self.place().map_or(0, |p| p.floors.len());
        let top = self.place().map_or(0, |p| p.top);
        let nz = self.z as i32 + dz;
        // Up from the first floor walked: out onto the land.
        if dz < 0 && self.z == top && top >= 1 { self.come_out(); return; }
        if nz < 0 || nz as usize >= n { return; }
        self.z = nz as usize;
        let name = self.floor().map(|f| f.name.clone()).unwrap_or_default();
        let verb = match how { Feature::Hole => "You drop through the hole", Feature::Grate => "You lift the grate and climb down", Feature::LadderDown => "You climb down the ladder", Feature::LadderUp | Feature::RopeSpot => "You climb up", Feature::StairsUp => "You go up the stairs", _ => "You go down the stairs" };
        self.say(Tone::Info, format!("{} to {}.", verb, name));
        if dz > 0 { self.stats.floors_seen = self.stats.floors_seen.max(self.z as u32 + 1); }
        self.companion_follow(true);
        // Arriving: wherever one stands must be open (the matching way up or down is here).
        self.look();
    }

    fn open_chest(&mut self, x: i32, y: i32) {
        let z = self.z;
        let Some(p) = self.place_mut() else { return };
        let Feature::Chest { items, .. } = std::mem::replace(&mut p.floors[z].at_mut(x, y).feature, Feature::Chest { items: Vec::new(), opened: true, lock: 0, quest: 0 }) else { return };
        self.stats.chests += 1;
        if items.is_empty() { self.say(Tone::Info, "The chest is empty."); return; }
        let list: Vec<String> = items.iter().map(|i| i.describe()).collect();
        self.say(Tone::Loot, format!("You open the chest: {}.", list.join(", ")));
        for it in items { if it.id == "gold" { self.stats.gold_found += it.count; } stow(&mut self.hero.pack, it); }
    }

    fn open_sarcophagus(&mut self, x: i32, y: i32) {
        let z = self.z;
        let tier = self.place().map_or(1, |p| p.spec.tier);
        let mut r = self.roll(0x5A2C);
        let Some(p) = self.place_mut() else { return };
        let Feature::Sarcophagus { items, .. } = std::mem::replace(&mut p.floors[z].at_mut(x, y).feature, Feature::Sarcophagus { items: Vec::new(), opened: true }) else { return };
        self.say(Tone::Info, "You heave the lid aside.");
        for it in items { self.say(Tone::Loot, format!("Among the grave goods: {}.", it.describe())); stow(&mut self.hero.pack, it); }
        // The dead do not like it.
        if r.gen_bool(0.35) {
            let def = if tier >= 3 { "mummy" } else { "skeleton" };
            if let Some(p) = self.place_mut() {
                let uid = p.next_uid; p.next_uid += 1;
                let mut m = Monster::new(uid, def, x, y, z);
                m.awake = true;
                let (mx, my) = (x, y);
                // It rises beside its coffin.
                let f = &p.floors[z];
                let spot = DIRS8.iter().map(|(dx, dy)| (mx + dx, my + dy)).find(|&(a, b)| f.walkable(a, b));
                if let Some((a, b)) = spot { m.x = a; m.y = b; m.home = (a, b); p.monsters.push(m); }
            }
            self.say(Tone::Danger, format!("Something stirs in the sarcophagus: a {} rises!", if tier >= 3 { "mummy" } else { "skeleton" }));
        }
    }

    fn pick_up(&mut self) -> Option<i32> {
        let (x, y, z) = (self.x, self.y, self.z);
        let items = self.place_mut()?.floors[z].items.remove(&(x, y))?;
        if items.is_empty() { return None; }
        let list: Vec<String> = items.iter().map(|i| i.describe()).collect();
        self.say(Tone::Loot, format!("You take {}.", list.join(", ")));
        for it in items { if it.id == "gold" { self.stats.gold_found += it.count; } super::quest::on_found(self, &it); stow(&mut self.hero.pack, it); }
        Some(50)
    }

    fn use_item(&mut self, k: usize) -> Option<i32> {
        let it = self.hero.pack.get(k)?.clone();
        let d = it.def();
        match d.kind.as_str() {
            "potion" => {
                self.hero.spend(&it.id, 1);
                if d.heal > 0 { let before = self.hero.hp; self.hero.hp = (self.hero.hp + d.heal).min(self.hero.max_hp()); let g = self.hero.hp - before; self.effects.push(Effect::Number { x: self.x, y: self.y, z: self.z, value: g, tone: Tone::Level }); }
                if d.mana > 0 { self.hero.mana = (self.hero.mana + d.mana).min(self.hero.max_mana()); self.effects.push(Effect::Number { x: self.x, y: self.y, z: self.z, value: d.mana, tone: Tone::Info }); }
                self.say(Tone::Info, format!("You drink {}. Aaaah...", super::item::article(&d.name)));
                Some(60)
            }
            "food" => {
                if self.hero.fed > 6000 { self.say(Tone::Info, "You are full."); return None; }
                self.hero.spend(&it.id, 1);
                self.hero.fed += d.food;
                self.say(Tone::Info, format!("You eat {}. Munch.", super::item::article(&d.name)));
                Some(80)
            }
            "rune" => {
                let Some(spell) = d.spell.clone() else { return None };
                let r = self.cast_spell(&spell, None, true);
                if r.is_some() { self.hero.spend(&it.id, 1); }
                r
            }
            "map" => self.read_map(k),
            "tool" if it.id == "shovel" => self.dig(),
            "light" => {
                self.hero.spend(&it.id, 1);
                self.hero.torch += d.burn;
                self.say(Tone::Info, "You light a torch.");
                self.look();
                Some(50)
            }
            _ => match d.slot { Some(_) => { match self.hero.equip(k) { Ok(l) => { self.say(Tone::Info, l); Some(60) } Err(e) => { self.say(Tone::Info, e); None } } } None => { self.say(Tone::Info, format!("You look at {}: {}.", it.describe(), it.stats())); None } },
        }
    }

    fn rest(&mut self) -> bool {
        if self.here.is_none() { return false; }
        if self.hero.fed == 0 { self.say(Tone::Danger, "You are too hungry to recover. Eat something first."); return false; }
        let mut n = 0;
        while n < 60 && (self.hero.hp < self.hero.max_hp() || self.hero.mana < self.hero.max_mana()) {
            if self.monsters_in_sight() > 0 { if n == 0 { self.say(Tone::Danger, "You cannot rest with enemies near."); } break; }
            self.pass(100);
            n += 1;
            if self.hero.hp <= 0 || self.here.is_none() { break; }
        }
        if n > 0 { self.say(Tone::Info, format!("You rest a while ({} turns).", n)); }
        n > 0
    }

    pub fn monsters_in_sight(&self) -> usize {
        let Some(p) = self.place() else { return 0 };
        p.monsters.iter().filter(|m| m.z == self.z && m.hp > 0 && self.visible(m.x, m.y)).count()
    }

    // -----------------------------------------------------------------------------------------
    // Fighting

    fn find_monster(&self, uid: u32) -> Option<usize> { self.place()?.monsters.iter().position(|m| m.uid == uid && m.hp > 0) }

    fn attack(&mut self, uid: u32) -> Option<i32> {
        let k = self.find_monster(uid)?;
        let m = &self.place()?.monsters[k];
        let (mx, my) = (m.x, m.y);
        let adjacent = (mx - self.x).abs() <= 1 && (my - self.y).abs() <= 1 && m.z == self.z;
        let w = self.hero.weapon().cloned();
        let ranged = w.as_ref().map_or(false, |w| w.def().range > 0);
        if adjacent && !(ranged && w.as_ref().map_or(false, |w| w.def().ammo.is_some() || w.def().kind == "wand")) { return self.melee(uid); }
        if !ranged {
            // Out of reach: go after it (Tibia's chase), a step down the way to it.
            let f = self.floor()?;
            // (Never over a way in, out, up or down: a chase must not carry one off the floor.)
            let transit = |x: i32, y: i32| matches!(f.at(x, y).feature, Feature::Exit | Feature::Entrance { .. } | Feature::StairsDown | Feature::StairsUp | Feature::LadderDown | Feature::LadderUp | Feature::Hole | Feature::Grate | Feature::RopeSpot);
            let d = f.distances(mx, my, 40, |x, y| (f.at(x, y).walkable() && !transit(x, y)) || (x, y) == (self.x, self.y) || (x, y) == (mx, my));
            let cur = d.get(self.y as usize * f.w + self.x as usize).copied().unwrap_or(i32::MAX);
            let step = DIRS8.iter().copied().filter(|(dx, dy)| { let (nx, ny) = (self.x + dx, self.y + dy); f.inside(nx, ny) && d[ny as usize * f.w + nx as usize] < cur }).min_by_key(|(dx, dy)| d[(self.y + dy) as usize * f.w + (self.x + dx) as usize]);
            match step { Some((dx, dy)) => return self.step(dx, dy), None => { self.say(Tone::Info, "You cannot reach it from here."); return None; } }
        }
        self.shoot(uid)
    }

    /// Damage words by the share of the foe's life a blow took.
    fn wound_words(share: f32, skill: Skill) -> (&'static str, &'static str) {
        let verb = match skill { Skill::Sword => ["nick", "slash", "gash", "cleave through"], Skill::Axe => ["nick", "hack", "chop deep into", "split"], Skill::Club => ["bruise", "bash", "smash", "crush"], Skill::Distance => ["graze", "hit", "pierce", "skewer"], Skill::Magic => ["sear", "burn", "blast", "engulf"], _ => ["punch", "hit", "pummel", "batter"] };
        let mark = ["", "", ", tearing the muscle", ", and something breaks"];
        let k = if share < 0.12 { 0 } else if share < 0.3 { 1 } else if share < 0.55 { 2 } else { 3 };
        (verb[k], mark[k])
    }

    fn body_part(m: &Monster, r: &mut ChaCha8Rng) -> &'static str {
        let look = &m.def().look;
        let parts: &[&str] = if look.starts_with("folk") { &["head", "upper body", "lower body", "left arm", "right arm", "left leg", "right leg"] }
            else if look.contains("spider") { &["cephalothorax", "abdomen", "first leg", "third leg", "fang"] }
            else if look.contains("bat") || look.contains("dragon") { &["head", "body", "left wing", "right wing", "tail"] }
            else if look.contains("adder") || look.contains("serpent") { &["head", "body", "tail"] }
            else { &["head", "body", "foreleg", "hind leg", "tail"] };
        parts[r.gen_range(0..parts.len())]
    }

    fn melee(&mut self, uid: u32) -> Option<i32> {
        let k = self.find_monster(uid)?;
        let skill = self.hero.weapon_skill();
        let (attack, wname, holy, poison, burns) = match self.hero.weapon() {
            Some(w) if w.def().kind == "weapon" && w.def().ammo.is_none() => (w.attack(), w.short(), w.material.as_deref().and_then(|m| data().material(m)).map_or(false, |m| m.holy), w.def().poison, w.def().burns),
            Some(w) => (4, format!("the {}", w.short()), false, 0, false),
            None => (5, "your fists".into(), false, 0, false),
        };
        let sk = self.hero.skill(if skill == Skill::Magic { Skill::Fist } else { skill }) as f32;
        let level = self.hero.level as f32;
        let strength = (self.hero.strength as f32 / 1000.0).clamp(0.5, 2.0).powf(0.3);
        let max = (0.085 * sk * attack as f32 + level / 5.0) * strength;
        let mut r = self.roll(uid as u64 * 999 + 11);
        let m = self.place()?.monsters[k].clone();
        // (Every blow does some harm: a third of its best at least; misses come from the foe's defense.)
        let mut dmg = r.gen_range(max.max(1.0) / 3.0..=max.max(1.0)).round() as i32;
        let block = r.gen_range(0..=(m.defense() / 2).max(0));
        let soak = if m.armor() > 0 { r.gen_range(m.armor() / 2..=m.armor()) } else { 0 };
        if holy && m.def().undead { dmg = dmg * 3 / 2; }
        self.hero.train(skill, 1).map(|l| self.say(Tone::Level, format!("You advance to {} {}.", skill.word(), l)));
        let left = m.x < self.x;
        self.facing = ((m.x - self.x).signum(), (m.y - self.y).signum());
        let _ = left;
        if dmg <= block {
            self.say(Tone::Info, format!("You miss {}.", m.the()));
            self.effects.push(Effect::Puff { x: m.x, y: m.y, z: m.z });
            return Some(100);
        }
        let dmg = dmg - soak;
        if dmg <= 0 {
            self.say(Tone::Info, format!("Your blow glances off {}'s hide.", m.the()));
            self.effects.push(Effect::Puff { x: m.x, y: m.y, z: m.z });
            return Some(100);
        }
        let part = Self::body_part(&m, &mut r);
        let (verb, mark) = Self::wound_words(dmg as f32 / m.max_hp as f32, skill);
        let with = if wname == "your fists" { String::new() } else { format!(" with your {}", wname) };
        self.say(Tone::Hit, format!("You {} {} in the {}{}{}! ({})", verb, m.the(), part, with, mark, dmg));
        if poison > 0 { if let Some(p) = self.place_mut() { p.monsters[k].poisoned += poison * 3; } }
        if burns { if let Some(p) = self.place_mut() { p.monsters[k].poisoned += 4; } }
        self.damage_monster(k, dmg);
        Some(100)
    }

    fn shoot(&mut self, uid: u32) -> Option<i32> {
        let k = self.find_monster(uid)?;
        let m = self.place()?.monsters[k].clone();
        let w = self.hero.weapon()?.clone();
        let d = w.def();
        let range = d.range.max(1);
        if (m.x - self.x).abs().max((m.y - self.y).abs()) > range { self.say(Tone::Info, format!("{} is out of range ({} cells).", cap(&m.the()), range)); return None; }
        if !self.floor()?.clear_line((self.x, self.y), (m.x, m.y)) { self.say(Tone::Info, "Something is in the way."); return None; }
        let mut r = self.roll(uid as u64 * 31 + 7);
        let (attack, skill, kind, element) = if d.kind == "wand" {
            if self.hero.mana < d.mana { self.say(Tone::Info, "You have no mana for the wand."); return None; }
            self.hero.mana -= d.mana;
            (w.attack(), Skill::Magic, d.element.clone().unwrap_or("fire".into()), true)
        } else if let Some(ammo) = &d.ammo {
            if !self.hero.spend(ammo, 1) { self.say(Tone::Info, format!("You have no {}s.", ammo)); return None; }
            // Half the shafts can be gathered again where they fell.
            if r.gen_bool(0.5) { let (mx, my, z) = (m.x, m.y, self.z); let a = Item::new(ammo, 1); if let Some(p) = self.place_mut() { p.floors[z].drop_item(mx, my, a); } }
            (data().item(ammo).map_or(10, |a| a.attack), Skill::Distance, ammo.clone(), false)
        } else {
            // Thrown: the weapon flies and lands there.
            let thrown = w.clone();
            let mut one = thrown.clone(); one.count = 1;
            let slot = &mut self.hero.equipped[Slot::Hand as usize];
            if let Some(s) = slot { if s.count > 1 { s.count -= 1; } else { *slot = None; } }
            let (mx, my, z) = (m.x, m.y, self.z);
            if let Some(p) = self.place_mut() { p.floors[z].drop_item(mx, my, one); }
            (w.attack(), Skill::Distance, d.id.clone(), false)
        };
        self.effects.push(Effect::Missile { from: (self.x, self.y), to: (m.x, m.y), z: self.z, kind: kind.clone() });
        let sk = self.hero.skill(skill) as f32;
        let level = self.hero.level as f32;
        let dmg = if element {
            let max = attack as f32 * (1.0 + sk * 0.05);
            r.gen_range(max * 0.66..=max).round() as i32
        } else {
            let hit = (0.4 + sk * 0.012 + level * 0.004).min(0.95);
            if !r.gen_bool(hit as f64) { self.hero.train(skill, 1); self.say(Tone::Info, format!("Your {} misses {}.", kind, m.the())); return Some(100); }
            let max = 0.09 * sk * attack as f32 + level / 5.0;
            let soak = if m.armor() > 0 { r.gen_range(m.armor() / 2..=m.armor()) } else { 0 };
            r.gen_range(max * 0.3..=max.max(1.0)).round() as i32 - soak
        };
        if let Some(l) = self.hero.train(skill, 1) { self.say(Tone::Level, format!("You advance to {} {}.", skill.word(), l)); }
        if dmg <= 0 { self.say(Tone::Info, format!("Your {} glances off {}.", kind, m.the())); return Some(100); }
        let part = Self::body_part(&m, &mut r);
        let (verb, mark) = Self::wound_words(dmg as f32 / m.max_hp as f32, skill);
        self.say(Tone::Hit, format!("You {} {} in the {}{}! ({})", verb, m.the(), part, mark, dmg));
        self.damage_monster(k, dmg);
        Some(100)
    }

    /// Spells: a heal, a strike at a target, a wave before the caster, a ball, a ring of blows.
    fn cast(&mut self, k: usize, target: Option<u32>) -> Option<i32> {
        let id = self.hero.spells.get(k)?.clone();
        self.cast_spell(&id, target, false)
    }

    /// Cast spell `id` (from a rune when `free`: no level or mana asked, no magic learned).
    fn cast_spell(&mut self, id: &str, target: Option<u32>, free: bool) -> Option<i32> {
        let sp = data().spell(id)?;
        let k = 0usize;
        if !free && self.hero.level < sp.level { self.say(Tone::Info, format!("You need level {} for {}.", sp.level, sp.name)); return None; }
        if !free && self.hero.mana < sp.mana { self.say(Tone::Info, "You do not have enough mana."); return None; }
        let ml = self.hero.skill(Skill::Magic) as f32;
        let lvl = self.hero.level as f32;
        let mut r = self.roll(0x5BE11 + k as u64);
        let power = |p: f32, r: &mut ChaCha8Rng| (p * (1.0 + ml * 0.08 + lvl * 0.02) * r.gen_range(0.85..1.15)).round() as i32;
        let pick_target = |g: &Game, range: i32| -> Option<usize> {
            let p = g.place()?;
            if let Some(t) = target { if let Some(i) = p.monsters.iter().position(|m| m.uid == t && m.hp > 0 && m.z == g.z) { return Some(i); } }
            p.monsters.iter().enumerate().filter(|(_, m)| m.z == g.z && m.hp > 0 && g.visible(m.x, m.y) && (m.x - g.x).abs().max((m.y - g.y).abs()) <= range)
                .min_by_key(|(_, m)| (m.x - g.x).abs() + (m.y - g.y).abs()).map(|(i, _)| i)
        };
        let element = sp.element.clone().unwrap_or_else(|| "energy".into());
        match sp.kind.as_str() {
            "heal" => {
                let h = power(sp.power, &mut r);
                let before = self.hero.hp;
                self.hero.hp = (self.hero.hp + h).min(self.hero.max_hp());
                self.effects.push(Effect::Number { x: self.x, y: self.y, z: self.z, value: self.hero.hp - before, tone: Tone::Level });
                self.effects.push(Effect::Area { cells: vec![(self.x, self.y)], z: self.z, kind: "heal".into() });
            }
            "light" => { self.hero.glow = 2000; self.look(); }
            "haste" => { self.hero.hasted = 800; }
            "strike" | "strike_distance" | "strike_melee" => {
                let range = if sp.kind == "strike_melee" { 1 } else { sp.range.max(1) };
                let Some(i) = pick_target(self, range) else { self.say(Tone::Info, "There is nothing to strike."); return None };
                let m = self.place()?.monsters[i].clone();
                if !self.floor()?.clear_line((self.x, self.y), (m.x, m.y)) { self.say(Tone::Info, "Something is in the way."); return None; }
                let dmg = if sp.kind == "strike" {
                    let mut d = power(sp.power, &mut r);
                    if element == "holy" && m.def().undead { d *= 2; }
                    d
                } else {
                    // A weapon blow made stronger.
                    let attack = self.hero.weapon().map_or(5, |w| w.attack().max(10));
                    let sk = self.hero.skill(if sp.kind == "strike_distance" { Skill::Distance } else { self.hero.weapon_skill() }) as f32;
                    ((0.085 * sk * attack as f32 + lvl / 5.0) * sp.power * r.gen_range(0.6..1.0)).round() as i32 - m.armor() / 2
                };
                self.effects.push(Effect::Missile { from: (self.x, self.y), to: (m.x, m.y), z: self.z, kind: element.clone() });
                self.effects.push(Effect::Area { cells: vec![(m.x, m.y)], z: self.z, kind: element.clone() });
                self.say(Tone::Hit, format!("\"{}!\" {} strikes {}. ({})", cap(&sp.words), sp.name, m.the(), dmg.max(0)));
                if dmg > 0 { self.damage_monster(i, dmg); }
            }
            "around" | "ball" | "wave" => {
                let cells: Vec<(i32, i32)> = match sp.kind.as_str() {
                    "around" => DIRS8.iter().map(|(dx, dy)| (self.x + dx, self.y + dy)).collect(),
                    "ball" => {
                        let Some(i) = pick_target(self, sp.range.max(1)) else { self.say(Tone::Info, "There is nothing to strike."); return None };
                        let m = &self.place()?.monsters[i];
                        let c = (m.x, m.y);
                        (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (c.0 + dx, c.1 + dy))).collect()
                    }
                    _ => {
                        let (fx, fy) = self.facing;
                        let (fx, fy) = if (fx, fy) == (0, 0) { (0, 1) } else { (fx, fy) };
                        let mut v = Vec::new();
                        for d in 1..=sp.range.max(1) { v.push((self.x + fx * d, self.y + fy * d)); if d >= 2 { v.push((self.x + fx * d + fy, self.y + fy * d + fx)); v.push((self.x + fx * d - fy, self.y + fy * d - fx)); } }
                        v
                    }
                };
                let f = self.floor()?;
                let cells: Vec<(i32, i32)> = cells.into_iter().filter(|&(x, y)| f.inside(x, y) && f.at(x, y).wall == Wall::None).collect();
                self.effects.push(Effect::Area { cells: cells.clone(), z: self.z, kind: if sp.kind == "around" { "blow".into() } else { element.clone() } });
                // (By uid: a monster slain leaves the list and moves the others' places in it.)
                let hit: Vec<u32> = self.place()?.monsters.iter().filter(|m| m.z == self.z && m.hp > 0 && cells.contains(&(m.x, m.y))).map(|m| m.uid).collect();
                self.say(Tone::Hit, format!("\"{}!\"", cap(&sp.words)));
                for uid in hit {
                    let Some(i) = self.find_monster(uid) else { continue };
                    let m = self.place()?.monsters[i].clone();
                    if m.hp <= 0 { continue; }
                    let dmg = if sp.kind == "around" {
                        let attack = self.hero.weapon().map_or(5, |w| w.attack().max(10));
                        let sk = self.hero.skill(self.hero.weapon_skill()) as f32;
                        ((0.085 * sk * attack as f32 + lvl / 5.0) * sp.power * r.gen_range(0.5..1.0)).round() as i32 - m.armor() / 2
                    } else { power(sp.power, &mut r) };
                    if dmg > 0 { self.say(Tone::Hit, format!("{} is struck. ({})", cap(&m.the()), dmg)); self.damage_monster(i, dmg); }
                }
            }
            _ => {}
        }
        if !free {
            self.hero.mana -= sp.mana;
            if let Some(l) = self.hero.train(Skill::Magic, sp.mana as u32) { self.say(Tone::Level, format!("You advance to magic level {}.", l)); }
        }
        Some(100)
    }

    fn choose(&mut self, k: usize) -> Option<i32> {
        let (x, y, z) = (self.x, self.y, self.z);
        let here = self.here?;
        let p = self.place_mut()?;
        let f = &mut p.floors[z];
        let spot = DIRS8.iter().map(|(dx, dy)| (x + dx, y + dy)).find(|&(a, b)| matches!(f.at(a, b).feature, Feature::QuestChest { .. }))?;
        let Feature::QuestChest { choices, taken, quest } = &mut f.at_mut(spot.0, spot.1).feature else { return None };
        if *taken { return None; }
        let it = choices.get(k)?.clone();
        *taken = true;
        let q = *quest;
        self.chosen.push(here);
        self.say(Tone::Quest, format!("You take {} from the iron-bound chest. The rest you leave; the chest closes.", it.describe()));
        let _ = q;
        super::quest::on_found(self, &it);
        stow(&mut self.hero.pack, it);
        self.stats.chests += 1;
        Some(100)
    }

    fn damage_monster(&mut self, k: usize, dmg: i32) {
        let turn = self.turn;
        let here = self.here.unwrap_or(0);
        let Some(p) = self.place_mut() else { return };
        let m = &mut p.monsters[k];
        m.hp -= dmg;
        m.awake = true;
        let (x, y, z) = (m.x, m.y, m.z);
        self.effects.push(Effect::Number { x, y, z, value: dmg, tone: Tone::Hit });
        if self.place().unwrap().monsters[k].hp > 0 { return; }
        // Slain.
        let m = self.place().unwrap().monsters[k].clone();
        let xp = m.xp();
        let mut r = self.roll(m.uid as u64 ^ 0x100D);
        let mut loot: Vec<Item> = Vec::new();
        for (id, chance, lo, hi) in &m.def().loot {
            if r.gen_range(0..1000) < *chance { stow(&mut loot, Item::new(id, r.gen_range(*lo..=*hi))); }
        }
        // Gear from the loot table gets a material and quality by the place's tier.
        let tier = self.place().map_or(1, |p| p.spec.tier);
        for it in loot.iter_mut() {
            let d = it.def();
            if matches!(d.kind.as_str(), "weapon" | "armour" | "shield") && !d.stack && r.gen_bool(0.3) { it.quality = r.gen_range(0..=tier.min(4)) as u8; }
        }
        for it in m.carries.iter().cloned() { stow(&mut loot, it); }
        // A treasure map marks somewhere near.
        for k in 0..loot.len() { if loot[k].id == "treasure_map" && loot[k].tag == 0 { loot[k].tag = self.map_target(); } }
        let name = m.the();
        let p = self.place_mut().unwrap();
        // (Not on a stair, a hole or the way out: stepping there to take it would carry one off.)
        let transit = |f: &super::map::Floor, x: i32, y: i32| matches!(f.at(x, y).feature, Feature::StairsDown | Feature::StairsUp | Feature::LadderDown | Feature::LadderUp | Feature::Hole | Feature::RopeSpot | Feature::Exit | Feature::Grate);
        let (lx, ly) = if transit(&p.floors[z], x, y) {
            let f = &p.floors[z];
            DIRS8.iter().map(|(dx, dy)| (x + dx, y + dy)).find(|&(a, b)| f.walkable(a, b) && !transit(f, a, b)).unwrap_or((x, y))
        } else { (x, y) };
        for it in loot.iter().cloned() { p.floors[z].drop_item(lx, ly, it); }
        let dead = p.monsters.remove(k);
        self.corpses.entry(here).or_default().push(Corpse { x, y, z, def: dead.def.clone(), name: dead.name.clone(), turn });
        if !dead.boss && here != LAND { self.respawn.push((here, dead.clone(), turn)); }
        self.stats.kills += 1;
        self.hero.kills += 1;
        if dead.boss {
            self.stats.bosses += 1; self.slain.push(dead.name.clone());
            let place = self.place().map(|p| p.spec.name.clone()).unwrap_or_default(); let lvl = self.hero.level;
            self.deeds.push((turn, format!("slew {} in {} (level {})", dead.name, place, lvl)));
            let creature = self.place().and_then(|p| p.spec.creature).filter(|_| dead.legend.is_some());
            let who = self.hero.name.clone();
            let kind = match creature { Some(c) => super::living::DeedKind::BeastSlain(c), None => super::living::DeedKind::BossSlain };
            self.chronicle(kind, format!("{} slew {}", who, dead.name), format!("{} slew {} in {}.", who, dead.name, place));
        }
        if dead.boss && self.companion.is_some() {
            if let Some(c) = self.companion.as_mut() { c.morale = (c.morale + 25).min(100); }
            let line = self.companion.as_ref().unwrap().line("won");
            self.say(Tone::Talk, line);
        }
        if dead.boss && self.place().map_or(false, |p| p.spec.kind == SiteKind::DarkFortress) && !self.victory {
            self.victory = true;
            let who = self.hero.name.clone();
            self.banner = Some(("The Shadow is broken.".into(), format!("{} has slain {} in the seat of its power. The land will tell of it for an age. (You may go on.)", who, dead.name)));
            self.say(Tone::Quest, format!("{} has broken the Shadow.", who));
            let t = self.turn;
            self.deeds.push((t, format!("broke the Shadow, slaying {} in its seat", dead.name)));
            self.chronicle(super::living::DeedKind::ShadowBroken, format!("{} broke the Shadow", who), format!("{} slew {} in the seat of its power and broke the Shadow.", who, dead.name));
        }
        let lead = if dead.boss { format!("{} falls, and does not rise.", cap(&name)) } else { format!("You slay {}.", name) };
        self.say(if dead.boss { Tone::Quest } else { Tone::Hit }, format!("{} ({} experience)", lead, xp));
        if !loot.is_empty() {
            let list: Vec<String> = loot.iter().map(|i| i.describe()).collect();
            self.say(Tone::Loot, format!("Loot of {}: {}.", dead.name, list.join(", ")));
        }
        for l in self.hero.gain_xp(xp as u64) {
            self.say(Tone::Level, format!("You advanced from level {} to level {}.{}", l - 1, l, if l == 8 && self.hero.calling.is_none() { " Go to a temple: the priest will give you a calling." } else { "" }));
            if l % 10 == 0 { let t = self.turn; self.deeds.push((t, format!("reached level {}", l))); }
        }
        super::quest::on_kill(self, &dead);
    }

    /// The adventurer takes `dmg` from `what`.
    pub fn hurt(&mut self, dmg: i32, what: &str) {
        if dmg <= 0 { return; }
        self.hero.hp -= dmg;
        self.effects.push(Effect::Number { x: self.x, y: self.y, z: self.z, value: dmg, tone: Tone::Hurt });
        if self.hero.hp <= 0 { self.die(what); }
    }

    fn die(&mut self, what: &str) {
        self.stats.deaths += 1;
        self.hero.deaths += 1;
        let blessed = std::mem::replace(&mut self.hero.blessed, false);
        let lost_xp = if blessed { 0 } else { self.hero.xp / 10 };
        let gold = if blessed { 0 } else { self.hero.gold() / 2 };
        if blessed { self.say(Tone::Level, "The god's blessing takes the blow: you lose nothing, but the blessing is spent."); }
        self.hero.take_gold(gold);
        let (x, y, z) = (self.x, self.y, self.z);
        if gold > 0 { if let Some(p) = self.place_mut() { p.floors[z].drop_item(x, y, Item::new("gold", gold)); } }
        self.hero.xp -= lost_xp;
        while self.hero.level > 1 && self.hero.xp < super::hero::xp_for(self.hero.level) { self.hero.level -= 1; }
        let place = self.place().map(|p| p.spec.name.clone()).unwrap_or_default();
        let t = self.turn;
        self.deeds.push((t, format!("was slain by {} in {}", what, place)));
        let who = self.hero.name.clone();
        self.chronicle(super::living::DeedKind::Fell, format!("{} was struck down by {}", who, what), format!("{} fell to {} in {}, and woke in the temple.", who, what, place));
        self.say(Tone::Death, format!("You are dead, slain by {} in {}. You lose {} experience{}.", what, place, lost_xp, if gold > 0 { format!(" and drop {} gold where you fell", gold) } else { String::new() }));
        self.banner = Some(("You are dead.".into(), format!("Slain by {}. You wake in the temple, poorer and wiser.", what)));
        self.hero.hp = self.hero.max_hp();
        self.hero.mana = self.hero.max_mana();
        self.hero.poisoned = 0;
        self.talk = None;
        self.wake_at_temple();
    }

    // -----------------------------------------------------------------------------------------
    // Time

    /// `cost` ticks pass: the adventurer's body, then every creature on this floor acts as its
    /// speed allows.
    pub fn pass(&mut self, cost: i32) {
        let before = self.turn / 100;
        self.turn += cost.max(1) as u64;
        let ticks = self.turn / 100 - before;
        for _ in 0..ticks {
            let h = &mut self.hero;
            if h.fed > 0 { h.fed -= 1; }
            if h.torch > 0 { h.torch -= 1; if h.torch == 0 { self.say(Tone::Info, "Your torch gutters out."); } }
            let h = &mut self.hero;
            if h.glow > 0 { h.glow -= 1; }
            if h.hasted > 0 { h.hasted -= 1; }
            if h.fed > 0 && self.turn / 100 % 3 == 0 {
                h.hp = (h.hp + 1 + h.level as i32 / 6).min(h.max_hp());
                h.mana = (h.mana + 1 + h.level as i32 / 4 + h.skill(Skill::Magic) as i32 / 3).min(h.max_mana());
            }
            if h.poisoned > 0 { h.poisoned -= 1; let d = 1 + h.poisoned / 8; self.hurt(d, "poison"); if self.hero.poisoned == 0 { self.say(Tone::Info, "The poison has run its course."); } }
            if self.hero.fed == 0 && self.turn / 100 % 50 == 0 { self.say(Tone::Danger, "You are hungry, and you will not heal until you eat (F eats, or click food in the pack)."); }
        }
        if self.here.is_some() { self.monsters_act(cost); }
        if before * 100 / super::land::DAY != self.turn / super::land::DAY { self.companion_day(); }
        self.land_tick(before * 100);
        self.corpses.values_mut().for_each(|v| v.retain(|c| self.turn < c.turn + 3000));
    }

    /// A day on the road with a companion: their heart mends a little; the grasping ones want
    /// their wage every ten days; one whose heart is gone leaves.
    fn companion_day(&mut self) {
        let Some(c) = self.companion.clone() else { return };
        let day = self.turn / super::land::DAY;
        let t = c.temper();
        let mut morale = (c.morale + if matches!(t, super::people::Temper::Kind | super::people::Temper::Cheerful) { 10 } else { 5 }).min(100);
        let mut paid = c.paid_day;
        if t == super::people::Temper::Greedy && day >= c.paid_day + 10 {
            let wage = 8 * self.hero.level.max(1);
            if self.hero.take_gold(wage) { paid = day; self.say(Tone::Talk, format!("{} counts out {} gold of wages and pockets it.", c.name, wage)); }
            else { morale -= 50; let l = c.line("pay"); self.say(Tone::Talk, l); }
        }
        if let Some(cc) = self.companion.as_mut() { cc.morale = morale; cc.paid_day = paid; }
        if morale <= 0 { let l = c.line("leave"); self.say(Tone::Danger, l); self.say(Tone::Info, format!("{} leaves you.", c.name)); self.companion = None; }
    }

    fn monsters_act(&mut self, cost: i32) {
        let Some(id) = self.here else { return };
        let z = self.z;
        let (hx, hy) = (self.x, self.y);
        let dist = {
            let p = &self.pl(id);
            let f = &p.floors[z];
            f.distances(hx, hy, 30, |x, y| f.at(x, y).walkable() || matches!(f.at(x, y).feature, Feature::Door { lock: 0, .. }))
        };
        self.companion_act(id, cost, &dist);
        let n = self.pl(id).monsters.len();
        let mut k = 0;
        while k < n.min(self.pl(id).monsters.len()) {
            if self.here != Some(id) || self.z != z { break; }
            let m = &self.pl(id).monsters[k];
            if m.z != z || m.hp <= 0 || (id == LAND && (m.x - hx).abs().max((m.y - hy).abs()) > 40) { k += 1; continue; }
            let speed = m.speed();
            {
                let m = &mut self.pl_mut(id).monsters[k];
                m.energy += speed * cost / 100;
                if m.poisoned > 0 { m.poisoned -= 1; m.hp -= 1 + m.poisoned / 6; }
                let regen = m.def().regen;
                if regen > 0 && m.hp < m.max_hp { m.hp = (m.hp + regen).min(m.max_hp); }
            }
            let mut acts = 0;
            while self.pl(id).monsters.get(k).map_or(false, |m| m.energy >= 100) && acts < 4 {
                self.pl_mut(id).monsters[k].energy -= 100;
                self.monster_turn(id, k, &dist);
                acts += 1;
                if self.here != Some(id) || self.z != z { return; }
            }
            k += 1;
        }
        // Poison may have killed something.
        let dead: Vec<usize> = self.pl(id).monsters.iter().enumerate().filter(|(_, m)| m.hp <= 0).map(|(i, _)| i).collect();
        for i in dead.into_iter().rev() { let hp = self.pl(id).monsters[i].hp; self.pl_mut(id).monsters[i].hp = 1; self.damage_monster(i, 1 - hp); }
    }

    fn monster_turn(&mut self, id: u32, k: usize, dist: &[i32]) {
        let (hx, hy, z) = (self.x, self.y, self.z);
        let m = self.pl(id).monsters[k].clone();
        let (dx, sees, outdoor) = {
            let f = &self.pl(id).floors[z];
            let dx = (hx - m.x).abs().max((hy - m.y).abs());
            (dx, dx <= 8 && f.clear_line((m.x, m.y), (hx, hy)), f.outdoor)
        };
        let d = m.def();
        if !m.awake {
            if sees && (dx <= 5 || outdoor) {
                self.pl_mut(id).monsters[k].awake = true;
                if !d.sounds.is_empty() && self.rng.gen_bool(0.5) { let s = d.sounds[self.rng.gen_range(0..d.sounds.len())].clone(); self.effects.push(Effect::Speech { x: m.x, y: m.y, z, text: s }); }
            } else if self.rng.gen_bool(0.25) {
                // Wander a little about home.
                let (ddx, ddy) = DIRS8[self.rng.gen_range(0..8)];
                let (nx, ny) = (m.x + ddx, m.y + ddy);
                let ok = self.pl(id).floors[z].walkable(nx, ny) && (nx - m.home.0).abs() + (ny - m.home.1).abs() < 6 && self.free(id, nx, ny);
                if ok { let mm = &mut self.pl_mut(id).monsters[k]; mm.x = nx; mm.y = ny; mm.left = ddx < 0 || (ddx == 0 && mm.left); }
            }
            return;
        }
        // Lost the scent: go home.
        if dx > 14 && !m.boss { self.pl_mut(id).monsters[k].awake = false; return; }
        let coward = d.ai == "coward" || (!m.boss && m.hp < m.max_hp / 6 && d.ai != "slow" && !d.undead);
        // Casters heal themselves when hurt.
        if d.heals > 0 && m.hp < m.max_hp / 2 && self.rng.gen_bool(0.3) {
            let mm = &mut self.pl_mut(id).monsters[k];
            mm.hp = (mm.hp + d.heals).min(mm.max_hp);
            self.effects.push(Effect::Area { cells: vec![(m.x, m.y)], z, kind: "heal".into() });
            return;
        }
        // Ranged: shoot when in range and in sight.
        if let Some((range, dmg, kind)) = &d.range {
            if sees && dx <= *range && dx >= 2 && self.rng.gen_bool(if m.boss { 0.6 } else { 0.5 }) {
                let mut r = self.roll(m.uid as u64 ^ 0xA1);
                let raw = r.gen_range(0..=((*dmg as f32 * m.scale) as i32).max(1));
                let elemental = matches!(kind.as_str(), "fire" | "poison" | "dark" | "energy" | "ice");
                let soak = if elemental { 0 } else { r.gen_range(0..=self.hero.armor().max(0)) };
                self.effects.push(Effect::Missile { from: (m.x, m.y), to: (hx, hy), z, kind: kind.clone() });
                if elemental { self.effects.push(Effect::Area { cells: vec![(hx, hy)], z, kind: kind.clone() }); }
                let dmgv = raw - soak;
                let what = match kind.as_str() { "fire" => "breathes fire at you", "poison" => "spits venom at you", "dark" => "hurls a bolt of darkness at you", "spear" => "throws a spear at you", _ => "shoots at you" };
                if dmgv > 0 {
                    self.say(Tone::Hurt, format!("{} {}. ({})", cap(&m.the()), what, dmgv));
                    if kind == "poison" { self.hero.poisoned += 6; }
                    self.hurt(dmgv, &m.a());
                } else { self.say(Tone::Info, format!("{} {}, and misses.", cap(&m.the()), what)); }
                return;
            }
        }
        // Adjacent: strike.
        if dx <= 1 && !coward {
            self.monster_strikes(id, k);
            return;
        }
        // Beside the companion (and not the adventurer): strike them.
        if let Some(c) = self.companion.clone() { if (c.x - m.x).abs() <= 1 && (c.y - m.y).abs() <= 1 && !coward {
            let mut r = self.roll(m.uid as u64 ^ 0xC0A);
            let raw = r.gen_range(0..=m.attack().max(1));
            let lvl = self.hero.level;
            let block = r.gen_range(0..=c.defense(lvl));
            let soak = r.gen_range(c.armor(lvl) / 2..=c.armor(lvl).max(1));
            let dmg = if raw <= block / 2 { 0 } else { raw - soak };
            self.pl_mut(id).monsters[k].struck_at = self.turn;
            if dmg > 0 {
                self.effects.push(Effect::Number { x: c.x, y: c.y, z, value: dmg, tone: Tone::Hurt });
                let dead = { let cc = self.companion.as_mut().unwrap(); cc.hp -= dmg; cc.hp <= 0 };
                if !dead {
                    let low = self.companion.as_ref().map_or(false, |cc| cc.hp * 4 < cc.max_hp);
                    if low && self.rng.gen_bool(0.25) {
                        let timid = self.companion.as_ref().map_or(false, |cc| cc.temper() == super::people::Temper::Timid);
                        if let Some(cc) = self.companion.as_mut() { cc.morale -= if timid { 25 } else { 12 }; }
                        let line = self.companion.as_ref().unwrap().line("hurt");
                        self.say(Tone::Talk, line);
                    }
                }
                if dead {
                    let name = c.name.clone();
                    self.say(Tone::Death, format!("{} falls to {}. You fight on alone.", name, m.the()));
                    self.corpses.entry(id).or_default().push(Corpse { x: c.x, y: c.y, z, def: "bandit".into(), name, turn: self.turn });
                    self.companion = None;
                }
            }
            return;
        } }
        // Step toward (or away from) the adventurer along the distance map.
        let (best, cur, door) = {
            let f = &self.pl(id).floors[z];
            let mut best: Option<(i32, i32, i32)> = None;
            for (ddx, ddy) in DIRS8 {
                let (nx, ny) = (m.x + ddx, m.y + ddy);
                if !f.inside(nx, ny) || (nx, ny) == (hx, hy) { continue; }
                let t = f.at(nx, ny);
                if !t.walkable() && !matches!(t.feature, Feature::Door { lock: 0, .. }) { continue; }
                if ddx != 0 && ddy != 0 && !f.at(m.x + ddx, m.y).walkable() && !f.at(m.x, m.y + ddy).walkable() { continue; }
                if !self.free(id, nx, ny) { continue; }
                let dd = dist[ny as usize * f.w + nx as usize];
                if dd == i32::MAX { continue; }
                let score = if coward { -dd } else { dd };
                if best.map_or(true, |b| score < b.2) { best = Some((nx, ny, score)); }
            }
            let cur = dist[m.y as usize * f.w + m.x as usize];
            let door = best.map_or(false, |(nx, ny, _)| matches!(f.at(nx, ny).feature, Feature::Door { open: false, .. }));
            (best, cur, door)
        };
        // Cornered: a coward with nowhere to run turns and fights.
        let fled = best.map_or(false, |(_, _, s)| coward && s < -cur.min(10_000));
        if coward && !fled && dx <= 1 { self.monster_strikes(id, k); return; }
        if let Some((nx, ny, s)) = best {
            if (!coward && (cur == i32::MAX || s < cur)) || (coward && s < -cur.min(10_000)) {
                let p = self.pl_mut(id);
                // Open a door on the way.
                if door { p.floors[z].at_mut(nx, ny).feature = Feature::Door { open: true, lock: 0 }; return; }
                let mm = &mut p.monsters[k];
                mm.left = nx < mm.x || (nx == mm.x && mm.left);
                mm.x = nx; mm.y = ny;
            }
        }
    }

    fn free(&self, id: u32, x: i32, y: i32) -> bool {
        !(x == self.x && y == self.y) && !self.companion.as_ref().map_or(false, |c| (c.x, c.y) == (x, y)) && !self.pl(id).monsters.iter().any(|m| m.z == self.z && m.x == x && m.y == y && m.hp > 0)
            && !self.pl(id).npcs.iter().any(|n| n.z == self.z && n.x == x && n.y == y)
    }

    fn companion_act(&mut self, id: u32, cost: i32, dist: &[i32]) {
        let Some(mut c) = self.companion.clone() else { return };
        let z = self.z;
        c.energy += cost;
        let lvl = self.hero.level;
        c.max_hp = 50 + 12 * lvl as i32;
        while c.energy >= 100 {
            c.energy -= 100;
            // A slow mend.
            if self.turn / 100 % 4 == 0 { c.hp = (c.hp + 1 + lvl as i32 / 8).min(c.max_hp); }
            let p = &self.pl(id);
            let f = &p.floors[z];
            // Strike what is beside them (the weakest first).
            let beside = p.monsters.iter().enumerate().filter(|(_, m)| m.z == z && m.hp > 0 && (m.x - c.x).abs() <= 1 && (m.y - c.y).abs() <= 1).min_by_key(|(_, m)| m.hp).map(|(i, m)| (i, m.clone()));
            if let Some((i, m)) = beside {
                let mut r = self.roll(m.uid as u64 ^ 0xC0B);
                let raw = r.gen_range(0..=c.attack(lvl));
                let block = r.gen_range(0..=(m.defense() / 2).max(0));
                let soak = if m.armor() > 0 { r.gen_range(m.armor() / 2..=m.armor()) } else { 0 };
                c.left = m.x < c.x;
                c.struck_at = self.turn;
                if raw > block && raw - soak > 0 {
                    let d = raw - soak;
                    if self.rng.gen_bool(0.35) { self.say(Tone::Hit, format!("{} strikes {}. ({})", c.name, m.the(), d)); }
                    let before = self.stats.kills;
                    self.companion = Some(c.clone());
                    self.damage_monster(i, d);
                    if let Some(cc) = self.companion.as_mut() { if self.stats.kills > before { cc.kills += 1; } c = cc.clone(); }
                }
                continue;
            }
            // Else keep close to the adventurer, or go at what is near and awake.
            let foe = p.monsters.iter().filter(|m| m.z == z && m.hp > 0 && m.awake && (m.x - c.x).abs().max((m.y - c.y).abs()) <= 5 && self.visible(m.x, m.y)).min_by_key(|m| (m.x - c.x).abs() + (m.y - c.y).abs()).map(|m| (m.x, m.y));
            let (tx, ty, near) = match foe { Some((x, y)) => (x, y, 1), None => (self.x, self.y, 2) };
            if (tx - c.x).abs().max((ty - c.y).abs()) <= near { continue; }
            let occupied = |x: i32, y: i32| (x, y) == (self.x, self.y) || p.monsters.iter().any(|m| m.z == z && m.x == x && m.y == y && m.hp > 0) || p.npcs.iter().any(|n| n.z == z && n.x == x && n.y == y);
            let step = if foe.is_none() {
                // Down the adventurer's distance map.
                DIRS8.iter().map(|(dx, dy)| (c.x + dx, c.y + dy)).filter(|&(x, y)| f.inside(x, y) && f.at(x, y).walkable() && !occupied(x, y)).min_by_key(|&(x, y)| dist[y as usize * f.w + x as usize])
            } else {
                DIRS8.iter().map(|(dx, dy)| (c.x + dx, c.y + dy)).filter(|&(x, y)| f.inside(x, y) && f.at(x, y).walkable() && !occupied(x, y)).min_by_key(|&(x, y)| (x - tx).abs().max((y - ty).abs()))
            };
            if let Some((x, y)) = step { c.left = x < c.x || (x == c.x && c.left); c.x = x; c.y = y; }
        }
        if self.companion.is_some() { self.companion = Some(c); }
        self.companion_follow(false);
    }

    fn monster_strikes(&mut self, id: u32, k: usize) {
        let m = self.pl(id).monsters[k].clone();
        let d = m.def();
        let mut r = self.roll(m.uid as u64 * 11 + self.hero.level as u64);
        self.pl_mut(id).monsters[k].struck_at = self.turn;
        self.pl_mut(id).monsters[k].left = self.x < m.x;
        // A beast of the history uses its own attack now and then.
        if let Some(legend) = &m.legend { if let Some(att) = &legend.attack { if r.gen_bool(0.2) {
            let dmg = (m.attack() as f32 * (1.0 + att.deadly)).round() as i32;
            let dmg = r.gen_range(dmg / 2..=dmg.max(1));
            self.say(Tone::Hurt, format!("{} {} ({})", cap(&m.the()), att.did_to("you"), dmg));
            self.effects.push(Effect::Area { cells: vec![(self.x, self.y)], z: self.z, kind: "fire".into() });
            self.hurt(dmg, &m.a());
            return;
        } } }
        let raw = r.gen_range(0..=m.attack().max(1));
        let block = r.gen_range(0..=self.hero.defense().max(0));
        if let Some(l) = self.hero.train(Skill::Shielding, 1) { self.say(Tone::Level, format!("You advance to shielding {}.", l)); }
        if raw <= block / 2 {
            if self.rng.gen_bool(0.3) { self.say(Tone::Info, format!("You block {}'s attack.", m.the())); }
            self.effects.push(Effect::Puff { x: self.x, y: self.y, z: self.z });
            return;
        }
        let armor = self.hero.armor();
        let soak = if armor > 0 { r.gen_range(armor / 2..=armor) } else { 0 };
        let dmg = raw - soak;
        if dmg <= 0 { self.effects.push(Effect::Puff { x: self.x, y: self.y, z: self.z }); return; }
        let how = match d.look.as_str() { l if l.starts_with("folk") => "hits you", "rat" | "giant rat" | "wolf" | "bear" | "wild boar" => "bites you", "spider" | "giant spider" => "bites you with its fangs", "bat" => "bites you", "adder" => "strikes at you", _ => "hits you" };
        let t = if dmg * 3 > self.hero.max_hp() { Tone::Danger } else { Tone::Hurt };
        self.say(t, format!("{} {}. ({})", cap(&m.the()), how, dmg));
        if d.poison > 0 && r.gen_bool(0.4) && !self.hero.equipped.iter().flatten().any(|i| i.def().resist.as_deref() == Some("poison")) { self.hero.poisoned += d.poison; self.say(Tone::Hurt, "You are poisoned."); }
        if d.lifesteal { let p = self.pl_mut(id); let mm = &mut p.monsters[k]; mm.hp = (mm.hp + dmg / 2).min(mm.max_hp); }
        self.hurt(dmg, &m.a());
    }

    // -----------------------------------------------------------------------------------------
    // The world map

    fn travel(&mut self, dx: i32, dy: i32) -> bool {
        if self.here.is_some() { return false; }
        let (nx, ny) = (self.tile.0 as i32 + dx, self.tile.1 as i32 + dy);
        if ny < 0 || ny >= self.world.h as i32 { return false; }
        let nx = nx.rem_euclid(self.world.w as i32) as usize;
        let ny = ny as usize;
        let k = ny * self.world.w + nx;
        if !self.world.land[k] { self.say(Tone::Info, "The sea. Without a ship there is no going on."); return false; }
        // Quick on a road, slower over country one has mapped, slow and lost over blank country
        // (the Mapmaker's map is worth having).
        let blank = self.mapped.get(k).copied().unwrap_or(0) == 0;
        let time = if self.world.road[k] { 1600 } else if blank { 3600 } else { 2400 };
        self.tile = (nx, ny);
        self.facing = (dx, dy);
        self.turn += time;
        self.hero.fed = (self.hero.fed - time as i32 / 100).max(0);
        // Healing on the road.
        self.hero.hp = (self.hero.hp + 6 + self.hero.level as i32).min(self.hero.max_hp());
        self.hero.mana = (self.hero.mana + 8 + self.hero.level as i32).min(self.hero.max_mana());
        if self.hero.fed == 0 { self.hurt(3, "hunger"); }
        if self.route.last() != Some(&(nx as u16, ny as u16)) { self.route.push((nx as u16, ny as u16)); }
        self.ink((nx, ny), false);
        // What is on this tile.
        let here: Vec<u32> = self.sites.iter().filter(|s| s.tile == (nx, ny) && s.kind != SiteKind::Wilds).map(|s| s.id).collect();
        for id in &here { if !self.known.contains(id) { self.known.push(*id); let s = self.site(*id).unwrap(); let line = format!("You come upon {}: {}.{}", s.name, s.kind.word(), if s.cause.is_empty() { String::new() } else { format!(" {}", s.cause) }); self.say(Tone::Quest, line); } }
        if let Some(id) = here.first() { let s = self.site(*id).unwrap(); let n = s.name.clone(); self.say(Tone::Info, format!("{} is here. (Enter to walk the land.)", n)); return true; }
        // Something on the road (an ambush, by the land's danger; worse in blank country).
        let danger = self.world.danger[k] as f64 / 255.0;
        let mut r = self.roll(0x7A5E ^ (nx * 131 + ny) as u64);
        let p = (0.02 + danger * 0.12).min(0.15) * if blank { 1.5 } else { 1.0 } * if self.night() { 1.4 } else { 1.0 };
        if r.gen_bool(p.min(0.3)) {
            // As dangerous as the land, but never far past the one walking it.
            let tier = (1 + (danger * 3.0) as u32).min(1 + self.hero.level / 7).clamp(1, 5);
            self.say(Tone::Danger, "Something moves on the road ahead. You are set upon!");
            self.ambush(tier);
            return true;
        }
        true
    }

    fn enter_here(&mut self) -> bool {
        if self.here.is_some() { return false; }
        // A town first where a town stands on its own ruins.
        let here: Vec<&SiteSpec> = self.sites.iter().filter(|s| s.tile == self.tile && s.kind != SiteKind::Wilds).collect();
        let id = here.iter().find(|s| s.kind == SiteKind::Town).or(here.first()).map(|s| s.id);
        self.land_here(id)
    }
}

fn fresh_rng() -> ChaCha8Rng { ChaCha8Rng::seed_from_u64(0xADE0) }

impl Game {
    /// The adventurer's legend as an HTML page in the journal's style: who they were, what they
    /// did (bosses, quests, treasures, falls), where they went.
    pub fn legend_html(&self) -> String {
        let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
        let h = &self.hero;
        let home = self.site(h.temple).map(|s| s.name.clone()).unwrap_or_default();
        let mut out = String::new();
        out.push_str("<!doctype html><html><head><meta charset=\"utf-8\"><title>The Legend of ");
        out.push_str(&esc(&h.name));
        out.push_str("</title><style>body{background:#2a221b;margin:0}main{max-width:760px;margin:30px auto;background:#eadec4;color:#38291f;padding:36px 48px;font-family:'IM Fell English',Georgia,serif;border:1px solid #382a20;box-shadow:0 0 0 4px #eadec4,0 0 0 5px #806a52}h1{color:#9a2a1e;font-variant:small-caps;letter-spacing:1px}h2{color:#9a2a1e;font-variant:small-caps;border-bottom:1px solid #806a52}li{margin:4px 0}.day{color:#9a2a1e;font-style:italic}</style></head><body><main>");
        out.push_str(&format!("<h1>The Legend of {}</h1><p><i>{} of the {}, of {}; level {}{}.</i></p>", esc(&h.name), esc(h.calling.as_deref().unwrap_or("a commoner")), esc(&h.race), esc(&home), h.level,
            if self.victory { ", who broke the Shadow" } else { "" }));
        out.push_str(&format!("<p>{} slain, {} of them named; {} quests done; {} places entered; {} times fallen.</p>", h.kills, self.stats.bosses, self.stats.quests_done, self.stats.sites_entered, h.deaths));
        out.push_str("<h2>Deeds</h2><ul>");
        for (t, d) in &self.deeds { out.push_str(&format!("<li><span class=\"day\">Day {}</span> — {} {}</li>", t / 144_000 + 1, esc(&h.name), esc(d))); }
        out.push_str("</ul><h2>What they carried</h2><ul>");
        for it in h.equipped.iter().flatten().chain(h.pack.iter().filter(|i| i.is_artifact())) {
            out.push_str(&format!("<li>{}{}</li>", esc(&it.describe()), it.story.as_ref().map(|s| format!(" — <i>{}</i>", esc(s))).unwrap_or_default()));
        }
        out.push_str("</ul><h2>Where they went</h2><ul>");
        let mut known: Vec<&SiteSpec> = self.sites.iter().filter(|s| self.places.contains_key(&s.id) && s.kind != SiteKind::Wilds).collect();
        known.sort_by_key(|s| s.id);
        for s in known { out.push_str(&format!("<li>{} ({}){}</li>", esc(&s.name), s.kind.word(), if s.cause.is_empty() { String::new() } else { format!(": <i>{}</i>", esc(&s.cause)) })); }
        out.push_str("</ul></main></body></html>");
        out
    }
}

/// The head of a saved adventure: what world it belongs to.
const SAVE_MAGIC: &[u8; 8] = b"ADVENT02";
/// Saves of the first builds (bincode): they cannot be read once the game has changed.
const OLD_MAGIC: &[u8; 8] = b"ADVENT01";

impl Game {
    /// Write the adventure to `path`: the header, then gzipped JSON of the world's size and the
    /// game (JSON keeps old saves readable as the game grows: new fields take their defaults).
    pub fn save(&mut self, path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
        use std::io::Write;
        // The land floor's changes into its chunks first.
        self.store_land();
        if let Some(dir) = path.parent() { if !dir.as_os_str().is_empty() { std::fs::create_dir_all(dir)?; } }
        let mut out = SAVE_MAGIC.to_vec();
        let mut z = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        serde_json::to_writer(&mut z, &((self.world.w as u64, self.world.h as u64, self.seed), self))?;
        out.extend(z.finish()?);
        let tmp = path.with_extension("tmp");
        std::fs::File::create(&tmp)?.write_all(&out)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
    fn read_save(path: &std::path::Path) -> Result<((u64, u64, u64), Game), Box<dyn std::error::Error>> {
        let data = std::fs::read(path)?;
        if data.len() >= 8 && &data[..8] == OLD_MAGIC { return Err("that adventure was saved by an earlier build of the game and cannot be read any more".into()); }
        if data.len() < 8 || &data[..8] != SAVE_MAGIC { return Err("not a saved adventure".into()); }
        let z = flate2::read::GzDecoder::new(&data[8..]);
        Ok(serde_json::from_reader(z)?)
    }
    /// Read an adventure saved by `save`; refused for another world.
    pub fn load(path: &std::path::Path, info: WorldInfo) -> Result<Game, Box<dyn std::error::Error>> {
        let ((w, h, _), mut g) = Self::read_save(path)?;
        if (w as usize, h as usize) != (info.w, info.h) { return Err(format!("that adventure belongs to a {}x{} world", w, h).into()); }
        g.world = info;
        g.rng = ChaCha8Rng::seed_from_u64(g.seed ^ g.turn);
        g.set_atlas();
        if !g.seamless {
            // Saved before the land could be walked: wake in the temple, the places made anew.
            g.into_the_land();
            g.wake_at_temple();
            g.say(Tone::Info, "The world has grown wide around you: every land between the towns can be walked now. You wake in your temple.");
        } else if g.here == Some(LAND) {
            g.build_land();
        }
        if g.mapped.len() != g.world.w * g.world.h { g.start_map(); }
        g.look();
        Ok(g)
    }
    /// A saved adventure read without its world (for `--adventure-inspect`).
    pub fn peek(path: &std::path::Path) -> Result<Game, Box<dyn std::error::Error>> { Ok(Self::read_save(path)?.1) }

    /// Where this adventure is saved by default.
    pub fn save_path(&self) -> std::path::PathBuf { std::path::PathBuf::from(format!("adventures/{}_{}.adv", self.seed, self.hero.name.to_lowercase().replace(|c: char| !c.is_alphanumeric(), "_"))) }
}

pub fn cap(s: &str) -> String { let mut c = s.chars(); c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default() }
