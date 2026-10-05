//! Pure software rendering of the tile view into a `0x00RRGGBB` pixel buffer.
//! Kept free of any window code so frames can be rendered (and checked) headlessly.

use super::atlas::Atlas;
use super::classify::TileWorld;

/// World camera: the tile coordinate at the centre of the screen and the tile size in pixels.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub cx: f32,
    pub cy: f32,
    pub tile_px: f32,
}

/// Zoom-region camera: centre in region cells and screen pixels per cell.
#[derive(Clone, Copy, Debug)]
pub struct ZoomCamera {
    pub cx: f32,
    pub cy: f32,
    pub px_per_cell: f32,
}

const OFF_MAP: u32 = 0x002A_2420;
const RIVER: [f32; 3] = [112.0, 148.0, 160.0];
const ICE: [f32; 3] = [214.0, 224.0, 226.0];
const ROAD: [f32; 3] = [136.0, 92.0, 60.0];
const ROAD_HALF_WIDTH: f32 = 0.04;
/// Ink colours shared with the atlas (sepia for land, blue-grey for water).
const INK: [f32; 3] = [56.0, 42.0, 32.0];
const SEA_INK: [f32; 3] = [40.0, 66.0, 82.0];
/// Pale water band along coasts.
const COAST_WASH: [f32; 3] = [172.0, 198.0, 192.0];

/// Smooth value noise in [-1, 1] over world tiles, `freq` lattice cells per tile. `freq` must be
/// a whole number so the noise wraps cleanly at the date line.
#[inline]
fn value_noise(tw: &TileWorld, wx: f32, wy: f32, freq: f32, seed: u64) -> f32 {
    let period = (tw.width as f32 * freq) as i64;
    let (x, y) = (wx * freq, wy * freq);
    let (x0, y0) = (x.floor(), y.floor());
    let (tx, ty) = (x - x0, y - y0);
    let (sx, sy) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
    let lat = |dx: i64, dy: i64| {
        let xi = (x0 as i64 + dx).rem_euclid(period) as u64;
        let yi = (y0 as i64 + dy) as u64;
        let mut h = xi.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ yi.wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ seed;
        h ^= h >> 31;
        h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        h ^= h >> 29;
        (h & 0xFFFF) as f32 / 32767.5 - 1.0
    };
    let top = lat(0, 0) + (lat(1, 0) - lat(0, 0)) * sx;
    let bot = lat(0, 1) + (lat(1, 1) - lat(0, 1)) * sx;
    top + (bot - top) * sy
}

#[inline]
fn tile_index(tw: &TileWorld, wx: f32, wy: f32) -> usize {
    let tx = (wx.floor() as i64).rem_euclid(tw.width as i64) as usize;
    let ty = (wy.floor().max(0.0) as usize).min(tw.height - 1);
    ty * tw.width + tx
}

/// Smoothstep-weighted interpolation of a per-tile value between tile centres.
#[inline]
fn smooth_field(tw: &TileWorld, wx: f32, wy: f32, f: impl Fn(usize) -> f32) -> f32 {
    let (fx, fy) = (wx - 0.5, (wy - 0.5).clamp(0.0, tw.height as f32 - 1.0));
    let (x0, y0) = (fx.floor(), fy.floor());
    let (tx, ty) = (fx - x0, fy - y0);
    let (sx, sy) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
    let w = tw.width as i64;
    let at = |dx: i64, dy: usize| {
        let x = (x0 as i64 + dx).rem_euclid(w) as usize;
        let y = (y0 as usize + dy).min(tw.height - 1);
        f(y * tw.width + x)
    };
    let top = at(0, 0) + (at(1, 0) - at(0, 0)) * sx;
    let bot = at(0, 1) + (at(1, 1) - at(0, 1)) * sx;
    top + (bot - top) * sy
}

/// Far-zoom fill colour at (wx, wy): the flat colour of the tile under a domain-warped position
/// (the same warp the detailed view uses for ground kinds), so biome patches and water depths
/// meander instead of showing tile squares; restricted to tiles of the requested class (land or
/// water), else the nearest such tile in the 3x3 around the pixel. Frozen water is iced over.
fn far_color(tw: &TileWorld, wx: f32, wy: f32, land: bool) -> [f32; 3] {
    let tile_col = |i: usize| {
        let c = tw.color[i];
        let c = [c[0] as f32, c[1] as f32, c[2] as f32];
        if tw.ground[i].is_water() && tw.season_frozen[i] { mix(c, ICE, 0.78) } else { c }
    };
    let (ux, uy) = warp(tw, wx, wy);
    let j = tile_index(tw, ux, uy);
    if tw.ground[j].is_water() != land {
        return tile_col(j);
    }
    let i = tile_index(tw, wx, wy);
    if tw.ground[i].is_water() != land {
        return tile_col(i);
    }
    // No tile of the right class under the pixel (a thin spit or inlet): nearest around it.
    let w = tw.width as i64;
    let (cx, cy) = (wx.floor() as i64, wy.floor() as i64);
    let mut best = (f32::MAX, i);
    for oy in -1i64..=1 {
        let ny = cy + oy;
        if ny < 0 || ny >= tw.height as i64 { continue; }
        for ox in -1i64..=1 {
            let k = ny as usize * tw.width + (cx + ox).rem_euclid(w) as usize;
            if tw.ground[k].is_water() == land { continue; }
            let d = (wx - (cx + ox) as f32 - 0.5).powi(2) + (wy - (cy + oy) as f32 - 0.5).powi(2);
            if d < best.0 { best = (d, k); }
        }
    }
    tile_col(best.1)
}

