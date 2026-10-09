//! The history watcher: a window that shows the world's history being written, in the style of
//! Dwarf Fortress's world generation screen but dressed as the ink map. The simulation runs on
//! a background thread and sends a frame per season; the window draws the map as it fills
//! (settlements, roads, borders, fields), marks what just happened on it (new roads glow,
//! foundings ring, battles and razings flare), and keeps a chronicle of the key events beside an
//! almanac of the age (year, peoples, souls, the great realms).
//!
//! Space pauses, `[` / `]` set the pace, `L` switches the chronicle between key events and
//! everything, the wheel zooms the map (or scrolls the chronicle), dragging pans, `H` fits the
//! map, clicking an entry flies to where it happened. Closing the window (or Esc) lets the
//! history finish unwatched; Enter at the end continues.

use std::collections::{HashMap, VecDeque};
use std::error::Error;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use minifb::{Key, KeyRepeat, MouseButton, MouseMode, Window, WindowOptions};

use crate::history::civilizations::settlement::SettlementType;
use crate::history::config::HistoryConfig;
use crate::history::data::GameData;
use crate::history::events::types::EventType;
use crate::history::simulation::HistoryEngine;
use crate::history::world_state::WorldHistory;
use crate::seasons::Season;
use crate::world::WorldData;

use super::atlas::Atlas;
use super::classify::{faction_color, HistoryOverlay, Site, TileWorld};
use super::render::{render_world_cached, render_world_lod, screen_to_world, snap_world_camera, Camera};
use super::fonts::{self, Face};
use super::text::{draw_ink, place_labels, text_width, Label};
use super::ui::*;

const ROAD_GLOW: u32 = 0x00F0_B040;

const PANEL_W: usize = 340;
const MARGIN: usize = 12;
const LINE: i64 = 12;
/// Chronicle entries kept (newest first).
const LOG_CAP: usize = 3000;
/// Pace: minimum seconds per season, slowest first; the last is as fast as the simulation goes.
const PACES: [(f32, &str); 5] = [(1.0, "a season a breath"), (0.4, "unhurried"), (0.15, "brisk"), (0.05, "swift"), (0.0, "headlong")];
/// Swift: small worlds (whose seasons take a millisecond) stay watchable; big worlds take longer
/// than this per season anyway.
const DEFAULT_PACE: usize = 3;

/// Controls shared with the simulation thread. The simulation always runs at full speed and
/// records the history; `paused` and `pace` only govern playback.
struct Control {
    paused: AtomicBool,
    pace: AtomicUsize,
    /// The window is gone: finish as fast as possible, nobody is watching.
    detached: AtomicBool,
}

#[derive(Clone)]
struct LogItem {
    year: u32,
    kind: EventType,
    title: String,
    location: Option<(usize, usize)>,
    key: bool,
}

#[derive(Clone)]
struct Realm {
    id: u64,
    name: String,
    arms: super::heraldry::Arms,
    population: u64,
    towns: usize,
}

#[derive(Clone, Default)]
struct Stats {
    peoples: usize,
    fallen: usize,
    towns: usize,
    ruins: usize,
    souls: u64,
    roads: usize,
    wars: usize,
    beasts: usize,
    artifacts: usize,
    monuments: usize,
    shadow: Option<ShadowStats>,
}

#[derive(Clone, Default)]
struct ShadowStats {
    name: String,
    lord: String,
    faction: u64,
    towns: usize,
    fallen: usize,
    held: u32,
    /// Share of land in its reach, and blighted.
    reach: f32,
    blight: f32,
    broken: bool,
}

/// A full overlay is kept every this many seasons, so jumping to any season replays at most
/// this many deltas.
const KEYFRAME_EVERY: usize = 40;

/// What changed in the history overlay from one season to the next (tile index, new value).
#[derive(Default)]
struct Delta {
    owner: Vec<(u32, u64)>,
    road: Vec<(u32, bool)>,
    farmland: Vec<(u32, u8)>,
    cover: Vec<(u32, u8)>,
    shadow: Vec<(u32, u8)>,
    /// The full settlement list when it changed.
    sites: Option<Vec<Site>>,
    shadow_faction: u64,
    shadow_seat: Option<(usize, usize)>,
}

fn diff<T: Copy + PartialEq>(a: &[T], b: &[T]) -> Vec<(u32, T)> {
    if a.len() != b.len() {
        return b.iter().enumerate().map(|(i, &v)| (i as u32, v)).collect();
    }
    a.iter().zip(b).enumerate().filter(|(_, (x, y))| x != y).map(|(i, (_, &y))| (i as u32, y)).collect()
}

fn put<T: Copy>(v: &mut Vec<T>, d: &[(u32, T)], n: usize, zero: T) {
    if d.is_empty() { return; }
    if v.len() != n { v.resize(n, zero); }
    for &(i, x) in d { v[i as usize] = x; }
}

impl Delta {
    fn between(a: &HistoryOverlay, b: &HistoryOverlay) -> Delta {
        Delta {
            owner: diff(&a.owner, &b.owner),
            road: diff(&a.road, &b.road),
            farmland: diff(&a.farmland, &b.farmland),
            cover: diff(&a.cover, &b.cover),
            shadow: diff(&a.shadow, &b.shadow),
            sites: (a.sites != b.sites).then(|| b.sites.clone()),
            shadow_faction: b.shadow_faction,
            shadow_seat: b.shadow_seat,
        }
    }

    fn apply(&self, o: &mut HistoryOverlay, n: usize) {
        put(&mut o.owner, &self.owner, n, u64::MAX);
        put(&mut o.road, &self.road, n, false);
        put(&mut o.farmland, &self.farmland, n, 0);
        put(&mut o.cover, &self.cover, n, 255);
        put(&mut o.shadow, &self.shadow, n, 0);
        if let Some(sites) = &self.sites { o.sites = sites.clone(); }
        o.shadow_faction = self.shadow_faction;
        o.shadow_seat = self.shadow_seat;
    }
}

/// One season of the history, as recorded for playback.
struct Step {
    year: u32,
    season: Season,
    step: u32,
    total: u32,
    stats: Stats,
    realms: Vec<Realm>,
    /// Faction names by id (hover, chronicle).
    names: HashMap<u64, String>,
    /// Living settlements and ruins: id -> (name, kind, population, faction, ruined).
    places: HashMap<u64, (String, SettlementType, u32, u64, bool)>,
    events: Vec<LogItem>,
    new_roads: Vec<(usize, usize)>,
    delta: Delta,
}

enum Msg {
    Status(String),
    /// The overlay at the dawn of history (before the first recorded step).
    Start(Box<HistoryOverlay>),
    Step(Box<Step>),
    /// The world as the game inherits it: lines for the closing card.
    Present(Vec<String>),
    /// Three places to settle, offered on the closing card.
    Sites(Vec<super::viewer::SiteOffer>),
    Done,
}

/// How an event is shown: chronicle glyph and colour, the mark it leaves on the map, and
/// whether it counts as a key event.
#[derive(Clone, Copy, PartialEq)]
enum MarkKind { Road, Founded, Battle, Razed, Disaster, Beast, Wonder, Faith }

fn style(kind: &EventType) -> (char, u32, Option<MarkKind>, bool) {
    use EventType::*;
    match kind {
        FactionFounded => ('*', GOLD, Some(MarkKind::Founded), true),
        FactionDestroyed => ('X', RUBRIC, Some(MarkKind::Razed), true),
        SettlementFounded => ('o', INK, Some(MarkKind::Founded), false),
        SettlementDestroyed => ('#', RUBRIC, Some(MarkKind::Razed), true),
        SettlementGrew => ('o', INK_FADED, None, false),
        WarDeclared => ('!', RUBRIC, Some(MarkKind::Battle), true),
        WarEnded => ('=', INK, None, true),
        BattleFought | SiegeBegun | SiegeEnded | Raid => ('x', RUBRIC, Some(MarkKind::Battle), false),
        Massacre => ('x', RUBRIC, Some(MarkKind::Razed), true),
        AllianceFormed => ('&', SEA, None, true),
        TreatySigned | TreatyBroken | AllianceBroken | TradeRouteEstablished => ('&', SEA, None, false),
        RulerCrowned => ('^', GOLD, None, false),
        RulerDeposed | SuccessionCrisis | Rebellion | Coup | Assassination => ('^', RUBRIC, None, true),
        ReligionFounded | HolyWarDeclared => ('+', VIOLET, Some(MarkKind::Faith), true),
        Miracle | TempleBuilt | TempleProfaned | CultFormed => ('+', VIOLET, Some(MarkKind::Faith), false),
        CreatureSlain | LairDestroyed => ('~', MOSS, Some(MarkKind::Beast), true),
        CreatureAppeared | MonsterRaid | LairEstablished | PopulationMigrated => ('~', MOSS, Some(MarkKind::Beast), false),
        HeroBorn | HeroDied | QuestBegun | QuestCompleted => ('-', INK_FADED, None, false),
        MasterworkCreated | ArtifactCreated | ArtifactLost | ArtifactFound | ArtifactDestroyed => ('$', GOLD, None, false),
        MonumentBuilt => ('$', GOLD, Some(MarkKind::Wonder), true),
        MonumentDestroyed => ('$', RUBRIC, Some(MarkKind::Razed), false),
        VolcanoErupted | Earthquake | Flood | Drought | Plague | MagicalCatastrophe => ('!', VIOLET, Some(MarkKind::Disaster), true),
        SpellInvented | MagicalExperiment | CurseApplied | CurseLifted => ('%', VIOLET, None, false),
        ForestCleared | GameScarce | WildlifeReturned => ('"', MOSS, None, false),
        LandScarred => ('@', RUBRIC, Some(MarkKind::Disaster), true),
        Authored => ('@', GOLD, None, true),
        Other => ('-', INK_FADED, None, false),
        ShadowRose => ('@', RUBRIC, Some(MarkKind::Disaster), true),
        ShadowConquest => ('X', RUBRIC, Some(MarkKind::Razed), true),
        ShadowRepelled => ('=', GOLD, Some(MarkKind::Battle), true),
        ShadowBroken => ('*', GOLD, Some(MarkKind::Wonder), true),
        ShadowLiberated => ('*', GOLD, Some(MarkKind::Battle), true),
        FigureMoved => ('>', INK_FADED, None, false),
        ShadowAlliance => ('*', GOLD, Some(MarkKind::Battle), true),
        ShadowBane => ('$', GOLD, None, true),
        Marriage => ('&', GOLD, None, false),
        AdventurerDeed => ('*', GOLD, None, true),
    }
}

/// A mark on the map for something that just happened.
struct Mark {
    x: usize,
    y: usize,
    kind: MarkKind,
    born: Instant,
}

impl MarkKind {
    fn life(self) -> f32 {
        match self {
            MarkKind::Road => 2.5,
            MarkKind::Founded | MarkKind::Faith | MarkKind::Beast => 3.0,
            MarkKind::Battle => 3.5,
            MarkKind::Razed | MarkKind::Disaster | MarkKind::Wonder => 5.0,
        }
    }
}

/// Simulate the history in a window. Falls back to an unwatched simulation if no window can
/// be opened.
pub fn watch_history(world: &WorldData, game_data: &GameData, config: HistoryConfig, mut engine: HistoryEngine, atlas: &Atlas) -> WorldHistory {
    let window = Window::new("The World Takes Shape", 1440, 900, WindowOptions { resize: true, ..WindowOptions::default() });
    let window = match window {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Could not open the watcher window ({e}); simulating unwatched");
            return engine.simulate_with_data(world, config, game_data);
        }
    };
    let mut base = TileWorld::build(world, atlas);
    base.set_season(world, Season::Summer);
    let ctl = Control { paused: AtomicBool::new(false), pace: AtomicUsize::new(DEFAULT_PACE), detached: AtomicBool::new(false) };
    let (tx, rx) = mpsc::channel();
    std::thread::scope(|scope| {
        let sim = {
            let ctl = &ctl;
            scope.spawn(move || simulate(world, game_data, config, engine, ctl, tx))
        };
        if let Err(e) = run_window(window, world, atlas, base, &ctl, rx) {
            eprintln!("Watcher error: {e}");
        }
        if !sim.is_finished() {
            eprintln!("Watcher closed; finishing the history unwatched...");
        }
        ctl.detached.store(true, Ordering::Relaxed);
        ctl.paused.store(false, Ordering::Relaxed);
        sim.join().expect("history simulation panicked")
    })
}

