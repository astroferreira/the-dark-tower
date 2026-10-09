//! Talking to the townsfolk (Tibia's keywords as choices): their name, their job, trade, work,
//! rumours (what their town has heard of the history, and of places to go), healing and a calling
//! at the temple, spells to learn, a bed at the inn.

use super::actor::Role;
use super::data::data;
use super::game::{Game, Tone};
use super::item::{stow, Item};
use super::quest::{self, Quest, State};

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Topic {
    Name, Job, Trade, Buy(String, u32, u32), SellLoot, SellGear, SellOne(usize), Quest, Accept, Report(usize), Rumours, Places,
    Heal, Calling, Become(String), Spells, Learn(String, u32), Rest(u32), Bye, Back,
    Bless(u32), Improve, Refine(super::hero::Slot, u32), Hire(u32),
    /// The Mapmaker's charts sold to a sage (gold for the tiles inked on foot).
    Charts(u32),
    /// What the inn's bard sings of the adventurer (from the history: what this town has heard).
    Songs,
    /// Ask about someone or something by name (`lore::ask`), the things the town knows to ask
    /// about, and a name of one's own (typed in the window).
    Ask(String),
    AskMenu,
    AskTyped,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
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

/// The town of the one spoken to (their home; else the town one stands in).
fn town_id(g: &Game) -> u32 {
    let home = g.talk.as_ref().and_then(|t| g.place().and_then(|p| p.npcs.get(t.npc))).map_or(0, |n| n.home);
    if home != 0 { home } else { g.site_here() }
}

/// The one spoken to: who they are (their temper) and what they remember of the adventurer.
fn speaker(g: &Game) -> Option<(super::actor::Npc, super::people::Temper)> {
    let n = g.talk.as_ref().and_then(|t| g.place().and_then(|p| p.npcs.get(t.npc))).cloned()?;
    let t = super::people::temper(&super::people::persona(&n));
    Some((n, t))
}

/// Their price for something worth `v` (their temper and memory).
fn their_price(g: &Game, v: u32) -> u32 { speaker(g).map_or(v, |(n, t)| ((v as f32) * super::people::price_factor(t, &n.met) * super::regard::price_factor(g.regard_of(n.home))).round().max(1.0) as u32) }

/// How many towns sing of the adventurer.
fn fame(g: &Game) -> usize { g.songs.len() }

/// What a sage pays for a tile inked on foot.
fn chart_price(g: &Game) -> u32 { 3 + g.hero.level / 4 }

fn main_menu(g: &Game, role: Role) -> Vec<(String, Topic)> {
    let mut v = vec![("Name".to_string(), Topic::Name), ("Job".into(), Topic::Job)];
    match role {
        Role::Smith | Role::Trader | Role::Innkeeper => { v.push(("Trade".into(), Topic::Trade)); if role == Role::Smith { v.push(("Improve what you wear".into(), Topic::Improve)); } }
        Role::Priest => {
            v.push(("Heal".into(), Topic::Heal));
            if g.hero.calling.is_none() { v.push(("Calling".into(), Topic::Calling)); }
            v.push(("Spells".into(), Topic::Spells));
            if !g.hero.blessed { let p = g.hero.blessing_price(); v.push((format!("A blessing: your next death costs nothing ({} gold)", p), Topic::Bless(p))); }
        }
        Role::Sage => {
            v.push(("Places".into(), Topic::Places));
            v.push(("Runes".into(), Topic::Trade));
            if g.charted > 0 { let p = g.charted * chart_price(g); v.push((format!("Sell your charts: {} lands newly mapped ({} gold)", g.charted, p), Topic::Charts(p))); }
        }
        _ => {}
    }
    if role == Role::Innkeeper {
        v.push(("A song from the bard".into(), Topic::Songs));
        v.push(("A bed (10 gold)".into(), Topic::Rest(10)));
        if g.companion.is_none() { let p = 80 * g.hero.level.max(1); v.push((format!("A sellsword to go with you ({} gold)", p), Topic::Hire(p))); }
    }
    if matches!(role, Role::Lord | Role::Guard | Role::Priest | Role::Sage | Role::Trader | Role::Townsfolk) {
        let mine: Vec<usize> = g.quests.iter().enumerate().filter(|(_, q)| q.town == town_id(g) && q.giver == g.place().map(|p| p.npcs.get(g.talk.as_ref().map_or(usize::MAX, |t| t.npc)).map(|n| n.name.clone()).unwrap_or_default()).unwrap_or_default() && q.state == State::Done).map(|(i, _)| i).collect();
        for i in mine { v.push((format!("Report: {}", g.quests[i].title), Topic::Report(i))); }
        v.push(("Quest".into(), Topic::Quest));
    }
    if matches!(role, Role::Townsfolk | Role::Innkeeper | Role::Guard | Role::Sage | Role::Lord) { v.push(("Rumours".into(), Topic::Rumours)); }
    if g.history.is_some() { v.push(("Ask about...".into(), Topic::AskMenu)); }
    v.push(("Bye".into(), Topic::Bye));
    v
}

/// Begin talking to person `k` of the place.
pub fn greet(g: &mut Game, k: usize) {
    let Some(n) = g.place().and_then(|p| p.npcs.get(k)).cloned() else { return };
    let hero = g.hero.name.clone();
    let town = g.site(n.home).map(|s| s.name.clone()).or_else(|| g.place().map(|p| p.spec.name.clone())).unwrap_or_default();
    let temper = super::people::temper(&super::people::persona(&n));
    let said = match n.role {
        Role::Sage if n.of == "hermit" && n.met.times == 0 => format!("Few come this way. I am {}, and I have walked this country forty years. I know where things lie, {}.", n.name, hero),
        Role::Townsfolk if n.of == "farmer" && n.met.times == 0 => format!("Morning, {}. Mind the fields. And the wolves, after dark.", hero),
        Role::Townsfolk if n.of == "drunk" => format!("Shiddown, shtranger! {}, ish it? Buy me a drink and I'll tell you everything. Everything! Ask me anything.", hero),
        Role::Lord if n.met.times == 0 && n.met.wronged == 0 && g.site(n.home).map_or(false, |s| s.lord.is_some()) => format!("You stand before {}, {}. Speak, {}.", n.name, n.of, hero),
        _ => super::people::greeting(&n, temper, &n.met, &hero, &town, fame(g), g.turn),
    };
    // What their town thinks of the hero colours it.
    let r = g.regard_of(n.home);
    let said = if r <= -40 && n.role != Role::Townsfolk { format!("\"You. Get out of my sight before I call the watch, {}.\"", hero) }
        else if r >= 30 && n.met.wronged == 0 { format!("{} (Friend of {}, the whole town knows your name.)", said, town) } else { said };
    let barred = r <= -40 && n.role != Role::Townsfolk;
    // They will remember this meeting.
    let turn = g.turn;
    if let Some(p) = g.place_mut() { if let Some(m) = p.npcs.get_mut(k) { m.met.times += 1; m.met.last = turn; } }
    g.talk = Some(Talk { npc: k, name: n.name.clone(), role: n.role, said, options: Vec::new(), offer: None });
    // A parcel for this town's trader: delivered and paid for here.
    if n.role == Role::Trader {
        let here = town_id(g);
        if let Some(qk) = g.quests.iter().position(|q| q.state == State::Open && matches!(q.goal, quest::Goal::Deliver { town, .. } if town == here)) {
            let quest::Goal::Deliver { tag, .. } = g.quests[qk].goal.clone() else { unreachable!() };
            if let Some(i) = g.hero.pack.iter().position(|i| i.tag == tag) {
                g.hero.pack.remove(i);
                g.quests[qk].state = State::Done;
                let lines = quest::report(g, qk);
                for l in &lines { g.say(Tone::Quest, l.clone()); }
                if let Some(t) = g.talk.as_mut() { t.said = format!("A parcel from afar? Ah, I've been waiting for that. {}", lines.join(" ")); }
            }
        }
    }
    let menu = if barred { vec![("Bye".to_string(), Topic::Bye)] } else { main_menu(g, n.role) };
    if let Some(t) = g.talk.as_mut() { t.options = menu; }
    g.say(Tone::Talk, format!("{}: \"{}\"", n.name, g.talk.as_ref().unwrap().said));
    // The other house of a feud hears what the hero came to say.
    super::tales::on_greet(g, &n.name, n.home);
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
        Role::Sage => { let mut v = vec![("rune_flame", 1), ("rune_holy", 1)]; if lvl >= 12 { v.extend([("rune_heal", 1), ("rune_stones", 1)]); } if lvl >= 20 { v.push(("rune_fire", 1)); } v }
        _ => Vec::new(),
    }
}

