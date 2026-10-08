//! Needs (Dwarf Fortress's personality needs): each settler has a handful of needs drawn from
//! their character (DF derives them from facets and values: the gregarious need company, the
//! pious prayer, the lover of the wild the sight of beasts...). Each has a strength (1-10) and a
//! focus that falls an hour at a time by that strength (a strength-5 need, met, is due again
//! in about three days) and is restored (to 400, DF's ceiling) when
//! the need is met. Met needs make for a focused hand (work a little quicker), unmet ones a
//! distracted one, and a need long unmet is a thought at dawn.
//!
//! What the camp already does meets many of them (prayer at the temple, a festival, a cup of
//! wine, the cook's supper, a song, a work made, a hunt). The rest, and those not met in time,
//! pull a settler away from work for a while to do something of their own, on the map: talk with
//! a friend by the woodpile, sit with a sick man, kneel on a rise, watch the deer, look at the
//! engraving of the raid, walk out toward the hills, take it easy by the water. Two settlers of
//! the same camp spend their spare hours differently, and two camps of different people differ.

use super::*;
use crate::persona::{Facet, Persona, Val};
use super::projects::ProjectKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Need {
    Socialize, Friends, Family, Pray, TakeItEasy, SeeAnimal, AdmireArt, Wander, Excitement,
    HelpSomebody, Learn, ThinkAbstractly, MakeMerry, Tradition, Martial, Craft, BeCreative,
    StayOccupied, Drink, GoodMeal,
}

pub const ALL: [Need; 20] = [
    Need::Socialize, Need::Friends, Need::Family, Need::Pray, Need::TakeItEasy, Need::SeeAnimal,
    Need::AdmireArt, Need::Wander, Need::Excitement, Need::HelpSomebody, Need::Learn,
    Need::ThinkAbstractly, Need::MakeMerry, Need::Tradition, Need::Martial, Need::Craft,
    Need::BeCreative, Need::StayOccupied, Need::Drink, Need::GoodMeal,
];

/// The first words of each spare-hours act's reason (for counting them in the decisions).
pub const ACT_WORDS: [&str; 18] = ["Talking", "Arguing", "Passing the time", "Spending time", "Praying", "Kneeling", "Taking it easy", "Watching", "Admiring",
    "Walking out", "Climbing", "Sitting", "Lending", "Reading", "Singing", "Telling", "Practising", "Whittling"];

/// DF's ceiling: focus is set to this when a need is met.
pub const FULL: i32 = 400;

impl Need {
    /// What is needed, as "too long without ___".
    pub fn word(self) -> &'static str {
        match self {
            Need::Socialize => "company", Need::Friends => "their friends", Need::Family => "their family",
            Need::Pray => "prayer", Need::TakeItEasy => "rest", Need::SeeAnimal => "the sight of beasts",
            Need::AdmireArt => "beautiful things", Need::Wander => "a walk alone", Need::Excitement => "excitement",
            Need::HelpSomebody => "a chance to help someone", Need::Learn => "something new to learn", Need::ThinkAbstractly => "time to think",
            Need::MakeMerry => "merriment", Need::Tradition => "the old ways", Need::Martial => "arms practice",
            Need::Craft => "a craft to work at", Need::BeCreative => "something new to make", Need::StayOccupied => "work",
            Need::Drink => "a drink", Need::GoodMeal => "a good meal",
        }
    }
}

#[derive(Clone, Debug)]
pub struct NeedState {
    pub need: Need,
    /// 1-10: how fast focus falls (DF's need_level).
    pub level: u8,
    /// FULL when just met; below 0, unmet (DF's focus_level).
    pub focus: i32,
    /// The day it was last met.
    pub met_day: u64,
}

/// A spare-hours act under way: what it meets, with whom, and what it was about.
#[derive(Clone, Debug)]
pub struct NeedAct {
    pub need: Need,
    /// Where it is done (a haunt's place, `haunts.rs`).
    pub at: Pos,
    pub with: Option<usize>,
    pub what: String,
    pub minutes: u32,
    /// What a talk is about (`talk.rs`).
    pub topic: Option<super::talk::Topic>,
}

