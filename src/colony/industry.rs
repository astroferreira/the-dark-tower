//! Industries: workshops below the camp and the materials that pass between them.
//!
//! The idea is Dwarf Fortress's production chains (type x material: the material decides what a
//! thing is and what it is good for): ore is only rock until a smelter turns it into bars with
//! fuel (charcoal burnt from logs, or nothing at all over the magma), and bars are only metal
//! until a forge turns them into tools, spearheads and mail; stone is dressed into blocks at a
//! mason's, logs into barrels at a carpenter's, clay and sand into pots and glass at a kiln.
//! Each workshop is a room cut off the stair (`delve.rs::plan_level`, `RoomKind::Mason` ...),
//! planned when its material is to hand (`industry_candidates`); each job is a `Job::Craft` with
//! its why's first words (`industry_option`, `finish_industry`), one hand at a shop at a time.
//! The stock is kept here (`Industry`), apart from the hauled `ItemKind`s: ore by kind, bars by
//! metal, charcoal, blocks, barrels, clay and sand. What it feeds: furniture of blocks
//! (`delve.rs`), iron tools (`iron_worked`: work 0.65x, two more picks), spearheads and mail of
//! the metal (`militia.rs`, `armour.rs`, which take bars), wine racked into barrels
//! (`drink.rs`), pots and glass for the caravans, who also buy spare blocks, bars and barrels
//! (`sell_stock`, `trade.rs`); the liaison can ask for ore or charcoal (`liaison.rs`).

use super::*;
use super::delve::RoomKind;
use super::projects::ProjectKind;
use crate::persona::{Attr, Facet};

/// The camp's worked materials.
#[derive(Clone, Debug, Default)]
pub struct Industry {
    /// Loads of ore by kind ("iron", "copper", "adamantine"): dug, found or bought.
    pub ore: Vec<(String, u32)>,
    /// Bars of metal by kind (adamantine: wafers).
    pub bars: Vec<(String, u32)>,
    pub charcoal: u32,
    /// Dressed stone blocks.
    pub blocks: u32,
    pub barrels: u32,
    /// Clay and sand dug out of the ground (for the kiln).
    pub clay: u32,
    pub sand: u32,
    /// Each hand's skill at the smelter and the forge (0..1), from their building hand at first.
    pub smith: crate::history::det::HashMap<usize, f32>,
    /// The camp's forged tools: their metal and the day.
    pub tools: Option<(String, u64)>,
    /// Tallies: ore smelted, bars made, charcoal burnt, stones dressed, barrels made, kiln
    /// works fired, arms and mail forged.
    pub smelted: u32,
    pub bars_made: u32,
    pub burned: u32,
    pub dressed: u32,
    pub barrels_made: u32,
    pub fired: u32,
    pub forged: u32,
    /// First times already told ("smelt iron", "blocks", ...).
    pub(crate) told: Vec<String>,
}

/// The room a workshop project cuts.
pub fn shop_room(k: ProjectKind) -> Option<RoomKind> {
    Some(match k {
        ProjectKind::MasonShop => RoomKind::Mason, ProjectKind::CarpenterShop => RoomKind::Carpenter, ProjectKind::Smelter => RoomKind::Smelter,
        ProjectKind::Forge => RoomKind::Forge, ProjectKind::Kiln => RoomKind::Kiln, _ => return None,
    })
}

/// The industries' jobs, by the first words of their why.
const JOBS: [&str; 7] = ["Dressing stone", "Making a barrel", "Burning charcoal", "Smelting", "Forging", "Firing", "Making tools"];

/// Whether a craft's why is one of the industries' jobs.
pub fn is_industry(why: &str) -> bool { JOBS.iter().any(|j| why.starts_with(j)) }

/// Minutes a job takes at its bench (before skill): charcoal 120, a block or a barrel 150,
/// smelting and firing 240, forging 300.
pub fn minutes(why: &str) -> Option<u32> {
    if why.starts_with("Burning charcoal") { Some(120) }
    else if why.starts_with("Dressing stone") || why.starts_with("Making a barrel") { Some(150) }
    else if why.starts_with("Smelting") || why.starts_with("Firing") { Some(240) }
    else if why.starts_with("Forging") || why.starts_with("Making tools") { Some(300) }
    else { None }
}

