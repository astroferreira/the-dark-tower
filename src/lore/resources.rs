//! Where the world's wealth is, derived from its geology and ecology.
//!
//! * **Metals** follow tectonics: copper, gold and silver in convergent arcs and orogens
//!   (high stress, volcanic neighbourhoods), tin and silver in granite uplands, iron in hard
//!   rock belts, gems around volcanoes and deep roots.
//! * **Coal** lies in ancient wet lowlands over sedimentary rock; **salt** in dry basins and
//!   salt flats and along arid coasts.
//! * **Land**: farmland (fertility) from water, warmth, flat ground, river floodplains and
//!   volcanic soils; **timber** from forests; **fish** from shallow cool seas off the coast.
//!
//! Everything is a pure function of the world, so history, the map and the embarks all agree.

use crate::history::det::HashMap;

use noise::{NoiseFn, Perlin};

use crate::biomes::ExtendedBiome;
use crate::erosion::materials::RockType;
use crate::history::civilizations::economy::ResourceType;
use crate::tilemap::Tilemap;
use crate::world::WorldData;

#[derive(Clone, Copy, Debug)]
pub struct Deposit {
    pub x: usize,
    pub y: usize,
    pub kind: ResourceType,
    /// 1 = poor, 2 = good, 3 = rich.
    pub richness: u8,
}

pub struct ResourceMap {
    pub deposits: Vec<Deposit>,
    by_tile: HashMap<(usize, usize), Vec<usize>>,
    /// Farmland quality 0..1.
    pub fertility: Tilemap<f32>,
    /// Forest cover 0..1.
    pub timber: Tilemap<f32>,
    /// Fishing quality 0..1 (sea tiles near the coast).
    pub fish: Tilemap<f32>,
}

/// Display colour for a resource.
pub fn resource_color(r: ResourceType) -> [u8; 3] {
    use ResourceType::*;
    match r {
        Iron => [150, 90, 70],
        Copper => [70, 190, 150],
        Tin => [190, 200, 210],
        Gold => [255, 210, 40],
        Silver => [225, 230, 240],
        Gems | Diamonds | Rubies | Emeralds => [220, 90, 220],
        Coal => [30, 30, 36],
        Salt => [250, 250, 245],
        Mithril => [140, 200, 255],
        Adamantine => [90, 60, 160],
        Stone => [150, 150, 150],
        Wood => [110, 80, 40],
        Fish => [90, 160, 220],
        _ => [200, 200, 200],
    }
}

pub fn resource_name(r: ResourceType) -> &'static str {
    use ResourceType::*;
    match r {
        Iron => "iron", Copper => "copper", Tin => "tin", Gold => "gold", Silver => "silver",
        Gems => "gems", Diamonds => "diamonds", Rubies => "rubies", Emeralds => "emeralds",
        Coal => "coal", Salt => "salt", Mithril => "mithril", Adamantine => "adamantine",
        Stone => "stone", Wood => "timber", Fish => "fish", Food => "farmland", Herbs => "herbs",
        _ => "goods",
    }
}

