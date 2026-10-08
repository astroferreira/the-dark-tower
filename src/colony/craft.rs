//! Works of their hands: crafts that show what happened.
//!
//! The idea is Dwarf Fortress's items and engravings: a made thing has a material, a quality
//! from its maker's skill (ordinary to masterful) and, often, an image of a real event, so the
//! objects of a fortress tell its history. Here, once a workshop stands, settlers given to art or
//! craft spend spare hours there making figurines, carved stones, bowls and their people's
//! instruments, of the stone or wood the camp has stored. The quality comes from their hands
//! (the building skill, sure hands, creativity, perfectionism); the image from their own past (a
//! battle they fought in, the fall of their town, `Past::images`) or from the camp's own story
//! (the raid). A fine work makes its maker proud; a masterwork is a moment, and those who love
//! beautiful things admire it.

use super::*;
use crate::persona::{Attr, Facet, Val};

pub const QUALITY: [&str; 6] = ["", "well-crafted ", "finely-crafted ", "superior ", "exceptional ", "masterful "];

/// A made thing.
#[derive(Clone, Debug)]
pub struct Work {
    pub maker: usize,
    /// "figurine", "carved stone", "bowl", or an instrument ("the dulmgar (a set of pipes)").
    pub kind: String,
    pub material: String,
    /// 0 ordinary .. 5 masterful.
    pub quality: u8,
    /// What it shows, and the world event behind it (None for the camp's own story).
    pub image: Option<(String, Option<crate::history::EventId>)>,
    pub day: u64,
    /// An instrument's own name in the maker's tongue ("the bramou").
    pub called: Option<String>,
    /// Sold to a caravan (`trade.rs`).
    pub traded: bool,
}

impl Work {
    /// "a masterful granite figurine showing the siege of Ripu".
    pub fn describe(&self) -> String {
        let head = format!("{}{} {}", QUALITY[self.quality as usize], self.material, self.kind);
        let a = if head.starts_with(['a', 'e', 'i', 'o', 'u']) { "an" } else { "a" };
        match (&self.image, &self.called) {
            (_, Some(name)) => format!("{} {} called {}", a, head, name),
            (Some((img, _)), None) => format!("{} {} showing {}", a, head, img),
            (None, None) => format!("{} {}", a, head),
        }
    }
}

impl Colony {
    /// The rock under the camp (below its soil), by the name a crafter gives it ("flint").
    pub(crate) fn land_stone(&self) -> String {
        let (x, y) = (self.camp.0 as usize, self.camp.1 as usize);
        let sz = self.map.surface_z[y * self.map.width + x].max(0) as usize;
        (0..=sz).rev().find_map(|z| match self.map.cell(x, y, z).material { crate::local::Material::Rock(r) if r != crate::erosion::materials::RockType::Sediment => crate::persona::material_of_the_land(&format!("{:?}", r)), _ => None }).unwrap_or("stone").to_string()
    }

    /// The land's tree, by its wood's name ("oak").
    pub(crate) fn land_wood(&self) -> String {
        self.map.cells.iter().find_map(|c| if let crate::local::Plant::Tree(t) = c.plant { crate::persona::material_of_the_land(&format!("{:?}", t)) } else { None }).unwrap_or("wood").to_string()
    }

    /// Where crafts are made: inside the workshop, if one stands.
    pub(crate) fn workshop_spot(&self) -> Option<Pos> {
        // Inside by the open front, else just before it.
        let p = self.projects.iter().find(|p| p.done && p.kind == projects::ProjectKind::Workshop)?;
        // The benches below, once cut (`delve.rs`).
        if let Some(b) = self.rooms.iter().find(|r| r.kind == super::delve::RoomKind::Workshop).and_then(|r| r.bed) { return Some(b); }
        [(p.at.0 + 2, p.at.1 + 3), (p.at.0 + 2, p.at.1 + 4), (p.at.0 + 2, p.at.1 + 2)].into_iter().find(|&q| nav::passable(&self.map, q))
    }

    /// How much settler `i` wants to make something now (0 = not at all): art lovers, those who
    /// value craft, the creative. Needs a workshop and stored material to spare.
    pub(crate) fn craft_wish(&self, i: usize) -> f32 {
        // (Only what the work under way does not need: the carvers had used every stone the
        // quarriers brought, and a guildhall waited sixty days for its first.)
        let spare = self.spare(ItemKind::Stone) + self.spare(ItemKind::Log);
        if spare < 2 { return 0.0; }
        self.craft_wish_any(i)
    }

