//! The first colony: seven settlers living on a playable area with no orders from anyone.
//!
//! The thinnest slice of roadmap Update 3. A clock (one tick = one game minute), three needs
//! (hunger, rest, shelter), and a handful of jobs each settler picks for itself by utility:
//! eat, sleep, forage shrubs, fish the river, fell trees, haul what lies about to the camp, and
//! build a hut once there are logs. Every settler keeps one line saying why it is doing what it
//! is doing, and the colony keeps a log of the moments that matter (the first meal from the
//! river, the hut's last wall, a settler going hungry). Deterministic: a ChaCha stream seeded
//! from the world and the site. Built and judged headless with `--sim-snapshot`.

pub mod nav;
pub mod arc;

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use crate::local::{LocalMap, Material, Plant, RoofPlan, Shape};
use nav::Pos;

pub const TICKS_PER_DAY: u64 = 1440;
/// Logs a hut takes.
pub const HUT_LOGS: u32 = 36;
/// Hut footprint (cells, 2 m each): walls on the ring, a door on the south side.
const HUT_W: usize = 6;
const HUT_H: usize = 5;
/// Food the settlers try to keep at the camp, per settler.
const FOOD_PER_SETTLER: u32 = 4;
/// Days a foraged shrub takes to bear again.
const SHRUB_REGROW_DAYS: u64 = 12;
/// How far (cells) settlers look for work.
const WORK_RADIUS: i32 = 60;
/// Path search budget (nodes).
const PATH_BUDGET: usize = 40_000;

/// Game time.
#[derive(Clone, Copy, Debug, Default)]
pub struct Clock { pub tick: u64 }

impl Clock {
    pub fn day(&self) -> u64 { self.tick / TICKS_PER_DAY + 1 }
    pub fn hour(&self) -> u64 { (self.tick % TICKS_PER_DAY) / 60 }
    pub fn minute(&self) -> u64 { self.tick % 60 }
    pub fn is_night(&self) -> bool { let h = self.hour(); h < 6 || h >= 21 }
    pub fn stamp(&self) -> String { format!("Day {}, {:02}:{:02}", self.day(), self.hour(), self.minute()) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemKind { Log, Food }

#[derive(Clone, Debug)]
pub struct Item { pub kind: ItemKind, pub at: Pos, pub stored: bool, pub reserved: bool }

/// What a settler is doing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Job {
    Idle,
    Eat,
    Sleep,
    Forage(Pos),
    Fish(Pos),
    Fell(Pos),
    Haul(usize),
    Build,
    Wander(Pos),
}

impl Job {
    pub fn verb(&self) -> &'static str {
        match self {
            Job::Idle => "idle", Job::Eat => "eating", Job::Sleep => "sleeping", Job::Forage(_) => "foraging",
            Job::Fish(_) => "fishing", Job::Fell(_) => "felling a tree", Job::Haul(_) => "hauling",
            Job::Build => "building the hut", Job::Wander(_) => "wandering",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Settler {
    pub name: String,
    pub pos: Pos,
    pub path: Vec<Pos>,
    /// 0 = fed, 1 = starving.
    pub hunger: f32,
    /// 0 = rested, 1 = exhausted.
    pub fatigue: f32,
    /// 0 = sheltered, 1 = chilled through (nights in the open).
    pub exposure: f32,
    pub job: Job,
    /// Minutes of work left once at the job's place.
    pub work_left: u32,
    pub carrying: Option<ItemKind>,
    /// Why this settler is doing what it is doing, in one line.
    pub why: String,
    pub alive: bool,
    /// Ticks spent wanting to move but unable to.
    pub stuck: u32,
    /// Ticks spent starving (hunger at 1).
    pub starving: u32,
    /// Liking for each kind of work (forage, fish, fell, haul, build), 0.75-1.3: settlers
    /// differ, so seven of them don't all do the same thing at once.
    pub taste: [f32; 5],
    /// Who they were before the colony (from the history; None for nameless wanderers).
    pub past: Option<crate::history::settlers::Past>,
}

/// The hut the settlers build together.
#[derive(Clone, Debug)]
pub struct Hut {
    /// Top-left corner of the footprint.
    pub at: Pos,
    pub logs_used: u32,
    pub done: bool,
}

/// What a dream asks of a settler, for a day.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dream {
    /// Of the hut finished: building and felling pull harder.
    Hut,
    /// Of plenty: foraging and fishing pull harder.
    Plenty,
    /// Of stillness: they rest by day.
    Rest,
}

impl Dream {
    pub fn word(self) -> &'static str { match self { Dream::Hut => "the hut standing finished", Dream::Plenty => "baskets full of berries and fish", Dream::Rest => "a long quiet sleep" } }
}

/// A place the patron marked.
#[derive(Clone, Copy, Debug)]
pub struct PlaceMark { pub at: Pos, pub radius: u16, pub forbidden: bool }

/// The patron's indirect hand: no orders, only favour. Favour (3 at most, one more each dawn)
/// is spent on marking a place favoured or forbidden, favouring a settler, or sending a dream.
#[derive(Clone, Debug)]
pub struct Patron {
    pub favour: u32,
    pub marks: Vec<PlaceMark>,
    pub favourite: Option<usize>,
    /// (settler, dream, until tick).
    pub dreams: Vec<(usize, Dream, u64)>,
    last_refill_day: u64,
}

pub const FAVOUR_MAX: u32 = 3;

/// A founding stone: the patron's hand in the colony's shape, placed without designations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoneKind {
    /// The hall (the hut) is raised beside it.
    Hall,
    /// Keep this grove: its trees are never felled.
    Grove,
    /// A shrine: a standing stone the settlers rest by.
    Shrine,
}

impl StoneKind {
    pub fn word(self) -> &'static str { match self { StoneKind::Hall => "hall stone", StoneKind::Grove => "grove stone", StoneKind::Shrine => "shrine" } }
}

/// A permanent mark a moment leaves on the colony: a grave, a raised stone. Drawn on the map and
/// opened by a click (the inspector shows its words and the day).
#[derive(Clone, Debug)]
pub struct ColonyMark {
    pub at: Pos,
    pub kind: MarkKind,
    pub title: String,
    /// The words on it.
    pub text: String,
    pub day: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkKind { Grave, Stone, Scorch }

/// How far a grove stone keeps the axe away.
pub const GROVE_RADIUS: i32 = 7;

pub struct Colony {
    pub map: LocalMap,
    pub patron: Patron,
    /// What its patron called it, and the places named.
    pub name: Option<String>,
    pub place_names: Vec<(Pos, String)>,
    pub stones: Vec<(StoneKind, Pos)>,
    /// What happened here, left on the ground.
    pub marks: Vec<ColonyMark>,
    /// Every patron's act, as a replayable line "tick verb args" (`apply_intervention`): the
    /// same world code and the same interventions tell the same story on any machine.
    pub interventions: Vec<String>,
    /// Script lines up to this tick have been applied (`run_days_scripted`).
    script_at: u64,
    /// The first arc (`arc.rs`), planned from the history at founding.
    pub arc: Option<arc::Arc>,
    /// A banner for a great moment: (text, tick shown).
    pub banner: Option<(String, u64)>,
    /// Who keeps watch tonight (the arc).
    pub(crate) watcher: Option<usize>,
    /// Everyone who laid a log in the hut, in order.
    builders: Vec<usize>,
    pub clock: Clock,
    pub settlers: Vec<Settler>,
    pub items: Vec<Item>,
    /// Where the stockpile and fire are.
    pub camp: Pos,
    pub hut: Option<Hut>,
    /// Day each foraged shrub bears again.
    shrub_ready: crate::history::det::HashMap<Pos, u64>,
    /// Targets someone already went for (trees, shrubs, fishing spots).
    claimed: crate::history::det::HashSet<Pos>,
    /// Unreachable targets, so nobody keeps trying them.
    unreachable: crate::history::det::HashSet<Pos>,
    pub log: Vec<String>,
    /// Every choice a settler made, with its reason.
    pub decisions: Vec<String>,
    rng: ChaCha8Rng,
    /// Moments already logged once (first meal, first log...).
    milestones: crate::history::det::HashSet<&'static str>,
    /// Items carried along with a hauled one (a forager's basket): stored when it arrives.
    basket: crate::history::det::HashMap<usize, Vec<usize>>,
}

impl Colony {
    /// Found a colony of `names` on `map`; the camp goes on the dry, open, flat ground nearest
    /// the centre.
    pub fn found(mut map: LocalMap, names: &[String], seed: u64) -> Self {
        let n = map.width;
        let centre = (n / 2) as i32;
        let mut camp = ((n / 2) as u16, (n / 2) as u16);
        let mut best = i32::MAX;
        for y in 8..n - 8 {
            for x in 8..n - 8 {
                if !nav::passable(&map, (x as u16, y as u16)) { continue; }
                let flat = (-2i32..=2).all(|d| {
                    let (xx, yy) = ((x as i32 + d) as usize, y);
                    map.surface_z[yy * n + xx] == map.surface_z[y * n + x] && nav::passable(&map, (xx as u16, yy as u16))
                });
                if !flat { continue; }
                let trees = (-2i32..=2).flat_map(|dy| (-2i32..=2).map(move |dx| (dx, dy)))
                    .filter(|&(dx, dy)| matches!(map.cell((x as i32 + dx) as usize, (y as i32 + dy) as usize, map.surface_z[((y as i32 + dy) as usize) * n + (x as i32 + dx) as usize] as usize).plant, Plant::Tree(_)))
                    .count() as i32;
                let score = (x as i32 - centre).abs() + (y as i32 - centre).abs() + trees * 6;
                if score < best { best = score; camp = (x as u16, y as u16); }
            }
        }
        // Clear the camp ground.
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                let (x, y) = ((camp.0 as i32 + dx) as usize, (camp.1 as i32 + dy) as usize);
                let k = map.idx(x, y, map.surface_z[y * n + x] as usize);
                map.cells[k].plant = Plant::None;
                map.cells[k].boulder = false;
            }
        }
        let mut rng = ChaCha8Rng::seed_from_u64(seed ^ 0xC010_4E57);
        let settlers = names.iter().enumerate().map(|(i, name)| {
            let pos = (camp.0 + (i as u16 % 3), camp.1 + (i as u16 / 3) % 3);
            Settler {
                name: name.clone(), pos: if nav::passable(&map, pos) { pos } else { camp }, path: Vec::new(),
                hunger: 0.2 + 0.2 * rng.gen::<f32>(), fatigue: 0.1 * rng.gen::<f32>(), exposure: 0.0,
                job: Job::Idle, work_left: 0, carrying: None, why: "Just arrived".into(),
                alive: true, stuck: 0, starving: 0,
                taste: [0; 5].map(|_| 0.75 + 0.55 * rng.gen::<f32>()),
                past: None,
            }
        }).collect::<Vec<_>>();
        // They arrive with two days of food.
        let items = (0..names.len() * 2).map(|_| Item { kind: ItemKind::Food, at: camp, stored: true, reserved: false }).collect();
        let mut c = Colony {
            map, clock: Clock { tick: 6 * 60 }, settlers, items, camp, hut: None,
            shrub_ready: Default::default(), claimed: Default::default(), unreachable: Default::default(),
            log: Vec::new(), decisions: Vec::new(), rng, milestones: Default::default(), basket: Default::default(),
            patron: Patron { favour: FAVOUR_MAX, marks: Vec::new(), favourite: None, dreams: Vec::new(), last_refill_day: 1 },
            name: None, place_names: Vec::new(), stones: Vec::new(), marks: Vec::new(), builders: Vec::new(), interventions: Vec::new(), script_at: 0, arc: None, banner: None, watcher: None,
        };
        c.hut = c.find_hut_site().map(|at| Hut { at, logs_used: 0, done: false });
        let site = c.hut.as_ref().map(|h| format!(" and a hut site at {},{}", h.at.0, h.at.1)).unwrap_or_default();
        c.note(format!("{} settlers make camp at {},{}{}. They carry two days of food.", names.len(), camp.0, camp.1, site));
        c
    }

