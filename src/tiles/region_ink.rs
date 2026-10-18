//! The zoomed region (Z on the world map) in the map's ink, not the climate colours.
//!
//! `region.render_rgb()` is a hillshaded climate colour per cell; blown up by the walker it was
//! flat green squares with pixel rivers, the one view out of the ink style. Here each pixel is
//! drawn from the region's fields sampled smoothly between cell centres:
//! - land is a wash of the cell's colour pulled toward parchment, lit toward the north-west and
//!   hatched away from it (never darkened), hachures down the steep slopes, contours inked every
//!   100 m (a heavier line each 500 m)
//! - the site layer (`region::plan`): every landform has its ground (bare rock washed in its
//!   rock's colour, cliffs ticked along their faces, scree and sand stippled, pavements
//!   cross-hatched, dunes as crescents across the wind, salt crusts as polygons, bogs and marshes
//!   tufted, lava ridged, ice edged in blue, badlands rilled...), looked up through a warp so
//!   its borders are ragged, not the cell grid
//! - sea and lakes are flat water washes with paler shallows, edged by an ink bank on the smooth
//!   0.5 contour of the water field, with a second, fainter offshore line; rapids are barred,
//!   falls have their drop, sinks and resurgences their half ring, dry valleys a dashed stream
//! - rivers are the same water, from a coverage field of their channels, inked along both banks
//! - woods are small tree symbols on a jittered grid (broadleaf scalloped, firs pointed, palms,
//!   mangroves on stilts, dead trees bare), with an ink edge along the wood's boundary
//! - places (`draw_places`) get a cartographer's glyph per kind and a label in the lettering
//!   hierarchy, thinned by rank and zoom; `draw_legend` keys what is in view; `describe` words
//!   the cell under the walker for the hover
//! - what the lore paints (fields, roads, sites) shows through in its own colours, muted.
//!
//! `Ink::new` precomputes the fields once per region (~90 ms for 1024x1024 cells); `draw`
//! touches only arrays per pixel and runs row-parallel.
//!
//! `PLANET_INK_DEMO=1`: a region whose site layer is empty (every landform `None`, no places)
//! is given demo classes from its slope, height, wetness, rock and shore, and one place of
//! every kind on fitting ground (`demo`), so every symbol can be seen: the glyph gallery.

use rayon::prelude::*;

use super::render::ZoomCamera;
use crate::erosion::materials::RockType;
use crate::region::plan::{Family, Landform, Place, PlaceKind};
use crate::region::zoom::ZoomRegion;

const INK: [f32; 3] = [56.0, 42.0, 32.0];
const SEA_INK: [f32; 3] = [40.0, 66.0, 82.0];
const PAPER: [f32; 3] = [234.0, 222.0, 196.0];
const DEEP: [f32; 3] = [96.0, 128.0, 142.0];
const SEA: [f32; 3] = [118.0, 150.0, 158.0];
const SHALLOW: [f32; 3] = [156.0, 184.0, 178.0];
const LAKE: [f32; 3] = [126.0, 160.0, 166.0];
const SNOW: [f32; 3] = [240.0, 242.0, 244.0];
const ICE_INK: [f32; 3] = [118.0, 148.0, 178.0];
const SAND: [f32; 3] = [224.0, 206.0, 164.0];
const RUBRIC: [f32; 3] = [154.0, 42.0, 30.0];
const OFF_MAP: u32 = 0x00DC_CCA8;

