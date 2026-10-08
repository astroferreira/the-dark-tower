//! A lord from home: the settlers' people send one of their ruler's blood to govern the camp.
//!
//! The idea is Dwarf Fortress's nobles: as a fortress grows, its civilization sends a baron, who
//! takes the reins, makes demands (rooms, items) and mandates, and is loved or hated by how the
//! dwarves feel about such things. Here, at founding (`plan_lord`), the camp's people's ruler and
//! their nearest living kin are noted. From day 90, once 14 live in the camp (not counting guests
//! or infants), the kin arrives (`lord_arrives`) as "lord of the camp": they take the office from
//! the speaker (a moment), and demand a hall of their own (`ProjectKind::LordsHall`, 5x4 roofed,
//! the camp's most urgent work). Those who hold tradition, law or loyalty dear welcome them
//! (+3 opinion); those who prize independence resent them (-4, `Feel::Mandate`), the old speaker
//! most. While the hall stands unbuilt past twenty days the lord's displeasure is a dawn line,
//! and the lord's mandates (`society.rs`) follow their own values. Ten days after arriving, and
//! forty days after each is met, the lord demands a fine work (fine or better) of a material
//! they love if the land gives it, else of its stone or wood by turns (`lord_demands`): the
//! crafters are set to it (craft wish x1.5, the material chosen, stone broken for it), and the
//! work presented is kept (`demand_met`, +6 with its maker). Thirty days unmet, the camp's best
//! maker goes to the stocks for it (a moment; everyone thinks less of the lord) and the demand
//! stands again.

use super::*;
use crate::history::world_state::WorldHistory;
use crate::persona::Val;

#[derive(Clone, Debug)]
pub struct Lord {
    pub name: String,
    /// "Ishra the Just, ruler of The Kingdom of Titankeep".
    pub ruler: String,
    pub people: String,
    pub past: crate::history::settlers::Past,
    pub came: Option<u64>,
    /// What the lord demands (a material), since when, and whether it was met.
    pub demand: Option<(String, u64, bool)>,
}

/// The ruler of `people` and their nearest living kin (a child, a sibling, a spouse).
pub fn plan_lord(h: &WorldHistory, people: Option<crate::history::FactionId>) -> Option<Lord> {
    let f = h.factions.get(&people?)?;
    let ruler = h.figures.get(&f.current_leader?)?;
    let mut kin: Vec<&crate::history::entities::figures::Figure> = ruler.children.iter().chain(ruler.spouse.iter())
        .filter_map(|c| h.figures.get(c)).filter(|c| c.is_alive() && c.id != ruler.id).collect();
    if kin.is_empty() {
        // Siblings: the ruler's parents' other children.
        for p in [ruler.parents.0, ruler.parents.1].into_iter().flatten() {
            if let Some(pf) = h.figures.get(&p) { kin.extend(pf.children.iter().filter_map(|c| h.figures.get(c)).filter(|c| c.is_alive() && c.id != ruler.id)); }
        }
    }
    // Not one who leads a band of outlaws (`history::bands`).
    let outlaws: Vec<crate::history::FigureId> = crate::history::bands::of(h).iter().map(|b| b.leader).collect();
    kin.retain(|c| !outlaws.contains(&c.id));
    kin.sort_by_key(|c| (c.birth_date, c.id));
    let k = kin.first()?;
    let now = h.current_date.year;
    let rel = if ruler.spouse == Some(k.id) { "spouse" } else if ruler.children.contains(&k.id) { "child" } else { "sibling" };
    let calling = format!("lord of the camp, {} of {} who rules {}", rel, ruler.full_name(), f.name);
    let mut past = crate::history::settlers::Past { age: now.saturating_sub(k.birth_date.year).max(18) as u32, people: k.faction.or(people), calling: calling.clone(), ..Default::default() };
    past.persona = Some(crate::persona::Persona::of_figure(h, k));
    past.arts = crate::history::settlers::arts_of(h, past.people);
    let last = k.events.iter().rev().filter_map(|e| h.chronicle.get(*e)).next();
    if let Some(e) = last { past.lines.push((format!("{} ({}).", e.title, e.date.year), Some(e.id))); }
    crate::history::settlers::fill_craft(h, &mut past);
    Some(Lord { name: k.full_name(), ruler: format!("{}, ruler of {}", ruler.full_name(), f.name), people: f.name.clone(), past, came: None, demand: None })
}

