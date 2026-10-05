//! The world's history written up as a book: a chronicler's annals, year by year through the
//! ages, followed by the peoples, the great wars, lives of note, beasts of legend and the
//! changing land. Output is a single self-contained HTML page (`--journal`, `J` in the tile
//! viewer).
//!
//! The chronicle holds tens of thousands of events, most of them routine (raids, treaties,
//! trade). The annals keep what shaped the world as entries and fold the routine into short
//! yearly or per-age tallies. Names of peoples, people and beasts that have an entry of their
//! own are linked, using each event's participants (not text matching).

use std::collections::BTreeMap;
use crate::history::det::{HashMap, HashSet};
use std::fmt::Write as _;

use crate::history::civilizations::military::War;
use crate::history::events::types::{Event, EventType};
use crate::history::world_state::WorldHistory;
use crate::history::{EntityId, EventId, FactionId, FigureId, LegendaryCreatureId};
use crate::lore::{FeatureKind, Gazetteer};
use crate::world::WorldData;

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            _ => o.push(c),
        }
    }
    o
}

/// "The The Highford Human" and similar doubled articles from name templates.
fn tidy(s: &str) -> String {
    s.replace("The The ", "The ").replace("the The ", "the ").replace(" the The ", " the ")
}

/// Strip a leading "The " so a name can follow "the" or sit in a list.
fn bare(name: &str) -> &str {
    name.strip_prefix("The ").unwrap_or(name)
}

/// Upper-case the first letter of the visible text (skipping any leading tag).
fn sentence(html: String) -> String {
    let mut out = String::with_capacity(html.len());
    let (mut in_tag, mut done) = (false, false);
    for c in html.chars() {
        if !done && !in_tag && c == '<' { in_tag = true; }
        if !done && !in_tag && c.is_alphabetic() {
            out.extend(c.to_uppercase());
            done = true;
            continue;
        }
        if in_tag && c == '>' { in_tag = false; }
        out.push(c);
    }
    out
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{} {}", n, if n == 1 { one } else { many })
}

