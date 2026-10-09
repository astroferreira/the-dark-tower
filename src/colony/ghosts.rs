//! The restless dead: ghosts, and the slabs that lay them to rest.
//!
//! The idea is Dwarf Fortress's ghosts and memorials: the dead who die badly come back to
//! trouble the living until a memorial slab is engraved and set for them. Here a settler killed
//! by violence (a raid, a beast, the night, a fell mood) is restless (`bury` calls `mourn`).
//! Someone of the camp may carve a slab in their memory (`slab_option`: their closest friend
//! first, else whoever most wants to make things; `Job::Craft` "Carving a slab ...", one stored
//! stone, at the workshop if there is one, else by the fire):
//! it is set at the grave (a raised stone, "In memory of X, who died ...") and they rest. Five
//! days unremembered, the ghost walks: two nights in three at first, one in four after five, seen by someone, the
//! nearer to the dead the worse (`Feel::TheDeep`), until the slab is carved ("X's ghost is at
//! rest"). The first haunting is a moment.

use super::*;
use crate::persona::Facet;

#[derive(Clone, Debug)]
pub struct Restless {
    pub who: usize,
    pub died: u64,
    pub cause: String,
    pub at_rest: bool,
    /// Nights it has walked.
    pub hauntings: u32,
}

/// A death that leaves the dead restless.
fn violent(cause: &str) -> bool {
    ["raid", "killed", "drained", "fell mood", "slain", "beast"].iter().any(|w| cause.contains(w))
}

impl Colony {
    /// From `bury`: a bad death is remembered as restless.
    pub(crate) fn mourn(&mut self, i: usize, cause: &str) {
        if !violent(cause) || cause.contains("put to death") { return; }
        // The priest's rites may lay them to rest (`priest.rs`).
        if self.rites(i) { return; }
        self.restless.push(ghosts::Restless { who: i, died: self.clock.day(), cause: cause.to_string(), at_rest: false, hauntings: 0 });
    }

    /// Someone carves the slab: the dead's closest friend first, else the most willing hand.
    pub(crate) fn slab_option(&self, i: usize) -> Option<(f32, Job, String)> {
        if self.clock.is_night() { return None; }
        let r = self.restless.iter().find(|r| !r.at_rest)?;
        // (Debug: PLANET_FORCE_GHOST=1 lets no one carve for the first six days, for the test:
        // friends carve a slab within days, and a ghost had grown rare.)
        if std::env::var("PLANET_FORCE_GHOST").is_ok() && self.clock.day() < r.died + 6 { return None; }
        if self.settlers.iter().enumerate().any(|(j, s)| j != i && s.alive && s.job == Job::Craft && s.why.starts_with("Carving a slab")) { return None; }
        let friend = (0..self.settlers.len()).filter(|&j| self.settlers[j].alive && self.settlers[j].guest_until == 0 && self.settlers[j].past.as_ref().map_or(true, |p| p.age >= 12))
            .max_by_key(|&j| (self.opinion(r.who, j), std::cmp::Reverse(j)))?;
        if !self.items.iter().any(|it| it.stored && matches!(it.kind, ItemKind::Stone | ItemKind::Log)) {
            // Nothing laid by: the friend fetches it.
            if friend != i { return None; }
            let name = self.settlers[r.who].name.clone();
            let me = self.settlers[i].pos;
            if let Some(t) = self.nearest_tree_by(me, |_, _| true) { return Some((0.9, Job::Fell(t), format!("Felling a tree for a post in memory of {}", name))); }
            if let Some(t) = self.nearest(me, |c, p| c.is_quarry_stone(p)) { return Some((0.9, Job::Quarry(t), format!("Breaking stone for a slab in memory of {}", name))); }
            return None;
        }
        let wish = if friend == i { 1.2 } else { self.maker_wish(i) * 0.8 };
        if wish < 0.3 { return None; }
        let name = self.settlers[r.who].name.clone();
        Some((wish, Job::Craft, format!("Carving a slab in memory of {}, who died {}", name, r.cause)))
    }

