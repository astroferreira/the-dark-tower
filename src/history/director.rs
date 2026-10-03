//! The director: hand-crafted-feeling events written by a local LLM *during* history
//! generation, with real consequences.
//!
//! The procedural simulation produces the moments; the director picks the dramatic ones (a
//! contested succession, a razed capital, a slain beast, a holy war) within a budget, shows the
//! model a short dossier of the people involved, and asks for what happens next. The model
//! never edits state: it answers with a title, a chronicle paragraph and a few *effects* chosen
//! from a fixed menu (opinion shifts, war, peace, alliances, deaths, marriages, exiles,
//! defections, titles, epithets, named treasures, monuments, prosperity or ruin, conversion).
//! The validator resolves every name against the dossier's cast and clamps every amount; the
//! engine then applies the effects through the same paths the simulation uses (wars end with
//! `War::end`, dead rulers get a successor through `succeed`), so consequences ripple on.
//!
//! An event can also open a *thread* (prophecy, feud, curse, vow, secret) with a trigger the
//! simulation can check (years pass, a ruler dies, a war breaks out, a town falls). When the
//! trigger fires the director asks the model to pay it off, which is what turns single events
//! into arcs.
//!
//! The model sits behind the `Author` trait, so tests use a scripted author.

use std::collections::HashMap;

use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::history::civilizations::military::{War, WarCause};
use crate::history::data::GameData;
use crate::history::entities::traits::DeathCause;
use crate::history::events::types::{Event, EventType};
use crate::history::objects::artifacts::{AcquisitionMethod, Artifact, ArtifactQuality, ArtifactType};
use crate::history::objects::monuments::{Monument, MonumentPurpose, MonumentType};
use crate::history::time::Date;
use crate::history::world_state::WorldHistory;
use crate::history::{EntityId, EventId, FactionId, FigureId, ReligionId, SettlementId};
use crate::lore::bard::{folk, geography, voice};
use crate::lore::Gazetteer;
use crate::world::WorldData;

// ---------------------------------------------------------------------------------------------
// What the model returns
// ---------------------------------------------------------------------------------------------

/// One consequence, flat so a JSON schema can constrain it. `kind` decides how the other
/// fields are read (see `EFFECT_MENU`).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EffectSpec {
    pub kind: String,
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub amount: i32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub detail: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ThreadSpec {
    /// none | prophecy | feud | curse | vow | secret
    pub kind: String,
    #[serde(default)]
    pub summary: String,
    /// years | ruler_dies | figure_dies | war_between | settlement_falls
    #[serde(default)]
    pub trigger: String,
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub years: i32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Proposal {
    pub title: String,
    pub text: String,
    #[serde(default)]
    pub effects: Vec<EffectSpec>,
    #[serde(default)]
    pub thread: Option<ThreadSpec>,
}

const EFFECT_KINDS: [&str; 16] = [
    "opinion", "war", "peace", "alliance", "death", "crown", "marriage", "exile", "defect", "title",
    "epithet", "artifact", "monument", "population", "wealth", "convert",
];

const EXAMPLE: &str = "EXAMPLE from another world (for the shape only; never reuse its names):
{\"title\": \"The Salt Oath Broken\", \"text\": \"Queen Ilsa of the Marrow people had sworn on salt and iron to defend the river lords, but when the Grey Hill raiders burned the ford she kept her spears at home. Her uncle Dovan called it cowardice before the whole court and rode north with his household to serve the river lords instead. The river lords never forgave the Marrow crown, and the ford became a border of ash.\", \"effects\": [{\"kind\": \"opinion\", \"subject\": \"River Lords\", \"object\": \"Marrow\", \"amount\": -45}, {\"kind\": \"defect\", \"subject\": \"Dovan the Grim\", \"object\": \"River Lords\"}, {\"kind\": \"epithet\", \"subject\": \"Ilsa\", \"detail\": \"the Oathbreaker\"}], \"thread\": {\"kind\": \"feud\", \"summary\": \"The river lords swore the Marrow crown would answer for the ford.\", \"trigger\": \"war_between\", \"subject\": \"River Lords\", \"object\": \"Marrow\"}}";

const EFFECT_MENU: &str = "\
- opinion: people `subject` changes its feeling toward people `object` by `amount` (-60..60)
- war: people `subject` declares war on people `object`; `detail` = cause (territorial, succession, religious, resource, revenge, conquest, independence)
- peace: peoples `subject` and `object` end their war
- alliance: peoples `subject` and `object` swear an alliance
- death: figure `subject` dies; `detail` = how (battle, assassination, execution, duel, disease, magic, accident)
- crown: figure `subject` becomes ruler of their own people (seizing or inheriting the throne)
- marriage: figure `subject` weds figure `object`
- exile: figure `subject` is cast out of their people
- defect: figure `subject` goes over to people `object`
- title: figure `subject` gains the title `detail`
- epithet: figure `subject` is known ever after as `detail` (e.g. \"the Oathbreaker\")
- artifact: figure `subject` comes to own a new treasure called `name`; `object` = weapon, armor, crown, ring, amulet, staff, book, goblet, instrument or relic; `detail` = what it is
- monument: people `subject` raise a monument called `name` at their town `object`; `detail` = what it honours
- population: town `subject` grows or dwindles by `amount` percent (-50..30)
- wealth: people `subject` gains or loses `amount` treasure (-500..500)
- convert: people `subject` adopts the faith `object`";

/// JSON schema for Ollama's constrained decoding.
pub fn proposal_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "title": { "type": "string" },
            "text": { "type": "string" },
            "effects": {
                "type": "array",
                "maxItems": 4,
                "items": {
                    "type": "object",
                    "properties": {
                        "kind": { "type": "string", "enum": EFFECT_KINDS },
                        "subject": { "type": "string" },
                        "object": { "type": "string" },
                        "amount": { "type": "integer" },
                        "name": { "type": "string" },
                        "detail": { "type": "string" }
                    },
                    "required": ["kind", "subject", "object", "amount", "name", "detail"]
                }
            },
            "thread": {
                "type": "object",
                "properties": {
                    "kind": { "type": "string", "enum": ["none", "prophecy", "feud", "curse", "vow", "secret"] },
                    "summary": { "type": "string" },
                    "trigger": { "type": "string", "enum": ["years", "ruler_dies", "figure_dies", "war_between", "settlement_falls"] },
                    "subject": { "type": "string" },
                    "object": { "type": "string" },
                    "years": { "type": "integer" }
                },
                "required": ["kind"]
            }
        },
        "required": ["title", "text", "effects"]
    })
}

// ---------------------------------------------------------------------------------------------
// The author (model) and threads
// ---------------------------------------------------------------------------------------------

/// Something that turns a prompt into a proposal's JSON.
pub trait Author {
    fn propose(&mut self, system: &str, prompt: &str) -> Result<String, String>;
    fn name(&self) -> String;
}

/// A local model through Ollama.
pub struct OllamaAuthor(pub crate::lore::bard::Bard);

