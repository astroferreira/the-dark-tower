//! The camp answers what keeps coming back: the wolves' den, the grave that will not stay shut.
//!
//! The idea is Dwarf Fortress's fortress answering its own troubles: hunters clear the dangerous
//! wildlife, and the dead that rise are dealt with (DF: corpses burned or entombed). Here, at
//! dawn (`reckon_responses`): after four wolf bites (`Colony::wolf_bites`), two to four of the
//! bold (bravery 50+, or bearing a spear; grown, well, not guests) go out to the nearest den
//! with spears and fire and kill the pack (a moment, a deed, the den empty for good:
//! `dens_cleared`); one time in three one of them is bitten. After the third time one grave's
//! dead has risen (`risings`), they dig it up and burn what is left on a pyre (a moment; that
//! grave is quiet for good: `burned`), and those who loved the dead grieve again.

use super::*;
use crate::persona::Facet;

impl Colony {
    /// Dawn: the den cleared, the restless burned.
    pub(crate) fn reckon_responses(&mut self) {
        if self.alive() == 0 { return; }
        let day = self.clock.day();
        // The wolves.
        if self.wolf_bites >= 4 {
            let camp = self.camp;
            let n = self.map.width;
            let den = (0..self.map.features.len()).filter(|&i| self.map.features[i] == crate::local::wildlife::Feature::Den)
                .map(|i| ((i % n) as u16, (i / n) as u16)).filter(|d| !self.dens_cleared.contains(d))
                .min_by_key(|d| (d.0 as i32 - camp.0 as i32).abs().max((d.1 as i32 - camp.1 as i32).abs()));
            if let Some(den) = den {
                let armed: Vec<usize> = self.arms.iter().filter_map(|a| a.holder).collect();
                let mut party: Vec<usize> = (0..self.settlers.len()).filter(|&i| {
                    let s = &self.settlers[i];
                    s.alive && s.guest_until == 0 && s.ill_until <= self.clock.tick && s.past.as_ref().map_or(true, |p| p.age >= 14)
                        && (armed.contains(&i) || s.persona.facet(Facet::Bravery) >= 50)
                }).collect();
                party.sort_by(|&a, &b| self.fight_skill(b).total_cmp(&self.fight_skill(a)).then(a.cmp(&b)));
                party.truncate(4);
                if party.len() >= 2 {
                    self.wolf_bites = 0;
                    self.dens_cleared.push(den);
                    let names: Vec<String> = party.iter().map(|&i| self.settlers[i].name.clone()).collect();
                    let line = format!("At first light {} go out to the wolves' den at {},{} with spears and fire, and kill the pack.", crate::persona::list(&names), den.0, den.1);
                    self.note(line.clone());
                    self.moment("The wolves' den".into(), line, "because the wolves had bitten four of them in the dark".into(), den);
                    let lead = party[0];
                    self.settlers[lead].deeds.push(format!("led the killing of the wolves at {},{} on day {}", den.0, den.1, day));
                    self.feel(lead, mind::Feel::Slew { what: "the wolves of the den".into() });
                    if crate::history::settlers::hash_pub(self.seed ^ day, 0x3E7) % 3 == 0 {
                        let hurt = party[(crate::history::settlers::hash_pub(self.seed ^ day, 0x3E8) as usize) % party.len()];
                        let hn = self.settlers[hurt].name.clone();
                        self.note(format!("{} comes back bitten on the arm.", hn));
                        self.wound(hurt, "left arm", 1, format!("the wolves' den, day {}", day));
                    }
                    // Wolves already out tonight go home and do not come back.
                    self.creatures.retain(|c| !(c.kind == creatures::CreatureKind::Wolf && c.name == "a wolf"));
                }
            }
        }
        // The grave that will not stay shut.
        let restless: Option<Pos> = self.risings.iter().filter(|(p, &n)| n >= 3 && !self.burned.contains(p)).map(|(p, _)| *p).min();
        if let Some(at) = restless {
            let Some(k) = self.marks.iter().position(|m| m.kind == MarkKind::Grave && m.at == at) else { self.burned.push(at); return };
            let who = self.marks[k].title.trim_start_matches("The grave of ").to_string();
            let named = !who.starts_with("An old");
            self.burned.push(at);
            let line = if named { format!("They dig up {} and burn what is left on a pyre; the grave at {},{} is empty now.", who, at.0, at.1) }
                else { format!("They dig up the old grave at {},{} and burn the bones in it on a pyre.", at.0, at.1) };
            self.note(line.clone());
            self.moment(if named { format!("The burning of {}", who) } else { "The old grave burned".into() }, line, "because the dead in it had risen three times".into(), at);
            if named {
                for j in 0..self.settlers.len() {
                    if !self.settlers[j].alive { continue; }
                    let close = self.settlers.iter().position(|s| s.name == who).map_or(false, |d| self.opinion(j, d) >= 12);
                    if close { self.feel(j, mind::Feel::Death { whom: format!("{}, burned", who), close: true }); }
                }
            }
        }
    }
}
