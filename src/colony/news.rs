//! News from the world, as each teller tells it, taken as each listener's past makes them.
//!
//! The idea is Dwarf Fortress's per-entity knowledge (`history::knowledge`): the camp hears only
//! what its tellers know, the way their people tell it. Caravans bring what their town has heard
//! (`trade.rs`), visitors what their home knows (`visitors.rs`), migrants what their people know,
//! and bards sing their people's deeds their people's way. Each item (`hear`) is kept for the
//! annals and moves those it touches: their people's fortune or a town of their past (doubled
//! for the people), and a people or person their past feels for (glad at the ill fortune of
//! those they hate). A listener whose people tells it otherwise will not hear it told so, or (told
//! plainly) adds their people's account.

use super::*;
use crate::history::knowledge::{Slant, Told};
use crate::history::EntityId;

/// An item of news the camp heard.
#[derive(Clone, Debug)]
pub struct Heard {
    pub day: u64,
    /// "the traders of Brolmdustoor", "Ielnveph the Brave".
    pub from: String,
    pub told: Told,
}

/// Why a listener is moved.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tie { Own, Foe, Dear }

impl Colony {
    /// Whether the camp has heard of this event already.
    pub fn has_heard(&self, e: crate::history::EventId) -> bool { self.heard.iter().any(|h| h.told.event == e) }

    /// The first of `items` the camp has not heard.
    pub(crate) fn unheard(&self, items: &[Told]) -> Option<Told> { items.iter().find(|t| !self.has_heard(t.event)).cloned() }

    /// The camp hears `told` from `from` (`teller`: the settler telling it, who is not moved).
    pub(crate) fn hear(&mut self, told: &Told, from: &str, teller: Option<usize>) {
        let day = self.clock.day();
        self.heard.push(Heard { day, from: from.to_string(), told: told.clone() });
        // How it sits with each, by their past.
        let mut moved: Vec<(usize, i32, Tie)> = Vec::new();
        let mut differ: Vec<(usize, String, String)> = Vec::new();
        for j in 0..self.settlers.len() {
            if !self.settlers[j].alive || Some(j) == teller { continue; }
            let Some(p) = self.settlers[j].past.as_ref() else { continue };
            let mut v = 0i32;
            let mut tie = Tie::Own;
            if let Some(f) = p.people { v += 2 * told.stake(&EntityId::Faction(f)) as i32; }
            for t in &p.towns { v += told.stake(&EntityId::Settlement(*t)) as i32; }
            if let Some((what, whom)) = &p.feeling {
                let s = told.stake(whom) as i32;
                if s != 0 {
                    let hate = what.starts_with("hates") || what.starts_with("has not forgiven");
                    let w = if hate { -2 * s } else { 2 * s };
                    if v == 0 { tie = if hate { Tie::Foe } else { Tie::Dear }; }
                    v += w;
                }
            }
            if v != 0 { moved.push((j, v.signum(), tie)); }
            // Their people tell it otherwise.
            if let Some(f) = p.people.filter(|f| Some(*f) != told.teller) {
                if let Some((_, name, g)) = told.others.iter().find(|o| o.0 == f) {
                    if told.gloss.as_deref() != Some(g.as_str()) { differ.push((j, name.clone(), g.clone())); }
                }
            }
        }
        for &(j, s, _) in &moved { self.feel(j, mind::Feel::News { what: format!("of {}", told.headline), good: s > 0 }); }
        let names = |v: &[usize], c: &Colony| -> String {
            let n: Vec<String> = v.iter().map(|&j| c.settlers[j].name.clone()).collect();
            if n.len() > 3 { format!("{} and {} others", n[..2].join(", "), n.len() - 2) } else { crate::persona::list(&n) }
        };
        for (tie, good) in [(Tie::Own, 1), (Tie::Own, -1), (Tie::Foe, 1), (Tie::Foe, -1), (Tie::Dear, 1), (Tie::Dear, -1)] {
            let who: Vec<usize> = moved.iter().filter(|m| m.1 == good && m.2 == tie).map(|m| m.0).collect();
            if who.is_empty() { continue; }
            let w = names(&who, self);
            let one = who.len() == 1;
            let v = |many: &str, single: &str| if one { single.to_string() } else { many.to_string() };
            self.note(match (tie, good > 0) {
                (Tie::Own, true) => format!("{} {} at the news: it is their people's, or their own town's.", w, v("take heart", "takes heart")),
                (Tie::Own, false) => format!("{} {} at the news: it is their people's, or their own town's.", w, v("grieve", "grieves")),
                (Tie::Foe, true) => format!("{} {} glad of the news: it is ill news for those they hate.", w, v("are", "is")),
                (Tie::Foe, false) => format!("{} {} the news ill: it is good news for those they hate.", w, v("take", "takes")),
                (Tie::Dear, true) => format!("{} {} at the news: it touches someone dear to them.", w, v("take heart", "takes heart")),
                (Tie::Dear, false) => format!("{} {} at the news: it touches someone dear to them.", w, v("grieve", "grieves")),
            });
        }
        // Differing accounts: one line per people.
        let mut peoples: Vec<String> = differ.iter().map(|d| d.1.clone()).collect();
        peoples.dedup();
        peoples.sort();
        peoples.dedup();
        for people in peoples {
            let who: Vec<usize> = differ.iter().filter(|d| d.1 == people).map(|d| d.0).collect();
            let g = differ.iter().find(|d| d.1 == people).map(|d| d.2.clone()).unwrap_or_default();
            let w = names(&who, self);
            if told.slant != Slant::Plain {
                self.note(format!("{} will not hear it told so: {} {} it {}.", w, people, if people.ends_with('s') { "call" } else { "calls" }, g));
                for &j in &who { self.feel(j, mind::Feel::News { what: format!("{} {} {} their own way", told.teller_name, crate::history::knowledge::tell_verb(&told.teller_name), told.headline), good: false }); }
            } else {
                self.note(format!("{} {}: {} {} it {}.", w, if who.len() == 1 { "adds" } else { "add" }, people, if people.ends_with('s') { "call" } else { "calls" }, g));
            }
        }
    }

    /// The news line in a sentence: "the Battle of X (247), a great victory... (as the Y tell it)".
    pub(crate) fn news_words(t: &Told) -> String {
        let a = t.as_told();
        if a.is_empty() { t.line() } else { format!("{} ({})", t.line(), a) }
    }
}
