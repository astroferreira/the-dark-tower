//! ASCII rendering and export module for world maps
//!
//! Provides functions to render world data as ASCII text and export to files.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Write};
use chrono::Local;

use crate::biomes::ExtendedBiome;
use crate::plates::{PlateId, Plate, PlateType};
use crate::scale::MapScale;
use crate::tilemap::Tilemap;

/// ASCII rendering modes
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AsciiMode {
    /// Show biome characters
    Biome,
    /// Show elevation gradient
    Height,
    /// Show temperature gradient
    Temperature,
    /// Show moisture gradient
    Moisture,
    /// Show plate IDs
    Plates,
    /// Show stress (convergent/divergent)
    Stress,
}

impl AsciiMode {
    pub fn name(&self) -> &'static str {
        match self {
            AsciiMode::Biome => "Biome",
            AsciiMode::Height => "Height",
            AsciiMode::Temperature => "Temperature",
            AsciiMode::Moisture => "Moisture",
            AsciiMode::Plates => "Plates",
            AsciiMode::Stress => "Stress",
        }
    }

    pub fn all() -> &'static [AsciiMode] {
        &[
            AsciiMode::Biome,
            AsciiMode::Height,
            AsciiMode::Temperature,
            AsciiMode::Moisture,
            AsciiMode::Plates,
            AsciiMode::Stress,
        ]
    }
}

/// Get ASCII character for a biome
pub fn biome_char(biome: &ExtendedBiome) -> char {
    match biome {
        // Water biomes
        ExtendedBiome::DeepOcean => '~',
        ExtendedBiome::Ocean => '.',
        ExtendedBiome::CoastalWater => ',',

        // Cold biomes
        ExtendedBiome::Ice => '#',
        ExtendedBiome::Tundra => ':',
        ExtendedBiome::BorealForest => 'B',

        // Temperate biomes
        ExtendedBiome::TemperateGrassland => '"',
        ExtendedBiome::TemperateForest => 'T',
        ExtendedBiome::TemperateRainforest => 'R',

        // Warm biomes
        ExtendedBiome::Desert => 'd',
        ExtendedBiome::Savanna => ';',
        ExtendedBiome::TropicalForest => 't',
        ExtendedBiome::TropicalRainforest => 'r',

        // Mountain biomes
        ExtendedBiome::AlpineTundra => '^',
        ExtendedBiome::SnowyPeaks => 'A',
        ExtendedBiome::Foothills => 'n',    // n for rolling hills shape
        ExtendedBiome::Lagoon => '=',       // = for calm protected water

        // Fantasy forests
        ExtendedBiome::DeadForest => 'X',
        ExtendedBiome::CrystalForest => 'C',
        ExtendedBiome::BioluminescentForest => '*',
        ExtendedBiome::MushroomForest => 'M',
        ExtendedBiome::PetrifiedForest => 'P',

        // Fantasy waters
        ExtendedBiome::AcidLake => 'a',
        ExtendedBiome::LavaLake => '@',
        ExtendedBiome::FrozenLake => 'o',
        ExtendedBiome::BioluminescentWater => 'b',

        // Wastelands
        ExtendedBiome::VolcanicWasteland => 'V',
        ExtendedBiome::SaltFlats => '_',
        ExtendedBiome::Ashlands => '%',
        ExtendedBiome::CrystalWasteland => 'c',

        // Wetlands
        ExtendedBiome::Swamp => 'S',
        ExtendedBiome::Marsh => 'm',
        ExtendedBiome::Bog => '&',
        ExtendedBiome::MangroveSaltmarsh => 'G',

        // Ultra-rare - Ancient/Primeval
        ExtendedBiome::AncientGrove => 'Y',      // Y for Yggdrasil-like ancient trees
        ExtendedBiome::TitanBones => 'W',        // W for skeletal remains
        ExtendedBiome::CoralPlateau => 'K',      // K for coral

        // Ultra-rare - Geothermal/Volcanic
        ExtendedBiome::ObsidianFields => 'O',    // O for obsidian
        ExtendedBiome::Geysers => 'g',           // g for geyser
        ExtendedBiome::TarPits => 'p',           // p for pit

        // Ultra-rare - Magical/Anomalous
        ExtendedBiome::FloatingStones => 'F',    // F for floating
        ExtendedBiome::Shadowfen => 'Z',         // Z for dark/shadow
        ExtendedBiome::PrismaticPools => 'Q',    // Q for iridescent
        ExtendedBiome::AuroraWastes => 'N',      // N for northern lights

        // Ultra-rare - Desert variants
        ExtendedBiome::SingingDunes => 'D',      // D for dunes
        ExtendedBiome::Oasis => 'I',             // I for island of green
        ExtendedBiome::GlassDesert => 'L',       // L for glass/crystal

        // Ultra-rare - Aquatic
        ExtendedBiome::AbyssalVents => 'v',      // v for vent
        ExtendedBiome::Sargasso => 'w',          // w for weed/seaweed

        // NEW BIOMES - Mystical / Supernatural
        ExtendedBiome::EtherealMist => 'E',      // E for ethereal
        ExtendedBiome::StarfallCrater => 'U',    // U for crater shape
        ExtendedBiome::LeyNexus => 'J',          // J for junction
        ExtendedBiome::WhisperingStones => 'H',  // H for henge
        ExtendedBiome::SpiritMarsh => 'z',       // z for spectral

        // NEW BIOMES - Extreme Geological
        ExtendedBiome::SulfurVents => 'u',       // u for sulfur
        ExtendedBiome::BasaltColumns => 'l',     // l for columns (looks like |)
        ExtendedBiome::PaintedHills => 'i',      // i for layers
        ExtendedBiome::RazorPeaks => 'j',        // j for jagged
        ExtendedBiome::SinkholeLakes => 'n',     // n for sinkhole

        // NEW BIOMES - Biological Wonders
        ExtendedBiome::ColossalHive => 'h',      // h for hive
        ExtendedBiome::BoneFields => 'e',        // e for extinction
        ExtendedBiome::CarnivorousBog => 'y',    // y for hungry/carnivorous
        ExtendedBiome::FungalBloom => 'f',       // f for fungal
        ExtendedBiome::KelpTowers => 'k',        // k for kelp

        // NEW BIOMES - Exotic Waters
        ExtendedBiome::BrinePools => 'q',        // q for saline
        ExtendedBiome::HotSprings => 's',        // s for springs
        ExtendedBiome::MirrorLake => '0',        // 0 for mirror/reflection
        ExtendedBiome::InkSea => '-',            // - for dark/deep
        ExtendedBiome::PhosphorShallows => '+',  // + for glowing

        // NEW BIOMES - Alien / Corrupted
        ExtendedBiome::VoidScar => '!',          // ! for danger/void
        ExtendedBiome::SiliconGrove => '$',      // $ for crystalline
        ExtendedBiome::SporeWastes => '(',       // ( for spreading
        ExtendedBiome::BleedingStone => ')',     // ) for oozing
        ExtendedBiome::HollowEarth => '?',       // ? for mystery/depth

        // NEW BIOMES - Ancient Ruins
        ExtendedBiome::SunkenCity => '[',        // [ for submerged structures
        ExtendedBiome::CyclopeanRuins => ']',    // ] for massive blocks
        ExtendedBiome::BuriedTemple => '/',      // / for sand-covered
        ExtendedBiome::OvergrownCitadel => '\\', // \ for vine-covered
        ExtendedBiome::DarkTower => 'Ω',        // Ω for the singular dark tower

        // OCEAN BIOMES - Realistic Shallow/Coastal
        ExtendedBiome::CoralReef => '⌇',         // coral branches
        ExtendedBiome::KelpForest => '|',        // vertical kelp fronds
        ExtendedBiome::SeagrassMeadow => '≈',    // gentle waves/grass

        // OCEAN BIOMES - Realistic Mid-depth
        ExtendedBiome::ContinentalShelf => '─',  // flat shelf
        ExtendedBiome::Seamount => '▲',          // underwater mountain

        // OCEAN BIOMES - Realistic Deep
        ExtendedBiome::OceanicTrench => '▼',     // deep trench
        ExtendedBiome::AbyssalPlain => '░',      // flat deep floor
        ExtendedBiome::MidOceanRidge => '═',     // spreading ridge
        ExtendedBiome::ColdSeep => '●',          // seep area
        ExtendedBiome::BrinePool => '○',         // underwater pool

        // OCEAN BIOMES - Fantasy
        ExtendedBiome::CrystalDepths => '◆',     // crystal formation
        ExtendedBiome::LeviathanGraveyard => '†', // bones/death
        ExtendedBiome::DrownedCitadel => '▓',    // sunken structure
        ExtendedBiome::VoidMaw => '◎',           // dark void
        ExtendedBiome::PearlGardens => '◇',      // pearl/gem
        ExtendedBiome::SirenShallows => '♪',     // musical/enchanted
        ExtendedBiome::FrozenAbyss => '❄',       // frozen deep
        ExtendedBiome::ThermalVents => '♨',      // thermal/hot

        // Karst & Cave biomes
        ExtendedBiome::KarstPlains => 'K',       // K for karst
        ExtendedBiome::TowerKarst => '▲',        // pointed towers
        ExtendedBiome::Sinkhole => '○',          // circular depression (hollow circle)
        ExtendedBiome::Cenote => '@',            // water-filled hole
        ExtendedBiome::CaveEntrance => '(',      // cave opening
        ExtendedBiome::CockpitKarst => 'π',      // star pattern

        // Volcanic biomes
        ExtendedBiome::Caldera => 'Θ',           // crater ring
        ExtendedBiome::ShieldVolcano => '∩',     // broad dome shape
        ExtendedBiome::VolcanicCone => '△',      // conical peak
        ExtendedBiome::LavaField => '▬',         // flat lava flow
        ExtendedBiome::FumaroleField => '≋',     // steam vents
        ExtendedBiome::VolcanicBeach => '▪',     // black sand
        ExtendedBiome::HotSpot => '●',           // active hot spot
    }
}

