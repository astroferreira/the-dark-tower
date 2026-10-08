//! What settlers feel: thoughts, stress, moods, needs and breakdowns.
//!
//! The idea is Dwarf Fortress's emotional layer. What happens leaves a thought (grief at a
//! burial, a cold night, pride in a finished hall, envy of the patron's favourite), and the
//! person's character decides how much it weighs: the stress-prone feel the bad more, the
//! cheerful the good, a lover of the wild hates felling trees, the pious need somewhere to pray.
//! Thoughts build or ease stress, stress sets a mood, and a mind pushed too far breaks in a way
//! that follows its character: the angry rage, the gloomy despair, the restless walk off, and
//! once in a long while someone gives up on the camp for good. Every break is logged with the
//! thoughts behind it, so it can always answer "why?".
//!
//! No RNG: weights come from the persona, breaks from thresholds.

use super::*;
use crate::persona::{Attr, Facet, Val};

/// A thought: what, when, and how much it weighed (+ eases, - distresses).
#[derive(Clone, Debug)]
pub struct Thought {
    pub tick: u64,
    pub text: String,
    pub weight: f32,
}

/// The way a mind breaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Break { Tantrum, Despair, Wandering }

impl Break {
    pub fn word(self) -> &'static str {
        match self { Break::Tantrum => "a tantrum", Break::Despair => "despair", Break::Wandering => "wandering off" }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Mind {
    /// 0 calm; above 1 near breaking; below 0 in good spirits.
    pub stress: f32,
    /// Recent thoughts, newest last (at most `KEEP`).
    pub thoughts: Vec<Thought>,
    /// The heaviest things of the last thirty days (four at most), kept apart so a break can
    /// name them after a run of light days has pushed them out of `thoughts` (DF: strong
    /// memories outlast the rest). They weigh nothing again; they only give reasons.
    pub scars: Vec<Thought>,
    /// A break under way, and the tick it ends.
    pub broken: Option<(Break, u64)>,
    /// How many times this mind has broken.
    pub breaks: u32,
    /// Today: time spent near others (minutes), prayers said, work of their hands done.
    pub company: u32,
    pub prayed: bool,
    /// Drilled this evening (`militia.rs`).
    pub drilled: bool,
    pub made: u32,
    /// The mood word last logged (moods are logged when they change for the worse).
    pub last_mood: &'static str,
    /// They gave up on the camp and walked away (not dead: gone).
    pub left: bool,
    /// The tick the last break ended (a mind does not break again within `REST_DAYS`).
    pub mended_at: u64,
    /// The day a sinking mood was last said aloud (at most one every three days).
    pub mood_said: u64,
    /// The last day they performed at the fire.
    pub performed: u64,
    /// What they have been through (`temper.rs`): horrors and brave acts today and in all, days
    /// content in a row, and whether horror has stopped touching them.
    pub horrors_today: u32,
    pub braved_today: u32,
    pub horrors: u32,
    pub content_days: u32,
    pub jaded: bool,
}

const KEEP: usize = 10;
/// Stress at which a mind breaks (at dawn), and at which a mind that has broken twice may leave.
pub const BREAK_AT: f32 = 1.2;
pub const LEAVE_AT: f32 = 2.0;
/// Days after a break before a mind can break again.
pub const REST_DAYS: u64 = 5;

/// A news line inside a sentence: "The Kingdom of X rises again (433)" -> "the Kingdom...".
fn lower_news(s: &str) -> String { s.strip_prefix("The ").map(|r| format!("the {}", r)).unwrap_or_else(|| s.to_string()) }

/// The mood a stress level reads as.
pub fn mood(stress: f32) -> &'static str {
    if stress < -0.6 { "in high spirits" }
    else if stress < -0.2 { "content" }
    else if stress < 0.4 { "fine" }
    else if stress < 0.8 { "unhappy" }
    else if stress < BREAK_AT { "miserable" }
    else { "at breaking point" }
}

