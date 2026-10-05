//! Landmarks: the world's extremes and wonders, picked out of the gazetteer and the terrain.
//!
//! The highest peak (and each continent's own summit), the longest river, the largest and the
//! deepest lake, the greatest waterfall, the deepest river gorge, crater lakes and groves of giant
//! trees (up to 6 each, including those the director pass planted, `lore::focal`), and the
//! largest desert and forest. Each carries an epithet ("the roof of the world")
//! and a measurement, and is tied to its gazetteer feature where it has one, so the tile viewer
//! can label and describe it and the journal can open with it.

use crate::biomes::ExtendedBiome;
use crate::world::WorldData;

use super::gazetteer::{FeatureKind, Gazetteer};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LandmarkKind {
    HighestPeak,
    ContinentSummit,
    LongestRiver,
    LargestLake,
    DeepestLake,
    GreatestWaterfall,
    DeepestGorge,
    CraterLake,
    GiantTrees,
    LargestDesert,
    LargestForest,
}

#[derive(Clone, Debug)]
pub struct Landmark {
    pub kind: LandmarkKind,
    /// The tile it is marked at.
    pub x: usize,
    pub y: usize,
    /// Gazetteer feature (peak, river, lake, region...) it belongs to, if any.
    pub feature: Option<u32>,
    pub name: String,
    /// What makes it a landmark: "the roof of the world", "the longest river in the world".
    pub epithet: String,
    /// The measurement: "6,212 m", "2,900 km".
    pub detail: String,
}

impl Landmark {
    /// "Mount Grandcairn, the roof of the world (6,212 m)".
    pub fn line(&self) -> String {
        format!("{}, {} ({})", self.name, self.epithet, self.detail)
    }
}

/// A place name as it reads after "of": "the Southford River" stays, "River Thalanor" becomes
/// "the River Thalanor", "Mount Grandcairn" stays.
fn with_article(name: &str) -> String {
    if name.starts_with("the ") || !name.starts_with("River ") { name.to_string() } else { format!("the {name}") }
}