/// Get ASCII character for elevation (11-level gradient)
pub fn height_char(elevation: f32) -> char {
    // Deep ocean to high peaks: -4000m to +4000m
    const CHARS: &[char] = &['~', '.', '-', '=', '+', '*', '#', '%', '^', 'A', 'M'];
    let normalized = ((elevation + 4000.0) / 8000.0).clamp(0.0, 1.0);
    let idx = (normalized * (CHARS.len() - 1) as f32) as usize;
    CHARS[idx.min(CHARS.len() - 1)]
}

/// Get ASCII character for temperature
pub fn temperature_char(temp: f32) -> char {
    // -30°C to +30°C
    const CHARS: &[char] = &['#', '=', '-', '.', ',', ';', ':', '+', '*', '@'];
    let normalized = ((temp + 30.0) / 60.0).clamp(0.0, 1.0);
    let idx = (normalized * (CHARS.len() - 1) as f32) as usize;
    CHARS[idx.min(CHARS.len() - 1)]
}

/// Get ASCII character for moisture
pub fn moisture_char(moisture: f32) -> char {
    // 0.0 to 1.0
    const CHARS: &[char] = &['_', '.', '-', ':', ';', '=', '+', '#', '%', '~'];
    let idx = (moisture * (CHARS.len() - 1) as f32) as usize;
    CHARS[idx.min(CHARS.len() - 1)]
}

/// Get ASCII character for stress
pub fn stress_char(stress: f32) -> char {
    // -1.0 (divergent) to +1.0 (convergent)
    if stress > 0.5 {
        '^' // Strong convergent (mountains)
    } else if stress > 0.2 {
        '+' // Moderate convergent
    } else if stress > 0.05 {
        '=' // Weak convergent
    } else if stress > -0.05 {
        '.' // Neutral
    } else if stress > -0.2 {
        '-' // Weak divergent
    } else if stress > -0.5 {
        'v' // Moderate divergent
    } else {
        '~' // Strong divergent (rifts)
    }
}

/// Get ASCII character for plate ID
pub fn plate_char(plate_id: PlateId, plates: &[Plate]) -> char {
    if plate_id.is_none() {
        ' '
    } else {
        let plate = &plates[plate_id.0 as usize];
        let base = if plate.plate_type == PlateType::Continental { 'A' } else { 'a' };
        let offset = (plate_id.0 % 26) as u8;
        (base as u8 + offset) as char
    }
}

/// Render a map to ASCII string
pub fn render_ascii_map(
    heightmap: &Tilemap<f32>,
    biomes: &Tilemap<ExtendedBiome>,
    temperature: &Tilemap<f32>,
    moisture: &Tilemap<f32>,
    stress_map: &Tilemap<f32>,
    plate_map: &Tilemap<PlateId>,
    plates: &[Plate],
    mode: AsciiMode,
) -> String {
    let width = heightmap.width;
    let height = heightmap.height;
    let mut result = String::with_capacity((width + 1) * height);

    for y in 0..height {
        for x in 0..width {
            let ch = match mode {
                AsciiMode::Biome => biome_char(biomes.get(x, y)),
                AsciiMode::Height => height_char(*heightmap.get(x, y)),
                AsciiMode::Temperature => temperature_char(*temperature.get(x, y)),
                AsciiMode::Moisture => moisture_char(*moisture.get(x, y)),
                AsciiMode::Stress => stress_char(*stress_map.get(x, y)),
                AsciiMode::Plates => plate_char(*plate_map.get(x, y), plates),
            };
            result.push(ch);
        }
        result.push('\n');
    }

    result
}

/// Generate legend for biome characters
pub fn biome_legend() -> String {
    let mut legend = String::new();
    legend.push_str("=== BIOME LEGEND ===\n");
    legend.push_str("WATER:\n");
    legend.push_str("  ~ DeepOcean    . Ocean       , Coastal\n");
    legend.push_str("COLD:\n");
    legend.push_str("  # Ice          : Tundra      B Boreal\n");
    legend.push_str("TEMPERATE:\n");
    legend.push_str("  \" Grassland    T TempForest  R TempRain\n");
    legend.push_str("WARM:\n");
    legend.push_str("  d Desert       ; Savanna     t TropForest  r TropRain\n");
    legend.push_str("MOUNTAIN:\n");
    legend.push_str("  ^ Alpine       A SnowyPeak\n");
    legend.push_str("FANTASY FORESTS:\n");
    legend.push_str("  X Dead         C Crystal     * Biolum      M Mushroom   P Petrified\n");
    legend.push_str("FANTASY WATERS:\n");
    legend.push_str("  a Acid         @ Lava        o Frozen      b BiolumWater\n");
    legend.push_str("WASTELANDS:\n");
    legend.push_str("  V Volcanic     _ Salt        % Ashlands    c CrystalWaste\n");
    legend.push_str("WETLANDS:\n");
    legend.push_str("  S Swamp        m Marsh       & Bog         G Mangrove\n");
    legend.push_str("ULTRA-RARE (Ancient):\n");
    legend.push_str("  Y AncientGrove W TitanBones  K CoralPlateau\n");
    legend.push_str("ULTRA-RARE (Geothermal):\n");
    legend.push_str("  O Obsidian     g Geysers     p TarPits\n");
    legend.push_str("ULTRA-RARE (Anomalous):\n");
    legend.push_str("  F Floating     Z Shadowfen   Q Prismatic   N Aurora\n");
    legend.push_str("ULTRA-RARE (Desert):\n");
    legend.push_str("  D SingingDunes I Oasis       L GlassDesert\n");
    legend.push_str("ULTRA-RARE (Aquatic):\n");
    legend.push_str("  v AbyssalVent  w Sargasso\n");
    legend.push_str("MYSTICAL:\n");
    legend.push_str("  E Ethereal     U Starfall    J LeyNexus    H Whisper    z Spirit\n");
    legend.push_str("GEOLOGICAL:\n");
    legend.push_str("  u Sulfur       l Basalt      i Painted     j Razor      n Sinkhole\n");
    legend.push_str("BIOLOGICAL:\n");
    legend.push_str("  h Hive         e Bone        y Carnivorous f Fungal     k Kelp\n");
    legend.push_str("EXOTIC WATERS:\n");
    legend.push_str("  q Brine        s HotSpring   0 Mirror      - Ink        + Phosphor\n");
    legend.push_str("ALIEN/CORRUPTED:\n");
    legend.push_str("  ! Void         $ Silicon     ( Spore       ) Bleeding   ? Hollow\n");
    legend.push_str("ANCIENT RUINS:\n");
    legend.push_str("  [ SunkenCity   ] Cyclopean   / Buried      \\ Overgrown\n");
    legend
}

