//! Who someone is: body, mind, character, values, likes and looks, rolled once from their
//! people's template (`data/defaults/persona.json`).
//!
//! The idea is Dwarf Fortress's: a species stores *distributions*, never values, and each
//! individual is a roll inside them, so every settler and figure is distinct yet stays in
//! character for their people. Every range is 7 breakpoints (six equally likely gaps, uniform
//! inside a gap), culture shifts the values as a second filter, and the words come from bands
//! defined next to the numbers, so the text can never contradict what the simulation reads.
//!
//! Nothing here draws from a shared RNG: a persona is a pure function of a seed (a figure's id
//! and name, a settler's name and the colony's seed), so adding it changed no history.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

const JSON: &str = include_str!("../data/defaults/persona.json");

/// The 19 attributes, in the data file's order. 1000 is an ordinary human.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Attr {
    Strength, Agility, Toughness, Endurance, Recuperation, DiseaseResistance,
    AnalyticalAbility, Focus, Willpower, Creativity, Intuition, Patience, Memory,
    LinguisticAbility, SpatialSense, Musicality, KinestheticSense, Empathy, SocialAwareness,
}
pub const N_ATTRS: usize = 19;

/// Personality facets, in the data file's order. 0-100, 50 ordinary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Facet {
    Love, Hate, Envy, Cheer, Gloom, Anger, Anxiety, StressVulnerability, Greed,
    Immoderation, Violence, Perseverance, Wastefulness, Discord, Friendliness, Politeness,
    Bravery, Confidence, Vanity, Ambition, Gratitude, Humour, Vengefulness, Pride,
    Cruelty, Curiosity, Bashfulness, Perfectionism, Tolerance, Altruism, Dutifulness,
    Orderliness, Trust, Gregariousness, ExcitementSeeking, Imagination, ArtInclined, Piety,
}
pub const N_FACETS: usize = 38;

/// Values (what someone holds dear), in the data file's order. -50..50.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Val {
    Law, Loyalty, Family, Friendship, Power, Truth, Cunning, Eloquence, Fairness,
    Decorum, Tradition, Artwork, Cooperation, Independence, Stoicism, Introspection,
    SelfControl, Tranquility, Harmony, Merriment, Craftsmanship, MartialProwess, Skill,
    HardWork, Sacrifice, Competition, Perseverance, Leisure, Commerce, Romance, Nature,
    Peace, Knowledge,
}
pub const N_VALUES: usize = 33;

const DEFAULT_ATTR: [f32; 7] = [200.0, 700.0, 900.0, 1000.0, 1100.0, 1300.0, 2000.0];
const DEFAULT_FACET: [f32; 4] = [0.0, 30.0, 70.0, 100.0];
const DEFAULT_FEATURE: [f32; 7] = [90.0, 95.0, 98.0, 100.0, 102.0, 105.0, 110.0];

/// A people's template.
#[derive(Clone, Debug)]
pub struct Template {
    pub word: String,
    /// What grows on the head: "hair", "crest", "mane", "none".
    pub covering: String,
    attrs: Vec<[f32; 7]>,
    facets: Vec<[f32; 4]>,
    values: Vec<f32>,
    features: HashMap<String, [f32; 7]>,
    hair: Vec<(String, u32)>,
    skin: Vec<(String, u32)>,
    eyes: Vec<(String, u32)>,
    hairstyles: Vec<String>,
    beards: String,
    grey_from: [f32; 7],
}

pub struct Data {
    attr_text: Vec<[String; 2]>,
    facet_text: Vec<[String; 4]>,
    value_names: Vec<String>,
    value_text: Vec<[String; 4]>,
    culture_values: Vec<Vec<(String, f32)>>,
    /// Feature name and its six phrases (by gap), in the file's order.
    features: Vec<(String, [Option<String>; 6])>,
    colours: HashMap<String, [u8; 3]>,
    materials: Vec<String>,
    land_materials: HashMap<String, String>,
    pref_colours: Vec<String>,
    creatures: Vec<(String, String)>,
    foods: Vec<String>,
    dislikes: Vec<String>,
    races: HashMap<String, Template>,
}

