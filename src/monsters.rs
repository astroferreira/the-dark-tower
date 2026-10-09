//! Generated monsters: what a beast *is*, as data, with words that cannot contradict it.
//!
//! The idea is Dwarf Fortress's forgotten beasts and titans: an ordered decision list where
//! each step narrows the next (body profile with what it must and cannot have, a structural
//! tweak, a body class or one uniform substance, a class tweak or odd eyes, a special attack,
//! colours), with thematic spheres biasing every pick, and every choice owning the text
//! fragments that describe it. The tables live in `data/defaults/monsters.json`.
//!
//! A legendary beast of the history gets its monster from what the history already says about
//! it (`of_legend`): its species' body (wings, tail, tentacles...), material and powers, its
//! epithet ("Storm-Caller" -> storm) and its lair's land (a volcano -> fire). Pure: seeded by the
//! beast's id and name, so no history changed.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

const JSON: &str = include_str!("../data/defaults/monsters.json");

#[derive(Clone, Debug)]
struct Profile { name: String, base: String, class: String, weight: f32, must: Vec<String>, cannot: Vec<String>, min_size: f32, spheres: Vec<String>, attack: Option<String> }

impl AttackDef {
    /// "breathed fire on Thano".
    pub fn did_to(&self, whom: &str) -> String { self.verb.replace("{}", whom) }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AttackDef {
    pub name: String,
    pub warning: String,
    /// For the log, with `{}` for whom it struck: "breathed fire on {}".
    pub verb: String,
    /// burn, poison, web, bleed, blind, chill, sicken.
    pub effect: String,
    pub ill_days: u32,
    /// Added to a raid's danger.
    pub deadly: f32,
    needs: Vec<String>,
    needs_base: Vec<String>,
    spheres: Vec<String>,
}

#[derive(Clone, Debug)]
struct Tweak { adj: String, with: String, has: String, flies: bool }

struct Data {
    bases: HashMap<String, (u32, Vec<String>)>,
    profiles: Vec<Profile>,
    tweaks: HashMap<String, Tweak>,
    /// Class -> (skin, blood, [(adj, has, new skin word)]).
    classes: HashMap<String, (String, String, Vec<(String, String, Option<String>)>)>,
    /// (key, adj, has, weight, evil), in file order.
    eyes: Vec<(String, String, String, u32, bool)>,
    /// (material, spheres), in file order.
    materials: Vec<(String, Vec<String>)>,
    attacks: Vec<AttackDef>,
    colours: HashMap<String, Vec<String>>,
    extras_default: Vec<String>,
    extras_evil: Vec<String>,
    /// (threshold, words), largest first.
    sizes: Vec<(f32, Vec<String>)>,
    kinds: HashMap<String, (String, Vec<String>)>,
}

fn strs(v: &Value) -> Vec<String> { v.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default() }
fn entries(v: &Value) -> Vec<(&String, &Value)> { v.as_object().map(|o| o.iter().filter(|(k, _)| !k.starts_with('_')).collect()).unwrap_or_default() }

fn load() -> Data {
    let v: Value = serde_json::from_str(JSON).expect("monsters.json");
    let bases = entries(&v["bases"]).into_iter().map(|(k, b)| (k.clone(), (b["legs"].as_u64().unwrap_or(0) as u32, strs(&b["allows"])))).collect();
    let profiles = v["profiles"].as_array().unwrap().iter().map(|p| Profile {
        name: p["name"].as_str().unwrap().into(), base: p["base"].as_str().unwrap().into(), class: p["class"].as_str().unwrap().into(),
        weight: p["weight"].as_f64().unwrap_or(1.0) as f32, must: strs(&p["must"]), cannot: strs(&p["cannot"]),
        min_size: p["min_size"].as_f64().unwrap_or(0.0) as f32, spheres: strs(&p["spheres"]), attack: p["attack"].as_str().map(String::from),
    }).collect();
    let tweaks = entries(&v["tweaks"]).into_iter().map(|(k, t)| (k.clone(), Tweak {
        adj: t["adj"].as_str().unwrap_or("").into(), with: t["with"].as_str().unwrap_or("").into(), has: t["has"].as_str().unwrap_or("").into(), flies: t["flies"].as_bool().unwrap_or(false),
    })).collect();
    let classes = entries(&v["classes"]).into_iter().map(|(k, c)| (k.clone(), (
        c["skin"].as_str().unwrap_or("skin").into(), c["blood"].as_str().unwrap_or("blood").into(),
        c["tweaks"].as_array().map(|a| a.iter().filter_map(|t| Some((t.get(0)?.as_str()?.to_string(), t.get(1)?.as_str()?.to_string(), t.get(2).and_then(|x| x.as_str()).map(String::from)))).collect()).unwrap_or_default(),
    ))).collect();
    let eyes = entries(&v["eyes"]).into_iter().map(|(k, e)| (k.clone(), e["adj"].as_str().unwrap_or("").into(), e["has"].as_str().unwrap_or("").into(), e["weight"].as_u64().unwrap_or(1) as u32, e["evil"].as_bool().unwrap_or(false))).collect();
    let materials = entries(&v["materials"]).into_iter().map(|(k, s)| (k.clone(), strs(s))).collect();
    let attacks = entries(&v["attacks"]).into_iter().map(|(k, a)| AttackDef {
        name: k.clone(), warning: a["warning"].as_str().unwrap_or("").into(), verb: a["verb"].as_str().unwrap_or("struck").into(),
        effect: a["effect"].as_str().unwrap_or("bleed").into(), ill_days: a["ill_days"].as_u64().unwrap_or(1) as u32, deadly: a["deadly"].as_f64().unwrap_or(0.0) as f32,
        needs: strs(&a["needs"]), needs_base: strs(&a["needs_base"]), spheres: strs(&a["spheres"]),
    }).collect();
    let colours = entries(&v["colours"]).into_iter().map(|(k, c)| (k.clone(), strs(c))).collect();
    let mut sizes: Vec<(f32, Vec<String>)> = entries(&v["sizes"]).into_iter().filter_map(|(_, s)| Some((s.get(0)?.as_f64()? as f32, strs(s.get(1)?)))).collect();
    sizes.sort_by(|a, b| b.0.total_cmp(&a.0));
    let kinds = entries(&v["kinds"]).into_iter().map(|(k, d)| (k.clone(), (d["word"].as_str().unwrap_or("beast").into(), strs(&d["spheres"])))).collect();
    Data { bases, profiles, tweaks, classes, eyes, materials, attacks, colours, extras_default: strs(&v["extras"]["default"]), extras_evil: strs(&v["extras"]["evil"]), sizes, kinds }
}

fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(load)
}

