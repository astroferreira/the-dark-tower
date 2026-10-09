//! The ink sprite kit: one hand for every creature, figure, building and mark drawn over the
//! embark (and the sprite sheet, `--sprite-sheet`).
//!
//! A sprite is built from shapes in a unit box (u right, v down, -1..1 across `size` pixels),
//! each stamped with the treatment the atlas and the glyphs use: a flat wash, a sepia ink rim
//! where the shape ends, a paler side towards the top-left light, and the far side hatched (from
//! 9 px). Later shapes are drawn over earlier ones with their own rims, so parts read like an
//! inked drawing (a leg over the body, an ear over the head). A `Pen` can face left (`flip`), so
//! a creature turns the way it walks while the light stays top-left on screen.

pub type Rgb = [f32; 3];

pub const INK: Rgb = [56.0, 42.0, 32.0];
pub const LIGHT: Rgb = [250.0, 244.0, 230.0];
pub const PAPER: Rgb = [236.0, 226.0, 204.0];
pub const BLOOD: Rgb = [140.0, 30.0, 26.0];

pub fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb { [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t] }

/// A cheap hash for per-pixel and per-thing variation.
pub fn hash(a: i64, b: i64, salt: u64) -> u64 {
    let mut h = (a as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (b as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ salt.wrapping_mul(0x1656_67B1_9E37_79F9);
    h ^= h >> 31;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^ (h >> 29)
}

/// How a shape is finished.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Finish {
    /// Wash, ink rim, lit side, hatched far side.
    Inked,
    /// Wash and rim, no light or hatching (small parts: eyes, buckles, stripes on a shield).
    Plain,
    /// Wash only, no rim (markings painted inside a shape already drawn).
    Paint,
}

pub struct Pen<'a> {
    put: &'a mut dyn FnMut(i64, i64, Rgb, f32),
    pub cx: f32,
    pub cy: f32,
    /// Pixels per unit (the unit box spans `2 * half`).
    pub half: f32,
    /// Facing left: u is mirrored on screen.
    pub flip: bool,
    /// Opacity of everything drawn (a creature a level off is faint).
    pub alpha: f32,
    /// A wash over every colour (a ghost's pallor, an illness), with its strength.
    pub tint: Option<(Rgb, f32)>,
    /// The ink's own colour (the dead and the ghostly are drawn in a paler line).
    pub ink: Rgb,
}