/// What can be felt.
#[derive(Clone, Debug)]
pub enum Feel {
    /// Someone died; whether they were close.
    Death { whom: String, close: bool },
    ColdNight,
    SleptWarm,
    Hungry,
    Ill,
    /// Struck in the raid.
    Struck,
    /// Saved someone, or was saved by someone.
    Saved { whom: String },
    SavedBy { whom: String },
    /// The raid came, and they lived through it.
    RaidNight,
    Quarrel { with: String },
    Friend { with: String },
    /// A work they laid stones or logs in stands finished.
    Built { what: String },
    /// Worked a material they like.
    LikedWork { material: String },
    /// Felled a tree while holding the wild dear.
    FelledTree,
    Favoured,
    Envy { of: String },
    Lonely,
    Prayed,
    NowhereToPray,
    Idle,
    Busy,
    /// The mine broke into a cavern.
    Breach { what: String },
    /// A wound ("a broken left arm").
    Wounded { what: String },
    /// Kept a festival.
    Festival { what: String },
    /// Punished by the speaker.
    Punished { by: String },
    /// Killed a creature they love; saw one grazing near the camp.
    KilledLiked { what: String },
    SawLiked { what: String },
    /// Heard news that touches them (their people, a town of their past).
    News { what: String, good: bool },
    /// A mandate against their values; peace made by the speaker.
    Mandate { what: String },
    Reconciled { by: String },
    /// A caravan came: from where, whether its people are hated, whether it bought.
    Caravan { town: String, hated: bool, sold: bool },
    /// Made something (quality 0-5), or admired another's masterwork.
    Made { what: String, quality: u8 },
    Admired { what: String },
    /// Performed one of their people's works at the fire, or heard one (`own`: of their people).
    Performed { what: String },
    Heard { what: String, own: bool },
    /// Something came up from the deep.
    TheDeep { what: String },
    /// Found a lost thing of the world (`relic.rs`).
    Found { what: String },
    /// Struck the blow that killed a beast; saw it fall.
    Slew { what: String },
    SawFall { what: String },
    /// Their pet's company (`pets.rs`).
    Pet { name: String },
    /// A cup of wine with a meal; too long without one (dwarves; `drink.rs`).
    Drank,
    Thirsty,
    /// Their own people raided the camp (`arc.rs`).
    Torn { people: String },
    /// Ate the cook's supper (`kitchen.rs`).
    AteWell { dish: String, fine: bool },
    /// Wore rags (`clothes.rs`).
    Ragged,
    /// A dream of a lifetime realized (`dreams.rs`).
    Dreamt { what: String },
    /// Ate half rations (`rations.rs`).
    Rationed,
    /// Slept in a bedroom of their own (`delve.rs`).
    OwnRoom,
}

