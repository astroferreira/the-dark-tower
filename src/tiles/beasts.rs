//! Creatures in ink: every animal, cave thing and monster the camp meets, drawn from its body.
//!
//! A `Look` is a body plan (a four-legged beast in profile, a spider or crab or insect or lizard
//! seen from above as old herbals draw them, a serpent, a worm, a bat with its wings open, a
//! fish, a bird, a blob, a great two-legged thing) with proportions, a coat, parts (antlers,
//! horns, tusks, wings, a shell, spines, a stinger...) and eyes. Game and cave life get theirs
//! from their names (`of_name`: "red deer", "wild boar", "pale crabs"); a generated monster from
//! what it is (`of_monster`: its base, its structural tweaks, its colour or substance, glowing
//! eyes), so the forgotten beast described in the log is the one walking up the stair.
//! Everything is drawn with the ink kit (`ink.rs`) so it shares the map's hand.

use super::ink::{colour_word, mix, Finish, Pen, Rgb, INK};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Base { Quad, Spider, Insect, Crab, Lizard, Serpent, Worm, Bat, Fish, Bird, Blob, Giant }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Horn { None, Antlers, Spiky, Palmate, Curved, Lyre, Bovine, Ram, Short }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tail { None, Stub, Long, Bushy, Tuft, Curly, Horse }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ear { Point, Round, Long, Floppy, None }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Eyes { Two, None, One, Three, Many }

/// Parts beyond the plan's own (bit flags).
pub mod part {
    pub const WINGS: u32 = 1;
    pub const TAIL: u32 = 2;
    pub const SHELL: u32 = 4;
    pub const TRUNK: u32 = 8;
    pub const HORNS: u32 = 16;
    pub const TUSKS: u32 = 32;
    pub const SPINES: u32 = 64;
    pub const ANTENNAE: u32 = 128;
    pub const MANDIBLES: u32 = 256;
    pub const STINGER: u32 = 512;
    pub const HOOD: u32 = 1024;
    pub const TENTACLES: u32 = 2048;
    pub const CREST: u32 = 4096;
    pub const MANE: u32 = 8192;
    pub const WOOL: u32 = 16384;
    pub const STRIPES: u32 = 32768;
    pub const SPOTS: u32 = 65536;
    pub const SHAGGY: u32 = 131072;
    pub const FEATHERS: u32 = 262144;
    pub const BEARD: u32 = 524288;
    pub const HUMP: u32 = 1048576;
    pub const LONG_HIND: u32 = 2097152;
    pub const FRILL: u32 = 4194304;
}

/// How a four-legged body is made (unit lengths; the sprite box is -1..1, ground at 0.8).
#[derive(Clone, Copy, Debug)]
pub struct Quad {
    pub body_rx: f32,
    pub body_ry: f32,
    pub leg: f32,
    pub leg_r: f32,
    pub neck: f32,
    /// Degrees above the horizontal (negative: the head carried low).
    pub neck_up: f32,
    pub neck_r: f32,
    pub head: f32,
    pub head_r: f32,
    pub snout: f32,
    pub horn: Horn,
    pub tail: Tail,
    pub ear: Ear,
    /// Hooves (dark feet) or paws.
    pub hooves: bool,
}

const QUAD: Quad = Quad { body_rx: 0.5, body_ry: 0.22, leg: 0.42, leg_r: 0.055, neck: 0.22, neck_up: 40.0, neck_r: 0.09, head: 0.22, head_r: 0.09, snout: 0.6, horn: Horn::None, tail: Tail::Stub, ear: Ear::Point, hooves: true };

#[derive(Clone, Debug)]
pub struct Look {
    pub base: Base,
    pub quad: Quad,
    pub coat: Rgb,
    /// Belly, face markings, the pale of a crab's underside.
    pub pale: Rgb,
    /// A second colour: mane, stripes, wing membrane, shell.
    pub accent: Rgb,
    pub parts: u32,
    pub eyes: Eyes,
    pub glow: Option<Rgb>,
    /// Opacity of the body (ice, smoke and glass things show the ground through).
    pub body_alpha: f32,
    /// Made of flame: a flickering glow and a bright core.
    pub flame: bool,
    /// Length in a settler's widths (a deer ~1.2, a hare 0.45, a forgotten beast 3+).
    pub len: f32,
    /// Flies: drawn above its shadow.
    pub flies: bool,
}

impl Look {
    fn new(base: Base, coat: Rgb, len: f32) -> Look {
        Look { base, quad: QUAD, coat, pale: mix(coat, [236.0, 226.0, 204.0], 0.55), accent: mix(coat, INK, 0.35), parts: 0, eyes: Eyes::Two, glow: None, body_alpha: 1.0, flame: false, len, flies: false }
    }
    fn has(&self, p: u32) -> bool { self.parts & p != 0 }
}

/// What the body is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pose { Stand, Walk(bool), Graze, Strike }

fn q(base: Quad, f: impl FnOnce(&mut Quad)) -> Quad { let mut x = base; f(&mut x); x }

