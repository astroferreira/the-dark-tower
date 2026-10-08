//! The woods grow back: what is felled returns in a year, where it is let alone.
//!
//! The idea is Dwarf Fortress's tree regrowth (saplings become trees) and the roadmap's ecology
//! coupling (logging depletes what the land gives, and it returns). Here every felled tree is
//! remembered with its kind (`Colony::felled`); at each season's turn (`regrow`), trees felled a
//! year ago (120 days) or more come back where the ground is still open: not built on or beside a
//! building, not worn into a track (fewer than 15 steps), not under a roof. The stump goes. A line
//! says how many came back; the first time is noted as the woods returning.

use super::*;

impl Colony {
    /// Each season's turn: the woods felled a year ago grow back where they are let alone.
    pub(crate) fn regrow(&mut self) {
        let day = self.clock.day();
        if day <= 1 || (day - 1) % SEASON_DAYS != 0 { return; }
        let n = self.map.width;
        let mut grown = 0;
        let felled = std::mem::take(&mut self.felled);
        let mut keep = Vec::new();
        for (p, kind, d) in felled {
            if day < d + ageing::YEAR_DAYS { keep.push((p, kind, d)); continue; }
            let i = p.1 as usize * n + p.0 as usize;
            let open = !self.built_near(p) && self.map.roofs[i] == 0 && self.steps.get(i).copied().unwrap_or(0) < 15;
            if !open { continue; }
            let k = self.map.idx(p.0 as usize, p.1 as usize, self.map.surface_z[i].max(0) as usize);
            if self.map.cells[k].plant != Plant::None { continue; }
            self.map.cells[k].plant = Plant::Tree(kind);
            if self.map.features[i] == crate::local::wildlife::Feature::Stump { self.map.features[i] = crate::local::wildlife::Feature::None; }
            grown += 1;
        }
        self.felled = keep;
        if grown > 0 {
            if self.milestones.insert("woods return") {
                self.note(format!("Where the first trees were felled a year ago, {} young trees stand again among the stumps.", grown));
            } else {
                self.note(format!("{} young trees have grown where trees were felled a year ago.", grown));
            }
        }
    }
}
