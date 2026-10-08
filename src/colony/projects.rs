//! After the first hut: the projects a colony sets itself.
//!
//! The hut used to be the colony's only goal; after day 2 the settlers kept food up and wandered.
//! Now each dawn, with nothing under way, the colony looks at its state and picks its next work,
//! logging why: a woodpile against the cold nights, a drying rack when food piles up, a second
//! hut when the first is crowded, a palisade when the trader's rumour names a threat. Where no
//! tree grows within reach they build in stone, quarried from boulders. Each project needs so
//! many logs or stones, laid one at a time by builders, and is stamped into the map when done
//! (a palisade rises a few posts per load as it goes).

use super::{Colony, ItemKind, Pos, HUT_H, HUT_W};
use crate::local::{Material, Plant, Shape};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectKind { Woodpile, DryingRack, SecondHut, Palisade, Windbreak, Smokehouse, Woodshed, Lookout, Fence, Mending, Storehouse, Workshop, Field, Jetty, Well, DugHall, Cellar, Mine, Temple, Lining, Still, Traps, LordsHall, CaveFarm, Tavern, Pen, DeepShaft, GuildHall, Kitchen, Library, Bedrooms, GreatHall, Tombs, Moat, Workshops, Hatch, Drawbridges, MasonShop, CarpenterShop, Smelter, Forge, Kiln,
    /// A gallery cut off the stair for its stone, when none is left in reach (`delve.rs`).
    StoneCut }

/// Works that feed the camp: built even while it goes hungry.
pub fn feeds(k: ProjectKind) -> bool { matches!(k, ProjectKind::Field | ProjectKind::CaveFarm | ProjectKind::Pen | ProjectKind::Jetty) }

/// Works that are dug, not built (no loads to lay).
pub fn is_dig(k: ProjectKind) -> bool { matches!(k, ProjectKind::DugHall | ProjectKind::Cellar | ProjectKind::Mine | ProjectKind::Bedrooms | ProjectKind::GreatHall | ProjectKind::Tombs | ProjectKind::Moat | ProjectKind::DeepShaft | ProjectKind::Workshops | ProjectKind::CaveFarm | ProjectKind::MasonShop | ProjectKind::CarpenterShop | ProjectKind::Smelter | ProjectKind::Forge | ProjectKind::Kiln | ProjectKind::StoneCut) }

