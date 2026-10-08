//! A lost thing of the history: an artifact lost near the camp, sought, found, and claimed back.
//!
//! The idea is Dwarf Fortress's artifacts as story carriers: one named object made, inherited,
//! lost, found, claimed and fought over by different systems, tying unrelated lives together.
//! At founding (`lost_near`) the nearest artifact the history lost (its owner died with no heir)
//! within six tiles is placed on the embark: on the surface 35-70 cells from the fire, or, where
//! the ground has caverns, in the dark below (found only when the mine breaks in). Settlers whose
//! people owned it know the tale from the start; the others hear it from the first caravan.
//! Those who know it and are curious or greedy search by day, the circle narrowing as days of
//! searching pass; anyone passing within two cells finds it. Found, it is a moment and kept in
//! the temple or the hut. If the people who owned it still stand, word reaches them and 12-20
//! days later an envoy asks for it back: the speaker (else the oldest) decides by fairness, law,
//! tradition, altruism against greed, and kinship with the owners. Given back, the envoy leaves
//! a gift; kept, a war band comes for it as the next chapter, and if the raid is not routed they
//! carry it off.

use super::*;
use crate::history::world_state::WorldHistory;
use crate::history::{EntityId, EventId, FactionId};
use crate::history::events::types::EventType;
use crate::persona::{Facet, Val};

#[derive(Clone, Debug)]
pub struct Relic {
    /// The history's artifact, and the figure who held it last.
    pub artifact: crate::history::ArtifactId,
    pub holder: Option<crate::history::FigureId>,
    /// "Grimjaw's Wrath".
    pub name: String,
    /// "a legendary sword".
    pub what: String,
    /// "lost in 412 after the death of Ilda the Bold".
    pub tale: String,
    pub cause: Option<EventId>,
    /// The people who owned it last, and their name.
    pub owners: Option<FactionId>,
    pub owners_name: String,
    pub owners_iron: bool,
    /// Where it lies, and whether in the dark below.
    pub at: Pos,
    pub below: bool,
    /// Who of the camp knows the tale (by index); everyone once the caravan has told it.
    pub known: bool,
    pub search_days: u32,
    /// Found: by whom and on which day.
    pub found: Option<(String, u64)>,
    /// When the envoy comes for it.
    pub envoy_day: Option<u64>,
    /// Refused: the war band coming is for it.
    pub contested: bool,
    /// "given back to the envoy of X on day 40", "carried off by ... on day 55".
    pub fate: Option<String>,
    /// Its seeker (`visitors.rs`) has asked for it.
    pub asked: bool,
}

fn dist(a: (usize, usize), b: (usize, usize), w: usize) -> usize {
    let dx = a.0.abs_diff(b.0);
    dx.min(w.saturating_sub(dx)) + a.1.abs_diff(b.1)
}

fn type_word(t: crate::history::objects::artifacts::ArtifactType) -> &'static str {
    use crate::history::objects::artifacts::ArtifactType as T;
    match t {
        T::Weapon => "blade", T::Armor => "mail", T::Crown => "crown", T::Ring => "ring", T::Amulet => "amulet",
        T::Staff => "staff", T::Book => "book", T::Goblet => "goblet", T::Instrument => "instrument", T::Relic => "relic",
    }
}

fn quality_word(q: crate::history::objects::artifacts::ArtifactQuality) -> &'static str {
    use crate::history::objects::artifacts::ArtifactQuality as Q;
    match q { Q::Fine => "fine", Q::Superior => "superior", Q::Masterwork => "masterwork", Q::Legendary => "legendary", Q::Divine => "god-touched" }
}

/// "a superior staff", "an ancient legendary blade".
pub fn describe(a: &crate::history::objects::artifacts::Artifact) -> String {
    let what = format!("{} {}", quality_word(a.quality), type_word(a.item_type));
    format!("{} {}", if what.starts_with(|c: char| "aeiou".contains(c)) { "an" } else { "a" }, what)
}

