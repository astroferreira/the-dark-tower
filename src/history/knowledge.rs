//! Who knows what, and how they tell it: per-town and per-people knowledge of the chronicle.
//!
//! The idea is Dwarf Fortress's per-entity knowledge (`local_known_events` on entities,
//! `known_info` on figures): what someone can tell you is a filter over the event log, which
//! gives rumours, secrets and differing accounts for free. Here it is a derived layer over the
//! finished history: nothing is saved, no RNG is drawn, the history is not changed.
//!
//! - Whether a town knows an event (`Knowledge::town_knows`): it took part (a participant, its
//!   tile, or its people among those involved) or word reached it. An event's fame (`Fame`, by
//!   kind) sets how far word goes, measured in town spacings (the median distance between
//!   neighbouring living towns, so a dev world and a 512-wide one behave alike), and how long it
//!   is remembered; word travels three spacings a year, so this year's news has not reached far
//!   towns yet. Word also comes along roads: through the town's trade partners (a route that ran
//!   at some time since the event; a quarter of the road's length counts) and its own people's
//!   other towns (half). The edge of the reach is fuzzy per (event, town), by a hash.
//! - A people knows what any of its living towns knows; a figure what their home town knows,
//!   plus their own deeds.
//! - How it is told (`account`): who won and lost is read off the chronicle (`stakes`); a
//!   people tells its own victory proudly (a commander's deed exaggerated, the enemy's dead
//!   multiplied), its defeat bitterly (treachery, numbers past counting, a broken truce), and a
//!   third people takes the side of a people it loves (opinion 40+) or against one it hates
//!   (-40 or worse). Everyone else tells it plainly.

use crate::history::det::HashMap;
use crate::history::events::types::{Event, EventType as E};
use crate::history::world_state::WorldHistory;
use crate::history::{EntityId, EventId, FactionId, FigureId, SettlementId};

/// How far word of an event goes and how long it is remembered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Fame { Local, Notable, Great, Legend }

impl Fame {
    /// Reach in town spacings.
    fn reach(self) -> f32 { match self { Fame::Local => 1.5, Fame::Notable => 3.5, Fame::Great => 7.0, Fame::Legend => f32::INFINITY } }
    /// Years before it is forgotten by those it did not touch.
    fn memory(self) -> u32 { match self { Fame::Local => 30, Fame::Notable => 80, Fame::Great => 200, Fame::Legend => u32::MAX } }
}

/// An event's fame, by its kind.
pub fn fame(e: &Event) -> Fame {
    let f = match e.event_type {
        E::FactionFounded | E::FactionDestroyed | E::ShadowRose | E::ShadowBroken | E::ShadowAlliance | E::ShadowBane | E::MagicalCatastrophe | E::ReligionFounded => Fame::Legend,
        E::WarDeclared | E::WarEnded | E::HolyWarDeclared | E::ShadowConquest | E::CreatureSlain | E::Plague | E::VolcanoErupted
            | E::SettlementDestroyed | E::Assassination | E::Coup | E::Earthquake => Fame::Great,
        E::SiegeEnded if !e.title.ends_with(" lifted") => Fame::Great,
        E::BattleFought | E::SiegeBegun | E::SiegeEnded | E::ShadowRepelled | E::ShadowLiberated | E::MonsterRaid | E::Massacre
            | E::Rebellion | E::SuccessionCrisis | E::RulerCrowned | E::RulerDeposed | E::TreatySigned | E::AllianceFormed | E::AllianceBroken
            | E::TreatyBroken | E::QuestCompleted | E::MonumentBuilt | E::Marriage | E::LandScarred | E::Flood | E::Drought | E::Miracle
            | E::TempleProfaned | E::ArtifactLost | E::ArtifactFound | E::Authored => Fame::Notable,
        _ if e.title.contains("rises again") => Fame::Great,
        _ => Fame::Local,
    };
    // A quest that came to nothing, a plot foiled: less to tell.
    if e.title.contains("empty-handed") { Fame::Local } else if e.title.starts_with("Failed ") { f.min(Fame::Notable) } else { f }
}

