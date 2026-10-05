//! Checks that the simulated past doesn't contradict itself.
//!
//! A reader who finds one contradiction in the annals stops trusting the rest, so these run at the
//! end of every history (printed with the summary) and in tests. Each violation is one line a
//! person can look up in the journal.

use crate::history::*;
use crate::history::events::types::EventType;
use crate::history::world_state::WorldHistory;
use crate::history::det::HashMap;

/// Youngest age at which anyone takes a throne.
pub const MIN_RULER_AGE: u32 = 14;

/// Every contradiction in the history, as readable lines (empty = consistent).
pub fn violations(h: &WorldHistory) -> Vec<String> {
    let mut out = Vec::new();
    let fname = |f: FactionId| h.factions.get(&f).map(|x| x.name.clone()).unwrap_or_else(|| format!("{:?}", f));
    let pname = |p: FigureId| h.figures.get(&p).map(|x| x.full_name()).unwrap_or_else(|| format!("{:?}", p));

    // A people ends once per time it is founded (a fallen people can rise again).
    let mut ended: HashMap<FactionId, i32> = HashMap::default();
    for e in h.chronicle.events.iter() {
        let Some(&f) = e.factions_involved.first() else { continue };
        match e.event_type {
            EventType::FactionDestroyed => *ended.entry(f).or_default() += 1,
            EventType::FactionFounded => *ended.entry(f).or_default() -= 1,
            _ => {}
        }
    }
    for (f, n) in &ended {
        if *n > 0 { out.push(format!("{} is destroyed {} more times than it was founded", fname(*f), n)); }
    }

    // An ended people does nothing afterwards: no crowning, war, conquest or founding.
    for f in h.factions.values() {
        let Some(gone) = f.dissolved else { continue };
        let acts = h.chronicle.events.iter().filter(|e| e.date > gone && e.factions_involved.first() == Some(&f.id))
            .filter(|e| matches!(e.event_type, EventType::RulerCrowned | EventType::WarDeclared | EventType::HolyWarDeclared
                | EventType::SiegeEnded | EventType::SettlementFounded | EventType::SettlementDestroyed | EventType::Coup))
            .count();
        if acts > 0 { out.push(format!("{} acts {} times after it ended in {}", f.name, acts, gone.year)); }
    }

    // The living rule; a crowned ruler is an adult who outlived no living predecessor.
    for f in h.factions.values().filter(|f| f.is_active()) {
        match f.current_leader.and_then(|l| h.figures.get(&l)) {
            Some(l) if !l.is_alive() => out.push(format!("{} is ruled by {}, who died in {}", f.name, l.full_name(),
                l.death_date.map(|d| d.year).unwrap_or(0))),
            _ => {}
        }
    }
    let mut crowned_by_faction: HashMap<FactionId, Vec<(crate::history::time::Date, FigureId)>> = HashMap::default();
    for e in h.chronicle.events.iter().filter(|e| e.event_type == EventType::RulerCrowned) {
        let (Some(&f), Some(EntityId::Figure(p))) = (e.factions_involved.first(), e.primary_participants.first()) else { continue };
        crowned_by_faction.entry(f).or_default().push((e.date, *p));
        if let Some(fig) = h.figures.get(p) {
            if fig.age_at(&e.date) < MIN_RULER_AGE {
                out.push(format!("{} is crowned in {} aged {}", fig.full_name(), e.date.year, fig.age_at(&e.date)));
            }
        }
    }
    for (f, mut reigns) in crowned_by_faction {
        reigns.sort_by_key(|r| r.0);
        for w in reigns.windows(2) {
            let (prev, (when, next)) = (w[0].1, w[1]);
            let alive_then = h.figures.get(&prev).map_or(false, |p| p.death_date.map_or(true, |d| d > when));
            if alive_then {
                out.push(format!("{} crowns {} in {} while {} still lives", fname(f), pname(next), when.year, pname(prev)));
            }
        }
    }

    // A figure dies once.
    let mut deaths: HashMap<FigureId, u32> = HashMap::default();
    for e in h.chronicle.events.iter().filter(|e| matches!(e.event_type, EventType::HeroDied | EventType::Assassination)) {
        if e.event_type == EventType::Assassination && e.title.starts_with("Failed") { continue; }
        if let Some(EntityId::Figure(p)) = e.primary_participants.first() { *deaths.entry(*p).or_default() += 1; }
    }
    for (p, n) in deaths {
        if n > 1 { out.push(format!("{} dies {} times", pname(p), n)); }
    }

    // One war at a time between a pair, and only between living peoples.
    let mut pairs: HashMap<(FactionId, FactionId), u32> = HashMap::default();
    for w in h.wars.values().filter(|w| w.is_active()) {
        for &a in &w.aggressors {
            for &d in &w.defenders {
                *pairs.entry(if a < d { (a, d) } else { (d, a) }).or_default() += 1;
                for x in [a, d] {
                    if !h.factions.get(&x).map_or(false, |f| f.is_active()) {
                        out.push(format!("{} is still at war after it ended ({})", fname(x), w.name));
                    }
                }
            }
        }
    }
    for ((a, b), n) in pairs {
        if n > 1 { out.push(format!("{} and {} fight {} wars at once", fname(a), fname(b), n)); }
    }

    out.sort();
    out.dedup();
    out
}

