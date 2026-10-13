//! The raid fought on the map (card 'Raids that visibly fight'; the user, 2026-10-08: "The raid
//! is just a black blob that comes and does nothing").
//!
//! The raid is still reckoned in one stroke where the attackers meet the camp (`raid_at`: who
//! dies, who is saved, whether they are routed), and the blows are told in the log
//! (`fight.rs`). Now it is also fought where it happened, for as long as such a fight lasts (20
//! minutes for a rout, 35 for a rescue, 45 when someone dies): the defenders (the one who saves
//! the struck, then the nearest grown hands on the surface, as many as the band and one more)
//! run to the attackers and stand at them, weapons out, and blows fly between each attacker and
//! the one at it (`fx_ink::draw_fights`); the struck, if they live, fall back to the fire with
//! the one who saved them; children and the old run indoors. The attackers hold their ground,
//! turning on whoever is nearest. Then the band breaks and runs off into the dark the way it
//! came, at a run; a beast slain in the fight falls where it fought and is gone from the map.

use super::creatures::CreatureKind;
use super::*;

/// The raid as it is fought on the map, after `raid_at` has reckoned it.
#[derive(Clone, Debug)]
pub struct Melee {
    pub at: Pos,
    pub until: u64,
    /// The one struck (alive: they fall back to the fire), and the one who saved them.
    pub struck: Option<usize>,
    pub saviour: Option<usize>,
    /// Those who run to meet the attackers.
    pub defenders: Vec<usize>,
    /// Whether the attackers are routed (they break sooner), or won the night.
    pub routed: bool,
}

impl Colony {
    /// After `raid_at` reckoned the raid at `at`: it is fought there for a while. `dead` the one
    /// who was killed (no longer on the map), `struck` one struck down who lives, `saviour` who
    /// dragged them back.
    pub(crate) fn begin_melee(&mut self, at: Pos, struck: Option<usize>, saviour: Option<usize>, killed: bool, routed: bool) {
        let tick = self.clock.tick;
        let d = |s: &Settler| (s.pos.0 as i32 - at.0 as i32).abs().max((s.pos.1 as i32 - at.1 as i32).abs());
        let band = self.creatures.iter().filter(|c| matches!(c.kind, CreatureKind::Beast | CreatureKind::Raider) && !c.leaving).count().max(1);
        // Those who stand aside against their own people do not fight (`fight.rs`).
        let aside = |c: &Colony, i: usize| c.fighting_people.is_some() && c.settlers[i].past.as_ref().and_then(|p| p.people) == c.fighting_people;
        let mut defenders: Vec<usize> = saviour.into_iter().filter(|&i| self.settlers[i].alive).collect();
        let mut near: Vec<usize> = (0..self.settlers.len()).filter(|&i| {
            let s = &self.settlers[i];
            s.alive && !self.below(i) && Some(i) != struck && !defenders.contains(&i) && d(s) <= 40
                && s.past.as_ref().map_or(true, |p| p.age >= 14 && p.age < 65) && s.ill_until <= tick && !aside(self, i)
        }).collect();
        near.sort_by_key(|&i| (d(&self.settlers[i]), i));
        for i in near { if defenders.len() > band { break; } defenders.push(i); }
        let until = tick + if killed { 45 } else if routed { 20 } else { 35 };
        self.melee = Some(Melee { at, until, struck: struck.filter(|&i| self.settlers[i].alive), saviour, defenders, routed });
        // The attackers hold where they met the camp.
        for c in self.creatures.iter_mut().filter(|c| matches!(c.kind, CreatureKind::Beast | CreatureKind::Raider) && !c.leaving) { c.path.clear(); c.path3.clear(); }
        // Those in it reconsider at once (the defenders run, the struck fall back, the young
        // awake near it hide); everyone else goes on as they were.
        for i in 0..self.settlers.len() { if self.settlers[i].alive && self.in_melee(i) { self.decide(i, true); } }
    }

    /// Whether settler `i` has a part in the melee: fighting, struck, saving, or young or old,
    /// awake on the surface near it.
    fn in_melee(&self, i: usize) -> bool {
        let Some(m) = self.melee.as_ref() else { return false };
        m.defenders.contains(&i) || m.struck == Some(i) || m.saviour == Some(i) || self.hides(i, m.at)
    }

    fn hides(&self, i: usize, at: Pos) -> bool {
        let s = &self.settlers[i];
        s.alive && !self.below(i) && !matches!(s.job, Job::Sleep) && s.past.as_ref().map_or(false, |p| p.age < 14 || p.age >= 65)
            && (s.pos.0 as i32 - at.0 as i32).abs().max((s.pos.1 as i32 - at.1 as i32).abs()) <= 40
    }

    /// Whether settler `i` is fighting in the melee (drawn with their weapon out).
    pub fn melee_fighting(&self, i: usize) -> bool { self.melee.as_ref().map_or(false, |m| m.defenders.contains(&i)) }

