//! The terrain's revision (2026-10-13, the terrain audit in `audit.md`).
//!
//! Revision 2 (the default) is the audited pipeline: constructive passes before erosion, a crust
//! hardness map feeding the erosion, flow accumulation and climate brought up to date with the
//! final heightmap, curvature with the right sign (and plan/profile curvature), the sphere's
//! metric in gradients, flow routing and blurs, seamless and pole-true noise coordinates,
//! Euclidean distance fields, signal-weighted ridged noise, asymmetric orogens with foreland
//! basins, shelf-slope-rise margins and fracture zones, fjords traced along glaciated valleys,
//! volcano profiles by type and lava that runs downhill, water tables by wetness index,
//! waterfalls of their true height, biome feathering across the whole zone, and day length and
//! maritime lag from the sun's hour angle.
//!
//! Revision 1 (`PLANET_TERRAIN_V1=1`, or `--terrain-v1`) is the terrain as it was, bit for bit:
//! the seed-pinned integration tests run with it (the dev world's tiles and timelines).

use std::sync::OnceLock;

static V1: OnceLock<bool> = OnceLock::new();

/// Set at start (`main`'s `--terrain-v1`); otherwise read from the environment on first use.
pub fn set_v1(on: bool) { let _ = V1.set(on); }

/// Whether the old terrain (revision 1) is asked for.
pub fn v1() -> bool {
    *V1.get_or_init(|| std::env::var("PLANET_TERRAIN_V1").map_or(false, |v| v != "0" && !v.is_empty()))
}