/// How strongly a character needs each thing, 0..~1.3 (the values -50..50, facets 0..100).
fn strength(p: &Persona, need: Need, faith: bool) -> f32 {
    let f = |x: Facet| (p.facet(x) as f32 - 50.0) / 50.0;
    let v = |x: Val| p.value(x) as f32 / 50.0;
    match need {
        Need::Socialize => f(Facet::Gregariousness) * 1.2 - 0.1,
        Need::Friends => v(Val::Friendship) * 1.1 + 0.1 * f(Facet::Friendliness),
        Need::Family => v(Val::Family) * 1.1,
        Need::Pray => if faith { f(Facet::Piety) * 1.3 } else { 0.0 },
        Need::TakeItEasy => v(Val::Leisure) - 0.5 * v(Val::HardWork),
        Need::SeeAnimal => v(Val::Nature) + 0.2 * f(Facet::Curiosity),
        Need::AdmireArt => v(Val::Artwork) + 0.3 * f(Facet::ArtInclined),
        Need::Wander => 0.6 * f(Facet::Curiosity) + 0.5 * v(Val::Independence) + 0.3 * v(Val::Nature) - 0.2,
        Need::Excitement => f(Facet::ExcitementSeeking) * 1.2 - 0.1,
        Need::HelpSomebody => f(Facet::Altruism) * 1.1 - 0.1,
        Need::Learn => v(Val::Knowledge) + 0.4 * f(Facet::Curiosity) - 0.1,
        Need::ThinkAbstractly => 0.8 * f(Facet::Imagination) + 0.6 * v(Val::Introspection) - 0.2,
        Need::MakeMerry => v(Val::Merriment) + 0.4 * f(Facet::Humour),
        Need::Tradition => v(Val::Tradition) * 1.1 - 0.1,
        Need::Martial => v(Val::MartialProwess) * 1.1,
        Need::Craft => v(Val::Craftsmanship) + 0.2 * f(Facet::Perfectionism),
        Need::BeCreative => f(Facet::ArtInclined) * 0.8 + 0.5 * f(Facet::Imagination) - 0.2,
        Need::StayOccupied => v(Val::HardWork) + 0.3 * f(Facet::Perseverance),
        // Dwarves all need drink (DF); others by their appetites.
        Need::Drink => if p.race == "dwarf" { 0.6 + 0.4 * f(Facet::Immoderation) } else { f(Facet::Immoderation) - 0.2 },
        Need::GoodMeal => f(Facet::Immoderation) * 0.8 + 0.3 * f(Facet::Greed) - 0.1,
    }
}

/// The needs of a character (strength 1+), each starting somewhere between met and due (by a
/// hash of the name, so the same settler always starts the same).
pub fn roll(p: &Persona, name: &str, faith: bool) -> Vec<NeedState> {
    ALL.iter().filter_map(|&n| {
        let level = (strength(p, n, faith) * 10.0).round().clamp(0.0, 10.0) as u8;
        (level >= 1).then(|| {
            let h = crate::persona::seed_of(name, 0x4EED ^ n as u64);
            NeedState { need: n, level, focus: 100 + (h % 300) as i32, met_day: 0 }
        })
    }).collect()
}

impl Colony {
    /// Settler `i`'s needs, rolled the first time they are asked for.
    pub(crate) fn ensure_needs(&mut self, i: usize) {
        if self.settlers[i].mind.needs_rolled { return; }
        let s = &self.settlers[i];
        let faith = s.past.as_ref().map_or(false, |p| p.faith.is_some());
        let needs = roll(&s.persona, &s.name, faith);
        let s = &mut self.settlers[i];
        s.mind.needs = needs;
        s.mind.needs_rolled = true;
    }

    /// A need of settler `i` is met (or partly: `amount` of focus restored).
    pub(crate) fn meet(&mut self, i: usize, need: Need, amount: i32) {
        self.ensure_needs(i);
        let day = self.clock.day();
        if let Some(n) = self.settlers[i].mind.needs.iter_mut().find(|n| n.need == need) {
            n.focus = (n.focus.max(0) + amount).min(FULL);
            n.met_day = day;
        }
    }

    /// Every hour: focus falls by each need's strength (DF's focus falls by need_level).
    pub(crate) fn needs_hour(&mut self) {
        for i in 0..self.settlers.len() {
            if !self.settlers[i].alive { continue; }
            self.ensure_needs(i);
            for n in self.settlers[i].mind.needs.iter_mut() {
                n.focus = (n.focus - n.level as i32).max(-FULL);
            }
        }
    }

    /// How focused settler `i` is, -1 (distracted by every need) .. 1 (all met): DF's
    /// current_focus, the needs' focus weighted by their strength.
    pub fn focus_of(&self, i: usize) -> f32 {
        let n = &self.settlers[i].mind.needs;
        let w: f32 = n.iter().map(|x| x.level as f32).sum();
        if w == 0.0 { return 0.0; }
        n.iter().map(|x| x.level as f32 * (x.focus as f32 / FULL as f32).clamp(-1.0, 1.0)).sum::<f32>() / w
    }

