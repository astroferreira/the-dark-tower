//! Graphical tile viewer window (minifb). All drawing lives in `render`; this file is input
//! handling and state.
//!
//! Two modes: the world map (tiles), and walking through a zoomed, re-simulated region. While
//! walking, the next region is generated on a background thread as the player nears an edge
//! and swapped in when ready. Zoomed terrain depends only on world position, so the swap is
//! invisible and the player can walk across the world region by region.

use std::error::Error;

use minifb::{Key, KeyRepeat, MouseButton, MouseMode, Window, WindowOptions};

use crate::history::world_state::WorldHistory;
use crate::lore::{build_gazetteer, FeatureKind, Gazetteer, Landmark};
use crate::region::zoom::{generate_zoom, ZoomParams, ZoomRegion};
use super::text::{place_labels, Label, LabelStyle};
use crate::world::WorldData;

use super::atlas::Atlas;
use super::overlays::{self, Overlay};
use super::classify::TileWorld;
use super::render::{render_local, render_minimap, render_world, render_zoom, screen_to_world, Camera, LocalCamera, ZoomCamera};
use crate::local::{generate_local, LocalMap, Plant, Shape, LOCAL_SIZE, TILE_M};

const MIN_TILE_PX: f32 = 1.0;
/// Seconds per season when the automatic year cycle is on.
const SEASON_SECONDS: f32 = 3.0;
const MAX_TILE_PX: f32 = 64.0;
/// Start prefetching the next region when the player is this many world tiles from an edge.
const PREFETCH_EDGE_TILES: f64 = 2.5;
/// Walking speed in screen pixels per frame (Shift = run).
const WALK_PX_PER_FRAME: f64 = 3.0;
const RUN_PX_PER_FRAME: f64 = 12.0;

/// A loaded high-resolution region.
struct ZoomState {
    region: ZoomRegion,
    rgb: Vec<[u8; 3]>,
    /// Global cell coordinate of the region's local cell (0, 0).
    origin: (i64, i64),
    /// World tile the region is centred on.
    tile: (usize, usize),
    /// Settlements, fields and roads placed in this region.
    lore: Option<crate::lore::RegionLore>,
}

/// Keep the poles at the window's edge: the map fills the window top to bottom when it can.
fn clamp_cy(cy: f32, tile_px: f32, h_px: usize, world_h: usize) -> f32 {
    let half = h_px as f32 / 2.0 / tile_px;
    let hgt = world_h as f32;
    if half * 2.0 >= hgt { hgt / 2.0 } else { cy.clamp(half, hgt - half) }
}

fn cells_per_tile() -> i64 {
    (ZoomParams::default().cells_per_tile.max(8) & !1) as i64
}

/// Generate (or assemble from cached chunks) the region centred on `tile`, with history
/// (settlements, fields, roads) painted on when available.
fn load_region(world: &WorldData, history: Option<&WorldHistory>, tile: (usize, usize), seed: u64) -> ZoomState {
    let params = ZoomParams { center_x: tile.0, center_y: tile.1, seed, ..ZoomParams::default() };
    let region = generate_zoom(world, &params);
    let mut rgb = region.render_rgb();
    let lore = history.map(|h| crate::lore::region_lore(world, h, &region));
    if let Some(l) = &lore { crate::lore::paint_region(l, &region, &mut rgb); }
    let s = cells_per_tile();
    let origin = (region.world_x0 * s, region.world_y0 * s);
    ZoomState { region, rgb, origin, tile, lore }
}

/// Names of the settlements in a zoomed region.
fn draw_region_labels(l: &crate::lore::RegionLore, cam: &ZoomCamera, buf: &mut [u32], w: usize, h: usize) {
    use crate::history::civilizations::settlement::SettlementType;
    let labels: Vec<Label> = l.sites.iter().map(|site| {
        let (rank, min_px, color) = match (site.destroyed_year.is_some(), site.kind) {
            (true, _) => (300, 1.2, 0x0078_6A58),
            (_, SettlementType::Capital) => (900, 0.0, 0x0030_1E14),
            (_, SettlementType::City | SettlementType::Port) => (850, 0.0, 0x0030_1E14),
            (_, SettlementType::Town | SettlementType::Fort) => (700, 0.4, 0x0030_1E14),
            _ => (400, 0.9, 0x004A_3624),
        };
        let text = if site.destroyed_year.is_some() { format!("ruins of {}", site.name) } else { site.name.clone() };
        let style = match (site.destroyed_year.is_some(), site.kind) {
            (true, _) => LabelStyle::Ruin,
            (_, SettlementType::Capital) => LabelStyle::Capital,
            (_, SettlementType::City | SettlementType::Port) => LabelStyle::City,
            _ => LabelStyle::Town,
        };
        Label { x: site.x as f32, y: site.y as f32 + 9.0, text, rank, min_tile_px: min_px, color, style }
    }).collect();
    place_labels(&labels, cam.px_per_cell, w, h, buf, &[], |x, y| {
        (w as f32 / 2.0 + (x - cam.cx) * cam.px_per_cell, h as f32 / 2.0 + (y - cam.cy) * cam.px_per_cell)
    });
}

/// Everything named on the world map: geographic features and settlements, highest rank first.
fn build_labels(world: &WorldData, history: Option<&WorldHistory>, gaz: &Gazetteer, landmarks: &[Landmark]) -> Vec<Label> {
    use crate::history::civilizations::settlement::SettlementType;
    let mut labels = Vec::new();
    for f in &gaz.features {
        let (min_tile_px, color): (f32, u32) = match f.kind {
            FeatureKind::Ocean | FeatureKind::Continent => (0.0, if f.kind.is_water() { 0x0026_4A60 } else { 0x003A_2A1E }),
            FeatureKind::Sea | FeatureKind::MountainRange | FeatureKind::Desert | FeatureKind::IceField => (2.0, if f.kind.is_water() { 0x0026_4A60 } else { 0x003A_2A1E }),
            FeatureKind::Forest | FeatureKind::Jungle | FeatureKind::Plains | FeatureKind::Tundra | FeatureKind::Gulf => (4.0, if f.kind.is_water() { 0x0026_4A60 } else { 0x004A_3828 }),
            FeatureKind::River | FeatureKind::Island => (6.0, if f.kind.is_water() { 0x0030_5670 } else { 0x004A_3828 }),
            FeatureKind::Lake | FeatureKind::Marsh => (10.0, if f.kind.is_water() { 0x0030_5670 } else { 0x004A_3828 }),
            FeatureKind::Peak => (10.0, 0x0030_1E14),
        };
        let text = if f.kind == FeatureKind::Peak { format!("{} {:.0}m", f.name, f.height_m) } else { f.name.clone() };
        // Bigger features of a kind rank above smaller ones; landmarks (the world's extremes)
        // rank above their kind and show from further out.
        let landmark = landmarks.iter().any(|l| l.feature == Some(f.id));
        let rank = f.kind.rank() * 10 + ((f.size as f32).log2() as u32).min(9) + if landmark { 400 } else { 0 };
        let min_tile_px = if landmark { min_tile_px.min(2.0) } else { min_tile_px };
        let style = match f.kind {
            FeatureKind::Ocean => LabelStyle::Ocean,
            FeatureKind::Sea | FeatureKind::Gulf => LabelStyle::Sea,
            FeatureKind::Continent | FeatureKind::Island => LabelStyle::Land,
            FeatureKind::MountainRange => LabelStyle::Range,
            FeatureKind::River | FeatureKind::Lake => LabelStyle::Water,
            FeatureKind::Peak => LabelStyle::Feature,
            _ => LabelStyle::Region,
        };
        labels.push(Label { x: f.anchor.0 as f32 + 0.5, y: f.anchor.1 as f32 + 0.5, text, rank, min_tile_px, color, style });
    }
    for l in landmarks.iter().filter(|l| l.feature.is_none()) {
        labels.push(Label { x: l.x as f32 + 0.5, y: l.y as f32 + 0.5, text: l.name.clone(), rank: 600, min_tile_px: 6.0, color: 0x0030_1E14, style: LabelStyle::Feature });
    }
    if let Some(h) = history {
        for s in h.settlements.values() {
            let (base, min_px) = if s.is_destroyed() {
                (300, 20.0)
            } else {
                match s.settlement_type {
                    SettlementType::Capital => (900, 5.0),
                    SettlementType::City | SettlementType::Port => (850, 8.0),
                    SettlementType::Town | SettlementType::Fort => (700, 12.0),
                    _ => (400, 20.0),
                }
            };
            let text = if s.is_destroyed() { format!("ruins of {}", s.name) } else { s.name.clone() };
            let color = if s.is_destroyed() { 0x0078_6A58 } else { 0x0030_1E14 };
            let style = if s.is_destroyed() { LabelStyle::Ruin } else { match s.settlement_type {
                SettlementType::Capital => LabelStyle::Capital,
                SettlementType::City | SettlementType::Port => LabelStyle::City,
                _ => LabelStyle::Town,
            } };
            labels.push(Label { x: s.location.0 as f32 + 0.5, y: s.location.1 as f32 + 1.4, text, rank: base + (s.population / 2000).min(99), min_tile_px: min_px, color, style });
        }
    }
    // Monuments, war hosts, sieges and outlaw bands, named under their sprites (`world_ink`).
    if let Some(h) = history {
        for t in super::world_ink::world_life(h) {
            let (rank, min_px) = match t.kind { super::world_ink::Kind::Monument(_) => (520, 14.0), super::world_ink::Kind::WarHost { .. } | super::world_ink::Kind::Siege { .. } => (640, 10.0), super::world_ink::Kind::Outlaws => (500, 14.0), _ => continue };
            if t.name.is_empty() || !t.first { continue; }
            labels.push(Label { x: t.tile.0 as f32 + 0.5, y: t.tile.1 as f32 + 1.3, text: t.name.clone(), rank, min_tile_px: min_px, color: if rank == 640 { 0x009A_2A1E } else { 0x0030_1E14 }, style: LabelStyle::Ruin });
        }
    }
    // The world's beasts, named in red above their sprites at the lair (`beasts::draw_world`).
    if let Some(h) = history {
        for b in super::beasts::world_beasts(h) {
            let up = 0.6 * (1.6 + b.look.len * 0.5) + 0.2;
            labels.push(Label { x: b.tile.0 as f32 + 0.5, y: b.tile.1 as f32 + 0.5 - up, text: b.name, rank: 650, min_tile_px: 12.0, color: 0x009A_2A1E, style: LabelStyle::Ruin });
        }
    }
    let _ = world;
    labels.sort_by_key(|l| std::cmp::Reverse(l.rank));
    labels
}

/// Place labels for the current world camera (wrapping around the date line).
fn draw_labels(labels: &[Label], cam: &Camera, world_w: usize, buf: &mut [u32], w: usize, h: usize) {
    draw_labels_avoiding(labels, cam, world_w, buf, w, h, &[]);
}

/// The screen rectangle the minimap takes (as `render_minimap` places it), for labels to avoid.
fn minimap_box(world_w: usize, world_h: usize, w: usize, h: usize) -> (i64, i64, i64, i64) {
    let mw = (w / 4).clamp(64, 360);
    let mh = (mw * world_h / world_w).max(1);
    if mw + 12 > w || mh + 12 > h { return (0, 0, 0, 0); }
    let (ox, oy) = ((w - mw - 10) as i64, 10i64);
    (ox - 4, oy - 4, ox + mw as i64 + 4, oy + mh as i64 + 4)
}

fn draw_labels_avoiding(labels: &[Label], cam: &Camera, world_w: usize, buf: &mut [u32], w: usize, h: usize, avoid: &[(i64, i64, i64, i64)]) {
    let ww = world_w as f32;
    place_labels(labels, cam.tile_px, w, h, buf, avoid, |x, y| {
        let mut dx = x - cam.cx;
        if dx > ww / 2.0 { dx -= ww; }
        if dx < -ww / 2.0 { dx += ww; }
        (w as f32 / 2.0 + dx * cam.tile_px, h as f32 / 2.0 + (y - cam.cy) * cam.tile_px)
    });
}

/// What is known about a world tile: place names, owner, settlement or ruin.
fn describe_tile(world: &WorldData, history: Option<&WorldHistory>, gaz: &Gazetteer, landmarks: &[Landmark], tw: &TileWorld, x: usize, y: usize) -> String {
    let mut parts = Vec::new();
    if let (Some(h), Some(sid)) = (history, tw.settlement[y * tw.width + x]) {
        if let Some(s) = h.settlements.get(&sid) {
            let faction = h.factions.get(&s.faction).map(|f| f.name.as_str()).unwrap_or("?");
            parts.push(match s.destroyed {
                Some(d) => format!("ruins of {} (fell in year {})", s.name, d.year),
                None => format!("{} - {:?} of {}, pop {}", s.name, s.settlement_type, faction, s.population),
            });
        }
    }
    let place = gaz.describe(x, y);
    if !place.is_empty() { parts.push(place); }
    parts.extend(crate::lore::landmarks::describe_at(landmarks, gaz, x, y));
    let res = world.resources();
    for d in res.deposits_at(x, y) {
        let q = ["poor", "good", "rich"][(d.richness - 1) as usize];
        parts.push(format!("{} {} deposit", q, crate::lore::resource_name(d.kind)));
    }
    let (fert, fish) = (*res.fertility.get(x, y), *res.fish.get(x, y));
    if let Some(soil) = world.soils().describe(x, y) { parts.push(soil); }
    if fert > 0.6 { parts.push("rich farmland".to_string()); } else if fert > 0.35 { parts.push("farmland".to_string()); }
    if fish > 0.35 { parts.push("fishing grounds".to_string()); }
    if let Some(h) = history {
        if let Some(f) = h.tile_history.get(x, y).current_owner.and_then(|id| h.factions.get(&id)) {
            parts.push(format!("held by {}", f.name));
        }
        if let Some(eco) = &h.ecology {
            if let Some(scar) = eco.scar_at(x, y) {
                if let Some(ev) = h.chronicle.events.iter().find(|e| e.id == scar.event) {
                    parts.push(format!("{} (year {})", ev.title, ev.date.year));
                }
            }
            let i = y * eco.width + x;
            if eco.farmland[i] > 0.4 { parts.push("fields".to_string()); }
            let pot = eco.forest_potential[i];
            if pot > 0.3 && eco.forest[i] < 0.3 * pot { parts.push("felled forest".to_string()); }
            let wild: Vec<String> = eco.wildlife_at(x, y, 0.15).iter().take(3)
                .map(|&(name, d)| format!("{} {}", if d > 0.6 { "many" } else if d > 0.3 { "some" } else { "few" }, name))
                .collect();
            if !wild.is_empty() { parts.push(wild.join(", ")); }
        }
    }
    let _ = world;
    parts.join(" | ")
}

/// Player marker: white ring around a red dot, centred at (cx, cy).
fn draw_marker(buf: &mut [u32], w: usize, h: usize, cx: f32, cy: f32, r: f32) {
    let ri = r.ceil() as i64 + 1;
    for dy in -ri..=ri {
        for dx in -ri..=ri {
            let (x, y) = (cx as i64 + dx, cy as i64 + dy);
            if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 { continue; }
            let d = ((dx * dx + dy * dy) as f32).sqrt();
            let k = y as usize * w + x as usize;
            if d <= r - 1.2 {
                buf[k] = 0x00E0_3030;
            } else if d <= r {
                buf[k] = 0x00FF_FFFF;
            } else if d <= r + 1.0 {
                buf[k] = 0x0010_1010;
            }
        }
    }
}

/// Square outline of half-size `half` centred at (cx, cy).
fn draw_box(buf: &mut [u32], w: usize, h: usize, cx: f32, cy: f32, half: f32, color: u32) {
    let (x0, x1) = ((cx - half) as i64, (cx + half) as i64);
    let (y0, y1) = ((cy - half) as i64, (cy + half) as i64);
    let mut put = |x: i64, y: i64| {
        if x >= 0 && y >= 0 && x < w as i64 && y < h as i64 { buf[y as usize * w + x as usize] = color; }
    };
    for x in x0..=x1 { put(x, y0); put(x, y1); }
    for y in y0..=y1 { put(x0, y); put(x1, y); }
}

fn save_rgb_png(path: &str, w: usize, h: usize, pixel: impl Fn(usize, usize) -> [u8; 3]) -> String {
    let img = image::RgbImage::from_fn(w as u32, h as u32, |x, y| image::Rgb(pixel(x as usize, y as usize)));
    match img.save(path) {
        Ok(()) => format!("saved {path}"),
        Err(e) => format!("save failed: {e}"),
    }
}

/// Open the viewer window and run until it is closed (Q / Esc / window close).
/// `embark`: open straight into the playable area at `start` (the `--dev-embark` loop).
static START_ZOOM: std::sync::OnceLock<f32> = std::sync::OnceLock::new();

/// The zoom (pixels per tile) the viewer opens at (`--tiles-zoom`, set from a plate's view).
pub fn set_start_zoom(px: f32) { let _ = START_ZOOM.set(px.clamp(1.0, 64.0)); }

/// The global cell a colony is to be made at (from a world code "...@X,Y:CX,CY").
static START_CELL: std::sync::OnceLock<(u64, u64)> = std::sync::OnceLock::new();
/// Embark positions ("cells") are kept in 1/64ths of a region cell, so a camp can be placed on a
/// river's bank (a region cell is kilometres across on small worlds).
pub const CELL_FRAC: f64 = 64.0;
pub fn set_start_cell(c: (u64, u64)) { let _ = START_CELL.set(c); }

/// A colony to open at a day: the patron's acts to replay and the day.
static RESUME: std::sync::OnceLock<(Vec<String>, u64)> = std::sync::OnceLock::new();
pub fn set_resume(script: Vec<String>, day: u64) { let _ = RESUME.set((script, day)); }

/// A colony's code: the world code and its tile, and the cell when it was made where the walker
/// stood ("76.96x48.earthlike.8.250@45,12:5460,1500").
pub fn colony_code(colony: &crate::colony::Colony) -> String {
    let base = super::plates::world_code(colony.map.world_tile).unwrap_or_default();
    match colony.cell { Some((x, y)) => format!("{}:{},{}", base, x, y), None => base }
}

/// The code made safe for a file name.
fn code_file(code: &str) -> String { code.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '_' }).collect() }

/// The playable area and seed for a colony at a world tile's centre, or at a global cell.
fn colony_site(world: &WorldData, history: Option<&WorldHistory>, tile: (usize, usize), cell: Option<(u64, u64)>) -> (crate::local::LocalMap, u64, (f64, f64)) {
    let s = cells_per_tile() as f64;
    let zs = load_region(world, history, tile, world.seed());
    // With no cell given: on the bank of the world's river where the world map has one on the
    // tile (DF: the world's river runs through the embark it crosses), else the tile's centre.
    let player = match cell {
        Some((x, y)) => (x as f64 / CELL_FRAC, y as f64 / CELL_FRAC),
        None => world_river_bank(world, &zs, tile).unwrap_or((tile.0 as f64 * s + s / 2.0, tile.1 as f64 * s + s / 2.0)),
    };
    let map = crate::local::generate_local(world, &zs.region, zs.lore.as_ref(), player.0 - zs.origin.0 as f64, player.1 - zs.origin.1 as f64);
    let seed = match cell { Some((x, y)) => world.seed() ^ (x << 20) ^ y, None => world.seed() ^ ((tile.0 as u64) << 20) ^ tile.1 as u64 };
    (map, seed, player)
}

/// Where to embark in `tile` to have its river beside the camp: the river cell (a channel 3 m wide
/// or more) nearest the tile's centre, then 50 m toward the centre, so the embark holds the river
/// and the camp dry ground. None where the tile has no river.
pub fn river_bank(world: &WorldData, history: Option<&WorldHistory>, tile: (usize, usize)) -> Option<(u64, u64)> {
    let s = cells_per_tile() as i64;
    let zs = load_region(world, history, tile, world.seed());
    let r = &zs.region;
    let (w, h) = (r.width as i64, r.height as i64);
    let (x0, y0) = (tile.0 as i64 * s - zs.origin.0, tile.1 as i64 * s - zs.origin.1);
    let (cx, cy) = (x0 as f64 + s as f64 / 2.0, y0 as f64 + s as f64 / 2.0);
    let mut best: Option<(f64, i64, i64)> = None;
    for y in y0.max(1)..(y0 + s).min(h - 1) {
        for x in x0.max(1)..(x0 + s).min(w - 1) {
            let k = (y * w + x) as usize;
            if crate::local::channel_width(r, k) < 2.0 { continue; }
            let d = (x as f64 - cx).hypot(y as f64 - cy);
            if best.map_or(true, |b| d < b.0) { best = Some((d, x, y)); }
        }
    }
    let (_, x, y) = best?;
    // A point on its bank, found the way the embark draws the channel (it meanders).
    let (px, py) = crate::local::bank_near(r, x as f64 + 0.5, y as f64 + 0.5)?;
    Some((((px + zs.origin.0 as f64) * CELL_FRAC).max(0.0) as u64, ((py + zs.origin.1 as f64) * CELL_FRAC).max(0.0) as u64))
}

/// Where to embark on a tile the world map gives a river (`local::world_river_width`): the
/// region's channel that carries the world's river through the tile (a channel at least half the
/// world river's width, the one nearest the tile's centre; else the tile's widest), and a point on
/// its bank (`local::bank_near`), in global region cells (the colony's code names only the tile:
/// this is where the tile's embark always lies). None where the world map has no river there.
fn world_river_bank(world: &WorldData, zs: &ZoomState, tile: (usize, usize)) -> Option<(f64, f64)> {
    let world_w = crate::local::world_river_width(world, tile.0, tile.1);
    if world_w <= 0.0 { return None; }
    let s = cells_per_tile();
    let r = &zs.region;
    let (w, h) = (r.width as i64, r.height as i64);
    let (x0, y0) = (tile.0 as i64 * s - zs.origin.0, tile.1 as i64 * s - zs.origin.1);
    let (cx, cy) = (x0 as f64 + s as f64 / 2.0, y0 as f64 + s as f64 / 2.0);
    let mut chans: Vec<(f32, f64, i64, i64)> = Vec::new();
    for y in y0.max(1)..(y0 + s).min(h - 1) {
        for x in x0.max(1)..(x0 + s).min(w - 1) {
            let cw = crate::local::channel_width(r, (y * w + x) as usize);
            if cw > 0.0 { chans.push((cw, (x as f64 + 0.5 - cx).hypot(y as f64 + 0.5 - cy), x, y)); }
        }
    }
    let widest = chans.iter().map(|c| c.0).fold(0.0, f32::max);
    let floor = (0.5 * world_w).min(widest);
    // Nearest the centre first; a bank may not be found by every cell (a lake or the sea beside it).
    chans.retain(|c| c.0 >= floor);
    chans.sort_by(|a, b| a.1.total_cmp(&b.1));
    chans.iter().take(24).find_map(|&(_, _, x, y)| crate::local::bank_near(r, x as f64 + 0.5, y as f64 + 0.5))
        .map(|(px, py)| (px + zs.origin.0 as f64, py + zs.origin.1 as f64))
}

/// Write the saga for each milestone the colony has reached and not yet written: numbered
/// `sagas/saga_<code>_NNN.png`. Returns the files written.
fn write_sagas(world: &WorldData, history: Option<&WorldHistory>, atlas: &Atlas, colony: &mut crate::colony::Colony) -> Vec<String> {
    let mut out = Vec::new();
    while colony.sagas_written < colony.milestones_hit.len() {
        let _ = std::fs::create_dir_all("sagas");
        let stem = format!("sagas/saga_{}", code_file(&colony_code(colony)));
        let n = (1..1000).find(|k| !std::path::Path::new(&format!("{stem}_{k:03}.png")).exists()).unwrap_or(999);
        let path = format!("{stem}_{n:03}.png");
        if saga_plate(world, history, atlas, colony, &path).is_ok() { out.push(path); }
        colony.sagas_written += 1;
    }
    out
}

/// Open settler `i`'s sheet in the ledger, stopping the clock while it is read (it starts again
/// when the sheet is closed).
fn actions_sheet(ui: &mut super::colony_ui::UiState, i: usize, speed: &mut u32, speed_before: &mut u32) {
    ui.sheet = Some(i);
    ui.selected = Some(i);
    if ui.open.is_none() { ui.open = Some(super::colony_ui::Tab::Settlers); }
    if *speed > 0 { *speed_before = *speed; *speed = 0; ui.paused_by_sheet = true; }
}

