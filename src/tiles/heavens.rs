//! The heavens card (H on the world map, `--heavens-snapshot FILE`): the world's star system as
//! an ink orrery on parchment (orbits on a log scale, the habitable zone washed green, the snow
//! line dashed, belts stippled, each planet lit from its star, the world in rubric with its
//! moons), a globe showing the tilt and the day, and the account of the heavens
//! (`astro::StarSystem::account`) set in IM Fell beside them.

use std::error::Error;
use std::f32::consts::{PI, TAU};

use crate::astro::{PlanetKind, StarSystem};
use crate::world::WorldData;

use super::fonts::{self, Face};
use super::ui::*;

fn rgb(c: [f32; 3]) -> u32 { super::ink::pack(c) }

/// A filled disc, lit on the side toward `light` (a unit direction), the far side hatched.
fn lit_disc(buf: &mut [u32], w: usize, h: usize, cx: f32, cy: f32, r: f32, col: u32, light: (f32, f32), bands: bool) {
    let ri = r.ceil() as i64 + 1;
    for dy in -ri..=ri {
        for dx in -ri..=ri {
            let (fx, fy) = (dx as f32, dy as f32);
            let d = (fx * fx + fy * fy).sqrt();
            if d > r + 0.5 { continue; }
            let (x, y) = (cx as i64 + dx, cy as i64 + dy);
            if d > r - 0.8 { blend_px(buf, w, h, x, y, INK, 0.9); continue; }
            let mut c = col;
            if bands && ((fy / r * 4.0).floor() as i64).rem_euclid(2) == 0 { c = mix(c, 0x0080_5A3A, 0.25); }
            let side = (fx * light.0 + fy * light.1) / r.max(1.0);
            if side < -0.15 {
                c = mix(c, INK, 0.35);
                if (dx - dy).rem_euclid(3) == 0 { c = mix(c, INK, 0.4); }
            } else if side > 0.55 {
                c = mix(c, 0x00FF_FAF0, 0.25);
            }
            blend_px(buf, w, h, x, y, c, 1.0);
        }
    }
}

/// A circle of radius r, dashed when `dash` > 0 (segment length in px).
fn circle(buf: &mut [u32], w: usize, h: usize, cx: f32, cy: f32, r: f32, col: u32, alpha: f32, dash: f32) {
    let steps = (TAU * r).max(16.0) as usize;
    for i in 0..steps {
        let s = i as f32;
        if dash > 0.0 && ((s / dash) as usize) % 2 == 1 { continue; }
        let a = s / steps as f32 * TAU;
        blend_px(buf, w, h, (cx + a.cos() * r) as i64, (cy + a.sin() * r) as i64, col, alpha);
    }
}

fn kind_colour(k: PlanetKind) -> [f32; 3] {
    match k {
        PlanetKind::Lava => [196.0, 92.0, 52.0],
        PlanetKind::Hothouse => [222.0, 202.0, 146.0],
        PlanetKind::Barren => [168.0, 160.0, 150.0],
        PlanetKind::ColdDesert => [190.0, 122.0, 92.0],
        PlanetKind::Ice => [222.0, 230.0, 234.0],
        PlanetKind::SuperEarth => [150.0, 142.0, 120.0],
        PlanetKind::MiniNeptune => [150.0, 190.0, 202.0],
        PlanetKind::IceGiant => [128.0, 178.0, 198.0],
        PlanetKind::GasGiant => [206.0, 172.0, 122.0],
        PlanetKind::Home => [88.0, 138.0, 168.0],
    }
}

/// Where along its orbit a planet is drawn: a fixed angle from its name, the world up and right.
fn orbit_angle(name: &str, home: bool) -> f32 {
    if home { return -0.65; }
    let h = name.bytes().fold(0x811C_9DC5u32, |a, b| (a ^ b as u32).wrapping_mul(0x0100_0193));
    (h % 6283) as f32 / 1000.0
}

