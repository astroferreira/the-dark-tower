//! Each people's arts: instruments, poems, music and dances, each made by someone.
//!
//! The idea is Dwarf Fortress's two-layer culture: a people gets a vocabulary (its instruments
//! and the shapes of its poetic, musical and dance forms, from `data/defaults/arts.json`, biased
//! by race and by what its culture values), and then every form is a work with provenance:
//! credited to a real figure of that people (the most creative of those who lived, by their
//! persona), in a year of their life and the town they lived in. Settlers carry their people's
//! forms (`settlers::Past::arts`) and perform them at the fire (`colony::mind`).
//!
//! Pure: built on demand from the people's id and the history, nothing saved, no RNG drawn.

use serde_json::Value;
use std::sync::OnceLock;
use crate::history::*;
use crate::history::world_state::WorldHistory;

const JSON: &str = include_str!("../../data/defaults/arts.json");

fn data() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| serde_json::from_str(JSON).expect("arts.json"))
}

fn strs(v: &Value) -> Vec<String> { v.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default() }

fn hash(seed: u64, salt: u64) -> u64 {
    let mut x = seed ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 31)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^ (x >> 29)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormKind { Poetic, Musical, Dance }

impl FormKind {
    pub fn word(self) -> &'static str { match self { FormKind::Poetic => "poem", FormKind::Musical => "music", FormKind::Dance => "dance" } }
}

#[derive(Clone, Debug)]
pub struct Instrument {
    /// Its own name in the people's tongue ("the gurnak").
    pub name: String,
    /// What kind of thing it is ("horn").
    pub kind: String,
    pub material: String,
    pub played: String,
}

impl Instrument {
    /// "the gurnak, a horn of aurochs horn, blown".
    pub fn describe(&self) -> String { format!("{}, {} of {}, {}", self.name, self.a_kind(), self.material, self.played) }
    /// "a horn", "a set of pipes".
    pub fn a_kind(&self) -> String { if self.kind.ends_with('s') { format!("a set of {}", self.kind) } else { format!("a {}", self.kind) } }
}

#[derive(Clone, Debug)]
pub struct Form {
    pub kind: FormKind,
    /// Its name ("the Ardenkalk").
    pub name: String,
    /// What it is ("a lament of four-line stanzas ...").
    pub what: String,
    pub author: Option<FigureId>,
    pub year: u32,
    pub town: Option<SettlementId>,
}

#[derive(Clone, Debug, Default)]
pub struct Arts {
    pub instruments: Vec<Instrument>,
    pub forms: Vec<Form>,
}

impl Arts {
    /// One line per form: "The Ardenkalk, a lament ... (made by Gagraarm at Brolmdustoor, 310)".
    pub fn lines(&self, h: &WorldHistory) -> Vec<String> {
        self.forms.iter().map(|f| {
            let who = f.author.and_then(|a| h.figures.get(&a)).map(|x| x.full_name());
            let at = f.town.and_then(|t| h.settlements.get(&t)).map(|t| t.name.clone());
            let prov = match (who, at) {
                (Some(w), Some(t)) => format!(" (made by {} at {}, {})", w, t, f.year),
                (Some(w), None) => format!(" (made by {}, {})", w, f.year),
                _ => format!(" (from {})", f.year),
            };
            format!("{}, {}{}", f.name, f.what, prov)
        }).collect()
    }
}

