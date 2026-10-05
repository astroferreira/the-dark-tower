//! The present day: the world as the game inherits it at the end of history.
//!
//! History is judged by the state it hands to the game, not by how much happened along the way:
//! a colony founded into a world with no wars, no grudges and no beasts has nothing to react to.
//! `PresentDay::of` reads the open threads off the finished history; `--present` prints them and
//! the end-of-history report prints the counts.

use crate::history::*;
use crate::history::events::types::EventType;
use crate::history::world_state::WorldHistory;

/// How dark the Shadow's corruption must be over a free town for it to stand on the frontier.
pub const FRONTIER_CORRUPTION: f32 = 0.2;
/// How far back (years) a fallen town, a quarrel or a dispute still counts as current.
pub const RECENT_YEARS: u32 = 25;
/// Opinion at or below which two peoples who are not at war bear a grudge.
pub const GRUDGE_OPINION: i32 = -40;
/// Distance (tiles) within which a beast's lair threatens a town.
pub const BEAST_REACH: usize = 12;

#[derive(Clone, Debug)]
pub struct WarNow { pub name: String, pub sides: (String, String), pub since: u32 }

#[derive(Clone, Debug)]
pub struct SiegeNow { pub town: String, pub attacker: String, pub defender: String, pub since: u32 }

/// Two peoples at peace who hate each other, and why (the war or quarrel behind it).
#[derive(Clone, Debug)]
pub struct Grudge { pub a: String, pub b: String, pub opinion: i32, pub because: Option<String> }

/// A living beast and the town nearest its lair.
#[derive(Clone, Debug)]
pub struct BeastNow { pub name: String, pub lair: (usize, usize), pub town: Option<(String, usize)>, pub raids: usize }

#[derive(Clone, Debug)]
pub struct FrontierTown { pub town: String, pub people: String, pub darkness: f32 }

#[derive(Clone, Debug)]
pub struct Dispute { pub resource: String, pub a: String, pub b: String, pub times: usize }

#[derive(Clone, Debug)]
pub struct FallenTown { pub town: String, pub year: u32, pub how: String, pub at: (usize, usize) }

#[derive(Clone, Debug, Default)]
pub struct PresentDay {
    pub year: u32,
    pub peoples: usize,
    pub wars: Vec<WarNow>,
    pub sieges: Vec<SiegeNow>,
    pub grudges: Vec<Grudge>,
    pub beasts: Vec<BeastNow>,
    pub frontier: Vec<FrontierTown>,
    pub disputes: Vec<Dispute>,
    /// Peoples whose ruler has no living child.
    pub heirless: Vec<String>,
    /// Towns taken or burned in the last `RECENT_YEARS`: their survivors are the displaced.
    pub fallen: Vec<FallenTown>,
}

