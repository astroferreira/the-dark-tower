//! Engravings: the walls of the dug hall carved with the camp's story and the settlers' pasts.
//!
//! The idea is Dwarf Fortress's engravings: smoothed rock carved with images of the fortress's
//! own events and of the world's history, each with a quality and a maker. Here, once a hall
//! stands dug into the hill, one settler at a time who wants to make things (`craft_wish_any`)
//! may spend spare daylight hours engraving a bare wall beside the hall's floor (`Job::Craft`
//! with "Engraving"), no material needed. The image is the camp's greatest moment not yet
//! engraved (a beast slain, a relic found, a raid, an artifact, a guest who came, a madness, a
//! cavern breached), else a scene of the engraver's own past (`Past::images`, a real chronicle
//! event). Quality as for works (`craft.rs`). Engravings are drawn on the wall in ink and named
//! on hover (`engraving_at`), listed in the annals, and admired by the art-loving.

use super::*;
use crate::persona::{Attr, Facet};

#[derive(Clone, Debug)]
pub struct Engraving {
    /// The wall cell carved, and the floor cell it faces.
    pub wall: Pos,
    pub from: Pos,
    /// The level of the floor it faces (the hall's, a bedroom's...).
    pub z: i32,
    /// "the death of Bornith the forgotten beast (day 52)", "the siege of Brolmdustoor".
    pub image: String,
    pub event: Option<crate::history::EventId>,
    pub quality: u8,
    pub maker: usize,
    pub day: u64,
}

/// The camp's moments worth engraving, greatest first.
fn worth(title: &str, text: &str) -> u32 {
    if title.starts_with("The death of") { 10 }
    else if text.contains("has made an artifact") { 9 }
    else if title.ends_with(" is found") || title.starts_with("The hoard of") { 8 }
    else if title == "The raid" { 7 }
    else if title.starts_with("They break into") { 6 }
    else if title.ends_with(" comes") || title.ends_with(" stays") { 5 }
    else if title.ends_with(" goes mad") || title.ends_with(" is cast out") { 4 }
    // What came later (sieges, risings, vows, thieves, the call to war) and the camp's own life.
    else if title.ends_with(" is deposed") || title.ends_with(" keeps the vow") || title.contains(" falls in ") { 9 }
    else if title == "The siege" || title == "The sally" { 7 }
    else if title.ends_with(" is taken") || title == "The siege lifts" || title.ends_with(" is stolen") { 6 }
    else if title.ends_with(" comes home") || title.starts_with("Called to ") || title.ends_with(" swear vengeance") || title == "A thief is caught" || title == "The armour holds" { 5 }
    else if title == "The wolves' den" || title.starts_with("The burning of") || title.starts_with("The friendship of") || title.ends_with(" is founded") || title.ends_with("'s dream") { 4 }
    else if title.starts_with("The wedding") || title.ends_with(" is born") || title.contains(" festival") { 3 }
    else { 0 }
}

impl Colony {
    /// Bare walls to engrave, in order: the hall's, then the great hall's, the owned bedrooms',
    /// the tombs' (DF engraves any smoothed wall): (the floor cell to stand on, the wall, the
    /// level, where it is in words).
    fn bare_walls(&self) -> Vec<(Pos, Pos, i32, String)> {
        use super::delve::RoomKind;
        let mut places: Vec<(Vec<Pos>, i32, String)> = Vec::new();
        // The lord's room first (`nobles.rs`).
        if let Some(r) = self.lords_room() { places.push((self.rooms[r].cells.clone(), self.rooms[r].z, "the walls of the lord's room".into())); }
        if !self.hall_cells.is_empty() { places.push((self.hall_cells.clone(), self.hall_z, "the hall's wall".into())); }
        for r in self.rooms.iter().filter(|r| r.kind == RoomKind::GreatHall) { places.push((r.cells.clone(), r.z, "the great hall's wall".into())); }
        for r in self.rooms.iter().filter(|r| r.kind == RoomKind::Bedroom && r.owner.is_some()) {
            let o = r.owner.unwrap();
            if !self.settlers[o].alive { continue; }
            places.push((r.cells.clone(), r.z, format!("the wall of {}'s room", self.settlers[o].name)));
        }
        for r in self.rooms.iter().filter(|r| r.kind == RoomKind::Tomb && r.owner.is_some()) { places.push((r.cells.clone(), r.z, "a wall of the tombs".into())); }
        let mut out = Vec::new();
        for (cells, z, word) in places {
            for &h in &cells {
                for (dx, dy) in [(0i32, -1i32), (1, 0), (0, 1), (-1, 0)] {
                    let (x, y) = (h.0 as i32 + dx, h.1 as i32 + dy);
                    if x < 0 || y < 0 || x as usize >= self.map.width || y as usize >= self.map.height { continue; }
                    let wall = (x as u16, y as u16);
                    if cells.contains(&wall) { continue; }
                    // Rock at the room's headroom: a wall face, not open ground.
                    let fz = z.max(0) as usize;
                    if fz + 1 >= self.map.depth || self.map.cell(x as usize, y as usize, fz + 1).shape != Shape::Wall { continue; }
                    if self.engravings.iter().any(|e| e.wall == wall && e.z == z) || out.iter().any(|o: &(Pos, Pos, i32, String)| o.1 == wall && o.2 == z) { continue; }
                    out.push((h, wall, z, word.clone()));
                }
            }
            if !out.is_empty() { break; }
        }
        out
    }

