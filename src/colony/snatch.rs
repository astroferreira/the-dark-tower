//! Snatchers: children stolen in the night, and one who comes back among the raiders.
//!
//! The idea is Dwarf Fortress's baby-snatchers: goblins slip into a fortress and carry off a child,
//! who is raised among them and may return years later as one of their soldiers in a siege. Here,
//! at founding (`viewer::found_colony`), the peoples among the camp's troubles whose race steals
//! children (goblins and orcs) are noted (`Colony::snatchers`), and the Shadow's raiders steal them
//! too (DF's goblins serve a demon); `PLANET_FORCE_SNATCH=1` makes every war band snatch and lets
//! any of them bring a child back. On the night such raiders come (`snatch_in_the_raid`, from
//! `raid_at`), one time in two a child of the camp (an infant to 11, born here or come with the
//! founders) is carried off in the confusion: unless the watcher is within six cells, who drives
//! the snatcher off ("X wakes to a stranger at Y's bed"). Taken, the child is gone from the camp
//! (`alive` false, `Colony::snatched`), a moment; the parents grieve as for a death and everyone
//! hears of it. Sixty days later the same raiders come again (`reckon_snatched` puts them first
//! among the troubles, "they came before and carried off X"), and the child is among them
//! (`snatched_return`). A small one (under six) rides on a raider's back and is pulled free only if
//! the camp routs the band. An older one stands at the edge of the firelight: if a parent or the
//! one who cared most lives, they call the child's name, and 65% (45% after 120 days) the child
//! drops the spear and walks to the fire (a moment, older by the years gone); else the child runs
//! with the raiders, and after a second refusal is "one of them now". A raid that brings one back
//! takes no other, and two are taken at most.

use super::*;
use crate::history::FactionId;

#[derive(Clone, Debug)]
pub struct Snatched {
    pub who: usize,
    pub people: String,
    pub faction: Option<FactionId>,
    pub day: u64,
    /// Seen among the raiders (times), and whether they came home.
    pub seen: u32,
    pub home: Option<u64>,
    /// The raiders who took them, to come again (`reckon_snatched`), and whether they are on
    /// their way.
    pub raiders: arc::Threat,
    pub coming: bool,
}

impl Colony {
    /// Whether a war band of this people steals children.
    fn snatches(&self, threat: &arc::Threat) -> bool {
        threat.kind == arc::ThreatKind::Shadow
            || (threat.kind == arc::ThreatKind::Warband && (threat.faction.map_or(false, |f| self.snatchers.contains(&f)) || forced()))
    }

    /// The raid's night: a child may be carried off, or one taken before may come back.
    pub(crate) fn snatch_in_the_raid(&mut self, threat: &arc::Threat, routed: bool, seed: u64) {
        if !self.snatches(threat) { return; }
        let people = threat.name.trim_start_matches("a war band of ").split(", led by ").next().unwrap_or("").to_string();
        let people = if people.starts_with("raiders of ") { format!("the {}", people) } else { people };
        // A raid that brings one back takes no other; two taken in all at most.
        if self.snatched_return(threat, &people, routed, seed) || self.snatched.len() >= 2 { return; }
        // The bell kept every child indoors (`ring_bell`).
        if self.bell_rung() { self.note(format!("The children were kept indoors at the bell: {} find no small one to carry off.", people)); return; }
        if crate::history::settlers::hash_pub(seed, 0x5A7C) % 2 != 0 { return; }
        let kids: Vec<usize> = (0..self.settlers.len()).filter(|&i| {
            let s = &self.settlers[i];
            s.alive && s.guest_until == 0 && s.past.as_ref().map_or(false, |p| p.age <= 11)
        }).collect();
        if kids.is_empty() { return; }
        let c = kids[(crate::history::settlers::hash_pub(seed, 0x5A7D) as usize) % kids.len()];
        let cn = self.settlers[c].name.clone();
        let cp = self.settlers[c].pos;
        let near = |p: Pos| (p.0 as i32 - cp.0 as i32).abs().max((p.1 as i32 - cp.1 as i32).abs());
        if let Some(w) = self.watcher.filter(|&w| self.settlers[w].alive && near(self.settlers[w].pos) <= 6) {
            let wn = self.settlers[w].name.clone();
            self.note(format!("{} hears something at {}'s bed and comes running: a stranger slips away into the dark with empty hands.", wn, cn));
            self.like(c, w, 4);
            return;
        }
        let day = self.clock.day();
        self.settlers[c].alive = false;
        self.settlers[c].job = Job::Idle;
        self.settlers[c].path.clear();
        self.snatched.push(snatch::Snatched { who: c, people: people.clone(), faction: threat.faction, day, seen: 0, home: None, raiders: threat.clone(), coming: false });
        let line = format!("In the confusion of the raid, {} is gone from {} bed: small tracks and larger ones lead off into the dark after {}.", cn, if self.settlers[c].persona.female { "her" } else { "his" }, people);
        self.note(line.clone());
        self.moment(format!("{} is taken", cn), line, format!("because {} take children to raise as their own", people), cp);
        let kin = self.kin_of(c);
        for j in 0..self.settlers.len() {
            if !self.settlers[j].alive || j == c { continue; }
            if kin.contains(&j) { self.feel(j, mind::Feel::Death { whom: cn.clone(), close: true }); }
            else { self.feel(j, mind::Feel::News { what: format!("{} was carried off by {}", cn, people), good: false }); }
        }
    }