impl ProjectKind {
    pub fn word(self) -> &'static str {
        match self {
            ProjectKind::Woodpile => "a woodpile", ProjectKind::DryingRack => "a drying rack", ProjectKind::SecondHut => "a second hut",
            ProjectKind::Palisade => "a palisade", ProjectKind::Windbreak => "a windbreak", ProjectKind::Smokehouse => "a smokehouse",
            ProjectKind::Woodshed => "a woodshed", ProjectKind::Lookout => "a lookout", ProjectKind::Fence => "a fence round the store",
            ProjectKind::Mending => "the mending of the palisade",
            ProjectKind::Storehouse => "a storehouse", ProjectKind::Temple => "a temple", ProjectKind::Workshop => "a workshop", ProjectKind::Field => "a fenced field",
            ProjectKind::Jetty => "a jetty", ProjectKind::Well => "a well",
            ProjectKind::DugHall => "a hall in the hill", ProjectKind::Cellar => "a cellar", ProjectKind::Mine => "a mine", ProjectKind::Lining => "the lining of the wet shaft", ProjectKind::Still => "a still", ProjectKind::Traps => "cage traps at the gates", ProjectKind::LordsHall => "a hall for the lord", ProjectKind::CaveFarm => "a farm under the rock", ProjectKind::Tavern => "a tavern", ProjectKind::Pen => "a pen for beasts", ProjectKind::DeepShaft => "the deep shaft", ProjectKind::GuildHall => "a guildhall", ProjectKind::Kitchen => "a kitchen", ProjectKind::Library => "a library", ProjectKind::Bedrooms => "bedrooms under the rock", ProjectKind::GreatHall => "a great hall below", ProjectKind::Tombs => "tombs under the rock", ProjectKind::Moat => "a ditch round the wall", ProjectKind::Workshops => "workshops below", ProjectKind::Hatch => "a hatch over the stair below", ProjectKind::Drawbridges => "drawbridges over the ditch",
            ProjectKind::MasonShop => "a mason's workshop below", ProjectKind::CarpenterShop => "a carpenter's workshop below", ProjectKind::Smelter => "a smelter below", ProjectKind::Forge => "a forge below", ProjectKind::Kiln => "a kiln below", ProjectKind::StoneCut => "a gallery cut for stone",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Project {
    pub kind: ProjectKind,
    /// Top-left of its footprint (the camp's centre for a palisade).
    pub at: Pos,
    pub needed: u32,
    pub used: u32,
    pub material: ItemKind,
    /// Why the colony took it on.
    pub why: String,
    pub done: bool,
    pub day: u64,
}

/// Radius of the palisade ring around the camp (cells).
/// Lots keep off the ring between these distances from the wall (`wall_r`): 1.5 inside it to
/// 4.5 outside (the ditch is dug 2 outside).
const RING_IN: f32 = -1.5;
const RING_OUT: f32 = 4.5;

impl Colony {
    /// The work under way: the hut until it stands, then the first unfinished project.
    pub(crate) fn active_project(&self) -> Option<usize> {
        // Nothing is built on ground the patron forbade.
        let ok = |p: &Project| !self.marked_at(p.at, true);
        // A woodpile running low is restocked before anything else.
        // (Only while there is wood to have: dev 50,20 burnt its trees out of reach, and waiting
        // on the woodpile held the palisade's mending for thirty days.)
        let wood = || self.wood_in_reach || self.items.iter().any(|it| it.kind == ItemKind::Log);
        if let Some(k) = self.projects.iter().position(|p| p.done && p.kind == ProjectKind::Woodpile && p.used < 4 && ok(p)).filter(|_| wood()) { return Some(k); }
        // (Digs are worked from their own plan, `dig_target`: the builders go on with the next
        // work meanwhile. A deep shaft stuck at its last cuts had held two guildhalls at no
        // stones for sixty days.)
        self.projects.iter().position(|p| !p.done && ok(p) && !is_dig(p.kind))
            // A finished woodpile burns down each night and is restocked.
            .or_else(|| self.projects.iter().position(|p| p.done && p.kind == ProjectKind::Woodpile && p.used < p.needed && ok(p)).filter(|_| wood()))
    }

    /// Whether the fire has wood (a stocked woodpile) or a stone windbreak shelters it.
    pub(crate) fn fire_kept(&self) -> bool {
        self.projects.iter().any(|p| p.done && match p.kind { ProjectKind::Woodpile => p.used > 0, ProjectKind::Windbreak => true, _ => false })
    }

    /// Each night the fire burns two logs from the woodpile.
    pub(crate) fn burn_wood(&mut self) {
        let Some(k) = self.projects.iter().position(|p| p.done && p.kind == ProjectKind::Woodpile) else { return };
        let before = self.projects[k].used;
        let burn = if self.hard_winter() { 4 } else { 2 };
        self.projects[k].used = before.saturating_sub(burn);
        if before > 0 && self.projects[k].used == 0 { self.note("The woodpile is burnt out; the fire will be small tonight.".into()); }
    }

    /// What the work under way still needs: (logs, stones).
    pub(crate) fn material_needed(&self) -> (u32, u32) {
        let mut need = (0, 0);
        let mut add = |m: ItemKind, n: u32| match m { ItemKind::Stone => need.1 += n, _ => need.0 += n };
        if let Some(h) = self.hut.as_ref().filter(|h| !h.done) { add(self.hut_material, super::HUT_LOGS.saturating_sub(h.logs_used)); }
        else if let Some(k) = self.active_project().filter(|&k| !is_dig(self.projects[k].kind)) { let p = &self.projects[k]; add(p.material, p.needed.saturating_sub(p.used)); }
        need
    }

    /// The material the work under way takes now.
    pub(crate) fn building_material(&self) -> Option<ItemKind> {
        if self.hut.as_ref().map_or(false, |h| !h.done) { return Some(self.hut_material); }
        self.active_project().filter(|&k| !is_dig(self.projects[k].kind)).map(|k| self.projects[k].material)
    }

    /// Whether a tree stands within working reach of the camp (else they build in stone).
    /// Neither timber nor quarry stone within reach, and hardly any stone laid by.
    pub(crate) fn materials_out(&self) -> bool {
        !self.timber_near() && self.nearest_pub(self.camp, |c, p| c.is_quarry_stone_pub(p)).is_none()
            && self.items.iter().filter(|it| it.kind == ItemKind::Stone && it.stored).count() < 4
    }

    pub(crate) fn timber_near(&self) -> bool {
        self.nearest_tree_by(self.camp, |_, _| true).is_some()
    }

    /// A work under way whose material has run out within reach (dev 50,20 waited thirty days to
    /// mend the palisade in stone with every boulder in reach already broken) is finished in the
    /// other material, or set aside when there is neither.
    fn reckon_materials(&mut self) {
        let have = |c: &Colony, m: ItemKind| c.items.iter().any(|it| it.kind == m && (it.stored || it.reserved || c.reachable_pub(c.camp, it.at)))
            || match m { ItemKind::Stone => c.nearest(c.camp, |c, p| c.is_quarry_stone(p)).is_some(), ItemKind::Log => c.timber_near(), ItemKind::Food => true };
        for k in 0..self.projects.len() {
            let p = &self.projects[k];
            if p.done || is_dig(p.kind) || have(self, p.material) { continue; }
            let (m, word) = (p.material, p.kind.word());
            let other = if m == ItemKind::Stone { ItemKind::Log } else { ItemKind::Stone };
            if have(self, other) {
                self.projects[k].material = other;
                self.note(format!("No {} is left within reach; they finish {} in {}.", if m == ItemKind::Stone { "stone" } else { "timber" }, word, if other == ItemKind::Stone { "stone" } else { "timber" }));
            } else {
                self.projects[k].done = true;
                let key = (self.projects[k].kind, self.projects[k].at, self.projects[k].day);
                if !matches!(key.0, ProjectKind::Lining | ProjectKind::Mending) { self.set_aside.push(key); }
                self.note(format!("Neither stone nor timber is left within reach; {} is set aside, {} of {} loads laid.", word, self.projects[k].used, self.projects[k].needed));
            }
        }
        // A work set aside is taken up again when stone or timber is to hand (one a dawn; a
        // palisade had stood at 14 of 24 loads for good once the boulders in reach were broken).
        if let Some(n) = self.set_aside.iter().position(|key| self.projects.iter().any(|p| (p.kind, p.at, p.day) == *key)) {
            let key = self.set_aside[n];
            let k = self.projects.iter().position(|p| (p.kind, p.at, p.day) == key).unwrap();
            let m = self.projects[k].material;
            let other = if m == ItemKind::Stone { ItemKind::Log } else { ItemKind::Stone };
            // (Enough laid by to finish it, or ten loads: a few stones at a time had set it aside
            // and taken it up again every other dawn.)
            let left = (self.projects[k].needed - self.projects[k].used.min(self.projects[k].needed)).min(10) as usize;
            let stored = |c: &Colony, x: ItemKind| c.items.iter().filter(|it| it.kind == x && it.stored).count() >= left.max(1);
            let take = if have(self, m) || stored(self, m) { Some(m) } else if have(self, other) || stored(self, other) { Some(other) } else { None };
            if let Some(t) = take {
                self.set_aside.remove(n);
                self.projects[k].done = false;
                self.projects[k].material = t;
                let p = &self.projects[k];
                self.note(format!("They take up {} again ({} of {} loads laid): there is {} to hand now.", p.kind.word(), p.used, p.needed, if t == ItemKind::Stone { "stone" } else { "timber" }));
            }
        }
        self.set_aside.retain(|key| self.projects.iter().any(|p| (p.kind, p.at, p.day) == *key));
        // A dig waiting on its lining, with the lining set aside: the wet shaft is abandoned
        // (dev 70,6 had planned nothing from day 39 to 150 behind a mine paused for ever).
        if self.dig_paused && !self.aquifer_lined && !self.projects.iter().any(|p| p.kind == ProjectKind::Lining && !p.done) {
            self.dig_paused = false;
            if let Some(k) = self.projects.iter().position(|q| !q.done && is_dig(q.kind)) {
                if let Some(first) = self.dig_plan.as_ref().and_then(|p| p.first().map(|c| c.p)) { self.digs_given_up.push(first); }
                let kind = self.projects[k].kind;
                self.projects.remove(k);
                self.dig_plan = None;
                self.dig_rooms.clear();
                self.note(format!("They abandon {}: the water in the rock cannot be held back with nothing to line it.", kind.word()));
            }
        }
    }

    /// At dawn, with nothing under way, the camp reckons its horizons (food against the days to
    /// winter, beds against heads, firewood against the cold, nights to a foretold raid against
    /// the wall) and the worst shortfall picks the next work, said aloud with its numbers. When
    /// nothing presses they improve what they have, so the list never ends.
    pub(crate) fn plan_projects(&mut self) {
        self.wood_in_reach = self.timber_near();
        self.reckon_materials();
        // A camp hungry five dawns running plans a work that feeds it (a field, a farm under the
        // rock, a pen, a jetty) ahead of whatever else is under way (seed 58 sat a year hungry
        // behind a well it never finished, building nothing because it was hungry).
        let under_way = self.projects.iter().any(|p| !p.done && !self.marked_at(p.at, true));
        let food_first = under_way && self.hungry_days >= 5 && !self.projects.iter().any(|p| !p.done && feeds(p.kind));
        if self.hut.as_ref().map_or(true, |h| !h.done) || (under_way && !food_first) { return; }
        let have = |c: &Colony, k: ProjectKind| c.projects.iter().any(|p| p.kind == k);
        let alive = self.alive();
        let material = if self.timber_near() && !self.way.as_ref().map_or(false, |w| w.stone_first) { ItemKind::Log } else { ItemKind::Stone };
        let food = self.food_stored();
        let day = self.clock.day();
        let hut = self.hut.as_ref().unwrap().at;
        let days_of_food = self.days_of_food();
        let winter = self.days_to_winter().filter(|&d| d > 0);
        let arc = self.arc.as_ref();
        let raid_in = arc.filter(|a| a.stage == 1 || a.stage == 2 || a.stage == 5).map(|a| a.raid_day.saturating_sub(day));
        let threat = arc.filter(|a| !a.events.is_empty()).map(|a| a.threat.name.clone());
        let last_raid_killed = arc.and_then(|a| a.events.iter().rev().find(|e| e.title == "The raid")).filter(|e| e.text.contains("was killed") && day <= e.day + 12).map(|e| e.day);
        let beds = super::HUT_BEDS + if have(self, ProjectKind::SecondHut) { 6 } else { 0 };
        // Candidates: (urgency, kind, why, loads, site).
        let mut c: Vec<(f32, ProjectKind, String, u32, Pos)> = Vec::new();
        if material == ItemKind::Stone && !have(self, ProjectKind::Windbreak) {
            c.push((2.2, ProjectKind::Windbreak, format!("the nights are cold ({:.0} °C by day), and there is no wood to burn", self.temperature()), 10, (hut.0, hut.1.saturating_sub(2))));
        }
        if material == ItemKind::Log && !have(self, ProjectKind::Woodpile) {
            c.push((2.2, ProjectKind::Woodpile, format!("the nights are cold ({:.0} °C by day), and no firewood is stacked", self.temperature()), 8, (hut.0 + HUT_W as u16 + 1, hut.1 + 1)));
        }
        if let Some(t) = threat.as_ref().filter(|_| !have(self, ProjectKind::Palisade)) {
            let n = raid_in.unwrap_or(20);
            let many = ["raiders", "deserters", "the Exiles", "outlaws"].iter().any(|w| t.starts_with(w));
            c.push((3.0 + 2.0 / (n as f32 + 1.0), ProjectKind::Palisade, format!("{} {} foretold{}, and there is no wall", t, if many { "are" } else { "is" }, raid_in.map(|n| format!(" in {} nights", n)).unwrap_or_default()), 24, self.camp));
        }
        if alive > beds {
            if let Some(at) = self.find_site_for(ProjectKind::SecondHut, HUT_W as u16, HUT_H as u16).filter(|_| !have(self, ProjectKind::SecondHut)) {
                c.push((2.0 + 0.3 * (alive - beds) as f32, ProjectKind::SecondHut, format!("there are {} of them and {} beds", alive, beds), 30, at));
            }
        }
        if let Some(d) = winter.filter(|&d| d <= 70 && !have(self, ProjectKind::Smokehouse)) {
            if let Some(at) = self.find_site_for(ProjectKind::Smokehouse, 3, 3) {
                c.push((2.5 + (70 - d.min(70)) as f32 / 30.0, ProjectKind::Smokehouse, format!("winter is {} days off and the store holds {:.0} days of food, and nothing keeps", d, days_of_food), 16, at));
            }
        }
        if let Some(d) = winter.filter(|&d| d <= 50 && material == ItemKind::Log && have(self, ProjectKind::Woodpile) && !have(self, ProjectKind::Woodshed)) {
            c.push((1.9, ProjectKind::Woodshed, format!("winter is {} days off; the woodpile holds 8 logs and a winter's nights burn a hundred", d), 10, (hut.0 + HUT_W as u16 + 1, hut.1 + 2)));
        }
        if food >= 3 * alive as u32 && !have(self, ProjectKind::DryingRack) {
            if let Some(at) = self.find_site_for(ProjectKind::DryingRack, 2, 2) {
                c.push((1.5, ProjectKind::DryingRack, format!("{} meals are stored and spoil in about 6 days", food), 6, at));
            }
        }
        if !have(self, ProjectKind::Lookout) && (last_raid_killed.is_some() || arc.map_or(false, |a| a.chapter > 0)) {
            let why = match last_raid_killed { Some(d) => format!("the raid of day {} took one of them before the camp was awake", d), None => format!("{} troubles have come in {} days, and they want to see the next one first", arc.map_or(0, |a| a.chapter + 1), day) };
            // At the watch post if its ground is clear, else the next free lot.
            let post = self.watch_post();
            let clear = (0..2u16).all(|dy| (0..2u16).all(|dx| { let q = (post.0 + dx, post.1 + dy); super::nav::passable(&self.map, q) && !self.built_near(q) && self.map.roofs[q.1 as usize * self.map.width + q.0 as usize] == 0 }));
            if let Some(at) = if clear { Some(post) } else { self.find_site_for(ProjectKind::Lookout, 2, 2) } {
                c.push((if last_raid_killed.is_some() { 3.2 } else { 1.2 }, ProjectKind::Lookout, why, 8, at));
            }
        }
        // The deep shaft below the cavern floor (`deep.rs`).
        if !have(self, ProjectKind::DeepShaft) && self.wants_deep_shaft() && self.dig_plan.is_none() {
            if let Some(plan) = self.plan_dig(ProjectKind::DeepShaft) {
                let below = plan.cuts.len();
                let at = self.spine.map(|s| s.at).unwrap_or(self.camp);
                c.push((1.1, ProjectKind::DeepShaft, format!("the cavern floor is not the bottom of the world: {} levels of rock lie under the stair's foot, and the miners say the deep glitters", below), below as u32, at));
            }
        }
        // A pen for beasts of a herd nearby (`livestock.rs`).
        if !have(self, ProjectKind::Pen) && day >= 40 && self.alive() >= 10 {
            if let (Some(herd), Some(at)) = (self.herd_near(), self.find_site_for(ProjectKind::Pen, 6, 5).or_else(|| self.find_site_for_sloped(ProjectKind::Pen, 6, 5, 1))) {
                c.push((1.2, ProjectKind::Pen, format!("{} graze within 30 paces; two penned would breed, and meat would keep on the hoof through {} winter days", herd, super::SEASON_DAYS), 10, at));
            }
        }
        // A tavern, once travellers have come and the camp has twelve (`tavern.rs`).
        // A library, once the camp has written two books (`books.rs`): they are kept, not sold.
        let books = self.works.iter().filter(|w| w.kind == "book").count();
        if !have(self, ProjectKind::Library) && books >= 2 && self.alive() >= 10 {
            if let Some(at) = self.find_site_for(ProjectKind::Library, 4, 3) {
                c.push((1.0, ProjectKind::Library, format!("{} books lie about the huts, and the damp and the traders get at them", books), 12, at));
            }
        }
        // A kitchen, once twelve live here and a storehouse keeps the food (`kitchen.rs`).
        if !have(self, ProjectKind::Kitchen) && self.alive() >= 12 && self.projects.iter().any(|p| p.done && p.kind == ProjectKind::Storehouse) {
            if let Some(at) = self.find_site_for(ProjectKind::Kitchen, 3, 3) {
                c.push((1.1, ProjectKind::Kitchen, format!("{} eat what they can grab from the store, and none of it cooked", self.alive()), 8, at));
            }
        }
        if !have(self, ProjectKind::Tavern) && self.alive() >= 12 && self.visitors.iter().any(|v| v.came) {
            if let Some(at) = self.find_site_for(ProjectKind::Tavern, 5, 4) {
                let came = self.visitors.iter().filter(|v| v.came).count();
                c.push((1.3, ProjectKind::Tavern, format!("{} {} come by the road and slept by the fire, and {} live here", came, if came == 1 { "traveller has" } else { "travellers have" }, self.alive()), 16, at));
            }
        }
        // A farm under the rock, once a hall or cellar is dug (`cavefarm.rs`).
        // (Dug as a level of plots off the stair: one dig at a time.)
        let farm_cuts = if !have(self, ProjectKind::CaveFarm) && self.can_cave_farm() && self.dig_plan.is_none() && !self.projects.iter().any(|p| !p.done && is_dig(p.kind)) { self.plan_dig(ProjectKind::CaveFarm).map(|p| p.cuts.len() as u32) } else { None };
        if let Some(cuts) = farm_cuts {
            let d = self.days_to_winter();
            let at = self.spine.map(|s| s.at).unwrap_or(self.camp);
            let why = match d {
                Some(d) => format!("{} live here and winter is {} days off; under the rock things grow in any season", self.alive(), d),
                None => format!("{} live here, and under the rock things grow in any season", self.alive()),
            };
            c.push((if d.map_or(false, |d| d <= 60) { 2.2 } else { 1.2 }, ProjectKind::CaveFarm, why, cuts, at));
        }
        // Cage traps at the gates, with the palisade up and a beast foretold or still to come
        // (`traps.rs`).
        if !have(self, ProjectKind::Traps) && self.projects.iter().any(|p| p.done && p.kind == ProjectKind::Palisade) {
            let beast = arc.and_then(|a| {
                let now = matches!(a.stage, 1 | 2 | 5) && matches!(a.threat.kind, super::arc::ThreatKind::Beast | super::arc::ThreatKind::Deep);
                if now { Some(a.threat.name.clone()) } else { a.later.iter().find(|t| t.kind == super::arc::ThreatKind::Beast).map(|t| t.name.clone()) }
            });
            if let Some(b) = beast {
                let n = self.gates().len();
                c.push((2.0, ProjectKind::Traps, format!("for fear of {}: a cage on a trip-stone at each of the {} gates", b, n), 8, self.camp));
            }
        }
        // A still, once a workshop stands and berries are to spare (`drink.rs`; dwarves sooner).
        if !have(self, ProjectKind::Still) && self.projects.iter().any(|p| p.done && p.kind == ProjectKind::Workshop) && food >= 3 * self.alive() as u32 {
            if let Some(at) = self.find_site_for(ProjectKind::Still, 2, 2) {
                let dwarves = self.settlers.iter().filter(|s| s.alive && s.persona.race == "dwarf").count();
                c.push((if dwarves * 2 >= self.alive() { 2.2 } else { 0.9 }, ProjectKind::Still, format!("{} meals are stored{}", food, if dwarves > 0 { format!(", and {} of them are dwarves, who want a drink with their meals", dwarves) } else { ", and berries keep better as wine".to_string() }), 6, at));
            }
        }
        if have(self, ProjectKind::Smokehouse) && !have(self, ProjectKind::Fence) {
            if let Some(at) = self.find_site_for(ProjectKind::Fence, 4, 4) {
                c.push((1.0, ProjectKind::Fence, format!("the store holds {} meals, and animals come at night", food), 8, at));
            }
        }
        // The palisade weathers: mended every twenty days or so.
        let last_mend = self.projects.iter().filter(|p| matches!(p.kind, ProjectKind::Mending | ProjectKind::Palisade)).map(|p| p.day).max();
        if let Some(m) = last_mend.filter(|&m| day >= m + 20) {
            c.push((0.8 + if last_raid_killed.is_some() { 2.5 } else { 0.0 }, ProjectKind::Mending, format!("the palisade has stood {} days in the weather", day - m), 6, self.camp));
        }
        // Buildings with a job, each answering a worry with a number.
        if food >= 4 * alive as u32 && !have(self, ProjectKind::Storehouse) {
            if let Some(at) = self.find_site_for(ProjectKind::Storehouse, 4, 3) {
                c.push((1.4, ProjectKind::Storehouse, format!("{} meals lie by the fire, and the rain gets at them", food), 12, at));
            }
        }
        let laid: u32 = self.settlers.iter().map(|s| s.loads_laid).sum();
        if laid >= 60 && !have(self, ProjectKind::Workshop) {
            if let Some(at) = self.find_site_for(ProjectKind::Workshop, 5, 4) {
                c.push((1.3, ProjectKind::Workshop, format!("they have laid {} loads with bad tools, and every log takes too long", laid), 14, at));
            }
        }
        if matches!(self.season(), crate::seasons::Season::Spring | crate::seasons::Season::Summer) && !have(self, ProjectKind::Field) && self.temperature() > 4.0 {
            if let Some(at) = self.find_site_for(ProjectKind::Field, 8, 6).or_else(|| self.find_site_for_sloped(ProjectKind::Field, 8, 6, 1)) {
                let to_autumn = (2 * super::SEASON_DAYS).saturating_sub((self.clock.day().max(1) - 1) % (4 * super::SEASON_DAYS));
                c.push((1.6, ProjectKind::Field, format!("the ground is soft and the harvest is {} days off; a field would feed them through the winter", to_autumn), 8, at));
            }
        }
        if !have(self, ProjectKind::Jetty) {
            if let Some(spot) = self.nearest_fishing(self.camp, super::FORAGE_RADIUS) {
                c.push((1.1, ProjectKind::Jetty, format!("the fish give out where they fish the bank; a jetty at {},{} reaches the deep water", spot.0, spot.1), 8, spot));
            }
        }
        if self.water_walked > 600 && !have(self, ProjectKind::Well) {
            c.push((1.5, ProjectKind::Well, format!("they have walked {} hours for water, the nearest {} cells from the fire", self.water_walked / 60, self.water_distance), 10, (self.camp.0 + 2, self.camp.1.saturating_sub(2))));
        }
        // Reasons to dig: warm, safe, cool. A hall into the hill beside the camp holds them all
        // through winter; on flat ground a cellar keeps the food.
        if !have(self, ProjectKind::DugHall) && !have(self, ProjectKind::Cellar) && self.dig_plan.is_none() {
            let to_winter = self.days_to_winter();
            if let Some(cells) = self.plan_dig(ProjectKind::DugHall).map(|p| p.cuts) {
                let chilled = format!("; {} nights have ended with someone chilled", self.chilled_nights);
                let why = match to_winter { Some(d) if d > 0 => format!("the hill beside the camp could hold them all, warm through the winter {} days off, and its one door can be held{}", d, chilled), _ => format!("the hill beside the camp could hold them all, warm and safe behind one door{}", chilled) };
                c.push((if to_winter.map_or(false, |d| d <= 80) { 2.4 } else { 1.2 }, ProjectKind::DugHall, why, cells.len() as u32, cells[0].p));
            } else if let Some(cells) = self.plan_dig(ProjectKind::Cellar).map(|p| p.cuts) {
                let why = format!("a cellar in the rock keeps food three times as long{}", to_winter.filter(|&d| d > 0).map(|d| format!(", and winter is {} days off", d)).unwrap_or_default());
                c.push((if to_winter.map_or(false, |d| d <= 80) { 1.7 } else { 0.9 }, ProjectKind::Cellar, why, cells.len() as u32, cells[0].p));
            }
        }
        // Reasons to dig deeper: rich. With a hall or cellar dug and a workshop to work ore (or a
        // people who dig), they sink a mine after it (`mine.rs`), and may dig too deep.
        let dug = self.projects.iter().any(|p| p.done && matches!(p.kind, ProjectKind::DugHall | ProjectKind::Cellar));
        let wants_ore = have(self, ProjectKind::Workshop) || self.way.as_ref().map_or(false, |w| w.stone_first);
        if dug && wants_ore && day >= 12 && !have(self, ProjectKind::Mine) && self.dig_plan.is_none() {
            if let Some(cells) = self.plan_dig(ProjectKind::Mine).map(|p| p.cuts) {
                let why = if self.ore_found > 0 { format!("the seam struck in the dig runs on down into the rock, and {} seams are not enough", self.ore_found) }
                    else { format!("they have dug {} loads of stone near the surface and struck no ore; the workshop wants it, and the deep rock may hold it", self.stone_dug) };
                c.push((1.25, ProjectKind::Mine, why, cells.len() as u32, cells[0].p));
            }
        }
        // Rooms down the stair: bedrooms, a great hall (`delve.rs`).
        self.delve_candidates(&mut c);
        // A temple to their god, when three or more of them are devout and share a faith (`Past::faith`).
        if !have(self, ProjectKind::Temple) && alive >= 8 && day >= 20 {
            if let Some((god, n)) = self.devout_faith() {
                if n >= 3 {
                    if let Some(at) = self.find_site_for(ProjectKind::Temple, 4, 4) {
                        c.push((1.2 + 0.2 * n as f32, ProjectKind::Temple, format!("{} of them are devout and pray to {}, with no roof but the sky", n, god), 16, at));
                    }
                }
            }
        }
        // A people's way: some works sooner, some later.
        if let Some(way) = &self.way {
            for cand in c.iter_mut() {
                let name = format!("{:?}", cand.1);
                if way.first.contains(&name) { cand.0 *= 1.6; }
                if way.late.contains(&name) { cand.0 *= 0.5; }
            }
        }
        // The camp's people press for works of their own wanting (`voices.rs`).
        // (Debug: PLANET_NO_VOICES=1 plans as before, to compare.)
        if std::env::var("PLANET_NO_VOICES").is_err() {
            self.pressed_candidates(&mut c);
            self.weigh_voices(&mut c);
        }
        if let Some((p, d)) = self.breach {
            c.push((4.0, ProjectKind::Mending, format!("the raid of day {} broke the palisade at {},{}", d, p.0, p.1), 6, self.camp));
        }
        if food_first { c.retain(|x| feeds(x.1)); }
        // No stone or timber to be had: only digs (which bring up stone) are worth planning (a
        // mending had been planned and set aside every dawn).
        if self.materials_out() { c.retain(|x| is_dig(x.1)); }
        c.sort_by(|a, b| b.0.total_cmp(&a.0));
        let Some((_, kind, why, needed, at)) = c.into_iter().next() else { return };
        // A fixed place on forbidden ground: take the plan's next lot instead.
        let at = if kind != ProjectKind::Palisade && kind != ProjectKind::Mending && self.marked_at(at, true) {
            match Self::footprint(kind).and_then(|(w, h)| self.find_site_for(kind, w, h)) { Some(p) => p, None => return }
        } else { at };
        let material = if matches!(kind, ProjectKind::Woodpile | ProjectKind::Woodshed) { ItemKind::Log } else { material };
        if kind == ProjectKind::Woodpile && !self.timber_near() { return; }
        let stuff = if material == ItemKind::Stone { "stone" } else { "timber" };
        self.plan_line = format!("Next: {}. {}.", kind.word().trim_start_matches("the "), super::arc::capital_word(&why));
        if is_dig(kind) || matches!(kind, ProjectKind::Traps | ProjectKind::CaveFarm | ProjectKind::DeepShaft | ProjectKind::Hatch | ProjectKind::Drawbridges) { self.note(format!("They set to work on {}: {}.", kind.word(), why)); }
        else { self.note(format!("They set to work on {} of {}: {}.", kind.word(), stuff, why)); }
        if is_dig(kind) {
            match self.plan_dig(kind) { Some(plan) => self.begin_dig(plan), None => return }
        }
        // The palisade's gates are chosen as it is begun (`traps.rs`).
        if kind == ProjectKind::Palisade && self.gate_dirs.is_empty() { self.choose_gates(); }
        let p = Project { kind, at, needed, used: 0, material, why, done: false, day };
        if food_first { self.projects.insert(0, p); } else { self.projects.push(p); }
    }

    /// Dawn: how many dawns running the camp has gone hungry.
    pub(crate) fn reckon_hunger_days(&mut self) {
        let hungry = self.food_stored() < 2 * self.alive() as u32 && self.settlers.iter().any(|o| o.alive && o.hunger > 0.85);
        self.hungry_days = if hungry { self.hungry_days + 1 } else { 0 };
    }

    /// `find_site` for the other modules.
    pub(crate) fn find_site_pub(&self, w: u16, h: u16) -> Option<Pos> { self.find_site(w, h) }

    /// A free, flat, open footprint of `w` x `h` near the camp (not on the hut or the camp).
    fn find_site(&self, w: u16, h: u16) -> Option<Pos> { self.find_site_sloped(w, h, 0) }

    /// `find_site` allowing the ground to rise or fall `slope` levels across the lot (a field
    /// can lie on gently sloping ground; a building cannot).
    fn find_site_sloped(&self, w: u16, h: u16, slope: i32) -> Option<Pos> {
        // Within the wall (3 past its radius); once that is full, beyond the ditch.
        let r = self.wall_r();
        self.find_site_within(w, h, slope, r + 3, None).or_else(|| self.find_site_within(w, h, slope, r + 13, None))
    }

    /// `find_site` for a work of `kind`: the lot is also chosen for what the work is for (the
    /// temple and the lookout on high ground, the workshop toward the timber, the smokehouse and
    /// the rack toward the fishing, the store and the kitchen by the hut, the field on deep soil,
    /// the pen toward the herd, the library away from the noise, the tavern by the fire).
    pub(crate) fn find_site_for(&self, kind: ProjectKind, w: u16, h: u16) -> Option<Pos> { self.find_site_for_sloped(kind, w, h, 0) }

    pub(crate) fn find_site_for_sloped(&self, kind: ProjectKind, w: u16, h: u16, slope: i32) -> Option<Pos> {
        // Fields and pens lie outside the wall, as round any walled village.
        let r = self.wall_r();
        if matches!(kind, ProjectKind::Field | ProjectKind::Pen) { return self.find_site_within(w, h, slope, r + 15, Some(kind)); }
        self.find_site_within(w, h, slope, r + 3, Some(kind)).or_else(|| self.find_site_within(w, h, slope, r + 13, Some(kind)))
    }

    /// What a lot at `(x, y)` (its middle `(mx, my)`) costs a work of `kind` beyond its distance
    /// from the fire (lower is better).
    fn purpose_cost(&self, kind: ProjectKind, x: i32, y: i32, mx: i32, my: i32, near: &Near) -> i32 {
        let n = self.map.width;
        let z = self.map.surface_z[y as usize * n + x as usize];
        let dist = |p: Option<Pos>| p.map_or(0, |p| (mx - p.0 as i32).abs().max((my - p.1 as i32).abs()));
        let d = (mx - self.camp.0 as i32).abs().max((my - self.camp.1 as i32).abs());
        match kind {
            ProjectKind::Temple => -4 * (z - near.zc) + dist(near.shrine) - d / 3,
            ProjectKind::Lookout => -5 * (z - near.zc),
            ProjectKind::Library => -d / 2,
            ProjectKind::Workshop | ProjectKind::GuildHall => dist(near.tree) * 3 / 5,
            ProjectKind::Smokehouse | ProjectKind::DryingRack => dist(near.water) * 3 / 5,
            ProjectKind::Storehouse | ProjectKind::Kitchen | ProjectKind::Still | ProjectKind::SecondHut | ProjectKind::Fence => dist(near.hut) * 3 / 5,
            ProjectKind::Tavern => d,
            ProjectKind::Pen => dist(near.herd) / 2,
            ProjectKind::Field => {
                // Deep soil (not rock a hand's depth down): up to four levels of it.
                let soil = (1..=4).take_while(|k| {
                    let zz = z - k;
                    zz >= 0 && matches!(self.map.cell(x as usize, y as usize, zz as usize).material, crate::local::Material::Soil | crate::local::Material::Clay)
                }).count() as i32;
                -5 * soil
            }
            _ => 0,
        }
    }

    fn find_site_within(&self, w: u16, h: u16, slope: i32, radius: i32, purpose: Option<ProjectKind>) -> Option<Pos> {
        let n = self.map.width as i32;
        let (cx, cy) = (self.camp.0 as i32, self.camp.1 as i32);
        let (xs, xe) = ((cx - radius).max(2), (cx + radius).min(n - w as i32 - 2));
        let (ys, ye) = ((cy - radius).max(2), (cy + radius).min(n - h as i32 - 2));
        if xs >= xe || ys >= ye { return None; }
        // The cells the lots cover, each marked once: taken (on or beside the hut or a work, by
        // the fire, over a dig) or impassable. (Asked per lot and cell, the list of works and
        // dug cells was most of the camp's planning.)
        let (bw, bh) = (xe - xs + w as i32 - 1, ye - ys + h as i32 - 1);
        let mut blocked = vec![false; (bw * bh) as usize];
        let mut block = |x0: i32, y0: i32, x1: i32, y1: i32| {
            for y in y0.max(ys)..=y1.min(ys + bh - 1) {
                for x in x0.max(xs)..=x1.min(xs + bw - 1) { blocked[((y - ys) * bw + x - xs) as usize] = true; }
            }
        };
        let rect = |at: Pos| (at.0 as i32 - 1, at.1 as i32 - 1, at.0 as i32 + HUT_W as i32, at.1 as i32 + HUT_H as i32);
        if let Some(hh) = self.hut.as_ref() { let (a, b, c, d) = rect(hh.at); block(a, b, c, d); }
        for p in self.projects.iter().filter(|p| p.kind != ProjectKind::Palisade) { let (a, b, c, d) = rect(p.at); block(a, b, c, d); }
        block(cx - 2, cy - 2, cx + 2, cy + 2);
        // Nor over a dig (a hut raised over a cellar's ramp trapped whoever was inside).
        for q in self.dig_plan.iter().flatten().filter(|c| c.z + 3 >= self.map.surface_z[c.p.1 as usize * self.map.width + c.p.0 as usize]).map(|c| c.p)
            .chain(self.hall_cells.iter().copied()).chain(self.delve_mouth).chain(self.spine.map(|s| s.at)) {
            block(q.0 as i32 - 1, q.1 as i32 - 1, q.0 as i32 + 1, q.1 as i32 + 1);
        }
        for y in ys..ys + bh {
            for x in xs..xs + bw {
                let k = ((y - ys) * bw + x - xs) as usize;
                if !blocked[k] && !super::nav::passable(&self.map, (x as u16, y as u16)) { blocked[k] = true; }
            }
        }
        let mut best: Option<(i32, Pos)> = None;
        let lane = self.lane();
        let near = purpose.map(|_| self.near_things());
        let wall = self.wall_r() as f32;
        // How the camp lays itself out: a sociable, orderly people keeps close about the fire;
        // an independent one spreads out (x0.6 .. x1.4 on the distance from the fire).
        let spread = self.camp_spread();
        for y in ys..ye {
            for x in xs..xe {
                let z0 = self.map.surface_z[(y * n + x) as usize];
                let ok = (0..h as i32).all(|dy| (0..w as i32).all(|dx| {
                    let (xx, yy) = (x + dx, y + dy);
                    !blocked[((yy - ys) * bw + xx - xs) as usize] && (self.map.surface_z[(yy * n + xx) as usize] - z0).abs() <= slope
                }));
                if !ok { continue; }
                // Off the ring the palisade and its ditch take (`wall_r`, and 2 past it): wholly
                // inside the wall or wholly beyond the ditch (a hut on the ring had left a gap).
                let corner = |dx: i32, dy: i32| (((x + dx - cx) * (x + dx - cx) + (y + dy - cy) * (y + dy - cy)) as f32).sqrt();
                let ds = [corner(0, 0), corner(w as i32, 0), corner(0, h as i32), corner(w as i32, h as i32)];
                let (dmin, dmax) = (ds.iter().cloned().fold(f32::MAX, f32::min), ds.iter().cloned().fold(0.0, f32::max));
                let outside_only = matches!(purpose, Some(ProjectKind::Field | ProjectKind::Pen));
                if !((dmax <= wall + RING_IN && !outside_only) || dmin >= wall + RING_OUT) { continue; }
                // Never on forbidden ground.
                if (0..h as i32).any(|dy| (0..w as i32).any(|dx| self.marked_at(((x + dx) as u16, (y + dy) as u16), true))) { continue; }
                // The plan: lots along the lane from the fire toward water (or the way the camp
                // looks), close to the fire; blessed ground first.
                let d = (x - cx).abs() + (y - cy).abs();
                let (lx, ly) = lane;
                let (rx, ry) = ((x + w as i32 / 2 - cx) as f32, (y + h as i32 / 2 - cy) as f32);
                let off_lane = (rx * ly - ry * lx).abs() as i32;
                let behind = if rx * lx + ry * ly < -2.0 { 6 } else { 0 };
                let blessed = (0..h as i32).any(|dy| (0..w as i32).any(|dx| self.marked_at(((x + dx) as u16, (y + dy) as u16), false)));
                let purpose = match (purpose, near.as_ref()) { (Some(k), Some(nr)) => self.purpose_cost(k, x, y, x + w as i32 / 2, y + h as i32 / 2, nr), _ => 0 };
                let score = (d as f32 * spread) as i32 + off_lane + behind + purpose - if blessed { 30 } else { 0 };
                if best.map_or(true, |b| score < b.0) { best = Some((score, (x as u16, y as u16))); }
            }
        }
        best.map(|b| b.1)
    }

    /// Lay one load into the project under way; stamp it into the map as it rises.
    pub(crate) fn project_step(&mut self, i: usize) {
        let Some(k) = self.active_project().filter(|&k| !is_dig(self.projects[k].kind)) else { return };
        let material = self.projects[k].material;
        let Some(item) = self.items.iter().position(|it| it.kind == material && it.stored) else { return };
        self.items.remove(item);
        self.fix_item_refs(item);
        let name = self.settlers[i].name.clone();
        self.projects[k].used += 1;
        self.count_load(i);
        let (kind, used, needed, at) = (self.projects[k].kind, self.projects[k].used, self.projects[k].needed, self.projects[k].at);
        if kind == ProjectKind::Palisade { self.raise_palisade(at, used, needed, material); }
        if self.projects[k].done { return; }
        if used >= needed {
            self.projects[k].done = true;
            // Those at the building feel it stand.
            for j in 0..self.settlers.len() {
                if self.settlers[j].alive && (j == i || self.settlers[j].job == super::Job::Build) {
                    self.feel(j, super::mind::Feel::Built { what: kind.word().to_string() });
                }
            }
            match kind {
                ProjectKind::SecondHut => self.raise_hut_at(at, material == ItemKind::Stone),
                ProjectKind::Woodpile => self.stamp_block(at, 3, 1, material),
                ProjectKind::Windbreak => self.stamp_block(at, HUT_W as u16, 1, material),
                ProjectKind::DryingRack => self.stamp_posts(at, 2, 2),
                ProjectKind::Smokehouse => self.stamp_block(at, 3, 3, material),
                ProjectKind::Woodshed => {
                    // A roofed store: the woodpile now holds twenty logs.
                    if let Some(w) = self.projects.iter_mut().find(|p| p.kind == ProjectKind::Woodpile) { w.needed = 20; }
                }
                ProjectKind::Lookout => self.raise_tower(at, material),
                ProjectKind::Still => self.stamp_posts(at, 2, 2),
                ProjectKind::Traps => self.set_traps(),
                ProjectKind::LordsHall => self.raise_building_at(at, 5, 4, material == ItemKind::Stone, true),
                ProjectKind::CaveFarm => {}
                ProjectKind::Hatch => self.seal_caverns(),
                ProjectKind::Drawbridges => self.build_drawbridges(),
                ProjectKind::Tavern => self.raise_building_at(at, 5, 4, material == ItemKind::Stone, true),
                ProjectKind::Pen => { self.stamp_posts(at, 6, 5); self.fill_pen(); }
                ProjectKind::GuildHall => self.raise_building_at(at, 4, 3, material == ItemKind::Stone, true),
                ProjectKind::Kitchen => self.raise_building_at(at, 3, 3, material == ItemKind::Stone, true),
                ProjectKind::Library => self.raise_building_at(at, 4, 3, material == ItemKind::Stone, true),
                ProjectKind::Fence => self.stamp_posts(at, 4, 4),
                ProjectKind::Palisade => {}
                ProjectKind::Mending => {
                    // Mending fills every gap in the ring; a breach is closed in stone where
                    // there is stone to be had.
                    if let Some((p, d)) = self.breach.take() {
                        let stone = self.nearest(self.camp, |c, q| c.is_quarry_stone(q)).is_some();
                        let m = if stone { ItemKind::Stone } else { material };
                        self.raise_palisade(self.camp, 24, 24, m);
                        self.note(format!("They close the breach the raid of day {} made at {},{}{}.", d, p.0, p.1, if stone { ", in stone this time" } else { "" }));
                    } else {
                        self.raise_palisade(self.camp, 24, 24, material);
                    }
                }
                ProjectKind::Storehouse => self.raise_building_at(at, 4, 3, material == ItemKind::Stone, true),
                ProjectKind::Temple => self.raise_building_at(at, 4, 4, material == ItemKind::Stone, true),
                ProjectKind::Workshop => self.raise_building_at(at, 5, 4, material == ItemKind::Stone, false),
                ProjectKind::Field => { self.stamp_posts(at, 8, 6); self.sow_field(at); }
                ProjectKind::Jetty => { self.jetty = Some(at); }
                ProjectKind::Well => self.stamp_block(at, 1, 1, ItemKind::Stone),
                ProjectKind::DugHall | ProjectKind::Cellar | ProjectKind::Mine | ProjectKind::Bedrooms | ProjectKind::GreatHall | ProjectKind::Tombs | ProjectKind::Moat | ProjectKind::DeepShaft | ProjectKind::Workshops | ProjectKind::MasonShop | ProjectKind::CarpenterShop | ProjectKind::Smelter | ProjectKind::Forge | ProjectKind::Kiln | ProjectKind::StoneCut => {}
                ProjectKind::Lining => {
                    // The wet shaft is lined: the dig goes on, dry (`dig.rs`).
                    self.aquifer_lined = true;
                    self.dig_paused = false;
                    self.note("They have lined the wet rock with dressed stone, course by course, and the shaft goes on down, dry.".into());
                }
            }
            self.plan_line.clear();
            let day = self.clock.day();
            let started = self.projects[k].day;
            let took = day.saturating_sub(started).max(1);
            let work = if took == 1 { "a day's work".to_string() } else { format!("{} days' work", took) };
            self.note(format!("{} finishes {} ({}).", name, kind.word(), work));
            let why = self.projects[k].why.clone();
            if kind == ProjectKind::Mending { return; }
            self.moment(format!("{} finished", super::arc::capital_word(kind.word())), format!("{} finishes {} after {}.", name, kind.word(), work),
                if why.starts_with("for ") || why.starts_with("against ") { format!("because they built it {}", why) } else { format!("because {}", why) }, at);
        }
    }

    /// A low stack (one level) of the material over a w x h footprint.
    fn stamp_block(&mut self, at: Pos, w: u16, h: u16, m: ItemKind) {
        let mat = if m == ItemKind::Stone { Material::Rock(crate::erosion::materials::RockType::Granite) } else { Material::Wood };
        for dy in 0..h { for dx in 0..w { self.wall_at((at.0 + dx) as usize, (at.1 + dy) as usize, mat); } }
    }

    /// Corner posts (a rack).
    fn stamp_posts(&mut self, at: Pos, w: u16, h: u16) {
        for (dx, dy) in [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)] { self.wall_at((at.0 + dx) as usize, (at.1 + dy) as usize, Material::Wood); }
    }

    fn wall_at(&mut self, x: usize, y: usize, m: Material) {
        let n = self.map.width;
        if x >= n || y >= self.map.height { return; }
        let sz = self.map.surface_z[y * n + x] as usize;
        if sz + 1 >= self.map.depth { return; }
        let k = self.map.idx(x, y, sz);
        self.map.cells[k].plant = Plant::None;
        let kw = self.map.idx(x, y, sz + 1);
        if self.map.cells[kw].water > 0 { return; }
        self.map.cells[kw].shape = Shape::Wall;
        self.map.cells[kw].material = m;
        // Anyone standing there steps aside.
        for s in &mut self.settlers {
            if s.pos == (x as u16, y as u16) { s.pos = (x as u16, (y + 1).min(self.map.height - 1) as u16); s.path.clear(); }
        }
    }

    /// The palisade rises as loads come in: each load sets the next stretch of the ring, which
    /// leaves gates on the four sides.
    fn raise_palisade(&mut self, centre: Pos, used: u32, needed: u32, m: ItemKind) {
        let ring: Vec<(i32, i32)> = {
            let r = self.wall_r();
            let mut v = Vec::new();
            for k in 0..(8 * r) {
                let a = k as f32 / (8 * r) as f32 * std::f32::consts::TAU;
                let p = ((a.cos() * r as f32).round() as i32, (a.sin() * r as f32).round() as i32);
                if v.last() != Some(&p) { v.push(p); }
            }
            v.dedup();
            // Open where the gates are (`traps.rs`: `gates`).
            let gates: Vec<(i32, i32)> = self.gate_dirs().into_iter().map(|d| super::traps::gate_point(d, r)).collect();
            v.into_iter().filter(|&(x, y)| !gates.iter().any(|g| (x - g.0).pow(2) + (y - g.1).pow(2) <= 2)).collect()
        };
        let mat = if m == ItemKind::Stone { Material::Rock(crate::erosion::materials::RockType::Granite) } else { Material::Wood };
        // A load raises the next stretch; (needed, needed) with `used` past the end... a mending
        // (used == needed and the ring already up) refills every gap.
        let (from, to) = if self.projects.iter().any(|p| p.kind == ProjectKind::Palisade && p.done) { (0, ring.len()) }
            else { ((ring.len() as u32 * (used - 1) / needed) as usize, (ring.len() as u32 * used / needed) as usize) };
        for &(dx, dy) in &ring[from..to.min(ring.len())] {
            let (x, y) = (centre.0 as i32 + dx, centre.1 as i32 + dy);
            if x < 1 || y < 1 { continue; }
            // Not over the huts or their doors.
            let on_hut = |at: Pos| x >= at.0 as i32 - 1 && x <= at.0 as i32 + HUT_W as i32 && y >= at.1 as i32 - 1 && y <= at.1 as i32 + HUT_H as i32 + 1;
            if self.hut.as_ref().map_or(false, |h| on_hut(h.at)) || self.projects.iter().any(|p| p.kind == ProjectKind::SecondHut && on_hut(p.at)) { continue; }
            if super::nav::passable(&self.map, (x as u16, y as u16)) { self.wall_at(x as usize, y as usize, mat); }
        }
    }

    /// Whether `p` is inside a finished second hut.
    pub(crate) fn in_second_hut(&self, p: Pos) -> bool {
        self.projects.iter().any(|q| q.kind == ProjectKind::SecondHut && q.done
            && p.0 > q.at.0 && p.0 < q.at.0 + HUT_W as u16 - 1 && p.1 > q.at.1 && p.1 < q.at.1 + HUT_H as u16 - 1)
    }

    /// Settlers past the first hut's room sleep in the second, once it stands.
    pub(crate) fn second_hut_bed(&self, i: usize) -> Option<Pos> {
        let q = self.projects.iter().find(|q| q.kind == ProjectKind::SecondHut && q.done)?;
        if i < 6 { return None; }
        let j = (i - 6) as u16;
        Some((q.at.0 + 1 + j % (HUT_W as u16 - 2), q.at.1 + 1 + (j / (HUT_W as u16 - 2)) % (HUT_H as u16 - 2)))
    }
}

impl Colony {
    /// Thirty days with and without the woodpile (the patron forbids its ground the moment it is
    /// planned): how many fall ill from the cold. Returns (ill with, ill without).
    pub fn cold_trial(mut with: Colony, mut without: Colony) -> (usize, usize) {
        with.run_days(120);
        let end = without.clock.tick + 120 * super::TICKS_PER_DAY;
        let mut forbidden = false;
        while without.clock.tick < end {
            without.tick();
            if !forbidden {
                if let Some(p) = without.projects.iter().find(|p| p.kind == ProjectKind::Woodpile) {
                    let at = p.at;
                    without.patron.favour = without.patron.favour.max(1);
                    let _ = without.mark_place(at, 2, true);
                    forbidden = true;
                }
            }
        }
        // Settler-nights that end chilled (exposure 0.5 or more at dawn), counted day by day.
        (with.chilled_nights as usize, without.chilled_nights as usize)
    }
}

impl Colony {
    /// Turn a field's ground to crop rows (sown; they ripen by autumn).
    pub(crate) fn sow_field(&mut self, at: Pos) {
        let n = self.map.width;
        for dy in 1..5u16 { for dx in 1..7u16 {
            let (x, y) = ((at.0 + dx) as usize, (at.1 + dy) as usize);
            if x >= n || y >= self.map.height { continue; }
            let k = self.map.idx(x, y, self.map.surface_z[y * n + x].max(0) as usize);
            if self.map.cells[k].shape != Shape::Wall { self.map.cells[k].plant = Plant::Crop(0); }
        } }
    }

