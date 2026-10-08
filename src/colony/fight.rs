//! Blows and wounds: the raid's clash played between bodies.
//!
//! The idea is Dwarf Fortress's combat report: each blow is a body against a body part, its
//! chance from agility against the target's size, and what it does from the weapon and the
//! layers it meets (`materials.rs`, `data/defaults/materials.json`; DF's attack geometry on the
//! item and physics on the material): a blow's momentum is strength x the drill x the weapon's
//! velocity x the square root of its mass (a head of its material on a haft of wood); an edge
//! cuts layer by layer while its material is well harder than the layer (else it lands blunt
//! there), a blunt blow is cushioned by flesh and must crack what is rigid; where it stops is
//! what the log says (glances off the granite of its flank with a ring, bruises, cuts, bites
//! deep, breaks the bone) and how much of the part it went through is its harm. A beast strikes
//! with its own weapons (jaws, tusks, horns, mandibles, talons, tentacles; of its substance when
//! that is weapon-worthy: an iron beast's jaws are iron) at a momentum of its size, raiders with
//! their people's iron (a club of wood). The raid's outcome is still the patron's game
//! (readiness against danger, `arc::raid_at`); the fight is how it happened, told blow by blow
//! from the settlers' personas and the monster's body (`monsters.rs`), and its wounds are real: a
//! broken arm makes felling and building slow, a broken leg makes walking slow, every wound
//! hurts, and they heal at the pace of the wounded (`Persona::healing`).

use super::*;
use crate::materials::{self, Blow, Layer, Outcome};
use crate::persona::Attr;

/// A wound on a body part.
#[derive(Clone, Debug)]
pub struct Wound {
    /// "left arm", "head", "right leg", "body".
    pub part: String,
    /// 1 bruised, 2 cut, 3 broken.
    pub severity: u8,
    pub healed_at: u64,
    /// What did it, where: "Gru's tusk, the night of day 30".
    pub from: String,
    /// The last day it was tended, whether it festers, and since when (`heal.rs`).
    pub tended: u64,
    pub infected: bool,
    pub fever_since: u64,
}

impl Wound {
    /// "a broken left arm", "cracked ribs", "a gashed side".
    pub fn word(&self) -> String {
        match (self.part.as_str(), self.severity) {
            ("body", 3) => "cracked ribs".into(),
            ("body", 2) => "a gashed side".into(),
            ("body", _) => "bruised ribs".into(),
            ("head", 3) => "a cracked skull".into(),
            (p, 3) => format!("a broken {}", p),
            (p, 2) => format!("a gashed {}", p),
            (p, _) => format!("a bruised {}", p),
        }
    }
    pub fn arm(&self) -> bool { self.part.ends_with("arm") }
    pub fn leg(&self) -> bool { self.part.ends_with("leg") }
}

pub(crate) const PARTS: [(&str, u32); 6] = [("head", 2), ("body", 4), ("left arm", 2), ("right arm", 2), ("left leg", 2), ("right leg", 2)];

