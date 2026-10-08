//! Small ink glyphs for the camp's things: a log, a stone, berries, a fish, a joint of meat, a
//! sheaf, cave fungus, a sack of provisions, bars, ore, a gem, a block, a work, a spear, armour,
//! drink, cloth, a hide, bones, charcoal, a barrel, a book. Drawn on the embark map (things lying
//! about and the heaps at the store) and beside the lines of the stocks ledger, so the same
//! thing looks the same everywhere.
//!
//! Every glyph is built from shapes in a unit box (u right, v down, -1..1) stamped with one
//! treatment, like the atlas: a flat wash, an ink rim where the shape ends, and the side away
//! from the top-left light hatched (from 9 px).

pub type Rgb = [f32; 3];

const INK: Rgb = [56.0, 42.0, 32.0];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    Log, Stone, Berries, Fish, Meat, Grain, Fungus, Provisions,
    Bars, Ore, Gem, Block, Work, Spear, Armour, Drink, Cloth, Hide, Bone, Charcoal, Barrel, Book, Herbs, Clay,
}

impl Glyph {
    /// The glyph for a load of `stuff`.
    pub fn of_stuff(s: crate::colony::Stuff) -> Glyph {
        use crate::colony::Stuff;
        match s {
            Stuff::Berries => Glyph::Berries, Stuff::Fish => Glyph::Fish, Stuff::Meat => Glyph::Meat, Stuff::Grain => Glyph::Grain,
            Stuff::Fungus => Glyph::Fungus, Stuff::Provisions => Glyph::Provisions, Stuff::Timber => Glyph::Log, Stuff::Stone => Glyph::Stone,
        }
    }
}

/// A metal's colour by name ("iron", "copper", "gold", ...).
pub fn metal_colour(name: &str) -> Rgb {
    let n = name.to_lowercase();
    if n.contains("adamant") { [150.0, 214.0, 214.0] }
    else if n.contains("gold") { [214.0, 176.0, 70.0] }
    else if n.contains("copper") { [190.0, 112.0, 70.0] }
    else if n.contains("bronze") { [176.0, 130.0, 70.0] }
    else if n.contains("silver") || n.contains("tin") { [206.0, 206.0, 212.0] }
    else { [148.0, 150.0, 158.0] }
}

