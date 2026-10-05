//! History at region scale: where exactly each settlement sits inside its world tile, how big
//! it is, its fields, and the roads between settlements routed over the real terrain.
//!
//! Everything is a pure function of the world, history and region data, so the same
//! settlement lands on the same spot whichever region (or embark) asks for it.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use crate::history::det::HashMap;

use crate::history::civilizations::settlement::{SettlementType, WallLevel};
use crate::history::entities::culture::ArchitectureStyle;
use crate::history::world_state::WorldHistory;
use crate::history::SettlementId;
use crate::region::zoom::ZoomRegion;
use crate::world::WorldData;

/// Building material family, from the culture's architecture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildStyle {
    /// Dressed stone blocks.
    Stone,
    /// Timber.
    Wood,
    /// Rammed earth / mud brick.
    Earth,
}

/// A settlement placed in a region (positions in region cells).
#[derive(Clone, Debug)]
pub struct Site {
    pub id: SettlementId,
    pub name: String,
    pub kind: SettlementType,
    pub population: u32,
    pub destroyed_year: Option<u32>,
    pub founded_year: u32,
    pub walls: WallLevel,
    pub style: BuildStyle,
    /// Region cell coordinates of the centre (fractional).
    pub x: f64,
    pub y: f64,
    /// Radius of the built-up core and of the farmland, metres.
    pub core_m: f32,
    pub fields_m: f32,
    pub faction_name: String,
}

pub struct RegionLore {
    pub sites: Vec<Site>,
    /// Road polylines in region cell coordinates.
    pub roads: Vec<Vec<(f64, f64)>>,
    /// Per region cell: 0 nothing, 1 road, 2 field, 3 built-up, 4 rubble.
    pub cover: Vec<u8>,
    /// Wildlife density per species (`history::ecology::SPECIES`) for each world tile in and
    /// around the region.
    pub wildlife: crate::history::det::HashMap<(usize, usize), Vec<f32>>,
    /// Battles fought on each world tile in and around the region: (title, year, the named dead).
    pub battles: crate::history::det::HashMap<(usize, usize), Vec<(String, u32, Vec<String>)>>,
}

impl RegionLore {
    pub const ROAD: u8 = 1;
    pub const FIELD: u8 = 2;
    pub const BUILT: u8 = 3;
    pub const RUBBLE: u8 = 4;
}

