//! Combat logging system
//!
//! Records detailed combat events for narrative generation and analysis.

use serde::{Deserialize, Serialize};

use crate::simulation::body::CombatEffect;
use crate::simulation::types::TileCoord;

/// Reference to a combatant for logging purposes
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CombatantRef {
    pub id: u64,
    pub name: String,
    pub faction: String,
}

/// Type of combat action
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CombatAction {
    Attack {
        weapon: String,
        damage_type: String,
    },
    Defend,
    Dodge,
    Flee,
    Unable,
}

impl CombatAction {
    pub fn display_name(&self) -> &str {
        match self {
            Self::Attack { .. } => "attack",
            Self::Defend => "defend",
            Self::Dodge => "dodge",
            Self::Flee => "flee",
            Self::Unable => "unable",
        }
    }
}

/// Result of a combat action
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CombatResult {
    Miss,
    Hit,
    Wound,
    Kill { cause: String },
    Blocked,
    Dodged,
    Fled,
}

impl CombatResult {
    pub fn display_name(&self) -> &str {
        match self {
            Self::Miss => "miss",
            Self::Hit => "hit",
            Self::Wound => "wound",
            Self::Kill { .. } => "kill",
            Self::Blocked => "blocked",
            Self::Dodged => "dodged",
            Self::Fled => "fled",
        }
    }
}

/// A single combat log entry
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CombatLogEntry {
    pub tick: u64,
    pub attacker: CombatantRef,
    pub defender: CombatantRef,
    pub action: CombatAction,
    pub target_part: Option<String>,
    pub damage: Option<f32>,
    pub wound_type: Option<String>,
    pub wound_severity: Option<String>,
    pub result: CombatResult,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub effects: Vec<CombatEffect>,
    pub narrative: String,
}

impl CombatLogEntry {
    /// Get a short description of this entry
    pub fn short_description(&self) -> String {
        match &self.result {
            CombatResult::Miss => format!("{} misses {}", self.attacker.name, self.defender.name),
            CombatResult::Hit => format!(
                "{} hits {}'s {}",
                self.attacker.name,
                self.defender.name,
                self.target_part.as_deref().unwrap_or("body")
            ),
            CombatResult::Wound => format!(
                "{} wounds {}'s {} ({} {})",
                self.attacker.name,
                self.defender.name,
                self.target_part.as_deref().unwrap_or("body"),
                self.wound_severity.as_deref().unwrap_or(""),
                self.wound_type.as_deref().unwrap_or("")
            ),
            CombatResult::Kill { cause } => {
                format!("{} kills {} ({})", self.attacker.name, self.defender.name, cause)
            }
            CombatResult::Blocked => {
                format!("{}'s attack is blocked by {}", self.attacker.name, self.defender.name)
            }
            CombatResult::Dodged => {
                format!("{} dodges {}'s attack", self.defender.name, self.attacker.name)
            }
            CombatResult::Fled => format!("{} flees", self.attacker.name),
        }
    }
}

/// Outcome of a combat encounter
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EncounterOutcome {
    Victory { winner: String },
    Fled { fleeing_party: String },
    Mutual,
    Ongoing,
}

impl EncounterOutcome {
    pub fn display_name(&self) -> &str {
        match self {
            Self::Victory { .. } => "victory",
            Self::Fled { .. } => "fled",
            Self::Mutual => "mutual destruction",
            Self::Ongoing => "ongoing",
        }
    }
}

/// A complete combat encounter log
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CombatEncounterLog {
    pub encounter_id: u64,
    pub start_tick: u64,
    pub end_tick: Option<u64>,
    pub location: Option<TileCoord>,
    pub participants: Vec<CombatantRef>,
    pub entries: Vec<CombatLogEntry>,
    pub outcome: EncounterOutcome,
    pub summary: Option<String>,
}

impl CombatEncounterLog {
    pub fn new(encounter_id: u64, start_tick: u64, location: Option<TileCoord>) -> Self {
        Self {
            encounter_id,
            start_tick,
            end_tick: None,
            location,
            participants: Vec::new(),
            entries: Vec::new(),
            outcome: EncounterOutcome::Ongoing,
            summary: None,
        }
    }

    /// Add a participant to the encounter
    pub fn add_participant(&mut self, participant: CombatantRef) {
        if !self.participants.iter().any(|p| p.id == participant.id) {
            self.participants.push(participant);
        }
    }

    /// Add a combat log entry
    pub fn add_entry(&mut self, entry: CombatLogEntry) {
        // Ensure participants are tracked
        self.add_participant(entry.attacker.clone());
        self.add_participant(entry.defender.clone());
        self.entries.push(entry);
    }

    /// End the encounter with an outcome
    pub fn end(&mut self, end_tick: u64, outcome: EncounterOutcome) {
        self.end_tick = Some(end_tick);
        self.outcome = outcome;
        self.generate_summary();
    }

    /// Generate a summary of the encounter
    fn generate_summary(&mut self) {
        let total_entries = self.entries.len();
        let kills: Vec<_> = self
            .entries
            .iter()
            .filter(|e| matches!(e.result, CombatResult::Kill { .. }))
            .collect();
        let wounds: Vec<_> = self
            .entries
            .iter()
            .filter(|e| matches!(e.result, CombatResult::Wound))
            .collect();

        let outcome_str = match &self.outcome {
            EncounterOutcome::Victory { winner } => format!("{} victorious", winner),
            EncounterOutcome::Fled { fleeing_party } => format!("{} fled", fleeing_party),
            EncounterOutcome::Mutual => "mutual destruction".to_string(),
            EncounterOutcome::Ongoing => "ongoing".to_string(),
        };

        self.summary = Some(format!(
            "Combat encounter with {} participants: {} attacks, {} kills, {} wounds. Outcome: {}",
            self.participants.len(),
            total_entries,
            kills.len(),
            wounds.len(),
            outcome_str
        ));
    }

