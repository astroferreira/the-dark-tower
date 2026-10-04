//! Advanced map export with LUT-based coloring, dithering, and multiple output modes
//!
//! Implements professional map export strategies:
//! - Color families for 100+ biomes (grouped by climate/type)
//! - Temperature/Moisture LUT for smooth transitions
//! - Border dithering to eliminate hard edges
//! - Multiple export modes: visual, data, legend

use crate::biomes::ExtendedBiome;
use crate::tilemap::Tilemap;
use crate::world::WorldData;
use image::{ImageBuffer, Rgb, RgbImage};
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;

/// Biome color family for grouping similar biomes
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BiomeFamily {
    DeepOcean,
    Ocean,
    CoastalWater,
    Polar,
    Boreal,
    TemperateDry,
    TemperateWet,
    Tropical,
    Arid,
    Mountain,
    Wetland,
    Volcanic,
    Fantasy,
    Ruins,
}

impl BiomeFamily {
    /// Get the base HSV color for this family
    /// Returns (hue 0-360, saturation 0-1, value 0-1)
    fn base_hsv(&self) -> (f32, f32, f32) {
        match self {
            BiomeFamily::DeepOcean => (220.0, 0.8, 0.25),      // Deep blue
            BiomeFamily::Ocean => (210.0, 0.7, 0.45),          // Ocean blue
            BiomeFamily::CoastalWater => (195.0, 0.5, 0.6),    // Light coastal blue
            BiomeFamily::Polar => (200.0, 0.1, 0.95),          // Near white with blue tint
            BiomeFamily::Boreal => (140.0, 0.5, 0.35),         // Dark green
            BiomeFamily::TemperateDry => (80.0, 0.4, 0.55),    // Yellow-green
            BiomeFamily::TemperateWet => (120.0, 0.6, 0.4),    // Rich green
            BiomeFamily::Tropical => (130.0, 0.7, 0.35),       // Deep tropical green
            BiomeFamily::Arid => (35.0, 0.5, 0.7),             // Sandy tan
            BiomeFamily::Mountain => (30.0, 0.15, 0.5),        // Gray-brown
            BiomeFamily::Wetland => (150.0, 0.4, 0.35),        // Murky green
            BiomeFamily::Volcanic => (15.0, 0.7, 0.3),         // Dark red-brown
            BiomeFamily::Fantasy => (280.0, 0.5, 0.5),         // Purple
            BiomeFamily::Ruins => (40.0, 0.2, 0.4),            // Weathered stone
        }
    }
}

