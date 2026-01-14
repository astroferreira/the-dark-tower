//! Climate system for generating temperature and moisture maps
//! Based on latitude, elevation, and ocean proximity

use crate::tilemap::Tilemap;

// =============================================================================
// CLIMATE PARAMETERS
// =============================================================================

/// Temperature at equator at sea level (Celsius)
const EQUATOR_TEMP: f32 = 30.0;

/// Temperature at poles at sea level (Celsius)
const POLE_TEMP: f32 = -30.0;

/// Temperature drop per 1000m elevation (lapse rate)
const ELEVATION_LAPSE_RATE: f32 = 6.5;

/// Ocean temperature moderation factor (0-1)
const OCEAN_MODERATION: f32 = 0.3;

// =============================================================================
// TEMPERATURE GENERATION
// =============================================================================

/// Generate temperature map based on latitude and elevation
/// Returns temperature in Celsius
pub fn generate_temperature(
    heightmap: &Tilemap<f32>,
    width: usize,
    height: usize,
) -> Tilemap<f32> {
    let mut temperature = Tilemap::new_with(width, height, 0.0f32);
    
    for y in 0..height {
        for x in 0..width {
            let elevation = *heightmap.get(x, y);
            
            // Latitude factor: 0 at equator, 1 at poles
            // Map y to latitude: y=0 is north pole, y=height/2 is equator, y=height is south pole
            let latitude_normalized = (y as f32 / height as f32 - 0.5).abs() * 2.0;
            
            // Base temperature from latitude (cosine curve for smoother transition)
            let lat_factor = latitude_normalized.powf(1.5); // More gradual near equator
            let base_temp = EQUATOR_TEMP - (EQUATOR_TEMP - POLE_TEMP) * lat_factor;
            
            // Elevation adjustment (only for land above sea level)
            let elevation_adjustment = if elevation > 0.0 {
                // Lapse rate: temperature drops with altitude
                -(elevation / 1000.0) * ELEVATION_LAPSE_RATE
            } else {
                // Ocean: slight warming effect in shallow water
                0.0
            };
            
            let temp = base_temp + elevation_adjustment;
            temperature.set(x, y, temp);
        }
    }
    
    temperature
}

// =============================================================================
// MOISTURE GENERATION
// =============================================================================

/// Generate moisture map based on ocean proximity and elevation
/// Returns moisture as 0.0-1.0
pub fn generate_moisture(
    heightmap: &Tilemap<f32>,
    width: usize,
    height: usize,
) -> Tilemap<f32> {
    use std::collections::VecDeque;
    
    // First pass: compute distance from ocean
    let mut ocean_distance = Tilemap::new_with(width, height, f32::MAX);
    let mut queue: VecDeque<(usize, usize, f32)> = VecDeque::new();
    
    // Initialize with ocean cells
    for y in 0..height {
        for x in 0..width {
            if *heightmap.get(x, y) <= 0.0 {
                ocean_distance.set(x, y, 0.0);
                queue.push_back((x, y, 0.0));
            }
        }
    }
    
    // BFS to compute distance
    while let Some((x, y, dist)) = queue.pop_front() {
        let neighbors = [
            (x.wrapping_sub(1), y),
            (x + 1, y),
            (x, y.wrapping_sub(1)),
            (x, y + 1),
        ];
        
        for (nx, ny) in neighbors {
            if nx >= width || ny >= height {
                continue;
            }
            let new_dist = dist + 1.0;
            if new_dist < *ocean_distance.get(nx, ny) {
                ocean_distance.set(nx, ny, new_dist);
                queue.push_back((nx, ny, new_dist));
            }
        }
    }
    
    // Second pass: compute moisture from distance
    let mut moisture = Tilemap::new_with(width, height, 0.0f32);
    let max_distance = 150.0; // Distance at which moisture drops to minimum
    
    for y in 0..height {
        for x in 0..width {
            let elevation = *heightmap.get(x, y);
            let dist = *ocean_distance.get(x, y);
            
            // Ocean is always max moisture
            if elevation <= 0.0 {
                moisture.set(x, y, 1.0);
                continue;
            }
            
            // Base moisture from ocean proximity
            let proximity_moisture = (1.0 - dist / max_distance).max(0.1);
            
            // Mountains can catch moisture (orographic effect)
            let orographic_bonus = if elevation > 500.0 && dist < 100.0 {
                0.2 * (elevation / 2000.0).min(1.0)
            } else {
                0.0
            };
            
            // Very high elevations are dry (above cloud level)
            let high_altitude_penalty = if elevation > 3000.0 {
                ((elevation - 3000.0) / 2000.0).min(0.5)
            } else {
                0.0
            };
            
            // Latitude affects moisture (tropical = wet, polar = dry)
            let latitude_normalized = (y as f32 / height as f32 - 0.5).abs() * 2.0;
            let tropical_bonus = if latitude_normalized < 0.3 {
                0.15 * (1.0 - latitude_normalized / 0.3)
            } else {
                0.0
            };
            
            let final_moisture = (proximity_moisture + orographic_bonus + tropical_bonus - high_altitude_penalty)
                .clamp(0.0, 1.0);
            
            moisture.set(x, y, final_moisture);
        }
    }
    
    moisture
}

