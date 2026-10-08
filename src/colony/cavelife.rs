//! Life in the caverns, walked in three dimensions.
//!
//! Dwarf Fortress gives each feature layer its own population (design guide ch. 11: the cavern
//! layers spawn their own creatures). Here a breached cavern (`mine.rs::breach_cavern`) gets a
//! few of the creatures its life list names (`LocalMap::caverns`): two hunters (pale spiders,
//! crawlers, serpents...) and three harmless (bats, crickets, crabs; the fish stay in the water),
//! each with a den on the cavern floor 8-36 cells from the stair's foot. They roam a few steps an
//! hour on their own floor. A hunter takes whoever is alone in the dark near it (a feller at the
//! fungus trees, a fisher at the still water): ill a day, and up the stair bleeding. One night in
//! three the two hunters nearest the surface climb the stair to hunt the camp's ground as wolves
//! do, and go back down at dawn. The hatch (`delve.rs::seal_caverns`) is a real barrier: no
//! creature's walk passes its place in the stair, so what is below it stays below. What the deep
//! sends (`mine.rs::wake_the_deep`, `deep.rs`) starts in its cavern, or at the shaft's foot out
//! of the hollow, and walks up the stair to the camp (it bursts the hatch).

use super::*;
use super::creatures::{Creature, CreatureKind};
use super::nav::P3;

/// Creatures set out on a breached cavern's floor: hunters and harmless life.
const HUNTERS: usize = 2;
const HARMLESS: usize = 3;
/// At most this many cavern creatures in all (three layers).
const CAP: usize = 15;
/// How far from its den a cavern creature roams before it turns back.
const ROAM: i32 = 12;
/// How near a hunter notices someone at work alone on its cavern's floor (cells), and the chance
/// in each twenty minutes that it does (one in `STALK`).
const NOTICE: i32 = 10;
const STALK: u64 = 4;
/// After a bite the cavern's hunters keep to their dens this long.
const REST: u64 = 6 * TICKS_PER_DAY;
/// Search budgets: a chase, and the climb up (or down) the stair.
const CHASE: usize = 3000;
const CLIMB: usize = 20_000;

/// Cavern life that hunts (the rest is harmless: bats, crickets, fish).
pub(crate) fn hunter(name: &str) -> bool {
    ["spider", "crawler", "horror", "hunt", "serpent", "eel", "mole", "grub", "toad", "lizard"].iter().any(|k| name.contains(k))
}

/// One of a kind: "giant cave spiders" -> "a giant cave spider", "things that hunt by sound" ->
/// "a thing that hunts by sound".
pub(crate) fn one_of(name: &str) -> String {
    if let Some(r) = name.strip_prefix("things that ") {
        let mut w = r.splitn(2, ' ');
        let verb = w.next().unwrap_or("hunt");
        return format!("a thing that {}s{}", verb, w.next().map(|x| format!(" {}", x)).unwrap_or_default());
    }
    let s = if name.ends_with("fish") { name.to_string() } else { name.strip_suffix('s').unwrap_or(name).to_string() };
    let an = s.starts_with(|c: char| "aeiou".contains(c));
    format!("{} {}", if an { "an" } else { "a" }, s)
}

fn cheb(a: (u16, u16), b: (u16, u16)) -> i32 { (a.0 as i32 - b.0 as i32).abs().max((a.1 as i32 - b.1 as i32).abs()) }

impl Colony {
    /// Where the stair stands on each breached cavern's floor: (layer, place).
    pub fn cavern_feet_pub(&self) -> &[(u8, P3)] { &self.cavern_feet }

    /// The floor level of cavern `layer` in column `p`, if it has floor there.
    fn layer_floor(&self, p: Pos, layer: usize) -> Option<i32> {
        let c = self.map.cavern_z.get(p.1 as usize * self.map.width + p.0 as usize)?.get(layer)?;
        (c.0 >= 0).then_some(c.0 as i32)
    }