/// Get the biome family and variation parameters for a biome
pub(crate) fn get_biome_family(biome: ExtendedBiome) -> (BiomeFamily, f32, f32) {
    use ExtendedBiome::*;

    // Returns (family, saturation_modifier, value_modifier)
    // Modifiers range from -0.3 to +0.3 for subtle variation within family
    match biome {
        // Ocean family
        DeepOcean | OceanicTrench | AbyssalPlain | VoidMaw =>
            (BiomeFamily::DeepOcean, 0.0, 0.0),
        Ocean | ContinentalShelf | MidOceanRidge =>
            (BiomeFamily::Ocean, 0.0, 0.0),
        CoastalWater | Lagoon | SeagrassMeadow =>
            (BiomeFamily::CoastalWater, 0.0, 0.0),
        CoralReef => (BiomeFamily::CoastalWater, 0.2, 0.15),
        KelpForest | KelpTowers => (BiomeFamily::CoastalWater, 0.3, -0.2),

        // Polar family
        Ice | SnowyPeaks | FrozenLake | FrozenAbyss =>
            (BiomeFamily::Polar, 0.0, 0.0),
        Tundra | AlpineTundra => (BiomeFamily::Polar, 0.1, -0.15),

        // Boreal family
        BorealForest => (BiomeFamily::Boreal, 0.0, 0.0),
        SubalpineForest => (BiomeFamily::Boreal, 0.05, -0.1),

        // Temperate dry family
        TemperateGrassland => (BiomeFamily::TemperateDry, 0.0, 0.0),
        MediterraneanShrubland => (BiomeFamily::TemperateDry, 0.1, -0.1),
        MonsoonForest => (BiomeFamily::Tropical, -0.1, 0.05),
        AlpineMeadow => (BiomeFamily::TemperateDry, 0.1, 0.1),
        Foothills => (BiomeFamily::TemperateDry, -0.1, -0.05),
        Savanna => (BiomeFamily::TemperateDry, 0.1, -0.1),

        // Temperate wet family
        TemperateForest => (BiomeFamily::TemperateWet, 0.0, 0.0),
        TemperateRainforest => (BiomeFamily::TemperateWet, 0.15, -0.1),
        MontaneForest => (BiomeFamily::TemperateWet, 0.1, -0.05),
        CloudForest => (BiomeFamily::TemperateWet, -0.1, 0.1),

        // Tropical family
        TropicalForest => (BiomeFamily::Tropical, 0.0, 0.0),
        TropicalRainforest => (BiomeFamily::Tropical, 0.1, -0.1),
        Paramo => (BiomeFamily::Tropical, -0.3, 0.2),

        // Arid family
        Desert => (BiomeFamily::Arid, 0.0, 0.0),
        SaltFlats => (BiomeFamily::Arid, -0.4, 0.3),
        SingingDunes => (BiomeFamily::Arid, 0.1, 0.05),
        GlassDesert => (BiomeFamily::Arid, -0.3, 0.2),
        Oasis => (BiomeFamily::Tropical, 0.2, 0.1),

        // Mountain family
        RazorPeaks => (BiomeFamily::Mountain, 0.0, 0.0),
        BasaltColumns => (BiomeFamily::Mountain, 0.1, -0.2),
        PaintedHills => (BiomeFamily::Mountain, 0.3, 0.2),
        KarstPlains | TowerKarst | CockpitKarst => (BiomeFamily::Mountain, -0.1, 0.15),
        Sinkhole | SinkholeLakes | Cenote => (BiomeFamily::Mountain, 0.1, -0.15),
        CaveEntrance => (BiomeFamily::Mountain, 0.0, -0.3),
        HighlandLake | CraterLake => (BiomeFamily::Ocean, 0.1, 0.1),

        // Wetland family
        Swamp => (BiomeFamily::Wetland, 0.0, 0.0),
        Marsh => (BiomeFamily::Wetland, -0.1, 0.1),
        Bog | CarnivorousBog => (BiomeFamily::Wetland, 0.1, -0.15),
        MangroveSaltmarsh => (BiomeFamily::Wetland, 0.05, 0.05),
        Sargasso => (BiomeFamily::Wetland, 0.15, 0.0),

        // Volcanic family
        VolcanicWasteland | Ashlands | LavaField => (BiomeFamily::Volcanic, 0.0, 0.0),
        Caldera | VolcanicCone | ShieldVolcano => (BiomeFamily::Volcanic, -0.1, 0.1),
        LavaLake | HotSpot | ThermalVents => (BiomeFamily::Volcanic, 0.3, 0.3),
        FumaroleField | SulfurVents => (BiomeFamily::Volcanic, 0.2, 0.4),
        ObsidianFields => (BiomeFamily::Volcanic, 0.1, -0.2),
        VolcanicBeach => (BiomeFamily::Volcanic, -0.2, 0.0),
        Geysers | HotSprings => (BiomeFamily::CoastalWater, 0.2, 0.2),

        // Fantasy family (magical biomes)
        CrystalForest | CrystalWasteland | CrystalDepths =>
            (BiomeFamily::Fantasy, 0.0, 0.3),
        BioluminescentForest | BioluminescentWater | PhosphorShallows =>
            (BiomeFamily::Fantasy, 0.2, 0.2),
        MushroomForest | FungalBloom => (BiomeFamily::Fantasy, 0.15, 0.0),
        EtherealMist | SpiritMarsh => (BiomeFamily::Fantasy, -0.3, 0.3),
        LeyNexus | PrismaticPools | AuroraWastes => (BiomeFamily::Fantasy, 0.1, 0.4),
        VoidScar | Shadowfen => (BiomeFamily::Fantasy, 0.2, -0.3),
        StarfallCrater => (BiomeFamily::Fantasy, 0.0, -0.1),
        WhisperingStones | FloatingStones => (BiomeFamily::Fantasy, -0.2, 0.1),
        SiliconGrove => (BiomeFamily::Fantasy, -0.3, 0.35),
        SporeWastes => (BiomeFamily::Fantasy, -0.1, -0.1),
        BleedingStone => (BiomeFamily::Volcanic, 0.2, 0.1),
        HollowEarth => (BiomeFamily::Mountain, 0.0, -0.25),

        // Special biomes
        DeadForest | PetrifiedForest => (BiomeFamily::Boreal, -0.4, 0.1),
        AcidLake => (BiomeFamily::Fantasy, 0.3, 0.3),
        AncientGrove => (BiomeFamily::Tropical, 0.0, -0.2),
        TitanBones | BoneFields => (BiomeFamily::Polar, -0.05, -0.1),
        CoralPlateau => (BiomeFamily::CoastalWater, 0.3, 0.3),
        TarPits | InkSea => (BiomeFamily::Volcanic, 0.0, -0.25),
        ColossalHive => (BiomeFamily::Arid, 0.2, -0.1),
        BrinePools | BrinePool | ColdSeep => (BiomeFamily::Ocean, -0.1, -0.1),
        MirrorLake => (BiomeFamily::Polar, 0.0, 0.0),

        // Ruins family
        SunkenCity | DrownedCitadel => (BiomeFamily::Ruins, 0.1, -0.1),
        CyclopeanRuins | BuriedTemple => (BiomeFamily::Ruins, 0.0, 0.0),
        OvergrownCitadel => (BiomeFamily::Ruins, 0.2, 0.1),
        DarkTower => (BiomeFamily::Ruins, 0.1, -0.3),

        // Ocean special
        Seamount => (BiomeFamily::Volcanic, -0.2, -0.1),
        LeviathanGraveyard => (BiomeFamily::Polar, -0.1, -0.2),
        PearlGardens | SirenShallows => (BiomeFamily::CoastalWater, 0.1, 0.2),
        AbyssalVents => (BiomeFamily::Volcanic, 0.2, -0.1),
    }
}

