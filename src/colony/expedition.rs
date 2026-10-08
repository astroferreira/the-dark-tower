//! Expeditions: the militia goes out to slay a beast in its lair.
//!
//! The idea is Dwarf Fortress's raids and missions: squads sent out from the fortress against a
//! site, gone for days, coming back with loot, captives, or fewer than left. Here, from day 30,
//! when the militia has three or more drilled hands under arms and the speaker neither values
//! peace (20+) nor is timid (bravery under 30), a beast of the world laired within ten tiles (the one foretold and
//! not yet come, whose raid is then called off if it falls, or one still to come) may be hunted: one dawn in ten the speaker
//! sends the best fighters (three to five, a visiting monster hunter first; never the speaker,
//! the healer, a moody maker or a child). They walk off the map (`away_until`; absent from the
//! camp's reckoning): to a lair at most fifteen days off (25 km a day) and back, plus two days;
//! or out to meet the beast foretold on the road, back within three days and before its raid. On their return the fight at
//! the lair is reckoned like the camp's own (`fight.rs`): their blows against the beast's size;
//! slain, it is a moment, a deed for the hardest striker, the beast dropped from the camp's
//! troubles, and its hoard carried home; each fighter may be wounded or killed by the beast,
//! more likely the bigger it is against their blows.

use super::*;
use crate::persona::{Attr, Facet, Val};

#[derive(Clone, Debug)]
pub struct Expedition {
    pub beast: String,
    pub party: Vec<usize>,
    pub back: u64,
    pub tiles: u64,
    pub monster: Option<crate::monsters::Monster>,
    /// Their blows against it (reckoned as they set out, with the spears they carry) and the
    /// hardest striker.
    pub harm: f32,
    pub striker: Option<usize>,
}

