//! What the world's peoples think of the camp, and why.
//!
//! The idea is Dwarf Fortress's diplomatic evaluation: a civilization's opinion of another is a sum
//! of enumerated causes (religion, relatives, claimed artifacts, specific events), each kept with
//! its reason, so grudges and friendships can say where they came from. Here each people the camp
//! deals with keeps a list of causes (`Regard::causes`, merged by kind): driving off their war band
//! (-5), cutting down its leader (-10), putting a prisoner of theirs to death (-20) or sending one
//! home unharmed (+20), a ransom taken (+3), a relic given back (+25) or kept (-15), tribute paid
//! (+5) or refused (-5), a sellsword of theirs hired (+3), each caravan traded with (+2, +10 at
//! most). At dawn (`reckon_regard`) a people at -15 or worse swears vengeance once: a war band of
//! theirs is the next trouble, its reason the worst cause. A people at +15 or better comes once in
//! friendship: meals (two a head, 30 at most), and their spears will not be raised against the camp
//! (their war bands leave the troubles to come). Both are moments; the annals list it all.

use super::*;
use crate::history::FactionId;

#[derive(Clone, Debug)]
pub struct Cause {
    pub key: &'static str,
    pub delta: i32,
    /// "put Ost, one of theirs, to death at the gate on day 40".
    pub text: String,
    pub day: u64,
}

#[derive(Clone, Debug)]
pub struct Regard {
    pub faction: FactionId,
    pub people: String,
    /// Their side of the world (a war band comes from there).
    pub from: Option<(usize, usize)>,
    pub causes: Vec<Cause>,
    /// Vengeance sworn or friendship offered (once), and the day.
    pub acted: Option<bool>,
    pub acted_day: u64,
}

impl Regard {
    pub fn total(&self) -> i32 { self.causes.iter().map(|c| c.delta).sum() }

    /// "sworn enemies", "friends of the camp" ...
    pub fn word(&self) -> &'static str {
        match (self.acted, self.total()) {
            (Some(false), _) => "sworn enemies of the camp",
            (Some(true), _) => "friends of the camp",
            (_, t) if t <= -10 => "think ill of the camp",
            (_, t) if t >= 10 => "think well of the camp",
            _ => "have no strong feeling for the camp",
        }
    }
}

impl Colony {
    /// A people's regard moves for a cause; causes of one `key` merge (the sum kept, the text the
    /// latest).
    pub(crate) fn regard(&mut self, faction: Option<FactionId>, people: &str, from: Option<(usize, usize)>, key: &'static str, delta: i32, text: String) {
        let Some(f) = faction else { return };
        let day = self.clock.day();
        let k = match self.regards.iter().position(|r| r.faction == f) {
            Some(k) => k,
            None => { self.regards.push(Regard { faction: f, people: people.to_string(), from, causes: Vec::new(), acted: None, acted_day: 0 }); self.regards.len() - 1 }
        };
        let r = &mut self.regards[k];
        if r.from.is_none() { r.from = from; }
        match r.causes.iter_mut().find(|c| c.key == key) {
            // Trade alone warms a people only so far (+10).
            Some(c) if key == "trade" => { c.delta = (c.delta + delta).min(10); c.text = text; c.day = day; }
            Some(c) if key == "routs" || key == "guests" || key == "legend" => { c.delta += delta; c.text = text; c.day = day; }
            _ => r.causes.push(Cause { key, delta, text, day }),
        }
    }