/// How a teller leans on an event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slant { Plain, Proud, Bitter }

/// An event as someone tells it.
#[derive(Clone, Debug)]
pub struct Told {
    pub event: EventId,
    pub year: u32,
    /// The plain headline: "the Battle of Ripu Field (247)".
    pub headline: String,
    /// The teller's gloss, which reads after the headline or after "they call it": "a great
    /// victory, where Othrasssz broke the Kingdom of Titankeep and 60 of the enemy fell".
    pub gloss: Option<String>,
    pub teller: Option<FactionId>,
    /// "the Sandrock Clans" (the teller's name in running text).
    pub teller_name: String,
    pub slant: Slant,
    /// How it sits with each people, town, person and beast it touches: +1 good, -1 bad.
    pub stakes: Vec<(EntityId, i8)>,
    /// How each people it touches tells it: (people, its name in running text, its gloss).
    pub others: Vec<(FactionId, String, String)>,
}

impl Told {
    /// The headline with the teller's gloss: "the Battle of Ripu Field (247), a great victory...".
    pub fn line(&self) -> String {
        match &self.gloss { Some(g) => format!("{}, {}", self.headline, g), None => self.headline.clone() }
    }
    /// "as the Git Clans tell it" (empty when told plainly).
    pub fn as_told(&self) -> String {
        if self.slant == Slant::Plain || self.teller_name.is_empty() { String::new() } else { format!("as {} {} it", self.teller_name, tell_verb(&self.teller_name)) }
    }
    /// How it sits with `who` (0 when it does not touch them).
    pub fn stake(&self, who: &EntityId) -> i8 { self.stakes.iter().find(|(e, _)| e == who).map_or(0, |s| s.1) }
}

/// "tell" for a plural name ("the Git Clans"), else "tells" ("the Ashpit Horde").
pub fn tell_verb(name: &str) -> &'static str { if name.ends_with('s') { "tell" } else { "tells" } }

/// A people's name in running text: "the Git Clans".
pub fn running_name(h: &WorldHistory, f: FactionId) -> String {
    h.factions.get(&f).map(|x| x.name.replacen("The ", "the ", 1)).unwrap_or_default()
}

fn hash(a: u64, b: u64) -> u64 { crate::history::settlers::hash_pub(a, b) }
fn unit(x: u64) -> f32 { (x % 10_000) as f32 / 10_000.0 }

fn towns_of(e: &Event) -> impl Iterator<Item = SettlementId> + '_ {
    e.primary_participants.iter().filter_map(|p| if let EntityId::Settlement(s) = p { Some(*s) } else { None })
}
fn figures_of(e: &Event) -> impl Iterator<Item = FigureId> + '_ {
    e.primary_participants.iter().filter_map(|p| if let EntityId::Figure(f) = p { Some(*f) } else { None })
}

/// The side that came out ahead, where the chronicle says: a battle's victor (named first in
/// its description), a war's ("X prevailed"), a siege's (the taker, or the holder when lifted).
pub fn victor(h: &WorldHistory, e: &Event) -> Option<FactionId> {
    let name = |f: &FactionId| h.factions.get(f).map(|x| x.name.clone()).unwrap_or_default();
    match e.event_type {
        E::BattleFought => e.factions_involved.iter().filter_map(|f| { let n = name(f); (!n.is_empty()).then(|| e.description.find(&n).map(|i| (i, *f))).flatten() }).min().map(|x| x.1),
        E::WarEnded => e.factions_involved.iter().copied().find(|f| e.description.contains(&format!("{} prevailed", name(f)))),
        E::SiegeEnded if e.title.ends_with(" lifted") => e.factions_involved.get(1).copied(),
        // A plot foiled is the target's people's day.
        E::Assassination if e.title.starts_with("Failed ") => e.factions_involved.get(1).copied(),
        E::SiegeEnded | E::SettlementDestroyed | E::Assassination => e.factions_involved.first().copied(),
        _ => None,
    }
}

