//! Things: a definition (`data::ItemDef`, its type and subtype) made of a material at a quality,
//! as DF builds items from type x subtype x material. Weapons and armour take their numbers from
//! all three; an artifact of the world's history is an item with its own name and story.

use super::data::{data, ItemDef};

/// DF's quality levels: ordinary, well-crafted, finely-crafted, superior, exceptional,
/// masterwork; 6 is an artifact.
pub const QUALITY: [(&str, &str, f32); 7] = [
    ("", "", 1.0), ("-", "well-crafted", 1.08), ("+", "finely-crafted", 1.15), ("*", "superior", 1.22),
    ("≡", "exceptional", 1.32), ("☼", "masterwork", 1.45), ("!", "legendary", 1.7),
];

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Item {
    pub id: String,
    pub count: u32,
    pub material: Option<String>,
    pub quality: u8,
    /// A name of its own (an artifact of the history, a named key).
    pub name: Option<String>,
    /// Its story (an artifact's description; what a key opens).
    pub story: Option<String>,
    /// A key's lock, a quest item's quest.
    pub tag: u32,
}

impl Item {
    pub fn new(id: &str, count: u32) -> Item {
        let mat = data().item(id).and_then(|d| d.material.clone());
        Item { id: id.to_string(), count: count.max(1), material: mat, quality: 0, name: None, story: None, tag: 0 }
    }
    pub fn of(id: &str, material: &str, quality: u8) -> Item {
        let mut i = Item::new(id, 1);
        if data().material(material).is_some() { i.material = Some(material.to_string()); }
        i.quality = quality.min(6);
        i
    }
    pub fn key(lock: u32, what: &str) -> Item {
        let mut i = Item::new("key", 1);
        i.tag = lock;
        i.name = Some(format!("key to {}", what));
        i
    }
    pub fn def(&self) -> &'static ItemDef { data().item(&self.id).unwrap_or_else(|| data().item("bone").unwrap()) }
    pub fn stacks(&self) -> bool { self.def().stack && self.name.is_none() && self.quality == 0 }
    pub fn same_stack(&self, o: &Item) -> bool { self.stacks() && o.stacks() && self.id == o.id && self.material == o.material }
    pub fn is_artifact(&self) -> bool { self.quality >= 6 }
    fn mat_att(&self) -> f32 { self.material.as_deref().and_then(|m| data().material(m)).map_or(1.0, |m| m.att) }
    fn mat_arm(&self) -> f32 { self.material.as_deref().and_then(|m| data().material(m)).map_or(1.0, |m| m.arm) }
    fn q(&self) -> f32 { QUALITY[self.quality.min(6) as usize].2 }
    /// The material is the item's own (not the subtype's usual one): it is named.
    fn odd_material(&self) -> bool { self.material.is_some() && self.material != self.def().material }

    /// Attack value (weapons, ammunition, wands).
    pub fn attack(&self) -> i32 { (self.def().attack as f32 * self.mat_att() * self.q()).round() as i32 }
    pub fn defense(&self) -> i32 { (self.def().defense as f32 * self.mat_arm() * self.q()).round() as i32 }
    pub fn armor(&self) -> i32 { let a = self.def().armor; if a == 0 { 0 } else { ((a as f32 * self.mat_arm() * self.q()).round() as i32).max(1) } }
    /// What a trader pays (a third of its worth; an artifact's worth is its fame).
    pub fn value(&self) -> u32 {
        let v = self.def().value as f32 * self.q().powi(3) * self.mat_att().max(0.5).powi(2);
        (v.round() as u32).max(if self.def().value > 0 { 1 } else { 0 }) * self.count
    }