/// The nearest artifact the history lost within six tiles of `tile`, as a relic not yet placed.
pub fn lost_near(h: &WorldHistory, tile: (usize, usize)) -> Option<Relic> {
    let w = h.tile_history.width.max(1);
    let mut best: Option<(usize, Relic)> = None;
    let mut arts: Vec<_> = h.artifacts.values().filter(|a| a.lost && !a.destroyed).collect();
    arts.sort_by_key(|a| a.id);
    for a in arts {
        // The loss, and the last place its last holder was seen.
        let lost = h.chronicle.events.iter().rev().find(|e| e.event_type == EventType::ArtifactLost && e.primary_participants.contains(&EntityId::Artifact(a.id)));
        let holder = lost.and_then(|e| e.primary_participants.iter().find_map(|p| match p { EntityId::Figure(f) => Some(*f), _ => None }));
        let place = lost.and_then(|e| e.location).or_else(|| {
            let f = holder?;
            let before = lost.map(|e| e.date);
            h.chronicle.events.iter().rev().filter(|e| before.map_or(true, |d| e.date <= d))
                .find(|e| e.location.is_some() && e.primary_participants.contains(&EntityId::Figure(f))).and_then(|e| e.location)
        }).or(a.creation_location);
        let Some(place) = place else { continue };
        let d = dist(place, tile, w);
        if d > 6 || best.as_ref().map_or(false, |(bd, _)| *bd <= d) { continue; }
        // Its last owners: a people, or a person's people.
        let owner = a.owner_history.last().map(|o| o.0.clone());
        let owners = match owner {
            Some(EntityId::Faction(f)) => Some(f),
            Some(EntityId::Figure(f)) => h.figures.get(&f).and_then(|f| f.faction),
            _ => None,
        }.filter(|f| h.factions.get(f).map_or(false, |x| x.is_active()));
        let owners_name = owners.and_then(|f| h.factions.get(&f)).map(|f| f.name.clone()).unwrap_or_default();
        let owners_iron = owners.and_then(|f| h.factions.get(&f)).map_or(false, |fac| {
            use crate::history::civilizations::economy::ResourceType as R;
            fac.resources.get(&R::Iron).copied().unwrap_or(0) > 0 || fac.resources.get(&R::Copper).copied().unwrap_or(0) > 0
        });
        let holder_name = holder.and_then(|f| h.figures.get(&f)).map(|f| f.full_name());
        let year = lost.map(|e| e.date.year);
        let tale = match (year, holder_name) {
            (Some(y), Some(n)) => format!("lost in {} after the death of {}", y, n),
            (Some(y), None) => format!("lost in {}", y),
            _ => "lost long ago".into(),
        };
        let what = describe(a);
        best = Some((d, Relic { artifact: a.id, holder, asked: false, name: a.name.clone(), what, tale, cause: lost.map(|e| e.id), owners, owners_name, owners_iron,
            at: (0, 0), below: false, known: false, search_days: 0, found: None, envoy_day: None, contested: false, fate: None }));
    }
    best.map(|(_, r)| r)
}

impl Colony {
    /// Lay the relic on the map: in the dark below where there are caverns (half the time), else
    /// on dry ground 35-70 cells from the fire. Those of its owners' people know the tale.
    pub fn place_relic(&mut self, mut r: Relic) {
        let roll = crate::history::settlers::hash_pub(self.seed, 0xA471);
        if !self.map.caverns.is_empty() && roll % 2 == 0 {
            r.below = true;
        } else {
            let mut placed = None;
            for k in 0..24u64 {
                let a = ((roll >> 8).wrapping_add(k * 97) % 628) as f32 / 100.0;
                let d = 35.0 + ((roll >> 20).wrapping_add(k * 13) % 36) as f32;
                let p = ((self.camp.0 as f32 + a.cos() * d) as i32, (self.camp.1 as f32 + a.sin() * d) as i32);
                if let Some(c) = self.passable_near_pub(p) { placed = Some(c); break; }
            }
            let Some(at) = placed else { return };
            r.at = at;
        }
        r.known = r.owners.is_some() && self.settlers.iter().any(|s| s.past.as_ref().and_then(|p| p.people) == r.owners);
        if r.known {
            let k = self.settlers.iter().find(|s| s.past.as_ref().and_then(|p| p.people) == r.owners).map(|s| s.name.clone()).unwrap_or_default();
            self.note(format!("{} tells by the fire of {}, {} of {}, {}, somewhere near here{}.", k, r.name, r.what, r.owners_name, r.tale, if r.below { ", and some say under the ground" } else { "" }));
        }
        self.relic = Some(r);
    }

