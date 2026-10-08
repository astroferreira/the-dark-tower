//! Furniture and fittings below the ground, seen from above in the ink hand: beds with blankets
//! in their owners' colours (a straw pallet before the bed is made), the great hall's long table
//! with its benches, bowls and cups, stone coffins in the tombs, the workshops' benches and tools,
//! fungus beds, the mason's block and chisels, the carpenter's bench and saw, the smelter's
//! furnace with bellows and ore, the kiln's dome with pots, the forge's hearth, anvil and
//! quenching tub, the cellar's casks and sacks, the hatch, a lair's bones and hoard, an old
//! mine's ore cart, and each artifact on its plinth, shaped as what it is.
//!
//! Every function draws at a map cell with a pen whose unit is one cell (`camp_ink::Cells`).

use super::ink::{mix, Finish, Pen, Rgb, INK};

const WOOD: Rgb = [164.0, 120.0, 78.0];
const DARK_WOOD: Rgb = [122.0, 86.0, 54.0];
const STONE: Rgb = [178.0, 172.0, 160.0];
const IRON: Rgb = [112.0, 114.0, 122.0];
const GLOW: Rgb = [240.0, 140.0, 56.0];
const LINEN: Rgb = [236.0, 228.0, 210.0];

/// A bed: frame, mattress, blanket in `blanket`, pillow; head to the north.
pub fn bed(pen: &mut Pen, blanket: Rgb, fine: bool) {
    pen.rect(0.12, 0.06, 0.88, 0.96, if fine { mix(WOOD, [200.0, 160.0, 90.0], 0.3) } else { WOOD });
    pen.rect_f(0.18, 0.12, 0.82, 0.9, LINEN, Finish::Plain);
    pen.rect(0.2, 0.14, 0.8, 0.32, LINEN);
    pen.rect(0.16, 0.4, 0.84, 0.92, blanket);
    pen.line_a((0.16, 0.5), (0.84, 0.5), INK, 1.0, 0.5);
    if fine { pen.ellipse(0.2, 0.08, 0.06, 0.06, [214.0, 176.0, 70.0]); pen.ellipse(0.8, 0.08, 0.06, 0.06, [214.0, 176.0, 70.0]); }
}

/// A straw pallet on the floor.
pub fn pallet(pen: &mut Pen) {
    pen.ellipse(0.5, 0.55, 0.36, 0.3, [204.0, 178.0, 110.0]);
    for k in 0..6 { let u = 0.22 + k as f32 * 0.11; pen.line_a((u, 0.38), (u + 0.06, 0.72), [150.0, 124.0, 60.0], 1.0, 0.8); }
}

/// The great hall's long table (5 cells from `u0`) with a bench along each side, bowls and cups.
pub fn long_table(pen: &mut Pen, fine: bool) {
    let top = if fine { mix(WOOD, [200.0, 160.0, 90.0], 0.25) } else { WOOD };
    pen.rect(0.3, -0.3, 4.7, -0.08, DARK_WOOD);
    pen.rect(0.3, 1.08, 4.7, 1.3, DARK_WOOD);
    pen.rect(0.1, 0.15, 4.9, 0.85, top);
    for k in 0..4 { let u = 0.6 + k as f32 * 1.25; pen.line_a((u + 0.6, 0.15), (u + 0.6, 0.85), INK, 1.0, 0.35); }
    for k in 0..5 {
        let u = 0.55 + k as f32 * 0.95;
        pen.ellipse(u, 0.32, 0.13, 0.09, [214.0, 200.0, 170.0]);
        pen.ellipse_f(u, 0.32, 0.07, 0.04, [150.0, 110.0, 70.0], Finish::Paint);
        pen.ellipse(u + 0.35, 0.68, 0.07, 0.07, [150.0, 104.0, 64.0]);
    }
}

