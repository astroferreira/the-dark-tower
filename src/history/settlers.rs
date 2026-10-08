//! Settlers with pasts: the colony's people come out of the history.
//!
//! Seven strangers with stock names had nothing to remember. Here they are drawn from what
//! happened near the embark: survivors of the nearest town that fell (several share that day),
//! veterans of their people's last war (who fought at a real battle under a real commander), and
//! younger kin of a living notable. Each has an age, two or three backstory lines that cite
//! chronicle events, and one feeling toward a real people or person. No RNG is drawn from the
//! history: choices hash the colony seed.

use crate::history::*;
use crate::history::events::types::{Event, EventType};
use crate::history::world_state::WorldHistory;

#[derive(Clone, Debug, Default)]
pub struct Past {
    pub age: u32,
    pub people: Option<FactionId>,
    /// What they were before: "a survivor of Ripu", "a veteran of the Salt War".
    pub calling: String,
    /// Backstory lines, each with the event it rests on.
    pub lines: Vec<(String, Option<EventId>)>,
    /// One feeling toward a real people or person: ("hates The Ashpit Horde, who took Ripu", whom).
    pub feeling: Option<(String, EntityId)>,
    /// Who they are (`persona.rs`): rolled from their people's template and culture.
    pub persona: Option<crate::persona::Persona>,
    /// Their people's songs, poems and dances (`arts.rs`): (name, what it is, kind word).
    pub arts: Vec<(String, String, &'static str)>,
    /// Their people's first instrument ("the dulmgar (a set of pipes)").
    pub instrument: Option<String>,
    /// What they could show in a carving: the events of their past, as phrases ("the siege of
    /// Ripu"), with the event (`colony::craft`).
    pub images: Vec<(String, EventId)>,
    /// The towns their past names (for news that touches them, `colony::trade`).
    pub towns: Vec<SettlementId>,
    /// Their people's faith: (the religion, its chief god as named: "Xilnar the Stormmother").
    pub faith: Option<(String, String)>,
}

/// What a settler can make and show: their people's instrument and the events of their past.
pub fn fill_craft(h: &WorldHistory, past: &mut Past) {
    past.instrument = past.people.and_then(|f| crate::history::arts::of_people(h, f).instruments.into_iter().next()).map(|i| format!("{} ({})", i.name, i.a_kind()));
    past.images = past.lines.iter().filter_map(|(_, e)| e.and_then(|e| h.chronicle.get(e))).map(|e| (phrase(h, e), e.id)).collect();
    past.images.dedup_by(|a, b| a.1 == b.1);
    past.towns = past.lines.iter().filter_map(|(_, e)| e.and_then(|e| h.chronicle.get(e))).flat_map(|e| e.primary_participants.iter().filter_map(|p| if let EntityId::Settlement(s) = p { Some(*s) } else { None }).collect::<Vec<_>>()).collect();
    past.towns.sort();
    past.towns.dedup();
    past.faith = past.people.and_then(|f| h.factions.get(&f)).and_then(|f| f.state_religion).and_then(|r| h.religions.get(&r))
        .and_then(|r| r.deities.first().and_then(|d| h.deities.get(d)).map(|d| {
            let domains: Vec<String> = d.domains.iter().map(|x| format!("{:?}", x).to_lowercase()).collect();
            let named = match (d.epithets.first(), domains.is_empty()) {
                (Some(e), _) => format!("{} {}", d.name, e),
                (None, false) => format!("{}, the god of {}", d.name, crate::persona::list(&domains)),
                (None, true) => d.name.clone(),
            };
            (r.name.clone(), named)
        }));
}

/// The forms a settler of `people` knows (`arts.rs`).
pub fn arts_of(h: &WorldHistory, people: Option<FactionId>) -> Vec<(String, String, &'static str)> {
    people.map(|f| crate::history::arts::of_people(h, f).forms.into_iter().map(|x| (x.name, x.what, x.kind.word())).collect()).unwrap_or_default()
}

/// A settler's persona: their people's race and culture, rolled from their name and the
/// colony's seed. Kin of a living figure take after them: about half their character and
/// often their hair (Dwarf Fortress has children inherit from parents; here the line runs to
/// the figure the settler misses).
pub fn persona_for(h: &WorldHistory, name: &str, past: &Past, seed: u64) -> crate::persona::Persona {
    use crate::persona::Persona;
    let race = past.people.and_then(|f| h.factions.get(&f)).and_then(|f| h.races.get(&f.race_id));
    let tag = race.map(|r| format!("{:?}", r.base_type).to_lowercase()).unwrap_or_else(|| "human".into());
    let culture = race.and_then(|r| h.cultures.get(&r.culture_id)).map(|c| &c.values);
    let mut p = Persona::roll(&tag, culture, crate::persona::seed_of(name, seed));
    if let Some((_, EntityId::Figure(f))) = &past.feeling {
        if let Some(fig) = h.figures.get(f) {
            let elder = Persona::of_figure(h, fig);
            if elder.race == p.race {
                for i in 0..p.facets.len().min(elder.facets.len()) {
                    if hash(seed ^ crate::persona::seed_of(name, 1), i as u64) % 2 == 0 {
                        let jitter = (hash(seed, 100 + i as u64) % 21) as i32 - 10;
                        p.facets[i] = (elder.facets[i] as i32 + jitter).clamp(0, 100) as u8;
                    }
                }
                if hash(seed, 0xA1) % 3 != 0 { p.hair = elder.hair.clone(); }
            }
        }
    }
    p
}

/// The module's hash, for callers that need the same mixing.
pub fn hash_pub(seed: u64, salt: u64) -> u64 { hash(seed, salt) }

fn hash(seed: u64, salt: u64) -> u64 {
    let mut x = seed ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 31)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^ (x >> 29)
}

