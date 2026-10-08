//! The temple's priest: who tends it, and what the rites do for the dead.
//!
//! The idea is Dwarf Fortress's temples and their priests (a religion's position held in a
//! fortress), and its dead who rest only when they are properly laid down. Here, once a temple
//! stands (`society.rs`), at dawn (`reckon_priest`) the most pious grown settler (piety 60+, not
//! a guest, not the lord) becomes its priest (office "Keeps the temple of X", a moment the first
//! time; chosen again when the priest dies or leaves). The priest consecrates the camp's graves
//! the next dawn (`Colony::consecrated`): the dead in them do not rise (`dead.rs`) unless the
//! Shadow's darkness here is 0.6 or deeper. When someone dies by violence with a priest living,
//! the priest says the rites over them (`rites`, from `bury` through `mourn`): one time in two the
//! dead are laid to rest by it and need no slab (`ghosts.rs`).

use super::*;
use crate::persona::Facet;

impl Colony {
    /// The living priest, if any.
    pub(crate) fn priest(&self) -> Option<usize> {
        (0..self.settlers.len()).find(|&i| self.settlers[i].alive && self.settlers[i].office.as_deref().map_or(false, |o| o.starts_with("Keeps the temple")))
    }

    /// Dawn: a priest for the temple; the graves consecrated.
    pub(crate) fn reckon_priest(&mut self) {
        let Some((at, god)) = self.temple() else { return };
        let lord = self.ruling_lord();
        let priest = match self.priest() {
            Some(p) => p,
            None => {
                let Some(p) = (0..self.settlers.len()).filter(|&i| {
                    let s = &self.settlers[i];
                    s.alive && !s.mind.left && s.guest_until == 0 && s.office.is_none() && Some(i) != lord && s.past.as_ref().map_or(true, |x| x.age >= 16)
                        && s.persona.facet(Facet::Piety) >= 60
                }).max_by_key(|&i| (self.settlers[i].persona.facet(Facet::Piety), std::cmp::Reverse(i))) else { return };
                self.settlers[p].office = Some(format!("Keeps the temple of {}", god));
                let name = self.settlers[p].name.clone();
                let line = format!("{} takes up the keeping of the temple of {}.", name, god);
                self.note(line.clone());
                if self.milestones.insert("first priest") {
                    self.moment(format!("{} keeps the temple", name), line, format!("because none in the camp prays to {} more often", god), at);
                }
                return;
            }
        };
        if !self.consecrated && self.marks.iter().any(|m| m.kind == MarkKind::Grave) {
            self.consecrated = true;
            let name = self.settlers[priest].name.clone();
            self.note(format!("{} walks among the graves at the camp's edge and consecrates them to {}.", name, god));
        }
    }

    /// A violent death with a priest living: the rites, and one time in two the dead rest.
    pub(crate) fn rites(&mut self, i: usize) -> bool {
        let Some(p) = self.priest().filter(|&p| p != i) else { return false };
        let god = self.temple().map(|t| t.1).unwrap_or_else(|| "their god".into());
        let (pn, dn) = (self.settlers[p].name.clone(), self.settlers[i].name.clone());
        let rest = crate::history::settlers::hash_pub(self.seed ^ self.clock.tick, 0x9417 + i as u64) % 2 == 0;
        let god = if god.contains(", ") { format!("{},", god) } else { god };
        self.note(format!("{} says the rites of {} over {}{}.", pn, god, dn, if rest { ", and the camp feels the dead lie easy" } else { "" }));
        rest
    }
}