/// The look of a creature by its name: game, livestock, pets, wolves, the camp's own dead, cave
/// life. Unknown names get a plain grazer.
pub fn of_name(name: &str) -> Look {
    let n = name.to_lowercase();
    let words: Vec<&str> = n.split(|c: char| !c.is_alphabetic()).filter(|w| !w.is_empty()).collect();
    // A phrase anywhere; a single word only at a word's start ("ox" is not in "fox").
    let has = |k: &str| if k.contains(' ') { n.contains(k) } else { words.iter().any(|w| w.starts_with(k)) };
    // Cave life first ("pale spiders" before "spider" monsters, "cave salamanders" before lizards).
    if has("bat") && !has("battle") { let mut l = Look::new(Base::Bat, [92.0, 80.0, 76.0], 0.55); l.accent = [70.0, 58.0, 56.0]; l.flies = true; return l; }
    if has("glowworm") { let mut l = Look::new(Base::Worm, [214.0, 220.0, 170.0], 0.4); l.glow = Some([190.0, 230.0, 150.0]); l.eyes = Eyes::None; return l; }
    if has("cricket") { let mut l = Look::new(Base::Insect, [196.0, 180.0, 150.0], 0.4); l.parts |= part::ANTENNAE | part::LONG_HIND; return l; }
    if has("spider") {
        let big = has("giant");
        let mut l = Look::new(Base::Spider, if has("pale") || big { [210.0, 204.0, 186.0] } else { [90.0, 80.0, 72.0] }, if big { 1.3 } else { 0.55 });
        l.parts |= part::MANDIBLES; l.eyes = Eyes::Many; return l;
    }
    if has("crab") || has("crawler") {
        let big = has("giant") || has("drowned");
        let mut l = Look::new(Base::Crab, if has("drowned") { [92.0, 116.0, 100.0] } else { [214.0, 200.0, 182.0] }, if big { 1.2 } else { 0.55 });
        if has("blind") { l.eyes = Eyes::None; }
        return l;
    }
    if has("salamander") || has("eyeless lizard") || has("newt") {
        let mut l = Look::new(Base::Lizard, if has("eyeless") || has("cave") { [226.0, 200.0, 186.0] } else { [190.0, 110.0, 60.0] }, 0.6);
        if has("eyeless") { l.eyes = Eyes::None; }
        return l;
    }
    if has("toad") { let mut l = Look::new(Base::Lizard, [150.0, 140.0, 100.0], if has("giant") { 0.9 } else { 0.4 }); l.parts |= part::SHAGGY; l.quad.body_ry = 0.5; return l; }
    if has("eel") || has("serpent") || has("snake") || has("adder") || has("viper") {
        let mut l = Look::new(Base::Serpent, if has("pale") { [220.0, 214.0, 196.0] } else { [110.0, 120.0, 80.0] }, if has("pale serpent") { 1.4 } else { 1.0 });
        if has("pale") { l.eyes = Eyes::None; }
        return l;
    }
    if has("grub") || has("maggot") || has("worm") {
        let mut l = Look::new(Base::Worm, [222.0, 210.0, 182.0], if has("giant") { 1.2 } else { 0.5 });
        if has("rock") { l.coat = [196.0, 184.0, 160.0]; }
        l.eyes = Eyes::None; return l;
    }
    if has("burrowing horror") { let mut l = Look::new(Base::Worm, [120.0, 96.0, 90.0], 1.8); l.parts |= part::MANDIBLES | part::SPINES; l.eyes = Eyes::None; return l; }
    if has("hunt by sound") { let mut l = Look::new(Base::Blob, [150.0, 136.0, 130.0], 1.2); l.parts |= part::TENTACLES; l.eyes = Eyes::None; return l; }
    if has("cave fish") || has("fish") { let mut l = Look::new(Base::Fish, [210.0, 210.0, 200.0], 0.5); if has("blind") { l.eyes = Eyes::None; } return l; }
    if has("mole") { let mut l = Look::new(Base::Quad, [84.0, 74.0, 70.0], 0.45); l.quad = q(QUAD, |x| { x.body_rx = 0.62; x.body_ry = 0.28; x.leg = 0.12; x.leg_r = 0.07; x.neck = 0.05; x.neck_up = 0.0; x.head = 0.3; x.head_r = 0.13; x.snout = 0.3; x.ear = Ear::None; x.tail = Tail::Long; x.hooves = false; }); l.eyes = Eyes::None; return l; }
    if has("rat") || has("mouse") { let mut l = Look::new(Base::Quad, [120.0, 108.0, 96.0], 0.35); l.quad = q(QUAD, |x| { x.body_rx = 0.5; x.body_ry = 0.24; x.leg = 0.14; x.neck = 0.06; x.neck_up = 5.0; x.head = 0.3; x.head_r = 0.11; x.snout = 0.35; x.ear = Ear::Round; x.tail = Tail::Long; x.hooves = false; }); return l; }
    // The night's dead and the cursed (they come as "wolves" in the simulation).
    if has("risen") || has("restless dead") { let mut l = Look::new(Base::Giant, [170.0, 176.0, 160.0], 1.0); l.parts |= part::SHAGGY; l.accent = [96.0, 92.0, 84.0]; l.eyes = Eyes::Two; l.glow = Some([150.0, 210.0, 190.0]); return l; }
    if has("under the full moon") || has(", changed") || has("were") {
        let mut l = Look::new(Base::Giant, [96.0, 84.0, 74.0], 1.25); l.parts |= part::SHAGGY | part::TAIL | part::MANE; l.glow = Some([230.0, 190.0, 80.0]); l.quad.ear = Ear::Point; l.quad.snout = 1.0; return l;
    }
    // Game, herds and pets.
    let mut l = Look::new(Base::Quad, [150.0, 110.0, 72.0], 1.1);
    if has("caribou") || has("reindeer") {
        l.coat = [140.0, 120.0, 96.0]; l.pale = [222.0, 214.0, 196.0]; l.parts |= part::MANE; l.accent = [214.0, 206.0, 188.0]; l.len = 1.2;
        l.quad = q(QUAD, |x| { x.body_ry = 0.24; x.leg = 0.4; x.neck_up = 30.0; x.horn = Horn::Spiky; });
    } else if has("moose") || has("elk") {
        l.coat = [92.0, 70.0, 54.0]; l.len = 1.45; l.parts |= part::HUMP;
        l.quad = q(QUAD, |x| { x.body_ry = 0.24; x.leg = 0.5; x.neck = 0.26; x.neck_up = 38.0; x.head = 0.28; x.head_r = 0.1; x.horn = if n.contains("moose") { Horn::Palmate } else { Horn::Antlers }; x.ear = Ear::Long; });
    } else if has("deer") || has("stag") || has("hind") {
        l.coat = [164.0, 106.0, 64.0]; l.len = 1.15;
        l.quad = q(QUAD, |x| { x.body_rx = 0.46; x.body_ry = 0.19; x.leg = 0.48; x.leg_r = 0.045; x.neck = 0.26; x.neck_up = 55.0; x.neck_r = 0.07; x.head = 0.2; x.head_r = 0.075; x.horn = Horn::Antlers; x.ear = Ear::Long; });
    } else if has("boar") || has("pig") || has("swine") || has("hog") {
        let wild = !has("pig");
        l.coat = if wild { [96.0, 78.0, 64.0] } else { [214.0, 162.0, 148.0] }; l.len = if wild { 0.95 } else { 0.85 };
        if wild { l.parts |= part::TUSKS | part::MANE; l.accent = [66.0, 54.0, 46.0]; }
        l.quad = q(QUAD, |x| { x.body_rx = 0.56; x.body_ry = 0.28; x.leg = 0.2; x.leg_r = 0.06; x.neck = 0.08; x.neck_up = -15.0; x.neck_r = 0.15; x.head = 0.32; x.head_r = 0.13; x.snout = 0.55; x.tail = if wild { Tail::Tuft } else { Tail::Curly }; x.ear = Ear::Point; });
    } else if has("aurochs") || has("ox") || has("cow") || has("cattle") || has("bison") || has("yak") || has("buffalo") || has("bull") {
        l.coat = if has("bison") || has("yak") { [92.0, 70.0, 52.0] } else if has("cow") || has("cattle") { [176.0, 136.0, 100.0] } else { [70.0, 54.0, 44.0] };
        l.len = 1.45;
        if has("bison") || has("yak") { l.parts |= part::HUMP | part::SHAGGY | part::MANE; l.accent = [70.0, 54.0, 42.0]; }
        if has("cow") || has("cattle") { l.parts |= part::SPOTS; l.accent = [236.0, 228.0, 212.0]; }
        l.quad = q(QUAD, |x| { x.body_rx = 0.58; x.body_ry = 0.28; x.leg = 0.32; x.leg_r = 0.07; x.neck = 0.14; x.neck_up = 5.0; x.neck_r = 0.14; x.head = 0.26; x.head_r = 0.11; x.snout = 0.75; x.horn = Horn::Bovine; x.tail = Tail::Tuft; x.ear = Ear::Floppy; });
    } else if has("antelope") || has("gazelle") {
        l.coat = [190.0, 142.0, 92.0]; l.pale = [236.0, 226.0, 204.0]; l.len = 1.0;
        l.quad = q(QUAD, |x| { x.body_rx = 0.44; x.body_ry = 0.18; x.leg = 0.46; x.leg_r = 0.04; x.neck = 0.24; x.neck_up = 50.0; x.neck_r = 0.065; x.head = 0.2; x.head_r = 0.07; x.horn = Horn::Lyre; x.ear = Ear::Long; });
    } else if has("ibex") || has("goat") {
        l.coat = if has("ibex") { [150.0, 124.0, 92.0] } else { [200.0, 190.0, 170.0] }; l.len = 0.9; l.parts |= part::BEARD;
        l.quad = q(QUAD, |x| { x.body_rx = 0.46; x.body_ry = 0.22; x.leg = 0.34; x.neck = 0.18; x.neck_up = 45.0; x.head = 0.2; x.horn = if n.contains("ibex") { Horn::Curved } else { Horn::Short }; x.tail = Tail::Stub; });
    } else if has("sheep") || has("ram") {
        l.coat = [226.0, 218.0, 198.0]; l.pale = [70.0, 62.0, 56.0]; l.len = 0.9; l.parts |= part::WOOL;
        l.quad = q(QUAD, |x| { x.body_rx = 0.52; x.body_ry = 0.27; x.leg = 0.26; x.leg_r = 0.045; x.neck = 0.1; x.neck_up = 20.0; x.head = 0.22; x.head_r = 0.09; x.horn = if n.contains("ram") { Horn::Ram } else { Horn::None }; x.ear = Ear::Floppy; });
    } else if has("horse") || has("mule") || has("donkey") || has("pony") || has("camel") {
        l.coat = if has("donkey") || has("mule") { [138.0, 124.0, 112.0] } else if has("camel") { [196.0, 160.0, 110.0] } else { [132.0, 88.0, 58.0] };
        l.len = 1.4; l.parts |= part::MANE; l.accent = mix(l.coat, INK, 0.45);
        if has("camel") { l.parts = part::HUMP; }
        l.quad = q(QUAD, |x| { x.body_rx = 0.52; x.body_ry = 0.22; x.leg = 0.48; x.leg_r = 0.055; x.neck = 0.3; x.neck_up = 55.0; x.neck_r = 0.1; x.head = 0.26; x.head_r = 0.08; x.snout = 0.7; x.tail = Tail::Horse; x.ear = if n.contains("donkey") || n.contains("mule") { Ear::Long } else { Ear::Point }; });
    } else if has("wolf") || has("dog") || has("hound") || has("jackal") || has("fox") || has("coyote") {
        let fox = has("fox");
        l.coat = if fox { [196.0, 104.0, 52.0] } else if has("dog") || has("hound") { [150.0, 116.0, 80.0] } else { [124.0, 120.0, 112.0] };
        l.pale = [226.0, 220.0, 206.0]; l.len = if fox { 0.7 } else if has("dog") { 0.8 } else { 1.0 };
        l.quad = q(QUAD, |x| { x.body_rx = 0.46; x.body_ry = 0.18; x.leg = 0.34; x.leg_r = 0.045; x.neck = 0.14; x.neck_up = 30.0; x.neck_r = 0.08; x.head = 0.24; x.head_r = 0.08; x.snout = 0.45; x.tail = Tail::Bushy; x.ear = Ear::Point; x.hooves = false; });
    } else if has("bear") {
        l.coat = if has("cave") || has("white") || has("polar") { [214.0, 206.0, 186.0] } else { [98.0, 72.0, 52.0] }; l.len = 1.4; l.parts |= part::SHAGGY;
        l.quad = q(QUAD, |x| { x.body_rx = 0.58; x.body_ry = 0.3; x.leg = 0.28; x.leg_r = 0.09; x.neck = 0.1; x.neck_up = 5.0; x.neck_r = 0.15; x.head = 0.26; x.head_r = 0.13; x.snout = 0.5; x.tail = Tail::None; x.ear = Ear::Round; x.hooves = false; });
    } else if has("lion") || has("cat") || has("lynx") || has("tiger") || has("leopard") {
        let big = !has("cat") || has("great");
        l.coat = if has("tiger") { [206.0, 128.0, 60.0] } else if has("leopard") { [206.0, 170.0, 100.0] } else if has("cat") { [150.0, 130.0, 110.0] } else { [196.0, 156.0, 96.0] };
        l.len = if big { 1.3 } else { 0.55 };
        if has("lion") { l.parts |= part::MANE; l.accent = [140.0, 92.0, 52.0]; }
        if has("tiger") { l.parts |= part::STRIPES; l.accent = [60.0, 46.0, 40.0]; }
        if has("leopard") || has("lynx") { l.parts |= part::SPOTS; l.accent = [80.0, 62.0, 48.0]; }
        l.quad = q(QUAD, |x| { x.body_rx = 0.5; x.body_ry = 0.2; x.leg = 0.32; x.leg_r = 0.06; x.neck = 0.1; x.neck_up = 25.0; x.neck_r = 0.1; x.head = 0.2; x.head_r = 0.11; x.snout = 0.5; x.tail = if big { Tail::Tuft } else { Tail::Long }; x.ear = Ear::Round; x.hooves = false; });
    } else if has("hare") || has("rabbit") {
        l.coat = [160.0, 134.0, 104.0]; l.len = 0.45;
        l.quad = q(QUAD, |x| { x.body_rx = 0.42; x.body_ry = 0.28; x.leg = 0.18; x.leg_r = 0.07; x.neck = 0.06; x.neck_up = 40.0; x.head = 0.22; x.head_r = 0.12; x.snout = 0.5; x.tail = Tail::Stub; x.ear = Ear::Long; x.hooves = false; });
    } else if has("mammoth") || has("elephant") {
        l.coat = if has("mammoth") { [110.0, 78.0, 56.0] } else { [140.0, 136.0, 130.0] }; l.len = 2.2;
        l.parts |= part::TRUNK | part::TUSKS | if has("mammoth") { part::SHAGGY | part::HUMP } else { 0 };
        l.quad = q(QUAD, |x| { x.body_rx = 0.56; x.body_ry = 0.32; x.leg = 0.36; x.leg_r = 0.1; x.neck = 0.06; x.neck_up = 10.0; x.neck_r = 0.2; x.head = 0.18; x.head_r = 0.18; x.snout = 0.9; x.tail = Tail::Tuft; x.ear = Ear::Floppy; });
    } else if has("tortoise") || has("turtle") {
        l.coat = [130.0, 128.0, 96.0]; l.accent = [112.0, 96.0, 64.0]; l.len = 0.7; l.parts |= part::SHELL;
        l.quad = q(QUAD, |x| { x.body_rx = 0.5; x.body_ry = 0.24; x.leg = 0.14; x.leg_r = 0.07; x.neck = 0.18; x.neck_up = 15.0; x.neck_r = 0.06; x.head = 0.14; x.head_r = 0.07; x.snout = 0.7; x.tail = Tail::Stub; x.ear = Ear::None; x.hooves = false; });
    } else if has("lizard") || has("crocodile") || has("dragon") || has("newt") {
        let mut m = Look::new(Base::Lizard, [110.0, 120.0, 80.0], if has("crocodile") { 1.6 } else { 0.6 });
        if has("dragon") { m.parts |= part::WINGS | part::HORNS; m.len = 2.5; }
        return m;
    } else if has("eagle") || has("hawk") || has("owl") || has("raven") || has("crow") || has("vulture") || has("bird") || has("roc") || has("hen") || has("goose") || has("duck") {
        let mut m = Look::new(Base::Bird, if has("raven") || has("crow") { [60.0, 56.0, 60.0] } else if has("owl") { [170.0, 140.0, 104.0] } else if has("goose") || has("duck") || has("hen") { [226.0, 220.0, 204.0] } else { [120.0, 92.0, 66.0] }, if has("roc") { 2.5 } else { 0.55 });
        if has("vulture") { m.pale = [210.0, 150.0, 140.0]; }
        return m;
    }
    l
}

