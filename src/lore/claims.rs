//! A world in one sentence: its records, the rarest first.
//!
//! Every world differs in its numbers; it also has to *feel* different. The landmarks give the
//! geography its records (the longest river, the highest peak); this gives the history its own
//! (the longest reign, the bloodiest battle, the town that changed hands most, the beast with the
//! most dead, the longest war, the greatest captain, the Shadow's tally). Each record is scored
//! against what is typical (the median record over dev seeds, so each world's real outliers
//! win), and the three most unusual make the world's sentence, used on the journal's title page
//! and the present-day report.

use crate::history::*;
use crate::history::det::HashMap;
use crate::history::events::types::EventType;
use crate::history::world_state::WorldHistory;

const TYPICAL_CITY: f32 = 45_000.0;
const TYPICAL_RAIDS: f32 = 25.0;

#[derive(Clone, Debug)]
pub struct Claim {
    /// A clause: "Sku the Butcher ruled The Ashpit Horde for 71 years".
    pub text: String,
    /// How unusual it is (1 = typical).
    pub rarity: f32,
}

fn thousands(n: u32) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 { out.push(','); }
        out.push(c);
    }
    out
}

/// The history's records, most unusual first.
pub fn claims(h: &WorldHistory) -> Vec<Claim> {
    let mut out = Vec::new();
    let now = h.current_date.year;
    let fname = |f: FactionId| h.factions.get(&f).map(|x| x.name.clone()).unwrap_or_default();

    // The longest reign.
    let mut crowned: HashMap<FigureId, (u32, Option<FactionId>)> = HashMap::default();
    for e in h.chronicle.events.iter().filter(|e| e.event_type == EventType::RulerCrowned) {
        for p in &e.primary_participants {
            if let EntityId::Figure(f) = p { crowned.entry(*f).or_insert((e.date.year, e.factions_involved.first().copied())); }
        }
    }
    let reign = crowned.iter()
        .filter_map(|(f, (from, fac))| h.figures.get(f).map(|x| (x.death_date.map_or(now, |d| d.year).saturating_sub(*from), x, *fac)))
        .max_by_key(|(years, x, _)| (*years, std::cmp::Reverse(x.id)));
    if let Some((years, x, fac)) = reign.filter(|r| r.0 > 0) {
        out.push(Claim { text: format!("{} ruled {} for {} years{}", x.full_name(), fac.map(fname).unwrap_or_default(), years,
            if x.is_alive() { ", and rules still" } else { "" }), rarity: years as f32 / 150.0 });
    }

    // The bloodiest battle ("(412 of 2,000 fell, 80 of 1,500)").
    let fell = |d: &str| -> u32 {
        let Some(i) = d.find(" fell, ") else { return 0 };
        let head = &d[..i];
        let a = head.rsplit('(').next().and_then(|x| x.split_whitespace().next()).and_then(|n| n.parse::<u32>().ok()).unwrap_or(0);
        let b = d[i + 7..].split_whitespace().next().and_then(|n| n.parse::<u32>().ok()).unwrap_or(0);
        a + b
    };
    if let Some((dead, e)) = h.chronicle.events.iter().filter(|e| e.event_type == EventType::BattleFought)
        .map(|e| (fell(&e.description), e)).max_by_key(|(d, e)| (*d, std::cmp::Reverse(e.id))).filter(|x| x.0 > 0) {
        out.push(Claim { text: format!("{} died in a single day at {} ({})", thousands(dead), e.title.replacen("The ", "the ", 1), e.date.year),
            rarity: dead as f32 / 800.0 });
    }

    // The town that changed hands most.
    let mut hands: HashMap<SettlementId, u32> = HashMap::default();
    for e in &h.chronicle.events {
        let taken = match e.event_type {
            EventType::SiegeEnded => e.description.contains(" captured "),
            EventType::ShadowConquest => !e.title.contains("burned"),
            EventType::ShadowLiberated => true,
            _ => false,
        };
        if !taken { continue; }
        if let Some(s) = e.primary_participants.iter().find_map(|p| if let EntityId::Settlement(s) = p { Some(*s) } else { None }) {
            *hands.entry(s).or_default() += 1;
        }
    }
    if let Some((s, n)) = hands.iter().max_by_key(|(s, n)| (**n, std::cmp::Reverse(**s))).filter(|x| *x.1 >= 2) {
        if let Some(t) = h.settlements.get(s) {
            out.push(Claim { text: format!("{} changed hands {} times", t.name, n), rarity: *n as f32 / 9.0 });
        }
    }

    // The beast with the most dead.
    let mut killed: HashMap<LegendaryCreatureId, u32> = HashMap::default();
    for e in h.chronicle.events.iter().filter(|e| e.event_type == EventType::MonsterRaid) {
        let n = e.description.split("killing ").nth(1).and_then(|x| x.split_whitespace().next()).and_then(|n| n.parse::<u32>().ok()).unwrap_or(0);
        if let Some(c) = e.primary_participants.iter().find_map(|p| if let EntityId::LegendaryCreature(c) = p { Some(*c) } else { None }) {
            *killed.entry(c).or_default() += n;
        }
    }
    if let Some((c, n)) = killed.iter().max_by_key(|(c, n)| (**n, std::cmp::Reverse(**c))).filter(|x| *x.1 > 0) {
        if let Some(b) = h.legendary_creatures.get(c) {
            out.push(Claim { text: format!("{} killed {} in its raids{}", b.full_name(), thousands(*n), if b.is_alive() { " and lives yet" } else { "" }),
                rarity: *n as f32 / 1000.0 });
        }
    }

    // The longest war.
    if let Some(w) = h.wars.values().max_by_key(|w| (w.ended.map_or(now, |d| d.year).saturating_sub(w.started.year), std::cmp::Reverse(w.id))) {
        let years = w.ended.map_or(now, |d| d.year).saturating_sub(w.started.year);
        if years > 0 {
            out.push(Claim { text: format!("{} lasted {} years{}", w.name.replacen("The ", "the ", 1), years, if w.ended.is_none() { " and is fought still" } else { "" }),
                rarity: years as f32 / 20.0 });
        }
    }

    // The greatest captain: most battles won (the victor's commander opens the account).
    let mut wins: HashMap<FigureId, u32> = HashMap::default();
    for e in h.chronicle.events.iter().filter(|e| e.event_type == EventType::BattleFought) {
        if let Some(EntityId::Figure(f)) = e.primary_participants.first() {
            if h.figures.get(f).map_or(false, |x| e.description.starts_with(&x.name)) { *wins.entry(*f).or_default() += 1; }
        }
    }
    if let Some((f, n)) = wins.iter().max_by_key(|(f, n)| (**n, std::cmp::Reverse(**f))).filter(|x| *x.1 >= 2) {
        if let Some(x) = h.figures.get(f) {
            out.push(Claim { text: format!("{} won {} battles", x.full_name(), n), rarity: *n as f32 / 25.0 });
        }
    }

    // The Shadow's tally.
    if let Some(sh) = h.shadow.as_ref() {
        let taken = h.chronicle.events.iter().filter(|e| e.event_type == EventType::ShadowConquest).count() as u32;
        let burned = h.chronicle.events.iter().filter(|e| e.event_type == EventType::ShadowConquest && e.title.contains("burned")).count() as u32;
        if taken > 0 {
            out.push(Claim { text: format!("{} took {} towns and burned {} of them", sh.name.replacen("the ", "the ", 1), taken, burned),
                rarity: taken as f32 / 26.0 });
        }
    }

    // The greatest city, and the town raided most.
    if let Some(t) = h.settlements.values().filter(|t| !t.is_destroyed()).max_by_key(|t| (t.population, std::cmp::Reverse(t.id))) {
        out.push(Claim { text: format!("{} grew to {} souls", t.name, thousands(t.population)), rarity: t.population as f32 / TYPICAL_CITY });
    }
    let mut raided: HashMap<SettlementId, u32> = HashMap::default();
    for e in h.chronicle.events.iter().filter(|e| matches!(e.event_type, EventType::MonsterRaid | EventType::Raid)) {
        if let Some(s) = e.primary_participants.iter().find_map(|p| if let EntityId::Settlement(s) = p { Some(*s) } else { None }) {
            *raided.entry(s).or_default() += 1;
        }
    }
    if let Some((s, n)) = raided.iter().max_by_key(|(s, n)| (**n, std::cmp::Reverse(**s))).filter(|x| *x.1 >= 3) {
        if let Some(t) = h.settlements.get(s) {
            out.push(Claim { text: format!("{} was raided {} times", t.name, n), rarity: *n as f32 / TYPICAL_RAIDS });
        }
    }

    out.sort_by(|a, b| b.rarity.total_cmp(&a.rarity).then(a.text.cmp(&b.text)));
    out
}

/// "In Neaslind, Sku the Butcher ruled The Ashpit Horde for 71 years; Ripu changed hands 6
/// times; and 1,240 died in a single day at the Battle of Ripu Ford (318)."
pub fn sentence(world: &str, claims: &[Claim]) -> String {
    let parts: Vec<&str> = claims.iter().take(3).map(|c| c.text.as_str()).collect();
    match parts.as_slice() {
        [] => format!("{} has no records yet.", world),
        [a] => format!("In {}, {}.", world, a),
        [a, b] => format!("In {}, {}; and {}.", world, a, b),
        [a, b, c, ..] => format!("In {}, {}; {}; and {}.", world, a, b, c),
    }
}
