//! Threats seen by day (card 'Monsters and threats roaming the surface'; the user, 2026-10-08:
//! "I didn't see any monsters roaming the surface level, nor any threats").
//!
//! Danger shows itself before it strikes. Where the tile's ecology keeps predators (wolves, bears,
//! lions: `LocalMap::predators`), one now and then comes out by day from its den, or in from the
//! edge of the map, stalks to the edge of the camp's sight (22-28 cells from the fire), watches a
//! while and slinks off, sooner if anyone comes near. Two days before a raid (the day before its
//! eve, when the attackers set out at dusk) the threat's scout rides in from the side the raid
//! will come from, sits watching the camp an hour from 34 cells out and goes; a beast is itself
//! seen far off, and turns away. Beasts of troubles still to come
//! pass along the far side of the map by daylight. Each is drawn on the map and named in red; the
//! first sighting of each is a moment.
//!
//! They only show themselves: nobody is struck by day (the night's wolves and the raid do that),
//! and their ids are drawn from their own range so the other creatures' are unchanged.

use super::creatures::{Creature, CreatureKind};
use super::*;

/// Ids of the day's prowlers and scouts, apart from the other creatures' (whose ids feed hashes).
const PROWL_IDS: u32 = 0x4000_0000;

fn hash(seed: u64, salt: u64) -> u64 { crate::history::settlers::hash_pub(seed, salt) }

fn compass(dx: f32, dy: f32) -> &'static str {
    let a = dy.atan2(dx).to_degrees();
    match ((a + 360.0 + 22.5) % 360.0 / 45.0) as usize {
        0 => "the east", 1 => "the south-east", 2 => "the south", 3 => "the south-west",
        4 => "the west", 5 => "the north-west", 6 => "the north", _ => "the north-east",
    }
}

impl Colony {
    fn prowl_id(&mut self) -> u32 { self.prowl_count += 1; PROWL_IDS + self.prowl_count }

    fn prowling(&self) -> bool { self.creatures.iter().any(|c| matches!(c.kind, CreatureKind::Prowler | CreatureKind::Scout)) }

    /// The day's work of what prowls (called every tick from `step_creatures`).
    pub(crate) fn prowl_tick(&mut self) {
        let (hour, minute) = (self.clock.hour(), self.clock.minute());
        if minute == 0 && self.alive() > 0 {
            let day = self.clock.day();
            let h = hash(self.seed ^ day, 0x9A0E);
            if hour == 8 + h % 7 { self.predator_comes(h); }
            if hour == 8 + (h >> 12) % 3 { self.scout_comes(); }
            if hour == 11 && day % 13 == self.seed % 13 { self.beast_passes(); }
        }
        if self.clock.tick % 10 == 5 { self.prowlers_watch(); }
    }

    /// The tile's predators, by day: one comes to the edge of sight now and then (a day in four
    /// to a day in two, by how many live here), from a den if the embark has one.
    fn predator_comes(&mut self, h: u64) {
        if self.map.predators.is_empty() || self.prowling() || self.clock.hour() >= 16 { return; }
        let total: f32 = self.map.predators.iter().map(|p| p.1).sum();
        let chance = (0.1 + 0.45 * total).min(0.35);
        if ((h >> 8) % 1000) as f32 >= chance * 1000.0 { return; }
        // Which: the tile's pack, its commonest predator (`bestiary.rs`: one pack, not three).
        let Some((name, _)) = self.pack() else { return };
        // From a den, else in from the edge on a side the day picks.
        let dens: Vec<Pos> = (0..self.map.features.len()).filter(|&i| self.map.features[i] == crate::local::wildlife::Feature::Den)
            .map(|i| ((i % self.map.width) as u16, (i / self.map.width) as u16)).filter(|d| !self.dens_cleared.contains(d)).collect();
        let c = (self.camp.0 as f32, self.camp.1 as f32);
        let from = if !dens.is_empty() && name.contains("wolf") || !dens.is_empty() && (h >> 30) % 2 == 0 {
            dens[(h >> 33) as usize % dens.len()]
        } else {
            let a = ((h >> 36) % 628) as f32 / 100.0;
            match self.passable_near_pub(((c.0 + a.cos() * 75.0) as i32, (c.1 + a.sin() * 75.0) as i32)) { Some(p) => p, None => return }
        };
        // To the edge of sight on the line from home to the fire.
        let (dx, dy) = (from.0 as f32 - c.0, from.1 as f32 - c.1);
        let d = (dx * dx + dy * dy).sqrt().max(1.0);
        let r = 22.0 + ((h >> 40) % 7) as f32;
        if d < r + 4.0 { return; }
        let Some(spot) = self.passable_near_pub(((c.0 + dx / d * r) as i32, (c.1 + dy / d * r) as i32)) else { return };
        let Some(path) = nav::path(&self.map, from, spot, 6000) else { return };
        let id = self.prowl_id();
        let a = if name.starts_with(['a', 'e', 'i', 'o', 'u']) { "an" } else { "a" };
        self.creatures.push(Creature { kind: CreatureKind::Prowler, name: format!("{} {}", a, name), pos: from, path: path.into_iter().skip(1).collect(), stride: 0, leaving: false,
            size: 1.0, home: from, spawned: self.clock.tick, id, z: None, path3: Vec::new(), home_z: 0, out: false, rest_until: 0 });
    }