/// Generate height legend
pub fn height_legend() -> String {
    "=== HEIGHT LEGEND ===\n\
     Deep ocean → High peaks:\n\
     ~ . - = + * # % ^ A M\n\
     (-4000m)        (+4000m)\n".to_string()
}

/// Calculate biome statistics
pub fn calculate_biome_stats(biomes: &Tilemap<ExtendedBiome>) -> HashMap<ExtendedBiome, usize> {
    let mut stats = HashMap::new();
    for y in 0..biomes.height {
        for x in 0..biomes.width {
            let biome = *biomes.get(x, y);
            *stats.entry(biome).or_insert(0) += 1;
        }
    }
    stats
}

/// Export world data to ASCII file
pub fn export_world_file(
    heightmap: &Tilemap<f32>,
    biomes: &Tilemap<ExtendedBiome>,
    temperature: &Tilemap<f32>,
    moisture: &Tilemap<f32>,
    stress_map: &Tilemap<f32>,
    plate_map: &Tilemap<PlateId>,
    plates: &[Plate],
    scale: &MapScale,
    seed: u64,
    path: &str,
    verbose: bool,
) -> io::Result<()> {
    let mut file = File::create(path)?;
    let width = heightmap.width;
    let height = heightmap.height;
    let total = width * height;

    // Header
    writeln!(file, "=== PLANET GENERATOR WORLD FILE ===")?;
    writeln!(file, "Seed: {}", seed)?;
    writeln!(file, "Size: {}x{}", width, height)?;
    let (w_km, h_km) = scale.map_size_km(width, height);
    writeln!(file, "Scale: {:.1} km/tile ({:.0} x {:.0} km)", scale.km_per_tile, w_km, h_km)?;
    writeln!(file, "Generated: {}", Local::now().format("%Y-%m-%d %H:%M:%S"))?;
    writeln!(file)?;

    // Biome map
    writeln!(file, "=== MAP (Biome View) ===")?;
    let map_str = render_ascii_map(
        heightmap, biomes, temperature, moisture, stress_map, plate_map, plates,
        AsciiMode::Biome,
    );
    write!(file, "{}", map_str)?;
    writeln!(file)?;

    // Legend
    write!(file, "{}", biome_legend())?;
    writeln!(file)?;

    // Statistics
    writeln!(file, "=== STATISTICS ===")?;
    writeln!(file, "Total tiles: {}", total)?;

    // Land/water count
    let mut land_count = 0;
    let mut water_count = 0;
    for y in 0..height {
        for x in 0..width {
            if *heightmap.get(x, y) > 0.0 {
                land_count += 1;
            } else {
                water_count += 1;
            }
        }
    }
    writeln!(file, "Land: {} ({:.1}%)", land_count, 100.0 * land_count as f64 / total as f64)?;
    writeln!(file, "Water: {} ({:.1}%)", water_count, 100.0 * water_count as f64 / total as f64)?;
    writeln!(file)?;

    // Biome distribution
    writeln!(file, "Biome Distribution:")?;
    let stats = calculate_biome_stats(biomes);
    let mut sorted_stats: Vec<_> = stats.iter().collect();
    sorted_stats.sort_by(|a, b| b.1.cmp(a.1)); // Sort by count descending
    for (biome, count) in sorted_stats {
        let pct = 100.0 * *count as f64 / total as f64;
        writeln!(file, "  {:20} {} {:>6} ({:>5.1}%)", biome.display_name(), biome_char(biome), count, pct)?;
    }
    writeln!(file)?;

    // Elevation stats
    let mut min_h = f32::MAX;
    let mut max_h = f32::MIN;
    let mut sum_h = 0.0f64;
    for y in 0..height {
        for x in 0..width {
            let h = *heightmap.get(x, y);
            min_h = min_h.min(h);
            max_h = max_h.max(h);
            sum_h += h as f64;
        }
    }
    writeln!(file, "Elevation:")?;
    writeln!(file, "  Min: {:.1}m  Max: {:.1}m  Mean: {:.1}m", min_h, max_h, sum_h / total as f64)?;
    writeln!(file)?;

    // Temperature stats
    let mut min_t = f32::MAX;
    let mut max_t = f32::MIN;
    for y in 0..height {
        for x in 0..width {
            let t = *temperature.get(x, y);
            min_t = min_t.min(t);
            max_t = max_t.max(t);
        }
    }
    writeln!(file, "Temperature: {:.1}°C to {:.1}°C", min_t, max_t)?;
    writeln!(file)?;

    // Plate stats
    let continental = plates.iter().filter(|p| p.plate_type == PlateType::Continental).count();
    let oceanic = plates.iter().filter(|p| p.plate_type == PlateType::Oceanic).count();
    writeln!(file, "Plates: {} total ({} continental, {} oceanic)", plates.len(), continental, oceanic)?;
    writeln!(file)?;

    // Verbose tile data
    if verbose {
        writeln!(file, "=== TILE DATA ===")?;
        writeln!(file, "[x,y,elevation,temperature,moisture,biome,stress]")?;
        for y in 0..height {
            for x in 0..width {
                let h = *heightmap.get(x, y);
                let t = *temperature.get(x, y);
                let m = *moisture.get(x, y);
                let b = biomes.get(x, y).display_name();
                let s = *stress_map.get(x, y);
                writeln!(file, "{},{},{:.1},{:.1},{:.2},{},{:.3}", x, y, h, t, m, b, s)?;
            }
        }
    }

    Ok(())
}

/// Print ASCII map to stdout
pub fn print_ascii_map(
    heightmap: &Tilemap<f32>,
    biomes: &Tilemap<ExtendedBiome>,
    temperature: &Tilemap<f32>,
    moisture: &Tilemap<f32>,
    stress_map: &Tilemap<f32>,
    plate_map: &Tilemap<PlateId>,
    plates: &[Plate],
    mode: AsciiMode,
) {
    let map_str = render_ascii_map(
        heightmap, biomes, temperature, moisture, stress_map, plate_map, plates,
        mode,
    );
    print!("{}", map_str);
}