fn simulate(
    world: &WorldData,
    game_data: &GameData,
    config: HistoryConfig,
    mut engine: HistoryEngine,
    ctl: &Control,
    tx: mpsc::Sender<Msg>,
) -> WorldHistory {
    let total = config.total_steps();
    let civs = config.initial_civilizations;
    let _ = tx.send(Msg::Status(format!("Recalling the ages before memory: {civs} peoples, their kings and their gods...")));
    let mut history = engine.begin(world, config, game_data);
    let (w, h) = (world.width, world.height);
    let mut prev = HistoryOverlay::from_history(&history, w, h);
    let mut seen = history.chronicle.len();
    let mut treasures = 0usize;
    let dawn = LogItem {
        year: history.current_date.year,
        kind: EventType::Other,
        title: format!(
            "Written history begins. {} peoples hold {} settlements; {} legendary beasts walk the wilds.",
            history.active_faction_count(),
            history.settlements.values().filter(|s| !s.is_destroyed()).count(),
            history.living_legendary_count()
        ),
        location: None,
        key: true,
    };
    let _ = tx.send(Msg::Start(Box::new(prev.clone())));
    let mut first = record(&history, world, &prev, 0, total, Vec::new(), Vec::new(), Delta::default());
    first.events.push(dawn);
    let _ = tx.send(Msg::Step(Box::new(first)));

    for step in 0..total {
        engine.step(&mut history, world, game_data);
        if ctl.detached.load(Ordering::Relaxed) {
            if step % 40 == 0 { eprintln!("  History: year {}", history.current_date.year); }
            continue;
        }
        let treasure = |k: &EventType| matches!(k, EventType::ArtifactFound | EventType::ArtifactLost | EventType::ArtifactCreated | EventType::ArtifactDestroyed | EventType::MasterworkCreated);
        let mut events: Vec<LogItem> = history.chronicle.events[seen..].iter().map(|e| LogItem {
            year: e.date.year,
            kind: e.event_type.clone(),
            title: e.title.clone(),
            location: e.location,
            // Treasures made, found and lost are told once a decade, not one by one.
            key: (style(&e.event_type).3 || e.is_major) && !treasure(&e.event_type),
        }).collect();
        treasures += events.iter().filter(|e| treasure(&e.kind)).count();
        if history.current_date.season == crate::seasons::Season::Winter && history.current_date.year % 10 == 0 && treasures > 0 {
            events.push(LogItem {
                year: history.current_date.year, kind: EventType::ArtifactFound,
                title: format!("In ten years {} treasure{} {} made, found or lost", treasures, if treasures == 1 { "" } else { "s" }, if treasures == 1 { "was" } else { "were" }),
                location: None, key: true,
            });
            treasures = 0;
        }
        seen = history.chronicle.len();
        let next = HistoryOverlay::from_history(&history, w, h);
        let new_roads = (0..w * h).filter(|&i| next.road[i] && !prev.road.get(i).copied().unwrap_or(false)).map(|i| (i % w, i / w)).collect();
        let delta = Delta::between(&prev, &next);
        let rec = record(&history, world, &next, step + 1, total, events, new_roads, delta);
        prev = next;
        let _ = tx.send(Msg::Step(Box::new(rec)));
    }
    let _ = tx.send(Msg::Status("Naming the ages...".into()));
    engine.finish(&mut history);
    // The world today, for the closing card.
    let present = history.present();
    let gaz = crate::lore::build_gazetteer(world, Some(&history), world.seed());
    let name = gaz.features.iter().filter(|f| f.kind == crate::lore::FeatureKind::Continent).max_by_key(|f| f.size).map(|f| f.name.clone()).unwrap_or_else(|| "the world".into());
    let mut lines = vec![crate::lore::claims::sentence(&name, &crate::lore::claims::claims(&history))];
    let n = |k: usize, one: &str, many: &str| format!("{} {}", k, if k == 1 { one } else { many });
    lines.push(format!("{} remain; {} fought and {} held.", n(present.peoples, "people", "peoples"), n(present.wars.len(), "war is", "wars are"), n(present.grudges.len(), "grudge", "grudges")));
    if !present.frontier.is_empty() {
        lines.push(format!("The Shadow presses on {} towns{}.", present.frontier.len(),
            present.stronghold.as_ref().map(|s| format!("; in its path stands {}", s.split(" (").next().unwrap_or(s))).unwrap_or_default()));
    }
    if let Some(w) = present.weaknesses.first() { lines.push(format!("What can wound it: {}.", w.split(" (since").next().unwrap_or(w))); }
    let near = present.beasts.iter().filter(|b| b.town.is_some()).count();
    if near > 0 { lines.push(format!("{} near towns.", n(near, "beast of legend lairs", "beasts of legend lair"))); }
    if !present.fallen.is_empty() { lines.push(format!("{} towns fell in the last {} years; their survivors are on the roads.", present.fallen.len(), crate::history::present::RECENT_YEARS)); }
    let _ = tx.send(Msg::Present(lines));
    let _ = tx.send(Msg::Status("Choosing three places to settle...".into()));
    let _ = tx.send(Msg::Sites(super::viewer::three_sites(world, &history)));
    let _ = tx.send(Msg::Done);
    history
}

/// Record one season: the almanac, the realms, names and places for hover and labels, and the
/// season's events and overlay changes.
#[allow(clippy::too_many_arguments)]
fn record(
    history: &WorldHistory,
    world: &WorldData,
    overlay: &HistoryOverlay,
    step: u32,
    total: u32,
    events: Vec<LogItem>,
    new_roads: Vec<(usize, usize)>,
    delta: Delta,
) -> Step {
    let mut realms: HashMap<u64, Realm> = HashMap::new();
    let mut places = HashMap::new();
    let mut stats = Stats::default();
    let names: HashMap<u64, String> = history.factions.iter().map(|(id, f)| (id.0, f.name.clone())).collect();
    for s in history.settlements.values() {
        let ruined = s.is_destroyed();
        places.insert(s.id.0, (s.name.clone(), s.settlement_type, s.population, s.faction.0, ruined));
        if ruined {
            stats.ruins += 1;
            continue;
        }
        stats.towns += 1;
        stats.souls += s.population as u64;
        if history.factions.get(&s.faction).map(|f| f.is_active()).unwrap_or(false) {
            let r = realms.entry(s.faction.0).or_insert_with(|| Realm {
                id: s.faction.0,
                name: names.get(&s.faction.0).cloned().unwrap_or_default(),
                arms: super::heraldry::arms_of(world, history, s.faction),
                population: 0,
                towns: 0,
            });
            r.population += s.population as u64;
            r.towns += 1;
        }
    }
    stats.peoples = history.active_faction_count();
    stats.fallen = history.factions.len() - stats.peoples;
    stats.roads = overlay.road.iter().filter(|&&r| r).count();
    stats.wars = history.wars.values().filter(|w| w.is_active()).count();
    stats.beasts = history.living_legendary_count();
    stats.artifacts = history.artifacts.len();
    stats.monuments = history.monuments.len();
    stats.shadow = history.shadow.as_ref().map(|sh| {
        let land = (0..world.width * world.height).filter(|&i| *world.heightmap.get(i % world.width, i / world.width) >= 0.0).count().max(1) as f32;
        let (reach, blight) = sh.extent();
        ShadowStats {
            name: sh.name.clone(),
            lord: sh.lord(history),
            faction: sh.faction.0,
            towns: history.factions.get(&sh.faction).map_or(0, |f| f.settlements.len()),
            fallen: sh.fallen.len(),
            held: sh.repelled,
            reach: reach as f32 / land,
            blight: blight as f32 / land,
            broken: sh.is_broken(),
        }
    });
    let mut realms: Vec<Realm> = realms.into_values().collect();
    realms.sort_by_key(|r| std::cmp::Reverse(r.population));
    Step {
        year: history.current_date.year,
        season: history.current_date.season,
        step,
        total,
        stats,
        realms,
        names,
        places,
        events,
        new_roads,
        delta,
    }
}

// ---------------------------------------------------------------------------------------------
// Drawing helpers
// ---------------------------------------------------------------------------------------------













fn short_num(n: u64) -> String {
    if n >= 1_000_000 { format!("{:.1}M", n as f64 / 1e6) } else if n >= 10_000 { format!("{}k", n / 1000) } else if n >= 1000 { format!("{:.1}k", n as f64 / 1e3) } else { n.to_string() }
}

fn season_name(s: Season) -> &'static str {
    match s { Season::Spring => "Spring", Season::Summer => "Summer", Season::Autumn => "Autumn", Season::Winter => "Winter" }
}

/// Screen position of a world tile's centre in the map view (wrapping at the date line).
fn tile_to_screen(cam: &Camera, world_w: usize, map: Rect, x: usize, y: usize) -> (f32, f32) {
    let ww = world_w as f32;
    let mut dx = x as f32 + 0.5 - cam.cx;
    if dx > ww / 2.0 { dx -= ww; }
    if dx < -ww / 2.0 { dx += ww; }
    (map.x as f32 + map.w as f32 / 2.0 + dx * cam.tile_px, map.y as f32 + map.h as f32 / 2.0 + (y as f32 + 0.5 - cam.cy) * cam.tile_px)
}

fn ring(buf: &mut [u32], w: usize, h: usize, clip: Rect, cx: f32, cy: f32, r: f32, thick: f32, color: u32, a: f32) {
    let ri = (r + thick + 1.0).ceil() as i64;
    for dy in -ri..=ri {
        for dx in -ri..=ri {
            let (x, y) = (cx as i64 + dx, cy as i64 + dy);
            if !clip.contains(x as f32, y as f32) { continue; }
            let d = ((dx * dx + dy * dy) as f32).sqrt();
            let cover = (thick / 2.0 + 0.5 - (d - r).abs()).clamp(0.0, 1.0);
            if cover > 0.0 { blend_px(buf, w, h, x, y, color, a * cover); }
        }
    }
}

fn dot(buf: &mut [u32], w: usize, h: usize, clip: Rect, cx: f32, cy: f32, r: f32, color: u32, a: f32) {
    let ri = (r + 1.0).ceil() as i64;
    for dy in -ri..=ri {
        for dx in -ri..=ri {
            let (x, y) = (cx as i64 + dx, cy as i64 + dy);
            if !clip.contains(x as f32, y as f32) { continue; }
            let d = ((dx * dx + dy * dy) as f32).sqrt();
            let cover = (r + 0.5 - d).clamp(0.0, 1.0);
            if cover > 0.0 { blend_px(buf, w, h, x, y, color, a * cover); }
        }
    }
}

fn cross(buf: &mut [u32], w: usize, h: usize, clip: Rect, cx: f32, cy: f32, r: f32, color: u32, a: f32) {
    let ri = r.ceil() as i64;
    for k in -ri..=ri {
        for t in 0..2 {
            for (x, y) in [(cx as i64 + k + t, cy as i64 + k), (cx as i64 + k + t, cy as i64 - k)] {
                if clip.contains(x as f32, y as f32) { blend_px(buf, w, h, x, y, color, a); }
            }
        }
    }
}

fn draw_mark(buf: &mut [u32], w: usize, h: usize, clip: Rect, m: &Mark, sx: f32, sy: f32, tile_px: f32, now: Instant) {
    let age = now.duration_since(m.born).as_secs_f32();
    let life = m.kind.life();
    let t = age / life;
    let fade = (1.0 - t).clamp(0.0, 1.0);
    let scale = tile_px.clamp(2.0, 5.0) / 3.0;
    match m.kind {
        MarkKind::Road => {
            let r = (tile_px * 0.25).clamp(1.6, 2.5);
            dot(buf, w, h, clip, sx, sy, r + 1.5 * fade, ROAD_GLOW, 0.85 * fade);
        }
        MarkKind::Founded => {
            dot(buf, w, h, clip, sx, sy, 1.8 * scale, GOLD, fade);
            ring(buf, w, h, clip, sx, sy, (2.0 + 9.0 * t) * scale, 1.2, GOLD, fade);
        }
        MarkKind::Faith => ring(buf, w, h, clip, sx, sy, (2.0 + 7.0 * t) * scale, 1.2, VIOLET, fade),
        MarkKind::Beast => ring(buf, w, h, clip, sx, sy, (3.0 + 5.0 * t) * scale, 1.0, MOSS, fade),
        MarkKind::Battle => {
            cross(buf, w, h, clip, sx, sy, 2.5 * scale, RUBRIC, fade);
            ring(buf, w, h, clip, sx, sy, (3.0 + 4.0 * t) * scale, 1.0, RUBRIC, 0.6 * fade);
        }
        MarkKind::Razed => {
            let flicker = 0.75 + 0.25 * (age * 14.0).sin();
            dot(buf, w, h, clip, sx, sy, 3.0 * scale, 0x00D0_5020, 0.7 * fade * flicker);
            cross(buf, w, h, clip, sx, sy, 3.0 * scale, RUBRIC, fade);
            ring(buf, w, h, clip, sx, sy, (4.0 + 10.0 * t) * scale, 1.5, RUBRIC, fade);
        }
        MarkKind::Disaster => {
            ring(buf, w, h, clip, sx, sy, (4.0 + 14.0 * t) * scale, 2.0, VIOLET, 0.8 * fade);
            ring(buf, w, h, clip, sx, sy, (2.0 + 8.0 * t) * scale, 1.0, VIOLET, 0.6 * fade);
        }
        MarkKind::Wonder => {
            let r = 3.0 * scale;
            for k in 0..=(r as i64) {
                let span = r as i64 - k;
                for s in [-1i64, 1] {
                    blend_px(buf, w, h, sx as i64 + span * s, sy as i64 + k, GOLD, fade);
                    blend_px(buf, w, h, sx as i64 + span * s, sy as i64 - k, GOLD, fade);
                }
            }
            ring(buf, w, h, clip, sx, sy, (4.0 + 6.0 * t) * scale, 1.0, GOLD, 0.6 * fade);
        }
    }
}

