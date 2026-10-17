//! The colony tools: found a camp where the game would, live it under the recorder, and ask
//! about it: its state, a settler, the log, why someone did what they did at a moment, what a
//! cell is and whether it can be reached, a way between two places; act as the patron; render
//! it; run the checks; branch it with other acts; write the colony explorer.

use serde_json::{json, Value};

use super::colony::{self, ColonyRun, VERBS};
use super::{get_bool, get_str, get_u64, get_usize, schema, world_prop, Output, Session, Tool, ToolResult};
use crate::colony::TICKS_PER_DAY;

pub fn tools() -> Vec<Tool> {
    vec![
        Tool { name: "colony_found", description: "Found a colony on a world with history, as the game does (the embark at the tile, the settlers from the history, the first arc), and keep it in the session with a recorder: every settler's job and why, positions, needs, stocks, works, opinions and frames of the map. Default tile: the dev embark. `days` lives it at once. `script` is the patron's acts, lines \"tick verb args\" (as `--interventions`).", schema: found_schema, run: found },
        Tool { name: "colony_run", description: "Live the colony on: `days` and/or `hours`, or until a log line contains `until` (with `days` as the limit). Answers why it stopped, the new log lines and the state.", schema: run_schema, run: run },
        Tool { name: "colony_status", description: "The colony now: clock, people, food (stored, days of food, goal), stocks, the work under way and the plan, works standing, the arc's troubles, readiness, standing lines, each settler's job and why, the hash (same as --sim-snapshot's).", schema: colony_schema, run: status },
        Tool { name: "colony_settler", description: "Everything about one settler (by name or number): past (with the world events it rests on), character, skills, needs, mind (stress, thoughts, scars, breaks), wounds, deeds, family, office, fate, what they do now and why, and their last decisions with how long each lasted.", schema: who_schema, run: settler },
        Tool { name: "colony_log", description: "The colony's log, its moments, or its decisions (every change of job, with why and how long), filtered by text, settler and days.", schema: log_schema, run: log },
        Tool { name: "colony_why", description: "Why was a settler doing what they did at a moment: the job and its reason at that tick, the decisions before and after, their needs, stress and place then, and their thoughts up to then.", schema: why_schema, run: why },
        Tool { name: "colony_track", description: "A settler's walk between two moments: where they stood at each sample (cell and level, below the ground or not), what they were doing, and how far they went; the way to read a job that took far longer than it should.", schema: track_schema, run: track },
        Tool { name: "colony_cell", description: "One cell of the embark: its ground and level, material, whether one can stand there, the building or mark on it, footsteps worn, who and what is near, and whether it can be walked to from the camp's fire (and how far).", schema: cell_schema, run: cell },
        Tool { name: "colony_path", description: "The way a settler would walk between two places (settler name, \"camp\", or \"x,y[,z]\"), with its length and levels; when there is none, how far one can get from the start and the nearest reachable cell to the goal.", schema: path_schema, run: path },
        Tool { name: "colony_act", description: "Act as the patron now (recorded for replays): bless X Y R | forbid X Y R | favour I | dream I Hut|Plenty|Rest|Watch | stone Hall|Grove|Shrine X Y | name NAME | refugees take|turn | bell.", schema: act_schema, run: act },
        Tool { name: "colony_render", description: "Render the colony now as PNG: `camp` (the window's view round the fire, or round `x`,`y`), `map` (the whole embark), or `level` (the delve at level `z`).", schema: render_schema, run: render },
        Tool { name: "colony_checks", description: "The debugger's checks over the recording: deaths and what the store held, hunger with food in the store, settlers stuck, standing where no one can stand, jumps no walk could make, jobs held for a day or more, settlers changing their minds over and over, idle shares, works stalled or set aside, days with nothing in the log, acts that failed; with `replay`, whether a second run tells the same story (determinism).", schema: checks_schema, run: checks },
        Tool { name: "colony_branch", description: "Replay the colony from its founding with other patron acts from a day on (`acts`: lines \"verb args\" applied at `from_day`), as long as the original has lived, and compare: whether the replay matches the original up to the fork, the first log line that differs, deaths, works and the people's fates in each. The branch is kept as a colony.", schema: branch_schema, run: branch },
        Tool { name: "colony_explorer", description: "Write the colony explorer: an HTML app over the recording (open index.html from disk). The camp's days and moments as a reader, every settler's page (linked to the history explorer for their past), an activity chart of who did what when and why, needs and stocks over time, the map replayed with everyone's walks, the bonds between settlers, the works rising, the checks; `compare` another colony (a branch) side by side.", schema: explorer_schema, run: explorer },
    ]
}

fn colony_prop() -> Value { json!({ "type": "string", "description": "Colony id from colony_found (default: the last founded or named)" }) }

fn found_schema() -> Value {
    schema(json!({
        "world": world_prop(),
        "x": { "type": "integer", "description": "World tile column (default: the dev embark)" },
        "y": { "type": "integer", "description": "World tile row" },
        "settlers": { "type": "integer", "description": "Founders (default 7)" },
        "days": { "type": "number", "description": "Days to live at once (default 0)" },
        "script": { "description": "The patron's acts: an array of lines \"tick verb args\", or a file of them" },
        "every": { "type": "integer", "description": "Minutes between position samples (default 10)" },
        "frame_days": { "type": "integer", "description": "A frame of the map every N days at noon (default 1, 0 for none)" },
        "id": { "type": "string", "description": "Name for the colony (default world@x,y)" },
    }), &[])
}
fn colony_schema() -> Value { schema(json!({ "colony": colony_prop() }), &[]) }
fn run_schema() -> Value {
    schema(json!({ "colony": colony_prop(), "days": { "type": "number", "description": "Days to live (the limit with `until`; default 1)" }, "hours": { "type": "number" }, "until": { "type": "string", "description": "Stop at the first new log line containing this" } }), &[])
}
fn who_schema() -> Value { schema(json!({ "colony": colony_prop(), "who": { "type": "string", "description": "Settler name (or its start) or number" }, "decisions": { "type": "integer", "description": "Recent decisions to list (default 15)" } }), &["who"]) }
fn log_schema() -> Value {
    schema(json!({
        "colony": colony_prop(), "kind": { "type": "string", "enum": ["log", "moments", "decisions"], "description": "Default log" },
        "text": { "type": "string" }, "who": { "type": "string", "description": "Lines naming this settler (decisions: this settler's)" },
        "from_day": { "type": "integer" }, "to_day": { "type": "integer" }, "limit": { "type": "integer", "description": "Default 60" }, "newest_first": { "type": "boolean" },
    }), &[])
}
fn why_schema() -> Value { schema(json!({ "colony": colony_prop(), "who": { "type": "string" }, "at": { "type": "string", "description": "\"Day 12, 14:30\", \"day 12\", or \"t17310\" (default now)" } }), &["who"]) }
fn track_schema() -> Value { schema(json!({ "colony": colony_prop(), "who": { "type": "string" }, "from": { "type": "string", "description": "\"Day 33, 17:00\" (default a day before `to`)" }, "to": { "type": "string", "description": "Default now" } }), &["who"]) }
fn cell_schema() -> Value { schema(json!({ "colony": colony_prop(), "x": { "type": "integer" }, "y": { "type": "integer" }, "z": { "type": "integer", "description": "Level (default the ground's)" } }), &["x", "y"]) }
fn path_schema() -> Value { schema(json!({ "colony": colony_prop(), "from": { "type": "string" }, "to": { "type": "string" } }), &["from", "to"]) }
fn act_schema() -> Value { schema(json!({ "colony": colony_prop(), "act": { "type": "string" } }), &["act"]) }
fn render_schema() -> Value {
    schema(json!({
        "colony": colony_prop(), "view": { "type": "string", "enum": ["camp", "map", "level"] },
        "x": { "type": "integer" }, "y": { "type": "integer" }, "z": { "type": "integer" }, "px": { "type": "number", "description": "Pixels a cell" },
    }), &[])
}
fn checks_schema() -> Value { schema(json!({ "colony": colony_prop(), "replay": { "type": "boolean", "description": "Run the colony again from its founding and compare (doubles the time)" } }), &[]) }
fn branch_schema() -> Value {
    schema(json!({
        "colony": colony_prop(), "from_day": { "type": "number", "description": "The day of the fork (acts apply at its 06:00 unless `at` is given)" },
        "at": { "type": "string", "description": "The fork's moment instead of from_day (\"Day 5, 14:00\")" },
        "acts": { "type": "array", "items": { "type": "string" }, "description": "Lines \"verb args\" applied at the fork" },
        "drop": { "type": "boolean", "description": "Drop the original's own acts from the fork on (default true)" },
        "id": { "type": "string" },
    }), &[])
}
fn explorer_schema() -> Value {
    schema(json!({
        "colony": colony_prop(), "dir": { "type": "string", "description": "Folder (default out/lab/colony_<id>)" },
        "compare": { "type": "string", "description": "Another colony id to compare with" },
        "history": { "type": "boolean", "description": "Also write the world's history explorer beside it and link the settlers' pasts to it (default true)" },
        "open": { "type": "boolean" },
    }), &[])
}