    fn note(&mut self, line: String) { self.log.push(format!("{}  {}", self.clock.stamp(), line)); }

    fn spend(&mut self) -> Result<(), String> {
        if self.patron.favour == 0 { return Err("no favour left today".into()); }
        self.patron.favour -= 1;
        Ok(())
    }

    /// Mark the ground within `radius` of `at` favoured (work there pulls harder) or forbidden
    /// (nobody works or wanders there).
    pub fn mark_place(&mut self, at: Pos, radius: u16, forbidden: bool) -> Result<String, String> {
        self.spend()?;
        self.patron.marks.retain(|m| m.at != at);
        self.patron.marks.push(PlaceMark { at, radius, forbidden });
        self.interventions.push(format!("{} {} {} {} {}", self.clock.tick, if forbidden { "forbid" } else { "bless" }, at.0, at.1, radius));
        let line = format!("The patron {} the ground about {},{}. (your doing)", if forbidden { "forbade" } else { "blessed" }, at.0, at.1);
        self.note(line.clone());
        Ok(line)
    }

    /// Favour one settler: they work with more heart, and the others notice.
    pub fn favour_settler(&mut self, i: usize) -> Result<String, String> {
        if !self.settlers.get(i).map_or(false, |s| s.alive) { return Err("no one there".into()); }
        self.spend()?;
        self.patron.favourite = Some(i);
        self.interventions.push(format!("{} favour {}", self.clock.tick, i));
        let name = self.settlers[i].name.clone();
        let line = format!("The patron favours {}; the others notice. (your doing)", name);
        self.note(line.clone());
        Ok(line)
    }

    /// Send a settler a dream: for a day it pulls them towards what they dreamt of.
    pub fn send_dream(&mut self, i: usize, dream: Dream) -> Result<String, String> {
        if !self.settlers.get(i).map_or(false, |s| s.alive) { return Err("no one there".into()); }
        self.spend()?;
        let until = self.clock.tick + TICKS_PER_DAY;
        self.patron.dreams.retain(|d| d.0 != i);
        self.patron.dreams.push((i, dream, until));
        self.interventions.push(format!("{} dream {} {:?}", self.clock.tick, i, dream));
        let name = self.settlers[i].name.clone();
        let line = format!("{} dreamt of {}. (your doing)", name, dream.word());
        self.note(line.clone());
        // Ask again now: a dream changes the day.
        self.decide(i, false);
        Ok(line)
    }

