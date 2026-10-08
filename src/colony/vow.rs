//! Vows of vengeance: sworn by a grave, kept by a blow.
//!
//! The idea is Dwarf Fortress's personal goals (a dwarf who dreams of revenge) and the
//! roadmap's director threads that "land on settlers" (vows, feuds). Here, when a raid kills a
//! settler (`swear_vengeance`, from `raid_at`), the one who was closest to them (a spouse, a
//! parent or child, else the friend at 8 or more; grown, not a guest) swears by the grave to see
//! the killer dead: a beast by its name, a war band by its leader if it has one, else the band's
//! people (a moment; `Colony::vows`). Until it is kept they drill twice as hard (`drill_done`)
//! and stand first among the defenders against that foe (`fight.rs`). When the killer falls
//! (`vow_kept`, from `maybe_slay`, a leader cut down, or the hunting party), the vow is kept: by
//! their own hand (a moment, a deed, and courage found: bravery +5) or by another's (a line; they
//! are at peace). A vow not kept is in the annals as a deed.

use super::*;
use crate::persona::Facet;

#[derive(Clone, Debug)]
pub struct Vow {
    pub who: usize,
    /// The killer as the camp names it: "Gru the forgotten beast", "Oissoasz the Slayer".
    pub target: String,
    pub for_whom: String,
    pub day: u64,
    pub kept: Option<u64>,
}

impl Colony {
    /// A raid killed `dead`: the closest swears vengeance on the killer.
    pub(crate) fn swear_vengeance(&mut self, dead: usize, threat: &arc::Threat) {
        let target = match threat.kind {
            arc::ThreatKind::Beast | arc::ThreatKind::Deep => threat.name.clone(),
            arc::ThreatKind::Warband => match threat.name.split(", led by ").nth(1) { Some(l) => l.to_string(), None => return },
            _ => return,
        };
        let grown = |c: &Colony, j: usize| c.settlers[j].alive && c.settlers[j].guest_until == 0 && c.settlers[j].past.as_ref().map_or(true, |p| p.age >= 14) && !c.vows.iter().any(|v| v.who == j && v.kept.is_none());
        let kin: Vec<usize> = self.children.iter().filter(|c| c.0 == dead).flat_map(|c| [c.1, c.2])
            .chain(self.children.iter().filter(|c| c.1 == dead || c.2 == dead).map(|c| c.0))
            .chain(self.widowed.iter().filter(|w| w.1 == dead).map(|w| w.0)).collect();
        let Some(who) = kin.into_iter().find(|&j| grown(self, j))
            .or_else(|| (0..self.settlers.len()).filter(|&j| j != dead && grown(self, j) && self.opinion(dead, j) >= 8).max_by_key(|&j| (self.opinion(dead, j), std::cmp::Reverse(j)))) else { return };
        let (wn, dn) = (self.settlers[who].name.clone(), self.settlers[dead].name.clone());
        let line = format!("By the grave of {}, {} swears to see {} dead.", dn, wn, target);
        self.note(line.clone());
        let at = self.settlers[who].pos;
        self.moment(format!("{}'s vow", wn), line, format!("because {} killed {}", target, dn), at);
        self.vows.push(vow::Vow { who, target, for_whom: dn, day: self.clock.day(), kept: None });
    }

    /// Whether settler `i` has sworn to kill `foe` (a raid's threat name or its leader).
    pub(crate) fn sworn_against(&self, i: usize, foe: &str) -> bool {
        self.vows.iter().any(|v| v.who == i && v.kept.is_none() && (foe.contains(&v.target) || v.target.contains(foe)))
    }

    /// The killer is dead: vows on it are kept, by the striker's hand or another's.
    pub(crate) fn vow_kept(&mut self, target: &str, striker: Option<usize>) {
        let day = self.clock.day();
        for k in 0..self.vows.len() {
            let v = self.vows[k].clone();
            if v.kept.is_some() || !(target.contains(&v.target) || v.target.contains(target)) { continue; }
            self.vows[k].kept = Some(day);
            let wn = self.settlers[v.who].name.clone();
            if striker == Some(v.who) {
                let line = format!("{} has kept the vow sworn by the grave of {}: {} is dead by {} hand.", wn, v.for_whom, v.target, if self.settlers[v.who].persona.female { "her" } else { "his" });
                self.note(line.clone());
                let at = self.settlers[v.who].pos;
                self.moment(format!("{} keeps the vow", wn), line, format!("because {} swore it on day {}", wn, v.day), at);
                self.settlers[v.who].deeds.push(format!("kept the vow sworn for {} on day {}", v.for_whom, day));
                let f = self.settlers[v.who].persona.facet(Facet::Bravery);
                self.settlers[v.who].persona.facets[Facet::Bravery as usize] = (f + 5).min(100);
            } else if self.settlers[v.who].alive {
                self.note(format!("{} is dead; {}'s vow for {} is kept by another's hand, and {} is at peace.", v.target, wn, v.for_whom, wn));
            }
            if self.settlers[v.who].alive { self.feel(v.who, mind::Feel::Reconciled { by: format!("the death of {}", v.target) }); }
        }
    }

    /// Settler `i`'s vow, for their page and the annals.
    pub fn vow_of(&self, i: usize) -> Option<String> {
        self.vows.iter().rev().find(|v| v.who == i).map(|v| match v.kept {
            Some(d) => format!("Kept a vow to see {} dead for {} (day {})", v.target, v.for_whom, d),
            None => format!("Sworn to see {} dead for {}", v.target, v.for_whom),
        })
    }
}