    /// The day before the raid, the threat's scout (a beast: itself, far off).
    fn scout_comes(&mut self) {
        let Some(a) = self.arc.as_ref() else { return };
        // (The day before the attackers set out: they leave at dusk on the raid's eve.)
        if a.stage != 2 || self.clock.day() + 2 != a.raid_day || self.siege.is_some() || self.scouted == a.raid_day { return; }
        let t = a.threat.clone();
        let raid_day = a.raid_day;
        if t.kind == super::arc::ThreatKind::Deep { return; }
        self.scouted = raid_day;
        let ((dx, dy), _) = self.threat_side_pub();
        let c = (self.camp.0 as f32, self.camp.1 as f32);
        let beast = t.kind == super::arc::ThreatKind::Beast;
        let Some(at) = self.passable_near_pub(((c.0 + dx * 64.0) as i32, (c.1 + dy * 64.0) as i32)) else { return };
        let r = if beast { 42.0 } else { 34.0 };
        let Some(spot) = self.passable_near_pub(((c.0 + dx * r) as i32, (c.1 + dy * r) as i32)) else { return };
        let Some(path) = nav::path(&self.map, at, spot, super::PATH_BUDGET) else { return };
        if let Some(m) = t.monster.as_ref() { if !self.foes_seen.iter().any(|f| f.0 == t.name) { self.foes_seen.push((t.name.clone(), m.clone())); } }
        let id = self.prowl_id();
        let (kind, name, size) = if beast { (CreatureKind::Prowler, t.name.clone(), t.size) } else { (CreatureKind::Scout, format!("a scout from {}", t.name), 1.0) };
        self.creatures.push(Creature { kind, name, pos: at, path: path.into_iter().skip(1).collect(), stride: 0, leaving: false,
            size, home: at, spawned: self.clock.tick, id, z: None, path3: Vec::new(), home_z: 0, out: false, rest_until: 0 });
    }

    /// Now and then a beast of a trouble still to come crosses the far side of the map by day.
    fn beast_passes(&mut self) {
        if self.prowling() || self.attackers_out() { return; }
        let Some(a) = self.arc.as_ref() else { return };
        let Some(t) = a.later.iter().chain(a.reserve.iter()).find(|t| t.kind == super::arc::ThreatKind::Beast && t.name != a.threat.name).cloned() else { return };
        // Which side it is on (its lair's), across the far half of the map from the camp.
        let (tx, ty) = self.map.world_tile;
        let (mut dx, mut dy) = t.from.map_or((0.0, 0.0), |f| (f.0 as f32 - tx as f32, f.1 as f32 - ty as f32));
        if dx.abs() + dy.abs() < 0.01 { let a = (self.seed % 360) as f32 / 57.3; dx = a.cos(); dy = a.sin(); }
        let n = (dx * dx + dy * dy).sqrt();
        let (dx, dy) = (dx / n, dy / n);
        let c = (self.camp.0 as f32, self.camp.1 as f32);
        let (Some(from), Some(to)) = (self.passable_near_pub(((c.0 + dx * 72.0 - dy * 60.0) as i32, (c.1 + dy * 72.0 + dx * 60.0) as i32)),
            self.passable_near_pub(((c.0 + dx * 72.0 + dy * 60.0) as i32, (c.1 + dy * 72.0 - dx * 60.0) as i32))) else { return };
        let Some(path) = nav::path(&self.map, from, to, super::PATH_BUDGET) else { return };
        // (It keeps to the far side: a way that comes near the camp is not taken.)
        if path.iter().any(|p| (p.0 as f32 - c.0).abs().max((p.1 as f32 - c.1).abs()) < 40.0) { return; }
        if let Some(m) = t.monster.as_ref() { if !self.foes_seen.iter().any(|f| f.0 == t.name) { self.foes_seen.push((t.name.clone(), m.clone())); } }
        let id = self.prowl_id();
        let side = compass(dx, dy);
        self.creatures.push(Creature { kind: CreatureKind::Prowler, name: t.name.clone(), pos: from, path: path.into_iter().skip(1).collect(), stride: 0, leaving: true,
            size: t.size, home: to, spawned: self.clock.tick, id, z: None, path3: Vec::new(), home_z: 0, out: false, rest_until: 0 });
        let line = format!("Far off to {}, {} crosses the hills by daylight. It does not turn toward the camp.", side, t.name);
        self.note(line.clone());
        if !self.prowled.contains(&t.name) {
            self.prowled.push(t.name.clone());
            self.moment("Seen far off".into(), line, format!("because {}", t.why), from);
        }
    }

