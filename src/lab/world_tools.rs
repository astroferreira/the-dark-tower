//! The lab's tools for the land: worlds, world tiles, regions, places, embarks, renders.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

use super::*;
use crate::region::plan::Place;

pub fn tools() -> Vec<Tool> {
    vec![
        Tool { name: "open_world", schema: open_world_schema, run: open_world,
            description: "Open a world and make it current. Either `path` (a .world file saved with --save-world), or a spec: `dev: true` (the 96x48 dev world, seed 10, with 250 years of history, ~2 s), or `seed` with optional width/height (default 512x256), star (default \"sol\"), style, history_years (default 0: no history; 250 at 512x256 takes minutes), civilizations, extra_args. A spec is made by the game's own binary (planet_generator --save-world) into worlds/lab/ and reused while that file is newer than the binary (`fresh: true` remakes it). Returns the world's id and summary." },
        Tool { name: "list_worlds", schema: || schema(json!({}), &[]), run: list_worlds,
            description: "The worlds open in this session (id, size, seed, history, regions kept) and the world files cached in worlds/lab/." },
        Tool { name: "world_summary", schema: || schema(json!({ "world": world_prop() }), &[]), run: world_summary,
            description: "A world at a glance: size and scale, star system, land share, the gazetteer's named features by kind (the largest of each with their anchor tiles), the landmarks (the world's extremes) and, with a history, its counts." },
        Tool { name: "tile", schema: tile_schema, run: tile,
            description: "Everything known about one world tile: height, climate, biome, plate and stress, rock hardness, water body, flow, soil, deposits, springs and aquifers, lava, the gazetteer features it belongs to, landmarks on it and, with a history, its owner, settlement, ruins and the events recorded there." },
        Tool { name: "find_sites", schema: || schema(json!({ "world": world_prop() }), &[]), run: find_sites,
            description: "Scan the world for the gallery's standard generation sites (young volcano, desert interior, big river floodplain and delta, hard and soft coast, bog, Mediterranean hillside, limestone plateau, glaciated valley, rift lake), as --survey-find." },
        Tool { name: "region_survey", schema: || schema(region_props(json!({})), &["x", "y"]), run: region_survey,
            description: "Measure the region round a world tile (as --region-survey): landform and rock mix, drainage density, Strahler orders and bifurcation ratio, Hack's exponent, hypsometric integral, slope percentiles, geomorphons, every place with its cause, caves, timings, check violations and agreement with a neighbouring window. Writes a four-panel PNG (height, landforms, slope, places)." },
        Tool { name: "places", schema: places_schema, run: places,
            description: "The places of the region round a world tile (natural wonders and the ages before: region::sites) as JSON: kind, name, epithet, cause, description, contents, absolute region cell, footprint, floor height, cave, rank, planning tile. Filter with `kind` (substring of the kind's word, e.g. \"barrow\", \"spring\") or `query` (substring of name, cause or description). Also lists the caves (name, kind, nodes, passages)." },
        Tool { name: "region_cell", schema: region_cell_schema, run: region_cell,
            description: "One region cell (~600 m at 512 wide) and why it is what it is: elevation, landform class, surface rock, river width, lake depth, drainage, temperature, moisture, its world tile's fields, and the nearest places with distance and cause. Give the cell as absolute region cells `ax`,`ay` (the frame places use), or as `cx`,`cy` within the region's window." },
        Tool { name: "views_agree", schema: || schema(region_props(json!({})), &["x", "y"]), run: views_agree,
            description: "Whether every place the region lists is shown in the embark made at its cell (ROADMAP Stage 1's one-truth test, as --views-agree): counts and the places not shown." },
        Tool { name: "place_census", schema: census_schema, run: place_census,
            description: "Sample N regions over the world (default 24; 0 = every window) and count the places by kind and age, windows with none, cause coverage, median spacing per kind and check violations (as --place-census). Slow: about a second per window." },
        Tool { name: "render_world", schema: render_world_schema, run: render_world,
            description: "Render the world map in ink round a tile (as --tiles-snapshot): returns the PNG paths (open them with your image reader, or pass inline_images). Optional season (spring/summer/autumn/winter) and overlay (height, temperature, moisture, drainage, plates, stress, biomes)." },
        Tool { name: "render_embark", schema: render_embark_schema, run: render_embark,
            description: "Render the region in ink and the embark round a tile (as --local-snapshot): the region at 4 px with its key, the embark's surface, levels and a section. `place` embarks at the region's nearest place whose kind or name holds the text (or a landform class: \"dune\", \"scree\"). Returns PNG paths and the embark's report (places carved, site, relief)." },
    ]
}

