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
use super::classify::{faction_color, TileWorld};
use super::render::{render_world, screen_to_world, Camera};
use super::text::{draw_ink, place_labels, text_width, Label};

// Palette: the ink map on a dark desk.
const DESK: u32 = 0x0026_201B;
const PAPER: u32 = 0x00EA_DEC4;
const PAPER_SHADE: u32 = 0x00DC_CCA8;
const INK: u32 = 0x0038_2A20;
const INK_FADED: u32 = 0x0080_6A52;
const RUBRIC: u32 = 0x009A_2A1E;
const GOLD: u32 = 0x00A8_7A26;
const SEA: u32 = 0x0030_5670;
const MOSS: u32 = 0x004E_6A30;
const VIOLET: u32 = 0x0064_3A6E;
const ROAD_GLOW: u32 = 0x00F0_B040;

const PANEL_W: usize = 340;
const MARGIN: usize = 12;
const LINE: i64 = 12;
/// Chronicle entries kept (newest first).
const LOG_CAP: usize = 3000;
/// Pace: minimum seconds per season, slowest first; the last is as fast as the simulation goes.
const PACES: [(f32, &str); 5] = [(1.0, "a season a breath"), (0.4, "unhurried"), (0.15, "brisk"), (0.05, "swift"), (0.0, "headlong")];
const DEFAULT_PACE: usize = 4;

/// Controls shared with the simulation thread.
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
}

struct Frame {
    tw: TileWorld,
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
}

enum Msg {
    Status(String),
    Frame(Box<Frame>),
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
            let (base, ctl) = (base.clone(), &ctl);
            scope.spawn(move || simulate(world, game_data, config, engine, base, atlas, ctl, tx))
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

#[allow(clippy::too_many_arguments)]
fn simulate(
    world: &WorldData,
    game_data: &GameData,
    config: HistoryConfig,
    mut engine: HistoryEngine,
    base: TileWorld,
    atlas: &Atlas,
    ctl: &Control,
    tx: mpsc::Sender<Msg>,
) -> WorldHistory {
    let total = config.total_steps();
    let civs = config.initial_civilizations;
    let _ = tx.send(Msg::Status(format!("Recalling the ages before memory: {civs} peoples, their kings and their gods...")));
    let mut history = engine.begin(world, config, game_data);
    let (w, h) = (world.width, world.height);
    let mut roads: Vec<bool> = (0..w * h).map(|i| history.tile_history.has_road(i % w, i / w)).collect();
    let mut seen = history.chronicle.len();
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
    let mut first = frame(&history, world, &base, atlas, 0, total, Vec::new(), Vec::new(), Season::Summer);
    first.events.push(dawn);
    let _ = tx.send(Msg::Frame(Box::new(first)));

    for step in 0..total {
        while ctl.paused.load(Ordering::Relaxed) && !ctl.detached.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(30));
        }
        let t0 = Instant::now();
        engine.step(&mut history, world, game_data);
        if ctl.detached.load(Ordering::Relaxed) {
            if step % 40 == 0 { eprintln!("  History: year {}", history.current_date.year); }
            continue;
        }
        let events: Vec<LogItem> = history.chronicle.events[seen..].iter().map(|e| LogItem {
            year: e.date.year,
            kind: e.event_type.clone(),
            title: e.title.clone(),
            location: e.location,
            key: style(&e.event_type).3 || e.is_major,
        }).collect();
        seen = history.chronicle.len();
        let mut new_roads = Vec::new();
        for (i, r) in roads.iter_mut().enumerate() {
            if !*r && history.tile_history.has_road(i % w, i / w) {
                *r = true;
                new_roads.push((i % w, i / w));
            }
        }
        let pace = ctl.pace.load(Ordering::Relaxed);
        // Seasons show only when watching slowly enough to read them (otherwise they strobe).
        let season = if pace <= 1 { history.current_date.season } else { Season::Summer };
        let f = frame(&history, world, &base, atlas, step + 1, total, events, new_roads, season);
        let _ = tx.send(Msg::Frame(Box::new(f)));
        let min = Duration::from_secs_f32(PACES[pace.min(PACES.len() - 1)].0);
        let spent = t0.elapsed();
        if spent < min && !ctl.detached.load(Ordering::Relaxed) {
            std::thread::sleep(min - spent);
        }
    }
    let _ = tx.send(Msg::Status("Naming the ages...".into()));
    engine.finish(&mut history);
    let _ = tx.send(Msg::Done);
    history
}

