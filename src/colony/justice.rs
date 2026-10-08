//! Crime and justice: who steals, who sees, and what the speaker does about it.
//!
//! The idea is Dwarf Fortress's justice: crimes are committed by character, witnessed or not, and
//! punished by whoever holds the office, as their values bend them. Here the greedy and the
//! immoderate may steal meals from the store at night (more when hungry), seen by a settler awake
//! near or guessed by the sharpest of the camp; a tantrum that tears down work is a crime too.
//! At dawn the speaker judges: one who holds the law dear puts the culprit in the stocks for a
//! day; one who does not care much makes them give back what they took; one who despises the
//! law lets it go, and the law-minded resent that. Without a speaker the camp only grumbles.

use super::*;
use crate::persona::{Attr, Facet, Val};

#[derive(Clone, Debug)]
pub struct Crime {
    pub who: usize,
    /// "stole 3 meals from the store", "tore down part of the palisade".
    pub what: String,
    pub day: u64,
    /// Meals to give back if so judged.
    pub meals: u32,
    pub judged: bool,
}

impl Colony {
    /// 23:00: perhaps a theft.
    pub(crate) fn night_theft(&mut self) {
        let day = self.clock.day();
        for i in 0..self.settlers.len() {
            let s = &self.settlers[i];
            if !s.alive || s.mind.broken.is_some() || self.vampire.map_or(false, |(v, _)| v == i) || s.past.as_ref().map_or(false, |p| p.age < 12) { continue; }
            // Only the very greedy or immoderate, a little bolder when hungry: at most about one
            // night in forty, by hash.
            let urge = s.persona.facet(Facet::Greed).max(s.persona.facet(Facet::Immoderation)) as i32 - 85 + if s.hunger > 0.8 { 10 } else { 0 };
            if urge <= 0 { continue; }
            if (crate::history::settlers::hash_pub(self.seed ^ day, 0x7EF7 + i as u64) % 600) as i32 >= urge { continue; }
            let food = self.food_stored();
            if food < 4 { continue; }
            let take = (2 + urge as u32 / 10).min(food / 4).max(1);
            for _ in 0..take { if let Some(k) = self.items.iter().rposition(|it| it.kind == ItemKind::Food && it.stored && !it.reserved) { self.items.remove(k); self.fix_refs_pub(k); } }
            // Seen by someone awake near, else guessed by the sharpest mind of the camp.
            let me = self.settlers[i].pos;
            let seen = (0..self.settlers.len()).find(|&j| j != i && self.settlers[j].alive && self.settlers[j].job != Job::Sleep
                && (self.settlers[j].pos.0 as i32 - me.0 as i32).abs().max((self.settlers[j].pos.1 as i32 - me.1 as i32).abs()) <= 6);
            let sharp = (0..self.settlers.len()).filter(|&j| j != i && self.settlers[j].alive).max_by(|&a, &b| self.settlers[a].persona.attr(Attr::Intuition).total_cmp(&self.settlers[b].persona.attr(Attr::Intuition)).then(b.cmp(&a)));
            let guessed = sharp.filter(|&j| self.settlers[j].persona.attr(Attr::Intuition) > 1200.0);
            let name = self.settlers[i].name.clone();
            match seen.or(guessed) {
                Some(w) => {
                    let wn = self.settlers[w].name.clone();
                    let meals = if take == 1 { "a meal".to_string() } else { format!("{} meals", take) };
                    self.note(format!("{} {} {} taking {} from the store in the night.", wn, if seen.is_some() { "sees" } else { "guesses it was" }, name, meals));
                    self.like(i, w, -3);
                    self.crimes.push(Crime { who: i, what: format!("stole {} from the store", meals), day, meals: take, judged: false });
                }
                None => self.note(format!("{} gone from the store in the morning, and no one saw who took {}.", if take == 1 { "A meal is".to_string() } else { format!("{} meals are", take) }, if take == 1 { "it" } else { "them" })),
            }
        }
    }

    /// A crime of a break (`mind.rs`): wrecking the camp's work.
    pub(crate) fn record_wrecking(&mut self, i: usize, what: &str) {
        let day = self.clock.day();
        self.crimes.push(Crime { who: i, what: format!("tore down part of {}", what), day, meals: 0, judged: false });
    }