/// Deterministic dice (SplitMix64).
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
    fn one_in(&mut self, n: u64) -> bool { self.next() % n == 0 }
    fn pick<'a, T>(&mut self, v: &'a [T]) -> Option<&'a T> { if v.is_empty() { None } else { Some(&v[(self.next() % v.len() as u64) as usize]) } }
    /// Weighted pick of an index (weights in order: no hash-map iteration, so the seed is stable).
    fn weighted(&mut self, w: &[f32]) -> Option<usize> {
        let total: f32 = w.iter().sum();
        if total <= 0.0 { return None; }
        let mut r = self.f() * total;
        for (i, x) in w.iter().enumerate() { if r < *x { return Some(i); } r -= x; }
        Some(w.len() - 1)
    }
}

/// What to make.
#[derive(Clone, Debug, Default)]
pub struct Request {
    /// "beast", "forgotten" (a forgotten beast of the deep), "titan".
    pub kind: String,
    /// Themes: fire, cold, water, earth, death, darkness, night, plague, storm, nature, sun, sky, magic.
    pub spheres: Vec<String>,
    /// 1 = a large bear.
    pub size: f32,
    /// Structural parts it must have (wings, tail, tentacles, horns, mandibles...).
    pub must: Vec<String>,
    /// A body base it should be (serpent, insect, spider, humanoid...).
    pub base: Option<String>,
    pub attack: Option<String>,
    /// A substance its whole body is made of.
    pub material: Option<String>,
    /// A thing of evil (the dark eye tweaks and the evil extras).
    pub evil: bool,
}