    /// The caravan tells the tale, if no one knew it.
    pub(crate) fn relic_told(&mut self, by: &str) {
        let Some(r) = self.relic.as_mut().filter(|r| !r.known && r.found.is_none()) else { return };
        r.known = true;
        let line = format!("The traders from {} tell of {}, {}{}, {}, somewhere near here.", by, r.name, r.what, if r.owners_name.is_empty() { String::new() } else { format!(" of {}", r.owners_name) }, r.tale);
        self.note(line);
    }

    /// Who searches, and where today: the curious and the greedy who know the tale, by day.
    pub(crate) fn relic_option(&self, i: usize) -> Option<(f32, Job, String)> {
        let r = self.relic.as_ref().filter(|r| r.known && r.found.is_none() && !r.below)?;
        if self.clock.is_night() { return None; }
        let p = &self.settlers[i].persona;
        let seeker = self.seeker_of_relic() == Some(i);
        let lust = if seeker { 1.0 } else { (p.facet(Facet::Curiosity).max(p.facet(Facet::Greed)) as f32 - 50.0) / 50.0 };
        if lust <= 0.0 { return None; }
        // The circle narrows as the days of searching add up.
        let radius = (16i32 - r.search_days as i32 / 2).max(3);
        let h = crate::history::settlers::hash_pub(self.seed ^ self.clock.day(), 0x5EA2 + i as u64);
        let dx = (h % (2 * radius as u64 + 1)) as i32 - radius;
        let dy = ((h >> 16) % (2 * radius as u64 + 1)) as i32 - radius;
        let spot = self.passable_near_pub((r.at.0 as i32 + dx, r.at.1 as i32 + dy))?;
        Some((0.2 + 0.35 * lust, Job::Wander(spot), format!("Searching the ground for {}, {}", r.name, r.tale)))
    }

    /// Every ten minutes: anyone within two cells finds it; at dawn the days of searching count;
    /// at 10:00 the envoy comes.
    pub(crate) fn relic_tick(&mut self) {
        let Some(r) = self.relic.clone() else { return };
        if r.found.is_none() && !r.below {
            if self.clock.minute() % 10 == 0 {
                let finder = (0..self.settlers.len()).find(|&i| self.settlers[i].alive
                    && (self.settlers[i].pos.0 as i32 - r.at.0 as i32).abs().max((self.settlers[i].pos.1 as i32 - r.at.1 as i32).abs()) <= 2);
                if let Some(i) = finder { self.find_relic(i, "in the earth"); }
            }
            if self.clock.hour() == 6 && self.clock.minute() == 0 && r.known {
                let searched = self.settlers.iter().any(|s| s.alive && s.why.starts_with("Searching the ground for"));
                if searched || r.search_days > 0 { if let Some(x) = self.relic.as_mut() { x.search_days += 1; } }
            }
        }
        if r.envoy_day == Some(self.clock.day()) && self.clock.hour() == 10 && self.clock.minute() == 0 && self.alive() > 0 {
            match &r.fate {
                None => self.relic_envoy(),
                Some(f) => self.note(format!("An envoy of {} comes for {}, but it is gone: {}. They leave empty-handed.", r.owners_name, r.name, f)),
            }
        }
        if self.clock.hour() == 10 && self.clock.minute() == 0 && r.found.is_some() && r.fate.is_none() && !r.asked { self.seeker_asks(); }
        if self.clock.hour() == 23 && self.clock.minute() == 0 && r.fate.is_none() { self.seeker_steals(); }
    }

    /// The mine broke into the dark: if the relic lies below, the miner finds it.
    pub(crate) fn relic_below(&mut self, miner: usize) {
        if self.relic.as_ref().map_or(false, |r| r.below && r.found.is_none()) { self.find_relic(miner, "on the cavern floor, where the lamplight fell"); }
    }

