//! A thing of the night among the migrants: the vampire.
//!
//! The idea is Dwarf Fortress's vampires: a cursed one arrives with the migrants, looks like
//! anyone, never eats or drinks, and feeds on sleepers at night; the fortress finds out through
//! witnesses or a sharp-eyed dwarf and the law deals with them. Here, where the Shadow's
//! darkness lies on the camp (0.2+), one migrant wave in three brings one (`PLANET_FORCE_VAMPIRE=1`
//! forces it). They never eat (`decide` gives them no meal: their hunger is held low). One night
//! in four at 02:00 they feed on a sleeper: the victim wakes weak and pale (ill two days, a
//! dread); fed on a second time, they are found dead at dawn with two marks at the throat (a
//! moment). Anyone awake within five cells may see it (one in two), and a victim may wake (one in five) and see
//! the face: either way it is a crime. After seven days among them, the sharpest mind of the camp
//! (intuition 1300+) notices they never eat, and after a feeding names them. The speaker judges
//! (`justice.rs`): put to death at dawn unless they despise the law (then cast out); with no
//! speaker the camp drives them out with fire.

use super::*;
use crate::persona::Attr;

impl Colony {
    /// A migrant wave may bring one.
    pub(crate) fn maybe_vampire(&mut self, newcomers: &[usize]) {
        if self.vampire.is_some() || newcomers.is_empty() { return; }
        let forced = std::env::var("PLANET_FORCE_VAMPIRE").is_ok();
        if !forced && (self.darkness < 0.2 || crate::history::settlers::hash_pub(self.seed ^ self.clock.day(), 0x7A3) % 3 != 0) { return; }
        let v = *newcomers.last().unwrap();
        self.vampire = Some((v, self.clock.day()));
        self.drained.clear();
        self.vampire_noticed = false;
    }

    /// Hourly: the vampire's hunger is not for bread.
    pub(crate) fn vampire_hour(&mut self) {
        if let Some((v, _)) = self.vampire { if self.settlers[v].alive { self.settlers[v].hunger = self.settlers[v].hunger.min(0.2); } }
    }

    /// 02:00, one night in four: it feeds.
    pub(crate) fn vampire_feeds(&mut self) {
        let Some((v, since)) = self.vampire else { return };
        if !self.settlers[v].alive || self.settlers[v].mind.left { return; }
        let day = self.clock.day();
        // It bides its time among them first.
        if day < since + 5 || crate::history::settlers::hash_pub(self.seed ^ day, 0xB100D) % 4 != 0 { return; }
        let sleepers: Vec<usize> = (0..self.settlers.len()).filter(|&j| j != v && self.settlers[j].alive && self.settlers[j].job == Job::Sleep && self.settlers[j].guest_until == 0).collect();
        if sleepers.is_empty() { return; }
        // The one already fed on goes first (it has a taste for them), else by hash.
        let victim = sleepers.iter().copied().find(|j| self.drained.get(j).copied().unwrap_or(0) > 0)
            .unwrap_or(sleepers[(crate::history::settlers::hash_pub(self.seed ^ day, 0xB1D) % sleepers.len() as u64) as usize]);
        let at = self.settlers[victim].pos;
        let (vn, xn) = (self.settlers[v].name.clone(), self.settlers[victim].name.clone());
        let n = { let e = self.drained.entry(victim).or_insert(0); *e += 1; *e };
        // Seen: someone awake near, or the victim waking.
        let seen = (0..self.settlers.len()).find(|&j| j != v && j != victim && self.settlers[j].alive && self.settlers[j].job != Job::Sleep
            && (self.settlers[j].pos.0 as i32 - at.0 as i32).abs().max((self.settlers[j].pos.1 as i32 - at.1 as i32).abs()) <= 5)
            .filter(|&j| crate::history::settlers::hash_pub(self.seed ^ day, 0x5EE + j as u64) % 2 == 0);
        let woke = crate::history::settlers::hash_pub(self.seed ^ day, 0xA7A) % 5 == 0;
        if n >= 2 && !woke {
            // Drained: found at dawn (the burial and the moment wait for the light).
            self.drained_dead.push(victim);
            self.settlers[victim].hunger = self.settlers[victim].hunger.max(0.0);
        } else {
            self.settlers[victim].ill_until = self.settlers[victim].ill_until.max(self.clock.tick + 2 * TICKS_PER_DAY);
            self.feel(victim, mind::Feel::TheDeep { what: "two small wounds at their throat".into() });
        }
        let witness = seen.map(|w| (w, false)).or(if woke { Some((victim, true)) } else { None });
        if let Some((w, own)) = witness {
            let wn = self.settlers[w].name.clone();
            let line = if own { format!("{} wakes in the dark to find {} bent over them, mouth to their throat, and screams.", xn, vn) }
                else { format!("{} sees {} bent over the sleeping {} in the dark, mouth to their throat.", wn, vn, xn) };
            self.note(line);
            self.crimes.push(justice::Crime { who: v, what: format!("drank the blood of {} as they slept", xn), day, meals: 0, judged: false });
            self.like(v, w, -10);
        }
    }

    /// Dawn: the drained are found; the sharp-eyed notice who never eats.
    pub(crate) fn vampire_dawn(&mut self) {
        for victim in std::mem::take(&mut self.drained_dead) {
            if !self.settlers[victim].alive { continue; }
            let name = self.settlers[victim].name.clone();
            let line = format!("{} is found dead at dawn, pale as ash, with two small wounds at the throat.", name);
            self.note(line.clone());
            let at = self.settlers[victim].pos;
            self.moment(format!("The death of {}", name), line, "because something in the camp feeds on the sleepers".into(), at);
            self.bury(victim, "in the night, drained of blood");
            for j in 0..self.settlers.len() { if self.settlers[j].alive { self.feel(j, mind::Feel::TheDeep { what: format!("{} drained of blood", name) }); } }
        }
        let Some((v, since)) = self.vampire else { return };
        if !self.settlers[v].alive || self.settlers[v].mind.left || self.clock.day() < since + 7 { return; }
        let sharp = (0..self.settlers.len()).filter(|&j| j != v && self.settlers[j].alive && self.settlers[j].guest_until == 0)
            .max_by(|&a, &b| self.settlers[a].persona.attr(Attr::Intuition).total_cmp(&self.settlers[b].persona.attr(Attr::Intuition)).then(b.cmp(&a)));
        let Some(k) = sharp.filter(|&k| self.settlers[k].persona.attr(Attr::Intuition) >= 1300.0) else { return };
        let (kn, vn) = (self.settlers[k].name.clone(), self.settlers[v].name.clone());
        if !self.vampire_noticed {
            self.vampire_noticed = true;
            self.note(format!("{} has noticed that {} is never seen to eat, and walks about while the others sleep.", kn, vn));
            return;
        }
        // After a feeding, the noticer names them.
        if self.drained.values().sum::<u32>() > 0 && !self.crimes.iter().any(|c| c.who == v) {
            self.note(format!("{} says aloud what {} has been thinking: {} never eats, and the sleepers wake pale.", kn, if self.settlers[k].persona.female { "she" } else { "he" }, vn));
            self.crimes.push(justice::Crime { who: v, what: "is a thing of the night that feeds on the sleepers".into(), day: self.clock.day(), meals: 0, judged: false });
        }
    }
}
