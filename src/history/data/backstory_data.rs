//! Backstory templates loaded from JSON.

use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use rand::Rng;

/// A title+description template pair.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TitleDescTemplate {
    pub title: String,
    pub desc: String,
}

/// Disease-aware death templates (for Disease cause).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiseaseDeathTemplates {
    pub templates: Vec<TitleDescTemplate>,
    pub diseases: Vec<String>,
}

/// Death templates can be either a simple list or a disease-aware structure.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DeathTemplateEntry {
    Simple(Vec<TitleDescTemplate>),
    WithDiseases(DiseaseDeathTemplates),
}

/// Reign event template with event type tag.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReignEventTemplate {
    pub event_type: String,
    pub title: String,
    pub desc: String,
}

/// All backstory templates loaded from JSON.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BackstoryTemplates {
    pub common_epithets: Vec<String>,
    pub race_epithets: HashMap<String, Vec<String>>,
    #[serde(default = "default_epithet_chance")]
    pub race_epithet_chance: f32,
    pub ruler_titles: HashMap<String, Vec<String>>,
    pub dynasty_patterns: HashMap<String, Vec<String>>,
    pub coronation_founding: HashMap<String, Vec<TitleDescTemplate>>,
    pub coronation_succession: Vec<TitleDescTemplate>,
    pub death_templates: HashMap<String, DeathTemplateEntry>,
    pub reign_event_templates: Vec<ReignEventTemplate>,
    pub enemy_names: HashMap<String, Vec<String>>,
    pub faction_adjectives: Vec<String>,
    pub plague_names: Vec<String>,
    pub beast_names: Vec<String>,
    pub succession_templates: Vec<TitleDescTemplate>,
}

fn default_epithet_chance() -> f32 {
    0.4
}

