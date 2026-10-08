//! Guilds: masters of one trade who band together, and the hall they ask for.
//!
//! The idea is Dwarf Fortress's guilds (nested organizations inside a site's government): when
//! enough of a fortress's citizens share a craft, they form a guild and petition for a guildhall;
//! the answer shapes how they feel about those who rule. Here, from day 60, at dawn
//! (`reckon_guilds`), when four grown settlers (not guests) are masters (0.8+) of a trade that is
//! their best (foraging, fishing, felling or building; carrying is no craft), they form its guild
//! ("The Lodge of the Mallet", a moment); others who become its masters join it. Ten days after
//! forming, its best hand petitions the speaker (else the oldest) for a guildhall: the speaker
//! grants it by valuing craftsmanship and hard work and by liking the members (each member's
//! opinion of them counts), against valuing tradition over new bodies of men. Granted: a
//! guildhall (`ProjectKind::GuildHall`, 4x3 roofed, 12 loads, urgency 1.4), and the members like
//! the speaker better (+3). Refused: they resent it (-3, `Feel::Mandate`), and ask again sixty
//! days later. Once the hall stands members learn their trade half again as fast (`guild_learning`,
//! in `finish`), and at each season's turn they feast in it (+1 opinion among them).

use super::*;
use crate::persona::Val;

/// Carrying is no craft: no guild.
const CARRY: usize = 3;

/// What each trade's guild is called after.
const TOOLS: [&str; 5] = ["Basket", "Net", "Axe", "Yoke", "Mallet"];
const BODIES: [&str; 4] = ["Lodge", "Fellowship", "Brotherhood", "Company"];

#[derive(Clone, Debug)]
pub struct Guild {
    /// The trade (an index into `ROLES`).
    pub trade: usize,
    pub name: String,
    pub members: Vec<usize>,
    pub day: u64,
    /// The petition: when next to ask, and the answer once given (true: a hall).
    pub ask_day: u64,
    pub granted: Option<bool>,
}

impl Colony {
    /// Masters of trade `k` (0.8+), for whom it is their best trade.
    fn masters_of(&self, k: usize) -> Vec<usize> {
        (0..self.settlers.len()).filter(|&i| {
            let s = &self.settlers[i];
            let best = (0..ROLES.len()).filter(|&t| t != CARRY).max_by(|&a, &b| s.skill[a].total_cmp(&s.skill[b]).then(b.cmp(&a)));
            s.alive && !s.mind.left && s.guest_until == 0 && s.past.as_ref().map_or(true, |p| p.age >= 14) && s.skill[k] >= 0.8 && best == Some(k)
        }).collect()
    }

    /// Dawn: guilds form and grow; petitions are made and answered; feasts at the season's turn.
    pub(crate) fn reckon_guilds(&mut self) {
        let day = self.clock.day();
        if self.alive() == 0 || day < 60 { return; }
        for k in (0..ROLES.len()).filter(|&k| k != CARRY) {
            let masters = self.masters_of(k);
            match self.guilds.iter().position(|g| g.trade == k) {
                None if masters.len() >= 4 => {
                    let body = BODIES[(crate::history::settlers::hash_pub(self.seed, 0x6017 + k as u64) % BODIES.len() as u64) as usize];
                    let name = format!("The {} of the {}", body, TOOLS[k]);
                    let names: Vec<String> = masters.iter().map(|&i| self.settlers[i].name.clone()).collect();
                    let line = format!("{} swear themselves to one another as {}, the camp's {}s.", crate::persona::list(&names), name, ROLES[k]);
                    self.note(line.clone());
                    let at = self.camp;
                    self.moment(format!("{} is founded", name), line, format!("because {} of them have mastered the work of the {}", masters.len(), ROLES[k]), at);
                    for &i in &masters { for &j in &masters { if i < j { self.like(i, j, 2); } } }
                    self.guilds.push(guild::Guild { trade: k, name, members: masters, day, ask_day: day + 10, granted: None });
                }
                Some(g) => {
                    let new: Vec<usize> = masters.into_iter().filter(|i| !self.guilds[g].members.contains(i)).collect();
                    if !new.is_empty() {
                        let names: Vec<String> = new.iter().map(|&i| self.settlers[i].name.clone()).collect();
                        let gname = self.guilds[g].name.clone();
                        self.note(format!("{} {} taken into {}.", crate::persona::list(&names), if new.len() == 1 { "is" } else { "are" }, gname));
                        self.guilds[g].members.extend(new);
                    }
                }
                None => {}
            }
        }
        for g in 0..self.guilds.len() {
            let gd = self.guilds[g].clone();
            let members: Vec<usize> = gd.members.iter().copied().filter(|&i| self.settlers[i].alive).collect();
            if members.is_empty() { continue; }
            if gd.granted != Some(true) && day >= gd.ask_day { self.petition(g, &members); }
            // A feast in the hall at each season's turn.
            if gd.granted == Some(true) && day % SEASON_DAYS == 1 && self.guild_hall_of(gd.trade).is_some() && members.len() >= 2 {
                for &i in &members { for &j in &members { if i < j { self.warm(i, j); } } }
                self.note(format!("{} feast in their hall at the turn of the season.", gd.name));
            }
        }
    }

