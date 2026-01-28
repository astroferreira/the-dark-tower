//! Query system for browsing world history.
//!
//! Provides search, filter, and detail retrieval over the WorldHistory database.

use crate::history::world_state::WorldHistory;
use crate::history::{FactionId, SettlementId, FigureId, WarId, ArtifactId, LegendaryCreatureId, ReligionId, EventId, EntityId};

/// A summary line for list views.
#[derive(Clone, Debug)]
pub struct ListEntry {
    pub label: String,
    pub detail: String,
    pub id: EntityRef,
}

/// Reference to any browsable entity.
#[derive(Clone, Debug)]
pub enum EntityRef {
    Faction(FactionId),
    Settlement(SettlementId),
    Figure(FigureId),
    War(WarId),
    Artifact(ArtifactId),
    Creature(LegendaryCreatureId),
    Religion(ReligionId),
    Event(EventId),
}

impl EntityRef {
    /// Return the category this entity belongs to.
    pub fn category(&self) -> Category {
        match self {
            EntityRef::Faction(_) => Category::Factions,
            EntityRef::Settlement(_) => Category::Settlements,
            EntityRef::Figure(_) => Category::Figures,
            EntityRef::War(_) => Category::Wars,
            EntityRef::Artifact(_) => Category::Artifacts,
            EntityRef::Creature(_) => Category::Creatures,
            EntityRef::Religion(_) => Category::Religions,
            EntityRef::Event(_) => Category::Events,
        }
    }
}

/// What category we're browsing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Category {
    Factions,
    Figures,
    Settlements,
    Wars,
    Creatures,
    Artifacts,
    Religions,
    Events,
    Timeline,
}

impl Category {
    pub fn all() -> &'static [Category] {
        &[
            Category::Factions,
            Category::Figures,
            Category::Settlements,
            Category::Wars,
            Category::Creatures,
            Category::Artifacts,
            Category::Religions,
            Category::Events,
            Category::Timeline,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            Category::Factions => "Factions",
            Category::Figures => "Figures",
            Category::Settlements => "Settlements",
            Category::Wars => "Wars",
            Category::Creatures => "Creatures",
            Category::Artifacts => "Artifacts",
            Category::Religions => "Religions",
            Category::Events => "Events",
            Category::Timeline => "Timeline",
        }
    }
}

/// Detail block for a single entity.
#[derive(Clone, Debug)]
pub struct EntityDetail {
    pub title: String,
    pub lines: Vec<DetailLine>,
}

/// A single line in an entity detail view.
#[derive(Clone, Debug)]
pub struct DetailLine {
    pub text: String,
    pub highlight: bool,
    /// Optional link to another entity (makes this line navigable).
    pub link: Option<EntityRef>,
}

impl DetailLine {
    pub fn normal(text: impl Into<String>) -> Self {
        Self { text: text.into(), highlight: false, link: None }
    }
    pub fn highlight(text: impl Into<String>) -> Self {
        Self { text: text.into(), highlight: true, link: None }
    }
    pub fn linked(text: impl Into<String>, entity: EntityRef) -> Self {
        Self { text: text.into(), highlight: false, link: Some(entity) }
    }
    pub fn linked_highlight(text: impl Into<String>, entity: EntityRef) -> Self {
        Self { text: text.into(), highlight: true, link: Some(entity) }
    }
}

