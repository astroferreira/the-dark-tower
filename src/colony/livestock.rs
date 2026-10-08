//! A pen of beasts: the camp keeps animals of a nearby herd, they breed, and in hard times one
//! is slaughtered.
//!
//! The idea is Dwarf Fortress's livestock: tame animals pastured, breeding, butchered for meat,
//! bone and hide. Here, from day 40 with ten in the camp and a herd grazing within 30 cells, the
//! camp builds a pen (`ProjectKind::Pen`, 6x5 posts); when it stands, two of the nearest herd are
//! driven into it (taken from the land; `Colony::pen` holds the kind and the head). At each turn
//! of the season the pen grows by half (one at least, from two head; twelve at most). At dawn,
//! when the store holds under half of what the camp wants and there are three or more head, one
//! is slaughtered: eight meals laid by the pen, and it counts as a hunt (bone and hide for those
//! who want them); those fond of the creature take it hard.

use super::*;
use super::creatures::CreatureKind;

impl Colony {
    /// Where the pen stands.
    pub fn pen_at(&self) -> Option<Pos> {
        self.projects.iter().find(|p| p.done && p.kind == projects::ProjectKind::Pen).map(|p| (p.at.0 + 3, p.at.1 + 2))
    }

    /// The herd grazing nearest the camp within 30 cells that has two head or more: its name.
    pub(crate) fn herd_near(&self) -> Option<String> {
        let camp = self.camp;
        let head = |name: &str| self.creatures.iter().filter(|c| c.kind == CreatureKind::Game && c.name == name).count();
        self.creatures.iter().filter(|c| c.kind == CreatureKind::Game && head(&c.name) >= 2)
            .map(|c| (c, (c.pos.0 as i32 - camp.0 as i32).abs().max((c.pos.1 as i32 - camp.1 as i32).abs())))
            .filter(|(_, d)| *d <= 30).min_by_key(|(c, d)| (*d, c.id)).map(|(c, _)| c.name.clone())
    }

    /// The pen stands: two of the herd are driven in.
    pub(crate) fn fill_pen(&mut self) {
        let Some(kind) = self.herd_near() else { return };
        let mut taken = 0;
        while taken < 2 {
            let Some(k) = self.creatures.iter().position(|c| c.kind == CreatureKind::Game && c.name == kind) else { break };
            self.creatures.remove(k);
            taken += 1;
        }
        if taken == 0 { return; }
        self.pen = Some((kind.clone(), taken));
        let line = format!("They drive {} {} into the new pen.", taken, kind);
        self.note(line);
    }

    /// The turn of a season: the pen grows.
    pub(crate) fn pen_breeds(&mut self) {
        let Some((kind, n)) = self.pen.clone() else { return };
        if n < 2 { return; }
        let born = (n / 2).max(1);
        let now = (n + born).min(12);
        if now > n {
            self.pen = Some((kind.clone(), now));
            self.note(format!("{} young {} in the pen this season; {} head now.", now - n, kind, now));
        }
    }

    /// Dawn: in want, one is slaughtered.
    pub(crate) fn pen_slaughter(&mut self) {
        let Some((kind, n)) = self.pen.clone() else { return };
        // One a week at most: the herd is not eaten in a few days (seed 23 had slaughtered five
        // dawns running).
        // (But any day when someone is starving.)
        let starving = self.settlers.iter().any(|s| s.alive && s.starving > 0);
        if n < 3 || self.food_stored() * 2 >= self.food_goal() || (!starving && self.slaughter_day > 0 && self.clock.day() < self.slaughter_day + 7) { return; }
        self.slaughter_day = self.clock.day();
        let at = self.pen_at().unwrap_or(self.camp);
        for _ in 0..8 { self.items.push(Item { kind: ItemKind::Food, at, stored: false, reserved: false }); }
        self.pen = Some((kind.clone(), n - 1));
        self.hunted += 1;
        self.note(format!("With the store low they slaughter one of the {} in the pen: eight meals, and the hide and bone.", kind));
        for j in 0..self.settlers.len() {
            if self.settlers[j].alive && fond_of(&self.settlers[j].persona, &kind) { self.feel(j, mind::Feel::KilledLiked { what: kind.trim_end_matches('s').to_string() }); }
        }
    }
}
