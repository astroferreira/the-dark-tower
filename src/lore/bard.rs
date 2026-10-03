//! The bard: songs, poems, legends, laments and artifact lore written by a local LLM (served by
//! Ollama) from the world's own history and geography.
//!
//! Procedural history supplies the facts; the model supplies the voice. Each piece is a
//! *commission* built from one subject (a people's founding, a notable life, a beast, a razed
//! town, a storied artifact, a bone field) with everything the writer would know: who speaks,
//! their people and temperament, the rivers and mountains around them, what happened. Prompts
//! keep the model to the given names and away from game vocabulary and year numbers.
//!
//! Generation takes seconds per piece (about 10 s with the default `gemma4:26b`, a mixture-of-
//! experts model with ~4B active parameters; over a minute with a dense 27B), so it runs as its own pass
//! (`--bard N`), most significant subjects first and kinds interleaved, and every finished piece
//! is kept in the world's `Library` (saved with the world), so runs can be stopped and resumed.

use crate::history::det::HashSet;

use serde::{Deserialize, Serialize};

use crate::history::entities::races::RaceType;
use crate::history::events::types::{Event, EventType};
use crate::history::world_state::WorldHistory;
use crate::history::{EntityId, FactionId, FigureId};
use crate::lore::{FeatureKind, Gazetteer};
use crate::world::WorldData;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WritingKind {
    /// A people's song of its own founding.
    FoundingSong,
    /// A poem by a notable figure about something in their life.
    Poem,
    /// A folk tale about a beast of legend.
    Legend,
    /// An elegy for a razed town.
    Lament,
    /// An artifact's inscription and story.
    ArtifactLore,
    /// A ballad of a scarred place (bone fields and the like).
    Ballad,
}

impl WritingKind {
    pub fn label(self) -> &'static str {
        match self {
            WritingKind::FoundingSong => "founding song",
            WritingKind::Poem => "poem",
            WritingKind::Legend => "legend",
            WritingKind::Lament => "lament",
            WritingKind::ArtifactLore => "inscription",
            WritingKind::Ballad => "ballad",
        }
    }
    /// Verse keeps its line breaks; prose is reflowed.
    pub fn is_verse(self) -> bool {
        !matches!(self, WritingKind::Legend)
    }
}

/// What a writing is about (ids of the history's entities).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Subject {
    Faction(u64),
    Figure(u64),
    Beast(u64),
    Settlement(u64),
    Artifact(u64),
    Place(usize, usize),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Writing {
    /// Unique per commission, so a subject is written once.
    pub key: String,
    pub kind: WritingKind,
    pub subject: Subject,
    pub title: String,
    /// Who wrote it, if a known figure.
    pub author: Option<u64>,
    /// Attribution as shown ("Vea'sienn the Founder", "the folk of Westwick").
    pub author_name: String,
    pub year: u32,
    pub text: String,
    pub model: String,
}

/// Everything written for this world so far.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Library {
    pub writings: Vec<Writing>,
}

impl Library {
    pub fn about(&self, subject: Subject) -> impl Iterator<Item = &Writing> {
        self.writings.iter().filter(move |w| w.subject == subject)
    }
    pub fn by_author(&self, figure: u64) -> impl Iterator<Item = &Writing> {
        self.writings.iter().filter(move |w| w.author == Some(figure))
    }
}

/// A piece to be written: subject, voice and the facts the writer knows.
#[derive(Clone, Debug)]
pub struct Commission {
    pub key: String,
    pub kind: WritingKind,
    pub subject: Subject,
    pub author: Option<u64>,
    pub author_name: String,
    pub year: u32,
    /// What to write, e.g. "a poem of 10 to 16 lines".
    pub form: String,
    /// How the writer's people speak.
    pub voice: String,
    pub facts: Vec<String>,
}

// ---------------------------------------------------------------------------------------------
// Context helpers
// ---------------------------------------------------------------------------------------------

