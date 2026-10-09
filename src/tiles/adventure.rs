//! Adventure mode's window: the place the adventurer is in, drawn as an inked plan with its light
//! and fog (`adventure_ink`), the adventurer and everything living on it, the log, a parchment
//! panel (life, mana and experience, what they wear and carry, skills, spells, quests), the
//! townsfolk's talk as a card of choices, the world map for travel.
//!
//! Keys: arrows / WASD / numpad walk (Q E Z C diagonals), bump to strike, open, talk; Space strikes
//! the target (Tab picks it), F1-F9 cast spells, G takes what lies here, R rests, H and J drink,
//! F eats, < > or Enter take stairs and ways out, 1-9 answer, Esc closes. The mouse: click to walk
//! there or strike, click a thing in the pack to use or wear it (right click drops it).

use super::adventure_ink::{self as ai, PARCH};
use super::fonts::{self, Face};
use super::folk::{self, Arm, Folk, Helm};
use super::ink::{mix, pack, Rgb, INK};
use super::ui::{card, Rect};
use crate::adventure::game::{Effect, Game, Tone};
use crate::adventure::hero::{Skill, Slot};
use crate::adventure::map::{Feature, Wall};
use crate::adventure::site::SiteKind;
use crate::adventure::Action;
use minifb::{Key, KeyRepeat, MouseButton, MouseMode, Window, WindowOptions};
use std::collections::HashMap;
use std::time::{Duration, Instant};

const PANEL_W: usize = 300;
const DESK: u32 = 0x0026_201B;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab { Pack, Skills, Spells, Quests }

/// What the window keeps between frames.
pub struct View {
    /// Pixels a cell.
    pub cs: f32,
    /// Where things are drawn (eased toward where they are).
    pos: HashMap<u32, (f32, f32)>,
    hero_pos: (f32, f32),
    cam: (f32, f32),
    last_floor: (Option<u32>, usize),
    plan: Option<ai::Plan>,
    effects: Vec<(Effect, Instant)>,
    pub tab: Tab,
    pub target: Option<u32>,
    /// Click-to-walk: the cell walked to.
    walk_to: Option<(i32, i32)>,
    last_step: Instant,
    /// Clickable areas: pack items, equipment slots, talk choices, tabs, spells.
    hits: Vec<(Rect, Hit)>,
    pub mouse: (f32, f32),
    world_cam: super::render::Camera,
    last_frame: Instant,
}

#[derive(Clone, Copy, PartialEq)]
enum Hit { Pack(usize), Slot(Slot), Choice(usize), Tab(Tab), Spell(usize), Chest(usize) }

impl View {
    pub fn new(g: &Game) -> View {
        View { cs: 32.0, pos: HashMap::new(), hero_pos: (g.x as f32, g.y as f32), cam: (g.x as f32, g.y as f32), last_floor: (g.here, g.z), plan: None, effects: Vec::new(),
            tab: Tab::Pack, target: None, walk_to: None, last_step: Instant::now(), hits: Vec::new(), mouse: (-1.0, -1.0),
            world_cam: super::render::Camera { cx: g.tile.0 as f32 + 0.5, cy: g.tile.1 as f32 + 0.5, tile_px: 24.0 }, last_frame: Instant::now() }
    }
}

fn hexc(c: Rgb) -> u32 { pack(c) }

fn put_into<'a>(buf: &'a mut [u32], w: usize, h: usize, clip: Option<Rect>) -> impl FnMut(i64, i64, Rgb, f32) + 'a {
    move |x: i64, y: i64, c: Rgb, a: f32| {
        if x < 0 || y < 0 || x as usize >= w || y as usize >= h { return; }
        if let Some(r) = clip { if !r.contains(x as f32, y as f32) { return; } }
        let k = y as usize * w + x as usize;
        let p = buf[k];
        let old = [((p >> 16) & 255) as f32, ((p >> 8) & 255) as f32, (p & 255) as f32];
        buf[k] = pack(mix(old, c, a.clamp(0.0, 1.0)));
    }
}

// ---------------------------------------------------------------------------------------------
// Figures

fn race_skin(race: &str, k: u64) -> (Rgb, bool, bool, bool) {
    match race {
        "orc" => ([118.0, 148.0, 94.0], true, false, false),
        "goblin" => ([150.0, 160.0, 100.0], false, true, false),
        "elf" | "fey" => ([234.0, 218.0, 198.0], false, true, false),
        "dwarf" => ([214.0, 170.0, 136.0], false, false, true),
        "undead" => ([196.0, 200.0, 184.0], false, false, false),
        "giant" => ([170.0, 150.0, 130.0], false, false, true),
        "shadow" => ([60.0, 54.0, 58.0], false, false, false),
        _ => ([[232.0, 196.0, 164.0], [210.0, 170.0, 136.0], [176.0, 130.0, 96.0], [130.0, 92.0, 66.0]][(k % 4) as usize], false, false, k % 3 == 0),
    }
}

fn helm_of(s: &str) -> Helm { match s { "hood" => Helm::Hood, "cap" => Helm::Cap, "nasal" => Helm::Nasal, "horned" => Helm::Horned, "crown" => Helm::Crown, "skull" => Helm::Skull, _ => Helm::None } }
fn arm_of(s: &str) -> Arm { match s { "spear" => Arm::Spear, "sword" => Arm::Sword, "axe" => Arm::Axe, "club" => Arm::Club, "bow" => Arm::Bow, "staff" => Arm::Staff, "torch" => Arm::Torch, _ => Arm::None } }

/// A person of the bestiary ("folk:orc:horned:axe").
fn folk_of_look(look: &str, k: u64) -> Folk {
    let p: Vec<&str> = look.split(':').collect();
    let race = p.get(1).copied().unwrap_or("human");
    let (skin, tusks, pointed, beard) = race_skin(race, k);
    let dress: Rgb = match race { "orc" => [110.0, 96.0, 70.0], "goblin" => [92.0, 104.0, 74.0], "undead" => [150.0, 146.0, 132.0], "shadow" => [58.0, 50.0, 52.0], "giant" => [120.0, 100.0, 80.0], _ => [[120.0, 84.0, 60.0], [96.0, 90.0, 120.0], [96.0, 116.0, 92.0]][(k % 3) as usize] };
    let undead = race == "undead";
    Folk { skin, hair: [70.0, 52.0, 40.0], dress, helm: helm_of(p.get(2).copied().unwrap_or("none")), arm: arm_of(p.get(3).copied().unwrap_or("none")),
        shield: None, tusks, pointed, beard, glow: if undead { Some([150.0, 210.0, 190.0]) } else if race == "shadow" { Some([230.0, 70.0, 40.0]) } else { None }, pale: undead }
}

/// The adventurer as they are dressed.
fn hero_folk(g: &Game) -> Folk {
    let h = &g.hero;
    let (_, tusks, pointed, _) = race_skin(&h.race, 1);
    let eq = |s: Slot| h.equipped[s as usize].as_ref();
    let helm = match eq(Slot::Head).map(|i| i.id.as_str()) { Some("leather_helmet") => Helm::Cap, Some("viking_helmet") => Helm::Horned, Some("chain_helmet") | Some("steel_helmet") => Helm::Nasal, Some(_) => Helm::Nasal, None => if h.calling.as_deref() == Some("sorcerer") || h.calling.as_deref() == Some("druid") { Helm::Hood } else { Helm::None } };
    let arm = match eq(Slot::Hand).map(|i| (i.def().skill.clone(), i.def().kind.clone(), i.id.clone())) {
        Some((_, k, _)) if k == "wand" => Arm::Staff,
        Some((Some(s), _, id)) => match s.as_str() { "sword" => Arm::Sword, "axe" => Arm::Axe, "club" => Arm::Club, "distance" => if id.contains("bow") { Arm::Bow } else { Arm::Spear }, _ => Arm::None },
        _ => Arm::None,
    };
    let dress = match eq(Slot::Body).map(|i| (i.def().glyph.clone(), i.material.clone())) {
        Some((g, m)) if g == "Mail" => { let c = m.as_deref().and_then(|m| crate::adventure::data::data().material(m)).map(|m| [m.colour[0] as f32, m.colour[1] as f32, m.colour[2] as f32]).unwrap_or([150.0, 150.0, 156.0]); c }
        Some((_, Some(m))) if m == "leather" => [140.0, 96.0, 60.0],
        _ => match h.calling.as_deref() { Some("sorcerer") => [110.0, 60.0, 120.0], Some("druid") => [80.0, 120.0, 70.0], Some("paladin") => [180.0, 160.0, 110.0], _ => [150.0, 130.0, 100.0] },
    };
    let shield = eq(Slot::Shield).map(|s| { let c = s.material.as_deref().and_then(|m| crate::adventure::data::data().material(m)).map(|m| [m.colour[0] as f32, m.colour[1] as f32, m.colour[2] as f32]).unwrap_or([150.0, 104.0, 62.0]); (c, [214.0, 172.0, 70.0]) });
    Folk { skin: h.skin, hair: h.hair, dress, helm, arm, shield, tusks, pointed, beard: h.beard, glow: None, pale: false }
}

fn npc_folk(n: &crate::adventure::actor::Npc, k: u64) -> Folk {
    use crate::adventure::actor::Role;
    let (skin, tusks, pointed, beard) = race_skin(&n.race, k);
    let (helm, arm, dress): (Helm, Arm, Rgb) = match n.role {
        Role::Priest => (Helm::Hood, Arm::Staff, [230.0, 226.0, 210.0]), Role::Smith => (Helm::None, Arm::Club, [100.0, 80.0, 64.0]),
        Role::Trader => (Helm::Cap, Arm::Pack, [128.0, 96.0, 60.0]), Role::Innkeeper => (Helm::None, Arm::None, [150.0, 110.0, 80.0]),
        Role::Lord => (Helm::Crown, Arm::Sword, [140.0, 40.0, 50.0]), Role::Guard => (Helm::Nasal, Arm::Spear, [90.0, 100.0, 130.0]),
        Role::Sage => (Helm::Hood, Arm::Staff, [70.0, 70.0, 120.0]), Role::Townsfolk => (Helm::None, Arm::None, [[120.0, 110.0, 90.0], [100.0, 120.0, 100.0], [140.0, 110.0, 110.0]][(k % 3) as usize]),
    };
    Folk { skin, hair: [[96.0, 62.0, 36.0], [40.0, 30.0, 24.0], [170.0, 120.0, 60.0], [200.0, 200.0, 200.0]][(k >> 3) as usize % 4], dress, helm, arm, shield: None, tusks, pointed, beard: beard && !n.female, glow: None, pale: false }
}