impl Colony {
    /// Settler `i` feels something: its weight follows their character; the thought is kept and
    /// moves their stress.
    pub(crate) fn feel(&mut self, i: usize, f: Feel) {
        // What a ruling lord does against someone is remembered (`rising.rs`).
        if self.settlers[i].alive {
            match &f {
                Feel::Mandate { .. } => self.grieve(i, 1),
                Feel::Punished { .. } => {
                    self.grieve(i, 2);
                    let friends: Vec<usize> = (0..self.settlers.len()).filter(|&j| j != i && self.settlers[j].alive && self.opinion(i, j) >= 12).collect();
                    for j in friends { self.grieve(j, 1); }
                }
                _ => {}
            }
        }
        let tick = self.clock.tick;
        let s = &self.settlers[i];
        if !s.alive { return; }
        let p = &s.persona;
        let fac = |x: Facet| p.facet(x) as f32 / 100.0;
        let val = |v: Val| p.value(v) as f32 / 50.0;
        let (text, base): (String, f32) = match &f {
            Feel::Death { whom, close } => (
                if *close { format!("grieved for {}, a friend", whom) } else { format!("saw {} laid in the ground", whom) },
                -(0.15 + if *close { 0.35 } else { 0.0 }) * (0.6 + 0.8 * fac(Facet::Love)) * (p.attr(Attr::Empathy) / 1000.0).clamp(0.4, 1.6).sqrt()),
            Feel::ColdNight => ("slept in the cold".into(), -0.06),
            Feel::SleptWarm => ("slept warm under a roof".into(), 0.02 + 0.03 * fac(Facet::Immoderation)),
            Feel::Hungry => ("went hungry".into(), -0.08),
            Feel::Ill => ("fell ill from the cold".into(), -0.12),
            Feel::Struck => ("was struck down in the raid".into(), -0.35 * (1.3 - fac(Facet::Bravery))),
            Feel::Saved { whom } => (format!("saved {} from the raiders", whom), 0.25 * (0.7 + 0.6 * fac(Facet::Pride))),
            Feel::SavedBy { whom } => (format!("was saved by {}", whom), 0.1 + 0.2 * fac(Facet::Gratitude)),
            Feel::RaidNight => ("lived through the night of the raid".into(), -0.18 * (0.4 + 1.2 * fac(Facet::Anxiety)) * (1.3 - fac(Facet::Bravery))),
            Feel::Quarrel { with } => (format!("quarrelled with {}", with), -0.12 + 0.2 * (fac(Facet::Discord) - 0.5).max(0.0)),
            Feel::Friend { with } => (format!("spent the evening with {}, a friend", with), 0.05 * (0.5 + fac(Facet::Friendliness)) * (1.0 + val(Val::Friendship).max(0.0))),
            Feel::Built { what } => (format!("saw {} they built stand finished", what), 0.12 * (1.0 + val(Val::Craftsmanship).max(0.0) + 0.5 * val(Val::HardWork).max(0.0))),
            Feel::LikedWork { material } => (format!("worked {}, which they like", material), 0.04),
            Feel::FelledTree => ("felled a tree, though they hold the wild dear".into(), -0.05 * (1.0 + val(Val::Nature))),
            Feel::Favoured => ("was favoured by the patron".into(), 0.15 * (0.6 + 0.8 * fac(Facet::Vanity).max(fac(Facet::Pride)))),
            Feel::Envy { of } => (format!("envied {}, the patron's favourite", of), -0.1 * (0.5 + fac(Facet::Envy))),
            Feel::Lonely => ("was lonely".into(), -0.06 * fac(Facet::Gregariousness) * 1.6),
            Feel::Prayed => ("prayed at the shrine".into(), 0.06 * (0.5 + fac(Facet::Piety))),
            Feel::NowhereToPray => ("had nowhere to pray".into(), -0.04 * fac(Facet::Piety) * 1.5),
            Feel::Idle => ("had nothing to do".into(), -0.03 * (1.0 + val(Val::HardWork).max(0.0)) + 0.03 * val(Val::Leisure).max(0.0)),
            Feel::Busy => ("worked hard all day".into(), 0.02 * (1.0 + val(Val::HardWork).max(0.0)) - 0.03 * val(Val::Leisure).max(0.0)),
            // Wonder for the curious and the thrill-seeking, dread for the anxious.
            Feel::Breach { what } => (
                if fac(Facet::Curiosity) + fac(Facet::ExcitementSeeking) > 1.0 + fac(Facet::Anxiety) { format!("marvelled at {}", what) } else { format!("dreaded {} opened under the camp", what) },
                0.12 * (fac(Facet::Curiosity) + fac(Facet::ExcitementSeeking) - 1.0 - fac(Facet::Anxiety) + 0.3) + 0.08 * val(Val::Knowledge)),
            Feel::Performed { what } => (format!("performed {} by the fire", what), 0.08 * (0.6 + fac(Facet::ArtInclined)) * (1.0 + val(Val::Artwork).max(0.0))),
            Feel::Heard { what, own } => (format!("heard {} by the fire", what), 0.035 * (0.2 + fac(Facet::ArtInclined)) * if *own { 1.3 } else { 0.8 } * (1.0 + 0.5 * val(Val::Artwork).max(0.0) + 0.5 * val(Val::Merriment).max(0.0))),
            Feel::Wounded { what } => (format!("suffered {}", what), if what.contains("broken") || what.contains("cracked") { -0.3 } else if what.contains("gashed") { -0.15 } else { -0.05 }),
            Feel::Made { what, quality } => (format!("made {}", what), (0.02 + 0.03 * *quality as f32) * (1.0 + val(Val::Craftsmanship).max(0.0)) - if *quality == 0 { 0.04 * fac(Facet::Perfectionism) } else { 0.0 }),
            Feel::Admired { what } => (format!("admired {}", what), 0.08 * (0.5 + fac(Facet::ArtInclined))),
            Feel::Caravan { town, hated, sold } => if *hated { (format!("had to trade with the people of {}", town), -0.08 * (0.5 + fac(Facet::Hate))) }
                else { (format!("met the traders of {}", town), 0.03 * (0.5 + fac(Facet::Gregariousness)) + if *sold { 0.04 * (0.5 + fac(Facet::Greed)) * (1.0 + val(Val::Commerce).max(0.0)) } else { 0.0 }) },
            Feel::Festival { what } => (format!("kept {} with everyone", what), 0.1 * (0.4 + fac(Facet::Gregariousness)) * (1.0 + val(Val::Merriment).max(0.0)) - 0.05 * fac(Facet::Bashfulness)),
            Feel::Mandate { what } => (format!("resented the mandate that {}", what), -0.08 * (0.5 + fac(Facet::Pride)) * (0.5 + fac(Facet::Discord))),
            Feel::Reconciled { by } => (format!("made peace, thanks to {}", by), 0.08 * (0.5 + fac(Facet::Gratitude))),
            Feel::News { what, good } => (format!("heard that {}", lower_news(what)), if *good { 0.12 * (0.6 + 0.8 * fac(Facet::Cheer)) } else { -0.18 * (0.5 + fac(Facet::Love)) }),
            Feel::KilledLiked { what } => (format!("killed a {}, a creature they love", what), -0.12 * (0.4 + fac(Facet::Love)) * (1.0 + val(Val::Nature).max(0.0))),
            Feel::SawLiked { what } => (format!("watched the {} grazing", what), 0.04 * (0.6 + fac(Facet::Curiosity))),
            Feel::Punished { by } => (format!("were put in the stocks by {}", by), -0.15 * (0.5 + fac(Facet::Pride))),
            Feel::TheDeep { what } => (format!("saw {}", what), -0.3 * (1.3 - fac(Facet::Bravery)) * (0.6 + 0.8 * fac(Facet::Anxiety))),
            Feel::Found { what } => (format!("found {}", what), 0.15 * (0.5 + fac(Facet::Curiosity).max(fac(Facet::Greed)))),
            Feel::Slew { what } => (format!("slew {}", what), 0.35 * (0.6 + 0.8 * fac(Facet::Pride).max(fac(Facet::Bravery))) * (1.0 + val(Val::MartialProwess).max(0.0))),
            Feel::AteWell { dish, fine } => (format!("ate {}{}", dish, if *fine { ", and it was fine" } else { "" }), (if *fine { 0.06 } else { 0.03 }) * (0.6 + 0.8 * fac(Facet::Immoderation))),
            Feel::Rationed => ("ate half rations".to_string(), Feel::rationed_weight(p)),
            Feel::OwnRoom => ("slept in a bedroom of their own".to_string(), 0.03 * (0.5 + fac(Facet::Bashfulness).max(fac(Facet::Orderliness)))),
            Feel::Dreamt { what } => (format!("realized a dream of {}", what), 0.6),
            Feel::Ragged => ("went about in rags".to_string(), -0.04 * (0.5 + fac(Facet::Vanity))),
            Feel::Torn { people } => (format!("saw their own people, {}, raid the camp", people), -0.25 * (0.5 + val(Val::Loyalty).max(0.0)) * (0.6 + 0.8 * fac(Facet::Love))),
            Feel::Drank => ("had a cup of berry wine with a meal".into(), 0.02 + 0.04 * fac(Facet::Immoderation)),
            Feel::Thirsty => ("has not had a drink in days".into(), -0.07),
            Feel::Pet { name } => (format!("spent time with {}, their pet", name), 0.03 * (0.5 + fac(Facet::Love)) * (1.0 + val(Val::Nature).max(0.0))),
            Feel::SawFall { what } => (format!("saw {} fall", what), 0.15 * (0.5 + fac(Facet::Bravery)) + 0.1 * fac(Facet::Vengefulness)),
        };
        // Character weighs it: the stress-prone and weak-willed feel the bad more; the cheerful
        // feel the good more, the gloomy less.
        let weight = if base < 0.0 {
            base * (0.5 + fac(Facet::StressVulnerability)) * (1000.0 / p.attr(Attr::Willpower).max(100.0)).powf(0.3)
        } else {
            base * (0.6 + 0.8 * fac(Facet::Cheer)) * (1.2 - 0.4 * fac(Facet::Gloom))
        };
        // Habit dulls a feeling (as in Dwarf Fortress): the same thought again weighs half as much
        // for each time it is still remembered.
        let again = self.settlers[i].mind.thoughts.iter().filter(|t| t.text == text).count() as i32;
        let weight = weight * 0.5f32.powi(again).max(0.2);
        // What it does to them in time (`temper.rs`): horrors and brave acts are counted; the
        // jaded feel horror and grief at half weight.
        let horror = matches!(&f, Feel::TheDeep { .. } | Feel::Death { close: true, .. } | Feel::Struck);
        let brave = matches!(&f, Feel::Saved { .. } | Feel::Slew { .. });
        if horror { self.settlers[i].mind.horrors_today += 1; }
        if brave { self.settlers[i].mind.braved_today += 1; }
        let weight = if self.settlers[i].mind.jaded && (horror || matches!(&f, Feel::Death { .. } | Feel::RaidNight)) { weight * 0.5 } else { weight };
        let s = &mut self.settlers[i];
        // Contentment cushions, but only so far (a death still bites a happy camp).
        s.mind.stress = (s.mind.stress - weight).clamp(-0.8, 3.0);
        if weight <= -0.15 {
            s.mind.scars.retain(|t| tick.saturating_sub(t.tick) <= 30 * TICKS_PER_DAY);
            s.mind.scars.push(Thought { tick, text: text.clone(), weight });
            if s.mind.scars.len() > 4 {
                let k = s.mind.scars.iter().enumerate().max_by(|a, b| a.1.weight.total_cmp(&b.1.weight)).map(|x| x.0).unwrap_or(0);
                s.mind.scars.remove(k);
            }
        }
        s.mind.thoughts.push(Thought { tick, text, weight });
        if s.mind.thoughts.len() > KEEP { s.mind.thoughts.remove(0); }
    }

