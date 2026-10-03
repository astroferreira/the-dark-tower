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

    // Houses front the streets: walk along each, both sides, with variable lot width.
    let stone_of = |r: f64| match site.style {
        BuildStyle::Stone => true,
        BuildStyle::Earth => false,
        BuildStyle::Wood => big && r < core * 0.35,
    };
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
                let house = House { u: cu, v: cv, ang: if side > 0.0 { ang } else { ang + std::f64::consts::PI }, hu, hv, stone: stone_of(r), reach: hu.hypot(hv) };
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
    // A keep on the plaza in capitals and forts; a hall in towns.
    if matches!(site.kind, SettlementType::Capital | SettlementType::Fort) {
        let ang = 0.0;
        plan.houses.push(House { u: 0.0, v: -plan.plaza_r - 14.0, ang, hu: 14.0, hv: 10.0, stone: true, reach: 17.2 });
    } else if big || site.kind == SettlementType::Town {
        plan.houses.push(House { u: 0.0, v: plan.plaza_r + 9.0, ang: 0.0, hu: 10.0, hv: 6.0, stone: stone_of(0.0), reach: 11.7 });
    }
    plan
}

/// What a plan puts at a point (frame metres).
enum Feature { Street, Plaza, Wall(bool), HouseWall(bool, bool), HouseFloor(bool, bool), Door(bool), Field(u64, bool), Garden, None }

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
}