    /// Work pace from focus: a focused hand 6% quicker, a distracted one 6% slower.
    pub(crate) fn focus_pace(&self, i: usize) -> f32 { 1.0 - 0.06 * self.focus_of(i) }

    /// A thing felt meets a need (DF: activities satisfy needs wherever they happen).
    pub(crate) fn needs_from_feel(&mut self, i: usize, f: &super::mind::Feel) {
        use super::mind::Feel as F;
        let met: &[Need] = match f {
            F::Festival { .. } => &[Need::MakeMerry, Need::Socialize, Need::Tradition, Need::Friends, Need::Family],
            F::Prayed => &[Need::Pray],
            F::Drank => &[Need::Drink],
            F::AteWell { .. } => &[Need::GoodMeal],
            F::Performed { .. } => &[Need::MakeMerry, Need::Tradition, Need::BeCreative],
            F::Heard { own: true, .. } => &[Need::Tradition, Need::MakeMerry],
            F::Heard { .. } => &[Need::MakeMerry],
            F::Friend { .. } => &[Need::Friends, Need::Socialize],
            F::Pet { .. } | F::SawLiked { .. } => &[Need::SeeAnimal],
            F::Admired { .. } => &[Need::AdmireArt],
            F::Made { .. } => &[Need::Craft, Need::BeCreative],
            F::Slew { .. } | F::RaidNight | F::Struck => &[Need::Excitement],
            F::Saved { .. } => &[Need::HelpSomebody, Need::Excitement],
            F::News { .. } => &[Need::Learn],
            F::Caravan { hated: false, .. } => &[Need::Socialize],
            F::Found { .. } | F::Breach { .. } => &[Need::Wander, Need::Excitement],
            F::Built { .. } => &[Need::StayOccupied],
            _ => &[],
        };
        for &n in met { self.meet(i, n, FULL); }
    }

    /// Dawn: yesterday's company meets the gregarious need in part; a need long unmet is felt.
    pub(crate) fn reckon_needs(&mut self) {
        let day = self.clock.day();
        for i in 0..self.settlers.len() {
            if !self.settlers[i].alive { continue; }
            self.ensure_needs(i);
            let company = self.settlers[i].mind.company;
            if company >= 120 { self.meet(i, Need::Socialize, 150); }
            if self.spouse_or_child_near(i) { self.meet(i, Need::Family, 150); }
            // One thought for the worst unmet need, weighed by its strength.
            let worst = self.settlers[i].mind.needs.iter().filter(|n| n.focus < -250)
                .max_by_key(|n| (n.level as i32) * -n.focus).map(|n| (n.need, n.level, day.saturating_sub(n.met_day.max(1))));
            if let Some((need, level, days)) = worst {
                self.feel(i, super::mind::Feel::NeedUnmet { what: need.word().to_string(), level, days });
            }
        }
    }

    /// Whether settler `i` slept or ate near their spouse or a child of theirs yesterday (here:
    /// one of them is within 6 cells now, at dawn, when families wake together).
    fn spouse_or_child_near(&self, i: usize) -> bool {
        let me = self.settlers[i].pos;
        let near = |j: usize| self.settlers[j].alive && cheb(self.settlers[j].pos, me) <= 6;
        self.settlers[i].spouse.map_or(false, near) || self.children.iter().any(|&(c, a, b)| (a == i || b == i) && near(c))
    }

    /// Settler `i`'s most pressing unmet need that something can be done about now, as an
    /// option for `decide` and the act it begins.
    /// (Needs are rolled by the hourly fall, `needs_hour`, before any is asked for.)
    pub(crate) fn need_option(&self, i: usize) -> Option<((f32, Job, String), NeedAct)> {
        let s = &self.settlers[i];
        if s.guest_until > 0 && s.visitor.is_some() && s.mind.needs.is_empty() { return None; }
        if s.ill_until > self.clock.tick || s.hunger >= 0.6 || s.past.as_ref().map_or(false, |p| p.age < 12) { return None; }
        let hour = self.clock.hour();
        if self.clock.is_night() || self.drill_due(i) { return None; }
        let mut due: Vec<&NeedState> = s.mind.needs.iter().filter(|n| n.focus < -60).collect();
        due.sort_by_key(|n| (std::cmp::Reverse(n.level as i32 * -n.focus), n.need as u8));
        let evening = (18..21).contains(&hour);
        for n in due {
            let deficit = (-n.focus as f32 / FULL as f32).min(1.0);
            let days = self.clock.day().saturating_sub(n.met_day.max(1));
            if let Some((target, why, act)) = self.need_act(i, n.need, evening, days) {
                // Leisure is cut short by a mandate against idle hands and by trouble on the way.
                let leisure = !matches!(n.need, Need::HelpSomebody | Need::Pray | Need::Martial | Need::Learn);
                let mut w = 0.2 + deficit * (0.3 + 0.06 * n.level as f32);
                if leisure && self.mandate == Some(society::Mandate::NoIdleHands) { w *= 0.5; }
                if self.trouble_foretold().is_some() && leisure { w *= 0.75; }
                // A thin store keeps them at the gathering.
                if self.food_stored() < self.food_goal() / 2 { w *= 0.4; }
                if evening { w += 0.2; }
                return Some(((w, Job::Wander(target), why), act));
            }
        }
        None
    }