/// A region's fields, ready for drawing.
pub struct Ink {
    w: usize,
    h: usize,
    /// Land wash per cell (lore paint and the landform's ground included); water cells hold
    /// their water colour.
    wash: Vec<[f32; 3]>,
    /// 1 where the cell is sea or lake, 0 on land.
    water: Vec<f32>,
    /// River channel coverage, 0..1.
    river: Vec<f32>,
    /// Elevation, metres.
    elev: Vec<f32>,
    /// The height's gradient (m per m) eastward and southward: the hillshade, the hachures, the
    /// cliff ticks and the water marks take their directions from it.
    gx: Vec<f32>,
    gy: Vec<f32>,
    /// 0 no wood, 1 broadleaf, 2 firs, 3 palms, 4 mangroves, 5 dead trees.
    wood: Vec<u8>,
    /// Wood coverage, smoothed: its 0.5 contour is the wood's inked edge.
    woodf: Vec<f32>,
    /// The lore painted this cell (fields, roads, sites).
    lore: Vec<bool>,
    /// Mean temperature (°C) and moisture, for the seasons.
    temp: Vec<f32>,
    moist: Vec<f32>,
    /// The site layer: the landform and the rock at the surface of every cell.
    land: Vec<Landform>,
    rock: Vec<RockType>,
    /// Coverage, smoothed, of the cliffs and of the ice: their edges are inked.
    cliff: Vec<f32>,
    ice: Vec<f32>,
    /// The places in this region, their positions in the region's own cells.
    places: Vec<Place>,
    /// The wind the dunes lie across (radians, screen).
    wind: f32,
    /// The classes and places are the gallery's (`PLANET_INK_DEMO`), not the site layer's.
    demo: bool,
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

/// Smooth value noise in 0..1 (one octave), for ragged borders.
fn vnoise(x: f32, y: f32, seed: u64) -> f32 {
    let (x0, y0) = (x.floor(), y.floor());
    let (tx, ty) = (x - x0, y - y0);
    let (xi, yi) = (x0 as i64, y0 as i64);
    let g = |dx: i64, dy: i64| (hash(xi + dx, yi + dy, seed) & 0xFFFF) as f32 / 65535.0;
    let (sx, sy) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
    let top = g(0, 0) + (g(1, 0) - g(0, 0)) * sx;
    let bot = g(0, 1) + (g(1, 1) - g(0, 1)) * sx;
    top + (bot - top) * sy
}

/// A 3x3 box blur of a 0/1 mask, `passes` times (the 0.5 contour stays where the mask's edge was).
fn smooth_mask(mut f: Vec<f32>, w: usize, h: usize, passes: usize) -> Vec<f32> {
    for _ in 0..passes {
        let src = f.clone();
        for y in 1..h.saturating_sub(1) {
            for x in 1..w.saturating_sub(1) {
                let mut sum = 0.0;
                for dy in 0..3 { for dx in 0..3 { sum += src[(y + dy - 1) * w + x + dx - 1]; } }
                f[y * w + x] = sum / 9.0;
            }
        }
    }
    f
}

/// The ground a landform shows, before the lore and the leaves: its wash, or none for plain ground.
fn ground_wash(l: Landform, rock: RockType, under: [f32; 3]) -> [f32; 3] {
    use Landform::*;
    let rock_w = || mix(super::local_ink::rock_wash(rock), PAPER, 0.3);
    match l {
        None => under,
        Cliff | Scree | Outcrop | Tor | SlotCanyon | Hoodoo | TowerKarst | KarstGorge | Arete | Stack | SeaCliff | Gorge | Erratic => rock_w(),
        Mesa => mix(rock_w(), under, 0.4),
        Badland => [198.0, 174.0, 140.0],
        Pavement => mix([214.0, 206.0, 178.0], PAPER, 0.25),
        Doline => mix(under, [186.0, 180.0, 150.0], 0.4),
        DryValley | Sink | Resurgence | Cenote => mix(under, [200.0, 194.0, 166.0], 0.3),
        UValley => mix(under, [196.0, 206.0, 176.0], 0.3),
        Cirque | Tarn | Kettle => mix(under, [186.0, 190.0, 176.0], 0.4),
        Moraine | Drumlin => mix(under, [188.0, 180.0, 160.0], 0.35),
        Outwash | Braided | Fan => mix(under, [210.0, 200.0, 176.0], 0.5),
        Glacier | Snowfield => SNOW,
        PatternedGround => mix(under, [204.0, 200.0, 190.0], 0.5),
        Crater | Caldera | CinderCone | AshPlain => [176.0, 166.0, 156.0],
        LavaField => [92.0, 76.0, 66.0],
        ColumnarBasalt => mix(super::local_ink::rock_wash(RockType::Basalt), PAPER, 0.25),
        Fumarole | ThermalGround => [226.0, 212.0, 160.0],
        Dune | SandSea | CoastalDune | Beach | Spit | Bar | BarrierIsland | RaisedBeach => SAND,
        DesertPavement => [204.0, 186.0, 150.0],
        Yardang => [214.0, 196.0, 156.0],
        Playa => [236.0, 230.0, 214.0],
        Floodplain | Delta => mix(under, [200.0, 208.0, 170.0], 0.4),
        Levee => mix(under, PAPER, 0.3),
        Backswamp | Marsh | Fen | ReedBed => mix(under, [168.0, 184.0, 146.0], 0.5),
        Oxbow | Terrace | Waterfall | Rapids | Estuary | Lagoon => under,
        Bog => mix(under, [178.0, 170.0, 130.0], 0.5),
        TidalFlat => [200.0, 204.0, 190.0],
        SaltMarsh => [188.0, 196.0, 172.0],
        Mangrove => [128.0, 146.0, 112.0],
        // Upheaval (ROADMAP Stage 7).
        ImpactCrater => mix(under, [170.0, 160.0, 150.0], 0.45),
        ImpactRim | CentralPeak | FaultScarp => rock_w(),
        Ejecta => mix(under, [196.0, 186.0, 166.0], 0.35),
        FaultTrough | SagPond => mix(under, [176.0, 186.0, 160.0], 0.35),
        PressureRidge => mix(under, [190.0, 180.0, 160.0], 0.3),
        LandslideScar => mix(rock_w(), [200.0, 170.0, 140.0], 0.3),
        LandslideDebris => mix(under, [190.0, 176.0, 156.0], 0.5),
        RockAvalanche => mix(under, [184.0, 172.0, 158.0], 0.7),
        BurnScar => mix(under, [150.0, 136.0, 120.0], 0.6),
        Maar => mix(under, [176.0, 166.0, 156.0], 0.4),
        FissureVents => [84.0, 70.0, 62.0],
        CollapseScar => mix(rock_w(), [150.0, 130.0, 120.0], 0.35),
        Lahar => [186.0, 178.0, 166.0],
        Traps => mix(super::local_ink::rock_wash(RockType::Basalt), under, 0.55),
        SaltSink => [230.0, 224.0, 208.0],
        ThawLake => mix(under, [180.0, 190.0, 184.0], 0.4),
        Crevasses => SNOW,
        AvalanchePath => mix(under, [214.0, 212.0, 200.0], 0.45),
        Scabland => mix(rock_w(), [200.0, 196.0, 180.0], 0.3),
        AbandonedChannel => mix(under, [170.0, 186.0, 150.0], 0.4),
    }
}

/// Ground that keeps the star's leaves (green things grow on it).
fn leafy_ground(l: Landform) -> bool {
    use Landform::*;
    matches!(l, None | UValley | Floodplain | Delta | Levee | Backswamp | Marsh | Fen | ReedBed | Bog | SaltMarsh | Mangrove
        | Moraine | Drumlin | Doline | DryValley | Terrace | Oxbow | PatternedGround | Cirque | Mesa
        | ImpactCrater | Ejecta | FaultTrough | PressureRidge | AbandonedChannel | ThawLake | Traps | Maar)
}

impl Ink {
    /// `painted` is the region's climate colours with the lore painted on (the walker's `rgb`);
    /// where it differs from the plain colours, the lore's colour shows.
    pub fn new(region: &ZoomRegion, painted: &[[u8; 3]]) -> Ink {
        let (w, h) = (region.width, region.height);
        let plain = region.render_rgb();
        let at = |x: usize, y: usize| region.elevation_m[y.min(h - 1) * w + x.min(w - 1)];
        let seed = region.params.seed;
        let s = (region.params.cells_per_tile.max(8) & !1) as i64;
        // The site layer, or the gallery's demo of it when asked for (the glyph gallery: every
        // kind of ground and place drawn once, whatever the land holds).
        let demo = std::env::var("PLANET_INK_DEMO").map_or(false, |v| v != "0");
        let (land, places): (Vec<Landform>, Vec<Place>) = if demo {
            demo::classify(region)
        } else {
            (region.landform.clone(), region.places.iter().map(|p| {
                let mut q = p.clone();
                q.x -= (region.world_x0 * s) as f64;
                q.y -= (region.world_y0 * s) as f64;
                q.cells = q.cells.iter().map(|&(x, y)| (x - region.world_x0 * s, y - region.world_y0 * s)).collect();
                q
            }).collect())
        };
        let rock = if region.rock.len() == w * h { region.rock.clone() } else { vec![RockType::Sandstone; w * h] };
        let mut wash = vec![[0.0f32; 3]; w * h];
        let mut water = vec![0.0f32; w * h];
        let mut gx = vec![0.0f32; w * h];
        let mut gy = vec![0.0f32; w * h];
        let mut wood = vec![0u8; w * h];
        let mut lore = vec![false; w * h];
        let mut cliff = vec![0.0f32; w * h];
        let mut ice = vec![0.0f32; w * h];
        let mut woodf = vec![0.0f32; w * h];
        for y in 0..h {
            for x in 0..w {
                let k = y * w + x;
                let e = region.elevation_m[k];
                let l = land[k];
                if e <= 0.0 {
                    water[k] = 1.0;
                    let t = (-e / 600.0).clamp(0.0, 1.0);
                    wash[k] = if t < 0.25 { mix(SHALLOW, SEA, t / 0.25) } else { mix(SEA, DEEP, (t - 0.25) / 0.75) };
                    if l == Landform::Lagoon { wash[k] = mix(wash[k], SHALLOW, 0.5); }
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
                    wash[k] = ground_wash(l, rock[k], muted);
                }
                gx[k] = (at(x + 1, y) - at(x.saturating_sub(1), y)) / (2.0 * region.cell_m);
                gy[k] = (at(x, y + 1) - at(x, y.saturating_sub(1))) / (2.0 * region.cell_m);
                let (t, m) = (region.temperature_c[k], region.moisture[k]);
                // Woods by the climate; the landform says which trees, and where none grow.
                if !lore[k] && m > 0.42 && t > -4.0 && e < 3200.0 && !l.is_bare_rock() && !matches!(l, Landform::Glacier | Landform::Snowfield | Landform::Bog | Landform::Playa | Landform::Dune | Landform::SandSea | Landform::TidalFlat | Landform::SaltMarsh | Landform::Beach
                    | Landform::Crevasses | Landform::AvalanchePath | Landform::LandslideDebris | Landform::SaltSink | Landform::Lahar) {
                    wood[k] = if matches!(l, Landform::AshPlain | Landform::LavaField | Landform::BurnScar) { 5 } else if t < 7.0 { 2 } else if t > 22.0 && m > 0.5 { 3 } else { 1 };
                }
                if l == Landform::Mangrove && !lore[k] { wood[k] = 4; }
                if wood[k] > 0 { woodf[k] = 1.0; }
                if matches!(l, Landform::Cliff | Landform::SeaCliff | Landform::FaultScarp | Landform::CollapseScar) { cliff[k] = 1.0; }
                if matches!(l, Landform::Glacier | Landform::Snowfield | Landform::Crevasses) { ice[k] = 1.0; }
                // Green ground (grass, woods' floor) takes the star's leaves, by how green it is.
                if e > 0.0 && t > -4.0 && m > 0.25 && leafy_ground(l) {
                    let g = super::ink::leafy(wash[k]);
                    wash[k] = mix(wash[k], g, ((m - 0.25) * 2.0).min(super::ink::GRASS_LEAVES));
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
        let cliff = smooth_mask(cliff, w, h, 1);
        let ice = smooth_mask(ice, w, h, 2);
        let woodf = smooth_mask(woodf, w, h, 2);
        let wind = (hash(region.world_x0, region.world_y0, seed ^ 0x1D) % 360) as f32 * std::f32::consts::PI / 180.0;
        Ink { w, h, wash, water, river, elev: region.elevation_m.clone(), gx, gy, wood, woodf, lore, temp: region.temperature_c.clone(), moist: region.moisture.clone(),
            land, rock, cliff, ice, places, wind, demo, seed,
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

    /// The landform at (x, y), looked up through a warp where landforms meet, so their borders
    /// are ragged rather than the cell grid.
    #[inline]
    fn land_at(&self, x: f32, y: f32) -> Landform {
        let k = self.cell(x, y);
        let l = self.land[k];
        let (xi, yi) = (x.max(0.0) as i64, y.max(0.0) as i64);
        let same = |dx: i64, dy: i64| {
            let (px, py) = ((xi + dx).clamp(0, self.w as i64 - 1) as usize, (yi + dy).clamp(0, self.h as i64 - 1) as usize);
            self.land[py * self.w + px] == l
        };
        if same(1, 0) && same(-1, 0) && same(0, 1) && same(0, -1) { return l; }
        let wx = x + 1.2 * (vnoise(x * 0.9, y * 0.9, self.seed ^ 0x51) - 0.5);
        let wy = y + 1.2 * (vnoise(x * 0.9 + 17.0, y * 0.9, self.seed ^ 0x52) - 0.5);
        self.land[self.cell(wx, wy)]
    }

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

    /// The landform, rock and slope at region cell (x, y), as words for the hover:
    /// "limestone pavement, on limestone, slope 12°".
    pub fn describe(&self, x: usize, y: usize) -> String {
        let k = y.min(self.h - 1) * self.w + x.min(self.w - 1);
        let slope = (self.gx[k].powi(2) + self.gy[k].powi(2)).sqrt().atan().to_degrees();
        let rock = format!("{:?}", self.rock[k]).to_lowercase();
        if self.land[k] == Landform::None { format!("{rock}, slope {slope:.0}°") }
        else {
            let warn = if self.land[k].is_dangerous() { " - dangerous ground" } else { "" };
            format!("{}, on {rock}, slope {slope:.0}°{warn}", self.land[k].name())
        }
    }

    /// The nearest place within `within` cells of region cell (x, y).
    pub fn nearest_place(&self, x: f64, y: f64, within: f64) -> Option<&Place> {
        self.places.iter().map(|p| (((p.x - x).powi(2) + (p.y - y).powi(2)).sqrt(), p))
            .filter(|(d, _)| *d <= within).min_by(|a, b| a.0.partial_cmp(&b.0).unwrap()).map(|(_, p)| p)
    }

    pub fn places(&self) -> &[Place] { &self.places }
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

/// Perpendicular distance from (u, v) to the segment a-b, in the same units.
fn seg_dist(u: f32, v: f32, a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = (dx * dx + dy * dy).max(1e-6);
    let t = (((u - a.0) * dx + (v - a.1) * dy) / len2).clamp(0.0, 1.0);
    ((u - a.0 - t * dx).powi(2) + (v - a.1 - t * dy).powi(2)).sqrt()
}

/// The ground's own marks for a landform at region pixel (lx, ly): stipple, ticks, crescents,
/// tufts, crusts... `d` is the downhill direction (unit), `slope` m per m. Returns the colour.
#[allow(clippy::too_many_arguments)]
fn ground_marks(ink: &Ink, l: Landform, rx: f32, ry: f32, lx: f32, ly: f32, ppc: f32, d: (f32, f32), slope: f32, mut c: [f32; 3]) -> [f32; 3] {
    use Landform::*;
    let seed = ink.seed;
    let (px, py) = (lx.floor() as i64, ly.floor() as i64);
    let hp = |salt: u64| hash(px, py, seed ^ salt);
    // Along (a) and down (b) the slope, in pixels.
    let t = (-d.1, d.0);
    let a = lx * t.0 + ly * t.1;
    let b = lx * d.0 + ly * d.1;
    // Along and across the wind, for the dunes.
    let (wc, ws) = (ink.wind.cos(), ink.wind.sin());
    let (along_wind, across_wind) = (lx * wc + ly * ws, -lx * ws + ly * wc);
    match l {
        Cliff | SeaCliff => {
            // Ticks down the face, every 5 px along it.
            if a.rem_euclid(5.0) < 1.2 { c = mix(c, INK, 0.75); }
        }
        Scree => { if hp(0x5C) % 7 == 0 { c = mix(c, INK, 0.45); } }
        Outcrop | Tor | Stack | Hoodoo | TowerKarst | Arete | ImpactRim | CentralPeak => {
            // Crags: small inverted strokes on a jittered grid.
            let sp = 7.0;
            let (gx, gy) = ((lx / sp).floor() as i64, (ly / sp).floor() as i64);
            let hs = hash(gx, gy, seed ^ 0xC4A6);
            if hs % 3 != 0 {
                let (cx, cy) = ((gx as f32 + 0.3 + 0.4 * ((hs >> 8) & 255) as f32 / 255.0) * sp, (gy as f32 + 0.3 + 0.4 * ((hs >> 16) & 255) as f32 / 255.0) * sp);
                let (u, v) = (lx - cx, ly - cy);
                let crag = (v + 1.2 - u.abs() * 1.1).abs() < 0.55 && v > -1.4 && v < 1.6;
                if crag { c = mix(c, INK, 0.7); }
            }
            if matches!(l, Tor | Hoodoo) && hp(0xA7) % 11 == 0 { c = mix(c, INK, 0.3); }
        }
        ColumnarBasalt => { if lx.rem_euclid(3.0) < 0.9 { c = mix(c, INK, 0.35); } }
        Badland => { if a.rem_euclid(3.0) < 0.8 { c = mix(c, INK, 0.3); } }
        SlotCanyon | Gorge | KarstGorge => { if (lx - ly).rem_euclid(3.0) < 1.0 { c = mix(c, INK, 0.38); } }
        Mesa => { if slope > 0.3 && a.rem_euclid(4.0) < 1.0 { c = mix(c, INK, 0.5); } }
        Pavement => {
            // Clints and grikes: a cross-hatch with gaps.
            let (gx, gy) = ((lx / 6.0).floor() as i64, (ly / 6.0).floor() as i64);
            let gap = hash(gx, gy, seed ^ 0x9A) % 3 == 0;
            if !gap && (lx.rem_euclid(6.0) < 0.9 || ly.rem_euclid(6.0) < 0.9) { c = mix(c, INK, 0.4); }
        }
        Doline | Kettle | Crater | Caldera | Cenote | Sink | ImpactCrater | Maar | SaltSink | ThawLake | SagPond => {
            // A dotted ring where the hollow's edge is (a 4-neighbour cell that isn't one).
            let k = ink.cell(rx, ry);
            let (xi, yi) = (k % ink.w, k / ink.w);
            let other = |dx: i64, dy: i64| ink.land[ink.cell((xi as i64 + dx) as f32 + 0.5, (yi as i64 + dy) as f32 + 0.5)] != l;
            if (other(1, 0) || other(-1, 0) || other(0, 1) || other(0, -1)) && (px + py).rem_euclid(3) == 0 { c = mix(c, INK, 0.55); }
            if l == Sink && ppc >= 2.0 {
                let (cx, cy) = ((xi as f32 + 0.5) * ppc, (yi as f32 + 0.5) * ppc);
                let r = ((lx - cx).powi(2) + (ly - cy).powi(2)).sqrt();
                if (r - 3.5).abs() < 0.8 && ly > cy { c = mix(c, INK, 0.85); }
            }
        }
        Resurgence => {
            if ppc >= 2.0 {
                let k = ink.cell(rx, ry);
                let (xi, yi) = (k % ink.w, k / ink.w);
                let (cx, cy) = ((xi as f32 + 0.5) * ppc, (yi as f32 + 0.5) * ppc);
                let r = ((lx - cx).powi(2) + (ly - cy).powi(2)).sqrt();
                if (r - 3.5).abs() < 0.8 && ly < cy { c = mix(c, SEA_INK, 0.85); }
            }
        }
        DryValley => {
            // A dashed stream down the valley.
            if a.rem_euclid(ppc.max(4.0)) < 1.2 && b.rem_euclid(8.0) < 4.5 { c = mix(c, SEA_INK, 0.5); }
        }
        Glacier => {
            // Crevasses across the flow, here and there.
            let (gx, gy) = ((a / 9.0).floor() as i64, (b / 9.0).floor() as i64);
            if b.rem_euclid(9.0) < 1.0 && hash(gx, gy, seed ^ 0x1CE) % 3 != 0 { c = mix(c, ICE_INK, 0.5); }
        }
        Snowfield => { if hp(0x5F) % 23 == 0 { c = mix(c, ICE_INK, 0.25); } }
        PatternedGround => {
            if worley_edge(lx, ly, 9.0, seed ^ 0xF0) { c = mix(c, [150.0, 160.0, 170.0], 0.4); }
        }
        LavaField => {
            // Pressure ridges: wavy lines across the flow.
            if (ly + 2.0 * (lx / 6.0).sin()).rem_euclid(9.0) < 1.2 { c = mix(c, [40.0, 34.0, 30.0], 0.55); }
        }
        CinderCone | AshPlain => { if hp(0xA5) % 9 == 0 { c = mix(c, INK, 0.3); } }
        Fumarole | ThermalGround => {
            // Puffs: small arcs on a grid, a dark vent under each fumarole's.
            let sp = 9.0;
            let (gx, gy) = ((lx / sp).floor() as i64, (ly / sp).floor() as i64);
            let hs = hash(gx, gy, seed ^ 0xF4);
            if hs % 2 == 0 {
                let (cx, cy) = ((gx as f32 + 0.5) * sp, (gy as f32 + 0.6) * sp);
                let (u, v) = (lx - cx, ly - cy);
                let r = (u * u + v * v).sqrt();
                if (r - 2.6).abs() < 0.7 && v < 0.0 { c = mix(c, INK, 0.5); }
                if l == Fumarole && r < 1.0 { c = mix(c, INK, 0.8); }
            }
        }
        Dune | SandSea | CoastalDune => {
            // Barchans: crescents whose horns point downwind, on a grid along the wind.
            let sp = if l == SandSea { 14.0 } else if l == CoastalDune { 8.0 } else { 10.0 };
            let (gx, gy) = ((along_wind / sp).floor() as i64, (across_wind / sp).floor() as i64);
            let hs = hash(gx, gy, seed ^ 0xD0E);
            if hs % 4 != 0 {
                let (cx, cy) = ((gx as f32 + 0.5) * sp + (((hs >> 8) & 255) as f32 / 255.0 - 0.5) * 3.0, (gy as f32 + 0.5) * sp + (((hs >> 16) & 255) as f32 / 255.0 - 0.5) * 3.0);
                let (u, v) = (along_wind - cx, across_wind - cy);
                let r = (u * u + v * v).sqrt();
                let rr = sp * 0.33;
                if (r - rr).abs() < 0.8 && u < 0.35 * rr { c = mix(c, INK, 0.5); }
            }
            if hp(0x5A) % 17 == 0 { c = mix(c, INK, 0.25); }
        }
        Yardang => { if across_wind.rem_euclid(4.0) < 0.9 { c = mix(c, INK, 0.3); } }
        DesertPavement => { if hp(0xDE) % 6 == 0 { c = mix(c, INK, 0.3); } }
        Playa => { if worley_edge(lx, ly, 12.0, seed ^ 0x9A1) { c = mix(c, INK, 0.4); } }
        Beach | Spit | Bar | BarrierIsland | RaisedBeach => { if hp(0xBE) % 13 == 0 { c = mix(c, INK, 0.3); } }
        Outwash | Braided | Fan => { if hp(0x0F) % 11 == 0 { c = mix(c, INK, 0.3); } }
        Moraine | Drumlin => { if hp(0x30) % 17 == 0 { c = mix(c, INK, 0.5); } }
        Erratic => {
            // Boulders: small outlined ellipses on a jittered grid.
            if boulder(lx, ly, 6.0, seed ^ 0xB0) { c = mix(c, INK, 0.75); }
        }
        TidalFlat | Estuary => { if px.rem_euclid(4) == 0 && py.rem_euclid(4) == 0 { c = mix(c, SEA_INK, 0.5); } }
        Bog => { if tuft(lx, ly, 7.0, seed ^ 0xB06, 2) { c = mix(c, INK, 0.55); } }
        Marsh | Backswamp | Fen | SaltMarsh => { if tuft(lx, ly, 7.0, seed ^ 0x3A5, 3) { c = mix(c, if l == SaltMarsh { SEA_INK } else { INK }, 0.5); } }
        ReedBed => { if tuft(lx, ly, 5.0, seed ^ 0x2EE, 5) { c = mix(c, [70.0, 92.0, 56.0], 0.6); } }
        Levee => { if slope < 0.02 && hp(0x1E) % 19 == 0 { c = mix(c, PAPER, 0.4); } }
        FaultScarp | CollapseScar => {
            // Heavy ticks down the face, close set: a fresh step.
            if a.rem_euclid(4.0) < 1.4 { c = mix(c, INK, 0.85); }
        }
        LandslideScar => { if b.rem_euclid(4.0) < 1.0 && a.rem_euclid(7.0) < 4.5 { c = mix(c, INK, 0.6); } }
        LandslideDebris | RockAvalanche | Ejecta | Scabland => {
            let sp = if l == RockAvalanche { 5.0 } else if l == Ejecta { 9.0 } else { 6.5 };
            if boulder(lx, ly, sp, seed ^ 0x51D) { c = mix(c, INK, 0.7); }
            if l == Scabland && b.rem_euclid(6.0) < 0.9 { c = mix(c, SEA_INK, 0.3); }
        }
        FaultTrough | AbandonedChannel => {
            if a.rem_euclid(ppc.max(4.0)) < 1.2 && b.rem_euclid(8.0) < 4.5 { c = mix(c, SEA_INK, 0.45); }
        }
        PressureRidge => { if (ly + 2.0 * (lx / 5.0).sin()).rem_euclid(7.0) < 1.0 { c = mix(c, INK, 0.45); } }
        BurnScar => { if hp(0xB5) % 7 == 0 { c = mix(c, [60.0, 50.0, 44.0], 0.6); } }
        FissureVents => { if (lx * 0.3 + ly).rem_euclid(7.0) < 1.1 { c = mix(c, [30.0, 22.0, 20.0], 0.7); } }
        Lahar => { if b.rem_euclid(5.0) < 0.8 { c = mix(c, INK, 0.25); } }
        Traps => { if slope > 0.05 && a.rem_euclid(3.0) < 0.8 { c = mix(c, INK, 0.45); } }
        Crevasses => {
            let (gx, gy) = ((a / 5.0).floor() as i64, (b / 5.0).floor() as i64);
            if b.rem_euclid(5.0) < 1.0 && hash(gx, gy, seed ^ 0x1CF) % 4 != 0 { c = mix(c, ICE_INK, 0.75); }
        }
        AvalanchePath => { if a.rem_euclid(6.0) < 1.0 { c = mix(c, ICE_INK, 0.35); } }
        Floodplain | Delta | UValley | Cirque | Terrace | Oxbow | Tarn | Waterfall | Rapids | Lagoon | Mangrove | None => {}
    }
    c
}

/// A tuft mark: short strokes rising from a base, on a jittered grid of `sp` px (`reed` strokes high).
fn tuft(lx: f32, ly: f32, sp: f32, seed: u64, high: i32) -> bool {
    let (gx, gy) = ((lx / sp).floor() as i64, (ly / sp).floor() as i64);
    let hs = hash(gx, gy, seed);
    if hs % 3 == 0 { return false; }
    let (cx, cy) = ((gx as f32 + 0.25 + 0.5 * ((hs >> 8) & 255) as f32 / 255.0) * sp, (gy as f32 + 0.25 + 0.5 * ((hs >> 16) & 255) as f32 / 255.0) * sp);
    let (u, v) = ((lx - cx).floor() as i32, (ly - cy).floor() as i32);
    let base = v == 0 && u.abs() <= 2;
    let strokes = (u == -2 || u == 0 || u == 2) && v <= 0 && v >= -high;
    base || strokes
}

/// A boulder outline on a jittered grid of `sp` px.
fn boulder(lx: f32, ly: f32, sp: f32, seed: u64) -> bool {
    let (gx, gy) = ((lx / sp).floor() as i64, (ly / sp).floor() as i64);
    let hs = hash(gx, gy, seed);
    if hs % 2 == 0 { return false; }
    let (cx, cy) = ((gx as f32 + 0.3 + 0.4 * ((hs >> 8) & 255) as f32 / 255.0) * sp, (gy as f32 + 0.3 + 0.4 * ((hs >> 16) & 255) as f32 / 255.0) * sp);
    let (u, v) = ((lx - cx) / 2.2, (ly - cy) / 1.5);
    let r = u * u + v * v;
    r < 1.0 && r > 0.45
}

/// On the edge between cells of a jittered grid of `sp` px (a salt crust, patterned ground).
fn worley_edge(lx: f32, ly: f32, sp: f32, seed: u64) -> bool {
    let (gx, gy) = ((lx / sp).floor() as i64, (ly / sp).floor() as i64);
    let (mut d1, mut d2) = (f32::MAX, f32::MAX);
    for oy in -1..=1 {
        for ox in -1..=1 {
            let hs = hash(gx + ox, gy + oy, seed);
            let fx = ((gx + ox) as f32 + 0.15 + 0.7 * ((hs >> 8) & 255) as f32 / 255.0) * sp;
            let fy = ((gy + oy) as f32 + 0.15 + 0.7 * ((hs >> 16) & 255) as f32 / 255.0) * sp;
            let d = ((lx - fx).powi(2) + (ly - fy).powi(2)).sqrt();
            if d < d1 { d2 = d1; d1 = d; } else if d < d2 { d2 = d; }
        }
    }
    d2 - d1 < 1.3
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
    let marks = ppc >= 1.0;
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
            let (lx, ly) = (rx * ppc, ry * ppc);
            // The slope under the pixel: its direction and steepness.
            let gxv = ink.bilinear(&ink.gx, rx, ry);
            let gyv = ink.bilinear(&ink.gy, rx, ry);
            let slope = (gxv * gxv + gyv * gyv).sqrt();
            let d = if slope > 1e-4 { (-gxv / slope, -gyv / slope) } else { (0.0, 1.0) };
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
                    let (gx, gy) = ((lx / 22.0).floor() as i64, (ly / 14.0).floor() as i64);
                    let hs = hash(gx, gy, ink.seed);
                    if hs % 3 == 0 {
                        let (cx, cy) = ((gx as f32 + 0.3 + 0.4 * ((hs >> 8) & 255) as f32 / 255.0) * 22.0, (gy as f32 + 0.5) * 14.0);
                        let (px, py) = (lx - cx, ly - cy);
                        if px.abs() < 4.0 && (py + 0.12 * px * px - 1.0).abs() < 0.6 { c = mix(c, SEA_INK, 0.45); }
                    }
                }
                // Rapids barred across the channel; a fall's bar and its drop.
                if marks && rv > wv {
                    let l = ink.land[k];
                    let t = (-d.1, d.0);
                    let b = lx * d.0 + ly * d.1;
                    if l == Landform::Rapids && b.rem_euclid(4.0) < 1.1 && rv > 0.6 { c = mix(c, SEA_INK, 0.6); }
                    if l == Landform::Waterfall {
                        let (xi, yi) = (k % ink.w, k / ink.w);
                        let (cx, cy) = ((xi as f32 + 0.5) * ppc, (yi as f32 + 0.5) * ppc);
                        let bc = cx * d.0 + cy * d.1;
                        let a = lx * t.0 + ly * t.1;
                        if (b - bc).abs() < 1.0 { c = mix(c, SEA_INK, 0.9); }
                        else if b > bc && b - bc < 6.0 && a.rem_euclid(3.0) < 1.0 { c = mix(c, SEA_INK, 0.55); }
                    }
                }
                // Ice on lakes and rivers in a hard season (the sea keeps open water).
                let t_w = ink.bilinear(&ink.temp, rx, ry) + shift;
                if t_w < -3.0 && (rv > wv || ink.elev[k] > 0.0) {
                    c = mix([214.0, 224.0, 226.0], c, 0.25);
                    if ((lx + ly * 0.4) as i64).rem_euclid(23) == 0 { c = mix(c, [130.0, 154.0, 166.0], 0.5); }
                }
                // Offshore line ~4 px out from the bank.
                if wv > 0.5 && (edge_px - 4.0).abs() < 0.5 && ppc >= 1.0 { c = mix(c, SEA_INK, 0.28); }
            } else {
                c = ink.wash_at(rx, ry);
                let l = if marks { ink.land_at(rx, ry) } else { ink.land[k] };
                // Mottling: the wash isn't flat.
                let n = (hash((rx * 0.7) as i64, (ry * 0.7) as i64, ink.seed ^ 3) & 255) as f32 / 255.0;
                c = mix(c, PAPER, 0.06 * n);
                // Light from the top left: the lit side pale, the far side hatched with
                // diagonal strokes locked to the land, closer where the slope is steep.
                let away = (gxv + gyv) * 0.707;
                if away < -0.015 { c = mix(c, [248.0, 242.0, 228.0], (-away * 4.0).min(0.3)); }
                let s = (away * 6.0).clamp(0.0, 1.0);
                if s > 0.12 {
                    let sp = if slope > 0.25 { 3.0 } else { 4.0 };
                    if (lx - ly).rem_euclid(sp) < 1.0 { c = mix(c, INK, (s * 0.5).min(0.36)); }
                }
                // Hachures down the steep slopes: strokes along the fall line, 5 px apart.
                if slope > 0.16 && !l.is_bare_rock() {
                    let a = lx * (-d.1) + ly * d.0;
                    if a.rem_euclid(5.0) < 1.0 { c = mix(c, INK, ((slope - 0.16) * 1.4).min(0.3)); }
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
                // The landform's own marks.
                if marks { c = ground_marks(ink, l, rx, ry, lx, ly, ppc, d, slope, c); }
                // The cliffs' upper edge, and the ice's edge in blue.
                if marks {
                    let cv = ink.bilinear(&ink.cliff, rx, ry);
                    if cv > 0.08 && cv < 0.92 {
                        let cx = ink.bilinear(&ink.cliff, rx + inv, ry);
                        let cy = ink.bilinear(&ink.cliff, rx, ry + inv);
                        let g = ((cx - cv).powi(2) + (cy - cv).powi(2)).sqrt().max(1e-4);
                        let e = (cv - 0.5).abs() / g;
                        // The edge on the uphill side of the face (where the face begins).
                        let uphill = (cx - cv) * d.0 + (cy - cv) * d.1 < 0.0;
                        if e < 1.0 && uphill { c = mix(c, INK, (1.0 - e) * 0.8); }
                    }
                    let iv = ink.bilinear(&ink.ice, rx, ry);
                    if iv > 0.08 && iv < 0.92 {
                        let ix = ink.bilinear(&ink.ice, rx + inv, ry);
                        let iy = ink.bilinear(&ink.ice, rx, ry + inv);
                        let g = ((ix - iv).powi(2) + (iy - iv).powi(2)).sqrt().max(1e-4);
                        let e = (iv - 0.5).abs() / g;
                        if e < 1.0 { c = mix(c, ICE_INK, (1.0 - e) * 0.7); }
                    }
                }
                // The season's snow: where the season's cold and the moisture allow, the ground
                // whitens (more the colder), hatching and contours still showing through.
                let t_here = ink.bilinear(&ink.temp, rx, ry) + shift;
                let snow = if t_here < 0.0 { ((-t_here) / 6.0).clamp(0.0, 1.0) * (ink.bilinear(&ink.moist, rx, ry) * 2.5).clamp(0.3, 1.0) } else { 0.0 };
                if snow > 0.0 { c = mix(c, SNOW, 0.75 * snow); }
                // Woods: an ink edge along the wood's boundary, then the trees.
                if trees && !ink.lore[k] {
                    let wf = ink.bilinear(&ink.woodf, rx, ry);
                    if wf > 0.1 && wf < 0.9 {
                        let wx = ink.bilinear(&ink.woodf, rx + inv, ry);
                        let wy = ink.bilinear(&ink.woodf, rx, ry + inv);
                        let g = ((wx - wf).powi(2) + (wy - wf).powi(2)).sqrt().max(1e-4);
                        let e = (wf - 0.5).abs() / g;
                        if e < 0.8 { c = mix(c, INK, (1.0 - e / 0.8) * 0.3); }
                    }
                    near.clear();
                    ink.trees_near(rx, ry, ppc, &mut near);
                    let rim_w = 1.1 / (sp_px(ppc) * 0.42);
                    let mut drawn = false;
                    for &(u, v2, kind, _) in near.iter() {
                        let (inside, rim) = match kind {
                            1 => {
                                // Scalloped crown over a short trunk.
                                let r = (u * u + (v2 + 0.15).powi(2)).sqrt();
                                let wob = 0.88 + 0.1 * ((u.atan2(v2 + 0.15)) * 5.0).cos();
                                let crown = r < wob;
                                let trunk = u.abs() < 0.12 && v2 > 0.6 && v2 < 1.1;
                                (crown || trunk, (crown && r > wob - rim_w) || (trunk && u.abs() > 0.05))
                            }
                            2 => {
                                // A fir: a tall triangle.
                                let half = (1.0 + v2) * 0.42;
                                let tri = v2 > -1.2 && v2 < 0.9 && u.abs() < half;
                                (tri, tri && (half - u.abs() < rim_w || v2 > 0.9 - rim_w))
                            }
                            3 => {
                                // A palm: a leaning trunk and four fronds from its top.
                                let trunk = seg_dist(u, v2, (0.05, 1.1), (0.0, -0.3)) < 0.1;
                                let top = (0.0, -0.3);
                                let fronds = [(-0.9, -0.1), (-0.5, -0.95), (0.5, -0.95), (0.9, -0.1)];
                                let frond = fronds.iter().any(|&f| seg_dist(u, v2, top, f) < 0.14);
                                (trunk || frond, trunk && u.abs() > 0.02 || frond)
                            }
                            4 => {
                                // A mangrove: a small crown on stilt roots.
                                let r = (u * u + (v2 + 0.35).powi(2)).sqrt();
                                let crown = r < 0.6;
                                let roots = [(-0.55, 1.05), (0.0, 1.1), (0.55, 1.05)].iter().any(|&f| seg_dist(u, v2, (0.0, 0.2), f) < 0.08);
                                (crown || roots, (crown && r > 0.6 - rim_w) || roots)
                            }
                            _ => {
                                // A dead tree: a bare forked trunk.
                                let trunk = seg_dist(u, v2, (0.0, 1.1), (0.0, -0.7)) < 0.1;
                                let branch = seg_dist(u, v2, (0.0, -0.2), (-0.6, -0.9)) < 0.08 || seg_dist(u, v2, (0.0, 0.2), (0.5, -0.4)) < 0.08;
                                (trunk || branch, true)
                            }
                        };
                        if inside {
                            let lit = u + v2 < 0.0;
                            let mut fill = match kind { 1 => [150.0, 164.0, 116.0], 2 => [124.0, 146.0, 112.0], 3 => [138.0, 160.0, 104.0], 4 => [110.0, 136.0, 100.0], _ => [120.0, 104.0, 88.0] };
                            // Autumn turns the broadleaves; winter bares them grey-brown, firs keep green.
                            if kind == 1 && autumn { fill = [184.0, 142.0, 92.0]; }
                            if kind == 1 && snow > 0.3 { fill = [150.0, 132.0, 112.0]; }
                            let fill = if kind == 5 { fill } else { super::ink::leafy(fill) };
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

// ---------------------------------------------------------------------------------------------
// Places: glyphs, labels, the key
// ---------------------------------------------------------------------------------------------

/// The colour of a glyph's ink, by what the place is.
fn glyph_ink(kind: PlaceKind) -> [f32; 3] {
    use PlaceKind::*;
    match kind {
        Spring | Waterfall | Oxbow | Tarn | Blowhole | CraterLake | Crannog | Ford | DrownedVillage | SaltSpring => SEA_INK,
        LandslideLake | SagPond | Maar => SEA_INK,
        Battlefield | Lair | PaintedCave | GasHollow | Quicksand | Sinkhole | CrevasseField | TarPit | CrownHoles => RUBRIC,
        _ => INK,
    }
}

/// How a place is lettered: natural places in italic, the works of the ages in small capitals,
/// the greatest larger.
fn label_style(p: &Place) -> (super::fonts::Face, f32, f32, u32) {
    use super::fonts::Face::*;
    let great = p.rank < 40;
    match p.kind.age() {
        crate::region::plan::Age::Natural => (Italic, if great { 17.0 } else { 13.0 }, if great { 1.0 } else { 0.3 }, 0x0036_2A1E),
        crate::region::plan::Age::Chronicle => (Italic, 13.0, 0.3, 0x0052_3C2C),
        _ => (SmallCaps, if great { 16.0 } else { 12.5 }, if great { 1.5 } else { 0.6 }, 0x0036_2A1E),
    }
}

/// The zoom (px a cell) a place's label and glyph first show at, by its rank.
fn min_ppc(p: &Place) -> f32 {
    match p.rank { 0..=40 => 0.5, 41..=120 => 2.0, 121..=200 => 4.0, _ => 8.0 }
}

/// A cartographer's glyph for `kind`, `size` px across, centred on (cx, cy) in screen pixels.
#[allow(clippy::too_many_arguments)]
fn draw_glyph(buf: &mut [u32], w: usize, h: usize, kind: PlaceKind, cx: f32, cy: f32, size: f32, seed: u64) {
    use PlaceKind::*;
    let ink = glyph_ink(kind);
    let r = size / 2.0;
    let mut put = |x: i64, y: i64, c: [f32; 3], a: f32| {
        if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 { return; }
        let k = y as usize * w + x as usize;
        let p = buf[k];
        let old = [((p >> 16) & 255) as f32, ((p >> 8) & 255) as f32, (p & 255) as f32];
        let m = mix(old, c, a.clamp(0.0, 1.0));
        buf[k] = ((m[0] as u32) << 16) | ((m[1] as u32) << 8) | m[2] as u32;
    };
    // Shapes in a unit box (u right, v down, -1..1) scanned over the glyph's pixels.
    let (x0, x1) = ((cx - r - 2.0).floor() as i64, (cx + r + 2.0).ceil() as i64);
    let (y0, y1) = ((cy - r - 2.0).floor() as i64, (cy + r + 2.0).ceil() as i64);
    let e = 1.2 / r; // one pixel, in unit box terms
    let ring = |u: f32, v: f32, ox: f32, oy: f32, rad: f32, th: f32| (((u - ox).powi(2) + (v - oy).powi(2)).sqrt() - rad).abs() < th;
    let disc = |u: f32, v: f32, ox: f32, oy: f32, rad: f32| (u - ox).powi(2) + (v - oy).powi(2) < rad * rad;
    let seg = |u: f32, v: f32, a: (f32, f32), b: (f32, f32), th: f32| seg_dist(u, v, a, b) < th;
    let hs = hash(cx as i64, cy as i64, seed);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let (u, v) = ((x as f32 + 0.5 - cx) / r, (y as f32 + 0.5 - cy) / r);
            // (fill colour, alpha) of this pixel, or nothing.
            let px: Option<([f32; 3], f32)> = match kind {
                CaveMouth | Lair | PaintedCave => {
                    let arch = disc(u, v, 0.0, 0.2, 0.75) && v <= 0.2;
                    let base = seg(u, v, (-0.9, 0.25), (0.9, 0.25), e);
                    if arch { Some((if kind == CaveMouth || disc(u, v, 0.0, 0.2, 0.55) { INK } else { ink }, 0.9)) }
                    else if base { Some((INK, 0.9)) }
                    else if kind == PaintedCave && disc(u, v, 0.0, -0.1, 0.18) { Some((RUBRIC, 1.0)) }
                    else if kind == Lair && (disc(u, v, -0.25, -0.1, 0.1) || disc(u, v, 0.25, -0.1, 0.1)) { Some((RUBRIC, 1.0)) }
                    else { None }
                }
                Spring | SaltSpring => {
                    if ring(u, v, 0.0, -0.3, 0.35, e) { Some((ink, 0.9)) }
                    else if seg(u, v, (0.0, 0.05), (0.25, 0.45), e * 0.8) || seg(u, v, (0.25, 0.45), (-0.1, 0.9), e * 0.8) { Some((SEA_INK, 0.8)) }
                    else if kind == SaltSpring && ring(u, v, 0.0, -0.3, 0.6, e * 0.6) && ((u * 7.0).floor() as i64 + (v * 7.0).floor() as i64).rem_euclid(2) == 0 { Some((INK, 0.6)) }
                    else { None }
                }
                Waterfall => {
                    if seg(u, v, (-0.8, -0.3), (0.8, -0.3), e * 1.3) { Some((SEA_INK, 0.95)) }
                    else if [-0.5f32, 0.0, 0.5].iter().any(|&ux| seg(u, v, (ux, -0.2), (ux, 0.7), e * 0.7)) { Some((SEA_INK, 0.7)) }
                    else { None }
                }
                HotSpring | Geyser | MudPot => {
                    let wavy = |ux: f32| (0..6).any(|i| { let t0 = i as f32 / 6.0; let t1 = (i + 1) as f32 / 6.0; let f = |t: f32| (ux + 0.18 * (t * 9.0).sin(), 0.7 - 1.5 * t); seg(u, v, f(t0), f(t1), e * 0.8) });
                    let pool = ring(u, v, 0.0, 0.6, 0.55, e) && v > 0.5;
                    if kind == HotSpring && (wavy(-0.4) || wavy(0.0) || wavy(0.4) || pool) { Some((INK, 0.8)) }
                    else if kind == Geyser && (seg(u, v, (0.0, 0.9), (0.0, -0.2), e * 1.2) || seg(u, v, (0.0, -0.2), (-0.6, -0.9), e * 0.8) || seg(u, v, (0.0, -0.2), (0.6, -0.9), e * 0.8) || seg(u, v, (0.0, -0.2), (0.0, -1.0), e * 0.8) || pool) { Some((INK, 0.85)) }
                    else if kind == MudPot && (ring(u, v, 0.0, 0.2, 0.7, e) || ring(u, v, -0.25, 0.1, 0.18, e * 0.8) || ring(u, v, 0.3, 0.35, 0.14, e * 0.8)) { Some((INK, 0.8)) }
                    else { None }
                }
                NaturalArch => {
                    let arch = ring(u, v, 0.0, 0.3, 0.75, e * 2.0) && v < 0.3;
                    let legs = seg(u, v, (-0.75, 0.3), (-0.75, 0.9), e * 2.0) || seg(u, v, (0.75, 0.3), (0.75, 0.9), e * 2.0);
                    if arch || legs { Some((ink, 0.85)) } else { None }
                }
                SeaStack | Tor | Erratic | BoulderField => {
                    let rock = super::local_ink::rock_wash(RockType::Granite);
                    let shape = match kind {
                        SeaStack => u.abs() < 0.3 + 0.2 * (v + 1.0) * 0.5 && v > -0.9 && v < 0.9,
                        Tor => (u.abs() < 0.7 && v > 0.0 && v < 0.8) || (u.abs() < 0.5 && v > -0.8 && v < 0.0),
                        Erratic => disc(u, v, 0.0, 0.1, 0.7) && v < 0.6,
                        _ => disc(u, v, -0.45, 0.3, 0.4) || disc(u, v, 0.4, 0.35, 0.35) || disc(u, v, 0.05, -0.35, 0.33),
                    };
                    if !shape { None } else {
                        let rim = match kind {
                            SeaStack => !(u.abs() < 0.3 + 0.2 * (v + 1.0) * 0.5 - e * 1.5 && v > -0.9 + e * 1.5 && v < 0.9 - e * 1.5),
                            _ => ((u + v) * 7.0).floor().rem_euclid(3.0) == 0.0 && u + v > 0.2,
                        };
                        Some((if rim { INK } else { rock }, 0.9))
                    }
                }
                TarSeep => {
                    if disc(u, v, 0.0, 0.0, 0.6) || disc(u, v, 0.4, 0.5, 0.25) || disc(u, v, -0.5, 0.4, 0.2) { Some(([30.0, 26.0, 24.0], 0.9)) } else { None }
                }
                PetrifiedForest => {
                    if [-0.6f32, -0.1, 0.45].iter().enumerate().any(|(i, &ux)| seg(u, v, (ux, 0.8), (ux, -0.2 - 0.2 * i as f32), e * 1.6)) { Some((super::local_ink::rock_wash(RockType::Sandstone), 0.95)) }
                    else if [-0.6f32, -0.1, 0.45].iter().enumerate().any(|(i, &ux)| seg(u, v, (ux, 0.8), (ux, -0.2 - 0.2 * i as f32), e * 2.4)) { Some((INK, 0.8)) }
                    else { None }
                }
                FossilBed => {
                    // An ammonite: a spiral of three turns.
                    let ang = v.atan2(u);
                    let rad = (u * u + v * v).sqrt();
                    let turn = (ang + std::f32::consts::PI) / std::f32::consts::TAU;
                    let spiral = (0..3).any(|n| ((rad - 0.28 * (n as f32 + turn)).abs() < e * 0.9) && rad < 0.9);
                    if spiral { Some((INK, 0.85)) } else { None }
                }
                AmberShore => {
                    if disc(u, v, 0.0, 0.2, 0.5) || (u.abs() < 0.5 - 0.5 * (-v - 0.2).max(0.0) * 1.4 && v > -0.9 && v < 0.2) {
                        let rim = !(disc(u, v, 0.0, 0.2, 0.5 - e * 1.5) || (u.abs() < 0.5 - 0.5 * (-v - 0.2).max(0.0) * 1.4 - e * 1.5 && v > -0.9 + e * 2.0 && v < 0.2));
                        Some((if rim { INK } else { [214.0, 150.0, 50.0] }, 0.95))
                    } else { None }
                }
                MeteorCrater => {
                    let rays = [(0.0f32, -1.0f32), (0.7, -0.7), (1.0, 0.0), (-0.7, -0.7), (-1.0, 0.0)];
                    if ring(u, v, 0.0, 0.0, 0.5, e * 1.2) { Some((INK, 0.9)) }
                    else if rays.iter().any(|&(dx, dy)| seg(u, v, (dx * 0.6, dy * 0.6), (dx, dy), e * 0.8)) { Some((INK, 0.6)) }
                    else { None }
                }
                Oxbow => {
                    let ang = v.atan2(u);
                    if ring(u, v, 0.0, 0.0, 0.6, e * 1.4) && ang.abs() > 0.8 { Some((SEA_INK, 0.9)) } else { None }
                }
                Tarn | CraterLake => {
                    let lake = disc(u, v, 0.0, 0.1, 0.5);
                    if lake { Some((if ring(u, v, 0.0, 0.1, 0.5, e * 1.2) { SEA_INK } else { LAKE }, 0.95)) }
                    else if kind == CraterLake && ring(u, v, 0.0, 0.1, 0.85, e * 1.2) { Some((INK, 0.8)) }
                    else { None }
                }
                Pavement => {
                    if (u.abs() < 0.8 && v.abs() < 0.8) && (((u + 0.8) * 2.5).fract() < 0.18 || ((v + 0.8) * 2.5).fract() < 0.18) { Some((INK, 0.7)) } else { None }
                }
                Blowhole => {
                    if ring(u, v, 0.0, 0.4, 0.35, e) || seg(u, v, (0.0, 0.1), (0.0, -0.6), e) || seg(u, v, (0.0, -0.6), (-0.4, -0.95), e * 0.8) || seg(u, v, (0.0, -0.6), (0.4, -0.95), e * 0.8) { Some((SEA_INK, 0.85)) } else { None }
                }
                GiantTree => {
                    let rr = (u * u + (v + 0.2).powi(2)).sqrt();
                    let wob = 0.8 + 0.1 * ((u.atan2(v + 0.2)) * 6.0).cos();
                    let crown = rr < wob;
                    let trunk = u.abs() < 0.12 && v > 0.5 && v < 1.0;
                    if crown || trunk { Some((if (crown && rr > wob - e * 1.5) || (trunk && u.abs() > 0.05) { INK } else { super::ink::leafy([150.0, 164.0, 116.0]) }, 0.95)) } else { None }
                }
                RockShelter => {
                    let roof = ring(u, v, 0.3, 0.3, 0.9, e * 2.0) && u < 0.3 && v < 0.3;
                    let hearth = disc(u, v, 0.1, 0.5, 0.15);
                    if roof { Some((INK, 0.9)) } else if hearth { Some((RUBRIC, 0.9)) } else { None }
                }
                KillSite | BoneBed | BoneField => {
                    let bone = |a: (f32, f32), b: (f32, f32)| seg(u, v, a, b, e * 1.2) || disc(u, v, a.0, a.1, e * 2.0) || disc(u, v, b.0, b.1, e * 2.0);
                    let bones = bone((-0.7, 0.6), (0.3, -0.5)) || (kind != KillSite && (bone((-0.2, 0.7), (0.8, 0.4)) || bone((0.1, -0.7), (0.7, -0.1))));
                    if bones { Some((if kind == BoneField { RUBRIC } else { INK }, 0.85)) }
                    else if kind == KillSite && seg(u, v, (-0.5, -0.8), (0.8, 0.7), e * 0.8) { Some((INK, 0.9)) }
                    else { None }
                }
                ShellMidden => {
                    let mound = disc(u, v, 0.0, 0.8, 0.95) && v < 0.8;
                    if mound {
                        let shells = [(-0.4f32, 0.4f32), (0.1, 0.1), (0.45, 0.5)].iter().any(|&(sx, sy)| ring(u, v, sx, sy, 0.15, e) && v < sy);
                        Some((if shells || ring(u, v, 0.0, 0.8, 0.95, e * 1.2) { INK } else { SAND }, 0.9))
                    } else { None }
                }
                FlintMine | OldMine | Quarry | CopperWorking => {
                    match kind {
                        FlintMine => if ring(u, v, 0.0, 0.0, 0.55, e) || (u.abs() < 0.25 && v.abs() < 0.25) { Some((INK, 0.9)) } else { None },
                        OldMine => if u.abs() < 0.5 && v > -0.4 && v < 0.5 { Some((if u.abs() > 0.5 - e * 2.0 || v < -0.4 + e * 2.0 { INK } else { [60.0, 50.0, 44.0] }, 0.95)) } else { None },
                        Quarry => if ring(u, v, 0.2, 0.2, 0.8, e * 1.4) && u < 0.2 && v < 0.2 || (u < 0.2 && v < 0.2 && u > -0.6 && v > -0.6 && ((u - v) * 6.0).floor().rem_euclid(2.0) == 0.0) { Some((INK, 0.8)) } else { None },
                        _ => if seg(u, v, (-0.6, 0.8), (0.4, -0.5), e * 1.2) || ring(u, v, 0.4, -0.5, 0.5, e * 1.2) && v < -0.5 { Some(([170.0, 100.0, 60.0], 0.95)) } else { None },
                    }
                }
                LongBarrow | RoundBarrows | Enclosure | Henge | HillFort | StoneCircle => {
                    let dotted = |rad_u: f32, rad_v: f32, ox: f32, oy: f32, n: f32| {
                        let dd = (((u - ox) / rad_u).powi(2) + ((v - oy) / rad_v).powi(2)).sqrt();
                        let ang = (v - oy).atan2(u - ox);
                        (dd - 1.0).abs() < e * 1.3 && ((ang / std::f32::consts::TAU * n).fract() < 0.5)
                    };
                    match kind {
                        LongBarrow => if dotted(0.85, 0.4, 0.0, 0.0, 14.0) { Some((INK, 0.9)) } else if (u / 0.85).powi(2) + (v / 0.4).powi(2) < 1.0 && (x + y).rem_euclid(3) == 0 { Some((INK, 0.25)) } else { None },
                        RoundBarrows => if dotted(0.3, 0.3, -0.5, 0.3, 8.0) || dotted(0.3, 0.3, 0.45, 0.4, 8.0) || dotted(0.3, 0.3, 0.0, -0.45, 8.0) { Some((INK, 0.9)) } else { None },
                        Enclosure => if dotted(0.85, 0.6, 0.0, 0.0, 10.0) { Some((INK, 0.9)) } else { None },
                        Henge => if ring(u, v, 0.0, 0.0, 0.45, e) || dotted(0.8, 0.8, 0.0, 0.0, 16.0) { Some((INK, 0.9)) } else { None },
                        HillFort => {
                            let rad = (u * u + v * v).sqrt();
                            let ang = v.atan2(u);
                            if ring(u, v, 0.0, 0.0, 0.55, e * 1.3) { Some((INK, 0.95)) }
                            else if rad > 0.55 && rad < 0.85 && ((ang / std::f32::consts::TAU * 16.0).fract() < 0.22) { Some((INK, 0.8)) }
                            else { None }
                        }
                        _ => if (0..8).any(|i| { let a = i as f32 / 8.0 * std::f32::consts::TAU; disc(u, v, 0.7 * a.cos(), 0.7 * a.sin(), e * 1.8) }) { Some((INK, 0.95)) } else { None },
                    }
                }
                StandingStone | Dolmen | RockArt => {
                    match kind {
                        StandingStone => if u.abs() < 0.18 && v > -0.9 && v < 0.6 { Some((if u > 0.05 { INK } else { super::local_ink::rock_wash(RockType::Granite) }, 0.95)) } else if seg(u, v, (-0.5, 0.65), (0.5, 0.65), e) { Some((INK, 0.6)) } else { None },
                        Dolmen => if (u.abs() < 0.85 && v > -0.6 && v < -0.2) || seg(u, v, (-0.5, -0.2), (-0.5, 0.6), e * 2.2) || seg(u, v, (0.5, -0.2), (0.5, 0.6), e * 2.2) { Some((if v < -0.6 + e * 2.0 || v > -0.2 - e * 2.0 { INK } else { super::local_ink::rock_wash(RockType::Granite) }, 0.95)) } else { None },
                        _ => if ring(u, v, 0.0, 0.1, 0.7, e * 1.2) && v < 0.3 || seg(u, v, (-0.7, 0.3), (0.7, 0.3), e * 1.2) { Some((INK, 0.9)) } else if [(-0.3f32, -0.2f32), (0.2, -0.3), (0.0, 0.05)].iter().any(|&(sx, sy)| ring(u, v, sx, sy, 0.1, e * 0.9)) { Some((INK, 0.8)) } else { None },
                    }
                }
                Crannog => {
                    if ring(u, v, 0.0, -0.2, 0.4, e * 1.3) { Some((INK, 0.9)) }
                    else if seg(u, v, (0.0, 0.2), (0.0, 0.95), e) && ((v * 6.0).floor() as i64).rem_euclid(2) == 0 { Some((INK, 0.8)) }
                    else if ring(u, v, 0.0, -0.2, 0.75, e * 0.7) && v > -0.2 { Some((SEA_INK, 0.6)) }
                    else { None }
                }
                BogHoard | BogBody => {
                    if kind == BogHoard && (disc(u, v, 0.0, 0.1, 0.5) && v > 0.0 || seg(u, v, (-0.5, 0.0), (0.5, 0.0), e * 1.2)) { Some((if ring(u, v, 0.0, 0.1, 0.5, e * 1.2) { INK } else { [200.0, 160.0, 60.0] }, 0.95)) }
                    else if kind == BogBody && (disc(u, v, -0.55, -0.1, 0.22) || (u > -0.35 && u < 0.8 && v.abs() < 0.25)) { Some((if ring(u, v, -0.55, -0.1, 0.22, e) || v.abs() > 0.25 - e * 1.5 { INK } else { [150.0, 110.0, 80.0] }, 0.9)) }
                    else { None }
                }
                DesertKite => {
                    if seg(u, v, (-0.9, 0.9), (-0.15, -0.3), e) || seg(u, v, (0.9, 0.9), (0.15, -0.3), e) || ring(u, v, 0.0, -0.5, 0.25, e) { Some((INK, 0.85)) } else { None }
                }
                Ridgeway | Causeway | Lynchets | Ford => {
                    match kind {
                        Ridgeway => if seg(u, v, (-0.9, 0.4), (0.9, -0.4), e * 1.2) && ((u * 5.0).floor() as i64).rem_euclid(2) == 0 { Some((INK, 0.85)) } else { None },
                        Causeway => if (seg(u, v, (-0.9, 0.2), (0.9, -0.2), e * 2.2) && !seg(u, v, (-0.9, 0.2), (0.9, -0.2), e * 0.8)) && ((u * 6.0).floor() as i64).rem_euclid(2) == 0 { Some((INK, 0.8)) } else { None },
                        Lynchets => if [-0.5f32, 0.0, 0.5].iter().any(|&vy| seg(u, v, (-0.8, vy + 0.15), (0.8, vy - 0.15), e)) { Some((INK, 0.8)) } else { None },
                        _ => if v.abs() < 0.15 && ((u * 5.0).floor() as i64).rem_euclid(2) == 0 && u.abs() < 0.9 { Some((SEA_INK, 0.9)) } else if seg(u, v, (-0.9, -0.5), (0.9, -0.5), e * 0.7) || seg(u, v, (-0.9, 0.5), (0.9, 0.5), e * 0.7) { Some((SEA_INK, 0.5)) } else { None },
                    }
                }
                ClearanceCairns => {
                    if [(-0.5f32, 0.3f32), (0.4, 0.4), (0.0, -0.4)].iter().any(|&(sx, sy)| (v - sy) > -0.3 && (v - sy) < 0.1 && (u - sx).abs() < 0.3 * (1.0 - ((v - sy) + 0.3) / 0.4).max(0.0) + 0.05) { Some((INK, 0.75)) } else { None }
                }
                Watchtower | MillRuin | DrownedVillage | Ruin | Tomb => {
                    match kind {
                        Watchtower => {
                            let body = u.abs() < 0.3 && v > -0.6 && v < 0.9;
                            let broken = v < -0.3 && ((u * 7.0).floor() as i64 + (hs as i64 & 1)).rem_euclid(2) == 0;
                            if body && !broken { Some((if u.abs() > 0.3 - e * 1.8 || v > 0.9 - e * 1.5 { INK } else { super::local_ink::rock_wash(RockType::Sandstone) }, 0.95)) } else { None }
                        }
                        MillRuin => {
                            let ang = v.atan2(u);
                            if ring(u, v, 0.0, 0.0, 0.7, e * 1.2) && ang < 2.0 || (0..6).any(|i| { let a = i as f32 / 6.0 * std::f32::consts::TAU; seg(u, v, (0.0, 0.0), (0.7 * a.cos(), 0.7 * a.sin()), e * 0.9) && a < 2.0 || a > 4.5 }) { Some((INK, 0.85)) } else { None }
                        }
                        DrownedVillage => {
                            let houses = (u.abs() < 0.25 && v > -0.1 && v < 0.4) || (u > 0.35 && u < 0.8 && v > 0.0 && v < 0.4) || (u < -0.4 && u > -0.85 && v > 0.1 && v < 0.4);
                            if houses { Some((INK, 0.7)) }
                            else if (v + 0.2 * (u * 7.0).sin()).abs() < e * 1.2 || (v + 0.5 + 0.2 * (u * 7.0 + 1.5).sin()).abs() < e * 1.2 { Some((SEA_INK, 0.8)) }
                            else { None }
                        }
                        Ruin => if seg(u, v, (-0.8, 0.6), (-0.8, -0.3), e * 2.0) || seg(u, v, (-0.8, 0.6), (0.2, 0.6), e * 2.0) || seg(u, v, (0.5, 0.6), (0.9, 0.6), e * 2.0) || seg(u, v, (0.3, -0.7), (0.3, -0.1), e * 2.0) { Some((INK, 0.85)) } else { None },
                        _ => if u.abs() < 0.6 && v.abs() < 0.8 { Some((if u.abs() > 0.6 - e * 1.8 || v.abs() > 0.8 - e * 1.8 || (u.abs() < 0.3 && v.abs() < 0.45 && (u.abs() > 0.3 - e * 1.5 || v.abs() > 0.45 - e * 1.5)) { INK } else { super::local_ink::rock_wash(RockType::Limestone) }, 0.95)) } else { None },
                    }
                }
                Battlefield => {
                    if seg(u, v, (-0.8, -0.8), (0.8, 0.8), e * 1.4) || seg(u, v, (0.8, -0.8), (-0.8, 0.8), e * 1.4) || seg(u, v, (-0.9, -0.3), (-0.3, -0.9), e * 1.2) || seg(u, v, (0.9, -0.3), (0.3, -0.9), e * 1.2) { Some((RUBRIC, 0.9)) } else { None }
                }
                Gorge => {
                    if seg(u, v, (-0.5, -0.9), (-0.3, 0.9), e * 1.6) || seg(u, v, (0.5, -0.9), (0.3, 0.9), e * 1.6) { Some((INK, 0.9)) }
                    else if u.abs() < 0.3 && ((u - v) * 5.0).floor().rem_euclid(2.0) == 0.0 { Some((INK, 0.3)) }
                    else { None }
                }
                #[allow(unreachable_patterns)]
                ImpactStructure => {
                    let ticks = (0..12).any(|i| { let a = i as f32 / 12.0 * std::f32::consts::TAU; seg(u, v, (0.62 * a.cos(), 0.62 * a.sin()), (0.9 * a.cos(), 0.9 * a.sin()), e * 0.8) });
                    if ring(u, v, 0.0, 0.0, 0.6, e * 1.4) { Some((INK, 0.95)) }
                    else if ticks { Some((INK, 0.6)) }
                    else if disc(u, v, 0.0, 0.0, 0.14) { Some((INK, 0.8)) }
                    else { None }
                }
                Landslide | CollapsedFlank => {
                    let scar = ring(u, v, 0.0, if kind == Landslide { -0.2 } else { 0.0 }, 0.6, e * 1.3) && v < if kind == Landslide { -0.1 } else { 0.35 };
                    let dots = [(-0.3f32, 0.35f32), (0.1, 0.55), (0.4, 0.3), (-0.05, 0.8), (0.35, 0.75)].iter().any(|&(dx, dy)| disc(u, v, dx, dy, 0.09));
                    if scar { Some((INK, 0.9)) } else if dots { Some((INK, 0.75)) } else { None }
                }
                LandslideLake | SagPond | Maar => {
                    let lake = disc(u, v, 0.0, 0.0, 0.5);
                    let edge = match kind {
                        LandslideLake => [(0.55f32, -0.4f32), (0.62, -0.1), (0.62, 0.2), (0.55, 0.5)].iter().any(|&(dx, dy)| disc(u, v, dx, dy, 0.1)),
                        SagPond => seg(u, v, (-0.95, -0.95), (0.95, 0.95), e * 0.9),
                        _ => (0..10).any(|i| { let a = i as f32 / 10.0 * std::f32::consts::TAU; disc(u, v, 0.72 * a.cos(), 0.72 * a.sin(), 0.08) }),
                    };
                    if lake { Some((if ring(u, v, 0.0, 0.0, 0.5, e * 1.2) { SEA_INK } else { LAKE }, 0.95)) }
                    else if edge { Some((INK, 0.8)) } else { None }
                }
                FaultScarp => {
                    if seg(u, v, (-0.95, -0.1), (0.95, 0.1), e * 1.4) { Some((INK, 0.95)) }
                    else if [-0.6f32, 0.0, 0.6].iter().any(|&x| seg(u, v, (x, 0.1 * x / 0.95), (x, 0.1 * x / 0.95 + 0.35), e * 1.0)) { Some((INK, 0.8)) }
                    else { None }
                }
                GasHollow => {
                    if ring(u, v, 0.0, 0.0, 0.65, e * 1.1) { Some((ink, 0.85)) }
                    else if seg(u, v, (-0.35, -0.35), (0.35, 0.35), e * 1.2) || seg(u, v, (0.35, -0.35), (-0.35, 0.35), e * 1.2) { Some((ink, 0.9)) }
                    else { None }
                }
                Sinkhole => {
                    let ticks = (0..10).any(|i| { let a = i as f32 / 10.0 * std::f32::consts::TAU; seg(u, v, (0.65 * a.cos(), 0.65 * a.sin()), (0.4 * a.cos(), 0.4 * a.sin()), e * 0.8) });
                    if ring(u, v, 0.0, 0.0, 0.65, e * 1.2) { Some((ink, 0.9)) } else if ticks { Some((ink, 0.65)) } else if disc(u, v, 0.0, 0.0, 0.18) { Some((INK, 0.9)) } else { None }
                }
                DryFalls => {
                    if seg(u, v, (-0.8, -0.3), (0.8, -0.3), e * 1.3) { Some((INK, 0.95)) }
                    else if [-0.5f32, 0.0, 0.5].iter().any(|&ux| seg(u, v, (ux, -0.2), (ux, 0.7), e * 0.7)) && ((v * 6.0).floor() as i64).rem_euclid(2) == 0 { Some((INK, 0.6)) }
                    else { None }
                }
                Quicksand => {
                    let dashed = ring(u, v, 0.0, 0.0, 0.7, e * 1.1) && ((v.atan2(u) * 4.0).floor() as i64).rem_euclid(2) == 0;
                    let wav = [-0.25f32, 0.15].iter().any(|&oy| ((v - oy) - 0.12 * (u * 9.0).sin()).abs() < e * 0.9 && u.abs() < 0.5);
                    if dashed { Some((ink, 0.85)) } else if wav { Some((SEA_INK, 0.8)) } else { None }
                }
                CrevasseField => {
                    let zig = [-0.45f32, 0.0, 0.45].iter().any(|&oy| (((u * 3.0).rem_euclid(1.0) - 0.5).abs() * 0.5 - (v - oy) - 0.1).abs() < e * 0.9 && u.abs() < 0.8);
                    if zig { Some((ICE_INK, 0.9)) } else if ring(u, v, 0.0, 0.0, 0.9, e * 0.8) { Some((ink, 0.5)) } else { None }
                }
                TarPit => {
                    if disc(u, v, 0.0, 0.1, 0.6) { Some(([30.0, 26.0, 24.0], 0.9)) }
                    else if seg(u, v, (-0.7, -0.6), (-0.2, -0.75), e * 1.2) || seg(u, v, (0.3, -0.75), (0.75, -0.55), e * 1.2) { Some((PAPER, 0.95)) }
                    else { None }
                }
                CrownHoles => {
                    if ring(u, v, -0.4, 0.3, 0.28, e) || ring(u, v, 0.35, 0.35, 0.24, e) || ring(u, v, 0.0, -0.35, 0.26, e) { Some((ink, 0.85)) } else { None }
                }
                Skyburn => {
                    // Felled trunks pointing out from a point.
                    let rays = (0..10).any(|i| { let a = i as f32 / 10.0 * std::f32::consts::TAU; seg(u, v, (0.35 * a.cos(), 0.35 * a.sin()), (0.9 * a.cos(), 0.9 * a.sin()), e * 0.9) });
                    if disc(u, v, 0.0, 0.0, 0.16) { Some((RUBRIC, 0.9)) } else if rays { Some((INK, 0.75)) } else { None }
                }
            };
            if let Some((c, a)) = px { put(x, y, c, a); }
        }
    }
}

/// The places in view: a glyph each, a dashed line along the cells of a track, and labels
/// placed greatest first without overlap. Returns the boxes the labels took (screen pixels), so
/// the town labels can keep off them.
pub fn draw_places(ink: &Ink, cam: &ZoomCamera, buf: &mut [u32], w: usize, h: usize) -> Vec<(i64, i64, i64, i64)> {
    let ppc = cam.px_per_cell;
    let to_screen = |x: f64, y: f64| ((x as f32 - cam.cx) * ppc + w as f32 / 2.0, (y as f32 - cam.cy) * ppc + h as f32 / 2.0);
    let size = (ppc * 2.6).clamp(10.0, 26.0);
    let mut shown: Vec<(&Place, f32, f32)> = Vec::new();
    for p in &ink.places {
        if ppc < min_ppc(p) { continue; }
        let (sx, sy) = to_screen(p.x, p.y);
        if sx < -size || sy < -size || sx > w as f32 + size || sy > h as f32 + size { continue; }
        // A track is a dashed line along its cells.
        if matches!(p.kind, PlaceKind::Ridgeway | PlaceKind::Causeway) && p.cells.len() > 1 {
            let dashed = p.kind == PlaceKind::Ridgeway;
            let mut run = 0.0f32;
            for pair in p.cells.windows(2) {
                let (a, b) = (to_screen(pair[0].0 as f64 + 0.5, pair[0].1 as f64 + 0.5), to_screen(pair[1].0 as f64 + 0.5, pair[1].1 as f64 + 0.5));
                let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt().max(1.0);
                let n = len.ceil() as i32;
                for i in 0..=n {
                    let t = i as f32 / n as f32;
                    run += len / n as f32;
                    if dashed && run.rem_euclid(8.0) > 5.0 { continue; }
                    if !dashed && (run.rem_euclid(4.0) > 2.0) { continue; }
                    let (x, y) = ((a.0 + (b.0 - a.0) * t) as i64, (a.1 + (b.1 - a.1) * t) as i64);
                    super::ui::blend_px(buf, w, h, x, y, 0x0038_2A20, 0.75);
                    if !dashed { super::ui::blend_px(buf, w, h, x, y + 1, 0x0038_2A20, 0.4); }
                }
            }
        }
        draw_glyph(buf, w, h, p.kind, sx, sy, size, ink.seed);
        shown.push((p, sx, sy));
    }
    // Labels: greatest first, whole or not at all, none over another or over a glyph.
    shown.sort_by_key(|(p, _, _)| p.rank);
    let mut boxes: Vec<(i64, i64, i64, i64)> = shown.iter().map(|&(_, sx, sy)| ((sx - size / 2.0) as i64, (sy - size / 2.0) as i64, (sx + size / 2.0) as i64, (sy + size / 2.0) as i64)).collect();
    let n_glyphs = boxes.len();
    let mut names: Vec<&str> = Vec::new();
    for &(p, sx, sy) in &shown {
        let text = if p.name.is_empty() { p.kind.word().trim_start_matches("a ").trim_start_matches("an ").to_string() } else { p.name.clone() };
        if names.contains(&text.as_str()) { continue; }
        let (face, px, tracking, color) = label_style(p);
        let tw = super::fonts::width(&text, face, px, tracking);
        let (asc, desc) = super::fonts::line_metrics(face, px);
        let th = asc + desc;
        let (x0, y0) = ((sx - tw / 2.0) as i64, (sy + size / 2.0 + 1.0) as i64);
        let (x1, y1) = (x0 + tw.ceil() as i64, y0 + th.ceil() as i64);
        if x0 < 3 || y0 < 3 || x1 > w as i64 - 3 || y1 > h as i64 - 3 { continue; }
        let hits = |r: &(i64, i64, i64, i64)| x0 - 4 < r.2 && x1 + 4 > r.0 && y0 - 3 < r.3 && y1 + 3 > r.1;
        // Its own glyph sits right above; every other box must be clear.
        let own = ((sx - size / 2.0) as i64, (sy - size / 2.0) as i64, (sx + size / 2.0) as i64, (sy + size / 2.0) as i64);
        if boxes.iter().any(|r| *r != own && hits(r)) { continue; }
        super::fonts::draw(buf, w, h, x0 as f32, y0 as f32, &text, face, px, tracking, color, Some(0x00EE_E4CC));
        boxes.push((x0, y0, x1, y1));
        names.push(if p.name.is_empty() { "" } else { p.name.as_str() });
    }
    let _ = n_glyphs;
    boxes
}

/// One line for `--local-snapshot`: the ground's classes by share of the land, and the places
/// by kind ("demo" when the gallery made them).
pub fn summary(ink: &Ink) -> String {
    let mut counts: Vec<(Landform, usize)> = Vec::new();
    let mut land = 0usize;
    for (k, &l) in ink.land.iter().enumerate() {
        if ink.water[k] > 0.5 { continue; }
        land += 1;
        if l == Landform::None { continue; }
        match counts.iter_mut().find(|(c, _)| *c == l) { Some(c) => c.1 += 1, None => counts.push((l, 1)) }
    }
    counts.sort_by(|a, b| b.1.cmp(&a.1));
    let mix: Vec<String> = counts.iter().map(|(l, n)| format!("{} {:.1}%", l.name(), 100.0 * *n as f32 / land.max(1) as f32)).collect();
    format!("{} classes ({}){}; {} places of {} kinds{}", counts.len(), if mix.is_empty() { "plain ground only".into() } else { mix.join(", ") },
        "", ink.places.len(), { let mut ks: Vec<PlaceKind> = ink.places.iter().map(|p| p.kind).collect(); ks.dedup(); ks.sort_by_key(|k| *k as u8); ks.dedup(); ks.len() },
        if ink.demo { " (PLANET_INK_DEMO: the gallery's own)" } else { "" })
}

/// The key: a parchment card bottom-left naming the ground in view by family (a swatch each)
/// and the places' glyphs in view (a glyph each). `K` in the walker.
pub fn draw_legend(ink: &Ink, cam: &ZoomCamera, buf: &mut [u32], w: usize, h: usize) {
    use super::ui::*;
    if w < 340 || h < 160 { return; }
    let ppc = cam.px_per_cell;
    let (x0, y0) = ((cam.cx - w as f32 / 2.0 / ppc).max(0.0) as usize, (cam.cy - h as f32 / 2.0 / ppc).max(0.0) as usize);
    let (x1, y1) = (((cam.cx + w as f32 / 2.0 / ppc) as usize).min(ink.w), ((cam.cy + h as f32 / 2.0 / ppc) as usize).min(ink.h));
    // The landforms in view, by cells (every other cell sampled).
    let mut counts: Vec<(Landform, usize)> = Vec::new();
    let mut y = y0;
    while y < y1 {
        let mut x = x0;
        while x < x1 {
            let l = ink.land[y * ink.w + x];
            if l != Landform::None {
                match counts.iter_mut().find(|(k, _)| *k == l) { Some(c) => c.1 += 1, None => counts.push((l, 1)) }
            }
            x += 2;
        }
        y += 2;
    }
    counts.sort_by(|a, b| b.1.cmp(&a.1));
    let kinds: Vec<PlaceKind> = {
        let mut ks: Vec<PlaceKind> = Vec::new();
        for p in &ink.places {
            if ppc < min_ppc(p) { continue; }
            let (sx, sy) = ((p.x as f32 - cam.cx) * ppc + w as f32 / 2.0, (p.y as f32 - cam.cy) * ppc + h as f32 / 2.0);
            if sx >= 0.0 && sy >= 0.0 && sx < w as f32 && sy < h as f32 && !ks.contains(&p.kind) { ks.push(p.kind); }
        }
        ks
    };
    let rows: Vec<(Option<Landform>, Option<PlaceKind>, String)> = counts.iter().take(14).map(|(l, _)| (Some(*l), None, l.name().to_string()))
        .chain(kinds.iter().take(14).map(|k| (None, Some(*k), k.word().trim_start_matches("a ").trim_start_matches("an ").to_string())))
        .collect();
    if rows.is_empty() { return; }
    let cols = if rows.len() > 9 { 2 } else { 1 };
    let per = (rows.len() + cols - 1) / cols;
    let row_h = 17;
    let r = Rect { x: 12, y: h - (per * row_h + 44), w: 12 + cols * 190, h: per * row_h + 32 };
    card(buf, w, r);
    super::text::draw_fell(buf, w, h, (r.x + 14) as i64, r.y as i64 + 10, "KEY", RUBRIC, 1, true);
    let hint = "K hides";
    super::text::draw_fell(buf, w, h, (r.x + r.w - 14 - super::text::fell_width(hint, 1, false)) as i64, r.y as i64 + 10, hint, INK_FADED, 1, false);
    for (i, (l, k, name)) in rows.iter().enumerate() {
        let (col, row) = (i / per, i % per);
        let (x, y) = (r.x + 14 + col * 190, r.y + 28 + row * row_h);
        if let Some(l) = l {
            let c = ground_wash(*l, RockType::Granite, [196.0, 190.0, 160.0]);
            let sw = Rect { x, y: y + 2, w: 18, h: 11 };
            let px = ((c[0] as u32) << 16) | ((c[1] as u32) << 8) | c[2] as u32;
            fill(buf, w, sw, px);
            // The family's mark on the swatch: a stroke for rock, a dot for wet, a bar for water.
            match l.family() {
                Family::Rock | Family::Karst => for i in 0..3 { blend_px(buf, w, h, (x + 4 + i * 4) as i64, (y + 9) as i64, INK, 0.8); blend_px(buf, w, h, (x + 4 + i * 4) as i64, (y + 8) as i64, INK, 0.5); },
                Family::Wet | Family::River => for i in 0..2 { blend_px(buf, w, h, (x + 5 + i * 7) as i64, (y + 7) as i64, INK, 0.8); },
                Family::Ice => outline(buf, w, sw, 0x0076_94B2),
                _ => {}
            }
            outline(buf, w, sw, INK);
        } else if let Some(k) = k {
            draw_glyph(buf, w, h, *k, (x + 9) as f32, (y + 8) as f32, 14.0, ink.seed);
        }
        super::text::draw_fell(buf, w, h, (x + 26) as i64, y as i64, name, INK, 1, false);
    }
}

// ---------------------------------------------------------------------------------------------
// The gallery: demo classes and places for a region whose site layer is empty
// ---------------------------------------------------------------------------------------------

pub mod demo {
    //! `PLANET_INK_DEMO=1`: landforms guessed from the region's own fields (slope, height,
    //! wetness, rock, the shore) and one place of every kind on fitting ground, so every symbol
    //! of the region ink can be looked at before the landform and site passes land. Not the
    //! real thing: a glyph gallery.

    use super::hash;
    use crate::erosion::materials::RockType;
    use crate::region::plan::{Landform, Place, PlaceKind};
    use crate::region::zoom::ZoomRegion;

    /// Demo classes and places for `region`, in the region's own cells.
    pub fn classify(region: &ZoomRegion) -> (Vec<Landform>, Vec<Place>) {
        use Landform::*;
        let (w, h) = (region.width, region.height);
        let n = w * h;
        let at = |x: i64, y: i64| region.elevation_m[(y.clamp(0, h as i64 - 1) as usize) * w + x.clamp(0, w as i64 - 1) as usize];
        let rock = |k: usize| region.rock.get(k).copied().unwrap_or(RockType::Sandstone);
        let seed = region.params.seed ^ 0xDE30;
        let mut land = vec![None; n];
        let mut slope = vec![0.0f32; n];
        let mut sea_near = vec![false; n];
        let mut lake_near = vec![false; n];
        let mut river_near = vec![0.0f32; n];
        let (_, sy0) = (region.world_x0, region.world_y0);
        let _ = sy0;
        for y in 0..h {
            for x in 0..w {
                let k = y * w + x;
                let (xi, yi) = (x as i64, y as i64);
                let gx = (at(xi + 1, yi) - at(xi - 1, yi)) / (2.0 * region.cell_m);
                let gy = (at(xi, yi + 1) - at(xi, yi - 1)) / (2.0 * region.cell_m);
                slope[k] = (gx * gx + gy * gy).sqrt();
                for dy in -2i64..=2 { for dx in -2i64..=2 {
                    let (px, py) = (xi + dx, yi + dy);
                    if px < 0 || py < 0 || px >= w as i64 || py >= h as i64 { continue; }
                    let kk = py as usize * w + px as usize;
                    if region.elevation_m[kk] <= 0.0 { sea_near[k] = true; }
                    if region.lake_depth_m[kk] > 0.0 { lake_near[k] = true; }
                    if region.river_width_m[kk] > river_near[k] { river_near[k] = region.river_width_m[kk]; }
                } }
            }
        }
        let noise = |x: usize, y: usize, salt: u64, sp: f32| super::vnoise(x as f32 / sp, y as f32 / sp, seed ^ salt);
        for y in 0..h {
            for x in 0..w {
                let k = y * w + x;
                let e = region.elevation_m[k];
                if e <= 0.0 { if sea_near[k] && noise(x, y, 0x1A, 20.0) > 0.7 && region.elevation_m[k] > -40.0 { land[k] = Lagoon; } continue; }
                if region.lake_depth_m[k] > 0.0 { continue; }
                let (t, m, s, r) = (region.temperature_c[k], region.moisture[k], slope[k], rock(k));
                let hard = matches!(r, RockType::Granite | RockType::Basalt);
                let rw = region.river_width_m[k];
                let l = if rw > 0.0 {
                    if sea_near[k] && rw > 40.0 { Estuary } else if s > 0.2 { Waterfall } else if s > 0.07 { Rapids } else { None }
                } else if sea_near[k] {
                    if hard && s > 0.04 { SeaCliff } else if t > 24.0 && m > 0.5 { Mangrove } else if m > 0.6 && s < 0.01 { SaltMarsh } else if s < 0.004 && noise(x, y, 0x7F, 30.0) > 0.55 { TidalFlat } else if m < 0.3 && noise(x, y, 0x5D, 12.0) > 0.5 { CoastalDune } else if river_near[k] > 40.0 { Delta } else { Beach }
                } else if t < -4.0 && e > 1200.0 && s < 0.35 { Glacier }
                else if t < -1.0 && e > 900.0 { Snowfield }
                else if s > 0.5 { Cliff }
                else if s > 0.3 && e > 1500.0 { Arete }
                else if r == RockType::Shale && s > 0.1 && s < 0.35 && m < 0.3 { Badland }
                else if r == RockType::Limestone && s < 0.04 && e > 150.0 {
                    let nv = noise(x, y, 0x9A, 9.0);
                    if nv > 0.82 { Doline } else if nv < 0.06 { Sink } else if noise(x, y, 0x9B, 40.0) > 0.4 { Pavement } else { None }
                }
                else if r == RockType::Limestone && s > 0.04 && s < 0.2 && e > 100.0 && noise(x, y, 0x9C, 25.0) > 0.7 { if t > 20.0 { TowerKarst } else { DryValley } }
                else if r == RockType::Basalt && s < 0.1 && noise(x, y, 0xBA, 30.0) > 0.52 { if noise(x, y, 0xBB, 8.0) > 0.86 { Fumarole } else if noise(x, y, 0xBB, 8.0) > 0.78 { ThermalGround } else { LavaField } }
                else if r == RockType::Basalt && s > 0.1 && s < 0.3 && noise(x, y, 0xBC, 20.0) > 0.7 { CinderCone }
                else if m < 0.14 && s < 0.06 {
                    let nv = noise(x, y, 0xD0, 35.0);
                    if nv > 0.72 { SandSea } else if nv > 0.5 { Dune } else if nv < 0.12 { Playa } else if nv < 0.3 { Yardang } else { DesertPavement }
                }
                else if m > 0.72 && t < 10.0 && s < 0.02 { Bog }
                else if m > 0.6 && t < 2.0 && s < 0.03 && noise(x, y, 0xF0, 20.0) > 0.5 { PatternedGround }
                else if lake_near[k] && m > 0.55 && s < 0.015 { if noise(x, y, 0x3A, 10.0) > 0.5 { ReedBed } else { Marsh } }
                else if river_near[k] > 60.0 && s < 0.01 { if noise(x, y, 0xF1, 12.0) > 0.78 { Backswamp } else if noise(x, y, 0xF1, 12.0) < 0.1 { Levee } else { Floodplain } }
                else if river_near[k] > 30.0 && s > 0.2 { Gorge }
                else if river_near[k] > 20.0 && s < 0.03 && e > 300.0 && noise(x, y, 0xF2, 20.0) > 0.6 { Braided }
                else if t < 3.0 && s < 0.08 && e > 300.0 {
                    let nv = noise(x, y, 0x1CE, 18.0);
                    if nv > 0.8 { Moraine } else if nv > 0.7 { Drumlin } else if nv < 0.08 { Erratic } else if nv < 0.2 { Outwash } else { None }
                }
                else if hard && s > 0.12 && s < 0.5 && e > 600.0 && noise(x, y, 0xC4, 10.0) > 0.78 { if r == RockType::Granite { Tor } else { ColumnarBasalt } }
                else if s > 0.3 { Scree }
                else if s > 0.12 && noise(x, y, 0xC5, 7.0) > 0.86 { Outcrop }
                else if r == RockType::Sandstone && m < 0.3 && s > 0.15 && s < 0.4 && e > 800.0 && noise(x, y, 0xC6, 20.0) > 0.7 { if noise(x, y, 0xC7, 6.0) > 0.7 { Hoodoo } else { Mesa } }
                else { None };
                land[k] = l;
            }
        }
        // Scree below a cliff.
        let src = land.clone();
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let k = y * w + x;
                if src[k] != None { continue; }
                let uphill = (-1i64..=1).flat_map(|dy| (-1i64..=1).map(move |dx| (dx, dy))).any(|(dx, dy)| {
                    let kk = ((y as i64 + dy) as usize) * w + (x as i64 + dx) as usize;
                    src[kk] == Cliff && region.elevation_m[kk] > region.elevation_m[k]
                });
                if uphill { land[k] = Scree; }
            }
        }
        // One place of every kind, on fitting ground, spaced out, scanned in a hashed order.
        let mut places: Vec<Place> = Vec::new();
        let fits = |kind: PlaceKind, k: usize| -> bool {
            use PlaceKind::*;
            let (x, y) = (k % w, k / w);
            let e = region.elevation_m[k];
            if e <= 0.0 && !matches!(kind, SeaStack | Crannog | DrownedVillage | Blowhole) { return false; }
            let (t, m, s, r, l) = (region.temperature_c[k], region.moisture[k], slope[k], rock(k), land[k]);
            let lake = region.lake_depth_m[k] > 0.0;
            let river = region.river_width_m[k] > 0.0;
            match kind {
                CaveMouth | PaintedCave | RockShelter | FlintMine => r == RockType::Limestone && s > 0.05 && !river && !lake,
                Spring => s > 0.05 && m > 0.4 && !river && !lake,
                Waterfall => river && s > 0.05,
                HotSpring | Geyser | MudPot => r == RockType::Basalt && !river && !lake,
                Gorge => river && s > 0.15,
                NaturalArch | Blowhole | SeaStack | ShellMidden | AmberShore => sea_near[k] && (kind != SeaStack || e <= 0.0) && (kind != ShellMidden || e > 0.0),
                Erratic | BoulderField => t < 4.0 && s < 0.1 && e > 0.0,
                SaltSpring | TarSeep => m < 0.35 && s < 0.05 && e > 0.0,
                PetrifiedForest => r == RockType::Basalt && s < 0.08,
                FossilBed => matches!(r, RockType::Shale | RockType::Limestone | RockType::Sandstone) && s > 0.15,
                MeteorCrater | DesertKite => m < 0.2 && s < 0.03,
                Oxbow => river_near[k] > 60.0 && s < 0.01,
                Tarn => lake && t < 4.0,
                Tor => r == RockType::Granite && s > 0.1 && s < 0.4,
                Pavement => l == Landform::Pavement,
                GiantTree => m > 0.5 && t > 4.0 && s < 0.1,
                CraterLake => lake && r == RockType::Basalt,
                KillSite | BoneBed => matches!(l, Landform::Bog | Landform::Floodplain | Landform::Marsh) || (river_near[k] > 20.0 && s < 0.02),
                LongBarrow | RoundBarrows | Ridgeway => s > 0.03 && s < 0.15 && e > 150.0 && (0..4).all(|i| at((x as i64) + [2, -2, 0, 0][i], (y as i64) + [0, 0, 2, -2][i]) <= e + 5.0),
                StoneCircle | Henge | Enclosure | StandingStone | Dolmen => s < 0.03 && e > 100.0 && !river && !lake && m > 0.3,
                RockArt => s > 0.1 && s < 0.4 && !river,
                HillFort | Watchtower => s > 0.05 && e > 200.0 && (0..4).filter(|&i| at((x as i64) + [3, -3, 0, 0][i], (y as i64) + [0, 0, 3, -3][i]) < e - 30.0).count() >= 3,
                Crannog => lake && lake_near[k],
                BogHoard | BogBody => l == Landform::Bog,
                CopperWorking | Quarry | OldMine => s > 0.1 && s < 0.4 && !river,
                Causeway => matches!(l, Landform::Bog | Landform::Marsh | Landform::Fen),
                Ford => river && s < 0.02 && region.river_width_m[k] < 80.0,
                Lynchets | ClearanceCairns => s > 0.04 && s < 0.2 && m > 0.35 && e > 0.0,
                MillRuin => river && s > 0.01,
                DrownedVillage => lake,
                Ruin | Battlefield | BoneField | Lair | Tomb => s < 0.1 && e > 0.0 && !river && !lake,
                ImpactStructure | Sinkhole | CrownHoles => s < 0.05 && e > 0.0 && !river && !lake,
                Landslide | CollapsedFlank => s > 0.2 && !river,
                LandslideLake | SagPond | Maar => lake,
                FaultScarp => s > 0.1 && s < 0.4,
                GasHollow => r == RockType::Basalt && s < 0.05 && !lake,
                DryFalls => s > 0.15 && !river && e > 100.0,
                Quicksand => (sea_near[k] || river_near[k] > 20.0) && s < 0.01 && e > 0.0,
                CrevasseField => l == Landform::Glacier,
                TarPit => m < 0.4 && s < 0.03 && e > 0.0,
                Skyburn => m > 0.45 && s < 0.2 && !lake,
            }
        };
        const KINDS: [PlaceKind; 75] = {
            use PlaceKind::*;
            [CaveMouth, Spring, Waterfall, HotSpring, Geyser, MudPot, Gorge, NaturalArch, SeaStack, Erratic, BoulderField, SaltSpring, TarSeep,
             PetrifiedForest, FossilBed, AmberShore, MeteorCrater, Oxbow, Tarn, Tor, Pavement, Blowhole, GiantTree, CraterLake, RockShelter,
             PaintedCave, KillSite, BoneBed, ShellMidden, FlintMine, LongBarrow, RoundBarrows, StoneCircle, StandingStone, Dolmen, RockArt,
             Enclosure, Henge, HillFort, Crannog, BogHoard, BogBody, CopperWorking, DesertKite, Ridgeway, Causeway, Ford, Lynchets,
             ClearanceCairns, Watchtower, Quarry, MillRuin, DrownedVillage, Ruin, Battlefield, BoneField, Lair, Tomb, OldMine, CaveMouth,
             ImpactStructure, Landslide, LandslideLake, FaultScarp, SagPond, Maar, GasHollow, CollapsedFlank, Sinkhole, DryFalls,
             Quicksand, CrevasseField, TarPit, CrownHoles, Skyburn]
        };
        let names = ["the Hollow Hill", "Grey Mare's Tail", "the Old Mother", "Wolf's Leap", "the Seven Sisters", "Hangman's Rock", "the Black Pool",
            "Giant's Grave", "the Sleeping Hound", "Thunder Stone", "the Nine Maidens", "Raven's Crag", "the Dancing Stones", "Bride's Well",
            "the Devil's Kitchen", "Kettle Hole", "the King's Seat", "Hunter's Rest", "the Whispering Mouth", "Salt Hollow"];
        let tile = (region.world_x0.rem_euclid(region.world_width as i64) as usize, region.world_y0.clamp(0, region.world_height as i64 - 1) as usize);
        for (i, &kind) in KINDS.iter().enumerate() {
            let mut best: Option<usize> = Option::None;
            for tries in 0..4000u64 {
                let hs = hash(tries as i64, i as i64, seed);
                let k = (hs % n as u64) as usize;
                if !fits(kind, k) { continue; }
                let (x, y) = ((k % w) as f64, (k / w) as f64);
                if places.iter().any(|p| ((p.x - x).powi(2) + (p.y - y).powi(2)).sqrt() < 40.0) { continue; }
                best = Some(k);
                break;
            }
            let Some(k) = best else { continue };
            let (x, y) = ((k % w) as f64 + 0.5, (k / w) as f64 + 0.5);
            let mut p = Place::point(kind, x, y, tile);
            p.name = if i % 3 == 0 { names[(hash(i as i64, 7, seed) % names.len() as u64) as usize].to_string() } else { String::new() };
            p.rank = match i % 4 { 0 => 20, 1 => 90, 2 => 160, _ => 230 };
            p.cause = format!("the gallery: {} on fitting ground", kind.word());
            p.description = "A demo place, so its glyph can be seen.".into();
            if matches!(kind, PlaceKind::Ridgeway | PlaceKind::Causeway) {
                let along = if kind == PlaceKind::Ridgeway { (1i64, 0i64) } else { (1, 1) };
                p.cells = (-6..=6).map(|j| ((x as i64) + j * along.0, (y as i64) + j * along.1)).collect();
            }
            places.push(p);
        }
        (land, places)
    }
}

/// The place inspector: a parchment card top-right for the place the walker went to (P): its
/// name and kind, its epithet, why it is there, what it holds, a warning where it is dangerous,
/// and that Enter embarks with it at the middle.
pub fn draw_place_card(p: &Place, buf: &mut [u32], w: usize, h: usize) {
    use super::ui::*;
    if w < 420 || h < 220 { return; }
    let cw = 360usize.min(w - 24);
    let chars = (cw - 28) / 7;
    let mut lines: Vec<(String, u32, bool)> = Vec::new();
    let title = if p.name.is_empty() { super::colony_hud::capitalize_pub(p.kind.word()) } else { p.name.clone() };
    lines.push((title, RUBRIC, true));
    let mut sub = p.kind.word().to_string();
    if !p.epithet.is_empty() { sub = format!("{}, {}", sub, p.epithet); }
    for l in wrap(&sub, chars) { lines.push((l, INK_FADED, false)); }
    lines.push((String::new(), INK, false));
    if !p.cause.is_empty() { for l in wrap(&format!("Why here: {}.", p.cause), chars) { lines.push((l, INK, false)); } }
    if !p.description.is_empty() && p.description != p.cause { for l in wrap(&p.description, chars).into_iter().take(4) { lines.push((l, INK, false)); } }
    if !p.contents.is_empty() { for l in wrap(&format!("Holds: {}.", p.contents.join(", ")), chars) { lines.push((l, INK, false)); } }
    if p.kind.is_dangerous() { lines.push(("Dangerous ground: go carefully.".into(), RUBRIC, false)); }
    lines.push((String::new(), INK, false));
    lines.push(("Enter embarks here; P the next place.".into(), INK_FADED, false));
    let r = Rect { x: w - cw - 12, y: 12, w: cw, h: 20 + lines.len() * 13 };
    card(buf, w, r);
    let mut y = r.y as i64 + 10;
    for (text, col, bold) in lines {
        if !text.is_empty() { super::text::draw_fell(buf, w, h, (r.x + 14) as i64, y, &text, col, 1, bold); }
        y += 13;
    }
}
