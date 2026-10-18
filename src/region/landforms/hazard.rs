//! Snow, ice and fire that kill (ROADMAP Stage 7; only with upheaval):
//!
//! - **Crevasse fields** on a glacier where it bends over a step in its bed or spreads at its
//!   snout: glacier cells steeper than 0.12 (at 610 m cells), or 0.06 at its edge.
//! - **Avalanche paths**: on wooded slopes of 24-50 degrees with a snowy winter (mean under 4 C,
//!   moisture over 0.35) the snow comes down the same chutes every year and the trees never
//!   grow back in them: strips down the slope.
//! - **Burned woods**: in the woods of the fire climates (boreal forest, savanna, the
//!   Mediterranean's dry summers: summer-dry and warm enough to burn, 0.42-0.7 moisture)
//!   lightning sets fires that burn patches of forest; the dead trees stand for decades.

use noise::{NoiseFn, Perlin};

use crate::biomes::ExtendedBiome;
use crate::erosion::materials::RockType;
use crate::region::plan::Landform;

use super::rock::{scale, unit};
use super::RegionFields;

pub fn classify(f: &RegionFields, _rock: &[RockType], land: &mut [Landform]) {
    if f.world.upheaval().is_none() { return; }
    // The chronicle's airbursts (`history::space`): the woods round each point flattened and
    // burned, `AIRBURST_BURN_KM` out (centred on its tile).
    let km = crate::upheaval::tile_km(f.world.width);
    let mut bursts: Vec<(f32, f32)> = Vec::new();
    let (tx0, ty0) = f.tile_of(0, 0);
    let (tw, th) = (f.gw / f.s + 1, f.gh / f.s + 1);
    for dy in 0..th { for dx in 0..tw {
        let (x, y) = ((tx0 + dx) % f.world.width, (ty0 + dy).min(f.world.height - 1));
        for ev in f.world.ground_events_at(x, y).into_iter().filter(|e| e.airburst) {
            bursts.push(((f.gx0 + dx as i64) as f32, y as f32));
            let _ = ev;
        }
    } }
    if !bursts.is_empty() {
        for j in 0..f.gh {
            for i in 0..f.gw {
                let k = f.idx(i, j);
                if f.is_sea(k) || f.lake_depth(k) > 0.0 || f.moist[k] < 0.35 || f.temp[k] < -4.0 { continue; }
                let (u, v) = f.to_world(i, j);
                if bursts.iter().any(|&(bu, bv)| crate::upheaval::distance_km(f.world.width, f.world.height, bu, bv, u, v) < crate::history::space::AIRBURST_BURN_KM)
                    && matches!(land[k], Landform::None | Landform::UValley | Landform::Terrace | Landform::Floodplain) {
                    land[k] = Landform::BurnScar;
                }
            }
        }
    }
    let _ = km;
    let k_scale = scale(f.cell_m);
    let noise = Perlin::new((f.seed ^ 0x4A2D) as u32);
    let src = land.to_vec();
    for j in 0..f.gh {
        for i in 0..f.gw {
            let k = f.idx(i, j);
            if f.is_sea(k) || f.lake_depth(k) > 0.0 { continue; }
            let slope = f.slope(i, j);
            let (ax, ay) = f.abs_cell(i, j);
            let (t, m) = (f.temp[k], f.moist[k]);
            if src[k] == Landform::Glacier {
                let edge = (-1i64..=1).any(|dy| (-1i64..=1).any(|dx| {
                    let (x, y) = (i as i64 + dx, j as i64 + dy);
                    x >= 0 && y >= 0 && (x as usize) < f.gw && (y as usize) < f.gh && src[f.idx(x as usize, y as usize)] != Landform::Glacier
                }));
                if slope > 0.12 * k_scale || (edge && slope > 0.06 * k_scale && unit(ax, ay, 0xC4E) < 0.3) { land[k] = Landform::Crevasses; }
                continue;
            }
            if src[k] != Landform::None && src[k] != Landform::UValley && src[k] != Landform::Cirque { continue; }
            let deg = slope.atan().to_degrees();
            if t < 4.0 && t > -6.0 && m > 0.35 && (24.0..50.0).contains(&deg) {
                // Chutes: narrow strips where a slow noise crosses zero, broken along their length.
                let n = noise.get([ax as f64 / 5.0, ay as f64 / 5.0]) as f32;
                if n.abs() < 0.12 { land[k] = Landform::AvalanchePath; continue; }
            }
            let (tx, ty) = f.tile_of(i, j);
            let biome = *f.world.biomes.get(tx, ty);
            let fire_climate = matches!(biome, ExtendedBiome::BorealForest | ExtendedBiome::Savanna | ExtendedBiome::SubalpineForest)
                || (t > 12.0 && m > 0.42 && m < 0.6);
            if fire_climate && m > 0.42 && m < 0.7 && t > -2.0 && slope < 0.4 {
                let n = noise.get([ax as f64 / 22.0 + 50.0, ay as f64 / 22.0]) as f32;
                if n > 0.42 && unit(tx as i64, ty as i64, 0xF12E) < 0.6 { land[k] = Landform::BurnScar; }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::history::events::types::EventType;
    use crate::history::geology::GroundEvent;
    use crate::region::plan::{Landform, PlaceKind};

    /// An airburst of the chronicle over a wooded tile burns the woods round it in the region and
    /// leaves its place there with its year.
    #[test]
    fn an_airburst_burns_the_woods_round_it() {
        std::env::set_var("PLANET_CHUNK_CACHE", "");
        let world = crate::world::generate_world_with_style(64, 32, 5, crate::plates::WorldStyle::Earthlike);
        world.set_upheaval(Some(crate::upheaval::Upheaval::earth()));
        // A wooded land tile away from the poles.
        let t = (0..64 * 32).map(|k| (k % 64, k / 64))
            .find(|&(x, y)| y > 6 && y < 26 && *world.heightmap.get(x, y) > 50.0 && *world.moisture.get(x, y) > 0.5 && *world.temperature.get(x, y) > 5.0)
            .expect("a wooded tile");
        world.set_ground_events(vec![GroundEvent { tile: t, year: 300, kind: EventType::Skyfall, title: "The sky bursts in the empty country".into(), id: crate::history::EventId(1), crater_m: 0.0, airburst: true }]);
        let params = crate::region::zoom::ZoomParams { center_x: t.0, center_y: t.1, tiles: 2, cells_per_tile: 32, erosion_iterations: 8, seed: 5 };
        let z = crate::region::zoom::generate_zoom(&world, &params);
        let burned = z.landform.iter().filter(|&&l| l == Landform::BurnScar).count();
        assert!(burned > 0, "no woods burned round the airburst");
        assert!(z.places.iter().any(|p| p.kind == PlaceKind::Skyburn && p.cause.contains("in the year 300")), "no place for the airburst");
    }

    /// A stone that fell in the chronicle leaves its crater in the region, with its year and its
    /// size for the embark to carve.
    #[test]
    fn a_fallen_stone_leaves_its_crater() {
        std::env::set_var("PLANET_CHUNK_CACHE", "");
        let world = crate::world::generate_world_with_style(64, 32, 5, crate::plates::WorldStyle::Earthlike);
        world.set_upheaval(Some(crate::upheaval::Upheaval::earth()));
        let t = (0..64 * 32).map(|k| (k % 64, k / 64)).find(|&(x, y)| y > 6 && y < 26 && *world.heightmap.get(x, y) > 50.0).expect("land");
        world.set_ground_events(vec![GroundEvent { tile: t, year: 212, kind: EventType::Skyfall, title: "A stone falls in the empty country".into(), id: crate::history::EventId(2), crater_m: 300.0, airburst: false }]);
        let params = crate::region::zoom::ZoomParams { center_x: t.0, center_y: t.1, tiles: 2, cells_per_tile: 32, erosion_iterations: 8, seed: 5 };
        let z = crate::region::zoom::generate_zoom(&world, &params);
        let p = z.places.iter().find(|p| p.kind == PlaceKind::MeteorCrater && p.cause.contains("in the year 212")).expect("the stone's crater");
        assert!((p.radius_m - 150.0).abs() < 1.0, "radius {}", p.radius_m);
    }
}
