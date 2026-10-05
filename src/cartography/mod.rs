//! Antique cartography paper map shader rendering pipeline
//!
//! Provides antique parchment paper generation, copperplate mountain engraving (hachuring),
//! ocean waterlining contour rings, hand-tinted watercolor washes, nautical rhumb lines,
//! compass roses, and decorative vintage map borders.

pub mod cpu;
pub mod decorations;
#[cfg(feature = "gpu")]
pub mod gpu;
/// Without the `gpu` feature cartography renders on the CPU.
#[cfg(not(feature = "gpu"))]
pub mod gpu {
    pub struct GpuCartographyContext;
    impl GpuCartographyContext {
        pub fn new() -> Option<Self> { None }
        pub fn render(&self, _: &crate::world::WorldData, _: &super::CartographyParams) -> Option<image::ImageBuffer<image::Rgb<u8>, Vec<u8>>> { None }
    }
}
pub mod params;

use std::error::Error;
use image::{ImageBuffer, Rgb};

pub use params::{CartographyParams, PaperStyle};
use crate::world::WorldData;

/// Render an antique cartography paper map of the world
/// Automatically attempts GPU compute shader rendering if enabled, falling back to multi-threaded CPU.
pub fn render_cartography_map(
    world: &WorldData,
    params: &CartographyParams,
) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
    if params.use_gpu {
        if let Some(gpu_ctx) = gpu::GpuCartographyContext::new() {
            if let Some(img) = gpu_ctx.render(world, params) {
                return img;
            }
        }
    }

    // High-performance CPU software shader fallback (Rayon)
    cpu::render_cartography_cpu(world, params)
}

/// Export an antique cartography paper map image directly to a file
pub fn export_cartography_image(
    world: &WorldData,
    filename: &str,
    params: Option<&CartographyParams>,
) -> Result<(), Box<dyn Error>> {
    let default_params = CartographyParams::default();
    let p = params.unwrap_or(&default_params);

    let img = render_cartography_map(world, p);
    img.save(filename)?;
    println!("Exported antique cartography map to {}", filename);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::generate_world;

    #[test]
    fn test_cartography_render_cpu() {
        let world = generate_world(64, 32, 42);
        let mut params = CartographyParams::default();
        params.use_gpu = false; // test CPU path directly
        let img = render_cartography_map(&world, &params);
        assert_eq!(img.width(), 64);
        assert_eq!(img.height(), 32);
    }

    #[test]
    fn test_cartography_all_styles() {
        let world = generate_world(64, 32, 101);
        for style in [
            PaperStyle::AgedParchment,
            PaperStyle::Atlas17thCentury,
            PaperStyle::CopperplateEngraving,
            PaperStyle::AntiquarianPatina,
        ] {
            let mut params = CartographyParams::default();
            params.style = style;
            params.use_gpu = false;
            let img = render_cartography_map(&world, &params);
            assert_eq!(img.width(), 64);
        }
    }

    #[test]
    fn test_cartography_upscaling() {
        let world = generate_world(64, 32, 202);
        let mut params = CartographyParams::default();
        params.use_gpu = false;
        params.upscale_factor = 4;
        let img = render_cartography_map(&world, &params);
        assert_eq!(img.width(), 256);
        assert_eq!(img.height(), 128);

        // Custom target resolution
        params.target_resolution = Some((500, 250));
        let img_custom = render_cartography_map(&world, &params);
        assert_eq!(img_custom.width(), 500);
        assert_eq!(img_custom.height(), 250);
    }
}

