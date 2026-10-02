pub mod crust;
pub mod generation;
pub mod simulation;
pub mod stress;
pub mod types;

pub use generation::generate_plates;
pub use stress::{add_wiggle, calculate_stress, enhance_stress, smooth_stress, spread_stress};
pub use types::{Plate, PlateId, PlateType, Vec2, WorldStyle};
pub use crust::{generate_tectonic_terrain, TectonicTerrain};
pub use simulation::{TectonicParams, TectonicSim};