/// The parchment banner across the top of the map for a great event.
fn banner(buf: &mut [u32], w: usize, h: usize, map: Rect, text: &str, alpha: f32) {
    let lines = wrap(text, (map.w.saturating_sub(80)) / 14);
    let scale = if lines.len() <= 2 { 2 } else { 1 };
    let lines = if scale == 2 { lines } else { wrap(text, (map.w.saturating_sub(80)) / 7) };
    let tw = lines.iter().map(|l| fell_width(l, scale)).max().unwrap_or(0);
    let bh = lines.len() * if scale == 2 { 24 } else { 16 } + 18;
    let bw = tw + 40;
    let bx = map.x + map.w.saturating_sub(bw) / 2;
    let by = map.y + 14;
    for y in by..(by + bh).min(h) {
        for x in bx..(bx + bw).min(w) {
            let edge = (x == bx || x == bx + bw - 1 || y == by || y == by + bh - 1) as u32;
            let c = if edge == 1 { INK } else { mix(PAPER, PAPER_SHADE, (hash(x / 3, y / 3) & 0xFF) as f32 / 600.0) };
            let k = y * w + x;
            buf[k] = mix(buf[k], c, alpha * 0.95);
        }
    }
    // Ribbon tails.
    for k in 0..8usize {
        for y in by + 4 + k / 2..by + bh - 4 - k / 2 {
            blend_px(buf, w, h, bx as i64 - 1 - k as i64, y as i64, PAPER_SHADE, alpha * 0.9);
            blend_px(buf, w, h, (bx + bw + k) as i64, y as i64, PAPER_SHADE, alpha * 0.9);
        }
    }
    if alpha > 0.5 {
        for (i, l) in lines.iter().enumerate() {
            let lx = bx + (bw - fell_width(l, scale)) / 2;
            fell(buf, w, h, lx as i64, (by + 10 + i * if scale == 2 { 24 } else { 16 }) as i64, l, RUBRIC, scale, false);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The window
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Layout { map: Rect, log: Rect, panel: Rect }

fn layout(w: usize, h: usize, world: &WorldData) -> Layout {
    let panel = Rect { x: w.saturating_sub(PANEL_W + MARGIN), y: MARGIN, w: PANEL_W, h: h.saturating_sub(2 * MARGIN) };
    let mw = w.saturating_sub(PANEL_W + 3 * MARGIN).max(64);
    let natural = mw * world.height / world.width;
    let mh = natural.min(h * 2 / 3).max(64).min(h.saturating_sub(2 * MARGIN + 120));
    let map = Rect { x: MARGIN, y: MARGIN, w: mw, h: mh };
    let ly = map.y + mh + MARGIN;
    let log = Rect { x: MARGIN, y: ly, w: mw, h: h.saturating_sub(ly + MARGIN).max(40) };
    Layout { map, log, panel }
}

fn fit_camera(world: &WorldData, map: Rect) -> Camera {
    let tile_px = (map.w as f32 / world.width as f32).min(map.h as f32 / world.height as f32 * 1.6).max(0.5);
    Camera { cx: world.width as f32 / 2.0, cy: world.height as f32 / 2.0, tile_px }
}

/// Everything the watcher shows, apart from the window. The simulation's recording arrives
/// season by season (`receive`); the view plays it back at the chosen pace (`play`), can jump
/// to any recorded season (`jump`), and draws into a pixel buffer. The window (or a headless
/// snapshot) drives it.
struct View<'a> {
    world: &'a WorldData,
    atlas: &'a Atlas,
    /// The land with no history on it; each shown season is drawn as `base` + its overlay.
    base: TileWorld,
    tw: TileWorld,
    steps: Vec<Box<Step>>,
    keyframes: Vec<(usize, HistoryOverlay)>,
    /// The overlay at the newest recorded season, and at the shown one.
    head: HistoryOverlay,
    cur: HistoryOverlay,
    shown: Option<usize>,
    next_advance: Instant,
    tw_dirty: bool,
    log: VecDeque<LogItem>,
    marks: Vec<Mark>,
    banner_msg: Option<(String, Instant)>,
    /// The closing card: the world today.
    present: Vec<String>,
    sites: Vec<super::viewer::SiteOffer>,
    /// Where each offered site is drawn (for clicks).
    site_rects: Vec<Rect>,
    status: String,
    /// The simulation has finished (everything is recorded).
    done: bool,
    show_all: bool,
    log_scroll: usize,
    size: (usize, usize),
    buf: Vec<u32>,
    bg: Vec<u32>,
    map_buf: Vec<u32>,
    map_dirty: bool,
    lay: Layout,
    cam: Camera,
    fitted: bool,
    last_render: Instant,
    entry_hits: Vec<(Rect, (usize, usize))>,
    /// The timeline bar in the panel (click or drag it to jump).
    timeline: Rect,
    /// Smooth zoom: the zoom the wheel asked for, eased towards each frame around `zoom_at`.
    zoom_target: f32,
    /// Album recording (A): a plate is saved for each great event as it plays.
    album: Option<(String, usize)>,
    /// What the album has already shown (titles) and the year of its last plate.
    album_seen: Vec<String>,
    album_last: u32,
    /// The largest continent's name, for plates ("the Lands of ...").
    world_name: String,
    zoom_at: (f32, f32),
    /// Last time the view moved (pan, zoom, drag); the map is drawn at full quality again once
    /// it has been still for a moment.
    moved: Instant,
    /// The map on screen is a moving-view preview, and how long a full-quality render takes.
    preview: bool,
    full_ms: f32,
    last_frame: Instant,
}

impl<'a> View<'a> {
    fn new(world: &'a WorldData, atlas: &'a Atlas, base: TileWorld) -> Self {
        let lay = layout(1440, 900, world);
        let cam = fit_camera(world, lay.map);
        View {
            world, atlas, tw: base.clone(), base, steps: Vec::new(), keyframes: Vec::new(),
            head: HistoryOverlay::default(), cur: HistoryOverlay::default(), shown: None,
            next_advance: Instant::now(), tw_dirty: false,
            log: VecDeque::new(), marks: Vec::new(),
            banner_msg: None, present: Vec::new(), sites: Vec::new(), site_rects: Vec::new(), status: "Raising the land...".into(),
            done: false, show_all: false, log_scroll: 0, size: (0, 0), buf: Vec::new(),
            bg: Vec::new(), map_buf: Vec::new(), map_dirty: true, lay, cam, fitted: true,
            last_render: Instant::now() - Duration::from_secs(1), entry_hits: Vec::new(),
            timeline: Rect::default(),
            zoom_target: cam.tile_px, album: None, album_seen: Vec::new(), album_last: 0,
            world_name: crate::lore::gazetteer::build_gazetteer(world, None, world.seed()).features.iter().filter(|f| f.kind == crate::lore::FeatureKind::Continent).max_by_key(|f| f.size).map(|f| f.name.clone()).unwrap_or_else(|| "the World".into()), zoom_at: (0.0, 0.0), moved: Instant::now() - Duration::from_secs(1),
            preview: false, full_ms: 0.0, last_frame: Instant::now(),
        }
    }

    fn tiles(&self) -> usize { self.world.width * self.world.height }

    /// Take in the simulation's recording.
    fn receive(&mut self, msg: Msg) {
        match msg {
            Msg::Status(s) => self.status = s,
            Msg::Present(lines) => self.present = lines,
            Msg::Sites(offers) => self.sites = offers,
            Msg::Done => self.done = true,
            Msg::Start(o) => self.head = *o,
            Msg::Step(step) => {
                let n = self.tiles();
                if !self.steps.is_empty() { step.delta.apply(&mut self.head, n); }
                self.steps.push(step);
                let k = self.steps.len() - 1;
                if k % KEYFRAME_EVERY == 0 { self.keyframes.push((k, self.head.clone())); }
                if self.shown.is_none() { self.jump(0); }
            }
        }
    }

    /// The season on show.
    fn current(&self) -> Option<&Step> {
        self.shown.map(|k| &*self.steps[k])
    }

    /// The last recorded season is on show and nothing more will come.
    fn complete(&self) -> bool {
        self.done && self.shown.map_or(false, |k| k + 1 == self.steps.len())
    }

    /// Show the next recorded season: apply its changes, mark its events on the map and add
    /// them to the chronicle.
    fn step_forward(&mut self) {
        let Some(k) = self.shown else { return };
        if k + 1 >= self.steps.len() { return; }
        let n = self.tiles();
        let k = k + 1;
        self.steps[k].delta.apply(&mut self.cur, n);
        self.shown = Some(k);
        let now = Instant::now();
        let step = &self.steps[k];
        for &(x, y) in &step.new_roads {
            self.marks.push(Mark { x, y, kind: MarkKind::Road, born: now });
        }
        for e in &step.events {
            if let (Some(kind), Some((x, y))) = (style(&e.kind).2, e.location) {
                self.marks.push(Mark { x, y, kind, born: now });
            }
            // Someone reading older entries keeps their place as new ones arrive.
            if self.log_scroll > 0 && (e.key || self.show_all) { self.log_scroll += 1; }
            let great = e.kind.is_major() || matches!(e.kind, EventType::Authored | EventType::SettlementDestroyed | EventType::LandScarred | EventType::ShadowRepelled);
            if great { self.banner_msg = Some((ascii(&e.title), now)); }
            // The album keeps one plate a title, and lesser great events five years apart.
            let worth = great && e.location.is_some() && !self.album_seen.contains(&e.title) && (e.kind.is_major() || e.year >= self.album_last + 5 || self.album_last == 0);
            if worth { if let Some((dir, n)) = self.album.clone() {
                let e = e.clone();
                let path = album_plate(self, k, &e, &dir, n);
                self.album = Some((dir, n + 1));
                self.album_seen.push(e.title.clone());
                self.album_last = e.year;
                self.status = path;
            } }
            self.log.push_front(e.clone());
        }
        self.log.truncate(LOG_CAP);
        // Too many glowing roads at once only blurs; keep the most recent.
        if self.marks.len() > 4000 { let n = self.marks.len() - 4000; self.marks.drain(..n); }
        self.tw_dirty = true;
    }

    /// Show recorded season `k` directly: rebuild its overlay from the nearest keyframe and its
    /// chronicle from the seasons before it.
    fn jump(&mut self, k: usize) {
        if self.steps.is_empty() { return; }
        let k = k.min(self.steps.len() - 1);
        let n = self.tiles();
        let (kf, overlay) = self.keyframes.iter().rev().find(|(i, _)| *i <= k).cloned().unwrap_or((0, self.head.clone()));
        self.cur = overlay;
        for i in kf + 1..=k { self.steps[i].delta.apply(&mut self.cur, n); }
        self.shown = Some(k);
        self.log.clear();
        for step in &self.steps[..=k] {
            for e in &step.events { self.log.push_front(e.clone()); }
        }
        self.log.truncate(LOG_CAP);
        self.log_scroll = 0;
        self.marks.clear();
        self.banner_msg = None;
        if k == 0 {
            // The peoples of the dawn appear all at once.
            let now = Instant::now();
            for site in &self.cur.sites {
                if site.kind != crate::tiles::TileKind::Ruins { self.marks.push(Mark { x: site.x, y: site.y, kind: MarkKind::Founded, born: now }); }
            }
        }
        self.tw_dirty = true;
    }

    /// Advance playback by the pace (several seasons per frame when headlong).
    fn play(&mut self, ctl: &Control) {
        if ctl.paused.load(Ordering::Relaxed) || self.shown.is_none() { return; }
        let pace = PACES[ctl.pace.load(Ordering::Relaxed).min(PACES.len() - 1)].0;
        let now = Instant::now();
        if now < self.next_advance { return; }
        let n = if pace == 0.0 { 6 } else { 1 };
        for _ in 0..n { self.step_forward(); }
        // Paced by drama: quiet seasons hurry by, falls and razings linger.
        let drama = self.current().map_or(1.0, |st| {
            let great = st.events.iter().any(|e| e.kind.is_major() || matches!(e.kind, EventType::SettlementDestroyed | EventType::ShadowConquest | EventType::ShadowAlliance));
            if great { 2.5 } else if st.events.iter().any(|e| e.key) { 1.0 } else { 0.35 }
        });
        self.next_advance = now + Duration::from_secs_f32(pace * drama);
        if self.complete() && self.banner_msg.as_ref().map_or(true, |b| !b.0.starts_with("The age is written")) {
            self.banner_msg = Some(("The age is written. Press Enter to choose where to settle.".into(), now));
        }
    }

    fn resize(&mut self, w: usize, h: usize) {
        if (w, h) == self.size || w < 200 || h < 200 { return; }
        self.size = (w, h);
        self.lay = layout(w, h, self.world);
        if self.fitted { self.cam = fit_camera(self.world, self.lay.map); self.zoom_target = self.cam.tile_px; }
        let lay = &self.lay;
        self.buf = vec![0; w * h];
        self.map_buf = vec![0; lay.map.w * lay.map.h];
        self.bg = (0..w * h).map(|i| {
            let grain = ((hash(i % w / 2, i / w / 9) & 0x1F) as f32 / 31.0) * 0.08;
            mix(DESK, 0x0040_3428, grain)
        }).collect();
        card(&mut self.bg, w, lay.panel);
        card(&mut self.bg, w, lay.log);
        outline(&mut self.bg, w, Rect { x: lay.map.x - 4, y: lay.map.y - 4, w: lay.map.w + 8, h: lay.map.h + 8 }, PAPER_SHADE);
        outline(&mut self.bg, w, Rect { x: lay.map.x - 2, y: lay.map.y - 2, w: lay.map.w + 4, h: lay.map.h + 4 }, INK);
        self.map_dirty = true;
    }

    /// Ask to zoom the map by `steps` around a screen point; `animate` eases towards it.
    fn zoom(&mut self, steps: f32, at: (f32, f32)) {
        self.zoom_target = (self.zoom_target * 1.25f32.powf(steps)).clamp(1.0, 32.0);
        self.zoom_at = at;
        self.fitted = false;
    }

    /// Set the zoom directly, keeping the world point under `at` in place.
    fn set_zoom(&mut self, tile_px: f32, at: (f32, f32)) {
        let map = self.lay.map;
        let (mx, my) = (at.0 - map.x as f32, at.1 - map.y as f32);
        let before = screen_to_world(&self.cam, mx, my, map.w, map.h);
        self.cam.tile_px = tile_px;
        let after = screen_to_world(&self.cam, mx, my, map.w, map.h);
        self.cam.cx += before.0 - after.0;
        self.cam.cy = (self.cam.cy + before.1 - after.1).clamp(0.0, self.world.height as f32);
        self.moved();
    }

    /// Pan by screen pixels.
    fn pan(&mut self, dx: f32, dy: f32) {
        self.cam.cx += dx / self.cam.tile_px;
        self.cam.cy = self.clamp_cy(self.cam.cy + dy / self.cam.tile_px, self.cam.tile_px);
        self.fitted = false;
        self.moved();
    }

    fn moved(&mut self) {
        self.moved = Instant::now();
        self.map_dirty = true;
    }

    /// Per-frame motion: ease the zoom towards its target (a fixed share of the gap per
    /// second, so it feels the same at any frame rate).
    fn animate(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;
        let ratio = self.zoom_target / self.cam.tile_px;
        if (ratio - 1.0).abs() > 0.002 {
            let k = 1.0 - (-dt * 14.0).exp();
            let next = if (ratio - 1.0).abs() < 0.01 { self.zoom_target } else { self.cam.tile_px * ratio.powf(k) };
            self.set_zoom(next, self.zoom_at);
        }
    }

    /// Keep the view from running past the poles: when the map is taller than the view, its
    /// centre stays half a view from the top and bottom rows.
    fn clamp_cy(&self, cy: f32, tile_px: f32) -> f32 {
        let half = self.lay.map.h as f32 / 2.0 / tile_px;
        let hgt = self.world.height as f32;
        if half * 2.0 >= hgt { hgt / 2.0 } else { cy.clamp(half, hgt - half) }
    }

    fn look_at(&mut self, x: usize, y: usize, tile_px: f32) {
        let cy = self.clamp_cy(y as f32 + 0.5, tile_px);
        self.cam = Camera { cx: x as f32 + 0.5, cy, tile_px };
        self.zoom_target = tile_px;
        self.fitted = false;
        self.map_dirty = true;
    }

    fn fit(&mut self) {
        self.cam = fit_camera(self.world, self.lay.map);
        self.zoom_target = self.cam.tile_px;
        self.fitted = true;
        self.map_dirty = true;
    }

    /// Draw the whole window. While the view moves the map is re-rendered every frame, as a
    /// half-resolution preview when a full render would not keep up; it sharpens once the view
    /// is still. A changing world re-renders it at most ~30 times a second.
    fn draw(&mut self, mouse: (f32, f32), hovering: bool, ctl: &Control) {
        let (w, h) = self.size;
        let world = self.world;
        self.cam.cx = self.cam.cx.rem_euclid(world.width as f32);
        let lay = self.lay;
        let now = Instant::now();
        let mut tm = [0.0f64; 5];
        if self.tw_dirty {
            let mut tw = self.base.clone();
            tw.apply_overlay(world, &self.cur, self.atlas);
            // Seasons show only when playing slowly enough to read them (otherwise they strobe).
            let slow = ctl.pace.load(Ordering::Relaxed) <= 1 || ctl.paused.load(Ordering::Relaxed);
            let season = self.current().filter(|_| slow).map_or(Season::Summer, |s| s.season);
            if season != Season::Summer { tw.set_season(world, season); }
            self.tw = tw;
            self.tw_dirty = false;
            self.map_dirty = true;
        }
        tm[0] = now.elapsed().as_secs_f64() * 1e3;
        let moving = now.duration_since(self.moved) < Duration::from_millis(120);
        if self.preview && !moving { self.map_dirty = true; }
        if self.map_dirty && (moving || now.duration_since(self.last_render) > Duration::from_millis(30)) {
            let t0 = Instant::now();
            // (Kept between frames: a pan draws only what comes into view, a season of history
            // only the tiles it changed. Debug: PLANET_WATCH_NOCACHE draws it afresh as before.)
            let fresh = std::env::var("PLANET_WATCH_NOCACHE").is_ok();
            let lod = if fresh && moving && self.full_ms > 14.0 { 2 } else { 1 };
            let cam = if fresh { self.cam } else { snap_world_camera(&self.cam, lay.map.w, lay.map.h) };
            if fresh { render_world_lod(&self.tw, self.atlas, &cam, &mut self.map_buf, lay.map.w, lay.map.h, lod); }
            else { render_world_cached(&self.tw, self.atlas, &cam, &mut self.map_buf, lay.map.w, lay.map.h); }
            tm[1] = t0.elapsed().as_secs_f64() * 1e3;
            overlay_realms(&self.tw, &cam, &mut self.map_buf, lay.map.w, lay.map.h);
            tm[2] = t0.elapsed().as_secs_f64() * 1e3 - tm[1];
            if lod == 1 { self.full_ms = t0.elapsed().as_secs_f32() * 1e3; }
            self.preview = lod > 1;
            if let Some(step) = self.shown.map(|k| &*self.steps[k]).filter(|_| !self.preview) {
                let labels = settlement_labels(step, &self.tw);
                let ww = world.width as f32;
                let (mw, mh) = (lay.map.w, lay.map.h);
                place_labels(&labels, cam.tile_px, mw, mh, &mut self.map_buf, &[], |x, y| {
                    let mut dx = x - cam.cx;
                    if dx > ww / 2.0 { dx -= ww; }
                    if dx < -ww / 2.0 { dx += ww; }
                    (mw as f32 / 2.0 + dx * cam.tile_px, mh as f32 / 2.0 + (y - cam.cy) * cam.tile_px)
                });
            }
            tm[3] = t0.elapsed().as_secs_f64() * 1e3 - tm[1] - tm[2];
            self.map_dirty = false;
            self.last_render = now;
        }
        let trest = Instant::now();

        let complete = self.complete();
        let buf = &mut self.buf;
        buf.copy_from_slice(&self.bg);
        for y in 0..lay.map.h {
            let row = (lay.map.y + y) * w + lay.map.x;
            buf[row..row + lay.map.w].copy_from_slice(&self.map_buf[y * lay.map.w..(y + 1) * lay.map.w]);
        }

        // Marks of what just happened.
        self.marks.retain(|m| now.duration_since(m.born).as_secs_f32() < m.kind.life());
        for m in &self.marks {
            let (sx, sy) = tile_to_screen(&self.cam, world.width, lay.map, m.x, m.y);
            draw_mark(buf, w, h, lay.map, m, sx, sy, self.cam.tile_px, now);
        }

        // Hover chip: what is under the mouse.
        if hovering && lay.map.contains(mouse.0, mouse.1) {
            if let Some(step) = self.shown.map(|k| &*self.steps[k]) {
                let (wx, wy) = screen_to_world(&self.cam, mouse.0 - lay.map.x as f32, mouse.1 - lay.map.y as f32, lay.map.w, lay.map.h);
                if wy >= 0.0 && (wy as usize) < world.height {
                    let (tx, ty) = ((wx.floor() as i64).rem_euclid(world.width as i64) as usize, wy as usize);
                    let text = hover_text(step, &self.tw, tx, ty);
                    if !text.is_empty() {
                        let r = Rect { x: lay.map.x + 8, y: lay.map.y + lay.map.h - 24, w: (fell_width(&text, 1) + 12).min(lay.map.w - 16), h: 18 };
                        fill(buf, w, r, PAPER);
                        outline(buf, w, r, INK);
                        fell(buf, w, h, r.x as i64 + 6, r.y as i64 + 5, &truncate(&text, (r.w - 12) / 6), INK, 1, false);
                    }
                }
            }
        }

        // Banner for great events (the closing one stays).
        if let Some((text, born)) = &self.banner_msg {
            let age = now.saturating_duration_since(*born).as_secs_f32();
            let alpha = if complete { 1.0 } else { (1.0 - (age - 4.0) / 0.8).clamp(0.0, 1.0) };
            if alpha > 0.0 { banner(buf, w, h, lay.map, text, alpha); } else { self.banner_msg = None; }
        }

        let shown = self.shown.unwrap_or(0);
        let souls: Vec<u64> = self.steps[..(shown + 1).min(self.steps.len())].iter().map(|s| s.stats.souls).collect();
        let status = match self.shown {
            None => self.status.clone(),
            Some(k) if k + 1 < self.steps.len() => "Reading from the chronicle...".to_string(),
            Some(_) if self.done => "The age is written.".to_string(),
            Some(_) => "The scribes are still writing...".to_string(),
        };
        let current = self.shown.map(|k| &*self.steps[k]);
        let written = self.steps.len().saturating_sub(1) as u32;
        self.timeline = draw_panel(buf, w, h, lay.panel, current, &souls, &status, ctl, complete, written);
        self.entry_hits = draw_log(buf, w, h, lay.log, &self.log, self.show_all, &mut self.log_scroll, mouse);
        tm[4] = trest.elapsed().as_secs_f64() * 1e3;
        if std::env::var("PLANET_TIME_WATCH").is_ok() { eprintln!("WATCH tw {:.2} map {:.2} realms {:.2} labels {:.2} panel {:.2}", tm[0], tm[1], tm[2], tm[3], tm[4]); }

        // The world today: once the age is written, the present the game inherits, and three
        // places to settle. Lettered in IM Fell.
        if complete && !self.present.is_empty() {
            use super::fonts::{self, Face};
            const SOFT: u32 = 0x005A_4634;
            let m = lay.map;
            let cw = (m.w * 3 / 5).max(420).min(m.w - 20);
            let inner = (cw - 36) as f32;
            let wrapped: Vec<(Vec<String>, Face, f32)> = self.present.iter().enumerate().map(|(k, l)| {
                let (face, px) = if k == 0 { (Face::Italic, 17.0) } else { (Face::Roman, 15.0) };
                (fonts::wrap(l, face, px, inner), face, px)
            }).collect();
            let text_h: usize = wrapped.iter().map(|(v, _, px)| v.len() * (*px as usize + 4) + 6).sum();
            let offers: Vec<(String, Vec<String>)> = self.sites.iter().enumerate().map(|(k, o)| {
                let mut lines = Vec::new();
                for l in [&o.who, &o.trouble, &o.land] { lines.extend(fonts::wrap(l, Face::Roman, 13.0, inner - 16.0).into_iter().take(2)); }
                (format!("{}  {}", k + 1, o.name), lines)
            }).collect();
            let site_h: usize = if offers.is_empty() { 0 } else { 30 + offers.iter().map(|(_, l)| 30 + l.len() * 16 + 8).sum::<usize>() };
            // Over the map, or the whole window when the three sites need the room.
            let ch = (52 + text_h + site_h + 30).min(h - 20);
            let ry = if ch <= m.h - 20 { m.y + (m.h - ch) / 2 } else { (h - ch) / 2 };
            let r = Rect { x: m.x + (m.w - cw) / 2, y: ry, w: cw, h: ch };
            card(buf, w, r);
            let x0 = (r.x + 18) as f32;
            let tw = fonts::width("The World Today", Face::SmallCaps, 24.0, 1.5);
            fonts::draw(buf, w, h, r.x as f32 + (cw as f32 - tw) / 2.0, (r.y + 14) as f32, "The World Today", Face::SmallCaps, 24.0, 1.5, RUBRIC, None);
            let mut y = (r.y + 52) as f32;
            let bottom = (r.y + r.h) as f32 - 30.0;
            for (k, (block, face, px)) in wrapped.iter().enumerate() {
                for l in block {
                    if y + px > bottom { break; }
                    fonts::draw(buf, w, h, x0, y, l, *face, *px, 0.0, if k == 0 { RUBRIC } else { INK }, None);
                    y += px + 4.0;
                }
                y += 6.0;
            }
            // Three places to settle: a click (or 1, 2, 3) embarks there.
            self.site_rects.clear();
            if !offers.is_empty() {
                y += 4.0;
                fonts::draw(buf, w, h, x0, y, "Where to settle", Face::SmallCaps, 17.0, 1.0, RUBRIC, None);
                y += 26.0;
                for (name, lines) in &offers {
                    let bh = 30 + lines.len() * 16;
                    if y as usize + bh > bottom as usize { break; }
                    let bx = Rect { x: r.x + 12, y: y as usize, w: cw - 24, h: bh };
                    super::ui::outline(buf, w, bx, INK_FADED);
                    fonts::draw(buf, w, h, x0, y + 6.0, name, Face::SmallCaps, 16.0, 0.5, RUBRIC, None);
                    let mut ly = y + 28.0;
                    for l in lines {
                        fonts::draw(buf, w, h, x0 + 8.0, ly, l, Face::Roman, 13.0, 0.0, INK, None);
                        ly += 16.0;
                    }
                    self.site_rects.push(bx);
                    y += bh as f32 + 8.0;
                }
            }
            let hint = if self.sites.is_empty() { "Enter: choose where to settle" } else { "Click or 1-3: settle there  \u{b7}  Enter: walk the map yourself" };
            fonts::draw(buf, w, h, x0, (r.y + r.h) as f32 - 24.0, hint, Face::Italic, 14.0, 0.0, SOFT, None);
        }

    }
}

fn run_window(mut window: Window, world: &WorldData, atlas: &Atlas, base: TileWorld, ctl: &Control, rx: mpsc::Receiver<Msg>) -> Result<(), Box<dyn Error>> {
    window.set_target_fps(60);
    let mut view = View::new(world, atlas, base);
    let mut drag: Option<((f32, f32), (f32, f32))> = None;
    let mut scrubbing = false;
    let mut was_down = false;
    // PLANET_WATCH_SCRIPT=FILE: "<frame> key <K>", "<frame> shot <file>", "<frame> status",
    // "<frame> quit" (keys by the viewer's names; the viewer after it reads PLANET_UI_SCRIPT).
    let script: Vec<(u64, String, Vec<String>)> = std::env::var("PLANET_WATCH_SCRIPT").ok().and_then(|f| std::fs::read_to_string(f).ok())
        .map(|t| t.lines().filter_map(|l| { let mut it = l.split_whitespace(); let f = it.next()?.parse().ok()?; let v = it.next()?.to_string(); Some((f, v, it.map(String::from).collect())) }).collect()).unwrap_or_default();
    let mut frame: u64 = 0;

    while window.is_open() {
        frame += 1;
        let mut script_keys: Vec<Key> = Vec::new();
        let mut script_shot: Option<String> = None;
        for (_, verb, args) in script.iter().filter(|e| e.0 == frame) {
            match verb.as_str() {
                "key" => if let Some(k) = args.first().and_then(|n| super::viewer::script_key_pub(n)) { script_keys.push(k); },
                "shot" => script_shot = args.first().cloned(),
                "status" => println!("Watcher script frame {}: year {} | {}", frame, view.current().map_or(0, |s| s.year), view.status),
                "quit" => return Ok(()),
                _ => {}
            }
        }
        while let Ok(msg) = rx.try_recv() { view.receive(msg); }
        let (w, h) = window.get_size();
        view.resize(w, h);
        if view.size.0 == 0 { window.update(); continue; }

        let mouse = window.get_mouse_pos(MouseMode::Clamp).unwrap_or((0.0, 0.0));
        let wheel = window.get_scroll_wheel().map(|s| s.1).unwrap_or(0.0);
        let down = window.get_mouse_down(MouseButton::Left);
        let clicked = down && !was_down;
        was_down = down;
        let pressed = |k: Key| window.is_key_pressed(k, KeyRepeat::No) || script_keys.contains(&k);
        if pressed(Key::Escape) && !view.complete() { break; }
        if view.complete() && (pressed(Key::Enter) || pressed(Key::Escape) || pressed(Key::Q)) { break; }
        // A site chosen on the closing card: the viewer embarks there.
        if view.complete() && !view.sites.is_empty() {
            let by_key = [Key::Key1, Key::Key2, Key::Key3].iter().position(|k| pressed(*k));
            let by_click = if clicked { view.site_rects.iter().position(|r| r.contains(mouse.0, mouse.1)) } else { None };
            if let Some(k) = by_key.or(by_click).filter(|&k| k < view.sites.len()) {
                super::viewer::set_chosen_site(view.sites[k].tile);
                if let Some(c) = view.sites[k].cell { super::viewer::set_start_cell(c); }
                break;
            }
        }
        if pressed(Key::Space) {
            let p = !ctl.paused.load(Ordering::Relaxed);
            ctl.paused.store(p, Ordering::Relaxed);
            view.tw_dirty = true;
        }
        if pressed(Key::LeftBracket) || pressed(Key::Minus) {
            let p = ctl.pace.load(Ordering::Relaxed);
            ctl.pace.store(p.saturating_sub(1), Ordering::Relaxed);
            view.status = format!("pace: {}", PACES[p.saturating_sub(1)].1);
        }
        if pressed(Key::RightBracket) || pressed(Key::Equal) {
            let p = ctl.pace.load(Ordering::Relaxed);
            ctl.pace.store((p + 1).min(PACES.len() - 1), Ordering::Relaxed);
            view.status = format!("pace: {}", PACES[(p + 1).min(PACES.len() - 1)].1);
        }
        // Step one season back or forward while paused.
        if window.is_key_pressed(Key::Comma, KeyRepeat::Yes) || script_keys.contains(&Key::Comma) {
            if let Some(k) = view.shown { view.jump(k.saturating_sub(1)); }
        }
        if window.is_key_pressed(Key::Period, KeyRepeat::Yes) || script_keys.contains(&Key::Period) { view.step_forward(); }
        if pressed(Key::L) { view.show_all = !view.show_all; view.log_scroll = 0; }
        if pressed(Key::H) || pressed(Key::Home) { view.fit(); }
        // Keyboard panning (Shift: faster), steady per second.
        let held = |k: Key| window.is_key_down(k);
        let kx = (held(Key::D) || held(Key::Right)) as i32 - (held(Key::A) || held(Key::Left)) as i32;
        let ky = (held(Key::S) || held(Key::Down)) as i32 - (held(Key::W) || held(Key::Up)) as i32;
        if kx != 0 || ky != 0 {
            let speed = if held(Key::LeftShift) || held(Key::RightShift) { 30.0 } else { 12.0 };
            view.pan(kx as f32 * speed, ky as f32 * speed);
        }
        if pressed(Key::P) {
            let path = format!("watch_{}_{}.png", world.seed(), view.current().map(|s| s.year).unwrap_or(0));
            view.status = save_png(&path, &view.buf, w, h);
        }
        // A: record an album: a plate for each great event from here on.
        if pressed(Key::A) {
            view.album = match view.album.take() {
                Some((dir, n)) => { view.status = format!("album closed: {} plates in {}", n, dir); None }
                None => { let dir = format!("plates/album_{}", world.seed()); let _ = std::fs::create_dir_all(&dir); view.status = format!("recording an album in {}: a plate at each great event", dir); Some((dir, 0)) }
            };
        }
        // G: the history recorded so far as a timelapse GIF.
        if pressed(Key::G) {
            let path = format!("timelapse_{}.gif", world.seed());
            window.set_title("Writing the timelapse...");
            view.status = match export_timelapse(&mut view, ctl, &path, LAPSE_SIZE.0, LAPSE_SIZE.1) {
                Ok((frames, bytes)) => format!("saved {} ({} frames, {:.1} MB)", path, frames, bytes as f64 / 1e6),
                Err(e) => format!("timelapse failed: {e}"),
            };
        }
        // The timeline: click or drag to jump to any recorded season.
        let bar = view.timeline;
        let on_bar = Rect { x: bar.x, y: bar.y.saturating_sub(4), w: bar.w, h: bar.h + 8 }.contains(mouse.0, mouse.1);
        if clicked && on_bar { scrubbing = true; }
        if !down { scrubbing = false; }
        if scrubbing && bar.w > 0 {
            if let Some(total) = view.current().map(|s| s.total) {
                let frac = ((mouse.0 - bar.x as f32) / bar.w as f32).clamp(0.0, 1.0);
                let k = (frac * total as f32).round() as usize;
                if Some(k.min(view.steps.len() - 1)) != view.shown { view.jump(k); }
            }
        }
        let over_map = view.lay.map.contains(mouse.0, mouse.1);
        if wheel != 0.0 && over_map { view.zoom(wheel.signum(), mouse); }
        if wheel != 0.0 && view.lay.log.contains(mouse.0, mouse.1) {
            view.log_scroll = if wheel > 0.0 { view.log_scroll.saturating_sub(3) } else { view.log_scroll + 3 };
        }
        match (down && !scrubbing, drag) {
            (true, None) if over_map => drag = Some((mouse, (view.cam.cx, view.cam.cy))),
            (true, Some((start, c0))) => {
                let (cx, cy) = (c0.0 - (mouse.0 - start.0) / view.cam.tile_px, (c0.1 - (mouse.1 - start.1) / view.cam.tile_px).clamp(0.0, world.height as f32));
                if (cx - view.cam.cx).abs() + (cy - view.cam.cy).abs() > 1e-4 {
                    view.cam.cx = cx;
                    view.cam.cy = cy;
                    view.fitted = false;
                    view.moved();
                }
            }
            (false, _) => drag = None,
            _ => {}
        }
        if clicked && !on_bar {
            if let Some(&(_, (x, y))) = view.entry_hits.iter().find(|(r, _)| r.contains(mouse.0, mouse.1)) {
                view.look_at(x, y, 10.0);
            }
        }

        view.animate();
        view.play(ctl);
        view.draw(mouse, drag.is_none() && !scrubbing, ctl);
        window.update_with_buffer(&view.buf, w, h)?;
        if let Some(path) = script_shot.take() { println!("Watcher script frame {}: {}", frame, save_png(&path, &view.buf, w, h)); }
    }
    Ok(())
}

fn save_png(path: &str, buf: &[u32], w: usize, h: usize) -> String {
    let img = image::RgbImage::from_fn(w as u32, h as u32, |x, y| {
        let p = buf[y as usize * w + x as usize];
        image::Rgb([(p >> 16) as u8, (p >> 8) as u8, p as u8])
    });
    match img.save(path) {
        Ok(()) => format!("saved {path}"),
        Err(e) => format!("save failed: {e}"),
    }
}

/// One frame a year (the same season every frame, so snow doesn't flicker) at 10 a second:
/// 250 years in 25 s.
const LAPSE_SEASONS_PER_SEC: f32 = 40.0;
const LAPSE_FPS: f32 = 10.0;
/// A pixel that moved less than this (per channel) since the last frame is left as it was: the
/// Shadow's wash and the territories shift by a shade every season, and repainting them all made
/// a 250-year timelapse ~29 MB.
const LAPSE_TOLERANCE: i32 = 14;

impl<'a> View<'a> {
    /// Age every mark and the banner by `dt`, as if that much time had passed (offline rendering
    /// draws frames faster than real time, and marks fade by the clock).
    fn age(&mut self, dt: Duration) {
        for m in &mut self.marks { m.born = m.born.checked_sub(dt).unwrap_or(m.born); }
        if let Some(b) = &mut self.banner_msg { b.1 = b.1.checked_sub(dt).unwrap_or(b.1); }
    }
}

/// Write the recording as an animated GIF: the whole history from the first season, the map
/// fitted, at `w` x `h`, ~25 s long, the last frame held 4 s. Returns the frames and bytes
/// written. The view is put back where it was.
fn export_timelapse(view: &mut View, ctl: &Control, path: &str, w: usize, h: usize) -> Result<(usize, u64), String> {
    let (shown, size, fitted, cam) = (view.shown, view.size, view.fitted, view.cam);
    // Drawn as if playing (the panel would say "paused").
    let paused = ctl.paused.swap(false, Ordering::Relaxed);
    view.resize(w, h);
    view.fit();
    let total = view.steps.len();
    if total == 0 { return Err("nothing recorded yet".into()); }
    let stride = ((LAPSE_SEASONS_PER_SEC / LAPSE_FPS).round() as usize).max(1);
    let frame_cs = (100.0 / LAPSE_FPS).round() as u16;
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut enc = gif::Encoder::new(std::io::BufWriter::new(file), w as u16, h as u16, &[]).map_err(|e| e.to_string())?;
    enc.set_repeat(gif::Repeat::Infinite).map_err(|e| e.to_string())?;
    view.jump(0);
    let mut prev: Vec<u32> = Vec::new();
    let mut frames = 0;
    loop {
        view.last_render = Instant::now() - Duration::from_secs(1);
        view.map_dirty = true;
        view.draw((-1.0, -1.0), false, ctl);
        let last = view.shown.map_or(true, |k| k + 1 >= total);
        // Only what changed since the last frame, unchanged pixels transparent: most of a frame
        // (parchment, the panel's frame, settled land) stays the same, and full frames came to
        // ~250 KB each.
        let cur = &view.buf;
        // `prev` is what the GIF shows so far; a pixel is repainted only if it moved past the
        // tolerance from that.
        let near = |a: u32, b: u32| ((a >> 16 & 255) as i32 - (b >> 16 & 255) as i32).abs() <= LAPSE_TOLERANCE
            && ((a >> 8 & 255) as i32 - (b >> 8 & 255) as i32).abs() <= LAPSE_TOLERANCE
            && ((a & 255) as i32 - (b & 255) as i32).abs() <= LAPSE_TOLERANCE;
        let fresh = prev.len() != cur.len();
        let changed: Vec<bool> = if fresh { vec![true; cur.len()] } else { (0..cur.len()).map(|i| !near(cur[i], prev[i])).collect() };
        let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0usize, 0usize);
        for y in 0..h { for x in 0..w { if changed[y * w + x] { x0 = x0.min(x); x1 = x1.max(x); y0 = y0.min(y); y1 = y1.max(y); } } }
        if x1 < x0 { (x0, y0, x1, y1) = (0, 0, 0, 0); }
        let (fw, fh) = (x1 - x0 + 1, y1 - y0 + 1);
        let mut rgba = Vec::with_capacity(fw * fh * 4);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let p = cur[y * w + x];
                rgba.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8, if changed[y * w + x] { 255 } else { 0 }]);
            }
        }
        let mut frame = gif::Frame::from_rgba_speed(fw as u16, fh as u16, &mut rgba, 10);
        frame.left = x0 as u16;
        frame.top = y0 as u16;
        frame.delay = if last { 400 } else { frame_cs };
        frame.dispose = gif::DisposalMethod::Keep;
        enc.write_frame(&frame).map_err(|e| e.to_string())?;
        if fresh { prev = cur.clone(); } else { for i in 0..cur.len() { if changed[i] { prev[i] = cur[i]; } } }
        frames += 1;
        if last { break; }
        for _ in 0..stride { view.step_forward(); }
        // Marks and banners fade as they would in the window at this pace.
        view.age(Duration::from_secs_f32(stride as f32 * 0.35));
    }
    drop(enc);
    let bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    if size.0 >= 200 { view.resize(size.0, size.1); }
    view.cam = cam;
    view.fitted = fitted;
    view.map_dirty = true;
    if let Some(k) = shown { view.jump(k); }
    ctl.paused.store(paused, Ordering::Relaxed);
    Ok((frames, bytes))
}

