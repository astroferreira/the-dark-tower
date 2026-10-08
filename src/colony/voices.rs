//! Voices: the camp's next work is argued for by its people (DF's dwarves make their needs and
//! preferences felt; here, with no overseer, they are what steers the plan). Each grown settler
//! leans toward the works that would meet their unmet needs or suit their character: the devout
//! toward a temple, the sociable and the drinkers toward a tavern, the anxious toward the wall and
//! the lookout, the lover of beasts toward a pen, the greedy toward the mine. The speaker's voice
//! counts twice. A work's urgency rises with the camp's support (1x unheard .. ~1.75x pressed by
//! all), the loudest voice is named in the plan ("Thano presses for it: he has gone 9 days
//! without prayer"), and a strongly pressed work may be raised before its usual time.

use super::*;
use super::needs::Need;
use super::projects::ProjectKind;
use crate::persona::{Facet, Val};

impl Colony {
    /// How much settler `i` wants `kind` built (0..~1.5), and in their own words why.
    pub(crate) fn voice(&self, i: usize, kind: ProjectKind) -> (f32, String) {
        let s = &self.settlers[i];
        let p = &s.persona;
        let they = if p.female { "she" } else { "he" };
        let day = self.clock.day();
        let fac = |x: Facet| ((p.facet(x) as f32 - 50.0) / 50.0).max(0.0);
        let val = |x: Val| (p.value(x) as f32 / 50.0).max(0.0);
        // A need's pull: its strength, more the longer it has gone unmet.
        let need = |n: Need| -> (f32, String) {
            match s.mind.needs.iter().find(|x| x.need == n) {
                Some(x) => {
                    let unmet = (-x.focus as f32 / needs::FULL as f32).clamp(0.0, 1.0);
                    let days = day.saturating_sub(x.met_day.max(1));
                    let why = if x.focus < 0 && days >= 2 { format!("{} has gone {} days without {}", they, days, n.word()) } else { format!("{} longs for {}", they, n.word()) };
                    (x.level as f32 / 10.0 * (0.5 + unmet), why)
                }
                None => (0.0, String::new()),
            }
        };
        let best = |v: Vec<(f32, String)>| v.into_iter().max_by(|a, b| a.0.total_cmp(&b.0)).unwrap_or((0.0, String::new()));
        let trait_ = |w: f32, why: &str| (w, format!("{} {}", they, why));
        match kind {
            ProjectKind::Temple => need(Need::Pray),
            ProjectKind::Tavern => best(vec![need(Need::Socialize), need(Need::Drink), need(Need::MakeMerry)]),
            ProjectKind::Library => best(vec![need(Need::Learn), need(Need::ThinkAbstractly)]),
            ProjectKind::Workshop | ProjectKind::GuildHall => best(vec![need(Need::Craft), need(Need::BeCreative)]),
            ProjectKind::Still => need(Need::Drink),
            ProjectKind::Kitchen => need(Need::GoodMeal),
            ProjectKind::Pen => need(Need::SeeAnimal),
            ProjectKind::Lookout => best(vec![need(Need::Excitement), trait_(fac(Facet::Anxiety) * 0.8, "starts at every sound in the night")]),
            ProjectKind::Palisade | ProjectKind::Traps | ProjectKind::Moat | ProjectKind::Drawbridges | ProjectKind::Mending =>
                best(vec![trait_(fac(Facet::Anxiety) * 0.9, "cannot sleep for fear of what comes"), trait_(val(Val::MartialProwess) * 0.7, "would meet trouble behind a wall of spears")]),
            ProjectKind::GreatHall => best(vec![need(Need::Socialize), need(Need::MakeMerry), need(Need::Friends)]),
            ProjectKind::Bedrooms => best(vec![trait_(fac(Facet::Bashfulness) * 0.8, "would sleep away from the others"), trait_(val(Val::Family) * 0.6, "wants a room for a family")]),
            ProjectKind::Mine | ProjectKind::DeepShaft => best(vec![trait_(fac(Facet::Greed) * 0.9, "dreams of the ore under the hill"), trait_(fac(Facet::Curiosity) * 0.5, "wants to know what lies below")]),
            ProjectKind::Storehouse | ProjectKind::Fence => trait_(fac(Facet::Orderliness) * 0.7, "cannot bear the store lying about"),
            ProjectKind::Field | ProjectKind::CaveFarm | ProjectKind::Smokehouse | ProjectKind::Jetty =>
                best(vec![trait_(val(Val::Family) * 0.4, "thinks of the children's winter"), trait_(fac(Facet::Anxiety) * 0.4, "fears a hungry winter")]),
            _ => (0.0, String::new()),
        }
    }