fn draw_monster(put: &mut dyn FnMut(i64, i64, Rgb, f32), m: &crate::adventure::actor::Monster, x: f32, y: f32, cs: f32, strike: bool, alpha: f32) {
    let d = m.def();
    let scale = m.scale.sqrt();
    if d.look.starts_with("folk") {
        let f = folk_of_look(&d.look, m.uid as u64);
        let s = cs / 22.0 * 1.25 * scale * if d.look.contains(":giant:") { 1.25 } else { 1.0 };
        folk::draw(put, &f, x, y + cs * 0.32, s, m.left, strike, alpha);
    } else {
        let look = match &m.legend { Some(l) => super::beasts::of_monster(l), None => super::beasts::of_name(&d.look) };
        let px = (super::beasts::px_for(&look, 1.0) * cs / 40.0).clamp(cs * 0.5, cs * 2.4) * scale;
        let pose = if strike { super::beasts::Pose::Strike } else { super::beasts::Pose::Stand };
        super::beasts::draw(put, &look, x, y + cs * 0.15, px, m.left, pose, alpha);
    }
}

// ---------------------------------------------------------------------------------------------
// The place

fn tone_colour(t: Tone) -> u32 {
    match t { Tone::Info => 0x0038_2A20, Tone::Hit => 0x0030_4A2A, Tone::Hurt => 0x009A_2A1E, Tone::Loot => 0x0080_5A10, Tone::Level => 0x0020_5A7A, Tone::Talk => 0x0046_3A6E, Tone::Quest => 0x00A8_6A10, Tone::Danger => 0x00B0_2010, Tone::Death => 0x0090_1010 }
}

/// Draw the whole window.
pub fn draw(g: &Game, v: &mut View, buf: &mut [u32], w: usize, h: usize) {
    let now = Instant::now();
    let dt = now.duration_since(v.last_frame).as_secs_f32().min(0.1);
    v.last_frame = now;
    v.hits.clear();
    for e in std::mem::take(&mut v.effects).into_iter() { v.effects.push(e); }
    let map_w = w.saturating_sub(PANEL_W);
    if g.here.is_some() { draw_place(g, v, buf, w, h, map_w, dt); } else { draw_world(g, v, buf, w, h, map_w); }
    draw_panel(g, v, buf, w, h, map_w);
    draw_log(g, buf, w, h, map_w);
    if g.talk.is_some() { draw_talk(g, v, buf, w, h, map_w); }
    else { draw_chest_choice(g, v, buf, w, h, map_w); }
    if let Some((title, text)) = &g.banner {
        let r = Rect { x: map_w / 2 - 220, y: h / 2 - 60, w: 440, h: 110 };
        card(buf, w, r);
        fonts::draw(buf, w, h, r.x as f32 + 24.0, r.y as f32 + 18.0, title, Face::SmallCaps, 28.0, 1.0, 0x0090_1010, None);
        for (k, l) in fonts::wrap(text, Face::Italic, 16.0, r.w as f32 - 48.0).iter().enumerate() { fonts::draw(buf, w, h, r.x as f32 + 24.0, r.y as f32 + 58.0 + k as f32 * 19.0, l, Face::Italic, 16.0, 0.0, 0x0038_2A20, None); }
    }
}

