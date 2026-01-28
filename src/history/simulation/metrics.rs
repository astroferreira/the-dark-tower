//! Evaluation metrics for simulation quality.
//!
//! Computes quantitative measures across four dimensions:
//! statistical realism, narrative richness, behavioral coherence,
//! and religious impact. Produces a composite quality score (0–100).

use std::collections::HashMap;
use crate::history::*;
use crate::history::events::types::{EventType, Event};
use crate::history::world_state::WorldHistory;
use crate::history::religion::worship::Doctrine;

/// Complete simulation metrics across all four dimensions.
#[derive(Clone, Debug)]
pub struct SimulationMetrics {
    // Statistical Realism
    pub population_gini: f32,
    pub faction_survival_rate: f32,
    pub wars_per_century: f32,
    pub avg_war_duration_years: f32,
    pub monopoly_detected: bool,

    // Narrative Richness
    pub event_type_entropy: f32,
    pub causality_ratio: f32,
    pub personality_triggered_events: u32,
    pub unique_event_types_used: u32,

    // Behavioral Coherence
    pub war_inclination_correlation: f32,
    pub diplomacy_inclination_correlation: f32,
    pub builder_inclination_correlation: f32,

    // Religious Impact
    pub holy_war_effect_size: f32,
    pub pacifism_effect_size: f32,
    pub schism_count: u32,
    pub conversion_count: u32,
    pub multi_faction_religion_count: u32,

    // Composite
    pub statistical_realism_score: f32,
    pub narrative_richness_score: f32,
    pub behavioral_coherence_score: f32,
    pub religious_impact_score: f32,
    pub composite_score: f32,
}

impl SimulationMetrics {
    /// Compute all metrics from a completed WorldHistory.
    pub fn compute(history: &WorldHistory) -> Self {
        let years = history.current_date.year.max(1) as f32;

        // === Statistical Realism ===
        let population_gini = compute_gini(history);
        let faction_survival_rate = compute_survival_rate(history);
        // Normalize wars per century by faction count (per 100 factions)
        let avg_faction_count = (history.factions.len() as f32).max(1.0);
        let raw_wars_per_century = (history.wars.len() as f32 / years) * 100.0;
        let wars_per_century = raw_wars_per_century / (avg_faction_count / 100.0).max(1.0);
        let avg_war_duration_years = compute_avg_war_duration(history);
        let monopoly_detected = detect_monopoly(history);

        // === Narrative Richness ===
        let event_type_entropy = compute_event_entropy(history);
        let causality_ratio = compute_causality_ratio(history);
        let personality_triggered_events = count_personality_events(history);
        let unique_event_types_used = count_unique_event_types(history);

        // === Behavioral Coherence ===
        let (war_corr, dip_corr, builder_corr) = compute_behavioral_correlations(history);

        // === Religious Impact ===
        let holy_war_effect_size = compute_doctrine_war_effect(history, Doctrine::HolyWar);
        let pacifism_effect_size = compute_doctrine_war_effect(history, Doctrine::Pacifism);
        let schism_count = count_schisms(history);
        let conversion_count = count_conversions(history);
        let multi_faction_religion_count = count_multi_faction_religions(history);

        // === Composite Scores (0–100 each) ===
        let statistical_realism_score = score_statistical_realism(
            population_gini, faction_survival_rate, wars_per_century, monopoly_detected,
        );
        let narrative_richness_score = score_narrative_richness(
            event_type_entropy, causality_ratio, personality_triggered_events, unique_event_types_used,
        );
        let behavioral_coherence_score = score_behavioral_coherence(
            war_corr, dip_corr, builder_corr,
        );
        let religious_impact_score = score_religious_impact(
            holy_war_effect_size, pacifism_effect_size, schism_count, conversion_count,
            multi_faction_religion_count,
        );

        // Weighted composite: 30 + 30 + 25 + 15
        let composite_score = statistical_realism_score * 0.30
            + narrative_richness_score * 0.30
            + behavioral_coherence_score * 0.25
            + religious_impact_score * 0.15;

        Self {
            population_gini,
            faction_survival_rate,
            wars_per_century,
            avg_war_duration_years,
            monopoly_detected,
            event_type_entropy,
            causality_ratio,
            personality_triggered_events,
            unique_event_types_used,
            war_inclination_correlation: war_corr,
            diplomacy_inclination_correlation: dip_corr,
            builder_inclination_correlation: builder_corr,
            holy_war_effect_size,
            pacifism_effect_size,
            schism_count,
            conversion_count,
            multi_faction_religion_count,
            statistical_realism_score,
            narrative_richness_score,
            behavioral_coherence_score,
            religious_impact_score,
            composite_score,
        }
    }