/// Simulate `config`'s history without a window and write the whole of it as a timelapse GIF
/// (`--watch-timelapse FILE`).
pub fn watch_timelapse(world: &WorldData, game_data: &GameData, config: HistoryConfig, engine: HistoryEngine, atlas: &Atlas, path: &str) -> WorldHistory {
    let mut base = TileWorld::build(world, atlas);
    base.set_season(world, Season::Summer);
    let ctl = Control { paused: AtomicBool::new(true), pace: AtomicUsize::new(PACES.len() - 1), detached: AtomicBool::new(false) };
    let (tx, rx) = mpsc::channel();
    std::thread::scope(|scope| {
        let sim = {
            let ctl = &ctl;
            scope.spawn(move || simulate(world, game_data, config, engine, ctl, tx))
        };
        let mut view = View::new(world, atlas, base);
        view.resize(LAPSE_SIZE.0, LAPSE_SIZE.1);
        for msg in rx.iter() { view.receive(msg); }
        let t0 = Instant::now();
        match export_timelapse(&mut view, &ctl, path, LAPSE_SIZE.0, LAPSE_SIZE.1) {
            Ok((frames, bytes)) => println!("Timelapse: {} frames, {:.1} MB, written to {} in {:.1}s", frames, bytes as f64 / 1e6, path, t0.elapsed().as_secs_f32()),
            Err(e) => eprintln!("Timelapse failed: {e}"),
        }
        sim.join().expect("history simulation panicked")
    })
}

