//! Settlers go under: halls cut into the hillside, cellars and rooms off a stair.
//!
//! Digging works on the map's cells in three dimensions (DF): a cut at `(p, z)` works on the
//! cell over the floor at level `z`. A room cut opens that cell (the walker's body space) and
//! leaves the rock above as the roof, so the hill or field above stays ground to walk on; a stair
//! cut turns it into a stair (`Shape::Stair`), opening the level below to the one above, so the
//! camp goes down a stair spine (`delve.rs`) to rooms on several levels. Opening the surface's
//! own cell lowers the ground (an open cut). A hall is a passage straight into the nearest rise
//! and a room at its end; a cellar is a stair down from beside the camp and a room. Each cut is a
//! `Job::Dig` whose time is the rock's hardness (limestone quick, granite slow); the rock dug out
//! is a stone load carried up to the mouth for the next building, and an ore seam is noted.
//! Rooms under rock are shelter (warm, roofed, safe in a raid).

use super::{nav, Colony, Item, ItemKind, Pos};
use crate::erosion::materials::RockType;
use crate::local::{Material, Plant, Shape};

/// One cut of a dig: the cell over the floor at `(p, z)` opened (a room) or cut into a stair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DigCell { pub p: Pos, pub z: i32, pub stair: bool }

impl DigCell {
    pub fn room(p: Pos, z: i32) -> Self { DigCell { p, z, stair: false } }
    pub fn stair(p: Pos, z: i32) -> Self { DigCell { p, z, stair: true } }
}

/// Minutes a cell of this material takes to dig (before skill and tools).
pub fn dig_minutes(m: Material) -> u32 {
    match m {
        Material::Ore(_) => 260,
        Material::Rock(RockType::Limestone) => 120,
        Material::Rock(RockType::Shale) | Material::Rock(RockType::Sediment) => 110,
        Material::Rock(RockType::Sandstone) | Material::Rock(RockType::Ice) => 160,
        Material::Rock(RockType::Granite) => 260,
        Material::Rock(RockType::Basalt) => 300,
        Material::Soil | Material::Clay | Material::Sand | Material::Gravel | Material::Snow => 50,
        _ => 90,
    }
}

impl Colony {
    /// Whether the column at `p` holds a room of the camp's under the rock (any level).
    pub fn under_rock(&self, p: Pos) -> bool {
        self.hall_cells.contains(&p) || self.rooms.iter().any(|r| r.cells.contains(&p))
    }

    /// Whether a cut is made.
    pub fn cut_done(&self, c: &DigCell) -> bool {
        let z1 = c.z + 1;
        if z1 < 0 || z1 as usize >= self.map.depth { return true; }
        let cell = self.map.cell(c.p.0 as usize, c.p.1 as usize, z1 as usize);
        if c.stair { cell.shape == Shape::Stair } else { matches!(cell.shape, Shape::Empty | Shape::Stair) }
    }

