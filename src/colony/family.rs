//! Families: weddings by the fire, and children born in the camp.
//!
//! The idea is Dwarf Fortress's relationships: dwarves who are close become lovers and marry,
//! and couples have children who inherit from both. Here, each dawn, two unwed adults (16+, not
//! guests, of one people) who think the world of each other (opinion 20+ both ways, from day 30) and
//! do not despise romance may wed (one dawn in thirty): a moment, and the camp keeps it like a festival.
//! A wedded couple with a wife of 45 or younger, no child under a year (120 days) and three meals
//! a head stored conceives one dawn in forty; sixty days later the child is born in the camp: a
//! settler of age 0 named in the mother's tongue, whose persona takes after both parents (each
//! facet from one or the other, hair, skin and eyes from one), with a past ("born in the camp
//! on day N to A and B"). Infants (under 4) do not work: they stay by their mother (`decide`),
//! eat and sleep. The parents and the camp are glad; a child's death is the heaviest grief.

use super::*;
use crate::persona::Val;

impl Colony {
    fn adult(&self, i: usize) -> bool {
        let s = &self.settlers[i];
        s.alive && !s.mind.left && s.guest_until == 0 && s.past.as_ref().map_or(true, |p| p.age >= 16)
    }

    /// Dawn: weddings, conceptions, births.
    pub(crate) fn reckon_family(&mut self) {
        let day = self.clock.day();
        // Births due.
        for k in 0..self.expecting.len() {
            let (mother, father, due) = self.expecting[k];
            if due != day { continue; }
            if !self.settlers[mother].alive { continue; }
            self.birth(mother, father);
        }
        self.expecting.retain(|&(_, _, due)| due > day);
        // Weddings.
        let n = self.settlers.len();
        for a in 0..n {
            // (The widowed mourn sixty days before they wed again.)
            let mourning = |c: &Colony, x: usize| c.widowed.iter().any(|w| w.0 == x && day < w.2 + 60);
            if !self.adult(a) || self.settlers[a].spouse.is_some() || mourning(self, a) { continue; }
            for b in a + 1..n {
                if !self.adult(b) || self.settlers[b].spouse.is_some() || mourning(self, b) { continue; }
                let (pa, pb) = (&self.settlers[a].persona, &self.settlers[b].persona);
                if pa.race != pb.race || pa.female == pb.female { continue; }
                if pa.value(Val::Romance) <= -26 || pb.value(Val::Romance) <= -26 { continue; }
                if day < 30 || self.opinion(a, b) < 20 || self.opinion(b, a) < 20 { continue; }
                if crate::history::settlers::hash_pub(self.seed ^ day, 0x3ED + (a * 64 + b) as u64) % 30 != 0 { continue; }
                self.settlers[a].spouse = Some(b);
                self.settlers[b].spouse = Some(a);
                let (an, bn) = (self.settlers[a].name.clone(), self.settlers[b].name.clone());
                let line = format!("{} and {} are wed by the fire, and the camp eats and sings with them.", an, bn);
                self.note(line.clone());
                let at = self.camp;
                self.moment(format!("The wedding of {} and {}", an, bn), line, format!("because {} and {} have grown close since they came", an, bn), at);
                for j in 0..n {
                    if self.settlers[j].alive { self.feel(j, mind::Feel::Festival { what: format!("the wedding of {} and {}", an, bn) }); }
                }
                break;
            }
        }
        // Conceptions.
        for i in 0..n {
            let Some(j) = self.settlers[i].spouse else { continue };
            if i > j || !self.settlers[i].alive || !self.settlers[j].alive { continue; }
            let (mother, father) = if self.settlers[i].persona.female { (i, j) } else { (j, i) };
            if self.settlers[mother].past.as_ref().map_or(30, |p| p.age) > 45 { continue; }
            if self.expecting.iter().any(|e| e.0 == mother) { continue; }
            if self.born.iter().any(|&(m, d)| m == mother && d + 120 > day) { continue; }
            if self.food_stored() < 3 * self.alive() as u32 { continue; }
            if crate::history::settlers::hash_pub(self.seed ^ day, 0xB4B + mother as u64) % 40 != 0 { continue; }
            self.expecting.push((mother, father, day + 60));
            let mn = self.settlers[mother].name.clone();
            self.note(format!("{} is with child.", mn));
        }
    }

