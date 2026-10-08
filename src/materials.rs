//! Materials, weapons, armour and bodies: what a blow is made of and what it meets.
//!
//! The idea is Dwarf Fortress's items and combat (design guide ch. 10): a thing is type x
//! subtype x material; the subtype (spear, axe, mallet, a beast's jaws) gives the attack's
//! geometry (edge or blunt, contact area, how deep its shape can go, its size), the material
//! gives the physics (density, so the weapon's mass and the blow's momentum; hardness, so whether
//! an edge bites; edge; what it costs to cut, to stop, to crack), and capability flags (weapon,
//! armour) say what may be made of what. A body is layers of tissue per part (skin, fat, muscle,
//! bone; a beast's fur, hide, chitin or plates of its substance on top; armour over a settler's
//! body and arms). A blow spends its momentum layer by layer: an edge cuts while its material is
//! hard enough against the layer (else it turns blunt there), a blunt blow is cushioned by soft
//! layers and must crack rigid ones; where it stops says what it did (glances off, turned by
//! the armour, bruises, cuts, bites deep, breaks the bone) and how much of the part it went
//! through is its harm. All numbers live in `data/defaults/materials.json`; pure functions, no
//! randomness here (the caller's hashed rolls scale the momentum).

use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

const JSON: &str = include_str!("../data/defaults/materials.json");

/// A material's physics.
#[derive(Clone, Debug, Deserialize)]
pub struct Material {
    /// g/cm3.
    pub density: f32,
    /// Shear yield, relative: an edge bites a layer only when `bite` x as hard.
    pub hard: f32,
    /// How keen an edge it takes, 0-1.
    pub edge: f32,
    /// What each mm costs an edge to cut through.
    pub resist: f32,
    /// What each mm takes from a blunt blow passing through.
    pub absorb: f32,
    /// What each mm of a rigid layer takes to crack.
    pub fracture: f32,
    pub rigid: bool,
    #[serde(default)]
    pub flags: Vec<String>,
}

impl Material {
    /// DF's capability flags: "weapon", "armour".
    pub fn can(&self, flag: &str) -> bool { self.flags.iter().any(|f| f == flag) }
}

/// A weapon subtype: the attack's geometry and the weapon's size.
#[derive(Clone, Debug, Deserialize)]
pub struct WeaponDef {
    pub attack: String,
    pub contact: f32,
    /// mm.
    pub penetration: f32,
    /// cm3 of the weapon's material.
    pub head: f32,
    /// cm3 of wood.
    pub haft: f32,
    pub velocity: f32,
}

/// A beast's own weapon: its geometry, what it is of, and its momentum per unit of size.
#[derive(Clone, Debug, Deserialize)]
pub struct NaturalDef {
    pub attack: String,
    pub contact: f32,
    pub penetration: f32,
    pub material: String,
    pub force: f32,
}

/// An armour subtype: its thickness and the parts it covers.
#[derive(Clone, Debug, Deserialize)]
pub struct ArmourDef {
    pub thick: f32,
    pub covers: Vec<String>,
}

struct Data {
    momentum: f32,
    harm: f32,
    bite: f32,
    bone_share: f32,
    deep: f32,
    crack: f32,
    size_power: f32,
    spread: f32,
    blunt: f32,
    hardness: f32,
    materials: HashMap<String, Material>,
    aliases: HashMap<String, String>,
    weapons: HashMap<String, WeaponDef>,
    natural: HashMap<String, NaturalDef>,
    armour: HashMap<String, ArmourDef>,
    coverings: HashMap<String, f32>,
    /// body kind -> part -> layers (tissue, mm).
    bodies: HashMap<String, HashMap<String, Vec<(String, f32)>>>,
}

fn table<T: serde::de::DeserializeOwned>(v: &Value) -> HashMap<String, T> {
    v.as_object().map(|o| o.iter().filter(|(k, _)| !k.starts_with('_'))
        .map(|(k, x)| (k.clone(), serde_json::from_value(x.clone()).unwrap_or_else(|e| panic!("materials.json {}: {}", k, e)))).collect()).unwrap_or_default()
}