    /// Where settler `i` would go to meet `need` now, why, and the act: their own place for it
    /// when they have one (`haunts.rs`), else wherever `need_act_here` finds.
    fn need_act(&self, i: usize, need: Need, evening: bool, days: u64) -> Option<(Pos, String, NeedAct)> {
        let (target, why, mut a) = self.need_act_here(i, need, evening, days)?;
        let s = &self.settlers[i];
        let (they, their) = if s.persona.female { ("she", "her") } else { ("he", "his") };
        let reason = why.split_once(": ").map(|x| x.1.to_string()).unwrap_or_default();
        let god = s.past.as_ref().and_then(|p| p.faith.as_ref()).map(|f| f.1.clone()).unwrap_or_default();
        let verb = match need {
            Need::Pray => format!("Praying to {}", god),
            Need::TakeItEasy => "Taking it easy".to_string(),
            Need::ThinkAbstractly => format!("Sitting alone with {} thoughts", their),
            Need::Wander => "Walking out alone".to_string(),
            Need::Craft | Need::BeCreative => "Whittling".to_string(),
            Need::Tradition => format!("Telling the old tales of {} people", their),
            _ => String::new(),
        };
        // Prayer at a temple or a standing stone stays there; elsewhere, their own place.
        let at_house = need == Need::Pray && (why.contains("at the temple") || why.contains("standing stone"));
        if super::haunts::kept(need) && !at_house {
            if let Some(h) = self.haunt_of(i, need) {
                let word = match need { Need::Pray => "cairn", Need::TakeItEasy => "bench", Need::ThinkAbstractly => "seat", Need::Wander => "waymark",
                    Need::Craft | Need::BeCreative => "carved post", Need::Tradition => "standing stone", _ => "place" };
                let place = if h.mark.is_some() { format!("at {} {}", their, word) } else { format!("at {},{}, where {} always goes", h.at.0, h.at.1, they) };
                a.at = h.at;
                return Some((h.at, format!("{} {}: {}", verb, place, reason), a));
            }
            // The devout without a place of their own may kneel at another's cairn.
            if need == Need::Pray {
                if let Some((h, title)) = self.shared_haunt(i, need, 30) {
                    a.at = h.at;
                    return Some((h.at, format!("{} at {}: {}", verb, title, reason), a));
                }
            }
        }
        a.at = target;
        Some((target, why, a))
    }

