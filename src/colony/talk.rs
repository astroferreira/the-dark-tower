//! Talk (Dwarf Fortress's conversations): two settlers who sit down together talk about
//! something. What they share comes up first: a moment of the camp's they both lived through
//! (the raid, a death, the first harvest), the town they both came from, a value both hold dear.
//! Where their values pull apart, the talk turns into an argument. Agreement and shared grief
//! draw them closer; an argument sets them apart, and between the quarrelsome it is said aloud.
//! The topic is chosen when they sit down (it is in the settler's why: "Arguing with Thano by the
//! woodpile about the law"), its effect when they get up (`complete_need`).

use super::*;

/// What a talk did to the two.
#[derive(Clone, Debug, PartialEq)]
pub enum Topic {
    /// A moment both lived through; `grief`: a death of someone dear to either.
    Memory { what: String, grief: bool },
    /// A home both came from.
    Home { town: String },
    /// A value both hold dear (its index).
    Agree { value: usize },
    /// A value one holds dear and the other scorns.
    Argue { value: usize },
    /// Nothing much: the day's work, the weather.
    Small,
}

impl Colony {
    /// What settlers `i` and `j` would talk about now: (the verb, "about ...", the topic).
    pub(crate) fn talk_topic(&self, i: usize, j: usize) -> (&'static str, String, Topic) {
        let (a, b) = (&self.settlers[i].persona, &self.settlers[j].persona);
        let h = crate::history::settlers::hash_pub(self.seed ^ ((i as u64) << 20 | j as u64), self.clock.day());
        let n = crate::persona::N_VALUES;
        // The sharpest disagreement and the warmest agreement among their values.
        let argue = (0..n).filter(|&k| (a.values[k] as i32) * (b.values[k] as i32) < 0 && (a.values[k] as i32).abs() >= 20 && (b.values[k] as i32).abs() >= 20)
            .max_by_key(|&k| ((a.values[k] as i32 - b.values[k] as i32).abs(), k));
        let agree = (0..n).filter(|&k| a.values[k] >= 25 && b.values[k] >= 25).max_by_key(|&k| (a.values[k] as i32 + b.values[k] as i32, k));
        // The quarrelsome argue more readily (discord, anger), the friendly less.
        let heat = (a.facet(crate::persona::Facet::Discord) as u64 + a.facet(crate::persona::Facet::Anger) as u64) / 2;
        let memory = self.moments.iter().rev().take_while(|m| self.clock.tick.saturating_sub(m.tick) <= 20 * TICKS_PER_DAY)
            .find(|m| !m.choice && MOVING.iter().any(|w| m.title.to_lowercase().contains(w)));
        // A home both name in their callings ("a survivor of Ripu").
        let place_of = |c: &str| c.rsplit_once(" of ").map(|x| x.1.to_string()).filter(|t| t.chars().next().map_or(false, |f| f.is_uppercase()));
        let home = match (self.settlers[i].past.as_ref(), self.settlers[j].past.as_ref()) {
            (Some(p), Some(q)) => place_of(&p.calling).filter(|t| place_of(&q.calling).as_deref() == Some(t.as_str())),
            _ => None,
        };
        let roll = h % 100;
        if let Some(k) = argue.filter(|_| roll < 15 + heat / 3) {
            let v = value_word(k);
            return ("Arguing", format!("about {}", v), Topic::Argue { value: k });
        }
        if let Some(m) = memory.filter(|_| roll % 3 == 0) {
            let what = format!("{} (day {})", lower(&m.title), m.tick / TICKS_PER_DAY + 1);
            let grief = m.title.starts_with("The death of") || m.text.contains("was killed");
            return ("Talking", format!("about {}", what), Topic::Memory { what, grief });
        }
        if let Some(town) = home.filter(|_| roll % 3 == 1) {
            return ("Talking", format!("about {}, where both of them come from", town), Topic::Home { town });
        }
        if let Some(k) = agree.filter(|_| roll % 3 == 2) {
            let v = value_word(k);
            return ("Talking", format!("about {}, which both hold dear", v), Topic::Agree { value: k });
        }
        let small = ["the day's work", "the weather", "the food", "the others", "the land round the camp"];
        ("Passing the time", format!("talking of {}", small[(h / 100) as usize % small.len()]), Topic::Small)
    }