fn strs(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default()
}

fn nums<const N: usize>(v: &Value) -> Option<[f32; N]> {
    let a = v.as_array()?;
    if a.len() != N { return None; }
    let mut out = [0.0; N];
    for (o, x) in out.iter_mut().zip(a) { *o = x.as_f64()? as f32; }
    Some(out)
}

fn weighted(v: &Value) -> Vec<(String, u32)> {
    v.as_array().map(|a| a.iter().filter_map(|p| Some((p.get(0)?.as_str()?.to_string(), p.get(1)?.as_u64()? as u32))).collect()).unwrap_or_default()
}

fn texts<const N: usize>(v: &Value, names: &[String]) -> Vec<[String; N]> {
    names.iter().map(|n| {
        let a = strs(&v[n.as_str()]);
        assert_eq!(a.len(), N, "persona.json: {} needs {} phrases", n, N);
        std::array::from_fn(|i| a[i].clone())
    }).collect()
}

fn load() -> Data {
    let v: Value = serde_json::from_str(JSON).expect("persona.json");
    let attr_names = strs(&v["attributes"]);
    let facet_names = strs(&v["facets"]);
    let value_names = strs(&v["values"]);
    assert_eq!(attr_names.len(), N_ATTRS);
    assert_eq!(facet_names.len(), N_FACETS);
    assert_eq!(value_names.len(), N_VALUES);
    let features: Vec<(String, [Option<String>; 6])> = v["features"].as_object().unwrap().iter()
        .filter(|(k, _)| !k.starts_with('_'))
        .map(|(k, a)| {
            let a = a.as_array().unwrap();
            (k.clone(), std::array::from_fn(|i| a.get(i).and_then(|x| x.as_str()).map(String::from)))
        }).collect();
    let colours = v["colours"].as_object().unwrap().iter().filter(|(k, _)| !k.starts_with('_'))
        .filter_map(|(k, c)| nums::<3>(c).map(|c| (k.clone(), c.map(|x| x as u8)))).collect();
    let p = &v["preferences"];
    let land_materials = p["materials_of_the_land"].as_object().unwrap().iter().filter(|(k, _)| !k.starts_with('_'))
        .filter_map(|(k, m)| Some((k.clone(), m.as_str()?.to_string()))).collect();
    let creatures = p["creatures"].as_array().unwrap().iter().filter_map(|c| Some((c.get(0)?.as_str()?.to_string(), c.get(1)?.as_str()?.to_string()))).collect();
    let culture_values = value_names.iter().map(|n| {
        v["culture_values"][n.as_str()].as_array().map(|a| a.iter().filter_map(|w| Some((w.get(0)?.as_str()?.to_string(), w.get(1)?.as_f64()? as f32))).collect()).unwrap_or_default()
    }).collect();

    // Races: each starts from "default" and overrides what it lists.
    let raw = v["races"].as_object().unwrap();
    let base = &raw["default"];
    let template = |r: &Value| -> Template {
        let pick = |k: &str| if r.get(k).is_some() { &r[k] } else { &base[k] };
        let attrs = attr_names.iter().map(|n| nums::<7>(&r["attributes"][n.as_str()]).unwrap_or(DEFAULT_ATTR)).collect();
        let facets = facet_names.iter().map(|n| nums::<4>(&r["facets"][n.as_str()]).unwrap_or(DEFAULT_FACET)).collect();
        let values = value_names.iter().map(|n| r["values"][n.as_str()].as_f64().unwrap_or(0.0) as f32).collect();
        let mut feats: HashMap<String, [f32; 7]> = HashMap::new();
        for src in [base, r] {
            if let Some(o) = src["features"].as_object() {
                for (k, a) in o { if let Some(a) = nums::<7>(a) { feats.insert(k.clone(), a); } }
            }
        }
        Template {
            word: pick("word").as_str().unwrap_or("person").to_string(),
            covering: pick("covering").as_str().unwrap_or("hair").to_string(),
            attrs, facets, values, features: feats,
            hair: weighted(pick("hair")), skin: weighted(pick("skin")), eyes: weighted(pick("eyes")),
            hairstyles: strs(pick("hairstyles")),
            beards: pick("beards").as_str().unwrap_or("men").to_string(),
            grey_from: nums::<7>(pick("grey_from")).unwrap_or([35.0, 45.0, 50.0, 55.0, 60.0, 65.0, 75.0]),
        }
    };
    let races = raw.iter().filter(|(k, _)| !k.starts_with('_')).map(|(k, r)| (k.clone(), template(r))).collect();
    Data {
        attr_text: texts::<2>(&v["attribute_text"], &attr_names),
        facet_text: texts::<4>(&v["facet_text"], &facet_names),
        value_text: texts::<4>(&v["value_text"], &value_names),
        value_names,
        culture_values,
        features,
        colours,
        materials: strs(&p["materials"]),
        land_materials,
        pref_colours: strs(&p["colours"]),
        creatures,
        foods: strs(&p["foods"]),
        dislikes: strs(&p["dislikes"]),
        races,
    }
}

