//! Years in the camp: everyone ages, children grow, the old die in their beds.
//!
//! The idea is Dwarf Fortress's ages and its scale trick of rolling the year of death at birth:
//! dying of old age is a comparison, not a simulation. Here the camp's year is four seasons
//! (`YEAR_DAYS`, 120 days); at the dawn a year turns, every settler is a year older (the turn is a
//! line, and children born in the camp are named as growing). Each settler's span is rolled once,
//! from their race's lifespan (`span`) and a hash of their name: when their age reaches it they
//! die in their sleep at dawn, a death of old age (no ghost, a grave like any other, a moment
//! when they were the camp's eldest). Children born in the camp are infants to four years
//! (`infant_choice`), and work from twelve like any child of the roster.

use super::*;

/// The camp's year: four seasons.
pub const YEAR_DAYS: u64 = 4 * SEASON_DAYS;

/// A race's span of years, before each settler's own share (+-20%).
fn lifespan(race: &str) -> u32 {
    match race {
        "dwarf" => 140, "elf" => 450, "orc" => 55, "goblin" => 60, "halfling" => 95,
        "giant" => 200, "reptilian" => 90, "beastfolk" => 60, "fey" | "elemental" | "construct" | "undead" => 1000,
        _ => 72,
    }
}

impl Colony {
    /// Settler `i`'s span of years (rolled once from their name and race; the same every run).
    pub fn span(&self, i: usize) -> u32 {
        let s = &self.settlers[i];
        let base = lifespan(&s.persona.race) as f32;
        let h = crate::history::settlers::hash_pub(crate::persona::seed_of(&s.name, 0xA6E), 0x5BA2) % 1000;
        (base * (0.8 + 0.4 * h as f32 / 1000.0)) as u32
    }

    /// Dawn of a year's turn: a year older; the old who have reached their span die in their sleep.
    pub(crate) fn reckon_years(&mut self) {
        let day = self.clock.day();
        if day <= 1 || (day - 1) % YEAR_DAYS != 0 { return; }
        let year = (day - 1) / YEAR_DAYS;
        for i in 0..self.settlers.len() {
            if !self.settlers[i].alive { continue; }
            if let Some(p) = self.settlers[i].past.as_mut() { p.age += 1; }
        }
        let grown: Vec<String> = self.children.iter().filter(|c| self.settlers[c.0].alive && self.settlers[c.0].past.as_ref().map_or(false, |p| p.age == 4)).map(|c| self.settlers[c.0].name.clone()).collect();
        let n = self.alive();
        self.note(format!("A year turns in the camp: its {} year. {} live here{}.", ordinal(year), n,
            if grown.is_empty() { String::new() } else { format!("; {} {} on their own feet now", crate::persona::list(&grown), if grown.len() == 1 { "is" } else { "are" }) }));
        // The old.
        let eldest = (0..self.settlers.len()).filter(|&i| self.settlers[i].alive).max_by_key(|&i| (self.settlers[i].past.as_ref().map_or(0, |p| p.age), std::cmp::Reverse(i)));
        for i in 0..self.settlers.len() {
            let s = &self.settlers[i];
            if !s.alive { continue; }
            let age = s.past.as_ref().map_or(30, |p| p.age);
            if age < self.span(i) { continue; }
            let name = s.name.clone();
            let line = format!("{} dies in {} sleep, aged {}, an old {} among them.", name, if s.persona.female { "her" } else { "his" }, age, if s.persona.female { "woman" } else { "man" });
            self.note(line.clone());
            if eldest == Some(i) {
                let at = self.settlers[i].pos;
                self.moment(format!("The death of {}", name), line, format!("because {} had lived out {} years", name, age), at);
            }
            self.bury(i, &format!("in {} sleep, of old age, aged {}", if self.settlers[i].persona.female { "her" } else { "his" }, age));
        }
    }
}

fn ordinal(n: u64) -> String {
    match n { 1 => "second".into(), 2 => "third".into(), 3 => "fourth".into(), 4 => "fifth".into(), 5 => "sixth".into(), 6 => "seventh".into(), 7 => "eighth".into(), 8 => "ninth".into(), 9 => "tenth".into(), _ => format!("{}th", n + 1) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_follow_the_races() {
        assert!(lifespan("dwarf") > lifespan("human") && lifespan("human") > lifespan("orc"));
        assert!(lifespan("elf") > lifespan("dwarf"));
        assert_eq!(lifespan("unknown people"), 72);
        assert_eq!(YEAR_DAYS, 120);
    }
}