/// List all entities of a category, optionally filtered by search text.
pub fn list_entities(history: &WorldHistory, category: Category, search: &str) -> Vec<ListEntry> {
    let filter = search.to_lowercase();
    let matches = |s: &str| -> bool {
        filter.is_empty() || s.to_lowercase().contains(&filter)
    };

    match category {
        Category::Factions => {
            let mut entries: Vec<_> = history.factions.values()
                .filter(|f| matches(&f.name))
                .map(|f| {
                    let status = if f.is_active() { "active" } else { "dissolved" };
                    ListEntry {
                        label: f.name.clone(),
                        detail: format!("{}, pop {}", status, f.total_population),
                        id: EntityRef::Faction(f.id),
                    }
                })
                .collect();
            entries.sort_by(|a, b| a.label.cmp(&b.label));
            entries
        }
        Category::Figures => {
            let mut entries: Vec<_> = history.figures.values()
                .filter(|f| matches(&f.name))
                .map(|f| {
                    let status = if f.is_alive() { "alive" } else { "dead" };
                    ListEntry {
                        label: f.name.clone(),
                        detail: format!("{}, born year {}", status, f.birth_date.year),
                        id: EntityRef::Figure(f.id),
                    }
                })
                .collect();
            entries.sort_by(|a, b| a.label.cmp(&b.label));
            entries
        }
        Category::Settlements => {
            let mut entries: Vec<_> = history.settlements.values()
                .filter(|s| matches(&s.name))
                .map(|s| {
                    let status = if s.is_destroyed() { "ruins" } else { "standing" };
                    ListEntry {
                        label: s.name.clone(),
                        detail: format!("{:?}, {}, pop {}", s.settlement_type, status, s.population),
                        id: EntityRef::Settlement(s.id),
                    }
                })
                .collect();
            entries.sort_by(|a, b| a.label.cmp(&b.label));
            entries
        }
        Category::Wars => {
            let mut entries: Vec<_> = history.wars.values()
                .filter(|w| matches(&w.name))
                .map(|w| {
                    let status = if w.ended.is_none() { "ongoing" } else { "ended" };
                    ListEntry {
                        label: w.name.clone(),
                        detail: format!("{}, started year {}", status, w.started.year),
                        id: EntityRef::War(w.id),
                    }
                })
                .collect();
            entries.sort_by(|a, b| a.label.cmp(&b.label));
            entries
        }
        Category::Creatures => {
            let mut entries: Vec<_> = history.legendary_creatures.values()
                .filter(|c| matches(&c.name))
                .map(|c| {
                    let status = if c.is_alive() { "alive" } else { "slain" };
                    ListEntry {
                        label: c.name.clone(),
                        detail: format!("{}, {}", c.species_id, status),
                        id: EntityRef::Creature(c.id),
                    }
                })
                .collect();
            entries.sort_by(|a, b| a.label.cmp(&b.label));
            entries
        }
        Category::Artifacts => {
            let mut entries: Vec<_> = history.artifacts.values()
                .filter(|a| matches(&a.name))
                .map(|a| {
                    ListEntry {
                        label: a.name.clone(),
                        detail: format!("{:?}, created year {}", a.item_type, a.creation_date.year),
                        id: EntityRef::Artifact(a.id),
                    }
                })
                .collect();
            entries.sort_by(|a, b| a.label.cmp(&b.label));
            entries
        }
        Category::Religions => {
            let mut entries: Vec<_> = history.religions.values()
                .filter(|r| matches(&r.name))
                .map(|r| {
                    ListEntry {
                        label: r.name.clone(),
                        detail: format!("{} followers", r.follower_count),
                        id: EntityRef::Religion(r.id),
                    }
                })
                .collect();
            entries.sort_by(|a, b| a.label.cmp(&b.label));
            entries
        }
        Category::Events => {
            let events = &history.chronicle.events;
            let mut entries: Vec<_> = events.iter()
                .filter(|e| {
                    if filter.is_empty() { return true; }
                    let type_name = format!("{:?}", e.event_type).to_lowercase();
                    type_name.contains(&filter) || e.description.to_lowercase().contains(&filter)
                })
                .map(|e| {
                    ListEntry {
                        label: format!("Year {} - {:?}", e.date.year, e.event_type),
                        detail: e.description.clone(),
                        id: EntityRef::Event(e.id),
                    }
                })
                .collect();
            entries.reverse(); // Most recent first
            entries
        }
        Category::Timeline => {
            // Show eras
            let entries: Vec<_> = history.timeline.eras.iter()
                .map(|era| {
                    let end_str = era.end
                        .map(|d| format!("{}", d.year))
                        .unwrap_or_else(|| "present".to_string());
                    ListEntry {
                        label: era.name.clone(),
                        detail: format!("Year {} - {}", era.start.year, end_str),
                        id: EntityRef::Event(EventId(0)), // placeholder
                    }
                })
                .collect();
            entries
        }
    }
}

/// Get detailed view for an entity.
pub fn entity_detail(history: &WorldHistory, entity: &EntityRef) -> EntityDetail {
    match entity {
        EntityRef::Faction(id) => faction_detail(history, *id),
        EntityRef::Settlement(id) => settlement_detail(history, *id),
        EntityRef::Figure(id) => figure_detail(history, *id),
        EntityRef::War(id) => war_detail(history, *id),
        EntityRef::Artifact(id) => artifact_detail(history, *id),
        EntityRef::Creature(id) => creature_detail(history, *id),
        EntityRef::Religion(id) => religion_detail(history, *id),
        EntityRef::Event(id) => event_detail(history, *id),
    }
}

