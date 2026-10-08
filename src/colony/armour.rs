//! Armour: what the militia wears, and the blows it turns.
//!
//! The idea is Dwarf Fortress's armour: layers of a material over the body, each blow tested
//! against what covers the part it lands on, so the same strike is a death in a shirt and a bruise
//! in mail. Here the best hand at the workshop makes armour for each spear borne
//! (`Job::Craft` "Making armour", by day; 1.0 while trouble is foretold, 0.6 in quiet times;
//! spears come first while trouble is foretold, armour first in quiet times), of the best to
//! hand: adamantine mail (turns 0.75 of blows) once the deep shaft struck it, iron mail (0.55)
//! where iron is worked from ore or bought, copper scale (0.45) from copper ore, or a leather
//! jerkin (0.25) from a hide of the hunt or the pen (`hunted` less
//! `hides_used`). At dawn the best armour goes to the best fighters with the spears. In the
//! clash (`fight.rs`) armour may turn a blow: a wound is one step lighter (a bruise not at all), a
//! defender's bruise is turned, and on the night a raid would kill, the victim's armour may turn
//! the killing blow into a grave wound (`armour_turns`, half its share against a beast's size; a
//! moment the first time). An expedition's armoured hands are hurt or fall at the lair less
//! often by what it turns (x(1 - cover)).

use super::*;

/// What a piece of armour is made from.
#[derive(Clone, Copy, Debug)]
enum Source { Ore, Hide, Beast(usize) }

#[derive(Clone, Debug)]
pub struct Armour {
    /// "a leather jerkin", "an iron mail shirt".
    pub kind: String,
    /// The share of blows it turns.
    pub cover: f32,
    pub maker: usize,
    pub day: u64,
    pub holder: Option<usize>,
}

impl Colony {
    /// What armour the workshop could make now: (kind, cover, uses a hide). Metal comes from the
    /// ore the camp works.
    fn armour_to_hand(&self) -> Option<(String, f32, Source)> {
        let metal = |m: &str| self.ores.iter().any(|o| o == m);
        if metal("adamantine") { return Some(("an adamantine mail shirt".into(), 0.75, Source::Ore)); }
        if self.iron_worked() && (metal("iron") || self.tools_bought) { return Some(("an iron mail shirt".into(), 0.55, Source::Ore)); }
        // The hide of a beast the camp slew (`fight.rs`): better than any leather.
        if let Some(b) = self.remains.iter().position(|r| r.3 > 0) {
            let r = &self.remains[b];
            return Some((format!("a coat of {}'s {}", r.1, r.4), if r.4.ends_with("plates") { 0.5 } else { 0.45 }, Source::Beast(b)));
        }
        if self.iron_worked() && metal("copper") { return Some(("a copper scale shirt".into(), 0.45, Source::Ore)); }
        if self.hunted > self.hides_used { return Some(("a leather jerkin".into(), 0.25, Source::Hide)); }
        None
    }

    /// The workshop makes armour for those who bear spears and lack it (spears first while
    /// trouble is foretold).
    pub(crate) fn armour_wanted(&self) -> bool {
        !self.arms.is_empty() && !(self.trouble_foretold().is_some() && self.arms_wanted()) && self.armour.len() < self.arms.len() && self.armour_to_hand().is_some()
    }

    /// A piece of armour finished.
    pub(crate) fn finish_armour(&mut self, i: usize) {
        let Some((kind, cover, src)) = self.armour_to_hand() else { return };
        match src { Source::Hide => self.hides_used += 1, Source::Beast(b) => self.remains[b].3 -= 1, Source::Ore => {} }
        // Mail forged at the magma (`deep.rs`): closer rings.
        let (kind, cover) = if self.magma_forge && matches!(src, Source::Ore) { (super::deep::magma_forged(&kind), (cover + 0.05).min(0.85)) } else { (kind, cover) };
        let day = self.clock.day();
        let name = self.settlers[i].name.clone();
        self.settlers[i].made.push(format!("{} (day {})", kind, day));
        self.armour.push(armour::Armour { kind: kind.clone(), cover, maker: i, day, holder: None });
        if self.armour.len() == 1 {
            let why = self.trouble_foretold().map(|t| format!("for fear of {}", t)).unwrap_or_else(|| "against the next trouble".into());
            self.note(format!("{} makes {} at the workshop, {}; the first armour in the camp.", name, kind, why));
        }
        self.armour_up();
    }

    /// The best armour to the best fighters (those who bear spears first).
    pub(crate) fn armour_up(&mut self) {
        let mut order: Vec<usize> = (0..self.armour.len()).collect();
        order.sort_by(|&a, &b| self.armour[b].cover.total_cmp(&self.armour[a].cover).then(a.cmp(&b)));
        let mut wearers: Vec<usize> = self.arms.iter().filter_map(|a| a.holder).filter(|&h| self.settlers[h].alive).collect();
        wearers.sort_by(|&a, &b| self.fight_skill(b).total_cmp(&self.fight_skill(a)).then(a.cmp(&b)));
        for a in self.armour.iter_mut() { a.holder = None; }
        for (k, &ai) in order.iter().enumerate() { self.armour[ai].holder = wearers.get(k).copied(); }
    }

    /// The armour settler `i` wears, if any.
    pub fn armour_of(&self, i: usize) -> Option<&armour::Armour> { self.armour.iter().find(|a| a.holder == Some(i)) }

    /// Does settler `i`'s armour turn a blow? `share` scales its cover (1 for a blow, 0.5 for a
    /// killing blow); a foe bigger than a man gets through more often.
    pub(crate) fn armour_turns(&self, i: usize, share: f32, size: f32, salt: u64) -> Option<String> {
        let a = self.armour_of(i)?;
        let r = (crate::history::settlers::hash_pub(self.seed ^ self.clock.tick ^ i as u64, salt) % 10_000) as f32 / 10_000.0;
        if r < a.cover * share / size.max(1.0) { Some(a.kind.clone()) } else { None }
    }
}