    /// At autumn's first dawn the field is reaped: a meal a crop row cell, left in the field for
    /// the carriers; in spring it is sown again.
    pub(crate) fn field_season(&mut self) {
        let Some(at) = self.projects.iter().find(|p| p.kind == ProjectKind::Field && p.done).map(|p| p.at) else { return };
        let into_year = (self.clock.day().max(1) - 1) % (4 * super::SEASON_DAYS);
        if into_year == 2 * super::SEASON_DAYS {
            let n = self.map.width;
            let mut reaped = 0;
            for dy in 1..5u16 { for dx in 1..7u16 {
                let (x, y) = ((at.0 + dx) as usize, (at.1 + dy) as usize);
                if x >= n || y >= self.map.height { continue; }
                let k = self.map.idx(x, y, self.map.surface_z[y * n + x].max(0) as usize);
                if matches!(self.map.cells[k].plant, Plant::Crop(_)) {
                    self.map.cells[k].plant = Plant::None;
                    // Seed grain from the caravan gives half again (`liaison.rs`).
                    let n = if self.seed_grain { 3 } else { 2 };
                    for _ in 0..n { self.items.push(super::Item { kind: ItemKind::Food, at: (x as u16, y as u16), stored: false, reserved: false }); }
                    reaped += n;
                }
            } }
            if reaped > 0 { self.note(format!("They reap the field: {} meals of grain to carry in before winter.", reaped)); }
        } else if into_year == 0 {
            self.sow_field(at);
            self.note("They sow the field again.".into());
        }
    }
}

impl Colony {
    /// A building's footprint (w, h), for kinds that have one.
    /// Whether `p` lies on or beside a building (standing or planned), the hut included.
    pub(crate) fn built_near(&self, p: Pos) -> bool {
        let (x, y) = (p.0 as i32, p.1 as i32);
        let in_rect = |at: Pos, rw: u16, rh: u16| x >= at.0 as i32 - 1 && x <= at.0 as i32 + rw as i32 && y >= at.1 as i32 - 1 && y <= at.1 as i32 + rh as i32;
        self.hut.as_ref().map_or(false, |h| in_rect(h.at, HUT_W as u16, HUT_H as u16))
            || self.projects.iter().any(|q| Self::footprint(q.kind).map_or(false, |(w, h)| in_rect(q.at, w, h)))
    }