// ============================================================================
// COLORIZED ASCII RENDERING
// ============================================================================

/// Get foreground color for a biome (lighter/contrasting color for the character)
pub fn biome_fg_color(biome: &ExtendedBiome) -> (u8, u8, u8) {
    // Use a contrasting/lighter color for readability on the background
    let (r, g, b) = biome.color();
    // Brighten the color for foreground or use white/black based on luminance
    let luminance = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
    if luminance > 128.0 {
        // Dark background color -> use darker foreground
        (r.saturating_sub(60), g.saturating_sub(60), b.saturating_sub(60))
    } else {
        // Light background color -> use brighter foreground
        (r.saturating_add(80).min(255), g.saturating_add(80).min(255), b.saturating_add(80).min(255))
    }
}

/// Get background color for a biome (the base biome color)
pub fn biome_bg_color(biome: &ExtendedBiome) -> (u8, u8, u8) {
    biome.color()
}

/// Format a single character with ANSI true color (24-bit) - foreground and background
pub fn ansi_colored_char(ch: char, fg: (u8, u8, u8), bg: (u8, u8, u8)) -> String {
    format!(
        "\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}m{}\x1b[0m",
        fg.0, fg.1, fg.2,
        bg.0, bg.1, bg.2,
        ch
    )
}

/// Format a string with ANSI true color (24-bit) - foreground only
pub fn ansi_fg_colored(text: &str, fg: (u8, u8, u8)) -> String {
    format!("\x1b[38;2;{};{};{}m{}\x1b[0m", fg.0, fg.1, fg.2, text)
}

/// Format a string with ANSI true color (24-bit) - foreground and background
pub fn ansi_colored(text: &str, fg: (u8, u8, u8), bg: (u8, u8, u8)) -> String {
    format!(
        "\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}m{}\x1b[0m",
        fg.0, fg.1, fg.2,
        bg.0, bg.1, bg.2,
        text
    )
}

/// Get color for elevation gradient
pub fn height_color(elevation: f32) -> (u8, u8, u8) {
    // Deep ocean to high peaks: -4000m to +4000m
    let normalized = ((elevation + 4000.0) / 8000.0).clamp(0.0, 1.0);

    if normalized < 0.4 {
        // Ocean: deep blue to light blue
        let t = normalized / 0.4;
        let r = (20.0 + t * 40.0) as u8;
        let g = (40.0 + t * 60.0) as u8;
        let b = (100.0 + t * 60.0) as u8;
        (r, g, b)
    } else if normalized < 0.5 {
        // Coastal/Beach: sandy
        (210, 190, 140)
    } else if normalized < 0.65 {
        // Lowlands: green
        let t = (normalized - 0.5) / 0.15;
        let r = (80.0 - t * 30.0) as u8;
        let g = (140.0 + t * 20.0) as u8;
        let b = (60.0 - t * 20.0) as u8;
        (r, g, b)
    } else if normalized < 0.8 {
        // Hills: brown/tan
        let t = (normalized - 0.65) / 0.15;
        let r = (100.0 + t * 40.0) as u8;
        let g = (100.0 - t * 20.0) as u8;
        let b = (70.0 - t * 20.0) as u8;
        (r, g, b)
    } else if normalized < 0.92 {
        // Mountains: gray rock
        let t = (normalized - 0.8) / 0.12;
        let r = (120.0 + t * 40.0) as u8;
        let g = (110.0 + t * 40.0) as u8;
        let b = (100.0 + t * 50.0) as u8;
        (r, g, b)
    } else {
        // Snow peaks: white
        let t = (normalized - 0.92) / 0.08;
        let v = (200.0 + t * 55.0) as u8;
        (v, v, v.min(255))
    }
}

/// Get color for temperature gradient
pub fn temperature_color(temp: f32) -> (u8, u8, u8) {
    // -30°C to +30°C: blue (cold) -> white (mild) -> red/orange (hot)
    let normalized = ((temp + 30.0) / 60.0).clamp(0.0, 1.0);

    if normalized < 0.3 {
        // Cold: deep blue to cyan
        let t = normalized / 0.3;
        let r = (50.0 + t * 100.0) as u8;
        let g = (100.0 + t * 100.0) as u8;
        let b = (200.0 + t * 55.0) as u8;
        (r, g, b)
    } else if normalized < 0.5 {
        // Cool: cyan to green
        let t = (normalized - 0.3) / 0.2;
        let r = (150.0 - t * 50.0) as u8;
        let g = (200.0 + t * 30.0) as u8;
        let b = (255.0 - t * 155.0) as u8;
        (r, g, b)
    } else if normalized < 0.7 {
        // Mild: green to yellow
        let t = (normalized - 0.5) / 0.2;
        let r = (100.0 + t * 155.0) as u8;
        let g = (230.0 - t * 30.0) as u8;
        let b = (100.0 - t * 50.0) as u8;
        (r, g, b)
    } else {
        // Hot: yellow to red
        let t = (normalized - 0.7) / 0.3;
        let r = 255;
        let g = (200.0 - t * 150.0) as u8;
        let b = (50.0 + t * 20.0) as u8;
        (r, g, b)
    }
}

/// Get color for moisture gradient
pub fn moisture_color(moisture: f32) -> (u8, u8, u8) {
    // 0.0 (dry) to 1.0 (wet): tan/brown -> green -> blue
    let m = moisture.clamp(0.0, 1.0);

    if m < 0.3 {
        // Dry: tan/brown
        let t = m / 0.3;
        let r = (210.0 - t * 50.0) as u8;
        let g = (180.0 - t * 30.0) as u8;
        let b = (120.0 + t * 30.0) as u8;
        (r, g, b)
    } else if m < 0.6 {
        // Moderate: greenish
        let t = (m - 0.3) / 0.3;
        let r = (160.0 - t * 80.0) as u8;
        let g = (150.0 + t * 50.0) as u8;
        let b = (150.0 - t * 50.0) as u8;
        (r, g, b)
    } else {
        // Wet: blue-green to blue
        let t = (m - 0.6) / 0.4;
        let r = (80.0 - t * 40.0) as u8;
        let g = (200.0 - t * 80.0) as u8;
        let b = (100.0 + t * 100.0) as u8;
        (r, g, b)
    }
}

/// Get color for stress gradient
pub fn stress_color(stress: f32) -> (u8, u8, u8) {
    // -1.0 (divergent/blue) to +1.0 (convergent/red)
    let s = stress.clamp(-1.0, 1.0);

    if s < -0.3 {
        // Strong divergent: blue
        let t = (-s - 0.3) / 0.7;
        (40, (80.0 + t * 80.0) as u8, (180.0 + t * 75.0) as u8)
    } else if s < 0.0 {
        // Weak divergent: cyan/neutral
        let t = -s / 0.3;
        ((100.0 - t * 60.0) as u8, (140.0 - t * 60.0) as u8, (140.0 + t * 40.0) as u8)
    } else if s < 0.3 {
        // Weak convergent: neutral/yellow
        let t = s / 0.3;
        ((100.0 + t * 80.0) as u8, (140.0 + t * 40.0) as u8, (140.0 - t * 80.0) as u8)
    } else {
        // Strong convergent: orange/red
        let t = (s - 0.3) / 0.7;
        ((180.0 + t * 75.0) as u8, (180.0 - t * 120.0) as u8, (60.0 - t * 30.0) as u8)
    }
}