pub fn run_tile_viewer(world: &WorldData, history: Option<&WorldHistory>, atlas: Atlas, start: Option<(usize, usize)>, embark: bool) -> Result<(), Box<dyn Error>> {
    println!("Building tile map...");
    let mut tw = TileWorld::build(world, &atlas);
    if let Some(h) = history { tw.apply_history(world, h, &atlas); }
    let gaz = build_gazetteer(world, history, world.seed());
    let landmarks = crate::lore::find_landmarks(world, &gaz);
    let labels = build_labels(world, history, &gaz, &landmarks);
    // The world's beasts at their lairs (`beasts::world_beasts`).
    let world_beasts = history.map(super::beasts::world_beasts).unwrap_or_default();
    let world_life = history.map(super::world_ink::world_life).unwrap_or_default();
    let mut show_labels = true;
    // Seasons: T steps through them, C toggles an automatic year cycle.
    let mut season = crate::seasons::Season::Summer;
    let mut auto_season: Option<std::time::Instant> = None;
    tw.set_season(world, season);
    let seed = world.seed();
    let zoom_tiles = ZoomParams::default().tiles;
    let s = cells_per_tile();
    let span_x = world.width as i64 * s;

    let mut window = Window::new(
        "The Dark Tower",
        1280,
        800,
        WindowOptions { resize: true, ..WindowOptions::default() },
    )?;
    window.set_target_fps(60);

    std::thread::scope(|scope| -> Result<(), Box<dyn Error>> {
        // Open on the stronghold in the Shadow's path (where the story is), else the whole map.
        let stronghold = if start.is_none() {
            history.and_then(|h| { let name = h.present().stronghold?; h.settlements.values().find(|t| t.name == name).map(|t| t.location) })
        } else { None };
        let fitted = (1280.0 / tw.width as f32).min(800.0 / tw.height as f32).max(1.0);
        let (sx, sy) = start.or(stronghold).unwrap_or((tw.width / 2, tw.height / 2));
        let px = START_ZOOM.get().copied().unwrap_or(if start.is_some() || stronghold.is_some() { 16.0 } else { fitted });
        let mut cam = Camera { cx: sx as f32 + 0.5, cy: clamp_cy(sy as f32 + 0.5, px, 800, tw.height), tile_px: px };

        // Walking state: the current region, the player in global cells, the zoom level, and a
        // region being generated in the background.
        let mut zoom: Option<ZoomState> = None;
        let mut zoom_active = false;
        let mut player = (0.0f64, 0.0f64);
        let mut px_per_cell = 4.0f32;
        let mut pending: Option<std::thread::ScopedJoinHandle<'_, ZoomState>> = None;
        // A region being surveyed for Z (on a worker: the window stays live): the loader, the
        // tile, when it began, and where the walker will stand.
        let mut surveying: Option<(std::thread::ScopedJoinHandle<'_, ZoomState>, (usize, usize), std::time::Instant, (f64, f64))> = None;
        // Playable area (embark) state.
        // The playable area and the colony living on it (the colony owns the map).
        let mut local: Option<(crate::colony::Colony, LocalCamera)> = None;
        let mut local_active = false;
        // Game clock: 0 = paused, else game hours per real second (1x, 3x, 10x).
        let mut speed: u32 = 1;
        let mut last_frame = std::time::Instant::now();
        let mut tick_debt = 0.0f64;
        // A colony just founded opens paused on a card: name it, place stones, Space to begin.
        let mut fresh_colony = false;

        if let (true, Some(tile)) = (embark, start) {
            let t0 = std::time::Instant::now();
            let z = load_region(world, history, tile, seed);
            let cell = START_CELL.get().copied();
            let (map, colony_seed, at) = colony_site(world, history, tile, cell);
            player = at;
            let cz = map.surface_z[(map.height / 2) * map.width + map.width / 2];
            let (mcx, mcy) = (map.width as f32 / 2.0, map.height as f32 / 2.0);
            let mut colony = found_colony(map, history, tile, colony_seed, 7);
            colony.cell = cell;
            // A fresh camp heeds the world's legends; a replay to a day does not.
            if RESUME.get().is_none() { colony.heed_legends(&crate::colony::legend::load(world.seed())); }
            // Opened at a day (--code ... --interventions FILE --day N): the patron's acts replayed.
            let resumed = RESUME.get().map(|(script, day)| { colony.run_days_scripted(day.saturating_sub(1), script); *day });
            local = Some((colony, LocalCamera { cx: mcx, cy: mcy, tile_px: 16.0, z: cz, surface_view: true }));
            fresh_colony = resumed.is_none();
            zoom = Some(z);
            zoom_active = true;
            local_active = true;
            println!("Embarked at tile {},{} in {:.2}s", tile.0, tile.1, t0.elapsed().as_secs_f32());
        }

        let mut show_minimap = true;
        // Data overlay (O cycles): height, temperature, moisture, drainage, plates, stress, biomes.
        let mut overlay = Overlay::None;
        let mut buf: Vec<u32> = Vec::new();
        let mut size = (0usize, 0usize);
        let mut dirty = true;
        let mut drag: Option<((f32, f32), (f32, f32))> = None;
        // The inspector: a stack of pages (click on the map opens one, links push more).
        let mut inspect: Vec<super::inspector::Subject> = Vec::new();
        // The player's notes on the map, and one being written (the tile, the text so far).
        let mut notes = crate::lore::notes::load(seed);
        let mut note_input: Option<((usize, usize), String)> = None;
        // The site report for the embark box: (where it was computed, the line).
        let mut site_line: (Option<(i64, i64)>, String) = (None, String::new());
        // The walker's box last moved at (the site line waits until it rests).
        let mut box_moved: ((i64, i64), std::time::Instant) = ((i64::MIN, i64::MIN), std::time::Instant::now());
        let mut inspect_hits: Vec<super::inspector::Hit> = Vec::new();
        let mut log_hits: Vec<super::colony_hud::LogHit> = Vec::new();
        let mut last_hud_mouse = (0.0f32, 0.0f32);
        // Great moments: the clock stops, the camera eases there, a card says what and why.
        let mut seen_moments = 0usize;
        let mut moment_card: Option<crate::colony::Moment> = None;
        let mut pause_on_moments = true;
        let mut speed_before: u32 = 1;
        let mut cam_target: Option<(f32, f32)> = None;
        // Colonies left behind (Esc, Enter): kept as they were, marked on the map, resumed by
        // Enter on their ground.
        let mut left: Vec<(crate::colony::Colony, LocalCamera)> = Vec::new();
        // The legends of camps made in this world before (`colony::legend`).
        let mut legends = crate::colony::legend::load(world.seed());
        let mut stash = false;
        // The colony's own prompts: its name being typed, a dream being chosen, leaving asked.
        let mut name_input: Option<String> = None;
        // A place being named (the patron's "name a place"): where, and the name typed so far.
        let mut place_input: Option<((u16, u16), String)> = None;
        let mut dream_for: Option<usize> = None;
        // The colony's ledger and the patron's buttons (`colony_ui`), and what can be clicked on it.
        let mut ui = super::colony_ui::UiState::default();
        let mut ui_hits: Vec<super::colony_ui::Hit> = Vec::new();
        let mut dream_hits: Vec<super::colony_ui::Hit> = Vec::new();
        let mut press_over_ui = false;
        // U: the camp cut open from the side (a section through the row at the view's centre).
        let mut section_view = false;
        let mut leave_asked = false;
        let mut press_at: Option<(f32, f32)> = None;
        let mut was_down = false;
        let mut was_right = false;
        let mut minimap_rect = (0usize, 0usize, 0usize, 0usize);
        let mut status = String::from("click: inspect | wheel: zoom | drag/arrows: pan | Z: walk here | N: minimap | J: journal | P: screenshot | Q: quit");
        let mut last_title = String::new();

        while window.is_open() {
            let (w, h) = window.get_size();
            if (w, h) != size {
                size = (w, h);
                buf = vec![0; w * h];
                dirty = true;
            }
            let mouse = window.get_mouse_pos(MouseMode::Clamp).unwrap_or((w as f32 / 2.0, h as f32 / 2.0));
            let wheel = window.get_scroll_wheel().map(|s| s.1).unwrap_or(0.0);
            let down = window.get_mouse_down(MouseButton::Left);
            let right = window.get_mouse_down(MouseButton::Right);
            // A click is a press and release without moving (a drag pans instead).
            if down && !was_down { press_at = Some(mouse); press_over_ui = local_active && super::colony_ui::over_ui(&ui, w, h, mouse); }
            let clicked = !down && was_down && press_at.map_or(false, |p| (p.0 - mouse.0).abs() < 4.0 && (p.1 - mouse.1).abs() < 4.0);
            let right_clicked = !right && was_right;
            was_down = down;
            was_right = right;
            // Writing a note (M): the keys write until Enter pins it or Esc drops it.
            let typing = note_input.is_some() || name_input.is_some() || place_input.is_some();
            if let (Some((at, text)), true) = (place_input.as_mut(), local_active) {
                let shift = window.is_key_down(Key::LeftShift) || window.is_key_down(Key::RightShift);
                let mut done = None;
                for k in window.get_keys_pressed(KeyRepeat::Yes) {
                    match k {
                        Key::Enter => { done = Some(true); break; }
                        Key::Escape => { done = Some(false); break; }
                        Key::Backspace => { text.pop(); }
                        _ => if let Some(c) = key_char(k, shift) { if text.len() < 40 { text.push(c); } },
                    }
                }
                if let Some(keep) = done {
                    let (at, name) = (*at, text.trim().to_string());
                    place_input = None;
                    if keep && !name.is_empty() {
                        let (colony, _) = local.as_mut().unwrap();
                        colony.name_place(at, &name);
                        status = format!("The place at {},{} is called {}.", at.0, at.1, name);
                    }
                }
                dirty = true;
            }
            if let (Some(text), true) = (name_input.as_mut(), local_active) {
                let shift = window.is_key_down(Key::LeftShift) || window.is_key_down(Key::RightShift);
                let mut done = None;
                for k in window.get_keys_pressed(KeyRepeat::Yes) {
                    match k {
                        Key::Enter => { done = Some(true); break; }
                        Key::Escape => { done = Some(false); break; }
                        Key::Backspace => { text.pop(); }
                        _ => if let Some(c) = key_char(k, shift) { if text.len() < 40 { text.push(c); } },
                    }
                }
                if let Some(keep) = done {
                    let name = name_input.take().unwrap_or_default();
                    if keep && !name.trim().is_empty() {
                        let (colony, _) = local.as_mut().unwrap();
                        colony.name_colony(name.trim());
                        status = format!("The settlement is called {}.", name.trim());
                    }
                }
                dirty = true;
            } else if note_input.is_some() {
                let shift = window.is_key_down(Key::LeftShift) || window.is_key_down(Key::RightShift);
                for k in window.get_keys_pressed(KeyRepeat::Yes) {
                    let (at, text) = note_input.as_mut().unwrap();
                    match k {
                        Key::Enter => {
                            if !text.trim().is_empty() {
                                let note = crate::lore::notes::Note { x: at.0, y: at.1, text: text.trim().to_string() };
                                status = match crate::lore::notes::add(seed, note) { Ok(all) => { notes = all; format!("note pinned ({})", crate::lore::notes::path(seed)) } Err(e) => format!("note not saved: {e}") };
                            }
                            note_input = None;
                            break;
                        }
                        Key::Escape => { note_input = None; break; }
                        Key::Backspace => { text.pop(); }
                        _ => if let Some(c) = key_char(k, shift) { if text.len() < 120 { text.push(c); } },
                    }
                }
                dirty = true;
            }
            let pressed = |k: Key| !typing && window.is_key_pressed(k, KeyRepeat::No);
            let held = |k: Key| !typing && window.is_key_down(k);
            let pan_x = (held(Key::Right) || held(Key::D)) as i32 - (held(Key::Left) || held(Key::A)) as i32;
            let pan_y = (held(Key::Down) || held(Key::S)) as i32 - (held(Key::Up) || held(Key::W)) as i32;
            let running = held(Key::LeftShift) || held(Key::RightShift);
            let zoom_key = if pressed(Key::Equal) || pressed(Key::NumPadPlus) { 1.0 } else if pressed(Key::Minus) || pressed(Key::NumPadMinus) { -1.0 } else { 0.0 };
            let step = if wheel != 0.0 { wheel.signum() } else { 0.0 } + zoom_key;

            let frame_dt = last_frame.elapsed().as_secs_f64().min(0.25);
            last_frame = std::time::Instant::now();
            if stash {
                stash = false;
                if let Some(entry) = local.take() { left.push(entry); }
            }
            if local_active {
                let (colony, lcam) = local.as_mut().unwrap();
                // The clock: Space pauses, 1/2/3 = 1x/3x/10x (1x = a game hour a real second).
                if fresh_colony {
                    fresh_colony = false;
                    seen_moments = colony.moments.len();
                    speed = 0;
                    speed_before = 1;
                    leave_asked = false;
                    dream_for = None;
                    let name = colony.name.clone().unwrap_or_else(|| format!("The camp at {},{}", colony.map.world_tile.0, colony.map.world_tile.1));
                    moment_card = Some(crate::colony::Moment {
                        tick: colony.clock.tick, title: name,
                        text: format!("Day 1, 06:00. {} settlers make camp. Name the settlement (N) and place your stones: H a hall stone (the hut is raised beside it), J a grove stone (its trees are spared), K a shrine. Then Space to begin.", colony.alive()),
                        because: "Nothing moves until you begin. Favour (three, one more each dawn) blesses (F) or forbids (X) ground, favours a settler (G) or sends a dream (R).".into(),
                        at: colony.camp, choice: false,
                    });
                    dirty = true;
                }
                // The ledger (`colony_ui`): tab keys, the wheel over the panel, clicks on its
                // buttons and lines; every act goes through the same `Colony` method as its key.
                use super::colony_ui::{Action, Tab, Tool};
                let over_ui_now = super::colony_ui::over_ui(&ui, w, h, mouse);
                let mut space_now = false;
                let mut skip_now = false;
                let mut ui_esc = false;
                let mut ui_clicked = false;
                let mut actions: Vec<Action> = Vec::new();
                for (k, t) in [(Key::C, Tab::Settlers), (Key::I, Tab::Stocks), (Key::O, Tab::Works), (Key::L, Tab::Annals), (Key::T, Tab::Camp)] {
                    if pressed(k) { actions.push(Action::Tab(t)); }
                }
                let body = super::colony_ui::body_rect(w, h);
                let wheel_on_panel = wheel != 0.0 && ui.open.is_some() && body.contains(mouse.0, mouse.1);
                if wheel_on_panel { ui.scroll_by(-wheel * 40.0, body.h as f32); dirty = true; }
                if clicked {
                    if let Some(hit) = dream_hits.iter().chain(ui_hits.iter()).find(|hh| hh.rect.contains(mouse.0, mouse.1)) { actions.push(hit.action); ui_clicked = true; }
                    else if over_ui_now { ui_clicked = true; }
                }
                if pressed(Key::Escape) && inspect.is_empty() && moment_card.is_none() && dream_for.is_none() && !leave_asked {
                    if ui.tool.take().is_some() { ui_esc = true; status = "Put down.".into(); }
                    else if ui.sheet.is_some() { ui_esc = true; actions.push(Action::Back); }
                    else if ui.open.is_some() { ui_esc = true; actions.push(Action::Close); }
                    if ui_esc { dirty = true; }
                }
                for a in actions {
                    dirty = true;
                    let resume = |ui: &mut super::colony_ui::UiState, speed: &mut u32, speed_before: u32| { if ui.paused_by_sheet { ui.paused_by_sheet = false; if *speed == 0 { *speed = speed_before.max(1); } } };
                    match a {
                        Action::Tab(t) => { if ui.open == Some(t) && ui.sheet.is_none() { ui.open = None; } else { ui.open = Some(t); } if ui.sheet.take().is_some() { resume(&mut ui, &mut speed, speed_before); } }
                        Action::Close => { ui.open = None; if ui.sheet.take().is_some() { resume(&mut ui, &mut speed, speed_before); } }
                        Action::Select(i) => {
                            if ui.selected == Some(i) { actions_sheet(&mut ui, i, &mut speed, &mut speed_before); }
                            else { ui.selected = Some(i); let p = colony.draw_pos(i); cam_target = Some((p.0 + 0.5, p.1 + 0.5)); }
                        }
                        Action::Sheet(i) => actions_sheet(&mut ui, i, &mut speed, &mut speed_before),
                        Action::Back => { ui.sheet = None; resume(&mut ui, &mut speed, speed_before); }
                        Action::Arm(t) => { if ui.tool == Some(t) { ui.tool = None; status = "Put down.".into(); } else { ui.tool = Some(t); status = t.prompt().into(); } }
                        Action::Bell => { status = match colony.ring_bell() { Ok(l) => l, Err(e) => e }; }
                        Action::NameCamp => { name_input = Some(String::new()); status = "Name the settlement: type, Enter to keep, Esc to drop".into(); }
                        Action::Favour(i) => { status = match colony.favour_settler(i) { Ok(l) => l, Err(e) => e }; }
                        Action::DreamFor(i) => { if colony.patron.favour == 0 { status = "no favour left today; it returns at dawn".into(); } else { dream_for = Some(i); status = format!("A dream for {}: choose below (or 1-4)", colony.settlers[i].name); } }
                        Action::SendDream(i, d) => { status = match colony.send_dream(i, d) { Ok(l) => l, Err(e) => e }; dream_for = None; }
                        Action::NoDream => { dream_for = None; status = "No dream sent.".into(); }
                        Action::Speed(sp) => { speed = sp; speed_before = sp; moment_card = None; }
                        Action::Pause => space_now = true,
                        Action::Skip => skip_now = true,
                        Action::Filter(f) => ui.annals = f,
                        Action::Event(id) => inspect = vec![super::inspector::Subject::Event(id)],
                        Action::Refugees(take) => { status = match colony.answer_refugees(take) { Ok(l) => l, Err(e) => e }; moment_card = None; speed = speed_before.max(1); }
                        Action::Look(x, y) => { lcam.surface_view = true; cam_target = Some((x as f32 + 0.5, y as f32 + 0.5)); }
                        Action::Top => ui.scroll_by(-1.0e9, body.h as f32),
                    }
                }
                // Choosing a dream: 1-4 pick it, Esc lets it go.
                if let Some(i) = dream_for {
                    let pick = [(Key::Key1, crate::colony::Dream::Hut), (Key::Key2, crate::colony::Dream::Plenty), (Key::Key3, crate::colony::Dream::Rest), (Key::Key4, crate::colony::Dream::Watch)]
                        .into_iter().find(|(k, _)| pressed(*k)).map(|(_, d)| d);
                    if let Some(d) = pick {
                        status = match colony.send_dream(i, d) { Ok(l) => l, Err(e) => e };
                        dream_for = None;
                        dirty = true;
                    } else if pressed(Key::Escape) {
                        dream_for = None;
                        status = "No dream sent.".into();
                        dirty = true;
                    }
                }
                let choosing = dream_for.is_some();
                // Leaving asks first: Enter leaves, Esc stays.
                let leave_now = leave_asked && pressed(Key::Enter);
                if leave_asked && !leave_now && pressed(Key::Escape) { leave_asked = false; status.clear(); dirty = true; }
                // Space resumes at the speed it paused (and closes a moment's card).
                if colony.moments.len() < seen_moments { seen_moments = 0; }
                // A choice on the card (the refugees): Y takes them in, N turns them away.
                let answering = moment_card.as_ref().map_or(false, |m| m.choice) && colony.refugees_waiting();
                if answering && (pressed(Key::Y) || pressed(Key::N)) {
                    status = match colony.answer_refugees(pressed(Key::Y)) { Ok(l) => l, Err(e) => e };
                    moment_card = None;
                    speed = speed_before.max(1);
                    dirty = true;
                }
                if (pressed(Key::Space) || space_now) && !leave_asked && !choosing && !answering {
                    if moment_card.take().is_some() { speed = speed_before.max(1); }
                    else if speed == 0 { speed = speed_before.max(1); }
                    else { speed_before = speed; speed = 0; }
                    dirty = true;
                }
                for (key, sp) in [(Key::Key1, 1), (Key::Key2, 3), (Key::Key3, 10)] {
                    if !choosing && pressed(key) { speed = sp; speed_before = sp; moment_card = None; dirty = true; }
                }
                // Skip (4 or Tab): run on to the next line of the log, or to the dawn.
                if !choosing && !leave_asked && (pressed(Key::Key4) || pressed(Key::Tab) || skip_now) {
                    moment_card = None;
                    let lines = colony.log.len();
                    let t0 = colony.clock.tick;
                    let cap = t0 + 3 * crate::colony::TICKS_PER_DAY;
                    while colony.clock.tick < cap {
                        colony.tick();
                        if colony.log.len() > lines || (colony.clock.hour() == 6 && colony.clock.minute() == 0) { break; }
                    }
                    status = format!("Skipped {} to {}", crate::colony::span_words(colony.clock.tick - t0), colony.clock.stamp());
                    dirty = true;
                }
                if pressed(Key::M) {
                    pause_on_moments = !pause_on_moments;
                    status = format!("Stopping for great moments: {}", if pause_on_moments { "on" } else { "off" });
                    dirty = true;
                }
                if speed > 0 {
                    // While the raid is on the map the clock runs at a third (and no faster than
                    // 1x): the attackers' approach takes about twenty real seconds to watch.
                    let rate = if colony.attackers_out() { speed.min(1) as f64 / 3.0 } else { speed as f64 };
                    tick_debt += frame_dt * 60.0 * rate;
                    let n = tick_debt.floor() as u64;
                    tick_debt -= n as f64;
                    for _ in 0..n {
                        colony.tick();
                        // Only a major moment stops the clock (`Moment::major`).
                        if pause_on_moments && colony.moments[seen_moments.min(colony.moments.len())..].iter().any(|m| m.major()) { tick_debt = 0.0; break; }
                    }
                    if n > 0 { dirty = true; }
                }
                // Minor moments go to the status line without stopping the clock.
                while moment_card.is_none() && colony.moments.len() > seen_moments && !colony.moments[seen_moments].major() {
                    let m = &colony.moments[seen_moments];
                    status = format!("{}: {}", m.title, m.text);
                    seen_moments += 1;
                    dirty = true;
                }
                if moment_card.is_none() && colony.moments.len() > seen_moments {
                    let m = colony.moments[seen_moments].clone();
                    seen_moments += 1;
                    if pause_on_moments {
                        if speed > 0 { speed_before = speed; }
                        speed = 0;
                        cam_target = Some((m.at.0 as f32 + 0.5, m.at.1 as f32 + 0.5));
                        moment_card = Some(m);
                        dirty = true;
                    } else {
                        status = format!("{}: {}", m.title, m.text);
                    }
                }
                // Ease the camera to the moment's place (about 600 ms).
                if let Some(t) = cam_target {
                    let k = 1.0 - (-(frame_dt as f32) / 0.15).exp();
                    lcam.cx += (t.0 - lcam.cx) * k;
                    lcam.cy += (t.1 - lcam.cy) * k;
                    if (t.0 - lcam.cx).abs() < 0.05 && (t.1 - lcam.cy).abs() < 0.05 { cam_target = None; }
                    dirty = true;
                }
                // The inspector: click a settler to read who they are; Esc closes it first.
                let panel = super::inspector::panel_rect(w, h);
                let over_panel = !inspect.is_empty() && panel.contains(mouse.0, mouse.1);
                let esc = pressed(Key::Escape) && !choosing && !leave_asked && !ui_esc;
                if esc && !inspect.is_empty() {
                    inspect.clear();
                    dirty = true;
                } else if esc && moment_card.is_some() {
                    moment_card = None;
                    dirty = true;
                } else if (esc || pressed(Key::Q)) && !leave_now {
                    leave_asked = true;
                    if speed > 0 { speed_before = speed; }
                    speed = 0;
                    dirty = true;
                } else if leave_now {
                    leave_asked = false;
                    local_active = false;
                    inspect.clear();
                    dirty = true;
                    // The colony is its code, the patron's acts and its day: written down, and kept
                    // (Enter on its ground resumes it). Elsewhere: --code CODE --interventions FILE --day N.
                    let code = colony_code(colony);
                    let _ = std::fs::create_dir_all("colonies");
                    let path = format!("colonies/{}.txt", code_file(&code));
                    let body = format!("# code {}\n# day {}\n{}\n", code, colony.clock.day(), colony.interventions.join("\n"));
                    // Its legend joins the world's (`colony::legend`).
                    let _ = crate::colony::legend::save(world.seed(), &mut legends, colony.legend(&code));
                    status = match std::fs::write(&path, body) {
                        Ok(()) => format!("Left on day {}; saved {path} (Enter on its ground to return)", colony.clock.day()),
                        Err(e) => format!("could not save {path}: {e}"),
                    };
                    stash = true;
                }
                if (pressed(Key::Backspace) || right_clicked) && !inspect.is_empty() {
                    inspect.pop();
                    dirty = true;
                }
                if clicked && !ui_clicked {
                    let (mx, my) = (lcam.cx + (mouse.0 - w as f32 / 2.0) / lcam.tile_px, lcam.cy + (mouse.1 - h as f32 / 2.0) / lcam.tile_px);
                    let at = (mx.max(0.0) as u16, my.max(0.0) as u16);
                    let who = colony.settler_at(mx, my, 0.9, if lcam.surface_view { None } else { Some(lcam.z) });
                    if let (Some(tool), false) = (ui.tool, over_panel) {
                        // A verb held from the buttons, carried out where the click fell.
                        let said: Result<String, String> = match tool {
                            Tool::Bless => colony.mark_place(at, 6, false),
                            Tool::Forbid => colony.mark_place(at, 6, true),
                            Tool::Hall => colony.place_stone(crate::colony::StoneKind::Hall, at),
                            Tool::Grove => colony.place_stone(crate::colony::StoneKind::Grove, at),
                            Tool::Shrine => colony.place_stone(crate::colony::StoneKind::Shrine, at),
                            Tool::Favour => who.map(|i| colony.favour_settler(i)).unwrap_or_else(|| Err("click a settler to favour them".into())),
                            Tool::Dream => match who { Some(i) if colony.patron.favour > 0 => { dream_for = Some(i); Ok(format!("A dream for {}: choose below (or 1-4)", colony.settlers[i].name)) } Some(_) => Err("no favour left today; it returns at dawn".into()), None => Err("click a settler to send them a dream".into()) },
                            Tool::NamePlace => { place_input = Some((at, String::new())); Ok("Name the place: type, Enter to keep, Esc to drop".into()) }
                        };
                        let ok = said.is_ok();
                        status = match said { Ok(l) => l, Err(e) => e };
                        // One use (the stones and names); blessing and forbidding stay in hand.
                        if ok && !matches!(tool, Tool::Bless | Tool::Forbid) { ui.tool = None; }
                        dirty = true;
                    } else if !over_panel && who.is_some() && inspect.is_empty() {
                        // A settler on the map: their sheet in the ledger.
                        let i = who.unwrap();
                        ui.open = Some(Tab::Settlers);
                        ui.selected = Some(i);
                        actions_sheet(&mut ui, i, &mut speed, &mut speed_before);
                        dirty = true;
                    } else if over_panel {
                        if let Some(hit) = inspect_hits.iter().find(|hh| hh.rect.contains(mouse.0, mouse.1)) {
                            inspect.push(hit.to);
                            dirty = true;
                        }
                    } else if let Some(hit) = log_hits.iter().find(|hh| hh.rect.contains(mouse.0, mouse.1)) {
                        // A line of the log opens the settler it names.
                        inspect = vec![super::inspector::Subject::Settler(hit.settler)];
                        dirty = true;
                    } else {
                        let (hx, hy) = (lcam.cx + (mouse.0 - w as f32 / 2.0) / lcam.tile_px, lcam.cy + (mouse.1 - h as f32 / 2.0) / lcam.tile_px);
                        if let Some(i) = colony.settler_at(hx, hy, 0.9, if lcam.surface_view { None } else { Some(lcam.z) }) {
                            inspect = vec![super::inspector::Subject::Settler(i)];
                            dirty = true;
                        } else if let Some(i) = colony.marks.iter().position(|m| (m.at.0 as f32 + 0.5 - hx).abs() < 0.9 && (m.at.1 as f32 + 0.5 - hy).abs() < 0.9) {
                            inspect = vec![super::inspector::Subject::ColonyMark(i)];
                            dirty = true;
                        }
                    }
                }
                // The patron's verbs at the mouse: F bless the ground, X forbid it (again on a
                // mark lifts it), G favour the settler under it, R choose a dream for them, N
                // name the settlement. D only pans.
                if !choosing && !leave_asked {
                    let (mx, my) = (lcam.cx + (mouse.0 - w as f32 / 2.0) / lcam.tile_px, lcam.cy + (mouse.1 - h as f32 / 2.0) / lcam.tile_px);
                    let at = (mx.max(0.0) as u16, my.max(0.0) as u16);
                    let who = colony.settler_at(mx, my, 0.9, if lcam.surface_view { None } else { Some(lcam.z) });
                    let said = if pressed(Key::H) { Some(colony.place_stone(crate::colony::StoneKind::Hall, at)) }
                        else if pressed(Key::J) { Some(colony.place_stone(crate::colony::StoneKind::Grove, at)) }
                        else if pressed(Key::K) { Some(colony.place_stone(crate::colony::StoneKind::Shrine, at)) }
                        else if pressed(Key::F) { Some(colony.mark_place(at, 6, false)) }
                        else if pressed(Key::X) { Some(colony.mark_place(at, 6, true)) }
                        else if pressed(Key::G) { Some(who.map(|i| colony.favour_settler(i)).unwrap_or_else(|| Err("G favours the settler under the mouse; there is no one there".into()))) }
                        // The bell: everyone under a roof until dawn (`Colony::ring_bell`).
                        else if pressed(Key::B) { Some(colony.ring_bell()) }
                        else if pressed(Key::R) {
                            match who {
                                Some(_) if colony.patron.favour == 0 => Some(Err("no favour left today; it returns at dawn".into())),
                                Some(i) => { dream_for = Some(i); Some(Ok(format!("A dream for {}: 1 the hut finished, 2 plenty, 3 rest, 4 the watch (Esc: none)", colony.settlers[i].name))) }
                                None => Some(Err("R sends a dream to the settler under the mouse; there is no one there".into())),
                            }
                        }
                        else if pressed(Key::N) && !colony.refugees_waiting() { name_input = Some(String::new()); Some(Ok("Name the settlement: type, Enter to keep, Esc to drop".into())) }
                        else { None };
                    if let Some(r) = said { status = match r { Ok(l) => l, Err(e) => e }; dirty = true; }
                }
                if pressed(Key::P) {
                    // P writes the colony's saga so far, not a raw frame.
                    colony.milestones_hit.push(format!("day {}", colony.clock.day()));
                    let files = write_sagas(world, history, &atlas, colony);
                    status = files.last().map(|f| format!("saga written: {f}")).unwrap_or_else(|| "the saga could not be written".into());
                }
                // A milestone reached (the first raid, the first winter, a year, the end): its saga.
                if colony.sagas_written < colony.milestones_hit.len() {
                    let files = write_sagas(world, history, &atlas, colony);
                    if let Some(f) = files.last() { status = format!("{}: saga written, {}", colony.milestones_hit.last().cloned().unwrap_or_default(), f); }
                }
                let map = &colony.map;
                if step != 0.0 && !wheel_on_panel {
                    let before = (lcam.cx + (mouse.0 - w as f32 / 2.0) / lcam.tile_px, lcam.cy + (mouse.1 - h as f32 / 2.0) / lcam.tile_px);
                    lcam.tile_px = (lcam.tile_px * 1.25f32.powf(step)).clamp(3.0, 48.0);
                    lcam.cx = before.0 - (mouse.0 - w as f32 / 2.0) / lcam.tile_px;
                    lcam.cy = before.1 - (mouse.1 - h as f32 / 2.0) / lcam.tile_px;
                    dirty = true;
                }
                if pan_x != 0 || pan_y != 0 {
                    let px = if running { 24.0 } else { 8.0 };
                    lcam.cx = (lcam.cx + pan_x as f32 * px / lcam.tile_px).clamp(0.0, map.width as f32);
                    lcam.cy = (lcam.cy + pan_y as f32 * px / lcam.tile_px).clamp(0.0, map.height as f32);
                    dirty = true;
                }
                match (down, drag) {
                    (true, None) if !press_over_ui => drag = Some((mouse, (lcam.cx, lcam.cy))),
                    (true, None) => {}
                    (true, Some((start, c0))) => {
                        lcam.cx = (c0.0 - (mouse.0 - start.0) / lcam.tile_px).clamp(0.0, map.width as f32);
                        lcam.cy = (c0.1 - (mouse.1 - start.1) / lcam.tile_px).clamp(0.0, map.height as f32);
                        dirty = true;
                    }
                    (false, _) => drag = None,
                }
                // DF keys: '<' goes up a level, '>' goes down.
                let up = window.is_key_pressed(Key::Comma, KeyRepeat::Yes) || window.is_key_pressed(Key::PageUp, KeyRepeat::Yes);
                let dn = window.is_key_pressed(Key::Period, KeyRepeat::Yes) || window.is_key_pressed(Key::PageDown, KeyRepeat::Yes);
                if up || dn {
                    // From the surface view, start from the ground's level under the view's centre.
                    if lcam.surface_view {
                        let (gx, gy) = ((lcam.cx as usize).min(map.width - 1), (lcam.cy as usize).min(map.height - 1));
                        lcam.z = map.surface_z[gy * map.width + gx];
                    }
                    lcam.z = (lcam.z + up as i32 - dn as i32).clamp(0, map.depth as i32 - 1);
                    lcam.surface_view = false;
                    dirty = true;
                }
                // [ and ]: up or down to the next level with something dug or built on it.
                let (jump_up, jump_dn) = (pressed(Key::LeftBracket), pressed(Key::RightBracket));
                if jump_up || jump_dn {
                    let levels = colony.delve_levels();
                    let cur = if lcam.surface_view { map.surface_z[colony.camp.1 as usize * map.width + colony.camp.0 as usize] } else { lcam.z };
                    let next = if jump_dn { levels.iter().copied().find(|&z| z < cur) } else { levels.iter().rev().copied().find(|&z| z > cur) };
                    match next {
                        Some(z) => { lcam.z = z; lcam.surface_view = false; }
                        None if jump_up => { lcam.surface_view = true; }
                        None => {}
                    }
                    dirty = true;
                }
                if pressed(Key::V) {
                    // Into the level view at the ground's level under the view's centre.
                    if lcam.surface_view {
                        let (gx, gy) = ((lcam.cx as usize).min(map.width - 1), (lcam.cy as usize).min(map.height - 1));
                        lcam.z = map.surface_z[gy * map.width + gx];
                    }
                    lcam.surface_view = !lcam.surface_view;
                    dirty = true;
                }
                if pressed(Key::U) { section_view = !section_view; dirty = true; }
                // Hover: describe the tile under the mouse at the viewed level.
                let (hx, hy) = (lcam.cx + (mouse.0 - w as f32 / 2.0) / lcam.tile_px, lcam.cy + (mouse.1 - h as f32 / 2.0) / lcam.tile_px);
                let info = if hx >= 0.0 && hy >= 0.0 && (hx as usize) < map.width && (hy as usize) < map.height {
                    let (tx, ty) = (hx as usize, hy as usize);
                    let z = if lcam.surface_view { map.surface_z[ty * map.width + tx] } else { lcam.z } as usize;
                    let c = map.cell(tx, ty, z);
                    let what = match c.shape {
                        Shape::Wall => format!("{:?} wall", c.material),
                        Shape::Floor | Shape::Ramp => {
                            let plant = match c.plant {
                                Plant::Tree(t) => format!(", {:?} tree", t),
                                Plant::Shrub => ", shrub".to_string(),
                                Plant::Grass => ", grass".to_string(),
                                Plant::Crop(k) => format!(", {} crop", ["wheat", "barley", "flax"][k as usize % 3]),
                                Plant::None => String::new(),
                            };
                            format!("{:?} {}{}{}", c.material, if c.shape == Shape::Ramp { "ramp" } else { "floor" }, plant, if c.boulder { ", boulder" } else { "" })
                        }
                        Shape::Stair => format!("a stair cut in the {:?}", c.material),
                        Shape::Empty if c.water > 0 => format!("water {}/7", c.water),
                        Shape::Empty => "open space".to_string(),
                    };
                    let feature = map.features[ty * map.width + tx];
                    let sign = if feature != crate::local::wildlife::Feature::None && z as i32 == map.surface_z[ty * map.width + tx] {
                        format!(", {}", feature.name())
                    } else {
                        String::new()
                    };
                    format!("({tx},{ty}) {what}{sign}")
                } else {
                    String::new()
                };
                let view = if lcam.surface_view { "surface view".to_string() } else { format!("z {} ({:.0} m)", lcam.z, map.z_elevation(lcam.z)) };
                // A settler under the mouse says what they are doing and why.
                let who = colony.settler_at(hx, hy, 0.8, if lcam.surface_view { None } else { Some(lcam.z) }).map(|i| &colony.settlers[i])
                    .map(|s| format!("{}: {} - {} | ", s.name, s.job.verb(), s.why))
                    .or_else(|| colony.marks.iter().find(|m| (m.at.0 as f32 + 0.5 - hx).abs() < 0.8 && (m.at.1 as f32 + 0.5 - hy).abs() < 0.8)
                        .map(|m| format!("{} (click to read) | ", m.title)))
                    .or_else(|| (hx >= 0.0 && hy >= 0.0).then(|| colony.building_at((hx as u16, hy as u16))).flatten().map(|b| format!("{} | ", b)))
                    .unwrap_or_default();
                let clock = format!("{} {}", colony.clock.stamp(), if speed == 0 { "(paused)".to_string() } else { format!("{}x", speed) });
                // The clock, favour, the hovered settler's why, the keys and the last act's answer
                // are drawn in the window (`colony_hud`); the title keeps the code and the cell.
                let _ = (&clock, &who);
                let title = format!("The Dark Tower | {} | {} | {}", colony_code(colony), view, info);
                // The HUD follows the mouse and the clock.
                if mouse != last_hud_mouse { last_hud_mouse = mouse; dirty = true; }
                if title != last_title { window.set_title(&title); last_title = title; }
            } else if zoom_active {
                // Swap in the background-generated region once it is ready.
                if pending.as_ref().map(|p| p.is_finished()).unwrap_or(false) {
                    let next = pending.take().unwrap().join().expect("region loader panicked");
                    zoom = Some(next);
                    dirty = true;
                }
                let z = zoom.as_mut().unwrap();
                // Keep the player in the region's frame across the date line.
                let rw = z.region.width as f64;
                while player.0 - (z.origin.0 as f64) < 0.0 { player.0 += span_x as f64; }
                while player.0 - (z.origin.0 as f64) >= rw { player.0 -= span_x as f64; }

                if pressed(Key::Escape) || pressed(Key::Z) || pressed(Key::Q) {
                    zoom_active = false;
                    // Centre the world map on where the player walked to.
                    cam.cx = (player.0 / s as f64) as f32;
                    cam.cy = (player.1 / s as f64) as f32;
                    dirty = true;
                }
                if step != 0.0 {
                    px_per_cell = (px_per_cell * 1.25f32.powf(step)).clamp(0.25, 32.0);
                    dirty = true;
                }
                if pressed(Key::F) { px_per_cell = 4.0; dirty = true; }
                if pan_x != 0 || pan_y != 0 {
                    let px = if running { RUN_PX_PER_FRAME } else { WALK_PX_PER_FRAME };
                    let norm = if pan_x != 0 && pan_y != 0 { std::f64::consts::FRAC_1_SQRT_2 } else { 1.0 };
                    let speed = px * norm / px_per_cell as f64;
                    // The player can't leave the loaded region; the next one loads before then.
                    let lx = (player.0 - z.origin.0 as f64 + pan_x as f64 * speed).clamp(0.0, z.region.width as f64 - 1.0);
                    let ly = (player.1 - z.origin.1 as f64 + pan_y as f64 * speed).clamp(0.0, z.region.height as f64 - 1.0);
                    player = (z.origin.0 as f64 + lx, z.origin.1 as f64 + ly);
                    dirty = true;
                }
                // Prefetch the region centred on the player's tile when nearing an edge (or when
                // the view would show past the region).
                let (lx, ly) = (player.0 - z.origin.0 as f64, player.1 - z.origin.1 as f64);
                let half_view = (w.max(h) as f64 / 2.0) / px_per_cell as f64;
                let edge = (PREFETCH_EDGE_TILES * s as f64).max(half_view + s as f64 / 2.0);
                let near_edge = lx < edge || ly < edge || lx > z.region.width as f64 - edge || ly > z.region.height as f64 - edge;
                let player_tile = (
                    (player.0.div_euclid(s as f64) as i64).rem_euclid(world.width as i64) as usize,
                    (player.1.div_euclid(s as f64) as i64).clamp(0, world.height as i64 - 1) as usize,
                );
                if near_edge && pending.is_none() && player_tile != z.tile {
                    pending = Some(scope.spawn(move || load_region(world, history, player_tile, seed)));
                    dirty = true;
                }
                if pressed(Key::X) {
                    let path = format!("zoom_{}_{}_{}.png", seed, z.tile.0, z.tile.1);
                    status = match z.region.save_png(std::path::Path::new(&path)) {
                        Ok(()) => format!("saved {path}"),
                        Err(e) => format!("save failed: {e}"),
                    };
                }

                // Enter on a colony left behind: back to it as it was.
                let back = left.iter().position(|(c, _)| match c.cell {
                    Some((x, y)) => (x as f64 / CELL_FRAC - player.0).abs().max((y as f64 / CELL_FRAC - player.1).abs()) <= 24.0,
                    None => c.map.world_tile == ((player.0 / s as f64) as usize % world.width, ((player.1 / s as f64) as usize).min(world.height - 1)),
                });
                if pressed(Key::Enter) && back.is_some() {
                    let (c, cam_l) = left.remove(back.unwrap());
                    status = format!("Back at {}, day {}", c.name.clone().unwrap_or_else(|| "the camp".into()), c.clock.day());
                    local = Some((c, cam_l));
                    local_active = true;
                    speed = 0;
                    dirty = true;
                    continue;
                }
                if pressed(Key::Enter) {
                    // Embark: the playable area is centred on the walker.
                    window.set_title("Generating playable area...");
                    let t0 = std::time::Instant::now();
                    // At the walker's position, kept to the precision a world code records, so the
                    // code rebuilds exactly this camp.
                    let cell = ((player.0 * CELL_FRAC).max(0.0) as u64, (player.1 * CELL_FRAC).max(0.0) as u64);
                    let here = ((player.0 / s as f64) as usize % world.width, ((player.1 / s as f64) as usize).min(world.height - 1));
                    let (map, colony_seed, _) = colony_site(world, history, here, Some(cell));
                    let _ = &z;
                    let cz = map.surface_z[(map.height / 2) * map.width + map.width / 2];
                    let lcam = LocalCamera { cx: map.width as f32 / 2.0, cy: map.height as f32 / 2.0, tile_px: 16.0, z: cz, surface_view: true };
                    status = format!("embarked in {:.2}s", t0.elapsed().as_secs_f32());
                    if !crate::colony::habitable(&map) {
                        status = "too much water here to make camp; walk to drier ground".into();
                        continue;
                    }
                    let verdict = crate::colony::survey(&map).verdict();
                    if let Err(why) = &verdict {
                        status = format!("no camp here: {why}; walk on");
                        continue;
                    }
                    let mut colony = found_colony(map, history, here, colony_seed, 7);
                    colony.cell = Some(cell);
                    colony.heed_legends(&legends);
                    local = Some((colony, lcam));
                    fresh_colony = true;
                    local_active = true;
                    if let Ok(Some(hard)) = verdict { status = format!("A hard place to live: {hard}"); }
                    dirty = true;
                    continue;
                }

                // Title: what the player is standing on.
                let r = &z.region;
                let k = (ly as usize).min(r.height - 1) * r.width + (lx as usize).min(r.width - 1);
                let e = r.elevation_m[k];
                let ground = if e <= 0.0 { format!("sea, {:.0} m deep", -e) }
                    else if r.lake_depth_m[k] > 0.0 { format!("lake, {:.0} m deep", r.lake_depth_m[k]) }
                    else if r.river_width_m[k] > 0.0 { format!("river ~{:.0} m wide", r.river_width_m[k]) }
                    else { format!("{:.0} m", e) };
                let km = r.cell_m as f64 / 1000.0;
                let loading = if pending.is_some() { " | loading next region..." } else { "" };
                // Where am I: a settlement here, else the named features of this world tile.
                let near = z.lore.as_ref().and_then(|l| {
                    l.sites.iter().filter_map(|site| {
                        let d = ((site.x - lx).powi(2) + (site.y - ly).powi(2)).sqrt() * r.cell_m as f64;
                        let label = if site.destroyed_year.is_some() {
                            (d < site.core_m as f64 + 300.0).then(|| format!("ruins of {} (fell in year {})", site.name, site.destroyed_year.unwrap_or(0)))
                        } else if d < site.core_m as f64 {
                            Some(format!("in {}, a {:?} of {} ({} people)", site.name, site.kind, site.faction_name, site.population))
                        } else if d < site.fields_m as f64 {
                            Some(format!("the fields of {}", site.name))
                        } else {
                            None
                        };
                        label.map(|t| (d, t))
                    }).min_by(|a, b| a.0.partial_cmp(&b.0).unwrap()).map(|(_, t)| t)
                });
                let place = near.unwrap_or_else(|| gaz.describe(player_tile.0, player_tile.1));
                // What an embark here would hold, recomputed when the box moves on (~30 ms).
                let key = ((player.0 / 24.0) as i64, (player.1 / 24.0) as i64);
                if box_moved.0 != key { box_moved = (key, std::time::Instant::now()); if site_line.0 != Some(key) { site_line.1 = "(the site is read when you stop)".into(); } }
                // Read the site only once the walker has stopped for a moment (it costs up to a
                // second near a town; walking past one used to stall).
                if site_line.0 != Some(key) && box_moved.1.elapsed().as_secs_f32() > 0.35 {
                    let m = generate_local(world, &z.region, z.lore.as_ref(), player.0 - z.origin.0 as f64, player.1 - z.origin.1 as f64);
                    site_line = (Some(key), if !crate::colony::habitable(&m) { "under water: no camp can be made here".into() } else {
                        match crate::colony::survey(&m).verdict() {
                            Err(why) => format!("NO CAMP: {why}"),
                            Ok(Some(hard)) => format!("{} | {}", hard.to_uppercase(), crate::local::site::report(&m).join(", ")),
                            Ok(None) => crate::local::site::report(&m).join(", "),
                        }
                    });
                }
                let title = format!(
                    "{} | {} | {:.1}°C | site: {} | arrows/WASD walk, Shift run, Enter embark, Esc map{} | {}",
                    place, ground, r.temperature_c[k], site_line.1, loading, status
                );
                let _ = km;
                if title != last_title { window.set_title(&title); last_title = title; }
            } else {
                let panel = super::inspector::panel_rect(w, h);
                let over_panel = !inspect.is_empty() && panel.contains(mouse.0, mouse.1);
                if pressed(Key::Escape) && !inspect.is_empty() {
                    inspect.clear();
                    dirty = true;
                } else if pressed(Key::Escape) || pressed(Key::Q) {
                    break;
                }
                if (pressed(Key::Backspace) || right_clicked) && !inspect.is_empty() {
                    inspect.pop();
                    dirty = true;
                }
                if clicked && history.is_some() {
                    if over_panel {
                        if let Some(hit) = inspect_hits.iter().find(|hh| hh.rect.contains(mouse.0, mouse.1)) {
                            inspect.push(hit.to);
                            dirty = true;
                        }
                    } else {
                        let (hx, hy) = screen_to_world(&cam, mouse.0, mouse.1, w, h);
                        let t = (hx.rem_euclid(tw.width as f32) as usize, (hy.max(0.0) as usize).min(tw.height - 1));
                        inspect = vec![super::inspector::Subject::Tile(t.0, t.1)];
                        dirty = true;
                    }
                }
                if step != 0.0 {
                    let before = screen_to_world(&cam, mouse.0, mouse.1, w, h);
                    cam.tile_px = (cam.tile_px * 1.25f32.powf(step)).clamp(MIN_TILE_PX, MAX_TILE_PX);
                    cam.cx = before.0 - (mouse.0 - w as f32 / 2.0) / cam.tile_px;
                    cam.cy = before.1 - (mouse.1 - h as f32 / 2.0) / cam.tile_px;
                    dirty = true;
                }
                if pan_x != 0 || pan_y != 0 {
                    cam.cx += pan_x as f32 * 12.0 / cam.tile_px;
                    cam.cy += pan_y as f32 * 12.0 / cam.tile_px;
                    dirty = true;
                }
                let (mx0, my0, mw, mh) = minimap_rect;
                let on_minimap = show_minimap && mw > 0
                    && mouse.0 >= mx0 as f32 && mouse.0 < (mx0 + mw) as f32
                    && mouse.1 >= my0 as f32 && mouse.1 < (my0 + mh) as f32;
                match (down, drag) {
                    // Pressing on the panel doesn't pan the map.
                    (true, None) if over_panel => {}
                    (true, _) if on_minimap && !over_panel => {
                        cam.cx = (mouse.0 - mx0 as f32) / mw as f32 * tw.width as f32;
                        cam.cy = (mouse.1 - my0 as f32) / mh as f32 * tw.height as f32;
                        dirty = true;
                    }
                    (true, None) => drag = Some((mouse, (cam.cx, cam.cy))),
                    (true, Some((start, c0))) => {
                        cam.cx = c0.0 - (mouse.0 - start.0) / cam.tile_px;
                        cam.cy = c0.1 - (mouse.1 - start.1) / cam.tile_px;
                        dirty = true;
                    }
                    (false, _) => drag = None,
                }
                cam.cx = cam.cx.rem_euclid(tw.width as f32);
                cam.cy = clamp_cy(cam.cy, cam.tile_px, h, tw.height);
                if pressed(Key::N) { show_minimap = !show_minimap; dirty = true; }
                if pressed(Key::M) {
                    let (hx, hy) = screen_to_world(&cam, mouse.0, mouse.1, w, h);
                    note_input = Some(((hx.rem_euclid(tw.width as f32) as usize, (hy.max(0.0) as usize).min(tw.height - 1)), String::new()));
                    status = "write a note: Enter pins it, Esc drops it".into();
                }
                if pressed(Key::P) {
                    // A plate: the map without the interface, framed, captioned and numbered.
                    let mut plate = vec![0u32; w * h];
                    render_world(&tw, &atlas, &cam, &mut plate, w, h);
                    super::world_ink::draw(&world_life, &cam, tw.width, &mut plate, w, h);
                    super::beasts::draw_world(&world_beasts, &cam, tw.width, &mut plate, w, h, false);
                    if show_labels { draw_labels(&labels, &cam, tw.width, &mut plate, w, h); }
                    draw_notes(&notes, None, &cam, tw.width, &mut plate, w, h);
                    let info = plate_info(world, history, &gaz, &tw, &cam, w, h, season);
                    super::plates::decorate(&mut plate, w, h, &info);
                    let path = super::plates::next_path(seed);
                    status = match super::plates::save(&path, &plate, w, h, (cam.cx, cam.cy, cam.tile_px), &info) {
                        Ok(()) => format!("saved {path}"),
                        Err(e) => format!("plate failed: {e}"),
                    };
                }

                let (hx, hy) = screen_to_world(&cam, mouse.0, mouse.1, w, h);
                let tile = (hx.rem_euclid(tw.width as f32) as usize, (hy.max(0.0) as usize).min(tw.height - 1));

                if pressed(Key::Z) {
                    // Walk from the tile under the mouse. Reuse the loaded region if it is there.
                    let target = (tile.0 as f64 * s as f64 + s as f64 / 2.0, tile.1 as f64 * s as f64 + s as f64 / 2.0);
                    let inside = zoom.as_ref().map(|z| {
                        let (lx, ly) = (target.0 - z.origin.0 as f64, target.1 - z.origin.1 as f64);
                        lx >= 0.0 && ly >= 0.0 && lx < z.region.width as f64 && ly < z.region.height as f64
                    }).unwrap_or(false);
                    if !inside {
                        // Surveyed on a worker; the map stays live under a card until it is ready.
                        if surveying.is_none() {
                            surveying = Some((scope.spawn(move || load_region(world, history, tile, seed)), tile, std::time::Instant::now(), target));
                        }
                        dirty = true;
                        continue;
                    }
                    player = target;
                    zoom_active = true;
                    dirty = true;
                    continue;
                }
                // A survey finished: walk into it.
                if surveying.as_ref().map_or(false, |sv| sv.0.is_finished()) {
                    let (handle, _, t0, target) = surveying.take().unwrap();
                    if let Some(p) = pending.take() { let _ = p.join(); }
                    zoom = Some(handle.join().expect("region loader panicked"));
                    status = format!("surveyed in {:.1}s", t0.elapsed().as_secs_f32());
                    player = target;
                    zoom_active = true;
                    dirty = true;
                    continue;
                }
                if surveying.is_some() { dirty = true; }

                if pressed(Key::L) { show_labels = !show_labels; dirty = true; }
                if pressed(Key::J) {
                    // Write the history journal next to the binary's working directory and open it.
                    if let Some(h) = history {
                        let path = std::path::PathBuf::from(format!("journal_{}.html", world.seed()));
                        match crate::lore::journal::write_journal(world, h, &gaz, &path) {
                            Ok(()) => {
                                let opener = if cfg!(target_os = "macos") { "open" } else if cfg!(target_os = "windows") { "explorer" } else { "xdg-open" };
                                let _ = std::process::Command::new(opener).arg(&path).spawn();
                                status = format!("journal written to {}", path.display());
                            }
                            Err(e) => status = format!("could not write journal: {e}"),
                        }
                    } else {
                        status = "no history to write a journal from (run without --no-history)".to_string();
                    }
                }
                if pressed(Key::R) { tw.show_resources = !tw.show_resources; dirty = true; }
                if pressed(Key::O) {
                    let back = window.is_key_down(Key::LeftShift) || window.is_key_down(Key::RightShift);
                    overlay = overlay.cycle(if back { -1 } else { 1 });
                    tw.overlay = overlays::colors(world, overlay);
                    tw.overlay_smooth = overlay.smooth();
                    dirty = true;
                }
                if pressed(Key::C) {
                    auto_season = if auto_season.is_some() { None } else { Some(std::time::Instant::now()) };
                }
                let step_season = pressed(Key::T)
                    || auto_season.map(|t| t.elapsed().as_secs_f32() > SEASON_SECONDS).unwrap_or(false);
                if step_season {
                    season = season.next();
                    tw.set_season(world, season);
                    if auto_season.is_some() { auto_season = Some(std::time::Instant::now()); }
                    dirty = true;
                }
                let place = describe_tile(world, history, &gaz, &landmarks, &tw, tile.0, tile.1);
                // A camp's legend on this tile (`colony::legend`).
                let place = match legends.iter().rev().find(|l| l.tile == tile) {
                    Some(l) => format!("{} | {} ({}, day {}): {}", place, l.name, l.fate, l.day, l.deeds.last().map(|d| d.as_str()).unwrap_or("nothing yet sung of it")),
                    None => place,
                };
                let shown = if overlay == Overlay::None { String::new() } else { format!("{}: {} | ", overlay.name(), overlays::describe(world, overlay, tile.0, tile.1)) };
                let title = format!(
                    "{}({},{}) {} | {:.0} m | {} {}°C | O overlays, T season, C auto{}, L labels, R resources | {}",
                    shown, tile.0, tile.1, place, world.heightmap.get(tile.0, tile.1), season.name(),
                    format!("{:.1}", world.seasonal_climate.as_ref().map(|c| c.get_temperature(tile.0, tile.1, season, tile.1 < tw.height / 2)).unwrap_or(*world.temperature.get(tile.0, tile.1))),
                    if auto_season.is_some() { " ON" } else { "" }, status
                );
                if title != last_title { window.set_title(&title); last_title = title; }
            }

            if dirty {
                if local_active {
                    let (colony, lcam) = local.as_ref().unwrap();
                    if section_view {
                        let half = ((w as f32 / 14.0) as usize / 2).max(8);
                        let cx = lcam.cx as usize;
                        super::local_ink::render_section_ink(colony, lcam.cy as usize, cx.saturating_sub(half), (cx + half).min(colony.map.width), &mut buf, w, h);
                    } else {
                        render_local(&colony.map, &atlas, lcam, &mut buf, w, h);
                        if lcam.surface_view { super::local_ink::draw_colony(colony, lcam, &mut buf, w, h, history); }
                        else { super::local_ink::draw_level(colony, lcam, &mut buf, w, h, history); }
                    }
                    let right = super::colony_ui::reserve_right(&ui, w);
                    let hide_chip = super::colony_ui::over_ui(&ui, w, h, mouse);
                    log_hits = super::colony_hud::draw(colony, lcam, &super::colony_hud::HudState { speed: speed as u32, status: &status, mouse, right, selected: ui.selected, hide_chip, bar: false }, &mut buf, w, h);
                    ui_hits = super::colony_ui::draw(colony, &mut ui, history, speed as u32, mouse, &mut buf, w, h);
                    let card = moment_card.as_ref().and_then(|m| super::colony_hud::draw_moment(m, Some(&colony), &mut buf, w, h));
                    dream_hits = match dream_for { Some(i) => super::colony_ui::dream_buttons(i, &mut buf, w, h, mouse, right), None => Vec::new() };
                    // The refugees' question: its two answers as buttons on the card.
                    if let (Some(r), true) = (card, moment_card.as_ref().map_or(false, |m| m.choice) && colony.refugees_waiting()) {
                        let r = if r.w >= 600 { super::ui::Rect { x: r.x + 120, w: r.w - 120, ..r } } else { r };
                        dream_hits.extend(super::colony_ui::choice_buttons(r, &mut buf, w, h, mouse));
                    }
                    if let Some(tool) = ui.tool { super::colony_hud::draw_hint(tool.prompt(), &mut buf, w, h.saturating_sub(36)); }
                    let prompt = if leave_asked {
                        Some((format!("Leave {}?", colony.name.clone().unwrap_or_else(|| "the camp".into())), "Enter to leave (the patron's acts are saved). Esc to stay.".to_string()))
                    } else if let Some(t) = &name_input {
                        Some(("Name the settlement".to_string(), format!("{}|", t)))
                    } else if let Some((at, t)) = &place_input {
                        Some((format!("Name the place at {},{}", at.0, at.1), format!("{}|", t)))
                    } else if let Some(i) = dream_for {
                        Some((format!("A dream for {}", colony.settlers[i].name), "1 the hut finished   2 plenty   3 rest   4 the watch   (Esc: none)".to_string()))
                    } else { None };
                    if let Some((title, text)) = prompt {
                        let _ = super::colony_hud::draw_moment(&crate::colony::Moment { tick: 0, title, text, because: String::new(), at: (0, 0), choice: false }, None, &mut buf, w, h);
                    }
                    inspect_hits.clear();
                    if let Some(&subject) = inspect.last() {
                        let page = match subject {
                            super::inspector::Subject::Settler(i) => colony.settlers.get(i).map(|st| super::inspector::settler_page_with(history, st, wounded_in(colony, &st.name), &colony.about(i))),
                            super::inspector::Subject::ColonyMark(i) => colony.marks.get(i).map(super::inspector::mark_page),
                            other => history.map(|hist| super::inspector::page(world, hist, other)),
                        };
                        if let Some(page) = page { inspect_hits = super::inspector::draw(&page, &mut buf, w, h, inspect.len()); }
                    }
                } else if zoom_active {
                    let z = zoom.as_ref().unwrap();
                    let cam_z = ZoomCamera {
                        cx: (player.0 - z.origin.0 as f64) as f32,
                        cy: (player.1 - z.origin.1 as f64) as f32,
                        px_per_cell,
                    };
                    render_zoom(&z.rgb, z.region.width, z.region.height, &cam_z, &mut buf, w, h);
                    if let Some(l) = &z.lore { draw_region_labels(l, &cam_z, &mut buf, w, h); }
                    // The playable area an embark here would cover.
                    let half = (LOCAL_SIZE as f32 * TILE_M / z.region.cell_m * px_per_cell / 2.0).max(6.0);
                    draw_box(&mut buf, w, h, w as f32 / 2.0, h as f32 / 2.0, half, 0x00F0_D23C);
                    draw_marker(&mut buf, w, h, w as f32 / 2.0, h as f32 / 2.0, (px_per_cell * 0.6).clamp(4.0, 10.0));
                } else {
                    render_world(&tw, &atlas, &cam, &mut buf, w, h);
                    super::world_ink::draw(&world_life, &cam, tw.width, &mut buf, w, h);
                    super::beasts::draw_world(&world_beasts, &cam, tw.width, &mut buf, w, h, false);
                    if show_labels {
                        let avoid = if show_minimap { vec![minimap_box(tw.width, tw.height, w, h)] } else { Vec::new() };
                        draw_labels_avoiding(&labels, &cam, tw.width, &mut buf, w, h, &avoid);
                    }
                    draw_notes(&notes, note_input.as_ref(), &cam, tw.width, &mut buf, w, h);
                    overlays::draw_legend(&mut buf, w, h, overlay);
                    // Colonies left behind: a gold ring and their name.
                    for (c, _) in &left {
                        let (tx, ty) = c.map.world_tile;
                        let (sx, sy) = ((tx as f32 + 0.5 - cam.cx) * cam.tile_px + w as f32 / 2.0, (ty as f32 + 0.5 - cam.cy) * cam.tile_px + h as f32 / 2.0);
                        if sx < 0.0 || sy < 0.0 || sx >= w as f32 || sy >= h as f32 { continue; }
                        for k in 0..48 {
                            let a = k as f32 / 48.0 * std::f32::consts::TAU;
                            let r = (cam.tile_px * 0.6).max(6.0);
                            super::ui::blend_px(&mut buf, w, h, (sx + r * a.cos()) as i64, (sy + r * a.sin()) as i64, super::ui::GOLD, 1.0);
                        }
                        let name = c.name.clone().unwrap_or_else(|| format!("The camp, day {}", c.clock.day()));
                        super::fonts::draw(&mut buf, w, h, sx + 10.0, sy - 8.0, &name, super::fonts::Face::Italic, 14.0, 0.0, 0x0030_1E14, Some(0x00EE_E4CC));
                    }
                    // Camps of earlier sessions, from the legends: a fainter dashed ring.
                    for l in legends.iter().filter(|l| !left.iter().any(|(c, _)| c.map.world_tile == l.tile)) {
                        let (tx, ty) = l.tile;
                        let (sx, sy) = ((tx as f32 + 0.5 - cam.cx) * cam.tile_px + w as f32 / 2.0, (ty as f32 + 0.5 - cam.cy) * cam.tile_px + h as f32 / 2.0);
                        if sx < 0.0 || sy < 0.0 || sx >= w as f32 || sy >= h as f32 { continue; }
                        for k in 0..48 {
                            if k % 4 >= 2 { continue; }
                            let a = k as f32 / 48.0 * std::f32::consts::TAU;
                            let r = (cam.tile_px * 0.6).max(6.0);
                            super::ui::blend_px(&mut buf, w, h, (sx + r * a.cos()) as i64, (sy + r * a.sin()) as i64, super::ui::GOLD, 0.8);
                        }
                        super::fonts::draw(&mut buf, w, h, sx + 10.0, sy - 8.0, &super::colony_hud::capitalize_pub(&l.name), super::fonts::Face::Italic, 14.0, 0.0, 0x0030_1E14, Some(0x00EE_E4CC));
                    }
                    // How to begin, on the map itself.
                    if let Some((_, t, t0, _)) = &surveying {
                        super::colony_hud::draw_hint(&format!("Surveying the land around {},{}... {:.0} s", t.0, t.1, t0.elapsed().as_secs_f32()), &mut buf, w, h);
                    } else if inspect.is_empty() { super::colony_hud::draw_hint("Z: walk into the land under the mouse.   Enter there: settle.", &mut buf, w, h); }
                    minimap_rect = if show_minimap {
                        let (hx, hy) = screen_to_world(&cam, mouse.0, mouse.1, w, h);
                        render_minimap(&tw, &cam, Some((hx, hy, zoom_tiles as f32 / 2.0)), &mut buf, w, h)
                    } else {
                        (0, 0, 0, 0)
                    };
                    inspect_hits.clear();
                    if let (Some(&subject), Some(hist)) = (inspect.last(), history) {
                        let page = super::inspector::page(world, hist, subject);
                        inspect_hits = super::inspector::draw(&page, &mut buf, w, h, inspect.len());
                    }
                }
                window.update_with_buffer(&buf, w, h)?;
                dirty = false;
            } else {
                window.update();
                // Keep polling while a region loads so the swap shows up promptly.
                if pending.is_some() { dirty = pending.as_ref().map(|p| p.is_finished()).unwrap_or(false); }
            }
        }
        if let Some(p) = pending.take() { let _ = p.join(); }
        Ok(())
    })
}

/// Headless check of the inspector as the window shows it: the map at 16 px/tile around `tile`
/// with the panel open on it, then one frame per followed link (`follow`: clickable line
/// numbers, 0 = the first). Writes `<prefix>_0.png`, `<prefix>_1.png`, ...
pub fn save_inspect_snapshots(world: &WorldData, history: &WorldHistory, atlas: &Atlas, tile: (usize, usize), follow: &[usize], prefix: &str) -> Result<Vec<String>, Box<dyn Error>> {
    use super::inspector::{draw, page, Subject};
    let mut tw = TileWorld::build(world, atlas);
    tw.apply_history(world, history, atlas);
    let gaz = build_gazetteer(world, Some(history), world.seed());
    let landmarks = crate::lore::find_landmarks(world, &gaz);
    let labels = build_labels(world, Some(history), &gaz, &landmarks);
    let (w, h) = (1280usize, 800usize);
    // Centre the tile in the part of the window the panel leaves free.
    let panel = super::inspector::panel_rect(w, h);
    let cam = Camera { cx: tile.0 as f32 + 0.5 + (panel.w as f32 / 2.0) / 16.0, cy: clamp_cy(tile.1 as f32 + 0.5, 16.0, 800, world.height), tile_px: 16.0 };
    let mut map = vec![0u32; w * h];
    render_world(&tw, atlas, &cam, &mut map, w, h);
    super::world_ink::draw(&super::world_ink::world_life(history), &cam, tw.width, &mut map, w, h);
    super::beasts::draw_world(&super::beasts::world_beasts(history), &cam, tw.width, &mut map, w, h, false);
    draw_labels(&labels, &cam, tw.width, &mut map, w, h);
    let (mx, my) = ((w - panel.w) as f32 / 2.0, h as f32 / 2.0);
    draw_marker(&mut map, w, h, mx, my, 7.0);
    let mut subject = Subject::Tile(tile.0, tile.1);
    let mut written = Vec::new();
    for step in 0..=follow.len() {
        let mut buf = map.clone();
        let p = page(world, history, subject);
        let hits = draw(&p, &mut buf, w, h, step + 1);
        let path = format!("{prefix}_{step}.png");
        save_rgb_png(&path, w, h, |x, y| { let q = buf[y * w + x]; [(q >> 16) as u8, (q >> 8) as u8, q as u8] });
        println!("Inspector page {}: {} ({} links)", step, p.title, hits.len());
        written.push(path);
        let Some(hit) = follow.get(step).and_then(|&k| hits.get(k)) else { break };
        subject = hit.to;
    }
    Ok(written)
}

/// What a plate of the world map says: the world's name (its largest continent), the year and
/// season, its one sentence, and the realms with most land in view.
fn plate_info(world: &WorldData, history: Option<&WorldHistory>, gaz: &crate::lore::Gazetteer, tw: &TileWorld, cam: &Camera, w: usize, h: usize, season: crate::seasons::Season) -> super::plates::PlateInfo {
    let world_name = gaz.features.iter().filter(|f| f.kind == crate::lore::FeatureKind::Continent)
        .max_by_key(|f| f.size).map(|f| f.name.clone()).unwrap_or_else(|| "the World".into());
    let caption = match history {
        Some(h) => crate::lore::claims::sentence(&world_name, &crate::lore::claims::claims(h)),
        None => format!("{}, a world without written history.", world_name),
    };
    let mut realms: Vec<(String, super::heraldry::Arms)> = Vec::new();
    if let Some(hist) = history {
        let mut count: std::collections::HashMap<u64, usize> = std::collections::HashMap::new();
        for sy in (0..h).step_by(8) {
            for sx in (0..w).step_by(8) {
                let (x, y) = screen_to_world(cam, sx as f32, sy as f32, w, h);
                if y < 0.0 || y >= tw.height as f32 { continue; }
                let i = y as usize * tw.width + (x.floor() as i64).rem_euclid(tw.width as i64) as usize;
                let o = tw.owner[i];
                if o != u64::MAX && !tw.ground[i].is_water() { *count.entry(o).or_default() += 1; }
            }
        }
        let mut v: Vec<(u64, usize)> = count.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        for (o, _) in v.into_iter().take(5) {
            if let Some(f) = hist.factions.values().find(|f| f.id.0 as u64 == o) {
                realms.push((f.name.clone(), super::heraldry::arms_of(world, hist, f.id)));
            }
        }
    }
    super::plates::PlateInfo {
        world_name, year: history.map(|h| h.current_date.year), season: format!("{:?}", season),
        seed: world.seed(), caption, realms,
    }
}

/// The ink map as a poster (`--poster FILE`): the whole world at `width` px with its lettering
/// scaled up, realm borders, the Shadow's dominion, ruins, the great battles with their years, a
/// compass rose in open sea, a scale bar, a ruled border and a cartouche with the world's name,
/// the year and its one sentence.
/// The ink map of the whole world, unlabelled, about `width` px wide (2 px a tile at least): the
/// legends' map, where sites and realms are pinned (`lore::legends`).
pub fn map_image(world: &WorldData, history: Option<&WorldHistory>, atlas: &Atlas, width: usize) -> image::RgbImage {
    let mut tw = TileWorld::build(world, atlas);
    tw.set_season(world, crate::seasons::Season::Summer);
    if let Some(h) = history { tw.apply_history(world, h, atlas); }
    let tile_px = (width / world.width).max(2) as f32;
    let (w, h) = ((tile_px as usize) * world.width, (tile_px as usize) * world.height);
    let cam = Camera { cx: world.width as f32 / 2.0, cy: world.height as f32 / 2.0, tile_px };
    let mut buf = vec![0u32; w * h];
    render_world(&tw, atlas, &cam, &mut buf, w, h);
    image::RgbImage::from_fn(w as u32, h as u32, |x, y| { let p = buf[y as usize * w + x as usize]; image::Rgb([(p >> 16) as u8, (p >> 8) as u8, p as u8]) })
}

pub fn save_poster(world: &WorldData, history: Option<&WorldHistory>, atlas: &Atlas, path: &str, width: usize) -> Result<(usize, usize), Box<dyn Error>> {
    use super::fonts::{self, Face};
    let t0 = std::time::Instant::now();
    let mut tw = TileWorld::build(world, atlas);
    tw.set_season(world, crate::seasons::Season::Summer);
    if let Some(h) = history { tw.apply_history(world, h, atlas); }
    let gaz = build_gazetteer(world, history, world.seed());
    let landmarks = crate::lore::find_landmarks(world, &gaz);
    let labels = build_labels(world, history, &gaz, &landmarks);
    let tile_px = (width / world.width).max(2) as f32;
    let (w, h) = ((tile_px as usize) * world.width, (tile_px as usize) * world.height);
    let cam = Camera { cx: world.width as f32 / 2.0, cy: world.height as f32 / 2.0, tile_px };
    let mut buf = vec![0u32; w * h];
    render_world(&tw, atlas, &cam, &mut buf, w, h);
    if let Some(hh) = history { super::world_ink::draw(&super::world_ink::world_life(hh), &cam, tw.width, &mut buf, w, h); super::beasts::draw_world(&super::beasts::world_beasts(hh), &cam, tw.width, &mut buf, w, h, false); }
    let font_scale = w as f32 / 2200.0;
    let to_screen = |x: f32, y: f32| (w as f32 / 2.0 + (x - cam.cx) * tile_px, h as f32 / 2.0 + (y - cam.cy) * tile_px);

    // The great battles: crossed swords and the year, the bloodiest forty.
    let mut reserved: Vec<(i64, i64, i64, i64)> = Vec::new();
    if let Some(hist) = history {
        let fell = |d: &str| -> u32 {
            let Some(i) = d.find(" fell, ") else { return 0 };
            let a = d[..i].rsplit('(').next().and_then(|x| x.split_whitespace().next()).and_then(|n| n.parse::<u32>().ok()).unwrap_or(0);
            a + d[i + 7..].split_whitespace().next().and_then(|n| n.parse::<u32>().ok()).unwrap_or(0)
        };
        let mut battles: Vec<(u32, &crate::history::events::types::Event)> = hist.chronicle.events.iter()
            .filter(|e| e.event_type == crate::history::events::types::EventType::BattleFought && e.location.is_some())
            .map(|e| (fell(&e.description), e)).collect();
        battles.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.id.cmp(&b.1.id)));
        let mut seen: Vec<(usize, usize)> = Vec::new();
        for (_, e) in battles {
            let at = e.location.unwrap();
            if seen.iter().any(|s| s.0.abs_diff(at.0) + s.1.abs_diff(at.1) < 4) { continue; }
            seen.push(at);
            if seen.len() > 40 { break; }
            let (sx, sy) = to_screen(at.0 as f32 + 0.5, at.1 as f32 + 0.5);
            let r = tile_px * 0.45;
            for k in -2..=2 {
                let o = k as f32 * 0.5;
                for t in 0..(r * 3.0) as i32 {
                    let f = t as f32 / (r * 3.0) * 2.0 - 1.0;
                    for (x, y) in [(sx + f * r + o, sy + f * r), (sx - f * r + o, sy + f * r)] {
                        super::ui::blend_px(&mut buf, w, h, x as i64, y as i64, 0x008A_2A1A, 0.9);
                    }
                }
            }
            let year = format!("{}", e.date.year);
            let px = 11.0 * font_scale;
            let yw = fonts::width(&year, Face::Italic, px, 0.0);
            fonts::draw(&mut buf, w, h, sx - yw / 2.0, sy + r + 2.0, &year, Face::Italic, px, 0.0, 0x008A_2A1A, Some(0x00EE_E4CC));
            reserved.push(((sx - r) as i64, (sy - r) as i64, (sx + r.max(yw / 2.0)) as i64, (sy + r + px * 1.3) as i64));
        }
    }

    // The cartouche, top left: name, year, the world's sentence.
    let world_name = gaz.features.iter().filter(|f| f.kind == crate::lore::FeatureKind::Continent).max_by_key(|f| f.size).map(|f| f.name.clone()).unwrap_or_else(|| "the World".into());
    let title = format!("The Annals of {}", world_name);
    let when = match history { Some(h) => format!("as it stood in the year {}", h.current_date.year), None => "before history".to_string() };
    let sentence = history.map(|h| crate::lore::claims::sentence(&world_name, &crate::lore::claims::claims(h))).unwrap_or_default();
    let (tpx, spx) = (44.0 * font_scale, 15.0 * font_scale);
    let cw = (fonts::width(&title, Face::SmallCaps, tpx, 2.0 * font_scale) + 80.0 * font_scale).max(w as f32 * 0.28) as usize;
    let words_per = ((cw as f32 - 60.0 * font_scale) / fonts::width("abcdefghij", Face::Italic, spx, 0.0) * 10.0) as usize;
    let lines = super::ui::wrap(&sentence, words_per.max(20));
    let ch = (tpx * 1.6 + spx * 2.4 + spx * 1.35 * lines.len() as f32 + 40.0 * font_scale) as usize;
    let card = super::ui::Rect { x: (60.0 * font_scale) as usize, y: (60.0 * font_scale) as usize, w: cw, h: ch };
    super::ui::card(&mut buf, w, card);
    let mut y = card.y as f32 + 20.0 * font_scale;
    fonts::draw(&mut buf, w, h, card.x as f32 + 30.0 * font_scale, y, &title, Face::SmallCaps, tpx, 2.0 * font_scale, 0x009A_2A1E, None);
    y += tpx * 1.3;
    fonts::draw(&mut buf, w, h, card.x as f32 + 32.0 * font_scale, y, &when, Face::Italic, spx * 1.2, 0.0, 0x0038_2A20, None);
    y += spx * 2.2;
    for l in &lines {
        fonts::draw(&mut buf, w, h, card.x as f32 + 32.0 * font_scale, y, l, Face::Italic, spx, 0.0, 0x0038_2A20, None);
        y += spx * 1.35;
    }
    reserved.push((card.x as i64, card.y as i64, (card.x + card.w) as i64, (card.y + card.h) as i64));

    // The scale bar, bottom left: 0 - 1000 - 2000 km in alternating ink.
    let km_tile = 40_075.0 / world.width as f32;
    // A round step whose segment is wide enough for its number.
    let step_km = [50.0f32, 100.0, 250.0, 500.0, 1000.0, 2000.0].into_iter()
        .find(|k| k / km_tile * tile_px >= 50.0 * font_scale).unwrap_or(2000.0);
    let seg = step_km / km_tile * tile_px;
    let (bx, by) = ((80.0 * font_scale) as usize, h - (110.0 * font_scale) as usize);
    let bh = (8.0 * font_scale) as usize;
    for k in 0..4 {
        let r = super::ui::Rect { x: bx + (k as f32 * seg) as usize, y: by, w: seg as usize, h: bh };
        super::ui::fill(&mut buf, w, r, if k % 2 == 0 { 0x0038_2A20 } else { 0x00EE_E4CC });
        super::ui::outline(&mut buf, w, r, 0x0038_2A20);
        let label = format!("{}", (k as f32 * step_km) as u32);
        fonts::draw(&mut buf, w, h, r.x as f32 - 4.0, (by + bh) as f32 + 4.0, &label, Face::Roman, 12.0 * font_scale, 0.0, 0x0038_2A20, Some(0x00EE_E4CC));
    }
    let last = format!("{} km", (4.0 * step_km) as u32);
    fonts::draw(&mut buf, w, h, bx as f32 + 4.0 * seg - 4.0, (by + bh) as f32 + 4.0, &last, Face::Roman, 12.0 * font_scale, 0.0, 0x0038_2A20, Some(0x00EE_E4CC));
    reserved.push((bx as i64 - 10, by as i64 - 10, (bx as f32 + 4.0 * seg + 120.0 * font_scale) as i64, (by + bh) as i64 + (40.0 * font_scale) as i64));

    super::text::place_labels_scaled(&labels, tile_px, font_scale, w, h, &mut buf, &reserved, to_screen);

    // Compass rose in the open sea, and the ruled border.
    let mut img = image::RgbImage::from_fn(w as u32, h as u32, |x, y| { let p = buf[y as usize * w + x as usize]; image::Rgb([(p >> 16) as u8, (p >> 8) as u8, p as u8]) });
    if let Some(&(ox, oy)) = crate::cartography::decorations::find_ocean_centers(&world.heightmap, 1).first() {
        let (sx, sy) = to_screen(ox as f32 + 0.5, oy as f32 + 0.5);
        crate::cartography::decorations::render_compass_rose(&mut img, (sx as usize, sy as usize), h as f32 / 14.0);
    }
    crate::cartography::decorations::render_vintage_border(&mut img, (24.0 * font_scale) as usize);
    img.save(path)?;
    println!("Poster: {}x{} written to {} in {:.1}s", w, h, path, t0.elapsed().as_secs_f32());
    Ok((w, h))
}

