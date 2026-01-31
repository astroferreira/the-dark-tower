//! Comprehensive world data export
//!
//! Exports complete WorldData in formats suitable for loading in external applications.
//! Supports JSON metadata + binary data arrays for efficiency.

use crate::biomes::ExtendedBiome;
use crate::plates::PlateId;
use crate::tilemap::Tilemap;
use crate::water_bodies::{WaterBodyId, WaterBodyType};
use crate::world::WorldData;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

/// Configuration for world export
#[derive(Clone, Debug)]
pub struct WorldExportConfig {
    /// Export heightmap data
    pub heightmap: bool,
    /// Export temperature data
    pub temperature: bool,
    /// Export moisture data
    pub moisture: bool,
    /// Export biome data
    pub biomes: bool,
    /// Export tectonic stress data
    pub stress: bool,
    /// Export plate assignment data
    pub plates: bool,
    /// Export water body data
    pub water_bodies: bool,
    /// Export water depth data
    pub water_depth: bool,
    /// Export flow accumulation (rivers) if available
    pub flow_accumulation: bool,
    /// Use binary format for large arrays (more compact)
    pub binary_arrays: bool,
    /// Include per-tile detailed data (large!)
    pub per_tile_details: bool,
}

impl Default for WorldExportConfig {
    fn default() -> Self {
        Self {
            heightmap: true,
            temperature: true,
            moisture: true,
            biomes: true,
            stress: true,
            plates: true,
            water_bodies: true,
            water_depth: true,
            flow_accumulation: true,
            binary_arrays: true,
            per_tile_details: false,
        }
    }
}

/// Export world data to a directory with JSON metadata and data files
pub fn export_world_data(
    world: &WorldData,
    output_dir: &Path,
    config: &WorldExportConfig,
) -> std::io::Result<Vec<String>> {
    std::fs::create_dir_all(output_dir)?;

    let mut exported_files = Vec::new();

    // Export metadata JSON
    let meta_path = output_dir.join("world_meta.json");
    export_metadata(world, &meta_path)?;
    exported_files.push(meta_path.display().to_string());

    // Export data arrays
    if config.heightmap {
        let path = output_dir.join(if config.binary_arrays { "heightmap.bin" } else { "heightmap.csv" });
        if config.binary_arrays {
            export_f32_binary(&world.heightmap, &path)?;
        } else {
            export_f32_csv(&world.heightmap, &path)?;
        }
        exported_files.push(path.display().to_string());
    }

    if config.temperature {
        let path = output_dir.join(if config.binary_arrays { "temperature.bin" } else { "temperature.csv" });
        if config.binary_arrays {
            export_f32_binary(&world.temperature, &path)?;
        } else {
            export_f32_csv(&world.temperature, &path)?;
        }
        exported_files.push(path.display().to_string());
    }

    if config.moisture {
        let path = output_dir.join(if config.binary_arrays { "moisture.bin" } else { "moisture.csv" });
        if config.binary_arrays {
            export_f32_binary(&world.moisture, &path)?;
        } else {
            export_f32_csv(&world.moisture, &path)?;
        }
        exported_files.push(path.display().to_string());
    }

    if config.biomes {
        let path = output_dir.join(if config.binary_arrays { "biomes.bin" } else { "biomes.csv" });
        if config.binary_arrays {
            export_biomes_binary(&world.biomes, &path)?;
        } else {
            export_biomes_csv(&world.biomes, &path)?;
        }
        // Also export biome legend
        let legend_path = output_dir.join("biomes_legend.json");
        export_biome_legend_from_world(&world.biomes, &legend_path)?;
        exported_files.push(path.display().to_string());
        exported_files.push(legend_path.display().to_string());
    }

    if config.stress {
        let path = output_dir.join(if config.binary_arrays { "stress.bin" } else { "stress.csv" });
        if config.binary_arrays {
            export_f32_binary(&world.stress_map, &path)?;
        } else {
            export_f32_csv(&world.stress_map, &path)?;
        }
        exported_files.push(path.display().to_string());
    }

    if config.plates {
        let path = output_dir.join(if config.binary_arrays { "plates.bin" } else { "plates.csv" });
        if config.binary_arrays {
            export_plates_binary(&world.plate_map, &path)?;
        } else {
            export_plates_csv(&world.plate_map, &path)?;
        }
        // Export plate metadata
        let plates_meta_path = output_dir.join("plates_meta.json");
        export_plates_metadata(&world.plates, &plates_meta_path)?;
        exported_files.push(path.display().to_string());
        exported_files.push(plates_meta_path.display().to_string());
    }

    if config.water_bodies {
        let path = output_dir.join(if config.binary_arrays { "water_bodies.bin" } else { "water_bodies.csv" });
        if config.binary_arrays {
            export_water_bodies_binary(&world.water_body_map, &path)?;
        } else {
            export_water_bodies_csv(&world.water_body_map, &path)?;
        }
        // Export water body metadata
        let wb_meta_path = output_dir.join("water_bodies_meta.json");
        export_water_bodies_metadata(&world.water_bodies, &wb_meta_path)?;
        exported_files.push(path.display().to_string());
        exported_files.push(wb_meta_path.display().to_string());
    }

    if config.water_depth {
        let path = output_dir.join(if config.binary_arrays { "water_depth.bin" } else { "water_depth.csv" });
        if config.binary_arrays {
            export_f32_binary(&world.water_depth, &path)?;
        } else {
            export_f32_csv(&world.water_depth, &path)?;
        }
        exported_files.push(path.display().to_string());
    }

    if config.flow_accumulation {
        if let Some(ref flow_acc) = world.flow_accumulation {
            let path = output_dir.join(if config.binary_arrays { "flow_accumulation.bin" } else { "flow_accumulation.csv" });
            if config.binary_arrays {
                export_f32_binary(flow_acc, &path)?;
            } else {
                export_f32_csv(flow_acc, &path)?;
            }
            exported_files.push(path.display().to_string());
        }
    }

    // Export per-tile details if requested (comprehensive but large)
    if config.per_tile_details {
        let path = output_dir.join("tiles.jsonl");
        export_per_tile_jsonl(world, &path)?;
        exported_files.push(path.display().to_string());
    }

    Ok(exported_files)
}