/// Convert HSV to RGB
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let h = h % 360.0;
    let s = s.clamp(0.0, 1.0);
    let v = v.clamp(0.0, 1.0);

    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;

    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    (
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

/// Get family-based color for a biome with variation
pub(crate) fn get_family_color(biome: ExtendedBiome) -> (u8, u8, u8) {
    let (family, sat_mod, val_mod) = get_biome_family(biome);
    let (h, s, v) = family.base_hsv();
    hsv_to_rgb(h, (s + sat_mod).clamp(0.0, 1.0), (v + val_mod).clamp(0.0, 1.0))
}

/// Sample color from a programmatic LUT based on temperature and moisture
/// This creates smooth, natural-looking transitions between climate zones
pub(crate) fn sample_lut(temperature: f32, moisture: f32, elevation: f32) -> (u8, u8, u8) {
    // Normalize inputs to 0-1 range
    // Temperature: -30 to 30 -> 0 to 1
    let t = ((temperature + 30.0) / 60.0).clamp(0.0, 1.0);
    // Moisture: 0 to 1 already
    let m = moisture.clamp(0.0, 1.0);
    // Elevation factor for mountain colors (0 to 1, higher = more mountain influence)
    let e = (elevation / 3000.0).clamp(0.0, 1.0);

    // Base hue: smoothly transitions from polar cyan/blue (200) -> boreal/temperate green (135-75) -> warm golden/desert (35)
    let base_hue = if t < 0.35 {
        let s = t / 0.35;
        200.0 - s * 65.0
    } else if t < 0.65 {
        let s = (t - 0.35) / 0.30;
        135.0 - s * 60.0
    } else {
        let s = ((t - 0.65) / 0.35).clamp(0.0, 1.0);
        75.0 - s * 40.0
    };

    // Moisture affects saturation: dry = desaturated, wet = saturated
    let saturation = 0.2 + m * 0.5;

    // Value (brightness): forests are darker, deserts lighter
    let value = if m > 0.6 {
        0.3 + (1.0 - m) * 0.3  // Wet = darker forests
    } else if m < 0.3 {
        0.5 + (0.3 - m) * 0.4  // Dry = lighter deserts
    } else {
        0.4 + m * 0.2  // Middle range
    };

    // Elevation shifts towards gray/white for mountains
    let final_hue = base_hue * (1.0 - e * 0.5);
    let final_sat = saturation * (1.0 - e * 0.7);
    let final_val = value + e * 0.3;

    hsv_to_rgb(final_hue, final_sat.clamp(0.0, 1.0), final_val.clamp(0.0, 1.0))
}

/// Blend two colors with a given ratio (0.0 = color a, 1.0 = color b)
fn blend_colors(a: (u8, u8, u8), b: (u8, u8, u8), ratio: f32) -> (u8, u8, u8) {
    let ratio = ratio.clamp(0.0, 1.0);
    let inv = 1.0 - ratio;
    (
        (a.0 as f32 * inv + b.0 as f32 * ratio) as u8,
        (a.1 as f32 * inv + b.1 as f32 * ratio) as u8,
        (a.2 as f32 * inv + b.2 as f32 * ratio) as u8,
    )
}

/// Check if a pixel is at a biome border and return blend info
fn get_border_blend(
    biomes: &Tilemap<ExtendedBiome>,
    x: usize,
    y: usize,
    rng: &mut ChaCha8Rng,
) -> Option<(ExtendedBiome, f32)> {
    let width = biomes.width;
    let height = biomes.height;
    let center_biome = *biomes.get(x, y);

    // Check 8 neighbors
    let mut neighbor_biomes: HashMap<ExtendedBiome, usize> = HashMap::new();

    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }

            let nx = ((x as i32 + dx).rem_euclid(width as i32)) as usize;
            let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;

            let neighbor = *biomes.get(nx, ny);
            if neighbor != center_biome {
                *neighbor_biomes.entry(neighbor).or_insert(0) += 1;
            }
        }
    }

    if neighbor_biomes.is_empty() {
        return None;
    }

    // Find most common different neighbor
    let (most_common, count) = neighbor_biomes.iter()
        .max_by_key(|(_, c)| *c)
        .map(|(b, c)| (*b, *c))
        .unwrap();

    // Probability of using neighbor color based on count
    // More neighbors = higher chance of blending
    let blend_chance = count as f32 / 8.0;

    if rng.gen::<f32>() < blend_chance * 0.6 {
        Some((most_common, blend_chance))
    } else {
        None
    }
}

