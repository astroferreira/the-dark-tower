//! The adventure's content: monsters, items, materials, callings and spells
//! (`data/defaults/adventure.json`, loaded once). DF keeps content as data and the code as rules;
//! the numbers are Tibia's shape (hit points, attack, defense, armour, experience) at a smaller
//! scale.

use serde::Deserialize;
use std::sync::OnceLock;

const JSON: &str = include_str!("../../data/defaults/adventure.json");

#[derive(Clone, Debug, Deserialize)]
pub struct MonsterDef {
    pub id: String,
    pub name: String,
    pub hp: i32,
    pub attack: i32,
    pub defense: i32,
    pub armor: i32,
    pub speed: i32,
    pub xp: u32,
    /// A beast's name for `tiles::beasts::of_name`, or "folk:RACE:HELM:ARM" for a person.
    pub look: String,
    pub ai: String,
    pub tier: u32,
    #[serde(default)]
    pub habitats: Vec<String>,
    /// (item, chance in 1000, min, max)
    #[serde(default)]
    pub loot: Vec<(String, u32, u32, u32)>,
    #[serde(default)]
    pub sounds: Vec<String>,
    /// (range in cells, damage, kind: arrow, spear, bolt, fire, poison, dark...)
    #[serde(default)]
    pub range: Option<(i32, i32, String)>,
    #[serde(default)]
    pub poison: i32,
    #[serde(default)]
    pub heals: i32,
    #[serde(default)]
    pub regen: i32,
    #[serde(default)]
    pub pack: u32,
    #[serde(default)]
    pub undead: bool,
    #[serde(default)]
    pub ghost: bool,
    #[serde(default)]
    pub lifesteal: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ItemDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub plural: Option<String>,
    pub kind: String,
    #[serde(default)]
    pub slot: Option<String>,
    #[serde(default)]
    pub skill: Option<String>,
    #[serde(default)]
    pub attack: i32,
    #[serde(default)]
    pub defense: i32,
    #[serde(default)]
    pub armor: i32,
    #[serde(default)]
    pub value: u32,
    pub glyph: String,
    #[serde(default)]
    pub material: Option<String>,
    #[serde(default)]
    pub stack: bool,
    #[serde(default)]
    pub two_handed: bool,
    #[serde(default)]
    pub range: i32,
    #[serde(default)]
    pub thrown: bool,
    #[serde(default)]
    pub ammo: Option<String>,
    #[serde(default)]
    pub heal: i32,
    #[serde(default)]
    pub mana: i32,
    #[serde(default)]
    pub food: i32,
    #[serde(default)]
    pub light: i32,
    #[serde(default)]
    pub burn: i32,
    #[serde(default)]
    pub poison: i32,
    #[serde(default)]
    pub element: Option<String>,
    #[serde(default)]
    pub calling: Vec<String>,
    #[serde(default)]
    pub magic: i32,
    #[serde(default)]
    pub melee: i32,
    #[serde(default)]
    pub hp: i32,
    #[serde(default)]
    pub resist: Option<String>,
    #[serde(default)]
    pub burns: bool,
    /// A rune's spell (cast once, by anyone, without mana).
    #[serde(default)]
    pub spell: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MaterialDef {
    pub name: String,
    pub att: f32,
    pub arm: f32,
    pub colour: [u8; 3],
    #[serde(default)]
    pub holy: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CallingDef {
    pub id: String,
    pub name: String,
    pub hp: i32,
    pub mana: i32,
    /// Skill-learning multipliers (lower learns faster), Tibia's vocation constants.
    pub melee: f32,
    pub distance: f32,
    pub magic: f32,
    pub shielding: f32,
    pub desc: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SpellDef {
    pub id: String,
    pub name: String,
    pub words: String,
    pub level: u32,
    pub mana: i32,
    pub kind: String,
    pub power: f32,
    #[serde(default)]
    pub range: i32,
    #[serde(default)]
    pub element: Option<String>,
    /// Empty: every calling (and those still without one).
    #[serde(default)]
    pub callings: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct Data {
    pub monsters: Vec<MonsterDef>,
    pub items: Vec<ItemDef>,
    pub materials: Vec<MaterialDef>,
    pub callings: Vec<CallingDef>,
    pub spells: Vec<SpellDef>,
}

impl Data {
    pub fn monster(&self, id: &str) -> Option<&MonsterDef> { self.monsters.iter().find(|m| m.id == id) }
    pub fn item(&self, id: &str) -> Option<&ItemDef> { self.items.iter().find(|m| m.id == id) }
    pub fn material(&self, name: &str) -> Option<&MaterialDef> { self.materials.iter().find(|m| m.name == name) }
    pub fn calling(&self, id: &str) -> Option<&CallingDef> { self.callings.iter().find(|m| m.id == id) }
    pub fn spell(&self, id: &str) -> Option<&SpellDef> { self.spells.iter().find(|m| m.id == id) }
    /// Monsters that live in `habitat`, at most `tier`.
    pub fn living_in(&self, habitat: &str, tier: u32) -> Vec<&MonsterDef> {
        self.monsters.iter().filter(|m| m.tier <= tier && m.habitats.iter().any(|h| h == habitat)).collect()
    }
}

pub fn data() -> &'static Data {
    static D: OnceLock<Data> = OnceLock::new();
    D.get_or_init(|| serde_json::from_str(JSON).expect("adventure.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn content_is_consistent() {
        let d = data();
        assert!(d.monsters.len() >= 30);
        for m in &d.monsters {
            for (id, chance, lo, hi) in &m.loot {
                assert!(d.item(id).is_some(), "{} drops unknown item {}", m.id, id);
                assert!(*chance <= 1000 && lo <= hi, "{} loot {}", m.id, id);
            }
            assert!(!m.habitats.is_empty(), "{} lives nowhere", m.id);
        }
        for i in &d.items {
            if let Some(mat) = &i.material { assert!(d.material(mat).is_some(), "{} of unknown {}", i.id, mat); }
            if let Some(a) = &i.ammo { assert!(d.item(a).is_some()); }
        }
        for s in &d.spells { for c in &s.callings { assert!(d.calling(c).is_some(), "{} for unknown {}", s.id, c); } }
        for i in d.items.iter().filter(|i| i.kind == "rune") { assert!(i.spell.as_deref().and_then(|s| d.spell(s)).is_some(), "rune {} of no spell", i.id); }
    }
}
