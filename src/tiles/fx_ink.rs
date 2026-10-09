//! What happens on the map, drawn as it happens: blows where attackers and settlers meet (sparks,
//! a slash, blood on the ground), a beast's special attack aimed at whom it strikes (fire, a
//! poison cloud, web, frost, a blinding dark), the besiegers' tents and fires, the ghosts of the
//! restless dead walking by their graves at night, the patron's bell ringing over the fire, evil
//! weather (a red rain, a black mist, grey ash, black hail) and snow falling in a deep freeze.

use super::camp_ink::Cells;
use super::ink::{hash, mix, Finish, Pen, Rgb, BLOOD, INK};
use super::render::LocalCamera;
use crate::colony::creatures::CreatureKind;
use crate::colony::Colony;

fn hostile(k: CreatureKind) -> bool { matches!(k, CreatureKind::Raider | CreatureKind::Beast | CreatureKind::Wolf | CreatureKind::CaveHunter | CreatureKind::Besieger) }

/// Blows: between each attacker and the settler it is at (within a cell and a half).
pub fn draw_fights(colony: &Colony, cam: &LocalCamera, put: &mut dyn FnMut(i64, i64, Rgb, f32), w: usize, h: usize) {
    let c = Cells::new(cam, w, h);
    let tick = colony.clock.tick;
    for (k, cr) in colony.creatures.iter().enumerate() {
        if !hostile(cr.kind) || cr.leaving || colony.creature_below(cr) { continue; }
        let Some((i, d)) = colony.settlers.iter().enumerate().filter(|(i, s)| s.alive && !colony.below(*i))
            .map(|(i, s)| (i, ((s.pos.0 as f32 - cr.pos.0 as f32).powi(2) + (s.pos.1 as f32 - cr.pos.1 as f32).powi(2)).sqrt()))
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap()) else { continue };
        if d > 1.6 { continue; }
        let sp = colony.draw_pos(i);
        let (ax, ay) = (cr.pos.0 as f32 + 0.5, cr.pos.1 as f32 + 0.5);
        let (bx, by) = (sp.0 + 0.5, sp.1 + 0.5);
        if !c.visible(ax, ay, w, h, 3.0) { continue; }
        let (mx, my) = ((ax + bx) / 2.0, (ay + by) / 2.0 - 0.3);
        let beat = (tick / 2 + k as u64) % 3;
        // Blood on the ground round the one struck (stays while they fight).
        {
            let mut pen = c.pen(put, bx, by + 0.3);
            for j in 0..6u64 {
                let r = hash(i as i64, j as i64, tick / 20);
                let (u, v) = (((r % 100) as f32 / 100.0 - 0.5) * 0.9, (((r >> 8) % 100) as f32 / 100.0 - 0.5) * 0.5);
                pen.ellipse_f(u, v, 0.05 + (r >> 16) as f32 % 3.0 * 0.015, 0.035, BLOOD, Finish::Paint);
            }
        }
        // The beast's own attack, aimed at its victim.
        if cr.kind == CreatureKind::Beast {
            let effect = colony.arc.as_ref().filter(|a| a.threat.name == cr.name).and_then(|a| a.threat.monster.as_ref())
                .or_else(|| colony.map.caverns.iter().filter_map(|cv| cv.beast.as_ref()).find(|(n, _)| *n == cr.name).map(|(_, m)| m))
                .and_then(|m| m.attack.as_ref()).map(|a| a.effect.clone()).unwrap_or_default();
            if beat != 1 && !effect.is_empty() { breath(&mut c.pen(put, ax, ay - 0.5), &effect, bx - ax, by - ay, tick); }
        }
        // The blow: a spark where the weapons meet, a slash across the victim.
        let mut pen = c.pen(put, mx, my);
        if beat == 0 {
            for a in 0..6 { let ang = a as f32 * 1.047 + (tick % 7) as f32 * 0.2; pen.line((0.0, 0.0), (ang.cos() * 0.32, ang.sin() * 0.32), [250.0, 236.0, 180.0], 1.5); }
            pen.glow(0.0, 0.0, 0.35, [255.0, 240.0, 190.0], 0.7);
        } else if beat == 1 {
            let (dx, dy) = (bx - mx, by - my);
            pen.path(&[(dx - 0.35, dy - 0.3), (dx, dy - 0.05), (dx + 0.32, dy + 0.3)], [250.0, 246.0, 236.0], 2.0);
            pen.path(&[(dx - 0.3, dy - 0.32), (dx + 0.28, dy + 0.26)], BLOOD, 1.0);
        }
    }
}

