//! The zoomed region (Z on the world map) in the map's ink, not the climate colours.
//!
//! `region.render_rgb()` is a hillshaded climate colour per cell; blown up by the walker it was
//! flat green squares with pixel rivers, the one view out of the ink style. Here each pixel is
//! drawn from the region's fields sampled smoothly between cell centres:
//! - land is a wash of the cell's colour pulled toward parchment, the far side of slopes hatched
//!   (never darkened), contours inked every 100 m (a heavier line each 500 m)
//! - sea and lakes are flat water washes with paler shallows, edged by an ink bank on the smooth
//!   0.5 contour of the water field, with a second, fainter offshore line
//! - rivers are the same water, from a coverage field of their channels, inked along both banks
//! - woods are small tree symbols on a jittered grid (broadleaf scalloped, firs pointed)
//! - what the lore paints (fields, roads, sites) shows through in its own colours, muted.
//!
//! `Ink::new` precomputes the fields once per region (~50 ms for 1024x1024 cells); `draw`
//! touches only arrays per pixel and runs row-parallel.

use rayon::prelude::*;

use super::render::ZoomCamera;
use crate::region::zoom::ZoomRegion;

const INK: [f32; 3] = [56.0, 42.0, 32.0];
const SEA_INK: [f32; 3] = [40.0, 66.0, 82.0];
const PAPER: [f32; 3] = [234.0, 222.0, 196.0];
const DEEP: [f32; 3] = [96.0, 128.0, 142.0];
const SEA: [f32; 3] = [118.0, 150.0, 158.0];
const SHALLOW: [f32; 3] = [156.0, 184.0, 178.0];
const LAKE: [f32; 3] = [126.0, 160.0, 166.0];
const OFF_MAP: u32 = 0x00DC_CCA8;

