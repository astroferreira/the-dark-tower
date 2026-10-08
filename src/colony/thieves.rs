//! Thieves in the night: a hostile people's thief comes for the camp's artifact.
//!
//! The idea is Dwarf Fortress's artifact thieves: once a fortress holds artifacts, thieves of
//! hostile civilizations sneak in for them, and a fortress that does not catch them loses them.
//! Here, twenty days after an artifact is made (a strange mood's work), at 23:00 one night in
//! forty (sixty days at least between attempts), a thief of a people among the camp's troubles comes for it (`artifact_thief`; peoples
//! that steal children first, then any war band's people). Someone awake within six cells of
//! the camp (the watch, a sleepless soul) catches the thief, who becomes the camp's prisoner
//! (`prisoners.rs`: judged at the next dawn). Else the artifact is gone ("carried off by a thief
//! of X", a moment; its maker grieves, the camp hears it as bad news; `Colony::stolen`). When a
//! war band of that people is later routed, one time in two the artifact is found among what
//! they leave behind (`recover_stolen`, from `raid_at`).

use super::*;
use crate::history::FactionId;

impl Colony {
    /// A work that is an artifact (a mood's).
    pub(crate) fn is_artifact(w: &craft::Work) -> bool {
        w.quality == 5 && w.called.is_some() && ["altar", "statue", "chest", "throne", "standing totem"].contains(&w.kind.as_str())
    }

    /// The people a thief would come from: one that steals children, else any war band's.
    fn thief_people(&self) -> Option<(FactionId, String)> {
        let a = self.arc.as_ref()?;
        let bands: Vec<&arc::Threat> = std::iter::once(&a.threat).chain(a.later.iter()).chain(a.reserve.iter())
            .filter(|t| t.kind == arc::ThreatKind::Warband && t.faction.is_some())
            // Not a people that came in friendship (`regard.rs`).
            .filter(|t| !self.regards.iter().any(|r| Some(r.faction) == t.faction && r.acted == Some(true))).collect();
        let pick = bands.iter().find(|t| self.snatchers.contains(&t.faction.unwrap())).or(bands.first())?;
        let name = pick.name.trim_start_matches("a war band of ").split(", led by ").next().unwrap_or("").to_string();
        Some((pick.faction?, name))
    }

    /// 23:00: a thief may come for an artifact.
    pub(crate) fn artifact_thief(&mut self) {
        let day = self.clock.day();
        let Some(k) = (0..self.works.len()).find(|&k| Self::is_artifact(&self.works[k]) && day >= self.works[k].day + 20
            && !self.stolen.iter().any(|s| Some(&s.0) == self.works[k].called.as_ref())) else { return };
        if crate::history::settlers::hash_pub(self.seed ^ day, 0x7E1F) % 40 != 0 || (self.thief_day > 0 && day < self.thief_day + 60) { return; }
        let Some((faction, people)) = self.thief_people() else { return };
        self.thief_day = day;
        let title = self.works[k].called.clone().unwrap_or_default();
        let camp = self.camp;
        let near = |p: Pos| (p.0 as i32 - camp.0 as i32).abs().max((p.1 as i32 - camp.1 as i32).abs()) <= 6;
        let awake = (0..self.settlers.len()).find(|&i| self.settlers[i].alive && self.settlers[i].job != Job::Sleep && near(self.settlers[i].pos))
            .or(self.watcher.filter(|&w| self.settlers[w].alive));
        let thief = {
            use rand::SeedableRng;
            let style = crate::history::naming::styles::NamingStyle::from_archetype(crate::history::NamingStyleId(0), crate::history::entities::races::RaceType::from_tag("goblin").default_naming_archetype());
            let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(self.seed ^ day ^ 0x7E20);
            crate::history::naming::generator::NameGenerator::personal_name(&style, &mut rng).split_whitespace().next().unwrap_or("Snag").to_string()
        };
        match awake {
            Some(w) if self.prisoner.is_none() => {
                let wn = self.settlers[w].name.clone();
                let line = format!("{} catches a thief at the edge of the camp with {} in their arms: {}, of {}, is bound and held.", wn, title, thief, people);
                self.note(line.clone());
                let at = self.settlers[w].pos;
                self.moment(format!("A thief is caught"), line, format!("because {} was awake when the thief came for {}", wn, title), at);
                self.prisoner = Some(prisoners::Prisoner { name: thief, people, faction: Some(faction), day, from: None, ransom_day: None });
                self.feel(w, mind::Feel::Saved { whom: title });
            }
            Some(w) => {
                let wn = self.settlers[w].name.clone();
                self.note(format!("{} hears someone at {} in the night and shouts; a shape runs off into the dark with empty hands.", wn, title));
            }
            None => {
                self.stolen.push((title.clone(), faction, day));
                let maker = self.works[k].maker;
                let line = format!("In the night {} is gone: a thief of {} carried it off while the camp slept.", title, people);
                self.note(line.clone());
                self.moment(format!("{} is stolen", title), line, format!("because {} want what the camp has made", people), camp);
                for j in 0..self.settlers.len() {
                    if !self.settlers[j].alive { continue; }
                    if j == maker { self.feel(j, mind::Feel::Death { whom: format!("{}, the work of {} hands", title, if self.settlers[j].persona.female { "her" } else { "his" }), close: true }); }
                    else { self.feel(j, mind::Feel::News { what: format!("{} was stolen in the night", title), good: false }); }
                }
            }
        }
    }

    /// A routed war band of the thieves' people may leave the stolen artifact behind.
    pub(crate) fn recover_stolen(&mut self, faction: Option<FactionId>, routed: bool, seed: u64) {
        let Some(f) = faction else { return };
        if !routed { return; }
        let Some(k) = self.stolen.iter().position(|s| s.1 == f) else { return };
        if crate::history::settlers::hash_pub(seed, 0x7E21) % 2 != 0 { return; }
        let (title, _, day) = self.stolen.remove(k);
        let line = format!("Among what the fleeing raiders leave behind is {}, stolen on day {}: it comes home.", title, day);
        self.note(line.clone());
        let at = self.camp;
        self.moment(format!("{} comes home", title), line, "because the camp broke the band of the people who took it".into(), at);
        for j in 0..self.settlers.len() { if self.settlers[j].alive { self.feel(j, mind::Feel::News { what: format!("{} came home", title), good: true }); } }
    }
}