/// A typed key as a character (letters, digits, space and a little punctuation).
fn key_char(k: Key, shift: bool) -> Option<char> {
    let c = match k {
        Key::A => 'a', Key::B => 'b', Key::C => 'c', Key::D => 'd', Key::E => 'e', Key::F => 'f', Key::G => 'g',
        Key::H => 'h', Key::I => 'i', Key::J => 'j', Key::K => 'k', Key::L => 'l', Key::M => 'm', Key::N => 'n',
        Key::O => 'o', Key::P => 'p', Key::Q => 'q', Key::R => 'r', Key::S => 's', Key::T => 't', Key::U => 'u',
        Key::V => 'v', Key::W => 'w', Key::X => 'x', Key::Y => 'y', Key::Z => 'z',
        Key::Key0 => '0', Key::Key1 => '1', Key::Key2 => '2', Key::Key3 => '3', Key::Key4 => '4',
        Key::Key5 => '5', Key::Key6 => '6', Key::Key7 => '7', Key::Key8 => '8', Key::Key9 => '9',
        Key::Space => ' ', Key::Period => '.', Key::Comma => ',', Key::Apostrophe => '\'', Key::Minus => '-', Key::Semicolon => ';',
        _ => return None,
    };
    Some(if shift { c.to_ascii_uppercase() } else { c })
}