/// An atlas of ages (`--watch-atlas FILE`): simulate the history as `--watch` does, then draw the
/// world at four moments, the dawn of history, a third and two thirds through, and the present,
/// two by two on one parchment plate. Each map has the realms, roads and towns of its year and
/// is captioned with the year and the age it fell in.
pub fn watch_atlas(world: &WorldData, game_data: &GameData, config: HistoryConfig, engine: HistoryEngine, atlas: &Atlas, path: &str) -> WorldHistory {
    let mut base = TileWorld::build(world, atlas);
    base.set_season(world, Season::Summer);
    let ctl = Control { paused: AtomicBool::new(true), pace: AtomicUsize::new(PACES.len() - 1), detached: AtomicBool::new(false) };
    let (tx, rx) = mpsc::channel();
    std::thread::scope(|scope| {
        let sim = {
            let ctl = &ctl;
            scope.spawn(move || simulate(world, game_data, config, engine, ctl, tx))
        };
        let mut view = View::new(world, atlas, base.clone());
        view.resize(LAPSE_SIZE.0, LAPSE_SIZE.1);
        for msg in rx.iter() { view.receive(msg); }
        let history = sim.join().expect("history simulation panicked");
        let total = view.steps.len();
        if total == 0 { eprintln!("Atlas: nothing recorded"); return history; }
        // The plate: 2x2 maps with margins, a title band.
        let (mw, mh) = (760usize, (760 * world.height / world.width).max(200));
        let (gap, top, margin) = (28usize, 92usize, 30usize);
        let (pw, ph) = (margin * 2 + mw * 2 + gap, top + (mh + 44) * 2 + gap + margin);
        let mut plate = vec![0u32; pw * ph];
        for y in 0..ph { for x in 0..pw { let n = (super::ui::hash(x / 3, y / 3) & 0xFF) as f32 / 255.0; plate[y * pw + x] = super::ui::mix(super::ui::PAPER, 0x00D8_C8A0, 0.35 * n); } }
        let gaz = crate::lore::gazetteer::build_gazetteer(world, Some(&history), world.seed());
        let name = gaz.features.iter().filter(|f| f.kind == crate::lore::FeatureKind::Continent).max_by_key(|f| f.size).map(|f| f.name.clone()).unwrap_or_else(|| "the World".into());
        let title = format!("An Atlas of Ages: the Lands of {}", name);
        let tw_ = super::fonts::width(&title, super::fonts::Face::SmallCaps, 30.0, 2.0);
        super::fonts::draw(&mut plate, pw, ph, (pw as f32 - tw_) / 2.0, 26.0, &title, super::fonts::Face::SmallCaps, 30.0, 2.0, super::ui::RUBRIC, None);
        for (i, k) in [0, total / 3, total * 2 / 3, total - 1].into_iter().enumerate() {
            view.jump(k);
            let mut tw = base.clone();
            tw.apply_overlay(world, &view.cur, atlas);
            let cam = Camera { cx: world.width as f32 / 2.0, cy: world.height as f32 / 2.0, tile_px: mw as f32 / world.width as f32 };
            let mut buf = vec![0u32; mw * mh];
            render_world_lod(&tw, atlas, &cam, &mut buf, mw, mh, 1);
            overlay_realms(&tw, &cam, &mut buf, mw, mh);
            let step = &*view.steps[k];
            let labels = settlement_labels(step, &tw);
            place_labels(&labels, cam.tile_px, mw, mh, &mut buf, &[], |x, y| (mw as f32 / 2.0 + (x - cam.cx) * cam.tile_px, mh as f32 / 2.0 + (y - cam.cy) * cam.tile_px));
            let (ox, oy) = (margin + (i % 2) * (mw + gap), top + (i / 2) * (mh + 44 + gap));
            for y in 0..mh { for x in 0..mw { plate[(oy + y) * pw + ox + x] = buf[y * mw + x]; } }
            let r = Rect { x: ox, y: oy, w: mw, h: mh };
            super::ui::outline(&mut plate, pw, r, super::ui::INK);
            super::ui::outline(&mut plate, pw, Rect { x: ox - 4, y: oy - 4, w: mw + 8, h: mh + 8 }, super::ui::INK_FADED);
            let year = step.year;
            let age = history.timeline.eras.iter().find(|e| e.start.year <= year && e.end.map_or(true, |d| d.year >= year)).map(|e| e.name.clone()).unwrap_or_default();
            let towns = step.places.values().filter(|p| !p.4).count();
            let cap = if age.is_empty() { format!("Year {}: {} towns, {} realms", year, towns, step.realms.len()) } else { format!("Year {}, {}: {} towns, {} realms", year, age, towns, step.realms.len()) };
            super::fonts::draw(&mut plate, pw, ph, ox as f32, (oy + mh + 10) as f32, &cap, super::fonts::Face::Italic, 18.0, 0.0, super::ui::INK, None);
        }
        let outer = Rect { x: 10, y: 10, w: pw - 20, h: ph - 20 };
        super::ui::outline(&mut plate, pw, outer, super::ui::INK);
        super::ui::outline(&mut plate, pw, Rect { x: 13, y: 13, w: pw - 26, h: ph - 26 }, super::ui::INK_FADED);
        super::viewer::save_rgb_png_pub(path, pw, ph, |x, y| { let q = plate[y * pw + x]; [(q >> 16) as u8, (q >> 8) as u8, q as u8] });
        println!("Atlas of ages: 4 maps (years {}), written to {}", [0, total / 3, total * 2 / 3, total - 1].iter().map(|&k| view.steps[k].year.to_string()).collect::<Vec<_>>().join(", "), path);
        history
    })
}