    /// The wish to make something, whether or not the material is laid by (when it is not, they
    /// gather it themselves: `decide`).
    pub(crate) fn craft_wish_any(&self, i: usize) -> f32 {
        if self.workshop_spot().is_none() { return 0.0; }
        // One at the workshop at a time (two when the speaker asks for works).
        let at_once = if self.mandate == Some(society::Mandate::Works) { 2 } else { 1 };
        // (Only those making works at the workshop: writers, brewers, engravers and the like
        // are elsewhere.)
        if self.settlers.iter().enumerate().filter(|(j, s)| *j != i && s.alive && s.job == Job::Craft && s.why.starts_with("Making something")).count() >= at_once { return 0.0; }
        self.maker_wish(i)
    }

    /// The wish to make things, from character (art, craftsmanship, creativity) and the
    /// mandates, whatever the workshop is doing: engravers, carvers of slabs and sewers work
    /// elsewhere (a workshop never empty had kept the hall's walls bare for 150 days).
    pub(crate) fn maker_wish(&self, i: usize) -> f32 {
        let p = &self.settlers[i].persona;
        let art = p.facet(Facet::ArtInclined) as f32 / 100.0;
        let craft = (p.value(Val::Craftsmanship) as f32 / 50.0).max(0.0);
        let creative = (p.attr(Attr::Creativity) / 1000.0 - 1.0).max(0.0);
        let mandate = match self.mandate { Some(society::Mandate::Works) => 1.6, Some(society::Mandate::NoIdleHands) => 1.3, _ => 1.0 }
            // The lord's demand stands (`nobles.rs`): every hand that can is set to it.
            * if self.lord.as_ref().map_or(false, |l| l.demand.as_ref().map_or(false, |d| !d.2)) { 1.5 } else { 1.0 };
        (0.15 + 0.45 * art + 0.25 * craft + 0.2 * creative).min(0.8) * if art + craft < 0.6 { 0.3 } else { 1.0 } * mandate
    }

