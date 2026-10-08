//! The delve: a stair spine down into the rock, with rooms on its levels (DF's fortress).
//!
//! The first dig makes the spine: a cellar's stair goes down from beside the camp, or a hall cut
//! into a hillside keeps its farthest cell for the stair to start from (`Spine`). Later digs go
//! on down it, two levels at a time, each making one level of rooms off the stair's foot, laid
//! out in four directions until one fits whole in solid rock out of the caverns, with a wall of
//! rock left between it and anything already dug (`plan_level`): bedrooms off a corridor
//! (`ProjectKind::Bedrooms`, a room of 2x2 for each with a door, `RoomKind::Bedroom` with its
//! bed), and a great hall where the camp eats together (`ProjectKind::GreatHall`, 7x5). The mine
//! is the same stair going on down after ore until it breaks into a cavern (`mine.rs`), where a
//! stair is let down to the cavern floor and the dark can be walked. At dawn (`reckon_rooms`)
//! bedrooms go to those who have none (a married pair share), each a thought of their own room
//! when they sleep there (`Feel::OwnRoom`). Rooms are kept in `Colony::rooms` with their level;
//! settlers stand and walk at their level (`Settler::z`, `nav::path3`), so a room under a field
//! keeps the field above to walk on.

use super::*;
use super::dig::DigCell;
use super::projects::ProjectKind;

/// What a dug room is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomKind { Hall, Cellar, Bedroom, GreatHall, Corridor, Tomb }

impl RoomKind {
    pub fn word(self) -> &'static str {
        match self {
            RoomKind::Hall => "the hall in the hill",
            RoomKind::Cellar => "the cellar",
            RoomKind::Bedroom => "a bedroom",
            RoomKind::GreatHall => "the great hall",
            RoomKind::Corridor => "a passage",
            RoomKind::Tomb => "a tomb",
        }
    }
}

/// A room cut in the rock, on one level.
#[derive(Clone, Debug)]
pub struct Room {
    pub kind: RoomKind,
    /// The floor level (walkers stand at `z`).
    pub z: i32,
    pub cells: Vec<Pos>,
    /// Whose bedroom it is.
    pub owner: Option<usize>,
    /// Where the bed stands (a bedroom), or the table (the great hall).
    pub bed: Option<Pos>,
    /// The day it was dug (set when its dig is done).
    pub day: u64,
    /// Its furniture made and set in place: a bedroom's bed, the great hall's table and benches
    /// (`furnish_option`), with what it was made of ("oak").
    pub furnished: Option<String>,
}

/// The stair spine: its column, the level it starts from (the surface or the hall's floor) and
/// the deepest level it reaches so far.
#[derive(Clone, Copy, Debug)]
pub struct Spine { pub at: Pos, pub top: i32, pub bottom: i32 }

/// A dig worked out before it starts: its cuts, the rooms it makes, the spine it begins (if it
/// begins one), and the mouth where the stone dug out is carried up to.
pub struct DelvePlan { pub cuts: Vec<DigCell>, pub rooms: Vec<Room>, pub spine: Option<Spine>, pub mouth: Pos }

fn room(kind: RoomKind, z: i32, cells: Vec<Pos>, bed: Option<Pos>) -> Room { Room { kind, z, cells, owner: None, bed, day: 0, furnished: None } }

impl Colony {
    /// The plan for a dig of this kind, if one can be made here now.
    pub(crate) fn plan_dig(&self, kind: ProjectKind) -> Option<DelvePlan> {
        let plan = self.plan_dig_inner(kind)?;
        // (Not a dig given up before: its way in was cut off.)
        (!plan.cuts.first().map_or(false, |c| self.digs_given_up.contains(&c.p))).then_some(plan)
    }

    /// A dig whose cuts no one can reach any more is given up (forty failed ways in a row): the
    /// work is struck off, nothing is registered, and the same dig is not planned again.
    pub(crate) fn give_up_dig(&mut self) {
        self.dig_fails = 0;
        let Some(first) = self.dig_plan.as_ref().and_then(|p| p.first().map(|c| c.p)) else { return };
        self.digs_given_up.push(first);
        let Some(k) = self.projects.iter().position(|q| !q.done && projects::is_dig(q.kind)) else { return };
        let kind = self.projects[k].kind;
        self.projects.remove(k);
        self.dig_plan = None;
        self.dig_rooms.clear();
        self.note(format!("They give up {}: no one can find a way to the rock they meant to cut.", kind.word()));
    }