impl Colony {
    /// Dawn: when the camp is big enough, the lord arrives.
    pub(crate) fn lord_arrives(&mut self) {
        let day = self.clock.day();
        let Some(l) = self.lord.clone() else { return };
        if l.came.is_some() || day < 90 { return; }
        let grown = (0..self.settlers.len()).filter(|&i| self.settlers[i].alive && self.settlers[i].guest_until == 0 && !self.children.iter().any(|c| c.0 == i)).count();
        if grown < 14 { return; }
        self.add_settler(l.name.clone(), Some(l.past.clone()));
        let k = self.settlers.len() - 1;
        if let Some(x) = self.lord.as_mut() { x.came = Some(day); }
        let old = self.speaker.filter(|&s| self.settlers[s].alive);
        if let Some(o) = old { if self.settlers[o].office.as_deref() == Some("Speaks for the camp") { self.settlers[o].office = None; } }
        self.speaker = Some(k);
        self.settlers[k].office = Some("Lord of the camp".into());
        let line = format!("{}, kin of {}, arrives with a writ: {} is to be lord of the camp{}.", l.name, l.ruler.replacen(", ruler of ", " who rules ", 1),
            if self.settlers[k].persona.female { "she" } else { "he" },
            old.map(|o| format!(", and {} speaks for it no longer", self.settlers[o].name)).unwrap_or_default());
        self.note(line.clone());
        let at = self.camp;
        self.moment(format!("{} comes to rule", l.name), line, format!("because the camp has grown to {} and {} wants it governed", grown, l.people), at);
        // How it sits with each: tradition, law and loyalty welcome a lord; independence does not.
        for j in 0..self.settlers.len() {
            if j == k || !self.settlers[j].alive { continue; }
            let p = &self.settlers[j].persona;
            let lean = (p.value(Val::Tradition) + p.value(Val::Law) + p.value(Val::Loyalty)) as i32 - 2 * p.value(Val::Independence) as i32;
            if lean >= 30 { self.like(k, j, 3); }
            else if lean <= -20 || Some(j) == old { self.like(k, j, -4); self.feel(j, mind::Feel::Mandate { what: format!("{} rule the camp", l.name) }); }
        }
        // A hall of their own, before anything else.
        if let Some(at) = self.find_site_pub(5, 4) {
            self.projects.insert(0, projects::Project { kind: projects::ProjectKind::LordsHall, at, needed: 20, used: 0, material: self.hut_material,
                why: format!("{} demands a hall of {} own, 5 paces by 4", l.name, if self.settlers[k].persona.female { "her" } else { "his" }), done: false, day });
        }
    }

    /// Dawn: a lord whose hall is not built after twenty days is displeased.
    pub(crate) fn lord_displeased(&mut self) {
        let Some(came) = self.lord.as_ref().and_then(|l| l.came) else { return };
        let day = self.clock.day();
        let Some(k) = self.speaker.filter(|&s| self.settlers[s].alive && self.settlers[s].office.as_deref() == Some("Lord of the camp")) else { return };
        let built = self.projects.iter().any(|p| p.done && p.kind == projects::ProjectKind::LordsHall);
        if built || day < came + 20 || (day - came) % 10 != 0 { return; }
        let name = self.settlers[k].name.clone();
        self.note(format!("{} is displeased: {} days, and still no hall fit for the lord of the camp.", name, day - came));
        for j in 0..self.settlers.len() { if j != k && self.settlers[j].alive { self.like(k, j, -1); } }
    }

    /// Dawn: a lord takes the best bedroom (DF's nobles and their room requirements) and wants it
    /// worth 8 or more (`room_value`: the bed and engravings); its furniture and its walls come
    /// first, and the camp's builder carves them by order whatever their taste. Unmet 30 days
    /// after they came, every twenty days the lord finds the rooms mean (the camp is liked less);
    /// met, the lord is pleased, once.
    pub(crate) fn lord_quarters(&mut self) {
        use super::delve::RoomKind;
        let Some(came) = self.lord.as_ref().and_then(|l| l.came) else { return };
        let Some(k) = self.speaker.filter(|&s| self.settlers[s].alive && self.settlers[s].office.as_deref() == Some("Lord of the camp")) else { return };
        let day = self.clock.day();
        // The best room: the lord's, whoever had it.
        let best = (0..self.rooms.len()).filter(|&r| self.rooms[r].kind == RoomKind::Bedroom)
            .max_by_key(|&r| (self.room_value(&self.rooms[r]), std::cmp::Reverse(r)));
        let Some(best) = best else { return };
        let mine = self.rooms.iter().position(|r| r.kind == RoomKind::Bedroom && r.owner == Some(k));
        if mine.map_or(true, |m| self.room_value(&self.rooms[m]) < self.room_value(&self.rooms[best])) && mine != Some(best) {
            let had = self.rooms[best].owner;
            self.rooms[best].owner = Some(k);
            if let Some(m) = mine { self.rooms[m].owner = had; }
            let lname = self.settlers[k].name.clone();
            match had {
                Some(o) if o != k && self.settlers[o].alive => {
                    let oname = self.settlers[o].name.clone();
                    self.note(format!("{} takes {}'s bedroom, the best below, for the lord's own{}.", lname, oname, if mine.is_some() { format!("; {} has the lord's old one", oname) } else { String::new() }));
                    self.feel(o, mind::Feel::Mandate { what: format!("give up their room to {}", lname) });
                    self.like(o, k, -3);
                }
                _ => self.note(format!("{} takes the best bedroom below for the lord's own.", lname)),
            }
        }
        let value = self.room_value(&self.rooms[best]);
        let name = self.settlers[k].name.clone();
        if value >= 8 {
            if self.milestones.insert("lord's rooms") {
                self.note(format!("{} is pleased with the lord's rooms: a bed of {} and walls carved with the camp's story.", name, self.rooms[best].furnished.clone().unwrap_or_else(|| "wood".into())));
                for j in 0..self.settlers.len() { if j != k && self.settlers[j].alive { self.like(k, j, 1); } }
            }
        } else if day >= came + 30 && (day - came) % 20 == 10 {
            let bare = if self.rooms[best].furnished.is_some() { "a bed and bare walls are" } else { "a bare room of rock is" };
            self.note(format!("{} finds the lord's rooms mean: {} no place for the kin of a ruler.", name, bare));
            for j in 0..self.settlers.len() { if j != k && self.settlers[j].alive { self.like(k, j, -1); } }
        }
    }

