//! Armour: what the militia wears, and the blows it turns.
//!
//! The idea is Dwarf Fortress's armour: layers of a material over the body, each blow tested
//! against what covers the part it lands on, so the same strike is a death in a shirt and a bruise
//! in mail. Here the best hand at the workshop makes armour for each spear borne
//! (`Job::Craft` "Making armour", by day; 1.0 while trouble is foretold, 0.6 in quiet times;
//! spears come first while trouble is foretold, armour first in quiet times), of the best to
//! hand: adamantine mail once the deep shaft struck it, iron mail where iron is worked from ore or
//! bought, a coat of a slain beast's hide or plates (when its stuff is armour-worthy: DF's
//! capability flags), copper scale from copper ore, or a leather jerkin from a hide of the hunt or
//! the pen (`hunted` less `hides_used`). What it does is the material's (`materials.rs`): a shirt
//! is a layer over the body and arms, its subtype's mm of its material, and a blow on a covered
//! part must cut it (an edge bites only what is well softer than itself: a raider's iron sword
//! cannot cut iron mail and lands blunt, spread by the rings) or crack it. At dawn the best armour
//! goes to the best fighters with the spears. In the clash (`fight.rs`) a wound is as deep as the
//! blow gets through it ("does not get through", "though the leather jerkin took the worst of
//! it"), a defender's bruise may be turned, and on the night a raid would kill, a killing blow on
//! the chest that the armour holds to a bruise is turned into a grave wound
//! (`armour_turns_killing`; a moment the first time). An expedition's armoured hands are hurt or
//! fall at the lair less often by the share of the beast's blows it lightens (`armour_guard`).

use super::*;

/// What a piece of armour is made from.
#[derive(Clone, Copy, Debug)]
enum Source { Ore, Hide, Beast(usize) }

#[derive(Clone, Debug)]
pub struct Armour {
    /// "a leather jerkin", "an iron mail shirt".
    pub kind: String,
    /// What it is of ("leather", "iron", "fur", "granite") and its shape ("jerkin", "mail
    /// shirt", "coat", "plate coat"; `materials.json`).
    pub material: String,
    pub piece: String,
    /// The maker's hand and the forge: x its thickness.
    pub quality: f32,
    pub maker: usize,
    pub day: u64,
    pub holder: Option<usize>,
}

impl Armour {
    /// How well it guards, for handing out: what it takes off a raider's iron sword and a big
    /// beast's jaws on the body.
    pub fn rating(&self) -> f32 {
        let worn = Some((self.material.as_str(), self.piece.as_str(), self.quality));
        [crate::materials::weapon_blow("sword", "iron", 1.0), crate::materials::natural_blow("jaws", None, 2.0)].iter().map(|b| {
            let with = crate::materials::strike(b, &crate::materials::body("settler", "body", 1.0, None, worn));
            let without = crate::materials::strike(b, &crate::materials::body("settler", "body", 1.0, None, None));
            (without.outcome.severity() as f32 - with.outcome.severity() as f32) + (without.harm - with.harm)
        }).sum()
    }
}

impl Colony {
    /// What armour the workshop could make now: (kind, material, piece, what it uses). Metal
    /// comes in bars at a forge (`industry.rs`); a beast's remains only when its stuff is
    /// armour-worthy.
    fn armour_to_hand(&self) -> Option<(String, String, String, Source)> {
        let metal = |m: &str| self.metal_forge() && self.bars_of(m) >= super::industry::MAIL_BARS;
        let made = |kind: &str, mat: &str, piece: &str, src: Source| Some((kind.to_string(), mat.to_string(), piece.to_string(), src));
        if metal("adamantine") { return made("an adamantine mail shirt", "adamantine", "mail shirt", Source::Ore); }
        if metal("iron") { return made("an iron mail shirt", "iron", "mail shirt", Source::Ore); }
        // The hide or plates of a beast the camp slew (`fight.rs`): better than any leather.
        let stuff = |skin: &str| skin.trim_end_matches(" plates").to_string();
        if let Some(b) = self.remains.iter().position(|r| r.3 > 0 && crate::materials::known(&stuff(&r.4)).map_or(false, |m| m.can("armour"))) {
            let r = &self.remains[b];
            return made(&format!("a coat of {}'s {}", r.1, r.4), &stuff(&r.4), if r.4.ends_with("plates") { "plate coat" } else { "coat" }, Source::Beast(b));
        }
        if metal("copper") { return made("a copper scale shirt", "copper", "scale shirt", Source::Ore); }
        if self.hunted > self.hides_used { return made("a leather jerkin", "leather", "jerkin", Source::Hide); }
        None
    }