impl Session {
    pub fn colony_index(&mut self, args: &Value) -> Result<usize, String> {
        let id = match get_str(args, "colony") {
            Some(id) => id.to_string(),
            None => self.current_colony.clone().ok_or("no colony: call colony_found first")?,
        };
        let i = self.colonies.iter().position(|c| c.id == id)
            .ok_or_else(|| format!("no colony '{id}' (colonies: {})", self.colonies.iter().map(|c| c.id.as_str()).collect::<Vec<_>>().join(", ")))?;
        self.current_colony = Some(id);
        Ok(i)
    }

    /// Live colony `ci` for `ticks` (with its world's history and the atlas).
    fn live(&mut self, ci: usize, ticks: u64, until: Option<&str>) -> Result<String, String> {
        let wid = self.colonies[ci].world.clone();
        let wi = self.world_index(&json!({ "world": wid }))?;
        self.atlas();
        let Session { worlds, colonies, atlas, .. } = self;
        let hist = worlds[wi].history.as_ref();
        Ok(colonies[ci].live(ticks, until, hist, atlas.as_ref().unwrap()))
    }
}

fn safe(s: &str) -> String { s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect() }

fn read_script(args: &Value) -> Result<Vec<String>, String> {
    let clean = |t: &str| t.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty() && !l.starts_with('#')).collect::<Vec<_>>();
    match args.get("script") {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(a)) => Ok(a.iter().filter_map(|v| v.as_str().map(|s| s.trim().to_string())).filter(|l| !l.is_empty()).collect()),
        Some(Value::String(s)) if !s.contains('\n') && std::path::Path::new(s).exists() => std::fs::read_to_string(s).map(|t| clean(&t)).map_err(|e| format!("{s}: {e}")),
        Some(Value::String(s)) => Ok(clean(s)),
        _ => Err("script: an array of lines or a file".into()),
    }
}

fn days_ticks(args: &Value, default_days: f64) -> u64 {
    let d = args.get("days").and_then(Value::as_f64).unwrap_or(if args.get("hours").is_some() { 0.0 } else { default_days });
    let h = args.get("hours").and_then(Value::as_f64).unwrap_or(0.0);
    ((d * TICKS_PER_DAY as f64) + h * 60.0).round().max(0.0) as u64
}

fn found(s: &mut Session, args: &Value) -> ToolResult {
    let wi = s.world_index(args)?;
    let script = read_script(args)?;
    let e = &s.worlds[wi];
    if e.history.is_none() { return Err(format!("world '{}' has no history; a colony's settlers come out of one (open a world with history)", e.id)); }
    let tile = match (get_usize(args, "x"), get_usize(args, "y")) {
        (Some(x), Some(y)) => (x, y),
        _ => crate::tiles::dev_embark::pick_dev_embark(&e.world, e.history.as_ref()),
    };
    let n = get_usize(args, "settlers").unwrap_or(7).max(1);
    let (col, seed) = colony::found(&e.world, e.history.as_ref(), tile, n)?;
    let id = get_str(args, "id").map(str::to_string).unwrap_or_else(|| format!("{}@{},{}", e.id, tile.0, tile.1));
    let dir = s.out_dir.join(format!("colony_{}", safe(&id)));
    let _ = std::fs::remove_dir_all(dir.join("frames"));
    let every = get_u64(args, "every").unwrap_or(10);
    let frame_days = get_u64(args, "frame_days").unwrap_or(1);
    let run = ColonyRun::new(id.clone(), e.id.clone(), tile, seed, n, col, script, every, frame_days, dir);
    s.colonies.retain(|c| c.id != id);
    s.colonies.push(run);
    s.current_colony = Some(id.clone());
    let ci = s.colonies.len() - 1;
    let ticks = days_ticks(args, 0.0);
    let stopped = if ticks > 0 { Some(s.live(ci, ticks, None)?) } else { None };
    let r = &s.colonies[ci];
    let c = &r.colony;
    let mut out = json!({
        "colony": id, "tile": [tile.0, tile.1], "code": crate::tiles::viewer::colony_code(c), "camp": [c.camp.0, c.camp.1], "camp_z": c.camp_z(),
        "founding": c.log.first(),
        "settlers": c.settlers.iter().map(|st| format!("{}, {}{}", st.name, st.past.as_ref().map_or(0, |p| p.age), st.past.as_ref().map(|p| format!(", {}", p.calling)).unwrap_or_default())).collect::<Vec<_>>(),
        "arc": c.arc.as_ref().map(|a| format!("{} ({:?}): {}; rumour day {}, refugees day {}, raid day {}", a.threat.name, a.threat.kind, a.threat.why, a.rumour_day, a.refugee_day, a.raid_day)),
        "script": r.script.len(),
    });
    if let Some(st) = stopped { out["lived"] = json!(st); out["now"] = json!(c.clock.stamp()); out["alive"] = json!(format!("{} of {}", c.alive(), c.company())); out["last_lines"] = json!(c.log.iter().rev().take(12).rev().collect::<Vec<_>>()); }
    Ok(Output::json(out))
}

