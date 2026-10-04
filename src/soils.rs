//! Soils: what the land is made of at the surface, derived from where a tile sits in the
//! landscape (slope, floodplains, closed basins, volcanoes) and from its climate (weathering
//! and leaching). Computed on first use (`WorldData::soils`) and never serialized.
//!
//! Depth follows the catena: thin on steep ground, deep where the landscape step and the rivers
//! laid sediment down (floodplains, lake and basin floors), deeper where warm, wet weather rots
//! rock fast. The kind follows the classic soil orders, simplified, and sets fertility: black
//! earth under steppe grass and young alluvium are the best farmland, leached tropical laterite
//! and acid taiga podzol are poor however green the land looks.

use crate::biomes::ExtendedBiome;
use crate::tilemap::Tilemap;
use crate::world::WorldData;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SoilKind {
    /// No soil: sea, lakes, ice sheets.
    None,
    /// Young river and delta sediment on floodplains: deep, rich.
    Alluvium,
    /// Chernozem: deep black earth under temperate grassland.
    BlackEarth,
    /// Weathered volcanic ash: rich.
    Andosol,
    /// Temperate forest soil: fair.
    BrownEarth,
    /// Red clay of the winter-rain lands.
    TerraRossa,
    /// Deep but leached tropical clay: poor.
    Laterite,
    /// Acid, ash-grey taiga soil: poor.
    Podzol,
    /// Waterlogged organic soil of bogs and cold flats.
    Peat,
    /// Thin, sandy or salty soil of dry lands.
    DesertSoil,
    /// Thin stony soil on steep ground and mountains.
    Rocky,
    /// Frozen ground under tundra.
    Permafrost,
}

impl SoilKind {
    pub fn name(self) -> &'static str {
        match self {
            SoilKind::None => "no soil",
            SoilKind::Alluvium => "alluvial soil",
            SoilKind::BlackEarth => "black earth",
            SoilKind::Andosol => "volcanic soil",
            SoilKind::BrownEarth => "brown forest soil",
            SoilKind::TerraRossa => "red clay soil",
            SoilKind::Laterite => "leached red laterite",
            SoilKind::Podzol => "acid grey podzol",
            SoilKind::Peat => "peat",
            SoilKind::DesertSoil => "thin desert soil",
            SoilKind::Rocky => "thin stony soil",
            SoilKind::Permafrost => "frozen ground",
        }
    }

    /// Natural fertility for farming, 0..1.
    pub fn fertility(self) -> f32 {
        match self {
            SoilKind::None => 0.0,
            SoilKind::BlackEarth => 1.0,
            SoilKind::Alluvium => 0.95,
            SoilKind::Andosol => 0.9,
            SoilKind::BrownEarth => 0.7,
            SoilKind::TerraRossa => 0.6,
            SoilKind::Laterite => 0.35,
            SoilKind::Podzol => 0.3,
            SoilKind::Peat => 0.25,
            SoilKind::DesertSoil => 0.15,
            SoilKind::Rocky => 0.1,
            SoilKind::Permafrost => 0.05,
        }
    }
}

pub struct SoilMap {
    pub kind: Tilemap<SoilKind>,
    /// Soil depth over bedrock (m).
    pub depth_m: Tilemap<f32>,
}

