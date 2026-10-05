//! Every name in the annals must be something the world holds. A reader who looks up "the bronze
//! people" or "Centaur raiders" and finds nothing stops trusting the rest of the chronicle.
//!
//! The dev world's chronicle is dumped (`--chronicle-dump`: every event's text, and every name
//! the world holds). A capitalised word inside a sentence must then be part of a real name, or a
//! word the game itself writes (taken from the string literals in `src/` and the data files,
//! except the old stock lists of enemies, adjectives and beasts, which named nothing real).

use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

/// Capitalised words (letters, apostrophes, hyphens) in `text`, with whether each starts a
/// sentence.
fn capitalised(text: &str) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    let mut sentence_start = true;
    for raw in text.split_whitespace() {
        let word: String = raw.trim_matches(|c: char| !c.is_alphanumeric() && c != '\'' && c != '-').to_string();
        let word = word.trim_end_matches("'s").to_string();
        if word.chars().next().map_or(false, |c| c.is_uppercase()) {
            out.push((word, sentence_start));
        }
        sentence_start = raw.ends_with('.') || raw.ends_with('!') || raw.ends_with('?') || raw.ends_with(':');
    }
    out
}

fn words_of(text: &str, into: &mut HashSet<String>) {
    for w in text.split(|c: char| !c.is_alphanumeric() && c != '\'' && c != '-') {
        let w = w.trim_matches(|c: char| c == '\'' || c == '-').trim_end_matches("'s");
        if !w.is_empty() { into.insert(w.to_string()); }
    }
}

/// Words in the string literals of every `.rs` file under `dir`.
fn literal_words(dir: &Path, into: &mut HashSet<String>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() { literal_words(&path, into); continue; }
        if path.extension().map_or(true, |e| e != "rs") { continue; }
        let src = std::fs::read_to_string(&path).unwrap();
        for (i, part) in src.split('"').enumerate() {
            if i % 2 == 1 { words_of(part, into); }
        }
    }
}

/// Words in the data files, skipping the stock lists that named nothing real.
fn data_words(dir: &Path, into: &mut HashSet<String>) {
    const STOCK: [&str; 3] = ["enemy_names", "faction_adjectives", "beast_names"];
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().map_or(true, |e| e != "json") { continue; }
        let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        fn walk(v: &serde_json::Value, into: &mut HashSet<String>) {
            match v {
                serde_json::Value::String(s) => words_of(s, into),
                serde_json::Value::Array(a) => a.iter().for_each(|x| walk(x, into)),
                serde_json::Value::Object(o) => o.iter().filter(|(k, _)| !STOCK.contains(&k.as_str())).for_each(|(_, x)| walk(x, into)),
                _ => {}
            }
        }
        walk(&json, into);
    }
}

#[test]
fn dev_chronicle_names_only_real_things() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dir = std::env::temp_dir().join(format!("flavour_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let dump = dir.join("chronicle.tsv");
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--dev", "--headless", "--chronicle-dump", dump.to_str().unwrap()])
        .output()
        .expect("run planet_generator");
    assert!(out.status.success(), "planet_generator failed: {}", String::from_utf8_lossy(&out.stderr));
    // Every treasure and monument names a real maker or deed ("Objects: 54 of 54 artifacts ...").
    let stderr = String::from_utf8_lossy(&out.stderr);
    let objects = stderr.lines().find(|l| l.starts_with("Objects:")).expect("objects line");
    let nums: Vec<usize> = objects.split(|c: char| !c.is_ascii_digit()).filter_map(|n| n.parse().ok()).collect();
    assert!(nums.len() >= 5 && nums[0] == nums[1] && nums[3] == nums[4] && nums[2] == 0,
        "treasures or monuments that remember nothing, or repeated names: {objects}");
    // Story sifting finds tales worth retelling: 10+ of 3+ linked events, of 4+ kinds.
    let tales = stderr.lines().find(|l| l.starts_with("Tales:")).expect("tales line");
    let rich: usize = tales.split('(').nth(1).and_then(|x| x.split_whitespace().next()).and_then(|n| n.parse().ok()).unwrap_or(0);
    let kinds = tales.split("events): ").nth(1).map_or(0, |x| x.split(", ").count());
    assert!(rich >= 10 && kinds >= 4, "too few tales worth telling: {tales}");
    let text = std::fs::read_to_string(&dump).unwrap();
    let _ = std::fs::remove_dir_all(&dir);

    let mut known = HashSet::new();
    literal_words(&root.join("src"), &mut known);
    data_words(&root.join("data/defaults"), &mut known);
    let mut events = Vec::new();
    for line in text.lines() {
        let cols: Vec<&str> = line.split('\t').collect();
        match cols.as_slice() {
            ["N", name] => words_of(name, &mut known),
            ["E", year, title, desc] => events.push((year.to_string(), title.to_string(), desc.to_string())),
            _ => {}
        }
    }
    assert!(events.len() > 1000, "too few events in the dump: {}", events.len());

    let mut unbound: Vec<String> = Vec::new();
    for (year, title, desc) in &events {
        for (field, starts) in [(title, true), (desc, false)] {
            for (i, (word, sentence_start)) in capitalised(field).into_iter().enumerate() {
                // A title's first word and a sentence's first word are capitalised anyway.
                if (starts && i == 0) || sentence_start { continue; }
                if !known.contains(&word) {
                    unbound.push(format!("{year}: '{word}' in \"{field}\""));
                }
            }
        }
    }
    unbound.sort();
    unbound.dedup();
    assert!(unbound.is_empty(), "{} names in the dev chronicle resolve to nothing:\n{}",
        unbound.len(), unbound.iter().take(40).cloned().collect::<Vec<_>>().join("\n"));
}