/// How an event sits with each people, town, person and beast it touches (+1 good, -1 bad).
pub fn stakes(h: &WorldHistory, e: &Event) -> Vec<(EntityId, i8)> {
    let mut v: Vec<(EntityId, i8)> = Vec::new();
    let mut put = |x: EntityId, s: i8| if !v.iter().any(|(y, _)| *y == x) { v.push((x, s)); };
    let shadow = h.shadow.as_ref().map(|s| s.faction);
    let all = |s: i8, put: &mut dyn FnMut(EntityId, i8)| {
        for f in &e.factions_involved { put(EntityId::Faction(*f), s); }
        for t in towns_of(e) { put(EntityId::Settlement(t), s); }
        for f in figures_of(e) { put(EntityId::Figure(f), s); }
    };
    match e.event_type {
        E::BattleFought | E::WarEnded | E::SiegeEnded | E::SettlementDestroyed | E::Assassination => {
            let Some(w) = victor(h, e) else { all(-1, &mut put); return v };
            for f in &e.factions_involved { put(EntityId::Faction(*f), if *f == w { 1 } else { -1 }); }
            // The town fought over is the defender's: taken or burned is bad for it, held good.
            let held = e.event_type == E::BattleFought && e.factions_involved.get(1) == Some(&w) || e.title.ends_with(" lifted");
            for t in towns_of(e) { put(EntityId::Settlement(t), if held { 1 } else { -1 }); }
            for f in figures_of(e) {
                let side = h.figures.get(&f).and_then(|x| x.faction);
                // An assassin's victim is named first; the assassin after.
                let s = if e.event_type == E::Assassination && !e.title.starts_with("Failed ") { if Some(f) == figures_of(e).next() { -1 } else { 1 } } else if side == Some(w) { 1 } else { -1 };
                put(EntityId::Figure(f), s);
            }
        }
        E::CreatureSlain => {
            for p in &e.primary_participants { put(p.clone(), if matches!(p, EntityId::LegendaryCreature(_)) { -1 } else { 1 }); }
            for f in &e.factions_involved { put(EntityId::Faction(*f), 1); }
        }
        E::MonsterRaid | E::Raid => {
            for t in towns_of(e) { put(EntityId::Settlement(t), -1); if let Some(s) = h.settlements.get(&t) { put(EntityId::Faction(s.faction), -1); } }
        }
        E::ShadowConquest => {
            for f in &e.factions_involved { put(EntityId::Faction(*f), if Some(*f) == shadow { 1 } else { -1 }); }
            for t in towns_of(e) { put(EntityId::Settlement(t), -1); }
        }
        E::ShadowRepelled | E::ShadowLiberated => {
            for f in &e.factions_involved { put(EntityId::Faction(*f), if Some(*f) == shadow { -1 } else { 1 }); }
            for t in towns_of(e) { put(EntityId::Settlement(t), 1); }
        }
        E::ShadowAlliance => {
            let won = crate::history::shadow::CheckShape::of_title(&e.title) == crate::history::shadow::CheckShape::Victory;
            for f in &e.factions_involved { put(EntityId::Faction(*f), if (Some(*f) == shadow) == won { -1 } else { 1 }); }
        }
        E::SiegeBegun | E::WarDeclared | E::HolyWarDeclared | E::Massacre | E::Plague | E::HeroDied | E::FactionDestroyed
            | E::Rebellion | E::SuccessionCrisis | E::Coup | E::TempleProfaned | E::Drought | E::Flood | E::Earthquake | E::VolcanoErupted
            | E::MagicalCatastrophe | E::ArtifactLost | E::TreatyBroken | E::AllianceBroken => all(-1, &mut put),
        E::SettlementFounded | E::FactionFounded | E::TreatySigned | E::AllianceFormed | E::MonumentBuilt | E::RulerCrowned
            | E::Marriage | E::QuestCompleted | E::ArtifactCreated | E::ArtifactFound | E::Miracle | E::TempleBuilt => all(1, &mut put),
        _ if e.title.contains("rises again") => all(1, &mut put),
        _ => {}
    }
    v
}