    /// Human-readable report.
    pub fn report(&self) -> String {
        let mut s = String::new();
        s.push_str("=== Simulation Quality Report ===\n\n");

        s.push_str("--- Statistical Realism ---\n");
        s.push_str(&format!("  Population Gini:       {:.3}\n", self.population_gini));
        s.push_str(&format!("  Faction survival rate:  {:.1}%\n", self.faction_survival_rate * 100.0));
        s.push_str(&format!("  Wars per century:       {:.1}\n", self.wars_per_century));
        s.push_str(&format!("  Avg war duration:       {:.1} years\n", self.avg_war_duration_years));
        s.push_str(&format!("  Monopoly detected:      {}\n", self.monopoly_detected));
        s.push_str(&format!("  Score: {:.1}/100\n\n", self.statistical_realism_score));

        s.push_str("--- Narrative Richness ---\n");
        s.push_str(&format!("  Event type entropy:     {:.2} bits\n", self.event_type_entropy));
        s.push_str(&format!("  Causality ratio:        {:.1}%\n", self.causality_ratio * 100.0));
        s.push_str(&format!("  Personality-triggered:   {}\n", self.personality_triggered_events));
        s.push_str(&format!("  Unique event types:     {}\n", self.unique_event_types_used));
        s.push_str(&format!("  Score: {:.1}/100\n\n", self.narrative_richness_score));

        s.push_str("--- Behavioral Coherence ---\n");
        s.push_str(&format!("  War inclination corr:   {:.3}\n", self.war_inclination_correlation));
        s.push_str(&format!("  Diplomacy incl. corr:   {:.3}\n", self.diplomacy_inclination_correlation));
        s.push_str(&format!("  Builder incl. corr:     {:.3}\n", self.builder_inclination_correlation));
        s.push_str(&format!("  Score: {:.1}/100\n\n", self.behavioral_coherence_score));

        s.push_str("--- Religious Impact ---\n");
        s.push_str(&format!("  HolyWar effect (d):     {:.3}\n", self.holy_war_effect_size));
        s.push_str(&format!("  Pacifism effect (d):    {:.3}\n", self.pacifism_effect_size));
        s.push_str(&format!("  Schisms:                {}\n", self.schism_count));
        s.push_str(&format!("  Conversions:            {}\n", self.conversion_count));
        s.push_str(&format!("  Multi-faction religions: {}\n", self.multi_faction_religion_count));
        s.push_str(&format!("  Score: {:.1}/100\n\n", self.religious_impact_score));

        s.push_str(&format!("=== Composite Quality Score: {:.1}/100 ===\n", self.composite_score));
        s.push_str(&format!("  (Statistical {:.0} * 0.30 + Narrative {:.0} * 0.30 + Behavioral {:.0} * 0.25 + Religious {:.0} * 0.15)\n",
            self.statistical_realism_score, self.narrative_richness_score,
            self.behavioral_coherence_score, self.religious_impact_score));

        s
    }
}

// === Statistical Realism helpers ===