/// The player's notes on the world map: an ink pin and the words in a hand.
fn draw_notes(notes: &[crate::lore::notes::Note], typing: Option<&((usize, usize), String)>, cam: &Camera, world_w: usize, buf: &mut [u32], w: usize, h: usize) {
    use super::fonts::{self, Face};
    if cam.tile_px < 2.0 { return; }
    let ww = world_w as f32;
    let to_screen = |x: f32, y: f32| {
        let mut dx = x - cam.cx;
        if dx > ww / 2.0 { dx -= ww; }
        if dx < -ww / 2.0 { dx += ww; }
        (w as f32 / 2.0 + dx * cam.tile_px, h as f32 / 2.0 + (y - cam.cy) * cam.tile_px)
    };
    let mut all: Vec<(usize, usize, String)> = notes.iter().map(|n| (n.x, n.y, n.text.clone())).collect();
    if let Some(((x, y), t)) = typing { all.push((*x, *y, format!("{}|", t))); }
    for (x, y, text) in all {
        let (sx, sy) = to_screen(x as f32 + 0.5, y as f32 + 0.5);
        if sx < -200.0 || sy < -40.0 || sx > w as f32 + 10.0 || sy > h as f32 + 40.0 { continue; }
        for dy in -3i64..=3 { for dx in -3i64..=3 {
            if dx * dx + dy * dy <= 9 { super::ui::blend_px(buf, w, h, sx as i64 + dx, sy as i64 + dy, if dx * dx + dy * dy >= 6 { 0x0020_2A40 } else { 0x00B0_3020 }, 1.0); }
        } }
        fonts::draw(buf, w, h, sx + 8.0, sy - 14.0, &text, Face::Hand, 19.0, 0.0, 0x002C_4A7A, Some(0x00EE_E4CC));
    }
}

/// Walking pace for travel times on the province map: a loaded party on foot, in km a day.
const KM_PER_DAY: f64 = 25.0;
/// Paths wind: straight-line distance times this is the distance walked.
const WINDING: f64 = 1.3;

/// The colony's theatre (the play-scale decision in ROADMAP, Update 5): the region around the
/// embark at `tile` with its three nearest living places, a line to each and the days to walk
/// it. Writes `<prefix>_province.png` and prints the places.
pub fn save_province_snapshot(world: &WorldData, history: Option<&WorldHistory>, tile: (usize, usize), prefix: &str) -> Result<String, Box<dyn Error>> {
    let zs = load_region(world, history, tile, world.seed());
    let s = cells_per_tile() as f64;
    let (ex, ey) = (tile.0 as f64 * s + s / 2.0 - zs.origin.0 as f64, tile.1 as f64 * s + s / 2.0 - zs.origin.1 as f64);
    let km = zs.region.cell_m as f64 / 1000.0;
    let lore = zs.lore.as_ref().ok_or("--province-snapshot needs a history")?;
    let mut near: Vec<(f64, &crate::lore::settle::Site)> = lore.sites.iter()
        .filter(|st| st.destroyed_year.is_none())
        .map(|st| (((st.x - ex).powi(2) + (st.y - ey).powi(2)).sqrt() * km, st))
        .collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    near.truncate(3);
    // Frame the embark and the three places, with a margin.
    let (mut x0, mut y0, mut x1, mut y1) = (ex, ey, ex, ey);
    for (_, st) in &near { x0 = x0.min(st.x); y0 = y0.min(st.y); x1 = x1.max(st.x); y1 = y1.max(st.y); }
    let span = ((x1 - x0).max(y1 - y0) * 1.3).max(250.0 / km);
    let (w, h) = (1100usize, 800usize);
    let cam = ZoomCamera { cx: ((x0 + x1) / 2.0) as f32, cy: ((y0 + y1) / 2.0) as f32, px_per_cell: (h as f64 / span) as f32 };
    let mut buf = vec![0u32; w * h];
    render_zoom(&zs.rgb, zs.region.width, zs.region.height, &cam, &mut buf, w, h);
    draw_region_labels(lore, &cam, &mut buf, w, h);
    let to_screen = |x: f64, y: f64| ((w as f32 / 2.0 + (x as f32 - cam.cx) * cam.px_per_cell), (h as f32 / 2.0 + (y as f32 - cam.cy) * cam.px_per_cell));
    let (sx, sy) = to_screen(ex, ey);
    // The 250 km theatre as a ring.
    let r = (250.0 / km) as f32 * cam.px_per_cell;
    for k in 0..720 {
        let a = k as f32 / 720.0 * std::f32::consts::TAU;
        if k % 6 < 3 { blend(&mut buf, w, h, sx + r * a.cos(), sy + r * a.sin(), 0x0030_1E14, 0.8); }
    }
    let mut lines = Vec::new();
    for (d, st) in &near {
        let (tx, ty) = to_screen(st.x, st.y);
        let n = ((tx - sx).hypot(ty - sy) / 3.0) as i32;
        for k in 0..n {
            if k % 5 < 3 {
                let f = k as f32 / n as f32;
                let (px, py) = (sx + (tx - sx) * f, sy + (ty - sy) * f);
                for (ox, oy) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] { blend(&mut buf, w, h, px + ox, py + oy, 0x008A_2A1A, 0.95); }
            }
        }
        let days = (d * WINDING / KM_PER_DAY).ceil();
        let label = format!("{} days", days);
        let (mx, my) = ((sx + tx) / 2.0, (sy + ty) / 2.0);
        let tw = super::text::text_width(&label, 1) as i64;
        for yy in -2..12i64 { for xx in -3..tw + 3 { blend(&mut buf, w, h, mx + xx as f32 - tw as f32 / 2.0, my + yy as f32, 0x00EF_E3C8, 0.85); } }
        super::text::draw_ink(&mut buf, w, h, mx as i64 - tw / 2, my as i64, &label, 0x008A_2A1A, 1, true);
        lines.push(format!("{} ({:?}, {} souls): {:.0} km, {} days on foot", st.name, st.kind, st.population, d, days));
    }
    draw_box(&mut buf, w, h, sx, sy, 8.0, 0x00F0_D23C);
    let title = format!("The theatre of the embark at {},{}: 250 km ring, nearest places", tile.0, tile.1);
    super::text::draw_ink(&mut buf, w, h, 12, 10, &title, 0x0030_1E14, 1, true);
    let path = format!("{prefix}_province.png");
    save_rgb_png(&path, w, h, |x, y| { let p = buf[y * w + x]; [(p >> 16) as u8, (p >> 8) as u8, p as u8] });
    println!("Province of the embark at {},{} ({:.1} km per region cell, {:.0} km per world tile):", tile.0, tile.1, km, km * s);
    for l in &lines { println!("  {l}"); }
    Ok(path)
}

fn blend(buf: &mut [u32], w: usize, h: usize, x: f32, y: f32, c: u32, a: f32) {
    if x < 0.0 || y < 0.0 || x as usize >= w || y as usize >= h { return; }
    let i = y as usize * w + x as usize;
    let mix = |sh: u32| -> u32 { let (p, q) = ((buf[i] >> sh & 255) as f32, (c >> sh & 255) as f32); ((p + (q - p) * a) as u32) << sh };
    buf[i] = mix(16) | mix(8) | mix(0);
}

/// Try the patron's verbs on the colony at `tile` (`--sim-patron`) and print the results.
pub fn patron_trial(world: &WorldData, history: Option<&WorldHistory>, tile: (usize, usize)) {
    let zs = load_region(world, history, tile, world.seed());
    let s = cells_per_tile() as f64;
    let (ex, ey) = (tile.0 as f64 * s + s / 2.0 - zs.origin.0 as f64, tile.1 as f64 * s + s / 2.0 - zs.origin.1 as f64);
    let map = crate::local::generate_local(world, &zs.region, zs.lore.as_ref(), ex, ey);
    let seed = world.seed() ^ ((tile.0 as u64) << 20) ^ tile.1 as u64;
    let mut colony = found_colony(map, history, tile, seed, 7);
    for line in colony.patron_trial() { println!("Patron {line}"); }
    for line in colony.log.iter().filter(|l| l.contains("your doing")) { println!("  {line}"); }
}

