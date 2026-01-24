//! Erosion simulation parameters and configuration

/// Erosion intensity preset
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ErosionPreset {
    /// No erosion - raw terrain
    None,
    /// Minimal erosion - subtle smoothing
    Minimal,
    /// Normal erosion - balanced
    #[default]
    Normal,
    /// Dramatic erosion - deep valleys and canyons
    Dramatic,
    /// Realistic erosion - high iteration count
    Realistic,
}

impl ErosionPreset {
    pub fn all() -> &'static [Self] {
        &[Self::None, Self::Minimal, Self::Normal, Self::Dramatic, Self::Realistic]
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::None => "No erosion (raw terrain)",
            Self::Minimal => "Subtle smoothing",
            Self::Normal => "Balanced erosion",
            Self::Dramatic => "Deep valleys and canyons",
            Self::Realistic => "High-quality simulation",
        }
    }
}

impl std::fmt::Display for ErosionPreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Minimal => write!(f, "minimal"),
            Self::Normal => write!(f, "normal"),
            Self::Dramatic => write!(f, "dramatic"),
            Self::Realistic => write!(f, "realistic"),
        }
    }
}

/// Global erosion simulation parameters
#[derive(Clone, Debug, PartialEq)]
pub struct ErosionParams {
    // =========================================================================
    // Hydraulic Erosion Parameters
    // =========================================================================

    /// Number of water droplets to simulate (default: 50000)
    pub hydraulic_iterations: usize,

    /// Momentum conservation factor (0.0-1.0)
    /// Higher values = droplets maintain direction longer, creating straighter paths
    pub droplet_inertia: f32,

    /// Sediment carrying capacity multiplier
    /// Higher values = droplets can carry more sediment before depositing
    pub droplet_capacity_factor: f32,

    /// Rate at which droplets erode terrain (0.0-1.0)
    /// Modulated by rock hardness
    pub droplet_erosion_rate: f32,

    /// Rate at which droplets deposit sediment (0.0-1.0)
    pub droplet_deposit_rate: f32,

    /// Water evaporation rate per step (0.0-1.0)
    /// Higher values = shorter droplet lifetimes
    pub droplet_evaporation: f32,

    /// Minimum water volume before droplet dies
    pub droplet_min_volume: f32,

    /// Maximum path length (steps) per droplet
    pub droplet_max_steps: usize,

    /// Radius for blurring erosion effects (in cells)
    pub droplet_erosion_radius: usize,

    /// Initial water volume for each droplet
    pub droplet_initial_water: f32,

    /// Initial velocity for droplets
    pub droplet_initial_velocity: f32,

    /// Gravity factor affecting droplet acceleration
    pub droplet_gravity: f32,

    // =========================================================================
    // Glacial Erosion Parameters (SIA Model)
    // =========================================================================

    /// Number of simulation timesteps for glacial erosion
    pub glacial_timesteps: usize,

    /// Time delta per step (in years)
    pub glacial_dt: f32,

    /// Glen's flow law coefficient A (ice deformation rate)
    /// Typical value: 2.4e-24 Pa^-3 s^-1 (but we use scaled values)
    pub ice_deform_coefficient: f32,

    /// Basal sliding coefficient (m/yr per Pa)
    pub ice_sliding_coefficient: f32,

    /// Bedrock erosion coefficient K (erodibility)
    pub erosion_coefficient: f32,

    /// Mass balance gradient (accumulation/ablation rate per meter above/below ELA)
    pub mass_balance_gradient: f32,

    /// Equilibrium Line Altitude (ELA) - elevation where accumulation = ablation
    /// This is derived from temperature, but can be overridden
    pub snowline_elevation: Option<f32>,

    /// Temperature threshold for ice formation (Celsius)
    pub glaciation_temperature: f32,

    /// Glen's flow law exponent (typically n=3)
    pub glen_exponent: f32,

    /// Ice density (kg/m^3)
    pub ice_density: f32,

    /// Gravitational acceleration (m/s^2)
    pub gravity: f32,

    /// Erosion law exponent (1 = linear, 2 = quadratic)
    pub erosion_exponent: f32,

    // =========================================================================
    // River Erosion Parameters (Trace-Based with Sediment Transport)
    // =========================================================================

    /// Enable flow-based river erosion (creates main drainage channels)
    pub enable_rivers: bool,

    /// Minimum flow accumulation for a cell to be a river source
    pub river_source_min_accumulation: f32,

    /// Minimum elevation above sea level for river sources
    pub river_source_min_elevation: f32,

    /// Sediment capacity multiplier (capacity = factor * flow * slope)
    pub river_capacity_factor: f32,

    /// Rate at which rivers erode when under capacity
    pub river_erosion_rate: f32,

    /// Rate at which rivers deposit when over capacity
    pub river_deposition_rate: f32,

    /// Maximum erosion per cell (prevents extreme valleys)
    pub river_max_erosion: f32,

    /// Maximum deposition per cell
    pub river_max_deposition: f32,

    /// Width of river channel (for cross-section erosion)
    pub river_channel_width: usize,

    // =========================================================================
    // General Settings
    // =========================================================================

    /// Enable particle-based hydraulic erosion (adds detail)
    pub enable_hydraulic: bool,

    /// Enable glacial erosion
    pub enable_glacial: bool,

    /// Enable geomorphometry analysis (realism scoring)
    pub enable_analysis: bool,

    /// Use GPU acceleration for hydraulic erosion (if available)
    pub use_gpu: bool,