    fn need_act_here(&self, i: usize, need: Need, evening: bool, days: u64) -> Option<(Pos, String, NeedAct)> {
        let s = &self.settlers[i];
        let me = s.pos;
        let they = if s.persona.female { "she" } else { "he" };
        let since = |what: &str| if days >= 2 { format!("{} has gone {} days without {}", they, days, what) } else { format!("{} longs for {}", they, what) };
        let act = |need: Need, with: Option<usize>, what: String, minutes: u32| NeedAct { need, at: (0, 0), with, what, minutes, topic: None };
        let awake_near = |j: usize, r: i32| {
            let o = &self.settlers[j];
            j != i && o.alive && o.job != Job::Sleep && o.ill_until <= self.clock.tick && !self.below(j) && o.mind.left == false
                && o.away_until == 0 && cheb(o.pos, me) <= r
        };
        match need {
            Need::Socialize | Need::Friends => {
                // The dearest awake within reach, else anyone.
                let j = (0..self.settlers.len()).filter(|&j| awake_near(j, 40)).max_by_key(|&j| (self.opinion(i, j), std::cmp::Reverse(j)))?;
                if need == Need::Friends && self.opinion(i, j) < 8 { return None; }
                let o = &self.settlers[j];
                let (verb, about, topic) = self.talk_topic(i, j);
                let mut a = act(need, Some(j), o.name.clone(), 40);
                a.topic = Some(topic);
                Some((o.pos, format!("{} with {} {} {}: {}", verb, o.name, self.place_word(o.pos), about, since(need.word())), a))
            }
            Need::Family => {
                let kin: Vec<usize> = s.spouse.into_iter().chain(self.children.iter().filter(|c| c.1 == i || c.2 == i).map(|c| c.0)).filter(|&j| awake_near(j, 60)).collect();
                let j = *kin.first()?;
                let o = &self.settlers[j];
                let poss = if s.persona.female { "her" } else { "his" };
                let who = if Some(j) == s.spouse { format!("{} {}", poss, if o.persona.female { "wife" } else { "husband" }) }
                    else { format!("{} {}", poss, if o.persona.female { "daughter" } else { "son" }) };
                Some((o.pos, format!("Spending time with {} {} {}: {}", who, o.name, self.place_word(o.pos), since(need.word())), act(need, Some(j), o.name.clone(), 45)))
            }
            Need::Pray => {
                let god = s.past.as_ref().and_then(|p| p.faith.as_ref()).map(|f| f.1.clone())?;
                if let Some((at, _)) = self.temple() { return Some((at, format!("Praying to {} at the temple: {}", god, since(need.word())), act(need, None, god, 45))); }
                if let Some(sh) = self.stones.iter().find(|x| x.0 == StoneKind::Shrine).map(|x| x.1) {
                    return Some((sh, format!("Praying to {} at the standing stone: {}", god, since(need.word())), act(need, None, god, 45)));
                }
                let hill = self.high_spot(i, 25)?;
                Some((hill, format!("Kneeling on the rise at {},{} to pray to {}: {}", hill.0, hill.1, god, since(need.word())), act(need, None, god, 45)))
            }
            Need::TakeItEasy => {
                let (at, where_) = self.pleasant_spot(i)?;
                Some((at, format!("Taking it easy {}: {} values leisure, and {}", where_, they, since("rest")), act(need, None, where_, 60)))
            }
            Need::SeeAnimal => {
                let c = self.creatures.iter().filter(|c| matches!(c.kind, creatures::CreatureKind::Game | creatures::CreatureKind::Pet) && c.z.is_none() && !c.leaving
                    && cheb(c.pos, self.camp) <= 45).min_by_key(|c| (cheb(c.pos, me), c.id))?;
                let name = c.name.clone();
                let liked = fond_of(&s.persona, &name);
                let what = if c.kind == creatures::CreatureKind::Pet { format!("{} the {}", name, "pet") } else { format!("the {}", plural(&name)) };
                let why = format!("Watching {} at {},{}: {}{}", what, c.pos.0, c.pos.1, since(need.word()), if liked { format!(", and {} loves them", they) } else { String::new() });
                Some((c.pos, why, act(need, None, name, 40)))
            }
            Need::AdmireArt => {
                // An artifact set in its room, an engraving, or the best work at the camp.
                let h = crate::history::settlers::hash_pub(self.seed ^ i as u64, self.clock.day());
                let mut sights: Vec<(Pos, String)> = Vec::new();
                for (name, room, at) in &self.placed {
                    let room = self.rooms.get(*room).map(|r| r.kind.word()).unwrap_or("its room");
                    sights.push((*at, format!("{} in {}", name, room)));
                }
                for e in self.engravings.iter().take(40) { sights.push((e.from, format!("the engraving of {}", e.image))); }
                if let Some(w) = self.works.iter().filter(|w| !w.traded && w.quality >= 3).max_by_key(|w| (w.quality, w.day)) {
                    sights.push((self.camp, format!("{} by the fire, made by {}", w.describe(), self.settlers[w.maker].name)));
                }
                if sights.is_empty() { return None; }
                let (at, what) = sights[(h % sights.len() as u64) as usize].clone();
                Some((at, format!("Admiring {}: {}", what, since(need.word())), act(need, None, what, 30)))
            }
            Need::Wander => {
                let to = self.far_spot(i, 22, 45)?;
                Some((to, format!("Walking out alone toward the {} for the walking's sake: {}", direction(self.camp, to), since(need.word())), act(need, None, String::new(), 40)))
            }
            Need::Excitement => {
                if let Some((t, _)) = self.tower { return Some((t, format!("Climbing the lookout to look out over the land: {}", since(need.word())), act(need, None, "the lookout".into(), 30))); }
                let hill = self.high_spot(i, 35)?;
                Some((hill, format!("Climbing the high ground at {},{}: {}", hill.0, hill.1, since(need.word())), act(need, None, "the high ground".into(), 30)))
            }
            Need::HelpSomebody => {
                let tick = self.clock.tick;
                let sick = (0..self.settlers.len()).filter(|&j| awake_or_abed(self, j, i) && cheb(self.settlers[j].pos, me) <= 40
                    && (self.settlers[j].ill_until > tick || !self.settlers[j].wounds.is_empty())).min_by_key(|&j| (cheb(self.settlers[j].pos, me), j));
                if let Some(j) = sick {
                    let o = &self.settlers[j];
                    let what = if o.ill_until > tick { "who lies ill" } else { "who is hurt" };
                    return Some((o.pos, format!("Sitting with {}, {}: {}", o.name, what, since(need.word())), act(need, Some(j), o.name.clone(), 40)));
                }
                let j = (0..self.settlers.len()).filter(|&j| awake_near(j, 30) && matches!(self.settlers[j].job, Job::Build | Job::Fell(_) | Job::Quarry(_))).min_by_key(|&j| (cheb(self.settlers[j].pos, me), j))?;
                let o = &self.settlers[j];
                Some((o.pos, format!("Lending {} a hand at {}: {}", o.name, o.job.verb(), since(need.word())), act(need, Some(j), o.name.clone(), 40)))
            }
            Need::Learn => {
                if let Some(p) = self.projects.iter().find(|p| p.done && p.kind == ProjectKind::Library) {
                    return Some((p.at, format!("Reading at the library: {}", since(need.word())), act(need, None, "the library".into(), 60)));
                }
                // Watch the best hand at their work, to learn.
                let j = (0..self.settlers.len()).filter(|&j| awake_near(j, 35) && skill_of(self.settlers[j].job).is_some())
                    .max_by(|&a, &b| { let k = |j: usize| self.settlers[j].skill[skill_of(self.settlers[j].job).unwrap()]; k(a).total_cmp(&k(b)).then(b.cmp(&a)) })?;
                let o = &self.settlers[j];
                if skill_of(o.job).map_or(true, |k| o.skill[k] < 0.4) { return None; }
                Some((o.pos, format!("Watching {} at {}, to learn how it is done: {}", o.name, o.job.verb(), since(need.word())), act(need, Some(j), o.name.clone(), 40)))
            }
            Need::ThinkAbstractly => {
                let hill = self.high_spot(i, 20)?;
                let what = if evening { "watching the sky darken" } else { "alone with their thoughts" };
                Some((hill, format!("Sitting on the rise at {},{}, {}: {}", hill.0, hill.1, what, since(need.word())), act(need, None, String::new(), 45)))
            }
            Need::MakeMerry if evening => {
                Some((self.near_fire(i), format!("Singing and joking by the fire: {}", since(need.word())), act(need, None, String::new(), 45)))
            }
            Need::Tradition if evening => {
                Some((self.near_fire(i), format!("Telling the old tales of {} people by the fire: {}", if s.persona.female { "her" } else { "his" }, since(need.word())), act(need, None, String::new(), 45)))
            }
            Need::Martial => {
                let at = self.drill_ground();
                Some((at, format!("Practising thrusts with a stave at the drill ground: {}", since(need.word())), act(need, None, String::new(), 40)))
            }
            Need::Craft | Need::BeCreative => {
                // Whittling by the fire, where there is no workshop or nothing to work.
                if self.workshop_spot().is_some() && self.items.iter().any(|it| it.stored && matches!(it.kind, ItemKind::Log | ItemKind::Stone)) { return None; }
                Some((self.near_fire(i), format!("Whittling a little figure from a stick by the fire: {}", since(need.word())), act(need, None, String::new(), 60)))
            }
            _ => None,
        }
    }

