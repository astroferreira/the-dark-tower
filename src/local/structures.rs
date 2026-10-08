//! Settlements, ruins, fields and roads at person scale.
//!
//! Each settlement gets a deterministic town plan in metres around its site (seeded by its
//! id, so every embark that touches it sees the same town): streets out along the directions
//! its roads arrive from, house lots along the streets built in the culture's material, a keep
//! and plaza for cities, a wall ring with gates for walled towns, and strip fields around it.
//! A ruin uses the same plan, but most walls have fallen, floors are rubble and plants are
//! taking it back.
//!
//! Buildings stand in three dimensions (DF's site realizations: castle towers, castle walls,
//! shop houses, hillock houses): a storey is two levels (its floor, then standing room), so a
//! keep of dressed stone rises two or three storeys with a floor laid at each and a stair up to
//! a walkable roof behind battlements; a stone wall has towers a level above its walk; larger
//! stone houses have an upper floor and some timber houses a loft; some houses have a cellar
//! cut under the ground, reached by a stair from inside (a carving people's bigger). In a ruin
//! the upper storeys have mostly fallen to rubble and the stairs are broken, but the cellars
//! are as they were, and empty.

use crate::history::civilizations::settlement::{SettlementType, WallLevel};
use crate::lore::settle::{BuildStyle, Site};
use crate::lore::RegionLore;
use crate::region::zoom::ZoomRegion;

use super::{Cell, LocalMap, Material, Plant, Shape, TILE_M};