pub fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(load)
}

/// A people's template by race tag ("dwarf"), "default" for an unknown one.
pub fn template(race: &str) -> &'static Template {
    let d = data();
    d.races.get(race).unwrap_or_else(|| &d.races["default"])
}

/// The ink colour of a named hair, skin or eye colour (as the text says it).
pub fn colour(name: &str) -> Option<[f32; 3]> {
    data().colours.get(name).map(|c| c.map(|x| x as f32))
}

/// The material a liked thing is called by, for a tree kind or rock type of the land ("Broadleaf"
/// -> "oak").
pub fn material_of_the_land(kind: &str) -> Option<&'static str> {
    data().land_materials.get(kind).map(|s| s.as_str())
}

/// Deterministic dice (SplitMix64): a persona is a function of its seed.
struct Dice(u64);
impl Dice {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn f(&mut self) -> f32 { (self.next() >> 40) as f32 / (1u64 << 24) as f32 }
    fn pick<'a, T>(&mut self, v: &'a [T]) -> Option<&'a T> { if v.is_empty() { None } else { Some(&v[(self.next() % v.len() as u64) as usize]) } }
    fn weighted(&mut self, v: &[(String, u32)]) -> String {
        let total: u32 = v.iter().map(|x| x.1).sum();
        if total == 0 { return String::new(); }
        let mut r = (self.next() % total as u64) as u32;
        for (s, w) in v { if r < *w { return s.clone(); } r -= w; }
        v[0].0.clone()
    }
    /// The 7-breakpoint roll: a gap (0-5) with equal odds, then uniform inside it.
    fn seven(&mut self, r: &[f32; 7]) -> (f32, u8) {
        let gap = (self.next() % 6) as usize;
        (r[gap] + (r[gap + 1] - r[gap]) * self.f(), gap as u8)
    }
    /// A facet: thirds between four breakpoints.
    fn four(&mut self, r: &[f32; 4]) -> f32 {
        let third = (self.next() % 3) as usize;
        r[third] + (r[third + 1] - r[third]) * self.f()
    }
}

/// A string hashed into a seed (FNV-1a).
pub fn seed_of(s: &str, salt: u64) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    for b in s.bytes() { h ^= b as u64; h = h.wrapping_mul(0x100_0000_01b3); }
    h
}

/// Liked and disliked things.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Likes {
    pub material: String,
    pub colour: String,
    /// A creature and why ("boar", "their ferocity").
    pub creature: (String, String),
    pub food: String,
    pub dislike: String,
}