/// A region's fields, ready for drawing.
pub struct Ink {
    w: usize,
    h: usize,
    /// Land wash per cell (lore paint included); water cells hold their water colour.
    wash: Vec<[f32; 3]>,
    /// 1 where the cell is sea or lake, 0 on land.
    water: Vec<f32>,
    /// River channel coverage, 0..1.
    river: Vec<f32>,
    /// Elevation, metres.
    elev: Vec<f32>,
    /// How far a cell faces away from the light (0 lit .. 1 in shade).
    shade: Vec<f32>,
    /// 0 no wood, 1 broadleaf, 2 firs.
    wood: Vec<u8>,
    /// The lore painted this cell (fields, roads, sites).
    lore: Vec<bool>,
    /// Mean temperature (°C) and moisture, for the seasons.
    temp: Vec<f32>,
    moist: Vec<f32>,
    seed: u64,
    /// Never shared by two regions' inks (the kept frame is keyed on it: a new region is swapped
    /// into the same place in memory).
    id: u64,
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn hash(x: i64, y: i64, seed: u64) -> u64 {
    let mut h = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ seed;
    h ^= h >> 31;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^ (h >> 29)
}

impl Ink {
    /// `painted` is the region's climate colours with the lore painted on (the walker's `rgb`);
    /// where it differs from the plain colours, the lore's colour shows.
    pub fn new(region: &ZoomRegion, painted: &[[u8; 3]]) -> Ink {
        let (w, h) = (region.width, region.height);
        let plain = region.render_rgb();
        let at = |x: usize, y: usize| region.elevation_m[y.min(h - 1) * w + x.min(w - 1)];
        let mut wash = vec![[0.0f32; 3]; w * h];
        let mut water = vec![0.0f32; w * h];
        let mut shade = vec![0.0f32; w * h];
        let mut wood = vec![0u8; w * h];
        let mut lore = vec![false; w * h];
        for y in 0..h {
            for x in 0..w {
                let k = y * w + x;
                let e = region.elevation_m[k];
                if e <= 0.0 {
                    water[k] = 1.0;
                    let t = (-e / 600.0).clamp(0.0, 1.0);
                    wash[k] = if t < 0.25 { mix(SHALLOW, SEA, t / 0.25) } else { mix(SEA, DEEP, (t - 0.25) / 0.75) };
                    continue;
                }
                if region.lake_depth_m[k] > 0.0 { water[k] = 1.0; wash[k] = LAKE; continue; }
                // The cell's own colour, unshaded: undo render_rgb's hillshade by taking the
                // brightest of the plain colour's neighbourhood is costly; the LUT colour is the
                // plain colour over its shade, so mute it toward parchment instead and let the
                // hatching carry the relief.
                let p = plain[k];
                let c = [p[0] as f32, p[1] as f32, p[2] as f32];
                let lum = (c[0] + c[1] + c[2]) / 3.0;
                // Even the shade out (the plain colour is hillshaded), then mute.
                let even = if lum > 1.0 { let g = 150.0 / lum; [c[0] * g, c[1] * g, c[2] * g] } else { c };
                // Half the saturation, toward a sepia grey, then toward parchment: the map's
                // sage and ochre washes rather than the climate LUT's lime.
                let grey = [150.0 * 1.04, 150.0, 150.0 * 0.86];
                let muted = mix(mix(even, grey, 0.55), PAPER, 0.42);
                let q = painted[k];
                if q != p {
                    lore[k] = true;
                    wash[k] = mix([q[0] as f32, q[1] as f32, q[2] as f32], PAPER, 0.25);
                } else {
                    wash[k] = muted;
                }
                let dzdx = (at(x + 1, y) - at(x.saturating_sub(1), y)) / (2.0 * region.cell_m);
                let dzdy = (at(x, y + 1) - at(x, y.saturating_sub(1))) / (2.0 * region.cell_m);
                // Light from the top left: slopes facing down-right are in shade.
                let away = (dzdx + dzdy) * 0.707;
                shade[k] = (away * 6.0).clamp(0.0, 1.0);
                let (t, m) = (region.temperature_c[k], region.moisture[k]);
                if !lore[k] && m > 0.42 && t > -4.0 && e < 3200.0 {
                    wood[k] = if t < 7.0 { 2 } else { 1 };
                }
            }
        }
        // River coverage: a disc per river cell, its radius from the channel's width.
        let mut river = vec![0.0f32; w * h];
        for y in 0..h {
            for x in 0..w {
                let width = region.river_width_m[y * w + x];
                if width <= 0.0 { continue; }
                let r = (width / region.cell_m * 0.5).max(0.45);
                let ri = r.ceil() as i64 + 1;
                for dy in -ri..=ri {
                    for dx in -ri..=ri {
                        let (px, py) = (x as i64 + dx, y as i64 + dy);
                        if px < 0 || py < 0 || px >= w as i64 || py >= h as i64 { continue; }
                        let d = ((dx * dx + dy * dy) as f32).sqrt();
                        let c = (r + 0.5 - d).clamp(0.0, 1.0);
                        let k = py as usize * w + px as usize;
                        if c > river[k] { river[k] = c; }
                    }
                }
            }
        }
        // Soften the channels' D8 staircase: two passes of a 3x3 blur, keeping the peak so a
        // narrow stream still reaches the bank contour.
        for _ in 0..2 {
            let src = river.clone();
            for y in 1..h.saturating_sub(1) {
                for x in 1..w.saturating_sub(1) {
                    let mut sum = 0.0;
                    for dy in 0..3 { for dx in 0..3 { sum += src[(y + dy - 1) * w + x + dx - 1]; } }
                    river[y * w + x] = (sum / 9.0 * 1.6).min(1.0);
                }
            }
        }
        Ink { w, h, wash, water, river, elev: region.elevation_m.clone(), shade, wood, lore, temp: region.temperature_c.clone(), moist: region.moisture.clone(), seed: region.params.seed,
            id: { static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1); NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) } }
    }

    #[inline]
    fn bilinear(&self, f: &[f32], x: f32, y: f32) -> f32 {
        let (x, y) = (x - 0.5, y - 0.5);
        let (x0, y0) = (x.floor(), y.floor());
        let (tx, ty) = (x - x0, y - y0);
        let (xi, yi) = (x0 as i64, y0 as i64);
        let g = |dx: i64, dy: i64| f[((yi + dy).clamp(0, self.h as i64 - 1) as usize) * self.w + (xi + dx).clamp(0, self.w as i64 - 1) as usize];
        let top = g(0, 0) + (g(1, 0) - g(0, 0)) * tx;
        let bot = g(0, 1) + (g(1, 1) - g(0, 1)) * tx;
        top + (bot - top) * ty
    }

    #[inline]
    fn wash_at(&self, x: f32, y: f32) -> [f32; 3] {
        let (x, y) = (x - 0.5, y - 0.5);
        let (x0, y0) = (x.floor(), y.floor());
        let (tx, ty) = (x - x0, y - y0);
        let (xi, yi) = (x0 as i64, y0 as i64);
        let g = |dx: i64, dy: i64| self.wash[((yi + dy).clamp(0, self.h as i64 - 1) as usize) * self.w + (xi + dx).clamp(0, self.w as i64 - 1) as usize];
        mix(mix(g(0, 0), g(1, 0), tx), mix(g(0, 1), g(1, 1), tx), ty)
    }

    #[inline]
    fn cell(&self, x: f32, y: f32) -> usize { (y.max(0.0) as usize).min(self.h - 1) * self.w + (x.max(0.0) as usize).min(self.w - 1) }

    /// The trees whose symbols may cover (x, y): (u, v) offset in symbol radii, kind, and the
    /// symbol's foot (row), at most nine, lowest foot first so the nearer tree is drawn over.
    fn trees_near(&self, x: f32, y: f32, ppc: f32, out: &mut Vec<(f32, f32, u8, f32)>) {
        out.clear();
        // Spacing ~1.6 cells, never under 14 px between symbols (the world map's trees).
        let sp = (14.0 / ppc).max(1.6);
        let (gx, gy) = ((x / sp).floor() as i64, (y / sp).floor() as i64);
        for oy in -1..=1 {
            for ox in -1..=1 {
                let (cx, cy) = (gx + ox, gy + oy);
                let hsh = hash(cx, cy, self.seed ^ 0x07EE);
                if hsh % 4 == 0 { continue; }
                let jx = ((hsh >> 8) & 255) as f32 / 255.0;
                let jy = ((hsh >> 16) & 255) as f32 / 255.0;
                let (tx, ty) = ((cx as f32 + 0.2 + 0.6 * jx) * sp, (cy as f32 + 0.2 + 0.6 * jy) * sp);
                if tx < 0.0 || ty < 0.0 || tx >= self.w as f32 || ty >= self.h as f32 { continue; }
                let kind = self.wood[self.cell(tx, ty)];
                if kind == 0 { continue; }
                let r = sp * 0.42;
                let (u, v) = ((x - tx) / r, (y - ty) / r);
                if u.abs() < 1.3 && v.abs() < 1.5 { out.push((u, v, kind, ty)); }
            }
        }
        out.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal));
    }
}

