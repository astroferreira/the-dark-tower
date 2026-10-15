//! A small bestiary with causes (card 'A small bestiary with causes: who they are, why here, what
//! they want'). The creatures a camp can meet are drawn from what the world already holds, each
//! with one want, one fear and one sign, and the cause that puts it here:
//! - the named beast near its lair (the arc's first threat when it is a beast): wants its hoard
//!   and its ground; fears a closed wall and many spears; its sign is the chronicle's kills;
//! - the pack: the tile's commonest predator (`LocalMap::predators`, from its ecology); wants
//!   meat; fears fire (wolves turn away at the edge of the firelight); its sign is tracks and a
//!   den. Only the pack prowls by day (`prowl.rs`): one pack, not three kinds;
//! - the nameless dead of a battle fought on the tile (graves of soldiers whose names are lost,
//!   or old bones on the ground): want a name and a burial, and rest when given one
//!   (`reckon_nameless`, `bury_the_nameless`); fear nothing; their sign is a cold light;
//! - the Shadow's things where its darkness lies on the tile (only with the Shadow let into the
//!   camp): want to spread; cannot cross blessed ground; the sign is the blight;
//! - what lives in a cavern once the stair reaches it (the cavern's own life list), and what
//!   sleeps in the deepest (the old songs speak of it before).
//! At founding at most one named monster and one pack are on the land: no site starts with more
//! than two kinds. `Colony::bestiary` lists them for the ledger's Camp tab, the hover chip and
//! `--sim-projects` ("Bestiary: ...").

use super::*;
use crate::local::wildlife::Feature;

/// "wolf" -> "wolves", "bear" -> "bears".
fn plural(n: &str) -> String {
    if n.ends_with('f') { format!("{}ves", &n[..n.len() - 1]) } else if n.ends_with('s') { n.to_string() } else { format!("{}s", n) }
}

/// One kind of creature on the embark, with why it is here and how to read it.
#[derive(Clone, Debug)]
pub struct Kind {
    /// What it is ("wolves", "Baelfang Storm-Caller").
    pub name: String,
    pub cause: String,
    pub want: String,
    pub fear: String,
    pub sign: String,
    /// On the land from the first day (the named beast, the pack).
    pub from_start: bool,
}

impl Colony {
    /// The tile's pack: its commonest predator, and how many the land keeps (a km²).
    pub fn pack(&self) -> Option<(String, f32)> {
        self.map.predators.iter().cloned().max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
    }

    /// The graves of the nameless dead (soldiers whose names are lost), and on a bone field the
    /// old bones on the ground within forty cells of the fire. (Elsewhere bones are a predator's
    /// leavings, not the dead.)
    pub fn nameless_graves(&self) -> Vec<Pos> {
        let mut out: Vec<Pos> = self.marks.iter().filter(|m| m.kind == MarkKind::Grave && m.title == "An old grave").map(|m| m.at).collect();
        if !self.map.bone_field { return out; }
        let (w, c) = (self.map.width, self.camp);
        for (i, f) in self.map.features.iter().enumerate() {
            if *f != Feature::Bones { continue; }
            let p = ((i % w) as u16, (i / w) as u16);
            if (p.0 as i32 - c.0 as i32).abs().max((p.1 as i32 - c.1 as i32).abs()) <= 40 { out.push(p); }
        }
        out
    }

    /// The battle the nameless fell in, from their graves' words ("Fell at the Battle of X in Y").
    fn nameless_battle(&self) -> Option<String> {
        self.marks.iter().filter(|m| m.kind == MarkKind::Grave && m.title == "An old grave")
            .find_map(|m| m.text.split("Fell at ").nth(1).map(|r| { let r = r.trim_end_matches('.'); r.rsplit_once(" in ").map(|(a, _)| a).unwrap_or(r).to_string() }))
    }