    fn find_relic(&mut self, i: usize, where_: &str) {
        let day = self.clock.day();
        let name = self.settlers[i].name.clone();
        let kept = match self.temple() { Some((_, god)) => format!("They keep it in the temple of {}", god), None => "They keep it in the hut, wrapped in hide".into() };
        let Some(r) = self.relic.as_mut() else { return };
        r.found = Some((name.clone(), day));
        // Found, its tale is known (it was lost here, and the finder tells of it).
        r.known = true;
        let (rname, what, tale, owners) = (r.name.clone(), r.what.clone(), r.tale.clone(), r.owners);
        // Word reaches its owners' people.
        if owners.is_some() { r.envoy_day = Some(day + 12 + crate::history::settlers::hash_pub(self.seed, 0xE4F0) % 9); }
        let line = format!("{} finds {} {}: {}, {}. {}.", name, rname, where_, what, tale, kept);
        self.note(line.clone());
        let at = self.settlers[i].pos;
        self.moment(format!("{} is found", rname), line, format!("because {} was {} near here", rname, tale), at);
        self.feel(i, mind::Feel::Found { what: rname.clone() });
        for j in 0..self.settlers.len() { if j != i && self.settlers[j].alive { self.feel(j, mind::Feel::Admired { what: rname.clone() }); } }
    }

    /// The envoy asks for it back; the speaker (else the oldest) decides.
    fn relic_envoy(&mut self) {
        let Some(r) = self.relic.clone() else { return };
        let alive: Vec<usize> = (0..self.settlers.len()).filter(|&i| self.settlers[i].alive && !self.settlers[i].mind.left).collect();
        let Some(&judge) = self.speaker.filter(|&s| self.settlers[s].alive).as_ref()
            .or_else(|| alive.iter().max_by_key(|&&i| (self.settlers[i].past.as_ref().map_or(30, |p| p.age), std::cmp::Reverse(i))))
        else { return };
        let p = &self.settlers[judge].persona;
        let kin = self.settlers[judge].past.as_ref().and_then(|x| x.people) == r.owners;
        let lean = (p.value(Val::Fairness) + p.value(Val::Law) + p.value(Val::Tradition)) as f32 / 3.0
            + (p.facet(Facet::Altruism) as f32 - p.facet(Facet::Greed) as f32) / 2.0 + if kin { 40.0 } else { 0.0 };
        let jn = self.settlers[judge].name.clone();
        let they = if p.female { "she" } else { "he" };
        let why = if kin { format!("{} is of {} {}self", they, r.owners_name, if p.female { "her" } else { "him" }) }
            else if lean > 0.0 { format!("{} {}", they, if p.value(Val::Fairness) >= p.value(Val::Law) { "holds that what was theirs is theirs" } else { "will not keep what the law gives to others" }) }
            else { format!("{} {}", they, if p.facet(Facet::Greed) >= 60 { "covets it" } else { "says what the earth gave up belongs to whoever dug it out" }) };
        self.note(format!("An envoy of {} comes to the camp: word has reached them that {} was found here, and they ask for it back.", r.owners_name, r.name));
        let day = self.clock.day();
        if lean > 0.0 {
            // Given back: a gift in return.
            let gift = if r.owners_iron && !self.tools_bought { self.tools_bought = true; "iron tools".to_string() } else {
                let n = 2 * self.alive() as u32;
                for _ in 0..n { self.items.push(Item::food(Stuff::Provisions, self.camp, true)); }
                format!("{} meals", n)
            };
            let line = format!("{} gives {} back to the envoy of {}, because {}. In thanks they leave {}.", jn, r.name, r.owners_name, why, gift);
            self.note(line.clone());
            let at = self.camp;
            self.moment(format!("{} goes home", r.name), line, format!("because {} found it, and {} had lost it {}", r.found.as_ref().map(|f| f.0.clone()).unwrap_or_default(), r.owners_name, r.tale.trim_start_matches("lost ")), at);
            for &j in &alive { if self.settlers[j].persona.facet(Facet::Greed) >= 76 { self.feel(j, mind::Feel::Mandate { what: format!("{} be given away", r.name) }); } }
            if let Some(x) = self.relic.as_mut() { x.fate = Some(format!("given back to the envoy of {} on day {}", r.owners_name, day)); }
            self.regard(r.owners, &r.owners_name, None, "relic", 25, format!("gave {} back to them on day {}", r.name, day));
        } else {
            let line = format!("{} will not give {} back, because {}. The envoy leaves without a word; they will come for it.", jn, r.name, why);
            self.note(line.clone());
            self.moment(format!("{} is kept", r.name), line, format!("because the camp found {} and {} want it back", r.name, r.owners_name), self.camp);
            if let Some(x) = self.relic.as_mut() { x.contested = true; }
            self.regard(r.owners, &r.owners_name, None, "relic", -15, format!("would not give {} back to them", r.name));
            // A war band comes for it: the next chapter.
            if let Some(a) = self.arc.as_mut() {
                let from = a.later.iter().chain(a.reserve.iter()).find(|t| t.faction == r.owners).and_then(|t| t.from).or(a.threat.from);
                a.later.insert(0, arc::Threat { kind: arc::ThreatKind::Warband, name: format!("a war band of {}", r.owners_name),
                    why: format!("the camp would not give back {}", r.name), cause: r.cause, cause_text: Some(format!("{} was {}", r.name, r.tale)),
                    faction: r.owners, from, size: 1.0, monster: None });
                a.quiet_until = Some(day + 4);
            }
        }
    }