    pub fn footprint(kind: ProjectKind) -> Option<(u16, u16)> {
        Some(match kind {
            ProjectKind::SecondHut => (HUT_W as u16, HUT_H as u16), ProjectKind::Storehouse => (4, 3), ProjectKind::Workshop | ProjectKind::LordsHall | ProjectKind::Tavern => (5, 4), ProjectKind::Temple => (4, 4), ProjectKind::GuildHall => (4, 3), ProjectKind::Kitchen => (3, 3), ProjectKind::Library => (4, 3),
            ProjectKind::Field => (8, 6), ProjectKind::Pen => (6, 5), ProjectKind::Smokehouse => (3, 3), ProjectKind::Woodpile => (3, 1), ProjectKind::Windbreak => (HUT_W as u16, 1),
            ProjectKind::DryingRack | ProjectKind::Lookout | ProjectKind::Still => (2, 2), ProjectKind::Fence => (4, 4), ProjectKind::Well | ProjectKind::Jetty => (1, 1),
            ProjectKind::Palisade | ProjectKind::Woodshed | ProjectKind::Mending | ProjectKind::DugHall | ProjectKind::Cellar | ProjectKind::Mine | ProjectKind::Bedrooms | ProjectKind::GreatHall | ProjectKind::Tombs | ProjectKind::Moat | ProjectKind::Workshops | ProjectKind::Hatch | ProjectKind::Drawbridges | ProjectKind::Lining | ProjectKind::Traps | ProjectKind::CaveFarm | ProjectKind::DeepShaft | ProjectKind::MasonShop | ProjectKind::CarpenterShop | ProjectKind::Smelter | ProjectKind::Forge | ProjectKind::Kiln | ProjectKind::StoneCut => return None,
        })
    }

