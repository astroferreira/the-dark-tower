//! Pure software rendering of the tile view into a `0x00RRGGBB` pixel buffer.
//! Kept free of any window code so frames can be rendered (and checked) headlessly.

use super::atlas::Atlas;
use super::classify::{TileWorld, DIRS};

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

const OFF_MAP: u32 = 0x0010_1014;
const RIVER: [f32; 3] = [58.0, 118.0, 190.0];
const FOAM: [f32; 3] = [210.0, 232.0, 240.0];

#[inline]
fn pack(c: [f32; 3]) -> u32 {
    let q = |v: f32| v.clamp(0.0, 255.0) as u32;
    (q(c[0]) << 16) | (q(c[1]) << 8) | q(c[2])
}

#[inline]
fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// Distance from (u, v) to the segment from the tile centre to the edge/corner in direction d.
#[inline]
fn seg_dist(u: f32, v: f32, d: (i32, i32)) -> f32 {
    let (ex, ey) = (0.5 * d.0 as f32, 0.5 * d.1 as f32);
    let (px, py) = (u - 0.5, v - 0.5);
    let t = ((px * ex + py * ey) / (ex * ex + ey * ey)).clamp(0.0, 1.0);
    let (dx, dy) = (px - ex * t, py - ey * t);
    (dx * dx + dy * dy).sqrt()
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
    let t = cam.tile_px;
    let detailed = t >= 4.0;
    let src_px = t.ceil() as usize;
    for sy in 0..h {
        let wy = cam.cy + (sy as f32 + 0.5 - h as f32 / 2.0) / t;
        if wy < 0.0 || wy >= tw.height as f32 {
            buf[sy * w..(sy + 1) * w].iter_mut().for_each(|p| *p = OFF_MAP);
            continue;
        }
        let ty = wy as usize;
        let v = wy - ty as f32;
        for sx in 0..w {
            let wx = cam.cx + (sx as f32 + 0.5 - w as f32 / 2.0) / t;
            let txf = wx.floor();
            let u = wx - txf;
            let tx = (txf as i64).rem_euclid(tw.width as i64) as usize;
            let i = ty * tw.width + tx;
            let shade = smooth_shade(tw, wx, wy);

            if !detailed {
                let c = tw.color[i];
                buf[sy * w + sx] = pack([c[0] as f32 * shade, c[1] as f32 * shade, c[2] as f32 * shade]);
                continue;
            }

            let var = tw.variant[i] as usize;
            let (gt, gs) = atlas.tile_for(tw.ground[i], var, src_px);
            let (gx, gy) = (((u * gs as f32) as usize).min(gs - 1), ((v * gs as f32) as usize).min(gs - 1));
            let gp = gt[gy * gs + gx];
            let mut col = [gp[0] as f32 * shade, gp[1] as f32 * shade, gp[2] as f32 * shade];

            // Coastline foam on water tiles next to land.
            let shore = tw.shore[i];
            if shore != 0 {
                let mut d = f32::MAX;
                for (b, dir) in DIRS.iter().enumerate() {
                    if shore & (1 << b) == 0 { continue; }
                    let dist = match dir {
                        (0, -1) => v,
                        (1, 0) => 1.0 - u,
                        (0, 1) => 1.0 - v,
                        (-1, 0) => u,
                        (dx, dy) => {
                            let cx = if *dx > 0 { 1.0 } else { 0.0 };
                            let cy = if *dy > 0 { 1.0 } else { 0.0 };
                            ((u - cx).powi(2) + (v - cy).powi(2)).sqrt()
                        }
                    };
                    d = d.min(dist);
                }
                let wobble = 0.03 * ((u * 17.0 + v * 11.0 + tx as f32 * 3.1).sin());
                if d < 0.10 + wobble {
                    col = mix(col, FOAM, 0.75);
                } else if d < 0.18 + wobble {
                    col = mix(col, FOAM, 0.25);
                }
            }

            // River channel, drawn under sprites so trees overhang it. Strokes from the 8
            // neighbouring tiles are included so diagonal rivers don't pinch at tile corners.
            if tw.river_near[i] {
                let mut edge = f32::MAX; // signed distance to the nearest stroke boundary
                for oy in -1i64..=1 {
                    let ny = ty as i64 + oy;
                    if ny < 0 || ny >= tw.height as i64 { continue; }
                    for ox in -1i64..=1 {
                        let nx = (tx as i64 + ox).rem_euclid(tw.width as i64) as usize;
                        let j = ny as usize * tw.width + nx;
                        let mask = tw.river[j];
                        if mask == 0 { continue; }
                        let (lu, lv) = (u - ox as f32, v - oy as f32);
                        for (b, dir) in DIRS.iter().enumerate() {
                            if mask & (1 << b) != 0 {
                                edge = edge.min(seg_dist(lu, lv, *dir) - tw.river_width[j]);
                            }
                        }
                    }
                }
                let aa = 1.0 / t;
                let cover = ((aa - edge) / (2.0 * aa)).clamp(0.0, 1.0);
                if cover > 0.0 {
                    col = mix(col, RIVER, cover);
                }
            }

            if let Some(sp) = tw.sprite[i] {
                let (st, ss) = atlas.tile_for(sp, var, src_px);
                let (px, py) = (((u * ss as f32) as usize).min(ss - 1), ((v * ss as f32) as usize).min(ss - 1));
                let p = st[py * ss + px];
                if p[3] > 0 {
                    let a = p[3] as f32 / 255.0;
                    col = mix(col, [p[0] as f32 * shade, p[1] as f32 * shade, p[2] as f32 * shade], a);
                }
            }

            buf[sy * w + sx] = pack(col);
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
                Material::Rock(_) | Material::Ice => TileKind::StoneWall,
                _ => TileKind::SoilWall,
            };
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
                Material::Rock(_) => TileKind::StoneFloor,
                Material::Air => TileKind::Dirt,
            };
            let ground_tint = if matches!(ground, TileKind::Grass | TileKind::Sand | TileKind::Snow | TileKind::SeaIce) { [1.0; 3] } else { tint };
            let sprite = if c.shape == Shape::Ramp {
                Some(TileKind::Ramp)
            } else {
                match c.plant {
                    Plant::Tree(t) => Some(tree_kind(t)),
                    Plant::Shrub => Some(TileKind::Shrub),
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
            let mut draw_z = if cam.surface_view { sz } else { cam.z };
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
                if let Some(sp) = sprite {
                    let q = tile_px(atlas, sp, var, u, v, src_px);
                    if q[3] > 0 {
                        c = mix(c, [q[0] as f32, q[1] as f32, q[2] as f32], q[3] as f32 / 255.0);
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
