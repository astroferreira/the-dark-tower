//! The lab: the world, its regions, its embarks and its history as tools an agent (or a shell)
//! can call (`planet_lab`, `src/bin/planet_lab.rs`; notes in `src/lab/CLAUDE.md`).
//!
//! One long-lived session holds the worlds it opened (loaded once, ~0.1 s from a world file)
//! and the regions it zoomed, so a run of questions about one place costs one generation.
//! Worlds are made by the game's own binary (`planet_generator --save-world`) into
//! `worlds/lab/`, never by a copy of `main`'s pipeline, so the lab sees exactly what the game
//! makes. Every tool takes JSON arguments and answers text or JSON, plus the files it wrote
//! (PNG renders an agent can open) and whatever the library printed while it ran.

pub mod colony;
pub mod colony_explorer;
pub mod colony_tools;
pub mod explorer;
pub mod history_tools;
pub mod mcp;
pub mod world_tools;

use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::history::world_state::WorldHistory;
use crate::region::zoom::{ZoomParams, ZoomRegion};
use crate::world::WorldData;

/// What a tool answers.
pub struct Output {
    pub body: Body,
    /// Files the tool wrote (renders, reports).
    pub files: Vec<PathBuf>,
    /// What the library printed to stdout while the tool ran (captured, so the protocol's
    /// channel stays clean).
    pub log: String,
    /// Print the whole log, not its tail (a report the library prints, such as an embark's).
    pub full_log: bool,
}

pub enum Body {
    Text(String),
    Json(Value),
}

impl Output {
    pub fn text(s: impl Into<String>) -> Output { Output { body: Body::Text(s.into()), files: Vec::new(), log: String::new(), full_log: false } }
    pub fn json(v: Value) -> Output { Output { body: Body::Json(v), files: Vec::new(), log: String::new(), full_log: false } }
    pub fn with_files(mut self, files: Vec<PathBuf>) -> Output { self.files = files; self }

    /// The answer as one block of text: the body, then the files, then the tail of the log.
    pub fn render(&self) -> String {
        let mut s = match &self.body {
            Body::Text(t) => t.trim_end().to_string(),
            Body::Json(v) => pretty(v, 0),
        };
        if !self.files.is_empty() {
            s.push_str("\n\nFiles:");
            for f in &self.files { s.push_str(&format!("\n  {}", f.display())); }
        }
        let log = self.log.trim();
        if !log.is_empty() {
            let lines: Vec<&str> = log.lines().collect();
            let keep = if self.full_log { lines.len() } else { LOG_TAIL };
            let tail = &lines[lines.len().saturating_sub(keep)..];
            s.push_str(&format!("\n\nLog ({} lines{}):\n", lines.len(), if lines.len() > keep { format!(", last {keep}") } else { String::new() }));
            s.push_str(&tail.join("\n"));
        }
        s
    }
}

const LOG_TAIL: usize = 40;

/// JSON for reading: objects one key a line, but any value whose compact form is short (a
/// coordinate pair, a small record) kept on one line, so a list of places reads as a list.
pub fn pretty(v: &Value, indent: usize) -> String {
    let compact = serde_json::to_string(v).unwrap_or_default();
    if compact.len() + indent * 2 <= 150 { return compact; }
    let pad = "  ".repeat(indent + 1);
    let end = "  ".repeat(indent);
    match v {
        Value::Object(m) => {
            let items: Vec<String> = m.iter().map(|(k, x)| format!("{pad}{}: {}", serde_json::to_string(k).unwrap_or_default(), pretty(x, indent + 1))).collect();
            format!("{{\n{}\n{end}}}", items.join(",\n"))
        }
        Value::Array(a) if a.iter().all(|x| !x.is_object() && !x.is_array()) => {
            // Scalars: wrapped lines of several.
            let mut lines: Vec<String> = Vec::new();
            let mut line = String::new();
            for x in a {
                let t = serde_json::to_string(x).unwrap_or_default();
                if !line.is_empty() && line.len() + t.len() + pad.len() > 110 { lines.push(std::mem::take(&mut line)); }
                if !line.is_empty() { line.push_str(", "); }
                line.push_str(&t);
            }
            if !line.is_empty() { lines.push(line); }
            format!("[\n{}\n{end}]", lines.iter().map(|l| format!("{pad}{l}")).collect::<Vec<_>>().join(",\n"))
        }
        Value::Array(a) => format!("[\n{}\n{end}]", a.iter().map(|x| format!("{pad}{}", pretty(x, indent + 1))).collect::<Vec<_>>().join(",\n")),
        _ => compact,
    }
}

