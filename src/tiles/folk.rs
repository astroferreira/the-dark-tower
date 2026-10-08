//! People who are not the camp's: raiders and besiegers, outlaws, the Shadow's soldiers, traders
//! with their pack beasts, the risen dead in the clothes they were buried in. Drawn as the
//! settlers are (`local_ink::draw_figure`: an inked head and shoulders about a settler's size)
//! so a stranger stands among them in the same hand, but armed and marked: a helm by their kind,
//! a weapon raised to strike, a shield in their band's colours, their race's skin and tusks.

use super::ink::{mix, Finish, Pen, Rgb, INK};
use crate::history::entities::races::RaceType;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Helm { None, Hood, Cap, Nasal, Horned, Crown, Skull }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arm { None, Spear, Sword, Axe, Club, Bow, Staff, Pack, Torch }

#[derive(Clone, Debug)]
pub struct Folk {
    pub skin: Rgb,
    pub hair: Rgb,
    pub dress: Rgb,
    pub helm: Helm,
    pub arm: Arm,
    /// A shield: field and charge colours.
    pub shield: Option<(Rgb, Rgb)>,
    pub tusks: bool,
    pub pointed: bool,
    pub beard: bool,
    /// Glowing eyes (the Shadow's, the risen dead's).
    pub glow: Option<Rgb>,
    /// Drawn in a faded line (the dead, ghosts).
    pub pale: bool,
}

fn h(s: &str, salt: u64) -> u64 { s.bytes().fold(0xcbf2_9ce4_8422_2325u64 ^ salt, |a, b| (a ^ b as u64).wrapping_mul(0x100_0000_01b3)) }

/// Muted heraldic tinctures for a band's shields (field, charge), from its name.
pub fn band_colours(name: &str) -> (Rgb, Rgb) {
    const FIELDS: [Rgb; 6] = [[150.0, 52.0, 44.0], [62.0, 84.0, 128.0], [70.0, 110.0, 70.0], [96.0, 64.0, 110.0], [52.0, 48.0, 46.0], [170.0, 120.0, 50.0]];
    const METALS: [Rgb; 2] = [[220.0, 186.0, 90.0], [226.0, 224.0, 214.0]];
    let k = h(name, 7);
    (FIELDS[(k % 6) as usize], METALS[((k >> 8) % 2) as usize])
}

/// Skin, tusks, pointed ears and beard for a people's race.
fn race_looks(race: Option<RaceType>, k: u64) -> (Rgb, bool, bool, bool) {
    match race {
        Some(RaceType::Orc) => ([118.0, 148.0, 94.0], true, false, false),
        Some(RaceType::Goblin) => ([150.0, 160.0, 100.0], false, true, false),
        Some(RaceType::Elf) | Some(RaceType::Fey) => ([234.0, 218.0, 198.0], false, true, false),
        Some(RaceType::Dwarf) => ([214.0, 170.0, 136.0], false, false, true),
        Some(RaceType::Undead) => ([196.0, 200.0, 184.0], false, false, false),
        Some(RaceType::Reptilian) => ([150.0, 168.0, 118.0], false, false, false),
        Some(RaceType::Giant) => ([200.0, 170.0, 140.0], false, false, true),
        _ => ([[232.0, 196.0, 164.0], [210.0, 170.0, 136.0], [176.0, 130.0, 96.0], [130.0, 92.0, 66.0]][(k % 4) as usize], false, false, k % 3 == 0),
    }
}

