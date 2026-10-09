//! The adventurer: one person of the world (DF's adventurer is a figure like any other; here they
//! are rolled from their people's template, `persona.rs`), grown the way Tibia grows its heroes:
//! experience from slain things raises the level (life and mana with it), every blow, shot, block
//! and spell trains its skill, and at level 8 the temple gives them a calling.

use super::data::data;
use super::item::{stow, Item};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Skill { Fist, Sword, Axe, Club, Distance, Shielding, Magic }

impl Skill {
    pub const ALL: [Skill; 7] = [Skill::Fist, Skill::Sword, Skill::Axe, Skill::Club, Skill::Distance, Skill::Shielding, Skill::Magic];
    pub fn word(self) -> &'static str {
        match self { Skill::Fist => "fist fighting", Skill::Sword => "sword fighting", Skill::Axe => "axe fighting", Skill::Club => "club fighting", Skill::Distance => "distance fighting", Skill::Shielding => "shielding", Skill::Magic => "magic level" }
    }
    pub fn of_weapon(s: Option<&str>) -> Skill {
        match s { Some("sword") => Skill::Sword, Some("axe") => Skill::Axe, Some("club") => Skill::Club, Some("distance") => Skill::Distance, _ => Skill::Fist }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Slot { Head, Neck, Body, Hand, Shield, Legs, Feet, Ring }

impl Slot {
    pub const ALL: [Slot; 8] = [Slot::Head, Slot::Neck, Slot::Body, Slot::Hand, Slot::Shield, Slot::Legs, Slot::Feet, Slot::Ring];
    pub fn of(s: &str) -> Option<Slot> {
        Some(match s { "head" => Slot::Head, "neck" => Slot::Neck, "body" => Slot::Body, "hand" => Slot::Hand, "shield" => Slot::Shield, "legs" => Slot::Legs, "feet" => Slot::Feet, "ring" => Slot::Ring, _ => return None })
    }
    pub fn word(self) -> &'static str {
        match self { Slot::Head => "head", Slot::Neck => "neck", Slot::Body => "body", Slot::Hand => "weapon", Slot::Shield => "shield", Slot::Legs => "legs", Slot::Feet => "feet", Slot::Ring => "ring" }
    }
}

/// Experience to reach `level` (Tibia's cubic curve, at a third of its scale).
pub fn xp_for(level: u32) -> u64 {
    let l = level as i64;
    if l <= 1 { return 0; }
    ((50 * (l * l * l - 6 * l * l + 17 * l - 12)) / 9).max(0) as u64
}