/// The look of a generated monster (`monsters::Monster`): its base, its structural parts, its
/// colour or substance, its eyes and their glow.
pub fn of_monster(m: &crate::monsters::Monster) -> Look {
    let profile = m.profile.as_str();
    let mut l = match m.base.as_str() {
        "quadruped" => match profile { "bat" => of_name("bat"), "rat" => of_name("rat"), "mole" => of_name("mole"), "tortoise" => of_name("tortoise"), p => of_name(p) },
        "insect" => { let mut l = Look::new(Base::Insect, [110.0, 90.0, 60.0], 1.0); if profile == "locust" || profile == "mantis" { l.parts |= part::LONG_HIND; } l }
        "spider" => { let mut l = of_name("spider"); if profile == "scorpion" { l.base = Base::Crab; l.parts |= part::STINGER | part::TAIL; } if profile == "tick" { l.quad.body_ry = 0.5; } l }
        "serpent" => of_name("serpent"),
        "worm" => { let mut l = of_name("worm"); if profile == "centipede" { l.parts |= part::LONG_HIND; } l }
        "crustacean" => of_name("crab"),
        "lizard" => { let mut l = Look::new(Base::Lizard, [110.0, 120.0, 80.0], 1.0); if profile == "toad" { l.quad.body_ry = 0.5; l.parts |= part::SHAGGY; } if profile == "crocodile" { l.quad.snout = 1.6; } l }
        "bird" => of_name(profile),
        "humanoid" => { let mut l = Look::new(Base::Giant, [130.0, 120.0, 100.0], 1.4); if profile == "ape" { l.parts |= part::SHAGGY; } if profile == "ogre" { l.quad.body_ry = 0.5; } if profile == "troll" { l.quad.leg = 0.3; } l }
        "blob" => { let mut l = Look::new(Base::Blob, [150.0, 130.0, 120.0], 1.0); if profile == "octopus" { l.parts |= part::TENTACLES; } l }
        _ => Look::new(Base::Quad, [130.0, 110.0, 90.0], 1.0),
    };
    // Colour: the whole body of one substance, else its skin's colour.
    let (coat, alpha, flame) = match m.material.as_deref() {
        Some("flame") => ([226.0, 120.0, 40.0], 0.9, true),
        Some("ice") => ([196.0, 222.0, 232.0], 0.78, false),
        Some("snow") => ([236.0, 236.0, 232.0], 1.0, false),
        Some("steam") | Some("smoke") => ([176.0, 176.0, 172.0], 0.62, false),
        Some("glass") => ([200.0, 220.0, 214.0], 0.6, false),
        Some(mat) => (colour_word(mat), 1.0, false),
        None => (colour_word(&m.colour), 1.0, false),
    };
    l.coat = coat;
    l.pale = mix(coat, [236.0, 226.0, 204.0], 0.45);
    l.accent = if m.colour.contains(" and ") { colour_word(m.colour.split(" and ").nth(1).unwrap_or("")) } else { mix(coat, INK, 0.35) };
    l.body_alpha = alpha;
    l.flame = flame;
    for t in &m.tweaks {
        l.parts |= match t.as_str() {
            "wings" => part::WINGS, "tail" => part::TAIL, "shell" => part::SHELL, "trunk" => part::TRUNK, "horns" => part::HORNS,
            "tusks" => part::TUSKS, "spines" => part::SPINES, "antennae" => part::ANTENNAE, "mandibles" => part::MANDIBLES,
            "stinger" => part::STINGER, "hood" => part::HOOD, "tentacles" => part::TENTACLES, "crest" => part::CREST, _ => 0,
        };
    }
    let s = m.short.to_lowercase();
    if s.contains("shaggy") { l.parts |= part::SHAGGY; }
    if s.contains("striped") { l.parts |= part::STRIPES; }
    if s.contains("frilled") { l.parts |= part::FRILL; }
    if s.contains("plumed") || m.class == "feathered reptile" { l.parts |= part::FEATHERS; }
    if s.contains("translucent") { l.body_alpha = l.body_alpha.min(0.65); }
    if s.contains("bloated") { l.quad.body_ry *= 1.3; }
    if l.has(part::HORNS) && l.base == Base::Quad && l.quad.horn == Horn::None { l.quad.horn = Horn::Bovine; }
    l.eyes = match m.eyes.as_deref() { Some("eyeless") => Eyes::None, Some("one-eyed") => Eyes::One, Some("three-eyed") => Eyes::Three, Some("many-eyed") => Eyes::Many, _ => if l.eyes == Eyes::None { Eyes::None } else { Eyes::Two } };
    l.glow = m.glow.as_deref().map(colour_word).map(|c| mix(c, [255.0, 250.0, 220.0], 0.3));
    l.flies = m.flies;
    l.len = (1.2 + m.size * 0.55).clamp(1.2, 4.0);
    l
}

