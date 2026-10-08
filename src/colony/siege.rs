//! Sieges: a war band that camps outside the walls instead of striking at once.
//!
//! The idea is Dwarf Fortress's sieges: an army arrives and waits at the edge of the map, the
//! fortress is cut off from the surface, and it must hold out on what it has stored, send its
//! militia out, or wait for the assault. Here, after the first trouble, on the eve of a raid
//! against a closed palisade by the Shadow's raiders or a war band of five or more (a camp of
//! sixteen), one time in two (`siege_begins`), the raiders do not come in the night: they light
//! their fires 25 cells out on their side and stand there (`CreatureKind::Besieger`; "The camp is
//! besieged", a moment; a mark where their camp is), and the assault is put off 3-5 days. While
//! the siege lasts, no one works more than 12 cells from the fire (`under_siege_out`: the ground
//! beyond is as if forbidden), so the camp lives on its store, its field, its farm under the rock
//! and its pen. Each dawn (`reckon_siege`) the camp counts the days and the meals; a speaker who
//! is brave (50+) or prizes arms, with three drilled spears or more, leads a sally out of the gate
//! (the raid fought at once at the besiegers' camp, +0.15 to readiness); on the last dawn before
//! the assault, a camp ready enough (0.85+) sees the besiegers' fires go cold (they leave, a
//! moment, no raid); else the assault comes that night, the siege's days counting as nights of
//! watch. A camp whose store is empty before then is found out: the assault comes that night.

use super::*;

#[derive(Clone, Debug)]
pub struct Siege {
    pub since: u64,
    pub until: u64,
    pub who: String,
    /// Where their fires are.
    pub at: Pos,
    pub sally: bool,
}

impl Colony {
    /// On the raid's eve: a war band or the Shadow's raiders against a closed palisade camp
    /// outside it.
    pub(crate) fn siege_begins(&mut self) -> bool {
        let Some(a) = self.arc.as_ref() else { return false };
        // Not the first trouble: a young camp meets its first raid in the night.
        if !matches!(a.threat.kind, arc::ThreatKind::Warband | arc::ThreatKind::Shadow) || self.siege.is_some() || a.chapter == 0 { return false; }
        let walled = self.projects.iter().any(|p| p.done && p.kind == projects::ProjectKind::Palisade);
        // The Shadow's raiders, or a band of five or more (a camp of sixteen).
        if !walled || (a.threat.kind == arc::ThreatKind::Warband && self.band_size() < 5) { return false; }
        let day = self.clock.day();
        if crate::history::settlers::hash_pub(self.seed ^ day, 0x5E1D) % 2 != 0 { return false; }
        let len = 3 + crate::history::settlers::hash_pub(self.seed ^ day, 0x5E1E) % 3;
        // "a war band of X, led by Y", "raiders of the Shadow of Z".
        let who = a.threat.name.clone();
        let ((dx, dy), side) = self.threat_side_pub();
        let at = self.passable_near_pub(((self.camp.0 as f32 + dx * 25.0) as i32, (self.camp.1 as f32 + dy * 25.0) as i32)).unwrap_or(self.camp);
        if let Some(a) = self.arc.as_mut() { a.raid_day = day + 1 + len; }
        self.siege = Some(siege::Siege { since: day + 1, until: day + 1 + len, who: who.clone(), at, sally: false });
        // They stand at their fires, in sight of the palisade.
        let n = self.band_size();
        for k in 0..n {
            let off = (k as f32 - (n as f32 - 1.0) / 2.0) * 2.0;
            let Some(p) = self.passable_near_pub(((at.0 as f32 - dy * off) as i32, (at.1 as f32 + dx * off) as i32)) else { continue };
            let id = self.new_creature_id();
            self.creatures.push(creatures::Creature { kind: creatures::CreatureKind::Besieger, name: who.clone(), pos: p, path: Vec::new(), stride: 0, leaving: false, size: 1.0, home: p, spawned: self.clock.tick, id, z: None, path3: Vec::new(), home_z: 0, out: false, rest_until: 0 });
        }
        self.marks.push(ColonyMark { at, kind: MarkKind::Scorch, title: "The besiegers' camp".into(),
            text: format!("Where {} lit their fires and waited, from day {}.", who.split(", led by ").next().unwrap_or(""), day + 1), day });
        let line = format!("{}{} not come in the night: at dusk their fires are lit at the edge of the clearing, out of {}. The camp is besieged; no one goes beyond the palisade.",
            arc_capital(&who), if who.contains(", led by ") { ", does" } else if who.starts_with("a ") { " does" } else { " do" }, side);
        self.note(line.clone());
        self.moment("The siege".into(), line, "because the palisade is closed and they came too many to be driven off in a night".into(), at);
        // Up go the bridges over the ditch (`delve.rs`).
        self.set_bridges(true);
        for j in 0..self.settlers.len() { if self.settlers[j].alive { self.feel(j, mind::Feel::News { what: "the camp is besieged".into(), good: false }); } }
        true
    }