/// Run the dev colony twice for 100 days, without founding stones and with them (a hall stone
/// away from the camp, a grove stone on the nearest trees, a shrine), and compare the layouts
/// (`--sim-founding`): where the hall stands, the kept grove, and how much of the picture differs.
pub fn founding_trial(world: &WorldData, history: Option<&WorldHistory>, atlas: &Atlas, tile: (usize, usize), prefix: &str) -> Result<(), Box<dyn Error>> {
    use super::local_ink::draw_colony;
    let zs = load_region(world, history, tile, world.seed());
    let s = cells_per_tile() as f64;
    let (ex, ey) = (tile.0 as f64 * s + s / 2.0 - zs.origin.0 as f64, tile.1 as f64 * s + s / 2.0 - zs.origin.1 as f64);
    let seed = world.seed() ^ ((tile.0 as u64) << 20) ^ tile.1 as u64;
    let mut frames = Vec::new();
    let mut huts = Vec::new();
    for with_stones in [false, true] {
        let map = crate::local::generate_local(world, &zs.region, zs.lore.as_ref(), ex, ey);
        let mut colony = found_colony(map, history, tile, seed, 7);
        let grove_at = colony.nearest_tree_to_camp();
        if with_stones {
            colony.name_colony("Hearthwater");
            let hall = colony.spot_from_camp(18, -10);
            let _ = colony.place_stone(crate::colony::StoneKind::Hall, hall);
            if let Some(g) = grove_at { let _ = colony.place_stone(crate::colony::StoneKind::Grove, g); colony.name_place(g, "the Old Grove"); }
            let shrine = colony.spot_from_camp(-12, 8);
            let _ = colony.place_stone(crate::colony::StoneKind::Shrine, shrine);
        }
        colony.run_days(100);
        let grove_trees = grove_at.map(|g| colony.trees_near(g, crate::colony::GROVE_RADIUS)).unwrap_or(0);
        let hut = colony.hut.as_ref().map(|h| (h.at, h.done));
        let hut_day = colony.log.iter().find(|l| l.contains("finishes the hut")).and_then(|l| l.split(',').next()).unwrap_or("never").to_string();
        println!("Founding {}: hut {:?} (finished {}), {} trees left around {:?}, {} alive",
            if with_stones { "with stones" } else { "without stones" }, hut, hut_day, grove_trees, grove_at, colony.alive());
        huts.push(hut);
        let (w, h) = (1024usize, 1024usize);
        let cam = LocalCamera { cx: colony.camp.0 as f32 + 4.0, cy: colony.camp.1 as f32, tile_px: 8.0, z: 0, surface_view: true };
        let mut buf = vec![0u32; w * h];
        render_local(&colony.map, atlas, &cam, &mut buf, w, h);
        draw_colony(&colony, &cam, &mut buf, w, h, history);
        let path = format!("{prefix}_{}.png", if with_stones { "stones" } else { "plain" });
        save_rgb_png(&path, w, h, |x, y| { let p = buf[y * w + x]; [(p >> 16) as u8, (p >> 8) as u8, p as u8] });
        frames.push(buf);
        for l in colony.log.iter().filter(|l| l.contains("your doing")) { println!("  {l}"); }
    }
    let differ = frames[0].iter().zip(&frames[1]).filter(|(a, b)| a != b).count();
    println!("Founding: layouts differ in {:.1}% of the camp picture; hut {} -> {}", 100.0 * differ as f32 / frames[0].len() as f32,
        huts[0].map(|h| format!("{},{}", h.0 .0, h.0 .1)).unwrap_or_default(), huts[1].map(|h| format!("{},{}", h.0 .0, h.0 .1)).unwrap_or_default());
    Ok(())
}

/// The colony's marks (`--sim-marks PREFIX`): live 30 days on the dev embark (the builders raise
/// their stone when the hut is done), then bury one settler as a death would, and render the
/// camp with the grave's page open.
pub fn marks_trial(world: &WorldData, history: Option<&WorldHistory>, atlas: &Atlas, tile: (usize, usize), prefix: &str) -> Result<(), Box<dyn Error>> {
    use super::local_ink::draw_colony;
    let zs = load_region(world, history, tile, world.seed());
    let s = cells_per_tile() as f64;
    let (ex, ey) = (tile.0 as f64 * s + s / 2.0 - zs.origin.0 as f64, tile.1 as f64 * s + s / 2.0 - zs.origin.1 as f64);
    let map = crate::local::generate_local(world, &zs.region, zs.lore.as_ref(), ex, ey);
    let seed = world.seed() ^ ((tile.0 as u64) << 20) ^ tile.1 as u64;
    let mut colony = found_colony(map, history, tile, seed, 7);
    colony.name_colony("Hearthwater");
    colony.run_days(30);
    colony.bury(6, "of a fever");
    for m in &colony.marks { println!("Mark: {} at {},{}: {}", m.title, m.at.0, m.at.1, m.text); }
    let (w, h) = (1280usize, 800usize);
    let cam = LocalCamera { cx: colony.camp.0 as f32 + 2.0, cy: colony.camp.1 as f32 + 2.0, tile_px: 16.0, z: 0, surface_view: true };
    let mut buf = vec![0u32; w * h];
    render_local(&colony.map, atlas, &cam, &mut buf, w, h);
    draw_colony(&colony, &cam, &mut buf, w, h, history);
    if let Some(g) = colony.marks.iter().find(|m| m.kind == crate::colony::MarkKind::Grave) {
        super::inspector::draw(&super::inspector::mark_page(g), &mut buf, w, h, 1);
    }
    let path = format!("{prefix}_marks.png");
    save_rgb_png(&path, w, h, |x, y| { let p = buf[y * w + x]; [(p >> 16) as u8, (p >> 8) as u8, p as u8] });
    println!("Saved {path}");
    Ok(())
}

/// The night a settler was struck down and lived (the first arc's rescue), for their scar.
pub fn wounded_in(colony: &crate::colony::Colony, name: &str) -> Option<(String, u64)> {
    colony.arc.as_ref()?.events.iter().find(|e| e.title == "The raid" && e.text.contains(&format!("{} was struck down", name)))
        .map(|e| (format!("From the raid of day {}", e.day), e.day))
}

/// The saga page: a colony in one image. Its map, a cartouche (name, days, year, world code),
/// the cast with their peoples' arms, what they were and their fate, and a timeline of the key
/// moments (the log's milestones and the arc). Written by `--sim-snapshot` as `<prefix>_saga.png`.
pub fn saga_plate(world: &WorldData, history: Option<&WorldHistory>, atlas: &Atlas, colony: &crate::colony::Colony, path: &str) -> Result<(), Box<dyn Error>> {
    use super::fonts::{self, Face};
    use super::local_ink::draw_colony;
    let (w, h) = (1600usize, 1000usize);
    let mut buf = vec![0x00EA_DEC4u32; w * h];
    // The map, left: the camp and its surroundings.
    let (mw, mh) = (980usize, 700usize);
    let mut map = vec![0u32; mw * mh];
    let cam = LocalCamera { cx: colony.camp.0 as f32 + 4.0, cy: colony.camp.1 as f32, tile_px: 9.0, z: 0, surface_view: true };
    render_local(&colony.map, atlas, &cam, &mut map, mw, mh);
    draw_colony(colony, &cam, &mut map, mw, mh, history);
    let (mx, my) = (30usize, 140usize);
    for y in 0..mh { for x in 0..mw { buf[(my + y) * w + mx + x] = map[y * mw + x]; } }
    super::ui::outline(&mut buf, w, super::ui::Rect { x: mx - 1, y: my - 1, w: mw + 2, h: mh + 2 }, 0x0038_2A20);
    super::ui::outline(&mut buf, w, super::ui::Rect { x: mx - 5, y: my - 5, w: mw + 10, h: mh + 10 }, 0x0080_6A52);
    // The cartouche.
    let name = colony.name.clone().unwrap_or_else(|| format!("The camp at {},{}", colony.map.world_tile.0, colony.map.world_tile.1));
    let year = history.map(|h| h.current_date.year).unwrap_or(0);
    let days = colony.clock.day().saturating_sub(1).max(1);
    fonts::draw(&mut buf, w, h, 34.0, 26.0, &format!("The Saga of {}", name), Face::SmallCaps, 46.0, 2.0, 0x009A_2A1E, None);
    let code = super::plates::world_code(colony.map.world_tile).unwrap_or_else(|| format!("seed {}", world.seed()));
    fonts::draw(&mut buf, w, h, 36.0, 84.0, &format!("{} days in the year {}  ·  world code {}", days, year, code), Face::Italic, 19.0, 0.0, 0x0038_2A20, None);
    // The cast.
    let (cx0, mut cy) = (1040.0f32, 140.0f32);
    fonts::draw(&mut buf, w, h, cx0, cy, "The Company", Face::SmallCaps, 24.0, 1.5, 0x009A_2A1E, None);
    cy += 36.0;
    for st in &colony.settlers {
        super::portraits::draw(&mut buf, w, h, cx0 as i64 - 14, cy as i64 - 4, 40, &super::portraits::of_settler(st, history, wounded_in(colony, &st.name)));
        if let (Some(hist), Some(f)) = (history, st.past.as_ref().and_then(|p| p.people)) {
            super::heraldry::draw(&mut buf, w, h, cx0 as i64 + 28, cy as i64 + 22, 14, &super::heraldry::arms_of(world, hist, f));
        }
        let fate = if st.alive { "lives".to_string() } else {
            colony.marks.iter().find(|m| m.kind == crate::colony::MarkKind::Grave && m.title.ends_with(&st.name)).map(|m| format!("died on day {}", m.day)).unwrap_or_else(|| "died".into())
        };
        fonts::draw(&mut buf, w, h, cx0 + 46.0, cy, &st.name, Face::Roman, 18.0, 0.0, if st.alive { 0x0038_2A20 } else { 0x009A_2A1E }, None);
        let calling = st.past.as_ref().map(|p| format!("{}, {}; {}", p.age, p.calling, fate)).unwrap_or(fate);
        let short: String = calling.chars().take(58).collect();
        fonts::draw(&mut buf, w, h, cx0 + 46.0, cy + 19.0, &short, Face::Italic, 14.0, 0.0, 0x0080_6A52, None);
        cy += 44.0;
        if cy > 820.0 { break; }
    }
    // The timeline: the camp's moments (each with its weight), and the founding, by day.
    let mut moments: Vec<(u64, String)> = Vec::new();
    let mut titles: Vec<String> = Vec::new();
    if let Some(first) = colony.log.first() { moments.push((1, first.splitn(2, "  ").nth(1).unwrap_or("").to_string())); titles.push("The founding".into()); }
    let mut weights: Vec<u8> = vec![2];
    for m in &colony.moments {
        let t = m.title.as_str();
        // Deaths, raids and slayings first; then works of note, hunts, relics; then the life of
        // the camp (weddings, births, guests, ghosts); then the rest.
        let w = if t.starts_with("The death of") || t == "The raid" || t.ends_with(" is killed") || t.contains("cast out") || t == "The siege" || t.ends_with(" is deposed") || t.ends_with(" swear vengeance") { 0 }
            else if t.contains("artifact") || m.text.contains("has made an artifact") || t.starts_with("The hunt for") || t.ends_with(" is found") || t.starts_with("They break into") || t.contains("goes mad") || t.ends_with(" is born")
                || t.ends_with(" is taken") || t.ends_with(" comes home") || t == "The sally" || t == "The siege lifts" || t == "The armour holds" || t.contains(" rises against ") || t == "Old enemies at the fire" { 1 }
            else if t.starts_with("The wedding") || t.ends_with(" comes") || t.starts_with("The ghost of") || t == "Migrants" || t.ends_with(" stays") || t.starts_with("Water in the rock")
                || t.starts_with("The friendship of") || t.starts_with("A hall for") || t.ends_with(" is founded") || t.ends_with(" among the raiders") || t.ends_with("'s dream") || t.ends_with("'s vow") { 2 }
            else { 3 };
        moments.push((m.tick / crate::colony::TICKS_PER_DAY + 1, m.text.clone()));
        titles.push(m.title.clone());
        weights.push(w);
    }
    let weight_of: Vec<u8> = weights;
    let mut ranked: Vec<(usize, &(u64, String))> = moments.iter().enumerate().collect();
    ranked.sort_by_key(|(i, _)| (weight_of[*i], *i));
    ranked.truncate(10);
    ranked.sort_by_key(|(i, _)| *i);
    let chosen_titles: Vec<String> = ranked.iter().map(|(i, _)| titles.get(*i).cloned().unwrap_or_default()).collect();
    let chosen: Vec<&(u64, String)> = ranked.into_iter().map(|(_, m)| m).collect();
    let (tx0, tx1, ty) = (40.0f32, w as f32 - 40.0, 900.0f32);
    for x in tx0 as usize..tx1 as usize { buf[ty as usize * w + x] = 0x0038_2A20; buf[(ty as usize + 1) * w + x] = 0x0038_2A20; }
    let n = chosen.len().max(1) as f32;
    for (k, (day, text)) in chosen.iter().enumerate() {
        let x = tx0 + (tx1 - tx0) * (k as f32 + 0.5) / n;
        let up = k % 2 == 0;
        for d in 0..14 { let y = if up { ty as usize - d } else { ty as usize + 2 + d }; buf[y * w + x as usize] = 0x0038_2A20; }
        // The moment's roundel on the line (`vignette.rs`), as its card had it.
        {
            let m = crate::colony::Moment { tick: 0, title: chosen_titles[k].clone(), text: text.clone(), because: String::new(), at: (0, 0), choice: false };
            let mut put = |px: i64, py: i64, c: [f32; 3], a: f32| super::ui::blend_px(&mut buf, w, h, px, py, super::ink::pack(c), a);
            super::vignette::draw(&mut put, Some(colony), &m, x, ty, 13.0);
        }
        fonts::draw(&mut buf, w, h, x - 6.0, if up { ty - 40.0 } else { ty + 18.0 }, &format!("Day {}", day), Face::SmallCaps, 13.0, 0.5, 0x009A_2A1E, None);
        // An arc line ("The raid: ...") shows what happened; others their first clause.
        let body = match text.split_once(": ") { Some((head, rest)) if head.len() < 12 => rest, _ => text.as_str() };
        let clause = body.split(['.', ';']).next().unwrap_or("");
        let mut short = String::new();
        for word in clause.split_whitespace() {
            if short.len() + word.len() + 1 > 40 { short.push('…'); break; }
            if !short.is_empty() { short.push(' '); }
            short.push_str(word);
        }
        // Kept inside the page at the right edge.
        let tw = fonts::width(&short, Face::Italic, 13.0, 0.0);
        let lx = (x - 6.0).min(w as f32 - 12.0 - tw).max(8.0);
        fonts::draw(&mut buf, w, h, lx, if up { ty - 26.0 } else { ty + 32.0 }, &short, Face::Italic, 13.0, 0.0, 0x0038_2A20, None);
    }
    save_rgb_png(path, w, h, |x, y| { let p = buf[y * w + x]; [(p >> 16) as u8, (p >> 8) as u8, p as u8] });
    Ok(())
}

/// Sixty unattended days on the colony at `tile` (`--sim-projects`): which projects it set
/// itself, why, and how far each got.
pub fn projects_trial(world: &WorldData, history: Option<&WorldHistory>, tile: (usize, usize), days: u64) {
    let zs = load_region(world, history, tile, world.seed());
    let s = cells_per_tile() as f64;
    let (ex, ey) = (tile.0 as f64 * s + s / 2.0 - zs.origin.0 as f64, tile.1 as f64 * s + s / 2.0 - zs.origin.1 as f64);
    let map = crate::local::generate_local(world, &zs.region, zs.lore.as_ref(), ex, ey);
    let seed = world.seed() ^ ((tile.0 as u64) << 20) ^ tile.1 as u64;
    if !crate::colony::habitable(&map) { println!("Projects at {},{}: the site is under water; no camp can be made here", tile.0, tile.1); return; }
    let sv = crate::colony::survey(&map);
    println!("Survey at {},{}: {} bushes, {} fishing spots, {:.1} meals a day (seven eat {:.1}): {}", tile.0, tile.1, sv.shrubs, sv.fishing, sv.meals_a_day, crate::colony::MEALS_NEEDED,
        match sv.verdict() { Err(e) => format!("refused, {e}"), Ok(Some(h)) => h, Ok(None) => "enough".into() });
    // PLANET_FORCE_CAMP=1 makes camp anyway (to watch what the settlers do about it).
    if sv.verdict().is_err() && std::env::var("PLANET_FORCE_CAMP").is_err() { return; }
    let mut colony = found_colony(map, history, tile, seed, 7);
    if std::env::var("PLANET_HALL_SCAN").is_ok() { println!("  Hall at founding: {}", if colony.plan_hall().is_some() { "a hillside to dig" } else { "flat" }); }
    // PLANET_SCRIPT=FILE: the patron's interventions ("tick verb args") replayed on the way.
    match std::env::var("PLANET_SCRIPT").ok().and_then(|f| std::fs::read_to_string(f).ok()) {
        Some(text) => { let script: Vec<String> = text.lines().map(String::from).collect(); colony.run_days_scripted(days, &script); }
        None => colony.run_days(days),
    }
    // The cold, with and without a woodpile (runs of 30 days or more), and how idle the colony is.
    if days >= 30 {
    let (with, without) = crate::colony::Colony::cold_trial(found_colony(crate::local::generate_local(world, &zs.region, zs.lore.as_ref(), ex, ey), history, tile, seed, 7),
        found_colony(crate::local::generate_local(world, &zs.region, zs.lore.as_ref(), ex, ey), history, tile, seed, 7));
    println!("Cold: {} nights ended chilled in 120 days (a winter) with a woodpile, {} with its ground forbidden", with, without);
    }
    let idle = colony.decisions.iter().filter(|d| d.contains("Nothing needs doing")).count();
    println!("Idle: {:.0}% of {} decisions were 'Nothing needs doing'", 100.0 * idle as f32 / colony.decisions.len().max(1) as f32, colony.decisions.len());
    let hut = colony.hut.as_ref().map(|h| format!("{} hut {}", if colony.hut_material == crate::colony::ItemKind::Stone { "stone" } else { "timber" }, if h.done { "built" } else { "unfinished" })).unwrap_or_else(|| "no hut site".into());
    let gone = colony.departed.map(|d| format!("; they left on day {}", d)).unwrap_or_default();
    let moved = colony.log.iter().filter(|l| l.contains("strike camp")).count();
    println!("Projects at {},{} after {} days: {}; {} of {} alive{}{}", tile.0, tile.1, days, hut, colony.alive(), colony.company(), gone,
        if moved > 0 { format!("; the camp moved {} time{}", moved, if moved == 1 { "" } else { "s" }) } else { String::new() });
    let laid_starving = colony.log.iter().filter(|l| l.contains("is starving")).filter_map(|l| l.split(',').next()).any(|d| colony.log.iter().any(|m| m.starts_with(&format!("{},", d)) && (m.contains("sets the first") || m.contains("finishes"))));
    if laid_starving { println!("  Built on a day someone starved"); }
    // How the camp laid itself out and spends its spare hours (`projects::wall_r`, `needs.rs`).
    let acts = colony.decisions.iter().filter(|d| crate::colony::needs::ACT_WORDS.iter().any(|w| d.contains(&format!("wandering        {}", w)))).count();
    println!("  Layout: wall radius {}, spread {:.2}; {} spare-hours acts", colony.wall_r(), colony.camp_spread(), acts);
    for p in &colony.projects {
        println!("  Project day {}: {} ({} of {} {}{}), {}", p.day, p.kind.word(), p.used, p.needed,
            if p.material == crate::colony::ItemKind::Stone { "stones" } else { "logs" }, if p.done { ", done" } else { "" }, p.why);
    }
    if std::env::var("PLANET_HALL_SCAN").is_ok() { println!("  Hall: {}", if colony.plan_hall().is_some() { "a hillside to dig" } else { "flat" }); }
    for p in &colony.map.places {
        // Can someone walk in from the mouth to the far end?
        let walk = p.mouth.zip(p.cells.last()).map_or(false, |(m, c)| crate::colony::nav::path3(&colony.map, None, crate::colony::nav::surface3(&colony.map, m), (c.0 .0, c.0 .1, c.1), 40000).is_some());
        let levels = p.cells.iter().map(|c| c.1).max().unwrap_or(0) - p.cells.iter().map(|c| c.1).min().unwrap_or(0) + 1;
        println!("  Place: {} ({:?}): {}; because {}; {} cells on {} levels, mouth {:?}, {}", p.name, p.kind, p.contents.join("; "), p.cause, p.cells.len(), levels, p.mouth, if walk { "walkable from the surface" } else { "not reached from the surface" });
    }
    if let Some(w) = &colony.were { println!("  Werebeast: {} (full moons every 28 days); {} cursed; {} cast out", w, colony.cursed.len(), colony.log.iter().filter(|l| l.contains(" casts ") || l.contains("casts ") && l.contains(" out")).count()); }
    if colony.darkness > 0.0 { println!("  Darkness: {:.2} under {}; {} graves; the dead rose {} times", colony.darkness, colony.shadow_name.clone().unwrap_or_default(), colony.marks.iter().filter(|m| m.kind == crate::colony::MarkKind::Grave).count(), colony.log.iter().filter(|l| l.contains("rises from the grave")).count()); }
    if let Some(p) = &colony.trade { println!("  Trade: {} caravans from {} ({} days' walk){}", colony.caravans, p.town, p.days, if colony.tools_bought { "; iron tools bought" } else { "" }); }
    if !colony.works.is_empty() {
        let best = colony.works.iter().max_by_key(|w| (w.quality, std::cmp::Reverse(w.day))).unwrap();
        println!("  Works: {} made, {} fine or better, {} showing a world event; best: {} by {}", colony.works.len(), colony.works.iter().filter(|w| w.quality >= 2).count(),
            colony.works.iter().filter(|w| w.image.as_ref().map_or(false, |x| x.1.is_some())).count(), best.describe(), colony.settlers[best.maker].name);
    }
    // The industries (`colony/industry.rs`): their shops and what passed through them.
    {
        use crate::colony::delve::RoomKind as R;
        let ind = &colony.industry;
        let shops: Vec<String> = colony.rooms.iter().filter(|r| matches!(r.kind, R::Mason | R::Carpenter | R::Smelter | R::Forge | R::Kiln)).map(|r| format!("{} (day {})", r.kind.word().trim_start_matches("the "), r.day)).collect();
        if !shops.is_empty() || ind.smelted > 0 || !ind.ore.is_empty() {
            let list = |v: &Vec<(String, u32)>| v.iter().map(|(k, n)| format!("{} {}", n, k)).collect::<Vec<_>>().join(", ");
            println!("  Industry: shops {}; ore left {}; {} loads smelted into {} bars (left {}); {} charcoal burnt; {} stones dressed ({} blocks left); {} barrels; {} fired; {} arms and mail forged; tools {}",
                if shops.is_empty() { "none".to_string() } else { shops.join(", ") }, if ind.ore.is_empty() { "none".to_string() } else { list(&ind.ore) }, ind.smelted, ind.bars_made,
                if ind.bars.is_empty() { "none".to_string() } else { list(&ind.bars) }, ind.burned, ind.dressed, ind.blocks, ind.barrels_made, ind.fired, ind.forged,
                ind.tools.as_ref().map(|(m, d)| format!("of {} (day {})", m, d)).unwrap_or_else(|| if colony.tools_bought { "bought".into() } else { "none".into() }));
        }
    }
    for c in &colony.map.caverns {
        println!("  Cavern: {}, {} levels down, {} cells of floor ({} fungus trees, {} under water): {}{}", c.name, c.depth_levels, c.floor_cells, c.fungus, c.water_cells, c.life.join(", "),
            c.beast.as_ref().map(|(n, m)| format!("; {} the {} sleeps there: {}", n, m.kind_word, m.description)).unwrap_or_default());
    }
    if let Some(m) = colony.map.magma_top { println!("  Magma: a sea of it at levels 1-{}{}{}", m, colony.map.magma_pipe.map(|p| format!("; a pipe rises at {},{} (volcanic ground)", p.0, p.1)).unwrap_or_default(), if colony.magma_forge { "; the deep shaft has reached it, and metal is forged at the magma" } else { "" }); }
    let game: Vec<String> = colony.map.game.iter().map(|(n, h)| format!("{} {}", h, n)).collect();
    println!("  Game: {} on the land; {} brought down", if game.is_empty() { "none".to_string() } else { game.join(", ") }, colony.hunted);
    if colony.stone_dug > 0 || colony.dug_cells() > 0 {
        println!("  Dug: {} cells under rock, {} stone loads dug out, {} seams struck; {} sleep in the hall; aquifer {}", colony.dug_cells(), colony.stone_dug, colony.ore_found, if colony.hall_cells.is_empty() { 0 } else { colony.alive() },
            match (colony.map.aquifer, colony.aquifer_struck) { (None, _) => "none".to_string(), (Some((lo, hi)), None) => format!("at levels {}-{}, untouched", lo, hi), (Some((lo, hi)), Some(d)) => format!("at levels {}-{}, struck on day {}{}", lo, hi, d, if colony.aquifer_lined { ", lined" } else { "" }) });
    }
    // PLANET_DUMP_LOG=FILE writes the whole log (for reading a long run).
    if let Ok(path) = std::env::var("PLANET_DUMP_LOG") { let _ = std::fs::write(path, colony.log.join("\n") + "\n"); }
    if let Ok(path) = std::env::var("PLANET_ANNALS") { let _ = std::fs::write(path, colony.annals(history)); }
    if let Ok(path) = std::env::var("PLANET_DUMP_DECISIONS") { let _ = std::fs::write(path, colony.decisions.join("\n") + "\n"); }
    // PLANET_FRAMES=PREFIX: the camp seen below: a section through the delve's stair (or the
    // dig, or the camp), each level with rooms on it, and the surface above.
    if let Ok(prefix) = std::env::var("PLANET_FRAMES") {
        let w0 = colony.map.width;
        let at = colony.spine.map(|s| s.at).or(colony.hall_cells.first().copied()).unwrap_or(colony.camp);
        let (w, h) = (1280usize, 800usize);
        let mut buf = vec![0u32; w * h];
        let (x0, x1) = ((at.0 as usize).min(colony.camp.0 as usize).saturating_sub(14), ((at.0 as usize).max(colony.camp.0 as usize) + 14).min(w0));
        super::local_ink::render_section_ink(&colony, at.1 as usize, x0, x1, &mut buf, w, h);
        save_rgb_png(&format!("{prefix}_section.png"), w, h, |x, y| { let q = buf[y * w + x]; [(q >> 16) as u8, (q >> 8) as u8, q as u8] });
        let mut levels: Vec<i32> = colony.rooms.iter().map(|r| r.z).chain(colony.tower.map(|t| t.1)).collect();
        levels.sort_unstable_by(|a, b| b.cmp(a));
        levels.dedup();
        if levels.is_empty() { levels.push(colony.map.surface_z[at.1 as usize * w0 + at.0 as usize]); }
        let mut names = Vec::new();
        for (n, &z) in levels.iter().enumerate() {
            let cells: Vec<crate::colony::nav::Pos> = colony.rooms.iter().filter(|r| r.z == z).flat_map(|r| r.cells.iter().copied()).chain(colony.tower.filter(|t| t.1 == z).map(|t| t.0)).collect();
            let (mx, my) = if cells.is_empty() { (at.0 as f32, at.1 as f32) } else { let k = cells.len() as f32; cells.iter().fold((0.0, 0.0), |a, c| (a.0 + c.0 as f32 / k, a.1 + c.1 as f32 / k)) };
            let cam = LocalCamera { cx: mx + 0.5, cy: my + 0.5, tile_px: 24.0, z, surface_view: false };
            let mut buf = vec![0u32; w * h];
            super::local_ink::render_level_ink(&colony.map, &cam, &mut buf, w, h);
            super::local_ink::draw_level(&colony, &cam, &mut buf, w, h, history);
            let name = if n == 0 { format!("{prefix}_below.png") } else { format!("{prefix}_below{}.png", n + 1) };
            save_rgb_png(&name, w, h, |x, y| { let q = buf[y * w + x]; [(q >> 16) as u8, (q >> 8) as u8, q as u8] });
            names.push(format!("{} (level {})", name, z));
        }
        // Each breached cavern at the stair's foot, with its life (`colony/cavelife.rs`).
        for &(layer, foot) in colony.cavern_feet_pub() {
            let cam = LocalCamera { cx: foot.0 as f32 + 0.5, cy: foot.1 as f32 + 0.5, tile_px: 20.0, z: foot.2, surface_view: false };
            let mut buf = vec![0u32; w * h];
            super::local_ink::render_level_ink(&colony.map, &cam, &mut buf, w, h);
            super::local_ink::draw_level(&colony, &cam, &mut buf, w, h, history);
            let name = format!("{prefix}_cavern{}.png", layer + 1);
            save_rgb_png(&name, w, h, |x, y| { let q = buf[y * w + x]; [(q >> 16) as u8, (q >> 8) as u8, q as u8] });
            names.push(format!("{} (level {})", name, foot.2));
        }
        let cam = LocalCamera { cx: (at.0 as f32 + colony.camp.0 as f32) / 2.0, cy: (at.1 as f32 + colony.camp.1 as f32) / 2.0, tile_px: 12.0, z: 0, surface_view: true };
        let mut buf = vec![0u32; w * h];
        super::local_ink::render_local_ink(&colony.map, &cam, &mut buf, w, h);
        super::local_ink::draw_colony(&colony, &cam, &mut buf, w, h, history);
        save_rgb_png(&format!("{prefix}_above.png"), w, h, |x, y| { let q = buf[y * w + x]; [(q >> 16) as u8, (q >> 8) as u8, q as u8] });
        // Each place in the hills at its deepest level (`local/places.rs`).
        for (k, pl) in colony.map.places.iter().enumerate().filter(|(_, p)| p.mouth.is_some()) {
            // (Halls: their great hall's level, the busiest, rather than the deepest chamber.)
            let hall_level = if pl.kind == crate::local::places::PlaceKind::Halls { pl.cells.iter().map(|c| c.1).fold(i32::MIN, |m, z| if pl.cells.iter().filter(|c| c.1 == z).count() > pl.cells.iter().filter(|c| c.1 == m).count() { z } else { m }) } else { i32::MIN };
            let Some(&(c, z)) = (if hall_level != i32::MIN { pl.cells.iter().find(|c| c.1 == hall_level) } else { pl.cells.iter().min_by_key(|c| c.1) }) else { continue };
            let cam = LocalCamera { cx: c.0 as f32 + 0.5, cy: c.1 as f32 + 0.5, tile_px: 20.0, z, surface_view: false };
            let mut buf = vec![0u32; w * h];
            super::local_ink::render_level_ink(&colony.map, &cam, &mut buf, w, h);
            super::local_ink::draw_level(&colony, &cam, &mut buf, w, h, history);
            save_rgb_png(&format!("{prefix}_place{}.png", k + 1), w, h, |x, y| { let q = buf[y * w + x]; [(q >> 16) as u8, (q >> 8) as u8, q as u8] });
        }
        // The stair's foot (a cavern's floor once one is breached) and the magma pipe's top, if
        // any: for checking cavern floors and the magma.
        let mut extra: Vec<(String, (u16, u16), i32)> = Vec::new();
        if let Some(sp) = colony.spine { extra.push((format!("{prefix}_foot.png"), sp.at, sp.bottom)); }
        if let Some(p) = colony.map.magma_pipe { let sz = colony.map.surface_z[p.1 as usize * w0 + p.0 as usize]; extra.push((format!("{prefix}_magma.png"), (p.0 as u16, p.1 as u16), sz - 4)); }
        for (name, c, z) in extra {
            let cam = LocalCamera { cx: c.0 as f32 + 0.5, cy: c.1 as f32 + 0.5, tile_px: 20.0, z, surface_view: false };
            let mut buf = vec![0u32; w * h];
            super::local_ink::render_level_ink(&colony.map, &cam, &mut buf, w, h);
            super::local_ink::draw_level(&colony, &cam, &mut buf, w, h, history);
            save_rgb_png(&name, w, h, |x, y| { let q = buf[y * w + x]; [(q >> 16) as u8, (q >> 8) as u8, q as u8] });
        }
        // The camp at its own ground level, as the level view draws it, beside the surface view
        // (to compare the two).
        let zc = colony.map.surface_z[colony.camp.1 as usize * w0 + colony.camp.0 as usize];
        let cam = LocalCamera { cx: colony.camp.0 as f32 + 0.5, cy: colony.camp.1 as f32 + 0.5, tile_px: 16.0, z: zc, surface_view: false };
        let mut buf = vec![0u32; w * h];
        // (Timed best of three, as `--local-snapshot` does.)
        let mut level_ms = f64::MAX;
        for _ in 0..3 {
            let t0 = std::time::Instant::now();
            super::local_ink::render_level_ink(&colony.map, &cam, &mut buf, w, h);
            super::local_ink::draw_level(&colony, &cam, &mut buf, w, h, history);
            level_ms = level_ms.min(t0.elapsed().as_secs_f64() * 1000.0);
        }
        save_rgb_png(&format!("{prefix}_camp_level.png"), w, h, |x, y| { let q = buf[y * w + x]; [(q >> 16) as u8, (q >> 8) as u8, q as u8] });
        let cam = LocalCamera { surface_view: true, ..cam };
        let mut buf = vec![0u32; w * h];
        let mut surface_ms = f64::MAX;
        for _ in 0..3 {
            let t0 = std::time::Instant::now();
            super::local_ink::render_local_ink(&colony.map, &cam, &mut buf, w, h);
            super::local_ink::draw_colony(&colony, &cam, &mut buf, w, h, history);
            surface_ms = surface_ms.min(t0.elapsed().as_secs_f64() * 1000.0);
        }
        save_rgb_png(&format!("{prefix}_camp_surface.png"), w, h, |x, y| { let q = buf[y * w + x]; [(q >> 16) as u8, (q >> 8) as u8, q as u8] });
        // (The ground alone, to tell its cost from the camp drawn over it.)
        let mut ground_ms = f64::MAX;
        for _ in 0..3 { let t0 = std::time::Instant::now(); super::local_ink::render_local_ink(&colony.map, &cam, &mut buf, w, h); ground_ms = ground_ms.min(t0.elapsed().as_secs_f64() * 1000.0); }
        println!("Frame times ({w}x{h} at 16 px, best of three): the camp's level {:.1} ms, its surface {:.1} ms (the ground alone {:.1} ms)", level_ms, surface_ms, ground_ms);
        println!("Frames: {prefix}_section.png, {}, {prefix}_above.png, {prefix}_camp_level.png, {prefix}_camp_surface.png", names.join(", "));
    }
    let deaths: Vec<String> = colony.log.iter().filter(|l| l.contains("died of") || l.contains("was killed") || l.contains("was the last")).map(|l| l.split("  ").next().unwrap_or("").to_string() + ": " + if l.contains("hunger") { "hunger" } else if l.contains("killed") { "raid" } else { "other" }).collect();
    if !deaths.is_empty() { println!("  Deaths: {}", deaths.join("; ")); }
    // The story after the first raid: arc beats since, and the longest stretch without a log line.
    if let Some(a) = colony.arc.as_ref() {
        let first_raid = a.events.iter().find(|e| e.title == "The raid").map_or(u64::MAX, |e| e.day);
        let after: Vec<String> = a.events.iter().filter(|e| e.day > first_raid).map(|e| format!("day {} {}", e.day, e.title)).collect();
        println!("  Arc after the first raid: {} beats ({}), {} chapters", after.len(), after.join(", "), a.chapter + 1);
    }
    let days: Vec<u64> = colony.log.iter().filter_map(|l| l.strip_prefix("Day ").and_then(|r| r.split(',').next()).and_then(|d| d.parse().ok())).collect();
    let gap = days.windows(2).map(|w| w[1] - w[0]).max().unwrap_or(0).max(days.last().map_or(0, |&d| colony.clock.day().saturating_sub(d)));
    println!("  Longest quiet: {} days without a log line", gap);
    if std::env::var("PLANET_DEBUG_CAVE").is_ok() { println!("  Cavern life: {:.1} ms roaming and hunting", crate::colony::creatures::CAVE_NS.load(std::sync::atomic::Ordering::Relaxed) as f64 / 1e6); }
    for l in colony.log.iter().filter(|l| l.contains("Winter comes") || l.contains("Autumn comes") || l.contains("envoy") || l.contains("tribute")) { println!("  {l}"); }
    let ill = colony.log.iter().filter(|l| l.contains("falls ill")).count();
    if ill > 0 { println!("  {} fell ill from the cold", ill); }
}