    /// Each dawn: yesterday's lot (the night, hunger, company, prayer, work), stress easing with
    /// time, moods said aloud when they sink, and a mind past breaking breaks.
    pub(crate) fn reckon_minds(&mut self) {
        let day = self.clock.day();
        let shrine = self.stones.iter().any(|s| s.0 == StoneKind::Shrine) || self.temple().is_some();
        for i in 0..self.settlers.len() {
            if !self.settlers[i].alive { continue; }
            let (exposure, hunger, ill, under_roof) = {
                let s = &self.settlers[i];
                (s.exposure, s.hunger, s.ill_until > self.clock.tick, self.in_hut(s.pos) || self.below(i))
            };
            if exposure >= 0.5 { self.feel(i, Feel::ColdNight); } else if under_roof { self.feel(i, Feel::SleptWarm); }
            if self.bedroom_of(i).map_or(false, |r| r.furnished.is_some()) && self.below(i) { self.feel(i, Feel::OwnRoom); }
            if hunger >= 0.85 { self.feel(i, Feel::Hungry); }
            // A liked creature grazing within sight of the camp.
            let seen = self.creatures.iter().filter(|c| c.kind == super::creatures::CreatureKind::Game)
                .find(|c| super::fond_of(&self.settlers[i].persona, &c.name) && (c.pos.0 as i32 - self.camp.0 as i32).abs().max((c.pos.1 as i32 - self.camp.1 as i32).abs()) <= 30).map(|c| c.name.clone());
            if let Some(what) = seen { self.feel(i, Feel::SawLiked { what }); }
            let _ = ill;
            // Needs: company for the sociable, prayer for the devout, work for the industrious.
            let (company, prayed, made, greg, pious) = {
                let m = &self.settlers[i].mind;
                let p = &self.settlers[i].persona;
                (m.company, m.prayed, m.made, p.facet(Facet::Gregariousness), p.facet(Facet::Piety))
            };
            if greg >= 60 && company < 60 { self.feel(i, Feel::Lonely); }
            if let Some(f) = self.friend_of(i).filter(|_| company >= 60) {
                let with = self.settlers[f].name.clone();
                self.feel(i, Feel::Friend { with });
            }
            if pious >= 70 {
                if prayed { self.feel(i, Feel::Prayed); } else if !shrine { self.feel(i, Feel::NowhereToPray); }
            }
            if day > 2 { if made == 0 { self.feel(i, Feel::Idle); } else if made >= 4 { self.feel(i, Feel::Busy); } }
            // Heavy thoughts linger (grief, wounds, terror): each weighs again, a quarter as much,
            // every dawn for five days.
            let today = self.clock.tick;
            let s = &mut self.settlers[i];
            let linger: f32 = s.mind.thoughts.iter().filter(|t| t.weight <= -0.15 && today.saturating_sub(t.tick) <= 5 * TICKS_PER_DAY).map(|t| -0.25 * t.weight).sum();
            s.mind.stress = (s.mind.stress + linger).min(3.0);
            s.mind.company = 0;
            s.mind.prayed = false;
            s.mind.drilled = false;
            s.mind.made = 0;
            // Time heals: stress eases a little each day, faster for the cheerful.
            let ease = 0.08 * (0.6 + 0.8 * s.persona.facet(Facet::Cheer) as f32 / 100.0);
            if s.mind.stress > 0.0 { s.mind.stress = (s.mind.stress - ease).max(0.0); } else { s.mind.stress = (s.mind.stress + 0.06).min(0.0); }
        }
        for i in 0..self.settlers.len() {
            if !self.settlers[i].alive || self.settlers[i].mind.broken.is_some() { continue; }
            let stress = self.settlers[i].mind.stress;
            let name = self.settlers[i].name.clone();
            let m = mood(stress);
            let worse = |a: &str, b: &str| { let r = |x: &str| ["in high spirits", "content", "fine", "unhappy", "miserable", "at breaking point"].iter().position(|y| *y == x).unwrap_or(2); r(a) > r(b) };
            if m != self.settlers[i].mind.last_mood {
                if worse(m, self.settlers[i].mind.last_mood) && matches!(m, "unhappy" | "miserable") && (self.settlers[i].mind.mood_said == 0 || day >= self.settlers[i].mind.mood_said + 3) {
                    self.settlers[i].mind.mood_said = day;
                    let why = self.worst_thoughts(i, 2);
                    self.note(format!("{} is {}: {}.", name, m, why));
                }
                self.settlers[i].mind.last_mood = m;
            }
            if stress >= LEAVE_AT && self.settlers[i].mind.breaks >= 2 && !self.holds_on(i) {
                self.walk_away(i);
            } else if stress >= BREAK_AT && (self.settlers[i].mind.breaks == 0 || self.clock.tick >= self.settlers[i].mind.mended_at + REST_DAYS * TICKS_PER_DAY) {
                self.break_down(i);
            }
        }
    }

