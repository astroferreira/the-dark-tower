//! Tales: work that comes from the world's state, with choices (DF's quests grow from what the
//! world is; Tibia's quests have their twists). Six kinds, each read off the world and each
//! with more than one way through, and an end that changes the town:
//! - a child snatched by the raiders of a camp, cave or lair near (fight for them, or pay);
//! - a caravan that never came, on a real trade route of the history (return the goods, or keep
//!   them);
//! - a cult meeting in a cellar under the town (a cell of a history's cult, or the Shadow's
//!   servants): its keeper's ledger names a townsman (expose them to the lord, or sell them their
//!   silence);
//! - a feud between two houses of the town (make peace, or side with one: the other leaves);
//! - a beast of the history the town pays tribute to (carry it, end it, or keep the gold);
//! - a plague and the herb that cures it in a place near (bring it to the priest, or sell it to
//!   a rival town).
//! Choices are cards (`Game::choice`, answered by `Action::Decide`).

use super::actor::Role;
use super::game::{Game, Tone};
use super::item::Item;
use super::quest::{Goal, Quest, State};
use super::site::SiteKind;
use super::world::dist;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TaleKind { Snatched, Caravan, Cult, Feud, Tribute, Plague }

impl TaleKind {
    pub fn word(self) -> &'static str {
        match self { TaleKind::Snatched => "a snatched child", TaleKind::Caravan => "a lost caravan", TaleKind::Cult => "a cult under the town", TaleKind::Feud => "a feud", TaleKind::Tribute => "a beast's tribute", TaleKind::Plague => "a plague" }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Tale {
    pub kind: TaleKind,
    pub stage: u8,
    /// The place it leads to (0: none).
    pub site: u32,
    /// The world tile it leads to.
    pub tile: (usize, usize),
    /// The other one in it: the child, the other house, the townsman in the ledger, the beast.
    pub other: String,
    /// Another town in it (the caravan's partner, the rival that would buy the cure).
    pub other_town: u32,
    /// The thing's tag (the goods, the ledger, the herb, the tribute).
    pub tag: u32,
    /// The price in it (the ransom, the tribute, the rival's offer).
    pub price: u32,
    /// What was chosen (0: nothing yet).
    pub chose: u8,
}

/// A choice to make: (words, code) options.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Choice { pub quest: u32, pub title: String, pub text: String, pub options: Vec<(String, u8)> }

fn tale(g: &Game, id: u32) -> Option<Tale> { g.quests.iter().find(|q| q.id == id).and_then(|q| match &q.goal { Goal::Tale(t) => Some(t.clone()), _ => None }) }
fn set_tale(g: &mut Game, id: u32, f: impl FnOnce(&mut Tale, &mut Quest)) {
    if let Some(q) = g.quests.iter_mut().find(|q| q.id == id) { let mut t = match &q.goal { Goal::Tale(t) => t.clone(), _ => return }; f(&mut t, q); q.goal = Goal::Tale(t); }
}

/// A tale `role` of town `town` might give: one of each kind a town, the hero's level fitting.
pub fn offer(g: &Game, town: u32, giver: &str, role: Role) -> Option<Quest> {
    let ts = g.site(town)?.clone();
    if ts.kind != SiteKind::Town { return None; }
    let home = ts.tile;
    let w = g.world.w;
    let lvl = g.hero.level;
    let had = |k: TaleKind| g.quests.iter().any(|q| q.town == town && matches!(&q.goal, Goal::Tale(t) if t.kind == k));
    let fits = |tier: u32| { let lo = tier.saturating_sub(1) * 7; lvl + 3 >= lo.max(3) && lvl <= lo + 22 };
    let id = g.quests.len() as u32 + 1;
    let tag = 0x7A1E_0000 + id;
    let mk = |kind: TaleKind, site: u32, tile: (usize, usize), other: String, other_town: u32, price: u32, title: String, text: String, gold: u32, xp: u64| Quest {
        id, town, giver: giver.into(), title, text, gold, xp, item: None, state: State::Open,
        goal: Goal::Tale(Tale { kind, stage: 0, site, tile, other, other_town, tag, price, chose: 0 }) };
    let near_sites = |kinds: &[SiteKind], within: i32| -> Option<super::site::SiteSpec> {
        g.sites.iter().filter(|s| kinds.contains(&s.kind) && s.boss.is_some() && fits(s.tier) && dist(s.tile, home, w) <= within && !s.boss.as_ref().map_or(false, |b| g.slain.contains(&b.name)))
            .min_by_key(|s| (dist(s.tile, home, w), s.id)).cloned()
    };
    let try_kind = |kind: TaleKind| -> Option<Quest> {
        match kind {
        TaleKind::Snatched => {
            let s = near_sites(&[SiteKind::Camp, SiteKind::Cave, SiteKind::Lair, SiteKind::Ruin], 5)?;
            let b = s.boss.as_ref()?;
            let child = super::town::person_name(&ts.people, ts.seed ^ 0xC41D);
            let ransom = 40 + 30 * s.tier;
            let d = dist(s.tile, home, w);
            Some(mk(TaleKind::Snatched, s.id, s.tile, child.clone(), 0, ransom, format!("Bring {} home", child),
                format!("Three nights ago raiders out of {} took {}, the miller's child, from the edge of the fields. {} holds them there, {} days' walk {}. Bring the child home, with the sword or with their price; I care not which.", s.name, child, b.name, d.max(1), super::quest::direction(home, s.tile, w)),
                50 * s.tier * s.tier, (100 * s.tier * s.tier) as u64))
        }
        TaleKind::Caravan if lvl >= 4 => {
            let h = g.history.clone()?;
            let me = ts.settlement.map(crate::history::SettlementId)?;
            let mut routes: Vec<_> = h.trade_routes.values().filter(|r| r.endpoints.0 == me || r.endpoints.1 == me).collect();
            routes.sort_by_key(|r| r.id);
            let (partner, ptile, pname) = routes.iter().filter_map(|r| { let o = if r.endpoints.0 == me { r.endpoints.1 } else { r.endpoints.0 }; g.sites.iter().find(|s| s.settlement == Some(o.0) && s.kind == SiteKind::Town).map(|s| (s.id, s.tile, s.name.clone())) })
                .chain(g.sites.iter().filter(|s| s.kind == SiteKind::Town && s.id != town && dist(s.tile, home, w) >= 3 && dist(s.tile, home, w) <= 9).min_by_key(|s| (dist(s.tile, home, w), s.id)).map(|s| (s.id, s.tile, s.name.clone())))
                .next()?;
            // The wreck: a land tile on the road between, a third of the way.
            let mut wreck = home;
            for k in 1..20 {
                let f = k as f32 / 20.0 * 0.6 + 0.2;
                let mut dx = ptile.0 as i32 - home.0 as i32; if dx > w as i32 / 2 { dx -= w as i32 } else if dx < -(w as i32 / 2) { dx += w as i32 }
                let x = (home.0 as i32 + (dx as f32 * f) as i32).rem_euclid(w as i32) as usize;
                let y = (home.1 as f32 + (ptile.1 as f32 - home.1 as f32) * f) as usize;
                if g.world.land[y * w + x] && (x, y) != home && (x, y) != ptile && !g.sites.iter().any(|s| s.tile == (x, y)) { wreck = (x, y); break; }
            }
            if wreck == home { return None; }
            let worth = 120 + 15 * lvl;
            Some(mk(TaleKind::Caravan, 0, wreck, pname.clone(), partner, worth, format!("The caravan to {}", pname),
                format!("Four wagons left here for {} a month ago, and never came there. The road runs {} through {}. Find what became of them. What they carried is worth {} gold to me, and the drivers had families.", pname, super::quest::direction(home, wreck, w), g.region_name(wreck), worth),
                worth / 2, 60 + 20 * dist(wreck, home, w) as u64))
        }
        TaleKind::Cult => {
            let cell = g.sites.iter().find(|s| s.kind == SiteKind::Cellar && s.tile == home && fits(s.tier.min(4)) && !s.boss.as_ref().map_or(false, |b| g.slain.contains(&b.name)))?.clone();
            // The townsman the ledger will name: one of the town's own.
            let suspects: Vec<String> = g.place().map(|p| p.npcs.iter().filter(|n| n.home == town && n.role == Role::Townsfolk && n.of != "drunk").map(|n| n.name.clone()).collect()).unwrap_or_default();
            let other = suspects.get((cell.seed % suspects.len().max(1) as u64) as usize).cloned().unwrap_or_else(|| "the steward".into());
            Some(mk(TaleKind::Cult, cell.id, home, other, 0, 0, "Chanting under the streets".into(),
                format!("People vanish from {} at night, and the watch hears chanting under the streets. Someone in this town opens a door to them. Find where they meet, under one of our own houses, and end it.", ts.name),
                90 * cell.tier.min(4), (140 * cell.tier.min(4) * cell.tier.min(4)) as u64))
        }
        TaleKind::Tribute => {
            let lair = g.sites.iter().filter(|s| s.kind == SiteKind::Lair && s.creature.is_some() && dist(s.tile, home, w) <= 6 && !s.boss.as_ref().map_or(true, |b| g.slain.contains(&b.name))).min_by_key(|s| (dist(s.tile, home, w), s.id))?.clone();
            let b = lair.boss.as_ref()?;
            let price = 60 + 20 * lair.tier;
            Some(mk(TaleKind::Tribute, lair.id, lair.tile, b.name.clone(), 0, price, format!("Tribute for {}", b.name),
                format!("Every season {} pays {} gold of tribute, left at the mouth of {}, so that {} spares our herds and our children. This season it falls to you to carry it, {} days {}. Or to end it, if you are what the songs say.", ts.name, price, lair.name, b.name, dist(lair.tile, home, w).max(1), super::quest::direction(home, lair.tile, w)),
                30 + price / 2, (60 * lair.tier) as u64))
        }
        TaleKind::Plague if lvl >= 3 => {
            let s = near_sites(&[SiteKind::Temple, SiteKind::Tomb, SiteKind::Ruin, SiteKind::Castle, SiteKind::Shrine, SiteKind::Cave], 6)?;
            let rival = g.sites.iter().filter(|r| r.kind == SiteKind::Town && r.id != town).min_by_key(|r| (dist(r.tile, home, w), r.id)).map(|r| r.id).unwrap_or(0);
            Some(mk(TaleKind::Plague, s.id, s.tile, "moonpetal".into(), rival, 300 + 20 * lvl, "A cure for the wasting".into(),
                format!("A wasting sickness is in {}: eleven dead since the thaw, and more abed. The old books say moonpetal cures it, and that it grows only in the damp dark of {}, {} days {}. Bring it to me.", ts.name, s.name, dist(s.tile, home, w).max(1), super::quest::direction(home, s.tile, w)),
                60 * s.tier, (120 * s.tier * s.tier) as u64))
        }
        TaleKind::Feud => {
            let others: Vec<String> = g.place().map(|p| p.npcs.iter().filter(|n| n.home == town && n.role == Role::Townsfolk && n.name != giver && n.of != "drunk" && n.of != "farmer").map(|n| n.name.clone()).collect()).unwrap_or_default();
            let other = others.first()?.clone();
            let wrong = ["moved the boundary stones of our field", "poisoned our well, I'm sure of it", "spread lies about my daughter", "stole three of our sheep and ate them at midwinter"][(ts.seed as usize ^ giver.len()) % 4];
            Some(mk(TaleKind::Feud, 0, home, other.clone(), 0, 40, format!("The house of {}", other),
                format!("{} and their house {}. Go and tell them, so they hear it from someone who carries a sword. Or end it any way you like, as long as it ends.", other, wrong),
                30 + 5 * lvl, 40 + 10 * lvl as u64))
        }
        _ => None,
        }
    };
    let kinds: &[TaleKind] = match role {
        Role::Guard => &[TaleKind::Snatched],
        Role::Trader => &[TaleKind::Caravan],
        Role::Lord => &[TaleKind::Cult, TaleKind::Tribute],
        Role::Priest => &[TaleKind::Plague],
        Role::Townsfolk => &[TaleKind::Tribute, TaleKind::Feud],
        _ => &[],
    };
    kinds.iter().filter(|k| !had(**k)).find_map(|k| try_kind(*k))
}

/// Accepted: what the hero is given (the tribute to carry), what is set in the world (the herb
/// in its place).
pub fn on_accept(g: &mut Game, q: &Quest) {
    let Goal::Tale(t) = &q.goal else { return };
    match t.kind {
        TaleKind::Tribute => { let mut it = Item::new("tribute", 1); it.tag = t.tag; it.name = Some(format!("the tribute of {}", g.site(q.town).map(|s| s.name.clone()).unwrap_or_default())); super::item::stow(&mut g.hero.pack, it); }
        TaleKind::Plague => {
            // The herb grows on the place's last floor, far from where one comes in.
            if !g.places.contains_key(&t.site) { if let Some(spec) = g.site(t.site).cloned() { g.places.insert(t.site, super::site::realize(&spec)); } }
            if let Some(p) = g.places.get_mut(&t.site) {
                let z = p.floors.len() - 1;
                let f = &mut p.floors[z];
                let from = f.find(|x| matches!(x, super::map::Feature::StairsUp | super::map::Feature::LadderUp | super::map::Feature::RopeSpot | super::map::Feature::Exit)).unwrap_or((f.w as i32 / 2, f.h as i32 / 2));
                let d = f.distances(from.0, from.1, 10_000, |x, y| f.at(x, y).walkable());
                let far = (0..f.h as i32).flat_map(|y| (0..f.w as i32).map(move |x| (x, y))).filter(|&(x, y)| f.at(x, y).walkable() && f.at(x, y).feature == super::map::Feature::None && d[(y as usize) * f.w + x as usize] < i32::MAX).max_by_key(|&(x, y)| (d[(y as usize) * f.w + x as usize], x, y));
                if let Some((x, y)) = far { let mut it = Item::new("moonpetal", 1); it.tag = t.tag; f.drop_item(x, y, it); }
            }
            let tile = t.tile; g.rumour(tile);
            if !g.known.contains(&t.site) { g.known.push(t.site); }
        }
        TaleKind::Snatched | TaleKind::Caravan => { let tile = t.tile; g.rumour(tile); if t.site != 0 && !g.known.contains(&t.site) { g.known.push(t.site); } }
        _ => {}
    }
}

/// Offer a choice for quest `id`.
fn ask(g: &mut Game, id: u32, title: &str, text: String, options: Vec<(String, u8)>) {
    g.choice = Some(Choice { quest: id, title: title.into(), text, options });
}

/// The hero walked onto tile `t` (on the land): the wreck of the caravan, the beast's mouth.
pub fn on_tile(g: &mut Game) {
    let t = g.tile;
    let open: Vec<(u32, Tale)> = g.quests.iter().filter(|q| q.state == State::Open).filter_map(|q| match &q.goal { Goal::Tale(x) if x.tile == t => Some((q.id, x.clone())), _ => None }).collect();
    for (id, x) in open {
        match (x.kind, x.stage) {
            (TaleKind::Caravan, 0) if g.on_land() => { wreck(g, id, &x); }
            (TaleKind::Tribute, 0) if g.on_land() && g.choice.is_none() => {
                let mut opts = vec![(format!("Leave the tribute at the mouth of the lair"), 1u8), (format!("Go in and end {} instead", x.other), 2)];
                opts.push(("Keep the gold for yourself".into(), 3));
                ask(g, id, "The beast's tribute", format!("The mouth of the lair is near: bones, a trampled path, the stink of {}. The tribute chest is heavy in your pack.", x.other), opts);
            }
            _ => {}
        }
    }
}

/// The caravan's wreck, set in the land where the hero comes on it.
fn wreck(g: &mut Game, id: u32, x: &Tale) {
    let (cx, cy) = (g.x, g.y);
    let Some(p) = g.land.as_mut() else { return };
    let f = &mut p.floors[0];
    // A few steps on, where the road was.
    let mut spots: Vec<(i32, i32)> = Vec::new();
    for r in 6..14i32 { for dy in -r..=r { for dx in -r..=r { if dx.abs().max(dy.abs()) == r { let (a, b) = (cx + dx, cy + dy); if f.walkable(a, b) && f.at(a, b).feature == super::map::Feature::None { spots.push((a, b)); } } } } if spots.len() > 20 { break; } }
    let Some(&(wx, wy)) = spots.get((x.tag as usize) % spots.len().max(1)) else { return };
    for (k, (dx, dy)) in [(0, 0), (1, 0), (-1, 1), (2, 1), (0, -1)].iter().enumerate() {
        let (a, b) = (wx + dx, wy + dy);
        if f.walkable(a, b) { f.at_mut(a, b).feature = [super::map::Feature::Crate, super::map::Feature::Bones, super::map::Feature::Barrel, super::map::Feature::Bones, super::map::Feature::Crate][k].clone(); }
    }
    let mut goods = Item::new("trade_goods", 1); goods.tag = x.tag; goods.name = Some(format!("the goods of the caravan to {}", x.other));
    f.drop_item(wx + 1, wy + 1, goods);
    // What did it.
    let n = 2 + (x.tag % 3) as i32;
    for k in 0..n {
        let (a, b) = (wx + 3 + k % 2 * 2, wy - 2 + k);
        if !p.floors[0].walkable(a, b) { continue; }
        let uid = 2_000_000 + x.tag * 8 + k as u32;
        let mut m = super::actor::Monster::new(uid, if g.hero.level >= 10 { "highwayman" } else { "bandit" }, a, b, 0);
        m.awake = true;
        p.monsters.push(m);
    }
    set_tale(g, id, |t, _| t.stage = 1);
    g.say(Tone::Quest, format!("Off the road: broken wagons, dead oxen, the drivers' bones. The caravan to {}. And the ones who did it are still here.", x.other));
    g.look();
}

/// The hero goes into a place: the raiders holding the child call out their price.
pub fn on_enter(g: &mut Game, site: u32) {
    let open: Vec<(u32, Tale)> = g.quests.iter().filter(|q| q.state == State::Open).filter_map(|q| match &q.goal { Goal::Tale(x) if x.site == site && x.kind == TaleKind::Snatched && x.stage == 0 => Some((q.id, x.clone())), _ => None }).collect();
    for (id, x) in open {
        let mut opts = vec![("Go in after the child with the sword".to_string(), 1u8)];
        if g.hero.gold() >= x.price { opts.push((format!("Pay their price ({} gold)", x.price), 2)); }
        ask(g, id, "The raiders' price", format!("A voice from the dark: \"We have the little one, {}. {} gold and they walk out to you. Come in with steel and see what you find.\"", g.hero.name, x.price), opts);
        set_tale(g, id, |t, _| t.stage = 1);
    }
}

/// Something slain: the raiders' chief (the child freed), the cult's keeper (the ledger), the beast.
pub fn on_kill(g: &mut Game, name: &str) {
    let open: Vec<(u32, Tale)> = g.quests.iter().filter(|q| q.state == State::Open).filter_map(|q| match &q.goal { Goal::Tale(x) => Some((q.id, x.clone())), _ => None }).collect();
    for (id, x) in open {
        let boss = g.site(x.site).and_then(|s| s.boss.as_ref()).map(|b| b.name.clone());
        if boss.as_deref() != Some(name) { continue; }
        match x.kind {
            TaleKind::Snatched => {
                set_tale(g, id, |t, q| { t.chose = 1; q.state = State::Done; });
                g.say(Tone::Quest, format!("Behind the chief's bed, tied and filthy and alive: {}. \"Are you taking me home?\" (Go back to the town.)", x.other));
            }
            TaleKind::Cult => {
                let mut ledger = Item::new("ledger", 1); ledger.tag = x.tag; ledger.name = Some("the cult's ledger".into());
                super::item::stow(&mut g.hero.pack, ledger);
                set_tale(g, id, |t, _| t.stage = 1);
                ask(g, id, "The cult's ledger", format!("Among the keeper's things, a ledger: names, dates, offerings. On the last page, in a hand you have seen on a shop's sign: {}. They opened the door to them.", x.other),
                    vec![(format!("Take it to the lord: let {} answer for it", x.other), 1), (format!("Sell {} their silence (they will pay 150 gold)", x.other), 2)]);
            }
            TaleKind::Tribute => {
                g.hero.pack.retain(|i| i.tag != x.tag);
                set_tale(g, id, |t, q| { t.chose = 2; q.state = State::Done; q.gold = q.gold * 3; q.xp *= 2; });
                g.say(Tone::Quest, format!("{} is dead. No more tribute. Go back and tell them.", x.other));
            }
            _ => {}
        }
    }
}

/// Something found: the caravan's goods, the herb.
pub fn on_found(g: &mut Game, it: &Item) {
    if it.tag == 0 { return; }
    let open: Vec<(u32, Tale)> = g.quests.iter().filter(|q| q.state == State::Open).filter_map(|q| match &q.goal { Goal::Tale(x) if x.tag == it.tag => Some((q.id, x.clone())), _ => None }).collect();
    for (id, x) in open {
        match x.kind {
            TaleKind::Caravan => ask(g, id, "The caravan's goods", format!("Bolts of cloth, salt, a strongbox: what the caravan carried to {}. Worth {} gold to the trader who sent it, and more to anyone who does not ask.", x.other, x.price),
                vec![("Bring them back to the trader".into(), 1), (format!("Keep them (sell them anywhere, for {} gold)", x.price * 3 / 2), 2)]),
            TaleKind::Plague => {
                let rival = g.site(x.other_town).map(|s| s.name.clone()).unwrap_or_else(|| "a rival town".into());
                ask(g, id, "The moonpetal", format!("Pale flowers in the damp, glowing faintly: moonpetal. A trader from {} would pay {} gold for it, they say, and ask no questions about who dies without it.", rival, x.price),
                    vec![("Bring it to the priest".into(), 1), (format!("Sell it to {} for {} gold", rival, x.price), 2)]);
            }
            _ => {}
        }
    }
}

/// The hero greets someone: the other house of a feud hears what the hero came to say.
pub fn on_greet(g: &mut Game, name: &str, home: u32) {
    let open: Vec<(u32, Tale, u32)> = g.quests.iter().filter(|q| q.state == State::Open && q.town == home).filter_map(|q| match &q.goal { Goal::Tale(x) if x.kind == TaleKind::Feud && x.other == name && x.stage == 0 => Some((q.id, x.clone(), q.town)), _ => None }).collect();
    for (id, x, _) in open {
        let giver = g.quests.iter().find(|q| q.id == id).map(|q| q.giver.clone()).unwrap_or_default();
        let mut opts = Vec::new();
        if g.hero.gold() >= x.price { opts.push((format!("Make peace: pay for a feast for both houses ({} gold)", x.price), 3u8)); }
        opts.push((format!("Side with {}: tell {} to leave the town", giver, x.other), 1));
        opts.push((format!("Side with {}: {} is the liar", x.other, giver), 2));
        ask(g, id, "The feud", format!("{} hears you out, red in the face. \"{} says that? Then hear my side...\" It is an old quarrel, and both have a share of the blame.", x.other, giver), opts);
    }
}

/// A choice made: the tale goes on its way, and the town remembers.
pub fn decide(g: &mut Game, k: usize) -> bool {
    let Some(c) = g.choice.take() else { return false };
    let Some(&(_, code)) = c.options.get(k) else { g.choice = Some(c); return false };
    let Some(x) = tale(g, c.quest) else { return true };
    let id = c.quest;
    let town = g.quests.iter().find(|q| q.id == id).map(|q| q.town).unwrap_or(0);
    let tname = g.site(town).map(|s| s.name.clone()).unwrap_or_default();
    let giver = g.quests.iter().find(|q| q.id == id).map(|q| q.giver.clone()).unwrap_or_default();
    match (x.kind, code) {
        (TaleKind::Snatched, 1) => g.say(Tone::Info, "You draw steel and go in after the child."),
        (TaleKind::Snatched, 2) => {
            g.hero.take_gold(x.price);
            set_tale(g, id, |t, q| { t.chose = 2; q.state = State::Done; q.xp /= 3; });
            g.say(Tone::Quest, format!("You throw the purse into the dark. A while later {} stumbles out, crying. The raiders keep their camp. (Go back to {}.)", x.other, tname));
        }
        (TaleKind::Caravan, 1) => { set_tale(g, id, |t, q| { t.chose = 1; t.stage = 2; q.state = State::Done; }); g.say(Tone::Quest, format!("You load the goods up. (Take them back to {} in {}.)", giver, tname)); }
        (TaleKind::Caravan, 2) => {
            if let Some(i) = g.hero.pack.iter().position(|i| i.tag == x.tag) { g.hero.pack.remove(i); }
            super::item::stow(&mut g.hero.pack, Item::new("gold", x.price * 3 / 2));
            set_tale(g, id, |t, q| { t.chose = 2; q.state = State::Failed; });
            wrong(g, town, &giver, 2);
            note(g, town, format!("The caravan to {} was robbed on the road, and its goods sold off by some adventurer.", x.other));
            g.say(Tone::Danger, format!("The goods are yours, and gold for them. In {} they will hear who sold them.", tname));
        }
        (TaleKind::Cult, 1) => { set_tale(g, id, |t, q| { t.chose = 1; q.state = State::Done; }); g.say(Tone::Quest, format!("Take the ledger to the lord of {}.", tname)); }
        (TaleKind::Cult, 2) => {
            g.hero.pack.retain(|i| i.tag != x.tag);
            super::item::stow(&mut g.hero.pack, Item::new("gold", 150));
            set_tale(g, id, |t, q| { t.chose = 2; q.state = State::Done; q.gold /= 3; });
            note(g, town, "The chanting under the streets has stopped. No one knows who opened the door, and no one asks.".into());
            g.say(Tone::Quest, format!("{} pays, white-faced, and burns the ledger. The lord will hear only that the cellar is empty.", x.other));
        }
        (TaleKind::Feud, c @ 1..=3) => {
            let (winner, loser) = match c { 1 => (giver.clone(), Some(x.other.clone())), 2 => (x.other.clone(), Some(giver.clone())), _ => (giver.clone(), None) };
            if c == 3 { g.hero.take_gold(x.price); }
            set_tale(g, id, |t, q| { t.chose = c; t.stage = 1; q.state = State::Done; q.giver = winner.clone(); if c == 3 { q.xp *= 2; } });
            match loser {
                Some(l) => {
                    g.remove_npc(town, &l, &format!("{} packs a cart and leaves {} for good, cursing your name.", l, tname));
                    wrong(g, town, &l, 1);
                    note(g, town, format!("The house of {} left {} after the quarrel with {}.", l, tname, winner));
                }
                None => { note(g, town, format!("The houses of {} and {} feasted together and made peace.", giver, x.other)); g.say(Tone::Quest, format!("A feast, a little too much ale, and the two houses embrace. (Tell {}.)", giver)); }
            }
        }
        (TaleKind::Tribute, 1) => {
            g.hero.pack.retain(|i| i.tag != x.tag);
            set_tale(g, id, |t, q| { t.chose = 1; q.state = State::Done; });
            note(g, town, format!("The tribute to {} was paid this season, and the herds were spared.", x.other));
            g.say(Tone::Quest, format!("You leave the chest on the bones at the mouth and walk away without looking back. (Go back to {}.)", tname));
        }
        (TaleKind::Tribute, 2) => { set_tale(g, id, |t, _| t.stage = 1); g.say(Tone::Info, format!("You keep the chest and go in after {}.", x.other)); if let Some(s) = g.site(x.site).map(|s| s.id) { if !g.known.contains(&s) { g.known.push(s); } } }
        (TaleKind::Tribute, 3) => {
            if let Some(i) = g.hero.pack.iter().position(|i| i.tag == x.tag) { g.hero.pack.remove(i); }
            super::item::stow(&mut g.hero.pack, Item::new("gold", x.price));
            set_tale(g, id, |t, q| { t.chose = 3; q.state = State::Failed; });
            wrong(g, town, &giver, 3);
            note(g, town, format!("The tribute to {} never reached it, and it came for the herds of {}.", x.other, tname));
            g.say(Tone::Danger, format!("The gold is yours. {} will pay for it in sheep and worse.", tname));
        }
        (TaleKind::Plague, 1) => { set_tale(g, id, |t, q| { t.chose = 1; q.state = State::Done; }); g.say(Tone::Quest, format!("Take the moonpetal to the priest of {}.", tname)); }
        (TaleKind::Plague, 2) => {
            g.hero.pack.retain(|i| i.tag != x.tag);
            super::item::stow(&mut g.hero.pack, Item::new("gold", x.price));
            set_tale(g, id, |t, q| { t.chose = 2; q.state = State::Failed; });
            wrong(g, town, &giver, 2);
            note(g, town, format!("The wasting took many in {}; the cure, they say, was sold to {}.", tname, g.site(x.other_town).map(|s| s.name.clone()).unwrap_or_default()));
            g.say(Tone::Danger, "You sell the moonpetal. Somewhere a bell begins to toll.");
        }
        _ => {}
    }
    true
}

/// Reported: what the giver does with what the hero brings (and what the town makes of it).
pub fn on_report(g: &mut Game, q: &Quest) -> Vec<String> {
    let Goal::Tale(x) = &q.goal else { return Vec::new() };
    let tname = g.site(q.town).map(|s| s.name.clone()).unwrap_or_default();
    let mut out = Vec::new();
    g.hero.pack.retain(|i| i.tag != x.tag || x.tag == 0);
    match (x.kind, x.chose) {
        (TaleKind::Snatched, _) => { out.push(format!("{} runs into their mother's arms. Half the town comes out to see.", x.other)); note(g, q.town, format!("{}, the miller's child, was brought home from the raiders by {}.", x.other, g.hero.name)); g.add_townsperson(q.town, &x.other); }
        (TaleKind::Caravan, _) => { out.push(format!("{} counts the goods and shakes your hand. \"The drivers' families will have this.\"", q.giver)); note(g, q.town, format!("{} brought back the goods of the caravan to {}.", g.hero.name, x.other)); }
        (TaleKind::Cult, 1) => {
            out.push(format!("The lord reads the ledger, and sends the watch for {}.", x.other));
            g.remove_npc(q.town, &x.other, &format!("The watch drags {} away in chains.", x.other));
            note(g, q.town, format!("{} was taken in chains for opening the town to the cult; {} found them out.", x.other, g.hero.name));
        }
        (TaleKind::Feud, 3) => out.push("\"Peace, at last. I never thought I'd see it.\"".into()),
        (TaleKind::Feud, _) => out.push("\"Good riddance to them.\"".into()),
        (TaleKind::Tribute, 2) => { out.push(format!("The whole of {} turns out to cheer you.", tname)); note(g, q.town, format!("{} slew {}, and {} pays tribute no more.", g.hero.name, x.other, tname)); }
        (TaleKind::Plague, _) => { out.push("The priest grinds the petals, and by the week's end the sick are sitting up.".into()); note(g, q.town, format!("The wasting left {} when {} brought moonpetal from {}.", tname, g.hero.name, g.site(x.site).map(|s| s.name.clone()).unwrap_or_default())); }
        _ => {}
    }
    // Everyone in the town hears of it.
    let deed = q.title.clone();
    for n in g.town_npcs_mut(q.town) { if !n.met.helped.contains(&deed) { n.met.helped.push(deed.clone()); } }
    out
}

/// The town remembers: a line in its news, kept.
fn note(g: &mut Game, town: u32, line: String) { if let Some(s) = g.sites.iter_mut().find(|s| s.id == town) { s.notes.push(line); if s.notes.len() > 8 { s.notes.remove(0); } } }

/// The town holds it against the hero.
fn wrong(g: &mut Game, town: u32, who: &str, n: u32) {
    for p in g.town_npcs_mut(town) { if p.name == who { p.met.wronged += n; } else if n >= 2 { p.met.wronged += 1; } }
}