/// One individual: who they are, as numbers the simulation reads and words the player reads.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Persona {
    /// Race tag ("dwarf").
    pub race: String,
    pub female: bool,
    /// Attributes now (`Attr` order), and how far each can grow with use.
    pub attrs: Vec<f32>,
    pub caps: Vec<f32>,
    /// Which sixth of their people's range each attribute was rolled in (0-5).
    pub attr_gap: Vec<u8>,
    pub facets: Vec<u8>,
    pub values: Vec<i8>,
    /// Appearance features: (name, gap 0-5, value); height is in cm.
    pub looks: Vec<(String, u8, f32)>,
    pub hair: String,
    pub skin: String,
    pub eyes: String,
    pub hairstyle: String,
    pub beard: bool,
    /// The age their hair starts to grey.
    pub grey_at: u32,
    pub likes: Likes,
}

/// Culture values as the history holds them (0-1, 0.5 neutral), by name.
pub fn culture_map(c: &crate::history::entities::culture::CultureValues) -> Vec<(&'static str, f32)> {
    vec![("martial", c.martial), ("tradition", c.tradition), ("collectivism", c.collectivism),
         ("nature_harmony", c.nature_harmony), ("magic_acceptance", c.magic_acceptance),
         ("xenophobia", c.xenophobia), ("honor_value", c.honor_value), ("wealth", c.wealth)]
}

impl Persona {
    /// Roll someone of `race` (a tag; unknown races use "default"), shaped by their culture's
    /// values if known. Pure: the same arguments give the same person.
    pub fn roll(race: &str, culture: Option<&crate::history::entities::culture::CultureValues>, seed: u64) -> Persona {
        let d = data();
        let t = template(race);
        let mut dice = Dice(seed ^ 0x9E50_4A11);
        let female = dice.next() % 2 == 0;
        let mut attrs = Vec::with_capacity(N_ATTRS);
        let mut caps = Vec::with_capacity(N_ATTRS);
        let mut attr_gap = Vec::with_capacity(N_ATTRS);
        for r in &t.attrs {
            let (v, gap) = dice.seven(r);
            attrs.push(v);
            // Potential (DF): twice the roll, or the roll plus the people's median.
            caps.push((v * 2.0).max(v + r[3]));
            attr_gap.push(gap);
        }
        let facets = t.facets.iter().map(|r| dice.four(r).round().clamp(0.0, 100.0) as u8).collect();
        let cult = culture.map(culture_map).unwrap_or_default();
        let values = (0..N_VALUES).map(|i| {
            let shift: f32 = d.culture_values[i].iter().map(|(k, w)| {
                cult.iter().find(|(n, _)| n == k).map(|(_, c)| w * (c - 0.5) * 2.0).unwrap_or(0.0)
            }).sum();
            let personal = (dice.f() + dice.f() + dice.f() - 1.5) / 1.5 * 30.0;
            (t.values[i] + shift + personal).round().clamp(-50.0, 50.0) as i8
        }).collect();
        let mut looks = Vec::new();
        for (name, _) in &d.features {
            let r = t.features.get(name).copied().unwrap_or(DEFAULT_FEATURE);
            let (v, gap) = dice.seven(&r);
            looks.push((name.clone(), gap, v));
        }
        let hair = dice.weighted(&t.hair);
        let skin = dice.weighted(&t.skin);
        let eyes = dice.weighted(&t.eyes);
        let hairstyle = dice.pick(&t.hairstyles).cloned().unwrap_or_default();
        let beard_roll = dice.f();
        let beard = !female && match t.beards.as_str() { "men" => beard_roll < 0.9, "few" => beard_roll < 0.3, _ => false };
        let grey_at = dice.seven(&t.grey_from).0 as u32;
        let likes = Likes {
            material: dice.pick(&d.materials).cloned().unwrap_or_default(),
            colour: dice.pick(&d.pref_colours).cloned().unwrap_or_default(),
            creature: dice.pick(&d.creatures).cloned().unwrap_or_default(),
            food: dice.pick(&d.foods).cloned().unwrap_or_default(),
            dislike: dice.pick(&d.dislikes).cloned().unwrap_or_default(),
        };
        Persona { race: race.to_string(), female, attrs, caps, attr_gap, facets, values, looks, hair, skin, eyes, hairstyle, beard, grey_at, likes }
    }