    fn plan_dig_inner(&self, kind: ProjectKind) -> Option<DelvePlan> {
        match kind {
            ProjectKind::DugHall => {
                let cuts = self.plan_hall()?;
                let z0 = cuts[0].z;
                let first = cuts[0].p;
                let mouth = [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)].iter()
                    .map(|&(dx, dy)| ((first.0 as i32 + dx) as u16, (first.1 as i32 + dy) as u16))
                    .find(|&q| !cuts.iter().any(|c| c.p == q) && nav::passable(&self.map, q) && self.map.surface_z[q.1 as usize * self.map.width + q.0 as usize] == z0)?;
                let d = |q: Pos| (q.0 as i32 - mouth.0 as i32).abs().max((q.1 as i32 - mouth.1 as i32).abs());
                let far = cuts.iter().map(|c| c.p).max_by_key(|&q| (d(q), std::cmp::Reverse((q.1, q.0))))?;
                let cells: Vec<Pos> = cuts.iter().map(|c| c.p).collect();
                Some(DelvePlan { rooms: vec![room(RoomKind::Hall, z0, cells, None)], spine: Some(Spine { at: far, top: z0, bottom: z0 }), mouth, cuts })
            }
            ProjectKind::Cellar => {
                let cuts = self.plan_cellar()?;
                let s = cuts[0].p;
                let z0 = cuts[0].z + 1;
                let zr = cuts.iter().filter(|c| !c.stair).map(|c| c.z).next()?;
                let cells: Vec<Pos> = cuts.iter().filter(|c| !c.stair).map(|c| c.p).collect();
                Some(DelvePlan { rooms: vec![room(RoomKind::Cellar, zr, cells, None)], spine: Some(Spine { at: s, top: z0, bottom: z0 }), mouth: s, cuts })
            }
            ProjectKind::Mine => {
                let sp = self.spine?;
                let mouth = self.delve_mouth?;
                if self.marked_at(mouth, true) || self.marked_at(sp.at, true) { return None; }
                let cuts: Vec<DigCell> = (1..=35).map(|k| sp.bottom - k).take_while(|&z| z >= 2).map(|z| DigCell::stair(sp.at, z)).collect();
                if cuts.is_empty() { return None; }
                Some(DelvePlan { cuts, rooms: Vec::new(), spine: None, mouth })
            }
            ProjectKind::Bedrooms => {
                let want = (self.grown_without_rooms() as i32).clamp(2, 8);
                self.plan_level(kind, want)
            }
            ProjectKind::GreatHall | ProjectKind::Tombs => self.plan_level(kind, 0),
            ProjectKind::Moat => self.plan_moat(),
            // The deep shaft (`deep.rs`): the stair on down from its foot (the cavern floor) to
            // the deepest rock, through the caverns below (a stair is let down through each).
            ProjectKind::DeepShaft => {
                let sp = self.spine?;
                let mouth = self.delve_mouth?;
                let cuts: Vec<DigCell> = (4..sp.bottom).rev().map(|z| DigCell::stair(sp.at, z)).collect();
                (cuts.len() >= 8).then(|| DelvePlan { cuts, rooms: Vec::new(), spine: None, mouth })
            }
            _ => None,
        }
    }

    /// Settlers grown and living here who have no bedroom (a married pair share one).
    pub(crate) fn grown_without_rooms(&self) -> usize {
        let mut n: usize = 0;
        for i in 0..self.settlers.len() {
            let s = &self.settlers[i];
            if !s.alive || s.guest_until > 0 || s.past.as_ref().map_or(false, |p| p.age < 12) { continue; }
            if self.bedroom_of(i).is_some() { continue; }
            if let Some(sp) = s.spouse { if sp < i && self.settlers[sp].alive { continue; } }
            n += 1;
        }
        // (Bedrooms dug and not yet given out count as theirs.)
        n.saturating_sub(self.rooms.iter().filter(|r| r.kind == RoomKind::Bedroom && r.owner.is_none()).count())
    }

    /// Settler `i`'s bedroom (their own, or their spouse's).
    pub fn bedroom_of(&self, i: usize) -> Option<&Room> {
        let sp = self.settlers[i].spouse;
        self.rooms.iter().find(|r| r.kind == RoomKind::Bedroom && r.owner.is_some() && (r.owner == Some(i) || r.owner == sp))
    }

    /// One more level of rooms down the spine: the stair two (or three, four) levels on, then the
    /// rooms laid out off its foot in the first direction where every cell is solid rock with a
    /// wall left round it.
    fn plan_level(&self, kind: ProjectKind, beds: i32) -> Option<DelvePlan> {
        let sp = self.spine?;
        let mouth = self.delve_mouth?;
        let (sx, sy) = (sp.at.0 as i32, sp.at.1 as i32);
        // (a along the corridor, b across it, what the cell is for: 0 passage, 1+k bedroom k,
        // 100 the great hall.)
        let mut layout: Vec<(i32, i32, i32)> = Vec::new();
        let mut beds_at: Vec<(i32, i32, i32)> = Vec::new();
        if kind == ProjectKind::Tombs {
            // A passage with a niche each side every other cell (ten), rock between them.
            for a in 1..=9 { layout.push((a, 0, 0)); }
            for (j, a) in [1, 3, 5, 7, 9].iter().enumerate() {
                layout.push((*a, 1, 1 + 2 * j as i32));
                layout.push((*a, -1, 2 + 2 * j as i32));
            }
        } else if kind == ProjectKind::Bedrooms {
            let per_side = (beds + 1) / 2;
            for a in 1..=3 * per_side - 2 { layout.push((a, 0, 0)); }
            for k in 0..beds {
                let (j, side) = (k % per_side, if k < per_side { 1 } else { -1 });
                let a0 = 1 + 3 * j;
                layout.push((a0, side, 1 + k));
                for a in a0..=a0 + 1 { for b in [2, 3] { layout.push((a, side * b, 1 + k)); } }
                beds_at.push((a0 + 1, side * 3, 1 + k));
            }
        } else {
            for a in 1..=2 { layout.push((a, 0, 0)); }
            for a in 3..=9 { for b in -2..=2 { layout.push((a, b, 100)); } }
        }
        let dirs = [(1i32, 0i32), (0, 1), (-1, 0), (0, -1)];
        let turn = (crate::history::settlers::hash_pub(self.seed, sp.at.0 as u64 * 977 + sp.at.1 as u64) % 4) as usize;
        // The shallowest free level two or more below where the spine starts: on the stair
        // already cut, or two to four levels past its foot.
        for zb in (sp.bottom - 4..=sp.top - 2).rev() {
            if zb < 4 { return None; }
            // The stair's own cells still to cut: solid rock out of the caverns and the water.
            let shaft_ok = (zb..sp.bottom).all(|z| {
                let c = self.map.cell(sx as usize, sy as usize, z as usize);
                matches!(c.shape, crate::local::Shape::Wall | crate::local::Shape::Floor | crate::local::Shape::Stair) && self.map.cavern_at(sx as usize, sy as usize, z).is_none() && c.water == 0
            });
            if !shaft_ok { continue; }
            for t in 0..4 {
                let (dx, dy) = dirs[(t + turn) % 4];
                let (ex, ey) = (-dy, dx);
                let at = |a: i32, b: i32| ((sx + dx * a + ex * b) as u16, (sy + dy * a + ey * b) as u16);
                let cells: Vec<(Pos, i32)> = layout.iter().map(|&(a, b, tag)| (at(a, b), tag)).collect();
                if !cells.iter().all(|&(p, _)| self.solid_room_cell(p, zb)) { continue; }
                // A wall of rock left round it: nothing open on its level beside it but the stair.
                let mine: crate::history::det::HashSet<Pos> = cells.iter().map(|c| c.0).collect();
                let walled = cells.iter().all(|&(p, _)| (-1i32..=1).all(|ddy| (-1i32..=1).all(|ddx| {
                    let q = ((p.0 as i32 + ddx) as u16, (p.1 as i32 + ddy) as u16);
                    mine.contains(&q) || q == sp.at || self.map.cell(q.0 as usize, q.1 as usize, (zb + 1) as usize).shape == crate::local::Shape::Wall
                })));
                if !walled { continue; }
                let mut cuts: Vec<DigCell> = (zb..sp.bottom).rev().map(|z| DigCell::stair(sp.at, z)).collect();
                cuts.extend(cells.iter().map(|&(p, _)| DigCell::room(p, zb)));
                let mut rooms = vec![room(RoomKind::Corridor, zb, cells.iter().filter(|c| c.1 == 0).map(|c| c.0).collect(), None)];
                if kind == ProjectKind::Tombs {
                    for k in 1..=10 { rooms.push(room(RoomKind::Tomb, zb, cells.iter().filter(|c| c.1 == k).map(|c| c.0).collect(), None)); }
                } else if kind == ProjectKind::Bedrooms {
                    for k in 1..=beds {
                        let bed = beds_at.iter().find(|b| b.2 == k).map(|b| at(b.0, b.1));
                        rooms.push(room(RoomKind::Bedroom, zb, cells.iter().filter(|c| c.1 == k).map(|c| c.0).collect(), bed));
                    }
                } else {
                    rooms.push(room(RoomKind::GreatHall, zb, cells.iter().filter(|c| c.1 == 100).map(|c| c.0).collect(), Some(at(6, 0))));
                }
                return Some(DelvePlan { cuts, rooms, spine: None, mouth });
            }
        }
        None
    }

    /// A dig begins: its cuts become the plan, its rooms wait for it, and the first dig sets the
    /// spine and the mouth.
    pub(crate) fn begin_dig(&mut self, plan: DelvePlan) {
        self.dig_plan = Some(plan.cuts);
        self.dig_rooms = plan.rooms;
        if self.spine.is_none() { self.spine = plan.spine; }
        if self.delve_mouth.is_none() { self.delve_mouth = Some(plan.mouth); }
    }

    /// A dig is done: its rooms stand (the hall's cells are where the camp sleeps and eats).
    pub(crate) fn dig_finished(&mut self, kind: ProjectKind) {
        let day = self.clock.day();
        let rooms: Vec<Room> = self.dig_rooms.drain(..).collect();
        for mut r in rooms {
            r.day = day;
            if r.kind == RoomKind::Hall { self.hall_cells = r.cells.clone(); self.hall_z = r.z; }
            self.rooms.push(r);
        }
        if kind == ProjectKind::DeepShaft { self.deep_shaft_done(); }
    }

    /// The lookout raised as a tower (DF's constructions: walls, a floor and a stair built of
    /// blocks or timber): on its 2x2 lot, three cells walled two levels high with a floor laid
    /// over them, and the fourth a stair from the ground to the platform (its door). The night's
    /// watch stands on the platform (`watch_post`, `spot_level`).
    pub(crate) fn raise_tower(&mut self, at: Pos, material: ItemKind) {
        use crate::local::Shape;
        let n = self.map.width;
        if at.0 as usize + 2 >= n || at.1 as usize + 2 >= self.map.height { return; }
        let mat = if material == ItemKind::Stone { crate::local::Material::Block(self.land_rock()) } else { crate::local::Material::Wood };
        let sz = (0..2).flat_map(|dy| (0..2).map(move |dx| (dx, dy))).map(|(dx, dy)| self.map.surface_z[(at.1 as usize + dy) * n + at.0 as usize + dx]).max().unwrap_or(0);
        if sz as usize + 5 >= self.map.depth { return; }
        // The stair: the lot's corner nearest the camp.
        let stair = (0..2u16).flat_map(|dy| (0..2u16).map(move |dx| (at.0 + dx, at.1 + dy)))
            .min_by_key(|q| ((q.0 as i32 - self.camp.0 as i32).abs() + (q.1 as i32 - self.camp.1 as i32).abs(), q.1, q.0)).unwrap();
        for dy in 0..2u16 { for dx in 0..2u16 {
            let (x, y) = ((at.0 + dx) as usize, (at.1 + dy) as usize);
            let base = self.map.surface_z[y * n + x];
            let g = self.map.idx(x, y, base as usize);
            self.map.cells[g].plant = crate::local::Plant::None;
            for z in base + 1..=sz + 3 {
                let k = self.map.idx(x, y, z as usize);
                if (x as u16, y as u16) == stair {
                    self.map.cells[k].shape = Shape::Stair;
                } else {
                    self.map.cells[k].shape = if z == sz + 3 { Shape::Floor } else { Shape::Wall };
                }
                self.map.cells[k].material = mat;
                self.map.cells[k].water = 0;
            }
        } }
        // Anyone inside the walls steps out.
        for s in &mut self.settlers {
            if s.pos.0 >= at.0 && s.pos.0 < at.0 + 2 && s.pos.1 >= at.1 && s.pos.1 < at.1 + 2 && s.pos != stair { s.pos = (at.0, at.1 + 2); s.path.clear(); }
        }
        self.tower = Some((stair, sz + 3));
        self.note(format!("The lookout stands three levels high, {}, with a stair inside: from its platform the watch sees over the wall.", if material == ItemKind::Stone { "of dressed stone" } else { "of timber" }));
    }

    /// The land's rock, for dressed blocks.
    fn land_rock(&self) -> crate::erosion::materials::RockType {
        let sz = self.map.surface_z[self.camp.1 as usize * self.map.width + self.camp.0 as usize];
        (0..sz).rev().find_map(|z| match self.map.cell(self.camp.0 as usize, self.camp.1 as usize, z as usize).material { crate::local::Material::Rock(r) => Some(r), _ => None })
            .unwrap_or(crate::erosion::materials::RockType::Granite)
    }

    /// The level of a breached cavern's floor in column `p` (the first breached layer that has
    /// floor there).
    pub fn cavern_floor_at(&self, p: Pos) -> Option<i32> {
        let k = p.1 as usize * self.map.width + p.0 as usize;
        let cz = self.map.cavern_z.get(k)?;
        self.breached.iter().find_map(|&l| { let f = cz[l as usize].0; (f >= 0).then_some(f as i32) })
    }

    /// Whether the stair has opened a cavern to walk in (its floor's level under the stair's
    /// foot, or beside it).
    pub fn cavern_level(&self) -> Option<i32> {
        if self.breached.is_empty() { return None; }
        let sp = self.spine?;
        (-1i32..=1).flat_map(|dy| (-1i32..=1).map(move |dx| (dx, dy)))
            .find_map(|(dx, dy)| self.cavern_floor_at(((sp.at.0 as i32 + dx) as u16, (sp.at.1 as i32 + dy) as u16)))
    }

    /// Whether `p` holds a fungus tree on a breached cavern's floor (its level).
    pub(crate) fn cavern_tree_level(&self, p: Pos) -> Option<i32> {
        let f = self.cavern_floor_at(p)?;
        let c = self.map.cell(p.0 as usize, p.1 as usize, f as usize);
        (c.plant == crate::local::Plant::Tree(crate::local::TreeKind::Fungus)).then_some(f)
    }

    /// The nearest fungus tree on the cavern floor to the stair's foot (within 30 cells), not
    /// claimed or given up on, with ground to stand beside it.
    pub(crate) fn cavern_tree(&self) -> Option<Pos> {
        self.cavern_level()?;
        let sp = self.spine?;
        let (w, h) = (self.map.width as i32, self.map.height as i32);
        for r in 1..=30i32 {
            for dy in -r..=r { for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r { continue; }
                let (x, y) = (sp.at.0 as i32 + dx, sp.at.1 as i32 + dy);
                if x < 1 || y < 1 || x >= w - 1 || y >= h - 1 { continue; }
                let p = (x as u16, y as u16);
                // (Not under a tree on the surface: a column's felling means its surface tree.)
                if matches!(self.floor_plant_pub(p), crate::local::Plant::Tree(_)) { continue; }
                let Some(f) = self.cavern_tree_level(p) else { continue };
                if self.claimed.contains(&p) || self.unreachable.contains(&p) || self.cavern_stand(p, f).is_none() { continue; }
                return Some(p);
            } }
        }
        None
    }

    /// Where to stand beside a cavern tree.
    pub(crate) fn cavern_stand(&self, p: Pos, f: i32) -> Option<Pos> {
        [(0i32, 1i32), (1, 0), (-1, 0), (0, -1)].iter().map(|&(dx, dy)| ((p.0 as i32 + dx) as u16, (p.1 as i32 + dy) as u16))
            .find(|&q| nav::standable(&self.map, q.0 as usize, q.1 as usize, f) && self.map.cell(q.0 as usize, q.1 as usize, f as usize).plant != crate::local::Plant::Tree(crate::local::TreeKind::Fungus))
    }

    /// A fungus tree felled in the cavern: two logs of its pale wood to carry up the stair; the
    /// cavern's hunters may find the feller in the dark (one time in eight).
    pub(crate) fn fell_cavern_tree(&mut self, i: usize, t: Pos, f: i32) -> bool {
        let k = self.map.idx(t.0 as usize, t.1 as usize, f as usize);
        if self.map.cells[k].plant != crate::local::Plant::Tree(crate::local::TreeKind::Fungus) { return false; }
        self.map.cells[k].plant = crate::local::Plant::None;
        let at = self.delve_mouth.unwrap_or(self.camp);
        for _ in 0..2 { self.items.push(Item { kind: ItemKind::Log, at, stored: false, reserved: false }); }
        let name = self.settlers[i].name.clone();
        let cavern = self.map.caverns.iter().find(|c| Some(c.layer as usize) == self.map.cavern_at(t.0 as usize, t.1 as usize, f + 1)).map(|c| c.name.clone()).unwrap_or_else(|| "the cavern".into());
        let first = self.milestones.insert("cavern wood");
        if first {
            let line = format!("{} fells a fungus tree in {}, its cap as wide as a hut: two logs of pale, light wood to haul up the stair.", name, cavern);
            self.note(line.clone());
            self.moment("Wood from the dark".into(), line, "because the stair reaches the cavern's floor, and timber is short above".into(), t);
        }
        if let Some(h) = self.cave_hunter.clone() {
            if crate::history::settlers::hash_pub(self.seed ^ self.clock.tick, 0xCA7E) % 8 == 0 {
                self.settlers[i].ill_until = self.clock.tick + TICKS_PER_DAY;
                self.note(format!("{} is set upon by {} while felling in the dark of {}, and comes up the stair bleeding.", name, h.trim_end_matches('s'), cavern));
                self.feel(i, mind::Feel::TheDeep { what: format!("the {} in the dark", h) });
            }
        }
        true
    }

    /// The next room to furnish: the great hall's table first (once ten live here), then the beds
    /// of bedrooms with an owner, eldest room first.
    fn unfurnished(&self) -> Option<usize> {
        self.rooms.iter().position(|r| r.kind == RoomKind::GreatHall && r.furnished.is_none())
            .or_else(|| self.rooms.iter().position(|r| r.kind == RoomKind::Bedroom && r.furnished.is_none() && r.owner.is_some()))
    }

    /// Furniture at the workshop (DF's beds, tables and chairs, which make a dug space a room): a
    /// bed (one log) for an owned bedroom, the great hall's table and benches (three logs). One
    /// maker at a time, by day, a hand who likes making things or the camp's builder.
    pub(crate) fn furnish_option(&self, i: usize) -> Option<(f32, Job, String)> {
        if self.clock.is_night() || self.workshop_spot().is_none() { return None; }
        if self.settlers.iter().enumerate().any(|(j, s)| j != i && s.alive && s.job == Job::Craft && s.why.starts_with("Making furniture")) { return None; }
        let k = self.unfurnished()?;
        let need = if self.rooms[k].kind == RoomKind::GreatHall { 3 } else { 1 };
        self.furniture_stuff(need)?;
        let builder = self.settlers[i].role == Some(4);
        let wish = self.craft_wish_any(i).max(if builder { 0.7 } else { 0.0 });
        if wish < 0.3 { return None; }
        let what = match self.rooms[k].kind {
            RoomKind::GreatHall => "the long table and benches for the great hall".to_string(),
            _ => format!("a bed for {}'s room", self.rooms[k].owner.map(|o| self.settlers[o].name.clone()).unwrap_or_default()),
        };
        Some((wish * 0.95, Job::Craft, format!("Making furniture: {} at the workshop", what)))
    }

    /// What furniture is made of: logs stored (the land's wood), else dressed stone.
    fn furniture_stuff(&self, need: usize) -> Option<ItemKind> {
        let n = |kind: ItemKind| self.items.iter().filter(|it| it.stored && it.kind == kind).count();
        if n(ItemKind::Log) >= need { Some(ItemKind::Log) } else if n(ItemKind::Stone) >= need { Some(ItemKind::Stone) } else { None }
    }

    /// The furniture is made and carried down to its room.
    pub(crate) fn finish_furniture(&mut self, i: usize) {
        let Some(k) = self.unfurnished() else { return };
        let need = if self.rooms[k].kind == RoomKind::GreatHall { 3 } else { 1 };
        let Some(stuff) = self.furniture_stuff(need) else { return };
        for _ in 0..need {
            let Some(it) = self.items.iter().position(|it| it.stored && it.kind == stuff) else { return };
            self.items.remove(it);
            self.fix_refs_pub(it);
        }
        let wood = if stuff == ItemKind::Log { self.land_wood() } else { format!("dressed {}", self.land_stone()) };
        self.rooms[k].furnished = Some(wood.clone());
        let name = self.settlers[i].name.clone();
        match self.rooms[k].kind {
            RoomKind::GreatHall => {
                let line = format!("{} makes a long table of {} and two benches, and they carry them down the stair: the great hall can seat the camp at one table.", name, wood);
                self.note(line.clone());
                let at = self.rooms[k].bed.unwrap_or(self.camp);
                self.moment("The great hall's table".into(), line, "because a dug hall is a cave until it has a table in it".into(), at);
            }
            _ => {
                let owner = self.rooms[k].owner;
                if let Some(o) = owner { if o != i { self.like(o, i, 2); } }
                if self.milestones.insert("first bed") {
                    let who = match owner { Some(o) if o == i => format!("{} own", if self.settlers[i].persona.female { "her" } else { "his" }), Some(o) => format!("{}'s", self.settlers[o].name), None => "a".into() };
                    self.note(format!("{} makes a bed of {} for {} room below: the first room in the rock with a bed in it.", name, wood, who));
                }
            }
        }
    }

    /// A death laid in a niche of the tombs below (DF's coffins and tombs): no grave on the
    /// surface, and the dead rest (no ghost walks for want of a slab, nothing rises).
    pub(crate) fn entomb(&mut self, i: usize, k: usize, cause: &str) {
        self.rooms[k].owner = Some(i);
        if let Some(r) = self.restless.iter_mut().find(|r| r.who == i) { r.at_rest = true; }
        let name = self.settlers[i].name.clone();
        let z = self.rooms[k].z;
        let rock = self.rock_word_at(z);
        let at = self.rooms[k].cells.first().copied().unwrap_or(self.camp);
        let down = self.dig_depth(at, z);
        self.note(format!("They carry {} down the stair and lay them in a niche of the tombs, cut in the {} {} levels down.", name, rock, down));
        self.mourn_death(i, cause, "They lay them in the tombs below.", at);
    }

    /// Who lies in the tombs.
    pub fn entombed(&self) -> Vec<usize> { self.rooms.iter().filter(|r| r.kind == RoomKind::Tomb).filter_map(|r| r.owner).collect() }

    /// Fishing places on breached cavern floors within 60 cells of the stair: floor beside the
    /// cavern's water (its lakes and pools), found once the cavern is open.
    pub(crate) fn find_cave_fishing(&mut self) {
        let Some(sp) = self.spine else { return };
        if self.cavern_level().is_none() { return; }
        let (w, h) = (self.map.width as i32, self.map.height as i32);
        let mut out = Vec::new();
        for dy in -60i32..=60 { for dx in -60i32..=60 {
            let (x, y) = (sp.at.0 as i32 + dx, sp.at.1 as i32 + dy);
            if x < 1 || y < 1 || x >= w - 1 || y >= h - 1 { continue; }
            let p = (x as u16, y as u16);
            let Some(f) = self.cavern_floor_at(p) else { continue };
            if !nav::standable(&self.map, x as usize, y as usize, f) || self.map.cell(x as usize, y as usize, (f + 1) as usize).water > 0 { continue; }
            // (Water over the neighbour's own floor: cavern floors are uneven.)
            let wet = [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)].iter().any(|(ox, oy)| {
                let (qx, qy) = ((x + ox) as usize, (y + oy) as usize);
                let qf = self.cavern_floor_at((qx as u16, qy as u16)).unwrap_or(f);
                (qf + 1..=qf + 2).any(|zz| zz >= 0 && (zz as usize) < self.map.depth && self.map.cell(qx, qy, zz as usize).water > 0)
            });
            if wet { out.push((p, f)); }
        } }
        self.cave_fish = out;
    }

    /// The nearest cavern fishing place to the stair not claimed, fished out or given up on:
    /// (place, levels below the mouth, the cavern's name).
    pub(crate) fn cave_fishing_spot(&self) -> Option<(Pos, i32, String)> {
        let sp = self.spine?;
        let day = self.clock.day();
        let &(p, f) = self.cave_fish.iter()
            .filter(|(p, _)| !self.claimed.contains(p) && !self.unreachable.contains(p) && self.shrub_ready.get(p).map_or(true, |&d| d <= day))
            .min_by_key(|(p, _)| ((p.0 as i32 - sp.at.0 as i32).abs().max((p.1 as i32 - sp.at.1 as i32).abs()), p.1, p.0))?;
        let name = self.map.caverns.iter().find(|c| Some(c.layer as usize) == self.map.cavern_at(p.0 as usize, p.1 as usize, f + 1)).map(|c| c.name.clone()).unwrap_or_else(|| "the cavern".into());
        Some((p, self.dig_depth(p, f), name))
    }

    /// The living settler under the mouse at `(hx, hy)` (cells) as a view shows them: in the
    /// surface view those not below, in a level view (`level`) those standing on it.
    pub fn settler_at(&self, hx: f32, hy: f32, reach: f32, level: Option<i32>) -> Option<usize> {
        (0..self.settlers.len()).find(|&i| {
            let s = &self.settlers[i];
            s.alive && (s.pos.0 as f32 + 0.5 - hx).abs() < reach && (s.pos.1 as f32 + 0.5 - hy).abs() < reach
                && match level { None => !self.below(i), Some(z) => self.here3(i).2 == z || (!self.below(i) && (self.here3(i).2 - z).abs() <= 2) }
        })
    }

    /// A ditch round the wall (DF's moats and channels): the ring two cells outside the palisade
    /// cut down two levels (each column's surface cell, then the one under it), crossings left
    /// at the four gates. What walks the surface cannot climb two levels, so raiders and beasts
    /// come in by the gates, where the cage traps stand.
    fn plan_moat(&self) -> Option<DelvePlan> {
        let r = 13i32;
        let (cx, cy) = (self.camp.0 as i32, self.camp.1 as i32);
        let w = self.map.width;
        let mut ring: Vec<Pos> = Vec::new();
        for dy in -r..=r { for dx in -r..=r {
            if dx.abs() != r && dy.abs() != r { continue; }
            // The crossings: three cells at each gate's axis.
            if dx.abs() <= 1 || dy.abs() <= 1 { continue; }
            let (x, y) = (cx + dx, cy + dy);
            if x < 4 || y < 4 || x as usize + 4 >= w || y as usize + 4 >= self.map.height { return None; }
            let p = (x as u16, y as u16);
            let sz = self.map.surface_z[y as usize * w + x as usize];
            if sz < 5 || self.built_near(p) || self.marked_at(p, true) { continue; }
            let wet = (sz - 1..=sz + 1).any(|z| self.map.cell(x as usize, y as usize, z as usize).water > 0);
            let rock = (sz - 2..=sz).all(|z| matches!(self.map.cell(x as usize, y as usize, z as usize).shape, crate::local::Shape::Wall | crate::local::Shape::Floor | crate::local::Shape::Ramp));
            if !wet && rock && self.map.cavern_at(x as usize, y as usize, sz - 1).is_none() { ring.push(p); }
        } }
        if ring.len() < 60 { return None; }
        let sz = |p: Pos| self.map.surface_z[p.1 as usize * w + p.0 as usize];
        let mut cuts: Vec<DigCell> = ring.iter().map(|&p| DigCell::room(p, sz(p) - 1)).collect();
        // (The cells beside each crossing stay a level deep: steps out of the ditch at the gates.)
        let step = |p: Pos| { let (dx, dy) = ((p.0 as i32 - cx).abs(), (p.1 as i32 - cy).abs()); (dx == 2 && dy == r) || (dy == 2 && dx == r) };
        cuts.extend(ring.iter().filter(|&&p| !step(p)).map(|&p| DigCell::room(p, sz(p) - 2)));
        Some(DelvePlan { cuts, rooms: Vec::new(), spine: None, mouth: self.camp })
    }

    /// Whether the ditch round the wall is dug.
    pub fn moat_dug(&self) -> bool { self.projects.iter().any(|p| p.done && p.kind == ProjectKind::Moat) }

    /// Picks to dig with (DF's miners each need one): two brought, two more made at the workshop,
    /// two more of metal once ore is worked or iron bought.
    pub fn picks(&self) -> usize {
        2 + if self.projects.iter().any(|p| p.done && p.kind == ProjectKind::Workshop) { 2 } else { 0 } + if self.iron_worked() { 2 } else { 0 }
    }

    /// Where a meal is eaten in the great hall (its table), if there is one.
    pub(crate) fn great_hall(&self) -> Option<(Pos, i32)> {
        self.rooms.iter().find(|r| r.kind == RoomKind::GreatHall && r.furnished.is_some()).map(|r| (r.bed.unwrap_or(r.cells[0]), r.z))
    }

    /// The level of a place a settler goes to: a room's for sleeping, eating and work at a room,
    /// a fellow settler's when walking to where one stands, else the surface's.
    pub(crate) fn spot_level(&self, i: usize, job: Job, why: &str, target: Pos) -> nav::P3 {
        if let Job::Dig(p, z) = job { if let Some(s) = self.dig_stand3(p, z) { return s; } }
        let in_room = |kinds: &[RoomKind]| self.rooms.iter().filter(|r| kinds.contains(&r.kind)).find(|r| r.cells.contains(&target) && nav::standable(&self.map, target.0 as usize, target.1 as usize, r.z)).map(|r| r.z);
        let z = match job {
            Job::Sleep => if self.bedroom_of(i).map_or(false, |r| r.bed == Some(target)) { self.bedroom_of(i).map(|r| r.z) } else { in_room(&[RoomKind::Hall, RoomKind::Cellar]) },
            Job::Eat => in_room(&[RoomKind::GreatHall, RoomKind::Hall, RoomKind::Cellar]),
            Job::Craft => in_room(&[RoomKind::Hall, RoomKind::GreatHall]),
            // A fishing place on a cavern's floor.
            Job::Fish(t) => self.cave_fish.iter().find(|c| c.0 == t).map(|c| c.1),
            // A fungus tree on the cavern floor: stand beside it down there.
            Job::Fell(t) if self.floor_plant_pub(t) == crate::local::Plant::None => self.cavern_tree_level(t),
            // The watch climbs to the lookout's platform.
            Job::Wander(t) if why.starts_with("Keeping watch") && self.tower.map_or(false, |(p, _)| p == t) => self.tower.map(|(_, z)| z),
            Job::Wander(_) => self.settlers.iter().enumerate()
                .find(|(j, s)| *j != i && s.alive && s.pos == target && !why.starts_with("Playing"))
                .map(|(j, _)| self.here3(j).2),
            _ => None,
        };
        match z {
            Some(z) => (target.0, target.1, z),
            None => nav::surface3(&self.map, target),
        }
    }

    /// Dawn: bedrooms go to those who have none (the eldest first; a married pair share).
    pub(crate) fn reckon_rooms(&mut self) {
        // A dead or departed owner's room is free again.
        for k in 0..self.rooms.len() {
            if let Some(o) = self.rooms[k].owner { if !self.settlers[o].alive { self.rooms[k].owner = None; } }
        }
        let mut want: Vec<usize> = (0..self.settlers.len()).filter(|&i| {
            let s = &self.settlers[i];
            s.alive && s.guest_until == 0 && s.past.as_ref().map_or(true, |p| p.age >= 12) && self.bedroom_of(i).is_none()
        }).collect();
        want.sort_by_key(|&i| (std::cmp::Reverse(self.settlers[i].past.as_ref().map_or(30, |p| p.age)), i));
        let mut given: Vec<String> = Vec::new();
        for i in want {
            if self.bedroom_of(i).is_some() { continue; }
            let Some(k) = self.rooms.iter().position(|r| r.kind == RoomKind::Bedroom && r.owner.is_none()) else { break };
            self.rooms[k].owner = Some(i);
            given.push(self.settlers[i].name.clone());
        }
        if !given.is_empty() {
            let z = self.rooms.iter().find(|r| r.kind == RoomKind::Bedroom).map(|r| r.z).unwrap_or(0);
            let rock = self.rock_word_at(z);
            let down = self.delve_mouth.map(|m| self.map.surface_z[m.1 as usize * self.map.width + m.0 as usize] - z).unwrap_or(0);
            let names = super::join_names(&given);
            self.note(format!("{} {} a bedroom of {} own, cut in the {} {} levels down.", names, if given.len() == 1 { "takes" } else { "take" }, "their", rock, down));
        }
    }

    /// The rock the rooms at level `z` are cut in ("granite"), from the spine's column.
    pub(crate) fn rock_word_at(&self, z: i32) -> String {
        let at = self.spine.map(|s| s.at).unwrap_or(self.camp);
        match self.map.cell(at.0 as usize, at.1 as usize, (z.max(0) as usize).min(self.map.depth - 1)).material {
            crate::local::Material::Rock(r) | crate::local::Material::Block(r) => format!("{:?}", r).to_lowercase(),
            crate::local::Material::Ore(r) => format!("{:?}-bearing rock", r).to_lowercase(),
            crate::local::Material::Soil | crate::local::Material::Clay => "earth".into(),
            _ => "rock".into(),
        }
    }

    /// Dig projects of the delve, for `plan_projects`: bedrooms when grown settlers sleep without
    /// a room of their own, a great hall when the camp outgrows the fire.
    pub(crate) fn delve_candidates(&self, c: &mut Vec<(f32, ProjectKind, String, u32, Pos)>) {
        if self.dig_plan.is_some() || self.spine.is_none() || self.delve_mouth.is_none() { return; }
        let dug = self.projects.iter().any(|p| p.done && matches!(p.kind, ProjectKind::DugHall | ProjectKind::Cellar));
        if !dug || self.projects.iter().any(|p| !p.done && projects::is_dig(p.kind)) { return; }
        let day = self.clock.day();
        let masons = self.way.as_ref().map_or(false, |w| w.stone_first);
        let without = self.grown_without_rooms();
        if day >= 20 && without >= 4 {
            if let Some(plan) = self.plan_dig(ProjectKind::Bedrooms) {
                let n = plan.rooms.iter().filter(|r| r.kind == RoomKind::Bedroom).count();
                let down = plan.rooms[0].z;
                let top = self.map.surface_z[self.delve_mouth.unwrap().1 as usize * self.map.width + self.delve_mouth.unwrap().0 as usize];
                let why = format!("{} of them sleep {} with no room of their own; {} bedrooms {} levels down, off a stair, would give them one", without,
                    if self.hall_cells.is_empty() { "in the huts and by the fire" } else { "on the hall's floor" }, n, top - down);
                c.push((if masons { 1.5 } else { 0.8 }, ProjectKind::Bedrooms, why, plan.cuts.len() as u32, self.spine.unwrap().at));
            }
        }
        let has = |k: RoomKind| self.rooms.iter().any(|r| r.kind == k);
        // A ditch round the wall, once the palisade stands and trouble keeps coming.
        let walled = self.projects.iter().any(|p| p.done && p.kind == ProjectKind::Palisade);
        let chapters = self.arc.as_ref().map_or(0, |a| a.chapter);
        if walled && chapters >= 1 && day >= 30 && !self.projects.iter().any(|p| p.kind == ProjectKind::Moat) {
            if let Some(plan) = self.plan_moat() {
                let raids = self.arc.as_ref().map_or(0, |a| a.events.iter().filter(|e| e.title == "The raid").count());
                let why = format!("{} {} come to the palisade; a ditch two levels deep round it would leave them only the four gates{}", raids, if raids == 1 { "raid has" } else { "raids have" },
                    if self.projects.iter().any(|p| p.done && p.kind == ProjectKind::Traps) { ", where the cages wait" } else { "" });
                c.push((if masons { 1.2 } else { 0.8 }, ProjectKind::Moat, why, plan.cuts.len() as u32, self.camp));
            }
        }
        // Tombs, once the dead lie at the camp's edge (two or more of the camp's own).
        let graves = self.marks.iter().filter(|m| m.kind == MarkKind::Grave && m.title.starts_with("The grave of ")).count();
        if day >= 40 && graves >= 2 && !has(RoomKind::Tomb) {
            if let Some(plan) = self.plan_dig(ProjectKind::Tombs) {
                let pious = self.temple().is_some();
                let why = format!("{} of the camp lie in graves at its edge, under the rain{}; ten niches cut in the rock would keep the dead", graves,
                    if self.darkness >= 0.2 { " and the Shadow" } else { "" });
                c.push((if masons { 1.2 } else { 0.7 } + if pious { 0.4 } else { 0.0 } + if self.darkness >= 0.2 { 0.4 } else { 0.0 }, ProjectKind::Tombs, why, plan.cuts.len() as u32, self.spine.unwrap().at));
            }
        }
        if day >= 30 && self.alive() >= 10 && !has(RoomKind::GreatHall) && (has(RoomKind::Bedroom) || masons) {
            if let Some(plan) = self.plan_dig(ProjectKind::GreatHall) {
                let why = format!("{} eat by the fire in all weathers; a great hall below would seat them all at one table, out of the wind", self.alive());
                c.push((if masons { 1.3 } else { 0.7 }, ProjectKind::GreatHall, why, plan.cuts.len() as u32, self.spine.unwrap().at));
            }
        }
    }
}