    fn birth(&mut self, mother: usize, father: usize) {
        use rand::SeedableRng;
        let day = self.clock.day();
        let (m, f) = (&self.settlers[mother], &self.settlers[father]);
        // A name in the mother's tongue.
        let arche = crate::history::entities::races::RaceType::from_tag(&m.persona.race).default_naming_archetype();
        let style = crate::history::naming::styles::NamingStyle::from_archetype(crate::history::NamingStyleId(0), arche);
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(self.seed ^ 0xB1B7 ^ day ^ (mother as u64) << 12);
        let name = crate::history::naming::generator::NameGenerator::personal_name(&style, &mut rng).split_whitespace().next().unwrap_or("Ash").to_string();
        // Takes after both.
        let seed = crate::persona::seed_of(&name, self.seed ^ day);
        let mut p = crate::persona::Persona::roll(&m.persona.race, None, seed);
        for k in 0..p.facets.len().min(m.persona.facets.len()).min(f.persona.facets.len()) {
            let from = if crate::history::settlers::hash_pub(seed, k as u64) % 2 == 0 { &m.persona } else { &f.persona };
            let jitter = (crate::history::settlers::hash_pub(seed, 100 + k as u64) % 21) as i32 - 10;
            p.facets[k] = (from.facets[k] as i32 + jitter).clamp(0, 100) as u8;
        }
        let looks = if crate::history::settlers::hash_pub(seed, 0xC0) % 2 == 0 { &m.persona } else { &f.persona };
        p.hair = looks.hair.clone();
        p.eyes = if crate::history::settlers::hash_pub(seed, 0xC1) % 2 == 0 { m.persona.eyes.clone() } else { f.persona.eyes.clone() };
        p.skin = if crate::history::settlers::hash_pub(seed, 0xC2) % 2 == 0 { m.persona.skin.clone() } else { f.persona.skin.clone() };
        p.beard = false;
        let camp = self.name.clone().unwrap_or_else(|| "the camp".into());
        let (mn, fname) = (m.name.clone(), f.name.clone());
        let mut past = crate::history::settlers::Past { age: 0, people: m.past.as_ref().and_then(|x| x.people), calling: format!("born in {}", camp), persona: Some(p.clone()), ..Default::default() };
        past.lines.push((format!("Born in {} on day {} to {} and {}.", camp, day, mn, fname), None));
        past.arts = m.past.as_ref().map(|x| x.arts.clone()).unwrap_or_default();
        past.faith = m.past.as_ref().and_then(|x| x.faith.clone());
        past.feeling = None;
        self.add_settler(name.clone(), Some(past));
        let c = self.settlers.len() - 1;
        self.settlers[c].pos = self.settlers[mother].pos;
        self.settlers[c].hunger = 0.2;
        self.settlers[c].fatigue = 0.2;
        self.born.push((mother, day));
        self.children.push((c, mother, father));
        for j in [mother, father] { self.like(j, c, 12); }
        let line = format!("{} gives birth to a {}, {}; {} is the father.", mn, if p.female { "daughter" } else { "son" }, name, fname);
        self.note(line.clone());
        let at = self.settlers[mother].pos;
        self.moment(format!("{} is born", name), line, format!("because {} and {} were wed in the camp", mn, fname), at);
        for j in [mother, father] { self.feel(j, mind::Feel::Festival { what: format!("the birth of {}", name) }); }
        for j in 0..self.settlers.len() { if j != mother && j != father && j != c && self.settlers[j].alive && self.settlers[j].persona.facet(crate::persona::Facet::Love) >= 60 { self.feel(j, mind::Feel::Friend { with: format!("little {}", name) }); } }
    }

    /// An infant stays by its mother (or the fire), eating and sleeping.
    pub(crate) fn infant_choice(&self, i: usize) -> Option<(Job, String)> {
        let (_, mother, _) = self.children.iter().find(|c| c.0 == i).copied()?;
        // Out of arms at four (`ageing.rs`).
        if self.settlers[i].past.as_ref().map_or(0, |p| p.age) >= 4 { return None; }
        let s = &self.settlers[i];
        if s.hunger >= 0.6 && self.food_stored() > 0 { return Some((Job::Eat, "Hungry, and fed by the fire".into())); }
        if self.clock.is_night() || s.fatigue >= 0.5 { return Some((Job::Sleep, "Asleep, a small bundle by the fire".into())); }
        let m = &self.settlers[mother];
        let to = if m.alive { m.pos } else { self.camp };
        let who = if m.alive { format!("Carried about by {}", m.name) } else { "Looked after by the whole camp".into() };
        Some((Job::Wander(to), who))
    }

    /// "wed to X; mother of Y", for the annals and the settler page.
    pub fn family_of(&self, i: usize) -> Option<String> {
        let mut bits = Vec::new();
        if let Some(j) = self.settlers[i].spouse { bits.push(format!("wed to {}", self.settlers[j].name)); }
        for w in self.widowed.iter().filter(|w| w.0 == i) {
            let word = if self.settlers[i].persona.female { "widow" } else { "widower" };
            bits.push(if self.settlers[i].spouse.is_some() { format!("once wed to {}", self.settlers[w.1].name) } else { format!("{} of {}", word, self.settlers[w.1].name) });
        }
        let kids: Vec<String> = self.children.iter().filter(|c| c.1 == i || c.2 == i).map(|c| self.settlers[c.0].name.clone()).collect();
        if !kids.is_empty() { bits.push(format!("{} of {}", if self.settlers[i].persona.female { "mother" } else { "father" }, crate::persona::list(&kids))); }
        if let Some(&(_, m, f)) = self.children.iter().find(|c| c.0 == i) { bits.push(format!("child of {} and {}", self.settlers[m].name, self.settlers[f].name)); }
        (!bits.is_empty()).then(|| bits.join("; "))
    }

    /// What the camp knows of settler `i` besides their past, for their page: family, pet,
    /// spear, guesthood.
    pub fn about(&self, i: usize) -> Vec<String> {
        let cap = |t: String| { let mut c = t.chars(); c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default() };
        let mut out = Vec::new();
        if let Some(f) = self.family_of(i) { out.push(cap(f)); }
        if let Some(p) = self.pet_of(i) { out.push(cap(p)); }
        if let Some(g) = self.guild_of(i) { out.push(g); }
        if let Some(v) = self.vow_of(i) { out.push(v); }
        if let Some(n) = self.needs_text(i) { out.push(n); }
        if let Some(d) = self.dream_line(i) { out.push(d); }
        if let Some(a) = self.arm_of(i) { out.push(format!("Bears {} in the militia{}", a.kind, self.armour_of(i).map(|x| format!(", and wears {}", x.kind)).unwrap_or_default())); }
        if let Some(v) = &self.settlers[i].visitor { out.push(format!("Came as {}{}", v, if self.settlers[i].guest_until == 0 { ", and stayed" } else { "; a guest" })); }
        out
    }
}