/// A stone coffin with a cross cut in its lid; an empty niche is a bare ledge.
pub fn coffin(pen: &mut Pen, filled: bool) {
    if !filled { pen.rect_f(0.18, 0.4, 0.82, 0.6, mix(STONE, INK, 0.2), Finish::Plain); return; }
    pen.poly(&[(0.3, 0.08), (0.7, 0.08), (0.82, 0.3), (0.74, 0.94), (0.26, 0.94), (0.18, 0.3)], mix(STONE, INK, 0.1));
    pen.poly(&[(0.34, 0.14), (0.66, 0.14), (0.74, 0.32), (0.68, 0.88), (0.32, 0.88), (0.26, 0.32)], STONE);
    pen.line((0.5, 0.24), (0.5, 0.74), INK, 1.0);
    pen.line((0.36, 0.38), (0.64, 0.38), INK, 1.0);
}

/// A heavy bench with tools laid on it, 2 cells long from the pen's origin.
pub fn bench(pen: &mut Pen, tools: bool) {
    pen.rect(0.05, 0.22, 1.95, 0.78, WOOD);
    pen.line_a((0.05, 0.5), (1.95, 0.5), INK, 1.0, 0.3);
    if tools {
        pen.bone(&[(0.4, 0.62), (0.75, 0.38)], DARK_WOOD, (pen.half * 0.04).max(1.0));
        pen.rect(0.66, 0.3, 0.86, 0.42, IRON);
        pen.bone(&[(1.2, 0.36), (1.65, 0.62)], IRON, (pen.half * 0.03).max(1.0));
        pen.ellipse(1.55, 0.35, 0.07, 0.07, DARK_WOOD);
    }
}

/// A bed of cave fungus: dark soil with pale caps (`n` of 3 grown).
pub fn fungus_bed(pen: &mut Pen, grown: usize) {
    pen.rect_f(0.08, 0.2, 0.92, 0.8, [92.0, 76.0, 66.0], Finish::Plain);
    for k in 0..grown.min(3) {
        let u = 0.25 + k as f32 * 0.25;
        pen.rect_f(u - 0.025, 0.5, u + 0.025, 0.66, [226.0, 214.0, 190.0], Finish::Plain);
        pen.ellipse(u, 0.46, 0.13, 0.1, [160.0, 112.0, 160.0]);
    }
}

/// The mason's: a bench with a chisel and mallet, and a stack of dressed blocks beside it.
pub fn mason(pen: &mut Pen) {
    pen.rect(-0.9, 0.25, 0.9, 0.75, STONE);
    pen.bone(&[(-0.4, 0.55), (-0.1, 0.35)], IRON, (pen.half * 0.03).max(1.0));
    pen.rect(0.15, 0.32, 0.45, 0.5, DARK_WOOD);
    for (bx, by) in [(1.15f32, 0.2f32), (1.6, 0.2), (1.38, -0.18)] { pen.rect(bx, by, bx + 0.4, by + 0.36, mix(STONE, [250.0, 244.0, 230.0], 0.15)); }
    for k in 0..5 { pen.dot(-0.6 + k as f32 * 0.31, 0.95 + (k % 2) as f32 * 0.06, mix(STONE, INK, 0.3)); }
}

/// The carpenter's: a bench with a saw and planks, a barrel, shavings.
pub fn carpenter(pen: &mut Pen) {
    pen.rect(-0.9, 0.25, 0.9, 0.75, WOOD);
    pen.poly(&[(-0.5, 0.4), (0.2, 0.36), (0.2, 0.5), (-0.5, 0.52)], [200.0, 202.0, 206.0]);
    pen.rect(0.2, 0.38, 0.42, 0.5, DARK_WOOD);
    for k in 0..3 { let v = -0.15 - k as f32 * 0.12; pen.rect(-0.8, v - 0.05, 0.6, v + 0.05, mix(WOOD, [236.0, 226.0, 204.0], 0.2)); }
    barrel(pen, 1.5, 0.5, 0.32);
    for k in 0..4 { let u = -0.7 + k as f32 * 0.45; pen.path(&[(u, 0.95), (u + 0.08, 0.88), (u + 0.12, 0.97)], [196.0, 160.0, 100.0], 1.0); }
}

