//! Drink: a still, berry wine, and the thirst of those who are used to it.
//!
//! The idea is Dwarf Fortress's drink: dwarves want alcohol, a still turns plants into it, sober
//! dwarves grumble and slow, and a feast is merrier with a barrel open. Here, once a workshop
//! stands and the store holds three meals a head, the camp raises a still (`ProjectKind::Still`,
//! 2x2 posts; dwarves sooner). Then one hand at a time who likes a drink (immoderation 50+, or
//! any dwarf) brews by day while drink is short (under two cups a head) and food is not
//! (`Job::Craft` "Brewing ..."): three stored meals become five cups of berry wine
//! (`Colony::drink`). A settler eating takes a cup with the meal when there is one (once a day;
//! `Feel::Drank`, more for the immoderate). At dawn a dwarf with no cup for three days is thirsty
//! (`Feel::Thirsty`), and works a little slower until they drink. A festival with a cup for
//! everyone opens the wine.

use super::*;
use crate::persona::Facet;

impl Colony {
    /// Whether the still stands.
    pub fn still(&self) -> Option<Pos> {
        // Just below its posts, or the nearest open ground beside them (something may have been
        // raised or dug where the brewer stood).
        let p = self.projects.iter().find(|p| p.done && p.kind == projects::ProjectKind::Still)?;
        [(0i32, 2i32), (1, 2), (-1, 0), (2, 0), (0, -1), (1, -1), (-1, 1), (2, 1)].iter()
            .map(|&(dx, dy)| ((p.at.0 as i32 + dx).max(0) as u16, (p.at.1 as i32 + dy).max(0) as u16))
            .find(|&q| nav::passable(&self.map, q))
    }

    fn likes_drink(&self, i: usize) -> bool {
        let p = &self.settlers[i].persona;
        p.race == "dwarf" || p.facet(Facet::Immoderation) >= 50
    }

    /// The brewer's option: by day, while drink is short and food is not.
    pub(crate) fn brew_option(&self, i: usize) -> Option<(f32, Job, String)> {
        if self.clock.is_night() || self.still().is_none() || !self.likes_drink(i) { return None; }
        let alive = self.alive() as u32;
        if self.drink >= 2 * alive || self.food_stored() < 4 * alive.max(1) { return None; }
        if self.settlers.iter().enumerate().any(|(j, s)| j != i && s.alive && s.job == Job::Craft && s.why.starts_with("Brewing")) { return None; }
        Some((0.6, Job::Craft, format!("Brewing berries into wine at the still ({} cups for {} mouths)", self.drink, alive)))
    }

    /// Three meals in, five cups out.
    pub(crate) fn finish_brew(&mut self, i: usize) {
        let mut used = 0;
        for _ in 0..3 {
            if let Some(k) = self.items.iter().rposition(|it| it.kind == ItemKind::Food && it.stored && !it.reserved) { self.items.remove(k); self.fix_refs_pub(k); used += 1; }
        }
        if used < 3 { return; }
        self.drink += 5;
        let name = self.settlers[i].name.clone();
        if self.milestones.insert("first wine") {
            let line = format!("{} draws the first berry wine from the still.", name);
            self.note(line.clone());
            let at = self.still().unwrap_or(self.camp);
            self.moment("The first wine".into(), line, "because the camp had berries to spare and a still to turn them".into(), at);
        }
    }

    /// With a meal: a cup, if there is one and they have not had one today.
    pub(crate) fn drink_with_meal(&mut self, i: usize) {
        let day = self.clock.day();
        if self.drink == 0 || self.settlers[i].last_drink == day { return; }
        self.drink -= 1;
        self.settlers[i].last_drink = day;
        self.feel(i, mind::Feel::Drank);
    }

    /// Dawn: dwarves too long without a cup are thirsty.
    pub(crate) fn reckon_thirst(&mut self) {
        let day = self.clock.day();
        if self.still().is_none() && day < 30 { return; }
        for i in 0..self.settlers.len() {
            let s = &self.settlers[i];
            if !s.alive || s.persona.race != "dwarf" || s.past.as_ref().map_or(false, |p| p.age < 12) { continue; }
            if s.last_drink + 3 < day { self.feel(i, mind::Feel::Thirsty); }
        }
    }

    /// Work pace without drink: a thirsty dwarf is a little slower.
    pub(crate) fn thirst_pace(&self, i: usize) -> f32 {
        let s = &self.settlers[i];
        if s.persona.race == "dwarf" && self.clock.day() >= 30 && s.last_drink + 3 < self.clock.day() { 1.08 } else { 1.0 }
    }
}
