//! Rare things worth showing off: world-defining outcomes, each with its cause and an honest rate.
//!
//! Histories look alike when every war lasts five years. These are the outcomes a player would
//! mention first ("my world has a king who has reigned since before history"), detected in the
//! finished history, each with the share of dev worlds that have it (measured over seeds 1-200
//! with `scripts/rare_rates.sh`; re-measure when history changes). The present-day report names
//! the ones this world has, with their rate.

use crate::history::*;
use crate::history::det::{HashMap, HashSet};
use crate::history::events::types::EventType;
use crate::history::world_state::WorldHistory;
use crate::world::WorldData;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Rare {
    /// A ruler crowned 50 years or more before written history began who still reigns.
    UndyingKing,
    /// A beast alive since the dawn that raided 15 times or more and was never slain.
    NeverSlain,
    /// A fallen people whose notables live on in exile.
    PeopleInExile,
    /// A realm holding five towns or more on each of two landmasses.
    EmpireAcrossTheSea,
    /// The one founding capital in the world never besieged or taken, though its people fought
    /// five wars or more.
    UnbrokenSeat,
    /// A people that fell, rose again and is now the greatest realm.
    RisenAgain,
    /// Two neighbouring peoples who never fought.
    UnbrokenPeace,
}

impl Rare {
    pub const ALL: [Rare; 7] = [Rare::UndyingKing, Rare::NeverSlain, Rare::PeopleInExile, Rare::EmpireAcrossTheSea, Rare::UnbrokenSeat, Rare::RisenAgain, Rare::UnbrokenPeace];

    /// Worlds in 100 dev worlds that have it (mean of seeds 1-100 and 101-200, measured 2026-10-05).
    pub fn per_hundred(self) -> u32 {
        match self {
            Rare::UndyingKing => RATES[0],
            Rare::NeverSlain => RATES[1],
            Rare::PeopleInExile => RATES[2],
            Rare::EmpireAcrossTheSea => RATES[3],
            Rare::UnbrokenSeat => RATES[4],
            Rare::RisenAgain => RATES[5],
            Rare::UnbrokenPeace => RATES[6],
        }
    }

    /// "1 world in 20".
    pub fn rate(self) -> String {
        let p = self.per_hundred().max(1);
        if p >= 50 { format!("{} worlds in 100", p) } else { format!("1 world in {}", (100.0 / p as f32).round() as u32) }
    }

    pub fn label(self) -> &'static str {
        match self {
            Rare::UndyingKing => "The Undying King",
            Rare::NeverSlain => "Never slain",
            Rare::PeopleInExile => "A people in exile",
            Rare::EmpireAcrossTheSea => "An empire across the sea",
            Rare::UnbrokenSeat => "The unbroken seat",
            Rare::RisenAgain => "Risen again",
            Rare::UnbrokenPeace => "Unbroken peace",
        }
    }
}

/// Measured rates per hundred dev worlds, in `Rare::ALL` order.
const RATES: [u32; 7] = [10, 24, 31, 21, 5, 6, 25];

