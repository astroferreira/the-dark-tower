//! The history explorer: a self-contained HTML app over one world's history, written by the
//! lab's `history_explorer` tool (or `planet_lab call ... history_explorer`). Views: a reader in
//! the manner of Dwarf Fortress's legends (every person, people, town, war, house, beast, thing,
//! year and age as a linked page), a timeline of the peoples with their wars and events, the
//! causal graph round any event, family trees, the web of peoples (wars, treaties, trade,
//! marriages over a span of years) and of a person, the map with territories and towns at any
//! year, and the checks (contradictions, uncaused kinds, events without a place, causal hubs,
//! chain depths, odd lives). The app is `explorer/` (HTML, CSS, JS, no outside libraries);
//! the data is `data.js` beside it; the full legends pages go to `legends/` and are linked.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::history::events::Event;
use crate::history::world_state::WorldHistory;
use crate::history::EntityId;
use crate::world::WorldData;

const INDEX: &str = include_str!("explorer/index.html");
const CSS: &str = include_str!("explorer/explorer.css");
const JS: &str = include_str!("explorer/explorer.js");

pub struct Written {
    pub dir: PathBuf,
    pub events: usize,
    pub entities: usize,
    pub data_bytes: usize,
    pub legends: Option<String>,
}

/// The entity's code in the app: a letter for its type and its id ("P12" a person).
pub fn code(id: &EntityId) -> String {
    match id {
        EntityId::Faction(x) => format!("F{}", x.0), EntityId::Settlement(x) => format!("S{}", x.0), EntityId::Figure(x) => format!("P{}", x.0),
        EntityId::Dynasty(x) => format!("H{}", x.0), EntityId::LegendaryCreature(x) => format!("B{}", x.0), EntityId::CreaturePopulation(x) => format!("C{}", x.0),
        EntityId::Deity(x) => format!("G{}", x.0), EntityId::Religion(x) => format!("R{}", x.0), EntityId::Artifact(x) => format!("A{}", x.0),
        EntityId::Monument(x) => format!("M{}", x.0),
    }
}

fn hex(c: u32) -> String { format!("#{:06x}", c & 0xFFFFFF) }

fn sorted<'a, K: Ord + Copy + 'a, V: 'a>(m: impl IntoIterator<Item = (&'a K, &'a V)>) -> Vec<(K, &'a V)> {
    let mut v: Vec<(K, &V)> = m.into_iter().map(|(k, v)| (*k, v)).collect();
    v.sort_by_key(|p| p.0);
    v
}