/// Smooth land indicator: 1 on land, 0 on water, with a little noise, so the 0.5 contour is a
/// rounded, irregular coastline instead of a staircase. It is sampled through a domain warp:
/// interpolating between tile centres alone keeps the tiles' axis-aligned outline, which made
/// lakes a few tiles across look like rounded rectangles.
#[inline]
fn land_field(tw: &TileWorld, wx: f32, wy: f32) -> f32 {
    let (sx, sy) = (
        wx + 0.42 * value_noise(tw, wx, wy, 0.9, 13),
        wy + 0.42 * value_noise(tw, wx, wy, 0.9, 14),
    );
    smooth_field(tw, sx, sy, |i| if tw.ground[i].is_water() { 0.0 } else { 1.0 })
        + 0.16 * value_noise(tw, wx, wy, 3.0, 11)
        + 0.04 * value_noise(tw, wx, wy, 6.0, 12)
}

/// Smooth water depth class (0 shallows/land, 1 sea, 2 deep ocean): its 1.5 contour is the
/// drawn edge of the deep ocean.
#[inline]
fn depth_field(tw: &TileWorld, wx: f32, wy: f32) -> f32 {
    use super::atlas::TileKind;
    smooth_field(tw, wx, wy, |i| match tw.ground[i] {
        TileKind::DeepOcean => 2.0,
        TileKind::Ocean | TileKind::Lake => 1.0,
        _ => 0.0,
    }) + 0.3 * value_noise(tw, wx, wy, 2.0, 41) + 0.08 * value_noise(tw, wx, wy, 6.0, 42)
}

/// Domain warp shared by everything that should not follow tile edges (ground kinds, borders).
#[inline]
fn warp(tw: &TileWorld, wx: f32, wy: f32) -> (f32, f32) {
    (
        wx + 0.32 * value_noise(tw, wx, wy, 1.0, 31) + 0.1 * value_noise(tw, wx, wy, 3.0, 33),
        wy + 0.32 * value_noise(tw, wx, wy, 1.0, 32) + 0.1 * value_noise(tw, wx, wy, 3.0, 34),
    )
}

/// Ink line of `half` width (tiles) along the `level` contour of `field` at (wx, wy):
/// returns coverage 0..1, using the field's numerical gradient for distance.
#[inline]
fn contour(field: impl Fn(f32, f32) -> f32, wx: f32, wy: f32, value: f32, level: f32, half: f32, t: f32) -> f32 {
    let e = 0.03;
    let gx = field(wx + e, wy) - field(wx - e, wy);
    let gy = field(wx, wy + e) - field(wx, wy - e);
    let grad = (gx * gx + gy * gy).sqrt() / (2.0 * e);
    if grad < 1e-3 { return 0.0; }
    let dist = (value - level).abs() / grad;
    ((half - dist) * t + 0.5).clamp(0.0, 1.0)
}

/// Sample an atlas tile at (u, v). When the tile has more pixels than the screen cell, four
/// sub-pixel samples are averaged (alpha-weighted), so thin ink lines fade instead of breaking up.
#[inline]
fn sample_tile(tile: &[[u8; 4]], s: usize, u: f32, v: f32, screen_px: f32) -> [f32; 4] {
    let at = |u: f32, v: f32| {
        let (x, y) = (((u * s as f32) as usize).min(s - 1), ((v * s as f32) as usize).min(s - 1));
        tile[y * s + x]
    };
    if (s as f32) <= screen_px * 1.05 {
        let p = at(u, v);
        return [p[0] as f32, p[1] as f32, p[2] as f32, p[3] as f32];
    }
    let d = 0.25 / screen_px;
    let mut acc = [0.0f32; 4];
    for (du, dv) in [(-d, -d), (d, -d), (-d, d), (d, d)] {
        let p = at((u + du).clamp(0.0, 0.9999), (v + dv).clamp(0.0, 0.9999));
        let a = p[3] as f32;
        for k in 0..3 { acc[k] += p[k] as f32 * a; }
        acc[3] += a;
    }
    if acc[3] <= 0.0 { return [0.0; 4]; }
    [acc[0] / acc[3], acc[1] / acc[3], acc[2] / acc[3], acc[3] / 4.0]
}

/// Faint world-anchored mottling, like uneven paper.
#[inline]
fn paper(tw: &TileWorld, wx: f32, wy: f32) -> f32 {
    1.0 + 0.035 * value_noise(tw, wx, wy, 1.0, 21) + 0.02 * value_noise(tw, wx, wy, 4.0, 22)
}

#[inline]
fn pack(c: [f32; 3]) -> u32 {
    let q = |v: f32| v.clamp(0.0, 255.0) as u32;
    (q(c[0]) << 16) | (q(c[1]) << 8) | q(c[2])
}

#[inline]
fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// Relief shading interpolated bilinearly between tile centres, so neighbouring tiles don't
/// jump in brightness (per-tile shading looks like a patchwork).
#[inline]
fn smooth_shade(tw: &TileWorld, wx: f32, wy: f32) -> f32 {
    let (fx, fy) = (wx - 0.5, (wy - 0.5).clamp(0.0, tw.height as f32 - 1.0));
    let (x0, y0) = (fx.floor(), fy.floor());
    let (tx, ty) = (fx - x0, fy - y0);
    let w = tw.width as i64;
    let xi = |dx: i64| ((x0 as i64 + dx).rem_euclid(w)) as usize;
    let yi = |dy: usize| (y0 as usize + dy).min(tw.height - 1);
    let s = |dx: i64, dy: usize| tw.shade[yi(dy) * tw.width + xi(dx)];
    let top = s(0, 0) * (1.0 - tx) + s(1, 0) * tx;
    let bot = s(0, 1) * (1.0 - tx) + s(1, 1) * tx;
    top * (1.0 - ty) + bot * ty
}

/// Screen position -> world tile coordinate under that pixel.
pub fn screen_to_world(cam: &Camera, sx: f32, sy: f32, w: usize, h: usize) -> (f32, f32) {
    (cam.cx + (sx - w as f32 / 2.0) / cam.tile_px, cam.cy + (sy - h as f32 / 2.0) / cam.tile_px)
}

pub fn render_world(tw: &TileWorld, atlas: &Atlas, cam: &Camera, buf: &mut [u32], w: usize, h: usize) {
    render_world_lod(tw, atlas, cam, buf, w, h, 1);
}

