//! EXR export functionality for heightmaps, river maps, biome maps, and normal maps

use crate::biomes::ExtendedBiome;
use crate::tilemap::Tilemap;
use exr::prelude::*;
use std::path::Path;

/// Export a heightmap to an EXR file with multiple channels.
/// Channels:
/// - "height": Raw elevation in meters
/// - "height_normalized": Elevation normalized to 0-1 range
pub fn export_heightmap_exr(
    heightmap: &Tilemap<f32>,
    path: &Path,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let width = heightmap.width;
    let height = heightmap.height;

    // Find min/max for normalization
    let mut min_h = f32::MAX;
    let mut max_h = f32::MIN;
    for y in 0..height {
        for x in 0..width {
            let h = *heightmap.get(x, y);
            if h < min_h { min_h = h; }
            if h > max_h { max_h = h; }
        }
    }
    let range = (max_h - min_h).max(1.0);

    // Raw elevation data
    let raw_pixels: Vec<f32> = (0..height)
        .flat_map(|y| (0..width).map(move |x| *heightmap.get(x, y)))
        .collect();

    // Normalized elevation (0-1 range)
    let norm_pixels: Vec<f32> = (0..height)
        .flat_map(|y| (0..width).map(move |x| {
            let h = *heightmap.get(x, y);
            (h - min_h) / range
        }))
        .collect();

    // Create the EXR image with both channels
    let layer = Layer::new(
        (width, height),
        LayerAttributes::named("heightmap"),
        Encoding::SMALL_FAST_LOSSLESS,
        AnyChannels::sort(smallvec::smallvec![
            AnyChannel::new("height", FlatSamples::F32(raw_pixels)),
            AnyChannel::new("height_normalized", FlatSamples::F32(norm_pixels)),
        ]),
    );

    let image = Image::from_layer(layer);

    image.write().to_file(path)?;

    Ok(())
}

/// Export a normal map computed from the heightmap gradients.
/// Channels (in tangent space, suitable for 3D applications):
/// - "normal_x": X component of surface normal (-1 to 1, remapped to 0-1 for compatibility)
/// - "normal_y": Y component of surface normal (-1 to 1, remapped to 0-1 for compatibility)
/// - "normal_z": Z component of surface normal (always positive, pointing up)
///
/// The z_scale parameter controls height exaggeration (higher = more dramatic normals).
/// Typical values: 0.01-0.1 for subtle terrain, 0.5-2.0 for dramatic cliffs.
pub fn export_normal_map_exr(
    heightmap: &Tilemap<f32>,
    path: &Path,
    z_scale: f32,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let width = heightmap.width;
    let height = heightmap.height;

    let mut normal_x: Vec<f32> = vec![0.5; width * height];
    let mut normal_y: Vec<f32> = vec![0.5; width * height];
    let mut normal_z: Vec<f32> = vec![1.0; width * height];

    for y in 0..height {
        for x in 0..width {
            // Get neighboring heights (with wrapping for x, clamping for y)
            let x_left = if x == 0 { width - 1 } else { x - 1 };
            let x_right = if x == width - 1 { 0 } else { x + 1 };
            let y_up = y.saturating_sub(1);
            let y_down = (y + 1).min(height - 1);

            let h_left = *heightmap.get(x_left, y);
            let h_right = *heightmap.get(x_right, y);
            let h_up = *heightmap.get(x, y_up);
            let h_down = *heightmap.get(x, y_down);

            // Calculate gradients
            let dzdx = (h_right - h_left) * z_scale / 2.0;
            let dzdy = (h_down - h_up) * z_scale / 2.0;

            // Surface normal: (-dzdx, -dzdy, 1) normalized
            let nx = -dzdx;
            let ny = -dzdy;
            let nz = 1.0_f32;
            let n_len = (nx * nx + ny * ny + nz * nz).sqrt();

            let idx = y * width + x;
            // Store as raw -1 to 1 values (EXR supports negative values)
            normal_x[idx] = nx / n_len;
            normal_y[idx] = ny / n_len;
            normal_z[idx] = nz / n_len;
        }
    }

    let layer = Layer::new(
        (width, height),
        LayerAttributes::named("normals"),
        Encoding::SMALL_FAST_LOSSLESS,
        AnyChannels::sort(smallvec::smallvec![
            AnyChannel::new("normal_x", FlatSamples::F32(normal_x)),
            AnyChannel::new("normal_y", FlatSamples::F32(normal_y)),
            AnyChannel::new("normal_z", FlatSamples::F32(normal_z)),
        ]),
    );

    let image = Image::from_layer(layer);

    image.write().to_file(path)?;

    Ok(())
}

