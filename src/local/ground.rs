//! The embark's ground from the landform classes (ROADMAP Stage 6): each column reads the
//! region's landform at its cell (through a jitter so cell borders blend, like the biome) and
//! the class says what the ground is at 2 m: bare rock on cliffs, outcrops and pavements,
//! scree and moraine of gravel and boulders, sand on dunes and beaches with the dunes' waves,
//! clay on playas and badlands with their rills, lava's black rock, ash, the bog's peat with
//! its pools, the braided river's gravel bars, the floodplain's deep soil, and what grows on
//! each. The rule changes material, soil depth, plants, boulders, relief and standing water;
//! the heights themselves come from the region (the landform passes shaped them there).

use super::*;
use crate::region::plan::Landform;

/// What a landform asks of the ground.
#[derive(Clone, Copy, Debug)]
pub(super) struct GroundRule {
    /// The floor's material, where the class sets one.
    pub floor: Option<Material>,
    /// Soil levels at most (0: bare rock or the floor material alone).
    pub soil_max: Option<i32>,
    /// Trees and shrubs scaled; whether grass grows.
    pub trees: f32,
    pub shrubs: f32,
    pub grass: bool,
    /// Boulders' share of the floor added.
    pub boulders: f32,
    /// Metre-scale relief the class adds.
    pub relief: Relief,
    /// Standing water in the hollows: the water line this far over the smooth ground (m).
    pub pools_m: Option<f32>,
    /// The trees that stand are dead (a burn).
    pub dead: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Relief {
    None,
    /// Dune waves: amplitude (m), wavelength (m).
    Dunes(f32, f32),
    /// Broken ground: amplitude (m).
    Rough(f32),
    /// Rills down the slope: depth (m).
    Rills(f32),
    /// Slots across the slope (crevasses in ice, fissures in fresh lava): depth (m), spacing (m).
    Slots(f32, f32),
}

const NONE: GroundRule = GroundRule { dead: false, floor: Option::None, soil_max: Option::None, trees: 1.0, shrubs: 1.0, grass: true, boulders: 0.0, relief: Relief::None, pools_m: Option::None };

/// The rule of a class, or none where the heights alone carry it.
pub(super) fn rule(land: Landform, rock: RockType) -> Option<GroundRule> {
    use Landform::*;
    let bare_rock = GroundRule { dead: false, floor: Some(Material::Rock(rock)), soil_max: Some(0), trees: 0.0, shrubs: 0.3, grass: false, boulders: 0.05, relief: Relief::Rough(1.2), pools_m: Option::None };
    Some(match land {
        Cliff | SeaCliff | Arete | Outcrop | ColumnarBasalt | Hoodoo | Tor | Stack | KarstGorge => bare_rock,
        Mesa => GroundRule { trees: 0.3, shrubs: 0.6, boulders: 0.03, relief: Relief::Rough(0.4), ..bare_rock },
        Scree => GroundRule { dead: false, floor: Some(Material::Gravel), soil_max: Some(1), trees: 0.1, shrubs: 0.4, grass: false, boulders: 0.25, relief: Relief::Rough(1.0), pools_m: Option::None },
        Fan | Outwash | RaisedBeach => GroundRule { dead: false, floor: Some(Material::Gravel), soil_max: Some(1), trees: 0.5, shrubs: 0.7, grass: true, boulders: 0.04, relief: Relief::None, pools_m: Option::None },
        Moraine | Drumlin => GroundRule { dead: false, floor: Some(Material::Gravel), soil_max: Some(2), trees: 0.6, shrubs: 0.8, grass: true, boulders: 0.12, relief: Relief::Rough(0.8), pools_m: Option::None },
        Erratic => GroundRule { boulders: 0.3, ..NONE },
        Pavement | DesertPavement => GroundRule { dead: false, floor: Some(if land == Pavement { Material::Rock(RockType::Limestone) } else { Material::Gravel }), soil_max: Some(0), trees: 0.05, shrubs: 0.2, grass: false, boulders: 0.02, relief: Relief::Rough(0.6), pools_m: Option::None },
        PatternedGround => GroundRule { dead: false, floor: Some(Material::Gravel), soil_max: Some(1), trees: 0.0, shrubs: 0.2, grass: true, boulders: 0.06, relief: Relief::Rough(0.5), pools_m: Option::None },
        Dune | SandSea => GroundRule { dead: false, floor: Some(Material::Sand), soil_max: Some(0), trees: 0.0, shrubs: 0.1, grass: false, boulders: 0.0, relief: Relief::Dunes(if land == SandSea { 3.5 } else { 2.5 }, 60.0), pools_m: Option::None },
        CoastalDune => GroundRule { dead: false, floor: Some(Material::Sand), soil_max: Some(0), trees: 0.05, shrubs: 0.4, grass: true, boulders: 0.0, relief: Relief::Dunes(2.0, 40.0), pools_m: Option::None },
        Yardang => GroundRule { dead: false, floor: Some(Material::Sand), soil_max: Some(0), trees: 0.0, shrubs: 0.1, grass: false, boulders: 0.01, relief: Relief::Rills(2.0), pools_m: Option::None },
        Badland => GroundRule { dead: false, floor: Some(Material::Clay), soil_max: Some(0), trees: 0.0, shrubs: 0.1, grass: false, boulders: 0.0, relief: Relief::Rills(2.5), pools_m: Option::None },
        Playa => GroundRule { dead: false, floor: Some(Material::Clay), soil_max: Some(0), trees: 0.0, shrubs: 0.0, grass: false, boulders: 0.0, relief: Relief::None, pools_m: Option::None },
        LavaField => GroundRule { dead: false, floor: Some(Material::Rock(RockType::Basalt)), soil_max: Some(0), trees: 0.1, shrubs: 0.3, grass: false, boulders: 0.08, relief: Relief::Rough(1.0), pools_m: Option::None },
        AshPlain => GroundRule { dead: false, floor: Some(Material::Sand), soil_max: Some(1), trees: 0.3, shrubs: 0.5, grass: true, boulders: 0.0, relief: Relief::None, pools_m: Option::None },
        Fumarole | ThermalGround => GroundRule { dead: false, floor: Some(Material::Gravel), soil_max: Some(0), trees: 0.0, shrubs: 0.1, grass: false, boulders: 0.03, relief: Relief::Rough(0.4), pools_m: Option::None },
        Crater | Caldera | CinderCone => GroundRule { dead: false, floor: Some(Material::Gravel), soil_max: Some(1), trees: 0.2, shrubs: 0.4, grass: true, boulders: 0.04, relief: Relief::Rough(0.6), pools_m: Option::None },
        Beach | Spit | Bar | BarrierIsland => GroundRule { dead: false, floor: Some(Material::Sand), soil_max: Some(0), trees: 0.0, shrubs: 0.05, grass: false, boulders: 0.0, relief: Relief::None, pools_m: Option::None },
        TidalFlat => GroundRule { dead: false, floor: Some(Material::Clay), soil_max: Some(0), trees: 0.0, shrubs: 0.0, grass: false, boulders: 0.0, relief: Relief::None, pools_m: Some(0.2) },
        SaltMarsh => GroundRule { dead: false, floor: Some(Material::Clay), soil_max: Some(1), trees: 0.0, shrubs: 0.3, grass: true, boulders: 0.0, relief: Relief::None, pools_m: Some(0.25) },
        Mangrove => GroundRule { dead: false, floor: Some(Material::Clay), soil_max: Some(1), trees: 1.2, shrubs: 0.5, grass: false, boulders: 0.0, relief: Relief::None, pools_m: Some(0.3) },
        Lagoon | Oxbow | Tarn | Kettle | Cenote | Delta | Estuary | ReedBed => NONE,
        Floodplain | Terrace => GroundRule { floor: Some(Material::Soil), trees: 0.8, ..NONE },
        Levee => GroundRule { floor: Some(Material::Soil), trees: 1.3, ..NONE },
        Backswamp => GroundRule { floor: Some(Material::Clay), trees: 0.5, shrubs: 0.8, pools_m: Some(0.3), ..NONE },
        Braided => GroundRule { dead: false, floor: Some(Material::Gravel), soil_max: Some(0), trees: 0.0, shrubs: 0.1, grass: false, boulders: 0.02, relief: Relief::Rough(0.5), pools_m: Some(0.15) },
        Bog => GroundRule { dead: false, floor: Some(Material::Clay), soil_max: Some(2), trees: 0.1, shrubs: 0.6, grass: true, boulders: 0.0, relief: Relief::None, pools_m: Some(0.3) },
        Fen | Marsh => GroundRule { dead: false, floor: Some(Material::Clay), soil_max: Some(2), trees: 0.1, shrubs: 0.3, grass: true, boulders: 0.0, relief: Relief::None, pools_m: Some(0.35) },
        Glacier | Snowfield => GroundRule { dead: false, floor: Some(Material::Snow), soil_max: Some(0), trees: 0.0, shrubs: 0.0, grass: false, boulders: 0.02, relief: Relief::Rough(0.8), pools_m: Option::None },
        Cirque | UValley => GroundRule { trees: 0.6, boulders: 0.03, ..NONE },
        TowerKarst => GroundRule { trees: 0.7, boulders: 0.04, ..NONE },
        // Upheaval (ROADMAP Stage 7).
        ImpactRim | CentralPeak | FaultScarp | CollapseScar => GroundRule { boulders: 0.12, relief: Relief::Rough(1.6), ..bare_rock },
        ImpactCrater => GroundRule { dead: false, floor: Some(Material::Gravel), soil_max: Some(1), trees: 0.5, shrubs: 0.6, grass: true, boulders: 0.06, relief: Relief::Rough(0.6), pools_m: Option::None },
        Ejecta => GroundRule { dead: false, floor: Some(Material::Gravel), soil_max: Some(1), trees: 0.6, shrubs: 0.8, grass: true, boulders: 0.09, relief: Relief::Rough(0.9), pools_m: Option::None },
        FaultTrough => GroundRule { floor: Some(Material::Clay), trees: 0.6, shrubs: 1.1, pools_m: Some(0.25), ..NONE },
        PressureRidge => GroundRule { soil_max: Some(1), boulders: 0.05, relief: Relief::Rough(1.0), ..NONE },
        LandslideScar => GroundRule { dead: false, floor: Some(Material::Rock(rock)), soil_max: Some(0), trees: 0.0, shrubs: 0.1, grass: false, boulders: 0.15, relief: Relief::Rough(1.5), pools_m: Option::None },
        LandslideDebris => GroundRule { dead: false, floor: Some(Material::Gravel), soil_max: Some(1), trees: 0.15, shrubs: 0.3, grass: true, boulders: 0.3, relief: Relief::Rough(2.0), pools_m: Some(0.1) },
        RockAvalanche => GroundRule { dead: false, floor: Some(Material::Gravel), soil_max: Some(0), trees: 0.0, shrubs: 0.1, grass: false, boulders: 0.45, relief: Relief::Rough(2.5), pools_m: Option::None },
        BurnScar => GroundRule { trees: 0.5, shrubs: 0.5, grass: true, boulders: 0.0, dead: true, ..NONE },
        Maar => GroundRule { dead: false, floor: Some(Material::Sand), soil_max: Some(1), trees: 0.4, shrubs: 0.6, grass: true, boulders: 0.02, relief: Relief::None, pools_m: Option::None },
        FissureVents => GroundRule { dead: false, floor: Some(Material::Rock(RockType::Basalt)), soil_max: Some(0), trees: 0.0, shrubs: 0.1, grass: false, boulders: 0.1, relief: Relief::Slots(2.0, 14.0), pools_m: Option::None },
        Lahar => GroundRule { dead: false, floor: Some(Material::Sand), soil_max: Some(0), trees: 0.1, shrubs: 0.2, grass: false, boulders: 0.08, relief: Relief::None, pools_m: Option::None },
        Traps => GroundRule { soil_max: Some(1), boulders: 0.04, relief: Relief::Rough(0.6), ..NONE },
        SaltSink => GroundRule { dead: false, floor: Some(Material::Clay), soil_max: Some(0), trees: 0.0, shrubs: 0.0, grass: false, boulders: 0.0, relief: Relief::Rough(0.4), pools_m: Some(0.15) },
        ThawLake => GroundRule { dead: false, floor: Some(Material::Clay), soil_max: Some(1), trees: 0.2, shrubs: 0.5, grass: true, boulders: 0.0, relief: Relief::None, pools_m: Some(0.3) },
        Crevasses => GroundRule { dead: false, floor: Some(Material::Ice), soil_max: Some(0), trees: 0.0, shrubs: 0.0, grass: false, boulders: 0.01, relief: Relief::Slots(5.0, 18.0), pools_m: Option::None },
        AvalanchePath => GroundRule { trees: 0.0, shrubs: 0.35, grass: true, boulders: 0.06, relief: Relief::Rough(0.5), ..NONE },
        Scabland => GroundRule { dead: false, floor: Some(Material::Gravel), soil_max: Some(0), trees: 0.05, shrubs: 0.3, grass: true, boulders: 0.15, relief: Relief::Dunes(2.5, 110.0), pools_m: Some(0.1) },
        AbandonedChannel => GroundRule { floor: Some(Material::Clay), trees: 0.6, shrubs: 1.2, pools_m: Some(0.2), ..NONE },
        SlotCanyon | Gorge | Waterfall | Rapids | DryValley | Doline | Sink | Resurgence | SagPond | None => return Option::None,
    })
}

/// The height the class adds at an absolute position (m): the dunes' waves across the wind,
/// broken ground, rills down the slope.
pub(super) fn relief_at(r: Relief, mx: f64, my: f64, wind: (f64, f64), slope_dir: (f64, f64), noise: &Perlin, span_m: f64) -> f32 {
    match r {
        Relief::None => 0.0,
        Relief::Dunes(amp, wave) => {
            // Crests across the wind, bent by noise; a slip face: the wave is sharper on the lee.
            let along = mx * wind.0 + my * wind.1;
            let bend = across_date_line(mx, span_m, |x| fbm(noise, x / 220.0, my / 220.0, 2)) as f64 * 25.0;
            let ph = ((along + bend) / wave as f64).rem_euclid(1.0) as f32;
            let crest = if ph < 0.65 { ph / 0.65 } else { 1.0 - (ph - 0.65) / 0.35 };
            amp * (crest - 0.5)
        }
        Relief::Rough(amp) => amp * across_date_line(mx, span_m, |x| fbm(noise, x / 11.0, my / 11.0, 3)),
        Relief::Rills(depth) => {
            // Rills run down the slope: grooves across it, a few metres apart.
            let across = mx * -slope_dir.1 + my * slope_dir.0;
            let bend = across_date_line(mx, span_m, |x| fbm(noise, x / 60.0, my / 60.0, 2)) as f64 * 6.0;
            let ph = ((across + bend) / 9.0).rem_euclid(1.0) as f32;
            -depth * (1.0 - (2.0 * ph - 1.0).abs()).powi(2)
        }
        Relief::Slots(depth, spacing) => {
            // Narrow cracks across the slope, broken along their length.
            let across = mx * -slope_dir.1 + my * slope_dir.0;
            let along = mx * slope_dir.0 + my * slope_dir.1;
            let bend = across_date_line(mx, span_m, |x| fbm(noise, x / 40.0, my / 40.0, 2)) as f64 * 5.0;
            let ph = ((along + bend) / spacing as f64).rem_euclid(1.0) as f32;
            let broken = across_date_line(mx, span_m, |x| fbm(noise, x / 23.0 + across / 31.0, my / 23.0, 2)) > -0.1;
            if ph < 0.12 && broken { -depth } else { 0.0 }
        }
    }
}

/// The region's landform under a column, read through a smooth warp of up to a third of a
/// cell (noise over absolute metres), so class borders wander instead of following the cells.
pub(super) fn landform_at(region: &ZoomRegion, zx: f64, zy: f64, mx: f64, my: f64, warp: &Perlin, span_m: f64) -> (Landform, RockType) {
    let (rw, rh) = (region.width as i64, region.height as i64);
    let jx = across_date_line(mx, span_m, |x| fbm(warp, x / 45.0, my / 45.0, 2)) as f64 * 0.35;
    let jy = across_date_line(mx, span_m, |x| fbm(warp, x / 45.0 + 17.0, my / 45.0 + 9.0, 2)) as f64 * 0.35;
    let (x, y) = (((zx + jx).floor() as i64).clamp(0, rw - 1), ((zy + jy).floor() as i64).clamp(0, rh - 1));
    let k = (y * rw + x) as usize;
    (region.landform.get(k).copied().unwrap_or(Landform::None), region.rock.get(k).copied().unwrap_or(RockType::Granite))
}

/// The prevailing wind at a latitude (unit vector in region cells: x east, y south), from the
/// landforms' belts (`landforms::volcanic::wind_dir`).
pub(super) fn wind_at(lat_deg: f32) -> (f64, f64) {
    let d = crate::region::landforms::volcanic::wind_dir(lat_deg);
    (d.0 as f64, d.1 as f64)
}
