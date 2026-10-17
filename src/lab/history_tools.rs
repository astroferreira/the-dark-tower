//! The lab's history debugger: query the chronicle, read an event whole, walk its causes back
//! and its effects forward, follow an entity through time, measure a history (causes,
//! contradictions, wars, tales), export it for ad-hoc analysis, and branch it: the same past to
//! a chosen year, then a different roll of the dice, compared with the history as it was.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use serde_json::{json, Value};

use super::*;
use crate::history::events::Event;
use crate::history::EntityId;

pub fn tools() -> Vec<Tool> {
    vec![
        Tool { name: "history_events", schema: events_schema, run: history_events,
            description: "Query the chronicle: events filtered by year range, kind (substring of the EventType, e.g. \"Siege\", \"War\"), text in title or description, place (tile `x`,`y` within `radius` tiles), an entity named in it (substring of a person's, town's, people's, beast's... name), major only, with or without a recorded cause. Returns the matches' count by kind and a page of them (id, year, season, kind, title, place)." },
        Tool { name: "history_event", schema: id_schema, run: history_event,
            description: "One event whole: text, kind, date, place (with the tile's biome), participants and peoples by name, outcome and consequences, the events it was caused by and the events it caused (both directions, from the causal links), artifacts and monuments made, the age/war/battle it belongs to, and the tale worth telling it is part of." },
        Tool { name: "history_why", schema: why_schema, run: history_why,
            description: "The causal chain of an event as an indented tree: `back` (default) follows causes and triggered_by to the roots, `forward` follows what it led to, `both` does both. Answers \"why did this happen?\" and \"what came of it?\". Depth default 6, at most 80 events." },
        Tool { name: "history_entity", schema: entity_schema, run: history_entity,
            description: "Find a person, town, people, beast, artifact, monument, war, faith, god or house by name (substring) or by `type` and `id`, and return its record and its timeline (every event naming it, in order). Several matches return the candidates." },
        Tool { name: "history_stats", schema: || schema(json!({ "world": world_prop() }), &[]), run: history_stats,
            description: "A history measured: summary counts, events by kind and by century, the share of events with a recorded cause by kind and the chain depth, contradictions (dead rulers ruling, figures dying twice...), the wars' and objects' reports, people in places, the present day, the tales worth telling, the world's records sentence and its rare outcomes." },
        Tool { name: "history_export", schema: export_schema, run: history_export,
            description: "Write the history as files for ad-hoc analysis (jq, duckdb, pandas): events.jsonl (every event with participants, peoples, causes), entities.jsonl (every named thing with its type, id and dates) and edges.csv (cause -> effect). Returns the paths." },
        Tool { name: "history_explorer", schema: explorer_schema, run: history_explorer,
            description: "Write the history explorer for a world: one HTML app (open index.html in a browser) with a reader in the manner of Dwarf Fortress's legends (every person, people, town, war, house, beast, year and age as a linked page), a zoomable timeline of the peoples with their wars and events, the causal graph round any event, family trees, the web of peoples (wars, raids, treaties, trade, marriages) and of a person, the map with territories and towns at any year (it plays), and checks (contradictions, causes by kind, events without a place, causal hubs and cycles, chain depths, impossible lives and towns). The full legends pages are written beside it and linked. Returns the path." },
        Tool { name: "history_branch", schema: branch_schema, run: history_branch,
            description: "What if the dice had fallen otherwise after year N? Re-simulates the world's history with its own seed to `from_year`, then goes on twice: once as it was (the baseline replay) and once with the history's random stream reseeded (`seed`). Compares the two futures: peoples that survive in one and not the other, towns standing or fallen, wars, the present day, the records, the first major events after the fork. The branch is kept as a new world (id returned) that every history tool can query. Costs two history runs after the fork (dev world ~2 s, 512x256 tens of seconds)." },
    ]
}

// ---------------------------------------------------------------------------------------------
// Indexes over a history, built on first use.

pub struct HistIndex {
    /// Event id -> position in the chronicle.
    pos: HashMap<u64, usize>,
    /// Event id -> the events that name it as a cause or trigger (and its own triggered list).
    effects: HashMap<u64, Vec<u64>>,
    /// Entity -> positions of the events naming it (participant or people involved).
    by_entity: HashMap<EntityId, Vec<usize>>,
}

fn build_index(h: &WorldHistory) -> HistIndex {
    let mut pos = HashMap::new();
    let mut effects: HashMap<u64, Vec<u64>> = HashMap::new();
    let mut by_entity: HashMap<EntityId, Vec<usize>> = HashMap::new();
    for (i, e) in h.chronicle.events.iter().enumerate() {
        pos.insert(e.id.0, i);
        for c in e.causes.iter().chain(e.triggered_by.iter()) { effects.entry(c.0).or_default().push(e.id.0); }
        for t in &e.triggered_events { effects.entry(e.id.0).or_default().push(t.0); }
        for p in &e.primary_participants { by_entity.entry(p.clone()).or_default().push(i); }
        for f in &e.factions_involved { by_entity.entry(EntityId::Faction(*f)).or_default().push(i); }
    }
    for v in effects.values_mut() { v.sort_unstable(); v.dedup(); }
    for v in by_entity.values_mut() { v.sort_unstable(); v.dedup(); }
    HistIndex { pos, effects, by_entity }
}

