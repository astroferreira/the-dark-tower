//! Pets: a settler who loves a creature tames one of the herd and keeps it.
//!
//! The idea is Dwarf Fortress's pets: a dwarf who likes a kind of animal adopts one, it follows
//! them about, and its death is a grief like a friend's. Here, at dawn, a settler fond of a herd
//! that grazes within 30 cells of the camp (`fond_of`, the persona's liked creature) may coax a
//! young one to the fire (one dawn in eight, one pet each, not a guest) and name it in their own
//! tongue. The animal becomes `CreatureKind::Pet` (the hunters leave it be): it follows its
//! keeper at a distance, ambling after them; each dawn it is a comfort (`Feel::Pet`). Wolves out
//! at night take a pet that strays alone far from the fire (its keeper grieves as for a close
//! friend); when its keeper dies or leaves, it wanders back to its herd.

use super::*;
use super::creatures::CreatureKind;

#[derive(Clone, Debug)]
pub struct Pet {
    pub id: u32,
    pub keeper: usize,
    /// "red deer".
    pub kind: String,
    /// Its name in the keeper's tongue.
    pub name: String,
    pub since: u64,
    pub alive: bool,
}

impl Colony {
    /// Dawn: the fond tame one; the pets comfort their keepers; a keeperless pet goes wild.
    pub(crate) fn reckon_pets(&mut self) {
        let day = self.clock.day();
        // Keepers gone: the animal goes back to the herd.
        for k in 0..self.pets.len() {
            if !self.pets[k].alive { continue; }
            let keeper = self.pets[k].keeper;
            if self.settlers[keeper].alive || self.settlers[keeper].away_until > 0 { continue; }
            self.pets[k].alive = false;
            let id = self.pets[k].id;
            if let Some(c) = self.creatures.iter_mut().find(|c| c.id == id) { c.kind = CreatureKind::Game; c.name = self.pets[k].kind.clone(); }
            self.note(format!("{}, who was {}'s {}, wanders back to the herd.", self.pets[k].name, self.settlers[keeper].name, self.pets[k].kind));
        }
        // Comfort.
        for k in 0..self.pets.len() {
            if self.pets[k].alive { let (i, n) = (self.pets[k].keeper, self.pets[k].name.clone()); self.feel(i, mind::Feel::Pet { name: n }); }
        }
        // Taming.
        for i in 0..self.settlers.len() {
            let s = &self.settlers[i];
            if !s.alive || s.guest_until > 0 || s.mind.broken.is_some() || self.pets.iter().any(|p| p.alive && p.keeper == i) { continue; }
            if crate::history::settlers::hash_pub(self.seed ^ day, 0x9E7 + i as u64) % 8 != 0 { continue; }
            let camp = self.camp;
            let Some(c) = self.creatures.iter().position(|c| c.kind == CreatureKind::Game && fond_of(&s.persona, &c.name)
                && (c.pos.0 as i32 - camp.0 as i32).abs().max((c.pos.1 as i32 - camp.1 as i32).abs()) <= 30) else { continue };
            let kind = self.creatures[c].name.clone();
            let name = self.pet_name(i);
            let keeper = s.name.clone();
            self.creatures[c].kind = CreatureKind::Pet;
            self.creatures[c].name = format!("{}, {}'s {}", name, keeper, kind);
            self.creatures[c].path.clear();
            let id = self.creatures[c].id;
            self.pets.push(pets::Pet { id, keeper: i, kind: kind.clone(), name: name.clone(), since: day, alive: true });
            let line = format!("{} coaxes a young {} to the fire with salt and bread, and names it {}.", keeper, kind, name);
            self.note(line);
            self.feel(i, mind::Feel::Pet { name });
        }
    }

    /// A name in the keeper's tongue: the first word of an artifact-style name.
    fn pet_name(&self, i: usize) -> String {
        use rand::SeedableRng;
        let p = &self.settlers[i].persona;
        let arche = crate::history::entities::races::RaceType::from_tag(&p.race).default_naming_archetype();
        let style = crate::history::naming::styles::NamingStyle::from_archetype(crate::history::NamingStyleId(0), arche);
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(self.seed ^ 0x9E7_9E7 ^ (i as u64) << 8 ^ self.clock.day());
        let full = crate::history::naming::generator::NameGenerator::personal_name(&style, &mut rng);
        full.split_whitespace().next().unwrap_or("Pip").to_string()
    }

    /// Every 15 minutes: a pet ambles after its keeper when it has fallen behind.
    pub(crate) fn pets_follow(&mut self) {
        for k in 0..self.pets.len() {
            if !self.pets[k].alive { continue; }
            let (id, keeper) = (self.pets[k].id, self.pets[k].keeper);
            let Some(c) = self.creatures.iter().position(|c| c.id == id) else { self.pets[k].alive = false; continue };
            if !self.creatures[c].path.is_empty() { continue; }
            let (from, to) = (self.creatures[c].pos, self.settlers[keeper].pos);
            if (from.0 as i32 - to.0 as i32).abs().max((from.1 as i32 - to.1 as i32).abs()) <= 3 { continue; }
            let near = ((to.0 as i32 + 2), (to.1 as i32 + 1));
            if let Some(t) = self.passable_near_pub(near) {
                self.creatures[c].path = nav::path(&self.map, from, t, 1500).map(|p| p.into_iter().skip(1).collect()).unwrap_or_default();
            }
        }
    }

