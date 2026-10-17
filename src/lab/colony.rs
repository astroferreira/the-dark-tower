//! The lab's colonies: a camp founded where the game founds it (`viewer::colony_site` and
//! `found_colony`, the game's own path), lived minute by minute under a recorder that keeps what
//! the colony itself forgets: every settler's job and why as spans of time, where everyone and
//! everything stood, needs and stress, the camp's stocks, the works rising, opinions, frames of
//! the map; and the oddities a debugger wants (a settler standing where no one can stand, a
//! jump no walk could make). The recording feeds the colony tools (`colony_tools.rs`) and the
//! colony explorer (`colony_explorer.rs`).
//!
//! The recorder only reads the colony between ticks, so a recorded run tells the same story as
//! an unrecorded one: the log (and the colony hash) is the game's, tick for tick.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::colony::{Colony, Job, TICKS_PER_DAY};
use crate::history::world_state::WorldHistory;
use crate::world::WorldData;

/// Job kinds, in `verb_ix` order (the app colours them).
pub const VERBS: [&str; 13] = ["idle", "eating", "sleeping", "foraging", "fishing", "felling", "hauling", "building", "wandering", "quarrying", "digging", "hunting", "crafting"];

pub fn verb_ix(j: &Job) -> u8 {
    match j {
        Job::Idle => 0, Job::Eat => 1, Job::Sleep => 2, Job::Forage(_) => 3, Job::Fish(_) => 4, Job::Fell(_) => 5, Job::Haul(_) => 6,
        Job::Build => 7, Job::Wander(_) => 8, Job::Quarry(_) => 9, Job::Dig(..) => 10, Job::Hunt(_) => 11, Job::Craft => 12,
    }
}

/// Where a job takes its settler, when it names a place (a hauled item's place, a hunted beast's).
pub fn job_target(c: &Colony, j: &Job) -> Option<(i32, i32)> {
    let p = |p: (u16, u16)| Some((p.0 as i32, p.1 as i32));
    match *j {
        Job::Forage(q) | Job::Fish(q) | Job::Fell(q) | Job::Wander(q) | Job::Quarry(q) | Job::Dig(q, _) => p(q),
        Job::Haul(k) => c.items.get(k).and_then(|it| p(it.at)),
        Job::Hunt(id) => c.creatures.iter().find(|x| x.id == id).and_then(|x| p(x.pos)),
        _ => None,
    }
}

/// The colony's series, sampled every `slow` ticks: these names, in this order.
pub const SERIES: [&str; 16] = ["food", "logs", "stone", "alive", "company", "loose", "hostile", "temperature", "darkness", "days_of_food", "food_goal", "readiness", "stage", "works_open", "works_done", "made"];

/// A job held from `t0` to `t1` (exclusive) with its reason.
#[derive(Clone)]
pub struct Span { pub who: usize, pub t0: u64, pub t1: u64, pub verb: u8, pub target: Option<(i32, i32)>, pub why: u32 }

/// A work of the camp as the recorder saw it come and go.
pub struct WorkSeen {
    pub key: (String, (u16, u16), u64),
    pub word: String,
    pub needed: u32,
    pub used: u32,
    pub material: String,
    pub why: String,
    pub dig: bool,
    pub first: u64,
    pub done_at: Option<u64>,
    pub gone_at: Option<u64>,
    /// (tick, loads laid) whenever it changed.
    pub steps: Vec<(u64, u32)>,
}

