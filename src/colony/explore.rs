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
//!
//! The unknown is blank (card 'The unknown is blank, and someone has to go in', 2026-10-12): a
//! place is not found by passing its mouth any more. Passing within three cells shows the way in
//! ("Ghuug comes on a door of fitted stone in the hillside at 60,120: no one knows what lies
//! behind it"), and on the second evening the eldest tells what the old folk said of one place in
//! these hills (`tell_of_places`). Someone has to go in: the bold (nerve from bravery, a
//! veteran's past, curiosity, less anxiety; never a child, a guest, the ill, nor a child of a
//! burned town), by day, one at a time (`go_in_option`; the why sets nerve against fear). The
//! cells they walk appear in the level view (`revealed`); the find is made at the far end. Some
//! do not come back (by the place: a living beast's den, a tomb, rotten props, a fall in the
//! dark): at dusk the camp misses them, and at dawn it decides whether to send others after them
//! (`reckon_lost`): found hurt, found dead, or never found. The patron's hand: a blessed way in
//! draws the bold sooner (x3, and the less bold), a forbidden one keeps everyone out, and they
//! say so.

use super::*;
use super::creatures::{Creature, CreatureKind};
use crate::local::places::PlaceKind;
use crate::persona::{Facet, Val};

/// Who is in a place now: the settler, the place, when they set out, whether they will not come
/// back, and whether they have gone in past its mouth.
#[derive(Clone, Debug)]
pub struct Exploring { pub who: usize, pub place: usize, pub began: u64, pub doomed: bool, pub inside: bool }

/// One who went in and did not come back: who, the place, the day, what befell them (0 trapped
/// and hurt, 1 dead, 2 never to be found) and whether the camp has looked for them.
#[derive(Clone, Debug)]
pub struct Lost { pub who: usize, pub place: usize, pub day: u64, pub fate: u8, pub missed: bool, pub searched: bool }

fn compass(dx: f32, dy: f32) -> &'static str {
    let a = dy.atan2(dx).to_degrees();
    match ((a + 360.0 + 22.5) % 360.0 / 45.0) as usize {
        0 => "east", 1 => "south-east", 2 => "south", 3 => "south-west", 4 => "west", 5 => "north-west", 6 => "north", _ => "north-east",
    }
}

/// The way in, as one sees it before going in.
fn way_in(kind: PlaceKind, alive: bool) -> &'static str {
    match kind {
        PlaceKind::OldMine => "the timbered mouth of an old working",
        PlaceKind::Tomb => "a door of fitted stone in the hillside",
        PlaceKind::Lair if alive => "a hole in the hill with bones at its lip and the reek of something living",
        PlaceKind::Lair => "a dark hole in the hill with old bones at its lip",
        PlaceKind::Cave | PlaceKind::Cavern => "the mouth of a cave",
        PlaceKind::Halls => "a carved door in the hill, its lintel cut with runes",
    }
}

/// What is risked going in, by the place: (the chance of not coming back, what befalls one).
fn danger(kind: PlaceKind, alive: bool) -> f32 {
    match kind {
        PlaceKind::Lair if alive => 0.7,
        PlaceKind::Tomb => 0.3,
        PlaceKind::OldMine => 0.3,
        PlaceKind::Cave | PlaceKind::Cavern => 0.25,
        PlaceKind::Lair | PlaceKind::Halls => 0.15,
    }
}

impl Colony {
    /// A settler's nerve for the dark: bravery, a veteran's past, curiosity, less anxiety.
    pub(crate) fn nerve(&self, i: usize) -> i32 {
        let p = &self.settlers[i].persona;
        p.facet(Facet::Bravery) as i32 + if self.is_veteran(i) { 20 } else { 0 }
            + (p.facet(Facet::Curiosity) as i32 - 50) / 2 - (p.facet(Facet::Anxiety) as i32 - 50) / 2
    }

    /// A child of a burned town (the roster's young survivors) will not go into the dark.
    fn child_of_a_burned_town(&self, i: usize) -> bool {
        self.settlers[i].past.as_ref().map_or(false, |p| p.calling.starts_with("a survivor of") && p.age <= 20)
    }

    fn place_alive(&self, k: usize) -> bool { self.map.places[k].cause.ends_with("and lives") }