/// A list in prose: "A", "A and B", "A, B and C", with "and N more" past `max`.
fn prose_list(items: &[String], max: usize) -> String {
    let shown: Vec<&String> = items.iter().take(max).collect();
    let more = items.len().saturating_sub(max);
    let mut s = match shown.len() {
        0 => String::new(),
        1 => shown[0].clone(),
        n => format!("{} and {}", shown[..n - 1].iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "), shown[n - 1]),
    };
    if more > 0 {
        s = format!("{}, {} and {} more", shown[..shown.len() - 1].iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "), shown[shown.len() - 1], more);
    }
    s
}

/// First number following `after` in `text` ("killing 54 people" -> 54).
fn number_after(text: &str, after: &str) -> Option<u32> {
    let i = text.find(after)? + after.len();
    let digits: String = text[i..].chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// Anchors for everything that has its own entry, and the names to link in text.
struct Links {
    factions: HashMap<FactionId, (String, String)>,
    figures: HashMap<FigureId, (String, String)>,
    beasts: HashMap<LegendaryCreatureId, (String, String)>,
}

impl Links {
    fn faction(&self, id: FactionId) -> String {
        match self.factions.get(&id) {
            Some((a, n)) => format!("<a href=\"#{}\">{}</a>", a, esc(n)),
            None => String::from("an unknown people"),
        }
    }

    /// Escaped `text` with the event's participants linked (first mention of each).
    fn text(&self, ev: &Event, text: &str) -> String {
        let mut names: Vec<(String, String)> = Vec::new();
        for f in &ev.factions_involved {
            if let Some((a, n)) = self.factions.get(f) { names.push((a.clone(), n.clone())); }
        }
        for p in &ev.primary_participants {
            let hit = match p {
                EntityId::Faction(f) => self.factions.get(f),
                EntityId::Figure(f) => self.figures.get(f),
                EntityId::LegendaryCreature(c) => self.beasts.get(c),
                _ => None,
            };
            if let Some((a, n)) = hit { names.push((a.clone(), n.clone())); }
        }
        self.link_names(&tidy(text), names)
    }

    fn link_names(&self, text: &str, mut names: Vec<(String, String)>) -> String {
        names.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
        names.dedup_by(|a, b| a.0 == b.0);
        // Swap names for placeholders first so a short name never matches inside a longer one.
        let mut out = text.to_string();
        let mut subs = Vec::new();
        for (anchor, name) in names {
            if name.len() < 3 { continue; }
            if let Some(i) = out.find(&name) {
                let key = format!("\u{1}{}\u{2}", subs.len());
                out.replace_range(i..i + name.len(), &key);
                subs.push(format!("<a href=\"#{}\">{}</a>", anchor, esc(&name)));
            }
        }
        let mut html = esc(&out);
        for (k, s) in subs.iter().enumerate() {
            html = html.replace(&format!("\u{1}{}\u{2}", k), s);
        }
        html
    }
}

/// A writing from the bard's library, set as verse (line breaks kept) or prose.
fn writing_html(idx: usize, w: &crate::lore::bard::Writing) -> String {
    let text = crate::lore::bard::strip_signature(&w.text, &w.author_name);
    let body = if w.kind.is_verse() {
        text.split("\n\n").map(|stanza| format!("<p>{}</p>", stanza.lines().map(|l| esc(l.trim())).collect::<Vec<_>>().join("<br>"))).collect::<String>()
    } else {
        text.split("\n\n").map(|para| format!("<p>{}</p>", esc(&para.replace('\n', " ")))).collect::<String>()
    };
    format!(
        "<figure class=\"writing{}\" id=\"w{}\"><figcaption><span class=\"w-title\">{}</span><span class=\"w-by\">{} · {}</span></figcaption><div class=\"w-body\">{}</div></figure>",
        if w.kind.is_verse() { " verse" } else { " prose" }, idx, esc(&w.title), esc(w.kind.label()), esc(&w.author_name), body
    )
}

/// What kind of entry an event makes (drives the filter chips and the margin mark).
fn kind_of(t: &EventType) -> &'static str {
    use EventType::*;
    match t {
        WarDeclared | WarEnded | BattleFought | SiegeBegun | SiegeEnded | Raid | Massacre | HolyWarDeclared => "war",
        RulerCrowned | RulerDeposed | SuccessionCrisis | Rebellion | Coup | Assassination | HeroDied | HeroBorn => "court",
        ReligionFounded | Miracle | TempleBuilt | TempleProfaned | CultFormed => "faith",
        CreatureAppeared | CreatureSlain | MonsterRaid | LairEstablished | LairDestroyed | QuestBegun | QuestCompleted => "beast",
        ForestCleared | GameScarce | WildlifeReturned | LandScarred => "land",
        VolcanoErupted | Earthquake | Flood | Drought | Plague | MagicalCatastrophe => "disaster",
        FactionFounded | SettlementFounded | SettlementGrew | PopulationMigrated => "found",
        FactionDestroyed | SettlementDestroyed => "fall",
        ArtifactCreated | ArtifactLost | ArtifactFound | ArtifactDestroyed | MasterworkCreated | MonumentBuilt
        | MonumentDestroyed | SpellInvented | MagicalExperiment | CurseApplied | CurseLifted => "craft",
        TreatySigned | TreatyBroken | AllianceFormed | AllianceBroken | TradeRouteEstablished => "peace",
        Authored => "story",
        _ => "other",
    }
}

const KINDS: [(&str, &str); 11] = [
    ("story", "Tales"),
    ("war", "War"),
    ("fall", "Falls"),
    ("found", "Foundings"),
    ("court", "Thrones"),
    ("faith", "Faith"),
    ("beast", "Beasts"),
    ("disaster", "Calamity"),
    ("land", "The land"),
    ("craft", "Works"),
    ("peace", "Peace"),
];

struct Entry {
    kind: &'static str,
    html: String,
}

/// Per-year tallies of routine events, written as one line each.
#[derive(Default)]
struct YearTally {
    raids: usize,
    raid_dead: u32,
    worst_raid: Option<(u32, String)>,
    villages: Vec<String>,
    crafted: Vec<String>,
    converted: Vec<String>,
    scarce: Vec<String>,
    treaties: usize,
    lesser_slain: usize,
    quarrels: Vec<String>,
}

#[derive(Default)]
struct AgeTally {
    trade: usize,
    goods: BTreeMap<String, usize>,
    found: usize,
    lost: usize,
    destroyed: usize,
    quests: usize,
    signs: usize,
}

pub fn render_journal(world: &WorldData, history: &WorldHistory, gaz: &Gazetteer) -> String {
    let events: &[Event] = &history.chronicle.events;
    let by_id: HashMap<EventId, &Event> = events.iter().map(|e| (e.id, e)).collect();
    use crate::lore::bard::{Subject, WritingKind};
    let empty = crate::lore::bard::Library::default();
    let library = history.library.as_ref().unwrap_or(&empty);
    let written = |subject: Subject| -> String {
        library.writings.iter().enumerate().filter(|(_, w)| w.subject == subject).map(|(i, w)| writing_html(i, w)).collect()
    };

    // --- Who gets an entry of their own ----------------------------------------------------
    let mut factions: Vec<_> = history.factions.values().collect();
    factions.sort_by_key(|f| (f.founded.year, f.id.0));

    let mut ruled: HashSet<FigureId> = HashSet::default();
    for e in events.iter().filter(|e| e.event_type == EventType::RulerCrowned) {
        for p in &e.primary_participants {
            if let EntityId::Figure(f) = p { ruled.insert(*f); }
        }
    }
    let mut lives: Vec<_> = history.figures.values()
        .filter(|f| ruled.contains(&f.id) || !f.kills.is_empty() || !f.titles.is_empty())
        .collect();
    lives.sort_by_key(|f| std::cmp::Reverse(f.events.len() + 3 * f.kills.len() + if ruled.contains(&f.id) { 5 } else { 0 }));
    lives.truncate(220);
    lives.sort_by_key(|f| (f.birth_date.year, f.id.0));

    let mut raids_by: HashMap<LegendaryCreatureId, (usize, u32)> = HashMap::default();
    let mut slain_by: HashMap<LegendaryCreatureId, &Event> = HashMap::default();
    for e in events {
        for p in &e.primary_participants {
            if let EntityId::LegendaryCreature(c) = p {
                match e.event_type {
                    EventType::MonsterRaid => {
                        let r = raids_by.entry(*c).or_default();
                        r.0 += 1;
                        r.1 += number_after(&e.description, "killing").unwrap_or(0);
                    }
                    EventType::CreatureSlain => { slain_by.insert(*c, e); }
                    _ => {}
                }
            }
        }
    }
    let scarred: HashSet<u64> = history.ecology.as_ref()
        .map(|eco| eco.scars.iter().filter_map(|s| match s.source {
            crate::history::ecology::ScarSource::Lair(id) | crate::history::ecology::ScarSource::Carcass(id) => Some(id),
            _ => None,
        }).collect())
        .unwrap_or_default();
    let mut beasts: Vec<_> = history.legendary_creatures.values()
        .filter(|c| raids_by.get(&c.id).map_or(0, |r| r.0) >= 3 || scarred.contains(&c.id.0))
        .collect();
    beasts.sort_by_key(|c| std::cmp::Reverse(raids_by.get(&c.id).map_or(0, |r| r.1) as usize + 400 * scarred.contains(&c.id.0) as usize));
    beasts.truncate(160);
    beasts.sort_by(|a, b| a.full_name().cmp(&b.full_name()));

    let links = Links {
        factions: factions.iter().map(|f| (f.id, (format!("f{}", f.id.0), tidy(&f.name)))).collect(),
        figures: lives.iter().map(|f| (f.id, (format!("p{}", f.id.0), f.name.clone()))).collect(),
        beasts: beasts.iter().map(|c| (c.id, (format!("b{}", c.id.0), c.full_name()))).collect(),
    };
    let wars_by_decl: HashMap<EventId, &War> = history.wars.values().filter_map(|w| w.declaration_event.map(|d| (d, w))).collect();

    // --- The ages (chapters) -----------------------------------------------------------------
    // Consecutive eras with the same name are one age.
    let mut ages: Vec<(String, u32, u32)> = Vec::new();
    for era in &history.timeline.eras {
        let name = era.name.split(" (Era").next().unwrap_or(&era.name).trim().to_string();
        let end = era.end.map(|d| d.year).unwrap_or(history.current_date.year);
        match ages.last_mut() {
            Some(last) if last.0 == name => last.2 = end,
            _ => ages.push((name, era.start.year, end)),
        }
    }
    if ages.is_empty() {
        ages.push(("The Age of Beginning".into(), 1, history.current_date.year));
    }
    // An age in which nothing was recorded (a quiet stretch before the first people) is not a book.
    while ages.len() > 1 && !events.iter().any(|e| e.date.year >= ages[0].1 && e.date.year <= ages[0].2) {
        ages.remove(0);
    }
    if let Some(first) = ages.first_mut() {
        first.1 = first.1.min(events.iter().map(|e| e.date.year).min().unwrap_or(1));
    }

    let mut body = String::new();
    let mut toc = String::new();
    let final_year = history.current_date.year;

    // Opening: the world itself.
    let continents: Vec<_> = { let mut v: Vec<_> = gaz.features.iter().filter(|f| f.kind == FeatureKind::Continent).collect(); v.sort_by_key(|f| std::cmp::Reverse(f.size)); v };
    let oceans: Vec<_> = { let mut v: Vec<_> = gaz.features.iter().filter(|f| f.kind == FeatureKind::Ocean).collect(); v.sort_by_key(|f| std::cmp::Reverse(f.size)); v };
    let ranges: Vec<_> = { let mut v: Vec<_> = gaz.features.iter().filter(|f| f.kind == FeatureKind::MountainRange).collect(); v.sort_by_key(|f| std::cmp::Reverse(f.size)); v };
    let peak = gaz.features.iter().filter(|f| f.kind == FeatureKind::Peak).max_by(|a, b| a.height_m.partial_cmp(&b.height_m).unwrap());
    let world_name = continents.first().map(|c| c.name.clone()).unwrap_or_else(|| "the World".into());
    let living = history.factions.values().filter(|f| f.is_active()).count();
    let razed = events.iter().filter(|e| e.event_type == EventType::SettlementDestroyed).count();

    let _ = write!(body, "<header class=\"title\" id=\"top\"><p class=\"eyebrow\">Being a true account of {} years</p><h1>The Annals of {}</h1>", final_year, esc(&world_name));
    let mut opening = format!(
        "These annals tell of the peoples of the world from the year {} to the year {}: how {} were founded and {} endure, how {} wars were fought and {} were razed, and what became of the land and its beasts.",
        ages.first().map(|a| a.1).unwrap_or(1), final_year,
        plural(history.factions.len(), "people", "peoples"), living,
        history.wars.len(), plural(razed, "town", "towns and cities"),
    );
    if !continents.is_empty() {
        let names: Vec<String> = continents.iter().take(4).map(|c| c.name.clone()).collect();
        let _ = write!(opening, " The great lands are {}", prose_list(&names, 4));
        if !oceans.is_empty() {
            let names: Vec<String> = oceans.iter().take(3).map(|c| c.name.clone()).collect();
            let _ = write!(opening, ", parted by {}", prose_list(&names, 3));
        }
        opening.push('.');
    }
    if let Some(p) = peak {
        let range = ranges.first().map(|r| format!(" Its longest mountains are {}.", r.name)).unwrap_or_default();
        let _ = write!(opening, " The highest summit is {}, {:.0} m above the sea.{}", p.name, p.height_m, range);
    }
    // The other wonders of the world (`lore::landmarks`), one clause each.
    {
        use super::landmarks::LandmarkKind as K;
        let marks = super::landmarks::find_landmarks(world, gaz);
        let clause = |k: K| marks.iter().find(|l| l.kind == k).map(|l| format!("{} ({})", l.name, l.detail));
        let mut wonders = Vec::new();
        if let Some(c) = clause(K::LongestRiver) { wonders.push(format!("the longest river is {c}")); }
        if let Some(c) = clause(K::LargestLake) { wonders.push(format!("the largest lake {c}")); }
        if let Some(c) = clause(K::GreatestWaterfall) { wonders.push(format!("the greatest falls {c}")); }
        if let Some(c) = clause(K::DeepestGorge) { wonders.push(format!("the deepest gorge {c}")); }
        if !wonders.is_empty() {
            let first = wonders.remove(0);
            let mut sentence = format!(" {}{}", first[..1].to_uppercase(), &first[1..]);
            if !wonders.is_empty() { let _ = write!(sentence, "; {}", wonders.join("; ")); }
            sentence.push('.');
            opening.push_str(&sentence);
        }
    }
    let _ = write!(body, "<p class=\"lede\">{}</p></header>", esc(&opening));

    let _ = write!(toc, "<li class=\"toc-part\">The Ages</li>");
    let mut yearly: BTreeMap<u32, Vec<Entry>> = BTreeMap::new();
    let mut tallies: BTreeMap<u32, YearTally> = BTreeMap::new();
    let notable_beast = |c: &LegendaryCreatureId| links.beasts.contains_key(c);

    // A people's founding is told once: the founder's own story (from the first crowning that
    // year), then the seat. The separate capital-founding and crowning entries are dropped.
    let mut founding_story: HashMap<FactionId, String> = HashMap::default();
    let mut folded: HashSet<EventId> = HashSet::default();
    for f in events.iter().filter(|e| e.event_type == EventType::FactionFounded && !is_revival(e)) {
        let Some(&fid) = f.factions_involved.first() else { continue };
        let fname = history.factions.get(&fid).map(|x| x.name.clone()).unwrap_or_default();
        for e in events.iter().filter(|e| e.date.year == f.date.year) {
            let same = e.factions_involved.contains(&fid) || (!fname.is_empty() && e.description.contains(&fname));
            match e.event_type {
                EventType::RulerCrowned if same && !founding_story.contains_key(&fid) => {
                    founding_story.insert(fid, links.text(e, &e.description));
                    folded.insert(e.id);
                }
                EventType::SettlementFounded if same && e.description.contains("capital") => { folded.insert(e.id); }
                _ => {}
            }
        }
    }

    for e in events {
        if folded.contains(&e.id) { continue; }
        let y = e.date.year;
        let kind = kind_of(&e.event_type);
        let mut push = |html: String| yearly.entry(y).or_default().push(Entry { kind, html });
        use EventType::*;
        match e.event_type {
            MonsterRaid => {
                let t = tallies.entry(y).or_default();
                t.raids += 1;
                let dead = number_after(&e.description, "killing").unwrap_or(0);
                t.raid_dead += dead;
                if t.worst_raid.as_ref().map_or(true, |w| dead > w.0) && dead > 0 {
                    t.worst_raid = Some((dead, links.text(e, &e.title)));
                }
            }
            TradeRouteEstablished | ArtifactFound | ArtifactLost | ArtifactDestroyed | QuestCompleted
            | QuestBegun | HeroBorn | LairEstablished | SiegeBegun => {}
            TreatySigned if e.primary_participants.is_empty() => tallies.entry(y).or_default().treaties += 1,
            Miracle if e.title.contains("converts to") => {
                tallies.entry(y).or_default().converted.push(links.text(e, &e.title.replace(" converts to ", " to ")));
            }
            Miracle if e.primary_participants.is_empty() => {}
            SettlementFounded if !e.description.contains("capital") => {
                let name = e.title.trim_start_matches("Founding of ").trim_end_matches(" founded").to_string();
                tallies.entry(y).or_default().villages.push(esc(&name));
            }
            ArtifactCreated => {
                let name = e.title.trim_start_matches("Creation of ").to_string();
                tallies.entry(y).or_default().crafted.push(format!("<i>{}</i>", esc(&name)));
            }
            GameScarce => {
                let place = e.title.trim_start_matches("Game grows scarce near ").to_string();
                tallies.entry(y).or_default().scarce.push(esc(&place));
            }
            CreatureSlain => {
                let notable = e.primary_participants.iter().any(|p| matches!(p, EntityId::LegendaryCreature(c) if notable_beast(c)));
                if notable { push(links.text(e, &e.description)); } else { tallies.entry(y).or_default().lesser_slain += 1; }
            }
            HeroDied => {
                let notable = e.primary_participants.iter().any(|p| matches!(p, EntityId::Figure(f) if links.figures.contains_key(f)));
                if notable { push(links.text(e, &e.description)); }
            }
            BattleFought if e.title.starts_with("Battle between") => {}
            Authored => push(format!("<span class=\"tale-title\" id=\"e{}\">{}</span> {}", e.id.0, esc(&e.title), links.text(e, &e.description))),
            FactionFounded => {
                let fid = e.factions_involved.first().copied();
                let seat = e.description.rsplit(" at ").next().map(|s| s.trim_end_matches('.').to_string());
                let html = match fid.and_then(|f| founding_story.get(&f)).filter(|_| !is_revival(e)) {
                    Some(story) => match &seat {
                        Some(seat) => format!("{} Its seat was {}.", story, esc(seat)),
                        None => story.clone(),
                    },
                    None => links.text(e, &e.description),
                };
                let song = fid.map(|f| written(Subject::Faction(f.0))).unwrap_or_default();
                push(format!("{}{}", html, song));
            }
            SettlementDestroyed => {
                let lament: String = e.primary_participants.iter().filter_map(|p| if let EntityId::Settlement(s) = p { Some(written(Subject::Settlement(s.0))) } else { None }).collect();
                push(format!("{}{}", links.text(e, &e.description), lament));
            }
            Raid if e.title.contains(" between ") => {
                // Quarrels between neighbours: "a dispute over gold between A and B".
                let what = e.title.split(" between ").next().unwrap_or("quarrel").to_string();
                let who = e.title.split(" between ").nth(1).unwrap_or("").to_string();
                tallies.entry(y).or_default().quarrels.push(format!("{} ({})", esc(&tidy(&who)), esc(&what)));
            }
            Assassination if e.title.starts_with("Failed") => {}
            WarEnded => {
                let war = e.triggered_by.and_then(|d| wars_by_decl.get(&d));
                let html = match war {
                    Some(w) => {
                        let years = e.date.year.saturating_sub(w.started.year);
                        let losses = w.casualties.aggressor_losses + w.casualties.defender_losses + w.casualties.civilian_losses;
                        let victor = w.victor.map(|v| format!("{} prevailed", links.faction(v))).unwrap_or_else(|| "neither side prevailed".into());
                        format!("<b>{}</b> ended after {} and {}; {}. {} fell.",
                            esc(&tidy(&w.name)), plural(years.max(1) as usize, "year", "years"), plural(w.battles.len(), "battle", "battles"), victor, losses)
                    }
                    None => links.text(e, &e.description),
                };
                push(html);
            }
            _ => push(links.text(e, &e.description)),
        }
    }

    for (age_no, (name, start, end)) in ages.iter().enumerate() {
        let in_age = |y: u32| y >= *start && (y <= *end || age_no + 1 == ages.len());
        let id = format!("age{}", age_no + 1);
        let _ = write!(toc, "<li><a href=\"#{}\">{}<span>{}–{}</span></a></li>", id, esc(name), start, end);

        // The age's headline: its gravest event.
        let rank = |t: &EventType| match t {
            EventType::FactionDestroyed => 6,
            EventType::HolyWarDeclared => 5,
            EventType::Plague => 4,
            EventType::SettlementDestroyed => 3,
            EventType::LandScarred => 2,
            EventType::WarDeclared => 1,
            _ => 0,
        };
        let headline = events.iter().filter(|e| in_age(e.date.year) && rank(&e.event_type) > 0)
            .max_by_key(|e| (rank(&e.event_type), std::cmp::Reverse(e.date.year)))
            .map(|e| tidy(&e.title));
        let count = |t: EventType| events.iter().filter(|e| in_age(e.date.year) && e.event_type == t).count();
        let mut age = AgeTally::default();
        for e in events.iter().filter(|e| in_age(e.date.year)) {
            match e.event_type {
                EventType::TradeRouteEstablished => {
                    age.trade += 1;
                    if let Some(i) = e.description.rfind(" for ") {
                        for good in e.description[i + 5..].trim_end_matches('.').split(|c| c == ',').flat_map(|s| s.split(" and ")) {
                            let good = good.trim().to_lowercase();
                            if !good.is_empty() && good.len() < 16 { *age.goods.entry(good).or_default() += 1; }
                        }
                    }
                }
                EventType::ArtifactFound => age.found += 1,
                EventType::ArtifactLost => age.lost += 1,
                EventType::ArtifactDestroyed => age.destroyed += 1,
                EventType::QuestCompleted => age.quests += 1,
                EventType::Miracle if !e.title.contains("converts to") => age.signs += 1,
                _ => {}
            }
        }
        let mut goods: Vec<_> = age.goods.iter().collect();
        goods.sort_by_key(|g| std::cmp::Reverse(*g.1));
        let goods: Vec<String> = goods.iter().take(3).map(|g| g.0.clone()).collect();
        let formal = history.wars.values().filter(|w| in_age(w.started.year)).count();
        let declared = events.iter().filter(|e| in_age(e.date.year) && e.event_type == EventType::WarDeclared && !e.title.contains("joins")).count();
        let wars_begun = formal.max(declared);
        let (rose, fell) = (count(EventType::FactionFounded), count(EventType::FactionDestroyed));
        let peoples = match (rose, fell) {
            (0, 0) => "no people rose or fell".to_string(),
            (r, 0) => format!("{} rose", plural(r, "people", "peoples")),
            (0, f) => format!("{} fell", plural(f, "people", "peoples")),
            (r, f) => format!("{} rose and {} fell", plural(r, "people", "peoples"), f),
        };
        let razed = count(EventType::SettlementDestroyed);
        let summary = format!(
            "In these {} years {}; {} began{}. {} were crowned. {} were opened{}. Heroes returned from {}, and {} and {} were recorded.",
            end.saturating_sub(*start) + 1,
            peoples,
            plural(wars_begun, "war", "wars"),
            if razed > 0 { format!(" and {} were razed", plural(razed, "town", "towns")) } else { String::new() },
            plural(count(EventType::RulerCrowned), "ruler", "rulers"),
            plural(age.trade, "trade road", "trade roads"),
            if goods.is_empty() { String::new() } else { format!(", chiefly for {}", prose_list(&goods, 3)) },
            plural(age.quests, "quest", "quests"),
            plural(age.signs, "sign or wonder", "signs and wonders"),
            plural(age.found + age.lost + age.destroyed, "treasure found, lost or broken", "treasures found, lost or broken"),
        );
        let _ = write!(body, "<section class=\"age\" id=\"{}\"><div class=\"age-head\"><p class=\"eyebrow\">Book {} · Years {}–{}</p><h2>{}</h2>{}<p class=\"age-sum\">{}</p></div>",
            id, roman(age_no + 1), start, end, esc(name),
            headline.map(|h| format!("<p class=\"age-headline\">{}</p>", esc(&h))).unwrap_or_default(),
            esc(&summary));
        let years: Vec<u32> = yearly.keys().chain(tallies.keys()).copied().filter(|y| in_age(*y)).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
        for y in years {
            let mut items = String::new();
            if let Some(list) = yearly.get(&y) {
                for en in list {
                    let _ = write!(items, "<li class=\"entry k-{}\" data-k=\"{}\">{}</li>", en.kind, en.kind, sentence(en.html.clone()));
                }
            }
            if let Some(t) = tallies.get(&y) {
                if !t.villages.is_empty() {
                    let _ = write!(items, "<li class=\"entry tally k-found\" data-k=\"found\">New settlements were founded at {}.</li>", prose_list(&t.villages, 6));
                }
                if !t.crafted.is_empty() {
                    let _ = write!(items, "<li class=\"entry tally k-craft\" data-k=\"craft\">Made this year: {}.</li>", prose_list(&t.crafted, 5));
                }
                if !t.converted.is_empty() {
                    let _ = write!(items, "<li class=\"entry tally k-faith\" data-k=\"faith\">Conversions: {}.</li>", prose_list(&t.converted, 4));
                }
                if !t.scarce.is_empty() {
                    let _ = write!(items, "<li class=\"entry tally k-land\" data-k=\"land\">Game grew scarce around {}.</li>", prose_list(&t.scarce, 6));
                }
                if t.raids > 0 {
                    let worst = t.worst_raid.as_ref().map(|w| format!(" The worst: {}, {} dead.", w.1, w.0)).unwrap_or_default();
                    let _ = write!(items, "<li class=\"entry tally k-beast\" data-k=\"beast\">Monsters raided {}, killing {}.{}{}</li>",
                        plural(t.raids, "time", "times"), t.raid_dead, worst,
                        if t.lesser_slain > 0 { format!(" {} slain by heroes.", plural(t.lesser_slain, "lesser beast was", "lesser beasts were")) } else { String::new() });
                } else if t.lesser_slain > 0 {
                    let _ = write!(items, "<li class=\"entry tally k-beast\" data-k=\"beast\">{} slain by heroes.</li>", plural(t.lesser_slain, "lesser beast was", "lesser beasts were"));
                }
                if !t.quarrels.is_empty() {
                    let _ = write!(items, "<li class=\"entry tally k-war\" data-k=\"war\">Quarrels soured {}.</li>", prose_list(&t.quarrels, 4));
                }
                if t.treaties > 0 {
                    let _ = write!(items, "<li class=\"entry tally k-peace\" data-k=\"peace\">{} signed.</li>", plural(t.treaties, "treaty was", "treaties were"));
                }
            }
            if !items.is_empty() {
                let _ = write!(body, "<div class=\"year\" id=\"y{}\"><div class=\"rubric\"><span class=\"anno\">Year</span>{}</div><ul>{}</ul></div>", y, y, items);
            }
        }
        body.push_str("</section>");
    }

    // --- The peoples ------------------------------------------------------------------------
    let _ = write!(toc, "<li class=\"toc-part\">The Record</li><li><a href=\"#peoples\">The Peoples<span>{}</span></a></li>", factions.len());
    let _ = write!(body, "<section class=\"part\" id=\"peoples\"><p class=\"eyebrow\">Part the Second</p><h2>The Peoples</h2><p class=\"part-intro\">Every people that rose in these years, in the order of their founding.</p>");
    for f in &factions {
        let race = history.races.get(&f.race_id).map(|r| r.name.clone()).unwrap_or_default();
        let founding = events.iter().find(|e| e.event_type == EventType::FactionFounded && e.factions_involved.contains(&f.id))
            .map(|e| links.text(e, &e.description)).unwrap_or_default();
        let rulers: Vec<String> = events.iter()
            .filter(|e| e.event_type == EventType::RulerCrowned && (e.factions_involved.contains(&f.id) || e.description.contains(&f.name)))
            .filter_map(|e| e.primary_participants.iter().find_map(|p| match p {
                EntityId::Figure(id) => history.figures.get(id).map(|fig| {
                    let name = links.figures.get(id).map(|(a, n)| format!("<a href=\"#{}\">{}</a>", a, esc(n))).unwrap_or_else(|| esc(&fig.name));
                    format!("{} <span class=\"yr\">{}</span>", name, e.date.year)
                }),
                _ => None,
            }))
            .collect();
        let wars: Vec<String> = f.wars.iter().filter_map(|w| history.wars.get(w)).map(|w| {
            let outcome = match (w.victor, w.ended) {
                (Some(v), _) if v == f.id => "won",
                (Some(_), _) => "lost",
                (None, Some(_)) => "drawn",
                (None, None) => "unended",
            };
            format!("{} <span class=\"yr\">{}, {}</span>", esc(&tidy(&w.name)), w.started.year, outcome)
        }).collect();
        let fate = match f.dissolved {
            Some(d) => format!("Ended in the year {}.", d.year),
            None => {
                let leader = f.current_leader.and_then(|l| history.figures.get(&l)).map(|l| format!(", ruled by {}", esc(&l.name))).unwrap_or_default();
                let faith = f.state_religion.and_then(|r| history.religions.get(&r)).map(|r| format!(", keeping {}", esc(&tidy(&r.name)))).unwrap_or_default();
                format!("Endures with {} and {} souls{}{}.", plural(f.settlements.len(), "settlement", "settlements"), f.total_population, leader, faith)
            }
        };
        let _ = write!(body, "<article class=\"card\" id=\"f{}\"><h3>{}</h3><p class=\"meta\">{} · {:?} · founded {}</p><p>{}</p>",
            f.id.0, esc(&tidy(&f.name)), esc(&race), f.government, f.founded.year, founding);
        if !rulers.is_empty() { let _ = write!(body, "<p><span class=\"label\">Rulers</span> {}</p>", rulers.join(" · ")); }
        if !wars.is_empty() { let _ = write!(body, "<p><span class=\"label\">Wars</span> {}</p>", wars.join(" · ")); }
        let _ = write!(body, "<p class=\"fate{}\">{}</p></article>", if f.dissolved.is_some() { " ended" } else { "" }, fate);
    }
    body.push_str("</section>");

    // --- The great wars ---------------------------------------------------------------------
    let mut wars: Vec<&War> = history.wars.values().collect();
    wars.sort_by_key(|w| std::cmp::Reverse(w.battles.len() * 10 + w.sieges.len() * 15 + (w.casualties.aggressor_losses + w.casualties.defender_losses) as usize / 50));
    wars.truncate(30);
    let _ = write!(toc, "<li><a href=\"#wars\">The Great Wars<span>{}</span></a></li>", wars.len());
    let _ = write!(body, "<section class=\"part\" id=\"wars\"><p class=\"eyebrow\">Part the Third</p><h2>The Great Wars</h2><p class=\"part-intro\">The {} longest and bloodiest of the {} wars.</p><div class=\"table-wrap\"><table><thead><tr><th>War</th><th>Years</th><th>Sides</th><th class=\"num\">Battles</th><th class=\"num\">Sieges</th><th class=\"num\">Fallen</th><th>Outcome</th></tr></thead><tbody>", wars.len(), history.wars.len());
    for w in &wars {
        let sides = format!("{} <span class=\"vs\">against</span> {}",
            w.aggressors.iter().map(|f| links.faction(*f)).collect::<Vec<_>>().join(", "),
            w.defenders.iter().map(|f| links.faction(*f)).collect::<Vec<_>>().join(", "));
        let outcome = match (w.victor, w.ended) {
            (Some(v), _) => format!("{} won", links.faction(v)),
            (None, Some(_)) => "No victor".into(),
            (None, None) => "Still fought".into(),
        };
        let fallen = w.casualties.aggressor_losses + w.casualties.defender_losses + w.casualties.civilian_losses;
        let _ = write!(body, "<tr><td><b>{}</b></td><td class=\"yrs\">{}–{}</td><td>{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td>{}</td></tr>",
            esc(&tidy(&w.name)), w.started.year, w.ended.map(|d| d.year.to_string()).unwrap_or_default(), sides, w.battles.len(), w.sieges.len(), fallen, outcome);
    }
    body.push_str("</tbody></table></div></section>");

    // --- Lives of note ----------------------------------------------------------------------
    let _ = write!(toc, "<li><a href=\"#lives\">Lives of Note<span>{}</span></a></li>", lives.len());
    let _ = write!(body, "<section class=\"part\" id=\"lives\"><p class=\"eyebrow\">Part the Fourth</p><h2>Lives of Note</h2><p class=\"part-intro\">Rulers, champions and slayers, in the order of their birth.</p><div class=\"cards\">");
    for f in &lives {
        let race = history.races.get(&f.race_id).map(|r| r.name.clone()).unwrap_or_default();
        let people = f.faction.map(|id| links.faction(id)).unwrap_or_else(|| "no people".into());
        let span = match (f.death_date, f.cause_of_death) {
            (Some(d), Some(c)) => format!("{}–{}, {}", f.birth_date.year, d.year, death_words(&format!("{:?}", c))),
            (Some(d), None) => format!("{}–{}", f.birth_date.year, d.year),
            _ => format!("born {}, living", f.birth_date.year),
        };
        let mut deeds: Vec<&Event> = f.events.iter().filter_map(|id| by_id.get(id).copied())
            .filter(|e| !matches!(e.event_type, EventType::HeroBorn | EventType::QuestBegun))
            .collect();
        deeds.sort_by_key(|e| e.date.total_seasons());
        let deeds: Vec<String> = deeds.iter().take(6).map(|e| format!("<li><span class=\"yr\">{}</span> {}</li>", e.date.year, links.text(e, &e.title))).collect();
        let name = match &f.epithet { Some(ep) if !ep.is_empty() => format!("{} {}", f.name, ep), _ => f.name.clone() };
        let _ = write!(body, "<article class=\"card\" id=\"p{}\"><h3>{}</h3><p class=\"meta\">{} · {} · {}</p>", f.id.0, esc(&name), esc(&race), people, span);
        if !f.titles.is_empty() { let _ = write!(body, "<p><span class=\"label\">Titles</span> {}</p>", esc(&f.titles.join(", "))); }
        if !f.kills.is_empty() { let _ = write!(body, "<p><span class=\"label\">Slew</span> {}</p>", plural(f.kills.len(), "foe of legend", "foes of legend")); }
        if !deeds.is_empty() { let _ = write!(body, "<ul class=\"deeds\">{}</ul>", deeds.join("")); }
        for (i, w) in library.writings.iter().enumerate().filter(|(_, w)| w.author == Some(f.id.0) && w.kind == WritingKind::Poem) {
            body.push_str(&writing_html(i, w));
        }
        body.push_str("</article>");
    }
    body.push_str("</div></section>");

    // --- Beasts of legend -------------------------------------------------------------------
    let _ = write!(toc, "<li><a href=\"#beasts\">Beasts of Legend<span>{}</span></a></li>", beasts.len());
    let _ = write!(body, "<section class=\"part\" id=\"beasts\"><p class=\"eyebrow\">Part the Fifth</p><h2>Beasts of Legend</h2><p class=\"part-intro\">The monsters that raided most often, and those whose lairs or carcasses changed the land.</p><div class=\"cards\">");
    for c in &beasts {
        let species = history.creature_species.get(&c.species_id);
        let kind = species.map(|s| format!("{} {}", format!("{:?}", s.size).to_lowercase(), s.name)).unwrap_or_default();
        let lair = c.lair_location.map(|(x, y)| { let d = gaz.describe(x, y); if d.is_empty() { format!("lair at {},{}", x, y) } else { format!("lair in {}", d) } }).unwrap_or_default();
        let (raids, dead) = raids_by.get(&c.id).copied().unwrap_or((0, 0));
        let fate = match slain_by.get(&c.id) {
            Some(e) => links.text(e, &e.description),
            None if c.is_alive() => "It lives still.".into(),
            None => format!("It died in the year {}.", c.death_date.map(|d| d.year).unwrap_or(0)),
        };
        let powers: Vec<String> = c.unique_abilities.iter().map(|a| words(&format!("{:?}", a))).collect();
        let _ = write!(body, "<article class=\"card\" id=\"b{}\"><h3>{}</h3><p class=\"meta\">{}{}</p>", c.id.0, esc(&c.full_name()), esc(&kind), if lair.is_empty() { String::new() } else { format!(" · {}", esc(&lair)) });
        if !powers.is_empty() { let _ = write!(body, "<p><span class=\"label\">Powers</span> {}</p>", esc(&powers.join(", "))); }
        if raids > 0 { let _ = write!(body, "<p><span class=\"label\">Raids</span> {}, {} dead</p>", raids, dead); }
        let _ = write!(body, "<p class=\"fate\">{}</p>{}</article>", fate, written(Subject::Beast(c.id.0)));
    }
    body.push_str("</div></section>");

    // --- The land ---------------------------------------------------------------------------
    if let Some(eco) = &history.ecology {
        let _ = write!(toc, "<li><a href=\"#land\">The Land Remembers<span>{}</span></a></li>", eco.scars.len());
        let forest: f32 = eco.forest.iter().sum();
        let farms = eco.farmland.iter().filter(|&&f| f > 0.35).count();
        let _ = write!(body, "<section class=\"part\" id=\"land\"><p class=\"eyebrow\">Part the Sixth</p><h2>The Land Remembers</h2><p class=\"part-intro\">By the year {}, {:.0}% of the old forest still stood and {} were under the plough. These are the beasts of field and forest, against their numbers when the annals begin.</p>",
            final_year, 100.0 * forest / eco.initial_forest.max(1e-6), plural(farms, "tract", "tracts"));
        body.push_str("<div class=\"fauna\">");
        for (k, sp) in crate::history::ecology::SPECIES.iter().enumerate() {
            let now: f32 = eco.fauna[k].iter().sum();
            let pct = 100.0 * now / eco.initial_totals.get(k).copied().unwrap_or(1.0).max(1e-6);
            let _ = write!(body, "<div class=\"fauna-row\"><span class=\"fauna-name\">{}</span><span class=\"bar\"><span style=\"width:{:.0}%\"></span></span><span class=\"num\">{:.0}%</span></div>", esc(sp.plural), pct.min(100.0), pct);
        }
        body.push_str("</div><h3 class=\"sub\">Scarred places</h3><ul class=\"scars\">");
        let mut scars: Vec<_> = eco.scars.iter().filter_map(|s| by_id.get(&s.event).map(|e| (s, *e))).collect();
        scars.sort_by_key(|(_, e)| e.date.year);
        for (s, e) in scars {
            let _ = write!(body, "<li><span class=\"yr\">{}</span> <b>{}</b> <span class=\"meta\">{:?}</span><br>{}{}</li>", e.date.year, esc(&e.title), s.biome, links.text(e, &e.description), written(Subject::Place(s.x, s.y)));
        }
        body.push_str("</ul></section>");
    }

    // --- Treasures, and the index of everything the bard wrote --------------------------------
    let lore: Vec<(usize, &crate::lore::bard::Writing)> = library.writings.iter().enumerate().filter(|(_, w)| w.kind == WritingKind::ArtifactLore).collect();
    if !lore.is_empty() {
        let _ = write!(toc, "<li><a href=\"#treasures\">Treasures<span>{}</span></a></li>", lore.len());
        let _ = write!(body, "<section class=\"part\" id=\"treasures\"><p class=\"eyebrow\">Part the Seventh</p><h2>Treasures</h2><p class=\"part-intro\">Storied things, with the words cut into them and what became of them.</p><div class=\"cards\">");
        for (i, w) in lore {
            let a = match w.subject { Subject::Artifact(id) => history.artifacts.values().find(|a| a.id.0 == id), _ => None };
            let meta = a.map(|a| format!("{} {} · made {}{}", words(&format!("{:?}", a.quality)), words(&format!("{:?}", a.item_type)), a.creation_date.year, if a.destroyed { " · destroyed" } else if a.lost { " · lost" } else { "" })).unwrap_or_default();
            let name = a.map(|a| a.name.clone()).unwrap_or_else(|| w.title.clone());
            let _ = write!(body, "<article class=\"card\"><h3>{}</h3><p class=\"meta\">{}</p>{}</article>", esc(&name), esc(&meta), writing_html(i, w));
        }
        body.push_str("</div></section>");
    }
    if let Some(tales) = history.tales.as_ref().filter(|t| !t.threads.is_empty()) {
        let _ = write!(toc, "<li><a href=\"#threads\">Threads of Fate<span>{}</span></a></li>", tales.threads.len());
        let _ = write!(body, "<section class=\"part\" id=\"threads\"><p class=\"eyebrow\">Prophecies, feuds, curses and vows</p><h2>Threads of Fate</h2><p class=\"part-intro\">What was foretold or sworn, and how it came to pass.</p><ul class=\"scars\">");
        for t in &tales.threads {
            let origin = by_id.get(&t.origin);
            let payoff = t.resolved.filter(|r| *r != t.origin).and_then(|r| by_id.get(&r));
            let _ = write!(body, "<li><span class=\"yr\">{}</span> <b>{}</b> <span class=\"meta\">{}</span><br>{}",
                t.opened, esc(&t.summary), esc(&t.kind),
                origin.map(|e| format!("Set in motion by <a href=\"#e{}\">{}</a>.", e.id.0, esc(&e.title))).unwrap_or_default());
            match payoff {
                Some(e) => { let _ = write!(body, " Fulfilled in the year {}: <a href=\"#e{}\">{}</a>.", e.date.year, e.id.0, esc(&e.title)); }
                None => body.push_str(" <i>Not yet come to pass.</i>"),
            }
            body.push_str("</li>");
        }
        body.push_str("</ul></section>");
    }
    if !library.writings.is_empty() {
        let _ = write!(toc, "<li><a href=\"#songs\">Songs and Sayings<span>{}</span></a></li>", library.writings.len());
        let _ = write!(body, "<section class=\"part\" id=\"songs\"><p class=\"eyebrow\">An index</p><h2>Songs and Sayings</h2><p class=\"part-intro\">Everything the bards of this world left behind, by the year it was made.</p><ul class=\"song-index\">");
        let mut idx: Vec<(usize, &crate::lore::bard::Writing)> = library.writings.iter().enumerate().collect();
        idx.sort_by_key(|(_, w)| w.year);
        for (i, w) in idx {
            if body.contains(&format!("id=\"w{}\"", i)) {
                let _ = write!(body, "<li><span class=\"yr\">{}</span> <a href=\"#w{}\">{}</a> <span class=\"meta\">{}, {}</span></li>", w.year, i, esc(&w.title), esc(w.kind.label()), esc(&w.author_name));
            } else {
                // Not shown anywhere else in the book: give it in full here.
                let _ = write!(body, "<li><span class=\"yr\">{}</span>{}</li>", w.year, writing_html(i, w));
            }
        }
        body.push_str("</ul></section>");
    }

    let chips: String = KINDS.iter().map(|(k, label)| format!("<button type=\"button\" class=\"chip k-{k}\" data-k=\"{k}\" aria-pressed=\"true\">{label}</button>")).collect();
    let title = format!("Annals of {}", world_name);
    format!(
        "<title>{title}</title>\n<link rel=\"preconnect\" href=\"https://fonts.googleapis.com\"><link rel=\"preconnect\" href=\"https://fonts.gstatic.com\" crossorigin><link rel=\"stylesheet\" href=\"https://fonts.googleapis.com/css2?family=IM+Fell+English:ital@0;1&family=IM+Fell+English+SC&family=Alegreya:ital,wght@0,400;0,600;1,400&family=Alegreya+Sans+SC:wght@500&display=swap\">\n<style>{css}</style>\n<div class=\"book\"><nav class=\"toc\" aria-label=\"Contents\"><details open id=\"toc-box\"><summary>Contents</summary><ol>{toc}</ol></details><div class=\"tools\"><label for=\"q\" class=\"label\">Search the annals</label><input id=\"q\" type=\"search\" placeholder=\"a name, a place, a year\" autocomplete=\"off\"><p class=\"label\">Show</p><div class=\"chips\">{chips}</div><p id=\"count\" class=\"meta\" aria-live=\"polite\"></p></div></nav><main>{body}<footer><p>Written from the chronicle of seed {seed}: {events} events in {years} years.</p></footer></main></div>\n<script>{js}</script>\n",
        title = esc(&title), css = CSS, toc = toc, chips = chips, body = body, seed = world.seed(), events = events.len(), years = final_year, js = JS,
    )
}

/// "ElementalControl" -> "elemental control".
fn words(camel: &str) -> String {
    let mut s = String::new();
    for (i, c) in camel.chars().enumerate() {
        if c.is_uppercase() && i > 0 { s.push(' '); }
        s.extend(c.to_lowercase());
    }
    s
}

fn death_words(cause: &str) -> &'static str {
    match cause {
        "Natural" => "died of age",
        "Battle" => "fell in battle",
        "Assassination" => "assassinated",
        "Execution" => "executed",
        "Duel" => "killed in a duel",
        "Monster" => "slain by a monster",
        "Disease" => "taken by sickness",
        "Magic" => "killed by sorcery",
        "Accident" => "died by mishap",
        "Suicide" => "died by their own hand",
        _ => "died",
    }
}

fn roman(n: usize) -> String {
    let table = [(10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I")];
    let (mut n, mut s) = (n, String::new());
    for (v, r) in table {
        while n >= v { s.push_str(r); n -= v; }
    }
    s
}

/// Write the journal as a standalone HTML file.
pub fn write_journal(world: &WorldData, history: &WorldHistory, gaz: &Gazetteer, path: &std::path::Path) -> std::io::Result<()> {
    let page = render_journal(world, history, gaz);
    let doc = format!("<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1, viewport-fit=cover\">\n{}</html>\n", page);
    std::fs::write(path, doc)
}

const CSS: &str = r#"
/* Layout: an annal. Contents and filters in a left rail; each year a red rubric in the margin
   with its entries beside it; the record (peoples, wars, lives, beasts, land) after the ages. */
:root {
  --page: #efe5cf;
  --page-deep: #e4d6b8;
  --ink: #382a20;
  --ink-soft: #6b5846;
  --rule: #c8b48f;
  --rubric: #9c3324;
  --sea: #2c4a5c;
  --wash: #f6efdf;
  --display: "IM Fell English", "Iowan Old Style", "Palatino Linotype", Palatino, Georgia, serif;
  --display-sc: "IM Fell English SC", "IM Fell English", Georgia, serif;
  --body: "Alegreya", "Iowan Old Style", Georgia, serif;
  --label: "Alegreya Sans SC", "Gill Sans", "Trebuchet MS", sans-serif;
  color-scheme: light;
}
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) {
    --page: #1d1813; --page-deep: #262019; --ink: #e8dcc2; --ink-soft: #b3a183; --rule: #4a3f31;
    --rubric: #e0826a; --sea: #9cc0d2; --wash: #2a231b; color-scheme: dark;
  }
}
:root[data-theme="dark"] {
  --page: #1d1813; --page-deep: #262019; --ink: #e8dcc2; --ink-soft: #b3a183; --rule: #4a3f31;
  --rubric: #e0826a; --sea: #9cc0d2; --wash: #2a231b; color-scheme: dark;
}
* { box-sizing: border-box; }
html { scroll-behavior: smooth; }
@media (prefers-reduced-motion: reduce) { html { scroll-behavior: auto; } }
body { margin: 0; background: var(--page); color: var(--ink); font-family: var(--body); font-size: 1.0625rem; line-height: 1.6; }
a { color: var(--sea); text-decoration-color: color-mix(in srgb, var(--sea) 40%, transparent); text-underline-offset: 2px; }
a:hover { text-decoration-color: currentColor; }
:focus-visible { outline: 2px solid var(--rubric); outline-offset: 2px; }
.book { display: grid; grid-template-columns: 17rem minmax(0, 1fr); gap: 3rem; max-width: 76rem; margin: 0 auto; padding-inline: 1.5rem; }
.toc { position: sticky; top: env(safe-area-inset-top, 0px); align-self: start; max-height: 100vh; overflow-y: auto; padding-block: 2rem; font-size: 0.95rem; }
.toc summary { font-family: var(--display-sc); font-size: 1.1rem; cursor: pointer; letter-spacing: 0.04em; }
.toc ol { list-style: none; padding: 0; margin: 0.6rem 0 0; display: grid; gap: 0.1rem; }
.toc li a { display: flex; justify-content: space-between; gap: 0.75rem; padding: 0.2rem 0; text-decoration: none; color: var(--ink); }
.toc li a span { color: var(--ink-soft); font-variant-numeric: tabular-nums; font-size: 0.85rem; white-space: nowrap; }
.toc li a:hover { color: var(--rubric); }
.toc-part { font-family: var(--label); color: var(--rubric); letter-spacing: 0.08em; margin-top: 0.9rem; font-size: 0.85rem; }
.tools { margin-top: 1.6rem; display: grid; gap: 0.5rem; }
.tools input { width: 100%; font: inherit; padding: 0.45rem 0.6rem; border: 1px solid var(--rule); border-radius: 3px; background: var(--wash); color: var(--ink); }
.chips { display: flex; flex-wrap: wrap; gap: 0.35rem; }
.chip { font-family: var(--label); font-size: 0.8rem; letter-spacing: 0.05em; border: 1px solid var(--rule); background: var(--wash); color: var(--ink); padding: 0.15rem 0.55rem; border-radius: 2px; cursor: pointer; }
.chip[aria-pressed="false"] { opacity: 0.45; text-decoration: line-through; }
.label { font-family: var(--label); font-size: 0.8rem; letter-spacing: 0.07em; color: var(--ink-soft); margin: 0; }
p .label { margin-right: 0.4rem; }
main { min-width: 0; padding-block: 2.5rem 4rem; max-width: 46rem; }
.eyebrow { font-family: var(--label); letter-spacing: 0.1em; color: var(--rubric); font-size: 0.85rem; margin: 0 0 0.3rem; }
h1, h2, h3 { font-family: var(--display); font-weight: 400; text-wrap: balance; line-height: 1.15; }
h1 { font-size: clamp(2.4rem, 6vw, 3.6rem); margin: 0 0 1rem; }
h2 { font-size: clamp(1.8rem, 4vw, 2.4rem); margin: 0 0 0.6rem; }
h3 { font-size: 1.35rem; margin: 0 0 0.25rem; }
.title { padding-bottom: 2rem; border-bottom: 1px solid var(--rule); }
.lede { font-size: 1.2rem; line-height: 1.55; margin: 0; max-width: 40rem; }
.lede::first-letter { font-family: var(--display); float: left; font-size: 3.6rem; line-height: 0.85; padding: 0.35rem 0.5rem 0 0; color: var(--rubric); }
.age { padding-top: 3rem; }
.age-head { margin-bottom: 1.5rem; }
.age-headline { font-family: var(--display); font-style: italic; font-size: 1.2rem; color: var(--ink-soft); margin: 0 0 0.6rem; }
.age-sum { margin: 0; padding: 0.8rem 1rem; background: var(--wash); border-left: 2px solid var(--rubric); }
.year { display: grid; grid-template-columns: 4.5rem minmax(0, 1fr); gap: 1rem; padding: 0.55rem 0; border-top: 1px dotted var(--rule); }
.rubric { font-family: var(--display); color: var(--rubric); font-size: 1.35rem; line-height: 1.2; text-align: right; font-variant-numeric: oldstyle-nums; }
.anno { display: block; font-family: var(--label); font-size: 0.7rem; letter-spacing: 0.1em; color: var(--ink-soft); }
.year ul { list-style: none; margin: 0; padding: 0; display: grid; gap: 0.35rem; }
.entry { position: relative; padding-left: 1rem; }
.entry::before { content: ""; position: absolute; left: 0; top: 0.65em; width: 0.4rem; height: 0.4rem; border-radius: 50%; background: var(--rule); }
.entry.k-war::before, .entry.k-fall::before { background: var(--rubric); }
.entry.k-land::before, .entry.k-found::before { background: var(--sea); }
.entry.k-fall { font-weight: 600; }
.entry.k-story { background: var(--wash); border-left: 2px solid var(--rubric); padding: 0.5rem 0.7rem 0.5rem 1rem; }
.entry.k-story::before { display: none; }
.tale-title { display: block; font-family: var(--display); font-size: 1.15rem; color: var(--rubric); }
.tally { color: var(--ink-soft); font-size: 0.95rem; }
.part { padding-top: 3.5rem; }
.part-intro { color: var(--ink-soft); margin-top: 0; }
.cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 19rem), 1fr)); gap: 1rem 1.5rem; }
.card { padding: 0.9rem 0 1rem; border-top: 1px solid var(--rule); min-width: 0; }
.card p { margin: 0.25rem 0; }
.meta { color: var(--ink-soft); font-size: 0.9rem; }
.desc { font-style: italic; }
.fate { margin-top: 0.4rem !important; }
.fate.ended { color: var(--rubric); }
.yr { font-family: var(--label); font-size: 0.8rem; color: var(--ink-soft); font-variant-numeric: tabular-nums; }
.deeds { list-style: none; padding: 0; margin: 0.4rem 0 0; font-size: 0.95rem; display: grid; gap: 0.15rem; }
.table-wrap { overflow-x: auto; }
table { border-collapse: collapse; width: 100%; font-size: 0.95rem; }
th { font-family: var(--label); font-weight: 500; letter-spacing: 0.06em; color: var(--ink-soft); text-align: left; border-bottom: 1px solid var(--ink); padding: 0.4rem 0.5rem; }
td { border-bottom: 1px dotted var(--rule); padding: 0.5rem; vertical-align: top; }
.num { text-align: right; font-variant-numeric: tabular-nums; }
.yrs { white-space: nowrap; font-variant-numeric: tabular-nums; }
.vs { color: var(--ink-soft); font-style: italic; }
.fauna { display: grid; gap: 0.3rem; margin: 1rem 0 2rem; max-width: 32rem; }
.fauna-row { display: grid; grid-template-columns: 7rem minmax(0, 1fr) 3.5rem; gap: 0.75rem; align-items: center; }
.fauna-name { font-family: var(--display); }
.bar { height: 0.55rem; background: var(--page-deep); border-radius: 1px; overflow: hidden; }
.bar span { display: block; height: 100%; background: var(--sea); }
.sub { margin-top: 1rem; }
.scars { list-style: none; padding: 0; display: grid; gap: 0.8rem; }
.writing { margin: 0.8rem 0 0.4rem; padding: 0.9rem 1.1rem; background: var(--wash); border: 1px solid var(--rule); border-radius: 2px; }
.writing figcaption { display: grid; gap: 0.1rem; margin-bottom: 0.5rem; }
.w-title { font-family: var(--display); font-size: 1.2rem; }
.w-by { font-family: var(--label); font-size: 0.78rem; letter-spacing: 0.06em; color: var(--rubric); }
.w-body p { margin: 0 0 0.7rem; }
.w-body p:last-child { margin-bottom: 0; }
.verse .w-body { font-style: italic; line-height: 1.5; }
.entry .writing { font-weight: 400; }
.song-index { list-style: none; padding: 0; display: grid; gap: 0.3rem; }
footer { margin-top: 4rem; padding-top: 1rem; border-top: 1px solid var(--rule); color: var(--ink-soft); font-size: 0.9rem; }
.hidden-by-filter { display: none; }
@media (max-width: 860px) {
  .book { grid-template-columns: minmax(0, 1fr); gap: 0; padding-inline: 1rem; }
  .toc { position: static; max-height: none; padding-block: 1rem 0; border-bottom: 1px solid var(--rule); }
  .year { grid-template-columns: 3.2rem minmax(0, 1fr); gap: 0.7rem; }
  .rubric { font-size: 1.15rem; }
}
"#;

