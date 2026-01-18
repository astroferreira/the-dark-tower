//! Debug script to output biomes as ASCII with fantasy biomes

use planet_generator::plates::{generate_plates, calculate_stress};
use planet_generator::heightmap::generate_heightmap;
use planet_generator::climate::{generate_temperature, generate_moisture};
use planet_generator::biomes::{ExtendedBiome, WorldBiomeConfig, classify_extended};
use planet_generator::ascii::{biome_char, ansi_colored_char, biome_fg_color, biome_bg_color};
use noise::{Perlin, Seedable};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use std::fs::File;
use std::io::Write;

fn main() {
    let width = 128;
    let height = 64;
    let seed = 12345u64;

    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    // Generate plates
    let (plate_map, plates) = generate_plates(width, height, None, &mut rng);

    // Generate stress
    let stress_map = calculate_stress(&plate_map, &plates);

    // Generate heightmap
    let heightmap = generate_heightmap(&plate_map, &plates, &stress_map, seed);

    // Generate climate
    let temperature = generate_temperature(&heightmap, width, height);
    let moisture = generate_moisture(&heightmap, width, height);

    // Setup fantasy biome config
    let biome_config = WorldBiomeConfig::default();
    let biome_noise = Perlin::new(1).set_seed(seed as u32);

    // Open output file
    let mut file = File::create("biome_debug.txt").unwrap();

    // Write header
    writeln!(file, "=== EXTENDED BIOME DEBUG MAP ({}x{}) seed={} ===", width, height, seed).unwrap();
    writeln!(file, "Fantasy Intensity: {:.0}%", biome_config.fantasy_intensity * 100.0).unwrap();
    writeln!(file).unwrap();

    // Legend - Base biomes
    writeln!(file, "LEGEND (Base Biomes):").unwrap();
    writeln!(file, "  ~ = Ocean/Water      # = Ice           T = Tundra").unwrap();
    writeln!(file, "  B = Boreal Forest    g = Grassland     f = Temperate Forest").unwrap();
    writeln!(file, "  F = Temp Rainforest  . = Desert        s = Savanna").unwrap();
    writeln!(file, "  t = Tropical Forest  R = Rainforest    A = Alpine  ^ = Snowy").unwrap();
    writeln!(file).unwrap();
    writeln!(file, "LEGEND (Fantasy Biomes):").unwrap();
    writeln!(file, "  D = Dead Forest      C = Crystal Forest    L = Biolum Forest").unwrap();
    writeln!(file, "  M = Mushroom Forest  P = Petrified Forest").unwrap();
    writeln!(file, "  a = Acid Lake        l = Lava Lake         i = Frozen Lake").unwrap();
    writeln!(file, "  b = Biolum Water").unwrap();
    writeln!(file, "  V = Volcanic Waste   S = Salt Flats        H = Ashlands").unwrap();
    writeln!(file, "  X = Crystal Waste").unwrap();
    writeln!(file, "  W = Swamp            m = Marsh             o = Bog").unwrap();
    writeln!(file, "  G = Mangrove").unwrap();
    writeln!(file).unwrap();

    // Count biomes
    let mut biome_counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();

    // Write biome map
    writeln!(file, "BIOME MAP:").unwrap();
    for y in 0..height {
        let mut line = String::new();
        for x in 0..width {
            let h = *heightmap.get(x, y);
            let temp = *temperature.get(x, y);
            let moist = *moisture.get(x, y);
            let stress = *stress_map.get(x, y);

            let biome = classify_extended(
                h, temp, moist, stress,
                x, y, width, height,
                &biome_config, &biome_noise,
            );

            let (ch, name) = match biome {
                // Base biomes
                ExtendedBiome::DeepOcean => ('~', "DeepOcean"),
                ExtendedBiome::Ocean => ('~', "Ocean"),
                ExtendedBiome::CoastalWater => ('~', "CoastalWater"),
                ExtendedBiome::Ice => ('#', "Ice"),
                ExtendedBiome::Tundra => ('T', "Tundra"),
                ExtendedBiome::BorealForest => ('B', "BorealForest"),
                ExtendedBiome::TemperateGrassland => ('g', "TemperateGrassland"),
                ExtendedBiome::TemperateForest => ('f', "TemperateForest"),
                ExtendedBiome::TemperateRainforest => ('F', "TemperateRainforest"),
                ExtendedBiome::Desert => ('.', "Desert"),
                ExtendedBiome::Savanna => ('s', "Savanna"),
                ExtendedBiome::TropicalForest => ('t', "TropicalForest"),
                ExtendedBiome::TropicalRainforest => ('R', "TropicalRainforest"),
                ExtendedBiome::AlpineTundra => ('A', "AlpineTundra"),
                ExtendedBiome::SnowyPeaks => ('^', "SnowyPeaks"),

                // Fantasy Forests
                ExtendedBiome::DeadForest => ('D', "DeadForest"),
                ExtendedBiome::CrystalForest => ('C', "CrystalForest"),
                ExtendedBiome::BioluminescentForest => ('L', "BioluminescentForest"),
                ExtendedBiome::MushroomForest => ('M', "MushroomForest"),
                ExtendedBiome::PetrifiedForest => ('P', "PetrifiedForest"),

                // Fantasy Waters
                ExtendedBiome::AcidLake => ('a', "AcidLake"),
                ExtendedBiome::LavaLake => ('l', "LavaLake"),
                ExtendedBiome::FrozenLake => ('i', "FrozenLake"),
                ExtendedBiome::BioluminescentWater => ('b', "BioluminescentWater"),

                // Wastelands
                ExtendedBiome::VolcanicWasteland => ('V', "VolcanicWasteland"),
                ExtendedBiome::SaltFlats => ('S', "SaltFlats"),
                ExtendedBiome::Ashlands => ('H', "Ashlands"),
                ExtendedBiome::CrystalWasteland => ('X', "CrystalWasteland"),

                // Wetlands
                ExtendedBiome::Swamp => ('W', "Swamp"),
                ExtendedBiome::Marsh => ('m', "Marsh"),
                ExtendedBiome::Bog => ('o', "Bog"),
                ExtendedBiome::MangroveSaltmarsh => ('G', "MangroveSaltmarsh"),

                // Ultra-rare - Ancient/Primeval
                ExtendedBiome::AncientGrove => ('Y', "AncientGrove"),
                ExtendedBiome::TitanBones => ('K', "TitanBones"),
                ExtendedBiome::CoralPlateau => ('c', "CoralPlateau"),

                // Ultra-rare - Geothermal/Volcanic
                ExtendedBiome::ObsidianFields => ('O', "ObsidianFields"),
                ExtendedBiome::Geysers => ('y', "Geysers"),
                ExtendedBiome::TarPits => ('p', "TarPits"),

                // Ultra-rare - Magical/Anomalous
                ExtendedBiome::FloatingStones => ('E', "FloatingStones"),
                ExtendedBiome::Shadowfen => ('Z', "Shadowfen"),
                ExtendedBiome::PrismaticPools => ('Q', "PrismaticPools"),
                ExtendedBiome::AuroraWastes => ('N', "AuroraWastes"),

                // Ultra-rare - Desert variants
                ExtendedBiome::SingingDunes => ('d', "SingingDunes"),
                ExtendedBiome::Oasis => ('I', "Oasis"),
                ExtendedBiome::GlassDesert => ('J', "GlassDesert"),

                // Ultra-rare - Aquatic
                ExtendedBiome::AbyssalVents => ('v', "AbyssalVents"),
                ExtendedBiome::Sargasso => ('w', "Sargasso"),

                // NEW BIOMES - Mystical / Supernatural
                ExtendedBiome::EtherealMist => ('E', "EtherealMist"),
                ExtendedBiome::StarfallCrater => ('U', "StarfallCrater"),
                ExtendedBiome::LeyNexus => ('J', "LeyNexus"),
                ExtendedBiome::WhisperingStones => ('H', "WhisperingStones"),
                ExtendedBiome::SpiritMarsh => ('z', "SpiritMarsh"),

                // NEW BIOMES - Extreme Geological
                ExtendedBiome::SulfurVents => ('u', "SulfurVents"),
                ExtendedBiome::BasaltColumns => ('l', "BasaltColumns"),
                ExtendedBiome::PaintedHills => ('i', "PaintedHills"),
                ExtendedBiome::RazorPeaks => ('j', "RazorPeaks"),
                ExtendedBiome::SinkholeLakes => ('n', "SinkholeLakes"),

                // NEW BIOMES - Biological Wonders
                ExtendedBiome::ColossalHive => ('h', "ColossalHive"),
                ExtendedBiome::BoneFields => ('e', "BoneFields"),
                ExtendedBiome::CarnivorousBog => ('y', "CarnivorousBog"),
                ExtendedBiome::FungalBloom => ('f', "FungalBloom"),
                ExtendedBiome::KelpTowers => ('k', "KelpTowers"),

                // NEW BIOMES - Exotic Waters
                ExtendedBiome::BrinePools => ('q', "BrinePools"),
                ExtendedBiome::HotSprings => ('r', "HotSprings"),
                ExtendedBiome::MirrorLake => ('0', "MirrorLake"),
                ExtendedBiome::InkSea => ('-', "InkSea"),
                ExtendedBiome::PhosphorShallows => ('+', "PhosphorShallows"),

                // NEW BIOMES - Alien / Corrupted
                ExtendedBiome::VoidScar => ('!', "VoidScar"),
                ExtendedBiome::SiliconGrove => ('$', "SiliconGrove"),
                ExtendedBiome::SporeWastes => ('(', "SporeWastes"),
                ExtendedBiome::BleedingStone => (')', "BleedingStone"),
                ExtendedBiome::HollowEarth => ('?', "HollowEarth"),

                // NEW BIOMES - Ancient Ruins
                ExtendedBiome::SunkenCity => ('[', "SunkenCity"),
                ExtendedBiome::CyclopeanRuins => (']', "CyclopeanRuins"),
                ExtendedBiome::BuriedTemple => ('/', "BuriedTemple"),
                ExtendedBiome::OvergrownCitadel => ('\\', "OvergrownCitadel"),
                ExtendedBiome::DarkTower => ('Ω', "DarkTower"),

                // OCEAN BIOMES - Realistic
                ExtendedBiome::CoralReef => ('c', "CoralReef"),
                ExtendedBiome::KelpForest => ('|', "KelpForest"),
                ExtendedBiome::SeagrassMeadow => ('=', "SeagrassMeadow"),
                ExtendedBiome::ContinentalShelf => ('-', "ContinentalShelf"),
                ExtendedBiome::Seamount => ('^', "Seamount"),
                ExtendedBiome::OceanicTrench => ('v', "OceanicTrench"),
                ExtendedBiome::AbyssalPlain => ('_', "AbyssalPlain"),
                ExtendedBiome::MidOceanRidge => ('=', "MidOceanRidge"),
                ExtendedBiome::ColdSeep => ('*', "ColdSeep"),
                ExtendedBiome::BrinePool => ('o', "BrinePool"),

                // OCEAN BIOMES - Fantasy
                ExtendedBiome::CrystalDepths => ('C', "CrystalDepths"),
                ExtendedBiome::LeviathanGraveyard => ('L', "LeviathanGraveyard"),
                ExtendedBiome::DrownedCitadel => ('D', "DrownedCitadel"),
                ExtendedBiome::VoidMaw => ('V', "VoidMaw"),
                ExtendedBiome::PearlGardens => ('P', "PearlGardens"),
                ExtendedBiome::SirenShallows => ('S', "SirenShallows"),
                ExtendedBiome::FrozenAbyss => ('F', "FrozenAbyss"),
                ExtendedBiome::ThermalVents => ('T', "ThermalVents"),

                // Additional biomes
                ExtendedBiome::Foothills => ('h', "Foothills"),
                ExtendedBiome::Lagoon => ('L', "Lagoon"),

                // Karst biomes
                ExtendedBiome::KarstPlains => ('K', "KarstPlains"),
                ExtendedBiome::TowerKarst => ('K', "TowerKarst"),
                ExtendedBiome::Sinkhole => ('K', "Sinkhole"),
                ExtendedBiome::Cenote => ('c', "Cenote"),
                ExtendedBiome::CaveEntrance => ('>', "CaveEntrance"),
                ExtendedBiome::CockpitKarst => ('<', "CockpitKarst"),

                // Volcanic biomes
                ExtendedBiome::Caldera => ('O', "Caldera"),
                ExtendedBiome::ShieldVolcano => ('V', "ShieldVolcano"),
                ExtendedBiome::VolcanicCone => ('^', "VolcanicCone"),
                ExtendedBiome::LavaField => ('~', "LavaField"),
                ExtendedBiome::FumaroleField => ('*', "FumaroleField"),
                ExtendedBiome::VolcanicBeach => ('.', "VolcanicBeach"),
                ExtendedBiome::HotSpot => ('!', "HotSpot"),
            };

            *biome_counts.entry(name).or_insert(0) += 1;
            line.push(ch);
        }
        writeln!(file, "{}", line).unwrap();
    }

    // Write statistics
    writeln!(file).unwrap();
    writeln!(file, "=== BIOME STATISTICS ===").unwrap();
    let total = (width * height) as f32;
    let mut sorted_biomes: Vec<_> = biome_counts.iter().collect();
    sorted_biomes.sort_by(|a, b| b.1.cmp(a.1));

    // Separate base and fantasy biomes
    let fantasy_names = ["DeadForest", "CrystalForest", "BioluminescentForest", "MushroomForest",
                         "PetrifiedForest", "AcidLake", "LavaLake", "FrozenLake", "BioluminescentWater",
                         "VolcanicWasteland", "SaltFlats", "Ashlands", "CrystalWasteland",
                         "Swamp", "Marsh", "Bog", "MangroveSaltmarsh"];

    writeln!(file, "\nBase Biomes:").unwrap();
    for (name, count) in &sorted_biomes {
        if !fantasy_names.contains(name) {
            let pct = (**count as f32 / total) * 100.0;
            writeln!(file, "  {:24} {:5} ({:5.1}%)", name, count, pct).unwrap();
        }
    }

    writeln!(file, "\nFantasy Biomes:").unwrap();
    let mut fantasy_total = 0usize;
    for (name, count) in &sorted_biomes {
        if fantasy_names.contains(name) {
            let pct = (**count as f32 / total) * 100.0;
            writeln!(file, "  {:24} {:5} ({:5.1}%)", name, count, pct).unwrap();
            fantasy_total += **count;
        }
    }
    let fantasy_pct = (fantasy_total as f32 / total) * 100.0;
    writeln!(file, "  {:24} {:5} ({:5.1}%)", "TOTAL FANTASY", fantasy_total, fantasy_pct).unwrap();

    // Write moisture statistics
    writeln!(file).unwrap();
    writeln!(file, "=== MOISTURE STATISTICS (land only) ===").unwrap();
    let mut land_moistures: Vec<f32> = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let h = *heightmap.get(x, y);
            if h > 0.0 {
                land_moistures.push(*moisture.get(x, y));
            }
        }
    }
    if !land_moistures.is_empty() {
        land_moistures.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let min = land_moistures[0];
        let max = land_moistures[land_moistures.len() - 1];
        let median = land_moistures[land_moistures.len() / 2];
        let mean: f32 = land_moistures.iter().sum::<f32>() / land_moistures.len() as f32;
        let q1 = land_moistures[land_moistures.len() / 4];
        let q3 = land_moistures[3 * land_moistures.len() / 4];

        writeln!(file, "  Min:    {:.3}", min).unwrap();
        writeln!(file, "  Q1:     {:.3}", q1).unwrap();
        writeln!(file, "  Median: {:.3}", median).unwrap();
        writeln!(file, "  Mean:   {:.3}", mean).unwrap();
        writeln!(file, "  Q3:     {:.3}", q3).unwrap();
        writeln!(file, "  Max:    {:.3}", max).unwrap();
    }

    // Write temperature statistics
    writeln!(file).unwrap();
    writeln!(file, "=== TEMPERATURE STATISTICS (land only) ===").unwrap();
    let mut land_temps: Vec<f32> = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let h = *heightmap.get(x, y);
            if h > 0.0 {
                land_temps.push(*temperature.get(x, y));
            }
        }
    }
    if !land_temps.is_empty() {
        land_temps.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let min = land_temps[0];
        let max = land_temps[land_temps.len() - 1];
        let median = land_temps[land_temps.len() / 2];
        let mean: f32 = land_temps.iter().sum::<f32>() / land_temps.len() as f32;

        writeln!(file, "  Min:    {:.1}°C", min).unwrap();
        writeln!(file, "  Median: {:.1}°C", median).unwrap();
        writeln!(file, "  Mean:   {:.1}°C", mean).unwrap();
        writeln!(file, "  Max:    {:.1}°C", max).unwrap();
    }

    // Sample some specific cells with extended biomes
    writeln!(file).unwrap();
    writeln!(file, "=== SAMPLE CELLS ===").unwrap();
    let samples = [(32, 16), (64, 32), (96, 48), (64, 8), (64, 56), (20, 30), (100, 40)];
    for (x, y) in samples {
        let h = *heightmap.get(x, y);
        let temp = *temperature.get(x, y);
        let moist = *moisture.get(x, y);
        let stress = *stress_map.get(x, y);
        let biome = classify_extended(
            h, temp, moist, stress,
            x, y, width, height,
            &biome_config, &biome_noise,
        );
        writeln!(file, "  ({:3},{:3}): h={:7.1}m, t={:6.1}°C, m={:.3}, s={:6.1} => {}",
                 x, y, h, temp, moist, stress, biome.display_name()).unwrap();
    }

    println!("Debug output written to biome_debug.txt");

    // Print a small colorized preview to terminal (40x20 region from center)
    println!("\n=== COLORIZED TERMINAL PREVIEW (40x20 from center) ===\n");
    let preview_w = 40;
    let preview_h = 20;
    let start_x = (width - preview_w) / 2;
    let start_y = (height - preview_h) / 2;

    for py in 0..preview_h {
        let y = start_y + py;
        for px in 0..preview_w {
            let x = start_x + px;
            let h = *heightmap.get(x, y);
            let temp = *temperature.get(x, y);
            let moist = *moisture.get(x, y);
            let stress = *stress_map.get(x, y);

            let biome = classify_extended(
                h, temp, moist, stress,
                x, y, width, height,
                &biome_config, &biome_noise,
            );

            let ch = biome_char(&biome);
            let fg = biome_fg_color(&biome);
            let bg = biome_bg_color(&biome);
            print!("{}", ansi_colored_char(ch, fg, bg));
        }
        println!("\x1b[0m"); // Reset colors at end of line
    }

    // Print a few biome samples with colors
    println!("\n=== BIOME COLOR SAMPLES ===\n");
    let sample_biomes = [
        ExtendedBiome::Ocean,
        ExtendedBiome::DeepOcean,
        ExtendedBiome::Ice,
        ExtendedBiome::Tundra,
        ExtendedBiome::BorealForest,
        ExtendedBiome::TemperateForest,
        ExtendedBiome::TropicalRainforest,
        ExtendedBiome::Desert,
        ExtendedBiome::Savanna,
        ExtendedBiome::SnowyPeaks,
        // Fantasy
        ExtendedBiome::LavaLake,
        ExtendedBiome::CrystalForest,
        ExtendedBiome::MushroomForest,
        ExtendedBiome::BioluminescentForest,
        ExtendedBiome::VolcanicWasteland,
        ExtendedBiome::Swamp,
        // Ultra-rare
        ExtendedBiome::VoidScar,
        ExtendedBiome::StarfallCrater,
        ExtendedBiome::SunkenCity,
        ExtendedBiome::EtherealMist,
    ];

    for biome in &sample_biomes {
        let ch = biome_char(biome);
        let fg = biome_fg_color(biome);
        let bg = biome_bg_color(biome);
        let colored = ansi_colored_char(ch, fg, bg);
        println!("  {} {} - {}", colored, colored, biome.display_name());
    }
    println!();
}
