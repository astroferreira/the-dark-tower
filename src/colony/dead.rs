//! Where the Shadow lies on the land, the dead do not rest.
//!
//! The idea is Dwarf Fortress's evil regions, where the dead rise; here the evil is this world's
//! own, the Shadow's corruption (`history/shadow.rs`), read for the embark's tile at founding.
//! On a dark night under its reach one of the camp's graves may give up its dead (the named dead
//! of a real battle that `local/site.rs` buried here, or a settler the camp buried itself), who
//! hunts a settler alone and far from the fire as wolves do, and goes back into the earth at
//! dawn. Graves on ground the patron blessed stay quiet.

use super::*;
use super::creatures::{Creature, CreatureKind};

impl Colony {
    /// At 21:00: perhaps the dead rise (darkness 0.2+, one night in about 1 / (darkness x 0.3)).
    pub(crate) fn dead_rise(&mut self) {
        if self.darkness < 0.2 || self.creatures.iter().any(|c| c.kind == CreatureKind::Wolf) { return; }
        // Consecrated graves hold, unless the dark here is deep (`priest.rs`).
        if self.consecrated && self.darkness < 0.6 { return; }
        let day = self.clock.day();
        if (crate::history::settlers::hash_pub(self.seed ^ day, 0xDEAD) % 1000) as f32 >= self.darkness * 300.0 { return; }
        // The camp's own dead rest three days first (the history's graves have rested long).
        let graves: Vec<usize> = (0..self.marks.len()).filter(|&k| self.marks[k].kind == MarkKind::Grave && !self.marked_at(self.marks[k].at, false) && !self.burned.contains(&self.marks[k].at) && (self.marks[k].day == 0 || day >= self.marks[k].day + 3)).collect();
        if graves.is_empty() { return; }
        // The named first.
        let k = graves.iter().copied().find(|&k| !self.marks[k].title.starts_with("An old")).filter(|_| day % 2 == 0)
            .unwrap_or(graves[(crate::history::settlers::hash_pub(self.seed, day) % graves.len() as u64) as usize]);
        let who = self.marks[k].title.trim_start_matches("The grave of ").to_string();
        let name = if who.starts_with("An old") { "the restless dead".to_string() } else { format!("{}, risen", who) };
        let at = self.marks[k].at;
        let Some(p) = self.passable_near_pub((at.0 as i32, at.1 as i32)) else { return };
        let id = self.new_creature_id();
        self.creatures.push(Creature { kind: CreatureKind::Wolf, name: name.clone(), pos: p, path: Vec::new(), stride: 0, leaving: false, size: 1.0, home: p, spawned: self.clock.tick, id });
        *self.risings.entry(at).or_insert(0) += 1;
        let shadow = self.shadow_name.clone().unwrap_or_else(|| "the Shadow".into());
        let line = format!("Under {}'s darkness, {} rises from the grave at {},{}.", shadow, if who.starts_with("An old") { "something".to_string() } else { who.clone() }, at.0, at.1);
        self.note(line.clone());
        if !self.milestones_hit.iter().any(|m| m == "the dead walk") {
            self.milestones_hit.push("the dead walk".into());
            self.moment("The dead walk".into(), line, format!("because {} lies on this land ({:.2} of its darkness), and the dead here were never blessed", shadow, self.darkness), at);
            for j in 0..self.settlers.len() { if self.settlers[j].alive { self.feel(j, mind::Feel::TheDeep { what: "the dead walk".into() }); } }
        }
    }
}