/// The rare outcomes this history has, each with a line saying who and why.
pub fn find(world: &WorldData, h: &WorldHistory) -> Vec<(Rare, String)> {
    let mut out = Vec::new();
    let start = h.config.prehistory_depth + 1;
    let fname = |f: FactionId| h.factions.get(&f).map(|x| x.name.clone()).unwrap_or_default();

    // The Undying King.
    let mut crowned: HashMap<FigureId, (u32, Option<FactionId>)> = HashMap::default();
    for e in h.chronicle.events.iter().filter(|e| e.event_type == EventType::RulerCrowned) {
        for p in &e.primary_participants { if let EntityId::Figure(f) = p { crowned.entry(*f).or_insert((e.date.year, e.factions_involved.first().copied())); } }
    }
    let mut kings: Vec<(&FigureId, &(u32, Option<FactionId>))> = crowned.iter().filter(|(f, (y, fac))| {
        *y + 50 < start && h.figures.get(f).map_or(false, |x| x.is_alive())
            && fac.and_then(|fa| h.factions.get(&fa)).map_or(false, |fa| fa.is_active() && fa.current_leader == Some(**f))
    }).collect();
    kings.sort_by_key(|k| (k.1 .0, *k.0));
    if let Some((f, (y, fac))) = kings.first() {
        out.push((Rare::UndyingKing, format!("{} has ruled {} since the year {}, before written history", h.figures[f].full_name(), fac.map(fname).unwrap_or_default(), y)));
    }

    // Never slain.
    let mut raids: HashMap<LegendaryCreatureId, u32> = HashMap::default();
    for e in h.chronicle.events.iter().filter(|e| e.event_type == EventType::MonsterRaid) {
        for p in &e.primary_participants { if let EntityId::LegendaryCreature(c) = p { *raids.entry(*c).or_default() += 1; } }
    }
    if let Some((c, n)) = raids.iter().filter(|(c, n)| **n >= 15 && h.legendary_creatures.get(c).map_or(false, |b| b.is_alive() && b.birth_date.map_or(true, |d| d.year < start)))
        .max_by_key(|(c, n)| (**n, std::cmp::Reverse(**c))) {
        out.push((Rare::NeverSlain, format!("{} has raided {} times since the dawn of history, and no one has ever slain it", h.legendary_creatures[c].full_name(), n)));
    }

    // A people in exile.
    if let Some(p) = h.people.as_ref() {
        let mut exiles: HashMap<FactionId, Vec<FigureId>> = HashMap::default();
        for (f, r) in &p.role {
            if *r != crate::history::people::Role::Exile { continue; }
            let Some(fig) = h.figures.get(f).filter(|x| x.is_alive()) else { continue };
            if let Some(fac) = fig.faction.filter(|fa| h.factions.get(fa).map_or(false, |x| !x.is_active())) { exiles.entry(fac).or_default().push(*f); }
        }
        let mut v: Vec<_> = exiles.into_iter().collect();
        v.sort_by_key(|(f, list)| (std::cmp::Reverse(list.len()), *f));
        let fell_long_ago = |f: FactionId| h.factions.get(&f).and_then(|x| x.dissolved).map_or(false, |d| d.year + 40 <= h.current_date.year);
        v.retain(|(f, list)| list.len() >= 4 && fell_long_ago(*f));
        if let Some((fac, list)) = v.first() {
            out.push((Rare::PeopleInExile, format!("{} lost its last town in {}, but {} of its people of note live on in exile", fname(*fac), h.factions.get(fac).and_then(|x| x.dissolved).map_or(0, |d| d.year), list.len())));
        }
    }

    // An empire across the sea: towns of one realm on two landmasses.
    let land = landmasses(world);
    let mut realms: Vec<(FactionId, usize)> = h.factions.values().filter(|f| f.is_active()).filter_map(|f| {
        let mut per: HashMap<u32, usize> = HashMap::default();
        for t in f.settlements.iter().filter_map(|s| h.settlements.get(s)).filter(|t| !t.is_destroyed()) {
            if let Some(&m) = land.get(t.location.1 * world.width + t.location.0).filter(|&&m| m != u32::MAX) { *per.entry(m).or_default() += 1; }
        }
        let strong = per.values().filter(|&&n| n >= 5).count();
        (strong >= 2).then(|| (f.id, strong))
    }).collect();
    realms.sort_by_key(|(f, n)| (std::cmp::Reverse(*n), *f));
    if let Some((f, n)) = realms.first() {
        out.push((Rare::EmpireAcrossTheSea, format!("{} holds towns on {} lands across the sea", fname(*f), n)));
    }

    // The unbroken seat: a capital founded before written history, never besieged or taken.
    let touched: HashSet<SettlementId> = h.chronicle.events.iter()
        .filter(|e| matches!(e.event_type, EventType::SiegeBegun | EventType::SiegeEnded | EventType::ShadowConquest | EventType::ShadowRepelled))
        .flat_map(|e| e.primary_participants.iter().filter_map(|p| if let EntityId::Settlement(s) = p { Some(*s) } else { None }))
        .collect();
    let mut seats: Vec<&crate::history::civilizations::settlement::Settlement> = h.settlements.values()
        .filter(|t| !t.is_destroyed() && t.founded.year < start && !touched.contains(&t.id))
        .filter(|t| h.factions.get(&t.faction).map_or(false, |f| f.capital == Some(t.id) && f.founded.year < start))
        .filter(|t| h.wars.values().filter(|w| w.aggressors.contains(&t.faction) || w.defenders.contains(&t.faction)).count() >= 5)
        .collect();
    seats.sort_by_key(|t| (t.founded.year, t.id));
    // Only when it is the one founding seat in the world never besieged.
    if seats.len() != 1 { seats.clear(); }
    if let Some(t) = seats.first() {
        out.push((Rare::UnbrokenSeat, format!("{}, seat of {} since {}, has never been besieged or taken through {} wars", t.name, fname(t.faction), t.founded.year,
            h.wars.values().filter(|w| w.aggressors.contains(&t.faction) || w.defenders.contains(&t.faction)).count())));
    }

    // Risen again: a people destroyed that stands again today.
    let mut fell: Vec<FactionId> = h.chronicle.events.iter().filter(|e| e.event_type == EventType::FactionDestroyed)
        .filter_map(|e| e.factions_involved.first().copied())
        .filter(|f| h.factions.get(f).map_or(false, |x| x.is_active())).collect();
    fell.sort();
    fell.dedup();
    let greatest = h.factions.values().filter(|f| f.is_active()).max_by_key(|f| (f.total_population, std::cmp::Reverse(f.id))).map(|f| f.id);
    if let Some(f) = fell.iter().find(|f| Some(**f) == greatest) {
        out.push((Rare::RisenAgain, format!("{} fell, rose again, and is now the greatest realm in the world", fname(*f))));
    }

    // Unbroken peace: two neighbouring peoples, both from the dawn, never at war.
    let at_war: HashSet<(FactionId, FactionId)> = h.wars.values().flat_map(|w| {
        let mut v = Vec::new();
        for a in &w.aggressors { for d in &w.defenders { v.push((*a.min(d), *a.max(d))); } }
        v
    }).collect();
    let mut old: Vec<&crate::history::civilizations::faction::Faction> = h.factions.values().filter(|f| f.is_active() && f.founded.year < start).collect();
    old.sort_by_key(|f| f.id);
    let seat = |f: &crate::history::civilizations::faction::Faction| f.capital.and_then(|c| h.settlements.get(&c)).map(|t| t.location);
    'peace: for (i, a) in old.iter().enumerate() {
        for b in &old[i + 1..] {
            let (Some(sa), Some(sb)) = (seat(a), seat(b)) else { continue };
            let d = sa.0.abs_diff(sb.0).min(world.width - sa.0.abs_diff(sb.0)) + sa.1.abs_diff(sb.1);
            if d > world.width / 6 { continue; }
            if !at_war.contains(&(a.id.min(b.id), a.id.max(b.id))) {
                out.push((Rare::UnbrokenPeace, format!("{} and {} have lived side by side since before history and never once gone to war", a.name, b.name)));
                break 'peace;
            }
        }
    }
    out
}