pub struct Recorder {
    /// Positions every `every` ticks; needs, series and works every `slow`.
    pub every: u64,
    pub slow: u64,
    pub opinion_days: u64,
    pub start_tick: u64,
    pub whys: Vec<String>,
    why_ix: HashMap<String, u32>,
    open: Vec<Option<(Span, Job, String)>>,
    pub spans: Vec<Span>,
    /// (tick, per settler [x, y, z, flags]).
    pub pos: Vec<(u64, Vec<[i32; 4]>)>,
    /// Creatures seen: id -> (kind, name); per position sample [id, x, y, z, leaving].
    pub creature_names: Vec<(u32, String, String)>,
    pub creatures: Vec<Vec<[i32; 5]>>,
    /// (tick, per settler [hunger, fatigue, exposure, stress, focus, stuck] x100 but stuck).
    pub needs: Vec<(u64, Vec<[i32; 6]>)>,
    pub series: Vec<(u64, [f32; 16])>,
    pub works: Vec<WorkSeen>,
    /// (tick, pairs [a, b, opinion] with an opinion).
    pub opinions: Vec<(u64, Vec<[i32; 3]>)>,
    /// (tick, settler, alive now).
    pub status: Vec<(u64, usize, bool)>,
    alive_was: Vec<bool>,
    last_pos: Vec<Option<(i32, i32, u64)>>,
    /// (tick, settler, x, y, z, verb): standing where `nav::standable` says no one can.
    pub unstandable: Vec<(u64, usize, i32, i32, i32, u8)>,
    pub unstandable_count: usize,
    /// (tick, settler, from, to): farther between two samples than any walk.
    pub jumps: Vec<(u64, usize, (i32, i32), (i32, i32))>,
    /// Frames of the camp: (tick, file name in the frames folder).
    pub frames: Vec<(u64, String)>,
    pub frame_days: u64,
    pub ticks_run: u64,
    pub secs: f64,
}

impl Recorder {
    pub fn new(start_tick: u64, every: u64, frame_days: u64) -> Recorder {
        Recorder {
            every: every.max(1), slow: 60, opinion_days: 1, start_tick, whys: Vec::new(), why_ix: HashMap::new(), open: Vec::new(), spans: Vec::new(),
            pos: Vec::new(), creature_names: Vec::new(), creatures: Vec::new(), needs: Vec::new(), series: Vec::new(), works: Vec::new(),
            opinions: Vec::new(), status: Vec::new(), alive_was: Vec::new(), last_pos: Vec::new(), unstandable: Vec::new(), unstandable_count: 0,
            jumps: Vec::new(), frames: Vec::new(), frame_days, ticks_run: 0, secs: 0.0,
        }
    }

    fn why_id(&mut self, why: &str) -> u32 {
        if let Some(&i) = self.why_ix.get(why) { return i; }
        self.whys.push(why.to_string());
        let i = (self.whys.len() - 1) as u32;
        self.why_ix.insert(why.to_string(), i);
        i
    }

    /// Look at the colony after a tick.
    pub fn observe(&mut self, c: &Colony) {
        let t = c.clock.tick;
        let n = c.settlers.len();
        while self.open.len() < n { self.open.push(None); self.alive_was.push(false); self.last_pos.push(None); }
        for i in 0..n {
            let s = &c.settlers[i];
            if s.alive != self.alive_was[i] {
                self.status.push((t, i, s.alive));
                self.alive_was[i] = s.alive;
                self.last_pos[i] = None;
            }
            let changed = match &self.open[i] {
                Some((_, job, why)) => !s.alive || *job != s.job || *why != s.why,
                None => s.alive,
            };
            if changed {
                if let Some((mut sp, _, _)) = self.open[i].take() { sp.t1 = t; self.spans.push(sp); }
                if s.alive {
                    let why = self.why_id(&s.why);
                    let sp = Span { who: i, t0: t, t1: t, verb: verb_ix(&s.job), target: job_target(c, &s.job), why };
                    self.open[i] = Some((sp, s.job, s.why.clone()));
                }
            }
        }
        if t % self.every == 0 { self.sample_positions(c); }
        if t % self.slow == 0 { self.sample_slow(c); }
        if t % TICKS_PER_DAY == 6 * 60 && (t / TICKS_PER_DAY) % self.opinion_days == 0 {
            let mut pairs = Vec::new();
            for a in 0..n { for b in a + 1..n {
                let v = c.opinion(a, b);
                if v != 0 { pairs.push([a as i32, b as i32, v]); }
            } }
            self.opinions.push((t, pairs));
        }
        self.ticks_run += 1;
    }

