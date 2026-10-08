//! A speaker and their mandates: the camp's first office.
//!
//! The idea is Dwarf Fortress's positions and nobles' mandates. Once the camp has grown (ten
//! souls, or a month in), it chooses someone to speak for it: whoever the others think best of,
//! weighted by their grace with people (social awareness, eloquence, confidence). The speaker
//! makes peace after a quarrel when they can feel what others feel, and each season proclaims a
//! mandate from the value they hold dearest. A mandate changes what the camp does (a song every
//! night, the trees near the fire spared, a watch every night, no idle hands, works for the
//! caravans, the festival kept even when the store is low), and each settler takes it well or
//! badly by their own values, which shows in what they think of the speaker. When the speaker
//! dies or leaves, the camp chooses again.

use super::*;
use crate::persona::{Attr, Facet, Val};

/// What the speaker asks of the camp.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mandate { Works, SpareTrees, Watch, Songs, NoIdleHands, Feasts }

impl Mandate {
    /// The value it comes from and the words of the proclamation.
    fn from_value(v: Val) -> Option<Mandate> {
        Some(match v {
            Val::Craftsmanship | Val::Commerce | Val::Artwork | Val::Skill => Mandate::Works,
            Val::Nature | Val::Tranquility => Mandate::SpareTrees,
            Val::MartialProwess | Val::Law | Val::Loyalty => Mandate::Watch,
            Val::Merriment | Val::Leisure | Val::Romance => Mandate::Songs,
            Val::HardWork | Val::Perseverance | Val::SelfControl => Mandate::NoIdleHands,
            Val::Tradition | Val::Family | Val::Friendship | Val::Cooperation | Val::Harmony => Mandate::Feasts,
            _ => return None,
        })
    }
    pub fn words(self) -> &'static str {
        match self {
            Mandate::Works => "every spare hand is to make something at the workshop, for the caravans and for pride",
            Mandate::SpareTrees => "no tree within twenty paces of the fire is to be felled",
            Mandate::Watch => "the watch is to be kept every night, raid or no raid",
            Mandate::Songs => "there is to be a song by the fire every night",
            Mandate::NoIdleHands => "no hand is to be idle: whoever has nothing to do is to find work",
            Mandate::Feasts => "the turn of each season is to be kept with a feast, however low the store",
        }
    }
    /// A few words for the HUD.
    pub fn short(self) -> &'static str {
        match self {
            Mandate::Works => "every spare hand at the workshop",
            Mandate::SpareTrees => "the trees by the fire spared",
            Mandate::Watch => "a watch every night",
            Mandate::Songs => "a song every night",
            Mandate::NoIdleHands => "no idle hands",
            Mandate::Feasts => "a feast each season",
        }
    }
    /// The values that welcome it (the rest of each settler's values decide nothing).
    fn values(self) -> &'static [Val] {
        match self {
            Mandate::Works => &[Val::Craftsmanship, Val::Commerce, Val::Artwork],
            Mandate::SpareTrees => &[Val::Nature, Val::Tranquility],
            Mandate::Watch => &[Val::MartialProwess, Val::Law, Val::Loyalty],
            Mandate::Songs => &[Val::Merriment, Val::Artwork, Val::Leisure],
            Mandate::NoIdleHands => &[Val::HardWork, Val::Perseverance],
            Mandate::Feasts => &[Val::Tradition, Val::Merriment, Val::Family],
        }
    }
}

