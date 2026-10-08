//! Prisoners: a raider taken alive, and what the camp does with them.
//!
//! The idea is Dwarf Fortress's captives: invaders taken in battle are held, ransomed, executed
//! or released, and the choice says something about whoever makes it. Here a rout of a war band
//! or of outlaws leaves one of them behind alive one time in three (`take_prisoner`, a moment;
//! named in their people's tongue). At the next dawn the speaker (else the oldest) decides
//! (`judge_prisoner`): one who values law or fairness holds them for ransom when their people
//! stand (an envoy comes 6-10 days later and pays two meals a head of the camp, and the prisoner
//! goes home); a cruel one (70+) or one who prizes arms over mercy has them put to death at the
//! gate (the merciful are troubled); the rest send them home with a warning. Held, a prisoner
//! slips the ropes one night in ten.

use super::*;
use crate::persona::{Attr, Facet, Val};

#[derive(Clone, Debug)]
pub struct Prisoner {
    pub name: String,
    pub people: String,
    pub faction: Option<crate::history::FactionId>,
    pub day: u64,
    /// Their side of the world.
    pub from: Option<(usize, usize)>,
    /// Held for ransom: the day the envoy comes.
    pub ransom_day: Option<u64>,
}

impl Colony {
    /// After a rout: one of them taken alive.
    pub(crate) fn take_prisoner(&mut self, threat: &arc::Threat, taker: Option<usize>, seed: u64) {
        if !matches!(threat.kind, arc::ThreatKind::Warband | arc::ThreatKind::Outlaws) || self.prisoner.is_some() { return; }
        if crate::history::settlers::hash_pub(seed, 0x9415) % 3 != 0 { return; }
        let people = threat.name.trim_start_matches("a war band of ").trim_start_matches("deserters of ").split(", led by ").next().unwrap_or("the hills").to_string();
        let people = if people.starts_with("a band of") || people.is_empty() { "the outlaws".to_string() } else { people };
        let name = {
            use rand::SeedableRng;
            let style = crate::history::naming::styles::NamingStyle::from_archetype(crate::history::NamingStyleId(0), crate::history::entities::races::RaceType::from_tag("human").default_naming_archetype());
            let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed ^ 0x9416);
            crate::history::naming::generator::NameGenerator::personal_name(&style, &mut rng).split_whitespace().next().unwrap_or("Ost").to_string()
        };
        let by = taker.map(|k| self.settlers[k].name.clone()).unwrap_or_else(|| "the camp".into());
        let line = format!("As the rest flee, {} drags one of them down and binds them: {}, of {}, is taken alive.", by, name, people);
        self.note(line.clone());
        let at = self.camp;
        self.moment(format!("{} is taken", name), line, "because the camp was ready and the band broke".into(), at);
        self.prisoner = Some(prisoners::Prisoner { name, people, faction: threat.faction, day: self.clock.day(), from: threat.from, ransom_day: None });
    }

    /// Dawn: the speaker decides; the ransom comes; the prisoner may be gone.
    pub(crate) fn reckon_prisoner(&mut self) {
        let Some(p) = self.prisoner.clone() else { return };
        let day = self.clock.day();
        if let Some(r) = p.ransom_day {
            if day >= r {
                let pay = 2 * self.alive() as u32;
                for _ in 0..pay { self.items.push(Item { kind: ItemKind::Food, at: self.camp, stored: true, reserved: false }); }
                self.note(format!("An envoy of {} comes for {} and pays {} meals in ransom; the prisoner walks home with them.", p.people, p.name, pay));
                self.regard(p.faction, &p.people, p.from, "ransom", 3, format!("took ransom for {} and let them go on day {}", p.name, day));
                self.prisoner = None;
            }
            return;
        }
        if day <= p.day { return; }
        let alive: Vec<usize> = (0..self.settlers.len()).filter(|&i| self.settlers[i].alive && self.settlers[i].guest_until == 0).collect();
        let Some(&judge) = self.speaker.filter(|&s| self.settlers[s].alive).as_ref()
            .or_else(|| alive.iter().max_by_key(|&&i| (self.settlers[i].past.as_ref().map_or(30, |x| x.age), std::cmp::Reverse(i)))) else { return };
        let jp = self.settlers[judge].persona.clone();
        let jn = self.settlers[judge].name.clone();
        let cruel = jp.facet(Facet::Cruelty) >= 70 || (jp.value(Val::MartialProwess) >= 25 && jp.attr(Attr::Empathy) < 800.0);
        let lawful = jp.value(Val::Law) >= 10 || jp.value(Val::Fairness) >= 10;
        let at = self.camp;
        if cruel {
            let line = format!("{} has {} put to death at the gate, as a warning to {}.", jn, p.name, p.people);
            self.note(line.clone());
            self.moment(format!("The death of {}", p.name), line, format!("because {} {}", jn, jp.facet_phrase(Facet::Cruelty as usize).unwrap_or_else(|| "has no mercy".into())), at);
            self.regard(p.faction, &p.people, p.from, "executed", -20, format!("put {}, one of theirs, to death at the gate on day {}", p.name, day));
            for &j in &alive { if self.settlers[j].persona.facet(Facet::Altruism) >= 70 { self.feel(j, mind::Feel::Mandate { what: format!("{} be put to death", p.name) }); } }
            self.prisoner = None;
        } else if lawful && p.faction.is_some() {
            let r = day + 6 + crate::history::settlers::hash_pub(self.seed ^ day, 0x9417) % 5;
            self.note(format!("{} holds {} for ransom, and sends word to {}.", jn, p.name, p.people));
            if let Some(x) = self.prisoner.as_mut() { x.ransom_day = Some(r); }
        } else {
            let line = format!("{} sends {} home to {} with a warning not to come back.", jn, p.name, p.people);
            self.note(line);
            self.regard(p.faction, &p.people, p.from, "mercy", 20, format!("sent {}, one of theirs, home unharmed on day {}", p.name, day));
            self.prisoner = None;
        }
    }

    /// 23:00: a held prisoner may slip away.
    pub(crate) fn prisoner_escapes(&mut self) {
        let Some(p) = self.prisoner.clone() else { return };
        if crate::history::settlers::hash_pub(self.seed ^ self.clock.day(), 0x9418) % 10 != 0 { return; }
        self.note(format!("In the night {} slips the ropes and is gone into the dark.", p.name));
        self.prisoner = None;
    }
}