    fn sample_positions(&mut self, c: &Colony) {
        let t = c.clock.tick;
        let mut row = Vec::with_capacity(c.settlers.len());
        for (i, s) in c.settlers.iter().enumerate() {
            let away = s.away_until > 0 || s.mind.left;
            let mut flags = 0;
            if s.alive { flags |= 1; }
            if c.below(i) { flags |= 2; }
            if s.job == Job::Sleep { flags |= 4; }
            if s.ill_until > t { flags |= 8; }
            if s.guest_until > 0 { flags |= 16; }
            if away { flags |= 32; }
            if s.carrying.is_some() { flags |= 64; }
            if s.mind.broken.is_some() { flags |= 128; }
            if !s.path.is_empty() { flags |= 256; }
            let (x, y, z) = (s.pos.0 as i32, s.pos.1 as i32, s.z);
            row.push([x, y, z, flags]);
            if !s.alive { continue; }
            let here = c.here3(i);
            if !crate::colony::nav::standable(&c.map, here.0 as usize, here.1 as usize, here.2) {
                self.unstandable_count += 1;
                if self.unstandable.len() < 400 { self.unstandable.push((t, i, here.0 as i32, here.1 as i32, here.2, verb_ix(&s.job))); }
            }
            // A walk covers a cell in 5 minutes at best (3.3 on a worn lane); allow a margin.
            if let Some((lx, ly, lt)) = self.last_pos[i] {
                let reach = ((t - lt) as f32 / 3.0).ceil() as i32 + 4;
                if (x - lx).abs().max((y - ly).abs()) > reach && self.jumps.len() < 400 { self.jumps.push((t, i, (lx, ly), (x, y))); }
            }
            self.last_pos[i] = Some((x, y, t));
        }
        self.pos.push((t, row));
        let mut cs = Vec::new();
        for cr in &c.creatures {
            if !self.creature_names.iter().any(|n| n.0 == cr.id) { self.creature_names.push((cr.id, format!("{:?}", cr.kind), cr.name.clone())); }
            let z = cr.z.unwrap_or_else(|| c.map.surface_z[cr.pos.1 as usize * c.map.width + cr.pos.0 as usize]);
            cs.push([cr.id as i32, cr.pos.0 as i32, cr.pos.1 as i32, z, cr.leaving as i32]);
        }
        self.creatures.push(cs);
    }

    fn sample_slow(&mut self, c: &Colony) {
        let t = c.clock.tick;
        let row: Vec<[i32; 6]> = (0..c.settlers.len()).map(|i| {
            let s = &c.settlers[i];
            let p = |v: f32| (v * 100.0).round() as i32;
            [p(s.hunger), p(s.fatigue), p(s.exposure), p(s.mind.stress), if s.alive { p(c.focus_of(i)) } else { 0 }, s.stuck as i32]
        }).collect();
        self.needs.push((t, row));
        let stored = |k: crate::colony::ItemKind| c.items.iter().filter(|i| i.kind == k && i.stored).count() as f32;
        let hostile = c.creatures.iter().filter(|x| matches!(x.kind, crate::colony::creatures::CreatureKind::Beast | crate::colony::creatures::CreatureKind::Raider | crate::colony::creatures::CreatureKind::Wolf | crate::colony::creatures::CreatureKind::CaveHunter | crate::colony::creatures::CreatureKind::Besieger)).count() as f32;
        let open = c.projects.iter().filter(|p| !p.done).count() as f32;
        let done = c.projects.iter().filter(|p| p.done).count() as f32 + c.hut.as_ref().map_or(0.0, |h| if h.done { 1.0 } else { 0.0 });
        self.series.push((t, [
            c.food_stored() as f32, c.logs_stored() as f32, stored(crate::colony::ItemKind::Stone), c.alive() as f32, c.company() as f32,
            c.items.iter().filter(|i| !i.stored).count() as f32, hostile, c.temperature(), c.darkness(), c.days_of_food(), c.food_goal() as f32,
            c.readiness().0, c.stage as f32, open, done, c.works.len() as f32,
        ]));
        // The works: the hut first, then each project by its kind, place and day.
        let mut seen: Vec<((String, (u16, u16), u64), String, u32, u32, String, String, bool, bool)> = Vec::new();
        if let Some(h) = &c.hut { seen.push((("Hut".into(), h.at, 1), "the hut".into(), crate::colony::HUT_LOGS, h.logs_used, format!("{:?}", c.hut_material), "the first shelter".into(), false, h.done)); }
        for p in &c.projects {
            seen.push(((format!("{:?}", p.kind), p.at, p.day), p.kind.word().to_string(), p.needed, p.used, format!("{:?}", p.material), p.why.clone(), crate::colony::projects::is_dig(p.kind), p.done));
        }
        for (key, word, needed, used, material, why, dig, done) in seen.iter().cloned() {
            match self.works.iter_mut().find(|w| w.key == key) {
                Some(w) => {
                    if w.used != used { w.steps.push((t, used)); w.used = used; }
                    w.needed = needed;
                    if done && w.done_at.is_none() { w.done_at = Some(t); }
                    w.gone_at = None;
                }
                None => self.works.push(WorkSeen { key, word, needed, used, material, why, dig, first: t, done_at: done.then_some(t), gone_at: None, steps: vec![(t, used)] }),
            }
        }
        for w in self.works.iter_mut() {
            if w.gone_at.is_none() && w.done_at.is_none() && !seen.iter().any(|s| s.0 == w.key) { w.gone_at = Some(t); }
        }
    }