/// Length of each caused event's chain of causes (links followed back, up to 20): (median, mean).
pub fn chain_depth(h: &WorldHistory) -> (usize, f32) {
    let mut depths: Vec<usize> = h.chronicle.events.iter().filter(|e| !e.causes.is_empty()).map(|e| {
        let (mut d, mut cur) = (0, e);
        while let Some(&c) = cur.causes.first() {
            let Some(ce) = h.chronicle.get(c) else { break };
            d += 1;
            if d >= 20 { break; }
            cur = ce;
        }
        d
    }).collect();
    if depths.is_empty() { return (0, 0.0); }
    depths.sort();
    (depths[depths.len() / 2], depths.iter().sum::<usize>() as f32 / depths.len() as f32)
}

/// How many events of each kind record a cause: (kind, with a cause, total), most uncaused first.
pub fn cause_report(h: &WorldHistory) -> Vec<(String, usize, usize)> {
    let mut by: HashMap<String, (usize, usize)> = HashMap::default();
    for e in &h.chronicle.events {
        let k = by.entry(format!("{:?}", e.event_type)).or_default();
        k.1 += 1;
        if !e.causes.is_empty() { k.0 += 1; }
    }
    let mut v: Vec<(String, usize, usize)> = by.into_iter().map(|(k, (c, t))| (k, c, t)).collect();
    v.sort_by(|a, b| (b.2 - b.1).cmp(&(a.2 - a.1)).then(a.0.cmp(&b.0)));
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::biomes::ExtendedBiome;
    use crate::history::config::HistoryConfig;
    use crate::history::data::GameData;
    use crate::history::simulation::HistoryEngine;
    use crate::plates::types::PlateId;
    use crate::scale::MapScale;
    use crate::seeds::WorldSeeds;
    use crate::tilemap::Tilemap;
    use crate::water_bodies::WaterBodyId;
    use crate::world::WorldData;

    fn world() -> WorldData {
        let (width, height) = (64, 32);
        let mut heightmap = Tilemap::new_with(width, height, 300.0);
        let mut biomes = Tilemap::new_with(width, height, ExtendedBiome::TemperateGrassland);
        for x in 0..width {
            for y in [0, height - 1] {
                *biomes.get_mut(x, y) = ExtendedBiome::Ocean;
                *heightmap.get_mut(x, y) = -100.0;
            }
        }
        WorldData::new(
            WorldSeeds::from_master(7), MapScale::new(20.0), heightmap,
            Tilemap::new_with(width, height, 15.0), Tilemap::new_with(width, height, 0.5),
            biomes, Tilemap::new_with(width, height, 0.0), Tilemap::new_with(width, height, PlateId(0)),
            Vec::new(), None, Tilemap::new_with(width, height, WaterBodyId::NONE), Vec::new(),
            Tilemap::new_with(width, height, 0.0), None, None,
        )
    }

    #[test]
    fn history_does_not_contradict_itself() {
        let world = world();
        let data = GameData::defaults();
        for seed in [3u64, 11, 76] {
            let config = HistoryConfig {
                simulation_years: 150,
                initial_civilizations: 8,
                initial_legendary_creatures: 10,
                ..HistoryConfig::default()
            };
            let h = HistoryEngine::new(seed).simulate_with_data(&world, config, &data);
            let v = violations(&h);
            assert!(v.is_empty(), "seed {seed}: {} contradictions:\n{}", v.len(), v.join("\n"));
        }
    }
}