/// One plate of the album: the world at step `k` (as it stood that season), centred on the
/// event's place at 12 px a tile, the place ringed in rubric, framed as a plate with the event
/// as its caption. Returns what it says it saved.
fn album_plate(view: &View, k: usize, e: &LogItem, dir: &str, n: usize) -> String {
    let (world, atlas) = (view.world, view.atlas);
    let Some((x, y)) = e.location else { return String::new() };
    let mut tw = view.base.clone();
    tw.apply_overlay(world, &view.cur, atlas);
    let (w, h) = (1280usize, 800usize);
    // About forty tiles across: close enough to read the place, wide enough for its country.
    let tile_px = (w as f32 / 40.0).max(12.0);
    let half_h = h as f32 / 2.0 / tile_px;
    let cy = (y as f32 + 0.5).clamp(half_h.min(world.height as f32 / 2.0), (world.height as f32 - half_h).max(world.height as f32 / 2.0));
    let cam = Camera { cx: x as f32 + 0.5, cy, tile_px };
    let mut buf = vec![0u32; w * h];
    render_world_lod(&tw, atlas, &cam, &mut buf, w, h, 1);
    overlay_realms(&tw, &cam, &mut buf, w, h);
    let step = &*view.steps[k];
    let labels = settlement_labels(step, &tw);
    let ww = world.width as f32;
    place_labels(&labels, cam.tile_px, w, h, &mut buf, &[], |lx, ly| {
        let mut dx = lx - cam.cx;
        if dx > ww / 2.0 { dx -= ww; }
        if dx < -ww / 2.0 { dx += ww; }
        (w as f32 / 2.0 + dx * cam.tile_px, h as f32 / 2.0 + (ly - cam.cy) * cam.tile_px)
    });
    // The place: a rubric ring with a fine ink rim.
    let (sx, sy) = (w as f32 / 2.0, h as f32 / 2.0 + (y as f32 + 0.5 - cy) * tile_px);
    for py in (sy as i64 - 22).max(0)..(sy as i64 + 22).min(h as i64) {
        for px in (sx as i64 - 22).max(0)..(sx as i64 + 22).min(w as i64) {
            let d = (((px as f32 - sx).powi(2) + (py as f32 - sy).powi(2)) as f32).sqrt();
            let k2 = py as usize * w + px as usize;
            if (d - 16.0).abs() < 1.6 { buf[k2] = super::ui::mix(buf[k2], RUBRIC, 0.9); } else if (d - 18.4).abs() < 0.7 { buf[k2] = super::ui::mix(buf[k2], INK, 0.7); }
        }
    }
    let info = super::plates::PlateInfo { world_name: view.world_name.clone(), year: Some(e.year), season: format!("{:?}", step.season), seed: world.seed(), caption: e.title.clone(), realms: Vec::new() };
    super::plates::decorate(&mut buf, w, h, &info);
    let slug: String = e.title.chars().filter(|c| c.is_ascii_alphanumeric() || *c == ' ').collect::<String>().split_whitespace().take(5).collect::<Vec<_>>().join("_").to_lowercase();
    let path = format!("{}/{:03}_{}_{}.png", dir, n + 1, e.year, slug);
    super::viewer::save_rgb_png_pub(&path, w, h, |px, py| { let q = buf[py * w + px]; [(q >> 16) as u8, (q >> 8) as u8, q as u8] });
    format!("album: {}", path)
}

/// `--watch-album DIR`: simulate the history and play it through as the window would, saving a
/// plate for each great event with a place (a falling town, a great battle, the Shadow's
/// conquests and its check, beasts, scars) into DIR.
pub fn watch_album(world: &WorldData, game_data: &GameData, config: HistoryConfig, engine: HistoryEngine, atlas: &Atlas, dir: &str) -> WorldHistory {
    let mut base = TileWorld::build(world, atlas);
    base.set_season(world, Season::Summer);
    let ctl = Control { paused: AtomicBool::new(true), pace: AtomicUsize::new(PACES.len() - 1), detached: AtomicBool::new(false) };
    let (tx, rx) = mpsc::channel();
    let _ = std::fs::create_dir_all(dir);
    std::thread::scope(|scope| {
        let sim = {
            let ctl = &ctl;
            scope.spawn(move || simulate(world, game_data, config, engine, ctl, tx))
        };
        let mut view = View::new(world, atlas, base);
        view.resize(LAPSE_SIZE.0, LAPSE_SIZE.1);
        for msg in rx.iter() { view.receive(msg); }
        let history = sim.join().expect("history simulation panicked");
        let gaz = crate::lore::gazetteer::build_gazetteer(world, Some(&history), world.seed());
        view.world_name = gaz.features.iter().filter(|f| f.kind == crate::lore::FeatureKind::Continent).max_by_key(|f| f.size).map(|f| f.name.clone()).unwrap_or_else(|| "the World".into());
        view.album = Some((dir.to_string(), 0));
        view.jump(0);
        let total = view.steps.len();
        while view.shown.map_or(false, |k| k + 1 < total) { view.step_forward(); }
        let n = view.album.as_ref().map_or(0, |a| a.1);
        println!("Album: {} plates of great events written to {}", n, dir);
        history
    })
}

/// The timelapse frame: wide enough for the panel and a readable map.
const LAPSE_SIZE: (usize, usize) = (960, 600);

/// Simulate `config`'s history without a window and save watcher frames as
/// `<prefix>_y<year>.png` at a quarter, half and the end, plus a close-up of the last event
/// with a place, for checking the watcher's look headlessly.
/// Frame budget of the history watcher (`--frame-bench-watch`): the history is simulated, then
/// played back a step at a time as the window would (each step drawn), then the camera pans
/// across the map. Prints the frame times of each.
pub fn watch_frame_bench(world: &WorldData, game_data: &GameData, config: HistoryConfig, engine: HistoryEngine, atlas: &Atlas) -> WorldHistory {
    let mut base = TileWorld::build(world, atlas);
    base.set_season(world, Season::Summer);
    let total = config.total_steps() as usize;
    let ctl = Control { paused: AtomicBool::new(false), pace: AtomicUsize::new(PACES.len() - 1), detached: AtomicBool::new(false) };
    let (tx, rx) = mpsc::channel();
    let report = |what: &str, ms: &mut Vec<f64>| {
        if ms.is_empty() { return; }
        ms.sort_by(|a, b| a.total_cmp(b));
        let q = |f: f64| ms[((ms.len() - 1) as f64 * f) as usize];
        let over = ms.iter().filter(|&&t| t > 1000.0 / 60.0).count();
        println!("{}: {} frames, ms p50 {:.2} p95 {:.2} p99 {:.2} max {:.1}; over 16.7 ms: {}", what, ms.len(), q(0.5), q(0.95), q(0.99), q(1.0), over);
    };
    std::thread::scope(|scope| {
        let sim = {
            let ctl = &ctl;
            scope.spawn(move || simulate(world, game_data, config, engine, ctl, tx))
        };
        let mut view = View::new(world, atlas, base);
        view.resize(1280, 800);
        for msg in rx.iter() { view.receive(msg); }
        let mut ms = Vec::new();
        view.jump(0);
        for _ in 0..total.min(400) {
            view.step_forward();
            view.last_render = Instant::now() - Duration::from_secs(1);
            let t0 = Instant::now();
            view.draw((0.0, 0.0), false, &ctl);
            ms.push(t0.elapsed().as_secs_f64() * 1000.0);
        }
        report("Watcher playing a step a frame", &mut ms);
        let mut ms = Vec::new();
        for f in 0..300 {
            let a = f as f32 * 0.02;
            view.cam.cx = world.width as f32 / 2.0 + 0.3 * world.width as f32 * a.cos();
            view.cam.cy = world.height as f32 / 2.0 + 0.2 * world.height as f32 * a.sin();
            view.moved = Instant::now();
            view.map_dirty = true;
            let t0 = Instant::now();
            view.draw((0.0, 0.0), false, &ctl);
            ms.push(t0.elapsed().as_secs_f64() * 1000.0);
        }
        report("Watcher panning", &mut ms);
        sim.join().expect("history simulation panicked")
    })
}

pub fn watch_snapshots(world: &WorldData, game_data: &GameData, config: HistoryConfig, engine: HistoryEngine, atlas: &Atlas, prefix: &str) -> (WorldHistory, Vec<String>) {
    let mut base = TileWorld::build(world, atlas);
    base.set_season(world, Season::Summer);
    let total = config.total_steps() as usize;
    let ctl = Control { paused: AtomicBool::new(true), pace: AtomicUsize::new(PACES.len() - 1), detached: AtomicBool::new(false) };
    let (tx, rx) = mpsc::channel();
    let mut files = Vec::new();
    let history = std::thread::scope(|scope| {
        let sim = {
            let ctl = &ctl;
            scope.spawn(move || simulate(world, game_data, config, engine, ctl, tx))
        };
        let mut view = View::new(world, atlas, base);
        view.resize(1440, 900);
        for msg in rx.iter() { view.receive(msg); }
        for k in [total / 4, total / 2, total] {
            view.jump(k);
            // Show the season's own events as fresh marks, as during playback.
            if k > 0 { view.jump(k - 1); view.step_forward(); }
            view.last_render = Instant::now() - Duration::from_secs(1);
            view.draw((0.0, 0.0), false, &ctl);
            let path = format!("{prefix}_y{}.png", view.current().map(|s| s.year).unwrap_or(0));
            save_png(&path, &view.buf, 1440, 900);
            files.push(path);
        }
        if let Some((x, y)) = view.log.iter().find_map(|e| e.location) {
            view.look_at(x, y, 10.0);
            view.last_render = Instant::now() - Duration::from_secs(1);
            view.draw((0.0, 0.0), false, &ctl);
            let path = format!("{prefix}_closeup.png");
            save_png(&path, &view.buf, 1440, 900);
            files.push(path);
        }
        sim.join().expect("history simulation panicked")
    });
    (history, files)
}