    /// Dawn: the speaker judges what is unjudged.
    pub(crate) fn reckon_justice(&mut self) {
        let Some(judge) = self.speaker.filter(|&s| self.settlers[s].alive) else {
            // With no speaker, a thing of the night is driven out with fire all the same.
            if let Some((v, _)) = self.vampire.filter(|&(v, _)| self.crimes.iter().any(|c| !c.judged && c.who == v)) {
                let vn = self.settlers[v].name.clone();
                let line = format!("At dawn the camp drives {} out with torches: no one will sleep near them again.", vn);
                self.note(line.clone());
                let at = self.settlers[v].pos;
                self.moment(format!("{} is driven out", vn), line, format!("because {} fed on the sleepers", vn), at);
                self.release(v);
                let s = &mut self.settlers[v];
                s.alive = false;
                s.mind.left = true;
                self.vampire = None;
            }
            for c in self.crimes.iter_mut().filter(|c| !c.judged) { c.judged = true; }
            return;
        };
        let pending: Vec<usize> = (0..self.crimes.len()).filter(|&k| !self.crimes[k].judged).collect();
        for k in pending {
            self.crimes[k].judged = true;
            let c = self.crimes[k].clone();
            if !self.settlers[c.who].alive || c.who == judge {
                if c.who == judge { self.note(format!("No one judges {}, who speaks for the camp, though {} {}.", self.settlers[judge].name, if self.settlers[judge].persona.female { "she" } else { "he" }, c.what)); }
                continue;
            }
            let (jn, cn) = (self.settlers[judge].name.clone(), self.settlers[c.who].name.clone());
            let law = self.settlers[judge].persona.value(Val::Law);
            // A thing of the night (`night.rs`): death, or the road.
            if self.vampire.map_or(false, |(v, _)| v == c.who) && (c.what.contains("blood") || c.what.contains("thing of the night")) {
                let line = if law > -26 {
                    format!("{} has {} dragged out at dawn and put to death by the fire: {} {}.", jn, cn, if self.settlers[c.who].persona.female { "she" } else { "he" }, c.what)
                } else {
                    format!("{} will not shed blood, and casts {} out into the light: {} {}.", jn, cn, if self.settlers[c.who].persona.female { "she" } else { "he" }, c.what)
                };
                self.note(line.clone());
                let at = self.settlers[c.who].pos;
                self.moment(format!("The end of {}", cn), line, format!("because {} {}, and {} {} the law", cn, c.what, jn, if law > -26 { "holds by" } else { "despises" }), at);
                self.release(c.who);
                if law > -26 { self.bury(c.who, "at dawn, put to death as a thing of the night"); } else { let s = &mut self.settlers[c.who]; s.alive = false; s.mind.left = true; }
                self.vampire = None;
                for k in self.crimes.iter_mut().filter(|x| x.who == c.who) { k.judged = true; }
                continue;
            }
            // A third offence goes to the stocks, unless the judge despises the law.
            let priors = self.crimes.iter().filter(|x| x.who == c.who && x.day < c.day).count();
            // A sixth offence: cast out, unless the judge is too close to them (three days in
            // the stocks instead).
            let close = self.settlers[judge].spouse == Some(c.who) || self.opinion(judge, c.who) >= 20;
            let line = if priors >= 5 && law > -26 && !close {
                let line = format!("{} has stolen from the camp {} times; {} casts {} out.", cn, priors + 1, jn, if self.settlers[c.who].persona.female { "her" } else { "him" });
                let at = self.settlers[c.who].pos;
                self.moment(format!("{} is cast out", cn), line.clone(), format!("because {} would not stop", cn), at);
                self.release(c.who);
                let s = &mut self.settlers[c.who];
                s.alive = false;
                s.mind.left = true;
                for j in 0..self.settlers.len() {
                    if j != c.who && self.settlers[j].alive && self.opinion(c.who, j) >= 12 { self.feel(j, mind::Feel::Death { whom: format!("{} cast out", cn), close: true }); }
                }
                line
            } else if priors >= 5 && law > -26 {
                self.stocks = Some((c.who, self.clock.tick + 3 * TICKS_PER_DAY));
                self.feel(c.who, mind::Feel::Punished { by: jn.clone() });
                format!("{} puts {} in the stocks for three days: {} {}, again.", jn, cn, if self.settlers[c.who].persona.female { "she" } else { "he" }, c.what)
            } else if law >= 26 || (priors >= 2 && law > -26) {
                self.stocks = Some((c.who, self.clock.tick + TICKS_PER_DAY));
                self.like(judge, c.who, -4);
                self.feel(c.who, mind::Feel::Punished { by: jn.clone() });
                format!("{} puts {} in the stocks for a day: {} {}.", jn, cn, if self.settlers[c.who].persona.female { "she" } else { "he" }, c.what)
            } else if law > -26 {
                for _ in 0..c.meals { self.items.push(Item { kind: ItemKind::Food, at: self.camp, stored: true, reserved: false }); }
                self.like(judge, c.who, -1);
                if c.meals > 0 { format!("{} makes {} give back the {} {} {} took.", jn, cn, c.meals, if c.meals == 1 { "meal" } else { "meals" }, if self.settlers[c.who].persona.female { "she" } else { "he" }) }
                else { format!("{} makes {} mend what {} broke.", jn, cn, if self.settlers[c.who].persona.female { "she" } else { "he" }) }
            } else {
                // Letting it go: the law-minded resent the speaker for it.
                for j in 0..self.settlers.len() {
                    if j != judge && j != c.who && self.settlers[j].alive && self.settlers[j].persona.value(Val::Law) >= 26 { self.like(judge, j, -2); }
                }
                format!("{} lets it go that {} {}, and those who hold the law dear mutter.", jn, cn, c.what)
            };
            self.note(line.clone());
            if self.milestones.insert("first judgement") {
                let at = self.settlers[judge].pos;
                self.moment("Judgement".into(), line, format!("because {} {}, and {} {} the law", cn, c.what, jn, if law >= 26 { "holds by" } else if law > -26 { "cares little for" } else { "despises" }), at);
            }
        }
    }

    /// One in the stocks stays at the fire, doing nothing, until the day is out.
    pub(crate) fn stocks_choice(&mut self, i: usize) -> Option<(Job, String)> {
        let (who, until) = self.stocks?;
        if who != i { return None; }
        if self.clock.tick >= until { self.stocks = None; return None; }
        if self.settlers[i].hunger >= 0.8 { return None; }
        Some((Job::Wander(self.camp), "In the stocks by the fire for a day".into()))
    }
}