/// Export world metadata as JSON
fn export_metadata(world: &WorldData, path: &Path) -> std::io::Result<()> {
    let mut file = BufWriter::new(File::create(path)?);

    writeln!(file, "{{")?;
    writeln!(file, "  \"format_version\": \"1.0\",")?;
    writeln!(file, "  \"width\": {},", world.width)?;
    writeln!(file, "  \"height\": {},", world.height)?;
    writeln!(file, "  \"seed\": {},", world.seed())?;
    writeln!(file, "  \"km_per_tile\": {},", world.scale.km_per_tile)?;

    // Height stats
    let (min_h, max_h) = height_stats(&world.heightmap);
    writeln!(file, "  \"min_elevation\": {:.1},", min_h)?;
    writeln!(file, "  \"max_elevation\": {:.1},", max_h)?;

    // Temperature stats
    let (min_t, max_t) = height_stats(&world.temperature);
    writeln!(file, "  \"min_temperature\": {:.1},", min_t)?;
    writeln!(file, "  \"max_temperature\": {:.1},", max_t)?;

    // Plate count
    writeln!(file, "  \"plate_count\": {},", world.plates.len())?;

    // Water body count
    writeln!(file, "  \"water_body_count\": {},", world.water_bodies.len())?;

    // Land/ocean stats
    let land_tiles = world.heightmap.iter().filter(|(_, _, &h)| h >= 0.0).count();
    let total_tiles = world.width * world.height;
    writeln!(file, "  \"land_tiles\": {},", land_tiles)?;
    writeln!(file, "  \"ocean_tiles\": {},", total_tiles - land_tiles)?;
    writeln!(file, "  \"land_percentage\": {:.2},", 100.0 * land_tiles as f64 / total_tiles as f64)?;

    // Data format info
    writeln!(file, "  \"data_format\": {{")?;
    writeln!(file, "    \"heightmap\": \"f32 little-endian, row-major (y * width + x)\",")?;
    writeln!(file, "    \"temperature\": \"f32 little-endian, row-major\",")?;
    writeln!(file, "    \"moisture\": \"f32 little-endian, row-major (0.0-1.0)\",")?;
    writeln!(file, "    \"biomes\": \"u8 row-major (see biomes_legend.json)\",")?;
    writeln!(file, "    \"stress\": \"f32 little-endian (-1.0 divergent to +1.0 convergent)\",")?;
    writeln!(file, "    \"plates\": \"u8 row-major (plate ID)\",")?;
    writeln!(file, "    \"water_bodies\": \"u16 little-endian row-major (0=land, 1=ocean, 2+=lakes)\",")?;
    writeln!(file, "    \"water_depth\": \"f32 little-endian row-major (meters, 0=dry)\",")?;
    writeln!(file, "    \"flow_accumulation\": \"f32 little-endian row-major (drainage area)\"")?;
    writeln!(file, "  }}")?;

    writeln!(file, "}}")?;
    Ok(())
}