/// The history as the app's data (`window.HISTORY`).
pub fn data(world: &WorldData, h: &WorldHistory, id: &str, legends: bool) -> Value {
    let mut kinds: Vec<String> = Vec::new();
    let mut kind_ix: HashMap<String, usize> = HashMap::new();
    let mut kind = |e: &Event| -> usize {
        let k = format!("{:?}", e.event_type);
        if let Some(&i) = kind_ix.get(&k) { return i; }
        kinds.push(k.clone());
        kind_ix.insert(k, kinds.len() - 1);
        kinds.len() - 1
    };
    let season = |s: &crate::seasons::Season| match s { crate::seasons::Season::Spring => 0, crate::seasons::Season::Summer => 1, crate::seasons::Season::Autumn => 2, crate::seasons::Season::Winter => 3 };
    let events: Vec<Value> = h.chronicle.events.iter().map(|e| {
        let (x, y) = e.location.map(|(x, y)| (x as i64, y as i64)).unwrap_or((-1, -1));
        json!([e.id.0, e.date.year, season(&e.date.season), kind(e), e.title, e.description, x, y, e.is_major as u8,
            e.primary_participants.iter().map(code).collect::<Vec<_>>(),
            e.factions_involved.iter().map(|f| f.0).collect::<Vec<_>>(),
            e.causes.iter().map(|c| c.0).collect::<Vec<_>>(),
            e.triggered_by.map(|t| t.0 as i64).unwrap_or(-1)])
    }).collect();

    let peoples: Vec<Value> = sorted(&h.factions).into_iter().map(|(k, f)| json!({
        "i": k.0, "n": f.name, "f": f.founded.year, "d": f.dissolved.map(|d| d.year), "c": hex(crate::tiles::classify::faction_color(k.0)),
        "cap": f.capital.map(|c| c.0), "gov": format!("{:?}", f.government), "race": h.races.get(&f.race_id).map(|r| r.name.clone()),
        "lead": f.current_leader.map(|l| l.0), "house": f.ruling_dynasty.map(|d| d.0), "faith": f.state_religion.map(|r| r.0),
        "pop": f.total_population, "wealth": f.wealth,
        "rel": sorted(&f.relations).into_iter().map(|(o, r)| json!([o.0, r.opinion, format!("{:?}", r.stance)])).collect::<Vec<_>>(),
    })).collect();

    let towns: Vec<Value> = sorted(&h.settlements).into_iter().map(|(k, s)| {
        let own: Vec<Value> = h.tile_history.get(s.location.0, s.location.1).ownership.iter()
            .map(|o| json!([o.faction.0, o.gained.year, o.lost.map(|l| l.year as i64).unwrap_or(-1)])).collect();
        json!({ "i": k.0, "n": s.name, "x": s.location.0, "y": s.location.1, "p": s.faction.0, "f": s.founded.year, "d": s.destroyed.map(|d| d.year),
            "pop": s.population, "t": format!("{:?}", s.settlement_type), "walls": format!("{:?}", s.walls), "own": own })
    }).collect();

    let figures: Vec<Value> = sorted(&h.figures).into_iter().map(|(k, f)| json!({
        "i": k.0, "n": f.full_name(), "p": f.faction.map(|x| x.0), "b": f.birth_date.year, "d": f.death_date.map(|d| d.year),
        "dc": f.cause_of_death.as_ref().map(|c| format!("{c:?}")), "pa": [f.parents.0.map(|x| x.0), f.parents.1.map(|x| x.0)],
        "sp": f.spouse.map(|x| x.0), "ch": f.children.iter().map(|c| c.0).collect::<Vec<_>>(), "h": f.dynasty.map(|d| d.0),
        "t": f.titles, "en": f.enemies.iter().map(|x| x.0).collect::<Vec<_>>(), "me": f.mentors.iter().map(|x| x.0).collect::<Vec<_>>(),
        "race": h.races.get(&f.race_id).map(|r| r.name.clone()),
    })).collect();

    let houses: Vec<Value> = sorted(&h.dynasties).into_iter().map(|(k, d)| json!({
        "i": k.0, "n": d.name, "f": d.founded.year, "fo": d.founder.0, "head": d.current_head.map(|x| x.0),
        "m": d.members.iter().map(|x| x.0).collect::<Vec<_>>(), "ruled": d.factions_ruled.iter().map(|x| x.0).collect::<Vec<_>>(),
    })).collect();

    let wars: Vec<Value> = sorted(&h.wars).into_iter().map(|(k, w)| json!({
        "i": k.0, "n": w.name, "a": w.aggressors.iter().map(|x| x.0).collect::<Vec<_>>(), "df": w.defenders.iter().map(|x| x.0).collect::<Vec<_>>(),
        "s": w.started.year, "e": w.ended.map(|d| d.year), "v": w.victor.map(|x| x.0), "cause": format!("{:?}", w.cause),
        "decl": w.declaration_event.map(|x| x.0), "b": w.battles.iter().map(|x| x.0).collect::<Vec<_>>(), "g": w.sieges.iter().map(|x| x.0).collect::<Vec<_>>(),
    })).collect();

    let simple = |n: &str, extra: Value| { let mut v = json!({ "n": n }); if let (Some(o), Some(e)) = (v.as_object_mut(), extra.as_object()) { for (k, x) in e { o.insert(k.clone(), x.clone()); } } v };
    let beasts: Vec<Value> = sorted(&h.legendary_creatures).into_iter().map(|(k, c)| { let mut v = simple(&c.full_name(), json!({ "d": c.death_date.map(|d| d.year), "lair": c.lair_location.map(|(x, y)| [x, y]), "kills": c.kills.len() })); v["i"] = json!(k.0); v }).collect();
    let artifacts: Vec<Value> = sorted(&h.artifacts).into_iter().map(|(k, a)| { let mut v = simple(&a.name, json!({})); v["i"] = json!(k.0); v }).collect();
    let monuments: Vec<Value> = sorted(&h.monuments).into_iter().map(|(k, m)| { let mut v = simple(&m.name, json!({ "x": m.location.0, "y": m.location.1, "f": m.built_date.year })); v["i"] = json!(k.0); v }).collect();
    let faiths: Vec<Value> = sorted(&h.religions).into_iter().map(|(k, r)| { let mut v = simple(&r.name, json!({})); v["i"] = json!(k.0); v }).collect();
    let gods: Vec<Value> = sorted(&h.deities).into_iter().map(|(k, d)| { let mut v = simple(&d.name, json!({})); v["i"] = json!(k.0); v }).collect();
    let pops: Vec<Value> = sorted(&h.populations).into_iter().map(|(k, p)| {
        let n = h.creature_species.get(&p.species_id).map(|s| format!("a population of {}", s.name)).unwrap_or_else(|| "a population".into());
        let mut v = simple(&n, json!({})); v["i"] = json!(k.0); v
    }).collect();

    // Who held each tile, and from when: territories at any year.
    let mut territory: Vec<i64> = Vec::new();
    for y in 0..world.height { for x in 0..world.width {
        for o in &h.tile_history.get(x, y).ownership {
            territory.extend([(y * world.width + x) as i64, o.faction.0 as i64, o.gained.year as i64, o.lost.map(|l| l.year as i64).unwrap_or(-1)]);
        }
    } }

    let ages: Vec<Value> = h.timeline.eras.iter().map(|a| json!([a.name, a.start.year, a.end.map(|d| d.year)])).collect();
    let tales: Vec<Value> = crate::lore::sifting::sift(h).iter().take(60).map(|t| json!({
        "k": format!("{:?}", t.kind), "t": t.title, "x": t.text, "e": t.events.iter().map(|e| e.0).collect::<Vec<_>>(), "y": t.year,
    })).collect();
    let first = h.chronicle.events.first().map(|e| e.date.year).unwrap_or(0);
    let gaz = crate::lore::build_gazetteer(world, Some(h), world.seed());
    // The named lands (the gazetteer): each with its kind, and which tiles each layer gives it.
    let lands: Vec<Value> = gaz.features.iter().map(|f| json!({
        "i": f.id, "n": f.name, "k": format!("{:?}", f.kind), "a": [f.anchor.0, f.anchor.1], "size": f.size,
        "hm": if f.height_m > 0.0 { Some(f.height_m.round()) } else { None }, "by": f.named_by.map(|x| x.0),
    })).collect();
    let layer = |m: &crate::tilemap::Tilemap<u32>| -> Vec<i64> { (0..world.width * world.height).map(|i| { let v = *m.get(i % world.width, i / world.width); if v == crate::lore::gazetteer::NONE { -1 } else { v as i64 } }).collect() };
    let land_layers = json!({ "water": layer(&gaz.water), "relief": layer(&gaz.relief), "region": layer(&gaz.region), "landmass": layer(&gaz.landmass) });
    let land = gaz.features.iter().filter(|f| matches!(f.kind, crate::lore::FeatureKind::Continent)).max_by_key(|f| f.size).map(|f| f.name.clone()).unwrap_or_else(|| "the world".into());

    json!({
        "meta": { "id": id, "seed": world.seed(), "w": world.width, "h": world.height, "first": first, "last": h.current_date.year,
            "land": land, "sentence": crate::lore::claims::sentence(&land, &crate::lore::claims::claims(h)),
            "present": h.present().counts(), "legends": legends, "made": chrono::Local::now().format("%Y-%m-%d %H:%M").to_string() },
        "kinds": kinds, "ages": ages, "events": events,
        "peoples": peoples, "towns": towns, "figures": figures, "houses": houses, "wars": wars,
        "beasts": beasts, "artifacts": artifacts, "monuments": monuments, "faiths": faiths, "gods": gods, "pops": pops,
        "territory": territory, "tales": tales, "lands": lands, "land_layers": land_layers,
        "checks": { "violations": crate::history::simulation::invariants::violations(h) },
    })
}