    /// Close the spans still open (at the end of a run, so the data holds them).
    pub fn open_spans(&self, now: u64) -> Vec<Span> {
        self.open.iter().flatten().map(|(sp, _, _)| { let mut s = sp.clone(); s.t1 = now; s }).collect()
    }
}

/// A colony the session holds: the camp, its recording and the patron's acts so far.
pub struct ColonyRun {
    pub id: String,
    pub world: String,
    pub tile: (usize, usize),
    pub seed: u64,
    pub settlers_asked: usize,
    pub colony: Colony,
    pub rec: Recorder,
    /// Interventions, "tick verb args" (`Colony::apply_intervention`), applied at their ticks.
    pub script: Vec<String>,
    applied_upto: u64,
    pub failures: Vec<String>,
    /// Where its frames and explorer go.
    pub dir: PathBuf,
}

/// Found a camp on world tile `tile` as the game does.
pub fn found(world: &WorldData, history: Option<&WorldHistory>, tile: (usize, usize), settlers: usize) -> Result<(Colony, u64), String> {
    let (map, seed, _) = crate::tiles::viewer::colony_site(world, history, tile, None);
    if !crate::colony::habitable(&map) { return Err(format!("the site at {},{} is under water; no camp can be made here", tile.0, tile.1)); }
    if let Err(why) = crate::colony::survey(&map).verdict() { return Err(format!("no camp at {},{}: {}", tile.0, tile.1, why)); }
    Ok((crate::tiles::viewer::found_colony(map, history, tile, seed, settlers), seed))
}

impl ColonyRun {
    pub fn new(id: String, world: String, tile: (usize, usize), seed: u64, settlers_asked: usize, colony: Colony, script: Vec<String>, every: u64, frame_days: u64, dir: PathBuf) -> ColonyRun {
        let t = colony.clock.tick;
        let mut rec = Recorder::new(t, every, frame_days);
        rec.observe(&colony);
        rec.ticks_run = 0;
        ColonyRun { id, world, tile, seed, settlers_asked, colony, rec, script, applied_upto: 0, failures: Vec::new(), dir }
    }

    pub fn frames_dir(&self) -> PathBuf { self.dir.join("frames") }

    /// Live up to `ticks` more ticks: the patron's scripted acts at their ticks, then the tick,
    /// then the recorder. Stops early when a new log line contains `until`, or when no one is
    /// left (dead, gone, or the camp departed). Frames of the map are drawn every `frame_days`
    /// days at noon. Returns why it stopped.
    pub fn live(&mut self, ticks: u64, until: Option<&str>, history: Option<&WorldHistory>, atlas: &crate::tiles::Atlas) -> String {
        let t0 = std::time::Instant::now();
        let end = self.colony.clock.tick + ticks;
        if self.rec.frames.is_empty() && self.rec.frame_days > 0 { self.frame(history, atlas); }
        let mut seen_log = self.colony.log.len();
        let mut stop = format!("ran to {}", stamp(end));
        while self.colony.clock.tick < end {
            let now = self.colony.clock.tick;
            for line in &self.script {
                let at = line.split_whitespace().next().and_then(|t| t.parse::<u64>().ok()).unwrap_or(u64::MAX);
                if at >= self.applied_upto && at <= now {
                    if let Err(e) = self.colony.apply_intervention(line) {
                        let l = format!("{}  (intervention failed: {e})", self.colony.clock.stamp());
                        self.failures.push(l.clone());
                        self.colony.log.push(l);
                    }
                }
            }
            self.applied_upto = now + 1;
            self.colony.tick();
            self.rec.observe(&self.colony);
            let t = self.colony.clock.tick;
            if self.rec.frame_days > 0 && t % TICKS_PER_DAY == 12 * 60 && ((t / TICKS_PER_DAY) % self.rec.frame_days == 0) { self.frame(history, atlas); }
            if let Some(text) = until {
                let lower = text.to_lowercase();
                if let Some(l) = self.colony.log[seen_log..].iter().find(|l| l.to_lowercase().contains(&lower)) { stop = format!("stopped at: {l}"); break; }
                seen_log = self.colony.log.len();
            }
            if self.colony.departed.is_some() { stop = format!("the camp departed ({})", self.colony.clock.stamp()); break; }
            if self.colony.settlers.iter().all(|s| !s.alive && s.away_until == 0) { stop = format!("no one is left ({})", self.colony.clock.stamp()); break; }
        }
        self.rec.secs += t0.elapsed().as_secs_f64();
        stop
    }