/// Gini coefficient of faction populations (0 = equal, 1 = one faction has all).
fn compute_gini(history: &WorldHistory) -> f32 {
    let mut pops: Vec<f64> = history.factions.values()
        .filter(|f| f.is_active())
        .map(|f| f.total_population as f64)
        .collect();
    if pops.len() < 2 { return 0.0; }
    pops.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = pops.len() as f64;
    let sum: f64 = pops.iter().sum();
    if sum == 0.0 { return 0.0; }
    let mut numerator = 0.0;
    for (i, &p) in pops.iter().enumerate() {
        numerator += (2.0 * (i as f64 + 1.0) - n - 1.0) * p;
    }
    (numerator / (n * sum)) as f32
}

fn compute_survival_rate(history: &WorldHistory) -> f32 {
    let total = history.factions.len() as f32;
    if total == 0.0 { return 1.0; }
    let active = history.active_faction_count() as f32;
    active / total
}

fn compute_avg_war_duration(history: &WorldHistory) -> f32 {
    let durations: Vec<f32> = history.wars.values()
        .filter_map(|w| {
            w.ended.map(|end| {
                let start_year = w.started.year as f32 + w.started.season_fraction();
                let end_year = end.year as f32 + end.season_fraction();
                (end_year - start_year).max(0.25)
            })
        })
        .collect();
    if durations.is_empty() { return 0.0; }
    durations.iter().sum::<f32>() / durations.len() as f32
}

fn detect_monopoly(history: &WorldHistory) -> bool {
    let active: Vec<_> = history.factions.values().filter(|f| f.is_active()).collect();
    if active.len() < 2 { return active.len() == 1; }
    let total_pop: u32 = active.iter().map(|f| f.total_population).sum();
    if total_pop == 0 { return false; }
    active.iter().any(|f| (f.total_population as f64 / total_pop as f64) > 0.8)
}

// === Narrative Richness helpers ===

/// Shannon entropy of event type distribution (higher = more diverse).
fn compute_event_entropy(history: &WorldHistory) -> f32 {
    let mut counts: HashMap<&EventType, u32> = HashMap::new();
    for event in &history.chronicle.events {
        *counts.entry(&event.event_type).or_insert(0) += 1;
    }
    let total = history.chronicle.len() as f32;
    if total == 0.0 { return 0.0; }
    let mut entropy = 0.0f32;
    for &count in counts.values() {
        let p = count as f32 / total;
        if p > 0.0 {
            entropy -= p * p.log2();
        }
    }
    entropy
}

/// Fraction of events that have a linked cause (triggered_by is Some).
fn compute_causality_ratio(history: &WorldHistory) -> f32 {
    let total = history.chronicle.len();
    if total == 0 { return 0.0; }
    let caused = history.chronicle.events.iter()
        .filter(|e| e.triggered_by.is_some() || !e.causes.is_empty())
        .count();
    caused as f32 / total as f32
}

/// Count events that are personality-driven (rebellions, personality-biased wars).
fn count_personality_events(history: &WorldHistory) -> u32 {
    // Count Rebellion events (always personality-driven) and HolyWarDeclared
    history.chronicle.events.iter()
        .filter(|e| matches!(e.event_type,
            EventType::Rebellion | EventType::HolyWarDeclared))
        .count() as u32
}

fn count_unique_event_types(history: &WorldHistory) -> u32 {
    let mut types = std::collections::HashSet::new();
    for event in &history.chronicle.events {
        types.insert(std::mem::discriminant(&event.event_type));
    }
    types.len() as u32
}

// === Behavioral Coherence helpers ===