/// Choose option `i` in the open conversation.
pub fn answer(g: &mut Game, i: usize) {
    let Some(t) = g.talk.clone() else { return };
    let Some((_, topic)) = t.options.get(i).cloned() else { return };
    answer_topic(g, topic);
}

/// Ask the one spoken to about a typed name.
pub fn ask_typed(g: &mut Game, q: &str) { g.typing = None; if !q.trim().is_empty() { answer_topic(g, Topic::Ask(q.trim().to_string())); } }

/// Take up `topic` in the open conversation.
pub fn answer_topic(g: &mut Game, topic: Topic) {
    let Some(t) = g.talk.clone() else { return };
    let n = match g.place().and_then(|p| p.npcs.get(t.npc)).cloned() { Some(n) => n, None => { g.talk = None; return } };
    let mut said = String::new();
    let mut options: Option<Vec<(String, Topic)>> = None;
    let mut offer = t.offer.clone();
    let town = g.site(n.home).map(|s| s.name.clone()).or_else(|| g.place().map(|p| p.spec.name.clone())).unwrap_or_default();
    match topic {
        Topic::Name => {
            let p = super::people::persona(&n);
            let looks = p.looks_text(&n.name, 20 + (crate::persona::seed_of(&n.name, 7) % 45) as u32);
            said = format!("I am {}, {} of {}.", n.name, n.role.word(), town);
            let more: Vec<String> = [Some(looks), p.character_text(), Some(p.likes_text())].into_iter().flatten().filter(|t| !t.is_empty()).collect();
            if !more.is_empty() { g.say(Tone::Info, more.join(" ")); }
        }
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
            let mut v: Vec<(String, Topic)> = stock(n.role, g).into_iter().map(|(id, cnt)| { let d = data().item(id).unwrap(); let price = their_price(g, d.value * cnt); (format!("Buy {}{} ({} gold)", if cnt > 1 { format!("{} ", cnt) } else { String::new() }, if cnt > 1 { super::item::plural(&d.name) } else { d.name.clone() }, price), Topic::Buy(id.into(), cnt, price)) }).collect();
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
                let f = speaker(g).map_or(1.0, |(n, t)| super::people::price_factor(t, &n.met));
                let pay: u32 = (sold.iter().map(|i| if topic == Topic::SellLoot { i.value() } else { (i.value() / 2).max(1) }).sum::<u32>() as f32 / f).round() as u32;
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
            } else { said = speaker(g).map_or("I have no work for you now.", |(_, t)| super::people::refusal(t)).into(); }
        }
        Topic::Accept => {
            if let Some(q) = offer.take() {
                said = "Good. Do not come back without it done.".into();
                g.say(Tone::Quest, format!("New quest: {}. {}", q.title, q.text));
                // Where it is: the adventurer now knows the place.
                if let quest::Goal::Slay { site, .. } | quest::Goal::Fetch { site, .. } | quest::Goal::Deliver { town: site, .. } = &q.goal { if !g.known.contains(site) { g.known.push(*site); } }
                if let quest::Goal::Deliver { tag, town } = &q.goal {
                    let mut parcel = Item::new("linen", 1);
                    parcel.name = Some(format!("a sealed parcel for {}", g.site(*town).map(|s| s.name.clone()).unwrap_or_default()));
                    parcel.tag = *tag;
                    stow(&mut g.hero.pack, parcel);
                }
                g.quests.push(q.clone());
                super::tales::on_accept(g, &q);
            }
        }
        Topic::Report(k) => {
            let title = g.quests.get(k).map(|q| q.title.clone()).unwrap_or_default();
            let lines = quest::report(g, k);
            said = lines.join(" ");
            for l in &lines { g.say(Tone::Quest, l.clone()); }
            // They remember who did it.
            let who = t.npc;
            if let Some(p) = g.place_mut() { if let Some(m) = p.npcs.get_mut(who) { m.met.helped.push(title); } }
        }
        Topic::Rumours => {
            // A place the adventurer does not know, near, and what the town has heard of the world.
            let here = g.place().map(|p| p.spec.tile).unwrap_or(g.tile);
            let w = g.world.w;
            let unknown = g.sites.iter().filter(|s| !g.known.contains(&s.id) && s.kind != super::site::SiteKind::Town && s.kind != super::site::SiteKind::Wilds && s.kind != super::site::SiteKind::Cellar)
                .min_by_key(|s| { let dx = (s.tile.0 as i32 - here.0 as i32).abs(); (dx.min(w as i32 - dx)).max((s.tile.1 as i32 - here.1 as i32).abs()) }).cloned();
            let news = g.site(town_id(g)).and_then(|s| s.notes.last().cloned()).or_else(|| g.place().and_then(|p| { let n = &p.spec.news; if n.is_empty() { None } else { Some(n[(g.turn as usize / 100) % n.len()].clone()) } }));
            let mut parts = Vec::new();
            if let Some(s) = unknown {
                let d = quest::direction(here, s.tile, w);
                let dd = { let dx = (s.tile.0 as i32 - here.0 as i32).abs(); (dx.min(w as i32 - dx)).max((s.tile.1 as i32 - here.1 as i32).abs()) };
                parts.push(format!("They say there is {} to the {}, {} days' walk: {}.{}", s.kind.word(), d, dd, s.name, if s.cause.is_empty() { String::new() } else { format!(" {}", s.cause) }));
                g.known.push(s.id);
                g.rumour(s.tile);
            }
            if let Some(nw) = news { parts.push(format!("And the news from the world: {}", nw)); }
            said = if parts.is_empty() { "Nothing new. The roads are quiet, for once.".into() } else { parts.join(" ") };
        }
        Topic::Places => {
            let price = 25;
            if g.hero.take_gold(price) {
                let here = g.place().map(|p| p.spec.tile).unwrap_or(g.tile);
                let w = g.world.w;
                let mut near: Vec<(i32, u32)> = g.sites.iter().filter(|s| !g.known.contains(&s.id) && s.kind != super::site::SiteKind::Wilds && s.kind != super::site::SiteKind::Cellar).map(|s| { let dx = (s.tile.0 as i32 - here.0 as i32).abs(); ((dx.min(w as i32 - dx)).max((s.tile.1 as i32 - here.1 as i32).abs()), s.id) }).collect();
                near.sort();
                let ids: Vec<u32> = near.iter().take(4).map(|x| x.1).collect();
                let names: Vec<String> = ids.iter().filter_map(|id| g.site(*id)).map(|s| format!("{} ({}, {})", s.name, s.kind.word(), quest::direction(here, s.tile, w))).collect();
                for id in ids { g.known.push(id); if let Some(t) = g.site(id).map(|s| s.tile) { g.rumour(t); } }
                // The sage's old maps sketch in the country between.
                let (w, h) = (g.world.w as i32, g.world.h as i32);
                for dy in -5..=5 { for dx in -5..=5 { let y = here.1 as i32 + dy; if y >= 0 && y < h && dx * dx + dy * dy <= 25 { g.rumour((((here.0 as i32 + dx).rem_euclid(w)) as usize, y as usize)); } } }
                said = if names.is_empty() { "You know every place I know.".into() } else { format!("For {} gold, the old maps: {}. They are on your map now.", price, names.join("; ")) };
            } else { said = format!("The maps are {} gold.", price); }
        }
        Topic::Heal => {
            // (The young are healed free; a timid priest asks nothing of a hero the songs tell of.)
            let free = g.hero.level <= 15 || (fame(g) >= 3 && speaker(g).map_or(false, |(_, t)| t == super::people::Temper::Timid));
            let cost = if free { 0 } else { g.hero.level * 5 };
            // The temple feeds the hungry who cannot pay.
            if g.hero.fed < 300 && g.hero.gold() < 10 && !g.hero.pack.iter().any(|i| i.def().kind == "food") { stow(&mut g.hero.pack, Item::new("bread", 2)); g.say(Tone::Info, "The priest presses two loaves into your hands. \"The god feeds the hungry.\""); }
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
            // The calling's first weapon, from the temple.
            let gift: Vec<Item> = match c.as_str() { "paladin" => vec![Item::new("bow", 1), Item::new("arrow", 60)], "sorcerer" => vec![Item::new("wand_of_embers", 1)], "druid" => vec![Item::new("snakebite_rod", 1)], _ => vec![Item::of("sword", "iron", 1)] };
            let gift_words: Vec<String> = gift.iter().map(|i| i.describe()).collect();
            for it in gift { stow(&mut g.hero.pack, it); }
            said = format!("Kneel. Rise, {} the {}. Take {}: it is the temple's gift to every {}.", g.hero.name, c, crate::persona::list(&gift_words), c);
            g.say(Tone::Level, format!("You are a {} now.", c));
        }
        Topic::Spells => {
            let mut v: Vec<(String, Topic)> = g.hero.may_learn().into_iter().map(|s| { let price = 40 * s.level.max(1); (format!("{} \"{}\": level {}, {} mana ({} gold)", s.name, s.words, s.level, s.mana, price), Topic::Learn(s.id.clone(), price)) }).collect();
            said = if v.is_empty() { "I have nothing more to teach you.".into() } else { "These words I can teach you.".into() };
            v.push(("Back".into(), Topic::Back));
            options = Some(v);
        }
        Topic::Learn(id, price) => {
            let sp = data().spell(&id).unwrap();
            if g.hero.level < sp.level { said = format!("You need level {} for {}.", sp.level, sp.name); }
            else if g.hero.take_gold(price) { g.hero.spells.push(id.clone()); said = format!("Say it with me: \"{}\". You know {} now.", sp.words, sp.name); g.say(Tone::Level, format!("You learned {} ({}).", sp.name, sp.words)); }
            else { said = format!("That is {} gold.", price); }
            let mut v: Vec<(String, Topic)> = g.hero.may_learn().into_iter().map(|s| { let price = 40 * s.level.max(1); (format!("{} \"{}\": level {}, {} mana ({} gold)", s.name, s.words, s.level, s.mana, price), Topic::Learn(s.id.clone(), price)) }).collect();
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
        Topic::Bless(price) => {
            if g.hero.blessed { said = "You are blessed already.".into(); }
            else if g.hero.take_gold(price) { g.hero.blessed = true; said = format!("Kneel. {} keeps you: the next time you fall, you lose nothing.", n.of); g.say(Tone::Level, "You are blessed."); }
            else { said = format!("The blessing asks {} gold of one of your standing.", price); }
        }
        Topic::Improve => {
            let mut v: Vec<(String, Topic)> = super::hero::Slot::ALL.iter().filter_map(|s| g.hero.equipped[*s as usize].as_ref().filter(|i| i.quality < 5 && !i.is_artifact() && matches!(i.def().kind.as_str(), "weapon" | "armour" | "shield")).map(|i| {
                let price = super::hero::Hero::refine_price(i);
                (format!("Your {}: to {} ({} gold)", i.short(), super::item::QUALITY[(i.quality + 1) as usize].1, price), Topic::Refine(*s, price))
            })).collect();
            said = if v.is_empty() { "There is nothing on you I can make finer.".into() } else { "Let me see what you wear. Each step finer costs more.".into() };
            v.push(("Back".into(), Topic::Back));
            options = Some(v);
        }
        Topic::Refine(slot, price) => {
            if g.hero.take_gold(price) {
                if let Some(it) = g.hero.equipped[slot as usize].as_mut() { it.quality = (it.quality + 1).min(5); said = format!("Hammer and file and a night at the forge: it is {} now.", it.describe()); }
            } else { said = format!("That is {} gold.", price); }
        }
        Topic::Hire(price) => {
            if g.companion.is_some() { said = "You have a blade at your side already.".into(); }
            else if g.hero.take_gold(price) {
                let race = g.place().map(|p| p.spec.people.clone()).filter(|r| !r.is_empty()).unwrap_or_else(|| "human".into());
                let name = super::town::person_name(&race, g.seed ^ g.turn ^ 0x5E11);
                let lvl = g.hero.level as i32;
                g.companion = Some(super::game::Companion { name: name.clone(), race, hp: 50 + 12 * lvl, max_hp: 50 + 12 * lvl, x: g.x, y: g.y, energy: 0, left: false, kills: 0, struck_at: 0, morale: 100, paid_day: g.turn / super::land::DAY });
                g.companion_follow(true);
                let t = g.companion.as_ref().map(|c| c.temper()).unwrap_or(super::people::Temper::Plain);
                said = format!("{} drains the cup, takes your coin and picks up a spear. \"Lead on.\" ({} by the look of them{})", name, t.word(), if t == super::people::Temper::Greedy { "; they will want wages every ten days" } else { "" });
                g.say(Tone::Level, format!("{} goes with you now.", name));
            } else { said = format!("A good blade costs {} gold.", price); }
        }
        Topic::Charts(price) => {
            let n_tiles = g.charted;
            g.charted = 0;
            stow(&mut g.hero.pack, Item::new("gold", price));
            g.stats.gold_found += price;
            said = format!("{} lands I had only guessed at, drawn true. Here: {} gold, and come back with more.", n_tiles, price);
        }
        Topic::Songs => {
            let town = town_id(g);
            said = match g.songs.get(&town).filter(|v| !v.is_empty()) {
                Some(v) => {
                    let k = (g.turn as usize / 100) % v.len();
                    format!("The bard tunes up and sings of {}: {}. The room drinks to it.", g.hero.name, v[k])
                }
                None => "The bard shrugs. \"No one sings of you here. Not yet. Do something worth a song.\"".into(),
            };
        }
        Topic::AskMenu => {
            let town = g.site(n.home).and_then(|s| s.settlement).map(crate::history::SettlementId);
            let mut v: Vec<(String, Topic)> = super::lore::subjects(g, town, 6).into_iter().map(|nm| (format!("Ask about {}", nm), Topic::Ask(nm))).collect();
            v.push(("Ask about someone or something else (type a name)".into(), Topic::AskTyped));
            v.push(("Back".into(), Topic::Back));
            said = "What do you want to know?".into();
            options = Some(v);
        }
        Topic::AskTyped => { said = "Type a name and press Enter.".into(); options = Some(t.options.clone()); g.typing = Some(String::new()); }
        Topic::Ask(q) => {
            let town = g.site(n.home).and_then(|s| s.settlement).map(crate::history::SettlementId);
            said = super::lore::ask(g, &n, town, &q);
        }
        Topic::Back => {}
        Topic::Bye => { g.say(Tone::Talk, format!("{}: \"Good bye, {}.\"", n.name, g.hero.name)); g.talk = None; return; }
    }
    if !said.is_empty() { g.say(Tone::Talk, format!("{}: \"{}\"", n.name, said)); }
    let menu = options.unwrap_or_else(|| main_menu(g, n.role));
    if let Some(t) = g.talk.as_mut() { if !said.is_empty() { t.said = said; } t.options = menu; t.offer = offer; }
}