/// The world's history and its index (built once per world).
fn hist(s: &mut Session, a: &Value) -> Result<usize, String> {
    let i = s.world_index(a)?;
    let e = &mut s.worlds[i];
    let Some(h) = e.history.as_ref() else {
        return Err(format!("world '{}' has no history: open it with history_years (or dev: true)", e.id));
    };
    if e.hist_index.is_none() { e.hist_index = Some(build_index(h)); }
    Ok(i)
}

fn parts(e: &WorldEntry) -> (&WorldHistory, &HistIndex) { (e.history.as_ref().unwrap(), e.hist_index.as_ref().unwrap()) }

fn event<'a>(h: &'a WorldHistory, ix: &HistIndex, id: u64) -> Option<&'a Event> { ix.pos.get(&id).map(|&p| &h.chronicle.events[p]) }

/// The name an entity goes by, with its type ("Figure 12: Agkh the Bold").
pub fn entity_name(h: &WorldHistory, id: &EntityId) -> String {
    let n = match id {
        EntityId::Faction(f) => h.factions.get(f).map(|x| x.name.clone()),
        EntityId::Settlement(s) => h.settlements.get(s).map(|x| x.name.clone()),
        EntityId::Figure(f) => h.figures.get(f).map(|x| x.full_name()),
        EntityId::Dynasty(d) => h.dynasties.get(d).map(|x| x.name.clone()),
        EntityId::LegendaryCreature(c) => h.legendary_creatures.get(c).map(|x| x.full_name()),
        EntityId::CreaturePopulation(p) => h.populations.get(p).and_then(|x| h.creature_species.get(&x.species_id)).map(|s| format!("a population of {}", s.name)),
        EntityId::Deity(d) => h.deities.get(d).map(|x| x.name.clone()),
        EntityId::Religion(r) => h.religions.get(r).map(|x| x.name.clone()),
        EntityId::Artifact(x) => h.artifacts.get(x).map(|x| x.name.clone()),
        EntityId::Monument(m) => h.monuments.get(m).map(|x| x.name.clone()),
    };
    n.unwrap_or_else(|| "(unknown)".into())
}

fn entity_ref(h: &WorldHistory, id: &EntityId) -> Value {
    let (t, n) = entity_key(id);
    json!({ "type": t, "id": n, "name": entity_name(h, id) })
}

fn entity_key(id: &EntityId) -> (&'static str, u64) {
    match id {
        EntityId::Faction(x) => ("people", x.0), EntityId::Settlement(x) => ("town", x.0), EntityId::Figure(x) => ("figure", x.0),
        EntityId::Dynasty(x) => ("house", x.0), EntityId::LegendaryCreature(x) => ("beast", x.0), EntityId::CreaturePopulation(x) => ("population", x.0),
        EntityId::Deity(x) => ("god", x.0), EntityId::Religion(x) => ("faith", x.0), EntityId::Artifact(x) => ("artifact", x.0), EntityId::Monument(x) => ("monument", x.0),
    }
}

fn brief(e: &Event) -> Value {
    json!({ "id": e.id.0, "year": e.date.year, "season": format!("{:?}", e.date.season), "kind": format!("{:?}", e.event_type),
        "title": e.title, "at": e.location.map(|(x, y)| json!([x, y])), "major": e.is_major })
}

fn line(e: &Event) -> String {
    format!("#{} y{} {:?}: {}{}", e.id.0, e.date.year, e.event_type, e.title, e.location.map_or(String::new(), |(x, y)| format!(" @{x},{y}")))
}

// ---------------------------------------------------------------------------------------------
// history_events

fn events_schema() -> Value {
    let (x, y) = tile_props();
    schema(json!({
        "world": world_prop(),
        "from_year": { "type": "integer" }, "to_year": { "type": "integer" },
        "kind": { "type": "string", "description": "Substring of the event kind (case-insensitive), e.g. \"siege\", \"founded\", \"beast\"" },
        "text": { "type": "string", "description": "Substring of title or description" },
        "x": x, "y": y, "radius": { "type": "integer", "description": "Tiles round x,y (default 0: that tile)" },
        "entity": { "type": "string", "description": "Substring of the name of anything the event names" },
        "major": { "type": "boolean", "description": "Only major events" },
        "caused": { "type": "boolean", "description": "true: only events with a recorded cause; false: only those without" },
        "newest_first": { "type": "boolean" },
        "limit": { "type": "integer", "description": "Default 50" }, "offset": { "type": "integer" },
    }), &[])
}