    /// Make the cut at `(p, z)`: open the cell over the floor (a room: the floor below is
    /// smoothed, the rock above kept as the roof; the surface's own cell lowers the ground) or cut
    /// it into a stair. Returns what was dug out.
    pub(crate) fn dig_cell(&mut self, p: Pos, z: i32, stair: bool) -> Option<Material> {
        let (x, y) = (p.0 as usize, p.1 as usize);
        let n = self.map.width;
        let sz = self.map.surface_z[y * n + x];
        if z + 1 > sz || z < 1 || (z + 1) as usize >= self.map.depth { return None; }
        let head = self.map.idx(x, y, (z + 1) as usize);
        let removed = self.map.cells[head].material;
        self.map.cells[head].plant = Plant::None;
        self.map.cells[head].boulder = false;
        if stair {
            self.map.cells[head].shape = Shape::Stair;
            return Some(removed);
        }
        self.map.cells[head].shape = Shape::Empty;
        self.map.cells[head].material = Material::Air;
        let floor = self.map.idx(x, y, z as usize);
        if self.map.cells[floor].shape == Shape::Wall { self.map.cells[floor].shape = Shape::Floor; }
        self.map.cells[floor].plant = Plant::None;
        if !matches!(self.map.cells[floor].material, Material::Rock(_) | Material::Ore(_) | Material::Soil | Material::Clay | Material::Sand | Material::Gravel) {
            self.map.cells[floor].material = removed;
        }
        // The surface's own cell: the ground is cut down a level.
        if z + 1 == sz { self.map.surface_z[y * n + x] = z; }
        Some(removed)
    }
    /// A hall: from the camp toward the nearest rise of three levels or more within 30 cells, a
    /// passage of four cells straight in at the foot's level, then a 5x4 room. None on flat ground
    /// (meals are eaten in its larder when it is nearer than the fire: `eat_spot`).
    pub fn plan_hall(&self) -> Option<Vec<DigCell>> {
        let n = self.map.width as i32;
        let (cx, cy) = (self.camp.0 as i32, self.camp.1 as i32);
        let sz = |x: i32, y: i32| self.map.surface_z[y as usize * self.map.width + x as usize];
        // The foot: the last cell at about the camp's level before the ground rises three levels
        // above it (the slope may climb a level a cell).
        let zc = sz(cx, cy);
        let mut best: Option<(i32, (i32, i32), (i32, i32))> = None;
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let mut foot: Option<(i32, i32)> = None;
            for k in 1..30 {
                let (x, y) = (cx + dx * k, cy + dy * k);
                if x < 4 || y < 4 || x >= n - 4 || y >= self.map.height as i32 - 4 { break; }
                let z = sz(x, y);
                if z < zc - 2 { break; }
                if z <= zc + 1 { if nav::passable(&self.map, (x as u16, y as u16)) { foot = Some((x, y)); } continue; }
                if z >= zc + 3 {
                    if let Some(f) = foot { if best.map_or(true, |b| k < b.0) { best = Some((k, f, (dx, dy))); } }
                    break;
                }
            }
        }
        let (_, foot, (dx, dy)) = best?;
        let z0 = sz(foot.0, foot.1);
        let mut cells: Vec<DigCell> = Vec::new();
        let mut at = foot;
        for _ in 0..10 {
            at = (at.0 + dx, at.1 + dy);
            if at.0 < 3 || at.1 < 3 || at.0 >= n - 3 || at.1 >= self.map.height as i32 - 3 { return None; }
            if sz(at.0, at.1) > z0 { cells.push(DigCell::room((at.0 as u16, at.1 as u16), z0)); }
            if cells.len() >= 4 { break; }
        }
        // The room at the passage's end, across it.
        let (px, py) = (-dy, dx);
        for a in 1..=4 { for b in -2..=2 {
            let (x, y) = (at.0 + dx * a + px * b, at.1 + dy * a + py * b);
            if x < 3 || y < 3 || x >= n - 3 || y >= self.map.height as i32 - 3 { continue; }
            if sz(x, y) > z0 && !cells.iter().any(|c| c.p == (x as u16, y as u16)) { cells.push(DigCell::room((x as u16, y as u16), z0)); }
        } }
        if cells.iter().any(|c| self.built_near(c.p)) { return None; }
        (cells.len() >= 12).then_some(cells)
    }

    /// A cellar: a stair down three levels from beside the camp (out to ten cells from the fire,
    /// on clear ground), then a 4x3 room off its foot, under two levels of ground.
    pub(crate) fn plan_cellar(&self) -> Option<Vec<DigCell>> {
        let (cx, cy) = (self.camp.0 as i32, self.camp.1 as i32);
        let n = self.map.width as i32;
        let spots: Vec<(i32, i32)> = [(3, 3), (-3, 3), (3, -3), (-3, -3), (4, 0), (-4, 0)].into_iter()
            .chain((5..=10).flat_map(|r| [(r, 0), (-r, 0), (0, r), (0, -r), (r, r), (-r, r), (r, -r), (-r, -r)])).collect();
        for (sx, sy) in spots {
            let start = (cx + sx, cy + sy);
            if start.0 < 8 || start.1 < 8 || start.0 >= n - 8 || start.1 >= self.map.height as i32 - 8 { continue; }
            let s = (start.0 as u16, start.1 as u16);
            let z0 = self.map.surface_z[start.1 as usize * self.map.width + start.0 as usize];
            let zr = z0 - 3;
            if zr < 3 { continue; }
            let dir = if sx != 0 { (sx.signum(), 0) } else { (0, sy.signum()) };
            let mut cells: Vec<DigCell> = (1..=3).map(|k| DigCell::stair(s, z0 - k)).collect();
            for a in 1..=4 { for b in -1..=1 {
                let (x, y) = if dir.0 != 0 { (start.0 + dir.0 * a, start.1 + b) } else { (start.0 + b, start.1 + dir.1 * a) };
                cells.push(DigCell::room((x as u16, y as u16), zr));
            } }
            // Clear of buildings (a cellar dug inside the second hut cut its door off), solid
            // rock and ground to cut, and the way in reachable from the fire.
            // (Only the stair's head needs clear ground: the room lies under two levels of it.)
            if self.built_near(s) || !nav::passable(&self.map, s) || self.marked_at(s, true) { continue; }
            if !cells.iter().filter(|c| !c.stair).all(|c| self.solid_room_cell(c.p, zr)) { continue; }
            if nav::path(&self.map, self.camp, s, 4000).is_none() { continue; }
            return Some(cells);
        }
        None
    }

    /// Whether a room can be cut at `(p, z)`: inside the map, rock or ground for its floor, body
    /// and roof, under at least one more level of ground, out of the caverns.
    pub(crate) fn solid_room_cell(&self, p: Pos, z: i32) -> bool {
        let (x, y) = (p.0 as usize, p.1 as usize);
        if x < 3 || y < 3 || x + 3 >= self.map.width || y + 3 >= self.map.height || z < 2 { return false; }
        if self.map.surface_z[y * self.map.width + x] < z + 3 { return false; }
        // (The roof may be another room's floor: rooms stack a level apart.)
        (z..=z + 2).all(|zz| { let c = self.map.cell(x, y, zz as usize); (c.shape == Shape::Wall || (zz == z + 2 && c.shape == Shape::Floor)) && self.map.cavern_at(x, y, zz).is_none() && c.water == 0 })
    }

    /// The next cut of the dig under way that someone can reach: (column, level, where to stand).
    pub(crate) fn dig_target(&self) -> Option<(Pos, i32, nav::P3)> { self.dig_reach(true) }

    /// As `dig_target`, optionally counting cuts another digger has claimed (the dig is done only
    /// when nothing is left to reach, claimed or not).
    fn dig_reach(&self, skip_claimed: bool) -> Option<(Pos, i32, nav::P3)> {
        if self.dig_paused { return None; }
        let plan = self.dig_plan.as_ref()?;
        for c in plan {
            if self.cut_done(c) || (skip_claimed && self.claimed.contains(&c.p)) { continue; }
            if let Some(s) = self.cut_stand(c) { return Some((c.p, c.z, s)); }
        }
        None
    }

    /// Where to stand to make a cut: on the stair cell itself for a stair (digging down underfoot),
    /// else beside the cell on its level (or the level above), on ground known to be walked to:
    /// the surface, the spine, or a cut already made.
    pub(crate) fn cut_stand(&self, c: &DigCell) -> Option<nav::P3> {
        let w = self.map.width;
        // (A place carved the old way lowers its columns' ground; that floor is not ground known
        // to be walked: it may lie cut off beside the stair.)
        let in_place = |q: nav::P3| self.map.places.iter().any(|pl| pl.cells.iter().any(|c| c.0 == (q.0, q.1) && c.1 == q.2));
        let known = |q: nav::P3| {
            (self.map.surface_z[q.1 as usize * w + q.0 as usize] == q.2 && !in_place(q))
                || self.spine.as_ref().map_or(false, |s| s.at == (q.0, q.1) && q.2 >= s.bottom && q.2 <= s.top)
                || (self.dig_plan.iter().flatten().any(|d| d.p == (q.0, q.1) && d.z == q.2 && self.cut_done(d)) && !in_place(q))
                || self.rooms.iter().any(|r| r.z == q.2 && r.cells.contains(&(q.0, q.1)))
                || (self.hall_z == q.2 && self.hall_cells.contains(&(q.0, q.1)))
        };
        if c.stair {
            let s = (c.p.0, c.p.1, c.z + 1);
            return (nav::standable(&self.map, c.p.0 as usize, c.p.1 as usize, c.z + 1) && known(s)).then_some(s);
        }
        for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
            let q = (c.p.0 as i32 + dx, c.p.1 as i32 + dy);
            if q.0 < 0 || q.1 < 0 || q.0 as usize >= w || q.1 as usize >= self.map.height { continue; }
            for qz in [c.z, c.z + 1] {
                let s = (q.0 as u16, q.1 as u16, qz);
                // (From a level above only on open ground: cutting a gallery's first cell from the
                // stair a level up left the next one out of reach.)
                if qz == c.z + 1 && self.map.surface_z[q.1 as usize * w + q.0 as usize] != qz { continue; }
                if nav::standable(&self.map, q.0 as usize, q.1 as usize, qz) && known(s) { return Some(s); }
            }
        }
        None
    }

    /// Where to stand to make the cut at `(p, z)` of the dig under way.
    pub(crate) fn dig_stand3(&self, p: Pos, z: i32) -> Option<nav::P3> {
        let c = *self.dig_plan.as_ref()?.iter().find(|c| c.p == p && c.z == z && !self.cut_done(c))?;
        self.cut_stand(&c)
    }

    /// A cell dug: its rock to carry as a stone load (an ore seam noted), the dig's count.
    pub(crate) fn finish_dig(&mut self, i: usize, p: Pos, z: i32) {
        self.claimed.remove(&p);
        // Into a cavern: the breach (`mine.rs`). The cell is dug all the same.
        let (x, y) = (p.0 as usize, p.1 as usize);
        let layer = self.map.cavern_at(x, y, z).or_else(|| self.map.cavern_at(x, y, z + 1))
            .or_else(|| [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)].iter().find_map(|(dx, dy)| {
                let (qx, qy) = (x as i32 + dx, y as i32 + dy);
                if qx < 0 || qy < 0 || qx as usize >= self.map.width || qy as usize >= self.map.height { return None; }
                self.map.cavern_at(qx as usize, qy as usize, z + 1)
            }));
        // Wet rock (`LocalMap::aquifer`): the water comes in faster than they can dig.
        if !self.aquifer_lined && self.map.is_aquifer(x, y, (z + 1).max(0) as usize) {
            self.strike_aquifer(i, p, z);
            return;
        }
        // A cluster of gems in the rock (`local::gem_in`), looked at before the cell is cut.
        let gem = crate::local::gem_in(&self.map, x, y, (z + 1).max(0) as usize);
        let stair = self.dig_plan.as_ref().map_or(false, |plan| plan.iter().any(|c| c.p == p && c.z == z && c.stair));
        self.dig_fails = 0;
        let Some(m) = self.dig_cell(p, z, stair) else { return };
        if stair { if let Some(sp) = self.spine.as_mut().filter(|sp| sp.at == p) { sp.bottom = sp.bottom.min(z); } }
        // A stair cut into open dark (a cavern below): a stair is let down to its floor at once.
        if stair && z >= 1 && self.map.cell(p.0 as usize, p.1 as usize, z as usize).shape == Shape::Empty && self.map.cavern_at(p.0 as usize, p.1 as usize, z).is_some() {
            self.let_down_stair_pub(p, z);
        }
        let name = self.settlers[i].name.clone();
        if let Some(g) = gem {
            let n = 1 + (crate::history::settlers::hash_pub(self.seed ^ self.clock.tick, 0x6E3) % 3) as u32;
            match self.gems.iter_mut().find(|x| x.0 == g) { Some(x) => x.1 += n, None => self.gems.push((g.to_string(), n)) }
            let first = self.milestones.insert("first gems");
            let down = self.dig_depth(p, z);
            let line = format!("{} finds a cluster of {} in the rock, {} {} down{}.", name, g, down, if down == 1 { "level" } else { "levels" }, if n > 1 { format!(", and prises out {}", n) } else { String::new() });
            self.note(line.clone());
            if first { self.moment(format!("{} in the rock", super::arc::capital_word(g)), line, "because the rock they dug holds such stones".into(), p); }
            for j in 0..self.settlers.len() { if self.settlers[j].alive && self.settlers[j].persona.likes.material == g { self.feel(j, super::mind::Feel::LikedWork { material: g.to_string() }); } }
        }
        // (Debug: PLANET_FORCE_ORE=1 makes the first eight loads of rock dug an iron seam, for the
        // industries' test: no dev seed strikes ore.)
        let m = if matches!(m, Material::Rock(_)) && self.ore_found < 8 && std::env::var("PLANET_FORCE_ORE").is_ok() { Material::Ore(crate::history::civilizations::economy::ResourceType::Iron) } else { m };
        if let Material::Ore(r) = m {
            self.ore_found += 1;
            let kind = format!("{:?}", r).to_lowercase();
            // A load of ore for the smelter (`industry.rs`): only rock until it is smelted.
            self.add_ore(&kind, 1);
            self.once("ore", format!("{} strikes a seam of {:?} at {},{}, {} levels down.", name, r, p.0, p.1, self.dig_depth(p, z)));
        }
        // Clay and sand for the kiln.
        match m { Material::Clay => self.industry.clay += 1, Material::Sand => self.industry.sand += 1, _ => {} }
        if matches!(m, Material::Rock(_) | Material::Ore(_)) {
            // Carried up to the mouth from below; left where it fell on the surface.
            let at = if self.below(i) { self.delve_mouth.unwrap_or(self.camp) } else { self.settlers[i].pos };
            self.items.push(Item { kind: ItemKind::Stone, at, stored: false, reserved: false });
            self.stone_dug += 1;
        }
        if let Some(k) = self.projects.iter().position(|q| !q.done && super::projects::is_dig(q.kind)) {
            self.projects[k].used += 1;
            if self.projects[k].used >= self.projects[k].needed || self.dig_reach(false).is_none() {
                if std::env::var("PLANET_DEBUG_DIG").is_ok() { eprintln!("DIGDONE day {} {:?} used {} of {}; left {}", self.clock.day(), self.projects[k].kind, self.projects[k].used, self.projects[k].needed, self.dig_plan.as_ref().map_or(0, |pl| pl.iter().filter(|c| !self.cut_done(c)).count())); }
                self.projects[k].done = true;
                self.projects[k].used = self.projects[k].needed;
                let kind = self.projects[k].kind;
                self.dig_finished(kind);
                self.dig_plan = None;
                let why = self.projects[k].why.clone();
                // (A ditch is dug in the open, not under rock.)
                if kind == super::projects::ProjectKind::Moat { self.note(format!("{} throws up the last spadeful of {}: it stands dug, two levels deep.", name, kind.word())); }
                else { self.note(format!("{} breaks through the last of {}: it stands dug, under rock.", name, kind.word())); }
                let at = p;
                self.moment(format!("{} dug", super::arc::capital_word(kind.word())), format!("{} finishes {}{}.", name, kind.word(), if kind == super::projects::ProjectKind::Moat { "" } else { " in the rock" }), format!("because {}", why), at);
            }
        }
        self.once("dig", format!("{} breaks the first ground for the dig at {},{}.", name, p.0, p.1));
        if let Some(l) = layer { self.breach_cavern(i, l, p, z); }
    }

    /// Levels below the ground at `p` of a cut at level `z` (for the log).
    pub(crate) fn dig_depth(&self, p: Pos, z: i32) -> i32 {
        let (x, y) = (p.0 as usize, p.1 as usize);
        let top = self.delve_mouth.map(|m| self.map.surface_z[m.1 as usize * self.map.width + m.0 as usize]).unwrap_or(self.map.surface_z[y * self.map.width + x]);
        (top - z).max(1)
    }

    /// Metal tools (work 0.65x instead of the workshop's 0.8x, two more picks): forged from bars
    /// of iron or copper (`industry.rs`: ore, smelter, forge), or bought from a caravan.
    pub fn iron_worked(&self) -> bool {
        self.industry.tools.is_some() || self.tools_bought
    }

    /// Cells dug under rock (halls, cellars, rooms).
    pub fn dug_cells(&self) -> usize {
        self.hall_cells.len() + self.rooms.iter().filter(|r| r.kind != super::delve::RoomKind::Hall).map(|r| r.cells.len()).sum::<usize>()
    }

    /// The dig breaks into the aquifer: water wells up and the shaft can go no lower until it is
    /// lined with stone (`reckon_aquifer`: twelve loads for a people who build in stone, twenty
    /// for the rest). The camp has water at the shaft from then on (no more walks for it).
    fn strike_aquifer(&mut self, i: usize, p: Pos, z: i32) {
        let name = self.settlers[i].name.clone();
        let down = self.dig_depth(p, z);
        let kind = self.projects.iter().find(|q| !q.done && super::projects::is_dig(q.kind)).map(|q| q.kind);
        let what = kind.map(|k| k.word()).unwrap_or("the dig");
        let k = self.map.idx(p.0 as usize, p.1 as usize, z.max(0) as usize);
        self.map.cells[k].water = self.map.cells[k].water.max(3);
        let masons = self.way.as_ref().map_or(false, |w| w.stone_first);
        let line = format!("{}'s pick opens wet rock in {}: water wells up through the stone, cold and clear, faster than any bucket can lift it.", name, what);
        self.note(line.clone());
        let levels = if down == 1 { "1 level".to_string() } else { format!("{} levels", down) };
        let because = format!("because the rock {} down holds the water of the land (an aquifer)", levels);
        self.moment("Water in the rock".into(), line, because, p);
        self.aquifer_struck = Some(self.clock.day());
        self.water_walked = 0;
        self.dig_paused = true;
        let need = self.lining_need();
        self.note(format!("They stop the dig: the shaft must be lined with dressed stone, course by course, before it can go lower ({} loads{}).", need, if masons { "; they build in stone and know the way of it" } else { "; it is new work to them" }));
        // The lining is a work like any other, ahead of the paused dig (`projects.rs`).
        let at = self.projects.iter().position(|q| !q.done && super::projects::is_dig(q.kind)).unwrap_or(self.projects.len());
        let day = self.clock.day();
        self.projects.insert(at, super::projects::Project { kind: super::projects::ProjectKind::Lining, at: p, needed: need as u32, used: 0, material: ItemKind::Stone,
            why: format!("the shaft struck wet rock {} down and must be lined before it can go lower", levels), done: false, day });
    }

    /// Loads of stone to line a wet shaft: twelve for a people who build in stone, else twenty.
    fn lining_need(&self) -> usize { if self.way.as_ref().map_or(false, |w| w.stone_first) { 12 } else { 20 } }

}