/// What a settler fights with: the words, the weapon's subtype and material, and the maker's hand.
pub(crate) struct Arms { pub words: String, pub subtype: &'static str, pub material: String, pub quality: f32 }

/// The weapon a settler fights with: a spear of the militia's (`militia.rs`), else what they work with.
fn weapon(c: &Colony, i: usize) -> Arms {
    if let Some(a) = c.arm_of(i) { return Arms { words: a.kind.clone(), subtype: "spear", material: a.material.clone(), quality: a.quality }; }
    tool(c, i)
}

/// What a settler works with: iron once ore is worked, else the land's stone and wood.
fn tool(c: &Colony, i: usize) -> Arms {
    let iron = c.iron_worked();
    let (words, subtype, material) = match c.settlers[i].role {
        Some(2) => if iron { ("an iron axe", "axe", "iron".to_string()) } else { ("an axe", "axe", c.land_stone()) },
        Some(4) => if iron { ("an iron hammer", "hammer", "iron".to_string()) } else { ("a mallet", "mallet", "wood".to_string()) },
        Some(1) => ("a fishing spear", "fishing spear", "bone".to_string()),
        _ => if iron { ("an iron-headed spear", "spear", "iron".to_string()) } else { ("a sharpened stake", "stake", "wood".to_string()) },
    };
    Arms { words: words.to_string(), subtype, material, quality: 1.0 }
}

/// What the foe strikes with: a beast's own body, a raider's arms.
pub(crate) struct Foe {
    /// "its tusks", "a spear".
    pub words: String,
    /// A natural weapon ("tusks") or a weapon subtype ("spear"; `materials.json`).
    key: &'static str,
    natural: bool,
    /// What it strikes with: the beast's substance, or the raider's weapon's material.
    material: Option<String>,
    size: f32,
}

impl Foe {
    /// Its blow, at `luck` x its momentum.
    pub(crate) fn blow(&self, luck: f32) -> Blow {
        let mut b = if self.natural { materials::natural_blow(self.key, self.material.as_deref(), self.size) }
            else { materials::weapon_blow(self.key, self.material.as_deref().unwrap_or("iron"), 1.0) };
        b.momentum *= luck;
        b
    }
}

/// What `foe` strikes with: a beast's tusks, horns, mandibles, tentacles, talons or jaws; raiders
/// carry their people's arms (the same weapon for the same people, by the name).
pub(crate) fn foe_of(foe: &str, monster: Option<&crate::monsters::Monster>) -> Foe {
    if let Some(m) = monster {
        let has = |t: &str| m.tweaks.iter().any(|x| x == t);
        let key = if has("tusks") { "tusks" } else if has("horns") { "horns" } else if has("mandibles") { "mandibles" } else if has("tentacles") { "tentacles" } else if m.base == "bird" { "talons" } else { "jaws" };
        return Foe { words: format!("its {}", key), key, natural: true, material: m.material.clone(), size: m.size.max(0.6) };
    }
    let (words, key) = [("a spear", "spear"), ("an axe", "axe"), ("a sword", "sword"), ("a mace", "mace"), ("a club", "club"), ("a long knife", "long knife")][(crate::persona::seed_of(foe.split(", led by ").next().unwrap_or(foe), 0xA4E5) % 6) as usize];
    Foe { words: words.to_string(), key, natural: false, material: Some(if key == "club" { "wood" } else { "iron" }.to_string()), size: 1.0 }
}

/// The parts of the foe a blow may land on.
fn foe_parts(monster: Option<&crate::monsters::Monster>) -> Vec<String> {
    let mut v = if monster.is_some() { vec!["head".to_string(), "flank".to_string(), "leg".to_string()] } else { vec!["arm".to_string(), "shoulder".to_string(), "leg".to_string()] };
    if let Some(m) = monster { for t in &m.tweaks { match t.as_str() { "wings" => v.push("wing".into()), "tail" => v.push("tail".into()), "tentacles" => v.push("tentacle".into()), "shell" => v.push("shell".into()), "horns" => v.push("horn".into()), _ => {} } } }
    v
}

/// The layers of the foe's `part`: a beast's covering (or plates of its substance) over its
/// flesh and bone, at its size; a raider's flesh.
fn foe_layers(monster: Option<&crate::monsters::Monster>, part: &str) -> Vec<Layer> {
    match monster {
        Some(m) => {
            let cover = m.material.as_deref().map(|x| (x, true)).unwrap_or((m.skin.as_str(), false));
            materials::body("beast", part, m.size.max(0.6), Some(cover), None)
        }
        None => materials::body("raider", part, 1.0, None, None),
    }
}

impl Colony {
    fn roll(&self, salt: u64) -> f32 { (crate::history::settlers::hash_pub(self.seed ^ self.clock.tick, salt) % 10_000) as f32 / 10_000.0 }