/// Compute behavioral coherence metrics.
///
/// War coherence: Cohen's d comparing war_inclination of leaders who declared wars
/// vs those who did not. Uses per-leader tracking via primary_participants.
///
/// Diplomacy/builder: Pearson correlation between current leader personality and
/// faction behavior (treaties signed, monuments built).
fn compute_behavioral_correlations(history: &WorldHistory) -> (f32, f32, f32) {
    if history.current_date.year < 50 {
        return (0.0, 0.0, 0.0);
    }

    // === War coherence: Cohen's d on per-leader war declarations ===
    // Identify leaders who declared wars via primary_participants
    let mut war_declaring_leaders: std::collections::HashSet<FigureId> = std::collections::HashSet::new();
    for event in &history.chronicle.events {
        if matches!(event.event_type, EventType::WarDeclared | EventType::HolyWarDeclared) {
            for participant in &event.primary_participants {
                if let EntityId::Figure(fid) = participant {
                    war_declaring_leaders.insert(*fid);
                }
            }
        }
    }

    // Collect war_inclination for current faction leaders, split by whether they declared wars
    let mut declared_war_scores: Vec<f32> = Vec::new();
    let mut no_war_scores: Vec<f32> = Vec::new();

    // Use all current faction leaders (active or not)
    for faction in history.factions.values() {
        if let Some(lid) = faction.current_leader {
            if let Some(figure) = history.figures.get(&lid) {
                let incl = figure.personality.war_inclination();
                if war_declaring_leaders.contains(&lid) {
                    declared_war_scores.push(incl);
                } else {
                    no_war_scores.push(incl);
                }
            }
        }
    }

    // Also include non-current leaders who declared wars (they're important signal)
    for &lid in &war_declaring_leaders {
        // Skip if already counted as current leader
        let already_counted = history.factions.values()
            .any(|f| f.current_leader == Some(lid));
        if already_counted { continue; }

        if let Some(figure) = history.figures.get(&lid) {
            declared_war_scores.push(figure.personality.war_inclination());
        }
    }

    let war_corr = if declared_war_scores.len() >= 3 && no_war_scores.len() >= 3 {
        cohen_d(&declared_war_scores, &no_war_scores)
    } else {
        0.0
    };

    // === Diplomacy and builder correlations: per-faction (current leader) ===
    let active_factions: Vec<_> = history.factions.values()
        .filter(|f| f.is_active())
        .collect();
    if active_factions.len() < 5 {
        return (war_corr, 0.0, 0.0);
    }

    let mut treaties_signed: HashMap<FactionId, u32> = HashMap::new();
    let mut monuments_built: HashMap<FactionId, u32> = HashMap::new();

    for event in &history.chronicle.events {
        match event.event_type {
            EventType::TreatySigned | EventType::AllianceFormed => {
                for &fid in &event.factions_involved {
                    *treaties_signed.entry(fid).or_insert(0) += 1;
                }
            }
            EventType::MonumentBuilt => {
                if let Some(&fid) = event.factions_involved.first() {
                    *monuments_built.entry(fid).or_insert(0) += 1;
                }
            }
            _ => {}
        }
    }

    let mut dip_xs = Vec::new();
    let mut dip_ys = Vec::new();
    let mut build_xs = Vec::new();
    let mut build_ys = Vec::new();

    for faction in &active_factions {
        let personality = faction.current_leader
            .and_then(|lid| history.figures.get(&lid))
            .map(|f| &f.personality);

        if let Some(p) = personality {
            dip_xs.push(p.diplomacy_inclination());
            dip_ys.push(*treaties_signed.get(&faction.id).unwrap_or(&0) as f32);
            build_xs.push(p.builder_inclination());
            build_ys.push(*monuments_built.get(&faction.id).unwrap_or(&0) as f32);
        }
    }

    let dip_corr = pearson_correlation(&dip_xs, &dip_ys);
    let builder_corr = pearson_correlation(&build_xs, &build_ys);

    (war_corr, dip_corr, builder_corr)
}

