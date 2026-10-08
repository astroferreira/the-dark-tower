//! The call to arms: the camp's people at war send for its spears.
//!
//! The idea is Dwarf Fortress's squads sent away on missions by the civilization, and the
//! world's wars reaching into a site: the people a fortress belongs to are at war, and some of
//! its citizens go and fight in it. Here, at founding (`plan`, from `viewer::found_colony`), the
//! first war in the history that the camp's people are fighting is noted. From day 60, once, at
//! dawn (`reckon_war_call`), a rider of that people brings the call: up to two grown settlers of
//! that people who value arms or loyalty and are brave (50+) go (not the lord, the priest, the
//! healer, a guest or a parent of an infant), away 60-90 days (`Settler::away_until`, "away at the
//! war"). They come back (`war_return`) seasoned (a deed, drill +0.2), or wounded too, or not at
//! all ("falls in X", a stone in their memory at the camp): by the hash of the day, one in two,
//! one in four, one in four. Their friends miss them while they are gone.

use super::*;
use crate::history::FactionId;
use crate::persona::{Facet, Val};

#[derive(Clone, Debug)]
pub struct WarCall {
    /// "The War of the Broken Ford".
    pub war: String,
    pub people: FactionId,
    pub people_name: String,
    pub enemy: String,
    /// Who went and when they come back; whether the call has come.
    pub gone: Vec<(usize, u64)>,
    pub called: bool,
}

/// The first war the camp's people are fighting, if any.
pub fn plan(h: &crate::history::world_state::WorldHistory, people: Option<FactionId>) -> Option<WarCall> {
    let p = people?;
    let mut wars: Vec<_> = h.wars.values().filter(|w| w.ended.is_none() && (w.aggressors.contains(&p) || w.defenders.contains(&p))).collect();
    wars.sort_by_key(|w| w.id);
    let w = wars.first()?;
    let enemy = if w.aggressors.contains(&p) { w.defenders.first() } else { w.aggressors.first() }.copied()?;
    Some(WarCall { war: w.name.clone(), people: p, people_name: h.factions.get(&p)?.name.clone(), enemy: h.factions.get(&enemy)?.name.clone(), gone: Vec::new(), called: false })
}

impl Colony {
    /// Dawn: the call comes; those away come home.
    pub(crate) fn reckon_war_call(&mut self) {
        let Some(c) = self.war_call.clone() else { return };
        let day = self.clock.day();
        if !c.called && day >= 60 && self.alive() >= 8 {
            if let Some(x) = self.war_call.as_mut() { x.called = true; }
            let lord = self.ruling_lord();
            let (priest, healer) = (self.priest(), self.healer);
            let mothers: Vec<usize> = self.children.iter().filter(|k| self.settlers[k.0].alive && self.settlers[k.0].past.as_ref().map_or(false, |p| p.age < 4)).flat_map(|k| [k.1, k.2]).collect();
            let mut go: Vec<usize> = (0..self.settlers.len()).filter(|&i| {
                let s = &self.settlers[i];
                s.alive && s.guest_until == 0 && s.past.as_ref().map_or(false, |p| p.age >= 16 && p.people == Some(c.people))
                    && Some(i) != lord && Some(i) != priest && Some(i) != healer && !mothers.contains(&i)
                    && (s.persona.value(Val::MartialProwess) > 0 || s.persona.value(Val::Loyalty) >= 20) && s.persona.facet(Facet::Bravery) >= 50
            }).collect();
            go.sort_by(|&a, &b| self.fight_skill(b).total_cmp(&self.fight_skill(a)).then(a.cmp(&b)));
            go.truncate(2);
            if go.is_empty() {
                self.note(format!("A rider of {} comes calling for spears for {}; no one here will go.", c.people_name, c.war));
                return;
            }
            let back = day + 60 + crate::history::settlers::hash_pub(self.seed, 0xCA11) % 31;
            let names: Vec<String> = go.iter().map(|&i| self.settlers[i].name.clone()).collect();
            let line = format!("A rider of {} comes calling for spears for {} against {}: {} take up {} and go.", c.people_name, c.war, c.enemy, crate::persona::list(&names), if go.len() == 1 { "a spear" } else { "spears" });
            self.note(line.clone());
            let at = self.camp;
            self.moment(format!("Called to {}", c.war), line, format!("because {} are at war and these hold their people's cause dear", c.people_name), at);
            for &i in &go {
                self.release(i);
                let s = &mut self.settlers[i];
                s.alive = false;
                s.away_until = back;
                s.why = format!("Away at {}", c.war);
                // Their friends miss them.
                for j in 0..self.settlers.len() {
                    if j != i && self.settlers[j].alive && self.opinion(i, j) >= 12 { let w = self.settlers[i].name.clone(); self.feel(j, mind::Feel::News { what: format!("{} went to the war", w), good: false }); }
                }
            }
            if let Some(x) = self.war_call.as_mut() { x.gone = go.iter().map(|&i| (i, back)).collect(); }
            return;
        }
        let due: Vec<(usize, u64)> = c.gone.iter().copied().filter(|&(_, b)| day >= b).collect();
        for (i, _) in due {
            if let Some(x) = self.war_call.as_mut() { x.gone.retain(|g| g.0 != i); }
            self.war_return(i, &c);
        }
    }

    fn war_return(&mut self, i: usize, c: &WarCall) {
        let day = self.clock.day();
        let name = self.settlers[i].name.clone();
        let roll = crate::history::settlers::hash_pub(self.seed ^ i as u64, 0xCA12) % 4;
        let s = &mut self.settlers[i];
        s.away_until = 0;
        if roll == 3 {
            // They do not come back.
            s.mind.left = true;
            let line = format!("Word comes from {}: {} fell in {}, and will not come home.", c.people_name, name, c.war);
            self.note(line.clone());
            let at = self.spot_from_camp(-5, -6);
            self.marks.push(ColonyMark { at, kind: MarkKind::Stone, title: format!("The stone of {}", name), text: format!("For {}, who went to {} and fell there, as word came on day {}.", name, c.war, day), day });
            self.moment(format!("{} falls in {}", name, c.war), line, format!("because {} answered the call of {}", name, c.people_name), at);
            for j in 0..self.settlers.len() {
                if self.settlers[j].alive && self.opinion(i, j) >= 6 { self.feel(j, mind::Feel::Death { whom: name.clone(), close: self.opinion(i, j) >= 12 }); }
            }
            return;
        }
        s.alive = true;
        s.pos = self.camp;
        s.path.clear();
        s.job = Job::Idle;
        s.hunger = s.hunger.min(0.6);
        s.drill = (s.drill + 0.2).min(0.6);
        s.deeds.push(format!("fought in {} against {}", c.war, c.enemy));
        let wound = roll == 2;
        let line = format!("{} comes home from {}{}.", name, c.war, if wound { ", limping, with a story they will not tell" } else { ", leaner and quieter, a seasoned hand with the spear" });
        self.note(line.clone());
        if wound { self.wound(i, "left leg", 2, format!("{}, day {}", c.war, day)); }
        let at = self.camp;
        self.moment(format!("{} comes home", name), line, format!("because {} answered the call of {}", name, c.people_name), at);
        self.feel(i, mind::Feel::Slew { what: format!("the enemies of {}", c.people_name) });
    }
}