fn draw_place(g: &Game, v: &mut View, buf: &mut [u32], w: usize, h: usize, map_w: usize, dt: f32) {
    let p = g.place().unwrap();
    let f = &p.floors[g.z];
    let cs = v.cs;
    let id = g.here.unwrap();
    // A new floor: things are drawn where they are.
    if v.last_floor != (g.here, g.z) {
        v.last_floor = (g.here, g.z);
        v.pos.clear();
        v.hero_pos = (g.x as f32, g.y as f32);
        v.cam = v.hero_pos;
        v.walk_to = None;
    }
    // Ease the drawn positions toward the true ones (a step in ~90 ms).
    let ease = |cur: (f32, f32), to: (f32, f32)| -> (f32, f32) {
        let (dx, dy) = (to.0 - cur.0, to.1 - cur.1);
        let d = (dx * dx + dy * dy).sqrt();
        if d > 3.0 || d < 0.01 { return to; }
        let step = (dt * 11.0).min(d);
        (cur.0 + dx / d * step, cur.1 + dy / d * step)
    };
    v.hero_pos = ease(v.hero_pos, (g.x as f32, g.y as f32));
    for m in p.monsters.iter().filter(|m| m.z == g.z) { let e = v.pos.entry(m.uid).or_insert((m.x as f32, m.y as f32)); *e = ease(*e, (m.x as f32, m.y as f32)); }
    v.cam = v.hero_pos;
    // The screen's top-left in world pixels (whole pixels).
    let ox = ((v.cam.0 + 0.5) * cs - map_w as f32 / 2.0).round() as i64;
    let oy = ((v.cam.1 + 0.5) * cs - h as f32 / 2.0).round() as i64;
    // Bring the plan up to date for the cells in view.
    let key = (id, g.z, cs.to_bits());
    if v.plan.as_ref().map_or(true, |pl| pl.key != key) { v.plan = Some(ai::Plan::new(key, f, cs)); }
    let plan = v.plan.as_mut().unwrap();
    let (cx0, cy0) = ((ox as f32 / cs).floor() as i32 - 1, (oy as f32 / cs).floor() as i32 - 1);
    let (cx1, cy1) = (((ox + map_w as i64) as f32 / cs).ceil() as i32 + 1, ((oy + h as i64) as f32 / cs).ceil() as i32 + 1);
    plan.refresh(f, p.spec.kind, cs, cx0, cy0, cx1, cy1);
    // Copy it with the light and fog: unseen underground is dark, unseen outdoors parchment;
    // seen but out of sight faded; in sight lit by the adventurer's light.
    let outdoor = f.outdoor;
    let light = if outdoor { 14.0 } else { g.hero.light() as f32 + 1.5 };
    let (hx, hy) = (v.hero_pos.0 + 0.5, v.hero_pos.1 + 0.5);
    let unseen = if outdoor { hexc(mix(PARCH, [214.0, 200.0, 170.0], 0.6)) } else { DESK };
    let fw = f.w;
    {
        use rayon::prelude::*;
        let plan = v.plan.as_ref().unwrap();
        let sight = &g.sight;
        buf.par_chunks_mut(w).enumerate().for_each(|(sy, row)| {
            let wy = oy + sy as i64;
            for sx in 0..map_w {
                let wx = ox + sx as i64;
                if wx < 0 || wy < 0 || wx as usize >= plan.w || wy as usize >= plan.h { row[sx] = unseen; continue; }
                let (cx, cy) = ((wx as f32 / cs) as usize, (wy as f32 / cs) as usize);
                let k = cy * fw + cx;
                if k >= f.seen.len() || !f.seen[k] { row[sx] = unseen; continue; }
                let p = plan.buf[wy as usize * plan.w + wx as usize];
                let c = [((p >> 16) & 255) as f32, ((p >> 8) & 255) as f32, (p & 255) as f32];
                let lit = sight.get(k).copied().unwrap_or(false);
                let c = if !lit {
                    let grey = (c[0] + c[1] + c[2]) / 3.0;
                    mix(mix(c, [grey, grey, grey], 0.6), if outdoor { PARCH } else { [70.0, 60.0, 52.0] }, if outdoor { 0.35 } else { 0.5 })
                } else if outdoor { c } else {
                    let (dx, dy) = (wx as f32 / cs - hx, wy as f32 / cs - hy);
                    let d = (dx * dx + dy * dy).sqrt() / light;
                    let dim = (d * d).clamp(0.0, 1.0) * 0.6;
                    mix(mix(c, [255.0, 226.0, 170.0], 0.06 * (1.0 - d).max(0.0)), [60.0, 50.0, 44.0], dim)
                };
                row[sx] = pack(c);
            }
        });
    }
    let clip = Rect { x: 0, y: 0, w: map_w, h };
    let to_screen = |x: f32, y: f32| ((x + 0.5) * cs - ox as f32, (y + 0.5) * cs - oy as f32);
    let vis = |x: i32, y: i32| g.visible(x, y);
    let mut put = put_into(buf, w, h, Some(clip));
    // Corpses and things lying about (in sight).
    if let Some(cs_list) = g.corpses.get(&id) {
        for c in cs_list.iter().filter(|c| c.z == g.z && vis(c.x, c.y)) {
            let (x, y) = to_screen(c.x as f32, c.y as f32);
            let mut pen = super::ink::Pen::new(&mut put, x, y, cs);
            pen.ellipse_f(0.05, 0.25, 0.55, 0.3, [130.0, 40.0, 34.0], super::ink::Finish::Paint);
            super::glyphs::draw(&mut put, super::glyphs::Glyph::Bone, x - cs * 0.15, y + cs * 0.1, cs * 0.45, None);
        }
    }
    for ((ix, iy), items) in f.items.iter() {
        if items.is_empty() || !vis(*ix, *iy) { continue; }
        let (x, y) = to_screen(*ix as f32, *iy as f32);
        let it = &items[0];
        let tint = it.material.as_deref().and_then(|m| crate::adventure::data::data().material(m)).filter(|_| matches!(it.def().kind.as_str(), "weapon" | "armour" | "shield")).map(|m| [m.colour[0] as f32, m.colour[1] as f32, m.colour[2] as f32]);
        super::glyphs::draw(&mut put, ai::glyph(&it.def().glyph), x, y + cs * 0.1, cs * 0.6, tint);
        if items.len() > 1 { super::glyphs::draw(&mut put, ai::glyph(&items[1].def().glyph), x + cs * 0.22, y - cs * 0.12, cs * 0.45, None); }
    }
    // People of the town.
    let mut labels: Vec<(f32, f32, String, u32)> = Vec::new();
    for (k, n) in p.npcs.iter().enumerate().filter(|(_, n)| n.z == g.z && vis(n.x, n.y)) {
        let (x, y) = to_screen(n.x as f32, n.y as f32);
        let fk = npc_folk(n, k as u64 * 7919 + 3);
        folk::draw(&mut put, &fk, x, y + cs * 0.32, cs / 22.0 * 1.25, false, false, 1.0);
        let near = (n.x - g.x).abs() <= 3 && (n.y - g.y).abs() <= 3;
        if near || n.role != crate::adventure::actor::Role::Townsfolk { labels.push((x, y - cs * 0.75, format!("{} ({})", n.name, n.role.word()), 0x0046_3A6E)); }
    }
    // Monsters, with their life bars (Tibia's).
    let turn = g.turn;
    for m in p.monsters.iter().filter(|m| m.z == g.z && m.hp > 0 && vis(m.x, m.y)) {
        let (px, py) = v.pos.get(&m.uid).copied().unwrap_or((m.x as f32, m.y as f32));
        let (x, y) = to_screen(px, py);
        let strike = turn.saturating_sub(m.struck_at) < 60;
        draw_monster(&mut put, m, x, y, cs, strike, 1.0);
        let share = m.hp as f32 / m.max_hp.max(1) as f32;
        let bw = cs * if m.boss { 1.2 } else { 0.8 };
        let (bx, by) = (x - bw / 2.0, y - cs * 0.62);
        let col: Rgb = if share > 0.6 { [60.0, 150.0, 60.0] } else if share > 0.3 { [200.0, 160.0, 40.0] } else { [190.0, 40.0, 30.0] };
        for yy in 0..3 { for xx in 0..bw as i64 { let c = if (xx as f32) < bw * share { col } else { [40.0, 30.0, 24.0] }; put(bx as i64 + xx, by as i64 + yy, c, 0.9); } }
        if Some(m.uid) == v.target { let mut pen = super::ink::Pen::new(&mut put, x, y, cs * 1.1); pen.line_a((-0.9, -0.9), (-0.5, -0.9), [190.0, 40.0, 30.0], 2.0, 1.0); pen.line_a((-0.9, -0.9), (-0.9, -0.5), [190.0, 40.0, 30.0], 2.0, 1.0); pen.line_a((0.9, 0.9), (0.5, 0.9), [190.0, 40.0, 30.0], 2.0, 1.0); pen.line_a((0.9, 0.9), (0.9, 0.5), [190.0, 40.0, 30.0], 2.0, 1.0); }
        if m.boss || Some(m.uid) == v.target { labels.push((x, by - 15.0, m.name.clone(), if m.boss { 0x0090_1010 } else { 0x0038_2A20 })); }
    }
    // The adventurer.
    let (hxs, hys) = to_screen(v.hero_pos.0, v.hero_pos.1);
    let hf = hero_folk(g);
    let left = g.facing.0 < 0;
    folk::draw(&mut put, &hf, hxs, hys + cs * 0.32, cs / 22.0 * 1.3, left, false, 1.0);
    // Effects.
    let now = Instant::now();
    v.effects.retain(|(e, born)| {
        let age = now.duration_since(*born).as_secs_f32();
        let life = match e { Effect::Number { .. } => 1.0, Effect::Missile { .. } => 0.22, Effect::Area { .. } => 0.4, Effect::Speech { .. } => 2.6, Effect::Puff { .. } => 0.3 };
        age < life
    });
    for (e, born) in v.effects.iter() {
        let age = now.duration_since(*born).as_secs_f32();
        match e {
            Effect::Missile { from, to, z, kind } if *z == g.z && (g.visible(from.0, from.1) || g.visible(to.0, to.1)) => {
                let t = (age / 0.22).min(1.0);
                let (ax, ay) = to_screen(from.0 as f32, from.1 as f32);
                let (bx, by) = to_screen(to.0 as f32, to.1 as f32);
                let (x, y) = (ax + (bx - ax) * t, ay + (by - ay) * t);
                let c = element_colour(kind);
                let mut pen = super::ink::Pen::new(&mut put, x, y, cs * 0.5);
                if matches!(kind.as_str(), "arrow" | "bolt" | "spear" | "small_stone") { let (dx, dy) = (bx - ax, by - ay); let l = (dx * dx + dy * dy).sqrt().max(1.0); pen.line((-dx / l * 0.8, -dy / l * 0.8), (dx / l * 0.8, dy / l * 0.8), [110.0, 80.0, 50.0], 2.0); }
                else { pen.glow(0.0, 0.0, 1.0, c, 0.9); pen.ellipse_f(0.0, 0.0, 0.35, 0.35, mix(c, [255.0, 255.0, 240.0], 0.4), super::ink::Finish::Paint); }
            }
            Effect::Area { cells, z, kind } if *z == g.z && cells.iter().any(|c| g.visible(c.0, c.1)) => {
                let a = 1.0 - age / 0.4;
                let c = element_colour(kind);
                for (cx, cy) in cells { let (x, y) = to_screen(*cx as f32, *cy as f32); let mut pen = super::ink::Pen::new(&mut put, x, y, cs); pen.glow(0.0, 0.0, 0.9, c, 0.7 * a); if kind == "heal" { for k in 0..4 { let ang = k as f32 * 1.57 + age * 6.0; pen.dot(0.5 * ang.cos(), 0.5 * ang.sin() - age, [255.0, 255.0, 220.0]); } } }
            }
            Effect::Puff { x, y, z } if *z == g.z => { let (sx, sy) = to_screen(*x as f32, *y as f32); let mut pen = super::ink::Pen::new(&mut put, sx, sy, cs); pen.glow(0.0, -0.2, 0.4 + age, [240.0, 236.0, 220.0], 0.5 * (1.0 - age / 0.3)); }
            _ => {}
        }
    }
    drop(put);
    for (e, born) in v.effects.iter() {
        let age = now.duration_since(*born).as_secs_f32();
        match e {
            Effect::Number { x, y, z, value, tone } if *z == g.z && (g.visible(*x, *y) || (*x, *y) == (g.x, g.y)) => {
                let (sx, sy) = to_screen(*x as f32, *y as f32);
                let col = match tone { Tone::Level => 0x0020_7A20, Tone::Info => 0x0020_4A9A, Tone::Hurt => 0x00B0_1810, _ => 0x00C0_3010 };
                let t = format!("{}", value);
                let tw = fonts::width(&t, Face::Roman, 15.0, 0.0);
                fonts::draw(buf, w, h, sx - tw / 2.0, sy - cs * 0.7 - age * 22.0, &t, Face::Roman, 15.0, 0.0, col, Some(0x00EE_E4CC));
            }
            Effect::Speech { x, y, z, text } if *z == g.z && g.visible(*x, *y) => {
                let (sx, sy) = to_screen(*x as f32, *y as f32);
                let tw = fonts::width(text, Face::Italic, 14.0, 0.0);
                fonts::draw(buf, w, h, (sx - tw / 2.0).max(4.0), sy - cs * 1.1, text, Face::Italic, 14.0, 0.0, 0x00B0_6010, Some(0x00EE_E4CC));
            }
            _ => {}
        }
    }
    for (x, y, t, col) in labels { let tw = fonts::width(&t, Face::Italic, 13.0, 0.0); if x - tw / 2.0 > 0.0 && x + tw / 2.0 < map_w as f32 { fonts::draw(buf, w, h, x - tw / 2.0, y, &t, Face::Italic, 13.0, 0.0, col, Some(0x00EE_E4CC)); } }
    // The place's name and floor, top-left.
    let title = format!("{} — {}", p.spec.name, f.name);
    let r = Rect { x: 10, y: 10, w: (fonts::width(&title, Face::SmallCaps, 18.0, 0.5) as usize + 28).min(map_w - 20), h: 34 };
    card(buf, w, r);
    fonts::draw(buf, w, h, 24.0, 17.0, &title, Face::SmallCaps, 18.0, 0.5, 0x0030_1E14, None);
    // Hover: what is under the mouse.
    let (mx, my) = v.mouse;
    if mx >= 0.0 && (mx as usize) < map_w {
        let (cx, cy) = (((mx + ox as f32) / cs).floor() as i32, ((my + oy as f32) / cs).floor() as i32);
        if f.inside(cx, cy) && f.seen[cy as usize * f.w + cx as usize] {
            let what = describe_cell(g, cx, cy);
            if !what.is_empty() {
                let tw = fonts::width(&what, Face::Italic, 14.0, 0.0);
                let r = Rect { x: ((mx + 14.0) as usize).min(map_w.saturating_sub(tw as usize + 20)), y: (my as usize + 18).min(h.saturating_sub(30)), w: tw as usize + 16, h: 22 };
                card(buf, w, r);
                fonts::draw(buf, w, h, r.x as f32 + 8.0, r.y as f32 + 4.0, &what, Face::Italic, 14.0, 0.0, 0x0038_2A20, None);
            }
        }
    }
}

fn element_colour(k: &str) -> Rgb {
    match k { "fire" => [240.0, 120.0, 40.0], "ice" => [150.0, 200.0, 240.0], "energy" => [170.0, 110.0, 230.0], "earth" | "poison" => [110.0, 170.0, 60.0], "holy" => [250.0, 230.0, 140.0], "dark" => [90.0, 50.0, 110.0], "heal" => [140.0, 220.0, 150.0], "blow" => [230.0, 230.0, 220.0], _ => [220.0, 220.0, 200.0] }
}