/// `render_world` computing one pixel in every `lod` x `lod` block (the rest are copies): a
/// cheap preview with the same look, for frames where the view is moving.
pub fn render_world_lod(tw: &TileWorld, atlas: &Atlas, cam: &Camera, buf: &mut [u32], w: usize, h: usize, lod: usize) {
    let lod = lod.max(1);
    let t = cam.tile_px;
    let detailed = t >= 4.0;
    let src_px = t.ceil() as usize;
    // Bands of `lod` rows are independent: render them in parallel.
    use rayon::prelude::*;
    buf[..w * h].par_chunks_mut(w * lod).enumerate().for_each(|(band, rows)| {
        let sy = band * lod;
        let wy = cam.cy + (sy as f32 + 0.5 * lod as f32 - h as f32 / 2.0) / t;
        if wy < 0.0 || wy >= tw.height as f32 {
            rows.fill(OFF_MAP);
            return;
        }
        let (row, rest) = rows.split_at_mut(w);
        let ty = wy as usize;
        let v = wy - ty as f32;
        for sx in (0..w).step_by(lod) {
            let wx = cam.cx + (sx as f32 + 0.5 * lod as f32 - w as f32 / 2.0) / t;
            let txf = wx.floor();
            let u = wx - txf;
            let tx = (txf as i64).rem_euclid(tw.width as i64) as usize;
            let i = ty * tw.width + tx;
            // Relief shading, softened and warm: shadows lean sepia rather than grey.
            let shade = 1.0 + (smooth_shade(tw, wx, wy) - 1.0) * 0.75;
            let sh = [shade.powf(0.8), shade, shade.powf(1.25)];

            let mottle = paper(tw, wx, wy);
            if !detailed {
                // Far zoom draws flat tile colours. Above ~1.5 px per tile a tile is several
                // pixels wide, so land or water comes from the smooth shoreline field (the fill
                // agrees with the coastline ink) and the colour from a warped tile lookup
                // (`far_color`): lakes, coasts and biome patches don't show the tile grid.
                let mut col = if t >= 1.5 {
                    far_color(tw, wx, wy, land_field(tw, wx, wy) >= 0.5)
                } else {
                    let c = tw.color[i];
                    let c = [c[0] as f32, c[1] as f32, c[2] as f32];
                    if tw.ground[i].is_water() && tw.season_frozen[i] { mix(c, ICE, 0.78) } else { c }
                };
                col = [0, 1, 2].map(|k| col[k] * sh[k] * mottle);
                // Coastline ink even when zoomed far out.
                if t >= 1.5 {
                    let lf = land_field(tw, wx, wy);
                    if (lf - 0.5).abs() < 0.45 {
                        let cover = contour(|x, y| land_field(tw, x, y), wx, wy, lf, 0.5, 0.6 / t, t);
                        col = mix(col, INK, 0.7 * cover);
                    }
                }
                let col = shadow_ink(tw, wx, wy, sx, sy, t, col);
                let col = overlay_tint(tw, wx, wy, col);
                row[sx] = pack(col);
                continue;
            }

            // Land or water is decided by the smooth coastline field, not by the tile square.
            let lf = land_field(tw, wx, wy);
            let is_land = lf >= 0.5;
            // The ground kind comes from a slightly warped position, so borders between biomes
            // (and between ocean depths) meander instead of following tile edges.
            let g = {
                let (warp_x, warp_y) = warp(tw, wx, wy);
                let j = tile_index(tw, warp_x, warp_y);
                if tw.ground[j].is_water() != is_land {
                    j
                } else if tw.ground[i].is_water() != is_land {
                    i
                } else {
                    // Nearest tile of the right class around this one.
                    let mut best = (f32::MAX, i);
                    for oy in -1i64..=1 {
                        let ny = ty as i64 + oy;
                        if ny < 0 || ny >= tw.height as i64 { continue; }
                        for ox in -1i64..=1 {
                            let nx = (tx as i64 + ox).rem_euclid(tw.width as i64) as usize;
                            let k = ny as usize * tw.width + nx;
                            if tw.ground[k].is_water() == is_land { continue; }
                            let d = (u - 0.5 - ox as f32).powi(2) + (v - 0.5 - oy as f32).powi(2);
                            if d < best.0 { best = (d, k); }
                        }
                    }
                    best.1
                }
            };

            // Open water takes its depth class from the smooth depth field, so the deep-ocean edge
            // is a drawn contour rather than a staircase of tiles.
            let depth = if is_land { 0.0 } else { depth_field(tw, wx, wy) };
            let ground = match tw.ground[g] {
                TileKind::DeepOcean | TileKind::Ocean | TileKind::Shallows => {
                    if depth >= 1.5 { TileKind::DeepOcean } else if depth >= 0.5 { TileKind::Ocean } else { TileKind::Shallows }
                }
                k => k,
            };
            let var = tw.variant[i] as usize;
            let (gt, gs) = atlas.tile_for(ground, var, src_px);
            let (gx, gy) = (((u * gs as f32) as usize).min(gs - 1), ((v * gs as f32) as usize).min(gs - 1));
            let gp = sample_tile(gt, gs, u, v, t);
            // Seasonal tint and snow blend smoothly between tiles (with a ragged edge for snow).
            let tint = [0, 1, 2].map(|k| smooth_field(tw, wx, wy, |n| tw.season_tint[n][k]));
            let mut col = [0, 1, 2].map(|k| gp[k] * sh[k] * tint[k]);
            let frozen = tw.season_frozen[g];
            let snow = if is_land {
                let s = smooth_field(tw, wx, wy, |n| tw.season_snow[n]);
                if s > 0.0 { (s + 0.25 * value_noise(tw, wx, wy, 4.0, 51)).clamp(0.0, 1.0) } else { 0.0 }
            } else {
                0.0
            };
            if frozen && tw.ground[g].is_water() {
                col = mix(col, ICE, 0.78);
            } else if snow > 0.0 {
                // Snow settles in a light, slightly mottled layer.
                let mottle = 0.9 + 0.1 * (((tx * 7 + ty * 13 + gx * 3 + gy * 5) % 5) as f32 / 4.0);
                col = mix(col, [240.0 * sh[0].min(1.1), 238.0 * sh[1].min(1.1), 232.0 * sh[2].min(1.1)], (snow * 0.88 * mottle).min(0.95));
            }

            // Coastline: an ink line along the smooth shore, a pale wash and two ripple lines
            // offshore, as on an engraved map.
            if (lf - 0.5).abs() < 0.45 {
                let e = 0.03;
                let gxd = land_field(tw, wx + e, wy) - land_field(tw, wx - e, wy);
                let gyd = land_field(tw, wx, wy + e) - land_field(tw, wx, wy - e);
                let grad = (gxd * gxd + gyd * gyd).sqrt() / (2.0 * e);
                if grad > 1e-3 {
                    let dist = (lf - 0.5) / grad; // tiles, positive on land
                    if !is_land {
                        let off = -dist;
                        if off < 0.3 { col = mix(col, COAST_WASH, 0.5 * (1.0 - off / 0.3)); }
                        if t >= 8.0 {
                            for (k, a) in [(1.0f32, 0.6f32), (2.0, 0.38)] {
                                let cover = (0.85 - (off - 0.13 * k).abs() * t).clamp(0.0, 1.0);
                                if cover > 0.0 { col = mix(col, SEA_INK, a * cover); }
                            }
                        }
                    }
                    let half = (0.7 / t).max(0.018);
                    let cover = ((half - dist.abs()) * t + 0.5).clamp(0.0, 1.0);
                    if cover > 0.0 { col = mix(col, INK, 0.85 * cover); }
                }
            }

            // Faint ink contour along the edge of the deep ocean.
            if !is_land && t >= 6.0 && (depth - 1.5).abs() < 0.4 {
                let cover = contour(|x, y| depth_field(tw, x, y), wx, wy, depth, 1.5, (0.5 / t).max(0.012), t);
                if cover > 0.0 { col = mix(col, SEA_INK, 0.35 * cover); }
            }

            // River channel, drawn under sprites so trees overhang it. Strokes from the 8
            // neighbouring tiles are included so diagonal rivers don't pinch at tile corners.
            if tw.river_near[i] {
                // Signed distance to the nearest river edge, along the curved strokes.
                let edge = tw.river_strokes.edge_distance(tw.width, tw.height, tx, ty, u, v, 0.6);
                let aa = 1.0 / t;
                // Ink banks a little outside the channel, then the water on top.
                let bank = (0.8 / t).min(0.04);
                let ink_cover = ((aa - (edge - bank)) / (2.0 * aa)).clamp(0.0, 1.0);
                if ink_cover > 0.0 && is_land {
                    col = mix(col, INK, 0.75 * ink_cover);
                }
                let cover = ((aa - edge) / (2.0 * aa)).clamp(0.0, 1.0);
                if cover > 0.0 {
                    col = mix(col, if frozen { ICE } else { RIVER }, cover);
                }
            }

            // Roads: thin dirt tracks along the curves built by `TileWorld::build_road_curves`.
            if tw.road_near[i] && t >= 3.0 {
                // A tile's road stays within about a quarter tile of it.
                let edge = tw.road_strokes.edge_distance(tw.width, tw.height, tx, ty, u, v, 0.35) - ROAD_HALF_WIDTH;
                let aa = 1.0 / t;
                let cover = ((aa - edge) / (2.0 * aa)).clamp(0.0, 1.0);
                if cover > 0.0 && is_land {
                    col = mix(col, ROAD, cover * 0.85);
                }
            }

            // Territory borders: a dotted line where the owner changes, sampled through the same
            // warp as the ground so borders meander like the biomes do.
            if is_land && t >= 10.0 && tw.owner_edge[i] {
                let owner_at = |x: f32, y: f32| {
                    let (a, b) = warp(tw, x, y);
                    tw.owner[tile_index(tw, a, b)]
                };
                let own = owner_at(wx, wy);
                let band = (1.2 / t).max(0.03);
                let border = own != u64::MAX
                    && [(band, 0.0), (-band, 0.0), (0.0, band), (0.0, -band)].iter().any(|&(dx, dy)| owner_at(wx + dx, wy + dy) != own);
                let dot = (((wx + wy) * t) as i64 / 3) % 2 == 0;
                if border && dot {
                    let c = super::classify::faction_color(own);
                    col = mix(col, [((c >> 16) & 255) as f32, ((c >> 8) & 255) as f32, (c & 255) as f32], 0.7);
                }
            }

            if let Some(sp) = tw.sprite[i].filter(|_| is_land || tw.ground[i].is_water()) {
                let (st, ss) = atlas.tile_for(sp, var, src_px);
                let p = sample_tile(st, ss, u, v, t);
                if p[3] > 0.0 {
                    let a = p[3] / 255.0;
                    let mut sc = [0, 1, 2].map(|k| p[k] * sh[k] * tint[k]);
                    // Trees and rooftops carry some snow too.
                    if snow > 0.0 { sc = mix(sc, [240.0, 238.0, 232.0], snow * 0.45); }
                    col = mix(col, sc, a);
                }
            }

            // Resource marker: a diamond in the tile's upper-right corner, size by richness.
            if tw.show_resources && t >= 5.0 {
                if let Some((rc, rich)) = tw.deposit[i] {
                    let r = 0.14 + 0.05 * rich as f32;
                    let d = (u - 0.72).abs() + (v - 0.28).abs();
                    if d < r {
                        col = if d > r - 1.5 / t { [20.0, 20.0, 24.0] } else { [rc[0] as f32, rc[1] as f32, rc[2] as f32] };
                    }
                }
            }

            let col = shadow_ink(tw, wx, wy, sx, sy, t, col);
            let col = overlay_tint(tw, wx, wy, col);
            row[sx] = pack([col[0] * mottle, col[1] * mottle, col[2] * mottle]);
        }
        if lod > 1 {
            for sx in (0..w).step_by(lod) {
                let v = row[sx];
                row[sx..(sx + lod).min(w)].fill(v);
            }
            for r in rest.chunks_mut(w) { r.copy_from_slice(&row[..r.len()]); }
        }
    });
    draw_shadow_seat(tw, cam, buf, w, h);
}