    /// Dawn: vengeance sworn, friendship offered.
    pub(crate) fn reckon_regard(&mut self) {
        if self.alive() == 0 || self.departed.is_some() { return; }
        let day = self.clock.day();
        for k in 0..self.regards.len() {
            let r = self.regards[k].clone();
            // Vengeance may end in peace: 180 days on (90 after their band was driven off since), an
            // envoy offers it; the speaker takes it unless they despise peace or hold the grudge
            // (vengefulness 70+).
            if r.acted == Some(false) {
                let driven = r.causes.iter().any(|c| c.key == "routs" && c.day > r.acted_day);
                if day < r.acted_day + if driven { 90 } else { 180 } { continue; }
                if self.arc.as_ref().map_or(false, |a| a.stage < 3 && a.threat.faction == Some(r.faction)) { continue; }
                let judge = self.speaker.filter(|&s| self.settlers[s].alive);
                let refuse = judge.map_or(false, |s| { let p = &self.settlers[s].persona; p.value(crate::persona::Val::Peace) <= -26 || p.facet(crate::persona::Facet::Vengefulness) >= 70 });
                let jn = judge.map(|s| self.settlers[s].name.clone()).unwrap_or_else(|| "the camp".into());
                self.regards[k].acted_day = day;
                if refuse {
                    self.note(format!("An envoy of {} comes offering peace; {} sends them away.", r.people, jn));
                    continue;
                }
                self.regards[k].acted = None;
                // Peace sets the old grievances aside: what they held against the camp is mended.
                let mend = (-r.total()).max(1);
                self.regard(Some(r.faction), &r.people, r.from, "peace", mend, format!("made peace with them on day {}", day));
                if let Some(a) = self.arc.as_mut() { a.later.retain(|x| x.faction != Some(r.faction) || x.kind != arc::ThreatKind::Warband); }
                let line = format!("An envoy of {} comes offering peace, and {} takes it: the vengeance is set aside, and the roads are open again.", r.people, jn);
                self.note(line.clone());
                let at = self.camp;
                self.moment(format!("Peace with {}", r.people), line, format!("because {} days of vengeance had cost both sides", day - r.acted_day), at);
                continue;
            }
            // Friendship can be broken by what comes after.
            if r.acted == Some(true) && r.total() > -15 { continue; }
            let t = r.total();
            if t <= -15 {
                // Not while a thread of theirs is already coming.
                let Some(a) = self.arc.as_ref() else { continue };
                if (a.stage < 3 && a.threat.faction == Some(r.faction)) || a.later.iter().any(|x| x.faction == Some(r.faction)) { continue; }
                // (What they hold against it since the last peace or friendship.)
                let recent = |c: &&Cause| r.acted_day == 0 || c.day >= r.acted_day;
                let worst = r.causes.iter().filter(recent).min_by_key(|c| (c.delta, c.day)).or_else(|| r.causes.iter().min_by_key(|c| (c.delta, c.day))).map(|c| c.text.clone()).unwrap_or_default();
                self.regards[k].acted = Some(false);
                self.regards[k].acted_day = day;
                let line = format!("Word comes down the roads: {} have sworn vengeance on the camp, which {}.", r.people, worst);
                self.note(line.clone());
                let at = self.camp;
                self.moment(format!("{} swear vengeance", capital(&r.people)), line, format!("because the camp {}", worst), at);
                for j in 0..self.settlers.len() { if self.settlers[j].alive { self.feel(j, mind::Feel::News { what: format!("{} have sworn vengeance on the camp", r.people), good: false }); } }
                let a = self.arc.as_mut().unwrap();
                let from = a.reserve.iter().find(|x| x.faction == Some(r.faction)).and_then(|x| x.from).or(r.from);
                a.reserve.retain(|x| x.faction != Some(r.faction));
                a.later.insert(0, arc::Threat { kind: arc::ThreatKind::Warband, name: format!("a war band of {}", r.people),
                    why: format!("they have sworn vengeance, for the camp {}", worst), cause: None, cause_text: Some(format!("the camp {}", worst)),
                    faction: Some(r.faction), from, size: 1.0, monster: None });
                if a.stage == 3 { a.quiet_until = Some(a.quiet_until.map_or(day + 6, |q| q.min(day + 6))); }
            } else if t >= 15 && r.acted.is_none() {
                let best = r.causes.iter().max_by_key(|c| (c.delta, c.day)).map(|c| c.text.clone()).unwrap_or_default();
                self.regards[k].acted = Some(true);
                self.regards[k].acted_day = day;
                let n = (2 * self.alive() as u32).min(30);
                for _ in 0..n { self.items.push(Item::food(Stuff::Provisions, self.camp, true)); }
                let mut dropped = false;
                if let Some(a) = self.arc.as_mut() {
                    let before = a.later.len() + a.reserve.len();
                    a.later.retain(|x| x.faction != Some(r.faction));
                    a.reserve.retain(|x| x.faction != Some(r.faction));
                    dropped = a.later.len() + a.reserve.len() < before;
                }
                let line = format!("An envoy of {} comes in friendship, remembering that the camp {}: they leave {} meals, and swear their spears will not be raised against it{}.",
                    r.people, best, n, if dropped { ", though there had been talk of war" } else { "" });
                self.note(line.clone());
                let at = self.camp;
                self.moment(format!("The friendship of {}", r.people), line, format!("because the camp {}", best), at);
                for j in 0..self.settlers.len() {
                    if self.settlers[j].alive && self.settlers[j].past.as_ref().and_then(|p| p.people) == Some(r.faction) { self.feel(j, mind::Feel::News { what: format!("their people's friendship with the camp"), good: true }); }
                }
            }
        }
    }
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    match c.next() { Some(f) => f.to_uppercase().collect::<String>() + c.as_str(), None => String::new() }
}
