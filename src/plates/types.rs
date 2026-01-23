use rand::Rng;
use rand_chacha::ChaCha8Rng;

/// World generation style presets that control land/ocean distribution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum WorldStyle {
    /// Earth-like distribution: ~35% land, mixed continent sizes
    #[default]
    Earthlike,
    /// Archipelago world: ~15-20% land, many small islands scattered everywhere
    Archipelago,
    /// Island chains: ~25% land, volcanic island arcs and small continents
    Islands,
    /// Pangaea-style: ~40% land concentrated in one supercontinent
    Pangaea,
    /// Continental: ~50% land, multiple large continents
    Continental,
    /// Water world: ~5-10% land, very sparse tiny islands
    Waterworld,
}

impl WorldStyle {
    /// Parse from string (for CLI)
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "earthlike" | "earth" | "default" => Some(Self::Earthlike),
            "archipelago" | "arch" => Some(Self::Archipelago),
            "islands" | "island" => Some(Self::Islands),
            "pangaea" | "supercontinent" => Some(Self::Pangaea),
            "continental" | "continents" => Some(Self::Continental),
            "waterworld" | "water" | "ocean" => Some(Self::Waterworld),
            _ => None,
        }
    }

    /// Target land fraction for this world style
    pub fn target_land_fraction(&self) -> f64 {
        match self {
            Self::Earthlike => 0.35,
            Self::Archipelago => 0.18,
            Self::Islands => 0.25,
            Self::Pangaea => 0.40,
            Self::Continental => 0.50,
            Self::Waterworld => 0.08,
        }
    }

    /// Minimum number of continental plates
    pub fn min_continental_plates(&self) -> usize {
        match self {
            Self::Earthlike => 1,
            Self::Archipelago => 4,  // Many small ones
            Self::Islands => 3,
            Self::Pangaea => 1,
            Self::Continental => 2,
            Self::Waterworld => 1,
        }
    }

    /// Maximum continental plate size as fraction of total area (0.0 = no limit)
    pub fn max_continental_plate_fraction(&self) -> f64 {
        match self {
            Self::Earthlike => 0.0,      // No limit
            Self::Archipelago => 0.08,   // Small islands only
            Self::Islands => 0.15,       // Medium islands max
            Self::Pangaea => 0.0,        // No limit, encourage big
            Self::Continental => 0.30,   // Large but not dominant
            Self::Waterworld => 0.03,    // Tiny islands only
        }
    }

    /// Whether to force many small plates
    pub fn force_many_plates(&self) -> bool {
        matches!(self, Self::Archipelago | Self::Waterworld)
    }

    /// Suggested plate count range (min, max)
    pub fn suggested_plate_count(&self) -> (usize, usize) {
        match self {
            Self::Earthlike => (6, 15),
            Self::Archipelago => (12, 20),  // More plates = more potential islands
            Self::Islands => (10, 18),
            Self::Pangaea => (5, 10),       // Fewer plates
            Self::Continental => (6, 12),
            Self::Waterworld => (15, 25),   // Many tiny plates
        }
    }

    /// All available world styles
    pub fn all() -> &'static [Self] {
        &[
            Self::Earthlike,
            Self::Archipelago,
            Self::Islands,
            Self::Pangaea,
            Self::Continental,
            Self::Waterworld,
        ]
    }

    /// Description of this world style
    pub fn description(&self) -> &'static str {
        match self {
            Self::Earthlike => "Earth-like (~35% land, mixed sizes)",
            Self::Archipelago => "Archipelago (~18% land, many tiny islands)",
            Self::Islands => "Island chains (~25% land, volcanic arcs)",
            Self::Pangaea => "Pangaea (~40% land, one supercontinent)",
            Self::Continental => "Continental (~50% land, large continents)",
            Self::Waterworld => "Waterworld (~8% land, sparse tiny islands)",
        }
    }
}

impl std::fmt::Display for WorldStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Earthlike => write!(f, "earthlike"),
            Self::Archipelago => write!(f, "archipelago"),
            Self::Islands => write!(f, "islands"),
            Self::Pangaea => write!(f, "pangaea"),
            Self::Continental => write!(f, "continental"),
            Self::Waterworld => write!(f, "waterworld"),
        }
    }
}