    /// A spare-hours act done: the need is met, and those it touched feel it.
    pub(crate) fn complete_need(&mut self, i: usize, a: NeedAct) {
        self.meet(i, a.need, FULL);
        self.visit_haunt(i, a.need, a.at);
        match a.need {
            Need::Socialize | Need::Friends | Need::Family => {
                if let Some(j) = a.with.filter(|&j| self.settlers[j].alive && cheb(self.settlers[j].pos, self.settlers[i].pos) <= 3) {
                    match &a.topic {
                        Some(t) => { let place = self.place_word(self.settlers[i].pos); self.talk_done(i, j, t, &place); }
                        None => { self.warm(i, j); self.warm(j, i); }
                    }
                    self.meet(j, Need::Socialize, FULL / 2);
                    if a.need == Need::Family { self.meet(j, Need::Family, FULL); }
                }
            }
            Need::HelpSomebody => {
                if let Some(j) = a.with.filter(|&j| self.settlers[j].alive) {
                    self.like(j, i, 2);
                    self.meet(j, Need::Socialize, FULL / 2);
                }
            }
            Need::SeeAnimal => {
                if fond_of(&self.settlers[i].persona, &a.what) { self.feel(i, super::mind::Feel::SawLiked { what: a.what.clone() }); }
            }
            Need::AdmireArt => { self.feel(i, super::mind::Feel::Admired { what: a.what.clone() }); }
            Need::Pray => { self.settlers[i].mind.prayed = true; }
            Need::Learn => {
                if let Some(k) = a.with.and_then(|j| skill_of(self.settlers[j].job)) {
                    let s = &mut self.settlers[i];
                    s.skill[k] = (s.skill[k] + 0.01 * s.persona.learning()).min(1.0);
                }
            }
            _ => {}
        }
    }