/// What a cell holds, in words.
fn describe_cell(g: &Game, x: i32, y: i32) -> String {
    let Some(p) = g.place() else { return String::new() };
    let f = &p.floors[g.z];
    if g.visible(x, y) {
        if let Some(m) = p.monsters.iter().find(|m| m.z == g.z && m.x == x && m.y == y && m.hp > 0) {
            return format!("{} ({} of {} life){}", crate::adventure::game::cap(&m.a()), m.hp, m.max_hp, m.legend.as_ref().map(|l| format!(": {}", l.short)).unwrap_or_default());
        }
        if let Some(n) = p.npcs.iter().find(|n| n.z == g.z && n.x == x && n.y == y) { return format!("{}, the {}", n.name, n.role.word()); }
        if let Some(items) = f.items.get(&(x, y)) { if !items.is_empty() { return items.iter().map(|i| i.describe()).collect::<Vec<_>>().join(", "); } }
    }
    let t = f.at(x, y);
    let w = t.feature.word();
    if !w.is_empty() { return crate::adventure::game::cap(w); }
    String::new()
}

// ---------------------------------------------------------------------------------------------
// The world map

fn site_mark(put: &mut dyn FnMut(i64, i64, Rgb, f32), kind: SiteKind, x: f32, y: f32, s: f32) {
    let mut pen = super::ink::Pen::new(put, x, y, s);
    let stone: Rgb = [176.0, 168.0, 156.0];
    let red: Rgb = [150.0, 40.0, 30.0];
    match kind {
        SiteKind::Town => { for (k, dx) in [-0.45f32, 0.0, 0.45].iter().enumerate() { let hgt = [0.3, 0.55, 0.35][k]; pen.rect(dx - 0.2, 0.5 - hgt, dx + 0.2, 0.5, [200.0, 170.0, 130.0]); pen.poly(&[(dx - 0.26, 0.5 - hgt), (*dx, 0.2 - hgt), (dx + 0.26, 0.5 - hgt)], [150.0, 60.0, 50.0]); } }
        SiteKind::Ruin | SiteKind::Castle => { pen.rect(-0.6, -0.1, -0.2, 0.6, stone); pen.rect(0.1, -0.4, 0.5, 0.6, stone); pen.line((-0.2, 0.1), (0.1, 0.3), crate::tiles::ink::INK, 1.0); if kind == SiteKind::Castle { for dx in [0.1f32, 0.3, 0.5] { pen.rect(dx - 0.06, -0.55, dx + 0.06, -0.4, stone); } } }
        SiteKind::Lair => { pen.ellipse(0.0, 0.1, 0.6, 0.45, [60.0, 50.0, 46.0]); for k in [-0.3f32, 0.0, 0.3] { pen.poly(&[(k - 0.08, -0.15), (k, 0.1), (k + 0.08, -0.15)], [230.0, 220.0, 200.0]); } pen.dot(-0.2, -0.3, red); pen.dot(0.2, -0.3, red); }
        SiteKind::Tomb => { pen.rect(-0.3, -0.5, 0.3, 0.55, stone); pen.line((0.0, -0.35), (0.0, 0.1), crate::tiles::ink::INK, 1.5); pen.line((-0.15, -0.2), (0.15, -0.2), crate::tiles::ink::INK, 1.5); }
        SiteKind::Temple | SiteKind::Shrine => { pen.poly(&[(-0.65, -0.2), (0.0, -0.65), (0.65, -0.2)], stone); for dx in [-0.45f32, -0.15, 0.15, 0.45] { pen.rect(dx - 0.07, -0.2, dx + 0.07, 0.5, stone); } if kind == SiteKind::Shrine { pen.glow(0.0, 0.1, 0.5, [200.0, 60.0, 50.0], 0.6); } }
        SiteKind::Cave | SiteKind::Mine | SiteKind::Halls => { pen.ellipse(0.0, 0.0, 0.65, 0.55, [130.0, 116.0, 100.0]); pen.ellipse_f(0.0, 0.15, 0.32, 0.38, [40.0, 34.0, 30.0], super::ink::Finish::Plain); if kind == SiteKind::Mine { pen.line((-0.5, -0.6), (0.5, 0.0), [120.0, 84.0, 50.0], 2.0); } }
        SiteKind::Labyrinth => { let pts: Vec<(f32, f32)> = (0..30).map(|k| { let a = k as f32 * 0.5; let r = 0.06 + k as f32 * 0.02; (r * a.cos(), r * a.sin()) }).collect(); pen.path(&pts, crate::tiles::ink::INK, 1.5); }
        SiteKind::Camp => { pen.poly(&[(-0.6, 0.5), (-0.2, -0.4), (0.2, 0.5)], [176.0, 140.0, 96.0]); pen.poly(&[(0.0, 0.5), (0.35, -0.2), (0.7, 0.5)], [160.0, 124.0, 84.0]); pen.glow(0.0, 0.5, 0.4, [255.0, 180.0, 80.0], 0.5); }
        SiteKind::DarkFortress => { pen.rect(-0.35, -0.7, 0.35, 0.6, [50.0, 44.0, 48.0]); pen.poly(&[(-0.45, -0.7), (0.0, -1.0), (0.45, -0.7)], [40.0, 34.0, 38.0]); pen.glow(0.0, -0.3, 0.4, [230.0, 60.0, 40.0], 0.8); }
        SiteKind::Wilds => {}
    }
}

fn draw_world(g: &Game, v: &mut View, buf: &mut [u32], w: usize, h: usize, map_w: usize) {
    // (The world map is drawn into a map-sized buffer by the caller's renderer.)
    let cam = v.world_cam;
    let tp = cam.tile_px;
    let to_screen = |tx: usize, ty: usize| -> (f32, f32) {
        let ww = g.world.w as f32;
        let mut dx = tx as f32 + 0.5 - cam.cx;
        if dx > ww / 2.0 { dx -= ww; } else if dx < -ww / 2.0 { dx += ww; }
        (map_w as f32 / 2.0 + dx * tp, h as f32 / 2.0 + (ty as f32 + 0.5 - cam.cy) * tp)
    };
    let clip = Rect { x: 0, y: 0, w: map_w, h };
    let mut labels = Vec::new();
    {
        let mut put = put_into(buf, w, h, Some(clip));
        for s in g.sites.iter().filter(|s| g.known.contains(&s.id) && s.kind != SiteKind::Wilds) {
            let (x, y) = to_screen(s.tile.0, s.tile.1);
            if x < -20.0 || y < -20.0 || x > map_w as f32 + 20.0 || y > h as f32 + 20.0 { continue; }
            site_mark(&mut put, s.kind, x, y, tp * 0.8);
            let d = { let dx = (s.tile.0 as i32 - g.tile.0 as i32).abs(); (dx.min(g.world.w as i32 - dx)).max((s.tile.1 as i32 - g.tile.1 as i32).abs()) };
            if tp >= 18.0 && (d <= if s.kind == SiteKind::Town { 10 } else { 5 } || s.tile == g.tile) { labels.push((x, y + tp * 0.45, s.name.clone(), s.kind)); }
        }
        let (x, y) = to_screen(g.tile.0, g.tile.1);
        let hf = hero_folk(g);
        let mut pen = super::ink::Pen::new(&mut put, x, y, tp);
        pen.glow(0.0, 0.0, 0.9, [255.0, 230.0, 160.0], 0.5);
        folk::draw(&mut put, &hf, x, y + tp * 0.3, (tp / 22.0 * 1.4).max(0.6), g.facing.0 < 0, false, 1.0);
    }
    for (x, y, t, k) in labels {
        let tw = fonts::width(&t, Face::Italic, 13.0, 0.0);
        if x - tw / 2.0 < 0.0 || x + tw / 2.0 > map_w as f32 { continue; }
        fonts::draw(buf, w, h, x - tw / 2.0, y, &t, Face::Italic, 13.0, 0.0, if k == SiteKind::Town { 0x0038_2A20 } else { 0x0090_2010 }, Some(0x00EE_E4CC));
    }
    // What is here.
    let here: Vec<String> = g.sites.iter().filter(|s| s.tile == g.tile && s.kind != SiteKind::Wilds).map(|s| format!("{} ({})", s.name, s.kind.word())).collect();
    let line = if here.is_empty() { format!("The road at {},{}. Arrows walk; the land's danger grows away from the towns.", g.tile.0, g.tile.1) } else { format!("{}. Enter to go in.", here.join("; ")) };
    let r = Rect { x: 10, y: 10, w: (fonts::width(&line, Face::Italic, 15.0, 0.0) as usize + 24).min(map_w - 20), h: 30 };
    card(buf, w, r);
    fonts::draw(buf, w, h, 22.0, 17.0, &line, Face::Italic, 15.0, 0.0, 0x0038_2A20, None);
}

// ---------------------------------------------------------------------------------------------
// The panel, the log, the talk

fn bar(buf: &mut [u32], w: usize, h: usize, x: usize, y: usize, bw: usize, share: f32, col: u32, text: &str) {
    for yy in 0..14 { for xx in 0..bw {
        let k = (y + yy) * w + x + xx;
        if k < buf.len() { buf[k] = if (xx as f32) < bw as f32 * share.clamp(0.0, 1.0) { col } else { 0x00D8_C8A6 }; }
        if yy == 0 || yy == 13 || xx == 0 || xx == bw - 1 { if k < buf.len() { buf[k] = 0x0038_2A20; } }
    } }
    let tw = fonts::width(text, Face::Roman, 12.0, 0.0);
    fonts::draw(buf, w, h, x as f32 + (bw as f32 - tw) / 2.0, y as f32 + 0.0, text, Face::Roman, 12.0, 0.0, 0x0020_1810, None);
}

