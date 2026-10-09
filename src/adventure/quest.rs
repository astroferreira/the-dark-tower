//! Work the world gives: a lord's commission to slay a beast of the history (DF's quests from a
//! site's ruler), the guard captain's bounties (Tibia's tasks: so many of a kind of monster), the
//! priest's charge to lay the risen of a tomb to rest, the sage's search for a treasure the
//! history lost. Each quest names its place and why; it is reported back to whoever gave it.

use super::actor::Monster;
use super::game::{Game, Tone};
use super::item::Item;
use super::site::SiteKind;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Goal {
    /// Kill the named boss of a place.
    Slay { site: u32, boss: String },
    /// Kill `count` of a kind (by definition id).
    Bounty { def: String, count: u32, done: u32 },
    /// Find a treasure (by its tag) and bring it back.
    Fetch { tag: u32, name: String, site: u32 },
    /// Carry a sealed parcel (by its tag) to the trader of a town.
    Deliver { tag: u32, town: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum State { Open, Done, Rewarded }

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Quest {
    pub id: u32,
    /// The town and the name of whoever gave it.
    pub town: u32,
    pub giver: String,
    pub title: String,
    pub text: String,
    pub goal: Goal,
    pub gold: u32,
    pub xp: u64,
    pub item: Option<Item>,
    pub state: State,
}

impl Quest {
    pub fn progress(&self) -> String {
        match (&self.goal, self.state) {
            (_, State::Rewarded) => "done".into(),
            (_, State::Done) => format!("go back to {}", self.giver),
            (Goal::Bounty { count, done, .. }, _) => format!("{} of {}", done, count),
            (Goal::Slay { .. }, _) => "not yet".into(),
            (Goal::Fetch { .. }, _) => "not found yet".into(),
            (Goal::Deliver { .. }, _) => "on the road".into(),
        }
    }
}

/// Something slain: bounties count it, a slaying is done.
pub fn on_kill(g: &mut Game, m: &Monster) {
    let mut lines = Vec::new();
    for q in g.quests.iter_mut().filter(|q| q.state == State::Open) {
        match &mut q.goal {
            Goal::Bounty { def, count, done } if *def == m.def => {
                *done += 1;
                if *done >= *count { q.state = State::Done; lines.push(format!("Bounty met: {}. Go back to {}.", q.title, q.giver)); }
                else if *done % 5 == 0 { lines.push(format!("{}: {} of {}.", q.title, done, count)); }
            }
            Goal::Slay { boss, .. } if m.boss && *boss == m.name => { q.state = State::Done; lines.push(format!("{} is dead. Go back to {} for your reward.", m.name, q.giver)); }
            _ => {}
        }
    }
    for l in lines { g.say(Tone::Quest, l); }
}

/// Something found: a sought treasure.
pub fn on_found(g: &mut Game, it: &Item) {
    if it.is_artifact() {
        let t = g.turn; let place = g.place().map(|p| p.spec.name.clone()).unwrap_or_default();
        g.deeds.push((t, format!("found {} in {}", it.short(), place)));
        let who = g.hero.name.clone();
        g.chronicle(super::living::DeedKind::RelicFound(it.tag as u64), format!("{} found {}", who, it.short()), format!("{} found {} in {}, lost for long years.", who, it.short(), place));
    }
    if it.tag == 0 || it.id == "key" { return; }
    let mut lines = Vec::new();
    for q in g.quests.iter_mut().filter(|q| q.state == State::Open) {
        if let Goal::Fetch { tag, name, .. } = &q.goal { if *tag == it.tag { q.state = State::Done; lines.push(format!("You have found {}. Take it to {}.", name, q.giver)); } }
    }
    for l in lines { g.say(Tone::Quest, l); }
}

fn dist(a: (usize, usize), b: (usize, usize), w: usize) -> i32 {
    let dx = (a.0 as i32 - b.0 as i32).abs();
    let dx = dx.min(w as i32 - dx);
    dx.max((a.1 as i32 - b.1 as i32).abs())
}

/// Compass word from a to b.
pub fn direction(a: (usize, usize), b: (usize, usize), w: usize) -> &'static str {
    let mut dx = b.0 as i32 - a.0 as i32;
    if dx > w as i32 / 2 { dx -= w as i32; } else if dx < -(w as i32 / 2) { dx += w as i32; }
    let dy = b.1 as i32 - a.1 as i32;
    let (ax, ay) = (dx.abs() as f32, dy.abs() as f32);
    if ax < 0.4 * ay { if dy < 0 { "north" } else { "south" } }
    else if ay < 0.4 * ax { if dx < 0 { "west" } else { "east" } }
    else { match (dx < 0, dy < 0) { (true, true) => "north-west", (false, true) => "north-east", (true, false) => "south-west", (false, false) => "south-east" } }
}

/// A commission from `role` in town `town` suited to the hero's level, not already given.
pub fn offer(g: &Game, town: u32, giver: &str, role: super::actor::Role) -> Option<Quest> {
    use super::actor::Role;
    let home = g.site(town)?.tile;
    let w = g.world.w;
    let lvl = g.hero.level;
    let taken = |site: u32| g.quests.iter().any(|q| matches!(&q.goal, Goal::Slay { site: s, .. } | Goal::Fetch { site: s, .. } if *s == site))
        || g.site(site).and_then(|s| s.boss.as_ref()).map_or(false, |b| g.slain.contains(&b.name));
    let id = g.quests.len() as u32 + 1;
    // Fit: a place's tier against the hero's level (tier 1 ~ level 1-6, 2 ~ 6-14, 3 ~ 12-24...).
    let fits = |tier: u32| { let lo = (tier.saturating_sub(1)) * 7; lvl + 3 >= lo.max(4) && lvl <= lo + 22 };
    match role {
        Role::Lord => {
            let s = g.sites.iter().filter(|s| matches!(s.kind, SiteKind::Lair | SiteKind::Camp | SiteKind::Cave | SiteKind::Castle | SiteKind::Labyrinth | SiteKind::Halls) && s.boss.is_some() && fits(s.tier) && !taken(s.id))
                .min_by_key(|s| (dist(s.tile, home, w), s.id))?;
            let b = s.boss.as_ref()?;
            let d = dist(s.tile, home, w);
            Some(Quest { id, town, giver: giver.into(), title: format!("Slay {}", b.name), goal: Goal::Slay { site: s.id, boss: b.name.clone() },
                text: format!("{} troubles the land. Go to {}, {} tiles {} of here, and kill it. {}", b.name, s.name, d, direction(home, s.tile, w), b.story),
                gold: 60 * s.tier * s.tier + 20 * d as u32, xp: (80 * s.tier * s.tier * s.tier) as u64, item: None, state: State::Open })
        }
        Role::Priest => {
            // The risen dead only (a ruin's bandit chief is the lord's business).
            let undead = |s: &super::site::SiteSpec| s.boss.as_ref().and_then(|b| super::data::data().monster(&b.def)).map_or(false, |m| m.undead);
            let s = g.sites.iter().filter(|s| matches!(s.kind, SiteKind::Tomb | SiteKind::Temple | SiteKind::Shrine | SiteKind::Ruin | SiteKind::Castle) && undead(s) && fits(s.tier) && !taken(s.id))
                .min_by_key(|s| (dist(s.tile, home, w), s.id))?;
            let b = s.boss.as_ref()?;
            let d = dist(s.tile, home, w);
            Some(Quest { id, town, giver: giver.into(), title: format!("Lay {} to rest", b.name), goal: Goal::Slay { site: s.id, boss: b.name.clone() },
                text: format!("The dead of {} walk. {} Go there ({} tiles {}) and end it, and the god will remember you.", s.name, b.story, d, direction(home, s.tile, w)),
                gold: 40 * s.tier * s.tier, xp: (90 * s.tier * s.tier * s.tier) as u64, item: Some(super::item::Item::new(if s.tier >= 3 { "strong_health_potion" } else { "health_potion" }, 3)), state: State::Open })
        }
        Role::Guard => {
            // A bounty on what lives near: rats at first, then what the nearest places hold.
            let tier = (lvl / 7 + 1).min(5);
            let pool: Vec<&super::data::MonsterDef> = super::data::data().monsters.iter().filter(|m| m.tier == tier || (tier > 1 && m.tier == tier - 1)).filter(|m| !g.quests.iter().any(|q| matches!(&q.goal, Goal::Bounty { def, .. } if *def == m.id) && q.state != State::Rewarded)).collect();
            if pool.is_empty() { return None; }
            let m = pool[(g.turn as usize / 977 + id as usize) % pool.len()];
            let count = (12 + lvl / 2).min(30);
            Some(Quest { id, town, giver: giver.into(), title: format!("Bounty: {} {}", count, super::item::plural(&m.name)), goal: Goal::Bounty { def: m.id.clone(), count, done: 0 },
                text: format!("Too many {} about. Kill {} of them and I'll pay the bounty. Where they live: {}.", super::item::plural(&m.name), count, m.habitats.iter().map(|h| h.replace('_', " ")).collect::<Vec<_>>().join(", ")),
                gold: m.xp * count / 2 + 10, xp: (m.xp * count) as u64 / 2, item: None, state: State::Open })
        }
        Role::Trader => {
            // A parcel for the trader of a town farther off (a reason to walk the world).
            // Once for each town it can be sent to, and not again for a while (the road is the work,
            // not a living: a parcel pays in gold more than in experience).
            let sent: Vec<u32> = g.quests.iter().filter(|q| q.town == town).filter_map(|q| match q.goal { Goal::Deliver { town, .. } => Some(town), _ => None }).collect();
            let last = g.quests.iter().filter(|q| q.town == town && matches!(q.goal, Goal::Deliver { .. })).count();
            if last > 0 && g.turn < (last as u64) * 400_000 { return None; }
            let t = g.sites.iter().filter(|s| s.kind == SiteKind::Town && s.id != town && !sent.contains(&s.id) && { let d = dist(s.tile, home, w); d >= 4 && d <= 6 + lvl as i32 } && g.world.reachable(home, s.tile)).min_by_key(|s| (dist(s.tile, home, w), s.id))?;
            let d = dist(t.tile, home, w);
            let tag = 0x0D00_0000 + id;
            Some(Quest { id, town, giver: giver.into(), title: format!("A parcel for {}", t.name), goal: Goal::Deliver { tag, town: t.id },
                text: format!("Take this sealed parcel to the trader of {}, {} days' walk {} of here. Don't open it. They'll pay you there.", t.name, d, direction(home, t.tile, w)),
                gold: 30 + 25 * d as u32, xp: 20 * d as u64, item: None, state: State::Open })
        }
        Role::Sage => {
            let (s, t) = g.sites.iter().filter(|s| !taken(s.id)).flat_map(|s| s.treasures.iter().map(move |t| (s, t))).min_by_key(|(s, _)| (dist(s.tile, home, w), s.id))?;
            let d = dist(s.tile, home, w);
            Some(Quest { id, town, giver: giver.into(), title: format!("Find {}", t.short()), goal: Goal::Fetch { tag: t.tag, name: t.short(), site: s.id },
                text: format!("{} {} It lies, if the old accounts are true, in {}, {} tiles {} of here. Bring it to me.", t.short(), t.story.clone().unwrap_or_default(), s.name, d, direction(home, s.tile, w)),
                gold: 300 + 80 * s.tier * s.tier, xp: (150 * s.tier * s.tier * s.tier) as u64, item: None, state: State::Open })
        }
        _ => None,
    }
}

/// Report a finished quest to its giver: the reward.
pub fn report(g: &mut Game, k: usize) -> Vec<String> {
    let mut out = Vec::new();
    let q = g.quests[k].clone();
    if q.state != State::Done { return out; }
    if let Goal::Fetch { tag, name, .. } = &q.goal {
        let Some(i) = g.hero.pack.iter().position(|i| i.tag == *tag) else { out.push(format!("You have not got {} with you.", name)); return out };
        g.hero.pack.remove(i);
        out.push(format!("You hand over {}.", name));
    }
    g.quests[k].state = State::Rewarded;
    g.stats.quests_done += 1;
    if !matches!(q.goal, Goal::Bounty { .. } | Goal::Deliver { .. }) {
        let t = g.turn; g.deeds.push((t, format!("finished \"{}\" for {}", q.title, q.giver)));
        let who = g.hero.name.clone();
        g.chronicle(super::living::DeedKind::QuestDone, format!("{} did the bidding of {}", who, q.giver), format!("{} finished \"{}\" for {}.", who, q.title, q.giver));
    }
    super::item::stow(&mut g.hero.pack, Item::new("gold", q.gold));
    out.push(format!("\"Well done.\" {} pays you {} gold.", q.giver, q.gold));
    if let Some(it) = q.item { out.push(format!("And gives you {}.", it.describe())); super::item::stow(&mut g.hero.pack, it); }
    let ups = g.hero.gain_xp(q.xp);
    out.push(format!("You gain {} experience.", q.xp));
    for l in ups { out.push(format!("You advanced to level {}.", l)); }
    out
}