pub type ToolResult = Result<Output, String>;

/// A tool's name, what it does (the agent reads this to choose), its JSON schema and its body.
pub struct Tool {
    pub name: &'static str,
    pub description: &'static str,
    pub schema: fn() -> Value,
    pub run: fn(&mut Session, &Value) -> ToolResult,
}

/// Every tool, in the order `tools/list` gives them.
pub fn tools() -> Vec<Tool> {
    let mut t = world_tools::tools();
    t.extend(history_tools::tools());
    t.extend(colony_tools::tools());
    t
}

/// A world the session holds.
pub struct WorldEntry {
    pub id: String,
    pub path: PathBuf,
    /// The same world saved before its history (open_world makes it beside a world with
    /// history), the land `history_branch` replays on: a saved world carries its history's
    /// scars and no record of the biomes they covered.
    pub prehistory_path: Option<PathBuf>,
    pub world: WorldData,
    pub history: Option<WorldHistory>,
    /// Regions zoomed in this session, most recent last (kept small: ~30 MB each).
    pub regions: Vec<(RegionKey, ZoomRegion)>,
    /// The chronicle's indexes (events by id, effects, events by entity), built on first use.
    pub hist_index: Option<history_tools::HistIndex>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RegionKey { pub x: usize, pub y: usize, pub tiles: usize, pub scale: usize, pub erosion: usize }

const REGIONS_KEPT: usize = 4;

pub struct Session {
    pub worlds: Vec<WorldEntry>,
    /// The world tools act on when no `world` is given: the last opened or named.
    pub current: Option<String>,
    /// Where renders and reports go.
    pub out_dir: PathBuf,
    /// Where generated worlds are cached.
    pub cache_dir: PathBuf,
    atlas: Option<crate::tiles::Atlas>,
    /// Colonies founded in this session (`colony_tools.rs`), and the one tools act on.
    pub colonies: Vec<colony::ColonyRun>,
    pub current_colony: Option<String>,
}

impl Session {
    pub fn new() -> Session {
        let out_dir = std::env::var("PLANET_LAB_OUT").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("out/lab"));
        let cache_dir = std::env::var("PLANET_LAB_WORLDS").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("worlds/lab"));
        Session { worlds: Vec::new(), current: None, out_dir, cache_dir, atlas: None, colonies: Vec::new(), current_colony: None }
    }

    pub fn atlas(&mut self) -> &crate::tiles::Atlas {
        self.atlas.get_or_insert_with(crate::tiles::Atlas::generated)
    }

    /// The world named by `args.world`, else the current one; makes it current and sets the
    /// process-wide state the game sets for a world (the leaves under its star, a moonless sky).
    pub fn world_index(&mut self, args: &Value) -> Result<usize, String> {
        let id = match args.get("world").and_then(Value::as_str) {
            Some(id) => id.to_string(),
            None => self.current.clone().ok_or("no world is open: call open_world first")?,
        };
        let i = self.worlds.iter().position(|w| w.id == id)
            .ok_or_else(|| format!("no open world '{id}' (open: {})", self.worlds.iter().map(|w| w.id.as_str()).collect::<Vec<_>>().join(", ")))?;
        self.current = Some(id);
        let w = &self.worlds[i].world;
        crate::tiles::ink::set_leaves(w.star_system.as_ref().map(|s| s.star.spectral.class));
        let sky = w.star_system.clone().unwrap_or_else(crate::astro::StarSystem::sol);
        crate::history::naming::generator::set_moonless(!sky.is_sol() && sky.night_moon().is_none());
        Ok(i)
    }

    pub fn entry(&mut self, args: &Value) -> Result<&mut WorldEntry, String> {
        let i = self.world_index(args)?;
        Ok(&mut self.worlds[i])
    }

    /// Add (or replace) a world and make it current.
    pub fn insert(&mut self, e: WorldEntry) {
        self.worlds.retain(|w| w.id != e.id);
        self.current = Some(e.id.clone());
        self.worlds.push(e);
    }

    /// The region round a world tile: from the session if zoomed before, else generated
    /// (the chunk cache on disk makes a second session's first zoom quick too).
    pub fn region(&mut self, args: &Value) -> Result<(usize, RegionKey), String> {
        let i = self.world_index(args)?;
        let (x, y) = xy(args, &self.worlds[i].world)?;
        let key = RegionKey {
            x, y,
            tiles: get_usize(args, "tiles").unwrap_or(8).max(1),
            scale: get_usize(args, "scale").unwrap_or(128),
            erosion: get_usize(args, "erosion").unwrap_or(40),
        };
        let e = &mut self.worlds[i];
        if let Some(pos) = e.regions.iter().position(|(k, _)| *k == key) {
            let r = e.regions.remove(pos);
            e.regions.push(r);
        } else {
            let params = zoom_params(&e.world, key);
            // The world's soils and resources are computed on first use, as `--places` does.
            let _ = (e.world.soils(), e.world.resources());
            let (zoom, _) = capture(|| crate::region::zoom::generate_zoom(&e.world, &params));
            e.regions.push((key, zoom));
            if e.regions.len() > REGIONS_KEPT { e.regions.remove(0); }
        }
        Ok((i, key))
    }

    pub fn out_path(&self, name: &str) -> Result<PathBuf, String> {
        std::fs::create_dir_all(&self.out_dir).map_err(|e| format!("{}: {e}", self.out_dir.display()))?;
        Ok(self.out_dir.join(name))
    }

    /// Run one tool by name, catching panics (a panic in the library answers an error and the
    /// session goes on) and capturing what the library prints.
    pub fn call(&mut self, name: &str, args: &Value) -> ToolResult {
        let tool = tools().into_iter().find(|t| t.name == name).ok_or_else(|| format!("unknown tool '{name}'"))?;
        let (res, log) = capture(|| std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (tool.run)(self, args))));
        match res {
            Ok(Ok(mut out)) => { if out.log.is_empty() { out.log = log; } else { out.log.push('\n'); out.log.push_str(&log); } Ok(out) }
            Ok(Err(e)) => Err(if log.trim().is_empty() { e } else { format!("{e}\n\nLog:\n{}", tail(&log, LOG_TAIL)) }),
            Err(p) => {
                let msg = p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_else(|| "panic".into());
                Err(format!("the tool panicked: {msg}\n\nLog:\n{}", tail(&log, LOG_TAIL)))
            }
        }
    }
}