fn hash(a: i64, b: i64, salt: u64) -> u64 {
    let mut h = (a as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (b as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ salt;
    h ^= h >> 31;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^ (h >> 29)
}
fn unit(h: u64) -> f64 { (h >> 11) as f64 / (1u64 << 53) as f64 }

fn seg_dist(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
    ((a.0 + dx * t - p.0).powi(2) + (a.1 + dy * t - p.1).powi(2)).sqrt()
}

/// A rectangular building in metres, in the plan's (warped) frame, oriented along its street.
struct House {
    u: f64,
    v: f64,
    /// Orientation of the house's long axis relative to the frame.
    ang: f64,
    /// Half extents along / across the house axes (m).
    hu: f64,
    hv: f64,
    /// Door on the -v face (toward the street) after rotation.
    stone: bool,
    /// Bounding radius, for quick rejection.
    reach: f64,
    kind: BuildingKind,
}

/// What a building of a town is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BuildingKind { House, Hall, Keep, Tower }

/// A town's building as built on the embark, in three dimensions.
#[derive(Clone, Debug)]
pub struct Building {
    pub kind: BuildingKind,
    pub stone: bool,
    pub ruined: bool,
    /// Storeys as built (1: the ground floor only); a loft is not a storey.
    pub storeys: u8,
    pub loft: bool,
    /// The level of the ground floor (the footprint's highest ground).
    pub ground: i32,
    /// The stair's column: up to the storeys and the roof, down to the cellar.
    pub stair: Option<(u16, u16)>,
    /// The levels of the floors laid above the ground floor (an upper storey, a loft) that stand.
    pub upper: Vec<i32>,
    /// The walkable roof behind battlements (a keep, a wall's tower), if it stands.
    pub platform: Option<i32>,
    /// The cellar: (column, floor level), the stair's foot first; empty if there is none.
    pub cellar: Vec<((u16, u16), i32)>,
}

struct Plan {
    site: Site,
    /// Site centre in metres (region frame).
    cx_m: f64,
    cy_m: f64,
    angle: f64,
    /// Streets as segments in the plan frame, with half-widths.
    streets: Vec<((f64, f64), (f64, f64), f64)>,
    houses: Vec<House>,
    wall_r: Option<(f64, f64)>,
    plaza_r: f64,
    seed: u64,
}

/// Smooth value noise in [-1, 1] for gently bending streets.
fn wob(seed: u64, a: f64) -> f64 {
    let (i, f) = (a.floor(), a - a.floor());
    let v = |k: f64| unit(hash(k as i64, 5, seed)) * 2.0 - 1.0;
    let t = f * f * (3.0 - 2.0 * f);
    v(i) * (1.0 - t) + v(i + 1.0) * t
}

impl Plan {
    /// Region metres -> plan frame: rotate to the street axis, then undo a slow warp so that
    /// "straight" streets and rows of houses become gently curved in the world.
    fn to_frame(&self, x_m: f64, y_m: f64) -> (f64, f64) {
        let (dx, dy) = (x_m - self.cx_m, y_m - self.cy_m);
        let (s, c) = self.angle.sin_cos();
        let (u, v) = (dx * c + dy * s, -dx * s + dy * c);
        (u + WARP_M * wob(self.seed, v / 90.0), v + WARP_M * wob(self.seed ^ 0x55, u / 90.0))
    }
}

/// Amplitude (m) of the street warp.
const WARP_M: f64 = 7.0;

fn rect_dist(p: (f64, f64), h: &House) -> f64 {
    // Signed distance (<0 inside) to the house rectangle.
    let (dx, dy) = (p.0 - h.u, p.1 - h.v);
    let (s, c) = h.ang.sin_cos();
    let (lu, lv) = (dx * c + dy * s, -dx * s + dy * c);
    (lu.abs() - h.hu).max(lv.abs() - h.hv)
}

fn plan_for(site: &Site, lore: &RegionLore, cell_m: f64) -> Plan {
    let id = site.id.0 as i64;
    let seed = id as u64 ^ 0xC17;
    let (cx_m, cy_m) = (site.x * cell_m, site.y * cell_m);
    // Street directions: the roads that leave this settlement.
    let mut dirs: Vec<f64> = Vec::new();
    for road in &lore.roads {
        for (end, next) in [(road.first(), road.get(1)), (road.last(), road.get(road.len().saturating_sub(2)))] {
            let (Some(e), Some(n)) = (end, next) else { continue };
            if ((e.0 - site.x).powi(2) + (e.1 - site.y).powi(2)).sqrt() < 1.0 {
                dirs.push((n.1 - e.1).atan2(n.0 - e.0));
            }
        }
    }
    // Plans are axis-aligned (snapped to the nearest quarter turn): rotated rectangles turn into
    // jagged diamonds on a tile grid. Approach roads still arrive at their real angle.
    let raw = dirs.first().copied().unwrap_or(unit(hash(id, 1, 3)) * std::f64::consts::PI);
    let angle = (raw / std::f64::consts::FRAC_PI_2).round() * std::f64::consts::FRAC_PI_2;
    let core = (site.core_m as f64).max(40.0);
    let big = matches!(site.kind, SettlementType::Capital | SettlementType::City);
    let mut plan = Plan { site: site.clone(), cx_m, cy_m, angle, streets: Vec::new(), houses: Vec::new(), wall_r: None, plaza_r: 0.0, seed };

    let main_w = if core > 200.0 { 4.0 } else { 3.0 };
    plan.streets.push(((-core * 1.05, 0.0), (core * 1.05, 0.0), main_w));
    plan.streets.push(((0.0, -core * 0.9), (0.0, core * 0.9), main_w * 0.8));
    for d in dirs.iter().skip(1) {
        let a = d - angle;
        plan.streets.push(((0.0, 0.0), (a.cos() * core * 1.05, a.sin() * core * 1.05), main_w));
    }
    // Back lanes in places big enough to need them.
    let mut lane = 34.0;
    while lane < core * 0.85 {
        for sign in [-1.0, 1.0] {
            let len = (core * core - lane * lane).max(0.0).sqrt();
            plan.streets.push(((-len, sign * lane), (len, sign * lane), 2.0));
            plan.streets.push(((sign * lane, -len), (sign * lane, len), 2.0));
        }
        lane += 34.0;
    }
    plan.plaza_r = if big { 11.0 } else if core > 110.0 { 7.0 } else { 0.0 };
    if site.walls != WallLevel::None && core >= 100.0 {
        let thick = if site.walls == WallLevel::Palisade { 1.0 } else { 3.0 };
        plan.wall_r = Some((core * 1.04, thick));
    }

    let stone_of = |r: f64| match site.style {
        BuildStyle::Stone => true,
        BuildStyle::Earth => false,
        BuildStyle::Wood => big && r < core * 0.35,
    };
    // A keep on the plaza in capitals and forts; a hall in towns (first, so no house is built
    // into it).
    if matches!(site.kind, SettlementType::Capital | SettlementType::Fort) {
        let ang = 0.0;
        plan.houses.push(House { u: 0.0, v: -plan.plaza_r - 14.0, ang, hu: 14.0, hv: 10.0, stone: true, reach: 17.2, kind: BuildingKind::Keep });
    } else if big || site.kind == SettlementType::Town {
        plan.houses.push(House { u: 0.0, v: plan.plaza_r + 9.0, ang: 0.0, hu: 10.0, hv: 6.0, stone: stone_of(0.0), reach: 11.7, kind: BuildingKind::Hall });
    }
    // Houses front the streets: walk along each, both sides, with variable lot width.
    let mut rng_k = 0u64;
    let streets = plan.streets.clone();
    for &(a, b, w) in &streets {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = (dx * dx + dy * dy).sqrt();
        if len < 8.0 { continue; }
        let ang = dy.atan2(dx);
        let (ux, uy) = (dx / len, dy / len);
        let (nx, ny) = (-uy, ux);
        for side in [-1.0f64, 1.0] {
            let mut t = 4.0 + unit(hash(id, rng_k as i64, 21)) * 4.0;
            while t < len - 4.0 {
                rng_k += 1;
                let hsh = hash(id, rng_k as i64, 33);
                let hu = 4.0 + (unit(hsh) * 3.0).floor();          // 4-6 m half-width: 8-12 m frontage
                let hv = 4.0 + (unit(hsh >> 9) * 3.0).floor();     // 4-6 m half-depth
                let setback = w + 1.5 + unit(hsh >> 18) * 2.0;
                let (cu, cv) = (a.0 + ux * (t + hu) + nx * side * (setback + hv), a.1 + uy * (t + hu) + ny * side * (setback + hv));
                let r = (cu * cu + cv * cv).sqrt();
                // Density falls off toward the edge; the very centre belongs to the plaza.
                let edge = core * (0.78 + 0.3 * unit(hash(id, rng_k as i64, 44)));
                let ok = r < edge && r > plan.plaza_r + hu.max(hv) + 2.0;
                t += 2.0 * hu + 1.0 + unit(hsh >> 27) * 5.0; // lots are rarely contiguous
                if !ok { continue; }
                // Face the street: rotate so the house's -v side looks at the street.
                let house = House { u: cu, v: cv, ang: if side > 0.0 { ang } else { ang + std::f64::consts::PI }, hu, hv, stone: stone_of(r), reach: hu.hypot(hv), kind: BuildingKind::House };
                if plan.wall_r.map(|(wr, _)| r + house.reach > wr - 4.0).unwrap_or(false) { continue; }
                // No overlaps with streets or other houses (allow a 1 m alley between houses).
                if plan.streets.iter().any(|&(p, q, sw)| seg_dist((cu, cv), p, q) - sw - house.hu.min(house.hv) < 0.0) { continue; }
                if plan.houses.iter().any(|o| {
                    let d = ((o.u - cu).powi(2) + (o.v - cv).powi(2)).sqrt();
                    d < o.reach + house.reach + 1.0 && (rect_dist((cu, cv), o) < house.hv.max(house.hu) + 0.5 || rect_dist((o.u, o.v), &house) < o.hu.max(o.hv) + 0.5)
                }) { continue; }
                plan.houses.push(house);
            }
        }
    }
    // Towers on a stone wall (DF's castle towers): square, 10 m, astride the ring between the
    // gates, about one every 70 m of wall, each door facing the town.
    if let Some((wr, _)) = plan.wall_r {
        if site.walls != WallLevel::Palisade {
            let count = ((std::f64::consts::TAU * wr / 70.0).round() as usize).clamp(4, 24);
            for k in 0..count {
                let th = (k as f64 + 0.5) * std::f64::consts::TAU / count as f64;
                let (tu, tv) = (wr * th.cos(), wr * th.sin());
                let ang = ((th - std::f64::consts::FRAC_PI_2) / std::f64::consts::FRAC_PI_2).round() * std::f64::consts::FRAC_PI_2;
                let tower = House { u: tu, v: tv, ang, hu: 5.0, hv: 5.0, stone: true, reach: 5.0f64.hypot(5.0), kind: BuildingKind::Tower };
                if plan.streets.iter().any(|&(a, b, w)| seg_dist((tu, tv), a, b) < w + 1.5 + tower.reach) { continue; }
                if plan.houses.iter().any(|o| {
                    let d = ((o.u - tu).powi(2) + (o.v - tv).powi(2)).sqrt();
                    d < o.reach + tower.reach + 1.0 && (rect_dist((tu, tv), o) < tower.reach + 0.5 || rect_dist((o.u, o.v), &tower) < o.hu.max(o.hv) + 0.5)
                }) { continue; }
                plan.houses.push(tower);
            }
        }
    }
    plan
}

/// What a plan puts at a point (frame metres).
enum Feature { Street, Plaza, Wall(bool), HouseWall(bool, bool), HouseFloor(bool, bool), Door(bool), Field(u64, bool), Garden, None }

/// The index of the house whose footprint (walls, floor, door) covers a point, if any.
fn house_at(plan: &Plan, u: f64, v: f64) -> Option<usize> {
    plan.houses.iter().position(|h| ((u - h.u).powi(2) + (v - h.v).powi(2)).sqrt() <= h.reach + 1.0 && rect_dist((u, v), h) <= 0.0)
}

fn feature_at(plan: &Plan, u: f64, v: f64) -> Feature {
    let ruined = plan.site.destroyed_year.is_some();
    let t = TILE_M as f64;
    for h in &plan.houses {
        if ((u - h.u).powi(2) + (v - h.v).powi(2)).sqrt() > h.reach + 1.0 { continue; }
        let d = rect_dist((u, v), h);
        if d > 0.0 { continue; }
        // Position in the house's own axes, for walls and the door on the -v side.
        let (dx, dy) = (u - h.u, v - h.v);
        let (s, c) = h.ang.sin_cos();
        let (lu, lv) = (dx * c + dy * s, -dx * s + dy * c);
        let on_wall = d > -t;
        if !on_wall { return Feature::HouseFloor(h.stone, ruined); }
        let door = lv < -h.hv + t && lu.abs() < t * 0.75;
        return if door { Feature::Door(ruined) } else { Feature::HouseWall(h.stone, ruined) };
    }
    let r = (u * u + v * v).sqrt();
    if let Some((wr, thick)) = plan.wall_r {
        if (r - wr).abs() < thick {
            let gate = plan.streets.iter().any(|&(a, b, w)| seg_dist((u, v), a, b) < w + 1.5);
            if !gate { return Feature::Wall(plan.site.walls != WallLevel::Palisade); }
        }
    }
    if r < plan.plaza_r { return Feature::Plaza; }
    let core = plan.site.core_m as f64;
    if r < core * 1.1 + 6.0 {
        for &(a, b, w) in &plan.streets {
            if seg_dist((u, v), a, b) < w { return Feature::Street; }
        }
        // Yards and kitchen gardens between buildings.
        if r < core * 1.05 { return Feature::Garden; }
    }
    if !ruined && r > core && r < plan.site.fields_m as f64 {
        // Strip fields: long parcels, alternate fallow and crops.
        let parcel = hash((u / 36.0).floor() as i64, (v / 110.0).floor() as i64, plan.site.id.0);
        let keep = 1.0 - (r / plan.site.fields_m as f64).powi(2);
        if unit(parcel) < keep * 0.9 {
            let row = ((u / t).floor() as i64).rem_euclid(2) == 0;
            return Feature::Field(parcel, row);
        }
    }
    Feature::None
}

/// Stamp settlements, ruins, fields and roads onto a generated local map.
pub fn apply(map: &mut LocalMap, region: &ZoomRegion, lore: &RegionLore, cx: f64, cy: f64) {
    let cell_m = region.cell_m as f64;
    let n = map.width;
    let tile_cells = TILE_M as f64 / cell_m;
    let half_m = n as f64 * TILE_M as f64 / 2.0;
    let (ex_m, ey_m) = (cx * cell_m, cy * cell_m);
    let plans: Vec<Plan> = lore.sites.iter()
        .filter(|s| {
            let reach = s.fields_m.max(s.core_m) as f64 * 1.1 + half_m * 1.5;
            ((s.x - cx) * cell_m).abs() < reach && ((s.y - cy) * cell_m).abs() < reach
        })
        .map(|s| plan_for(s, lore, cell_m))
        .collect();
    // Road segments near the embark, in metres.
    let mut roads: Vec<((f64, f64), (f64, f64))> = Vec::new();
    for road in &lore.roads {
        for w in road.windows(2) {
            let (a, b) = ((w[0].0 * cell_m, w[0].1 * cell_m), (w[1].0 * cell_m, w[1].1 * cell_m));
            if seg_dist((ex_m, ey_m), a, b) < half_m * 1.5 + cell_m { roads.push((a, b)); }
        }
    }
    if plans.is_empty() && roads.is_empty() { return; }
    // Columns under each standing house's roof: (plan, house) -> cells.
    let mut roofed: std::collections::BTreeMap<(usize, usize), (bool, bool, Vec<(usize, usize)>)> = Default::default();
    // Every house's cells (standing or fallen), to raise its walls, storeys and stair after.
    let mut footprints: std::collections::BTreeMap<(usize, usize), Vec<HouseCell>> = Default::default();
    for j in 0..n {
        for i in 0..n {
            let x_m = (cx + (i as f64 + 0.5 - n as f64 / 2.0) * tile_cells) * cell_m;
            let y_m = (cy + (j as f64 + 0.5 - n as f64 / 2.0) * tile_cells) * cell_m;
            let sz = map.surface_z[j * n + i] as usize;
            let floor_k = map.idx(i, j, sz);
            if map.cells[floor_k].shape == Shape::Empty || map.cells[floor_k].material == Material::Ice {
                continue;
            }
            let underwater = sz + 1 < map.depth && map.cells[map.idx(i, j, sz + 1)].water > 0;
            let (gx, gy) = ((x_m / TILE_M as f64).floor() as i64, (y_m / TILE_M as f64).floor() as i64);

            let mut done = false;
            for (pi, plan) in plans.iter().enumerate() {
                let (u, v) = plan.to_frame(x_m, y_m);
                if u.abs() > plan.site.fields_m.max(plan.site.core_m) as f64 * 1.1 + 5.0 || v.abs() > plan.site.fields_m.max(plan.site.core_m) as f64 * 1.1 + 5.0 { continue; }
                let f = feature_at(plan, u, v);
                if matches!(f, Feature::None) { continue; }
                if plan.site.destroyed_year.is_none() && !underwater {
                    if let Some(hi) = house_at(plan, u, v) {
                        let flat = matches!(plan.houses[hi].kind, BuildingKind::Keep | BuildingKind::Tower);
                        roofed.entry((pi, hi)).or_insert_with(|| (plan.houses[hi].stone, flat, Vec::new())).2.push((i, j));
                    }
                }
                // Streets, houses and fields stop at the water's edge.
                if underwater { continue; }
                let part = match f { Feature::HouseWall(..) => Some(Part::Wall), Feature::HouseFloor(..) => Some(Part::Floor), Feature::Door(_) => Some(Part::Door), _ => None };
                if let (Some(part), Some(hi)) = (part, house_at(plan, u, v)) {
                    let (lu, lv) = local_uv(&plan.houses[hi], u, v);
                    footprints.entry((pi, hi)).or_default().push(HouseCell { i, j, part, lu, lv, gx, gy });
                }
                let wobble = hash(gx, gy, 31);
                let stone_rock = crate::erosion::materials::RockType::Granite;
                let block = |stone: bool| if stone { Material::Block(stone_rock) } else { match plan.site.style { BuildStyle::Earth => Material::Clay, _ => Material::Wood } };
                let floor = &mut map.cells[floor_k];
                match f {
                    Feature::Street => {
                        floor.material = if plan.site.kind == SettlementType::Capital || plan.site.kind == SettlementType::City { Material::Block(stone_rock) } else { Material::Gravel };
                        floor.plant = Plant::None;
                        floor.boulder = false;
                        if plan.site.destroyed_year.is_some() && wobble % 4 == 0 { floor.plant = Plant::Grass; }
                    }
                    Feature::Plaza => {
                        floor.material = Material::Block(stone_rock);
                        floor.plant = Plant::None;
                        floor.boulder = plan.site.destroyed_year.is_some() && wobble % 9 == 0;
                    }
                    Feature::HouseFloor(stone, ruined) => {
                        floor.material = if stone { Material::Block(stone_rock) } else { Material::Wood };
                        floor.plant = Plant::None;
                        floor.boulder = false;
                        if ruined {
                            // Rotten floors, rubble and saplings.
                            floor.material = if stone { Material::Block(stone_rock) } else { Material::Soil };
                            floor.boulder = wobble % 7 == 0;
                            floor.plant = if wobble % 5 == 1 { Plant::Shrub } else if wobble % 3 == 0 { Plant::Grass } else { Plant::None };
                        }
                    }
                    Feature::HouseWall(..) => {
                        // Raised with the rest of its house (`raise`).
                        floor.plant = Plant::None;
                        floor.boulder = false;
                    }
                    Feature::Wall(stone) => {
                        floor.plant = Plant::None;
                        floor.boulder = false;
                        // In ruins only some stretches of wall still stand; the rest is rubble.
                        let standing = plan.site.destroyed_year.is_none() || wobble % 100 < 40;
                        if !standing {
                            floor.boulder = wobble % 2 == 0;
                        } else {
                            let storeys = if plan.site.walls != WallLevel::Palisade { 2 } else { 1 };
                            for dz in 1..=storeys {
                                if sz + dz < map.depth {
                                    let k = map.idx(i, j, sz + dz);
                                    map.cells[k] = Cell { shape: Shape::Wall, material: block(stone), water: 0, plant: Plant::None, boulder: false };
                                }
                            }
                        }
                    }
                    Feature::Garden => {
                        // Yards: trampled earth with the odd shrub or kitchen garden; trees are cleared.
                        if floor.shape != Shape::Ramp && matches!(floor.material, Material::Soil | Material::Clay | Material::Sand | Material::Snow) {
                            if matches!(floor.plant, Plant::Tree(_)) || floor.boulder { floor.plant = Plant::None; floor.boulder = false; }
                            if floor.plant == Plant::None && wobble % 11 == 0 { floor.plant = Plant::Shrub; }
                            if floor.plant == Plant::None && plan.site.destroyed_year.is_none() && wobble % 9 == 1 { floor.plant = Plant::Crop((wobble % 3) as u8); }
                            if plan.site.destroyed_year.is_none() && wobble % 4 != 0 && floor.plant == Plant::None { floor.material = Material::Soil; }
                        }
                    }
                    Feature::Door(ruined) => {
                        floor.material = if ruined { Material::Soil } else { Material::Wood };
                        floor.plant = Plant::None;
                        floor.boulder = false;
                    }
                    Feature::Field(parcel, row) => {
                        if matches!(floor.material, Material::Soil | Material::Clay | Material::Sand) {
                            floor.material = Material::Soil;
                            floor.boulder = false;
                            floor.plant = if row && parcel % 4 != 0 { Plant::Crop((parcel % 3) as u8) } else { Plant::None };
                        }
                    }
                    Feature::None => {}
                }
                done = true;
                break;
            }
            if done || underwater { continue; }
            // Roads between settlements.
            if roads.iter().any(|&(a, b)| seg_dist((x_m, y_m), a, b) < 2.2) {
                let floor = &mut map.cells[floor_k];
                if floor.shape == Shape::Floor || floor.shape == Shape::Ramp {
                    floor.material = Material::Gravel;
                    floor.plant = Plant::None;
                    floor.boulder = false;
                }
            }
        }
    }
    // Houses in three dimensions: walls to their height, upper floors, stairs, battlements,
    // cellars.
    for ((pi, hi), cells) in &footprints {
        if let Some(b) = raise(map, &plans[*pi], *hi, cells) { map.buildings.push(b); }
    }
    // Roofs: each house's ridge runs along the long axis of its footprint (principal axis of
    // its cells), half its width to either side.
    for (_, (stone, flat, cells)) in roofed {
        if cells.len() < 4 { continue; }
        let k = cells.len() as f32;
        let (mx, my) = cells.iter().fold((0.0, 0.0), |a, &(i, j)| (a.0 + i as f32 + 0.5, a.1 + j as f32 + 0.5));
        let (mx, my) = (mx / k, my / k);
        let (mut sxx, mut syy, mut sxy) = (0.0f32, 0.0f32, 0.0f32);
        for &(i, j) in &cells {
            let (dx, dy) = (i as f32 + 0.5 - mx, j as f32 + 0.5 - my);
            sxx += dx * dx; syy += dy * dy; sxy += dx * dy;
        }
        let ang = 0.5 * (2.0 * sxy).atan2(sxx - syy);
        let axis = (ang.cos(), ang.sin());
        let half_width = cells.iter().map(|&(i, j)| ((i as f32 + 0.5 - mx) * -axis.1 + (j as f32 + 0.5 - my) * axis.0).abs()).fold(0.0, f32::max) + 0.5;
        map.houses.push(crate::local::RoofPlan { cx: mx, cy: my, axis, half_width, stone, flat });
        let id = map.houses.len() as u32;
        for (i, j) in cells { map.roofs[j * n + i] = id; }
    }
}

// ---------------------------------------------------------------------------------------------
// Buildings in three dimensions (DF's site realizations)
// ---------------------------------------------------------------------------------------------

/// What a house's cell is: a wall, the floor inside, or the doorway.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Part { Wall, Floor, Door }

/// A cell of a house's footprint: the embark column, what it is, where it lies in the house's
/// own axes (m; the door on the -v side) and its absolute 2 m tile (for hashing).
struct HouseCell { i: usize, j: usize, part: Part, lu: f64, lv: f64, gx: i64, gy: i64 }

/// A point (frame metres) in a house's own axes.
fn local_uv(h: &House, u: f64, v: f64) -> (f64, f64) {
    let (dx, dy) = (u - h.u, v - h.v);
    let (s, c) = h.ang.sin_cos();
    (dx * c + dy * s, -dx * s + dy * c)
}

/// How a house is raised, in levels over its ground floor: the top of its walls, the floors laid
/// above (a storey is two levels: its floor, then standing room), a walkable roof, whether the
/// upper floor is a loft over the back half, and its cellar (levels down, the room's reach in
/// cells from the stair across and along, whether the room may run past the walls).
struct Design { wall_top: i32, floors: Vec<i32>, platform: Option<i32>, loft: bool, cellar: Option<(i32, (i32, i32), bool)> }

fn design(plan: &Plan, hi: usize) -> Design {
    let h = &plan.houses[hi];
    let site = &plan.site;
    let hh = hash(site.id.0 as i64, hi as i64, 0x5707);
    let big = matches!(site.kind, SettlementType::Capital | SettlementType::City);
    let townish = !matches!(site.kind, SettlementType::Camp | SettlementType::Outpost);
    let storeys = |n: i32| (2 * n - 1, (1..n).map(|k| 2 * k).collect::<Vec<i32>>());
    match h.kind {
        BuildingKind::Keep => {
            // Two storeys in a fort, three in a capital, and the roof behind battlements.
            let (wall_top, floors) = storeys(if site.kind == SettlementType::Capital { 3 } else { 2 });
            Design { wall_top, platform: Some(wall_top + 1), floors, loft: false, cellar: Some((3, (5, 5), site.carved)) }
        }
        // A level above the wall's walk (the wall is two high).
        BuildingKind::Tower => Design { wall_top: 2, floors: Vec::new(), platform: Some(3), loft: false, cellar: None },
        BuildingKind::Hall | BuildingKind::House => {
            let area = h.hu * h.hv;
            let hall = h.kind == BuildingKind::Hall;
            // Upper storeys for stone: most houses in a city, the larger half in a town.
            let village = matches!(site.kind, SettlementType::Village | SettlementType::Camp | SettlementType::Outpost);
            let two = h.stone && !village && ((hall && big) || (big && hh % 3 != 0) || (area >= 20.0 && hh % 2 == 0));
            let loft = !two && !h.stone && area >= 20.0 && hh % 3 == 0;
            let (wall_top, floors) = if two { storeys(2) } else if loft { (2, vec![2]) } else { (1, Vec::new()) };
            // A carving people's run past the walls, two to three cells each way from the stair.
            let cellar = if site.carved && (hall || (hh >> 8) % 2 == 0) { Some((3, (2 + ((hh >> 12) % 2) as i32, 2 + ((hh >> 13) % 2) as i32), true)) }
                else if townish && (hall || (hh >> 8) % 4 == 0) { let r = if area >= 24.0 { 2 } else { 1 }; Some((2, (r, r), false)) }
                else { None };
            Design { wall_top, floors, platform: None, loft, cellar }
        }
    }
}

/// Dressed stone for walls and floors (as the streets).
const DRESSED: crate::erosion::materials::RockType = crate::erosion::materials::RockType::Granite;

/// Raise a house from its footprint: walls to its height, the floors above, the stair, a
/// keep's or tower's roof and battlements, the cellar. A ruin keeps stumps of wall, a few
/// fragments of its upper floors over rubble, the foot of its stair, and its cellar whole.
fn raise(map: &mut LocalMap, plan: &Plan, hi: usize, cells: &[HouseCell]) -> Option<Building> {
    let h = &plan.houses[hi];
    let n = map.width;
    let depth = map.depth as i32;
    let ruined = plan.site.destroyed_year.is_some();
    let d = design(plan, hi);
    let sz = |map: &LocalMap, i: usize, j: usize| map.surface_z[j * n + i];
    let base = cells.iter().map(|c| sz(map, c.i, c.j)).max()?;
    let wall_mat = if h.stone { Material::Block(DRESSED) } else { match plan.site.style { BuildStyle::Earth => Material::Clay, _ => Material::Wood } };
    let floor_mat = if h.stone { Material::Block(DRESSED) } else { Material::Wood };
    let set = |map: &mut LocalMap, i: usize, j: usize, z: i32, shape: Shape, material: Material| {
        if z >= 0 && z < depth {
            let k = map.idx(i, j, z as usize);
            map.cells[k] = Cell { shape, material, water: 0, plant: Plant::None, boulder: false };
        }
    };
    // The stair: the floor's back corner, away from the door (left or right by the house).
    // It needs floor beside it on every level (a house turned to its street can pinch its
    // corners to a cell between walls): without one the house keeps to a single storey.
    let side = if hash(plan.site.id.0 as i64, hi as i64, 0x57A1) % 2 == 0 { 1.0 } else { -1.0 };
    let inside: std::collections::HashSet<(usize, usize)> = cells.iter().filter(|c| c.part == Part::Floor).map(|c| (c.i, c.j)).collect();
    let open_beside = |c: &&HouseCell| [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)].iter()
        .filter(|&&(dx, dy)| inside.contains(&((c.i as i64 + dx) as usize, (c.j as i64 + dy) as usize))).count() >= 2;
    let stair = cells.iter().filter(|c| c.part == Part::Floor).filter(open_beside)
        .max_by_key(|c| ((c.lv / TILE_M as f64).round() as i64, (c.lu * side / TILE_M as f64).round() as i64, c.j, c.i))
        .map(|c| (c.i, c.j));
    let d = if stair.is_some() { d } else { Design { wall_top: 1, floors: Vec::new(), platform: None, loft: false, cellar: None } };
    // Walls, to their full height; in a ruin some stretches stand (lower, mostly), the rest is
    // rubble on the ground.
    let mut wall_top: std::collections::HashMap<(usize, usize), i32> = Default::default();
    for c in cells.iter().filter(|c| c.part != Part::Floor) {
        let s = sz(map, c.i, c.j);
        let wobble = hash(c.gx, c.gy, 31);
        let top = if !ruined { base + d.wall_top } else if wobble % 100 < 40 {
            let u = unit(hash(c.gx, c.gy, 0x7A11));
            base + 1 + (u * u * d.wall_top as f64) as i32
        } else { s };
        if top <= s {
            let k = map.idx(c.i, c.j, s as usize);
            map.cells[k].boulder = wobble % 2 == 0;
            continue;
        }
        // Over a doorway, the lintel and the wall above it (fallen in a ruin).
        if c.part == Part::Door && ruined { continue; }
        let from = if c.part == Part::Door { s + 2 } else { s + 1 };
        for z in from..=top { set(map, c.i, c.j, z, Shape::Wall, wall_mat); }
        wall_top.insert((c.i, c.j), top);
    }
    // Rubble of the fallen storeys on a ruin's ground floor.
    if ruined && d.wall_top > 1 {
        for c in cells.iter().filter(|c| c.part == Part::Floor && Some((c.i, c.j)) != stair) {
            if hash(c.gx, c.gy, 0x2B) % 3 == 0 {
                let k = map.idx(c.i, c.j, sz(map, c.i, c.j) as usize);
                map.cells[k].boulder = true;
                map.cells[k].plant = Plant::None;
            }
        }
    }
    // Floors laid over the rooms (a loft over the back half); in a ruin only fragments of stone
    // floors still held by a standing wall.
    let mut upper = Vec::new();
    for (fi, &f) in d.floors.iter().enumerate() {
        let z = base + f;
        let mut laid = 0;
        for c in cells.iter().filter(|c| c.part == Part::Floor && Some((c.i, c.j)) != stair) {
            if d.loft && c.lv < 0.0 { continue; }
            if ruined {
                let held = [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)].iter().any(|&(dx, dy)| {
                    let q = ((c.i as i64 + dx) as usize, (c.j as i64 + dy) as usize);
                    wall_top.get(&q).map(|&t| t >= z).unwrap_or(false)
                });
                if !(h.stone && held && hash(c.gx, c.gy, 0x51AB + fi as u64) % 4 == 0) { continue; }
            }
            set(map, c.i, c.j, z, Shape::Floor, floor_mat);
            laid += 1;
        }
        if laid > 0 && z < depth { upper.push(z); }
    }
    // The walkable roof: dressed flags over the whole footprint, merlons on every other cell of
    // the walls (the gaps between them stood in to look out).
    let platform = if ruined { None } else { d.platform.map(|p| base + p).filter(|&z| z + 1 < depth) };
    if let Some(z) = platform {
        for c in cells {
            if Some((c.i, c.j)) == stair { continue; }
            set(map, c.i, c.j, z, Shape::Floor, floor_mat);
            if c.part != Part::Floor && (c.i + c.j) % 2 == 0 { set(map, c.i, c.j, z + 1, Shape::Wall, wall_mat); }
        }
    }
    // The stair up to the highest floor or the roof; a ruin's broken off after its first step.
    if let (Some((si, sj)), Some(top)) = (stair, d.platform.or(d.floors.last().copied())) {
        let s = sz(map, si, sj);
        let top = if ruined { s + 1 } else { (base + top).min(depth - 2) };
        for z in s + 1..=top { set(map, si, sj, z, Shape::Stair, floor_mat); }
    }
    // The cellar, reached by the same stair (DF's up/down stair).
    let cellar = match (d.cellar, stair) {
        (Some((down, reach, beyond)), Some(s)) => {
            cut_cellar(map, s, down, reach, beyond, &inside)
        }
        _ => Vec::new(),
    };
    Some(Building {
        kind: h.kind, stone: h.stone, ruined,
        storeys: if d.loft { 1 } else { 1 + d.floors.len() as u8 },
        loft: d.loft, ground: base,
        stair: stair.filter(|_| !d.floors.is_empty() || d.platform.is_some() || !cellar.is_empty()).map(|(i, j)| (i as u16, j as u16)),
        upper, platform, cellar,
    })
}