/// Export f32 tilemap as binary (little-endian)
fn export_f32_binary(tilemap: &Tilemap<f32>, path: &Path) -> std::io::Result<()> {
    let mut file = BufWriter::new(File::create(path)?);
    for y in 0..tilemap.height {
        for x in 0..tilemap.width {
            let value = *tilemap.get(x, y);
            file.write_all(&value.to_le_bytes())?;
        }
    }
    Ok(())
}

/// Export f32 tilemap as CSV
fn export_f32_csv(tilemap: &Tilemap<f32>, path: &Path) -> std::io::Result<()> {
    let mut file = BufWriter::new(File::create(path)?);
    for y in 0..tilemap.height {
        let row: Vec<String> = (0..tilemap.width)
            .map(|x| format!("{:.2}", tilemap.get(x, y)))
            .collect();
        writeln!(file, "{}", row.join(","))?;
    }
    Ok(())
}

/// Export biome tilemap as binary (u8)
fn export_biomes_binary(tilemap: &Tilemap<ExtendedBiome>, path: &Path) -> std::io::Result<()> {
    let mut file = BufWriter::new(File::create(path)?);
    for y in 0..tilemap.height {
        for x in 0..tilemap.width {
            let biome = *tilemap.get(x, y);
            file.write_all(&[biome as u8])?;
        }
    }
    Ok(())
}

/// Export biome tilemap as CSV
fn export_biomes_csv(tilemap: &Tilemap<ExtendedBiome>, path: &Path) -> std::io::Result<()> {
    let mut file = BufWriter::new(File::create(path)?);
    for y in 0..tilemap.height {
        let row: Vec<String> = (0..tilemap.width)
            .map(|x| format!("{}", *tilemap.get(x, y) as u8))
            .collect();
        writeln!(file, "{}", row.join(","))?;
    }
    Ok(())
}

/// Export biome legend as JSON (only biomes used in the world)
fn export_biome_legend_from_world(biomes: &Tilemap<ExtendedBiome>, path: &Path) -> std::io::Result<()> {
    use std::collections::BTreeMap;

    // Collect unique biomes from the world
    let mut biome_set: BTreeMap<u8, ExtendedBiome> = BTreeMap::new();
    for (_, _, &biome) in biomes.iter() {
        biome_set.entry(biome as u8).or_insert(biome);
    }

    let mut file = BufWriter::new(File::create(path)?);
    writeln!(file, "{{")?;
    writeln!(file, "  \"description\": \"Biome IDs used in this world map\",")?;
    writeln!(file, "  \"biomes\": [")?;

    let biome_list: Vec<_> = biome_set.into_iter().collect();
    for (i, (id, biome)) in biome_list.iter().enumerate() {
        let color = biome.color();
        let comma = if i < biome_list.len() - 1 { "," } else { "" };
        writeln!(
            file,
            "    {{ \"id\": {}, \"name\": \"{:?}\", \"color\": [{}, {}, {}] }}{}",
            id,
            biome,
            color.0,
            color.1,
            color.2,
            comma
        )?;
    }

    writeln!(file, "  ]")?;
    writeln!(file, "}}")?;
    Ok(())
}

/// Export plate tilemap as binary (u8)
fn export_plates_binary(tilemap: &Tilemap<PlateId>, path: &Path) -> std::io::Result<()> {
    let mut file = BufWriter::new(File::create(path)?);
    for y in 0..tilemap.height {
        for x in 0..tilemap.width {
            let plate_id = tilemap.get(x, y).0;
            file.write_all(&[plate_id])?;
        }
    }
    Ok(())
}

/// Export plate tilemap as CSV
fn export_plates_csv(tilemap: &Tilemap<PlateId>, path: &Path) -> std::io::Result<()> {
    let mut file = BufWriter::new(File::create(path)?);
    for y in 0..tilemap.height {
        let row: Vec<String> = (0..tilemap.width)
            .map(|x| format!("{}", tilemap.get(x, y).0))
            .collect();
        writeln!(file, "{}", row.join(","))?;
    }
    Ok(())
}

/// Export plate metadata as JSON
fn export_plates_metadata(plates: &[crate::plates::Plate], path: &Path) -> std::io::Result<()> {
    let mut file = BufWriter::new(File::create(path)?);
    writeln!(file, "{{")?;
    writeln!(file, "  \"plates\": [")?;

    for (i, plate) in plates.iter().enumerate() {
        let comma = if i < plates.len() - 1 { "," } else { "" };
        writeln!(file, "    {{")?;
        writeln!(file, "      \"id\": {},", plate.id.0)?;
        writeln!(file, "      \"type\": \"{:?}\",", plate.plate_type)?;
        writeln!(file, "      \"base_elevation\": {:.1},", plate.base_elevation)?;
        writeln!(file, "      \"velocity_x\": {:.4},", plate.velocity.x)?;
        writeln!(file, "      \"velocity_y\": {:.4},", plate.velocity.y)?;
        writeln!(file, "      \"color\": [{}, {}, {}]", plate.color[0], plate.color[1], plate.color[2])?;
        writeln!(file, "    }}{}", comma)?;
    }

    writeln!(file, "  ]")?;
    writeln!(file, "}}")?;
    Ok(())
}

