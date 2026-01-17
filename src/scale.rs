//! Map scale configuration for generating maps at different physical scales.
//!
//! Supports scales from local (1 km/tile) to planetary (50 km/tile).

/// Map scale configuration
#[derive(Clone, Copy, Debug)]
pub struct MapScale {
    /// Physical distance one tile represents (in kilometers)
    pub km_per_tile: f32,

    /// Scale factor for distance-based thresholds (linear scaling)
    /// Reference: 5 km/tile = 1.0
    pub distance_scale: f32,

    /// Scale factor for noise frequencies (inverse scaling)
    /// Larger tiles need lower frequencies for larger features
    /// Reference: 5 km/tile = 1.0
    pub frequency_scale: f32,

    /// Scale factor for elevation thresholds (square root scaling)
    /// Maintains proportional mountain heights across scales
    /// Reference: 5 km/tile = 1.0
    pub elevation_scale: f32,
}

/// Reference scale (5 km per tile) - kingdom/regional maps
const REFERENCE_KM: f32 = 5.0;

impl MapScale {
    /// Create a new scale configuration
    ///
    /// # Arguments
    /// * `km_per_tile` - Physical distance one tile represents in kilometers
    pub fn new(km_per_tile: f32) -> Self {
        let ratio = km_per_tile / REFERENCE_KM;
        Self {
            km_per_tile,
            distance_scale: ratio,
            frequency_scale: 1.0 / ratio,
            elevation_scale: ratio.sqrt(),
        }
    }

    /// Planetary scale - full planet view (50 km/tile)
    /// At 512x256: ~25,600 x 12,800 km (Earth-like)
    pub fn planetary() -> Self {
        Self::new(50.0)
    }

    /// Continental scale - country-level view (20 km/tile)
    /// At 512x256: ~10,240 x 5,120 km
    pub fn continental() -> Self {
        Self::new(20.0)
    }

    /// Regional scale - kingdom-level view (5 km/tile) [DEFAULT]
    /// At 512x256: ~2,560 x 1,280 km
    pub fn regional() -> Self {
        Self::new(5.0)
    }

    /// Local scale - adventure-level view (1 km/tile)
    /// At 512x256: ~512 x 256 km
    pub fn local() -> Self {
        Self::new(1.0)
    }

    /// Calculate the total map dimensions in kilometers
    pub fn map_size_km(&self, width: usize, height: usize) -> (f32, f32) {
        (width as f32 * self.km_per_tile, height as f32 * self.km_per_tile)
    }

    /// Format map size as a human-readable string
    pub fn format_map_size(&self, width: usize, height: usize) -> String {
        let (w_km, h_km) = self.map_size_km(width, height);
        if w_km >= 1000.0 {
            format!("{:.1} × {:.1} thousand km", w_km / 1000.0, h_km / 1000.0)
        } else {
            format!("{:.0} × {:.0} km", w_km, h_km)
        }
    }

    /// Get a descriptive name for this scale
    pub fn name(&self) -> &'static str {
        if self.km_per_tile >= 40.0 {
            "Planetary"
        } else if self.km_per_tile >= 15.0 {
            "Continental"
        } else if self.km_per_tile >= 3.0 {
            "Regional"
        } else {
            "Local"
        }
    }
}

impl Default for MapScale {
    fn default() -> Self {
        Self::regional()
    }
}

/// Scale presets for UI selection
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScalePreset {
    Planetary,
    Continental,
    Regional,
    Local,
    Custom,
}

impl ScalePreset {
    /// Get all presets in order
    pub fn all() -> &'static [ScalePreset] {
        &[
            ScalePreset::Planetary,
            ScalePreset::Continental,
            ScalePreset::Regional,
            ScalePreset::Local,
            ScalePreset::Custom,
        ]
    }

    /// Get display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            ScalePreset::Planetary => "Planetary (50 km/tile)",
            ScalePreset::Continental => "Continental (20 km/tile)",
            ScalePreset::Regional => "Regional (5 km/tile)",
            ScalePreset::Local => "Local (1 km/tile)",
            ScalePreset::Custom => "Custom",
        }
    }

    /// Get the MapScale for this preset
    pub fn to_scale(&self) -> Option<MapScale> {
        match self {
            ScalePreset::Planetary => Some(MapScale::planetary()),
            ScalePreset::Continental => Some(MapScale::continental()),
            ScalePreset::Regional => Some(MapScale::regional()),
            ScalePreset::Local => Some(MapScale::local()),
            ScalePreset::Custom => None, // User must specify
        }
    }

    /// Determine which preset matches a given km_per_tile value
    pub fn from_km(km: f32) -> ScalePreset {
        if (km - 50.0).abs() < 0.1 {
            ScalePreset::Planetary
        } else if (km - 20.0).abs() < 0.1 {
            ScalePreset::Continental
        } else if (km - 5.0).abs() < 0.1 {
            ScalePreset::Regional
        } else if (km - 1.0).abs() < 0.1 {
            ScalePreset::Local
        } else {
            ScalePreset::Custom
        }
    }
}

// =============================================================================
// HELPER FUNCTIONS FOR SCALING VALUES
// =============================================================================

/// Scale a distance-based threshold (linear scaling)
/// Use for: coastal ranges, detection distances, river widths
#[inline]
pub fn scale_distance(base: f32, scale: &MapScale) -> f32 {
    base * scale.distance_scale
}

/// Scale a noise frequency (inverse scaling)
/// Use for: terrain noise, biome noise, detail frequencies
#[inline]
pub fn scale_frequency(base: f64, scale: &MapScale) -> f64 {
    base * scale.frequency_scale as f64
}

/// Scale an elevation threshold (square root scaling)
/// Use for: mountain heights, altitude penalties, erosion limits
#[inline]
pub fn scale_elevation(base: f32, scale: &MapScale) -> f32 {
    base * scale.elevation_scale
}

/// Scale a pixel/tile count (linear scaling)
/// Use for: channel widths in tiles, detection radii in pixels
#[inline]
pub fn scale_tiles(base: usize, scale: &MapScale) -> usize {
    ((base as f32) * scale.distance_scale).max(1.0) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scale_factors() {
        let regional = MapScale::regional();
        assert!((regional.distance_scale - 1.0).abs() < 0.001);
        assert!((regional.frequency_scale - 1.0).abs() < 0.001);
        assert!((regional.elevation_scale - 1.0).abs() < 0.001);

        let planetary = MapScale::planetary();
        assert!((planetary.distance_scale - 10.0).abs() < 0.001);
        assert!((planetary.frequency_scale - 0.1).abs() < 0.001);
        assert!((planetary.elevation_scale - 3.162).abs() < 0.01);

        let local = MapScale::local();
        assert!((local.distance_scale - 0.2).abs() < 0.001);
        assert!((local.frequency_scale - 5.0).abs() < 0.001);
        assert!((local.elevation_scale - 0.447).abs() < 0.01);
    }

    #[test]
    fn test_map_size() {
        let scale = MapScale::regional();
        let (w, h) = scale.map_size_km(512, 256);
        assert!((w - 2560.0).abs() < 0.1);
        assert!((h - 1280.0).abs() < 0.1);
    }
}
