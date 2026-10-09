//! The world's living things on the world map, beyond its towns and roads: monuments as what
//! they are (a statue, an obelisk, a tomb, a pyramid, a temple, a castle, a wall, a tower, a
//! bridge, a fountain, a memorial, a trophy, an altar), the war hosts of every war still fought
//! (armed figures under their people's banner at the war's latest battle), siege camps round
//! the towns still besieged, the outlaw bands' camps at their hideouts, caravans on the trade
//! routes still used, the creature populations of the wild as their species, and an altar by
//! the lair of a beast that a cult worships. (The history keeps no army records; its wars,
//! battles and sieges are where its armies are.) Computed once (`world_life`), drawn from 8 px a
//! tile (`draw`), the map's width wrapping east-west.

use super::beasts::{self, Look, Pose};
use super::ink::{mix, Finish, Pen, Rgb, INK};
use crate::history::world_state::WorldHistory;

#[derive(Clone, Debug)]
pub enum Kind {
    Monument(crate::history::objects::monuments::MonumentType),
    WarHost { field: Rgb, charge: Rgb, race: Option<crate::history::entities::races::RaceType> },
    Siege { field: Rgb, charge: Rgb },
    Outlaws,
    Caravan,
    Wild(Box<Look>),
    Cult,
}

#[derive(Clone, Debug)]
pub struct Thing {
    pub tile: (usize, usize),
    pub kind: Kind,
    pub name: String,
    /// Where in its tile it stands (several things can share a tile), in tiles.
    pub off: (f32, f32),
    /// The first thing on its tile (the one named on the map).
    pub first: bool,
}

/// A people's tinctures (as the band colours of `folk.rs`).
fn tinctures(h: &WorldHistory, f: crate::history::FactionId) -> (Rgb, Rgb) {
    super::folk::band_colours(&h.factions.get(&f).map(|x| x.name.clone()).unwrap_or_default())
}

/// The look of a species of the wild from its body (as `monsters::of_legend` reads a beast's).
pub fn of_species(sp: &crate::history::creatures::generator::CreatureSpecies) -> Look {
    use crate::history::creatures::anatomy::{BodyMaterial, BodyPartType};
    let count = |t: BodyPartType| sp.body_parts.iter().filter(|p| p.part_type == t).map(|p| p.count as u32).sum::<u32>();
    let has = |t: BodyPartType| count(t) > 0;
    let legs = count(BodyPartType::Legs);
    let base = match (legs, has(BodyPartType::Arms), has(BodyPartType::Tentacles), has(BodyPartType::Wings)) {
        (_, _, true, _) if legs == 0 => "blob",
        (0, _, _, _) => "serpent",
        (2, true, _, _) => "giant",
        (2, false, _, true) => "eagle",
        (6, _, _, _) => "cricket",
        (8, _, _, _) => "spider",
        _ => "",
    };
    let mut look = if base.is_empty() { beasts::of_name(&sp.name) } else { beasts::of_name(base) };
    if base == "giant" { look = beasts::of_name("restless dead"); look.glow = None; }
    if base == "blob" { look = beasts::of_name("things that hunt by sound"); }
    if has(BodyPartType::Wings) { look.parts |= beasts::part::WINGS; }
    if has(BodyPartType::Horns) && look.base == beasts::Base::Quad && look.quad.horn == beasts::Horn::None { look.quad.horn = beasts::Horn::Bovine; }
    if has(BodyPartType::Tail) { look.parts |= beasts::part::TAIL; }
    if has(BodyPartType::Mandibles) { look.parts |= beasts::part::MANDIBLES; }
    let coat = match sp.body_parts.first().map(|p| p.material) {
        Some(BodyMaterial::Chitin) => Some([96.0, 80.0, 60.0]), Some(BodyMaterial::Scales) => Some([100.0, 120.0, 80.0]), Some(BodyMaterial::Feathers) => Some([150.0, 120.0, 90.0]),
        Some(BodyMaterial::Stone) => Some([150.0, 146.0, 136.0]), Some(BodyMaterial::Metal) => Some([130.0, 132.0, 140.0]), Some(BodyMaterial::Crystal) => Some([170.0, 160.0, 210.0]),
        Some(BodyMaterial::Shadow) => Some([50.0, 46.0, 56.0]), Some(BodyMaterial::Flame) => Some([214.0, 110.0, 50.0]), Some(BodyMaterial::Ooze) => Some([120.0, 150.0, 80.0]),
        Some(BodyMaterial::Bone) => Some([226.0, 218.0, 196.0]), Some(BodyMaterial::Ice) => Some([190.0, 216.0, 230.0]), _ => None,
    };
    if let Some(c) = coat.filter(|_| !base.is_empty() || !sp.name.is_empty()) { if base != "" { look.coat = c; look.pale = mix(c, [236.0, 226.0, 204.0], 0.45); } }
    look
}

