//! An adventurer that plays itself, for the tests and for balance: explores floor by floor,
//! fights what it meets, loots, rests, drinks when hurt, goes back to town to sell, heal, buy,
//! take a calling and quests, and walks the world to the next place it can face.

use super::actor::Role;
use super::game::{Action, Game};
use super::hero::{Skill, Slot};
use super::map::{Feature, DIRS8};
use super::npc::Topic;
use super::quest::State;
use super::site::SiteKind;
use super::land::LAND;
use std::collections::{HashSet, VecDeque};

#[derive(Default, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Bot {
    /// Places it has finished (boss dead or every floor walked).
    pub done: HashSet<u32>,
    /// Errands done this town visit (role words).
    errands: HashSet<String>,
    /// Going home to town.
    homeward: bool,
    pub target: Option<u32>,
    pub stuck: u32,
    /// What it chose last (for the trace).
    pub why: &'static str,
    /// The monster it is after (kept while it lives and is near, seen or not).
    hunt: Option<u32>,
    /// Turns spent on the quarry without hurting it, and the quarries given up (until the turn).
    hunt_turns: (u32, i32),
    ignore: std::collections::HashMap<u32, u64>,
    /// The cell it stood on before its last step.
    prev_cell: Option<(i32, i32, usize)>,
    /// Leaving this place for good (climbing out).
    leaving: Option<u32>,
    /// Deaths per place, and the level at the last.
    died: std::collections::HashMap<u32, (u32, u32)>,
    /// The place it was in at the last act (to notice arriving).
    was_in: Option<u32>,
    deaths_seen: u32,
    last: (i32, i32, usize, Option<u32>),
    /// Floors walked out fully (place, z).
    walked: HashSet<(u32, usize)>,
    /// Whether the last `talk_*` spoke (not only walked toward them).
    #[serde(skip)]
    talked: bool,
    /// Times it set out after each tale (quest id): given up after a few.
    #[serde(skip)]
    tale_tries: std::collections::HashMap<u32, u32>,
    /// The town whose errands it is about (kept until it leaves them done, whatever tile the
    /// border puts it on).
    #[serde(skip)]
    visiting: Option<u32>,
    /// Steps walked toward a place's way in on the land (site, steps): given up past 200.
    #[serde(skip)]
    walk_in: (u32, u32),
    /// Searches made on each floor (place, z).
    #[serde(skip)]
    searches: std::collections::HashMap<(u32, usize), u32>,
    /// Steps walked toward a hidden door's wall (given up after 300).
    search_walk: std::collections::HashMap<(u32, usize), u32>,
}

/// First step from the hero toward the nearest cell where `goal` holds (8-way BFS; doors that
/// open count as passable). None when none is reachable.
fn path_to(g: &Game, goal: &dyn Fn(i32, i32) -> bool, max: usize) -> Option<(i32, i32)> { path_avoiding(g, goal, max, None, true) }

/// `path_to`, never through `avoid` (the cell it just left: a quarry dodging between two cells
/// made it turn back and forth between two routes).
/// (`into`: a way in, out, up or down may be the goal; false when the goal is only somewhere to
/// stand, such as beside a foe.)
fn path_avoiding(g: &Game, goal: &dyn Fn(i32, i32) -> bool, max: usize, avoid: Option<(i32, i32)>, into: bool) -> Option<(i32, i32)> {
    let f = g.floor()?;
    let p = g.place()?;
    let keys: Vec<u32> = g.hero.pack.iter().filter(|i| i.id == "key").map(|i| i.tag).collect();
    let lvl = g.hero.level;
    let rope = g.hero.count("rope") > 0;
    let pass = |x: i32, y: i32| -> bool {
        let t = f.at(x, y);
        if let Feature::LevelDoor { level } = t.feature { return lvl >= level; }
        // (Without a rope a rope spot is only floor: a pocket below a hole joined by it stranded the bot.)
        if t.feature == Feature::RopeSpot && !rope { return true; }
        // Ways in, up and down carry one off: only stepped on when they are the goal.
        if matches!(t.feature, Feature::Entrance { .. } | Feature::StairsDown | Feature::StairsUp | Feature::LadderDown | Feature::LadderUp | Feature::Hole | Feature::Exit | Feature::Grate | Feature::RopeSpot) { return false; }
        if t.walkable() { return true; }
        match &t.feature { Feature::Door { lock, .. } => *lock == 0 || keys.contains(lock), _ => false }
    };
    // (Monsters do not block: walking into one strikes it. People do.)
    let blocked_by = |x: i32, y: i32| p.npcs.iter().any(|n| n.z == g.z && n.x == x && n.y == y);
    let mut prev = vec![u32::MAX; f.w * f.h];
    let start = (g.x, g.y);
    let idx = |x: i32, y: i32| y as usize * f.w + x as usize;
    prev[idx(start.0, start.1)] = idx(start.0, start.1) as u32;
    let mut q = VecDeque::new();
    q.push_back(start);
    let mut n = 0;
    while let Some((x, y)) = q.pop_front() {
        n += 1;
        if n > max { break; }
        if (x, y) != start && goal(x, y) && f.at(x, y).walkable() {
            // Walk back to the first step.
            let (mut cx, mut cy) = (x, y);
            loop {
                let pk = prev[idx(cx, cy)] as usize;
                let (px, py) = ((pk % f.w) as i32, (pk / f.w) as i32);
                if (px, py) == start { return Some((cx - start.0, cy - start.1)); }
                cx = px; cy = py;
            }
        }
        for (dx, dy) in DIRS8 {
            let (nx, ny) = (x + dx, y + dy);
            if !f.inside(nx, ny) || prev[idx(nx, ny)] != u32::MAX { continue; }
            if dx != 0 && dy != 0 && !f.at(x + dx, y).walkable() && !f.at(x, y + dy).walkable() { continue; }
            // A goal that blocks must be something a bump acts on (a chest, a lever, a door, a
            // person); a barrel beside a monster is no way to reach it.
            let bumpable = matches!(f.at(nx, ny).feature, Feature::Chest { opened: false, .. } | Feature::Sarcophagus { opened: false, .. } | Feature::QuestChest { taken: false, .. } | Feature::Plinth { item: Some(_) } | Feature::Lever { .. } | Feature::Door { open: false, .. })
                || p.npcs.iter().any(|n| n.z == g.z && n.x == nx && n.y == ny);
            if Some((nx, ny)) == avoid { continue; }
            let transit = matches!(f.at(nx, ny).feature, Feature::Entrance { .. } | Feature::StairsDown | Feature::StairsUp | Feature::LadderDown | Feature::LadderUp | Feature::Hole | Feature::Exit | Feature::Grate) || (rope && f.at(nx, ny).feature == Feature::RopeSpot);
            let is_goal = goal(nx, ny) && ((pass(nx, ny) || (transit && into)) && !blocked_by(nx, ny) || bumpable);
            if !is_goal && (!pass(nx, ny) || blocked_by(nx, ny)) { continue; }
            prev[idx(nx, ny)] = idx(x, y) as u32;
            if is_goal && !pass(nx, ny) {
                // A goal one bumps (a chest, a person): stop at it.
                let (mut cx, mut cy) = (nx, ny);
                loop {
                    let pk = prev[idx(cx, cy)] as usize;
                    let (px, py) = ((pk % f.w) as i32, (pk / f.w) as i32);
                    if (px, py) == start { return Some((cx - start.0, cy - start.1)); }
                    cx = px; cy = py;
                }
            }
            q.push_back((nx, ny));
        }
    }
    None
}

fn score(it: &super::item::Item) -> i32 {
    let d = it.def();
    match d.kind.as_str() { "weapon" if d.range == 0 || d.thrown => it.attack() * 2 + it.defense() - if d.two_handed { 6 } else { 0 }, "shield" => it.defense(), "armour" | "jewel" => it.armor() * 3 + d.hp / 3 + d.melee * 4, _ => -1000 }
}