    /// Place a founding stone (five at most). A hall stone moves the hut site beside it while no
    /// log is laid; a grove stone keeps the axe from the trees around it; a shrine is a standing
    /// stone the settlers rest by.
    pub fn place_stone(&mut self, kind: StoneKind, at: Pos) -> Result<String, String> {
        if self.stones.len() >= 5 { return Err("five founding stones are set already".into()); }
        if !nav::passable(&self.map, at) { return Err("no footing for a stone there".into()); }
        self.stones.retain(|s| !(s.0 == kind && kind == StoneKind::Hall));
        self.stones.push((kind, at));
        self.interventions.push(format!("{} stone {:?} {} {}", self.clock.tick, kind, at.0, at.1));
        if kind == StoneKind::Shrine {
            let k = at.1 as usize * self.map.width + at.0 as usize;
            self.map.features[k] = crate::local::wildlife::Feature::Stone;
        }
        let mut line = format!("The patron set a {} at {},{}.", kind.word(), at.0, at.1);
        if kind == StoneKind::Hall && self.hut.as_ref().map_or(true, |h| h.logs_used == 0) {
            self.hut = self.find_hut_site().map(|a| Hut { at: a, logs_used: 0, done: false });
            if let Some(h) = &self.hut { line.push_str(&format!(" The hall will stand at {},{}.", h.at.0, h.at.1)); }
        }
        self.note(format!("{} (your doing)", line));
        Ok(line)
    }

    /// A settler dies of `cause`: the others bury them at the edge of the camp, under words from
    /// their life.
    pub fn bury(&mut self, i: usize, cause: &str) {
        self.settlers[i].alive = false;
        let s = &self.settlers[i];
        let n = self.marks.iter().filter(|m| m.kind == MarkKind::Grave).count() as i32;
        let at = self.spot_from_camp(-8 + 2 * n, 6);
        let past = s.past.as_ref().map(|p| format!(" Aged {}, {}.", p.age, p.calling)).unwrap_or_default();
        let feeling = s.past.as_ref().and_then(|p| p.feeling.as_ref()).map(|f| format!(" Remembered as one who {}.", f.0)).unwrap_or_default();
        let text = format!("Here lies {}.{} Died {} on day {}.{}", s.name, past, cause, self.clock.day(), feeling);
        let title = format!("The grave of {}", s.name);
        // The ground is cleared for it.
        let w = self.map.width;
        for dy in -1i32..=1 { for dx in -1i32..=1 {
            let (x, y) = ((at.0 as i32 + dx).max(0) as usize, (at.1 as i32 + dy).max(0) as usize);
            if x < w && y < self.map.height {
                let k = self.map.idx(x, y, self.map.surface_z[y * w + x].max(0) as usize);
                self.map.cells[k].plant = Plant::None;
            }
        } }
        self.marks.push(ColonyMark { at, kind: MarkKind::Grave, title, text, day: self.clock.day() });
        let name = s.name.clone();
        self.note(format!("They bury {} at {},{}.", name, at.0, at.1));
    }

    /// Name the colony.
    pub fn name_colony(&mut self, name: &str) {
        self.name = Some(name.to_string());
        self.interventions.push(format!("{} name {}", self.clock.tick, name));
        self.note(format!("The settlement is called {} by its patron. (your doing)", name));
    }

    /// Name a place: "called the Long Field by its patron".
    pub fn name_place(&mut self, at: Pos, name: &str) {
        self.place_names.push((at, name.to_string()));
        self.note(format!("The ground about {},{} is called {} by its patron. (your doing)", at.0, at.1, name));
    }

    fn marked(&self, p: Pos, forbidden: bool) -> bool {
        self.patron.marks.iter().any(|m| m.forbidden == forbidden
            && (m.at.0 as i32 - p.0 as i32).abs().max((m.at.1 as i32 - p.1 as i32).abs()) <= m.radius as i32)
    }

