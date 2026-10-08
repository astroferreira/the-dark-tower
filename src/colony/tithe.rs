//! The tithe: the people who claim the land send for their share.
//!
//! The idea is Dwarf Fortress's tax collector (its monarch's tithe on the fortress) and the
//! roadmap's "tax collectors and tribute demands from the faction that claims the land". Here
//! the camp's trading town's people claim it (`trade::Partner`, the founders' people's nearest
//! town). From day 100, once a camp year (`YEAR_DAYS`), at dawn (`reckon_tithe`) a collector
//! comes for a tenth of the store (ten meals at least, but never below half a meal a head for
//! twenty days, forty with winter near) and two unsold works (never an artifact or
//! a masterwork). With under two meals a head stored, the collector sees the want and takes
//! nothing. Else the speaker (else the oldest) pays if they are of that people and hold loyalty,
//! law and tradition above independence and greed, or if they are not of that people but the
//! camp fears it (fewer than three spears); else refuses. Paid: their regard +5
//! (`regard.rs`), and the greedy and independent count it a grievance under a lord (`rising.rs`
//! reads `Feel::Mandate`). Refused: regard -10 ("refused them the tithe").

use super::*;
use crate::persona::{Facet, Val};

impl Colony {
    /// Dawn, once a camp year from day 100: the collector comes.
    pub(crate) fn reckon_tithe(&mut self) {
        let day = self.clock.day();
        if day < 100 || self.alive() == 0 || (day - 100) % ageing::YEAR_DAYS != 0 { return; }
        let Some(p) = self.trade.clone() else { return };
        // A people sworn to vengeance sends spears, not collectors (`regard.rs`).
        if self.regards.iter().any(|r| r.faction == p.faction && r.acted == Some(false)) { return; }
        let alive = self.alive() as u32;
        let food = self.food_stored();
        if food < 2 * alive {
            self.note(format!("A collector of {} comes for the tithe, sees how little the store holds, and takes nothing this year.", p.people));
            return;
        }
        // Never what the camp needs: twenty days a head left in the store (forty with winter
        // near), whatever the tenth would be.
        let winter_near = self.days_to_winter().map_or(true, |d| d <= 30) || self.hard_winter();
        let keep = alive * if winter_near { 40 } else { 20 } / 2;
        let meals = (food / 10).max(10).min(food.saturating_sub(keep));
        let works: Vec<usize> = (0..self.works.len()).filter(|&k| !self.works[k].traded && self.works[k].quality < 5).take(2).collect();
        let pool: Vec<usize> = (0..self.settlers.len()).filter(|&i| self.settlers[i].alive && self.settlers[i].guest_until == 0).collect();
        let Some(&judge) = self.speaker.filter(|&s| self.settlers[s].alive).as_ref()
            .or_else(|| pool.iter().max_by_key(|&&i| (self.settlers[i].past.as_ref().map_or(30, |x| x.age), std::cmp::Reverse(i)))) else { return };
        let jp = self.settlers[judge].persona.clone();
        let theirs = self.settlers[judge].past.as_ref().and_then(|x| x.people) == Some(p.faction);
        let lean = jp.value(Val::Loyalty) as i32 + jp.value(Val::Law) as i32 + jp.value(Val::Tradition) as i32 - jp.value(Val::Independence) as i32 - jp.facet(Facet::Greed) as i32 / 2;
        let afraid = self.militia_ready().1 < 3;
        let pay = if theirs { lean >= -10 } else { afraid };
        let jn = self.settlers[judge].name.clone();
        if meals == 0 && works.is_empty() {
            self.note(format!("A collector of {} comes for the tithe, sees the store must last the camp, and takes nothing this year.", p.people));
            return;
        }
        let w = format!("{} {}", works.len(), if works.len() == 1 { "work" } else { "works" });
        let what = match (meals, works.is_empty()) { (0, _) => w, (_, true) => format!("{} meals", meals), _ => format!("{} meals and {}", meals, w) };
        if pay {
            for _ in 0..meals { if let Some(k) = self.items.iter().rposition(|it| it.kind == ItemKind::Food && it.stored && !it.reserved) { self.items.remove(k); self.fix_refs_pub(k); } }
            for &k in &works { self.works[k].traded = true; }
            let why = if theirs { format!("{} {} of {} {}self", if jp.female { "she" } else { "he" }, "is", p.people, if jp.female { "her" } else { "him" }) } else { "the camp has too few spears to say no".to_string() };
            self.note(format!("A collector of {} comes for the tithe; {} pays it ({}), because {}.", p.people, jn, what, why));
            self.regard(Some(p.faction), &p.people, Some(p.from), "tithe", 5, format!("paid them the tithe on day {}", day));
            for &j in &pool {
                let s = &self.settlers[j].persona;
                if j != judge && (s.facet(Facet::Greed) >= 70 || s.value(Val::Independence) >= 26) { self.feel(j, mind::Feel::Mandate { what: format!("the tithe be paid to {}", p.people) }); }
            }
        } else {
            let why = if theirs { if jp.value(Val::Independence) > 0 { "the camp is its own".to_string() } else { "the store is theirs who filled it".to_string() } } else { format!("the camp owes {} nothing", p.people) };
            let line = format!("A collector of {} comes for the tithe ({}); {} refuses it, because {}. The collector leaves saying the town will hear of it.", p.people, what, jn, why);
            self.note(line.clone());
            let at = self.camp;
            self.moment("The tithe refused".into(), line, format!("because {} would not pay", jn), at);
            self.regard(Some(p.faction), &p.people, Some(p.from), "tithe", -10, format!("refused them the tithe on day {}", day));
        }
    }
}
