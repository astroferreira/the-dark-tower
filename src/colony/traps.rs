//! Cage traps at the gates: a beast caught alive.
//!
//! The idea is Dwarf Fortress's cage traps: a cage on a mechanism at the entrance catches what
//! walks in, and a caught beast is the fortress's trophy. Here, with the palisade standing and a
//! beast foretold or already come, the camp sets cage traps at its four gates
//! and the mine's mouth (`ProjectKind::Traps`, eight loads; marks `MarkKind::Cage`). On the night
//! of a beast's raid (not a giant: size 2.6 or less) a trap may take it before anyone is hurt: two
//! times in three when it comes in near one, one in three otherwise. The beast sits caged at the gate from
//! then on (a moment; `Colony::caged`, a treasure of the camp, and it troubles them no more).

use super::*;

impl Colony {
    /// The gates of the palisade ring (`projects.rs`): the four points where the ring is open.
    pub(crate) fn gates(&self) -> Vec<Pos> {
        let r = self.wall_r();
        [(r, 0), (-r, 0), (0, r), (0, -r)].iter().filter_map(|&(dx, dy)| {
            let (x, y) = (self.camp.0 as i32 + dx, self.camp.1 as i32 + dy);
            (x >= 1 && y >= 1 && (x as usize) < self.map.width && (y as usize) < self.map.height).then_some((x as u16, y as u16))
        }).collect()
    }

    /// Where the traps are: the gates and the mine's mouth.
    fn trap_places(&self) -> Vec<Pos> { self.gates().into_iter().chain(self.mine_mouth()).collect() }

    /// The traps stand: a cage mark at each gate.
    pub(crate) fn set_traps(&mut self) {
        let day = self.clock.day();
        for g in self.gates() {
            self.marks.push(ColonyMark { at: g, kind: MarkKind::Cage, title: "A cage trap".into(), text: format!("A cage on a trip-stone at the gate, set on day {}.", day), day });
        }
    }

    /// The raid's night: perhaps the trap takes the beast. Returns whether it did.
    pub(crate) fn trap_takes(&mut self, threat: &arc::Threat, clash: Option<Pos>, seed: u64) -> bool {
        if !self.projects.iter().any(|p| p.done && p.kind == projects::ProjectKind::Traps) { return false; }
        if !matches!(threat.kind, arc::ThreatKind::Beast | arc::ThreatKind::Deep) { return false; }
        let size = threat.monster.as_ref().map_or(1.0, |m| m.size);
        if size > 2.6 { return false; }
        let gates = self.trap_places();
        // Out of the deep, it passes the trap at the mine's mouth first.
        let from_mine = threat.kind == arc::ThreatKind::Deep && self.mine_mouth().is_some();
        let near = from_mine || clash.map_or(false, |c| gates.iter().any(|g| (g.0 as i32 - c.0 as i32).abs().max((g.1 as i32 - c.1 as i32).abs()) <= 4));
        let roll = crate::history::settlers::hash_pub(seed, 0xCA6E) % 3;
        if !(if near { roll < 2 } else { roll == 0 }) { return false; }
        let nearest = clash.and_then(|c| gates.iter().copied().min_by_key(|g| (g.0 as i32 - c.0 as i32).abs().max((g.1 as i32 - c.1 as i32).abs())));
        let gate = (if from_mine { self.mine_mouth() } else { None }).or(nearest).or(gates.first().copied()).unwrap_or(self.camp);
        let day = self.clock.day();
        let name = threat.name.clone();
        let place = if self.mine_mouth() == Some(gate) { "at the mouth of the mine" } else { "at the gate" };
        let line = format!("{} blunders onto the trip-stone {}: the cage drops and the bars hold. It rages there till dawn, and does not get out.", super::arc::capital_word(&name), place);
        // (The mine's trap was set when the mine was dug: its mark is made now.)
        if !self.marks.iter().any(|m| m.kind == MarkKind::Cage && m.at == gate) {
            self.marks.push(ColonyMark { at: gate, kind: MarkKind::Cage, title: "A cage trap".into(), text: String::new(), day });
        }
        self.note(line.clone());
        self.moment(format!("{} is caged", super::arc::capital_word(&name)), line, "because the camp had set cage traps at its gates".into(), gate);
        if let Some(m) = self.marks.iter_mut().find(|m| m.kind == MarkKind::Cage && m.at == gate) {
            m.title = format!("The cage of {}", name);
            m.text = format!("Here {} sits caged, taken {} on the night of day {}.", name, place, day);
        }
        self.caged.push(name.clone());
        self.treasures.push(format!("{}, caged {} on day {}", name, place, day));
        if let Some(a) = self.arc.as_mut() { a.later.retain(|t| t.name != name); a.reserve.retain(|t| t.name != name); }
        if self.were.as_deref() == Some(name.as_str()) { self.were = None; }
        for j in 0..self.settlers.len() { if self.settlers[j].alive { self.feel(j, mind::Feel::Admired { what: format!("{} in its cage", name) }); } }
        true
    }
}