/// The rock a metal is found in, for vein placement underground (None = anywhere).
pub fn host_rocks(r: ResourceType) -> &'static [RockType] {
    use ResourceType::*;
    match r {
        Iron => &[RockType::Sandstone, RockType::Granite, RockType::Basalt, RockType::Shale],
        Copper => &[RockType::Basalt, RockType::Granite, RockType::Sandstone],
        Tin => &[RockType::Granite],
        Gold => &[RockType::Granite, RockType::Limestone, RockType::Basalt],
        Silver => &[RockType::Granite, RockType::Limestone],
        Gems | Diamonds | Rubies | Emeralds => &[RockType::Basalt, RockType::Granite],
        Coal => &[RockType::Shale, RockType::Sandstone, RockType::Sediment],
        Salt => &[RockType::Limestone, RockType::Shale, RockType::Sediment],
        _ => &[],
    }
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl ResourceMap {
    pub fn deposits_at(&self, x: usize, y: usize) -> impl Iterator<Item = &Deposit> {
        self.by_tile.get(&(x, y)).into_iter().flatten().map(move |&i| &self.deposits[i])
    }

    /// Deposits within `radius` tiles (wrap-aware in x).
    pub fn deposits_near(&self, x: usize, y: usize, radius: i64, w: usize) -> Vec<&Deposit> {
        let mut out = Vec::new();
        for dy in -radius..=radius {
            let yy = y as i64 + dy;
            if yy < 0 { continue; }
            for dx in -radius..=radius {
                if dx * dx + dy * dy > radius * radius { continue; }
                let xx = (x as i64 + dx).rem_euclid(w as i64) as usize;
                out.extend(self.deposits_at(xx, yy as usize));
            }
        }
        out
    }

    /// Goods a settlement at (x, y) can produce locally, with relative quantity.
    pub fn local_production(&self, world: &WorldData, x: usize, y: usize) -> Vec<(ResourceType, f32)> {
        let (w, h) = (world.width, world.height);
        let r = 4i64;
        let (mut fert, mut timber, mut fish, mut n) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        let mut stone = 0.0f32;
        for dy in -r..=r {
            let yy = y as i64 + dy;
            if yy < 0 || yy >= h as i64 { continue; }
            for dx in -r..=r {
                if dx * dx + dy * dy > r * r { continue; }
                let xx = (x as i64 + dx).rem_euclid(w as i64) as usize;
                fert += *self.fertility.get(xx, yy as usize);
                timber += *self.timber.get(xx, yy as usize);
                fish += *self.fish.get(xx, yy as usize);
                if *world.heightmap.get(xx, yy as usize) > 400.0 { stone += 1.0; }
                n += 1.0;
            }
        }
        let n = n.max(1.0);
        let mut out = Vec::new();
        let food = (fert / n * 1.6 + fish / n * 1.2).min(2.0);
        out.push((ResourceType::Food, food.max(0.15)));
        if timber / n > 0.2 { out.push((ResourceType::Wood, timber / n * 2.0)); }
        if fish / n > 0.08 { out.push((ResourceType::Fish, fish / n * 3.0)); }
        if stone / n > 0.3 { out.push((ResourceType::Stone, stone / n)); }
        for d in self.deposits_near(x, y, r + 2, w) {
            out.push((d.kind, d.richness as f32));
        }
        out
    }

    /// Fertility averaged around a tile (population capacity).
    pub fn fertility_near(&self, x: usize, y: usize, radius: i64, w: usize) -> f32 {
        let h = self.fertility.height;
        let (mut s, mut n) = (0.0f32, 0.0f32);
        for dy in -radius..=radius {
            let yy = y as i64 + dy;
            if yy < 0 || yy >= h as i64 { continue; }
            for dx in -radius..=radius {
                if dx * dx + dy * dy > radius * radius { continue; }
                s += *self.fertility.get((x as i64 + dx).rem_euclid(w as i64) as usize, yy as usize);
                n += 1.0;
            }
        }
        s / n.max(1.0)
    }

    /// Value of the deposits near a site: how attractive it is to settle or fight over.
    pub fn wealth_near(&self, x: usize, y: usize, radius: i64, w: usize) -> f32 {
        self.deposits_near(x, y, radius, w).iter().map(|d| d.richness as f32 * d.kind.base_value() as f32).sum()
    }
}