/// A data overlay washed over the map. The map's own light and dark (relief, ink lines) still
/// modulate it, so coastlines, rivers and labels stay readable through the colour.
#[inline]
fn overlay_tint(tw: &TileWorld, wx: f32, wy: f32, col: [f32; 3]) -> [f32; 3] {
    if tw.overlay.is_empty() { return col; }
    let o = if tw.overlay_smooth {
        [0, 1, 2].map(|k| smooth_field(tw, wx, wy, |i| tw.overlay[i][k]))
    } else {
        tw.overlay[tile_index(tw, wx, wy)]
    };
    let lum = (col[0] * 0.3 + col[1] * 0.59 + col[2] * 0.11) / 200.0;
    let lum = lum.clamp(0.25, 1.15);
    mix(col, [o[0] * lum, o[1] * lum, o[2] * lum], 0.78)
}

/// Ink for the Shadow's own lands and its seat.
const SHADOW_INK: [f32; 3] = [30.0, 20.0, 26.0];

/// The Shadow on the map, in three layers: its *reach* drains the colour from the land (a cold
/// grey wash that deepens with corruption), its *blight* is in the tiles themselves (ash, dead
/// woods; see `TileWorld::apply_history`), and its *dominion* is cross-hatched in dark ink,
/// densely where the corruption is deepest, inside a jagged border.
#[inline]
fn shadow_ink(tw: &TileWorld, wx: f32, wy: f32, sx: usize, sy: usize, t: f32, col: [f32; 3]) -> [f32; 3] {
    if tw.shadow.is_empty() { return col; }
    let i = tile_index(tw, wx, wy);
    if !tw.shadow_near[i] { return col; }
    let c = smooth_field(tw, wx, wy, |i| if tw.ground[i].is_water() { 0.0 } else { tw.shadow[i] });
    let mut col = col;
    if c > 0.08 {
        let s = ((c - 0.08) / 0.55).clamp(0.0, 1.0);
        let s = s * s * (3.0 - 2.0 * s);
        let lum = col[0] * 0.3 + col[1] * 0.59 + col[2] * 0.11;
        col = mix(col, [lum * 0.98, lum * 0.94, lum * 0.97], 0.8 * s);
        col = [col[0] * (1.0 - 0.3 * s), col[1] * (1.0 - 0.3 * s), col[2] * (1.0 - 0.3 * s)];
    }
    let dominion = |x: f32, y: f32| {
        smooth_field(tw, x, y, |i| tw.dominion[i] as u8 as f32)
            + 0.16 * value_noise(tw, x, y, 4.0, 77)
            + 0.07 * value_noise(tw, x, y, 11.0, 78)
    };
    // Away from a border the dominion field is plainly in or out (its noise can't cross 0.5).
    let edge = tw.dominion_edge[i];
    let d = if edge { dominion(wx, wy) } else if tw.dominion[i] { 1.0 } else { 0.0 };
    if d > 0.5 {
        // Hatching in screen space, like a pen: one diagonal, crossed where the hold is deepest.
        let gap = if t < 3.0 { 4 } else { 5 };
        let a = (sx + sy) % gap == 0;
        let b = (sx + gap * 64 - sy % (gap * 64)) % gap == 0;
        if a || (c > 0.7 && b) {
            col = mix(col, SHADOW_INK, 0.6);
        }
    }
    if edge && (d - 0.5).abs() < 0.4 {
        let cover = contour(dominion, wx, wy, d, 0.5, (1.4 / t).max(0.05), t);
        col = mix(col, SHADOW_INK, 0.9 * cover);
    }
    col
}

