//! Talking to the townsfolk (Tibia's keywords as choices): their name, their job, trade, work,
//! rumours (what their town has heard of the history, and of places to go), healing and a calling
//! at the temple, spells to learn, a bed at the inn.

use super::actor::Role;
use super::data::data;
use super::game::{Game, Tone};
use super::item::{stow, Item};
use super::quest::{self, Quest, State};

#[derive(Clone, Debug, PartialEq)]
pub enum Topic {
    Name, Job, Trade, Buy(String, u32, u32), SellLoot, SellGear, SellOne(usize), Quest, Accept, Report(usize), Rumours, Places,
    Heal, Calling, Become(String), Spells, Learn(String, u32), Rest(u32), Bye, Back,
}

#[derive(Clone, Debug)]
pub struct Talk {
    pub npc: usize,
    pub name: String,
    pub role: Role,
    /// What they said last (shown in the card).
    pub said: String,
    pub options: Vec<(String, Topic)>,
    /// A quest offered and not yet taken.
    pub offer: Option<Quest>,
}

fn town_id(g: &Game) -> u32 { g.here.unwrap_or(0) }

fn main_menu(g: &Game, role: Role) -> Vec<(String, Topic)> {
    let mut v = vec![("Name".to_string(), Topic::Name), ("Job".into(), Topic::Job)];
    match role {
        Role::Smith | Role::Trader | Role::Innkeeper => v.push(("Trade".into(), Topic::Trade)),
        Role::Priest => { v.push(("Heal".into(), Topic::Heal)); if g.hero.calling.is_none() { v.push(("Calling".into(), Topic::Calling)); } v.push(("Spells".into(), Topic::Spells)); }
        Role::Sage => v.push(("Places".into(), Topic::Places)),
        _ => {}
    }
    if role == Role::Innkeeper { v.push(("A bed (10 gold)".into(), Topic::Rest(10))); }
    if matches!(role, Role::Lord | Role::Guard | Role::Priest | Role::Sage) {
        let mine: Vec<usize> = g.quests.iter().enumerate().filter(|(_, q)| q.town == town_id(g) && q.giver == g.place().map(|p| p.npcs.get(g.talk.as_ref().map_or(usize::MAX, |t| t.npc)).map(|n| n.name.clone()).unwrap_or_default()).unwrap_or_default() && q.state == State::Done).map(|(i, _)| i).collect();
        for i in mine { v.push((format!("Report: {}", g.quests[i].title), Topic::Report(i))); }
        v.push(("Quest".into(), Topic::Quest));
    }
    if matches!(role, Role::Townsfolk | Role::Innkeeper | Role::Guard | Role::Sage | Role::Lord) { v.push(("Rumours".into(), Topic::Rumours)); }
    v.push(("Bye".into(), Topic::Bye));
    v
}

/// Begin talking to person `k` of the place.
pub fn greet(g: &mut Game, k: usize) {
    let Some(n) = g.place().and_then(|p| p.npcs.get(k)).cloned() else { return };
    let hero = g.hero.name.clone();
    let town = g.place().map(|p| p.spec.name.clone()).unwrap_or_default();
    let said = match n.role {
        Role::Priest => format!("Welcome, {}, to the temple of {}. The god's light on you.", hero, n.of),
        Role::Smith => format!("Hello, {}. Iron and steel, sharp and true. Looking for a blade?", hero),
        Role::Trader => format!("Welcome, {}! Potions, food, torches, rope, arrows. And I buy what you drag out of the dark.", hero),
        Role::Innkeeper => format!("Come in, {}, sit by the fire. A meal, a bed, the news of the road.", hero),
        Role::Lord => format!("You stand before the lord of {}. Speak, {}.", town, hero),
        Role::Guard => format!("Halt. Ah, {}. The roads are bad and the bounties are good.", hero),
        Role::Sage => format!("Ah, a visitor. I am {}, and I keep what is known of the old days. What would you learn, {}?", n.name, hero),
        Role::Townsfolk => format!("Good day, {}.", hero),
    };
    g.talk = Some(Talk { npc: k, name: n.name.clone(), role: n.role, said, options: Vec::new(), offer: None });
    let menu = main_menu(g, n.role);
    if let Some(t) = g.talk.as_mut() { t.options = menu; }
    g.say(Tone::Talk, format!("{}: \"{}\"", n.name, g.talk.as_ref().unwrap().said));
}

