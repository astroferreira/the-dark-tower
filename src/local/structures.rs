//! Settlements, ruins, fields and roads at person scale.
//!
//! Each settlement gets a deterministic town plan in metres around its site (seeded by its
//! id, so every embark that touches it sees the same town): streets out along the directions
//! its roads arrive from, house lots along the streets built in the culture's material, a keep
//! and plaza for cities, a wall ring with gates for walled towns, and strip fields around it.
//! A ruin uses the same plan, but most walls have fallen, floors are rubble and plants are
//! taking it back.

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

/// A rectangular building in metres (site-local frame, rotated by the plan angle).
struct House {
    /// Centre in the plan's rotated frame (m).
    u: f64,
    v: f64,
    half_u: f64,
    half_v: f64,
    /// Which side has the door: 0 = -v, 1 = +u, 2 = +v, 3 = -u.
    door: u8,
    stone: bool,
}

struct Plan {
    site: Site,
    /// Site centre in metres (region frame).
    cx_m: f64,
    cy_m: f64,
    angle: f64,
    /// Streets as segments in the rotated frame (m), with half-widths.
    streets: Vec<((f64, f64), (f64, f64), f64)>,
    houses: Vec<House>,
    wall_r: Option<(f64, f64)>,
    plaza_r: f64,
}

impl Plan {
    fn to_frame(&self, x_m: f64, y_m: f64) -> (f64, f64) {
        let (dx, dy) = (x_m - self.cx_m, y_m - self.cy_m);
        let (s, c) = self.angle.sin_cos();
        (dx * c + dy * s, -dx * s + dy * c)
    }
}