/// Export an ambient occlusion approximation based on local terrain curvature.
/// Higher values indicate more exposed areas (ridges), lower values indicate sheltered areas (valleys).
pub fn export_occlusion_map_exr(
    heightmap: &Tilemap<f32>,
    path: &Path,
    sample_radius: usize,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let width = heightmap.width;
    let height = heightmap.height;

    let mut occlusion: Vec<f32> = vec![0.5; width * height];

    for y in 0..height {
        for x in 0..width {
            let center_h = *heightmap.get(x, y);
            let mut total_diff = 0.0_f32;
            let mut count = 0;

            // Sample neighbors in a radius
            for dy in -(sample_radius as i32)..=(sample_radius as i32) {
                for dx in -(sample_radius as i32)..=(sample_radius as i32) {
                    if dx == 0 && dy == 0 {
                        continue;
                    }

                    let nx = ((x as i32 + dx).rem_euclid(width as i32)) as usize;
                    let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;

                    let neighbor_h = *heightmap.get(nx, ny);
                    let dist = ((dx * dx + dy * dy) as f32).sqrt();

                    // How much higher/lower is neighbor relative to distance?
                    total_diff += (center_h - neighbor_h) / (dist * 100.0);
                    count += 1;
                }
            }

            // Normalize: positive = ridge/exposed, negative = valley/sheltered
            let avg_diff = if count > 0 { total_diff / count as f32 } else { 0.0 };

            // Map to 0-1 range (0.5 = flat, >0.5 = ridge, <0.5 = valley)
            occlusion[y * width + x] = (0.5 + avg_diff * 5.0).clamp(0.0, 1.0);
        }
    }

    let layer = Layer::new(
        (width, height),
        LayerAttributes::named("occlusion"),
        Encoding::SMALL_FAST_LOSSLESS,
        AnyChannels::sort(smallvec::smallvec![
            AnyChannel::new("ao", FlatSamples::F32(occlusion)),
        ]),
    );

    let image = Image::from_layer(layer);

    image.write().to_file(path)?;

    Ok(())
}

/// Export a river/flow accumulation map to an EXR file.
/// The flow values are stored as 32-bit floats in a single channel named "flow".
pub fn export_river_map_exr(
    flow_map: &Tilemap<f32>,
    path: &Path,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let width = flow_map.width;
    let height = flow_map.height;

    // Convert tilemap data to a flat vector
    let pixels: Vec<f32> = (0..height)
        .flat_map(|y| (0..width).map(move |x| *flow_map.get(x, y)))
        .collect();

    // Create the EXR image with a single "flow" channel
    let layer = Layer::new(
        (width, height),
        LayerAttributes::named("rivers"),
        Encoding::SMALL_FAST_LOSSLESS,
        AnyChannels::sort(smallvec::smallvec![
            AnyChannel::new("flow", FlatSamples::F32(pixels)),
        ]),
    );

    let image = Image::from_layer(layer);

    image.write().to_file(path)?;

    Ok(())
}

/// Export a biome map to an EXR file.
/// Each biome type is stored as an integer ID in a single channel named "biome".
/// The IDs correspond to the ExtendedBiome enum variants (0-indexed).
pub fn export_biome_map_exr(
    biome_map: &Tilemap<ExtendedBiome>,
    path: &Path,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let width = biome_map.width;
    let height = biome_map.height;

    // Convert biome enum to integer IDs (as f32 for EXR compatibility)
    let pixels: Vec<f32> = (0..height)
        .flat_map(|y| (0..width).map(move |x| biome_to_id(*biome_map.get(x, y)) as f32))
        .collect();

    // Create the EXR image with a single "biome" channel
    let layer = Layer::new(
        (width, height),
        LayerAttributes::named("biomes"),
        Encoding::SMALL_FAST_LOSSLESS,
        AnyChannels::sort(smallvec::smallvec![
            AnyChannel::new("biome", FlatSamples::F32(pixels)),
        ]),
    );

    let image = Image::from_layer(layer);

    image.write().to_file(path)?;

    Ok(())
}