    /// Which breached layer a creature living at `p` on level `z` belongs to.
    fn layer_of(&self, p: Pos, z: i32) -> Option<usize> {
        self.breached.iter().map(|&l| l as usize).find(|&l| self.layer_floor(p, l) == Some(z))
    }

    /// A dry place on cavern `layer`'s own floor (not under water, not the stair).
    fn on_floor(&self, q: P3, layer: usize) -> bool {
        self.layer_floor((q.0, q.1), layer) == Some(q.2)
            && (q.2 as usize + 1) < self.map.depth && self.map.cell(q.0 as usize, q.1 as usize, q.2 as usize + 1).water == 0
    }

    /// The floor of cavern `layer` walkable from `foot`, outward from it (at most `limit` places).
    fn cavern_reach(&self, layer: usize, foot: P3, limit: usize) -> Vec<P3> {
        let mut seen: crate::history::det::HashSet<P3> = Default::default();
        let mut out = vec![foot];
        seen.insert(foot);
        let mut nb = Vec::new();
        let mut k = 0;
        while k < out.len() && out.len() < limit {
            nav::steps3(&self.map, out[k], &mut nb);
            for &(q, _) in nb.iter() {
                if self.on_floor(q, layer) && seen.insert(q) { out.push(q); }
            }
            k += 1;
        }
        out
    }

    /// The breach: cavern `layer`'s own creatures set out on its floor, the stair's foot being
    /// at or under the cut `(p, z)` that opened it.
    pub(crate) fn populate_cavern(&mut self, layer: usize, p: Pos, z: i32) {
        // The foot: the cavern floor at the stair let down (or beside the cut).
        let foot = (-1i32..=1).flat_map(|dy| (-1i32..=1).map(move |dx| (dx, dy))).filter_map(|(dx, dy)| {
            let q = ((p.0 as i32 + dx).max(0) as u16, (p.1 as i32 + dy).max(0) as u16);
            let f = self.layer_floor(q, layer)?;
            (f <= z && nav::standable(&self.map, q.0 as usize, q.1 as usize, f)).then_some((q.0, q.1, f))
        }).next();
        let Some(foot) = foot else { return };
        if !self.cavern_feet.iter().any(|f| f.0 == layer as u8) { self.cavern_feet.push((layer as u8, foot)); }
        let Some(c) = self.map.caverns.iter().find(|c| c.layer as usize == layer).cloned() else { return };
        let reach = self.cavern_reach(layer, foot, 4000);
        let dens: Vec<P3> = reach.iter().copied().filter(|q| (8..=36).contains(&cheb((q.0, q.1), (foot.0, foot.1)))).collect();
        if dens.is_empty() { return; }
        let walkers: Vec<&String> = c.life.iter().filter(|l| !l.contains("fish")).collect();
        let hunters: Vec<&String> = walkers.iter().copied().filter(|l| hunter(l)).collect();
        let harmless: Vec<&String> = walkers.iter().copied().filter(|l| !hunter(l)).collect();
        let mut set: Vec<(CreatureKind, String)> = Vec::new();
        if !hunters.is_empty() { for k in 0..HUNTERS { set.push((CreatureKind::CaveHunter, one_of(hunters[k % hunters.len()]))); } }
        if !harmless.is_empty() { for k in 0..HARMLESS { set.push((CreatureKind::CaveLife, one_of(harmless[k % harmless.len()]))); } }
        for (k, (kind, name)) in set.into_iter().enumerate() {
            if self.creatures.iter().filter(|c| matches!(c.kind, CreatureKind::CaveHunter | CreatureKind::CaveLife)).count() >= CAP { break; }
            let at = dens[(crate::history::settlers::hash_pub(self.seed ^ (layer as u64 * 0x51), 0xCA0E + k as u64) % dens.len() as u64) as usize];
            let id = self.new_creature_id();
            if std::env::var("PLANET_DEBUG_CAVE").is_ok() { eprintln!("CAVE day {} layer {} {:?} {} den {:?} (foot {:?}, {} floor reached)", self.clock.day(), layer, kind, name, at, foot, reach.len()); }
            self.creatures.push(Creature { kind, name, pos: (at.0, at.1), path: Vec::new(), stride: 0, leaving: false, size: 1.0, home: (at.0, at.1), spawned: self.clock.tick, id,
                z: Some(at.2), path3: Vec::new(), home_z: at.2, out: false, rest_until: 0 });
        }
    }

