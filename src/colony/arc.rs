//! The first arc: the world reaches the colony.
//!
//! One complete story through every layer, from the history to the camp: on day 3 a trader brings
//! a rumour of the nearest real threat (a living beast whose lair is near, the Shadow's raiders, or
//! the war band of the people who took the nearest fallen town), caused by that threat's last deed
//! in the chronicle; on day 6 two survivors of a real fallen town reach the camp and stay; for the
//! nights between, one settler keeps watch (or no one does, if they are starving); on the night of
//! day 14 the raid comes, and how ready the camp is (the watch kept, the hut standing, the numbers)
//! decides whether someone dies or is saved. Each step is an `ArcEvent` caused by the one before,
//! shown as a banner, written to the log, and told afterwards as a tale (`tale`). Everything is
//! hashed from the seed: the same world code tells the same arc.

use crate::history::*;
use crate::history::events::types::EventType;
use crate::history::world_state::WorldHistory;
use super::{Colony, ColonyMark, MarkKind, TICKS_PER_DAY};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThreatKind { Beast, Shadow, Warband, Outlaws, Envoy, Deep }

#[derive(Clone, Debug)]
pub struct Threat {
    pub kind: ThreatKind,
    /// "Morfang the Terrible", "raiders of the Shadow of Skullfang", "a war band of The Git Clans".
    pub name: String,
    /// Why it is near, in the chronicle's words.
    pub why: String,
    pub cause: Option<EventId>,
    /// The cause in the chronicle's words ("Kronix the Unending raided Ripu in 447"), for the
    /// rumour's "because" (which must not repeat the rumour).
    pub cause_text: Option<String>,
    /// The people behind it (a war band's, the Shadow's), for settlers who hate them.
    pub faction: Option<FactionId>,
    /// The world tile it comes from (a lair, the Shadow's seat, a taken town): the raid enters
    /// the map from that side.
    pub from: Option<(usize, usize)>,
    /// How big it is (a beast's size; 1 for raiders).
    pub size: f32,
    /// What a beast is (`monsters::of_legend`): its body, its special attack, its description.
    pub monster: Option<crate::monsters::Monster>,
}

#[derive(Clone, Debug)]
pub struct ArcEvent {
    pub day: u64,
    pub title: String,
    pub text: String,
    /// Why it happened: the step before, or the world event behind the first.
    pub because: String,
    pub world_cause: Option<EventId>,
}

#[derive(Clone, Debug)]
pub struct Arc {
    pub threat: Threat,
    /// The fallen town the refugees come from: (name, the fall, year).
    pub fallen: Option<(String, EventId, u32)>,
    /// Whether that town had fallen before (so its survivors' "fallen in" names the latest).
    fell_before: bool,
    refugees: Vec<(String, crate::history::settlers::Past)>,
    pub events: Vec<ArcEvent>,
    pub(crate) stage: u8,
    /// Nights someone kept watch before the raid.
    pub watches: u32,
    last_watch_day: u64,
    seed: u64,
    pub rumour_day: u64,
    pub refugee_day: u64,
    pub raid_day: u64,
    /// Where the refugees camp if the patron turned them away.
    pub turned_away: Option<super::nav::Pos>,
    /// Nights a veteran kept the watch (they count for more).
    pub veteran_watches: u32,
    /// The troubles still to come, nearest thread first (`plan`): each becomes a chapter after
    /// 10-20 quiet days.
    pub later: Vec<Threat>,
    /// Which chapter this is (0 = the first arc, with the refugees).
    pub chapter: u32,
    pub(crate) quiet_until: Option<u64>,
    /// Further real threats, farther off (beasts within 12 tiles, peoples with grudges, other
    /// bands), drawn in one at a time when the camp's wealth grows (Dwarf Fortress: riches draw
    /// sieges), and the wealth at the last draw.
    pub reserve: Vec<Threat>,
    pub(crate) wealth_drawn: u32,
    /// The troubles that can come again once all the rest are spent: the roads' outlaws, the
    /// Shadow's raiders (DF: a fortress that grows rich is never left alone for long).
    pub(crate) recurring: Vec<Threat>,
}

/// Days of the beats, drawn per colony in `plan` (rumour 2-5, refugees 5-9, raid 11-18, sooner
/// when the threat is near); kept on the `Arc`.

fn hash(seed: u64, salt: u64) -> u64 {
    let mut x = seed ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 31)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^ (x >> 29)
}

fn dist(a: (usize, usize), b: (usize, usize), w: usize) -> usize {
    let dx = a.0.abs_diff(b.0);
    dx.min(w.saturating_sub(dx)) + a.1.abs_diff(b.1)
}

/// The outlaws near `tile`: the nearest band of exiles within ten tiles (`history/bands.rs`),
/// with its leader, its hideout and the fall that made it; else nameless robbers.
fn outlaws(h: &WorldHistory, tile: (usize, usize), w: usize) -> Threat {
    let near = crate::history::bands::of(h).into_iter().map(|b| (dist(b.hideout, tile, w), b)).filter(|(d, _)| *d <= 10).min_by_key(|(d, b)| (*d, b.leader));
    match near {
        Some((_, b)) => Threat {
            kind: ThreatKind::Outlaws, name: b.title(h),
            why: format!("they went into exile when {} fell, and live by raiding from the hills at {},{}", b.town, b.hideout.0, b.hideout.1),
            cause: b.cause, cause_text: b.cause.and_then(|c| h.chronicle.get(c)).map(|e| format!("{} in {}", e.title, e.date.year)),
            faction: b.people, from: Some(b.hideout), size: 1.0, monster: None,
        },
        None => Threat { kind: ThreatKind::Outlaws, name: "a band of outlaws".into(), why: "the roads are lawless".into(), cause: None, cause_text: Some("the roads have been lawless since the last war".into()), faction: None, from: None, size: 1.0, monster: None },
    }
}