    /// Whose room comes first for furniture and engravings: the lord's.
    pub(crate) fn lords_room(&self) -> Option<usize> {
        let k = self.speaker.filter(|&s| self.settlers[s].alive && self.settlers[s].office.as_deref() == Some("Lord of the camp"))?;
        self.rooms.iter().position(|r| r.kind == super::delve::RoomKind::Bedroom && r.owner == Some(k))
    }

    /// Dawn: the lord demands a fine work of a material they love; unmet in thirty days, someone
    /// pays for it in the stocks, and the lord demands again.
    pub(crate) fn lord_demands(&mut self) {
        let day = self.clock.day();
        let Some(k) = self.speaker.filter(|&s| self.settlers[s].alive && self.settlers[s].office.as_deref() == Some("Lord of the camp")) else { return };
        if self.workshop_spot().is_none() { return; }
        let Some(lord) = self.lord.clone() else { return };
        let name = self.settlers[k].name.clone();
        match lord.demand {
            None => {
                if lord.came.map_or(true, |c| day < c + 10) { return; }
                // What they love, if the land gives it; else its stone.
                let liked = self.settlers[k].persona.likes.material.clone();
                let (stone, wood) = (self.land_stone(), self.land_wood());
                // What they love if the land gives it; else stone and wood by turns.
                let turns = self.works.iter().filter(|w| w.traded && w.quality >= 2 && w.day >= lord.came.unwrap_or(0)).count();
                let what = if liked == stone || liked == wood { liked } else if turns % 2 == 0 { stone } else { wood };
                self.note(format!("{} demands a fine work of {} for {} hall, within thirty days.", name, what, if self.settlers[k].persona.female { "her" } else { "his" }));
                if let Some(l) = self.lord.as_mut() { l.demand = Some((what, day, false)); }
            }
            Some((what, since, false)) if day >= since + 30 => {
                // Someone pays: the camp's best maker, in the stocks.
                let maker = (0..self.settlers.len()).filter(|&j| j != k && self.settlers[j].alive && self.settlers[j].guest_until == 0 && self.settlers[j].past.as_ref().map_or(true, |p| p.age >= 14))
                    .max_by(|&a, &b| self.settlers[a].skill[4].total_cmp(&self.settlers[b].skill[4]).then(b.cmp(&a)));
                if let Some(m) = maker {
                    let mn = self.settlers[m].name.clone();
                    self.stocks = Some((m, self.clock.tick + TICKS_PER_DAY));
                    self.feel(m, mind::Feel::Punished { by: name.clone() });
                    for j in 0..self.settlers.len() { if j != k && self.settlers[j].alive { self.like(k, j, -2); } }
                    let line = format!("Thirty days and no work of {}: {} has {} put in the stocks for it.", what, name, mn);
                    self.note(line.clone());
                    let at = self.settlers[m].pos;
                    self.moment(format!("{} is punished", mn), line, format!("because {} demanded a fine work of {} and none was made", name, what), at);
                }
                if let Some(l) = self.lord.as_mut() { l.demand = Some((what, day, false)); }
            }
            // Forty days after one is met, the lord wants something else.
            Some((_, since, true)) if day >= since + 40 => { if let Some(l) = self.lord.as_mut() { l.demand = None; } }
            _ => {}
        }
    }

    /// A work finished: does it meet the lord's demand?
    pub(crate) fn demand_met(&mut self, maker: usize, what: &str, quality: u8) {
        let Some((want, _, false)) = self.lord.as_ref().and_then(|l| l.demand.clone()) else { return };
        if quality < 2 || !what.contains(&want) { return; }
        let Some(k) = self.speaker.filter(|&s| self.settlers[s].alive && self.settlers[s].office.as_deref() == Some("Lord of the camp")) else { return };
        if let Some(l) = self.lord.as_mut() { l.demand = Some((want, self.clock.day(), true)); }
        if let Some(w) = self.works.last_mut() { w.traded = true; }
        let (mn, ln) = (self.settlers[maker].name.clone(), self.settlers[k].name.clone());
        self.note(format!("{} presents {} with {}; the lord is pleased.", mn, ln, what));
        self.like(k, maker, 6);
        self.feel(maker, mind::Feel::Admired { what: format!("the lord's pleasure in {}", what) });
    }
}