/// Bars a mail shirt takes (a spearhead takes one).
pub const MAIL_BARS: u32 = 2;
/// Bars a set of tools takes.
const TOOL_BARS: u32 = 2;
/// Blocks kept for furniture before the caravans may buy the rest, and the mason's goal.
const BLOCKS_KEPT: u32 = 6;
const BLOCKS_GOAL: u32 = 12;
/// Bars of each metal kept for arms before the caravans may buy the rest.
const BARS_KEPT: u32 = 4;
/// Barrels the still keeps (each a cup more a brewing); the rest go to the traders.
const BARRELS_KEPT: u32 = 3;

/// The metals arms are forged of, best first.
const ARM_METALS: [&str; 3] = ["adamantine", "iron", "copper"];

fn count(v: &[(String, u32)], k: &str) -> u32 { v.iter().find(|x| x.0 == k).map_or(0, |x| x.1) }

fn add(v: &mut Vec<(String, u32)>, k: &str, n: u32) {
    match v.iter_mut().find(|x| x.0 == k) { Some(x) => x.1 += n, None => v.push((k.to_string(), n)) }
}

impl Colony {
    /// The bench of a dug workshop of this kind.
    pub(crate) fn shop(&self, k: RoomKind) -> Option<Pos> { self.rooms.iter().find(|r| r.kind == k).and_then(|r| r.bed) }

    /// Metal can be worked: a forge below, or the forge over the magma (`deep.rs`).
    pub fn metal_forge(&self) -> bool { self.magma_forge || self.shop(RoomKind::Forge).is_some() }

    pub fn bars_of(&self, metal: &str) -> u32 { count(&self.industry.bars, metal) }
    pub fn ore_total(&self) -> u32 { self.industry.ore.iter().map(|x| x.1).sum() }

    /// Ore comes in (a seam dug, an old mine's leavings, the deep vein, a caravan's load).
    pub(crate) fn add_ore(&mut self, kind: &str, n: u32) {
        add(&mut self.industry.ore, kind, n);
        if !self.ores.iter().any(|o| o == kind) { self.ores.push(kind.to_string()); }
    }

    /// Takes `n` bars of a metal if the camp has them.
    pub(crate) fn take_bars(&mut self, metal: &str, n: u32) -> bool {
        match self.industry.bars.iter_mut().find(|x| x.0 == metal && x.1 >= n) { Some(x) => { x.1 -= n; true } None => false }
    }

    /// The metal named in a made thing ("an iron-headed spear" -> iron).
    pub fn metal_in(kind: &str) -> Option<&'static str> { ARM_METALS.iter().copied().find(|m| kind.contains(m)) }

    /// A hand's skill at the smelter and the forge (a builder's hands give a start).
    pub fn smith_skill(&self, i: usize) -> f32 {
        self.industry.smith.get(&i).copied().unwrap_or_else(|| 0.5 * self.settlers[i].skill[4])
    }

    /// The force a smith's hand puts into a forged head, as a builder's into a carved one
    /// (`militia.rs`: 0.9 + 0.2 x skill).
    pub fn smith_hand(&self, i: usize) -> f32 { 0.9 + 0.2 * self.smith_skill(i) }

    fn train_smith(&mut self, i: usize) {
        let s = self.smith_skill(i);
        let gain = 0.04 * (1.0 - s) * self.settlers[i].persona.learning();
        self.industry.smith.insert(i, (s + gain).min(1.0));
    }

    /// The quality word of a forged thing (as for any work: `craft::QUALITY`), from the smith's
    /// skill, sure hands and a little luck.
    fn forged_quality(&self, i: usize, salt: u64) -> u8 {
        let h = crate::history::settlers::hash_pub(self.seed ^ self.clock.tick ^ (i as u64) << 20, salt);
        let p = &self.settlers[i].persona;
        let q = 0.5 * self.smith_skill(i) + 0.2 * (p.attr(Attr::KinestheticSense) / 2000.0).min(1.0) + 0.15 * p.facet(Facet::Perfectionism) as f32 / 100.0 + 0.15 * (h % 1000) as f32 / 1000.0;
        (((q - 0.35) * 9.0).floor() as i32).clamp(0, 5) as u8
    }