/// Plan the arc for a colony at world `tile`: the threat, and the refugees' town.
pub fn plan(h: &WorldHistory, tile: (usize, usize), seed: u64) -> Arc {
    use crate::history::naming::styles::NamingStyle;
    use crate::history::naming::generator::NameGenerator;
    use rand::SeedableRng;
    let w = h.tile_history.width.max(1);
    let now = h.current_date.year;
    let fname = |f: FactionId| h.factions.get(&f).map(|x| x.name.clone()).unwrap_or_default();
    // A war band is led by a real warrior of its people: the living one with the most kills.
    let band = |f: FactionId| {
        let leader = h.figures.values().filter(|x| x.is_alive() && x.faction == Some(f) && !x.kills.is_empty())
            .max_by_key(|x| (x.kills.len(), std::cmp::Reverse(x.id))).map(|x| x.full_name());
        match leader { Some(l) => format!("a war band of {}, led by {}", fname(f), l), None => format!("a war band of {}", fname(f)) }
    };
    // The nearest fall in living memory.
    let fall = h.chronicle.events.iter()
        .filter(|e| e.date.year + 60 >= now && matches!(e.event_type, EventType::SiegeEnded | EventType::ShadowConquest | EventType::SettlementDestroyed))
        .filter_map(|e| e.primary_participants.iter().find_map(|p| if let EntityId::Settlement(s) = p { h.settlements.get(s) } else { None }).map(|t| (dist(t.location, tile, w), e, t)))
        // The nearest town's latest fall (the settlers' roster tells the same one).
        .min_by_key(|(d, e, _)| (*d, std::cmp::Reverse(e.id)));
    // The threat: a beast laired near, else the Shadow if its reach is near, else the takers of
    // the nearest fallen town.
    let beast = h.legendary_creatures.values().filter(|c| c.is_alive())
        .filter_map(|c| c.lair_location.map(|l| (dist(l, tile, w), c)))
        .filter(|(d, _)| *d <= 4)
        .min_by_key(|(d, c)| (*d, c.id));
    let shadow_near = h.shadow.as_ref().filter(|s| !s.is_broken())
        .filter(|s| (0..=6i64).any(|r| { let (x, y) = (tile.0 as i64, tile.1 as i64); [(r, 0), (-r, 0), (0, r), (0, -r)].iter().any(|&(dx, dy)| {
            let yy = (y + dy).clamp(0, s.height as i64 - 1) as usize;
            let xx = (x + dx).rem_euclid(s.width as i64) as usize;
            s.at(xx, yy) >= crate::history::shadow::REACH }) }));
    let threat = if let Some((d, c)) = beast {
        let last = h.chronicle.last_of(EntityId::LegendaryCreature(c.id));
        let km = d as f32 * 40_075.0 / w as f32;
        let ev = last.and_then(|e| h.chronicle.get(e)).map(|e| format!("; its last deed: {} ({})", e.title.replacen("The ", "the ", 1), e.date.year)).unwrap_or_default();
        // Walkers take 25 km a day; a beast on the hunt covers ~150 km a night, so it can be at the
        // camp before the raid's night however far its lair.
        let walk = (km / 25.0).ceil().max(1.0);
        let hunt = (km / 150.0).ceil().max(1.0);
        let far = if walk as u64 > 9 { format!("its lair is {} days' walk from here, {} nights' hunting for it", walk, hunt) }
            else { format!("its lair is {} days' walk from here", walk) };
        Threat { kind: ThreatKind::Beast, name: c.full_name(), why: format!("{}{}", far, ev), cause: last, cause_text: None, faction: None, from: c.lair_location, size: c.size_multiplier.clamp(0.8, 3.0), monster: Some(crate::monsters::of_legend(h, c)) }
    } else if let Some(s) = shadow_near {
        Threat { kind: ThreatKind::Shadow, name: format!("raiders of {}", s.name), why: format!("{} reaches this far", s.name), cause: Some(s.last_deed), cause_text: None, faction: Some(s.faction), from: Some(s.seat), size: 1.0, monster: None }
    } else if let Some((_, e, t)) = fall.filter(|(_, e, _)| e.factions_involved.len() >= 2) {
        let taker = e.factions_involved[0];
        Threat { kind: ThreatKind::Warband, name: band(taker), why: format!("they took {} in {}", t.name, e.date.year), cause: Some(e.id), cause_text: None, faction: Some(taker), from: Some(t.location), size: 1.0, monster: None }
    } else {
        outlaws(h, tile, w)
    };
    let mut threat = threat;
    if threat.cause_text.is_none() {
        // A beast: where it made its lair (the rumour already names its last deed); else the deed.
        let lair = if threat.kind == ThreatKind::Beast {
            beast.and_then(|(_, c)| h.chronicle.events.iter().rev().find(|e| e.event_type == EventType::LairEstablished
                && e.primary_participants.contains(&EntityId::LegendaryCreature(c.id))))
                .map(|e| format!("{} made its lair within reach in {} ({})", threat.name, e.date.year, e.title.replacen("The ", "the ", 1)))
                .or_else(|| Some(format!("no one has slain {}, and its lair lies within its night's hunting of the camp", threat.name)))
        } else { None };
        threat.cause_text = lair.or_else(|| threat.cause.and_then(|c| h.chronicle.get(c)).map(|e| format!("{} in {}", e.title.replacen("The ", "the ", 1), e.date.year)));
    }
    // Two refugees from the fallen town, in its people's tongue.
    let mut refugees = Vec::new();
    let fallen = fall.map(|(_, e, t)| (t.name.clone(), e.id, e.date.year));
    if let Some((_, e, t)) = fall {
        let people = e.factions_involved.get(1).copied().or(Some(t.faction));
        let arche = people.and_then(|f| h.factions.get(&f)).and_then(|f| h.races.get(&f.race_id))
            .map(|r| r.base_type.default_naming_archetype()).unwrap_or(crate::history::naming::styles::NamingArchetype::Compound);
        let style = NamingStyle::from_archetype(NamingStyleId(0), arche);
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed ^ 0xAE7E);
        for k in 0..2u64 {
            let name = NameGenerator::personal_name(&style, &mut rng);
            let age = 16 + (hash(seed, 40 + k) % 30) as u32;
            let past = crate::history::settlers::Past {
                age, people, calling: format!("a refugee from {}", t.name),
                lines: vec![(format!("Fled {} when it {} in {}, and has walked the roads since.", t.name,
                    if e.title.contains("burned") || e.event_type == EventType::SettlementDestroyed { "burned" } else { "fell" }, e.date.year), Some(e.id))],
                feeling: e.factions_involved.first().copied().filter(|f| Some(*f) != people).map(|f| (format!("hates {}", fname(f)), EntityId::Faction(f))),
                persona: None,
                arts: Vec::new(),
                instrument: None,
                images: Vec::new(),
                towns: Vec::new(),
                faith: None,
            };
            let mut past = past;
            past.persona = Some(crate::history::settlers::persona_for(h, &name, &past, seed));
            past.arts = crate::history::settlers::arts_of(h, past.people);
            crate::history::settlers::fill_craft(h, &mut past);
            refugees.push((name, past));
        }
    }
    // The troubles after the first: threads of the present day near this camp, one of each
    // kind, none the same people or beast as the first.
    let mut later: Vec<Threat> = Vec::new();
    let home_people = h.settlements.values().filter(|t| !t.is_destroyed()).min_by_key(|t| (dist(t.location, tile, w), t.id)).map(|t| t.faction);
    let taken = |later: &Vec<Threat>, f: Option<FactionId>| f.is_some() && (threat.faction == f || later.iter().any(|t| t.faction == f));
    if let Some(p) = home_people {
        // A war their people are fighting: the enemy's war band.
        let mut wars: Vec<_> = h.wars.values().filter(|w| w.ended.is_none() && (w.aggressors.contains(&p) || w.defenders.contains(&p))).collect();
        wars.sort_by_key(|w| w.id);
        if let Some(wr) = wars.first() {
            let enemy = if wr.aggressors.contains(&p) { wr.defenders.first() } else { wr.aggressors.first() }.copied();
            if let Some(e) = enemy.filter(|e| !taken(&later, Some(*e))) {
                later.push(Threat { kind: ThreatKind::Warband, name: band(e), why: format!("{} is at war with {} ({})", fname(e), fname(p), wr.name),
                    cause: wr.declaration_event, cause_text: Some(format!("{} began in {}", wr.name, wr.started.year)), faction: Some(e),
                    from: h.settlements.values().filter(|t| t.faction == e && !t.is_destroyed()).min_by_key(|t| (dist(t.location, tile, w), t.id)).map(|t| t.location), size: 1.0, monster: None });
            }
        }
        // A people with a grudge against theirs: an envoy asks tribute.
        let mut grudges: Vec<(i32, FactionId)> = h.factions.values().filter(|f| f.is_active() && f.id != p)
            .filter_map(|f| f.relations.get(&p).filter(|r| !r.stance.is_at_war() && r.opinion <= crate::history::present::GRUDGE_OPINION).map(|r| (r.opinion, f.id))).collect();
        grudges.sort();
        if let Some(&(_, g)) = grudges.iter().find(|(_, g)| !taken(&later, Some(*g))) {
            let last_war = h.factions.get(&g).and_then(|f| f.relations.get(&p)).and_then(|r| r.last_war).and_then(|wid| h.wars.get(&wid));
            later.push(Threat { kind: ThreatKind::Envoy, name: format!("an envoy of {}", fname(g)),
                why: format!("{} bear {} a grudge{}", fname(g), fname(p), last_war.map(|wr| format!(" from {}", wr.name)).unwrap_or_default()),
                cause: last_war.and_then(|wr| wr.declaration_event), cause_text: last_war.map(|wr| format!("{} ({}-{})", wr.name, wr.started.year, wr.ended.map(|d| d.year).unwrap_or(now))), faction: Some(g),
                from: h.settlements.values().filter(|t| t.faction == g && !t.is_destroyed()).min_by_key(|t| (dist(t.location, tile, w), t.id)).map(|t| t.location), size: 1.0, monster: None });
        }
    }
    // Another beast within ten tiles.
    let first_beast = beast.map(|(_, c)| c.id);
    if let Some((d, c)) = h.legendary_creatures.values().filter(|c| c.is_alive() && Some(c.id) != first_beast)
        .filter_map(|c| c.lair_location.map(|l| (dist(l, tile, w), c))).filter(|(d, _)| *d <= 10).min_by_key(|(d, c)| (*d, c.id)) {
        let last = h.chronicle.last_of(EntityId::LegendaryCreature(c.id));
        let km = d as f32 * 40_075.0 / w as f32;
        later.push(Threat { kind: ThreatKind::Beast, name: c.full_name(), why: format!("its lair is {} days' walk from here, {} nights' hunting for it", (km / 25.0).ceil().max(1.0), (km / 150.0).ceil().max(1.0)),
            cause: last, cause_text: last.and_then(|e| h.chronicle.get(e)).map(|e| format!("{} in {}", e.title.replacen("The ", "the ", 1), e.date.year)), faction: None,
            from: c.lair_location, size: c.size_multiplier.clamp(0.8, 3.0), monster: Some(crate::monsters::of_legend(h, c)) });
    }
    // The Shadow, if it reaches within ten tiles and was not the first.
    if threat.kind != ThreatKind::Shadow {
        if let Some(sh) = h.shadow.as_ref().filter(|s| !s.is_broken()).filter(|s| (0..=10i64).any(|r| { let (x, y) = (tile.0 as i64, tile.1 as i64); [(r, 0), (-r, 0), (0, r), (0, -r)].iter().any(|&(dx, dy)| {
            let yy = (y + dy).clamp(0, s.height as i64 - 1) as usize; let xx = (x + dx).rem_euclid(s.width as i64) as usize; s.at(xx, yy) >= crate::history::shadow::REACH }) })) {
            later.push(Threat { kind: ThreatKind::Shadow, name: format!("raiders of {}", sh.name), why: format!("{} reaches this far", sh.name), cause: Some(sh.last_deed),
                cause_text: h.chronicle.get(sh.last_deed).map(|e| format!("{} in {}", e.title.replacen("The ", "the ", 1), e.date.year)), faction: Some(sh.faction), from: Some(sh.seat), size: 1.0, monster: None });
        }
    }
    if later.is_empty() || threat.kind != ThreatKind::Outlaws {
        later.push(outlaws(h, tile, w));
    }
    let near = match (&threat.kind, beast) { (ThreatKind::Beast, Some((d, _))) => (4 - d.min(4)) as u64 / 2, (ThreatKind::Shadow, _) => 1, _ => 0 };
    let rumour_day = 2 + hash(seed, 0xBEA7) % 4;
    let refugee_day = (5 + hash(seed, 0xBEA8) % 5).max(rumour_day + 2);
    let raid_day = (11 + hash(seed, 0xBEA9) % 8).saturating_sub(near).max(refugee_day + 4);
    let fell_before = fall.map_or(false, |(_, e, t)| h.chronicle.events.iter().any(|x| x.id < e.id
        && matches!(x.event_type, EventType::SiegeEnded | EventType::ShadowConquest | EventType::SettlementDestroyed)
        && x.primary_participants.contains(&EntityId::Settlement(t.id))));
    // The reserve: what the camp's riches may draw in later, nearest first.
    let mut reserve: Vec<Threat> = Vec::new();
    let used_beast = |r: &Vec<Threat>, name: &str| threat.name == name || later.iter().chain(r.iter()).any(|t| t.name == name);
    let mut beasts: Vec<(usize, &crate::history::creatures::legendary::LegendaryCreature)> = h.legendary_creatures.values().filter(|c| c.is_alive())
        .filter_map(|c| c.lair_location.map(|l| (dist(l, tile, w), c))).filter(|(d, _)| *d <= 12).collect();
    beasts.sort_by_key(|(d, c)| (*d, c.id));
    for (d, c) in beasts {
        if reserve.len() >= 3 { break; }
        if used_beast(&reserve, &c.full_name()) { continue; }
        let km = d as f32 * 40_075.0 / w as f32;
        let last = h.chronicle.last_of(EntityId::LegendaryCreature(c.id));
        reserve.push(Threat { kind: ThreatKind::Beast, name: c.full_name(), why: format!("its lair is {} days' walk off, but word of the camp's riches has reached it", (km / 25.0).ceil().max(1.0)),
            cause: last, cause_text: last.and_then(|e| h.chronicle.get(e)).map(|e| format!("{} in {}", e.title.replacen("The ", "the ", 1), e.date.year)), faction: None,
            from: c.lair_location, size: c.size_multiplier.clamp(0.8, 3.0), monster: Some(crate::monsters::of_legend(h, c)) });
    }
    if let Some(p) = home_people {
        let mut foes: Vec<(i32, FactionId)> = h.factions.values().filter(|f| f.is_active() && f.id != p)
            .filter_map(|f| f.relations.get(&p).filter(|r| r.opinion < 0).map(|r| (r.opinion, f.id))).collect();
        foes.sort();
        for (_, e) in foes.into_iter().filter(|(_, e)| !taken(&later, Some(*e))).take(2) {
            reserve.push(Threat { kind: ThreatKind::Warband, name: band(e), why: format!("{} think ill of {}, and the camp grows rich", fname(e), fname(p)),
                cause: None, cause_text: Some(format!("{} have no love for {}", fname(e), fname(p))), faction: Some(e),
                from: h.settlements.values().filter(|t| t.faction == e && !t.is_destroyed()).min_by_key(|t| (dist(t.location, tile, w), t.id)).map(|t| t.location), size: 1.0, monster: None });
        }
    }
    let recurring: Vec<Threat> = std::iter::once(&threat).chain(later.iter()).filter(|t| matches!(t.kind, ThreatKind::Outlaws | ThreatKind::Shadow)).cloned().collect();
    Arc { threat, fallen, fell_before, refugees, events: Vec::new(), stage: 0, watches: 0, last_watch_day: 0, seed, rumour_day, refugee_day, raid_day, turned_away: None, veteran_watches: 0, later, chapter: 0, quiet_until: None, reserve, wealth_drawn: 0, recurring }
}