/// One generated monster.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Monster {
    /// Its kind's word ("forgotten beast") and the body it was made on ("hornet").
    pub kind_word: String,
    pub profile: String,
    pub base: String,
    pub legs: u32,
    pub class: String,
    pub skin: String,
    pub blood: String,
    pub material: Option<String>,
    pub tweaks: Vec<String>,
    pub eyes: Option<String>,
    pub glow: Option<String>,
    pub colour: String,
    pub attack: Option<AttackDef>,
    pub flies: bool,
    pub size: f32,
    pub spheres: Vec<String>,
    /// A short phrase: "a towering eyeless hornet of obsidian".
    pub short: String,
    /// The full description, in sentences.
    pub description: String,
    /// What a beast of the history holds in its lair ("The Staff of Greenburg, a superior staff")
    /// and how many it has killed there; empty and 0 for a made-up monster.
    pub hoard: Vec<String>,
    pub kills: usize,
}

fn cap(s: &str) -> String { let mut c = s.chars(); match c.next() { Some(f) => f.to_uppercase().collect::<String>() + c.as_str(), None => String::new() } }
fn article(s: &str) -> &'static str { if s.starts_with(['a', 'e', 'i', 'o', 'u']) { "an" } else { "a" } }

/// Make a monster.
pub fn generate(req: &Request, seed: u64) -> Monster {
    let d = data();
    let mut dice = Dice(seed ^ 0xB1A5_7EAD);
    let mut spheres = req.spheres.clone();
    if let Some((_, extra)) = d.kinds.get(&req.kind) { for s in extra { if !spheres.contains(s) { spheres.push(s.clone()); } } }
    let size = req.size.max(0.1);
    let share = |a: &[String]| a.iter().filter(|s| spheres.contains(s)).count() as f32;

    // 1. The body profile: fits the parts it must have, its size, the hinted base; spheres pull.
    let fits = |p: &Profile| {
        let allows = d.bases.get(&p.base).map(|b| &b.1);
        p.min_size <= size
            && req.must.iter().all(|m| p.must.contains(m) || allows.map_or(false, |a| a.contains(m)))
            && !req.must.iter().any(|m| p.cannot.contains(m))
    };
    let weights: Vec<f32> = d.profiles.iter().map(|p| if !fits(p) { 0.0 } else {
        p.weight * (1.0 + 2.0 * share(&p.spheres)) * if req.base.as_deref() == Some(p.base.as_str()) { 8.0 } else if req.base.is_some() { 0.3 } else { 1.0 }
    }).collect();
    let p = d.profiles[dice.weighted(&weights).unwrap_or(0)].clone();
    let (legs, allows) = d.bases.get(&p.base).cloned().unwrap_or((4, Vec::new()));

    // 2. Structural tweaks: what the profile and the request demand, then maybe one more the body allows.
    let mut tweaks: Vec<String> = Vec::new();
    for t in p.must.iter().chain(req.must.iter()) {
        if !tweaks.contains(t) && (allows.contains(t) || p.must.contains(t)) && !p.cannot.contains(t) { tweaks.push(t.clone()); }
    }
    if tweaks.len() < 2 && dice.f() < 0.7 {
        let open: Vec<&String> = allows.iter().filter(|t| !tweaks.contains(t) && !p.cannot.contains(t)).collect();
        if let Some(t) = dice.pick(&open) { tweaks.push((*t).clone()); }
    }
    let flies = tweaks.iter().any(|t| d.tweaks.get(t).map_or(false, |x| x.flies));

    // 3. Class, or one uniform substance (1 in 12; 1 in 4 where a sphere is elemental).
    let elemental = spheres.iter().any(|s| matches!(s.as_str(), "fire" | "cold" | "earth"));
    let material = req.material.clone().or_else(|| {
        if dice.one_in(if elemental { 4 } else { 12 }) {
            let w: Vec<f32> = d.materials.iter().map(|(_, s)| 0.2 + share(s)).collect();
            dice.weighted(&w).map(|i| d.materials[i].0.clone())
        } else { None }
    });
    let (mut skin, blood, class_tweaks) = d.classes.get(&p.class).cloned().unwrap_or(("skin".into(), "blood".into(), Vec::new()));

    // 4. A class tweak, or odd eyes (dark ones only for an evil thing); glowing eyes 1 in 8.
    let mut second: Option<(String, String)> = None;
    let mut eyes = None;
    if material.is_none() && !class_tweaks.is_empty() && dice.f() < 0.5 {
        if let Some((adj, has, new_skin)) = dice.pick(&class_tweaks).cloned() {
            if let Some(n) = new_skin { skin = n; }
            second = Some((adj, has));
        }
    } else if dice.f() < 0.6 {
        let w: Vec<f32> = d.eyes.iter().map(|e| if e.4 && !req.evil { 0.0 } else { e.3 as f32 * if e.4 { 2.0 } else { 1.0 } }).collect();
        if let Some(i) = dice.weighted(&w) {
            let e = &d.eyes[i];
            second = Some((e.1.clone(), e.2.clone()));
            eyes = Some(e.0.clone());
        }
    }
    let colour_list = spheres.iter().find_map(|s| d.colours.get(s)).or_else(|| d.colours.get("any")).cloned().unwrap_or_default();
    let glow = if eyes.as_deref() != Some("eyeless") && dice.one_in(8) { dice.pick(&colour_list).cloned() } else { None };

    // 5. A special attack: always one (forgotten beasts and titans have one), as hinted, the
    //    profile's own, else weighed by the spheres among those the body can make.
    let attack = req.attack.clone().or(p.attack.clone()).and_then(|a| d.attacks.iter().find(|x| x.name == a).cloned()).or_else(|| {
        let w: Vec<f32> = d.attacks.iter().map(|a| {
            if !a.needs.iter().all(|n| tweaks.contains(n)) || (!a.needs_base.is_empty() && !a.needs_base.contains(&p.base)) { 0.0 } else { 0.15 + share(&a.spheres) * 4.0 }
        }).collect();
        dice.weighted(&w).map(|i| d.attacks[i].clone())
    });

    // 6. Colours.
    let colour = dice.pick(&colour_list).cloned().unwrap_or_else(|| "grey".into());

    // 7. Words: each from the choice that made it.
    let size_word = d.sizes.iter().find(|(t, _)| size >= *t).and_then(|(_, w)| dice.pick(w).cloned()).unwrap_or_else(|| "large".into());
    let first = tweaks.first().and_then(|t| d.tweaks.get(t)).cloned();
    let mut adjs: Vec<String> = vec![size_word];
    if let Some((a, _)) = &second { adjs.push(a.clone()); }
    if let Some(t) = &first { if !adjs.contains(&t.adj) { adjs.push(t.adj.clone()); } }
    let of = material.as_ref().map(|m| format!(" of {}", m)).unwrap_or_default();
    let head = format!("{} {}", adjs.join(" "), p.name);
    let short = format!("{} {}{}", article(&head), head, of);
    let mut text = format!("{}{}.", cap(&format!("{} {}", article(&head), head)), material.as_ref().map(|m| format!(", composed entirely of {}", m)).unwrap_or_default());
    let mut has: Vec<String> = Vec::new();
    for t in tweaks.iter().skip(1).take(1).filter_map(|t| d.tweaks.get(t)) { has.push(t.has.clone()); }
    if let Some(t) = &first { has.insert(0, t.has.clone()); }
    if let Some((_, h)) = &second { has.push(h.clone()); }
    let extras = if req.evil { &d.extras_evil } else { &d.extras_default };
    if let Some(e) = dice.pick(extras) { has.push(e.clone()); }
    if !has.is_empty() {
        let n = has.len();
        let joined = if n == 1 { has[0].clone() } else { format!("{} and {}", has[..n - 1].join(", "), has[n - 1]) };
        text.push_str(&format!(" {}.", cap(&joined)));
    }
    if material.is_none() { text.push_str(&format!(" Its {} {} {}.", skin, if skin.ends_with('s') && !skin.ends_with("ss") { "are" } else { "is" }, colour)); }
    if let Some(g) = &glow { text.push_str(&format!(" Its eyes glow {}.", g)); }
    if let Some(a) = &attack { text.push_str(&format!(" {}", a.warning)); }
    let kind_word = d.kinds.get(&req.kind).map(|k| k.0.clone()).unwrap_or_else(|| "beast".into());
    Monster { kind_word, profile: p.name, base: p.base, legs, class: p.class, skin, blood, material, tweaks, eyes, glow, colour, attack, flies, size, spheres, short, description: text, hoard: Vec::new(), kills: 0 }
}