    /// Wolves take a pet alone, far from the fire (called with the pack's place and reach).
    pub(crate) fn wolves_take_pet(&mut self, wolf: Pos) -> bool {
        let camp = self.camp;
        let found = self.pets.iter().position(|p| p.alive && self.creatures.iter().any(|c| c.id == p.id && {
            let far = (c.pos.0 as i32 - camp.0 as i32).abs().max((c.pos.1 as i32 - camp.1 as i32).abs()) > 7;
            let near_wolf = (c.pos.0 as i32 - wolf.0 as i32).abs().max((c.pos.1 as i32 - wolf.1 as i32).abs()) <= 1;
            let alone = !self.settlers.iter().any(|s| s.alive && (s.pos.0 as i32 - c.pos.0 as i32).abs().max((s.pos.1 as i32 - c.pos.1 as i32).abs()) <= 4);
            far && near_wolf && alone
        }));
        let Some(k) = found else { return false };
        self.pets[k].alive = false;
        let id = self.pets[k].id;
        self.creatures.retain(|c| c.id != id);
        let (name, kind, keeper) = (self.pets[k].name.clone(), self.pets[k].kind.clone(), self.pets[k].keeper);
        let kn = self.settlers[keeper].name.clone();
        let line = format!("Wolves take {}, {}'s {}, at the edge of the woods.", name, kn, kind);
        self.note(line);
        self.feel(keeper, mind::Feel::Death { whom: format!("{}, {} {}", name, if self.settlers[keeper].persona.female { "her" } else { "his" }, kind), close: true });
        true
    }

    /// A hunter of the night reaches settler `i`: their pet, within twelve cells (it follows them at a distance, and comes running), stands over
    /// them (DF: animals defend their owners). A big beast (elk, bison, boar, bear...) drives the
    /// hunters off two times in three, a small one one time in three; failing, one time in three
    /// the hunters take the pet instead. Returns whether the keeper was spared.
    pub(crate) fn pet_defends(&mut self, i: usize, hunter: &str) -> bool {
        let me = self.settlers[i].pos;
        let Some(k) = self.pets.iter().position(|p| p.alive && p.keeper == i && self.creatures.iter().any(|c| c.id == p.id
            && (c.pos.0 as i32 - me.0 as i32).abs().max((c.pos.1 as i32 - me.1 as i32).abs()) <= 12)) else { return false };
        let (name, kind, id) = (self.pets[k].name.clone(), self.pets[k].kind.clone(), self.pets[k].id);
        let big = ["elk", "moose", "bison", "boar", "bear", "aurochs", "yak", "caribou", "ox", "horse", "camel", "mammoth"].iter().any(|b| kind.contains(b));
        let roll = crate::history::settlers::hash_pub(self.seed ^ self.clock.tick, 0x9E7 + i as u64) % 3;
        let kn = self.settlers[i].name.clone();
        let her = if self.settlers[i].persona.female { "her" } else { "him" };
        let hunters = if hunter == "a wolf" { "the wolves".to_string() } else { hunter.to_string() };
        if roll < if big { 2 } else { 1 } {
            self.note(format!("{}, {}'s {}, comes running, stands over {} and lashes out at {} until they slink back into the dark.", name, kn, kind, her, hunters));
            self.feel(i, mind::Feel::SavedBy { whom: format!("{}, {} {}", name, if self.settlers[i].persona.female { "her" } else { "his" }, kind) });
            return true;
        }
        if roll == 2 {
            self.pets[k].alive = false;
            self.creatures.retain(|c| c.id != id);
            self.note(format!("{}, {}'s {}, comes running between {} and {}, and is dragged down in {} place.", name, kn, kind, her, hunters, if self.settlers[i].persona.female { "her" } else { "his" }));
            self.feel(i, mind::Feel::Death { whom: format!("{}, {} {}", name, if self.settlers[i].persona.female { "her" } else { "his" }, kind), close: true });
            return true;
        }
        false
    }

    /// Where pets can be found for wolves to stalk: positions of living pets far from the fire.
    pub(crate) fn stray_pets(&self) -> Vec<Pos> {
        let camp = self.camp;
        self.pets.iter().filter(|p| p.alive).filter_map(|p| self.creatures.iter().find(|c| c.id == p.id)).map(|c| c.pos)
            .filter(|q| (q.0 as i32 - camp.0 as i32).abs().max((q.1 as i32 - camp.1 as i32).abs()) > 7).collect()
    }

    /// "Keeps a red deer named Bru (since day 40)", for the settler's page and the annals.
    pub fn pet_of(&self, i: usize) -> Option<String> {
        let p = self.pets.iter().rev().find(|p| p.keeper == i)?;
        Some(if p.alive { format!("keeps a {} named {} (since day {})", p.kind, p.name, p.since) } else { format!("kept a {} named {}", p.kind, p.name) })
    }
}
