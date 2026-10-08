//! Caverns: three layers of open dark under every embark, lined up across the world.
//!
//! The idea is Dwarf Fortress's feature layers: coarse fields decide the fine detail. Each layer
//! sits at a depth below the region's smooth surface (the zoomed region's elevation without the
//! metre-scale relief), its openness comes from the world tile's rock (limestone hollows,
//! granite stays tight) and its water from the tile's water table, and its shape is 3-D noise
//! keyed on absolute position, so the caverns under two neighbouring embarks meet at the edge.
//! Deeper layers are taller and stranger. Each layer's floor grows fungus woods and cave moss,
//! pools gather in its hollows, and a list of what lives there; under the deepest one that holds
//! enough room sleeps a forgotten beast (`monsters.rs`), the same one under a 3x3 block of world
//! tiles, so neighbours share it.
//!
//! The colony meets them when a mine breaks in (`colony::dig`).

use super::*;
use crate::region::handshake::TileHandshake;

pub const LAYERS: usize = 3;
/// Depth of each layer's middle below the region's smooth surface (m).
const DEPTH_M: [f32; LAYERS] = [22.0, 52.0, 86.0];
/// Half the height of each layer's band (m).
const BAND_M: [f32; LAYERS] = [5.0, 6.5, 8.0];
/// Levels of rock kept over a cavern (no cavern breaks the surface).
const ROOF: i32 = 4;
/// Floor cells a layer needs to count.
const MIN_FLOOR: usize = 40;

/// What lives in each layer, by how wet it is (dry, wet).
const LIFE: [[&[&str]; 2]; LAYERS] = [
    [&["bats", "cave crickets", "pale spiders", "glowworms"], &["blind cave fish", "pale crabs", "bats", "cave salamanders"]],
    [&["giant cave spiders", "rock grubs", "crawling moles", "eyeless lizards"], &["eyeless salamanders", "giant cave toads", "pale eels", "giant cave spiders"]],
    [&["things that hunt by sound", "pale serpents", "giant rock grubs", "burrowing horrors"], &["pale serpents", "drowned crawlers", "blind giant crabs", "things that hunt by sound"]],
];

/// One layer as it lies under this embark.
#[derive(Clone, Debug, Default)]
pub struct Cavern {
    /// 0 the first (shallowest) .. 2 the third.
    pub layer: u8,
    pub name: String,
    pub floor_cells: usize,
    /// Mean floor level (z), and how many levels that lies under the mean surface.
    pub mean_z: i32,
    pub depth_levels: i32,
    pub water_cells: usize,
    pub fungus: usize,
    /// What lives there ("bats", "giant cave spiders").
    pub life: Vec<String>,
    /// A forgotten beast that sleeps there: its name and what it is.
    pub beast: Option<(String, crate::monsters::Monster)>,
}

/// Ordinal for a layer's name.
fn ordinal(k: usize) -> &'static str { ["first", "second", "third"][k.min(2)] }

/// How open the rock is to hollowing (karst most, crystalline least).
fn openness(rock: crate::erosion::materials::RockType) -> f32 {
    use crate::erosion::materials::RockType::*;
    match rock { Limestone => 0.22, Sandstone => 0.08, Shale => 0.04, Sediment => 0.0, Basalt => -0.04, Granite => -0.08, Ice => -0.3 }
}

