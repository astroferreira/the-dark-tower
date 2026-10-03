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
use crate::lore::{build_gazetteer, FeatureKind, Gazetteer};
use crate::region::zoom::{generate_zoom, ZoomParams, ZoomRegion};
use super::text::{place_labels, Label};
use crate::world::WorldData;

use super::atlas::Atlas;
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
            (true, _) => (300, 1.2, 0x00A0_9890),
            (_, SettlementType::Capital) => (900, 0.0, 0x00FF_E08A),
            (_, SettlementType::City | SettlementType::Port) => (850, 0.0, 0x00FF_E08A),
            (_, SettlementType::Town | SettlementType::Fort) => (700, 0.4, 0x00FF_E08A),
            _ => (400, 0.9, 0x00F0_DCB4),
        };
        let text = if site.destroyed_year.is_some() { format!("ruins of {}", site.name) } else { site.name.clone() };
        Label { x: site.x as f32, y: site.y as f32 + 9.0, text, rank, min_tile_px: min_px, color }
    }).collect();
    place_labels(&labels, cam.px_per_cell, w, h, buf, |x, y| {
        (w as f32 / 2.0 + (x - cam.cx) * cam.px_per_cell, h as f32 / 2.0 + (y - cam.cy) * cam.px_per_cell)
    });
}

/// Everything named on the world map: geographic features and settlements, highest rank first.
fn build_labels(world: &WorldData, history: Option<&WorldHistory>, gaz: &Gazetteer) -> Vec<Label> {
    use crate::history::civilizations::settlement::SettlementType;
    let mut labels = Vec::new();
    for f in &gaz.features {
        let (min_tile_px, color) = match f.kind {
            FeatureKind::Ocean | FeatureKind::Continent => (0.0, if f.kind.is_water() { 0x009C_C8EE } else { 0x00F2_E6C8 }),
            FeatureKind::Sea | FeatureKind::MountainRange | FeatureKind::Desert | FeatureKind::IceField => (2.0, if f.kind.is_water() { 0x009C_C8EE } else { 0x00F2_E6C8 }),
            FeatureKind::Forest | FeatureKind::Jungle | FeatureKind::Plains | FeatureKind::Tundra | FeatureKind::Gulf => (4.0, if f.kind.is_water() { 0x009C_C8EE } else { 0x00E6_DCC0 }),
            FeatureKind::River | FeatureKind::Island => (6.0, if f.kind.is_water() { 0x00A8_D4F4 } else { 0x00E6_DCC0 }),
            FeatureKind::Lake | FeatureKind::Marsh => (10.0, if f.kind.is_water() { 0x00A8_D4F4 } else { 0x00E6_DCC0 }),
            FeatureKind::Peak => (10.0, 0x00FF_FFFF),
        };
        let text = if f.kind == FeatureKind::Peak { format!("{} {:.0}m", f.name, f.height_m) } else { f.name.clone() };
        // Bigger features of a kind rank above smaller ones.
        let rank = f.kind.rank() * 10 + ((f.size as f32).log2() as u32).min(9);
        labels.push(Label { x: f.anchor.0 as f32 + 0.5, y: f.anchor.1 as f32 + 0.5, text, rank, min_tile_px, color });
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
            let color = if s.is_destroyed() { 0x00A0_9890 } else { 0x00FF_E08A };
            labels.push(Label { x: s.location.0 as f32 + 0.5, y: s.location.1 as f32 + 1.4, text, rank: base + (s.population / 2000).min(99), min_tile_px: min_px, color });
        }
    }
    let _ = world;
    labels.sort_by_key(|l| std::cmp::Reverse(l.rank));
    labels
}

/// Place labels for the current world camera (wrapping around the date line).
fn draw_labels(labels: &[Label], cam: &Camera, world_w: usize, buf: &mut [u32], w: usize, h: usize) {
    let ww = world_w as f32;
    place_labels(labels, cam.tile_px, w, h, buf, |x, y| {
        let mut dx = x - cam.cx;
        if dx > ww / 2.0 { dx -= ww; }
        if dx < -ww / 2.0 { dx += ww; }
        (w as f32 / 2.0 + dx * cam.tile_px, h as f32 / 2.0 + (y - cam.cy) * cam.tile_px)
    });
}