impl SoilMap {
    /// "deep alluvial soil (6 m)".
    pub fn describe(&self, x: usize, y: usize) -> Option<String> {
        let k = *self.kind.get(x, y);
        if k == SoilKind::None { return None; }
        let d = *self.depth_m.get(x, y);
        let depth = if d >= 4.0 { "deep " } else if d < 1.0 { "shallow " } else { "" };
        Some(format!("{}{} ({:.0} m)", depth, k.name(), d.max(0.5)))
    }
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn compute_soils(world: &WorldData) -> SoilMap {
    use ExtendedBiome as B;
    let (w, h) = (world.width, world.height);
    let hm = &world.heightmap;
    let mut kind = Tilemap::new_with(w, h, SoilKind::None);
    let mut depth = Tilemap::new_with(w, h, 0.0f32);
    let river_threshold = crate::water_bodies::river_flow_threshold(w);
    let volcano_near = |x: usize, y: usize| world.volcanoes.iter().any(|v| {
        let dx = (v.x as i64 - x as i64).abs();
        let dx = dx.min(w as i64 - dx);
        let dy = (v.y as i64 - y as i64).abs();
        dx * dx + dy * dy <= 9
    });
    // Ground below the spill level of a closed hollow: lake beds and basin floors fill with
    // sediment.
    let hollow = crate::erosion::landscape::lake_depth(hm);

    for y in 0..h {
        for x in 0..w {
            let e = *hm.get(x, y);
            let water = world.water_body_map.get(x, y);
            if e <= 0.0 || water.is_lake() || water.is_ocean() { continue; }
            let biome = *world.biomes.get(x, y);
            if matches!(biome, B::Ice) { continue; }
            let t = *world.temperature.get(x, y);
            let m = *world.moisture.get(x, y);

            // Relief around the tile (m): steep ground keeps little soil.
            let mut lo = e;
            let mut hi = e;
            for (nx, ny) in hm.neighbors_8(x, y) {
                let v = (*hm.get(nx, ny)).max(0.0);
                lo = lo.min(v);
                hi = hi.max(v);
            }
            let relief = hi - lo;
            let steep = smoothstep(500.0, 1800.0, relief);
            let river = world.flow_accumulation.as_ref().map(|f| *f.get(x, y) >= 0.5 * river_threshold).unwrap_or(false)
                || water.is_river();
            let floodplain = river && relief < 400.0 && e < 1200.0;
            let basin = *hollow.get(x, y) > 0.5;

            // Weathering: warm, wet climates rot rock deep; cold or dry ones barely at all.
            let weathering = smoothstep(-5.0, 25.0, t) * smoothstep(0.08, 0.6, m);
            let mut d = 0.4 + 2.6 * weathering;
            if floodplain || basin { d += 4.0; }
            d *= 1.0 - 0.85 * steep;
            if e > 3000.0 { d *= 0.4; }

            let volcanic = volcano_near(x, y) || matches!(biome, B::VolcanicWasteland | B::Ashlands | B::LavaField | B::ShieldVolcano);
            let k = if volcanic && e < 3000.0 {
                // Ash weathers into rich soil even on a volcano's flanks.
                SoilKind::Andosol
            } else if steep > 0.6 || matches!(biome, B::SnowyPeaks | B::AlpineTundra) {
                SoilKind::Rocky
            } else if matches!(biome, B::Tundra | B::AuroraWastes) || t < -5.0 {
                if m > 0.5 && relief < 200.0 { SoilKind::Peat } else { SoilKind::Permafrost }
            } else if matches!(biome, B::Swamp | B::Marsh | B::Bog) {
                SoilKind::Peat
            } else if floodplain {
                SoilKind::Alluvium
            } else if matches!(biome, B::Desert | B::SaltFlats | B::SingingDunes | B::GlassDesert) || m < 0.12 {
                SoilKind::DesertSoil
            } else if matches!(biome, B::MediterraneanShrubland) {
                SoilKind::TerraRossa
            } else if t >= 20.0 && m > 0.45 {
                SoilKind::Laterite
            } else if matches!(biome, B::BorealForest | B::SubalpineForest) || (t < 4.0 && m > 0.33) {
                SoilKind::Podzol
            } else if matches!(biome, B::TemperateGrassland | B::Savanna | B::AlpineMeadow) && m < 0.45 {
                SoilKind::BlackEarth
            } else {
                SoilKind::BrownEarth
            };
            if basin && k == SoilKind::DesertSoil { d = d.max(2.0); }
            kind.set(x, y, k);
            depth.set(x, y, d.clamp(0.1, 12.0));
        }
    }
    SoilMap { kind, depth_m: depth }
}