impl Author for OllamaAuthor {
    fn propose(&mut self, system: &str, prompt: &str) -> Result<String, String> {
        self.0.chat_json(system, prompt, &proposal_schema(), 900)
    }
    fn name(&self) -> String {
        self.0.model.clone()
    }
}

/// When a thread comes due.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Trigger {
    Year(u32),
    RulerDies { faction: FactionId, ruler: FigureId },
    FigureDies(FigureId),
    WarBetween(FactionId, FactionId),
    SettlementFalls(SettlementId),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Thread {
    pub id: u32,
    pub kind: String,
    pub summary: String,
    pub opened: u32,
    /// The authored event that opened it.
    pub origin: EventId,
    pub factions: Vec<FactionId>,
    pub trigger: Trigger,
    /// The authored event that closed it.
    pub resolved: Option<EventId>,
}

/// Everything the director leaves behind (saved with the world).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Tales {
    pub threads: Vec<Thread>,
    pub authored: Vec<EventId>,
    pub model: String,
}

// ---------------------------------------------------------------------------------------------
// The cast: who may be named, and how
// ---------------------------------------------------------------------------------------------

#[derive(Default)]
struct Cast {
    peoples: Vec<(String, FactionId)>,
    figures: Vec<(String, FigureId)>,
    towns: Vec<(String, SettlementId)>,
    faiths: Vec<(String, ReligionId)>,
}

fn norm(s: &str) -> String {
    s.trim().trim_start_matches("the ").trim_start_matches("The ").to_lowercase()
}

/// Find a cast member by the name the model used: exact (case-insensitive), else unique
/// containment either way ("Vea'sienn" for "Vea'sienn the Founder").
fn lookup<T: Copy>(list: &[(String, T)], name: &str) -> Option<T> {
    let n = norm(name);
    if n.is_empty() { return None; }
    if let Some((_, v)) = list.iter().find(|(k, _)| norm(k) == n) { return Some(*v); }
    let hits: Vec<T> = list.iter().filter(|(k, _)| { let k = norm(k); k.contains(&n) || n.contains(&k) }).map(|(_, v)| *v).collect();
    if hits.len() == 1 { Some(hits[0]) } else { None }
}

impl Cast {
    fn people(&self, n: &str) -> Option<FactionId> { lookup(&self.peoples, n) }
    fn figure(&self, n: &str) -> Option<FigureId> { lookup(&self.figures, n) }
    fn town(&self, n: &str) -> Option<SettlementId> { lookup(&self.towns, n) }
    fn faith(&self, n: &str) -> Option<ReligionId> { lookup(&self.faiths, n) }
}

/// "The Westdale Human" -> "Westdale" (the key the model uses for a people).
fn people_key(history: &WorldHistory, f: FactionId) -> String {
    crate::lore::bard::people(history, f).trim_start_matches("the ").trim_end_matches(" people").to_string()
}

fn figure_label(history: &WorldHistory, id: FigureId) -> String {
    history.figures.get(&id).map(|f| f.full_name()).unwrap_or_default()
}

// ---------------------------------------------------------------------------------------------
// The director
// ---------------------------------------------------------------------------------------------

pub struct Director {
    author: Box<dyn Author>,
    gaz: Gazetteer,
    /// How many events may be authored over the whole history.
    pub budget: usize,
    start_year: u32,
    years: u32,
    scanned: usize,
    last_for: HashMap<FactionId, u32>,
    pub tales: Tales,
    next_thread: u32,
    /// Print each authored event as it happens.
    pub verbose: bool,
    pub failures: usize,
    /// Kinds of moment recently written about, to keep the story varied.
    recent_kinds: Vec<EventType>,
}

const SYSTEM: &str = "You are the chronicler and dramatist of an invented world with no contact \
with ours. The world's history is being simulated; at its turning points you write what happens \
next, as if a novelist had planned it: driven by the characters' natures and grudges, specific, \
surprising but plausible, with consequences. Rules: use only the names in the cast and do not \
invent named people (treasures and monuments you create may get new names); the chronicle text \
is three to five sentences in the past tense, plain and vivid, with no digits and no game words \
(no 'faction', 'settlement', 'opinion', 'stats'); choose one to four effects that the text \
describes, from the menu, referring to the cast exactly as listed; open a thread only when the \
event plants something that should come back later, and say in the summary what is foretold or \
sworn. Reply with JSON only.";

impl Director {
    pub fn new(author: Box<dyn Author>, world: &WorldData, budget: usize, start_year: u32, years: u32) -> Self {
        let model = author.name();
        Self {
            author,
            gaz: crate::lore::build_gazetteer(world, None, world.seed()),
            budget,
            start_year,
            years: years.max(1),
            scanned: 0,
            last_for: HashMap::new(),
            tales: Tales { model, ..Default::default() },
            next_thread: 1,
            verbose: true,
            failures: 0,
            recent_kinds: Vec::new(),
        }
    }

    fn written(&self) -> usize {
        self.tales.authored.len()
    }

    /// May the director spend an event now? Paced so the budget lasts the whole history.
    fn can_spend(&self, year: u32) -> bool {
        if self.written() >= self.budget { return false; }
        let elapsed = year.saturating_sub(self.start_year) as f32 / self.years as f32;
        (self.written() as f32) < self.budget as f32 * elapsed + 1.0
    }

    /// Run after each simulation step.
    pub fn step(&mut self, history: &mut WorldHistory, world: &WorldData, game_data: &GameData, rng: &mut impl Rng) {
        let date = history.current_date;
        // 1. Threads that have come due are paid off first.
        if let Some(t) = self.due_thread(history) {
            if self.written() < self.budget {
                let thread = self.tales.threads[t].clone();
                let origin = history.chronicle.events.iter().find(|e| e.id == thread.origin).map(|e| e.title.clone()).unwrap_or_default();
                let moment = format!(
                    "Long ago ({}), this was set in motion: {} Now the time has come: {}. Write how it comes to pass.",
                    origin, thread.summary, describe_trigger(history, &thread.trigger)
                );
                if let Some(id) = self.author_event(history, world, game_data, rng, &moment, &thread.factions, Some(thread.origin), false) {
                    self.tales.threads[t].resolved = Some(id);
                } else {
                    // Don't retry forever on a thread the model can't resolve.
                    self.tales.threads[t].resolved = Some(thread.origin);
                }
            }
        }

        // 2. The most dramatic thing that just happened.
        let new = &history.chronicle.events[self.scanned.min(history.chronicle.events.len())..];
        let mut best: Option<(i32, usize)> = None;
        for (k, e) in new.iter().enumerate() {
            if e.event_type == EventType::Authored { continue; }
            let mut score = drama(e);
            if score == 0 { continue; }
            let factions: Vec<FactionId> = e.factions_involved.iter().copied().filter(|f| history.factions.get(f).map_or(false, |f| f.is_active())).collect();
            if factions.is_empty() { continue; }
            if factions.iter().any(|f| self.last_for.get(f).map_or(false, |y| date.year < y + 12)) { score -= 5; }
            // The same kind of moment again soon reads as a formula.
            score -= 2 * self.recent_kinds.iter().filter(|k| **k == e.event_type).count() as i32;
            if best.map_or(true, |b| score > b.0) { best = Some((score, self.scanned + k)); }
        }
        self.scanned = history.chronicle.events.len();
        let Some((score, idx)) = best else { return };
        if score < 5 || !self.can_spend(date.year) { return; }
        let e = history.chronicle.events[idx].clone();
        self.recent_kinds.push(e.event_type.clone());
        if self.recent_kinds.len() > 5 { self.recent_kinds.remove(0); }
        let factions: Vec<FactionId> = e.factions_involved.iter().copied().filter(|f| history.factions.get(f).map_or(false, |f| f.is_active())).take(2).collect();
        let context = match e.event_type {
            EventType::CreatureSlain => " The slain beast was a monster that had preyed on these lands; its death is a deliverance and a hero's triumph.",
            EventType::SettlementDestroyed => " The town's own people mourn it; its destroyers may gloat or fear revenge.",
            _ => "",
        };
        let moment = format!("This has just happened: {}. {}{} Write the next turn of the story.", e.title, crate::lore::bard::humanize(history, &e.description), context);
        self.author_event(history, world, game_data, rng, &moment, &factions, Some(e.id), true);
        self.scanned = history.chronicle.events.len();
    }