/// Convert an ExtendedBiome variant to a unique integer ID.
/// IDs are assigned based on the enum variant order.
fn biome_to_id(biome: ExtendedBiome) -> u32 {
    use ExtendedBiome::*;
    match biome {
        // Base biomes (0-12)
        DeepOcean => 0,
        Ocean => 1,
        CoastalWater => 2,
        Ice => 3,
        Tundra => 4,
        BorealForest => 5,
        TemperateGrassland => 6,
        TemperateForest => 7,
        TemperateRainforest => 8,
        Desert => 9,
        Savanna => 10,
        TropicalForest => 11,
        TropicalRainforest => 12,

        // Mountain zonation biomes (13-24)
        MontaneForest => 13,
        CloudForest => 14,
        Paramo => 15,
        SubalpineForest => 16,
        AlpineMeadow => 17,
        AlpineTundra => 18,
        SnowyPeaks => 19,
        HighlandLake => 20,
        CraterLake => 21,
        Foothills => 22,
        Lagoon => 23,

        // Fantasy forests (24-28)
        DeadForest => 24,
        CrystalForest => 25,
        BioluminescentForest => 26,
        MushroomForest => 27,
        PetrifiedForest => 28,

        // Fantasy waters (29-32)
        AcidLake => 29,
        LavaLake => 30,
        FrozenLake => 31,
        BioluminescentWater => 32,

        // Wastelands (33-36)
        VolcanicWasteland => 33,
        SaltFlats => 34,
        Ashlands => 35,
        CrystalWasteland => 36,

        // Wetlands (37-40)
        Swamp => 37,
        Marsh => 38,
        Bog => 39,
        MangroveSaltmarsh => 40,

        // Ultra-rare biomes - Ancient/Primeval (41-43)
        AncientGrove => 41,
        TitanBones => 42,
        CoralPlateau => 43,

        // Ultra-rare biomes - Geothermal/Volcanic (44-46)
        ObsidianFields => 44,
        Geysers => 45,
        TarPits => 46,

        // Ultra-rare biomes - Magical/Anomalous (47-50)
        FloatingStones => 47,
        Shadowfen => 48,
        PrismaticPools => 49,
        AuroraWastes => 50,

        // Ultra-rare biomes - Desert variants (51-53)
        SingingDunes => 51,
        Oasis => 52,
        GlassDesert => 53,

        // Ultra-rare biomes - Aquatic (54-55)
        AbyssalVents => 54,
        Sargasso => 55,

        // Mystical / Supernatural (56-60)
        EtherealMist => 56,
        StarfallCrater => 57,
        LeyNexus => 58,
        WhisperingStones => 59,
        SpiritMarsh => 60,

        // Extreme Geological (61-65)
        SulfurVents => 61,
        BasaltColumns => 62,
        PaintedHills => 63,
        RazorPeaks => 64,
        SinkholeLakes => 65,

        // Biological Wonders (66-70)
        ColossalHive => 66,
        BoneFields => 67,
        CarnivorousBog => 68,
        FungalBloom => 69,
        KelpTowers => 70,

        // Exotic Waters (71-75)
        BrinePools => 71,
        HotSprings => 72,
        MirrorLake => 73,
        InkSea => 74,
        PhosphorShallows => 75,

        // Alien / Corrupted (76-80)
        VoidScar => 76,
        SiliconGrove => 77,
        SporeWastes => 78,
        BleedingStone => 79,
        HollowEarth => 80,

        // Ancient Ruins (81-85)
        SunkenCity => 81,
        CyclopeanRuins => 82,
        BuriedTemple => 83,
        OvergrownCitadel => 84,
        DarkTower => 85,

        // Realistic Ocean - Shallow/Coastal (86-88)
        CoralReef => 86,
        KelpForest => 87,
        SeagrassMeadow => 88,

        // Realistic Ocean - Mid-depth (89-90)
        ContinentalShelf => 89,
        Seamount => 90,

        // Realistic Ocean - Deep (91-95)
        OceanicTrench => 91,
        AbyssalPlain => 92,
        MidOceanRidge => 93,
        ColdSeep => 94,
        BrinePool => 95,

        // Fantasy Ocean (96-103)
        CrystalDepths => 96,
        LeviathanGraveyard => 97,
        DrownedCitadel => 98,
        VoidMaw => 99,
        PearlGardens => 100,
        SirenShallows => 101,
        FrozenAbyss => 102,
        ThermalVents => 103,

        // Karst & Cave Biomes (104-109)
        KarstPlains => 104,
        TowerKarst => 105,
        Sinkhole => 106,
        Cenote => 107,
        CaveEntrance => 108,
        CockpitKarst => 109,

        // Volcanic Biomes (110-116)
        Caldera => 110,
        ShieldVolcano => 111,
        VolcanicCone => 112,
        LavaField => 113,
        FumaroleField => 114,
        VolcanicBeach => 115,
        HotSpot => 116,
    }
}