impl Colony {
    /// How the camp stands, for the HUD: its speaker and mandate, its temple, a mood, the stocks,
    /// the moon, the dead, the unhappy. Short lines, most pressing first.
    pub fn standing(&self) -> Vec<String> {
        let mut out = Vec::new();
        let day = self.clock.day();
        if let Some(m) = self.mood.as_ref().filter(|m| !m.done && self.settlers[m.who].alive) {
            out.push(format!("{} is taken by a mood: wants stone, wood and {}", self.settlers[m.who].name, m.wants));
        }
        if self.full_moon() && (self.were.is_some() || !self.cursed.is_empty()) {
            out.push("The full moon is up tonight".into());
        } else if self.were.is_some() && (14 + 28 - day % 28) % 28 <= 3 {
            let n = (14 + 28 - day % 28) % 28;
            out.push(format!("The full moon in {} {}", n, if n == 1 { "day" } else { "days" }));
        }
        if let Some(s) = self.siege.as_ref().filter(|s| day >= s.since) {
            out.push(format!("Besieged by {}: day {} of the siege", s.who.split(", led by ").next().unwrap_or(""), day - s.since + 1));
        }
        if let Some((who, until)) = self.stocks.filter(|&(_, u)| u > self.clock.tick) {
            let _ = until;
            out.push(format!("{} is in the stocks", self.settlers[who].name));
        }
        let unhappy = self.settlers.iter().filter(|s| s.alive && s.mind.stress >= 0.8).count();
        let broken = self.settlers.iter().filter(|s| s.alive && s.mind.broken.is_some()).count();
        if broken > 0 { out.push(format!("{} {} lost to a fit", broken, if broken == 1 { "is" } else { "are" })); }
        else if unhappy == 1 { if let Some(s) = self.settlers.iter().find(|s| s.alive && s.mind.stress >= 0.8) { out.push(format!("{} is miserable", s.name)); } }
        else if unhappy > 1 { out.push(format!("{} are miserable", unhappy)); }
        if let Some(sp) = self.speaker.filter(|&s| self.settlers[s].alive) {
            out.push(match self.mandate {
                Some(m) => format!("{} speaks for the camp: {}", self.settlers[sp].name, m.short()),
                None => format!("{} speaks for the camp", self.settlers[sp].name),
            });
        }
        if let Some(e) = &self.expedition { out.push(format!("{} away hunting {}", e.party.len(), e.beast)); }
        // The lord's demand, and the days left to meet it (`nobles.rs`).
        if let Some((what, since, false)) = self.lord.as_ref().and_then(|l| l.demand.clone()) {
            let left = (since + 30).saturating_sub(day);
            out.push(format!("The lord wants a fine work of {} ({} {} left)", what, left, if left == 1 { "day" } else { "days" }));
        }
        if self.rations { out.push(format!("On half rations: {:.0} days of food in the store", self.days_of_food())); }
        if let Some(c) = self.war_call.as_ref().filter(|c| !c.gone.is_empty()) { out.push(format!("{} away at {}", c.gone.len(), c.war)); }
        // Those sworn to vengeance on the camp (`regard.rs`).
        let foes: Vec<String> = self.regards.iter().filter(|r| r.acted == Some(false)).map(|r| r.people.clone()).collect();
        if !foes.is_empty() { out.push(format!("Sworn to vengeance on the camp: {}", crate::persona::list(&foes))); }
        if let Some((_, god)) = self.temple() { out.push(format!("A temple to {}", god)); }
        if let Some(r) = self.relic.as_ref().filter(|r| r.known && r.fate.is_none()) {
            out.push(match &r.found { Some(_) if r.contested => format!("{} is kept; they will come for it", r.name), Some(_) => format!("{} is kept in the camp", r.name), None => format!("{} lies somewhere near", r.name) });
        }
        if self.milestones_hit.iter().any(|m| m == "the dead walk") { out.push("The dead do not rest here".into()); }
        out
    }

    /// The god most of the devout share (piety 60+), and how many: ("Xilnar the Stormmother", 4).
    pub fn devout_faith(&self) -> Option<(String, usize)> {
        let mut count: Vec<(String, usize)> = Vec::new();
        for s in self.settlers.iter().filter(|s| s.alive && s.persona.facet(Facet::Piety) >= 60) {
            if let Some((_, god)) = s.past.as_ref().and_then(|p| p.faith.clone()) {
                match count.iter_mut().find(|c| c.0 == god) { Some(c) => c.1 += 1, None => count.push((god, 1)) }
            }
        }
        count.into_iter().max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
    }

    /// The temple's floor, and the god it was raised to, once one stands.
    pub fn temple(&self) -> Option<(Pos, String)> {
        let p = self.projects.iter().find(|p| p.done && p.kind == super::projects::ProjectKind::Temple)?;
        let god = p.why.split(" pray to ").nth(1).and_then(|r| r.split(", with no roof").next()).unwrap_or("their god").to_string();
        Some(((p.at.0 + 1, p.at.1 + 2), god))
    }

    /// Each dawn: choose a speaker when the camp is big or old enough and has none; each season's
    /// first day, a mandate.
    pub(crate) fn reckon_society(&mut self) {
        let day = self.clock.day();
        let speaker_alive = self.speaker.map_or(false, |s| self.settlers[s].alive);
        if !speaker_alive {
            let lost = self.speaker.map(|s| self.settlers[s].name.clone());
            self.speaker = None;
            if self.alive() >= 10 || (day >= 30 && self.alive() >= 4) { self.choose_speaker(lost); }
        }
        if let Some(s) = self.speaker {
            if self.mandate.is_none() || (day > 1 && (day - 1) % SEASON_DAYS == 0 && self.mandate_day != day) { self.proclaim(s); }
        }
    }