    fn due_thread(&self, history: &WorldHistory) -> Option<usize> {
        let year = history.current_date.year;
        self.tales.threads.iter().position(|t| {
            if t.resolved.is_some() { return false; }
            match &t.trigger {
                Trigger::Year(y) => year >= *y,
                Trigger::RulerDies { ruler, .. } | Trigger::FigureDies(ruler) => history.figures.get(ruler).map_or(true, |f| !f.is_alive()),
                Trigger::WarBetween(a, b) => history.wars.values().any(|w| w.is_active() && ((w.aggressors.contains(a) && w.defenders.contains(b)) || (w.aggressors.contains(b) && w.defenders.contains(a)))),
                Trigger::SettlementFalls(s) => history.settlements.get(s).map_or(true, |s| s.is_destroyed()),
            }
        })
    }

    /// Ask the model, validate, apply. Returns the authored event's id.
    #[allow(clippy::too_many_arguments)]
    fn author_event(&mut self, history: &mut WorldHistory, world: &WorldData, game_data: &GameData, rng: &mut impl Rng, moment: &str, factions: &[FactionId], cause: Option<EventId>, may_open_thread: bool) -> Option<EventId> {
        // Whoever acted in the moment is in the cast, whatever their standing.
        let actors: Vec<FigureId> = cause.and_then(|c| history.chronicle.events.iter().rev().find(|e| e.id == c))
            .map(|e| e.primary_participants.iter().filter_map(|p| if let EntityId::Figure(f) = p { Some(*f) } else { None }).collect())
            .unwrap_or_default();
        let (cast, dossier) = build_dossier(history, world, &self.gaz, factions, &actors);
        let thread_rule = if may_open_thread {
            "THREAD (optional): kind none, or prophecy / feud / curse / vow / secret with a trigger: years (with years 5..80), ruler_dies (subject = people), figure_dies (subject = figure), war_between (subject, object = peoples), settlement_falls (subject = town)."
        } else {
            "THREAD: this event resolves an old thread, so set thread kind to none."
        };
        let recent_titles: Vec<String> = self.tales.authored.iter().rev().take(12)
            .filter_map(|id| history.chronicle.events.iter().rev().find(|e| e.id == *id).map(|e| e.title.clone()))
            .collect();
        let variety = if recent_titles.is_empty() { String::new() } else {
            format!("\n\nEarlier titles (do not reuse their words or their pattern, e.g. no more \"The Silence of...\"): {}", recent_titles.join("; "))
        };
        let prompt = format!(
            "THE MOMENT\n{}{}\n\n{}\nEFFECTS MENU (use the cast names exactly as listed above)\n{}\n\n{}\n\n{}\n\nNow write the event: a short evocative title naming what happened (never a year), the chronicle text, and its effects.",
            moment, variety, dossier, EFFECT_MENU, thread_rule, EXAMPLE,
        );
        if let Ok(path) = std::env::var("PLANET_DIRECTOR_LOG") {
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
                let _ = writeln!(f, "===== PROMPT (year {})\n{}", history.current_date.year, prompt);
            }
        }
        let raw = match self.author.propose(SYSTEM, &prompt) {
            Ok(r) => r,
            Err(e) => { self.failures += 1; if self.verbose { eprintln!("  director: model failed: {e}"); } return None; }
        };
        if let Ok(path) = std::env::var("PLANET_DIRECTOR_LOG") {
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
                let _ = writeln!(f, "===== REPLY\n{}\n", raw);
            }
        }
        let proposal: Proposal = match serde_json::from_str(&raw) {
            Ok(p) => p,
            Err(e) => { self.failures += 1; if self.verbose { eprintln!("  director: unreadable reply ({e})"); } return None; }
        };
        let title_is_date = proposal.title.to_lowercase().starts_with("the year") || proposal.title.to_lowercase().starts_with("year ");
        if proposal.title.trim().is_empty() || title_is_date || proposal.text.trim().len() < 20 {
            self.failures += 1;
            if self.verbose { eprintln!("  director: rejected reply (title {:?})", proposal.title); }
            return None;
        }

        let id = history.id_generators.next_event();
        let applied = apply_effects(history, game_data, rng, &cast, &proposal.effects, id);
        let date = history.current_date;
        let mut ev = Event::new(id, EventType::Authored, date, proposal.title.trim().to_string(), proposal.text.trim().to_string());
        ev.is_major = true;
        if let Some(c) = cause { ev = ev.caused_by(c); }
        for f in factions { ev = ev.with_faction(*f); }
        for p in &applied.participants { ev = ev.with_participant(p.clone()); }
        let loc = factions.first().and_then(|f| history.factions.get(f)).and_then(|f| f.capital).and_then(|c| history.settlements.get(&c)).map(|s| s.location);
        if let Some((x, y)) = loc { ev = ev.at_location(x, y); }
        history.chronicle.record(ev);
        if let Some((x, y)) = loc { history.tile_history.record_event(x, y, id); }
        for (fid, eid) in &applied.figure_events { if let Some(f) = history.figures.get_mut(fid) { f.events.push(*eid); } }
        self.tales.authored.push(id);
        for f in factions { self.last_for.insert(*f, date.year); }

        if may_open_thread {
            if let Some(t) = proposal.thread.as_ref().filter(|t| t.kind != "none" && !t.summary.trim().is_empty()) {
                if let Some(trigger) = resolve_trigger(history, &cast, t) {
                    self.tales.threads.push(Thread {
                        id: self.next_thread,
                        kind: t.kind.clone(),
                        summary: t.summary.trim().to_string(),
                        opened: date.year,
                        origin: id,
                        factions: factions.to_vec(),
                        trigger,
                        resolved: None,
                    });
                    self.next_thread += 1;
                }
            }
        }
        if self.verbose {
            eprintln!("  [{}/{}] year {}: {} ({})", self.written(), self.budget, date.year, proposal.title.trim(), applied.summary.join("; "));
        }
        Some(id)
    }
}