    /// The level of the hatch on the stair: where the way up from the shallowest breached
    /// cavern first climbs the stair's own column (the step above where it comes onto it).
    pub(crate) fn hatch_level(&self) -> Option<i32> {
        let sp = self.spine?;
        let top = self.mine_mouth().or(self.delve_mouth).and_then(|m| self.passable_near_pub((m.0 as i32, m.1 as i32))).map(|p| nav::surface3(&self.map, p))?;
        let &(_, foot) = self.cavern_feet.iter().max_by_key(|f| f.1 .2)?;
        let way = self.creature_path3(foot, top, CLIMB, true)?;
        way.windows(2).find(|w| (w[0].0, w[0].1) == sp.at && (w[1].0, w[1].1) == sp.at && w[1].2 > w[0].2).map(|w| w[1].2)
    }

    /// Debug (`PLANET_DEBUG_CAVE`): whether each breached cavern's floor still has a way up the
    /// stair to the mouth for a creature (the hatch barring it).
    pub(crate) fn debug_cave_ways(&self, when: &str) {
        if std::env::var("PLANET_DEBUG_CAVE").is_err() { return; }
        let Some(top) = self.mine_mouth().and_then(|m| self.passable_near_pub((m.0 as i32, m.1 as i32))).map(|p| nav::surface3(&self.map, p)) else { return };
        for &(l, foot) in &self.cavern_feet {
            let way = self.creature_path3(foot, top, CLIMB, false).map(|p| p.len());
            eprintln!("CAVE day {} {}: layer {} foot {:?} -> mouth {:?}: {:?} (hatch {:?})", self.clock.day(), when, l, foot, top, way, self.hatch);
        }
    }

    /// Every hour cavern creatures at home amble a few steps on their own floor, turning back
    /// toward their den when they have strayed (a hunter after a chase too).
    pub(crate) fn cave_life_roams(&mut self) {
        let mut nb = Vec::new();
        for k in 0..self.creatures.len() {
            let c = &self.creatures[k];
            if !matches!(c.kind, CreatureKind::CaveHunter | CreatureKind::CaveLife) || c.out || !c.path3.is_empty() || c.z.is_none() { continue; }
            let Some(layer) = self.layer_of(c.home, c.home_z) else { continue };
            let mut at = self.creature_here3(c);
            let home = c.home;
            let mut walk = Vec::new();
            let h = (self.clock.tick / 60).wrapping_mul(2654435761).wrapping_add(c.id as u64 * 97);
            for s in 0..(2 + h % 3) {
                nav::steps3(&self.map, at, &mut nb);
                let opts: Vec<P3> = nb.iter().map(|o| o.0).filter(|&q| self.on_floor(q, layer)).collect();
                if opts.is_empty() { break; }
                let next = if cheb((at.0, at.1), home) > ROAM {
                    *opts.iter().min_by_key(|q| (cheb((q.0, q.1), home), q.1, q.0)).unwrap()
                } else {
                    opts[((h >> (8 + 4 * s)) % opts.len() as u64) as usize]
                };
                walk.push(next);
                at = next;
            }
            self.creatures[k].path3 = walk;
        }
    }