/// Cut a cellar `down` levels under the stair's column `s`: the stair down from the house's
/// floor, and a room round its foot (cells within `reach` (x, y) of the stair, under the house
/// unless `beyond`), each under a roof of at least one level of ground, so the ground above
/// stays as it was.
/// Only whole ground is cut: no caverns, water, magma, wet aquifer rock or other cellars. Too
/// little room (fewer than three cells beside the stair) and nothing is cut.
fn cut_cellar(map: &mut LocalMap, s: (usize, usize), down: i32, reach: (i32, i32), beyond: bool, inside: &std::collections::HashSet<(usize, usize)>) -> Vec<((u16, u16), i32)> {
    let n = map.width;
    let sz = map.surface_z[s.1 * n + s.0];
    let fz = sz - down;
    if fz < 2 { return Vec::new(); }
    let whole = |map: &LocalMap, x: usize, y: usize, z: i32| {
        let c = map.cell(x, y, z as usize);
        c.shape == Shape::Wall && c.water == 0 && c.material != Material::Magma && map.cavern_at(x, y, z).is_none() && !map.is_aquifer(x, y, z as usize)
    };
    if !(fz..sz).all(|z| whole(map, s.0, s.1, z)) { return Vec::new(); }
    let mut ok: std::collections::HashSet<(usize, usize)> = Default::default();
    for dy in -reach.1..=reach.1 {
        for dx in -reach.0..=reach.0 {
            let (x, y) = (s.0 as i32 + dx, s.1 as i32 + dy);
            if (dx, dy) == (0, 0) || x < 2 || y < 2 || x >= n as i32 - 2 || y >= map.height as i32 - 2 { continue; }
            let (x, y) = (x as usize, y as usize);
            if !beyond && !inside.contains(&(x, y)) { continue; }
            let top = map.surface_z[y * n + x];
            if top < fz + 2 { continue; }
            // Floor, room and roof: whole ground (the roof may be the ground's own floor).
            let roof = map.cell(x, y, (fz + 2) as usize);
            let roof_ok = if fz + 2 < top { whole(map, x, y, fz + 2) } else { matches!(roof.shape, Shape::Floor | Shape::Ramp) && roof.water == 0 };
            // A wall of ground left between this and another cellar (or any space cut before).
            let apart = (-1i32..=1).all(|ey| (-1i32..=1).all(|ex| {
                let (qx, qy) = ((x as i32 + ex) as usize, (y as i32 + ey) as usize);
                (qx, qy) == s || map.cell(qx, qy, (fz + 1) as usize).shape != Shape::Empty
            }));
            if apart && whole(map, x, y, fz) && whole(map, x, y, fz + 1) && roof_ok { ok.insert((x, y)); }
        }
    }
    // The room: what of that is joined to the stair's foot.
    let mut room: Vec<(usize, usize)> = Vec::new();
    let mut todo = vec![s];
    let mut seen: std::collections::HashSet<(usize, usize)> = [s].into_iter().collect();
    while let Some(p) = todo.pop() {
        for (dx, dy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
            let q = ((p.0 as i32 + dx) as usize, (p.1 as i32 + dy) as usize);
            if ok.contains(&q) && seen.insert(q) { room.push(q); todo.push(q); }
        }
    }
    if room.len() < 3 { return Vec::new(); }
    room.sort_by_key(|&(x, y)| (y, x));
    for z in fz + 1..=sz {
        let k = map.idx(s.0, s.1, z as usize);
        map.cells[k].shape = Shape::Stair;
        map.cells[k].plant = Plant::None;
        map.cells[k].boulder = false;
    }
    let foot = map.idx(s.0, s.1, fz as usize);
    map.cells[foot].shape = Shape::Floor;
    for &(x, y) in &room {
        let k = map.idx(x, y, (fz + 1) as usize);
        map.cells[k] = Cell::AIR;
        let f = map.idx(x, y, fz as usize);
        map.cells[f].shape = Shape::Floor;
        map.cells[f].plant = Plant::None;
        map.cells[f].boulder = false;
    }
    std::iter::once(s).chain(room).map(|(x, y)| ((x as u16, y as u16), fz)).collect()
}