fn run(s: &mut Session, args: &Value) -> ToolResult {
    let ci = s.colony_index(args)?;
    let ticks = days_ticks(args, 1.0);
    let before = s.colonies[ci].colony.log.len();
    let t0 = std::time::Instant::now();
    let stopped = s.live(ci, ticks, get_str(args, "until"))?;
    let c = &s.colonies[ci].colony;
    let new: Vec<&String> = c.log[before..].iter().collect();
    let shown: Vec<&String> = new.iter().rev().take(40).rev().copied().collect();
    Ok(Output::json(json!({
        "stopped": stopped, "now": c.clock.stamp(), "secs": (t0.elapsed().as_secs_f64() * 100.0).round() / 100.0,
        "alive": format!("{} of {}", c.alive(), c.company()), "food": c.food_stored(), "days_of_food": (c.days_of_food() * 10.0).round() / 10.0,
        "new_lines": new.len(), "lines": shown,
    })))
}

fn status(s: &mut Session, args: &Value) -> ToolResult {
    let ci = s.colony_index(args)?;
    let wi = s.world_index(&json!({ "world": s.colonies[ci].world }))?;
    let hist = s.worlds[wi].history.as_ref();
    let r = &s.colonies[ci];
    let c = &r.colony;
    let ready = c.readiness();
    let stored = |k: crate::colony::ItemKind| c.items.iter().filter(|i| i.kind == k && i.stored).count();
    Ok(Output::json(json!({
        "colony": r.id, "name": c.name, "now": c.clock.stamp(), "season": format!("{:?}", c.season()), "temperature": (c.temperature() * 10.0).round() / 10.0,
        "sky": c.sky_line(), "alive": format!("{} of {}", c.alive(), c.company()), "stage": (["camp", "hamlet", "village"].get(c.stage as usize).copied().unwrap_or("?")),
        "food": { "stored": c.food_stored(), "days": (c.days_of_food() * 10.0).round() / 10.0, "goal": c.food_goal(), "drink": c.drink },
        "stocks": { "logs": c.logs_stored(), "stone": stored(crate::colony::ItemKind::Stone), "loose_items": c.items.iter().filter(|i| !i.stored).count() },
        "plan": c.plan_line, "standing": c.standing(),
        "works": c.projects.iter().filter(|p| !p.done).map(|p| format!("{} {}/{} ({})", p.kind.word(), p.used, p.needed, if crate::colony::projects::is_dig(p.kind) { "dig" } else { "build" })).collect::<Vec<_>>(),
        "works_done": c.projects.iter().filter(|p| p.done).map(|p| format!("{} (day {})", p.kind.word(), p.day)).collect::<Vec<_>>(),
        "hut": c.hut.as_ref().map(|h| format!("{} {}/{}", if h.done { "built" } else { "rising" }, h.logs_used, crate::colony::HUT_LOGS)),
        "arc": c.arc.as_ref().map(|a| json!({ "threat": a.threat.name, "chapter": a.chapter, "raid_day": a.raid_day, "beats": a.events.iter().map(|e| format!("day {}: {}", e.day, e.title)).collect::<Vec<_>>(), "later": a.later.iter().map(|t| t.name.clone()).collect::<Vec<_>>() })),
        "readiness": format!("{:.2}: {} {}", ready.0, ready.1, ready.2),
        "roll_call": c.roll_call(), "moments": c.moments.len(), "log_lines": c.log.len(), "works_made": c.works.len(),
        "recorded": { "ticks": r.rec.ticks_run, "secs": (r.rec.secs * 100.0).round() / 100.0, "decisions": r.rec.spans.len(), "frames": r.rec.frames.len() },
        "script": r.script, "hash": r.hash(hist),
    })))
}

/// A recorded span as a line: "Day 3, 14:05  felling (2 h 10 m) Felling a tree for the hut".
fn span_line(r: &ColonyRun, sp: &colony::Span) -> String {
    let dur = sp.t1.saturating_sub(sp.t0);
    format!("{}  {} for {}{}  {}", colony::stamp(sp.t0), VERBS[sp.verb as usize], dur_words(dur),
        sp.target.map(|t| format!(" at {},{}", t.0, t.1)).unwrap_or_default(), r.rec.whys[sp.why as usize])
}

fn dur_words(t: u64) -> String {
    if t >= TICKS_PER_DAY { format!("{}d {}h", t / TICKS_PER_DAY, (t % TICKS_PER_DAY) / 60) } else if t >= 60 { format!("{}h {:02}m", t / 60, t % 60) } else { format!("{}m", t) }
}

fn all_spans(r: &ColonyRun) -> Vec<colony::Span> {
    let mut v = r.rec.spans.clone();
    v.extend(r.rec.open_spans(r.colony.clock.tick));
    v.sort_by_key(|s| (s.t0, s.who));
    v
}

fn settler(s: &mut Session, args: &Value) -> ToolResult {
    let ci = s.colony_index(args)?;
    let r = &s.colonies[ci];
    let i = r.who(get_str(args, "who").unwrap_or(""))?;
    let c = &r.colony;
    let st = &c.settlers[i];
    let k = get_usize(args, "decisions").unwrap_or(15);
    let spans: Vec<colony::Span> = all_spans(r).into_iter().filter(|x| x.who == i).collect();
    let mut by_verb = [0u64; 13];
    for x in &spans { by_verb[x.verb as usize] += x.t1 - x.t0; }
    let total: u64 = by_verb.iter().sum::<u64>().max(1);
    let mut shares: Vec<(usize, u64)> = by_verb.iter().copied().enumerate().filter(|x| x.1 > 0).collect();
    shares.sort_by(|a, b| b.1.cmp(&a.1));
    let past = st.past.as_ref();
    Ok(Output::json(json!({
        "i": i, "name": st.name, "age": past.map(|p| p.age), "race": st.persona.race, "calling": past.map(|p| p.calling.clone()),
        "fate": c.fate(i), "now": format!("{} - {} (at {},{} level {}{})", st.job.verb(), st.why, st.pos.0, st.pos.1, st.z, if c.below(i) { ", below the ground" } else { "" }),
        "needs": format!("hunger {:.0}%, fatigue {:.0}%, exposure {:.0}%, stress {:.2}, focus {:.2}", st.hunger * 100.0, st.fatigue * 100.0, st.exposure * 100.0, st.mind.stress, if st.alive { c.focus_of(i) } else { 0.0 }),
        "past": past.map(|p| p.lines.iter().map(|(t, e)| match e { Some(e) => format!("{t} [event {}]", e.0), None => t.clone() }).collect::<Vec<_>>()),
        "feeling": past.and_then(|p| p.feeling.as_ref().map(|f| f.0.clone())), "faith": past.and_then(|p| p.faith.as_ref().map(|f| f.1.clone())),
        "who": st.persona.describe(&st.name, past.map_or(20, |p| p.age)), "about": c.about(i),
        "skills": crate::colony::ROLES.iter().zip(st.skill.iter()).map(|(r, v)| format!("{r} {:.2}", v)).collect::<Vec<_>>(),
        "role": st.role.map(|r| crate::colony::ROLES[r]), "office": st.office, "deeds": st.deeds, "made": st.made,
        "wounds": st.wounds.iter().map(|w| format!("{} sev {} from {}", w.part, w.severity, w.from)).collect::<Vec<_>>(),
        "mind": { "stress": st.mind.stress, "breaks": st.mind.breaks, "jaded": st.mind.jaded, "left": st.mind.left,
            "thoughts": st.mind.thoughts.iter().map(|t| format!("{} ({:+.2}) {}", colony::stamp(t.tick), t.weight, t.text)).collect::<Vec<_>>(),
            "scars": st.mind.scars.iter().map(|t| format!("{} ({:+.2}) {}", colony::stamp(t.tick), t.weight, t.text)).collect::<Vec<_>>(),
            "needs": st.mind.needs.iter().map(|n| format!("{:?} L{} focus {}", n.need, n.level, n.focus)).collect::<Vec<_>>() },
        "time_spent": shares.iter().map(|(v, t)| format!("{} {:.0}%", VERBS[*v], *t as f64 * 100.0 / total as f64)).collect::<Vec<_>>(),
        "stuck_ticks": st.stuck, "decisions_recorded": spans.len(),
        "last_decisions": spans.iter().rev().take(k).rev().map(|x| span_line(r, x)).collect::<Vec<_>>(),
    })))
}