/// Configuration for map export
#[derive(Clone)]
pub struct MapExportConfig {
    /// Use LUT-based coloring instead of discrete biome colors
    pub use_lut: bool,
    /// Enable border dithering
    pub dithering: bool,
    /// Dithering seed for reproducibility
    pub dither_seed: u64,
    /// Apply hillshading
    pub hillshade: bool,
    /// Hillshade intensity (0.0 to 1.0)
    pub hillshade_intensity: f32,
    /// Height exaggeration for hillshade
    pub height_exaggeration: f32,
    /// Blend LUT with biome colors (0.0 = pure LUT, 1.0 = pure biome)
    pub lut_biome_blend: f32,
    /// Integer upscaling factor (1 = native, 2 = 2x, 4 = 4x, etc.)
    pub upscale_factor: usize,
    /// Explicit target resolution (width, height) overriding upscale_factor if specified
    pub target_resolution: Option<(usize, usize)>,
}

impl Default for MapExportConfig {
    fn default() -> Self {
        Self {
            use_lut: true,
            dithering: true,
            dither_seed: 42,
            hillshade: true,
            hillshade_intensity: 0.5,
            height_exaggeration: 0.035,
            lut_biome_blend: 0.4, // 40% biome color, 60% LUT
            upscale_factor: 1,
            target_resolution: None,
        }
    }
}

/// Compute hillshade values for the entire map using Half-Lambert lighting
/// Half-Lambert prevents pitch-black shadows while maintaining good contrast
fn compute_hillshade(
    heightmap: &Tilemap<f32>,
    z_factor: f32,
) -> Vec<f32> {
    let width = heightmap.width;
    let height = heightmap.height;

    // Light direction from top-left (sun angle)
    let light_dir = (-0.6_f32, -0.6_f32, 0.5_f32);
    let light_len = (light_dir.0 * light_dir.0 + light_dir.1 * light_dir.1 + light_dir.2 * light_dir.2).sqrt();
    let light_dir = (light_dir.0 / light_len, light_dir.1 / light_len, light_dir.2 / light_len);

    let mut hillshade = vec![1.0; width * height];

    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let h = *heightmap.get(x, y);

            if h < 0.0 {
                continue;
            }

            let h_left = *heightmap.get(x.wrapping_sub(1), y);
            let h_right = *heightmap.get(x + 1, y);
            let h_up = *heightmap.get(x, y.wrapping_sub(1));
            let h_down = *heightmap.get(x, y + 1);

            let dzdx = (h_right - h_left) * z_factor / 2.0;
            let dzdy = (h_down - h_up) * z_factor / 2.0;

            let nx = -dzdx;
            let ny = -dzdy;
            let nz = 1.0_f32;
            let n_len = (nx * nx + ny * ny + nz * nz).sqrt();
            let nx = nx / n_len;
            let ny = ny / n_len;
            let nz = nz / n_len;

            // Standard diffuse: ranges from -1 to 1
            let n_dot_l = nx * light_dir.0 + ny * light_dir.1 + nz * light_dir.2;

            // Half-Lambert: wraps lighting to prevent pitch-black shadows
            // Maps [-1, 1] to [0, 1] then squares for softer falloff
            let half_lambert = (n_dot_l * 0.5 + 0.5).powi(2);

            // Ambient term ensures minimum visibility (20% grey floor)
            let ambient = 0.25;
            let diffuse_strength = 0.75;

            let brightness = ambient + half_lambert * diffuse_strength;

            // Allow slight over-brightening for sun-facing slopes
            hillshade[y * width + x] = brightness.clamp(0.2, 1.15);
        }
    }

    hillshade
}