/// A beast's special attack, from its mouth (the pen's origin) towards (dx, dy) cells.
fn breath(pen: &mut Pen, effect: &str, dx: f32, dy: f32, tick: u64) {
    let len = (dx * dx + dy * dy).sqrt().max(0.5);
    let (ux, uy) = (dx / len, dy / len);
    let (colour, core): (Rgb, Rgb) = match effect {
        "burn" => ([226.0, 110.0, 40.0], [250.0, 210.0, 110.0]),
        "poison" | "sicken" => ([150.0, 170.0, 70.0], [196.0, 206.0, 120.0]),
        "chill" => ([170.0, 206.0, 226.0], [236.0, 246.0, 250.0]),
        "blind" => ([60.0, 56.0, 64.0], [100.0, 96.0, 104.0]),
        "web" => ([236.0, 236.0, 230.0], [255.0, 255.0, 250.0]),
        _ => ([170.0, 40.0, 36.0], [210.0, 80.0, 70.0]),
    };
    if effect == "web" {
        for k in 0..5 {
            let a = (k as f32 - 2.0) * 0.18;
            let (ex, ey) = ((ux * a.cos() - uy * a.sin()) * len * 1.1, (ux * a.sin() + uy * a.cos()) * len * 1.1);
            pen.line_a((0.0, 0.0), (ex, ey), colour, 1.0, 0.9);
        }
        for r in [0.4f32, 0.75] { for k in 0..4 { let a0 = (k as f32 - 2.0) * 0.18; let a1 = a0 + 0.18;
            let p0 = ((ux * a0.cos() - uy * a0.sin()) * len * r, (ux * a0.sin() + uy * a0.cos()) * len * r);
            let p1 = ((ux * a1.cos() - uy * a1.sin()) * len * r, (ux * a1.sin() + uy * a1.cos()) * len * r);
            pen.line_a(p0, p1, colour, 1.0, 0.8); } }
        return;
    }
    // A cone of puffs widening towards the victim.
    for k in 0..9 {
        let t = (k as f32 + (tick % 3) as f32 / 3.0) / 9.0;
        let side = ((k * 37 % 11) as f32 / 11.0 - 0.5) * t * 0.7;
        let (px, py) = (ux * len * 1.1 * t - uy * side, uy * len * 1.1 * t + ux * side);
        let r = 0.1 + t * 0.32;
        pen.glow(px, py, r * 1.4, colour, 0.75 - t * 0.3);
        if t < 0.6 { pen.glow(px, py, r * 0.6, core, 0.6); }
    }
}