/// Render a colorized ASCII map to string with ANSI codes
pub fn render_colored_ascii_map(
    heightmap: &Tilemap<f32>,
    biomes: &Tilemap<ExtendedBiome>,
    temperature: &Tilemap<f32>,
    moisture: &Tilemap<f32>,
    stress_map: &Tilemap<f32>,
    plate_map: &Tilemap<PlateId>,
    plates: &[Plate],
    mode: AsciiMode,
) -> String {
    let width = heightmap.width;
    let height = heightmap.height;
    // Estimate: each cell needs ~40 bytes for ANSI codes
    let mut result = String::with_capacity(width * height * 45);

    for y in 0..height {
        for x in 0..width {
            let (ch, fg, bg) = match mode {
                AsciiMode::Biome => {
                    let biome = biomes.get(x, y);
                    (biome_char(biome), biome_fg_color(biome), biome_bg_color(biome))
                }
                AsciiMode::Height => {
                    let h = *heightmap.get(x, y);
                    let color = height_color(h);
                    // Use darker version for foreground
                    let fg = (color.0.saturating_sub(40), color.1.saturating_sub(40), color.2.saturating_sub(40));
                    (height_char(h), fg, color)
                }
                AsciiMode::Temperature => {
                    let t = *temperature.get(x, y);
                    let color = temperature_color(t);
                    let fg = (color.0.saturating_sub(40), color.1.saturating_sub(40), color.2.saturating_sub(40));
                    (temperature_char(t), fg, color)
                }
                AsciiMode::Moisture => {
                    let m = *moisture.get(x, y);
                    let color = moisture_color(m);
                    let fg = (color.0.saturating_sub(40), color.1.saturating_sub(40), color.2.saturating_sub(40));
                    (moisture_char(m), fg, color)
                }
                AsciiMode::Stress => {
                    let s = *stress_map.get(x, y);
                    let color = stress_color(s);
                    let fg = (color.0.saturating_sub(40), color.1.saturating_sub(40), color.2.saturating_sub(40));
                    (stress_char(s), fg, color)
                }
                AsciiMode::Plates => {
                    let pid = *plate_map.get(x, y);
                    let ch = plate_char(pid, plates);
                    // Color based on plate type
                    let color = if pid.is_none() {
                        (30, 30, 30)
                    } else {
                        let plate = &plates[pid.0 as usize];
                        if plate.plate_type == PlateType::Continental {
                            // Continental: earthy browns/greens
                            let hue = (pid.0 * 37 % 60) as u8;
                            (100 + hue, 80 + hue / 2, 60)
                        } else {
                            // Oceanic: blues
                            let hue = (pid.0 * 43 % 60) as u8;
                            (40, 60 + hue / 2, 120 + hue)
                        }
                    };
                    let fg = (color.0.saturating_add(60), color.1.saturating_add(60), color.2.saturating_add(60));
                    (ch, fg, color)
                }
            };
            result.push_str(&ansi_colored_char(ch, fg, bg));
        }
        result.push_str("\x1b[0m\n"); // Reset at end of line
    }

    result
}

/// Print colorized ASCII map to stdout
pub fn print_colored_ascii_map(
    heightmap: &Tilemap<f32>,
    biomes: &Tilemap<ExtendedBiome>,
    temperature: &Tilemap<f32>,
    moisture: &Tilemap<f32>,
    stress_map: &Tilemap<f32>,
    plate_map: &Tilemap<PlateId>,
    plates: &[Plate],
    mode: AsciiMode,
) {
    let map_str = render_colored_ascii_map(
        heightmap, biomes, temperature, moisture, stress_map, plate_map, plates,
        mode,
    );
    print!("{}", map_str);
}