    /// A frame of the whole embark at 3 px a cell as the camp then stood, without its people
    /// and creatures (the explorer draws those from the samples).
    pub fn frame(&mut self, history: Option<&WorldHistory>, atlas: &crate::tiles::Atlas) {
        let dir = self.frames_dir();
        if std::fs::create_dir_all(&dir).is_err() { return; }
        let t = self.colony.clock.tick;
        let name = format!("t{t:07}.png");
        let path = dir.join(&name);
        render_map(&mut self.colony, history, atlas, 3.0, &path);
        self.rec.frames.push((t, name));
    }

    /// The patron acts now: the line is "verb args" (bless x y r, forbid x y r, favour i, dream
    /// i Hut|Plenty|Rest|Watch, stone Hall|Grove|Shrine x y, name ..., refugees take|turn, bell),
    /// recorded in the script so a replay does the same.
    pub fn act(&mut self, verb: &str) -> Result<String, String> {
        let line = format!("{} {}", self.colony.clock.tick, verb.trim());
        let r = self.colony.apply_intervention(&line);
        if r.is_ok() { self.script.push(line); self.applied_upto = self.applied_upto.max(self.colony.clock.tick + 1); }
        r
    }

    /// One number for the story, as `--sim-snapshot` prints it ("Colony hash"): FNV over the
    /// world's chronicle and the colony's log.
    pub fn hash(&self, history: Option<&WorldHistory>) -> String {
        let mut hsh: u64 = 0xcbf2_9ce4_8422_2325;
        let mut feed = |t: &str| for b in t.bytes() { hsh ^= b as u64; hsh = hsh.wrapping_mul(0x100_0000_01b3); };
        if let Some(h) = history { for e in &h.chronicle.events { feed(&e.title); feed(&e.description); } }
        for l in &self.colony.log { feed(l); }
        format!("{hsh:016x}")
    }

    /// The settler named (any case, a prefix will do) or numbered.
    pub fn who(&self, s: &str) -> Result<usize, String> {
        if let Ok(i) = s.trim().parse::<usize>() { return if i < self.colony.settlers.len() { Ok(i) } else { Err(format!("no settler {i} (0-{})", self.colony.settlers.len().saturating_sub(1))) }; }
        let l = s.trim().to_lowercase();
        let names = || self.colony.settlers.iter().map(|x| x.name.to_lowercase());
        if let Some(i) = names().position(|n| n == l) { return Ok(i); }
        if let Some(i) = names().position(|n| n.starts_with(&l)) { return Ok(i); }
        names().position(|n| n.contains(&l)).ok_or_else(|| format!("no settler '{s}' (settlers: {})", self.colony.settlers.iter().map(|x| x.name.as_str()).collect::<Vec<_>>().join(", ")))
    }
}

/// "Day N, HH:MM" for a tick.
pub fn stamp(t: u64) -> String { crate::colony::Clock { tick: t }.stamp() }