    fn dream_of(&self, i: usize) -> Option<Dream> {
        self.patron.dreams.iter().find(|d| d.0 == i && d.2 > self.clock.tick).map(|d| d.1)
    }
    fn once(&mut self, key: &'static str, line: String) { if self.milestones.insert(key) { self.note(line); } }

    pub fn food_stored(&self) -> u32 { self.items.iter().filter(|i| i.kind == ItemKind::Food && i.stored).count() as u32 }
    pub fn logs_stored(&self) -> u32 { self.items.iter().filter(|i| i.kind == ItemKind::Log && i.stored).count() as u32 }
    pub fn alive(&self) -> usize { self.settlers.iter().filter(|s| s.alive).count() }

    /// A flat, dry 6x5 patch near the camp for the hut.
    fn find_hut_site(&self) -> Option<Pos> {
        let n = self.map.width as i32;
        let (cx, cy) = (self.camp.0 as i32, self.camp.1 as i32);
        // Beside the hall stone if the patron placed one, else near the camp.
        let (sx, sy) = self.stones.iter().find(|s| s.0 == StoneKind::Hall).map(|s| (s.1 .0 as i32, s.1 .1 as i32)).unwrap_or((cx, cy));
        let mut best: Option<(i32, Pos)> = None;
        for y in (sy - 14).max(2)..(sy + 14).min(n - HUT_H as i32 - 2) {
            for x in (sx - 14).max(2)..(sx + 14).min(n - HUT_W as i32 - 2) {
                // Not on the camp itself.
                if (x..x + HUT_W as i32).contains(&cx) && (y..y + HUT_H as i32).contains(&cy) { continue; }
                let z0 = self.map.surface_z[(y * n + x) as usize];
                let ok = (0..HUT_H as i32).all(|dy| (0..HUT_W as i32).all(|dx| {
                    let (xx, yy) = (x + dx, y + dy);
                    self.map.surface_z[(yy * n + xx) as usize] == z0 && nav::passable(&self.map, (xx as u16, yy as u16))
                }));
                if !ok { continue; }
                // Not on a stone.
                if self.stones.iter().any(|(_, at)| (x..x + HUT_W as i32).contains(&(at.0 as i32)) && (y..y + HUT_H as i32).contains(&(at.1 as i32))) { continue; }
                let d = (x - sx).abs() + (y - sy).abs();
                if best.map_or(true, |b| d < b.0) { best = Some((d, (x as u16, y as u16))); }
            }
        }
        best.map(|b| b.1)
    }

    fn hut_cells(&self) -> Vec<Pos> {
        let Some(h) = &self.hut else { return Vec::new() };
        (0..HUT_H).flat_map(|dy| (0..HUT_W).map(move |dx| (h.at.0 + dx as u16, h.at.1 + dy as u16))).collect()
    }

    fn in_hut(&self, p: Pos) -> bool {
        self.hut.as_ref().map_or(false, |h| h.done && p.0 > h.at.0 && p.0 < h.at.0 + HUT_W as u16 - 1 && p.1 > h.at.1 && p.1 < h.at.1 + HUT_H as u16 - 1)
    }

    /// Advance the colony by one game minute.
    pub fn tick(&mut self) {
        self.clock.tick += 1;
        // Favour returns with the dawn.
        let day = self.clock.day();
        if day > self.patron.last_refill_day && self.clock.hour() >= 6 {
            self.patron.last_refill_day = day;
            self.patron.favour = (self.patron.favour + 1).min(FAVOUR_MAX);
        }
        if self.arc.is_some() { self.arc_tick(); }
        let night = self.clock.is_night();
        for i in 0..self.settlers.len() {
            if !self.settlers[i].alive { continue; }
            self.update_needs(i, night);
            if !self.settlers[i].alive { continue; }
            // Choose: when the job is done, or every hour if something matters much more now.
            let idle = self.settlers[i].job == Job::Idle;
            if idle || self.clock.tick % 60 == (i as u64 * 7) % 60 {
                self.decide(i, idle);
            }
            self.act(i);
        }
    }

    fn update_needs(&mut self, i: usize, night: bool) {
        let sheltered = self.in_hut(self.settlers[i].pos);
        let s = &mut self.settlers[i];
        let asleep = s.job == Job::Sleep && s.work_left > 0 && s.path.is_empty();
        s.hunger = (s.hunger + 1.0 / (18.0 * 60.0)).min(1.0);
        s.fatigue = if asleep { (s.fatigue - 1.0 / (7.0 * 60.0)).max(0.0) } else { (s.fatigue + 1.0 / (17.0 * 60.0)).min(1.0) };
        s.exposure = if sheltered { (s.exposure - 1.0 / 120.0).max(0.0) }
            else if night { (s.exposure + 1.0 / (10.0 * 60.0)).min(1.0) }
            else { (s.exposure - 1.0 / (6.0 * 60.0)).max(0.0) };
        if s.hunger >= 1.0 { s.starving += 1; } else { s.starving = 0; }
        let starving_days = s.starving as u64 / TICKS_PER_DAY;
        let name = s.name.clone();
        if s.starving == TICKS_PER_DAY as u32 / 4 {
            self.note(format!("{} is starving.", name));
        }
        if starving_days >= 4 {
            self.settlers[i].alive = false;
            self.note(format!("{} died of hunger.", name));
            self.bury(i, "of hunger");
        }
    }

    /// Pick the best thing to do now. `free`: the settler has nothing in hand.
    fn decide(&mut self, i: usize, free: bool) {
        let s = self.settlers[i].clone();
        let food = self.food_stored();
        let food_goal = FOOD_PER_SETTLER * self.alive() as u32;
        let night = self.clock.is_night();
        let hut_pending = self.hut.as_ref().map_or(false, |h| !h.done);
        let logs_needed = self.hut.as_ref().map_or(0, |h| HUT_LOGS.saturating_sub(h.logs_used));
        let logs_about = self.items.iter().filter(|it| it.kind == ItemKind::Log).count() as u32;
        let loose = self.items.iter().position(|it| !it.stored && !it.reserved && self.reachable_from(s.pos, it.at));

        // Utilities, 0..~3. Each option says why.
        let mut options: Vec<(f32, Job, String)> = Vec::new();
        if food > 0 && s.hunger > 0.35 {
            options.push((s.hunger * s.hunger * 3.0, Job::Eat, format!("Hungry ({:.0}%) and there is food at the camp", s.hunger * 100.0)));
        }
        // Night is for sleeping, rested or not; by day only the tired lie down.
        let sleepy = s.fatigue * s.fatigue * 2.5 + if night { 0.3 + if s.fatigue > 0.25 { 0.5 } else { 0.0 } } else { 0.0 }
            + if night && s.exposure > 0.3 { 0.3 } else { 0.0 };
        if s.fatigue > 0.2 || night {
            let place = if self.hut.as_ref().map_or(false, |h| h.done) { "in the hut" } else { "by the fire" };
            options.push((sleepy, Job::Sleep, format!("Tired ({:.0}%){}; sleeping {}", s.fatigue * 100.0, if night { " and it is night" } else { "" }, place)));
        }
        if food < food_goal {
            let short = (food_goal - food) as f32 / food_goal as f32;
            if let Some(t) = self.nearest(s.pos, |c, p| c.is_ripe_shrub(p)) {
                options.push(((0.4 + short) * s.taste[0], Job::Forage(t), format!("The camp has {} of the {} meals it needs; picking berries at {},{}", food, food_goal, t.0, t.1)));
            }
            if let Some(t) = self.nearest(s.pos, |c, p| c.is_fishing_spot(p)) {
                options.push(((0.35 + short) * s.taste[1], Job::Fish(t), format!("The camp has {} of the {} meals it needs; fishing at {},{}", food, food_goal, t.0, t.1)));
            }
        }
        let fellers = self.settlers.iter().filter(|o| o.alive && matches!(o.job, Job::Fell(_))).count() as u32;
        if hut_pending && logs_about + 2 * fellers < logs_needed {
            if let Some(t) = self.nearest(s.pos, |c, p| c.is_felling_tree(p)) {
                options.push((0.75 * s.taste[2], Job::Fell(t), format!("The hut needs {} more logs; felling the tree at {},{}", logs_needed - logs_about.min(logs_needed), t.0, t.1)));
            }
        }
        if let Some(k) = loose {
            let is_food = self.items[k].kind == ItemKind::Food;
            let what = if is_food { "food" } else { "a log" };
            let short = if is_food && food < food_goal { 1.0 } else { 0.0 };
            options.push(((0.8 + short) * s.taste[3], Job::Haul(k), format!("{} lies at {},{}; carrying it to the camp", capital(what), self.items[k].at.0, self.items[k].at.1)));
        }
        // No more builders than there are logs at the camp to set.
        let builders = self.settlers.iter().filter(|o| o.alive && o.job == Job::Build).count() as u32;
        if hut_pending && self.logs_stored() > builders && !night {
            options.push((0.85 * s.taste[4], Job::Build, format!("There are logs at the camp; raising the hut ({} of {} logs in)", self.hut.as_ref().map_or(0, |h| h.logs_used), HUT_LOGS)));
        }
        // Idle hours are spent by the fire, or by the shrine if the patron set one.
        let shrine = self.stones.iter().find(|s| s.0 == StoneKind::Shrine).map(|s| s.1);
        let (rest_at, rest_why) = match shrine { Some(p) if i % 2 == 0 => (p, "resting by the shrine"), _ => (self.camp, "resting near the fire") };
        let spots: Vec<Pos> = (0..8).map(|_| (rest_at.0.saturating_add_signed(self.rng.gen_range(-4..=4)), rest_at.1.saturating_add_signed(self.rng.gen_range(-4..=4)))).collect();
        let wander_to = spots.into_iter().find(|&p| nav::passable(&self.map, p) && !self.in_hut(p) && p != rest_at).unwrap_or(self.camp);
        options.push((0.05, Job::Wander(wander_to), format!("Nothing needs doing; {}", rest_why)));

        // The night's watch (the first arc): the watcher stays up at the camp's edge.
        if self.watcher == Some(i) && night {
            let threat = self.arc.as_ref().map(|a| a.threat.name.clone()).unwrap_or_default();
            let edge = self.spot_from_camp(0, -6);
            options.push((2.5, Job::Wander(edge), format!("Keeping watch at the camp's edge, for fear of {}", threat)));
        }
        // The patron's hand: favoured ground pulls, dreams pull, the favourite works with heart.
        // Work in a favoured place: look there too, and prefer it.
        if self.patron.marks.iter().any(|m| !m.forbidden) {
            let fav = |c: &Colony, p: Pos| c.marked(p, false);
            if food < food_goal {
                if let Some(t) = self.nearest(s.pos, |c, p| c.is_ripe_shrub(p) && fav(c, p)) {
                    options.push(((0.4 + 1.0) * s.taste[0] * 1.4, Job::Forage(t), format!("Picking berries at {},{}, on the ground the patron blessed", t.0, t.1)));
                }
            }
            if hut_pending && logs_about + 2 * fellers < logs_needed {
                if let Some(t) = self.nearest(s.pos, |c, p| c.is_felling_tree(p) && fav(c, p)) {
                    options.push((0.75 * s.taste[2] * 1.4, Job::Fell(t), format!("Felling the tree at {},{}, on the ground the patron blessed", t.0, t.1)));
                }
            }
        }
        let forbidden_near = self.patron.marks.iter().any(|m| m.forbidden && (m.at.0 as i32 - s.pos.0 as i32).abs().max((m.at.1 as i32 - s.pos.1 as i32).abs()) <= WORK_RADIUS);
        let dream = self.dream_of(i);
        let favourite = self.patron.favourite == Some(i);
        for o in options.iter_mut() {
            let pull = match (dream, &o.1) {
                (Some(Dream::Hut), Job::Build | Job::Fell(_)) => 1.0,
                (Some(Dream::Plenty), Job::Forage(_) | Job::Fish(_)) => 0.8,
                (Some(Dream::Rest), Job::Sleep) => 1.2,
                _ => 0.0,
            };
            if pull > 0.0 {
                o.0 += pull;
                o.2 = format!("Dreamt of {}; {}", dream.unwrap().word(), lower_first(&o.2));
            }
            if favourite && !matches!(o.1, Job::Eat | Job::Sleep | Job::Wander(_)) {
                o.0 *= 1.15;
                o.2.push_str(" (the patron's favourite)");
            }
            if forbidden_near && matches!(o.1, Job::Forage(_) | Job::Fish(_) | Job::Fell(_) | Job::Wander(_)) {
                o.2.push_str(", keeping off the forbidden ground");
            }
        }
        // Wandering into forbidden ground is no rest.
        options.retain(|o| !matches!(o.1, Job::Wander(p) if self.marked(p, true)));
        let Some((best_u, best, why)) = options.into_iter().max_by(|a, b| a.0.total_cmp(&b.0)) else { return };
        if !free {
            // Keep the current job unless something matters much more (an empty belly, sleep).
            let current_u = match s.job { Job::Eat | Job::Sleep => 2.0, Job::Haul(_) if s.carrying.is_some() => 1.5, _ => 0.6 };
            // The same kind of work elsewhere is no reason to drop this one.
            let same_kind = std::mem::discriminant(&best) == std::mem::discriminant(&s.job);
            if same_kind || best_u < current_u * 1.6 { return; }
            self.release(i);
        }
        self.start(i, best, why);
    }

    fn reachable_from(&self, from: Pos, to: Pos) -> bool {
        let d = (from.0 as i32 - to.0 as i32).abs().max((from.1 as i32 - to.1 as i32).abs());
        d <= WORK_RADIUS && !self.unreachable.contains(&to)
    }

    fn nearest(&self, from: Pos, ok: impl Fn(&Colony, Pos) -> bool) -> Option<Pos> {
        let n = self.map.width as i32;
        // Search rings outward from the settler.
        for r in 1..=WORK_RADIUS {
            let mut found: Option<Pos> = None;
            for d in -r..=r {
                for (x, y) in [(from.0 as i32 + d, from.1 as i32 - r), (from.0 as i32 + d, from.1 as i32 + r), (from.0 as i32 - r, from.1 as i32 + d), (from.0 as i32 + r, from.1 as i32 + d)] {
                    if x < 1 || y < 1 || x >= n - 1 || y >= n - 1 { continue; }
                    let p = (x as u16, y as u16);
                    if self.claimed.contains(&p) || self.unreachable.contains(&p) || self.marked(p, true) { continue; }
                    if ok(self, p) && found.map_or(true, |f| (f.1, f.0) > (p.1, p.0)) { found = Some(p); }
                }
            }
            if found.is_some() { return found; }
        }
        None
    }

    fn floor_plant(&self, p: Pos) -> Plant {
        let (x, y) = (p.0 as usize, p.1 as usize);
        self.map.cell(x, y, self.map.surface_z[y * self.map.width + x] as usize).plant
    }
    fn is_ripe_shrub(&self, p: Pos) -> bool {
        self.floor_plant(p) == Plant::Shrub && self.shrub_ready.get(&p).map_or(true, |&d| d <= self.clock.day())
    }
    fn is_felling_tree(&self, p: Pos) -> bool {
        // Not trees inside the hut site.
        matches!(self.floor_plant(p), Plant::Tree(_)) && !self.hut_cells().contains(&p)
            && !self.stones.iter().any(|(k, at)| *k == StoneKind::Grove && (at.0 as i32 - p.0 as i32).abs().max((at.1 as i32 - p.1 as i32).abs()) <= GROVE_RADIUS)
    }
    /// Dry ground next to water a settler can stand on.
    fn is_fishing_spot(&self, p: Pos) -> bool {
        if !nav::passable(&self.map, p) || self.water_at(p) { return false; }
        [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)].iter().any(|&(dx, dy)| self.water_at(((p.0 as i32 + dx) as u16, (p.1 as i32 + dy) as u16)))
    }
    fn water_at(&self, p: Pos) -> bool {
        let (x, y) = (p.0 as usize, p.1 as usize);
        if x >= self.map.width || y >= self.map.height { return false; }
        let z = self.map.surface_z[y * self.map.width + x] + 1;
        (z as usize) < self.map.depth && self.map.cell(x, y, z as usize).water > 0
    }