impl Colony {
    fn arc_event(&mut self, title: String, text: String, world_cause: Option<EventId>) {
        let day = self.clock.day();
        let Some(arc) = self.arc.as_mut() else { return };
        let was = if arc.threat.kind == ThreatKind::Beast { "was" } else { "were" };
        let because = match (title.as_str(), arc.chapter) {
            ("A rumour" | "An envoy", _) => match &arc.threat.cause_text { Some(t) => format!("because {}", t), None => format!("because {}", arc.threat.why) },
            ("Refugees", _) => match &arc.fallen { Some((t, _, y)) => format!("because {} fell in {}, and its survivors are still on the roads", t, y), None => "because the danger the trader spoke of is real".into() },
            ("The raid", 0) => format!("because {} {} near, as the trader said, and the refugees had been followed", arc.threat.name, was),
            ("The raid", _) => format!("because {} {} near, as was foretold", arc.threat.name, was),
            _ => format!("because {}", arc.threat.why),
        };
        arc.events.push(ArcEvent { day, title: title.clone(), text: text.clone(), because: because.clone(), world_cause });
        let at = if title == "The raid" { self.clash_at.unwrap_or_else(|| self.spot_from_camp(4, -5)) } else { self.camp };
        self.moment(title.clone(), text.clone(), because, at);
        self.banner = Some((title.clone(), self.clock.tick));
        self.note(format!("{}: {}", title, text));
    }