fn hash(a: i64, b: i64, salt: u64) -> u64 {
    let mut h = (a as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (b as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ salt;
    h ^= h >> 31;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^ (h >> 29)
}

fn build_style(history: &WorldHistory, faction: crate::history::FactionId, kind: SettlementType) -> BuildStyle {
    let arch = history.factions.get(&faction)
        .and_then(|f| history.races.get(&f.race_id))
        .and_then(|r| history.cultures.get(&r.culture_id))
        .map(|c| c.architecture);
    match arch {
        Some(ArchitectureStyle::Stone | ArchitectureStyle::Carved | ArchitectureStyle::Metalwork | ArchitectureStyle::Crystal) => BuildStyle::Stone,
        Some(ArchitectureStyle::Earthen | ArchitectureStyle::Bone) => BuildStyle::Earth,
        Some(_) => BuildStyle::Wood,
        // Unknown culture: cities build in stone, smaller places in timber.
        None => if matches!(kind, SettlementType::Capital | SettlementType::City) { BuildStyle::Stone } else { BuildStyle::Wood },
    }
}

/// Built-up radius (m) for a population: ~85 m for a village of 200, ~1.3 km for 50,000.
pub fn core_radius_m(population: u32, kind: SettlementType) -> f32 {
    let p = population.max(60) as f32;
    let r = 60.0 * (p / 100.0).sqrt();
    let min = match kind {
        SettlementType::Capital | SettlementType::City => 250.0,
        SettlementType::Town | SettlementType::Port | SettlementType::Fort => 120.0,
        _ => 50.0,
    };
    r.clamp(min, 2000.0)
}

pub fn region_lore(world: &WorldData, history: &WorldHistory, region: &ZoomRegion) -> RegionLore {
    let s = (region.params.cells_per_tile.max(8) & !1) as i64;
    let (rw, rh) = (region.width, region.height);
    let tiles_x = rw as i64 / s;
    let cell_m = region.cell_m as f64;
    let ww = world.width as i64;
    // World tile -> region-local tile offset, if inside the window (+ a margin for roads).
    let local_tile = |tx: usize, ty: usize, margin: i64| -> Option<(i64, i64)> {
        let dx = (tx as i64 - region.world_x0).rem_euclid(ww);
        let dx = if dx > ww / 2 { dx - ww } else { dx };
        let dy = ty as i64 - region.world_y0;
        if dx >= -margin && dy >= -margin && dx < tiles_x + margin && dy < rh as i64 / s + margin { Some((dx, dy)) } else { None }
    };
    let land = |x: i64, y: i64| -> bool {
        if x < 0 || y < 0 || x >= rw as i64 || y >= rh as i64 { return false; }
        let k = y as usize * rw + x as usize;
        region.elevation_m[k] > 0.0 && region.lake_depth_m[k] == 0.0
    };
    let slope = |x: i64, y: i64| -> f32 {
        let at = |x: i64, y: i64| region.elevation_m[y.clamp(0, rh as i64 - 1) as usize * rw + x.clamp(0, rw as i64 - 1) as usize];
        let gx = (at(x + 1, y) - at(x - 1, y)) / (2.0 * cell_m as f32);
        let gy = (at(x, y + 1) - at(x, y - 1)) / (2.0 * cell_m as f32);
        (gx * gx + gy * gy).sqrt()
    };

    // Settlement sites: the best cell inside the settlement's world tile (flat, dry, by water).
    let mut sites = Vec::new();
    let mut site_cell: HashMap<(usize, usize), (f64, f64)> = HashMap::default();
    let mut list: Vec<_> = history.settlements.values().collect();
    list.sort_by_key(|st| st.id.0);
    for st in list {
        let (tx, ty) = st.location;
        let Some((lx, ly)) = local_tile(tx, ty, 0) else { continue };
        let (cx0, cy0) = (lx * s, ly * s);
        let core_probe = core_radius_m(st.population.max(if st.is_destroyed() { 200 } else { 0 }), st.settlement_type) as f64 * 1.1;
        let mut best: Option<(i64, i64, f32)> = None;
        let margin = s / 6;
        for y in cy0 + margin..cy0 + s - margin {
            for x in cx0 + margin..cx0 + s - margin {
                if !land(x, y) { continue; }
                let k = y as usize * rw + x as usize;
                let mut score = -slope(x, y) * 40.0;
                // The built-up area must be on dry land: sample a ring at the settlement's radius.
                let rc = ((core_probe / cell_m).max(1.5)).min(60.0);
                let mut dry = 0;
                for k in 0..16 {
                    let a = k as f64 / 16.0 * std::f64::consts::TAU;
                    if land((x as f64 + a.cos() * rc) as i64, (y as f64 + a.sin() * rc) as i64) { dry += 1; }
                }
                score -= (16 - dry) as f32 * 1.5;
                // Water nearby (river, lake, sea) within ~2 cells.
                let wet = (-2..=2).any(|dy| (-2..=2).any(|dx| {
                    let (nx, ny) = (x + dx, y + dy);
                    nx >= 0 && ny >= 0 && nx < rw as i64 && ny < rh as i64 && {
                        let j = ny as usize * rw + nx as usize;
                        region.river_width_m[j] > 0.0 || region.lake_depth_m[j] > 0.0 || region.elevation_m[j] <= 0.0
                    }
                }));
                if wet { score += 3.0; }
                if region.river_width_m[k] > 0.0 { score -= 2.0; } // not *in* the river
                score += (hash(x + region.world_x0 * s, y + region.world_y0 * s, st.id.0) % 1000) as f32 / 4000.0;
                if best.map(|b| score > b.2).unwrap_or(true) { best = Some((x, y, score)); }
            }
        }
        let Some((bx, by, _)) = best else { continue };
        let core_m = core_radius_m(st.population.max(if st.is_destroyed() { 200 } else { 0 }), st.settlement_type);
        let faction_name = history.factions.get(&st.faction).map(|f| f.name.clone()).unwrap_or_default();
        let site = Site {
            id: st.id,
            name: st.name.clone(),
            kind: st.settlement_type,
            population: st.population,
            destroyed_year: st.destroyed.map(|d| d.year),
            founded_year: st.founded.year,
            walls: st.walls,
            style: build_style(history, st.faction, st.settlement_type),
            x: bx as f64 + 0.5,
            y: by as f64 + 0.5,
            core_m,
            fields_m: if st.is_destroyed() { 0.0 } else { core_m * 2.5 + 400.0 },
            faction_name,
        };
        site_cell.insert((tx, ty), (site.x, site.y));
        sites.push(site);
    }

    // Cover: fields, built-up areas, rubble.
    let mut cover = vec![0u8; rw * rh];
    for site in &sites {
        let reach = (site.fields_m.max(site.core_m) as f64 / cell_m).ceil() as i64 + 1;
        for y in (site.y as i64 - reach).max(0)..=(site.y as i64 + reach).min(rh as i64 - 1) {
            for x in (site.x as i64 - reach).max(0)..=(site.x as i64 + reach).min(rw as i64 - 1) {
                if !land(x, y) { continue; }
                let d = ((x as f64 + 0.5 - site.x).powi(2) + (y as f64 + 0.5 - site.y).powi(2)).sqrt() * cell_m;
                let k = y as usize * rw + x as usize;
                if d <= (site.core_m as f64).max(cell_m * 0.5) {
                    cover[k] = if site.destroyed_year.is_some() { RegionLore::RUBBLE } else { RegionLore::BUILT };
                } else if d <= site.fields_m as f64 && slope(x, y) < 0.08 && region.river_width_m[k] == 0.0 && cover[k] == 0 {
                    // Fields thin out with distance.
                    let keep = 1.0 - (d / site.fields_m as f64).powi(2);
                    let r = (hash(x + region.world_x0 * s, y + region.world_y0 * s, 77) % 1000) as f64 / 1000.0;
                    if r < keep { cover[k] = RegionLore::FIELD; }
                }
            }
        }
    }

    // Roads: for each pair of neighbouring road tiles, route between their anchor points
    // (a settlement's site, else the tile centre) with A* over slope, avoiding water.
    let road_tile = |tx: usize, ty: usize| history.tile_history.get(tx, ty).has_road || history.tile_history.get(tx, ty).settlement.is_some();
    let anchor = |tx: usize, ty: usize| -> Option<(f64, f64)> {
        if let Some(&p) = site_cell.get(&(tx, ty)) { return Some(p); }
        let (lx, ly) = local_tile(tx, ty, 1)?;
        Some(((lx * s + s / 2) as f64, (ly * s + s / 2) as f64))
    };
    let mut roads = Vec::new();
    let (wx0, wy0) = (region.world_x0, region.world_y0);
    for ly in -1..=(rh as i64 / s) {
        for lx in -1..=tiles_x {
            let ty = wy0 + ly;
            if ty < 0 || ty >= world.height as i64 { continue; }
            let tx = (wx0 + lx).rem_euclid(ww) as usize;
            let ty = ty as usize;
            if !road_tile(tx, ty) { continue; }
            // Link east, south-east, south, south-west (each pair once).
            for (dx, dy) in [(1i64, 0i64), (1, 1), (0, 1), (-1, 1)] {
                let ny = ty as i64 + dy;
                if ny < 0 || ny >= world.height as i64 { continue; }
                let (nx, ny) = ((tx as i64 + dx).rem_euclid(ww) as usize, ny as usize);
                if !road_tile(nx, ny) { continue; }
                // Skip diagonals already joined through an orthogonal neighbour.
                if dx != 0 && dy != 0 {
                    let a = road_tile((tx as i64 + dx).rem_euclid(ww) as usize, ty);
                    let b = road_tile(tx, ny);
                    if a || b { continue; }
                }
                let (Some(a), Some(b)) = (anchor(tx, ty), anchor(nx, ny)) else { continue };
                if let Some(path) = route(region, a, b, s) {
                    for &(x, y) in &path {
                        if x >= 0.0 && y >= 0.0 && (x as usize) < rw && (y as usize) < rh {
                            let k = y as usize * rw + x as usize;
                            if cover[k] == 0 || cover[k] == RegionLore::FIELD { cover[k] = RegionLore::ROAD; }
                        }
                    }
                    roads.push(path);
                }
            }
        }
    }
    let mut wildlife = crate::history::det::HashMap::default();
    if let Some(eco) = &history.ecology {
        for dy in -1..=(rh as i64 / s + 1) {
            for dx in -1..=(tiles_x + 1) {
                let ty = region.world_y0 + dy;
                if ty < 0 || ty >= world.height as i64 { continue; }
                let tx = (region.world_x0 + dx).rem_euclid(ww) as usize;
                let i = ty as usize * world.width + tx;
                wildlife.insert((tx, ty as usize), eco.fauna.iter().map(|f| f[i]).collect());
            }
        }
    }
    // Battles on the region's tiles, with those who fell in them (for graves on the field).
    let mut battles: crate::history::det::HashMap<(usize, usize), Vec<(String, u32, Vec<String>)>> = Default::default();
    let tiles_y = rh as i64 / s + 1;
    for e in history.chronicle.events.iter().filter(|e| e.event_type == crate::history::events::types::EventType::BattleFought) {
        let Some((x, y)) = e.location else { continue };
        let dx = (x as i64 - region.world_x0).rem_euclid(ww);
        let dy = y as i64 - region.world_y0;
        if dx > tiles_x + 1 || dy < -1 || dy > tiles_y + 1 { continue; }
        let dead: Vec<String> = e.primary_participants.iter().filter_map(|p| match p {
            crate::history::EntityId::Figure(f) => history.figures.get(f).filter(|x| x.death_date == Some(e.date)).map(|x| x.full_name()),
            _ => None,
        }).collect();
        battles.entry((x, y)).or_default().push((e.title.clone(), e.date.year, dead));
    }
    RegionLore { sites, roads, cover, wildlife, battles }
}

/// Least-cost path between two points (region cells) favouring gentle ground; crossing a river
/// cell costs extra (a ford or bridge), lakes and sea are avoided. Returns cell centres.
fn route(region: &ZoomRegion, a: (f64, f64), b: (f64, f64), s: i64) -> Option<Vec<(f64, f64)>> {
    let (rw, rh) = (region.width as i64, region.height as i64);
    let pad = s / 2;
    let (x0, x1) = ((a.0.min(b.0) as i64 - pad).max(0), (a.0.max(b.0) as i64 + pad).min(rw - 1));
    let (y0, y1) = ((a.1.min(b.1) as i64 - pad).max(0), (a.1.max(b.1) as i64 + pad).min(rh - 1));
    if x0 > x1 || y0 > y1 { return None; } // both ends lie beyond the same edge
    let clampc = |p: (f64, f64)| ((p.0 as i64).clamp(x0, x1), (p.1 as i64).clamp(y0, y1));
    let (sa, sb) = (clampc(a), clampc(b));
    let bw = (x1 - x0 + 1) as usize;
    let bh = (y1 - y0 + 1) as usize;
    let idx = |x: i64, y: i64| (y - y0) as usize * bw + (x - x0) as usize;
    let cell_m = region.cell_m;
    let cost = |x: i64, y: i64, nx: i64, ny: i64| -> Option<u32> {
        let k = ny as usize * rw as usize + nx as usize;
        if region.lake_depth_m[k] > 0.0 || region.elevation_m[k] <= 0.0 { return None; }
        let rise = (region.elevation_m[k] - region.elevation_m[y as usize * rw as usize + x as usize]).abs() / cell_m;
        let diag = if nx != x && ny != y { 1.414 } else { 1.0 };
        let river = if region.river_width_m[k] > 0.0 { 6.0 } else { 0.0 };
        Some(((diag * (1.0 + 60.0 * rise) + river) * 100.0) as u32)
    };
    let mut dist = vec![u32::MAX; bw * bh];
    let mut prev = vec![u32::MAX; bw * bh];
    let mut heap = BinaryHeap::new();
    dist[idx(sa.0, sa.1)] = 0;
    heap.push(Reverse((0u32, sa.0, sa.1)));
    while let Some(Reverse((d, x, y))) = heap.pop() {
        if (x, y) == sb { break; }
        if d > dist[idx(x, y)] { continue; }
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 { continue; }
                let (nx, ny) = (x + dx, y + dy);
                if nx < x0 || ny < y0 || nx > x1 || ny > y1 { continue; }
                let Some(c) = cost(x, y, nx, ny) else { continue };
                let nd = d + c;
                if nd < dist[idx(nx, ny)] {
                    dist[idx(nx, ny)] = nd;
                    prev[idx(nx, ny)] = idx(x, y) as u32;
                    heap.push(Reverse((nd, nx, ny)));
                }
            }
        }
    }
    if dist[idx(sb.0, sb.1)] == u32::MAX { return None; }
    let mut path = Vec::new();
    let mut cur = idx(sb.0, sb.1);
    loop {
        let (x, y) = ((cur % bw) as i64 + x0, (cur / bw) as i64 + y0);
        path.push((x as f64 + 0.5, y as f64 + 0.5));
        if cur == idx(sa.0, sa.1) { break; }
        cur = prev[cur] as usize;
        if cur == u32::MAX as usize { return None; }
    }
    path.reverse();
    Some(path)
}