fn draw_panel(g: &Game, v: &mut View, buf: &mut [u32], w: usize, h: usize, map_w: usize) {
    let r = Rect { x: map_w, y: 0, w: PANEL_W, h };
    card(buf, w, r);
    let x0 = map_w + 16;
    let hero = &g.hero;
    fonts::draw(buf, w, h, x0 as f32, 12.0, &hero.name, Face::SmallCaps, 22.0, 0.5, 0x0030_1E14, None);
    let title = format!("Level {} {} of the {}", hero.level, hero.calling.as_deref().unwrap_or("commoner"), hero.race);
    fonts::draw(buf, w, h, x0 as f32, 38.0, &title, Face::Italic, 14.0, 0.0, 0x005A_4634, None);
    let bw = PANEL_W - 32;
    bar(buf, w, h, x0, 60, bw, hero.hp as f32 / hero.max_hp() as f32, 0x00B0_3A2A, &format!("life {} / {}", hero.hp, hero.max_hp()));
    bar(buf, w, h, x0, 78, bw, hero.mana as f32 / hero.max_mana() as f32, 0x0040_6AB0, &format!("mana {} / {}", hero.mana, hero.max_mana()));
    bar(buf, w, h, x0, 96, bw, hero.level_progress(), 0x00A8_8A30, &format!("experience {} (level {} at {})", hero.xp, hero.level + 1, crate::adventure::hero::xp_for(hero.level + 1)));
    let mut status = vec![format!("{} gold", hero.gold())];
    if hero.poisoned > 0 { status.push("poisoned".into()); }
    if hero.fed == 0 { status.push("hungry".into()); }
    if hero.torch > 0 { status.push("torch lit".into()); }
    if hero.hasted > 0 { status.push("hasted".into()); }
    fonts::draw(buf, w, h, x0 as f32, 114.0, &status.join(" · "), Face::Italic, 13.0, 0.0, 0x005A_4634, None);
    // The paper doll: eight slots.
    let slot_s = 40usize;
    let doll: [(Slot, usize, usize); 8] = [(Slot::Head, 1, 0), (Slot::Neck, 2, 0), (Slot::Hand, 0, 1), (Slot::Body, 1, 1), (Slot::Shield, 2, 1), (Slot::Ring, 0, 2), (Slot::Legs, 1, 2), (Slot::Feet, 3, 1)];
    let (dx, dy) = (x0 + 30, 136);
    {
        let mut put = put_into(buf, w, h, None);
        for (s, cx, cy) in doll {
            let rr = Rect { x: dx + cx * (slot_s + 6), y: dy + cy * (slot_s + 4), w: slot_s, h: slot_s };
            for yy in rr.y..rr.y + rr.h { for xx in rr.x..rr.x + rr.w { put(xx as i64, yy as i64, [220.0, 206.0, 176.0], 0.7); } }
            for xx in rr.x..rr.x + rr.w { put(xx as i64, rr.y as i64, INK, 0.6); put(xx as i64, (rr.y + rr.h - 1) as i64, INK, 0.6); }
            for yy in rr.y..rr.y + rr.h { put(rr.x as i64, yy as i64, INK, 0.6); put((rr.x + rr.w - 1) as i64, yy as i64, INK, 0.6); }
            match &hero.equipped[s as usize] {
                Some(it) => { let tint = it.material.as_deref().and_then(|m| crate::adventure::data::data().material(m)).map(|m| [m.colour[0] as f32, m.colour[1] as f32, m.colour[2] as f32]); super::glyphs::draw(&mut put, ai::glyph(&it.def().glyph), (rr.x + slot_s / 2) as f32, (rr.y + slot_s / 2) as f32, slot_s as f32 * 0.8, tint); }
                None => {}
            }
            v.hits.push((rr, Hit::Slot(s)));
        }
    }
    let ay = dy + 3 * (slot_s + 4) + 4;
    let stats = format!("attack {} · defense {} · armour {}", hero.weapon().map_or(5, |w| w.attack()), hero.defense(), hero.armor());
    fonts::draw(buf, w, h, x0 as f32, ay as f32, &stats, Face::Roman, 13.0, 0.0, 0x0038_2A20, None);
    // Tabs.
    let ty = ay + 22;
    let tabs = [(Tab::Pack, "Pack"), (Tab::Skills, "Skills"), (Tab::Spells, "Spells"), (Tab::Quests, "Quests")];
    for (k, (t, name)) in tabs.iter().enumerate() {
        let rr = Rect { x: x0 + k * 66, y: ty, w: 62, h: 22 };
        let on = v.tab == *t;
        if on { for yy in rr.y..rr.y + rr.h { for xx in rr.x..rr.x + rr.w { buf[yy * w + xx] = 0x00F2_E8D0; } } }
        super::ui::outline(buf, w, rr, if on { 0x0038_2A20 } else { 0x0080_6A52 });
        let tw = fonts::width(name, Face::SmallCaps, 14.0, 0.3);
        fonts::draw(buf, w, h, rr.x as f32 + (rr.w as f32 - tw) / 2.0, rr.y as f32 + 3.0, name, Face::SmallCaps, 14.0, 0.3, if on { 0x009A_2A1E } else { 0x0038_2A20 }, None);
        v.hits.push((rr, Hit::Tab(*t)));
    }
    let by = ty + 30;
    let bottom = h.saturating_sub(16);
    match v.tab {
        Tab::Pack => {
            let cell = 44usize;
            let cols = (PANEL_W - 32) / cell;
            let mut put_hits: Vec<(Rect, usize)> = Vec::new();
            {
                let mut put = put_into(buf, w, h, None);
                for (k, it) in hero.pack.iter().enumerate() {
                    let (cx, cy) = (k % cols, k / cols);
                    let rr = Rect { x: x0 + cx * cell, y: by + cy * cell, w: cell - 4, h: cell - 4 };
                    if rr.y + rr.h > bottom { break; }
                    for yy in rr.y..rr.y + rr.h { for xx in rr.x..rr.x + rr.w { put(xx as i64, yy as i64, [222.0, 208.0, 180.0], 0.6); } }
                    let tint = it.material.as_deref().and_then(|m| crate::adventure::data::data().material(m)).filter(|_| matches!(it.def().kind.as_str(), "weapon" | "armour" | "shield")).map(|m| [m.colour[0] as f32, m.colour[1] as f32, m.colour[2] as f32]);
                    super::glyphs::draw(&mut put, ai::glyph(&it.def().glyph), (rr.x + rr.w / 2) as f32, (rr.y + rr.h / 2) as f32, cell as f32 * 0.72, tint);
                    if it.is_artifact() { let mut pen = super::ink::Pen::new(&mut put, (rr.x + rr.w / 2) as f32, (rr.y + rr.h / 2) as f32, cell as f32); pen.glow(0.0, 0.0, 0.7, [255.0, 220.0, 120.0], 0.35); }
                    put_hits.push((rr, k));
                }
            }
            for (rr, k) in put_hits {
                let it = &hero.pack[k];
                if it.count > 1 { let t = format!("{}", it.count); fonts::draw(buf, w, h, (rr.x + rr.w) as f32 - fonts::width(&t, Face::Roman, 12.0, 0.0) - 2.0, (rr.y + rr.h) as f32 - 14.0, &t, Face::Roman, 12.0, 0.0, 0x0020_1810, Some(0x00EE_E4CC)); }
                v.hits.push((rr, Hit::Pack(k)));
            }
            // The item under the mouse, described.
            if let Some((_, Hit::Pack(k))) = v.hits.iter().find(|(r, h)| matches!(h, Hit::Pack(_)) && r.contains(v.mouse.0, v.mouse.1)).copied() {
                let it = &hero.pack[k];
                let lines = fonts::wrap(&format!("{}: {}.{}", it.describe(), it.stats(), it.story.as_ref().map(|s| format!(" {}", s)).unwrap_or_default()), Face::Italic, 13.0, (PANEL_W - 32) as f32);
                let y0 = bottom.saturating_sub(lines.len() * 16 + 4);
                for (i, l) in lines.iter().enumerate() { fonts::draw(buf, w, h, x0 as f32, (y0 + i * 16) as f32, l, Face::Italic, 13.0, 0.0, 0x0038_2A20, Some(0x00EE_E4CC)); }
            }
        }
        Tab::Skills => {
            let mut y = by;
            for s in Skill::ALL {
                fonts::draw(buf, w, h, x0 as f32, y as f32, &format!("{}: {}", crate::adventure::game::cap(s.word()), hero.skill(s)), Face::Roman, 14.0, 0.0, 0x0038_2A20, None);
                bar(buf, w, h, x0 + 170, y + 2, PANEL_W - 200, hero.skill_progress(s), 0x0060_8A50, "");
                y += 22;
            }
            y += 8;
            for l in [format!("Slain: {}", hero.kills), format!("Deaths: {}", hero.deaths), format!("Bosses: {}", g.stats.bosses), format!("Places entered: {}", g.stats.sites_entered)] {
                fonts::draw(buf, w, h, x0 as f32, y as f32, &l, Face::Italic, 13.0, 0.0, 0x005A_4634, None); y += 18;
            }
        }
        Tab::Spells => {
            let mut y = by;
            if hero.spells.is_empty() { for l in fonts::wrap("You know no words of power. A temple's priest teaches them (level 3 and on, more with a calling).", Face::Italic, 14.0, (PANEL_W - 32) as f32) { fonts::draw(buf, w, h, x0 as f32, y as f32, &l, Face::Italic, 14.0, 0.0, 0x005A_4634, None); y += 18; } }
            for (k, id) in hero.spells.iter().enumerate() {
                let Some(sp) = crate::adventure::data::data().spell(id) else { continue };
                let rr = Rect { x: x0, y, w: PANEL_W - 32, h: 36 };
                fonts::draw(buf, w, h, x0 as f32, y as f32, &format!("F{}  {}", k + 1, sp.name), Face::SmallCaps, 15.0, 0.3, if hero.mana >= sp.mana { 0x0038_2A20 } else { 0x0090_8070 }, None);
                fonts::draw(buf, w, h, x0 as f32 + 30.0, y as f32 + 17.0, &format!("\"{}\", {} mana", sp.words, sp.mana), Face::Italic, 13.0, 0.0, 0x005A_4634, None);
                v.hits.push((rr, Hit::Spell(k)));
                y += 40;
            }
        }
        Tab::Quests => {
            let mut y = by;
            if g.quests.is_empty() { for l in fonts::wrap("No quests yet. The lord, the guard captain, the priest and the sage of a town have work.", Face::Italic, 14.0, (PANEL_W - 32) as f32) { fonts::draw(buf, w, h, x0 as f32, y as f32, &l, Face::Italic, 14.0, 0.0, 0x005A_4634, None); y += 18; } }
            for q in g.quests.iter().rev() {
                if y + 40 > bottom { break; }
                let done = q.state == crate::adventure::quest::State::Rewarded;
                fonts::draw(buf, w, h, x0 as f32, y as f32, &q.title, Face::SmallCaps, 14.0, 0.2, if done { 0x0090_8070 } else { 0x009A_2A1E }, None);
                fonts::draw(buf, w, h, x0 as f32, y as f32 + 16.0, &format!("for {} of {}: {}", q.giver, g.site(q.town).map(|s| s.name.as_str()).unwrap_or(""), q.progress()), Face::Italic, 13.0, 0.0, 0x005A_4634, None);
                y += 38;
            }
        }
    }
}