impl BackstoryTemplates {
    /// Get ruler titles for a race tag, falling back to "_default".
    pub fn ruler_titles_for(&self, tag: &str) -> &[String] {
        self.ruler_titles.get(tag)
            .or_else(|| self.ruler_titles.get("_default"))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Get dynasty patterns for a race tag, falling back to "_default".
    pub fn dynasty_patterns_for(&self, tag: &str) -> &[String] {
        self.dynasty_patterns.get(tag)
            .or_else(|| self.dynasty_patterns.get("_default"))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Get coronation founding templates for a race tag, falling back to "_default".
    pub fn coronation_founding_for(&self, tag: &str) -> &[TitleDescTemplate] {
        self.coronation_founding.get(tag)
            .or_else(|| self.coronation_founding.get("_default"))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Get death templates for a cause name, falling back to "_default".
    pub fn death_templates_for(&self, cause: &str) -> &DeathTemplateEntry {
        static DEFAULT_ENTRY: std::sync::LazyLock<DeathTemplateEntry> = std::sync::LazyLock::new(|| {
            DeathTemplateEntry::Simple(vec![TitleDescTemplate {
                title: "Death of {N}".to_string(),
                desc: "{N} has died.".to_string(),
            }])
        });
        self.death_templates.get(cause)
            .or_else(|| self.death_templates.get("_default"))
            .unwrap_or(&DEFAULT_ENTRY)
    }

    /// Get enemy names for a race tag, falling back to "_default".
    pub fn enemy_names_for(&self, tag: &str) -> &[String] {
        self.enemy_names.get(tag)
            .or_else(|| self.enemy_names.get("_default"))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Get race-specific epithets, falling back to "_default".
    pub fn race_epithets_for(&self, tag: &str) -> &[String] {
        self.race_epithets.get(tag)
            .or_else(|| self.race_epithets.get("_default"))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Pick a random epithet (race-specific or common).
    pub fn random_epithet(&self, tag: &str, rng: &mut impl Rng) -> String {
        let race_specific = self.race_epithets_for(tag);
        if !race_specific.is_empty() && rng.gen::<f32>() < self.race_epithet_chance {
            race_specific[rng.gen_range(0..race_specific.len())].clone()
        } else if !self.common_epithets.is_empty() {
            self.common_epithets[rng.gen_range(0..self.common_epithets.len())].clone()
        } else {
            "the Unknown".to_string()
        }
    }

    /// Pick a random ruler title for a race tag.
    pub fn random_ruler_title(&self, tag: &str, rng: &mut impl Rng) -> String {
        let titles = self.ruler_titles_for(tag);
        if titles.is_empty() {
            "Ruler".to_string()
        } else {
            titles[rng.gen_range(0..titles.len())].clone()
        }
    }

    /// Generate a dynasty name for a race tag using the founder's name.
    pub fn dynasty_name(&self, tag: &str, founder: &str, rng: &mut impl Rng) -> String {
        let patterns = self.dynasty_patterns_for(tag);
        if patterns.is_empty() {
            format!("House of {}", founder)
        } else {
            patterns[rng.gen_range(0..patterns.len())].replace("{}", founder)
        }
    }

    /// Pick a random enemy name for a race tag.
    pub fn random_enemy(&self, tag: &str, rng: &mut impl Rng) -> String {
        let enemies = self.enemy_names_for(tag);
        if enemies.is_empty() {
            "barbarian".to_string()
        } else {
            enemies[rng.gen_range(0..enemies.len())].clone()
        }
    }

    /// Pick a random faction adjective.
    pub fn random_faction_adjective(&self, rng: &mut impl Rng) -> String {
        if self.faction_adjectives.is_empty() {
            "distant".to_string()
        } else {
            self.faction_adjectives[rng.gen_range(0..self.faction_adjectives.len())].clone()
        }
    }

    /// Pick a random plague name.
    pub fn random_plague(&self, rng: &mut impl Rng) -> String {
        if self.plague_names.is_empty() {
            "Plague".to_string()
        } else {
            self.plague_names[rng.gen_range(0..self.plague_names.len())].clone()
        }
    }

    /// Pick a random beast name.
    pub fn random_beast(&self, rng: &mut impl Rng) -> String {
        if self.beast_names.is_empty() {
            "beast".to_string()
        } else {
            self.beast_names[rng.gen_range(0..self.beast_names.len())].clone()
        }
    }

    /// Generate a coronation description (founding or succession).
    pub fn coronation_description(
        &self,
        name: &str,
        faction_name: &str,
        ruler_title: &str,
        generation: u32,
        predecessor_name: Option<&str>,
        race_tag: &str,
        rng: &mut impl Rng,
    ) -> (String, String) {
        if generation == 0 {
            let templates = self.coronation_founding_for(race_tag);
            if templates.is_empty() {
                return (
                    format!("Founding of {}", faction_name),
                    format!("{} founded {}.", name, faction_name),
                );
            }
            let t = &templates[rng.gen_range(0..templates.len())];
            let title = t.title.replace("{N}", name).replace("{F}", faction_name).replace("{T}", ruler_title);
            let desc = t.desc.replace("{N}", name).replace("{F}", faction_name).replace("{T}", ruler_title);
            (title, desc)
        } else {
            let pred = predecessor_name.unwrap_or("the previous ruler");
            if self.coronation_succession.is_empty() {
                return (
                    format!("{} crowned ruler of {}", name, faction_name),
                    format!("{} succeeded {}.", name, pred),
                );
            }
            let t = &self.coronation_succession[rng.gen_range(0..self.coronation_succession.len())];
            let title = t.title.replace("{N}", name).replace("{F}", faction_name).replace("{T}", ruler_title).replace("{P}", pred);
            let desc = t.desc.replace("{N}", name).replace("{F}", faction_name).replace("{T}", ruler_title).replace("{P}", pred);
            (title, desc)
        }
    }

    /// Generate a death description for a given cause.
    pub fn death_description(
        &self,
        full_name: &str,
        short_name: &str,
        faction_name: &str,
        cause: &str,
        rng: &mut impl Rng,
    ) -> (String, String) {
        let entry = self.death_templates_for(cause);
        match entry {
            DeathTemplateEntry::Simple(templates) => {
                if templates.is_empty() {
                    return (format!("Death of {}", full_name), format!("{} has died.", full_name));
                }
                let t = &templates[rng.gen_range(0..templates.len())];
                let title = t.title.replace("{N}", full_name).replace("{S}", short_name).replace("{F}", faction_name);
                let desc = t.desc.replace("{N}", full_name).replace("{S}", short_name).replace("{F}", faction_name);
                (title, desc)
            }
            DeathTemplateEntry::WithDiseases(dd) => {
                let disease = if dd.diseases.is_empty() {
                    "an unknown illness".to_string()
                } else {
                    dd.diseases[rng.gen_range(0..dd.diseases.len())].clone()
                };
                if dd.templates.is_empty() {
                    return (format!("Death of {}", full_name), format!("{} died of {}.", full_name, disease));
                }
                let t = &dd.templates[rng.gen_range(0..dd.templates.len())];
                let title = t.title.replace("{N}", full_name).replace("{S}", short_name).replace("{F}", faction_name).replace("{D}", &disease);
                let desc = t.desc.replace("{N}", full_name).replace("{S}", short_name).replace("{F}", faction_name).replace("{D}", &disease);
                (title, desc)
            }
        }
    }

    /// Generate a succession event description (for step.rs live simulation).
    pub fn succession_description(
        &self,
        new_name: &str,
        dead_name: &str,
        faction_name: &str,
        rng: &mut impl Rng,
    ) -> (String, String) {
        if self.succession_templates.is_empty() {
            return (
                format!("Coronation of {}", new_name),
                format!("{} succeeded {} as ruler of {}.", new_name, dead_name, faction_name),
            );
        }
        let t = &self.succession_templates[rng.gen_range(0..self.succession_templates.len())];
        let title = t.title.replace("{N}", new_name).replace("{F}", faction_name).replace("{DEAD}", dead_name);
        let desc = t.desc.replace("{N}", new_name).replace("{F}", faction_name).replace("{DEAD}", dead_name);
        (title, desc)
    }
}