/// Letters a label at the first of a few spots beside (px, py) that is free of the others.
fn place_label(placed: &mut Vec<(f32, f32, f32, f32)>, buf: &mut [u32], w: usize, h: usize, px: f32, py: f32, gap: f32, text: &str, face: Face, size: f32, col: u32) {
    let tw = fonts::width(text, face, size, if face == Face::SmallCaps { 0.5 } else { 0.0 });
    let th = size + 2.0;
    let spots = [(px + gap, py - th * 0.5), (px - gap - tw, py - th * 0.5), (px - tw * 0.5, py + gap), (px - tw * 0.5, py - gap - th), (px + gap, py + gap * 0.5), (px + gap, py - gap - th)];
    for (x, y) in spots {
        let free = placed.iter().all(|&(ax, ay, bx, by)| x + tw < ax || x > bx || y + th < ay || y > by);
        if free {
            placed.push((x - 2.0, y - 1.0, x + tw + 2.0, y + th + 1.0));
            fonts::draw(buf, w, h, x, y, text, face, size, if face == Face::SmallCaps { 0.5 } else { 0.0 }, col, Some(PAPER));
            return;
        }
    }
}

/// The orrery in a square of side `size` centred at (ox, oy).
fn orrery(sys: &StarSystem, buf: &mut [u32], w: usize, h: usize, ox: f32, oy: f32, size: f32) {
    let radius = size / 2.0;
    let host = sys.host();
    let (hz_in, hz_out) = host.hz_au();
    let mut lo = hz_in;
    let mut hi = hz_out;
    for p in &sys.planets { lo = lo.min(p.semi_major_au); hi = hi.max(p.semi_major_au); }
    for b in &sys.belts { hi = hi.max(b.outer_au); }
    let (lo, hi) = ((lo * 0.75).ln(), (hi * 1.08).ln());
    let star_r = (9.0 + 7.0 * sys.star.radius_rsun.powf(0.4)).min(radius * 0.12);
    let r_of = |a: f32| star_r + 10.0 + (radius - star_r - 16.0) * ((a.ln() - lo) / (hi - lo)).clamp(0.0, 1.0);

    // The habitable zone, washed green between its edges.
    let (r0, r1) = (r_of(hz_in), r_of(hz_out));
    let ri = r1.ceil() as i64 + 1;
    for dy in -ri..=ri {
        for dx in -ri..=ri {
            let d = ((dx * dx + dy * dy) as f32).sqrt();
            if d >= r0 && d <= r1 { blend_px(buf, w, h, ox as i64 + dx, oy as i64 + dy, MOSS, 0.16); }
        }
    }
    circle(buf, w, h, ox, oy, r0, MOSS, 0.8, 3.0);
    circle(buf, w, h, ox, oy, r1, MOSS, 0.8, 3.0);
    let mut placed: Vec<(f32, f32, f32, f32)> = Vec::new();
    // The planets' discs first, so no label covers one.
    for p in &sys.planets {
        let home = p.kind == PlanetKind::Home;
        let ang = orbit_angle(&p.name, home);
        let r = r_of(p.semi_major_au);
        let (px, py) = (ox + ang.cos() * r, oy + ang.sin() * r);
        placed.push((px - 7.0, py - 7.0, px + 7.0, py + 7.0));
    }
    placed.push((ox - star_r, oy - star_r, ox + star_r, oy + star_r));
    place_label(&mut placed, buf, w, h, ox, oy - r1 - 4.0, 4.0, "the habitable zone", Face::Italic, 13.0, MOSS);
    // The snow line.
    let snow = r_of(host.snow_line_au());
    circle(buf, w, h, ox, oy, snow, SEA, 0.7, 6.0);
    let snow_label = (ox + snow * 0.7071, oy + snow * 0.7071);
    // Belts: stippled rings.
    for (bi, b) in sys.belts.iter().enumerate() {
        let (a, c) = (r_of(b.inner_au), r_of(b.outer_au));
        let n = ((c * c - a * a) * 0.05) as usize + 60;
        for i in 0..n {
            let hsh = hash(i, bi * 977 + 13);
            let ang = (hsh & 0xFFFF) as f32 / 65535.0 * TAU;
            let t = ((hsh >> 16) & 0xFFFF) as f32 / 65535.0;
            let rr = a + (c - a) * t;
            blend_px(buf, w, h, (ox + ang.cos() * rr) as i64, (oy + ang.sin() * rr) as i64, if b.icy { SEA } else { INK_FADED }, 0.75);
        }
    }
    // Orbits.
    for p in &sys.planets {
        let home = p.kind == PlanetKind::Home;
        circle(buf, w, h, ox, oy, r_of(p.semi_major_au), if home { RUBRIC } else { INK_FADED }, if home { 0.9 } else { 0.55 }, 0.0);
    }
    // The star: a glow, then the disc in its own colour.
    let star_col = rgb(sys.star.paint_rgb());
    let gi = (star_r * 2.6) as i64;
    for dy in -gi..=gi {
        for dx in -gi..=gi {
            let d = ((dx * dx + dy * dy) as f32).sqrt() / (star_r * 2.6);
            if d < 1.0 { blend_px(buf, w, h, ox as i64 + dx, oy as i64 + dy, star_col, (1.0 - d).powi(2) * 0.55); }
        }
    }
    // A close pair: two discs at the centre, the lesser beside the first; a wide companion: a
    // small star at the orrery's edge, pointed to.
    let pair = sys.companion.as_ref().filter(|c| c.circumbinary);
    let (sx, cx2) = if pair.is_some() { (ox - star_r * 0.55, ox + star_r * 0.95) } else { (ox, ox) };
    lit_disc(buf, w, h, sx, oy, if pair.is_some() { star_r * 0.8 } else { star_r }, star_col, (0.0, 0.0), false);
    if let Some(c) = &sys.companion {
        let col = rgb(c.star.paint_rgb());
        let r2 = (star_r * 0.8 * (c.star.radius_rsun / sys.star.radius_rsun).powf(0.4)).max(3.0);
        if c.circumbinary {
            lit_disc(buf, w, h, cx2, oy + star_r * 0.35, r2, col, (0.0, 0.0), false);
        } else {
            let (ex, ey) = (ox + radius * 0.86, oy - radius * 0.86);
            for i in 0..40 {
                let t = i as f32 / 40.0;
                if i % 3 != 0 { blend_px(buf, w, h, (ox + (ex - ox) * (0.55 + 0.45 * t)) as i64, (oy + (ey - oy) * (0.55 + 0.45 * t)) as i64, INK_FADED, 0.8); }
            }
            lit_disc(buf, w, h, ex, ey, 4.5, col, (0.0, 0.0), false);
            let label = format!("{}, {} AU", c.star.name, crate::astro::sky::round_sig(c.separation_au));
            fonts::draw(buf, w, h, ex - fonts::width(&label, Face::Italic, 12.0, 0.0) - 8.0, ey - 6.0, &label, Face::Italic, 12.0, 0.0, INK, Some(PAPER));
        }
    }
    // The planets, lit from the star, and their names.
    for p in &sys.planets {
        let home = p.kind == PlanetKind::Home;
        let r = r_of(p.semi_major_au);
        let ang = orbit_angle(&p.name, home);
        let (px, py) = (ox + ang.cos() * r, oy + ang.sin() * r);
        let size = if home { 5.5 } else if p.kind.is_giant() { (3.5 + 1.4 * p.radius_earth.sqrt()).min(9.0) } else { (1.8 + 1.6 * p.radius_earth.sqrt()).min(5.0) };
        let light = (-ang.cos(), -ang.sin());
        if p.rings {
            for i in 0..90 {
                let a = i as f32 / 90.0 * TAU;
                blend_px(buf, w, h, (px + a.cos() * size * 2.1) as i64, (py + a.sin() * size * 0.7) as i64, INK, 0.8);
            }
        }
        lit_disc(buf, w, h, px, py, size, rgb(kind_colour(p.kind)), light, p.kind == PlanetKind::GasGiant);
        if home {
            for (k, m) in sys.world.moons.iter().enumerate() {
                let ma = k as f32 * 2.1 + 0.9;
                let md = size + 4.0 + 3.0 * k as f32;
                lit_disc(buf, w, h, px + ma.cos() * md, py + ma.sin() * md, if m.mass_moons > 0.2 { 2.0 } else { 1.2 }, 0x00C8_C0B4, light, false);
            }
        }
    }
    // Names: the world and the star first, then the planets outward, then the snow line.
    for p in sys.planets.iter().filter(|p| p.kind == PlanetKind::Home) {
        let ang = orbit_angle(&p.name, true);
        let r = r_of(p.semi_major_au);
        place_label(&mut placed, buf, w, h, ox + ang.cos() * r, oy + ang.sin() * r, 12.0, &p.name, Face::SmallCaps, 15.0, RUBRIC);
    }
    let star_label = match pair { Some(c) => format!("{} and {}", sys.star.name, c.star.name), None => sys.star.name.clone() };
    place_label(&mut placed, buf, w, h, ox, oy, star_r + 5.0, &star_label, Face::SmallCaps, 14.0, INK);
    for p in sys.planets.iter().filter(|p| p.kind != PlanetKind::Home) {
        let ang = orbit_angle(&p.name, false);
        let r = r_of(p.semi_major_au);
        place_label(&mut placed, buf, w, h, ox + ang.cos() * r, oy + ang.sin() * r, 8.0, &p.name, Face::Italic, 12.0, INK);
    }
    place_label(&mut placed, buf, w, h, snow_label.0, snow_label.1, 3.0, "snow line", Face::Italic, 12.0, SEA);
}