/// Compute specular highlights for water surfaces
fn compute_water_specular(
    heightmap: &Tilemap<f32>,
    x: usize,
    y: usize,
) -> f32 {
    let width = heightmap.width;
    let height = heightmap.height;

    // Light and view direction (viewer looking straight down, light from top-left)
    let light_dir = (-0.6_f32, -0.6_f32, 0.5_f32);
    let light_len = (light_dir.0 * light_dir.0 + light_dir.1 * light_dir.1 + light_dir.2 * light_dir.2).sqrt();
    let light_dir = (light_dir.0 / light_len, light_dir.1 / light_len, light_dir.2 / light_len);

    // View direction (straight up from the map)
    let view_dir = (0.0_f32, 0.0_f32, 1.0_f32);

    // Get terrain slope for water surface (water follows terrain slightly)
    let x_left = if x == 0 { width - 1 } else { x - 1 };
    let x_right = if x == width - 1 { 0 } else { x + 1 };
    let y_up = y.saturating_sub(1);
    let y_down = (y + 1).min(height - 1);

    let h_left = *heightmap.get(x_left, y);
    let h_right = *heightmap.get(x_right, y);
    let h_up = *heightmap.get(x, y_up);
    let h_down = *heightmap.get(x, y_down);

    // Very gentle slope for water surface
    let z_factor = 0.005;
    let dzdx = (h_right - h_left) * z_factor / 2.0;
    let dzdy = (h_down - h_up) * z_factor / 2.0;

    // Water surface normal (mostly flat with slight terrain influence)
    let nx = -dzdx * 0.3;
    let ny = -dzdy * 0.3;
    let nz = 1.0_f32;
    let n_len = (nx * nx + ny * ny + nz * nz).sqrt();
    let nx = nx / n_len;
    let ny = ny / n_len;
    let nz = nz / n_len;

    // Reflect light direction around normal
    let n_dot_l = nx * light_dir.0 + ny * light_dir.1 + nz * light_dir.2;
    let reflect_x = light_dir.0 - 2.0 * n_dot_l * nx;
    let reflect_y = light_dir.1 - 2.0 * n_dot_l * ny;
    let reflect_z = light_dir.2 - 2.0 * n_dot_l * nz;

    // Specular intensity (view dot reflect)
    let spec = (reflect_x * view_dir.0 + reflect_y * view_dir.1 + reflect_z * view_dir.2).max(0.0);

    // Sharpen specular highlight (higher power = tighter highlight)
    spec.powf(16.0)
}

