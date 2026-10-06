//! Creatures on the map: the raid arrives on legs, and wolves come out of their dens at night.
//!
//! The arc's attackers (a named beast, drawn at its size, or a war band of three) enter at the
//! map's edge on the side their lair, seat or town lies, an hour before midnight on the eve of
//! the raid, and creep to the camp (15 minutes a cell: about 20 real seconds at 1x across 90
//! cells). The clash is played where they meet the first settler: the one nearest is struck,
//! the one nearest them comes to their aid (`Colony::raid_at`). A closed palisade sends them
//! round to a gate, since they path like anyone else. Where the embark has a predator's den,
//! two wolves come out at dusk and hunt a settler alone and far from the fire; they fear it
//! and go home at dawn.

use super::{nav, Colony, Pos, TICKS_PER_DAY};
use crate::local::wildlife::Feature;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreatureKind { Beast, Raider, Wolf }

#[derive(Clone, Debug)]
pub struct Creature {
    pub kind: CreatureKind,
    pub name: String,
    pub pos: Pos,
    pub path: Vec<Pos>,
    pub stride: i32,
    /// Done here: walking back the way it came, gone at the path's end.
    pub leaving: bool,
    pub size: f32,
    /// Where it came from (the edge, or a den).
    pub home: Pos,
    pub spawned: u64,
}

/// Cells from the camp at which the attackers enter (they walk a settler's pace from dusk and
/// reach the camp in the small hours).
const ENTRY: f32 = 84.0;
/// A step costs this many times a settler's (they creep at night).
const CREEP: i32 = 1;

fn compass(dx: f32, dy: f32) -> &'static str {
    let a = dy.atan2(dx).to_degrees();
    match ((a + 360.0 + 22.5) % 360.0 / 45.0) as usize {
        0 => "the east", 1 => "the south-east", 2 => "the south", 3 => "the south-west",
        4 => "the west", 5 => "the north-west", 6 => "the north", _ => "the north-east",
    }
}

impl Colony {
    /// The direction (unit vector on the map) the arc's threat comes from, and its compass word.
    fn threat_side(&self) -> ((f32, f32), &'static str) {
        let from = self.arc.as_ref().and_then(|a| a.threat.from);
        let (tx, ty) = self.map.world_tile;
        let mut d = match from {
            Some((fx, fy)) => (fx as f32 - tx as f32, fy as f32 - ty as f32),
            None => (0.0, 0.0),
        };
        if d.0.abs() + d.1.abs() < 0.01 {
            let a = (self.seed % 360) as f32 * std::f32::consts::PI / 180.0;
            d = (a.cos(), a.sin());
        }
        let n = (d.0 * d.0 + d.1 * d.1).sqrt();
        let d = (d.0 / n, d.1 / n);
        (d, compass(d.0, d.1))
    }