impl Colony {
    /// Dawn: perhaps the speaker sends the militia after a beast.
    pub(crate) fn reckon_expedition(&mut self) {
        let day = self.clock.day();
        if let Some(e) = self.expedition.clone() {
            if day >= e.back { self.expedition_returns(e); }
            return;
        }
        // One dawn in ten; while a beast is foretold, at once (it is coming).
        let urgent = self.arc.as_ref().map_or(false, |a| matches!(a.stage, 1 | 2 | 5) && a.threat.kind == arc::ThreatKind::Beast);
        if day < 30 || self.attackers_out() || (!urgent && crate::history::settlers::hash_pub(self.seed ^ day, 0xE4B) % 10 != 0) { return; }
        let Some(sp) = self.speaker.filter(|&s| self.settlers[s].alive) else { return };
        let p = &self.settlers[sp].persona;
        // Any speaker sends them but one who values peace or is timid (`PLANET_FORCE_HUNT=1`
        // overrides the speaker, for trying the flow).
        let forced = std::env::var("PLANET_FORCE_HUNT").is_ok();
        if !forced && (p.value(Val::Peace) >= 20 || p.facet(Facet::Bravery) < 30) { return; }
        if self.militia_ready().1 < 2 { return; }
        // A beast still to come, laired near.
        let here = (self.map.world_tile.0 as i64, self.map.world_tile.1 as i64);
        // The beast foretold and not yet come, or one still to come; laired within ten tiles.
        let foretold = self.arc.as_ref().filter(|a| matches!(a.stage, 1 | 2 | 5)).map(|a| a.threat.clone()).into_iter();
        let Some(target) = self.arc.as_ref().and_then(|a| foretold.chain(a.later.iter().cloned()).chain(a.reserve.iter().cloned())
            .filter(|t| t.kind == arc::ThreatKind::Beast && t.monster.is_some())
            .filter_map(|t| t.from.map(|l| (((l.0 as i64 - here.0).abs() + (l.1 as i64 - here.1).abs()) as u64, t)))
            .filter(|(d, _)| *d <= 10).min_by_key(|(d, t)| (*d, t.name.clone()))) else { return };
        let (tiles, threat) = target;
        // How far that is on foot (25 km a day): a lair more than fifteen days off is out of
        // reach; the beast foretold is met on the road before it comes.
        let km = tiles.max(1) as f32 * 40_075.0 / self.world_width.max(1) as f32;
        let walk = (km / 25.0).ceil() as u64;
        let meet = self.arc.as_ref().map_or(false, |a| a.threat.name == threat.name && matches!(a.stage, 1 | 2 | 5));
        if !meet && walk > 15 { return; }
        // The party: a monster hunter first, then the best fighters.
        let mood = self.mood.as_ref().filter(|m| !m.done).map(|m| m.who);
        let mut cands: Vec<usize> = (0..self.settlers.len()).filter(|&i| {
            let s = &self.settlers[i];
            s.alive && i != sp && Some(i) != self.healer && Some(i) != mood && s.ill_until <= self.clock.tick && s.past.as_ref().map_or(true, |p| p.age >= 16)
                && self.open_wounds(i).next().is_none() && !self.children.iter().any(|c| c.0 == i)
        }).collect();
        cands.sort_by(|&a, &b| {
            let hunter = |i: usize| self.settlers[i].visitor.as_deref().map_or(false, |v| v.starts_with("a monster hunter"));
            hunter(b).cmp(&hunter(a)).then(self.fight_skill(b).total_cmp(&self.fight_skill(a))).then(a.cmp(&b))
        });
        cands.truncate(5);
        if cands.len() < 3 { return; }
        let raid_day = self.arc.as_ref().map_or(day + 4, |a| a.raid_day);
        let back = if meet { (day + 3).min(raid_day.saturating_sub(1)).max(day + 1) } else { day + 2 * walk + 2 };
        // Their blows: strength, the spear, the hand, against the beast's size.
        let size = threat.monster.as_ref().map_or(1.0, |m| m.size).max(0.6);
        let (mut harm, mut striker, mut hardest) = (0.0f32, None, 0.0f32);
        for &i in &cands {
            let p = &self.settlers[i].persona;
            let skill = self.fight_skill(i);
            let force = self.arm_of(i).map_or(1.0, |a| a.force) * (1.0 + 0.5 * skill);
            let hit = (p.attr(Attr::Agility) / 1000.0 * 0.5 + 0.3 + 0.1 * size + 0.25 * skill).min(0.95);
            let power = p.attr(Attr::Strength) / 1000.0 * force / size;
            harm += hit * power;
            if power > hardest { hardest = power; striker = Some(i); }
        }
        let (spn, names) = (self.settlers[sp].name.clone(), cands.iter().map(|&i| self.settlers[i].name.clone()).collect::<Vec<_>>());
        let line = if meet { format!("{} sends {} out to meet {} on the road, before it reaches the camp.", spn, crate::persona::list(&names), threat.name) }
            else { format!("{} sends {} out to hunt {} in its lair, {} days' walk away.", spn, crate::persona::list(&names), threat.name, walk) };
        self.note(line.clone());
        let at = self.camp;
        self.moment(format!("The hunt for {}", threat.name), line, format!("because {} would rather meet {} out there than at the gate, and the militia is drilled and armed", spn, threat.name), at);
        for &i in &cands {
            self.release(i);
            let s = &mut self.settlers[i];
            s.alive = false;
            s.away_until = back;
            s.why = format!("Away hunting {}", threat.name);
        }
        self.expedition = Some(expedition::Expedition { beast: threat.name.clone(), party: cands, back, tiles: walk, monster: threat.monster.clone(), harm, striker });
    }