/// An attacker of the threat `t` (the `n`th of its band): the Shadow's soldiers in black with
/// horned helms and red eyes, a war band helmed with spears and shields in its colours, outlaws
/// hooded with clubs and bows.
pub fn raider(t: Option<&crate::colony::arc::Threat>, name: &str, n: u32, history: Option<&crate::history::world_state::WorldHistory>) -> Folk {
    use crate::colony::arc::ThreatKind;
    let race = t.and_then(|t| t.faction).and_then(|f| history.and_then(|h| h.factions.get(&f)))
        .and_then(|f| history.and_then(|h| h.races.get(&f.race_id))).map(|r| r.base_type.clone());
    let k = h(name, n as u64);
    let (skin, tusks, pointed, beard) = race_looks(race.clone(), k);
    let hairs: [Rgb; 4] = [[40.0, 30.0, 24.0], [96.0, 62.0, 36.0], [140.0, 50.0, 30.0], [70.0, 52.0, 40.0]];
    let kind = t.map(|t| t.kind);
    let shadow = kind == Some(ThreatKind::Shadow) || name.contains("Shadow");
    let outlaws = kind == Some(ThreatKind::Outlaws) || name.starts_with("a band of") || name.contains("outlaws") || name.contains("deserters");
    let (field, metal) = band_colours(t.map(|t| t.name.as_str()).unwrap_or(name));
    if shadow {
        return Folk { skin, hair: [40.0, 34.0, 32.0], dress: [58.0, 50.0, 52.0], helm: Helm::Horned, arm: if n % 3 == 2 { Arm::Axe } else { Arm::Sword },
            shield: Some(([44.0, 40.0, 40.0], [170.0, 40.0, 30.0])), tusks, pointed, beard: false, glow: Some([230.0, 70.0, 40.0]), pale: false };
    }
    if outlaws {
        return Folk { skin, hair: hairs[(k % 4) as usize], dress: [[110.0, 96.0, 70.0], [92.0, 104.0, 74.0], [120.0, 84.0, 60.0]][(k % 3) as usize],
            helm: Helm::Hood, arm: [Arm::Club, Arm::Bow, Arm::Axe][(n % 3) as usize], shield: None, tusks, pointed, beard, glow: None, pale: false };
    }
    let undead = race == Some(RaceType::Undead);
    Folk { skin, hair: hairs[(k % 4) as usize], dress: mix(field, [120.0, 110.0, 96.0], 0.35), helm: if undead { Helm::Skull } else if tusks { Helm::Horned } else { Helm::Nasal },
        arm: if n % 4 == 3 { Arm::Axe } else { Arm::Spear }, shield: Some((field, metal)), tusks, pointed, beard, glow: if undead { Some([150.0, 210.0, 190.0]) } else { None }, pale: undead }
}

/// A trader of a caravan: a travelling coat, a staff, a pack.
pub fn trader(name: &str, n: u32) -> Folk {
    let k = h(name, 31 + n as u64);
    let (skin, _, pointed, beard) = race_looks(None, k);
    Folk { skin, hair: [[96.0, 62.0, 36.0], [40.0, 30.0, 24.0], [170.0, 120.0, 60.0]][(k % 3) as usize], dress: [[128.0, 96.0, 60.0], [100.0, 90.0, 120.0], [96.0, 120.0, 100.0]][(k >> 4) as usize % 3],
        helm: if k % 2 == 0 { Helm::Cap } else { Helm::None }, arm: if n == 0 { Arm::Staff } else { Arm::Pack }, shield: None, tusks: false, pointed, beard, glow: None, pale: false }
}