impl Bot {
    /// One act. Returns false when it found nothing to do.
    pub fn step(&mut self, g: &mut Game) -> bool {
        if g.banner.is_some() { g.banner = None; self.errands.clear(); self.homeward = false; }
        if g.stats.deaths > self.deaths_seen {
            self.deaths_seen = g.stats.deaths;
            if let Some(t) = self.target { let e = self.died.entry(t).or_insert((0, 0)); e.0 += 1; e.1 = g.hero.level; }
            self.homeward = false;
        }
        // A tale's choice: the first way (the one that goes on with the work).
        if let Some(c) = &g.choice {
            self.why = "decide";
            // (A riddle it answers right: the bot has read the books.)
            let k = if c.riddle.is_some() { c.options.iter().position(|o| o.1 == 1).unwrap_or(0) } else { 0 };
            return g.act(Action::Decide(k));
        }
        let pos = (g.x, g.y, g.z, g.here);
        if pos == self.last { self.stuck += 1; } else { self.stuck = 0; if self.last.3 == pos.3 { self.prev_cell = Some((self.last.0, self.last.1, self.last.2)); } else { self.prev_cell = None; } self.last = pos; }
        if g.talk.is_some() { g.talk = None; }
        let r = match g.here {
            None => self.on_road(g),
            Some(LAND) => self.on_land(g),
            Some(id) => self.below(g, id),
        };
        self.was_in = g.here;
        r
    }

    fn need_town(&self, g: &Game) -> bool {
        let loot: u32 = g.hero.pack.iter().filter(|i| i.def().kind == "loot").map(|i| i.value()).sum();
        g.hero.count("health_potion") + g.hero.count("strong_health_potion") == 0 && g.hero.gold() >= 60
            || loot > 250 || (g.hero.level >= 8 && g.hero.calling.is_none())
            || g.hero.fed < 300 && g.hero.gold() >= 10 && g.hero.pack.iter().all(|i| i.def().kind != "food")
    }

    fn on_road(&mut self, g: &mut Game) -> bool {
        self.errands.clear();
        let home = g.site(g.hero.temple).map(|s| s.tile).unwrap_or(g.tile);
        let level = g.hero.level;
        let w = g.world.w;
        // A parcel to deliver, or work done to report: to that town first.
        let parcel = g.quests.iter().find_map(|q| match (&q.goal, q.state) { (super::quest::Goal::Deliver { town, .. }, State::Open) => g.site(*town).map(|s| (s.id, s.tile)), _ => None })
            .or_else(|| g.quests.iter().find(|q| q.state == State::Done).and_then(|q| g.site(q.town)).map(|s| (s.id, s.tile)));
        if let (Some((pid, ptile)), false) = (parcel, self.need_town(g)) {
            if g.tile == ptile { return g.act(Action::EnterSite(pid)); }
            if let Some((dx, dy)) = g.world.step_toward(g.tile, ptile) { if g.act(Action::Travel(dx, dy)) { return true; } }
        }
        // A tale to follow: to its place or its tile.
        if !self.need_town(g) {
            if let Some((qid, site, tile)) = self.tale_goal(g) {
                if site != 0 { self.target = Some(site); self.done.remove(&site); }
                if g.tile == tile {
                    let n = self.tale_tries.entry(qid).or_insert(0);
                    *n += 1;
                    if *n > 6 {
                        // Given up: it cannot be done the way it knows.
                        if let Some(q) = g.quests.iter_mut().find(|q| q.id == qid) { q.state = State::Failed; }
                        return g.act(Action::Wait);
                    }
                    return if site != 0 { g.act(Action::EnterSite(site)) } else { g.act(Action::Land) };
                }
                if let Some((dx, dy)) = g.world.step_toward(g.tile, tile) { if g.act(Action::Travel(dx, dy)) { return true; } }
            }
        }
        let dest = if self.homeward || self.need_town(g) { self.homeward = true; Some(home) } else {
            let pick = g.sites.iter().filter(|s| s.kind != SiteKind::Town && s.kind != SiteKind::Wilds && s.kind != SiteKind::Cellar && !self.done.contains(&s.id) && s.tier.saturating_sub(1) * 8 <= level && self.died.get(&s.id).map_or(true, |d| d.0 < 2 || level >= d.1 + 3))
                .min_by_key(|s| { let dx = (s.tile.0 as i32 - g.tile.0 as i32).abs(); ((dx.min(w as i32 - dx)).max((s.tile.1 as i32 - g.tile.1 as i32).abs()), s.id) }).map(|s| (s.id, s.tile));
            // Nothing new it can face: back to a place already walked, where things have come back.
            let pick = pick.or_else(|| g.sites.iter().filter(|s| self.done.contains(&s.id) && s.kind != SiteKind::Town && s.kind != SiteKind::Wilds && s.kind != SiteKind::Cellar && s.tier.saturating_sub(1) * 8 <= level && self.died.get(&s.id).map_or(true, |d| d.0 < 2 || level >= d.1 + 3))
                .min_by_key(|s| { let dx = (s.tile.0 as i32 - g.tile.0 as i32).abs(); ((dx.min(w as i32 - dx)).max((s.tile.1 as i32 - g.tile.1 as i32).abs()), s.id) }).map(|s| (s.id, s.tile)));
            if let Some((id, _)) = pick { self.done.remove(&id); }
            self.target = pick.map(|p| p.0);
            pick.map(|p| p.1).or(Some(home))
        };
        let dest = dest.unwrap();
        if std::env::var("PLANET_ADV_TRACE").is_ok() { eprintln!("   road: at {:?} dest {:?} target {:?} homeward {} need_town {} parcel {:?} potions {} gold {} fed {} food {} loot {}", g.tile, dest, self.target.and_then(|t| g.site(t)).map(|s| (s.name.clone(), s.tier)), self.homeward, self.need_town(g), parcel, g.hero.count("health_potion"), g.hero.gold(), g.hero.fed, g.hero.pack.iter().any(|i| i.def().kind == "food"), g.hero.pack.iter().filter(|i| i.def().kind == "loot").map(|i| format!("{}={}", i.id, i.value())).collect::<Vec<_>>().join(",")); }
        if g.tile == dest {
            let id = if self.homeward { g.hero.temple } else { self.target.unwrap_or(g.hero.temple) };
            self.homeward = false;
            return g.act(Action::EnterSite(id));
        }
        // Over land by the shortest way (the coast in the way had stopped it).
        if let Some((dx, dy)) = g.world.step_toward(g.tile, dest) { if g.act(Action::Travel(dx, dy)) { return true; } }
        self.done.extend(self.target);
        false
    }

    /// Talk to the one of this name here (as `talk_to`).
    fn talk_named(&mut self, g: &mut Game, name: &str, want: &dyn Fn(&Topic) -> bool, then: &dyn Fn(&Topic) -> bool) -> Option<bool> {
        let p = g.place()?;
        let k = p.npcs.iter().position(|n| n.name == name)?;
        let role = p.npcs[k].role;
        self.talk_k(g, k, role, want, then)
    }

    fn talk_to(&mut self, g: &mut Game, role: Role, want: &dyn Fn(&Topic) -> bool, then: &dyn Fn(&Topic) -> bool) -> Option<bool> {
        let p = g.place()?;
        let here = self.visiting.unwrap_or_else(|| g.site_here());
        // This town's own (a neighbouring town's people share the land floor).
        let k = p.npcs.iter().position(|n| n.role == role && n.home == here && n.of != "drunk" && n.of != "farmer")?;
        self.talk_k(g, k, role, want, then)
    }

    fn talk_k(&mut self, g: &mut Game, k: usize, role: Role, want: &dyn Fn(&Topic) -> bool, then: &dyn Fn(&Topic) -> bool) -> Option<bool> {
        let p = g.place()?;
        let (nx, ny) = (p.npcs[k].x, p.npcs[k].y);
        if (nx - g.x).abs() <= 1 && (ny - g.y).abs() <= 1 {
            super::npc::greet(g, k);
            for _ in 0..4 {
                let Some(t) = g.talk.clone() else { break };
                if let Some(i) = t.options.iter().position(|(_, tp)| want(tp)) { super::npc::answer(g, i); }
                else { break; }
                if let Some(t) = g.talk.clone() { if let Some(i) = t.options.iter().position(|(_, tp)| then(tp)) { super::npc::answer(g, i); } }
            }
            g.talk = None;
            self.errands.insert(role.word().to_string());
            self.talked = true;
            return Some(true);
        }
        let step = path_to(g, &|x, y| x == nx && y == ny, 40_000)?;
        Some(g.act(Action::Move(step.0, step.1)))
    }