/// The besiegers' camp: tents and a fire round where their fires are.
pub fn draw_siege(colony: &Colony, cam: &LocalCamera, put: &mut dyn FnMut(i64, i64, Rgb, f32), w: usize, h: usize) {
    let Some(s) = colony.siege.as_ref() else { return };
    if colony.clock.tick > s.until { return; }
    let c = Cells::new(cam, w, h);
    let (field, charge) = super::folk::band_colours(&s.who);
    for (k, (dx, dy)) in [(-2.5f32, -1.5f32), (2.0, -2.0), (-1.0, 2.0), (2.6, 1.4)].iter().enumerate() {
        let (x, y) = (s.at.0 as f32 + dx, s.at.1 as f32 + dy);
        if !c.visible(x, y, w, h, 3.0) { continue; }
        let mut pen = c.pen(put, x, y);
        pen.ground_shadow(0.6, 1.0, 0.8, 0.15);
        let cloth = if k % 2 == 0 { mix(field, [220.0, 210.0, 190.0], 0.4) } else { [200.0, 188.0, 160.0] };
        pen.poly(&[(-0.2, 1.0), (0.6, -0.2), (1.4, 1.0)], cloth);
        pen.poly(&[(0.45, 1.0), (0.6, 0.35), (0.75, 1.0)], [60.0, 50.0, 46.0]);
        pen.bone(&[(0.6, -0.2), (0.6, -0.65)], [122.0, 86.0, 54.0], 1.0);
        pen.poly(&[(0.6, -0.65), (0.95, -0.55), (0.6, -0.45)], charge);
    }
    // Their fire.
    let (x, y) = (s.at.0 as f32 + 0.5, s.at.1 as f32 + 0.5);
    if c.visible(x, y, w, h, 2.0) {
        let mut pen = c.pen(put, x, y);
        pen.glow(0.0, 0.0, 0.9, [250.0, 170.0, 70.0], 0.5);
        pen.limb((-0.25, 0.12), 0.05, (0.25, -0.05), 0.05, [122.0, 86.0, 54.0]);
        pen.limb((-0.22, -0.05), 0.05, (0.25, 0.12), 0.05, [122.0, 86.0, 54.0]);
        let f = ((colony.clock.tick as f32).sin() * 0.5 + 0.5) * 0.12;
        pen.poly(&[(-0.14, 0.04), (0.0, -0.38 - f), (0.14, 0.04)], [226.0, 120.0, 46.0]);
    }
}

/// The restless dead at night: a pale figure drifting by its grave, a wisp beneath it.
pub fn draw_ghosts(colony: &Colony, cam: &LocalCamera, put: &mut dyn FnMut(i64, i64, Rgb, f32), w: usize, h: usize, scale: f32, labels: &mut Vec<(f32, f32, String)>) {
    if !colony.clock.is_night() { return; }
    let c = Cells::new(cam, w, h);
    let tick = colony.clock.tick;
    for (k, r) in colony.restless.iter().enumerate().filter(|(_, r)| !r.at_rest) {
        let Some(s) = colony.settlers.get(r.who) else { continue };
        let grave = colony.marks.iter().find(|m| m.kind == crate::colony::MarkKind::Grave && m.title == format!("The grave of {}", s.name)).map(|m| m.at).unwrap_or(colony.camp);
        let ph = tick as f32 / 50.0 + k as f32 * 2.0;
        let (x, y) = (grave.0 as f32 + 0.5 + ph.sin() * 1.6, grave.1 as f32 + 0.5 + (ph * 0.7).cos() * 1.0 - 0.6);
        if !c.visible(x, y, w, h, 2.0) { continue; }
        let (sx, sy) = (c.x0 + x * c.t, c.y0 + y * c.t);
        {
            let mut pen = Pen::new(put, sx, sy + 8.0 * scale, 22.0 * scale).faint(0.55);
            pen.glow(0.0, -0.3, 1.2, [200.0, 230.0, 220.0], 0.35);
            pen.poly_f(&[(-0.45, 0.3), (0.0, 1.2), (0.12, 0.7), (0.3, 1.0), (0.45, 0.3)], [214.0, 230.0, 222.0], Finish::Paint);
        }
        let f = super::folk::Folk { skin: [214.0, 226.0, 220.0], hair: [190.0, 200.0, 196.0], dress: [196.0, 214.0, 208.0], helm: super::folk::Helm::None, arm: super::folk::Arm::None,
            shield: None, tusks: false, pointed: false, beard: false, glow: Some([170.0, 240.0, 220.0]), pale: true };
        let top = super::folk::draw(put, &f, sx, sy, scale, (ph.cos()) < 0.0, false, 0.6);
        labels.push((sx, top - 4.0, format!("the ghost of {}", s.name)));
    }
}