fn dist(a: (usize, usize), b: (usize, usize), w: usize) -> usize {
    let dx = a.0.abs_diff(b.0);
    dx.min(w.saturating_sub(dx)) + a.1.abs_diff(b.1)
}

/// An event as running text: "the siege of Ripu", "the founding of Basalttower", "the Battle of
/// Ripu Ford".
fn phrase(h: &WorldHistory, e: &Event) -> String {
    if let Some(m) = e.factions_involved.iter().find_map(|f| crate::history::remembrance::memory_of(h, e, *f)) { return m.phrase; }
    if let Some(rest) = e.title.strip_prefix("The ") { return format!("the {}", rest); }
    if let Some(town) = e.title.strip_suffix(" founded") { return format!("the founding of {}", town); }
    match e.title.split_once(" of ") {
        Some((head, tail)) if !head.contains(' ') => format!("the {} of {}", head.to_lowercase(), tail),
        _ => e.title.clone(),
    }
}

fn town_of(e: &Event) -> Option<SettlementId> {
    e.primary_participants.iter().find_map(|p| if let EntityId::Settlement(s) = p { Some(*s) } else { None })
}

/// `n` settlers for an embark at world tile `tile`, each with a name and a past.
pub fn roster(h: &WorldHistory, tile: (usize, usize), n: usize, seed: u64) -> Vec<(String, Past)> {
    use crate::history::naming::styles::NamingStyle;
    use crate::history::naming::generator::NameGenerator;
    use rand::SeedableRng;
    let now = h.current_date.year;
    let w = h.tile_history.width.max(1);
    let fname = |f: FactionId| h.factions.get(&f).map(|x| x.name.clone()).unwrap_or_default();
    let tname = |s: SettlementId| h.settlements.get(&s).map(|x| x.name.clone()).unwrap_or_default();
    let mut out: Vec<Past> = Vec::new();

    // Home: the nearest living town and its people.
    let home = h.settlements.values().filter(|t| !t.is_destroyed())
        .min_by_key(|t| (dist(t.location, tile, w), t.id));
    let home_people = home.map(|t| t.faction);

    // Survivors of the nearest town that fell in living memory (three of them, one shared day).
    let fall = h.chronicle.events.iter()
        .filter(|e| e.date.year + 45 >= now && matches!(e.event_type, EventType::SiegeEnded | EventType::ShadowConquest | EventType::SettlementDestroyed))
        .filter_map(|e| town_of(e).and_then(|s| h.settlements.get(&s)).map(|t| (dist(t.location, tile, w), e, t)))
        // The nearest town's latest fall (the first arc's refugees tell the same one).
        .min_by_key(|(d, e, _)| (*d, std::cmp::Reverse(e.id)));
    if let Some((_, e, t)) = fall {
        let taker = e.factions_involved.first().copied().filter(|f| Some(*f) != e.factions_involved.get(1).copied());
        let old_people = e.factions_involved.get(1).copied().or(Some(t.faction));
        let cause = e.causes.first().and_then(|c| h.chronicle.get(*c));
        let burned = e.title.contains("burned") || e.event_type == EventType::SettlementDestroyed || e.description.contains("burned");
        // Who fled where: the notables' flights from this town (`people.rs`), caused by the fall.
        let flights: Vec<&Event> = h.chronicle.events.iter()
            .filter(|x| x.event_type == EventType::FigureMoved && x.causes.contains(&e.id)).collect();
        let since = now - e.date.year;
        for k in 0..3u64 {
            // An adult at the fall, a child, a youth.
            let age_then = match k { 0 => 18 + hash(seed, 10) % 14, 1 => 3 + hash(seed, 11) % 8, _ => 11 + hash(seed, 12) % 6 } as u32;
            let age = since + age_then;
            let how = match k { 0 => "fought on the walls when", 1 => "was a child when", _ => "lost family when" };
            let mut lines = vec![(format!("Born in {}; {} it {} in {}.", t.name, how, if burned { "burned" } else { "fell" }, e.date.year), Some(e.id))];
            match (k, flights.get(k as usize % flights.len().max(1)).filter(|_| !flights.is_empty()), cause) {
                (1 | 2, Some(fl), _) => {
                    let who = fl.primary_participants.iter().find_map(|p| if let EntityId::Figure(f) = p { h.figures.get(f) } else { None });
                    let to = town_of(fl).map(tname);
                    lines.push((match (who, to) {
                        (Some(w), Some(to)) => format!("Followed {} to {} after the fall.", w.full_name(), to),
                        (Some(w), None) => format!("Went into exile with {}, who had no town left to go to.", w.full_name()),
                        _ => format!("Fled with the others: {}.", fl.description.trim_end_matches('.')),
                    }, Some(fl.id)));
                }
                (_, _, Some(c)) => lines.push((format!("{} {}.", if k == 0 { "Saw it begin with" } else { "Remembers" }, phrase(h, c)), Some(c.id))),
                _ => {}
            }
            let feeling = taker.map(|f| (format!("hates {}, who took {}", fname(f), t.name), EntityId::Faction(f)));
            out.push(Past { age, people: old_people, calling: format!("a survivor of {}", t.name), lines, feeling, persona: None, arts: Vec::new(), instrument: None, images: Vec::new(), towns: Vec::new(), faith: None });
        }
    }

    // Veterans of their people's last war: the latest battle it fought, under its commander.
    let people = home_people.or_else(|| out.first().and_then(|p| p.people));
    if let Some(p) = people {
        // The two veterans fought in their people's two latest battles (one each).
        let battles: Vec<&Event> = h.chronicle.events.iter().rev()
            .filter(|e| e.event_type == EventType::BattleFought && e.factions_involved.contains(&p)).take(2).collect();
        for (bi, b) in battles.iter().enumerate() {
            let b = *b;
            let war = h.wars.values().find(|w| w.battles.contains(&b.id));
            let commander = b.primary_participants.iter().find_map(|x| if let EntityId::Figure(f) = x { h.figures.get(f).filter(|fig| fig.faction == Some(p)) } else { None });
            let enemy = b.factions_involved.iter().copied().find(|f| *f != p);
            let ks: Vec<u64> = if battles.len() == 1 { vec![0, 1] } else { vec![bi as u64] };
            for k in ks {
                let age = (now - b.date.year) + 19 + (hash(seed, 20 + k) % 20) as u32;
                let mut lines = vec![(format!("Fought at {} in {}{}.", phrase(h, b), b.date.year,
                    commander.map(|c| format!(" under {}", c.full_name())).unwrap_or_default()), Some(b.id))];
                if let Some(w) = war {
                    match w.ended {
                        Some(end) => {
                            let ended = h.chronicle.events.iter().find(|e| e.event_type == EventType::WarEnded && e.date == end && e.title.ends_with(&w.name));
                            lines.push((format!("Came home when {} ended in {}.", w.name.replacen("The ", "the ", 1), end.year), ended.map(|e| e.id).or(w.declaration_event)));
                        }
                        None => lines.push((format!("Left {} while it is still fought.", w.name.replacen("The ", "the ", 1)), w.declaration_event)),
                    }
                }
                let feeling = if k == 0 {
                    commander.map(|c| (format!("would follow {} anywhere", c.full_name()), EntityId::Figure(c.id)))
                } else {
                    enemy.map(|f| (format!("has not forgiven {}", fname(f)), EntityId::Faction(f)))
                };
                out.push(Past { age, people: Some(p), calling: format!("a veteran of {}", war.map(|w| w.name.replacen("The ", "the ", 1)).unwrap_or_else(|| "the wars".into())), lines, feeling, persona: None, arts: Vec::new(), instrument: None, images: Vec::new(), towns: Vec::new(), faith: None });
            }
        }
    }

    // Younger kin of a living notable of their people, until the roster is full.
    let mut notables: Vec<(FigureId, SettlementId)> = h.people.as_ref().map(|pp| pp.home.iter()
        .filter(|(f, _)| h.figures.get(f).map_or(false, |x| x.is_alive() && x.faction == people))
        // Kin of someone the chronicle knows (a newly raised heir has done nothing yet).
        .filter(|(f, _)| h.chronicle.last_of(EntityId::Figure(**f)).is_some())
        .map(|(f, s)| (*f, *s)).collect()).unwrap_or_default();
    notables.sort_by_key(|(f, s)| (h.settlements.get(s).map_or(usize::MAX, |t| dist(t.location, tile, w)), *f));
    let mut k = 0u64;
    while out.len() < n {
        let Some(&(f, s)) = notables.get(k as usize % notables.len().max(1)).filter(|_| !notables.is_empty()) else { break };
        let fig = &h.figures[&f];
        let role = h.people.as_ref().and_then(|pp| pp.role.get(&f)).map(|r| r.word()).unwrap_or("notable");
        let mut lines = Vec::new();
        // Old enough to have seen what they remember (at least 6 then), and no older than 70.
        let seen = h.chronicle.last_of(EntityId::Figure(f)).and_then(|x| h.chronicle.get(x));
        let left = h.chronicle.last_of(EntityId::Settlement(s)).and_then(|x| h.chronicle.get(x));
        let oldest = [seen, left].iter().flatten().map(|e| e.date.year).min().unwrap_or(now);
        let age = (16 + (hash(seed, 30 + k) % 16) as u32).max(now.saturating_sub(oldest) + 6);
        let lived = |e: &Event| e.date.year + age >= now + 6;
        match seen.filter(|e| lived(e) && age <= 70) {
            Some(ev) => lines.push((format!("Younger kin of {}, {} of {}; last saw them at {} ({}).", fig.full_name(), role, tname(s), phrase(h, ev), ev.date.year), Some(ev.id))),
            None => lines.push((format!("Younger kin of {}, {} of {}.", fig.full_name(), role, tname(s)), None)),
        }
        if let Some(ev) = left.filter(|e| lived(e) && age <= 70) {
            lines.push((format!("Left {} after {} ({}).", tname(s), phrase(h, ev), ev.date.year), Some(ev.id)));
        }
        let age = age.min(70);
        out.push(Past { age, people, calling: format!("kin of {}", fig.name), lines, feeling: Some((format!("misses {}", fig.full_name()), EntityId::Figure(f))), persona: None, arts: Vec::new(), instrument: None, images: Vec::new(), towns: Vec::new(), faith: None });
        k += 1;
        if k > 64 { break; }
    }
    // A world with no history near: plain wanderers.
    while out.len() < n { out.push(Past { age: 18 + out.len() as u32 * 3, calling: "a wanderer".into(), ..Default::default() }); }
    out.truncate(n);

    // Names in their people's tongue.
    let mut names: Vec<String> = Vec::new();
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed ^ 0x5E77_1E25);
    out.into_iter().map(|past| {
        let arche = past.people.and_then(|f| h.factions.get(&f)).and_then(|f| h.races.get(&f.race_id))
            .map(|r| r.base_type.default_naming_archetype()).unwrap_or(crate::history::naming::styles::NamingArchetype::Compound);
        let style = NamingStyle::from_archetype(NamingStyleId(0), arche);
        let mut name = NameGenerator::personal_name(&style, &mut rng);
        let mut tries = 0;
        while names.contains(&name) && tries < 20 { name = NameGenerator::personal_name(&style, &mut rng); tries += 1; }
        names.push(name.clone());
        let mut past = past;
        past.persona = Some(persona_for(h, &name, &past, seed));
        past.arts = arts_of(h, past.people);
        fill_craft(h, &mut past);
        (name, past)
    }).collect()
}