    /// On the land: a town's errands in a town (unless something is at it); else fight what
    /// comes, then walk to the way into the place it came for, or take to the road.
    fn on_land(&mut self, g: &mut Game) -> bool {
        let threat = g.place().map_or(false, |p| p.monsters.iter().any(|m| m.hp > 0 && m.awake && g.visible(m.x, m.y) && (m.x - g.x).abs().max((m.y - g.y).abs()) <= 8));
        if std::env::var("PLANET_ADV_TOWN").is_ok() && threat { let near: Vec<String> = g.place().map(|p| p.monsters.iter().filter(|m| m.hp > 0 && m.awake && g.visible(m.x, m.y) && (m.x - g.x).abs().max((m.y - g.y).abs()) <= 8).map(|m| format!("{}@{},{}", m.def, m.x, m.y)).collect()).unwrap_or_default(); eprintln!("   threat at {},{}: {:?}", g.x, g.y, near); }
        if !threat && g.place().map_or(false, |p| p.spec.kind == SiteKind::Town) { let id = g.site_here(); self.visiting = Some(id); return self.in_town(g, id); }
        if let Some(t) = self.visiting.filter(|t| !threat && g.site(*t).map_or(false, |s| s.kind == SiteKind::Town && super::world::dist(s.tile, g.tile, g.world.w) <= 1)) { return self.in_town(g, t); }
        self.visiting = None;
        self.below(g, LAND)
    }

    /// The way on from the land: into the place it came for (its way in on this land), else
    /// the road.
    /// The open tale to follow now: (its place or 0, its tile).
    fn tale_goal(&self, g: &Game) -> Option<(u32, u32, (usize, usize))> {
        use super::tales::TaleKind as K;
        let lvl = g.hero.level;
        let able = |site: u32| g.site(site).map_or(true, |s| s.tier.saturating_sub(1) * 8 <= lvl) && self.died.get(&site).map_or(true, |d| d.0 < 2);
        g.quests.iter().filter(|q| q.state == State::Open).find_map(|q| match &q.goal {
            super::quest::Goal::Tale(t) => match (t.kind, t.stage) {
                (K::Snatched, _) | (K::Plague, _) | (K::Cult, 0) if able(t.site) => Some((q.id, t.site, g.site(t.site).map(|s| s.tile).unwrap_or(t.tile))),
                (K::Caravan, 0) | (K::Caravan, 1) | (K::Tribute, 0) => Some((q.id, 0, t.tile)),
                _ => None,
            },
            _ => None,
        })
    }

    fn land_way(&mut self, g: &mut Game) -> bool {
        // A caravan's goods lying here: pick them up.
        let goods: Vec<u32> = g.quests.iter().filter(|q| q.state == State::Open).filter_map(|q| match &q.goal { super::quest::Goal::Tale(t) if t.kind == super::tales::TaleKind::Caravan && t.stage == 1 && t.tile == g.tile => Some(t.tag), _ => None }).collect();
        if let (Some(&tag), Some(f)) = (goods.first(), g.floor()) {
            let at: Vec<(i32, i32)> = f.items.iter().filter(|(_, v)| v.iter().any(|i| i.tag == tag)).map(|(k, _)| *k).collect();
            if let Some(&(x, y)) = at.first() {
                if (g.x, g.y) == (x, y) { self.why = "goods"; return g.act(Action::PickUp); }
                if let Some(step) = path_to(g, &|a, b| (a, b) == (x, y), 20_000) { self.why = "to the wreck"; return g.act(Action::Move(step.0, step.1)); }
            }
        }
        if let Some(t) = self.target.filter(|t| !self.done.contains(t) && !self.homeward && !self.need_town(g)) {
            if g.site(t).map_or(false, |s| s.tile == g.tile || super::world::dist(s.tile, g.tile, g.world.w) <= 1) {
                let f = g.floor().unwrap();
                let is_in = |x: i32, y: i32| matches!(f.at(x, y).feature, Feature::Entrance { site, .. } if site == t);
                if is_in(g.x, g.y) { self.why = "go in"; return g.act(Action::Climb); }
                if self.walk_in.0 != t { self.walk_in = (t, 0); }
                self.walk_in.1 += 1;
                if self.walk_in.1 <= 200 { if let Some(step) = path_to(g, &is_in, 40_000) { self.why = "to the way in"; return g.act(Action::Move(step.0, step.1)); } }
                self.done.insert(t);
            }
        }
        self.why = "road";
        if g.act(Action::WorldMap) { return true; }
        g.act(Action::Wait)
    }