/// The raid three times on the colony at `tile` (`--sim-raid`): with no patron, a careful one and
/// a careless one; prints each night's outcome.
pub fn raid_trial(world: &WorldData, history: Option<&WorldHistory>, tile: (usize, usize)) {
    use crate::colony::arc::PatronStyle;
    let zs = load_region(world, history, tile, world.seed());
    let s = cells_per_tile() as f64;
    let (ex, ey) = (tile.0 as f64 * s + s / 2.0 - zs.origin.0 as f64, tile.1 as f64 * s + s / 2.0 - zs.origin.1 as f64);
    let seed = world.seed() ^ ((tile.0 as u64) << 20) ^ tile.1 as u64;
    for style in [PatronStyle::Absent, PatronStyle::Careful, PatronStyle::Careless] {
        let map = crate::local::generate_local(world, &zs.region, zs.lore.as_ref(), ex, ey);
        if !crate::colony::habitable(&map) { println!("Raid at {},{}: the site is under water", tile.0, tile.1); return; }
        if let Err(why) = crate::colony::survey(&map).verdict() { println!("Raid at {},{}: no camp, {}", tile.0, tile.1, why); return; }
        let mut colony = found_colony(map, history, tile, seed, 7);
        let line = colony.raid_trial(style);
        let ready = colony.readiness().0;
        let outcome = if line.contains("was killed") { "death" } else if line.contains("dragged them back") { "rescue" } else if line.contains("went away with nothing") { "rout" } else { "none" };
        println!("Raid {:?}: {} (ready {:.2}): {}", style, outcome, ready, line);
    }
}

/// A camp whose berries are gone (`--sim-move`): every bush within reach of the camp is
/// stripped on day 1; the settlers should strike camp for better ground rather than starve.
pub fn move_trial(world: &WorldData, history: Option<&WorldHistory>, tile: (usize, usize), days: u64) {
    let zs = load_region(world, history, tile, world.seed());
    let s = cells_per_tile() as f64;
    let (ex, ey) = (tile.0 as f64 * s + s / 2.0 - zs.origin.0 as f64, tile.1 as f64 * s + s / 2.0 - zs.origin.1 as f64);
    let map = crate::local::generate_local(world, &zs.region, zs.lore.as_ref(), ex, ey);
    let seed = world.seed() ^ ((tile.0 as u64) << 20) ^ tile.1 as u64;
    let mut colony = found_colony(map, history, tile, seed, 7);
    colony.strip_berries(45);
    colony.run_days(days);
    if let Ok(path) = std::env::var("PLANET_DUMP_LOG") { let _ = std::fs::write(path, colony.log.join("\n") + "\n"); }
    if let Ok(path) = std::env::var("PLANET_DUMP_DECISIONS") { let _ = std::fs::write(path, colony.decisions.join("\n") + "\n"); }
    println!("Move at {},{} after {} days: {} of {} alive; the camp at {},{}{}", tile.0, tile.1, days, colony.alive(), colony.company(), colony.camp.0, colony.camp.1,
        colony.departed.map(|d| format!("; they left on day {}", d)).unwrap_or_default());
    for l in colony.log.iter().filter(|l| l.contains("strike camp") || l.contains("starving") || l.contains("died") || l.contains("leave on day")) { println!("  {l}"); }
    for m in colony.moments.iter().filter(|m| m.title == "They move the camp") { println!("  Moment: {} ({})", m.title, m.because); }
    for m in colony.marks.iter().filter(|m| m.title == "The old camp") { println!("  Mark: {} at {},{}: {}", m.title, m.at.0, m.at.1, m.text); }
}

/// The refugees' choice (`--sim-refugees`): the dev colony lives to the refugees twice, takes
/// them in once and turns them away once, and lives on to the raid; prints the beats, the three
/// days after the answer and the raid's line for each.
pub fn refugee_trial(world: &WorldData, history: Option<&WorldHistory>, tile: (usize, usize)) {
    let zs = load_region(world, history, tile, world.seed());
    let s = cells_per_tile() as f64;
    let (ex, ey) = (tile.0 as f64 * s + s / 2.0 - zs.origin.0 as f64, tile.1 as f64 * s + s / 2.0 - zs.origin.1 as f64);
    let seed = world.seed() ^ ((tile.0 as u64) << 20) ^ tile.1 as u64;
    for take in [true, false] {
        let map = crate::local::generate_local(world, &zs.region, zs.lore.as_ref(), ex, ey);
        let mut colony = found_colony(map, history, tile, seed, 7);
        let Some((r, f, d)) = colony.arc.as_ref().map(|a| (a.rumour_day, a.refugee_day, a.raid_day)) else { println!("Refugees: no arc"); return };
        if take { println!("Beats: rumour day {}, refugees day {}, raid day {}", r, f, d); }
        while !colony.refugees_waiting() && colony.clock.day() <= f + 1 { colony.tick(); }
        let from = colony.log.len();
        match colony.answer_refugees(take) { Ok(l) => println!("Refugees {}: {}", if take { "taken in" } else { "turned away" }, l), Err(e) => println!("Refugees: {e}") }
        let end = colony.clock.tick + 3 * crate::colony::TICKS_PER_DAY;
        while colony.clock.tick < end { colony.tick(); }
        for l in &colony.log[from..] { if !l.contains("keeps watch") && !l.contains("spoil") { println!("  {l}"); } }
        let raid = colony.arc.as_ref().map_or(0, |a| a.raid_day);
        while colony.clock.day() <= raid { colony.tick(); }
        let line = colony.arc.as_ref().and_then(|a| crate::colony::arc::ending(a)).map(|e| e.text.clone()).unwrap_or_default();
        println!("  Raid (day {}): {}", raid, line);
        for l in colony.log.iter().filter(|l| l.contains("refugees' fire") || l.contains("cairn")) { println!("  {l}"); }
    }
}

/// A place to settle, offered at the end of the history: where, who the settlers would be, the
/// trouble nearest, and what the land gives and lacks.
#[derive(Clone, Debug)]
pub struct SiteOffer { pub tile: (usize, usize), pub name: String, pub who: String, pub trouble: String, pub land: String, /// Where in the tile to embark (beside its river), if not its centre.
    pub cell: Option<(u64, u64)> }

/// The site the player chose on the watcher's closing card (the viewer embarks there).
static CHOSEN_SITE: std::sync::OnceLock<(usize, usize)> = std::sync::OnceLock::new();
pub fn set_chosen_site(t: (usize, usize)) { let _ = CHOSEN_SITE.set(t); }
pub fn chosen_site() -> Option<(usize, usize)> { CHOSEN_SITE.get().copied() }

/// Three places to settle that differ: scored like the dev embark (a river, woods, high ground,
/// a living town near), then chosen greedily so each differs from those before in its threat or
/// its settlers' people, at least six tiles apart, and livable.
pub fn three_sites(world: &WorldData, history: &WorldHistory) -> Vec<SiteOffer> {
    let (w, h) = (world.width, world.height);
    let towns: Vec<(usize, usize)> = history.settlements.values().filter(|s| !s.is_destroyed()).map(|s| s.location).collect();
    let mut cands: Vec<((usize, usize), f32)> = Vec::new();
    for y in 1..h - 1 {
        for x in 0..w {
            if *world.heightmap.get(x, y) <= 0.0 || world.water_body_map.get(x, y).is_lake() { continue; }
            let t = *world.temperature.get(x, y);
            if !(2.0..=26.0).contains(&t) { continue; }
            let (mut river, mut woods, mut lo, mut hi) = (false, 0, f32::MAX, f32::MIN);
            for dy in -1i64..=1 { for dx in -1i64..=1 {
                let (nx, ny) = ((x as i64 + dx).rem_euclid(w as i64) as usize, (y as i64 + dy) as usize);
                if world.water_body_map.get(nx, ny).is_river() { river = true; }
                if format!("{:?}", world.biomes.get(nx, ny)).contains("Forest") { woods += 1; }
                let e = *world.heightmap.get(nx, ny); lo = lo.min(e); hi = hi.max(e);
            } }
            let near = towns.iter().map(|&(tx, ty)| { let dx = x.abs_diff(tx).min(w - x.abs_diff(tx)); dx.max(y.abs_diff(ty)) }).min().unwrap_or(usize::MAX);
            let score = if river { 3.0 } else { 0.0 } + (woods as f32 / 3.0).min(2.0) + if hi - lo > 300.0 { 1.0 } else { 0.0 }
                + match near { 1..=3 => 2.0, 0 => -5.0, _ => 0.0 };
            cands.push(((x, y), score));
        }
    }
    cands.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let gaz = build_gazetteer(world, Some(history), world.seed());
    let fname = |f: Option<crate::history::FactionId>| f.and_then(|f| history.factions.get(&f)).map(|x| x.name.clone()).unwrap_or_else(|| "no people".into());
    let mut picked: Vec<(SiteOffer, crate::colony::arc::ThreatKind, Option<crate::history::FactionId>)> = Vec::new();
    // Two passes: first three different landscapes (world biomes), then any.
    let biome_of = |t: (usize, usize)| world.biomes.get(t.0, t.1).parent_biome();
    for pass in 0..2 {
    for &(tile, _) in cands.iter().take(80) {
        if picked.len() == 3 { break; }
        if picked.iter().any(|(o, _, _)| o.tile == tile) { continue; }
        if pass == 0 && picked.iter().any(|(o, _, _)| biome_of(o.tile) == biome_of(tile)) { continue; }
        let far = picked.iter().all(|(o, _, _)| { let dx = tile.0.abs_diff(o.tile.0).min(w - tile.0.abs_diff(o.tile.0)); dx.max(tile.1.abs_diff(o.tile.1)) >= 6 });
        if !far { continue; }
        let seed = world.seed() ^ ((tile.0 as u64) << 20) ^ tile.1 as u64;
        let arc = crate::colony::arc::plan(history, tile, seed);
        let roster = crate::history::settlers::roster(history, tile, 7, seed);
        let people = roster.first().and_then(|r| r.1.people);
        if picked.iter().any(|(_, k, p)| *k == arc.threat.kind && *p == people) { continue; }
        // Embark beside the tile's river where it has one (Dwarf Fortress: the world's river runs
        // through the embark tiles it crosses).
        let cell = river_bank(world, Some(history), tile);
        // The first pass offers only sites beside water.
        if pass == 0 && cell.is_none() { continue; }
        let (map, _, _) = colony_site(world, Some(history), tile, cell);
        if !crate::colony::habitable(&map) || crate::colony::survey(&map).verdict().is_err() { continue; }
        let (gift, lack) = crate::local::site::gift_and_lack(&map);
        let callings: Vec<String> = { let mut v: Vec<String> = roster.iter().map(|r| r.1.calling.clone()).collect(); v.dedup(); v.into_iter().take(2).collect() };
        let offer = SiteOffer {
            tile,
            name: format!("{} ({})", gaz.describe(tile.0, tile.1), world.biomes.get(tile.0, tile.1).display_name().to_lowercase()),
            who: format!("Seven of {}: {}", fname(people), callings.join("; ")),
            trouble: format!("{}: {}", crate::colony::arc::capital_word(&arc.threat.name), arc.threat.why),
            land: format!("Gives {}; lacks {}{}", gift, lack, if map.furnished.is_empty() { String::new() } else { format!(" (made livable: {})", map.furnished.join(", ")) }),
            cell,
        };
        picked.push((offer, arc.threat.kind, people));
    }
    }
    picked.into_iter().map(|p| p.0).collect()
}

/// What grows on embarks across the world's biomes (`--embark-survey`): for one land tile of
/// each world biome, the embark's own biome and its share of trees, shrubs, grass and bare
/// ground.
pub fn embark_survey(world: &WorldData, history: Option<&WorldHistory>) {
    let mut seen: Vec<String> = Vec::new();
    for y in (2..world.height - 2).step_by(3) {
        for x in (0..world.width).step_by(3) {
            if *world.heightmap.get(x, y) <= 0.0 { continue; }
            let b = format!("{:?}", world.biomes.get(x, y));
            if seen.contains(&b) { continue; }
            seen.push(b.clone());
            let (map, _, _) = colony_site(world, history, (x, y), None);
            let n = map.width * map.height;
            let (mut t, mut sh, mut g, mut none) = (0, 0, 0, 0);
            for j in 0..map.height { for i in 0..map.width {
                match map.cell(i, j, map.surface_z[j * map.width + i].max(0) as usize).plant {
                    crate::local::Plant::Tree(_) => t += 1, crate::local::Plant::Shrub => sh += 1, crate::local::Plant::Grass => g += 1, _ => none += 1,
                }
            } }
            let pct = |k: usize| 100.0 * k as f32 / n as f32;
            println!("Embark at {},{}: world {} ({:.0} C, moisture {:.2}) -> local {:?}: trees {:.0}%, shrubs {:.0}%, grass {:.0}%, bare {:.0}%",
                x, y, b, world.temperature.get(x, y), world.moisture.get(x, y), map.biome, pct(t), pct(sh), pct(g), pct(none));
        }
    }
}

/// Whether embarks carry the world's rivers (`--river-survey`): for every land tile within six
/// tiles of `centre` (`PLANET_SURVEY_RADIUS`), what the world says (a river on the tile, its
/// width from the world's discharge), the widest channel the zoomed region draws in the tile, and the default embark
/// (`colony_site` with no cell): the widest channel it holds and its water columns.
pub fn river_survey(world: &WorldData, history: Option<&WorldHistory>, centre: (usize, usize)) {
    let s = cells_per_tile();
    let (mut river_tiles, mut carried, mut dry_tiles, mut dry_wet) = (0, 0, 0, 0);
    let r: i64 = std::env::var("PLANET_SURVEY_RADIUS").ok().and_then(|v| v.parse().ok()).unwrap_or(6);
    for dy in -r..=r {
        for dx in -r..=r {
            let (x, y) = ((centre.0 as i64 + dx).rem_euclid(world.width as i64) as usize, centre.1 as i64 + dy);
            if y < 1 || y >= world.height as i64 - 1 { continue; }
            let y = y as usize;
            if *world.heightmap.get(x, y) <= 0.0 || world.water_body_map.get(x, y).is_lake() { continue; }
            let river = world.water_body_map.get(x, y).is_river();
            let zs = load_region(world, history, (x, y), world.seed());
            let r = &zs.region;
            let (x0, y0) = (x as i64 * s - zs.origin.0, y as i64 * s - zs.origin.1);
            let mut widest = 0.0f32;
            for j in y0.max(0)..(y0 + s).min(r.height as i64) { for i in x0.max(0)..(x0 + s).min(r.width as i64) {
                widest = widest.max(crate::local::channel_width(r, (j * r.width as i64 + i) as usize));
            } }
            let world_w = crate::local::world_river_width(world, x, y);
            let (map, _, _) = colony_site(world, history, (x, y), None);
            let n = map.width;
            let wet = (0..n * n).filter(|&c| { let z = map.surface_z[c] as usize + 1; z < map.depth && map.cell(c % n, c / n, z).water > 0 }).count();
            if river { river_tiles += 1; if map.river_m > 0.0 { carried += 1; } } else { dry_tiles += 1; if map.river_m > 0.0 { dry_wet += 1; } }
            println!("Tile {x},{y}: world {} (world width {:.0} m); region widest channel {:.0} m; embark river {:.0} m, {} water columns",
                if river { "RIVER" } else { "dry" }, world_w, widest, map.river_m, wet);
        }
    }
    println!("River survey: {carried} of {river_tiles} world-river tiles' embarks carry a river; {dry_wet} of {dry_tiles} other land tiles' embarks do");
}

/// Roles under strain (`--sim-roles`): 30 days, then the camp's builder dies of a fever; 30 more.
/// Prints the minutes a load took before and after, who took up the hammer, and how a
/// woodcutter's felling quickened.
/// A camp's legend heeded by the next (`--sim-legend`): the dev camp lives 100 days; its legend
/// goes through JSON as the window keeps it; a second camp founded two tiles away heeds it.
pub fn legend_trial(world: &WorldData, history: Option<&WorldHistory>, tile: (usize, usize)) {
    let (map, seed, _) = colony_site(world, history, tile, None);
    let mut a = found_colony(map, history, tile, seed, 7);
    a.run_days(130);
    let legend = a.legend("trial");
    let json = serde_json::to_string(&vec![legend.clone()]).unwrap_or_default();
    let legends: Vec<crate::colony::legend::Legend> = serde_json::from_str(&json).unwrap_or_default();
    println!("Legend: {} ({}, day {}): {} deeds; slain: {:?}; relic: {:?}; regards: {:?}", legend.name, legend.fate, legend.day, legend.deeds.len(), legend.slain, legend.relic,
        legend.regards.iter().map(|r| format!("{} {:+}", r.1, r.2)).collect::<Vec<_>>());
    let near = ((tile.0 + 2).min(world.width - 1), tile.1);
    let (map, seed2, _) = colony_site(world, history, near, None);
    let mut b = found_colony(map, history, near, seed2, 7);
    let threats = |c: &crate::colony::Colony| c.arc.as_ref().map(|x| std::iter::once(x.threat.name.clone()).chain(x.later.iter().map(|t| t.name.clone())).chain(x.reserve.iter().map(|t| t.name.clone())).collect::<Vec<_>>()).unwrap_or_default();
    let before = threats(&b);
    b.heed_legends(&legends);
    let after = threats(&b);
    println!("Second camp at {},{}: threats before {:?}; after {:?}", near.0, near.1, before, after);
    println!("  {}", b.log.last().cloned().unwrap_or_default());
    println!("Second camp's regards: {:?}", b.regards.iter().map(|r| format!("{} {:+}", r.people, r.total())).collect::<Vec<_>>());
}

pub fn roles_trial(world: &WorldData, history: Option<&WorldHistory>, tile: (usize, usize)) {
    let (map, seed, _) = colony_site(world, history, tile, None);
    let mut colony = found_colony(map, history, tile, seed, 7);
    let first_fell = 1.3 - 0.6 * colony.settlers.iter().map(|s| s.skill[2]).fold(0.0f32, f32::max);
    colony.run_days(30);
    let loads = |c: &crate::colony::Colony| c.settlers.iter().map(|s| s.loads_laid).sum::<u32>();
    let (l0, t0) = (loads(&colony), colony.clock.tick);
    colony.run_days(10);
    let before = (colony.clock.tick - t0) as f32 / (loads(&colony) - l0).max(1) as f32;
    let Some(_) = colony.settlers.iter().position(|s| s.alive && s.role == Some(4)) else { println!("Roles: no builder by day 40"); return };
    // The camp's quickest builder dies (named the builder first, if the role had not passed to
    // them yet): the trial is of losing the best hand.
    let pace = |s: &crate::colony::Settler| (1.3 - 0.6 * s.skill[4]) * s.persona.work_time(4);
    let b = (0..colony.settlers.len()).filter(|&i| colony.settlers[i].alive && colony.settlers[i].skill[4] >= 0.45)
        .min_by(|&x, &y| pace(&colony.settlers[x]).total_cmp(&pace(&colony.settlers[y])).then(x.cmp(&y))).unwrap();
    for s in colony.settlers.iter_mut() { if s.role == Some(4) { s.role = None; } }
    colony.settlers[b].role = Some(4);
    let name = colony.settlers[b].name.clone();
    // Minutes a load takes their hands: skill and body (`Persona::work_time`).
    let old_hand = 60.0 * (1.3 - 0.6 * colony.settlers[b].skill[4]) * colony.settlers[b].persona.work_time(4);
    // The next best hand at that moment (before ten days' practice change it).
    let next_hand = (0..colony.settlers.len()).filter(|&i| i != b && colony.settlers[i].alive && colony.settlers[i].skill[4] >= 0.45)
        .map(|i| 60.0 * pace(&colony.settlers[i])).fold(f32::MAX, f32::min);
    colony.bury(b, "of a fever");
    let (l1, t1) = (loads(&colony), colony.clock.tick);
    colony.run_days(10);
    let after = (colony.clock.tick - t1) as f32 / (loads(&colony) - l1).max(1) as f32;
    let took = colony.log.iter().find(|l| l.contains("takes up the hammer")).cloned().unwrap_or_else(|| "no one took up the hammer".into());
    let wc = colony.settlers.iter().filter(|s| s.alive).map(|s| s.skill[2]).fold(0.0f32, f32::max);
    let new_hand = if next_hand < f32::MAX { next_hand } else { 78.0 };
    let _ = (before, after);
    println!("Roles trial: the builder {} died on day 40; a load took {:.1} min of their work, {:.1} of the new builder's", name, old_hand, new_hand);
    println!("  {}", took);
    println!("  Felling: the best woodcutter now takes {:.2}x the base time (the greenest start took {:.2}x)", 1.3 - 0.6 * wc, first_fell.max(1.3 - 0.6 * 0.0));
}

/// The plan follows the patron (`--sim-plan`): one colony blesses a meadow west of the camp on
/// day 3, another forbids everything east of the fire; 40 days each. Prints where the works stood.
pub fn plan_trial(world: &WorldData, history: Option<&WorldHistory>, tile: (usize, usize)) {
    for variant in ["bless", "forbid"] {
        let (map, seed, _) = colony_site(world, history, tile, None);
        let mut c = found_colony(map, history, tile, seed, 7);
        c.run_days(2);
        let camp = c.camp;
        let mark = if variant == "bless" { (camp.0.saturating_sub(12), camp.1 + 6) } else { (camp.0 + 16, camp.1) };
        c.patron.favour = 3;
        let _ = c.mark_place(mark, if variant == "bless" { 6 } else { 13 }, variant == "forbid");
        let before = c.projects.len();
        c.run_days(40);
        let inside = |p: (u16, u16)| (p.0 as i32 - mark.0 as i32).abs().max((p.1 as i32 - mark.1 as i32).abs()) <= if variant == "bless" { 7 } else { 13 };
        let new: Vec<_> = c.projects.iter().skip(before).filter(|p| crate::colony::Colony::footprint(p.kind).is_some()).collect();
        let n_in = new.iter().filter(|p| inside(p.at)).count();
        let firsts: Vec<String> = new.iter().take(3).map(|p| format!("{} at {},{}{}", p.kind.word(), p.at.0, p.at.1, if inside(p.at) { " (on the mark)" } else { "" })).collect();
        println!("Plan {}: {} of {} new buildings on the marked ground; first: {}", variant, n_in, new.len(), firsts.join("; "));
    }
}