/// The patron's bell, ringing over the fire.
pub fn draw_bell(colony: &Colony, cam: &LocalCamera, put: &mut dyn FnMut(i64, i64, Rgb, f32), w: usize, h: usize) {
    if !colony.bell_rung() { return; }
    let c = Cells::new(cam, w, h);
    let (x, y) = (colony.camp.0 as f32 + 0.5, colony.camp.1 as f32 - 2.2);
    if !c.visible(x, y, w, h, 2.0) { return; }
    let swing = ((colony.clock.tick as f32) * 0.9).sin() * 0.25;
    let mut pen = Pen::new(put, c.x0 + x * c.t, c.y0 + y * c.t, (c.t * 2.4).max(26.0));
    let bronze = [196.0, 150.0, 70.0];
    pen.shape(bronze, Finish::Inked, [-0.5, -0.5, 0.5, 0.5], &move |u, v| {
        let (a, b) = (u * swing.cos() - v * swing.sin(), u * swing.sin() + v * swing.cos());
        b > -0.42 && b < 0.32 && a.abs() < 0.16 + (b + 0.42) * 0.38
    });
    pen.ellipse(-0.42 * swing.sin(), 0.38, 0.07, 0.07, [120.0, 90.0, 50.0]);
    for r in [0.5f32, 0.66] { for s in [-1.0f32, 1.0] { pen.path(&[(s * r, -0.25), (s * (r + 0.08), 0.0), (s * r, 0.25)], INK, 1.2); } }
}

/// Evil weather over the whole frame, and snow in a deep freeze.
pub fn draw_weather(colony: &Colony, buf: &mut [u32], w: usize, h: usize, mask: Option<&[bool]>) {
    let tick = colony.clock.tick;
    let kind = colony.evil_weather_over();
    let snow = colony.frozen();
    if kind.is_none() && !snow { return; }
    let shows = |k: usize| mask.map_or(true, |m| m[k]);
    let blend = |p: u32, c: Rgb, a: f32| -> u32 {
        let old = [((p >> 16) & 0xFF) as f32, ((p >> 8) & 0xFF) as f32, (p & 0xFF) as f32];
        super::ink::pack(mix(old, c, a))
    };
    let shift = (tick * 7) as i64;
    for y in 0..h {
        for x in 0..w {
            let k = y * w + x;
            if !shows(k) { continue; }
            let (xi, yi) = (x as i64, y as i64);
            match kind {
                Some(0) => {
                    // A red rain: slanting streaks, a red cast.
                    buf[k] = blend(buf[k], [150.0, 60.0, 50.0], 0.12);
                    let s = (xi * 2 + yi + shift * 3).rem_euclid(97);
                    if s < 2 && hash(xi / 3, (yi - xi * 2 / 3 + shift) / 9, 0xA1) % 5 == 0 { buf[k] = blend(buf[k], [150.0, 36.0, 32.0], 0.7); }
                }
                Some(1) => {
                    // A black mist rolling in low: dark banks.
                    let n = ((xi as f32 / 90.0 + tick as f32 * 0.02).sin() + (yi as f32 / 60.0 - tick as f32 * 0.013).cos()) * 0.25 + 0.5;
                    buf[k] = blend(buf[k], [36.0, 32.0, 40.0], 0.25 + 0.3 * n);
                }
                Some(2) => {
                    // Grey ash settling like snow.
                    buf[k] = blend(buf[k], [150.0, 146.0, 140.0], 0.18);
                    if hash(xi / 2, (yi - shift / 2) / 2, 0xA5) % 140 == 0 { buf[k] = blend(buf[k], [110.0, 106.0, 102.0], 0.8); }
                }
                Some(_) => {
                    // Black hail.
                    buf[k] = blend(buf[k], [60.0, 66.0, 80.0], 0.12);
                    if hash(xi / 2, (yi - shift * 2) / 3, 0xA7) % 160 == 0 { buf[k] = blend(buf[k], [30.0, 30.0, 40.0], 0.9); }
                }
                None => {
                    if hash(xi / 2, (yi - shift / 3) / 2, 0x5A0) % 220 == 0 { buf[k] = blend(buf[k], [250.0, 250.0, 248.0], 0.85); }
                }
            }
        }
    }
}