/// Export heightmap, river map, and biome map to separate EXR files.
/// Files are named: {prefix}_heightmap.exr, {prefix}_rivers.exr, {prefix}_biomes.exr
/// Export all world data to separate EXR files for 3D texturing.
/// Files created:
/// - {prefix}_heightmap.exr: Raw elevation + normalized (0-1)
/// - {prefix}_rivers.exr: Flow accumulation data
/// - {prefix}_biomes.exr: Biome IDs as integers
/// - {prefix}_normals.exr: Surface normal map (X, Y, Z components)
/// - {prefix}_occlusion.exr: Ambient occlusion approximation
pub fn export_world_exr(
    heightmap: &Tilemap<f32>,
    flow_map: &Tilemap<f32>,
    biome_map: &Tilemap<ExtendedBiome>,
    output_dir: &Path,
    seed: u64,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let prefix = format!("world_{}", seed);

    let heightmap_path = output_dir.join(format!("{}_heightmap.exr", prefix));
    let river_path = output_dir.join(format!("{}_rivers.exr", prefix));
    let biome_path = output_dir.join(format!("{}_biomes.exr", prefix));
    let normal_path = output_dir.join(format!("{}_normals.exr", prefix));
    let occlusion_path = output_dir.join(format!("{}_occlusion.exr", prefix));

    println!("Exporting heightmap to: {}", heightmap_path.display());
    export_heightmap_exr(heightmap, &heightmap_path)?;

    println!("Exporting river map to: {}", river_path.display());
    export_river_map_exr(flow_map, &river_path)?;

    println!("Exporting biome map to: {}", biome_path.display());
    export_biome_map_exr(biome_map, &biome_path)?;

    // Normal map with moderate z_scale for good terrain detail
    // 0.03 provides subtle but visible normals for terrain features
    println!("Exporting normal map to: {}", normal_path.display());
    export_normal_map_exr(heightmap, &normal_path, 0.03)?;

    // Occlusion map with radius of 3 tiles for local curvature
    println!("Exporting occlusion map to: {}", occlusion_path.display());
    export_occlusion_map_exr(heightmap, &occlusion_path, 3)?;

    Ok(())
}

/// Export a combined heightmap with rivers as a multi-channel EXR.
/// Channels: "height" (elevation in meters), "flow" (flow accumulation)
pub fn export_combined_exr(
    heightmap: &Tilemap<f32>,
    flow_map: &Tilemap<f32>,
    path: &Path,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let width = heightmap.width;
    let height = heightmap.height;

    // Convert heightmap data
    let height_pixels: Vec<f32> = (0..height)
        .flat_map(|y| (0..width).map(move |x| *heightmap.get(x, y)))
        .collect();

    // Convert flow data
    let flow_pixels: Vec<f32> = (0..height)
        .flat_map(|y| (0..width).map(move |x| *flow_map.get(x, y)))
        .collect();

    // Create the EXR image with both channels
    let layer = Layer::new(
        (width, height),
        LayerAttributes::named("world"),
        Encoding::SMALL_FAST_LOSSLESS,
        AnyChannels::sort(smallvec::smallvec![
            AnyChannel::new("height", FlatSamples::F32(height_pixels)),
            AnyChannel::new("flow", FlatSamples::F32(flow_pixels)),
        ]),
    );

    let image = Image::from_layer(layer);

    image.write().to_file(path)?;

    Ok(())
}