fn history_events(s: &mut Session, a: &Value) -> ToolResult {
    let i = hist(s, a)?;
    let (h, _) = parts(&s.worlds[i]);
    let from = get_usize(a, "from_year").map(|v| v as u32);
    let to = get_usize(a, "to_year").map(|v| v as u32);
    let kind = get_str(a, "kind").map(str::to_lowercase);
    let text = get_str(a, "text").map(str::to_lowercase);
    let ent = get_str(a, "entity").map(str::to_lowercase);
    let at = match (get_usize(a, "x"), get_usize(a, "y")) { (Some(x), Some(y)) => Some((x as i64, y as i64, get_usize(a, "radius").unwrap_or(0) as i64)), _ => None };
    let major = get_bool(a, "major");
    let caused = a.get("caused").and_then(Value::as_bool);
    let w = s.worlds[i].world.width as i64;
    let mut m: Vec<&Event> = h.chronicle.events.iter().filter(|e| {
        from.map_or(true, |f| e.date.year >= f) && to.map_or(true, |t| e.date.year <= t)
            && (!major || e.is_major)
            && kind.as_ref().map_or(true, |k| format!("{:?}", e.event_type).to_lowercase().contains(k.as_str()))
            && text.as_ref().map_or(true, |t| e.title.to_lowercase().contains(t.as_str()) || e.description.to_lowercase().contains(t.as_str()))
            && caused.map_or(true, |c| c == (!e.causes.is_empty() || e.triggered_by.is_some()))
            && at.map_or(true, |(x, y, r)| e.location.map_or(false, |(ex, ey)| {
                let dx = (ex as i64 - x).rem_euclid(w); let dx = dx.min(w - dx);
                dx <= r && (ey as i64 - y).abs() <= r
            }))
            && ent.as_ref().map_or(true, |n| e.primary_participants.iter().any(|p| entity_name(h, p).to_lowercase().contains(n.as_str()))
                || e.factions_involved.iter().any(|f| entity_name(h, &EntityId::Faction(*f)).to_lowercase().contains(n.as_str())))
    }).collect();
    if get_bool(a, "newest_first") { m.reverse(); }
    let mut by_kind: HashMap<String, usize> = HashMap::new();
    for e in &m { *by_kind.entry(format!("{:?}", e.event_type)).or_default() += 1; }
    let mut kinds: Vec<(String, usize)> = by_kind.into_iter().collect();
    kinds.sort_by(|p, q| q.1.cmp(&p.1).then(p.0.cmp(&q.0)));
    let (offset, limit) = (get_usize(a, "offset").unwrap_or(0), get_usize(a, "limit").unwrap_or(50));
    Ok(Output::json(json!({
        "matching": m.len(), "of": h.chronicle.events.len(),
        "by_kind": kinds.iter().take(20).map(|(k, n)| json!([k, n])).collect::<Vec<_>>(),
        "offset": offset,
        "events": m.iter().skip(offset).take(limit).map(|e| brief(e)).collect::<Vec<_>>(),
    })))
}

// ---------------------------------------------------------------------------------------------
// history_event

fn id_schema() -> Value { schema(json!({ "world": world_prop(), "id": { "type": "integer", "description": "Event id" } }), &["id"]) }

fn history_event(s: &mut Session, a: &Value) -> ToolResult {
    let i = hist(s, a)?;
    let entry = &s.worlds[i];
    let (h, ix) = parts(entry);
    let id = get_u64(a, "id").ok_or("needs id")?;
    let e = event(h, ix, id).ok_or_else(|| format!("no event #{id}"))?;
    let ev = |id: u64| event(h, ix, id).map(brief).unwrap_or_else(|| json!({ "id": id, "missing": true }));
    let tale = crate::lore::sifting::sift(h).into_iter().find(|t| t.events.contains(&e.id))
        .map(|t| json!({ "kind": format!("{:?}", t.kind), "title": t.title, "text": t.text, "events": t.events.iter().map(|x| x.0).collect::<Vec<_>>() }));
    let place = e.location.map(|(x, y)| {
        let w = &entry.world;
        json!({ "tile": [x, y], "biome": w.biomes.get(x, y).display_name(),
            "settlement": h.tile_history.get(x, y).settlement.and_then(|s| h.settlements.get(&s)).map(|s| s.name.clone()) })
    });
    Ok(Output::json(json!({
        "id": e.id.0, "year": e.date.year, "season": format!("{:?}", e.date.season), "kind": format!("{:?}", e.event_type), "major": e.is_major,
        "title": e.title, "description": e.description, "place": place,
        "participants": e.primary_participants.iter().map(|p| entity_ref(h, p)).collect::<Vec<_>>(),
        "peoples": e.factions_involved.iter().map(|f| entity_ref(h, &EntityId::Faction(*f))).collect::<Vec<_>>(),
        "outcome": format!("{:?}", e.outcome),
        "consequences": e.consequences.iter().map(|c| format!("{c:?}")).collect::<Vec<_>>(),
        "caused_by": e.causes.iter().map(|c| ev(c.0)).collect::<Vec<_>>(),
        "triggered_by": e.triggered_by.map(|t| ev(t.0)),
        "led_to": ix.effects.get(&e.id.0).map(|v| v.iter().map(|x| ev(*x)).collect::<Vec<_>>()).unwrap_or_default(),
        "artifacts_made": e.artifacts_created.iter().filter_map(|x| h.artifacts.get(x).map(|a| a.name.clone())).collect::<Vec<_>>(),
        "monuments_made": e.monuments_created.iter().filter_map(|x| h.monuments.get(x).map(|m| m.name.clone())).collect::<Vec<_>>(),
        "part_of": crate::history::collections::context(h, e).iter().map(|(t, id)| json!({ "what": t, "event": id.map(|x| x.0) })).collect::<Vec<_>>(),
        "tale": tale,
    })))
}

// ---------------------------------------------------------------------------------------------
// history_why

fn why_schema() -> Value {
    schema(json!({ "world": world_prop(), "id": { "type": "integer" },
        "direction": { "type": "string", "enum": ["back", "forward", "both"] },
        "depth": { "type": "integer", "description": "Default 6" } }), &["id"])
}