/// Generate colorized legend for biome characters
pub fn biome_legend_colored() -> String {
    let mut legend = String::new();
    legend.push_str("\x1b[1m=== BIOME LEGEND ===\x1b[0m\n");

    // Helper to format a biome entry with color
    let fmt = |biome: ExtendedBiome, label: &str| -> String {
        let ch = biome_char(&biome);
        let fg = biome_fg_color(&biome);
        let bg = biome_bg_color(&biome);
        format!("{} {}", ansi_colored_char(ch, fg, bg), ansi_fg_colored(label, bg))
    };

    legend.push_str("\x1b[1mWATER:\x1b[0m\n");
    legend.push_str(&format!("  {}  {}  {}\n",
        fmt(ExtendedBiome::DeepOcean, "DeepOcean"),
        fmt(ExtendedBiome::Ocean, "Ocean"),
        fmt(ExtendedBiome::CoastalWater, "Coastal")));

    legend.push_str("\x1b[1mCOLD:\x1b[0m\n");
    legend.push_str(&format!("  {}  {}  {}\n",
        fmt(ExtendedBiome::Ice, "Ice"),
        fmt(ExtendedBiome::Tundra, "Tundra"),
        fmt(ExtendedBiome::BorealForest, "Boreal")));

    legend.push_str("\x1b[1mTEMPERATE:\x1b[0m\n");
    legend.push_str(&format!("  {}  {}  {}\n",
        fmt(ExtendedBiome::TemperateGrassland, "Grassland"),
        fmt(ExtendedBiome::TemperateForest, "TempForest"),
        fmt(ExtendedBiome::TemperateRainforest, "TempRain")));

    legend.push_str("\x1b[1mWARM:\x1b[0m\n");
    legend.push_str(&format!("  {}  {}  {}  {}\n",
        fmt(ExtendedBiome::Desert, "Desert"),
        fmt(ExtendedBiome::Savanna, "Savanna"),
        fmt(ExtendedBiome::TropicalForest, "TropForest"),
        fmt(ExtendedBiome::TropicalRainforest, "TropRain")));

    legend.push_str("\x1b[1mMOUNTAIN:\x1b[0m\n");
    legend.push_str(&format!("  {}  {}\n",
        fmt(ExtendedBiome::AlpineTundra, "Alpine"),
        fmt(ExtendedBiome::SnowyPeaks, "SnowyPeak")));

    legend.push_str("\x1b[1mFANTASY FORESTS:\x1b[0m\n");
    legend.push_str(&format!("  {}  {}  {}  {}  {}\n",
        fmt(ExtendedBiome::DeadForest, "Dead"),
        fmt(ExtendedBiome::CrystalForest, "Crystal"),
        fmt(ExtendedBiome::BioluminescentForest, "Biolum"),
        fmt(ExtendedBiome::MushroomForest, "Mushroom"),
        fmt(ExtendedBiome::PetrifiedForest, "Petrified")));

    legend.push_str("\x1b[1mFANTASY WATERS:\x1b[0m\n");
    legend.push_str(&format!("  {}  {}  {}  {}\n",
        fmt(ExtendedBiome::AcidLake, "Acid"),
        fmt(ExtendedBiome::LavaLake, "Lava"),
        fmt(ExtendedBiome::FrozenLake, "Frozen"),
        fmt(ExtendedBiome::BioluminescentWater, "BiolumWater")));

    legend.push_str("\x1b[1mWASTELANDS:\x1b[0m\n");
    legend.push_str(&format!("  {}  {}  {}  {}\n",
        fmt(ExtendedBiome::VolcanicWasteland, "Volcanic"),
        fmt(ExtendedBiome::SaltFlats, "Salt"),
        fmt(ExtendedBiome::Ashlands, "Ash"),
        fmt(ExtendedBiome::CrystalWasteland, "CrystalWaste")));

    legend.push_str("\x1b[1mWETLANDS:\x1b[0m\n");
    legend.push_str(&format!("  {}  {}  {}  {}\n",
        fmt(ExtendedBiome::Swamp, "Swamp"),
        fmt(ExtendedBiome::Marsh, "Marsh"),
        fmt(ExtendedBiome::Bog, "Bog"),
        fmt(ExtendedBiome::MangroveSaltmarsh, "Mangrove")));

    legend.push_str("\x1b[1mULTRA-RARE:\x1b[0m\n");
    legend.push_str(&format!("  {}  {}  {}  {}  {}\n",
        fmt(ExtendedBiome::AncientGrove, "Ancient"),
        fmt(ExtendedBiome::TitanBones, "Titan"),
        fmt(ExtendedBiome::CoralPlateau, "Coral"),
        fmt(ExtendedBiome::ObsidianFields, "Obsidian"),
        fmt(ExtendedBiome::Geysers, "Geyser")));
    legend.push_str(&format!("  {}  {}  {}  {}  {}\n",
        fmt(ExtendedBiome::TarPits, "TarPit"),
        fmt(ExtendedBiome::FloatingStones, "Floating"),
        fmt(ExtendedBiome::Shadowfen, "Shadow"),
        fmt(ExtendedBiome::PrismaticPools, "Prismatic"),
        fmt(ExtendedBiome::AuroraWastes, "Aurora")));
    legend.push_str(&format!("  {}  {}  {}  {}  {}\n",
        fmt(ExtendedBiome::SingingDunes, "Dunes"),
        fmt(ExtendedBiome::Oasis, "Oasis"),
        fmt(ExtendedBiome::GlassDesert, "Glass"),
        fmt(ExtendedBiome::AbyssalVents, "Abyssal"),
        fmt(ExtendedBiome::Sargasso, "Sargasso")));

    legend.push_str("\x1b[1mMYSTICAL:\x1b[0m\n");
    legend.push_str(&format!("  {}  {}  {}  {}  {}\n",
        fmt(ExtendedBiome::EtherealMist, "Ethereal"),
        fmt(ExtendedBiome::StarfallCrater, "Starfall"),
        fmt(ExtendedBiome::LeyNexus, "LeyNexus"),
        fmt(ExtendedBiome::WhisperingStones, "Whisper"),
        fmt(ExtendedBiome::SpiritMarsh, "Spirit")));

    legend.push_str("\x1b[1mGEOLOGICAL:\x1b[0m\n");
    legend.push_str(&format!("  {}  {}  {}  {}  {}\n",
        fmt(ExtendedBiome::SulfurVents, "Sulfur"),
        fmt(ExtendedBiome::BasaltColumns, "Basalt"),
        fmt(ExtendedBiome::PaintedHills, "Painted"),
        fmt(ExtendedBiome::RazorPeaks, "Razor"),
        fmt(ExtendedBiome::SinkholeLakes, "Sinkhole")));

    legend.push_str("\x1b[1mBIOLOGICAL:\x1b[0m\n");
    legend.push_str(&format!("  {}  {}  {}  {}  {}\n",
        fmt(ExtendedBiome::ColossalHive, "Hive"),
        fmt(ExtendedBiome::BoneFields, "Bone"),
        fmt(ExtendedBiome::CarnivorousBog, "Carnivorous"),
        fmt(ExtendedBiome::FungalBloom, "Fungal"),
        fmt(ExtendedBiome::KelpTowers, "Kelp")));

    legend.push_str("\x1b[1mEXOTIC WATERS:\x1b[0m\n");
    legend.push_str(&format!("  {}  {}  {}  {}  {}\n",
        fmt(ExtendedBiome::BrinePools, "Brine"),
        fmt(ExtendedBiome::HotSprings, "HotSpring"),
        fmt(ExtendedBiome::MirrorLake, "Mirror"),
        fmt(ExtendedBiome::InkSea, "Ink"),
        fmt(ExtendedBiome::PhosphorShallows, "Phosphor")));

    legend.push_str("\x1b[1mALIEN/CORRUPTED:\x1b[0m\n");
    legend.push_str(&format!("  {}  {}  {}  {}  {}\n",
        fmt(ExtendedBiome::VoidScar, "Void"),
        fmt(ExtendedBiome::SiliconGrove, "Silicon"),
        fmt(ExtendedBiome::SporeWastes, "Spore"),
        fmt(ExtendedBiome::BleedingStone, "Bleeding"),
        fmt(ExtendedBiome::HollowEarth, "Hollow")));

    legend.push_str("\x1b[1mANCIENT RUINS:\x1b[0m\n");
    legend.push_str(&format!("  {}  {}  {}  {}\n",
        fmt(ExtendedBiome::SunkenCity, "Sunken"),
        fmt(ExtendedBiome::CyclopeanRuins, "Cyclopean"),
        fmt(ExtendedBiome::BuriedTemple, "Buried"),
        fmt(ExtendedBiome::OvergrownCitadel, "Overgrown")));

    legend
}