/// One site, peoples' ways (`--sim-ways`): the dev colony lived 60 days as dwarves, elves, orcs
/// and humans; prints each order of works and how far the camp frames differ.
pub fn ways_trial(world: &WorldData, history: Option<&WorldHistory>, atlas: &Atlas, tile: (usize, usize)) {
    let mut frames: Vec<(String, Vec<u32>)> = Vec::new();
    for people in ["dwarf", "elf", "orc", "human"] {
        let (map, seed, _) = colony_site(world, history, tile, None);
        let mut c = found_colony(map, history, tile, seed, 7);
        if let Some(way) = crate::colony::projects::build_way(people) { c.adopt_way(way); }
        c.run_days(60);
        let order: Vec<String> = c.projects.iter().filter(|p| p.kind != crate::colony::projects::ProjectKind::Mending).take(6).map(|p| p.kind.word().trim_start_matches("a ").to_string()).collect();
        let felled_inside = (0..c.map.height).flat_map(|y| (0..c.map.width).map(move |x| (x, y))).filter(|&(x, y)| {
            ((x as i32 - c.camp.0 as i32).pow(2) + (y as i32 - c.camp.1 as i32).pow(2)) <= 13 * 13 && c.map.features[y * c.map.width + x] == crate::local::wildlife::Feature::Stump
        }).count();
        let plural = match people { "dwarf" => "dwarves", "elf" => "elves", other => other };
        println!("Way of the {}: hut in {}; works: {}; stumps inside the wall: {}", if plural == people { format!("{}s", people) } else { plural.to_string() }, if c.hut_material == crate::colony::ItemKind::Stone { "stone" } else { "timber" }, order.join(", "), felled_inside);
        let (w, h) = (640usize, 640usize);
        let cam = LocalCamera { cx: c.camp.0 as f32 + 0.5, cy: c.camp.1 as f32 + 0.5, tile_px: 12.0, z: 0, surface_view: true };
        let mut buf = vec![0u32; w * h];
        render_local(&c.map, atlas, &cam, &mut buf, w, h);
        super::local_ink::draw_colony(&c, &cam, &mut buf, w, h, history);
        frames.push((people.to_string(), buf));
    }
    for a in 0..frames.len() { for b in a + 1..frames.len() {
        let d = frames[a].1.iter().zip(&frames[b].1).filter(|(x, y)| x != y).count() as f32 / frames[a].1.len() as f32;
        println!("  {} vs {}: camp frames differ in {:.0}% of pixels", frames[a].0, frames[b].0, d * 100.0);
    } }
}

/// Found the colony on an embark at world `tile`: with a history, the settlers come out of it
/// (`history::settlers::roster`: survivors, veterans, kin); without one they are nameless
/// wanderers with stock names.
fn found_colony(map: crate::local::LocalMap, history: Option<&WorldHistory>, tile: (usize, usize), seed: u64, n: usize) -> crate::colony::Colony {
    match history {
        Some(h) => {
            // Those who may come later as migrants are the same roster drawn further.
            let mut roster = crate::history::settlers::roster(h, tile, n + 6, seed);
            let later: Vec<(String, crate::history::settlers::Past)> = roster.split_off(n.min(roster.len()));
            let names: Vec<String> = roster.iter().map(|r| r.0.clone()).collect();
            // The founders choose the camp's place by who they are (`Leaning`).
            let founders: Vec<crate::persona::Persona> = roster.iter().filter_map(|r| r.1.persona.clone()).collect();
            let mut colony = crate::colony::Colony::found_by(map, &names, seed, &founders);
            for (st, (_, past)) in colony.settlers.iter_mut().zip(roster) {
                st.skill = crate::colony::skills_from_past(Some(&past), &st.name);
                if let Some(p) = &past.persona { st.persona = p.clone(); }
                st.taste = crate::colony::rhythm::taste_of(&st.persona, &st.name);
                st.past = Some(past);
            }
            // The first arc: the world will reach this camp.
            colony.arc = Some(crate::colony::arc::plan(h, tile, seed));
            // The camp's own people do not come as its first raiders: those who do are deserters,
            // outlaws of that people whom everyone will fight.
            {
                let mut count: Vec<(crate::history::FactionId, usize)> = Vec::new();
                for st in &colony.settlers { if let Some(f) = st.past.as_ref().and_then(|p| p.people) { match count.iter_mut().find(|c| c.0 == f) { Some(c) => c.1 += 1, None => count.push((f, 1)) } } }
                let most = count.iter().max_by_key(|c| (c.1, std::cmp::Reverse(c.0))).map(|c| c.0);
                if let Some(a) = colony.arc.as_mut() {
                    let own = |t: &crate::colony::arc::Threat| matches!(t.kind, crate::colony::arc::ThreatKind::Warband | crate::colony::arc::ThreatKind::Envoy) && t.faction.is_some() && t.faction == most;
                    let desert = |t: &mut crate::colony::arc::Threat| {
                        let people = t.name.trim_start_matches("a war band of ").trim_start_matches("an envoy of ").split(", led by ").next().unwrap_or("").to_string();
                        t.kind = crate::colony::arc::ThreatKind::Outlaws;
                        t.name = format!("deserters of {}", people);
                        t.why = format!("they left the war bands of {} and live by raiding now", people);
                        t.faction = None;
                    };
                    if own(&a.threat) { desert(&mut a.threat); }
                    for t in a.later.iter_mut().chain(a.reserve.iter_mut()) { if own(t) { desert(t); } }
                }
            }
            // Their people's way of building (data/defaults/building_ways.json).
            let race = colony.settlers.first().and_then(|s| s.past.as_ref()).and_then(|p| p.people).and_then(|f| h.factions.get(&f))
                .and_then(|f| h.races.get(&f.race_id)).map(|r| format!("{:?}", r.base_type).to_lowercase());
            if let Some(way) = race.as_deref().and_then(crate::colony::projects::build_way) { colony.adopt_way(way); }
            // The graves already on this ground become the colony's marks (hover and click).
            colony.adopt_graves();
            // The town of their people that will send caravans (`trade.rs`).
            colony.trade = crate::colony::trade::partner(h, tile, colony.settlers.first().and_then(|s| s.past.as_ref()).and_then(|p| p.people));
            colony.migrants = later;
            // What the migrants' people know, as they tell it (`history::knowledge`).
            if let Some(f) = colony.migrants.first().and_then(|m| m.1.people) {
                colony.migrant_news = crate::history::knowledge::Knowledge::new(h).news_of_people(f, 30, 6);
            }
            colony.world_width = h.tile_history.width.max(1);
            // Peoples among the troubles who steal children: goblins and orcs (`snatch.rs`).
            if let Some(a) = colony.arc.as_ref() {
                let mut v: Vec<crate::history::FactionId> = std::iter::once(&a.threat).chain(a.later.iter()).chain(a.reserve.iter()).filter_map(|t| t.faction)
                    .filter(|f| h.factions.get(f).and_then(|x| h.races.get(&x.race_id)).map_or(false, |r| matches!(format!("{:?}", r.base_type).to_lowercase().as_str(), "goblin" | "orc")))
                    .collect();
                v.sort(); v.dedup();
                colony.snatchers = v;
            }
            // The war their people fight, which may call for the camp's spears (`warcall.rs`).
            colony.war_call = crate::colony::warcall::plan(h, colony.settlers.first().and_then(|s| s.past.as_ref()).and_then(|p| p.people));
            // The lord their people will send when the camp has grown (`nobles.rs`).
            colony.lord = crate::colony::nobles::plan_lord(h, colony.settlers.first().and_then(|s| s.past.as_ref()).and_then(|p| p.people));
            // A shapeshifter laired within eight tiles is a werebeast under the moon (`curse.rs`).
            {
                use crate::history::creatures::anatomy::MagicAbility;
                let w = h.tile_history.width.max(1);
                colony.were = h.legendary_creatures.values().filter(|c| c.is_alive())
                    .filter(|c| c.unique_abilities.contains(&MagicAbility::Shapeshifting) || h.creature_species.get(&c.species_id).map_or(false, |sp| sp.magical_abilities.contains(&MagicAbility::Shapeshifting)))
                    .filter_map(|c| c.lair_location.map(|l| { let dx = l.0.abs_diff(tile.0); (dx.min(w - dx) + l.1.abs_diff(tile.1), c) }))
                    .filter(|(d, _)| *d <= 8).min_by_key(|(d, c)| (*d, c.id)).map(|(_, c)| c.full_name());
                // PLANET_FORCE_WERE=<name>: a werebeast near any camp (for trying the curse).
                if let Ok(name) = std::env::var("PLANET_FORCE_WERE") { colony.were = Some(name); }
            }
            // Figures of the world who may visit: hunters of its beasts, bards (`visitors.rs`).
            if let Some(a) = colony.arc.as_ref() {
                let beasts: Vec<String> = std::iter::once(&a.threat).chain(a.later.iter()).chain(a.reserve.iter())
                    .filter(|t| t.kind == crate::colony::arc::ThreatKind::Beast).map(|t| t.name.clone()).collect();
                let home = colony.settlers.first().and_then(|s| s.past.as_ref()).and_then(|p| p.people);
                let relic = crate::colony::relic::lost_near(h, tile);
                let rid = relic.as_ref().map(|r| (r.artifact, r.holder));
                // A lost artifact of the history near here (`relic.rs`).
                if let Some(r) = relic { colony.place_relic(r); }
                colony.visitors = crate::colony::visitors::plan(h, tile, &beasts, rid, home, seed);
                // PLANET_FORCE_SEEKER=<name>: a seeker for the relic (for trying the flow).
                if let (Ok(name), Some(r)) = (std::env::var("PLANET_FORCE_SEEKER"), colony.relic.as_ref()) {
                    if !colony.visitors.iter().any(|v| matches!(v.kind, crate::colony::visitors::VisitKind::Seeker { .. })) {
                        let v = crate::colony::visitors::forced_seeker(&name, &r.name, r.owners, seed);
                        colony.visitors.push(v);
                    }
                }
            }
            // How deep the Shadow lies here (`dead.rs`).
            if let Some(sh) = h.shadow.as_ref().filter(|s| !s.is_broken()) {
                colony.darkness = sh.at(tile.0, tile.1);
                colony.shadow_name = Some(sh.name.clone());
            }
            colony
        }
        None => crate::colony::Colony::found(map, &settler_names(seed, n), seed),
    }
}

/// The seven names of the dev colony's settlers, drawn from the human naming style.
pub fn settler_names(seed: u64, n: usize) -> Vec<String> {
    use rand::SeedableRng;
    let style = crate::history::naming::styles::NamingStyle::from_archetype(crate::history::NamingStyleId(0), crate::history::naming::styles::NamingArchetype::Compound);
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed ^ 0x5E77_1E25);
    let mut names: Vec<String> = Vec::new();
    while names.len() < n {
        let name = crate::history::naming::generator::NameGenerator::personal_name(&style, &mut rng);
        if !names.contains(&name) { names.push(name); }
    }
    names
}

/// Found a colony of `n` settlers on the embark at `tile` and time it: two game days, reported
/// as ticks per second and the fastest game speed it sustains (1x = an hour a second).
pub fn colony_bench(world: &WorldData, history: Option<&WorldHistory>, tile: (usize, usize), n: usize) {
    let zs = load_region(world, history, tile, world.seed());
    let s = cells_per_tile() as f64;
    let (ex, ey) = (tile.0 as f64 * s + s / 2.0 - zs.origin.0 as f64, tile.1 as f64 * s + s / 2.0 - zs.origin.1 as f64);
    let map = crate::local::generate_local(world, &zs.region, zs.lore.as_ref(), ex, ey);
    let mut colony = found_colony(map, history, tile, world.seed(), n);
    let t0 = std::time::Instant::now();
    colony.run_days(2);
    let secs = t0.elapsed().as_secs_f64();
    let ticks = 2.0 * crate::colony::TICKS_PER_DAY as f64;
    println!("Colony bench: {} settlers, 2 game days in {:.2}s = {:.0} ticks/s; 1x = 60 ticks/s, so up to {:.0}x; {} alive",
        n, secs, ticks / secs, ticks / secs / 60.0, colony.alive());
}

/// Headless run of the first colony: found it on the embark at `tile`, let it live 30 days
/// unattended, and write frames on days 1, 10 and 30 (`<prefix>_dayN.png`, the whole area at
/// 6 px and a close-up of the camp at 16 px) and its log (`<prefix>_log.txt`).
pub fn save_colony_snapshots(world: &WorldData, history: Option<&WorldHistory>, atlas: &Atlas, tile: (usize, usize), prefix: &str, script: &[String]) -> Result<Vec<String>, Box<dyn Error>> {
    use super::local_ink::draw_colony;
    let zs = load_region(world, history, tile, world.seed());
    let s = cells_per_tile() as f64;
    let (ex, ey) = {
        let (tx, ty) = (tile.0 as f64 * s + s / 2.0 - zs.origin.0 as f64, tile.1 as f64 * s + s / 2.0 - zs.origin.1 as f64);
        (tx, ty)
    };
    let _ = (zs, ex, ey);
    let cell = START_CELL.get().copied();
    let (map, seed, _) = colony_site(world, history, tile, cell);
    if !crate::colony::habitable(&map) { return Err(format!("the site at {},{} is under water; no camp can be made here", tile.0, tile.1).into()); }
    if let Err(why) = crate::colony::survey(&map).verdict() { return Err(format!("no camp at {},{}: {}", tile.0, tile.1, why).into()); }
    let mut colony = found_colony(map, history, tile, seed, 7);
    colony.cell = cell;
    let old_graves: Vec<&crate::colony::ColonyMark> = colony.marks.iter().filter(|m| m.day == 0).collect();
    if !old_graves.is_empty() {
        let named: Vec<&&crate::colony::ColonyMark> = old_graves.iter().filter(|m| !m.title.starts_with("An old")).collect();
        println!("Graves on this ground: {}, {} named (e.g. {})", old_graves.len(), named.len(), named.first().unwrap_or(&&old_graves[0]).text);
    }
    // Who they are: each settler's past, and the events they share.
    let mut lines = Vec::new();
    let mut seen: std::collections::HashMap<crate::history::EventId, usize> = std::collections::HashMap::new();
    let mut fewest = usize::MAX;
    for st in &colony.settlers {
        let Some(p) = &st.past else { fewest = 0; continue };
        let evs: Vec<crate::history::EventId> = p.lines.iter().filter_map(|l| l.1).collect();
        fewest = fewest.min(evs.len());
        for e in &evs { *seen.entry(*e).or_default() += 1; }
        lines.push(format!("{}, {}, {}{}", st.name, p.age, p.calling, p.feeling.as_ref().map(|f| format!("; {}", f.0)).unwrap_or_default()));
        for (l, _) in &p.lines { lines.push(format!("    {}", l)); }
        for para in st.persona.describe(&st.name, p.age) { lines.push(format!("  * {}", para)); }
    }
    let shared = seen.values().filter(|n| **n >= 2).count();
    std::fs::write(format!("{prefix}_settlers.txt"), lines.join("\n") + "\n")?;
    println!("Settlers: {} with pasts, each citing at least {} events; {} events shared by two or more",
        colony.settlers.iter().filter(|s| s.past.is_some()).count(), if fewest == usize::MAX { 0 } else { fewest }, shared);
    let mut written = Vec::new();
    let mut day_done = 0u64;
    for day in [1u64, 10, 30] {
        let t0 = std::time::Instant::now();
        colony.run_days_scripted(day - day_done, script);
        day_done = day;
        println!("Colony day {}: {} alive, {} food, {} logs stored, hut {}, simulated in {:.2}s",
            colony.clock.day() - 1, colony.alive(), colony.food_stored(), colony.logs_stored(),
            colony.hut.as_ref().map(|h| if h.done { "built".to_string() } else { format!("{}/{} logs", h.logs_used, crate::colony::HUT_LOGS) }).unwrap_or_else(|| "no site".into()),
            t0.elapsed().as_secs_f32());
        for line in colony.roll_call() { println!("  {line}"); }
        let n = colony.map.width;
        for (name, cam, (w, h)) in [
            (format!("day{day}"), LocalCamera { cx: n as f32 / 2.0, cy: n as f32 / 2.0, tile_px: 6.0, z: 0, surface_view: true }, (n * 6, n * 6)),
            (format!("day{day}_camp"), LocalCamera { cx: colony.camp.0 as f32 + 4.0, cy: colony.camp.1 as f32 + 2.0, tile_px: 16.0, z: 0, surface_view: true }, (1024, 640)),
        ] {
            let mut buf = vec![0u32; w * h];
            render_local(&colony.map, atlas, &cam, &mut buf, w, h);
            draw_colony(&colony, &cam, &mut buf, w, h, history);
            let path = format!("{prefix}_{name}.png");
            save_rgb_png(&path, w, h, |x, y| { let q = buf[y * w + x]; [(q >> 16) as u8, (q >> 8) as u8, q as u8] });
            written.push(path);
        }
    }
    // The camp as the window shows it: the HUD, with the mouse over the first settler.
    {
        let (w, h) = (1280usize, 800usize);
        let mut buf = vec![0u32; w * h];
        let st = &colony.settlers.iter().find(|s| s.alive).unwrap_or(&colony.settlers[0]);
        let cam = LocalCamera { cx: st.pos.0 as f32 + 6.0, cy: st.pos.1 as f32 + 3.0, tile_px: 16.0, z: 0, surface_view: true };
        render_local(&colony.map, atlas, &cam, &mut buf, w, h);
        draw_colony(&colony, &cam, &mut buf, w, h, history);
        let mouse = ((st.pos.0 as f32 + 0.5 - cam.cx) * cam.tile_px + w as f32 / 2.0, (st.pos.1 as f32 + 0.5 - cam.cy) * cam.tile_px + h as f32 / 2.0);
        let last = colony.log.iter().rev().find(|l| l.contains("(your doing)")).cloned().unwrap_or_default();
        let status = last.split_once("  ").map(|x| x.1.to_string()).unwrap_or_default();
        super::colony_hud::draw(&colony, &cam, &super::colony_hud::HudState { speed: 1, status: &status, mouse, right: 0, selected: None, hide_chip: false, bar: true }, &mut buf, w, h);
        let path = format!("{prefix}_hud.png");
        save_rgb_png(&path, w, h, |x, y| { let p = buf[y * w + x]; [(p >> 16) as u8, (p >> 8) as u8, p as u8] });
        written.push(path);
    }
    // The raid's moment as the window stops for it: the camera on its place, the card up.
    println!("Moments: {}, {} major ({})", colony.moments.len(), colony.moments.iter().filter(|m| m.major()).count(), colony.moments.iter().map(|m| format!("day {} {}", m.tick / crate::colony::TICKS_PER_DAY + 1, m.title)).collect::<Vec<_>>().join(", "));
    if let Some(m) = colony.moments.iter().find(|m| m.title == "The raid").or(colony.moments.last()) {
        let (w, h) = (1280usize, 800usize);
        let mut buf = vec![0u32; w * h];
        let cam = LocalCamera { cx: m.at.0 as f32 + 0.5, cy: m.at.1 as f32 + 0.5, tile_px: 16.0, z: 0, surface_view: true };
        render_local(&colony.map, atlas, &cam, &mut buf, w, h);
        draw_colony(&colony, &cam, &mut buf, w, h, history);
        super::colony_hud::draw(&colony, &cam, &super::colony_hud::HudState { speed: 0, status: "", mouse: (-100.0, -100.0), right: 0, selected: None, hide_chip: false, bar: true }, &mut buf, w, h);
        let _ = super::colony_hud::draw_moment(m, Some(&colony), &mut buf, w, h);
        let path = format!("{prefix}_moment.png");
        save_rgb_png(&path, w, h, |x, y| { let p = buf[y * w + x]; [(p >> 16) as u8, (p >> 8) as u8, p as u8] });
        written.push(path);
    }
    // The first settler's inspector page, as a click shows it.
    {
        let (w, h) = (1280usize, 800usize);
        let mut buf = vec![0u32; w * h];
        let st = &colony.settlers[0];
        let cam = LocalCamera { cx: st.pos.0 as f32 + 0.5, cy: st.pos.1 as f32 + 0.5, tile_px: 16.0, z: 0, surface_view: true };
        render_local(&colony.map, atlas, &cam, &mut buf, w, h);
        draw_colony(&colony, &cam, &mut buf, w, h, history);
        let page = super::inspector::settler_page_with(history, st, wounded_in(&colony, &st.name), &colony.about(0));
        super::inspector::draw(&page, &mut buf, w, h, 1);
        let path = format!("{prefix}_settler.png");
        save_rgb_png(&path, w, h, |x, y| { let p = buf[y * w + x]; [(p >> 16) as u8, (p >> 8) as u8, p as u8] });
        written.push(path);
    }
    // The ledger's leaves and a sheet, as the window will show them (`colony_ui`).
    written.extend(super::colony_ui::save_ui_snapshots(&colony, history, atlas, prefix));
    let path = format!("{prefix}_log.txt");
    std::fs::write(&path, colony.log.join("\n") + "\n")?;
    written.push(path);
    let path = format!("{prefix}_decisions.txt");
    std::fs::write(&path, colony.decisions.join("\n") + "\n")?;
    written.push(path);
    // One number for the whole story: the world's chronicle and the colony's log. The same world
    // code and the same interventions give the same hash on any machine.
    let mut hsh: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |t: &str| for b in t.bytes() { hsh ^= b as u64; hsh = hsh.wrapping_mul(0x100_0000_01b3); };
    if let Some(hist) = history { for e in &hist.chronicle.events { feed(&e.title); feed(&e.description); } }
    for l in &colony.log { feed(l); }
    println!("Colony hash: {:016x} ({} interventions)", hsh, colony.interventions.len());
    println!("Colony code: {}; milestones: {}", colony_code(&colony), if colony.milestones_hit.is_empty() { "none".to_string() } else { colony.milestones_hit.join(", ") });
    let stuck: u32 = colony.settlers.iter().map(|s| s.stuck).sum();
    for s in colony.settlers.iter().filter(|s| s.stuck > 100) {
        println!("  Stuck: {} at {},{} ({} times), last {} - {}; passable here: {}; camp at {},{}", s.name, s.pos.0, s.pos.1, s.stuck, s.job.verb(), s.why, crate::colony::nav::passable(&colony.map, s.pos), colony.camp.0, colony.camp.1);
        for dy in -3i32..=3 {
            let row: String = (-3i32..=3).map(|dx| { let p = ((s.pos.0 as i32 + dx) as u16, (s.pos.1 as i32 + dy) as u16); if p == colony.camp { 'C' } else if p == s.pos { '@' } else if crate::colony::nav::passable(&colony.map, p) { '.' } else { '#' } }).collect();
            println!("    {row}");
        }
    }
    println!("Colony after 30 days: {} of {} alive, {} times a settler found no way to a target, {} log lines", colony.alive(), colony.company(), stuck, colony.log.len());
    {
        let path = format!("{prefix}_saga.png");
        saga_plate(world, history, atlas, &colony, &path)?;
        written.push(path);
    }
    {
        // The faces at 48 px, and how far apart the two most alike are.
        let n = colony.settlers.len();
        let (fw, fh) = (n * 120 + 20, 190usize);
        let mut buf = vec![0x00EA_DEC4u32; fw * fh];
        let mut tiles: Vec<Vec<u32>> = Vec::new();
        for (k, st) in colony.settlers.iter().enumerate() {
            let p = super::portraits::of_settler(st, history, wounded_in(&colony, &st.name));
            super::portraits::draw(&mut buf, fw, fh, (20 + k * 120) as i64, 10, 96, &p);
            let mut t = vec![0x00EA_DEC4u32; 48 * 48];
            super::portraits::draw(&mut t, 48, 48, 0, 0, 48, &p);
            super::portraits::draw(&mut buf, fw, fh, (44 + k * 120) as i64, 112, 48, &p);
            super::text::draw_ink(&mut buf, fw, fh, (20 + k * 120) as i64, 172, &super::ui::truncate(&super::ui::ascii(&st.name), 15), 0x0038_2A20, 1, false);
            tiles.push(t);
        }
        let mut least = f32::MAX;
        for a in 0..n { for b in a + 1..n {
            let d = tiles[a].iter().zip(&tiles[b]).filter(|(x, y)| x != y).count() as f32 / (48.0 * 48.0);
            least = least.min(d);
        } }
        let path = format!("{prefix}_faces.png");
        save_rgb_png(&path, fw, fh, |x, y| { let p = buf[y * fw + x]; [(p >> 16) as u8, (p >> 8) as u8, p as u8] });
        written.push(path);
        println!("Faces: {} settlers; the two most alike differ in {:.0}% of their 48 px pixels", n, least * 100.0);
    }
    {
        // The camp's annals: every moment, its works and its people (`colony/annals.rs`).
        let path = format!("{prefix}_annals.html");
        std::fs::write(&path, colony.annals(history))?;
        written.push(path);
    }
    if colony.arc.is_some() {
        let path = format!("{prefix}_tale.html");
        std::fs::write(&path, colony.tale(history))?;
        written.push(path);
        for e in &colony.arc.as_ref().unwrap().events { println!("Arc day {}: {} {} ({})", e.day, e.title, e.text, e.because); }
    }
    // The raid on legs: how long the attackers were on the map before the clash, and from where.
    for (from, at) in &colony.raid_watch {
        println!("Raid on legs: the attackers were on the map {} ticks before the clash, out of {}", at - from, colony.raid_side);
    }
    let bites = colony.log.iter().filter(|l| l.contains("bitten by wolves")).count();
    if bites > 0 { println!("Wolves: {} bites", bites); }
    // Worn ground: cells walked often.
    {
        let worn = |k: u16| colony.steps.iter().filter(|&&n| n >= k).count();
        println!("Paths: {} cells worn to a track (15+ steps), {} to a lane (120+); most walked {}", worn(15), worn(120), colony.steps.iter().max().copied().unwrap_or(0));
    }
    // Roles: who the camp knows for what, and the builder's share of the loads.
    {
        let roles: Vec<String> = colony.settlers.iter().filter_map(|s| s.role.map(|k| format!("{} the {}", s.name, crate::colony::ROLES[k]))).collect();
        let total: u32 = colony.settlers.iter().map(|s| s.loads_laid).sum();
        let builder = colony.settlers.iter().find(|s| s.role == Some(4)).map(|s| s.loads_laid).unwrap_or(0);
        let _ = (builder, total);
        println!("Roles: {} ({}); since one was named the builder laid {} of {} loads", roles.len(), roles.join(", "), colony.builder_share.0, colony.builder_share.1);
    }
    // Pasts that act: watches kept by veterans vs the others, and log lines that give a past as the reason.
    {
        let vets: Vec<String> = colony.settlers.iter().filter(|s| s.past.as_ref().map_or(false, |p| p.calling.starts_with("a veteran"))).map(|s| s.name.clone()).collect();
        let watches: Vec<&String> = colony.log.iter().filter(|l| l.contains("keeps watch tonight")).collect();
        let by_vets = watches.iter().filter(|l| vets.iter().any(|v| l.contains(&format!("  {} keeps", v)))).count();
        let n_others = colony.settlers.len().saturating_sub(vets.len()).max(1);
        let pasts = colony.log.iter().filter(|l| ["goes quiet", ", as at ", "will not sleep", "from before the fall", "quarrel by the fire", "takes it hard"].iter().any(|k| l.contains(k))).count();
        println!("Pasts: {} watches by {} veterans, {} by {} others; {} log lines give a past as the reason", by_vets, vets.len(), watches.len() - by_vets, n_others, pasts);
    }
    // On the map a settler is told apart by skin, hair and dress: how many look alike.
    {
        let all = super::local_ink::settler_looks(&colony, history);
        let looks: Vec<String> = all.iter().zip(&colony.settlers).filter(|(_, s)| s.alive).map(|(l, _)| format!("{:?}", l)).collect();
        let distinct = looks.iter().collect::<std::collections::HashSet<_>>().len();
        println!("Figures: {} of {} living settlers look different on the map", distinct, looks.len());
    }
    // The raid on legs, as the window shows it: a second founding lived to the attackers' approach.
    {
        let (map2, seed2, _) = colony_site(world, history, tile, START_CELL.get().copied());
        let mut c2 = found_colony(map2, history, tile, seed2, 7);
        c2.cell = colony.cell;
        let limit = 40 * crate::colony::TICKS_PER_DAY;
        while !c2.attackers_out() && c2.clock.tick < limit { c2.tick(); }
        for _ in 0..240 { c2.tick(); }
        if let Some(a) = c2.creatures.iter().find(|c| matches!(c.kind, crate::colony::creatures::CreatureKind::Beast | crate::colony::creatures::CreatureKind::Raider)).map(|c| c.pos) {
            let (w, h) = (1280usize, 800usize);
            let mid = ((a.0 as f32 + c2.camp.0 as f32) / 2.0, (a.1 as f32 + c2.camp.1 as f32) / 2.0);
            let cam = LocalCamera { cx: mid.0, cy: mid.1, tile_px: 8.0, z: 0, surface_view: true };
            let mut buf = vec![0u32; w * h];
            render_local(&c2.map, atlas, &cam, &mut buf, w, h);
            draw_colony(&c2, &cam, &mut buf, w, h, history);
            let path = format!("{prefix}_raidnight.png");
            save_rgb_png(&path, w, h, |x, y| { let q = buf[y * w + x]; [(q >> 16) as u8, (q >> 8) as u8, q as u8] });
            written.push(path);
            // Close up as they reach the camp (the attackers as figures or as their beast).
            let near = |c: &crate::colony::Colony| c.creatures.iter().filter(|k| matches!(k.kind, crate::colony::creatures::CreatureKind::Beast | crate::colony::creatures::CreatureKind::Raider))
                .map(|k| (k.pos.0 as i32 - c.camp.0 as i32).abs().max((k.pos.1 as i32 - c.camp.1 as i32).abs())).min();
            let mut guard = 0;
            while near(&c2).map_or(false, |d| d > 9) && guard < 4000 { c2.tick(); guard += 1; }
            if let Some(a) = c2.creatures.iter().filter(|c| matches!(c.kind, crate::colony::creatures::CreatureKind::Beast | crate::colony::creatures::CreatureKind::Raider)).min_by_key(|k| (k.pos.0 as i32 - c2.camp.0 as i32).abs().max((k.pos.1 as i32 - c2.camp.1 as i32).abs())).map(|c| c.pos) {
                let (w, h) = (1024usize, 640usize);
                let cam = LocalCamera { cx: (a.0 as f32 + c2.camp.0 as f32) / 2.0, cy: (a.1 as f32 + c2.camp.1 as f32) / 2.0, tile_px: 20.0, z: 0, surface_view: true };
                let mut buf = vec![0u32; w * h];
                render_local(&c2.map, atlas, &cam, &mut buf, w, h);
                draw_colony(&c2, &cam, &mut buf, w, h, history);
                let path = format!("{prefix}_raidclose.png");
                save_rgb_png(&path, w, h, |x, y| { let q = buf[y * w + x]; [(q >> 16) as u8, (q >> 8) as u8, q as u8] });
                written.push(path);
            }
            // The clash: an attacker at a settler, blows drawn.
            let at_one = |c: &crate::colony::Colony| c.creatures.iter().filter(|k| matches!(k.kind, crate::colony::creatures::CreatureKind::Beast | crate::colony::creatures::CreatureKind::Raider) && !k.leaving)
                .find_map(|k| c.settlers.iter().filter(|s| s.alive).find(|s| (s.pos.0 as i32 - k.pos.0 as i32).abs() <= 1 && (s.pos.1 as i32 - k.pos.1 as i32).abs() <= 1).map(|_| k.pos));
            let mut guard = 0;
            while at_one(&c2).is_none() && c2.attackers_out() && guard < 3000 { c2.tick(); guard += 1; }
            // (The clash is fought in one moment where they meet the camp: show the melee there.)
            for _ in 0..10 { c2.tick(); }
            if let Some(a) = at_one(&c2).or(c2.clash_at) {
                let (w, h) = (1024usize, 640usize);
                let cam = LocalCamera { cx: a.0 as f32 + 0.5, cy: a.1 as f32 + 0.5, tile_px: 24.0, z: 0, surface_view: true };
                let mut buf = vec![0u32; w * h];
                render_local(&c2.map, atlas, &cam, &mut buf, w, h);
                draw_colony(&c2, &cam, &mut buf, w, h, history);
                let path = format!("{prefix}_clash.png");
                save_rgb_png(&path, w, h, |x, y| { let q = buf[y * w + x]; [(q >> 16) as u8, (q >> 8) as u8, q as u8] });
                written.push(path);
            }
        }
    }
    // The wild: the game nearest the camp, close up.
    if let Some(g) = colony.creatures.iter().filter(|c| c.z.is_none() && !matches!(c.kind, crate::colony::creatures::CreatureKind::Beast | crate::colony::creatures::CreatureKind::Raider))
        .min_by_key(|k| (k.pos.0 as i32 - colony.camp.0 as i32).abs() + (k.pos.1 as i32 - colony.camp.1 as i32).abs()).map(|c| c.pos) {
        let (w, h) = (1024usize, 640usize);
        let cam = LocalCamera { cx: g.0 as f32, cy: g.1 as f32, tile_px: 24.0, z: 0, surface_view: true };
        let mut buf = vec![0u32; w * h];
        render_local(&colony.map, atlas, &cam, &mut buf, w, h);
        draw_colony(&colony, &cam, &mut buf, w, h, history);
        let path = format!("{prefix}_wild.png");
        save_rgb_png(&path, w, h, |x, y| { let q = buf[y * w + x]; [(q >> 16) as u8, (q >> 8) as u8, q as u8] });
        written.push(path);
    }
    // The camp at noon and at midnight of the next day (after everything above is reckoned).
    let mut lum = Vec::new();
    for (name, hour) in [("noon", 12u64), ("midnight", 24)] {
        let target = (colony.clock.tick / crate::colony::TICKS_PER_DAY) * crate::colony::TICKS_PER_DAY + hour * 60;
        while colony.clock.tick < target { colony.tick(); }
        let (w, h) = (1024usize, 640usize);
        let cam = LocalCamera { cx: colony.camp.0 as f32 + 4.0, cy: colony.camp.1 as f32 + 2.0, tile_px: 16.0, z: 0, surface_view: true };
        let mut buf = vec![0u32; w * h];
        render_local(&colony.map, atlas, &cam, &mut buf, w, h);
        draw_colony(&colony, &cam, &mut buf, w, h, history);
        lum.push(buf.iter().map(|&q| (((q >> 16) & 255) + ((q >> 8) & 255) + (q & 255)) as f64).sum::<f64>() / (w * h * 3) as f64);
        let path = format!("{prefix}_{name}.png");
        save_rgb_png(&path, w, h, |x, y| { let q = buf[y * w + x]; [(q >> 16) as u8, (q >> 8) as u8, q as u8] });
        written.push(path);
    }
    println!("Night: the midnight frame is {:.0}% darker than noon", 100.0 * (1.0 - lum[1] / lum[0]));
    Ok(written)
}