    /// The workshop makes armour for those who bear spears and lack it (spears first while
    /// trouble is foretold).
    pub(crate) fn armour_wanted(&self) -> bool {
        !self.arms.is_empty() && !(self.trouble_foretold().is_some() && self.arms_wanted()) && self.armour.len() < self.arms.len() && self.armour_to_hand().is_some()
    }

    /// A piece of armour finished.
    pub(crate) fn finish_armour(&mut self, i: usize) {
        let Some((kind, material, piece, src)) = self.armour_to_hand() else { return };
        match src { Source::Hide => self.hides_used += 1, Source::Beast(b) => self.remains[b].3 -= 1, Source::Ore => { if let Some(m) = Self::metal_in(&kind) { self.take_bars(m, super::industry::MAIL_BARS); } } }
        // A sure hand works it closer (a smith's, for metal).
        let quality = if matches!(src, Source::Ore) { self.smith_hand(i) } else { 0.9 + 0.2 * self.settlers[i].skill[4] };
        // Mail forged at the magma (`deep.rs`): closer rings.
        let (kind, quality) = if self.magma_forge && matches!(src, Source::Ore) { (super::deep::magma_forged(&kind), quality * 1.15) } else { (kind, quality) };
        let day = self.clock.day();
        let name = self.settlers[i].name.clone();
        self.settlers[i].made.push(format!("{} (day {})", kind, day));
        self.armour.push(armour::Armour { kind: kind.clone(), material, piece, quality, maker: i, day, holder: None });
        if matches!(src, Source::Ore) { self.forged_arm(i, &kind); }
        if self.armour.len() == 1 {
            let why = self.trouble_foretold().map(|t| format!("for fear of {}", t)).unwrap_or_else(|| "against the next trouble".into());
            self.note(format!("{} makes {} at {}, {}; the first armour in the camp.", name, kind, if matches!(src, Source::Ore) { "the forge" } else { "the workshop" }, why));
        }
        self.armour_up();
    }

    /// The best armour to the best fighters (those who bear spears first).
    pub(crate) fn armour_up(&mut self) {
        let mut order: Vec<usize> = (0..self.armour.len()).collect();
        let rating: Vec<f32> = self.armour.iter().map(|a| a.rating()).collect();
        order.sort_by(|&a, &b| rating[b].total_cmp(&rating[a]).then(a.cmp(&b)));
        let mut wearers: Vec<usize> = self.arms.iter().filter_map(|a| a.holder).filter(|&h| self.settlers[h].alive).collect();
        wearers.sort_by(|&a, &b| self.fight_skill(b).total_cmp(&self.fight_skill(a)).then(a.cmp(&b)));
        for a in self.armour.iter_mut() { a.holder = None; }
        for (k, &ai) in order.iter().enumerate() { self.armour[ai].holder = wearers.get(k).copied(); }
    }

    /// The armour settler `i` wears, if any.
    pub fn armour_of(&self, i: usize) -> Option<&armour::Armour> { self.armour.iter().find(|a| a.holder == Some(i)) }

    /// Does settler `i`'s armour turn the killing blow? It falls on the head or the chest (by the
    /// hash), half again as hard as a wounding blow; turned when the armour covers that part and
    /// holds it to a bruise.
    pub(crate) fn armour_turns_killing(&self, i: usize, foe: &fight::Foe, salt: u64) -> Option<String> {
        let a = self.armour_of(i)?;
        let h = crate::history::settlers::hash_pub(self.seed ^ self.clock.tick ^ i as u64, salt);
        let part = if h % 2 == 0 { "head" } else { "body" };
        let luck = 0.85 + 0.3 * ((h >> 8) % 1000) as f32 / 1000.0;
        let struck = crate::materials::strike(&foe.blow(1.5 * luck), &self.body_of(i, part, true));
        if struck.outcome.severity() <= 1 { Some(a.kind.clone()) } else { None }
    }

    /// The share of a foe's blows settler `i`'s armour lightens (each part by its odds; a step of
    /// severity counted against the steps the wound would have had).
    pub(crate) fn armour_guard(&self, i: usize, foe: &fight::Foe) -> f32 {
        if self.armour_of(i).is_none() { return 0.0; }
        let blow = foe.blow(1.0);
        let (mut guard, mut total) = (0.0f32, 0.0f32);
        for (part, w) in fight::PARTS {
            let with = crate::materials::strike(&blow, &self.body_of(i, part, true)).outcome.severity() as f32;
            let without = crate::materials::strike(&blow, &self.body_of(i, part, false)).outcome.severity() as f32;
            if without > 0.0 { guard += w as f32 * (without - with).max(0.0) / without; }
            total += w as f32;
        }
        guard / total.max(1.0)
    }
}
