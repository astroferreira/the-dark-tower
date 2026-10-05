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

pub fn run_tile_viewer(world: &WorldData, history: Option<&WorldHistory>, atlas: Atlas, start: Option<(usize, usize)>, embark: bool) -> Result<(), Box<dyn Error>> {
    println!("Building tile map...");
    let mut tw = TileWorld::build(world, &atlas);
    if let Some(h) = history { tw.apply_history(world, h, &atlas); }
    let gaz = build_gazetteer(world, history, world.seed());
    let landmarks = crate::lore::find_landmarks(world, &gaz);
    let labels = build_labels(world, history, &gaz, &landmarks);
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
        let mut cam = Camera { cx: sx as f32 + 0.5, cy: sy as f32 + 0.5, tile_px: START_ZOOM.get().copied().unwrap_or(16.0) };

        // Walking state: the current region, the player in global cells, the zoom level, and a
        // region being generated in the background.
        let mut zoom: Option<ZoomState> = None;
        let mut zoom_active = false;
        let mut player = (0.0f64, 0.0f64);
        let mut px_per_cell = 4.0f32;
        let mut pending: Option<std::thread::ScopedJoinHandle<'_, ZoomState>> = None;
        // Playable area (embark) state.
        // The playable area and the colony living on it (the colony owns the map).
        let mut local: Option<(crate::colony::Colony, LocalCamera)> = None;
        let mut local_active = false;
        // Game clock: 0 = paused, else game hours per real second (1x, 3x, 10x).
        let mut speed: u32 = 1;
        let mut last_frame = std::time::Instant::now();
        let mut tick_debt = 0.0f64;

        if let (true, Some(tile)) = (embark, start) {
            let t0 = std::time::Instant::now();
            let z = load_region(world, history, tile, seed);
            player = (tile.0 as f64 * s as f64 + s as f64 / 2.0, tile.1 as f64 * s as f64 + s as f64 / 2.0);
            let map = generate_local(world, &z.region, z.lore.as_ref(), player.0 - z.origin.0 as f64, player.1 - z.origin.1 as f64);
            let cz = map.surface_z[(map.height / 2) * map.width + map.width / 2];
            let (mcx, mcy) = (map.width as f32 / 2.0, map.height as f32 / 2.0);
            let colony_seed = seed ^ ((tile.0 as u64) << 20) ^ tile.1 as u64;
            let colony = found_colony(map, history, tile, colony_seed, 7);
            local = Some((colony, LocalCamera { cx: mcx, cy: mcy, tile_px: 16.0, z: cz, surface_view: true }));
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
        // The site report for the embark box: (where it was computed, the line).
        let mut site_line: (Option<(i64, i64)>, String) = (None, String::new());
        let mut inspect_hits: Vec<super::inspector::Hit> = Vec::new();
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
            if down && !was_down { press_at = Some(mouse); }
            let clicked = !down && was_down && press_at.map_or(false, |p| (p.0 - mouse.0).abs() < 4.0 && (p.1 - mouse.1).abs() < 4.0);
            let right_clicked = !right && was_right;
            was_down = down;
            was_right = right;
            let pressed = |k: Key| window.is_key_pressed(k, KeyRepeat::No);
            let held = |k: Key| window.is_key_down(k);
            let pan_x = (held(Key::Right) || held(Key::D)) as i32 - (held(Key::Left) || held(Key::A)) as i32;
            let pan_y = (held(Key::Down) || held(Key::S)) as i32 - (held(Key::Up) || held(Key::W)) as i32;
            let running = held(Key::LeftShift) || held(Key::RightShift);
            let zoom_key = if pressed(Key::Equal) || pressed(Key::NumPadPlus) { 1.0 } else if pressed(Key::Minus) || pressed(Key::NumPadMinus) { -1.0 } else { 0.0 };
            let step = if wheel != 0.0 { wheel.signum() } else { 0.0 } + zoom_key;

            let frame_dt = last_frame.elapsed().as_secs_f64().min(0.25);
            last_frame = std::time::Instant::now();
            if local_active {
                let (colony, lcam) = local.as_mut().unwrap();
                // The clock: Space pauses, 1/2/3 = 1x/3x/10x (1x = a game hour a real second).
                if pressed(Key::Space) { speed = if speed == 0 { 1 } else { 0 }; dirty = true; }
                if pressed(Key::Key1) { speed = 1; }
                if pressed(Key::Key2) { speed = 3; }
                if pressed(Key::Key3) { speed = 10; }
                if speed > 0 {
                    tick_debt += frame_dt * 60.0 * speed as f64;
                    let n = tick_debt.floor() as u64;
                    tick_debt -= n as f64;
                    for _ in 0..n { colony.tick(); }
                    if n > 0 { dirty = true; }
                }
                // The inspector: click a settler to read who they are; Esc closes it first.
                let panel = super::inspector::panel_rect(w, h);
                let over_panel = !inspect.is_empty() && panel.contains(mouse.0, mouse.1);
                if pressed(Key::Escape) && !inspect.is_empty() {
                    inspect.clear();
                    dirty = true;
                } else if pressed(Key::Escape) || pressed(Key::Q) {
                    local_active = false;
                    inspect.clear();
                    dirty = true;
                }
                if (pressed(Key::Backspace) || right_clicked) && !inspect.is_empty() {
                    inspect.pop();
                    dirty = true;
                }
                if clicked {
                    if over_panel {
                        if let Some(hit) = inspect_hits.iter().find(|hh| hh.rect.contains(mouse.0, mouse.1)) {
                            inspect.push(hit.to);
                            dirty = true;
                        }
                    } else {
                        let (hx, hy) = (lcam.cx + (mouse.0 - w as f32 / 2.0) / lcam.tile_px, lcam.cy + (mouse.1 - h as f32 / 2.0) / lcam.tile_px);
                        if let Some(i) = colony.settlers.iter().position(|st| (st.pos.0 as f32 + 0.5 - hx).abs() < 0.9 && (st.pos.1 as f32 + 0.5 - hy).abs() < 0.9) {
                            inspect = vec![super::inspector::Subject::Settler(i)];
                            dirty = true;
                        } else if let Some(i) = colony.marks.iter().position(|m| (m.at.0 as f32 + 0.5 - hx).abs() < 0.9 && (m.at.1 as f32 + 0.5 - hy).abs() < 0.9) {
                            inspect = vec![super::inspector::Subject::ColonyMark(i)];
                            dirty = true;
                        }
                    }
                }
                // The patron's verbs at the mouse: F bless the ground, X forbid it, G favour the
                // settler under it, D send them a dream (of the hut while it stands unfinished,
                // of plenty when food is short, else of rest).
                {
                    let (mx, my) = (lcam.cx + (mouse.0 - w as f32 / 2.0) / lcam.tile_px, lcam.cy + (mouse.1 - h as f32 / 2.0) / lcam.tile_px);
                    let at = (mx.max(0.0) as u16, my.max(0.0) as u16);
                    let who = colony.settlers.iter().position(|st| st.alive && (st.pos.0 as f32 + 0.5 - mx).abs() < 0.9 && (st.pos.1 as f32 + 0.5 - my).abs() < 0.9);
                    let said = if pressed(Key::H) { Some(colony.place_stone(crate::colony::StoneKind::Hall, at)) }
                        else if pressed(Key::J) { Some(colony.place_stone(crate::colony::StoneKind::Grove, at)) }
                        else if pressed(Key::K) { Some(colony.place_stone(crate::colony::StoneKind::Shrine, at)) }
                        else if pressed(Key::F) { Some(colony.mark_place(at, 6, false)) }
                        else if pressed(Key::X) { Some(colony.mark_place(at, 6, true)) }
                        else if pressed(Key::G) { who.map(|i| colony.favour_settler(i)) }
                        else if pressed(Key::D) {
                            let dream = if colony.hut.as_ref().map_or(false, |hh| !hh.done) { crate::colony::Dream::Hut }
                                else if colony.food_stored() < 3 * colony.alive() as u32 { crate::colony::Dream::Plenty } else { crate::colony::Dream::Rest };
                            who.map(|i| colony.send_dream(i, dream))
                        } else { None };
                    if let Some(r) = said { status = match r { Ok(l) => l, Err(e) => e }; dirty = true; }
                }
                let map = &colony.map;
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
                let who = colony.settlers.iter().filter(|s| s.alive)
                    .find(|s| (s.pos.0 as f32 + 0.5 - hx).abs() < 0.8 && (s.pos.1 as f32 + 0.5 - hy).abs() < 0.8)
                    .map(|s| format!("{}: {} - {} | ", s.name, s.job.verb(), s.why)).unwrap_or_default();
                let clock = format!("{} {}", colony.clock.stamp(), if speed == 0 { "(paused)".to_string() } else { format!("{}x", speed) });
                let title = format!(
                    "{} | favour {} | {}{} | {} | Space pause, 1/2/3 speed, F bless, X forbid, G favour, D dream, H/J/K hall/grove/shrine stone, </> level, V surface, Esc back | {}",
                    clock, colony.patron.favour, who, view, info, status
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
                    let lcam = LocalCamera { cx: map.width as f32 / 2.0, cy: map.height as f32 / 2.0, tile_px: 16.0, z: cz, surface_view: true };
                    status = format!("embarked in {:.2}s", t0.elapsed().as_secs_f32());
                    let colony_seed = seed ^ (player.0 as u64) << 20 ^ player.1 as u64;
                    let here = ((player.0 / s as f64) as usize % world.width, ((player.1 / s as f64) as usize).min(world.height - 1));
                    let colony = found_colony(map, history, here, colony_seed, 7);
                    local = Some((colony, lcam));
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
                // What an embark here would hold, recomputed when the box moves on (~30 ms).
                let key = ((player.0 / 24.0) as i64, (player.1 / 24.0) as i64);
                if site_line.0 != Some(key) {
                    let m = generate_local(world, &z.region, z.lore.as_ref(), player.0 - z.origin.0 as f64, player.1 - z.origin.1 as f64);
                    site_line = (Some(key), crate::local::site::report(&m).join(", "));
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
                cam.cy = cam.cy.clamp(0.0, tw.height as f32);
                if pressed(Key::N) { show_minimap = !show_minimap; dirty = true; }
                if pressed(Key::P) {
                    // A plate: the map without the interface, framed, captioned and numbered.
                    let mut plate = vec![0u32; w * h];
                    render_world(&tw, &atlas, &cam, &mut plate, w, h);
                    if show_labels { draw_labels(&labels, &cam, tw.width, &mut plate, w, h); }
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
                    render_local(&colony.map, &atlas, lcam, &mut buf, w, h);
                    if lcam.surface_view { super::local_ink::draw_colony(colony, lcam, &mut buf, w, h); }
                    inspect_hits.clear();
                    if let Some(&subject) = inspect.last() {
                        let page = match subject {
                            super::inspector::Subject::Settler(i) => colony.settlers.get(i).map(|st| super::inspector::settler_page(history, st)),
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
                    if show_labels {
                        let avoid = if show_minimap { vec![minimap_box(tw.width, tw.height, w, h)] } else { Vec::new() };
                        draw_labels_avoiding(&labels, &cam, tw.width, &mut buf, w, h, &avoid);
                    }
                    overlays::draw_legend(&mut buf, w, h, overlay);
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
    let cam = Camera { cx: tile.0 as f32 + 0.5 + (panel.w as f32 / 2.0) / 16.0, cy: tile.1 as f32 + 0.5, tile_px: 16.0 };
    let mut map = vec![0u32; w * h];
    render_world(&tw, atlas, &cam, &mut map, w, h);
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
        println!("Founding {}: hut {:?}, {} trees left around {:?}, {} alive",
            if with_stones { "with stones" } else { "without stones" }, hut, grove_trees, grove_at, colony.alive());
        huts.push(hut);
        let (w, h) = (1024usize, 1024usize);
        let cam = LocalCamera { cx: colony.camp.0 as f32 + 4.0, cy: colony.camp.1 as f32, tile_px: 8.0, z: 0, surface_view: true };
        let mut buf = vec![0u32; w * h];
        render_local(&colony.map, atlas, &cam, &mut buf, w, h);
        draw_colony(&colony, &cam, &mut buf, w, h);
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
    draw_colony(&colony, &cam, &mut buf, w, h);
    if let Some(g) = colony.marks.iter().find(|m| m.kind == crate::colony::MarkKind::Grave) {
        super::inspector::draw(&super::inspector::mark_page(g), &mut buf, w, h, 1);
    }
    let path = format!("{prefix}_marks.png");
    save_rgb_png(&path, w, h, |x, y| { let p = buf[y * w + x]; [(p >> 16) as u8, (p >> 8) as u8, p as u8] });
    println!("Saved {path}");
    Ok(())
}

/// Found the colony on an embark at world `tile`: with a history, the settlers come out of it
/// (`history::settlers::roster`: survivors, veterans, kin); without one they are nameless
/// wanderers with stock names.
fn found_colony(map: crate::local::LocalMap, history: Option<&WorldHistory>, tile: (usize, usize), seed: u64, n: usize) -> crate::colony::Colony {
    match history {
        Some(h) => {
            let roster = crate::history::settlers::roster(h, tile, n, seed);
            let names: Vec<String> = roster.iter().map(|r| r.0.clone()).collect();
            let mut colony = crate::colony::Colony::found(map, &names, seed);
            for (st, (_, past)) in colony.settlers.iter_mut().zip(roster) { st.past = Some(past); }
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
pub fn save_colony_snapshots(world: &WorldData, history: Option<&WorldHistory>, atlas: &Atlas, tile: (usize, usize), prefix: &str) -> Result<Vec<String>, Box<dyn Error>> {
    use super::local_ink::draw_colony;
    let zs = load_region(world, history, tile, world.seed());
    let s = cells_per_tile() as f64;
    let (ex, ey) = {
        let (tx, ty) = (tile.0 as f64 * s + s / 2.0 - zs.origin.0 as f64, tile.1 as f64 * s + s / 2.0 - zs.origin.1 as f64);
        (tx, ty)
    };
    let map = crate::local::generate_local(world, &zs.region, zs.lore.as_ref(), ex, ey);
    let seed = world.seed() ^ ((tile.0 as u64) << 20) ^ tile.1 as u64;
    let mut colony = found_colony(map, history, tile, seed, 7);
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
    }
    let shared = seen.values().filter(|n| **n >= 2).count();
    std::fs::write(format!("{prefix}_settlers.txt"), lines.join("\n") + "\n")?;
    println!("Settlers: {} with pasts, each citing at least {} events; {} events shared by two or more",
        colony.settlers.iter().filter(|s| s.past.is_some()).count(), if fewest == usize::MAX { 0 } else { fewest }, shared);
    let mut written = Vec::new();
    let mut day_done = 0u64;
    for day in [1u64, 10, 30] {
        let t0 = std::time::Instant::now();
        colony.run_days(day - day_done);
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
            draw_colony(&colony, &cam, &mut buf, w, h);
            let path = format!("{prefix}_{name}.png");
            save_rgb_png(&path, w, h, |x, y| { let q = buf[y * w + x]; [(q >> 16) as u8, (q >> 8) as u8, q as u8] });
            written.push(path);
        }
    }
    // The first settler's inspector page, as a click shows it.
    {
        let (w, h) = (1280usize, 800usize);
        let mut buf = vec![0u32; w * h];
        let st = &colony.settlers[0];
        let cam = LocalCamera { cx: st.pos.0 as f32 + 0.5, cy: st.pos.1 as f32 + 0.5, tile_px: 16.0, z: 0, surface_view: true };
        render_local(&colony.map, atlas, &cam, &mut buf, w, h);
        draw_colony(&colony, &cam, &mut buf, w, h);
        let page = super::inspector::settler_page(history, st);
        super::inspector::draw(&page, &mut buf, w, h, 1);
        let path = format!("{prefix}_settler.png");
        save_rgb_png(&path, w, h, |x, y| { let p = buf[y * w + x]; [(p >> 16) as u8, (p >> 8) as u8, p as u8] });
        written.push(path);
    }
    let path = format!("{prefix}_log.txt");
    std::fs::write(&path, colony.log.join("\n") + "\n")?;
    written.push(path);
    let path = format!("{prefix}_decisions.txt");
    std::fs::write(&path, colony.decisions.join("\n") + "\n")?;
    written.push(path);
    let stuck: u32 = colony.settlers.iter().map(|s| s.stuck).sum();
    println!("Colony after 30 days: {} of 7 alive, {} times a settler found no way to a target, {} log lines", colony.alive(), stuck, colony.log.len());
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
    let (cx, cy) = center.unwrap_or_else(|| crate::region::zoom::pick_interesting_window(world, ZoomParams::default().tiles));
    let (w, h) = (1280usize, 800usize);
    let fit = (w as f32 / tw.width as f32).min(h as f32 / tw.height as f32);
    let shots = [
        ("overview", Camera { cx: tw.width as f32 / 2.0, cy: tw.height as f32 / 2.0, tile_px: fit }),
        // Just below the detailed-tile threshold (4 px): the far-zoom fill at its largest.
        ("3px", Camera { cx: cx as f32 + 0.5, cy: cy as f32 + 0.5, tile_px: 3.5 }),
        ("16px", Camera { cx: cx as f32 + 0.5, cy: cy as f32 + 0.5, tile_px: 16.0 }),
        ("32px", Camera { cx: cx as f32 + 0.5, cy: cy as f32 + 0.5, tile_px: 32.0 }),
    ];
    let mut written = Vec::new();
    let mut buf = vec![0u32; w * h];
    {
        // A plate of the 16 px view, as P makes it.
        let cam = Camera { cx: cx as f32 + 0.5, cy: cy as f32 + 0.5, tile_px: 16.0 };
        render_world(&tw, atlas, &cam, &mut buf, w, h);
        draw_labels(&labels, &cam, tw.width, &mut buf, w, h);
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
            best = best.min(t0.elapsed().as_secs_f32() * 1000.0);
        }
        println!("render {name}: {best:.1} ms");
        let t0 = std::time::Instant::now();
        draw_labels_avoiding(&labels, &cam, tw.width, &mut buf, w, h, &[minimap_box(tw.width, tw.height, w, h)]);
        println!("labels {name}: {:.1} ms", t0.elapsed().as_secs_f32() * 1000.0);
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
        }).map(|site| (site.x, site.y))
    });
    let (ex, ey) = target.unwrap_or_else(|| pick_embark_spot(&zs.region));
    let region = &zs.region;
    let t0 = std::time::Instant::now();
    let map = crate::local::generate_local(world, region, zs.lore.as_ref(), ex, ey);
    println!("Playable area: {}x{} tiles x {} z-levels, biome {:?}, generated in {:.2}s", map.width, map.height, map.depth, map.biome, t0.elapsed().as_secs_f32());
    let site = crate::local::site::report(&map);
    println!("Site ({} kinds): {}", site.len(), site.join(", "));
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