/// Draw `look` standing at (x, y) on screen (the middle of its feet), `px` pixels for its unit
/// box (two units across), facing left or right, in `pose`, at opacity `alpha`.
pub fn draw(put: &mut dyn FnMut(i64, i64, Rgb, f32), look: &Look, x: f32, y: f32, px: f32, left: bool, pose: Pose, alpha: f32) {
    // The sprite's ground line is v = 0.8 for profile bodies; top-down ones sit centred on the spot.
    let top_down = matches!(look.base, Base::Spider | Base::Insect | Base::Crab | Base::Lizard | Base::Serpent | Base::Worm);
    let cy = if top_down { y } else { y - 0.8 * px / 2.0 };
    let lift = if look.flies { px * 0.35 } else { 0.0 };
    {
        // The shadow on the ground (under a flier, apart from it).
        let mut pen = Pen::new(put, x, cy, px).facing_left(left).faint(alpha);
        let (sv, srx) = if top_down { (0.05, 0.75) } else { (0.8, 0.6) };
        if look.flies || !top_down { pen.ground_shadow(0.0, sv, srx, 0.1 + if top_down { 0.35 } else { 0.0 }); }
    }
    let mut pen = Pen::new(put, x, cy - lift, px).facing_left(left).faint(alpha * look.body_alpha);
    if look.flame { pen.glow(0.0, 0.1, 1.1, [250.0, 170.0, 70.0], 0.35); }
    let step = match pose { Pose::Walk(a) => if a { 1.0 } else { -1.0 }, _ => 0.0 };
    match look.base {
        Base::Quad => quad(&mut pen, look, step, pose),
        Base::Spider => spider(&mut pen, look, step),
        Base::Insect => insect(&mut pen, look, step),
        Base::Crab => crab(&mut pen, look, step),
        Base::Lizard => lizard(&mut pen, look, step),
        Base::Serpent => serpent(&mut pen, look, step),
        Base::Worm => worm(&mut pen, look, step),
        Base::Bat => bat(&mut pen, look, step),
        Base::Fish => fish(&mut pen, look, step),
        Base::Bird => bird(&mut pen, look, step),
        Base::Blob => blob(&mut pen, look, step),
        Base::Giant => giant(&mut pen, look, step, pose),
    }
    if look.flame {
        // Licks of flame off its back.
        for k in 0..4 {
            let u = -0.5 + k as f32 * 0.32;
            let h = 0.18 + 0.1 * ((k as f32 * 1.7 + step).sin().abs());
            pen.poly_f(&[(u - 0.08, -0.05), (u + 0.02, -0.05 - h - 0.25), (u + 0.1, -0.05)], [250.0, 200.0, 90.0], Finish::Paint);
        }
    }
}

/// Eyes on a head at (u, v) of radius r: a dot, none, a great single eye, three, many; glowing.
fn eyes(pen: &mut Pen, look: &Look, u: f32, v: f32, r: f32) {
    let ink = INK;
    let spots: Vec<(f32, f32)> = match look.eyes {
        Eyes::None => Vec::new(),
        Eyes::Two => vec![(u, v)],
        Eyes::One => vec![(u - r * 0.1, v)],
        Eyes::Three => vec![(u, v), (u - r * 0.5, v - r * 0.35), (u + r * 0.2, v - r * 0.45)],
        Eyes::Many => vec![(u, v), (u - r * 0.45, v - r * 0.3), (u + r * 0.25, v - r * 0.4), (u - r * 0.2, v + r * 0.25), (u - r * 0.7, v), (u + r * 0.05, v - r * 0.8)],
    };
    if look.eyes == Eyes::One && pen.half >= 6.0 {
        pen.ellipse_f(u - r * 0.1, v, r * 0.45, r * 0.4, [236.0, 226.0, 196.0], Finish::Plain);
        pen.dot(u - r * 0.05, v, look.glow.unwrap_or(ink));
    }
    for &(a, b) in &spots {
        match look.glow {
            Some(g) => { pen.glow(a, b, (r * 1.4).max(1.6 / pen.half), g, 0.8); pen.dot(a, b, mix(g, [255.0, 255.0, 240.0], 0.5)); }
            None => pen.dot(a, b, ink),
        }
    }
}

