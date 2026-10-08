//! Childhood: children born in the camp play, tag along, and come of age.
//!
//! The idea is Dwarf Fortress's children: they do not work; they play and follow the adults about,
//! and at twelve they become citizens who take up a trade. Here a child born in the camp
//! (`Colony::children`), from four to eleven (`child_choice`, after the infant years in
//! `family.rs`), eats and sleeps like anyone; by day, when a parent is at work, they follow them
//! ("Tagging along after X, who is felling a tree"), else they play near the fire. Each dawn a
//! child who spent the day at a parent's side learns a little of that parent's best trade
//! (`reckon_childhood`: a tenth of the gap, up to 0.4). At twelve they come of age (a line; the
//! parent's trade is named), and from then on choose their work like anyone.

use super::*;

impl Colony {
    /// Settler `i`'s parents, if they were born in the camp.
    fn parents_of(&self, i: usize) -> Option<(usize, usize)> {
        self.children.iter().find(|c| c.0 == i).map(|c| (c.1, c.2))
    }

    /// A child of four to eleven born here: their day.
    pub(crate) fn child_choice(&self, i: usize) -> Option<(Job, String)> {
        let (m, f) = self.parents_of(i)?;
        let age = self.settlers[i].past.as_ref().map_or(0, |p| p.age);
        if !(4..12).contains(&age) { return None; }
        let s = &self.settlers[i];
        if s.hunger >= 0.6 && self.food_stored() > 0 { return Some((Job::Eat, "Hungry, and fed by the fire".into())); }
        if self.clock.is_night() || s.fatigue >= 0.6 { return Some((Job::Sleep, "Asleep by the fire".into())); }
        // A parent at work: tag along.
        let busy = |p: usize| self.settlers[p].alive && !matches!(self.settlers[p].job, Job::Sleep | Job::Eat | Job::Idle);
        if let Some(p) = [m, f].into_iter().find(|&p| busy(p)) {
            return Some((Job::Wander(self.settlers[p].pos), format!("Tagging along after {}, who is {}", self.settlers[p].name, self.settlers[p].job.verb())));
        }
        let h = crate::history::settlers::hash_pub(self.seed ^ self.clock.tick / 120, i as u64);
        let spot = self.spot_from_camp((h % 9) as i32 - 4, ((h / 9) % 9) as i32 - 4);
        Some((Job::Wander(spot), "Playing by the fire".into()))
    }

    /// Dawn: children learn at their parents' side; at twelve they come of age.
    pub(crate) fn reckon_childhood(&mut self) {
        for k in 0..self.children.len() {
            let (c, m, f) = self.children[k];
            if !self.settlers[c].alive { continue; }
            let age = self.settlers[c].past.as_ref().map_or(0, |p| p.age);
            if (4..12).contains(&age) {
                // The parent they tagged along after most: the one alive with the higher hand.
                let Some(p) = [m, f].into_iter().filter(|&p| self.settlers[p].alive).max_by(|&a, &b| {
                    let best = |x: usize| self.settlers[x].skill.iter().copied().fold(0.0f32, f32::max);
                    best(a).total_cmp(&best(b)).then(b.cmp(&a))
                }) else { continue };
                let t = (0..ROLES.len()).max_by(|&a, &b| self.settlers[p].skill[a].total_cmp(&self.settlers[p].skill[b]).then(b.cmp(&a))).unwrap_or(0);
                let sk = &mut self.settlers[c].skill[t];
                *sk = (*sk + 0.1 * (0.4 - *sk).max(0.0)).min(0.4);
            } else if age >= 12 && !self.come_of_age.contains(&c) {
                self.come_of_age.push(c);
                let t = (0..ROLES.len()).max_by(|&a, &b| self.settlers[c].skill[a].total_cmp(&self.settlers[c].skill[b]).then(b.cmp(&a))).unwrap_or(0);
                let name = self.settlers[c].name.clone();
                self.note(format!("{} has come of age, and takes up the work of a {}, as learned at a parent's side.", name, ROLES[t]));
            }
        }
    }
}