fn history_why(s: &mut Session, a: &Value) -> ToolResult {
    let i = hist(s, a)?;
    let (h, ix) = parts(&s.worlds[i]);
    let id = get_u64(a, "id").ok_or("needs id")?;
    let root = event(h, ix, id).ok_or_else(|| format!("no event #{id}"))?;
    let depth = get_usize(a, "depth").unwrap_or(6);
    let dir = get_str(a, "direction").unwrap_or("back");
    let mut out = String::new();
    let mut stats = (0usize, 0usize); // (events shown, roots reached)
    for (label, back) in [("Because", true), ("It led to", false)] {
        if (back && dir == "forward") || (!back && dir == "back") { continue; }
        out.push_str(&format!("{label} (from {}):\n{}\n", if back { "effect back to its causes" } else { "cause on to its effects" }, line(root)));
        let mut seen: HashSet<u64> = HashSet::from([id]);
        let mut budget = 80usize;
        walk(h, ix, root, back, 1, depth, &mut seen, &mut budget, &mut out, &mut stats);
        if budget == 0 { out.push_str("  ... (80 events shown; ask a deeper link with its own id)\n"); }
        out.push('\n');
    }
    out.push_str(&format!("{} linked events shown; {} chains end at an event with no recorded cause.", stats.0, stats.1));
    Ok(Output::text(out))
}

#[allow(clippy::too_many_arguments)]
fn walk(h: &WorldHistory, ix: &HistIndex, e: &Event, back: bool, d: usize, max: usize, seen: &mut HashSet<u64>, budget: &mut usize, out: &mut String, stats: &mut (usize, usize)) {
    let next: Vec<u64> = if back {
        let mut v: Vec<u64> = e.causes.iter().map(|c| c.0).collect();
        if let Some(t) = e.triggered_by { if !v.contains(&t.0) { v.push(t.0); } }
        v
    } else { ix.effects.get(&e.id.0).cloned().unwrap_or_default() };
    if back && next.is_empty() { stats.1 += 1; }
    if d > max { if !next.is_empty() { out.push_str(&format!("{}...\n", "  ".repeat(d))); } return; }
    for n in next {
        if *budget == 0 { return; }
        let Some(c) = event(h, ix, n) else { continue };
        if !seen.insert(n) { out.push_str(&format!("{}(again) {}\n", "  ".repeat(d), line(c))); continue; }
        *budget -= 1;
        stats.0 += 1;
        out.push_str(&format!("{}{}\n", "  ".repeat(d), line(c)));
        walk(h, ix, c, back, d + 1, max, seen, budget, out, stats);
    }
}

// ---------------------------------------------------------------------------------------------
// history_entity

fn entity_schema() -> Value {
    schema(json!({ "world": world_prop(),
        "name": { "type": "string", "description": "Substring of the name (case-insensitive)" },
        "type": { "type": "string", "enum": ["people", "town", "figure", "beast", "artifact", "monument", "war", "faith", "god", "house"] },
        "id": { "type": "integer" },
        "limit": { "type": "integer", "description": "Timeline events to list (default 60)" } }), &[])
}

enum Found { E(EntityId), War(crate::history::WarId) }