/// A tick from "Day N, HH:MM" (the log's stamp), "day N", "N" (a day) or a raw tick ("t123").
pub fn parse_when(s: &str) -> Option<u64> {
    let s = s.trim().to_lowercase();
    if let Some(t) = s.strip_prefix('t') { return t.parse().ok(); }
    let s = s.trim_start_matches("day").trim();
    let (d, hm) = match s.split_once(',') { Some((d, hm)) => (d.trim(), Some(hm.trim())), None => match s.split_once(' ') { Some((d, hm)) => (d.trim(), Some(hm.trim())), None => (s, None) } };
    let day: u64 = d.parse().ok()?;
    let (h, m) = match hm.and_then(|x| x.split_once(':')) { Some((h, m)) => (h.parse().ok()?, m.parse().ok()?), None => (6u64, 0u64) };
    Some((day.max(1) - 1) * TICKS_PER_DAY + h * 60 + m)
}

/// The tick of a log line's stamp.
pub fn line_tick(l: &str) -> Option<u64> {
    let (st, _) = l.split_once("  ")?;
    parse_when(st)
}

/// The whole embark drawn as the colony's map at `px` a cell, at the moment's state, without
/// its people and creatures; written to `path`.
pub fn render_map(c: &mut Colony, history: Option<&WorldHistory>, atlas: &crate::tiles::Atlas, px: f32, path: &Path) {
    use crate::tiles::render::{render_local, LocalCamera};
    let n = c.map.width;
    let (w, h) = ((n as f32 * px) as usize, (c.map.height as f32 * px) as usize);
    let cam = LocalCamera { cx: n as f32 / 2.0, cy: c.map.height as f32 / 2.0, tile_px: px, z: 0, surface_view: true };
    let mut buf = vec![0u32; w * h];
    render_local(&c.map, atlas, &cam, &mut buf, w, h);
    // People and creatures are hidden for the drawing and put back (drawing only reads).
    let alive: Vec<bool> = c.settlers.iter().map(|s| s.alive).collect();
    for s in c.settlers.iter_mut() { s.alive = false; }
    let creatures = std::mem::take(&mut c.creatures);
    let banner = c.banner.take();
    let clock = c.clock.tick;
    // Drawn at noon of the day, so night does not wash the frame.
    c.clock.tick = clock - clock % TICKS_PER_DAY + 12 * 60;
    crate::tiles::local_ink::draw_colony(c, &cam, &mut buf, w, h, history);
    c.clock.tick = clock;
    c.banner = banner;
    c.creatures = creatures;
    for (s, a) in c.settlers.iter_mut().zip(alive) { s.alive = a; }
    let path = path.to_string_lossy().to_string();
    crate::tiles::viewer::save_rgb_png_pub(&path, w, h, |x, y| { let q = buf[y * w + x]; [(q >> 16) as u8, (q >> 8) as u8, q as u8] });
}

/// A level of the delve drawn whole at `px` a cell, as the window's level view draws it.
pub fn render_level(c: &Colony, history: Option<&WorldHistory>, z: i32, px: f32, path: &Path) {
    use crate::tiles::render::LocalCamera;
    let n = c.map.width;
    let (w, h) = ((n as f32 * px) as usize, (c.map.height as f32 * px) as usize);
    let cam = LocalCamera { cx: n as f32 / 2.0, cy: c.map.height as f32 / 2.0, tile_px: px, z, surface_view: false };
    let mut buf = vec![0u32; w * h];
    crate::tiles::local_ink::render_level_ink(&c.map, &cam, &mut buf, w, h);
    crate::tiles::local_ink::draw_level(c, &cam, &mut buf, w, h, history);
    let path = path.to_string_lossy().to_string();
    crate::tiles::viewer::save_rgb_png_pub(&path, w, h, |x, y| { let q = buf[y * w + x]; [(q >> 16) as u8, (q >> 8) as u8, q as u8] });
}

