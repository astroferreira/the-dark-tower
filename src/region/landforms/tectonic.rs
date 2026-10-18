//! Faults at region scale (ROADMAP Stage 7; `crate::upheaval::Faults`): within a plate
//! boundary's zone (or a rift's) the ground is broken along the strike by the fault's kind:
//!
//! - **Normal** faults (the crust pulling apart): tilted blocks `NORMAL_SPACING_KM` apart, each
//!   rotated so one edge stands up as a scarp and the other sinks under the next block (a
//!   sawtooth across the strike); a block's tilt flips by hash, so where two blocks tilt away
//!   from each other a horst stands and where they tilt together a graben sinks.
//! - **Thrust** faults (the plates meeting): the same blocks set further apart, steeper, with a
//!   fold over the fault's upper edge.
//! - **Strike-slip** faults (sliding past): a straight trough along the trace a cell wide, with
//!   sag ponds where the trace steps apart and pressure ridges where it steps together; the
//!   rivers that meet it turn along it before they cross (offset streams).
//!
//! Throws taper to nothing at a segment's ends (`*_SEGMENT_KM`), and a segment is active by hash
//! with the zone's intensity, so faults die out and step over as real ones do. Shaped before the
//! rivers (they cut the scarps into facets) as a pure function of position.

use noise::{NoiseFn, Perlin};

use crate::erosion::materials::RockType;
use crate::region::plan::Landform;
use crate::upheaval::{FaultKind, Faults};

use super::{ChunkGrid, RegionFields};
use super::rock::{scale, smoothstep, unit};

const NORMAL_SPACING_KM: f32 = 18.0;
const THRUST_SPACING_KM: f32 = 20.0;
const STRIKE_SPACING_KM: f32 = 45.0;
const NORMAL_SEGMENT_KM: f32 = 28.0;
const THRUST_SEGMENT_KM: f32 = 40.0;
const STRIKE_SEGMENT_KM: f32 = 70.0;
/// Throw (m) at full intensity, before the rivers wear it.
const NORMAL_THROW_M: f32 = 750.0;
const THRUST_THROW_M: f32 = 520.0;
/// A strike-slip trough's depth (m) and its ponds' and ridges' heights.
const TROUGH_M: f32 = 14.0;
const POND_M: f32 = 12.0;
const RIDGE_M: f32 = 18.0;
/// Below this intensity the ground is not broken at region scale.
const MIN_INTENSITY: f32 = 0.08;

/// What the faults do at a point.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct FaultHit {
    pub kind: FaultKind,
    /// Height change (m).
    pub delta_m: f32,
    /// On a scarp's face, with the step's height (m).
    pub face_m: f32,
    /// In a strike-slip trough, a pond or on a pressure ridge.
    pub trough: bool,
    pub pond: bool,
    pub ridge: bool,
}

pub(crate) struct FaultField<'a> {
    faults: &'a Faults,
    noise: Perlin,
    s: usize,
    cell_km: f32,
    seed: u64,
}