    /// The next wall to engrave, if any.
    pub(crate) fn engrave_spot(&self) -> Option<Pos> { self.bare_walls().first().map(|w| w.0) }

    /// Where the next engraving goes, in words ("the hall's wall", "the wall of X's room").
    pub(crate) fn engrave_place(&self) -> String { self.bare_walls().first().map(|w| w.3.clone()).unwrap_or_else(|| "the hall's wall".into()) }

    /// Spare hours carving the hall's walls, for those who want to make things.
    pub(crate) fn engrave_option(&self, i: usize) -> Option<(f32, Job, String)> {
        if self.clock.is_night() { return None; }
        if self.settlers.iter().enumerate().any(|(j, s)| j != i && s.alive && s.job == Job::Craft && s.why.starts_with("Engraving")) { return None; }
        let wish = self.craft_wish_any(i);
        let builder = self.settlers[i].role == Some(4);
        if wish < 0.3 && !builder { return None; }
        // (The walls are listed once here: `engrave_place` and `engrave_spot` list them each.)
        let walls = self.bare_walls();
        let place = walls.first().map(|w| w.3.clone()).unwrap_or_else(|| "the hall's wall".into());
        // The lord's order: the camp's builder carves the lord's walls, taste or none (`nobles.rs`).
        let ordered = builder && place == "the walls of the lord's room";
        let wish = if ordered { wish.max(0.6) } else { wish };
        if wish < 0.3 { return None; }
        walls.first()?;
        let (image, _) = self.engraving_image(i);
        Some((wish * 0.9, Job::Craft, format!("Engraving {} with {}", place, image)))
    }