/// What the chronicle reads as a phrase in running text: "the Battle of Ripu Field (247)".
pub fn headline(e: &Event) -> String {
    let t = e.title.trim_end_matches('.').replace(" The ", " the ");
    let t = match t.strip_prefix("The ") { Some(r) => format!("the {}", r), None => t };
    format!("{} ({})", t, e.date.year)
}

/// An event as a phrase in running text where its title allows one: "the trade dispute between
/// X and Y (430)", "the Battle of Ripu Field (247)", "the assassination of Ksexshosk (218)".
fn as_phrase(e: &Event) -> Option<String> {
    let t = headline(e);
    let first = t.split(' ').next().unwrap_or("");
    if first == "the" { return Some(t); }
    if first.starts_with(|c: char| c.is_lowercase()) { return Some(format!("the {}", t)); }
    const NOUNS: [&str; 10] = ["Assassination", "Death", "Siege", "End", "Founding", "Treaty", "Failed", "Alliance", "Rebellion", "Unrest"];
    NOUNS.contains(&first).then(|| format!("the {}{}", first.to_lowercase(), &t[first.len()..]))
}

/// The losses in a battle's description ("(17 of 143 fell, 41 of 1018)"): (loser's dead, winner's dead).
fn battle_losses(desc: &str) -> Option<(u32, u32)> {
    let open = desc.find(" (")?;
    let inner = &desc[open + 2..desc[open..].find(')')? + open];
    let mut parts = inner.split(", ");
    let a = parts.next()?.split(" of ").next()?.trim().parse().ok()?;
    let b = parts.next()?.split(" of ").next()?.trim().parse().ok()?;
    Some((a, b))
}

/// The side `teller` takes on `e`: its own stake, else that of a people it loves (40+), else
/// against one it hates (-40 or worse).
fn side(h: &WorldHistory, e: &Event, stakes: &[(EntityId, i8)], teller: FactionId) -> (i8, bool) {
    // A war is ill news for both, but the one who declared it tells it as the other's doing.
    let declared: Vec<(EntityId, i8)>;
    let stakes = if matches!(e.event_type, E::WarDeclared | E::HolyWarDeclared) && e.factions_involved.len() >= 2 {
        declared = vec![(EntityId::Faction(e.factions_involved[0]), 1), (EntityId::Faction(e.factions_involved[1]), -1)];
        &declared[..]
    } else { stakes };
    let own = stakes.iter().find(|(x, _)| *x == EntityId::Faction(teller)).map_or(0, |s| s.1);
    if own != 0 { return (own, false); }
    let Some(t) = h.factions.get(&teller) else { return (0, false) };
    let mut best: (i32, i8) = (0, 0);
    for (x, s) in stakes {
        let EntityId::Faction(f) = x else { continue };
        let op = t.relations.get(f).map_or(0, |r| r.opinion);
        if op >= 40 && op > best.0.abs() { best = (op, *s); }
        if op <= -40 && -op > best.0.abs() { best = (op, -*s); }
    }
    (best.1, best.1 != 0)
}