    /// The talk between `i` and `j` is over: its topic moves them.
    pub(crate) fn talk_done(&mut self, i: usize, j: usize, topic: &Topic, place: &str) {
        let (a, b) = (self.settlers[i].name.clone(), self.settlers[j].name.clone());
        let day = self.clock.day();
        let said = |c: &Colony| c.talks_said.iter().any(|&(x, y, d)| ((x, y) == (i, j) || (x, y) == (j, i)) && day < d + 20);
        match topic {
            Topic::Argue { value } => {
                self.like(i, j, -3);
                self.like(j, i, -3);
                let v = value_word(*value);
                let (pa, pb) = (&self.settlers[i].persona, &self.settlers[j].persona);
                let (hold, scorn) = if pa.values[*value] > 0 { (format!("{} holds {} dear", a, v), format!("{} has no use for it", b)) } else { (format!("{} has no use for {}", a, v), format!("{} holds it dear", b)) };
                let hot = pa.facet(crate::persona::Facet::Anger) >= 65 || pb.facet(crate::persona::Facet::Anger) >= 65;
                if !said(self) {
                    self.note(format!("{} and {} argue {} about {}: {}; {}{}.", a, b, place, v, hold, scorn, if hot { ", and voices are raised" } else { "" }));
                    self.talks_said.push((i, j, day));
                }
                if hot { self.feel(i, super::mind::Feel::Quarrel { with: b.clone() }); self.feel(j, super::mind::Feel::Quarrel { with: a.clone() }); }
            }
            Topic::Agree { .. } | Topic::Home { .. } => {
                self.like(i, j, 2);
                self.like(j, i, 2);
            }
            Topic::Memory { what, grief } => {
                if *grief {
                    // Grief shared is lighter: the closer they are, the more it helps.
                    self.like(i, j, 2);
                    self.like(j, i, 2);
                    if !said(self) && self.opinion(i, j) >= 12 {
                        self.note(format!("{} and {} sit together {} and talk about {}.", a, b, place, what));
                        self.talks_said.push((i, j, day));
                    }
                    self.feel(i, super::mind::Feel::Reconciled { by: b.clone() });
                } else {
                    self.warm(i, j);
                    self.warm(j, i);
                }
            }
            Topic::Small => { self.warm(i, j); self.warm(j, i); }
        }
    }

}

/// The moments people talk over afterwards.
const MOVING: [&str; 14] = ["raid", "death", "died", "born", "wed", "climbs", "masterwork", "festival", "siege", "falls", "fever", "migrants", "caravan", "ghost"];

/// A value's name as said in a sentence ("martial prowess").
fn value_word(k: usize) -> String {
    // ("MartialProwess" or "martial prowess" alike: spaced at the capitals, lower case.)
    let mut out = String::new();
    for (n, ch) in crate::persona::Persona::value_name_at(k).chars().enumerate() {
        if ch.is_uppercase() && n > 0 && !out.ends_with(' ') { out.push(' '); }
        out.extend(ch.to_lowercase());
    }
    out
}

fn lower(s: &str) -> String { let mut c = s.chars(); c.next().map(|f| f.to_lowercase().collect::<String>() + c.as_str()).unwrap_or_default() }

impl Colony {
    /// Idle hours spent talking (DF's breaks): with nothing to do, a settler sits down with
    /// someone near who is idle, eating or resting too, the dearest first; in the evening by the
    /// fire the pull is stronger. As an option for `decide`, with its act (a talk meets company).
    pub(crate) fn idle_talk(&self, i: usize) -> Option<((f32, Job, String), super::needs::NeedAct)> {
        let s = &self.settlers[i];
        if self.clock.is_night() || self.drill_due(i) || s.ill_until > self.clock.tick || s.hunger >= 0.6 || s.past.as_ref().map_or(false, |p| p.age < 6) || self.below(i) { return None; }
        let me = s.pos;
        let idle = |j: usize| {
            let o = &self.settlers[j];
            j != i && o.alive && !o.mind.left && o.away_until == 0 && o.ill_until <= self.clock.tick && !self.below(j)
                && matches!(o.job, Job::Wander(_) | Job::Idle | Job::Eat) && super::needs::cheb(o.pos, me) <= 12
        };
        let j = (0..self.settlers.len()).filter(|&j| idle(j)).max_by_key(|&j| (self.opinion(i, j), std::cmp::Reverse(j)))?;
        let o = &self.settlers[j];
        let (verb, about, topic) = self.talk_topic(i, j);
        let evening = (18..21).contains(&self.clock.hour());
        let w = if evening { 0.3 } else { 0.06 };
        let why = format!("{} with {} {} {}", verb, o.name, self.place_word(o.pos), about);
        let act = super::needs::NeedAct { need: super::needs::Need::Socialize, at: o.pos, with: Some(j), what: o.name.clone(), minutes: 40, topic: Some(topic) };
        Some(((w, Job::Wander(o.pos), why), act))
    }
}