/// Everything to draw, once per world (or per shown season in the watcher).
pub fn world_life(h: &WorldHistory) -> Vec<Thing> {
    let mut v = Vec::new();
    for m in h.monuments.values() {
        v.push(Thing { tile: m.location, kind: Kind::Monument(m.monument_type.clone()), name: m.name.clone(), off: (0.0, 0.0), first: true });
    }
    for war in h.wars.values().filter(|w| w.is_active()) {
        let Some(at) = war.battles.iter().rev().chain(war.sieges.iter().rev()).find_map(|e| h.chronicle.get(*e).and_then(|e| e.location)) else { continue };
        let Some(&f) = war.aggressors.first() else { continue };
        let (field, charge) = tinctures(h, f);
        let race = h.factions.get(&f).and_then(|fc| h.races.get(&fc.race_id)).map(|r| r.base_type.clone());
        v.push(Thing { tile: at, kind: Kind::WarHost { field, charge, race }, name: format!("the host of {}", war.name), off: (0.0, 0.0), first: true });
    }
    for s in h.sieges.values().filter(|s| s.is_active()) {
        let Some(t) = h.settlements.get(&s.target) else { continue };
        let (field, charge) = tinctures(h, s.attacker);
        v.push(Thing { tile: t.location, kind: Kind::Siege { field, charge }, name: format!("the siege of {}", t.name), off: (0.0, 0.0), first: true });
    }
    for b in crate::history::bands::of(h) {
        v.push(Thing { tile: b.hideout, kind: Kind::Outlaws, name: b.name.clone(), off: (0.0, 0.0), first: true });
    }
    for r in h.trade_routes.values().filter(|r| r.is_active() && r.path.len() > 4) {
        let k = (r.id.0 as usize * 7 + 3) % r.path.len();
        v.push(Thing { tile: r.path[k], kind: Kind::Caravan, name: String::new(), off: (0.0, 0.0), first: true });
    }
    for p in h.populations.values().filter(|p| !p.is_extinct() && p.leader.is_none()) {
        let Some(sp) = h.creature_species.get(&p.species_id) else { continue };
        v.push(Thing { tile: p.location, kind: Kind::Wild(Box::new(of_species(sp))), name: sp.name.clone(), off: (0.0, 0.0), first: true });
    }
    for c in h.cults.values().filter(|c| c.is_active()) {
        let Some(b) = h.legendary_creatures.get(&c.worshipped_creature).filter(|b| b.is_alive()) else { continue };
        if let Some(l) = b.lair_location { v.push(Thing { tile: (l.0 + 1, l.1), kind: Kind::Cult, name: c.name.clone(), off: (0.0, 0.0), first: true }); }
    }
    // On a shared tile the living come first (a host, a siege, outlaws), then the rest.
    let rank = |k: &Kind| match k { Kind::WarHost { .. } => 0, Kind::Siege { .. } => 1, Kind::Outlaws => 2, Kind::Cult => 3, Kind::Caravan => 4, Kind::Wild(_) => 5, Kind::Monument(_) => 6 };
    v.sort_by(|a, b| (a.tile.1, a.tile.0, rank(&a.kind)).cmp(&(b.tile.1, b.tile.0, rank(&b.kind))).then(a.name.cmp(&b.name)));
    // Things sharing a tile stand round it (five at most; the rest are not drawn).
    const SPOTS: [(f32, f32); 5] = [(0.0, 0.0), (-0.7, 0.35), (0.7, 0.35), (-0.5, -0.55), (0.5, -0.55)];
    let mut out: Vec<Thing> = Vec::new();
    let (mut prev, mut k) = (None, 0usize);
    for mut t in v {
        if prev == Some(t.tile) { k += 1; } else { prev = Some(t.tile); k = 0; }
        if k >= SPOTS.len() { continue; }
        t.off = SPOTS[k];
        t.first = k == 0;
        out.push(t);
    }
    out
}