fn plan_for(site: &Site, lore: &RegionLore, cell_m: f64) -> Plan {
    let id = site.id.0 as i64;
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
    let angle = dirs.first().copied().unwrap_or(unit(hash(id, 1, 3)) * std::f64::consts::PI);
    let core = site.core_m as f64;
    let mut plan = Plan { site: site.clone(), cx_m, cy_m, angle, streets: Vec::new(), houses: Vec::new(), wall_r: None, plaza_r: 0.0 };

    // Main street along the plan axis, a cross street, and one street per extra road.
    let main_w = if core > 200.0 { 3.0 } else { 2.0 };
    plan.streets.push(((-core * 1.05, 0.0), (core * 1.05, 0.0), main_w));
    plan.streets.push(((0.0, -core * 0.8), (0.0, core * 0.8), main_w * 0.75));
    for d in dirs.iter().skip(1) {
        let a = d - angle;
        plan.streets.push(((0.0, 0.0), (a.cos() * core * 1.05, a.sin() * core * 1.05), main_w));
    }
    // Back lanes parallel to the main street in larger places.
    let mut lane = 32.0;
    while lane < core * 0.8 {
        for sign in [-1.0, 1.0] {
            let len = (core * core - lane * lane).max(0.0).sqrt();
            plan.streets.push(((-len, sign * lane), (len, sign * lane), 1.5));
        }
        lane += 32.0;
    }
    let big = matches!(site.kind, SettlementType::Capital | SettlementType::City);
    plan.plaza_r = if big { 18.0 } else if core > 100.0 { 9.0 } else { 0.0 };
    if site.walls != WallLevel::None && core >= 100.0 {
        let thick = if site.walls == WallLevel::Palisade { 1.0 } else { 3.0 };
        plan.wall_r = Some((core * 1.02, thick));
    }

    // House lots on a jittered grid; keep those fronting a street.
    let step = 13.0;
    let n = (core / step).ceil() as i64 + 1;
    for gy in -n..=n {
        for gx in -n..=n {
            let hsh = hash(gx + id * 7919, gy, 11);
            let u = gx as f64 * step + (unit(hsh) - 0.5) * 3.0;
            let v = gy as f64 * step + (unit(hsh >> 7) - 0.5) * 3.0;
            let r = (u * u + v * v).sqrt();
            let edge = core * (0.8 + 0.35 * unit(hash(gx, gy, id as u64)));
            if r > edge || r < plan.plaza_r + 6.0 { continue; }
            let half_u = 3.0 + (unit(hsh >> 13) * 3.0).floor();
            let half_v = 3.0 + (unit(hsh >> 19) * 3.0).floor();
            // Distance from the house edge to the nearest street, and that street's side.
            let mut best = (f64::MAX, 0u8);
            for &(a, b, w) in &plan.streets {
                let d = seg_dist((u, v), a, b) - w;
                if d < best.0 {
                    // Door faces the street: compare the closest point's offset.
                    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                    let l2 = dx * dx + dy * dy;
                    let t = if l2 > 0.0 { (((u - a.0) * dx + (v - a.1) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
                    let (px, py) = (a.0 + dx * t - u, a.1 + dy * t - v);
                    let side = if px.abs() > py.abs() { if px > 0.0 { 1 } else { 3 } } else if py > 0.0 { 2 } else { 0 };
                    best = (d, side);
                }
            }
            let clearance = best.0 - half_u.max(half_v);
            if clearance < 1.0 || clearance > 9.0 { continue; }
            if let Some((wr, _)) = plan.wall_r {
                if r + half_u.max(half_v) > wr - 4.0 { continue; }
            }
            let stone = match site.style {
                BuildStyle::Stone => true,
                BuildStyle::Earth => false,
                BuildStyle::Wood => big && r < core * 0.4,
            };
            plan.houses.push(House { u, v, half_u, half_v, door: best.1, stone });
        }
    }
    // A keep at the centre of capitals and forts.
    if matches!(site.kind, SettlementType::Capital | SettlementType::Fort) {
        plan.houses.push(House { u: 0.0, v: -plan.plaza_r - 12.0, half_u: 10.0, half_v: 8.0, door: 2, stone: true });
    }
    plan
}

/// What a plan puts at a point (frame metres).
enum Feature { Street, Plaza, Wall(bool), HouseWall(bool, bool), HouseFloor(bool, bool), Door(bool), Field(u64, bool), None }

fn feature_at(plan: &Plan, u: f64, v: f64) -> Feature {
    let ruined = plan.site.destroyed_year.is_some();
    for h in &plan.houses {
        let (du, dv) = (u - h.u, v - h.v);
        if du.abs() > h.half_u || dv.abs() > h.half_v { continue; }
        let on_wall = du.abs() > h.half_u - TILE_M as f64 || dv.abs() > h.half_v - TILE_M as f64;
        if !on_wall { return Feature::HouseFloor(h.stone, ruined); }
        let door = match h.door {
            0 => dv < -h.half_v + TILE_M as f64 && du.abs() < 1.0,
            1 => du > h.half_u - TILE_M as f64 && dv.abs() < 1.0,
            2 => dv > h.half_v - TILE_M as f64 && du.abs() < 1.0,
            _ => du < -h.half_u + TILE_M as f64 && dv.abs() < 1.0,
        };
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
    for &(a, b, w) in &plan.streets {
        let reach = if plan.wall_r.is_some() || r < plan.site.core_m as f64 * 1.1 { 1.0 } else { 0.0 };
        if reach > 0.0 && seg_dist((u, v), a, b) < w { return Feature::Street; }
    }
    if !ruined && r > plan.site.core_m as f64 && r < plan.site.fields_m as f64 {
        // Strip fields: long parcels across the plan axis, alternate fallow and crops.
        let parcel = hash((u / 40.0).floor() as i64, (v / 90.0).floor() as i64, plan.site.id.0);
        let keep = 1.0 - (r / plan.site.fields_m as f64).powi(2);
        if unit(parcel) < keep * 0.9 {
            let row = ((u / TILE_M as f64).floor() as i64).rem_euclid(2) == 0;
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
            for plan in &plans {
                let (u, v) = plan.to_frame(x_m, y_m);
                if u.abs() > plan.site.fields_m.max(plan.site.core_m) as f64 * 1.1 + 5.0 || v.abs() > plan.site.fields_m.max(plan.site.core_m) as f64 * 1.1 + 5.0 { continue; }
                let f = feature_at(plan, u, v);
                if matches!(f, Feature::None) { continue; }
                // Streets, houses and fields stop at the water's edge.
                if underwater { continue; }
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
                    Feature::HouseWall(stone, _) | Feature::Wall(stone) => {
                        floor.plant = Plant::None;
                        floor.boulder = false;
                        // In ruins only some stretches of wall still stand; the rest is rubble.
                        let standing = plan.site.destroyed_year.is_none() || wobble % 100 < 40;
                        if !standing {
                            floor.boulder = wobble % 2 == 0;
                        } else {
                            let storeys = if matches!(f, Feature::Wall(_)) && plan.site.walls != WallLevel::Palisade { 2 } else { 1 };
                            for dz in 1..=storeys {
                                if sz + dz < map.depth {
                                    let k = map.idx(i, j, sz + dz);
                                    map.cells[k] = Cell { shape: Shape::Wall, material: block(stone), water: 0, plant: Plant::None, boulder: false };
                                }
                            }
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
}