/// 1D Catmull-Rom cubic spline interpolation
#[inline(always)]
fn catmull_rom_1d(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    let t2 = t * t;
    let t3 = t2 * t;
    0.5 * ((2.0 * p1) +
           (-p0 + p2) * t +
           (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2 +
           (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
}

/// Sample continuous sub-pixel field using Catmull-Rom bicubic spline with seamless X-wrapping
#[inline(always)]
fn sample_tilemap_catmull_rom(map: &Tilemap<f32>, u: f32, v: f32) -> f32 {
    let w = map.width as i32;
    let h = map.height as i32;

    let x0 = u.floor() as i32;
    let y0 = v.floor() as i32;
    let tx = u - x0 as f32;
    let ty = v - y0 as f32;

    let mut col_vals = [0.0f32; 4];
    for (j, dy) in (-1..=2).enumerate() {
        let y_sample = (y0 + dy).clamp(0, h - 1) as usize;
        let p0 = *map.get((x0 - 1).rem_euclid(w) as usize, y_sample);
        let p1 = *map.get(x0.rem_euclid(w) as usize, y_sample);
        let p2 = *map.get((x0 + 1).rem_euclid(w) as usize, y_sample);
        let p3 = *map.get((x0 + 2).rem_euclid(w) as usize, y_sample);
        col_vals[j] = catmull_rom_1d(p0, p1, p2, p3, tx);
    }

    catmull_rom_1d(col_vals[0], col_vals[1], col_vals[2], col_vals[3], ty)
}

/// Biome family colour at fractional simulation coordinates, bilinearly blended between the
/// four surrounding cells (cell centres sit at integer coordinates; x wraps).
pub(crate) fn bilinear_biome_color(biomes: &Tilemap<ExtendedBiome>, u: f32, v: f32) -> (u8, u8, u8) {
    let (w, h) = (biomes.width as i64, biomes.height as i64);
    let (x0, y0) = (u.floor(), v.floor());
    let (tx, ty) = (u - x0, v - y0);
    let xi = |dx: i64| ((x0 as i64 + dx).rem_euclid(w)) as usize;
    let yi = |dy: i64| ((y0 as i64 + dy).clamp(0, h - 1)) as usize;
    let c = |dx: i64, dy: i64| get_family_color(*biomes.get(xi(dx), yi(dy)));
    let (c00, c10, c01, c11) = (c(0, 0), c(1, 0), c(0, 1), c(1, 1));
    let mix = |a: u8, b: u8, cc: u8, d: u8| {
        let top = a as f32 * (1.0 - tx) + b as f32 * tx;
        let bot = cc as f32 * (1.0 - tx) + d as f32 * tx;
        (top * (1.0 - ty) + bot * ty) as u8
    };
    (
        mix(c00.0, c10.0, c01.0, c11.0),
        mix(c00.1, c10.1, c01.1, c11.1),
        mix(c00.2, c10.2, c01.2, c11.2),
    )
}

/// Rasterize the Bezier river network at output resolution as anti-aliased strokes.
/// Returns per-pixel river opacity (0 = none). Rivers keep their curved shape at any
/// upscale instead of becoming the stair-stepped blocks of their simulation cells, and
/// width/opacity follow discharge so headwaters fade in rather than starting abruptly.
fn rasterize_rivers_hires(
    network: &crate::erosion::RiverNetwork,
    sim_w: usize,
    sim_h: usize,
    out_w: usize,
    out_h: usize,
) -> Vec<f32> {
    const MIN_DRAW_FLOW: f32 = 25.0;
    const FULL_DRAW_FLOW: f32 = 60.0;
    let sx = out_w as f32 / sim_w as f32;
    let sy = out_h as f32 / sim_h as f32;
    let mut cov = vec![0.0f32; out_w * out_h];

    for seg in &network.segments {
        if seg.p0.flow_accumulation.max(seg.p3.flow_accumulation) < MIN_DRAW_FLOW {
            continue;
        }
        let len_px = seg.approximate_length(8) * sx.max(sy);
        let samples = (len_px / 0.35).ceil() as usize + 2;
        for i in 0..=samples {
            let pt = seg.evaluate(i as f32 / samples as f32);
            let fade = ((pt.flow_accumulation - MIN_DRAW_FLOW) / (FULL_DRAW_FLOW - MIN_DRAW_FLOW)).clamp(0.0, 1.0);
            if fade <= 0.0 {
                continue;
            }
            let size = ((pt.flow_accumulation / MIN_DRAW_FLOW).ln() / 500.0f32.ln()).clamp(0.0, 1.0);
            let strength = fade * (0.55 + 0.35 * size);
            let r = (pt.width * sx * 0.5).max(0.45);
            let cx = pt.world_x * sx;
            let cy = pt.world_y * sy;
            let (x0, x1) = ((cx - r - 1.0).floor() as i64, (cx + r + 1.0).ceil() as i64);
            let (y0, y1) = ((cy - r - 1.0).floor() as i64, (cy + r + 1.0).ceil() as i64);
            for py in y0..=y1 {
                if py < 0 || py >= out_h as i64 {
                    continue;
                }
                for px in x0..=x1 {
                    let dx = px as f32 + 0.5 - cx;
                    let dy = py as f32 + 0.5 - cy;
                    let c = (r + 0.5 - (dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0) * strength;
                    if c > 0.0 {
                        let idx = py as usize * out_w + px.rem_euclid(out_w as i64) as usize;
                        if c > cov[idx] {
                            cov[idx] = c;
                        }
                    }
                }
            }
        }
    }
    cov
}

/// Export the visual map (the pretty version with shading, blending, and optional procedural super-resolution)
pub fn export_visual_map(
    world: &WorldData,
    filename: &str,
    config: &MapExportConfig,
) -> Result<(), Box<dyn Error>> {
    let sim_w = world.heightmap.width;
    let sim_h = world.heightmap.height;
    const RIVER_THRESHOLD: f32 = 50.0;

    let upscale_factor = config.upscale_factor.max(1);
    let (out_w, out_h) = match config.target_resolution {
        Some((tw, th)) => (tw.max(16), th.max(16)),
        None => (sim_w * upscale_factor, sim_h * upscale_factor),
    };

    let mut rng = ChaCha8Rng::seed_from_u64(config.dither_seed);
    let mut img = ImageBuffer::new(out_w as u32, out_h as u32);

    let upscaled = out_w > sim_w || out_h > sim_h;

    let river_cov = world
        .river_network
        .as_ref()
        .map(|net| rasterize_rivers_hires(net, sim_w, sim_h, out_w, out_h));

    let light_dir = (-0.6_f32, -0.6_f32, 0.5_f32);
    let light_len = (light_dir.0 * light_dir.0 + light_dir.1 * light_dir.1 + light_dir.2 * light_dir.2).sqrt();
    let light_dir = (light_dir.0 / light_len, light_dir.1 / light_len, light_dir.2 / light_len);

    for y_out in 0..out_h {
        let sim_v = ((y_out as f32 + 0.5) / out_h as f32) * (sim_h as f32);
        for x_out in 0..out_w {
            let sim_u = ((x_out as f32 + 0.5) / out_w as f32) * (sim_w as f32);

            let nearest_x = (sim_u.round() as usize).rem_euclid(sim_w);
            let nearest_y = (sim_v.round() as usize).clamp(0, sim_h - 1);

            let biome = *world.biomes.get(nearest_x, nearest_y);
            let h = sample_tilemap_catmull_rom(&world.heightmap, sim_u, sim_v);
            let temp = sample_tilemap_catmull_rom(&world.temperature, sim_u, sim_v);
            let moist = sample_tilemap_catmull_rom(&world.moisture, sim_u, sim_v);
            let water_depth = sample_tilemap_catmull_rom(&world.water_depth, sim_u, sim_v);

            let flow_acc = world.flow_accumulation.as_ref()
                .map(|fa| *fa.get(nearest_x, nearest_y))
                .unwrap_or(0.0);
            // Prefer the smooth Bezier network; fall back to per-cell flow accumulation.
            let river_opacity = match &river_cov {
                Some(cov) => cov[y_out * out_w + x_out],
                None if flow_acc > RIVER_THRESHOLD => 0.5 + (flow_acc / 200.0).min(1.0) * 0.35,
                None => 0.0,
            };
            let is_river = h >= 0.0 && river_opacity > 0.02;
            // Thin anti-aliased rivers keep the terrain's hillshade underneath them.
            let is_water = h < 0.0 || water_depth > 0.5 || (river_cov.is_none() && is_river);

            let shade = if is_water || !config.hillshade {
                1.0
            } else {
                let delta = 0.5f32;
                let h_l = sample_tilemap_catmull_rom(&world.heightmap, sim_u - delta, sim_v);
                let h_r = sample_tilemap_catmull_rom(&world.heightmap, sim_u + delta, sim_v);
                let h_u = sample_tilemap_catmull_rom(&world.heightmap, sim_u, sim_v - delta);
                let h_d = sample_tilemap_catmull_rom(&world.heightmap, sim_u, sim_v + delta);

                let dzdx = (h_r - h_l) * config.height_exaggeration / (2.0 * delta);
                let dzdy = (h_d - h_u) * config.height_exaggeration / (2.0 * delta);

                let nx = -dzdx;
                let ny = -dzdy;
                let nz = 1.0f32;
                let n_len = (nx * nx + ny * ny + nz * nz).sqrt();
                let nx = nx / n_len;
                let ny = ny / n_len;
                let nz = nz / n_len;

                let n_dot_l = nx * light_dir.0 + ny * light_dir.1 + nz * light_dir.2;
                let half_lambert = (n_dot_l * 0.5 + 0.5).powi(2);
                let base_shade = (0.25 + half_lambert * 0.75).clamp(0.2, 1.15);
                1.0 + (base_shade - 1.0) * config.hillshade_intensity
            };

            // First, compute terrain color
            let terrain_color = {
                let lut_color = if config.use_lut {
                    sample_lut(temp, moist, h.max(0.0))
                } else {
                    get_family_color(biome)
                };

                // When upscaling, blend biome colours bilinearly between the surrounding cells so
                // biome borders stay smooth instead of becoming one block per simulation cell.
                let biome_color = if upscaled {
                    bilinear_biome_color(&world.biomes, sim_u, sim_v)
                } else {
                    get_family_color(biome)
                };
                let mut color = blend_colors(lut_color, biome_color, config.lut_biome_blend);

                // Apply dithering at borders (native resolution only: upscaled, it paints
                // whole simulation cells and looks blocky)
                if config.dithering && !upscaled {
                    if let Some((neighbor_biome, _)) = get_border_blend(&world.biomes, nearest_x, nearest_y, &mut rng) {
                        let neighbor_color = get_family_color(neighbor_biome);
                        color = blend_colors(color, neighbor_color, 0.3);
                    }
                }

                // Apply hillshading to terrain
                let shaded_r = ((color.0 as f32) * shade).clamp(0.0, 255.0) as u8;
                let shaded_g = ((color.1 as f32) * shade).clamp(0.0, 255.0) as u8;
                let shaded_b = ((color.2 as f32) * shade).clamp(0.0, 255.0) as u8;
                (shaded_r, shaded_g, shaded_b)
            };

            let (r, g, b) = if h < 0.0 {
                // Ocean - depth-based color with subtle specular
                // Shallow shelves are light, the deep ocean dark (this ramp used to run the other
                // way, which drew a dark halo along every coast). sqrt spreads the shelf range.
                let depth_factor = ((-h) / 4000.0).min(1.0).sqrt();
                let spec = compute_water_specular(&world.heightmap, nearest_x, nearest_y) * 0.3;

                let base_r = 70.0 - depth_factor * 55.0;
                let base_g = 140.0 - depth_factor * 100.0;
                let base_b = 185.0 - depth_factor * 90.0;

                let r = (base_r + spec * 200.0).clamp(0.0, 255.0) as u8;
                let g = (base_g + spec * 200.0).clamp(0.0, 255.0) as u8;
                let b = (base_b + spec * 150.0).clamp(0.0, 255.0) as u8;

                (r, g, b)
            } else if is_river {
                // River - semi-transparent over terrain with specular highlight
                let river_base = (50, 120, 180);
                let opacity = river_opacity;

                let blended = blend_colors(terrain_color, river_base, opacity);
                let spec = compute_water_specular(&world.heightmap, nearest_x, nearest_y);
                let spec_intensity = 0.6;

                let r = (blended.0 as f32 + spec * spec_intensity * 255.0).clamp(0.0, 255.0) as u8;
                let g = (blended.1 as f32 + spec * spec_intensity * 255.0).clamp(0.0, 255.0) as u8;
                let b = (blended.2 as f32 + spec * spec_intensity * 200.0).clamp(0.0, 255.0) as u8;

                (r, g, b)
            } else if water_depth > 0.5 {
                // Lake - semi-transparent with specular
                let lake_base = (60, 110, 170);
                let opacity = 0.6 + (water_depth / 10.0).min(0.3);

                let blended = blend_colors(terrain_color, lake_base, opacity);
                let spec = compute_water_specular(&world.heightmap, nearest_x, nearest_y);
                let spec_intensity = 0.5;

                let r = (blended.0 as f32 + spec * spec_intensity * 255.0).clamp(0.0, 255.0) as u8;
                let g = (blended.1 as f32 + spec * spec_intensity * 255.0).clamp(0.0, 255.0) as u8;
                let b = (blended.2 as f32 + spec * spec_intensity * 200.0).clamp(0.0, 255.0) as u8;

                (r, g, b)
            } else {
                terrain_color
            };

            img.put_pixel(x_out as u32, y_out as u32, Rgb([r, g, b]));
        }
    }

    img.save(filename)?;
    println!("Exported visual map to {}", filename);
    Ok(())
}

/// Export the data map (raw biome colors, no shading, for analysis)
pub fn export_data_map(
    world: &WorldData,
    filename: &str,
) -> Result<(), Box<dyn Error>> {
    let width = world.heightmap.width;
    let height = world.heightmap.height;

    let mut img = ImageBuffer::new(width as u32, height as u32);

    for y in 0..height {
        for x in 0..width {
            let biome = *world.biomes.get(x, y);
            let (r, g, b) = biome.color(); // Use original distinct colors
            img.put_pixel(x as u32, y as u32, Rgb([r, g, b]));
        }
    }

    img.save(filename)?;
    println!("Exported data map to {}", filename);
    Ok(())
}

/// Export a legend image showing all biomes present in the world
pub fn export_legend(
    world: &WorldData,
    filename: &str,
) -> Result<(), Box<dyn Error>> {
    // Collect all unique biomes in the world
    let mut biomes_present: Vec<ExtendedBiome> = Vec::new();
    let mut seen: std::collections::HashSet<u8> = std::collections::HashSet::new();

    for (_, _, biome) in world.biomes.iter() {
        let id = *biome as u8;
        if !seen.contains(&id) {
            seen.insert(id);
            biomes_present.push(*biome);
        }
    }

    // Sort biomes by family for organized legend
    biomes_present.sort_by_key(|b| {
        let (family, _, _) = get_biome_family(*b);
        (family as u8, *b as u8)
    });

    // Create legend image
    let swatch_size = 20;
    let text_width = 200;
    let row_height = 24;
    let padding = 4;
    let legend_width = swatch_size + padding + text_width;
    let legend_height = (biomes_present.len() as u32 + 1) * row_height;

    let mut img: RgbImage = ImageBuffer::from_pixel(legend_width, legend_height, Rgb([255, 255, 255]));

    // Draw each biome entry
    for (i, biome) in biomes_present.iter().enumerate() {
        let y_offset = (i as u32 + 1) * row_height;
        let (r, g, b) = get_family_color(*biome);

        // Draw color swatch
        for dy in 0..swatch_size {
            for dx in 0..swatch_size {
                img.put_pixel(dx + 2, y_offset + dy, Rgb([r, g, b]));
            }
        }

        // Draw border around swatch
        for dx in 0..swatch_size {
            img.put_pixel(dx + 2, y_offset, Rgb([0, 0, 0]));
            img.put_pixel(dx + 2, y_offset + swatch_size - 1, Rgb([0, 0, 0]));
        }
        for dy in 0..swatch_size {
            img.put_pixel(2, y_offset + dy, Rgb([0, 0, 0]));
            img.put_pixel(swatch_size + 1, y_offset + dy, Rgb([0, 0, 0]));
        }
    }

    img.save(filename)?;
    println!("Exported legend to {} ({} biomes)", filename, biomes_present.len());

    // Also save a text legend file
    let text_filename = filename.replace(".png", ".txt");
    let mut legend_text = String::new();
    legend_text.push_str("BIOME LEGEND\n");
    legend_text.push_str("============\n\n");

    let mut current_family: Option<BiomeFamily> = None;
    for biome in &biomes_present {
        let (family, _, _) = get_biome_family(*biome);
        if current_family != Some(family) {
            current_family = Some(family);
            legend_text.push_str(&format!("\n[{:?}]\n", family));
        }
        let (r, g, b) = get_family_color(*biome);
        legend_text.push_str(&format!(
            "  {:?} - RGB({}, {}, {})\n",
            biome, r, g, b
        ));
    }

    std::fs::write(&text_filename, &legend_text)?;
    println!("Exported legend text to {}", text_filename);

    Ok(())
}

/// Export all map variants at once
pub fn export_all_maps(
    world: &WorldData,
    output_dir: &Path,
    seed: u64,
    config: &MapExportConfig,
) -> Result<(), Box<dyn Error>> {
    let prefix = format!("world_{}", seed);

    // Visual map (pretty, with shading and blending)
    let visual_path = output_dir.join(format!("{}_visual.png", prefix));
    export_visual_map(world, visual_path.to_str().unwrap(), config)?;

    // Data map (raw biome colors for analysis)
    let data_path = output_dir.join(format!("{}_data.png", prefix));
    export_data_map(world, data_path.to_str().unwrap())?;

    // Legend
    let legend_path = output_dir.join(format!("{}_legend.png", prefix));
    export_legend(world, legend_path.to_str().unwrap())?;

    Ok(())
}