/// Where the last clash was fought: for its first hour a melee (dust, flashing blades, blood
/// flying), then for a day its aftermath (trampled ground, blood, a broken spear, arrows).
pub fn draw_clash(colony: &Colony, cam: &LocalCamera, put: &mut dyn FnMut(i64, i64, Rgb, f32), w: usize, h: usize) {
    let Some(at) = colony.clash_at else { return };
    let tick = colony.clock.tick;
    let age = tick.saturating_sub(colony.clash_tick);
    if colony.clash_tick == 0 || age > crate::colony::TICKS_PER_DAY { return; }
    let c = Cells::new(cam, w, h);
    let (x, y) = (at.0 as f32 + 0.5, at.1 as f32 + 0.5);
    if !c.visible(x, y, w, h, 4.0) { return; }
    let fade = 1.0 - age as f32 / crate::colony::TICKS_PER_DAY as f32 * 0.6;
    let mut pen = c.pen(put, x, y).faint(fade);
    // Trampled ground and blood.
    pen.ellipse_f(0.0, 0.0, 1.8, 1.2, [150.0, 128.0, 96.0], Finish::Paint);
    for j in 0..14i64 {
        let r = hash(at.0 as i64, j, colony.clash_tick);
        let (u, v) = (((r % 1000) as f32 / 1000.0 - 0.5) * 3.0, (((r >> 10) % 1000) as f32 / 1000.0 - 0.5) * 2.0);
        let s = 0.06 + ((r >> 20) % 5) as f32 * 0.03;
        pen.ellipse_f(u, v, s * 1.4, s, BLOOD, Finish::Paint);
    }
    // A broken spear, two arrows.
    pen.bone(&[(-0.9, 0.5), (-0.2, 0.2)], [120.0, 84.0, 50.0], (pen.half * 0.04).max(1.0));
    pen.poly(&[(-0.2, 0.14), (0.05, 0.12), (-0.18, 0.28)], [176.0, 178.0, 184.0]);
    pen.bone(&[(0.6, -0.6), (0.95, -0.2)], [150.0, 120.0, 80.0], 1.0);
    pen.bone(&[(1.1, 0.4), (1.3, 0.8)], [150.0, 120.0, 80.0], 1.0);
    if age < 60 {
        // The melee: a cloud of dust and blades flashing in it.
        let mut pen = c.pen(put, x, y - 0.4);
        for k in 0..7 {
            let a = k as f32 * 0.9 + tick as f32 * 0.3;
            pen.glow(a.cos() * 0.9, a.sin() * 0.5, 0.8, [196.0, 176.0, 140.0], 0.45);
        }
        for k in 0..4u64 {
            if (tick / 2 + k) % 3 != 0 { continue; }
            let r = hash(k as i64, (tick / 2) as i64, 0xB1A);
            let (u, v) = (((r % 100) as f32 / 100.0 - 0.5) * 1.8, (((r >> 8) % 100) as f32 / 100.0 - 0.5) * 1.0);
            for a in 0..6 { let ang = a as f32 * 1.047; pen.line((u, v), (u + ang.cos() * 0.28, v + ang.sin() * 0.28), [252.0, 240.0, 190.0], 1.5); }
            pen.glow(u, v, 0.3, [255.0, 240.0, 190.0], 0.7);
        }
    }
}