fn stock(role: Role, g: &Game) -> Vec<(&'static str, u32)> {
    let lvl = g.hero.level;
    match role {
        Role::Smith => {
            let mut v = vec![("club", 1), ("dagger", 1), ("short_sword", 1), ("hatchet", 1), ("spear", 1), ("sword", 1), ("axe", 1), ("mace", 1), ("wooden_shield", 1), ("studded_shield", 1),
                ("leather_helmet", 1), ("leather_armor", 1), ("leather_legs", 1), ("leather_boots", 1), ("chain_helmet", 1), ("brass_armor", 1), ("chain_legs", 1)];
            if lvl >= 10 { v.extend([("viking_helmet", 1), ("scale_armor", 1), ("chain_armor", 1), ("round_shield", 1)]); }
            if lvl >= 20 { v.extend([("broadsword", 1), ("battle_axe", 1), ("battle_hammer", 1), ("plate_legs", 1)]); }
            v
        }
        Role::Trader => {
            let mut v = vec![("health_potion", 1), ("mana_potion", 1), ("bread", 1), ("cheese", 1), ("meat", 1), ("torch", 1), ("rope", 1), ("shovel", 1), ("arrow", 10), ("bolt", 10), ("bow", 1), ("crossbow", 1)];
            if lvl >= 15 { v.extend([("strong_health_potion", 1), ("strong_mana_potion", 1)]); }
            if g.hero.calling.as_deref() == Some("sorcerer") { v.push(("wand_of_embers", 1)); }
            if g.hero.calling.as_deref() == Some("druid") { v.push(("snakebite_rod", 1)); }
            v
        }
        Role::Innkeeper => vec![("bread", 1), ("cheese", 1), ("meat", 1), ("fish", 1)],
        _ => Vec::new(),
    }
}

