//! What a hero builds up (card adv-crafting-progression): things worked from what was taken
//! (the smith's recipes and the sage's enchantments, `Data::recipes`), a horse, passage by sea
//! between ports, a house in a town (a stash, a bed, a trophy wall), a warband (sellswords and
//! freed captives following beside the companion: `Game::band`) and titles the lord grants.
//!
//! What a boss carried keeps its story ("taken from Ghaz the Red", `Item::story`), and a thing
//! worked from it takes the name: three dragon scales from Ghaz the Red make "Ghaz the Red's
//! dragon shield", its story the smith's, the town's and the slayer's.

use super::actor::Role;
use super::data::data;
use super::game::{Companion, Game, Tone};
use super::item::{stow, Item};
use super::npc::Topic;
use super::site::SiteKind;

/// The followers beyond the companion (two at most: three blades at one's side).
pub const BAND: usize = 2;

/// A recipe one can make here (by the speaker's role), and whether one has what it needs.
fn recipes_for(role: Role) -> Vec<(usize, &'static super::data::RecipeDef)> {
    let at = if role == Role::Sage { "sage" } else { "smith" };
    data().recipes.iter().enumerate().filter(|(_, r)| r.at == at).collect()
}

fn has_all(g: &Game, r: &super::data::RecipeDef) -> bool {
    r.needs.iter().all(|(id, n)| g.hero.count(id) >= *n) && g.hero.gold() >= r.gold && (r.enchant.is_none() || target_of(g, r).is_some())
}

/// What an enchantment goes on: the weapon in hand, or the body's armour for warding.
fn target_of(g: &Game, r: &super::data::RecipeDef) -> Option<super::hero::Slot> {
    use super::hero::Slot;
    let slot = if r.enchant.as_deref() == Some("warding") { Slot::Body } else { Slot::Hand };
    g.hero.equipped[slot as usize].as_ref().filter(|i| i.enchant.is_none() && matches!(i.def().kind.as_str(), "weapon" | "armour")).map(|_| slot)
}

fn title_case(s: &str) -> String { s.to_string() }

/// Make recipe `k`: take its needs, give the thing (named after the beast it came from).
pub fn make(g: &mut Game, k: usize, smith: &str, town: &str) -> String {
    let Some(r) = data().recipes.get(k) else { return String::new() };
    if !has_all(g, r) {
        let want: Vec<String> = r.needs.iter().map(|(id, n)| format!("{} {}", n, data().item(id).map_or(id.clone(), |d| if *n > 1 { d.plural.clone().unwrap_or(format!("{}s", d.name)) } else { d.name.clone() }))).collect();
        return format!("For that I need {}{}.", want.join(", "), if r.gold > 0 { format!(" and {} gold", r.gold) } else { String::new() });
    }
    // Whose remains it is worked from.
    let from: Option<String> = r.needs.iter().find_map(|(id, _)| g.hero.pack.iter().find(|i| &i.id == id && i.story.as_deref().map_or(false, |s| s.starts_with("taken from "))).and_then(|i| i.story.as_ref().map(|s| s.trim_start_matches("taken from ").to_string())));
    // Take what is needed (the storied ones first).
    for (id, n) in r.needs.iter() {
        let mut left = *n;
        let mut order: Vec<usize> = (0..g.hero.pack.len()).filter(|&i| &g.hero.pack[i].id == id).collect();
        order.sort_by_key(|&i| g.hero.pack[i].story.is_none());
        for i in order { let t = g.hero.pack[i].count.min(left); g.hero.pack[i].count -= t; left -= t; if left == 0 { break; } }
        g.hero.pack.retain(|i| i.count > 0);
    }
    if r.gold > 0 { g.hero.take_gold(r.gold); }
    if let Some(e) = &r.enchant {
        let slot = target_of(g, r).unwrap();
        let it = g.hero.equipped[slot as usize].as_mut().unwrap();
        it.enchant = Some(e.clone());
        let d = it.describe();
        g.say(Tone::Level, format!("Runes are cut and something answers in the metal: {}.", d));
        return format!("It is done. Your {} will not forget it.", d);
    }
    let mut it = Item::new(&r.makes, r.count.max(1));
    if r.count <= 1 && matches!(it.def().kind.as_str(), "weapon" | "armour" | "shield") { it.quality = 2; }
    if let Some(b) = &from {
        if r.count <= 1 {
            it.quality = 3;
            it.name = Some(format!("{}'s {}", b, title_case(&it.def().name)));
            it.story = Some(format!("Worked by {} the smith of {} from what {} slew of {}.", smith, town, g.hero.name, b));
            let t = g.turn;
            let n = it.name.clone().unwrap();
            g.deeds.push((t, format!("had {} made from {}", n, b)));
        }
    }
    let d = it.describe();
    stow(&mut g.hero.pack, it);
    g.say(Tone::Loot, format!("You get {}.", d));
    format!("There: {}. Good work, if I say so myself.", d)
}