    /// Prowlers and scouts at the edge of sight: they watch, and go.
    fn prowlers_watch(&mut self) {
        let camp = (self.camp.0 as i32, self.camp.1 as i32);
        for k in 0..self.creatures.len() {
            let c = &self.creatures[k];
            if !matches!(c.kind, CreatureKind::Prowler | CreatureKind::Scout) || c.leaving { continue; }
            let (pos, scout) = (c.pos, c.kind == CreatureKind::Scout);
            let far = |p: Pos| (p.0 as i32 - camp.0).abs().max((p.1 as i32 - camp.1).abs());
            // Anyone coming near: it slinks off (a scout turns his horse).
            let near = (0..self.settlers.len()).find(|&i| self.settlers[i].alive && !self.below(i)
                && (self.settlers[i].pos.0 as i32 - pos.0 as i32).abs().max((self.settlers[i].pos.1 as i32 - pos.1 as i32).abs()) <= if scout { 10 } else { 7 });
            if let Some(i) = near {
                let (who, what) = (self.settlers[i].name.clone(), self.creatures[k].name.clone());
                let line = if scout { format!("{} rides out toward {}, who turns and is gone.", who, what) }
                    else { format!("{} comes upon {} {} cells from the fire; it slinks off.", who, what, far(pos)) };
                self.note(line);
                self.prowler_home(k);
                continue;
            }
            if !self.creatures[k].path.is_empty() { continue; }
            // At the edge of sight: it watches a while (an hour or two; a scout an hour), and goes
            // before dusk.
            if self.creatures[k].rest_until == 0 {
                let linger = if scout { 60 } else { 60 + hash(self.seed, self.creatures[k].id as u64) % 70 };
                self.creatures[k].rest_until = self.clock.tick + linger;
                self.prowler_seen(k);
            } else if self.clock.tick >= self.creatures[k].rest_until || self.clock.hour() >= 18 {
                self.prowler_home(k);
            }
        }
    }

    fn prowler_home(&mut self, k: usize) {
        let (from, home) = (self.creatures[k].pos, self.creatures[k].home);
        self.creatures[k].path = nav::path(&self.map, from, home, super::PATH_BUDGET).map(|p| p.into_iter().skip(1).collect()).unwrap_or_default();
        self.creatures[k].leaving = true;
    }

    /// It stands watching: the camp sees it (a line; a moment the first time of its kind).
    fn prowler_seen(&mut self, k: usize) {
        let c = self.creatures[k].clone();
        let (dx, dy) = (c.pos.0 as f32 - self.camp.0 as f32, c.pos.1 as f32 - self.camp.1 as f32);
        let side = compass(dx, dy);
        let cells = dx.abs().max(dy.abs()) as i32;
        if c.kind == CreatureKind::Scout {
            let t = self.arc.as_ref().map(|a| a.threat.clone());
            let line = format!("A rider sits watching the camp from {}, {} cells out: {}. In an hour they are gone.", side, cells, c.name);
            self.note(line.clone());
            let because = t.map(|t| format!("because {} {}", t.name, if t.name.starts_with("raiders") || t.name.starts_with("outlaws") { "are" } else { "is" }) + " coming, and sends ahead to look").unwrap_or_default();
            self.moment("Watched".into(), line, because, c.pos);
            return;
        }
        let beast = self.arc.as_ref().map_or(false, |a| a.threat.name == c.name);
        if beast {
            let line = format!("Far off to {}, {} stands in daylight looking toward the camp, {} cells out, and turns away.", side, c.name, cells);
            self.note(line.clone());
            let why = self.arc.as_ref().map(|a| a.threat.why.clone()).unwrap_or_default();
            self.moment("Seen far off".into(), line, format!("because {}", why), c.pos);
            return;
        }
        // Among trees, at their edge or in the open (trees within two cells).
        let trees = (-2i32..=2).flat_map(|dy| (-2i32..=2).map(move |dx| (dx, dy))).filter(|&(dx, dy)| {
            let (x, y) = ((c.pos.0 as i32 + dx).clamp(0, self.map.width as i32 - 1) as usize, (c.pos.1 as i32 + dy).clamp(0, self.map.height as i32 - 1) as usize);
            matches!(self.map.cells[self.map.idx(x, y, self.map.surface_z[y * self.map.width + x] as usize)].plant, crate::local::Plant::Tree(_))
        }).count();
        let ground = match trees { 0 => "the open ground", 1..=5 => "the edge of the trees", _ => "among the trees" };
        let line = format!("{} watches the camp from {} to {}, {} cells from the fire.", super::arc::capital_word(&c.name), ground, side, cells);
        self.note(line.clone());
        let kind = c.name.split_whitespace().last().unwrap_or("").to_string();
        if !self.prowled.contains(&kind) {
            self.prowled.push(kind.clone());
            let d = self.map.predators.iter().find(|p| p.0 == kind).map_or(0.0, |p| p.1);
            self.moment("Watched from the edge".into(), line, format!("because {} live in these hills ({:.2} of what the land can keep), and come to see what is new", crate::history::ecology::SPECIES.iter().find(|s| s.name == kind).map_or(kind.as_str(), |s| s.plural), d), c.pos);
        }
    }
}