    /// Dusk, one night in three: the cavern's hunters nearest the surface climb the stair to hunt
    /// the camp's ground (not past the hatch).
    pub(crate) fn cave_hunters_out(&mut self) {
        let Some(_) = self.cave_hunter.as_ref() else { return };
        let Some(mouth) = self.mine_mouth() else { return };
        // Not every night: one in three, hashed by the day.
        if (self.clock.day().wrapping_mul(0x9E37_79B9) ^ self.seed) % 3 != 0 { return; }
        let Some(top) = self.passable_near_pub((mouth.0 as i32, mouth.1 as i32)).map(|p| nav::surface3(&self.map, p)) else { return };
        let tick = self.clock.tick;
        let hatch = self.hatch.map(|h| h.1);
        let mut who: Vec<usize> = (0..self.creatures.len()).filter(|&k| {
            let c = &self.creatures[k];
            c.kind == CreatureKind::CaveHunter && !c.out && c.rest_until <= tick && c.z.map_or(false, |z| hatch.map_or(true, |h| z > h))
        }).collect();
        who.sort_by_key(|&k| (-self.creatures[k].z.unwrap_or(0), self.creatures[k].id));
        let mut path: Option<Vec<P3>> = None;
        for &k in who.iter().take(2) {
            let from = self.creature_here3(&self.creatures[k]);
            // (The second follows the first's way when it starts where the first did.)
            let p = match &path { Some(p) if p.first().map_or(false, |q| cheb((q.0, q.1), (from.0, from.1)) <= 1) => Some(p.clone()), _ => self.creature_path3(from, top, CLIMB, false) };
            if std::env::var("PLANET_DEBUG_CAVE").is_ok() { eprintln!("CAVE day {} climb {} from {:?}: {:?}", self.clock.day(), self.creatures[k].name, from, p.as_ref().map(|p| p.len())); }
            let Some(p) = p else { continue };
            path = Some(p.clone());
            let c = &mut self.creatures[k];
            c.out = true;
            c.path3 = p;
        }
    }

    /// Dawn: the hunters out of their cavern go back down to their dens.
    pub(crate) fn cave_hunters_home(&mut self) {
        for k in 0..self.creatures.len() {
            if self.creatures[k].kind == CreatureKind::CaveHunter && self.creatures[k].out { self.cave_hunter_home(k); }
        }
    }

    /// Hunter `k` goes back to its den (out of sight at once when it can find no way).
    fn cave_hunter_home(&mut self, k: usize) {
        let c = &self.creatures[k];
        let (from, home) = (self.creature_here3(c), (c.home.0, c.home.1, c.home_z));
        let p = self.creature_path3(from, home, CLIMB, false);
        let c = &mut self.creatures[k];
        c.out = false;
        match p {
            Some(p) => c.path3 = p,
            None => { c.pos = (home.0, home.1); c.z = Some(home.2); c.path3.clear(); }
        }
    }

    /// A creature walking in three dimensions reaches the surface: the cavern's hunters come up
    /// the mine; what the deep sent climbs out of it.
    pub(crate) fn creature_comes_up(&mut self, k: usize) {
        match self.creatures[k].kind {
            CreatureKind::CaveHunter if self.creatures[k].out => {
                let what = self.cave_hunter.clone().unwrap_or_else(|| self.creatures[k].name.clone());
                self.once("cave hunters", format!("Something comes up the mine after dark: {}, hunting.", what));
            }
            CreatureKind::Beast if !self.creatures[k].out && !self.creatures[k].leaving => {
                self.creatures[k].out = true;
                let name = self.creatures[k].name.clone();
                let at = self.creatures[k].pos;
                let (short, why) = self.arc.as_ref().map(|a| (a.threat.monster.as_ref().map(|m| m.short.clone()).unwrap_or_default(), a.threat.why.clone())).unwrap_or_default();
                let line = format!("Something climbs out of the mine: {}.", name);
                self.note(line.clone());
                let burst = if self.hatch.is_some() { " The hatch in the stair lies splintered below it." } else { "" };
                self.moment("Out of the deep".into(), format!("{} {} heaves itself out of the mine's mouth.{}", line, super::arc::capital_word(&short), burst), format!("because {}", why), at);
                for j in 0..self.settlers.len() { if self.settlers[j].alive { let w = short.clone(); self.feel(j, mind::Feel::TheDeep { what: w }); } }
            }
            _ => {}
        }
    }