    fn start(&mut self, i: usize, job: Job, why: String) {
        let target = match job {
            Job::Eat | Job::Build => self.camp,
            Job::Sleep => self.sleep_spot(i),
            Job::Forage(p) | Job::Fish(p) | Job::Wander(p) => p,
            Job::Fell(p) => self.stand_next_to(p).unwrap_or(p),
            Job::Haul(k) => self.items[k].at,
            Job::Idle => return,
        };
        let target = if job == Job::Build { self.hut_door_side() } else { target };
        let from = self.settlers[i].pos;
        match nav::path(&self.map, from, target, PATH_BUDGET) {
            Some(p) => {
                match job {
                    Job::Forage(t) | Job::Fish(t) | Job::Fell(t) => { self.claimed.insert(t); }
                    Job::Haul(k) => self.items[k].reserved = true,
                    _ => {}
                }
                if !matches!(job, Job::Wander(_)) || self.settlers[i].why != why {
                    self.decisions.push(format!("{}  {:<9} {:<16} {}", self.clock.stamp(), self.settlers[i].name, job.verb(), why));
                }
                let s = &mut self.settlers[i];
                s.path = p.into_iter().skip(1).collect();
                s.job = job;
                s.why = why;
                s.work_left = match job {
                    Job::Eat => 20, Job::Sleep => 7 * 60, Job::Forage(_) => 45, Job::Fish(_) => 90,
                    Job::Fell(_) => 150, Job::Build => 45, Job::Haul(_) => 2, Job::Wander(_) => 30, Job::Idle => 0,
                };
            }
            None => {
                if let Job::Forage(t) | Job::Fish(t) | Job::Fell(t) = job { self.unreachable.insert(t); }
                if let Job::Haul(k) = job { let at = self.items[k].at; self.unreachable.insert(at); }
                if !matches!(job, Job::Wander(_)) { self.settlers[i].stuck += 1; }
                self.settlers[i].job = Job::Idle;
            }
        }
    }