fn quad(pen: &mut Pen, l: &Look, step: f32, pose: Pose) {
    let qd = l.quad;
    let g = 0.8;
    let coat = l.coat;
    let far = mix(coat, INK, 0.22);
    let (rx, ry) = (qd.body_rx, qd.body_ry);
    let (bx, by) = (-0.1, g - qd.leg - ry * 0.55);
    let hoof = mix(coat, INK, 0.65);
    // Legs: two segments, a little bend at the knee; the diagonal pairs swing together.
    let leg = |pen: &mut Pen, ux: f32, swing: f32, c: Rgb, front: bool| {
        let top = (ux, by + ry * 0.2);
        let foot = (ux + swing * 0.09, g);
        let knee = (ux + swing * 0.04 + if front { 0.02 } else { -0.04 }, by + ry * 0.6 + (g - by - ry * 0.6) * 0.45);
        pen.limb(top, qd.leg_r * 1.5, knee, qd.leg_r, c);
        pen.limb(knee, qd.leg_r, foot, qd.leg_r * 0.85, c);
        if qd.hooves { pen.rect_f(foot.0 - qd.leg_r, g - qd.leg_r * 1.1, foot.0 + qd.leg_r, g + qd.leg_r * 0.3, hoof, Finish::Plain); }
    };
    let (fx, hx) = (bx + rx * 0.62, bx - rx * 0.6);
    let pose_graze = pose == Pose::Graze;
    leg(pen, fx - 0.07, -step, far, true);
    leg(pen, hx - 0.07, step, far, false);
    leg(pen, fx, step, coat, true);
    leg(pen, hx, -step, coat, false);
    // Tail.
    let tail_root = (bx - rx * 0.95, by - ry * 0.35);
    match qd.tail {
        Tail::None => {}
        Tail::Stub => pen.ellipse(tail_root.0 - 0.02, tail_root.1, 0.05, 0.06, l.pale),
        Tail::Long => pen.limb(tail_root, 0.035, (tail_root.0 - 0.4, tail_root.1 + 0.15 + 0.05 * step), 0.012, mix(coat, l.pale, 0.3)),
        Tail::Bushy => pen.limb(tail_root, 0.06, (tail_root.0 - 0.28, tail_root.1 + 0.28), 0.085, coat),
        Tail::Tuft => { let end = (tail_root.0 - 0.18, tail_root.1 + 0.36); pen.limb(tail_root, 0.022, end, 0.018, coat); pen.ellipse(end.0, end.1, 0.045, 0.06, l.accent); }
        Tail::Curly => pen.path(&[tail_root, (tail_root.0 - 0.08, tail_root.1 - 0.06), (tail_root.0 - 0.12, tail_root.1 + 0.02), (tail_root.0 - 0.06, tail_root.1 + 0.04)], INK, 1.2),
        Tail::Horse => pen.limb((tail_root.0, tail_root.1 + 0.02), 0.06, (tail_root.0 - 0.14, tail_root.1 + 0.5), 0.05, l.accent),
    }
    if l.has(part::TAIL) && qd.tail == Tail::None || l.has(part::TAIL) && matches!(qd.tail, Tail::Stub) {
        pen.limb(tail_root, 0.06, (tail_root.0 - 0.55, tail_root.1 + 0.25), 0.015, coat);
    }
    // Wings folded up over the back (a monster's): drawn behind the body.
    if l.has(part::WINGS) { wing_profile(pen, l, (bx + rx * 0.3, by - ry * 0.6), 0.9, step); }
    // Body (a hump over the shoulders, wool in scallops, shaggy hair hanging).
    pen.ellipse(bx, by, rx, ry, coat);
    if l.has(part::HUMP) { pen.ellipse(bx + rx * 0.45, by - ry * 0.6, rx * 0.38, ry * 0.6, coat); }
    if l.has(part::WOOL) {
        for k in 0..7 {
            let a = k as f32 / 7.0 * std::f32::consts::TAU;
            pen.ellipse(bx + a.cos() * rx * 0.8, by + a.sin() * ry * 0.75, rx * 0.3, ry * 0.42, coat);
        }
        pen.ellipse(bx, by, rx * 0.75, ry * 0.7, coat);
    }
    if l.has(part::SHAGGY) {
        for k in 0..8 {
            let u = bx - rx * 0.8 + k as f32 * rx * 0.23;
            pen.line((u, by + ry * 0.5), (u - 0.02, by + ry * 1.15), mix(coat, INK, 0.45), 1.0);
        }
    }
    // The pale belly.
    pen.shape(l.pale, Finish::Paint, [bx - rx, by, bx + rx, by + ry], &move |u, v| {
        let e = ((u - bx) / rx).powi(2) + ((v - by) / ry).powi(2);
        e < 0.72 && v > by + ry * 0.45
    });
    if l.has(part::STRIPES) {
        for k in 0..5 {
            let u0 = bx - rx * 0.6 + k as f32 * rx * 0.3;
            let c = l.accent;
            pen.shape(c, Finish::Paint, [u0 - 0.05, by - ry, u0 + 0.05, by + ry], &move |u, v| (u - u0 - (v - by) * 0.3).abs() < 0.025 && ((u - bx) / rx).powi(2) + ((v - by) / ry).powi(2) < 0.8);
        }
    }
    if l.has(part::SPOTS) {
        for k in 0..5u32 {
            let (u, v) = (bx - rx * 0.55 + (k as f32 * 0.37 % 1.0) * rx * 1.2, by - ry * 0.3 + ((k * 7 % 5) as f32 / 5.0) * ry * 0.6);
            pen.ellipse_f(u, v, rx * 0.12, ry * 0.2, l.accent, Finish::Paint);
        }
    }
    if l.has(part::SHELL) {
        let shell = l.accent;
        pen.shape(shell, Finish::Inked, [bx - rx * 1.05, by - ry * 1.5, bx + rx * 1.05, by + ry * 0.4], &move |u, v| v < by + ry * 0.35 && ((u - bx) / (rx * 1.05)).powi(2) + ((v - by - ry * 0.35) / (ry * 1.85)).powi(2) <= 1.0);
        for k in [-0.4f32, 0.0, 0.4] { pen.line((bx + k * rx, by - ry * 1.2), (bx + k * rx * 1.3, by + ry * 0.3), INK, 1.0); }
    }
    if l.has(part::SPINES) {
        for k in 0..6 {
            let u = bx - rx * 0.7 + k as f32 * rx * 0.28;
            let v = by - ry * (1.0 - ((u - bx) / rx).powi(2)).max(0.0).sqrt();
            pen.poly(&[(u - 0.04, v + 0.03), (u - 0.02, v - 0.16), (u + 0.04, v + 0.03)], mix(l.accent, [236.0, 226.0, 204.0], 0.4));
        }
    }
    // Neck and head.
    let up = if pose_graze { -45.0f32 } else { qd.neck_up }.to_radians();
    let n0 = (bx + rx * 0.72, by - ry * 0.25);
    let n1 = (n0.0 + up.cos() * qd.neck, n0.1 - up.sin() * qd.neck);
    if qd.neck > 0.02 { pen.limb(n0, qd.neck_r * 1.3, n1, qd.neck_r, coat); }
    if l.has(part::MANE) {
        let c = l.accent;
        pen.limb((n0.0 - 0.04, n0.1 - ry * 0.4), qd.neck_r * 0.9, (n1.0 - 0.02, n1.1 - qd.head_r * 0.6), qd.neck_r * 0.75, c);
        if qd.ear == Ear::Round && qd.neck_up < 30.0 { pen.ellipse(n1.0, n1.1, qd.head_r * 1.8, qd.head_r * 1.7, c); }
    }
    let drop = if pose_graze { 0.6f32 } else { (qd.neck_up / 90.0).clamp(-0.4, 0.6) * 0.45 };
    let ha = (n1.0, n1.1);
    let hb = (ha.0 + qd.head * (1.0 - drop * 0.3), ha.1 + qd.head * drop);
    // Ears behind the head.
    let ear_c = mix(coat, INK, 0.12);
    match qd.ear {
        Ear::Point => pen.poly(&[(ha.0 - qd.head_r * 0.6, ha.1 - qd.head_r * 0.5), (ha.0 - qd.head_r * 0.35, ha.1 - qd.head_r * 2.3), (ha.0 + qd.head_r * 0.4, ha.1 - qd.head_r * 0.6)], ear_c),
        Ear::Round => pen.ellipse(ha.0 - qd.head_r * 0.3, ha.1 - qd.head_r * 0.95, qd.head_r * 0.42, qd.head_r * 0.42, ear_c),
        Ear::Long => pen.ellipse_rot(ha.0 - qd.head_r * 0.5, ha.1 - qd.head_r * 1.4, qd.head_r * 0.35, qd.head_r * 1.1, -0.5, ear_c),
        Ear::Floppy => pen.ellipse_rot(ha.0 - qd.head_r * 0.6, ha.1 + qd.head_r * 0.2, qd.head_r * 0.9, qd.head_r * 0.35, 0.6, ear_c),
        Ear::None => {}
    }
    pen.limb(ha, qd.head_r, hb, qd.head_r * qd.snout, coat);
    // Muzzle: the snout's tip darker.
    pen.ellipse_f(hb.0, hb.1, qd.head_r * qd.snout * 0.8, qd.head_r * qd.snout * 0.8, mix(coat, INK, 0.45), Finish::Plain);
    if l.has(part::BEARD) { pen.poly(&[(hb.0 - qd.head_r * 0.6, hb.1 + qd.head_r * 0.3), (hb.0 - qd.head_r * 0.3, hb.1 + qd.head_r * 2.0), (hb.0, hb.1 + qd.head_r * 0.4)], mix(coat, INK, 0.3)); }
    if l.has(part::TRUNK) {
        let c = coat;
        pen.limb(hb, qd.head_r * 0.45, (hb.0 + 0.08, hb.1 + 0.25), qd.head_r * 0.3, c);
        pen.limb((hb.0 + 0.08, hb.1 + 0.25), qd.head_r * 0.3, (hb.0 + 0.02, hb.1 + 0.42), qd.head_r * 0.22, c);
    }
    if l.has(part::TUSKS) {
        let ivory = [234.0, 224.0, 196.0];
        let big = l.has(part::TRUNK);
        if big { pen.bone(&[(hb.0 - 0.02, hb.1 + 0.05), (hb.0 + 0.12, hb.1 + 0.18), (hb.0 + 0.22, hb.1 + 0.08)], ivory, (pen.half * 0.06).max(1.5)); }
        else { pen.poly(&[(hb.0 - qd.head_r * 0.4, hb.1), (hb.0 + qd.head_r * 0.2, hb.1 - qd.head_r * 1.3), (hb.0 + qd.head_r * 0.1, hb.1 + qd.head_r * 0.1)], ivory); }
    }
    // Horns and antlers from the crown.
    let crown = (ha.0 + qd.head_r * 0.1, ha.1 - qd.head_r * 0.8);
    let horn = [228.0, 214.0, 186.0];
    let antler = [196.0, 170.0, 130.0];
    let w = (pen.half * 0.035).max(1.0);
    match qd.horn {
        Horn::None => {}
        Horn::Antlers => {
            let (cx, cy) = crown;
            pen.bone(&[(cx, cy), (cx - 0.06, cy - 0.18), (cx - 0.02, cy - 0.34), (cx + 0.06, cy - 0.44)], antler, w * 1.3);
            pen.bone(&[(cx - 0.05, cy - 0.14), (cx + 0.06, cy - 0.2)], antler, w);
            pen.bone(&[(cx - 0.03, cy - 0.29), (cx + 0.08, cy - 0.32)], antler, w);
            pen.bone(&[(cx - 0.02, cy - 0.34), (cx - 0.12, cy - 0.42)], antler, w);
        }
        Horn::Spiky => {
            let (cx, cy) = crown;
            pen.bone(&[(cx, cy), (cx - 0.1, cy - 0.2), (cx - 0.06, cy - 0.38), (cx + 0.06, cy - 0.46)], antler, w * 1.3);
            for k in 0..3 { let t = 0.2 + k as f32 * 0.09; pen.bone(&[(cx - 0.08, cy - t), (cx + 0.04, cy - t - 0.04)], antler, w); }
            pen.bone(&[(cx + 0.01, cy - 0.06), (cx + 0.12, cy + 0.02)], antler, w);
        }
        Horn::Palmate => {
            let (cx, cy) = crown;
            pen.limb((cx, cy), 0.025, (cx - 0.04, cy - 0.1), 0.02, antler);
            pen.poly(&[(cx - 0.04, cy - 0.08), (cx - 0.22, cy - 0.16), (cx - 0.26, cy - 0.28), (cx - 0.16, cy - 0.24), (cx - 0.14, cy - 0.32), (cx - 0.05, cy - 0.24), (cx + 0.0, cy - 0.3), (cx + 0.06, cy - 0.12)], antler);
        }
        Horn::Curved => {
            let (cx, cy) = crown;
            pen.bone(&[(cx, cy + 0.02), (cx - 0.04, cy - 0.14), (cx - 0.14, cy - 0.24), (cx - 0.26, cy - 0.22), (cx - 0.32, cy - 0.1)], horn, w * 2.2);
        }
        Horn::Lyre => {
            let (cx, cy) = crown;
            pen.path(&[(cx, cy), (cx - 0.05, cy - 0.16), (cx, cy - 0.3), (cx - 0.04, cy - 0.38)], [80.0, 66.0, 54.0], w * 1.4);
        }
        Horn::Bovine => {
            let (cx, cy) = (crown.0 + 0.02, crown.1 + 0.02);
            pen.bone(&[(cx - 0.02, cy), (cx + 0.08, cy - 0.06), (cx + 0.12, cy - 0.16), (cx + 0.1, cy - 0.22)], horn, w * 2.0);
        }
        Horn::Ram => {
            let (cx, cy) = (crown.0 - 0.02, crown.1 + 0.04);
            let pts: Vec<(f32, f32)> = (0..10).map(|k| { let a = k as f32 / 9.0 * 4.6 - 1.4; let r = 0.12 - k as f32 * 0.008; (cx - 0.02 + a.cos() * r, cy + 0.04 + a.sin() * r) }).collect();
            pen.bone(&pts, horn, w * 2.0);
        }
        Horn::Short => { let (cx, cy) = crown; pen.path(&[(cx, cy), (cx - 0.06, cy - 0.12), (cx - 0.1, cy - 0.15)], [120.0, 104.0, 84.0], w * 1.4); }
    }
    if l.has(part::CREST) { let (cx, cy) = crown; pen.poly(&[(cx - 0.12, cy + 0.02), (cx - 0.04, cy - 0.22), (cx + 0.04, cy - 0.1), (cx + 0.08, cy + 0.02)], l.accent); }
    // The eye.
    eyes(pen, l, ha.0 + (hb.0 - ha.0) * 0.3, ha.1 + (hb.1 - ha.1) * 0.25 - qd.head_r * 0.25, qd.head_r * 0.9);
}

