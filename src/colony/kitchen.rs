//! The kitchen: a cook, and the evening meal made of what the camp grows and hunts.
//!
//! The idea is Dwarf Fortress's cooking: raw food is cooked into prepared meals named for their
//! ingredients, and a fine meal is a good thought. Here, once twelve live in the camp and a
//! storehouse stands, the camp raises a kitchen (`ProjectKind::Kitchen`, 3x3 roofed, 8 loads).
//! At dawn (`reckon_cook`) the cook is named: the settler who most loves food (immoderation and
//! their liked food, else the steadiest hand), office "Cooks for the camp". At 16:00, with three
//! meals a head or more stored, the cook makes the evening meal (`cook_supper`): a dish named
//! from what the camp has (`dish`: venison or the pen's meat after a hunt, fish from the jetty or
//! the river, grain from the field, cave mushrooms from the farm under the rock, berries, wine at
//! the still), its quality from the cook's sure hands and patience. Those who eat between 16:00
//! and 22:00 that day have a good thought (`Feel::AteWell`), more if it was fine; a cook who has
//! cooked thirty suppers is named in the annals.

use super::*;
use crate::persona::{Attr, Facet};

impl Colony {
    /// The living cook, if any.
    pub(crate) fn cook(&self) -> Option<usize> {
        (0..self.settlers.len()).find(|&i| self.settlers[i].alive && self.settlers[i].office.as_deref() == Some("Cooks for the camp"))
    }

    fn kitchen(&self) -> bool { self.projects.iter().any(|p| p.done && p.kind == projects::ProjectKind::Kitchen) }

    /// Dawn: a cook for the kitchen.
    pub(crate) fn reckon_cook(&mut self) {
        if !self.kitchen() || self.cook().is_some() { return; }
        let lord = self.ruling_lord();
        let Some(c) = (0..self.settlers.len()).filter(|&i| {
            let s = &self.settlers[i];
            s.alive && !s.mind.left && s.guest_until == 0 && s.office.is_none() && Some(i) != lord && s.past.as_ref().map_or(true, |p| p.age >= 14)
        }).max_by_key(|&i| {
            let p = &self.settlers[i].persona;
            (p.facet(Facet::Immoderation) as i32 + (p.attr(Attr::Patience) / 40.0) as i32, std::cmp::Reverse(i))
        }) else { return };
        self.settlers[c].office = Some("Cooks for the camp".into());
        let name = self.settlers[c].name.clone();
        self.note(format!("{} takes over the kitchen: the camp will eat a cooked supper now.", name));
    }

    /// The dish: what the camp has, cooked together.
    fn dish(&self) -> String {
        let day = self.clock.day();
        let mut parts: Vec<String> = Vec::new();
        if self.hunted > 0 {
            let meat = self.pen.as_ref().map(|p| p.0.trim_end_matches('s').to_string()).unwrap_or_else(|| "venison".into());
            parts.push(if meat == "venison" { meat } else { format!("{} meat", meat) });
        }
        if self.jetty.is_some() || !self.fishing_spots.is_empty() { parts.push("river fish".into()); }
        if self.projects.iter().any(|p| p.done && p.kind == projects::ProjectKind::Field) { parts.push(if self.seed_grain { "good barley".into() } else { "barley".into() }); }
        if self.projects.iter().any(|p| p.done && p.kind == projects::ProjectKind::CaveFarm) { parts.push("cave mushrooms".into()); }
        if !self.hard_winter() { parts.push("berries".into()); }
        if parts.is_empty() { parts.push("what the store holds".into()); }
        // Two of them, turning with the days.
        let k = (day as usize) % parts.len();
        let first = parts[k].clone();
        let second = parts.get((k + 1) % parts.len()).filter(|s| **s != first).cloned();
        let kind = ["a stew of", "a roast of", "a pie of", "a broth of"][(day % 4) as usize];
        match second { Some(s) => format!("{} {} and {}", kind, first, s), None => format!("{} {}", kind, first) }
    }

    /// 16:00: the cook makes supper.
    pub(crate) fn cook_supper(&mut self) {
        let Some(c) = self.cook() else { return };
        if self.settlers[c].ill_until > self.clock.tick || self.food_stored() < 3 * self.alive() as u32 { self.supper = None; return; }
        let p = &self.settlers[c].persona;
        let q = (p.attr(Attr::KinestheticSense) / 2000.0).min(1.0) * 0.5 + (p.attr(Attr::Patience) / 2000.0).min(1.0) * 0.3
            + (crate::history::settlers::hash_pub(self.seed ^ self.clock.day(), 0xC00C) % 100) as f32 / 500.0;
        let fine = q >= 0.6;
        let dish = self.dish();
        self.supper = Some((self.clock.day(), dish.clone(), fine));
        self.suppers += 1;
        if self.suppers == 1 {
            let name = self.settlers[c].name.clone();
            self.note(format!("{} cooks the camp's first supper in the kitchen: {}.", name, dish));
        }
    }

    /// A meal eaten in the evening, the day the cook made supper: a good thought.
    pub(crate) fn ate_supper(&mut self, i: usize) {
        let Some((day, dish, fine)) = self.supper.clone() else { return };
        let h = self.clock.hour();
        if day != self.clock.day() || !(16..22).contains(&h) || self.settlers[i].last_supper == day { return; }
        self.settlers[i].last_supper = day;
        self.feel(i, mind::Feel::AteWell { dish, fine });
    }
}