fn draw_log(g: &Game, buf: &mut [u32], w: usize, h: usize, map_w: usize) {
    let lw = (map_w as f32 * 0.62).min(720.0) as usize;
    let lines: Vec<&crate::adventure::game::Line> = g.log.iter().rev().take(7).collect();
    let wrapped: Vec<(String, Tone, usize)> = lines.iter().rev().enumerate().flat_map(|(k, l)| fonts::wrap(&l.text, Face::Roman, 14.0, lw as f32 - 24.0).into_iter().map(move |s| (s, l.tone, k))).collect();
    let wrapped: Vec<(String, Tone, usize)> = wrapped.into_iter().rev().take(9).collect::<Vec<_>>().into_iter().rev().collect();
    let hh = wrapped.len() * 17 + 14;
    let r = Rect { x: 10, y: h.saturating_sub(hh + 10), w: lw, h: hh };
    card(buf, w, r);
    let n = wrapped.len();
    for (i, (l, tone, _)) in wrapped.iter().enumerate() {
        let col = super::ui::mix(tone_colour(*tone), 0x00EA_DEC4, if i + 3 < n { 0.35 } else { 0.0 });
        fonts::draw(buf, w, h, r.x as f32 + 12.0, (r.y + 7 + i * 17) as f32, l, Face::Roman, 14.0, 0.0, col, None);
    }
}

fn draw_talk(g: &Game, v: &mut View, buf: &mut [u32], w: usize, h: usize, map_w: usize) {
    let t = g.talk.as_ref().unwrap();
    let cw = (map_w as f32 * 0.7).min(640.0) as usize;
    let said = fonts::wrap(&t.said, Face::Italic, 16.0, cw as f32 - 120.0);
    let opts: Vec<Vec<String>> = t.options.iter().enumerate().map(|(k, (s, _))| fonts::wrap(&format!("{}  {}", k + 1, s), Face::Roman, 15.0, cw as f32 - 60.0)).collect();
    let ch = 56 + said.len() * 20 + opts.iter().map(|o| o.len() * 18 + 4).sum::<usize>() + 16;
    let r = Rect { x: (map_w - cw) / 2, y: 60, w: cw, h: ch.min(h - 80) };
    card(buf, w, r);
    // Their face (the figure) and name.
    if let Some(n) = g.place().and_then(|p| p.npcs.get(t.npc)) {
        let fk = npc_folk(n, t.npc as u64 * 7919 + 3);
        let mut put = put_into(buf, w, h, None);
        folk::draw(&mut put, &fk, (r.x + 50) as f32, (r.y + 78) as f32, 3.0, false, false, 1.0);
    }
    fonts::draw(buf, w, h, (r.x + 100) as f32, (r.y + 14) as f32, &format!("{}, {}", t.name, t.role.word()), Face::SmallCaps, 20.0, 0.5, 0x009A_2A1E, None);
    let mut y = r.y + 44;
    for l in &said { fonts::draw(buf, w, h, (r.x + 100) as f32, y as f32, l, Face::Italic, 16.0, 0.0, 0x0038_2A20, None); y += 20; }
    y += 10;
    for (k, o) in opts.iter().enumerate() {
        let rr = Rect { x: r.x + 30, y, w: cw - 60, h: o.len() * 18 + 2 };
        let hover = rr.contains(v.mouse.0, v.mouse.1);
        for (i, l) in o.iter().enumerate() { fonts::draw(buf, w, h, rr.x as f32, (y + i * 18) as f32, l, Face::Roman, 15.0, 0.0, if hover { 0x009A_2A1E } else { 0x0046_3A6E }, None); }
        v.hits.push((rr, Hit::Choice(k)));
        y += o.len() * 18 + 4;
        if y > r.y + r.h { break; }
    }
}

/// A quest chest beside the adventurer: its rewards to choose from.
fn draw_chest_choice(g: &Game, v: &mut View, buf: &mut [u32], w: usize, h: usize, map_w: usize) {
    let Some(f) = g.floor() else { return };
    if g.chosen.contains(&g.here.unwrap_or(0)) { return; }
    let Some(choices) = crate::adventure::map::DIRS8.iter().find_map(|(dx, dy)| match &f.at(g.x + dx, g.y + dy).feature { Feature::QuestChest { choices, taken: false, .. } => Some(choices.clone()), _ => None }) else { return };
    let cw = 520usize;
    let r = Rect { x: (map_w - cw) / 2, y: 70, w: cw, h: 70 + choices.len() * 54 };
    card(buf, w, r);
    fonts::draw(buf, w, h, (r.x + 24) as f32, (r.y + 14) as f32, "You may take one thing", Face::SmallCaps, 20.0, 0.5, 0x009A_2A1E, None);
    fonts::draw(buf, w, h, (r.x + 24) as f32, (r.y + 38) as f32, "Whatever you leave stays in the chest for ever.", Face::Italic, 14.0, 0.0, 0x005A_4634, None);
    for (k, it) in choices.iter().enumerate() {
        let rr = Rect { x: r.x + 20, y: r.y + 62 + k * 54, w: cw - 40, h: 50 };
        let hover = rr.contains(v.mouse.0, v.mouse.1);
        {
            let mut put = put_into(buf, w, h, None);
            let tint = it.material.as_deref().and_then(|m| crate::adventure::data::data().material(m)).map(|m| [m.colour[0] as f32, m.colour[1] as f32, m.colour[2] as f32]);
            super::glyphs::draw(&mut put, ai::glyph(&it.def().glyph), (rr.x + 24) as f32, (rr.y + 24) as f32, 40.0, tint);
        }
        fonts::draw(buf, w, h, (rr.x + 56) as f32, rr.y as f32 + 4.0, &format!("{}  {}", k + 1, crate::adventure::game::cap(&it.describe())), Face::Roman, 16.0, 0.0, if hover { 0x009A_2A1E } else { 0x0038_2A20 }, None);
        fonts::draw(buf, w, h, (rr.x + 74) as f32, rr.y as f32 + 24.0, &it.stats(), Face::Italic, 13.0, 0.0, 0x005A_4634, None);
        v.hits.push((rr, Hit::Chest(k)));
    }
}

// ---------------------------------------------------------------------------------------------
// The window

/// Feed the game's effects to the view (with the time they began).
pub fn take_effects(g: &mut Game, v: &mut View) { let now = Instant::now(); for e in g.take_effects() { v.effects.push((e, now)); } }

fn act(g: &mut Game, v: &mut View, a: Action) { g.act(a); take_effects(g, v); }

/// The first step toward (tx, ty) on the current floor (8-way BFS over walkable cells and doors).
fn step_toward(g: &Game, tx: i32, ty: i32) -> Option<(i32, i32)> {
    let f = g.floor()?;
    let d = f.distances(tx, ty, 200, |x, y| f.at(x, y).walkable() || matches!(f.at(x, y).feature, Feature::Door { .. }) || (x, y) == (g.x, g.y));
    let cur = d.get(g.y as usize * f.w + g.x as usize).copied().unwrap_or(i32::MAX);
    if cur == i32::MAX { return None; }
    crate::adventure::map::DIRS8.iter().filter(|(dx, dy)| { let (nx, ny) = (g.x + dx, g.y + dy); f.inside(nx, ny) && d[ny as usize * f.w + nx as usize] < cur })
        .min_by_key(|(dx, dy)| d[(g.y + dy) as usize * f.w + (g.x + dx) as usize]).copied()
}