fn faction_detail(history: &WorldHistory, id: FactionId) -> EntityDetail {
    let Some(faction) = history.factions.get(&id) else {
        return EntityDetail { title: "Unknown Faction".to_string(), lines: vec![] };
    };

    let mut lines = vec![];
    lines.push(DetailLine::highlight(format!("Status: {}", if faction.is_active() { "Active" } else { "Dissolved" })));

    // Race
    if let Some(race) = history.races.get(&faction.race_id) {
        lines.push(DetailLine::normal(format!("Race: {:?} ({})", race.base_type, race.name)));
    }

    // Government
    lines.push(DetailLine::normal(format!("Government: {:?}", faction.government)));
    lines.push(DetailLine::normal(format!("Population: {}", faction.total_population)));

    // Leader
    if let Some(leader_id) = faction.current_leader {
        if let Some(leader) = history.figures.get(&leader_id) {
            lines.push(DetailLine::linked(
                format!("Leader: {}", leader.name),
                EntityRef::Figure(leader_id),
            ));
        }
    }

    // Capital
    if let Some(cap_id) = faction.capital {
        if let Some(cap) = history.settlements.get(&cap_id) {
            lines.push(DetailLine::linked(
                format!("Capital: {}", cap.name),
                EntityRef::Settlement(cap_id),
            ));
        }
    }

    lines.push(DetailLine::normal(""));
    lines.push(DetailLine::highlight("Settlements:"));
    for sid in &faction.settlements {
        if let Some(s) = history.settlements.get(sid) {
            lines.push(DetailLine::linked(
                format!("  {} ({:?}, pop {})", s.name, s.settlement_type, s.population),
                EntityRef::Settlement(*sid),
            ));
        }
    }

    // Diplomatic relations - show opinion with other factions
    if !faction.relations.is_empty() {
        lines.push(DetailLine::normal(""));
        lines.push(DetailLine::highlight("Diplomatic Relations:"));
        
        // Sort relations by opinion (highest first)
        let mut relations: Vec<_> = faction.relations.iter().collect();
        relations.sort_by(|a, b| b.1.opinion.cmp(&a.1.opinion));
        
        for (other_id, relation) in relations {
            if let Some(other_faction) = history.factions.get(other_id) {
                let stance_str = match relation.stance {
                    crate::history::civilizations::diplomacy::DiplomaticStance::Allied => "Allied",
                    crate::history::civilizations::diplomacy::DiplomaticStance::Friendly => "Friendly",
                    crate::history::civilizations::diplomacy::DiplomaticStance::Neutral => "Neutral",
                    crate::history::civilizations::diplomacy::DiplomaticStance::Hostile => "Hostile",
                    crate::history::civilizations::diplomacy::DiplomaticStance::War => "At War",
                    crate::history::civilizations::diplomacy::DiplomaticStance::Vassal => "Vassal",
                    crate::history::civilizations::diplomacy::DiplomaticStance::Overlord => "Overlord",
                };
                let opinion_indicator = if relation.opinion >= 50 {
                    "+++"
                } else if relation.opinion >= 20 {
                    "++"
                } else if relation.opinion >= 0 {
                    "+"
                } else if relation.opinion >= -20 {
                    "-"
                } else if relation.opinion >= -50 {
                    "--"
                } else {
                    "---"
                };
                lines.push(DetailLine::linked(
                    format!("  {} ({}) [{}] {}", other_faction.name, stance_str, relation.opinion, opinion_indicator),
                    EntityRef::Faction(*other_id),
                ));
            }
        }
    }

    // Related events (all of them, no cap)
    lines.push(DetailLine::normal(""));
    lines.push(DetailLine::highlight("Key Events:"));
    let events: Vec<_> = history.chronicle.events.iter()
        .filter(|e| e.factions_involved.contains(&id))
        .collect();
    for event in &events {
        lines.push(DetailLine::linked(
            format!("  Year {}: {}", event.date.year, event.description),
            EntityRef::Event(event.id),
        ));
    }

    EntityDetail { title: faction.name.clone(), lines }
}