    fn in_town(&mut self, g: &mut Game, id: u32) -> bool {
        if std::env::var("PLANET_ADV_TOWN").is_ok() {
            let roles: Vec<String> = g.place().map(|p| p.npcs.iter().filter(|n| n.home == id).map(|n| format!("{:?}", n.role)).collect()).unwrap_or_default();
            let loot: Vec<String> = g.hero.pack.iter().filter(|i| i.def().kind == "loot").map(|i| format!("{}x{}={}", i.id, i.count, i.value())).collect();
            let tr = g.place().and_then(|p| p.npcs.iter().find(|n| n.role == Role::Trader && n.home == id).map(|n| (n.x, n.y)));
            let reach = tr.and_then(|(tx, ty)| path_to(g, &|x, y| (x, y) == (tx, ty), 40_000)).is_some();
            eprintln!("   town {} at {},{}: gold {} fed {} need {} loot {:?} trader {:?} reachable {} errands {:?}", g.site(id).map(|s| s.name.clone()).unwrap_or_default(), g.x, g.y, g.hero.gold(), g.hero.fed, self.need_town(g), loot, tr, reach, self.errands);
        }
        self.homeward = false;
        let hp_low = g.hero.hp < g.hero.max_hp() * 9 / 10 || g.hero.poisoned > 0;
        // Errands, in order.
        if !self.errands.contains("trader") {
            let gold = g.hero.gold();
            let want_potions = (gold / 120).min(6);
            if let Some(r) = self.talk_to(g, Role::Trader, &|t| matches!(t, Topic::Trade), &|t| matches!(t, Topic::SellLoot)) {
                // Buy potions and food after selling.
                if self.errands.contains("trader") {
                    if let Some(k) = g.place().and_then(|p| p.npcs.iter().position(|n| n.role == Role::Trader && Some(n.home) == self.visiting.or(Some(g.site_here())))) {
                        super::npc::greet(g, k);
                        if let Some(i) = g.talk.as_ref().and_then(|t| t.options.iter().position(|(_, tp)| matches!(tp, Topic::Trade))) { super::npc::answer(g, i); }
                        for _ in 0..want_potions.max(1) { if let Some(i) = g.talk.as_ref().and_then(|t| t.options.iter().position(|(_, tp)| matches!(tp, Topic::Buy(id, _, _) if id == "health_potion"))) { super::npc::answer(g, i); } }
                        if g.hero.count("bread") + g.hero.count("meat") < 3 { for _ in 0..3 { if let Some(i) = g.talk.as_ref().and_then(|t| t.options.iter().position(|(_, tp)| matches!(tp, Topic::Buy(id, _, _) if id == "bread"))) { super::npc::answer(g, i); } } }
                        if g.hero.count("torch") < 2 { if let Some(i) = g.talk.as_ref().and_then(|t| t.options.iter().position(|(_, tp)| matches!(tp, Topic::Buy(id, _, _) if id == "torch"))) { super::npc::answer(g, i); super::npc::answer(g, i); } }
                        let wand = match g.hero.calling.as_deref() { Some("sorcerer") => Some("wand_of_embers"), Some("druid") => Some("snakebite_rod"), _ => None };
                        if let Some(wd) = wand { if g.hero.weapon().map_or(true, |w| w.id != wd) && !g.hero.pack.iter().any(|i| i.id == wd) { if let Some(i) = g.talk.as_ref().and_then(|t| t.options.iter().position(|(_, tp)| matches!(tp, Topic::Buy(id, _, _) if id == wd))) { super::npc::answer(g, i); } } }
                        if g.hero.calling.as_deref() == Some("paladin") {
                            if !g.hero.pack.iter().any(|i| i.id == "bow") && g.hero.weapon().map_or(true, |w| w.id != "bow") { if let Some(i) = g.talk.as_ref().and_then(|t| t.options.iter().position(|(_, tp)| matches!(tp, Topic::Buy(id, _, _) if id == "bow"))) { super::npc::answer(g, i); } }
                            for _ in 0..6 { if g.hero.count("arrow") >= 80 || g.hero.gold() < 40 { break; } if let Some(i) = g.talk.as_ref().and_then(|t| t.options.iter().position(|(_, tp)| matches!(tp, Topic::Buy(id, _, _) if id == "arrow"))) { super::npc::answer(g, i); } }
                        }
                        let mine = match g.hero.calling.as_deref() { Some("sorcerer") => Some("wand_of_embers"), Some("druid") => Some("snakebite_rod"), Some("paladin") => Some("bow"), _ => None };
                        if let Some(wd) = mine { if let Some(k) = g.hero.pack.iter().position(|i| i.id == wd) { let _ = g.hero.equip(k); } }
                        // A parcel to carry, from level 6 (once the sewers are behind).
                        if g.hero.level >= 6 && !g.quests.iter().any(|q| q.state == super::quest::State::Open && matches!(q.goal, super::quest::Goal::Deliver { .. })) {
                            g.talk = None;
                            super::npc::greet(g, k);
                            if let Some(i) = g.talk.as_ref().and_then(|t| t.options.iter().position(|(_, tp)| matches!(tp, Topic::Quest))) { super::npc::answer(g, i); }
                            if let Some(i) = g.talk.as_ref().and_then(|t| t.options.iter().position(|(_, tp)| matches!(tp, Topic::Accept))) { super::npc::answer(g, i); }
                        }
                        if g.hero.count("rope") == 0 && g.hero.gold() > 60 { if let Some(i) = g.talk.as_ref().and_then(|t| t.options.iter().position(|(_, tp)| matches!(tp, Topic::Buy(id, _, _) if id == "rope"))) { super::npc::answer(g, i); } }
                        g.talk = None;
                    }
                }
                return r;
            }
        }
        if !self.errands.contains("priest") && (hp_low || (g.hero.fed < 300 && g.hero.gold() < 10) || (!g.hero.blessed && g.hero.gold() > g.hero.blessing_price() * 3) || (g.hero.level >= 8 && g.hero.calling.is_none()) || g.hero.may_learn().iter().any(|s| s.level <= g.hero.level && 40 * s.level.max(1) <= g.hero.gold() / 2)) {
            let Some(k) = g.place().and_then(|p| p.npcs.iter().position(|n| n.role == Role::Priest && Some(n.home) == self.visiting.or(Some(g.site_here())))) else { return false };
            let (nx, ny) = { let n = &g.place().unwrap().npcs[k]; (n.x, n.y) };
            if (nx - g.x).abs() > 1 || (ny - g.y).abs() > 1 {
                if let Some(step) = path_to(g, &|x, y| x == nx && y == ny, 40_000) { return g.act(Action::Move(step.0, step.1)); }
                self.errands.insert("priest".into());
                return false;
            }
            super::npc::greet(g, k);
            let pick = |g: &Game, f: &dyn Fn(&Topic) -> bool| g.talk.as_ref().and_then(|t| t.options.iter().position(|(_, tp)| f(tp)));
            if hp_low { if let Some(i) = pick(g, &|t| matches!(t, Topic::Heal)) { super::npc::answer(g, i); } }
            if g.hero.level >= 8 && g.hero.calling.is_none() {
                if let Some(i) = pick(g, &|t| matches!(t, Topic::Calling)) { super::npc::answer(g, i); }
                let want = std::env::var("PLANET_ADV_CALLING").unwrap_or_else(|_| "knight".into());
                if let Some(i) = pick(g, &|t| matches!(t, Topic::Become(c) if *c == want)) { super::npc::answer(g, i); }
            }
            if !g.hero.blessed && g.hero.gold() > g.hero.blessing_price() * 3 { if let Some(i) = pick(g, &|t| matches!(t, Topic::Bless(_))) { super::npc::answer(g, i); } }
            if let Some(i) = pick(g, &|t| matches!(t, Topic::Spells)) { super::npc::answer(g, i); }
            for _ in 0..6 {
                let afford = g.hero.gold() / 2;
                let lvl = g.hero.level;
                let Some(i) = pick(g, &|t| matches!(t, Topic::Learn(id, price) if *price <= afford && super::data::data().spell(id).map_or(false, |s| s.level <= lvl))) else { break };
                super::npc::answer(g, i);
            }
            g.talk = None;
            self.errands.insert("priest".into());
            let mine = match g.hero.calling.as_deref() { Some("sorcerer") => Some("wand_of_embers"), Some("druid") => Some("snakebite_rod"), Some("paladin") => Some("bow"), _ => None };
            if let Some(wd) = mine { if g.hero.weapon().map_or(true, |w| w.id != wd) { if let Some(k) = g.hero.pack.iter().position(|i| i.id == wd) { let _ = g.hero.equip(k); } } }
            return true;
        }
        if !self.errands.contains("smith") {
            // Sell old gear, buy the best weapon and armour that fits the purse.
            if let Some(r) = self.talk_to(g, Role::Smith, &|t| matches!(t, Topic::Trade), &|t| matches!(t, Topic::SellGear)) {
                if self.errands.contains("smith") { self.shop_smith(g); }
                return r;
            }
        }
        if !self.errands.contains("innkeeper") && g.companion.is_none() && g.hero.level >= 10 && g.hero.gold() > 80 * g.hero.level * 4 {
            if let Some(r) = self.talk_to(g, Role::Innkeeper, &|t| matches!(t, Topic::Hire(_)), &|_| false) { return r; }
        }
        // Tales: done ones reported to whoever gave them; a feud's other house; a townsperson's
        // trouble and the priest's.
        let report: Option<String> = g.quests.iter().find(|q| q.state == State::Done && q.town == id && matches!(q.goal, super::quest::Goal::Tale(_))).map(|q| q.giver.clone());
        if let Some(giver) = report { if let Some(r) = self.talk_named(g, &giver, &|t| matches!(t, Topic::Report(_)), &|_| false) { return r; } }
        let feud: Option<String> = g.quests.iter().find_map(|q| match &q.goal { super::quest::Goal::Tale(t) if q.state == State::Open && q.town == id && t.kind == super::tales::TaleKind::Feud && t.stage == 0 => Some(t.other.clone()), _ => None });
        if let Some(other) = feud { if !self.errands.contains("feud") { self.errands.insert("feud".into()); if let Some(r) = self.talk_named(g, &other, &|_| false, &|_| false) { return r; } } }
        let open_tales = g.quests.iter().filter(|q| q.state == State::Open && matches!(q.goal, super::quest::Goal::Tale(_))).count();
        for role in [Role::Lord, Role::Guard, Role::Sage, Role::Priest, Role::Townsfolk] {
            let w = format!("{} quest", role.word());
            if self.errands.contains(&w) || (role == Role::Townsfolk && g.hero.level < 3) || (matches!(role, Role::Priest | Role::Townsfolk) && open_tales >= 3) { continue; }
            self.talked = false;
            if let Some(r) = self.talk_to(g, role, &|t| matches!(t, Topic::Report(_) | Topic::Quest), &|t| matches!(t, Topic::Accept)) {
                if self.talked { self.errands.insert(w); }
                return r;
            }
        }
        // A cult's cellar under this town, or the place it set out for on this same tile: its way in.
        let cellar: Option<u32> = g.quests.iter().find_map(|q| match &q.goal { super::quest::Goal::Tale(t) if q.state == State::Open && t.kind == super::tales::TaleKind::Cult && t.stage == 0 && g.site(t.site).map_or(false, |s| s.tile == g.tile) => Some(t.site), _ => None })
            .or_else(|| self.target.filter(|t| !self.done.contains(t) && !self.homeward && !self.need_town(g) && g.hero.hp * 5 > g.hero.max_hp() * 4 && *t != id && self.leaving != Some(*t) && g.site(*t).map_or(false, |s| s.tile == g.tile && s.kind != SiteKind::Town)));
        if let Some(site) = cellar {
            let f = g.floor().unwrap();
            let door = |x: i32, y: i32| matches!(f.at(x, y).feature, Feature::Entrance { site: s, .. } if s == site);
            if door(g.x, g.y) { return g.act(Action::Climb); }
            self.visiting = None;
            if self.walk_in.0 != site { self.walk_in = (site, 0); }
            self.walk_in.1 += 1;
            if self.walk_in.1 <= 200 { if let Some(step) = path_to(g, &door, 40_000) { self.why = "to the cellar"; return g.act(Action::Move(step.0, step.1)); } }
            self.done.insert(site);
        }
        // Equip what was bought.
        self.equip_best(g);
        // Then out: the sewers while small, the world after.
        let sewer_done = self.done.contains(&id) || g.hero.level >= 6;
        if sewer_done { self.why = "road"; self.visiting = None; return g.act(Action::WorldMap); }
        let f = g.floor().unwrap();
        let grate = |x: i32, y: i32| matches!(f.at(x, y).feature, Feature::Entrance { site, .. } if site == id);
        if grate(g.x, g.y) { return g.act(Action::Climb); }
        // (The errand list is cleared when it leaves town: `below` and `on_road`.)
        if let Some(step) = path_to(g, &grate, 40_000) { return g.act(Action::Move(step.0, step.1)); }
        false
    }