/// Export ASCII biome map as a PNG image with accompanying legend
/// Each tile is rendered as a colored cell with the biome's character
pub fn export_ascii_png(
    biomes: &Tilemap<ExtendedBiome>,
    path: &str,
) -> io::Result<()> {
    use image::{Rgb, RgbImage};

    let width = biomes.width;
    let height = biomes.height;

    // Cell size in pixels (each ASCII character becomes this many pixels)
    const CELL_SIZE: u32 = 8;

    let img_width = width as u32 * CELL_SIZE;
    let img_height = height as u32 * CELL_SIZE;

    let mut img = RgbImage::new(img_width, img_height);

    // Simple 5x7 bitmap font for ASCII characters (stored as u8 bitmasks)
    // Each character is 5 pixels wide, 7 pixels tall
    let font = create_bitmap_font();

    // Count biomes for the legend
    let mut biome_counts: HashMap<ExtendedBiome, usize> = HashMap::new();

    for y in 0..height {
        for x in 0..width {
            let biome = *biomes.get(x, y);
            *biome_counts.entry(biome).or_insert(0) += 1;

            let (r, g, b) = biome.color();
            let ch = biome_char(&biome);

            // Calculate brightness for text color contrast
            let brightness = (r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000;
            let text_color = if brightness > 128 { Rgb([0, 0, 0]) } else { Rgb([255, 255, 255]) };
            let bg_color = Rgb([r, g, b]);

            // Fill cell with background color
            let cell_x = x as u32 * CELL_SIZE;
            let cell_y = y as u32 * CELL_SIZE;

            for py in 0..CELL_SIZE {
                for px in 0..CELL_SIZE {
                    img.put_pixel(cell_x + px, cell_y + py, bg_color);
                }
            }

            // Draw character (centered in cell)
            if let Some(glyph) = font.get(&ch) {
                let offset_x = (CELL_SIZE - 5) / 2;
                let offset_y = (CELL_SIZE - 7) / 2;

                for (row_idx, &row) in glyph.iter().enumerate() {
                    for col in 0..5u32 {
                        if (row >> (4 - col)) & 1 == 1 {
                            let px = cell_x + offset_x + col;
                            let py = cell_y + offset_y + row_idx as u32;
                            if px < img_width && py < img_height {
                                img.put_pixel(px, py, text_color);
                            }
                        }
                    }
                }
            }
        }
    }

    img.save(path).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    // Export legend text file alongside the PNG
    let legend_path = path.replace(".png", "_legend.txt").replace(".PNG", "_legend.txt");
    export_png_legend(&legend_path, &biome_counts, width, height)?;

    Ok(())
}

/// Export a legend text file for the ASCII PNG
fn export_png_legend(
    path: &str,
    biome_counts: &HashMap<ExtendedBiome, usize>,
    width: usize,
    height: usize,
) -> io::Result<()> {
    let mut file = File::create(path)?;
    let total = (width * height) as f32;

    writeln!(file, "=== ASCII BIOME MAP LEGEND ===")?;
    writeln!(file, "Image size: {}x{} tiles ({}x{} pixels)",
             width, height, width * 8, height * 8)?;
    writeln!(file)?;

    // Sort biomes by count (descending)
    let mut sorted: Vec<_> = biome_counts.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1));

    writeln!(file, "=== BIOME DISTRIBUTION ===")?;
    writeln!(file)?;
    writeln!(file, "{:<4} {:<24} {:>8} {:>8}  {}",
             "Char", "Biome", "Count", "Percent", "RGB Color")?;
    writeln!(file, "{}", "-".repeat(70))?;

    for (biome, count) in &sorted {
        let ch = biome_char(biome);
        let (r, g, b) = biome.color();
        let pct = (**count as f32 / total) * 100.0;
        writeln!(file, "{:<4} {:<24} {:>8} {:>7.1}%  ({:>3}, {:>3}, {:>3})",
                 ch, biome.display_name(), count, pct, r, g, b)?;
    }

    writeln!(file)?;
    writeln!(file, "=== CHARACTER LEGEND ===")?;
    writeln!(file)?;

    // Group by category
    writeln!(file, "BASE BIOMES:")?;
    writeln!(file, "  ~  Deep Ocean          .  Ocean              ,  Coastal Water")?;
    writeln!(file, "  #  Ice                 :  Tundra             B  Boreal Forest")?;
    writeln!(file, "  \"  Temperate Grass     T  Temperate Forest   R  Temp Rainforest")?;
    writeln!(file, "  d  Desert              ;  Savanna            t  Tropical Forest")?;
    writeln!(file, "  r  Trop Rainforest     ^  Alpine Tundra      A  Snowy Peaks")?;
    writeln!(file)?;

    writeln!(file, "FANTASY FORESTS:")?;
    writeln!(file, "  X  Dead Forest         C  Crystal Forest     *  Bioluminescent")?;
    writeln!(file, "  M  Mushroom Forest     P  Petrified Forest")?;
    writeln!(file)?;

    writeln!(file, "FANTASY WATERS:")?;
    writeln!(file, "  a  Acid Lake           @  Lava Lake          o  Frozen Lake")?;
    writeln!(file, "  b  Bioluminescent Water")?;
    writeln!(file)?;

    writeln!(file, "WASTELANDS:")?;
    writeln!(file, "  V  Volcanic Wasteland  _  Salt Flats         %  Ashlands")?;
    writeln!(file, "  c  Crystal Wasteland")?;
    writeln!(file)?;

    writeln!(file, "WETLANDS:")?;
    writeln!(file, "  S  Swamp               m  Marsh              &  Bog")?;
    writeln!(file, "  G  Mangrove Saltmarsh")?;
    writeln!(file)?;

    writeln!(file, "OCEAN BIOMES (Realistic):")?;
    writeln!(file, "  ⌇  Coral Reef          |  Kelp Forest        ≈  Seagrass Meadow")?;
    writeln!(file, "  ─  Continental Shelf   ▲  Seamount           ▼  Oceanic Trench")?;
    writeln!(file, "  ░  Abyssal Plain       ═  Mid-Ocean Ridge    ●  Cold Seep")?;
    writeln!(file, "  ○  Brine Pool")?;
    writeln!(file)?;

    writeln!(file, "OCEAN BIOMES (Fantasy):")?;
    writeln!(file, "  ◆  Crystal Depths      †  Leviathan Grave    ▓  Drowned Citadel")?;
    writeln!(file, "  ◎  Void Maw            ◇  Pearl Gardens      ♪  Siren Shallows")?;
    writeln!(file, "  ❄  Frozen Abyss        ♨  Thermal Vents")?;
    writeln!(file)?;

    writeln!(file, "ULTRA-RARE & SPECIAL:")?;
    writeln!(file, "  Y  Ancient Grove       W  Titan Bones        K  Coral Plateau")?;
    writeln!(file, "  O  Obsidian Fields     g  Geysers            p  Tar Pits")?;
    writeln!(file, "  F  Floating Stones     Z  Shadowfen          Q  Prismatic Pools")?;
    writeln!(file, "  N  Aurora Wastes       D  Singing Dunes      I  Oasis")?;
    writeln!(file, "  L  Glass Desert        v  Abyssal Vents      w  Sargasso")?;
    writeln!(file)?;

    writeln!(file, "MYSTICAL:")?;
    writeln!(file, "  E  Ethereal Mist       U  Starfall Crater    J  Ley Nexus")?;
    writeln!(file, "  H  Whispering Stones   z  Spirit Marsh")?;
    writeln!(file)?;

    writeln!(file, "GEOLOGICAL:")?;
    writeln!(file, "  u  Sulfur Vents        l  Basalt Columns     i  Painted Hills")?;
    writeln!(file, "  j  Razor Peaks         n  Sinkhole Lakes")?;
    writeln!(file)?;

    writeln!(file, "BIOLOGICAL:")?;
    writeln!(file, "  h  Colossal Hive       e  Bone Fields        y  Carnivorous Bog")?;
    writeln!(file, "  f  Fungal Bloom        k  Kelp Towers")?;
    writeln!(file)?;

    writeln!(file, "EXOTIC WATERS:")?;
    writeln!(file, "  q  Brine Pools         s  Hot Springs        0  Mirror Lake")?;
    writeln!(file, "  -  Ink Sea             +  Phosphor Shallows")?;
    writeln!(file)?;

    writeln!(file, "ALIEN/CORRUPTED:")?;
    writeln!(file, "  !  Void Scar           $  Silicon Grove      (  Spore Wastes")?;
    writeln!(file, "  )  Bleeding Stone      ?  Hollow Earth")?;
    writeln!(file)?;

    writeln!(file, "ANCIENT RUINS:")?;
    writeln!(file, "  [  Sunken City         ]  Cyclopean Ruins    /  Buried Temple")?;
    writeln!(file, "  \\  Overgrown Citadel   Ω  Dark Tower")?;

    Ok(())
}