fn history_entity(s: &mut Session, a: &Value) -> ToolResult {
    let i = hist(s, a)?;
    let (h, ix) = parts(&s.worlds[i]);
    use crate::history::*;
    let ty = get_str(a, "type");
    let want = |t: &str| ty.map_or(true, |x| x == t);
    let mut found: Vec<(Found, String)> = Vec::new();
    if let Some(id) = get_u64(a, "id") {
        let t = ty.ok_or("an id needs its type")?;
        let f = match t {
            "people" => Found::E(EntityId::Faction(FactionId(id))), "town" => Found::E(EntityId::Settlement(SettlementId(id))),
            "figure" => Found::E(EntityId::Figure(FigureId(id))), "beast" => Found::E(EntityId::LegendaryCreature(LegendaryCreatureId(id))),
            "artifact" => Found::E(EntityId::Artifact(ArtifactId(id))), "monument" => Found::E(EntityId::Monument(MonumentId(id))),
            "faith" => Found::E(EntityId::Religion(ReligionId(id))), "god" => Found::E(EntityId::Deity(DeityId(id))),
            "house" => Found::E(EntityId::Dynasty(DynastyId(id))), "war" => Found::War(WarId(id)),
            _ => return Err(format!("unknown type '{t}'")),
        };
        found.push((f, String::new()));
    } else {
        let n = get_str(a, "name").ok_or("give a name, or a type and an id")?.to_lowercase();
        let hit = |s: &str| s.to_lowercase().contains(&n);
        let mut push = |f: Found, name: String| found.push((f, name));
        if want("people") { for (k, v) in &h.factions { if hit(&v.name) { push(Found::E(EntityId::Faction(*k)), v.name.clone()); } } }
        if want("town") { for (k, v) in &h.settlements { if hit(&v.name) { push(Found::E(EntityId::Settlement(*k)), v.name.clone()); } } }
        if want("figure") { for (k, v) in &h.figures { if hit(&v.full_name()) { push(Found::E(EntityId::Figure(*k)), v.full_name()); } } }
        if want("beast") { for (k, v) in &h.legendary_creatures { if hit(&v.full_name()) { push(Found::E(EntityId::LegendaryCreature(*k)), v.full_name()); } } }
        if want("artifact") { for (k, v) in &h.artifacts { if hit(&v.name) { push(Found::E(EntityId::Artifact(*k)), v.name.clone()); } } }
        if want("monument") { for (k, v) in &h.monuments { if hit(&v.name) { push(Found::E(EntityId::Monument(*k)), v.name.clone()); } } }
        if want("faith") { for (k, v) in &h.religions { if hit(&v.name) { push(Found::E(EntityId::Religion(*k)), v.name.clone()); } } }
        if want("god") { for (k, v) in &h.deities { if hit(&v.name) { push(Found::E(EntityId::Deity(*k)), v.name.clone()); } } }
        if want("house") { for (k, v) in &h.dynasties { if hit(&v.name) { push(Found::E(EntityId::Dynasty(*k)), v.name.clone()); } } }
        if want("war") { for (k, v) in &h.wars { if hit(&v.name) { push(Found::War(*k), v.name.clone()); } } }
        // An exact name first, then shorter names (closer matches).
        found.sort_by_key(|(_, name)| (name.to_lowercase() != n, name.len(), name.clone()));
    }
    if found.is_empty() { return Err("nothing by that name".into()); }
    let exact = found.iter().filter(|(_, name)| name.to_lowercase() == get_str(a, "name").unwrap_or("").to_lowercase()).count();
    if found.len() > 1 && exact != 1 {
        let c: Vec<Value> = found.iter().take(40).map(|(f, name)| match f {
            Found::E(e) => { let (t, id) = entity_key(e); json!({ "type": t, "id": id, "name": name }) }
            Found::War(w) => json!({ "type": "war", "id": w.0, "name": name }),
        }).collect();
        return Ok(Output::json(json!({ "candidates": found.len(), "first": c, "hint": "call again with type and id" })));
    }
    let limit = get_usize(a, "limit").unwrap_or(60);
    let (record, timeline): (Value, Vec<&Event>) = match &found[0].0 {
        Found::War(w) => {
            let war = h.wars.get(w).ok_or("no such war")?;
            let mut evs: Vec<&Event> = war.declaration_event.iter().chain(war.battles.iter()).chain(war.sieges.iter()).filter_map(|x| event(h, ix, x.0)).collect();
            evs.sort_by_key(|e| (e.date, e.id.0));
            let names = |v: &Vec<FactionId>| v.iter().map(|f| entity_name(h, &EntityId::Faction(*f))).collect::<Vec<_>>();
            (json!({ "type": "war", "id": w.0, "name": war.name, "cause": format!("{:?}", war.cause), "aggressors": names(&war.aggressors), "defenders": names(&war.defenders),
                "started": war.started.year, "ended": war.ended.map(|d| d.year), "victor": war.victor.map(|f| entity_name(h, &EntityId::Faction(f))),
                "battles": war.battles.len(), "sieges": war.sieges.len(), "casualties": format!("{:?}", war.casualties) }), evs)
        }
        Found::E(id) => {
            let evs: Vec<&Event> = ix.by_entity.get(id).map(|v| v.iter().map(|&p| &h.chronicle.events[p]).collect()).unwrap_or_default();
            (entity_record(h, id), evs)
        }
    };
    Ok(Output::json(json!({
        "record": record, "events": timeline.len(),
        "timeline": timeline.iter().take(limit).map(|e| brief(e)).collect::<Vec<_>>(),
    })))
}

fn entity_record(h: &WorldHistory, id: &EntityId) -> Value {
    let (t, n) = entity_key(id);
    let mut r = json!({ "type": t, "id": n, "name": entity_name(h, id) });
    let fname = |f: crate::history::FactionId| entity_name(h, &EntityId::Faction(f));
    match id {
        EntityId::Faction(f) => if let Some(x) = h.factions.get(f) {
            let towns: Vec<String> = h.settlements.values().filter(|s| s.faction == *f && !s.is_destroyed()).map(|s| s.name.clone()).collect();
            r["founded"] = json!(x.founded.year); r["dissolved"] = json!(x.dissolved.map(|d| d.year));
            r["government"] = json!(format!("{:?}", x.government));
            r["capital"] = json!(x.capital.and_then(|c| h.settlements.get(&c)).map(|s| s.name.clone()));
            r["living_towns"] = json!(towns.len()); r["towns"] = json!(towns.into_iter().take(20).collect::<Vec<_>>());
        },
        EntityId::Settlement(s) => if let Some(x) = h.settlements.get(s) {
            r["people"] = json!(fname(x.faction)); r["at"] = json!([x.location.0, x.location.1]); r["population"] = json!(x.population);
            r["founded"] = json!(x.founded.year); r["destroyed"] = json!(x.destroyed.map(|d| d.year));
        },
        EntityId::Figure(f) => if let Some(x) = h.figures.get(f) {
            r["people"] = json!(x.faction.map(fname)); r["born"] = json!(x.birth_date.year); r["died"] = json!(x.death_date.map(|d| d.year));
            r["titles"] = json!(x.titles); r["epithet"] = json!(x.epithet);
        },
        EntityId::LegendaryCreature(c) => if let Some(x) = h.legendary_creatures.get(c) {
            r["died"] = json!(x.death_date.map(|d| d.year)); r["lair"] = json!(x.lair_location.map(|(x, y)| [x, y])); r["kills"] = json!(x.kills.len());
        },
        _ => {}
    }
    r
}

// ---------------------------------------------------------------------------------------------
// history_stats

