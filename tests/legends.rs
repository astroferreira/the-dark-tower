//! The legends export (`--legends DIR`, `lore::legends`): the dev world's whole history as linked
//! pages. Every link must lead somewhere, every event must have its entry, an earlier camp's
//! legend gets a page, and the same seed writes the same pages.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::process::Command;

/// Run the dev world in `cwd` (which holds a camp's legend) and write its legends to `cwd/out`.
fn export(cwd: &Path, out: &str) -> String {
    let run = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .current_dir(cwd)
        .args(["--dev", "--headless", "--legends", out])
        .output()
        .expect("run planet_generator");
    let stderr = String::from_utf8_lossy(&run.stderr).to_string();
    assert!(run.status.success(), "planet_generator failed: {stderr}");
    stderr
}

/// Every value of `attr="..."` in `html`.
fn attrs<'a>(html: &'a str, attr: &str) -> Vec<&'a str> {
    let key = format!("{attr}=\"");
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(i) = rest.find(&key) {
        rest = &rest[i + key.len()..];
        let end = rest.find('"').unwrap();
        out.push(&rest[..end]);
        rest = &rest[end..];
    }
    out
}

/// The first name of `kind` in the search index ("[\"Name\",\"beast\",\"beast-3.html\"]").
fn first_of(js: &str, kind: &str) -> String {
    let at = js.find(&format!("\",\"{kind}\",\"")).unwrap_or_else(|| panic!("no {kind} in the index"));
    let start = js[..at].rfind("[\"").unwrap() + 2;
    js[start..at].to_string()
}

fn read_all(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(dir).unwrap().map(|e| {
        let p = e.unwrap().path();
        (p.file_name().unwrap().to_string_lossy().to_string(), std::fs::read(&p).unwrap())
    }).collect()
}

#[test]
fn dev_legends_are_whole_linked_and_stable() {
    let tmp = tempfile::tempdir().unwrap();
    let cwd = tmp.path();
    // First without camps, to learn a beast and a treasure of the world.
    export(cwd, "plain");
    let js = std::fs::read_to_string(cwd.join("plain/legends.js")).unwrap();
    let (beast, treasure) = (first_of(&js, "beast"), first_of(&js, "treasure"));
    // An earlier camp that slew that beast and kept that treasure.
    std::fs::create_dir_all(cwd.join("colonies")).unwrap();
    let camp = format!(r#"[{{"code": "76@45,12", "name": "the camp at 45,12", "tile": [45, 12], "day": 120, "alive": 5, "came": 7, "fate": "lives",
        "deeds": ["Day 52: Babagk drives a spear into {beast}, which does not rise."], "slain": ["{beast}"], "relic": "{treasure}, kept in the camp", "regards": []}}]"#);
    std::fs::write(cwd.join("colonies/legends_76.json"), camp).unwrap();
    let stderr = export(cwd, "a");
    let dir = cwd.join("a");
    let files = read_all(&dir);
    let count = |prefix: &str| files.keys().filter(|k| k.starts_with(prefix) && k.ends_with(".html")).count();
    let (figures, sites, beasts, wars, peoples, years) = (count("figure-"), count("site-"), count("beast-"), count("war-"), count("people-"), count("year-"));
    assert!(figures >= 200 && sites >= 30 && beasts >= 30 && wars >= 20 && peoples >= 4 && years >= 100,
        "too few pages: {figures} figures, {sites} sites, {beasts} beasts, {wars} wars, {peoples} peoples, {years} years");
    for page in ["index.html", "ages.html", "peoples.html", "sites.html", "figures.html", "beasts.html", "treasures.html", "wars.html", "faiths.html", "arts.html", "years.html", "camps.html", "legends.css", "legends.js", "map.png"] {
        assert!(files.contains_key(page), "no {page}");
    }

    // Every internal link and image resolves, anchors included.
    let mut ids: BTreeMap<&str, HashSet<&str>> = BTreeMap::new();
    let html: Vec<(&String, &str)> = files.iter().filter(|(k, _)| k.ends_with(".html")).map(|(k, v)| (k, std::str::from_utf8(v).unwrap())).collect();
    for (name, text) in &html { ids.insert(name.as_str(), attrs(text, "id").into_iter().collect()); }
    let mut links = 0;
    for (name, text) in &html {
        for target in attrs(text, "href").into_iter().chain(attrs(text, "src")) {
            if target.starts_with("http") { continue; }
            links += 1;
            let (file, anchor) = target.split_once('#').unwrap_or((target, ""));
            let file = if file.is_empty() { name.as_str() } else { file };
            assert!(files.contains_key(file), "{name} links to a missing page: {target}");
            if !anchor.is_empty() { assert!(ids[file].contains(anchor), "{name} links to a missing entry: {target}"); }
        }
    }
    assert!(links > 10_000, "only {links} links");

    // Every event of the chronicle has its entry on its year's page.
    let events: usize = stderr.lines().find_map(|l| l.strip_prefix("Events: ")).and_then(|l| l.split_whitespace().next()).and_then(|n| n.parse().ok()).expect("events line");
    let entries: usize = html.iter().filter(|(k, _)| k.starts_with("year-")).map(|(_, t)| t.matches("<article class=\"ev ").count()).sum();
    assert_eq!(entries, events, "every event has one entry");
    // Entries carry their causes; the camp's page links the beast it slew and the treasure it kept.
    assert!(html.iter().filter(|(k, _)| k.starts_with("year-")).any(|(_, t)| t.contains(">Because<")), "no cause chains");
    let camp = std::str::from_utf8(&files["camp-1.html"]).unwrap();
    assert!(camp.contains(&format!("\">{beast}</a>")) && camp.contains(&format!("\">{treasure}</a>")), "the camp's legend is not linked to the world: {camp}");

    // The same seed writes the same legends.
    export(cwd, "b");
    let again = read_all(&cwd.join("b"));
    assert_eq!(files.len(), again.len());
    for (k, v) in &files { assert!(again.get(k) == Some(v), "{k} differs between two runs"); }
}
