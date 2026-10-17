//! The dev embark (`--dev-embark`, the lab's colonies): the world tile a dev colony is founded
//! on when none is given. Moved out of `main.rs` so the lab founds its colonies where the game does.

use crate::history::world_state::WorldHistory;
use crate::world::WorldData;

/// The dev embark's tile: the best-scored candidate whose embark is habitable.
pub fn pick_dev_embark(world: &WorldData, history: Option<&WorldHistory>) -> (usize, usize) {
    let ranked = dev_embark_candidates(world, history);
    let first = ranked.first().copied().unwrap_or((world.width / 2, world.height / 2));
    // On the audited terrain the best tile whose embark has dry land for a camp (seeds 8, 18 and
    // 23 had put theirs in a river or on the sea at the embark's scale); revision 1 keeps its pick.
    if crate::terrain_rev::v1() { return first; }
    ranked.iter().take(12).copied().find(|&t| { let ok = super::viewer::embark_habitable(world, history, t); if std::env::var("PLANET_DEBUG_EMBARK").is_ok() { eprintln!("embark candidate {:?}: {}", t, ok); } ok }).unwrap_or(first)
}

/// The dev embark's candidate tiles, best first (ties to the lowest index), by `pick_dev_embark`'s rule.
pub fn dev_embark_candidates(world: &WorldData, history: Option<&WorldHistory>) -> Vec<(usize, usize)> {
    let (w, h) = (world.width, world.height);
    let towns: Vec<(usize, usize)> = history.map(|h| h.settlements.values()
        .filter(|s| !s.is_destroyed()).map(|s| s.location).collect()).unwrap_or_default();
    let mut scored: Vec<((usize, usize), f32)> = Vec::new();
    for y in 1..h - 1 {
        for x in 0..w {
            if *world.heightmap.get(x, y) <= 0.0 || world.water_body_map.get(x, y).is_lake() { continue; }
            // A temperate first home: no ice, tundra or scorched desert.
            let t = *world.temperature.get(x, y);
            if !(4.0..=24.0).contains(&t) { continue; }
            let (mut river, mut woods, mut lo, mut hi) = (false, 0, f32::MAX, f32::MIN);
            for dy in -1i64..=1 {
                for dx in -1i64..=1 {
                    let (nx, ny) = ((x as i64 + dx).rem_euclid(w as i64) as usize, (y as i64 + dy) as usize);
                    if world.water_body_map.get(nx, ny).is_river() { river = true; }
                    if format!("{:?}", world.biomes.get(nx, ny)).contains("Forest") { woods += 1; }
                    let e = *world.heightmap.get(nx, ny);
                    lo = lo.min(e);
                    hi = hi.max(e);
                }
            }
            let near = towns.iter().map(|&(tx, ty)| {
                let dx = x.abs_diff(tx).min(w - x.abs_diff(tx));
                dx.max(y.abs_diff(ty))
            }).min().unwrap_or(usize::MAX);
            let score = if river { 3.0 } else { 0.0 }
                + (woods as f32 / 3.0).min(2.0)
                + if hi - lo > 300.0 { 1.0 } else { 0.0 }
                + match near { 1..=3 => 2.0, 0 => -5.0, _ => 0.0 };
            scored.push(((x, y), score));
        }
    }
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then((a.0 .1, a.0 .0).cmp(&(b.0 .1, b.0 .0))));
    scored.into_iter().map(|(t, _)| t).collect()
}