impl<'a> Pen<'a> {
    /// A pen drawing a sprite `size` pixels across, centred at (cx, cy).
    pub fn new(put: &'a mut dyn FnMut(i64, i64, Rgb, f32), cx: f32, cy: f32, size: f32) -> Pen<'a> {
        Pen { put, cx, cy, half: (size / 2.0).max(0.5), flip: false, alpha: 1.0, tint: None, ink: INK }
    }

    pub fn facing_left(mut self, left: bool) -> Self { self.flip = left; self }
    pub fn faint(mut self, a: f32) -> Self { self.alpha = a; self }

    /// Unit point to screen.
    pub fn at(&self, u: f32, v: f32) -> (f32, f32) { (self.cx + if self.flip { -u } else { u } * self.half, self.cy + v * self.half) }
    fn unit(&self, x: f32, y: f32) -> (f32, f32) { let u = (x - self.cx) / self.half; (if self.flip { -u } else { u }, (y - self.cy) / self.half) }

    fn colour(&self, c: Rgb) -> Rgb { match self.tint { Some((t, s)) => mix(c, t, s), None => c } }

    /// Put one screen pixel (tinted, at the pen's opacity).
    pub fn pixel(&mut self, x: i64, y: i64, c: Rgb, a: f32) {
        let c = self.colour(c);
        let al = a * self.alpha;
        (self.put)(x, y, c, al);
    }

    /// A shape inside the unit rectangle `bb` (u0, v0, u1, v1) where `inside` holds.
    pub fn shape(&mut self, fill: Rgb, finish: Finish, bb: [f32; 4], inside: &dyn Fn(f32, f32) -> bool) {
        let (ax, ay) = self.at(bb[0], bb[1]);
        let (bx, by) = self.at(bb[2], bb[3]);
        let (x0, x1) = (ax.min(bx).floor() as i64 - 1, ax.max(bx).ceil() as i64 + 1);
        let (y0, y1) = (ay.min(by).floor() as i64 - 1, ay.max(by).ceil() as i64 + 1);
        let e = 1.0 / self.half;
        let hatch = self.half >= 4.5 && finish == Finish::Inked;
        let small = self.half < 3.5;
        let (mx, my) = ((x0 + x1) as f32 / 2.0, (y0 + y1) as f32 / 2.0);
        let (hw, hh) = (((x1 - x0) as f32 / 2.0).max(1.0), ((y1 - y0) as f32 / 2.0).max(1.0));
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (u, v) = self.unit(x as f32 + 0.5, y as f32 + 0.5);
                if !inside(u, v) { continue; }
                if finish != Finish::Paint {
                    let rim = !inside(u + e, v) || !inside(u - e, v) || !inside(u, v + e) || !inside(u, v - e);
                    if rim {
                        let ink = self.ink;
                        if small { self.pixel(x, y, mix(fill, ink, 0.6), 0.95); } else { self.pixel(x, y, ink, 0.95); }
                        continue;
                    }
                }
                let mut c = fill;
                if finish == Finish::Inked {
                    // Light from the top left of the screen, whichever way the sprite faces.
                    let (sx, sy) = ((x as f32 + 0.5 - mx) / hw, (y as f32 + 0.5 - my) / hh);
                    // The lit edge: pale, with a dithered step so a big body has no hard split.
                    let lit = sx + sy;
                    if lit < -0.85 || (lit < -0.7 && (x + y) % 2 == 0) { c = mix(c, LIGHT, 0.2); }
                    if hatch && sx + sy > 0.4 && (x - y).rem_euclid(3) == 0 { c = mix(c, self.ink, 0.5); }
                }
                self.pixel(x, y, c, 1.0);
            }
        }
    }

    pub fn ellipse(&mut self, u: f32, v: f32, rx: f32, ry: f32, fill: Rgb) { self.ellipse_f(u, v, rx, ry, fill, Finish::Inked); }
    pub fn ellipse_f(&mut self, u: f32, v: f32, rx: f32, ry: f32, fill: Rgb, f: Finish) {
        let (rx, ry) = (rx.max(0.01), ry.max(0.01));
        self.shape(fill, f, [u - rx, v - ry, u + rx, v + ry], &move |p, q| ((p - u) / rx).powi(2) + ((q - v) / ry).powi(2) <= 1.0);
    }

    /// An ellipse turned by `ang` radians (clockwise on screen, before any flip).
    pub fn ellipse_rot(&mut self, u: f32, v: f32, rx: f32, ry: f32, ang: f32, fill: Rgb) {
        let (c, s) = (ang.cos(), ang.sin());
        let r = rx.max(ry);
        self.shape(fill, Finish::Inked, [u - r, v - r, u + r, v + r], &move |p, q| {
            let (dx, dy) = (p - u, q - v);
            let (a, b) = (dx * c + dy * s, -dx * s + dy * c);
            (a / rx).powi(2) + (b / ry).powi(2) <= 1.0
        });
    }

    /// A limb from `a` (radius `ra`) to `b` (radius `rb`), rounded at both ends.
    pub fn limb(&mut self, a: (f32, f32), ra: f32, b: (f32, f32), rb: f32, fill: Rgb) { self.limb_f(a, ra, b, rb, fill, Finish::Inked); }
    pub fn limb_f(&mut self, a: (f32, f32), ra: f32, b: (f32, f32), rb: f32, fill: Rgb, f: Finish) {
        let r = ra.max(rb);
        let bb = [a.0.min(b.0) - r, a.1.min(b.1) - r, a.0.max(b.0) + r, a.1.max(b.1) + r];
        self.shape(fill, f, bb, &move |p, q| in_limb(p, q, a, ra, b, rb));
    }

    /// A polygon (any, even-odd), points in unit space.
    pub fn poly(&mut self, pts: &[(f32, f32)], fill: Rgb) { self.poly_f(pts, fill, Finish::Inked); }
    pub fn poly_f(&mut self, pts: &[(f32, f32)], fill: Rgb, f: Finish) {
        if pts.len() < 3 { return; }
        let pts: Vec<(f32, f32)> = pts.to_vec();
        let bb = pts.iter().fold([f32::MAX, f32::MAX, f32::MIN, f32::MIN], |b, p| [b[0].min(p.0), b[1].min(p.1), b[2].max(p.0), b[3].max(p.1)]);
        self.shape(fill, f, bb, &move |u, v| in_poly(&pts, u, v));
    }

    pub fn rect(&mut self, u0: f32, v0: f32, u1: f32, v1: f32, fill: Rgb) { self.rect_f(u0, v0, u1, v1, fill, Finish::Inked); }
    pub fn rect_f(&mut self, u0: f32, v0: f32, u1: f32, v1: f32, fill: Rgb, f: Finish) {
        let (a, b, c, d) = (u0.min(u1), v0.min(v1), u0.max(u1), v0.max(v1));
        self.shape(fill, f, [a, b, c, d], &move |u, v| u >= a && u <= c && v >= b && v <= d);
    }

    /// An ink stroke `width` pixels wide between two unit points.
    pub fn line(&mut self, a: (f32, f32), b: (f32, f32), c: Rgb, width: f32) { self.line_a(a, b, c, width, 0.95); }
    pub fn line_a(&mut self, a: (f32, f32), b: (f32, f32), c: Rgb, width: f32, alpha: f32) {
        let (ax, ay) = self.at(a.0, a.1);
        let (bx, by) = self.at(b.0, b.1);
        let r = (width / 2.0).max(0.5);
        let (x0, x1) = ((ax.min(bx) - r).floor() as i64, (ax.max(bx) + r).ceil() as i64);
        let (y0, y1) = ((ay.min(by) - r).floor() as i64, (ay.max(by) + r).ceil() as i64);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let d = seg_dist(x as f32 + 0.5, y as f32 + 0.5, (ax, ay), (bx, by));
                if d <= r { self.pixel(x, y, c, alpha); }
                else if d <= r + 0.6 { self.pixel(x, y, c, alpha * 0.35); }
            }
        }
    }

    /// A polyline through unit points.
    pub fn path(&mut self, pts: &[(f32, f32)], c: Rgb, width: f32) {
        for k in 1..pts.len() { self.line(pts[k - 1], pts[k], c, width); }
    }

    /// A pale stroke with an ink rim (antlers, horns, tusks, bones, ropes): the path drawn in ink
    /// two pixels wider, then in `fill`.
    pub fn bone(&mut self, pts: &[(f32, f32)], fill: Rgb, width: f32) {
        let ink = self.ink;
        for k in 1..pts.len() { self.line(pts[k - 1], pts[k], ink, width + 2.0); }
        for k in 1..pts.len() { self.line_a(pts[k - 1], pts[k], fill, width, 1.0); }
    }

    /// One pixel-sized dot (two across from 10 px a unit).
    pub fn dot(&mut self, u: f32, v: f32, c: Rgb) {
        let (x, y) = self.at(u, v);
        let n = if self.half >= 10.0 { 2 } else { 1 };
        for dy in 0..n { for dx in 0..n { self.pixel(x.floor() as i64 + dx, y.floor() as i64 + dy, c, 1.0); } }
    }

    /// A soft glow (fire, glowing eyes, a lantern): no rim, fading out to radius `r` (units).
    pub fn glow(&mut self, u: f32, v: f32, r: f32, c: Rgb, strength: f32) {
        let (x, y) = self.at(u, v);
        let rp = r * self.half;
        let ri = rp.ceil() as i64 + 1;
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                let d = ((dx as f32).powi(2) + (dy as f32).powi(2)).sqrt() / rp.max(0.5);
                if d >= 1.0 { continue; }
                let a = (1.0 - d).powi(2) * strength;
                self.pixel(x as i64 + dx, y as i64 + dy, c, a);
            }
        }
    }

    /// A soft shadow on the ground under the sprite (an ellipse of ink at low opacity).
    pub fn ground_shadow(&mut self, u: f32, v: f32, rx: f32, ry: f32) {
        let (x, y) = self.at(u, v);
        let (rxp, ryp) = ((rx * self.half).max(1.0), (ry * self.half).max(1.0));
        let (ix, iy) = (rxp.ceil() as i64 + 1, ryp.ceil() as i64 + 1);
        for dy in -iy..=iy {
            for dx in -ix..=ix {
                let e = (dx as f32 / rxp).powi(2) + (dy as f32 / ryp).powi(2);
                if e < 1.0 { let ink = self.ink; self.pixel(x as i64 + dx, y as i64 + dy, ink, 0.22 * (1.0 - e)); }
            }
        }
    }
}