#[allow(clippy::too_many_arguments)]
fn frame(
    history: &WorldHistory,
    world: &WorldData,
    base: &TileWorld,
    atlas: &Atlas,
    step: u32,
    total: u32,
    events: Vec<LogItem>,
    new_roads: Vec<(usize, usize)>,
    season: Season,
) -> Frame {
    let mut tw = base.clone();
    tw.apply_history(world, history, atlas);
    tw.set_season(world, season);
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
                population: 0,
                towns: 0,
            });
            r.population += s.population as u64;
            r.towns += 1;
        }
    }
    stats.peoples = history.active_faction_count();
    stats.fallen = history.factions.len() - stats.peoples;
    stats.roads = tw.road.iter().filter(|&&r| r != 0).count();
    stats.wars = history.wars.values().filter(|w| w.is_active()).count();
    stats.beasts = history.living_legendary_count();
    stats.artifacts = history.artifacts.len();
    stats.monuments = history.monuments.len();
    let mut realms: Vec<Realm> = realms.into_values().collect();
    realms.sort_by_key(|r| std::cmp::Reverse(r.population));
    Frame {
        tw,
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
    }
}

// ---------------------------------------------------------------------------------------------
// Drawing helpers
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Default)]
struct Rect { x: usize, y: usize, w: usize, h: usize }

impl Rect {
    fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x as f32 && py >= self.y as f32 && px < (self.x + self.w) as f32 && py < (self.y + self.h) as f32
    }
}

fn mix(a: u32, b: u32, t: f32) -> u32 {
    let t = t.clamp(0.0, 1.0);
    let ch = |s: u32| {
        let (x, y) = (((a >> s) & 0xFF) as f32, ((b >> s) & 0xFF) as f32);
        ((x + (y - x) * t) as u32) << s
    };
    ch(16) | ch(8) | ch(0)
}

fn hash(x: usize, y: usize) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B1) ^ (y as u32).wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^ (h >> 12)
}

fn fill(buf: &mut [u32], w: usize, r: Rect, color: u32) {
    for y in r.y..r.y + r.h {
        buf[y * w + r.x..y * w + r.x + r.w].fill(color);
    }
}

fn blend_px(buf: &mut [u32], w: usize, h: usize, x: i64, y: i64, color: u32, a: f32) {
    if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h {
        let k = y as usize * w + x as usize;
        buf[k] = mix(buf[k], color, a);
    }
}

fn hline(buf: &mut [u32], w: usize, x0: usize, x1: usize, y: usize, color: u32) {
    buf[y * w + x0..y * w + x1].fill(color);
}

fn outline(buf: &mut [u32], w: usize, r: Rect, color: u32) {
    hline(buf, w, r.x, r.x + r.w, r.y, color);
    hline(buf, w, r.x, r.x + r.w, r.y + r.h - 1, color);
    for y in r.y..r.y + r.h {
        buf[y * w + r.x] = color;
        buf[y * w + r.x + r.w - 1] = color;
    }
}

/// A parchment card with a mottled wash and a double ink rule, like the map's own frame.
fn card(buf: &mut [u32], w: usize, r: Rect) {
    for y in r.y..r.y + r.h {
        for x in r.x..r.x + r.w {
            let n = (hash(x / 3, y / 3) & 0xFF) as f32 / 255.0;
            let edge = ((x - r.x).min(r.x + r.w - 1 - x).min(y - r.y).min(r.y + r.h - 1 - y)) as f32;
            let foxing = (1.0 - edge / 18.0).max(0.0) * 0.18;
            buf[y * w + x] = mix(mix(PAPER, PAPER_SHADE, 0.35 * n), 0x00B8_9A6A, foxing);
        }
    }
    outline(buf, w, r, INK);
    outline(buf, w, Rect { x: r.x + 3, y: r.y + 3, w: r.w - 6, h: r.h - 6 }, INK_FADED);
}

/// Small-caps style heading with a rule under it.
fn heading(buf: &mut [u32], w: usize, h: usize, x: usize, y: i64, width: usize, text: &str) {
    draw_ink(buf, w, h, x as i64, y, text, RUBRIC, 1, true);
    let tx = x + text_width(text, 1) + 6;
    if tx < x + width {
        let ry = (y + 4) as usize;
        if ry < h { hline(buf, w, tx, x + width, ry, INK_FADED); }
    }
}