    /// Get the full narrative of the encounter
    pub fn full_narrative(&self) -> String {
        let mut narrative = String::new();

        // Header
        if let Some(loc) = &self.location {
            narrative.push_str(&format!(
                "Combat at ({}, {}) - Tick {}\n",
                loc.x, loc.y, self.start_tick
            ));
        } else {
            narrative.push_str(&format!("Combat - Tick {}\n", self.start_tick));
        }

        // Participants
        narrative.push_str("Participants: ");
        let participant_names: Vec<_> = self.participants.iter().map(|p| p.name.as_str()).collect();
        narrative.push_str(&participant_names.join(", "));
        narrative.push_str("\n\n");

        // Entries grouped by tick
        let mut current_tick = 0;
        for entry in &self.entries {
            if entry.tick != current_tick {
                current_tick = entry.tick;
                narrative.push_str(&format!("\n[Tick {}]\n", current_tick));
            }
            narrative.push_str(&format!("  {}\n", entry.narrative));
        }

        // Outcome
        if let Some(summary) = &self.summary {
            narrative.push_str(&format!("\n{}\n", summary));
        }

        narrative
    }
}

/// Global combat log storage
#[derive(Debug, Default, Clone)]
pub struct CombatLogStore {
    encounters: Vec<CombatEncounterLog>,
    next_encounter_id: u64,
    current_encounter: Option<u64>,
}

impl CombatLogStore {
    pub fn new() -> Self {
        Self {
            encounters: Vec::new(),
            next_encounter_id: 1,
            current_encounter: None,
        }
    }

    /// Start a new combat encounter
    pub fn start_encounter(&mut self, tick: u64, location: Option<TileCoord>) -> u64 {
        let id = self.next_encounter_id;
        self.next_encounter_id += 1;

        let encounter = CombatEncounterLog::new(id, tick, location);
        self.encounters.push(encounter);
        self.current_encounter = Some(id);

        id
    }

    /// Add an entry to the current encounter
    pub fn add_entry(&mut self, entry: CombatLogEntry) {
        if let Some(id) = self.current_encounter {
            if let Some(encounter) = self.encounters.iter_mut().find(|e| e.encounter_id == id) {
                encounter.add_entry(entry);
            }
        }
    }

    /// Add an entry to a specific encounter
    pub fn add_entry_to_encounter(&mut self, encounter_id: u64, entry: CombatLogEntry) {
        if let Some(encounter) = self
            .encounters
            .iter_mut()
            .find(|e| e.encounter_id == encounter_id)
        {
            encounter.add_entry(entry);
        }
    }

    /// End the current encounter
    pub fn end_current_encounter(&mut self, tick: u64, outcome: EncounterOutcome) {
        if let Some(id) = self.current_encounter.take() {
            if let Some(encounter) = self.encounters.iter_mut().find(|e| e.encounter_id == id) {
                encounter.end(tick, outcome);
            }
        }
    }

    /// End a specific encounter
    pub fn end_encounter(&mut self, encounter_id: u64, tick: u64, outcome: EncounterOutcome) {
        if let Some(encounter) = self
            .encounters
            .iter_mut()
            .find(|e| e.encounter_id == encounter_id)
        {
            encounter.end(tick, outcome);
        }
        if self.current_encounter == Some(encounter_id) {
            self.current_encounter = None;
        }
    }

    /// Get recent encounters
    pub fn recent_encounters(&self, count: usize) -> Vec<&CombatEncounterLog> {
        self.encounters.iter().rev().take(count).collect()
    }

    /// Get all encounters
    pub fn all_encounters(&self) -> &[CombatEncounterLog] {
        &self.encounters
    }

    /// Get an encounter by ID
    pub fn get_encounter(&self, id: u64) -> Option<&CombatEncounterLog> {
        self.encounters.iter().find(|e| e.encounter_id == id)
    }

    /// Get total number of encounters
    pub fn encounter_count(&self) -> usize {
        self.encounters.len()
    }

    /// Get recent log entries across all encounters
    pub fn recent_entries(&self, count: usize) -> Vec<&CombatLogEntry> {
        self.encounters
            .iter()
            .rev()
            .flat_map(|e| e.entries.iter().rev())
            .take(count)
            .collect()
    }

    /// Clear all logs
    pub fn clear(&mut self) {
        self.encounters.clear();
        self.current_encounter = None;
    }

    /// Get statistics
    pub fn stats(&self) -> CombatLogStats {
        let total_attacks = self.encounters.iter().map(|e| e.entries.len()).sum();
        let total_kills = self
            .encounters
            .iter()
            .flat_map(|e| &e.entries)
            .filter(|e| matches!(e.result, CombatResult::Kill { .. }))
            .count();
        let total_wounds = self
            .encounters
            .iter()
            .flat_map(|e| &e.entries)
            .filter(|e| matches!(e.result, CombatResult::Wound))
            .count();

        CombatLogStats {
            total_encounters: self.encounters.len(),
            total_attacks,
            total_kills,
            total_wounds,
        }
    }
}

/// Statistics from combat logs
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CombatLogStats {
    pub total_encounters: usize,
    pub total_attacks: usize,
    pub total_kills: usize,
    pub total_wounds: usize,
}