fn settlement_detail(history: &WorldHistory, id: SettlementId) -> EntityDetail {
    let Some(settlement) = history.settlements.get(&id) else {
        return EntityDetail { title: "Unknown Settlement".to_string(), lines: vec![] };
    };

    let mut lines = vec![];
    lines.push(DetailLine::highlight(format!("Type: {:?}", settlement.settlement_type)));
    lines.push(DetailLine::normal(format!("Population: {}", settlement.population)));
    lines.push(DetailLine::normal(format!("Location: ({}, {})", settlement.location.0, settlement.location.1)));
    lines.push(DetailLine::normal(format!("Founded: Year {}", settlement.founded.year)));

    if let Some(faction) = history.factions.get(&settlement.faction) {
        lines.push(DetailLine::linked(
            format!("Faction: {}", faction.name),
            EntityRef::Faction(settlement.faction),
        ));
    }

    if settlement.is_destroyed() {
        lines.push(DetailLine::highlight("Status: DESTROYED"));
    }

    // Resources
    if !settlement.local_resources.is_empty() {
        lines.push(DetailLine::normal(""));
        lines.push(DetailLine::highlight("Resources:"));
        for r in &settlement.local_resources {
            lines.push(DetailLine::normal(format!("  {:?}", r)));
        }
    }

    EntityDetail { title: settlement.name.clone(), lines }
}

fn figure_detail(history: &WorldHistory, id: FigureId) -> EntityDetail {
    let Some(figure) = history.figures.get(&id) else {
        return EntityDetail { title: "Unknown Figure".to_string(), lines: vec![] };
    };

    let mut lines = vec![];
    let status = if figure.is_alive() { "Alive" } else { "Dead" };
    lines.push(DetailLine::highlight(format!("Status: {}", status)));

    if let Some(race) = history.races.get(&figure.race_id) {
        lines.push(DetailLine::normal(format!("Race: {:?}", race.base_type)));
    }

    lines.push(DetailLine::normal(format!("Born: Year {}", figure.birth_date.year)));
    if let Some(ref death) = figure.death_date {
        lines.push(DetailLine::normal(format!("Died: Year {}", death.year)));
    }
    if let Some(ref cause) = figure.cause_of_death {
        lines.push(DetailLine::normal(format!("Cause: {:?}", cause)));
    }

    if let Some(faction_id) = figure.faction {
        if let Some(faction) = history.factions.get(&faction_id) {
            lines.push(DetailLine::linked(
                format!("Faction: {}", faction.name),
                EntityRef::Faction(faction_id),
            ));
        }
    }

    // Traits
    lines.push(DetailLine::normal(""));
    lines.push(DetailLine::highlight("Personality:"));
    lines.push(DetailLine::normal(format!("  {:?}", figure.personality)));

    // Titles
    if !figure.titles.is_empty() {
        lines.push(DetailLine::normal(""));
        lines.push(DetailLine::highlight("Titles:"));
        for title in &figure.titles {
            lines.push(DetailLine::normal(format!("  {}", title)));
        }
    }

    // Related events
    let figure_entity = EntityId::Figure(id);
    let events: Vec<_> = history.chronicle.events.iter()
        .filter(|e| e.primary_participants.contains(&figure_entity))
        .collect();
    if !events.is_empty() {
        lines.push(DetailLine::normal(""));
        lines.push(DetailLine::highlight("Events:"));
        for event in &events {
            lines.push(DetailLine::linked(
                format!("  Year {}: {}", event.date.year, event.description),
                EntityRef::Event(event.id),
            ));
        }
    }

    EntityDetail { title: figure.name.clone(), lines }
}

