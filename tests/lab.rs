//! The lab (`src/lab/`): the MCP handshake, the tool list's schemas and the output's form.
//! Worlds are made by the game binary, so the tools that need one are checked by hand
//! (`planet_lab call open_world '{"dev":true}' ...`, see `src/lab/CLAUDE.md`).

use planet_generator::lab::{self, mcp, Session};
use serde_json::{json, Value};

#[test]
fn mcp_handshake_lists_tools_and_answers_errors() {
    let mut s = Session::new();
    let init = mcp::handle(&mut s, &json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "test", "version": "0" } } })).unwrap();
    assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
    assert!(init["result"]["capabilities"]["tools"].is_object());
    assert!(mcp::handle(&mut s, &json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })).is_none(), "notifications get no reply");

    let list = mcp::handle(&mut s, &json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })).unwrap();
    let names: Vec<&str> = list["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    for want in ["open_world", "tile", "places", "region_cell", "render_embark", "history_events", "history_why", "history_branch", "colony_found", "colony_why", "colony_explorer"] {
        assert!(names.contains(&want), "missing tool {want}: {names:?}");
    }

    // A tool error is a result with isError, not a protocol error.
    let call = mcp::handle(&mut s, &json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": "tile", "arguments": { "x": 1, "y": 1 } } })).unwrap();
    assert_eq!(call["result"]["isError"], true);
    assert!(call["result"]["content"][0]["text"].as_str().unwrap().contains("open_world"));
    let unknown = mcp::handle(&mut s, &json!({ "jsonrpc": "2.0", "id": 4, "method": "nope" })).unwrap();
    assert_eq!(unknown["error"]["code"], -32601);
}

#[test]
fn every_tool_has_a_valid_schema() {
    let mut seen = std::collections::HashSet::new();
    for t in lab::tools() {
        assert!(seen.insert(t.name), "tool {} listed twice", t.name);
        assert!(t.description.len() > 40, "{} needs a description an agent can choose by", t.name);
        let s: Value = (t.schema)();
        assert_eq!(s["type"], "object", "{}", t.name);
        let props = s["properties"].as_object().unwrap_or_else(|| panic!("{} has no properties", t.name));
        for r in s["required"].as_array().unwrap() {
            assert!(props.contains_key(r.as_str().unwrap()), "{} requires {} it does not describe", t.name, r);
        }
    }
}

#[test]
fn output_keeps_short_values_on_one_line() {
    let v = json!({ "at": [12, 34], "places": [{ "kind": "a tor", "abs_cell": [1.0, 2.0] }], "long": "x".repeat(200) });
    let text = lab::pretty(&v, 0);
    assert!(text.contains("\"at\": [12,34]"), "{text}");
    assert!(text.contains("{\"abs_cell\":[1.0,2.0],\"kind\":\"a tor\"}"), "{text}");
}

#[test]
fn capture_takes_what_the_library_prints() {
    // Written to the descriptors themselves: the test harness intercepts println! in tests.
    let raw = |fd: i32, s: &str| unsafe { libc::write(fd, s.as_ptr() as *const libc::c_void, s.len()); };
    let (n, log) = lab::capture(|| { raw(1, "to stdout\n"); raw(2, "to stderr\n"); 7 });
    assert_eq!(n, 7);
    assert!(log.contains("to stdout") && log.contains("to stderr"), "{log:?}");
}

/// The explorer's data refers only to things it holds: every cause is an event, every code a
/// named thing, every town's and tile's holder a people; and the app is written whole.
#[test]
fn the_history_explorer_holds_together() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("dev.world");
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--headless", "--save-world"]).arg(&path).output().expect("run the game");
    assert!(status.status.success(), "{}", String::from_utf8_lossy(&status.stderr));
    let (world, history) = planet_generator::world::load_world(&path).unwrap();
    let h = history.expect("the dev world has a history");
    let d = lab::explorer::data(&world, &h, "dev", false);

    let events = d["events"].as_array().unwrap();
    assert_eq!(events.len(), h.chronicle.events.len());
    let ids: std::collections::HashSet<u64> = events.iter().map(|e| e[0].as_u64().unwrap()).collect();
    let mut codes = std::collections::HashSet::new();
    for (key, letter) in [("peoples", "F"), ("towns", "S"), ("figures", "P"), ("houses", "H"), ("wars", "W"), ("beasts", "B"), ("artifacts", "A"), ("monuments", "M"), ("faiths", "R"), ("gods", "G"), ("pops", "C")] {
        for o in d[key].as_array().unwrap() { codes.insert(format!("{letter}{}", o["i"])); }
    }
    let kinds = d["kinds"].as_array().unwrap().len();
    for e in events {
        assert!((e[3].as_u64().unwrap() as usize) < kinds, "kind index out of range: {e}");
        for c in e[11].as_array().unwrap() { assert!(ids.contains(&c.as_u64().unwrap()), "event {} names a missing cause {c}", e[0]); }
        for p in e[9].as_array().unwrap() { assert!(codes.contains(p.as_str().unwrap()), "event {} names {p}, which the data lacks", e[0]); }
        for f in e[10].as_array().unwrap() { assert!(codes.contains(&format!("F{f}")), "event {} names people {f}, which the data lacks", e[0]); }
    }
    for t in d["towns"].as_array().unwrap() {
        assert!(codes.contains(&format!("F{}", t["p"])), "town {} of a missing people", t["n"]);
        for r in t["own"].as_array().unwrap() { assert!(codes.contains(&format!("F{}", r[0])), "town {} held by a missing people", t["n"]); }
    }
    let terr = d["territory"].as_array().unwrap();
    assert_eq!(terr.len() % 4, 0);
    for r in terr.chunks(4) {
        assert!((r[0].as_u64().unwrap() as usize) < world.width * world.height);
        assert!(codes.contains(&format!("F{}", r[1])), "a tile held by a missing people {}", r[1]);
    }
    assert!(!d["ages"].as_array().unwrap().is_empty());

    let out = dir.path().join("explorer");
    let atlas = planet_generator::tiles::Atlas::generated();
    let w = lab::explorer::write(&world, &h, "dev", &atlas, &out, false).unwrap();
    for f in ["index.html", "explorer.js", "explorer.css", "data.js", "map.png"] { assert!(out.join(f).exists(), "{f} not written"); }
    assert!(w.data_bytes > 100_000);
    if let Ok(o) = std::process::Command::new("node").arg("--check").arg(out.join("explorer.js")).output() {
        assert!(o.status.success(), "explorer.js does not parse: {}", String::from_utf8_lossy(&o.stderr));
    }
}