    fn shop_smith(&mut self, g: &mut Game) {
        let Some(k) = g.place().and_then(|p| p.npcs.iter().position(|n| n.role == Role::Smith && Some(n.home) == self.visiting.or(Some(g.site_here())))) else { return };
        super::npc::greet(g, k);
        if let Some(i) = g.talk.as_ref().and_then(|t| t.options.iter().position(|(_, tp)| matches!(tp, Topic::Trade))) { super::npc::answer(g, i); }
        // For each slot, the best affordable upgrade (keep a third of the gold for potions).
        for _ in 0..4 {
            let Some(t) = g.talk.clone() else { break };
            let budget = g.hero.gold() * 2 / 3;
            let mut best: Option<(usize, i32)> = None;
            for (i, (_, tp)) in t.options.iter().enumerate() {
                if let Topic::Buy(id, _, price) = tp {
                    if *price > budget { continue; }
                    let it = super::item::Item::new(id, 1);
                    let d = it.def();
                    let Some(slot) = d.slot.as_deref().and_then(Slot::of) else { continue };
                    if d.range > 0 && !d.thrown || d.thrown || d.two_handed { continue; }
                    let cur = g.hero.equipped[slot as usize].as_ref().map_or(0, score);
                    let gain = score(&it) - cur;
                    if gain > 0 && best.map_or(true, |b| gain > b.1) { best = Some((i, gain)); }
                }
            }
            let Some((i, _)) = best else { break };
            super::npc::answer(g, i);
            self.equip_best(g);
        }
        g.talk = None;
    }

    fn equip_best(&mut self, g: &mut Game) {
        for _ in 0..8 {
            let mut best: Option<(usize, i32)> = None;
            for (k, it) in g.hero.pack.iter().enumerate() {
                let d = it.def();
                let Some(slot) = d.slot.as_deref().and_then(Slot::of) else { continue };
                if (d.range > 0 && !d.thrown) || d.thrown || d.kind == "wand" || d.two_handed { continue; }
                if !d.calling.is_empty() { continue; }
                // Casters keep their wand, paladins their bow.
                let has_ammo = g.hero.count("arrow") + g.hero.count("bolt") > 0;
                if slot == Slot::Hand && (matches!(g.hero.calling.as_deref(), Some("sorcerer") | Some("druid")) || (g.hero.calling.as_deref() == Some("paladin") && has_ammo)) { continue; }
                let cur = g.hero.equipped[slot as usize].as_ref().map_or(0, score);
                let gain = score(it) - cur;
                if gain > 0 && best.map_or(true, |b| gain > b.1) { best = Some((k, gain)); }
            }
            match best { Some((k, _)) => { let _ = g.hero.equip(k); } None => break }
        }
    }

