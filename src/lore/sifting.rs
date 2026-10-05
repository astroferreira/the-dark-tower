//! Story sifting: the coincidences people retell, found in the finished history.
//!
//! A simulation makes events; readers remember the shape of a few of them: the captain named for
//! a victory who later dies on the same field, the ford fought over by three generations, the town
//! taken and won back, the beast that raided one town until someone from it killed it, the people
//! who held out in their last town, the blade that broke the Shadow ending in a dragon's hoard.
//! `sift` runs these queries over the chronicle (participants and causes) and returns each find
//! as a tale of two or three sentences with its events linked and a score. The journal prints
//! them as "Tales worth telling"; the inspector shows the tale on each of its events.

use crate::history::*;
use crate::history::det::HashMap;
use crate::history::events::types::{Event, EventType};
use crate::history::world_state::WorldHistory;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TaleKind {
    /// Named for a victory at a place, killed at the same place.
    FellWhereTheyWon,
    /// One place fought over across generations.
    Echo,
    /// A town taken, then won back by its old people.
    Reversal,
    /// A beast that raided one town again and again, until it was slain.
    BeastsBane,
    /// A people that lost its seat and held out in its last town.
    LastStand,
    /// The weapon that broke the Shadow, in a beast's hoard or a stranger's hands.
    BladeAstray,
}