    /// The kinds of creature on this embark, each with its cause, want, fear and sign.
    pub fn bestiary(&self) -> Vec<Kind> {
        let mut out = Vec::new();
        // The named beast near its lair.
        if let Some(t) = self.arc.as_ref().map(|a| &a.threat).filter(|t| t.kind == super::arc::ThreatKind::Beast) {
            let what = t.monster.as_ref().map(|m| m.short.clone()).unwrap_or_else(|| "a beast of legend".into());
            out.push(Kind {
                name: t.name.clone(),
                cause: format!("{} ({})", t.why, what),
                want: "its hoard and its ground".into(),
                fear: "a closed wall and many spears".into(),
                sign: t.cause_text.clone().map(|c| format!("the chronicle's kills: {}", c)).unwrap_or_else(|| "kills and scorched earth on the roads".into()),
                from_start: true,
            });
        }
        // The pack.
        if let Some((name, d)) = self.pack() {
            let name = plural(&name);
            out.push(Kind {
                name: name.clone(),
                cause: format!("the land here keeps {} ({:.2} a km², from the tile's ecology)", name, d),
                want: "meat: a lone settler or a stray beast far from the fire".into(),
                fear: "fire: they will not come into the firelight".into(),
                sign: if self.dens().is_empty() { "tracks at the edge of the woods".into() } else { format!("tracks, and a den at {}", self.dens().iter().map(|d| format!("{},{}", d.0, d.1)).next().unwrap_or_default()) },
                from_start: true,
            });
        }
        // The nameless dead (who walk only with myth).
        let graves = self.nameless_graves();
        if !graves.is_empty() && crate::myth::on() {
            let battle = self.nameless_battle();
            out.push(Kind {
                name: "the nameless dead".into(),
                cause: match &battle { Some(b) => format!("soldiers who fell at {} lie here with their names lost ({} graves)", b, graves.len()), None => "this is a bone field, and its dead lie unburied".to_string() },
                want: "a name and a burial".into(),
                fear: "nothing; they rest when given what they want".into(),
                sign: "a cold pale light among the old graves at night".into(),
                from_start: false,
            });
        }
        // The Shadow's things.
        if self.darkness >= 0.2 && super::shadow_in_camp() {
            out.push(Kind {
                name: format!("the things of {}", self.shadow_name.clone().unwrap_or_else(|| "the Shadow".into())),
                cause: format!("the Shadow's darkness lies on this tile ({:.2})", self.darkness),
                want: "to spread".into(), fear: "blessed ground, which they cannot cross".into(), sign: "the blight on the land".into(),
                from_start: false,
            });
        }
        // The caverns: what lives there once the stair reaches it, and what sleeps deepest.
        for c in &self.map.caverns {
            let open = self.breached.contains(&c.layer);
            if open && !c.life.is_empty() {
                out.push(Kind {
                    name: crate::persona::list(&c.life),
                    cause: format!("they live in {}, {} levels down, which the stair broke into", c.name, c.depth_levels),
                    want: "the dark kept to themselves; the hunters, meat".into(), fear: "light, and a barred hatch".into(), sign: "a cold draught up the stair".into(),
                    from_start: false,
                });
            }
            if let Some((name, m)) = &c.beast {
                let woken = self.moments.iter().any(|x| x.text.contains(name.as_str()));
                out.push(Kind {
                    name: if woken { name.clone() } else { "something that sleeps in the deep".into() },
                    cause: format!("it sleeps in {} ({}), and the old songs speak of it", c.name, m.short),
                    want: "to be left asleep".into(), fear: "nothing under the world".into(), sign: "a sound from below, when the picks come near".into(),
                    from_start: false,
                });
            }
        }
        out
    }