    /// A guild's petition for a hall.
    fn petition(&mut self, g: usize, members: &[usize]) {
        let day = self.clock.day();
        let alive: Vec<usize> = (0..self.settlers.len()).filter(|&i| self.settlers[i].alive && self.settlers[i].guest_until == 0).collect();
        let Some(&judge) = self.speaker.filter(|&s| self.settlers[s].alive).as_ref()
            .or_else(|| alive.iter().max_by_key(|&&i| (self.settlers[i].past.as_ref().map_or(30, |p| p.age), std::cmp::Reverse(i)))) else { return };
        let gd = self.guilds[g].clone();
        let k = gd.trade;
        // The best hand asks; not the one who answers.
        let Some(best) = members.iter().copied().filter(|&m| m != judge).max_by(|&a, &b| self.settlers[a].skill[k].total_cmp(&self.settlers[b].skill[k]).then(b.cmp(&a))) else { return };
        let p = &self.settlers[judge].persona;
        let liking: i32 = members.iter().map(|&m| self.opinion(judge, m)).sum::<i32>() / members.len() as i32;
        let lean = p.value(Val::Craftsmanship) as i32 + p.value(Val::HardWork) as i32 - p.value(Val::Tradition) as i32 / 2 + 3 * liking
            + if members.contains(&judge) { 15 } else { 0 };
        let (jn, bn) = (self.settlers[judge].name.clone(), self.settlers[best].name.clone());
        let they = if p.female { "she" } else { "he" };
        if lean >= 0 {
            self.guilds[g].granted = Some(true);
            let why = if members.contains(&judge) { format!("{} is one of them", they) }
                else if p.value(Val::Craftsmanship) >= p.value(Val::HardWork) { format!("{} honours good work", they) } else { format!("{} honours hard work", they) };
            let line = format!("{} asks {} for a hall for {}; {} grants it, because {}.", bn, jn, gd.name, jn, why);
            self.note(line.clone());
            let at = self.camp;
            self.moment(format!("A hall for {}", gd.name), line, format!("because {} of the camp's {}s are sworn to it", members.len(), ROLES[k]), at);
            for &m in members { if m != judge { self.like(m, judge, 3); } }
            if let Some(at) = self.find_site_pub(4, 3) {
                self.projects.push(projects::Project { kind: projects::ProjectKind::GuildHall, at, needed: 12, used: 0, material: self.hut_material,
                    why: format!("{} were granted a hall of their own: {} of the camp's {}s are sworn to it", gd.name, members.len(), ROLES[k]), done: false, day });
            }
        } else {
            self.guilds[g].granted = Some(false);
            self.guilds[g].ask_day = day + 60;
            let why = if p.value(Val::Tradition) > 0 { format!("{} holds that the camp is one fire, not many", they) } else { format!("{} has no love for them", they) };
            self.note(format!("{} asks {} for a hall for {}; {} refuses, because {}.", bn, jn, gd.name, jn, why));
            for &m in members { if m != judge { self.like(m, judge, -3); self.feel(m, mind::Feel::Mandate { what: format!("{} be refused a hall", gd.name) }); } }
        }
    }

    /// The guildhall of a trade, if it stands.
    fn guild_hall_of(&self, trade: usize) -> Option<Pos> {
        let name = &self.guilds.iter().find(|g| g.trade == trade)?.name;
        self.projects.iter().find(|p| p.done && p.kind == projects::ProjectKind::GuildHall && p.why.starts_with(name.as_str())).map(|p| p.at)
    }

    /// How much faster settler `i` learns trade `k`: half again in a guild with a hall.
    pub(crate) fn guild_learning(&self, i: usize, k: usize) -> f32 {
        if self.guilds.iter().any(|g| g.trade == k && g.granted == Some(true) && g.members.contains(&i)) && self.guild_hall_of(k).is_some() { 1.5 } else { 1.0 }
    }

    /// Settler `i`'s guild, for their page and the annals.
    pub fn guild_of(&self, i: usize) -> Option<String> {
        self.guilds.iter().find(|g| g.members.contains(&i)).map(|g| format!("Sworn to {}{}", g.name, match g.granted { Some(true) => ", which has its hall", Some(false) => ", refused a hall", None => "" }))
    }
}