fn war_detail(history: &WorldHistory, id: WarId) -> EntityDetail {
    let Some(war) = history.wars.get(&id) else {
        return EntityDetail { title: "Unknown War".to_string(), lines: vec![] };
    };

    let mut lines = vec![];
    let status = if war.ended.is_none() { "Ongoing" } else { "Ended" };
    lines.push(DetailLine::highlight(format!("Status: {}", status)));
    lines.push(DetailLine::normal(format!("Started: Year {}", war.started.year)));
    if let Some(ref end) = war.ended {
        lines.push(DetailLine::normal(format!("Ended: Year {}", end.year)));
    }

    // Belligerents
    lines.push(DetailLine::normal("Aggressors:"));
    for fid in &war.aggressors {
        if let Some(f) = history.factions.get(fid) {
            lines.push(DetailLine::linked(
                format!("  {}", f.name),
                EntityRef::Faction(*fid),
            ));
        }
    }
    lines.push(DetailLine::normal("Defenders:"));
    for fid in &war.defenders {
        if let Some(f) = history.factions.get(fid) {
            lines.push(DetailLine::linked(
                format!("  {}", f.name),
                EntityRef::Faction(*fid),
            ));
        }
    }

    lines.push(DetailLine::normal(format!("Battles: {}", war.battles.len())));
    let total_cas = war.casualties.aggressor_losses + war.casualties.defender_losses + war.casualties.civilian_losses;
    lines.push(DetailLine::normal(format!("Casualties: {}", total_cas)));

    EntityDetail { title: war.name.clone(), lines }
}

fn artifact_detail(history: &WorldHistory, id: ArtifactId) -> EntityDetail {
    let Some(artifact) = history.artifacts.get(&id) else {
        return EntityDetail { title: "Unknown Artifact".to_string(), lines: vec![] };
    };

    let mut lines = vec![];
    lines.push(DetailLine::highlight(format!("Type: {:?}", artifact.item_type)));
    lines.push(DetailLine::normal(format!("Quality: {:?}", artifact.quality)));
    lines.push(DetailLine::normal(format!("Created: Year {}", artifact.creation_date.year)));

    if let Some(creator_id) = artifact.creator {
        if let Some(creator) = history.figures.get(&creator_id) {
            lines.push(DetailLine::linked(
                format!("Creator: {}", creator.name),
                EntityRef::Figure(creator_id),
            ));
        }
    }

    if let Some(ref owner) = artifact.current_owner {
        lines.push(DetailLine::normal(format!("Current Owner: {}", owner)));
    }

    if let Some((lx, ly)) = artifact.current_location {
        lines.push(DetailLine::normal(format!("Location: ({}, {})", lx, ly)));
    }

    if artifact.lost {
        lines.push(DetailLine::highlight("Status: LOST"));
    }
    if artifact.destroyed {
        lines.push(DetailLine::highlight("Status: DESTROYED"));
    }

    if !artifact.inscriptions.is_empty() {
        lines.push(DetailLine::normal(""));
        lines.push(DetailLine::highlight("Inscriptions:"));
        for insc in &artifact.inscriptions {
            lines.push(DetailLine::normal(format!("  {:?}", insc)));
        }
    }

    EntityDetail { title: artifact.name.clone(), lines }
}

fn creature_detail(history: &WorldHistory, id: LegendaryCreatureId) -> EntityDetail {
    let Some(creature) = history.legendary_creatures.get(&id) else {
        return EntityDetail { title: "Unknown Creature".to_string(), lines: vec![] };
    };

    let mut lines = vec![];
    let status = if creature.is_alive() { "Alive" } else { "Slain" };
    lines.push(DetailLine::highlight(format!("Status: {}", status)));
    lines.push(DetailLine::normal(format!("Species ID: {}", creature.species_id)));
    if let Some(ref birth) = creature.birth_date {
        lines.push(DetailLine::normal(format!("Born: Year {}", birth.year)));
    }

    if let Some((x, y)) = creature.lair_location {
        lines.push(DetailLine::normal(format!("Lair: ({}, {})", x, y)));
    }

    lines.push(DetailLine::normal(format!("Kills: {}", creature.kills.len())));
    lines.push(DetailLine::normal(format!("Size: {:.1}x", creature.size_multiplier)));
    lines.push(DetailLine::normal(format!("Worshippers: {}", creature.worshipper_count)));

    if !creature.artifacts_owned.is_empty() {
        lines.push(DetailLine::normal(""));
        lines.push(DetailLine::highlight("Artifacts:"));
        for aid in &creature.artifacts_owned {
            if let Some(artifact) = history.artifacts.get(aid) {
                lines.push(DetailLine::linked(
                    format!("  {}", artifact.name),
                    EntityRef::Artifact(*aid),
                ));
            }
        }
    }

    // Related events
    let creature_entity = EntityId::LegendaryCreature(id);
    let events: Vec<_> = history.chronicle.events.iter()
        .filter(|e| e.primary_participants.contains(&creature_entity))
        .collect();
    if !events.is_empty() {
        lines.push(DetailLine::normal(""));
        lines.push(DetailLine::highlight("Events:"));
        for event in &events {
            lines.push(DetailLine::linked(
                format!("  Year {}: {}", event.date.year, event.description),
                EntityRef::Event(event.id),
            ));
        }
    }

    EntityDetail { title: creature.name.clone(), lines }
}