/// Create a simple 5x7 bitmap font for common ASCII characters
fn create_bitmap_font() -> HashMap<char, [u8; 7]> {
    let mut font = HashMap::new();

    // Each entry is 7 rows of 5-bit patterns (MSB = leftmost pixel)
    // Format: 0bXXXXX where X is pixel on/off

    // Basic punctuation and symbols
    font.insert('~', [0b00000, 0b01000, 0b10101, 0b00010, 0b00000, 0b00000, 0b00000]);
    font.insert('.', [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100]);
    font.insert(',', [0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100, 0b11000]);
    font.insert(':', [0b00000, 0b01100, 0b01100, 0b00000, 0b01100, 0b01100, 0b00000]);
    font.insert(';', [0b00000, 0b01100, 0b01100, 0b00000, 0b01100, 0b01100, 0b11000]);
    font.insert('"', [0b01010, 0b01010, 0b01010, 0b00000, 0b00000, 0b00000, 0b00000]);
    font.insert('#', [0b01010, 0b11111, 0b01010, 0b01010, 0b11111, 0b01010, 0b00000]);
    font.insert('_', [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b11111]);
    font.insert('-', [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000]);
    font.insert('+', [0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000]);
    font.insert('=', [0b00000, 0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000]);
    font.insert('*', [0b00000, 0b10101, 0b01110, 0b11111, 0b01110, 0b10101, 0b00000]);
    font.insert('^', [0b00100, 0b01010, 0b10001, 0b00000, 0b00000, 0b00000, 0b00000]);
    font.insert('&', [0b01100, 0b10010, 0b01100, 0b10110, 0b10001, 0b10001, 0b01110]);
    font.insert('%', [0b11001, 0b11010, 0b00100, 0b01000, 0b01011, 0b10011, 0b00000]);
    font.insert('@', [0b01110, 0b10001, 0b10111, 0b10101, 0b10111, 0b10000, 0b01110]);
    font.insert('!', [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100]);
    font.insert('?', [0b01110, 0b10001, 0b00001, 0b00110, 0b00100, 0b00000, 0b00100]);
    font.insert('/', [0b00001, 0b00010, 0b00100, 0b00100, 0b01000, 0b10000, 0b00000]);
    font.insert('\\', [0b10000, 0b01000, 0b00100, 0b00100, 0b00010, 0b00001, 0b00000]);
    font.insert('[', [0b01110, 0b01000, 0b01000, 0b01000, 0b01000, 0b01000, 0b01110]);
    font.insert(']', [0b01110, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b01110]);
    font.insert('(', [0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010]);
    font.insert(')', [0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000]);
    font.insert('|', [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100]);
    font.insert('$', [0b00100, 0b01111, 0b10100, 0b01110, 0b00101, 0b11110, 0b00100]);
    font.insert('0', [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110]);

    // Letters (uppercase)
    font.insert('A', [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001]);
    font.insert('B', [0b11110, 0b10001, 0b11110, 0b10001, 0b10001, 0b10001, 0b11110]);
    font.insert('C', [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110]);
    font.insert('D', [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110]);
    font.insert('E', [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111]);
    font.insert('F', [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000]);
    font.insert('G', [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110]);
    font.insert('H', [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001]);
    font.insert('I', [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110]);
    font.insert('J', [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100]);
    font.insert('K', [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001]);
    font.insert('L', [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111]);
    font.insert('M', [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001]);
    font.insert('N', [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001]);
    font.insert('O', [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110]);
    font.insert('P', [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000]);
    font.insert('Q', [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101]);
    font.insert('R', [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001]);
    font.insert('S', [0b01110, 0b10001, 0b10000, 0b01110, 0b00001, 0b10001, 0b01110]);
    font.insert('T', [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100]);
    font.insert('U', [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110]);
    font.insert('V', [0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b01010, 0b00100]);
    font.insert('W', [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001]);
    font.insert('X', [0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b01010, 0b10001]);
    font.insert('Y', [0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100]);
    font.insert('Z', [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111]);

    // Letters (lowercase)
    font.insert('a', [0b00000, 0b00000, 0b01110, 0b00001, 0b01111, 0b10001, 0b01111]);
    font.insert('b', [0b10000, 0b10000, 0b10110, 0b11001, 0b10001, 0b10001, 0b11110]);
    font.insert('c', [0b00000, 0b00000, 0b01110, 0b10000, 0b10000, 0b10001, 0b01110]);
    font.insert('d', [0b00001, 0b00001, 0b01101, 0b10011, 0b10001, 0b10001, 0b01111]);
    font.insert('e', [0b00000, 0b00000, 0b01110, 0b10001, 0b11111, 0b10000, 0b01110]);
    font.insert('f', [0b00110, 0b01001, 0b01000, 0b11100, 0b01000, 0b01000, 0b01000]);
    font.insert('g', [0b00000, 0b01111, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110]);
    font.insert('h', [0b10000, 0b10000, 0b10110, 0b11001, 0b10001, 0b10001, 0b10001]);
    font.insert('i', [0b00100, 0b00000, 0b01100, 0b00100, 0b00100, 0b00100, 0b01110]);
    font.insert('j', [0b00010, 0b00000, 0b00110, 0b00010, 0b00010, 0b10010, 0b01100]);
    font.insert('k', [0b10000, 0b10000, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010]);
    font.insert('l', [0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110]);
    font.insert('m', [0b00000, 0b00000, 0b11010, 0b10101, 0b10101, 0b10001, 0b10001]);
    font.insert('n', [0b00000, 0b00000, 0b10110, 0b11001, 0b10001, 0b10001, 0b10001]);
    font.insert('o', [0b00000, 0b00000, 0b01110, 0b10001, 0b10001, 0b10001, 0b01110]);
    font.insert('p', [0b00000, 0b11110, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000]);
    font.insert('q', [0b00000, 0b01111, 0b10001, 0b01111, 0b00001, 0b00001, 0b00001]);
    font.insert('r', [0b00000, 0b00000, 0b10110, 0b11001, 0b10000, 0b10000, 0b10000]);
    font.insert('s', [0b00000, 0b00000, 0b01110, 0b10000, 0b01110, 0b00001, 0b11110]);
    font.insert('t', [0b01000, 0b01000, 0b11100, 0b01000, 0b01000, 0b01001, 0b00110]);
    font.insert('u', [0b00000, 0b00000, 0b10001, 0b10001, 0b10001, 0b10011, 0b01101]);
    font.insert('v', [0b00000, 0b00000, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100]);
    font.insert('w', [0b00000, 0b00000, 0b10001, 0b10001, 0b10101, 0b10101, 0b01010]);
    font.insert('x', [0b00000, 0b00000, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001]);
    font.insert('y', [0b00000, 0b10001, 0b10001, 0b01111, 0b00001, 0b10001, 0b01110]);
    font.insert('z', [0b00000, 0b00000, 0b11111, 0b00010, 0b00100, 0b01000, 0b11111]);

    // Unicode characters used for ocean biomes (render as simple shapes)
    font.insert('⌇', [0b10101, 0b01010, 0b10101, 0b01010, 0b10101, 0b01010, 0b10101]); // coral
    font.insert('≈', [0b00000, 0b01010, 0b10101, 0b00000, 0b01010, 0b10101, 0b00000]); // waves
    font.insert('─', [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000]); // horizontal
    font.insert('▲', [0b00100, 0b00100, 0b01010, 0b01010, 0b10001, 0b10001, 0b11111]); // triangle up
    font.insert('▼', [0b11111, 0b10001, 0b10001, 0b01010, 0b01010, 0b00100, 0b00100]); // triangle down
    font.insert('░', [0b10101, 0b01010, 0b10101, 0b01010, 0b10101, 0b01010, 0b10101]); // shade
    font.insert('═', [0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000]); // double line
    font.insert('●', [0b00000, 0b01110, 0b11111, 0b11111, 0b11111, 0b01110, 0b00000]); // filled circle
    font.insert('○', [0b00000, 0b01110, 0b10001, 0b10001, 0b10001, 0b01110, 0b00000]); // hollow circle
    font.insert('◆', [0b00100, 0b01110, 0b11111, 0b11111, 0b11111, 0b01110, 0b00100]); // diamond
    font.insert('†', [0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00100, 0b00100]); // cross/dagger
    font.insert('▓', [0b11011, 0b01110, 0b11011, 0b01110, 0b11011, 0b01110, 0b11011]); // dense shade
    font.insert('◎', [0b01110, 0b10001, 0b10101, 0b10101, 0b10101, 0b10001, 0b01110]); // target
    font.insert('◇', [0b00100, 0b01010, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100]); // hollow diamond
    font.insert('♪', [0b00010, 0b00011, 0b00010, 0b00010, 0b01110, 0b11110, 0b01100]); // music note
    font.insert('❄', [0b10101, 0b01110, 0b11111, 0b01110, 0b11111, 0b01110, 0b10101]); // snowflake
    font.insert('♨', [0b01010, 0b10101, 0b01010, 0b00000, 0b11111, 0b10001, 0b01110]); // hot springs
    font.insert('Ω', [0b01110, 0b10001, 0b10001, 0b10001, 0b01010, 0b01010, 0b11011]); // omega (Dark Tower)

    font
}