/// The glosses a side gives an event, as clauses that read after the headline or after "they
/// call it": (proud, bitter, plain). None where every side tells it alike.
fn glosses(h: &WorldHistory, e: &Event, pick: u64) -> Option<(String, String, String)> {
    let name = |f: Option<FactionId>| f.map(|f| running_name(h, f)).unwrap_or_default();
    let fig = |f: Option<FigureId>| f.and_then(|f| h.figures.get(&f)).map(|x| x.full_name());
    let w = victor(h, e);
    let l = e.factions_involved.iter().copied().find(|f| Some(*f) != w);
    let k = |n: u64| (hash(pick, n) % n.max(1)) as usize;
    match e.event_type {
        E::BattleFought => {
            let w = w?;
            let (wn, ln) = (name(Some(w)), name(l));
            let cmd = |side: Option<FactionId>| figures_of(e).find(|f| h.figures.get(f).and_then(|x| x.faction) == side).and_then(|f| fig(Some(f)));
            let wc = cmd(Some(w)).unwrap_or_else(|| format!("the host of {}", wn));
            let lc = cmd(l).unwrap_or_else(|| format!("the host of {}", ln));
            let dead = battle_losses(&e.description).map(|(ld, _)| ((ld * (2 + (hash(pick, 7) % 3) as u32) + 9) / 10 * 10).max(20)).unwrap_or(100);
            let fell = e.description.contains("died on the field");
            let proud = [
                format!("a great victory, where {} broke {} and {} of the enemy fell", wc, ln, dead),
                format!("a victory of a handful over a host, won by {}", wc),
                format!("a rout, {} driving {} from the field with hardly a loss", wc, ln),
            ][k(3)].clone();
            let bitter = [
                format!("a defeat by treachery, {} sold to {} by a traitor in the camp", lc, wn),
                format!("a defeat by numbers past counting, though {} held as long as any could", lc),
                format!("a foul blow, {} striking under a flag of truce", wn),
            ][k(3)].clone() + if fell { ", and they mourn the fallen as heroes" } else { "" };
            Some((proud, bitter, format!("a victory of {} over {}", wn, ln)))
        }
        E::WarEnded => {
            let wn = name(Some(w?));
            let proud = ["a war won by right and by valour".to_string(), format!("the triumph of {}", wn)][k(2)].clone();
            let bitter = ["a peace forced on them when their allies broke faith", "a war not lost but left unfinished"][k(2)].to_string();
            Some((proud, bitter, format!("a war {} won", wn)))
        }
        E::WarDeclared | E::HolyWarDeclared => {
            let a = e.factions_involved.first().copied();
            let d = e.factions_involved.get(1).copied()?;
            let (an, dn) = (name(a), name(Some(d)));
            let cause = e.causes.first().and_then(|c| h.chronicle.get(*c)).and_then(|c| as_phrase(c));
            // The declarer tells it as the other's doing; the other as an unprovoked attack.
            let declarer = format!("a war {} brought on themselves{}", dn, cause.map(|c| format!(" by {}", c)).unwrap_or_default());
            let target = [format!("an unprovoked attack by {}", an), format!("a war of {}'s greed", an)][k(2)].clone();
            Some((declarer, target, format!("a war {} began against {}", an, dn)))
        }
        E::SiegeEnded | E::SettlementDestroyed => {
            let town = towns_of(e).next().and_then(|t| h.settlements.get(&t)).map(|t| t.name.clone())?;
            let (wn, ln) = (name(w), name(l));
            if e.title.ends_with(" lifted") {
                Some((format!("a triumph, {} broken on the walls of {}", ln, town), "a siege given up for the winter and the sickness, not for its walls".into(), format!("a siege {} gave up", ln)))
            } else if e.event_type == E::SettlementDestroyed || e.description.contains("burned it") {
                Some(("a nest of raiders cleared".into(), format!("a massacre, {} burned with its children inside", town), format!("the burning of {} by {}", town, wn)))
            } else {
                Some((format!("a just conquest, {} glad to open its gates", town), "a betrayal, a gate left open in the night".into(), format!("the taking of {} by {}", town, wn)))
            }
        }
        E::CreatureSlain => {
            let slayer = figures_of(e).next().and_then(|f| fig(Some(f)))?;
            let proud = [
                format!("a deed beyond any other, {} fighting it three days and nights", slayer),
                format!("a deed beyond any other, {} slaying it alone with a broken spear", slayer),
                format!("a deed beyond any other, {} carrying its head home alone", slayer),
            ][k(3)].clone();
            Some((proud, format!("no great deed: it was old and half-dead already, and {} found it dying", slayer), format!("a slaying by {}", slayer)))
        }
        E::Assassination if e.title.starts_with("Failed ") => {
            let an = name(l);
            Some(("a plot foiled, as the gods meant it to be".into(), "a slander: no agent of theirs was ever there".into(), format!("a plot of {} that failed", an)))
        }
        E::Assassination => {
            let an = name(w);
            Some(("a tyrant's just end".into(), format!("a murder by the hired knives of {}", an), format!("a killing by an agent of {}", an)))
        }
        E::ShadowConquest => {
            let lord = h.shadow.as_ref().map(|s| s.name.clone()).unwrap_or_else(|| "the Shadow".into());
            Some((format!("a submission to {}", lord), format!("a betrayal, traitors opening the gates to {}", lord), String::new()))
        }
        E::ShadowRepelled => {
            let town = towns_of(e).next().and_then(|t| h.settlements.get(&t)).map(|t| t.name.clone())?;
            Some((format!("a triumph, every soul of {} on the walls", town), "a raid of the free peoples' that cost them dear".into(), String::new()))
        }
        E::ShadowLiberated => Some(("a liberation won by their own hands".into(), "a town lost to rebels and traitors".into(), String::new())),
        _ => None,
    }
}