/// Carve the caverns into `map` (after its rock is laid). `refs` is each column's smooth surface
/// elevation (m), `abs` a column's absolute position (m). Returns the layers that hold room.
pub(super) fn carve(map: &mut LocalMap, refs: &[f32], hs: &TileHandshake, abs: &dyn Fn(usize, usize) -> (f64, f64), seed: u32, tile: (usize, usize)) -> Vec<Cavern> {
    let n = map.width;
    let shape = Perlin::new(seed.wrapping_add(811));
    let detail = Perlin::new(seed.wrapping_add(812));
    let lift = Perlin::new(seed.wrapping_add(813));
    let band_n = Perlin::new(seed.wrapping_add(814));
    // The rock the layers lie in: deeper layers further down the tile's stack.
    let rock_at = |depth_m: f32| -> crate::erosion::materials::RockType {
        let mut d = (depth_m / Z_STEP_M) as i32;
        let mut rock = crate::erosion::materials::RockType::Granite;
        for l in &hs.rock_stack { rock = l.rock_type; if d <= l.thickness as i32 { break; } d -= l.thickness as i32; }
        rock
    };
    let wet = hs.water_table.clamp(0.0, 1.0);
    let mean_surface = map.surface_z.iter().map(|&z| z as f64).sum::<f64>() / map.surface_z.len().max(1) as f64;
    map.cavern_z = vec![[(-1, -1); LAYERS]; n * n];
    let mut out = Vec::new();
    for k in 0..LAYERS {
        let open_k = openness(rock_at(DEPTH_M[k])) + 0.04 * k as f32;
        let (mut floors, mut zsum) = (0usize, 0i64);
        for j in 0..n {
            for i in 0..n {
                let (mx, my) = abs(i, j);
                let col = j * n + i;
                let centre = refs[col] - DEPTH_M[k] + 10.0 * fbm(&lift, mx / 400.0, my / 400.0, 2);
                let band = BAND_M[k] * (0.45 + 0.75 * (0.5 + 0.5 * fbm(&band_n, mx / 150.0 + k as f64 * 31.0, my / 150.0, 2)));
                let sz = map.surface_z[col];
                let lo = (((centre - band - map.z_min_m) / Z_STEP_M).floor() as i32).max(2);
                let hi = (((centre + band - map.z_min_m) / Z_STEP_M).ceil() as i32).min(sz - ROOF);
                if hi <= lo { continue; }
                // Open cells in the band: 3-D noise over absolute position.
                let mut first_open: Option<i32> = None;
                let mut last_open = -1;
                for z in lo..=hi {
                    let zm = map.z_min_m + (z as f32 + 0.5) * Z_STEP_M;
                    let edge = 1.0 - ((zm - centre).abs() / band).min(1.0);
                    let v = shape.get([mx / 38.0, my / 38.0, zm as f64 / 9.0 + k as f64 * 50.0]) as f32
                        + 0.45 * detail.get([mx / 14.0, my / 14.0, zm as f64 / 4.0 + k as f64 * 50.0]) as f32;
                    if v + open_k + 0.5 * edge > 0.82 {
                        // Keep one solid run per column per layer (no floating slabs).
                        if first_open.is_none() { first_open = Some(z); }
                        if last_open >= 0 && z > last_open + 1 { break; }
                        last_open = z;
                    }
                }
                let Some(a) = first_open else { continue };
                if a - 1 < 1 || last_open < a { continue; }
                for z in a..=last_open {
                    let c = map.idx(i, j, z as usize);
                    map.cells[c] = Cell::AIR;
                }
                // The ground of the cavern is the cell below the open run.
                let f = map.idx(i, j, (a - 1) as usize);
                map.cells[f].shape = Shape::Floor;
                map.cavern_z[col][k] = ((a - 1) as i16, last_open as i16);
                floors += 1;
                zsum += (a - 1) as i64;
            }
        }
        if floors < MIN_FLOOR {
            // Too little room to count: fill it back in.
            for col in 0..n * n {
                let (f, t) = map.cavern_z[col][k];
                if f < 0 { continue; }
                let (i, j) = (col % n, col / n);
                let below = map.cell(i, j, (f - 1).max(0) as usize).material;
                for z in f..=t { let c = map.idx(i, j, z as usize); map.cells[c] = Cell { shape: Shape::Wall, material: below, water: 0, plant: Plant::None, boulder: false }; }
                map.cavern_z[col][k] = (-1, -1);
            }
            continue;
        }
        let mean_z = (zsum / floors as i64) as i32;
        // Pools in the hollows: water up to a level set by the tile's water table.
        let water_line = mean_z - 2 + (wet * 2.0).round() as i32 - k as i32;
        let (mut water_cells, mut fungus) = (0, 0);
        for col in 0..n * n {
            let (f, t) = map.cavern_z[col][k];
            if f < 0 { continue; }
            let (i, j) = (col % n, col / n);
            let (mx, my) = abs(i, j);
            let (gx, gy) = ((mx / TILE_M as f64).floor() as i64, (my / TILE_M as f64).floor() as i64);
            let mut flooded = false;
            for z in (f + 1)..=t.min(water_line as i16) {
                let c = map.idx(i, j, z as usize);
                map.cells[c].water = WATER_FULL;
                flooded = true;
                water_cells += 1;
            }
            if flooded { continue; }
            let fl = map.idx(i, j, f as usize);
            let r = hash(gx, gy, 0xCA7E + k as u64);
            if t > f + 1 && r < 0.05 + 0.08 * wet + 0.03 * k as f32 { map.cells[fl].plant = Plant::Tree(TreeKind::Fungus); fungus += 1; }
            else if r < 0.4 { map.cells[fl].plant = Plant::Grass; }
        }
        let hk = |salt: u64| { let mut h = (tile.0 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (tile.1 as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ salt ^ seed as u64; h ^= h >> 31; h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9); h ^ (h >> 29) };
        let pool = LIFE[k][if wet > 0.45 { 1 } else { 0 }];
        let mut life: Vec<String> = Vec::new();
        for s in 0..3u64 { let x = pool[(hk(0x11FE + s * 7 + k as u64) % pool.len() as u64) as usize]; if !life.iter().any(|l| l == x) { life.push(x.to_string()); } }
        out.push(Cavern {
            layer: k as u8, name: format!("the {} cavern", ordinal(k)), floor_cells: floors, mean_z,
            depth_levels: (mean_surface as i32 - mean_z).max(0), water_cells, fungus, life, beast: None,
        });
    }
    // A forgotten beast sleeps in the deepest layer with room (the same under a 3x3 block of
    // world tiles: its seed is the block's).
    if let Some(deep) = out.iter_mut().filter(|c| c.floor_cells >= 300).max_by_key(|c| c.layer) {
        let block = ((tile.0 / 3) as u64, (tile.1 / 3) as u64);
        let bseed = block.0.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ block.1.wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ (seed as u64).wrapping_mul(0xFB);
        let mut spheres = vec!["darkness".to_string(), "earth".to_string()];
        if wet > 0.45 { spheres.push("water".into()); }
        if matches!(rock_at(DEPTH_M[deep.layer as usize]), crate::erosion::materials::RockType::Basalt) { spheres.push("fire".into()); }
        let size = 2.0 + (bseed % 200) as f32 / 100.0;
        let m = crate::monsters::generate(&crate::monsters::Request { kind: "forgotten".into(), spheres, size, evil: bseed % 3 == 0, ..Default::default() }, bseed);
        use rand::SeedableRng;
        let style = crate::history::naming::styles::NamingStyle::from_archetype(crate::history::NamingStyleId(0), crate::history::naming::styles::NamingArchetype::Harsh);
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(bseed);
        let name = crate::history::naming::generator::NameGenerator::personal_name(&style, &mut rng);
        deep.beast = Some((name, m));
    }
    out
}

impl LocalMap {
    /// Which cavern layer the cell (x, y, z) lies in (its floor or the open space above it).
    pub fn cavern_at(&self, x: usize, y: usize, z: i32) -> Option<usize> {
        let col = self.cavern_z.get(y * self.width + x)?;
        col.iter().position(|&(f, t)| f >= 0 && z >= f as i32 && z <= t as i32)
    }
}