/// The Shadow's seat: a black tower with a red eye.
fn draw_shadow_seat(tw: &TileWorld, cam: &Camera, buf: &mut [u32], w: usize, h: usize) {
    let Some((x, y)) = tw.shadow_seat else { return };
    let ww = tw.width as f32;
    let mut dx = x as f32 + 0.5 - cam.cx;
    if dx > ww / 2.0 { dx -= ww; }
    if dx < -ww / 2.0 { dx += ww; }
    let cx = w as f32 / 2.0 + dx * cam.tile_px;
    let base = h as f32 / 2.0 + (y as f32 + 0.5 - cam.cy) * cam.tile_px;
    let s = (cam.tile_px * 1.1).clamp(9.0, 40.0);
    let ink = pack(SHADOW_INK);
    let halo = pack([226.0, 214.0, 190.0]);
    let mut put = |px: f32, py: f32, c: u32| {
        let (px, py) = (px as i64, py as i64);
        if px >= 0 && py >= 0 && (px as usize) < w && (py as usize) < h { buf[py as usize * w + px as usize] = c; }
    };
    // Tapering shaft, horned crown, on a parchment halo so it reads on any ground.
    let top = base - s * 1.3;
    for pass in 0..2 {
        let c = if pass == 0 { halo } else { ink };
        let grow = if pass == 0 { 1.5 } else { 0.0 };
        let mut py = top - grow;
        while py <= base + grow {
            let f = (py - top) / (base - top);
            let half = s * (0.12 + 0.16 * f) + grow;
            let mut px = cx - half;
            while px <= cx + half { put(px, py, c); px += 1.0; }
            py += 1.0;
        }
        for k in 0..(s * 0.35) as i64 {
            let k = k as f32;
            for side in [-1.0f32, 1.0] {
                for g in 0..=(grow as i64) * 2 {
                    put(cx + side * (s * 0.12 + k * 0.45) + side * g as f32 * 0.5, top - k - grow, c);
                }
            }
        }
    }
    let eye = pack([200.0, 40.0, 24.0]);
    for ey in -1..=1 {
        for ex in -2..=2 {
            if ex * ex + ey * ey * 4 <= 5 { put(cx + ex as f32, top + s * 0.3 + ey as f32, eye); }
        }
    }
}

