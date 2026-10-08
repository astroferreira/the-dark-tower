//! The deep shaft: adamantine, and the hollow under the world.
//!
//! The idea is Dwarf Fortress's deepest secret: below the caverns, glittering adamantine runs in
//! spires down into the dark, and those who mine it out to the bottom break into a hollow where
//! demons wait. Here, once the mine has broken into a cavern and nothing of the deep is still
//! coming (the forgotten beast slain, caged or long gone), a camp that builds in stone, or whose
//! speaker is greedy (60+), sinks a deep shaft below the cavern floor (`ProjectKind::DeepShaft`, 24
//! loads of timber shoring; the work is told, not walked). When it is done the shaft has reached
//! the deepest rock: where the land has three cavern layers, one camp in two strikes adamantine
//! ("a vein of adamantine, glittering blue-white", a moment; `Colony::ores` gains it, and spearheads
//! of it strike at 2.2). Greed follows the vein: 12-20 days later the miners break into a hollow
//! that should not be there, and something comes up out of it: a demon (a generated monster of
//! fire and darkness, size 3.5, not to be caged or easily slain) as the next trouble of the camp,
//! with a sound from below first. Once only.

use super::*;
use super::arc::{Threat, ThreatKind};

impl Colony {
    /// Whether the camp would sink the deep shaft now.
    pub(crate) fn wants_deep_shaft(&self) -> bool {
        if self.breached.is_empty() || self.clock.day() < 60 { return false; }
        // Nothing of the deep still coming.
        if self.arc.as_ref().map_or(false, |a| a.threat.kind == ThreatKind::Deep && matches!(a.stage, 0 | 1 | 2 | 5) || a.later.iter().any(|t| t.kind == ThreatKind::Deep)) { return false; }
        let masons = self.way.as_ref().map_or(false, |w| w.stone_first);
        let greedy = self.speaker.filter(|&s| self.settlers[s].alive).map_or(false, |s| self.settlers[s].persona.facet(crate::persona::Facet::Greed) >= 60);
        masons || greedy
    }

    /// The shaft is done: the deepest rock, and perhaps adamantine.
    pub(crate) fn deep_shaft_done(&mut self) {
        let day = self.clock.day();
        let three = self.map.caverns.len() >= 3;
        let struck = three && crate::history::settlers::hash_pub(self.seed, 0xADA) % 2 == 0;
        let at = self.mine_mouth().unwrap_or(self.camp);
        if struck {
            if !self.ores.iter().any(|o| o == "adamantine") { self.ores.push("adamantine".into()); }
            let line = "At the bottom of the deep shaft the miners strike a vein of adamantine, glittering blue-white, running down into the dark.".to_string();
            self.note(line.clone());
            self.moment("Adamantine".into(), line, "because they sank the deep shaft past the caverns into the deepest rock".into(), at);
            for j in 0..self.settlers.len() { if self.settlers[j].alive { self.feel(j, mind::Feel::Breach { what: "the adamantine".into() }); } }
            self.hollow_day = Some(day + 12 + crate::history::settlers::hash_pub(self.seed, 0x401) % 9);
        } else {
            self.note("The deep shaft reaches the deepest rock, black and barren; the miners find nothing worth the labour.".into());
        }
    }

    /// Dawn: following the vein down, the miners break into the hollow.
    pub(crate) fn reckon_hollow(&mut self) {
        let Some(d) = self.hollow_day else { return };
        if self.clock.day() < d || self.alive() == 0 { return; }
        self.hollow_day = None;
        let req = crate::monsters::Request { kind: "forgotten".into(), spheres: vec!["fire".into(), "darkness".into()], size: 3.5, evil: true, ..Default::default() };
        let m = crate::monsters::generate(&req, self.seed ^ 0xDE40);
        let name = {
            use rand::SeedableRng;
            let style = crate::history::naming::styles::NamingStyle::from_archetype(crate::history::NamingStyleId(0), crate::history::entities::races::RaceType::from_tag("demon").default_naming_archetype());
            let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(self.seed ^ 0xDE41);
            crate::history::naming::generator::NameGenerator::personal_name(&style, &mut rng).split_whitespace().next().unwrap_or("Ulgrath").to_string()
        };
        let at = self.mine_mouth().unwrap_or(self.camp);
        let line = format!("Following the adamantine down, the miners break into a hollow that should not be there: the air is hot, and something laughs in the dark below.");
        self.note(line.clone());
        self.moment("The hollow".into(), line, "because they followed the adamantine to its root".into(), at);
        for j in 0..self.settlers.len() { if self.settlers[j].alive { self.feel(j, mind::Feel::TheDeep { what: "the hollow under the world".into() }); } }
        let t = Threat {
            kind: ThreatKind::Deep, name: format!("{} the demon", name),
            why: "the miners broke into the hollow under the world".into(), cause: None,
            cause_text: Some(format!("the deep shaft broke into the hollow on day {}", self.clock.day())),
            faction: None, from: None, size: 3.5, monster: Some(m),
        };
        if let Some(a) = self.arc.as_mut() {
            a.later.insert(0, t);
            if a.stage == 3 { a.quiet_until = Some(self.clock.day() + 3); }
        }
    }
}