    fn release(&mut self, i: usize) {
        match self.settlers[i].job {
            Job::Forage(t) | Job::Fish(t) | Job::Fell(t) => { self.claimed.remove(&t); }
            Job::Haul(k) => {
                if let Some(kind) = self.settlers[i].carrying.take() {
                    // Put it (and the rest of the basket) down where they stand.
                    let at = self.settlers[i].pos;
                    self.items[k] = Item { kind, at, stored: at == self.camp, reserved: false };
                    for e in self.basket.remove(&k).unwrap_or_default() {
                        if e < self.items.len() { self.items[e] = Item { kind: self.items[e].kind, at, stored: at == self.camp, reserved: false }; }
                    }
                } else if k < self.items.len() { self.items[k].reserved = false; }
            }
            _ => {}
        }
        self.settlers[i].job = Job::Idle;
        self.settlers[i].path.clear();
    }

    fn sleep_spot(&self, i: usize) -> Pos {
        match &self.hut {
            Some(h) if h.done => (h.at.0 + 1 + (i as u16 % (HUT_W as u16 - 2)), h.at.1 + 1 + (i as u16 / (HUT_W as u16 - 2)) % (HUT_H as u16 - 2)),
            _ => {
                let p = (self.camp.0.saturating_add_signed((i as i16 % 3) - 1), self.camp.1.saturating_add_signed((i as i16 / 3) % 3 - 1));
                if nav::passable(&self.map, p) { p } else { self.camp }
            }
        }
    }

    fn hut_door_side(&self) -> Pos {
        match &self.hut {
            Some(h) => (h.at.0 + HUT_W as u16 / 2, h.at.1 + HUT_H as u16),
            None => self.camp,
        }
    }

    fn stand_next_to(&self, p: Pos) -> Option<Pos> {
        [(0i32, 1i32), (1, 0), (-1, 0), (0, -1)].iter()
            .map(|&(dx, dy)| ((p.0 as i32 + dx) as u16, (p.1 as i32 + dy) as u16))
            .find(|&q| nav::passable(&self.map, q) && !matches!(self.floor_plant(q), Plant::Tree(_)))
    }

    /// Walk, then work, then finish.
    fn act(&mut self, i: usize) {
        if !self.settlers[i].path.is_empty() {
            // About 30 m a minute (15 cells), slower through water and trees.
            let mut budget = 15i32;
            while budget > 0 {
                let Some(&next) = self.settlers[i].path.first() else { break };
                let Some(c) = nav::cost(&self.map, next.0 as usize, next.1 as usize) else {
                    // Blocked since the path was found (a wall went up): find another way.
                    self.settlers[i].stuck += 1;
                    self.release(i);
                    return;
                };
                budget -= (c / 10).max(1) as i32;
                self.settlers[i].pos = next;
                self.settlers[i].path.remove(0);
            }
            return;
        }
        let job = self.settlers[i].job;
        if job == Job::Idle { return; }
        if self.settlers[i].work_left > 0 {
            self.settlers[i].work_left -= 1;
            // Sleep ends when rested.
            if job == Job::Sleep && self.settlers[i].fatigue <= 0.02 && !self.clock.is_night() { self.settlers[i].work_left = 0; }
            if self.settlers[i].work_left > 0 { return; }
        }
        self.finish(i, job);
    }

    fn finish(&mut self, i: usize, job: Job) {
        let name = self.settlers[i].name.clone();
        let day = self.clock.day();
        match job {
            Job::Eat => {
                if let Some(k) = self.items.iter().position(|it| it.kind == ItemKind::Food && it.stored) {
                    self.items.remove(k);
                    self.fix_item_refs(k);
                    self.settlers[i].hunger = (self.settlers[i].hunger - 0.55).max(0.0);
                }
            }
            Job::Sleep => {}
            Job::Forage(t) => {
                self.claimed.remove(&t);
                self.shrub_ready.insert(t, day + SHRUB_REGROW_DAYS);
                let n = 2 + self.rng.gen_range(0..2);
                for _ in 0..n { self.items.push(Item { kind: ItemKind::Food, at: self.settlers[i].pos, stored: false, reserved: false }); }
                self.once("forage", format!("{} brings in the first berries from the shrubs at {},{}.", name, t.0, t.1));
                // Carry the basket home (what doesn't fit is left for others to fetch).
                if self.carry_home(i, n) { return; }
            }
            Job::Fish(t) => {
                self.claimed.remove(&t);
                let caught = self.rng.gen_range(1..4);
                for _ in 0..caught { self.items.push(Item { kind: ItemKind::Food, at: self.settlers[i].pos, stored: false, reserved: false }); }
                self.once("fish", format!("{} catches the first fish from the river at {},{}.", name, t.0, t.1));
                // A spot fished out for a while.
                self.unreachable.insert(t);
                if self.carry_home(i, caught) { return; }
            }
            Job::Fell(t) => {
                self.claimed.remove(&t);
                let (x, y) = (t.0 as usize, t.1 as usize);
                let k = self.map.idx(x, y, self.map.surface_z[y * self.map.width + x] as usize);
                if matches!(self.map.cells[k].plant, Plant::Tree(_)) {
                    self.map.cells[k].plant = Plant::None;
                    for _ in 0..2 { self.items.push(Item { kind: ItemKind::Log, at: t, stored: false, reserved: false }); }
                    self.once("fell", format!("{} fells the first tree, at {},{}: two logs for the hut.", name, t.0, t.1));
                }
            }
            Job::Haul(k) => {
                if self.settlers[i].carrying.is_none() {
                    // Picked up: now carry it to the camp.
                    if k < self.items.len() && self.items[k].at == self.settlers[i].pos && !self.items[k].stored {
                        self.settlers[i].carrying = Some(self.items[k].kind);
                        let from = self.settlers[i].pos;
                        if let Some(p) = nav::path(&self.map, from, self.camp, PATH_BUDGET) {
                            self.settlers[i].path = p.into_iter().skip(1).collect();
                            self.settlers[i].work_left = 2;
                            return;
                        }
                        self.settlers[i].carrying = None;
                    }
                    if k < self.items.len() { self.items[k].reserved = false; }
                } else {
                    let kind = self.settlers[i].carrying.take().unwrap();
                    if k < self.items.len() {
                        self.items[k] = Item { kind, at: self.camp, stored: true, reserved: false };
                    }
                    for e in self.basket.remove(&k).unwrap_or_default() {
                        if e < self.items.len() { self.items[e].stored = true; self.items[e].reserved = false; self.items[e].at = self.camp; }
                    }
                }
            }
            Job::Build => self.build_step(i),
            Job::Wander(_) | Job::Idle => {}
        }
        self.settlers[i].job = Job::Idle;
    }