// ---------------------------------------------------------------------------------------------
// Worlds

fn open_world_schema() -> Value {
    schema(json!({
        "path": { "type": "string", "description": "A .world file to load" },
        "dev": { "type": "boolean", "description": "The dev world (96x48, seed 10, 250 years of history)" },
        "seed": { "type": "integer" },
        "width": { "type": "integer", "description": "Default 512" },
        "height": { "type": "integer", "description": "Default 256" },
        "star": { "type": "string", "description": "\"sol\" (default), \"auto\" (the seed's own star) or a spec such as K3" },
        "style": { "type": "string", "description": "earthlike (default), archipelago, islands, pangaea, continental, waterworld" },
        "history_years": { "type": "integer", "description": "Years of history to simulate (default 0)" },
        "civilizations": { "type": "integer" },
        "extra_args": { "type": "array", "items": { "type": "string" }, "description": "More planet_generator arguments, e.g. [\"--obliquity\", \"60\"]" },
        "fresh": { "type": "boolean", "description": "Remake the world file even if cached" },
        "id": { "type": "string", "description": "Name for the world in this session (default derived from the spec)" },
    }), &[])
}

fn generator_exe() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let bin = exe.with_file_name("planet_generator");
    if bin.exists() { Ok(bin) } else { Err(format!("{} not found: build it with `cargo build --release`", bin.display())) }
}