fn log(s: &mut Session, args: &Value) -> ToolResult {
    let ci = s.colony_index(args)?;
    let r = &s.colonies[ci];
    let c = &r.colony;
    let kind = get_str(args, "kind").unwrap_or("log");
    let text = get_str(args, "text").map(str::to_lowercase);
    let from = get_u64(args, "from_day").map(|d| (d.max(1) - 1) * TICKS_PER_DAY).unwrap_or(0);
    let to = get_u64(args, "to_day").map(|d| d * TICKS_PER_DAY).unwrap_or(u64::MAX);
    let limit = get_usize(args, "limit").unwrap_or(60);
    let who = get_str(args, "who").map(|w| r.who(w)).transpose()?;
    let ok_text = |l: &str| text.as_ref().map_or(true, |t| l.to_lowercase().contains(t));
    let mut lines: Vec<String> = match kind {
        "moments" => c.moments.iter().filter(|m| m.tick >= from && m.tick < to)
            .map(|m| format!("{}  {}{}: {} (because {})", colony::stamp(m.tick), if m.major() { "" } else { "(minor) " }, m.title, m.text, m.because))
            .filter(|l| ok_text(l) && who.map_or(true, |i| l.contains(&c.settlers[i].name))).collect(),
        "decisions" => all_spans(r).iter().filter(|x| x.t0 >= from && x.t0 < to && who.map_or(true, |i| x.who == i))
            .map(|x| format!("{:<10} {}", c.settlers[x.who].name, span_line(r, x))).filter(|l| ok_text(l)).collect(),
        _ => c.log.iter().filter(|l| colony::line_tick(l).map_or(true, |t| t >= from && t < to))
            .filter(|l| ok_text(l) && who.map_or(true, |i| l.contains(&c.settlers[i].name))).cloned().collect(),
    };
    let total = lines.len();
    if get_bool(args, "newest_first") { lines.reverse(); }
    lines.truncate(limit);
    Ok(Output::text(format!("{total} {kind} lines{}\n{}", if total > limit { format!(" (first {limit})") } else { String::new() }, lines.join("\n"))))
}

fn why(s: &mut Session, args: &Value) -> ToolResult {
    let ci = s.colony_index(args)?;
    let r = &s.colonies[ci];
    let c = &r.colony;
    let i = r.who(get_str(args, "who").unwrap_or(""))?;
    let at = match get_str(args, "at") { Some(a) => colony::parse_when(a).ok_or_else(|| format!("cannot read the moment '{a}'"))?, None => c.clock.tick };
    let spans: Vec<colony::Span> = all_spans(r).into_iter().filter(|x| x.who == i).collect();
    let k = spans.iter().rposition(|x| x.t0 <= at).ok_or_else(|| format!("{} had made no choice by {}", c.settlers[i].name, colony::stamp(at)))?;
    let near = |samples: &Vec<(u64, Vec<[i32; 6]>)>| samples.iter().rev().find(|x| x.0 <= at).and_then(|x| x.1.get(i).copied());
    let pos = r.rec.pos.iter().rev().find(|x| x.0 <= at).and_then(|x| x.1.get(i).copied());
    let needs = near(&r.rec.needs);
    let series = r.rec.series.iter().rev().find(|x| x.0 <= at);
    let thoughts: Vec<String> = c.settlers[i].mind.thoughts.iter().chain(c.settlers[i].mind.scars.iter()).filter(|t| t.tick <= at && t.tick + 3 * TICKS_PER_DAY >= at)
        .map(|t| format!("{} ({:+.2}) {}", colony::stamp(t.tick), t.weight, t.text)).collect();
    let log_near: Vec<&String> = c.log.iter().filter(|l| colony::line_tick(l).map_or(false, |t| t + 120 >= at && t <= at + 30) && l.contains(&c.settlers[i].name)).collect();
    Ok(Output::json(json!({
        "who": c.settlers[i].name, "at": colony::stamp(at),
        "doing": span_line(r, &spans[k]),
        "before": spans[k.saturating_sub(8)..k].iter().map(|x| span_line(r, x)).collect::<Vec<_>>(),
        "after": spans.iter().skip(k + 1).take(4).map(|x| span_line(r, x)).collect::<Vec<_>>(),
        "place": pos.map(|p| format!("{},{} level {}{}", p[0], p[1], p[2], if p[3] & 2 != 0 { " (below the ground)" } else { "" })),
        "needs": needs.map(|n| format!("hunger {}%, fatigue {}%, exposure {}%, stress {:.2}, focus {:.2}, stuck {} ticks so far", n[0], n[1], n[2], n[3] as f32 / 100.0, n[4] as f32 / 100.0, n[5])),
        "camp": series.map(|x| format!("{} meals stored ({:.1} days, goal {}), {} logs, {} stone, {} alive", x.1[0], x.1[9], x.1[10], x.1[1], x.1[2], x.1[3])),
        "thoughts_then": thoughts, "log_then": log_near,
        "note": "thoughts are the ten (and the scars) still remembered now: older ones are gone",
    })))
}