fn load() -> Data {
    let v: Value = serde_json::from_str(JSON).expect("materials.json");
    let f = |k: &str| v[k].as_f64().unwrap_or(1.0) as f32;
    let bodies = v["bodies"].as_object().unwrap().iter().filter(|(k, _)| !k.starts_with('_')).map(|(kind, parts)| {
        (kind.clone(), parts.as_object().unwrap().iter().map(|(p, layers)| {
            (p.clone(), layers.as_array().unwrap().iter().filter_map(|l| Some((l.get(0)?.as_str()?.to_string(), l.get(1)?.as_f64()? as f32))).collect())
        }).collect())
    }).collect();
    Data {
        momentum: f("momentum"), harm: f("harm"), bite: f("bite"), bone_share: f("bone_share"), deep: f("deep"), crack: f("crack"), size_power: f("size_power"), spread: f("spread"), blunt: f("blunt"), hardness: f("hardness"),
        materials: table(&v["materials"]), aliases: table(&v["aliases"]), weapons: table(&v["weapons"]), natural: table(&v["natural"]),
        armour: table(&v["armour"]), coverings: table(&v["coverings"]), bodies,
    }
}

fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(load)
}

/// A material by name ("oak" is wood), if known.
pub fn known(name: &str) -> Option<&'static Material> {
    let d = data();
    let n = d.aliases.get(name).map(|s| s.as_str()).unwrap_or(name);
    d.materials.get(n)
}

/// A material by name; anything unknown is stone.
pub fn material(name: &str) -> &'static Material { known(name).unwrap_or_else(|| &data().materials["stone"]) }

pub fn weapon(name: &str) -> &'static WeaponDef { data().weapons.get(name).unwrap_or_else(|| &data().weapons["spear"]) }
pub fn natural(name: &str) -> &'static NaturalDef { data().natural.get(name).unwrap_or_else(|| &data().natural["jaws"]) }
pub fn armour(name: &str) -> &'static ArmourDef { data().armour.get(name).unwrap_or_else(|| &data().armour["jerkin"]) }

/// What a blow did.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Outcome { Glance, Turned, Bruise, Cut, Deep, Broken }