/// The world as a globe: its axis tilted toward the star, the day side lit; a locked world's
/// substellar point marked.
fn globe(sys: &StarSystem, buf: &mut [u32], w: usize, h: usize, cx: f32, cy: f32, r: f32) {
    let tilt = sys.world.spin.obliquity_deg.to_radians();
    lit_disc(buf, w, h, cx, cy, r, rgb(kind_colour(PlanetKind::Home)), (-1.0, 0.0), false);
    // The star's rays from the left.
    for k in -2..=2 {
        let y = cy + k as f32 * r * 0.35;
        for x in 0..(r * 0.9) as i64 { if x % 6 < 4 { blend_px(buf, w, h, (cx - r * 2.2) as i64 + x, y as i64, GOLD, 0.8); } }
    }
    // The axis and the equator.
    let (ax, ay) = (tilt.sin(), -tilt.cos());
    for i in -((r * 1.35) as i64)..=((r * 1.35) as i64) {
        let t = i as f32;
        blend_px(buf, w, h, (cx + ax * t) as i64, (cy + ay * t) as i64, INK, 0.95);
    }
    for i in -(r as i64)..=(r as i64) {
        let t = i as f32 * 0.97;
        blend_px(buf, w, h, (cx - ay * t) as i64, (cy + ax * t) as i64, RUBRIC, 0.7);
    }
    if sys.world.spin.locked {
        lit_disc(buf, w, h, cx - r * 0.75, cy, 2.5, GOLD, (-1.0, 0.0), false);
    }
}