/// Get a mapping of biome IDs to their names for reference.
/// Returns a vector of (id, name) tuples.
pub fn get_biome_id_mapping() -> Vec<(u32, &'static str)> {
    vec![
        (0, "DeepOcean"),
        (1, "Ocean"),
        (2, "CoastalWater"),
        (3, "Ice"),
        (4, "Tundra"),
        (5, "BorealForest"),
        (6, "TemperateGrassland"),
        (7, "TemperateForest"),
        (8, "TemperateRainforest"),
        (9, "Desert"),
        (10, "Savanna"),
        (11, "TropicalForest"),
        (12, "TropicalRainforest"),
        (13, "MontaneForest"),
        (14, "CloudForest"),
        (15, "Paramo"),
        (16, "SubalpineForest"),
        (17, "AlpineMeadow"),
        (18, "AlpineTundra"),
        (19, "SnowyPeaks"),
        (20, "HighlandLake"),
        (21, "CraterLake"),
        (22, "Foothills"),
        (23, "Lagoon"),
        (24, "DeadForest"),
        (25, "CrystalForest"),
        (26, "BioluminescentForest"),
        (27, "MushroomForest"),
        (28, "PetrifiedForest"),
        (29, "AcidLake"),
        (30, "LavaLake"),
        (31, "FrozenLake"),
        (32, "BioluminescentWater"),
        (33, "VolcanicWasteland"),
        (34, "SaltFlats"),
        (35, "Ashlands"),
        (36, "CrystalWasteland"),
        (37, "Swamp"),
        (38, "Marsh"),
        (39, "Bog"),
        (40, "MangroveSaltmarsh"),
        (41, "AncientGrove"),
        (42, "TitanBones"),
        (43, "CoralPlateau"),
        (44, "ObsidianFields"),
        (45, "Geysers"),
        (46, "TarPits"),
        (47, "FloatingStones"),
        (48, "Shadowfen"),
        (49, "PrismaticPools"),
        (50, "AuroraWastes"),
        (51, "SingingDunes"),
        (52, "Oasis"),
        (53, "GlassDesert"),
        (54, "AbyssalVents"),
        (55, "Sargasso"),
        (56, "EtherealMist"),
        (57, "StarfallCrater"),
        (58, "LeyNexus"),
        (59, "WhisperingStones"),
        (60, "SpiritMarsh"),
        (61, "SulfurVents"),
        (62, "BasaltColumns"),
        (63, "PaintedHills"),
        (64, "RazorPeaks"),
        (65, "SinkholeLakes"),
        (66, "ColossalHive"),
        (67, "BoneFields"),
        (68, "CarnivorousBog"),
        (69, "FungalBloom"),
        (70, "KelpTowers"),
        (71, "BrinePools"),
        (72, "HotSprings"),
        (73, "MirrorLake"),
        (74, "InkSea"),
        (75, "PhosphorShallows"),
        (76, "VoidScar"),
        (77, "SiliconGrove"),
        (78, "SporeWastes"),
        (79, "BleedingStone"),
        (80, "HollowEarth"),
        (81, "SunkenCity"),
        (82, "CyclopeanRuins"),
        (83, "BuriedTemple"),
        (84, "OvergrownCitadel"),
        (85, "DarkTower"),
        (86, "CoralReef"),
        (87, "KelpForest"),
        (88, "SeagrassMeadow"),
        (89, "ContinentalShelf"),
        (90, "Seamount"),
        (91, "OceanicTrench"),
        (92, "AbyssalPlain"),
        (93, "MidOceanRidge"),
        (94, "ColdSeep"),
        (95, "BrinePool"),
        (96, "CrystalDepths"),
        (97, "LeviathanGraveyard"),
        (98, "DrownedCitadel"),
        (99, "VoidMaw"),
        (100, "PearlGardens"),
        (101, "SirenShallows"),
        (102, "FrozenAbyss"),
        (103, "ThermalVents"),
        (104, "KarstPlains"),
        (105, "TowerKarst"),
        (106, "Sinkhole"),
        (107, "Cenote"),
        (108, "CaveEntrance"),
        (109, "CockpitKarst"),
        (110, "Caldera"),
        (111, "ShieldVolcano"),
        (112, "VolcanicCone"),
        (113, "LavaField"),
        (114, "FumaroleField"),
        (115, "VolcanicBeach"),
        (116, "HotSpot"),
    ]
}