    /// What the melee asks of settler `i`, while it lasts.
    pub(crate) fn melee_choice(&self, i: usize) -> Option<(Job, String)> {
        let m = self.melee.as_ref()?;
        let foe = self.arc.as_ref().map(|a| a.threat.name.clone()).unwrap_or_else(|| "the raiders".into());
        if m.struck == Some(i) {
            let by = m.saviour.map(|s| format!(", {} beside them", self.settlers[s].name)).unwrap_or_default();
            return Some((Job::Wander(self.camp), format!("Falling back to the fire, struck down by {}{}", foe, by)));
        }
        if m.saviour == Some(i) && m.struck.is_some() {
            let who = m.struck.map(|s| self.settlers[s].name.clone()).unwrap_or_default();
            return Some((Job::Wander(self.camp), format!("Dragging {} back to the fire", who)));
        }
        if m.defenders.contains(&i) {
            // At the attacker nearest them: a cell beside it.
            let p = self.settlers[i].pos;
            let foe_at = self.creatures.iter().filter(|c| matches!(c.kind, CreatureKind::Beast | CreatureKind::Raider) && !c.leaving && c.z.is_none())
                .min_by_key(|c| ((c.pos.0 as i32 - p.0 as i32).abs().max((c.pos.1 as i32 - p.1 as i32).abs()), c.id)).map(|c| c.pos).unwrap_or(m.at);
            let side = (i as i32 % 3 - 1, (i as i32 / 3) % 3 - 1);
            let to = self.passable_near_pub((foe_at.0 as i32 + if side == (0, 0) { 1 } else { side.0 }, foe_at.1 as i32 + side.1)).unwrap_or(foe_at);
            return Some((Job::Wander(to), format!("Running to meet {}", foe)));
        }
        // The young and the old awake near it run indoors.
        if self.hides(i, m.at) {
            return Some((Job::Wander(self.sleep_spot(i)), format!("Hiding indoors from {}", foe)));
        }
        None
    }

    /// Each tick of the melee: the attackers turn on whoever is nearest; when its time is up the
    /// band breaks and runs.
    pub(crate) fn melee_tick(&mut self) {
        let Some(m) = self.melee.clone() else { return };
        if self.clock.tick >= m.until {
            let part: Vec<usize> = (0..self.settlers.len()).filter(|&i| self.in_melee(i)).collect();
            self.melee = None;
            let foe = self.arc.as_ref().map(|a| a.threat.clone());
            let side = if self.raid_side.is_empty() { "the dark".to_string() } else { self.raid_side.clone() };
            // ("A war band of X, led by Y, draws off": the leader set off by commas.)
            let foe = foe.map(|mut t| { if t.name.contains(", led by ") { t.name.push(','); } t });
            // A beast slain in the fight falls where it fought.
            let slain = foe.as_ref().map_or(false, |t| self.slain.iter().any(|s| *s == t.name.trim_end_matches(',')));
            if slain {
                self.creatures.retain(|c| !(c.kind == CreatureKind::Beast && foe.as_ref().map_or(false, |t| t.name.trim_end_matches(',') == c.name)));
            } else if let Some(t) = foe.as_ref() {
                let line = match t.kind {
                    super::arc::ThreatKind::Beast | super::arc::ThreatKind::Deep => format!("{} turns from the camp and lopes off toward {}.", super::arc::capital_word(&t.name), side),
                    _ => {
                        let many = t.name.starts_with("raiders") || t.name.starts_with("the ") || t.name.starts_with("outlaws");
                        if m.routed { format!("{} {} and {} off toward {}.", super::arc::capital_word(&t.name), if many { "break" } else { "breaks" }, if many { "run" } else { "runs" }, side) }
                        else { format!("{} {} off toward {}, into the dark.", super::arc::capital_word(&t.name), if many { "draw" } else { "draws" }, side) }
                    }
                };
                self.note(line);
            }
            self.attackers_retreat();
            for i in part { if self.settlers[i].alive && matches!(self.settlers[i].job, Job::Wander(_)) { self.decide(i, true); } }
            return;
        }
        // The attackers turn on the nearest defender within three cells (every few minutes).
        if self.clock.tick % 4 != 0 { return; }
        let settlers: Vec<(usize, Pos)> = m.defenders.iter().filter(|&&i| self.settlers[i].alive).map(|&i| (i, self.settlers[i].pos)).collect();
        for k in 0..self.creatures.len() {
            let c = &self.creatures[k];
            if !matches!(c.kind, CreatureKind::Beast | CreatureKind::Raider) || c.leaving || c.z.is_some() || !c.path.is_empty() { continue; }
            let p = c.pos;
            let Some(&(_, sp)) = settlers.iter().min_by_key(|(i, s)| ((s.0 as i32 - p.0 as i32).abs().max((s.1 as i32 - p.1 as i32).abs()), *i)) else { continue };
            let dist = (sp.0 as i32 - p.0 as i32).abs().max((sp.1 as i32 - p.1 as i32).abs());
            if dist >= 2 && dist <= 4 {
                if let Some(path) = nav::path(&self.map, p, sp, 200) {
                    // A step toward them, stopping beside them.
                    self.creatures[k].path = path.into_iter().skip(1).take((dist - 1) as usize).collect();
                }
            }
        }
    }
}
