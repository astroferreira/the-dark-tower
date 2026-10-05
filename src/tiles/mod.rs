//! Graphical tile viewer (Dwarf Fortress style): a pixel-art tile atlas, world classification
//! into tiles, pure software rendering, and a minifb window around it.

pub mod atlas;
pub mod classify;
pub mod inspector;
pub mod local_ink;
pub mod overlays;
pub mod render;
pub mod start;
pub mod text;
mod ui;
pub mod viewer;
pub mod watcher;

pub use atlas::{Atlas, TileKind};
pub use classify::TileWorld;
pub use viewer::run_tile_viewer;
