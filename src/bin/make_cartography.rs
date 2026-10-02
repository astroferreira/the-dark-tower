//! Dedicated tool to render and export high-resolution antique old paper cartography maps
//! with procedural super-resolution upscaling (Catmull-Rom spline interpolation & micro-fractals)

use std::error::Error;
use image::{ImageBuffer, Rgb};
use planet_generator::world::generate_world;
use planet_generator::cartography::{render_cartography_map, CartographyParams, PaperStyle};

fn main() -> Result<(), Box<dyn Error>> {
    println!("============================================================");
    println!("  Antique Cartography Super-Resolution Procedural Renderer  ");
    println!("============================================================");

    // Simulation grid resolution
    let sim_w = 1024usize;
    let sim_h = 512usize;
    let upscale = 4usize; // 4x procedural upscaling -> 4096x2048 per map

    println!("Simulation Grid: {}x{}", sim_w, sim_h);
    println!("Upscale Factor:  {}x (Target Resolution: {}x{})", upscale, sim_w * upscale, sim_h * upscale);

    // 1. Generate showcase cartography maps for different seeds and styles
    let showcase_configs: [(u64, PaperStyle, &str, &str); 4] = [
        (333, PaperStyle::AgedParchment, "cartography_parchment_333.png", "1. Aged Parchment (Seed 333)"),
        (101, PaperStyle::Atlas17thCentury, "cartography_atlas_101.png", "2. 17th Century Atlas (Seed 101)"),
        (555, PaperStyle::CopperplateEngraving, "cartography_copperplate_555.png", "3. Copperplate Engraving (Seed 555)"),
        (999, PaperStyle::AntiquarianPatina, "cartography_patina_999.png", "4. Antiquarian Patina (Seed 999)"),
    ];

    let mut rendered_files = Vec::new();

    for (seed, style, filename, label) in showcase_configs {
        println!("\nGenerating World {} (Seed {})...", label, seed);
        let world = generate_world(sim_w, sim_h, seed);

        if let Some(ref net) = world.river_network {
            let mut counts = std::collections::BTreeMap::new();
            for seg in &net.segments {
                *counts.entry(seg.stream_order).or_insert(0) += 1;
            }
            println!(
                "Hydrology: {} sources, {} confluences, {} river segments. Strahler orders: {:?}",
                net.sources.len(),
                net.confluences.len(),
                net.segments.len(),
                counts
            );
        }
        
        println!("Rendering High-Resolution {} -> {}...", label, filename);
        let mut params = CartographyParams::default();
        params.style = style;
        params.upscale_factor = upscale;
        params.paper_roughness = 0.70;
        params.paper_stains = 0.65;
        params.waterline_count = 6;
        params.hachure_intensity = 0.85;
        
        let img = render_cartography_map(&world, &params);
        println!("Rendered {}x{} ({} pixels)", img.width(), img.height(), img.width() * img.height());
        img.save(filename)?;
        println!("Saved {}", filename);
        rendered_files.push(filename);
    }

    // 2. Render a Flagship Ultra-Resolution Master Map (1536x768 sim -> 6144x3072 render)
    println!("\nGenerating Flagship High-Resolution World (1536x768, Seed 777)...");
    let world_flagship = generate_world(1536, 768, 777);
    if let Some(ref net) = world_flagship.river_network {
        let mut counts = std::collections::BTreeMap::new();
        for seg in &net.segments {
            *counts.entry(seg.stream_order).or_insert(0) += 1;
        }
        println!(
            "Flagship Hydrology: {} sources, {} confluences, {} river segments. Strahler orders: {:?}",
            net.sources.len(),
            net.confluences.len(),
            net.segments.len(),
            counts
        );
    }

    println!("Rendering Flagship 6K Master Map (6144x3072)...");
    let mut params_flagship = CartographyParams::default();
    params_flagship.style = PaperStyle::AgedParchment;
    params_flagship.target_resolution = Some((6144, 3072));
    params_flagship.paper_roughness = 0.75;
    params_flagship.paper_stains = 0.70;
    params_flagship.waterline_count = 7;
    params_flagship.hachure_intensity = 0.90;

    let img_flagship = render_cartography_map(&world_flagship, &params_flagship);
    img_flagship.save("cartography_flagship_6k.png")?;
    println!("Successfully exported 6K Masterpiece: cartography_flagship_6k.png (6144x3072)");

    // 3. Stitch a 2x2 comparison quad mosaic
    println!("\nCreating 4-Style Cartography Mosaic...");
    let thumb_w = 2048u32;
    let thumb_h = 1024u32;
    let margin = 30u32;
    let title_h = 50u32;
    let total_w = margin * 3 + thumb_w * 2;
    let total_h = title_h + margin * 3 + thumb_h * 2;

    let bg_color = Rgb([20, 16, 12]);
    let mut mosaic = ImageBuffer::from_pixel(total_w, total_h, bg_color);

    for (idx, &filename) in rendered_files.iter().enumerate() {
        let col = (idx as u32) % 2;
        let row = (idx as u32) / 2;

        let cell_x = margin + col * (thumb_w + margin);
        let cell_y = title_h + margin + row * (thumb_h + margin);

        println!("Downsampling & placing {} into mosaic cell ({}, {})...", filename, col, row);
        let img = image::open(filename)?.to_rgb8();
        let resized = image::imageops::resize(&img, thumb_w, thumb_h, image::imageops::FilterType::Triangle);
        for y in 0..resized.height() {
            for x in 0..resized.width() {
                mosaic.put_pixel(cell_x + x, cell_y + y, *resized.get_pixel(x, y));
            }
        }
    }

    mosaic.save("cartography_mosaic.png")?;
    println!("\nSuccessfully generated High-Resolution Cartography Mosaic: cartography_mosaic.png ({}x{})", total_w, total_h);
    println!("All high-resolution map generation complete!\n");

    Ok(())
}
