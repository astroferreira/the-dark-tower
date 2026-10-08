//! Rations: when the store will not last, the speaker cuts the meals.
//!
//! A colony's last resort before famine, read from the camp's own numbers. At dawn
//! (`reckon_rations`), in a hard winter, when the store's days of food
//! (`days_of_food`) fall short of the days left until spring, the speaker (else the oldest) orders
//! half rations (a line, the first time a moment): every second meal a settler eats takes nothing
//! from the store (`Settler::rationed`), and each meal on rations is a small misery
//! (`Feel::Rationed`; the greedy and the immoderate mind it most). Rations end when the store holds
//! what is left of winter and a fifth more, or spring comes.

use super::*;
use crate::persona::Facet;

impl Colony {
    /// Days until the next spring (0 outside winter's reach).
    fn days_to_spring(&self) -> u64 {
        let into = (self.clock.day().max(1) - 1) % (4 * SEASON_DAYS);
        if into >= 3 * SEASON_DAYS { 4 * SEASON_DAYS - into } else { 0 }
    }

    /// Dawn: rations begin or end.
    pub(crate) fn reckon_rations(&mut self) {
        if self.alive() == 0 { return; }
        // (A mild winter's bushes still give: only a hard one counts.)
        let left = if self.hard_winter() { self.days_to_spring() } else { 0 };
        let have = self.days_of_food();
        if self.rations {
            if left == 0 || have >= left as f32 * 1.2 {
                self.rations = false;
                self.note("The rations are lifted: the store will see them through.".into());
            }
            return;
        }
        if left == 0 || have >= left as f32 { return; }
        let pool: Vec<usize> = (0..self.settlers.len()).filter(|&i| self.settlers[i].alive && self.settlers[i].guest_until == 0).collect();
        let Some(&judge) = self.speaker.filter(|&s| self.settlers[s].alive).as_ref()
            .or_else(|| pool.iter().max_by_key(|&&i| (self.settlers[i].past.as_ref().map_or(30, |p| p.age), std::cmp::Reverse(i)))) else { return };
        self.rations = true;
        let jn = self.settlers[judge].name.clone();
        let line = format!("{} orders the store rationed: half a meal at a time until spring. The store holds {:.0} days of food, and spring is {} days off.", jn, have, left);
        self.note(line.clone());
        if self.milestones.insert("rations") {
            let at = self.camp;
            self.moment("Rations".into(), line, format!("because {} would rather all go hungry than some starve", jn), at);
        }
    }

    /// A meal under rations: whether it takes from the store (every second one does).
    pub(crate) fn ration_takes(&mut self, i: usize) -> bool {
        if !self.rations { return true; }
        let s = &mut self.settlers[i];
        s.rationed = !s.rationed;
        let take = !s.rationed;
        if !take { self.feel(i, mind::Feel::Rationed); }
        take
    }
}

impl mind::Feel {
    /// (For the weight: the greedy and immoderate mind short meals most.)
    pub(crate) fn rationed_weight(p: &crate::persona::Persona) -> f32 {
        -0.03 * (0.5 + 0.5 * p.facet(Facet::Greed) as f32 / 100.0 + 0.5 * p.facet(Facet::Immoderation) as f32 / 100.0)
    }
}