    /// The two (or `n`) heaviest recent thoughts, as a clause.
    pub fn worst_thoughts(&self, i: usize, n: usize) -> String {
        let m = &self.settlers[i].mind;
        let mut t: Vec<&Thought> = m.thoughts.iter().filter(|t| t.weight < 0.0).chain(m.scars.iter()).collect();
        t.sort_by(|a, b| a.weight.total_cmp(&b.weight).then(b.tick.cmp(&a.tick)));
        let mut seen: Vec<&str> = Vec::new();
        let parts: Vec<String> = t.into_iter().filter(|t| if seen.contains(&t.text.as_str()) { false } else { seen.push(&t.text); true })
            .take(n).map(|t| format!("{} (day {})", t.text, t.tick / TICKS_PER_DAY + 1)).collect();
        if parts.is_empty() { "nothing they will name".into() } else { format!("they {}", crate::persona::list(&parts)) }
    }

    /// Whether duty, loyalty or stubbornness keeps someone at the camp past their limit.
    fn holds_on(&self, i: usize) -> bool {
        let p = &self.settlers[i].persona;
        p.facet(Facet::Perseverance) >= 76 || p.facet(Facet::Dutifulness) >= 76 || p.value(Val::Loyalty) >= 26 || p.value(Val::Family) >= 41
    }

