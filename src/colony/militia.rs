//! The militia: spears made at the workshop and drills by the fire when trouble is foretold.
//!
//! The idea is Dwarf Fortress's squads: a captain, members, weapons assigned to them, training
//! that raises a fighting skill, and the fight reading that skill and that weapon. Here, from the
//! rumour of a raid until it comes (1.1), and at leisure after the first raid (0.5), the
//! workshop's crafters make spears (a log each: stone- or
//! flint-tipped, iron-headed once iron is worked) until every adult who would fight has one; the
//! brave and the martial drill from 17:00 to 19:00 at the drill ground under the camp's best
//! fighter (a veteran first), each evening adding to their `Settler::drill` by how quickly they
//! learn. At dawn the spears go to the best fighters. In the clash (`fight.rs`) the spear
//! replaces the work tool: a spear-shaped blow of its head's material (`materials.rs`: the head's
//! density weighs the spear, its hardness and edge decide what it cuts), and the fighting skill
//! (a veteran's 0.4 plus drill) adds to the chance to hit and to the blow's momentum; each armed
//! and drilled hand adds 0.015 to readiness (0.06 at most), shown in the eve's tally as "N under
//! arms".

use super::*;
use crate::persona::{Facet, Val};

#[derive(Clone, Debug)]
pub struct Arm {
    /// "a flint-tipped spear", "an iron-headed spear".
    pub kind: String,
    /// What its head is of ("flint", "iron", "adamantine"; `materials.json`).
    pub material: String,
    /// The maker's hand and the forge: x the blow's momentum.
    pub quality: f32,
    pub maker: usize,
    pub day: u64,
    pub holder: Option<usize>,
}

impl Arm {
    /// How good a spear it is, for handing out: its harm to a big furred beast's flank.
    pub fn rating(&self) -> f32 {
        let b = crate::materials::weapon_blow("spear", &self.material, self.quality);
        crate::materials::strike(&b, &crate::materials::body("beast", "flank", 1.5, Some(("fur", false)), None)).harm
    }
}

impl Colony {
    /// The trouble foretold: the thread whose rumour has come and whose raid has not.
    pub fn trouble_foretold(&self) -> Option<String> {
        let a = self.arc.as_ref()?;
        if matches!(a.stage, 1 | 2 | 5) { Some(a.threat.name.clone()) } else { None }
    }

    /// A settler's hand in a fight: a veteran's 0.4 (others 0.05) and their drill.
    pub fn fight_skill(&self, i: usize) -> f32 {
        let base = if self.is_veteran(i) { 0.4 } else { 0.05 };
        (base + self.settlers[i].drill).min(1.0)
    }

    /// Who would fight: adults not too timid, at most eight.
    fn militia(&self) -> Vec<usize> {
        let mut v: Vec<usize> = (0..self.settlers.len()).filter(|&i| {
            let s = &self.settlers[i];
            s.alive && !s.mind.left && s.past.as_ref().map_or(true, |p| p.age >= 14) && (s.persona.facet(Facet::Bravery) >= 30 || s.persona.value(Val::MartialProwess) > 0)
        }).collect();
        v.sort_by(|&a, &b| self.fight_skill(b).total_cmp(&self.fight_skill(a)).then(a.cmp(&b)));
        v.truncate(8);
        v
    }

    /// The workshop makes spears while hands lack them: urgently while trouble is foretold, and
    /// at leisure once the first raid has come (DF's forges work in peacetime too).
    pub(crate) fn arms_wanted(&self) -> bool {
        let raided = self.arc.as_ref().map_or(false, |a| a.chapter > 0 || a.stage == 3);
        (self.trouble_foretold().is_some() || raided) && self.arms.len() < self.militia().len()
    }