impl PresentDay {
    pub fn of(h: &WorldHistory) -> Self {
        let year = h.current_date.year;
        let fname = |f: FactionId| h.factions.get(&f).map(|x| x.name.clone()).unwrap_or_default();
        let active = |f: &FactionId| h.factions.get(f).map_or(false, |x| x.is_active());
        let recent = |y: u32| y + RECENT_YEARS >= year;
        let mut p = PresentDay { year, peoples: h.factions.values().filter(|f| f.is_active()).count(), ..Default::default() };

        let mut wars: Vec<_> = h.wars.values().filter(|w| w.is_active()).collect();
        wars.sort_by_key(|w| (w.started, w.id));
        for w in wars {
            let side = |v: &Vec<FactionId>| v.iter().filter(|f| active(f)).map(|&f| fname(f)).collect::<Vec<_>>().join(", ");
            p.wars.push(WarNow { name: w.name.clone(), sides: (side(&w.aggressors), side(&w.defenders)), since: w.started.year });
        }

        let mut sieges: Vec<_> = h.sieges.values().filter(|s| s.is_active()).collect();
        sieges.sort_by_key(|s| (s.started, s.id));
        for s in sieges {
            p.sieges.push(SiegeNow {
                town: h.settlements.get(&s.target).map(|t| t.name.clone()).unwrap_or_default(),
                attacker: fname(s.attacker), defender: fname(s.defender), since: s.started.year,
            });
        }

        // Grudges: the lower of the two opinions, between living peoples at peace.
        let mut ids: Vec<FactionId> = h.factions.values().filter(|f| f.is_active()).map(|f| f.id).collect();
        ids.sort();
        let shadow_people = h.shadow.as_ref().filter(|s| !s.is_broken()).map(|s| s.faction);
        for (i, &a) in ids.iter().enumerate() {
            for &b in &ids[i + 1..] {
                let rel = |x: FactionId, y: FactionId| h.factions.get(&x).and_then(|f| f.relations.get(&y));
                let (Some(ra), Some(rb)) = (rel(a, b), rel(b, a)) else { continue };
                if ra.stance.is_at_war() || rb.stance.is_at_war() { continue; }
                let opinion = ra.opinion.min(rb.opinion);
                if opinion > GRUDGE_OPINION { continue; }
                let because = ra.last_war.or(rb.last_war).and_then(|w| h.wars.get(&w))
                    .map(|w| format!("{} ({}-{})", w.name, w.started.year, w.ended.map(|d| d.year).unwrap_or(year)))
                    .or_else(|| last_quarrel(h, a, b));
                p.grudges.push(Grudge { a: fname(a), b: fname(b), opinion, because });
            }
        }
        p.grudges.sort_by_key(|g| g.opinion);

        let towns: Vec<_> = h.settlements.values().filter(|s| !s.is_destroyed()).collect();
        let mut beasts: Vec<_> = h.legendary_creatures.values().filter(|c| c.is_alive()).collect();
        beasts.sort_by_key(|c| c.id);
        for c in beasts {
            let Some(lair) = c.lair_location else { continue };
            let town = towns.iter()
                .map(|t| (t, dist(lair, t.location, h.tile_history.width)))
                .filter(|(_, d)| *d <= BEAST_REACH)
                .min_by_key(|(t, d)| (*d, t.id))
                .map(|(t, d)| (t.name.clone(), d));
            let raids = h.chronicle.events.iter()
                .filter(|e| e.event_type == EventType::MonsterRaid && e.primary_participants.contains(&EntityId::LegendaryCreature(c.id)))
                .count();
            p.beasts.push(BeastNow { name: c.full_name(), lair, town, raids });
        }
        p.beasts.sort_by_key(|b| (b.town.is_none(), b.town.as_ref().map(|t| t.1).unwrap_or(usize::MAX), std::cmp::Reverse(b.raids)));

        if let Some(sh) = h.shadow.as_ref().filter(|s| !s.is_broken()) {
            for t in &towns {
                if Some(t.faction) == shadow_people { continue; }
                let dark = sh.at(t.location.0, t.location.1);
                if dark >= FRONTIER_CORRUPTION {
                    p.frontier.push(FrontierTown { town: t.name.clone(), people: fname(t.faction), darkness: dark });
                }
            }
            p.frontier.sort_by(|a, b| b.darkness.total_cmp(&a.darkness).then(a.town.cmp(&b.town)));
        }

        // Disputes over resources that are still being quarrelled over.
        let mut disputes: crate::history::det::HashMap<(String, String, String), usize> = Default::default();
        for e in h.chronicle.events.iter().filter(|e| recent(e.date.year) && e.event_type == EventType::Raid) {
            let Some(rest) = e.title.strip_prefix("dispute over ") else { continue };
            let Some((res, _)) = rest.split_once(" between ") else { continue };
            if e.factions_involved.len() < 2 || !e.factions_involved.iter().all(|f| active(f)) { continue; }
            let (mut a, mut b) = (fname(e.factions_involved[0]), fname(e.factions_involved[1]));
            if a > b { std::mem::swap(&mut a, &mut b); }
            *disputes.entry((res.to_string(), a, b)).or_default() += 1;
        }
        let mut disputes: Vec<_> = disputes.into_iter().map(|((resource, a, b), times)| Dispute { resource, a, b, times }).collect();
        disputes.sort_by(|x, y| y.times.cmp(&x.times).then(x.resource.cmp(&y.resource)).then(x.a.cmp(&y.a)));
        p.disputes = disputes;

        for f in h.factions.values().filter(|f| f.is_active()) {
            let heir = f.current_leader.and_then(|l| h.figures.get(&l))
                .map_or(false, |l| l.children.iter().any(|c| h.figures.get(c).map_or(false, |c| c.is_alive())));
            if !heir { p.heirless.push(f.name.clone()); }
        }
        p.heirless.sort();

        for e in h.chronicle.events.iter().filter(|e| recent(e.date.year)) {
            let how = match e.event_type {
                EventType::SiegeEnded if e.description.contains("burned it to the ground") => "burned",
                EventType::SiegeEnded if e.description.contains(" captured ") => "taken",
                EventType::ShadowConquest => "fell to the Shadow",
                EventType::SettlementDestroyed => "destroyed",
                _ => continue,
            };
            let town = e.primary_participants.iter().find_map(|p| match p {
                EntityId::Settlement(s) => h.settlements.get(s).map(|t| (t.name.clone(), t.location)),
                _ => None,
            });
            if let Some((town, at)) = town {
                p.fallen.push(FallenTown { town, year: e.date.year, how: how.to_string(), at });
            }
        }
        p.fallen.dedup_by(|a, b| a.town == b.town && a.year == b.year);
        p
    }