    /// Buildings going up: (top-left, w, h, share of loads laid), the hut first.
    pub fn rising(&self) -> Vec<(Pos, u16, u16, f32)> {
        let mut v = Vec::new();
        if let Some(h) = self.hut.as_ref().filter(|h| !h.done && h.logs_used > 0) { v.push((h.at, HUT_W as u16, HUT_H as u16, h.logs_used as f32 / super::HUT_LOGS as f32)); }
        for q in self.projects.iter().filter(|q| !q.done && q.used > 0) {
            if let Some((w, h)) = Self::footprint(q.kind) { v.push((q.at, w, h, q.used as f32 / q.needed.max(1) as f32)); }
        }
        v
    }

    /// The building standing on `p`, said with why it was built: "A storehouse, built on day 15:
    /// 55 meals lay by the fire, and the rain got at them."
    pub fn building_at(&self, p: Pos) -> Option<String> {
        if let Some(e) = self.engraving_at(p) { return Some(e); }
        if let Some(h) = self.hut.as_ref().filter(|h| h.done) {
            if p.0 >= h.at.0 && p.0 < h.at.0 + HUT_W as u16 && p.1 >= h.at.1 && p.1 < h.at.1 + HUT_H as u16 { return Some("The hut, the camp's first roof".into()); }
        }
        for q in self.projects.iter().filter(|q| q.done) {
            let (w, h): (u16, u16) = match q.kind {
                ProjectKind::SecondHut => (HUT_W as u16, HUT_H as u16), ProjectKind::Storehouse => (4, 3), ProjectKind::Workshop | ProjectKind::LordsHall | ProjectKind::Tavern => (5, 4), ProjectKind::Temple => (4, 4), ProjectKind::GuildHall => (4, 3), ProjectKind::Kitchen => (3, 3), ProjectKind::Library => (4, 3),
                ProjectKind::Field => (8, 6), ProjectKind::Pen => (6, 5), ProjectKind::Smokehouse => (3, 3), ProjectKind::Woodpile => (3, 1), ProjectKind::Windbreak => (HUT_W as u16, 1),
                ProjectKind::DryingRack | ProjectKind::Lookout | ProjectKind::Still => (2, 2), ProjectKind::Fence => (4, 4), ProjectKind::Well | ProjectKind::Jetty => (1, 1),
                ProjectKind::Palisade | ProjectKind::Woodshed | ProjectKind::Mending | ProjectKind::DugHall | ProjectKind::Cellar | ProjectKind::Mine | ProjectKind::Bedrooms | ProjectKind::GreatHall | ProjectKind::Tombs | ProjectKind::Moat | ProjectKind::Workshops | ProjectKind::Hatch | ProjectKind::Drawbridges | ProjectKind::Lining | ProjectKind::Traps | ProjectKind::CaveFarm | ProjectKind::DeepShaft | ProjectKind::MasonShop | ProjectKind::CarpenterShop | ProjectKind::Smelter | ProjectKind::Forge | ProjectKind::Kiln | ProjectKind::StoneCut => continue,
            };
            let pad = if matches!(q.kind, ProjectKind::Well | ProjectKind::Jetty) { 1 } else { 0 };
            if p.0 + pad >= q.at.0 && p.0 < q.at.0 + w + pad && p.1 + pad >= q.at.1 && p.1 < q.at.1 + h + pad {
                let mut word = q.kind.word().to_string();
                if let Some(c) = word.get_mut(0..1) { c.make_ascii_uppercase(); }
                return Some(format!("{}, begun on day {}: {}", word, q.day, q.why));
            }
        }
        None
    }
}

impl Colony {
    /// The raid breaks the palisade near where it came in: the wall within three cells is torn
    /// down, and the breach waits to be mended (first among the works).
    pub(crate) fn break_palisade(&mut self, at: Pos) {
        if !self.projects.iter().any(|p| p.kind == ProjectKind::Palisade && p.done) { return; }
        let n = self.map.width as i32;
        let mut broken = 0;
        for dy in -4i32..=4 { for dx in -4i32..=4 {
            let (x, y) = (at.0 as i32 + dx, at.1 as i32 + dy);
            if x < 1 || y < 1 || x >= n - 1 || y >= self.map.height as i32 - 1 { continue; }
            let (xu, yu) = (x as usize, y as usize);
            // Only the palisade's ring (`wall_r` from the camp), not the huts.
            let r = ((x - self.camp.0 as i32).pow(2) + (y - self.camp.1 as i32).pow(2)) as f32;
            if (r.sqrt() - self.wall_r() as f32).abs() > 1.0 { continue; }
            let sz = self.map.surface_z[yu * self.map.width + xu] as usize;
            if sz + 1 >= self.map.depth { continue; }
            let k = self.map.idx(xu, yu, sz + 1);
            if self.map.cells[k].shape == Shape::Wall {
                self.map.cells[k].shape = Shape::Empty;
                self.map.cells[k].material = Material::Air;
                broken += 1;
            }
        } }
        if broken > 0 {
            self.breach = Some((at, self.clock.day()));
            self.note(format!("The palisade is broken where they came in, at {},{}.", at.0, at.1));
        }
    }
}

/// What lots are measured against (`purpose_cost`).
pub(crate) struct Near { zc: i32, tree: Option<Pos>, water: Option<Pos>, hut: Option<Pos>, herd: Option<Pos>, shrine: Option<Pos> }

impl Colony {
    fn near_things(&self) -> Near {
        let n = self.map.width;
        Near {
            zc: self.map.surface_z[self.camp.1 as usize * n + self.camp.0 as usize],
            tree: self.nearest_tree(self.camp),
            water: self.fishing_near(self.camp, 40),
            hut: self.hut.as_ref().map(|h| h.at),
            herd: self.creatures.iter().filter(|c| c.kind == super::creatures::CreatureKind::Game && c.z.is_none())
                .min_by_key(|c| ((c.pos.0 as i32 - self.camp.0 as i32).abs().max((c.pos.1 as i32 - self.camp.1 as i32).abs()), c.id)).map(|c| c.pos),
            shrine: self.stones.iter().find(|s| s.0 == super::StoneKind::Shrine).map(|s| s.1),
        }
    }