/// Render viewer frames headlessly (no window): the whole map, then 3.5, 16 and 32 px/tile
/// views around `center` (or the most interesting region). Writes `<prefix>_overview.png`,
/// `<prefix>_3px.png`, `<prefix>_16px.png`, `<prefix>_32px.png`.
pub fn save_snapshots(world: &WorldData, history: Option<&WorldHistory>, atlas: &Atlas, prefix: &str, center: Option<(usize, usize)>, season: crate::seasons::Season, overlay: Overlay) -> Result<Vec<String>, Box<dyn Error>> {
    let mut tw = TileWorld::build(world, atlas);
    tw.show_resources = true;
    tw.set_season(world, season);
    if let Some(h) = history { tw.apply_history(world, h, atlas); }
    tw.overlay = overlays::colors(world, overlay);
    tw.overlay_smooth = overlay.smooth();
    let gaz = build_gazetteer(world, history, world.seed());
    let landmarks = crate::lore::find_landmarks(world, &gaz);
    let labels = build_labels(world, history, &gaz, &landmarks);
    let world_beasts = history.map(super::beasts::world_beasts).unwrap_or_default();
    let world_life = history.map(super::world_ink::world_life).unwrap_or_default();
    {
        use super::world_ink::Kind;
        let n = |f: &dyn Fn(&Kind) -> bool| world_life.iter().filter(|t| f(&t.kind)).count();
        println!("World life: {} monuments, {} war hosts, {} sieges, {} outlaw camps, {} caravans, {} wild populations, {} cult altars; {} beasts at their lairs",
            n(&|k| matches!(k, Kind::Monument(_))), n(&|k| matches!(k, Kind::WarHost { .. })), n(&|k| matches!(k, Kind::Siege { .. })), n(&|k| matches!(k, Kind::Outlaws)),
            n(&|k| matches!(k, Kind::Caravan)), n(&|k| matches!(k, Kind::Wild(_))), n(&|k| matches!(k, Kind::Cult)), world_beasts.len());
        // The first of each kind, for checking its sprite (`PLANET_WORLD_LIFE=1`).
        if std::env::var("PLANET_WORLD_LIFE").is_ok() { for t in &world_life { println!("  {:?} at {:?}: {}", std::mem::discriminant(&t.kind), t.tile, t.name); } }
    }
    let (cx, cy) = center.unwrap_or_else(|| crate::region::zoom::pick_interesting_window(world, ZoomParams::default().tiles));
    let (w, h) = (1280usize, 800usize);
    let fit = (w as f32 / tw.width as f32).min(h as f32 / tw.height as f32);
    let shots = [
        ("overview", Camera { cx: tw.width as f32 / 2.0, cy: tw.height as f32 / 2.0, tile_px: fit }),
        // Just below the detailed-tile threshold (4 px): the far-zoom fill at its largest.
        ("3px", Camera { cx: cx as f32 + 0.5, cy: clamp_cy(cy as f32 + 0.5, 3.5, h, tw.height), tile_px: 3.5 }),
        ("16px", Camera { cx: cx as f32 + 0.5, cy: clamp_cy(cy as f32 + 0.5, 16.0, h, tw.height), tile_px: 16.0 }),
        ("32px", Camera { cx: cx as f32 + 0.5, cy: clamp_cy(cy as f32 + 0.5, 32.0, h, tw.height), tile_px: 32.0 }),
    ];
    let mut written = Vec::new();
    let mut buf = vec![0u32; w * h];
    {
        // A plate of the 16 px view, as P makes it.
        let cam = Camera { cx: cx as f32 + 0.5, cy: clamp_cy(cy as f32 + 0.5, 16.0, h, tw.height), tile_px: 16.0 };
        render_world(&tw, atlas, &cam, &mut buf, w, h);
        super::world_ink::draw(&world_life, &cam, tw.width, &mut buf, w, h);
                    super::beasts::draw_world(&world_beasts, &cam, tw.width, &mut buf, w, h, false);
        draw_labels(&labels, &cam, tw.width, &mut buf, w, h);
        draw_notes(&crate::lore::notes::load(world.seed()), None, &cam, tw.width, &mut buf, w, h);
        let info = plate_info(world, history, &gaz, &tw, &cam, w, h, season);
        super::plates::decorate(&mut buf, w, h, &info);
        let path = format!("{prefix}_plate.png");
        super::plates::save(&path, &buf, w, h, (cam.cx, cam.cy, cam.tile_px), &info)?;
        written.push(path);
    }
    for (name, cam) in shots {
        // Best of three renders: the frame time the window would see.
        let mut best = f32::MAX;
        for _ in 0..3 {
            let t0 = std::time::Instant::now();
            render_world(&tw, atlas, &cam, &mut buf, w, h);
            super::world_ink::draw(&world_life, &cam, tw.width, &mut buf, w, h);
                    super::beasts::draw_world(&world_beasts, &cam, tw.width, &mut buf, w, h, false);
            best = best.min(t0.elapsed().as_secs_f32() * 1000.0);
        }
        println!("render {name}: {best:.1} ms");
        let t0 = std::time::Instant::now();
        draw_labels_avoiding(&labels, &cam, tw.width, &mut buf, w, h, &[minimap_box(tw.width, tw.height, w, h)]);
        println!("labels {name}: {:.1} ms", t0.elapsed().as_secs_f32() * 1000.0);
        draw_notes(&crate::lore::notes::load(world.seed()), None, &cam, tw.width, &mut buf, w, h);
        render_minimap(&tw, &cam, Some((cx as f32, cy as f32, ZoomParams::default().tiles as f32 / 2.0)), &mut buf, w, h);
        overlays::draw_legend(&mut buf, w, h, overlay);
        let path = format!("{prefix}_{name}.png");
        let img = image::RgbImage::from_fn(w as u32, h as u32, |x, y| {
            let p = buf[y as usize * w + x as usize];
            image::Rgb([(p >> 16) as u8, (p >> 8) as u8, p as u8])
        });
        img.save(&path)?;
        written.push(path);
    }
    Ok(written)
}

/// Pick an embark spot in a region: the river cell nearest the centre (so the playable area has
/// water to look at), or the centre itself.
pub fn pick_embark_spot(region: &ZoomRegion) -> (f64, f64) {
    let (w, h) = (region.width, region.height);
    let (cx, cy) = (w as f64 / 2.0, h as f64 / 2.0);
    let mut best = (cx, cy);
    let mut best_d = f64::MAX;
    for y in 0..h {
        for x in 0..w {
            let k = y * w + x;
            if region.river_width_m[k] > 0.0 && region.elevation_m[k] > 0.0 {
                let d = (x as f64 + 0.5 - cx).powi(2) + (y as f64 + 0.5 - cy).powi(2);
                if d < best_d { best_d = d; best = (x as f64 + 0.5, y as f64 + 0.5); }
            }
        }
    }
    best
}

/// Render a playable area headlessly: surface view, the floor level at the centre, a level a few
/// z below it, and a cross-section through the middle row.
pub fn save_local_snapshots(world: &WorldData, history: Option<&WorldHistory>, atlas: &Atlas, center: Option<(usize, usize)>, prefix: &str) -> Result<Vec<String>, Box<dyn Error>> {
    use super::render::{render_cross_section, render_local, LocalCamera};
    // Default: the largest living settlement (else the most interesting terrain).
    let biggest = history.and_then(|h| h.settlements.values().filter(|s| !s.is_destroyed()).max_by_key(|s| s.population).map(|s| s.location));
    let tile = center.or(biggest).unwrap_or_else(|| crate::region::zoom::pick_interesting_window(world, ZoomParams::default().tiles));
    let zs = load_region(world, history, tile, world.seed());
    let mut written = Vec::new();
    {
        let path = format!("{prefix}_region.png");
        let (rw, rh) = (zs.region.width, zs.region.height);
        // Render the way the window does (so labels show); a 1:1 view of the region centre.
        let (vw, vh) = (1280usize, 800usize);
        let mut vbuf = vec![0u32; vw * vh];
        let cam = ZoomCamera { cx: rw as f32 / 2.0, cy: rh as f32 / 2.0, px_per_cell: 1.0 };
        render_zoom(&zs.rgb, rw, rh, &cam, &mut vbuf, vw, vh);
        if let Some(l) = &zs.lore { draw_region_labels(l, &cam, &mut vbuf, vw, vh); }
        image::RgbImage::from_fn(vw as u32, vh as u32, |x, y| {
            let p = vbuf[y as usize * vw + x as usize];
            image::Rgb([(p >> 16) as u8, (p >> 8) as u8, p as u8])
        }).save(&path)?;
        written.push(path);
    }
    // Embark on the settlement in this tile if there is one, else by a river.
    let s = cells_per_tile() as f64;
    let target = zs.lore.as_ref().and_then(|l| {
        let (tx, ty) = (zs.region.world_x0 as f64 * s, zs.region.world_y0 as f64 * s);
        l.sites.iter().find(|site| {
            let (gx, gy) = (site.x + tx, site.y + ty);
            (gx / s) as usize == tile.0 && (gy / s) as usize == tile.1
        }).map(|site| {
            println!("Embarking on {} ({:?}, walls {:?}, {:?}{}{}, core {:.0} m)", site.name, site.kind, site.walls, site.style,
                if site.carved { ", carved" } else { "" }, site.destroyed_year.map(|y| format!(", fell in {y}")).unwrap_or_default(), site.core_m);
            (site.x, site.y)
        })
    });
    let (ex, ey) = target.unwrap_or_else(|| pick_embark_spot(&zs.region));
    // PLANET_LOCAL_OFFSET="dx,dy" (metres) moves the embark off the town's centre (to its walls).
    let (ex, ey) = match std::env::var("PLANET_LOCAL_OFFSET").ok().and_then(|v| v.split_once(',').and_then(|(a, b)| Some((a.trim().parse::<f64>().ok()?, b.trim().parse::<f64>().ok()?)))) {
        Some((dx, dy)) => (ex + dx / zs.region.cell_m as f64, ey + dy / zs.region.cell_m as f64),
        None => (ex, ey),
    };
    let region = &zs.region;
    let t0 = std::time::Instant::now();
    let map = crate::local::generate_local(world, region, zs.lore.as_ref(), ex, ey);
    println!("Playable area: {}x{} tiles x {} z-levels, biome {:?}, generated in {:.2}s", map.width, map.height, map.depth, map.biome, t0.elapsed().as_secs_f32());
    let site = crate::local::site::report(&map);
    println!("Site ({} kinds): {}", site.len(), site.join(", "));
    town_levels_report(&map);
    if std::env::var("PLANET_LOCAL_AUDIT").is_ok() {
        let issues = map.audit();
        if issues.is_empty() { println!("Audit: clean"); } else { for i in &issues { println!("Audit: {i}"); } }
    }
    let n = map.width;
    let cz = map.surface_z[(n / 2) * n + n / 2];
    let (w, h) = (n * 6, n * 6);
    let mut buf = vec![0u32; w * h];
    let mut save = |name: &str, buf: &[u32]| -> Result<(), Box<dyn Error>> {
        let path = format!("{prefix}_{name}.png");
        let img = image::RgbImage::from_fn(w as u32, h as u32, |x, y| {
            let p = buf[y as usize * w + x as usize];
            image::Rgb([(p >> 16) as u8, (p >> 8) as u8, p as u8])
        });
        img.save(&path)?;
        written.push(path);
        Ok(())
    };
    let cam = |z: i32, surface_view: bool| LocalCamera { cx: n as f32 / 2.0, cy: n as f32 / 2.0, tile_px: 6.0, z, surface_view };
    render_local(&map, atlas, &cam(cz, true), &mut buf, w, h);
    save("surface", &buf)?;
    render_local(&map, atlas, &cam(cz, false), &mut buf, w, h);
    save(&format!("z{cz}"), &buf)?;
    render_local(&map, atlas, &cam(cz - 4, false), &mut buf, w, h);
    save(&format!("z{}", cz - 4), &buf)?;
    render_local(&map, atlas, &cam(cz - 12, false), &mut buf, w, h);
    save(&format!("z{}", cz - 12), &buf)?;
    // PLANET_LOCAL_Z="1,2,3": more level slices at those absolute z-levels (the bottom of the
    // map is the magma sea at 1-3), written to `<prefix>_lvl<z>.png`.
    if let Ok(list) = std::env::var("PLANET_LOCAL_Z") {
        for z in list.split(',').filter_map(|s| s.trim().parse::<i32>().ok()) {
            render_local(&map, atlas, &cam(z.clamp(0, map.depth as i32 - 1), false), &mut buf, w, h);
            save(&format!("lvl{z}"), &buf)?;
        }
    }
    // A close-up at the viewer's real scale (16 px per tile), at the centre or at the cell
    // given by PLANET_LOCAL_CLOSE="x,y" (for checking a particular spot).
    let (cw, ch) = (1024usize, 640usize);
    let mut close = vec![0u32; cw * ch];
    let (ccx, ccy) = std::env::var("PLANET_LOCAL_CLOSE").ok()
        .and_then(|v| v.split_once(',').and_then(|(a, b)| Some((a.trim().parse::<f32>().ok()?, b.trim().parse::<f32>().ok()?))))
        .unwrap_or((n as f32 / 2.0, n as f32 / 2.0));
    let t0 = std::time::Instant::now();
    render_local(&map, atlas, &LocalCamera { cx: ccx, cy: ccy, tile_px: 16.0, z: cz, surface_view: true }, &mut close, cw, ch);
    println!("render close-up (1024x640 at 16 px): {:.1} ms", t0.elapsed().as_secs_f64() * 1000.0);
    let mut big = vec![0u32; 1280 * 800];
    let t0 = std::time::Instant::now();
    render_local(&map, atlas, &LocalCamera { cx: ccx, cy: ccy, tile_px: 6.0, z: cz, surface_view: true }, &mut big, 1280, 800);
    println!("render 1280x800 at 6 px: {:.1} ms", t0.elapsed().as_secs_f64() * 1000.0);
    let path = format!("{prefix}_close16.png");
    image::RgbImage::from_fn(cw as u32, ch as u32, |x, y| {
        let p = close[y as usize * cw + x as usize];
        image::Rgb([(p >> 16) as u8, (p >> 8) as u8, p as u8])
    }).save(&path)?;
    written.push(path);
    // Levels relative to the centre's ground (PLANET_LOCAL_LEVELS="2,4,-2": a town's upper floors,
    // roofs and cellars), whole and as a 16 px close-up at the close-up's spot.
    if let Ok(list) = std::env::var("PLANET_LOCAL_LEVELS") {
        for dz in list.split(',').filter_map(|s| s.trim().parse::<i32>().ok()) {
            let z = cz + dz;
            render_local(&map, atlas, &cam(z, false), &mut buf, w, h);
            render_local(&map, atlas, &LocalCamera { cx: ccx, cy: ccy, tile_px: 16.0, z, surface_view: false }, &mut close, cw, ch);
            for (path, b, bw, bh) in [(format!("{prefix}_lvl{z}.png"), &buf, w, h), (format!("{prefix}_lvl{z}_close.png"), &close, cw, ch)] {
                image::RgbImage::from_fn(bw as u32, bh as u32, |x, y| {
                    let p = b[y as usize * bw + x as usize];
                    image::Rgb([(p >> 16) as u8, (p >> 8) as u8, p as u8])
                }).save(&path)?;
                written.push(path);
            }
        }
    }
    // The section's row: the middle, or PLANET_LOCAL_ROW.
    let row = std::env::var("PLANET_LOCAL_ROW").ok().and_then(|v| v.trim().parse::<usize>().ok()).filter(|&r| r < n).unwrap_or(n / 2);
    let section = render_cross_section(&map, row, 4);
    let path = format!("{prefix}_section.png");
    section.save(&path)?;
    written.push(path);
    Ok(written)
}

/// Print what the towns on an embark are in three dimensions (`local::structures`), and check
/// them: each keep's and tower's roof, each upper floor and each cellar walked to from the
/// street (`nav::path3`), and the ground solid below the surface but for caverns, places and
/// cellars.
fn town_levels_report(map: &crate::local::LocalMap) {
    use crate::colony::nav::{path3, standable};
    use crate::local::structures::BuildingKind as K;
    use crate::local::Shape;
    let b = &map.buildings;
    if b.is_empty() { return; }
    let n = map.width;
    let sz = |x: usize, y: usize| map.surface_z[y * n + x];
    let count = |f: &dyn Fn(&crate::local::structures::Building) -> bool| b.iter().filter(|x| f(x)).count();
    let cellars: Vec<_> = b.iter().filter(|x| !x.cellar.is_empty()).collect();
    let cellar_cells: usize = cellars.iter().map(|x| x.cellar.len()).sum();
    let downs: Vec<i32> = cellars.iter().map(|x| sz(x.cellar[0].0 .0 as usize, x.cellar[0].0 .1 as usize) - x.cellar[0].1).collect();
    println!("Town in three dimensions: {} buildings ({} ruined): {} standing two storeys or more, {} lofts, {} keeps, {} towers on the walls; {} cellars ({} cells, {}-{} levels down, {} under ruins)",
        b.len(), count(&|x| x.ruined), count(&|x| !x.ruined && x.storeys >= 2), count(&|x| !x.ruined && x.loft), count(&|x| x.kind == K::Keep), count(&|x| x.kind == K::Tower),
        cellars.len(), cellar_cells, downs.iter().min().unwrap_or(&0), downs.iter().max().unwrap_or(&0), count(&|x| x.ruined && !x.cellar.is_empty()));
    // From the street: the nearest open ground outside every roof, round the stair.
    let street = |s: (u16, u16)| -> Option<(u16, u16, i32)> {
        for r in 1..24i32 { for dy in -r..=r { for dx in -r..=r {
            if dx.abs() != r && dy.abs() != r { continue; }
            let (x, y) = (s.0 as i32 + dx, s.1 as i32 + dy);
            if x < 0 || y < 0 || x >= n as i32 || y >= map.height as i32 { continue; }
            let (x, y) = (x as usize, y as usize);
            if map.roofs[y * n + x] == 0 && standable(map, x, y, sz(x, y)) && map.cell(x, y, sz(x, y) as usize + 1).shape == Shape::Empty
                && !b.iter().any(|o| o.stair == Some((x as u16, y as u16))) { return Some((x as u16, y as u16, sz(x, y))); }
        } } }
        None
    };
    // A cell of floor beside the stair at level z (or the stair's own top).
    let beside = |s: (u16, u16), z: i32| -> Option<(u16, u16, i32)> {
        [(1i32, 0i32), (-1, 0), (0, 1), (0, -1), (0, 0)].iter().map(|&(dx, dy)| ((s.0 as i32 + dx) as usize, (s.1 as i32 + dy) as usize))
            .find(|&(x, y)| x < n && y < map.height && standable(map, x, y, z)).map(|(x, y)| (x as u16, y as u16, z))
    };
    let walk = |s: (u16, u16), to: Option<(u16, u16, i32)>| -> Option<usize> {
        let from = street(s)?;
        path3(map, None, from, to?, 40_000).map(|p| p.len())
    };
    let tally = |what: &str, items: Vec<((u16, u16), Option<(u16, u16, i32)>)>| {
        if items.is_empty() { return; }
        let walked: Vec<usize> = items.iter().filter_map(|&(s, to)| walk(s, to)).collect();
        println!("  {what}: {} of {} walked to from the street{}", walked.len(), items.len(),
            if walked.is_empty() { String::new() } else { format!(" ({}-{} steps)", walked.iter().min().unwrap(), walked.iter().max().unwrap()) });
    };
    tally("keep roofs", b.iter().filter(|x| x.kind == K::Keep).filter_map(|x| Some((x.stair?, beside(x.stair?, x.platform?)))).collect());
    tally("tower roofs", b.iter().filter(|x| x.kind == K::Tower).filter_map(|x| Some((x.stair?, beside(x.stair?, x.platform?)))).collect());
    tally("upper floors", b.iter().filter(|x| !x.ruined && x.kind != K::Tower).flat_map(|x| x.upper.iter().filter_map(move |&z| Some((x.stair?, beside(x.stair?, z))))).collect());
    tally("cellars", cellars.iter().filter_map(|x| { let (c, z) = *x.cellar.last()?; Some((x.cellar[0].0, Some((c.0, c.1, z)))) }).collect());
    // Below the ground: solid but for caverns, places and cellars.
    let cut: std::collections::HashSet<(u16, u16)> = map.places.iter().flat_map(|p| p.cells.iter().map(|c| c.0))
        .chain(b.iter().flat_map(|x| x.cellar.iter().map(|c| c.0))).collect();
    let mut open = 0;
    for y in 0..map.height { for x in 0..n {
        if cut.contains(&(x as u16, y as u16)) { continue; }
        for z in 0..sz(x, y).max(0) as usize {
            let c = map.cell(x, y, z);
            let above = map.cell(x, y, z + 1);
            let bed = c.shape == Shape::Floor && (above.water > 0 || above.material == crate::local::Material::Ice);
            if c.shape != Shape::Wall && c.water == 0 && !bed && map.cavern_at(x, y, z as i32).is_none() { open += 1; }
        }
    } }
    println!("  below the ground: {open} cells open outside caverns, places and cellars");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Walking prefetches a region centred further along; swapping it in must not change what
    /// is on screen. Compare the rendered pixels of two regions over their overlap.
    #[test]
    fn region_swap_is_invisible() {
        let world = crate::world::generate_world_with_style(128, 64, 5, crate::plates::WorldStyle::Earthlike);
        let (cx, cy) = crate::region::zoom::pick_interesting_window(&world, 8);
        let a = load_region(&world, None, (cx, cy), 9);
        let b = load_region(&world, None, (cx + 3, cy), 9);
        let s = cells_per_tile();
        assert_eq!(b.origin.0 - a.origin.0, 3 * s, "regions are positioned in shared global cells");
        // Overlap in global cells, skipping the one-cell border where hillshading is clamped.
        let (x0, x1) = (b.origin.0 + 1, a.origin.0 + a.region.width as i64 - 1);
        let (y0, y1) = (a.origin.1 + 1, a.origin.1 + a.region.height as i64 - 1);
        let (mut diff, mut total) = (0usize, 0usize);
        for gy in y0..y1 {
            for gx in x0..x1 {
                let pa = a.rgb[(gy - a.origin.1) as usize * a.region.width + (gx - a.origin.0) as usize];
                let pb = b.rgb[(gy - b.origin.1) as usize * b.region.width + (gx - b.origin.0) as usize];
                total += 1;
                if pa != pb { diff += 1; }
            }
        }
        let frac = diff as f64 / total as f64;
        println!("pixels differing across a region swap: {:.4}% of {}", frac * 100.0, total);
        assert!(frac < 0.002, "region swap should be visually seamless");
    }
}
