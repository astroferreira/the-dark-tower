//! Creatures on the map: the raid arrives on legs, and wolves come out of their dens at night.
//!
//! The arc's attackers (a named beast, drawn at its size, or a war band of three) enter at the
//! map's edge on the side their lair, seat or town lies, an hour before midnight on the eve of
//! the raid, and creep to the camp (15 minutes a cell: about 20 real seconds at 1x across 90
//! cells). The clash is played where they meet the first settler: the one nearest is struck,
//! the one nearest them comes to their aid (`Colony::raid_at`). A closed palisade sends them
//! round to a gate, since they path like anyone else. Where the embark has a predator's den,
//! two wolves come out at dusk and hunt a settler alone and far from the fire; they fear it
//! and go home at dawn.
//!
//! Creatures walk in three dimensions where they must (`Creature::z`, `path3`): what lives in a
//! breached cavern roams its floor, its hunters climb the stair at night (`cavelife.rs`), and what
//! comes from the deep starts in its cavern and walks up the stair to the camp. Those that keep
//! to the surface walk the old column walk (`nav::path`).

use super::{nav, Colony, Pos, TICKS_PER_DAY};
use super::nav::P3;
use crate::local::wildlife::Feature;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreatureKind { Beast, Raider, Wolf, Game, Trader, Pet, Besieger,
    /// A hunter of a breached cavern (`cavelife.rs`): it roams the cavern floor, takes those who
    /// work alone in the dark, and climbs the stair on some nights.
    CaveHunter,
    /// Harmless cavern life (bats, crickets, crabs...) roaming its floor.
    CaveLife }

#[derive(Clone, Debug)]
pub struct Creature {
    pub kind: CreatureKind,
    pub name: String,
    pub pos: Pos,
    pub path: Vec<Pos>,
    pub stride: i32,
    /// Done here: walking back the way it came, gone at the path's end.
    pub leaving: bool,
    pub size: f32,
    /// Where it came from (the edge, or a den).
    pub home: Pos,
    pub spawned: u64,
    /// Unique among the colony's creatures (a hunter follows one).
    pub id: u32,
    /// The level it stands on when it walks in three dimensions (a cavern's floor, the stair);
    /// None for those that keep to the surface (they stand on their column's ground).
    pub z: Option<i32>,
    /// A walk in three dimensions, followed before `path`.
    pub path3: Vec<P3>,
    /// The level of its home (a cavern's floor) for those that live below.
    pub home_z: i32,
    /// Out of its cavern: a cave hunter up the stair for the night, a beast come up.
    pub out: bool,
    /// Resting until this tick (a cave hunter after a kill).
    pub rest_until: u64,
}

/// Time spent on cavern life's roaming and hunting (`PLANET_DEBUG_CAVE` prints it).
pub static CAVE_NS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Cells from the camp at which the attackers enter (they walk a settler's pace from dusk and
/// reach the camp in the small hours).
const ENTRY: f32 = 84.0;
/// A step costs this many times a settler's (they creep at night).
const CREEP: i32 = 1;

fn compass(dx: f32, dy: f32) -> &'static str {
    let a = dy.atan2(dx).to_degrees();
    match ((a + 360.0 + 22.5) % 360.0 / 45.0) as usize {
        0 => "the east", 1 => "the south-east", 2 => "the south", 3 => "the south-west",
        4 => "the west", 5 => "the north-west", 6 => "the north", _ => "the north-east",
    }
}

impl Colony {
    /// Where creature `c` stands, with its level (its column's ground for a surface walker).
    pub fn creature_here3(&self, c: &Creature) -> P3 {
        let sz = self.map.surface_z[c.pos.1 as usize * self.map.width + c.pos.0 as usize];
        (c.pos.0, c.pos.1, c.z.unwrap_or(sz))
    }

    /// Whether creature `c` is below the ground (in a cavern, on the stair): the surface view
    /// does not draw it, and only those below meet it.
    pub fn creature_below(&self, c: &Creature) -> bool {
        c.z.map_or(false, |z| z < self.map.surface_z[c.pos.1 as usize * self.map.width + c.pos.0 as usize])
    }