// =============================================================================
// BIOME CLASSIFICATION
// =============================================================================

/// Biome types based on temperature and moisture
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Biome {
    // Ocean biomes
    DeepOcean,
    Ocean,
    CoastalWater,
    
    // Cold biomes
    Ice,
    Tundra,
    BorealForest,
    
    // Temperate biomes
    TemperateGrassland,
    TemperateForest,
    TemperateRainforest,
    
    // Warm biomes
    Desert,
    Savanna,
    TropicalForest,
    TropicalRainforest,
    
    // Mountain biomes
    AlpineTundra,
    SnowyPeaks,
}

impl Biome {
    /// Classify biome based on temperature (Celsius) and moisture (0-1)
    pub fn classify(elevation: f32, temperature: f32, moisture: f32) -> Biome {
        // Ocean biomes
        if elevation <= 0.0 {
            if elevation < -2000.0 {
                return Biome::DeepOcean;
            } else if elevation < -100.0 {
                return Biome::Ocean;
            } else {
                return Biome::CoastalWater;
            }
        }
        
        // High elevation biomes
        if elevation > 2500.0 {
            if temperature < -10.0 {
                return Biome::SnowyPeaks;
            } else {
                return Biome::AlpineTundra;
            }
        }
        
        // Land biomes based on temperature and moisture
        match (temperature, moisture) {
            // Freezing temperatures
            (t, _) if t < -10.0 => Biome::Ice,
            (t, _) if t < 0.0 => Biome::Tundra,
            
            // Cold temperatures
            (t, m) if t < 10.0 => {
                if m > 0.5 { Biome::BorealForest } else { Biome::Tundra }
            }
            
            // Temperate temperatures
            (t, m) if t < 20.0 => {
                if m > 0.7 { Biome::TemperateRainforest }
                else if m > 0.4 { Biome::TemperateForest }
                else { Biome::TemperateGrassland }
            }
            
            // Warm/tropical temperatures
            (_, m) => {
                if m > 0.7 { Biome::TropicalRainforest }
                else if m > 0.4 { Biome::TropicalForest }
                else if m > 0.2 { Biome::Savanna }
                else { Biome::Desert }
            }
        }
    }
    
    /// Get RGB color for biome visualization
    pub fn color(&self) -> (u8, u8, u8) {
        match self {
            // Ocean
            Biome::DeepOcean => (20, 40, 80),
            Biome::Ocean => (30, 60, 120),
            Biome::CoastalWater => (60, 100, 160),
            
            // Cold
            Biome::Ice => (240, 250, 255),
            Biome::Tundra => (180, 190, 170),
            Biome::BorealForest => (50, 80, 50),
            
            // Temperate
            Biome::TemperateGrassland => (140, 170, 80),
            Biome::TemperateForest => (40, 100, 40),
            Biome::TemperateRainforest => (30, 80, 50),
            
            // Warm
            Biome::Desert => (210, 180, 120),
            Biome::Savanna => (170, 160, 80),
            Biome::TropicalForest => (30, 120, 30),
            Biome::TropicalRainforest => (20, 90, 40),
            
            // Mountain
            Biome::AlpineTundra => (140, 140, 130),
            Biome::SnowyPeaks => (255, 255, 255),
        }
    }
}

/// Generate biome map from heightmap, temperature, and moisture
pub fn generate_biomes(
    heightmap: &Tilemap<f32>,
    temperature: &Tilemap<f32>,
    moisture: &Tilemap<f32>,
) -> Tilemap<Biome> {
    let width = heightmap.width;
    let height = heightmap.height;
    
    let mut biomes = Tilemap::new_with(width, height, Biome::Ocean);
    
    for y in 0..height {
        for x in 0..width {
            let elev = *heightmap.get(x, y);
            let temp = *temperature.get(x, y);
            let moist = *moisture.get(x, y);
            
            let biome = Biome::classify(elev, temp, moist);
            biomes.set(x, y, biome);
        }
    }
    
    biomes
}