    /// Advance the arc (called every tick while one is planned).
    pub(crate) fn arc_tick(&mut self) {
        let Some(arc) = self.arc.as_ref() else { return };
        let (day, hour, stage) = (self.clock.day(), self.clock.hour(), arc.stage);
        let (rumour_day, refugee_day, raid_day) = (arc.rumour_day, arc.refugee_day, arc.raid_day);
        match stage {
            0 if day >= rumour_day && hour >= 10 && arc.threat.kind == ThreatKind::Envoy => {
                // An envoy asks tribute: paid if the store can bear it, else refused, and they will
                // come back with spears.
                let t = arc.threat.clone();
                let ask = 2 * self.alive() as u32;
                let food = self.food_stored();
                self.arc_event("An envoy".into(), format!("{} comes to the camp: {}. They ask {} meals in tribute.", capital(&t.name), t.why, ask), t.cause);
                let people = t.name.trim_start_matches("an envoy of ").to_string();
                if food >= ask {
                    for _ in 0..ask {
                        if let Some(k) = self.items.iter().rposition(|it| it.kind == super::ItemKind::Food && it.stored && !it.reserved) { self.items.remove(k); self.fix_refs_pub(k); }
                    }
                    let at = self.spot_from_camp(-6, 0);
                    self.marks.push(ColonyMark { at, kind: MarkKind::Stone, title: "The tribute stone".into(),
                        text: format!("Where {} meals were paid to the envoy of {} on day {}, and the peace was kept.", ask, people, day), day });
                    self.note(format!("They pay the tribute: {} meals leave the store. The envoy goes away satisfied, for now.", ask));
                    self.regard(t.faction, &people, t.from, "tribute", 5, format!("paid them {} meals in tribute on day {}", ask, day));
                    let a = self.arc.as_mut().unwrap();
                    a.stage = 3;
                } else {
                    self.regard(t.faction, &people, t.from, "tribute", -5, format!("would not pay them tribute on day {}", day));
                    self.note(format!("They have only {} meals; they cannot pay. The envoy leaves, promising to come back with spears.", food));
                    let a = self.arc.as_mut().unwrap();
                    a.threat.kind = ThreatKind::Warband;
                    a.threat.name = format!("a war band of {}", people);
                    a.refugee_day = day;
                    a.raid_day = day + 6;
                    a.stage = 2;
                }
            }
            // Riches draw trouble: with no thread left, a quiet of 25 days and ten works (or an
            // artifact) made since the last draw, the next of the reserve is drawn in.
            3 if arc.later.is_empty() && !arc.reserve.is_empty() && self.alive() > 0 && self.departed.is_none() && hour == 7 => {
                let wealth = self.works.len() as u32 + 10 * self.works.iter().filter(|w| w.quality >= 5).count() as u32 + 10 * self.treasures.len() as u32
                    + if self.relic.as_ref().map_or(false, |r| r.found.is_some() && r.fate.is_none()) { 10 } else { 0 };
                let since = arc.events.last().map_or(0, |e| e.day);
                if wealth >= arc.wealth_drawn + 10 && day >= since + 25 {
                    let a = self.arc.as_mut().unwrap();
                    let next = a.reserve.remove(0);
                    a.wealth_drawn = wealth;
                    a.later.push(next);
                    a.quiet_until = Some(day + 3);
                }
            }
            // All spent: the roads' outlaws or the Shadow's raiders come again, sixty quiet days
            // on, when the camp has grown richer since the last draw.
            3 if arc.later.is_empty() && arc.reserve.is_empty() && !arc.recurring.is_empty() && self.alive() > 0 && self.departed.is_none() && hour == 7 => {
                let wealth = self.works.len() as u32 + 10 * self.works.iter().filter(|w| w.quality >= 5).count() as u32 + 10 * self.treasures.len() as u32;
                let since = arc.events.last().map_or(0, |e| e.day);
                if wealth >= arc.wealth_drawn + 10 && day >= since + 60 {
                    let a = self.arc.as_mut().unwrap();
                    let k = (hash(a.seed, day) % a.recurring.len() as u64) as usize;
                    let mut t = a.recurring[k].clone();
                    t.why = if t.kind == ThreatKind::Shadow { format!("{} reaches this far still, and the camp has grown rich", t.name.trim_start_matches("raiders of ")) } else { "word of the camp's riches has spread along the roads".to_string() };
                    a.wealth_drawn = wealth;
                    a.later.push(t);
                    a.quiet_until = Some(day + 3);
                }
            }
            3 if !arc.later.is_empty() && self.alive() > 0 && self.departed.is_none() => {
                // Between troubles: 10-20 quiet days, then the next thread of the world.
                let a = self.arc.as_mut().unwrap();
                match a.quiet_until {
                    None => a.quiet_until = Some(day + 10 + hash(a.seed ^ a.chapter as u64, 0x9017) % 11),
                    Some(q) if day >= q && hour >= 7 => {
                        let next = a.later.remove(0);
                        a.chapter += 1;
                        a.threat = next;
                        a.rumour_day = day;
                        a.refugee_day = day;
                        a.raid_day = day + 5 + hash(a.seed ^ a.chapter as u64, 0x9018) % 4;
                        a.watches = 0;
                        a.veteran_watches = 0;
                        a.last_watch_day = 0;
                        a.turned_away = None;
                        a.quiet_until = None;
                        a.stage = 0;
                    }
                    _ => {}
                }
            }
            0 if day >= rumour_day && hour >= 10 => {
                let t = arc.threat.clone();
                let what = match t.kind {
                    ThreatKind::Beast => match &t.monster {
                        // What it is, in the words of those who saw it (`monsters.rs`).
                        Some(m) => format!("{} is stirring; {}. Those who have seen it say it is {}. {}", t.name, t.why, m.short, m.attack.as_ref().map(|a| a.warning.clone()).unwrap_or_default()).trim_end().to_string(),
                        None => format!("{} is stirring; {}.", t.name, t.why),
                    },
                    ThreatKind::Shadow => format!("{}; its raiders have been seen on the roads.", t.why),
                    ThreatKind::Warband => format!("{}{} is roaming the hills: {}.", t.name, if t.name.contains(", led by ") { "," } else { "" }, t.why),
                    ThreatKind::Outlaws if t.name.starts_with("the Exiles") => format!("{}{} are robbing travellers on the roads: {}.", t.name, if t.name.contains(", led by ") { "," } else { "" }, t.why),
                    ThreatKind::Outlaws => "outlaws are robbing travellers on the roads.".to_string(),
                    ThreatKind::Deep => String::new(),
                    ThreatKind::Envoy => format!("{} is on the road.", t.name),
                };
                self.arc.as_mut().unwrap().stage = 1;
                if t.kind == ThreatKind::Deep {
                    // Foretold by the miners, not a trader: the deep has gone quiet, then not quiet.
                    let life = self.map.caverns.first().and_then(|c| c.life.first().cloned()).unwrap_or_else(|| "cave crickets".into());
                    let warn = t.monster.as_ref().and_then(|m| m.attack.as_ref()).map(|a| format!(" The old stories of the deep say: {}", a.warning)).unwrap_or_default();
                    self.arc_event("A sound from below".into(), format!("The miners hear something vast moving far below the mine, and the {} of the cavern have gone silent.{}", life, warn), None);
                } else {
                    self.arc_event("A rumour".into(), format!("A trader passing the camp says {}", what), t.cause);
                }
                // Those who hate the threat's people take it to heart.
                let haters = self.haters_of_threat();
                if let Some(&k) = haters.first() {
                    let (name, why) = (self.settlers[k].name.clone(), self.settlers[k].past.as_ref().and_then(|p| p.feeling.as_ref()).map(|f| they_form(&f.0)).unwrap_or_default());
                    let others = match haters.len() - 1 { 0 => String::new(), 1 => " So does one other.".into(), n => format!(" So do {} others.", n) };
                    self.note(format!("{} hears the name and goes quiet: they {}. They will not run this time.{}", name, why, others));
                }
            }
            1 if arc.chapter > 0 => {
                // Later chapters have no refugees: straight to the watch.
                self.arc.as_mut().unwrap().stage = 2;
            }
            1 if day >= refugee_day && hour >= 16 && self.alive() == 0 => {
                // Nobody is left to take them in: they find graves and walk on.
                self.arc.as_mut().unwrap().stage = 3;
            }
            1 if day >= refugee_day && hour >= 16 => {
                let a = self.arc.as_mut().unwrap();
                match (a.fallen.clone(), a.refugees.is_empty()) {
                    (Some((town, fall, year)), false) => {
                        // They wait at the camp's edge: the patron takes them in or turns them away
                        // (unanswered, the camp takes them in at dawn).
                        a.stage = 5;
                        let names: Vec<String> = a.refugees.iter().map(|r| r.0.clone()).collect();
                        let again = if a.fell_before { ", not its first fall" } else { "" };
                        let text = format!("{} and {}, survivors of {} (fallen in {}{}), wait at the edge of the camp. They say the same danger is coming. Two more mouths and two more hands; take them in, or turn them away?",
                            names[0], names[1], town, year, again);
                        self.arc_event("Refugees".into(), text, Some(fall));
                        if let Some(m) = self.moments.last_mut() { m.choice = true; }
                    }
                    _ => {
                        a.stage = 2;
                        self.arc_event("Refugees".into(), "No one comes; the roads are empty, which frightens them more.".into(), None);
                    }
                }
            }
            5 if day > refugee_day && hour >= 6 => {
                // Nobody answered: the camp takes them in.
                let _ = self.resolve_refugees(true, false);
            }
            2 => {
                // The watch: from the refugees' coming until the raid, someone stays up every
                // other night; every night if the patron blessed the watch post, never if it is
                // forbidden ground, and a settler who dreamt of the watch keeps it that night.
                if hour == 21 && self.clock.minute() == 0 && day > arc.last_watch_day && day < raid_day {
                    self.arc.as_mut().unwrap().last_watch_day = day;
                    self.set_watch(day);
                }
                if day + 1 == raid_day && hour == 21 && self.clock.minute() == 1 {
                    let (_, tally, _) = self.readiness();
                    self.note(format!("On the eve of the raid: {}.", tally));
                }
                if hour == 6 { self.watcher = None; }
                // The attackers set out on the eve, an hour before midnight, and the raid is
                // fought where they meet the camp (by dawn at the latest).
                if day + 1 == raid_day && hour == 19 && self.clock.minute() == 0 && !self.attackers_out() && !self.siege_begins() { self.send_attackers(); }
                if self.attackers_out() {
                    if let Some(at) = self.attackers_clash() {
                        let spawned = self.creatures.iter().filter(|c| c.kind != super::creatures::CreatureKind::Wolf).map(|c| c.spawned).min().unwrap_or(self.clock.tick);
                        self.raid_watch.push((spawned, self.clock.tick));
                        self.raid_at(Some(at));
                        self.attackers_retreat();
                    }
                }
                if self.arc.as_ref().map_or(false, |a| a.stage == 2) && day >= raid_day && hour >= 5 && hour < 6 {
                    self.raid_at(None);
                    self.attackers_retreat();
                }
            }
            _ => {}
        }
    }