    /// Simulation scale factor for high-resolution erosion.
    /// 4 = 4x upscale (default), 1 = no upscaling (faster but lower quality).
    /// Higher values produce sharper river channels but use more memory and time.
    pub simulation_scale: usize,

    /// Roughness strength for high-res erosion "crumple" effect.
    /// Higher values create more terrain variation, encouraging river meandering.
    /// Try 15.0-30.0. Only applies when simulation_scale > 1.
    pub hires_roughness: f32,

    /// Domain warp strength for high-res erosion.
    /// Creates organic, non-linear flow paths. Try 5.0-15.0.
    /// Only applies when simulation_scale > 1.
    pub hires_warp: f32,
}

impl Default for ErosionParams {
    fn default() -> Self {
        Self {
            // Hydraulic erosion defaults - "POLISHED" config: sharp rivers that still merge
            hydraulic_iterations: 750_000,
            droplet_inertia: 0.3,           // Low inertia - water turns easily, meanders naturally
            droplet_capacity_factor: 10.0,
            droplet_erosion_rate: 0.05,     // Slow digging - prevents trench lock
            droplet_deposit_rate: 0.2,      // Moderate deposition - forces merging without blobby rivers
            droplet_evaporation: 0.001,     // Low evaporation - long-lived droplets find merges
            droplet_min_volume: 0.01,
            droplet_max_steps: 3000,
            droplet_erosion_radius: 3,      // Medium brush - sharp valleys, still breaks parallel streams
            droplet_initial_water: 1.0,
            droplet_initial_velocity: 1.0,
            droplet_gravity: 8.0,

            // Glacial erosion defaults (scaled for our heightmap units)
            glacial_timesteps: 500,
            glacial_dt: 100.0,  // 100 years per step
            ice_deform_coefficient: 1e-7,  // Scaled Glen's A
            ice_sliding_coefficient: 5e-4,  // Basal sliding factor
            erosion_coefficient: 1e-4,  // Bedrock erodibility
            mass_balance_gradient: 0.005,  // m/yr per m elevation
            snowline_elevation: None,  // Derived from temperature
            glaciation_temperature: -3.0,  // Ice forms below this temp (enables coastal glaciation for fjords)
            glen_exponent: 3.0,
            ice_density: 917.0,  // kg/m^3
            gravity: 9.81,  // m/s^2
            erosion_exponent: 1.0,  // Linear erosion law

            // River erosion defaults - very dense capillary network
            enable_rivers: true,
            river_source_min_accumulation: 15.0,   // Baseline threshold
            river_source_min_elevation: 100.0,     // Start higher up for longer rivers
            river_capacity_factor: 20.0,           // High capacity = more erosion
            river_erosion_rate: 1.0,               // Maximum erosion rate
            river_deposition_rate: 0.5,            // Disable deposition
            river_max_erosion: 150.0,              // Deep channels
            river_max_deposition: 0.0,             // No deposition
            river_channel_width: 2,                // Wide channels for visibility

            // General
            enable_hydraulic: true,       // Enabled (was false)
            enable_glacial: true,         // Enabled for fjords and glacial valleys
            enable_analysis: true,        // Enabled for testing
            use_gpu: true,                // Use GPU if available
            simulation_scale: 4,          // 4x upscale for high-quality river channels
            hires_roughness: 20.0,        // Roughness for river meandering
            hires_warp: 0.0,              // Disabled - meandering via targeted roughness + meander erosion
        }
    }
}

impl ErosionParams {
    /// Create a fast configuration for testing (fewer iterations)
    pub fn fast() -> Self {
        Self {
            hydraulic_iterations: 10_000,
            glacial_timesteps: 100,
            ..Default::default()
        }
    }

    /// Create a high-quality configuration (more iterations)
    pub fn high_quality() -> Self {
        Self {
            hydraulic_iterations: 200_000,
            glacial_timesteps: 1000,
            ..Default::default()
        }
    }

    /// Only hydraulic erosion
    pub fn hydraulic_only() -> Self {
        Self {
            enable_glacial: false,
            ..Default::default()
        }
    }

    /// Only glacial erosion
    pub fn glacial_only() -> Self {
        Self {
            enable_hydraulic: false,
            ..Default::default()
        }
    }

    /// Compute ice density * gravity (commonly used in SIA)
    pub fn rho_g(&self) -> f32 {
        self.ice_density * self.gravity
    }

    /// Create parameters from a preset
    pub fn from_preset(preset: ErosionPreset) -> Self {
        match preset {
            ErosionPreset::None => Self {
                enable_hydraulic: false,
                enable_glacial: false,
                enable_rivers: false,
                ..Default::default()
            },
            ErosionPreset::Minimal => Self {
                hydraulic_iterations: 50_000,
                glacial_timesteps: 100,
                droplet_erosion_rate: 0.02,
                river_max_erosion: 50.0,
                ..Default::default()
            },
            ErosionPreset::Normal => Self::default(),
            ErosionPreset::Dramatic => Self {
                hydraulic_iterations: 750_000,
                glacial_timesteps: 750,
                droplet_erosion_rate: 0.1,
                droplet_capacity_factor: 15.0,
                river_max_erosion: 250.0,
                river_erosion_rate: 1.5,
                erosion_coefficient: 2e-4,
                ..Default::default()
            },
            ErosionPreset::Realistic => Self {
                hydraulic_iterations: 1_000_000,
                glacial_timesteps: 1000,
                droplet_erosion_rate: 0.03,
                droplet_deposit_rate: 0.15,
                droplet_evaporation: 0.001,
                droplet_max_steps: 3000,
                river_source_min_accumulation: 5.0,
                ..Default::default()
            },
        }
    }
}