    /// "a finely-crafted iron sword", "12 gold coins", "Gutterblade, a steel sword".
    pub fn describe(&self) -> String {
        let d = self.def();
        if self.count > 1 {
            let pl = d.plural.clone().unwrap_or_else(|| plural(&d.name));
            return format!("{} {}", self.count, pl);
        }
        let q = QUALITY[self.quality.min(6) as usize].1;
        let mat = if self.odd_material() { format!("{} ", self.material.as_deref().unwrap_or("")) } else { String::new() };
        let base = format!("{}{}{}", if q.is_empty() || self.is_artifact() { String::new() } else { format!("{} ", q) }, mat, d.name);
        match &self.name {
            Some(n) if d.kind == "key" => n.clone(),
            Some(n) => format!("{}, {}", n, article(&base)),
            None => article(&base),
        }
    }
    /// The name shown in a list: the item's own name, else what it is.
    pub fn short(&self) -> String {
        match &self.name { Some(n) => n.clone(), None => {
            let s = self.describe();
            s.strip_prefix("a ").or_else(|| s.strip_prefix("an ")).unwrap_or(&s).to_string()
        } }
    }
    /// What it does, for the look panel: "attack 14, defense 12", "armour 4", "heals 40".
    pub fn stats(&self) -> String {
        let d = self.def();
        let mut v = Vec::new();
        match d.kind.as_str() {
            "weapon" => {
                if d.ammo.is_some() { v.push(format!("range {}, shoots {}s", d.range, d.ammo.as_deref().unwrap_or(""))); }
                else { v.push(format!("attack {}", self.attack())); if self.defense() > 0 { v.push(format!("defense {}", self.defense())); } }
                if d.thrown { v.push(format!("thrown {} cells", d.range)); }
                if d.two_handed { v.push("two-handed".into()); }
                if let Some(s) = &d.skill { v.push(format!("{} fighting", s)); }
            }
            "ammo" => v.push(format!("attack {}", self.attack())),
            "wand" => v.push(format!("{} {}-{}, {} mana a shot, for {}", d.element.as_deref().unwrap_or(""), self.attack() * 2 / 3, self.attack(), d.mana, d.calling.join(" and "))),
            "shield" => v.push(format!("defense {}", self.defense())),
            "armour" | "jewel" => { if self.armor() > 0 { v.push(format!("armour {}", self.armor())); } }
            "potion" => { if d.heal > 0 { v.push(format!("heals {}", d.heal)); } if d.mana > 0 { v.push(format!("restores {} mana", d.mana)); } }
            "food" => v.push(format!("feeds {} turns", d.food)),
            "light" => v.push(format!("light {}", d.light)),
            _ => {}
        }
        if d.magic > 0 { v.push(format!("magic +{}", d.magic)); }
        if d.melee > 0 { v.push(format!("melee +{}", d.melee)); }
        if d.hp > 0 { v.push(format!("life +{}", d.hp)); }
        if d.poison > 0 { v.push("poisons".into()); }
        if let Some(r) = &d.resist { v.push(format!("wards off {}", r)); }
        if self.material.as_deref().and_then(|m| data().material(m)).map_or(false, |m| m.holy) { v.push("bane of the dead".into()); }
        if self.value() > 0 { v.push(format!("worth {} gold", self.value())); }
        v.join(", ")
    }
}

pub fn plural(s: &str) -> String {
    // The last word takes the plural; a few are irregular.
    if let Some((head, last)) = s.rsplit_once(' ') { if !matches!(last, "of" | "the") { return format!("{} {}", head, plural(last)); } }
    match s { "wolf" => return "wolves".into(), "dead" | "cyclops" | "sheep" | "fish" | "deer" => return s.into(), "mummy" => return "mummies".into(), "teeth" => return s.into(), _ => {} }
    if s.ends_with('s') || s.ends_with("sh") || s.ends_with("ch") { format!("{}es", s) }
    else if s.ends_with("ey") { format!("{}s", s) }
    else if s.ends_with('y') { format!("{}ies", &s[..s.len() - 1]) }
    else { format!("{}s", s) }
}

pub fn article(s: &str) -> String {
    let first = s.chars().next().unwrap_or('x').to_ascii_lowercase();
    if "aeiou".contains(first) { format!("an {}", s) } else { format!("a {}", s) }
}

/// Put `it` into `bag`, stacking with a like stack.
pub fn stow(bag: &mut Vec<Item>, it: Item) {
    if let Some(s) = bag.iter_mut().find(|s| s.same_stack(&it)) { s.count += it.count; } else { bag.push(it); }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn items_read_and_scale() {
        let s = Item::of("sword", "steel", 5);
        assert!(s.attack() > Item::new("sword", 1).attack());
        assert_eq!(Item::new("gold", 12).describe(), "12 gold coins");
        assert_eq!(Item::new("axe", 1).describe(), "an axe");
        assert!(Item::of("sword", "steel", 5).describe().contains("masterwork steel sword"));
        let mut bag = Vec::new();
        stow(&mut bag, Item::new("gold", 3));
        stow(&mut bag, Item::new("gold", 4));
        assert_eq!(bag.len(), 1);
        assert_eq!(bag[0].count, 7);
    }
}