fn religion_detail(history: &WorldHistory, id: ReligionId) -> EntityDetail {
    let Some(religion) = history.religions.get(&id) else {
        return EntityDetail { title: "Unknown Religion".to_string(), lines: vec![] };
    };

    let mut lines = vec![];
    lines.push(DetailLine::normal(format!("Followers: {}", religion.follower_count)));
    lines.push(DetailLine::normal(format!("Founded: Year {}", religion.origin_date.year)));

    if !religion.deities.is_empty() {
        lines.push(DetailLine::normal(""));
        lines.push(DetailLine::highlight("Deities:"));
        for did in &religion.deities {
            if let Some(deity) = history.deities.get(did) {
                lines.push(DetailLine::normal(format!("  {} ({:?})", deity.name, deity.domains)));
            }
        }
    }

    if let Some(founder_id) = religion.founder {
        if let Some(founder) = history.figures.get(&founder_id) {
            lines.push(DetailLine::linked(
                format!("Founder: {}", founder.name),
                EntityRef::Figure(founder_id),
            ));
        }
    }

    if !religion.follower_factions.is_empty() {
        lines.push(DetailLine::normal(""));
        lines.push(DetailLine::highlight("Follower Factions:"));
        for fid in &religion.follower_factions {
            if let Some(faction) = history.factions.get(fid) {
                lines.push(DetailLine::linked(
                    format!("  {}", faction.name),
                    EntityRef::Faction(*fid),
                ));
            }
        }
    }

    EntityDetail { title: religion.name.clone(), lines }
}

fn event_detail(history: &WorldHistory, id: EventId) -> EntityDetail {
    let Some(event) = history.chronicle.get(id) else {
        return EntityDetail { title: "Unknown Event".to_string(), lines: vec![] };
    };

    let mut lines = vec![];
    lines.push(DetailLine::highlight(format!("Type: {:?}", event.event_type)));
    lines.push(DetailLine::normal(format!("Date: {}", event.date)));
    lines.push(DetailLine::normal(format!("Major: {}", if event.is_major { "Yes" } else { "No" })));
    lines.push(DetailLine::normal(""));
    lines.push(DetailLine::normal(event.description.clone()));

    // Causes
    if !event.causes.is_empty() {
        lines.push(DetailLine::normal(""));
        lines.push(DetailLine::highlight("Caused by:"));
        for cause_id in &event.causes {
            if let Some(cause) = history.chronicle.get(*cause_id) {
                lines.push(DetailLine::linked(
                    format!("  Year {}: {:?}", cause.date.year, cause.event_type),
                    EntityRef::Event(*cause_id),
                ));
            }
        }
    }

    // Triggered events
    if !event.triggered_events.is_empty() {
        lines.push(DetailLine::normal(""));
        lines.push(DetailLine::highlight("Led to:"));
        for effect_id in &event.triggered_events {
            if let Some(effect) = history.chronicle.get(*effect_id) {
                lines.push(DetailLine::linked(
                    format!("  Year {}: {:?}", effect.date.year, effect.event_type),
                    EntityRef::Event(*effect_id),
                ));
            }
        }
    }

    EntityDetail { title: format!("{:?} (Year {})", event.event_type, event.date.year), lines }
}

