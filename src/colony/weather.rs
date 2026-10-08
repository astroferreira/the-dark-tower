//! Evil weather: what drifts over a camp under the Shadow.
//!
//! The idea is Dwarf Fortress's evil regions: clouds and rains that carry a syndrome, harmless
//! under a roof and terrible in the open. Here, where the Shadow's darkness on the tile is 0.35 or
//! more (`Colony::darkness`), from day 20, every 12-24 days (by a hash of the day) at an hour between 10:00 and
//! 17:00 something comes over the camp, its kind by the Shadow and the day: a red rain, a black
//! mist, a cloud of grey ash that stings, a hail of cold that burns. Those under a roof (the
//! huts, the hall in the rock, any roofed building) or within four cells of one (they run for
//! it) are safe; the rest are caught: ill for one to three days by their hardiness, and a dread
//! (`Feel::TheDeep`). In the growing season half the standing crop in the field blackens; a pen
//! of two or more loses a head. The first is a moment; later ones a line naming who was caught.
//! Settlers who sleep by the fire are no safer than those working in the open, so roofs matter.

use super::*;

const KINDS: [(&str, &str); 4] = [
    ("a red rain", "it falls warm on the skin and smells of iron"),
    ("a black mist", "it rolls in low over the ground and the birds stop singing"),
    ("a cloud of grey ash", "it stings the eyes and settles on everything like snow"),
    ("a hail of black ice", "the stones burn where they touch"),
];

impl Colony {
    /// Under a roof, or near enough to reach one.
    fn sheltered(&self, p: Pos) -> bool {
        let d = |a: Pos, w: u16, h: u16| {
            let dx = if p.0 < a.0 { a.0 - p.0 } else if p.0 >= a.0 + w { p.0 - (a.0 + w - 1) } else { 0 };
            let dy = if p.1 < a.1 { a.1 - p.1 } else if p.1 >= a.1 + h { p.1 - (a.1 + h - 1) } else { 0 };
            dx.max(dy)
        };
        if let Some(h) = self.hut.as_ref().filter(|h| h.done) { if d(h.at, HUT_W as u16, HUT_H as u16) <= 4 { return true; } }
        if self.hall_cells.iter().any(|&c| (c.0 as i32 - p.0 as i32).abs().max((c.1 as i32 - p.1 as i32).abs()) <= 4) { return true; }
        self.projects.iter().filter(|q| q.done && matches!(q.kind, projects::ProjectKind::SecondHut | projects::ProjectKind::Storehouse | projects::ProjectKind::Workshop
            | projects::ProjectKind::Temple | projects::ProjectKind::Tavern | projects::ProjectKind::LordsHall | projects::ProjectKind::Smokehouse | projects::ProjectKind::GuildHall))
            .any(|q| Colony::footprint(q.kind).map_or(false, |(w, h)| d(q.at, w, h) <= 4))
    }

    /// The evil weather over the camp now, for the map (the hour it comes and the one after):
    /// 0 red rain, 1 black mist, 2 grey ash, 3 black hail. Read only; mirrors `evil_weather`.
    pub fn evil_weather_over(&self) -> Option<usize> {
        if self.darkness < 0.35 { return None; }
        let day = self.clock.day();
        let h = crate::history::settlers::hash_pub(self.seed ^ 0xE71C, day / 12);
        let start = 10 + h / 12 % 8;
        if day % 12 != h % 12 || day < 20 || self.clock.hour() < start || self.clock.hour() > start + 1 { return None; }
        if self.darkness < 0.5 && (day / 12) % 2 == 1 { return None; }
        Some((h / 96 % KINDS.len() as u64) as usize)
    }

