//! Healing: wounds that fester, and the one who tends them.
//!
//! The idea is Dwarf Fortress's health care: wounds left untended get infected, a diagnostician
//! and doctors tend the wounded, and care speeds healing. Here a gashed or broken wound
//! (severity 2+) that no one tended yesterday may fester at dawn (8% a day per severity, by
//! hash): a fever (ill), and after four days of fever untended, one chance in three of death
//! ("of the fever from the wound"). Whenever someone is wounded the camp looks to its healer:
//! the adult with the most empathy and focus (`Settler` with `office` "Tends the wounded" once
//! named; a moment the first time). By day the healer goes to each wounded settler not yet
//! tended today (`Job::Wander` "Tending ..."), and at the bedside (`tend`) the wound is cleaned
//! and bound: it heals a quarter sooner, a festering wound is cleaned (the fever breaks in a
//! day), and the wounded are grateful.

use super::*;
use crate::persona::Attr;

impl Colony {
    /// Who tends the wounded: the most empathic and focused adult, not a guest.
    fn choose_healer(&self) -> Option<usize> {
        (0..self.settlers.len()).filter(|&i| self.settlers[i].alive && self.settlers[i].guest_until == 0 && self.settlers[i].past.as_ref().map_or(true, |p| p.age >= 16))
            .max_by(|&a, &b| {
                let sc = |i: usize| self.settlers[i].persona.attr(Attr::Empathy) + self.settlers[i].persona.attr(Attr::Focus);
                sc(a).total_cmp(&sc(b)).then(b.cmp(&a))
            })
    }

    /// Dawn: festering, fevers, and the healer named when someone is hurt.
    pub(crate) fn reckon_wounds(&mut self) {
        let day = self.clock.day();
        let now = self.clock.tick;
        // Someone hurt: the camp looks to its healer first.
        let anyone = (0..self.settlers.len()).any(|i| self.settlers[i].alive && self.open_wounds(i).next().is_some());
        if anyone && self.healer.map_or(true, |h| !self.settlers[h].alive) {
            if let Some(h) = self.choose_healer() {
                let first = self.healer.is_none();
                self.healer = Some(h);
                if self.settlers[h].office.is_none() { self.settlers[h].office = Some("Tends the wounded".into()); }
                let name = self.settlers[h].name.clone();
                let line = format!("{} takes it on to tend the wounded.", name);
                self.note(line.clone());
                if first {
                    let at = self.settlers[h].pos;
                    self.moment(format!("{} tends the wounded", name), line, format!("because {} has the gentlest hands and the steadiest mind in the camp", name), at);
                }
            }
        }
        for i in 0..self.settlers.len() {
            if !self.settlers[i].alive { continue; }
            for w in 0..self.settlers[i].wounds.len() {
                let wd = &self.settlers[i].wounds[w];
                if wd.healed_at <= now { continue; }
                let untended = wd.tended + 1 < day;
                if wd.severity >= 2 && !wd.infected && untended && crate::history::settlers::hash_pub(self.seed ^ day, 0x1F3C + (i * 8 + w) as u64) % 100 < 8 * wd.severity as u64 {
                    let word = wd.word();
                    self.settlers[i].wounds[w].infected = true;
                    self.settlers[i].wounds[w].fever_since = day;
                    let name = self.settlers[i].name.clone();
                    self.note(format!("{}'s {} festers, untended; {} burns with fever.", name, word.trim_start_matches("a ").trim_start_matches("an "), if self.settlers[i].persona.female { "she" } else { "he" }));
                    self.settlers[i].ill_until = self.settlers[i].ill_until.max(now + TICKS_PER_DAY);
                    self.feel(i, mind::Feel::Ill);
                } else if self.settlers[i].wounds[w].infected {
                    self.settlers[i].ill_until = self.settlers[i].ill_until.max(now + TICKS_PER_DAY);
                    let since = self.settlers[i].wounds[w].fever_since;
                    if untended && day >= since + 4 && crate::history::settlers::hash_pub(self.seed ^ day, 0xFE7E + i as u64) % 3 == 0 {
                        let name = self.settlers[i].name.clone();
                        let word = self.settlers[i].wounds[w].word();
                        let line = format!("{} dies of the fever from {} {} that no one tended.", name, if self.settlers[i].persona.female { "her" } else { "his" }, word.trim_start_matches("a ").trim_start_matches("an "));
                        self.note(line.clone());
                        let at = self.settlers[i].pos;
                        self.moment(format!("The death of {}", name), line, format!("because the wound festered and the camp {}", if self.healer.is_some() { "had no time to tend it" } else { "had no one to tend it" }), at);
                        self.bury(i, "of a festering wound");
                        break;
                    }
                }
            }
        }
    }

    /// The healer's rounds: the nearest wounded settler not tended today.
    pub(crate) fn tend_option(&self, i: usize) -> Option<(f32, Job, String)> {
        if self.healer != Some(i) || self.clock.is_night() { return None; }
        let day = self.clock.day();
        let me = self.settlers[i].pos;
        let (p, w) = (0..self.settlers.len()).filter(|&j| j != i && self.settlers[j].alive)
            .filter_map(|j| self.open_wounds(j).filter(|w| w.tended < day).max_by_key(|w| (w.infected, w.severity)).map(|w| (j, w.clone())))
            .min_by_key(|(j, w)| (!w.infected, (self.settlers[*j].pos.0 as i32 - me.0 as i32).abs().max((self.settlers[*j].pos.1 as i32 - me.1 as i32).abs())))?;
        let to = self.settlers[p].pos;
        Some((if w.infected { 1.6 } else { 1.1 }, Job::Wander(to), format!("Tending {}'s {}", self.settlers[p].name, w.word().trim_start_matches("a ").trim_start_matches("an "))))
    }

    /// At the bedside: clean and bind the worst wound of whoever is within reach.
    pub(crate) fn tend(&mut self, i: usize) {
        let day = self.clock.day();
        let me = self.settlers[i].pos;
        let Some(p) = (0..self.settlers.len()).filter(|&j| j != i && self.settlers[j].alive
            && (self.settlers[j].pos.0 as i32 - me.0 as i32).abs().max((self.settlers[j].pos.1 as i32 - me.1 as i32).abs()) <= 3
            && self.open_wounds(j).any(|w| w.tended < day)).next() else { return };
        let now = self.clock.tick;
        let skill = (self.settlers[i].persona.attr(Attr::Empathy) + self.settlers[i].persona.attr(Attr::Focus)) / 2000.0;
        let mut cured = false;
        // Herbs from the caravan: a third sooner again (`liaison.rs`).
        let herbs = self.herbs > 0;
        if herbs { self.herbs -= 1; }
        for w in self.settlers[p].wounds.iter_mut().filter(|w| w.healed_at > now && w.tended < day) {
            w.tended = day;
            // Bound: a quarter sooner (more for sure hands).
            let left = w.healed_at - now;
            w.healed_at = now + (left as f32 * (0.8 - 0.1 * skill.min(1.5)) * if herbs { 0.67 } else { 1.0 }).max(60.0) as u64;
            if w.infected { w.infected = false; cured = true; }
        }
        let (hn, pn) = (self.settlers[i].name.clone(), self.settlers[p].name.clone());
        if cured { self.note(format!("{} cleans the festering wound of {}; the fever will break.", hn, pn)); }
        else if self.milestones.insert("first tending") { self.note(format!("{} binds {}'s wounds with boiled cloth and moss.", hn, pn)); }
        self.like(p, i, 2);
        self.feel(p, mind::Feel::SavedBy { whom: format!("{}'s care", hn) });
    }
}