/// Write the explorer for a world's history into `dir`: `index.html` (open it in a browser),
/// `data.js`, `map.png` (the land without the history, under the territories) and, with
/// `legends`, the full legends pages in `legends/`.
pub fn write(world: &WorldData, h: &WorldHistory, id: &str, atlas: &crate::tiles::Atlas, dir: &Path, legends: bool) -> std::io::Result<Written> {
    write_compare(world, h, id, atlas, dir, legends, None)
}

/// As `write`, with a second history of the same land (a branch, or the same seed after a
/// change) for the Compare tab (`data2.js`).
pub fn write_compare(world: &WorldData, h: &WorldHistory, id: &str, atlas: &crate::tiles::Atlas, dir: &Path, legends: bool,
    other: Option<(&WorldData, &WorldHistory, &str)>) -> std::io::Result<Written> {
    std::fs::create_dir_all(dir)?;
    let io = |e: String| std::io::Error::new(std::io::ErrorKind::Other, e);
    let width = (world.width * 16).clamp(1024, 2048);
    let land = crate::tiles::viewer::map_image(world, None, atlas, width);
    land.save(dir.join("map.png")).map_err(|e| io(e.to_string()))?;
    let legends_line = if legends {
        let gaz = crate::lore::build_gazetteer(world, Some(h), world.seed());
        let map = crate::tiles::viewer::map_image(world, Some(h), atlas, 1024);
        let camps = crate::colony::legend::load(world.seed());
        Some(crate::lore::legends::write_legends(world, h, &gaz, &camps, Some(&map), &dir.join("legends"))?.line())
    } else { None };
    let mut d = data(world, h, id, legends);
    d["meta"]["map"] = json!({ "file": "map.png", "w": land.width(), "h": land.height() });
    let text = format!("window.HISTORY = {};\n", serde_json::to_string(&d).map_err(|e| io(e.to_string()))?);
    std::fs::write(dir.join("data.js"), &text)?;
    let other_text = match other {
        Some((w2, h2, id2)) => format!("window.HISTORY_B = {};\n", serde_json::to_string(&data(w2, h2, id2, false)).map_err(|e| io(e.to_string()))?),
        None => "window.HISTORY_B = null;\n".to_string(),
    };
    std::fs::write(dir.join("data2.js"), other_text)?;
    std::fs::write(dir.join("explorer.css"), CSS)?;
    std::fs::write(dir.join("explorer.js"), JS)?;
    std::fs::write(dir.join("index.html"), INDEX.replace("{{TITLE}}", &format!("The history of {}", d["meta"]["land"].as_str().unwrap_or("the world"))))?;
    let entities = ["peoples", "towns", "figures", "houses", "wars", "beasts", "artifacts", "monuments", "faiths", "gods"].iter().map(|k| d[*k].as_array().map_or(0, |a| a.len())).sum();
    Ok(Written { dir: dir.to_path_buf(), events: h.chronicle.events.len(), entities, data_bytes: text.len(), legends: legends_line })
}