/// The arts of people `fid`.
pub fn of_people(h: &WorldHistory, fid: FactionId) -> Arts {
    use rand::SeedableRng;
    let Some(fac) = h.factions.get(&fid) else { return Arts::default() };
    let d = data();
    let race = h.races.get(&fac.race_id);
    let tag = race.map(|r| format!("{:?}", r.base_type).to_lowercase()).unwrap_or_else(|| "human".into());
    let culture = race.and_then(|r| h.cultures.get(&r.culture_id)).map(|c| c.values.clone());
    let seed = hash(fid.0, 0xA275);
    // Words in the people's tongue.
    let arche = race.map(|r| r.base_type.default_naming_archetype()).unwrap_or(crate::history::naming::styles::NamingArchetype::Compound);
    let style = crate::history::naming::styles::NamingStyle::from_archetype(NamingStyleId(0), arche);
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed);
    let mut word = || crate::history::naming::generator::NameGenerator::personal_name(&style, &mut rng);

    // Layer 1: the vocabulary. Two instruments of the race's kinds.
    let kinds = d["instruments"]["kinds"].as_array().cloned().unwrap_or_default();
    let by_race = { let r = strs(&d["instruments"]["by_race"][tag.as_str()]); if r.is_empty() { strs(&d["instruments"]["by_race"]["default"]) } else { r } };
    let mut instruments = Vec::new();
    for k in 0..4u64 {
        if instruments.len() >= 2 { break; }
        let kind = &by_race[(hash(seed, 10 + k) % by_race.len() as u64) as usize];
        if instruments.iter().any(|i: &Instrument| &i.kind == kind) { continue; }
        let Some(def) = kinds.iter().find(|x| x[0].as_str() == Some(kind.as_str())) else { continue };
        let mats = strs(&def[2]);
        let material = mats[(hash(seed, 20 + k) % mats.len().max(1) as u64) as usize % mats.len().max(1)].clone();
        instruments.push(Instrument { name: format!("the {}", word().to_lowercase()), kind: kind.clone(), material, played: def[1].as_str().unwrap_or("played").to_string() });
    }
    // Subjects lean on what the culture values.
    let mut subjects = strs(&d["subjects"]["any"]);
    if let Some(c) = &culture {
        for (v, key) in [(c.martial, "martial"), (c.tradition, "tradition"), (c.nature_harmony, "nature"), (c.wealth, "wealth"), (c.honor_value, "honor")] {
            if v > 0.6 { let mut s = strs(&d["subjects"][key]); s.extend(subjects.drain(..)); subjects = s; }
        }
    }
    // Layer 2: works, each credited to a real figure of the people: the most creative and
    // art-loving of those who lived (persona), in turn, in a year of their adult life.
    let mut makers: Vec<(f32, &crate::history::entities::figures::Figure)> = h.figures.values().filter(|f| f.faction == Some(fid)).map(|f| {
        let p = crate::persona::Persona::of_figure(h, f);
        (p.attr(crate::persona::Attr::Creativity) / 1000.0 + p.facet(crate::persona::Facet::ArtInclined) as f32 / 50.0 + p.attr(crate::persona::Attr::Musicality) / 2000.0, f)
    }).collect();
    makers.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.id.cmp(&b.1.id)));
    let mut forms = Vec::new();
    let mut used: Vec<String> = Vec::new();
    let plan = [(FormKind::Poetic, "poetic"), (FormKind::Musical, "musical"), (FormKind::Poetic, "poetic"), (FormKind::Dance, "dance"), (FormKind::Musical, "musical")];
    for (k, (kind, key)) in plan.iter().enumerate() {
        let templates = strs(&d[*key]);
        if templates.is_empty() { continue; }
        // A shape not used before by this people.
        let first = (hash(seed, 40 + k as u64) % templates.len() as u64) as usize;
        let Some(t) = (0..templates.len()).map(|j| &templates[(first + j) % templates.len()]).find(|t| !used.contains(*t)) else { continue };
        used.push(t.clone());
        let instrument = instruments.get(k % instruments.len().max(1)).map(|i| format!("{} ({})", i.name.trim_start_matches("the "), i.a_kind())).unwrap_or_else(|| "drum".into());
        let what = t.replace("{instrument}", &instrument).replace("{subject}", &subjects[(hash(seed, 50 + k as u64) % subjects.len() as u64) as usize])
            .replace("{lines}", ["three", "four", "five", "six", "seven", "nine"][(hash(seed, 60 + k as u64) % 6) as usize]);
        let maker = makers.get(k % makers.len().max(1)).map(|m| m.1);
        let (year, town) = match maker {
            Some(f) => {
                let start = (f.birth_date.year + 20).max(fac.founded.year);
                let end = f.death_date.map(|d| d.year).unwrap_or(h.current_date.year).max(start);
                let y = start + (hash(seed, 70 + k as u64) % (end - start + 1) as u64) as u32;
                let town = h.people.as_ref().and_then(|p| p.home.get(&f.id)).copied().or(fac.capital);
                (y, town)
            }
            None => (fac.founded.year, fac.capital),
        };
        forms.push(Form { kind: *kind, name: format!("the {}", word()), what, author: maker.map(|f| f.id), year, town });
    }
    forms.sort_by_key(|f| f.year);
    Arts { instruments, forms }
}

#[cfg(test)]
mod tests {
    #[test]
    fn arts_data_parses() {
        let d = super::data();
        assert!(d["poetic"].as_array().map_or(0, |a| a.len()) >= 4);
        assert!(d["instruments"]["kinds"].as_array().map_or(0, |a| a.len()) >= 6);
    }
}
