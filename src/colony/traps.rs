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
    /// The gates of the palisade ring (`projects.rs`): the points where the ring is open, in the
    /// directions the camp chose for them (`choose_gates`).
    pub(crate) fn gates(&self) -> Vec<Pos> {
        let r = self.wall_r();
        self.gate_dirs().into_iter().filter_map(|d| {
            let (dx, dy) = gate_point(d, r);
            let (x, y) = (self.camp.0 as i32 + dx, self.camp.1 as i32 + dy);
            (x >= 1 && y >= 1 && (x as usize) < self.map.width && (y as usize) < self.map.height).then_some((x as u16, y as u16))
        }).collect()
    }

    /// The compass directions of the gates (the four axes until the camp chooses).
    pub(crate) fn gate_dirs(&self) -> Vec<(i32, i32)> {
        if self.gate_dirs.is_empty() { vec![(1, 0), (-1, 0), (0, 1), (0, -1)] } else { self.gate_dirs.clone() }
    }

    /// Where the gates go, chosen when the palisade is begun: toward where the camp's paths
    /// lead (the water, the woods, the road to the trading town), each on one of the eight
    /// compass points and a quarter turn apart at least; two for an anxious people, four for a
    /// bold one, three otherwise. Said aloud.
    pub(crate) fn choose_gates(&mut self) {
        const DIRS: [(i32, i32); 8] = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)];
        let snap = |dx: f32, dy: f32| -> usize { ((dy.atan2(dx) / std::f32::consts::FRAC_PI_4).round() as i32).rem_euclid(8) as usize };
        let mut want: Vec<(usize, String)> = Vec::new();
        let lane = self.lane();
        want.push((snap(lane.0, lane.1), "the water".into()));
        let r = self.wall_r();
        // The woods: the eighth of the land beyond the wall (out to 25 cells past it) that holds
        // the most standing trees, when it holds a good many.
        let mut trees = [0u32; 8];
        let n = self.map.width as i32;
        for dy in -(r + 25)..=(r + 25) { for dx in -(r + 25)..=(r + 25) {
            if (dx * dx + dy * dy) as f32 <= ((r + 3) * (r + 3)) as f32 || (dx + dy) % 2 != 0 { continue; }
            let (x, y) = (self.camp.0 as i32 + dx, self.camp.1 as i32 + dy);
            if x < 0 || y < 0 || x >= n || y >= self.map.height as i32 { continue; }
            if matches!(self.floor_plant_pub((x as u16, y as u16)), crate::local::Plant::Tree(_)) { trees[snap(dx as f32, dy as f32)] += 1; }
        } }
        let (k, &most) = trees.iter().enumerate().max_by_key(|(k, c)| (**c, std::cmp::Reverse(*k))).unwrap();
        let mean = trees.iter().sum::<u32>() / 8;
        if most >= 30 && most >= mean * 3 / 2 { want.push((k, "the woods".into())); }
        if let Some(p) = self.trade.as_ref() {
            let (tx, ty) = self.map.world_tile;
            let d = (p.from.0 as f32 - tx as f32, p.from.1 as f32 - ty as f32);
            if d.0.abs() + d.1.abs() > 0.5 { want.push((snap(d.0, d.1), format!("the road to {}", p.town))); }
        }
        let fear: f32 = { let f: Vec<f32> = self.settlers.iter().take(7).map(|s| s.persona.facet(crate::persona::Facet::Anxiety) as f32).collect(); f.iter().sum::<f32>() / f.len().max(1) as f32 };
        let most = if fear >= 58.0 { 2 } else if fear < 45.0 { 4 } else { 3 };
        let mut chosen: Vec<(usize, String)> = Vec::new();
        for (k, why) in want {
            if chosen.iter().any(|c| { let d = (c.0 as i32 - k as i32).rem_euclid(8); d <= 1 || d >= 7 }) { continue; }
            chosen.push((k, why));
        }
        // At least two, across from each other; then a quarter turn from the first, as needed.
        while chosen.len() < 2 || (chosen.len() < most && chosen.len() < 4) {
            let first = chosen.first().map_or(0, |c| c.0);
            let next = [(first + 4) % 8, (first + 2) % 8, (first + 6) % 8].into_iter()
                .find(|&k| !chosen.iter().any(|c| { let d = (c.0 as i32 - k as i32).rem_euclid(8); d <= 1 || d >= 7 }));
            match next { Some(k) => chosen.push((k, String::new())), None => break }
        }
        chosen.truncate(most.max(2));
        self.gate_dirs = chosen.iter().map(|c| DIRS[c.0]).collect();
        let named: Vec<String> = chosen.iter().filter(|c| !c.1.is_empty()).map(|c| format!("{} toward {}", COMPASS[c.0], c.1)).collect();
        let n = chosen.len();
        self.note(format!("They will leave {} gate{} in the wall{}{}.", n, if n == 1 { "" } else { "s" },
            if named.is_empty() { String::new() } else { format!(": {}", super::join_names(&named)) },
            if fear >= 58.0 { ", and no more: they are an uneasy people" } else { "" }));
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

const COMPASS: [&str; 8] = ["east", "south-east", "south", "south-west", "west", "north-west", "north", "north-east"];

/// A gate's place on the ring of radius `r`, from its compass direction.
pub(crate) fn gate_point(d: (i32, i32), r: i32) -> (i32, i32) {
    let l = ((d.0 * d.0 + d.1 * d.1) as f32).sqrt();
    ((d.0 as f32 / l * r as f32).round() as i32, (d.1 as f32 / l * r as f32).round() as i32)
}