    /// A mind breaks, the way its character runs: the angry rage, the gloomy despair, the
    /// restless walk off for a day.
    fn break_down(&mut self, i: usize) {
        let p = &self.settlers[i].persona;
        let kind = if p.facet(Facet::Anger) >= 60 || p.facet(Facet::Violence) >= 60 { Break::Tantrum }
            else if p.facet(Facet::Gloom) >= 55 || p.facet(Facet::Cheer) <= 30 { Break::Despair }
            else { Break::Wandering };
        let until = self.clock.tick + match kind { Break::Tantrum => TICKS_PER_DAY / 2, _ => TICKS_PER_DAY };
        let why = self.worst_thoughts(i, 3);
        let name = self.settlers[i].name.clone();
        let character = self.settlers[i].persona.facet_phrase(match kind { Break::Tantrum => Facet::Anger, Break::Despair => Facet::Gloom, Break::Wandering => Facet::ExcitementSeeking } as usize);
        let s = &mut self.settlers[i];
        s.mind.broken = Some((kind, until));
        s.mind.breaks += 1;
        // Breaking spends some of it.
        s.mind.stress -= 0.5;
        self.release(i);
        let pos = self.settlers[i].pos;
        if self.watcher == Some(i) { self.watcher = None; }
        let line = match kind {
            Break::Tantrum => {
                // A rage breaks things: the work under way loses a load, and the nearest takes it badly.
                let lost = self.knock_down_work();
                if let Some(w) = &lost { self.record_wrecking(i, w); }
                let near = (0..self.settlers.len()).filter(|&j| j != i && self.settlers[j].alive)
                    .min_by_key(|&j| ((self.settlers[j].pos.0 as i32 - pos.0 as i32).abs().max((self.settlers[j].pos.1 as i32 - pos.1 as i32).abs()), j));
                let mut extra = String::new();
                if let Some(j) = near {
                    self.like(i, j, -5);
                    extra = format!(" and shouts at {}", self.settlers[j].name);
                    let me = name.clone();
                    self.feel(j, Feel::Quarrel { with: me });
                    self.make_peace(i, j);
                }
                format!("{} throws a tantrum{}{}", name, lost.map(|w| format!(", tears down part of {}", w)).unwrap_or_default(), extra)
            }
            Break::Despair => format!("{} sinks into despair and lies by the fire, unable to work", name),
            Break::Wandering => format!("{} walks off into the wild and will not say where", name),
        };
        let because = format!("because {}{}", why, character.map(|c| format!(", and {} {}", if self.settlers[i].persona.female { "she" } else { "he" }, c)).unwrap_or_default());
        self.note(format!("{} ({}).", line, because));
        self.moment(format!("{} breaks", name), format!("{}.", line), because, pos);
    }

    /// Someone gives up on the camp and walks away for good.
    fn walk_away(&mut self, i: usize) {
        let why = self.worst_thoughts(i, 3);
        let name = self.settlers[i].name.clone();
        let pos = self.settlers[i].pos;
        self.release(i);
        let s = &mut self.settlers[i];
        s.mind.left = true;
        s.alive = false;
        let line = format!("{} has had enough. At first light {} shoulders a pack and leaves the camp for good", name, if s.persona.female { "she" } else { "he" });
        self.note(format!("{} (because {}).", line, why));
        self.moment(format!("{} leaves", name), format!("{}.", line), format!("because {}", why), pos);
        // The others feel the loss, less than a death.
        for j in 0..self.settlers.len() {
            if j != i && self.settlers[j].alive && self.opinion(i, j) >= 6 {
                let whom = name.clone();
                self.feel(j, Feel::Death { whom: format!("{} go", whom), close: false });
            }
        }
    }

