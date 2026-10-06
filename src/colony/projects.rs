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
pub enum ProjectKind { Woodpile, DryingRack, SecondHut, Palisade, Windbreak, Smokehouse, Woodshed, Lookout, Fence, Mending }

impl ProjectKind {
    pub fn word(self) -> &'static str {
        match self {
            ProjectKind::Woodpile => "a woodpile", ProjectKind::DryingRack => "a drying rack", ProjectKind::SecondHut => "a second hut",
            ProjectKind::Palisade => "a palisade", ProjectKind::Windbreak => "a windbreak", ProjectKind::Smokehouse => "a smokehouse",
            ProjectKind::Woodshed => "a woodshed", ProjectKind::Lookout => "a lookout", ProjectKind::Fence => "a fence round the store",
            ProjectKind::Mending => "the mending of the palisade",
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
const PALISADE_R: i32 = 11;

impl Colony {
    /// The work under way: the hut until it stands, then the first unfinished project.
    pub(crate) fn active_project(&self) -> Option<usize> {
        // Nothing is built on ground the patron forbade.
        let ok = |p: &Project| !self.marked_at(p.at, true);
        // A woodpile running low is restocked before anything else.
        if let Some(k) = self.projects.iter().position(|p| p.done && p.kind == ProjectKind::Woodpile && p.used < 4 && ok(p)) { return Some(k); }
        self.projects.iter().position(|p| !p.done && ok(p))
            // A finished woodpile burns down each night and is restocked.
            .or_else(|| self.projects.iter().position(|p| p.done && p.kind == ProjectKind::Woodpile && p.used < p.needed && ok(p)))
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
        else if let Some(k) = self.active_project() { let p = &self.projects[k]; add(p.material, p.needed.saturating_sub(p.used)); }
        need
    }

    /// The material the work under way takes now.
    pub(crate) fn building_material(&self) -> Option<ItemKind> {
        if self.hut.as_ref().map_or(false, |h| !h.done) { return Some(self.hut_material); }
        self.active_project().map(|k| self.projects[k].material)
    }

    /// Whether a tree stands within working reach of the camp (else they build in stone).
    pub(crate) fn timber_near(&self) -> bool {
        self.nearest(self.camp, |c, p| c.is_felling_tree(p)).is_some()
    }

    /// At dawn, with nothing under way, the camp reckons its horizons (food against the days to
    /// winter, beds against heads, firewood against the cold, nights to a foretold raid against
    /// the wall) and the worst shortfall picks the next work, said aloud with its numbers. When
    /// nothing presses they improve what they have, so the list never ends.
    pub(crate) fn plan_projects(&mut self) {
        if self.hut.as_ref().map_or(true, |h| !h.done) || self.projects.iter().any(|p| !p.done && !self.marked_at(p.at, true)) { return; }
        let have = |c: &Colony, k: ProjectKind| c.projects.iter().any(|p| p.kind == k);
        let alive = self.alive();
        let material = if self.timber_near() { ItemKind::Log } else { ItemKind::Stone };
        let food = self.food_stored();
        let day = self.clock.day();
        let hut = self.hut.as_ref().unwrap().at;
        let days_of_food = food as f32 / (alive.max(1) as f32 * super::MEALS_NEEDED / 7.0);
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
            c.push((3.0 + 2.0 / (n as f32 + 1.0), ProjectKind::Palisade, format!("{} is foretold{}, and there is no wall", t, raid_in.map(|n| format!(" in {} nights", n)).unwrap_or_default()), 24, self.camp));
        }
        if alive > beds {
            if let Some(at) = self.find_site(HUT_W as u16, HUT_H as u16).filter(|_| !have(self, ProjectKind::SecondHut)) {
                c.push((2.0 + 0.3 * (alive - beds) as f32, ProjectKind::SecondHut, format!("there are {} of them and {} beds", alive, beds), 30, at));
            }
        }
        if let Some(d) = winter.filter(|&d| d <= 70 && !have(self, ProjectKind::Smokehouse)) {
            if let Some(at) = self.find_site(3, 3) {
                c.push((2.5 + (70 - d.min(70)) as f32 / 30.0, ProjectKind::Smokehouse, format!("winter is {} days off and the store holds {:.0} days of food, and nothing keeps", d, days_of_food), 16, at));
            }
        }
        if let Some(d) = winter.filter(|&d| d <= 50 && material == ItemKind::Log && have(self, ProjectKind::Woodpile) && !have(self, ProjectKind::Woodshed)) {
            c.push((1.9, ProjectKind::Woodshed, format!("winter is {} days off; the woodpile holds 8 logs and a winter's nights burn a hundred", d), 10, (hut.0 + HUT_W as u16 + 1, hut.1 + 2)));
        }
        if food >= 3 * alive as u32 && !have(self, ProjectKind::DryingRack) {
            if let Some(at) = self.find_site(2, 2) {
                c.push((1.5, ProjectKind::DryingRack, format!("{} meals are stored and spoil in about 6 days", food), 6, at));
            }
        }
        if !have(self, ProjectKind::Lookout) && (last_raid_killed.is_some() || arc.map_or(false, |a| a.chapter > 0)) {
            let why = match last_raid_killed { Some(d) => format!("the raid of day {} took one of them before the camp was awake", d), None => format!("{} troubles have come in {} days, and they want to see the next one first", arc.map_or(0, |a| a.chapter + 1), day) };
            c.push((if last_raid_killed.is_some() { 3.2 } else { 1.2 }, ProjectKind::Lookout, why, 8, self.watch_post()));
        }
        if have(self, ProjectKind::Smokehouse) && !have(self, ProjectKind::Fence) {
            if let Some(at) = self.find_site(4, 4) {
                c.push((1.0, ProjectKind::Fence, format!("the store holds {} meals, and animals come at night", food), 8, at));
            }
        }
        // The palisade weathers: mended every twenty days or so.
        let last_mend = self.projects.iter().filter(|p| matches!(p.kind, ProjectKind::Mending | ProjectKind::Palisade)).map(|p| p.day).max();
        if let Some(m) = last_mend.filter(|&m| day >= m + 20) {
            c.push((0.8 + if last_raid_killed.is_some() { 2.5 } else { 0.0 }, ProjectKind::Mending, format!("the palisade has stood {} days in the weather", day - m), 6, self.camp));
        }
        c.sort_by(|a, b| b.0.total_cmp(&a.0));
        let Some((_, kind, why, needed, at)) = c.into_iter().next() else { return };
        let material = if matches!(kind, ProjectKind::Woodpile | ProjectKind::Woodshed) { ItemKind::Log } else { material };
        if kind == ProjectKind::Woodpile && !self.timber_near() { return; }
        let stuff = if material == ItemKind::Stone { "stone" } else { "timber" };
        self.plan_line = format!("Next: {}. {}.", kind.word().trim_start_matches("the "), super::arc::capital_word(&why));
        self.note(format!("They set to work on {} of {}: {}.", kind.word(), stuff, why));
        self.projects.push(Project { kind, at, needed, used: 0, material, why, done: false, day });
    }

    /// A free, flat, open footprint of `w` x `h` near the camp (not on the hut or the camp).
    fn find_site(&self, w: u16, h: u16) -> Option<Pos> {
        let n = self.map.width as i32;
        let (cx, cy) = (self.camp.0 as i32, self.camp.1 as i32);
        let taken = |x: i32, y: i32| {
            let in_rect = |at: Pos, rw: u16, rh: u16| x >= at.0 as i32 - 1 && x <= at.0 as i32 + rw as i32 && y >= at.1 as i32 - 1 && y <= at.1 as i32 + rh as i32;
            self.hut.as_ref().map_or(false, |hh| in_rect(hh.at, HUT_W as u16, HUT_H as u16))
                || self.projects.iter().any(|p| p.kind != ProjectKind::Palisade && in_rect(p.at, HUT_W as u16, HUT_H as u16))
                || (x - cx).abs() <= 2 && (y - cy).abs() <= 2
        };
        let mut best: Option<(i32, Pos)> = None;
        for y in (cy - 10).max(2)..(cy + 10).min(n - h as i32 - 2) {
            for x in (cx - 10).max(2)..(cx + 10).min(n - w as i32 - 2) {
                let z0 = self.map.surface_z[(y * n + x) as usize];
                let ok = (0..h as i32).all(|dy| (0..w as i32).all(|dx| {
                    let (xx, yy) = (x + dx, y + dy);
                    !taken(xx, yy) && self.map.surface_z[(yy * n + xx) as usize] == z0 && super::nav::passable(&self.map, (xx as u16, yy as u16))
                }));
                if !ok { continue; }
                let d = (x - cx).abs() + (y - cy).abs();
                if best.map_or(true, |b| d < b.0) { best = Some((d, (x as u16, y as u16))); }
            }
        }
        best.map(|b| b.1)
    }

    /// Lay one load into the project under way; stamp it into the map as it rises.
    pub(crate) fn project_step(&mut self, i: usize) {
        let Some(k) = self.active_project() else { return };
        let material = self.projects[k].material;
        let Some(item) = self.items.iter().position(|it| it.kind == material && it.stored) else { return };
        self.items.remove(item);
        self.fix_item_refs(item);
        let name = self.settlers[i].name.clone();
        self.projects[k].used += 1;
        let (kind, used, needed, at) = (self.projects[k].kind, self.projects[k].used, self.projects[k].needed, self.projects[k].at);
        if kind == ProjectKind::Palisade { self.raise_palisade(at, used, needed, material); }
        if self.projects[k].done { return; }
        if used >= needed {
            self.projects[k].done = true;
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
                ProjectKind::Lookout => self.stamp_posts(at, 2, 2),
                ProjectKind::Fence => self.stamp_posts(at, 4, 4),
                ProjectKind::Palisade | ProjectKind::Mending => {}
            }
            self.plan_line.clear();
            let day = self.clock.day();
            let started = self.projects[k].day;
            self.note(format!("{} finishes {} ({} days' work).", name, kind.word(), day.saturating_sub(started).max(1)));
            let why = self.projects[k].why.clone();
            if kind == ProjectKind::Mending { return; }
            self.moment(format!("{} finished", super::arc::capital_word(kind.word())), format!("{} finishes {} after {} days' work.", name, kind.word(), day.saturating_sub(started).max(1)),
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
            let r = PALISADE_R;
            let mut v = Vec::new();
            for k in 0..(8 * r) {
                let a = k as f32 / (8 * r) as f32 * std::f32::consts::TAU;
                let p = ((a.cos() * r as f32).round() as i32, (a.sin() * r as f32).round() as i32);
                if v.last() != Some(&p) { v.push(p); }
            }
            v.dedup();
            v.into_iter().filter(|(x, y)| x.abs() > 1 && y.abs() > 1).collect()
        };
        let mat = if m == ItemKind::Stone { Material::Rock(crate::erosion::materials::RockType::Granite) } else { Material::Wood };
        let (from, to) = ((ring.len() as u32 * (used - 1) / needed) as usize, (ring.len() as u32 * used / needed) as usize);
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
        let _ = (&with, &without);
        (with.chilled_nights as usize, without.chilled_nights as usize)
    }
}