/// What is known about a world tile: place names, owner, settlement or ruin.
fn describe_tile(world: &WorldData, history: Option<&WorldHistory>, gaz: &Gazetteer, tw: &TileWorld, x: usize, y: usize) -> String {
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
    let res = world.resources();
    for d in res.deposits_at(x, y) {
        let q = ["poor", "good", "rich"][(d.richness - 1) as usize];
        parts.push(format!("{} {} deposit", q, crate::lore::resource_name(d.kind)));
    }
    let (fert, fish) = (*res.fertility.get(x, y), *res.fish.get(x, y));
    if fert > 0.6 { parts.push("rich farmland".to_string()); } else if fert > 0.35 { parts.push("farmland".to_string()); }
    if fish > 0.35 { parts.push("fishing grounds".to_string()); }
    if let Some(h) = history {
        if let Some(f) = h.tile_history.get(x, y).current_owner.and_then(|id| h.factions.get(&id)) {
            parts.push(format!("held by {}", f.name));
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
pub fn run_tile_viewer(world: &WorldData, history: Option<&WorldHistory>, atlas: Atlas, start: Option<(usize, usize)>) -> Result<(), Box<dyn Error>> {
    println!("Building tile map...");
    let mut tw = TileWorld::build(world, &atlas);
    if let Some(h) = history { tw.apply_history(world, h, &atlas); }
    let gaz = build_gazetteer(world, history, world.seed());
    let labels = build_labels(world, history, &gaz);
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
        "Planet viewer",
        1280,
        800,
        WindowOptions { resize: true, ..WindowOptions::default() },
    )?;
    window.set_target_fps(60);

    std::thread::scope(|scope| -> Result<(), Box<dyn Error>> {
        let (sx, sy) = start.unwrap_or((tw.width / 2, tw.height / 2));
        let mut cam = Camera { cx: sx as f32 + 0.5, cy: sy as f32 + 0.5, tile_px: 16.0 };

        // Walking state: the current region, the player in global cells, the zoom level, and a
        // region being generated in the background.
        let mut zoom: Option<ZoomState> = None;
        let mut zoom_active = false;
        let mut player = (0.0f64, 0.0f64);
        let mut px_per_cell = 4.0f32;
        let mut pending: Option<std::thread::ScopedJoinHandle<'_, ZoomState>> = None;
        // Playable area (embark) state.
        let mut local: Option<(LocalMap, LocalCamera)> = None;
        let mut local_active = false;

        let mut show_minimap = true;
        let mut buf: Vec<u32> = Vec::new();
        let mut size = (0usize, 0usize);
        let mut dirty = true;
        let mut drag: Option<((f32, f32), (f32, f32))> = None;
        let mut minimap_rect = (0usize, 0usize, 0usize, 0usize);
        let mut status = String::from("wheel: zoom | drag/arrows: pan | Z: walk here | N: minimap | P: screenshot | Q: quit");
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
            let pressed = |k: Key| window.is_key_pressed(k, KeyRepeat::No);
            let held = |k: Key| window.is_key_down(k);
            let pan_x = (held(Key::Right) || held(Key::D)) as i32 - (held(Key::Left) || held(Key::A)) as i32;
            let pan_y = (held(Key::Down) || held(Key::S)) as i32 - (held(Key::Up) || held(Key::W)) as i32;
            let running = held(Key::LeftShift) || held(Key::RightShift);
            let zoom_key = if pressed(Key::Equal) || pressed(Key::NumPadPlus) { 1.0 } else if pressed(Key::Minus) || pressed(Key::NumPadMinus) { -1.0 } else { 0.0 };
            let step = if wheel != 0.0 { wheel.signum() } else { 0.0 } + zoom_key;

            if local_active {
                let (map, lcam) = local.as_mut().unwrap();
                if pressed(Key::Escape) || pressed(Key::Q) {
                    local_active = false;
                    dirty = true;
                }
                if step != 0.0 {
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
                    (true, None) => drag = Some((mouse, (lcam.cx, lcam.cy))),
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
                    lcam.z = (lcam.z + up as i32 - dn as i32).clamp(0, map.depth as i32 - 1);
                    lcam.surface_view = false;
                    dirty = true;
                }
                if pressed(Key::V) { lcam.surface_view = !lcam.surface_view; dirty = true; }
                if pressed(Key::P) {
                    let path = format!("embark_{seed}.png");
                    status = save_rgb_png(&path, w, h, |x, y| {
                        let p = buf[y * w + x];
                        [(p >> 16) as u8, (p >> 8) as u8, p as u8]
                    });
                }
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
                        Shape::Empty if c.water > 0 => format!("water {}/7", c.water),
                        Shape::Empty => "open space".to_string(),
                    };
                    format!("({tx},{ty}) {what}")
                } else {
                    String::new()
                };
                let view = if lcam.surface_view { "surface view".to_string() } else { format!("z {} ({:.0} m)", lcam.z, map.z_elevation(lcam.z)) };
                let title = format!(
                    "Playable area {}x{} ({:?}) | {} | {} | </> level, V surface, wheel zoom, P screenshot, Esc back | {}",
                    map.width, map.height, map.biome, view, info, status
                );
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

                if pressed(Key::Enter) {
                    // Embark: the playable area is centred on the walker.
                    window.set_title("Generating playable area...");
                    let t0 = std::time::Instant::now();
                    let map = generate_local(world, &z.region, z.lore.as_ref(), player.0 - z.origin.0 as f64, player.1 - z.origin.1 as f64);
                    let cz = map.surface_z[(map.height / 2) * map.width + map.width / 2];
                    let lcam = LocalCamera { cx: map.width as f32 / 2.0, cy: map.height as f32 / 2.0, tile_px: 16.0, z: cz, surface_view: false };
                    status = format!("embarked in {:.2}s", t0.elapsed().as_secs_f32());
                    local = Some((map, lcam));
                    local_active = true;
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
                let title = format!(
                    "{} | {} | {:.1}°C | arrows/WASD walk, Shift run, wheel zoom, Enter embark, X save, Esc map{} | {}",
                    place, ground, r.temperature_c[k], loading, status
                );
                let _ = km;
                if title != last_title { window.set_title(&title); last_title = title; }
            } else {
                if pressed(Key::Escape) || pressed(Key::Q) {
                    break;
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
                    (true, _) if on_minimap => {
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
                cam.cy = cam.cy.clamp(0.0, tw.height as f32);
                if pressed(Key::N) { show_minimap = !show_minimap; dirty = true; }
                if pressed(Key::P) {
                    let path = format!("view_{seed}.png");
                    status = save_rgb_png(&path, w, h, |x, y| {
                        let p = buf[y * w + x];
                        [(p >> 16) as u8, (p >> 8) as u8, p as u8]
                    });
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
                        // Dim the frame so it is obvious work is happening (a few seconds).
                        for p in buf.iter_mut() { *p = (*p >> 1) & 0x007F_7F7F; }
                        window.set_title(&format!("Simulating region around tile ({}, {})...", tile.0, tile.1));
                        window.update_with_buffer(&buf, w, h)?;
                        let t0 = std::time::Instant::now();
                        if let Some(p) = pending.take() { let _ = p.join(); }
                        zoom = Some(load_region(world, history, tile, seed));
                        status = format!("simulated in {:.1}s", t0.elapsed().as_secs_f32());
                    }
                    player = target;
                    zoom_active = true;
                    dirty = true;
                    continue;
                }

                if pressed(Key::L) { show_labels = !show_labels; dirty = true; }
                if pressed(Key::R) { tw.show_resources = !tw.show_resources; dirty = true; }
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
                let place = describe_tile(world, history, &gaz, &tw, tile.0, tile.1);
                let title = format!(
                    "({},{}) {} | {:.0} m | {} {}°C | T season, C auto{}, L labels, R resources | {}",
                    tile.0, tile.1, place, world.heightmap.get(tile.0, tile.1), season.name(),
                    format!("{:.1}", world.seasonal_climate.as_ref().map(|c| c.get_temperature(tile.0, tile.1, season, tile.1 < tw.height / 2)).unwrap_or(*world.temperature.get(tile.0, tile.1))),
                    if auto_season.is_some() { " ON" } else { "" }, status
                );
                if title != last_title { window.set_title(&title); last_title = title; }
            }

            if dirty {
                if local_active {
                    let (map, lcam) = local.as_ref().unwrap();
                    render_local(map, &atlas, lcam, &mut buf, w, h);
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
                    if show_labels {
                        draw_labels(&labels, &cam, tw.width, &mut buf, w, h);
                    }
                    minimap_rect = if show_minimap {
                        let (hx, hy) = screen_to_world(&cam, mouse.0, mouse.1, w, h);
                        render_minimap(&tw, &cam, Some((hx, hy, zoom_tiles as f32 / 2.0)), &mut buf, w, h)
                    } else {
                        (0, 0, 0, 0)
                    };
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

/// Render viewer frames headlessly (no window): the whole map, then 16 and 32 px/tile close-ups
/// around `center` (or the most interesting region). Writes `<prefix>_overview.png`, `<prefix>_16px.png`,
/// `<prefix>_32px.png`.
pub fn save_snapshots(world: &WorldData, history: Option<&WorldHistory>, atlas: &Atlas, prefix: &str, center: Option<(usize, usize)>, season: crate::seasons::Season) -> Result<Vec<String>, Box<dyn Error>> {
    let mut tw = TileWorld::build(world, atlas);
    tw.show_resources = true;
    tw.set_season(world, season);
    if let Some(h) = history { tw.apply_history(world, h, atlas); }
    let gaz = build_gazetteer(world, history, world.seed());
    let labels = build_labels(world, history, &gaz);
    let (cx, cy) = center.unwrap_or_else(|| crate::region::zoom::pick_interesting_window(world, ZoomParams::default().tiles));
    let (w, h) = (1280usize, 800usize);
    let fit = (w as f32 / tw.width as f32).min(h as f32 / tw.height as f32);
    let shots = [
        ("overview", Camera { cx: tw.width as f32 / 2.0, cy: tw.height as f32 / 2.0, tile_px: fit }),
        ("16px", Camera { cx: cx as f32 + 0.5, cy: cy as f32 + 0.5, tile_px: 16.0 }),
        ("32px", Camera { cx: cx as f32 + 0.5, cy: cy as f32 + 0.5, tile_px: 32.0 }),
    ];
    let mut written = Vec::new();
    let mut buf = vec![0u32; w * h];
    for (name, cam) in shots {
        render_world(&tw, atlas, &cam, &mut buf, w, h);
        draw_labels(&labels, &cam, tw.width, &mut buf, w, h);
        render_minimap(&tw, &cam, Some((cx as f32, cy as f32, ZoomParams::default().tiles as f32 / 2.0)), &mut buf, w, h);
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
        }).map(|site| (site.x, site.y))
    });
    let (ex, ey) = target.unwrap_or_else(|| pick_embark_spot(&zs.region));
    let region = &zs.region;
    let t0 = std::time::Instant::now();
    let map = crate::local::generate_local(world, region, zs.lore.as_ref(), ex, ey);
    println!("Playable area: {}x{} tiles x {} z-levels, biome {:?}, generated in {:.2}s", map.width, map.height, map.depth, map.biome, t0.elapsed().as_secs_f32());
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
    // A close-up at the viewer's real scale (16 px per tile).
    let (cw, ch) = (1024usize, 640usize);
    let mut close = vec![0u32; cw * ch];
    render_local(&map, atlas, &LocalCamera { cx: n as f32 / 2.0, cy: n as f32 / 2.0, tile_px: 16.0, z: cz, surface_view: true }, &mut close, cw, ch);
    let path = format!("{prefix}_close16.png");
    image::RgbImage::from_fn(cw as u32, ch as u32, |x, y| {
        let p = close[y as usize * cw + x as usize];
        image::Rgb([(p >> 16) as u8, (p >> 8) as u8, p as u8])
    }).save(&path)?;
    written.push(path);
    let section = render_cross_section(&map, n / 2, 4);
    let path = format!("{prefix}_section.png");
    section.save(&path)?;
    written.push(path);
    Ok(written)
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
