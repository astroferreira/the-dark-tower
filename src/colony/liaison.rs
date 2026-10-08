//! The caravan's liaison: what the camp asks of its trading town, and what comes of it.
//!
//! The idea is Dwarf Fortress's outpost liaison: with each caravan comes someone who asks what the
//! fortress would have brought next year, and the next caravan brings it. Here, after each
//! caravan's trade (`liaison`, from `caravan_arrives`), the speaker (else the oldest) names the
//! camp's worst want, with its reason: healing herbs while someone lies wounded, salt when food
//! keeps under sixty days and there is no smokehouse or the store is low, seed grain for a field
//! that stands, iron tools when the town has iron and the camp works none. The next caravan brings
//! it if the camp has works to trade (else they take it home again): salt keeps food twice as long
//! for sixty days (`keeps_days`), seed grain makes the field yield half again (`field_season`,
//! three meals a crop), herbs make six tendings heal a third sooner (`tend`), tools make iron
//! worked. Each gift is a line naming the town.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Want { Herbs, Salt, SeedGrain, IronTools, Cloth, Ore, Charcoal }

impl Want {
    pub fn word(self) -> &'static str {
        match self { Want::Herbs => "healing herbs", Want::Salt => "a cask of salt", Want::SeedGrain => "sacks of seed grain", Want::IronTools => "iron tools", Want::Cloth => "bolts of cloth", Want::Ore => "loads of iron ore", Want::Charcoal => "sacks of charcoal" }
    }
}

impl Colony {
    /// The camp's worst want and why.
    fn worst_want(&self, iron: bool) -> Option<(Want, String)> {
        let day = self.clock.day();
        let wounded = (0..self.settlers.len()).filter(|&i| self.settlers[i].alive && self.open_wounds(i).any(|w| w.severity >= 2)).count();
        if wounded > 0 && self.herbs == 0 {
            let who = if wounded == 1 { "one of them lies".to_string() } else { format!("{} of them lie", wounded) };
            return Some((Want::Herbs, format!("{} wounded, and the healer has only moss", who)));
        }
        let alive = self.alive().max(1) as u32;
        let smoke = self.projects.iter().any(|p| p.done && p.kind == projects::ProjectKind::Smokehouse);
        if self.salt_until <= day && self.keeps_days() < 60 && (!smoke || self.food_stored() < 3 * alive) {
            return Some((Want::Salt, format!("the store holds {} days of food and keeps it {} days", self.days_of_food() as u32, self.keeps_days())));
        }
        if !self.seed_grain && self.projects.iter().any(|p| p.done && p.kind == projects::ProjectKind::Field) {
            return Some((Want::SeedGrain, "the field gives two meals a row, and better seed would give more".into()));
        }
        let ragged = (0..self.settlers.len()).filter(|&i| self.settlers[i].alive && self.settlers[i].guest_until == 0 && self.clothes_worn(i) >= 180).count();
        if ragged >= 3 && self.cloth == 0 && self.hunted <= self.hides_used { return Some((Want::Cloth, format!("{} of them go about in rags", ragged))); }
        // The smelter (`industry.rs`): ore for it when none is struck, charcoal when there is
        // ore but no wood to burn.
        if iron && self.shop(super::delve::RoomKind::Smelter).is_some() && self.ore_total() == 0 {
            return Some((Want::Ore, "the smelter stands cold: no ore has been struck in the camp's rock".into()));
        }
        if self.ore_total() > 0 && self.industry.charcoal == 0 && !self.magma_forge && !self.wood_in_reach && !self.items.iter().any(|it| it.kind == ItemKind::Log) {
            return Some((Want::Charcoal, format!("{} loads of ore wait at the smelter and there is no wood to burn", self.ore_total())));
        }
        if iron && !self.iron_worked() { return Some((Want::IronTools, "the camp works with stone and bone".into())); }
        None
    }

    /// After a caravan's trade: what was asked last time comes; the liaison asks again.
    pub(crate) fn liaison(&mut self, p: &trade::Partner, traded: bool) {
        let day = self.clock.day();
        if let Some(w) = self.request.take() {
            if traded {
                let what = match w {
                    Want::Salt => { self.salt_until = day + 60; "the food will keep twice as long".to_string() }
                    Want::SeedGrain => { self.seed_grain = true; "the field will give half again".to_string() }
                    Want::Herbs => { self.herbs = 6; "the healer's work will go quicker".to_string() }
                    Want::IronTools => { self.tools_bought = true; "the camp's works will go quicker".to_string() }
                    Want::Cloth => { self.cloth += 6; "there will be clothes for those in rags".to_string() }
                    Want::Ore => { self.add_ore("iron", 8); "the smelter has work".to_string() }
                    Want::Charcoal => { self.industry.charcoal += 10; "the smelter has fuel".to_string() }
                };
                self.note(format!("As asked, the traders of {} have brought {}: {}.", p.town, w.word(), what));
            } else {
                self.note(format!("The traders of {} brought {} as asked, but the camp had nothing to give for it; they take it home again.", p.town, w.word()));
            }
        }
        let alive: Vec<usize> = (0..self.settlers.len()).filter(|&i| self.settlers[i].alive && self.settlers[i].guest_until == 0).collect();
        let Some(&judge) = self.speaker.filter(|&s| self.settlers[s].alive).as_ref()
            .or_else(|| alive.iter().max_by_key(|&&i| (self.settlers[i].past.as_ref().map_or(30, |x| x.age), std::cmp::Reverse(i)))) else { return };
        let Some((want, why)) = self.worst_want(p.iron) else { return };
        let jn = self.settlers[judge].name.clone();
        self.note(format!("The traders' liaison asks {} what the camp would have of {} next season: {} asks for {}, because {}.", jn, p.town, jn, want.word(), why));
        self.request = Some(want);
    }
}