/// How a people's poets sound.
pub(crate) fn voice(race: &RaceType) -> &'static str {
    match race {
        RaceType::Human => "plain, earthy and sincere, like an old ballad; fields, rivers, kin and hearth",
        RaceType::Dwarf => "terse and weighty; stone, forge, oaths and the deep roots of mountains",
        RaceType::Elf => "lyrical and long-memoried; starlight, old trees, slow seasons and loss",
        RaceType::Orc => "blunt and fierce; boasts, blood, iron, wind and the strength of the clan",
        RaceType::Goblin => "sly and mocking, quick rhythms; shadows, tricks and hunger",
        RaceType::Halfling => "homely and warm with gentle humour; gardens, kitchens, small roads and good company",
        RaceType::Reptilian => "cold and patient; sun-warmed stone, scales, water and the long sleep",
        RaceType::Fey => "dreamlike and riddling; mist, moonlight, bargains and the edges of things",
        RaceType::Undead => "mournful and grave-cold; memory, dust, the living left behind",
        RaceType::Elemental => "vast and elemental; fire, wind, stone and tide speaking as themselves",
        RaceType::Beastfolk => "primal; the hunt, the pack, the moon, scent and the wild",
        RaceType::Giant => "slow and vast; mountains, ages and the smallness of other folk",
        RaceType::Construct => "precise and measured, a made thing learning to grieve",
        _ => "spare and strange",
    }
}

/// One member of a race: "dwarf", "elf".
pub(crate) fn singular(rt: &RaceType, name: &str) -> String {
    match rt {
        RaceType::Human => "human".into(),
        RaceType::Dwarf => "dwarf".into(),
        RaceType::Elf => "elf".into(),
        RaceType::Orc => "orc".into(),
        RaceType::Goblin => "goblin".into(),
        RaceType::Halfling => "halfling".into(),
        RaceType::Reptilian => "reptilian".into(),
        RaceType::Fey => "fey".into(),
        RaceType::Undead => "undead lord".into(),
        RaceType::Elemental => "elemental".into(),
        RaceType::Beastfolk => "beastfolk".into(),
        RaceType::Giant => "giant".into(),
        RaceType::Construct => "construct".into(),
        _ => name.to_lowercase(),
    }
}

fn words(camel: &str) -> String {
    let mut s = String::new();
    for (i, c) in camel.chars().enumerate() {
        if c.is_uppercase() && i > 0 { s.push(' '); }
        s.extend(c.to_lowercase());
    }
    s
}

/// "The Westdale Human" -> "the Westdale people".
pub(crate) fn people(history: &WorldHistory, f: FactionId) -> String {
    history.factions.get(&f).map(|fac| {
        let race = history.races.get(&fac.race_id).map(|r| r.name.clone()).unwrap_or_default();
        let base = fac.name.trim_start_matches("The ").trim_start_matches("The ");
        let base = base.strip_suffix(&format!(" {}", race)).unwrap_or(base);
        let one = history.races.get(&fac.race_id).map(|r| singular(&r.base_type, &r.name)).unwrap_or_default();
        let stem = |w: &str| w.to_lowercase().chars().take(3).collect::<String>();
        let base = base.rsplit_once(' ').map(|(a, last)| if stem(last) == stem(&one) || stem(last) == stem(&race) { a } else { base }).unwrap_or(base);
        format!("the {} people", base)
    }).unwrap_or_else(|| "a forgotten people".into())
}

/// A people as a plural of persons: "the Rotfang orcs" (event texts name peoples like a single
/// person, "The Rotfang Orc", which the model otherwise reads as one individual).
pub(crate) fn folk(history: &WorldHistory, f: FactionId) -> String {
    let p = people(history, f);
    let race = history.factions.get(&f).and_then(|fac| history.races.get(&fac.race_id)).map(|r| r.name.to_lowercase()).unwrap_or_default();
    match p.strip_suffix(" people") {
        Some(stem) if !race.is_empty() => format!("{} {}", stem, race),
        _ => p,
    }
}

/// Rewrite an event text for the model: peoples become plurals, numbers become words.
pub(crate) fn humanize(history: &WorldHistory, text: &str) -> String {
    let mut s = tidy(text);
    let mut factions: Vec<_> = history.factions.values().collect();
    factions.sort_by_key(|f| std::cmp::Reverse(f.name.len()));
    for f in factions {
        if s.contains(&f.name) { s = s.replace(&f.name, &folk(history, f.id)); }
    }
    numbers_to_words(&s)
}

