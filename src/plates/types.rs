use rand::Rng;
use rand_chacha::ChaCha8Rng;

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

    /// Generate a random plate with the given ID.
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