/// Paint fields, settlements and roads onto a rendered region. At region scale a town is
/// smaller than a cell, so settlements are drawn as map symbols: a filled disc (size by rank)
/// with a dark rim, a wall ring for walled places, and a hollow grey ring for ruins.
pub fn paint_region(lore: &RegionLore, region: &ZoomRegion, rgb: &mut [[u8; 3]]) {
    let (rw, rh) = (region.width, region.height);
    let s = (region.params.cells_per_tile.max(8) & !1) as i64;
    let mix = |a: [u8; 3], b: [u8; 3], t: f32| -> [u8; 3] {
        [(a[0] as f32 * (1.0 - t) + b[0] as f32 * t) as u8, (a[1] as f32 * (1.0 - t) + b[1] as f32 * t) as u8, (a[2] as f32 * (1.0 - t) + b[2] as f32 * t) as u8]
    };
    const FIELD_COLORS: [[u8; 3]; 4] = [[214, 190, 90], [150, 176, 70], [176, 130, 82], [196, 196, 100]];
    for (k, &c) in lore.cover.iter().enumerate() {
        if c != RegionLore::FIELD { continue; }
        let (x, y) = ((k % rw) as i64, (k / rw) as i64);
        let (gx, gy) = (x + region.world_x0 * s, y + region.world_y0 * s);
        // Parcels: strips with their own crop colour, alternating furrow shade.
        let parcel = hash(gx.div_euclid(3), gy.div_euclid(5), 5);
        let furrow = if (gx + gy) % 2 == 0 { 0.9 } else { 1.0 };
        let col = FIELD_COLORS[(parcel % 4) as usize];
        rgb[k] = mix(rgb[k], [(col[0] as f32 * furrow) as u8, (col[1] as f32 * furrow) as u8, (col[2] as f32 * furrow) as u8], 0.75);
    }
    // Roads: two cells wide with a lighter centre, so they read at full-region zoom.
    for road in &lore.roads {
        for w in road.windows(2) {
            let steps = (((w[1].0 - w[0].0).abs().max((w[1].1 - w[0].1).abs())) * 2.0).ceil().max(1.0) as i32;
            for i in 0..=steps {
                let t = i as f64 / steps as f64;
                let (x, y) = (w[0].0 + (w[1].0 - w[0].0) * t, w[0].1 + (w[1].1 - w[0].1) * t);
                for (ox, oy, a) in [(0.0, 0.0, 0.95f32), (1.0, 0.0, 0.6), (0.0, 1.0, 0.6), (-1.0, 0.0, 0.35), (0.0, -1.0, 0.35)] {
                    let (px, py) = ((x + ox) as i64, (y + oy) as i64);
                    if px < 0 || py < 0 || px >= rw as i64 || py >= rh as i64 { continue; }
                    let k = py as usize * rw + px as usize;
                    if region.elevation_m[k] > 0.0 && region.lake_depth_m[k] == 0.0 {
                        rgb[k] = mix(rgb[k], [196, 160, 108], a);
                    }
                }
            }
        }
    }
    // Settlements.
    for site in &lore.sites {
        let r = match site.kind {
            SettlementType::Capital => 6.0,
            SettlementType::City | SettlementType::Port => 5.0,
            SettlementType::Town | SettlementType::Fort => 4.0,
            _ => 3.0,
        };
        let ruined = site.destroyed_year.is_some();
        let (cx, cy) = (site.x, site.y);
        let reach = r as i64 + 2;
        for dy in -reach..=reach {
            for dx in -reach..=reach {
                let (px, py) = (cx as i64 + dx, cy as i64 + dy);
                if px < 0 || py < 0 || px >= rw as i64 || py >= rh as i64 { continue; }
                let d = (((px as f64 + 0.5 - cx).powi(2) + (py as f64 + 0.5 - cy).powi(2)).sqrt()) as f32;
                let k = py as usize * rw + px as usize;
                let walled = site.walls != WallLevel::None && !ruined && r >= 4.0;
                if ruined {
                    if d > r - 1.0 && d <= r { rgb[k] = [120, 114, 106]; }
                    else if d <= r - 1.0 { rgb[k] = mix(rgb[k], [96, 92, 86], 0.35); }
                } else if d <= r - 1.2 {
                    rgb[k] = if (px + py) % 2 == 0 { [214, 96, 64] } else { [236, 214, 170] };
                } else if d <= r {
                    rgb[k] = if walled { [232, 228, 218] } else { [40, 30, 24] };
                } else if d <= r + 1.0 {
                    rgb[k] = mix(rgb[k], [20, 16, 12], 0.7);
                }
            }
        }
    }
}