    fn tell_once(&mut self, key: String, line: String) -> bool {
        if self.industry.told.contains(&key) { return false; }
        self.industry.told.push(key);
        self.note(line);
        true
    }

    fn stored_count(&self, k: ItemKind) -> u32 { self.items.iter().filter(|it| it.stored && it.kind == k).count() as u32 }

    /// Logs and stones laid by beyond what the work under way still needs, less two.
    fn spare(&self, k: ItemKind) -> u32 {
        let (logs, stones) = self.material_needed();
        let need = if k == ItemKind::Log { logs } else { stones };
        self.stored_count(k).saturating_sub(need + 2)
    }

    fn take_stored(&mut self, k: ItemKind) -> bool {
        let Some(it) = self.items.iter().position(|it| it.stored && it.kind == k) else { return false };
        self.items.remove(it);
        self.fix_refs_pub(it);
        true
    }

    /// Where an industry job is done.
    pub(crate) fn industry_spot(&self, why: &str) -> Option<Pos> {
        let k = if why.starts_with("Dressing stone") { RoomKind::Mason } else if why.starts_with("Making a barrel") { RoomKind::Carpenter }
            else if why.starts_with("Burning charcoal") || why.starts_with("Smelting") { RoomKind::Smelter } else if why.starts_with("Firing") { RoomKind::Kiln } else { RoomKind::Forge };
        self.shop(k).or_else(|| if k == RoomKind::Forge { self.workshop_spot() } else { None })
    }

    /// Where arms are made: the forge when a bar is to hand for them, else the workshop.
    pub(crate) fn arms_spot(&self) -> Option<(Pos, &'static str)> {
        if self.metal_forge() && ARM_METALS.iter().any(|m| self.bars_of(m) > 0) {
            if let Some(p) = self.shop(RoomKind::Forge) { return Some((p, "the forge")); }
        }
        self.workshop_spot().map(|p| (p, "the workshop"))
    }

