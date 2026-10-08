//! Places in the hills: the old works and graves of the history, found by those who wander.
//!
//! The idea is Dwarf Fortress's discoveries on the map (caves, lairs, old mines, tombs with their
//! dead), each with what the history left in it. The embark's places (`local::places`: carved at
//! generation from the lore, each with a mouth on the surface) are found by whoever passes within
//! three cells of a mouth (checked every ten minutes); the curious (curiosity 65+) roam the hills
//! by day looking (`explore_option`, a spot 30-80 cells out, hashed by the day). Found, by kind:
//! an old mine gives up the ore its town left (the camp has that ore, `Colony::ores`); a lair of
//! a dead beast gives up its hoard (treasures), a living one's is backed away from; a tomb is left
//! sealed by a finder who holds tradition dear or is pious, else robbed of the dead's arms (a
//! treasure), and the dead may rise for them that night (always under the Shadow, else one time
//! in three: a hunter named for them, as in `dead.rs`); a cave may show a vein of gems in its
//! wall. Each find is a moment.

use super::*;
use super::creatures::{Creature, CreatureKind};
use crate::local::places::PlaceKind;
use crate::persona::{Facet, Val};

impl Colony {
    /// The curious roam the hills by day.
    pub(crate) fn explore_option(&self, i: usize) -> Option<(f32, Job, String)> {
        if self.clock.is_night() { return None; }
        if !self.map.places.iter().enumerate().any(|(k, p)| p.mouth.is_some() && !self.places_found.contains(&k)) { return None; }
        let p = &self.settlers[i].persona;
        if p.facet(Facet::Curiosity) < 65 || self.settlers[i].past.as_ref().map_or(false, |x| x.age < 14) { return None; }
        let h = crate::history::settlers::hash_pub(self.seed ^ self.clock.day(), 0xE9F0 + i as u64);
        // One roam in three follows old tracks toward a place not yet found (within ten cells
        // of its mouth); the rest go where the day takes them.
        // (Only places that can be walked to.)
        let camp = self.camp;
        let unfound: Vec<Pos> = self.map.places.iter().enumerate().filter(|(k, _)| !self.places_found.contains(k)).filter_map(|(_, p)| p.mouth)
            .filter(|m| self.reachable_from(camp, *m)).collect();
        let spot = if h % 3 == 0 && !unfound.is_empty() {
            let m = unfound[(h / 3) as usize % unfound.len()];
            let (dx, dy) = (((h >> 8) % 21) as i32 - 10, ((h >> 16) % 21) as i32 - 10);
            self.passable_near_pub((m.0 as i32 + dx, m.1 as i32 + dy))?
        } else {
            let a = (h % 628) as f32 / 100.0;
            let d = 30.0 + ((h >> 12) % 50) as f32;
            self.passable_near_pub(((self.camp.0 as f32 + a.cos() * d) as i32, (self.camp.1 as f32 + a.sin() * d) as i32))?
        };
        Some((0.15 + 0.2 * (p.facet(Facet::Curiosity) as f32 - 65.0) / 35.0, Job::Wander(spot), "Roaming the hills to see what is there".into()))
    }

    /// Every ten minutes: anyone near an unfound mouth finds the place.
    pub(crate) fn explore_tick(&mut self) {
        if self.clock.minute() % 10 != 0 { return; }
        for k in 0..self.map.places.len() {
            if self.places_found.contains(&k) { continue; }
            let Some(m) = self.map.places[k].mouth else { continue };
            let finder = (0..self.settlers.len()).find(|&i| self.settlers[i].alive
                && (self.settlers[i].pos.0 as i32 - m.0 as i32).abs().max((self.settlers[i].pos.1 as i32 - m.1 as i32).abs()) <= 3);
            if let Some(i) = finder { self.find_place(i, k); }
        }
    }