/// Choose option `i` in the open conversation.
pub fn answer(g: &mut Game, i: usize) {
    let Some(t) = g.talk.clone() else { return };
    let Some((_, topic)) = t.options.get(i).cloned() else { return };
    let n = match g.place().and_then(|p| p.npcs.get(t.npc)).cloned() { Some(n) => n, None => { g.talk = None; return } };
    let mut said = String::new();
    let mut options: Option<Vec<(String, Topic)>> = None;
    let mut offer = t.offer.clone();
    let town = g.place().map(|p| p.spec.name.clone()).unwrap_or_default();
    match topic {
        Topic::Name => said = format!("I am {}, {} of {}.", n.name, n.role.word(), town),
        Topic::Job => said = match n.role {
            Role::Priest => format!("I tend the temple of {}. I heal the hurt, I give callings to those of level 8, and I teach the words of power.", n.of),
            Role::Smith => "I make and mend arms and armour, and buy what is worth melting down.".into(),
            Role::Trader => "I sell what an adventurer needs and buy what an adventurer finds: pelts, silk, teeth, gems.".into(),
            Role::Innkeeper => "I keep this inn. A bed is ten gold and you wake whole.".into(),
            Role::Lord => format!("I rule {} for the {}. And I have work for those who can swing a sword.", town, n.of),
            Role::Guard => "I keep the walls and pay bounties on the vermin and worse that crowd the roads.".into(),
            Role::Sage => "I read the old accounts. I can tell you where the old places lie, and what was lost in them.".into(),
            Role::Townsfolk => "I keep my head down and my door barred at night.".into(),
        },
        Topic::Trade => {
            said = "Have a look.".into();
            let mut v: Vec<(String, Topic)> = stock(n.role, g).into_iter().map(|(id, cnt)| { let d = data().item(id).unwrap(); let price = d.value * cnt; (format!("Buy {}{} ({} gold)", if cnt > 1 { format!("{} ", cnt) } else { String::new() }, if cnt > 1 { super::item::plural(&d.name) } else { d.name.clone() }, price), Topic::Buy(id.into(), cnt, price)) }).collect();
            if matches!(n.role, Role::Trader) { v.insert(0, ("Sell all loot".into(), Topic::SellLoot)); }
            if matches!(n.role, Role::Smith) { v.insert(0, ("Sell arms and armour from the pack".into(), Topic::SellGear)); }
            v.push(("Back".into(), Topic::Back));
            options = Some(v);
        }
        Topic::Buy(id, cnt, price) => {
            if g.hero.take_gold(price) { stow(&mut g.hero.pack, Item::new(&id, cnt)); said = format!("Here you are: {}. ({} gold left.)", Item::new(&id, cnt).describe(), g.hero.gold()); }
            else { said = format!("That is {} gold, and you have {}. Come back richer.", price, g.hero.gold()); }
            options = Some(t.options.clone());
        }
        Topic::SellLoot | Topic::SellGear => {
            let want = |i: &Item| if topic == Topic::SellLoot { i.def().kind == "loot" && i.id != "key" && i.tag == 0 } else { matches!(i.def().kind.as_str(), "weapon" | "armour" | "shield") && !i.is_artifact() };
            let sold: Vec<Item> = g.hero.pack.iter().filter(|i| want(i)).cloned().collect();
            if sold.is_empty() { said = "You have nothing I buy.".into(); }
            else {
                let pay: u32 = sold.iter().map(|i| if topic == Topic::SellLoot { i.value() } else { (i.value() / 2).max(1) }).sum();
                g.hero.pack.retain(|i| !want(i));
                stow(&mut g.hero.pack, Item::new("gold", pay));
                said = format!("{} for {} gold. Pleasure.", sold.iter().map(|i| i.describe()).collect::<Vec<_>>().join(", "), pay);
            }
            options = Some(t.options.clone());
        }
        Topic::SellOne(_) => {}
        Topic::Quest => {
            // Work already given and not done?
            if let Some(q) = g.quests.iter().find(|q| q.giver == n.name && q.state == State::Open) { said = format!("You have my commission already: {}. ({})", q.title, q.progress()); }
            else if let Some(q) = quest::offer(g, town_id(g), &n.name, n.role) {
                said = format!("{} I will pay {} gold.", q.text, q.gold);
                offer = Some(q);
                options = Some(vec![("Accept".into(), Topic::Accept), ("Not now".into(), Topic::Back)]);
            } else { said = "I have no work for you now. Come back when you are stronger, or when the land is worse.".into(); }
        }
        Topic::Accept => {
            if let Some(q) = offer.take() {
                said = "Good. Do not come back without it done.".into();
                g.say(Tone::Quest, format!("New quest: {}. {}", q.title, q.text));
                // Where it is: the adventurer now knows the place.
                if let quest::Goal::Slay { site, .. } | quest::Goal::Fetch { site, .. } = &q.goal { if !g.known.contains(site) { g.known.push(*site); } }
                g.quests.push(q);
            }
        }
        Topic::Report(k) => { let lines = quest::report(g, k); said = lines.join(" "); for l in &lines { g.say(Tone::Quest, l.clone()); } }
        Topic::Rumours => {
            // A place the adventurer does not know, near, and what the town has heard of the world.
            let here = g.place().map(|p| p.spec.tile).unwrap_or(g.tile);
            let w = g.world.w;
            let unknown = g.sites.iter().filter(|s| !g.known.contains(&s.id) && s.kind != super::site::SiteKind::Town && s.kind != super::site::SiteKind::Wilds)
                .min_by_key(|s| { let dx = (s.tile.0 as i32 - here.0 as i32).abs(); (dx.min(w as i32 - dx)).max((s.tile.1 as i32 - here.1 as i32).abs()) }).cloned();
            let news = g.place().and_then(|p| { let n = &p.spec.news; if n.is_empty() { None } else { Some(n[(g.turn as usize / 100) % n.len()].clone()) } });
            let mut parts = Vec::new();
            if let Some(s) = unknown {
                let d = quest::direction(here, s.tile, w);
                let dd = { let dx = (s.tile.0 as i32 - here.0 as i32).abs(); (dx.min(w as i32 - dx)).max((s.tile.1 as i32 - here.1 as i32).abs()) };
                parts.push(format!("They say there is {} to the {}, {} days' walk: {}.{}", s.kind.word(), d, dd, s.name, if s.cause.is_empty() { String::new() } else { format!(" {}", s.cause) }));
                g.known.push(s.id);
            }
            if let Some(nw) = news { parts.push(format!("And the news from the world: {}", nw)); }
            said = if parts.is_empty() { "Nothing new. The roads are quiet, for once.".into() } else { parts.join(" ") };
        }
        Topic::Places => {
            let price = 25;
            if g.hero.take_gold(price) {
                let here = g.place().map(|p| p.spec.tile).unwrap_or(g.tile);
                let w = g.world.w;
                let mut near: Vec<(i32, u32)> = g.sites.iter().filter(|s| !g.known.contains(&s.id) && s.kind != super::site::SiteKind::Wilds).map(|s| { let dx = (s.tile.0 as i32 - here.0 as i32).abs(); ((dx.min(w as i32 - dx)).max((s.tile.1 as i32 - here.1 as i32).abs()), s.id) }).collect();
                near.sort();
                let ids: Vec<u32> = near.iter().take(4).map(|x| x.1).collect();
                let names: Vec<String> = ids.iter().filter_map(|id| g.site(*id)).map(|s| format!("{} ({}, {})", s.name, s.kind.word(), quest::direction(here, s.tile, w))).collect();
                for id in ids { g.known.push(id); }
                said = if names.is_empty() { "You know every place I know.".into() } else { format!("For {} gold, the old maps: {}. They are on your map now.", price, names.join("; ")) };
            } else { said = format!("The maps are {} gold.", price); }
        }
        Topic::Heal => {
            let free = g.hero.level <= 15;
            let cost = if free { 0 } else { g.hero.level * 5 };
            if g.hero.hp >= g.hero.max_hp() && g.hero.poisoned == 0 { said = "You are whole. Go with the god.".into(); }
            else if g.hero.take_gold(cost) { g.hero.hp = g.hero.max_hp(); g.hero.poisoned = 0; said = if free { "You are healed. The god asks nothing of the young.".into() } else { format!("You are healed. ({} gold to the temple.)", cost) }; }
            else { said = format!("The temple asks {} gold of one of your standing.", cost); }
        }
        Topic::Calling => {
            if g.hero.level < 8 { said = format!("Come back at level 8, child. You are level {}.", g.hero.level); }
            else {
                said = "Choose, and choose well: it is for life.".into();
                let mut v: Vec<(String, Topic)> = data().callings.iter().map(|c| (format!("{}: {}", super::game::cap(&c.name), c.desc), Topic::Become(c.id.clone()))).collect();
                v.push(("Not yet".into(), Topic::Back));
                options = Some(v);
            }
        }
        Topic::Become(c) => {
            g.hero.calling = Some(c.clone());
            g.hero.hp = g.hero.max_hp();
            g.hero.mana = g.hero.max_mana();
            said = format!("Kneel. Rise, {} the {}.", g.hero.name, c);
            g.say(Tone::Level, format!("You are a {} now.", c));
        }
        Topic::Spells => {
            let mut v: Vec<(String, Topic)> = g.hero.may_learn().into_iter().map(|s| { let price = 30 * s.level * s.level / 2; (format!("{} \"{}\": level {}, {} mana ({} gold)", s.name, s.words, s.level, s.mana, price), Topic::Learn(s.id.clone(), price)) }).collect();
            said = if v.is_empty() { "I have nothing more to teach you.".into() } else { "These words I can teach you.".into() };
            v.push(("Back".into(), Topic::Back));
            options = Some(v);
        }
        Topic::Learn(id, price) => {
            let sp = data().spell(&id).unwrap();
            if g.hero.level < sp.level { said = format!("You need level {} for {}.", sp.level, sp.name); }
            else if g.hero.take_gold(price) { g.hero.spells.push(id.clone()); said = format!("Say it with me: \"{}\". You know {} now.", sp.words, sp.name); g.say(Tone::Level, format!("You learned {} ({}).", sp.name, sp.words)); }
            else { said = format!("That is {} gold.", price); }
            let mut v: Vec<(String, Topic)> = g.hero.may_learn().into_iter().map(|s| { let price = 30 * s.level * s.level / 2; (format!("{} \"{}\": level {}, {} mana ({} gold)", s.name, s.words, s.level, s.mana, price), Topic::Learn(s.id.clone(), price)) }).collect();
            v.push(("Back".into(), Topic::Back));
            options = Some(v);
        }
        Topic::Rest(price) => {
            if g.hero.take_gold(price) {
                g.hero.hp = g.hero.max_hp(); g.hero.mana = g.hero.max_mana(); g.hero.fed = g.hero.fed.max(600); g.hero.poisoned = 0;
                g.turn += 6000;
                said = "You sleep through the night and wake whole, with breakfast.".into();
            } else { said = "Ten gold for the bed.".into(); }
        }
        Topic::Back => {}
        Topic::Bye => { g.say(Tone::Talk, format!("{}: \"Good bye, {}.\"", n.name, g.hero.name)); g.talk = None; return; }
    }
    if !said.is_empty() { g.say(Tone::Talk, format!("{}: \"{}\"", n.name, said)); }
    let menu = options.unwrap_or_else(|| main_menu(g, n.role));
    if let Some(t) = g.talk.as_mut() { if !said.is_empty() { t.said = said; } t.options = menu; t.offer = offer; }
}
