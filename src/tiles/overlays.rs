//! Data overlays for the tile viewer: the world's raw fields (height, temperature, moisture,
//! drainage, tectonic plates and stress, biome classes) washed over the ink map in muted
//! palettes, with a legend. These are the views the old terminal explorer cycled with V,
//! redrawn for the tile viewer (`O` cycles them).

use crate::world::WorldData;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overlay {
    None,
    Height,
    Temperature,
    Moisture,
    Drainage,
    Plates,
    Stress,
    Biomes,
}

const ORDER: [Overlay; 8] = [
    Overlay::None, Overlay::Height, Overlay::Temperature, Overlay::Moisture,
    Overlay::Drainage, Overlay::Plates, Overlay::Stress, Overlay::Biomes,
];

/// How a legend reads: a colour ramp between labelled values, or a note for categories.
pub enum Legend {
    Ramp { stops: Vec<(f32, [u8; 3])>, labels: Vec<(f32, String)> },
    Note(&'static str),
}

impl Overlay {
    pub fn cycle(self, d: i32) -> Overlay {
        let i = ORDER.iter().position(|o| *o == self).unwrap_or(0) as i32;
        ORDER[(i + d).rem_euclid(ORDER.len() as i32) as usize]
    }

    pub fn name(self) -> &'static str {
        match self {
            Overlay::None => "Map",
            Overlay::Height => "Height",
            Overlay::Temperature => "Temperature",
            Overlay::Moisture => "Moisture",
            Overlay::Drainage => "Drainage",
            Overlay::Plates => "Tectonic plates",
            Overlay::Stress => "Tectonic stress",
            Overlay::Biomes => "Biomes",
        }
    }

    /// Continuous fields are blended between tile centres; categories keep crisp edges.
    pub fn smooth(self) -> bool {
        !matches!(self, Overlay::Plates | Overlay::Biomes)
    }

    pub fn legend(self) -> Legend {
        let lab = |v: &[(f32, &str)]| v.iter().map(|(x, s)| (*x, s.to_string())).collect();
        match self {
            Overlay::None => Legend::Note(""),
            Overlay::Height => Legend::Ramp { stops: HEIGHT.to_vec(), labels: lab(&[(-4000.0, "-4 km"), (0.0, "sea level"), (4500.0, "4.5 km")]) },
            Overlay::Temperature => Legend::Ramp { stops: TEMP.to_vec(), labels: lab(&[(-30.0, "-30 C"), (0.0, "0"), (24.0, "24 C")]) },
            Overlay::Moisture => Legend::Ramp { stops: MOIST.to_vec(), labels: lab(&[(0.0, "dry"), (0.3, "0.3"), (0.8, "wet")]) },
            Overlay::Drainage => Legend::Ramp { stops: FLOW.to_vec(), labels: lab(&[(0.0, "1"), (2.0, "100"), (4.0, "10k tiles")]) },
            Overlay::Stress => Legend::Ramp { stops: STRESS.to_vec(), labels: lab(&[(-0.6, "rifting"), (0.0, "calm"), (0.8, "collision")]) },
            Overlay::Plates => Legend::Note("One colour per plate; continental crust is lighter, oceanic darker."),
            Overlay::Biomes => Legend::Note("Biome classes as flat colours (hover a tile for its name)."),
        }
    }
}

// Palettes: muted, so the ink map's lines stay readable through them.
const HEIGHT: [(f32, [u8; 3]); 10] = [
    (-6000.0, [36, 62, 92]), (-2000.0, [62, 100, 128]), (-200.0, [112, 152, 168]), (-0.01, [160, 192, 198]), (0.0, [120, 150, 96]),
    (400.0, [160, 172, 104]), (1200.0, [196, 168, 108]), (2400.0, [160, 120, 92]), (3600.0, [176, 164, 158]),
    (4500.0, [236, 232, 226]),
];
const TEMP: [(f32, [u8; 3]); 7] = [
    (-35.0, [64, 76, 140]), (-15.0, [110, 140, 186]), (0.0, [214, 222, 216]), (10.0, [190, 204, 136]),
    (20.0, [222, 180, 96]), (28.0, [206, 120, 64]), (36.0, [150, 46, 38]),
];
const MOIST: [(f32, [u8; 3]); 6] = [
    (0.0, [196, 150, 88]), (0.12, [214, 192, 132]), (0.25, [168, 186, 116]), (0.4, [104, 156, 118]),
    (0.6, [62, 124, 140]), (0.9, [44, 82, 140]),
];
const FLOW: [(f32, [u8; 3]); 6] = [
    (0.0, [228, 218, 194]), (0.8, [214, 208, 188]), (1.4, [150, 176, 192]), (2.0, [74, 122, 176]), (3.0, [36, 78, 150]), (4.5, [20, 44, 110]),
];
const STRESS: [(f32, [u8; 3]); 6] = [
    (-0.8, [48, 92, 150]), (-0.2, [140, 170, 196]), (0.0, [226, 218, 198]), (0.15, [222, 186, 120]),
    (0.4, [196, 112, 64]), (1.0, [140, 36, 36]),
];

fn ramp(stops: &[(f32, [u8; 3])], v: f32) -> [f32; 3] {
    let to = |c: [u8; 3]| [c[0] as f32, c[1] as f32, c[2] as f32];
    if v <= stops[0].0 { return to(stops[0].1); }
    for w in stops.windows(2) {
        let ((a, ca), (b, cb)) = (w[0], w[1]);
        if v <= b {
            let t = (v - a) / (b - a).max(1e-6);
            return [0, 1, 2].map(|k| ca[k] as f32 + (cb[k] as f32 - ca[k] as f32) * t);
        }
    }
    to(stops[stops.len() - 1].1)
}