fn track(s: &mut Session, args: &Value) -> ToolResult {
    let ci = s.colony_index(args)?;
    let r = &s.colonies[ci];
    let i = r.who(get_str(args, "who").unwrap_or(""))?;
    let to = match get_str(args, "to") { Some(a) => colony::parse_when(a).ok_or_else(|| format!("cannot read '{a}'"))?, None => r.colony.clock.tick };
    let from = match get_str(args, "from") { Some(a) => colony::parse_when(a).ok_or_else(|| format!("cannot read '{a}'"))?, None => to.saturating_sub(TICKS_PER_DAY) };
    let spans = all_spans(r);
    let mine: Vec<&colony::Span> = spans.iter().filter(|x| x.who == i).collect();
    let mut lines = Vec::new();
    let mut walked = 0i32;
    let mut last: Option<[i32; 4]> = None;
    for (t, row) in r.rec.pos.iter().filter(|p| p.0 >= from && p.0 <= to) {
        let Some(p) = row.get(i).copied() else { continue };
        if let Some(l) = last { walked += (p[0] - l[0]).abs().max((p[1] - l[1]).abs()); }
        let job = mine.iter().rev().find(|x| x.t0 <= *t).map(|x| format!("{} ({})", VERBS[x.verb as usize], clip(&r.rec.whys[x.why as usize], 60))).unwrap_or_default();
        let moved = last.map_or(true, |l| l[0] != p[0] || l[1] != p[1] || l[2] != p[2]);
        lines.push(format!("{}  {:>3},{:<3} z {:<3}{}{}{}  {}", colony::stamp(*t), p[0], p[1], p[2], if p[3] & 2 != 0 { " below" } else { "      " }, if p[3] & 256 != 0 { " walking" } else { "        " }, if moved { "" } else { " (still)" }, job));
        last = Some(p);
    }
    let camp = r.colony.camp;
    Ok(Output::text(format!("{} from {} to {}: {} cells walked (camp fire at {},{} level {})\n{}", r.colony.settlers[i].name, colony::stamp(from), colony::stamp(to), walked, camp.0, camp.1, r.colony.camp_z(), lines.join("\n"))))
}

fn clip(s: &str, n: usize) -> String { if s.chars().count() > n { format!("{}...", s.chars().take(n).collect::<String>()) } else { s.to_string() } }

/// "camp", a settler, or "x,y[,z]" as a place on the map.
fn place(r: &ColonyRun, s: &str) -> Result<crate::colony::nav::P3, String> {
    let c = &r.colony;
    let t = s.trim();
    if t.eq_ignore_ascii_case("camp") || t.eq_ignore_ascii_case("fire") { return Ok(crate::colony::nav::surface3(&c.map, c.camp)); }
    let nums: Vec<i64> = t.split(|ch: char| ch == ',' || ch.is_whitespace()).filter(|x| !x.is_empty()).filter_map(|x| x.parse().ok()).collect();
    if nums.len() >= 2 && t.chars().next().map_or(false, |ch| ch.is_ascii_digit()) {
        let (x, y) = (nums[0].clamp(0, c.map.width as i64 - 1) as u16, nums[1].clamp(0, c.map.height as i64 - 1) as u16);
        let p = crate::colony::nav::surface3(&c.map, (x, y));
        return Ok(if nums.len() >= 3 { (x, y, nums[2] as i32) } else { p });
    }
    let i = r.who(t)?;
    Ok(c.here3(i))
}

fn cell(s: &mut Session, args: &Value) -> ToolResult {
    let ci = s.colony_index(args)?;
    let r = &s.colonies[ci];
    let c = &r.colony;
    let x = get_usize(args, "x").ok_or("needs x")?.min(c.map.width - 1);
    let y = get_usize(args, "y").ok_or("needs y")?.min(c.map.height - 1);
    let k = y * c.map.width + x;
    let ground = c.map.surface_z[k];
    let z = args.get("z").and_then(Value::as_i64).map(|z| z as i32).unwrap_or(ground);
    let nav = crate::colony::nav::standable(&c.map, x, y, z);
    let p = (x as u16, y as u16);
    let from = crate::colony::nav::surface3(&c.map, c.camp);
    let way = crate::colony::nav::path3(&c.map, Some(&c.steps), from, (p.0, p.1, z), 200_000);
    let cellz = |zz: i32| (zz >= 0 && (zz as usize) < c.map.depth).then(|| { let q = c.map.cell(x, y, zz as usize); format!("{:?} {:?} water {}", q.material, q.shape, q.water) });
    let near = |a: (u16, u16), d: i32| (a.0 as i32 - x as i32).abs().max((a.1 as i32 - y as i32).abs()) <= d;
    Ok(Output::json(json!({
        "cell": [x, y, z], "ground_level": ground, "levels": { "below": cellz(z - 1), "at": cellz(z), "above": cellz(z + 1) },
        "material": c.material_at(p, Some(z)), "standable": nav, "passable_surface": crate::colony::nav::passable(&c.map, p),
        "roof": c.map.roofs[k] != 0, "feature": format!("{:?}", c.map.features[k]), "footsteps": c.steps.get(k), "building": c.building_at(p),
        "marks": c.marks.iter().filter(|m| near(m.at, 2)).map(|m| format!("{:?} {}", m.kind, m.title)).collect::<Vec<_>>(),
        "patron_marks": c.patron.marks.iter().filter(|m| near(m.at, m.radius as i32)).map(|m| if m.forbidden { "forbidden ground" } else { "blessed ground" }).collect::<Vec<_>>(),
        "settlers_near": c.settlers.iter().filter(|st| st.alive && near(st.pos, 3)).map(|st| format!("{} ({}, level {})", st.name, st.job.verb(), st.z)).collect::<Vec<_>>(),
        "creatures_near": c.creatures.iter().filter(|cr| near(cr.pos, 6)).map(|cr| format!("{:?} {}", cr.kind, cr.name)).collect::<Vec<_>>(),
        "places_near": c.map.places.iter().filter(|pl| pl.mouth.map_or(false, |m| near(m, 6))).map(|pl| pl.name.clone()).collect::<Vec<_>>(),
        "from_the_fire": match &way { Some(w) => json!(format!("reachable: {} steps", w.len())), None => json!("not reachable from the fire (within 200,000 nodes)") },
    })))
}

fn path(s: &mut Session, args: &Value) -> ToolResult {
    let ci = s.colony_index(args)?;
    let r = &s.colonies[ci];
    let c = &r.colony;
    let a = place(r, get_str(args, "from").ok_or("needs from")?)?;
    let b = place(r, get_str(args, "to").ok_or("needs to")?)?;
    let t0 = std::time::Instant::now();
    let way = crate::colony::nav::path3(&c.map, Some(&c.steps), a, b, 400_000);
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    match way {
        Some(w) => {
            let levels: Vec<i32> = { let mut l: Vec<i32> = w.iter().map(|p| p.2).collect(); l.dedup(); l };
            let shown: Vec<String> = w.iter().step_by((w.len() / 24).max(1)).map(|p| format!("{},{},{}", p.0, p.1, p.2)).collect();
            Ok(Output::json(json!({ "from": [a.0, a.1, a.2], "to": [b.0, b.1, b.2], "steps": w.len(), "levels": levels, "ms": (ms * 10.0).round() / 10.0, "way": shown })))
        }
        None => {
            // How far one gets: the walkable cells reached from the start (to a limit), and the
            // one nearest the goal.
            let mut seen = std::collections::HashSet::new();
            let mut q = std::collections::VecDeque::new();
            seen.insert(a);
            q.push_back(a);
            let mut best = (i64::MAX, a);
            let mut out = Vec::new();
            while let Some(p) = q.pop_front() {
                let d = (p.0 as i64 - b.0 as i64).abs().max((p.1 as i64 - b.1 as i64).abs()) * 4 + (p.2 as i64 - b.2 as i64).abs();
                if d < best.0 { best = (d, p); }
                if seen.len() > 300_000 { break; }
                out.clear();
                crate::colony::nav::steps3(&c.map, p, &mut out);
                for (n, _) in out.iter() { if seen.insert(*n) { q.push_back(*n); } }
            }
            Ok(Output::json(json!({
                "from": [a.0, a.1, a.2], "to": [b.0, b.1, b.2], "way": null, "ms": (ms * 10.0).round() / 10.0,
                "goal_standable": crate::colony::nav::standable(&c.map, b.0 as usize, b.1 as usize, b.2),
                "reachable_from_start": seen.len(), "nearest_reachable": [best.1 .0, best.1 .1, best.1 .2],
                "nearest_is_off_by": format!("{} cells across, {} levels", (best.1 .0 as i64 - b.0 as i64).abs().max((best.1 .1 as i64 - b.1 as i64).abs()), (best.1 .2 - b.2).abs()),
            })))
        }
    }
}