    /// The party comes back: the fight at the lair reckoned.
    fn expedition_returns(&mut self, e: expedition::Expedition) {
        self.expedition = None;
        let Some(m) = e.monster.clone() else { return };
        let size = m.size.max(0.6);
        let day = self.clock.day();
        let (harm, striker) = (e.harm, e.striker);
        // Killed at the camp while they were out: they come home to the news.
        if self.slain.contains(&e.beast) {
            for &i in &e.party { let s = &mut self.settlers[i]; s.alive = true; s.away_until = 0; s.pos = self.camp; s.path.clear(); s.job = Job::Idle; }
            let names: Vec<String> = e.party.iter().map(|&i| self.settlers[i].name.clone()).collect();
            self.note(format!("{} come home from the hunt to find {} already dead at the camp.", crate::persona::list(&names), e.beast));
            return;
        }
        let slain = harm >= 1.1 * size;
        // Back from the lair: each may have been struck.
        let mut fallen = Vec::new();
        let mut hurt = Vec::new();
        for (k, &i) in e.party.iter().enumerate() {
            let s = &mut self.settlers[i];
            s.alive = true;
            s.away_until = 0;
            s.pos = self.camp;
            s.path.clear();
            s.job = Job::Idle;
            s.hunger = s.hunger.min(0.6);
            // Armour turns some of what the lair gives (`armour.rs`).
            let worn = self.armour_of(i).map_or(1.0, |a| 1.0 - a.cover);
            let danger = (size / (size + harm)).clamp(0.05, 0.9) * if slain { 0.4 } else { 0.8 } * worn;
            let roll = (crate::history::settlers::hash_pub(self.seed ^ day, 0xE7 + k as u64) % 1000) as f32 / 1000.0;
            if roll < danger * 0.3 { fallen.push(i); } else if roll < danger { hurt.push(i); }
        }
        let names: Vec<String> = e.party.iter().filter(|i| !fallen.contains(i)).map(|&i| self.settlers[i].name.clone()).collect();
        if slain {
            let k = striker.unwrap_or(e.party[0]);
            let kn = self.settlers[k].name.clone();
            let line = format!("The hunting party comes home with the head of {}: {} struck the blow that killed it.", e.beast, kn);
            self.note(line.clone());
            let at = self.camp;
            self.moment(format!("The death of {}", e.beast), line, format!("because {} sent the militia out to hunt it", self.speaker.map(|s| self.settlers[s].name.clone()).unwrap_or_else(|| "the camp".into())), at);
            self.settlers[k].deeds.push(format!("slew {} on the hunt, day {}", e.beast, day));
            self.feel(k, mind::Feel::Slew { what: e.beast.clone() });
            for j in 0..self.settlers.len() { if j != k && self.settlers[j].alive { self.feel(j, mind::Feel::SawFall { what: e.beast.clone() }); } }
            self.vow_kept(&e.beast, Some(k));
            self.slain.push(e.beast.clone());
            if let Some(a) = self.arc.as_mut() {
                a.later.retain(|t| t.name != e.beast);
                a.reserve.retain(|t| t.name != e.beast);
                // The beast foretold will not come now.
                if a.threat.name == e.beast && matches!(a.stage, 1 | 2 | 5) {
                    a.stage = 3;
                    self.note(format!("The watch is stood down: {} will not come now.", e.beast));
                }
            }
            if self.were.as_deref() == Some(e.beast.as_str()) { self.were = None; }
            if !m.hoard.is_empty() {
                self.note(format!("From its lair they bring {}.", crate::persona::list(&m.hoard)));
                for h in &m.hoard { self.treasures.push(format!("{}, from the hoard of {}", h, e.beast)); }
            }
        } else {
            self.note(format!("The hunting party comes home without {}: {} could not bring it down.", e.beast, crate::persona::list(&names)));
            for &i in &e.party { if !fallen.contains(&i) { self.feel(i, mind::Feel::Struck); } }
        }
        for &i in &hurt {
            let part = ["left arm", "right leg", "body", "right arm"][(crate::history::settlers::hash_pub(day, i as u64) % 4) as usize];
            let sev = if size > 1.6 { 2 } else { 1 };
            self.wound(i, part, sev, format!("{}'s lair, day {}", e.beast, day));
        }
        for &i in &fallen {
            let name = self.settlers[i].name.clone();
            self.note(format!("{} did not come back: {} fell at the lair of {}.", name, if self.settlers[i].persona.female { "she" } else { "he" }, e.beast));
            self.bury(i, &format!("at the lair of {}, slain by the beast", e.beast));
        }
    }
}
