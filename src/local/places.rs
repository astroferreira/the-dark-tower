//! Something down there: places in the rock of an embark, each with a cause.
//!
//! Nothing is placed by a bare roll. A beast laired on the tile has its lair (its hoard by name,
//! the bones of those it killed); a battle fought here with named dead leaves a tomb; a town that
//! fell on this tile leaves its old mine where ore lies near; limestone or a cave biome makes a
//! natural cave; deep under some tiles of soft rock lies a cavern with water. Places are carved
//! like the colony's digs (2.5-D: a column's floor lowered under a roof of rock): a passage into
//! a rise, or a sinkhole ramp down to a room on flat ground. The deep cavern is cut as open cells
//! far below the floor, reached only by digging (card 'Dig too deep').

use super::{LocalMap, Material, Plant, Shape};
use crate::erosion::materials::RockType;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaceKind { Cave, Lair, Tomb, OldMine, Cavern, Halls }

impl PlaceKind {
    pub fn word(self) -> &'static str {
        match self { PlaceKind::Cave => "a cave", PlaceKind::Lair => "a lair", PlaceKind::Tomb => "a tomb", PlaceKind::OldMine => "an old mine", PlaceKind::Cavern => "a deep cavern", PlaceKind::Halls => "halls under the ground" }
    }
}

/// A place in the rock: its kind, name, why it is here, what lies in it, its cells (column,
/// floor level), the mouth on the surface (none for a deep cavern) and whether anyone has found
/// it yet.
#[derive(Clone, Debug)]
pub struct UnderPlace {
    pub kind: PlaceKind,
    pub name: String,
    pub cause: String,
    pub contents: Vec<String>,
    pub cells: Vec<((u16, u16), i32)>,
    pub mouth: Option<(u16, u16)>,
    pub found: bool,
}