const JS: &str = r#"
(function () {
  var q = document.getElementById('q');
  var count = document.getElementById('count');
  var chips = Array.prototype.slice.call(document.querySelectorAll('.chip'));
  var years = Array.prototype.slice.call(document.querySelectorAll('.year'));
  var cards = Array.prototype.slice.call(document.querySelectorAll('.card, tbody tr, .scars li'));
  if (window.matchMedia('(max-width: 860px)').matches) { document.getElementById('toc-box').open = false; }
  function apply() {
    var term = q.value.trim().toLowerCase();
    var off = {};
    chips.forEach(function (c) { if (c.getAttribute('aria-pressed') === 'false') off[c.dataset.k] = true; });
    var shown = 0;
    years.forEach(function (y) {
      var any = false;
      var yearHit = term && y.id === 'y' + term;
      y.querySelectorAll('.entry').forEach(function (e) {
        var ok = !off[e.dataset.k] && (!term || yearHit || e.textContent.toLowerCase().indexOf(term) >= 0);
        e.classList.toggle('hidden-by-filter', !ok);
        if (ok) { any = true; shown++; }
      });
      y.classList.toggle('hidden-by-filter', !any);
    });
    cards.forEach(function (c) {
      c.classList.toggle('hidden-by-filter', !!term && c.textContent.toLowerCase().indexOf(term) < 0);
    });
    count.textContent = (term || Object.keys(off).length) ? shown + ' annal entries match' : '';
  }
  q.addEventListener('input', apply);
  chips.forEach(function (c) {
    c.addEventListener('click', function () {
      c.setAttribute('aria-pressed', c.getAttribute('aria-pressed') === 'false' ? 'true' : 'false');
      apply();
    });
  });
})();
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prose_lists_read_naturally() {
        let v = |s: &[&str]| s.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(prose_list(&v(&["A"]), 3), "A");
        assert_eq!(prose_list(&v(&["A", "B"]), 3), "A and B");
        assert_eq!(prose_list(&v(&["A", "B", "C"]), 3), "A, B and C");
        assert_eq!(prose_list(&v(&["A", "B", "C", "D", "E"]), 3), "A, B, C and 2 more");
        assert_eq!(number_after("A Flood devastated Southdale, killing 54 people.", "killing"), Some(54));
        assert_eq!(tidy("The The Highford Human School"), "The Highford Human School");
        assert_eq!(roman(4), "IV");
    }

    #[test]
    fn links_replace_names_once_and_escape_the_rest() {
        let links = Links { factions: HashMap::default(), figures: HashMap::default(), beasts: HashMap::default() };
        let html = links.link_names("Ann & Annabel met Annabel", vec![("p1".into(), "Annabel".into()), ("p2".into(), "Ann".into())]);
        assert_eq!(html, "<a href=\"#p2\">Ann</a> &amp; <a href=\"#p1\">Annabel</a> met Annabel");
    }
}

/// A fallen people restored in its old seat (`step_revivals`) is recorded as a founding, but its
/// story is the rising, not the people's original founding myth.
fn is_revival(e: &Event) -> bool {
    e.event_type == EventType::FactionFounded && e.title.ends_with(" rises again")
}
