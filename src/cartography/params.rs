//! Parameters and configuration for antique cartography paper rendering

use serde::{Deserialize, Serialize};

/// Visual presets for cartography old paper rendering
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaperStyle {
    /// Classic warm aged parchment with tea stains and subtle creases
    AgedParchment,
    /// 17th-century European atlas style (Blaeu / Ortelius style, muted watercolor washes)
    Atlas17thCentury,
    /// Monochrome copperplate engraved nautical chart
    CopperplateEngraving,
    /// Dark weathered antiquarian manuscript with heavy patina and burned edges
    AntiquarianPatina,
}

impl Default for PaperStyle {
    fn default() -> Self {
        PaperStyle::AgedParchment
    }
}

/// Configuration parameters for cartography rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartographyParams {
    /// Overall visual style preset
    pub style: PaperStyle,
    /// Intensity of paper fiber grain and surface roughness (0.0 - 1.0)
    pub paper_roughness: f32,
    /// Intensity of tea stains, foxing spots, and age discoloration (0.0 - 1.0)
    pub paper_stains: f32,
    /// Strength of aged edge vignette / burned border patina (0.0 - 1.0)
    pub vignette_strength: f32,
    /// Number of concentric coastal waterline rings radiating into sea (0 - 8)
    pub waterline_count: usize,
    /// Base spacing between waterline contour rings in pixels
    pub waterline_spacing: f32,
    /// Intensity of mountain engraving hachure strokes (0.0 - 1.0)
    pub hachure_intensity: f32,
    /// Frequency / density of hachure engraving lines
    pub hachure_density: f32,
    /// Opacity and vibrance of hand-tinted biome watercolor washes (0.0 - 1.0)
    pub watercolor_opacity: f32,
    /// Whether to render nautical navigation rhumb lines across oceans
    pub show_rhumb_lines: bool,
    /// Whether to render latitude/longitude graticule lines
    pub show_graticule: bool,
    /// Whether to draw an ornate antique compass rose / windrose
    pub show_compass_rose: bool,
    /// Whether to frame the map with a vintage double-ruler border
    pub show_vintage_border: bool,
    /// Thickness of vintage border frame in pixels
    pub border_width: usize,
    /// Integer upscaling factor (1 = native, 2 = 2x, 4 = 4x, etc.)
    pub upscale_factor: usize,
    /// Explicit target resolution (width, height) overriding upscale_factor if specified
    pub target_resolution: Option<(usize, usize)>,
    /// Try GPU compute shader first, falling back to multi-threaded CPU
    pub use_gpu: bool,
}

impl Default for CartographyParams {
    fn default() -> Self {
        Self {
            style: PaperStyle::AgedParchment,
            paper_roughness: 0.65,
            paper_stains: 0.60,
            vignette_strength: 0.70,
            waterline_count: 5,
            waterline_spacing: 3.2,
            hachure_intensity: 0.85,
            hachure_density: 0.65,
            watercolor_opacity: 0.75,
            show_rhumb_lines: false,
            show_graticule: false,
            show_compass_rose: false,
            show_vintage_border: false,
            border_width: 0,
            upscale_factor: 1,
            target_resolution: None,
            use_gpu: false,
        }
    }
}