pub fn in_limb(p: f32, q: f32, a: (f32, f32), ra: f32, b: (f32, f32), rb: f32) -> bool {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    let t = if l2 <= 1e-6 { 0.0 } else { (((p - a.0) * dx + (q - a.1) * dy) / l2).clamp(0.0, 1.0) };
    let (cx, cy) = (a.0 + dx * t, a.1 + dy * t);
    let r = ra + (rb - ra) * t;
    (p - cx).powi(2) + (q - cy).powi(2) <= r * r
}

pub fn in_poly(pts: &[(f32, f32)], u: f32, v: f32) -> bool {
    let mut inside = false;
    let n = pts.len();
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (pts[i], pts[j]);
        if (a.1 > v) != (b.1 > v) && u < (b.0 - a.0) * (v - a.1) / (b.1 - a.1) + a.0 { inside = !inside; }
        j = i;
    }
    inside
}

pub fn seg_dist(x: f32, y: f32, a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    let t = if l2 <= 1e-6 { 0.0 } else { (((x - a.0) * dx + (y - a.1) * dy) / l2).clamp(0.0, 1.0) };
    ((x - a.0 - dx * t).powi(2) + (y - a.1 - dy * t).powi(2)).sqrt()
}

/// A colour by its word ("crimson", "bone white", "sickly yellow", "black and red": the first),
/// muted to sit on the parchment.
pub fn colour_word(word: &str) -> Rgb {
    let w = word.to_lowercase();
    let table: [(&str, Rgb); 44] = [
        ("bone", [226.0, 218.0, 196.0]), ("ashen", [168.0, 162.0, 150.0]), ("charcoal", [70.0, 66.0, 64.0]), ("midnight", [44.0, 52.0, 84.0]),
        ("dark violet", [80.0, 54.0, 96.0]), ("bruise", [104.0, 70.0, 110.0]), ("sickly", [196.0, 186.0, 96.0]), ("sea green", [90.0, 146.0, 128.0]),
        ("grey-green", [128.0, 142.0, 116.0]), ("pale blue", [170.0, 196.0, 214.0]), ("steel", [104.0, 128.0, 150.0]), ("slate", [100.0, 110.0, 120.0]),
        ("crimson", [150.0, 36.0, 44.0]), ("scarlet", [180.0, 52.0, 40.0]), ("orange", [206.0, 124.0, 54.0]), ("amber", [204.0, 144.0, 56.0]),
        ("ochre", [190.0, 148.0, 80.0]), ("umber", [110.0, 82.0, 56.0]), ("rust", [160.0, 84.0, 52.0]), ("teal", [60.0, 130.0, 130.0]),
        ("indigo", [62.0, 60.0, 120.0]), ("golden", [210.0, 170.0, 70.0]), ("gold", [210.0, 170.0, 70.0]), ("silver", [196.0, 198.0, 204.0]),
        ("white", [232.0, 230.0, 222.0]), ("black", [52.0, 46.0, 46.0]), ("grey", [140.0, 138.0, 132.0]), ("gray", [140.0, 138.0, 132.0]),
        ("brown", [132.0, 96.0, 64.0]), ("red", [160.0, 50.0, 40.0]), ("blue", [70.0, 100.0, 160.0]), ("green", [80.0, 130.0, 70.0]),
        ("yellow", [214.0, 186.0, 80.0]), ("purple", [116.0, 70.0, 130.0]), ("violet", [120.0, 80.0, 150.0]), ("pink", [210.0, 140.0, 140.0]),
        ("tawny", [184.0, 140.0, 84.0]), ("dun", [170.0, 150.0, 110.0]), ("russet", [150.0, 80.0, 50.0]), ("pale", [222.0, 214.0, 196.0]),
        ("dark", [70.0, 62.0, 58.0]), ("obsidian", [40.0, 36.0, 44.0]), ("copper", [184.0, 110.0, 70.0]), ("iron", [120.0, 122.0, 128.0]),
    ];
    let mut best: Option<(usize, Rgb)> = None;
    for (k, c) in table.iter() {
        if let Some(pos) = w.find(k) { if best.map_or(true, |(b, _)| pos < b) { best = Some((pos, *c)); } }
    }
    best.map(|b| b.1).unwrap_or([150.0, 130.0, 110.0])
}