    /// Dawn: sixty days after a child was taken (and sixty after each sighting), the raiders who
    /// took them come again, if no thread of theirs is already coming.
    pub(crate) fn reckon_snatched(&mut self) {
        let day = self.clock.day();
        for k in 0..self.snatched.len() {
            let s = self.snatched[k].clone();
            if s.home.is_some() || s.coming || s.seen >= 2 || day < s.day + 60 * (1 + s.seen as u64) || self.alive() == 0 || self.departed.is_some() { continue; }
            let Some(a) = self.arc.as_mut() else { return };
            if a.later.iter().any(|t| t.faction == s.faction && t.kind == s.raiders.kind) { self.snatched[k].coming = true; continue; }
            let mut t = s.raiders.clone();
            t.why = format!("they came before and carried off {} on day {}", self.settlers[s.who].name, s.day);
            t.cause_text = Some(format!("{} were carried off from the camp on day {}", self.settlers[s.who].name, s.day).replacen(" were ", " was ", 1));
            a.later.insert(0, t);
            if a.stage == 3 { a.quiet_until = Some(a.quiet_until.map_or(day + 8, |q| q.min(day + 8))); }
            self.snatched[k].coming = true;
        }
    }

    /// A child's parents (or, for a founder's child, the one who thinks best of them).
    fn kin_of(&self, c: usize) -> Vec<usize> {
        let mut v: Vec<usize> = self.children.iter().filter(|x| x.0 == c).flat_map(|x| [x.1, x.2]).collect();
        if v.is_empty() {
            if let Some(b) = (0..self.settlers.len()).filter(|&j| j != c && self.settlers[j].alive).max_by_key(|&j| (self.opinion(j, c), std::cmp::Reverse(j))) { v.push(b); }
        }
        v
    }

    /// The same people raids again: a child taken is among them. Returns whether one was.
    fn snatched_return(&mut self, threat: &arc::Threat, people: &str, routed: bool, seed: u64) -> bool {
        let day = self.clock.day();
        let Some(k) = self.snatched.iter().position(|s| s.home.is_none() && s.seen < 2 && (s.faction == threat.faction || forced()) && day >= s.day + 60) else { return false };
        let s = self.snatched[k].clone();
        let c = s.who;
        let cn = self.settlers[c].name.clone();
        let years = ((day - s.day) / ageing::YEAR_DAYS) as u32;
        let age = self.settlers[c].past.as_ref().map_or(8, |p| p.age) + years;
        let caller = self.kin_of(c).into_iter().find(|&j| self.settlers[j].alive);
        self.snatched[k].seen += 1;
        self.snatched[k].coming = false;
        let at = self.camp;
        let home = |col: &mut Colony| {
            col.snatched[k].home = Some(day);
            let st = &mut col.settlers[c];
            st.alive = true;
            st.pos = col.camp;
            st.path.clear();
            st.job = Job::Idle;
            st.hunger = st.hunger.min(0.6);
            if let Some(past) = st.past.as_mut() { past.age += years; }
        };
        // A small one is carried: only a rout wins them back.
        if age < 6 {
            let first = format!("On a raider's back, wrapped in furs, is a small one: {}, taken from this camp on day {}.", cn, s.day);
            if routed {
                let by = caller.map(|p| self.settlers[p].name.clone()).unwrap_or_else(|| "the camp".into());
                home(self);
                let line = format!("{} As the raiders break, {} pulls {} free: home after {} days among {}.", first, by, cn, day - s.day, people);
                self.note(line.clone());
                self.moment(format!("{} comes home", cn), line, "because the camp broke the band that carried them".into(), at);
                if let Some(p) = caller { self.like(c, p, 10); self.feel(p, mind::Feel::News { what: format!("{} came home from {}", cn, people), good: true }); }
            } else {
                self.note(format!("{} No one can reach {} before the raiders are gone into the dark again.", first, cn));
                if let Some(p) = caller { self.feel(p, mind::Feel::Death { whom: cn.clone(), close: true }); }
            }
            return true;
        }
        let first = format!("Among the raiders is a young one who stops at the edge of the firelight: it is {}, taken from this camp on day {}, in the gear of {}.", cn, s.day, people);
        let Some(p) = caller else {
            self.note(format!("{} No one is left who would call the name, and {} goes back into the dark with them.", first, cn));
            self.snatched[k].seen = 2;
            return true;
        };
        let pn = self.settlers[p].name.clone();
        // Taken lately, the camp is still home; long gone, less so.
        let chance = if day - s.day < 120 { 65 } else { 45 };
        if crate::history::settlers::hash_pub(seed ^ s.seen as u64, 0x5A7E) % 100 < chance {
            home(self);
            let line = format!("{} {} calls {}'s name across the fire, and {} drops the spear and walks to the fire, home after {} days among {}.", first, pn, cn, cn, day - s.day, people);
            self.note(line.clone());
            self.moment(format!("{} comes home", cn), line, format!("because {} called {} by name", pn, cn), at);
            self.like(c, p, 10);
            self.feel(p, mind::Feel::News { what: format!("{} came home from {}", cn, people), good: true });
            self.feel(c, mind::Feel::Torn { people: people.to_string() });
        } else {
            let lost = s.seen + 1 >= 2;
            let line = format!("{} {} calls {}'s name, but {} turns and runs with the raiders into the dark{}.", first, pn, cn, cn, if lost { &", one of them now"[..] } else { "" });
            self.note(line.clone());
            self.moment(format!("{} among the raiders", cn), line, format!("because {} took {} on day {} and raised them as their own", people, cn, s.day), at);
            self.feel(p, mind::Feel::Death { whom: cn.clone(), close: true });
        }
        true
    }
}

/// `PLANET_FORCE_SNATCH=1`: every war band steals children, and any brings one back (for tests).
fn forced() -> bool { std::env::var("PLANET_FORCE_SNATCH").is_ok() }