fn hash(a: i64, b: i64, salt: u64) -> u64 {
    let mut h = (a as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (b as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ salt;
    h ^= h >> 31;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^ (h >> 29)
}

/// Open a column's floor at level `z` under a roof (as the colony digs). False if it can't be.
fn carve(map: &mut LocalMap, x: usize, y: usize, z: i32) -> bool {
    let n = map.width;
    let sz = map.surface_z[y * n + x];
    if z > sz || z < 1 || (z + 2) as usize >= map.depth { return false; }
    let head = map.idx(x, y, (z + 1) as usize);
    map.cells[head].shape = Shape::Empty;
    map.cells[head].material = Material::Air;
    map.cells[head].plant = Plant::None;
    map.cells[head].boulder = false;
    map.cells[head].water = 0;
    let floor = map.idx(x, y, z as usize);
    map.cells[floor].shape = Shape::Floor;
    map.cells[floor].plant = Plant::None;
    map.cells[floor].boulder = false;
    map.surface_z[y * n + x] = z;
    true
}

fn dry_floor(map: &LocalMap, x: usize, y: usize) -> bool {
    let sz = map.surface_z[y * map.width + x];
    if sz < 1 || (sz + 1) as usize >= map.depth { return false; }
    let c = map.cell(x, y, sz as usize);
    let above = map.cell(x, y, (sz + 1) as usize);
    c.water == 0 && above.water == 0 && above.shape != Shape::Wall && !matches!(c.material, Material::Wood | Material::Block(_))
}

/// Find a way in near `(ax, ay)` (within 40 cells: the spot may lie in a lake): a passage into a rise if there is one, else a
/// sinkhole ramp down to a room. Returns the planned cells (column, floor level) and the mouth.
fn plan_way_in(map: &LocalMap, ax: i32, ay: i32, salt: u64, taken: &[(u16, u16)]) -> Option<(Vec<((u16, u16), i32)>, (u16, u16))> {
    let n = map.width as i32;
    let sz = |x: i32, y: i32| map.surface_z[(y * n + x) as usize];
    let inside = |x: i32, y: i32| x >= 6 && y >= 6 && x < n - 6 && y < map.height as i32 - 6;
    let clear = |cells: &[((u16, u16), i32)]| cells.iter().all(|c| !taken.iter().any(|t| (t.0 as i32 - c.0 .0 as i32).abs() <= 3 && (t.1 as i32 - c.0 .1 as i32).abs() <= 3));
    // A rise first: the nearest foot with ground three levels up within six cells.
    for r in 0..40i32 {
        for k in 0..(8 * r.max(1)) {
            let a = (k as f32 / (8 * r.max(1)) as f32) * std::f32::consts::TAU + (salt % 628) as f32 / 100.0;
            let (fx, fy) = (ax + (a.cos() * r as f32) as i32, ay + (a.sin() * r as f32) as i32);
            if !inside(fx, fy) || !dry_floor(map, fx as usize, fy as usize) { continue; }
            let z0 = sz(fx, fy);
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let rise = (1..=6).find(|&k| inside(fx + dx * k, fy + dy * k) && sz(fx + dx * k, fy + dy * k) >= z0 + 3);
                let Some(k0) = rise else { continue };
                // Every cell from the foot to the rise stays at about the foot's level.
                if (1..k0).any(|k| sz(fx + dx * k, fy + dy * k) > z0 + 2 || sz(fx + dx * k, fy + dy * k) < z0) { continue; }
                let mut cells = Vec::new();
                let (mut x, mut y) = (fx, fy);
                for _ in 0..12 {
                    x += dx; y += dy;
                    if !inside(x, y) { break; }
                    if sz(x, y) > z0 { cells.push(((x as u16, y as u16), z0)); }
                    if cells.len() >= 4 { break; }
                }
                if cells.len() < 3 { continue; }
                let (px, py) = (-dy, dx);
                for a in 1..=4 { for b in -2..=2 {
                    let (rx, ry) = (x + dx * a + px * b, y + dy * a + py * b);
                    if inside(rx, ry) && sz(rx, ry) > z0 + 1 { cells.push(((rx as u16, ry as u16), z0)); }
                } }
                if cells.len() >= 10 && clear(&cells) { return Some((cells, (fx as u16, fy as u16))); }
            }
        }
    }
    // Flat ground: a sinkhole of three steps down, then a 4x3 room three levels below.
    for r in 0..40i32 {
        for k in 0..(8 * r.max(1)) {
            let a = (k as f32 / (8 * r.max(1)) as f32) * std::f32::consts::TAU + (salt % 628) as f32 / 100.0;
            let (sx, sy) = (ax + (a.cos() * r as f32) as i32, ay + (a.sin() * r as f32) as i32);
            if !inside(sx, sy) || !dry_floor(map, sx as usize, sy as usize) { continue; }
            let z0 = sz(sx, sy);
            let dir = if hash(sx as i64, sy as i64, salt) % 2 == 0 { (1, 0) } else { (0, 1) };
            let mut cells = Vec::new();
            let mut ok = true;
            for k in 1..=3 {
                let (x, y) = (sx + dir.0 * k, sy + dir.1 * k);
                if !inside(x, y) || !dry_floor(map, x as usize, y as usize) || (sz(x, y) - z0).abs() > 1 { ok = false; break; }
                cells.push(((x as u16, y as u16), z0 - k));
            }
            if !ok { continue; }
            let end = (sx + dir.0 * 3, sy + dir.1 * 3);
            for a in 1..=4 { for b in -1..=1 {
                let (x, y) = if dir.0 != 0 { (end.0 + dir.0 * a, end.1 + b) } else { (end.0 + b, end.1 + dir.1 * a) };
                if !inside(x, y) || sz(x, y) < z0 - 1 || !dry_floor(map, x as usize, y as usize) { ok = false; }
                cells.push(((x as u16, y as u16), z0 - 3));
            } }
            if ok && clear(&cells) { return Some((cells, (sx as u16, sy as u16))); }
        }
    }
    None
}

// ---------------------------------------------------------------------------------------------
// Places in three dimensions (DF's site layouts: mines, crypts and dens on several levels)
// ---------------------------------------------------------------------------------------------

/// One cut of a place: open the cell over a floor (a room), open two (a step down a slope, or a
/// den two levels high), or cut a stair.
#[derive(Clone, Copy)]
enum Cut { Open(i32, i32, i32), Tall(i32, i32, i32), Stair(i32, i32, i32) }

/// Whether the cells a cut works on are rock or ground to cut (or open air at or above the
/// surface), out of the caverns and the water, inside the map.
fn cuttable(map: &LocalMap, c: Cut) -> bool {
    let (x, y, z, n) = match c { Cut::Open(x, y, z) => (x, y, z, 1), Cut::Tall(x, y, z) => (x, y, z, 2), Cut::Stair(x, y, z) => (x, y, z, 1) };
    if x < 4 || y < 4 || x >= map.width as i32 - 4 || y >= map.height as i32 - 4 || z < 3 || (z + 3) as usize >= map.depth { return false; }
    let (ux, uy) = (x as usize, y as usize);
    let sz = map.surface_z[uy * map.width + ux];
    if sz < z { return false; }
    (z..=z + n).all(|zz| {
        let cell = map.cell(ux, uy, zz as usize);
        cell.water == 0 && map.cavern_at(ux, uy, zz).is_none() && (zz > sz || matches!(cell.shape, Shape::Wall | Shape::Floor | Shape::Ramp))
    })
}

fn apply(map: &mut LocalMap, c: Cut) {
    let n = map.width;
    let open = |map: &mut LocalMap, x: usize, y: usize, zz: i32| {
        let k = map.idx(x, y, zz as usize);
        map.cells[k].shape = Shape::Empty;
        map.cells[k].material = Material::Air;
        map.cells[k].plant = Plant::None;
        map.cells[k].boulder = false;
    };
    match c {
        Cut::Open(x, y, z) | Cut::Tall(x, y, z) => {
            let (ux, uy) = (x as usize, y as usize);
            let sz = map.surface_z[uy * n + ux];
            let top = if matches!(c, Cut::Tall(..)) { z + 2 } else { z + 1 };
            for zz in z + 1..=top { if zz <= sz { open(map, ux, uy, zz); } }
            let f = map.idx(ux, uy, z as usize);
            if map.cells[f].shape == Shape::Wall { map.cells[f].shape = Shape::Floor; }
            map.cells[f].plant = Plant::None;
            map.cells[f].boulder = false;
            // The surface's own cell opened: the ground is cut down there.
            if top >= sz && z < sz { map.surface_z[uy * n + ux] = z; }
        }
        Cut::Stair(x, y, z) => {
            let k = map.idx(x as usize, y as usize, (z + 1) as usize);
            map.cells[k].shape = Shape::Stair;
            map.cells[k].plant = Plant::None;
            map.cells[k].boulder = false;
        }
    }
}

/// A place laid out on several levels from a mouth on flat dry ground at `(mx, my)`: the cuts
/// and the floor cells walked (column, level), the far end last.
fn layout(map: &LocalMap, kind: PlaceKind, mx: i32, my: i32, salt: u64) -> Option<(Vec<Cut>, Vec<((u16, u16), i32)>)> {
    let z0 = map.surface_z[my as usize * map.width + mx as usize];
    let dirs = [(1i32, 0i32), (0, 1), (-1, 0), (0, -1)];
    let d0 = (hash(mx as i64, my as i64, salt) % 4) as usize;
    let mut cuts: Vec<Cut> = Vec::new();
    let mut walk: Vec<((u16, u16), i32)> = Vec::new();
    let pos = |x: i32, y: i32| (x as u16, y as u16);
    // A stair shaft from the mouth down to `bottom`.
    let shaft = |cuts: &mut Vec<Cut>, walk: &mut Vec<((u16, u16), i32)>, x: i32, y: i32, top: i32, bottom: i32| {
        for z in (bottom..top).rev() { cuts.push(Cut::Stair(x, y, z)); walk.push((pos(x, y), z)); }
    };
    // A straight passage on level `z` from beside `(x, y)` along `d`, `len` long.
    let passage = |cuts: &mut Vec<Cut>, walk: &mut Vec<((u16, u16), i32)>, x: i32, y: i32, d: (i32, i32), len: i32, z: i32| -> (i32, i32) {
        let (mut px, mut py) = (x, y);
        for _ in 0..len { px += d.0; py += d.1; cuts.push(Cut::Open(px, py, z)); walk.push((pos(px, py), z)); }
        (px, py)
    };
    // A room on level `z` beyond `(x, y)` along `d`: `deep` cells on, `half` cells either side.
    let chamber = |cuts: &mut Vec<Cut>, walk: &mut Vec<((u16, u16), i32)>, x: i32, y: i32, d: (i32, i32), deep: i32, half: i32, z: i32, tall: bool| {
        let e = (-d.1, d.0);
        for a in 1..=deep { for b in -half..=half {
            let (cx, cy) = (x + d.0 * a + e.0 * b, y + d.1 * a + e.1 * b);
            cuts.push(if tall { Cut::Tall(cx, cy, z) } else { Cut::Open(cx, cy, z) });
            walk.push((pos(cx, cy), z));
        } }
    };
    match kind {
        PlaceKind::OldMine => {
            // A shaft eight levels down, with a gallery off it at three levels (each its own
            // way) and side workings; the deepest gallery ends at the seam.
            shaft(&mut cuts, &mut walk, mx, my, z0, z0 - 8);
            for (j, lvl) in [3, 6, 8].iter().enumerate() {
                let d = dirs[(d0 + j) % 4];
                let len = 6 + (hash(mx as i64, *lvl as i64, salt) % 4) as i32;
                let end = passage(&mut cuts, &mut walk, mx, my, d, len, z0 - lvl);
                let side = (-d.1, d.0);
                passage(&mut cuts, &mut walk, mx + d.0 * (len / 2), my + d.1 * (len / 2), side, 3, z0 - lvl);
                if j == 2 { walk.push((pos(end.0, end.1), z0 - lvl)); }
            }
        }
        PlaceKind::Tomb => {
            // A stair four levels down to an antechamber, a passage to the crypt, and a second
            // stair from the crypt's far end down to the inner tomb.
            let d = dirs[d0];
            shaft(&mut cuts, &mut walk, mx, my, z0, z0 - 4);
            let zb = z0 - 4;
            chamber(&mut cuts, &mut walk, mx, my, d, 3, 1, zb, false);
            let (ex, ey) = passage(&mut cuts, &mut walk, mx + d.0 * 3, my + d.1 * 3, d, 3, zb);
            chamber(&mut cuts, &mut walk, ex, ey, d, 5, 1, zb, false);
            let (sx, sy) = (ex + d.0 * 5, ey + d.1 * 5);
            shaft(&mut cuts, &mut walk, sx, sy, zb, zb - 2);
            chamber(&mut cuts, &mut walk, sx, sy, d, 3, 1, zb - 2, true);
        }
        PlaceKind::Halls => {
            // DF's mountain halls: a stair five levels down from the plaza to a great hall two
            // levels high, a chamber off each of its other sides (stores, a forge, the dead),
            // and a stair on down from the hall's far end to a deep chamber.
            let d = dirs[d0];
            let e = (-d.1, d.0);
            shaft(&mut cuts, &mut walk, mx, my, z0, z0 - 5);
            let zb = z0 - 5;
            let (hx, hy) = passage(&mut cuts, &mut walk, mx, my, d, 2, zb);
            chamber(&mut cuts, &mut walk, hx, hy, d, 9, 3, zb, true);
            let (cx, cy) = (hx + d.0 * 5, hy + d.1 * 5);
            for side in [1i32, -1] {
                let (px, py) = passage(&mut cuts, &mut walk, cx + e.0 * side * 3, cy + e.1 * side * 3, (e.0 * side, e.1 * side), 2, zb);
                chamber(&mut cuts, &mut walk, px, py, (e.0 * side, e.1 * side), 3, 1, zb, false);
            }
            let (fx, fy) = passage(&mut cuts, &mut walk, hx + d.0 * 9, hy + d.1 * 9, d, 2, zb);
            shaft(&mut cuts, &mut walk, fx, fy, zb, zb - 3);
            chamber(&mut cuts, &mut walk, fx, fy, d, 3, 1, zb - 3, false);
        }
        PlaceKind::Lair | PlaceKind::Cave | PlaceKind::Cavern => {
            // A tunnel winding down from a pit, a level every other step, to a den two levels
            // high (a lair's wide, a cave's long and narrow).
            let (mut x, mut y, mut z) = (mx, my, z0);
            let mut d = dirs[d0];
            for k in 0..16 {
                if k % 5 == 4 { let t = hash(x as i64, y as i64, salt ^ k as u64) % 3; d = match t { 0 => (-d.1, d.0), 1 => (d.1, -d.0), _ => d }; }
                x += d.0; y += d.1;
                let down = k % 2 == 0 && z > z0 - 6;
                if down { z -= 1; cuts.push(Cut::Tall(x, y, z)); } else { cuts.push(Cut::Open(x, y, z)); }
                walk.push((pos(x, y), z));
            }
            let (deep, half) = if kind == PlaceKind::Lair { (6, 3) } else { (7, 1) };
            chamber(&mut cuts, &mut walk, x, y, d, deep, half, z, true);
        }
    }
    cuts.iter().all(|&c| cuttable(map, c)).then_some((cuts, walk))
}

/// Ore left in the rock round the end of an old gallery.
fn seam(map: &mut LocalMap, end: (u16, u16), z: i32, ore: crate::history::civilizations::economy::ResourceType) {
    for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
        let (x, y) = ((end.0 as i32 + dx) as usize, (end.1 as i32 + dy) as usize);
        let k = map.idx(x, y, (z + 1) as usize);
        if map.cells[k].shape == Shape::Wall && matches!(map.cells[k].material, Material::Rock(_)) { map.cells[k].material = Material::Ore(ore); }
    }
}