impl Game {
    /// The towns with a port, other than this one: (id, name, days by sea, fare).
    pub fn passages(&self, from: u32) -> Vec<(u32, String, u32, u32)> {
        let Some(here) = self.site(from).filter(|s| s.town.as_ref().map_or(false, |t| t.port)) else { return Vec::new() };
        let w = self.world.w;
        let mut v: Vec<(u32, String, u32, u32)> = self.sites.iter().filter(|s| s.kind == SiteKind::Town && s.id != from && s.town.as_ref().map_or(false, |t| t.port))
            .map(|s| { let d = super::world::dist(s.tile, here.tile, w) as u32; let days = (d + 3) / 4; (s.id, s.name.clone(), days.max(1), 20 + 15 * d) }).collect();
        v.sort_by_key(|x| (x.2, x.0));
        v.truncate(6);
        v
    }

    /// Sail to port `to`: days pass at sea, and one comes ashore at its gate.
    pub fn sail(&mut self, to: u32, fare: u32) -> bool {
        let Some(dest) = self.site(to).cloned() else { return false };
        if !self.hero.take_gold(fare) { return false; }
        let days = self.passages(self.site_here()).iter().find(|p| p.0 == to).map_or(1, |p| p.2);
        self.talk = None;
        if self.on_land() { self.store_land(); }
        self.turn += days as u64 * super::land::DAY / 2;
        self.hero.fed = self.hero.fed.max(1500);
        self.say(Tone::Info, format!("You take ship. {} day{} of grey water and gulls, and {} rises out of the sea.", days, if days == 1 { "" } else { "s" }, dest.name));
        self.land_at(dest.tile, None);
        self.stats.voyages += 1;
        true
    }

    /// Riding: a horse in the pack (stabled when one goes under the ground).
    pub fn mounted(&self) -> bool { self.hero.count("horse") > 0 }

    /// All who follow the hero (the companion first).
    pub fn followers(&self) -> usize { self.companion.iter().count() + self.band.len() }

    /// Take someone into the band (or as the companion if there is none); false if it is full.
    pub fn join(&mut self, name: String, race: String) -> bool {
        let lvl = self.hero.level as i32;
        let c = Companion { name: name.clone(), race, hp: 50 + 12 * lvl, max_hp: 50 + 12 * lvl, x: self.x, y: self.y, energy: 0, left: false, kills: 0, struck_at: 0, morale: 100, paid_day: self.turn / super::land::DAY };
        if self.companion.is_none() { self.companion = Some(c); }
        else if self.band.len() < BAND { self.band.push(c); }
        else { return false; }
        self.companion_follow(true);
        true
    }

    /// The band takes its turns: each one as the companion would (`companion_act`).
    pub fn band_act(&mut self, id: u32, cost: i32, dist: &[i32]) {
        if self.band.is_empty() { return; }
        let main = self.companion.take();
        let band = std::mem::take(&mut self.band);
        let mut kept = Vec::new();
        for c in band { self.companion = Some(c); self.companion_act_one(id, cost, dist); if let Some(c) = self.companion.take() { kept.push(c); } }
        self.band = kept;
        self.companion = main;
        // A fallen companion's place is taken by the next of the band.
        if self.companion.is_none() && !self.band.is_empty() { self.companion = Some(self.band.remove(0)); }
    }