    /// A rage tears down a load of the work under way, if any. Returns what it was.
    fn knock_down_work(&mut self) -> Option<String> {
        if let Some(k) = self.projects.iter().position(|p| !p.done && p.used > 0 && !super::projects::is_dig(p.kind)) {
            self.projects[k].used -= 1;
            return Some(self.projects[k].kind.word().replacen("a ", "the ", 1));
        }
        None
    }

    /// A break under way decides for them: the raging stamp about the camp, the despairing lie
    /// by the fire, the wanderer heads for the trees. None when they are themselves.
    pub(crate) fn broken_choice(&mut self, i: usize) -> Option<(Job, String)> {
        let (kind, until) = self.settlers[i].mind.broken?;
        // Hunger still wins: a break pauses for a meal.
        if self.settlers[i].hunger >= 0.8 && self.food_stored() > 0 { return None; }
        if self.clock.tick >= until {
            self.settlers[i].mind.broken = None;
            self.settlers[i].mind.mended_at = self.clock.tick;
            let name = self.settlers[i].name.clone();
            self.note(format!("{} comes back to {}.", name, if self.settlers[i].persona.female { "herself" } else { "himself" }));
            return None;
        }
        let pos = self.settlers[i].pos;
        Some(match kind {
            Break::Tantrum => (Job::Wander(self.near(self.camp, 5, i as u64 + self.clock.tick / 60)), "In a rage, stamping about the camp".into()),
            Break::Despair => (Job::Sleep, "Lying by the fire in despair, unable to work".into()),
            Break::Wandering => (Job::Wander(self.near(pos, 30, i as u64 + self.clock.tick / 120)), "Walking alone in the wild".into()),
        })
    }

    /// A passable spot within `r` of `at`, chosen by hash.
    fn near(&self, at: Pos, r: i32, salt: u64) -> Pos {
        for k in 0..16u64 {
            let h = crate::history::settlers::hash_pub(salt.wrapping_add(k), 0x3E4D);
            let dx = (h % (2 * r as u64 + 1)) as i32 - r;
            let dy = ((h >> 20) % (2 * r as u64 + 1)) as i32 - r;
            let x = (at.0 as i32 + dx).clamp(2, self.map.width as i32 - 3) as u16;
            let y = (at.1 as i32 + dy).clamp(2, self.map.height as i32 - 3) as u16;
            if nav::passable(&self.map, (x, y)) { return (x, y); }
        }
        at
    }

    /// Every other evening the one most given to art sings, plays or leads a dance of their
    /// people's (`history/arts.rs`, carried in `Past::arts`) by the fire; those near listen. It
    /// eases them, more for lovers of art and for those of the same people.
    pub(crate) fn evening_arts(&mut self) {
        let day = self.clock.day();
        // Every other evening, or every evening when the speaker asks for songs.
        if self.mandate != Some(super::society::Mandate::Songs) && crate::history::settlers::hash_pub(day ^ self.seed, 0xA27) % 2 != 0 { return; }
        let camp = self.camp;
        let near = |p: Pos| (p.0 as i32 - camp.0 as i32).abs().max((p.1 as i32 - camp.1 as i32).abs()) <= 8;
        let tick = self.clock.tick;
        // Those taught a work by a visitor are eager to show it (`visitors.rs`).
        let score = |s: &Settler| s.persona.facet(Facet::ArtInclined) as f32 + s.persona.attr(Attr::Musicality) / 40.0
            + if s.past.as_ref().map_or(false, |p| p.arts.iter().any(|a| a.1.starts_with("taught by "))) { 8.0 } else { 0.0 };
        let Some(i) = (0..self.settlers.len()).filter(|&i| {
            let s = &self.settlers[i];
            s.alive && s.mind.broken.is_none() && s.ill_until <= tick && near(s.pos) && s.past.as_ref().map_or(false, |p| !p.arts.is_empty())
                && s.mind.performed + 1 < day.max(1)
        }).max_by(|&a, &b| score(&self.settlers[a]).total_cmp(&score(&self.settlers[b])).then(b.cmp(&a))) else { return };
        if score(&self.settlers[i]) < 55.0 { return; }
        let arts = self.settlers[i].past.as_ref().map(|p| p.arts.clone()).unwrap_or_default();
        let taught: Vec<usize> = (0..arts.len()).filter(|&k| arts[k].1.starts_with("taught by ")).collect();
        let pick = if !taught.is_empty() && day % 3 == 0 { taught[(day as usize / 3) % taught.len()] } else { (crate::history::settlers::hash_pub(day, i as u64) % arts.len() as u64) as usize };
        let (name, what, kind) = arts[pick].clone();
        let way = what.strip_prefix("taught by ").map(|t| format!("as {} taught them", t)).unwrap_or_else(|| "as their people do".into());
        let people = self.settlers[i].past.as_ref().and_then(|p| p.people);
        let listeners: Vec<usize> = (0..self.settlers.len()).filter(|&j| j != i && self.settlers[j].alive && near(self.settlers[j].pos)).collect();
        let who = self.settlers[i].name.clone();
        let verb = match kind { "poem" => "recites", "dance" => "leads a dance,", _ => "plays" };
        self.note(format!("By the fire {} {} {}, {}; {} {}.", who, verb, name, way, listeners.len(), if kind == "dance" { "join in" } else { "listen" }));
        self.settlers[i].mind.performed = day;
        let what = name.clone();
        self.feel(i, Feel::Performed { what: what.clone() });
        for j in listeners {
            let own = self.settlers[j].past.as_ref().and_then(|p| p.people) == people && people.is_some();
            self.feel(j, Feel::Heard { what: what.clone(), own });
            self.warm(i, j);
        }
    }