/// Tries to raise a skill from `level` (Tibia: 50 x 1.1^(s-10) x the calling's constant).
fn tries_for(level: u32, mult: f32, magic: bool) -> u32 {
    if magic { (400.0 * mult.powf(level as f32)).min(1.0e9) as u32 }
    else { (50.0 * mult * 1.1f32.powf(level as f32 - 10.0)).max(8.0) as u32 }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Hero {
    pub name: String,
    pub race: String,
    pub female: bool,
    pub calling: Option<String>,
    pub level: u32,
    pub xp: u64,
    pub hp: i32,
    pub mana: i32,
    /// Skill levels and their tries toward the next.
    pub skills: [(u32, u32); 7],
    pub equipped: [Option<Item>; 8],
    pub pack: Vec<Item>,
    pub spells: Vec<String>,
    /// Turns of food left (regeneration while fed).
    pub fed: i32,
    /// A lit torch: turns of light left.
    pub torch: i32,
    /// A light spell's turns.
    pub glow: i32,
    pub poisoned: i32,
    pub hasted: i32,
    /// The temple where they wake after death (a town's site id).
    pub temple: u32,
    pub deaths: u32,
    pub kills: u32,
    /// A temple's blessing: the next death costs nothing.
    pub blessed: bool,
    /// The persona's look (skin, hair) for the figure.
    pub skin: [f32; 3],
    pub hair: [f32; 3],
    pub beard: bool,
    pub strength: u32,
}

impl Hero {
    pub fn new(name: &str, race: &str, female: bool, seed: u64) -> Hero {
        let p = crate::persona::Persona::roll(race, None, seed);
        let mut h = Hero {
            name: name.into(), race: race.into(), female, calling: None, level: 1, xp: 0, hp: 0, mana: 0,
            skills: [(10, 0); 7], equipped: Default::default(), pack: Vec::new(), spells: Vec::new(), fed: 300, torch: 0, glow: 0,
            poisoned: 0, hasted: 0, temple: 0, deaths: 0, kills: 0, blessed: false,
            skin: crate::persona::colour(&p.skin).unwrap_or([214.0, 172.0, 140.0]), hair: crate::persona::colour(&p.hair).unwrap_or([90.0, 60.0, 40.0]),
            beard: p.beard && !female, strength: p.attr(crate::persona::Attr::Strength) as u32,
        };
        h.skills[Skill::Magic as usize] = (0, 0);
        h.hp = h.max_hp();
        h.mana = h.max_mana();
        // What a commoner sets out with.
        h.equipped[Slot::Hand as usize] = Some(Item::new("club", 1));
        h.equipped[Slot::Body as usize] = Some(Item::new("rags", 1));
        for it in [Item::new("gold", 20), Item::new("bread", 3), Item::new("torch", 2), Item::new("health_potion", 1)] { stow(&mut h.pack, it); }
        h
    }

    fn calling_def(&self) -> Option<&'static super::data::CallingDef> { self.calling.as_deref().and_then(|c| data().calling(c)) }
    pub fn max_hp(&self) -> i32 {
        let per = self.calling_def().map_or(8, |c| c.hp / 2 + 2);
        let ring: i32 = self.equipped.iter().flatten().map(|i| i.def().hp).sum();
        60 + per * (self.level as i32 - 1) + ring
    }
    pub fn max_mana(&self) -> i32 { let per = self.calling_def().map_or(4, |c| c.mana / 2 + 1); 20 + per * (self.level as i32 - 1) }
    pub fn skill(&self, s: Skill) -> u32 {
        let bonus: i32 = match s { Skill::Magic => self.equipped.iter().flatten().map(|i| i.def().magic).sum(), Skill::Sword | Skill::Axe | Skill::Club | Skill::Fist => self.equipped.iter().flatten().map(|i| i.def().melee).sum(), _ => 0 };
        (self.skills[s as usize].0 as i32 + bonus).max(0) as u32
    }
    pub fn weapon(&self) -> Option<&Item> { self.equipped[Slot::Hand as usize].as_ref() }
    pub fn weapon_skill(&self) -> Skill { match self.weapon() { Some(w) if w.def().kind == "weapon" => Skill::of_weapon(w.def().skill.as_deref()), Some(w) if w.def().kind == "wand" => Skill::Magic, _ => Skill::Fist } }
    pub fn armor(&self) -> i32 { self.equipped.iter().flatten().map(|i| i.armor()).sum() }
    pub fn defense(&self) -> i32 {
        let shield = self.equipped[Slot::Shield as usize].as_ref().map_or(0, |s| s.defense());
        let weapon = self.weapon().map_or(0, |w| w.defense());
        shield.max(weapon / 2) + self.skill(Skill::Shielding) as i32 / 3
    }
    pub fn light(&self) -> i32 {
        let t = if self.torch > 0 { data().item("torch").map_or(5, |d| d.light) } else { 0 };
        let g = if self.glow > 0 { 6 } else { 0 };
        (t.max(g)).max(1)
    }
    /// Turns a step takes (lower is faster), and a strike.
    pub fn step_time(&self) -> i32 { let base = 100 - (self.level as i32 / 2).min(30); if self.hasted > 0 { base * 7 / 10 } else { base } }
    pub fn gold(&self) -> u32 { self.pack.iter().filter(|i| i.id == "gold").map(|i| i.count).sum() }
    pub fn take_gold(&mut self, n: u32) -> bool {
        if self.gold() < n { return false; }
        let mut left = n;
        for it in self.pack.iter_mut().filter(|i| i.id == "gold") { let t = it.count.min(left); it.count -= t; left -= t; }
        self.pack.retain(|i| i.count > 0);
        true
    }
    pub fn count(&self, id: &str) -> u32 { self.pack.iter().filter(|i| i.id == id).map(|i| i.count).sum() }
    /// Take `n` of `id` from the pack.
    pub fn spend(&mut self, id: &str, n: u32) -> bool {
        if self.count(id) < n { return false; }
        let mut left = n;
        for it in self.pack.iter_mut().filter(|i| i.id == id) { let t = it.count.min(left); it.count -= t; left -= t; }
        self.pack.retain(|i| i.count > 0);
        true
    }

    /// Experience gained; returns the levels reached.
    pub fn gain_xp(&mut self, xp: u64) -> Vec<u32> {
        self.xp += xp;
        let mut up = Vec::new();
        while self.xp >= xp_for(self.level + 1) {
            self.level += 1;
            up.push(self.level);
            self.hp = self.max_hp();
            self.mana = self.max_mana();
        }
        up
    }
    /// One try at a skill (a blow landed or missed, a shot, a block, mana spent); the new level
    /// when it rises.
    pub fn train(&mut self, s: Skill, tries: u32) -> Option<u32> {
        let c = self.calling_def();
        let mult = match s { Skill::Magic => c.map_or(3.0, |c| c.magic), Skill::Distance => c.map_or(1.5, |c| c.distance), Skill::Shielding => c.map_or(1.5, |c| c.shielding), _ => c.map_or(1.5, |c| c.melee) };
        let magic = s == Skill::Magic;
        let e = &mut self.skills[s as usize];
        e.1 += tries;
        let need = tries_for(e.0, mult, magic);
        if e.1 >= need { e.1 -= need; e.0 += 1; return Some(e.0); }
        None
    }
    /// Share of the way to the next skill level (for the skill bars).
    pub fn skill_progress(&self, s: Skill) -> f32 {
        let c = self.calling_def();
        let mult = match s { Skill::Magic => c.map_or(3.0, |c| c.magic), Skill::Distance => c.map_or(1.5, |c| c.distance), Skill::Shielding => c.map_or(1.5, |c| c.shielding), _ => c.map_or(1.5, |c| c.melee) };
        let e = self.skills[s as usize];
        e.1 as f32 / tries_for(e.0, mult, s == Skill::Magic).max(1) as f32
    }
    pub fn level_progress(&self) -> f32 {
        let (a, b) = (xp_for(self.level), xp_for(self.level + 1));
        ((self.xp - a) as f32 / (b - a).max(1) as f32).clamp(0.0, 1.0)
    }

    /// Put on `it` from the pack (by index); what was worn goes back in the pack.
    pub fn equip(&mut self, k: usize) -> Result<String, String> {
        let it = self.pack.get(k).cloned().ok_or("nothing there")?;
        let d = it.def();
        let slot = match d.kind.as_str() { "ammo" => return Err("Arrows and bolts are drawn from the pack as the bow wants them.".into()), _ => d.slot.as_deref().and_then(Slot::of).ok_or_else(|| format!("You cannot wear {}.", it.describe()))? };
        if !d.calling.is_empty() && !self.calling.as_ref().map_or(false, |c| d.calling.contains(c)) { return Err(format!("Only a {} can use {}.", d.calling.join(" or a "), it.describe())); }
        let mut one = it.clone();
        one.count = 1;
        if d.thrown { one.count = it.count; }
        if one.count >= it.count { self.pack.remove(k); } else { self.pack[k].count -= one.count; }
        if d.two_handed { if let Some(s) = self.equipped[Slot::Shield as usize].take() { stow(&mut self.pack, s); } }
        if slot == Slot::Shield { if self.weapon().map_or(false, |w| w.def().two_handed) { if let Some(w) = self.equipped[Slot::Hand as usize].take() { stow(&mut self.pack, w); } } }
        let line = format!("You {} {}.", if slot == Slot::Hand { "take up" } else { "put on" }, one.describe());
        if let Some(old) = self.equipped[slot as usize].replace(one) { stow(&mut self.pack, old); }
        self.hp = self.hp.min(self.max_hp());
        Ok(line)
    }
    pub fn unequip(&mut self, s: Slot) -> Option<String> {
        let it = self.equipped[s as usize].take()?;
        let line = format!("You take off {}.", it.describe());
        stow(&mut self.pack, it);
        Some(line)
    }

    /// Spells this hero may learn (by calling and level), and those they know.
    pub fn may_learn(&self) -> Vec<&'static super::data::SpellDef> {
        data().spells.iter().filter(|s| !self.spells.contains(&s.id) && (s.callings.is_empty() || self.calling.as_ref().map_or(false, |c| s.callings.contains(c)))).collect()
    }
    /// What the blessing costs at this level.
    pub fn blessing_price(&self) -> u32 { (100 * self.level).clamp(100, 4000) }
    /// What the smith asks to work `it` one step finer.
    pub fn refine_price(it: &Item) -> u32 { let q = it.quality as u32 + 1; (it.def().value.max(10) * q * 2 + 60 * q * q).min(60_000) }

    pub fn title(&self) -> String {
        format!("{}, {} {}{}", self.name, if self.level >= 20 { "the renowned" } else if self.level >= 8 { "the" } else { "a" },
            self.calling.as_deref().unwrap_or(if self.level >= 8 { "wanderer" } else { "commoner" }), if self.level >= 8 && self.calling.is_none() { " without a calling" } else { "" })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn levels_and_skills_grow() {
        assert_eq!(xp_for(1), 0);
        assert!(xp_for(2) > 0 && xp_for(3) > xp_for(2) && xp_for(8) > 1000);
        let mut h = Hero::new("Ash", "human", false, 1);
        assert_eq!(h.level, 1);
        let up = h.gain_xp(xp_for(5));
        assert_eq!(up, vec![2, 3, 4, 5]);
        assert!(h.max_hp() > 60);
        let mut rose = 0;
        for _ in 0..2000 { if h.train(Skill::Club, 1).is_some() { rose += 1; } }
        assert!(rose >= 3, "club rose {} times", rose);
        let k = h.pack.iter().position(|i| i.id == "torch").unwrap();
        assert!(h.equip(k).is_err());
    }
}