    /// The layers of settler `i`'s `part`: skin, fat, muscle and bone by their toughness, under
    /// their armour if `armoured` and it covers the part.
    pub(crate) fn body_of(&self, i: usize, part: &str, armoured: bool) -> Vec<Layer> {
        let tough = (self.settlers[i].persona.attr(Attr::Toughness) / 1000.0).clamp(0.6, 1.6);
        let worn = if armoured { self.armour_of(i).map(|a| (a.material.as_str(), a.piece.as_str(), a.quality)) } else { None };
        materials::body("settler", part, tough, None, worn)
    }

    /// Settler `i`'s blow with `arms`: strength x the drill (x(1 + 0.5 x skill)) x the maker's hand x `luck`.
    fn settler_blow(&self, i: usize, arms: &Arms, luck: f32) -> Blow {
        let strength = self.settlers[i].persona.attr(Attr::Strength) / 1000.0 * (1.0 + 0.5 * self.fight_skill(i)) * arms.quality * luck;
        materials::weapon_blow(arms.subtype, &arms.material, strength)
    }

    /// The harm settler `i`'s blow does a beast on the mean, over its parts (an expedition's reckoning).
    pub(crate) fn blow_harm(&self, i: usize, monster: Option<&crate::monsters::Monster>) -> f32 {
        let arms = weapon(self, i);
        let blow = self.settler_blow(i, &arms, 1.0);
        let parts = foe_parts(monster);
        parts.iter().map(|p| materials::strike(&blow, &foe_layers(monster, p)).harm).sum::<f32>() / parts.len().max(1) as f32
    }

    /// Give settler `i` a wound on `part` of `severity` from `from`; it heals at their pace.
    pub(crate) fn wound(&mut self, i: usize, part: &str, severity: u8, from: String) {
        let days = match severity { 3 => 24.0, 2 => 8.0, _ => 3.0 } / self.settlers[i].persona.healing();
        let w = Wound { part: part.to_string(), severity, healed_at: self.clock.tick + (days * TICKS_PER_DAY as f32) as u64, from, tended: self.clock.day(), infected: false, fever_since: 0 };
        let word = w.word();
        self.settlers[i].wounds.push(w);
        self.feel(i, mind::Feel::Wounded { what: word });
    }

    /// Wounds still open on settler `i`.
    pub fn open_wounds(&self, i: usize) -> impl Iterator<Item = &fight::Wound> {
        let now = self.clock.tick;
        self.settlers[i].wounds.iter().filter(move |w| w.healed_at > now)
    }

    /// How much slower work goes for wounds (a broken arm: heavy work 1.6x).
    pub(crate) fn wound_work(&self, i: usize, heavy: bool) -> f32 {
        let mut f: f32 = 1.0;
        for w in self.open_wounds(i) {
            if w.arm() { f *= if heavy { 1.0 + 0.2 * w.severity as f32 } else { 1.0 + 0.08 * w.severity as f32 }; }
            if w.part == "body" { f *= 1.0 + 0.1 * w.severity as f32; }
        }
        f
    }

    /// Walking for wounds: a broken leg halves the pace.
    pub(crate) fn wound_walk(&self, i: usize) -> f32 {
        self.open_wounds(i).filter(|w| w.leg()).fold(1.0, |f, w| f * (1.0 - 0.16 * w.severity as f32))
    }