/// Export water body tilemap as binary (u16 little-endian)
fn export_water_bodies_binary(tilemap: &Tilemap<WaterBodyId>, path: &Path) -> std::io::Result<()> {
    let mut file = BufWriter::new(File::create(path)?);
    for y in 0..tilemap.height {
        for x in 0..tilemap.width {
            let wb_id = tilemap.get(x, y).0;
            file.write_all(&wb_id.to_le_bytes())?;
        }
    }
    Ok(())
}

/// Export water body tilemap as CSV
fn export_water_bodies_csv(tilemap: &Tilemap<WaterBodyId>, path: &Path) -> std::io::Result<()> {
    let mut file = BufWriter::new(File::create(path)?);
    for y in 0..tilemap.height {
        let row: Vec<String> = (0..tilemap.width)
            .map(|x| format!("{}", tilemap.get(x, y).0))
            .collect();
        writeln!(file, "{}", row.join(","))?;
    }
    Ok(())
}

/// Export water body metadata as JSON
fn export_water_bodies_metadata(
    water_bodies: &[crate::water_bodies::WaterBody],
    path: &Path,
) -> std::io::Result<()> {
    let mut file = BufWriter::new(File::create(path)?);
    writeln!(file, "{{")?;
    writeln!(file, "  \"water_bodies\": [")?;

    for (i, wb) in water_bodies.iter().enumerate() {
        let comma = if i < water_bodies.len() - 1 { "," } else { "" };
        let type_name = match wb.body_type {
            WaterBodyType::None => "Land",
            WaterBodyType::Ocean => "Ocean",
            WaterBodyType::Lake => "Lake",
            WaterBodyType::River => "River",
        };
        writeln!(file, "    {{")?;
        writeln!(file, "      \"id\": {},", wb.id.0)?;
        writeln!(file, "      \"type\": \"{}\",", type_name)?;
        writeln!(file, "      \"tile_count\": {},", wb.tile_count)?;
        writeln!(file, "      \"min_elevation\": {:.1},", wb.min_elevation)?;
        writeln!(file, "      \"max_elevation\": {:.1},", wb.max_elevation)?;
        writeln!(file, "      \"avg_elevation\": {:.1},", wb.avg_elevation)?;
        writeln!(file, "      \"bounds\": [{}, {}, {}, {}]", wb.bounds.0, wb.bounds.1, wb.bounds.2, wb.bounds.3)?;
        writeln!(file, "    }}{}", comma)?;
    }

    writeln!(file, "  ]")?;
    writeln!(file, "}}")?;
    Ok(())
}

/// Export per-tile detailed data as JSON Lines (one JSON object per line)
fn export_per_tile_jsonl(world: &WorldData, path: &Path) -> std::io::Result<()> {
    let mut file = BufWriter::new(File::create(path)?);

    for y in 0..world.height {
        for x in 0..world.width {
            let h = *world.heightmap.get(x, y);
            let temp = *world.temperature.get(x, y);
            let moist = *world.moisture.get(x, y);
            let biome = *world.biomes.get(x, y);
            let stress = *world.stress_map.get(x, y);
            let plate = world.plate_map.get(x, y).0;
            let wb_id = world.water_body_map.get(x, y).0;
            let water_depth = *world.water_depth.get(x, y);

            let flow = world.flow_accumulation
                .as_ref()
                .map(|fa| *fa.get(x, y))
                .unwrap_or(0.0);

            writeln!(
                file,
                "{{\"x\":{},\"y\":{},\"h\":{:.1},\"t\":{:.1},\"m\":{:.2},\"b\":{},\"s\":{:.2},\"p\":{},\"w\":{},\"wd\":{:.1},\"f\":{:.0}}}",
                x, y, h, temp, moist, biome as u8, stress, plate, wb_id, water_depth, flow
            )?;
        }
    }

    Ok(())
}

/// Get min/max from a f32 tilemap
fn height_stats(tilemap: &Tilemap<f32>) -> (f32, f32) {
    let mut min = f32::MAX;
    let mut max = f32::MIN;
    for (_, _, &v) in tilemap.iter() {
        min = min.min(v);
        max = max.max(v);
    }
    (min, max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_config_default() {
        let config = WorldExportConfig::default();
        assert!(config.heightmap);
        assert!(config.biomes);
        assert!(config.binary_arrays);
    }
}