pub fn zoom_params(world: &WorldData, k: RegionKey) -> ZoomParams {
    ZoomParams { center_x: k.x, center_y: k.y, tiles: k.tiles, cells_per_tile: k.scale, erosion_iterations: k.erosion, seed: world.seed() }
}

fn tail(s: &str, n: usize) -> String {
    let lines: Vec<&str> = s.trim().lines().collect();
    lines[lines.len().saturating_sub(n)..].join("\n")
}

// ---------------------------------------------------------------------------------------------
// Capturing stdout. The library prints progress with `println!`; under MCP stdout is the
// protocol's channel, so `mcp::serve` moves the protocol to a duplicate of fd 1 and points fd 1
// at stderr, and each call points fd 1 at a temporary file for its length.

static CAPTURE_DEPTH: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Run `f` with fds 1 and 2 redirected to a temporary file; return its result and what it
/// printed (the library reports progress on both).
/// Nested captures pass through to the outermost.
pub fn capture<R>(f: impl FnOnce() -> R) -> (R, String) {
    use std::sync::atomic::Ordering;
    if CAPTURE_DEPTH.fetch_add(1, Ordering::SeqCst) > 0 {
        let r = f();
        CAPTURE_DEPTH.fetch_sub(1, Ordering::SeqCst);
        return (r, String::new());
    }
    let path = std::env::temp_dir().join(format!("planet_lab_{}.log", std::process::id()));
    let file = std::fs::File::create(&path);
    let (r, text) = match file {
        Ok(file) => {
            let _ = std::io::stdout().flush();
            // SAFETY: plain descriptor juggling on fds this process owns.
            let fd = std::os::unix::io::AsRawFd::as_raw_fd(&file);
            let (out, err) = unsafe { (libc::dup(1), libc::dup(2)) };
            unsafe { libc::dup2(fd, 1); libc::dup2(fd, 2); }
            let r = f();
            let _ = std::io::stdout().flush();
            let _ = std::io::stderr().flush();
            unsafe { libc::dup2(out, 1); libc::dup2(err, 2); libc::close(out); libc::close(err); }
            drop(file);
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            let _ = std::fs::remove_file(&path);
            (r, text)
        }
        Err(_) => (f(), String::new()),
    };
    CAPTURE_DEPTH.fetch_sub(1, Ordering::SeqCst);
    (r, text)
}