pub fn run(world: &crate::world::WorldData, history: Option<&crate::history::world_state::WorldHistory>, atlas: &super::atlas::Atlas, seed: u64, load: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let mut g = match load {
        Some(path) => crate::adventure::Game::load(std::path::Path::new(path), world.width, world.height)?,
        None => crate::adventure::new_game(world, history, seed, None),
    };
    if load.is_some() { let n = g.hero.name.clone(); g.say(Tone::Level, format!("{} takes up the road again.", n)); }
    else { g.say(Tone::Info, "Keys: arrows or WASD walk (Q E Z C diagonals), bump to strike, open and talk. ? for all the keys."); }
    let mut v = View::new(&g);
    take_effects(&mut g, &mut v);
    let (mut w, mut h) = (1280usize, 800usize);
    let mut window = Window::new("The Dark Tower — an adventure", w, h, WindowOptions { resize: true, ..WindowOptions::default() })?;
    window.set_target_fps(60);
    window.set_key_repeat_delay(0.22);
    window.set_key_repeat_rate(0.11);
    let mut buf = vec![0u32; w * h];
    // The world map underneath travel: the tile world kept between frames.
    let mut tw = super::classify::TileWorld::build(world, atlas);
    if let Some(hh) = history { tw.apply_history(world, hh, atlas); }
    tw.set_season(world, crate::seasons::Season::Summer);
    let mut was_down = false;
    let mut was_right = false;
    let mut esc_once = false;
    let mut show_help = false;
    while window.is_open() {
        let (nw, nh) = window.get_size();
        if (nw, nh) != (w, h) && nw > PANEL_W + 200 && nh > 300 { w = nw; h = nh; buf = vec![0u32; w * h]; }
        if let Some(m) = window.get_mouse_pos(MouseMode::Clamp) { v.mouse = m; }
        let down = window.get_mouse_down(MouseButton::Left);
        let rdown = window.get_mouse_down(MouseButton::Right);
        let click = down && !was_down;
        let rclick = rdown && !was_right;
        was_down = down;
        was_right = rdown;
        let pressed = |k: Key| window.is_key_pressed(k, KeyRepeat::No);
        let repeat = |k: Key| window.is_key_pressed(k, KeyRepeat::Yes);
        let map_w = w.saturating_sub(PANEL_W);
        // Zoom.
        if let Some((_, dy)) = window.get_scroll_wheel() {
            if v.mouse.0 < map_w as f32 && dy != 0.0 {
                if g.here.is_some() { v.cs = (v.cs + dy.signum() * 4.0).clamp(20.0, 56.0); v.plan = None; }
                else { v.world_cam.tile_px = (v.world_cam.tile_px * if dy > 0.0 { 1.15 } else { 1.0 / 1.15 }).clamp(6.0, 48.0); }
            }
        }
        // Clicks on the panel and cards.
        if click || rclick {
            if let Some((_, hit)) = v.hits.iter().find(|(r, _)| r.contains(v.mouse.0, v.mouse.1)).copied() {
                match hit {
                    Hit::Pack(k) => { if rclick { act(&mut g, &mut v, Action::Drop(k)); } else { act(&mut g, &mut v, Action::UseItem(k)); } }
                    Hit::Slot(s) => act(&mut g, &mut v, Action::Unequip(s)),
                    Hit::Choice(k) => { crate::adventure::npc::answer(&mut g, k); }
                    Hit::Tab(t) => v.tab = t,
                    Hit::Spell(k) => { let t = v.target; act(&mut g, &mut v, Action::Cast(k, t)) }
                    Hit::Chest(k) => act(&mut g, &mut v, Action::Choose(k)),
                }
            } else if v.mouse.0 < map_w as f32 && g.here.is_some() && g.talk.is_none() && click {
                // Click on the map: strike what is there, else walk to it.
                let cs = v.cs;
                let ox = ((v.cam.0 + 0.5) * cs - map_w as f32 / 2.0).round();
                let oy = ((v.cam.1 + 0.5) * cs - h as f32 / 2.0).round();
                let (cx, cy) = (((v.mouse.0 + ox) / cs).floor() as i32, ((v.mouse.1 + oy) / cs).floor() as i32);
                if let Some(m) = g.place().and_then(|p| p.monsters.iter().find(|m| m.z == g.z && m.x == cx && m.y == cy && m.hp > 0)) { let uid = m.uid; v.target = Some(uid); act(&mut g, &mut v, Action::Attack(uid)); }
                else { v.walk_to = Some((cx, cy)); }
            }
        }
        // Keys.
        if g.talk.is_some() {
            let digits = [Key::Key1, Key::Key2, Key::Key3, Key::Key4, Key::Key5, Key::Key6, Key::Key7, Key::Key8, Key::Key9];
            for (k, key) in digits.iter().enumerate() { if pressed(*key) { crate::adventure::npc::answer(&mut g, k); } }
            if pressed(Key::Escape) { g.talk = None; }
        } else if g.here.is_none() {
            let dirs = [(Key::Up, (0, -1)), (Key::W, (0, -1)), (Key::Down, (0, 1)), (Key::S, (0, 1)), (Key::Left, (-1, 0)), (Key::A, (-1, 0)), (Key::Right, (1, 0)), (Key::D, (1, 0)),
                (Key::Q, (-1, -1)), (Key::E, (1, -1)), (Key::Z, (-1, 1)), (Key::C, (1, 1)), (Key::NumPad8, (0, -1)), (Key::NumPad2, (0, 1)), (Key::NumPad4, (-1, 0)), (Key::NumPad6, (1, 0)), (Key::NumPad7, (-1, -1)), (Key::NumPad9, (1, -1)), (Key::NumPad1, (-1, 1)), (Key::NumPad3, (1, 1))];
            for (key, (dx, dy)) in dirs { if repeat(key) { act(&mut g, &mut v, Action::Travel(dx, dy)); break; } }
            if pressed(Key::Enter) { act(&mut g, &mut v, Action::Enter); }
            v.world_cam.cx = g.tile.0 as f32 + 0.5;
            v.world_cam.cy = g.tile.1 as f32 + 0.5;
        } else {
            let dirs = [(Key::Up, (0, -1)), (Key::W, (0, -1)), (Key::Down, (0, 1)), (Key::S, (0, 1)), (Key::Left, (-1, 0)), (Key::A, (-1, 0)), (Key::Right, (1, 0)), (Key::D, (1, 0)),
                (Key::Q, (-1, -1)), (Key::E, (1, -1)), (Key::Z, (-1, 1)), (Key::C, (1, 1)), (Key::NumPad8, (0, -1)), (Key::NumPad2, (0, 1)), (Key::NumPad4, (-1, 0)), (Key::NumPad6, (1, 0)), (Key::NumPad7, (-1, -1)), (Key::NumPad9, (1, -1)), (Key::NumPad1, (-1, 1)), (Key::NumPad3, (1, 1))];
            for (key, (dx, dy)) in dirs { if repeat(key) { v.walk_to = None; act(&mut g, &mut v, Action::Move(dx, dy)); break; } }
            if pressed(Key::G) { act(&mut g, &mut v, Action::PickUp); }
            if pressed(Key::R) { act(&mut g, &mut v, Action::Rest); }
            if pressed(Key::Comma) || pressed(Key::Period) || pressed(Key::Enter) || pressed(Key::NumPad5) { act(&mut g, &mut v, Action::Climb); }
            if pressed(Key::H) { if let Some(k) = g.hero.pack.iter().position(|i| i.id.contains("health_potion")) { act(&mut g, &mut v, Action::UseItem(k)); } }
            if pressed(Key::J) { if let Some(k) = g.hero.pack.iter().position(|i| i.id.contains("mana_potion")) { act(&mut g, &mut v, Action::UseItem(k)); } }
            if pressed(Key::F) { if let Some(k) = g.hero.pack.iter().position(|i| i.def().kind == "food") { act(&mut g, &mut v, Action::UseItem(k)); } }
            if pressed(Key::Tab) {
                // The next visible monster, nearest first.
                let mut seen: Vec<(i32, u32)> = g.place().map(|p| p.monsters.iter().filter(|m| m.z == g.z && m.hp > 0 && g.visible(m.x, m.y)).map(|m| ((m.x - g.x).abs().max((m.y - g.y).abs()), m.uid)).collect()).unwrap_or_default();
                seen.sort();
                v.target = match v.target.and_then(|t| seen.iter().position(|s| s.1 == t)) { Some(i) => seen.get(i + 1).or(seen.first()).map(|s| s.1), None => seen.first().map(|s| s.1) };
            }
            if pressed(Key::Space) { if let Some(t) = v.target { act(&mut g, &mut v, Action::Attack(t)); } else { act(&mut g, &mut v, Action::Wait); } }
            let fkeys = [Key::F1, Key::F2, Key::F3, Key::F4, Key::F5, Key::F6, Key::F7, Key::F8, Key::F9];
            for (k, key) in fkeys.iter().enumerate() { if pressed(*key) { let t = v.target; act(&mut g, &mut v, Action::Cast(k, t)); } }
            let digits = [Key::Key1, Key::Key2, Key::Key3, Key::Key4, Key::Key5, Key::Key6, Key::Key7, Key::Key8, Key::Key9];
            for (k, key) in digits.iter().enumerate() { if pressed(*key) { act(&mut g, &mut v, Action::Choose(k)); } }
            if pressed(Key::I) { v.tab = Tab::Pack; }
            if pressed(Key::K) { v.tab = Tab::Skills; }
            if pressed(Key::L) { v.tab = Tab::Quests; }
            if pressed(Key::M) { v.tab = Tab::Spells; }
            // Click-to-walk: a step every ~110 ms while nothing hostile is in sight.
            if let Some((tx, ty)) = v.walk_to {
                if (g.x, g.y) == (tx, ty) || g.monsters_in_sight() > 0 && v.last_step.elapsed() > Duration::from_millis(0) && g.place().map_or(false, |p| p.monsters.iter().any(|m| m.z == g.z && m.hp > 0 && m.awake && g.visible(m.x, m.y))) { v.walk_to = None; }
                else if v.last_step.elapsed() > Duration::from_millis(110) {
                    v.last_step = Instant::now();
                    match step_toward(&g, tx, ty) { Some((dx, dy)) => { let before = (g.x, g.y, g.z, g.here); act(&mut g, &mut v, Action::Move(dx, dy)); if (g.x, g.y, g.z, g.here) == before { v.walk_to = None; } } None => v.walk_to = None }
                }
            }
            // The target dies or leaves sight: let it go.
            if let Some(t) = v.target { if !g.place().map_or(false, |p| p.monsters.iter().any(|m| m.uid == t && m.hp > 0 && m.z == g.z)) { v.target = None; } }
        }
        if pressed(Key::F10) { match g.save(&g.save_path()) { Ok(()) => { let p = g.save_path().display().to_string(); g.say(Tone::Level, format!("Saved to {}.", p)); } Err(e) => g.say(Tone::Danger, format!("Could not save: {}", e)) } }
        if pressed(Key::Slash) { show_help = !show_help; }
        if pressed(Key::Escape) && show_help { show_help = false; }
        else if pressed(Key::Escape) && g.talk.is_none() {
            if esc_once { let _ = g.save(&g.save_path()); break; }
            esc_once = true;
            g.say(Tone::Info, "Press Esc again to leave the adventure (it is saved).");
        } else if window.get_keys_pressed(KeyRepeat::No).iter().any(|k| *k != Key::Escape) { esc_once = false; }
        // Draw.
        let map_w = w.saturating_sub(PANEL_W);
        if g.here.is_none() {
            let mut mbuf = vec![0u32; map_w * h];
            super::render::render_world_cached(&tw, atlas, &v.world_cam, &mut mbuf, map_w, h);
            for y in 0..h { buf[y * w..y * w + map_w].copy_from_slice(&mbuf[y * map_w..(y + 1) * map_w]); }
            v.world_cam = super::render::snap_world_camera(&v.world_cam, map_w, h);
        }
        draw(&g, &mut v, &mut buf, w, h);
        if show_help { draw_help(&mut buf, w, h, w.saturating_sub(PANEL_W)); }
        let title = format!("The Dark Tower — {} — level {}", g.hero.name, g.hero.level);
        window.set_title(&title);
        window.update_with_buffer(&buf, w, h)?;
    }
    Ok(())
}

