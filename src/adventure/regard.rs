//! Peoples remember what the adventurer did (DF's regard from enumerated causes): each town's
//! regard of the hero (-100 .. 100) moves with what they do to it, and half of it spreads to the
//! other towns of its people. Causes: striking or killing its people or its watch (-), slaying a
//! beast that troubled it (+, and every townsperson names the deed), its work done (+), a tale's
//! betrayal (-). Effects: prices (a friend's discount, an enemy's markup), the welcome, the watch
//! barring the gate and coming at the hero when they show their face (-40 or worse), bounty
//! hunters sent after them on the road (-30 or worse). It fades toward nothing, a point a day.

use super::actor::{Monster, Role};
use super::game::{Game, Tone};
use super::site::SiteKind;

/// A cause of regard.
pub enum Cause<'a> { Struck, Killed, BeastSlain(&'a str), WorkDone, Betrayed }

impl Game {
    /// The town's regard of the hero.
    pub fn regard_of(&self, town: u32) -> i32 { self.regard.get(&town).copied().unwrap_or(0) }

    /// Change town `town`'s regard for a cause (half of it to the other towns of its people).
    pub fn regard(&mut self, town: u32, cause: Cause) {
        let (d, line) = match &cause {
            Cause::Struck => (-25, None),
            Cause::Killed => (-60, None),
            Cause::BeastSlain(b) => (35, Some(format!("slaying {}", b))),
            Cause::WorkDone => (8, None),
            Cause::Betrayed => (-30, None),
        };
        let people = self.site(town).map(|s| s.people.clone()).unwrap_or_default();
        let kin: Vec<u32> = self.sites.iter().filter(|s| s.kind == SiteKind::Town && s.id != town && !people.is_empty() && s.people == people).map(|s| s.id).collect();
        let e = self.regard.entry(town).or_insert(0);
        *e = (*e + d).clamp(-100, 100);
        for k in kin { let e = self.regard.entry(k).or_insert(0); *e = (*e + d / 2).clamp(-100, 100); }
        // Everyone in the town knows what was done.
        if let Some(deed) = line { for n in self.town_npcs_mut(town) { if !n.met.helped.contains(&deed) { n.met.helped.push(deed.clone()); } } }
        if d < 0 { for n in self.town_npcs_mut(town) { n.met.wronged += if d <= -50 { 2 } else { 1 }; } }
    }

    /// A beast slain: every town within six tiles it troubled is grateful.
    pub fn beast_slain(&mut self, at: (usize, usize), name: &str) {
        let w = self.world.w;
        let towns: Vec<(u32, String)> = self.sites.iter().filter(|s| s.kind == SiteKind::Town && super::world::dist(s.tile, at, w) <= 6).map(|s| (s.id, s.name.clone())).collect();
        for (t, tname) in towns {
            self.regard(t, Cause::BeastSlain(name));
            self.say(Tone::Quest, format!("Word of it will reach {}: they will not forget who slew {}.", tname, name));
        }
    }

    /// A town's people struck (by the hero, in the street): they cry out and the watch comes.
    pub fn assault(&mut self, k: usize) -> Option<i32> {
        let p = self.place()?;
        let n = p.npcs.get(k)?.clone();
        let town = n.home;
        let def = if n.role == Role::Guard { "watchman" } else { "townsman" };
        let uid = self.fresh_uid();
        let mut m = Monster::new(uid, def, n.x, n.y, n.z);
        m.name = n.name.clone();
        m.boss = false;
        m.awake = true;
        m.town = town;
        let name = n.name.clone();
        if let Some(p) = self.place_mut() { p.npcs.remove(k); p.monsters.push(m); }
        self.say(Tone::Danger, format!("You strike {}! \"Murder! Help! The watch!\"", name));
        if town != 0 {
            self.regard(town, Cause::Struck);
            self.call_the_watch(town, 2);
        }
        self.talk = None;
        Some(100)
    }

    /// The town's watch comes at the hero (on the land, near them).
    pub fn call_the_watch(&mut self, town: u32, n: usize) {
        if !self.on_land() { return; }
        let tname = self.site(town).map(|s| s.name.clone()).unwrap_or_default();
        for k in 0..n {
            let (x, y) = self.open_near(self.x + 4 + k as i32, self.y - 3);
            let uid = self.fresh_uid();
            let mut m = Monster::new(uid, "watchman", x, y, 0);
            m.name = format!("a watchman of {}", tname);
            m.awake = true;
            m.town = town;
            if let Some(p) = self.land.as_mut() { p.monsters.push(m); }
        }
    }

    /// A townsperson or one of the watch slain.
    pub fn town_blood(&mut self, town: u32, name: &str) {
        if town == 0 { return; }
        self.regard(town, Cause::Killed);
        let tname = self.site(town).map(|s| s.name.clone()).unwrap_or_default();
        self.say(Tone::Danger, format!("{} is dead by your hand. {} will not forget it.", name, tname));
        let turn = self.turn;
        self.deeds.push((turn, format!("killed {} of {}", name, tname)));
    }

    /// Coming into a town's land: a hated face is met at the gate by the watch.
    pub fn at_the_gate(&mut self, town: u32) {
        let r = self.regard_of(town);
        if r > -40 || !self.on_land() { return; }
        let tname = self.site(town).map(|s| s.name.clone()).unwrap_or_default();
        let near = self.land.as_ref().map_or(0, |p| p.monsters.iter().filter(|m| m.town == town && m.hp > 0).count());
        if near >= 2 { return; }
        self.say(Tone::Danger, format!("The watch of {} knows your face. \"Halt! In the lord's name!\"", tname));
        self.call_the_watch(town, 3);
    }

    /// A day passes: regard fades toward nothing.
    pub fn regard_day(&mut self) {
        for v in self.regard.values_mut() { if *v > 0 { *v -= 1; } else if *v < 0 { *v += 1; } }
        self.regard.retain(|_, v| *v != 0);
    }

    /// The worst regard of any town near, for bounty hunters on the road.
    pub fn hunted_by(&self) -> Option<(u32, i32)> {
        let w = self.world.w;
        self.regard.iter().filter(|(t, r)| **r <= -30 && self.site(**t).map_or(false, |s| super::world::dist(s.tile, self.tile, w) <= 12)).min_by_key(|(t, r)| (**r, **t)).map(|(t, r)| (*t, *r))
    }
}

/// What regard does to prices: a friend's discount, an enemy's markup.
pub fn price_factor(r: i32) -> f32 { (1.0 - r as f32 * 0.004).clamp(0.85, 1.4) }