/// Minimap in the top-right corner: tile colours, the visible area in yellow and an optional
/// marker (the zoom window) in white. Returns its screen rectangle (x, y, w, h).
pub fn render_minimap(
    tw: &TileWorld,
    cam: &Camera,
    marker: Option<(f32, f32, f32)>,
    buf: &mut [u32],
    w: usize,
    h: usize,
) -> (usize, usize, usize, usize) {
    let mw = (w / 4).clamp(64, 360);
    let mh = (mw * tw.height / tw.width).max(1);
    if mw + 12 > w || mh + 12 > h {
        return (0, 0, 0, 0);
    }
    let (ox, oy) = (w - mw - 10, 10);
    for y in oy - 2..oy + mh + 2 {
        for x in ox - 2..ox + mw + 2 {
            buf[y * w + x] = 0x00C8_C8C8;
        }
    }
    for y in 0..mh {
        for x in 0..mw {
            let i = (y * tw.height / mh) * tw.width + x * tw.width / mw;
            let c = tw.color[i];
            buf[(oy + y) * w + ox + x] = pack([c[0] as f32, c[1] as f32, c[2] as f32]);
        }
    }
    let to_mm = |wx: f32, wy: f32| -> (i64, i64) {
        (
            (wx.rem_euclid(tw.width as f32) / tw.width as f32 * mw as f32) as i64,
            (wy.clamp(0.0, tw.height as f32 - 0.01) / tw.height as f32 * mh as f32) as i64,
        )
    };
    let mut rect = |x0: f32, y0: f32, x1: f32, y1: f32, color: u32| {
        let (ax, ay) = to_mm(x0, y0);
        let (bx, by) = to_mm(x1, y1);
        let span = if bx >= ax { bx - ax } else { bx + mw as i64 - ax };
        for k in 0..=span {
            let x = ((ax + k) % mw as i64) as usize;
            buf[(oy + ay as usize) * w + ox + x] = color;
            buf[(oy + by as usize) * w + ox + x] = color;
        }
        for y in ay.min(by)..=ay.max(by) {
            buf[(oy + y as usize) * w + ox + ax as usize] = color;
            buf[(oy + y as usize) * w + ox + bx as usize] = color;
        }
    };
    let vw = (w as f32 / cam.tile_px).min(tw.width as f32 - 1.0);
    let vh = (h as f32 / cam.tile_px).min(tw.height as f32 - 1.0);
    rect(cam.cx - vw / 2.0, cam.cy - vh / 2.0, cam.cx + vw / 2.0, cam.cy + vh / 2.0, 0x00F0_D23C);
    if let Some((mx, my, half)) = marker {
        rect(mx - half, my - half, mx + half, my + half, 0x00FF_FFFF);
    }
    (ox, oy, mw, mh)
}

