//! Signs of the animals that live around a playable area, from the world's ecology: game trails
//! worn down to water, burrows in open soil, nests in the trees, predator dens among the rocks
//! with the bones of their kills, and the bones of old battles on a bone field.
//!
//! Everything is placed by position hash, so the same spot always looks the same.

use crate::biomes::ExtendedBiome;
use crate::history::ecology::{BEAR, LION, SPECIES, WOLF, Trophic};
use crate::lore::RegionLore;
use crate::world::WorldData;

use super::{LocalMap, Material, Plant, Shape};

/// A mark left by animals (or the dead) on a surface tile.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Feature {
    None,
    /// Trampled path; grass worn away.
    Trail,
    Burrow,
    Nest,
    /// A predator's den.
    Den,
    Bones,
}

impl Feature {
    pub fn name(self) -> &'static str {
        match self {
            Feature::None => "",
            Feature::Trail => "game trail",
            Feature::Burrow => "burrow",
            Feature::Nest => "nest",
            Feature::Den => "den",
            Feature::Bones => "bones",
        }
    }
}

fn hash(a: i64, b: i64, salt: u64) -> u64 {
    let mut h = (a as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (b as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ salt;
    h ^= h >> 31;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^ (h >> 29)
}

fn unit(a: i64, b: i64, salt: u64) -> f32 {
    (hash(a, b, salt) % 1_000_000) as f32 / 1_000_000.0
}

/// Add signs of life to `map`. `origin` is the absolute tile position of the map's (0, 0), used
/// to key the hashes.
pub fn apply(map: &mut LocalMap, world: &WorldData, lore: &RegionLore, origin: (i64, i64)) {
    let n = map.width;
    let Some(dens) = lore.wildlife.get(&map.world_tile) else { return };
    let grazers: f32 = SPECIES.iter().zip(dens).filter(|(s, _)| s.trophic == Trophic::Grazer).map(|(_, d)| *d).sum();
    let predators = dens[WOLF] + dens[BEAR] + dens[LION];
    let bone_field = matches!(*world.biomes.get(map.world_tile.0, map.world_tile.1), ExtendedBiome::BoneFields | ExtendedBiome::TitanBones);

    let surface = |map: &LocalMap, x: usize, y: usize| map.idx(x, y, map.surface_z[y * n + x] as usize);
    let open = |map: &LocalMap, x: usize, y: usize| {
        let c = map.cells[surface(map, x, y)];
        let above_free = (map.surface_z[y * n + x] as usize + 1) >= map.depth
            || map.cells[map.idx(x, y, map.surface_z[y * n + x] as usize + 1)].shape != Shape::Wall;
        c.water == 0 && matches!(c.shape, Shape::Floor | Shape::Ramp) && above_free
            && !matches!(c.material, Material::Wood | Material::Block(_) | Material::Ice)
            && map.features[y * n + x] == Feature::None
    };

    // --- Trails: least-cost paths down to water --------------------------------------------
    // Cost to reach water from every cell (Dijkstra, 8 neighbours). Ground cost varies with a
    // position hash smoothed over ~8 tiles and with climbing, so paths wind, follow easy ground
    // and merge as they near the water, like real game trails.
    let rough = |x: usize, y: usize| -> f32 {
        let (gx, gy) = ((origin.0 + x as i64).div_euclid(8), (origin.1 + y as i64).div_euclid(8));
        let (fx, fy) = (((origin.0 + x as i64).rem_euclid(8)) as f32 / 8.0, ((origin.1 + y as i64).rem_euclid(8)) as f32 / 8.0);
        let v = |a: i64, b: i64| unit(a, b, 71);
        let top = v(gx, gy) + (v(gx + 1, gy) - v(gx, gy)) * fx;
        let bot = v(gx, gy + 1) + (v(gx + 1, gy + 1) - v(gx, gy + 1)) * fx;
        top + (bot - top) * fy
    };
    let mut cost = vec![f32::INFINITY; n * n];
    let mut heap = std::collections::BinaryHeap::new();
    #[derive(PartialEq)]
    struct Item(f32, usize);
    impl Eq for Item {}
    impl PartialOrd for Item { fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(o)) } }
    impl Ord for Item { fn cmp(&self, o: &Self) -> std::cmp::Ordering { o.0.partial_cmp(&self.0).unwrap_or(std::cmp::Ordering::Equal) } }
    for y in 0..n {
        for x in 0..n {
            let sz = map.surface_z[y * n + x] as usize;
            if sz + 1 < map.depth && map.cells[map.idx(x, y, sz + 1)].water > 0 {
                cost[y * n + x] = 0.0;
                heap.push(Item(0.0, y * n + x));
            }
        }
    }
    const STEPS: [(i64, i64, f32); 8] = [(1, 0, 1.0), (-1, 0, 1.0), (0, 1, 1.0), (0, -1, 1.0), (1, 1, 1.414), (1, -1, 1.414), (-1, 1, 1.414), (-1, -1, 1.414)];
    while let Some(Item(c, i)) = heap.pop() {
        if c > cost[i] { continue; }
        let (x, y) = (i % n, i / n);
        for (dx, dy, len) in STEPS {
            let (nx, ny) = (x as i64 + dx, y as i64 + dy);
            if nx < 0 || ny < 0 || nx >= n as i64 || ny >= n as i64 { continue; }
            let j = ny as usize * n + nx as usize;
            let climb = (map.surface_z[j] - map.surface_z[i]).abs();
            if climb > 1 { continue; } // animals take steps of at most one level
            let nc = c + len * (1.0 + 3.0 * rough(nx as usize, ny as usize) + 2.0 * climb as f32);
            if nc < cost[j] { cost[j] = nc; heap.push(Item(nc, j)); }
        }
    }
    let trails = (grazers * 6.0).round() as usize;
    for t in 0..trails {
        // Start somewhere well away from water and follow the cost field down to it.
        let (mut x, mut y) = ((hash(origin.0, t as i64, 31) % n as u64) as usize, (hash(origin.1, t as i64, 37) % n as u64) as usize);
        if !cost[y * n + x].is_finite() || cost[y * n + x] < 60.0 { continue; }
        for _ in 0..4 * n {
            if cost[y * n + x] <= 0.0 { break; }
            let k = surface(map, x, y);
            if open(map, x, y) || map.features[y * n + x] == Feature::Trail {
                if !matches!(map.cells[k].plant, Plant::Tree(_)) { map.cells[k].plant = Plant::None; }
                map.cells[k].boulder = false;
                map.features[y * n + x] = Feature::Trail;
            }
            // Step to the neighbour that is cheapest to reach water from.
            let mut best: Option<(f32, usize, usize)> = None;
            for (dx, dy, _) in STEPS {
                let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                if nx < 0 || ny < 0 || nx >= n as i64 || ny >= n as i64 { continue; }
                let c = cost[ny as usize * n + nx as usize];
                if c < cost[y * n + x] && best.map_or(true, |b| c < b.0) { best = Some((c, nx as usize, ny as usize)); }
            }
            match best { Some((_, nx, ny)) => { x = nx; y = ny; } None => break }
        }
    }

    // --- Burrows, nests, dens, bones ---------------------------------------------------------
    for y in 0..n {
        for x in 0..n {
            if !open(map, x, y) { continue; }
            let (ax, ay) = (origin.0 + x as i64, origin.1 + y as i64);
            let k = surface(map, x, y);
            let c = map.cells[k];
            let r = unit(ax, ay, 53);
            let soft = matches!(c.material, Material::Soil | Material::Sand | Material::Clay);
            if bone_field && r < 0.06 {
                map.features[y * n + x] = Feature::Bones;
            } else if soft && c.plant != Plant::None && !matches!(c.plant, Plant::Tree(_)) && r < 0.0015 + 0.004 * grazers {
                map.features[y * n + x] = Feature::Burrow;
            } else if matches!(c.plant, Plant::Tree(_)) && r < 0.02 {
                map.features[y * n + x] = Feature::Nest;
            } else if predators > 0.15 && c.boulder && r < 0.08 * predators {
                // A den in the lee of a boulder; the bones of kills lie around it.
                map.features[y * n + x] = Feature::Den;
                for (dx, dy) in [(1i64, 1i64), (-1, 2), (2, -1)] {
                    let (bx, by) = (x as i64 + dx, y as i64 + dy);
                    if bx < 0 || by < 0 || bx >= n as i64 || by >= n as i64 { continue; }
                    if unit(ax + dx, ay + dy, 59) < 0.6 && open(map, bx as usize, by as usize) {
                        map.features[by as usize * n + bx as usize] = Feature::Bones;
                    }
                }
            } else if r > 0.9995 - 0.0015 * predators {
                map.features[y * n + x] = Feature::Bones;
            }
        }
    }
}