/// A bat-like or feathered wing raised over the back (profile bodies).
fn wing_profile(pen: &mut Pen, l: &Look, root: (f32, f32), span: f32, step: f32) {
    let lift = 0.12 * step;
    let (rx, ry) = root;
    if l.has(part::FEATHERS) {
        let c = l.accent;
        for k in 0..5 {
            let a = (100.0 + k as f32 * 18.0).to_radians();
            let tip = (rx + a.cos() * span * (0.7 + 0.08 * k as f32), ry - a.sin() * span * (0.85 - 0.08 * k as f32) - lift);
            pen.limb(root, 0.04, tip, 0.07, mix(c, [236.0, 226.0, 204.0], 0.1 * k as f32));
        }
        return;
    }
    let bone = mix(l.coat, INK, 0.4);
    let tips: Vec<(f32, f32)> = (0..4).map(|k| { let a = (95.0 + k as f32 * 22.0).to_radians(); (rx + a.cos() * span * 0.9, ry - a.sin() * span * (0.95 - k as f32 * 0.12) - lift) }).collect();
    let mut pts = vec![root];
    pts.extend(tips.iter().copied());
    pts.push((rx - span * 0.55, ry + 0.02));
    let membrane = mix(l.accent, [236.0, 226.0, 204.0], 0.15);
    pen.poly(&pts, membrane);
    for t in &tips { pen.line(root, *t, bone, 1.0); }
}

fn spider(pen: &mut Pen, l: &Look, step: f32) {
    let leg_c = mix(l.coat, INK, 0.3);
    let w = (pen.half * 0.05).max(1.0);
    for side in [-1.0f32, 1.0] {
        for k in 0..4 {
            let a = (-55.0 + k as f32 * 36.0).to_radians();
            let wig = if (k % 2 == 0) == (side > 0.0) { step * 0.06 } else { -step * 0.06 };
            let root = (0.12 + k as f32 * -0.05, side * 0.1);
            let knee = (root.0 + a.sin() * 0.3 + wig, side * (0.42 + 0.05 * (k as f32 - 1.5).abs()));
            let foot = (knee.0 + a.sin() * 0.36 + wig, side * 0.82);
            pen.path(&[root, knee, foot], leg_c, w);
        }
    }
    pen.ellipse(-0.3, 0.0, 0.38 * (1.0 + l.quad.body_ry - 0.22), 0.3 * (1.0 + l.quad.body_ry - 0.22), l.coat);
    pen.ellipse_f(-0.34, 0.0, 0.14, 0.08, l.accent, Finish::Paint);
    pen.ellipse(0.15, 0.0, 0.2, 0.17, mix(l.coat, l.pale, 0.2));
    if l.has(part::MANDIBLES) {
        for s in [-1.0f32, 1.0] { pen.limb((0.3, s * 0.06), 0.045, (0.42, s * 0.03), 0.02, mix(l.coat, INK, 0.5)); }
    }
    if l.has(part::STINGER) { pen.path(&[(-0.66, 0.0), (-0.85, -0.1), (-0.82, -0.22)], INK, w * 1.5); }
    eyes(pen, l, 0.24, -0.04, 0.12);
}

fn insect(pen: &mut Pen, l: &Look, step: f32) {
    let leg_c = mix(l.coat, INK, 0.4);
    let w = (pen.half * 0.04).max(1.0);
    for side in [-1.0f32, 1.0] {
        for k in 0..3 {
            let wig = if (k % 2 == 0) == (side > 0.0) { step * 0.07 } else { -step * 0.07 };
            let root = (0.12 - k as f32 * 0.1, side * 0.08);
            let long = l.has(part::LONG_HIND) && k == 2;
            let knee = (root.0 + if k == 0 { 0.18 } else if long { -0.05 } else { -0.05 } + wig, side * if long { 0.42 } else { 0.32 });
            let foot = (knee.0 + if k == 0 { 0.16 } else if long { -0.5 } else { -0.18 } + wig, side * if long { 0.32 } else { 0.58 });
            pen.path(&[root, knee, foot], leg_c, if long { w * 1.6 } else { w });
        }
    }
    // Abdomen, thorax, head.
    pen.ellipse(-0.38, 0.0, 0.36, 0.2, l.coat);
    for k in 0..3 { let u = -0.55 + k as f32 * 0.14; pen.line((u, -0.17), (u, 0.17), mix(l.coat, INK, 0.5), 1.0); }
    if l.has(part::STRIPES) { for k in 0..2 { let u = -0.5 + k as f32 * 0.2; pen.ellipse_f(u, 0.0, 0.05, 0.17, l.accent, Finish::Paint); } }
    pen.ellipse(0.04, 0.0, 0.14, 0.13, mix(l.coat, INK, 0.1));
    pen.ellipse(0.26, 0.0, 0.1, 0.1, l.coat);
    if l.has(part::WINGS) {
        for s in [-1.0f32, 1.0] {
            pen.ellipse_rot(-0.32, s * 0.22, 0.4, 0.14, s * 0.35, [226.0, 230.0, 224.0]);
            pen.line((0.02, s * 0.04), (-0.62, s * 0.36), mix(INK, [226.0, 230.0, 224.0], 0.4), 1.0);
        }
    }
    {
        for s in [-1.0f32, 1.0] { pen.path(&[(0.32, s * 0.05), (0.5, s * 0.16), (0.7, s * 0.3)], INK, 1.0); }
    }
    if l.has(part::MANDIBLES) { for s in [-1.0f32, 1.0] { pen.limb((0.33, s * 0.05), 0.035, (0.44, s * 0.01), 0.015, INK); } }
    if l.has(part::STINGER) { pen.poly(&[(-0.7, -0.05), (-0.92, 0.0), (-0.7, 0.05)], mix(l.coat, INK, 0.5)); }
    eyes(pen, l, 0.3, -0.05, 0.08);
}

fn crab(pen: &mut Pen, l: &Look, step: f32) {
    let leg_c = mix(l.coat, INK, 0.25);
    let w = (pen.half * 0.05).max(1.0);
    for side in [-1.0f32, 1.0] {
        for k in 0..3 {
            let wig = if (k % 2 == 0) == (side > 0.0) { step * 0.05 } else { -step * 0.05 };
            let root = (-0.05 - k as f32 * 0.14, side * 0.2);
            let knee = (root.0 - 0.06 + wig, side * 0.5);
            let foot = (knee.0 - 0.12 + wig, side * 0.72);
            pen.path(&[root, knee, foot], leg_c, w * 1.3);
        }
        // A claw forward on each side.
        let arm = (0.32, side * 0.36);
        pen.limb((0.1, side * 0.2), 0.06, arm, 0.05, l.coat);
        pen.ellipse(arm.0 + 0.12, arm.1, 0.15, 0.09, l.coat);
        pen.line((arm.0 + 0.14, arm.1), (arm.0 + 0.28, arm.1 - side * 0.04), INK, 1.0);
    }
    if l.has(part::STINGER) || l.has(part::TAIL) {
        let segs = [(-0.5, 0.0), (-0.7, 0.0), (-0.85, -0.05), (-0.9, -0.18)];
        for k in 0..segs.len() - 1 { pen.limb(segs[k], 0.08 - k as f32 * 0.015, segs[k + 1], 0.06 - k as f32 * 0.015, l.coat); }
        pen.poly(&[(-0.86, -0.18), (-0.84, -0.34), (-0.94, -0.2)], INK);
    }
    pen.ellipse(-0.08, 0.0, 0.36, 0.28, l.coat);
    pen.shape(l.pale, Finish::Paint, [-0.4, -0.25, 0.25, 0.25], &|u, v| ((u + 0.08) / 0.28).powi(2) + (v / 0.18).powi(2) < 1.0 && u > 0.0);
    if l.eyes != Eyes::None {
        for s in [-1.0f32, 1.0] { pen.line((0.22, s * 0.08), (0.34, s * 0.12), INK, 1.0); pen.dot(0.35, s * 0.12, l.glow.unwrap_or(INK)); }
    }
    if l.has(part::ANTENNAE) { for s in [-1.0f32, 1.0] { pen.path(&[(0.26, s * 0.03), (0.6, s * 0.12), (0.8, s * 0.05)], INK, 1.0); } }
}

