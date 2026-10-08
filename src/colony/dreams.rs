//! Dreams: what each settler most wants from life, and the day they have it.
//!
//! The idea is Dwarf Fortress's life goals: each dwarf dreams of something from their values
//! (raising a family, crafting a masterwork, becoming a legendary warrior...), and realizing it is
//! one of the best things that can happen to them. Here a settler's dream comes from their
//! dearest value that has one (`life_dream`): family or romance, a child of their own; craftsmanship
//! or artwork, a masterwork; martial prowess, a beast or a war leader slain by their hand;
//! knowledge, a book of their writing; power, to speak for the camp; nature or curiosity, to find
//! what lies in the hills; peace, to see a war set aside. At dawn (`reckon_dreams`) the camp's
//! days are read for it: a child born to them, a work of quality 5 or an artifact by their hand,
//! a deed of slaying, a book, the office of speaker or lord, a place they found, a peace made
//! while they lived here. Realized, it is a moment, a deep contentment (`Feel::Dreamt`, stress
//! eased to the floor), and cheer grows (+5); the settler page and annals say it.

use super::*;
use crate::persona::{Facet, Val};

/// What a settler can dream of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifeDream { Child, Masterwork, Slay, Book, Rule, Discover, Peace }

impl LifeDream {
    pub fn word(self) -> &'static str {
        match self {
            LifeDream::Child => "raising a child of their own",
            LifeDream::Masterwork => "making a masterwork",
            LifeDream::Slay => "slaying a great foe with their own hand",
            LifeDream::Book => "writing a book that will outlast them",
            LifeDream::Rule => "speaking for the camp",
            LifeDream::Discover => "finding what lies hidden in the hills",
            LifeDream::Peace => "seeing a war set aside",
        }
    }
}

impl Colony {
    /// Settler `i`'s dream, from their dearest value that has one.
    pub fn life_dream(&self, i: usize) -> Option<LifeDream> {
        let s = &self.settlers[i];
        if s.guest_until > 0 || s.past.as_ref().map_or(false, |p| p.age < 14) { return None; }
        let p = &s.persona;
        let mut c: Vec<(i32, LifeDream)> = vec![
            (p.value(Val::Family).max(p.value(Val::Romance)) as i32, LifeDream::Child),
            (p.value(Val::Craftsmanship).max(p.value(Val::Artwork)) as i32, LifeDream::Masterwork),
            (p.value(Val::MartialProwess) as i32, LifeDream::Slay),
            (p.value(Val::Knowledge) as i32, LifeDream::Book),
            (p.value(Val::Power) as i32, LifeDream::Rule),
            ((p.value(Val::Nature) as i32).max((p.facet(Facet::Curiosity) as i32 - 50) / 2), LifeDream::Discover),
            (p.value(Val::Peace) as i32, LifeDream::Peace),
        ];
        c.sort_by_key(|&(v, d)| (std::cmp::Reverse(v), d as u8));
        c.first().filter(|(v, _)| *v >= 15).map(|&(_, d)| d)
    }

    /// Whether settler `i`'s dream has come true in the camp.
    fn dream_met(&self, i: usize, d: LifeDream) -> bool {
        let s = &self.settlers[i];
        match d {
            LifeDream::Child => self.children.iter().any(|c| c.1 == i || c.2 == i),
            LifeDream::Masterwork => self.works.iter().any(|w| w.maker == i && w.quality >= 5),
            LifeDream::Slay => s.deeds.iter().any(|x| x.starts_with("slew ") || x.starts_with("kept the vow")),
            LifeDream::Book => self.works.iter().any(|w| w.maker == i && w.kind == "book"),
            LifeDream::Rule => self.speaker == Some(i),
            LifeDream::Discover => self.log.iter().any(|l| l.contains(&format!("{} finds ", s.name)) && (l.contains("in the hills") || l.contains("finds a cave") || l.contains("finds the"))),
            LifeDream::Peace => self.regards.iter().any(|r| r.causes.iter().any(|c| c.key == "peace")),
        }
    }

    /// Dawn: dreams come true.
    pub(crate) fn reckon_dreams(&mut self) {
        for i in 0..self.settlers.len() {
            if !self.settlers[i].alive || self.dreamt.contains(&i) { continue; }
            let Some(d) = self.life_dream(i) else { continue };
            if !self.dream_met(i, d) { continue; }
            self.dreamt.push(i);
            let name = self.settlers[i].name.clone();
            let her = self.settlers[i].persona.female;
            let w = d.word().replace("their ", if her { "her " } else { "his " }).replace("outlast them", if her { "outlast her" } else { "outlast him" });
            let line = format!("{} has realized a dream of a lifetime: {}.", name, w);
            self.note(line.clone());
            let at = self.settlers[i].pos;
            self.moment(format!("{}'s dream", name), line, format!("because {} had always dreamt of {}", name, w), at);
            self.feel(i, mind::Feel::Dreamt { what: d.word().to_string() });
            let f = self.settlers[i].persona.facet(Facet::Cheer);
            self.settlers[i].persona.facets[Facet::Cheer as usize] = (f + 5).min(100);
        }
    }

    /// Settler `i`'s dream, for their page and the annals.
    pub fn dream_line(&self, i: usize) -> Option<String> {
        let d = self.life_dream(i)?;
        let pos = if self.settlers[i].persona.female { "her" } else { "his" };
        let w = d.word().replace("their ", &format!("{} ", pos)).replace("outlast them", if self.settlers[i].persona.female { "outlast her" } else { "outlast him" });
        Some(if self.dreamt.contains(&i) { format!("Realized a dream of {}", w) } else { format!("Dreams of {}", w) })
    }
}