/// Colour at a legend position (for drawing the ramp).
pub fn ramp_color(stops: &[(f32, [u8; 3])], v: f32) -> [f32; 3] {
    ramp(stops, v)
}

fn hsv(h: f32, s: f32, v: f32) -> [f32; 3] {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0), 1 => (x, c, 0.0), 2 => (0.0, c, x), 3 => (0.0, x, c), 4 => (x, 0.0, c), _ => (c, 0.0, x),
    };
    [(r + m) * 255.0, (g + m) * 255.0, (b + m) * 255.0]
}

/// Per-tile colours for an overlay (empty for `Overlay::None`).
pub fn colors(world: &WorldData, mode: Overlay) -> Vec<[f32; 3]> {
    let (w, h) = (world.width, world.height);
    let mut out = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let e = *world.heightmap.get(x, y);
            let c = match mode {
                Overlay::None => return Vec::new(),
                Overlay::Height => ramp(&HEIGHT, e),
                Overlay::Temperature => ramp(&TEMP, *world.temperature.get(x, y)),
                Overlay::Moisture => ramp(&MOIST, *world.moisture.get(x, y)),
                Overlay::Drainage => {
                    if e < 0.0 {
                        [70, 100, 128].map(|v| v as f32)
                    } else {
                        let f = world.flow_accumulation.as_ref().map_or(1.0, |a| *a.get(x, y)).max(1.0);
                        ramp(&FLOW, f.log10())
                    }
                }
                Overlay::Stress => ramp(&STRESS, *world.stress_map.get(x, y)),
                Overlay::Plates => {
                    let id = world.plate_map.get(x, y).0 as u64;
                    // Golden-angle hues: consecutive plates land far apart on the colour wheel.
                    let hue = (id as f32 * 137.508) % 360.0;
                    let continental = world.plates.iter().find(|p| p.id.0 as u64 == id).map_or(e >= 0.0, |p| p.plate_type == crate::plates::PlateType::Continental);
                    hsv(hue, if continental { 0.32 } else { 0.45 }, if continental { 0.86 } else { 0.62 })
                }
                Overlay::Biomes => {
                    let (r, g, b) = world.biomes.get(x, y).color();
                    [r as f32, g as f32, b as f32]
                }
            };
            out.push(c);
        }
    }
    out
}

/// The value under a tile, for the hover line.
pub fn describe(world: &WorldData, mode: Overlay, x: usize, y: usize) -> String {
    match mode {
        Overlay::None => String::new(),
        Overlay::Height => format!("{:.0} m", world.heightmap.get(x, y)),
        Overlay::Temperature => format!("{:.1} C mean", world.temperature.get(x, y)),
        Overlay::Moisture => format!("moisture {:.2}", world.moisture.get(x, y)),
        Overlay::Drainage => format!("drains {:.0} tiles", world.flow_accumulation.as_ref().map_or(0.0, |a| *a.get(x, y))),
        Overlay::Plates => {
            let id = world.plate_map.get(x, y).0;
            let kind = world.plates.iter().find(|p| p.id.0 == id).map_or("?", |p| if p.plate_type == crate::plates::PlateType::Continental { "continental" } else { "oceanic" });
            format!("plate {} ({})", id, kind)
        }
        Overlay::Stress => format!("stress {:+.2}", world.stress_map.get(x, y)),
        Overlay::Biomes => format!("{:?}", world.biomes.get(x, y)),
    }
}

/// The legend: a parchment box in the bottom-left corner naming the overlay, with its colour
/// ramp and labelled values (or a note for category overlays).
pub fn draw_legend(buf: &mut [u32], w: usize, h: usize, mode: Overlay) {
    use super::text::{draw_ink, text_width};
    use super::ui::*;
    if mode == Overlay::None || w < 340 || h < 120 { return; }
    let r = Rect { x: 12, y: h - 84, w: 300, h: 72 };
    card(buf, w, r);
    let x = r.x + 14;
    draw_ink(buf, w, h, x as i64, r.y as i64 + 12, &mode.name().to_uppercase(), RUBRIC, 1, true);
    let hint = "O next";
    draw_ink(buf, w, h, (r.x + r.w - 14 - text_width(hint, 1)) as i64, r.y as i64 + 12, hint, INK_FADED, 1, false);
    match mode.legend() {
        Legend::Ramp { stops, labels } => {
            let bar = Rect { x, y: r.y + 28, w: r.w - 28, h: 12 };
            let (lo, hi) = (stops[0].0, stops[stops.len() - 1].0);
            for i in 0..bar.w {
                let v = lo + (hi - lo) * i as f32 / (bar.w - 1) as f32;
                let c = ramp_color(&stops, v);
                let px = ((c[0] as u32) << 16) | ((c[1] as u32) << 8) | c[2] as u32;
                for yy in bar.y..bar.y + bar.h { buf[yy * w + bar.x + i] = px; }
            }
            outline(buf, w, bar, INK);
            for (v, label) in labels {
                let px = bar.x + ((v - lo) / (hi - lo) * (bar.w - 1) as f32) as usize;
                for yy in bar.y + bar.h..bar.y + bar.h + 3 { buf[yy * w + px] = INK; }
                let tw = text_width(&label, 1);
                let lx = px.saturating_sub(tw / 2).clamp(bar.x, bar.x + bar.w - tw);
                draw_ink(buf, w, h, lx as i64, (bar.y + bar.h + 5) as i64, &label, INK, 1, false);
            }
        }
        Legend::Note(text) => {
            let mut y = r.y as i64 + 28;
            for line in wrap(text, (r.w - 28) / 7) {
                draw_ink(buf, w, h, x as i64, y, &line, INK, 1, false);
                y += 12;
            }
        }
    }
}