    /// Each hour: an evil cloud may come over.
    pub(crate) fn evil_weather(&mut self) {
        if self.darkness < 0.35 || self.alive() == 0 || self.clock.minute() != 0 { return; }
        let day = self.clock.day();
        let h = crate::history::settlers::hash_pub(self.seed ^ 0xE71C, day / 12);
        // One day in each twelve-day span, at one hour by day.
        if day % 12 != h % 12 || self.clock.hour() != 10 + h / 12 % 8 || day < 20 { return; }
        // Every other span only, where the dark is thin.
        if self.darkness < 0.5 && (day / 12) % 2 == 1 { return; }
        let (what, how) = KINDS[(h / 96 % KINDS.len() as u64) as usize];
        let shadow = self.shadow_name.clone().unwrap_or_else(|| "the Shadow".into());
        let mut caught = Vec::new();
        for i in 0..self.settlers.len() {
            if !self.settlers[i].alive || self.settlers[i].away_until > 0 { continue; }
            let p = self.settlers[i].pos;
            // At the patron's bell everyone is under a roof (`ring_bell`).
            if self.sheltered(p) || self.bell_rung() { continue; }
            let days = (1.0 + 2.0 / self.settlers[i].persona.hardiness().max(0.5)).min(3.0);
            let until = self.clock.tick + (days * TICKS_PER_DAY as f32) as u64;
            self.settlers[i].ill_until = self.settlers[i].ill_until.max(until);
            caught.push(self.settlers[i].name.clone());
        }
        // A sickness more than a horror: dread the first time, after that it is being ill.
        let first = !self.milestones.contains("evil weather");
        for i in 0..self.settlers.len() {
            if self.settlers[i].alive && caught.contains(&self.settlers[i].name) {
                if first { self.feel(i, mind::Feel::TheDeep { what: what.to_string() }); } else { self.feel(i, mind::Feel::Ill); }
            }
        }
        // The field's crop, in the growing season.
        let mut blighted = 0;
        if let Some(at) = self.projects.iter().find(|p| p.kind == projects::ProjectKind::Field && p.done).map(|p| p.at) {
            let n = self.map.width;
            for dy in 1..5u16 { for dx in 1..7u16 {
                let (x, y) = ((at.0 + dx) as usize, (at.1 + dy) as usize);
                if x >= n || y >= self.map.height || (dx + dy) % 2 == 0 { continue; }
                let k = self.map.idx(x, y, self.map.surface_z[y * n + x].max(0) as usize);
                if matches!(self.map.cells[k].plant, Plant::Crop(_)) { self.map.cells[k].plant = Plant::None; blighted += 1; }
            } }
        }
        let mut beast = String::new();
        if let Some((kind, head)) = self.pen.clone().filter(|p| p.1 >= 2) { self.pen = Some((kind.clone(), head - 1)); beast = kind; }
        let mut tail = Vec::new();
        if blighted > 0 { tail.push(format!("half the grain blackens in the field")); }
        if !beast.is_empty() { tail.push(format!("a {} in the pen sickens and dies", beast)); }
        let tail = if tail.is_empty() { String::new() } else { format!("; {}", tail.join(", and ")) };
        let who = if caught.is_empty() { "Everyone is under a roof in time".to_string() }
            else { format!("{} {} caught in the open and {} ill", if caught.len() > 4 { format!("{}, {}, {} and {} others", caught[0], caught[1], caught[2], caught.len() - 3) } else { crate::persona::list(&caught) }, if caught.len() == 1 { "is" } else { "are" }, if caught.len() == 1 { "falls" } else { "fall" }) };
        let line = format!("Out of the {} sky comes {}: {}. {}{}.", if self.clock.hour() < 12 { "morning" } else { "afternoon" }, what, how, who, tail);
        self.note(line.clone());
        if self.milestones.insert("evil weather") {
            let at = self.camp;
            self.moment(format!("{}", capital_first(what)), line, format!("because the darkness of {} lies on this land", shadow), at);
        }
    }
}

fn capital_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() { Some(f) => f.to_uppercase().collect::<String>() + c.as_str(), None => String::new() }
}