/// Pixels between tree symbols at this zoom.
fn sp_px(ppc: f32) -> f32 { (14.0 / ppc).max(1.6) * ppc }

/// Draw the region at `cam` into `buf`.
/// How far a season moves the mean temperature (°C): a winter 12 colder, a summer 7 warmer.
fn season_shift(s: crate::seasons::Season) -> f32 {
    use crate::seasons::Season::*;
    match s { Spring => -2.0, Summer => 7.0, Autumn => -3.0, Winter => -12.0 }
}

pub fn draw(ink: &Ink, cam: &ZoomCamera, season: crate::seasons::Season, buf: &mut [u32], w: usize, h: usize) {
    let ox = cam.cx * cam.px_per_cell - w as f32 / 2.0;
    let oy = cam.cy * cam.px_per_cell - h as f32 / 2.0;
    draw_rows(ink, ox, oy, cam.px_per_cell, season, buf, w, h, None, true);
}

/// The camera moved onto whole screen pixels, as `draw_kept` draws.
pub fn snap_camera(cam: &ZoomCamera, w: usize, h: usize) -> ZoomCamera {
    let t = cam.px_per_cell;
    let ox = (cam.cx * t - w as f32 / 2.0).round();
    let oy = (cam.cy * t - h as f32 / 2.0).round();
    ZoomCamera { cx: (ox + w as f32 / 2.0) / t, cy: (oy + h as f32 / 2.0) / t, px_per_cell: t }
}