    /// The deep's attacker sets out from below: out of the hollow at the deep shaft's foot, or
    /// from its cavern's floor, up the stair to the camp. False when it has no way up.
    pub(crate) fn deep_comes_up(&mut self, name: &str, size: f32) -> bool {
        let Some(mouth) = self.mine_mouth() else { return false };
        let Some(top) = self.passable_near_pub((mouth.0 as i32, mouth.1 as i32)).map(|p| nav::surface3(&self.map, p)) else { return false };
        let hollow = self.arc.as_ref().map_or(false, |a| a.threat.why.contains("hollow"));
        let (start, whence) = if hollow {
            // The shaft's foot: the lowest place on the stair one can stand.
            let Some(sp) = self.spine else { return false };
            let Some(z) = (sp.bottom..sp.bottom + 6).find(|&z| nav::standable(&self.map, sp.at.0 as usize, sp.at.1 as usize, z)) else { return false };
            ((sp.at.0, sp.at.1, z), "the hollow at the foot of the deep shaft".to_string())
        } else {
            // The beast's own cavern if the stair reaches it, else the deepest one it does: it
            // comes up out of the deep into it.
            let beast_layer = self.map.caverns.iter().find(|c| c.beast.is_some()).map(|c| c.layer);
            let Some(&(layer, foot)) = self.cavern_feet.iter().filter(|f| Some(f.0) == beast_layer).chain(self.cavern_feet.iter().filter(|f| Some(f.0) != beast_layer)).max_by_key(|f| (Some(f.0) == beast_layer, f.0)) else { return false };
            let reach = self.cavern_reach(layer as usize, foot, 3000);
            let far: Vec<P3> = reach.iter().copied().filter(|q| (10..=20).contains(&cheb((q.0, q.1), (foot.0, foot.1)))).collect();
            let at = if far.is_empty() { *reach.last().unwrap_or(&foot) } else { far[(crate::history::settlers::hash_pub(self.seed, 0xBEA5) % far.len() as u64) as usize] };
            let cname = self.map.caverns.iter().find(|c| c.layer == layer).map(|c| c.name.clone()).unwrap_or_else(|| "the cavern".into());
            (at, cname)
        };
        let Some(path3) = self.creature_path3(start, top, super::PATH_BUDGET, true) else { return false };
        let id = self.new_creature_id();
        self.creatures.push(Creature { kind: CreatureKind::Beast, name: name.to_string(), pos: (start.0, start.1), path: Vec::new(), stride: 0, leaving: false, size, home: (start.0, start.1), spawned: self.clock.tick, id,
            z: Some(start.2), path3, home_z: start.2, out: false, rest_until: 0 });
        self.raid_side = "the mine".into();
        let levels = self.map.surface_z[top.1 as usize * self.map.width + top.0 as usize] - start.2;
        self.note(format!("Something stirs in {}, {} levels down: {} is coming up the stair.", whence, levels, name));
        true
    }