    /// A few words for where `p` is: by a work standing there, the fire, water or trees.
    pub(crate) fn place_word(&self, p: Pos) -> String {
        if cheb(p, self.camp) <= 3 { return "by the fire".into(); }
        if let Some(k) = self.projects.iter().filter(|q| q.done && q.kind != ProjectKind::Palisade && q.kind != ProjectKind::Mending)
            .min_by_key(|q| cheb(q.at, p)).filter(|q| cheb(q.at, p) <= 4).map(|q| q.kind) {
            let w = k.word();
            return format!("by the {}", w.strip_prefix("an ").or_else(|| w.strip_prefix("a ")).or_else(|| w.strip_prefix("the ")).unwrap_or(w));
        }
        if self.hut.as_ref().map_or(false, |h| cheb(h.at, p) <= 5) { return "by the hut".into(); }
        let n = self.map.width;
        let wet = (-2i32..=2).any(|dy| (-2i32..=2).any(|dx| {
            let (x, y) = (p.0 as i32 + dx, p.1 as i32 + dy);
            x >= 0 && y >= 0 && (x as usize) < n && (y as usize) < self.map.height && {
                let z = self.map.surface_z[y as usize * n + x as usize] + 1;
                (z as usize) < self.map.depth && self.map.cell(x as usize, y as usize, z as usize).water > 0
            }
        }));
        if wet { return "by the water".into(); }
        let trees = (-2i32..=2).any(|dy| (-2i32..=2).any(|dx| {
            let q = ((p.0 as i32 + dx).max(0) as u16, (p.1 as i32 + dy).max(0) as u16);
            matches!(self.floor_plant_pub(q), crate::local::Plant::Tree(_))
        }));
        if trees { "under the trees".into() } else { format!("at {},{}", p.0, p.1) }
    }

    /// The highest open ground within `r` cells of the camp (by a hash, among the near-highest).
    pub(crate) fn high_spot(&self, i: usize, r: i32) -> Option<Pos> {
        let n = self.map.width as i32;
        let (cx, cy) = (self.camp.0 as i32, self.camp.1 as i32);
        let mut best: Option<(i32, u64, Pos)> = None;
        for y in (cy - r).max(2)..=(cy + r).min(self.map.height as i32 - 3) {
            for x in (cx - r).max(2)..=(cx + r).min(n - 3) {
                if (x + y) % 2 != 0 { continue; }
                let p = (x as u16, y as u16);
                if !nav::passable(&self.map, p) || self.marked(p, true) || self.in_hut(p) { continue; }
                let z = self.map.surface_z[y as usize * n as usize + x as usize];
                let h = crate::history::settlers::hash_pub(self.seed ^ (i as u64) << 20, (x as u64) << 16 | y as u64) % 1000;
                if best.map_or(true, |b| (z, h) > (b.0, b.1)) { best = Some((z, h, p)); }
            }
        }
        best.map(|b| b.2)
    }

    /// A pleasant place near the camp: by the water, else in the shade of trees.
    fn pleasant_spot(&self, i: usize) -> Option<(Pos, String)> {
        let me = self.settlers[i].pos;
        if let Some(w) = self.fishing_spots.iter().copied().filter(|&p| cheb(p, self.camp) <= 30 && !self.marked(p, true)).min_by_key(|&p| (cheb(p, me), p)) {
            return Some((w, format!("by the water at {},{}", w.0, w.1)));
        }
        let t = self.tree_spot(i)?;
        Some((t, format!("in the shade of the trees at {},{}", t.0, t.1)))
    }