    pub(crate) fn is_veteran(&self, k: usize) -> bool {
        // A veteran of their people's battles, or one who came home from the camp's war
        // (`warcall.rs`).
        self.settlers[k].past.as_ref().map_or(false, |p| p.calling.starts_with("a veteran"))
            || self.settlers[k].deeds.iter().any(|d| d.starts_with("fought in "))
    }

    /// "the Battle of Elderpyramid Wood": where a veteran fought ("Fought at X in Y under Z.").
    pub(crate) fn battle_of(&self, k: usize) -> Option<String> {
        let line = self.settlers[k].past.as_ref()?.lines.iter().find(|(t, _)| t.starts_with("Fought at "))?.0.clone();
        let rest = line.trim_start_matches("Fought at ");
        // Cut at " in <year>" (battle names can hold " in ": "the Battle in the Woods of X").
        let cut = rest.match_indices(" in ").find(|(i, _)| rest[i + 4..].starts_with(|c: char| c.is_ascii_digit())).map(|(i, _)| i).unwrap_or(rest.len());
        Some(rest[..cut].to_string())
    }

    /// Living settlers who hate the people behind the arc's threat.
    pub(crate) fn haters_of_threat(&self) -> Vec<usize> {
        let Some(f) = self.arc.as_ref().and_then(|a| a.threat.faction) else { return Vec::new() };
        (0..self.settlers.len()).filter(|&k| self.settlers[k].alive
            && self.settlers[k].past.as_ref().and_then(|p| p.feeling.as_ref()).map_or(false, |x| (x.0.starts_with("hates") || x.0.starts_with("has not forgiven")) && x.1 == EntityId::Faction(f))).collect()
    }

    /// Whether refugees wait at the edge for the patron's answer.
    pub fn refugees_waiting(&self) -> bool { self.arc.as_ref().map_or(false, |a| a.stage == 5) }

    /// The patron's answer to the refugees at the edge (a recorded intervention).
    pub fn answer_refugees(&mut self, take: bool) -> Result<String, String> {
        if !self.refugees_waiting() { return Err("no one waits at the edge of the camp".into()); }
        self.interventions.push(format!("{} refugees {}", self.clock.tick, if take { "take" } else { "turn" }));
        self.resolve_refugees(take, true)
    }

    fn resolve_refugees(&mut self, take: bool, patron: bool) -> Result<String, String> {
        let yours = if patron { " (your doing)" } else { "" };
        let a = self.arc.as_mut().unwrap();
        a.stage = 2;
        let refugees = std::mem::take(&mut a.refugees);
        let (town, fall) = a.fallen.clone().map(|(t, e, _)| (t, Some(e))).unwrap_or_default();
        let names: Vec<String> = refugees.iter().map(|r| r.0.clone()).collect();
        if take {
            // They were followed: the danger comes two days sooner.
            a.raid_day = a.raid_day.saturating_sub(2).max(a.refugee_day + 2);
            for (name, past) in refugees { self.add_settler(name, Some(past)); }
            let line = format!("{} and {} are taken in{}. They were followed: the danger will come sooner.", names[0], names[1], yours);
            self.note(line.clone());
            if let Some(k) = (0..self.settlers.len()).find(|&k| self.settlers[k].alive && !names.contains(&self.settlers[k].name)
                && self.settlers[k].past.as_ref().map_or(false, |p| !town.is_empty() && p.calling.contains(&town))) {
                let who = self.settlers[k].name.clone();
                self.note(format!("{}, also of {}, knows {} from before the fall, and makes room by the fire.", who, town, names[0]));
                for n in &names { if let Some(j) = self.settlers.iter().position(|s| &s.name == n) { self.like(k, j, 4); } }
            }
            let _ = fall;
            Ok(line)
        } else {
            let at = self.spot_from_camp(0, 28);
            self.arc.as_mut().unwrap().turned_away = Some(at);
            self.marks.push(ColonyMark { at, kind: MarkKind::Stone, title: "The refugees' fire".into(),
                text: format!("{} and {}, survivors of {}, camped here after they were turned away on day {}.", names[0], names[1], town, self.clock.day()), day: self.clock.day() });
            let line = format!("{} and {} are turned away{}. They make a fire of their own at the far edge of the clearing.", names[0], names[1], yours);
            self.note(line.clone());
            // Someone from the same fallen town takes it hard.
            if let Some(k) = (0..self.settlers.len()).find(|&k| self.settlers[k].alive && self.settlers[k].past.as_ref().map_or(false, |p| !town.is_empty() && p.calling.contains(&town))) {
                let who = self.settlers[k].name.clone();
                self.note(format!("{}, also of {}, takes it hard: they were our own, and we sent them into the dark.", who, town));
            }
            Ok(line)
        }
    }