    fn below(&mut self, g: &mut Game, id: u32) -> bool {
        if id != LAND { self.errands.clear(); }
        if g.hero.calling.as_deref() == Some("paladin") {
            let arrows = g.hero.count("arrow") > 0;
            let bow = g.hero.weapon().map_or(false, |w| w.id == "bow");
            if bow && !arrows { let _ = g.hero.unequip(Slot::Hand); self.equip_best(g); }
            if !bow && arrows { if let Some(k) = g.hero.pack.iter().position(|i| i.id == "bow") { let _ = g.hero.equip(k); } }
        }
        if self.was_in != Some(id) { self.walked.retain(|w| w.0 != id); self.leaving = None; self.hunt = None; }
        if self.leaving == Some(id) { self.why = "leaving"; return self.go_up(g, id); }
        let h = &g.hero;
        let (hp, max) = (h.hp, h.max_hp());
        let in_sight = g.monsters_in_sight();
        // Hurt: drink, heal, rest, or flee upward.
        if hp * 3 < max {
            for p in ["rune_heal", "strong_health_potion", "health_potion"] { if let Some(k) = g.hero.pack.iter().position(|i| i.id == p) { self.why = "drink"; return g.act(Action::UseItem(k)); } }
            if let Some(k) = g.hero.spells.iter().position(|s| s == "wounds" || s == "heal" || s == "intense_heal") { if g.hero.mana >= 20 { return g.act(Action::Cast(k, None)); } }
        }
        if hp * 2 < max && in_sight == 0 && g.hero.fed > 0 { self.why = "rest"; return g.act(Action::Rest); }
        // Flee up only when the way up is a few steps off (running down a long gallery with a
        // goblin at one's back is how heroes die).
        if hp * 3 < max && in_sight > 0 && id != LAND {
            let f = g.floor().unwrap();
            let rope = g.hero.count("rope") > 0;
            let up = |x: i32, y: i32| match f.at(x, y).feature { Feature::StairsUp | Feature::LadderUp | Feature::Exit => true, Feature::RopeSpot => rope, _ => false };
            if up(g.x, g.y) || path_to(g, &up, 250).is_some() { self.why = "flee"; return self.go_up(g, id); }
        }
        if g.hero.fed < 400 { if let Some(k) = g.hero.pack.iter().position(|i| i.def().kind == "food") { return g.act(Action::UseItem(k)); } }
        let outdoor = g.floor().map_or(true, |f| f.outdoor);
        if !outdoor && g.hero.torch == 0 && g.hero.glow == 0 { if let Some(k) = g.hero.pack.iter().position(|i| i.id == "torch") { return g.act(Action::UseItem(k)); } }
        // Fight: the nearest visible monster.
        // Whatever is at its side first (the weakest); then what it hunts or sees, if it can walk
        // to it within 25 steps (a beast seen through a palisade is not a quarry).
        let walk = { let f = g.floor().unwrap(); f.distances(g.x, g.y, 26, |x, y| f.at(x, y).walkable() || matches!(f.at(x, y).feature, Feature::Door { lock: 0, .. })) };
        let fw = g.floor().unwrap().w;
        let steps = |x: i32, y: i32| { let mut best = i32::MAX; for (dx, dy) in DIRS8 { let (nx, ny) = (x + dx, y + dy); if nx >= 0 && ny >= 0 && (nx as usize) < fw && (ny as usize) * fw + (nx as usize) < walk.len() { best = best.min(walk[ny as usize * fw + nx as usize]); } } best };
        let beside = g.place().and_then(|p| p.monsters.iter().filter(|m| m.z == g.z && m.hp > 0 && (m.x - g.x).abs() <= 1 && (m.y - g.y).abs() <= 1).min_by_key(|m| m.hp).map(|m| (m.uid, m.x, m.y)));
        let hunted = beside.or(self.hunt.and_then(|u| g.place().and_then(|p| p.monsters.iter().find(|m| m.uid == u && m.z == g.z && m.hp > 0 && steps(m.x, m.y) <= 25)).map(|m| (m.uid, m.x, m.y))));
        let target = hunted.or_else(|| g.place().and_then(|p| p.monsters.iter().filter(|m| m.z == g.z && m.hp > 0 && g.visible(m.x, m.y) && (m.awake || (m.x - g.x).abs().max((m.y - g.y).abs()) <= 6) && steps(m.x, m.y) <= 25)
            .min_by_key(|m| steps(m.x, m.y)).map(|m| (m.uid, m.x, m.y))));
        // A quarry it cannot catch (faster and fleeing) is let go for a while.
        let target = target.filter(|t| self.ignore.get(&t.0).map_or(true, |&until| g.turn > until));
        if let Some((uid, _, _)) = target {
            let hp = g.place().and_then(|p| p.monsters.iter().find(|m| m.uid == uid)).map_or(0, |m| m.hp);
            if self.hunt == Some(uid) && hp >= self.hunt_turns.1 { self.hunt_turns.0 += 1; } else { self.hunt_turns = (0, hp); }
            self.hunt_turns.1 = hp;
            if self.hunt_turns.0 > 25 { self.ignore.insert(uid, g.turn + 30_000); self.hunt = None; self.hunt_turns = (0, 0); }
        }
        let target = target.filter(|t| self.ignore.get(&t.0).map_or(true, |&until| g.turn > until));
        self.hunt = target.map(|t| t.0);
        if std::env::var("PLANET_ADV_TRACE").is_ok() { if let Some((uid, mx, my)) = target { eprintln!("   target {} at {},{} from {},{}", uid, mx, my, g.x, g.y); } }
        if let Some((uid, mx, my)) = target {
            // A bow, a thrown spear or a wand: shoot from where it stands.
            let ranged = g.hero.weapon().map_or(0, |w| w.def().range);
            if ranged > 1 && (mx - g.x).abs().max((my - g.y).abs()) <= ranged && g.visible(mx, my) && g.floor().map_or(false, |f| f.clear_line((g.x, g.y), (mx, my))) {
                let wand = g.hero.weapon().map_or(false, |w| w.def().kind == "wand");
                let ammo_ok = g.hero.weapon().map_or(false, |w| w.def().ammo.as_ref().map_or(true, |a| g.hero.count(a) > 0));
                if (!wand || g.hero.mana >= 3) && ammo_ok {
                    // Strike spells first for the casters.
                    if let Some(k) = g.hero.spells.iter().position(|s| matches!(s.as_str(), "flame" | "ice" | "holy" | "ethereal")) { if g.hero.mana >= 40 { self.why = "cast"; return g.act(Action::Cast(k, Some(uid))); } }
                    self.why = "shoot"; return g.act(Action::Attack(uid));
                }
            }
            // Crowded: a rune of fire or stones.
            let crowd = g.place().map_or(0, |p| p.monsters.iter().filter(|m| m.z == g.z && m.hp > 0 && (m.x - mx).abs() <= 1 && (m.y - my).abs() <= 1).count());
            if crowd >= 3 { for r in ["rune_fire", "rune_stones"] { if let Some(k) = g.hero.pack.iter().position(|i| i.id == r) { self.why = "rune"; return g.act(Action::UseItem(k)); } } }
            if (mx - g.x).abs() <= 1 && (my - g.y).abs() <= 1 {
                // A strong blow when there is mana to spare.
                if let Some(k) = g.hero.spells.iter().position(|s| s == "brutal") { if g.hero.mana >= 60 { return g.act(Action::Cast(k, Some(uid))); } }
                self.why = "attack"; return g.act(Action::Attack(uid));
            }
            let avoid = self.prev_cell.filter(|c| c.2 == g.z).map(|c| (c.0, c.1));
            let step = path_avoiding(g, &|x, y| (x - mx).abs() <= 1 && (y - my).abs() <= 1, 3000, avoid, false).or_else(|| path_avoiding(g, &|x, y| (x - mx).abs() <= 1 && (y - my).abs() <= 1, 3000, None, false));
            if let Some(step) = step { self.why = "approach"; if g.act(Action::Move(step.0, step.1)) { return true; } }
        }
        // Loot.
        let has_items = g.floor().map_or(false, |f| f.items.get(&(g.x, g.y)).map_or(false, |v| !v.is_empty()));
        if has_items { let r = g.act(Action::PickUp); self.equip_best(g); return r; }
        // A quest chest beside: the most valuable.
        if let Some(f) = g.floor() {
            for (dx, dy) in DIRS8 { if let Feature::QuestChest { choices, taken: false, .. } = &f.at(g.x + dx, g.y + dy).feature {
                if !g.chosen.contains(&id) { let k = choices.iter().enumerate().max_by_key(|(_, c)| c.value()).map(|(k, _)| k).unwrap_or(0); let r = g.act(Action::Choose(k)); self.equip_best(g); return r; }
            } }
        }
        // Home when it should.
        let tier = g.place().map_or(1, |p| p.spec.tier);
        let deep_enough = g.hero.level + 2 < (tier.saturating_sub(1) * 6 + g.z as u32 * 3).max(1);
        if id == LAND { return self.land_way(g); }
        if self.need_town(g) || deep_enough || self.stuck > 40 { self.why = if self.need_town(g) { "town" } else if deep_enough { "too deep" } else { "stuck" }; return self.go_up(g, id); }
        // Explore: things to open, items lying about, the edge of what is seen.
        let f = g.floor().unwrap();
        let keys: Vec<u32> = g.hero.pack.iter().filter(|i| i.id == "key").map(|i| i.tag).collect();
        let interesting = |x: i32, y: i32| -> bool {
            let t = f.at(x, y);
            match &t.feature {
                Feature::Chest { opened: false, .. } | Feature::Sarcophagus { opened: false, .. } => return true,
                Feature::Plinth { item: Some(_) } => return true,
                Feature::QuestChest { taken: false, .. } => return !g.chosen.contains(&id),
                // A puzzle's levers only in their order (it has read the engraving).
                Feature::Lever { pulled: false, id: lid } => return g.place().map_or(true, |p| p.levers.iter().filter(|l| l.z == g.z && l.order.contains(lid)).all(|l| l.order.get(l.pulled.len()) == Some(lid))),
                Feature::RiddleDoor { open: false, .. } => return true,
                Feature::Door { open: false, lock } if *lock > 0 && keys.contains(lock) => return true,
                _ => {}
            }
            let transit = matches!(t.feature, Feature::StairsDown | Feature::StairsUp | Feature::LadderDown | Feature::LadderUp | Feature::Hole | Feature::RopeSpot | Feature::Exit | Feature::Grate);
            if f.items.get(&(x, y)).map_or(false, |v| !v.is_empty()) && t.walkable() && !transit { return true; }
            // A frontier: seen and walkable (or a door to open), beside the unseen.
            let k = y as usize * f.w + x as usize;
            let door = matches!(t.feature, Feature::Door { open: false, lock: 0 });
            f.seen[k] && (t.walkable() || door) && DIRS8.iter().any(|(dx, dy)| { let (nx, ny) = (x + dx, y + dy); f.inside(nx, ny) && !f.seen[ny as usize * f.w + nx as usize] })
        };
        if !self.walked.contains(&(id, g.z)) {
            if let Some(step) = path_to(g, &interesting, 20_000) { self.why = "explore"; return g.act(Action::Move(step.0, step.1)); }
            // Walked: tap the walls where a hidden door is (it has a nose for drafts), a few times.
            let secret: Option<(i32, i32)> = f.cells(|t| t.feature == Feature::SecretDoor).into_iter().next();
            let tries = self.searches.entry((id, g.z)).or_insert(0);
            if let (Some((sx, sy)), true) = (secret, *tries < 12) {
                if (sx - g.x).abs().max((sy - g.y).abs()) <= 2 { *tries += 1; self.why = "search"; return g.act(Action::Search); }
                let walk = self.search_walk.entry((id, g.z)).or_insert(0);
                *walk += 1;
                if *walk < 300 { if let Some(step) = path_to(g, &|x, y| (x - sx).abs().max((y - sy).abs()) <= 1, 20_000) { self.why = "to the wall"; return g.act(Action::Move(step.0, step.1)); } }
            }
            self.walked.insert((id, g.z));
        }
        // All walked: down if there is a way, else this place is done.
        let n = g.place().map_or(1, |p| p.floors.len());
        if g.z + 1 < n {
            let down = |x: i32, y: i32| matches!(f.at(x, y).feature, Feature::StairsDown | Feature::LadderDown | Feature::Hole | Feature::Grate);
            if down(g.x, g.y) { return g.act(Action::Climb); }
            if let Some(step) = path_to(g, &down, 20_000) { return g.act(Action::Move(step.0, step.1)); }
        }
        self.done.insert(id);
        self.leaving = Some(id);
        self.go_up(g, id)
    }

    fn go_up(&mut self, g: &mut Game, _id: u32) -> bool {
        if g.on_land() { return self.land_way(g); }
        let f = g.floor().unwrap();
        let has_rope = g.hero.count("rope") > 0;
        if matches!(f.at(g.x, g.y).feature, Feature::StairsUp | Feature::LadderUp | Feature::Exit) || (has_rope && f.at(g.x, g.y).feature == Feature::RopeSpot) { return g.act(Action::Climb); }
        let up = |x: i32, y: i32| match f.at(x, y).feature { Feature::StairsUp | Feature::LadderUp | Feature::Exit => true, Feature::RopeSpot => has_rope, _ => false };
        if let Some(step) = path_to(g, &up, 20_000) { return g.act(Action::Move(step.0, step.1)); }
        // Trapped below a hole without a rope: keep exploring for another way; else wait.
        g.act(Action::Wait)
    }
}

fn self_walked_count(b: &Bot) -> usize { b.walked.len() }