    /// A festival at the turn of each season (Dwarf Fortress's festivals): with food enough, all
    /// near the fire eat an extra meal and keep their people's dance and music; it eases them by
    /// how much they value merriment and company, and draws everyone closer. Not on the eve of a
    /// foretold raid.
    pub(crate) fn festival(&mut self) {
        let alive: Vec<usize> = (0..self.settlers.len()).filter(|&i| self.settlers[i].alive && self.settlers[i].mind.broken.is_none()).collect();
        if alive.len() < 3 { return; }
        if self.arc.as_ref().map_or(false, |a| a.stage == 2 && a.raid_day <= self.clock.day() + 1) { self.note("No festival this turn of the season: a raid is foretold.".into()); return; }
        // Not in a week the store ran empty.
        if self.food_warned_day > 0 && self.food_warned_day + 7 >= self.clock.day() { self.note("No festival this turn of the season: the store ran empty this week.".into()); return; }
        let food = self.food_stored() as usize;
        let need = if self.mandate == Some(super::society::Mandate::Feasts) { 2 } else { 3 };
        if food < need * alive.len() { self.note(format!("No festival this turn of the season: {} meals stored for {} mouths.", food, alive.len())); return; }
        // Each eats an extra meal.
        for _ in 0..alive.len() {
            if let Some(k) = self.items.iter().rposition(|it| it.kind == ItemKind::Food && it.stored && !it.reserved) { self.items.remove(k); self.fix_refs_pub(k); }
        }
        // The dance and the music: the first settler's people's, if they know any.
        let forms: Vec<(String, String, &'static str)> = alive.iter().find_map(|&i| self.settlers[i].past.as_ref().filter(|p| !p.arts.is_empty()).map(|p| p.arts.clone())).unwrap_or_default();
        let dance = forms.iter().find(|f| f.2 == "dance").map(|f| f.0.clone());
        let music = forms.iter().find(|f| f.2 == "music").map(|f| f.0.clone());
        let season = format!("{:?}", self.season()).to_lowercase();
        let honour = self.temple().map(|(_, god)| format!(" in honour of {}", god)).unwrap_or_default();
        let what = match (&dance, &music) {
            (Some(d), Some(m)) => format!("dance {} and play {}", d, m),
            (Some(d), None) => format!("dance {}", d),
            (None, Some(m)) => format!("play {}", m),
            _ => "sing what songs they know".to_string(),
        };
        // A cup for everyone: the wine is opened (`drink.rs`).
        let wine = self.drink >= alive.len() as u32 && alive.len() > 0;
        if wine { self.drink -= alive.len() as u32; }
        let line = format!("At the turn of {} the camp holds a festival{}: all {} eat together by the fire{}, then {}.", season, honour, alive.len(), if wine { " and open the berry wine" } else { "" }, what);
        self.note(line.clone());
        let at = self.camp;
        self.moment(format!("The {} festival", season), line, format!("because the store held {} meals and the season had turned", food), at);
        let name = dance.or(music).unwrap_or_else(|| format!("the {} festival", season));
        for &i in &alive { self.feel(i, Feel::Festival { what: name.clone() }); }
        for a in 0..alive.len() { for b in a + 1..alive.len() { self.warm(alive[a], alive[b]); } }
    }

    /// Work pace from mood: the high-spirited work a little faster, the miserable slower.
    pub(crate) fn mood_pace(&self, i: usize) -> f32 {
        let st = self.settlers[i].mind.stress;
        if st < -0.6 { 0.92 } else if st > BREAK_AT * 0.66 { 1.12 } else { 1.0 }
    }
}