    /// The raid, fought at `clash` (where the attackers met the camp) or at the camp.
    pub(crate) fn raid_at(&mut self, clash: Option<super::nav::Pos>) {
        let Some(arc) = self.arc.as_mut() else { return };
        arc.stage = 3;
        let (threat, seed) = (arc.threat.clone(), arc.seed);
        self.watcher = None;
        if !self.milestones_hit.iter().any(|m| m == "the first raid") { self.milestones_hit.push("the first raid".into()); }
                let alive: Vec<usize> = (0..self.settlers.len()).filter(|&i| self.settlers[i].alive).collect();
        // How ready they are: the watch kept, walls and a roof to hold, hands to fight.
        let (ready, tally, patron) = self.readiness();
        let readiness = format!(" ({}{})", tally, patron);
        // How strong the threat is, and the night's luck.
        let strength = match threat.kind { ThreatKind::Deep => 0.85, ThreatKind::Beast => 0.8, ThreatKind::Shadow => 0.7, ThreatKind::Warband | ThreatKind::Envoy => 0.6, ThreatKind::Outlaws => 0.4 };
        let chapter = self.arc.as_ref().map_or(0, |a| a.chapter) as u64;
        let seed = seed ^ chapter.wrapping_mul(0x51_7CC1_B727_220A);
        // A beast's special attack makes it deadlier (fire, poison, a blood-drinker...).
        let attack = threat.monster.as_ref().and_then(|m| m.attack.clone());
        // A bigger band is more dangerous (`band_size`).
        let band = if matches!(threat.kind, ThreatKind::Beast | ThreatKind::Deep) { 0.0 } else { 0.05 * (self.band_size() as f32 - 3.0) };
        let danger = strength + 0.4 * (hash(seed, 0xBA1D) % 1000) as f32 / 1000.0 + attack.as_ref().map_or(0.0, |a| a.deadly) + band;
        // (Debug: PLANET_FORCE_RAID_DEATH=1 makes the first raid take a life, for the vow's test.)
        let danger = if chapter == 0 && std::env::var("PLANET_FORCE_RAID_DEATH").is_ok() { danger.max(ready + 0.5) } else { danger };
        let who = match threat.kind {
            ThreatKind::Beast => threat.name.clone(),
            _ => capital(&threat.name),
        };
        // A sally out of a siege meets them at their own fires at first light (`siege.rs`).
        let sally = self.siege.as_ref().map_or(false, |s| s.sally);
        let who = if clash.is_some() && !self.raid_side.is_empty() && !sally { format!("{}, out of {},", who, self.raid_side) } else { who };
        let came = if sally { if threat.name.starts_with("a ") { "was met at their own fires at first light" } else { "were met at their own fires at first light" } } else { "came in the night" };
        let at = clash.unwrap_or_else(|| self.spot_from_camp(4 + 3 * chapter as i32, -5 + 2 * chapter as i32));
        self.clash_at = Some(at);
        self.clash_tick = self.clock.tick;
        self.marks.push(ColonyMark { at, kind: MarkKind::Scorch, title: "Scorched ground".into(),
            text: format!("Burned in the raid of day {}, when {} came in the night.", self.clock.day(), who), day: self.clock.day() });
        // The refugees turned away were found first.
        if let Some(at) = self.arc.as_ref().and_then(|a| a.turned_away) {
            if hash(seed, 0xCA1E) % 10 < 6 {
                let day = self.clock.day();
                self.marks.push(ColonyMark { at: (at.0 + 1, at.1), kind: MarkKind::Grave, title: "The refugees' cairn".into(),
                    text: format!("Turned away from the camp, they were the first {} found, on the night of day {}.", threat.name, day), day });
                self.note(format!("At first light they find the refugees' fire cold: {} found them first. They raise a cairn.", threat.name));
            } else {
                self.note("The refugees' fire still burns at first light; they had hidden, and they walk on.".into());
            }
        }
        // Exiles of a town some settlers fled: one who knew them talks instead of fighting.
        if threat.kind == ThreatKind::Outlaws {
            if let Some(town) = threat.name.strip_prefix("the Exiles of ").and_then(|r| r.split(", led by ").next()).map(String::from) {
                let leader = threat.name.split(", led by ").nth(1).unwrap_or("their leader").to_string();
                let knower = alive.iter().copied().find(|&i| self.settlers[i].past.as_ref().map_or(false, |p| p.calling.ends_with(&format!(" {}", town))));
                if let Some(k) = knower {
                    let kname = self.settlers[k].name.clone();
                    let take = (self.alive() as u32).min(self.food_stored());
                    for _ in 0..take { if let Some(j) = self.items.iter().rposition(|it| it.kind == super::ItemKind::Food && it.stored && !it.reserved) { self.items.remove(j); self.fix_refs_pub(j); } }
                    self.arc_event("The raid".into(), format!("{} came in the night, but {} knew {} from {} before the fall. They talked by the fire till dawn; the exiles took {} meals and went back to the hills{}.", who, kname, leader, town, take, readiness), None);
                    // The scorch was not made; the knower is the camp's peacemaker tonight.
                    if self.marks.last().map_or(false, |m| m.kind == MarkKind::Scorch) { self.marks.pop(); }
                    self.feel(k, super::mind::Feel::Reconciled { by: format!("old friends from {}", town) });
                    for j in alive.iter().copied().filter(|&j| j != k) { self.like(k, j, 2); }
                    let at = self.settlers[k].pos;
                    self.moment(format!("{} talks with the exiles", kname), format!("{} knew {} from {} before the fall, and talked the exiles out of a fight.", kname, leader, town), format!("because {} is a survivor of {} too", kname, town), at);
                    return;
                }
            }
        }
        // A beast may walk into a cage trap at the gate (`traps.rs`).
        if self.trap_takes(&threat, clash, seed) {
            if self.marks.last().map_or(false, |m| m.kind == MarkKind::Scorch) { self.marks.pop(); }
            self.arc_event("The raid".into(), format!("{} came in the night and was taken in a cage trap{}.", who, readiness), None);
            return;
        }
        // Where they came in, the palisade is broken.
        if let Some(p) = clash { self.break_palisade(p); }
        if alive.is_empty() { return; }
        // Those of the raiders' own people stand aside (`fight.rs`), and are torn.
        self.fighting_people = if matches!(threat.kind, ThreatKind::Warband) { threat.faction } else { None };
        if let Some(f) = self.fighting_people {
            let aside: Vec<usize> = alive.iter().copied().filter(|&i| self.settlers[i].past.as_ref().and_then(|p| p.people) == Some(f)).collect();
            if !aside.is_empty() {
                let names: Vec<String> = aside.iter().map(|&i| self.settlers[i].name.clone()).collect();
                let people = threat.name.trim_start_matches("a war band of ").split(", led by ").next().unwrap_or("").to_string();
                self.note(format!("{} {} aside: {} will not raise a hand against {} own people, {}.", crate::persona::list(&names), if aside.len() == 1 { "stands" } else { "stand" }, if aside.len() == 1 { "they" } else { "they" }, if aside.len() == 1 { "their" } else { "their" }, people));
                for &i in &aside { self.feel(i, super::mind::Feel::Torn { people: people.clone() }); }
            }
        }
        // The one nearest the clash is struck; with no clash, a seeded choice.
        let d = |i: usize, p: super::nav::Pos| (self.settlers[i].pos.0 as i32 - p.0 as i32).abs().max((self.settlers[i].pos.1 as i32 - p.1 as i32).abs());
        let victim = match clash { Some(p) => alive.iter().copied().min_by_key(|&i| (d(i, p), i)).unwrap(), None => alive[(hash(seed, 0x51C7) as usize) % alive.len()] };
        let vname = self.settlers[victim].name.clone();
        // Armour may turn the killing blow (`armour.rs`): struck down, but alive.
        let mail = if danger > ready + 0.3 { self.armour_turns_killing(victim, &super::fight::foe_of(&capital(&threat.name), threat.monster.as_ref()), 0xA4A3 ^ seed) } else { None };
        if danger > ready + 0.3 && mail.is_none() {
            let how = attack.as_ref().map(|a| format!(" It {}; {}", a.did_to(&vname), vname)).unwrap_or_else(|| format!(" {}", vname));
            self.arc_event("The raid".into(), format!("{} {}.{} was killed before the others could reach them{}.", who, came, how, readiness), None);
            // The clash, blow by blow (`fight.rs`).
            let foe = capital(&threat.name);
            for l in self.fight(&foe, threat.monster.as_ref(), victim, None, "death") { self.note(l); }
            self.bury(victim, &format!("in the raid of {}", who));
            // The closest swears vengeance (`vow.rs`).
            self.swear_vengeance(victim, &threat);
            // Overwhelmed: a second falls before they are driven off.
            if danger > ready + 0.65 {
                let vp = self.settlers[victim].pos;
                let near = |c: &Colony, i: usize| (c.settlers[i].pos.0 as i32 - vp.0 as i32).abs().max((c.settlers[i].pos.1 as i32 - vp.1 as i32).abs());
                let second = alive.iter().copied().filter(|&i| i != victim && self.settlers[i].alive && self.settlers[i].past.as_ref().map_or(true, |p| p.age >= 12))
                    .min_by_key(|&i| (near(self, i), i));
                if let Some(second) = second {
                    let sn = self.settlers[second].name.clone();
                    self.note(format!("Before they are driven off {} falls too, trying to reach {}.", sn, vname));
                    self.bury(second, &format!("in the raid of {}", who));
                }
            }
            // It killed, but the camp may yet kill it (`fight.rs`).
            if matches!(threat.kind, ThreatKind::Beast | ThreatKind::Deep) { self.maybe_slay(&threat, 1.6, at); }
        } else if danger > ready || mail.is_some() {
            let vp = self.settlers[victim].pos;
            let saviour = alive.iter().copied().filter(|&i| i != victim).min_by_key(|&i| (d(i, vp), i)).unwrap_or(victim);
            self.like(victim, saviour, 6);
            let sname = self.settlers[saviour].name.clone();
            self.arc_event("The raid".into(), format!("{} {}. {} was struck down, but {} dragged them back {}, and they lived{}.", who, came, vname, sname, if sally { "to the gate" } else { "to the fire" }, readiness), None);
            if let Some(m) = &mail {
                let line = format!("The blow that should have killed {} is turned by {}.", vname, m.replacen("a ", "the ", 1).replacen("an ", "the ", 1));
                self.note(line.clone());
                if self.milestones.insert("armour saves") {
                    let at = self.settlers[victim].pos;
                    self.moment("The armour holds".into(), line, format!("because the camp made {} at the workshop for fear of {}", m, threat.name), at);
                }
            }
            let foe = capital(&threat.name);
            for l in self.fight(&foe, threat.monster.as_ref(), victim, Some(saviour), "rescue") { self.note(l); }
            self.feel(victim, super::mind::Feel::Struck);
            // The wound its attack leaves.
            if let Some(a) = &attack {
                let after = match a.effect.as_str() {
                    "burn" => "the burns will take days to heal", "poison" => "the poison lingers in the blood", "web" => "they cut them free of the webs",
                    "bleed" => "they lost much blood", "blind" => "for days they could not see", "chill" => "the cold of it stays in the bones", _ => "the sickness of it lingers",
                };
                self.note(format!("It {}: {}.", a.did_to(&vname), after));
                let until = self.clock.tick + a.ill_days as u64 * super::TICKS_PER_DAY;
                self.settlers[victim].ill_until = self.settlers[victim].ill_until.max(until);
            }
            // Hurt badly enough, the beast does not get away even as it wins the night (`fight.rs`).
            if matches!(threat.kind, ThreatKind::Beast | ThreatKind::Deep) { self.maybe_slay(&threat, 1.3, at); }
            self.feel(victim, super::mind::Feel::SavedBy { whom: sname.clone() });
            self.feel(saviour, super::mind::Feel::Saved { whom: vname.clone() });
            let stone_at = self.spot_from_camp(-4, -5);
            self.marks.push(ColonyMark { at: stone_at, kind: MarkKind::Stone, title: format!("The stone of {}", sname),
                text: format!("Raised for {}, who saved {} on the night of the raid, day {}.", sname, vname, self.clock.day()), day: self.clock.day() });
        } else {
            self.arc_event("The raid".into(), if sally { format!("{} {}, and broke before the spears{}.", who, came, readiness) } else { format!("{} came in the night, found the watch awake and the camp ready, and went away with nothing{}.", who, readiness) }, None);
            let foe = capital(&threat.name);
            let mut lines = self.fight(&foe, threat.monster.as_ref(), victim, None, "rout");
            // A beast hurt enough does not get away (`fight.rs`).
            let slain = matches!(threat.kind, ThreatKind::Beast | ThreatKind::Deep) && {
                let last = lines.pop();
                for l in lines.drain(..) { self.note(l); }
                let s = self.maybe_slay(&threat, 1.0, at);
                if !s { if let Some(l) = last { self.note(l); } }
                s
            };
            if !slain { for l in lines { self.note(l); } }
            // One of a broken band may be taken alive (`prisoners.rs`).
            let taker = self.blows.1;
            if threat.kind == ThreatKind::Warband {
                let people = threat.name.trim_start_matches("a war band of ").split(", led by ").next().unwrap_or("").to_string();
                let day = self.clock.day();
                self.regard(threat.faction, &people, threat.from, "routs", -5, format!("drove off their war band on day {}", day));
            }
            self.take_prisoner(&threat, taker, seed);
            // A war band's leader may fall to the camp's blows (`band` in `plan`).
            // (Blows enough, and one rout in three: a leader is guarded.)
            // (Debug: PLANET_FORCE_LEADER_FALL=1 drops the roll and the blows needed, for its test.)
            let forced = std::env::var("PLANET_FORCE_LEADER_FALL").is_ok();
            if threat.kind == ThreatKind::Warband && ((self.blows.0 >= 1.4 && hash(seed, 0x1EAD) % 3 == 0) || forced) {
                if let (Some(leader), Some(k)) = (threat.name.split(", led by ").nth(1).map(String::from), self.blows.1) {
                    let people = threat.name.trim_start_matches("a war band of ").split(", led by ").next().unwrap_or("").to_string();
                    let kn = self.settlers[k].name.clone();
                    let guest = self.visitors.iter().any(|v| v.came && v.name == leader);
                    let line = format!("{} cuts down {}, who led them{}; the band of {} carries the body off into the dark.", kn, leader, if guest { ", and who once sat at this camp's fire" } else { "" }, people);
                    self.note(line.clone());
                    self.moment(format!("The death of {}", leader), line, format!("because {} led the war band against a camp that was ready for it", leader), at);
                    self.settlers[k].deeds.push(format!("slew {} of {} on day {}", leader, people, self.clock.day()));
                    self.feel(k, super::mind::Feel::Slew { what: leader.clone() });
                    self.regard(threat.faction, &people, threat.from, "leader", -10, format!("cut down {}, who led their war band, on day {}", leader, self.clock.day()));
                    self.vow_kept(&leader, Some(k));
                    self.slain.push(leader);
                }
            }
        }
        // Goblins and orcs steal children in the confusion (`snatch.rs`).
        self.snatch_in_the_raid(&threat, danger <= ready, seed);
        // A routed band may leave a stolen artifact behind (`thieves.rs`).
        if threat.kind == ThreatKind::Warband { self.recover_stolen(threat.faction, danger <= ready, seed); }
        // A war band come for the relic carries it off unless routed (`relic.rs`).
        self.relic_after_raid(threat.faction, danger <= ready, &capital(&threat.name));
        // Everyone who lived through the night carries it.
        for j in 0..self.settlers.len() { if self.settlers[j].alive { self.feel(j, super::mind::Feel::RaidNight); } }
        self.fighting_people = None;
        self.siege = None;
    }