fn act(s: &mut Session, args: &Value) -> ToolResult {
    let ci = s.colony_index(args)?;
    let r = &mut s.colonies[ci];
    let a = get_str(args, "act").ok_or("needs act")?;
    match r.act(a) {
        Ok(msg) => Ok(Output::text(format!("{}: {msg}\n(recorded as \"{}\")", r.colony.clock.stamp(), r.script.last().cloned().unwrap_or_default()))),
        Err(e) => Err(e),
    }
}

fn render(s: &mut Session, args: &Value) -> ToolResult {
    use crate::tiles::render::{render_local, LocalCamera};
    let ci = s.colony_index(args)?;
    let wid = s.colonies[ci].world.clone();
    let wi = s.world_index(&json!({ "world": wid }))?;
    s.atlas();
    let view = get_str(args, "view").unwrap_or("camp").to_string();
    let out = s.out_path(&format!("colony_{}_{}_t{}.png", safe(&s.colonies[ci].id), view, s.colonies[ci].colony.clock.tick))?;
    let Session { worlds, colonies, atlas, .. } = s;
    let hist = worlds[wi].history.as_ref();
    let atlas = atlas.as_ref().unwrap();
    let r = &mut colonies[ci];
    let c = &mut r.colony;
    match view.as_str() {
        "map" => { let px = args.get("px").and_then(Value::as_f64).unwrap_or(4.0) as f32; colony::render_map(c, hist, atlas, px, &out); }
        "level" => {
            let z = args.get("z").and_then(Value::as_i64).map(|z| z as i32).or_else(|| c.delve_levels().into_iter().find(|&z| z < c.camp_z())).unwrap_or(c.camp_z());
            colony::render_level(c, hist, z, args.get("px").and_then(Value::as_f64).unwrap_or(4.0) as f32, &out);
        }
        _ => {
            let (w, h) = (1280usize, 800usize);
            let px = args.get("px").and_then(Value::as_f64).unwrap_or(16.0) as f32;
            let cx = get_usize(args, "x").map(|x| x as f32).unwrap_or(c.camp.0 as f32 + 0.5);
            let cy = get_usize(args, "y").map(|y| y as f32).unwrap_or(c.camp.1 as f32 + 0.5);
            let cam = LocalCamera { cx, cy, tile_px: px, z: 0, surface_view: true };
            let mut buf = vec![0u32; w * h];
            render_local(&c.map, atlas, &cam, &mut buf, w, h);
            crate::tiles::local_ink::draw_colony(c, &cam, &mut buf, w, h, hist);
            let path = out.to_string_lossy().to_string();
            crate::tiles::viewer::save_rgb_png_pub(&path, w, h, |x, y| { let q = buf[y * w + x]; [(q >> 16) as u8, (q >> 8) as u8, q as u8] });
        }
    }
    Ok(Output::text(format!("{} at {}", view, c.clock.stamp())).with_files(vec![out]))
}

/// A finding of the checks: what, how many, and examples.
fn finding(what: &str, n: usize, bad: bool, examples: Vec<String>) -> Value {
    json!({ "check": what, "count": n, "flag": bad, "examples": examples.into_iter().take(8).collect::<Vec<_>>() })
}