    /// A spear from a stored log.
    pub(crate) fn finish_arm(&mut self, i: usize) {
        let Some(k) = self.items.iter().position(|it| it.stored && it.kind == ItemKind::Log)
            .or_else(|| self.items.iter().position(|it| it.stored && it.kind == ItemKind::Stone)) else { return };
        self.items.remove(k);
        self.fix_refs_pub(k);
        let stone = self.land_stone();
        // A sure hand makes a better head.
        let hand = 0.9 + 0.2 * self.settlers[i].skill[4];
        // The best head to hand: adamantine, iron, copper (a bar of it at a forge, `industry.rs`,
        // and the smith's hand), obsidian from the rock, bone from the hunt where the land gives
        // no named stone, the land's stone; whatever it is must be weapon-worthy (DF's capability
        // flags), and what it does in a blow is its material's (`materials.rs`).
        let metal = self.arm_metal(1);
        let smith = self.smith_hand(i);
        let gem = |g: &str| self.gems.iter().any(|x| x.0 == g && x.1 > 0);
        let worthy = |m: &str| crate::materials::known(m).map_or(false, |x| x.can("weapon"));
        let (kind, material, hand) = if metal == Some("adamantine") { ("an adamantine-headed spear".to_string(), "adamantine", smith) }
            else if metal == Some("iron") { ("an iron-headed spear".to_string(), "iron", smith) }
            else if metal == Some("copper") { ("a copper-headed spear".to_string(), "copper", smith) }
            else if gem("obsidian") { ("an obsidian-tipped spear".to_string(), "obsidian", hand) }
            else if (stone == "stone" || !worthy(&stone)) && self.hunted > 0 { ("a bone-tipped spear".to_string(), "bone", hand) }
            else { (format!("a {}-tipped spear", stone), stone.as_str(), hand) };
        let material = material.to_string();
        if material == "obsidian" { if let Some(g) = self.gems.iter_mut().find(|x| x.0 == "obsidian") { g.1 -= 1; } }
        let forged = Self::metal_in(&kind).filter(|m| self.take_bars(m, 1)).is_some();
        let kind = if forged { self.forged_kind(i, &kind) } else { kind };
        // Metal forged at the magma (`deep.rs`): a truer temper.
        let at_magma = self.magma_forge && matches!(material.as_str(), "iron" | "copper" | "adamantine");
        let (kind, quality) = if at_magma { (super::deep::magma_forged(&kind), hand * 1.12) } else { (kind, hand) };
        let day = self.clock.day();
        let name = self.settlers[i].name.clone();
        self.settlers[i].made.push(format!("{} (day {})", kind, day));
        self.arms.push(militia::Arm { kind: kind.clone(), material, quality, maker: i, day, holder: None });
        let why = self.trouble_foretold().map(|t| format!("for fear of {}", t)).unwrap_or_else(|| "against the next trouble".into());
        if self.arms.len() == 1 { self.note(format!("{} makes {} at {}, {}; the first of the camp's arms.", name, kind, if forged { "the forge" } else { "the workshop" }, why)); }
        if forged { self.forged_arm(i, &kind); }
        self.arm_militia();
    }

    /// The spears to the best fighters.
    pub(crate) fn arm_militia(&mut self) {
        let m = self.militia();
        let mut order: Vec<usize> = (0..self.arms.len()).collect();
        let rating: Vec<f32> = self.arms.iter().map(|a| a.rating()).collect();
        order.sort_by(|&a, &b| rating[b].total_cmp(&rating[a]).then(a.cmp(&b)));
        for a in self.arms.iter_mut() { a.holder = None; }
        for (k, &ai) in order.iter().enumerate() { self.arms[ai].holder = m.get(k).copied(); }
        self.armour_up();
    }

    /// The spear settler `i` holds, if any.
    pub fn arm_of(&self, i: usize) -> Option<&militia::Arm> { self.arms.iter().find(|a| a.holder == Some(i)) }

    /// Where they drill: a flat spot west of the fire.
    pub(crate) fn drill_ground(&self) -> Pos { self.spot_from_camp(-7, 2) }

    /// The evening drill, for the militia, while trouble is foretold.
    pub(crate) fn drill_option(&self, i: usize) -> Option<(f32, Job, String)> {
        let threat = self.trouble_foretold()?;
        let h = self.clock.hour();
        if !(17..19).contains(&h) || self.settlers[i].ill_until > self.clock.tick || self.settlers[i].mind.drilled { return None; }
        let m = self.militia();
        if !m.contains(&i) { return None; }
        let captain = m.first().copied()?;
        let p = &self.settlers[i].persona;
        let keen = 0.35 + 0.25 * p.facet(Facet::Bravery) as f32 / 100.0 + 0.3 * (p.value(Val::MartialProwess) as f32 / 50.0).max(0.0);
        let why = if captain == i { format!("Drilling the militia with the spear, for fear of {}", threat) }
            else { format!("Drilling with the spear under {}, for fear of {}", self.settlers[captain].name, threat) };
        Some((keen, Job::Wander(self.drill_ground()), why))
    }

    /// 19:00: those who drilled learn.
    pub(crate) fn drill_done(&mut self) {
        let ground = self.drill_ground();
        let mut drilled = Vec::new();
        for i in 0..self.settlers.len() {
            let s = &self.settlers[i];
            if !s.alive || !s.why.starts_with("Drilling") { continue; }
            if (s.pos.0 as i32 - ground.0 as i32).abs().max((s.pos.1 as i32 - ground.1 as i32).abs()) > 5 { continue; }
            // A vow drives the drill (`vow.rs`).
            let sworn = self.vows.iter().any(|v| v.who == i && v.kept.is_none());
            let gain = 0.05 * s.persona.learning() * if sworn { 2.0 } else { 1.0 };
            let s = &mut self.settlers[i];
            s.drill = (s.drill + gain).min(0.6);
            s.mind.drilled = true;
            drilled.push(s.name.clone());
        }
        if drilled.is_empty() { return; }
        if self.milestones.insert("first drill") {
            let captain = self.militia().first().map(|&c| self.settlers[c].name.clone()).unwrap_or_default();
            let threat = self.trouble_foretold().unwrap_or_default();
            self.note(format!("{} drills {} with the spear by the fire, for fear of {}.", captain, crate::persona::list(&drilled.iter().filter(|n| **n != captain).cloned().collect::<Vec<_>>()), threat));
        }
    }

    /// The militia's share of readiness, and its words for the tally.
    pub(crate) fn militia_ready(&self) -> (f32, usize) {
        let n = self.arms.iter().filter_map(|a| a.holder).filter(|&i| self.settlers[i].alive && self.settlers[i].drill >= 0.1).count();
        ((0.015 * n as f32).min(0.06), n)
    }
}