/// `e` as `teller` tells it.
pub fn account(h: &WorldHistory, e: &Event, teller: Option<FactionId>) -> Told {
    let stakes = stakes(h, e);
    let teller_name = teller.map(|f| running_name(h, f)).unwrap_or_default();
    let pick = |f: FactionId| hash(e.id.0 as u64, 0x7E11 ^ f.0 as u64);
    let gloss_of = |f: FactionId| -> (Option<String>, Slant) {
        let (s, _) = side(h, e, &stakes, f);
        match (glosses(h, e, pick(f)), s) {
            (Some((p, _, _)), 1) => (Some(p), Slant::Proud),
            (Some((_, b, _)), -1) => (Some(b), Slant::Bitter),
            (Some((_, _, plain)), _) if !plain.is_empty() => (Some(plain), Slant::Plain),
            _ => (None, Slant::Plain),
        }
    };
    let (gloss, slant) = teller.map(gloss_of).unwrap_or_else(|| (glosses(h, e, 0).map(|g| g.2).filter(|g| !g.is_empty()), Slant::Plain));
    // How each people it touches tells it, where that differs from the plain telling.
    let mut others = Vec::new();
    for &f in &e.factions_involved {
        if let (Some(g), sl) = gloss_of(f) { if sl != Slant::Plain { others.push((f, running_name(h, f), g)); } }
    }
    Told { event: e.id, year: e.date.year, headline: as_phrase(e).unwrap_or_else(|| headline(e)), gloss, teller, teller_name, slant, stakes, others }
}

/// The knowledge layer over one history (cheap to build: trade links and town spacing).
pub struct Knowledge<'a> {
    pub h: &'a WorldHistory,
    w: usize,
    /// Median distance between neighbouring living towns, in tiles.
    pub spacing: f32,
    /// Each town's trade partners: (partner, route established, dissolved or now).
    links: HashMap<SettlementId, Vec<(SettlementId, u32, u32)>>,
    /// The living towns of each people, by id.
    kin: HashMap<FactionId, Vec<SettlementId>>,
}

/// Word travels this many town spacings a year.
const SPEED: f32 = 3.0;

impl<'a> Knowledge<'a> {
    pub fn new(h: &'a WorldHistory) -> Self {
        let w = h.tile_history.width.max(1);
        let now = h.current_date.year;
        let mut towns: Vec<(SettlementId, (usize, usize), FactionId)> = h.settlements.values().filter(|s| !s.is_destroyed()).map(|s| (s.id, s.location, s.faction)).collect();
        towns.sort_by_key(|t| t.0);
        let mut nn: Vec<f32> = towns.iter().map(|a| towns.iter().filter(|b| b.0 != a.0).map(|b| dist(a.1, b.1, w)).fold(f32::INFINITY, f32::min)).filter(|d| d.is_finite()).collect();
        nn.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let spacing = nn.get(nn.len() / 2).copied().unwrap_or(4.0).max(1.0);
        let mut links: HashMap<SettlementId, Vec<(SettlementId, u32, u32)>> = HashMap::default();
        let mut routes: Vec<_> = h.trade_routes.values().collect();
        routes.sort_by_key(|r| r.id);
        for r in routes {
            let (a, b) = r.endpoints;
            let span = (r.established.year, r.dissolved.map_or(now, |d| d.year));
            links.entry(a).or_default().push((b, span.0, span.1));
            links.entry(b).or_default().push((a, span.0, span.1));
        }
        let mut kin: HashMap<FactionId, Vec<SettlementId>> = HashMap::default();
        for t in &towns { kin.entry(t.2).or_default().push(t.0); }
        Knowledge { h, w, spacing, links, kin }
    }