thread_local! {
    /// The last region frame drawn: (ink id, zoom, season, window), where the screen stood (whole
    /// pixels) and its pixels.
    static KEPT: std::cell::RefCell<Option<((u64, u32, u8, usize, usize), i64, i64, Vec<u32>)>> = std::cell::RefCell::new(None);
}

/// `draw` kept between frames: the camera on whole pixels (`snap_camera`), a walk shifts the
/// last frame and draws only what comes into view. (Drawn afresh, the region cost ~70 ms of
/// work a frame across the cores.) `PLANET_REGION_CHECK=1` compares each frame with a fresh one.
pub fn draw_kept(ink: &Ink, cam: &ZoomCamera, season: crate::seasons::Season, buf: &mut [u32], w: usize, h: usize) {
    let t = cam.px_per_cell;
    let ox = (cam.cx * t - w as f32 / 2.0).round() as i64;
    let oy = (cam.cy * t - h as f32 / 2.0).round() as i64;
    let key = (ink.id, t.to_bits(), season as u8, w, h);
    KEPT.with(|cell| {
        let mut kept = cell.borrow_mut();
        match kept.as_mut() {
            Some(k) if k.0 == key && (ox - k.1).abs() < w as i64 && (oy - k.2).abs() < h as i64 => {
                let (dx, dy) = (ox - k.1, oy - k.2);
                if dx != 0 || dy != 0 {
                    let old = std::mem::take(&mut k.3);
                    let mut moved = vec![OFF_MAP; w * h];
                    moved.chunks_mut(w).enumerate().for_each(|(sy, row)| {
                        let o = sy as i64 + dy;
                        if o < 0 || o >= h as i64 { return; }
                        let src = &old[o as usize * w..(o as usize + 1) * w];
                        let (a, b) = ((-dx).max(0) as usize, (w as i64 - dx.max(0)) as usize);
                        if b > a { row[a..b].copy_from_slice(&src[(a as i64 + dx) as usize..(b as i64 + dx) as usize]); }
                    });
                    let exposed = move |sx: usize, sy: usize| { let (px, py) = (sx as i64 + dx, sy as i64 + dy); px < 0 || py < 0 || px >= w as i64 || py >= h as i64 };
                    draw_rows(ink, ox as f32, oy as f32, t, season, &mut moved, w, h, Some(&exposed), (dx.unsigned_abs() as usize * h + dy.unsigned_abs() as usize * w) > 150_000);
                    k.1 = ox; k.2 = oy; k.3 = moved;
                }
            }
            _ => {
                draw_rows(ink, ox as f32, oy as f32, t, season, buf, w, h, None, true);
                *kept = Some((key, ox, oy, buf.to_vec()));
            }
        }
        let k = kept.as_ref().unwrap();
        buf.copy_from_slice(&k.3);
        if std::env::var("PLANET_REGION_CHECK").is_ok() {
            let mut fresh = vec![0u32; w * h];
            draw_rows(ink, ox as f32, oy as f32, t, season, &mut fresh, w, h, None, true);
            let bad = fresh.iter().zip(buf.iter()).filter(|(a, b)| a != b).count();
            if bad > 0 { eprintln!("REGION CHECK: {} pixels differ", bad); }
        }
    });
}