/// A prisoner held by the camp (`prisoners.rs`): one of their people, unarmed, bound to a post
/// near the fire with their name over them.
pub fn draw_prisoner(colony: &Colony, cam: &LocalCamera, put: &mut dyn FnMut(i64, i64, Rgb, f32), w: usize, h: usize, scale: f32, labels: &mut Vec<(f32, f32, String)>) {
    let Some(pr) = colony.prisoner.as_ref() else { return };
    let c = Cells::new(cam, w, h);
    let (x, y) = (colony.camp.0 as f32 - 2.5, colony.camp.1 as f32 + 2.5);
    if !c.visible(x, y, w, h, 2.0) { return; }
    let (sx, sy) = (c.x0 + x * c.t, c.y0 + y * c.t);
    {
        let mut pen = Pen::new(put, sx, sy, 22.0 * scale);
        pen.bone(&[(0.0, 0.7), (0.0, -1.1)], [122.0, 86.0, 54.0], (scale * 2.0).max(1.5));
    }
    let mut f = super::folk::raider(None, &format!("a war band of {}", pr.people), 9, None);
    f.arm = super::folk::Arm::None;
    f.shield = None;
    f.helm = super::folk::Helm::None;
    let top = super::folk::draw(put, &f, sx, sy, scale, false, false, 1.0);
    let mut pen = Pen::new(put, sx, sy, 22.0 * scale);
    for v in [0.05f32, 0.3] { pen.line((-0.6, v), (0.6, v + 0.05), [196.0, 170.0, 120.0], (scale * 1.4).max(1.0)); }
    labels.push((sx, top - 4.0, format!("{}, a prisoner", pr.name)));
}

/// The stocks round a settler serving their sentence: a plank with holes over their shoulders on
/// two posts.
pub fn draw_stocks(put: &mut dyn FnMut(i64, i64, Rgb, f32), x: f32, y: f32, scale: f32) {
    let mut pen = Pen::new(put, x, y, 22.0 * scale);
    for u in [-0.75f32, 0.75] { pen.bone(&[(u, 0.75), (u, -0.15)], [122.0, 86.0, 54.0], (scale * 2.0).max(1.5)); }
    pen.rect(-0.85, -0.12, 0.85, 0.08, [164.0, 120.0, 78.0]);
    for u in [-0.45f32, 0.45] { pen.ellipse_f(u, -0.02, 0.08, 0.06, INK, Finish::Paint); }
}

/// Settlers away from the map (an expedition against a beast, others off on the roads): a
/// signpost by the camp naming who went and why, with a small banner when it is a war party.
pub fn draw_away_sign(colony: &Colony, cam: &LocalCamera, put: &mut dyn FnMut(i64, i64, Rgb, f32), w: usize, h: usize, labels: &mut Vec<(f32, f32, String)>) {
    let tick = colony.clock.tick;
    let away: Vec<usize> = colony.settlers.iter().enumerate().filter(|(_, s)| s.alive && s.away_until > tick).map(|(i, _)| i).collect();
    let party = colony.expedition.as_ref();
    if away.is_empty() && party.is_none() { return; }
    let c = Cells::new(cam, w, h);
    let (x, y) = (colony.camp.0 as f32 + 3.5, colony.camp.1 as f32 + 2.5);
    if !c.visible(x, y, w, h, 2.0) { return; }
    let mut pen = c.pen(put, x, y);
    pen.bone(&[(0.0, 0.9), (0.0, -0.9)], DARKWOOD, (pen.half * 0.08).max(1.5));
    pen.poly(&[(0.05, -0.85), (1.0, -0.85), (1.2, -0.65), (1.0, -0.45), (0.05, -0.45)], [196.0, 170.0, 120.0]);
    pen.line((0.2, -0.65), (0.9, -0.65), INK, 1.0);
    if party.is_some() { pen.poly(&[(-0.02, -0.9), (-0.6, -0.8), (-0.45, -0.65), (-0.6, -0.5), (-0.02, -0.5)], [150.0, 52.0, 44.0]); }
    let n = party.map_or(away.len(), |p| p.party.len().max(away.len()));
    let text = match party {
        Some(p) => format!("{} gone after {}", n, p.beast),
        None => format!("{} away on the roads", n),
    };
    labels.push((c.x0 + (x + 0.6) * c.t, c.y0 + (y - 0.95) * c.t - 2.0, text));
}
const DARKWOOD: Rgb = [122.0, 86.0, 54.0];