/// Each land cell's landmass (connected land, 8-way, wrapping east-west); u32::MAX at sea.
fn landmasses(world: &WorldData) -> Vec<u32> {
    let (w, h) = (world.width, world.height);
    let mut id = vec![u32::MAX; w * h];
    let mut next = 0;
    for start in 0..w * h {
        if id[start] != u32::MAX || *world.heightmap.get(start % w, start / w) < 0.0 { continue; }
        let mut stack = vec![start];
        id[start] = next;
        while let Some(i) = stack.pop() {
            let (x, y) = (i % w, i / w);
            for dy in -1i64..=1 { for dx in -1i64..=1 {
                let ny = y as i64 + dy;
                if ny < 0 || ny >= h as i64 { continue; }
                let nx = (x as i64 + dx).rem_euclid(w as i64) as usize;
                let j = ny as usize * w + nx;
                if id[j] == u32::MAX && *world.heightmap.get(nx, ny as usize) >= 0.0 { id[j] = next; stack.push(j); }
            } }
        }
        next += 1;
    }
    id
}

/// One line for the end-of-history report: which rare outcomes this world has.
pub fn report(found: &[(Rare, String)]) -> String {
    let kinds: Vec<String> = found.iter().map(|(r, _)| format!("{:?}", r)).collect();
    format!("Rare: {} ({})", found.len(), kinds.join(", "))
}