    /// The hatch in the stair (`delve.rs::seal_caverns`) as the place no creature's walk may
    /// stand: nothing passes it up or down.
    pub(crate) fn hatch_bar(&self) -> Option<P3> { self.hatch.map(|(p, z)| (p.0, p.1, z)) }

    /// A walk in three dimensions for a creature (the hatch barred unless `burst`), without the
    /// first cell.
    pub(crate) fn creature_path3(&self, from: P3, to: P3, budget: usize, burst: bool) -> Option<Vec<P3>> {
        let bar = if burst { None } else { self.hatch_bar() };
        nav::path3_barred(&self.map, None, from, to, budget, bar).map(|p| p.into_iter().skip(1).collect())
    }

    /// The direction (unit vector on the map) the arc's threat comes from, and its compass word.
    pub(crate) fn threat_side_pub(&self) -> ((f32, f32), &'static str) { self.threat_side() }

    fn threat_side(&self) -> ((f32, f32), &'static str) {
        let from = self.arc.as_ref().and_then(|a| a.threat.from);
        let (tx, ty) = self.map.world_tile;
        let mut d = match from {
            Some((fx, fy)) => (fx as f32 - tx as f32, fy as f32 - ty as f32),
            None => (0.0, 0.0),
        };
        if d.0.abs() + d.1.abs() < 0.01 {
            let a = (self.seed % 360) as f32 * std::f32::consts::PI / 180.0;
            d = (a.cos(), a.sin());
        }
        let n = (d.0 * d.0 + d.1 * d.1).sqrt();
        let d = (d.0 / n, d.1 / n);
        (d, compass(d.0, d.1))
    }

    pub(crate) fn passable_near_pub(&self, p: (i32, i32)) -> Option<Pos> { self.passable_near(p) }

    /// A passable cell near `p` (searching outward).
    fn passable_near(&self, p: (i32, i32)) -> Option<Pos> {
        let n = self.map.width as i32;
        for r in 0i32..20 {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs().max(dy.abs()) != r { continue; }
                    let q = ((p.0 + dx).clamp(2, n - 3) as u16, (p.1 + dy).clamp(2, self.map.height as i32 - 3) as u16);
                    if nav::passable(&self.map, q) { return Some(q); }
                }
            }
        }
        None
    }

    /// How many raiders come: three, and one more for every eight in the camp (eight at most).
    pub(crate) fn band_size(&self) -> usize { (3 + self.alive() / 8).min(8) }

    /// The attackers set out from the map's edge on the threat's side.
    pub(crate) fn send_attackers(&mut self) {
        // Besiegers at their fires become the assault (`siege.rs`).
        let besieged = self.siege.as_ref().map(|s| s.at);
        self.creatures.retain(|c| c.kind != CreatureKind::Besieger);
        let Some(arc) = self.arc.as_ref() else { return };
        if let Some(m) = arc.threat.monster.as_ref() {
            if !self.foes_seen.iter().any(|f| f.0 == arc.threat.name) { let e = (arc.threat.name.clone(), m.clone()); self.foes_seen.push(e); }
        }
        let Some(arc) = self.arc.as_ref() else { return };
        let (kind, n, name, size) = match arc.threat.kind {
            super::arc::ThreatKind::Beast | super::arc::ThreatKind::Deep => (CreatureKind::Beast, 1, arc.threat.name.clone(), arc.threat.size),
            // A band grows with the camp it comes for (DF's sieges grow with the fortress).
            _ => (CreatureKind::Raider, self.band_size(), arc.threat.name.clone(), 1.0),
        };
        // What comes from the deep starts in its cavern (or at the shaft's foot, out of the
        // hollow) and walks up the stair (`cavelife.rs`); with no way up, it comes out of the
        // mine's mouth as before.
        if arc.threat.kind == super::arc::ThreatKind::Deep {
            let short = arc.threat.monster.as_ref().map(|m| m.short.clone()).unwrap_or_default();
            let why = arc.threat.why.clone();
            if self.deep_comes_up(&name, size) { return; }
            if let Some(at) = self.mine_mouth().and_then(|m| self.passable_near((m.0 as i32, m.1 as i32))) {
                let path = nav::path(&self.map, at, self.camp, super::PATH_BUDGET).map(|p| p.into_iter().skip(1).collect()).unwrap_or_default();
                let id = self.new_creature_id();
                self.creatures.push(Creature { kind: CreatureKind::Beast, name: name.clone(), pos: at, path, stride: 0, leaving: false, size, home: at, spawned: self.clock.tick, id, z: None, path3: Vec::new(), home_z: 0, out: false, rest_until: 0 });
                self.raid_side = "the mine".into();
                let line = format!("Something climbs out of the mine: {}.", name);
                self.note(line.clone());
                self.moment("Out of the deep".into(), format!("{} {} heaves itself out of the mine's mouth.", line, super::arc::capital_word(&short)), format!("because {}", why), at);
                for j in 0..self.settlers.len() { if self.settlers[j].alive { let w = short.clone(); self.feel(j, super::mind::Feel::TheDeep { what: w }); } }
                return;
            }
        }
        let ((dx, dy), side) = self.threat_side();
        let c = (self.camp.0 as f32, self.camp.1 as f32);
        for k in 0..n {
            let off = (k as f32 - (n as f32 - 1.0) / 2.0) * 4.0;
            let p = match besieged {
                Some(b) => ((b.0 as f32 - dy * off) as i32, (b.1 as f32 + dx * off) as i32),
                None => ((c.0 + dx * ENTRY - dy * off) as i32, (c.1 + dy * ENTRY + dx * off) as i32),
            };
            let Some(at) = self.passable_near(p) else { continue };
            let path = nav::path(&self.map, at, self.camp, super::PATH_BUDGET).map(|p| p.into_iter().skip(1).collect()).unwrap_or_default();
            let id = self.new_creature_id();
            self.creatures.push(Creature { kind, name: name.clone(), pos: at, path, stride: 0, leaving: false, size, home: at, spawned: self.clock.tick, id, z: None, path3: Vec::new(), home_z: 0, out: false, rest_until: 0 });
        }
        if let Some(first) = self.creatures.iter().find(|c| matches!(c.kind, CreatureKind::Beast | CreatureKind::Raider)).map(|c| c.pos) {
            self.raid_side = side.to_string();
            let line = format!("Something moves at the edge of the clearing, out of {}.", side);
            self.note(line.clone());
            let who = self.arc.as_ref().map(|a| a.threat.name.clone()).unwrap_or_default();
            self.moment("They are coming".into(), line, format!("because {} {} on the move, as was foretold", who, if kind == CreatureKind::Beast { "is" } else { "are" }), first);
        }
    }

    pub(crate) fn attackers_out(&self) -> bool { self.creatures.iter().any(|c| matches!(c.kind, CreatureKind::Beast | CreatureKind::Raider)) }

    /// Where the attackers meet the camp: the first of them within two cells of a living
    /// settler, or at the end of their path (the fire).
    pub(crate) fn attackers_clash(&self) -> Option<Pos> {
        for c in self.creatures.iter().filter(|c| matches!(c.kind, CreatureKind::Beast | CreatureKind::Raider) && !c.leaving) {
            // On the surface it meets those on the surface; below (coming up the stair from the
            // deep), those within a level of it.
            let low = self.creature_below(c);
            let cz = self.creature_here3(c).2;
            if (0..self.settlers.len()).any(|j| { let s = &self.settlers[j]; s.alive && (s.pos.0 as i32 - c.pos.0 as i32).abs().max((s.pos.1 as i32 - c.pos.1 as i32).abs()) <= 2
                && if low || self.below(j) { (self.here3(j).2 - cz).abs() <= 1 } else { true } }) { return Some(c.pos); }
            if c.path.is_empty() && c.path3.is_empty() { return Some(c.pos); }
        }
        None
    }

    /// The attackers break off and go back the way they came.
    pub(crate) fn attackers_retreat(&mut self) {
        for k in 0..self.creatures.len() {
            if !matches!(self.creatures[k].kind, CreatureKind::Beast | CreatureKind::Raider) { continue; }
            let (from, home) = (self.creatures[k].pos, self.creatures[k].home);
            // What came up from the deep goes back down the stair.
            if self.creatures[k].z.is_some() {
                let (from3, home3) = (self.creature_here3(&self.creatures[k]), (home.0, home.1, self.creatures[k].home_z));
                let path3 = self.creature_path3(from3, home3, super::PATH_BUDGET, true).unwrap_or_default();
                let c = &mut self.creatures[k];
                c.leaving = true;
                c.path.clear();
                c.path3 = path3;
                continue;
            }
            let path = nav::path(&self.map, from, home, super::PATH_BUDGET).map(|p| p.into_iter().skip(1).collect()).unwrap_or_default();
            let c = &mut self.creatures[k];
            c.leaving = true;
            c.path = path;
        }
    }

    /// Move every creature a tick; those leaving vanish at their path's end. Wolves come out at
    /// dusk where there is a den and go home at dawn.
    pub(crate) fn step_creatures(&mut self) {
        let (hour, minute) = (self.clock.hour(), self.clock.minute());
        if hour == 21 && minute == 0 { self.moon_rises(); self.dead_rise(); self.tomb_dead_rise(); self.wolves_out(); }
        // The cavern's hunters set out up the stair after dusk, and go back down at dawn.
        if hour == 20 && minute == 0 { self.cave_hunters_out(); }
        if hour == 6 && minute == 0 {
            for k in 0..self.creatures.len() {
                if self.creatures[k].kind == CreatureKind::Wolf && !self.creatures[k].leaving { self.wolf_home(k); }
            }
            self.cave_hunters_home();
        }
        // Cavern life roams its floor; its hunters stalk whoever is alone in the dark.
        let t0 = std::time::Instant::now();
        if self.clock.tick % 60 == 30 { self.cave_life_roams(); }
        if self.clock.tick % 20 == 10 { self.cave_hunt(); }
        CAVE_NS.fetch_add(t0.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
        // Wolves hunt a settler alone and far from the fire.
        if self.clock.tick % 20 == 0 { self.wolves_hunt(); }
        // Game grazes: an amble every hour; a herd thinned by hunting comes back in time.
        if self.clock.tick % 60 == 0 { self.game_grazes(); }
        if self.clock.tick % 15 == 0 && !self.pets.is_empty() { self.pets_follow(); }
        if self.clock.hour() == 5 && minute == 0 && self.clock.day() % 4 == 0 && !self.hard_winter() { self.game_returns(); }
        for k in 0..self.creatures.len() {
            let speed = if matches!(self.creatures[k].kind, CreatureKind::Wolf | CreatureKind::CaveHunter) { 2 } else { 1 };
            // A walk in three dimensions (under the ground, up and down the stair).
            if !self.creatures[k].path3.is_empty() {
                self.creatures[k].stride += speed;
                let was_below = self.creature_below(&self.creatures[k]);
                loop {
                    let Some(&next) = self.creatures[k].path3.first() else { self.creatures[k].stride = 0; break };
                    let here = self.creatures[k].pos;
                    let c = nav::cost3(&self.map, next.0 as usize, next.1 as usize, next.2).map_or(30, |c| c as i32);
                    let c = if here == (next.0, next.1) { c + 6 } else if here.0 != next.0 && here.1 != next.1 { c * 14 / 10 } else { c };
                    let cost = c * if self.creatures[k].kind == CreatureKind::Beast { CREEP } else { 1 } / 2;
                    if self.creatures[k].stride < cost { break; }
                    self.creatures[k].stride -= cost;
                    self.creatures[k].pos = (next.0, next.1);
                    self.creatures[k].z = Some(next.2);
                    self.creatures[k].path3.remove(0);
                }
                if was_below && !self.creature_below(&self.creatures[k]) { self.creature_comes_up(k); }
                continue;
            }
            self.creatures[k].stride += speed;
            loop {
                let Some(&next) = self.creatures[k].path.first() else { self.creatures[k].stride = 0; break };
                let cost = nav::cost(&self.map, next.0 as usize, next.1 as usize).map_or(30, |c| c as i32) * if self.creatures[k].kind == CreatureKind::Wolf { 1 } else { CREEP } / 2;
                if self.creatures[k].stride < cost { break; }
                self.creatures[k].stride -= cost;
                self.creatures[k].pos = next;
                self.creatures[k].path.remove(0);
            }
        }
        self.creatures.retain(|c| !(c.leaving && c.path.is_empty() && c.path3.is_empty()));
    }

    fn dens(&self) -> Vec<Pos> {
        let n = self.map.width;
        (0..self.map.features.len()).filter(|&i| self.map.features[i] == Feature::Den).map(|i| ((i % n) as u16, (i / n) as u16))
            .filter(|d| !self.dens_cleared.contains(d)).collect()
    }

    fn wolves_out(&mut self) {
        if self.creatures.iter().any(|c| c.kind == CreatureKind::Wolf) { return; }
        let dens = self.dens();
        if dens.is_empty() { return; }
        let den = dens[(self.clock.day() as usize * 7 + self.seed as usize) % dens.len()];
        for k in 0..2u16 {
            let Some(at) = self.passable_near((den.0 as i32 + k as i32, den.1 as i32)) else { continue };
            let id = self.new_creature_id();
            self.creatures.push(Creature { kind: CreatureKind::Wolf, name: "a wolf".into(), pos: at, path: Vec::new(), stride: 0, leaving: false, size: 1.0, home: at, spawned: self.clock.tick, id, z: None, path3: Vec::new(), home_z: 0, out: false, rest_until: 0 });
        }
    }

    fn wolf_home(&mut self, k: usize) {
        let (from, home) = (self.creatures[k].pos, self.creatures[k].home);
        self.creatures[k].path = nav::path(&self.map, from, home, super::PATH_BUDGET).map(|p| p.into_iter().skip(1).collect()).unwrap_or_default();
        self.creatures[k].leaving = true;
    }

    fn wolves_hunt(&mut self) {
        let camp = self.camp;
        for k in 0..self.creatures.len() {
            if self.creatures[k].kind != CreatureKind::Wolf || self.creatures[k].leaving { continue; }
            let wp = self.creatures[k].pos;
            // A werebeast is bolder than wolves: anyone outdoors past the firelight, alone.
            let bold = self.creatures[k].name.ends_with(super::curse::MOON) || self.creatures[k].name.ends_with(super::curse::CHANGED);
            // (A werebeast takes whoever it reaches, company or not, and is then driven off.)
            let (fire_r, alone_r) = if bold { (-1, -1) } else { (7, 4) };
            let far_from_fire = |p: Pos| (p.0 as i32 - camp.0 as i32).abs().max((p.1 as i32 - camp.1 as i32).abs()) > fire_r;
            // The prey: a living settler outdoors, far from the fire, with no one else near.
            // (Debug: PLANET_FORCE_PET_PREY=1 also lets a keeper be taken wherever they are on the
            // surface, alone or not, to test the pet's defence: since settlers keep their own
            // hours no keeper was caught alone at night on sixteen seeds.)
            let forced = std::env::var("PLANET_FORCE_PET_PREY").is_ok();
            let prey = (0..self.settlers.len()).filter(|&i| {
                let s = &self.settlers[i];
                let keeper = forced && self.pets.iter().any(|p| p.alive && p.keeper == i);
                // (Never the changed one's own body.)
                s.alive && !self.below(i) && self.creatures[k].name.strip_suffix(super::curse::CHANGED) != Some(s.name.as_str())
                    && (s.pos.0 as i32 - wp.0 as i32).abs().max((s.pos.1 as i32 - wp.1 as i32).abs()) <= 40
                    && (keeper || (far_from_fire(s.pos) && (bold || self.roof_over(s.pos).is_none())
                    && !self.settlers.iter().enumerate().any(|(j, o)| j != i && o.alive && (o.pos.0 as i32 - s.pos.0 as i32).abs().max((o.pos.1 as i32 - s.pos.1 as i32).abs()) <= alone_r)))
            // (Debug: PLANET_FORCE_PET_PREY=1 makes a pet's keeper the first prey among those
            // alone, for the test: a keeper caught alone at night has grown rare.)
            }).min_by_key(|&i| (std::env::var("PLANET_FORCE_PET_PREY").is_ok() && !self.pets.iter().any(|p| p.alive && p.keeper == i),
                (self.settlers[i].pos.0 as i32 - wp.0 as i32).abs().max((self.settlers[i].pos.1 as i32 - wp.1 as i32).abs())));
            let Some(i) = prey else {
                // No lone settler: a pet strayed far from the fire will do (`pets.rs`).
                if bold { continue; }
                if self.wolves_take_pet(wp) { for j in 0..self.creatures.len() { if self.creatures[j].kind == CreatureKind::Wolf { self.wolf_home(j); } } return; }
                if let Some(pp) = self.stray_pets().into_iter().filter(|q| (q.0 as i32 - wp.0 as i32).abs().max((q.1 as i32 - wp.1 as i32).abs()) <= 40)
                    .min_by_key(|q| (q.0 as i32 - wp.0 as i32).abs().max((q.1 as i32 - wp.1 as i32).abs())) {
                    self.creatures[k].path = nav::path(&self.map, wp, pp, 4000).map(|p| p.into_iter().skip(1).collect()).unwrap_or_default();
                }
                continue;
            };
            let sp = self.settlers[i].pos;
            if (sp.0 as i32 - wp.0 as i32).abs().max((sp.1 as i32 - wp.1 as i32).abs()) <= 1 {
                // A pet at its keeper's side may stand over them (`pets.rs`; not against a
                // werebeast).
                let what = self.creatures[k].name.clone();
                if !bold && self.pet_defends(i, &what) {
                    for j in 0..self.creatures.len() { if self.creatures[j].kind == CreatureKind::Wolf { self.wolf_home(j); } }
                    return;
                }
                // A bite, and the pack goes home.
                let name = self.settlers[i].name.clone();
                self.settlers[i].ill_until = self.clock.tick + TICKS_PER_DAY;
                let what = self.creatures[k].name.clone();
                if what.ends_with(super::curse::MOON) { self.note(format!("{} breaks into the camp and bites {} before it is driven off.", what.trim_end_matches(super::curse::MOON), name)); }
                else if what == "a wolf" { self.note(format!("{} is bitten by wolves at the edge of the woods, alone and far from the fire.", name)); self.wolf_bites += 1; }
                else if let Some(who) = what.strip_suffix(super::curse::CHANGED) { self.note(format!("{} is set upon in the dark by a beast with {}'s eyes.", name, who)); }
                else if what.contains("risen") || what.contains("dead") { self.note(format!("{} is set upon by {} in the dark, alone and far from the fire.", name, what)); }
                else { self.note(format!("{} is set upon by {} come up from the mine, alone and far from the fire.", name, what)); self.cave_bites += 1; }
                self.cursed_bite(&what, i);
                for j in 0..self.creatures.len() { if self.creatures[j].kind == CreatureKind::Wolf { self.wolf_home(j); } }
                return;
            }
            // A werebeast finds its way round a palisade to a gate (a wider search).
            self.creatures[k].path = nav::path(&self.map, wp, sp, if bold { 30_000 } else { 4000 }).map(|p| p.into_iter().skip(1).collect()).unwrap_or_default();
        }
    }
}