/// Draw the world's living things for camera `cam` (from 8 px a tile).
pub fn draw(things: &[Thing], cam: &super::render::Camera, world_w: usize, buf: &mut [u32], w: usize, h: usize) {
    if cam.tile_px < 8.0 { return; }
    let t = cam.tile_px;
    let mut put = |x: i64, y: i64, c: Rgb, a: f32| super::ui::blend_px(buf, w, h, x, y, super::ink::pack(c), a);
    // Monuments and the still things first, the living (hosts, sieges, outlaws, caravans) over them.
    let still = |k: &Kind| matches!(k, Kind::Monument(_) | Kind::Cult | Kind::Wild(_));
    let ordered = things.iter().filter(|t| still(&t.kind)).chain(things.iter().filter(|t| !still(&t.kind)));
    for th in ordered {
        let mut dx = th.tile.0 as f32 + 0.5 - cam.cx;
        let ww = world_w as f32;
        if dx > ww / 2.0 { dx -= ww; } else if dx < -ww / 2.0 { dx += ww; }
        let (sx, sy) = ((dx + th.off.0) * t + w as f32 / 2.0, (th.tile.1 as f32 + 0.5 + th.off.1 - cam.cy) * t + h as f32 / 2.0);
        let px = (t * 1.8).clamp(18.0, 64.0);
        if sx < -px || sy < -px || sx > w as f32 + px || sy > h as f32 + px { continue; }
        let scale = (t / 16.0).clamp(0.6, 1.6);
        match &th.kind {
            Kind::Monument(m) => { let mp = (t * 1.25).clamp(14.0, 44.0); monument(&mut Pen::new(&mut put, sx, sy - mp * 0.2, mp), m) }
            Kind::WarHost { field, charge, race } => {
                for (k, ox) in [(0u32, -0.55f32), (1, 0.0), (2, 0.55)] {
                    let mut f = super::folk::raider(None, "a war band of the host", k, None);
                    f.shield = Some((*field, *charge));
                    if matches!(race, Some(crate::history::entities::races::RaceType::Orc) | Some(crate::history::entities::races::RaceType::Goblin)) { f.skin = [118.0, 148.0, 94.0]; f.tusks = true; }
                    super::folk::draw(&mut put, &f, sx + ox * px * 0.5, sy + if k == 1 { -2.0 } else { 2.0 }, scale * 0.8, k == 2, false, 1.0);
                }
                let mut pen = Pen::new(&mut put, sx, sy, px);
                pen.bone(&[(0.0, 0.3), (0.0, -0.95)], [122.0, 86.0, 54.0], (scale * 1.5).max(1.0));
                pen.poly(&[(0.0, -0.95), (0.42, -0.85), (0.32, -0.72), (0.42, -0.6), (0.0, -0.6)], *field);
                pen.shape(*charge, Finish::Paint, [0.0, -0.95, 0.42, -0.6], &|u, v| (v + 0.775).abs() < 0.04 && u > 0.04 && u < 0.32);
            }
            Kind::Siege { field, charge } => {
                for (k, (ox, oy)) in [(-0.85f32, -0.45f32), (0.85, -0.3), (-0.6, 0.6), (0.7, 0.65)].iter().enumerate() {
                    let mut pen = Pen::new(&mut put, sx + ox * t * 0.8, sy + oy * t * 0.8, px * 0.5);
                    let cloth = if k % 2 == 0 { mix(*field, [220.0, 210.0, 190.0], 0.4) } else { [200.0, 188.0, 160.0] };
                    pen.poly(&[(-0.6, 0.5), (0.0, -0.5), (0.6, 0.5)], cloth);
                    pen.poly(&[(-0.1, 0.5), (0.0, 0.05), (0.1, 0.5)], [60.0, 50.0, 46.0]);
                    pen.bone(&[(0.0, -0.5), (0.0, -0.85)], [122.0, 86.0, 54.0], 1.0);
                    pen.poly(&[(0.0, -0.85), (0.35, -0.75), (0.0, -0.65)], *charge);
                }
                let mut pen = Pen::new(&mut put, sx, sy + t * 0.9, px * 0.4);
                pen.glow(0.0, 0.0, 0.9, [250.0, 170.0, 70.0], 0.5);
                pen.poly(&[(-0.2, 0.2), (0.0, -0.4), (0.2, 0.2)], [226.0, 120.0, 46.0]);
            }
            Kind::Outlaws => {
                let mut pen = Pen::new(&mut put, sx - px * 0.25, sy, px * 0.6);
                pen.poly(&[(-0.6, 0.5), (0.0, -0.45), (0.6, 0.5)], [130.0, 110.0, 80.0]);
                pen.poly(&[(-0.1, 0.5), (0.0, 0.1), (0.1, 0.5)], [50.0, 42.0, 38.0]);
                drop(pen);
                let f = super::folk::raider(None, "a band of outlaws", 1, None);
                super::folk::draw(&mut put, &f, sx + px * 0.25, sy + 2.0, scale * 0.75, true, false, 1.0);
            }
            Kind::Caravan => {
                let mule = beasts::of_name("mule");
                beasts::draw(&mut put, &mule, sx - px * 0.15, sy + px * 0.25, px * 0.7, false, Pose::Walk(true), 1.0);
                let mut pen = Pen::new(&mut put, sx - px * 0.15, sy + px * 0.25 - px * 0.7 * 0.42, px * 0.7);
                pen.rect(-0.38, -0.08, -0.02, 0.12, [196.0, 160.0, 96.0]);
                pen.rect(-0.02, -0.12, 0.26, 0.1, [170.0, 130.0, 80.0]);
                drop(pen);
                super::folk::draw(&mut put, &super::folk::trader("a caravan", 0), sx + px * 0.3, sy + 2.0, scale * 0.7, false, false, 1.0);
            }
            Kind::Wild(look) => {
                if t < 10.0 { continue; }
                let p = (px * 0.9 * look.len.sqrt()).clamp(16.0, 56.0);
                beasts::draw(&mut put, look, sx, sy + p * 0.3, p, (th.tile.0 + th.tile.1) % 2 == 0, Pose::Stand, 0.95);
            }
            Kind::Cult => {
                let mut pen = Pen::new(&mut put, sx, sy, px * 0.6);
                pen.rect(-0.45, -0.05, 0.45, 0.5, [150.0, 140.0, 130.0]);
                pen.rect(-0.55, -0.15, 0.55, -0.02, [176.0, 168.0, 156.0]);
                for u in [-0.3f32, 0.3] { pen.rect(u - 0.05, -0.45, u + 0.05, -0.15, [236.0, 226.0, 200.0]); pen.glow(u, -0.52, 0.18, [250.0, 190.0, 90.0], 0.8); }
                pen.ellipse_f(0.0, 0.2, 0.12, 0.1, [140.0, 30.0, 26.0], Finish::Paint);
            }
        }
    }
}

