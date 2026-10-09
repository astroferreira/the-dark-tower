//! Farms under the rock: food that does not care about winter.
//!
//! The idea is Dwarf Fortress's underground farming: plump helmets and cave wheat grown in muddied
//! rooms below ground, harvested whatever the season above. Here, once a hall is dug into the
//! hill or a cellar into the rock, the camp plants an underground farm (`ProjectKind::CaveFarm`,
//! eight loads of soil carried down: logs or stone stand in for the work): a people who build in
//! stone know the way of it from the start; others only once the mine has broken into a cavern and
//! brought up its spores. Every eight days, whatever the season, the farm yields: twelve meals,
//! twenty with a breached cavern's richer spores, laid by the hall or cellar for the carriers
//! (`cave_harvest`); the first is a moment.

use super::*;

impl Colony {
    /// What the farm grows: the dwarves' plump helmets, or the cavern's pale growths.
    fn cave_crop(&self) -> String {
        if self.way.as_ref().map_or(false, |w| w.stone_first) { "plump helmets".into() }
        else { self.map.caverns.first().map(|c| if c.fungus > 0 { "pale cave mushrooms".to_string() } else { "cave moss".to_string() }).unwrap_or_else(|| "pale cave mushrooms".into()) }
    }

    /// Where the farm lies: the hall's floor, else the cellar's.
    fn farm_spot(&self) -> Option<Pos> {
        if let Some(f) = self.rooms.iter().find(|r| r.kind == super::delve::RoomKind::Farm).and_then(|r| r.bed) { return Some(f); }
        if let Some(&h) = self.hall_cells.first() { return Some(h); }
        self.projects.iter().find(|p| p.done && p.kind == projects::ProjectKind::Cellar).map(|p| p.at)
    }

    /// Whether the camp knows how (masons) or has the spores (a breached cavern).
    pub(crate) fn can_cave_farm(&self) -> bool {
        self.farm_spot().is_some() && (self.way.as_ref().map_or(false, |w| w.stone_first) || !self.breached.is_empty())
    }

    /// Every eighth day at 07:00: the harvest below.
    pub(crate) fn cave_harvest(&mut self) {
        let Some(p) = self.projects.iter().find(|p| p.done && p.kind == projects::ProjectKind::CaveFarm).cloned() else { return };
        let day = self.clock.day();
        if day <= p.day || (day - p.day) % 8 != 0 { return; }
        // Carried up to the mouth of the dig (the hall or cellar lies under the rock).
        let at = self.delve_mouth.or(self.farm_spot()).unwrap_or(p.at);
        let n = if self.breached.is_empty() { 12 } else { 20 };
        for _ in 0..n { self.items.push(Item::food(Stuff::Fungus, at, false)); }
        let crop = self.cave_crop();
        if self.milestones.insert("first cave harvest") {
            let line = format!("They pick the first {} in the farm under the rock: {} meals, and winter or no winter, more in eight days.", crop, n);
            self.note(line.clone());
            self.moment("The farm under the rock".into(), line, "because they dug down into the rock and planted it".into(), at);
        } else if self.hard_winter() {
            self.note(format!("Below the frozen ground the farm gives {} meals of {}.", n, crop));
        }
    }
}