/// Spheres a word suggests (an epithet, a power, a land).
pub fn spheres_of_words(text: &str) -> Vec<String> {
    let t = text.to_lowercase();
    let mut out: Vec<String> = Vec::new();
    for (keys, sphere) in [
        (&["storm", "thunder", "lightning", "wind", "tempest"][..], "storm"),
        (&["flame", "fire", "burn", "ember", "ash", "scorch", "cinder", "magma", "smolder", "char", "sun"][..], "fire"),
        (&["frost", "ice", "snow", "winter", "cold", "pale", "bitter", "white"][..], "cold"),
        (&["shadow", "night", "dark", "dusk", "black", "void", "gloom"][..], "darkness"),
        (&["plague", "rot", "blight", "mire", "bog", "murk", "slime", "fen", "pox"][..], "plague"),
        (&["death", "bone", "grave", "doom", "skull", "corpse", "dread", "unending", "eternal"][..], "death"),
        (&["stone", "crag", "peak", "iron", "granite", "ridge", "earth", "mountain", "deep"][..], "earth"),
        (&["sea", "tide", "brine", "wave", "abyss", "coral", "river", "lake"][..], "water"),
        (&["thorn", "moss", "bark", "grove", "wild", "root"][..], "nature"),
        (&["arcane", "ether", "shimmer", "crystal", "phase", "spell", "rune"][..], "magic"),
    ] {
        if keys.iter().any(|k| t.contains(k)) && !out.iter().any(|o| o == sphere) { out.push(sphere.to_string()); }
    }
    out
}