/// A cask seen from above at (u, v): staves and hoops.
pub fn barrel(pen: &mut Pen, u: f32, v: f32, r: f32) {
    pen.ellipse(u, v, r, r, [160.0, 110.0, 64.0]);
    pen.ellipse_f(u, v, r * 0.72, r * 0.72, [176.0, 128.0, 80.0], Finish::Plain);
    pen.ellipse_f(u, v, r * 0.12, r * 0.12, INK, Finish::Paint);
}

/// The smelter: a round furnace with a glowing mouth, bellows, a heap of ore, bars cooling.
pub fn smelter(pen: &mut Pen, metal: Rgb) {
    pen.glow(0.5, 0.5, 1.2, GLOW, 0.3);
    pen.ellipse(0.5, 0.5, 0.75, 0.75, [92.0, 84.0, 80.0]);
    pen.ellipse(0.5, 0.5, 0.48, 0.48, [120.0, 110.0, 104.0]);
    pen.ellipse_f(0.5, 0.5, 0.26, 0.26, GLOW, Finish::Plain);
    pen.ellipse_f(0.5, 0.5, 0.12, 0.12, [252.0, 214.0, 120.0], Finish::Paint);
    pen.poly(&[(-0.55, 0.3), (-0.05, 0.42), (-0.05, 0.58), (-0.55, 0.7)], [150.0, 108.0, 72.0]);
    for (u, v) in [(1.4f32, 0.2f32), (1.6, 0.35), (1.45, 0.45), (1.7, 0.15)] { pen.ellipse(u, v, 0.11, 0.09, [120.0, 104.0, 92.0]); pen.dot(u, v, metal); }
    for k in 0..2 { pen.rect(1.25 + k as f32 * 0.32, 0.75, 1.5 + k as f32 * 0.32, 0.88, metal); }
}

/// The kiln: a dome of red clay with its glowing mouth, fired pots beside it.
pub fn kiln(pen: &mut Pen) {
    pen.glow(0.5, 0.6, 1.0, GLOW, 0.25);
    pen.ellipse(0.5, 0.5, 0.72, 0.68, [176.0, 96.0, 70.0]);
    for k in 0..3 { let r = 0.2 + k as f32 * 0.17; pen.line_a((0.5 - r, 0.5 - r * 0.4), (0.5 + r, 0.5 - r * 0.4), INK, 1.0, 0.3); }
    pen.ellipse_f(0.5, 0.95, 0.22, 0.12, GLOW, Finish::Plain);
    for (u, v) in [(1.45f32, 0.3f32), (1.7, 0.55), (1.4, 0.75)] {
        pen.ellipse(u, v, 0.14, 0.14, [190.0, 120.0, 84.0]);
        pen.ellipse_f(u, v, 0.06, 0.06, [90.0, 60.0, 50.0], Finish::Paint);
    }
}

/// The forge: a hearth of stone with live coals, an anvil on its block, a quenching tub.
pub fn forge(pen: &mut Pen, magma: bool) {
    pen.glow(-0.4, 0.5, 1.1, GLOW, 0.3);
    pen.rect(-0.9, 0.08, 0.12, 0.92, [92.0, 84.0, 80.0]);
    pen.rect_f(-0.68, 0.3, -0.1, 0.7, if magma { [220.0, 90.0, 40.0] } else { [70.0, 50.0, 44.0] }, Finish::Plain);
    for (u, v) in [(-0.55f32, 0.4f32), (-0.3, 0.55), (-0.45, 0.6), (-0.22, 0.4)] { pen.dot(u, v, [252.0, 190.0, 90.0]); }
    pen.rect(0.95, 0.5, 1.3, 0.9, DARK_WOOD);
    pen.poly(&[(0.6, 0.32), (1.62, 0.32), (1.5, 0.52), (0.82, 0.52)], IRON);
    pen.poly(&[(0.6, 0.32), (0.48, 0.38), (0.6, 0.44)], IRON);
    barrel(pen, 1.75, 1.0, 0.24);
    pen.ellipse_f(1.75, 1.0, 0.16, 0.16, [80.0, 100.0, 120.0], Finish::Paint);
    pen.bone(&[(1.0, 0.18), (1.35, -0.05)], DARK_WOOD, (pen.half * 0.04).max(1.0));
    pen.rect(1.3, -0.14, 1.48, 0.0, IRON);
}