fn history_stats(s: &mut Session, a: &Value) -> ToolResult {
    let i = hist(s, a)?;
    let entry = &s.worlds[i];
    let (h, _) = parts(entry);
    use crate::history::simulation::invariants as inv;
    let sm = h.summary();
    let mut out = format!("{sm}\n");
    let mut kinds: HashMap<String, usize> = HashMap::new();
    let mut centuries: std::collections::BTreeMap<u32, usize> = Default::default();
    for e in &h.chronicle.events { *kinds.entry(format!("{:?}", e.event_type)).or_default() += 1; *centuries.entry(e.date.year / 100 * 100).or_default() += 1; }
    let mut kinds: Vec<(String, usize)> = kinds.into_iter().collect();
    kinds.sort_by(|p, q| q.1.cmp(&p.1).then(p.0.cmp(&q.0)));
    out.push_str("\nEvents by kind: ");
    out.push_str(&kinds.iter().take(30).map(|(k, n)| format!("{k} {n}")).collect::<Vec<_>>().join(", "));
    out.push_str("\nEvents by century: ");
    out.push_str(&centuries.iter().map(|(c, n)| format!("{c}s {n}")).collect::<Vec<_>>().join(", "));
    let rep = inv::cause_report(h);
    let (c, t) = rep.iter().fold((0, 0), |a, r| (a.0 + r.1, a.1 + r.2));
    let (median, mean) = inv::chain_depth(h);
    out.push_str(&format!("\n\nCauses: {c} of {t} events ({:.0}%) record a cause; chains run {median} links back (median; mean {mean:.1}). Uncaused, most first: ", 100.0 * c as f64 / t.max(1) as f64));
    out.push_str(&rep.iter().take(12).map(|(k, c, t)| format!("{k} {}/{t}", t - c)).collect::<Vec<_>>().join(", "));
    let v = inv::violations(h);
    out.push_str(&format!("\n\nContradictions: {}", if v.is_empty() { "none (consistent)".to_string() } else { format!("{}:\n  {}", v.len(), v.iter().take(20).cloned().collect::<Vec<_>>().join("\n  ")) }));
    out.push_str(&format!("\n\n{}\n{}\n{}", inv::war_report(h), inv::objects_report(h), crate::history::people::report(h)));
    out.push_str(&format!("\n\nPresent day: {}", h.present().counts()));
    out.push_str(&format!("\nRecords: {}", crate::lore::claims::sentence("this world", &crate::lore::claims::claims(h))));
    out.push_str(&format!("\n{}", crate::lore::rare::report(&crate::lore::rare::find(&entry.world, h))));
    out.push_str(&format!("\n{}", crate::lore::sifting::report(&crate::lore::sifting::sift(h))));
    Ok(Output::text(out))
}

// ---------------------------------------------------------------------------------------------
// history_export

fn export_schema() -> Value {
    schema(json!({ "world": world_prop(), "dir": { "type": "string", "description": "Directory (default out/lab/<world>_history)" } }), &[])
}

fn history_export(s: &mut Session, a: &Value) -> ToolResult {
    use std::io::Write as _;
    let i = hist(s, a)?;
    let dir = match get_str(a, "dir") { Some(d) => PathBuf::from(d), None => s.out_dir.join(format!("{}_history", s.worlds[i].id)) };
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let (h, _) = parts(&s.worlds[i]);
    let io = |e: std::io::Error| e.to_string();
    let ent = |p: &EntityId| { let (t, id) = entity_key(p); format!("{t}:{id}") };
    let mut ev = std::io::BufWriter::new(std::fs::File::create(dir.join("events.jsonl")).map_err(io)?);
    let mut edges = std::io::BufWriter::new(std::fs::File::create(dir.join("edges.csv")).map_err(io)?);
    writeln!(edges, "cause,effect,link").map_err(io)?;
    for e in &h.chronicle.events {
        let row = json!({ "id": e.id.0, "year": e.date.year, "season": format!("{:?}", e.date.season), "kind": format!("{:?}", e.event_type),
            "major": e.is_major, "title": e.title, "description": e.description, "x": e.location.map(|l| l.0), "y": e.location.map(|l| l.1),
            "participants": e.primary_participants.iter().map(ent).collect::<Vec<_>>(),
            "peoples": e.factions_involved.iter().map(|f| f.0).collect::<Vec<_>>(),
            "causes": e.causes.iter().map(|c| c.0).collect::<Vec<_>>(), "triggered_by": e.triggered_by.map(|t| t.0) });
        writeln!(ev, "{row}").map_err(io)?;
        for c in &e.causes { writeln!(edges, "{},{},cause", c.0, e.id.0).map_err(io)?; }
        if let Some(t) = e.triggered_by { writeln!(edges, "{},{},trigger", t.0, e.id.0).map_err(io)?; }
    }
    let mut en = std::io::BufWriter::new(std::fs::File::create(dir.join("entities.jsonl")).map_err(io)?);
    let mut n = 0usize;
    let mut put = |v: Value| -> Result<(), String> { n += 1; writeln!(en, "{v}").map_err(io) };
    for (k, v) in &h.factions { put(json!({ "type": "people", "id": k.0, "name": v.name, "founded": v.founded.year, "dissolved": v.dissolved.map(|d| d.year) }))?; }
    for (k, v) in &h.settlements { put(json!({ "type": "town", "id": k.0, "name": v.name, "people": v.faction.0, "x": v.location.0, "y": v.location.1, "founded": v.founded.year, "destroyed": v.destroyed.map(|d| d.year), "population": v.population }))?; }
    for (k, v) in &h.figures { put(json!({ "type": "figure", "id": k.0, "name": v.full_name(), "people": v.faction.map(|f| f.0), "born": v.birth_date.year, "died": v.death_date.map(|d| d.year) }))?; }
    for (k, v) in &h.legendary_creatures { put(json!({ "type": "beast", "id": k.0, "name": v.full_name(), "died": v.death_date.map(|d| d.year), "kills": v.kills.len() }))?; }
    for (k, v) in &h.wars { put(json!({ "type": "war", "id": k.0, "name": v.name, "started": v.started.year, "ended": v.ended.map(|d| d.year), "aggressors": v.aggressors.iter().map(|f| f.0).collect::<Vec<_>>(), "defenders": v.defenders.iter().map(|f| f.0).collect::<Vec<_>>(), "victor": v.victor.map(|f| f.0) }))?; }
    for (k, v) in &h.artifacts { put(json!({ "type": "artifact", "id": k.0, "name": v.name }))?; }
    for (k, v) in &h.monuments { put(json!({ "type": "monument", "id": k.0, "name": v.name, "x": v.location.0, "y": v.location.1 }))?; }
    drop(put);
    ev.flush().map_err(io)?; edges.flush().map_err(io)?; en.flush().map_err(io)?;
    Ok(Output::text(format!("{} events, {} entities written.", h.chronicle.events.len(), n))
        .with_files(vec![dir.join("events.jsonl"), dir.join("entities.jsonl"), dir.join("edges.csv")]))
}