pub fn compute_resources(world: &WorldData) -> ResourceMap {
    let (w, h) = (world.width, world.height);
    let hm = &world.heightmap;
    let seed = world.seed() as u32;
    let land = |x: usize, y: usize| *hm.get(x, y) > 0.0 && !world.water_body_map.get(x, y).is_lake();

    // Volcano proximity (tiles), a cheap distance field.
    let mut volc = Tilemap::new_with(w, h, f32::MAX);
    {
        use std::collections::VecDeque;
        let mut q = VecDeque::new();
        for v in &world.volcanoes {
            if v.x < w && v.y < h { volc.set(v.x, v.y, 0.0); q.push_back((v.x, v.y)); }
        }
        while let Some((x, y)) = q.pop_front() {
            let d = *volc.get(x, y) + 1.0;
            if d > 12.0 { continue; }
            for (nx, ny) in volc.neighbors_8(x, y) {
                if d < *volc.get(nx, ny) { volc.set(nx, ny, d); q.push_back((nx, ny)); }
            }
        }
    }

    // ---- Land quality ----
    // Farmland: a warm, wet enough climate on good soil (`world.soils()`: black earth, alluvium
    // and volcanic soil are rich, laterite, podzol and thin stony soils poor).
    let soils = world.soils();
    let mut fertility = Tilemap::new_with(w, h, 0.0f32);
    let mut timber = Tilemap::new_with(w, h, 0.0f32);
    for y in 0..h {
        for x in 0..w {
            if !land(x, y) { continue; }
            let e = *hm.get(x, y);
            let t = *world.temperature.get(x, y);
            let m = *world.moisture.get(x, y);
            let warm = smoothstep(-6.0, 8.0, t) * (1.0 - smoothstep(30.0, 38.0, t));
            let wet = smoothstep(0.05, 0.4, m) * (1.0 - 0.3 * smoothstep(0.7, 1.0, m));
            let low = 1.0 - 0.6 * smoothstep(200.0, 2500.0, e);
            let soil = soils.kind.get(x, y).fertility();
            let f = warm * wet * low * (0.15 + 0.85 * soil);
            let biome = *world.biomes.get(x, y);
            fertility.set(x, y, f.clamp(0.0, 1.0));
            let tr = match biome {
                ExtendedBiome::TemperateForest | ExtendedBiome::TemperateRainforest | ExtendedBiome::TropicalRainforest
                | ExtendedBiome::TropicalForest | ExtendedBiome::MontaneForest | ExtendedBiome::CloudForest => 1.0,
                ExtendedBiome::BorealForest | ExtendedBiome::SubalpineForest => 0.85,
                ExtendedBiome::MonsoonForest => 0.8,
                ExtendedBiome::MediterraneanShrubland => 0.2,
                ExtendedBiome::Savanna | ExtendedBiome::Swamp | ExtendedBiome::MangroveSaltmarsh => 0.3,
                _ => 0.0,
            };
            timber.set(x, y, tr);
        }
    }

    // ---- Fish: shallow sea within a few tiles of land, richer in cool water ----
    let mut fish = Tilemap::new_with(w, h, 0.0f32);
    {
        use std::collections::VecDeque;
        let mut dist = Tilemap::new_with(w, h, i32::MAX);
        let mut q = VecDeque::new();
        for y in 0..h { for x in 0..w { if land(x, y) { dist.set(x, y, 0); q.push_back((x, y)); } } }
        while let Some((x, y)) = q.pop_front() {
            let d = *dist.get(x, y) + 1;
            if d > 6 { continue; }
            for (nx, ny) in dist.neighbors(x, y) {
                if d < *dist.get(nx, ny) { dist.set(nx, ny, d); q.push_back((nx, ny)); }
            }
        }
        for y in 0..h {
            for x in 0..w {
                let e = *hm.get(x, y);
                let d = *dist.get(x, y);
                if e > 0.0 || d == i32::MAX || d == 0 { continue; }
                let shelf = 1.0 - smoothstep(60.0, 800.0, -e);
                let t = *world.temperature.get(x, y);
                let cool = 0.6 + 0.4 * (1.0 - smoothstep(10.0, 26.0, t));
                let near = 1.0 - (d as f32 - 1.0) / 6.0;
                fish.set(x, y, (shelf * cool * near).clamp(0.0, 1.0));
            }
        }
    }

    // ---- Ore deposits ----
    let clump = |kind_salt: u32, scale: f64| Perlin::new(seed.wrapping_add(kind_salt));
    let area = (w * h) as f32 / (512.0 * 256.0);
    struct Spec { kind: ResourceType, salt: u32, count: f32, spacing: i64 }
    let specs = [
        Spec { kind: ResourceType::Iron, salt: 11, count: 44.0, spacing: 7 },
        Spec { kind: ResourceType::Copper, salt: 12, count: 30.0, spacing: 8 },
        Spec { kind: ResourceType::Tin, salt: 13, count: 14.0, spacing: 10 },
        Spec { kind: ResourceType::Gold, salt: 14, count: 16.0, spacing: 12 },
        Spec { kind: ResourceType::Silver, salt: 15, count: 16.0, spacing: 11 },
        Spec { kind: ResourceType::Gems, salt: 16, count: 14.0, spacing: 12 },
        Spec { kind: ResourceType::Coal, salt: 17, count: 34.0, spacing: 7 },
        Spec { kind: ResourceType::Salt, salt: 18, count: 26.0, spacing: 8 },
    ];
    let stress = &world.stress_map;
    // Granite anywhere in the top layers (uplifted basement) counts for tin and silver.
    let has_granite = |x: usize, y: usize| -> bool {
        world.handshakes.as_ref()
            .map(|hs| hs.get(x, y).tile.rock_stack.iter().take(3).any(|l| l.rock_type == RockType::Granite))
            .unwrap_or(false)
    };
    let rock_at = |x: usize, y: usize| -> RockType {
        world.handshakes.as_ref()
            .and_then(|hs| hs.get(x, y).tile.rock_stack.first().map(|l| l.rock_type))
            .unwrap_or(RockType::Sandstone)
    };
    let mut deposits: Vec<Deposit> = Vec::new();
    for spec in &specs {
        let noise = clump(spec.salt, 1.0);
        let mut scored: Vec<(f32, usize, usize)> = Vec::new();
        for y in (2..h.saturating_sub(2)).step_by(1) {
            for x in 0..w {
                if !land(x, y) { continue; }
                let e = *hm.get(x, y);
                let s = *stress.get(x, y);
                let hard = world.hardness_map.as_ref().map(|m| *m.get(x, y)).unwrap_or(0.5);
                let m = *world.moisture.get(x, y);
                let biome = *world.biomes.get(x, y);
                let vd = *volc.get(x, y);
                let rock = rock_at(x, y);
                let volcanic = if vd < 12.0 { 1.0 - vd / 12.0 } else { 0.0 };
                let orogen = smoothstep(0.05, 0.3, s);
                let suit = match spec.kind {
                    ResourceType::Iron => 0.25 + 0.6 * orogen + 0.4 * hard.min(1.0) * smoothstep(100.0, 700.0, e) + if matches!(rock, RockType::Sandstone) { 0.15 } else { 0.0 },
                    ResourceType::Copper => 0.7 * orogen + 0.7 * volcanic + if matches!(rock, RockType::Basalt) { 0.3 } else { 0.0 },
                    ResourceType::Tin => if has_granite(x, y) { 0.5 * orogen + 0.5 * smoothstep(300.0, 1800.0, e) } else { 0.05 * orogen },
                    ResourceType::Gold => 0.9 * orogen * smoothstep(300.0, 1800.0, e) + 0.4 * volcanic,
                    ResourceType::Silver => 0.7 * orogen * if has_granite(x, y) || matches!(rock, RockType::Limestone) { 1.0 } else { 0.4 },
                    ResourceType::Gems => 0.9 * volcanic + 0.4 * smoothstep(0.25, 0.5, s),
                    ResourceType::Coal => if e < 500.0 && m > 0.35 && !matches!(rock, RockType::Basalt | RockType::Granite) { 0.6 * (1.0 - hard) + 0.4 * smoothstep(0.35, 0.7, m) } else { 0.0 },
                    ResourceType::Salt => {
                        let dry = matches!(biome, ExtendedBiome::SaltFlats | ExtendedBiome::Desert);
                        if dry && e < 900.0 { 0.9 } else if m < 0.12 && e < 300.0 { 0.5 } else { 0.0 }
                    }
                    _ => 0.0,
                };
                if suit < 0.12 { continue; }
                let n = (noise.get([x as f64 * 0.09, y as f64 * 0.09 + spec.salt as f64]) as f32 + 1.0) * 0.5;
                let score = suit * (0.35 + 0.65 * n);
                scored.push((score, x, y));
            }
        }
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap().then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        let want = (spec.count * area).round().max(2.0) as usize;
        let mut chosen: Vec<(usize, usize)> = Vec::new();
        for &(score, x, y) in &scored {
            let _ = score;
            if chosen.len() >= want { break; }
            let far = chosen.iter().all(|&(cx, cy)| {
                let dx = (x as i64 - cx as i64).abs();
                let dx = dx.min(w as i64 - dx);
                dx * dx + (y as i64 - cy as i64).pow(2) >= spec.spacing * spec.spacing
            });
            if !far { continue; }
            // Best-ranked sites are rich: top fifth, then the middle half, then poor.
            let rank = chosen.len() as f32 / want as f32;
            let richness = if rank < 0.2 { 3 } else if rank < 0.7 { 2 } else { 1 };
            chosen.push((x, y));
            deposits.push(Deposit { x, y, kind: spec.kind, richness });
        }
    }
    let mut by_tile: HashMap<(usize, usize), Vec<usize>> = HashMap::default();
    for (i, d) in deposits.iter().enumerate() {
        by_tile.entry((d.x, d.y)).or_default().push(i);
    }
    ResourceMap { deposits, by_tile, fertility, timber, fish }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resources_follow_geology_and_are_deterministic() {
        let world = crate::world::generate_world_with_style(160, 80, 21, crate::plates::WorldStyle::Earthlike);
        let a = compute_resources(&world);
        let b = compute_resources(&world);
        assert_eq!(a.deposits.len(), b.deposits.len());
        assert!(a.deposits.iter().zip(b.deposits.iter()).all(|(p, q)| (p.x, p.y, p.kind) == (q.x, q.y, q.kind)));
        assert!(!a.deposits.is_empty(), "a world has ore");
        assert!(a.deposits.iter().all(|d| *world.heightmap.get(d.x, d.y) > 0.0), "deposits are on land");
        assert!(a.fertility.iter().all(|(_, _, &f)| (0.0..=1.0).contains(&f)));
        // Copper and gold sit in tectonically active ground: higher mean stress than iron's
        // background, and well above the planet average.
        let mean = |k: ResourceType| {
            let v: Vec<f32> = a.deposits.iter().filter(|d| d.kind == k).map(|d| *world.stress_map.get(d.x, d.y)).collect();
            v.iter().sum::<f32>() / v.len().max(1) as f32
        };
        let avg: f32 = world.stress_map.iter().map(|(_, _, &s)| s).sum::<f32>() / (160 * 80) as f32;
        assert!(mean(ResourceType::Gold) > avg, "gold {} vs avg {}", mean(ResourceType::Gold), avg);
        assert!(mean(ResourceType::Copper) > avg);
    }
}