/// Realms and roads at the zooms where the tile renderer leaves them out (it draws roads from
/// 3 px per tile and borders from 10): a faction-coloured wash over claimed land with a border
/// where the owner changes, and roads as sepia lines between tile centres.
fn overlay_realms(tw: &TileWorld, cam: &Camera, buf: &mut [u32], w: usize, h: usize) {
    let t = cam.tile_px;
    if t < 10.0 {
        let tile_at = |sx: usize, sy: usize| -> Option<usize> {
            let (wx, wy) = screen_to_world(cam, sx as f32 + 0.5, sy as f32 + 0.5, w, h);
            if wy < 0.0 || wy >= tw.height as f32 { return None; }
            Some(wy as usize * tw.width + (wx.floor() as i64).rem_euclid(tw.width as i64) as usize)
        };
        let owner = |i: usize| if tw.ground[i].is_water() { u64::MAX } else { tw.owner[i] };
        let row: Vec<Option<usize>> = (0..w).map(|sx| tile_at(sx, 0)).collect();
        let col_x: Vec<i64> = row.iter().map(|i| i.map(|i| (i % tw.width) as i64).unwrap_or(-1)).collect();
        for sy in 0..h {
            let (_, wy) = screen_to_world(cam, 0.0, sy as f32 + 0.5, w, h);
            if wy < 0.0 || wy >= tw.height as f32 { continue; }
            let ty = wy as usize;
            let (_, wy2) = screen_to_world(cam, 0.0, sy as f32 + 1.5, w, h);
            let ty2 = (wy2.max(0.0) as usize).min(tw.height - 1);
            for sx in 0..w {
                if col_x[sx] < 0 { continue; }
                let i = ty * tw.width + col_x[sx] as usize;
                let o = owner(i);
                if o == u64::MAX { continue; }
                let right = if sx + 1 < w { owner(ty * tw.width + col_x[sx + 1] as usize) } else { o };
                let below = owner(ty2 * tw.width + col_x[sx] as usize);
                let c = faction_color(o);
                let k = sy * w + sx;
                buf[k] = if right != o || below != o { mix(buf[k], mix(c, INK, 0.25), 0.85) } else { mix(buf[k], c, 0.2) };
            }
        }
    }
    if t < 3.0 {
        // Roads along the same curves the tile renderer draws close up.
        let (x0, y0) = screen_to_world(cam, 0.0, 0.0, w, h);
        let (x1, y1) = screen_to_world(cam, w as f32, h as f32, w, h);
        let road = mix(0x0088_5C3C, INK, 0.2);
        for ty in (y0.floor().max(0.0) as usize)..(y1.ceil().max(0.0) as usize).min(tw.height) {
            for txi in x0.floor() as i64..=x1.ceil() as i64 {
                let tx = txi.rem_euclid(tw.width as i64) as usize;
                let j = ty * tw.width + tx;
                let n = tw.road_strokes.len[j] as usize;
                if n == 0 { continue; }
                let ox = w as f32 / 2.0 + (txi as f32 - cam.cx) * t;
                let oy = h as f32 / 2.0 + (ty as f32 - cam.cy) * t;
                let s0 = tw.road_strokes.start[j] as usize;
                for sg in &tw.road_strokes.segs[s0..s0 + n] {
                    let len = ((sg[2] - sg[0]).hypot(sg[3] - sg[1]) * t).ceil().max(1.0) as i64;
                    for k in 0..=len {
                        let f = k as f32 / len as f32;
                        let (px, py) = (ox + (sg[0] + (sg[2] - sg[0]) * f) * t, oy + (sg[1] + (sg[3] - sg[1]) * f) * t);
                        blend_px(buf, w, h, px as i64, py as i64, road, 0.9);
                    }
                }
            }
        }
    }
}

/// Labels for the larger living settlements (capitals first).
fn settlement_labels(f: &Step, tw: &TileWorld) -> Vec<Label> {
    let mut by_id: HashMap<u64, (usize, usize)> = HashMap::new();
    for (i, s) in tw.settlement.iter().enumerate() {
        if let Some(id) = s { by_id.insert(id.0, (i % tw.width, i / tw.width)); }
    }
    let mut labels: Vec<Label> = f.places.iter().filter_map(|(id, (name, kind, pop, _, ruined))| {
        if *ruined { return None; }
        let &(x, y) = by_id.get(id)?;
        let (rank, min_px) = match kind {
            SettlementType::Capital => (900, 2.5),
            SettlementType::City | SettlementType::Port => (850, 6.0),
            SettlementType::Town | SettlementType::Fort => (700, 10.0),
            _ => (400, 16.0),
        };
        Some(Label { x: x as f32 + 0.5, y: y as f32 + 1.6, text: name.clone(), rank: rank + (*pop / 2000).min(99), min_tile_px: min_px, color: INK, style: super::text::LabelStyle::Town, angle: 0.0 })
    }).collect();
    labels.sort_by_key(|l| std::cmp::Reverse(l.rank));
    labels
}

fn hover_text(f: &Step, tw: &TileWorld, x: usize, y: usize) -> String {
    let i = y * tw.width + x;
    let mut parts = Vec::new();
    if let Some(id) = tw.settlement[i] {
        if let Some((name, kind, pop, fac, ruined)) = f.places.get(&id.0) {
            let realm = f.names.get(fac).map(|s| ascii(s)).unwrap_or_default();
            parts.push(if *ruined { format!("ruins of {}", ascii(name)) } else { format!("{} - {:?} of {}, {} souls", ascii(name), kind, realm, short_num(*pop as u64)) });
        }
    } else if tw.owner[i] != u64::MAX {
        if let Some(n) = f.names.get(&tw.owner[i]) { parts.push(format!("lands of {}", ascii(n))); }
    }
    if tw.road[i] != 0 && tw.settlement[i].is_none() { parts.push("a road".into()); }
    parts.join(" | ")
}

#[allow(clippy::too_many_arguments)]
/// The almanac panel. Returns the timeline bar's rectangle (click or drag it to jump).
fn draw_panel(buf: &mut [u32], w: usize, h: usize, p: Rect, f: Option<&Step>, souls_hist: &[u64], status: &str, ctl: &Control, done: bool, written: u32) -> Rect {
    let x = p.x + 16;
    let iw = p.w - 32;
    let mut y = p.y as i64 + 16;
    fell(buf, w, h, x as i64, y, "The World", RUBRIC, 2, true);
    y += 22;
    fell(buf, w, h, x as i64, y, "takes shape", RUBRIC, 2, true);
    y += 26;
    hline(buf, w, x, x + iw, y as usize, INK);
    hline(buf, w, x, x + iw, y as usize + 2, INK_FADED);
    y += 12;

    let Some(f) = f else {
        for line in wrap(status, iw / 7) {
            fell(buf, w, h, x as i64, y, &line, INK, 1, false);
            y += LINE + 4;
        }
        return Rect::default();
    };

    // Year and season, large.
    let year = format!("Year {}", f.year);
    fell(buf, w, h, x as i64, y, &year, INK, 3, true);
    y += 28;
    fell(buf, w, h, x as i64, y, season_name(f.season), INK_FADED, 2, false);
    y += 22;
    // The timeline: written so far (pale), shown (gold), a tick every 50 years. Click or drag
    // it to jump.
    let bar = Rect { x, y: y as usize, w: iw, h: 9 };
    outline(buf, w, bar, INK);
    let span = (bar.w - 4) as f32;
    let total = f.total.max(1) as f32;
    let written_w = (span * (written as f32 / total).min(1.0)) as usize;
    if written_w > 0 { fill(buf, w, Rect { x: bar.x + 2, y: bar.y + 2, w: written_w, h: bar.h - 4 }, mix(PAPER_SHADE, INK_FADED, 0.35)); }
    let filled = (span * (f.step as f32 / total).min(1.0)) as usize;
    if filled > 0 { fill(buf, w, Rect { x: bar.x + 2, y: bar.y + 2, w: filled, h: bar.h - 4 }, mix(INK, GOLD, 0.6)); }
    // The playhead.
    let hx = (bar.x + 2 + filled).min(bar.x + bar.w - 1);
    for ty in bar.y.saturating_sub(2)..bar.y + bar.h + 2 { buf[ty * w + hx] = RUBRIC; }
    let years = f.total / 4;
    for k in (50..years).step_by(50) {
        let tx = bar.x + ((bar.w as f32) * k as f32 / years.max(1) as f32) as usize;
        for ty in bar.y + bar.h..bar.y + bar.h + 3 { buf[ty * w + tx] = INK; }
    }
    y += 14;
    fell(buf, w, h, x as i64, y, &format!("{} of {} years", f.step / 4, years), INK_FADED, 1, false);
    let paused = ctl.paused.load(Ordering::Relaxed);
    let paused_label = format!("- paused ({}) -", PACES[ctl.pace.load(Ordering::Relaxed).min(PACES.len() - 1)].1);
    let state = if done { "complete" } else if paused { paused_label.as_str() } else { PACES[ctl.pace.load(Ordering::Relaxed).min(PACES.len() - 1)].1 };
    let sc = if paused { RUBRIC } else { INK_FADED };
    fell(buf, w, h, (x + iw - fell_width(state, 1)) as i64, y, state, sc, 1, paused);
    y += LINE + 4;
    if !done && !paused {
        fell(buf, w, h, x as i64, y, &truncate(status, iw / 7), INK_FADED, 1, false);
    }
    y += LINE + 6;

    // Almanac.
    fell_heading(buf, w, h, x, y, iw, "ALMANAC");
    y += 16;
    let s = &f.stats;
    let rows = [
        ("Peoples", format!("{}", s.peoples), format!("{} fallen", s.fallen)),
        ("Settlements", format!("{}", s.towns), format!("{} in ruin", s.ruins)),
        ("Roads", format!("{} tiles", s.roads), String::new()),
        ("Wars raging", format!("{}", s.wars), String::new()),
        ("Legendary beasts", format!("{}", s.beasts), String::new()),
        ("Treasures", format!("{}", s.artifacts), format!("{} monuments", s.monuments)),
    ];
    for (label, value, note) in rows {
        fell(buf, w, h, x as i64, y, label, INK, 1, false);
        let vx = x + 140;
        fell(buf, w, h, vx as i64, y, &value, if label == "Wars raging" && s.wars > 0 { RUBRIC } else { INK }, 1, true);
        if !note.is_empty() {
            fell(buf, w, h, (x + iw - fell_width(&note, 1)) as i64, y, &note, INK_FADED, 1, false);
        }
        y += LINE + 4;
    }
    y += 6;

    // Souls over the age, as an ink line over a pale wash.
    fell_heading(buf, w, h, x, y, iw, &format!("SOULS  {}", short_num(s.souls)));
    y += 14;
    let chart = Rect { x, y: y as usize, w: iw, h: 44 };
    if souls_hist.len() >= 2 {
        let max = *souls_hist.iter().max().unwrap_or(&1) as f32;
        let min = *souls_hist.iter().min().unwrap_or(&0) as f32 * 0.9;
        let n = souls_hist.len();
        let mut prev: Option<(i64, i64)> = None;
        for cx in 0..chart.w {
            let k = (cx * (n - 1)) / (chart.w - 1).max(1);
            let v = (souls_hist[k] as f32 - min) / (max - min).max(1.0);
            let py = chart.y as i64 + chart.h as i64 - 1 - (v * (chart.h - 2) as f32) as i64;
            for yy in py.max(chart.y as i64)..(chart.y + chart.h) as i64 {
                blend_px(buf, w, h, (chart.x + cx) as i64, yy, GOLD, 0.18);
            }
            if let Some((px0, py0)) = prev {
                for yy in py.min(py0)..=py.max(py0) { blend_px(buf, w, h, px0 + 1, yy, INK, 0.9); }
            }
            blend_px(buf, w, h, (chart.x + cx) as i64, py, INK, 1.0);
            prev = Some(((chart.x + cx) as i64, py));
        }
    }
    hline(buf, w, chart.x, chart.x + chart.w, chart.y + chart.h, INK_FADED);
    y += chart.h as i64 + 12;

    // The Shadow.
    if let Some(sh) = &s.shadow {
        fell_heading(buf, w, h, x, y, iw, if sh.broken { "THE SHADOW, BROKEN" } else { "THE SHADOW" });
        y += 16;
        let mut title = ascii(&sh.name);
        if let Some(c) = title.get_mut(0..1) { c.make_ascii_uppercase(); }
        fell(buf, w, h, x as i64, y, &truncate(&title, iw / 7), RUBRIC, 1, true);
        y += LINE + 4;
        fell(buf, w, h, x as i64, y, &truncate(&ascii(&sh.lord), iw / 7), INK_FADED, 1, false);
        y += LINE + 6;
        // Darkened land: a bar of reach with blight inside it.
        let bar = Rect { x, y: y as usize, w: iw, h: 7 };
        outline(buf, w, bar, INK);
        let rw = ((bar.w - 2) as f32 * sh.reach.min(1.0)) as usize;
        let bw = ((bar.w - 2) as f32 * sh.blight.min(1.0)) as usize;
        if rw > 0 { fill(buf, w, Rect { x: bar.x + 1, y: bar.y + 1, w: rw, h: bar.h - 2 }, 0x0090_8478); }
        if bw > 0 { fill(buf, w, Rect { x: bar.x + 1, y: bar.y + 1, w: bw, h: bar.h - 2 }, 0x001E_141A); }
        y += 12;
        let line = format!("{:.0}% of the land darkened, {:.0}% blighted", sh.reach * 100.0, sh.blight * 100.0);
        fell(buf, w, h, x as i64, y, &truncate(&line, iw / 7), INK_FADED, 1, false);
        y += LINE + 4;
        let line = format!("holds {} towns   {} fallen   {} held out", sh.towns, sh.fallen, sh.held);
        fell(buf, w, h, x as i64, y, &truncate(&line, iw / 7), INK, 1, false);
        y += LINE + 8;
    }

    // The great realms.
    fell_heading(buf, w, h, x, y, iw, "GREAT REALMS");
    y += 16;
    let top = f.realms.first().map(|r| r.population).unwrap_or(1).max(1);
    let footer_top = (p.y + p.h) as i64 - 80;
    for r in &f.realms {
        if y + 22 > footer_top { break; }
        super::heraldry::draw(buf, w, h, x as i64, y - 2, 15, &r.arms);
        let pop = format!("{}  {}", r.towns, short_num(r.population));
        let name_chars = (iw - 18 - fell_width(&pop, 1) - 8) / 7;
        let dark = s.shadow.as_ref().map_or(false, |sh| sh.faction == r.id && !sh.broken);
        fell(buf, w, h, (x + 16) as i64, y + 1, &truncate(&ascii(&r.name), name_chars), if dark { RUBRIC } else { INK }, 1, dark);
        fell(buf, w, h, (x + iw - fell_width(&pop, 1)) as i64, y + 1, &pop, INK_FADED, 1, false);
        let bw = ((iw - 16) as f32 * r.population as f32 / top as f32) as usize;
        if bw > 0 { hline(buf, w, x + 16, x + 16 + bw, (y + 16) as usize, mix(faction_color(r.id), INK, 0.2)); }
        y += 22;
    }

    // Controls.
    let mut fy = footer_top + 2;
    hline(buf, w, x, x + iw, fy as usize - 4, INK_FADED);
    for line in [
        "SPACE pause   [ ] pace   < > one season",
        "drag the timeline to any year   L log   G gif   A album",
        "wheel zoom   drag/WASD pan   H fit map",
        if done { "ENTER choose where to settle" } else { "click an entry: go there   ESC hurry" },
    ] {
        fy += 2;
        fell(buf, w, h, x as i64, fy, line, INK_FADED, 1, false);
        fy += LINE + 3;
    }
    bar
}