/// Play `turns` acts with the bot; returns the bot (for its record).
pub fn run(g: &mut Game, acts: usize) -> Bot {
    let mut b = Bot::default();
    for _ in 0..acts { if !b.step(g) { let _ = g.act(Action::Wait); } }
    let _ = Skill::Magic;
    b
}

/// `--adventure-bot N`: a new adventure on this world played by the bot for N acts; prints the
/// hero's record every N/10 acts and the last of the log.
pub fn report(world: &crate::world::WorldData, history: Option<&crate::history::world_state::WorldHistory>, acts: usize, seed: u64) {
    let t0 = std::time::Instant::now();
    // (PLANET_ADV_LOAD=FILE plays on from a saved adventure of this world.)
    let mut g = match std::env::var("PLANET_ADV_LOAD") {
        Ok(path) => match super::Game::load(std::path::Path::new(&path), super::world::build(world, history, seed, None).info) { Ok(g) => g, Err(e) => { println!("Could not load {}: {}", path, e); return; } },
        Err(_) => super::new_game(world, history, seed, None),
    };
    let kinds = |k: SiteKind| g.sites.iter().filter(|s| s.kind == k).count();
    println!("Adventure on seed {}: {} places ({} towns, {} ruins, {} castles, {} lairs, {} tombs, {} temples, {} shrines, {} caves, {} mines, {} labyrinths, {} camps, {} halls, {} dark fortress); built in {:.0} ms",
        seed, g.sites.len(), kinds(SiteKind::Town), kinds(SiteKind::Ruin), kinds(SiteKind::Castle), kinds(SiteKind::Lair), kinds(SiteKind::Tomb), kinds(SiteKind::Temple), kinds(SiteKind::Shrine),
        kinds(SiteKind::Cave), kinds(SiteKind::Mine), kinds(SiteKind::Labyrinth), kinds(SiteKind::Camp), kinds(SiteKind::Halls), kinds(SiteKind::DarkFortress), t0.elapsed().as_secs_f64() * 1000.0);
    let home = g.site(g.hero.temple).map(|s| s.name.clone()).unwrap_or_default();
    println!("{} of the {} sets out from {}.", g.hero.name, g.hero.race, home);
    if let Ok(which) = std::env::var("PLANET_ADV_SITES") { for s in g.sites.iter().filter(|s| which.is_empty() || s.name.contains(&which)) { println!("  site {} {:?} '{}' tile {:?} tier {}", s.id, s.kind, s.name, s.tile, s.tier); } }
    if let Some(h) = history { if std::env::var("PLANET_ADV_DEBUG").is_ok() {
        use crate::history::events::types::EventType as E;
        let battles = h.chronicle.events.iter().filter(|e| e.event_type == E::BattleFought && e.location.is_some()).count();
        let hero_died = h.chronicle.events.iter().filter(|e| e.event_type == E::HeroDied && e.location.is_some()).count();
        let razed: Vec<String> = h.settlements.values().filter(|s| s.destroyed.is_some()).map(|s| format!("{:?}", s.settlement_type)).collect();
        let tombs = h.monuments.values().filter(|m| format!("{:?}", m.monument_type) == "Tomb").count();
        let temples_m = h.monuments.values().filter(|m| format!("{:?}", m.monument_type) == "Temple").count();
        let holy: usize = h.religions.values().map(|r| r.holy_sites.len()).sum();
        let sacred: usize = h.deities.values().map(|d| d.sacred_places.len()).sum();
        let cults = h.cults.values().filter(|c| c.headquarters.is_some()).count();
        let fig_battle = h.figures.values().filter(|f| f.cause_of_death == Some(crate::history::entities::traits::DeathCause::Battle)).count();
        println!("debug: battles {} hero-died {} figures-died-in-battle {} razed {:?} tomb monuments {} temple monuments {} holy sites {} sacred places {} cults with hq {} (of {}), shadow {:?}", battles, hero_died, fig_battle, razed, tombs, temples_m, holy, sacred, cults, h.cults.len(), h.shadow.as_ref().map(|s| (s.seat, s.broken.is_some())));
    } }
    let mut b = Bot::default();
    // The history goes on beside the adventure.
    let mut living = history.map(super::living::Living::new);
    if let Some(l) = living.as_mut() { l.sync(&mut g, world); }
    // PLANET_ADV_ASK=1: the home town's sage and its drunk asked about the same beast.
    if let (Ok(_), Some(h)) = (std::env::var("PLANET_ADV_ASK"), g.history.clone()) {
        let home = g.site(g.hero.temple).map(|s| s.tile).unwrap_or(g.tile);
        let beast = h.legendary_creatures.values().filter(|c| c.death_date.is_none() && c.lair_location.is_some()).min_by_key(|c| (super::world::dist(c.lair_location.unwrap(), home, g.world.w), c.id.0)).map(|c| (c.name.clone(), c.lair_location.unwrap()));
        if let Some((name, lair)) = beast {
            for who in ["sage", "drunk"] {
                let k = g.place().and_then(|p| p.npcs.iter().position(|n| if who == "sage" { n.role == super::actor::Role::Sage && n.home == g.hero.temple } else { n.of == "drunk" && n.home == g.hero.temple }));
                let Some(k) = k else { println!("Ask {}: nobody", who); continue };
                let n = g.place().unwrap().npcs[k].clone();
                let town = g.site(n.home).and_then(|s| s.settlement).map(crate::history::SettlementId);
                let before = g.mapped.get(lair.1 * g.world.w + lair.0).copied().unwrap_or(0);
                let said = super::lore::ask(&mut g, &n, town, &name);
                let after = g.mapped.get(lair.1 * g.world.w + lair.0).copied().unwrap_or(0);
                println!("Ask {} about {}: {} [marked {}]", who, name, said, before == 0 && after > 0 || before > 0 && said.contains("on your map"));
            }
        }
    }
    let t1 = std::time::Instant::now();
    let step = (acts / 10).max(1);
    let mut dumped = false;
    let mut last_sig = (u32::MAX, 0, 0, 0);
    let trace: Option<(usize, usize)> = std::env::var("PLANET_ADV_TRACE").ok().and_then(|v| { let (a, b) = v.split_once(',')?; Some((a.parse().ok()?, b.parse().ok()?)) });
    for k in 0..acts {
        let before = (g.x, g.y, g.log.len());
        if !b.step(&mut g) { let _ = g.act(Action::Wait); }
        if let Some(l) = living.as_mut() { if l.sync(&mut g, world) && std::env::var("PLANET_ADV_WORLD").is_ok() { for line in g.log[before.2.min(g.log.len())..].iter().filter(|l| l.tone == super::game::Tone::Danger || l.text.starts_with("Word") || l.text.contains(" rules ")) { println!("  world at day {}: {}", g.turn / super::land::DAY + 1, line.text); } } }
        if std::env::var("PLANET_ADV_DEATHS").is_ok() { for l in g.log[before.2.min(g.log.len())..].iter().filter(|l| l.tone == super::game::Tone::Death) { println!("  death at act {} (level {}, {} on {:?}): {}", k, g.hero.level, g.place().map(|p| p.spec.name.clone()).unwrap_or_default(), g.tile, l.text); } }
        if let Some((a, z)) = trace { if k >= a && k < z {
            let near: Vec<String> = g.place().map(|p| p.monsters.iter().filter(|m| m.z == g.z && ((m.x - g.x).abs().max((m.y - g.y).abs()) <= 3 || (m.awake && (m.x - g.x).abs().max((m.y - g.y).abs()) <= 14))).map(|m| format!("{}@{},{} hp{} awake{} vis{} fear{}", m.def, m.x, m.y, m.hp, m.awake, g.visible(m.x, m.y), m.fear)).collect()).unwrap_or_default();
            let items: Vec<String> = g.floor().map(|f| f.items.iter().filter(|((x, y), v)| !v.is_empty() && (x - g.x).abs().max((y - g.y).abs()) <= 4).map(|((x, y), v)| format!("{},{}:{}:{:?}", x, y, v[0].id, f.at(*x, *y).feature.word())).collect()).unwrap_or_default();
            println!("act {} [{}]: {:?} -> {},{} | {:?} | items {:?} | {}", k, b.why, (before.0, before.1), g.x, g.y, near, items, g.log[before.2.min(g.log.len())..].iter().map(|l| l.text.clone()).collect::<Vec<_>>().join(" / "));
        } }
        if (k + 1) % 1000 == 0 {
            let sig = (g.stats.kills, g.stats.chests, g.stats.sites_entered, g.z);
            if sig == last_sig && !dumped && std::env::var("PLANET_ADV_DUMP").is_ok() { dumped = true; println!("act {}: {}", k, dump(&g)); for l in g.log.iter().rev().take(8) { println!("   log: {}", l.text); } }
            last_sig = sig;
        }
        if (k + 1) % step == 0 {
            let s = &g.stats;
            let at = g.place().map(|p| format!("{} ({})", p.spec.name, p.floors[g.z].name)).unwrap_or_else(|| format!("the road at {},{}", g.tile.0, g.tile.1));
            println!("  act {:>6}: level {:>2} ({}), {}/{} hp, {} gold, kills {}, bosses {}, chests {}, deaths {}, quests {}, places {}; at {}",
                k + 1, g.hero.level, g.hero.calling.as_deref().unwrap_or("no calling"), g.hero.hp, g.hero.max_hp(), g.hero.gold(), s.kills, s.bosses, s.chests, s.deaths, s.quests_done, s.sites_entered, at);
        }
    }
    // A save and a load give the same adventure back, and it goes on the same way.
    if let Ok(path) = std::env::var("PLANET_ADV_SAVE") {
        let p = std::path::Path::new(&path);
        g.save(p).expect("save");
        let mut back = super::Game::load(p, g.world.clone()).expect("load");
        let same = back.hero.level == g.hero.level && back.hero.xp == g.hero.xp && back.places.len() == g.places.len() && back.log.len() == g.log.len() && back.hero.pack == g.hero.pack;
        // The history replays to the same: the seasons and the deeds in the same places.
        if let (Some(h), Some(l)) = (history, living.as_ref()) {
            let mut l2 = super::living::Living::new(h);
            l2.sync(&mut back, world);
            let (a, b2) = (l.history.chronicle.events.len(), l2.history.chronicle.events.len());
            println!("History: {} seasons, {} events live, {} replayed: {}", back.seasons, a, b2, if a == b2 && l.history.current_date == l2.history.current_date { "the same" } else { "DIFFERENT" });
        }
        let mut b2 = b.clone();
        let mut g2 = g.clone();
        g2.rng = back.rng.clone();
        for _ in 0..500 { if !b2.step(&mut back) { let _ = back.act(Action::Wait); } }
        for _ in 0..500 { if !b.step(&mut g2) { let _ = g2.act(Action::Wait); } }
        println!("Save: {} bytes; round trip {}; 500 acts after: {}", std::fs::metadata(p).map(|m| m.len()).unwrap_or(0), if same { "ok" } else { "DIFFERS" }, if back.hero.xp == g2.hero.xp && back.turn == g2.turn { "the same" } else { "different" });
    }
    let skills: Vec<String> = super::hero::Skill::ALL.iter().map(|s| format!("{} {}", s.word(), g.hero.skill(*s))).collect();
    println!("Skills: {}", skills.join(", "));
    let gear: Vec<String> = g.hero.equipped.iter().flatten().map(|i| i.describe()).collect();
    println!("Wears: {}", gear.join(", "));
    println!("Quests: {}", g.quests.iter().map(|q| format!("{} [{}]", q.title, q.progress())).collect::<Vec<_>>().join("; "));
    if let (Ok(dir), Some(l)) = (std::env::var("PLANET_ADV_LEGENDS"), living.as_ref()) {
        let gaz = crate::lore::build_gazetteer(world, Some(&l.history), seed);
        match crate::lore::legends::write_legends(world, &l.history, &gaz, &[], None, std::path::Path::new(&dir)) { Ok(r) => println!("Legends written to {}: {}", dir, r.line()), Err(e) => println!("Legends failed: {}", e) }
    }
    {
        let mut kinds: Vec<String> = g.quests.iter().filter(|q| matches!(q.state, State::Rewarded | State::Failed)).filter_map(|q| match &q.goal { super::quest::Goal::Tale(t) => Some(format!("{:?}:{}", t.kind, t.chose)), _ => None }).collect();
        kinds.sort();
        let open: Vec<String> = g.quests.iter().filter(|q| matches!(q.state, State::Open | State::Done)).filter_map(|q| match &q.goal { super::quest::Goal::Tale(t) => Some(format!("{:?}@{}", t.kind, t.stage)), _ => None }).collect();
        println!("Tales: {} done [{}]; open [{}]", g.stats.tales_done, kinds.join(", "), open.join(", "));
        let rooms: usize = g.places.values().map(|p| p.rooms.iter().filter(|r| r.seen).count()).sum();
        let riddles = g.places.values().flat_map(|p| p.floors.iter()).flat_map(|f| f.tiles.iter()).filter(|t| matches!(t.feature, Feature::RiddleDoor { open: true, .. })).count();
        let gates = g.places.values().filter(|p| p.levers.iter().any(|l| l.pulled.len() == l.order.len() && !l.order.is_empty())).count();
        let hidden = g.places.values().flat_map(|p| p.floors.iter()).flat_map(|f| f.tiles.iter()).filter(|t| t.feature == Feature::SecretDoor).count();
        let walked = self_walked_count(&b);
        println!("Rooms: {} authored rooms seen, {} hidden doors found ({} still hidden), {} riddles answered, {} lever puzzles solved; {} floors walked out", rooms, g.stats.secrets, hidden, riddles, gates, walked);
    }
    let songs: usize = g.songs.values().map(|v| v.len()).sum();
    println!("Songs: {} towns sing of {} ({} songs); deeds in the chronicle: {}", g.songs.len(), g.hero.name, songs, g.hero_events.len());
    if let Ok(path) = std::env::var("PLANET_ADV_LEGEND") { let _ = std::fs::write(&path, g.legend_html()); println!("Legend written to {} ({} deeds)", path, g.deeds.len()); }
    if let Some(c) = &g.companion { println!("Companion: {} ({} of {} life, {} slain)", c.name, c.hp, c.max_hp, c.kills); }
    println!("{} acts in {:.1} s ({:.0} µs an act)", acts, t1.elapsed().as_secs_f64(), t1.elapsed().as_secs_f64() * 1e6 / acts.max(1) as f64);
    println!("Last of the log:");
    for l in g.log.iter().rev().take(25).collect::<Vec<_>>().into_iter().rev() { println!("  {}", l.text); }
}

