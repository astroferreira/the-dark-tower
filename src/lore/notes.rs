//! Marginalia: the player writes in the chronicle.
//!
//! Chronicle lines stay terse (what, who, where, because) and leave room: the player pins notes to
//! places on the map. Notes live beside the world (`notes_<seed>.json`, or `<world file>.notes.json`
//! for a saved world), are drawn on the map in a handwritten face, and the journal sets each one in
//! the margin beside the first entry of its place, and lists them all in a Marginalia part.

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Note {
    pub x: usize,
    pub y: usize,
    pub text: String,
}

static PATH: OnceLock<String> = OnceLock::new();

/// Where this world's notes are kept (set once by `main`).
pub fn set_path(path: String) { let _ = PATH.set(path); }

pub fn path(seed: u64) -> String { PATH.get().cloned().unwrap_or_else(|| format!("notes_{}.json", seed)) }

pub fn load(seed: u64) -> Vec<Note> {
    std::fs::read_to_string(path(seed)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn save(seed: u64, notes: &[Note]) -> std::io::Result<()> {
    std::fs::write(path(seed), serde_json::to_string_pretty(notes).unwrap_or_default())
}

/// Pin a note and save.
pub fn add(seed: u64, note: Note) -> std::io::Result<Vec<Note>> {
    let mut all = load(seed);
    all.push(note);
    save(seed, &all)?;
    Ok(all)
}

/// Whether a world tile is within `r` tiles of a note (wrapping east-west on `width`).
pub fn near(n: &Note, x: usize, y: usize, width: usize, r: usize) -> bool {
    let dx = n.x.abs_diff(x);
    dx.min(width.saturating_sub(dx)) <= r && n.y.abs_diff(y) <= r
}