/// Generate a text summary of the complete world history.
pub fn legends_export(history: &WorldHistory) -> String {
    let mut out = String::new();
    let summary = history.summary();
    out.push_str(&format!("{}\n", summary));

    // Eras
    out.push_str("=== Eras ===\n");
    for era in &history.timeline.eras {
        let end_str = era.end.map(|d| format!("{}", d.year)).unwrap_or_else(|| "present".to_string());
        out.push_str(&format!("  {} (Year {} - {})\n", era.name, era.start.year, end_str));
    }
    out.push('\n');

    // Factions
    out.push_str("=== Factions ===\n");
    for faction in history.factions.values() {
        let status = if faction.is_active() { "Active" } else { "Dissolved" };
        out.push_str(&format!("  {} [{}] - Pop: {}\n", faction.name, status, faction.total_population));
    }
    out.push('\n');

    // Legendary creatures
    out.push_str("=== Legendary Creatures ===\n");
    for creature in history.legendary_creatures.values() {
        let status = if creature.is_alive() { "alive" } else { "slain" };
        out.push_str(&format!("  {} ({}) - {} kills, {}\n",
            creature.name, creature.species_id, creature.kills.len(), status));
    }
    out.push('\n');

    // Major events
    out.push_str("=== Major Events ===\n");
    for event in history.chronicle.major_events() {
        out.push_str(&format!("  Year {}: {}\n", event.date.year, event.description));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::config::HistoryConfig;
    use crate::history::simulation::HistoryEngine;
    use crate::tilemap::Tilemap;
    use crate::biomes::ExtendedBiome;
    use crate::seeds::WorldSeeds;
    use crate::scale::MapScale;
    use crate::plates::PlateId;
    use crate::water_bodies::WaterBodyId;
    use crate::world::WorldData;

    fn make_test_world() -> WorldData {
        let width = 64;
        let height = 32;
        let mut heightmap = Tilemap::new_with(width, height, 0.3);
        let mut biomes = Tilemap::new_with(width, height, ExtendedBiome::TemperateGrassland);
        for x in 0..width {
            *biomes.get_mut(x, 0) = ExtendedBiome::Ocean;
            *heightmap.get_mut(x, 0) = -0.1;
        }
        let seeds = WorldSeeds::from_master(42);
        let scale = MapScale::new(1.0);
        let temperature = Tilemap::new_with(width, height, 15.0);
        let moisture = Tilemap::new_with(width, height, 0.5);
        let stress_map = Tilemap::new_with(width, height, 0.0);
        let plate_map = Tilemap::new_with(width, height, PlateId(0));
        let water_body_map = Tilemap::new_with(width, height, WaterBodyId::NONE);
        let water_depth = Tilemap::new_with(width, height, 0.0);
        WorldData::new(
            seeds, scale, heightmap, temperature, moisture,
            biomes, stress_map, plate_map, Vec::new(),
            None, water_body_map, Vec::new(), water_depth,
            None, None,
        )
    }

    #[test]
    fn test_list_factions() {
        let world = make_test_world();
        let config = HistoryConfig {
            simulation_years: 50,
            initial_civilizations: 3,
            ..HistoryConfig::default()
        };
        let mut engine = HistoryEngine::new(42);
        let history = engine.simulate(&world, config);

        let factions = list_entities(&history, Category::Factions, "");
        assert!(!factions.is_empty());
    }

    #[test]
    fn test_search_filter() {
        let world = make_test_world();
        let config = HistoryConfig {
            simulation_years: 50,
            initial_civilizations: 3,
            ..HistoryConfig::default()
        };
        let mut engine = HistoryEngine::new(42);
        let history = engine.simulate(&world, config);

        // Search with impossible string returns empty
        let results = list_entities(&history, Category::Factions, "zzzzzznotafaction");
        assert!(results.is_empty());

        // All events should list something
        let events = list_entities(&history, Category::Events, "");
        assert!(!events.is_empty());
    }

    #[test]
    fn test_entity_detail() {
        let world = make_test_world();
        let config = HistoryConfig {
            simulation_years: 50,
            initial_civilizations: 3,
            ..HistoryConfig::default()
        };
        let mut engine = HistoryEngine::new(42);
        let history = engine.simulate(&world, config);

        // Get first faction detail
        if let Some(faction) = history.factions.values().next() {
            let detail = entity_detail(&history, &EntityRef::Faction(faction.id));
            assert!(!detail.title.is_empty());
            assert!(!detail.lines.is_empty());
        }
    }

    #[test]
    fn test_legends_export() {
        let world = make_test_world();
        let config = HistoryConfig {
            simulation_years: 50,
            initial_civilizations: 3,
            ..HistoryConfig::default()
        };
        let mut engine = HistoryEngine::new(42);
        let history = engine.simulate(&world, config);

        let export = legends_export(&history);
        assert!(export.contains("Factions"));
        assert!(export.contains("Legendary Creatures"));
    }
}