/// The zoomed region (pre-rendered RGB) scaled to the screen, nearest-neighbour.
pub fn render_zoom(rgb: &[[u8; 3]], rw: usize, rh: usize, cam: &ZoomCamera, buf: &mut [u32], w: usize, h: usize) {
    for sy in 0..h {
        let ry = cam.cy + (sy as f32 + 0.5 - h as f32 / 2.0) / cam.px_per_cell;
        for sx in 0..w {
            let rx = cam.cx + (sx as f32 + 0.5 - w as f32 / 2.0) / cam.px_per_cell;
            buf[sy * w + sx] = if rx < 0.0 || ry < 0.0 || rx >= rw as f32 || ry >= rh as f32 {
                OFF_MAP
            } else {
                let c = rgb[ry as usize * rw + rx as usize];
                ((c[0] as u32) << 16) | ((c[1] as u32) << 8) | c[2] as u32
            };
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Local (playable area) rendering
// ---------------------------------------------------------------------------------------------

use crate::erosion::materials::RockType;
use crate::local::{Cell, LocalMap, Material, Plant, Shape, TreeKind};
use super::atlas::TileKind;

/// Local-map camera: tile at the screen centre, tile size, viewed z-level, and whether to show
/// every column at its own surface instead of a single level.
#[derive(Clone, Copy, Debug)]
pub struct LocalCamera {
    pub cx: f32,
    pub cy: f32,
    pub tile_px: f32,
    pub z: i32,
    pub surface_view: bool,
}

/// Colour multiplier per material, so walls and floors show what they are made of.
fn material_tint(m: Material) -> [f32; 3] {
    match m {
        // Distinct per rock so strata read clearly in walls and the cross-section.
        Material::Rock(RockType::Granite) => [1.15, 0.86, 0.84],
        Material::Rock(RockType::Basalt) => [0.50, 0.52, 0.60],
        Material::Rock(RockType::Sandstone) => [1.35, 1.02, 0.66],
        Material::Rock(RockType::Limestone) => [1.32, 1.30, 1.18],
        Material::Rock(RockType::Shale) => [0.70, 0.74, 0.84],
        Material::Rock(RockType::Sediment) => [1.10, 0.92, 0.70],
        Material::Rock(RockType::Ice) | Material::Ice => [0.95, 1.10, 1.25],
        Material::Clay => [1.18, 0.82, 0.70],
        Material::Sand => [1.45, 1.25, 0.86],
        Material::Gravel => [1.05, 1.02, 0.98],
        _ => [1.0, 1.0, 1.0],
    }
}

fn tree_kind(t: TreeKind) -> TileKind {
    match t {
        TreeKind::Broadleaf => TileKind::BigBroadleaf,
        TreeKind::Conifer => TileKind::BigConifer,
        TreeKind::Jungle => TileKind::BigJungle,
        TreeKind::Palm => TileKind::Palm,
        TreeKind::Acacia => TileKind::Acacia,
        TreeKind::Dead => TileKind::DeadTree,
    }
}

/// Ground tile, tint and sprite for a solid or floor cell.
fn cell_tiles(c: &Cell) -> (TileKind, [f32; 3], Option<TileKind>) {
    let tint = material_tint(c.material);
    match c.shape {
        Shape::Wall => {
            let kind = match c.material {
                Material::Wood => TileKind::WoodWall,
                Material::Block(_) => TileKind::BlockWall,
                Material::Ore(_) => TileKind::OreWall,
                Material::Rock(_) | Material::Ice => TileKind::StoneWall,
                _ => TileKind::SoilWall,
            };
            let tint = if matches!(c.material, Material::Wood | Material::Block(_)) { [1.0; 3] } else { tint };
            (kind, tint, None)
        }
        _ => {
            let vegetated = !matches!(c.plant, Plant::None);
            let ground = match c.material {
                Material::Soil | Material::Clay if vegetated => TileKind::Grass,
                Material::Soil | Material::Clay => TileKind::Dirt,
                Material::Sand => TileKind::Sand,
                Material::Gravel => TileKind::Gravel,
                Material::Snow => TileKind::Snow,
                Material::Ice => TileKind::SeaIce,
                Material::Rock(_) | Material::Block(_) | Material::Ore(_) => TileKind::StoneFloor,
                Material::Wood => TileKind::WoodFloor,
                Material::Air => TileKind::Dirt,
            };
            let ground_tint = if matches!(ground, TileKind::Grass | TileKind::Sand | TileKind::Snow | TileKind::SeaIce) { [1.0; 3] } else { tint };
            let sprite = if c.shape == Shape::Ramp {
                Some(TileKind::Ramp)
            } else {
                match c.plant {
                    Plant::Tree(t) => Some(tree_kind(t)),
                    Plant::Shrub => Some(TileKind::Shrub),
                    Plant::Crop(_) => Some(TileKind::Crops),
                    _ if c.boulder => Some(TileKind::Boulder),
                    _ => None,
                }
            };
            (ground, ground_tint, sprite)
        }
    }
}

fn water_kind(levels: f32) -> TileKind {
    if levels <= 1.0 { TileKind::Shallows } else if levels <= 3.0 { TileKind::Lake } else { TileKind::Ocean }
}

#[inline]
fn tile_px(atlas: &Atlas, kind: TileKind, var: usize, u: f32, v: f32, px: usize) -> [u8; 4] {
    let (t, s) = atlas.tile_for(kind, var, px);
    t[((v * s as f32) as usize).min(s - 1) * s + ((u * s as f32) as usize).min(s - 1)]
}

pub fn render_local(map: &LocalMap, atlas: &Atlas, cam: &LocalCamera, buf: &mut [u32], w: usize, h: usize) {
    // The surface is drawn in ink (local_ink.rs); z-level slices keep the tiles below.
    if cam.surface_view {
        super::local_ink::render_local_ink(map, cam, buf, w, h);
        return;
    }
    let t = cam.tile_px;
    let src_px = t.ceil() as usize;
    let (mw, mh) = (map.width, map.height);
    for sy in 0..h {
        let fy = cam.cy + (sy as f32 + 0.5 - h as f32 / 2.0) / t;
        for sx in 0..w {
            let fx = cam.cx + (sx as f32 + 0.5 - w as f32 / 2.0) / t;
            if fx < 0.0 || fy < 0.0 || fx >= mw as f32 || fy >= mh as f32 {
                buf[sy * w + sx] = OFF_MAP;
                continue;
            }
            let (tx, ty) = (fx as usize, fy as usize);
            let (u, v) = (fx - tx as f32, fy - ty as f32);
            let col_i = ty * mw + tx;
            let var = ((tx * 7 + ty * 13) ^ (tx * ty)) % 4;
            let sz = map.surface_z[col_i];

            // Which cell to draw, and how much to dim it (looking down through open space).
            let mut dim = 1.0f32;
            // Surface view shows the top of whatever stands on the ground (walls of buildings).
            let top = {
                let mut z = sz;
                while (z + 1) < map.depth as i32 && z < sz + 3 && map.cell(tx, ty, (z + 1) as usize).shape == Shape::Wall { z += 1; }
                z
            };
            let mut draw_z = if cam.surface_view { top } else { cam.z };
            let mut water_levels = 0.0f32;
            if cam.surface_view {
                let mut z = sz + 1;
                while (z as usize) < map.depth && map.cell(tx, ty, z as usize).water > 0 {
                    water_levels += map.cell(tx, ty, z as usize).water as f32 / crate::local::WATER_FULL as f32;
                    z += 1;
                }
            } else {
                if draw_z < 0 || draw_z as usize >= map.depth {
                    buf[sy * w + sx] = OFF_MAP;
                    continue;
                }
                let here = map.cell(tx, ty, draw_z as usize);
                if here.shape == Shape::Empty {
                    if here.water > 0 {
                        // Water at this level: depth is this cell plus the water below it.
                        let mut z = draw_z;
                        while z > sz {
                            water_levels += map.cell(tx, ty, z as usize).water as f32 / crate::local::WATER_FULL as f32;
                            z -= 1;
                        }
                    } else {
                        // Open space: show the first thing below, darker the further down.
                        let mut k = 1;
                        while k <= 10 && draw_z - k >= 0 {
                            let c = map.cell(tx, ty, (draw_z - k) as usize);
                            if c.shape != Shape::Empty || c.water > 0 { break; }
                            k += 1;
                        }
                        if k > 10 || draw_z - k < 0 {
                            buf[sy * w + sx] = 0x0008_080A;
                            continue;
                        }
                        draw_z -= k;
                        dim = 0.80f32.powi(k);
                        let c = map.cell(tx, ty, draw_z as usize);
                        if c.water > 0 {
                            let mut z = draw_z;
                            while z > sz {
                                water_levels += map.cell(tx, ty, z as usize).water as f32 / crate::local::WATER_FULL as f32;
                                z -= 1;
                            }
                        }
                    }
                }
            }

            // Ice over water reads as a frozen surface, not as ground.
            // Frozen water: an ice floor directly over water, or ice on ground that sits below the
            // smooth surface (a frozen stream bed).
            let frozen = water_levels == 0.0 && {
                let c = map.cell(tx, ty, draw_z.clamp(0, map.depth as i32 - 1) as usize);
                c.material == Material::Ice && c.shape != Shape::Wall
            };
            let mut col = if frozen {
                // Clear blue ice, distinct from the white snow around it.
                let p = tile_px(atlas, TileKind::SeaIce, var, u, v, src_px);
                [p[0] as f32 * 0.70, p[1] as f32 * 0.86, p[2] as f32 * 1.0]
            } else if water_levels > 0.0 {
                let p = tile_px(atlas, water_kind(water_levels), var, u, v, src_px);
                [p[0] as f32, p[1] as f32, p[2] as f32]
            } else {
                let cell = map.cell(tx, ty, draw_z.clamp(0, map.depth as i32 - 1) as usize);
                let (ground, tint, sprite) = cell_tiles(cell);
                let p = tile_px(atlas, ground, var, u, v, src_px);
                let mut c = [p[0] as f32 * tint[0], p[1] as f32 * tint[1], p[2] as f32 * tint[2]];
                // Ore flecks (the bright pixels of the ore-wall tile) take the ore's colour.
                if let (TileKind::OreWall, Material::Ore(r)) = (ground, cell.material) {
                    if p[0] > 235 {
                        let rc = crate::lore::resource_color(r);
                        c = [rc[0] as f32, rc[1] as f32, rc[2] as f32];
                    }
                }
                if let Some(sp) = sprite {
                    let q = tile_px(atlas, sp, var, u, v, src_px);
                    if q[3] > 0 {
                        c = mix(c, [q[0] as f32, q[1] as f32, q[2] as f32], q[3] as f32 / 255.0);
                    }
                }
                // Animal signs on the surface.
                if draw_z == sz {
                    use crate::local::wildlife::Feature;
                    let mark = match map.features[col_i] {
                        Feature::None => None,
                        Feature::Trail => {
                            // Trodden earth whatever the soil, a little of the ground showing through.
                            let d = tile_px(atlas, TileKind::Dirt, var, u, v, src_px);
                            c = mix(c, [d[0] as f32 * 0.92, d[1] as f32 * 0.9, d[2] as f32 * 0.86], 0.75);
                            None
                        }
                        Feature::Burrow => Some(TileKind::Burrow),
                        Feature::Nest => Some(TileKind::Nest),
                        Feature::Den => Some(TileKind::Den),
                        Feature::Bones | Feature::Grave => Some(TileKind::Bones),
                        Feature::Stone | Feature::Spring => None,
                    };
                    if let Some(k) = mark {
                        let q = tile_px(atlas, k, var, u, v, src_px);
                        if q[3] > 0 {
                            c = mix(c, [q[0] as f32, q[1] as f32, q[2] as f32], q[3] as f32 / 255.0);
                        }
                    }
                }
                c
            };

            // Relief shading from the surface heights (surface view and visible floors).
            if cam.surface_view || draw_z == sz {
                let at = |x: i64, y: i64| map.surface_m[(y.clamp(0, mh as i64 - 1) as usize) * mw + x.clamp(0, mw as i64 - 1) as usize];
                let (x, y) = (tx as i64, ty as i64);
                let g = (at(x - 1, y) - at(x + 1, y)) + (at(x, y - 1) - at(x, y + 1));
                let shade = (1.0 + g / 12.0).clamp(0.7, 1.3);
                col = [col[0] * shade, col[1] * shade, col[2] * shade];
            }
            buf[sy * w + sx] = pack([col[0] * dim, col[1] * dim, col[2] * dim]);
        }
    }
}

/// Side view of one row of the map (x across, z up): strata, surface and water.
pub fn render_cross_section(map: &LocalMap, row: usize, px: usize) -> image::RgbImage {
    let (w, d) = (map.width, map.depth);
    image::RgbImage::from_fn((w * px) as u32, (d * px) as u32, |x, y| {
        let (tx, z) = (x as usize / px, d - 1 - y as usize / px);
        let c = map.cell(tx, row, z);
        let rgb = if c.shape == Shape::Empty {
            if c.water > 0 { [50, 110, 175] } else { [200, 222, 240] }
        } else {
            let base = match c.material {
                Material::Soil => [120, 86, 52],
                Material::Clay => [150, 92, 70],
                Material::Sand => [222, 196, 132],
                Material::Gravel => [150, 146, 138],
                Material::Snow => [240, 244, 250],
                Material::Ice => [190, 220, 240],
                Material::Rock(_) | Material::Air => [125, 120, 115],
                Material::Wood => [140, 96, 54],
                Material::Block(_) => [180, 176, 168],
                Material::Ore(r) => crate::lore::resource_color(r),
            };
            let t = material_tint(c.material);
            let f = if c.shape == Shape::Wall { 1.0 } else { 1.15 };
            let rock = matches!(c.material, Material::Rock(_));
            let (r, g, b) = if rock {
                ((base[0] as f32 * t[0] * f) as u8, (base[1] as f32 * t[1] * f) as u8, (base[2] as f32 * t[2] * f) as u8)
            } else {
                ((base[0] as f32 * f).min(255.0) as u8, (base[1] as f32 * f).min(255.0) as u8, (base[2] as f32 * f).min(255.0) as u8)
            };
            match c.plant {
                Plant::Tree(_) if c.shape != Shape::Wall => [40, 100, 40],
                Plant::Grass => [90, 150, 60],
                _ => [r, g, b],
            }
        };
        image::Rgb(rgb)
    })
}