    /// After a raid by those who want it: unless routed, they carry it off.
    pub(crate) fn relic_after_raid(&mut self, faction: Option<FactionId>, routed: bool, who: &str) {
        let Some(r) = self.relic.clone().filter(|r| r.contested && r.fate.is_none() && r.owners == faction && faction.is_some()) else { return };
        let day = self.clock.day();
        if routed {
            self.note(format!("{} stays in the camp: the war band went away with nothing.", r.name));
        } else {
            self.note(format!("In the dark they found what they came for: {} carry off {}.", who, r.name));
            if let Some(x) = self.relic.as_mut() { x.fate = Some(format!("carried off by {} on day {}", who, day)); }
        }
        if let Some(x) = self.relic.as_mut() { x.contested = false; }
    }

    /// One line for the annals and the HUD.
    pub fn relic_line(&self) -> Option<String> {
        let r = self.relic.as_ref()?;
        Some(match (&r.found, &r.fate) {
            (None, _) if r.known => format!("{} lies somewhere near, {}", r.name, r.tale),
            (None, _) => return None,
            (Some((who, d)), None) => format!("{}, {}, found by {} on day {}, is kept in the camp", r.name, r.what, who, d),
            (Some((who, d)), Some(f)) => format!("{}, {}, found by {} on day {}, {}", r.name, r.what, who, d, f),
        })
    }

    /// The guest who came seeking the relic, if one is here.
    pub fn seeker_of_relic(&self) -> Option<usize> {
        (0..self.settlers.len()).find(|&i| self.settlers[i].alive && self.settlers[i].guest_until > 0 && self.settlers[i].visitor.as_deref().map_or(false, |v| v.starts_with("a seeker") || v.starts_with("an heir")))
    }