/// The chronicle: newest entries first, red year rubrics, glyphs by kind. Returns the clickable
/// rectangles of entries with a place.
fn draw_log(buf: &mut [u32], w: usize, h: usize, r: Rect, log: &VecDeque<LogItem>, show_all: bool, scroll: &mut usize, mouse: (f32, f32)) -> Vec<(Rect, (usize, usize))> {
    let x = r.x + 16;
    let iw = r.w.saturating_sub(32);
    let mut y = r.y as i64 + 14;
    use super::fonts::{self, Face};
    // Lettered in IM Fell: the years as red rubrics, the entries in roman (great events in small
    // capitals), 15 px.
    const PX: f32 = 15.0;
    const ROW: i64 = 19;
    fonts::draw(buf, w, h, x as f32, y as f32 - 4.0, "Chronicle of the Age", Face::SmallCaps, 17.0, 1.0, RUBRIC, None);
    let filter = if show_all { "all that happens" } else { "key events" };
    let fw = fonts::width(filter, Face::Italic, 13.0, 0.0);
    fonts::draw(buf, w, h, (x + iw) as f32 - fw, y as f32 - 2.0, filter, Face::Italic, 13.0, 0.0, 0x005A_4634, None);
    y += 22;
    let bottom = (r.y + r.h) as i64 - if *scroll > 0 { 24 } else { 10 };
    let text_x = x + 64;
    let text_w = iw.saturating_sub(64) as f32;
    let mut hits = Vec::new();
    let entries: Vec<&LogItem> = log.iter().filter(|e| show_all || e.key).collect();
    *scroll = (*scroll).min(entries.len().saturating_sub(1));
    let mut last_year = None;
    for (n, e) in entries.iter().skip(*scroll).enumerate() {
        let face = if e.kind.is_major() { Face::SmallCaps } else { Face::Roman };
        let lines = fonts::wrap(&e.title, face, PX, text_w.max(80.0));
        let eh = lines.len() as i64 * ROW;
        if y + eh > bottom { break; }
        let rect = Rect { x: x - 4, y: (y - 2) as usize, w: iw + 8, h: eh as usize + 2 };
        let hover = e.location.is_some() && rect.contains(mouse.0, mouse.1);
        if hover { fill(buf, w, rect, PAPER_SHADE); }
        // Older entries fade towards the paper (no further than a readable brown).
        let age = (n as f32 / 40.0).min(0.4);
        let (glyph, gcol, _, _) = style(&e.kind);
        if last_year != Some(e.year) {
            fonts::draw(buf, w, h, x as f32, y as f32, &format!("{}", e.year), Face::Italic, PX, 0.0, mix(RUBRIC, PAPER, age * 0.6), None);
            last_year = Some(e.year);
        }
        draw_event_icon(buf, w, h, glyph, mix(gcol, PAPER, age), (x + 48) as f32, (y + 9) as f32, 16.0);
        let ink = if e.kind.is_major() || e.kind == EventType::Authored { INK } else { mix(INK, PAPER, age) };
        for (k, l) in lines.iter().enumerate() {
            fonts::draw(buf, w, h, text_x as f32, (y + k as i64 * ROW) as f32, l, face, PX, if face == Face::SmallCaps { 0.3 } else { 0.0 }, ink, None);
        }
        if let Some(loc) = e.location { hits.push((rect, loc)); }
        y += eh + 2;
    }
    if entries.is_empty() {
        fell(buf, w, h, text_x as i64, y, "The page is still blank.", INK_FADED, 1, false);
    }
    if *scroll > 0 {
        let note = format!("({} newer above - scroll up)", scroll);
        fell(buf, w, h, (x + iw - fell_width(&note, 1)) as i64, (r.y + r.h) as i64 - 14, &note, RUBRIC, 1, false);
    }
    hits
}

/// The chronicle's mark for an event (by `style`'s key), drawn as a small ink icon in its colour:
/// a star for a founding, a flame for a razing, a house for a settlement, crossed blades for war
/// and battle, an olive branch for peace, linked rings for an alliance or a marriage, a crown for
/// a ruler, a sun for faith, a horned skull for a beast, a chest or an obelisk for treasure, a
/// spiral for magic, a tree for the wild, the Shadow's eye, footprints for a journey.
pub(crate) fn draw_event_icon(buf: &mut [u32], w: usize, h: usize, key: char, colour: u32, cx: f32, cy: f32, size: f32) {
    use super::ink::{Finish, Pen};
    let rgb = |c: u32| [((c >> 16) & 255) as f32, ((c >> 8) & 255) as f32, (c & 255) as f32];
    let col = rgb(colour);
    let mut put = |x: i64, y: i64, c: [f32; 3], a: f32| blend_px(buf, w, h, x, y, super::ink::pack(c), a);
    let mut pen = Pen::new(&mut put, cx, cy, size);
    let lw = (size * 0.08).max(1.0);
    match key {
        '*' => { let pts: Vec<(f32, f32)> = (0..10).map(|k| { let a = k as f32 * 0.6283 - 1.5708; let r = if k % 2 == 0 { 0.85 } else { 0.38 }; (a.cos() * r, a.sin() * r) }).collect(); pen.poly(&pts, col); }
        'X' | '#' => {
            if key == '#' { pen.rect(-0.55, 0.0, 0.55, 0.75, [176.0, 160.0, 140.0]); pen.poly(&[(-0.7, 0.05), (0.0, -0.45), (0.7, 0.05)], [150.0, 130.0, 110.0]); }
            pen.poly(&[(-0.4, 0.75), (-0.5, 0.1), (-0.15, -0.3), (-0.05, -0.85), (0.25, -0.25), (0.5, 0.1), (0.4, 0.75)], col);
            pen.poly_f(&[(-0.18, 0.7), (0.0, 0.0), (0.18, 0.7)], [250.0, 214.0, 120.0], Finish::Paint);
        }
        'o' => { pen.rect(-0.55, -0.05, 0.55, 0.75, col); pen.poly(&[(-0.75, 0.0), (0.0, -0.75), (0.75, 0.0)], super::ink::mix(col, [250.0, 244.0, 230.0], 0.3)); }
        '!' | 'x' => {
            if colour == VIOLET || (key == '!' && col[2] > col[0]) {
                pen.poly(&[(0.15, -0.9), (-0.45, 0.1), (0.0, 0.1), (-0.2, 0.9), (0.5, -0.2), (0.05, -0.2), (0.3, -0.9)], col);
            } else {
                pen.bone(&[(-0.75, 0.75), (0.7, -0.7)], [200.0, 202.0, 210.0], lw * 1.5);
                pen.bone(&[(0.75, 0.75), (-0.7, -0.7)], [200.0, 202.0, 210.0], lw * 1.5);
                pen.line((-0.75, 0.75), (-0.45, 0.45), col, lw * 2.0);
                pen.line((0.75, 0.75), (0.45, 0.45), col, lw * 2.0);
            }
        }
        '=' => { pen.path(&[(-0.7, 0.7), (0.0, 0.0), (0.7, -0.7)], [110.0, 120.0, 70.0], lw * 1.5); for k in 0..4 { let t = -0.45 + k as f32 * 0.35; pen.ellipse_rot(t + 0.15, -t - 0.1, 0.22, 0.1, -0.8, [130.0, 150.0, 80.0]); } }
        '&' => { pen.shape(col, Finish::Plain, [-0.9, -0.6, 0.9, 0.6], &|u, v| { let a = ((u + 0.3).powi(2) + v * v).sqrt(); let b = ((u - 0.3).powi(2) + v * v).sqrt(); (a - 0.45).abs() < 0.13 || (b - 0.45).abs() < 0.13 }); }
        '^' => { pen.poly(&[(-0.75, 0.55), (-0.75, -0.3), (-0.38, 0.1), (0.0, -0.6), (0.38, 0.1), (0.75, -0.3), (0.75, 0.55)], col); }
        '+' => { pen.ellipse(0.0, 0.0, 0.38, 0.38, col); for k in 0..8 { let a = k as f32 * 0.785; pen.line((a.cos() * 0.5, a.sin() * 0.5), (a.cos() * 0.85, a.sin() * 0.85), col, lw); } }
        '~' => {
            pen.ellipse(0.0, 0.05, 0.45, 0.5, [232.0, 224.0, 204.0]);
            for u in [-0.17f32, 0.17] { pen.ellipse_f(u, 0.0, 0.11, 0.13, super::ink::INK, Finish::Paint); }
            pen.bone(&[(-0.35, -0.3), (-0.7, -0.8)], col, lw * 1.4);
            pen.bone(&[(0.35, -0.3), (0.7, -0.8)], col, lw * 1.4);
        }
        '$' => { pen.rect(-0.7, -0.1, 0.7, 0.65, [150.0, 104.0, 64.0]); pen.poly(&[(-0.7, -0.1), (-0.55, -0.55), (0.55, -0.55), (0.7, -0.1)], col); pen.rect_f(-0.12, 0.05, 0.12, 0.3, col, Finish::Plain); }
        '%' => { let pts: Vec<(f32, f32)> = (0..16).map(|k| { let a = k as f32 * 0.7; let r = 0.08 + k as f32 * 0.05; (a.cos() * r, a.sin() * r) }).collect(); pen.path(&pts, col, lw * 1.3); }
        '"' => { pen.rect(-0.08, 0.2, 0.08, 0.85, [120.0, 86.0, 54.0]); pen.ellipse(0.0, -0.15, 0.55, 0.5, [100.0, 136.0, 76.0]); }
        '@' => {
            if colour == GOLD { pen.poly(&[(0.7, -0.8), (-0.4, 0.5), (-0.55, 0.75), (-0.3, 0.6), (0.8, -0.7)], [236.0, 226.0, 200.0]); pen.line((-0.4, 0.5), (-0.6, 0.8), super::ink::INK, lw); }
            else { pen.ellipse(0.0, 0.0, 0.8, 0.42, [236.0, 226.0, 200.0]); pen.ellipse_f(0.0, 0.0, 0.3, 0.35, col, Finish::Plain); pen.ellipse_f(0.0, 0.0, 0.08, 0.3, super::ink::INK, Finish::Paint); }
        }
        '>' => { for (u, v) in [(-0.35f32, 0.35f32), (0.3, -0.3)] { pen.ellipse_f(u, v, 0.16, 0.24, col, Finish::Paint); pen.ellipse_f(u, v - 0.32, 0.1, 0.08, col, Finish::Paint); } }
        _ => { pen.ellipse(0.0, -0.45, 0.22, 0.22, col); pen.poly(&[(-0.35, 0.8), (-0.25, -0.15), (0.25, -0.15), (0.35, 0.8)], col); }
    }
}

/// The watcher's panel lettering in IM Fell (it had been the 8x8 bitmap font): scale 1 = 14 px
/// roman (bold: small caps), 2 = 21 px small caps, 3 = 30 px small caps; `y` as the bitmap's top.
fn fell(buf: &mut [u32], w: usize, h: usize, x: i64, y: i64, text: &str, color: u32, scale: usize, bold: bool) {
    let (face, px, track) = fell_face(scale, bold);
    fonts::draw(buf, w, h, x as f32, y as f32 - 3.0, text, face, px, track, color, None);
}

fn fell_face(scale: usize, bold: bool) -> (Face, f32, f32) {
    match scale { 0 | 1 => if bold { (Face::SmallCaps, 14.0, 0.3) } else { (Face::Roman, 14.0, 0.0) }, 2 => (Face::SmallCaps, 21.0, 0.6), _ => (Face::SmallCaps, 30.0, 0.8) }
}

fn fell_width(text: &str, scale: usize) -> usize {
    let (face, px, track) = fell_face(scale, false);
    fonts::width(text, face, px, track).ceil() as usize
}

/// A section heading in rubric small capitals, words capitalised, with a rule after it.
fn fell_heading(buf: &mut [u32], w: usize, h: usize, x: usize, y: i64, width: usize, text: &str) {
    let title: String = text.split(' ').map(|wd| { let l = wd.to_lowercase(); let mut c = l.chars(); match c.next() { Some(f) => f.to_uppercase().collect::<String>() + c.as_str(), None => String::new() } }).collect::<Vec<_>>().join(" ");
    let title = title.replace(" Of ", " of ").replace(" The ", " the ");
    fonts::draw(buf, w, h, x as f32, y as f32 - 4.0, &title, Face::SmallCaps, 15.0, 0.5, RUBRIC, None);
    let tx = x + fonts::width(&title, Face::SmallCaps, 15.0, 0.5).ceil() as usize + 6;
    if tx < x + width { let ry = (y + 5) as usize; if ry < h { hline(buf, w, tx, x + width, ry, INK_FADED); } }
}
