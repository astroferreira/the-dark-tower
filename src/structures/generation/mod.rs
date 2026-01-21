//! Structure generation algorithms
//!
//! This module contains various procedural generation algorithms for
//! creating human-made structures:
//!
//! - BSP (Binary Space Partitioning) for room layouts
//! - L-Systems for castle walls and complex patterns
//! - Cellular automata for decay/ruins effects
//! - Dijkstra-based road network generation
//! - Organic shape generation (circles, blobs, irregular shapes)
//! - Mines, shafts, and underground fortresses

pub mod bsp;
pub mod decay;
pub mod lsystem;
pub mod mines;
pub mod roads;
pub mod shapes;

pub use bsp::*;
pub use decay::*;
pub use lsystem::*;
pub use mines::*;
pub use roads::*;
pub use shapes::*;