    /// A history figure's persona, consistent with the personality the history acted on:
    /// bravery, cruelty, ambition, greed, trust, duty, piety, patience, charm and cunning are
    /// set from it, the rest rolled from the figure's id and name.
    pub fn of_figure(h: &crate::history::world_state::WorldHistory, f: &crate::history::entities::figures::Figure) -> Persona {
        let race = h.races.get(&f.race_id);
        let tag = race.map(|r| format!("{:?}", r.base_type).to_lowercase()).unwrap_or_else(|| "human".into());
        let culture = race.and_then(|r| h.cultures.get(&r.culture_id)).map(|c| &c.values);
        let mut p = Persona::roll(&tag, culture, seed_of(&f.name, f.id.0 as u64));
        let q = &f.personality;
        let pct = |x: f32| (x * 100.0).round().clamp(0.0, 100.0) as u8;
        p.facets[Facet::Bravery as usize] = pct(q.bravery);
        p.facets[Facet::Cruelty as usize] = pct(q.cruelty);
        p.facets[Facet::Ambition as usize] = pct(q.ambition);
        p.facets[Facet::Greed as usize] = pct(q.greed);
        p.facets[Facet::Trust as usize] = pct(1.0 - q.paranoia);
        p.facets[Facet::Dutifulness as usize] = pct(q.honor);
        p.facets[Facet::Piety as usize] = pct(q.piety);
        p.values[Val::Cunning as usize] = ((q.cunning - 0.5) * 90.0).round() as i8;
        p.values[Val::Fairness as usize] = ((q.honor - 0.5) * 80.0).round() as i8;
        // Patience and charm are abilities: place them in the people's own range.
        let t = template(&tag);
        let at = |a: Attr, x: f32| { let r = &t.attrs[a as usize]; let pos = x.clamp(0.0, 0.999) * 6.0; let g = pos as usize; (r[g] + (r[g + 1] - r[g]) * pos.fract(), g as u8) };
        for (a, x) in [(Attr::Patience, q.patience), (Attr::SocialAwareness, q.charisma), (Attr::LinguisticAbility, q.charisma)] {
            let (v, g) = at(a, x);
            p.attrs[a as usize] = v;
            p.caps[a as usize] = p.caps[a as usize].max(v);
            p.attr_gap[a as usize] = g;
        }
        p
    }

    pub fn attr(&self, a: Attr) -> f32 { self.attrs.get(a as usize).copied().unwrap_or(1000.0) }
    pub fn facet(&self, f: Facet) -> u8 { self.facets.get(f as usize).copied().unwrap_or(50) }
    pub fn value(&self, v: Val) -> i8 { self.values.get(v as usize).copied().unwrap_or(0) }

    /// A facet as a multiplier: 50 -> 1, 0 -> 1 - k, 100 -> 1 + k.
    pub fn lean(&self, f: Facet, k: f32) -> f32 { 1.0 + k * (self.facet(f) as f32 - 50.0) / 50.0 }

    /// Grow an attribute with use toward its cap (DF's improvement: the nearer the cap, the
    /// slower).
    pub fn train(&mut self, a: Attr, amount: f32) {
        let i = a as usize;
        if i >= self.attrs.len() { return; }
        let (v, cap) = (self.attrs[i], self.caps[i]);
        if cap > v { self.attrs[i] = (v + amount * (cap - v) / cap.max(1.0)).min(cap); }
    }

    fn geo(&self, a: &[Attr]) -> f32 {
        (a.iter().map(|&x| (self.attr(x).max(50.0) / 1000.0).ln()).sum::<f32>() / a.len() as f32).exp()
    }

    /// How long work takes, by kind (forage, fish, fell, haul, build, the colony's `skill_of`):
    /// 1 for an ordinary human, less for the strong at felling, the patient at fishing.
    pub fn work_time(&self, kind: usize) -> f32 {
        let g = match kind {
            0 => self.geo(&[Attr::Agility, Attr::SpatialSense, Attr::Intuition]),
            1 => self.geo(&[Attr::Patience, Attr::KinestheticSense, Attr::Focus]),
            2 => self.geo(&[Attr::Strength, Attr::Endurance, Attr::Agility]),
            3 => self.geo(&[Attr::Strength, Attr::Endurance]),
            _ => self.geo(&[Attr::Strength, Attr::SpatialSense, Attr::KinestheticSense]),
        };
        // Skill matters more than build (a master's hand is 0.7x, a green one 1.3x); the body
        // shifts it by about a sixth either way.
        g.powf(-0.15).clamp(0.85, 1.18)
    }

