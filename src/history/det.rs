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

/// A quick fixed hasher (FxHash, as in rustc) for small keys looked up in hot loops (a cell's
/// position, a 3D place). Its iteration order differs from `HashMap`'s: use these only for maps
/// and sets that are never iterated (lookups, inserts, removes), so swapping one in changes
/// nothing a run does.
#[derive(Default, Clone, Copy)]
pub struct FxHasher { hash: u64 }

impl FxHasher {
    #[inline]
    fn add(&mut self, w: u64) { self.hash = (self.hash.rotate_left(5) ^ w).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95); }
}

impl std::hash::Hasher for FxHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) { for &b in bytes { self.add(b as u64); } }
    #[inline] fn write_u8(&mut self, i: u8) { self.add(i as u64); }
    #[inline] fn write_u16(&mut self, i: u16) { self.add(i as u64); }
    #[inline] fn write_u32(&mut self, i: u32) { self.add(i as u64); }
    #[inline] fn write_u64(&mut self, i: u64) { self.add(i); }
    #[inline] fn write_usize(&mut self, i: usize) { self.add(i as u64); }
    #[inline] fn finish(&self) -> u64 { self.hash }
}

pub type FastHasher = BuildHasherDefault<FxHasher>;
/// Not to be iterated (see `FxHasher`).
pub type FastMap<K, V> = std::collections::HashMap<K, V, FastHasher>;
/// Not to be iterated (see `FxHasher`).
pub type FastSet<T> = std::collections::HashSet<T, FastHasher>;