fn lizard(pen: &mut Pen, l: &Look, step: f32) {
    let fat = l.quad.body_ry >= 0.4;
    let coat = l.coat;
    let leg = |pen: &mut Pen, u: f32, side: f32, fwd: f32| {
        let root = (u, side * if fat { 0.2 } else { 0.1 });
        let knee = (u + fwd * 0.12, side * if fat { 0.42 } else { 0.32 });
        let foot = (knee.0 + fwd * 0.08 + 0.04, side * if fat { 0.52 } else { 0.42 });
        pen.limb(root, 0.06, knee, 0.045, coat);
        pen.limb(knee, 0.045, foot, 0.04, coat);
        for t in [-0.05f32, 0.0, 0.05] { pen.line(foot, (foot.0 + 0.06, foot.1 + side * 0.03 + t), INK, 1.0); }
    };
    if l.has(part::WINGS) {
        for s in [-1.0f32, 1.0] {
            let lift = 0.1 * step;
            let pts = [(0.15, s * 0.08), (0.05, s * (0.85 + lift)), (-0.2, s * (0.7 + lift)), (-0.35, s * (0.75 + lift)), (-0.45, s * 0.45), (-0.3, s * 0.1)];
            let membrane = mix(l.accent, [236.0, 226.0, 204.0], 0.15);
            pen.poly(&pts, membrane);
            for t in &pts[1..5] { pen.line(pts[0], *t, mix(coat, INK, 0.4), 1.0); }
        }
    }
    leg(pen, 0.2, -1.0, step);
    leg(pen, 0.2, 1.0, -step);
    leg(pen, -0.25, -1.0, -step);
    leg(pen, -0.25, 1.0, step);
    // Tail: a tapering curve behind.
    let tail: Vec<(f32, f32)> = (0..7).map(|k| { let t = k as f32 / 6.0; (-0.35 - t * 0.6, (t * 3.0 + step * 0.5).sin() * 0.08 * t) }).collect();
    if !fat { for k in 0..6 { pen.limb(tail[k], 0.09 * (1.0 - k as f32 / 7.0), tail[k + 1], 0.09 * (1.0 - (k + 1) as f32 / 7.0), coat); } }
    pen.ellipse(-0.05, 0.0, 0.38, if fat { 0.32 } else { 0.15 }, coat);
    if l.has(part::SHAGGY) { for k in 0..6 { let u = -0.3 + k as f32 * 0.1; pen.dot(u, ((k * 5 % 3) as f32 - 1.0) * 0.08, mix(coat, INK, 0.4)); } }
    if l.has(part::SPINES) || l.has(part::FRILL) { for k in 0..5 { let u = -0.35 + k as f32 * 0.14; pen.poly(&[(u - 0.04, 0.0), (u, -0.12), (u + 0.04, 0.0)], l.accent); } }
    let snout = 0.16 * l.quad.snout / 0.6;
    pen.limb((0.3, 0.0), if fat { 0.16 } else { 0.1 }, (0.36 + snout, 0.0), 0.05, coat);
    if l.has(part::FRILL) { pen.ellipse(0.3, 0.0, 0.06, 0.2, l.accent); }
    if l.has(part::HORNS) { for s in [-1.0f32, 1.0] { pen.bone(&[(0.3, s * 0.07), (0.22, s * 0.16), (0.12, s * 0.17)], [228.0, 214.0, 186.0], (pen.half * 0.04).max(1.0)); } }
    if l.eyes != Eyes::None { for s in [-1.0f32, 1.0] { let mut lk = l.clone(); lk.eyes = Eyes::Two; eyes(pen, &lk, 0.38, s * 0.06, 0.04); } }
}

fn serpent(pen: &mut Pen, l: &Look, step: f32) {
    let n = 14;
    let pts: Vec<(f32, f32)> = (0..=n).map(|k| { let t = k as f32 / n as f32; (0.75 - t * 1.6, (t * 7.0 + step * 0.6).sin() * 0.22 * (0.4 + t * 0.6)) }).collect();
    let r = |k: usize| 0.1 * (1.0 - (k as f32 / n as f32).powf(1.6) * 0.85);
    for k in (0..n).rev() { pen.limb(pts[k], r(k), pts[k + 1], r(k + 1), l.coat); }
    for k in (1..n).step_by(2) { pen.ellipse_f(pts[k].0, pts[k].1, r(k) * 0.5, r(k) * 0.35, l.accent, Finish::Paint); }
    if l.has(part::WINGS) { for s in [-1.0f32, 1.0] { pen.poly(&[(0.4, 0.0), (0.2, s * 0.7), (-0.1, s * 0.55), (0.0, s * 0.1)], mix(l.accent, [236.0, 226.0, 204.0], 0.15)); } }
    if l.has(part::HOOD) { pen.ellipse(0.6, 0.0, 0.1, 0.22, l.coat); pen.ellipse_f(0.6, 0.0, 0.05, 0.12, l.pale, Finish::Paint); }
    pen.ellipse(0.82, 0.0, 0.14, 0.1, l.coat);
    if l.has(part::HORNS) { for s in [-1.0f32, 1.0] { pen.bone(&[(0.78, s * 0.06), (0.66, s * 0.18)], [228.0, 214.0, 186.0], 1.5); } }
    pen.path(&[(0.95, 0.0), (1.02, 0.0), (1.06, -0.03)], [170.0, 40.0, 40.0], 1.0);
    if l.eyes != Eyes::None { for s in [-1.0f32, 1.0] { let mut lk = l.clone(); lk.eyes = Eyes::Two; eyes(pen, &lk, 0.86, s * 0.05, 0.04); } }
}

fn worm(pen: &mut Pen, l: &Look, step: f32) {
    let n = if l.has(part::LONG_HIND) { 12 } else { 7 };
    let pts: Vec<(f32, f32)> = (0..n).map(|k| { let t = k as f32 / (n - 1) as f32; (0.6 - t * 1.3, (t * 4.0 + step * 0.7).sin() * 0.12) }).collect();
    let r = if l.len > 1.0 { 0.22 } else { 0.17 };
    for k in (0..n).rev() {
        let rr = r * (1.0 - k as f32 / n as f32 * 0.45);
        pen.ellipse(pts[k].0, pts[k].1, rr * 0.8, rr, mix(l.coat, l.pale, if k % 2 == 0 { 0.0 } else { 0.25 }));
        if l.has(part::LONG_HIND) { for s in [-1.0f32, 1.0] { pen.line(pts[k], (pts[k].0 - 0.05, pts[k].1 + s * (rr + 0.12)), INK, 1.0); } }
    }
    if l.has(part::SPINES) { for k in 1..n { pen.poly(&[(pts[k].0 - 0.04, pts[k].1 - r * 0.7), (pts[k].0, pts[k].1 - r * 1.5), (pts[k].0 + 0.04, pts[k].1 - r * 0.7)], l.accent); } }
    if l.has(part::MANDIBLES) { for s in [-1.0f32, 1.0] { pen.path(&[(0.7, s * 0.06), (0.86, s * 0.16), (0.92, s * 0.04)], INK, (pen.half * 0.05).max(1.0)); } }
    if l.has(part::TENTACLES) { for k in 0..4 { let a = -0.6 + k as f32 * 0.4; pen.path(&[(0.7, 0.0), (0.85, a * 0.3), (0.95, a * 0.5 + step * 0.05)], mix(l.coat, INK, 0.3), 1.5); } }
    if let Some(g) = l.glow { pen.glow(pts[n - 1].0, pts[n - 1].1, 0.35, g, 0.8); }
    if l.eyes != Eyes::None { eyes(pen, l, 0.66, -0.06, 0.06); }
}

fn bat(pen: &mut Pen, l: &Look, step: f32) {
    // Front on, wings open; the beat narrows them.
    let up = 0.15 * step;
    for s in [-1.0f32, 1.0] {
        let pts = [(s * 0.1, -0.1), (s * 0.45, -0.45 - up), (s * 0.9, -0.3 - up), (s * 0.85, 0.05), (s * 0.68, -0.02), (s * 0.55, 0.15), (s * 0.38, 0.05), (s * 0.22, 0.2), (s * 0.1, 0.1)];
        pen.poly(&pts, l.accent);
        pen.path(&[(s * 0.1, -0.1), (s * 0.45, -0.45 - up), (s * 0.9, -0.3 - up)], mix(l.coat, INK, 0.5), 1.0);
    }
    pen.ellipse(0.0, 0.05, 0.13, 0.2, l.coat);
    pen.ellipse(0.0, -0.2, 0.1, 0.09, l.coat);
    for s in [-1.0f32, 1.0] { pen.poly(&[(s * 0.03, -0.25), (s * 0.1, -0.38), (s * 0.1, -0.22)], l.coat); }
    if l.eyes != Eyes::None { for s in [-1.0f32, 1.0] { pen.dot(s * 0.04, -0.21, l.glow.unwrap_or([200.0, 60.0, 50.0])); } }
}

fn fish(pen: &mut Pen, l: &Look, step: f32) {
    pen.poly(&[(-0.45, 0.4), (-0.8, 0.22 + step * 0.05), (-0.8, 0.58 - step * 0.05)], mix(l.coat, INK, 0.15));
    pen.ellipse(0.0, 0.4, 0.5, 0.2, l.coat);
    pen.poly(&[(-0.05, 0.22), (0.15, 0.08), (0.2, 0.24)], mix(l.coat, INK, 0.15));
    pen.line((0.32, 0.3), (0.32, 0.5), INK, 1.0);
    if l.eyes != Eyes::None { pen.dot(0.38, 0.36, INK); }
}

fn bird(pen: &mut Pen, l: &Look, step: f32) {
    let g = 0.8;
    let w = (pen.half * 0.035).max(1.0);
    if !l.flies {
        pen.path(&[(-0.02, 0.42), (-0.02 + step * 0.06, g)], [180.0, 140.0, 70.0], w);
        pen.path(&[(0.06, 0.42), (0.06 - step * 0.06, g)], [180.0, 140.0, 70.0], w);
    }
    pen.poly(&[(-0.3, 0.22), (-0.62, 0.36), (-0.55, 0.42), (-0.28, 0.36)], mix(l.coat, INK, 0.25));
    pen.ellipse(0.0, 0.25, 0.32, 0.2, l.coat);
    if l.flies {
        let up = 0.25 * step;
        pen.poly(&[(0.05, 0.15), (-0.2, -0.45 - up), (-0.45, -0.3 - up), (-0.2, 0.18)], l.accent);
    } else {
        pen.ellipse_rot(-0.06, 0.24, 0.26, 0.12, 0.15, l.accent);
    }
    pen.ellipse(0.28, 0.02, 0.13, 0.12, mix(l.coat, l.pale, 0.2));
    pen.poly(&[(0.38, -0.02), (0.56, 0.04), (0.38, 0.07)], [200.0, 160.0, 70.0]);
    if l.has(part::CREST) || l.has(part::HORNS) { pen.poly(&[(0.2, -0.06), (0.24, -0.26), (0.32, -0.08)], l.accent); }
    eyes(pen, l, 0.32, -0.01, 0.08);
}