impl TaleKind {
    pub fn label(self) -> &'static str {
        match self {
            TaleKind::FellWhereTheyWon => "Irony",
            TaleKind::Echo => "Echo",
            TaleKind::Reversal => "Reversal",
            TaleKind::BeastsBane => "Nemesis",
            TaleKind::LastStand => "Last stand",
            TaleKind::BladeAstray => "Irony",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Tale {
    pub kind: TaleKind,
    pub title: String,
    /// Two or three sentences.
    pub text: String,
    /// The events it is made of, in order.
    pub events: Vec<EventId>,
    /// Higher is better: more linked events, a longer span, a rarer kind.
    pub score: f32,
    pub year: u32,
}

fn tale(kind: TaleKind, title: String, text: String, events: Vec<&Event>, rarity: f32) -> Tale {
    let span = events.last().map_or(0, |l| l.date.year).saturating_sub(events.first().map_or(0, |f| f.date.year));
    Tale {
        kind, title, text,
        year: events.last().map_or(0, |e| e.date.year),
        score: events.len() as f32 + (span as f32 / 25.0).min(4.0) + rarity,
        events: events.iter().map(|e| e.id).collect(),
    }
}

fn settlement_of(e: &Event) -> Option<SettlementId> {
    e.primary_participants.iter().find_map(|p| if let EntityId::Settlement(s) = p { Some(*s) } else { None })
}

/// Every tale in the history, best first.
pub fn sift(h: &WorldHistory) -> Vec<Tale> {
    let mut tales = Vec::new();
    let town = |s: SettlementId| h.settlements.get(&s).map(|t| t.name.clone()).unwrap_or_default();
    let people = |f: FactionId| h.factions.get(&f).map(|x| x.name.clone()).unwrap_or_default();
    let mut at_town: HashMap<SettlementId, Vec<&Event>> = HashMap::default();
    for e in &h.chronicle.events {
        if let Some(s) = settlement_of(e) { at_town.entry(s).or_default().push(e); }
    }
    let mut towns: Vec<SettlementId> = at_town.keys().copied().collect();
    towns.sort();
    let mut battles_of: HashMap<FigureId, Vec<&Event>> = HashMap::default();
    let mut slain_of: HashMap<LegendaryCreatureId, &Event> = HashMap::default();
    for e in &h.chronicle.events {
        match e.event_type {
            EventType::BattleFought => for p in &e.primary_participants {
                if let EntityId::Figure(f) = p { battles_of.entry(*f).or_default().push(e); }
            },
            EventType::CreatureSlain => for p in &e.primary_participants {
                if let EntityId::LegendaryCreature(c) = p { slain_of.entry(*c).or_insert(e); }
            },
            _ => {}
        }
    }

    // Fell where they won: "X the Victor of T" dies in a battle at T.
    let mut figs: Vec<&crate::history::entities::figures::Figure> = h.figures.values().filter(|f| f.epithet.is_some() && f.death_date.is_some()).collect();
    figs.sort_by_key(|f| f.id);
    for f in figs {
        let ep = f.epithet.as_deref().unwrap_or_default();
        let Some((_, place)) = ep.split_once(" of ") else { continue };
        let battles: Vec<&Event> = battles_of.get(&f.id).map_or(Vec::new(), |v| v.iter().copied()
            .filter(|e| settlement_of(e).map_or(false, |s| town(s) == place)).collect());
        let won = battles.iter().find(|e| e.description.starts_with(&f.name));
        let died = battles.iter().rev().find(|e| Some(e.date) == f.death_date);
        if let (Some(won), Some(died)) = (won, died) {
            if won.id == died.id { continue; }
            let war = died.causes.first().and_then(|c| h.chronicle.get(*c));
            let mut evs = vec![*won, *died];
            if let Some(w) = war { evs.insert(1, w); }
            evs.sort_by_key(|e| e.date);
            let years = died.date.year - won.date.year;
            tales.push(tale(TaleKind::FellWhereTheyWon,
                format!("{} {}", f.name, ep),
                format!("{} was hailed {} after a victory there in {}. {}, {} died on the same ground, in {}.",
                    f.name, ep, won.date.year, match years { 1 => "The next year".to_string(), n => format!("{} years later", n) },
                    f.name, died.title.replacen("The ", "the ", 1)),
                evs, 2.0));
        }
    }

    // Echo: three or more battles at one town over forty years or more.
    for &s in &towns {
        let battles: Vec<&Event> = at_town[&s].iter().copied().filter(|e| e.event_type == EventType::BattleFought).collect();
        if battles.len() < 3 { continue; }
        let (first, last) = (battles[0], battles[battles.len() - 1]);
        if last.date.year < first.date.year + 40 { continue; }
        let mid = battles[battles.len() / 2];
        let mut sides: Vec<FactionId> = battles.iter().flat_map(|e| e.factions_involved.iter().copied()).collect();
        sides.sort();
        sides.dedup();
        tales.push(tale(TaleKind::Echo,
            format!("The fields of {}", town(s)),
            if first.title == last.title {
                format!("{} battles were fought at {} between {} and {}, by {} peoples, each one called {}.",
                    battles.len(), town(s), first.date.year, last.date.year, sides.len(), first.title.replacen("The ", "the ", 1))
            } else {
                format!("{} battles were fought at {} between {} and {}, by {} peoples. The first was {}; the last, {}.",
                    battles.len(), town(s), first.date.year, last.date.year, sides.len(),
                    first.title.replacen("The ", "the ", 1), last.title.replacen("The ", "the ", 1))
            },
            vec![first, mid, last], 0.5));
    }

    // Reversal: a town taken from a people and later won back by it.
    for &s in &towns {
        let takes: Vec<&Event> = at_town[&s].iter().copied()
            .filter(|e| matches!(e.event_type, EventType::SiegeEnded | EventType::ShadowConquest | EventType::ShadowLiberated))
            // A town burned is not taken back.
            .filter(|e| !e.description.contains("burned") && !e.title.contains("burned") && !e.description.contains("ashes"))
            .collect();
        for (i, lost) in takes.iter().enumerate() {
            let (Some(&taker), Some(&loser)) = (lost.factions_involved.first(), lost.factions_involved.get(1)) else { continue };
            if lost.event_type == EventType::ShadowLiberated { continue; }
            let back = takes[i + 1..].iter().find(|e| e.factions_involved.first() == Some(&loser) && e.factions_involved.contains(&taker));
            let Some(back) = back else { continue };
            let years = back.date.year - lost.date.year;
            if years < 5 { continue; }
            let between = takes[i + 1..].iter().find(|e| e.date < back.date && e.id != back.id).copied();
            let mut evs = vec![*lost];
            if let Some(b) = between { evs.push(b); }
            evs.push(*back);
            if let Some(c) = back.causes.first().and_then(|c| h.chronicle.get(*c)).filter(|_| evs.len() < 3) { evs.insert(evs.len() - 1, c); }
            tales.push(tale(TaleKind::Reversal,
                format!("{} won back", town(s)),
                format!("{} lost {} to {} in {}. {} years later {} took it back.",
                    people(loser), town(s), people(taker), lost.date.year, years, people(loser)),
                evs, 1.0));
            break;
        }
    }

    // A beast's bane: a beast that raided one town three times or more and was then slain.
    let mut raids: HashMap<(LegendaryCreatureId, SettlementId), Vec<&Event>> = HashMap::default();
    for e in h.chronicle.events.iter().filter(|e| e.event_type == EventType::MonsterRaid) {
        let beast = e.primary_participants.iter().find_map(|p| if let EntityId::LegendaryCreature(c) = p { Some(*c) } else { None });
        if let (Some(b), Some(s)) = (beast, settlement_of(e)) { raids.entry((b, s)).or_default().push(e); }
    }
    let mut keys: Vec<_> = raids.keys().copied().collect();
    keys.sort();
    for (b, s) in keys {
        let rs = &raids[&(b, s)];
        if rs.len() < 3 { continue; }
        let slain = slain_of.get(&b).copied();
        // Slain while the raids are still remembered (within a generation of the last).
        let Some(slain) = slain.filter(|e| e.date.year <= rs[rs.len() - 1].date.year + 30) else { continue };
        let beast = h.legendary_creatures.get(&b).map(|c| c.full_name()).unwrap_or_default();
        let slayer = slain.primary_participants.iter().find_map(|p| if let EntityId::Figure(f) = p { h.figures.get(f) } else { None });
        let dead: u32 = rs.iter().filter_map(|e| e.description.split("killing ").nth(1).and_then(|x| x.split_whitespace().next()).and_then(|n| n.parse::<u32>().ok())).sum();
        let from_town = slayer.and_then(|f| h.people.as_ref().and_then(|p| p.home.get(&f.id))).map_or(false, |home| *home == s);
        tales.push(tale(TaleKind::BeastsBane,
            format!("{} and {}", beast, town(s)),
            format!("{} raided {} {} times between {} and {}, killing {}. {} {} slew it in {}.",
                beast, town(s), rs.len(), rs[0].date.year, rs[rs.len() - 1].date.year, dead,
                slayer.map(|f| f.full_name()).unwrap_or_else(|| "A hero".into()),
                if from_town { format!("of {}", town(s)) } else { String::new() }, slain.date.year).replace("  ", " "),
            vec![rs[0], rs[rs.len() - 1], slain], if from_town { 2.0 } else { 0.5 }));
    }

    // Last stand: a people loses its seat, holds out in what remains for five to forty years,
    // then falls (once per people: a people that rose again and fell twice is told once).
    let mut told: Vec<FactionId> = Vec::new();
    for e in h.chronicle.events.iter().filter(|e| e.event_type == EventType::FactionDestroyed) {
        let Some(&f) = e.factions_involved.first() else { continue };
        if told.contains(&f) { continue; }
        let seat_fall = h.chronicle.events.iter().filter(|x| x.date < e.date && matches!(x.event_type, EventType::SiegeEnded | EventType::ShadowConquest)
            && x.factions_involved.get(1) == Some(&f)
            && settlement_of(x).and_then(|s| h.settlements.get(&s)).map_or(false, |t| t.settlement_type == crate::history::civilizations::settlement::SettlementType::Capital))
            .last();
        let Some(seat_fall) = seat_fall else { continue };
        let years = e.date.year - seat_fall.date.year;
        if !(5..=40).contains(&years) { continue; }
        let last_fall = e.causes.first().and_then(|c| h.chronicle.get(*c)).filter(|c| c.id != seat_fall.id);
        let last_town = last_fall.and_then(settlement_of).map(town);
        let founding = h.chronicle.events.iter().find(|x| x.event_type == EventType::FactionFounded && x.factions_involved.first() == Some(&f));
        let mut evs: Vec<&Event> = founding.into_iter().chain([seat_fall]).chain(last_fall).chain([e]).collect();
        evs.dedup_by_key(|x| x.id);
        tales.push(tale(TaleKind::LastStand,
            format!("The last of {}", people(f)),
            { let seat_name = settlement_of(seat_fall).map(town).unwrap_or_default();
            match last_town.filter(|t| *t != seat_name) {
                Some(t) => format!("{} lost its seat, {}, in {}. It held out {} more years, last at {}, until {} fell in {}.",
                    people(f), settlement_of(seat_fall).map(town).unwrap_or_default(), seat_fall.date.year, years, t, t, e.date.year),
                None => format!("{} lost its seat, {}, in {} and held out {} more years in what remained, until the year {}.",
                    people(f), settlement_of(seat_fall).map(town).unwrap_or_default(), seat_fall.date.year, years, e.date.year),
            } },
            evs, 1.0));
        told.push(f);
    }

    // The blade astray: the Shadow's bane, now in a beast's hoard or a stranger's hands.
    for bane in h.chronicle.events.iter().filter(|e| e.event_type == EventType::ShadowBane) {
        let Some(a) = bane.primary_participants.iter().find_map(|p| if let EntityId::Artifact(a) = p { Some(*a) } else { None }) else { continue };
        let alliance = bane.causes.first().and_then(|c| h.chronicle.get(*c));
        let taken = h.chronicle.events.iter().filter(|e| e.date >= bane.date && e.id != bane.id && e.primary_participants.contains(&EntityId::Artifact(a))).last();
        let (Some(alliance), Some(taken)) = (alliance, taken) else { continue };
        let name = h.artifacts.get(&a).map(|x| x.name.clone()).unwrap_or_default();
        let holder = taken.primary_participants.iter().find_map(|p| match p {
            EntityId::LegendaryCreature(c) => h.legendary_creatures.get(c).map(|b| format!("it lay in the hoard of {}", b.full_name())),
            EntityId::Figure(f) => h.figures.get(f).map(|x| format!("it was in the hands of {}", x.full_name())),
            _ => None,
        });
        let Some(holder) = holder else { continue };
        tales.push(tale(TaleKind::BladeAstray,
            format!("Where {} went", name),
            format!("{} broke the Shadow at {} in {}, and was lost in the fall of the seat. By {} {}.",
                name, settlement_of(bane).map(town).unwrap_or_default(), bane.date.year, taken.date.year, holder),
            vec![alliance, bane, taken], 3.0));
    }

    tales.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.year.cmp(&b.year)).then(a.title.cmp(&b.title)));
    interleave(tales)
}