/// Unique identifier for a tectonic plate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct PlateId(pub u8);

impl PlateId {
    pub const NONE: PlateId = PlateId(255);

    pub fn is_none(&self) -> bool {
        *self == Self::NONE
    }
}

/// Type of tectonic plate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlateType {
    /// Oceanic plates are denser and sit lower.
    Oceanic,
    /// Continental plates are less dense and sit higher.
    Continental,
}

/// A 2D velocity vector.
#[derive(Clone, Copy, Debug)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn dot(&self, other: &Vec2) -> f32 {
        self.x * other.x + self.y * other.y
    }

    pub fn length(&self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    pub fn normalize(&self) -> Self {
        let len = self.length();
        if len > 0.0001 {
            Self {
                x: self.x / len,
                y: self.y / len,
            }
        } else {
            Self { x: 0.0, y: 0.0 }
        }
    }
}

/// A tectonic plate with its properties.
#[derive(Clone, Debug)]
pub struct Plate {
    pub id: PlateId,
    pub plate_type: PlateType,
    pub velocity: Vec2,
    pub base_elevation: f32,
    pub color: [u8; 3],
}

impl Plate {
    /// Create a border plate (always oceanic, stationary).
    /// Used for map edges to create natural coastlines.
    pub fn oceanic_border(id: PlateId) -> Self {
        Self {
            id,
            plate_type: PlateType::Oceanic,
            velocity: Vec2::new(0.0, 0.0),  // Stationary
            base_elevation: -1000.0,
            color: [20, 40, 80],  // Dark blue
        }
    }

    /// Create a plate with a specific type (continental or oceanic).
    pub fn new_with_type(id: PlateId, rng: &mut ChaCha8Rng, is_continental: bool) -> Self {
        let plate_type = if is_continental {
            PlateType::Continental
        } else {
            PlateType::Oceanic
        };

        // Base elevation based on plate type
        let base_elevation = match plate_type {
            PlateType::Oceanic => rng.gen_range(-0.4..-0.1),
            PlateType::Continental => rng.gen_range(0.05..0.2),
        };

        // Random velocity direction and magnitude
        // Lower magnitude = less stress at boundaries = rarer mountains/rifts
        let angle = rng.gen_range(0.0..std::f32::consts::TAU);
        let magnitude = rng.gen_range(0.1..0.6);
        let velocity = Vec2::new(angle.cos() * magnitude, angle.sin() * magnitude);

        // Generate a color for visualization
        let color = match plate_type {
            PlateType::Oceanic => [
                rng.gen_range(30..80),
                rng.gen_range(60..120),
                rng.gen_range(150..220),
            ],
            PlateType::Continental => [
                rng.gen_range(100..180),
                rng.gen_range(140..200),
                rng.gen_range(80..140),
            ],
        };

        Self {
            id,
            plate_type,
            velocity,
            base_elevation,
            color,
        }
    }

    /// Generate a random plate with the given ID (original behavior).
    pub fn random(id: PlateId, rng: &mut ChaCha8Rng) -> Self {
        // ~60% oceanic, ~40% continental
        let plate_type = if rng.gen::<f32>() < 0.6 {
            PlateType::Oceanic
        } else {
            PlateType::Continental
        };

        // Base elevation based on plate type
        let base_elevation = match plate_type {
            PlateType::Oceanic => rng.gen_range(-0.4..-0.1),
            PlateType::Continental => rng.gen_range(0.05..0.2),
        };

        // Random velocity direction and magnitude
        // Lower magnitude = less stress at boundaries = rarer mountains/rifts
        let angle = rng.gen_range(0.0..std::f32::consts::TAU);
        let magnitude = rng.gen_range(0.1..0.6);
        let velocity = Vec2::new(angle.cos() * magnitude, angle.sin() * magnitude);

        // Generate a color for visualization
        let color = match plate_type {
            PlateType::Oceanic => [
                rng.gen_range(30..80),
                rng.gen_range(60..120),
                rng.gen_range(150..220),
            ],
            PlateType::Continental => [
                rng.gen_range(100..180),
                rng.gen_range(140..200),
                rng.gen_range(80..140),
            ],
        };

        Self {
            id,
            plate_type,
            velocity,
            base_elevation,
            color,
        }
    }
}