// ---------------------------------------------------------------------------------------------
// history_explorer

fn explorer_schema() -> Value {
    schema(json!({ "world": world_prop(),
        "dir": { "type": "string", "description": "Directory (default out/lab/<world>_explorer)" },
        "legends": { "type": "boolean", "description": "Also write the full legends pages into legends/ (default true)" },
        "open": { "type": "boolean", "description": "Open it in the default browser" },
        "compare": { "type": "string", "description": "Another open world of the same land (e.g. a history_branch) for the Compare tab" } }), &[])
}

fn history_explorer(s: &mut Session, a: &Value) -> ToolResult {
    let i = hist(s, a)?;
    let dir = match get_str(a, "dir") { Some(d) => PathBuf::from(d), None => s.out_dir.join(format!("{}_explorer", s.worlds[i].id.replace(['~', '/'], "_"))) };
    let legends = a.get("legends").and_then(Value::as_bool).unwrap_or(true);
    s.atlas();
    let t0 = std::time::Instant::now();
    let e = &s.worlds[i];
    let other = match get_str(a, "compare") {
        Some(c) => {
            let o = s.worlds.iter().find(|w| w.id == c).ok_or_else(|| format!("no open world '{c}' to compare with"))?;
            Some((&o.world, o.history.as_ref().ok_or_else(|| format!("world '{c}' has no history"))?, o.id.as_str()))
        }
        None => None,
    };
    let w = super::explorer::write_compare(&e.world, e.history.as_ref().unwrap(), &e.id, s.atlas.as_ref().unwrap(), &dir, legends, other).map_err(|e| e.to_string())?;
    let index = std::fs::canonicalize(w.dir.join("index.html")).unwrap_or_else(|_| w.dir.join("index.html"));
    if get_bool(a, "open") { let _ = std::process::Command::new("open").arg(&index).status(); }
    Ok(Output::text(format!("History explorer for '{}' written in {:.1}s: {} events, {} named things, data {:.1} MB{}.\nOpen file://{}",
        e.id, t0.elapsed().as_secs_f32(), w.events, w.entities, w.data_bytes as f64 / 1e6,
        w.legends.map_or(String::new(), |l| format!("; legends: {l}")), index.display())).with_files(vec![index]))
}

// ---------------------------------------------------------------------------------------------
// history_branch

fn branch_schema() -> Value {
    schema(json!({ "world": world_prop(),
        "from_year": { "type": "integer", "description": "The fork: the history runs as it was until the first season of this year" },
        "seed": { "type": "integer", "description": "Seed for the random stream after the fork (default: the history seed + from_year)" },
        "history_seed": { "type": "integer", "description": "The history's own seed (default world seed + 1000, as the game uses unless --history-seed was given)" },
        "base_world": { "type": "string", "description": "World id or .world file holding the land before the history (default: the one open_world made beside it; else the saved world with its scars undone, which may not replay exactly)" },
        "id": { "type": "string", "description": "Name for the branched world (default <world>~y<year>s<seed>)" } }), &["from_year"])
}