/// How much a moment deserves an authored sequel.
fn drama(e: &Event) -> i32 {
    use EventType::*;
    match e.event_type {
        FactionDestroyed => 10,
        SettlementDestroyed => 8,
        SuccessionCrisis | Coup => 7,
        Assassination if !e.title.starts_with("Failed") => 7,
        HolyWarDeclared | Plague | RulerDeposed => 6,
        WarEnded | Rebellion | FactionFounded => 5,
        CreatureSlain => 4,
        WarDeclared if !e.title.contains("joins") => 4,
        ReligionFounded => 4,
        RulerCrowned | AllianceFormed | ArtifactLost => 3,
        _ => 0,
    }
}

fn describe_trigger(history: &WorldHistory, t: &Trigger) -> String {
    match t {
        Trigger::Year(_) => "the appointed years have passed".into(),
        Trigger::RulerDies { faction, ruler } => format!("{} of the {} has died", figure_label(history, *ruler), people_key(history, *faction)),
        Trigger::FigureDies(f) => format!("{} has died", figure_label(history, *f)),
        Trigger::WarBetween(a, b) => format!("the {} and the {} are at war", people_key(history, *a), people_key(history, *b)),
        Trigger::SettlementFalls(s) => format!("{} has fallen", history.settlements.get(s).map(|s| s.name.clone()).unwrap_or_default()),
    }
}

fn resolve_trigger(history: &WorldHistory, cast: &Cast, t: &ThreadSpec) -> Option<Trigger> {
    let year = history.current_date.year;
    // Blank or unknown subjects fall back to the people at the centre of the moment.
    let main = cast.peoples.first().map(|p| p.1);
    match t.trigger.as_str() {
        "years" => Some(Trigger::Year(year + if t.years == 0 { 20 } else { t.years.clamp(5, 80) } as u32)),
        "ruler_dies" => {
            let f = cast.people(&t.subject).or(main)?;
            let ruler = history.factions.get(&f)?.current_leader?;
            Some(Trigger::RulerDies { faction: f, ruler })
        }
        "figure_dies" => cast.figure(&t.subject)
            .or_else(|| main.and_then(|f| history.factions.get(&f)).and_then(|f| f.current_leader))
            .map(Trigger::FigureDies),
        "war_between" => {
            let (a, b) = (cast.people(&t.subject)?, cast.people(&t.object)?);
            (a != b).then_some(Trigger::WarBetween(a, b))
        }
        "settlement_falls" => cast.town(&t.subject).map(Trigger::SettlementFalls),
        // No trigger given: let it ripen for a generation.
        _ => Some(Trigger::Year(year + 25)),
    }
}

// ---------------------------------------------------------------------------------------------
// Dossier
// ---------------------------------------------------------------------------------------------

/// The cast the model may name, and the situation in words.
fn build_dossier(history: &WorldHistory, world: &WorldData, gaz: &Gazetteer, factions: &[FactionId], actors: &[FigureId]) -> (Cast, String) {
    let mut cast = Cast::default();
    let mut d = String::from("Cast and situation:\n");
    let year = history.current_date.year;
    for &fid in factions {
        let Some(f) = history.factions.get(&fid) else { continue };
        let key = people_key(history, fid);
        cast.peoples.push((key.clone(), fid));
        let race = history.races.get(&f.race_id);
        let faith = f.state_religion.and_then(|r| history.religions.get(&r)).map(|r| r.name.clone());
        d.push_str(&format!(
            "\nPEOPLE {} ({}), a {}. Their voice: {}.{}\n",
            key,
            race.map(|r| r.name.clone()).unwrap_or_default(),
            spaced(&format!("{:?}", f.government)),
            race.map(|r| voice(&r.base_type)).unwrap_or(""),
            faith.as_ref().map(|r| format!(" Faith: {}.", r)).unwrap_or_default(),
        ));
        if let Some(r) = f.state_religion.and_then(|r| history.religions.get(&r)) { cast.faiths.push((r.name.clone(), r.id)); }
        // Towns: the seat and the largest others.
        let mut towns: Vec<_> = f.settlements.iter().filter_map(|s| history.settlements.get(s)).filter(|s| !s.is_destroyed()).collect();
        towns.sort_by_key(|s| std::cmp::Reverse((Some(s.id) == f.capital, s.population)));
        for (k, s) in towns.iter().take(3).enumerate() {
            cast.towns.push((s.name.clone(), s.id));
            let land = if k == 0 { format!(": {}", geography(world, gaz, s.location.0, s.location.1)) } else { String::new() };
            d.push_str(&format!("  TOWN {}{}{}\n", s.name, if Some(s.id) == f.capital { " (seat)" } else { "" }, land));
        }
        // People: the ruler first, then the most storied living figures.
        let mut figs: Vec<_> = history.figures.values().filter(|p| p.faction == Some(fid) && p.is_alive()).collect();
        figs.sort_by_key(|p| std::cmp::Reverse((Some(p.id) == f.current_leader, actors.contains(&p.id), p.events.len() + 3 * p.kills.len())));
        let shown: Vec<_> = figs.iter().take(4).copied()
            .chain(figs.iter().skip(4).filter(|p| actors.contains(&p.id)).copied())
            .collect();
        for p in shown {
            let name = p.full_name();
            cast.figures.push((name.clone(), p.id));
            let spouse = p.spouse.and_then(|s| history.figures.get(&s)).map(|s| format!(", wed to {}", s.full_name())).unwrap_or_default();
            let titles = if p.titles.is_empty() { String::new() } else { format!(", {}", p.titles.iter().take(2).cloned().collect::<Vec<_>>().join(", ")) };
            d.push_str(&format!(
                "  FIGURE {}{}: {} years old, {}{}{}\n",
                name, if Some(p.id) == f.current_leader { " (ruler)" } else { "" },
                p.age_at(&history.current_date), p.personality.dominant_trait(), titles, spouse,
            ));
        }
        // Feelings toward others.
        let mut rel: Vec<_> = f.relations.iter().filter(|(o, _)| history.factions.get(o).map_or(false, |o| o.is_active())).collect();
        rel.sort_by_key(|(_, r)| -(r.opinion.abs()));
        let feelings: Vec<String> = rel.iter().take(3).map(|(o, r)| format!("{} {:?} ({})", people_key(history, **o), r.stance, feeling(r.opinion))).collect();
        if !feelings.is_empty() { d.push_str(&format!("  Toward others: {}\n", feelings.join("; "))); }
        let wars: Vec<String> = history.wars.values().filter(|w| w.is_active())
            .filter_map(|w| if w.aggressors.contains(&fid) { w.defenders.first() } else if w.defenders.contains(&fid) { w.aggressors.first() } else { None })
            .map(|o| people_key(history, *o)).collect();
        d.push_str(&format!("  At war with: {}\n", if wars.is_empty() { "nobody".to_string() } else { wars.join(", ") }));
        // What happened to them lately.
        let recent: Vec<String> = history.chronicle.events.iter().rev()
            .filter(|e| e.factions_involved.contains(&fid) && e.date.year + 30 >= year && drama(e) > 0 || (e.event_type == EventType::Authored && e.factions_involved.contains(&fid)))
            .take(6)
            .map(|e| format!("{} ({} years ago)", crate::lore::bard::humanize(history, &e.title), year.saturating_sub(e.date.year)))
            .collect();
        if !recent.is_empty() { d.push_str(&format!("  Lately: {}\n", recent.join("; "))); }
    }
    // Other peoples they are entangled with may be named in effects too.
    for &fid in factions {
        if let Some(f) = history.factions.get(&fid) {
            let mut rel: Vec<_> = f.relations.iter().filter(|(o, _)| history.factions.get(o).map_or(false, |o| o.is_active())).collect();
            rel.sort_by_key(|(_, r)| -(r.opinion.abs()));
            for (o, _) in rel.into_iter().take(3) {
                if !cast.peoples.iter().any(|(_, x)| x == o) {
                    cast.peoples.push((people_key(history, *o), *o));
                }
            }
        }
    }
    let others: Vec<String> = cast.peoples.iter().skip(factions.len()).map(|(k, o)| {
        let race = history.factions.get(o).and_then(|f| history.races.get(&f.race_id)).map(|r| r.name.clone()).unwrap_or_default();
        let ruler = history.factions.get(o).and_then(|f| f.current_leader).map(|r| format!(", ruled by {}", figure_label(history, r))).unwrap_or_default();
        format!("{} ({}{})", k, race, ruler)
    }).collect();
    if !others.is_empty() { d.push_str(&format!("\nNEIGHBOURING PEOPLES (may be named as peoples in effects): {}\n", others.join("; "))); }
    d.push_str("Peoples already at war cannot declare war on each other again, and a people cannot war on itself.\n");
    d.push_str("In effects, name a people by its key (the name after PEOPLE, or before the brackets among the neighbours) and a figure by the full name listed after FIGURE.\n");
    let _ = folk;
    (cast, d)
}

