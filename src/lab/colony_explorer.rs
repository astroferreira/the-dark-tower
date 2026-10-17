//! The colony explorer: a self-contained HTML app over a recorded colony (`colony_explorer`).
//! Views: the camp's days as a reader (moments, the log, each settler's page with their past
//! linked into the history explorer), an activity chart (a row per settler, who did what when,
//! with the why of every stretch), needs and stocks over time, the map replayed (frames of the
//! camp as it stood, everyone's walks), the bonds between settlers through the days, the works
//! rising, and the checks; with a second colony (a branch), the two side by side. The app is
//! `colony_explorer/` (HTML, CSS, JS, no outside libraries); the data is `data.js` beside it.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::colony::{self, ColonyRun};
use crate::history::world_state::WorldHistory;

const INDEX: &str = include_str!("colony_explorer/index.html");
const CSS: &str = include_str!("explorer/explorer.css");
const CSS2: &str = include_str!("colony_explorer/colony.css");
const JS: &str = include_str!("colony_explorer/colony.js");

pub struct Written {
    pub dir: PathBuf,
    pub decisions: usize,
    pub samples: usize,
    pub frames: usize,
    pub levels: usize,
    pub data_bytes: usize,
}

/// Write the app for colony `ci` (and `other` to compare) into `dir`.
pub fn write(runs: &mut [ColonyRun], ci: usize, other: Option<usize>, history: Option<&WorldHistory>, atlas: &crate::tiles::Atlas, dir: &Path, history_link: Option<&str>) -> std::io::Result<Written> {
    std::fs::create_dir_all(dir)?;
    let io = |e: String| std::io::Error::new(std::io::ErrorKind::Other, e);
    // The camp as it stands now closes the frames.
    for k in std::iter::once(ci).chain(other) {
        let r = &mut runs[k];
        if r.rec.frames.last().map_or(true, |f| f.0 != r.colony.clock.tick) { r.frame(history, atlas); }
    }
    let r = &runs[ci];
    // Frames live in the run's folder; another folder gets copies.
    if r.dir != dir {
        std::fs::create_dir_all(dir.join("frames"))?;
        for (_, f) in &r.rec.frames { let _ = std::fs::copy(r.frames_dir().join(f), dir.join("frames").join(f)); }
    }
    // The delve's levels as they stand now.
    let mut levels = Vec::new();
    let camp_z = r.colony.camp_z();
    let lv: Vec<i32> = r.colony.delve_levels().into_iter().filter(|&z| z != camp_z).take(14).collect();
    if !lv.is_empty() { std::fs::create_dir_all(dir.join("levels"))?; }
    for z in lv {
        let file = format!("levels/z{z}.png");
        colony::render_level(&r.colony, history, z, 3.0, &dir.join(&file));
        let rooms: Vec<String> = r.colony.rooms.iter().filter(|m| m.z == z).map(|m| format!("{:?}", m.kind)).collect();
        levels.push(json!([z, file, z - camp_z, rooms]));
    }
    std::fs::write(dir.join("annals.html"), r.colony.annals(history))?;
    let mut d = colony::data(r, history, history_link);
    d["meta"]["levels"] = json!(levels);
    d["meta"]["annals"] = json!("annals.html");
    let text = format!("window.COLONY = {};\n", serde_json::to_string(&d).map_err(|e| io(e.to_string()))?);
    std::fs::write(dir.join("data.js"), &text)?;
    let other_text = match other {
        Some(oi) => {
            let o = &runs[oi];
            let mut d2 = colony::data(o, history, history_link);
            // Its frames from its own folder.
            let rel = relative(dir, &o.frames_dir());
            if let Some(fr) = d2["meta"]["frames"].as_array_mut() {
                for f in fr.iter_mut() { if let Some(p) = f[1].as_str() { f[1] = json!(format!("{}/{}", rel, p.trim_start_matches("frames/"))); } }
            }
            format!("window.COLONY_B = {};\n", serde_json::to_string(&d2).map_err(|e| io(e.to_string()))?)
        }
        None => "window.COLONY_B = null;\n".to_string(),
    };
    std::fs::write(dir.join("data2.js"), other_text)?;
    std::fs::write(dir.join("colony.css"), format!("{CSS}\n/* The colony explorer's own. */\n{CSS2}"))?;
    std::fs::write(dir.join("colony.js"), JS)?;
    let title = d["meta"]["name"].as_str().unwrap_or("The camp").to_string();
    std::fs::write(dir.join("index.html"), INDEX.replace("{{TITLE}}", &title))?;
    Ok(Written {
        dir: dir.to_path_buf(), decisions: d["spans"].as_array().map_or(0, |a| a.len()), samples: r.rec.pos.len(),
        frames: r.rec.frames.len(), levels: d["meta"]["levels"].as_array().map_or(0, |a| a.len()), data_bytes: text.len(),
    })
}

/// `to` relative to the folder `from`.
fn relative(from: &Path, to: &Path) -> String {
    let abs = |p: &Path| std::env::current_dir().map(|d| d.join(p)).unwrap_or_else(|_| p.to_path_buf());
    let (a, b) = (abs(from), abs(to));
    let ac: Vec<_> = a.components().collect();
    let bc: Vec<_> = b.components().collect();
    let common = ac.iter().zip(bc.iter()).take_while(|(x, y)| x == y).count();
    let mut out: Vec<String> = std::iter::repeat("..".to_string()).take(ac.len() - common).collect();
    out.extend(bc[common..].iter().map(|c| c.as_os_str().to_string_lossy().to_string()));
    if out.is_empty() { ".".into() } else { out.join("/") }
}