fn blob(pen: &mut Pen, l: &Look, step: f32) {
    if l.has(part::TENTACLES) {
        for k in 0..6 {
            let u = -0.5 + k as f32 * 0.2;
            let sway = ((k as f32 * 1.3) + step).sin() * 0.12;
            pen.limb((u, 0.45), 0.07, (u + sway, 0.8), 0.025, mix(l.coat, INK, 0.2));
        }
    }
    for (u, v, r) in [(-0.3, 0.38, 0.32), (0.25, 0.4, 0.34), (0.0, 0.12, 0.42), (-0.1, 0.5, 0.3), (0.35, 0.15, 0.22)] {
        pen.ellipse(u, v + step * 0.01, r, r * 0.9, l.coat);
    }
    pen.ellipse_f(0.0, 0.3, 0.45, 0.25, l.pale, Finish::Paint);
    if l.eyes == Eyes::None {
        // Mouths instead.
        for (u, v) in [(-0.15, 0.25), (0.2, 0.32), (0.0, 0.05)] { pen.ellipse_f(u, v, 0.08, 0.04, [70.0, 30.0, 30.0], Finish::Plain); }
    } else {
        eyes(pen, l, 0.05, 0.08, 0.2);
    }
}

/// A great two-legged thing (troll, giant, ogre, ape; the risen dead; a werebeast), front on.
fn giant(pen: &mut Pen, l: &Look, step: f32, pose: Pose) {
    let g = 0.8;
    let coat = l.coat;
    let fat = l.quad.body_ry >= 0.4;
    let beast = l.has(part::TAIL) || l.has(part::MANE);
    let dark = mix(coat, INK, 0.25);
    // Legs.
    pen.limb((-0.14, 0.3), 0.1, (-0.16 - step * 0.04, g), 0.08, dark);
    pen.limb((0.14, 0.3), 0.1, (0.16 + step * 0.04, g), 0.08, dark);
    if l.has(part::WINGS) { for s in [-1.0f32, 1.0] { pen.poly(&[(s * 0.15, -0.35), (s * 0.75, -0.75), (s * 0.95, -0.2), (s * 0.7, -0.05), (s * 0.55, 0.15), (s * 0.2, 0.05)], mix(l.accent, [236.0, 226.0, 204.0], 0.15)); } }
    if l.has(part::TAIL) { pen.limb((0.0, 0.3), 0.06, (0.45, 0.65), 0.03, coat); }
    // Body, hunched for a beast.
    pen.ellipse(0.0, 0.0, if fat { 0.4 } else { 0.3 }, 0.38, coat);
    if l.has(part::SHAGGY) { for k in 0..7 { let u = -0.24 + k as f32 * 0.08; pen.line((u, 0.25), (u + 0.01, 0.42), mix(coat, INK, 0.45), 1.0); } }
    // Arms, long for trolls and beasts; raised to strike.
    let reach = if l.quad.leg < 0.42 || beast { 0.62 } else { 0.42 };
    let strike = pose == Pose::Strike;
    for s in [-1.0f32, 1.0] {
        let hand = if strike && s > 0.0 { (0.5, -0.55) } else { (s * 0.42, -0.15 + reach + step * s * 0.05) };
        pen.limb((s * 0.26, -0.22), 0.09, hand, 0.07, coat);
        if beast { for t in [-0.04f32, 0.0, 0.04] { pen.line(hand, (hand.0 + s * 0.04 + t, hand.1 + 0.1), [236.0, 226.0, 204.0], 1.0); } }
    }
    // Head (a muzzle for a beast).
    let (hx, hy) = (0.0, -0.5);
    if l.has(part::MANE) { pen.ellipse(hx, hy + 0.05, 0.26, 0.24, l.accent); }
    pen.ellipse(hx, hy, 0.17, 0.17, coat);
    if beast {
        for s in [-1.0f32, 1.0] { pen.poly(&[(s * 0.06, hy - 0.12), (s * 0.18, hy - 0.34), (s * 0.18, hy - 0.08)], coat); }
        pen.ellipse(hx, hy + 0.1, 0.1, 0.08, mix(coat, INK, 0.2));
        pen.dot(hx, hy + 0.08, INK);
    }
    if l.has(part::HORNS) { for s in [-1.0f32, 1.0] { pen.bone(&[(s * 0.1, hy - 0.12), (s * 0.24, hy - 0.26), (s * 0.22, hy - 0.4)], [228.0, 214.0, 186.0], (pen.half * 0.05).max(1.0)); } }
    if l.has(part::TUSKS) { for s in [-1.0f32, 1.0] { pen.poly(&[(s * 0.06, hy + 0.06), (s * 0.09, hy - 0.06), (s * 0.12, hy + 0.06)], [234.0, 224.0, 196.0]); } }
    if l.has(part::TRUNK) { pen.limb((0.0, hy + 0.05), 0.04, (0.04, hy + 0.3), 0.03, coat); }
    // Eyes: two, unless the look says otherwise.
    match l.eyes {
        Eyes::Two => for s in [-1.0f32, 1.0] { let mut lk = l.clone(); lk.eyes = Eyes::Two; eyes(pen, &lk, s * 0.06, hy - 0.02, 0.05); },
        _ => eyes(pen, l, 0.0, hy - 0.02, 0.12),
    }
}

/// Whether a creature of this look is drawn bigger than its cell: its pixel size for a cell of
/// `t` pixels, at the figures' scale (`scale`, as the settlers'), so a deer stands a little
/// wider than a settler and a forgotten beast towers over them.
pub fn px_for(look: &Look, scale: f32) -> f32 { (look.len * 56.0 * scale).max(20.0) }

/// A legendary beast of the world at its lair, as the world map draws it.
pub struct WorldBeast { pub tile: (usize, usize), pub name: String, pub look: Look }

/// Every living legendary beast with a lair, drawn from what the history says it is
/// (`monsters::of_legend`, the same monster the colony meets).
pub fn world_beasts(h: &crate::history::world_state::WorldHistory) -> Vec<WorldBeast> {
    let mut v: Vec<WorldBeast> = h.legendary_creatures.values().filter(|c| c.is_alive())
        .filter_map(|c| c.lair_location.map(|tile| WorldBeast { tile, name: c.full_name(), look: of_monster(&crate::monsters::of_legend(h, c)) }))
        .collect();
    v.sort_by(|a, b| a.tile.cmp(&b.tile).then(a.name.cmp(&b.name)));
    v
}

/// Draw the world's beasts at their lairs (from 6 px a tile; named from 12 px), the map's width
/// wrapping east-west. `labels` gets the names to letter.
pub fn draw_world(beasts: &[WorldBeast], cam: &super::render::Camera, world_w: usize, buf: &mut [u32], w: usize, h: usize, named: bool) {
    if cam.tile_px < 6.0 { return; }
    let mut put = |x: i64, y: i64, c: Rgb, a: f32| {
        if x < 0 || y < 0 || x as usize >= w || y as usize >= h { return; }
        let k = y as usize * w + x as usize;
        let p = buf[k];
        let old = [((p >> 16) & 0xFF) as f32, ((p >> 8) & 0xFF) as f32, (p & 0xFF) as f32];
        buf[k] = super::ink::pack(mix(old, c, a.clamp(0.0, 1.0)));
    };
    let mut names = Vec::new();
    for b in beasts {
        let mut dx = b.tile.0 as f32 + 0.5 - cam.cx;
        let ww = world_w as f32;
        if dx > ww / 2.0 { dx -= ww; } else if dx < -ww / 2.0 { dx += ww; }
        let (sx, sy) = (dx * cam.tile_px + w as f32 / 2.0, (b.tile.1 as f32 + 0.5 - cam.cy) * cam.tile_px + h as f32 / 2.0);
        let px = (cam.tile_px * (1.6 + b.look.len * 0.5)).clamp(22.0, 96.0);
        if sx < -px || sy < -px || sx > w as f32 + px || sy > h as f32 + px { continue; }
        let left = (b.tile.0 + b.tile.1) % 2 == 0;
        draw(&mut put, &b.look, sx, sy + px * 0.3, px, left, Pose::Stand, 1.0);
        if named && cam.tile_px >= 12.0 { names.push((sx, sy + px * 0.3 - px * if b.look.flies { 1.3 } else { 1.0 }, b.name.clone())); }
    }
    for (x, y, n) in names {
        let tw = super::fonts::width(&n, super::fonts::Face::Italic, 13.0, 0.0);
        super::fonts::draw(buf, w, h, x - tw / 2.0, y - 16.0, &n, super::fonts::Face::Italic, 13.0, 0.0, 0x009A_2A1E, Some(0x00EE_E4CC));
    }
}

/// Where a four-legged body's neck meets its shoulders, in the sprite's unit box (for a pet's
/// collar); other bodies: just above the middle.
pub fn neck_point(look: &Look) -> (f32, f32) {
    if look.base != Base::Quad { return (0.1, -0.1); }
    let q = look.quad;
    let by = 0.8 - q.leg - q.body_ry * 0.55;
    let n0 = (-0.1 + q.body_rx * 0.72, by - q.body_ry * 0.25);
    let up = q.neck_up.to_radians();
    (n0.0 + up.cos() * q.neck * 0.45, n0.1 - up.sin() * q.neck * 0.45)
}