impl Outcome {
    /// A wound's severity on a settler: 0 none, 1 bruised, 2 gashed, 3 broken.
    pub fn severity(self) -> u8 { match self { Outcome::Glance | Outcome::Turned => 0, Outcome::Bruise => 1, Outcome::Cut | Outcome::Deep => 2, Outcome::Broken => 3 } }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { Armour, Cover, Soft, Muscle, Bone }

/// A layer of a body part.
#[derive(Clone, Debug)]
pub struct Layer { pub mat: &'static Material, pub name: String, pub thick: f32, pub kind: Kind }

/// A blow about to land.
#[derive(Clone, Debug)]
pub struct Blow { pub momentum: f32, pub edge: bool, pub contact: f32, pub penetration: f32, pub mat: &'static Material }

/// Where a blow stopped and what it did.
#[derive(Clone, Debug)]
pub struct Struck {
    pub outcome: Outcome,
    /// How badly the part is hurt: `harm` x the share of the part's resistance the blow went
    /// through (1.2 at most).
    pub harm: f32,
    /// The layer it stopped in (its material's name: "granite", "the mail shirt"'s "iron").
    pub stopped: String,
}

/// A weapon's blow: a subtype of a material in a hand of `strength` (attribute/1000, times skill
/// and the maker's hand).
pub fn weapon_blow(subtype: &str, mat: &str, strength: f32) -> Blow {
    let w = weapon(subtype);
    let m = material(mat);
    let kg = (w.head * m.density + w.haft * material("wood").density) / 1000.0;
    Blow { momentum: data().momentum * strength * w.velocity * kg.max(0.05).sqrt(), edge: w.attack == "edge", contact: w.contact, penetration: w.penetration, mat: m }
}

/// A beast's blow with its own weapon: of its substance when that is weapon-worthy (an iron
/// beast's jaws are iron), else of tooth, horn, claw...
pub fn natural_blow(kind: &str, substance: Option<&str>, size: f32) -> Blow {
    let n = natural(kind);
    let mat = substance.and_then(known).filter(|m| m.can("weapon")).unwrap_or_else(|| material(&n.material));
    Blow { momentum: n.force * size.powf(data().size_power), edge: n.attack == "edge", contact: n.contact, penetration: n.penetration, mat }
}

/// "left arm" -> "arm".
fn part_key(part: &str) -> &str { part.trim_start_matches("left ").trim_start_matches("right ") }

/// The layers of a part. `kind` "settler", "raider" or "beast"; `scale` the body's (a settler's
/// toughness, a beast's size); `cover` a beast's covering word or substance ("fur", "granite"),
/// `plated` when it is a substance; `worn` armour (material, subtype, quality) over the parts it covers.
pub fn body(kind: &str, part: &str, scale: f32, cover: Option<(&str, bool)>, worn: Option<(&str, &str, f32)>) -> Vec<Layer> {
    let d = data();
    let key = part_key(part);
    let parts = &d.bodies[kind];
    let layers = parts.get(key).or_else(|| parts.values().next()).unwrap();
    let mut out = Vec::new();
    if let Some((mat, sub, q)) = worn {
        let a = armour(sub);
        if a.covers.iter().any(|c| c == key) { out.push(Layer { mat: material(mat), name: mat.to_string(), thick: a.thick * q, kind: Kind::Armour }); }
    }
    let (cname, plated) = cover.unwrap_or(("skin", false));
    for (t, mm) in layers {
        let (name, thick, k) = match t.as_str() {
            "$cover" => (cname.to_string(), mm * if plated { d.coverings["plates"] } else { d.coverings.get(cname).copied().unwrap_or(3.0) } * scale, Kind::Cover),
            "$shell" => if plated { (cname.to_string(), mm * d.coverings["plates"] * scale, Kind::Cover) } else { ("chitin".to_string(), mm * d.coverings["chitin"] * scale, Kind::Cover) },
            "$horn" => (if plated && known(cname).map_or(false, |m| m.rigid && m.can("weapon")) { cname.to_string() } else { "horn".to_string() }, mm * scale, Kind::Cover),
            "bone" => ("bone".to_string(), mm * scale, Kind::Bone),
            "muscle" => ("muscle".to_string(), mm * scale, Kind::Muscle),
            other => (other.to_string(), mm * scale, Kind::Soft),
        };
        let mat = known(&name).unwrap_or_else(|| material("skin"));
        out.push(Layer { mat, name, thick, kind: k });
    }
    out
}

/// What an edge pays to cut a layer: its resistance, the edge's contact and keenness, and how
/// much harder the weapon is than the layer.
fn edge_cost(b: &Blow, l: &Layer) -> f32 {
    let keen = 1.0 / (0.5 + 0.5 * b.mat.edge);
    let ratio = (l.mat.hard.max(0.01) / b.mat.hard.max(0.01)).powf(data().hardness);
    l.mat.resist * l.thick * b.contact * keen * ratio
}

fn bites(b: &Blow, l: &Layer) -> bool { b.mat.hard >= data().bite * l.mat.hard }

/// What a blunt blow pays to pass a layer (or to crack it, if rigid).
fn blunt_cost(l: &Layer) -> f32 { if l.mat.rigid { l.mat.fracture * l.thick } else { l.mat.absorb * l.thick } }

/// The blow against the layers, outermost first.
pub fn strike(b: &Blow, layers: &[Layer]) -> Struck {
    let d = data();
    // The part's resistance to this blow, for the harm (to an edge the bone counts a share: a
    // cut through the flesh to the bone is a grave one).
    let r_body: f32 = {
        let mut e = b.edge;
        layers.iter().filter(|l| l.kind != Kind::Armour).map(|l| {
            if l.kind == Kind::Bone && b.edge { return edge_cost(b, l) * d.bone_share; }
            if e && !bites(b, l) { e = false; }
            if e { edge_cost(b, l) } else { blunt_cost(l) }
        }).sum::<f32>().max(1.0)
    };
    let scale = d.harm * if b.edge { 1.0 } else { d.blunt };
    let done = |o: Outcome, spent: f32, l: &Layer| Struck { outcome: o, harm: if matches!(o, Outcome::Glance | Outcome::Turned) { 0.0 } else { scale * (spent / r_body).min(1.2) }, stopped: l.name.clone() };
    let mut edge = b.edge;
    let mut m = b.momentum;
    let mut spent = 0.0f32;
    let mut depth = 0.0f32;
    // A blow spread by armour must be the harder to crack the bone under it.
    let mut spread = 1.0f32;
    // Whether an edge has cut into the flesh (an edge turned by armour has not).
    let mut cut = false;
    let first_body = layers.iter().position(|l| l.kind != Kind::Armour).unwrap_or(0);
    for (i, l) in layers.iter().enumerate() {
        let body = l.kind != Kind::Armour;
        // The shape can go no deeper: the rest of the blow lands blunt.
        if edge && body && depth >= b.penetration { edge = false; }
        // An edge that has cut through the flesh to a bone it cannot bite has bitten deep; what
        // is left of it may crack the bone.
        if l.kind == Kind::Bone && edge && !bites(b, l) {
            let c = d.crack * spread * l.mat.fracture * l.thick;
            spent += m.min(c) * d.bone_share;
            return done(if m > c { Outcome::Broken } else { Outcome::Deep }, spent, l);
        }
        // Too soft to bite this layer: the edge turns, and the blow lands blunt.
        if edge && !bites(b, l) { edge = false; }
        if edge {
            let c = edge_cost(b, l);
            if m < c {
                if body { spent += m * if l.kind == Kind::Bone { d.bone_share } else { 1.0 }; }
                let frac = m / c;
                let o = match l.kind {
                    Kind::Armour => Outcome::Turned,
                    // A glance off the covering hurts nothing.
                    Kind::Cover => if l.mat.rigid && i == first_body { Outcome::Glance } else { Outcome::Bruise },
                    Kind::Soft => Outcome::Bruise,
                    Kind::Muscle => if frac >= d.deep { Outcome::Deep } else { Outcome::Cut },
                    // Stopped in the bone: it may yet crack under what is left.
                    Kind::Bone => if m > d.crack * spread * l.mat.fracture * l.thick { Outcome::Broken } else { Outcome::Deep },
                };
                return done(o, spent, l);
            }
            m -= c;
            if body { spent += c * if l.kind == Kind::Bone { d.bone_share } else { 1.0 }; depth += l.thick; cut = true; }
            if l.kind == Kind::Bone { return done(Outcome::Broken, spent, l); }
        } else {
            let c = blunt_cost(l) * if l.kind == Kind::Bone { spread } else { 1.0 };
            if l.kind == Kind::Bone {
                // (An edge turned before the bone counts its share as an edge would.)
                spent += m.min(c) * if b.edge { d.bone_share } else { 1.0 };
                let o = if m > c { Outcome::Broken } else if cut && layers[i - 1].kind == Kind::Muscle { Outcome::Deep } else { Outcome::Bruise };
                return done(o, spent, l);
            }
            if m <= c {
                let o = match l.kind { Kind::Armour => Outcome::Turned, Kind::Cover if l.mat.rigid && i == first_body => Outcome::Glance, _ => Outcome::Bruise };
                if body && o != Outcome::Glance { spent += m; }
                return done(o, spent, l);
            }
            m -= c;
            if body { spent += c; depth += l.thick; } else { spread = d.spread; }
        }
    }
    // Through everything (a tentacle has no bone).
    let last = layers.last().unwrap();
    done(Outcome::Deep, spent, last)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(b: &Blow, kind: &str, part: &str, scale: f32, cover: Option<(&str, bool)>, worn: Option<(&str, &str, f32)>) -> Struck { strike(b, &body(kind, part, scale, cover, worn)) }

    /// The table loads, and the materials and shapes tell blows apart (`cargo test materials --
    /// --nocapture` prints the table of what each weapon does to each body).
    #[test]
    fn materials_tell_blows_apart() {
        assert!(known("oak").is_some() && material("flint").can("weapon") && !material("fur").can("weapon") && material("iron").can("armour") && !material("salt").can("armour"));
        let s = 1.2;
        let fur2 = Some(("fur", false));
        // A stone edge cannot bite granite plates or iron: it glances off with a ring.
        assert_eq!(hit(&weapon_blow("spear", "flint", s), "beast", "flank", 2.0, Some(("granite", true)), None).outcome, Outcome::Glance);
        assert_eq!(hit(&weapon_blow("spear", "iron", s), "beast", "flank", 1.5, Some(("iron", true)), None).outcome, Outcome::Glance);
        // Adamantine cuts through granite plates into the flesh.
        assert!(hit(&weapon_blow("spear", "adamantine", s), "beast", "flank", 2.0, Some(("granite", true)), None).outcome >= Outcome::Cut);
        // Harder, keener heads harm a furred beast more; a sharpened stake least.
        let harm = |w: &str, m: &str| hit(&weapon_blow(w, m, s), "beast", "flank", 2.0, fur2, None).harm;
        assert!(harm("spear", "adamantine") > harm("spear", "iron") && harm("spear", "iron") > harm("spear", "flint") && harm("spear", "flint") > harm("stake", "wood"), "adamantine {} iron {} flint {} stake {}", harm("spear", "adamantine"), harm("spear", "iron"), harm("spear", "flint"), harm("stake", "wood"));
        // A spear bites deep into a raider; a mallet bruises.
        assert_eq!(hit(&weapon_blow("spear", "flint", s), "raider", "shoulder", 1.0, None, None).outcome, Outcome::Deep);
        assert_eq!(hit(&weapon_blow("mallet", "wood", s), "raider", "shoulder", 1.0, None, None).outcome, Outcome::Bruise);
        // Armour by layer: iron mail turns a raider's iron sword into a bruise (the edge cannot
        // bite it), leather only lightens it; nothing covers the legs.
        let sword = weapon_blow("sword", "iron", 1.0);
        assert!(hit(&sword, "settler", "body", 1.0, None, None).outcome.severity() == 2);
        assert!(hit(&sword, "settler", "body", 1.0, None, Some(("iron", "mail shirt", 1.0))).outcome.severity() <= 1);
        assert!(hit(&sword, "settler", "body", 1.0, None, Some(("leather", "jerkin", 1.0))).harm < hit(&sword, "settler", "body", 1.0, None, None).harm);
        assert_eq!(hit(&sword, "settler", "leg", 1.0, None, Some(("adamantine", "mail shirt", 1.0))).outcome, hit(&sword, "settler", "leg", 1.0, None, None).outcome);
        assert_eq!(hit(&sword, "settler", "body", 1.0, None, Some(("adamantine", "mail shirt", 1.0))).outcome, Outcome::Turned);
        // A big beast breaks bones; a small one gashes.
        assert_eq!(hit(&natural_blow("tusks", None, 2.5), "settler", "arm", 1.0, None, None).outcome, Outcome::Broken);
        assert_eq!(hit(&natural_blow("jaws", None, 1.0), "settler", "arm", 1.0, None, None).outcome.severity(), 2);
        // An iron beast bites with iron.
        assert!(std::ptr::eq(natural_blow("jaws", Some("iron"), 2.0).mat, material("iron")) && std::ptr::eq(natural_blow("jaws", Some("smoke"), 2.0).mat, material("tooth")));
        // The whole table, for tuning.
        for (w, m) in [("spear", "flint"), ("spear", "sandstone"), ("spear", "bone"), ("spear", "copper"), ("spear", "iron"), ("spear", "adamantine"), ("stake", "wood"), ("axe", "flint"), ("axe", "iron"), ("mallet", "wood"), ("hammer", "iron")] {
            let b = weapon_blow(w, m, s);
            for (kind, cov, size) in [("raider", None, 1.0), ("beast", fur2, 1.0), ("beast", fur2, 2.0), ("beast", Some(("chitin", false)), 2.0), ("beast", Some(("granite", true)), 2.0)] {
                let parts: &[&str] = if kind == "raider" { &["arm", "shoulder", "leg"] } else { &["head", "flank", "leg", "horn", "shell"] };
                let row: Vec<String> = parts.iter().map(|p| { let r = hit(&b, kind, p, size, cov, None); format!("{}:{:?}/{:.2}", p, r.outcome, r.harm) }).collect();
                println!("{} {} (M {:.0}) v {} {} {}: {}", m, w, b.momentum, kind, cov.map_or("", |c| c.0), size, row.join(" "));
            }
        }
    }
}