/// The rock under the embark's middle.
pub(crate) fn host_rock(map: &LocalMap) -> Option<RockType> {
    let (x, y) = (map.width / 2, map.height / 2);
    // The commonest rock in the centre column's top 30 levels (what a cave or a dig is cut in;
    // the deep rock under the caverns doesn't count), loose sediment aside.
    let mut count: Vec<(RockType, usize)> = Vec::new();
    let sz = map.surface_z[y * map.width + x].max(0) as usize;
    for z in sz.saturating_sub(30)..map.depth {
        if let Material::Rock(r) = map.cell(x, y, z).material {
            if r == RockType::Sediment { continue; }
            match count.iter_mut().find(|c| c.0 == r) { Some(c) => c.1 += 1, None => count.push((r, 1)) }
        }
    }
    count.into_iter().max_by_key(|c| c.1).map(|c| c.0)
}

/// Put the places this embark's history and rock call for. `centre` is kept clear (the camp).
pub fn place(map: &mut LocalMap, lore: Option<&crate::lore::RegionLore>, world: &crate::world::WorldData, world_tile: (usize, usize), tile_of: impl Fn(f64, f64) -> (usize, usize), seed: u64) -> Vec<UnderPlace> {
    let mut out: Vec<UnderPlace> = Vec::new();
    let (n, h) = (map.width as i32, map.height as i32);
    let mut want: Vec<(PlaceKind, String, String, Vec<String>)> = Vec::new();
    // A beast's lair on this tile.
    if let Some(lairs) = lore.and_then(|l| l.lairs.get(&world_tile)) {
        if let Some(b) = lairs.iter().find(|b| b.alive).or(lairs.first()) {
            let mut contents: Vec<String> = b.hoard.iter().map(|a| format!("{}, in its hoard", a)).collect();
            if b.kills > 0 { contents.push(format!("the bones of {} it killed", b.kills)); }
            let cause = if b.alive { format!("{} lairs here, and lives", b.name) } else { format!("{} laired here until its death in {}", b.name, b.died.unwrap_or(0)) };
            want.push((PlaceKind::Lair, format!("The lair of {}", b.name), cause, contents));
        }
    }
    // A tomb for the most notable named dead of a battle fought here.
    if let Some(battles) = lore.and_then(|l| l.battles.get(&world_tile)) {
        // A battle's dead first (a lone death event's title already names its figure).
        if let Some((title, year, dead)) = battles.iter().filter(|b| !b.2.is_empty()).max_by_key(|b| (b.0.contains("Battle"), b.2.len(), b.1)) {
            let who = dead[0].clone();
            let short = who.split(',').next().unwrap_or(&who).split(" of ").next().unwrap_or(&who).to_string();
            let mut contents = vec![format!("{} in a stone chamber, with their arms", who)];
            if dead.len() > 1 { contents.push(format!("{} more of the fallen beside them", dead.len() - 1)); }
            let cause = if title.contains("Battle") { format!("{} fell here in the {} ({})", who, title.trim_start_matches("The ").trim_start_matches("the "), year) } else { format!("{} fell here in {}", who, year) };
            want.push((PlaceKind::Tomb, format!("The tomb of {}", short), cause, contents));
        }
    }
    // The old works of a town that fell on this tile: its mine where ore lies near, else the
    // undercroft it kept its stores in.
    let ore = world.resources().deposits_near(world_tile.0, world_tile.1, 1, world.width).into_iter().next().map(|d| d.kind);
    if let Some(l) = lore {
        if let Some(t) = l.sites.iter().filter(|t| t.destroyed_year.is_some()).find(|t| tile_of(t.x, t.y) == world_tile) {
            let year = t.destroyed_year.unwrap_or(0);
            // A people who carved their dwellings (dwarves: DF's mountain halls) left halls.
            if t.carved {
                want.push((PlaceKind::Halls, format!("The halls of {}", t.name), format!("{} of {} carved its halls under the ground here, and they stand empty since it fell in {}", t.name, t.faction_name, year),
                    vec!["a pillared hall gone cold, empty bins, a forge without fire".to_string(), "bones by a broken door".to_string()]));
            } else {
            match ore {
                Some(kind) => want.push((PlaceKind::OldMine, format!("The old {:?} mine of {}", kind, t.name), format!("{} worked the {:?} here until it fell in {}", t.name, kind, year), vec![format!("worked-out galleries and a seam of {:?} left behind", kind)])),
                None => want.push((PlaceKind::OldMine, format!("The undercroft of {}", t.name), format!("{} kept its stores under the ground here until it fell in {}", t.name, year), vec!["empty bins, a broken lamp, and a door barred from within".into()])),
            }
            }
        }
    }
    // A natural cave where the rock or the biome says so.
    let biome = *world.biomes.get(world_tile.0, world_tile.1);
    use crate::biomes::ExtendedBiome as B;
    let rock = host_rock(map);
    let karst = matches!(biome, B::KarstPlains | B::TowerKarst | B::CaveEntrance | B::CockpitKarst);
    // Water opens caves in limestone on half the tiles (hashed by the tile), in sandstone and
    // shale on a third, in karst always.
    let roll = hash(world_tile.0 as i64, world_tile.1 as i64, 0xCA4E);
    let hollowed = match rock { Some(RockType::Limestone) => roll % 2 == 0, Some(RockType::Sandstone) | Some(RockType::Shale) => roll % 3 == 0, _ => false };
    if karst || hollowed {
        let why = if karst { format!("the {:?} here is hollowed by water", biome) } else { format!("the {:?} here is hollowed by water", rock.unwrap()).to_lowercase() };
        want.push((PlaceKind::Cave, format!("A cave in the {}", format!("{:?}", rock.unwrap_or(RockType::Limestone)).to_lowercase()), why, vec!["dripping stone and a cold draught".into()]));
    }
    let mut taken: Vec<(u16, u16)> = vec![((n / 2) as u16, (h / 2) as u16)];
    for (k, (kind, name, cause, contents)) in want.into_iter().enumerate() {
        // Away from the middle (where camps are made), on dry ground 30-80 cells out, the spots
        // tried in an order hashed by the tile (never a bare roll).
        let salt = seed ^ (k as u64 + 1).wrapping_mul(0x51ED);
        let mut spots: Vec<(u64, i32, i32)> = Vec::new();
        for y in (8..h - 8).step_by(6) { for x in (8..n - 8).step_by(6) {
            let d = (((x - n / 2).pow(2) + (y - h / 2).pow(2)) as f32).sqrt();
            if (30.0..80.0).contains(&d) && dry_floor(map, x as usize, y as usize) { spots.push((hash(x as i64, y as i64, salt ^ world_tile.0 as u64 ^ ((world_tile.1 as u64) << 16)), x, y)); }
        } }
        spots.sort();
        // In three dimensions first (a mine's shaft and galleries, a tomb's stair and crypts, a
        // lair's winding tunnel down to its den), else the old passage or sinkhole.
        let clear = |cells: &[((u16, u16), i32)], taken: &[(u16, u16)]| cells.iter().all(|c| !taken.iter().any(|t| (t.0 as i32 - c.0 .0 as i32).abs() <= 3 && (t.1 as i32 - c.0 .1 as i32).abs() <= 3));
        let laid = spots.iter().take(24).find_map(|&(_, x, y)| {
            if !dry_floor(map, x as usize, y as usize) { return None; }
            let (cuts, walk) = layout(map, kind, x, y, salt)?;
            clear(&walk, &taken).then_some((cuts, walk, (x as u16, y as u16)))
        });
        if let Some((cuts, walk, mouth)) = laid {
            for &c in &cuts { apply(map, c); }
            // The old mine's seam, left in the gallery's end wall.
            if kind == PlaceKind::OldMine { if let (Some(o), Some(&(end, z))) = (ore, walk.last()) { seam(map, end, z, o); } }
            taken.extend(walk.iter().map(|c| c.0));
            out.push(UnderPlace { kind, name, cause, contents, cells: walk, mouth: Some(mouth), found: false });
            continue;
        }
        let Some((cells, mouth)) = spots.iter().take(12).find_map(|&(_, x, y)| plan_way_in(map, x, y, salt, &taken)) else { continue };
        for &((x, y), z) in &cells { carve(map, x as usize, y as usize, z); }
        taken.extend(cells.iter().map(|c| c.0));
        out.push(UnderPlace { kind, name, cause, contents, cells, mouth: Some(mouth), found: false });
    }
    // The caverns under every embark are their own record (`caverns.rs`, `LocalMap::caverns`);
    // where the first lies shallow (13 levels or less) it is within a mine's reach, and known.
    if let Some(c) = map.caverns.iter().find(|c| c.layer == 0 && c.depth_levels <= 13 && c.floor_cells >= 300) {
        let n = map.width;
        let mut cells: Vec<((u16, u16), i32)> = (0..n * map.height).filter_map(|col| {
            let (f, _) = map.cavern_z[col][0];
            (f >= 0).then(|| (((col % n) as u16, (col / n) as u16), f as i32))
        }).collect();
        cells.sort_by_key(|((x, y), _)| ((*x as i32 - n as i32 / 2).abs() + (*y as i32 - n as i32 / 2).abs(), *x, *y));
        cells.truncate(60);
        let mut contents = vec![format!("{} levels down: {}", c.depth_levels, c.life.join(", "))];
        if c.fungus > 0 { contents.push(format!("a wood of {} fungus trees", c.fungus)); }
        if c.water_cells > 0 { contents.push("still black pools".into()); }
        let rock_word = format!("{:?}", rock.unwrap_or(RockType::Granite)).to_lowercase();
        out.push(UnderPlace { kind: PlaceKind::Cavern, name: "The first cavern".into(), cause: format!("the first cavern rises to within {} levels of the ground here, under the {}", c.depth_levels, rock_word), contents, cells, mouth: None, found: false });
    }
    out
}