/// A gem's colour by name (a hash of it when the name says nothing).
pub fn gem_colour(name: &str) -> Rgb {
    let n = name.to_lowercase();
    for (k, c) in [("ruby", [176.0, 40.0, 56.0]), ("garnet", [140.0, 30.0, 40.0]), ("emerald", [50.0, 140.0, 80.0]), ("sapphire", [50.0, 80.0, 170.0]),
        ("amethyst", [130.0, 80.0, 160.0]), ("topaz", [214.0, 170.0, 60.0]), ("jade", [90.0, 150.0, 100.0]), ("opal", [190.0, 200.0, 210.0]),
        ("diamond", [214.0, 226.0, 232.0]), ("onyx", [50.0, 46.0, 50.0]), ("amber", [210.0, 140.0, 40.0]), ("jasper", [170.0, 70.0, 50.0])] {
        if n.contains(k) { return c; }
    }
    let h = n.bytes().fold(7u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
    [[70.0, 130.0, 180.0], [150.0, 60.0, 120.0], [60.0, 150.0, 120.0]][(h % 3) as usize]
}

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb { [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t] }

/// One shape: filled with `fill`, rimmed in ink, its lower-right hatched.
fn stamp(put: &mut dyn FnMut(i64, i64, Rgb, f32), cx: f32, cy: f32, size: f32, fill: Rgb, inside: &dyn Fn(f32, f32) -> bool) {
    let half = size / 2.0;
    let e = 1.0 / half;
    let (x0, x1) = ((cx - half - 1.0).floor() as i64, (cx + half + 1.0).ceil() as i64);
    let (y0, y1) = ((cy - half - 1.0).floor() as i64, (cy + half + 1.0).ceil() as i64);
    let hatch = size >= 9.0;
    for y in y0..=y1 {
        for x in x0..=x1 {
            let (u, v) = ((x as f32 + 0.5 - cx) / half, (y as f32 + 0.5 - cy) / half);
            if !inside(u, v) { continue; }
            let rim = !inside(u + e, v) || !inside(u - e, v) || !inside(u, v + e) || !inside(u, v - e);
            if rim { put(x, y, INK, 0.95); continue; }
            let mut c = fill;
            // Light from the top left: a paler lit side, the far side hatched.
            if u + v < -0.6 { c = mix(c, [250.0, 244.0, 230.0], 0.22); }
            if hatch && u + v > 0.35 && (x - y).rem_euclid(3) == 0 { c = mix(c, INK, 0.55); }
            put(x, y, c, 1.0);
        }
    }
}

/// An ink line between two unit-box points.
fn stroke(put: &mut dyn FnMut(i64, i64, Rgb, f32), cx: f32, cy: f32, size: f32, a: (f32, f32), b: (f32, f32), c: Rgb, alpha: f32) {
    let half = size / 2.0;
    let (ax, ay, bx, by) = (cx + a.0 * half, cy + a.1 * half, cx + b.0 * half, cy + b.1 * half);
    let n = ((bx - ax).abs().max((by - ay).abs()) * 1.5) as i32 + 1;
    for k in 0..=n {
        let t = k as f32 / n as f32;
        put((ax + (bx - ax) * t).floor() as i64, (ay + (by - ay) * t).floor() as i64, c, alpha);
    }
}

fn ellipse(cx: f32, cy: f32, rx: f32, ry: f32) -> impl Fn(f32, f32) -> bool { move |u, v| ((u - cx) / rx).powi(2) + ((v - cy) / ry).powi(2) <= 1.0 }

fn polygon(pts: &'static [(f32, f32)]) -> impl Fn(f32, f32) -> bool {
    // Convex, clockwise on screen (v down).
    move |u, v| (0..pts.len()).all(|k| { let (a, b) = (pts[k], pts[(k + 1) % pts.len()]); (b.0 - a.0) * (v - a.1) - (b.1 - a.1) * (u - a.0) >= 0.0 })
}

/// Draw `g` centred at (cx, cy), `size` pixels across; `tint` colours metals and gems (None:
/// the glyph's own colour).
pub fn draw(put: &mut dyn FnMut(i64, i64, Rgb, f32), g: Glyph, cx: f32, cy: f32, size: f32, tint: Option<Rgb>) {
    let s = size.max(5.0);
    let st = |put: &mut dyn FnMut(i64, i64, Rgb, f32), fill: Rgb, f: &dyn Fn(f32, f32) -> bool| stamp(put, cx, cy, s, fill, f);
    match g {
        Glyph::Log => {
            // Lying from lower left to upper right; the cut end pale with its rings.
            let (ca, sa) = ((-0.5f32).cos(), (-0.5f32).sin());
            let rot = move |u: f32, v: f32| (u * ca + v * sa, -u * sa + v * ca);
            st(put, tint.unwrap_or([150.0, 104.0, 64.0]), &move |u, v| { let (p, q) = rot(u, v); p.abs() <= 0.72 && q.abs() <= 0.3 || ((p + 0.72) / 0.14).powi(2) + (q / 0.3).powi(2) <= 1.0 });
            st(put, [218.0, 186.0, 134.0], &move |u, v| { let (p, q) = rot(u, v); ((p - 0.72) / 0.16).powi(2) + (q / 0.3).powi(2) <= 1.0 });
            if s >= 10.0 { let (p0, p1) = ((-0.55f32, 0.0f32), (0.45f32, 0.0f32)); let back = |p: (f32, f32)| (p.0 * ca - p.1 * sa, p.0 * sa + p.1 * ca); stroke(put, cx, cy, s, back(p0), back(p1), INK, 0.4); }
        }
        Glyph::Stone => {
            static P: [(f32, f32); 6] = [(-0.45, -0.55), (0.2, -0.72), (0.75, -0.25), (0.65, 0.5), (-0.1, 0.72), (-0.78, 0.12)];
            st(put, tint.unwrap_or([170.0, 166.0, 156.0]), &polygon(&P));
        }
        Glyph::Berries => {
            st(put, [92.0, 122.0, 60.0], &ellipse(0.32, -0.62, 0.36, 0.2));
            let red = [170.0, 38.0, 52.0];
            for (x, y) in [(-0.38, 0.28), (0.38, 0.3), (0.0, -0.22)] { st(put, red, &ellipse(x, y, 0.4, 0.4)); }
        }
        Glyph::Fish => {
            static T: [(f32, f32); 3] = [(0.38, 0.0), (0.92, -0.42), (0.92, 0.42)];
            st(put, [128.0, 156.0, 172.0], &polygon(&T));
            st(put, [150.0, 176.0, 188.0], &ellipse(-0.15, 0.0, 0.64, 0.36));
            put((cx - 0.5 * s / 2.0) as i64, (cy - 0.08 * s / 2.0).floor() as i64, INK, 1.0);
        }
        Glyph::Meat => {
            st(put, [232.0, 224.0, 204.0], &move |u, v| { let (p, q) = (u * 0.8 - v * 0.6, u * 0.6 + v * 0.8); (p - 0.45).abs() <= 0.35 && q.abs() <= 0.12 });
            st(put, [232.0, 224.0, 204.0], &ellipse(0.62, -0.5, 0.18, 0.18));
            st(put, [168.0, 74.0, 62.0], &ellipse(-0.18, 0.18, 0.56, 0.48));
        }
        Glyph::Grain => {
            let gold = [206.0, 170.0, 80.0];
            for (x, y) in [(-0.5, -0.55), (0.0, -0.7), (0.5, -0.55)] { stroke(put, cx, cy, s, (0.0, 0.85), (x, y), [150.0, 116.0, 50.0], 0.95); st(put, gold, &ellipse(x, y, 0.16, 0.26)); }
            st(put, [140.0, 96.0, 50.0], &move |u, v| u.abs() <= 0.24 && (v - 0.3).abs() <= 0.1);
        }
        Glyph::Fungus => {
            st(put, [226.0, 214.0, 190.0], &move |u, v| u.abs() <= 0.22 && v >= -0.1 && v <= 0.78);
            st(put, [148.0, 94.0, 150.0], &move |u, v| v <= 0.08 && (u / 0.82).powi(2) + ((v - 0.08) / 0.7).powi(2) <= 1.0);
        }
        Glyph::Provisions => {
            st(put, [196.0, 168.0, 116.0], &move |u, v| u.abs() <= 0.16 && v >= -0.78 && v <= -0.3);
            st(put, [196.0, 168.0, 116.0], &ellipse(0.0, 0.2, 0.64, 0.6));
            stroke(put, cx, cy, s, (-0.25, -0.42), (0.25, -0.42), INK, 0.9);
        }
        Glyph::Bars => {
            let m = tint.unwrap_or([148.0, 150.0, 158.0]);
            st(put, m, &move |u, v| (v - 0.35).abs() <= 0.25 && u.abs() <= 0.7 - 0.25 * (0.6 - v) );
            st(put, m, &move |u, v| (v + 0.2).abs() <= 0.25 && u.abs() <= 0.55 - 0.25 * (0.05 - v));
        }
        Glyph::Ore => {
            static P: [(f32, f32); 6] = [(-0.5, -0.5), (0.3, -0.68), (0.8, -0.1), (0.5, 0.6), (-0.3, 0.7), (-0.8, 0.1)];
            st(put, [120.0, 104.0, 92.0], &polygon(&P));
            let fleck = tint.unwrap_or([190.0, 112.0, 70.0]);
            for (x, y) in [(-0.2, -0.2), (0.25, 0.15), (-0.35, 0.3)] { put((cx + x * s / 2.0) as i64, (cy + y * s / 2.0) as i64, fleck, 1.0); }
        }
        Glyph::Gem => {
            let c = tint.unwrap_or([70.0, 130.0, 180.0]);
            st(put, c, &move |u, v| u.abs() / 0.62 + (v + 0.05).abs() / 0.8 <= 1.0);
            if s >= 9.0 { stroke(put, cx, cy, s, (-0.45, -0.2), (0.45, -0.2), INK, 0.6); stroke(put, cx, cy, s, (0.0, -0.2), (0.0, 0.7), INK, 0.35); }
        }
        Glyph::Block => {
            st(put, tint.unwrap_or([186.0, 178.0, 158.0]), &move |u, v| u.abs() <= 0.66 && v.abs() <= 0.56);
            stroke(put, cx, cy, s, (-0.55, -0.3), (0.55, -0.3), INK, 0.4);
        }
        Glyph::Work => {
            let c = tint.unwrap_or([180.0, 128.0, 80.0]);
            st(put, c, &move |u, v| u.abs() <= 0.34 && (v + 0.68).abs() <= 0.1);
            st(put, c, &move |u, v| u.abs() <= 0.18 && v >= -0.62 && v <= -0.1);
            st(put, c, &ellipse(0.0, 0.25, 0.52, 0.5));
        }
        Glyph::Spear => {
            stroke(put, cx, cy, s, (-0.8, 0.8), (0.4, -0.4), [120.0, 84.0, 50.0], 1.0);
            stroke(put, cx, cy, s, (-0.75, 0.8), (0.45, -0.4), [120.0, 84.0, 50.0], 0.7);
            let m = tint.unwrap_or([150.0, 150.0, 158.0]);
            st(put, m, &move |u, v| { let (p, q) = ((u - v) * 0.7071, (u + v) * 0.7071); ((p - 0.82) / 0.3).abs() + (q / 0.14).abs() <= 1.0 });
        }
        Glyph::Armour => {
            let c = tint.unwrap_or([140.0, 100.0, 64.0]);
            st(put, c, &move |u, v| ((u.abs() <= 0.44 && v >= -0.5 && v <= 0.8) || (u.abs() <= 0.86 && v >= -0.5 && v <= -0.05)) && (u * u + (v + 0.6).powi(2)) > 0.05);
        }
        Glyph::Drink => {
            st(put, [150.0, 104.0, 64.0], &move |u, v| { let d = ((u - 0.45).powi(2) + v.powi(2)).sqrt(); d <= 0.36 && d >= 0.2 });
            st(put, [150.0, 104.0, 64.0], &move |u, v| u.abs() <= 0.44 && v >= -0.55 && v <= 0.72);
            st(put, [236.0, 226.0, 200.0], &move |u, v| u.abs() <= 0.44 && (v + 0.62).abs() <= 0.12);
        }
        Glyph::Cloth => {
            st(put, tint.unwrap_or([130.0, 106.0, 150.0]), &move |u, v| u.abs() <= 0.72 && v.abs() <= 0.46);
            for k in 0..3 { let x = -0.4 + k as f32 * 0.4; stroke(put, cx, cy, s, (x, -0.4), (x + 0.15, 0.4), INK, 0.35); }
        }
        Glyph::Hide => {
            st(put, [176.0, 136.0, 96.0], &move |u, v| (u / 0.48).powi(2) + (v / 0.62).powi(2) <= 1.0
                || [(-0.55, -0.6), (0.55, -0.6), (-0.55, 0.6), (0.55, 0.6)].iter().any(|&(x, y): &(f32, f32)| ((u - x) / 0.22).powi(2) + ((v - y) / 0.16).powi(2) <= 1.0));
        }
        Glyph::Bone => {
            let b = [232.0, 224.0, 204.0];
            st(put, b, &move |u, v| { let (p, q) = ((u - v) * 0.7071, (u + v) * 0.7071); p.abs() <= 0.6 && q.abs() <= 0.12 });
            for (x, y) in [(-0.5, -0.32), (-0.32, -0.5), (0.5, 0.32), (0.32, 0.5)] { st(put, b, &ellipse(x, y, 0.17, 0.17)); }
        }
        Glyph::Charcoal => {
            for (x, y, r) in [(-0.35, 0.25, 0.36), (0.35, 0.3, 0.32), (0.0, -0.25, 0.36)] { st(put, [52.0, 46.0, 44.0], &ellipse(x, y, r, r * 0.85)); }
        }
        Glyph::Barrel => {
            st(put, [150.0, 104.0, 64.0], &move |u, v| (u / (0.5 + 0.12 * (1.0 - v * v))).abs() <= 1.0 && v.abs() <= 0.72);
            for y in [-0.4, 0.4] { stroke(put, cx, cy, s, (-0.55, y), (0.55, y), INK, 0.8); }
        }
        Glyph::Book => {
            st(put, tint.unwrap_or([120.0, 54.0, 44.0]), &move |u, v| u.abs() <= 0.56 && v.abs() <= 0.7);
            stroke(put, cx, cy, s, (-0.38, -0.66), (-0.38, 0.66), INK, 0.6);
        }
        Glyph::Herbs => {
            stroke(put, cx, cy, s, (0.0, 0.85), (0.0, -0.7), [80.0, 104.0, 50.0], 1.0);
            for (x, y) in [(-0.32, -0.35), (0.32, -0.05), (-0.3, 0.3), (0.28, -0.6)] { st(put, [110.0, 140.0, 70.0], &ellipse(x, y, 0.26, 0.15)); }
        }
        Glyph::Clay => {
            st(put, tint.unwrap_or([176.0, 112.0, 80.0]), &move |u, v| v >= -0.3 + 0.9 * u.abs() * u.abs() - 0.6 && v <= 0.6 && u.abs() <= 0.85);
        }
    }
}

/// Draw a glyph into a `0x00RRGGBB` buffer.
pub fn draw_u32(buf: &mut [u32], w: usize, h: usize, g: Glyph, cx: f32, cy: f32, size: f32, tint: Option<Rgb>) {
    let mut put = |x: i64, y: i64, c: Rgb, a: f32| {
        if x < 0 || y < 0 || x as usize >= w || y as usize >= h { return; }
        let k = y as usize * w + x as usize;
        let p = buf[k];
        let old = [((p >> 16) & 0xFF) as f32, ((p >> 8) & 0xFF) as f32, (p & 0xFF) as f32];
        let n = mix(old, c, a.clamp(0.0, 1.0));
        buf[k] = ((n[0] as u32) << 16) | ((n[1] as u32) << 8) | n[2] as u32;
    };
    draw(&mut put, g, cx, cy, size, tint);
}

/// A heap of `n` of a thing, as the store shows it: up to six glyphs piled in a cell (the lower
/// ones first), centred at (cx, cy) in a cell `cell` pixels wide.
pub fn heap(put: &mut dyn FnMut(i64, i64, Rgb, f32), g: Glyph, n: usize, cx: f32, cy: f32, cell: f32, tint: Option<Rgb>) {
    const SPOTS: [(f32, f32); 6] = [(-0.24, 0.2), (0.24, 0.22), (0.0, 0.26), (-0.12, -0.02), (0.14, 0.0), (0.0, -0.22)];
    let size = (cell * 0.62).clamp(6.0, 20.0);
    let shown = n.clamp(1, 6);
    if shown == 1 { draw(put, g, cx, cy, size * 1.1, tint); return; }
    for &(dx, dy) in SPOTS.iter().take(shown) { draw(put, g, cx + dx * cell, cy + dy * cell, size, tint); }
}