    fn choose_speaker(&mut self, lost: Option<String>) {
        let alive: Vec<usize> = (0..self.settlers.len()).filter(|&i| self.settlers[i].alive && self.settlers[i].guest_until == 0 && self.settlers[i].past.as_ref().map_or(true, |p| p.age >= 16)).collect();
        if alive.is_empty() { return; }
        // What the others think of them, and their grace with people.
        let score = |c: &Colony, i: usize| -> f32 {
            let liked: i32 = alive.iter().filter(|&&j| j != i).map(|&j| c.opinion(i, j)).sum();
            let p = &c.settlers[i].persona;
            liked as f32 + 6.0 * (p.attr(Attr::SocialAwareness) / 1000.0 + p.attr(Attr::LinguisticAbility) / 1000.0) + 4.0 * p.facet(Facet::Confidence) as f32 / 100.0
        };
        let Some(&s) = alive.iter().max_by(|&&a, &&b| score(self, a).total_cmp(&score(self, b)).then(b.cmp(&a))) else { return };
        self.speaker = Some(s);
        for k in 0..self.settlers.len() { if self.settlers[k].office.as_deref() == Some("Speaks for the camp") { self.settlers[k].office = None; } }
        self.settlers[s].office = Some("Speaks for the camp".into());
        let name = self.settlers[s].name.clone();
        let fond = alive.iter().filter(|&&j| j != s && self.opinion(s, j) >= 3).count();
        let why = format!("{} of them think well of {}, and {} has a way with people", fond, name, if self.settlers[s].persona.female { "she" } else { "he" });
        let line = match lost { Some(l) => format!("With {} gone, the camp chooses {} to speak for it.", l, name), None if self.alive() >= 10 => format!("The camp has grown to {}: they choose {} to speak for it.", self.alive(), name), None => format!("A month on, the camp chooses {} to speak for it.", name) };
        self.note(line.clone());
        let at = self.settlers[s].pos;
        self.moment(format!("{} speaks for the camp", name), line, format!("because {}", why), at);
    }

    /// The speaker's mandate for the season, from the value they hold dearest that has one.
    fn proclaim(&mut self, s: usize) {
        self.mandate_day = self.clock.day();
        let p = &self.settlers[s].persona;
        let mut vals: Vec<(i8, usize)> = (0..crate::persona::N_VALUES).map(|k| (p.values.get(k).copied().unwrap_or(0), k)).collect();
        vals.sort_by_key(|(v, k)| (std::cmp::Reverse(*v), *k));
        let all = [Val::Law, Val::Loyalty, Val::Family, Val::Friendship, Val::Power, Val::Truth, Val::Cunning, Val::Eloquence, Val::Fairness,
            Val::Decorum, Val::Tradition, Val::Artwork, Val::Cooperation, Val::Independence, Val::Stoicism, Val::Introspection,
            Val::SelfControl, Val::Tranquility, Val::Harmony, Val::Merriment, Val::Craftsmanship, Val::MartialProwess, Val::Skill,
            Val::HardWork, Val::Sacrifice, Val::Competition, Val::Perseverance, Val::Leisure, Val::Commerce, Val::Romance, Val::Nature,
            Val::Peace, Val::Knowledge];
        let Some((m, v)) = vals.iter().find_map(|&(_, k)| Mandate::from_value(all[k]).map(|m| (m, all[k]))) else { return };
        if self.mandate == Some(m) { return; }
        self.mandate = Some(m);
        let name = self.settlers[s].name.clone();
        let line = format!("{} proclaims that {}.", name, m.words());
        self.note(format!("{} ({} {}).", line.trim_end_matches('.'), if self.settlers[s].persona.female { "she" } else { "he" }, self.settlers[s].persona.agree(&format!("values {}", crate::persona::Persona::value_name(v).chars().fold(String::new(), |mut a, c| { if c.is_uppercase() && !a.is_empty() { a.push(' '); } a.extend(c.to_lowercase()); a })))));
        // Each takes it by their own values: those who hold its values welcome it, those who
        // scorn them resent the speaker for it.
        for j in 0..self.settlers.len() {
            if j == s || !self.settlers[j].alive { continue; }
            let lean: i32 = m.values().iter().map(|&x| self.settlers[j].persona.value(x) as i32).sum();
            if lean >= 26 { self.like(s, j, 2); } else if lean <= -26 { self.like(s, j, -3); self.feel(j, mind::Feel::Mandate { what: m.words().to_string() }); }
        }
    }

    /// After a quarrel, a speaker who feels what others feel makes peace between them.
    pub(crate) fn make_peace(&mut self, a: usize, b: usize) {
        let Some(s) = self.speaker.filter(|&s| s != a && s != b && self.settlers[s].alive) else { return };
        if self.settlers[s].persona.attr(Attr::Empathy) < 900.0 && self.settlers[s].persona.facet(Facet::Discord) > 50 { return; }
        self.like(a, b, 4);
        let (sn, an, bn) = (self.settlers[s].name.clone(), self.settlers[a].name.clone(), self.settlers[b].name.clone());
        self.note(format!("{}, who speaks for the camp, sits {} and {} down by the fire until they make peace.", sn, an, bn));
        for k in [a, b] { self.feel(k, mind::Feel::Reconciled { by: sn.clone() }); }
    }
}