    fn find_place(&mut self, i: usize, k: usize) {
        self.places_found.push(k);
        let p = self.map.places[k].clone();
        let name = self.settlers[i].name.clone();
        let at = p.mouth.unwrap_or(self.camp);
        let day = self.clock.day();
        let lower = { let mut c = p.name.chars(); c.next().map(|f| f.to_lowercase().collect::<String>() + c.as_str()).unwrap_or_default() };
        match p.kind {
            PlaceKind::OldMine => {
                let ore = p.name.split_whitespace().nth(2).filter(|w| w.chars().next().map_or(false, |c| c.is_uppercase()) && !w.starts_with("mine")).map(|w| w.to_lowercase());
                let line = match &ore {
                    Some(o) => {
                        if !self.ores.contains(o) { self.ores.push(o.clone()); }
                        self.ore_found += 1;
                        format!("{} finds {} in the hills: {}. They bring out the {} left behind.", name, lower, p.cause, o)
                    }
                    None => format!("{} finds {} in the hills: {}. {}.", name, lower, p.cause, super::arc::capital_word(&p.contents.first().cloned().unwrap_or_default())),
                };
                self.note(line.clone());
                self.moment(p.name.clone(), line, format!("because {}", p.cause), at);
            }
            PlaceKind::Lair => {
                let alive = p.cause.ends_with("and lives");
                let line = if alive {
                    format!("{} finds {} in the hills and backs away from its mouth: {}.", name, lower, p.cause)
                } else {
                    let hoard: Vec<String> = p.contents.iter().filter(|c| c.ends_with(", in its hoard")).map(|c| c.trim_end_matches(", in its hoard").to_string()).collect();
                    for h in &hoard { self.treasures.push(format!("{}, from {}", h, lower)); }
                    if hoard.is_empty() { format!("{} finds {}, long empty: {}.", name, lower, p.cause) }
                    else { format!("{} finds {}: {}. In the dark lies its hoard: {}. They carry it home.", name, lower, p.cause, crate::persona::list(&hoard)) }
                };
                self.note(line.clone());
                self.moment(p.name.clone(), line, format!("because {}", p.cause), at);
                if alive { self.feel(i, mind::Feel::TheDeep { what: lower.clone() }); }
            }
            PlaceKind::Tomb => {
                let pers = &self.settlers[i].persona;
                let respect = pers.value(Val::Tradition) >= 10 || pers.facet(Facet::Piety) >= 60;
                let dead = p.contents.first().map(|c| c.split(" in a stone chamber").next().unwrap_or(c).to_string()).unwrap_or_else(|| "the dead".into());
                if respect {
                    let line = format!("{} finds {} in the hills ({}), and leaves it sealed.", name, lower, p.cause);
                    self.note(line.clone());
                    self.moment(p.name.clone(), line, format!("because {} holds the dead's rest dear", name), at);
                } else {
                    self.treasures.push(format!("the arms of {}, from {}", dead, lower));
                    let line = format!("{} breaks into {} ({}) and carries off the arms {} was buried with.", name, lower, p.cause, dead);
                    self.note(line.clone());
                    self.moment(p.name.clone(), line, format!("because {} cares more for good iron than for the dead", name), at);
                    // The dead may come for them.
                    if self.darkness >= 0.2 || crate::history::settlers::hash_pub(self.seed, 0x70B + k as u64) % 3 == 0 { self.tomb_risen = Some((dead, day)); }
                }
            }
            PlaceKind::Cave => {
                let gem = ["rock crystal", "amber", "jet", "garnets"][(crate::history::settlers::hash_pub(self.seed, 0xCA7E + k as u64) % 4) as usize];
                let vein = crate::history::settlers::hash_pub(self.seed, 0xCA7F + k as u64) % 2 == 0;
                if vein { match self.gems.iter_mut().find(|x| x.0 == gem) { Some(x) => x.1 += 2, None => self.gems.push((gem.to_string(), 2)) } }
                let line = format!("{} finds {}: {}{}.", name, lower, p.contents.first().cloned().unwrap_or_default(), if vein { format!(", and a vein of {} in its wall", gem) } else { String::new() });
                self.note(line.clone());
                self.moment(p.name.clone(), line, format!("because {}", p.cause), at);
            }
            PlaceKind::Cavern => {}
            PlaceKind::Halls => {
                // A fallen town's halls give up what its people left: an heirloom and a few
                // stones; a living town's are its people's own.
                let fallen = p.cause.contains("since it fell") || p.cause.contains("until it fell");
                let line = if fallen {
                    let town = p.name.trim_start_matches("The halls of ").to_string();
                    let gem = ["rock crystal", "garnets", "amber", "jet"][(crate::history::settlers::hash_pub(self.seed, 0x4A12 + k as u64) % 4) as usize];
                    match self.gems.iter_mut().find(|x| x.0 == gem) { Some(x) => x.1 += 3, None => self.gems.push((gem.to_string(), 3)) }
                    self.treasures.push(format!("a carved stone chest from the halls of {}", town));
                    self.reclaim_halls(k, &town);
                    format!("{} goes down the stair into {} ({}): {}. In the deep chamber lies a carved stone chest, and {} in a cup on the forge.", name, lower, p.cause, super::arc::capital_word(&p.contents.first().cloned().unwrap_or_default()), gem)
                } else {
                    format!("{} walks down into {}, {}: its people let them see the great hall.", name, lower, p.cause)
                };
                self.note(line.clone());
                self.moment(p.name.clone(), line, format!("because {}", p.cause), at);
            }
        }
        self.feel(i, mind::Feel::Found { what: lower });
    }

    /// 21:00 after a tomb was robbed: its dead rise and come for their arms.
    pub(crate) fn tomb_dead_rise(&mut self) {
        let Some((dead, day)) = self.tomb_risen.clone() else { return };
        if self.clock.day() != day { self.tomb_risen = None; return; }
        self.tomb_risen = None;
        let Some(k) = self.map.places.iter().position(|p| p.kind == PlaceKind::Tomb) else { return };
        let Some(m) = self.map.places[k].mouth else { return };
        let Some(at) = self.passable_near_pub((m.0 as i32, m.1 as i32)) else { return };
        let id = self.new_creature_id();
        self.creatures.push(Creature { kind: CreatureKind::Wolf, name: format!("{}, risen", dead), pos: at, path: Vec::new(), stride: 0, leaving: false, size: 1.1, home: at, spawned: self.clock.tick, id, z: None, path3: Vec::new(), home_z: 0, out: false, rest_until: 0 });
        let line = format!("At nightfall something walks out of the robbed tomb: {}, risen, comes for the arms taken from it.", dead);
        self.note(line.clone());
        self.moment(format!("{} rises", dead), line, "because the tomb was robbed".into(), at);
    }
}