/// Pearson correlation coefficient between two series.
fn pearson_correlation(xs: &[f32], ys: &[f32]) -> f32 {
    let n = xs.len();
    if n < 3 { return 0.0; }
    let n_f = n as f32;
    let mean_x = xs.iter().sum::<f32>() / n_f;
    let mean_y = ys.iter().sum::<f32>() / n_f;
    let mut cov = 0.0f32;
    let mut var_x = 0.0f32;
    let mut var_y = 0.0f32;
    for i in 0..n {
        let dx = xs[i] - mean_x;
        let dy = ys[i] - mean_y;
        cov += dx * dy;
        var_x += dx * dx;
        var_y += dy * dy;
    }
    let denom = (var_x * var_y).sqrt();
    if denom < 1e-10 { return 0.0; }
    cov / denom
}

// === Religious Impact helpers ===

/// Compute Cohen's d effect size: compare wars per century of existence between
/// factions with vs without a specific doctrine in their state religion.
/// Includes ALL factions (active and destroyed) to avoid survivorship bias.
/// Normalizes by faction lifetime to prevent early-death factions from showing
/// fewer wars despite being more warlike per year.
fn compute_doctrine_war_effect(history: &WorldHistory, doctrine: Doctrine) -> f32 {
    let all_factions: Vec<_> = history.factions.values().collect();
    if all_factions.len() < 4 { return 0.0; }

    let current_year = history.current_date.year;

    // Count wars declared per faction (first faction listed = the declarer)
    // Include both WarDeclared and HolyWarDeclared events
    let mut wars_per_faction: HashMap<FactionId, u32> = HashMap::new();
    for event in &history.chronicle.events {
        if matches!(event.event_type, EventType::WarDeclared | EventType::HolyWarDeclared) {
            if let Some(&fid) = event.factions_involved.first() {
                *wars_per_faction.entry(fid).or_insert(0) += 1;
            }
        }
    }

    // Split ALL factions into two groups by doctrine
    // Normalize war count by faction lifetime (wars per century)
    let mut with_doctrine: Vec<f32> = Vec::new();
    let mut without_doctrine: Vec<f32> = Vec::new();

    for faction in &all_factions {
        let has_it = faction.state_religion
            .and_then(|rid| history.religions.get(&rid))
            .map_or(false, |r| r.has_doctrine(doctrine));
        let raw_wars = *wars_per_faction.get(&faction.id).unwrap_or(&0) as f32;

        // Faction lifetime in centuries (min 0.25 century = 25 years to avoid division spikes)
        let end_year = faction.dissolved.map(|d| d.year).unwrap_or(current_year);
        let lifetime_centuries = (end_year.saturating_sub(faction.founded.year) as f32 / 100.0).max(0.25);
        let wars_per_century = raw_wars / lifetime_centuries;

        if has_it {
            with_doctrine.push(wars_per_century);
        } else {
            without_doctrine.push(wars_per_century);
        }
    }

    if with_doctrine.len() < 2 || without_doctrine.len() < 2 {
        return 0.0;
    }

    cohen_d(&with_doctrine, &without_doctrine)
}

/// Cohen's d: (mean1 - mean2) / pooled_std.
fn cohen_d(a: &[f32], b: &[f32]) -> f32 {
    let n1 = a.len() as f32;
    let n2 = b.len() as f32;
    let mean1 = a.iter().sum::<f32>() / n1;
    let mean2 = b.iter().sum::<f32>() / n2;
    let var1 = a.iter().map(|x| (x - mean1).powi(2)).sum::<f32>() / (n1 - 1.0).max(1.0);
    let var2 = b.iter().map(|x| (x - mean2).powi(2)).sum::<f32>() / (n2 - 1.0).max(1.0);
    let pooled_std = (((n1 - 1.0) * var1 + (n2 - 1.0) * var2) / (n1 + n2 - 2.0)).sqrt();
    if pooled_std < 1e-10 { return 0.0; }
    (mean1 - mean2) / pooled_std
}

fn count_schisms(history: &WorldHistory) -> u32 {
    history.religions.values()
        .map(|r| r.heresies.len() as u32)
        .sum()
}

fn count_conversions(history: &WorldHistory) -> u32 {
    // Count Miracle events that mention "converts" (our conversion events)
    history.chronicle.events.iter()
        .filter(|e| e.event_type == EventType::Miracle && e.title.contains("converts"))
        .count() as u32
}