/// `--adventure-snapshot PREFIX`: frames of the adventure without a window (the town, the
/// sewers, a fight, a place deep down, the world map, a conversation), by the bot's play.
pub fn snapshots(world: &crate::world::WorldData, history: Option<&crate::history::world_state::WorldHistory>, atlas: &super::atlas::Atlas, seed: u64, prefix: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let (w, h) = (1280usize, 800usize);
    let mut g = crate::adventure::new_game(world, history, seed, None);
    let mut v = View::new(&g);
    let mut bot = crate::adventure::bot::Bot::default();
    let mut tw = super::classify::TileWorld::build(world, atlas);
    if let Some(hh) = history { tw.apply_history(world, hh, atlas); }
    tw.set_season(world, crate::seasons::Season::Summer);
    let mut files = Vec::new();
    let shoot = |g: &mut Game, v: &mut View, name: &str, files: &mut Vec<String>| -> Result<(), Box<dyn std::error::Error>> {
        let mut buf = vec![0u32; w * h];
        let map_w = w - PANEL_W;
        take_effects(g, v);
        // Let the eased positions settle.
        v.last_frame = Instant::now() - Duration::from_millis(500);
        if g.here.is_none() {
            v.world_cam = super::render::Camera { cx: g.tile.0 as f32 + 0.5, cy: g.tile.1 as f32 + 0.5, tile_px: 24.0 };
            let mut mbuf = vec![0u32; map_w * h];
            super::render::render_world_cached(&tw, atlas, &v.world_cam, &mut mbuf, map_w, h);
            for y in 0..h { buf[y * w..y * w + map_w].copy_from_slice(&mbuf[y * map_w..(y + 1) * map_w]); }
        }
        let t0 = Instant::now();
        draw(g, v, &mut buf, w, h);
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        let t1 = Instant::now();
        draw(g, v, &mut buf, w, h);
        let ms2 = t1.elapsed().as_secs_f64() * 1000.0;
        let path = format!("{}_{}.png", prefix, name);
        image::RgbImage::from_fn(w as u32, h as u32, |x, y| { let p = buf[y as usize * w + x as usize]; image::Rgb([(p >> 16) as u8, (p >> 8) as u8, p as u8]) }).save(&path)?;
        println!("  {} ({:.1} ms first frame, {:.1} ms kept)", path, ms, ms2);
        files.push(path);
        Ok(())
    };
    // The temple at the start.
    shoot(&mut g, &mut v, "temple", &mut files)?;
    // A talk with the priest.
    if let Some(k) = g.place().and_then(|p| p.npcs.iter().position(|n| n.role == crate::adventure::actor::Role::Priest)) { crate::adventure::npc::greet(&mut g, k); crate::adventure::npc::answer(&mut g, 1); }
    shoot(&mut g, &mut v, "talk", &mut files)?;
    g.talk = None;
    // Play until the sewers, then until a fight, then deeper.
    let mut took = [false; 5];
    for k in 0..60_000 {
        // (Only the last act's effects are shown, as the window would.)
        let _ = g.take_effects();
        if !bot.step(&mut g) { let _ = g.act(Action::Wait); }
        let in_sewer = g.here == Some(g.hero.temple) && g.z >= 1;
        let fighting = g.monsters_in_sight() >= 2 && g.here.is_some();
        if !took[0] && in_sewer && fighting { took[0] = true; shoot(&mut g, &mut v, "sewer", &mut files)?; }
        if !took[1] && g.here.is_none() && k > 2000 { took[1] = true; shoot(&mut g, &mut v, "world", &mut files)?; }
        if !took[2] && g.here.is_some() && g.here != Some(g.hero.temple) && g.z == 0 && g.floor().map_or(false, |f| f.outdoor) && fighting { took[2] = true; shoot(&mut g, &mut v, "surface", &mut files)?; }
        if !took[3] && g.here.is_some() && g.here != Some(g.hero.temple) && g.z >= 1 && fighting { took[3] = true; v.tab = Tab::Skills; shoot(&mut g, &mut v, "deep", &mut files)?; v.tab = Tab::Pack; }
        if !took[4] && g.place().map_or(false, |p| p.monsters.iter().any(|m| m.boss && m.z == g.z && g.visible(m.x, m.y))) { took[4] = true; shoot(&mut g, &mut v, "boss", &mut files)?; }
        if took.iter().all(|t| *t) { break; }
    }
    Ok(files)
}

/// `--adventure-gallery PREFIX`: one floor of every kind of place (the second floor where there
/// is one), wholly revealed, with its monsters, drawn as the window draws it: PREFIX_KIND.png.
pub fn gallery(world: &crate::world::WorldData, history: Option<&crate::history::world_state::WorldHistory>, seed: u64, prefix: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    use crate::adventure::site::{BossSpec, SiteSpec};
    let (w, h) = (1280usize, 800usize);
    let mut files = Vec::new();
    let kinds = [SiteKind::Cave, SiteKind::Lair, SiteKind::Mine, SiteKind::Ruin, SiteKind::Tomb, SiteKind::Temple, SiteKind::Shrine, SiteKind::Castle, SiteKind::Labyrinth, SiteKind::Camp, SiteKind::Halls, SiteKind::DarkFortress, SiteKind::Wilds];
    for (n, kind) in kinds.iter().enumerate() {
        let mut g = crate::adventure::new_game(world, history, seed, None);
        let id = 800_000 + n as u32;
        let floors = if *kind == SiteKind::Wilds { 1 } else { 3 };
        let boss = BossSpec { def: "troll".into(), name: "the Gallery Boss".into(), scale: 1.4, legend: None, hoard: vec![], story: String::new() };
        g.sites.push(SiteSpec { id, kind: *kind, name: format!("{:?}", kind), tile: g.tile, seed: seed ^ (n as u64 * 7919), tier: 3, cause: String::new(), boss: Some(boss), treasures: vec![],
            surface: crate::adventure::map::Ground::Grass, rock: "granite".into(), floors, people: String::new(), god: String::new(), news: Vec::new() });
        g.enter_site(id, true);
        let z = if floors > 1 { 1 } else { 0 };
        g.z = z;
        // Stand where one arrives on that floor, and see it all.
        let p = g.places.get_mut(&id).unwrap();
        let f = &mut p.floors[z];
        let arrive = f.find(|x| matches!(x, Feature::StairsUp | Feature::LadderUp | Feature::RopeSpot)).or_else(|| f.find(|x| matches!(x, Feature::Exit))).unwrap_or((f.w as i32 / 2, f.h as i32 / 2));
        for s in f.seen.iter_mut() { *s = true; }
        g.x = arrive.0; g.y = arrive.1;
        g.hero.torch = 1000;
        g.look();
        let fw = g.places[&id].floors[z].w;
        g.sight = vec![true; fw * g.places[&id].floors[z].h];
        let mut v = View::new(&g);
        v.cs = 22.0;
        let mut buf = vec![0u32; w * h];
        draw(&g, &mut v, &mut buf, w, h);
        let path = format!("{}_{:?}.png", prefix, kind).to_lowercase();
        image::RgbImage::from_fn(w as u32, h as u32, |x, y| { let p = buf[y as usize * w + x as usize]; image::Rgb([(p >> 16) as u8, (p >> 8) as u8, p as u8]) }).save(&path)?;
        files.push(path);
    }
    Ok(files)
}

/// The keys, on a card (? toggles it).
fn draw_help(buf: &mut [u32], w: usize, h: usize, map_w: usize) {
    let lines: [(&str, &str); 17] = [
        ("Arrows, WASD, numpad", "walk; Q E Z C the diagonals; bump into a thing to strike it, open it or talk"),
        ("Mouse", "click to walk there or to strike; click a thing in the pack to use or wear it, right click to drop it"),
        ("Space / Tab", "strike or shoot the target / choose the next target in sight"),
        ("F1 - F9", "cast the spells you know (the Spells tab lists them)"),
        ("G", "take what lies here"),
        ("R", "rest until whole (not with enemies near, not hungry)"),
        ("H / J / F", "drink a health potion / a mana potion / eat"),
        ("< > Enter", "take the stairs, ladder, hole or way out underfoot; Enter on the map goes in"),
        ("1 - 9", "answer in a conversation; choose from a quest chest beside you"),
        ("I K M L", "the Pack, Skills, Spells and Quests tabs"),
        ("Wheel", "zoom"),
        ("F10", "save (leaving saves too)"),
        ("Esc", "close a card; twice to leave"),
        ("Towns", "the priest heals, gives a calling at level 8, teaches spells and blesses"),
        ("", "the smith and the trader buy and sell; the lord, the guard and the sage give work"),
        ("The world", "walk tile by tile; places are named on the map; the farther from towns, the worse"),
        ("Death", "costs a tenth of your experience and half your gold, unless you were blessed"),
    ];
    let cw = 720usize.min(map_w.saturating_sub(40));
    let r = Rect { x: (map_w - cw) / 2, y: 40, w: cw, h: 60 + lines.len() * 24 };
    card(buf, w, r);
    fonts::draw(buf, w, h, (r.x + 24) as f32, (r.y + 14) as f32, "How to play", Face::SmallCaps, 22.0, 0.5, 0x009A_2A1E, None);
    for (k, (key, what)) in lines.iter().enumerate() {
        let y = (r.y + 50 + k * 24) as f32;
        fonts::draw(buf, w, h, (r.x + 24) as f32, y, key, Face::SmallCaps, 15.0, 0.3, 0x0038_2A20, None);
        let t = super::ui::truncate(what, 90);
        fonts::draw(buf, w, h, (r.x + 210) as f32, y, &t, Face::Italic, 14.0, 0.0, 0x005A_4634, None);
    }
}
