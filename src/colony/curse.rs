//! Under the full moon: the werebeast and its curse.
//!
//! The idea is Dwarf Fortress's night creatures, the werebeasts above all: a shapeshifting beast
//! of the history (a legendary creature with the power to change shape, laired within eight
//! tiles: `found_colony` records it) hunts on full-moon nights (every 28th day, the 14th of each
//! cycle), and its bite curses. A cursed settler changes on the next full moons: they slip away
//! from the camp at dusk and something with their eyes hunts a settler alone, then they come back
//! at dawn, scratched and remembering nothing. Their bite curses too. When one of them has bitten
//! twice, the camp casts them out (its speaker if it has one).

use super::*;
use super::creatures::{Creature, CreatureKind};

/// The marks in a hunter's name that carry the curse.
pub const MOON: &str = " under the full moon";
pub const CHANGED: &str = ", changed";

impl Colony {
    pub fn full_moon(&self) -> bool { self.clock.day() % 28 == 14 }

    /// 21:00 on a full moon: the werebeast comes, and the cursed change.
    pub(crate) fn moon_rises(&mut self) {
        if !self.full_moon() { return; }
        let mut said = false;
        if let Some(beast) = self.were.clone() {
            let a = (self.seed.wrapping_add(self.clock.day()) % 628) as f32 / 100.0;
            let p = ((self.camp.0 as f32 + a.cos() * 30.0) as i32, (self.camp.1 as f32 + a.sin() * 30.0) as i32);
            if let Some(at) = self.passable_near_pub(p) {
                let id = self.new_creature_id();
                self.creatures.push(Creature { kind: CreatureKind::Wolf, name: format!("{}{}", beast, MOON), pos: at, path: Vec::new(), stride: 0, leaving: false, size: 1.4, home: at, spawned: self.clock.tick, id, z: None, path3: Vec::new(), home_z: 0, out: false, rest_until: 0 });
                self.note(format!("The full moon rises, and something howls in the hills: {} is abroad.", beast));
                said = true;
            }
        }
        self.changed.clear();
        for i in self.cursed.clone() {
            if !self.settlers[i].alive { continue; }
            self.changed.push(i);
            let name = self.settlers[i].name.clone();
            let at = self.settlers[i].pos;
            self.release(i);
            // Their body goes out into the dark; something with their eyes hunts.
            let a = (i as f32 * 1.7 + self.clock.day() as f32) % 6.28;
            let away = ((self.camp.0 as f32 + a.cos() * 45.0) as i32, (self.camp.1 as f32 + a.sin() * 45.0) as i32);
            if let Some(far) = self.passable_near_pub(away) { self.start(i, Job::Wander(far), "Gone out into the night under the full moon".into()); }
            let id = self.new_creature_id();
            self.creatures.push(Creature { kind: CreatureKind::Wolf, name: format!("{}{}", name, CHANGED), pos: at, path: Vec::new(), stride: 0, leaving: false, size: 1.0, home: at, spawned: self.clock.tick, id, z: None, path3: Vec::new(), home_z: 0, out: false, rest_until: 0 });
            if !said { self.note("The full moon rises.".into()); said = true; }
        }
    }

    /// A bite by a cursed hunter curses the bitten; the changed one's bites are counted.
    pub(crate) fn cursed_bite(&mut self, hunter: &str, victim: usize) {
        let from_moon = hunter.ends_with(MOON);
        let changed = hunter.strip_suffix(CHANGED).map(String::from);
        if !from_moon && changed.is_none() { return; }
        if !self.cursed.contains(&victim) {
            self.cursed.push(victim);
            let name = self.settlers[victim].name.clone();
            self.note(format!("{}'s bite heals strangely fast.", name));
        }
        if let Some(who) = changed {
            if let Some(k) = self.settlers.iter().position(|s| s.name == who) {
                let n = self.were_bites.entry(k).or_insert(0);
                *n += 1;
                let vname = self.settlers[victim].name.clone();
                self.note(format!("{} swears the thing that bit them had {}'s eyes.", vname, who));
                self.feel(victim, mind::Feel::TheDeep { what: format!("{}'s eyes in a beast's face", who) });
            }
        }
    }

    /// 06:00 after a full moon: the changed come back; one who has bitten twice is cast out.
    pub(crate) fn moon_sets(&mut self) {
        if self.clock.day() % 28 != 15 { return; }
        // Only those who changed last night (not the newly bitten).
        for i in std::mem::take(&mut self.changed) {
            if !self.settlers[i].alive { continue; }
            let name = self.settlers[i].name.clone();
            self.note(format!("{} comes back at dawn, scratched and silent, and remembers nothing of the night.", name));
            if self.were_bites.get(&i).copied().unwrap_or(0) >= 2 {
                let judge = self.speaker.filter(|&s| s != i && self.settlers[s].alive).map(|s| self.settlers[s].name.clone());
                let line = match &judge {
                    Some(j) => format!("{}, who speaks for the camp, casts {} out: twice now the beast under the moon had {}'s eyes.", j, name, name),
                    None => format!("The camp casts {} out: twice now the beast under the moon had {}'s eyes.", name, name),
                };
                self.note(line.clone());
                let at = self.settlers[i].pos;
                self.moment(format!("{} is cast out", name), line, format!("because {} was bitten under the full moon and carries the curse", name), at);
                self.release(i);
                let s = &mut self.settlers[i];
                s.mind.left = true;
                s.alive = false;
                self.cursed.retain(|&c| c != i);
                for j in 0..self.settlers.len() {
                    if j != i && self.settlers[j].alive && self.opinion(i, j) >= 6 { self.feel(j, mind::Feel::Death { whom: format!("{} cast out", name), close: true }); }
                }
            }
        }
    }
}
