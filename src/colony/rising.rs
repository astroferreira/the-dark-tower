//! A rising against the lord: grievances kept, and a night the camp settles them.
//!
//! The idea is Dwarf Fortress's unhappy citizens and their nobles: a noble's mandates and
//! punishments make enemies, and enough enemies make a noble's end (in DF, a tantrum or a brawl in
//! the hall). Here, while a lord rules the camp (`nobles.rs`), each thing done against a settler
//! in the lord's name is a grievance (`Colony::grievances`, counted in `feel`): a mandate against
//! their values or a choice they resent (`Feel::Mandate`), the stocks (`Feel::Punished`, two),
//! and a friend (opinion 12+) put in the stocks (one). At dawn (`reckon_rising`), thirty days or
//! more after the lord came, when the aggrieved (three grievances or more; grown, not guests)
//! number three and a quarter of the grown camp, one dawn in four the boldest of them (bravery,
//! independence, violence) leads them to the lord's hall (a moment). The loyal are those who hold
//! the lord at 20+ with one grievance at most and value loyalty, law or tradition. More aggrieved than
//! loyal: the lord is deposed: killed if the leader is cruel or violent (70+), else sent back
//! down the road; the office is empty until the camp chooses again; the lord's people (and the
//! ruler, their kin) hold it against the camp (`regard.rs`: -35 killed, -20 driven out). Fewer:
//! the loyal seize the leader, who goes to the stocks for three days (a cruel lord has them put
//! to death); the grievances stand. Once per lord.

use super::*;
use crate::persona::{Facet, Val};

impl Colony {
    /// The lord, if one rules the camp now.
    pub(crate) fn ruling_lord(&self) -> Option<usize> {
        self.speaker.filter(|&s| self.settlers[s].alive && self.settlers[s].office.as_deref() == Some("Lord of the camp"))
    }

    /// A grievance against the ruling lord (`feel` calls this for mandates and punishments).
    pub(crate) fn grieve(&mut self, i: usize, n: u32) {
        let Some(k) = self.ruling_lord() else { return };
        if i == k { return; }
        *self.grievances.entry(i).or_insert(0) += n;
    }

    /// Dawn: the aggrieved may rise.
    pub(crate) fn reckon_rising(&mut self) {
        let Some(k) = self.ruling_lord() else { return };
        let day = self.clock.day();
        let Some(came) = self.lord.as_ref().and_then(|l| l.came) else { return };
        if day < came + 30 || self.lord_risen { return; }
        let grown: Vec<usize> = (0..self.settlers.len()).filter(|&i| i != k && self.settlers[i].alive && self.settlers[i].guest_until == 0
            && self.settlers[i].past.as_ref().map_or(true, |p| p.age >= 14)).collect();
        let aggrieved: Vec<usize> = grown.iter().copied().filter(|i| self.grievances.get(i).copied().unwrap_or(0) >= 3).collect();
        if aggrieved.len() < 3 || aggrieved.len() * 4 < grown.len() { return; }
        if crate::history::settlers::hash_pub(self.seed ^ day, 0x415E) % 4 != 0 { return; }
        self.lord_risen = true;
        let bold = |c: &Colony, i: usize| { let p = &c.settlers[i].persona; p.facet(Facet::Bravery) as i32 + p.value(Val::Independence) as i32 + p.facet(Facet::Violence) as i32 };
        let leader = *aggrieved.iter().max_by_key(|&&i| (bold(self, i), std::cmp::Reverse(i))).unwrap();
        let loyal: Vec<usize> = grown.iter().copied().filter(|&i| {
            let p = &self.settlers[i].persona;
            self.grievances.get(&i).copied().unwrap_or(0) <= 1 && self.opinion(i, k) >= 20 && (p.value(Val::Loyalty) > 0 || p.value(Val::Law) > 0 || p.value(Val::Tradition) > 0)
        }).collect();
        let (ln, lord) = (self.settlers[leader].name.clone(), self.settlers[k].name.clone());
        let at = self.settlers[k].pos;
        let people = self.lord.as_ref().map(|l| l.people.clone()).unwrap_or_default();
        let faction = self.settlers[k].past.as_ref().and_then(|p| p.people);
        let stand = |n: usize| match n { 0 => "no one stands by the lord".to_string(), 1 => "one stands by the lord, and it is not enough".to_string(), n => format!("{} stand by the lord, and it is not enough", n) };
        let first = format!("Before dawn {} and {} others who have had enough of {} go to the lord's hall", ln, aggrieved.len() - 1, lord);
        if aggrieved.len() > loyal.len() {
            let lp = &self.settlers[leader].persona;
            let kill = lp.facet(Facet::Cruelty) >= 70 || lp.facet(Facet::Violence) >= 70;
            let line = if kill {
                format!("{}; {}. {} is killed in the hall.", first, stand(loyal.len()), lord)
            } else {
                format!("{}; {}. {} is put on the road back to {} with what {} can carry.", first, stand(loyal.len()), lord, people, if self.settlers[k].persona.female { "she" } else { "he" })
            };
            self.note(line.clone());
            self.moment(format!("{} is deposed", lord), line, format!("because {} of the camp held grievances against {}", aggrieved.len(), lord), at);
            self.settlers[k].office = None;
            self.speaker = None;
            if kill {
                self.bury(k, "in the lord's hall, at the hands of the camp");
                self.regard(faction, &people, None, "lord", -35, format!("killed {}, kin of their ruler, in the lord's hall", lord));
            } else {
                self.release(k);
                let s = &mut self.settlers[k];
                s.alive = false;
                s.mind.left = true;
                self.regard(faction, &people, None, "lord", -20, format!("drove out {}, kin of their ruler", lord));
            }
            for &j in &loyal { self.feel(j, mind::Feel::Mandate { what: format!("{} be {}", lord, if kill { "killed" } else { "driven out" }) }); }
            for &j in &aggrieved { self.feel(j, mind::Feel::Reconciled { by: format!("the end of {}'s rule", lord) }); }
            self.grievances.clear();
        } else {
            let cruel = self.settlers[k].persona.facet(Facet::Cruelty) >= 70;
            let line = format!("{}; but {} stand by the lord and seize {} at the door.{}", first, loyal.len(), ln,
                if cruel { format!(" {} has {} put to death at dawn.", lord, ln) } else { format!(" {} has {} put in the stocks for three days.", lord, ln) });
            self.note(line.clone());
            self.moment(format!("{} rises against {}", ln, lord), line, format!("because {} of the camp held grievances against {}, and more stood by the lord", aggrieved.len(), lord), at);
            if cruel {
                self.bury(leader, &format!("at dawn, put to death by {} for rising against the lord", lord));
            } else {
                self.stocks = Some((leader, self.clock.tick + 3 * TICKS_PER_DAY));
                self.feel(leader, mind::Feel::Punished { by: lord.clone() });
            }
        }
    }
}