    /// Where word of `e` starts: its tile, else its towns, else its peoples' seats.
    fn origins(&self, e: &Event) -> Vec<(usize, usize)> {
        if let Some(l) = e.location { return vec![l]; }
        let t: Vec<(usize, usize)> = towns_of(e).filter_map(|s| self.h.settlements.get(&s)).map(|s| s.location).collect();
        if !t.is_empty() { return t; }
        e.factions_involved.iter().filter_map(|f| self.h.factions.get(f)).filter_map(|f| f.capital).filter_map(|c| self.h.settlements.get(&c)).map(|s| s.location).collect()
    }

    /// How far word must go from `e` to town `s`, in spacings: directly, through a trade
    /// partner, or through another town of its people.
    fn distance(&self, s: SettlementId, at: (usize, usize), faction: FactionId, origins: &[(usize, usize)], year: u32) -> f32 {
        let from = |p: (usize, usize)| origins.iter().map(|o| dist(p, *o, self.w)).fold(f32::INFINITY, f32::min);
        let mut best = from(at);
        if let Some(ls) = self.links.get(&s) {
            for (p, _, until) in ls {
                if *until < year { continue; }
                if let Some(t) = self.h.settlements.get(p) { best = best.min(from(t.location) + 0.25 * dist(at, t.location, self.w)); }
            }
        }
        if let Some(ks) = self.kin.get(&faction) {
            for k in ks.iter().filter(|k| **k != s) {
                if let Some(t) = self.h.settlements.get(k) { best = best.min(from(t.location) + 0.5 * dist(at, t.location, self.w)); }
            }
        }
        best / self.spacing
    }

    /// Whether town `s` knows of `e`.
    pub fn town_knows(&self, s: SettlementId, e: &Event) -> bool {
        let Some(t) = self.h.settlements.get(&s) else { return false };
        let now = self.h.current_date.year;
        if e.date.year > now { return false; }
        let fame = fame(e);
        let age = now - e.date.year;
        // Its own: a participant, its ground, or its people's doing.
        if towns_of(e).any(|x| x == s) || e.location == Some(t.location) { return true; }
        if e.factions_involved.contains(&t.faction) && (fame >= Fame::Notable || age <= fame.memory()) { return true; }
        if age > fame.memory() { return false; }
        let origins = self.origins(e);
        if origins.is_empty() { return false; }
        let d = self.distance(s, t.location, t.faction, &origins, e.date.year);
        let jitter = 0.7 + 0.6 * unit(hash(e.id.0 as u64, 0x4B10 ^ ((s.0 as u64) << 20)));
        let fade = if fame == Fame::Legend { 1.0 } else { 1.0 - 0.5 * age as f32 / fame.memory() as f32 };
        d <= fame.reach() * fade * jitter && d <= SPEED * (age as f32 + 1.0)
    }

    /// Whether people `f` knows of `e` (any of its living towns does, or it took part).
    pub fn people_knows(&self, f: FactionId, e: &Event) -> bool {
        if e.factions_involved.contains(&f) { return true; }
        self.kin.get(&f).map_or(false, |ks| ks.iter().any(|s| self.town_knows(*s, e)))
    }

    /// Whether figure `fig` knows of `e`: their own deeds, else what their home (else their
    /// people) knows.
    pub fn figure_knows(&self, fig: FigureId, e: &Event) -> bool {
        if figures_of(e).any(|f| f == fig) { return true; }
        let home = self.h.people.as_ref().and_then(|p| p.home.get(&fig)).copied();
        match home {
            Some(s) => self.town_knows(s, e),
            None => self.h.figures.get(&fig).and_then(|x| x.faction).map_or(false, |f| self.people_knows(f, e)),
        }
    }