/// Best first, but kinds taking turns: the best of each kind, then the second best of each...
fn interleave(tales: Vec<Tale>) -> Vec<Tale> {
    let mut by_kind: Vec<(TaleKind, std::collections::VecDeque<Tale>)> = Vec::new();
    for t in tales {
        match by_kind.iter_mut().find(|k| k.0 == t.kind) {
            Some(k) => k.1.push_back(t),
            None => by_kind.push((t.kind, std::collections::VecDeque::from([t]))),
        }
    }
    let mut out = Vec::new();
    loop {
        let mut any = false;
        for (_, q) in by_kind.iter_mut() {
            if let Some(t) = q.pop_front() { out.push(t); any = true; }
        }
        if !any { break; }
    }
    out
}

/// One line for the end-of-history report.
pub fn report(tales: &[Tale]) -> String {
    let mut kinds: Vec<(TaleKind, usize)> = Vec::new();
    for t in tales { match kinds.iter_mut().find(|k| k.0 == t.kind) { Some(k) => k.1 += 1, None => kinds.push((t.kind, 1)) } }
    kinds.sort();
    let rich = tales.iter().filter(|t| t.events.len() >= 3).count();
    format!("Tales: {} worth telling ({} of 3+ linked events): {}", tales.len(), rich,
        kinds.iter().map(|(k, n)| format!("{} {:?}", n, k)).collect::<Vec<_>>().join(", "))
}