/// The recording as JSON for the colony explorer (`window.COLONY`).
pub fn data(run: &ColonyRun, history: Option<&WorldHistory>, history_link: Option<&str>) -> Value {
    let c = &run.colony;
    let r = &run.rec;
    let now = c.clock.tick;
    let name = c.name.clone().unwrap_or_else(|| format!("The camp at {},{}", c.camp.0, c.camp.1));
    let people_code = |f: Option<crate::history::FactionId>| f.map(|f| format!("F{}", f.0));
    let mut spans: Vec<Span> = r.spans.clone();
    spans.extend(r.open_spans(now));
    spans.sort_by_key(|s| (s.t0, s.who));
    let flat_spans: Vec<Value> = spans.iter().map(|s| json!([s.who, s.t0, s.t1, s.verb, s.target.map_or(-1, |t| t.0), s.target.map_or(-1, |t| t.1), s.why])).collect();
    let settlers: Vec<Value> = c.settlers.iter().enumerate().map(|(i, s)| {
        let past = s.past.as_ref();
        let age = past.map_or(0, |p| p.age);
        let joined = r.status.iter().find(|x| x.1 == i && x.2).map_or(r.start_tick, |x| x.0);
        json!({
            "i": i, "name": s.name, "age": age, "race": s.persona.race, "female": s.persona.female,
            "calling": past.map(|p| p.calling.clone()), "people": people_code(past.and_then(|p| p.people)),
            "lines": past.map(|p| p.lines.iter().map(|(t, e)| json!([t, e.map(|e| e.0)])).collect::<Vec<_>>()).unwrap_or_default(),
            "feeling": past.and_then(|p| p.feeling.as_ref().map(|f| f.0.clone())),
            "faith": past.and_then(|p| p.faith.as_ref().map(|f| f.1.clone())),
            "who": s.persona.describe(&s.name, age),
            "skill": s.skill.iter().map(|v| (v * 100.0).round() as i32).collect::<Vec<_>>(),
            "taste": s.taste.iter().map(|v| (v * 100.0).round() as i32).collect::<Vec<_>>(),
            "role": s.role.map(|r| crate::colony::ROLES[r]), "office": s.office,
            "spouse": s.spouse, "fate": c.fate(i), "about": c.about(i), "alive": s.alive,
            "wounds": s.wounds.iter().map(|w| format!("{} ({}): {}", w.part, ["", "bruised", "cut", "broken"][w.severity.min(3) as usize], w.from)).collect::<Vec<_>>(),
            "deeds": s.deeds, "made": s.made, "loads": s.loads_laid, "drill": (s.drill * 100.0).round() as i32,
            "stress": (s.mind.stress * 100.0).round() as i32, "breaks": s.mind.breaks, "jaded": s.mind.jaded,
            "thoughts": s.mind.thoughts.iter().map(|t| json!([t.tick, t.text, (t.weight * 100.0).round() as i32])).collect::<Vec<_>>(),
            "scars": s.mind.scars.iter().map(|t| json!([t.tick, t.text, (t.weight * 100.0).round() as i32])).collect::<Vec<_>>(),
            "needs": s.mind.needs.iter().map(|n| json!([format!("{:?}", n.need), n.level, n.focus])).collect::<Vec<_>>(),
            "guest": s.visitor, "stuck": s.stuck, "joined": joined, "job": s.job.verb(), "why": s.why,
        })
    }).collect();
    let log: Vec<Value> = c.log.iter().map(|l| {
        let t = line_tick(l).unwrap_or(0);
        let text = l.split_once("  ").map_or(l.as_str(), |x| x.1);
        json!([t, text])
    }).collect();
    let moments: Vec<Value> = c.moments.iter().map(|m| json!([m.tick, m.title, m.text, m.because, m.at.0, m.at.1, m.major()])).collect();
    let works: Vec<Value> = r.works.iter().map(|w| json!({
        "kind": w.key.0, "word": w.word, "at": [w.key.1 .0, w.key.1 .1], "day": w.key.2, "needed": w.needed, "used": w.used, "material": w.material,
        "why": w.why, "dig": w.dig, "first": w.first, "done": w.done_at, "gone": w.gone_at, "steps": w.steps.iter().map(|s| json!([s.0, s.1])).collect::<Vec<_>>(),
    })).collect();
    let made: Vec<Value> = c.works.iter().map(|w| json!({
        "maker": w.maker, "kind": w.kind, "material": w.material, "quality": w.quality, "day": w.day, "traded": w.traded, "called": w.called,
        "image": w.image.as_ref().map(|x| x.0.clone()), "event": w.image.as_ref().and_then(|x| x.1.map(|e| e.0)),
    })).collect();
    let marks: Vec<Value> = c.marks.iter().map(|m| json!({ "kind": format!("{:?}", m.kind), "at": [m.at.0, m.at.1], "title": m.title, "text": m.text, "day": m.day })).collect();
    let arc = c.arc.as_ref().map(|a| json!({
        "threat": a.threat.name, "kind": format!("{:?}", a.threat.kind), "why": a.threat.why, "cause": a.threat.cause.map(|e| e.0),
        "rumour_day": a.rumour_day, "refugee_day": a.refugee_day, "raid_day": a.raid_day, "chapter": a.chapter,
        "later": a.later.iter().map(|t| json!([t.name, format!("{:?}", t.kind), t.why])).collect::<Vec<_>>(),
        "events": a.events.iter().map(|e| json!([e.day, e.title, e.text, e.because, e.world_cause.map(|e| e.0)])).collect::<Vec<_>>(),
    }));
    let regards: Vec<Value> = c.regards.iter().map(|g| json!({
        "people": g.people, "code": format!("F{}", g.faction.0), "sum": g.causes.iter().map(|x| x.delta).sum::<i32>(),
        "causes": g.causes.iter().map(|x| json!([x.day, x.delta, x.text])).collect::<Vec<_>>(),
    })).collect();
    let ready = c.readiness();
    let world_code = crate::tiles::viewer::colony_code(c);
    json!({
        "meta": {
            "id": run.id, "name": name, "world": run.world, "tile": [run.tile.0, run.tile.1], "seed": run.seed, "code": world_code,
            "start": r.start_tick, "now": now, "every": r.every, "slow": r.slow, "day_ticks": TICKS_PER_DAY,
            "w": c.map.width, "h": c.map.height, "camp": [c.camp.0, c.camp.1], "camp_z": c.camp_z(),
            "frames": r.frames.iter().map(|f| json!([f.0, format!("frames/{}", f.1)])).collect::<Vec<_>>(), "frame_px": 3,
            "history": history_link, "hash": run.hash(history), "secs": (r.secs * 100.0).round() / 100.0, "ticks": r.ticks_run,
            "plan": c.plan_line, "standing": c.standing(), "readiness": [ready.0, ready.1, ready.2], "script": run.script,
            "made_at": stamp(now), "myth": crate::myth::on(),
        },
        "verbs": VERBS, "whys": r.whys, "spans": flat_spans, "settlers": settlers,
        "pos": { "ticks": r.pos.iter().map(|p| p.0).collect::<Vec<_>>(), "rows": r.pos.iter().map(|p| p.1.iter().flat_map(|q| q.iter().copied()).collect::<Vec<i32>>()).collect::<Vec<_>>() },
        "creature_names": r.creature_names.iter().map(|c| json!([c.0, c.1, c.2])).collect::<Vec<_>>(),
        "creatures": r.creatures.iter().map(|row| row.iter().flat_map(|q| q.iter().copied()).collect::<Vec<i32>>()).collect::<Vec<_>>(),
        "needs": { "ticks": r.needs.iter().map(|p| p.0).collect::<Vec<_>>(), "rows": r.needs.iter().map(|p| p.1.iter().flat_map(|q| q.iter().copied()).collect::<Vec<i32>>()).collect::<Vec<_>>() },
        "series": { "fields": SERIES, "ticks": r.series.iter().map(|p| p.0).collect::<Vec<_>>(), "rows": r.series.iter().map(|p| p.1.iter().map(|v| (*v as f64 * 100.0).round() / 100.0).collect::<Vec<f64>>()).collect::<Vec<_>>() },
        "opinions": r.opinions.iter().map(|(t, ps)| json!([t, ps.iter().flat_map(|q| q.iter().copied()).collect::<Vec<i32>>()])).collect::<Vec<_>>(),
        "status": r.status.iter().map(|s| json!([s.0, s.1, s.2])).collect::<Vec<_>>(),
        "log": log, "moments": moments, "works": works, "made": made, "marks": marks, "arc": arc, "regards": regards,
        "children": c.children.iter().map(|x| json!([x.0, x.1, x.2])).collect::<Vec<_>>(),
        "odd": {
            "unstandable": r.unstandable.iter().map(|u| json!([u.0, u.1, u.2, u.3, u.4, u.5])).collect::<Vec<_>>(), "unstandable_count": r.unstandable_count,
            "jumps": r.jumps.iter().map(|j| json!([j.0, j.1, j.2 .0, j.2 .1, j.3 .0, j.3 .1])).collect::<Vec<_>>(),
            "failures": run.failures,
        },
    })
}