/// The region's pixels for the screen whose top-left is the region pixel (ox, oy) at `ppc`
/// pixels a cell: every pixel, or those `need` asks for.
#[allow(clippy::too_many_arguments)]
fn draw_rows(ink: &Ink, ox: f32, oy: f32, ppc: f32, season: crate::seasons::Season, buf: &mut [u32], w: usize, h: usize, need: Option<&(dyn Fn(usize, usize) -> bool + Sync)>, par: bool) {
    let shift = season_shift(season);
    let autumn = season == crate::seasons::Season::Autumn;
    let inv = 1.0 / ppc;
    let trees = ppc >= 1.5;
    let contours = ppc >= 0.75;
    let _ = h;
    let draw_row = |(sy, row): (usize, &mut [u32])| {
        if let Some(n) = need { if !(0..w).any(|sx| n(sx, sy)) { return; } }
        let mut near: Vec<(f32, f32, u8, f32)> = Vec::with_capacity(9);
        let ry = (oy + sy as f32 + 0.5) * inv;
        for (sx, out) in row.iter_mut().enumerate() {
            if let Some(n) = need { if !n(sx, sy) { continue; } }
            let rx = (ox + sx as f32 + 0.5) * inv;
            if rx < 0.0 || ry < 0.0 || rx >= ink.w as f32 || ry >= ink.h as f32 { *out = OFF_MAP; continue; }
            let k = ink.cell(rx, ry);
            // Water: sea and lakes plus river channels, one smooth field.
            let wv = ink.bilinear(&ink.water, rx, ry);
            let rv = ink.bilinear(&ink.river, rx, ry);
            let v = wv.max(rv);
            let vx = ink.bilinear(&ink.water, rx + inv, ry).max(ink.bilinear(&ink.river, rx + inv, ry));
            let vy = ink.bilinear(&ink.water, rx, ry + inv).max(ink.bilinear(&ink.river, rx, ry + inv));
            let grad = ((vx - v).powi(2) + (vy - v).powi(2)).sqrt().max(1e-4);
            // Distance to the 0.5 contour, in pixels.
            let edge_px = (v - 0.5).abs() / grad;
            let mut c;
            if v >= 0.5 {
                c = if wv >= rv { ink.wash_at(rx, ry) } else { LAKE };
                if wv < 0.5 { c = mix(c, SHALLOW, 0.25); }
                // Pale water along the shore, fading over ~5 px.
                let shore = (1.0 - edge_px / 5.0).clamp(0.0, 1.0);
                c = mix(c, SHALLOW, 0.5 * shore);
                // A wave mark here and there on open water.
                if wv > 0.98 && ppc >= 1.5 {
                    let (gx, gy) = ((rx * ppc / 22.0).floor() as i64, (ry * ppc / 14.0).floor() as i64);
                    let hs = hash(gx, gy, ink.seed);
                    if hs % 3 == 0 {
                        let (cx, cy) = ((gx as f32 + 0.3 + 0.4 * ((hs >> 8) & 255) as f32 / 255.0) * 22.0, (gy as f32 + 0.5) * 14.0);
                        let (px, py) = (rx * ppc - cx, ry * ppc - cy);
                        if px.abs() < 4.0 && (py + 0.12 * px * px - 1.0).abs() < 0.6 { c = mix(c, SEA_INK, 0.45); }
                    }
                }
                // Ice on lakes and rivers in a hard season (the sea keeps open water).
                let t_w = ink.bilinear(&ink.temp, rx, ry) + shift;
                if t_w < -3.0 && (rv > wv || ink.elev[k] > 0.0) {
                    c = mix([214.0, 224.0, 226.0], c, 0.25);
                    if ((rx * ppc + ry * ppc * 0.4) as i64).rem_euclid(23) == 0 { c = mix(c, [130.0, 154.0, 166.0], 0.5); }
                }
                // Offshore line ~4 px out from the bank.
                if wv > 0.5 && (edge_px - 4.0).abs() < 0.5 && ppc >= 1.0 { c = mix(c, SEA_INK, 0.28); }
            } else {
                c = ink.wash_at(rx, ry);
                // Mottling: the wash isn't flat.
                let n = (hash((rx * 0.7) as i64, (ry * 0.7) as i64, ink.seed ^ 3) & 255) as f32 / 255.0;
                c = mix(c, PAPER, 0.06 * n);
                // Hatching on the far side of slopes: diagonal strokes locked to the land.
                let s = ink.bilinear(&ink.shade, rx, ry);
                if s > 0.12 {
                    let (lx, ly) = (rx * ppc, ry * ppc);
                    let d = (lx - ly).rem_euclid(4.0);
                    if d < 1.0 { c = mix(c, INK, (s * 0.45).min(0.32)); }
                }
                // Contours: every 100 m, every 500 m heavier.
                if contours {
                    let e = ink.bilinear(&ink.elev, rx, ry);
                    let ex = ink.bilinear(&ink.elev, rx + inv, ry);
                    let ey = ink.bilinear(&ink.elev, rx, ry + inv);
                    let eg = ((ex - e).powi(2) + (ey - e).powi(2)).sqrt().max(1e-3);
                    let step = if eg > 60.0 { 500.0 } else { 100.0 };
                    let f = (e / step).fract();
                    let dist = f.min(1.0 - f) * step / eg;
                    if dist < 0.55 && e > 0.0 {
                        let major = ((e / step).round() as i64 * step as i64) % 500 == 0;
                        c = mix(c, INK, if major { 0.42 } else { 0.18 });
                    }
                }
                // The season's snow: where the season's cold and the moisture allow, the ground
                // whitens (more the colder), hatching and contours still showing through.
                let t_here = ink.bilinear(&ink.temp, rx, ry) + shift;
                let snow = if t_here < 0.0 { ((-t_here) / 6.0).clamp(0.0, 1.0) * (ink.bilinear(&ink.moist, rx, ry) * 2.5).clamp(0.3, 1.0) } else { 0.0 };
                if snow > 0.0 { c = mix(c, [240.0, 242.0, 244.0], 0.75 * snow); }
                // Woods.
                if trees && !ink.lore[k] {
                    near.clear();
                    ink.trees_near(rx, ry, ppc, &mut near);
                    let rim_w = 1.1 / (sp_px(ppc) * 0.42);
                    let mut drawn = false;
                    for &(u, v2, kind, _) in near.iter() {
                        let (inside, rim) = if kind == 1 {
                            // Scalloped crown over a short trunk.
                            let r = (u * u + (v2 + 0.15).powi(2)).sqrt();
                            let wob = 0.88 + 0.1 * ((u.atan2(v2 + 0.15)) * 5.0).cos();
                            let crown = r < wob;
                            let trunk = u.abs() < 0.12 && v2 > 0.6 && v2 < 1.1;
                            (crown || trunk, (crown && r > wob - rim_w) || (trunk && u.abs() > 0.05))
                        } else {
                            // A fir: a tall triangle.
                            let half = (1.0 + v2) * 0.42;
                            let tri = v2 > -1.2 && v2 < 0.9 && u.abs() < half;
                            (tri, tri && (half - u.abs() < rim_w || v2 > 0.9 - rim_w))
                        };
                        if inside {
                            let lit = u + v2 < 0.0;
                            let mut fill = if kind == 1 { [150.0, 164.0, 116.0] } else { [124.0, 146.0, 112.0] };
                            // Autumn turns the broadleaves; winter bares them grey-brown, firs keep green.
                            if kind == 1 && autumn { fill = [184.0, 142.0, 92.0]; }
                            if kind == 1 && snow > 0.3 { fill = [150.0, 132.0, 112.0]; }
                            c = if rim { mix(fill, INK, 0.75) } else if lit { mix(fill, if snow > 0.2 { [244.0, 246.0, 248.0] } else { PAPER }, if snow > 0.2 { 0.6 } else { 0.25 }) } else { fill };
                            drawn = true;
                            break;
                        }
                    }
                    // A faint shadow down-right of a tree, on the ground only.
                    if !drawn && near.iter().any(|&(u, v2, _, _)| (u - 0.2).powi(2) + (v2 - 0.9).powi(2) < 0.18) { c = mix(c, INK, 0.12); }
                }
            }
            // The bank: ink along the 0.5 contour.
            if edge_px < 0.9 { c = mix(c, SEA_INK, (1.0 - edge_px / 0.9) * 0.85); }
            *out = ((c[0].clamp(0.0, 255.0) as u32) << 16) | ((c[1].clamp(0.0, 255.0) as u32) << 8) | c[2].clamp(0.0, 255.0) as u32;
        }
    };
    // (A narrow strip come into view is drawn on this thread: a parallel pass over every row
    // waits on every worker.)
    if !par { buf.chunks_mut(w).enumerate().for_each(draw_row); }
    else { buf.par_chunks_mut(w).enumerate().for_each(draw_row); }
}