/// The cellar's stores at a cell: casks and a sack or two.
pub fn cellar_stores(pen: &mut Pen, k: usize) {
    match k % 3 {
        0 => { barrel(pen, 0.3, 0.32, 0.22); barrel(pen, 0.72, 0.36, 0.22); barrel(pen, 0.5, 0.72, 0.22); }
        1 => { pen.ellipse(0.4, 0.55, 0.26, 0.3, [196.0, 168.0, 116.0]); pen.rect(0.34, 0.18, 0.46, 0.28, [196.0, 168.0, 116.0]); barrel(pen, 0.78, 0.4, 0.18); }
        _ => { pen.rect(0.15, 0.25, 0.85, 0.75, [150.0, 110.0, 70.0]); pen.line((0.15, 0.25), (0.85, 0.75), INK, 1.0); pen.line((0.85, 0.25), (0.15, 0.75), INK, 1.0); }
    }
}

/// The hatch over the stair: planks bound with iron, an iron ring.
pub fn hatch(pen: &mut Pen) {
    pen.rect(0.05, 0.05, 0.95, 0.95, [140.0, 104.0, 68.0]);
    for k in 1..4 { let u = k as f32 * 0.225 + 0.05; pen.line_a((u, 0.05), (u, 0.95), INK, 1.0, 0.4); }
    for v in [0.3, 0.7] { pen.rect_f(0.05, v - 0.05, 0.95, v + 0.05, IRON, Finish::Plain); }
    pen.ellipse_f(0.5, 0.5, 0.12, 0.12, IRON, Finish::Plain);
    pen.ellipse_f(0.5, 0.5, 0.07, 0.07, [140.0, 104.0, 68.0], Finish::Paint);
}

/// A lair at the end of a place in the hills: bones and a glinting hoard.
pub fn lair(pen: &mut Pen) {
    let bone = [232.0, 224.0, 204.0];
    for k in 0..5 { let a = k as f32 * 1.3; pen.bone(&[(0.5 + a.cos() * 0.15, 0.5 + a.sin() * 0.12), (0.5 + a.cos() * 0.42, 0.5 + a.sin() * 0.36)], bone, (pen.half * 0.04).max(1.0)); }
    pen.ellipse(0.25, 0.3, 0.12, 0.1, bone);
    for (u, v) in [(0.6f32, 0.6f32), (0.7, 0.55), (0.65, 0.68), (0.55, 0.7), (0.75, 0.66)] { pen.ellipse(u, v, 0.06, 0.05, [214.0, 176.0, 70.0]); }
    pen.ellipse(0.68, 0.45, 0.05, 0.07, [60.0, 120.0, 170.0]);
}

/// An old mine's ore cart on its rails.
pub fn ore_cart(pen: &mut Pen) {
    pen.line((0.3, -0.2), (0.3, 1.2), IRON, 1.0);
    pen.line((0.7, -0.2), (0.7, 1.2), IRON, 1.0);
    pen.rect(0.2, 0.25, 0.8, 0.8, [110.0, 90.0, 70.0]);
    for (u, v) in [(0.38f32, 0.42f32), (0.6, 0.45), (0.5, 0.62)] { pen.ellipse(u, v, 0.1, 0.08, [120.0, 104.0, 92.0]); }
}

