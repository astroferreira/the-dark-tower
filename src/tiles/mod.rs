//! Graphical tile viewer (Dwarf Fortress style): a pixel-art tile atlas, world classification
//! into tiles, pure software rendering, and a minifb window around it.

pub mod atlas;
pub mod beasts;
pub mod camp_ink;
pub mod ink;
pub mod sprite_sheet;
mod colony_hud;
mod colony_ui;
pub mod classify;
pub mod folk;
pub mod fonts;
pub mod furniture;
pub mod glyphs;
pub mod heraldry;
pub mod inspector;
pub mod local_ink;
pub mod overlays;
pub mod plates;
pub mod portraits;
pub mod render;
pub mod start;
pub mod text;
mod ui;
pub mod viewer;
pub mod watcher;

pub use atlas::{Atlas, TileKind};
pub use classify::TileWorld;
pub use viewer::run_tile_viewer;
