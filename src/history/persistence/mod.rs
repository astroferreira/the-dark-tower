//! History persistence - save/load and export.
//!
//! Saves and loads WorldHistory to/from JSON files, and exports
//! legends summaries to readable text and markdown formats.

pub mod serialize;

pub use serialize::{save_history, load_history, export_legends_text, export_legends_markdown};
