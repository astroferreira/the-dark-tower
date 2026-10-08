//! Written works: histories, treatises and the camp's own chronicle.
//!
//! The idea is Dwarf Fortress's written content: scholars write books with authors, dates and
//! subjects (histories of real events, treatises on skills, poems), which then exist in the world
//! as items. Here a learned settler (who values knowledge, or of analytical ability 1300+; 16+),
//! with a quiet place to write (a temple, a tavern, a lord's hall or a hall in the hill), may spend
//! spare daylight hours writing (`Job::Craft` "Writing ...", one at a time). The book is a work
//! (`craft::Work`, kind "book", of bark, or of hide once beasts are hunted) with a title and a
//! subject: once the camp has seen a year and has none, "The Chronicle of <camp>" (its moments);
//! else a history of an event of the writer's past ("On the siege of X"), else a treatise on their
//! best trade ("A Treatise on Felling"). Quality as for works. Caravans buy books like other works;
//! the annals list them.

use super::*;
use crate::persona::{Attr, Val};

const TRADES: [&str; 5] = ["Gathering", "Fishing", "Felling", "Carrying Loads", "Building"];

impl Colony {
    fn quiet_place(&self) -> Option<Pos> {
        self.temple_at().or_else(|| self.tavern()).or_else(|| self.hall_cells.first().copied())
            .or_else(|| self.projects.iter().find(|p| p.done && p.kind == projects::ProjectKind::LordsHall).map(|p| (p.at.0 + 2, p.at.1 + 2)))
    }

    fn learned(&self, i: usize) -> bool {
        let s = &self.settlers[i];
        s.past.as_ref().map_or(true, |p| p.age >= 16) && (s.persona.value(Val::Knowledge) >= 20 || s.persona.attr(Attr::AnalyticalAbility) >= 1300.0)
    }

    /// The writer's option.
    pub(crate) fn write_option(&self, i: usize) -> Option<(f32, Job, String)> {
        if self.clock.is_night() || !self.learned(i) || self.quiet_place().is_none() { return None; }
        if self.settlers.iter().enumerate().any(|(j, s)| j != i && s.alive && s.job == Job::Craft && s.why.starts_with("Writing")) { return None; }
        // A book a season at most, and only with something new to write.
        let last = self.works.iter().filter(|w| w.maker == i && w.kind == "book").map(|w| w.day).max();
        // (Twice a season with a library to write in.)
        let library = self.projects.iter().any(|p| p.done && p.kind == projects::ProjectKind::Library);
        if last.map_or(false, |d| self.clock.day() < d + if library { SEASON_DAYS / 2 } else { SEASON_DAYS }) { return None; }
        let (title, _) = self.book_subject(i);
        if self.works.iter().any(|w| w.called.as_deref() == Some(title.as_str())) { return None; }
        Some((0.35 + 0.2 * (self.settlers[i].persona.value(Val::Knowledge) as f32 / 50.0).max(0.0), Job::Craft, format!("Writing {}", title)))
    }

    /// What settler `i` would write: the chronicle, a history, or a treatise.
    fn book_subject(&self, i: usize) -> (String, Option<(String, Option<crate::history::EventId>)>) {
        let camp = self.name.clone().unwrap_or_else(|| format!("the Camp at {},{}", self.map.world_tile.0, self.map.world_tile.1));
        let has = |t: &str| self.works.iter().any(|w| w.called.as_deref() == Some(t));
        let chronicle = format!("The Chronicle of {}", camp);
        if self.clock.day() > 120 && !has(&chronicle) { return (chronicle, Some(("the camp's first years".into(), None))); }
        // Histories of the scenes of their past that read as such ("the siege of X").
        if let Some((t, e)) = self.settlers[i].past.as_ref().and_then(|p| p.images.iter().find(|(t, _)| t.starts_with("the ") && !t.contains(':') && !has(&format!("On {}", t))).cloned()) {
            return (format!("On {}", t), Some((t, Some(e))));
        }
        let best = (0..5).max_by(|&a, &b| self.settlers[i].skill[a].total_cmp(&self.settlers[i].skill[b])).unwrap_or(4);
        (format!("A Treatise on {}", TRADES[best]), None)
    }

    /// The book is finished.
    pub(crate) fn finish_book(&mut self, i: usize) {
        let (title, image) = self.book_subject(i);
        let s = &self.settlers[i];
        let p = &s.persona;
        let h = crate::history::settlers::hash_pub(self.seed ^ self.clock.tick, 0xB00C + i as u64);
        let q = 0.3 * (p.attr(Attr::AnalyticalAbility) / 2000.0).min(1.0) + 0.25 * (p.attr(Attr::LinguisticAbility) / 2000.0).min(1.0)
            + 0.2 * (p.attr(Attr::Creativity) / 2000.0).min(1.0) + 0.25 * (h % 1000) as f32 / 1000.0;
        let quality = (((q - 0.3) * 9.0).floor() as i32).clamp(0, 5) as u8;
        let material = if self.hunted > 0 { "hide" } else { "bark" }.to_string();
        let day = self.clock.day();
        let name = s.name.clone();
        let work = craft::Work { maker: i, kind: "book".into(), material, quality, image: image.clone(), day, called: Some(title.clone()), traded: false };
        let kind = format!("{}book of {}", craft::QUALITY[quality as usize], work.material);
        let kind = format!("{} {}", if kind.starts_with(|c: char| "aeiou".contains(c)) { "an" } else { "a" }, kind);
        self.settlers[i].made.push(format!("{}, {} (day {})", title, kind, day));
        self.works.push(work);
        self.feel(i, mind::Feel::Made { what: title.clone(), quality });
        let line = format!("{} finishes writing {} ({}).", name, title, kind);
        self.note(line.clone());
        if title.starts_with("The Chronicle of") {
            let at = self.settlers[i].pos;
            self.moment(title.clone(), line, format!("because {} would have the camp's years remembered", name), at);
        }
    }
}