    /// What settler `i` would engrave: the camp's greatest moment not yet on a wall, else a
    /// scene of their own past.
    fn engraving_image(&self, i: usize) -> (String, Option<crate::history::EventId>) {
        let carved: crate::history::det::FastSet<&str> = self.engravings.iter().map(|e| e.image.as_str()).collect();
        let done = |s: &str| carved.contains(s);
        // The greatest moment not yet carved, the earliest of equals (`max_by_key` over
        // (worth, Reverse(tick)) keeps the last of equal maxima): a moment that cannot beat the
        // best so far is not phrased at all.
        let mut best: Option<(u32, u64, String)> = None;
        for m in &self.moments {
            let w = worth(&m.title, &m.text);
            if w == 0 { continue; }
            if best.as_ref().map_or(false, |b| (w, std::cmp::Reverse(m.tick)) < (b.0, std::cmp::Reverse(b.1))) { continue; }
            let t = &m.title;
            let day = m.tick / TICKS_PER_DAY + 1;
            let scene = if t == "The raid" { format!("the raid of day {}", day) }
                else if m.text.contains("has made an artifact") { format!("the making of {}", t) }
                else if let Some(x) = t.strip_suffix(" comes") { format!("the coming of {}", x) }
                else if let Some(x) = t.strip_suffix(" stays") { format!("{} taking a place at the fire", x) }
                else if let Some(x) = t.strip_suffix(" goes mad") { format!("the madness of {}", x) }
                else if let Some(x) = t.strip_suffix(" is cast out") { format!("the casting out of {}", x) }
                else if let Some(x) = t.strip_suffix(" is found") { format!("the finding of {}", x) }
                else if let Some(x) = t.strip_prefix("They break into ") { format!("the breaking into {}", x) }
                else if let Some(x) = t.strip_suffix(" is founded") { format!("the founding of {}", x.replacen("The ", "the ", 1)) }
                else if let Some(x) = t.strip_suffix(" is born") { format!("the birth of {}", x) }
                else if let Some(x) = t.strip_suffix(" comes home") { format!("the homecoming of {}", x) }
                else if let Some(x) = t.strip_suffix(" is taken") { format!("the taking of {}", x) }
                else if let Some(x) = t.strip_suffix(" is stolen") { format!("the theft of {}", x) }
                else if let Some(x) = t.strip_suffix(" is deposed") { format!("the fall of {}", x) }
                else if let Some(x) = t.strip_suffix(" keeps the vow") { format!("{} keeping the vow", x) }
                else if let Some(x) = t.strip_suffix(" swear vengeance") { format!("the vengeance sworn by {}", x) }
                else if let Some(x) = t.strip_suffix("'s dream") { format!("the dream of {} come true", x) }
                else if let Some(x) = t.strip_prefix("The ") { format!("the {}", x) }
                else { t.clone() };
            let phrase = if t == "The raid" || m.text.contains("has made an artifact") { scene } else { format!("{} (day {})", scene, day) };
            if !done(&phrase) { best = Some((w, m.tick, phrase)); }
        }
        if let Some((_, _, p)) = best { return (p, None); }
        let images = self.settlers[i].past.as_ref().map(|p| p.images.clone()).unwrap_or_default();
        let fresh: Vec<_> = images.into_iter().filter(|(t, _)| !done(t)).collect();
        if !fresh.is_empty() {
            let (t, e) = fresh[(crate::history::settlers::hash_pub(self.seed ^ self.clock.day(), i as u64) % fresh.len() as u64) as usize].clone();
            return (t, Some(e));
        }
        // What the others remember, then the camp by its fire (once).
        if let Some((t, e)) = self.settlers.iter().filter_map(|s| s.past.as_ref()).flat_map(|p| p.images.iter()).find(|(t, _)| !done(t)) {
            return (t.clone(), Some(*e));
        }
        if !done("the camp by its fire") { return ("the camp by its fire".into(), None); }
        (format!("the camp in its {} year", ["first", "second", "third", "fourth", "fifth"][((self.clock.day() - 1) / ageing::YEAR_DAYS).min(4) as usize]), None)
    }

    /// The carving is done.
    pub(crate) fn finish_engraving(&mut self, i: usize) {
        let Some((from, wall, z, place)) = self.bare_walls().first().cloned() else { return };
        let (image, event) = self.engraving_image(i);
        let s = &self.settlers[i];
        let p = &s.persona;
        let h = crate::history::settlers::hash_pub(self.seed ^ self.clock.tick, 0xE46A + i as u64);
        let q = 0.35 * s.skill[4] + 0.2 * (p.attr(Attr::KinestheticSense) / 2000.0).min(1.0) + 0.15 * (p.attr(Attr::Creativity) / 2000.0).min(1.0)
            + 0.15 * p.facet(Facet::Perfectionism) as f32 / 100.0 + 0.15 * (h % 1000) as f32 / 1000.0;
        let quality = (((q - 0.35) * 9.0).floor() as i32).clamp(0, 5) as u8;
        let day = self.clock.day();
        let name = s.name.clone();
        self.engravings.push(engrave::Engraving { wall, from, z, image: image.clone(), event, quality, maker: i, day });
        let what = format!("{}image of {}", craft::QUALITY[quality as usize], image);
        self.settlers[i].made.push(format!("an engraving: {} (day {})", what, day));
        self.feel(i, mind::Feel::Made { what: format!("an engraving of {}", image), quality });
        if self.engravings.len() == 1 || quality >= 3 {
            self.note(format!("{} engraves {} with {} {}.", name, place, if what.starts_with(|c: char| "aeiou".contains(c)) { "an" } else { "a" }, what));
        }
        if quality >= 4 {
            for j in 0..self.settlers.len() {
                if j != i && self.settlers[j].alive && self.settlers[j].persona.facet(Facet::ArtInclined) >= 60 { self.feel(j, mind::Feel::Admired { what: format!("the engraving of {}", image) }); }
            }
        }
    }

    /// The engraving on wall cell `p`, in words, for hover.
    pub fn engraving_at(&self, p: Pos) -> Option<String> {
        let e = self.engravings.iter().find(|e| e.wall == p)?;
        let maker = self.settlers.get(e.maker).map(|s| s.name.clone()).unwrap_or_default();
        Some(format!("An engraving: {}image of {}, by {}, day {}", craft::QUALITY[e.quality as usize], e.image, maker, e.day))
    }
}