fn history_branch(s: &mut Session, a: &Value) -> ToolResult {
    let i = hist(s, a)?;
    let from_year = get_usize(a, "from_year").ok_or("needs from_year")? as u32;
    let (world_id, path, saved_events, saved_last) = {
        let e = &s.worlds[i];
        let h = e.history.as_ref().unwrap();
        (e.id.clone(), e.path.clone(), h.chronicle.events.len(), h.chronicle.events.last().map(|e| e.title.clone()))
    };
    let base_seed = get_u64(a, "history_seed").unwrap_or(s.worlds[i].world.seed().wrapping_add(1000));
    let fork_seed = get_u64(a, "seed").unwrap_or(base_seed.wrapping_add(from_year as u64));
    let config = s.worlds[i].history.as_ref().unwrap().config.clone();
    let last_year = config.prehistory_depth + config.simulation_years;
    if from_year > last_year { return Err(format!("the history ends in year {last_year}")); }
    // The world as it was before the history: reloaded (WorldData is not Clone) and its
    // scars and anomalies undone, as `main` does before simulating.
    let t0 = std::time::Instant::now();
    let land = match get_str(a, "base_world") {
        Some(b) => s.worlds.iter().find(|w| w.id == b).map(|w| w.path.clone()).unwrap_or_else(|| PathBuf::from(b)),
        None => s.worlds[i].prehistory_path.clone().unwrap_or_else(|| path.clone()),
    };
    let exact_land = land != path;
    let (mut world, _) = crate::world::load_world(&land).map_err(|e| format!("{}: {e}", land.display()))?;
    crate::history::ecology::Ecology::naturalize(&mut world);
    let game_data = crate::history::data::GameData::load_from(std::path::Path::new("data"));
    let mut engine = crate::history::simulation::HistoryEngine::new(base_seed);
    engine.shadow = crate::myth::on();
    let total = config.total_steps();
    let mut base = engine.begin(&world, config, &game_data);
    let mut step = 0u32;
    while step < total && base.current_date.year < from_year { engine.step(&mut base, &world, &game_data); step += 1; }
    let fork_events = base.chronicle.events.len();
    let mut branch = base.clone();
    let mut fork_engine = crate::history::simulation::HistoryEngine::new(fork_seed);
    fork_engine.shadow = engine.shadow;
    for _ in step..total { engine.step(&mut base, &world, &game_data); }
    for _ in step..total { fork_engine.step(&mut branch, &world, &game_data); }
    engine.finish(&mut base);
    fork_engine.finish(&mut branch);
    let secs = t0.elapsed().as_secs_f32();
    let replay_matches = base.chronicle.events.len() == saved_events && base.chronicle.events.last().map(|e| e.title.clone()) == saved_last;

    let side = |h: &WorldHistory| {
        let sm = h.summary();
        let living = h.settlements.values().filter(|s| !s.is_destroyed()).count();
        json!({ "events": sm.total_events, "events_after_fork": sm.total_events.saturating_sub(fork_events), "active_peoples": sm.active_factions,
            "living_towns": living, "ruins": sm.total_settlements - living, "wars": h.wars.len(), "living_figures": sm.living_figures,
            "present": h.present().counts(), "records": crate::lore::claims::sentence("this world", &crate::lore::claims::claims(h)),
            "first_major_after_fork": h.chronicle.events.iter().skip(fork_events).filter(|e| e.is_major).take(12)
                .map(|e| format!("y{} {}", e.date.year, e.title)).collect::<Vec<_>>() })
    };
    let alive_peoples = |h: &WorldHistory| -> HashSet<String> { h.factions.values().filter(|f| f.is_active()).map(|f| f.name.clone()).collect() };
    let standing = |h: &WorldHistory| -> HashSet<String> { h.settlements.values().filter(|s| !s.is_destroyed()).map(|s| s.name.clone()).collect() };
    let diff = |a: &HashSet<String>, b: &HashSet<String>| { let mut v: Vec<String> = a.difference(b).cloned().collect(); v.sort(); v };
    let (pa, pb, ta, tb) = (alive_peoples(&base), alive_peoples(&branch), standing(&base), standing(&branch));
    let first_diff = base.chronicle.events.iter().zip(branch.chronicle.events.iter()).position(|(x, y)| x.title != y.title);
    let report = json!({
        "fork": { "year": from_year, "step": step, "events_before_fork": fork_events, "history_seed": base_seed, "fork_seed": fork_seed, "seconds": (secs * 10.0).round() / 10.0 },
        "replay_matches_saved": replay_matches,
        "first_difference_at_event": first_diff.map(|p| json!({ "index": p, "baseline": line(&base.chronicle.events[p]), "branch": line(&branch.chronicle.events[p]) })),
        "baseline": side(&base), "branch": side(&branch),
        "peoples_alive_only_in_baseline": diff(&pa, &pb), "peoples_alive_only_in_branch": diff(&pb, &pa),
        "towns_standing_only_in_baseline": diff(&ta, &tb).into_iter().take(40).collect::<Vec<_>>(),
        "towns_standing_only_in_branch": diff(&tb, &ta).into_iter().take(40).collect::<Vec<_>>(),
    });
    // Keep the branch as a world of its own (its scars on its own copy of the land).
    if let Some(eco) = branch.ecology.as_mut() { eco.apply_scars(&mut world); }
    let id = get_str(a, "id").map(str::to_string).unwrap_or_else(|| format!("{world_id}~y{from_year}s{fork_seed}"));
    let current = s.current.clone();
    s.insert(WorldEntry { id: id.clone(), path, prehistory_path: Some(land.clone()), world, history: Some(branch), regions: Vec::new(), hist_index: None });
    s.current = current;
    let mut r = report;
    r["branch_world"] = json!(id);
    r["land"] = json!(if exact_land { format!("the world before its history ({})", land.display()) } else { "the saved world with its scars undone (approximate)".to_string() });
    if !replay_matches { r["note"] = json!("The baseline replay differs from the saved history (the code changed since the world file was made, or the history used --history-seed): compare branch with baseline, not with the saved world."); }
    Ok(Output::json(r))
}
