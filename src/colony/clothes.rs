//! Clothes: what they wear wears out, and someone sews new.
//!
//! The idea is Dwarf Fortress's clothing: garments wear and tatter, ragged dwarves are unhappy,
//! and the fortress's tailors and leatherworkers keep them clothed from cloth and leather. Here
//! each settler's clothes have an age (`Colony::clothes`: the day they were made; those who came
//! with their own came in clothes 0-150 days old, by their name). At 180 days they are rags: at
//! dawn every fifth day the ragged feel it (`Feel::Ragged`; the first time is a line). With a
//! workshop standing and a hide to hand (the hunt's and the pen's, shared with armour:
//! `hunted` less `hides_used`) or cloth from the caravans (`Colony::cloth`, two suits a bolt),
//! one settler at a time who likes to make things sews for the most ragged (`sew_option`,
//! `Job::Craft` "Sewing"): new clothes, and the one clothed likes the sewer better (+2). The
//! liaison asks for bolts of cloth when three are ragged and no hide is to hand (`liaison.rs`).

use super::*;

impl Colony {
    /// Days settler `i`'s clothes have been worn.
    pub fn clothes_worn(&self, i: usize) -> u64 {
        let day = self.clock.day();
        match self.clothes.get(&i) {
            Some(&d) => day.saturating_sub(d),
            None => day + crate::history::settlers::hash_pub(crate::persona::seed_of(&self.settlers[i].name, 0xC107), 0xC108) % 150,
        }
    }

    fn ragged(&self, i: usize) -> bool { self.settlers[i].alive && self.settlers[i].guest_until == 0 && self.clothes_worn(i) >= 180 }

    /// Dawn, every fifth day: the ragged feel it.
    pub(crate) fn reckon_clothes(&mut self) {
        let day = self.clock.day();
        if day % 5 != 0 { return; }
        let ragged: Vec<usize> = (0..self.settlers.len()).filter(|&i| self.ragged(i)).collect();
        if ragged.is_empty() { return; }
        if self.milestones.insert("rags") {
            let names: Vec<String> = ragged.iter().take(3).map(|&i| self.settlers[i].name.clone()).collect();
            self.note(format!("{}{} {} in rags now: the clothes they came in are worn through.", crate::persona::list(&names), if ragged.len() > 3 { format!(" and {} others", ragged.len() - 3) } else { String::new() }, if ragged.len() == 1 { "is" } else { "are" }));
        }
        for i in ragged { self.feel(i, mind::Feel::Ragged); }
    }

    /// What there is to sew with: cloth from the caravans, else a hide.
    fn sewing_stuff(&self) -> Option<&'static str> {
        if self.cloth > 0 { Some("cloth") } else if self.hunted > self.hides_used { Some("hide") } else { None }
    }

    /// By day, for one who likes making things: new clothes for the most ragged.
    pub(crate) fn sew_option(&self, i: usize) -> Option<(f32, Job, String)> {
        if self.clock.is_night() || self.workshop_spot().is_none() { return None; }
        if self.settlers.iter().enumerate().any(|(j, s)| j != i && s.alive && s.job == Job::Craft && s.why.starts_with("Sewing")) { return None; }
        let stuff = self.sewing_stuff()?;
        let who = (0..self.settlers.len()).filter(|&j| self.ragged(j)).max_by_key(|&j| (self.clothes_worn(j), std::cmp::Reverse(j)))?;
        let wish = self.craft_wish_any(i);
        if wish < 0.3 { return None; }
        // (Below the workshop's other work: rags are a small misery, and sewing had crowded out
        // the carving, the books and the armour.)
        Some((wish * 0.45, Job::Craft, format!("Sewing new clothes of {} for {}", stuff, self.settlers[who].name)))
    }

    /// The sewing is done.
    pub(crate) fn finish_sew(&mut self, i: usize) {
        let Some(stuff) = self.sewing_stuff() else { return };
        let Some(who) = (0..self.settlers.len()).filter(|&j| self.ragged(j)).max_by_key(|&j| (self.clothes_worn(j), std::cmp::Reverse(j))) else { return };
        if stuff == "cloth" { self.cloth_used += 1; if self.cloth_used % 2 == 0 { self.cloth -= 1; } } else { self.hides_used += 1; }
        let day = self.clock.day();
        self.clothes.insert(who, day);
        if who != i { self.like(who, i, 2); }
        let (sn, wn) = (self.settlers[i].name.clone(), self.settlers[who].name.clone());
        if self.milestones.insert("first sewing") {
            let what = if stuff == "cloth" { "a coat and breeches of the traders' cloth" } else { "a jerkin and leggings of hide" };
            self.note(format!("{} sews {} {} at the workshop: out of rags at last.", sn, if who == i { "for herself or himself".to_string() } else { wn }, what).replace("for herself or himself", if self.settlers[i].persona.female { "herself" } else { "himself" }));
        }
    }
}
