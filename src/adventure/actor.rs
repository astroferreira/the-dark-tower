//! Monsters and people in a place. A monster is a definition (`data::MonsterDef`) given a body on a
//! floor; a boss carries its own name and, for a beast of the world's history, its generated
//! monster (body, special attack, description) and hoard. People (`Npc`) live in towns.

use super::data::{data, MonsterDef};
use super::item::Item;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Monster {
    pub uid: u32,
    pub def: String,
    /// Its name ("cave rat", "Bornith the Plague-Bearer").
    pub name: String,
    pub hp: i32,
    pub max_hp: i32,
    pub x: i32,
    pub y: i32,
    pub z: usize,
    /// Time banked toward its next act (speed added each tick; acts at 100).
    pub energy: i32,
    pub awake: bool,
    /// A named boss: harder, never flees, guards its hoard.
    pub boss: bool,
    /// Multipliers over its definition (a boss's size).
    pub scale: f32,
    /// A beast of the history: its body and special attack.
    pub legend: Option<crate::monsters::Monster>,
    /// What it carries besides its loot table (a hoard, a key).
    pub carries: Vec<Item>,
    pub poisoned: i32,
    pub slowed: i32,
    /// Where it was set (it goes back there when it loses the scent).
    pub home: (i32, i32),
    /// Facing left (sprites turn the way they walk).
    pub left: bool,
    /// The last time (game turn) it struck, for the strike pose.
    pub struck_at: u64,
    /// Out by night only: gone at dawn (the risen of a battlefield, a werewolf).
    #[serde(default)]
    pub night: bool,
    /// The town it belongs to (one of its people or its watch turned on the hero; 0 none).
    #[serde(default)]
    pub town: u32,
    /// Turns its arm is maimed (its blows weaker), and turns it is afraid (it flees).
    #[serde(default)]
    pub maimed: i32,
    #[serde(default)]
    pub fear: i32,
    /// A boss that has called its own to help (once).
    #[serde(default)]
    pub called: bool,
}

impl Monster {
    pub fn new(uid: u32, def: &str, x: i32, y: i32, z: usize) -> Monster {
        let d = data().monster(def).unwrap_or_else(|| data().monster("rat").unwrap());
        Monster { uid, def: d.id.clone(), name: d.name.clone(), hp: d.hp, max_hp: d.hp, x, y, z, energy: 0, awake: false, boss: false, scale: 1.0,
            legend: None, carries: Vec::new(), poisoned: 0, slowed: 0, home: (x, y), left: uid % 2 == 0, struck_at: 0, night: false, town: 0, maimed: 0, fear: 0, called: false }
    }
    /// A named boss over `def`, `scale` times as strong.
    pub fn boss(uid: u32, def: &str, name: &str, scale: f32, x: i32, y: i32, z: usize) -> Monster {
        let mut m = Monster::new(uid, def, x, y, z);
        m.name = name.to_string();
        m.boss = true;
        m.scale = scale;
        m.max_hp = (m.max_hp as f32 * scale * scale).round() as i32;
        m.hp = m.max_hp;
        m
    }
    pub fn def(&self) -> &'static MonsterDef { data().monster(&self.def).unwrap() }
    pub fn attack(&self) -> i32 { (self.def().attack as f32 * self.scale * if self.maimed > 0 { 0.6 } else { 1.0 }).round() as i32 }
    pub fn defense(&self) -> i32 { (self.def().defense as f32 * self.scale.sqrt()).round() as i32 }
    pub fn armor(&self) -> i32 { (self.def().armor as f32 * self.scale.sqrt()).round() as i32 }
    pub fn speed(&self) -> i32 { let s = self.def().speed; if self.slowed > 0 { s * 2 / 3 } else { s } }
    pub fn xp(&self) -> u32 { (self.def().xp as f32 * self.scale * self.scale).round() as u32 * if self.boss { 2 } else { 1 } }
    /// "the cave rat", "Bornith the Plague-Bearer".
    pub fn the(&self) -> String { if self.boss { self.name.clone() } else { format!("the {}", self.name) } }
    pub fn a(&self) -> String { if self.boss { self.name.clone() } else { super::item::article(&self.name) } }
}

/// What a townsperson does: it decides what they say and trade.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Role { Priest, Smith, Trader, Innkeeper, Lord, Guard, Sage, Townsfolk }

impl Role {
    pub fn word(self) -> &'static str {
        match self { Role::Priest => "priest", Role::Smith => "smith", Role::Trader => "trader", Role::Innkeeper => "innkeeper", Role::Lord => "lord", Role::Guard => "captain of the guard", Role::Sage => "sage", Role::Townsfolk => "townsman" }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Npc {
    pub name: String,
    pub role: Role,
    pub x: i32,
    pub y: i32,
    pub z: usize,
    /// Where they stand when not wandering.
    pub post: (i32, i32),
    /// Their race (for their figure and their way of talking).
    pub race: String,
    pub female: bool,
    /// Which god (a priest's), which people (a lord's).
    pub of: String,
    /// The town they belong to (0: the land's own, a hermit or a farmer).
    #[serde(default)]
    pub home: u32,
    /// What they remember of the adventurer.
    #[serde(default)]
    pub met: super::people::Met,
}