    /// Walking pace (1 ordinary): agility and endurance.
    pub fn walk(&self) -> f32 { self.geo(&[Attr::Agility, Attr::Endurance]).powf(0.25).clamp(0.85, 1.2) }

    /// How fast they tire (1 ordinary).
    pub fn tiring(&self) -> f32 { (self.attr(Attr::Endurance) / 1000.0).max(0.1).powf(-0.4).clamp(0.7, 1.5) }

    /// How fast they learn a skill (1 ordinary): focus, memory and sure hands.
    pub fn learning(&self) -> f32 { self.geo(&[Attr::Focus, Attr::Memory, Attr::KinestheticSense]).powf(0.6).clamp(0.6, 1.6) }

    /// How well they stand the cold and sickness (1 ordinary, higher is hardier).
    pub fn hardiness(&self) -> f32 { self.geo(&[Attr::Toughness, Attr::DiseaseResistance]).powf(0.5).clamp(0.6, 1.8) }

    /// How fast illness passes (1 ordinary, higher is quicker).
    pub fn healing(&self) -> f32 { (self.attr(Attr::Recuperation) / 1000.0).max(0.1).powf(0.5).clamp(0.4, 1.6) }

    // --- Words ---

    fn subject(&self) -> &'static str { if self.female { "She" } else { "He" } }

    /// Make a data phrase agree with this person ("their" -> "her").
    pub fn agree(&self, s: &str) -> String {
        let (pos, refl, they) = if self.female { ("her", "herself", "she goes") } else { ("his", "himself", "he goes") };
        s.replace("themselves", refl).replace("their", pos).replace("they go", they)
    }

    fn feature(&self, name: &str) -> Option<(String, f32)> {
        let d = data();
        let (_, phrases) = d.features.iter().find(|(n, _)| n == name)?;
        let (_, gap, v) = self.looks.iter().find(|(n, _, _)| n == name)?;
        phrases[*gap as usize].clone().map(|p| (p, *v))
    }

    pub fn height_cm(&self) -> f32 { self.looks.iter().find(|l| l.0 == "height").map(|l| l.2).unwrap_or(170.0) }

    /// The hair as it is at `age`: greying from `grey_at`.
    pub fn hair_at(&self, age: u32) -> String {
        if age >= self.grey_at + 25 { "white".into() }
        else if age >= self.grey_at + 10 { "grey".into() }
        else if age >= self.grey_at { format!("grey-streaked {}", self.hair) }
        else { self.hair.clone() }
    }