    /// Open ground beside a standing tree within 25 cells of the camp.
    fn tree_spot(&self, i: usize) -> Option<Pos> {
        let n = self.map.width as i32;
        let (cx, cy) = (self.camp.0 as i32, self.camp.1 as i32);
        let h = crate::history::settlers::hash_pub(self.seed ^ i as u64, 0x7AEE);
        for r in 4..25i32 {
            for k in 0..8u64 {
                let a = ((h + k * 7919) % 360) as f32 * std::f32::consts::PI / 180.0;
                let (x, y) = (cx + (a.cos() * r as f32) as i32, cy + (a.sin() * r as f32) as i32);
                if x < 2 || y < 2 || x >= n - 2 || y >= self.map.height as i32 - 2 { continue; }
                let p = (x as u16, y as u16);
                if matches!(self.floor_plant_pub(p), crate::local::Plant::Tree(_)) {
                    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                        let q = ((x + dx) as u16, (y + dy) as u16);
                        if nav::passable(&self.map, q) && !self.marked(q, true) { return Some(q); }
                    }
                }
            }
        }
        None
    }

    /// Open ground `lo`..`hi` cells from the camp in a direction of the settler's own (a hash
    /// of their name and the day), on the camp's side of any water.
    fn far_spot(&self, i: usize, lo: i32, hi: i32) -> Option<Pos> {
        let n = self.map.width as i32;
        let h = crate::history::settlers::hash_pub(crate::persona::seed_of(&self.settlers[i].name, 0x3A1C), self.clock.day());
        for k in 0..12u64 {
            let a = (((h >> 8) + k * 31) % 360) as f32 * std::f32::consts::PI / 180.0;
            let r = lo + ((h + k) % (hi - lo + 1) as u64) as i32;
            let (x, y) = (self.camp.0 as i32 + (a.cos() * r as f32) as i32, self.camp.1 as i32 + (a.sin() * r as f32) as i32);
            if x < 2 || y < 2 || x >= n - 2 || y >= self.map.height as i32 - 2 { continue; }
            let p = (x as u16, y as u16);
            if nav::passable(&self.map, p) && !self.marked(p, true) && !self.unreachable.contains(&p) { return Some(p); }
        }
        None
    }

    /// A place a few cells from the fire, a different one for each settler.
    fn near_fire(&self, i: usize) -> Pos {
        let a = (i as f32 * 2.399) % std::f32::consts::TAU;
        self.spot_from_camp((a.cos() * 3.0).round() as i32, (a.sin() * 3.0).round() as i32)
    }

    /// The settler's needs in words, strongest first: "needs prayer (unmet 4 days), company".
    pub fn needs_text(&self, i: usize) -> Option<String> {
        let mut n: Vec<&NeedState> = self.settlers[i].mind.needs.iter().collect();
        if n.is_empty() { return None; }
        n.sort_by_key(|x| (std::cmp::Reverse(x.level), x.need as u8));
        let day = self.clock.day();
        let parts: Vec<String> = n.iter().take(6).map(|x| if x.focus < 0 { format!("{} (unmet {} days)", x.need.word(), day.saturating_sub(x.met_day.max(1))) } else { x.need.word().to_string() }).collect();
        let f = self.focus_of(i);
        let state = if f > 0.5 { "focused" } else if f > 0.0 { "settled" } else if f > -0.4 { "distracted" } else { "badly distracted" };
        Some(format!("Needs {}; {}", parts.join(", "), state))
    }
}

/// Whether `j` (not `i`) may be visited where they lie: alive, here, on the surface.
fn awake_or_abed(c: &Colony, j: usize, i: usize) -> bool {
    let o = &c.settlers[j];
    j != i && o.alive && !o.mind.left && o.away_until == 0 && !c.below(j)
}

pub(crate) fn cheb(a: Pos, b: Pos) -> i32 { (a.0 as i32 - b.0 as i32).abs().max((a.1 as i32 - b.1 as i32).abs()) }

/// The compass word from `a` toward `b`.
pub(crate) fn direction(a: Pos, b: Pos) -> &'static str {
    let (dx, dy) = (b.0 as f32 - a.0 as f32, b.1 as f32 - a.1 as f32);
    let ang = dy.atan2(dx).to_degrees();
    match ((ang + 360.0 + 22.5) % 360.0 / 45.0) as u32 { 0 => "east", 1 => "south-east", 2 => "south", 3 => "south-west", 4 => "west", 5 => "north-west", 6 => "north", _ => "north-east" }
}

fn plural(name: &str) -> String {
    if name.ends_with("deer") || name.ends_with("sheep") || name.ends_with("bison") || name.ends_with("elk") || name.ends_with("caribou") || name.ends_with("moose") { name.to_string() }
    else if name.ends_with('s') || name.ends_with("x") { format!("{}es", name) } else { format!("{}s", name) }
}

#[allow(dead_code)]
fn cap(s: &str) -> String { let mut c = s.chars(); c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default() }