    /// A passable cell near `p` (searching outward).
    fn passable_near(&self, p: (i32, i32)) -> Option<Pos> {
        let n = self.map.width as i32;
        for r in 0i32..20 {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs().max(dy.abs()) != r { continue; }
                    let q = ((p.0 + dx).clamp(2, n - 3) as u16, (p.1 + dy).clamp(2, self.map.height as i32 - 3) as u16);
                    if nav::passable(&self.map, q) { return Some(q); }
                }
            }
        }
        None
    }

    /// The attackers set out from the map's edge on the threat's side.
    pub(crate) fn send_attackers(&mut self) {
        let Some(arc) = self.arc.as_ref() else { return };
        let (kind, n, name, size) = match arc.threat.kind {
            super::arc::ThreatKind::Beast => (CreatureKind::Beast, 1, arc.threat.name.clone(), arc.threat.size),
            _ => (CreatureKind::Raider, 3, arc.threat.name.clone(), 1.0),
        };
        let ((dx, dy), side) = self.threat_side();
        let c = (self.camp.0 as f32, self.camp.1 as f32);
        for k in 0..n {
            let off = (k as f32 - (n as f32 - 1.0) / 2.0) * 4.0;
            let p = ((c.0 + dx * ENTRY - dy * off) as i32, (c.1 + dy * ENTRY + dx * off) as i32);
            let Some(at) = self.passable_near(p) else { continue };
            let path = nav::path(&self.map, at, self.camp, super::PATH_BUDGET).map(|p| p.into_iter().skip(1).collect()).unwrap_or_default();
            self.creatures.push(Creature { kind, name: name.clone(), pos: at, path, stride: 0, leaving: false, size, home: at, spawned: self.clock.tick });
        }
        if let Some(first) = self.creatures.iter().find(|c| c.kind != CreatureKind::Wolf).map(|c| c.pos) {
            self.raid_side = side.to_string();
            let line = format!("Something moves at the edge of the clearing, out of {}.", side);
            self.note(line.clone());
            let who = self.arc.as_ref().map(|a| a.threat.name.clone()).unwrap_or_default();
            self.moment("They are coming".into(), line, format!("because {} {} on the move, as was foretold", who, if kind == CreatureKind::Beast { "is" } else { "are" }), first);
        }
    }

    pub(crate) fn attackers_out(&self) -> bool { self.creatures.iter().any(|c| c.kind != CreatureKind::Wolf) }

    /// Where the attackers meet the camp: the first of them within two cells of a living
    /// settler, or at the end of their path (the fire).
    pub(crate) fn attackers_clash(&self) -> Option<Pos> {
        for c in self.creatures.iter().filter(|c| c.kind != CreatureKind::Wolf && !c.leaving) {
            if self.settlers.iter().any(|s| s.alive && (s.pos.0 as i32 - c.pos.0 as i32).abs().max((s.pos.1 as i32 - c.pos.1 as i32).abs()) <= 2) { return Some(c.pos); }
            if c.path.is_empty() { return Some(c.pos); }
        }
        None
    }

    /// The attackers break off and go back the way they came.
    pub(crate) fn attackers_retreat(&mut self) {
        for k in 0..self.creatures.len() {
            if self.creatures[k].kind == CreatureKind::Wolf { continue; }
            let (from, home) = (self.creatures[k].pos, self.creatures[k].home);
            let path = nav::path(&self.map, from, home, super::PATH_BUDGET).map(|p| p.into_iter().skip(1).collect()).unwrap_or_default();
            let c = &mut self.creatures[k];
            c.leaving = true;
            c.path = path;
        }
    }

    /// Move every creature a tick; those leaving vanish at their path's end. Wolves come out at
    /// dusk where there is a den and go home at dawn.
    pub(crate) fn step_creatures(&mut self) {
        let (hour, minute) = (self.clock.hour(), self.clock.minute());
        if hour == 21 && minute == 0 { self.wolves_out(); }
        if hour == 6 && minute == 0 {
            for k in 0..self.creatures.len() {
                if self.creatures[k].kind == CreatureKind::Wolf && !self.creatures[k].leaving { self.wolf_home(k); }
            }
        }
        // Wolves hunt a settler alone and far from the fire.
        if self.clock.tick % 20 == 0 { self.wolves_hunt(); }
        for k in 0..self.creatures.len() {
            let speed = if self.creatures[k].kind == CreatureKind::Wolf { 2 } else { 1 };
            self.creatures[k].stride += speed;
            loop {
                let Some(&next) = self.creatures[k].path.first() else { self.creatures[k].stride = 0; break };
                let cost = nav::cost(&self.map, next.0 as usize, next.1 as usize).map_or(30, |c| c as i32) * if self.creatures[k].kind == CreatureKind::Wolf { 1 } else { CREEP } / 2;
                if self.creatures[k].stride < cost { break; }
                self.creatures[k].stride -= cost;
                self.creatures[k].pos = next;
                self.creatures[k].path.remove(0);
            }
        }
        self.creatures.retain(|c| !(c.leaving && c.path.is_empty()));
    }

    fn dens(&self) -> Vec<Pos> {
        let n = self.map.width;
        (0..self.map.features.len()).filter(|&i| self.map.features[i] == Feature::Den).map(|i| ((i % n) as u16, (i / n) as u16)).collect()
    }

    fn wolves_out(&mut self) {
        if self.creatures.iter().any(|c| c.kind == CreatureKind::Wolf) { return; }
        let dens = self.dens();
        if dens.is_empty() { return; }
        let den = dens[(self.clock.day() as usize * 7 + self.seed as usize) % dens.len()];
        for k in 0..2u16 {
            let Some(at) = self.passable_near((den.0 as i32 + k as i32, den.1 as i32)) else { continue };
            self.creatures.push(Creature { kind: CreatureKind::Wolf, name: "a wolf".into(), pos: at, path: Vec::new(), stride: 0, leaving: false, size: 1.0, home: at, spawned: self.clock.tick });
        }
    }

    fn wolf_home(&mut self, k: usize) {
        let (from, home) = (self.creatures[k].pos, self.creatures[k].home);
        self.creatures[k].path = nav::path(&self.map, from, home, super::PATH_BUDGET).map(|p| p.into_iter().skip(1).collect()).unwrap_or_default();
        self.creatures[k].leaving = true;
    }

    fn wolves_hunt(&mut self) {
        let camp = self.camp;
        let far_from_fire = |p: Pos| (p.0 as i32 - camp.0 as i32).abs().max((p.1 as i32 - camp.1 as i32).abs()) > 7;
        for k in 0..self.creatures.len() {
            if self.creatures[k].kind != CreatureKind::Wolf || self.creatures[k].leaving { continue; }
            let wp = self.creatures[k].pos;
            // The prey: a living settler outdoors, far from the fire, with no one else near.
            let prey = (0..self.settlers.len()).filter(|&i| {
                let s = &self.settlers[i];
                s.alive && far_from_fire(s.pos) && self.roof_over(s.pos).is_none()
                    && (s.pos.0 as i32 - wp.0 as i32).abs().max((s.pos.1 as i32 - wp.1 as i32).abs()) <= 40
                    && !self.settlers.iter().enumerate().any(|(j, o)| j != i && o.alive && (o.pos.0 as i32 - s.pos.0 as i32).abs().max((o.pos.1 as i32 - s.pos.1 as i32).abs()) <= 4)
            }).min_by_key(|&i| (self.settlers[i].pos.0 as i32 - wp.0 as i32).abs().max((self.settlers[i].pos.1 as i32 - wp.1 as i32).abs()));
            let Some(i) = prey else { continue };
            let sp = self.settlers[i].pos;
            if (sp.0 as i32 - wp.0 as i32).abs().max((sp.1 as i32 - wp.1 as i32).abs()) <= 1 {
                // A bite, and the pack goes home.
                let name = self.settlers[i].name.clone();
                self.settlers[i].ill_until = self.clock.tick + TICKS_PER_DAY;
                self.note(format!("{} is bitten by wolves at the edge of the woods, alone and far from the fire.", name));
                for j in 0..self.creatures.len() { if self.creatures[j].kind == CreatureKind::Wolf { self.wolf_home(j); } }
                return;
            }
            self.creatures[k].path = nav::path(&self.map, wp, sp, 4000).map(|p| p.into_iter().skip(1).collect()).unwrap_or_default();
        }
    }
}
