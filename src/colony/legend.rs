//! Legends of the camps: what a colony did, kept with the world.
//!
//! The idea is Dwarf Fortress's legends: a fortress's deeds join the world's history, so a later
//! fortress in the same world finds the beast its predecessor slew already dead and hears songs
//! of it. The history itself stays as generated (codes rebuild worlds exactly); a colony's legend
//! is kept beside it in `colonies/legends_<seed>.json` when the window leaves a colony
//! (`Colony::legend`, `save`), loaded with the world (`load`), shown on the camp's tile, and heeded
//! by later colonies of the window (`heed_legends`): the beasts slain no longer come, a werebeast
//! slain no longer howls, a relic carried off by an earlier camp is not found again, the world's
//! peoples hold half of what they thought of earlier camps (twelve at most) for or against the
//! newcomers (`Legend::regards`), and the newcomers know the nearest earlier camp's tale. Headless runs and `--code` replays do not read
//! legends, so codes still give the same colony.

use super::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Legend {
    pub code: String,
    pub name: String,
    pub tile: (usize, usize),
    pub day: u64,
    pub alive: usize,
    pub came: usize,
    /// "lives", "fell on day 80", "left the land on day 40".
    pub fate: String,
    /// The great moments, "Day 52: Babagk drives a fishing spear into ...".
    pub deeds: Vec<String>,
    pub slain: Vec<String>,
    /// The relic found there and whether it was kept.
    pub relic: Option<String>,
    /// What the world's peoples came to think of it (`regard.rs`): (people, their name, the sum,
    /// its weightiest cause). Older legends have none.
    #[serde(default)]
    pub regards: Vec<(crate::history::FactionId, String, i32, String)>,
}

fn path(seed: u64) -> String { format!("colonies/legends_{}.json", seed) }

/// The legends kept for the world of `seed`.
pub fn load(seed: u64) -> Vec<Legend> {
    std::fs::read_to_string(path(seed)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

/// Keep `legend`, replacing an earlier telling of the same camp.
pub fn save(seed: u64, legends: &mut Vec<Legend>, legend: Legend) -> std::io::Result<()> {
    legends.retain(|l| l.code != legend.code);
    legends.push(legend);
    std::fs::create_dir_all("colonies")?;
    std::fs::write(path(seed), serde_json::to_string_pretty(legends).unwrap_or_default())
}

impl Colony {
    /// This camp's legend so far.
    pub fn legend(&self, code: &str) -> Legend {
        let name = self.name.clone().unwrap_or_else(|| format!("the camp at {},{}", self.map.world_tile.0, self.map.world_tile.1));
        let fate = if let Some(d) = self.departed { format!("left the land on day {}", d) }
            else if self.alive() == 0 { format!("fell by day {}", self.clock.day()) }
            else { "lives".to_string() };
        let great = |t: &str| t.starts_with("The death of") || t.ends_with(" is found") || t == "The raid" || t.starts_with("They break into")
            || t.ends_with(" comes") || t.starts_with("The wedding") || t.ends_with(" is born") || t.contains("artifact") || t.starts_with("Water in the rock") || t.ends_with(" is cast out")
            || t == "The siege" || t == "The sally" || t.ends_with(" is deposed") || t.ends_with(" keeps the vow") || t.ends_with(" is taken") || t.ends_with(" comes home")
            || t.contains(" falls in ") || t.ends_with(" is stolen") || t.ends_with(" swear vengeance") || t.starts_with("The friendship of") || t.ends_with("'s dream");
        let deeds = self.moments.iter().filter(|m| great(&m.title) || m.text.contains("has made an artifact"))
            .map(|m| format!("Day {}: {}", m.tick / TICKS_PER_DAY + 1, m.text)).take(16).collect();
        let relic = self.relic.as_ref().filter(|r| r.found.is_some()).map(|r| format!("{}{}", r.name, r.fate.as_ref().map(|f| format!(", {}", f)).unwrap_or_else(|| ", kept in the camp".into())));
        let regards = self.regards.iter().filter(|r| r.total().abs() >= 10).map(|r| {
            let cause = r.causes.iter().max_by_key(|c| (c.delta.abs(), c.day)).map(|c| c.text.clone()).unwrap_or_default();
            (r.faction, r.people.clone(), r.total(), cause)
        }).collect();
        Legend { code: code.to_string(), name, tile: self.map.world_tile, day: self.clock.day(), alive: self.alive(), came: self.company(), fate, deeds, slain: self.slain.clone(), relic, regards }
    }

    /// A new camp heeds the legends of the world: the slain stay dead, the taken stay taken, and
    /// the nearest earlier camp's tale is known.
    pub fn heed_legends(&mut self, legends: &[Legend]) {
        let slain: Vec<&String> = legends.iter().flat_map(|l| l.slain.iter()).collect();
        if let Some(a) = self.arc.as_mut() {
            a.later.retain(|t| !slain.contains(&&t.name));
            a.reserve.retain(|t| !slain.contains(&&t.name));
            if slain.contains(&&a.threat.name) && !a.later.is_empty() { a.threat = a.later.remove(0); }
        }
        if self.were.as_ref().map_or(false, |w| slain.contains(&w)) { self.were = None; }
        let taken = |r: &relic::Relic| legends.iter().any(|l| l.relic.as_ref().map_or(false, |x| x.starts_with(&r.name)));
        if self.relic.as_ref().map_or(false, |r| taken(r)) { self.relic = None; }
        // Peoples remember what earlier camps did: half of it, twelve at most, holds against
        // (or for) the newcomers (`regard.rs`).
        for l in legends {
            for (f, people, total, cause) in &l.regards {
                let d = (total / 2).clamp(-12, 12);
                if d == 0 { continue; }
                self.regard(Some(*f), people, None, "legend", d, format!("is remembered for {}: it {}", l.name, cause));
            }
        }
        // The nearest earlier camp's tale.
        let here = self.map.world_tile;
        if let Some(l) = legends.iter().filter(|l| l.tile != here || l.code.is_empty()).min_by_key(|l| l.tile.0.abs_diff(here.0) + l.tile.1.abs_diff(here.1)) {
            let deed = l.deeds.iter().rev().find(|d| d.contains("does not rise") || d.contains(" finds ")).or(l.deeds.last());
            let line = match deed {
                Some(d) => format!("They know the songs of {}, {} tiles away: {}", l.name, l.tile.0.abs_diff(here.0) + l.tile.1.abs_diff(here.1), d.split_once(": ").map(|x| x.1).unwrap_or(d)),
                None => format!("They know of {}, {} tiles away, which {}.", l.name, l.tile.0.abs_diff(here.0) + l.tile.1.abs_diff(here.1), l.fate),
            };
            self.note(line);
        }
    }
}
