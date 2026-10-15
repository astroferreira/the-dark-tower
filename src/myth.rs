//! Myth and magic: on or off for the whole game (2026-10-13: the game takes a medieval, realistic
//! road, and the fantasy layer is off by default).
//!
//! Off (the default): no Shadow, no legendary beasts or forgotten beasts, every people human
//! (their tongues still differ), no fantasy biomes or crystal wastes, no ghosts, risen dead,
//! werebeasts, vampires or hauntings, caverns holding only what real caves hold (no fungus
//! forests, giant spiders or what sleeps below), no magma sea, adamantine or hollow under the
//! world, no possessed moods, and adventure mode (a fantasy of its own) closed.
//! On (`--myth`, or `PLANET_MYTH=1`): the game as it was, bit for bit; the tests of those systems
//! run with it.
//!
//! The binary sets it once in `main`; a library unit test sees it on (the systems' own tests).

use std::sync::OnceLock;

static MYTH: OnceLock<bool> = OnceLock::new();

/// Set once, at start (`main`).
pub fn set(on: bool) { let _ = MYTH.set(on); }

/// Whether myth and magic are in the world.
pub fn on() -> bool { *MYTH.get().unwrap_or(&cfg!(test)) }

/// The environment's wish (`PLANET_MYTH=1`), for `main`.
pub fn from_env() -> bool { std::env::var("PLANET_MYTH").map_or(false, |v| v != "0" && !v.is_empty()) }