/// "TribalCouncil" -> "tribal council".
fn spaced(camel: &str) -> String {
    let mut s = String::new();
    for (i, c) in camel.chars().enumerate() {
        if c.is_uppercase() && i > 0 { s.push(' '); }
        s.extend(c.to_lowercase());
    }
    s
}

fn feeling(op: i32) -> &'static str {
    match op {
        i32::MIN..=-60 => "hatred",
        -59..=-20 => "enmity",
        -19..=19 => "wary",
        20..=59 => "warm",
        _ => "devoted",
    }
}

// ---------------------------------------------------------------------------------------------
// Applying effects
// ---------------------------------------------------------------------------------------------

#[derive(Default)]
struct Applied {
    summary: Vec<String>,
    participants: Vec<EntityId>,
    figure_events: Vec<(FigureId, EventId)>,
}

fn war_cause(s: &str) -> WarCause {
    match s.trim().to_lowercase().as_str() {
        "territorial" => WarCause::Territorial,
        "succession" => WarCause::Succession,
        "religious" => WarCause::Religious,
        "resource" => WarCause::Resource,
        "conquest" => WarCause::Conquest,
        "independence" => WarCause::Independence,
        _ => WarCause::Revenge,
    }
}

fn death_cause(s: &str) -> DeathCause {
    match s.trim().to_lowercase().as_str() {
        "battle" => DeathCause::Battle,
        "assassination" => DeathCause::Assassination,
        "execution" => DeathCause::Execution,
        "duel" => DeathCause::Duel,
        "disease" => DeathCause::Disease,
        "magic" => DeathCause::Magic,
        "accident" => DeathCause::Accident,
        _ => DeathCause::Unknown,
    }
}

fn artifact_type(s: &str) -> ArtifactType {
    match s.trim().to_lowercase().as_str() {
        "weapon" => ArtifactType::Weapon,
        "armor" | "armour" => ArtifactType::Armor,
        "crown" => ArtifactType::Crown,
        "ring" => ArtifactType::Ring,
        "amulet" => ArtifactType::Amulet,
        "staff" => ArtifactType::Staff,
        "book" => ArtifactType::Book,
        "goblet" => ArtifactType::Goblet,
        "instrument" => ArtifactType::Instrument,
        _ => ArtifactType::Relic,
    }
}

fn at_war(history: &WorldHistory, a: FactionId, b: FactionId) -> Option<crate::history::WarId> {
    history.wars.values().find(|w| w.is_active() && ((w.aggressors.contains(&a) && w.defenders.contains(&b)) || (w.aggressors.contains(&b) && w.defenders.contains(&a)))).map(|w| w.id)
}

/// Clean a field the model garbled (a common slip under constrained decoding is to write the
/// next field inside the string: `"Elderkeep', 'amount': 30"`); returns the clean text and any
/// amount recovered from it.
fn clean_field(s: &str) -> (String, Option<i32>) {
    let amount = s.find("amount").and_then(|i| {
        let digits: String = s[i..].chars().skip_while(|c| !c.is_ascii_digit() && *c != '-').take_while(|c| c.is_ascii_digit() || *c == '-').collect();
        digits.parse().ok()
    });
    let mut t = s;
    for cut in ["',", "\",", "', ", "\n"] {
        if let Some(i) = t.find(cut) { t = &t[..i]; }
    }
    let t = t.trim().trim_matches(|c: char| c == '\'' || c == '"' || c == ',' || c.is_whitespace());
    (t.chars().take(80).collect(), amount)
}

/// Clean every field of an effect.
fn tidy_effect(fx: &EffectSpec) -> EffectSpec {
    let (subject, a1) = clean_field(&fx.subject);
    let (object, a2) = clean_field(&fx.object);
    let (name, a3) = clean_field(&fx.name);
    let detail = clean_field(&fx.detail).0;
    let amount = if fx.amount != 0 { fx.amount } else { a1.or(a2).or(a3).unwrap_or(0) };
    EffectSpec { kind: fx.kind.trim().to_lowercase(), subject, object, amount, name, detail }
}