impl Colony {
    pub(crate) fn new_creature_id(&mut self) -> u32 { self.next_creature += 1; self.next_creature }

    /// The embark's game (`LocalMap::game`) set out on the land, 20-70 cells from the camp.
    pub(crate) fn spawn_game(&mut self) {
        let herds = self.map.game.clone();
        for (k, (name, head)) in herds.iter().enumerate() {
            for j in 0..*head {
                let a = (self.seed.wrapping_add((k * 31 + j as usize) as u64 * 0x9E37) % 628) as f32 / 100.0;
                let r = 20.0 + ((self.seed >> 8).wrapping_add(j as u64 * 17 + k as u64 * 5) % 50) as f32;
                let p = ((self.camp.0 as f32 + a.cos() * r) as i32, (self.camp.1 as f32 + a.sin() * r) as i32);
                if let Some(at) = self.passable_near(p) {
                    let id = self.new_creature_id();
                    self.creatures.push(Creature { kind: CreatureKind::Game, name: name.clone(), pos: at, path: Vec::new(), stride: 0, leaving: false, size: 1.0, home: at, spawned: self.clock.tick, id, z: None, path3: Vec::new(), home_z: 0, out: false, rest_until: 0 });
                }
            }
        }
    }

    /// Grazing animals amble a few cells, and keep away from the fire.
    fn game_grazes(&mut self) {
        for k in 0..self.creatures.len() {
            if self.creatures[k].kind != CreatureKind::Game || !self.creatures[k].path.is_empty() { continue; }
            let c = &self.creatures[k];
            let h = (self.clock.tick / 60).wrapping_mul(2654435761).wrapping_add(c.id as u64 * 97);
            let (dx, dy) = ((h % 9) as i32 - 4, ((h / 9) % 9) as i32 - 4);
            let mut to = (c.pos.0 as i32 + dx, c.pos.1 as i32 + dy);
            // Shy of the camp.
            if (to.0 - self.camp.0 as i32).abs().max((to.1 - self.camp.1 as i32).abs()) < 15 { to = (c.home.0 as i32, c.home.1 as i32); }
            if let Some(t) = self.passable_near(to) {
                let from = self.creatures[k].pos;
                self.creatures[k].path = nav::path(&self.map, from, t, 400).map(|p| p.into_iter().skip(1).collect()).unwrap_or_default();
            }
        }
    }