    /// A work is finished: its material from the store, its quality from the maker's hands, its
    /// image from their past or the camp's story.
    pub(crate) fn finish_craft(&mut self, i: usize) {
        // The lord's demand chooses the material when it can (`nobles.rs`).
        let want = self.lord.as_ref().and_then(|l| l.demand.as_ref()).filter(|d| !d.2).map(|d| d.0.clone());
        let prefer = want.map(|w| if w == self.land_stone() { Some(ItemKind::Stone) } else if w == self.land_wood() { Some(ItemKind::Log) } else { None }).flatten();
        // (A material the work under way does not need first.)
        let free = |c: &Colony, k: ItemKind| c.spare(k) > 0;
        let Some(k) = prefer.filter(|&p| free(self, p)).and_then(|p| self.items.iter().position(|it| it.stored && it.kind == p))
            .or_else(|| self.items.iter().position(|it| it.stored && matches!(it.kind, ItemKind::Stone | ItemKind::Log) && free(self, it.kind)))
            .or_else(|| self.items.iter().position(|it| it.stored && matches!(it.kind, ItemKind::Stone | ItemKind::Log))) else { return };
        let kind_item = self.items[k].kind;
        self.items.remove(k);
        self.fix_refs_pub(k);
        let day = self.clock.day();
        let h = |salt: u64| crate::history::settlers::hash_pub(self.seed ^ self.clock.tick ^ (i as u64) << 20, salt);
        let material = match kind_item { ItemKind::Stone => self.land_stone(), _ => self.land_wood() };
        // The bones of a beast the camp slew, one work in two while they last: a famous
        // material, and the work shows its death.
        let beast = self.remains.iter().position(|r| r.2 > 0).filter(|_| h(9) % 2 == 0);
        let (material, beast_death) = match beast {
            Some(b) => { self.remains[b].2 -= 1; (format!("{}-bone", self.remains[b].1), Some(format!("the death of {}", self.remains[b].0))) }
            None => (material, None),
        };
        let s = &self.settlers[i];
        let p = &s.persona;
        // Quality: the building hand, sure hands, creativity, care, and a little luck.
        let q = 0.35 * s.skill[4] + 0.2 * (p.attr(Attr::KinestheticSense) / 2000.0).min(1.0) + 0.15 * (p.attr(Attr::Creativity) / 2000.0).min(1.0)
            + 0.15 * p.facet(Facet::Perfectionism) as f32 / 100.0 + 0.15 * (h(1) % 1000) as f32 / 1000.0;
        // Ordinary for most hands; masterful only for a master's on a good day.
        let quality = (((q - 0.35) * 9.0).floor() as i32).clamp(0, 5) as u8;
        // What it is: their people's instrument for the musical, else a carving.
        // "the bramou (a drum)" -> a drum called the bramou.
        let (kind, called) = match (&s.past.as_ref().and_then(|x| x.instrument.clone()), p.attr(Attr::Musicality) > 1200.0) {
            (Some(inst), true) if h(2) % 3 == 0 => {
                let (name, rest) = inst.split_once(" (").unwrap_or((inst.as_str(), "a drum)"));
                (rest.trim_end_matches(')').trim_start_matches("a set of ").trim_start_matches("a ").to_string(), Some(name.to_string()))
            }
            _ => (["figurine", "carved stone", "bowl", "plaque"][(h(3) % 4) as usize].to_string(), None),
        };
        let kind = if kind_item == ItemKind::Log && kind == "carved stone" { "carved post".to_string() } else { kind };
        // The image: a moment of their past, else the camp's own story.
        let past_images = s.past.as_ref().map(|x| x.images.clone()).unwrap_or_default();
        let camp_story: Vec<String> = self.arc.as_ref().map(|a| a.events.iter().filter(|e| e.title == "The raid").map(|e| format!("the raid of day {}", e.day)).collect()).unwrap_or_default();
        // The scene this maker has shown least (their past, then the camp's raids), so one hand
        // does not carve the same thing over and over; ties by the day's hash.
        let shown = |t: &str| self.works.iter().filter(|w| w.maker == i && w.image.as_ref().map_or(false, |x| x.0 == t)).count();
        let mut scenes: Vec<(String, Option<crate::history::EventId>)> = past_images.iter().map(|(t, e)| (t.clone(), Some(*e))).chain(camp_story.iter().map(|t| (t.clone(), None))).collect();
        let n = scenes.len().max(1) as u64;
        let r = (h(5) % n) as usize;
        if !scenes.is_empty() { scenes.rotate_left(r); }
        let image = if called.is_some() { None } else if let Some(d) = beast_death.clone() { Some((d, None)) } else { scenes.into_iter().min_by_key(|(t, _)| shown(t)) };
        // A gem set in it, if the camp has one: the perfectionist and the one who loves that
        // stone first. (As in DF, a decoration adds worth, not craftsmanship: `trade.rs`.)
        let (kind, quality) = match self.gems.iter().position(|g| g.1 > 0) {
            Some(k) if self.settlers[i].persona.facet(Facet::Perfectionism) >= 50 || self.settlers[i].persona.likes.material == self.gems[k].0 || h(6) % 3 == 0 => {
                self.gems[k].1 -= 1;
                (format!("{} set with {}", kind, self.gems[k].0), quality)
            }
            _ => (kind, quality),
        };
        let work = Work { maker: i, kind, material, quality, image, day, called, traded: false };
        // Crafting uses the building hand but does not train it (a carver is not a mason).
        self.settlers[i].mind.made += 1;
        let what = work.describe();
        let name = self.settlers[i].name.clone();
        self.settlers[i].made.push(format!("{} (day {})", what, day));
        self.works.push(work);
        self.feel(i, mind::Feel::Made { what: what.clone(), quality });
        self.demand_met(i, &what, quality);
        if quality >= 5 {
            let line = format!("{} makes a masterwork: {}.", name, what);
            self.note(line.clone());
            let pos = self.settlers[i].pos;
            self.moment(format!("A masterwork by {}", name), line, format!("because {} has the hands for it ({:.0}% of a master builder's) and cares for the work", name, self.settlers[i].skill[4] * 100.0), pos);
            for j in 0..self.settlers.len() {
                if j != i && self.settlers[j].alive && self.settlers[j].persona.facet(Facet::ArtInclined) >= 60 {
                    let w = what.clone();
                    self.feel(j, mind::Feel::Admired { what: w });
                }
            }
        } else if quality >= 3 || self.works.len() == 1 || what.contains(" set with ") {
            self.note(format!("{} finishes {} at the workshop.", name, what));
        }
    }
}