    /// Settler `i` just made the last `n` items where they stand: carry one home now (a basket
    /// holds what a forager picks; the rest is left for others). Returns true if they set off.
    fn carry_home(&mut self, i: usize, n: usize) -> bool {
        if n == 0 { return false; }
        let k = self.items.len() - 1;
        // Everything picked rides in one basket: the extra items are carried along.
        let extra: Vec<usize> = (self.items.len() - n..self.items.len() - 1).collect();
        let from = self.settlers[i].pos;
        let Some(p) = nav::path(&self.map, from, self.camp, PATH_BUDGET) else { return false };
        for &e in &extra { self.items[e].reserved = true; self.items[e].at = self.camp; }
        self.items[k].reserved = true;
        let s = &mut self.settlers[i];
        s.carrying = Some(self.items[k].kind);
        s.job = Job::Haul(k);
        s.path = p.into_iter().skip(1).collect();
        s.work_left = 2;
        s.why = format!("Carrying {} back to the camp", if n == 1 { "it".to_string() } else { format!("all {}", n) });
        // The rest of the basket is stored when it arrives.
        self.basket.insert(k, extra);
        true
    }

    /// An item vector index was removed: fix haul jobs pointing past it.
    fn fix_item_refs(&mut self, removed: usize) {
        for s in &mut self.settlers {
            if let Job::Haul(k) = s.job {
                if k == removed { s.job = Job::Idle; s.path.clear(); s.carrying = None; }
                else if k > removed { s.job = Job::Haul(k - 1); }
            }
        }
        let shift = |k: usize| if k > removed { k - 1 } else { k };
        self.basket = std::mem::take(&mut self.basket).into_iter()
            .filter(|(k, _)| *k != removed)
            .map(|(k, v)| (shift(k), v.into_iter().filter(|&e| e != removed).map(shift).collect()))
            .collect();
    }

    /// Put one log into the hut; the last one finishes it.
    fn build_step(&mut self, i: usize) {
        if self.hut.as_ref().map_or(true, |h| h.done) { return; }
        let Some(k) = self.items.iter().position(|it| it.kind == ItemKind::Log && it.stored) else { return };
        self.items.remove(k);
        self.fix_item_refs(k);
        let name = self.settlers[i].name.clone();
        let Some(hut) = self.hut.as_mut() else { return };
        hut.logs_used += 1;
        let used = hut.logs_used;
        if !self.builders.contains(&i) { self.builders.push(i); }
        if used == 1 { self.note(format!("{} sets the first log of the hut.", name)); }
        if used >= HUT_LOGS {
            self.raise_hut();
            self.note(format!("{} finishes the hut. Tonight they sleep under a roof.", name));
            // The builders raise a stone by the door with their names on it.
            if let Some(h) = &self.hut {
                let at = (h.at.0 + HUT_W as u16 / 2 + 1, h.at.1 + HUT_H as u16);
                let names: Vec<String> = self.builders.iter().map(|&b| self.settlers[b].name.clone()).collect();
                let hall = self.name.as_ref().map(|n| format!("{}'s hall", n)).unwrap_or_else(|| "This hall".into());
                let text = format!("{} was raised on day {} by {}.", hall, self.clock.day(), join_names(&names));
                self.marks.push(ColonyMark { at, kind: MarkKind::Stone, title: "The builders' stone".into(), text, day: self.clock.day() });
                self.note("The builders raise a stone by the door with their names on it.".into());
            }
        }
    }

    /// Stamp the finished hut into the map: timber walls on the ring with a door on the south
    /// side, a timber floor, and a roof the ink renderer draws.
    fn raise_hut(&mut self) {
        let Some(h) = self.hut.as_mut() else { return };
        h.done = true;
        let at = h.at;
        let n = self.map.width;
        let mut cells = Vec::new();
        for dy in 0..HUT_H {
            for dx in 0..HUT_W {
                let (x, y) = (at.0 as usize + dx, at.1 as usize + dy);
                let sz = self.map.surface_z[y * n + x] as usize;
                let k = self.map.idx(x, y, sz);
                self.map.cells[k].plant = Plant::None;
                self.map.cells[k].boulder = false;
                self.map.cells[k].material = Material::Wood;
                let ring = dx == 0 || dy == 0 || dx == HUT_W - 1 || dy == HUT_H - 1;
                let door = dy == HUT_H - 1 && dx == HUT_W / 2;
                if ring && !door && sz + 1 < self.map.depth {
                    let kw = self.map.idx(x, y, sz + 1);
                    self.map.cells[kw].shape = Shape::Wall;
                    self.map.cells[kw].material = Material::Wood;
                }
                cells.push((x, y));
            }
        }
        let (cx, cy) = (at.0 as f32 + HUT_W as f32 / 2.0, at.1 as f32 + HUT_H as f32 / 2.0);
        self.map.houses.push(RoofPlan { cx, cy, axis: (1.0, 0.0), half_width: HUT_H as f32 / 2.0, stone: false });
        let id = self.map.houses.len() as u32;
        for (x, y) in cells { self.map.roofs[y * n + x] = id; }
        // Anyone standing in a wall steps out of it.
        for s in &mut self.settlers {
            if nav::cost(&self.map, s.pos.0 as usize, s.pos.1 as usize).is_none() { s.pos = (at.0 + HUT_W as u16 / 2, at.1 + HUT_H as u16); }
        }
    }

    /// Run `days` game days.
    /// Try each of the patron's verbs on a fresh colony and check it changes what the settlers do
    /// within a game day (`--sim-patron`). Returns one line per verb, "ok" or why not.
    pub fn patron_trial(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        self.run_days(1);
        let inside = |m: &PlaceMark, p: Pos| (m.at.0 as i32 - p.0 as i32).abs().max((m.at.1 as i32 - p.1 as i32).abs()) <= m.radius as i32;
        // Forbid the trees nearest the camp.
        let tree = self.nearest(self.camp, |c, p| c.is_felling_tree(p));
        let forbid = tree.map(|t| PlaceMark { at: t, radius: 8, forbidden: true });
        if let Some(m) = forbid { let _ = self.mark_place(m.at, m.radius, true); }
        // Bless a berry patch away from the camp.
        let patch = (12..40).find_map(|r| {
            let p = (self.camp.0.saturating_add(r), self.camp.1);
            self.nearest(p, |c, q| c.is_ripe_shrub(q) && (q.0 as i32 - c.camp.0 as i32).abs() + (q.1 as i32 - c.camp.1 as i32).abs() >= 12)
        });
        let bless = patch.map(|t| PlaceMark { at: t, radius: 6, forbidden: false });
        if let Some(m) = bless { let _ = self.mark_place(m.at, m.radius, false); }
        let fav = 0usize;
        let _ = self.favour_settler(fav);
        let (mut felled_inside, mut kept_off, mut forage_inside, mut fav_seen) = (0, 0, 0, false);
        let start = self.decisions.len();
        for _ in 0..TICKS_PER_DAY {
            self.tick();
            for st in &self.settlers {
                if let (Some(m), Job::Fell(t)) = (forbid, st.job) { if inside(&m, t) { felled_inside += 1; } }
                if let (Some(m), Job::Forage(t)) = (bless, st.job) { if inside(&m, t) { forage_inside += 1; } }
            }
        }
        for d in &self.decisions[start..] {
            if d.contains("keeping off the forbidden ground") { kept_off += 1; }
            if d.contains(&self.settlers[fav].name) && d.contains("(the patron's favourite)") { fav_seen = true; }
        }
        out.push(match forbid { Some(_) if felled_inside == 0 && kept_off > 0 => format!("forbid: ok ({} choices kept off it, no tree felled inside)", kept_off),
            Some(_) => format!("forbid: FAILED ({} felling ticks inside, {} choices kept off)", felled_inside, kept_off), None => "forbid: no tree to forbid".into() });
        out.push(match bless { Some(_) if forage_inside > 0 => format!("bless: ok ({} settler-minutes foraging on the blessed ground)", forage_inside),
            Some(_) => "bless: FAILED (nobody foraged there)".into(), None => "bless: no berry patch".into() });
        out.push(if fav_seen { "favourite: ok".into() } else { "favourite: FAILED".into() });
        // A dream, after the dawn gives favour back.
        let dreamer = 1usize;
        let _ = self.send_dream(dreamer, Dream::Plenty);
        let start = self.decisions.len();
        let mut dreamt = false;
        for _ in 0..TICKS_PER_DAY {
            self.tick();
            if matches!(self.settlers[dreamer].job, Job::Forage(_) | Job::Fish(_)) && self.settlers[dreamer].why.starts_with("Dreamt of") { dreamt = true; }
        }
        let _ = start;
        out.push(if dreamt { "dream: ok".into() } else { "dream: FAILED".into() });
        out
    }