/// One line on the wars: how long they ran, whether their battles had people in them, and how
/// alike their names are.
pub fn war_report(h: &WorldHistory) -> String {
    let ended: Vec<u32> = h.wars.values().filter_map(|w| w.ended.map(|e| e.year.saturating_sub(w.started.year))).collect();
    let mut d = ended.clone();
    d.sort();
    let q = |p: f32| d.get(((d.len() as f32 - 1.0) * p) as usize).copied().unwrap_or(0);
    let battles: Vec<_> = h.chronicle.events.iter().filter(|e| e.event_type == EventType::BattleFought).collect();
    let with_figure = battles.iter().filter(|e| e.primary_participants.iter().any(|p| matches!(p, EntityId::Figure(_)))).count();
    let names: crate::history::det::HashSet<&str> = h.wars.values().map(|w| w.name.as_str()).collect();
    let mut fell: Vec<(String, usize)> = Vec::new();
    for e in h.chronicle.events.iter().filter(|e| e.event_type == EventType::FactionDestroyed) {
        let why = e.causes.first().and_then(|c| h.chronicle.get(*c)).map(|c| format!("{:?}", c.event_type)).unwrap_or_else(|| e.description.split_whitespace().rev().take(3).collect::<Vec<_>>().join("_"));
        match fell.iter_mut().find(|x| x.0 == why) { Some(x) => x.1 += 1, None => fell.push((why, 1)) }
    }
    fell.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    format!("Wars: {} ({} ended, lasting {}-{} years, median {}, 90% under {}, {} within a year); {} battles, {:.0}% name a figure; {} distinct names; peoples fallen: {}",
        h.wars.len(), ended.len(), q(0.0), q(1.0), q(0.5), q(0.9), d.iter().filter(|x| **x == 0).count(), battles.len(),
        100.0 * with_figure as f32 / battles.len().max(1) as f32, names.len(),
        fell.iter().map(|(k, n)| format!("{} {}", n, k)).collect::<Vec<_>>().join(", "))
}

/// One line on the world's treasures and monuments: how many name a real maker or deed.
pub fn objects_report(h: &WorldHistory) -> String {
    let arts = h.artifacts.values().count();
    let arts_ok = h.artifacts.values().filter(|a| {
        !a.description.is_empty() && (a.creator.map_or(false, |c| h.figures.contains_key(&c)) || !a.inscriptions.is_empty())
    }).count();
    let mons = h.monuments.values().count();
    let mons_ok = h.monuments.values().filter(|m| m.commemorates.map_or(false, |e| h.chronicle.get(e).is_some())).count();
    let mut names: Vec<&str> = h.artifacts.values().map(|a| a.name.as_str()).collect();
    names.sort();
    let dupes = names.windows(2).filter(|w| w[0] == w[1]).count();
    format!("Objects: {} of {} artifacts name a real maker or deed ({} repeated names); {} of {} monuments remember a real event",
        arts_ok, arts, dupes, mons_ok, mons)
}