/// Greedy word wrap to `max_chars` per line.
fn wrap(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > max_chars {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() { line.push(' '); }
        line.push_str(word);
    }
    if !line.is_empty() { lines.push(line); }
    lines
}

/// Fold accented letters to ASCII for the 8x8 font.
fn ascii(s: &str) -> String {
    s.chars().map(|c| match c {
        'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'ö' | 'õ' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'Á' | 'À' | 'Â' | 'Ä' => 'A',
        'É' | 'È' => 'E',
        'Ó' | 'Ö' => 'O',
        '’' | '‘' => '\'',
        '—' | '–' => '-',
        c => c,
    }).collect()
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars { s.to_string() } else { format!("{}.", s.chars().take(max_chars.saturating_sub(1)).collect::<String>()) }
}

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
    let scale = tile_px.clamp(2.0, 12.0) / 3.0;
    match m.kind {
        MarkKind::Road => {
            let r = (tile_px * 0.45).max(1.6);
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
    let tw = lines.iter().map(|l| text_width(l, scale)).max().unwrap_or(0);
    let bh = lines.len() * 10 * scale + 16;
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
            let lx = bx + (bw - text_width(l, scale)) / 2;
            draw_ink(buf, w, h, lx as i64, (by + 8 + i * 10 * scale) as i64, l, RUBRIC, scale, false);
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

/// Everything the watcher shows, apart from the window: fed frames from the simulation, drawn
/// into a pixel buffer. The window (or a headless snapshot) drives it.
struct View<'a> {
    world: &'a WorldData,
    atlas: &'a Atlas,
    tw: TileWorld,
    latest: Option<Box<Frame>>,
    log: VecDeque<LogItem>,
    marks: Vec<Mark>,
    banner_msg: Option<(String, Instant)>,
    souls_hist: Vec<u64>,
    status: String,
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
}

impl<'a> View<'a> {
    fn new(world: &'a WorldData, atlas: &'a Atlas, base: TileWorld) -> Self {
        let lay = layout(1440, 900, world);
        let cam = fit_camera(world, lay.map);
        View {
            world, atlas, tw: base, latest: None, log: VecDeque::new(), marks: Vec::new(),
            banner_msg: None, souls_hist: Vec::new(), status: "Raising the land...".into(),
            done: false, show_all: false, log_scroll: 0, size: (0, 0), buf: Vec::new(),
            bg: Vec::new(), map_buf: Vec::new(), map_dirty: true, lay, cam, fitted: true,
            last_render: Instant::now() - Duration::from_secs(1), entry_hits: Vec::new(),
        }
    }

    fn receive(&mut self, msg: Msg) {
        match msg {
            Msg::Status(s) => self.status = s,
            Msg::Done => {
                self.done = true;
                self.status = "The age is written.".into();
                self.banner_msg = Some(("The age is written. Press Enter to walk the world.".into(), Instant::now()));
            }
            Msg::Frame(f) => {
                let now = Instant::now();
                for &(x, y) in &f.new_roads {
                    self.marks.push(Mark { x, y, kind: MarkKind::Road, born: now });
                }
                for e in &f.events {
                    if let (Some(kind), Some((x, y))) = (style(&e.kind).2, e.location) {
                        self.marks.push(Mark { x, y, kind, born: now });
                    }
                    // Someone reading older entries keeps their place as new ones arrive.
                    if self.log_scroll > 0 && (e.key || self.show_all) { self.log_scroll += 1; }
                    let great = e.kind.is_major() || matches!(e.kind, EventType::Authored | EventType::SettlementDestroyed | EventType::LandScarred);
                    if great && f.step > 0 { self.banner_msg = Some((ascii(&e.title), now)); }
                    self.log.push_front(e.clone());
                }
                self.log.truncate(LOG_CAP);
                if f.step == 0 {
                    // The peoples of the dawn appear all at once.
                    for (i, s) in f.tw.settlement.iter().enumerate() {
                        if s.is_some() { self.marks.push(Mark { x: i % f.tw.width, y: i / f.tw.width, kind: MarkKind::Founded, born: now }); }
                    }
                }
                // Too many glowing roads at once only blurs; keep the most recent.
                if self.marks.len() > 4000 { let n = self.marks.len() - 4000; self.marks.drain(..n); }
                self.souls_hist.push(f.stats.souls);
                self.status = if f.step == 0 { "The first year dawns.".into() } else { "Writing the chronicle...".into() };
                self.tw = f.tw.clone();
                self.latest = Some(f);
                self.map_dirty = true;
            }
        }
    }

    fn resize(&mut self, w: usize, h: usize) {
        if (w, h) == self.size || w < 200 || h < 200 { return; }
        self.size = (w, h);
        self.lay = layout(w, h, self.world);
        if self.fitted { self.cam = fit_camera(self.world, self.lay.map); }
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

    /// Zoom the map by `steps` around a screen point.
    fn zoom(&mut self, steps: f32, at: (f32, f32)) {
        let map = self.lay.map;
        let (mx, my) = (at.0 - map.x as f32, at.1 - map.y as f32);
        let before = screen_to_world(&self.cam, mx, my, map.w, map.h);
        self.cam.tile_px = (self.cam.tile_px * 1.2f32.powf(steps)).clamp(1.0, 32.0);
        let after = screen_to_world(&self.cam, mx, my, map.w, map.h);
        self.cam.cx += before.0 - after.0;
        self.cam.cy += before.1 - after.1;
        self.fitted = false;
        self.map_dirty = true;
    }

    fn look_at(&mut self, x: usize, y: usize, tile_px: f32) {
        self.cam = Camera { cx: x as f32 + 0.5, cy: y as f32 + 0.5, tile_px };
        self.fitted = false;
        self.map_dirty = true;
    }

    /// Draw the whole window. `immediate` re-renders the map now (while dragging or zooming);
    /// otherwise a changing world re-renders it at most ~8 times a second.
    fn draw(&mut self, mouse: (f32, f32), hovering: bool, immediate: bool, ctl: &Control) {
        let (w, h) = self.size;
        let world = self.world;
        self.cam.cx = self.cam.cx.rem_euclid(world.width as f32);
        let lay = Layout { map: self.lay.map, log: self.lay.log, panel: self.lay.panel };
        let now = Instant::now();
        if self.map_dirty && (immediate || now.duration_since(self.last_render) > Duration::from_millis(120)) {
            let cam = self.cam;
            render_world(&self.tw, self.atlas, &cam, &mut self.map_buf, lay.map.w, lay.map.h);
            overlay_realms(&self.tw, &cam, &mut self.map_buf, lay.map.w, lay.map.h);
            if let Some(f) = &self.latest {
                let labels = settlement_labels(f);
                let ww = world.width as f32;
                let (mw, mh) = (lay.map.w, lay.map.h);
                place_labels(&labels, cam.tile_px, mw, mh, &mut self.map_buf, |x, y| {
                    let mut dx = x - cam.cx;
                    if dx > ww / 2.0 { dx -= ww; }
                    if dx < -ww / 2.0 { dx += ww; }
                    (mw as f32 / 2.0 + dx * cam.tile_px, mh as f32 / 2.0 + (y - cam.cy) * cam.tile_px)
                });
            }
            self.map_dirty = false;
            self.last_render = now;
        }

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
            if let Some(f) = &self.latest {
                let (wx, wy) = screen_to_world(&self.cam, mouse.0 - lay.map.x as f32, mouse.1 - lay.map.y as f32, lay.map.w, lay.map.h);
                if wy >= 0.0 && (wy as usize) < world.height {
                    let (tx, ty) = ((wx.floor() as i64).rem_euclid(world.width as i64) as usize, wy as usize);
                    let text = hover_text(f, &self.tw, tx, ty);
                    if !text.is_empty() {
                        let r = Rect { x: lay.map.x + 8, y: lay.map.y + lay.map.h - 22, w: (text_width(&text, 1) + 12).min(lay.map.w - 16), h: 15 };
                        fill(buf, w, r, PAPER);
                        outline(buf, w, r, INK);
                        draw_ink(buf, w, h, r.x as i64 + 6, r.y as i64 + 4, &truncate(&text, (r.w - 12) / 7), INK, 1, false);
                    }
                }
            }
        }

        // Banner for great events (the closing one stays).
        if let Some((text, born)) = &self.banner_msg {
            let age = now.saturating_duration_since(*born).as_secs_f32();
            let alpha = if self.done { 1.0 } else { (1.0 - (age - 4.0) / 0.8).clamp(0.0, 1.0) };
            if alpha > 0.0 { banner(buf, w, h, lay.map, text, alpha); } else { self.banner_msg = None; }
        }

        draw_panel(buf, w, h, lay.panel, self.latest.as_deref(), &self.souls_hist, &self.status, ctl, self.done);
        self.entry_hits = draw_log(buf, w, h, lay.log, &self.log, self.show_all, &mut self.log_scroll, mouse);
    }
}

fn run_window(mut window: Window, world: &WorldData, atlas: &Atlas, base: TileWorld, ctl: &Control, rx: mpsc::Receiver<Msg>) -> Result<(), Box<dyn Error>> {
    window.set_target_fps(60);
    let mut view = View::new(world, atlas, base);
    let mut drag: Option<((f32, f32), (f32, f32))> = None;
    let mut was_down = false;

    while window.is_open() {
        while let Ok(msg) = rx.try_recv() { view.receive(msg); }
        let (w, h) = window.get_size();
        view.resize(w, h);
        if view.size.0 == 0 { window.update(); continue; }

        let mouse = window.get_mouse_pos(MouseMode::Clamp).unwrap_or((0.0, 0.0));
        let wheel = window.get_scroll_wheel().map(|s| s.1).unwrap_or(0.0);
        let down = window.get_mouse_down(MouseButton::Left);
        let clicked = down && !was_down;
        was_down = down;
        let pressed = |k: Key| window.is_key_pressed(k, KeyRepeat::No);
        if pressed(Key::Escape) && !view.done { break; }
        if view.done && (pressed(Key::Enter) || pressed(Key::Escape) || pressed(Key::Q)) { break; }
        if pressed(Key::Space) {
            let p = !ctl.paused.load(Ordering::Relaxed);
            ctl.paused.store(p, Ordering::Relaxed);
        }
        if pressed(Key::LeftBracket) || pressed(Key::Minus) {
            let p = ctl.pace.load(Ordering::Relaxed);
            ctl.pace.store(p.saturating_sub(1), Ordering::Relaxed);
        }
        if pressed(Key::RightBracket) || pressed(Key::Equal) {
            let p = ctl.pace.load(Ordering::Relaxed);
            ctl.pace.store((p + 1).min(PACES.len() - 1), Ordering::Relaxed);
        }
        if pressed(Key::L) { view.show_all = !view.show_all; view.log_scroll = 0; }
        if pressed(Key::H) || pressed(Key::Home) {
            view.cam = fit_camera(world, view.lay.map);
            view.fitted = true;
            view.map_dirty = true;
        }
        if pressed(Key::P) {
            let path = format!("watch_{}_{}.png", world.seed(), view.latest.as_ref().map(|f| f.year).unwrap_or(0));
            view.status = save_png(&path, &view.buf, w, h);
        }
        let over_map = view.lay.map.contains(mouse.0, mouse.1);
        if wheel != 0.0 && over_map { view.zoom(wheel.signum(), mouse); }
        if wheel != 0.0 && view.lay.log.contains(mouse.0, mouse.1) {
            view.log_scroll = if wheel > 0.0 { view.log_scroll.saturating_sub(3) } else { view.log_scroll + 3 };
        }
        match (down, drag) {
            (true, None) if over_map => drag = Some((mouse, (view.cam.cx, view.cam.cy))),
            (true, Some((start, c0))) => {
                if (mouse.0 - start.0).abs() + (mouse.1 - start.1).abs() > 1.0 {
                    view.cam.cx = c0.0 - (mouse.0 - start.0) / view.cam.tile_px;
                    view.cam.cy = (c0.1 - (mouse.1 - start.1) / view.cam.tile_px).clamp(0.0, world.height as f32);
                    view.fitted = false;
                    view.map_dirty = true;
                }
            }
            (false, _) => drag = None,
            _ => {}
        }
        if clicked {
            if let Some(&(_, (x, y))) = view.entry_hits.iter().find(|(r, _)| r.contains(mouse.0, mouse.1)) {
                view.look_at(x, y, 10.0);
            }
        }

        view.draw(mouse, drag.is_none(), drag.is_some() || wheel != 0.0, ctl);
        window.update_with_buffer(&view.buf, w, h)?;
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

/// Simulate `config`'s history without a window and save watcher frames as
/// `<prefix>_y<year>.png` at a few points (a quarter, half, the end) plus a close-up of the
/// busiest place, for checking the watcher's look headlessly.
pub fn watch_snapshots(world: &WorldData, game_data: &GameData, config: HistoryConfig, engine: HistoryEngine, atlas: &Atlas, prefix: &str) -> (WorldHistory, Vec<String>) {
    let mut base = TileWorld::build(world, atlas);
    base.set_season(world, Season::Summer);
    let total = config.total_steps();
    let ctl = Control { paused: AtomicBool::new(false), pace: AtomicUsize::new(PACES.len() - 1), detached: AtomicBool::new(false) };
    let (tx, rx) = mpsc::channel();
    let mut files = Vec::new();
    let history = std::thread::scope(|scope| {
        let sim = {
            let (base, ctl) = (base.clone(), &ctl);
            scope.spawn(move || simulate(world, game_data, config, engine, base, atlas, ctl, tx))
        };
        let mut view = View::new(world, atlas, base);
        view.resize(1440, 900);
        let shots = [total / 4, total / 2, total];
        for msg in rx.iter() {
            let step = match &msg { Msg::Frame(f) => Some(f.step), _ => None };
            let done = matches!(msg, Msg::Done);
            view.receive(msg);
            if step.map(|s| shots[..2].contains(&s)).unwrap_or(false) || done {
                view.draw((0.0, 0.0), false, true, &ctl);
                let path = format!("{prefix}_y{}.png", view.latest.as_ref().map(|f| f.year).unwrap_or(0));
                save_png(&path, &view.buf, 1440, 900);
                files.push(path);
            }
            if done {
                // A close-up of the most recent event with a place, with the chronicle as is.
                if let Some((x, y)) = view.log.iter().find_map(|e| e.location) {
                    view.look_at(x, y, 10.0);
                    view.draw((0.0, 0.0), false, true, &ctl);
                    let path = format!("{prefix}_closeup.png");
                    save_png(&path, &view.buf, 1440, 900);
                    files.push(path);
                }
            }
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
        let (x0, y0) = screen_to_world(cam, 0.0, 0.0, w, h);
        let (x1, y1) = screen_to_world(cam, w as f32, h as f32, w, h);
        let road = mix(0x0088_5C3C, INK, 0.2);
        for ty in (y0.floor().max(0.0) as usize)..(y1.ceil().max(0.0) as usize).min(tw.height) {
            for txi in x0.floor() as i64..=x1.ceil() as i64 {
                let tx = txi.rem_euclid(tw.width as i64) as usize;
                let mask = tw.road[ty * tw.width + tx];
                if mask == 0 { continue; }
                let sx = w as f32 / 2.0 + (txi as f32 + 0.5 - cam.cx) * t;
                let sy = h as f32 / 2.0 + (ty as f32 + 0.5 - cam.cy) * t;
                // Each link once: N, NE, E, SE (the others are their neighbours' links).
                for (b, (dx, dy)) in super::classify::DIRS.iter().enumerate().take(4) {
                    if mask & (1 << b) == 0 { continue; }
                    let steps = (t.ceil() as i64).max(1);
                    for k in 0..=steps {
                        let f = k as f32 / steps as f32;
                        blend_px(buf, w, h, (sx + *dx as f32 * t * f) as i64, (sy + *dy as f32 * t * f) as i64, road, 0.9);
                    }
                }
            }
        }
    }
}

/// Labels for the larger living settlements (capitals first).
fn settlement_labels(f: &Frame) -> Vec<Label> {
    let mut by_id: HashMap<u64, (usize, usize)> = HashMap::new();
    for (i, s) in f.tw.settlement.iter().enumerate() {
        if let Some(id) = s { by_id.insert(id.0, (i % f.tw.width, i / f.tw.width)); }
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
        Some(Label { x: x as f32 + 0.5, y: y as f32 + 1.6, text: ascii(name), rank: rank + (*pop / 2000).min(99), min_tile_px: min_px, color: INK })
    }).collect();
    labels.sort_by_key(|l| std::cmp::Reverse(l.rank));
    labels
}

fn hover_text(f: &Frame, tw: &TileWorld, x: usize, y: usize) -> String {
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
fn draw_panel(buf: &mut [u32], w: usize, h: usize, p: Rect, f: Option<&Frame>, souls_hist: &[u64], status: &str, ctl: &Control, done: bool) {
    let x = p.x + 16;
    let iw = p.w - 32;
    let mut y = p.y as i64 + 16;
    draw_ink(buf, w, h, x as i64, y, "THE WORLD", RUBRIC, 2, true);
    y += 18;
    draw_ink(buf, w, h, x as i64, y, "TAKES SHAPE", RUBRIC, 2, true);
    y += 24;
    hline(buf, w, x, x + iw, y as usize, INK);
    hline(buf, w, x, x + iw, y as usize + 2, INK_FADED);
    y += 12;

    let Some(f) = f else {
        for line in wrap(status, iw / 7) {
            draw_ink(buf, w, h, x as i64, y, &line, INK, 1, false);
            y += LINE;
        }
        return;
    };

    // Year and season, large.
    let year = format!("Year {}", f.year);
    draw_ink(buf, w, h, x as i64, y, &year, INK, 3, true);
    y += 28;
    draw_ink(buf, w, h, x as i64, y, season_name(f.season), INK_FADED, 2, false);
    y += 22;
    // Progress through the age, with a tick every 50 years.
    let bar = Rect { x, y: y as usize, w: iw, h: 9 };
    outline(buf, w, bar, INK);
    let frac = if f.total > 0 { f.step as f32 / f.total as f32 } else { 1.0 };
    let filled = ((bar.w - 4) as f32 * frac) as usize;
    if filled > 0 { fill(buf, w, Rect { x: bar.x + 2, y: bar.y + 2, w: filled, h: bar.h - 4 }, mix(INK, GOLD, 0.6)); }
    let years = f.total / 4;
    for k in (50..years).step_by(50) {
        let tx = bar.x + ((bar.w as f32) * k as f32 / years.max(1) as f32) as usize;
        for ty in bar.y + bar.h..bar.y + bar.h + 3 { buf[ty * w + tx] = INK; }
    }
    y += 14;
    draw_ink(buf, w, h, x as i64, y, &format!("{} of {} years", f.step / 4, years), INK_FADED, 1, false);
    let paused = ctl.paused.load(Ordering::Relaxed);
    let state = if done { "complete" } else if paused { "- paused -" } else { PACES[ctl.pace.load(Ordering::Relaxed).min(PACES.len() - 1)].1 };
    let sc = if paused { RUBRIC } else { INK_FADED };
    draw_ink(buf, w, h, (x + iw - text_width(state, 1)) as i64, y, state, sc, 1, paused);
    y += LINE + 4;
    if !done && !paused {
        draw_ink(buf, w, h, x as i64, y, &truncate(status, iw / 7), INK_FADED, 1, false);
    }
    y += LINE + 6;

    // Almanac.
    heading(buf, w, h, x, y, iw, "ALMANAC");
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
        draw_ink(buf, w, h, x as i64, y, label, INK, 1, false);
        let vx = x + 140;
        draw_ink(buf, w, h, vx as i64, y, &value, if label == "Wars raging" && s.wars > 0 { RUBRIC } else { INK }, 1, true);
        if !note.is_empty() {
            draw_ink(buf, w, h, (x + iw - text_width(&note, 1)) as i64, y, &note, INK_FADED, 1, false);
        }
        y += LINE + 1;
    }
    y += 6;

    // Souls over the age, as an ink line over a pale wash.
    heading(buf, w, h, x, y, iw, &format!("SOULS  {}", short_num(s.souls)));
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

    // The great realms.
    heading(buf, w, h, x, y, iw, "GREAT REALMS");
    y += 16;
    let top = f.realms.first().map(|r| r.population).unwrap_or(1).max(1);
    let footer_top = (p.y + p.h) as i64 - 66;
    for r in &f.realms {
        if y + 22 > footer_top { break; }
        let sw = Rect { x, y: y as usize, w: 10, h: 10 };
        fill(buf, w, sw, faction_color(r.id));
        outline(buf, w, sw, INK);
        let pop = format!("{}  {}", r.towns, short_num(r.population));
        let name_chars = (iw - 18 - text_width(&pop, 1) - 8) / 7;
        draw_ink(buf, w, h, (x + 16) as i64, y + 1, &truncate(&ascii(&r.name), name_chars), INK, 1, false);
        draw_ink(buf, w, h, (x + iw - text_width(&pop, 1)) as i64, y + 1, &pop, INK_FADED, 1, false);
        let bw = ((iw - 16) as f32 * r.population as f32 / top as f32) as usize;
        if bw > 0 { hline(buf, w, x + 16, x + 16 + bw, (y + 12) as usize, mix(faction_color(r.id), INK, 0.2)); }
        y += 18;
    }

    // Controls.
    let mut fy = footer_top + 6;
    hline(buf, w, x, x + iw, fy as usize - 4, INK_FADED);
    for line in [
        "SPACE pause    [ ] pace    L log filter",
        "wheel zoom   drag pan   H fit the map",
        if done { "ENTER walk the world" } else { "click an entry: go there   ESC hurry" },
    ] {
        fy += 2;
        draw_ink(buf, w, h, x as i64, fy, line, INK_FADED, 1, false);
        fy += LINE + 2;
    }
}

/// The chronicle: newest entries first, red year rubrics, glyphs by kind. Returns the clickable
/// rectangles of entries with a place.
fn draw_log(buf: &mut [u32], w: usize, h: usize, r: Rect, log: &VecDeque<LogItem>, show_all: bool, scroll: &mut usize, mouse: (f32, f32)) -> Vec<(Rect, (usize, usize))> {
    let x = r.x + 16;
    let iw = r.w.saturating_sub(32);
    let mut y = r.y as i64 + 14;
    heading(buf, w, h, x, y, iw.saturating_sub(150), "CHRONICLE OF THE AGE");
    let filter = if show_all { "all that happens" } else { "key events" };
    draw_ink(buf, w, h, (x + iw - text_width(filter, 1)) as i64, y, filter, INK_FADED, 1, false);
    y += 18;
    let bottom = (r.y + r.h) as i64 - if *scroll > 0 { 22 } else { 10 };
    let text_x = x + 64;
    let max_chars = (iw.saturating_sub(64)) / 7;
    let mut hits = Vec::new();
    let entries: Vec<&LogItem> = log.iter().filter(|e| show_all || e.key).collect();
    *scroll = (*scroll).min(entries.len().saturating_sub(1));
    let mut last_year = None;
    for (n, e) in entries.iter().skip(*scroll).enumerate() {
        let lines = wrap(&ascii(&e.title), max_chars.max(10));
        let eh = lines.len() as i64 * LINE;
        if y + eh > bottom { break; }
        let rect = Rect { x: x - 4, y: (y - 2) as usize, w: iw + 8, h: eh as usize + 2 };
        let hover = e.location.is_some() && rect.contains(mouse.0, mouse.1);
        if hover { fill(buf, w, rect, PAPER_SHADE); }
        // Older entries fade towards the paper.
        let age = (n as f32 / 40.0).min(0.55);
        let (glyph, gcol, _, _) = style(&e.kind);
        if last_year != Some(e.year) {
            draw_ink(buf, w, h, x as i64, y, &format!("{}", e.year), mix(RUBRIC, PAPER, age * 0.7), 1, true);
            last_year = Some(e.year);
        }
        draw_ink(buf, w, h, (x + 44) as i64, y, &glyph.to_string(), mix(gcol, PAPER, age), 1, true);
        let ink = if e.kind.is_major() || e.kind == EventType::Authored { INK } else { mix(INK, PAPER, age) };
        for (k, l) in lines.iter().enumerate() {
            draw_ink(buf, w, h, text_x as i64, y + k as i64 * LINE, l, ink, 1, e.kind.is_major());
        }
        if let Some(loc) = e.location { hits.push((rect, loc)); }
        y += eh + 2;
    }
    if entries.is_empty() {
        draw_ink(buf, w, h, text_x as i64, y, "The page is still blank.", INK_FADED, 1, false);
    }
    if *scroll > 0 {
        let note = format!("({} newer above - scroll up)", scroll);
        draw_ink(buf, w, h, (x + iw - text_width(&note, 1)) as i64, (r.y + r.h) as i64 - 14, &note, RUBRIC, 1, false);
    }
    hits
}
