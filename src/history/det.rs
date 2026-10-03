//! Hash maps and sets with a fixed hasher, so iteration order is the same every run.
//!
//! The standard `HashMap` seeds its hasher randomly per process. Much of the history
//! simulation iterates maps (factions, settlements, figures...) while drawing from the RNG,
//! so a random iteration order made the same seed tell a different history each run. These
//! aliases make history (and the lore built from it) reproducible from the seed.

use std::collections::hash_map::DefaultHasher;
use std::hash::BuildHasherDefault;

pub type Hasher = BuildHasherDefault<DefaultHasher>;
pub type HashMap<K, V> = std::collections::HashMap<K, V, Hasher>;
pub type HashSet<T> = std::collections::HashSet<T, Hasher>;