/// A sheet of labelled cells on parchment, for `--sprite-sheet`.
pub struct Sheet { pub w: usize, pub h: usize, pub buf: Vec<u32>, cell: (usize, usize), cols: usize, next: usize }

impl Sheet {
    pub fn new(cols: usize, rows: usize, cw: usize, ch: usize) -> Sheet {
        let (w, h) = (cols * cw, rows * ch + 40);
        let mut buf = vec![0x00EA_DEC4u32; w * h];
        // A faint parchment mottle so washes are judged against the map's ground.
        for y in 0..h { for x in 0..w {
            let n = (hash(x as i64 / 3, y as i64 / 3, 0x5EE7) % 1000) as f32 / 1000.0;
            let c = mix([234.0, 222.0, 196.0], [222.0, 208.0, 178.0], n * 0.5);
            buf[y * w + x] = pack(c);
        } }
        Sheet { w, h, buf, cell: (cw, ch), cols, next: 0 }
    }

    pub fn put(&mut self) -> impl FnMut(i64, i64, Rgb, f32) + '_ {
        let (w, h) = (self.w, self.h);
        let buf = &mut self.buf;
        move |x: i64, y: i64, c: Rgb, a: f32| {
            if x < 0 || y < 0 || x as usize >= w || y as usize >= h { return; }
            let k = y as usize * w + x as usize;
            let p = buf[k];
            let old = [((p >> 16) & 0xFF) as f32, ((p >> 8) & 0xFF) as f32, (p & 0xFF) as f32];
            buf[k] = pack(mix(old, c, a.clamp(0.0, 1.0)));
        }
    }

    pub fn title(&mut self, text: &str) {
        let w = self.w;
        let h = self.h;
        super::fonts::draw(&mut self.buf, w, h, 12.0, 8.0, text, super::fonts::Face::SmallCaps, 22.0, 1.0, 0x0030_1E14, None);
    }

    /// The next cell's centre (for the sprite) and write its label under it.
    pub fn cell(&mut self, label: &str) -> (f32, f32) {
        let k = self.next;
        self.next += 1;
        let (cw, ch) = self.cell;
        let (x0, y0) = ((k % self.cols) * cw, (k / self.cols) * ch + 40);
        let (w, h) = (self.w, self.h);
        let lw = super::fonts::width(label, super::fonts::Face::Italic, 12.0, 0.0);
        super::fonts::draw(&mut self.buf, w, h, x0 as f32 + (cw as f32 - lw) / 2.0, (y0 + ch) as f32 - 18.0, label, super::fonts::Face::Italic, 12.0, 0.0, 0x0030_1E14, None);
        (x0 as f32 + cw as f32 / 2.0, y0 as f32 + (ch as f32 - 16.0) / 2.0)
    }

    pub fn save(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let w = self.w;
        image::RgbImage::from_fn(w as u32, self.h as u32, |x, y| { let p = self.buf[y as usize * w + x as usize]; image::Rgb([(p >> 16) as u8, (p >> 8) as u8, p as u8]) }).save(path)?;
        Ok(())
    }
}

pub fn pack(c: Rgb) -> u32 { ((c[0].clamp(0.0, 255.0) as u32) << 16) | ((c[1].clamp(0.0, 255.0) as u32) << 8) | c[2].clamp(0.0, 255.0) as u32 }