fn thousands(v: f32) -> String {
    let n = v.round() as i64;
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 { out.push(','); }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

/// Find the world's landmarks. Distances use the planet's real tile size (~78 km at 512 wide).
pub fn find_landmarks(world: &WorldData, gaz: &Gazetteer) -> Vec<Landmark> {
    let (w, h) = (world.width, world.height);
    let km = crate::erosion::landscape::tile_km(w);
    let hm = &world.heightmap;
    let mut out = Vec::new();

    // Peaks: the world's highest, and each continent's own summit where it differs.
    let peaks: Vec<_> = gaz.features.iter().filter(|f| f.kind == FeatureKind::Peak).collect();
    let top = peaks.iter().max_by(|a, b| a.height_m.partial_cmp(&b.height_m).unwrap()).copied();
    if let Some(p) = top {
        out.push(Landmark {
            kind: LandmarkKind::HighestPeak, x: p.anchor.0, y: p.anchor.1, feature: Some(p.id),
            name: p.name.clone(), epithet: "the roof of the world".into(), detail: format!("{} m", thousands(p.height_m)),
        });
    }
    for c in gaz.features.iter().filter(|f| f.kind == FeatureKind::Continent) {
        let summit = peaks.iter()
            .filter(|p| *gaz.landmass.get(p.anchor.0, p.anchor.1) == c.id)
            .max_by(|a, b| a.height_m.partial_cmp(&b.height_m).unwrap());
        if let Some(p) = summit {
            if Some(p.id) == top.map(|t| t.id) { continue; }
            out.push(Landmark {
                kind: LandmarkKind::ContinentSummit, x: p.anchor.0, y: p.anchor.1, feature: Some(p.id),
                name: p.name.clone(), epithet: format!("the summit of {}", c.name), detail: format!("{} m", thousands(p.height_m)),
            });
        }
    }

    // The longest river: its main stem, measured along the course.
    let length_km = |path: &[(usize, usize)]| -> f32 {
        path.windows(2).map(|s| {
            let dx = (s[0].0 as i64 - s[1].0 as i64).abs();
            let dx = dx.min(w as i64 - dx);
            let dy = (s[0].1 as i64 - s[1].1 as i64).abs();
            if dx != 0 && dy != 0 { std::f32::consts::SQRT_2 } else { 1.0 }
        }).sum::<f32>() * km
    };
    let rivers: Vec<_> = gaz.features.iter().filter(|f| f.kind == FeatureKind::River && f.path.len() > 1).collect();
    if let Some(r) = rivers.iter().max_by(|a, b| length_km(&a.path).partial_cmp(&length_km(&b.path)).unwrap()) {
        out.push(Landmark {
            kind: LandmarkKind::LongestRiver, x: r.anchor.0, y: r.anchor.1, feature: Some(r.id),
            name: r.name.clone(), epithet: "the longest river in the world".into(), detail: format!("{} km", thousands(length_km(&r.path))),
        });
    }

    // Lakes: the largest (area) and the deepest (from the water bodies).
    let lakes: Vec<_> = gaz.features.iter().filter(|f| f.kind == FeatureKind::Lake).collect();
    if let Some(l) = lakes.iter().max_by_key(|l| l.size) {
        out.push(Landmark {
            kind: LandmarkKind::LargestLake, x: l.anchor.0, y: l.anchor.1, feature: Some(l.id),
            name: l.name.clone(), epithet: "the largest lake in the world".into(),
            detail: format!("{} km\u{b2}", thousands(l.size as f32 * km * km)),
        });
    }
    let deepest = world.water_bodies.iter()
        .filter(|b| b.id.is_lake() && b.max_depth > 0.0)
        .filter_map(|b| {
            // A tile of the body that the gazetteer named.
            let (x0, y0, x1, y1) = b.bounds;
            (y0..=y1.min(h - 1)).flat_map(|y| (x0..=x1.min(w - 1)).map(move |x| (x, y)))
                .find(|&(x, y)| *world.water_body_map.get(x, y) == b.id && gaz.feature(*gaz.water.get(x, y)).map(|f| f.kind == FeatureKind::Lake).unwrap_or(false))
                .map(|t| (b.max_depth, t))
        })
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    if let Some((depth, (x, y))) = deepest {
        let f = gaz.feature(*gaz.water.get(x, y)).unwrap();
        out.push(Landmark {
            kind: LandmarkKind::DeepestLake, x: f.anchor.0, y: f.anchor.1, feature: Some(f.id),
            name: f.name.clone(), epithet: "the deepest lake in the world".into(), detail: format!("{} m deep", thousands(depth)),
        });
    }

    // The greatest waterfall: the highest drop on a named river (the waterfall detector also
    // marks drops on streams too small to be drawn, which would put falls in a dry plain).
    if let Some(uw) = &world.underground_water {
        let on_river = |x: usize, y: usize| gaz.feature(*gaz.water.get(x, y)).filter(|r| r.kind == FeatureKind::River);
        let best = uw.waterfalls.iter()
            .filter(|(x, y, f)| f.is_present && on_river(*x, *y).is_some())
            .max_by(|a, b| a.2.drop_height.partial_cmp(&b.2.drop_height).unwrap());
        if let Some((x, y, f)) = best {
            let r = on_river(x, y).unwrap();
            out.push(Landmark {
                kind: LandmarkKind::GreatestWaterfall, x, y, feature: None,
                name: format!("the Falls of {}", with_article(&r.name)),
                epithet: "the greatest waterfall in the world".into(), detail: format!("a drop of {} m", thousands(f.drop_height)),
            });
        }
    }

    // The deepest gorge: the river tile with high ground on both banks. For each axis through
    // the tile, the lower of the two opposite neighbours is the canyon's rim on that line; the
    // best line gives the depth (one-sided relief, a river beside a range, doesn't count).
    let mut gorge: Option<(f32, usize, usize, u32)> = None;
    for r in &rivers {
        for &(x, y) in &r.path {
            let e = *hm.get(x, y);
            if e <= 0.0 || y == 0 || y + 1 >= h { continue; }
            let at = |dx: i64, dy: i64| *hm.get((x as i64 + dx).rem_euclid(w as i64) as usize, (y as i64 + dy) as usize);
            let depth = [(1, 0), (0, 1), (1, 1), (1, -1)]
                .iter()
                .map(|&(dx, dy)| at(dx, dy).min(at(-dx, -dy)) - e)
                .fold(f32::MIN, f32::max);
            if gorge.map(|g| depth > g.0).unwrap_or(true) { gorge = Some((depth, x, y, r.id)); }
        }
    }
    if let Some((depth, x, y, rid)) = gorge.filter(|g| g.0 > 300.0) {
        let r = gaz.feature(rid).unwrap();
        out.push(Landmark {
            kind: LandmarkKind::DeepestGorge, x, y, feature: None,
            name: format!("the Gorge of {}", with_article(&r.name)),
            epithet: "the deepest gorge in the world".into(), detail: format!("{} m deep", thousands(depth)),
        });
    }

    // Crater lakes and groves of giant trees (biome patches), largest first, a few of each.
    let patches = |biome: ExtendedBiome| -> Vec<Vec<(usize, usize)>> {
        let mut seen = vec![false; w * h];
        let mut comps = Vec::new();
        for s in 0..w * h {
            if seen[s] || *world.biomes.get(s % w, s / w) != biome { continue; }
            let mut comp = vec![(s % w, s / w)];
            seen[s] = true;
            let mut k = 0;
            while k < comp.len() {
                let (x, y) = comp[k];
                for (nx, ny) in hm.neighbors_8(x, y) {
                    let j = ny * w + nx;
                    if !seen[j] && *world.biomes.get(nx, ny) == biome { seen[j] = true; comp.push((nx, ny)); }
                }
                k += 1;
            }
            comps.push(comp);
        }
        comps.sort_by_key(|c| std::cmp::Reverse(c.len()));
        comps
    };
    let near = |x: usize, y: usize| -> String {
        let d = gaz.describe(x, y);
        d.split(", ").next().map(|s| s.to_string()).filter(|s| !s.is_empty()).unwrap_or_else(|| "the wilds".into())
    };
    // Crater lakes and groves are small things on large tiles: their size comes from their kind
    // (a crater a few km across, a grove some hectares), and each has a name of its own (naming
    // them after the region gave three "crater lakes of Neaslind").
    let mut taken: Vec<String> = Vec::new();
    let mut own_name = |x: usize, y: usize, salt: u64, form: &dyn Fn(&str) -> String| -> String {
        use rand::SeedableRng;
        let style = crate::history::naming::styles::NamingStyle::from_archetype(
            crate::history::NamingStyleId(0), crate::history::naming::styles::NamingArchetype::Flowing);
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(world.seed() ^ salt ^ ((x as u64) << 32) ^ y as u64);
        for _ in 0..20 {
            let n = form(&crate::history::naming::generator::NameGenerator::place_name(&style, &mut rng));
            if !taken.contains(&n) { taken.push(n.clone()); return n; }
        }
        format!("{} {}", form("Nameless"), taken.len())
    };
    let unit = |x: usize, y: usize, salt: u64| -> f32 {
        let mut h = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ salt ^ world.seed();
        h ^= h >> 31; h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9); h ^= h >> 29;
        (h % 10_000) as f32 / 10_000.0
    };
    for comp in patches(ExtendedBiome::CraterLake).into_iter().take(6) {
        let (x, y) = comp[comp.len() / 2];
        let across = 2.0 + 10.0 * unit(x, y, 0xC4A7);
        out.push(Landmark {
            kind: LandmarkKind::CraterLake, x, y, feature: None,
            name: own_name(x, y, 0xC4A7, &|n| format!("Lake {}", n)),
            epithet: format!("a lake in an ancient crater, in {}", with_article(&near(x, y))),
            detail: format!("{:.1} km across", across),
        });
    }
    for comp in patches(ExtendedBiome::AncientGrove).into_iter().take(6) {
        let (x, y) = comp[comp.len() / 2];
        let hectares = 20.0 + 380.0 * unit(x, y, 0x6E0E).powi(2);
        out.push(Landmark {
            kind: LandmarkKind::GiantTrees, x, y, feature: None,
            name: own_name(x, y, 0x6E0E, &|n| format!("the {} Wood", n)),
            epithet: format!("a grove of trees older than any people, in {}", with_article(&near(x, y))),
            detail: format!("{} hectares", thousands(hectares.round())),
        });
    }

    // The largest desert and forest.
    for (kinds, kind, epithet) in [
        (&[FeatureKind::Desert][..], LandmarkKind::LargestDesert, "the largest desert in the world"),
        (&[FeatureKind::Forest, FeatureKind::Jungle][..], LandmarkKind::LargestForest, "the largest forest in the world"),
    ] {
        if let Some(f) = gaz.features.iter().filter(|f| kinds.contains(&f.kind)).max_by_key(|f| f.size) {
            out.push(Landmark {
                kind, x: f.anchor.0, y: f.anchor.1, feature: Some(f.id),
                name: f.name.clone(), epithet: epithet.into(), detail: format!("{} km\u{b2}", thousands(f.size as f32 * km * km)),
            });
        }
    }
    out
}

/// Landmark descriptions for a tile: those marked at it, or whose feature covers it.
pub fn describe_at(landmarks: &[Landmark], gaz: &Gazetteer, x: usize, y: usize) -> Vec<String> {
    let layers = [*gaz.water.get(x, y), *gaz.relief.get(x, y), *gaz.region.get(x, y)];
    landmarks.iter()
        .filter(|l| (l.x, l.y) == (x, y) || l.feature.map(|f| layers.contains(&f) || (l.x, l.y) == (x, y)).unwrap_or(false))
        .map(|l| format!("{} ({})", l.epithet, l.detail))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::thousands;

    #[test]
    fn thousands_groups_digits() {
        assert_eq!(thousands(6212.4), "6,212");
        assert_eq!(thousands(950.0), "950");
        assert_eq!(thousands(1234567.0), "1,234,567");
    }

    #[test]
    fn names_read_after_of() {
        use super::with_article;
        assert_eq!(with_article("the Southford River"), "the Southford River");
        assert_eq!(with_article("River Thalanor"), "the River Thalanor");
        assert_eq!(with_article("Mount Grandcairn"), "Mount Grandcairn");
    }
}