    /// The clash told blow by blow, ending as the raid's outcome says: `death` (the victim falls),
    /// `rescue` (struck down, dragged clear by the saviour) or a rout (the defenders drive it off).
    /// Returns the lines (logged by the caller in order).
    pub(crate) fn fight(&mut self, foe: &str, monster: Option<&crate::monsters::Monster>, victim: usize, saviour: Option<usize>, outcome: &str) -> Vec<String> {
        let mut lines = Vec::new();
        let day = self.clock.day();
        // The foe as named before raiders become "one of the raiders" (for vows).
        let named = foe.to_string();
        let size = monster.map_or(1.0, |m| m.size).max(0.6);
        // What the foe strikes with: a beast's own body, a raider's arms.
        let arms_of_foe = foe_of(foe, monster);
        let natural = arms_of_foe.words.clone();
        let natural = natural.as_str();
        let foe_word = if monster.is_some() { foe.to_string() } else { "One of the raiders".to_string() };
        let foe = foe_word.as_str();
        // Whose part a blow lands on.
        let its = if monster.is_some() { "its" } else { "the raider's" };
        let their_parts = foe_parts(monster);
        let vname = self.settlers[victim].name.clone();
        let pick_part = |r: f32| -> &'static str {
            let total: u32 = PARTS.iter().map(|p| p.1).sum();
            let mut x = (r * total as f32) as u32;
            for (p, w) in PARTS { if x < w { return p; } x -= w; }
            "body"
        };
        let the = |t: &str| t.replacen("a ", "the ", 1).replacen("an ", "the ", 1);
        // The first blow: the foe on the nearest, the victim.
        let dodge = self.settlers[victim].persona.attr(Attr::Agility) / 1000.0;
        if outcome != "death" && self.roll(1) < 0.25 * dodge {
            lines.push(format!("{} lunges at {} with {}; {} twists aside.", foe, vname, natural, vname));
        }
        let part = pick_part(self.roll(2));
        // Defenders strike back: the saviour, the watcher, the strongest awake.
        // A monster hunter who came for it stands first (`visitors.rs`).
        let hunters: Vec<usize> = (0..self.settlers.len()).filter(|&d| d != victim && self.settlers[d].alive && self.settlers[d].visitor.as_deref().map_or(false, |v| v.starts_with("a monster hunter") || v.starts_with("a sellsword"))).collect();
        // Then the militia: those who bear spears, best first (`militia.rs`).
        let mut armed: Vec<usize> = self.arms.iter().filter_map(|a| a.holder).collect();
        armed.sort_by(|&a, &b| self.fight_skill(b).total_cmp(&self.fight_skill(a)).then(a.cmp(&b)));
        // Those sworn to see this foe dead stand first (`vow.rs`).
        let sworn: Vec<usize> = (0..self.settlers.len()).filter(|&d| self.settlers[d].alive && self.sworn_against(d, &named)).collect();
        let mut defenders: Vec<usize> = sworn.into_iter().chain(hunters).chain(saviour).chain(self.watcher).chain(armed)
            .filter(|&d| d != victim && self.settlers[d].alive && self.settlers[d].ill_until <= self.clock.tick).collect();
        if defenders.is_empty() {
            if let Some(b) = (0..self.settlers.len()).filter(|&d| d != victim && self.settlers[d].alive).max_by(|&a, &b| self.settlers[a].persona.attr(Attr::Strength).total_cmp(&self.settlers[b].persona.attr(Attr::Strength)).then(b.cmp(&a))) { defenders.push(b); }
        }
        { let mut seen = Vec::new(); defenders.retain(|d| if seen.contains(d) { false } else { seen.push(*d); true }); }
        // None raise a hand against their own people (`raid_at` sets whose these raiders are).
        let theirs = |c: &Colony, d: usize| c.fighting_people.is_some() && c.settlers[d].past.as_ref().and_then(|p| p.people) == c.fighting_people;
        defenders.retain(|&d| !theirs(self, d));
        // In a rout the whole camp turns out: a third defender, the strongest not yet in it.
        if outcome == "rout" {
            if let Some(b) = (0..self.settlers.len()).filter(|&d| d != victim && self.settlers[d].alive && !defenders.contains(&d) && self.settlers[d].ill_until <= self.clock.tick)
                .max_by(|&a, &b| self.settlers[a].persona.attr(Attr::Strength).total_cmp(&self.settlers[b].persona.attr(Attr::Strength)).then(b.cmp(&a))) { defenders.push(b); }
        }
        self.blows = (0.0, None);
        let mut hardest = 0.0f32;
        match outcome {
            // A special attack that killed is already told; else the killing blow, to the head or chest.
            "death" if monster.map_or(true, |m| m.attack.is_none()) => {
                lines.push(format!("{} strikes {} in the {} with {}, and {} goes down.", foe, vname, if self.roll(3) < 0.5 { "head" } else { "chest" }, natural, vname));
            }
            "death" => lines.push(format!("The others come running, too late.")),
            "rescue" => {
                // Struck on a part: the foe's blow against the victim's layers, under their
                // armour (`armour.rs`) and without it, to say what the armour took.
                let blow = arms_of_foe.blow(0.85 + 0.3 * self.roll(4));
                let with = materials::strike(&blow, &self.body_of(victim, part, true)).outcome.severity();
                let bare = materials::strike(&blow, &self.body_of(victim, part, false)).outcome.severity().max(1);
                let worn = self.armour_of(victim).map(|a| a.kind.clone());
                let sev = if worn.is_none() { bare } else { with };
                if sev == 0 {
                    lines.push(format!("{} catches {} with {}, but it does not get through {}.", foe, vname, natural, the(&worn.unwrap_or_default())));
                } else {
                    let w = Wound { part: part.into(), severity: sev, healed_at: 0, from: String::new(), tended: 0, infected: false, fever_since: 0 };
                    let took = worn.filter(|_| sev < bare).map(|t| format!(", though {} took the worst of it", the(&t))).unwrap_or_default();
                    lines.push(format!("{} catches {} with {}: {}{}.", foe, vname, natural, w.word(), took));
                    self.wound(victim, part, sev, format!("{}, the night of day {}", natural.replacen("its ", &format!("{}'s ", foe), 1), day));
                }
            }
            _ => {}
        }
        for (k, &d) in defenders.iter().enumerate().take(3) {
            let arms = weapon(self, d);
            let arm = arms.words.clone();
            let dname = self.settlers[d].name.clone();
            let p = &self.settlers[d].persona;
            let hit = (p.attr(Attr::Agility) / 1000.0 * 0.5 + 0.3 + 0.1 * size + 0.25 * self.fight_skill(d)).min(0.95);
            let target = &their_parts[(self.roll(10 + k as u64) * their_parts.len() as f32) as usize % their_parts.len()];
            if self.roll(20 + k as u64) > hit {
                lines.push(format!("{} swings {} at {} {} and misses.", dname, arm, its, target));
            } else {
                // The blow against the layers of the part it lands on (`materials.rs`).
                let blow = self.settler_blow(d, &arms, 0.85 + 0.3 * self.roll(40 + k as u64));
                let struck = materials::strike(&blow, &foe_layers(monster, target));
                match struck.outcome {
                    Outcome::Glance | Outcome::Turned => {
                        let what = if struck.stopped == *target { format!("{} {}", its, target) } else { format!("the {} of {} {}", struck.stopped, its, target) };
                        lines.push(format!("{}'s {} glances off {} with a ring.", dname, arm.trim_start_matches("an ").trim_start_matches("a "), what));
                    }
                    o => {
                        let what = match o {
                            Outcome::Bruise => "bruises",
                            Outcome::Cut => "cuts",
                            Outcome::Deep => "bites deep into",
                            _ => if matches!(target.as_str(), "head" | "shell" | "horn") { "cracks" } else { "breaks" },
                        };
                        // A thrust or a cut that breaks the bone says so after it.
                        if o == Outcome::Broken && blow.edge {
                            lines.push(format!("{} drives {} into {} {}, and the bone {}.", dname, arm, its, target, if target == "head" { "cracks" } else { "breaks" }));
                        } else {
                            lines.push(format!("{} {} {} {} with {}.", dname, what, its, target, arm));
                        }
                        self.blows.0 += struck.harm;
                        if struck.harm > hardest { hardest = struck.harm; self.blows.1 = Some(d); }
                    }
                }
            }
            // The foe answers the bold: a bruise for a defender who stood close, unless their
            // armour turns the glancing blow.
            // (Those behind the first are caught less often.)
            if outcome != "death" && self.roll(30 + 2 * k as u64) < if k == 0 { 0.3 } else { 0.15 } {
                let dp = pick_part(self.roll(31 + 2 * k as u64));
                let blow = arms_of_foe.blow(0.5 * (0.85 + 0.3 * self.roll(50 + k as u64)));
                let turned = self.armour_of(d).filter(|_| materials::strike(&blow, &self.body_of(d, dp, true)).outcome.severity() == 0).map(|a| a.kind.clone());
                if let Some(t) = turned {
                    lines.push(format!("{} catches {} in the {} with {}, but {} turns it.", foe, dname, if dp == "body" { "ribs" } else { dp }, natural, the(&t)));
                } else {
                    lines.push(format!("{} catches {} in the {} with {}: a bruise to remember.", foe, dname, if dp == "body" { "ribs" } else { dp }, natural));
                    self.wound(d, dp, 1, format!("{}, the night of day {}", natural.replacen("its ", &format!("{}'s ", foe), 1), day));
                }
            }
        }
        // The rest of the camp crowds in with what they have: half blows, by expectation.
        if monster.is_some() {
            let before = self.blows.0;
            for d in 0..self.settlers.len() {
                if d == victim || defenders.iter().take(3).any(|&x| x == d) || !self.settlers[d].alive || self.settlers[d].ill_until > self.clock.tick || theirs(self, d) { continue; }
                if self.settlers[d].past.as_ref().map_or(false, |p| p.age < 14) { continue; }
                let arms = weapon(self, d);
                let p = &self.settlers[d].persona;
                let hit = (p.attr(Attr::Agility) / 1000.0 * 0.5 + 0.3 + 0.1 * size + 0.25 * self.fight_skill(d)).min(0.95);
                let target = &their_parts[(self.roll(60 + d as u64) * their_parts.len() as f32) as usize % their_parts.len()];
                let struck = materials::strike(&self.settler_blow(d, &arms, 1.0), &foe_layers(monster, target));
                self.blows.0 += 0.5 * hit * struck.harm;
            }
            if self.blows.0 > before + 0.3 { lines.push(format!("The rest of the camp crowds in with {} and torches.", if self.iron_worked() { "iron and stakes" } else { "axes, stakes" })); }
        }
        match outcome {
            "rescue" => if let Some(s) = saviour {
                let sally = self.siege.as_ref().map_or(false, |x| x.sally);
                lines.push(format!("{} drags {} back {}.", self.settlers[s].name, vname, if sally { "through the gate" } else { "to the fire" }));
            },
            "rout" => lines.push(format!("{} gives ground before the fire and the blades, and is gone into the dark.", foe)),
            _ => {}
        }
        lines
    }

