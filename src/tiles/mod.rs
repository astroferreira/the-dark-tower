//! Graphical tile viewer (Dwarf Fortress style): a pixel-art tile atlas, world classification
//! into tiles, pure software rendering, and a minifb window around it.

pub mod atlas;
pub mod beasts;
pub mod camp_ink;
pub mod ink;
pub mod inventory;
pub mod sprite_sheet;
mod colony_hud;
mod colony_ui;
pub mod coverage;
pub mod classify;
pub mod folk;
pub mod fonts;
pub mod fx_ink;
pub mod furniture;
pub mod glyphs;
pub mod heraldry;
pub mod inspector;
pub mod local_ink;
pub mod overlays;
pub mod plates;
pub mod portraits;
pub mod region_ink;
pub mod render;
pub mod start;
pub mod status_ink;
pub mod text;
mod ui;
pub mod viewer;
pub mod vignette;
pub mod watcher;
pub mod world_ink;

pub use atlas::{Atlas, TileKind};
pub use classify::TileWorld;
pub use viewer::run_tile_viewer;