    /// Choose tonight's watcher, if there is one, and say so.
    fn set_watch(&mut self, day: u64) {
        let fit = |c: &Colony, i: usize| c.settlers[i].alive && c.settlers[i].hunger < 0.8 && c.settlers[i].ill_until <= c.clock.tick && c.settlers[i].past.as_ref().map_or(true, |p| p.age >= 12);
        let threat = self.arc.as_ref().unwrap().threat.name.clone();
        let post = self.watch_post();
        let dreamer = (0..self.settlers.len()).find(|&i| fit(self, i) && self.dream_of(i) == Some(super::Dream::Watch));
        let blessed = self.marked_at(post, false);
        let forbidden = self.marked_at(post, true);
        // The bravest first; a coward keeps watch only when nobody else can.
        let mut fits: Vec<usize> = (0..self.settlers.len()).filter(|&i| fit(self, i)).collect();
        fits.sort_by_key(|&i| (std::cmp::Reverse(self.settlers[i].persona.facet(crate::persona::Facet::Bravery)), i));
        if fits.iter().any(|&i| self.settlers[i].persona.facet(crate::persona::Facet::Bravery) > 9) {
            fits.retain(|&i| self.settlers[i].persona.facet(crate::persona::Facet::Bravery) > 9);
        }
        // On the raid's eve, one who hates the threat's people will not sleep.
        let eve = self.arc.as_ref().map_or(false, |a| day + 1 == a.raid_day);
        let hater = if eve { self.haters_of_threat().into_iter().find(|&k| fit(self, k)) } else { None };
        // Veterans volunteer first, in turn.
        let vets: Vec<usize> = fits.iter().copied().filter(|&k| self.is_veteran(k)).collect();
        let pick = |n: usize| -> Option<usize> { if !vets.is_empty() { vets.get(n % vets.len()).copied() } else { fits.get(n % fits.len().max(1)).copied() } };
        let (watcher, why) = if let Some(i) = dreamer {
            (Some(i), "as they dreamt".to_string())
        } else if let (Some(i), false) = (hater, forbidden) {
            let f = self.settlers[i].past.as_ref().and_then(|p| p.feeling.as_ref()).map(|f| they_form(&f.0)).unwrap_or_default();
            (Some(i), format!("and will not sleep: they {}", f))
        } else if forbidden {
            self.note(format!("No one keeps watch tonight: the watch post is on ground the patron forbade."));
            (None, String::new())
        } else if blessed {
            (pick(day as usize), "at the post the patron blessed".to_string())
        } else if self.mandate == Some(super::society::Mandate::Watch) || (day - self.arc.as_ref().map_or(0, |a| a.refugee_day)) % 2 == 1 {
            (pick(day as usize / 2), String::new())
        } else {
            (None, String::new())
        };
        if let Some(i) = watcher {
            let vet = self.is_veteran(i);
            let a = self.arc.as_mut().unwrap();
            a.watches += 1;
            if vet { a.veteran_watches += 1; }
            self.watcher = Some(i);
            let name = self.settlers[i].name.clone();
            // A veteran says where they stood watch before.
            let battle = if vet && why.is_empty() { self.battle_of(i).map(|b| format!(", as at {}", b)).unwrap_or_default() }
                // The brave say so.
                else if why.is_empty() && self.settlers[i].persona.facet(crate::persona::Facet::Bravery) >= 76 { format!(" ({})", self.settlers[i].persona.facet_phrase(crate::persona::Facet::Bravery as usize).unwrap_or_default().replacen("is ", if self.settlers[i].persona.female { "she is " } else { "he is " }, 1)) }
                else { String::new() };
            self.note(format!("{} keeps watch tonight{}{}, for fear of {}.", name, if why.is_empty() { String::new() } else { format!(" {}", why) }, battle, threat));
        }
    }