    /// Who may go into the dark: grown, well, not a guest or away, not a child of a burned town.
    fn may_go_in(&self, i: usize) -> bool {
        let s = &self.settlers[i];
        s.alive && s.guest_until == 0 && s.away_until == 0 && s.ill_until <= self.clock.tick && s.past.as_ref().map_or(true, |p| p.age >= 16)
            && !s.wounds.iter().any(|w| w.severity >= 2) && !self.child_of_a_burned_town(i)
    }

    /// Going into a place whose way in is known, by day: the bold, one at a time.
    fn go_in_option(&self, i: usize) -> Option<(f32, Job, String)> {
        let hour = self.clock.hour();
        if !(6..=14).contains(&hour) || !self.may_go_in(i) { return None; }
        if self.explorer.as_ref().map_or(false, |e| e.who != i) { return None; }
        // (One at a time: another may have set out since the last ten minutes.)
        if (0..self.settlers.len()).any(|j| j != i && self.settlers[j].alive && self.settlers[j].why.starts_with("Going in by ")) { return None; }
        let day = self.clock.day();
        let camp = self.camp;
        let open: Vec<usize> = self.mouths_seen.iter().copied().filter(|&k| !self.places_found.contains(&k)
            // (A place that kept someone is left alone for twenty days.)
            && !self.lost.iter().any(|l| l.place == k && day < l.day + 20)).collect();
        let nerve = self.nerve(i);
        for k in open {
            let Some(m) = self.map.places[k].mouth else { continue };
            let blessed = self.marked(m, false);
            if nerve < if blessed { 45 } else { 58 } { continue; }
            let word = way_in(self.map.places[k].kind, self.place_alive(k));
            if self.marked(m, true) {
                // Forbidden: they look, and stay away (said once in five days).
                let mut t = self.turned_away.borrow_mut();
                if t.1.is_none() && (t.0 == 0 || day >= t.0 + 5) {
                    *t = (day, Some(format!("{} looks toward {} at {},{} and stays away: the patron has forbidden that ground.", self.settlers[i].name, word, m.0, m.1)));
                }
                continue;
            }
            let Some(&(far, _)) = self.map.places[k].cells.last() else { continue };
            if !self.reachable_from(camp, m) { continue; }
            let p = &self.settlers[i].persona;
            let (she, her) = if p.female { ("she", "her") } else { ("he", "him") };
            let heart = if self.is_veteran(i) { format!("{} has stood in battle, and the dark is only dark", she) }
                else if p.facet(Facet::Anxiety) >= 65 { format!("though the dark frightens {}", her) }
                else if p.facet(Facet::Bravery) >= 75 { format!("{} fears little", she) }
                else if p.facet(Facet::Curiosity) >= 65 { format!("{} must know what is there", she) }
                else { format!("{} has the nerve for it", she) };
            let w = (0.2 + 0.5 * ((nerve - 50) as f32 / 50.0).clamp(0.0, 1.0)) * if blessed { 3.0 } else { 1.0 } * if self.rumoured.contains(&k) { 1.5 } else { 1.0 };
            let why = format!("Going in by {} at {},{} to see what lies there: {}{}", word, m.0, m.1, heart,
                if blessed { "; the patron blessed the way in" } else if self.rumoured.contains(&k) { "; the old folk spoke of it" } else { "" });
            return Some((w, Job::Wander(far), why));
        }
        None
    }

    /// The level a settler going into a place walks to: its far end's.
    pub(crate) fn place_far_level(&self, t: Pos) -> Option<i32> {
        self.map.places.iter().find(|p| p.cells.last().map_or(false, |c| c.0 == t)).and_then(|p| p.cells.last()).map(|c| c.1)
    }