fn count_multi_faction_religions(history: &WorldHistory) -> u32 {
    history.religions.values()
        .filter(|r| r.follower_factions.len() >= 2)
        .count() as u32
}

// === Scoring functions (0–100) ===

fn score_statistical_realism(gini: f32, survival: f32, wars_century: f32, monopoly: bool) -> f32 {
    let mut score = 0.0;

    // Gini: ideal is 0.3–0.6 (moderate inequality)
    let gini_score = if (0.3..=0.6).contains(&gini) {
        25.0
    } else if (0.15..=0.75).contains(&gini) {
        15.0
    } else {
        5.0
    };
    score += gini_score;

    // Survival rate: target 30%–80%
    let survival_score = if (0.3..=0.8).contains(&survival) {
        25.0
    } else if (0.15..=0.9).contains(&survival) {
        15.0
    } else {
        5.0
    };
    score += survival_score;

    // Wars per century per 100 factions: target 3–15
    let war_score = if (3.0..=15.0).contains(&wars_century) {
        25.0
    } else if (1.0..=25.0).contains(&wars_century) {
        15.0
    } else {
        5.0
    };
    score += war_score;

    // No monopoly
    let mono_score = if !monopoly { 25.0 } else { 5.0 };
    score += mono_score;

    score
}

fn score_narrative_richness(entropy: f32, causality: f32, personality_events: u32, unique_types: u32) -> f32 {
    let mut score = 0.0;

    // Entropy: max is ~5.5 bits for 45 types, good is > 3.0
    let entropy_score = (entropy / 5.0 * 25.0).min(25.0);
    score += entropy_score;

    // Causality: higher is better. Use stepped scoring since >10% requires major infrastructure.
    let causality_score = if causality >= 0.10 { 25.0 }
        else if causality >= 0.05 { 20.0 }
        else if causality >= 0.02 { 15.0 }
        else if causality >= 0.005 { 10.0 }
        else { (causality * 1000.0).min(5.0) };
    score += causality_score;

    // Personality events: more is better, target > 5
    let personality_score = (personality_events as f32 / 20.0 * 25.0).min(25.0);
    score += personality_score;

    // Unique types: target > 15
    let unique_score = (unique_types as f32 / 30.0 * 25.0).min(25.0);
    score += unique_score;

    score
}

fn score_behavioral_coherence(war_d: f32, dip_corr: f32, builder_corr: f32) -> f32 {
    // War coherence: Cohen's d — leaders who declare wars should have higher war_inclination
    // d > 0.2 small, > 0.5 medium, > 0.8 large
    let war_score = if war_d > 0.5 { 33.3 }
        else if war_d > 0.3 { 25.0 }
        else if war_d > 0.1 { 16.7 }
        else if war_d > 0.0 { 8.3 }
        else { 0.0 };

    // Diplomacy/builder: Pearson correlation
    let score_corr = |r: f32| -> f32 {
        if r > 0.5 { 33.3 }
        else if r > 0.3 { 25.0 }
        else if r > 0.1 { 16.7 }
        else if r > 0.0 { 8.3 }
        else { 0.0 }
    };
    war_score + score_corr(dip_corr) + score_corr(builder_corr)
}