pub fn check_findings(r: &ColonyRun) -> Vec<Value> {
    let c = &r.colony;
    let rec = &r.rec;
    let name = |i: usize| c.settlers.get(i).map_or("?".to_string(), |s| s.name.clone());
    let mut out = Vec::new();
    // Deaths, and what the store held then.
    let series_at = |t: u64| rec.series.iter().rev().find(|x| x.0 <= t).map(|x| x.1);
    let deaths: Vec<String> = (0..c.settlers.len()).filter(|&i| !c.settlers[i].alive && c.settlers[i].away_until == 0 && !c.settlers[i].mind.left).map(|i| {
        let t = rec.status.iter().rev().find(|x| x.1 == i && !x.2).map_or(0, |x| x.0);
        format!("{} {}{}", name(i), c.fate(i), series_at(t).map(|s| format!(" (the store held {} meals)", s[0])).unwrap_or_default())
    }).collect();
    let starved_with_food = deaths.iter().filter(|d| d.contains("hunger") || d.contains("starv")).filter(|d| !d.contains("held 0 meals")).count();
    out.push(finding("deaths", deaths.len(), starved_with_food > 0, deaths));
    // Hungry (95%+) for three hours running while the store held food.
    let mut hungry: Vec<String> = Vec::new();
    let mut run_len = vec![0usize; c.settlers.len()];
    for (k, (t, row)) in rec.needs.iter().enumerate() {
        let food = rec.series.get(k).map_or(0.0, |s| s.1[0]);
        for (i, n) in row.iter().enumerate() {
            let alive = rec.pos.iter().rev().find(|p| p.0 <= *t).and_then(|p| p.1.get(i)).map_or(false, |q| q[3] & 1 != 0 && q[3] & 32 == 0);
            if alive && n[0] >= 95 && food >= 1.0 { run_len[i] += 1; } else { run_len[i] = 0; }
            if run_len[i] == 3 { hungry.push(format!("{} from {}: hunger {}% with {} meals stored", name(i), colony::stamp(t - 2 * rec.slow), n[0], food)); }
        }
    }
    out.push(finding("hungry 3+ hours with food in the store", hungry.len(), !hungry.is_empty(), hungry));
    // Stuck: ticks spent wanting to move and unable.
    let mut stuck: Vec<(usize, u32)> = c.settlers.iter().enumerate().map(|(i, s)| (i, s.stuck)).filter(|x| x.1 > 0).collect();
    stuck.sort_by(|a, b| b.1.cmp(&a.1));
    let total_stuck: u32 = stuck.iter().map(|x| x.1).sum();
    out.push(finding("ticks stuck (wanting to move, unable)", total_stuck as usize, stuck.iter().any(|x| x.1 > 100), stuck.iter().map(|(i, n)| format!("{} {} ticks", name(*i), n)).collect()));
    out.push(finding("standing where no one can stand", rec.unstandable_count, rec.unstandable_count > 0,
        rec.unstandable.iter().map(|u| format!("{} {} at {},{} level {} ({})", colony::stamp(u.0), name(u.1), u.2, u.3, u.4, VERBS[u.5 as usize])).collect()));
    out.push(finding("jumps no walk could make", rec.jumps.len(), !rec.jumps.is_empty(),
        rec.jumps.iter().map(|j| format!("{} {} {},{} -> {},{}", colony::stamp(j.0), name(j.1), j.2 .0, j.2 .1, j.3 .0, j.3 .1)).collect()));
    let spans = all_spans(r);
    // Jobs held a day or more (sleep, the bench, an expedition aside).
    let long: Vec<String> = spans.iter().filter(|x| x.t1 - x.t0 >= TICKS_PER_DAY && !matches!(x.verb, 2 | 12))
        .map(|x| format!("{} {}", name(x.who), span_line(r, x))).collect();
    out.push(finding("a job held a day or more", long.len(), !long.is_empty(), long));
    // Minds changed over and over: eight or more choices within an hour.
    let mut thrash: Vec<String> = Vec::new();
    let mut by_who: Vec<Vec<u64>> = vec![Vec::new(); c.settlers.len()];
    for x in &spans { by_who[x.who].push(x.t0); }
    for (i, ts) in by_who.iter().enumerate() {
        let mut k0 = 0;
        let mut hits = 0;
        let mut first = None;
        for k in 0..ts.len() {
            while ts[k] - ts[k0] > 60 { k0 += 1; }
            if k - k0 + 1 >= 8 { hits += 1; first.get_or_insert(ts[k0]); }
        }
        if hits > 0 { thrash.push(format!("{}: {} times, first at {}", name(i), hits, colony::stamp(first.unwrap()))); }
    }
    out.push(finding("eight choices within an hour", thrash.len(), !thrash.is_empty(), thrash));
    // Idle shares.
    let mut idle: Vec<(usize, f64)> = (0..c.settlers.len()).map(|i| {
        let mine: Vec<&colony::Span> = spans.iter().filter(|x| x.who == i).collect();
        let all: u64 = mine.iter().map(|x| x.t1 - x.t0).sum();
        let id: u64 = mine.iter().filter(|x| x.verb == 0 || (x.verb == 8 && r.rec.whys[x.why as usize].to_lowercase().contains("nothing"))).map(|x| x.t1 - x.t0).sum();
        (i, if all > 0 { id as f64 / all as f64 } else { 0.0 })
    }).collect();
    idle.sort_by(|a, b| b.1.total_cmp(&a.1));
    out.push(finding("idle share of waking time (highest)", idle.iter().filter(|x| x.1 > 0.2).count(), idle.first().map_or(false, |x| x.1 > 0.35),
        idle.iter().take(6).map(|(i, f)| format!("{} {:.0}%", name(*i), f * 100.0)).collect()));
    // Works stalled (no load for 8+ days, not done) or set aside.
    let now = c.clock.tick;
    let stalled: Vec<String> = rec.works.iter().filter(|w| w.done_at.is_none() && w.gone_at.is_none()).filter(|w| now - w.steps.last().map_or(w.first, |s| s.0) >= 8 * TICKS_PER_DAY)
        .map(|w| format!("{} at {},{}: {}/{} since {}", w.word, w.key.1 .0, w.key.1 .1, w.used, w.needed, colony::stamp(w.steps.last().map_or(w.first, |s| s.0)))).collect();
    out.push(finding("works stalled 8+ days", stalled.len(), !stalled.is_empty(), stalled));
    let gone: Vec<String> = rec.works.iter().filter(|w| w.gone_at.is_some()).map(|w| format!("{} at {},{}: {}/{} laid, gone {}", w.word, w.key.1 .0, w.key.1 .1, w.used, w.needed, colony::stamp(w.gone_at.unwrap()))).collect();
    out.push(finding("works set aside or struck off", gone.len(), false, gone));
    // Days with nothing in the log.
    let ticks: Vec<u64> = c.log.iter().filter_map(|l| colony::line_tick(l)).collect();
    let mut gaps: Vec<String> = Vec::new();
    for w in ticks.windows(2) { if w[1] > w[0] + 2 * TICKS_PER_DAY { gaps.push(format!("{} to {}", colony::stamp(w[0]), colony::stamp(w[1]))); } }
    out.push(finding("two days or more with nothing in the log", gaps.len(), false, gaps));
    out.push(finding("patron's acts that failed", r.failures.len(), !r.failures.is_empty(), r.failures.clone()));
    let loose_max = rec.series.iter().map(|x| x.1[5]).fold(0.0f32, f32::max);
    let loose_now = rec.series.last().map_or(0.0, |x| x.1[5]);
    out.push(finding("things left lying (now / most)", loose_now as usize, loose_now > 40.0, vec![format!("{loose_now} now, {loose_max} at most")]));
    out
}

fn checks(s: &mut Session, args: &Value) -> ToolResult {
    let ci = s.colony_index(args)?;
    let mut findings = check_findings(&s.colonies[ci]);
    if get_bool(args, "replay") {
        let r = &s.colonies[ci];
        let (wid, tile, n, script, ticks) = (r.world.clone(), r.tile, r.settlers_asked, r.script.clone(), r.colony.clock.tick - r.rec.start_tick);
        let wi = s.world_index(&json!({ "world": wid }))?;
        let e = &s.worlds[wi];
        let (mut col, _) = colony::found(&e.world, e.history.as_ref(), tile, n)?;
        let t0 = std::time::Instant::now();
        let mut applied = 0u64;
        let end = col.clock.tick + ticks;
        while col.clock.tick < end {
            let now = col.clock.tick;
            for line in &script { let at = line.split_whitespace().next().and_then(|t| t.parse::<u64>().ok()).unwrap_or(u64::MAX); if at >= applied && at <= now { let _ = col.apply_intervention(line); } }
            applied = now + 1;
            col.tick();
        }
        let a = &s.colonies[ci].colony.log;
        let b = &col.log;
        let first = a.iter().zip(b.iter()).position(|(x, y)| x != y);
        let same = first.is_none() && a.len() == b.len();
        findings.push(finding("a second run from the founding tells the same story", if same { 0 } else { 1 }, !same,
            vec![if same { format!("{} log lines identical; unrecorded replay took {:.2} s", a.len(), t0.elapsed().as_secs_f64()) } else {
                match first { Some(k) => format!("line {k} differs:\n  {}\n  {}", a[k], b[k]), None => format!("{} lines against {}", a.len(), b.len()) } }]));
    }
    Ok(Output::json(json!({ "colony": s.colonies[ci].id, "now": s.colonies[ci].colony.clock.stamp(), "findings": findings })))
}