/// A monument as what it is, standing on its ground (unit box, base at v = 0.8).
pub fn monument(pen: &mut Pen, m: &crate::history::objects::monuments::MonumentType) {
    use crate::history::objects::monuments::MonumentType as M;
    let stone = [186.0, 178.0, 162.0];
    let gold = [214.0, 176.0, 70.0];
    pen.ground_shadow(0.0, 0.8, 0.55, 0.12);
    match m {
        M::Statue => { pen.rect(-0.3, 0.45, 0.3, 0.8, stone); pen.ellipse(0.0, -0.55, 0.14, 0.14, stone); pen.poly(&[(-0.2, 0.45), (-0.14, -0.38), (0.14, -0.38), (0.2, 0.45)], stone); pen.bone(&[(0.14, -0.3), (0.35, -0.75)], stone, 2.0); }
        M::Obelisk => { pen.poly(&[(-0.18, 0.8), (-0.12, -0.6), (0.0, -0.85), (0.12, -0.6), (0.18, 0.8)], stone); pen.rect(-0.3, 0.68, 0.3, 0.8, stone); }
        M::Tomb => { pen.rect(-0.55, 0.1, 0.55, 0.8, stone); pen.poly(&[(-0.65, 0.12), (0.0, -0.35), (0.65, 0.12)], mix(stone, INK, 0.1)); pen.rect_f(-0.15, 0.4, 0.15, 0.8, [60.0, 52.0, 48.0], Finish::Plain); }
        M::Pyramid => { pen.poly(&[(-0.75, 0.8), (0.0, -0.6), (0.75, 0.8)], [206.0, 186.0, 140.0]); for k in 1..4 { let v = -0.6 + k as f32 * 0.35; let hw = (v + 0.6) / 1.4 * 0.75; pen.line_a((-hw, v), (hw, v), INK, 1.0, 0.5); } }
        M::Temple => { pen.rect(-0.6, 0.65, 0.6, 0.8, stone); for u in [-0.45f32, -0.15, 0.15, 0.45] { pen.rect(u - 0.06, -0.15, u + 0.06, 0.65, stone); } pen.poly(&[(-0.68, -0.15), (0.0, -0.55), (0.68, -0.15)], stone); }
        M::Castle => { pen.rect(-0.55, -0.1, 0.55, 0.8, stone); for u in [-0.55f32, 0.4] { pen.rect(u, -0.45, u + 0.15, 0.8, stone); } for k in 0..5 { let u = -0.4 + k as f32 * 0.2; pen.rect(u, -0.2, u + 0.1, -0.1, stone); } pen.rect_f(-0.12, 0.45, 0.12, 0.8, [60.0, 52.0, 48.0], Finish::Plain); }
        M::Wall => { pen.rect(-0.8, 0.2, 0.8, 0.8, stone); for k in 0..8 { let u = -0.8 + k as f32 * 0.2; pen.rect(u, 0.05, u + 0.1, 0.2, stone); } }
        M::Tower => { pen.rect(-0.22, -0.55, 0.22, 0.8, stone); for k in 0..3 { let u = -0.22 + k as f32 * 0.16; pen.rect(u, -0.7, u + 0.1, -0.55, stone); } pen.rect_f(-0.06, -0.3, 0.06, -0.15, [60.0, 52.0, 48.0], Finish::Plain); }
        M::Bridge => { pen.shape(stone, Finish::Inked, [-0.8, 0.0, 0.8, 0.8], &|u, v| v >= 0.0 && v <= 0.8 && !((u / 0.5).powi(2) + ((v - 0.8) / 0.55).powi(2) < 1.0)); pen.rect_f(-0.8, 0.75, 0.8, 0.8, [130.0, 160.0, 176.0], Finish::Plain); }
        M::Fountain => { pen.ellipse(0.0, 0.62, 0.55, 0.2, stone); pen.ellipse_f(0.0, 0.6, 0.42, 0.13, [130.0, 160.0, 176.0], Finish::Plain); pen.rect(-0.06, -0.1, 0.06, 0.6, stone); for s in [-1.0f32, 1.0] { pen.path(&[(0.0, -0.15), (s * 0.25, -0.35), (s * 0.4, 0.4)], [130.0, 170.0, 200.0], 1.5); } }
        M::Memorial => { pen.shape(stone, Finish::Inked, [-0.35, -0.5, 0.35, 0.8], &|u, v| u.abs() <= 0.3 && v <= 0.8 && (v >= -0.2 || (u / 0.3).powi(2) + ((v + 0.2) / 0.3).powi(2) <= 1.0)); for k in 0..3 { let v = 0.0 + k as f32 * 0.2; pen.line_a((-0.18, v), (0.18, v), INK, 1.0, 0.5); } }
        M::Trophy => { pen.rect(-0.1, -0.2, 0.1, 0.8, [122.0, 86.0, 54.0]); pen.ellipse(0.0, -0.45, 0.28, 0.24, [232.0, 224.0, 204.0]); for s in [-1.0f32, 1.0] { pen.bone(&[(s * 0.2, -0.55), (s * 0.45, -0.85)], [232.0, 224.0, 204.0], 2.0); } }
        M::Altar => { pen.rect(-0.45, 0.2, 0.45, 0.8, stone); pen.rect(-0.55, 0.1, 0.55, 0.22, stone); pen.glow(0.0, -0.05, 0.3, [250.0, 190.0, 90.0], 0.8); pen.poly_f(&[(-0.06, 0.1), (0.0, -0.2), (0.06, 0.1)], gold, Finish::Plain); }
    }
}
