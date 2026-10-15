//! Places: the world's springs and waterfalls brought down to the region (ROADMAP Stage 1), the
//! natural wonders and the ages before (Stage 4), and the cave graphs the embark carves (Stage
//! 5). Planned per world tile from the blended region's fields within the tile and its
//! neighbours (the margin guarantees them), so any window plans a tile's places the same way;
//! positions are absolute region cells (`region::plan`).
//!
//! `mod.rs` fixes only the order; each file keeps its rules, numbers and names.

pub mod water;
pub mod natural;
pub mod prehistory;
pub mod names;
pub mod caves;

use crate::erosion::materials::RockType;
use crate::region::landforms::RegionFields;
use crate::region::plan::{CaveGraph, Landform, Place};

/// What the planner made for a window: the places of its core tiles and the caves they open into.
#[derive(Clone, Debug, Default)]
pub struct SitePlan {
    pub places: Vec<Place>,
    pub caves: Vec<CaveGraph>,
}

/// The requested window's core tiles: (x0, y0, tiles across, tiles down), x unwrapped.
#[derive(Clone, Copy, Debug)]
pub struct Core {
    pub x0: i64,
    pub y0: i64,
    pub w: usize,
    pub h: usize,
}

impl Core {
    /// Every core tile as (unwrapped x, y).
    pub fn tiles(&self) -> impl Iterator<Item = (i64, i64)> + '_ {
        (self.y0..self.y0 + self.h as i64).flat_map(move |y| (self.x0..self.x0 + self.w as i64).map(move |x| (x, y)))
    }
}

/// Plan the places of the window's core tiles from the blended fields and their labels.
pub fn plan(f: &RegionFields, land: &[Landform], rock: &[RockType], core: Core) -> SitePlan {
    let mut out = SitePlan::default();
    water::plan(f, land, rock, core, &mut out);
    natural::plan(f, land, rock, core, &mut out);
    prehistory::plan(f, land, rock, core, &mut out);
    caves::plan(f, land, rock, core, &mut out);
    names::name_all(f, &mut out);
    out
}