fn branch(s: &mut Session, args: &Value) -> ToolResult {
    let ci = s.colony_index(args)?;
    let (wid, tile, n, script, ticks, base_id, every, frame_days) = {
        let r = &s.colonies[ci];
        (r.world.clone(), r.tile, r.settlers_asked, r.script.clone(), r.colony.clock.tick - r.rec.start_tick, r.id.clone(), r.rec.every, r.rec.frame_days)
    };
    let fork = match (get_str(args, "at"), args.get("from_day").and_then(Value::as_f64)) {
        (Some(a), _) => colony::parse_when(a).ok_or_else(|| format!("cannot read '{a}'"))?,
        (None, Some(d)) => ((d.max(1.0) - 1.0) * TICKS_PER_DAY as f64) as u64 + 6 * 60,
        _ => return Err("needs from_day or at".into()),
    };
    let acts: Vec<String> = args.get("acts").and_then(Value::as_array).map(|a| a.iter().filter_map(|v| v.as_str()).map(|v| format!("{fork} {}", v.trim())).collect()).unwrap_or_default();
    let drop = args.get("drop").and_then(Value::as_bool).unwrap_or(true);
    let mut new_script: Vec<String> = script.iter().filter(|l| !drop || l.split_whitespace().next().and_then(|t| t.parse::<u64>().ok()).map_or(true, |t| t < fork)).cloned().collect();
    new_script.extend(acts.iter().cloned());
    let id = get_str(args, "id").map(str::to_string).unwrap_or_else(|| format!("{}~{}", base_id, colony::stamp(fork).replace("Day ", "d").replace(", ", "_").replace(':', "")));
    let wi = s.world_index(&json!({ "world": wid }))?;
    let e = &s.worlds[wi];
    let (col, seed) = colony::found(&e.world, e.history.as_ref(), tile, n)?;
    let dir = s.out_dir.join(format!("colony_{}", safe(&id)));
    let _ = std::fs::remove_dir_all(dir.join("frames"));
    let run = ColonyRun::new(id.clone(), wid, tile, seed, n, col, new_script, every, frame_days, dir);
    s.colonies.retain(|c| c.id != id);
    s.colonies.push(run);
    let bi = s.colonies.len() - 1;
    let t0 = std::time::Instant::now();
    s.live(bi, ticks, None)?;
    let secs = t0.elapsed().as_secs_f64();
    let ci = s.colonies.iter().position(|c| c.id == base_id).unwrap();
    let (a, b) = (&s.colonies[ci].colony, &s.colonies[bi].colony);
    let before_fork = |l: &&String| colony::line_tick(l).map_or(true, |t| t < fork);
    let pre_a: Vec<&String> = a.log.iter().filter(before_fork).collect();
    let pre_b: Vec<&String> = b.log.iter().filter(before_fork).collect();
    let replay_matches = pre_a == pre_b;
    let first = a.log.iter().zip(b.log.iter()).position(|(x, y)| x != y);
    let fates = |c: &crate::colony::Colony| c.settlers.iter().enumerate().map(|(i, st)| format!("{}: {}", st.name, c.fate(i))).collect::<Vec<_>>();
    let only = |x: &crate::colony::Colony, y: &crate::colony::Colony| x.projects.iter().filter(|p| p.done && !y.projects.iter().any(|q| q.done && q.kind == p.kind)).map(|p| p.kind.word().to_string()).collect::<Vec<_>>();
    let deaths = |c: &crate::colony::Colony| c.settlers.iter().filter(|s| !s.alive && s.away_until == 0 && !s.mind.left).count();
    s.current_colony = Some(base_id.clone());
    Ok(Output::json(json!({
        "branch": id, "fork": colony::stamp(fork), "acts": acts, "secs": (secs * 100.0).round() / 100.0,
        "replay_matches_before_fork": replay_matches,
        "first_difference": first.map(|k| json!({ "line": k, "original": a.log[k], "branch": b.log[k] })),
        "original": { "alive": format!("{} of {}", a.alive(), a.company()), "deaths": deaths(a), "works_done": a.projects.iter().filter(|p| p.done).count(), "made": a.works.len(), "moments": a.moments.len() },
        "branched": { "alive": format!("{} of {}", b.alive(), b.company()), "deaths": deaths(b), "works_done": b.projects.iter().filter(|p| p.done).count(), "made": b.works.len(), "moments": b.moments.len() },
        "works_only_in_original": only(a, b), "works_only_in_branch": only(b, a),
        "fates_differ": fates(a).iter().zip(fates(b).iter()).filter(|(x, y)| x != y).map(|(x, y)| format!("{x}  |  {y}")).collect::<Vec<_>>(),
        "note": format!("the branch is colony '{id}'; colony_explorer {{\"compare\":\"{id}\"}} shows both"),
    })))
}

fn explorer(s: &mut Session, args: &Value) -> ToolResult {
    let ci = s.colony_index(args)?;
    let wid = s.colonies[ci].world.clone();
    let wi = s.world_index(&json!({ "world": wid }))?;
    s.atlas();
    let dir = get_str(args, "dir").map(std::path::PathBuf::from).unwrap_or_else(|| s.colonies[ci].dir.clone());
    let with_history = args.get("history").and_then(Value::as_bool).unwrap_or(true);
    let mut files = Vec::new();
    // The history explorer beside it, for the settlers' pasts.
    let link = if with_history {
        let hdir = s.out_dir.join(format!("{}_explorer", safe(&wid)));
        let e = &s.worlds[wi];
        if let Some(h) = e.history.as_ref() {
            let fresh = !hdir.join("index.html").exists();
            if fresh { super::explorer::write(&e.world, h, &e.id, s.atlas.as_ref().unwrap(), &hdir, e.world.width <= 128).map_err(|err| err.to_string())?; }
            Some(relative(&dir, &hdir.join("index.html")))
        } else { None }
    } else { None };
    let other = match get_str(args, "compare") {
        Some(o) => Some(s.colonies.iter().position(|c| c.id == o).ok_or_else(|| format!("no colony '{o}'"))?),
        None => None,
    };
    let Session { worlds, colonies, atlas, .. } = s;
    let hist = worlds[wi].history.as_ref();
    let w = super::colony_explorer::write(&mut colonies[..], ci, other, hist, atlas.as_ref().unwrap(), &dir, link.as_deref()).map_err(|e| e.to_string())?;
    let index = w.dir.join("index.html");
    files.push(index.clone());
    if get_bool(args, "open") { let _ = std::process::Command::new("open").arg(&index).status(); }
    Ok(Output::text(format!("The colony explorer for {}: {} decisions, {} position samples, {} frames, {} levels; data {:.1} MB{}.\nOpen {}",
        colonies[ci].id, w.decisions, w.samples, w.frames, w.levels, w.data_bytes as f64 / 1e6,
        link.map(|l| format!("; the settlers' pasts link to the history explorer ({l})")).unwrap_or_default(), index.display())).with_files(files))
}

/// `to` as a path relative to the folder `from` (both under the same root).
fn relative(from: &std::path::Path, to: &std::path::Path) -> String {
    let abs = |p: &std::path::Path| std::env::current_dir().map(|d| d.join(p)).unwrap_or_else(|_| p.to_path_buf());
    let (a, b) = (abs(from), abs(to));
    let ac: Vec<_> = a.components().collect();
    let bc: Vec<_> = b.components().collect();
    let common = ac.iter().zip(bc.iter()).take_while(|(x, y)| x == y).count();
    let mut out: Vec<String> = std::iter::repeat("..".to_string()).take(ac.len() - common).collect();
    out.extend(bc[common..].iter().map(|c| c.as_os_str().to_string_lossy().to_string()));
    out.join("/")
}