    /// While besieged: ground more than 12 cells from the fire is out of bounds.
    pub(crate) fn under_siege_out(&self, p: Pos) -> bool {
        self.siege.as_ref().map_or(false, |_| (p.0 as i32 - self.camp.0 as i32).abs().max((p.1 as i32 - self.camp.1 as i32).abs()) > 12)
    }

    /// Dawn: the days and meals counted; a sally, the fires gone cold, or the assault.
    pub(crate) fn reckon_siege(&mut self) {
        let Some(s) = self.siege.clone() else { if self.bridges_up { self.set_bridges(false); } return };
        // (Bridges not yet up while someone was outside: try again.)
        self.set_bridges(true);
        let day = self.clock.day();
        if day < s.since || self.alive() == 0 { return; }
        if self.arc.as_ref().map_or(true, |a| a.stage != 2) { self.creatures.retain(|c| c.kind != creatures::CreatureKind::Besieger); self.siege = None; return; }
        let food = self.food_stored();
        let alive = self.alive() as u32;
        // Brave hands out of the gate.
        let speaker = self.speaker.filter(|&k| self.settlers[k].alive);
        let bold = speaker.map_or(false, |k| { let p = &self.settlers[k].persona; p.facet(crate::persona::Facet::Bravery) >= 50 || p.value(crate::persona::Val::MartialProwess) > 0 });
        let (_, spears) = self.militia_ready();
        if bold && spears >= 3 && day > s.since {
            let k = speaker.unwrap();
            let line = format!("At first light {} leads {} spears out of the gate against the camp of {}.", self.settlers[k].name, spears, s.who.split(", led by ").next().unwrap_or(""));
            self.note(line.clone());
            self.moment("The sally".into(), line, format!("because {} would not wait to be starved", self.settlers[k].name), s.at);
            if let Some(x) = self.siege.as_mut() { x.sally = true; }
            self.set_bridges(false);
            self.creatures.retain(|c| c.kind != creatures::CreatureKind::Besieger);
            self.raid_at(Some(s.at));
            self.siege = None;
            return;
        }
        // Out of food before the assault: the besiegers see it.
        if food == 0 && day + 1 < s.until {
            self.note(format!("The store is empty on day {} of the siege, and the besiegers know it: they will come tonight.", day - s.since + 1));
            if let Some(a) = self.arc.as_mut() { a.raid_day = day + 1; }
            if let Some(x) = self.siege.as_mut() { x.until = day + 1; }
            return;
        }
        if day + 1 == s.until {
            let (ready, _, _) = self.readiness();
            // (An island behind raised bridges wears a siege out sooner.)
            if ready >= if self.bridges_up { 0.75 } else { 0.85 } {
                let line = format!("In the night the besiegers' fires went cold: seeing the walls manned and the spears ready, {} have gone.", s.who.split(", led by ").next().unwrap_or(""));
                self.note(line.clone());
                self.moment("The siege lifts".into(), line, format!("because the camp held out {} days and was ready for them", day - s.since + 1), s.at);
                if let Some(a) = self.arc.as_mut() { a.stage = 3; }
                self.creatures.retain(|c| c.kind != creatures::CreatureKind::Besieger);
                self.siege = None;
                return;
            }
        }
        self.note(format!("Day {} of the siege: the fires of {} still burn at the edge of the clearing; {} meals in the store for {}.", day - s.since + 1, s.who.split(", led by ").next().unwrap_or(""), food, alive));
    }
}

fn arc_capital(s: &str) -> String {
    let mut c = s.chars();
    match c.next() { Some(f) => f.to_uppercase().collect::<String>() + c.as_str(), None => String::new() }
}