// ---------------------------------------------------------------------------------------------
// Argument helpers.

pub fn get_usize(args: &Value, k: &str) -> Option<usize> {
    args.get(k).and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))).map(|v| v as usize)
}
pub fn get_u64(args: &Value, k: &str) -> Option<u64> {
    args.get(k).and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok())))
}
pub fn get_str<'a>(args: &'a Value, k: &str) -> Option<&'a str> { args.get(k).and_then(Value::as_str) }
pub fn get_bool(args: &Value, k: &str) -> bool { args.get(k).and_then(Value::as_bool).unwrap_or(false) }

/// The world tile `x`,`y` of the arguments, checked against the world.
pub fn xy(args: &Value, w: &WorldData) -> Result<(usize, usize), String> {
    let x = get_usize(args, "x").ok_or("needs x (a world tile column)")?;
    let y = get_usize(args, "y").ok_or("needs y (a world tile row)")?;
    if x >= w.width || y >= w.height { return Err(format!("tile {x},{y} is outside the {}x{} world", w.width, w.height)); }
    Ok((x, y))
}

/// JSON schema helpers for the tool list.
pub fn schema(props: Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": props, "required": required })
}
pub fn world_prop() -> Value { json!({ "type": "string", "description": "World id from open_world (default: the current world)" }) }
pub fn tile_props() -> (Value, Value) {
    (json!({ "type": "integer", "description": "World tile column" }), json!({ "type": "integer", "description": "World tile row" }))
}
pub fn region_props(extra: Value) -> Value {
    let (x, y) = tile_props();
    let mut p = json!({
        "world": world_prop(), "x": x, "y": y,
        "tiles": { "type": "integer", "description": "World tiles across the region (default 8, as the walker)" },
        "scale": { "type": "integer", "description": "Region cells per world tile (default 128)" },
        "erosion": { "type": "integer", "description": "Region erosion iterations (default 40)" },
    });
    if let (Some(p), Some(e)) = (p.as_object_mut(), extra.as_object()) { for (k, v) in e { p.insert(k.clone(), v.clone()); } }
    p
}

/// Whether `path` is older than the game binary that would remake it.
pub fn older_than_build(path: &Path, exe: &Path) -> bool {
    let m = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    matches!((m(path), m(exe)), (Some(a), Some(b)) if a < b)
}
