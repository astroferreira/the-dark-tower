//! Strange moods: one settler is seized, claims the workshop, and makes an artifact.
//!
//! The idea is Dwarf Fortress's strange moods. Once a workshop stands, a dawn may find (again 120
//! days after the last began, in a camp of ten, never the same settler twice: `moods_had`) the most creative settler (creativity, art-inclination, sure hands; the chance
//! grows with the camp's days) taken by a mood: they claim the workshop and want stone and wood,
//! three loads of each, laid by. With them they work day on day and make an artifact: masterful,
//! named in their own tongue, of the camp's stone or wood, showing a moment of their own past;
//! they rise to a master's hand at building. If the loads are not there in six days, the mood
//! sours: they go mad (a break that does not mend) and in the end walk away.

use super::*;
use crate::persona::{Attr, Facet};

/// The kind of mood, from the one it takes (as Dwarf Fortress's fey, secretive, possessed,
/// macabre and fell moods).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MoodKind {
    /// Says what it wants.
    Fey,
    /// Will not say what more it wants (the bashful and the distrustful).
    Secretive,
    /// Something else works their hands: no loads wanted, but no skill gained (the imaginative
    /// and devout, or anyone under the Shadow's darkness).
    Possessed,
    /// Wants bone (the gloomy): from the hunt, or the graves.
    Macabre,
    /// Wants a life (the cruel and violent): kills the one nearest, and makes of their bones.
    Fell,
}

impl MoodKind {
    pub fn word(self) -> &'static str {
        match self { MoodKind::Fey => "a fey mood", MoodKind::Secretive => "a secretive mood", MoodKind::Possessed => "a possession", MoodKind::Macabre => "a macabre mood", MoodKind::Fell => "a fell mood" }
    }
}

#[derive(Clone, Debug)]
pub struct Mood {
    pub kind: MoodKind,
    /// A fell mood's victim, once taken.
    pub victim: Option<String>,
    pub who: usize,
    pub since: u64,
    /// Work done on it (days at the workshop with the loads at hand).
    pub days: u32,
    pub done: bool,
    /// The one thing more it wants: the maker's liked material.
    pub wants: String,
}

/// Whether the camp has the wanted material: a metal once its own ore is struck (iron also when
/// bought), bone and
/// hide once a beast is hunted, gems and fine stone once the mine strikes ore or breaks into a
/// cavern; the land's own wood and stone always.
fn has(c: &Colony, m: &str) -> bool {
    match m {
        // The metal itself, dug from the camp's own seams (iron also from the caravans' tools).
        "iron" => c.ores.iter().any(|o| o == "iron") || c.tools_bought,
        "copper" | "tin" | "silver" | "gold" => c.ores.iter().any(|o| o == m),
        "bone" => c.hunted > 0 || c.marks.iter().any(|m| m.kind == MarkKind::Grave),
        "horn" | "leather" | "wool" => c.hunted > 0,
        "amber" | "jet" | "rock crystal" | "garnets" | "marble" | "obsidian" | "agate" => c.gems.iter().any(|g| g.0 == m && g.1 > 0) || !c.breached.is_empty(),
        _ => true,
    }
}

/// What a mood needs: stone and wood, three loads of each.
const NEED: usize = 3;