fn numbers_to_words(s: &str) -> String {
    const SMALL: [&str; 13] = ["no", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve"];
    let mut out = String::new();
    let mut digits = String::new();
    let flush = |digits: &mut String, out: &mut String| {
        if digits.is_empty() { return; }
        let n: u64 = digits.parse().unwrap_or(0);
        out.push_str(match n { 0..=12 => SMALL[n as usize], 13..=40 => "a score and more", 41..=150 => "many", _ => "countless" });
        digits.clear();
    };
    for c in s.chars() {
        if c.is_ascii_digit() { digits.push(c); } else { flush(&mut digits, &mut out); out.push(c); }
    }
    flush(&mut digits, &mut out);
    out.replace("one seasons", "one season").replace("one years", "one year")
}

fn race_of_faction(history: &WorldHistory, f: FactionId) -> Option<(String, RaceType)> {
    let fac = history.factions.get(&f)?;
    let r = history.races.get(&fac.race_id)?;
    Some((r.name.clone(), r.base_type.clone()))
}

/// The land around a tile in words: biome, climate, height, and named features nearby.
pub(crate) fn geography(world: &WorldData, gaz: &Gazetteer, x: usize, y: usize) -> String {
    let biome = words(&format!("{:?}", world.biomes.get(x, y)));
    let t = *world.temperature.get(x, y);
    let climate = if t < -10.0 { "bitterly cold" } else if t < 2.0 { "cold" } else if t < 12.0 { "cool" } else if t < 20.0 { "mild" } else { "hot" };
    let e = *world.heightmap.get(x, y);
    let height = if e > 2500.0 { "high in the mountains" } else if e > 1200.0 { "in the uplands" } else if e < 80.0 { "near sea level" } else { "in the lowlands" };
    let mut near: Vec<String> = Vec::new();
    let w = world.width as i64;
    for f in &gaz.features {
        let close = |p: (usize, usize), r: i64| {
            let mut dx = (p.0 as i64 - x as i64).abs();
            dx = dx.min(w - dx);
            dx <= r && (p.1 as i64 - y as i64).abs() <= r
        };
        let hit = match f.kind {
            FeatureKind::River => f.path.iter().any(|&p| close(p, 3)),
            FeatureKind::Peak | FeatureKind::Lake => close(f.anchor, 5),
            FeatureKind::MountainRange | FeatureKind::Forest | FeatureKind::Jungle | FeatureKind::Desert
            | FeatureKind::Marsh | FeatureKind::Sea | FeatureKind::Gulf | FeatureKind::Plains | FeatureKind::Tundra => close(f.anchor, 8),
            _ => false,
        };
        if hit {
            let kind = match f.kind { FeatureKind::River => "river", FeatureKind::Peak => "peak", FeatureKind::Lake => "lake", FeatureKind::MountainRange => "mountains", FeatureKind::Forest => "forest", FeatureKind::Jungle => "jungle", FeatureKind::Desert => "desert", FeatureKind::Marsh => "marsh", FeatureKind::Sea => "sea", FeatureKind::Gulf => "gulf", FeatureKind::Plains => "plains", FeatureKind::Tundra => "tundra", _ => "land" };
            near.push(format!("{} ({})", f.name, kind));
        }
        if near.len() >= 5 { break; }
    }
    let place = gaz.describe(x, y);
    let mut s = if e <= 0.0 {
        format!("{} {} waters", climate, biome)
    } else {
        format!("{} {} country, {}", climate, biome, height)
    };
    if !place.is_empty() { s.push_str(&format!(", in {}", place)); }
    if !near.is_empty() { s.push_str(&format!("; nearby: {}", near.join(", "))); }
    s
}

fn figure_name(history: &WorldHistory, id: FigureId) -> String {
    history.figures.get(&id).map(|f| match &f.epithet {
        Some(e) if !e.is_empty() => format!("{} {}", f.name, e),
        _ => f.name.clone(),
    }).unwrap_or_default()
}

fn tidy(s: &str) -> String {
    s.replace("The The ", "The ").replace("the The ", "the ")
}

// ---------------------------------------------------------------------------------------------
// Commissions
// ---------------------------------------------------------------------------------------------

/// Everything worth writing, most significant first within each kind, kinds interleaved,
/// skipping what the library already holds.
pub fn commissions(world: &WorldData, history: &WorldHistory, gaz: &Gazetteer, library: &Library) -> Vec<Commission> {
    let done: HashSet<&str> = library.writings.iter().map(|w| w.key.as_str()).collect();
    let events = &history.chronicle.events;
    let mut songs = Vec::new();
    let mut poems = Vec::new();
    let mut legends = Vec::new();
    let mut laments = Vec::new();
    let mut lore = Vec::new();
    let mut ballads = Vec::new();

    // Founding songs, biggest peoples first.
    let mut factions: Vec<_> = history.factions.values().collect();
    factions.sort_by_key(|f| std::cmp::Reverse((f.settlements.len(), f.total_population)));
    for f in factions {
        let Some(found) = events.iter().find(|e| e.event_type == EventType::FactionFounded && e.factions_involved.contains(&f.id)) else { continue };
        let founder = found.primary_participants.iter().find_map(|p| if let EntityId::Figure(id) = p { Some(*id) } else { None });
        let story = events.iter()
            .find(|e| e.event_type == EventType::RulerCrowned && e.date.year == found.date.year && (e.factions_involved.contains(&f.id) || e.description.contains(&f.name)))
            .map(|e| tidy(&e.description))
            .unwrap_or_else(|| tidy(&found.description));
        let capital = f.capital.and_then(|c| history.settlements.get(&c));
        let (race, rt) = race_of_faction(history, f.id).unwrap_or_else(|| (String::new(), RaceType::Human));
        let mut facts = vec![
            format!("The people: {} ({}), governed as a {}.", people(history, f.id), race, words(&format!("{:?}", f.government))),
            format!("How it began: {}", humanize(history, &story)),
        ];
        if let Some(c) = capital {
            facts.push(format!("Their first home and seat: {}, a place of {}.", c.name, geography(world, gaz, c.location.0, c.location.1)));
        }
        if let Some(id) = founder { facts.push(format!("The founder: {}.", figure_name(history, id))); }
        songs.push(Commission {
            key: format!("song-f{}", f.id.0),
            kind: WritingKind::FoundingSong,
            subject: Subject::Faction(f.id.0),
            author: None,
            author_name: format!("sung by {}", people(history, f.id)),
            year: found.date.year + 30,
            form: "a founding song of 12 to 20 lines, as the people still sing it generations later, in stanzas".into(),
            voice: voice(&rt).into(),
            facts,
        });
    }

    // Poems by notable figures, on something from their own life.
    let mut ruled: HashSet<FigureId> = HashSet::default();
    for e in events.iter().filter(|e| e.event_type == EventType::RulerCrowned) {
        for p in &e.primary_participants { if let EntityId::Figure(id) = p { ruled.insert(*id); } }
    }
    let mut figures: Vec<_> = history.figures.values().filter(|f| ruled.contains(&f.id) || !f.kills.is_empty()).collect();
    figures.sort_by_key(|f| std::cmp::Reverse(f.events.len() + 4 * f.kills.len()));
    for f in figures {
        let Some(fid) = f.faction else { continue };
        let (race, rt) = history.races.get(&f.race_id).map(|r| (r.name.clone(), r.base_type.clone())).unwrap_or_else(|| (String::new(), RaceType::Human));
        let home = history.factions.get(&fid).and_then(|fac| fac.capital).and_then(|c| history.settlements.get(&c));
        let died = f.death_date.map(|d| d.year).unwrap_or(history.current_date.year);
        let mut facts = vec![format!(
            "The poet: {}, a {} {} of {}{}.",
            figure_name(history, f.id), f.personality.dominant_trait(), singular(&rt, &race), people(history, fid),
            if f.titles.is_empty() { String::new() } else { format!(", {}", f.titles.join(", ")) }
        )];
        if let Some(h) = home { facts.push(format!("Home: {}, {}.", h.name, geography(world, gaz, h.location.0, h.location.1))); }
        // Theme: love, grief, a slain beast, war, or the homeland.
        let spouse = f.spouse.and_then(|s| history.figures.get(&s));
        let lost_child = f.children.iter().filter_map(|c| history.figures.get(c)).find(|c| c.death_date.map_or(false, |d| d.year < died));
        let slain: Vec<String> = f.kills.iter().filter_map(|k| if let EntityId::LegendaryCreature(c) = k { history.legendary_creatures.get(c).map(|c| c.full_name()) } else { None }).collect();
        let war = history.wars.values().filter(|w| (w.aggressors.contains(&fid) || w.defenders.contains(&fid)) && w.started.year >= f.birth_date.year + 16 && w.started.year <= died).max_by_key(|w| w.battles.len());
        let (theme, year) = if let Some(s) = spouse {
            facts.push(format!("Beloved: {}, their spouse.", figure_name(history, s.id)));
            ("a love poem to their spouse, rooted in the land they shared", f.birth_date.year + 25)
        } else if let Some(c) = lost_child {
            facts.push(format!("Grief: their child {} died before them.", figure_name(history, c.id)));
            ("an elegy for their child", c.death_date.map(|d| d.year).unwrap_or(died))
        } else if !slain.is_empty() {
            let more = slain.len().saturating_sub(2);
            let named = slain.iter().take(2).cloned().collect::<Vec<_>>().join(" and ");
            facts.push(if more > 0 { format!("Deeds: slew {} and many other beasts of legend.", named) } else { format!("Deed: slew {}.", named) });
            ("a poem about the hunt and the slaying, and what it cost", died.saturating_sub(5))
        } else if let Some(w) = war {
            let loser_or_victor = match w.victor { Some(v) if v == fid => "their people won", Some(_) => "their people lost", None => "it ended with no victor" };
            facts.push(format!("War: {} ({}).", humanize(history, &w.name), loser_or_victor));
            ("a war poem: the march, a battle and the return", w.ended.map(|d| d.year).unwrap_or(w.started.year))
        } else {
            ("a poem about their homeland in one season", f.birth_date.year + 30)
        };
        poems.push(Commission {
            key: format!("poem-p{}", f.id.0),
            kind: WritingKind::Poem,
            subject: Subject::Figure(f.id.0),
            author: Some(f.id.0),
            author_name: figure_name(history, f.id),
            year,
            form: format!("{}, 10 to 18 lines, in the poet's own voice", theme),
            voice: voice(&rt).into(),
            facts,
        });
    }

    // Legends of beasts: those that scarred the land, then the worst raiders.
    let mut raids: crate::history::det::HashMap<u64, (usize, Vec<String>)> = Default::default();
    for e in events.iter().filter(|e| e.event_type == EventType::MonsterRaid) {
        for p in &e.primary_participants {
            if let EntityId::LegendaryCreature(c) = p {
                let r = raids.entry(c.0).or_default();
                r.0 += 1;
                if let Some(target) = e.title.split(" raids ").nth(1) { if !r.1.iter().any(|t| t == target) { r.1.push(target.to_string()); } }
            }
        }
    }
    let scar_of = |id: u64| history.ecology.as_ref().and_then(|eco| eco.scars.iter().find(|s| matches!(s.source, crate::history::ecology::ScarSource::Lair(c) | crate::history::ecology::ScarSource::Carcass(c) if c == id)).cloned());
    let mut beasts: Vec<_> = history.legendary_creatures.values().filter(|c| raids.get(&c.id.0).map_or(0, |r| r.0) >= 3 || scar_of(c.id.0).is_some()).collect();
    beasts.sort_by_key(|c| std::cmp::Reverse(raids.get(&c.id.0).map_or(0, |r| r.0) + 50 * scar_of(c.id.0).is_some() as usize));
    for c in beasts {
        let Some((x, y)) = c.lair_location else { continue };
        let species = history.creature_species.get(&c.species_id);
        let mut facts = vec![format!(
            "The beast: {}, a {} {}{}.",
            c.full_name(),
            species.map(|s| format!("{:?}", s.size).to_lowercase()).unwrap_or_default(),
            species.map(|s| s.name.clone()).unwrap_or_else(|| "creature".into()),
            if c.unique_abilities.is_empty() { String::new() } else { format!(" with the powers of {}", c.unique_abilities.iter().map(|a| words(&format!("{:?}", a))).collect::<Vec<_>>().join(" and ")) },
        )];
        facts.push(format!("Its lair: {}.", geography(world, gaz, x, y)));
        if let Some((n, targets)) = raids.get(&c.id.0) {
            let often = if *n >= 10 { "again and again, for generations" } else if *n >= 5 { "many times" } else { "more than once" };
            facts.push(format!("It raided {}; places it struck: {}.", often, targets.iter().take(4).cloned().collect::<Vec<_>>().join(", ")));
        }
        if let Some(e) = events.iter().find(|e| e.event_type == EventType::CreatureSlain && e.primary_participants.contains(&EntityId::LegendaryCreature(c.id))) {
            facts.push(format!("Its end: {}", humanize(history, &e.description)));
        } else {
            facts.push("It is said to live still.".into());
        }
        if let Some(s) = scar_of(c.id.0) {
            let told = events.iter().find(|e| e.id == s.event).map(|e| e.description.clone()).unwrap_or_default();
            facts.push(format!("What it left on the land: {}", humanize(history, &told)));
        }
        let teller = raids.get(&c.id.0).and_then(|r| r.1.first().cloned());
        legends.push(Commission {
            key: format!("legend-b{}", c.id.0),
            kind: WritingKind::Legend,
            subject: Subject::Beast(c.id.0),
            author: None,
            author_name: teller.map(|t| format!("as told in {}", t)).unwrap_or_else(|| "a folk tale".into()),
            year: history.current_date.year,
            form: "a folk tale of 120 to 180 words in prose, as villagers tell it to children by the fire".into(),
            voice: "old storytellers: concrete, eerie, with one vivid image and a warning at the end".into(),
            facts,
        });
    }

    // Laments for razed towns, the greatest first.
    let mut razings: Vec<&Event> = events.iter().filter(|e| e.event_type == EventType::SettlementDestroyed).collect();
    razings.sort_by_key(|e| std::cmp::Reverse(e.primary_participants.iter().filter_map(|p| if let EntityId::Settlement(s) = p { history.settlements.get(s).map(|s| s.population_cap) } else { None }).max().unwrap_or(0)));
    for e in razings {
        let Some(s) = e.primary_participants.iter().find_map(|p| if let EntityId::Settlement(s) = p { history.settlements.get(s) } else { None }) else { continue };
        let (race, rt) = race_of_faction(history, s.faction).unwrap_or_else(|| (String::new(), RaceType::Human));
        laments.push(Commission {
            key: format!("lament-s{}", s.id.0),
            kind: WritingKind::Lament,
            subject: Subject::Settlement(s.id.0),
            author: None,
            author_name: format!("a survivor of {}", s.name),
            year: e.date.year + 1,
            form: "a lament of 10 to 16 lines by one who escaped, remembering particular streets, people and sounds".into(),
            voice: voice(&rt).into(),
            facts: vec![
                format!("The town: {}, home of {} ({}), {}.", s.name, people(history, s.faction), race, geography(world, gaz, s.location.0, s.location.1)),
                format!("Its fall: {}", humanize(history, &e.description)),
            ],
        });
    }

    // Artifact lore for the most storied treasures.
    let mut arts: Vec<_> = history.artifacts.values().collect();
    arts.sort_by_key(|a| std::cmp::Reverse(a.historical_importance as usize + 20 * a.owner_history.len() + a.involved_in.len()));
    for a in arts.into_iter().take(80) {
        let creator = a.creator.map(|c| figure_name(history, c));
        let owners: Vec<String> = a.owner_history.iter().filter_map(|(o, _, _, how)| match o {
            EntityId::Figure(f) => Some(format!("{} ({})", figure_name(history, *f), words(&format!("{:?}", how)))),
            EntityId::Faction(f) => Some(format!("{} ({})", people(history, *f), words(&format!("{:?}", how)))),
            _ => None,
        }).take(5).collect();
        let mut facts = vec![format!("The treasure: {}, a {} {}.", a.name, words(&format!("{:?}", a.quality)), words(&format!("{:?}", a.item_type)))];
        if !a.description.is_empty() { facts.push(format!("As recorded: {}", humanize(history, &a.description))); }
        if let Some(c) = &creator { facts.push(format!("Made by {}.", c)); }
        if let Some((x, y)) = a.creation_location { facts.push(format!("Made in {}.", geography(world, gaz, x, y))); }
        if !owners.is_empty() { facts.push(format!("Passed through the hands of: {}.", owners.join(", "))); }
        if a.destroyed { facts.push("It was destroyed.".into()); } else if a.lost { facts.push("It is lost; no one knows where.".into()); }
        lore.push(Commission {
            key: format!("lore-a{}", a.id.0),
            kind: WritingKind::ArtifactLore,
            subject: Subject::Artifact(a.id.0),
            author: a.creator.map(|c| c.0),
            author_name: creator.map(|c| format!("inscribed by {}", c)).unwrap_or_else(|| "inscription".into()),
            year: a.creation_date.year,
            form: "the words inscribed on it (2 to 4 short lines), then a blank line, then 60 to 100 words of prose telling its story as a loremaster would".into(),
            voice: "a loremaster: precise, grave, fond of the object".into(),
            facts,
        });
    }

    // Ballads of bone fields and other battle scars.
    if let Some(eco) = &history.ecology {
        for s in eco.scars.iter().filter(|s| matches!(s.source, crate::history::ecology::ScarSource::Battlefield(..))) {
            let Some(told) = events.iter().find(|e| e.id == s.event) else { continue };
            let battles: Vec<String> = events.iter()
                .filter(|e| e.event_type == EventType::BattleFought && e.location == Some((s.x, s.y)))
                .filter_map(|e| e.factions_involved.first().map(|f| people(history, *f)))
                .collect::<std::collections::BTreeSet<_>>().into_iter().take(4).collect();
            ballads.push(Commission {
                key: format!("ballad-{}-{}", s.x, s.y),
                kind: WritingKind::Ballad,
                subject: Subject::Place(s.x, s.y),
                author: None,
                author_name: "a soldiers' ballad".into(),
                year: told.date.year,
                form: "a ballad of 12 to 20 lines with a refrain, as soldiers sing it".into(),
                voice: "rough, rhythmic, bitter and proud".into(),
                facts: vec![
                    format!("The place: {}.", tidy(&told.title)),
                    format!("What it became: {}", humanize(history, &told.description)),
                    format!("The land: {}.", geography(world, gaz, s.x, s.y)),
                    format!("Those who fought there: {}.", battles.join(", ")),
                ],
            });
        }
    }

    // Interleave the kinds so any budget gets a varied first harvest.
    let mut queues = [songs, poems, legends, laments, lore, ballads].map(|q| q.into_iter().filter(|c| !done.contains(c.key.as_str())).collect::<std::collections::VecDeque<_>>());
    let pattern = [0usize, 1, 2, 3, 4, 1, 5, 1, 2, 0];
    let mut out = Vec::new();
    loop {
        let mut any = false;
        for &k in &pattern {
            if let Some(c) = queues[k].pop_front() { out.push(c); any = true; }
        }
        if !any { break; }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// The model
// ---------------------------------------------------------------------------------------------

const SYSTEM: &str = "You are the bard of an invented world that has never met ours. You write \
one piece at a time from the facts you are given, in the voice you are given. Rules: write in \
English; use only the proper names in the facts and invent no new named people or places; never \
use modern words or game words (no 'faction', 'settlement', 'biome', 'tile', 'level', 'stats'); \
never write numbers of years, raids or battles; peoples are many persons, never one. The facts \
are background the writer knows, not a checklist: use a few, leave most unsaid, and never \
recite place descriptions as given. Do not explain or add notes. Answer in exactly this shape: \
a first line 'Title: <a short title>', a blank line, then the piece itself.";

/// A local model served by Ollama.
pub struct Bard {
    pub url: String,
    pub model: String,
    client: reqwest::blocking::Client,
}

impl Bard {
    pub fn new(url: &str, model: &str) -> Self {
        Self {
            url: url.trim_end_matches('/').to_string(),
            model: model.to_string(),
            client: reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(900)).build().expect("http client"),
        }
    }

    /// Check the server answers and has the model.
    pub fn check(&self) -> Result<(), String> {
        let tags: serde_json::Value = self.client.get(format!("{}/api/tags", self.url)).send()
            .map_err(|e| format!("cannot reach Ollama at {} ({e}); start it with `ollama serve`", self.url))?
            .json().map_err(|e| e.to_string())?;
        let has = tags["models"].as_array().map_or(false, |m| m.iter().any(|m| m["name"].as_str() == Some(&self.model)));
        if has { Ok(()) } else { Err(format!("model {} is not installed; run `ollama pull {}` or pass --bard-model", self.model, self.model)) }
    }

    pub fn prompt(c: &Commission) -> String {
        let mut p = format!("Write {}.\nVoice: {}.\nAttribution: {}.\nFacts:\n", c.form, c.voice, c.author_name);
        for f in &c.facts { p.push_str("- "); p.push_str(f); p.push('\n'); }
        p.push_str("Draw on the land and the particular names above; one concrete image is worth more than many abstractions.");
        p
    }

    /// One chat call constrained to a JSON schema; returns the raw JSON text.
    pub fn chat_json(&self, system: &str, user: &str, schema: &serde_json::Value, max_tokens: u32) -> Result<String, String> {
        let body = serde_json::json!({
            "model": self.model,
            "stream": false,
            "think": false,
            "format": schema,
            "options": { "temperature": 0.8, "top_p": 0.95, "num_predict": max_tokens },
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": user },
            ],
        });
        let resp: serde_json::Value = self.client.post(format!("{}/api/chat", self.url)).json(&body).send()
            .map_err(|e| e.to_string())?
            .json().map_err(|e| e.to_string())?;
        resp["message"]["content"].as_str().map(|s| s.to_string()).ok_or_else(|| format!("unexpected reply: {resp}"))
    }

    pub fn write(&self, c: &Commission) -> Result<Writing, String> {
        let body = serde_json::json!({
            "model": self.model,
            "stream": false,
            "think": false,
            "options": { "temperature": 0.85, "top_p": 0.95, "num_predict": 700 },
            "messages": [
                { "role": "system", "content": SYSTEM },
                { "role": "user", "content": Self::prompt(c) },
            ],
        });
        let resp: serde_json::Value = self.client.post(format!("{}/api/chat", self.url)).json(&body).send()
            .map_err(|e| e.to_string())?
            .json().map_err(|e| e.to_string())?;
        let raw = resp["message"]["content"].as_str().ok_or_else(|| format!("unexpected reply: {resp}"))?;
        let (title, mut text) = parse_reply(raw);
        text = strip_signature(&text, &c.author_name);
        if text.trim().len() < 40 { return Err(format!("reply too short: {raw:?}")); }
        Ok(Writing {
            key: c.key.clone(),
            kind: c.kind,
            subject: c.subject,
            title,
            author: c.author,
            author_name: c.author_name.clone(),
            year: c.year,
            text,
            model: self.model.clone(),
        })
    }
}

/// Drop a closing signature line ("— Fner-dwoi the Brave") naming the attributed author.
pub fn strip_signature(text: &str, author: &str) -> String {
    if let Some((body, last)) = text.trim_end().rsplit_once('\n') {
        let last = last.trim();
        let sig = last.trim_start_matches(|c: char| c == '—' || c == '-' || c == '–' || c == '~').trim().to_lowercase();
        let author = author.to_lowercase();
        let signed = last.len() != sig.len() || author.contains(&sig) || sig.contains(&author);
        if !sig.is_empty() && signed && (author.contains(&sig) || sig.contains(&author)) {
            return body.trim_end().to_string();
        }
    }
    text.to_string()
}

/// Split "Title: X\n\ntext" (dropping any <think> block and stray markdown).
pub fn parse_reply(raw: &str) -> (String, String) {
    let mut s = raw.to_string();
    while let (Some(a), Some(b)) = (s.find("<think>"), s.find("</think>")) {
        if b < a { break; }
        s.replace_range(a..b + "</think>".len(), "");
    }
    let s = s.replace("**", "").replace("*", "");
    let mut lines = s.trim().lines();
    let first = lines.next().unwrap_or("").trim();
    let (title, rest) = match first.strip_prefix("Title:").or_else(|| first.strip_prefix("TITLE:")).or_else(|| first.strip_prefix("# ")) {
        Some(t) => {
            let title = t.trim().trim_matches('"').to_string();
            // Drop a repeated title line at the top of the body.
            let body: Vec<&str> = lines.skip_while(|l| l.trim().is_empty() || l.trim().trim_start_matches("Title:").trim().trim_matches('"') == title).collect();
            (title, body.join("\n"))
        }
        None => ("Untitled".to_string(), s.trim().to_string()),
    };
    (title, rest.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replies_are_split_into_title_and_text() {
        let (t, x) = parse_reply("<think>hmm</think>\nTitle: **The Birch Flood**\n\nIce ran down\nthe valley.");
        assert_eq!(t, "The Birch Flood");
        assert_eq!(x, "Ice ran down\nthe valley.");
        let (t, x) = parse_reply("Title: Ash\n\nTitle: Ash\n\nGrey rain.");
        assert_eq!((t.as_str(), x.as_str()), ("Ash", "Grey rain."));
        assert_eq!(strip_signature("Bones.\n\n— A soldiers' ballad", "a soldiers' ballad"), "Bones.");
        assert_eq!(strip_signature("Bones.\nStill here.", "a soldiers' ballad"), "Bones.\nStill here.");
        assert_eq!(numbers_to_words("after a siege of 1 seasons, 94 fell"), "after a siege of one season, many fell");
        let (t, x) = parse_reply("Ice ran down the valley.");
        assert_eq!(t, "Untitled");
        assert_eq!(x, "Ice ran down the valley.");
    }
}