/// The colony recorder tells the game's own story: a colony founded and lived 30 days in the
/// lab on the dev world (loaded from its file) has the same colony hash as `--sim-snapshot` on
/// the freshly made world; a second, unrecorded run agrees; the recording holds together; and
/// the colony explorer is written whole.
#[test]
fn a_colony_is_recorded_as_the_game_lives_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("dev.world");
    let made = std::process::Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--headless", "--save-world"]).arg(&path).output().expect("run the game");
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let snap = std::process::Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--headless", "--sim-snapshot"]).arg(dir.path().join("sim")).output().expect("run the game");
    let text = String::from_utf8_lossy(&snap.stdout);
    let game_hash = text.lines().find_map(|l| l.strip_prefix("Colony hash: ")).map(|l| l.split_whitespace().next().unwrap().to_string()).expect("the game prints its colony hash");

    let mut s = Session::new();
    s.out_dir = dir.path().join("out");
    s.call("open_world", &json!({ "path": path.to_string_lossy() })).unwrap();
    s.call("colony_found", &json!({ "days": 30, "frame_days": 10 })).unwrap();
    let status = s.call("colony_status", &json!({})).unwrap();
    let lab::Body::Json(st) = &status.body else { panic!("colony_status answers JSON") };
    assert_eq!(st["hash"].as_str().unwrap(), game_hash, "the lab's colony is not the game's (a loaded world must found the same camp)");

    let checks = s.call("colony_checks", &json!({ "replay": true })).unwrap();
    let lab::Body::Json(ck) = &checks.body else { panic!("colony_checks answers JSON") };
    let same = ck["findings"].as_array().unwrap().iter().find(|f| f["check"].as_str().unwrap().contains("same story")).unwrap();
    assert_eq!(same["count"], 0, "a second run differs: {}", same["examples"]);

    let r = &s.colonies[0];
    let mut spans = r.rec.spans.clone();
    spans.extend(r.rec.open_spans(r.colony.clock.tick));
    for i in 0..r.colony.settlers.len() {
        let mut mine: Vec<_> = spans.iter().filter(|x| x.who == i).collect();
        mine.sort_by_key(|x| x.t0);
        for w in mine.windows(2) { assert!(w[0].t1 <= w[1].t0, "{}'s choices overlap at {}", r.colony.settlers[i].name, w[1].t0); }
        assert!(mine.iter().all(|x| x.t0 <= x.t1 && (x.why as usize) < r.rec.whys.len()));
    }
    assert!(spans.len() > 1000, "only {} choices recorded in 30 days", spans.len());
    assert_eq!(r.rec.pos.len() as u64, 30 * 1440 / r.rec.every + 1, "a sample at the founding and every few minutes after");

    let out = dir.path().join("colony");
    let w = s.call("colony_explorer", &json!({ "dir": out.to_string_lossy(), "history": false })).unwrap();
    assert!(w.render().contains("decisions"));
    for f in ["index.html", "colony.js", "colony.css", "data.js", "data2.js", "annals.html"] { assert!(out.join(f).exists(), "{f} not written"); }
    assert!(std::fs::read_dir(out.join("frames")).unwrap().count() >= 4, "the map's frames are missing");
    if let Ok(o) = std::process::Command::new("node").arg("--check").arg(out.join("colony.js")).output() {
        assert!(o.status.success(), "colony.js does not parse: {}", String::from_utf8_lossy(&o.stderr));
    }
}