impl Colony {
    /// Each dawn: perhaps a mood strikes; a mood under way works or sours.
    pub(crate) fn reckon_mood(&mut self) {
        let day = self.clock.day();
        if let Some(m) = self.mood.clone().filter(|m| !m.done && self.settlers[m.who].alive) {
            let stone = self.items.iter().filter(|it| it.stored && it.kind == ItemKind::Stone).count();
            let wood = self.items.iter().filter(|it| it.stored && it.kind == ItemKind::Log).count();
            let name = self.settlers[m.who].name.clone();
            let wanted = has(self, &m.wants);
            // A fell mood takes its victim on the first dawn.
            if m.kind == MoodKind::Fell && m.victim.is_none() {
                let me = self.settlers[m.who].pos;
                let near = (0..self.settlers.len()).filter(|&j| j != m.who && self.settlers[j].alive && self.settlers[j].guest_until == 0)
                    .min_by_key(|&j| ((self.settlers[j].pos.0 as i32 - me.0 as i32).abs().max((self.settlers[j].pos.1 as i32 - me.1 as i32).abs()), j));
                if let Some(v) = near {
                    let vn = self.settlers[v].name.clone();
                    let line = format!("{}, in the grip of a fell mood, drags {} into the workshop and kills {} there.", name, vn, if self.settlers[v].persona.female { "her" } else { "him" });
                    self.note(line.clone());
                    let at = self.settlers[v].pos;
                    self.moment(format!("{} is killed", vn), line, format!("because {} was taken by a fell mood, and {} {}", name, if self.settlers[m.who].persona.female { "she" } else { "he" }, self.settlers[m.who].persona.facet_phrase(Facet::Cruelty as usize).unwrap_or_else(|| "is cruel".into())), at);
                    self.bury(v, &format!("in the workshop, killed by {} in a fell mood", name));
                    for j in 0..self.settlers.len() {
                        if j != m.who && self.settlers[j].alive { self.feel(j, mind::Feel::Death { whom: vn.clone(), close: self.opinion(v, j) >= 6 }); self.like(m.who, j, -12); }
                    }
                    if let Some(mm) = self.mood.as_mut() { mm.victim = Some(vn.clone()); mm.wants = format!("the bones of {}", vn); }
                }
                return;
            }
            let loads = match m.kind { MoodKind::Possessed | MoodKind::Fell => true, _ => stone >= NEED && wood >= NEED };
            let wanted = wanted || m.kind == MoodKind::Possessed || m.kind == MoodKind::Fell;
            if loads && wanted {
                let mm = self.mood.as_mut().unwrap();
                mm.days += 1;
                if mm.days >= 3 { self.finish_artifact(m.who, NEED); }
                else if mm.days == 1 { self.note(format!("{} works at the workshop without a word, with stone and wood laid by.", name)); }
            } else if day >= m.since + 6 {
                // Soured: madness.
                let mm = self.mood.as_mut().unwrap();
                mm.done = true;
                let why = if wanted { format!("{} wanted {} stone and {} wood laid by, and the camp had {} and {}", if self.settlers[m.who].persona.female { "she" } else { "he" }, NEED, NEED, stone, wood) }
                    else { format!("{} wanted {}, and the camp had none", if self.settlers[m.who].persona.female { "she" } else { "he" }, m.wants) };
                let line = format!("{}'s mood sours into madness: {} tears at the workshop's walls and will not be spoken to.", name, if self.settlers[m.who].persona.female { "she" } else { "he" });
                self.note(format!("{} ({}).", line, why));
                let at = self.settlers[m.who].pos;
                self.moment(format!("{} goes mad", name), line, format!("because {}", why), at);
                let s = &mut self.settlers[m.who];
                s.mind.stress = s.mind.stress.max(mind::LEAVE_AT);
                s.mind.breaks = s.mind.breaks.max(2);
                s.mind.broken = Some((mind::Break::Tantrum, self.clock.tick + 3 * TICKS_PER_DAY));
            } else if day == m.since + 1 {
                match m.kind {
                    MoodKind::Secretive => self.note(format!("{} draws shapes in the dust of the workshop floor and will not say what more is wanted ({} stone, {} wood laid by; {} of each wanted).", name, stone, wood, NEED)),
                    _ => self.note(format!("{} paces the workshop, muttering of stone, wood and {} ({} stone, {} wood laid by; {} of each wanted{}).", name, m.wants, stone, wood, NEED, if wanted { "" } else { "; there is no such thing in the camp" })),
                }
            }
            return;
        }
        // Moods come again (DF: each dwarf at most once): a hundred and twenty days after the last
        // began, in a camp of ten, to one who has not had one.
        if self.workshop_spot().is_none() || day < 20 { return; }
        if let Some(m) = &self.mood { if day < m.since + 120 || self.alive() < 10 { return; } }
        // The chance grows with the camp's age: about one in forty dawns.
        if crate::history::settlers::hash_pub(self.seed ^ day, 0x300D) % 40 != 0 { return; }
        let score = |s: &Settler| s.persona.attr(Attr::Creativity) / 1000.0 + s.persona.facet(Facet::ArtInclined) as f32 / 100.0 + s.persona.attr(Attr::KinestheticSense) / 2000.0;
        let had: Vec<usize> = self.moods_had.clone();
        let Some(who) = (0..self.settlers.len()).filter(|&i| self.settlers[i].alive && self.settlers[i].guest_until == 0 && self.settlers[i].mind.broken.is_none() && self.settlers[i].past.as_ref().map_or(true, |p| p.age >= 14) && !had.contains(&i))
            .max_by(|&a, &b| score(&self.settlers[a]).total_cmp(&score(&self.settlers[b])).then(b.cmp(&a))) else { return };
        self.mood_done = true;
        self.moods_had.push(who);
        let p = &self.settlers[who].persona;
        let alive = self.alive();
        let kind = if p.facet(Facet::Cruelty) >= 75 && p.facet(Facet::Violence) >= 60 && alive >= 5 { MoodKind::Fell }
            else if p.facet(Facet::Gloom) >= 70 { MoodKind::Macabre }
            else if (p.facet(Facet::Imagination) >= 80 && p.facet(Facet::Piety) >= 60) || (self.darkness >= 0.3 && crate::history::settlers::hash_pub(self.seed, 0x9055) % 3 == 0) { MoodKind::Possessed }
            else if p.facet(Facet::Bashfulness) >= 70 || p.facet(Facet::Trust) <= 20 { MoodKind::Secretive }
            else { MoodKind::Fey };
        // PLANET_FORCE_MOOD=fey|secretive|possessed|macabre|fell (for trying each).
        let kind = match std::env::var("PLANET_FORCE_MOOD").as_deref() {
            Ok("fey") => MoodKind::Fey, Ok("secretive") => MoodKind::Secretive, Ok("possessed") => MoodKind::Possessed,
            Ok("macabre") => MoodKind::Macabre, Ok("fell") => MoodKind::Fell, _ => kind,
        };
        let wants = if kind == MoodKind::Macabre { "bone".to_string() } else { self.settlers[who].persona.likes.material.clone() };
        // (PLANET_FORCE_MOOD_WANT=tin forces what the mood asks for, for the madness's test.)
        let wants = std::env::var("PLANET_FORCE_MOOD_WANT").ok().filter(|w| !w.is_empty()).unwrap_or(wants);
        self.mood = Some(Mood { kind, victim: None, who, since: day, days: 0, done: false, wants });
        let name = self.settlers[who].name.clone();
        let line = match kind {
            MoodKind::Fey => format!("{} is taken by a strange mood and claims the workshop.", name),
            MoodKind::Secretive => format!("{} is taken by a secretive mood, claims the workshop and speaks to no one.", name),
            MoodKind::Possessed => format!("{} is possessed: {} walks into the workshop with someone else's eyes.", name, if self.settlers[who].persona.female { "she" } else { "he" }),
            MoodKind::Macabre => format!("{} is taken by a macabre mood, claims the workshop and mutters of bones.", name),
            MoodKind::Fell => format!("{} is taken by a fell mood and stares at the others as if weighing them.", name),
        };
        self.note(line.clone());
        let at = self.settlers[who].pos;
        self.moment(format!("{} is taken by a mood", name), line, format!("because {} {}", if self.settlers[who].persona.female { "she" } else { "he" }, self.settlers[who].persona.facet_phrase(Facet::ArtInclined as usize).unwrap_or_else(|| "has the makings of a master".into())), at);
    }