    /// The slab is done: set at the grave, and the dead rest.
    pub(crate) fn finish_slab(&mut self, i: usize) {
        let Some(k) = self.restless.iter().position(|r| !r.at_rest) else { return };
        // A slab of stone, else a post of wood.
        let stone = self.items.iter().position(|it| it.stored && it.kind == ItemKind::Stone);
        let wood = stone.is_none();
        if let Some(s) = stone.or_else(|| self.items.iter().position(|it| it.stored && it.kind == ItemKind::Log)) { self.items.remove(s); self.fix_refs_pub(s); }
        let thing = if wood { "post" } else { "slab" };
        self.restless[k].at_rest = true;
        let (dead, cause, walked) = (self.restless[k].who, self.restless[k].cause.clone(), self.restless[k].hauntings);
        let (dn, cn) = (self.settlers[dead].name.clone(), self.settlers[i].name.clone());
        let grave = self.marks.iter().find(|m| m.kind == MarkKind::Grave && m.title == format!("The grave of {}", dn)).map(|m| m.at);
        let at = grave.map(|g| (g.0 + 1, g.1)).unwrap_or_else(|| self.spot_from_camp(-6, 7));
        let day = self.clock.day();
        self.marks.push(ColonyMark { at, kind: MarkKind::Stone, title: format!("The {} of {}", thing, dn),
            text: format!("In memory of {}, who died {}. Carved by {} on day {}.", dn, cause, cn, day), day });
        self.settlers[i].made.push(format!("a memorial {} for {} (day {})", thing, dn, day));
        if walked > 0 {
            self.note(format!("{} sets a {} carved in memory of {} by the grave, and {}'s ghost is at rest.", cn, thing, dn, dn));
        } else {
            self.note(format!("{} sets a {} carved in memory of {} by the grave.", cn, thing, dn));
        }
        for j in 0..self.settlers.len() { if self.settlers[j].alive && self.opinion(dead, j) >= 6 { self.feel(j, mind::Feel::Reconciled { by: format!("the slab for {}", dn) }); } }
    }

    /// 23:30: the unremembered walk.
    pub(crate) fn ghosts_walk(&mut self) {
        let day = self.clock.day();
        for k in 0..self.restless.len() {
            let r = self.restless[k].clone();
            if r.at_rest || day < r.died + 5 { continue; }
            // Most nights at first; after five, one night in four.
            let roll = crate::history::settlers::hash_pub(self.seed ^ day, 0x6057 + k as u64);
            if if r.hauntings < 5 { roll % 3 == 0 } else { roll % 4 != 0 } { continue; }
            // Seen by the one who was closest, else by anyone awake or not.
            let alive: Vec<usize> = (0..self.settlers.len()).filter(|&j| self.settlers[j].alive && self.settlers[j].guest_until == 0).collect();
            let Some(&seer) = alive.iter().max_by_key(|&&j| (self.opinion(r.who, j), (crate::history::settlers::hash_pub(day, j as u64) % 7) as i32)) else { continue };
            let (dn, sn) = (self.settlers[r.who].name.clone(), self.settlers[seer].name.clone());
            let place = ["by the well", "at the edge of the firelight", "in the door of the hut", "among the graves", "on the path to the woods"][(crate::history::settlers::hash_pub(day, 0x91 + k as u64) % 5) as usize];
            let line = format!("The ghost of {} is seen {} in the night; {} cannot sleep after.", dn, place, sn);
            self.note(line.clone());
            self.restless[k].hauntings += 1;
            if r.hauntings == 0 {
                let at = self.marks.iter().find(|m| m.kind == MarkKind::Grave && m.title == format!("The grave of {}", dn)).map(|m| m.at).unwrap_or(self.camp);
                self.moment(format!("The ghost of {}", dn), line, format!("because {} died {} and no one has carved a slab in their memory", dn, r.cause), at);
            }
            self.feel(seer, mind::Feel::TheDeep { what: format!("the ghost of {}", dn) });
            self.settlers[seer].fatigue = (self.settlers[seer].fatigue + 0.3).min(1.0);
            // The brave and the pious are less shaken; the others hear of it.
            for &j in &alive {
                if j != seer && self.settlers[j].persona.facet(Facet::Anxiety) >= 70 && day % 2 == 0 { self.feel(j, mind::Feel::News { what: format!("the ghost of {} walks", dn), good: false }); }
            }
        }
    }
}