    /// The bold go in; the curious roam the hills by day.
    pub(crate) fn explore_option(&self, i: usize) -> Option<(f32, Job, String)> {
        if self.clock.is_night() { return None; }
        if let Some(o) = self.go_in_option(i) { return Some(o); }
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

    /// Every minute the one going in (the cells they walk appear); every ten minutes, anyone
    /// near a way in not yet seen sees it; at 20:00 the camp misses whoever did not come back.
    pub(crate) fn explore_tick(&mut self) {
        let said = self.turned_away.borrow_mut().1.take();
        if let Some(line) = said { self.note(line); }
        if self.explorer.is_none() && self.clock.minute() % 10 == 0 {
            // Someone has set out into a place (their choice in `decide`).
            let going = (0..self.settlers.len()).find(|&i| self.settlers[i].alive && matches!(self.settlers[i].job, Job::Wander(_)) && self.settlers[i].why.starts_with("Going in by "));
            if let Some(i) = going {
                let Job::Wander(t) = self.settlers[i].job else { unreachable!() };
                if let Some(k) = self.map.places.iter().position(|p| p.cells.last().map_or(false, |c| c.0 == t)) {
                    let risk = danger(self.map.places[k].kind, self.place_alive(k)) * (1.25 - self.nerve(i) as f32 / 200.0).clamp(0.6, 1.2);
                    let doomed = (crate::history::settlers::hash_pub(self.seed ^ 0xD00D, (k as u64) << 16 | i as u64) % 1000) as f32 / 1000.0 < risk;
                    self.explorer = Some(Exploring { who: i, place: k, began: self.clock.tick, doomed, inside: false });
                }
            }
        }
        if self.explorer.is_some() { self.explorer_step(); }
        if self.clock.hour() == 20 && self.clock.minute() == 0 { self.miss_the_lost(); self.tell_of_places(); }
        if self.clock.minute() % 10 != 0 { return; }
        for k in 0..self.map.places.len() {
            if self.places_found.contains(&k) || self.mouths_seen.contains(&k) { continue; }
            let Some(m) = self.map.places[k].mouth else { continue };
            // (Or anyone at all, when the patron has blessed the ground there: eyes are drawn to it.)
            let blessed = self.marked(m, false);
            let finder = (0..self.settlers.len()).find(|&i| self.settlers[i].alive && !self.below(i)
                && (blessed || (self.settlers[i].pos.0 as i32 - m.0 as i32).abs().max((self.settlers[i].pos.1 as i32 - m.1 as i32).abs()) <= 3));
            if let Some(i) = finder {
                self.mouths_seen.push(k);
                let name = self.settlers[i].name.clone();
                let word = way_in(self.map.places[k].kind, self.place_alive(k));
                if blessed { self.note(format!("{}'s eye is drawn to {} at {},{}, where the patron blessed the ground: no one knows what lies inside.", name, word, m.0, m.1)); }
                else { self.note(format!("{} comes on {} at {},{}: no one knows what lies inside.", name, word, m.0, m.1)); }
            }
        }
    }

    /// The one in a place: what they walk past appears; at the far end, the find; if the place
    /// keeps them, they are gone (missed at dusk); if they turn back, the walk is over.
    fn explorer_step(&mut self) {
        let Some(e) = self.explorer.clone() else { return };
        let i = e.who;
        let going = self.settlers[i].alive && matches!(self.settlers[i].job, Job::Wander(_)) && self.settlers[i].why.starts_with("Going in by ");
        if !going { self.explorer = None; return; }
        let (x, y, z) = self.here3(i);
        let cells = self.map.places[e.place].cells.clone();
        for &(c, cz) in &cells {
            if (c.0 as i32 - x as i32).abs().max((c.1 as i32 - y as i32).abs()) <= 3 && (cz - z).abs() <= 2 && !self.revealed.contains(&(c, cz)) {
                self.revealed.push((c, cz));
            }
        }
        let inside = cells.iter().any(|&(c, cz)| c == (x, y) && cz == z) && z < self.map.surface_z[y as usize * self.map.width + x as usize];
        if inside && !e.inside { if let Some(ex) = self.explorer.as_mut() { ex.inside = true; } }
        let far = *cells.last().unwrap();
        let at_far = (far.0 .0 as i32 - x as i32).abs().max((far.0 .1 as i32 - y as i32).abs()) <= 1 && (far.1 - z).abs() <= 1;
        // The place keeps the doomed past its middle.
        let deep = cells.iter().position(|&(c, cz)| c == (x, y) && cz == z).map_or(false, |p| p * 2 >= cells.len());
        if e.doomed && (deep || at_far) {
            let k = e.place;
            let alive = self.place_alive(k);
            let fate = if alive { 1 } else { (crate::history::settlers::hash_pub(self.seed ^ 0x105E, (k as u64) << 16 | i as u64) % 3) as u8 };
            let s = &mut self.settlers[i];
            s.alive = false;
            s.away_until = u64::MAX / 2;
            s.path.clear();
            s.job = Job::Idle;
            s.why = format!("Lost in {}", { let n = &self.map.places[k].name; let mut c = n.chars(); c.next().map(|f| f.to_lowercase().collect::<String>() + c.as_str()).unwrap_or_default() });
            self.lost.push(Lost { who: i, place: k, day: self.clock.day(), fate, missed: false, searched: false });
            self.explorer = None;
            return;
        }
        if at_far {
            self.explorer = None;
            for c in cells { if !self.revealed.contains(&c) { self.revealed.push(c); } }
            if !self.places_found.contains(&e.place) { self.find_place(i, e.place); }
            self.settlers[i].job = Job::Idle;
            self.settlers[i].path.clear();
        }
    }

    /// 20:00: the camp misses whoever went into a place and has not come back.
    fn miss_the_lost(&mut self) {
        for n in 0..self.lost.len() {
            if self.lost[n].missed { continue; }
            self.lost[n].missed = true;
            let (i, k) = (self.lost[n].who, self.lost[n].place);
            let name = self.settlers[i].name.clone();
            let m = self.map.places[k].mouth.unwrap_or(self.camp);
            let word = way_in(self.map.places[k].kind, self.place_alive(k));
            let roar = if self.place_alive(k) { " Toward noon a roar came up out of the hill." } else { "" };
            let line = format!("{} went into {} at {},{} this morning and has not come back.{}", name, word, m.0, m.1, roar);
            self.note(line.clone());
            self.moment(format!("{} does not come back", name), line, format!("because {} went into the dark alone", name), m);
            for j in 0..self.settlers.len() {
                if self.settlers[j].alive { let w = name.clone(); self.feel(j, mind::Feel::TheDeep { what: format!("{} gone into the dark", w) }); }
            }
        }
    }

    /// Dawn after someone is missed: the camp decides whether to go in after them. Those who
    /// love them go whatever their nerve; a speaker will not send more into a living beast's den
    /// unless brave. Found hurt and carried up, found dead, or never found.
    pub(crate) fn reckon_lost(&mut self) {
        for n in 0..self.lost.len() {
            let l = self.lost[n].clone();
            if l.searched || !l.missed || self.clock.day() <= l.day { continue; }
            self.lost[n].searched = true;
            let (i, k) = (l.who, l.place);
            let name = self.settlers[i].name.clone();
            let word = way_in(self.map.places[k].kind, self.place_alive(k));
            let by = word.split(" with ").next().unwrap_or(word).to_string();
            // (What they know it as: its name once found, else the dark under the hill.)
            let dark = if self.places_found.contains(&k) { let n = &self.map.places[k].name; let mut c = n.chars(); format!("the dark of {}", c.next().map(|f| f.to_lowercase().collect::<String>() + c.as_str()).unwrap_or_default()) }
                else { "the dark under the hill".to_string() };
            let m = self.map.places[k].mouth.unwrap_or(self.camp);
            let love = |c: &Colony, j: usize| c.opinion(j, i) >= 12 || c.settlers[j].spouse == Some(i);
            let mut party: Vec<usize> = (0..self.settlers.len()).filter(|&j| j != i && self.may_go_in(j) && (self.nerve(j) >= 45 || love(self, j))).collect();
            party.sort_by_key(|&j| (std::cmp::Reverse(love(self, j) as i32), std::cmp::Reverse(self.nerve(j)), j));
            party.truncate(2);
            let speaker_afraid = self.place_alive(k) && self.speaker.map_or(false, |sp| self.settlers[sp].alive && self.settlers[sp].persona.facet(Facet::Bravery) < 60 && !party.iter().any(|&j| love(self, j)));
            if party.is_empty() || speaker_afraid {
                let why = if speaker_afraid { format!("{} will not send more into the den of a living beast", self.settlers[self.speaker.unwrap()].name) } else { "no one has the nerve for it".to_string() };
                let line = format!("No one goes into {} after {}: {}. {} is given up for lost.", word, name, why, name);
                self.note(line.clone());
                self.moment(format!("{} given up for lost", name), line, format!("because {} kept {}", dark, name), m);
                self.grieve_for(i);
                continue;
            }
            let names: Vec<String> = party.iter().map(|&j| self.settlers[j].name.clone()).collect();
            let they = crate::persona::list(&names);
            let ground = self.map.surface_z[m.1 as usize * self.map.width + m.0 as usize];
            let found = match l.fate {
                0 => {
                    // Trapped and hurt: carried up with a broken leg.
                    let s = &mut self.settlers[i];
                    s.alive = true; s.away_until = 0; s.pos = m; s.path.clear(); s.job = Job::Idle; s.z = ground;
                    let tick = self.clock.tick;
                    self.settlers[i].wounds.push(super::fight::Wound { part: "left leg".into(), severity: 3, healed_at: tick + 24 * TICKS_PER_DAY, from: format!("a fall in {}", dark), tended: self.clock.day(), infected: false, fever_since: 0 });
                    for &j in &party { self.feel(j, mind::Feel::Saved { whom: name.clone() }); }
                    self.feel(i, mind::Feel::SavedBy { whom: names[0].clone() });
                    format!("{} go in by {} with torches and rope to look for {}, and find {} at the bottom of a fall in the dark, alive, with a broken leg. They carry {} up into the light.", they, by, name, if self.settlers[i].persona.female { "her" } else { "him" }, if self.settlers[i].persona.female { "her" } else { "him" })
                }
                1 => {
                    // Dead: carried up and buried.
                    let s = &mut self.settlers[i];
                    s.alive = true; s.away_until = 0; s.pos = m; s.path.clear(); s.job = Job::Idle; s.z = ground;
                    let cause = if self.place_alive(k) { "in the den of the beast under the hill".to_string() } else { format!("in {}", dark) };
                    self.bury(i, &cause);
                    format!("{} go in by {} with torches and rope to look for {}, and find {} dead in the dark. They carry the body up.", they, by, name, if self.settlers[i].persona.female { "her" } else { "him" })
                }
                _ => {
                    self.grieve_for(i);
                    format!("{} go in by {} with torches and rope to look for {}, and search {} until the torches burn low. They come back without {}.", they, by, name, dark, if self.settlers[i].persona.female { "her" } else { "him" })
                }
            };
            self.note(found.clone());
            self.moment(format!("The search for {}", name), found, format!("because {} went into {} and did not come back", name, dark), m);
            // The searchers saw the place: it is found (its find by the first of them).
            for c in self.map.places[k].cells.clone() { if !self.revealed.contains(&c) { self.revealed.push(c); } }
            if !self.places_found.contains(&k) { let j = party[0]; self.find_place(j, k); }
        }
    }

    /// Those who loved one lost for good grieve as for a death.
    fn grieve_for(&mut self, i: usize) {
        let name = self.settlers[i].name.clone();
        for j in 0..self.settlers.len() {
            if j == i || !self.settlers[j].alive { continue; }
            let close = self.opinion(j, i) >= 12 || self.settlers[j].spouse == Some(i);
            if close || self.opinion(j, i) >= 4 { self.feel(j, mind::Feel::Death { whom: name.clone(), close }); }
        }
    }

    /// The second evening: the eldest tells what the old folk said of a place in these hills
    /// (its cause, and which way it lies): its way in is known from then on.
    pub(crate) fn tell_of_places(&mut self) {
        if self.clock.day() != 2 || self.clock.hour() != 20 || self.clock.minute() != 0 { return; }
        let Some(k) = (0..self.map.places.len()).find(|&k| self.map.places[k].mouth.is_some() && !self.mouths_seen.contains(&k) && !self.places_found.contains(&k)
            && self.map.places[k].kind != PlaceKind::Cavern) else { return };
        let Some(teller) = (0..self.settlers.len()).filter(|&i| self.settlers[i].alive && self.settlers[i].guest_until == 0)
            .max_by_key(|&i| (self.settlers[i].past.as_ref().map_or(0, |p| p.age), std::cmp::Reverse(i))) else { return };
        let m = self.map.places[k].mouth.unwrap();
        let (dx, dy) = (m.0 as f32 - self.camp.0 as f32, m.1 as f32 - self.camp.1 as f32);
        let dir = compass(dx, dy);
        // (A cause that opens with a name keeps its capital: "Glauzgagu, captain of ...".)
        let cause = { let c = &self.map.places[k].cause; if c.starts_with("The ") { c.replacen("The ", "the ", 1) } else { c.clone() } };
        let name = self.settlers[teller].name.clone();
        let line = format!("By the fire {} tells what the old folk say of these hills: {}. Its way in should lie to the {}, some {} paces from the fire.", name, cause, dir, ((dx * dx + dy * dy).sqrt() * 2.0 / 0.75) as u32 / 10 * 10);
        self.note(line);
        self.mouths_seen.push(k);
        self.rumoured.push(k);
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
                        // (What the old miners left: six loads for the smelter, `industry.rs`.)
                        self.add_ore(o, 6);
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
                    // (The robbed dead rising is myth.)
                    if crate::myth::on() && (self.darkness >= 0.2 || crate::history::settlers::hash_pub(self.seed, 0x70B + k as u64) % 3 == 0) { self.tomb_risen = Some((dead, day)); }
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