/// Validate each effect against the cast and apply what holds; the rest is dropped.
fn apply_effects(history: &mut WorldHistory, game_data: &GameData, rng: &mut impl Rng, cast: &Cast, effects: &[EffectSpec], authored: EventId) -> Applied {
    let mut out = Applied::default();
    let date = history.current_date;
    let mut seen: Vec<EffectSpec> = Vec::new();
    for fx in effects.iter().take(4) {
        let fx = &tidy_effect(fx);
        // The same effect twice (a frequent repetition) counts once.
        if seen.contains(fx) { continue; }
        seen.push(fx.clone());
        let ok = match fx.kind.as_str() {
            "opinion" => match (cast.people(&fx.subject), cast.people(&fx.object)) {
                (Some(a), Some(b)) if a != b && fx.amount != 0 => {
                    let delta = fx.amount.clamp(-60, 60);
                    if let Some(f) = history.factions.get_mut(&a) { f.get_relation_mut(b, 0.0).adjust_opinion(delta); }
                    if let Some(f) = history.factions.get_mut(&b) { f.get_relation_mut(a, 0.0).adjust_opinion(delta / 2); }
                    out.summary.push(format!("opinion {} -> {} {:+}", fx.subject, fx.object, delta));
                    true
                }
                _ => false,
            },
            "war" => match (cast.people(&fx.subject), cast.people(&fx.object)) {
                (Some(a), Some(b)) if a != b && at_war(history, a, b).is_none() => {
                    let war_id = history.id_generators.next_war();
                    let cause = war_cause(&fx.detail);
                    let (na, nb) = (history.factions.get(&a).map(|f| f.name.clone()).unwrap_or_default(), history.factions.get(&b).map(|f| f.name.clone()).unwrap_or_default());
                    let name = format!("{:?} War of {} and {}", cause, na, nb);
                    let decl = history.id_generators.next_event();
                    let mut war = War::new(war_id, name.clone(), a, b, date, cause);
                    war.declaration_event = Some(decl);
                    history.wars.insert(war_id, war);
                    for (x, y) in [(a, b), (b, a)] {
                        if let Some(f) = history.factions.get_mut(&x) { f.wars.push(war_id); f.get_relation_mut(y, 0.0).declare_war(war_id); }
                    }
                    history.chronicle.record(Event::new(decl, EventType::WarDeclared, date, name, format!("{} declared war on {}.", na, nb)).with_faction(a).with_faction(b).caused_by(authored));
                    out.summary.push(format!("war {} on {}", fx.subject, fx.object));
                    true
                }
                _ => false,
            },
            "peace" => match (cast.people(&fx.subject), cast.people(&fx.object)) {
                (Some(a), Some(b)) => match at_war(history, a, b) {
                    Some(w) => {
                        if let Some(war) = history.wars.get_mut(&w) { war.end(date, None); }
                        for (x, y) in [(a, b), (b, a)] { if let Some(f) = history.factions.get_mut(&x) { f.get_relation_mut(y, 0.0).make_peace(); } }
                        out.summary.push(format!("peace {} / {}", fx.subject, fx.object));
                        true
                    }
                    None => false,
                },
                _ => false,
            },
            "alliance" => match (cast.people(&fx.subject), cast.people(&fx.object)) {
                (Some(a), Some(b)) if a != b && at_war(history, a, b).is_none() => {
                    for (x, y) in [(a, b), (b, a)] {
                        if let Some(f) = history.factions.get_mut(&x) {
                            let r = f.get_relation_mut(y, 0.0);
                            r.stance = crate::history::civilizations::diplomacy::DiplomaticStance::Allied;
                            r.adjust_opinion(20);
                        }
                    }
                    out.summary.push(format!("alliance {} / {}", fx.subject, fx.object));
                    true
                }
                _ => false,
            },
            "death" => match cast.figure(&fx.subject).filter(|f| history.figures.get(f).map_or(false, |f| f.is_alive())) {
                Some(fid) => {
                    let faction = history.figures.get(&fid).and_then(|f| f.faction);
                    if let Some(f) = history.figures.get_mut(&fid) { f.kill(date, death_cause(&fx.detail)); }
                    out.participants.push(EntityId::Figure(fid));
                    out.figure_events.push((fid, authored));
                    if let Some(fac) = faction {
                        if history.factions.get(&fac).map_or(false, |f| f.current_leader == Some(fid)) {
                            crate::history::simulation::step::succeed(history, fid, fac, game_data, rng);
                        }
                    }
                    out.summary.push(format!("death of {}", fx.subject));
                    true
                }
                None => false,
            },
            "crown" => match cast.figure(&fx.subject).and_then(|f| history.figures.get(&f)).filter(|f| f.is_alive()).and_then(|f| f.faction.map(|fac| (f.id, fac))) {
                Some((fid, fac)) if history.factions.get(&fac).map_or(false, |f| f.current_leader != Some(fid)) => {
                    let old = history.factions.get(&fac).and_then(|f| f.current_leader);
                    if let Some(f) = history.factions.get_mut(&fac) {
                        f.current_leader = Some(fid);
                        if !f.notable_figures.contains(&fid) { f.notable_figures.push(fid); }
                    }
                    let (who, people) = (figure_label(history, fid), history.factions.get(&fac).map(|f| f.name.clone()).unwrap_or_default());
                    let eid = history.id_generators.next_event();
                    let mut ev = Event::new(eid, EventType::RulerCrowned, date, format!("{} takes the throne", who), format!("{} became ruler of {}.", who, people))
                        .with_faction(fac).with_participant(EntityId::Figure(fid)).caused_by(authored);
                    if let Some(o) = old { ev = ev.with_participant(EntityId::Figure(o)); }
                    history.chronicle.record(ev);
                    out.participants.push(EntityId::Figure(fid));
                    out.figure_events.push((fid, authored));
                    out.summary.push(format!("{} crowned", fx.subject));
                    true
                }
                _ => false,
            },
            "marriage" => match (cast.figure(&fx.subject), cast.figure(&fx.object)) {
                (Some(a), Some(b)) if a != b && [a, b].iter().all(|x| history.figures.get(x).map_or(false, |f| f.is_alive() && f.spouse.is_none())) => {
                    if let Some(f) = history.figures.get_mut(&a) { f.spouse = Some(b); }
                    if let Some(f) = history.figures.get_mut(&b) { f.spouse = Some(a); }
                    out.participants.extend([EntityId::Figure(a), EntityId::Figure(b)]);
                    out.figure_events.extend([(a, authored), (b, authored)]);
                    out.summary.push(format!("marriage {} & {}", fx.subject, fx.object));
                    true
                }
                _ => false,
            },
            "exile" | "defect" => {
                let target = if fx.kind == "defect" { cast.people(&fx.object) } else { None };
                match cast.figure(&fx.subject).filter(|f| history.figures.get(f).map_or(false, |f| f.is_alive())) {
                    Some(fid) if fx.kind == "exile" || target.is_some() => {
                        let old = history.figures.get(&fid).and_then(|f| f.faction);
                        if target.is_some() && target == old { false } else {
                            if let Some(f) = history.figures.get_mut(&fid) { f.faction = target; }
                            out.participants.push(EntityId::Figure(fid));
                            out.figure_events.push((fid, authored));
                            if let Some(fac) = old {
                                if history.factions.get(&fac).map_or(false, |f| f.current_leader == Some(fid)) {
                                    // A ruler cast out or gone over is replaced as if dead.
                                    crate::history::simulation::step::succeed(history, fid, fac, game_data, rng);
                                }
                            }
                            out.summary.push(format!("{} {}", fx.kind, fx.subject));
                            true
                        }
                    }
                    _ => false,
                }
            }
            "title" | "epithet" => match cast.figure(&fx.subject) {
                // Titles and epithets are names ("the Oathbreaker", "Warden of the Ford"), not traits.
                Some(fid) if !fx.detail.trim().is_empty() && fx.detail.len() <= 60
                    && (fx.detail.starts_with(|c: char| c.is_uppercase()) || fx.detail.starts_with("the ")) => {
                    if let Some(f) = history.figures.get_mut(&fid) {
                        if fx.kind == "title" { f.titles.push(fx.detail.trim().to_string()); } else { f.epithet = Some(fx.detail.trim().to_string()); }
                    }
                    out.participants.push(EntityId::Figure(fid));
                    out.figure_events.push((fid, authored));
                    out.summary.push(format!("{} {}: {}", fx.kind, fx.subject, fx.detail.trim()));
                    true
                }
                _ => false,
            },
            "artifact" => match cast.figure(&fx.subject) {
                Some(fid) if !fx.name.trim().is_empty() && fx.name.len() <= 50 => {
                    let aid = history.id_generators.next_artifact();
                    let mut a = Artifact::new(aid, fx.name.trim().to_string(), artifact_type(&fx.object), ArtifactQuality::Legendary, date, None);
                    a.description = fx.detail.trim().to_string();
                    a.owner_history.push((EntityId::Figure(fid), date, None, AcquisitionMethod::Found));
                    a.current_owner = Some(EntityId::Figure(fid));
                    a.creation_event = Some(authored);
                    a.involved_in.push(authored);
                    history.artifacts.insert(aid, a);
                    if let Some(f) = history.figures.get_mut(&fid) { f.artifacts.push(aid); }
                    out.participants.extend([EntityId::Figure(fid), EntityId::Artifact(aid)]);
                    out.figure_events.push((fid, authored));
                    out.summary.push(format!("treasure {} for {}", fx.name.trim(), fx.subject));
                    true
                }
                _ => false,
            },
            "monument" => match (cast.people(&fx.subject), cast.town(&fx.object)) {
                (Some(fac), Some(sid)) if !fx.name.trim().is_empty() && fx.name.len() <= 60 => {
                    let loc = history.settlements.get(&sid).map(|s| s.location).unwrap_or((0, 0));
                    let mid = history.id_generators.next_monument();
                    let mut m = Monument::new(mid, fx.name.trim().to_string(), MonumentType::Memorial, loc, fac, date, MonumentPurpose::HonorDead);
                    m.construction_event = Some(authored);
                    history.monuments.insert(mid, m);
                    if let Some(s) = history.settlements.get_mut(&sid) { s.monuments.push(mid); }
                    out.participants.push(EntityId::Monument(mid));
                    out.summary.push(format!("monument {}", fx.name.trim()));
                    true
                }
                _ => false,
            },
            "population" => match cast.town(&fx.subject).filter(|s| fx.amount != 0 && history.settlements.get(s).map_or(false, |s| !s.is_destroyed())) {
                Some(sid) => {
                    let pct = fx.amount.clamp(-50, 30);
                    let (old, fac) = history.settlements.get(&sid).map(|s| (s.population, s.faction)).unwrap();
                    let new = ((old as f32) * (1.0 + pct as f32 / 100.0)).max(1.0) as u32;
                    if let Some(s) = history.settlements.get_mut(&sid) { s.population = new; }
                    if let Some(f) = history.factions.get_mut(&fac) { f.total_population = f.total_population.saturating_sub(old).saturating_add(new); }
                    out.participants.push(EntityId::Settlement(sid));
                    out.summary.push(format!("{} {:+}%", fx.subject, pct));
                    true
                }
                None => false,
            },
            "wealth" => match cast.people(&fx.subject).filter(|_| fx.amount != 0) {
                Some(fac) => {
                    let delta = fx.amount.clamp(-500, 500);
                    if let Some(f) = history.factions.get_mut(&fac) { f.wealth = (f.wealth as i64 + delta as i64).max(0) as u32; }
                    out.summary.push(format!("wealth {} {:+}", fx.subject, delta));
                    true
                }
                None => false,
            },
            "convert" => match (cast.people(&fx.subject), cast.faith(&fx.object)) {
                (Some(fac), Some(rel)) => {
                    if let Some(f) = history.factions.get_mut(&fac) { f.state_religion = Some(rel); }
                    out.summary.push(format!("{} converts to {}", fx.subject, fx.object));
                    true
                }
                _ => false,
            },
            _ => false,
        };
        if !ok { out.summary.push(format!("(dropped {} {})", fx.kind, fx.subject)); }
    }
    out
}