    /// Looks: "Ilda is a dwarf, aged 43. She is short for a dwarf and broad, with deep-set grey
    /// eyes, a long nose and high cheekbones. Her black hair is braided, streaked with grey."
    pub fn looks_text(&self, name: &str, age: u32) -> String {
        let t = template(&self.race);
        let mut out = format!("{} is {} {}, aged {}.", name, if t.word.starts_with(['a', 'e', 'i', 'o', 'u']) { "an" } else { "a" }, t.word, age);
        let mut body: Vec<String> = Vec::new();
        if let Some((h, _)) = self.feature("height") { body.push(format!("{} for {} {}", h, if t.word.starts_with(['a', 'e', 'i', 'o', 'u']) { "an" } else { "a" }, t.word)); }
        if let Some((b, _)) = self.feature("build") { body.push(b); }
        let mut parts: Vec<String> = Vec::new();
        let eye_word = self.eyes.trim_end_matches(" eyes").to_string();
        match self.feature("eyes") {
            Some((e, _)) => parts.push(format!("{} {} eyes", e, eye_word)),
            None => parts.push(format!("{} eyes", eye_word)),
        }
        for f in ["nose", "cheeks", "chin", "ears", "lips"] {
            if let Some((p, _)) = self.feature(f) { if parts.len() < 4 { parts.push(p); } }
        }
        let skin = self.skin.trim_end_matches(" skin");
        let s = self.subject();
        if body.is_empty() { out.push_str(&format!(" {} has {} skin, {}.", s, skin, list(&parts))); }
        else { out.push_str(&format!(" {} is {}, with {} skin, {}.", s, list(&body), skin, list(&parts))); }
        let pos = if self.female { "Her" } else { "His" };
        match t.covering.as_str() {
            "none" => {}
            "hair" => {
                let length = self.feature("hair_length").map(|x| format!("{} ", x.0)).unwrap_or_default();
                if self.hairstyle == "none" || self.hairstyle.is_empty() {
                    out.push_str(&format!(" {} {}{} hair", pos, length, self.hair_at(age)));
                } else {
                    out.push_str(&format!(" {} {}{} hair is {}", pos, length, self.hair_at(age), self.hairstyle));
                }
                if self.beard { out.push_str(&format!(", and {} wears a beard.", s.to_lowercase())); } else { out.push('.'); }
            }
            c => out.push_str(&format!(" {} {} {} is {}.", pos, self.hair, c, self.hairstyle)),
        }
        if let Some((v, _)) = self.feature("voice") { out.push_str(&format!(" {} has {}.", s, v)); }
        out
    }

    /// Body and mind: "She is mighty and very tough, but tires quickly and forgets things quickly."
    pub fn gifts_text(&self) -> Option<String> {
        let d = data();
        let mut good = Vec::new();
        let mut bad = Vec::new();
        for i in 0..N_ATTRS.min(self.attr_gap.len()) {
            match self.attr_gap[i] { 5 => good.push(self.agree(&d.attr_text[i][1])), 0 => bad.push(self.agree(&d.attr_text[i][0])), _ => {} }
        }
        good.truncate(4);
        bad.truncate(3);
        let s = self.subject();
        match (good.is_empty(), bad.is_empty()) {
            (true, true) => None,
            (false, true) => Some(format!("{} {}.", s, list(&good))),
            (true, false) => Some(format!("{} {}.", s, list(&bad))),
            (false, false) => Some(format!("{} {}, but {}.", s, list(&good), list(&bad))),
        }
    }

    /// The facet phrase for a band, if the facet is out of the ordinary.
    pub fn facet_phrase(&self, f: usize) -> Option<String> {
        let v = *self.facets.get(f)?;
        let band = match v { 0..=9 => 0, 10..=24 => 1, 76..=90 => 2, 91..=100 => 3, _ => return None };
        Some(self.agree(&data().facet_text[f][band]))
    }

    /// Character: the four facets furthest from ordinary.
    pub fn character_text(&self) -> Option<String> {
        let mut idx: Vec<usize> = (0..self.facets.len()).filter(|&i| self.facet_phrase(i).is_some()).collect();
        idx.sort_by_key(|&i| (std::cmp::Reverse((self.facets[i] as i32 - 50).abs()), i));
        let phrases: Vec<String> = idx.into_iter().take(4).filter_map(|i| self.facet_phrase(i)).collect();
        if phrases.is_empty() { None } else { Some(format!("{} {}.", self.subject(), list(&phrases))) }
    }

    /// What they hold dear: the three strongest values.
    pub fn values_text(&self) -> Option<String> {
        let d = data();
        let mut idx: Vec<usize> = (0..self.values.len()).filter(|&i| self.values[i].abs() >= 26).collect();
        idx.sort_by_key(|&i| (std::cmp::Reverse(self.values[i].abs()), i));
        let phrases: Vec<String> = idx.into_iter().take(3).map(|i| {
            let v = self.values[i];
            let band = if v <= -41 { 0 } else if v < 0 { 1 } else if v < 41 { 2 } else { 3 };
            self.agree(&d.value_text[i][band])
        }).collect();
        if phrases.is_empty() { return None; }
        // "values art and values craftsmanship" -> "values art and craftsmanship".
        let valued: Vec<String> = phrases.iter().filter_map(|p| p.strip_prefix("values ").map(String::from)).collect();
        let mut merged: Vec<String> = phrases.into_iter().filter(|p| !p.starts_with("values ")).collect();
        if !valued.is_empty() { merged.insert(0, format!("values {}", list(&valued))); }
        Some(format!("{} {}.", self.subject(), list(&merged)))
    }