    /// "Bestiary: wolves (the land here keeps wolves ...); ..." with the count at founding.
    pub fn bestiary_line(&self) -> String {
        let b = self.bestiary();
        let start = b.iter().filter(|k| k.from_start).count();
        format!("Bestiary: {} kinds, {} on the land from the first day: {}", b.len(), start,
            b.iter().map(|k| format!("{} ({}; wants {}; fears {}; its sign: {})", k.name, k.cause, k.want, k.fear, k.sign)).collect::<Vec<_>>().join("; "))
    }

    /// 23:30 from the fifth night: where the nameless dead lie unburied, one night in three a pale
    /// soldier walks among their graves, and the one sleeping nearest cannot sleep after. After
    /// two such nights, someone who holds the old ways dear (or the pious, or the speaker, or the
    /// eldest) means to bury them and give them a name (`bury_option`).
    pub(crate) fn reckon_nameless(&mut self) {
        // (Hauntings are myth: `crate::myth`.)
        if !crate::myth::on() { return; }
        if self.nameless_rest || self.clock.hour() != 23 || self.clock.minute() != 30 || self.clock.day() < 5 { return; }
        let graves = self.nameless_graves();
        if graves.is_empty() { return; }
        if crate::history::settlers::hash_pub(self.seed ^ 0x6A57, self.clock.day()) % 3 != 0 { return; }
        let at = graves[(self.clock.day() as usize) % graves.len()];
        let Some(i) = (0..self.settlers.len()).filter(|&i| self.settlers[i].alive && !self.below(i))
            .min_by_key(|&i| (self.settlers[i].pos.0 as i32 - at.0 as i32).abs().max((self.settlers[i].pos.1 as i32 - at.1 as i32).abs())) else { return };
        let name = self.settlers[i].name.clone();
        self.hauntings += 1;
        self.haunt_night = Some((self.clock.day(), at));
        let battle = self.nameless_battle();
        let who = match &battle { Some(b) => format!("a pale soldier of {}", b), None => "a pale figure".into() };
        let line = format!("{} walks among the old graves at {},{} in the night and sinks into the ground; {} saw it, and cannot sleep after.", super::arc::capital_word(&who), at.0, at.1, name);
        self.note(line.clone());
        if self.hauntings == 1 {
            self.moment("The nameless dead".into(), line, "because they fell here with their names lost, and no one buried them".into(), at);
        }
        self.settlers[i].fatigue = (self.settlers[i].fatigue + 0.3).min(1.0);
        self.feel(i, mind::Feel::TheDeep { what: "the nameless dead".into() });
        if self.hauntings >= 2 && self.burier.is_none() {
            use crate::persona::{Facet, Val};
            let grown = |c: &Colony, j: usize| c.settlers[j].alive && c.settlers[j].guest_until == 0 && c.settlers[j].past.as_ref().map_or(true, |p| p.age >= 16);
            let pick = (0..self.settlers.len()).filter(|&j| grown(self, j) && self.settlers[j].persona.value(Val::Tradition) >= 10)
                .max_by_key(|&j| (self.settlers[j].persona.value(Val::Tradition), std::cmp::Reverse(j)))
                .or_else(|| (0..self.settlers.len()).filter(|&j| grown(self, j) && self.settlers[j].persona.facet(Facet::Piety) >= 50).max_by_key(|&j| (self.settlers[j].persona.facet(Facet::Piety), std::cmp::Reverse(j))))
                .or_else(|| self.speaker.filter(|&j| grown(self, j)))
                .or_else(|| (0..self.settlers.len()).filter(|&j| grown(self, j)).max_by_key(|&j| (self.settlers[j].past.as_ref().map_or(0, |p| p.age), std::cmp::Reverse(j))));
            if let Some(j) = pick {
                self.burier = Some(j);
                let n = self.settlers[j].name.clone();
                self.note(format!("{} says the dead of the old graves walk because no one gave them a name or a burial, and means to give them both.", n));
            }
        }
    }

