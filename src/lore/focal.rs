//! The director pass: every sizeable region gets a focal point.
//!
//! Each named region (forest, jungle, desert, plains, tundra) is checked for something worth
//! travelling to: a named peak, a lake, a volcano, a giant-tree grove, an oasis or a crater lake.
//! A region with none gets one planted at its most interior tile, chosen by what it is: an
//! ancient grove in a forest or jungle, an oasis in a desert, a crater lake (an old impact) on
//! plains or tundra. Planted features keep a minimum distance from each other and from existing
//! ones, so rare things stay rare. They are biome patches (saved with the world), so the
//! landmark list, labels and the viewer pick them up like any other.

use std::collections::VecDeque;

use crate::biomes::ExtendedBiome;
use crate::tilemap::Tilemap;
use crate::world::WorldData;

use super::gazetteer::{FeatureKind, Gazetteer};

#[derive(Clone, Debug)]
pub struct Placed {
    pub x: usize,
    pub y: usize,
    pub biome: ExtendedBiome,
    /// The region it was placed in.
    pub region: String,
    pub tiles: usize,
}

/// Biomes that already make a region worth visiting.
fn is_focal(b: ExtendedBiome) -> bool {
    use ExtendedBiome::*;
    matches!(b, AncientGrove | Oasis | CraterLake | HighlandLake | VolcanicCone | Caldera | ShieldVolcano
        | Geysers | HotSprings | Cenote | TowerKarst | Sinkhole | TitanBones | CyclopeanRuins | OvergrownCitadel)
}

/// Plant focal points in regions that lack one. Edits `world.biomes`; returns what was placed.
pub fn place_focal_points(world: &mut WorldData, gaz: &Gazetteer, seed: u64) -> Vec<Placed> {
    let (w, h) = (world.width, world.height);
    let area = (w * h) as f32 / (512.0 * 256.0);
    let min_region = ((60.0 * area).round() as usize).max(12);
    let spacing = ((14.0 * (w as f32 / 512.0)).round() as i64).max(5);

    // Existing focal points: features, volcanoes and focal biomes.
    let mut anchors: Vec<(usize, usize)> = Vec::new();
    for f in &gaz.features {
        if matches!(f.kind, FeatureKind::Peak | FeatureKind::Lake) { anchors.push(f.anchor); }
    }
    anchors.extend(world.volcanoes.iter().map(|v| (v.x, v.y)));
    for (x, y, &b) in world.biomes.iter() {
        if is_focal(b) { anchors.push((x, y)); }
    }
    let far_from = |anchors: &[(usize, usize)], x: usize, y: usize| {
        anchors.iter().all(|&(ax, ay)| {
            let dx = (ax as i64 - x as i64).abs();
            let dx = dx.min(w as i64 - dx);
            let dy = (ay as i64 - y as i64).abs();
            dx.max(dy) >= spacing
        })
    };

    let mut regions: Vec<_> = gaz.features.iter()
        .filter(|f| matches!(f.kind, FeatureKind::Forest | FeatureKind::Jungle | FeatureKind::Desert | FeatureKind::Plains | FeatureKind::Tundra))
        .filter(|f| f.size >= min_region)
        .collect();
    // Largest regions first: they claim the spacing.
    regions.sort_by_key(|f| std::cmp::Reverse(f.size));

    let mut placed = Vec::new();
    for f in regions {
        let tiles: Vec<(usize, usize)> = gaz.region.iter().filter(|(_, _, &id)| id == f.id).map(|(x, y, _)| (x, y)).collect();
        if tiles.is_empty() { continue; }
        // Already has something? A peak or lake anchored inside, a volcano or focal biome within.
        let inside: std::collections::HashSet<(usize, usize)> = tiles.iter().copied().collect();
        if anchors.iter().any(|a| inside.contains(a)) { continue; }

        // The most interior tile: farthest (in steps) from the region's edge.
        let mut dist = Tilemap::new_with(w, h, u32::MAX);
        let mut q = VecDeque::new();
        for &(x, y) in &tiles {
            let edge = world.heightmap.neighbors(x, y).iter().any(|n| !inside.contains(n));
            if edge { dist.set(x, y, 0); q.push_back((x, y)); }
        }
        while let Some((x, y)) = q.pop_front() {
            let d = *dist.get(x, y) + 1;
            for (nx, ny) in world.heightmap.neighbors(x, y) {
                if inside.contains(&(nx, ny)) && *dist.get(nx, ny) > d { dist.set(nx, ny, d); q.push_back((nx, ny)); }
            }
        }
        // Tie-break by a hash so the choice is stable but not always the first row.
        let hash = |x: usize, y: usize| ((x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ seed) % 997;
        let Some(&(cx, cy)) = tiles.iter()
            .filter(|&&(x, y)| *dist.get(x, y) != u32::MAX && *world.heightmap.get(x, y) > 0.0)
            .max_by_key(|&&(x, y)| (*dist.get(x, y), hash(x, y)))
        else { continue };
        if !far_from(&anchors, cx, cy) { continue; }

        let biome = match f.kind {
            FeatureKind::Forest | FeatureKind::Jungle => ExtendedBiome::AncientGrove,
            FeatureKind::Desert => ExtendedBiome::Oasis,
            _ => ExtendedBiome::CraterLake,
        };
        // A small patch around the centre, inside the region: 1 ring for lakes and oases, 2 for
        // groves (the old trees spread).
        let r: i64 = if biome == ExtendedBiome::AncientGrove { 2 } else { 1 };
        let mut n = 0;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy > r * r + 1 { continue; }
                let y = cy as i64 + dy;
                if y < 0 || y >= h as i64 { continue; }
                let x = (cx as i64 + dx).rem_euclid(w as i64) as usize;
                let y = y as usize;
                if !inside.contains(&(x, y)) || *world.heightmap.get(x, y) <= 0.0 { continue; }
                // Crater lakes keep a compact, rounded patch (some corners); oases are the centre
                // and a few edge neighbours, never a square of palms.
                if biome == ExtendedBiome::CraterLake && dx != 0 && dy != 0 && hash(x, y) % 2 == 0 { continue; }
                if biome == ExtendedBiome::Oasis && (dx != 0 || dy != 0) && (dx != 0 && dy != 0 || hash(x, y) % 3 == 0) { continue; }
                world.biomes.set(x, y, biome);
                n += 1;
            }
        }
        if n == 0 { continue; }
        anchors.push((cx, cy));
        placed.push(Placed { x: cx, y: cy, biome, region: f.name.clone(), tiles: n });
    }
    placed
}