/// An artifact on a gilded plinth, shaped as what it is (a figurine, a chest, an instrument, a
/// cup or crown or ring, a weapon, a book, else a carved stone).
pub fn artifact(pen: &mut Pen, title: &str) {
    let t = title.to_lowercase();
    pen.rect(0.12, 0.12, 0.88, 0.88, [200.0, 170.0, 90.0]);
    pen.rect_f(0.18, 0.18, 0.82, 0.82, [222.0, 196.0, 120.0], Finish::Plain);
    pen.glow(0.5, 0.5, 0.5, [255.0, 230.0, 160.0], 0.3);
    let has = |k: &str| t.contains(k);
    let gem = [130.0, 80.0, 160.0];
    if has("figurine") || has("statue") || has("idol") {
        pen.ellipse(0.5, 0.32, 0.1, 0.1, [190.0, 180.0, 170.0]);
        pen.poly(&[(0.38, 0.75), (0.42, 0.42), (0.58, 0.42), (0.62, 0.75)], [190.0, 180.0, 170.0]);
    } else if has("chest") || has("box") || has("coffer") {
        pen.rect(0.26, 0.38, 0.74, 0.72, DARK_WOOD);
        pen.rect(0.26, 0.3, 0.74, 0.42, WOOD);
        pen.rect_f(0.46, 0.44, 0.54, 0.54, [214.0, 176.0, 70.0], Finish::Plain);
    } else if has("pipe") || has("flute") || has("horn") {
        for k in 0..4 { let u = 0.32 + k as f32 * 0.11; pen.rect(u, 0.3, u + 0.08, 0.72 - k as f32 * 0.07, WOOD); }
    } else if has("drum") {
        pen.ellipse(0.5, 0.5, 0.26, 0.22, [230.0, 214.0, 180.0]); pen.ellipse_f(0.5, 0.5, 0.2, 0.16, [210.0, 190.0, 150.0], Finish::Plain);
    } else if has("harp") || has("lyre") || has("lute") || has("fiddle") {
        pen.bone(&[(0.35, 0.75), (0.35, 0.28), (0.65, 0.4), (0.62, 0.75), (0.35, 0.75)], WOOD, (pen.half * 0.04).max(1.0));
        for k in 0..3 { let u = 0.42 + k as f32 * 0.07; pen.line_a((u, 0.35), (u, 0.73), INK, 1.0, 0.6); }
    } else if has("crown") || has("circlet") {
        pen.poly(&[(0.28, 0.65), (0.28, 0.38), (0.39, 0.52), (0.5, 0.32), (0.61, 0.52), (0.72, 0.38), (0.72, 0.65)], [214.0, 176.0, 70.0]);
        pen.dot(0.5, 0.56, gem);
    } else if has("ring") || has("amulet") || has("bracelet") || has("necklace") {
        pen.ellipse(0.5, 0.52, 0.2, 0.2, [214.0, 176.0, 70.0]); pen.ellipse_f(0.5, 0.52, 0.11, 0.11, [222.0, 196.0, 120.0], Finish::Plain);
        pen.ellipse(0.5, 0.32, 0.07, 0.07, gem);
    } else if has("cup") || has("goblet") || has("chalice") || has("bowl") || has("mug") {
        pen.ellipse(0.5, 0.4, 0.2, 0.08, [214.0, 176.0, 70.0]);
        pen.poly(&[(0.3, 0.4), (0.7, 0.4), (0.56, 0.6), (0.44, 0.6)], [214.0, 176.0, 70.0]);
        pen.rect(0.46, 0.58, 0.54, 0.72, [214.0, 176.0, 70.0]);
    } else if has("sword") || has("axe") || has("spear") || has("mace") || has("hammer") || has("dagger") {
        pen.bone(&[(0.3, 0.75), (0.7, 0.25)], [200.0, 202.0, 210.0], (pen.half * 0.06).max(1.5));
        pen.line((0.3, 0.55), (0.48, 0.73), INK, 1.5);
    } else if has("book") || has("tome") || has("scroll") {
        pen.rect(0.28, 0.3, 0.72, 0.72, [130.0, 58.0, 46.0]); pen.line((0.5, 0.3), (0.5, 0.72), INK, 1.0);
    } else {
        pen.poly(&[(0.32, 0.72), (0.3, 0.42), (0.45, 0.28), (0.66, 0.34), (0.7, 0.72)], [176.0, 170.0, 162.0]);
        pen.dot(0.5, 0.5, gem);
    }
}