    /// The one who means to bury the nameless dead, by day: to their graves.
    pub(crate) fn bury_option(&self, i: usize) -> Option<(f32, Job, String)> {
        if self.nameless_rest || self.burier != Some(i) || self.clock.is_night() { return None; }
        let graves = self.nameless_graves();
        let at = *graves.first()?;
        let battle = self.nameless_battle().map(|b| format!(" of {}", b)).unwrap_or_default();
        Some((1.2, Job::Wander(at), format!("Gathering the old bones into the graves at {},{} and carving a stone for the nameless dead{}: they walk until someone gives them a name", at.0, at.1, battle)))
    }

    /// The burier at the graves: the bones gathered in, a stone carved with the name they are
    /// given, and the hauntings end (said, and a mark).
    pub(crate) fn bury_the_nameless(&mut self) {
        let Some(i) = self.burier else { return };
        if self.nameless_rest || !self.settlers[i].alive { if !self.settlers[i].alive { self.burier = None; } return; }
        let Job::Wander(t) = self.settlers[i].job else { return };
        if !self.settlers[i].why.starts_with("Gathering the old bones") || self.settlers[i].pos != t || !self.settlers[i].path.is_empty() { return; }
        let (w, c) = (self.map.width, self.camp);
        let mut bones = 0;
        for k in 0..self.map.features.len() {
            if self.map.features[k] != Feature::Bones { continue; }
            let p = ((k % w) as i32, (k / w) as i32);
            if (p.0 - c.0 as i32).abs().max((p.1 - c.1 as i32).abs()) <= 40 { self.map.features[k] = Feature::None; bones += 1; }
        }
        let battle = self.nameless_battle();
        let name = match &battle { Some(b) => format!("the fallen of {}", b), None => "the dead of the field".into() };
        for m in self.marks.iter_mut().filter(|m| m.kind == MarkKind::Grave && m.title == "An old grave") {
            m.title = format!("A grave of {}", name);
            m.text = format!("{} Named and buried on day {}.", m.text, self.clock.day());
        }
        let who = self.settlers[i].name.clone();
        let day = self.clock.day();
        self.marks.push(ColonyMark { at: t, kind: MarkKind::Stone, title: format!("The stone of {}", name),
            text: format!("Here lie {}, whose names are lost; {} carved this so that they rest, on day {}.", name, who, day), day });
        self.nameless_rest = true;
        self.haunt_night = None;
        let line = format!("{} gathers {} into the graves at {},{} and carves a stone for {}: \"here they lie, and rest\". No pale light walks among the graves after.",
            who, if bones > 0 { "the bones of the field" } else { "what bones the ground gives up" }, t.0, t.1, name);
        self.note(line.clone());
        self.moment("The nameless dead rest".into(), line, "because they were given a name and a burial, which is all they wanted".into(), t);
        self.settlers[i].job = Job::Idle;
        for j in 0..self.settlers.len() { if self.settlers[j].alive { self.feel(j, mind::Feel::Remembered { whom: name.clone() }); } }
    }

    /// Wolves fear fire: one on its way that would come within six cells of a lit fire stops at
    /// the edge of the firelight (said once).
    pub(crate) fn wolves_fear_fire(&mut self, k: usize) -> bool {
        let c = &self.creatures[k];
        if c.kind != super::creatures::CreatureKind::Wolf || c.leaving || c.name != "a wolf" { return false; }
        // (Debug: PLANET_FORCE_PET_PREY=1 sends them to a pet's keeper wherever they are, the fire
        // or no fire: the test is of the pet's defence.)
        if std::env::var("PLANET_FORCE_PET_PREY").is_ok() { return false; }
        let Some(&next) = c.path.first() else { return false };
        let camp = self.camp;
        if (next.0 as i32 - camp.0 as i32).abs().max((next.1 as i32 - camp.1 as i32).abs()) > 6 { return false; }
        self.creatures[k].path.clear();
        self.once("wolves and fire", "The wolves circle at the edge of the firelight, their eyes green in it, and will not come nearer the fire.".into());
        true
    }
}
