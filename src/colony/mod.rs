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
}

/// The hut the settlers build together.
#[derive(Clone, Debug)]
pub struct Hut {
    /// Top-left corner of the footprint.
    pub at: Pos,
    pub logs_used: u32,
    pub done: bool,
}

pub struct Colony {
    pub map: LocalMap,
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
            }
        }).collect::<Vec<_>>();
        // They arrive with two days of food.
        let items = (0..names.len() * 2).map(|_| Item { kind: ItemKind::Food, at: camp, stored: true, reserved: false }).collect();
        let mut c = Colony {
            map, clock: Clock { tick: 6 * 60 }, settlers, items, camp, hut: None,
            shrub_ready: Default::default(), claimed: Default::default(), unreachable: Default::default(),
            log: Vec::new(), decisions: Vec::new(), rng, milestones: Default::default(), basket: Default::default(),
        };
        c.hut = c.find_hut_site().map(|at| Hut { at, logs_used: 0, done: false });
        let site = c.hut.as_ref().map(|h| format!(" and a hut site at {},{}", h.at.0, h.at.1)).unwrap_or_default();
        c.note(format!("{} settlers make camp at {},{}{}. They carry two days of food.", names.len(), camp.0, camp.1, site));
        c
    }

    fn note(&mut self, line: String) { self.log.push(format!("{}  {}", self.clock.stamp(), line)); }
    fn once(&mut self, key: &'static str, line: String) { if self.milestones.insert(key) { self.note(line); } }

    pub fn food_stored(&self) -> u32 { self.items.iter().filter(|i| i.kind == ItemKind::Food && i.stored).count() as u32 }
    pub fn logs_stored(&self) -> u32 { self.items.iter().filter(|i| i.kind == ItemKind::Log && i.stored).count() as u32 }
    pub fn alive(&self) -> usize { self.settlers.iter().filter(|s| s.alive).count() }

    /// A flat, dry 6x5 patch near the camp for the hut.
    fn find_hut_site(&self) -> Option<Pos> {
        let n = self.map.width as i32;
        let (cx, cy) = (self.camp.0 as i32, self.camp.1 as i32);
        let mut best: Option<(i32, Pos)> = None;
        for y in (cy - 14).max(2)..(cy + 14).min(n - HUT_H as i32 - 2) {
            for x in (cx - 14).max(2)..(cx + 14).min(n - HUT_W as i32 - 2) {
                // Not on the camp itself.
                if (x..x + HUT_W as i32).contains(&cx) && (y..y + HUT_H as i32).contains(&cy) { continue; }
                let z0 = self.map.surface_z[(y * n + x) as usize];
                let ok = (0..HUT_H as i32).all(|dy| (0..HUT_W as i32).all(|dx| {
                    let (xx, yy) = (x + dx, y + dy);
                    self.map.surface_z[(yy * n + xx) as usize] == z0 && nav::passable(&self.map, (xx as u16, yy as u16))
                }));
                if !ok { continue; }
                let d = (x - cx).abs() + (y - cy).abs();
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
        let spots: Vec<Pos> = (0..8).map(|_| (self.camp.0.saturating_add_signed(self.rng.gen_range(-6..=6)), self.camp.1.saturating_add_signed(self.rng.gen_range(-6..=6)))).collect();
        let wander_to = spots.into_iter().find(|&p| nav::passable(&self.map, p) && !self.in_hut(p)).unwrap_or(self.camp);
        options.push((0.05, Job::Wander(wander_to), "Nothing needs doing; resting near the fire".into()));

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
                    if self.claimed.contains(&p) || self.unreachable.contains(&p) { continue; }
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
        if used == 1 { self.note(format!("{} sets the first log of the hut.", name)); }
        if used >= HUT_LOGS {
            self.raise_hut();
            self.note(format!("{} finishes the hut. Tonight they sleep under a roof.", name));
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

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}