    /// The best metal to hand for a spearhead (1 bar) or a mail shirt (`MAIL_BARS`).
    pub(crate) fn arm_metal(&self, bars: u32) -> Option<&'static str> {
        if !self.metal_forge() { return None; }
        ARM_METALS.iter().copied().find(|m| self.bars_of(m) >= bars)
    }

    /// The industries' work for settler `i` now, if any: by day, grown, well, one hand to a
    /// shop, the material to hand and the stock short.
    pub(crate) fn industry_option(&self, i: usize) -> Option<(f32, Job, String)> {
        if self.clock.is_night() { return None; }
        let shops = self.rooms.iter().any(|r| matches!(r.kind, RoomKind::Mason | RoomKind::Carpenter | RoomKind::Smelter | RoomKind::Forge | RoomKind::Kiln));
        if !shops && !(self.magma_forge && self.workshop_spot().is_some()) { return None; }
        let s = &self.settlers[i];
        if s.ill_until > self.clock.tick || s.guest_until > 0 || s.past.as_ref().map_or(false, |p| p.age < 14) { return None; }
        let busy = |p: &str| self.settlers.iter().enumerate().any(|(j, o)| j != i && o.alive && o.job == Job::Craft && o.why.starts_with(p));
        let hand = 0.8 + 0.4 * s.skill[4];
        let smith = 0.7 + 0.6 * self.smith_skill(i);
        let mut opts: Vec<(f32, String)> = Vec::new();
        let ind = &self.industry;
        // The mason's: rough stone into blocks.
        if ind.blocks < BLOCKS_GOAL && self.shop(RoomKind::Mason).is_some() && !busy("Dressing stone") && self.spare(ItemKind::Stone) >= 1 {
            opts.push((0.4 * hand, format!("Dressing stone into blocks at the mason's ({} laid by)", ind.blocks)));
        }
        // The carpenter's: barrels for the still (three it keeps; more for the traders while
        // logs are plenty).
        if ind.barrels < BARRELS_KEPT + 3 && self.shop(RoomKind::Carpenter).is_some() && !busy("Making a barrel") && self.still().is_some() {
            let spare = self.spare(ItemKind::Log);
            if spare >= 1 && (ind.barrels < BARRELS_KEPT || spare >= 6) {
                opts.push((0.35 * hand, format!("Making a barrel at the carpenter's, {}", if ind.barrels < BARRELS_KEPT { "for the still" } else { "for the traders" })));
            }
        }
        // The smelter: ore and fuel into bars; charcoal burnt from logs while ore waits for it.
        if self.shop(RoomKind::Smelter).is_some() {
            if let Some((ore, n)) = ind.ore.iter().find(|x| x.1 > 0).cloned() {
                if (self.magma_forge || ind.charcoal > 0) && !busy("Smelting") {
                    let fuel = if self.magma_forge { "in the magma's heat".to_string() } else { format!("with charcoal ({} sacks)", ind.charcoal) };
                    opts.push((0.55 * smith, format!("Smelting {} ore at the smelter, {} ({} loads wait)", ore, fuel, n)));
                } else if !self.magma_forge && ind.charcoal < 3 && self.spare(ItemKind::Log) >= 1 && !busy("Burning charcoal") {
                    opts.push((0.5 * hand, format!("Burning charcoal at the smelter: {} loads of {} ore wait for fuel", n, ore)));
                }
            }
        }
        // The kiln: clay into pots, sand into glass, a log for the fire (none over the magma),
        // while the camp has few unsold.
        if self.shop(RoomKind::Kiln).is_some() && (ind.clay >= 2 || ind.sand >= 2) && (self.magma_forge || self.spare(ItemKind::Log) >= 1) && !busy("Firing") {
            let unsold = self.works.iter().filter(|w| !w.traded && (w.material == "fired clay" || w.material == "green glass")).count();
            if unsold < 3 {
                opts.push((0.35 * hand, format!("Firing {} at the kiln", if ind.clay >= 2 { "pots of clay" } else { "glass of sand" })));
            }
        }
        // The forge: tools first, then spears and mail of metal for those who bear wood and leather.
        if self.metal_forge() && !busy("Forging") && !busy("Making tools") {
            let tool_metal = ["iron", "copper"].into_iter().find(|m| self.bars_of(m) >= TOOL_BARS);
            if ind.tools.is_none() && !self.tools_bought && tool_metal.is_some() {
                opts.push((0.6 * smith, format!("Making tools of {} at {}: picks, axes and mallets", tool_metal.unwrap(), if self.shop(RoomKind::Forge).is_some() { "the forge" } else { "the magma forge" })));
            } else if let Some(m) = self.arm_metal(1) {
                // A spear of wood and stone borne by someone, and a shaft to hand.
                let worst = self.arms.iter().filter(|a| a.holder.is_some() && Self::metal_in(&a.kind).is_none()).min_by(|a, b| a.force.total_cmp(&b.force));
                let shaft = self.stored_count(ItemKind::Log) + self.stored_count(ItemKind::Stone) > 0;
                if let (Some(a), true) = (worst, shaft) {
                    let who = self.settlers[a.holder.unwrap()].name.clone();
                    opts.push((0.55 * smith, format!("Forging a spear of {} at the forge, to replace {}'s {}", m, who, a.kind.trim_start_matches("a ").trim_start_matches("an "))));
                } else if let Some(m) = self.arm_metal(MAIL_BARS) {
                    let worst = self.armour.iter().filter(|a| a.holder.is_some() && Self::metal_in(&a.kind).is_none()).min_by(|a, b| a.cover.total_cmp(&b.cover));
                    if let Some(a) = worst {
                        let who = self.settlers[a.holder.unwrap()].name.clone();
                        opts.push((0.5 * smith, format!("Forging mail of {} at the forge, to replace {}'s {}", m, who, a.kind.trim_start_matches("a ").trim_start_matches("an "))));
                    }
                }
            }
        }
        // (Food first: while the store is short of its goal the shops wait on the gatherers.)
        let short = if opts.is_empty() || self.food_stored() >= self.food_goal() { 1.0 } else { 0.6 };
        opts.into_iter().max_by(|a, b| a.0.total_cmp(&b.0)).map(|(w, why)| (w * short, Job::Craft, why))
    }

    /// An industry job done.
    pub(crate) fn finish_industry(&mut self, i: usize) {
        let why = self.settlers[i].why.clone();
        let name = self.settlers[i].name.clone();
        let day = self.clock.day();
        if why.starts_with("Dressing stone") {
            if !self.take_stored(ItemKind::Stone) { return; }
            self.industry.blocks += 2;
            self.industry.dressed += 1;
            let stone = self.land_stone();
            self.tell_once("blocks".into(), format!("{} dresses the first blocks of {} at the mason's: two square blocks from a rough load, for tables and the traders.", name, stone));
        } else if why.starts_with("Making a barrel") {
            if !self.take_stored(ItemKind::Log) { return; }
            self.industry.barrels += 1;
            self.industry.barrels_made += 1;
            let wood = self.land_wood();
            self.tell_once("barrel".into(), format!("{} coopers the first barrel of {} at the carpenter's: the still's wine will be racked into it.", name, wood));
        } else if why.starts_with("Burning charcoal") {
            if !self.take_stored(ItemKind::Log) { return; }
            self.industry.charcoal += 1;
            self.industry.burned += 1;
            self.tell_once("charcoal".into(), format!("{} burns the first charcoal at the smelter: a log smothered in earth until it is black and light.", name));
        } else if why.starts_with("Smelting") {
            let Some(k) = self.industry.ore.iter().position(|x| x.1 > 0) else { return };
            if !self.magma_forge {
                if self.industry.charcoal == 0 { return; }
                self.industry.charcoal -= 1;
            }
            self.industry.ore[k].1 -= 1;
            let metal = self.industry.ore[k].0.clone();
            // Two bars a load (adamantine: one wafer a strand).
            let n = if metal == "adamantine" { 1 } else { 2 };
            add(&mut self.industry.bars, &metal, n);
            self.industry.smelted += 1;
            self.industry.bars_made += n;
            self.train_smith(i);
            let fuel = if self.magma_forge { "in the magma's heat, with no fuel".to_string() } else { "and a sack of charcoal".to_string() };
            let what = if metal == "adamantine" { "a wafer of adamantine from a strand".to_string() } else { format!("two bars of {} from a load of ore {}", metal, fuel) };
            let line = format!("{} smelts the first {} at the smelter: {}.", name, metal, what);
            if self.tell_once(format!("smelt {}", metal), line.clone()) {
                let at = self.shop(RoomKind::Smelter).unwrap_or(self.camp);
                let first = self.industry.told.iter().filter(|t| t.starts_with("smelt ")).count() == 1;
                if first { self.moment("The first bars".into(), line, format!("because the camp had {} ore and a smelter to work it", metal), at); }
            }
        } else if why.starts_with("Making tools") {
            let Some(m) = ["iron", "copper"].into_iter().find(|m| self.bars_of(m) >= TOOL_BARS) else { return };
            self.take_bars(m, TOOL_BARS);
            self.industry.tools = Some((m.to_string(), day));
            self.train_smith(i);
            let line = format!("{} forges a set of {} tools: picks, axes and mallets of {} for the camp's work. Felling, quarrying, building and digging go quicker, and two more can dig at once.", name, m, m);
            self.note(line.clone());
            let at = self.shop(RoomKind::Forge).unwrap_or(self.camp);
            self.moment(format!("Tools of {}", m), line, format!("because the smelter gave bars of {} and the forge turned them into tools", m), at);
        } else if why.starts_with("Forging a spear") {
            // A new spear of metal (`finish_arm` takes the bar and the shaft); the poorest one
            // left without a bearer is broken up for its shaft.
            let before = self.arms.len();
            let old: Option<String> = self.arms.iter().filter(|a| a.holder.is_some() && Self::metal_in(&a.kind).is_none()).min_by(|a, b| a.force.total_cmp(&b.force)).map(|a| a.kind.clone());
            self.finish_arm(i);
            if self.arms.len() > before {
                if let Some(k) = (0..self.arms.len()).filter(|&k| self.arms[k].holder.is_none()).min_by(|&a, &b| self.arms[a].force.total_cmp(&self.arms[b].force)) {
                    self.arms.remove(k);
                    self.arm_militia();
                }
                let new = self.arms.iter().rev().find(|a| a.maker == i && a.day == day).map(|a| a.kind.clone()).unwrap_or_default();
                if let Some(o) = old { self.note(format!("{} forges {} at the forge; {} is put by.", name, new, o)); }
            }
        } else if why.starts_with("Forging mail") {
            let before = self.armour.len();
            self.finish_armour(i);
            if self.armour.len() > before {
                if let Some(k) = (0..self.armour.len()).filter(|&k| self.armour[k].holder.is_none()).min_by(|&a, &b| self.armour[a].cover.total_cmp(&self.armour[b].cover)) {
                    let old = self.armour.remove(k);
                    self.armour_up();
                    let new = self.armour.last().map(|a| a.kind.clone()).unwrap_or_default();
                    self.note(format!("{} forges {} at the forge; {} is put by.", name, new, old.kind));
                }
            }
        } else if why.starts_with("Firing") {
            let clay = self.industry.clay >= 2;
            if !clay && self.industry.sand < 2 { return; }
            if !self.magma_forge && !self.take_stored(ItemKind::Log) { return; }
            if clay { self.industry.clay -= 2; } else { self.industry.sand -= 2; }
            let h = crate::history::settlers::hash_pub(self.seed ^ self.clock.tick ^ (i as u64) << 20, 0x4117);
            let (material, kind) = if clay { ("fired clay", ["jar", "jug", "bowl", "vase"][(h % 4) as usize]) } else { ("green glass", ["bottle", "cup", "string of beads", "vase"][(h % 4) as usize]) };
            let quality = {
                let s = &self.settlers[i];
                let p = &s.persona;
                let q = 0.35 * s.skill[4] + 0.2 * (p.attr(Attr::KinestheticSense) / 2000.0).min(1.0) + 0.15 * (p.attr(Attr::Creativity) / 2000.0).min(1.0)
                    + 0.15 * p.facet(Facet::Perfectionism) as f32 / 100.0 + 0.15 * ((h >> 8) % 1000) as f32 / 1000.0;
                (((q - 0.35) * 9.0).floor() as i32).clamp(0, 5) as u8
            };
            let work = craft::Work { maker: i, kind: kind.to_string(), material: material.to_string(), quality, image: None, day, called: None, traded: false };
            let what = work.describe();
            self.settlers[i].made.push(format!("{} (day {})", what, day));
            self.settlers[i].mind.made += 1;
            self.works.push(work);
            self.industry.fired += 1;
            self.feel(i, mind::Feel::Made { what: what.clone(), quality });
            self.tell_once(format!("fired {}", material), format!("{} draws {} from the kiln, the first {} made in the camp.", name, what, material));
        }
    }

    /// A spear or mail of metal from `finish_arm` / `finish_armour`: the first of each metal is
    /// told, with the smith's hand in it.
    pub(crate) fn forged_arm(&mut self, i: usize, kind: &str) {
        let Some(m) = Self::metal_in(kind) else { return };
        self.industry.forged += 1;
        self.train_smith(i);
        let name = self.settlers[i].name.clone();
        let at = if self.shop(RoomKind::Forge).is_some() { "the forge" } else { "the magma forge" };
        self.tell_once(format!("arm {}", m), format!("{} forges {} at {}: the first of the camp's arms of {}.", name, kind, at, m));
    }

    /// The quality word for a forged head, put before its kind ("a superior iron-headed spear";
    /// ordinary and well-crafted pieces go unnamed).
    pub(crate) fn forged_kind(&self, i: usize, kind: &str) -> String {
        let q = self.forged_quality(i, 0xF06E);
        if q < 2 { return kind.to_string(); }
        let rest = kind.strip_prefix("an ").or_else(|| kind.strip_prefix("a ")).unwrap_or(kind);
        format!("a {}{}", craft::QUALITY[q as usize], rest)
    }

    /// What the caravan buys of the stock: blocks past those kept for furniture, bars past
    /// those kept for arms (never adamantine), barrels past two. (Value, what was sold.)
    pub(crate) fn sell_stock(&mut self) -> (u32, Vec<String>) {
        let mut value = 0;
        let mut sold = Vec::new();
        if self.industry.blocks > BLOCKS_KEPT {
            let n = self.industry.blocks - BLOCKS_KEPT;
            self.industry.blocks = BLOCKS_KEPT;
            value += n;
            sold.push(format!("{} blocks of {}", n, self.land_stone()));
        }
        for k in 0..self.industry.bars.len() {
            let (m, n) = self.industry.bars[k].clone();
            if m == "adamantine" || n <= BARS_KEPT { continue; }
            let each = match m.as_str() { "gold" => 10, "silver" => 6, _ => 4 };
            self.industry.bars[k].1 = BARS_KEPT;
            value += (n - BARS_KEPT) * each;
            sold.push(format!("{} bars of {}", n - BARS_KEPT, m));
        }
        if self.industry.barrels > BARRELS_KEPT {
            let n = self.industry.barrels - BARRELS_KEPT;
            self.industry.barrels = BARRELS_KEPT;
            value += 2 * n;
            sold.push(format!("{} barrel{}", n, if n == 1 { "" } else { "s" }));
        }
        (value, sold)
    }

    /// Workshops to cut below, for `delve_candidates`: a smelter when ore lies waiting, a forge
    /// when the smelter stands (none needed over the magma), a mason's once much stone has come
    /// up, a carpenter's when the still or the rooms want wood worked, a kiln for clay and sand.
    pub(crate) fn industry_candidates(&self, c: &mut Vec<(f32, ProjectKind, String, u32, Pos)>) {
        let Some(sp) = self.spine else { return };
        let day = self.clock.day();
        let planned = |k: ProjectKind| self.projects.iter().any(|p| p.kind == k);
        let workshop = self.projects.iter().any(|p| p.done && p.kind == ProjectKind::Workshop);
        let masons = self.way.as_ref().map_or(false, |w| w.stone_first);
        let push =|c: &mut Vec<(f32, ProjectKind, String, u32, Pos)>, u: f32, k: ProjectKind, why: String| {
            if let Some(plan) = self.plan_dig(k) { c.push((u, k, why, plan.cuts.len() as u32, sp.at)); }
        };
        let ore = self.ore_total();
        if ore > 0 && !planned(ProjectKind::Smelter) {
            let kind = self.industry.ore.iter().find(|x| x.1 > 0).map(|x| x.0.clone()).unwrap_or_default();
            let fuel = if self.magma_forge { "the magma's heat would melt it" } else { "charcoal burnt from logs would melt it" };
            push(c, 1.6, ProjectKind::Smelter, format!("{} loads of {} ore lie by the stair, and ore is only rock until it is smelted; in a furnace below, {}", ore, kind, fuel));
        }
        if self.shop(RoomKind::Smelter).is_some() && !self.magma_forge && !planned(ProjectKind::Forge) {
            let bars: u32 = self.industry.bars.iter().map(|x| x.1).sum();
            push(c, 1.5, ProjectKind::Forge, format!("the smelter gives bars ({} so far), and only a forge turns them into tools and spearheads", bars));
        }
        if workshop && day >= 45 && !planned(ProjectKind::MasonShop) && (self.stone_dug >= 40 || masons) {
            push(c, if masons { 0.9 } else { 0.55 }, ProjectKind::MasonShop, format!("{} loads of stone have come up the stair and lie rough; a mason's bench would dress them into blocks for tables and the traders", self.stone_dug));
        }
        let unfurnished = self.rooms.iter().filter(|r| matches!(r.kind, RoomKind::Bedroom | RoomKind::GreatHall) && r.furnished.is_none()).count();
        if workshop && day >= 45 && !planned(ProjectKind::CarpenterShop) && self.felled.len() >= 40 && (self.still().is_some() || unfurnished >= 2) {
            let want = if self.still().is_some() { "the still wants barrels".to_string() } else { format!("{} rooms below want beds and tables", unfurnished) };
            push(c, 0.5, ProjectKind::CarpenterShop, format!("{} trees have been felled and the logs are worked with axes in the open; {}, and a carpenter's bench would make them", self.felled.len(), want));
        }
        let earth = self.industry.clay + self.industry.sand;
        if workshop && day >= 60 && !planned(ProjectKind::Kiln) && earth >= 6 {
            let what = if self.industry.clay >= self.industry.sand { "clay" } else { "sand" };
            push(c, 0.45, ProjectKind::Kiln, format!("{} loads of {} have come out of the ground; a kiln would fire it into {}", earth, what, if what == "clay" { "pots and jars" } else { "glass" }));
        }
    }
}