    /// One line of counts for the end-of-history report.
    pub fn counts(&self) -> String {
        format!(
            "Present day (year {}): {} peoples, {} wars, {} sieges, {} grudges, {} living beasts ({} near a town), {} towns on the Shadow's frontier, {} disputes, {} towns fallen in the last {} years",
            self.year, self.peoples, self.wars.len(), self.sieges.len(), self.grudges.len(), self.beasts.len(),
            self.beasts.iter().filter(|b| b.town.is_some()).count(), self.frontier.len(), self.disputes.len(),
            self.fallen.len(), RECENT_YEARS,
        )
    }

    /// The full report, for `--present`.
    pub fn report(&self) -> String {
        use std::fmt::Write;
        let mut s = String::new();
        let _ = writeln!(s, "=== The present day: year {} ===", self.year);
        let _ = writeln!(s, "{}", self.counts());
        let _ = writeln!(s, "Wars under way:");
        for w in &self.wars { let _ = writeln!(s, "  {} since {}: {} against {}", w.name, w.since, w.sides.0, w.sides.1); }
        let _ = writeln!(s, "Sieges:");
        for x in &self.sieges { let _ = writeln!(s, "  {} besieged by {} since {} (held by {})", x.town, x.attacker, x.since, x.defender); }
        let _ = writeln!(s, "Grudges:");
        for g in &self.grudges {
            let _ = writeln!(s, "  {} and {} ({}){}", g.a, g.b, g.opinion, g.because.as_ref().map(|c| format!(", since {}", c)).unwrap_or_default());
        }
        let _ = writeln!(s, "Living beasts:");
        for b in &self.beasts {
            let near = b.town.as_ref().map(|(t, d)| format!("{} tiles from {}", d, t)).unwrap_or_else(|| "far from any town".into());
            let _ = writeln!(s, "  {} at {:?}, {}; {} raids", b.name, b.lair, near, b.raids);
        }
        let _ = writeln!(s, "The Shadow's frontier:");
        for t in &self.frontier { let _ = writeln!(s, "  {} ({}), darkness {:.2}", t.town, t.people, t.darkness); }
        let _ = writeln!(s, "Disputes over resources (last {} years):", RECENT_YEARS);
        for d in &self.disputes { let _ = writeln!(s, "  {} between {} and {} ({} times)", d.resource, d.a, d.b, d.times); }
        let _ = writeln!(s, "Rulers without a living heir: {}", if self.heirless.is_empty() { "none".into() } else { self.heirless.join(", ") });
        let _ = writeln!(s, "Towns fallen in the last {} years:", RECENT_YEARS);
        for f in &self.fallen { let _ = writeln!(s, "  {} {} in {} (tile {},{})", f.town, f.how, f.year, f.at.0, f.at.1); }
        s
    }
}

fn last_quarrel(h: &WorldHistory, a: FactionId, b: FactionId) -> Option<String> {
    h.chronicle.events.iter().rev()
        .find(|e| e.event_type == EventType::Raid && e.factions_involved.contains(&a) && e.factions_involved.contains(&b))
        .map(|e| format!("a {} in {}", e.title.split(" between ").next().unwrap_or("quarrel"), e.date.year))
}

/// Tile distance on a map that wraps east-west.
fn dist(a: (usize, usize), b: (usize, usize), width: usize) -> usize {
    let dx = a.0.abs_diff(b.0);
    let dx = dx.min(width.saturating_sub(dx));
    ((dx * dx + a.1.abs_diff(b.1).pow(2)) as f64).sqrt().round() as usize
}

impl WorldHistory {
    /// The world as the game inherits it at the end of history.
    pub fn present(&self) -> PresentDay { PresentDay::of(self) }
}