    /// Living towns that know `e`, of all living towns.
    pub fn known_in(&self, e: &Event) -> (usize, usize) {
        let mut all: Vec<SettlementId> = self.kin.values().flatten().copied().collect();
        all.sort();
        (all.iter().filter(|s| self.town_knows(**s, e)).count(), all.len())
    }

    /// News: the `n` events of the last `years` that `knows` most worth telling (of notable fame
    /// or more; the more famous, the newer and the more the teller's own, the better), one per
    /// headline, told by `teller`, newest first.
    fn news(&self, knows: impl Fn(&Event) -> bool, teller: Option<FactionId>, years: u32, n: usize) -> Vec<Told> {
        let now = self.h.current_date.year;
        let mut seen: Vec<&str> = Vec::new();
        let mut cands: Vec<(i64, &Event)> = Vec::new();
        for e in self.h.chronicle.events.iter().rev() {
            if e.date.year + years < now { break; }
            let f = fame(e);
            if f < Fame::Notable || seen.contains(&e.title.as_str()) || !knows(e) { continue; }
            seen.push(&e.title);
            let own = teller.map_or(false, |t| e.factions_involved.contains(&t));
            let score = f as i64 * 20 - (now - e.date.year) as i64 + if own { 15 } else { 0 };
            cands.push((score, e));
        }
        // Best first; among equals the newer (the scan was newest first and the sort is stable).
        cands.sort_by_key(|c| std::cmp::Reverse(c.0));
        cands.truncate(n);
        cands.sort_by_key(|c| std::cmp::Reverse((c.1.date, c.1.id)));
        cands.into_iter().map(|(_, e)| account(self.h, e, teller)).collect()
    }

    /// What town `s` has heard of the last `years` (up to `n`), as its people tell it.
    pub fn news_of_town(&self, s: SettlementId, years: u32, n: usize) -> Vec<Told> {
        let f = self.h.settlements.get(&s).map(|t| t.faction);
        self.news(|e| self.town_knows(s, e), f, years, n)
    }

    /// What people `f` has heard of, as it tells it.
    pub fn news_of_people(&self, f: FactionId, years: u32, n: usize) -> Vec<Told> {
        self.news(|e| self.people_knows(f, e), Some(f), years, n)
    }

    /// What figure `fig` knows, as their people tell it.
    pub fn news_of_figure(&self, fig: FigureId, years: u32, n: usize) -> Vec<Told> {
        let f = self.h.figures.get(&fig).and_then(|x| x.faction);
        self.news(|e| self.figure_knows(fig, e), f, years, n)
    }

    /// The deeds of people `f` that it tells its own way (proud or bitter), newest first: what
    /// its bards sing.
    pub fn sung_by(&self, f: FactionId, years: u32, n: usize) -> Vec<Told> {
        let now = self.h.current_date.year;
        self.h.chronicle.events.iter().rev().take_while(|e| e.date.year + years >= now)
            .filter(|e| e.factions_involved.contains(&f) && fame(e) >= Fame::Notable)
            .map(|e| account(self.h, e, Some(f))).filter(|t| t.slant != Slant::Plain)
            .take(n).collect()
    }
}

fn dist(a: (usize, usize), b: (usize, usize), w: usize) -> f32 {
    let dx = a.0.abs_diff(b.0);
    let dx = dx.min(w.saturating_sub(dx)) as f32;
    let dy = a.1.abs_diff(b.1) as f32;
    (dx * dx + dy * dy).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn battle_losses_are_read() {
        assert_eq!(battle_losses("A of B beat C of D in the Battle of X (17 of 143 fell, 41 of 1018). The walls held."), Some((17, 41)));
        assert_eq!(battle_losses("no numbers"), None);
    }

    #[test]
    fn plural_names_tell() {
        assert_eq!(tell_verb("the Git Clans"), "tell");
        assert_eq!(tell_verb("the Ashpit Horde"), "tells");
    }
}