    /// After the clash: a beast the camp's blows hurt past its size (x`need`: 1 after a rout, 1.3
    /// after a rescue, 1.6 after it killed) is slain. Its death is a moment and a stone; the one who struck hardest is
    /// its slayer; a werebeast's curse no longer comes under the moon; and if its lair is within
    /// two tiles, two go and bring home its hoard (`hoard_home`).
    pub(crate) fn maybe_slay(&mut self, threat: &arc::Threat, need: f32, at: Pos) -> bool {
        let Some(m) = threat.monster.as_ref() else { return false };
        let (harm, striker) = self.blows;
        // (Debug: PLANET_DEBUG_BLOWS prints the camp's harm against what the beast needs.)
        if std::env::var("PLANET_DEBUG_BLOWS").is_ok() { eprintln!("blows day {}: {} harm {:.2} need {:.2} (size {:.2}, {})", self.clock.day(), threat.name, harm, need * m.size.max(0.6), m.size, m.material.clone().unwrap_or_else(|| m.skin.clone())); }
        let Some(k) = striker else { return false };
        if harm < need * m.size.max(0.6) { return false; }
        let name = threat.name.clone();
        let day = self.clock.day();
        let kname = self.settlers[k].name.clone();
        let arm = weapon(self, k).words;
        let part = if m.tweaks.iter().any(|t| t == "shell") { "the soft place under its shell" } else if m.flies { "its throat as it rises" } else { "its heart" };
        let killed = if m.kills > 0 { format!(", that had killed {} in the world's long history", m.kills) } else { String::new() };
        let line = format!("{} drives {} into {}: {}{} falls, and does not rise.", kname, arm, part, name, killed);
        self.note(line.clone());
        self.moment(format!("The death of {}", name), line, format!("because the camp was ready for it and {}'s blow went home", kname), at);
        self.marks.push(ColonyMark { at, kind: MarkKind::Stone, title: format!("The bones of {}", name),
            text: format!("Here {} fell to {}'s {} on the night of day {}.", name, kname, arm.trim_start_matches("an ").trim_start_matches("a "), day), day });
        self.settlers[k].deeds.push(format!("slew {} on day {}", name, day));
        self.feel(k, mind::Feel::Slew { what: name.clone() });
        for j in 0..self.settlers.len() { if j != k && self.settlers[j].alive { self.feel(j, mind::Feel::SawFall { what: name.clone() }); } }
        self.slain.push(name.clone());
        self.vow_kept(&name, Some(k));
        // What is left of it: bones and hide for the workshop (DF: a beast's remains are
        // materials like any other, and famous).
        let short = name.split_whitespace().next().unwrap_or(&name).trim_end_matches(',').to_string();
        let bones = (4.0 + 3.0 * m.size).round() as u32;
        let hides = (1.0 + m.size).round() as u32;
        let skin = m.material.clone().map(|x| format!("{} plates", x)).unwrap_or_else(|| if m.skin == "skin" { "hide".to_string() } else { m.skin.clone() });
        self.remains.push((name.clone(), short, bones, hides, skin));
        if self.were.as_deref() == Some(name.as_str()) {
            self.were = None;
            self.note(format!("With {} dead, no beast will howl under the next full moon{}.", name, if self.cursed.is_empty() { "" } else { "; but the curse in the blood of the bitten does not die with it" }));
        }
        // Those still to come that were this beast are no longer coming.
        if let Some(a) = self.arc.as_mut() { a.later.retain(|t| t.name != name); a.reserve.retain(|t| t.name != name); }
        // Its hoard.
        if !m.hoard.is_empty() {
            let here = (self.map.world_tile.0 as i64, self.map.world_tile.1 as i64);
            let lair = threat.from.map(|l| ((l.0 as i64 - here.0).abs() + (l.1 as i64 - here.1).abs()) as u64);
            match lair {
                Some(d) if d <= 2 => {
                    let back = day + 2 + 2 * d;
                    self.hoard_due = Some((back, name.clone(), m.hoard.clone()));
                    self.note(format!("{} and the bravest set out for the lair of {}, {} days' walk, for what it hoarded.", kname, name, (1 + d).max(1)));
                }
                _ => self.note(format!("Its hoard lies in its lair, too far to fetch: {}.", crate::persona::list(&m.hoard))),
            }
        }
        true
    }

    /// The hoard comes home: each thing in it becomes the camp's, kept and admired.
    pub(crate) fn hoard_home(&mut self) {
        let Some((day, beast, hoard)) = self.hoard_due.clone() else { return };
        if self.clock.day() < day || self.clock.hour() != 16 || self.clock.minute() != 0 { return; }
        self.hoard_due = None;
        let line = format!("They come back from the lair of {} with its hoard: {}.", beast, crate::persona::list(&hoard));
        self.note(line.clone());
        let at = self.camp;
        self.moment(format!("The hoard of {}", beast), line, format!("because {} was slain and its lair was near", beast), at);
        for h in &hoard {
            self.treasures.push(format!("{}, from the hoard of {}", h, beast));
            let called = h.split(", ").next().unwrap_or(h).to_string();
            for j in 0..self.settlers.len() { if self.settlers[j].alive { self.feel(j, mind::Feel::Admired { what: called.clone() }); } }
        }
    }
}