    /// The camp's support for `kind` (0..~1.5: the grown settlers' voices, the speaker's twice)
    /// and its loudest voice when it is loud (who, why).
    pub(crate) fn support(&self, kind: ProjectKind) -> (f32, Option<(usize, String)>) {
        let mut sum = 0.0;
        let mut weight = 0.0;
        let mut loudest: Option<(f32, usize, String)> = None;
        for i in 0..self.settlers.len() {
            let s = &self.settlers[i];
            if !s.alive || s.mind.left || s.guest_until > 0 || s.past.as_ref().map_or(false, |p| p.age < 14) { continue; }
            let (v, why) = self.voice(i, kind);
            let w = if self.speaker == Some(i) { 2.0 } else { 1.0 };
            sum += v * w;
            weight += w;
            if v >= 0.55 && loudest.as_ref().map_or(true, |l| v * w > l.0) { loudest = Some((v * w, i, why)); }
        }
        if weight == 0.0 { return (0.0, None); }
        (sum / weight, loudest.map(|l| (l.1, l.2)))
    }

    /// Scale each candidate by the camp's support and name the loudest voice in its reason.
    pub(crate) fn weigh_voices(&self, c: &mut Vec<(f32, ProjectKind, String, u32, Pos)>) {
        for cand in c.iter_mut() {
            let (sup, loud) = self.support(cand.1);
            // (Pressed works rise; a work nobody argues for keeps its own urgency: a field
            // nobody loves still feeds them.)
            cand.0 *= 1.0 + 0.5 * sup.min(1.5);
            if let Some((i, why)) = loud.filter(|_| !cand.2.contains("presses for it")) {
                cand.2.push_str(&format!("; {} presses for it: {}", self.settlers[i].name, why));
            }
        }
    }

    /// Works a camp's people press for before their usual time: a temple for two who pray, a
    /// tavern for a sociable camp, a workshop for makers, a library for the learned, a pen for
    /// those who love beasts. (Each only when the loudest voice is loud and the camp leans its way.)
    pub(crate) fn pressed_candidates(&self, c: &mut Vec<(f32, ProjectKind, String, u32, Pos)>) {
        let day = self.clock.day();
        let alive = self.alive();
        let mut early = |kind: ProjectKind, min_day: u64, min_alive: usize, size: (u16, u16), loads: u32, what: &str| {
            if self.projects.iter().any(|p| p.kind == kind) || c.iter().any(|x| x.1 == kind) || day < min_day || alive < min_alive { return; }
            let (sup, loud) = self.support(kind);
            let Some((i, why)) = loud else { return };
            if sup < 0.3 { return; }
            if let Some(at) = self.find_site_for(kind, size.0, size.1) {
                c.push((0.9 + sup, kind, format!("{}; {} presses for it: {}", what, self.settlers[i].name, why), loads, at));
            }
        };
        if self.settlers.iter().filter(|s| s.alive && s.past.as_ref().map_or(false, |p| p.faith.is_some()) && s.persona.facet(Facet::Piety) >= 60).count() >= 2 {
            early(ProjectKind::Temple, 12, 6, (4, 4), 16, "the devout among them have no roof to pray under");
        }
        early(ProjectKind::Tavern, 25, 8, (5, 4), 16, "the evenings are long and there is nowhere to share a cup");
        let laid: u32 = self.settlers.iter().map(|s| s.loads_laid).sum();
        if laid >= 25 { early(ProjectKind::Workshop, 8, 5, (5, 4), 14, "they have no bench to make anything at"); }
        if self.works.iter().any(|w| w.kind == "book") { early(ProjectKind::Library, 30, 8, (4, 3), 12, "what they know is kept in nobody's head but their own"); }
        if self.herd_near().is_some() { early(ProjectKind::Pen, 20, 6, (6, 5), 10, "a herd grazes near, and some of them would keep beasts"); }
    }
}