    /// A town's titles one may be granted: regard 50+ and three works done for it.
    pub fn title_due(&self, town: u32) -> Option<String> {
        let name = self.site(town)?.name.clone();
        if self.titles.iter().any(|t| t.ends_with(&name)) { return None; }
        let works = self.quests.iter().filter(|q| q.town == town && q.state == super::quest::State::Rewarded).count();
        if self.regard_of(town) < 50 || works < 3 { return None; }
        Some(if self.hero.level >= 15 { format!("Knight of {}", name) } else { format!("Warden of {}", name) })
    }
}

/// The new talk: crafting, sailing, horses, the house, titles.
pub fn answer(g: &mut Game, n: &super::actor::Npc, town: u32, topic: Topic) -> (String, Option<Vec<(String, Topic)>>) {
    let tname = g.site(town).map(|s| s.name.clone()).unwrap_or_default();
    match topic {
        Topic::Craft => {
            let mut v: Vec<(String, Topic)> = recipes_for(n.role).into_iter().filter(|(_, r)| r.needs.iter().any(|(id, _)| g.hero.count(id) > 0)).map(|(k, r)| {
                let needs: Vec<String> = r.needs.iter().map(|(id, c)| format!("{} {}", c, data().item(id).map_or(id.as_str(), |d| d.name.as_str()))).collect();
                let what = match &r.enchant { Some(e) => format!("Enchant {} with {}", if e == "warding" { "your armour" } else { "your weapon" }, e), None => format!("{}{}", if r.count > 1 { format!("{} ", r.count) } else { String::new() }, data().item(&r.makes).map_or(r.makes.as_str(), |d| d.name.as_str())) };
                (format!("{} ({}{}){}", what, needs.join(", "), if r.gold > 0 { format!(", {} gold", r.gold) } else { String::new() }, if has_all(g, r) { "" } else { " (not enough)" }), Topic::Make(k))
            }).collect();
            let said = if v.is_empty() { if n.role == Role::Sage { "Bring me shards of the elements, venom, the dust of the dead, and a blade to bind them to." } else { "Bring me hides, bones, scales, horn: I can work them into something worth carrying." }.to_string() } else { "Here is what I can make of what you carry.".to_string() };
            v.push(("Back".into(), Topic::Back));
            (said, Some(v))
        }
        Topic::Make(k) => (make(g, k, &n.name, &tname), None),
        Topic::Passage => {
            let mut v: Vec<(String, Topic)> = g.passages(town).into_iter().map(|(id, name, days, fare)| (format!("To {} ({} day{}, {} gold)", name, days, if days == 1 { "" } else { "s" }, fare), Topic::Sail(id, fare))).collect();
            let said = if v.is_empty() { "No ship sails from here.".to_string() } else { "Ships leave on the tide. Where to?".to_string() };
            v.push(("Back".into(), Topic::Back));
            (said, Some(v))
        }
        Topic::Sail(to, fare) => {
            if g.sail(to, fare) { (String::new(), None) } else { (format!("The fare is {} gold.", fare), None) }
        }
        Topic::BuyHorse(price) => {
            if g.mounted() { ("You have a horse already.".into(), None) }
            else if g.hero.take_gold(price) { stow(&mut g.hero.pack, Item::new("horse", 1)); g.say(Tone::Level, "You have a horse now: the roads go by twice as fast."); ("A good beast. Feed her and she will carry you to the end of the world.".into(), None) }
            else { (format!("A horse is {} gold.", price), None) }
        }
        Topic::BuyHouse(price) => {
            if g.houses.contains(&town) { ("You own a house here already.".into(), None) }
            else if g.hero.take_gold(price) {
                g.houses.push(town);
                let t = g.turn;
                g.deeds.push((t, format!("bought a house in {}", tname)));
                g.say(Tone::Level, format!("You own a house in {} now: a stash, a bed, a wall for trophies (ask the innkeeper).", tname));
                ("The deed is yours. Welcome to the town, neighbour.".into(), None)
            } else { (format!("A house within the walls is {} gold.", price), None) }
        }
        Topic::House => {
            let stash = g.stash.get(&town).map_or(0, |v| v.len());
            let v = vec![("Store your loot".into(), Topic::StoreLoot), (format!("Take back what is stored ({} things)", stash), Topic::TakeStash), ("Sleep in your own bed".into(), Topic::Sleep), ("Your trophy wall".into(), Topic::Trophies), ("Back".into(), Topic::Back)];
            ("Your house is as you left it.".into(), Some(v))
        }
        Topic::StoreLoot => {
            let (put, keep): (Vec<Item>, Vec<Item>) = std::mem::take(&mut g.hero.pack).into_iter().partition(|i| matches!(i.def().kind.as_str(), "loot") || (i.name.is_some() && i.def().kind != "key"));
            g.hero.pack = keep;
            let n2 = put.len();
            let s = g.stash.entry(town).or_default();
            for it in put { stow(s, it); }
            (format!("You put {} things away in your house.", n2), None)
        }
        Topic::TakeStash => {
            let items = g.stash.remove(&town).unwrap_or_default();
            let n2 = items.len();
            for it in items { stow(&mut g.hero.pack, it); }
            (format!("You take {} things from your house.", n2), None)
        }
        Topic::Sleep => {
            g.hero.hp = g.hero.max_hp(); g.hero.mana = g.hero.max_mana(); g.hero.poisoned = 0; g.hero.wounds.clear();
            g.turn += super::land::DAY / 3;
            ("You sleep in your own bed, and wake whole.".into(), None)
        }
        Topic::Trophies => {
            let mut t: Vec<String> = g.slain.clone();
            t.sort(); t.dedup();
            let named: Vec<String> = g.hero.pack.iter().chain(g.hero.equipped.iter().flatten()).filter_map(|i| i.name.clone()).collect();
            (if t.is_empty() { "The wall is bare yet.".into() } else { format!("On your wall: the heads, horns and banners of {}.{}", t.join(", "), if named.is_empty() { String::new() } else { format!(" And with you: {}.", named.join(", ")) }) }, None)
        }
        Topic::Title => {
            match g.title_due(town) {
                Some(title) => {
                    g.titles.push(title.clone());
                    let t = g.turn;
                    g.deeds.push((t, format!("was named {}", title)));
                    let who = g.hero.name.clone();
                    g.chronicle(super::living::DeedKind::QuestDone, format!("{} named {}", who, title), format!("{} was named {} by its lord.", who, title));
                    g.say(Tone::Level, format!("You are {} now.", title));
                    (format!("Kneel. Rise, {}, {}.", who, title), None)
                }
                None => ("Serve this town longer, and well, and we will speak of it.".into(), None),
            }
        }
        _ => (String::new(), None),
    }
}