fn open_world(s: &mut Session, a: &Value) -> ToolResult {
    let t0 = std::time::Instant::now();
    // (file, id, how it was made ("" when reused), the land before its history)
    let (path, id, made, prehistory_path): (PathBuf, String, String, Option<PathBuf>) = if let Some(p) = get_str(a, "path") {
        let path = PathBuf::from(p);
        let id = get_str(a, "id").map(str::to_string).unwrap_or_else(|| path.file_stem().map(|f| f.to_string_lossy().into_owned()).unwrap_or_else(|| p.to_string()));
        // A world file opened by path: its land before the history, if saved beside it.
        let pre = path.with_extension("pre.world");
        (path, id, String::new(), Some(pre))
    } else {
        let dev = get_bool(a, "dev");
        let seed = get_u64(a, "seed");
        if !dev && seed.is_none() { return Err("give `path`, `dev: true` or a `seed`".into()); }
        let mut args: Vec<String> = Vec::new();
        let mut key = String::new();
        if dev { args.push("--dev".into()); key.push_str("dev"); }
        if let Some(seed) = seed { args.extend(["--seed".into(), seed.to_string()]); key.push_str(&format!("s{seed}")); }
        if !dev || get_usize(a, "width").is_some() {
            let (w, h) = (get_usize(a, "width").unwrap_or(512), get_usize(a, "height").unwrap_or(256));
            args.extend(["--width".into(), w.to_string(), "--height".into(), h.to_string()]);
            key.push_str(&format!("_{w}x{h}"));
        }
        let star = get_str(a, "star").unwrap_or(if dev { "" } else { "sol" });
        if !star.is_empty() && star != "auto" { args.extend(["--star".into(), star.into()]); key.push_str(&format!("_{star}")); }
        if let Some(style) = get_str(a, "style") { args.extend(["--world-style".into(), style.into()]); key.push_str(&format!("_{style}")); }
        if let Some(c) = get_usize(a, "civilizations") { args.extend(["--civilizations".into(), c.to_string()]); key.push_str(&format!("_c{c}")); }
        match get_usize(a, "history_years") {
            Some(0) if !dev => { args.push("--no-history".into()); key.push_str("_h0"); }
            Some(n) if n > 0 => { args.extend(["--history-years".into(), n.to_string()]); key.push_str(&format!("_h{n}")); }
            _ => { if !dev { args.push("--no-history".into()); key.push_str("_h0"); } }
        }
        if let Some(extra) = a.get("extra_args").and_then(Value::as_array) {
            let extra: Vec<String> = extra.iter().filter_map(|v| v.as_str().map(str::to_string)).collect();
            if !extra.is_empty() {
                use std::hash::{Hash, Hasher};
                let mut h = std::collections::hash_map::DefaultHasher::new();
                extra.hash(&mut h);
                key.push_str(&format!("_x{:08x}", h.finish() as u32));
                args.extend(extra);
            }
        }
        let key = key.trim_start_matches('_').to_string();
        let exe = generator_exe()?;
        std::fs::create_dir_all(&s.cache_dir).map_err(|e| format!("{}: {e}", s.cache_dir.display()))?;
        let path = s.cache_dir.join(format!("{key}.world"));
        let with_history = dev || get_usize(a, "history_years").map_or(false, |n| n > 0);
        let pre = with_history.then(|| s.cache_dir.join(format!("{key}.pre.world")));
        let stale = !path.exists() || older_than_build(&path, &exe) || get_bool(a, "fresh") || pre.as_ref().map_or(false, |p| !p.exists());
        let made = if stale {
            let t = std::time::Instant::now();
            let mut cmd = Command::new(&exe);
            cmd.args(&args).arg("--headless").arg("--save-world").arg(&path);
            if let Some(p) = &pre { cmd.arg("--save-prehistory").arg(p); }
            let out = cmd.output().map_err(|e| format!("running {}: {e}", exe.display()))?;
            let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
            if !out.status.success() || !path.exists() {
                return Err(format!("planet_generator {} failed ({}):\n{}", args.join(" "), out.status, tail_lines(&text, 30)));
            }
            format!("made with `planet_generator {} --headless --save-world {}` in {:.1}s", args.join(" "), path.display(), t.elapsed().as_secs_f32())
        } else { String::new() };
        (path, get_str(a, "id").map(str::to_string).unwrap_or(key), made, pre)
    };
    let (world, history) = crate::world::load_world(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let prehistory_path = prehistory_path.filter(|p| p.exists());
    s.insert(WorldEntry { id: id.clone(), path: path.clone(), prehistory_path, world, history, regions: Vec::new(), hist_index: None });
    let _ = s.world_index(&json!({ "world": id }))?;
    let mut out = summary_json(s, &json!({}))?;
    out["opened_in_s"] = json!((t0.elapsed().as_secs_f32() * 10.0).round() / 10.0);
    out["file"] = json!(path.display().to_string());
    out["made"] = json!(if made.is_empty() { "reused the world file".to_string() } else { made });
    Ok(Output::json(out))
}

fn tail_lines(s: &str, n: usize) -> String {
    let l: Vec<&str> = s.lines().collect();
    l[l.len().saturating_sub(n)..].join("\n")
}

fn list_worlds(s: &mut Session, _: &Value) -> ToolResult {
    let open: Vec<Value> = s.worlds.iter().map(|w| json!({
        "id": w.id, "file": w.path.display().to_string(), "size": format!("{}x{}", w.world.width, w.world.height),
        "seed": w.world.seed(), "history": w.history.is_some(), "current": s.current.as_deref() == Some(w.id.as_str()),
        "regions_kept": w.regions.iter().map(|(k, _)| format!("{},{} ({} tiles)", k.x, k.y, k.tiles)).collect::<Vec<_>>(),
    })).collect();
    let mut cached: Vec<String> = std::fs::read_dir(&s.cache_dir).map(|d| d.filter_map(|e| e.ok()).map(|e| e.path())
        .filter(|p| p.extension().map_or(false, |x| x == "world")).map(|p| p.display().to_string()).collect()).unwrap_or_default();
    cached.sort();
    Ok(Output::json(json!({ "open": open, "cached": cached })))
}

fn summary_json(s: &mut Session, a: &Value) -> Result<Value, String> {
    let e = s.entry(a)?;
    let w = &e.world;
    let land = (0..w.width * w.height).filter(|&i| *w.heightmap.get(i % w.width, i / w.width) >= 0.0).count();
    let gaz = crate::lore::build_gazetteer(w, e.history.as_ref(), w.seed());
    let mut by_kind: std::collections::BTreeMap<String, Vec<&crate::lore::Feature>> = Default::default();
    for f in &gaz.features { by_kind.entry(format!("{:?}", f.kind)).or_default().push(f); }
    let features: serde_json::Map<String, Value> = by_kind.into_iter().map(|(k, mut fs)| {
        fs.sort_by_key(|f| std::cmp::Reverse(f.size));
        let top: Vec<Value> = fs.iter().take(5).map(|f| json!({ "name": f.name, "at": [f.anchor.0, f.anchor.1], "size": f.size })).collect();
        (k, json!({ "count": fs.len(), "largest": top }))
    }).collect();
    let landmarks: Vec<Value> = crate::lore::find_landmarks(w, &gaz).iter()
        .map(|l| json!({ "kind": format!("{:?}", l.kind), "at": [l.x, l.y], "line": l.line() })).collect();
    let history = e.history.as_ref().map(|h| {
        let sm = h.summary();
        let living = h.settlements.values().filter(|s| !s.is_destroyed()).count();
        json!({
            "years": sm.years_simulated, "events": sm.total_events, "major_events": sm.major_events,
            "peoples": sm.total_factions, "active_peoples": sm.active_factions,
            "settlements": sm.total_settlements, "living_settlements": living, "ruins": sm.total_settlements - living,
            "figures": sm.total_figures, "living_figures": sm.living_figures, "wars": h.wars.len(),
            "present": h.present().counts(),
        })
    });
    Ok(json!({
        "id": e.id, "seed": w.seed(), "size": [w.width, w.height],
        // Every world has the Earth's radius (astro::HomeWorld): the equator over the map's width.
        "km_per_tile_at_equator": (40_075.0 / w.width as f32).round(),
        "land_share": ((land as f32 / (w.width * w.height) as f32) * 1000.0).round() / 1000.0,
        "star": w.star_system.as_ref().map(|s| s.summary()),
        "features": features, "landmarks": landmarks, "history": history,
    }))
}

fn world_summary(s: &mut Session, a: &Value) -> ToolResult { Ok(Output::json(summary_json(s, a)?)) }

// ---------------------------------------------------------------------------------------------
// A world tile

fn tile_schema() -> Value {
    let (x, y) = tile_props();
    schema(json!({ "world": world_prop(), "x": x, "y": y,
        "events": { "type": "integer", "description": "How many of the tile's chronicle events to list (default 12, newest first)" } }), &["x", "y"])
}

fn r1(v: f32) -> f64 { (v as f64 * 10.0).round() / 10.0 }
fn r3(v: f32) -> f64 { (v as f64 * 1000.0).round() / 1000.0 }

fn tile(s: &mut Session, a: &Value) -> ToolResult {
    let e = s.entry(a)?;
    let w = &e.world;
    let (x, y) = xy(a, w)?;
    let info = w.get_tile_info(x, y);
    let soils = w.soils();
    let res = w.resources();
    let deposits: Vec<Value> = res.deposits_at(x, y).map(|d| json!({ "kind": crate::lore::resources::resource_name(d.kind), "richness": d.richness })).collect();
    let gaz = crate::lore::build_gazetteer(w, e.history.as_ref(), w.seed());
    let member: Vec<String> = [&gaz.water, &gaz.relief, &gaz.region, &gaz.landmass].iter()
        .filter_map(|layer| gaz.feature(*layer.get(x, y))).map(|f| format!("{} ({:?})", f.name, f.kind)).collect();
    let landmarks: Vec<String> = crate::lore::find_landmarks(w, &gaz).iter().filter(|l| l.x == x && l.y == y).map(|l| l.line()).collect();
    let lat = 90.0 - (y as f32 + 0.5) / w.height as f32 * 180.0;
    let mut out = json!({
        "tile": [x, y], "latitude_deg": r1(lat),
        "elevation_m": r1(info.elevation), "temperature_c": r1(info.temperature), "moisture": r3(info.moisture),
        "biome": info.biome.display_name(), "biome_id": format!("{:?}", info.biome),
        "plate": format!("{:?}", info.plate_id), "stress": r3(info.stress), "hardness": info.hardness.map(r3),
        "water_body": format!("{:?}", info.water_body_type), "water_body_size": info.water_body_size, "water_depth_m": r1(info.water_depth),
        "flow_accumulation": w.flow_accumulation.as_ref().map(|f| r1(*f.get(x, y))),
        "river_tile": w.river_tile_cache.as_ref().map(|r| *r.get(x, y)),
        "soil": soils.describe(x, y), "fertility": r3(*res.fertility.get(x, y)), "timber": r3(*res.timber.get(x, y)), "fish": r3(*res.fish.get(x, y)),
        "deposits": deposits,
        "underground_water": if info.water_features.has_any() { json!(format!("{:?}", info.water_features)) } else { Value::Null },
        "lava": w.lava_map.as_ref().map(|l| format!("{:?}", l.get(x, y))),
        "features": member, "landmarks": landmarks,
    });
    if let Some(h) = e.history.as_ref() {
        let th = h.tile_history.get(x, y);
        let faction = |id: crate::history::FactionId| h.factions.get(&id).map(|f| f.name.clone()).unwrap_or_else(|| format!("{id}"));
        let settlement = |id: crate::history::SettlementId| h.settlements.get(&id).map(|s| json!({
            "id": id.0, "name": s.name, "people": faction(s.faction), "population": s.population,
            "founded": s.founded.year, "destroyed": s.is_destroyed(),
        }));
        let n = get_usize(a, "events").unwrap_or(12);
        let mut evs: Vec<&crate::history::events::Event> = h.chronicle.events_at_location(x, y);
        evs.sort_by_key(|e| std::cmp::Reverse((e.date.year, e.id.0)));
        let total = evs.len();
        out["history"] = json!({
            "owner": th.current_owner.map(faction),
            "owners_before": th.ownership.iter().rev().skip(1).take(6).map(|o| format!("{o:?}")).collect::<Vec<_>>(),
            "settlement": th.settlement.and_then(settlement),
            "former_settlements": th.former_settlements.iter().filter_map(|id| settlement(*id)).collect::<Vec<_>>(),
            "road": h.tile_history.has_road(x, y),
            "events_here": total,
            "latest_events": evs.iter().take(n).map(|e| json!({ "id": e.id.0, "year": e.date.year, "kind": format!("{:?}", e.event_type), "title": e.title })).collect::<Vec<_>>(),
        });
    }
    Ok(Output::json(out))
}

fn find_sites(s: &mut Session, a: &Value) -> ToolResult {
    let e = s.entry(a)?;
    let sites = crate::region::survey::find_sites(&e.world);
    Ok(Output::text(crate::region::survey::sites_report(&e.world, &sites)))
}

// ---------------------------------------------------------------------------------------------
// Regions

fn region_survey(s: &mut Session, a: &Value) -> ToolResult {
    let i = s.world_index(a)?;
    let (x, y) = xy(a, &s.worlds[i].world)?;
    let key = RegionKey { x, y, tiles: get_usize(a, "tiles").unwrap_or(8).max(1), scale: get_usize(a, "scale").unwrap_or(128), erosion: get_usize(a, "erosion").unwrap_or(40) };
    std::fs::create_dir_all(&s.out_dir).map_err(|e| e.to_string())?;
    let w = &s.worlds[i].world;
    let report = crate::region::survey::run(w, &zoom_params(w, key), &s.out_dir);
    let png = s.out_dir.join(format!("survey_{}_{}_{}.png", w.seed(), x, y));
    Ok(Output::text(report).with_files(if png.exists() { vec![png] } else { vec![] }))
}

fn places_schema() -> Value {
    schema(region_props(json!({
        "kind": { "type": "string", "description": "Only places whose kind's word holds this text" },
        "query": { "type": "string", "description": "Only places whose name, epithet, cause or description holds this text" },
        "limit": { "type": "integer", "description": "At most this many (default 200)" },
    })), &["x", "y"])
}

pub fn place_json(p: &Place, z: &ZoomRegion) -> Value {
    let s = (z.params.cells_per_tile.max(8) & !1) as f64;
    json!({
        "kind": p.kind.word(), "name": p.name, "epithet": p.epithet, "title": p.title(),
        "cause": p.cause, "description": p.description, "contents": p.contents,
        "abs_cell": [((p.x * 10.0).round() / 10.0), ((p.y * 10.0).round() / 10.0)],
        "window_cell": [(p.x - z.world_x0 as f64 * s).floor(), (p.y - z.world_y0 as f64 * s).floor()],
        "radius_m": p.radius_m, "footprint_cells": p.cells.len(), "z_m": p.z_m, "cave": p.cave, "rank": p.rank,
        "tile": [p.tile.0, p.tile.1],
    })
}

fn places(s: &mut Session, a: &Value) -> ToolResult {
    let (i, key) = s.region(a)?;
    let z = &s.worlds[i].regions.iter().find(|(k, _)| *k == key).unwrap().1;
    let kind = get_str(a, "kind").map(str::to_lowercase);
    let q = get_str(a, "query").map(str::to_lowercase);
    let limit = get_usize(a, "limit").unwrap_or(200);
    let matching: Vec<&Place> = z.places.iter().filter(|p| kind.as_ref().map_or(true, |k| p.kind.word().to_lowercase().contains(k.as_str())))
        .filter(|p| q.as_ref().map_or(true, |q| [&p.name, &p.epithet, &p.cause, &p.description].iter().any(|t| t.to_lowercase().contains(q.as_str())))).collect();
    let mut kinds: std::collections::BTreeMap<&str, usize> = Default::default();
    for p in &z.places { *kinds.entry(p.kind.word()).or_default() += 1; }
    let caves: Vec<Value> = z.caves.iter().enumerate().map(|(k, c)| json!({ "index": k, "name": c.name, "kind": format!("{:?}", c.kind), "nodes": c.nodes.len(), "passages": c.passages.len(), "tile": [c.tile.0, c.tile.1] })).collect();
    Ok(Output::json(json!({
        "region": { "center": [key.x, key.y], "tiles": key.tiles, "cells": [z.width, z.height], "cell_m": r1(z.cell_m), "world_x0": z.world_x0, "world_y0": z.world_y0 },
        "total_places": z.places.len(), "by_kind": kinds, "matching": matching.len(),
        "places": matching.iter().take(limit).map(|p| place_json(p, z)).collect::<Vec<_>>(),
        "caves": caves,
    })))
}

fn region_cell_schema() -> Value {
    schema(region_props(json!({
        "ax": { "type": "number", "description": "Absolute region cell column (the frame places use)" },
        "ay": { "type": "number", "description": "Absolute region cell row" },
        "cx": { "type": "integer", "description": "Cell column within the region's window" },
        "cy": { "type": "integer", "description": "Cell row within the region's window" },
        "near": { "type": "integer", "description": "How many nearest places to list (default 5)" },
    })), &["x", "y"])
}

fn region_cell(s: &mut Session, a: &Value) -> ToolResult {
    let (i, key) = s.region(a)?;
    let e = &s.worlds[i];
    let z = &e.regions.iter().find(|(k, _)| *k == key).unwrap().1;
    let sc = (z.params.cells_per_tile.max(8) & !1) as i64;
    let (ox, oy) = (z.world_x0 * sc, z.world_y0 * sc);
    let (cx, cy) = match (a.get("ax").and_then(Value::as_f64), a.get("ay").and_then(Value::as_f64), get_usize(a, "cx"), get_usize(a, "cy")) {
        (Some(ax), Some(ay), _, _) => (ax.floor() as i64 - ox, ay.floor() as i64 - oy),
        (_, _, Some(cx), Some(cy)) => (cx as i64, cy as i64),
        _ => ((z.width / 2) as i64, (z.height / 2) as i64),
    };
    if cx < 0 || cy < 0 || cx >= z.width as i64 || cy >= z.height as i64 {
        return Err(format!("cell {cx},{cy} is outside the region's {}x{} window (absolute cells {}..{}, {}..{})", z.width, z.height, ox, ox + z.width as i64, oy, oy + z.height as i64));
    }
    let k = cy as usize * z.width + cx as usize;
    let (ax, ay) = (cx + ox, cy + oy);
    let tile = ((ax.div_euclid(sc)).rem_euclid(e.world.width as i64) as usize, (ay.div_euclid(sc)).clamp(0, e.world.height as i64 - 1) as usize);
    let n = get_usize(a, "near").unwrap_or(5);
    let mut near: Vec<(f64, &Place)> = z.places.iter().map(|p| (((p.x - ax as f64 - 0.5).powi(2) + (p.y - ay as f64 - 0.5).powi(2)).sqrt() * z.cell_m as f64, p)).collect();
    near.sort_by(|p, q| p.0.total_cmp(&q.0));
    let info = e.world.get_tile_info(tile.0, tile.1);
    Ok(Output::json(json!({
        "window_cell": [cx, cy], "abs_cell": [ax, ay], "cell_m": r1(z.cell_m),
        "elevation_m": r1(z.elevation_m[k]), "landform": z.landform[k].name(), "rock": format!("{:?}", z.rock[k]),
        "river_width_m": r1(z.river_width_m[k]), "lake_depth_m": r1(z.lake_depth_m[k]), "drainage_cells": r1(z.drainage_cells[k]),
        "temperature_c": r1(z.temperature_c[k]), "moisture": r3(z.moisture[k]),
        "world_tile": { "tile": [tile.0, tile.1], "biome": info.biome.display_name(), "elevation_m": r1(info.elevation), "hardness": info.hardness.map(r3), "stress": r3(info.stress) },
        "nearest_places": near.iter().take(n).map(|(d, p)| json!({ "distance_m": d.round(), "title": p.title(), "kind": p.kind.word(), "cause": p.cause, "abs_cell": [p.x.floor(), p.y.floor()] })).collect::<Vec<_>>(),
    })))
}

fn views_agree(s: &mut Session, a: &Value) -> ToolResult {
    let (i, key) = s.region(a)?;
    let e = &s.worlds[i];
    let z = &e.regions.iter().find(|(k, _)| *k == key).unwrap().1;
    let (agree, judged, failed) = crate::local::sites::views_agree(&e.world, z);
    Ok(Output::json(json!({ "region": [key.x, key.y], "shown": agree, "judged": judged,
        "share": if judged > 0 { r3(agree as f32 / judged as f32) } else { 0.0 }, "not_shown": failed })))
}

fn census_schema() -> Value {
    schema(json!({ "world": world_prop(),
        "n": { "type": "integer", "description": "Windows to sample (default 24; 0 every window)" },
        "tiles": { "type": "integer", "description": "World tiles across each window (default 8)" } }), &[])
}

fn place_census(s: &mut Session, a: &Value) -> ToolResult {
    let e = s.entry(a)?;
    let (n, tiles) = (get_usize(a, "n").unwrap_or(24), get_usize(a, "tiles").unwrap_or(8).max(1));
    let c = crate::region::survey::census(&e.world, n, tiles, e.world.seed());
    Ok(Output::text(crate::region::survey::census_report(&c, e.world.seed(), tiles)))
}

// ---------------------------------------------------------------------------------------------
// Renders

fn render_world_schema() -> Value {
    let (x, y) = tile_props();
    schema(json!({ "world": world_prop(), "x": x, "y": y,
        "season": { "type": "string" }, "overlay": { "type": "string" },
        "history": { "type": "boolean", "description": "Draw the history on the map (default true when the world has one)" },
        "inline_images": { "type": "boolean", "description": "Also return the PNGs inline (MCP only)" } }), &[])
}

fn new_files(dir: &Path, since: std::time::SystemTime, prefix: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir).map(|d| d.filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with(prefix))
        .filter(|e| e.metadata().and_then(|m| m.modified()).map_or(false, |t| t >= since))
        .map(|e| e.path()).collect()).unwrap_or_default();
    v.sort();
    v
}