/// The floor as text around the hero (for finding where the bot sticks): # wall, . floor, @ hero,
/// m monster, > < ways down and up, + door, = locked, C chest, ? the unseen.
pub fn dump(g: &Game) -> String {
    let Some(f) = g.floor() else { return "on the road".into() };
    let mut out = format!("{} z{} at {},{}: hp {}/{}, fed {}, rope {}\n", f.name, g.z, g.x, g.y, g.hero.hp, g.hero.max_hp(), g.hero.fed, g.hero.count("rope"));
    // (Around the adventurer on a big floor.)
    let (x0, x1, y0, y1) = if f.w > 100 { ((g.x - 40).max(0), (g.x + 40).min(f.w as i32 - 1), (g.y - 25).max(0), (g.y + 25).min(f.h as i32 - 1)) } else { (0, f.w as i32 - 1, 0, f.h as i32 - 1) };
    for y in y0..=y1 {
        for x in x0..=x1 {
            let t = f.at(x, y);
            let c = if (x, y) == (g.x, g.y) { '@' }
                else if g.place().map_or(false, |p| p.monsters.iter().any(|m| m.z == g.z && m.x == x && m.y == y && m.hp > 0)) { 'm' }
                else if g.place().map_or(false, |p| p.npcs.iter().any(|n| n.z == g.z && n.x == x && n.y == y)) { 'p' }
                else { match &t.feature {
                    Feature::Entrance { .. } => 'E', Feature::Sign { .. } => 'S',
                    Feature::StairsDown | Feature::LadderDown | Feature::Hole | Feature::Grate => '>', Feature::StairsUp | Feature::LadderUp | Feature::RopeSpot => '<', Feature::Exit => 'E',
                    Feature::Door { lock, open } => if *lock > 0 { '=' } else if *open { '\'' } else { '+' }, Feature::Chest { .. } | Feature::QuestChest { .. } => 'C', Feature::Gate { .. } => '#', Feature::Lever { .. } => 'L',
                    _ => if t.wall != super::map::Wall::None { '#' } else if !f.seen[y as usize * f.w + x as usize] { '?' } else { '.' } } };
            out.push(c);
        }
        out.push('\n');
    }
    out
}