fn score_religious_impact(
    holy_war_d: f32, pacifism_d: f32,
    schisms: u32, conversions: u32, multi_religion: u32,
) -> f32 {
    let mut score = 0.0;

    // HolyWar should increase wars (positive d)
    let hw_score = if holy_war_d > 0.5 { 20.0 }
        else if holy_war_d > 0.1 { 12.0 }
        else { 4.0 };
    score += hw_score;

    // Pacifism should decrease wars (negative d)
    let pac_score = if pacifism_d < -0.5 { 20.0 }
        else if pacifism_d < -0.1 { 12.0 }
        else { 4.0 };
    score += pac_score;

    // Schisms: at least 1 is good
    let schism_score = if schisms >= 3 { 20.0 }
        else if schisms >= 1 { 12.0 }
        else { 4.0 };
    score += schism_score;

    // Conversions: some is good
    let conv_score = if conversions >= 3 { 20.0 }
        else if conversions >= 1 { 12.0 }
        else { 4.0 };
    score += conv_score;

    // Multi-faction religions
    let multi_score = if multi_religion >= 3 { 20.0 }
        else if multi_religion >= 1 { 12.0 }
        else { 4.0 };
    score += multi_score;

    score
}

/// Welch's t-test between two sample sets.
/// Returns (t-statistic, degrees of freedom, is_significant at p<0.05).
pub fn welch_t_test(a: &[f32], b: &[f32]) -> (f32, f32, bool) {
    let n1 = a.len() as f32;
    let n2 = b.len() as f32;
    if n1 < 2.0 || n2 < 2.0 {
        return (0.0, 0.0, false);
    }
    let mean1 = a.iter().sum::<f32>() / n1;
    let mean2 = b.iter().sum::<f32>() / n2;
    let var1 = a.iter().map(|x| (x - mean1).powi(2)).sum::<f32>() / (n1 - 1.0);
    let var2 = b.iter().map(|x| (x - mean2).powi(2)).sum::<f32>() / (n2 - 1.0);
    let se = (var1 / n1 + var2 / n2).sqrt();
    if se < 1e-10 {
        return (0.0, 0.0, false);
    }
    let t = (mean1 - mean2) / se;
    let df_num = (var1 / n1 + var2 / n2).powi(2);
    let df_den = (var1 / n1).powi(2) / (n1 - 1.0) + (var2 / n2).powi(2) / (n2 - 1.0);
    let df = if df_den > 0.0 { df_num / df_den } else { 1.0 };
    // Approximate p<0.05 critical value for df>=5 is ~2.0 (conservative)
    let significant = t.abs() > 2.0 && df >= 4.0;
    (t, df, significant)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pearson_correlation_perfect() {
        let xs = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let ys = vec![2.0, 4.0, 6.0, 8.0, 10.0];
        let r = pearson_correlation(&xs, &ys);
        assert!((r - 1.0).abs() < 0.001, "Perfect correlation should be ~1.0, got {}", r);
    }

    #[test]
    fn test_pearson_correlation_negative() {
        let xs = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let ys = vec![10.0, 8.0, 6.0, 4.0, 2.0];
        let r = pearson_correlation(&xs, &ys);
        assert!((r - (-1.0)).abs() < 0.001, "Perfect neg correlation should be ~-1.0, got {}", r);
    }

    #[test]
    fn test_cohen_d() {
        let a = vec![5.0, 6.0, 7.0, 8.0, 9.0];
        let b = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let d = cohen_d(&a, &b);
        assert!(d > 1.0, "Large difference should give d > 1.0, got {}", d);
    }

    #[test]
    fn test_welch_t_test() {
        let a = vec![5.0, 6.0, 7.0, 8.0, 9.0];
        let b = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let (t, df, sig) = welch_t_test(&a, &b);
        assert!(t > 0.0, "t should be positive");
        assert!(df > 0.0, "df should be positive");
        assert!(sig, "Should be significant for large difference");
    }

    #[test]
    fn test_gini_equal() {
        // Test with mock data - equal populations
        let values: Vec<f64> = vec![100.0, 100.0, 100.0, 100.0];
        let n = values.len() as f64;
        let sum: f64 = values.iter().sum();
        let mut num = 0.0;
        for (i, &p) in values.iter().enumerate() {
            num += (2.0 * (i as f64 + 1.0) - n - 1.0) * p;
        }
        let gini = (num / (n * sum)) as f32;
        assert!(gini.abs() < 0.01, "Equal populations should have Gini ~0");
    }
}