impl<'a> FaultField<'a> {
    pub fn new(world: &'a crate::world::WorldData, s: usize, cell_m: f32, seed: u64) -> Option<FaultField<'a>> {
        world.upheaval()?;
        Some(FaultField { faults: world.faults(), noise: Perlin::new((seed ^ 0xFA17) as u32), s, cell_km: cell_m / 1000.0, seed })
    }

    fn block(&self, kind: FaultKind, b: i64, along: f32, seg_km: f32, intensity: f32) -> (f32, f32) {
        // The block's tilt amplitude (m) and its sign, tapered along the strike.
        let q = (along + b as f32 * 7.31) / seg_km;
        let seg = q.floor() as i64;
        let fs = q - seg as f32;
        let p = (intensity * 1.15).min(0.92);
        let active = unit(b, seg, self.seed ^ 0xB10C) < p;
        if !active { return (0.0, 1.0); }
        let throw = match kind { FaultKind::Thrust => THRUST_THROW_M, _ => NORMAL_THROW_M };
        let size = 0.45 + 0.55 * unit(b, seg, self.seed ^ 0xB10D);
        let taper = (std::f32::consts::PI * fs).sin().max(0.0).sqrt();
        let sign = if unit(b, 0, self.seed ^ 0xB10E) < 0.5 { 1.0 } else { -1.0 };
        (throw * intensity * size * taper, sign)
    }

    /// The faults at absolute region cell (ax, ay) (cell centres at k + 0.5).
    pub fn at(&self, ax: f64, ay: f64) -> FaultHit {
        let s = self.s as f32;
        let (u, v) = ((ax as f32 + 0.5) / s - 0.5, (ay as f32 + 0.5) / s - 0.5);
        let smp = self.faults.at(u, v);
        if smp.kind == FaultKind::None || smp.intensity < MIN_INTENSITY { return FaultHit::default(); }
        let intensity = smoothstep(MIN_INTENSITY, MIN_INTENSITY + 0.15, smp.intensity) * smp.intensity;
        let (x, y) = ((ax as f32 + 0.5) * self.cell_km, (ay as f32 + 0.5) * self.cell_km);
        let (c, sn) = (smp.strike.cos(), smp.strike.sin());
        let along = x * c + y * sn;
        let warp = 2.5 * self.noise.get([x as f64 / 30.0, y as f64 / 30.0]) as f32 + 1.0 * self.noise.get([x as f64 / 9.0 + 11.0, y as f64 / 9.0]) as f32;
        let across = -x * sn + y * c + warp;
        let mut hit = FaultHit { kind: smp.kind, ..Default::default() };
        match smp.kind {
            FaultKind::Normal | FaultKind::Thrust => {
                let (spacing, seg_km) = if smp.kind == FaultKind::Normal { (NORMAL_SPACING_KM, NORMAL_SEGMENT_KM) } else { (THRUST_SPACING_KM, THRUST_SEGMENT_KM) };
                let q = across / spacing;
                let b = q.floor() as i64;
                let fu = q - b as f32;
                let (a, sg) = self.block(smp.kind, b, along, seg_km, intensity);
                let mine = sg * a * (fu - 0.5);
                let (al, sl) = self.block(smp.kind, b - 1, along, seg_km, intensity);
                let left_end = sl * al * 0.5;
                // The face: a step a cell wide where this block meets the one before it.
                let eps = (0.9 * self.cell_km / spacing).min(0.3);
                let mut d = left_end + (mine - left_end) * smoothstep(0.0, eps, fu);
                let jump = (mine - left_end).abs();
                if fu < eps * 1.5 && jump > 8.0 { hit.face_m = jump; }
                if smp.kind == FaultKind::Thrust {
                    // A fold over the fault's upper edge.
                    let up = if sg * -0.5 * a > left_end { 1.0 } else { 0.0 };
                    d += up * 0.25 * a * (-fu / 0.12).exp();
                    if fu < 0.12 && up > 0.0 && a > 20.0 { hit.ridge = true; }
                }
                hit.delta_m = d;
            }
            FaultKind::StrikeSlip => {
                let q = across / STRIKE_SPACING_KM;
                let b = q.round() as i64;
                let dist = (q - b as f32).abs() * STRIKE_SPACING_KM;
                let seg_q = (along + b as f32 * 13.7) / STRIKE_SEGMENT_KM;
                let seg = seg_q.floor() as i64;
                let p = (intensity * 1.2).min(0.95);
                if unit(b, seg, self.seed ^ 0x551F) >= p { return hit; }
                let taper = (std::f32::consts::PI * (seg_q - seg as f32)).sin().max(0.0).sqrt();
                let w = (0.8 * self.cell_km).max(0.4);
                let core = (-(dist / w).powi(2)).exp();
                if core < 0.05 { return hit; }
                // Steps along the trace: ponds where it pulls apart, ridges where it pushes.
                let ph = (along / 7.0 + unit(b, seg, self.seed ^ 0x5520) * 10.0).sin();
                let step = if ph > 0.55 { -POND_M * (ph - 0.55) / 0.45 } else if ph < -0.65 { RIDGE_M * (-ph - 0.65) / 0.35 } else { 0.0 };
                hit.delta_m = (-TROUGH_M + step) * core * taper * intensity.sqrt();
                hit.trough = core > 0.35 && taper > 0.2;
                hit.pond = hit.trough && ph > 0.7;
                hit.ridge = hit.trough && ph < -0.75;
            }
            FaultKind::None => {}
        }
        hit
    }
}

pub fn shape_before(h: &mut [f32], g: &ChunkGrid) {
    let Some(field) = FaultField::new(g.world, g.s, g.cell_m, g.seed) else { return };
    let gw = g.gw;
    for j in 0..gw {
        for i in 0..gw {
            let c = j * gw + i;
            if h[c] <= 0.0 { continue; }
            let (ax, ay) = g.abs_cell(i, j);
            let hit = field.at(ax as f64, ay as f64);
            if hit.delta_m != 0.0 { h[c] = (h[c] + hit.delta_m).max(0.5); }
        }
    }
}

pub fn classify(f: &RegionFields, _rock: &[RockType], land: &mut [Landform]) {
    let Some(field) = FaultField::new(f.world, f.s, f.cell_m, f.seed) else { return };
    let k_scale = scale(f.cell_m);
    for j in 0..f.gh {
        for i in 0..f.gw {
            let k = f.idx(i, j);
            if f.is_sea(k) { continue; }
            let (ax, ay) = f.abs_cell(i, j);
            let hit = field.at(ax as f64, ay as f64);
            if hit.kind == FaultKind::None { continue; }
            let slope = f.slope(i, j);
            let l = if hit.face_m > 0.0 && slope > 0.05 * k_scale { Landform::FaultScarp }
                else if hit.pond && f.lake_depth(k) > 0.0 { Landform::SagPond }
                else if hit.ridge { Landform::PressureRidge }
                else if hit.trough { Landform::FaultTrough }
                else { continue };
            // Water, cliffs and the glaciers keep their own labels.
            if matches!(land[k], Landform::Glacier | Landform::Waterfall | Landform::Gorge) { continue; }
            land[k] = l;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::upheaval::Upheaval;

    #[test]
    fn faults_break_the_ground_the_same_way_in_every_chunk() {
        let world = crate::world::generate_world_with_style(64, 32, 5, crate::plates::WorldStyle::Earthlike);
        world.set_upheaval(Some(Upheaval::earth()));
        let faults = world.faults();
        // A boundary tile with a strong zone.
        let Some(t) = (0..64 * 32).filter(|&k| faults.kind[k] != FaultKind::None && *world.heightmap.get(k % 64, k / 64) > 100.0)
            .max_by(|&a, &b| faults.intensity[a].partial_cmp(&faults.intensity[b]).unwrap()) else { return };
        let (tx, ty) = ((t % 64) as i64, (t / 64) as i64);
        let s = 32usize;
        let gw = 3 * s;
        let make = |gx0: i64| {
            let g = ChunkGrid { world: &world, gw, s, gx0, gy0: ty - 1, cell_m: 1200.0, seed: 3 };
            let mut h = vec![400.0f32; gw * gw];
            shape_before(&mut h, &g);
            (g, h)
        };
        let (ga, ha) = make(tx - 1);
        let (gb, hb) = make(tx);
        let mut moved = 0;
        for j in 0..gw {
            for i in s..gw {
                assert_eq!(ga.abs_cell(i, j), gb.abs_cell(i - s, j));
                assert_eq!(ha[j * gw + i].to_bits(), hb[j * gw + i - s].to_bits());
                if (ha[j * gw + i] - 400.0).abs() > 5.0 { moved += 1; }
            }
        }
        assert!(moved > 0, "the fault zone moved no ground");
    }
}