    /// Every fourth dawn one head of game comes back where a herd was thinned.
    fn game_returns(&mut self) {
        let herds = self.map.game.clone();
        for (name, head) in herds {
            let now = self.creatures.iter().filter(|c| c.kind == CreatureKind::Game && c.name == name).count() as u32;
            if now < head {
                let home = self.creatures.iter().find(|c| c.name == name).map(|c| c.home).unwrap_or((self.camp.0.saturating_add(40), self.camp.1));
                if let Some(at) = self.passable_near((home.0 as i32, home.1 as i32)) {
                    let id = self.new_creature_id();
                    self.creatures.push(Creature { kind: CreatureKind::Game, name: name.clone(), pos: at, path: Vec::new(), stride: 0, leaving: false, size: 1.0, home: at, spawned: self.clock.tick, id, z: None, path3: Vec::new(), home_z: 0, out: false, rest_until: 0 });
                }
            }
        }
    }

    /// The nearest game within `r` cells of `from`: (id, where).
    pub(crate) fn nearest_game(&self, from: Pos, r: i32) -> Option<(u32, Pos)> {
        self.creatures.iter().filter(|c| c.kind == CreatureKind::Game && !self.game_unreachable.contains(&c.id))
            .map(|c| (c, (c.pos.0 as i32 - from.0 as i32).abs().max((c.pos.1 as i32 - from.1 as i32).abs())))
            .filter(|(_, d)| *d <= r).min_by_key(|(c, d)| (*d, c.id)).map(|(c, _)| (c.id, c.pos))
    }

    /// A hunt ends: if the quarry is within two cells, it is taken (six meals to carry home).
    pub(crate) fn finish_hunt(&mut self, i: usize, id: u32) -> bool {
        let me = self.settlers[i].pos;
        let Some(k) = self.creatures.iter().position(|c| c.id == id) else { return false };
        let c = &self.creatures[k];
        if (c.pos.0 as i32 - me.0 as i32).abs().max((c.pos.1 as i32 - me.1 as i32).abs()) > 2 { return false; }
        let name = c.name.clone();
        self.creatures.remove(k);
        // One who loves the creature is sorry to have killed it.
        if super::fond_of(&self.settlers[i].persona, &name) { let w = name.clone(); self.feel(i, super::mind::Feel::KilledLiked { what: w }); }
        for _ in 0..6 { self.items.push(super::Item::food(super::Stuff::Meat, me, false)); }
        let who = self.settlers[i].name.clone();
        self.once("hunt", format!("{} brings down the first {} at {},{}: meat for six meals.", who, name, me.0, me.1));
        self.hunted += 1;
        true
    }
}