/// Draw the card over the frame.
pub fn draw(sys: &StarSystem, buf: &mut [u32], w: usize, h: usize) {
    let cw = 1100.min(w.saturating_sub(24));
    let ch = 760.min(h.saturating_sub(24));
    if cw < 500 || ch < 360 { return; }
    let c = Rect { x: (w - cw) / 2, y: (h - ch) / 2, w: cw, h: ch };
    card(buf, w, c);
    let x0 = c.x as f32 + 28.0;
    let mut y = c.y as f32 + 18.0;
    fonts::draw(buf, w, h, x0, y, &format!("The Heavens of {}", sys.world.name), Face::SmallCaps, 30.0, 1.2, RUBRIC, None);
    y += 38.0;
    let a_an = crate::astro::sky::a_an;
    let sub = match &sys.companion {
        Some(c) if c.circumbinary => format!("{} and {}, {} {} and {} {}, and their {} worlds", sys.star.name, c.star.name, a_an(&sys.star.spectral.label()), sys.star.spectral.label(), a_an(&c.star.spectral.label()), c.star.spectral.label(), sys.planets.len()),
        _ => format!("{}, {} {} {}, and its {} worlds", sys.star.name, a_an(&sys.star.spectral.label()), sys.star.spectral.label(), sys.star.spectral.class.dwarf_word(), sys.planets.len()),
    };
    fonts::draw(buf, w, h, x0, y, &sub, Face::Italic, 17.0, 0.0, INK_FADED, None);
    y += 28.0;
    hline(buf, w, x0 as usize, c.x + c.w - 28, y as usize, INK);
    hline(buf, w, x0 as usize, c.x + c.w - 28, y as usize + 2, INK_FADED);
    y += 14.0;

    // Left: the orrery, the globe and the neighbours; right: the account.
    let left_w = (cw as f32 * 0.42).min(470.0);
    let neighbours = sys.neighbour_lines();
    let list_h = 16.0 * neighbours.len() as f32 + 8.0;
    let orrery_size = left_w.min(ch as f32 - (y - c.y as f32) - 110.0 - list_h).max(200.0);
    orrery(sys, buf, w, h, x0 + left_w / 2.0, y + orrery_size / 2.0, orrery_size);
    let gy = y + orrery_size + 36.0;
    let mut ly = gy + 40.0;
    let bottom = (c.y + c.h) as f32 - 40.0;
    for l in &neighbours {
        if ly + 15.0 > bottom { break; }
        let mut t = l.clone();
        while fonts::width(&t, Face::Italic, 13.0, 0.0) > left_w && t.len() > 4 { t.pop(); }
        fonts::draw(buf, w, h, x0, ly, &t, Face::Italic, 13.0, 0.0, INK, None);
        ly += 16.0;
    }
    if gy + 40.0 < (c.y + c.h) as f32 {
        globe(sys, buf, w, h, x0 + 90.0, gy, 30.0);
        let w_ = &sys.world;
        let day = w_.spin.solar_day_hours(&w_.orbit).map(|d| format!("a day of {:.1} hours", d)).unwrap_or_else(|| "no day: one face to the sun".into());
        let lines = [format!("Tilt {:.0}°; {}", w_.spin.obliquity_deg, day), format!("a year of {:.0} Earth days at {:.2} AU", w_.orbit.period_days, w_.orbit.semi_major_au)];
        for (k, l) in lines.iter().enumerate() {
            fonts::draw(buf, w, h, x0 + 140.0, gy - 22.0 + k as f32 * 20.0, l, Face::Italic, 15.0, 0.0, INK, None);
        }
    }

    // The account, flowing down the right column.
    let tx = x0 + left_w + 30.0;
    let tw = (c.x + c.w) as f32 - 28.0 - tx;
    let mut ty = y;
    'sections: for s in sys.account().into_iter().filter(|s| s.title != "Neighbours") {
        if ty + 40.0 > bottom { break; }
        fonts::draw(buf, w, h, tx, ty, s.title, Face::SmallCaps, 15.5, 0.6, RUBRIC, None);
        ty += 19.0;
        for line in &s.lines {
            for l in fonts::wrap(line, Face::Roman, 13.5, tw) {
                if ty + 16.0 > bottom { break 'sections; }
                fonts::draw(buf, w, h, tx, ty, &l, Face::Roman, 13.5, 0.0, INK, None);
                ty += 15.8;
            }
        }
        ty += 5.0;
    }
    let keys = "Tab: the worlds as globes    H or Esc: back to the map";
    let kw = fonts::width(keys, Face::Italic, 14.0, 0.0);
    fonts::draw(buf, w, h, c.x as f32 + (c.w as f32 - kw) / 2.0, (c.y + c.h) as f32 - 28.0, keys, Face::Italic, 14.0, 0.0, INK_FADED, None);
}

/// The system of a world (the Sun's for worlds saved before star systems).
pub fn system_of(world: &WorldData) -> StarSystem {
    world.star_system.clone().unwrap_or_else(StarSystem::sol)
}

/// `--heavens-snapshot FILE`: the card on the desk, 1280x800.
pub fn save_snapshot(world: &WorldData, path: &str) -> Result<(), Box<dyn Error>> {
    let (w, h) = (1280usize, 800usize);
    let mut buf = vec![DESK; w * h];
    for (i, p) in buf.iter_mut().enumerate() {
        let grain = ((hash(i % w / 2, i / w / 9) & 0x1F) as f32 / 31.0) * 0.08;
        *p = mix(DESK, 0x0040_3428, grain);
    }
    draw(&system_of(world), &mut buf, w, h);
    image::RgbImage::from_fn(w as u32, h as u32, |x, y| { let p = buf[y as usize * w + x as usize]; image::Rgb([(p >> 16) as u8, (p >> 8) as u8, p as u8]) }).save(path)?;
    let _ = PI;
    Ok(())
}