    /// Every twenty minutes: a hunter at home takes whoever works alone on its cavern's floor
    /// near its den; a hunter up the stair for the night hunts the camp's ground as wolves do. A
    /// chase that finds no way waits an hour.
    pub(crate) fn cave_hunt(&mut self) {
        let (tick, camp) = (self.clock.tick, self.camp);
        // Who hunts now: below at home (now and then, or already stalking), or up for the night.
        let hunting: Vec<(usize, bool)> = (0..self.creatures.len()).filter_map(|k| {
            let c = &self.creatures[k];
            if c.kind != CreatureKind::CaveHunter || c.rest_until > tick { return None; }
            let low = self.creature_below(c);
            // (Climbing up, or walking home on the surface at dawn: not hunting.)
            if low == c.out { return None; }
            if low && c.path3.is_empty() && crate::history::settlers::hash_pub(self.seed ^ tick, 0x57A1 + c.id as u64) % STALK != 0 { return None; }
            Some((k, low))
        }).collect();
        if hunting.is_empty() { return; }
        let at: Vec<Option<P3>> = (0..self.settlers.len()).map(|i| self.settlers[i].alive.then(|| self.here3(i))).collect();
        let alone = |i: usize, p: P3| !at.iter().enumerate().any(|(j, q)| j != i && q.map_or(false, |q| cheb((q.0, q.1), (p.0, p.1)) <= 4 && (q.2 - p.2).abs() <= 2));
        for (k, low) in hunting {
            let c = &self.creatures[k];
            let me = self.creature_here3(c);
            let home = c.home;
            let layer = self.layer_of(c.home, c.home_z);
            let prey = (0..self.settlers.len()).filter_map(|i| at[i].map(|s| (i, s))).filter(|&(i, s)| {
                let d = cheb((s.0, s.1), (me.0, me.1));
                let near = if low {
                    // At work on its own cavern's floor (a feller, a fisher), not passing on the stair.
                    d <= NOTICE && cheb((s.0, s.1), home) <= 2 * ROAM && self.settlers[i].path.is_empty()
                        && layer.map_or(false, |l| self.layer_floor((s.0, s.1), l) == Some(s.2))
                } else {
                    d <= 40 && cheb((s.0, s.1), camp) > 7 && s.2 >= self.map.surface_z[s.1 as usize * self.map.width + s.0 as usize] && self.roof_over((s.0, s.1)).is_none()
                };
                near && alone(i, s)
            }).min_by_key(|&(i, s)| (cheb((s.0, s.1), (me.0, me.1)), i));
            let Some((i, s)) = prey else { continue };
            if cheb((s.0, s.1), (me.0, me.1)) <= 1 && (s.2 - me.2).abs() <= 1 {
                self.cave_bite(k, i, low, layer);
                return;
            }
            // (Still on the way to where the prey stands: no new search.)
            if self.creatures[k].path3.last().map_or(false, |q| cheb((q.0, q.1), (s.0, s.1)) <= 1 && (q.2 - s.2).abs() <= 1) { continue; }
            match self.creature_path3(me, s, CHASE, false) {
                Some(p) => self.creatures[k].path3 = p,
                None => self.creatures[k].rest_until = tick + 60,
            }
        }
    }

    /// Hunter `k` sets upon settler `i`: ill a day; in the dark they drop their work and come up
    /// the stair bleeding; on the surface the hunters go back down.
    fn cave_bite(&mut self, k: usize, i: usize, low: bool, layer: Option<usize>) {
        let what = self.creatures[k].name.clone();
        let name = self.settlers[i].name.clone();
        // On the surface a pet at its keeper's side may stand over them, as against wolves
        // (`pets.rs`); the hunters go back down.
        if !low && self.pet_defends(i, &what) {
            self.cave_hunters_home();
            return;
        }
        self.settlers[i].ill_until = self.clock.tick + TICKS_PER_DAY;
        self.cave_bites += 1;
        self.creatures[k].rest_until = self.clock.tick + REST;
        if low {
            // The cavern's hunters all keep to their dens a while after.
            for j in 0..self.creatures.len() {
                let c = &self.creatures[j];
                if j != k && c.kind == CreatureKind::CaveHunter && !c.out && self.layer_of(c.home, c.home_z) == layer {
                    self.creatures[j].rest_until = self.clock.tick + REST;
                    self.cave_hunter_home(j);
                }
            }
            let cavern = self.map.caverns.iter().find(|c| Some(c.layer as usize) == layer).map(|c| c.name.clone()).unwrap_or_else(|| "the cavern".into());
            let doing = match self.settlers[i].job { Job::Fell(_) => " while felling", Job::Fish(_) => " while fishing", _ => "" };
            self.note(format!("{} is set upon by {}{} in the dark of {}, and comes up the stair bleeding.", name, what, doing, cavern));
            self.feel(i, mind::Feel::TheDeep { what: format!("{} in the dark", what.trim_start_matches("a ").trim_start_matches("an ")) });
            if matches!(self.settlers[i].job, Job::Fell(_) | Job::Fish(_)) { self.release(i); }
            self.cave_hunter_home(k);
        } else {
            self.note(format!("{} is set upon by {} come up from the mine, alone and far from the fire.", name, what));
            self.cave_hunters_home();
        }
    }
}