    /// Where the hut stands, and how many trees are left within `r` of a point (for comparing
    /// layouts).
    pub fn trees_near(&self, at: Pos, r: i32) -> usize {
        let mut n = 0;
        for y in (at.1 as i32 - r).max(1)..(at.1 as i32 + r).min(self.map.height as i32 - 1) {
            for x in (at.0 as i32 - r).max(1)..(at.0 as i32 + r).min(self.map.width as i32 - 1) {
                if matches!(self.floor_plant((x as u16, y as u16)), Plant::Tree(_)) { n += 1; }
            }
        }
        n
    }

    /// The camp and a free spot `dx`, `dy` from it (the nearest passable cell), for trials.
    pub fn spot_from_camp(&self, dx: i32, dy: i32) -> Pos {
        let (x, y) = (self.camp.0 as i32 + dx, self.camp.1 as i32 + dy);
        for r in 0..10 {
            for (ox, oy) in [(0, 0), (r, 0), (-r, 0), (0, r), (0, -r), (r, r), (-r, -r)] {
                let p = ((x + ox).clamp(2, self.map.width as i32 - 3) as u16, (y + oy).clamp(2, self.map.height as i32 - 3) as u16);
                if nav::passable(&self.map, p) { return p; }
            }
        }
        self.camp
    }

    /// The nearest tree to the camp that would be felled.
    pub fn nearest_tree_to_camp(&self) -> Option<Pos> { self.nearest(self.camp, |c, p| c.is_felling_tree(p)) }

    /// Replay one recorded act ("tick verb args", as `interventions` writes them); the tick is
    /// the caller's business. Returns what happened, or why not.
    pub fn apply_intervention(&mut self, line: &str) -> Result<String, String> {
        let w: Vec<&str> = line.split_whitespace().collect();
        let num = |k: usize| w.get(k).and_then(|x| x.parse::<u16>().ok()).ok_or_else(|| format!("bad intervention: {line}"));
        match w.get(1).copied() {
            Some("bless") | Some("forbid") => self.mark_place((num(2)?, num(3)?), num(4)?, w[1] == "forbid"),
            Some("favour") => self.favour_settler(num(2)? as usize),
            Some("dream") => {
                let d = match w.get(3).copied() { Some("Hut") => Dream::Hut, Some("Plenty") => Dream::Plenty, _ => Dream::Rest };
                self.send_dream(num(2)? as usize, d)
            }
            Some("stone") => {
                let k = match w.get(2).copied() { Some("Hall") => StoneKind::Hall, Some("Grove") => StoneKind::Grove, _ => StoneKind::Shrine };
                self.place_stone(k, (num(3)?, num(4)?))
            }
            Some("name") => { let n = w[2..].join(" "); self.name_colony(&n); Ok(n) }
            _ => Err(format!("unknown intervention: {line}")),
        }
    }

    /// Live `days` days, applying the scripted interventions ("tick verb args") at their ticks.
    pub fn run_days_scripted(&mut self, days: u64, script: &[String]) {
        let end = self.clock.tick + days * TICKS_PER_DAY;
        while self.clock.tick < end {
            // Every line whose tick has come and not yet run (lines timed before the colony's
            // first morning run at once).
            let now = self.clock.tick;
            for line in script {
                let t = line.split_whitespace().next().and_then(|t| t.parse::<u64>().ok()).unwrap_or(u64::MAX);
                if t >= self.script_at && t <= now {
                    if let Err(e) = self.apply_intervention(line) { self.note(format!("(intervention failed: {e})")); }
                }
            }
            self.script_at = now + 1;
            self.tick();
        }
    }

    pub fn run_days(&mut self, days: u64) {
        let end = self.clock.tick + days * TICKS_PER_DAY;
        let mut warned_food = false;
        while self.clock.tick < end {
            self.tick();
            if self.clock.tick % TICKS_PER_DAY == 6 * 60 {
                let hut = match &self.hut { Some(h) if h.done => "the hut stands".to_string(), Some(h) => format!("the hut has {} of {} logs", h.logs_used, HUT_LOGS), None => "no hut site".into() };
                let hungry = self.settlers.iter().filter(|s| s.alive && s.hunger > 0.7).count();
                let chilled = self.settlers.iter().filter(|s| s.alive && s.exposure > 0.6).count();
                self.note(format!("Dawn: {} of {} alive, {} meals at the camp, {} logs stored, {}{}{}.",
                    self.alive(), self.settlers.len(), self.food_stored(), self.logs_stored(), hut,
                    if hungry > 0 { format!(", {} hungry", hungry) } else { String::new() },
                    if chilled > 0 { format!(", {} chilled from the night", chilled) } else { String::new() }));
            }
            if self.clock.tick % 60 == 0 {
                let low = self.food_stored() == 0;
                if low && !warned_food { self.note("The camp has no food left.".into()); }
                warned_food = low;
            }
        }
    }

    /// One line per settler: who, what, why.
    pub fn roll_call(&self) -> Vec<String> {
        self.settlers.iter().map(|s| if s.alive {
            format!("{:<9} {:<16} hunger {:>3.0}% rest {:>3.0}% chill {:>3.0}%  {}", s.name, s.job.verb(), s.hunger * 100.0, (1.0 - s.fatigue) * 100.0, s.exposure * 100.0, s.why)
        } else { format!("{:<9} dead", s.name) }).collect()
    }
}

fn join_names(v: &[String]) -> String {
    match v {
        [] => "no one".into(),
        [a] => a.clone(),
        [rest @ .., last] => format!("{} and {}", rest.join(", "), last),
    }
}

fn lower_first(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_lowercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}