    pub fn likes_text(&self) -> String {
        let l = &self.likes;
        format!("{} likes {}, the colour {}, {} for {} and {}, and cannot abide {}.",
            self.subject(), l.material, l.colour, l.creature.0, l.creature.1, l.food, l.dislike)
    }

    /// The whole description, as paragraphs: looks, gifts, character, values, likes.
    pub fn describe(&self, name: &str, age: u32) -> Vec<String> {
        let mut out = vec![self.looks_text(name, age)];
        if let Some(g) = self.gifts_text() { out.push(g); }
        let mut heart = String::new();
        if let Some(c) = self.character_text() { heart.push_str(&c); }
        if let Some(v) = self.values_text() { if !heart.is_empty() { heart.push(' '); } heart.push_str(&v); }
        if !heart.is_empty() { out.push(heart); }
        out.push(self.likes_text());
        out
    }

    /// The name of a value (for logs).
    pub fn value_name(v: Val) -> &'static str { &data().value_names[v as usize] }
    /// The name of the value at index `k` (0..N_VALUES).
    pub fn value_name_at(k: usize) -> &'static str { &data().value_names[k] }
}

/// "a, b and c".
pub fn list(items: &[String]) -> String {
    match items.len() {
        0 => String::new(),
        1 => items[0].clone(),
        n => format!("{} and {}", items[..n - 1].join(", "), items[n - 1]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_loads_and_rolls_are_pure() {
        let d = data();
        assert!(d.races.contains_key("dwarf") && d.races.contains_key("default"));
        let a = Persona::roll("dwarf", None, 7);
        let b = Persona::roll("dwarf", None, 7);
        assert_eq!(a.attrs, b.attrs);
        assert_eq!(a.describe("Ilda", 43), b.describe("Ilda", 43));
        assert_ne!(Persona::roll("dwarf", None, 8).attrs, a.attrs);
    }

    #[test]
    fn peoples_differ_as_their_templates_say() {
        // Over many rolls dwarves are stronger and shorter than elves, and elves more agile.
        let mean = |race: &str, f: &dyn Fn(&Persona) -> f32| (0..400).map(|s| f(&Persona::roll(race, None, s))).sum::<f32>() / 400.0;
        assert!(mean("dwarf", &|p| p.attr(Attr::Strength)) > mean("elf", &|p| p.attr(Attr::Strength)) + 300.0);
        assert!(mean("elf", &|p| p.attr(Attr::Agility)) > mean("dwarf", &|p| p.attr(Attr::Agility)) + 400.0);
        assert!(mean("dwarf", &|p| p.height_cm()) < mean("elf", &|p| p.height_cm()) - 40.0);
        assert!(mean("dwarf", &|p| p.work_time(2)) < mean("elf", &|p| p.work_time(2)));
    }

    #[test]
    fn words_match_numbers() {
        for s in 0..200 {
            let p = Persona::roll("human", None, s);
            let paras = p.describe("Ana", 30);
            let text = paras[..paras.len() - 1].join(" ");
            assert!(!text.contains("their") && !text.contains("themselves"), "{}", text);
            // A "mighty" person rolled in the top sixth of strength.
            if text.contains("is mighty") { assert_eq!(p.attr_gap[Attr::Strength as usize], 5); }
            if text.contains("is a coward") { assert!(p.facet(Facet::Bravery) <= 9); }
        }
        let p = Persona::roll("dwarf", None, 3);
        println!("{}", p.describe("Ilda", 43).join("\n"));
    }
}