    /// How ready the camp is for the raid: (score, the tally in words, what the patron did).
    /// Unattended about 0.6; the patron's blessing of the watch post, a dream of the watch, a
    /// favoured veteran to captain it and a dream that hurries the walls swing it by ~0.3.
    pub fn readiness(&self) -> (f32, String, String) {
        let watches = self.arc.as_ref().map_or(0, |a| a.watches);
        let hut = self.hut.as_ref().map_or(false, |h| h.done);
        let alive: Vec<usize> = (0..self.settlers.len()).filter(|&i| self.settlers[i].alive).collect();
        let palisade = self.projects.iter().find(|p| p.kind == super::projects::ProjectKind::Palisade);
        let walls = palisade.map_or(0.0, |p| if p.done { 1.0 } else { p.used as f32 / p.needed.max(1) as f32 });
        let captain = self.patron.favourite.filter(|&i| self.settlers[i].alive
            && self.settlers[i].past.as_ref().map_or(false, |p| p.calling.starts_with("a veteran")));
        // The watch as a share of the nights between the refugees and the raid (every night: 0.4).
        let nights = self.arc.as_ref().map_or(8, |a| a.raid_day.saturating_sub(a.refugee_day).max(1)) as f32;
        let ready = 0.4 * (watches as f32).min(nights) / nights + if hut { 0.15 } else { 0.0 } + 0.15 * walls + 0.01 * alive.len() as f32
            + if captain.is_some() { 0.1 } else { 0.0 }
            // Veterans know how to keep a watch.
            + (0.02 * self.arc.as_ref().map_or(0, |a| a.veteran_watches) as f32).min(0.08)
            // A hall in the rock with one door: safe.
            + if self.projects.iter().any(|p| p.done && p.kind == super::projects::ProjectKind::DugHall) { 0.2 } else { 0.0 }
            // Arms of the ore they dug, made at the workshop: 0.05 a seam, 0.1 at most.
            + if self.iron_worked() { 0.05 * self.ore_found.min(2) as f32 } else { 0.0 }
            // A watch post the patron blessed: a keener watch.
            + if self.marked_at(self.watch_post(), false) { 0.1 } else { 0.0 }
            // A lookout sees them coming.
            + if self.projects.iter().any(|p| p.done && p.kind == super::projects::ProjectKind::Lookout) { 0.08 } else { 0.0 }
            // A ditch round the wall: they come by the gates or not at all (`delve.rs`).
            + if self.moat_dug() { 0.08 } else { 0.0 }
            // The bridges raised: they must cross the ditch under the spears (`delve.rs`).
            + if self.bridges_up { 0.12 } else { 0.0 }
            // Spears in drilled hands (`militia.rs`).
            + self.militia_ready().0
            // A sally at dawn catches the besiegers in their camp (`siege.rs`).
            + if self.siege.as_ref().map_or(false, |s| s.sally) { 0.15 } else { 0.0 }
            // The patron's bell: the camp was warned (`ring_bell`).
            + if self.bell_rung() { 0.1 } else { 0.0 };
        let wall_words = match palisade {
            None => "no palisade".to_string(),
            Some(p) if p.done => "the palisade closed".into(),
            Some(_) if walls >= 0.5 => "the palisade half raised".into(),
            Some(_) if walls > 0.0 => "the palisade begun".into(),
            Some(_) => "the palisade only staked out".into(),
        };
        let armed = self.militia_ready().1;
        let tally = format!("{} night{} of watch kept, {}, {}, {} to fight{}{}", watches, if watches == 1 { "" } else { "s" },
            if self.bridges_up { format!("{}, ditched and its bridges raised", wall_words) } else if self.moat_dug() { format!("{} and ditched", wall_words) } else { wall_words }, if hut { "the hut standing" } else { "no roof to hold" }, alive.len(), if armed > 0 { format!(", {} of them drilled and under arms", armed) } else { String::new() },
            if self.bell_rung() { ", warned by the bell" } else { "" });
        let patron = match captain {
            Some(i) => format!("; {}, the patron's favourite, captained the watch", self.settlers[i].name),
            None => String::new(),
        };
        (ready, tally, patron)
    }

    /// A newcomer joins at the camp.
    pub fn add_settler(&mut self, name: String, past: Option<crate::history::settlers::Past>) {
        let mut s = self.settlers[0].clone();
        s.name = name;
        s.pos = self.camp;
        s.path.clear();
        s.hunger = 0.6;
        s.fatigue = 0.6;
        s.job = super::Job::Idle;
        s.carrying = None;
        s.why = "Just arrived, footsore".into();
        s.alive = true;
        s.stuck = 0;
        s.starving = 0;
        s.skill = super::skills_from_past(past.as_ref(), &s.name);
        s.persona = past.as_ref().and_then(|p| p.persona.clone()).unwrap_or_else(|| crate::persona::Persona::roll("human", None, crate::persona::seed_of(&s.name, self.seed)));
        s.stride_frac = 0.0;
        s.mind = Default::default();
        s.wounds = Vec::new();
        s.office = None;
        s.made = Vec::new();
        s.deeds = Vec::new();
        s.drill = 0.0;
        s.guest_until = 0;
        s.visitor = None;
        s.spouse = None;
        s.bed_blocked_until = 0;
        s.away_until = 0;
        s.last_drink = self.clock.day();
        s.role = None;
        s.loads_laid = 0;
        s.past = past;
        self.settlers.push(s);
    }

    /// The arc told afterwards, as a journal entry (HTML).
    pub fn tale(&self, history: Option<&WorldHistory>) -> String {
        let Some(arc) = &self.arc else { return String::new() };
        let name = self.name.clone().unwrap_or_else(|| "the camp".into());
        let mut out = format!("<!doctype html><meta charset=\"utf-8\"><title>The First Trouble of {0}</title>\
<style>body{{background:#efe6d0;color:#382a20;font:18px/1.6 'IM Fell English',Georgia,serif;max-width:44rem;margin:3rem auto;padding:0 1rem}}\
h2{{color:#9a2a1e;font-variant:small-caps;letter-spacing:.05em}}b{{color:#9a2a1e}}.cause{{color:#806a52;font-size:.9em}}</style>\
<article class=\"tale\"><h2>The First Trouble of {0}</h2>", esc(&name));
        for e in &arc.events {
            let cause = e.world_cause.and_then(|c| history.and_then(|h| h.chronicle.get(c)))
                .map(|c| format!(" <span class=\"cause\">(In the chronicle: {}, year {}.)</span>", esc(&c.title), c.date.year)).unwrap_or_default();
            out.push_str(&format!("<p><b>Day {}. {}.</b> {} <i>It happened {}.</i>{}</p>", e.day, esc(&e.title), esc(&e.text), esc(&e.because), cause));
        }
        out.push_str("</article>");
        out
    }
}

/// A feeling in the third person plural: "hates X" -> "hate X", "has not forgiven" -> "have not forgiven".
pub(crate) fn they_form(f: &str) -> String {
    if let Some(r) = f.strip_prefix("hates ") { format!("hate {}", r) }
    else if let Some(r) = f.strip_prefix("has not forgiven ") { format!("have not forgiven {}", r) }
    else if let Some(r) = f.strip_prefix("misses ") { format!("miss {}", r) }
    else { f.to_string() }
}
fn lower(s: &str) -> String { let mut c = s.chars(); c.next().map(|f| f.to_lowercase().collect::<String>() + c.as_str()).unwrap_or_default() }
pub(crate) fn capital_word(s: &str) -> String { capital(s) }
fn capital(s: &str) -> String { let mut c = s.chars(); c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default() }
fn esc(s: &str) -> String { s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;") }

/// The arc's last step, for a test or a plate: did the raid end in a death, a rescue or a rout?
pub fn ending(arc: &Arc) -> Option<&ArcEvent> { arc.events.iter().rev().find(|e| e.title == "The raid") }

#[allow(dead_code)]
const _DAY: u64 = TICKS_PER_DAY;

/// How the patron plays a raid trial (`Colony::raid_trial`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatronStyle { Absent, Careful, Careless }

impl Colony {
    /// Live up to the raid with a scripted patron and return the raid's line. Careful: once the
    /// refugees are in, bless the watch post, favour a veteran to captain the watch, and each
    /// dawn dream the walls up (or a watch). Careless: forbid the ground at the camp's north
    /// edge (where the watch would stand) and send dreams of rest.
    pub fn raid_trial(&mut self, style: PatronStyle) -> String {
        let until = |c: &Colony, day: u64, hour: u64| c.clock.tick < (day - 1) * TICKS_PER_DAY + hour * 60;
        let step_to = |c: &mut Colony, day: u64, hour: u64| { while until(c, day, hour) { c.tick(); } };
        match style {
            PatronStyle::Absent => {}
            PatronStyle::Careless => {
                step_to(self, 1, 8);
                let post = self.watch_post();
                let _ = self.mark_place(post, 8, true);
                let _ = self.send_dream(0, super::Dream::Rest);
                let _ = self.send_dream(1, super::Dream::Rest);
                let raid_day = self.arc.as_ref().map_or(14, |a| a.raid_day);
                for day in 2..raid_day {
                    step_to(self, day, 7);
                    let i = (day as usize) % self.settlers.len();
                    let _ = self.send_dream(i, super::Dream::Rest);
                }
            }
            PatronStyle::Careful => {
                let refugee_day = self.arc.as_ref().map_or(6, |a| a.refugee_day);
                step_to(self, refugee_day, 18);
                let post = self.watch_post();
                let _ = self.mark_place(post, 3, false);
                let vet = (0..self.settlers.len()).find(|&i| self.settlers[i].alive
                    && self.settlers[i].past.as_ref().map_or(false, |p| p.calling.starts_with("a veteran")));
                if let Some(v) = vet { let _ = self.favour_settler(v); }
                for day in refugee_day + 1..=self.arc.as_ref().map_or(14, |a| a.raid_day) {
                    step_to(self, day, 7);
                    let walls_up = self.projects.iter().any(|p| p.kind == super::projects::ProjectKind::Palisade && p.done);
                    let builder = (0..self.settlers.len()).find(|&i| self.settlers[i].alive && Some(i) != vet && self.dream_of(i).is_none());
                    if let Some(b) = builder {
                        let _ = self.send_dream(b, if walls_up { super::Dream::Watch } else { super::Dream::Hut });
                    }
                }
            }
        }
        let raid_day = self.arc.as_ref().map_or(14, |a| a.raid_day);
        step_to(self, raid_day + 1, 7);
        self.arc.as_ref().and_then(|a| ending(a)).map(|e| e.text.clone()).unwrap_or_else(|| "no raid".into())
    }
}