    /// The palisade's radius (9..13): a close, orderly people walls in a tight ring, an
    /// independent one a wide ring with room inside (`camp_spread`). The gates, the ditch (2
    /// outside), the cages and the drawbridges follow it.
    pub fn wall_r(&self) -> i32 {
        (13.0 - ((self.camp_spread() - 0.6) / 0.8 * 4.0)).round().clamp(9.0, 13.0) as i32
    }

    /// The camp's spread: the founders' gregariousness and orderliness (mean, 0..100) set how
    /// close the works keep to the fire (x1.4 for the most, x0.6 for the least).
    pub(crate) fn camp_spread(&self) -> f32 {
        let f: Vec<f32> = self.settlers.iter().take(7).map(|s| (s.persona.facet(crate::persona::Facet::Gregariousness) as f32 + s.persona.facet(crate::persona::Facet::Orderliness) as f32) / 2.0).collect();
        if f.is_empty() { return 1.0; }
        let m = f.iter().sum::<f32>() / f.len() as f32;
        // (Founders' means run about 50..65 on the dev seeds: centred there.)
        1.0 + 0.4 * ((m - 57.0) / 7.0).clamp(-1.0, 1.0)
    }

    /// The camp's lane: the unit direction from the fire toward the nearest water (the walk
    /// everyone makes), else toward the map's centre, else east.
    pub(crate) fn lane(&self) -> (f32, f32) {
        let n = self.map.width as i32;
        let (cx, cy) = (self.camp.0 as i32, self.camp.1 as i32);
        for r in 3..70i32 {
            for dy in -r..=r { for dx in -r..=r {
                if dx.abs().max(dy.abs()) != r { continue; }
                let (x, y) = (cx + dx, cy + dy);
                if x < 0 || y < 0 || x >= n || y >= self.map.height as i32 { continue; }
                let z = self.map.surface_z[y as usize * self.map.width + x as usize] + 1;
                if (z as usize) < self.map.depth && self.map.cell(x as usize, y as usize, z as usize).water > 0 {
                    let l = ((dx * dx + dy * dy) as f32).sqrt();
                    return (dx as f32 / l, dy as f32 / l);
                }
            } }
        }
        let (dx, dy) = ((n / 2 - cx) as f32, (self.map.height as i32 / 2 - cy) as f32);
        let l = (dx * dx + dy * dy).sqrt();
        if l > 1.0 { (dx / l, dy / l) } else { (1.0, 0.0) }
    }
}

/// How a people builds (`data/defaults/building_ways.json`).
#[derive(Clone, Debug, Default)]
pub struct BuildWay { pub people: String, pub first: Vec<String>, pub late: Vec<String>, pub stone_first: bool, pub keep_trees: bool, pub say: String }

const WAYS_JSON: &str = include_str!("../../data/defaults/building_ways.json");

/// The way a race builds (by its lower-case tag), from the data file.
pub fn build_way(race: &str) -> Option<BuildWay> {
    let all: serde_json::Value = serde_json::from_str(WAYS_JSON).ok()?;
    let w = all.get(race)?;
    let list = |k: &str| w.get(k).and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
    Some(BuildWay {
        people: race.to_string(), first: list("first"), late: list("late"),
        stone_first: w.get("stone_first").and_then(|v| v.as_bool()).unwrap_or(false),
        keep_trees: w.get("keep_trees").and_then(|v| v.as_bool()).unwrap_or(false),
        say: w.get("say").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
    })
}

/// Every people the data file names (for its test).
pub fn build_way_names() -> Vec<String> {
    serde_json::from_str::<serde_json::Value>(WAYS_JSON).ok().and_then(|v| v.as_object().map(|o| o.keys().filter(|k| !k.starts_with('_')).cloned().collect())).unwrap_or_default()
}

impl Colony {
    /// Adopt a people's way of building: said in the log; stone before timber if their way says so.
    pub fn adopt_way(&mut self, mut way: BuildWay) {
        if !way.say.is_empty() { self.note(way.say.clone()); }
        if way.stone_first {
            // Stone first only where there is stone to build with (40+ cells to quarry in reach).
            let n = self.map.width as i32;
            let (cx, cy) = (self.camp.0 as i32, self.camp.1 as i32);
            let mut stone = 0;
            for y in (cy - 40).max(1)..(cy + 40).min(self.map.height as i32 - 1) { for x in (cx - 40).max(1)..(cx + 40).min(n - 1) {
                if self.is_quarry_stone((x as u16, y as u16)) { stone += 1; }
            } }
            if stone >= 40 {
                if self.hut.as_ref().map_or(true, |h| h.logs_used == 0) { self.hut_material = ItemKind::Stone; }
            } else {
                way.stone_first = false;
                self.note(format!("But there is little stone here ({} cells to quarry), so they build in timber for now.", stone));
            }
        }
        self.way = Some(way);
    }
}