fn render_world(s: &mut Session, a: &Value) -> ToolResult {
    let i = s.world_index(a)?;
    let center = match (get_usize(a, "x"), get_usize(a, "y")) { (Some(_), Some(_)) => Some(xy(a, &s.worlds[i].world)?), _ => None };
    let season = match get_str(a, "season").unwrap_or("summer").to_lowercase().as_str() {
        "spring" => crate::seasons::Season::Spring, "autumn" | "fall" => crate::seasons::Season::Autumn,
        "winter" => crate::seasons::Season::Winter, _ => crate::seasons::Season::Summer,
    };
    use crate::tiles::overlays::Overlay;
    let overlay = match get_str(a, "overlay").map(str::to_lowercase).as_deref() {
        Some("height") => Overlay::Height, Some("temperature" | "temp") => Overlay::Temperature, Some("moisture") => Overlay::Moisture,
        Some("drainage" | "flow" | "rivers") => Overlay::Drainage, Some("plates") => Overlay::Plates, Some("stress") => Overlay::Stress,
        Some("biomes" | "biome") => Overlay::Biomes, Some("upheaval" | "faults" | "impacts" | "craters") => Overlay::Upheaval, None | Some("none" | "") => Overlay::None,
        Some(o) => return Err(format!("unknown overlay '{o}'")),
    };
    let stem = format!("{}_world_{}", s.worlds[i].id, center.map_or("auto".to_string(), |(x, y)| format!("{x}_{y}")));
    let prefix = s.out_path(&stem)?;
    s.atlas();
    let atlas = s.atlas.as_ref().unwrap();
    let e = &s.worlds[i];
    let hist = if a.get("history").and_then(Value::as_bool).unwrap_or(true) { e.history.as_ref() } else { None };
    let files = crate::tiles::viewer::save_snapshots(&e.world, hist, atlas, &prefix.to_string_lossy(), center, season, overlay).map_err(|e| e.to_string())?;
    Ok(Output::text(format!("World map rendered{}.", center.map_or(String::new(), |(x, y)| format!(" round {x},{y}"))))
        .with_files(files.into_iter().map(PathBuf::from).collect()))
}