/// The menu lines for the new talk (by role and town).
pub fn menu(g: &Game, role: Role, town: u32) -> Vec<(String, Topic)> {
    let mut v = Vec::new();
    let size = g.site(town).and_then(|s| s.town.as_ref()).map_or(0, |t| t.size);
    match role {
        Role::Smith => v.push(("Work something from what you carry".to_string(), Topic::Craft)),
        Role::Sage => v.push(("Enchant something".to_string(), Topic::Craft)),
        Role::Innkeeper => {
            if g.site(town).and_then(|s| s.town.as_ref()).map_or(false, |t| t.port) { v.push(("Passage by sea".to_string(), Topic::Passage)); }
            if size >= 2 && !g.mounted() { v.push(("A horse from the stable (400 gold)".to_string(), Topic::BuyHorse(400))); }
            if g.houses.contains(&town) { v.push(("Your house".to_string(), Topic::House)); }
            if g.companion.is_some() && g.band.len() < BAND { let p = 120 * g.hero.level.max(1); v.push((format!("Another sellsword for your band ({} gold)", p), Topic::Hire(p))); }
        }
        Role::Lord => {
            if !g.houses.contains(&town) && town != 0 { let p = 1000 + 600 * size as u32; v.push((format!("Buy a house within the walls ({} gold)", p), Topic::BuyHouse(p))); }
            if g.title_due(town).is_some() { v.push(("Ask for a title".to_string(), Topic::Title)); }
        }
        _ => {}
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adventure::game::Action;

    /// A dragon's scales, taken from it, become a shield in its name.
    #[test]
    fn a_dragons_scales_become_its_shield() {
        let mut g = crate::adventure::land::tests::game();
        let mut scales = Item::new("dragon_scale", 3);
        scales.story = Some("taken from Ghaz the Red".into());
        stow(&mut g.hero.pack, scales);
        stow(&mut g.hero.pack, Item::new("gold", 500));
        let k = data().recipes.iter().position(|r| r.makes == "dragon_shield").unwrap();
        let said = make(&mut g, k, "Brann", "Greenburg");
        let s = g.hero.pack.iter().find(|i| i.id == "dragon_shield").unwrap_or_else(|| panic!("no shield: {}", said));
        assert_eq!(s.name.as_deref(), Some("Ghaz the Red's dragon shield"));
        assert!(s.story.as_deref().unwrap().contains("Brann the smith of Greenburg") && s.quality == 3);
        assert_eq!(g.hero.count("dragon_scale"), 0);
        // An enchantment at the sage goes on the blade in hand.
        stow(&mut g.hero.pack, Item::new("fire_shard", 2));
        stow(&mut g.hero.pack, Item::new("gold", 300));
        g.hero.equipped[crate::adventure::hero::Slot::Hand as usize] = Some(Item::new("sword", 1));
        let k = data().recipes.iter().position(|r| r.enchant.as_deref() == Some("flame")).unwrap();
        make(&mut g, k, "Wyn", "Greenburg");
        assert_eq!(g.hero.weapon().unwrap().enchant.as_deref(), Some("flame"));
        assert!(g.hero.weapon().unwrap().describe().ends_with("of flame"));
    }

    /// A ship from one port crosses the sea to another: days pass, one comes ashore there.
    #[test]
    fn a_boat_crosses_the_sea() {
        let mut g = crate::adventure::land::tests::game();
        let mut other = g.sites.iter().find(|s| s.id == 1).unwrap().clone();
        other.id = 3; other.name = "Saltmere".into(); other.tile = (7, 4); other.seed = 99;
        g.sites.push(other);
        for s in g.sites.iter_mut().filter(|s| s.kind == SiteKind::Town) { if let Some(t) = s.town.as_mut() { t.port = true; } }
        g.set_atlas();
        g.land_at((3, 2), None);
        let p = g.passages(1);
        let (to, _, days, fare) = p.iter().find(|x| x.0 == 3).cloned().expect("a passage to Saltmere");
        stow(&mut g.hero.pack, Item::new("gold", fare));
        let t0 = g.turn;
        assert!(g.sail(to, fare));
        assert_eq!(g.tile, (7, 4));
        assert!(g.on_land() && g.turn >= t0 + days as u64 * crate::adventure::land::DAY / 2);
        assert_eq!(g.stats.voyages, 1);
    }

    /// Three blades follow at once: a sellsword hired, two more for the band; they keep up.
    #[test]
    fn three_companions_follow_at_once() {
        let mut g = crate::adventure::land::tests::game();
        g.land_at((3, 2), None);
        stow(&mut g.hero.pack, Item::new("gold", 5000));
        let inn = g.place().unwrap().npcs.iter().position(|n| n.role == Role::Innkeeper && n.home == 1).expect("an innkeeper");
        for _ in 0..3 {
            crate::adventure::npc::greet(&mut g, inn);
            let i = g.talk.as_ref().unwrap().options.iter().position(|(_, t)| matches!(t, Topic::Hire(_))).expect("a sellsword for hire");
            crate::adventure::npc::answer(&mut g, i);
            g.talk = None;
        }
        assert_eq!(g.followers(), 3);
        crate::adventure::npc::greet(&mut g, inn);
        assert!(!g.talk.as_ref().unwrap().options.iter().any(|(_, t)| matches!(t, Topic::Hire(_))), "a fourth for hire");
        g.talk = None;
        // Walk away: they keep up.
        for _ in 0..12 { if let Some(p) = g.land.as_mut() { p.monsters.clear(); } g.act(Action::Move(0, 1)); }
        let (hx, hy) = (g.x, g.y);
        for c in g.companion.iter().chain(g.band.iter()) { assert!((c.x - hx).abs().max((c.y - hy).abs()) <= 6, "{} fell behind", c.name); }
    }
}