/// Draw `f` standing at (x, y) (as `draw_figure` places a settler), `scale` as the settlers', facing
/// left or right; `strike` raises the weapon. Returns the top of the head.
pub fn draw(put: &mut dyn FnMut(i64, i64, Rgb, f32), f: &Folk, x: f32, y: f32, scale: f32, left: bool, strike: bool, alpha: f32) -> f32 {
    // The settlers' bust is ~14 x 18 px at scale 1: a unit box of 22 px holds it.
    let size = 22.0 * scale;
    let mut pen = Pen::new(put, x, y - 2.0 * scale, size).facing_left(left).faint(alpha);
    if f.pale { pen.ink = mix(INK, [150.0, 160.0, 150.0], 0.45); pen.tint = Some(([200.0, 214.0, 206.0], 0.3)); }
    let w = (scale * 1.4).max(1.0);
    let haft: Rgb = [120.0, 84.0, 50.0];
    let steel: Rgb = [176.0, 178.0, 184.0];
    // The weapon behind the shoulder when held up, in front when it strikes.
    let weapon = |pen: &mut Pen, strike: bool| {
        let (a, b) = if strike { ((0.55, 0.15), (0.95, -0.75)) } else { ((0.62, 0.55), (0.62, -0.85)) };
        match f.arm {
            Arm::None | Arm::Pack => {}
            Arm::Spear => { pen.bone(&[a, b], haft, w); let d = (b.0 - a.0, b.1 - a.1); let l = (d.0 * d.0 + d.1 * d.1).sqrt(); let (ux, uy) = (d.0 / l, d.1 / l);
                pen.poly(&[(b.0 - uy * 0.07, b.1 + ux * 0.07), (b.0 + ux * 0.22, b.1 + uy * 0.22), (b.0 + uy * 0.07, b.1 - ux * 0.07)], steel); }
            Arm::Sword => { let mid = (a.0 + (b.0 - a.0) * 0.35, a.1 + (b.1 - a.1) * 0.35); pen.bone(&[mid, b], steel, w * 1.6); pen.bone(&[a, mid], haft, w); pen.line((mid.0 - 0.1, mid.1), (mid.0 + 0.1, mid.1), INK, w); }
            Arm::Axe => { let top = (a.0 + (b.0 - a.0) * 0.7, a.1 + (b.1 - a.1) * 0.7); pen.bone(&[a, top], haft, w); pen.poly(&[(top.0, top.1 - 0.05), (top.0 + 0.22, top.1 - 0.16), (top.0 + 0.24, top.1 + 0.1), (top.0, top.1 + 0.06)], steel); }
            Arm::Club => { let top = (a.0 + (b.0 - a.0) * 0.65, a.1 + (b.1 - a.1) * 0.65); pen.limb(a, 0.035, top, 0.08, haft); }
            Arm::Bow => { let c = (0.62, -0.15); pen.path(&[(c.0 - 0.05, c.1 - 0.55), (c.0 + 0.1, c.1 - 0.25), (c.0 + 0.12, c.1), (c.0 + 0.1, c.1 + 0.25), (c.0 - 0.05, c.1 + 0.55)], haft, w * 1.3); pen.line((c.0 - 0.05, c.1 - 0.55), (c.0 - 0.05, c.1 + 0.55), [220.0, 210.0, 190.0], 1.0); }
            Arm::Staff => pen.bone(&[(0.6, 0.62), (0.6, -0.8)], haft, w),
            Arm::Torch => { pen.bone(&[(0.6, 0.5), (0.62, -0.5)], haft, w); pen.glow(0.62, -0.62, 0.35, [250.0, 180.0, 80.0], 0.8); pen.ellipse_f(0.62, -0.6, 0.06, 0.1, [250.0, 210.0, 110.0], Finish::Plain); }
        }
    };
    if !strike { weapon(&mut pen, false); }
    // A pack on the back.
    if f.arm == Arm::Pack || f.arm == Arm::Staff { pen.rect(-0.62, -0.05, -0.18, 0.5, [176.0, 140.0, 90.0]); pen.line((-0.62, 0.15), (-0.18, 0.15), INK, 1.0); }
    // Shoulders.
    pen.shape(f.dress, Finish::Inked, [-0.66, -0.05, 0.66, 0.5], &|u, v| v <= 0.5 && (u / 0.64).powi(2) + ((v - 0.5) / 0.55).powi(2) <= 1.0);
    // Head.
    let (hx, hy, hr) = (0.0, -0.32, 0.38);
    if f.pointed { for s in [-1.0f32, 1.0] { pen.poly(&[(s * 0.3, -0.36), (s * 0.58, -0.56), (s * 0.34, -0.22)], f.skin); } }
    pen.ellipse(hx, hy, hr, hr, f.skin);
    // Hair, a beard, tusks.
    if f.helm == Helm::None || f.helm == Helm::Crown { pen.shape(f.hair, Finish::Paint, [-hr, hy - hr, hr, hy], &|u, v| (u / (hr - 0.06)).powi(2) + ((v - hy) / (hr - 0.06)).powi(2) <= 1.0 && v < hy - 0.12); }
    if f.beard { pen.shape(f.hair, Finish::Inked, [-0.24, hy, 0.24, hy + 0.52], &|u, v| v > hy + 0.08 && (u / 0.24).powi(2) + ((v - hy - 0.12) / 0.4).powi(2) <= 1.0); }
    if f.tusks { for s in [-1.0f32, 1.0] { pen.poly(&[(s * 0.1, hy + 0.18), (s * 0.15, hy + 0.02), (s * 0.2, hy + 0.2)], [234.0, 224.0, 196.0]); } }
    // Eyes.
    for s in [-1.0f32, 1.0] {
        let (ex, ey) = (s * 0.13, hy - 0.02);
        match f.glow { Some(g) => { pen.glow(ex, ey, 0.14, g, 0.7); pen.dot(ex, ey, g); } None => if scale >= 0.8 { pen.dot(ex, ey, INK); } }
    }
    // Helm.
    let iron: Rgb = [150.0, 150.0, 156.0];
    match f.helm {
        Helm::None => {}
        Helm::Hood => pen.shape(f.dress, Finish::Inked, [-hr - 0.08, hy - hr - 0.12, hr + 0.08, hy + 0.3], &move |u, v| {
            let outer = (u / (hr + 0.08)).powi(2) + ((v - hy + 0.02) / (hr + 0.12)).powi(2) <= 1.0;
            let face = (u / (hr - 0.1)).powi(2) + ((v - hy - 0.06) / (hr - 0.1)).powi(2) <= 1.0 && v > hy - 0.14;
            outer && !face }),
        Helm::Cap => pen.shape(mix(f.dress, INK, 0.2), Finish::Inked, [-hr, hy - hr - 0.06, hr, hy - 0.08], &move |u, v| v < hy - 0.1 && (u / hr).powi(2) + ((v - hy) / (hr + 0.04)).powi(2) <= 1.0),
        Helm::Nasal | Helm::Horned | Helm::Skull => {
            let c = if f.helm == Helm::Skull { [222.0, 214.0, 192.0] } else { iron };
            pen.shape(c, Finish::Inked, [-hr - 0.04, hy - hr - 0.06, hr + 0.04, hy - 0.04], &move |u, v| v < hy - 0.06 && (u / (hr + 0.04)).powi(2) + ((v - hy) / (hr + 0.06)).powi(2) <= 1.0);
            if f.helm == Helm::Nasal { pen.rect_f(-0.04, hy - 0.1, 0.04, hy + 0.12, c, Finish::Plain); }
            if f.helm == Helm::Horned { for s in [-1.0f32, 1.0] { pen.bone(&[(s * 0.3, hy - 0.2), (s * 0.5, hy - 0.4), (s * 0.46, hy - 0.62)], [228.0, 214.0, 186.0], w * 1.4); } }
            if f.helm == Helm::Skull { for s in [-1.0f32, 1.0] { pen.ellipse_f(s * 0.13, hy - 0.16, 0.07, 0.06, INK, Finish::Paint); } }
        }
        Helm::Crown => { for k in 0..3 { let u = -0.2 + k as f32 * 0.2; pen.poly(&[(u - 0.08, hy - 0.24), (u, hy - 0.46), (u + 0.08, hy - 0.24)], [214.0, 176.0, 70.0]); } pen.rect(-0.3, hy - 0.3, 0.3, hy - 0.2, [214.0, 176.0, 70.0]); }
    }
    // The shield in front of the near shoulder, in the band's colours: a stripe or a roundel.
    if let Some((field, charge)) = f.shield {
        let (sx, sy) = (-0.32, 0.26);
        pen.shape(field, Finish::Inked, [sx - 0.3, sy - 0.32, sx + 0.3, sy + 0.4], &move |u, v| {
            let (a, b) = (u - sx, v - sy);
            a.abs() <= 0.28 && b >= -0.3 && (b <= 0.08 || (a / 0.28).powi(2) + ((b - 0.08) / 0.3).powi(2) <= 1.0)
        });
        pen.shape(charge, Finish::Paint, [sx - 0.3, sy - 0.32, sx + 0.3, sy + 0.4], &move |u, v| {
            let (a, b) = (u - sx, v - sy);
            (a.abs() < 0.06 && b > -0.24 && b < 0.3) || (b.abs() < 0.05 && a.abs() < 0.2)
        });
    }
    if strike { weapon(&mut pen, true); }
    let (_, top) = pen.at(0.0, hy - hr);
    top
}