/// Advance the date like a real step would, for tests.
#[allow(dead_code)]
fn tick(history: &mut WorldHistory) {
    history.current_date = history.current_date.next();
    let _ = Date::new;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::config::HistoryConfig;
    use crate::history::simulation::setup::initialize_world;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    /// Replies with scripted proposals, in order, and records the prompts it was given.
    struct Scripted {
        replies: Vec<String>,
        prompts: Vec<String>,
    }
    impl Author for Scripted {
        fn propose(&mut self, _system: &str, prompt: &str) -> Result<String, String> {
            self.prompts.push(prompt.to_string());
            if self.replies.is_empty() { Err("no more replies".into()) } else { Ok(self.replies.remove(0)) }
        }
        fn name(&self) -> String { "scripted".into() }
    }

    fn world() -> WorldData {
        use crate::biomes::ExtendedBiome;
        use crate::plates::PlateId;
        use crate::scale::MapScale;
        use crate::seeds::WorldSeeds;
        use crate::tilemap::Tilemap;
        use crate::water_bodies::WaterBodyId;
        let (w, h) = (64, 32);
        let mut heightmap = Tilemap::new_with(w, h, 300.0);
        let mut biomes = Tilemap::new_with(w, h, ExtendedBiome::TemperateGrassland);
        for x in 0..w { *biomes.get_mut(x, 0) = ExtendedBiome::Ocean; *heightmap.get_mut(x, 0) = -100.0; }
        WorldData::new(
            WorldSeeds::from_master(11), MapScale::new(1.0), heightmap,
            Tilemap::new_with(w, h, 14.0), Tilemap::new_with(w, h, 0.5),
            biomes, Tilemap::new_with(w, h, 0.0), Tilemap::new_with(w, h, PlateId(0)), Vec::new(),
            None, Tilemap::new_with(w, h, WaterBodyId::NONE), Vec::new(), Tilemap::new_with(w, h, 0.0),
            None, None,
        )
    }

    fn setup() -> (WorldData, WorldHistory, GameData, ChaCha8Rng) {
        let mut rng = ChaCha8Rng::seed_from_u64(5);
        let world = world();
        let game_data = GameData::defaults();
        let config = HistoryConfig { initial_civilizations: 3, initial_legendary_creatures: 1, simulation_years: 5, prehistory_depth: 0, ..HistoryConfig::default() };
        let history = initialize_world(&world, config, &game_data, &mut rng);
        (world, history, game_data, rng)
    }

    #[test]
    fn effects_are_validated_and_applied_through_the_engine() {
        let (world, mut history, game_data, mut rng) = setup();
        let mut ids: Vec<FactionId> = history.factions.keys().copied().collect();
        ids.sort();
        let (a, b) = (ids[0], ids[1]);
        let ruler = history.factions[&a].current_leader.expect("a ruler");
        let (ka, kb) = (people_key(&history, a), people_key(&history, b));
        let ruler_name = figure_label(&history, ruler);
        let reply = serde_json::json!({
            "title": "The Poisoned Cup",
            "text": "At the feast of reconciliation the ruler drank from a cup sent by the neighbours, and did not rise again. The court swore vengeance before the body was cold.",
            "effects": [
                { "kind": "death", "subject": ruler_name, "detail": "assassination" },
                { "kind": "war", "subject": ka, "object": kb, "detail": "revenge" },
                { "kind": "marriage", "subject": "Nobody Real", "object": "Also Invented" },
                { "kind": "wealth", "subject": ka, "amount": 99999 }
            ],
            "thread": { "kind": "vow", "summary": "The heir vowed to burn the poisoners' seat.", "trigger": "years", "years": 20 }
        }).to_string();
        let mut director = Director::new(Box::new(Scripted { replies: vec![reply], prompts: vec![] }), &world, 5, 1, 100);
        director.verbose = false;
        let wealth_before = history.factions[&a].wealth;
        let id = director.author_event(&mut history, &world, &game_data, &mut rng, "Test moment.", &[a, b], None, true).expect("authored");

        assert!(!history.figures[&ruler].is_alive(), "the named ruler died");
        let heir = history.factions[&a].current_leader.expect("a successor was installed");
        assert_ne!(heir, ruler);
        assert!(history.figures[&heir].is_alive());
        assert!(at_war(&history, a, b).is_some(), "the war was declared through the engine");
        assert_eq!(history.factions[&a].wealth, wealth_before + 500, "amounts are clamped");
        let ev = history.chronicle.events.iter().find(|e| e.id == id).unwrap();
        assert_eq!(ev.event_type, EventType::Authored);
        assert!(history.chronicle.events.iter().any(|e| e.event_type == EventType::WarDeclared && e.triggered_by == Some(id)));
        assert_eq!(director.tales.threads.len(), 1, "the vow opened a thread");
    }

    #[test]
    fn threads_come_due_and_are_paid_off() {
        let (world, mut history, game_data, mut rng) = setup();
        let mut ids: Vec<FactionId> = history.factions.keys().copied().collect();
        ids.sort();
        let a = ids[0];
        let first = serde_json::json!({
            "title": "The Seer's Word", "text": "A blind seer told the court that a flood of fire would come within a generation, and the ruler laughed.",
            "effects": [], "thread": { "kind": "prophecy", "summary": "Fire will come within a generation.", "trigger": "years", "years": 5 }
        }).to_string();
        let payoff = serde_json::json!({
            "title": "The Fire Foretold", "text": "The fire the seer had promised came on a dry wind and took the granaries; the people remembered her words.",
            "effects": [], "thread": { "kind": "none" }
        }).to_string();
        let mut director = Director::new(Box::new(Scripted { replies: vec![first, payoff], prompts: vec![] }), &world, 10, 1, 100);
        director.verbose = false;
        director.author_event(&mut history, &world, &game_data, &mut rng, "Opening.", &[a], None, true).unwrap();
        assert_eq!(director.tales.threads.len(), 1);
        // Five years later the thread is due; the director pays it off before anything else.
        history.current_date = Date::new(history.current_date.year + 5, crate::seasons::Season::Spring);
        director.scanned = history.chronicle.events.len();
        director.step(&mut history, &world, &game_data, &mut rng);
        let t = &director.tales.threads[0];
        let resolved = t.resolved.expect("resolved");
        let ev = history.chronicle.events.iter().find(|e| e.id == resolved).unwrap();
        assert_eq!(ev.title, "The Fire Foretold");
        assert_eq!(ev.triggered_by, Some(t.origin), "the payoff links back to the prophecy");
    }

    #[test]
    fn unreadable_replies_change_nothing() {
        let (world, mut history, game_data, mut rng) = setup();
        let a = *history.factions.keys().next().unwrap();
        let before = history.chronicle.len();
        let mut director = Director::new(Box::new(Scripted { replies: vec!["not json".into()], prompts: vec![] }), &world, 5, 1, 100);
        director.verbose = false;
        assert!(director.author_event(&mut history, &world, &game_data, &mut rng, "x", &[a], None, true).is_none());
        assert_eq!(history.chronicle.len(), before);
        assert_eq!(director.failures, 1);
    }

    #[test]
    fn crown_makes_a_figure_ruler_and_traits_are_not_titles() {
        let (world, mut history, game_data, mut rng) = setup();
        let a = *history.factions.keys().min().unwrap();
        let ruler = history.factions[&a].current_leader.unwrap();
        // A rival of the same people.
        let rid = history.id_generators.next_figure();
        let mut rival = crate::history::entities::figures::Figure::new(rid, "Morwen Ashgrave".into(), history.factions[&a].race_id, Date::new(1, crate::seasons::Season::Spring), Default::default());
        rival.faction = Some(a);
        history.figures.insert(rid, rival);
        let reply = serde_json::json!({
            "title": "The Night of Knives", "text": "Morwen Ashgrave's household opened the gates at midnight and by dawn she sat the throne while the old ruler fled.",
            "effects": [
                { "kind": "crown", "subject": "Morwen Ashgrave", "object": "", "amount": 0, "name": "", "detail": "" },
                { "kind": "title", "subject": "Morwen Ashgrave", "object": "", "amount": 0, "name": "", "detail": "greed" }
            ]
        }).to_string();
        let mut director = Director::new(Box::new(Scripted { replies: vec![reply], prompts: vec![] }), &world, 5, 1, 100);
        director.verbose = false;
        director.author_event(&mut history, &world, &game_data, &mut rng, "Moment.", &[a], None, true).unwrap();
        assert_eq!(history.factions[&a].current_leader, Some(rid));
        assert_ne!(Some(ruler), history.factions[&a].current_leader);
        assert!(history.figures[&rid].titles.is_empty(), "a personality trait is not a title");
    }

    #[test]
    fn garbled_fields_are_cleaned() {
        let fx = EffectSpec { kind: "Opinion".into(), subject: "Basaltmount".into(), object: "Uustraalst the Valiant', 'amount': 30".into(), amount: 0, name: String::new(), detail: String::new() };
        let t = tidy_effect(&fx);
        assert_eq!((t.kind.as_str(), t.object.as_str(), t.amount), ("opinion", "Uustraalst the Valiant", 30));
        assert_eq!(clean_field("Elderkeep', ").0, "Elderkeep");
    }

    #[test]
    fn names_resolve_leniently_but_not_ambiguously() {
        let list = vec![("Vea'sienn the Founder".to_string(), 1), ("Westdale".to_string(), 2), ("West".to_string(), 3)];
        assert_eq!(lookup(&list, "vea'sienn"), Some(1));
        assert_eq!(lookup(&list, "the Westdale"), Some(2));
        assert_eq!(lookup(&list, "Nobody"), None);
    }
}