    /// 10:00: the seeker asks for the found relic; the speaker (else the oldest) decides.
    fn seeker_asks(&mut self) {
        let Some(k) = self.seeker_of_relic() else { return };
        let Some(r) = self.relic.clone() else { return };
        if let Some(x) = self.relic.as_mut() { x.asked = true; }
        let alive: Vec<usize> = (0..self.settlers.len()).filter(|&i| self.settlers[i].alive && self.settlers[i].guest_until == 0).collect();
        let Some(&judge) = self.speaker.filter(|&s| self.settlers[s].alive).as_ref()
            .or_else(|| alive.iter().max_by_key(|&&i| (self.settlers[i].past.as_ref().map_or(30, |p| p.age), std::cmp::Reverse(i)))) else { return };
        let kn = self.settlers[k].name.clone();
        let p = &self.settlers[judge].persona;
        let kin = self.settlers[judge].past.as_ref().and_then(|x| x.people).is_some() && self.settlers[judge].past.as_ref().and_then(|x| x.people) == self.settlers[k].past.as_ref().and_then(|x| x.people);
        let liked = self.opinion(k, judge);
        let lean = (p.value(Val::Fairness) + p.value(Val::Tradition)) as f32 / 2.0 + (p.facet(Facet::Altruism) as f32 - p.facet(Facet::Greed) as f32) / 2.0
            + if kin { 30.0 } else { 0.0 } + 2.0 * liked as f32 - if r.owners.is_some() { 15.0 } else { 0.0 };
        let jn = self.settlers[judge].name.clone();
        let since = self.visitors.iter().find(|v| v.name == kn).map(|v| v.why.clone()).unwrap_or_default();
        self.note(format!("{} asks {} for {}, the thing {} came for.", kn, jn, r.name, if self.settlers[k].persona.female { "she" } else { "he" }));
        let day = self.clock.day();
        if lean > 0.0 {
            let line = format!("{} gives {} to {}, who has sought it so long{}.", jn, r.name, kn, if kin { ", one of their own people" } else { "" });
            self.note(line.clone());
            let at = self.settlers[k].pos;
            self.moment(format!("{} goes with {}", r.name, kn), line, format!("because {} {}", kn, since), at);
            if let Some(x) = self.relic.as_mut() { x.fate = Some(format!("given to {}, who had sought it, on day {}", kn, day)); }
            self.like(k, judge, 6);
            self.settlers[k].guest_until = self.clock.tick + TICKS_PER_DAY - 60;
            self.settlers[k].deeds.push(format!("was given {} on day {}", r.name, day));
        } else {
            let line = format!("{} will not give {} to {}.", jn, r.name, kn);
            self.note(line);
            self.like(k, judge, -6);
            self.feel(k, mind::Feel::Mandate { what: format!("{} be kept from them", r.name) });
            // A cunning or greedy seeker means to take it anyway: tonight.
            let sp = &self.settlers[k].persona;
            if sp.facet(Facet::Greed) >= 60 || sp.value(Val::Cunning) >= 15 || sp.value(Val::Law) <= -15 {
                self.settlers[k].guest_until = self.clock.tick + TICKS_PER_DAY - 60;
                self.seeker_night = Some(day);
            } else {
                self.settlers[k].guest_until = self.clock.tick + TICKS_PER_DAY - 60;
            }
        }
    }

    /// 23:00 the night a refused seeker means to steal it: seen by someone awake near, they are
    /// caught (a crime, judged at dawn); unseen, they and the relic are gone by morning.
    fn seeker_steals(&mut self) {
        if self.seeker_night != Some(self.clock.day()) { return; }
        self.seeker_night = None;
        let Some(k) = self.seeker_of_relic() else { return };
        let Some(r) = self.relic.clone() else { return };
        let kn = self.settlers[k].name.clone();
        let me = self.settlers[k].pos;
        let seen = (0..self.settlers.len()).find(|&j| j != k && self.settlers[j].alive && self.settlers[j].job != Job::Sleep
            && (self.settlers[j].pos.0 as i32 - me.0 as i32).abs().max((self.settlers[j].pos.1 as i32 - me.1 as i32).abs()) <= 6);
        let day = self.clock.day();
        match seen {
            Some(w) => {
                let wn = self.settlers[w].name.clone();
                let line = format!("{} catches {} in the dark with {} under their cloak.", wn, kn, r.name);
                self.note(line.clone());
                self.moment(format!("{} is caught", kn), line, format!("because {} would not give {} up, and {} meant to have it", self.speaker.map(|s| self.settlers[s].name.clone()).unwrap_or_else(|| "the camp".into()), r.name, kn), me);
                self.crimes.push(justice::Crime { who: k, what: format!("tried to steal {}", r.name), day, meals: 0, judged: false });
                self.like(k, w, -5);
            }
            None => {
                let line = format!("In the night {} slips away, and {} with them.", kn, r.name);
                self.note(line.clone());
                self.moment(format!("{} is stolen", r.name), line, format!("because {} was refused it, and wanted it more than the camp's good will", kn), me);
                if let Some(x) = self.relic.as_mut() { x.fate = Some(format!("stolen by {}, who had sought it, on day {}", kn, day)); }
                self.release(k);
                let s = &mut self.settlers[k];
                s.alive = false;
                s.mind.left = true;
                for j in 0..self.settlers.len() { if self.settlers[j].alive { self.feel(j, mind::Feel::Mandate { what: format!("{} be let go so easily", r.name) }); } }
            }
        }
    }
}