    /// The mood's settler: at the workshop all day (their job), unless they must eat or sleep.
    pub(crate) fn mood_choice(&self, i: usize) -> Option<(Job, String)> {
        let m = self.mood.as_ref()?;
        if m.done || m.who != i || self.clock.is_night() || self.settlers[i].hunger >= 0.7 { return None; }
        let spot = self.workshop_spot()?;
        Some((Job::Wander(spot), "Taken by a mood: at the workshop, and will not leave it".into()))
    }

    /// The artifact: the loads used, a name in the maker's tongue, an image of their past.
    fn finish_artifact(&mut self, i: usize, need: usize) {
        use rand::SeedableRng;
        for kind in [ItemKind::Stone, ItemKind::Log] {
            for _ in 0..need { if let Some(k) = self.items.iter().position(|it| it.stored && it.kind == kind) { self.items.remove(k); self.fix_refs_pub(k); } }
        }
        let p = &self.settlers[i].persona;
        let arche = crate::history::entities::races::RaceType::from_tag(&p.race).default_naming_archetype();
        let style = crate::history::naming::styles::NamingStyle::from_archetype(crate::history::NamingStyleId(0), arche);
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(self.seed ^ 0xA27F ^ i as u64);
        let title = crate::history::naming::generator::NameGenerator::artifact_name(&style, &mut rng);
        let image = self.settlers[i].past.as_ref().and_then(|x| x.images.first().cloned());
        let kind = ["altar", "statue", "chest", "throne", "standing totem"][(crate::history::settlers::hash_pub(self.seed, i as u64) % 5) as usize];
        let wants = self.mood.as_ref().map(|m| m.wants.clone()).unwrap_or_else(|| "wood".into());
        let mk = self.mood.as_ref().map_or(MoodKind::Fey, |m| m.kind);
        let stuff = match mk { MoodKind::Fell => "bone".to_string(), MoodKind::Possessed => "stone and wood".to_string(), MoodKind::Macabre => "stone, wood and bone".to_string(), _ => format!("stone, wood and {}", wants) };
        let image = if mk == MoodKind::Fell { self.mood.as_ref().and_then(|m| m.victim.clone()).map(|v| (format!("the death of {}", v), image.as_ref().map(|x| x.1).unwrap_or(crate::history::EventId(0)))) } else { image };
        let stuff_said = if mk == MoodKind::Fell { wants.clone() } else { stuff.clone() };
        let what = format!("{}, {} {} of {}{}", title, if kind.starts_with(['a', 'e', 'i', 'o', 'u']) { "an" } else { "a" }, kind, stuff_said, image.as_ref().map(|(t, _)| format!(", showing {}", t)).unwrap_or_default());
        let name = self.settlers[i].name.clone();
        let day = self.clock.day();
        self.works.push(craft::Work { maker: i, kind: kind.to_string(), material: stuff.clone(), quality: 5, image: image.map(|(t, e)| (t, if mk == MoodKind::Fell { None } else { Some(e) })), day, called: Some(title.clone()), traded: true });
        self.settlers[i].made.push(format!("{} (day {}, an artifact)", what, day));
        // The possessed learn nothing: it was not their hands.
        if mk != MoodKind::Possessed { self.settlers[i].skill[4] = self.settlers[i].skill[4].max(0.95); }
        if let Some(m) = self.mood.as_mut() { m.done = true; }
        let line = format!("{} has made an artifact: {}.", name, what);
        self.note(line.clone());
        let at = self.settlers[i].pos;
        self.moment(format!("{}", title), line, format!("because {} was taken by a mood and the camp laid stone and wood by", name), at);
        self.feel(i, mind::Feel::Made { what: title.clone(), quality: 5 });
        for j in 0..self.settlers.len() { if j != i && self.settlers[j].alive { self.feel(j, mind::Feel::Admired { what: title.clone() }); } }
    }
}