/// The monster a legendary beast of the history is: its species' body, material and powers, its
/// epithet and its kind's land decide the request; its id and name seed the rest.
pub fn of_legend(h: &crate::history::world_state::WorldHistory, c: &crate::history::creatures::legendary::LegendaryCreature) -> Monster {
    use crate::history::creatures::anatomy::{BodyPartType, BodyMaterial, BodyPartSpecial, MagicAbility, CreatureSize};
    let sp = h.creature_species.get(&c.species_id);
    let mut req = Request { kind: "beast".into(), ..Default::default() };
    let mut words = format!("{} {}", c.name, c.epithet);
    if let Some(sp) = sp {
        words.push(' ');
        words.push_str(&sp.name);
        let count = |t: BodyPartType| sp.body_parts.iter().filter(|p| p.part_type == t).map(|p| p.count as u32).sum::<u32>();
        let has = |t: BodyPartType| count(t) > 0;
        if has(BodyPartType::Wings) { req.must.push("wings".into()); }
        if has(BodyPartType::Tentacles) { req.must.push("tentacles".into()); }
        if has(BodyPartType::Horns) { req.must.push("horns".into()); }
        if has(BodyPartType::Mandibles) { req.must.push("mandibles".into()); }
        let legs = count(BodyPartType::Legs);
        req.base = Some(match (legs, has(BodyPartType::Arms), has(BodyPartType::Tentacles), has(BodyPartType::Wings)) {
            (_, _, true, _) if legs == 0 => "blob",
            (0, _, _, _) => "serpent",
            (2, true, _, _) => "humanoid",
            (2, false, _, true) => "bird",
            (6, _, _, _) => "insect",
            (8, _, _, _) => "spider",
            _ => "quadruped",
        }.to_string());
        if has(BodyPartType::Tail) && req.base.as_deref() != Some("serpent") { req.must.push("tail".into()); }
        let mat = sp.body_parts.first().map(|p| p.material);
        req.material = match mat {
            Some(BodyMaterial::Stone) => Some("granite"), Some(BodyMaterial::Metal) => Some("iron"), Some(BodyMaterial::Crystal) => Some("glass"),
            Some(BodyMaterial::Shadow) => Some("smoke"), Some(BodyMaterial::Flame) => Some("flame"), Some(BodyMaterial::Ooze) => Some("mud"),
            Some(BodyMaterial::Bone) => Some("bone"), Some(BodyMaterial::Ice) => Some("ice"), _ => None,
        }.map(String::from);
        let specials: Vec<BodyPartSpecial> = sp.body_parts.iter().flat_map(|p| p.specials.iter().copied()).collect();
        req.attack = specials.iter().find_map(|s| match s {
            BodyPartSpecial::FireBreathing => Some("fire"), BodyPartSpecial::IceBreathing => Some("frost"), BodyPartSpecial::Acidic => Some("acid"),
            BodyPartSpecial::Paralyzing => Some("gaze"), BodyPartSpecial::Venomous => Some("bite"), _ => None,
        }).map(String::from);
        for a in sp.magical_abilities.iter().chain(c.unique_abilities.iter()) {
            let s = match a { MagicAbility::Necromancy | MagicAbility::CurseWeaving => "death", MagicAbility::ElementalControl => "storm", MagicAbility::Illusions | MagicAbility::Spellcasting | MagicAbility::TimeManipulation | MagicAbility::Teleportation => "magic", MagicAbility::MindControl => "darkness", MagicAbility::Shapeshifting => "night", MagicAbility::HealingAura => "nature" };
            if !req.spheres.iter().any(|x| x == s) { req.spheres.push(s.into()); }
        }
        let base = match sp.size { CreatureSize::Tiny => 0.2, CreatureSize::Small => 0.4, CreatureSize::Medium => 0.7, CreatureSize::Large => 1.0, CreatureSize::Huge => 1.8, CreatureSize::Gargantuan => 3.0, CreatureSize::Colossal => 4.0 };
        req.size = base * c.size_multiplier.max(0.3);
    } else {
        req.size = 1.5 * c.size_multiplier.max(0.3);
    }
    for s in spheres_of_words(&words) { if !req.spheres.contains(&s) { req.spheres.push(s); } }
    // The land its kind was made for (the species' first habitat: the same wherever it is asked).
    if let Some(b) = sp.and_then(|sp| sp.habitat.first().copied()) {
        let name = format!("{:?}", b);
        for s in spheres_of_words(&name) { if !req.spheres.contains(&s) { req.spheres.push(s); } }
        let extra = match name.as_str() {
            n if n.contains("Volcanic") || n.contains("Lava") || n.contains("Obsidian") || n.contains("Sulfur") => Some("fire"),
            n if n.contains("Tundra") || n.contains("Ice") || n.contains("Snow") || n.contains("Glacier") => Some("cold"),
            n if n.contains("Swamp") || n.contains("Marsh") || n.contains("Bog") => Some("plague"),
            n if n.contains("Desert") || n.contains("Dune") || n.contains("Savanna") => Some("sun"),
            n if n.contains("Forest") || n.contains("Grove") || n.contains("Jungle") => Some("nature"),
            n if n.contains("Ocean") || n.contains("Coastal") || n.contains("Lagoon") => Some("water"),
            _ => None,
        };
        if let Some(e) = extra { if !req.spheres.iter().any(|x| x == e) { req.spheres.push(e.into()); } }
    }
    req.evil = req.spheres.iter().any(|s| matches!(s.as_str(), "death" | "darkness" | "plague"));
    let seed = crate::persona::seed_of(&c.name, c.id.0 as u64 ^ 0x0B57);
    let mut m = generate(&req, seed);
    m.hoard = c.artifacts_owned.iter().filter_map(|a| h.artifacts.get(a)).filter(|a| !a.destroyed)
        .map(|a| format!("{}, {}", a.name, crate::colony::relic::describe(a))).collect();
    m.kills = c.kills.len();
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monsters_are_pure_and_say_what_they_are() {
        for s in 0..300u64 {
            let req = Request { kind: "forgotten".into(), spheres: vec!["fire".into()], size: 2.0 + (s % 3) as f32, ..Default::default() };
            let a = generate(&req, s);
            let b = generate(&req, s);
            assert_eq!(a.description, b.description);
            assert!(a.attack.is_some(), "a forgotten beast without a special attack: {}", a.description);
            // The words come from the choices: the profile, the warning, the material.
            assert!(a.description.contains(&a.profile), "{}", a.description);
            if let Some(m) = &a.material { assert!(a.description.contains(m.as_str())); }
            assert!(a.description.contains(&a.attack.as_ref().unwrap().warning));
            // A stinger attack only on a body with a stinger.
            if a.attack.as_ref().unwrap().name == "sting" { assert!(a.tweaks.iter().any(|t| t == "stinger"), "{}", a.description); }
        }
        let wings = Request { kind: "beast".into(), must: vec!["wings".into()], size: 1.0, ..Default::default() };
        for s in 0..100u64 { assert!(generate(&wings, s).flies); }
        println!("{}", generate(&Request { kind: "forgotten".into(), spheres: vec!["darkness".into()], size: 3.0, evil: true, ..Default::default() }, 4242).description);
    }
}
