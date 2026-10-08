//! The tavern: lodging for travellers, songs every night, a cup for the sociable, and swords for
//! hire.
//!
//! The idea is Dwarf Fortress's taverns (since its fifth version): a hall with drink where
//! visitors lodge, performers play and mercenaries look for work. Here, once a visitor has come
//! and twelve live in the camp, it raises a tavern (`ProjectKind::Tavern`, 5x4 roofed). With it,
//! bards come twice as often and stay twice as long (`visitors.rs`), the performances are "in the
//! tavern", and at 20:00 the sociable (gregarious 60+) who are by the fire share a cup there while
//! drink lasts (`Feel::Friend` "an evening at the tavern"). When a raid is foretold and a tavern
//! stands, a sellsword (a living warrior of a people at peace with the settlers', from
//! `visitors::plan`) comes to it on the first dawn of the trouble and offers to fight for meals, two a head of
//! the camp: paid if the store bears it, they stand first in the fight with a veteran's hand, and
//! leave the day after the raid; refused, they drink and go.

use super::*;

impl Colony {
    pub fn tavern(&self) -> Option<Pos> {
        self.projects.iter().find(|p| p.done && p.kind == projects::ProjectKind::Tavern).map(|p| (p.at.0 + 2, p.at.1 + 2))
    }

    /// 20:00: the sociable share a cup at the tavern.
    pub(crate) fn tavern_evening(&mut self) {
        if self.tavern().is_none() || self.drink == 0 { return; }
        let camp = self.camp;
        let near = |p: Pos| (p.0 as i32 - camp.0 as i32).abs().max((p.1 as i32 - camp.1 as i32).abs()) <= 12;
        let who: Vec<usize> = (0..self.settlers.len()).filter(|&i| self.settlers[i].alive && near(self.settlers[i].pos)
            && self.settlers[i].persona.facet(crate::persona::Facet::Gregariousness) >= 60 && self.settlers[i].past.as_ref().map_or(true, |p| p.age >= 14)).collect();
        for &i in &who {
            if self.drink == 0 { break; }
            self.drink -= 1;
            self.feel(i, mind::Feel::Friend { with: "everyone, an evening at the tavern".into() });
        }
        for a in 0..who.len() { for b in a + 1..who.len() { self.warm(who[a], who[b]); } }
        // A brawl (DF's tavern fights): two who dislike each other, one of them quick to anger or
        // drawn to violence, come to blows over their cups one evening in four; the one who
        // started it has a crime to answer for at dawn (`justice.rs`).
        let hot = |c: &Colony, i: usize| { let p = &c.settlers[i].persona; p.facet(crate::persona::Facet::Anger) >= 70 || p.facet(crate::persona::Facet::Violence) >= 60 };
        let pair = who.iter().flat_map(|&a| who.iter().map(move |&b| (a, b))).filter(|&(a, b)| a != b && hot(self, a) && self.opinion(a, b) <= -3)
            .min_by_key(|&(a, b)| (self.opinion(a, b), a, b));
        if let Some((a, b)) = pair {
            // (Not the same two again within twenty days.)
            let again = self.crimes.iter().any(|c| c.who == a && c.what.contains(&format!("struck {}", self.settlers[b].name)) && c.day + 20 > self.clock.day());
            if !again && crate::history::settlers::hash_pub(self.seed ^ self.clock.day(), 0xB4A1 + a as u64) % 4 == 0 {
                let day = self.clock.day();
                let (an, bn) = (self.settlers[a].name.clone(), self.settlers[b].name.clone());
                self.note(format!("{} and {} come to blows over their cups at the tavern; {} goes home with a split lip.", an, bn, bn));
                self.wound(b, "head", 1, format!("{}'s fist at the tavern, day {}", an, day));
                self.like(a, b, -4);
                self.feel(b, mind::Feel::Quarrel { with: an.clone() });
                self.crimes.push(justice::Crime { who: a, what: format!("struck {} at the tavern", bn), day, meals: 0, judged: false });
            }
        }
    }

    /// Dawn: with a raid foretold and a tavern, a sellsword may come and offer to fight.
    pub(crate) fn sellsword_comes(&mut self) {
        let day = self.clock.day();
        if self.tavern().is_none() || self.trouble_foretold().is_none() || self.sellsword_hired.is_some() { return; }
        // (The first dawn trouble is foretold with a tavern standing: word travels fast there.)
        let Some(k) = self.visitors.iter().position(|v| !v.came && matches!(v.kind, visitors::VisitKind::Sellsword)) else { return };
        self.visitors[k].came = true;
        let v = self.visitors[k].clone();
        let threat = self.trouble_foretold().unwrap_or_default();
        let price = 2 * self.alive() as u32;
        let judge = self.speaker.filter(|&s| self.settlers[s].alive).map(|s| self.settlers[s].name.clone()).unwrap_or_else(|| "the camp".into());
        if self.food_stored() < price + 3 * self.alive() as u32 {
            self.note(format!("{}, {}, drinks at the tavern and offers {} sword against {} for {} meals; the store cannot bear it, and {} drinks up and goes.", v.name, v.calling, if v.past.persona.as_ref().map_or(false, |p| p.female) { "her" } else { "his" }, threat, price, v.name));
            return;
        }
        for _ in 0..price { if let Some(j) = self.items.iter().rposition(|it| it.kind == ItemKind::Food && it.stored && !it.reserved) { self.items.remove(j); self.fix_refs_pub(j); } }
        let raid = self.arc.as_ref().map_or(day + 5, |a| a.raid_day.max(day));
        self.add_settler(v.name.clone(), Some(v.past.clone()));
        let i = self.settlers.len() - 1;
        self.settlers[i].guest_until = (raid + 1) * TICKS_PER_DAY + 9 * 60;
        self.settlers[i].visitor = Some(v.calling.clone());
        self.settlers[i].drill = v.hand;
        self.sellsword_hired = Some(i);
        let (f, pn) = (v.past.people, v.calling.trim_start_matches("a sellsword of ").to_string());
        self.regard(f, &pn, None, "guests", 3, format!("hired {}, a sellsword of theirs, on day {}", v.name, day));
        let line = format!("{}, {}, is hired at the tavern to stand against {}: {} pays {} meals.", v.name, v.calling, threat, judge, price);
        self.note(line.clone());
        let at = self.tavern().unwrap_or(self.camp);
        self.moment(format!("{} is hired", v.name), line, format!("because {} is coming and the tavern draws those who live by the sword", threat), at);
    }
}