fn render_embark_schema() -> Value {
    let (x, y) = tile_props();
    schema(json!({ "world": world_prop(), "x": x, "y": y,
        "place": { "type": "string", "description": "Embark at the region's nearest place whose kind or name holds this text, or a landform class" },
        "levels": { "type": "string", "description": "Extra level frames relative to the ground, e.g. \"2,-2,-6\" (PLANET_LOCAL_LEVELS)" },
        "history": { "type": "boolean", "description": "Use the world's history (default true)" },
        "inline_images": { "type": "boolean", "description": "Also return the PNGs inline (MCP only)" } }), &["x", "y"])
}

fn render_embark(s: &mut Session, a: &Value) -> ToolResult {
    let i = s.world_index(a)?;
    let (x, y) = xy(a, &s.worlds[i].world)?;
    let place = get_str(a, "place");
    let stem = format!("{}_embark_{x}_{y}{}", s.worlds[i].id, place.map_or(String::new(), |p| format!("_{}", p.replace(|c: char| !c.is_alphanumeric(), "-"))));
    let prefix = s.out_path(&stem)?;
    s.atlas();
    let atlas = s.atlas.as_ref().unwrap();
    let e = &s.worlds[i];
    let hist = if a.get("history").and_then(Value::as_bool).unwrap_or(true) { e.history.as_ref() } else { None };
    let set = |k: &str, v: Option<&str>| match v { Some(v) => std::env::set_var(k, v), None => std::env::remove_var(k) };
    let (old_place, old_levels) = (std::env::var("PLANET_EMBARK_PLACE").ok(), std::env::var("PLANET_LOCAL_LEVELS").ok());
    set("PLANET_EMBARK_PLACE", place);
    if let Some(l) = get_str(a, "levels") { set("PLANET_LOCAL_LEVELS", Some(l)); }
    let since = std::time::SystemTime::now() - std::time::Duration::from_secs(1);
    let res = crate::tiles::viewer::save_local_snapshots(&e.world, hist, atlas, Some((x, y)), &prefix.to_string_lossy()).map_err(|e| e.to_string());
    set("PLANET_EMBARK_PLACE", old_place.as_deref());
    set("PLANET_LOCAL_LEVELS", old_levels.as_deref());
    let mut files: Vec<PathBuf> = res?.into_iter().map(PathBuf::from).collect();
    // The region frames are written beside the embark's under the same prefix.
    for f in new_files(&s.out_dir, since, &stem) { if !files.contains(&f) { files.push(f); } }
    let mut out = Output::text(format!("Embark rendered round {x},{y}{}; its report is the log below.", place.map_or(String::new(), |p| format!(" at the nearest '{p}'")))).with_files(files);
    out.full_log = true;
    Ok(out)
}
